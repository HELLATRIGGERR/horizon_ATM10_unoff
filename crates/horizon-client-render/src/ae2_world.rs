//! Experimental persistent AE2 machine state. Simulation belongs to the game.
use crate::items::ItemStack;
use horizon_world::nbt::Compound;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MachineKind {
    Drive,
    Chest,
    VibrationChamber,
    Interface,
    PatternProvider,
    Inscriber,
    Charger,
    MolecularAssembler,
    Controller,
    EnergyAcceptor,
    EnergyCell,
    DenseEnergyCell,
    CableBus,
    WirelessAccessPoint,
    Terminal,
    CraftingTerminal,
    PatternEncodingTerminal,
    CraftingUnit,
    CraftingAccelerator,
    CraftingStorage1k,
    CraftingStorage4k,
    CraftingStorage16k,
    CraftingStorage64k,
    CraftingStorage256k,
    CraftingMonitor,
    IoPort,
    CellWorkbench,
    Condenser,
    SpatialIoPort,
    SpatialPylon,
    SpatialAnchor,
    QuantumRing,
    QuantumLink,
    GrowthAccelerator,
    CrystalResonanceGenerator,
    CreativeEnergyCell,
    DebugEnergyGenerator,
    DebugItemGenerator,
    DebugCubeGenerator,
    DebugPhantomNode,
    SkyStoneTank,
    SkyStoneChest,
    SmoothSkyStoneChest,
    LightDetector,
    Paint,
    Crank,
    MysteriousCube,
    /// A machine of an AE2 addon: an index into the table the addon rows
    /// install ([`install_ext_machines`]). Addon features stay out of this
    /// crate; saves keep the block's id, so nothing on disk changes.
    Ext(u16),
}
/// An addon machine's row: its block id and the inventories `Machine::new`
/// gives it, and what of its saved state is physical (dropped when broken).
#[derive(Clone, Copy, Debug)]
pub struct ExtMachine {
    pub id: &'static str,
    pub physical_slots: usize,
    pub upgrade_slots: usize,
    /// Owned keys kept in the machine's `config` that a break drops: one
    /// item of each and its amount (AE2's `AEKey.addDrops` splits them).
    pub key_drops: Option<fn(&Machine) -> Vec<(ItemStack, u64)>>,
    /// `AEBasePoweredBlockEntity.setInternalMaxPower`: the AE of its own
    /// buffer, filled by Forge Energy on its power sides; none without one.
    pub power_capacity: Option<f64>,
    /// The state of a cable part (its item's id), which no block holds: MEGA's
    /// Cell Dock (`CellDockPart`), a machine of its own kind on the cable bus.
    pub part: bool,
}
static EXT_MACHINES: std::sync::OnceLock<Vec<ExtMachine>> = std::sync::OnceLock::new();
/// The addon machines, installed once at start-up before any world loads;
/// a second call keeps the first table.
pub fn install_ext_machines(machines: Vec<ExtMachine>) {
    let _ = EXT_MACHINES.set(machines);
}
/// The installed addon machines (none before [`install_ext_machines`]).
pub fn ext_machines() -> &'static [ExtMachine] {
    EXT_MACHINES.get().map_or(&[], Vec::as_slice)
}
impl MachineKind {
    /// An addon machine's row.
    pub fn ext(self) -> Option<&'static ExtMachine> {
        match self {
            Self::Ext(i) => ext_machines().get(usize::from(i)),
            _ => None,
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::Drive => "ae2:drive",
            Self::Chest => "ae2:chest",
            Self::VibrationChamber => "ae2:vibration_chamber",
            Self::Interface => "ae2:interface",
            Self::PatternProvider => "ae2:pattern_provider",
            Self::Inscriber => "ae2:inscriber",
            Self::Charger => "ae2:charger",
            Self::MolecularAssembler => "ae2:molecular_assembler",
            Self::Controller => "ae2:controller",
            Self::EnergyAcceptor => "ae2:energy_acceptor",
            Self::EnergyCell => "ae2:energy_cell",
            Self::DenseEnergyCell => "ae2:dense_energy_cell",
            Self::CableBus => "ae2:cable_bus",
            Self::WirelessAccessPoint => "ae2:wireless_access_point",
            Self::Terminal => "ae2:terminal",
            Self::CraftingTerminal => "ae2:crafting_terminal",
            Self::PatternEncodingTerminal => "ae2:pattern_encoding_terminal",
            Self::CraftingUnit => "ae2:crafting_unit",
            Self::CraftingAccelerator => "ae2:crafting_accelerator",
            Self::CraftingStorage1k => "ae2:1k_crafting_storage",
            Self::CraftingStorage4k => "ae2:4k_crafting_storage",
            Self::CraftingStorage16k => "ae2:16k_crafting_storage",
            Self::CraftingStorage64k => "ae2:64k_crafting_storage",
            Self::CraftingStorage256k => "ae2:256k_crafting_storage",
            Self::CraftingMonitor => "ae2:crafting_monitor",
            Self::IoPort => "ae2:io_port",
            Self::CellWorkbench => "ae2:cell_workbench",
            Self::Condenser => "ae2:condenser",
            Self::SpatialIoPort => "ae2:spatial_io_port",
            Self::SpatialPylon => "ae2:spatial_pylon",
            Self::SpatialAnchor => "ae2:spatial_anchor",
            Self::QuantumRing => "ae2:quantum_ring",
            Self::QuantumLink => "ae2:quantum_link",
            Self::GrowthAccelerator => "ae2:growth_accelerator",
            Self::CrystalResonanceGenerator => "ae2:crystal_resonance_generator",
            Self::CreativeEnergyCell => "ae2:creative_energy_cell",
            Self::DebugEnergyGenerator => "ae2:debug_energy_gen",
            Self::DebugItemGenerator => "ae2:debug_item_gen",
            Self::DebugCubeGenerator => "ae2:debug_cube_gen",
            Self::DebugPhantomNode => "ae2:debug_phantom_node",
            Self::SkyStoneTank => "ae2:sky_stone_tank",
            Self::SkyStoneChest => "ae2:sky_stone_chest",
            Self::SmoothSkyStoneChest => "ae2:smooth_sky_stone_chest",
            Self::LightDetector => "ae2:light_detector",
            Self::Paint => "ae2:paint",
            Self::Crank => "ae2:crank",
            Self::MysteriousCube => "ae2:mysterious_cube",
            Self::Ext(_) => self.ext().map_or("ae2:unknown_addon_machine", |m| m.id),
        }
    }
    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "ae2:drive" => Self::Drive,
            "ae2:chest" => Self::Chest,
            "ae2:vibration_chamber" => Self::VibrationChamber,
            "ae2:interface" => Self::Interface,
            "ae2:pattern_provider" => Self::PatternProvider,
            "ae2:inscriber" => Self::Inscriber,
            "ae2:charger" => Self::Charger,
            "ae2:molecular_assembler" => Self::MolecularAssembler,
            "ae2:controller" => Self::Controller,
            "ae2:energy_acceptor" => Self::EnergyAcceptor,
            "ae2:energy_cell" => Self::EnergyCell,
            "ae2:dense_energy_cell" => Self::DenseEnergyCell,
            "ae2:cable_bus" => Self::CableBus,
            "ae2:wireless_access_point" => Self::WirelessAccessPoint,
            "ae2:terminal" => Self::Terminal,
            "ae2:crafting_terminal" => Self::CraftingTerminal,
            "ae2:pattern_encoding_terminal" => Self::PatternEncodingTerminal,
            "ae2:crafting_unit" => Self::CraftingUnit,
            "ae2:crafting_accelerator" => Self::CraftingAccelerator,
            "ae2:1k_crafting_storage" => Self::CraftingStorage1k,
            "ae2:4k_crafting_storage" => Self::CraftingStorage4k,
            "ae2:16k_crafting_storage" => Self::CraftingStorage16k,
            "ae2:64k_crafting_storage" => Self::CraftingStorage64k,
            "ae2:256k_crafting_storage" => Self::CraftingStorage256k,
            "ae2:crafting_monitor" => Self::CraftingMonitor,
            "ae2:io_port" => Self::IoPort,
            "ae2:cell_workbench" => Self::CellWorkbench,
            "ae2:condenser" => Self::Condenser,
            "ae2:spatial_io_port" => Self::SpatialIoPort,
            "ae2:spatial_pylon" => Self::SpatialPylon,
            "ae2:spatial_anchor" => Self::SpatialAnchor,
            "ae2:quantum_ring" => Self::QuantumRing,
            "ae2:quantum_link" => Self::QuantumLink,
            "ae2:growth_accelerator" => Self::GrowthAccelerator,
            "ae2:crystal_resonance_generator" => Self::CrystalResonanceGenerator,
            "ae2:creative_energy_cell" => Self::CreativeEnergyCell,
            "ae2:debug_energy_gen" => Self::DebugEnergyGenerator,
            "ae2:debug_item_gen" => Self::DebugItemGenerator,
            "ae2:debug_cube_gen" => Self::DebugCubeGenerator,
            "ae2:debug_phantom_node" => Self::DebugPhantomNode,
            "ae2:sky_stone_tank" => Self::SkyStoneTank,
            "ae2:sky_stone_chest" => Self::SkyStoneChest,
            "ae2:smooth_sky_stone_chest" => Self::SmoothSkyStoneChest,
            "ae2:light_detector" => Self::LightDetector,
            "ae2:paint" => Self::Paint,
            "ae2:crank" => Self::Crank,
            "ae2:mysterious_cube" => Self::MysteriousCube,
            _ => {
                let i = ext_machines().iter().position(|m| m.id == id)?;
                Self::Ext(u16::try_from(i).ok()?)
            }
        })
    }
    pub fn physical_slots(self) -> usize {
        match self {
            Self::Drive => 10,
            Self::DebugItemGenerator => 16,
            Self::Chest | Self::VibrationChamber | Self::Charger | Self::WirelessAccessPoint => 1,
            Self::Interface | Self::PatternProvider | Self::CraftingTerminal => 9,
            Self::MolecularAssembler => 11,
            Self::Inscriber => 4,
            Self::IoPort => 12,
            Self::CellWorkbench => 1,
            Self::Condenser | Self::SpatialIoPort => 2,
            Self::PatternEncodingTerminal => 2,
            Self::QuantumRing | Self::QuantumLink => 1,
            Self::SkyStoneChest | Self::SmoothSkyStoneChest => 36,
            Self::Ext(_) => self.ext().map_or(0, |m| m.physical_slots),
            _ => 0,
        }
    }
    /// Physical card slots from AE2 19.2.18; cell-workbench upgrades live on its cell.
    pub fn upgrade_slots(self) -> usize {
        match self {
            Self::Inscriber => 4,
            Self::VibrationChamber | Self::IoPort => 3,
            Self::MolecularAssembler => 5,
            Self::Interface => 1,
            Self::Ext(_) => self.ext().map_or(0, |m| m.upgrade_slots),
            _ => 0,
        }
    }
}
/// A block or part of an addon built in that is one of AE2's machines with
/// other inventory sizes. Its behaviour is `kind`'s; only its ids and sizes
/// differ (ExtendedAE's `TileExPatternProvider extends
/// PatternProviderBlockEntity` and the like). The addons' rows install them
/// ([`install_machine_variants`]), as they do [`ExtMachine`]s.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MachineVariant {
    /// The block's id, which is also its block entity's, or the part's item id.
    pub id: &'static str,
    /// The AE2 block or part it behaves as, for code that goes by AE2's ids.
    pub base: &'static str,
    /// The block whose screen its menu shows (itself, for a block).
    pub block: &'static str,
    pub kind: MachineKind,
    pub slots: usize,
    pub upgrades: usize,
    /// A part on a cable, not a block.
    pub part: bool,
}
static MACHINE_VARIANTS: std::sync::OnceLock<Vec<MachineVariant>> = std::sync::OnceLock::new();
/// The addon machine variants, installed once at start-up before any world
/// loads; a second call keeps the first table.
pub fn install_machine_variants(variants: Vec<MachineVariant>) {
    let _ = MACHINE_VARIANTS.set(variants);
}
/// The installed machine variants (none before [`install_machine_variants`]).
pub fn machine_variants() -> &'static [MachineVariant] {
    MACHINE_VARIANTS.get().map_or(&[], Vec::as_slice)
}
/// The addon row of block or part `id`.
pub fn variant(id: &str) -> Option<&'static MachineVariant> {
    machine_variants().iter().find(|v| v.id == id)
}
/// The AE2 id that block or part `id` behaves as: its own for AE2's.
pub fn base_id(id: &str) -> &str {
    variant(id).map_or(id, |v| v.base)
}
/// The upgrade cards bus part `id` holds in its record: AE2's five
/// (`IOBusPart.getUpgradeSlots`), or an addon bus's own (a part variant of
/// `MachineKind::CableBus`, which is no machine of its own).
pub fn bus_card_slots(id: &str) -> usize {
    variant(id).filter(|v| v.part && v.kind == MachineKind::CableBus).map_or(5, |v| v.upgrades)
}
/// Whether block `id` is an AE2 machine, AE2's own or an addon's.
pub fn is_machine_block(id: &str) -> bool {
    MachineKind::from_id(id).is_some_and(|kind| kind.ext().is_none_or(|m| !m.part)) || variant(id).is_some_and(|v| !v.part)
}
/// A crafting CPU block: AE2's `AbstractCraftingUnitBlock` on the
/// `ICraftingUnitType` it is built with. AE2's own are [`AE2_CRAFTING_UNITS`]
/// (`CraftingUnitType`); an addon's are installed with
/// [`install_crafting_units`], each also a [`MachineVariant`] of AE2's block
/// of the same role. All of them are `CraftingBlockEntity`s, so any mix of
/// them forms one cluster (`CraftingCPUCalculator.isValidBlockEntity`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CraftingUnit {
    /// The block's id.
    pub id: &'static str,
    /// AE2's kind of the same role (unit, co-processor, a storage, monitor):
    /// the kind of its machine.
    pub kind: MachineKind,
    /// `ICraftingUnitType.getStorageBytes`.
    pub storage_bytes: u64,
    /// `ICraftingUnitType.getAcceleratorThreads`.
    pub accelerator_threads: u32,
    /// The namespace of its formed model's textures
    /// (`AbstractCraftingUnitModelProvider`'s materials, `block/crafting/`).
    pub textures: &'static str,
    /// The type's affix: a co-processor's or storage's formed model lights
    /// `<textures>:block/crafting/<affix>_light`.
    pub affix: &'static str,
}
impl CraftingUnit {
    /// `<textures>:block/crafting/<name>`, a texture of its formed model.
    pub fn texture(&self, name: &str) -> String {
        format!("{}:block/crafting/{name}", self.textures)
    }
}
const fn ae2_unit(id: &'static str, kind: MachineKind, storage_kb: u64, affix: &'static str) -> CraftingUnit {
    CraftingUnit {
        id,
        kind,
        // CraftingUnitType.getStorageBytes: 1024 × storageKb.
        storage_bytes: 1024 * storage_kb,
        // getAcceleratorThreads: 1 on the Crafting Co-Processing Unit.
        accelerator_threads: if matches!(kind, MachineKind::CraftingAccelerator) { 1 } else { 0 },
        textures: "ae2",
        affix,
    }
}
/// AE2 19.2.18's `CraftingUnitType`, block by block.
pub const AE2_CRAFTING_UNITS: [CraftingUnit; 8] = [
    ae2_unit("ae2:crafting_unit", MachineKind::CraftingUnit, 0, "unit"),
    ae2_unit("ae2:crafting_accelerator", MachineKind::CraftingAccelerator, 0, "accelerator"),
    ae2_unit("ae2:1k_crafting_storage", MachineKind::CraftingStorage1k, 1, "1k_storage"),
    ae2_unit("ae2:4k_crafting_storage", MachineKind::CraftingStorage4k, 4, "4k_storage"),
    ae2_unit("ae2:16k_crafting_storage", MachineKind::CraftingStorage16k, 16, "16k_storage"),
    ae2_unit("ae2:64k_crafting_storage", MachineKind::CraftingStorage64k, 64, "64k_storage"),
    ae2_unit("ae2:256k_crafting_storage", MachineKind::CraftingStorage256k, 256, "256k_storage"),
    ae2_unit("ae2:crafting_monitor", MachineKind::CraftingMonitor, 0, "monitor"),
];
static CRAFTING_UNITS: std::sync::OnceLock<Vec<CraftingUnit>> = std::sync::OnceLock::new();
/// The addons' crafting CPU blocks, installed once at start-up before any
/// world loads; a second call keeps the first table.
pub fn install_crafting_units(units: Vec<CraftingUnit>) {
    let _ = CRAFTING_UNITS.set(units);
}
/// Every crafting CPU block: AE2's, then the installed addons'.
pub fn crafting_units() -> impl Iterator<Item = &'static CraftingUnit> {
    AE2_CRAFTING_UNITS.iter().chain(CRAFTING_UNITS.get().map_or(&[][..], Vec::as_slice))
}
/// The crafting CPU block `id`, AE2's or an addon's.
pub fn crafting_unit(id: &str) -> Option<&'static CraftingUnit> {
    crafting_units().find(|unit| unit.id == id)
}
impl MachineKind {
    /// AE2's crafting CPU kinds: what a crafting block of any mod behaves as.
    pub fn is_crafting(self) -> bool {
        matches!(
            self,
            Self::CraftingUnit
                | Self::CraftingAccelerator
                | Self::CraftingStorage1k
                | Self::CraftingStorage4k
                | Self::CraftingStorage16k
                | Self::CraftingStorage64k
                | Self::CraftingStorage256k
                | Self::CraftingMonitor
        )
    }
    /// AE2's energy cell kinds: what an `EnergyCellBlock` of any mod behaves as.
    pub fn is_energy_cell(self) -> bool {
        matches!(self, Self::EnergyCell | Self::DenseEnergyCell)
    }
}
/// An energy cell: AE2's `EnergyCellBlock(maxPower, chargeRate, priority)`
/// with its `EnergyCellBlockItem` and `EnergyCellBlockEntity`. AE2's own are
/// [`AE2_ENERGY_CELLS`]; an addon's are installed with
/// [`install_energy_cells`], each also a [`MachineVariant`] of AE2's cell of
/// the same kind, so the grid, the Charger and the drops treat them alike.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnergyCell {
    /// The block's id, which is also its item's.
    pub id: &'static str,
    /// AE2's kind it behaves as.
    pub kind: MachineKind,
    /// `EnergyCellBlock.getMaxPower`: the block entity's `StoredEnergyAmount`
    /// maximum and the item's `getAEMaxPower`, in AE.
    pub max_power: f64,
    /// `EnergyCellBlock.getChargeRate`: the item's `getChargeRate`, AE a tick
    /// a Charger puts in.
    pub charge_rate: f64,
    /// `EnergyCellBlock.getPriority`: the block entity's `IAEPowerStorage`
    /// priority in the grid's energy service.
    pub priority: i32,
}
impl EnergyCell {
    /// `EnergyCellBlockEntity.getStorageLevelFromFillFactor`: the `fullness`
    /// of the block's state, 0 to `EnergyCellBlock.MAX_FULLNESS` (4), for
    /// `stored` AE.
    pub fn storage_level(&self, stored: f64) -> u8 {
        (4.0 * (stored / self.max_power + 0.01).clamp(0.0, 1.0)).floor() as u8
    }
}
/// AE2 19.2.18's `AEBlocks.ENERGY_CELL` and `DENSE_ENERGY_CELL`.
pub const AE2_ENERGY_CELLS: [EnergyCell; 2] = [
    EnergyCell { id: "ae2:energy_cell", kind: MachineKind::EnergyCell, max_power: 200_000.0, charge_rate: 800.0, priority: 200 },
    EnergyCell {
        id: "ae2:dense_energy_cell",
        kind: MachineKind::DenseEnergyCell,
        max_power: 1_600_000.0,
        charge_rate: 1_600.0,
        priority: 1_600,
    },
];
static ENERGY_CELLS: std::sync::OnceLock<Vec<EnergyCell>> = std::sync::OnceLock::new();
/// The addons' energy cells, installed once at start-up before any world
/// loads; a second call keeps the first table.
pub fn install_energy_cells(cells: Vec<EnergyCell>) {
    let _ = ENERGY_CELLS.set(cells);
}
/// Every energy cell: AE2's, then the installed addons'.
pub fn energy_cells() -> impl Iterator<Item = &'static EnergyCell> {
    AE2_ENERGY_CELLS.iter().chain(ENERGY_CELLS.get().map_or(&[][..], Vec::as_slice))
}
/// The energy cell block or item `id`, AE2's or an addon's.
pub fn energy_cell(id: &str) -> Option<&'static EnergyCell> {
    energy_cells().find(|cell| cell.id == id)
}
/// How many patterns a Pattern Provider of any built-in size holds at most.
pub fn max_provider_slots() -> usize {
    machine_variants()
        .iter()
        .filter(|v| v.kind == MachineKind::PatternProvider)
        .map(|v| v.slots)
        .fold(MachineKind::PatternProvider.physical_slots(), usize::max)
}
/// `slots` are owned physical inventory only. Network contents must never be copied here.
#[derive(Clone, Debug, PartialEq)]
pub struct Machine {
    pub kind: MachineKind,
    /// The addon block or part this is, when it is not AE2's own.
    pub variant: Option<&'static MachineVariant>,
    pub slots: Vec<Option<ItemStack>>,
    /// Real installed cards; never synthetic settings or virtual display slots.
    pub upgrades: Vec<Option<ItemStack>>,
    pub energy: f64,
    /// MEChestBlockEntity captures its buffer maximum in its constructor. This
    /// runtime-only value is rebuilt from configuration when the entity loads.
    pub chest_power_capacity: Option<f64>,
    pub burn_remaining: f64,
    pub burn_rate: f64,
    pub progress: f64,
    pub config: Compound,
}
impl Machine {
    pub fn new(kind: MachineKind) -> Self {
        Self {
            kind,
            variant: None,
            slots: vec![None; kind.physical_slots()],
            upgrades: vec![None; kind.upgrade_slots()],
            energy: 0.0,
            chest_power_capacity: (kind == MachineKind::Chest)
                .then(|| (500.0 * crate::ae2_common_config::gameplay().usage_multiplier)
                    .min(ae2_machines_core::stored_energy::MAX_MAXIMUM)),
            burn_remaining: 0.0,
            burn_rate: 1.0,
            progress: 0.0,
            config: Compound::new(),
        }
    }
    /// The addon's machine of row `variant`.
    pub fn of_variant(variant: &'static MachineVariant) -> Self {
        let mut machine = Self::new(variant.kind);
        machine.variant = Some(variant);
        machine.slots = vec![None; variant.slots];
        machine.upgrades = vec![None; variant.upgrades];
        machine
    }
    /// The block entity's block id, or for a part its item's.
    pub fn id(&self) -> &'static str {
        self.variant.map_or_else(|| self.kind.id(), |v| v.id)
    }
    /// The size of `slots`, whatever the machine holds now.
    pub fn physical_slots(&self) -> usize {
        self.variant.map_or_else(|| self.kind.physical_slots(), |v| v.slots)
    }
    /// The crafting CPU block this is, AE2's or an addon's: what it adds to
    /// its cluster (`CraftingCPUCluster.addBlockEntity`).
    pub fn crafting_unit(&self) -> Option<&'static CraftingUnit> {
        if !self.kind.is_crafting() {
            return None;
        }
        crafting_unit(self.id())
    }
    /// The energy cell this is, AE2's or an addon's: its capacity, charge
    /// rate and priority (`EnergyCellBlock`).
    pub fn energy_cell(&self) -> Option<&'static EnergyCell> {
        if !self.kind.is_energy_cell() {
            return None;
        }
        energy_cell(self.id())
    }
    /// The size of `upgrades`.
    pub fn upgrade_slots(&self) -> usize {
        self.variant.map_or_else(|| self.kind.upgrade_slots(), |v| v.upgrades)
    }
    /// Private AEBasePoweredBlockEntity buffer maximum, fixed at construction.
    pub fn private_power_capacity(&self) -> Option<f64> {
        match self.kind {
            MachineKind::Inscriber | MachineKind::Charger | MachineKind::GrowthAccelerator => Some(1600.0),
            MachineKind::Chest => self.chest_power_capacity,
            MachineKind::Ext(_) => self.kind.ext().and_then(|m| m.power_capacity),
            _ => None,
        }
    }
    pub fn set_private_energy(&mut self, amount: f64) {
        if let Some(maximum) = self.private_power_capacity() {
            let mut stored = ae2_machines_core::stored_energy::StoredEnergy::new(self.energy, maximum);
            stored.set_stored(amount);
            self.energy = stored.stored();
        }
    }
    /// The powered machine and energy-cell source classes use setStored when
    /// their saved block entity is loaded. Other machine state is untouched.
    pub fn source_energy_capacity(&self) -> Option<f64> {
        self.private_power_capacity().or(match self.kind {
            MachineKind::Controller => Some(8000.0),
            MachineKind::EnergyAcceptor => Some(0.0),
            _ => self.energy_cell().map(|cell| cell.max_power),
        })
    }
    pub fn set_saved_energy(&mut self, amount: f64) {
        if let Some(maximum) = self.source_energy_capacity() {
            let mut stored = ae2_machines_core::stored_energy::StoredEnergy::new(0.0, maximum);
            stored.set_stored(amount);
            self.energy = stored.stored();
        } else {
            self.energy = amount;
        }
    }
    pub fn private_energy_insert(&mut self, amount: f64, commit: bool) -> f64 {
        let Some(maximum) = self.private_power_capacity() else { return 0.0; };
        let mut stored = ae2_machines_core::stored_energy::StoredEnergy::new(self.energy, maximum);
        let accepted = stored.insert(amount, commit);
        if commit { self.energy = stored.stored(); }
        accepted
    }
    pub fn private_energy_extract(&mut self, amount: f64, commit: bool) -> f64 {
        let Some(maximum) = self.private_power_capacity() else { return 0.0; };
        let mut stored = ae2_machines_core::stored_energy::StoredEnergy::new(self.energy, maximum);
        let extracted = stored.extract(amount, commit);
        if commit { self.energy = stored.stored(); }
        extracted
    }
    /// Real owned inventory, including crafting CPU escrow. Job metadata and
    /// terminal listings are virtual and must never be emitted as drops.
    pub fn physical_contents(&self) -> Vec<ItemStack> {
        let mut out=self.physical_slots_and_upgrades();
        if let Some(drops) = self.kind.ext().and_then(|m| m.key_drops) {
            out.extend(drops(self).into_iter().map(|(item, amount)| ItemStack {
                count: u32::try_from(amount).unwrap_or(u32::MAX),
                ..item
            }));
        }
        if self.kind.is_crafting() {
            if let Some(horizon_world::nbt::Tag::List(entries)) =
                self.config.get(&"ae2_cpu_escrow".into())
            {
                for entry in entries {
                    // Decoder's compatibility repairs are unsafe for escrow:
                    // require a positive saved count before decoding.
                    if matches!(entry.get("count"),Some(horizon_world::nbt::Tag::Int(n)) if *n>0) {
                        if let Some(stack) = horizon_items::item_nbt::item_from_nbt(entry) {
                            out.push(stack);
                        }
                    }
                }
            }
        }
        if self.kind == MachineKind::PatternProvider {
            if let Some(horizon_world::nbt::Tag::List(entries)) =
                self.config.get(&"ae2_provider_return_items".into())
            {
                for entry in entries.iter().take(9) {
                    if matches!(entry.get("count"),Some(horizon_world::nbt::Tag::Int(n)) if *n>0) {
                        if let Some(stack) = horizon_items::item_nbt::item_from_nbt(entry) {
                            out.push(stack);
                        }
                    }
                }
            }
            if self.config.contains_key(&crate::ae2_provider_queue::QUEUE.into()) {
                if let Ok(entries)=crate::ae2_provider_queue::read(&self.config) {
                    if let Ok(items)=crate::ae2_provider_queue::physical_items(&entries) {out.extend(items);}
                }
            } else if let Some(horizon_world::nbt::Tag::List(entries)) = self.config.get(&"ae2_provider_send_items".into()) {
                if entries.len() <= 8192 {
                    for entry in entries {
                        if matches!(entry.get("count"),Some(horizon_world::nbt::Tag::Int(n)) if *n>0) {
                            if let Some(stack)=horizon_items::item_nbt::item_from_nbt(entry) {out.push(stack);}
                        }
                    }
                }
            }
        }
        out
    }
    /// Physical block slots and installed upgrades; deferred AE2 inventories
    /// are handled by the source-aware drop path using their own limits.
    pub fn physical_slots_and_upgrades(&self) -> Vec<ItemStack> {
        let physical_slots = if self.kind == MachineKind::DebugItemGenerator { &[][..] } else { &self.slots[..] };
        physical_slots
            .iter()
            .chain(self.upgrades.iter())
            .flatten()
            .cloned()
            .collect()
    }
    pub fn installed_speed_cards(&self) -> usize {
        self.upgrades
            .iter()
            .flatten()
            .filter(|s| s.item.as_ref() == "ae2:speed_card" && s.count == 1)
            .count()
    }
    pub fn for_block(id: &str) -> Option<Self> {
        if let Some(kind) = MachineKind::from_id(id).filter(|kind| kind.ext().is_none_or(|m| !m.part)) {
            return Some(Self::new(kind));
        }
        variant(id).filter(|v| !v.part).map(Self::of_variant)
    }
}
