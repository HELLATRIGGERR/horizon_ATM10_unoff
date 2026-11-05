//! Mekanism's Combiner and Precision Sawmill and its factories: blocks that
//! run Mekanism's recipes in processes of their own, each with its cached
//! recipe and progress (`CachedRecipe`, as `crate::machine::process` runs
//! it) on one energy store. The Combiner (`TileEntityCombiner`) takes a main
//! and an extra item; the Precision Sawmill (`TileEntityPrecisionSawmill`) gives
//! a main output and a secondary one by chance (`SawmillRecipe.ChanceOutput`).
//! A factory (`TileEntityFactory`, `FactoryTier`) runs 3, 5, 7 or 9 processes
//! of its machine's recipes side by side: smelting, enriching, crushing,
//! combining (one extra slot for all) and sawing, and compressing, purifying,
//! injecting and infusing (`TileEntityItemStackChemicalToItemStackFactory`:
//! one chemical tank and its slot for all, the chemical each process takes a
//! tick by `crate::advanced`'s rules, its use so far by process). It stores
//! `max(storage / 2, usage) * processes` Joules (`Factory.setMachineData`),
//! each process takes the machine's Joules a tick, and it may sort its inputs
//! evenly over its processes (`sortInventory`). A Tier Installer
//! (`ItemTierInstaller`) turns a machine into a basic factory and a factory
//! into the next tier, keeping what it holds (`MachineUpgradeData`).
use crate::machine::MachineKind;

/// `FactoryTier`: how many processes a factory runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FactoryTier {
    Basic,
    Advanced,
    Elite,
    Ultimate,
}

impl FactoryTier {
    pub const ALL: [Self; 4] = [Self::Basic, Self::Advanced, Self::Elite, Self::Ultimate];

    pub fn processes(self) -> usize {
        match self {
            Self::Basic => 3,
            Self::Advanced => 5,
            Self::Elite => 7,
            Self::Ultimate => 9,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Advanced => "advanced",
            Self::Elite => "elite",
            Self::Ultimate => "ultimate",
        }
    }

    pub fn of_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.name() == name)
    }

    /// `AttributeUpgradeable` of a factory: the next tier, none for the ultimate.
    pub fn next(self) -> Option<Self> {
        Self::ALL.get(self as usize + 1).copied()
    }
}

/// `FactoryType` of the factories this port has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FactoryType {
    Smelting,
    Enriching,
    Crushing,
    Combining,
    Sawing,
    Compressing,
    Purifying,
    Injecting,
    Infusing,
}

impl FactoryType {
    pub const ALL: [Self; 9] = [
        Self::Smelting,
        Self::Enriching,
        Self::Crushing,
        Self::Combining,
        Self::Sawing,
        Self::Compressing,
        Self::Purifying,
        Self::Injecting,
        Self::Infusing,
    ];

    /// `getRegistryNameComponent`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Smelting => "smelting",
            Self::Enriching => "enriching",
            Self::Crushing => "crushing",
            Self::Combining => "combining",
            Self::Sawing => "sawing",
            Self::Compressing => "compressing",
            Self::Purifying => "purifying",
            Self::Injecting => "injecting",
            Self::Infusing => "infusing",
        }
    }

    /// `getBaseMachine`: whose settings and recipes it has.
    pub fn base(self) -> MachineKind {
        match self {
            Self::Smelting => MachineKind::EnergizedSmelter,
            Self::Enriching => MachineKind::EnrichmentChamber,
            Self::Crushing => MachineKind::Crusher,
            Self::Combining => MachineKind::Combiner,
            Self::Sawing => MachineKind::PrecisionSawmill,
            Self::Compressing => MachineKind::OsmiumCompressor,
            Self::Purifying => MachineKind::PurificationChamber,
            Self::Injecting => MachineKind::ChemicalInjectionChamber,
            Self::Infusing => MachineKind::MetallurgicInfuser,
        }
    }

    pub fn of_base(kind: MachineKind) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.base() == kind)
    }

    /// `TileEntityItemStackChemicalToItemStackFactory`'s types: an item and a
    /// chemical in, one chemical tank for all its processes.
    pub fn uses_chemical(self) -> bool {
        matches!(self, Self::Compressing | Self::Purifying | Self::Injecting | Self::Infusing)
    }

    /// `allowExtractingChemical`: the compressing and infusing factories'
    /// tanks give on their output sides (`setupIOConfig(CHEMICAL, tank,
    /// RIGHT)`, `createModern`); the purifying and injecting ones' only to the
    /// factory (`setupInputConfig`, `inputModern`: `notExternal`).
    pub fn gives_chemical(self) -> bool {
        matches!(self, Self::Compressing | Self::Infusing)
    }

    /// `getInitialChemicalTanks`: what the tank holds a process (the
    /// infusing factory's 1000 mB, the others' 210, as their machines').
    pub fn tank_per_process(self) -> Option<i64> {
        match self {
            Self::Infusing => Some(1000),
            t if t.uses_chemical() => Some(crate::advanced::MAX_GAS),
            _ => None,
        }
    }
}

