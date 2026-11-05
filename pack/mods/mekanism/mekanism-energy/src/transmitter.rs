//! The tiers and settings of Mekanism's Mechanical Pipes, Logistical
//! Transporters and Fluid Tanks (`PipeTier`, `TransporterTier`,
//! `FluidTankTier`), with Mekanism 10.7's defaults; `tiers.toml`
//! (`[transmitters.fluid]`, `[transmitters.items]`, `[fluid_tanks]`) changes
//! them, a value out of the mod's range keeping the default, as NeoForge's
//! config spec corrects it. They are apart from `config::EnergyConfig` so
//! the energy settings stay as they are.
use crate::tier::CableTier;
use std::path::Path;

/// `PipeTier`: what a mechanical pipe adds to its network's buffer and what
/// a pull side draws a tick (`getPipeCapacity`, `getPipePullAmount`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PipeTier {
    Basic,
    Advanced,
    Elite,
    Ultimate,
}

impl PipeTier {
    pub const ALL: [Self; 4] = [Self::Basic, Self::Advanced, Self::Elite, Self::Ultimate];

    pub fn name(self) -> &'static str {
        CableTier::ALL[self as usize].name()
    }

    /// The tier of block `mekanism:<tier>_mechanical_pipe`.
    pub fn of_block(id: &str) -> Option<Self> {
        let name = id.strip_prefix("mekanism:")?.strip_suffix("_mechanical_pipe")?;
        Self::ALL.into_iter().find(|t| t.name() == name)
    }

    pub fn default_capacity(self) -> i32 {
        [2_000, 8_000, 32_000, 128_000][self as usize]
    }

    pub fn default_pull(self) -> i32 {
        [250, 1_000, 8_000, 32_000][self as usize]
    }
}

/// `TransporterTier`: how many items a pull side takes at once
/// (`getPullAmount`) and how far a stack moves a tick (`getSpeed`, in
/// hundredths of a block).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TransporterTier {
    Basic,
    Advanced,
    Elite,
    Ultimate,
}

impl TransporterTier {
    pub const ALL: [Self; 4] = [Self::Basic, Self::Advanced, Self::Elite, Self::Ultimate];

    pub fn name(self) -> &'static str {
        CableTier::ALL[self as usize].name()
    }

    /// The tier of block `mekanism:<tier>_logistical_transporter`.
    pub fn of_block(id: &str) -> Option<Self> {
        let name = id.strip_prefix("mekanism:")?.strip_suffix("_logistical_transporter")?;
        Self::ALL.into_iter().find(|t| t.name() == name)
    }

    pub fn default_pull(self) -> u32 {
        [1, 16, 32, 64][self as usize]
    }

    pub fn default_speed(self) -> u32 {
        [5, 10, 20, 50][self as usize]
    }
}

/// `FluidTankTier`: a fluid tank's mB and what it gives down a tick while
/// active.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FluidTankTier {
    Basic,
    Advanced,
    Elite,
    Ultimate,
    Creative,
}

impl FluidTankTier {
    pub const ALL: [Self; 5] = [Self::Basic, Self::Advanced, Self::Elite, Self::Ultimate, Self::Creative];

