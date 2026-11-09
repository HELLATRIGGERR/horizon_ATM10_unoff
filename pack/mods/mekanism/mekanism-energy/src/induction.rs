//! Mekanism's Induction Matrix (`MatrixMultiblockData`, `MatrixValidator`, `MatrixEnergyContainer`): a cuboid of
//! induction casings with induction ports and structural glass in its sides, and induction cells, providers or air
//! inside. Its Joules live in its cells, each a store of its tier's capacity (`tiers.toml` `[induction]`
//! `<tier>Capacity`) in the cell's own block entity, so they stay when the matrix breaks; its providers add up to
//! how much goes in and how much goes out a tick (`<tier>Output`). What goes in and out during a tick is queued and
//! lands in the cells at the matrix's next tick, the first cells first. A port takes energy in its input mode and
//! gives it in its output mode, where it also puts it out to the block it faces.
use std::path::Path;

/// `InductionCellTier` and `InductionProviderTier`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InductionTier {
    Basic,
    Advanced,
    Elite,
    Ultimate,
}

impl InductionTier {
    pub const ALL: [Self; 4] = [Self::Basic, Self::Advanced, Self::Elite, Self::Ultimate];

    pub fn name(self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Advanced => "advanced",
            Self::Elite => "elite",
            Self::Ultimate => "ultimate",
        }
    }

    /// `mekanism:<tier>_induction_cell`.
    pub fn of_cell(block: &str) -> Option<Self> {
        let rest = block.strip_prefix("mekanism:")?.strip_suffix("_induction_cell")?;
        Self::ALL.into_iter().find(|t| t.name() == rest)
    }

    /// `mekanism:<tier>_induction_provider`.
    pub fn of_provider(block: &str) -> Option<Self> {
        let rest = block.strip_prefix("mekanism:")?.strip_suffix("_induction_provider")?;
        Self::ALL.into_iter().find(|t| t.name() == rest)
    }

    fn default_capacity(self) -> u64 {
        match self {
            Self::Basic => 8_000_000_000,
            Self::Advanced => 64_000_000_000,
            Self::Elite => 512_000_000_000,
            Self::Ultimate => 4_000_000_000_000,
        }
    }

    fn default_output(self) -> u64 {
        match self {
            Self::Basic => 256_000,
            Self::Advanced => 2_048_000,
            Self::Elite => 16_384_000,
            Self::Ultimate => 131_072_000,
        }
    }
}

/// `tiers.toml` `[induction]`: each cell tier's Joules and each provider tier's Joules a tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InductionConfig {
    pub cells: [u64; 4],
    pub providers: [u64; 4],
}

impl Default for InductionConfig {
    fn default() -> Self {
        Self { cells: InductionTier::ALL.map(InductionTier::default_capacity), providers: InductionTier::ALL.map(InductionTier::default_output) }
    }
}

impl InductionConfig {
    pub fn from_tiers(tiers: Option<&toml::Table>) -> Self {
        let mut config = Self::default();
        let Some(section) = tiers.and_then(|t| t.get("induction")).and_then(toml::Value::as_table) else {
            return config;
        };
        let positive = |key: String| section.get(&key).and_then(toml::Value::as_integer).filter(|v| *v >= 1).map(|v| v as u64);
        for (i, tier) in InductionTier::ALL.into_iter().enumerate() {
            if let Some(v) = positive(format!("{}Capacity", tier.name())) {
                config.cells[i] = v;
            }
            if let Some(v) = positive(format!("{}Output", tier.name())) {
                config.providers[i] = v;
            }
        }
        config
    }

    /// `<config>/Mekanism/tiers.toml`.
    pub fn load(config: &Path) -> Self {
        let tiers = std::fs::read_to_string(config.join("Mekanism").join("tiers.toml")).ok().and_then(|t| t.parse::<toml::Table>().ok());
        Self::from_tiers(tiers.as_ref())
    }