/// A factory: `mekanism:<tier>_<type>_factory`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FactoryKind {
    pub tier: FactoryTier,
    pub kind: FactoryType,
}

impl FactoryKind {
    pub fn id(self) -> String {
        format!("mekanism:{}_{}_factory", self.tier.name(), self.kind.name())
    }

    pub fn of_block(id: &str) -> Option<Self> {
        let rest = id.strip_prefix("mekanism:")?.strip_suffix("_factory")?;
        let (tier, kind) = rest.split_once('_')?;
        Some(Self { tier: FactoryTier::of_name(tier)?, kind: FactoryType::ALL.into_iter().find(|t| t.name() == kind)? })
    }

    pub fn all() -> impl Iterator<Item = Self> {
        FactoryTier::ALL.into_iter().flat_map(|tier| FactoryType::ALL.into_iter().map(move |kind| Self { tier, kind }))
    }

    /// Its chemical tank's capacity: `tank_per_process` times its processes.
    pub fn chemical_capacity(self) -> Option<i64> {
        self.kind.tank_per_process().map(|each| each * self.tier.processes() as i64)
    }
}

/// The blocks `crate::factory` runs: the Combiner and the Precision Sawmill,
/// and the factories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProcessKind {
    Machine(MachineKind),
    Factory(FactoryKind),
}

/// Where a block of processes keeps its items: Mekanism's own slot order
/// (each process's input, output and secondary output in turn, then the
/// shared extra slot, then the energy slot; the combiner's main input, extra
/// input, output, energy), then the upgrade component's input and output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    pub inputs: Vec<usize>,
    pub outputs: Vec<usize>,
    /// The sawing processes' secondary outputs, by process; empty otherwise.
    pub secondaries: Vec<usize>,
    pub extra: Option<usize>,
    pub energy: usize,
    pub upgrade: usize,
    pub upgrade_out: usize,
    pub count: usize,
}

impl Layout {
    /// The layout of any machine of `crate::machine` (one process): the
    /// combiner's extra input, the infuser's infusion slot and an advanced
    /// machine's chemical slot are its extra slot.
    pub fn of_machine(kind: MachineKind) -> Self {
        let s = kind.slots();
        Self {
            inputs: vec![s.input],
            outputs: vec![s.output],
            secondaries: s.output2.filter(|_| kind == MachineKind::PrecisionSawmill).into_iter().collect(),
            extra: s.infusion.filter(|_| kind == MachineKind::Combiner || kind.infuses() || kind.advanced()),
            energy: s.energy,
            upgrade: s.upgrade,
            upgrade_out: s.upgrade_out,
            count: s.count,
        }
    }

    /// `TileEntityItemToItemFactory.addSlots` (input, output by process),
    /// `TileEntitySawingFactory`'s (input, output, secondary), the combining
    /// factory's extra slot after them (and the chemical factories' chemical
    /// slot, `ChemicalInventorySlot.fillOrConvert`), the energy slot last.
    pub fn of_factory(factory: FactoryKind) -> Self {
        let n = factory.tier.processes();
        let per = if factory.kind == FactoryType::Sawing { 3 } else { 2 };
        let mut next = per * n;
        let extra = (factory.kind == FactoryType::Combining || factory.kind.uses_chemical()).then(|| {
            next += 1;
            next - 1
        });
        Self {
            inputs: (0..n).map(|i| per * i).collect(),
            outputs: (0..n).map(|i| per * i + 1).collect(),
            secondaries: if per == 3 { (0..n).map(|i| per * i + 2).collect() } else { Vec::new() },
            extra,
            energy: next,
            upgrade: next + 1,
            upgrade_out: next + 2,
            count: next + 3,
        }
    }