    pub fn name(self) -> &'static str {
        ["basic", "advanced", "elite", "ultimate", "creative"][self as usize]
    }

    /// The tier of block `mekanism:<tier>_fluid_tank`.
    pub fn of_block(id: &str) -> Option<Self> {
        let name = id.strip_prefix("mekanism:")?.strip_suffix("_fluid_tank")?;
        Self::ALL.into_iter().find(|t| t.name() == name)
    }

    pub fn default_storage(self) -> i32 {
        [32_000, 64_000, 128_000, 256_000, i32::MAX][self as usize]
    }

    pub fn default_output(self) -> i32 {
        [1_000, 4_000, 16_000, 64_000, i32::MAX / 2][self as usize]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransmitterConfig {
    /// `[transmitters.fluid]` `<tier>Capacity` and `<tier>PullAmount`, in `PipeTier::ALL` order.
    pub pipes: [(i32, i32); 4],
    /// `[transmitters.items]` `<tier>PullAmount` and `<tier>Speed`, in `TransporterTier::ALL` order.
    pub transporters: [(u32, u32); 4],
    /// `[fluid_tanks]` `<tier>Capacity` and `<tier>Output`, in `FluidTankTier::ALL` order.
    pub fluid_tanks: [(i32, i32); 5],
}

impl Default for TransmitterConfig {
    fn default() -> Self {
        Self {
            pipes: PipeTier::ALL.map(|t| (t.default_capacity(), t.default_pull())),
            transporters: TransporterTier::ALL.map(|t| (t.default_pull(), t.default_speed())),
            fluid_tanks: FluidTankTier::ALL.map(|t| (t.default_storage(), t.default_output())),
        }
    }
}

impl TransmitterConfig {
    /// Reads `<config>/Mekanism/tiers.toml`.
    pub fn load(config: &Path) -> Self {
        let tiers = std::fs::read_to_string(config.join("Mekanism").join("tiers.toml")).ok().and_then(|t| t.parse::<toml::Table>().ok());
        Self::from_tables(tiers.as_ref())
    }

    pub fn from_tables(tiers: Option<&toml::Table>) -> Self {
        let mut config = Self::default();
        let section = |path: &[&str]| -> Option<toml::Table> {
            let mut at = tiers?;
            for key in path {
                at = at.get(*key)?.as_table()?;
            }
            Some(at.clone())
        };
        // Every key's range is 1..=Integer.MAX_VALUE (`TierConfig`).
        let int = |table: &toml::Table, key: &str, max: i64| table.get(key).and_then(toml::Value::as_integer).filter(|v| (1..=max).contains(v));
        if let Some(fluid) = section(&["transmitters", "fluid"]) {
            for (i, tier) in PipeTier::ALL.into_iter().enumerate() {
                if let Some(v) = int(&fluid, &format!("{}Capacity", tier.name()), i32::MAX as i64) {
                    config.pipes[i].0 = v as i32;
                }
                if let Some(v) = int(&fluid, &format!("{}PullAmount", tier.name()), i32::MAX as i64) {
                    config.pipes[i].1 = v as i32;
                }
            }
        }
        if let Some(items) = section(&["transmitters", "items"]) {
            for (i, tier) in TransporterTier::ALL.into_iter().enumerate() {
                if let Some(v) = int(&items, &format!("{}PullAmount", tier.name()), i32::MAX as i64) {
                    config.transporters[i].0 = v as u32;
                }
                if let Some(v) = int(&items, &format!("{}Speed", tier.name()), i32::MAX as i64) {
                    config.transporters[i].1 = v as u32;
                }
            }
        }
        if let Some(tanks) = section(&["fluid_tanks"]) {
            for (i, tier) in FluidTankTier::ALL.into_iter().enumerate() {
                if let Some(v) = int(&tanks, &format!("{}Capacity", tier.name()), i32::MAX as i64) {
                    config.fluid_tanks[i].0 = v as i32;
                }
                if let Some(v) = int(&tanks, &format!("{}Output", tier.name()), i32::MAX as i64) {
                    config.fluid_tanks[i].1 = v as i32;
                }
            }
        }
        config
    }

    /// A pipe tier's capacity and pull a tick (mB).
    pub fn pipe(&self, tier: PipeTier) -> (i32, i32) {
        self.pipes[tier as usize]
    }

    /// A transporter tier's pull amount (items) and speed.
    pub fn transporter(&self, tier: TransporterTier) -> (u32, u32) {
        self.transporters[tier as usize]
    }

    /// A fluid tank tier's mB and output a tick.
    pub fn fluid_tank(&self, tier: FluidTankTier) -> (i32, i32) {
        self.fluid_tanks[tier as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_come_from_block_ids() {
        assert_eq!(PipeTier::of_block("mekanism:elite_mechanical_pipe"), Some(PipeTier::Elite));
        assert_eq!(PipeTier::of_block("mekanism:creative_mechanical_pipe"), None);
        assert_eq!(TransporterTier::of_block("mekanism:ultimate_logistical_transporter"), Some(TransporterTier::Ultimate));
        assert_eq!(TransporterTier::of_block("mekanism:restrictive_transporter"), None);
        assert_eq!(FluidTankTier::of_block("mekanism:creative_fluid_tank"), Some(FluidTankTier::Creative));
    }

    /// ATM10 8.4's `tiers.toml`, as the NeoForge oracle ran it.
    #[test]
    fn atm10_tiers() {
        let tiers: toml::Table = "[transmitters.fluid]
basicCapacity = 2000
basicPullAmount = 250
advancedCapacity = 8000
advancedPullAmount = 1000
eliteCapacity = 32000
elitePullAmount = 8000
ultimateCapacity = 128000
ultimatePullAmount = 32000
[transmitters.items]
basicPullAmount = 1
basicSpeed = 5
advancedPullAmount = 16
advancedSpeed = 10
elitePullAmount = 32
eliteSpeed = 20
ultimatePullAmount = 0
ultimateSpeed = 50
[fluid_tanks]
basicCapacity = 32000
basicOutput = 1000
creativeCapacity = 2147483647
creativeOutput = 1073741823
eliteOutput = 0
"
        .parse()
        .unwrap();
        let c = TransmitterConfig::from_tables(Some(&tiers));
        assert_eq!(c.pipe(PipeTier::Basic), (2_000, 250));
        assert_eq!(c.pipe(PipeTier::Ultimate), (128_000, 32_000));
        assert_eq!(c.transporter(TransporterTier::Elite), (32, 20));
        assert_eq!(c.transporter(TransporterTier::Ultimate), (64, 50), "a pull of 0 is out of range");
        assert_eq!(c.fluid_tank(FluidTankTier::Creative), (i32::MAX, 1_073_741_823));
        assert_eq!(c.fluid_tank(FluidTankTier::Elite), (128_000, 16_000), "0 is out of range");
        assert_eq!(TransmitterConfig::from_tables(None), TransmitterConfig::default());
    }

    /// Pipe networks are the tubes' with a fluid. The oracle (`mekm2-work/oracle`, x3 P2): two basic pipes pulling
    /// 250 a tick from a creative tank with nothing at their end fill to 4000, 2000 each, and then pull nothing; x3b
    /// P5b: a pipe pulling lava beside a water network stays apart from it.
    #[test]
    fn pipe_networks_hold_one_fluid_up_to_their_pipes_capacity() {
        use crate::network::{Connection, Member, Networks};
        const W: Option<&str> = Some("minecraft:water");
        let (capacity, pull) = TransmitterConfig::default().pipe(PipeTier::Basic);
        let pipe = |share: u64, content: Option<&str>| Member {
            capacity: capacity as u64,
            pull: pull as u64,
            connections: [Connection::Normal; 6],
            share,
            content: content.map(str::to_owned),
        };
        let mut nets = Networks::default();
        nets.add_with([1, 0, 0], pipe(0, None));
        nets.add_with([2, 0, 0], pipe(0, None));
        let mut pulled = Vec::new();
        for _ in 0..18 {
            let room = nets.available_pull([1, 0, 0]);
            pulled.push(room);
            assert_eq!(nets.insert_of([1, 0, 0], W, room, false), 0);
        }
        assert_eq!(&pulled[..2], &[250, 250]);
        assert_eq!(pulled[16..], [0, 0], "full after 16 ticks");
        let shares: Vec<u64> = [[1, 0, 0], [2, 0, 0]].iter().map(|p| nets.save_share(*p).unwrap()).collect();
        assert_eq!(shares, [2_000, 2_000]);
        // Lava beside the water network: its own network, apart.
        nets.add_with([4, 0, 0], pipe(250, Some("minecraft:lava")));
        nets.add_with([3, 0, 0], pipe(0, None));
        assert_eq!(nets.networks().count(), 2);
        assert_eq!(nets.network_of([3, 0, 0]).unwrap().0, nets.network_of([2, 0, 0]).unwrap().0, "the new pipe joins the bigger");
        assert_eq!(nets.insert_of([3, 0, 0], Some("minecraft:lava"), 10, false), 10, "no lava into water");
    }
}