    pub fn cell(&self, tier: InductionTier) -> u64 {
        self.cells[tier as usize]
    }

    pub fn provider(&self, tier: InductionTier) -> u64 {
        self.providers[tier as usize]
    }
}

/// A cell of a formed matrix: its position, its Joules and its tier's capacity (`MachineEnergyContainer.internal`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub pos: [i32; 3],
    pub stored: u64,
    pub capacity: u64,
}

/// `MatrixEnergyContainer`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MatrixEnergy {
    /// In the order they take and give (Mekanism: its hash map's; here by position).
    pub cells: Vec<Cell>,
    pub providers: usize,
    pub transfer_cap: u64,
    pub storage_cap: u64,
    cached_total: u64,
    pub queued_input: u64,
    pub queued_output: u64,
    pub last_input: u64,
    pub last_output: u64,
}

impl MatrixEnergy {
    /// `addCell` (`MathUtils.addClamped` to `Long.MAX_VALUE`).
    pub fn add_cell(&mut self, cell: Cell) {
        self.storage_cap = self.storage_cap.saturating_add(cell.capacity).min(i64::MAX as u64);
        self.cached_total = self.cached_total.saturating_add(cell.stored).min(i64::MAX as u64);
        self.cells.push(cell);
    }

    /// `addProvider`.
    pub fn add_provider(&mut self, output: u64) {
        self.providers += 1;
        self.transfer_cap = self.transfer_cap.saturating_add(output).min(i64::MAX as u64);
    }

    /// `getEnergy`: the cells' Joules and what is queued.
    pub fn energy(&self) -> u64 {
        (self.cached_total + self.queued_input).saturating_sub(self.queued_output)
    }

    pub fn needed(&self) -> u64 {
        self.storage_cap.saturating_sub(self.energy())
    }

    /// `insert`: what of `amount` is left, at most the transfer cap a tick and what the cells still take.
    pub fn insert(&mut self, amount: u64, simulate: bool) -> u64 {
        if amount == 0 {
            return amount;
        }
        let to_add = amount.min(self.transfer_cap.saturating_sub(self.queued_input)).min(self.needed());
        if to_add == 0 {
            return amount;
        }
        if !simulate {
            self.queued_input += to_add;
        }
        amount - to_add
    }

    /// `extract`: what it gives of `amount`, at most the transfer cap a tick and what it holds.
    pub fn extract(&mut self, amount: u64, simulate: bool) -> u64 {
        if self.energy() == 0 || amount == 0 {
            return 0;
        }
        let amount = amount.min(self.transfer_cap.saturating_sub(self.queued_output)).min(self.energy());
        if amount > 0 && !simulate {
            self.queued_output += amount;
        }
        amount
    }

    /// `tick`: the queued Joules into (or out of) the cells, the first first; the queues become the last tick's
    /// input and output. Whether any cell changed.
    pub fn tick(&mut self) -> bool {
        let changed = self.queued_input != self.queued_output;
        if self.queued_input < self.queued_output {
            self.remove_energy(self.queued_output - self.queued_input);
        } else if self.queued_input > self.queued_output {
            self.add_energy(self.queued_input - self.queued_output);
        }
        self.last_input = self.queued_input;
        self.last_output = self.queued_output;
        self.queued_input = 0;
        self.queued_output = 0;
        changed
    }

    fn add_energy(&mut self, mut energy: u64) {
        self.cached_total = self.cached_total.saturating_add(energy);
        for cell in &mut self.cells {
            let took = energy.min(cell.capacity.saturating_sub(cell.stored));
            cell.stored += took;
            energy -= took;
            if energy == 0 {
                break;
            }
        }
    }

    fn remove_energy(&mut self, mut energy: u64) {
        self.cached_total = self.cached_total.saturating_sub(energy);
        for cell in &mut self.cells {
            let took = energy.min(cell.stored);
            cell.stored -= took;
            energy -= took;
            if energy == 0 {
                break;
            }
        }
    }
}