    /// The slots that take what goes in (inputs, extra, energy), for hoppers.
    pub fn takers(&self) -> Vec<usize> {
        let mut out = self.inputs.clone();
        out.extend(self.extra);
        out.push(self.energy);
        out
    }

    /// The output slots, each process's main then secondary.
    pub fn givers(&self) -> Vec<usize> {
        let mut out = Vec::new();
        for (i, o) in self.outputs.iter().enumerate() {
            out.push(*o);
            out.extend(self.secondaries.get(i));
        }
        out
    }

    /// `parseUpgradeData`: which of the old block's slots go where in the
    /// new one (`from`, `to`) when a tier installer replaces it: each
    /// process's input and outputs to the same process, the extra and energy
    /// slots, and the upgrade component's two.
    pub fn transfer(&self, to: &Layout) -> Vec<(usize, usize)> {
        let mut pairs: Vec<(usize, usize)> = Vec::new();
        for (list, other) in [(&self.inputs, &to.inputs), (&self.outputs, &to.outputs), (&self.secondaries, &to.secondaries)] {
            pairs.extend(list.iter().zip(other.iter()).map(|(a, b)| (*a, *b)));
        }
        if let (Some(a), Some(b)) = (self.extra, to.extra) {
            pairs.push((a, b));
        }
        pairs.extend([(self.energy, to.energy), (self.upgrade, to.upgrade), (self.upgrade_out, to.upgrade_out)]);
        pairs
    }

    /// The vanilla menu that shows this many slots (the screen leaves the rest out).
    pub fn menu(&self) -> (&'static str, usize) {
        match self.count {
            0..=9 => ("minecraft:generic_3x3", 9),
            10..=18 => ("minecraft:generic_9x2", 18),
            19..=27 => ("minecraft:generic_9x3", 27),
            _ => ("minecraft:generic_9x4", 36),
        }
    }
}

impl ProcessKind {
    /// The Combiner, the Precision Sawmill, or a factory.
    pub fn of_block(id: &str) -> Option<Self> {
        match MachineKind::of_block(id) {
            Some(kind) if kind.processes_like_a_factory() => Some(Self::Machine(kind)),
            Some(_) => None,
            None => FactoryKind::of_block(id).map(Self::Factory),
        }
    }

    pub fn id(self) -> String {
        match self {
            Self::Machine(kind) => kind.id().to_owned(),
            Self::Factory(factory) => factory.id(),
        }
    }

    /// The machine whose settings, recipes and side configuration it has.
    pub fn base(self) -> MachineKind {
        match self {
            Self::Machine(kind) => kind,
            Self::Factory(factory) => factory.kind.base(),
        }
    }

    pub fn tier(self) -> Option<FactoryTier> {
        match self {
            Self::Machine(_) => None,
            Self::Factory(factory) => Some(factory.tier),
        }
    }

    pub fn processes(self) -> usize {
        self.tier().map_or(1, FactoryTier::processes)
    }

    pub fn is_factory(self) -> bool {
        matches!(self, Self::Factory(_))
    }

    pub fn saws(self) -> bool {
        self.base() == MachineKind::PrecisionSawmill
    }

    /// A factory of an item and a chemical: its chemical tank's capacity.
    pub fn chemical_capacity(self) -> Option<i64> {
        match self {
            Self::Machine(_) => None,
            Self::Factory(factory) => factory.chemical_capacity(),
        }
    }

    /// The upgrades it takes: its machine's (`setFrom(origMachine, AttributeUpgradeSupport)`).
    pub fn upgrades(self) -> &'static [crate::machine::Upgrade] {
        self.base().upgrades()
    }

    pub fn layout(self) -> Layout {
        match self {
            Self::Machine(kind) => Layout::of_machine(kind),
            Self::Factory(factory) => Layout::of_factory(factory),
        }
    }

    /// Its Joules held before energy upgrades: the machine's `storage`, a
    /// factory's `max(storage * 0.5, usage) * processes` (`Factory.setMachineData`).
    pub fn base_storage(self, usage: u64, storage: u64) -> u64 {
        match self {
            Self::Machine(_) => storage,
            Self::Factory(factory) => {
                let each = (storage as f64 * 0.5).max(usage as f64);
                crate::clamp_to_long(each * factory.tier.processes() as f64).max(0) as u64
            }
        }
    }
}

/// `ItemTierInstaller` (`mekanism:<tier>_tier_installer`): the tier it takes
/// a block from (none: a machine without one) and the tier it gives.
pub fn installer(item: &str) -> Option<(Option<FactoryTier>, FactoryTier)> {
    let tier = FactoryTier::of_name(item.strip_prefix("mekanism:")?.strip_suffix("_tier_installer")?)?;
    let from = match tier {
        FactoryTier::Basic => None,
        t => Some(FactoryTier::ALL[t as usize - 1]),
    };
    Some((from, tier))
}

/// What a tier installer of `item` makes of the block `id` (`AttributeUpgradeable.upgradeResult`):
/// a factory machine becomes the basic factory of its type, a factory the next
/// tier; the installer's from tier must be the block's (`baseTier == fromTier`).
pub fn upgraded(id: &str, item: &str) -> Option<String> {
    let (from, to) = installer(item)?;
    if let Some(factory) = FactoryKind::of_block(id) {
        return (from == Some(factory.tier) && factory.tier.next() == Some(to)).then(|| FactoryKind { tier: to, ..factory }.id());
    }
    let kind = FactoryType::of_base(MachineKind::of_block(id)?)?;
    (from.is_none() && to == FactoryTier::Basic).then(|| FactoryKind { tier: to, kind }.id())
}

/// The layout of any block an installer reads: a machine of `crate::machine`
/// (the chamber, crusher and smelter, the advanced machines, the infuser) or
/// one of these.
pub fn layout_of(id: &str) -> Option<Layout> {
    match ProcessKind::of_block(id) {
        Some(kind) => Some(kind.layout()),
        None => MachineKind::of_block(id).filter(|k| FactoryType::of_base(*k).is_some()).map(Layout::of_machine),
    }
}

/// A process's chance output (`BasicSawmillRecipe.BasicChanceOutput`): with
/// `roll` from `[0, 1)`, the secondary output comes when `roll <= chance`
/// (a chance of 0 never rolls).
pub fn secondary_comes(chance: f64, roll: f64) -> bool {
    chance > 0.0 && roll <= chance
}

/// One input slot of a factory for `sort_inputs`: the item's key, how many,
/// and its stack size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortSlot<K> {
    pub key: K,
    pub count: u32,
    pub max_stack: u32,
}

/// `TileEntityFactory.sortInventory`: the inputs (by process) spread evenly
/// over the processes. Processes that hold the same item (`key`) share their
/// total; an empty process joins the first kinds that can fill more of them
/// (`addEmptySlotsAsTargets`, as far as `takes(process, key)`: its output
/// takes that recipe's), then each kind is split evenly in multiples of what
/// an operation needs (`min_per_slot(key, total, first process)`, at least 1),
/// the remainder to the first ones (`distributeItems`). The kinds go in the
/// order they are first met here; Mekanism goes in its hash map's order,
/// which only decides which kind gets the empty processes first. Returns each
/// process's new count (none: empty) of the kind it now holds.
pub fn sort_inputs<K: Clone + PartialEq>(
    slots: &[Option<SortSlot<K>>],
    mut min_per_slot: impl FnMut(&K, u32, usize) -> u32,
    mut takes: impl FnMut(usize, &K) -> bool,
) -> Vec<Option<(K, u32)>> {
    struct Group<K> {
        key: K,
        max_stack: u32,
        processes: Vec<usize>,
        total: u32,
        min: Option<u32>,
    }
    let mut groups: Vec<Group<K>> = Vec::new();
    let mut empty: Vec<usize> = Vec::new();
    for (process, slot) in slots.iter().enumerate() {
        match slot {
            None => empty.push(process),
            Some(s) => match groups.iter_mut().find(|g| g.key == s.key) {
                Some(g) => {
                    g.processes.push(process);
                    g.total += s.count;
                }
                None => groups.push(Group { key: s.key.clone(), max_stack: s.max_stack.max(1), processes: vec![process], total: s.count, min: None }),
            },
        }
    }
    let mut out: Vec<Option<(K, u32)>> = slots.iter().map(|s| s.as_ref().map(|s| (s.key.clone(), s.count))).collect();
    if groups.is_empty() {
        return out;
    }
    let mut min_of = |g: &mut Group<K>| -> u32 {
        if g.min.is_none() {
            let larger = g.total.min(g.max_stack);
            g.min = Some(min_per_slot(&g.key, larger, g.processes[0]).max(1));
        }
        g.min.unwrap_or(1)
    };
    if !empty.is_empty() {
        for g in groups.iter_mut() {
            let min = min_of(g);
            let max_slots = (g.total / min) as usize;
            if max_slots <= 1 || max_slots <= g.processes.len() {
                continue;
            }
            let to_add = max_slots - g.processes.len();
            let mut added = Vec::new();
            for &process in &empty {
                if !takes(process, &g.key) {
                    continue;
                }
                added.push(process);
                if added.len() >= to_add {
                    break;
                }
            }
            g.processes.extend(&added);
            empty.retain(|p| !added.contains(p));
            if empty.is_empty() {
                break;
            }
        }
    }
    for g in groups.iter_mut() {
        let count = g.processes.len() as u32;
        if count == 1 {
            continue;
        }
        let mut per_slot = g.total / count;
        if per_slot == g.max_stack {
            continue;
        }
        let mut remainder = g.total % count;
        let mut min = min_of(g);
        if min > 1 {
            let rest = per_slot % min;
            if rest > 0 {
                per_slot -= rest;
                remainder += rest * count;
            }
            if per_slot + min > g.max_stack {
                min = g.max_stack.saturating_sub(per_slot);
            }
        }
        for &process in &g.processes {
            let mut size = per_slot;
            if remainder > 0 {
                if remainder > min {
                    size += min;
                    remainder -= min;
                } else {
                    size += remainder;
                    remainder = 0;
                }
            }
            out[process] = (size > 0).then(|| (g.key.clone(), size));
        }
    }
    out
}