/// An induction port's mode (`getActive`: true is output).
pub fn port_name(output: bool) -> &'static str {
    if output { "Output" } else { "Input" }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matrix(cells: &[(u64, u64)], providers: &[u64]) -> MatrixEnergy {
        let mut m = MatrixEnergy::default();
        for (i, (stored, capacity)) in cells.iter().enumerate() {
            m.add_cell(Cell { pos: [i as i32, 0, 0], stored: *stored, capacity: *capacity });
        }
        for p in providers {
            m.add_provider(*p);
        }
        m
    }

    /// ATM10 8.4's tiers.toml `[induction]` (Mekanism's defaults).
    #[test]
    fn tiers_set_capacity_and_transfer() {
        let atm10: toml::Table = "[induction]\nbasicCapacity = 8000000000\nadvancedCapacity = 64000000000\neliteCapacity = 512000000000\nultimateCapacity = 4000000000000\nbasicOutput = 256000\nadvancedOutput = 2048000\neliteOutput = 16384000\nultimateOutput = 131072000\n".parse().unwrap();
        let c = InductionConfig::from_tiers(Some(&atm10));
        assert_eq!(c, InductionConfig::default());
        assert_eq!(c.cell(InductionTier::Ultimate), 4_000_000_000_000);
        assert_eq!(c.provider(InductionTier::Elite), 16_384_000);
        let m = matrix(&[(0, c.cell(InductionTier::Basic)), (0, c.cell(InductionTier::Advanced))], &[c.provider(InductionTier::Basic), c.provider(InductionTier::Basic)]);
        assert_eq!((m.storage_cap, m.transfer_cap, m.providers), (72_000_000_000, 512_000, 2));
        assert_eq!(InductionTier::of_cell("mekanism:elite_induction_cell"), Some(InductionTier::Elite));
        assert_eq!(InductionTier::of_provider("mekanism:basic_induction_provider"), Some(InductionTier::Basic));
        assert_eq!(InductionTier::of_cell("mekanism:basic_induction_provider"), None);
    }

    #[test]
    fn input_and_output_each_stop_at_the_transfer_cap_a_tick() {
        let mut m = matrix(&[(0, 8_000_000_000)], &[256_000]);
        assert_eq!(m.insert(1_000_000, false), 744_000, "256 000 in");
        assert_eq!(m.insert(1_000, false), 1_000, "nothing more this tick");
        assert_eq!(m.energy(), 256_000, "queued counts at once");
        assert_eq!(m.extract(300_000, false), 256_000, "out is its own cap, from what is queued in");
        assert!(!m.tick(), "in and out cancel");
        assert_eq!((m.cells[0].stored, m.last_input, m.last_output), (0, 256_000, 256_000));
        m.insert(100_000, false);
        assert!(m.tick());
        assert_eq!((m.cells[0].stored, m.last_input, m.last_output, m.energy()), (100_000, 100_000, 0, 100_000));
    }

    #[test]
    fn the_cells_fill_and_empty_first_first() {
        let mut m = matrix(&[(0, 100), (0, 1_000)], &[10_000]);
        m.insert(500, false);
        m.tick();
        assert_eq!(m.cells.iter().map(|c| c.stored).collect::<Vec<_>>(), [100, 400]);
        m.extract(150, false);
        m.tick();
        assert_eq!(m.cells.iter().map(|c| c.stored).collect::<Vec<_>>(), [0, 350]);
        assert_eq!(m.insert(2_000, true), 1_250, "the cells' room: 1100 - 350");
    }

    #[test]
    fn no_providers_move_nothing_and_no_cells_hold_nothing() {
        let mut m = matrix(&[(500, 1_000)], &[]);
        assert_eq!(m.insert(100, false), 100);
        assert_eq!(m.extract(100, false), 0);
        let mut empty = matrix(&[], &[256_000]);
        assert_eq!(empty.insert(100, false), 100, "no room");
        assert_eq!(empty.extract(100, false), 0);
    }
}