/// The button of the sorting tab (`GuiSortingTab`, `PacketGuiInteract.AUTO_SORT_BUTTON`).
pub const SORT_BUTTON: i32 = 150;

/// A factory's own data slots after a machine's (`menu::MACHINE_SLOTS`): 0
/// how many processes, 1 sorting, 2..6 the Joules used last tick
/// (`lastUsage`), then each process's progress.
pub const FACTORY_SLOTS: usize = 6 + 9;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FactoryView {
    pub processes: u8,
    pub sorting: bool,
    pub last_usage: u64,
    pub progress: Vec<u32>,
}

impl FactoryView {
    pub fn pack(&self) -> Vec<i32> {
        let mut out = Vec::with_capacity(FACTORY_SLOTS);
        out.push(i32::from(self.processes));
        out.push(i32::from(self.sorting));
        out.extend((0..4).map(|i| ((self.last_usage >> (16 * i)) & 0xFFFF) as i32));
        out.extend((0..9).map(|i| self.progress.get(i).copied().unwrap_or(0).min(0x7FFF) as i32));
        out
    }

    /// The view at `at` in `data`; none until the server's values arrived.
    pub fn unpack(data: &[i32], at: usize) -> Option<Self> {
        let data = data.get(at..at + FACTORY_SLOTS)?;
        let small = |i: usize| data[i] as u16 as i16;
        let processes = small(0).clamp(0, 9) as u8;
        Some(Self {
            processes,
            sorting: data[1] != 0,
            last_usage: (0..4).fold(0u64, |v, i| v | (u64::from(data[2 + i] as u16) << (16 * i))),
            progress: (0..usize::from(processes)).map(|i| small(6 + i).max(0) as u32).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sides::DataType;

    #[test]
    fn factories_are_named_and_laid_out_as_mekanisms() {
        let basic = FactoryKind { tier: FactoryTier::Basic, kind: FactoryType::Smelting };
        assert_eq!(basic.id(), "mekanism:basic_smelting_factory");
        assert_eq!(FactoryKind::of_block("mekanism:ultimate_sawing_factory"), Some(FactoryKind { tier: FactoryTier::Ultimate, kind: FactoryType::Sawing }));
        assert_eq!(FactoryKind::of_block("mekanism:basic_infusing_factory"), Some(FactoryKind { tier: FactoryTier::Basic, kind: FactoryType::Infusing }));
        assert_eq!(FactoryKind::of_block("mekanism:basic_dissolving_factory"), None, "no such factory");
        assert_eq!(FactoryKind::all().count(), 36);
        // Measured on NeoForge: process i of an item-to-item factory at 2i, 2i + 1; the ultimate's energy slot 18.
        let ultimate = Layout::of_factory(FactoryKind { tier: FactoryTier::Ultimate, kind: FactoryType::Smelting });
        assert_eq!((ultimate.inputs[8], ultimate.outputs[8], ultimate.energy, ultimate.count), (16, 17, 18, 21));
        assert_eq!(ultimate.menu(), ("minecraft:generic_9x3", 27));
        let combining = Layout::of_factory(FactoryKind { tier: FactoryTier::Basic, kind: FactoryType::Combining });
        assert_eq!((combining.extra, combining.energy), (Some(6), 7));
        let sawing = Layout::of_factory(FactoryKind { tier: FactoryTier::Basic, kind: FactoryType::Sawing });
        assert_eq!((sawing.inputs.clone(), sawing.outputs.clone(), sawing.secondaries.clone(), sawing.energy), (vec![0, 3, 6], vec![1, 4, 7], vec![2, 5, 8], 9));
        assert_eq!(sawing.givers(), [1, 2, 4, 5, 7, 8]);
        let combiner = ProcessKind::of_block("mekanism:combiner").map(ProcessKind::layout);
        assert_eq!(combiner.map(|l| (l.inputs, l.extra, l.outputs, l.energy)), Some((vec![0], Some(1), vec![2], 3)));
        let sawmill = ProcessKind::Machine(MachineKind::PrecisionSawmill).layout();
        assert_eq!((sawmill.outputs.clone(), sawmill.secondaries.clone(), sawmill.energy), (vec![1], vec![2], 3));
        assert_eq!(ProcessKind::of_block("mekanism:enrichment_chamber"), None, "the machine module's");
    }

    #[test]
    fn a_factory_stores_half_its_machines_store_a_process() {
        // Measured on NeoForge with ATM10's configs: 30 000, 50 000, 70 000, 90 000 J; the combining factory 60 000.
        let of = |tier, kind, usage, storage| ProcessKind::Factory(FactoryKind { tier, kind }).base_storage(usage, storage);
        assert_eq!(of(FactoryTier::Basic, FactoryType::Smelting, 50, 20_000), 30_000);
        assert_eq!(of(FactoryTier::Advanced, FactoryType::Smelting, 50, 20_000), 50_000);
        assert_eq!(of(FactoryTier::Elite, FactoryType::Enriching, 50, 20_000), 70_000);
        assert_eq!(of(FactoryTier::Ultimate, FactoryType::Smelting, 50, 20_000), 90_000);
        assert_eq!(of(FactoryTier::Basic, FactoryType::Combining, 50, 40_000), 60_000);
        assert_eq!(of(FactoryTier::Basic, FactoryType::Smelting, 50_000, 20_000), 150_000, "at least a tick of each process");
        assert_eq!(ProcessKind::Machine(MachineKind::Combiner).base_storage(50, 40_000), 40_000);
        assert_eq!(MachineKind::Combiner.default_storage(), 40_000);
        assert_eq!(MachineKind::Combiner.default_items(), [DataType::Input, DataType::Input, DataType::Output, DataType::Energy, DataType::Input, DataType::Extra]);
        assert!(MachineKind::Combiner.item_supports(DataType::Extra) && !MachineKind::PrecisionSawmill.item_supports(DataType::Extra));
    }

    #[test]
    fn tier_installers_go_one_tier_up() {
        assert_eq!(installer("mekanism:basic_tier_installer"), Some((None, FactoryTier::Basic)));
        assert_eq!(installer("mekanism:elite_tier_installer"), Some((Some(FactoryTier::Advanced), FactoryTier::Elite)));
        assert_eq!(upgraded("mekanism:enrichment_chamber", "mekanism:basic_tier_installer").as_deref(), Some("mekanism:basic_enriching_factory"));
        assert_eq!(upgraded("mekanism:precision_sawmill", "mekanism:basic_tier_installer").as_deref(), Some("mekanism:basic_sawing_factory"));
        assert_eq!(upgraded("mekanism:enrichment_chamber", "mekanism:advanced_tier_installer"), None);
        assert_eq!(upgraded("mekanism:basic_combining_factory", "mekanism:advanced_tier_installer").as_deref(), Some("mekanism:advanced_combining_factory"));
        assert_eq!(upgraded("mekanism:basic_combining_factory", "mekanism:elite_tier_installer"), None);
        assert_eq!(upgraded("mekanism:ultimate_smelting_factory", "mekanism:ultimate_tier_installer"), None);
        assert_eq!(upgraded("mekanism:metallurgic_infuser", "mekanism:basic_tier_installer").as_deref(), Some("mekanism:basic_infusing_factory"));
        assert_eq!(upgraded("mekanism:osmium_compressor", "mekanism:basic_tier_installer").as_deref(), Some("mekanism:basic_compressing_factory"));
        assert_eq!(upgraded("mekanism:purification_chamber", "mekanism:basic_tier_installer").as_deref(), Some("mekanism:basic_purifying_factory"));
        assert_eq!(upgraded("mekanism:chemical_injection_chamber", "mekanism:basic_tier_installer").as_deref(), Some("mekanism:basic_injecting_factory"));
        assert_eq!(upgraded("mekanism:elite_injecting_factory", "mekanism:ultimate_tier_installer").as_deref(), Some("mekanism:ultimate_injecting_factory"));
        assert_eq!(upgraded("mekanism:chemical_dissolution_chamber", "mekanism:basic_tier_installer"), None, "Mekanism has no such factory");
        assert_eq!(upgraded("mekanism:electrolytic_separator", "mekanism:basic_tier_installer"), None);
        // A chamber's input, output, energy and upgrade slots to process 0 of the factory.
        let chamber = layout_of("mekanism:enrichment_chamber").unwrap();
        let factory = layout_of("mekanism:basic_enriching_factory").unwrap();
        assert_eq!(chamber.transfer(&factory), [(0, 0), (1, 1), (2, 6), (3, 7), (4, 8)]);
        let sawmill = layout_of("mekanism:precision_sawmill").unwrap();
        let sawing = layout_of("mekanism:basic_sawing_factory").unwrap();
        assert_eq!(sawmill.transfer(&sawing), [(0, 0), (1, 1), (2, 2), (3, 9), (4, 10), (5, 11)]);
        let combiner = layout_of("mekanism:combiner").unwrap();
        let combining = layout_of("mekanism:basic_combining_factory").unwrap();
        assert_eq!(combiner.transfer(&combining), [(0, 0), (2, 1), (1, 6), (3, 7), (4, 8), (5, 9)]);
        let advanced = layout_of("mekanism:advanced_combining_factory").unwrap();
        assert_eq!(combining.transfer(&advanced)[..3], [(0, 0), (2, 2), (4, 4)]);
        assert_eq!(combining.transfer(&advanced)[6], (6, 10), "the extra slot");
        // An advanced machine's chemical slot and the infuser's infusion slot are the factory's chemical slot.
        let compressor = layout_of("mekanism:osmium_compressor").unwrap();
        let compressing = layout_of("mekanism:basic_compressing_factory").unwrap();
        assert_eq!(compressor.transfer(&compressing), [(0, 0), (2, 1), (1, 6), (3, 7), (4, 8), (5, 9)]);
        let infuser = layout_of("mekanism:metallurgic_infuser").unwrap();
        let infusing = layout_of("mekanism:basic_infusing_factory").unwrap();
        assert_eq!(infuser.transfer(&infusing), [(1, 0), (2, 1), (0, 6), (3, 7), (4, 8), (5, 9)]);
    }

    #[test]
    fn the_chemical_factories_share_one_tank() {
        let of = |tier, kind| FactoryKind { tier, kind };
        // `getInitialChemicalTanks`: 210 mB a process, the infusing factory's 1000.
        assert_eq!(of(FactoryTier::Basic, FactoryType::Compressing).chemical_capacity(), Some(630));
        assert_eq!(of(FactoryTier::Ultimate, FactoryType::Purifying).chemical_capacity(), Some(1890));
        assert_eq!(of(FactoryTier::Advanced, FactoryType::Injecting).chemical_capacity(), Some(1050));
        assert_eq!(of(FactoryTier::Basic, FactoryType::Infusing).chemical_capacity(), Some(3000));
        assert_eq!(of(FactoryTier::Ultimate, FactoryType::Infusing).chemical_capacity(), Some(9000));
        assert_eq!(of(FactoryTier::Basic, FactoryType::Combining).chemical_capacity(), None);
        // Its chemical slot after the processes, as the combining factory's extra slot.
        let basic = Layout::of_factory(of(FactoryTier::Basic, FactoryType::Infusing));
        assert_eq!((basic.inputs.clone(), basic.outputs.clone(), basic.extra, basic.energy, basic.count), (vec![0, 2, 4], vec![1, 3, 5], Some(6), 7, 10));
        let ultimate = Layout::of_factory(of(FactoryTier::Ultimate, FactoryType::Compressing));
        assert_eq!((ultimate.extra, ultimate.energy, ultimate.upgrade, ultimate.menu()), (Some(18), 19, 20, ("minecraft:generic_9x3", 27)));
        assert!(FactoryType::Compressing.gives_chemical() && FactoryType::Infusing.gives_chemical());
        assert!(!FactoryType::Purifying.gives_chemical() && !FactoryType::Injecting.gives_chemical());
        // The chambers' factories take chemical upgrades, as their machines.
        let purifying = ProcessKind::Factory(of(FactoryTier::Basic, FactoryType::Purifying));
        assert!(purifying.upgrades().contains(&crate::machine::Upgrade::Chemical));
        assert!(!ProcessKind::Factory(of(FactoryTier::Basic, FactoryType::Compressing)).upgrades().contains(&crate::machine::Upgrade::Chemical));
    }

    fn slot(key: &'static str, count: u32) -> Option<SortSlot<&'static str>> {
        Some(SortSlot { key, count, max_stack: 64 })
    }

    #[test]
    fn sorting_spreads_inputs_as_on_neoforge() {
        // Ten raw iron in process 0 of a basic factory: 4, 3, 3.
        let sorted = sort_inputs(&[slot("iron", 10), None, None], |_, _, _| 1, |_, _| true);
        assert_eq!(sorted, [Some(("iron", 4)), Some(("iron", 3)), Some(("iron", 3))]);
        // Five and two iron and two gold over an advanced factory, gold first in Java's map this time:
        // 3, 1, 2, 1, 2 there; here iron comes first and takes both empty processes.
        let sorted = sort_inputs(&[slot("iron", 5), slot("gold", 2), slot("iron", 2), None, None], |_, _, _| 1, |_, _| true);
        assert_eq!(sorted, [Some(("iron", 2)), Some(("gold", 2)), Some(("iron", 2)), Some(("iron", 2)), Some(("iron", 1))]);
        let sorted = sort_inputs(&[slot("gold", 2), slot("iron", 5), None, slot("iron", 2), None], |_, _, _| 1, |_, _| true);
        assert_eq!(sorted, [Some(("gold", 1)), Some(("iron", 3)), Some(("gold", 1)), Some(("iron", 2)), Some(("iron", 2))]);
        // Four obsidian dust an operation: ten over a basic combining factory, 6 and 4.
        let sorted = sort_inputs(&[slot("dust", 10), None, None], |_, _, _| 4, |_, _| true);
        assert_eq!(sorted, [Some(("dust", 6)), Some(("dust", 4)), None]);
        // A process whose output can't take the recipe's isn't filled; one item stays put.
        let sorted = sort_inputs(&[slot("iron", 10), None, None], |_, _, _| 1, |p, _| p != 1);
        assert_eq!(sorted, [Some(("iron", 5)), None, Some(("iron", 5))]);
        assert_eq!(sort_inputs(&[slot("iron", 1), None, None], |_, _, _| 1, |_, _| true), [Some(("iron", 1)), None, None]);
        let full = [slot("iron", 64), slot("iron", 64), slot("iron", 64)];
        assert_eq!(sort_inputs(&full, |_, _, _| 1, |_, _| true), full.map(|s| s.map(|s| (s.key, s.count))));
    }

    #[test]
    fn the_view_round_trips_and_chance_rolls_as_java() {
        let view = FactoryView { processes: 3, sorting: true, last_usage: 150, progress: vec![1, 199, 0] };
        let mut data = vec![0; 55];
        data.extend(view.pack());
        assert_eq!(FactoryView::unpack(&data, 55), Some(view));
        assert_eq!(FactoryView::unpack(&data, 56), None);
        assert!(secondary_comes(0.25, 0.25) && !secondary_comes(0.25, 0.26) && !secondary_comes(0.0, 0.0) && secondary_comes(1.0, 0.99));
    }
}
