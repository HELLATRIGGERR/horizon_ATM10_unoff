//! ExtendedAE 1.21-2.2.42's rows in the definitions registry (feature
//! `ae2-extendedae`). Derived from ExtendedAE (`TileExPatternProvider`,
//! `PartExPatternProvider`, `TileExInterface`, `PartExInterface`,
//! `TileExDrive`, `TileExIOPort`, `TileCrystalAssembler`, `TileCircuitCutter`,
//! `TileCrystalFixer`, `TileExInscriber`, `TileExCharger`,
//! `TileExMolecularAssembler`, `EAERegistryHandler.registerUpgrades`, `EAEConfig`),
//! https://github.com/GlodBlock/ExtendedAE commit
//! `1f86f927552f02451236465c0c96a12a3de8f66c` (2.2.42), LGPL-3.0; see
//! `ae2/addons/extendedae/`.
//!
//! Its bigger devices are AE2's with other sizes, so each is a
//! [`MachineVariant`] on AE2's kind. Its own machines (Crystal Assembler,
//! Circuit Cutter, Crystal Fixer, and the Extended Inscriber and Charger, whose
//! threads are AE2's machines side by side) are [`ExtMachine`]s. The game's
//! side is in `game/ae2x/extendedae/`.
use super::defs::Defs;
use crate::ae2_world::{ExtMachine, MachineKind, MachineVariant};
use std::sync::OnceLock;

pub const NAMESPACE: &str = "extendedae";
/// `TileCrystalAssembler`: nine inputs, one output, a fluid tank of 16 buckets.
pub const CRYSTAL_ASSEMBLER: &str = "extendedae:crystal_assembler";
/// `TileCircuitCutter`, the Circuit Slicer: one input, one output.
pub const CIRCUIT_CUTTER: &str = "extendedae:circuit_cutter";
/// `TileCrystalFixer`: one fuel slot; it works on the block in front of it.
pub const CRYSTAL_FIXER: &str = "extendedae:crystal_fixer";
pub const EX_IO_PORT: &str = "extendedae:ex_io_port";
pub const EX_IMPORT_BUS: &str = "extendedae:ex_import_bus_part";
pub const EX_EXPORT_BUS: &str = "extendedae:ex_export_bus_part";
/// `TileExInscriber`: four `InscriberThread`s of AE2's four slots each.
pub const EX_INSCRIBER: &str = "extendedae:ex_inscriber";
/// `TileExCharger`: four charging slots.
pub const EX_CHARGER: &str = "extendedae:ex_charger";
/// `TileExMolecularAssembler`: eight `CraftingThread`s.
pub const EX_ASSEMBLER: &str = "extendedae:ex_molecular_assembler";
/// `TileExInscriber.MAX_THREAD` and `TileExCharger.MAX_THREAD`.
pub const THREADS: usize = 4;
/// `TileExMolecularAssembler.MAX_THREAD` and `TileAssemblerMatrixCrafter.MAX_THREAD`:
/// the crafting threads of the Extended Molecular Assembler and of a crafter core.
pub const ASSEMBLER_THREADS: usize = 8;
/// `ItemInfinityCell`s of the mod: endless water and cobblestone.
pub const INFINITY_WATER_CELL: &str = "extendedae:infinity_water_cell";
pub const INFINITY_COBBLESTONE_CELL: &str = "extendedae:infinity_cobblestone_cell";
/// `ItemVoidCell`.
pub const VOID_CELL: &str = "extendedae:void_cell";
/// The Assembler Matrix's blocks (`BlockAssemblerMatrix*`): its edges, faces
/// (walls or glass) and inner function blocks (crafter, pattern and speed cores).
pub const MATRIX_FRAME: &str = "extendedae:assembler_matrix_frame";
pub const MATRIX_WALL: &str = "extendedae:assembler_matrix_wall";
pub const MATRIX_GLASS: &str = "extendedae:assembler_matrix_glass";
pub const MATRIX_CRAFTER: &str = "extendedae:assembler_matrix_crafter";
pub const MATRIX_PATTERN: &str = "extendedae:assembler_matrix_pattern";
pub const MATRIX_SPEED: &str = "extendedae:assembler_matrix_speed";
/// `TileAssemblerMatrixPattern.INV_SIZE`.
pub const MATRIX_PATTERNS: usize = 36;
/// `ClusterAssemblerMatrix.addSpeedCore`: the speed cores that count.
pub const MATRIX_MAX_SPEED: usize = 5;

/// A block of the Assembler Matrix, by its role in the structure
/// (`CalculatorAssemblerMatrix.verifyInternalStructure`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatrixRole {
    /// `TileAssemblerMatrixFrame`: every edge and corner.
    Frame,
    /// `TileAssemblerMatrixWall` and `TileAssemblerMatrixGlass`: the faces.
    Wall,
    /// The `TileAssemblerMatrixFunction`s, inside.
    Crafter,
    Pattern,
    Speed,
}
impl MatrixRole {
    pub fn of(id: &str) -> Option<Self> {
        Some(match id {
            MATRIX_FRAME => Self::Frame,
            MATRIX_WALL | MATRIX_GLASS => Self::Wall,
            MATRIX_CRAFTER => Self::Crafter,
            MATRIX_PATTERN => Self::Pattern,
            MATRIX_SPEED => Self::Speed,
            _ => return None,
        })
    }
    /// `TileAssemblerMatrixFunction`: an inner block.
    pub fn function(self) -> bool {
        matches!(self, Self::Crafter | Self::Pattern | Self::Speed)
    }
    /// `setIdlePowerUsage`: 0.5 AE/t, 1 for a function block.
    pub fn idle_power(self) -> f64 {
        if self.function() { 1.0 } else { 0.5 }
    }
}

/// `extendedae-common.toml` (`EAEConfig`): what this port reads of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    /// `device.extended_io_bus_multiplier`, 2 to 128: the Extended Import and
    /// Export Buses' operations, times AE2's.
    pub bus_speed: u64,
    /// `item.infinity_cell_energy_cost`, 0.1 to 64: an infinity cell's AE/t.
    pub infinity_cell_cost: f64,
    /// `device.assembler_matrix_max_size`, 3 to 16: how far the Assembler
    /// Matrix's corners may lie apart on each axis (`checkMultiblockScale`).
    pub matrix_size: i32,
}
impl Default for Config {
    fn default() -> Self {
        Self { bus_speed: 8, infinity_cell_cost: 8.0, matrix_size: 6 }
    }
}
impl Config {
    /// A value of the wrong type or outside its range falls back to its
    /// default, as `ModConfigSpec` corrects it.
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut config = Self::default();
        let speed = value.get("device").and_then(|d| d.get("extended_io_bus_multiplier")).and_then(toml::Value::as_integer);
        if let Some(n) = speed.filter(|n| (2..=128).contains(n)) {
            config.bus_speed = n as u64;
        }
        let cost = value.get("item").and_then(|d| d.get("infinity_cell_energy_cost")).and_then(toml::Value::as_float);
        if let Some(n) = cost.filter(|n| (0.1..=64.0).contains(n)) {
            config.infinity_cell_cost = n;
        }
        let size = value.get("device").and_then(|d| d.get("assembler_matrix_max_size")).and_then(toml::Value::as_integer);
        if let Some(n) = size.filter(|n| (3..=16).contains(n)) {
            config.matrix_size = n as i32;
        }
        Ok(config)
    }
    /// The game's `config/extendedae-common.toml`, read once (`ModConfigEvent`).
    pub fn get() -> Self {
        static CONFIG: OnceLock<Config> = OnceLock::new();
        *CONFIG.get_or_init(|| {
            let path = crate::logging::game_home().join("config/extendedae-common.toml");
            match std::fs::read_to_string(&path) {
                Ok(text) => Self::parse(&text).unwrap_or_else(|error| {
                    log::warn!("invalid ExtendedAE config {}: {error}", path.display());
                    Self::default()
                }),
                Err(_) => Self::default(),
            }
        })
    }
}

/// The variants, as the mod sizes them.
pub const MACHINES: &[MachineVariant] = &[
    // PatternProviderLogic(node, host, 36); its return inventory stays nine slots.
    MachineVariant {
        id: "extendedae:ex_pattern_provider",
        base: "ae2:pattern_provider",
        block: "extendedae:ex_pattern_provider",
        kind: MachineKind::PatternProvider,
        slots: 36,
        upgrades: 0,
        part: false,
    },
    MachineVariant {
        id: "extendedae:ex_pattern_provider_part",
        base: "ae2:cable_pattern_provider",
        block: "extendedae:ex_pattern_provider",
        kind: MachineKind::PatternProvider,
        slots: 36,
        upgrades: 0,
        part: true,
    },
    // InterfaceLogic(node, host, item, 36): 36 config and 36 storage slots, one card.
    MachineVariant {
        id: "extendedae:ex_interface",
        base: "ae2:interface",
        block: "extendedae:ex_interface",
        kind: MachineKind::Interface,
        slots: 36,
        upgrades: 1,
        part: false,
    },
    MachineVariant {
        id: "extendedae:ex_interface_part",
        base: "ae2:cable_interface",
        block: "extendedae:ex_interface",
        kind: MachineKind::Interface,
        slots: 36,
        upgrades: 1,
        part: true,
    },
    // getCellCount() = 20.
    MachineVariant {
        id: "extendedae:ex_drive",
        base: "ae2:drive",
        block: "extendedae:ex_drive",
        kind: MachineKind::Drive,
        slots: 20,
        upgrades: 0,
        part: false,
    },
    // PartExImportBus and PartExExportBus: AE2's buses with eight cards
    // (getUpgradeSlots) and busSpeed times the operations. A bus is no
    // machine of its own: the kind CableBus says so (`bus_card_slots`).
    MachineVariant {
        id: EX_IMPORT_BUS,
        base: "ae2:import_bus",
        block: EX_IMPORT_BUS,
        kind: MachineKind::CableBus,
        slots: 0,
        upgrades: 8,
        part: true,
    },
    MachineVariant {
        id: EX_EXPORT_BUS,
        base: "ae2:export_bus",
        block: EX_EXPORT_BUS,
        kind: MachineKind::CableBus,
        slots: 0,
        upgrades: 8,
        part: true,
    },
    // IOPortBlockEntity's six cells in, six out; UpgradeInventories.forMachine(EX_IO_PORT, 5).
    // TileExMolecularAssembler: eight threads; the first is AE2's eleven
    // slots (the grid, the output, a pattern slot it never shows), the others
    // keep theirs in the machine's config (`game/ae2_assembler_threads.rs`);
    // five cards and the mod's speeds (`CraftingThread.tick`).
    MachineVariant {
        id: EX_ASSEMBLER,
        base: "ae2:molecular_assembler",
        block: EX_ASSEMBLER,
        kind: MachineKind::MolecularAssembler,
        slots: 11,
        upgrades: 5,
        part: false,
    },
    MachineVariant {
        id: EX_IO_PORT,
        base: "ae2:io_port",
        block: EX_IO_PORT,
        kind: MachineKind::IoPort,
        slots: 12,
        upgrades: 5,
        part: false,
    },
    // TileAssemblerMatrixPattern: an ICraftingProvider and PatternContainer of
    // 36 patterns, so the grid's crafting service and the Pattern Access
    // Terminal take it as a provider; its jobs go to the matrix's crafter
    // cores, not to its neighbours (`game/ae2x/extendedae/matrix.rs`).
    MachineVariant {
        id: MATRIX_PATTERN,
        base: "ae2:pattern_provider",
        block: MATRIX_PATTERN,
        kind: MachineKind::PatternProvider,
        slots: MATRIX_PATTERNS,
        upgrades: 0,
        part: false,
    },
];

/// `PartExImportBus` and `PartExExportBus.getOperationsPerTick`: AE2's times
/// `extended_io_bus_multiplier`; 1 for any other bus.
pub fn bus_operations_multiplier(id: &str) -> u64 {
    if matches!(id, EX_IMPORT_BUS | EX_EXPORT_BUS) { Config::get().bus_speed } else { 1 }
}

/// `TileExIOPort.tickingRequest`: 2,048 items a tick, doubled by each speed
/// card up to five (AE2's own port moves 256, doubled up to three).
pub fn io_port_budget(speed_cards: usize) -> u64 {
    2048 << speed_cards.min(5)
}

/// The block state's `working` of the Crystal Assembler and Circuit Cutter
/// (`setWorking`), kept in the machine's config.
pub const WORKING: &str = "eae_working";

/// What the mod's own machines do not save (`saveAdditional`): the Crystal
/// Assembler's and Circuit Cutter's progress and `isWorking`. The Crystal
/// Fixer saves its progress.
pub fn reset_unsaved(m: &mut crate::ae2_world::Machine) {
    match m.kind.ext().map(|row| row.id) {
        Some(CRYSTAL_ASSEMBLER | CIRCUIT_CUTTER) => {
            m.progress = 0.0;
            m.config.remove(&WORKING.into());
        }
        // InscriberThread keeps neither its processingTime nor its smash.
        Some(EX_INSCRIBER) => {
            for thread in 0..THREADS {
                m.config.remove(&thread_key(thread).as_str().into());
            }
        }
        _ => {}
    }
}

/// Where the Extended Inscriber keeps thread `n`'s progress and smash, none
/// of which it saves.
pub fn thread_key(n: usize) -> String {
    format!("eae_thread{n}")
}

/// The mod's own machines: physical slots, cards, and the AE buffer
/// (`POWER_MAXIMUM_AMOUNT`, 8,000 on each).
pub const OWN_MACHINES: &[ExtMachine] = &[
    ExtMachine { id: CRYSTAL_ASSEMBLER, physical_slots: 10, upgrade_slots: 4, key_drops: None, power_capacity: Some(8000.0), part: false },
    ExtMachine { id: CIRCUIT_CUTTER, physical_slots: 2, upgrade_slots: 4, key_drops: None, power_capacity: Some(8000.0), part: false },
    ExtMachine { id: CRYSTAL_FIXER, physical_slots: 1, upgrade_slots: 0, key_drops: None, power_capacity: Some(8000.0), part: false },
    // Each thread's top, bottom, middle and output; four speed cards; 1,600 AE.
    ExtMachine { id: EX_INSCRIBER, physical_slots: 16, upgrade_slots: 4, key_drops: None, power_capacity: Some(1600.0), part: false },
    // POWER_MAXIMUM_AMOUNT 3,200.
    ExtMachine { id: EX_CHARGER, physical_slots: THREADS, upgrade_slots: 0, key_drops: None, power_capacity: Some(3200.0), part: false },
    // The Assembler Matrix: plain nodes, and the crafter core, whose first
    // thread is AE2's assembler's eleven slots (no pattern slot of its own) and
    // whose seven more keep theirs in its config (`game/ae2_assembler_threads.rs`).
    ExtMachine { id: MATRIX_FRAME, physical_slots: 0, upgrade_slots: 0, key_drops: None, power_capacity: None, part: false },
    ExtMachine { id: MATRIX_WALL, physical_slots: 0, upgrade_slots: 0, key_drops: None, power_capacity: None, part: false },
    ExtMachine { id: MATRIX_GLASS, physical_slots: 0, upgrade_slots: 0, key_drops: None, power_capacity: None, part: false },
    ExtMachine { id: MATRIX_CRAFTER, physical_slots: 11, upgrade_slots: 0, key_drops: None, power_capacity: None, part: false },
    ExtMachine { id: MATRIX_SPEED, physical_slots: 0, upgrade_slots: 0, key_drops: None, power_capacity: None, part: false },
];

/// An `ItemInfinityCell` (`InfinityCellInventory`): the one key it holds
/// (`getRecord`), an item or a fluid, and the model a drive draws for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InfinityCell {
    pub id: String,
    pub fluid: bool,
    pub record: String,
    pub drive_model: String,
}
/// The mod's two (`EAESingletons`, `registerStorageHandler`).
fn own_infinity_cells() -> [InfinityCell; 2] {
    let cell = |id: &str, fluid: bool, record: &str, model: &str| InfinityCell { id: id.into(), fluid, record: record.into(), drive_model: model.into() };
    [
        cell(INFINITY_WATER_CELL, true, "minecraft:water", "extendedae:block/drive/infinity_water_cell"),
        cell(INFINITY_COBBLESTONE_CELL, false, "minecraft:cobblestone", "extendedae:block/drive/infinity_cobblestone_cell"),
    ]
}
/// Where the content keeps the pack's own infinity cells: All the Mods 10's
/// KubeJS `custom_infinity_cell`s (`InfinityCellBuilder`), which
/// `prepare-addon.ts` reads from the pack's startup script.
pub const PACK_INFINITY_CELLS: &str = "data/extendedae/ae2x/infinity_cells.json";
static PACK_CELLS: OnceLock<Vec<InfinityCell>> = OnceLock::new();
/// The pack's rows, `[{"id", "item" or "fluid", "model"}]`; a row of neither
/// is left out, as `TraceableSupplier` fails the cell.
pub fn parse_pack_cells(text: &str) -> Result<Vec<InfinityCell>, String> {
    let rows: Vec<serde_json::Value> = serde_json::from_str(text).map_err(|e| e.to_string())?;
    Ok(rows
        .iter()
        .filter_map(|row| {
            let id = row.get("id")?.as_str()?.to_owned();
            let (fluid, record) = match (row.get("item").and_then(|v| v.as_str()), row.get("fluid").and_then(|v| v.as_str())) {
                (Some(item), None) => (false, item),
                (None, Some(fluid)) => (true, fluid),
                _ => return None,
            };
            // A cell with no model draws AE2's generic one (`StorageCellModels`).
            let drive_model = row.get("model").and_then(|v| v.as_str()).unwrap_or("ae2:block/drive/drive_cell").to_owned();
            Some(InfinityCell { id, fluid, record: record.to_owned(), drive_model })
        })
        .collect())
}
/// The pack's infinity cells from the content root `root`, once, before any
/// world loads; none when the content has no such file.
pub fn install_pack_cells(root: &std::path::Path) {
    let cells = match std::fs::read_to_string(root.join(PACK_INFINITY_CELLS)) {
        Ok(text) => parse_pack_cells(&text).unwrap_or_else(|error| {
            log::warn!("ExtendedAE pack infinity cells: {error}");
            Vec::new()
        }),
        Err(_) => Vec::new(),
    };
    let _ = PACK_CELLS.set(cells);
}
/// The infinity cell `id`: the mod's, then the pack's.
pub fn infinity_cell(id: &str) -> Option<InfinityCell> {
    own_infinity_cells().into_iter().find(|c| c.id == id).or_else(|| PACK_CELLS.get()?.iter().find(|c| c.id == id).cloned())
}
/// The model a drive draws for the infinity cell `id`.
pub fn infinity_cell_model(id: &str) -> Option<&'static str> {
    match id {
        INFINITY_WATER_CELL => Some("extendedae:block/drive/infinity_water_cell"),
        INFINITY_COBBLESTONE_CELL => Some("extendedae:block/drive/infinity_cobblestone_cell"),
        _ => PACK_CELLS.get()?.iter().find(|c| c.id == id).map(|c| c.drive_model.as_str()),
    }
}
/// `ItemInfinityCell.getName`: "ME Infinity %s Cell" of its record's name.
pub fn infinity_cell_name(id: &str, translate: &dyn Fn(&str) -> Option<String>, item_name: &dyn Fn(&str) -> String) -> Option<String> {
    let cell = infinity_cell(id)?;
    let record = if cell.fluid {
        // AEFluidKey.getDisplayName: the fluid type's description, a fluid block's name.
        let (ns, path) = cell.record.split_once(':').unwrap_or(("minecraft", &cell.record));
        translate(&format!("block.{ns}.{path}")).or_else(|| translate(&format!("fluid_type.{ns}.{path}"))).unwrap_or_else(|| path.replace('_', " "))
    } else {
        item_name(&cell.record)
    };
    let format = translate("item.extendedae.infinity_cell_name").unwrap_or_else(|| "ME Infinity %s Cell".into());
    Some(format.replacen("%s", &record, 1).replace("%1$s", &record))
}

pub fn register(defs: &mut Defs) {
    for machine in MACHINES {
        defs.add_variant(*machine);
    }
    for machine in OWN_MACHINES {
        defs.add_machine(*machine);
    }
    // EAERegistryHandler.registerUpgrades, for the devices carried so far.
    for host in ["extendedae:ex_interface", "extendedae:ex_interface_part"] {
        defs.add_upgrade("ae2:fuzzy_card", host, 1);
        defs.add_upgrade("ae2:crafting_card", host, 1);
    }
    for (card, max) in [("ae2:capacity_card", 5), ("ae2:redstone_card", 1), ("ae2:speed_card", 4), ("ae2:fuzzy_card", 1), ("ae2:crafting_card", 1)] {
        defs.add_upgrade(card, EX_EXPORT_BUS, max);
    }
    for (card, max) in [("ae2:capacity_card", 5), ("ae2:redstone_card", 1), ("ae2:speed_card", 4), ("ae2:inverter_card", 1), ("ae2:fuzzy_card", 1)] {
        defs.add_upgrade(card, EX_IMPORT_BUS, max);
    }
    defs.add_upgrade("ae2:speed_card", EX_IO_PORT, 5);
    defs.add_upgrade("ae2:redstone_card", EX_IO_PORT, 1);
    defs.add_upgrade("ae2:speed_card", CRYSTAL_ASSEMBLER, 4);
    defs.add_upgrade("ae2:speed_card", CIRCUIT_CUTTER, 4);
    defs.add_upgrade("ae2:speed_card", EX_ASSEMBLER, 5);
    defs.add_upgrade("ae2:speed_card", EX_INSCRIBER, 4);
    // ItemVoidCell: StorageCells.addCellHandler(VoidCellInventory.HANDLER),
    // CellConfig.create (63 filters), UpgradeInventories.forItem(is, 2),
    // getIdleDrain 1.
    defs.add_handler_cell(
        crate::ae2x::defs::HandlerCellDef { id: VOID_CELL.into(), idle_drain: 1.0, drive_model: "extendedae:block/drive/void_cell".into(), config_slots: 63 },
        2,
    );
    defs.add_upgrade("ae2:fuzzy_card", VOID_CELL, 1);
    defs.add_upgrade("ae2:inverter_card", VOID_CELL, 1);
}

#[cfg(test)]
mod tests {
    use crate::ae2_world::{Machine, MachineKind, base_id, is_machine_block, max_provider_slots, variant};
    use crate::containers::BlockEntity;

    #[test]
    fn extendedae_rows_keep_the_mods_sizes() {
        crate::ae2x::install();
        for (id, kind, slots, upgrades) in [
            ("extendedae:ex_pattern_provider", MachineKind::PatternProvider, 36, 0),
            ("extendedae:ex_interface", MachineKind::Interface, 36, 1),
            ("extendedae:ex_drive", MachineKind::Drive, 20, 0),
        ] {
            let machine = Machine::for_block(id).unwrap();
            assert_eq!((machine.kind, machine.slots.len(), machine.upgrades.len()), (kind, slots, upgrades), "{id}");
            assert_eq!(machine.id(), id);
            assert!(is_machine_block(id));
            assert_eq!(BlockEntity::Ae2(Box::new(machine)).id(), id);
        }
        for (id, base) in [
            ("extendedae:ex_pattern_provider_part", "ae2:cable_pattern_provider"),
            ("extendedae:ex_interface_part", "ae2:cable_interface"),
        ] {
            // Parts are no blocks.
            assert!(Machine::for_block(id).is_none());
            assert!(!is_machine_block(id));
            assert_eq!(variant(id).unwrap().slots, 36);
            assert_eq!(base_id(id), base);
        }
        assert_eq!(base_id("extendedae:ex_drive"), "ae2:drive");
        assert_eq!(base_id("ae2:drive"), "ae2:drive");
        assert_eq!(max_provider_slots(), 36);
        // AE2's own machines stay as they are, and a variant is no addon kind of its own.
        assert_eq!(Machine::for_block("ae2:pattern_provider").unwrap().slots.len(), 9);
        assert_eq!(Machine::for_block("ae2:drive").unwrap().id(), "ae2:drive");
        assert!(MachineKind::from_id("extendedae:ex_drive").is_none());
        let port = Machine::for_block(super::EX_IO_PORT).unwrap();
        assert_eq!((port.kind, port.slots.len(), port.upgrades.len()), (MachineKind::IoPort, 12, 5));
        assert_eq!(base_id(super::EX_IO_PORT), "ae2:io_port");
        // The buses are parts and no machines, with eight cards.
        for (id, base) in [(super::EX_IMPORT_BUS, "ae2:import_bus"), (super::EX_EXPORT_BUS, "ae2:export_bus")] {
            assert_eq!(base_id(id), base);
            assert_eq!(crate::ae2_world::bus_card_slots(id), 8);
            assert!(Machine::for_block(id).is_none() && !is_machine_block(id));
        }
        assert_eq!(crate::ae2_world::bus_card_slots("ae2:import_bus"), 5);
        assert_eq!(crate::ae2_world::bus_card_slots("extendedae:ex_interface_part"), 5);
    }

    #[test]
    fn extendedae_machines_have_their_inventories_and_cards() {
        crate::ae2x::install();
        let defs = crate::ae2x::defs::get();
        for (id, slots, upgrades) in [(super::CRYSTAL_ASSEMBLER, 10, 4), (super::CIRCUIT_CUTTER, 2, 4), (super::CRYSTAL_FIXER, 1, 0)] {
            assert!(matches!(MachineKind::from_id(id), Some(MachineKind::Ext(_))));
            let machine = Machine::for_block(id).unwrap();
            assert_eq!((machine.slots.len(), machine.upgrades.len(), machine.id()), (slots, upgrades, id));
            assert_eq!(machine.private_power_capacity(), Some(8000.0));
            assert!(is_machine_block(id));
        }
        assert_eq!(defs.upgrade_limit("ae2:speed_card", super::CRYSTAL_ASSEMBLER), 4);
        for (id, slots, upgrades, power) in [(super::EX_INSCRIBER, 16, 4, 1600.0), (super::EX_CHARGER, 4, 0, 3200.0)] {
            let machine = Machine::for_block(id).unwrap();
            assert_eq!((machine.slots.len(), machine.upgrades.len(), machine.private_power_capacity()), (slots, upgrades, Some(power)), "{id}");
        }
        assert_eq!(defs.upgrade_limit("ae2:speed_card", super::EX_INSCRIBER), 4);
        // The Extended Molecular Assembler is AE2's with five cards.
        let assembler = Machine::for_block(super::EX_ASSEMBLER).unwrap();
        assert_eq!((assembler.kind, assembler.slots.len(), assembler.upgrades.len(), assembler.id()), (MachineKind::MolecularAssembler, 11, 5, super::EX_ASSEMBLER));
        assert_eq!(base_id(super::EX_ASSEMBLER), "ae2:molecular_assembler");
        assert_eq!(defs.upgrade_limit("ae2:speed_card", super::EX_ASSEMBLER), 5);
        assert_eq!(defs.upgrade_limit("ae2:speed_card", super::EX_IO_PORT), 5);
        assert_eq!(defs.upgrade_limit("ae2:capacity_card", super::EX_IO_PORT), 0);
        assert_eq!(defs.upgrade_limit("ae2:inverter_card", super::EX_IMPORT_BUS), 1);
        assert_eq!(defs.upgrade_limit("ae2:inverter_card", super::EX_EXPORT_BUS), 0);
        assert_eq!(defs.upgrade_limit("ae2:crafting_card", super::EX_EXPORT_BUS), 1);
        // 2048, then x2, x4, x8, x16, x32; AE2's port moves 256.
        assert_eq!((0..=6).map(super::io_port_budget).collect::<Vec<_>>(), [2048, 4096, 8192, 16384, 32768, 65536, 65536]);
    }

    #[test]
    fn extendedae_config_reads_the_bus_multiplier() {
        use super::Config;
        assert_eq!(Config::parse("[device]\n\textended_io_bus_multiplier = 8\n").unwrap().bus_speed, 8);
        assert_eq!(Config::parse("[device]\nextended_io_bus_multiplier = 16").unwrap().bus_speed, 16);
        // Outside 2..=128: the default, as ModConfigSpec corrects it.
        assert_eq!(Config::parse("[device]\nextended_io_bus_multiplier = 1").unwrap().bus_speed, 8);
        assert_eq!(Config::parse("").unwrap(), Config::default());
        assert_eq!(Config::default().infinity_cell_cost, 8.0);
        assert_eq!(Config::parse("[item]\ninfinity_cell_energy_cost = 2.5").unwrap().infinity_cell_cost, 2.5);
        assert_eq!(Config::parse("[item]\ninfinity_cell_energy_cost = 100.0").unwrap().infinity_cell_cost, 8.0);
        // device.assembler_matrix_max_size: 6 by default (ATM10 ships no file), 3 to 16.
        assert_eq!(Config::default().matrix_size, 6);
        assert_eq!(Config::parse("[device]\nassembler_matrix_max_size = 9").unwrap().matrix_size, 9);
        assert_eq!(Config::parse("[device]\nassembler_matrix_max_size = 2").unwrap().matrix_size, 6);
    }

    #[test]
    fn infinity_cells_hold_their_record() {
        use super::*;
        let water = infinity_cell(INFINITY_WATER_CELL).unwrap();
        assert_eq!((water.fluid, water.record.as_str()), (true, "minecraft:water"));
        let cobble = infinity_cell(INFINITY_COBBLESTONE_CELL).unwrap();
        assert_eq!((cobble.fluid, cobble.record.as_str()), (false, "minecraft:cobblestone"));
        assert_eq!(infinity_cell_model(INFINITY_COBBLESTONE_CELL), Some("extendedae:block/drive/infinity_cobblestone_cell"));
        assert!(infinity_cell("ae2:item_storage_cell_1k").is_none());
        let pack = parse_pack_cells(r#"[{"id":"kubejs:lava_cell","fluid":"minecraft:lava","model":"kubejs:block/drive/lava_cell"},{"id":"kubejs:dirt_cell","item":"minecraft:dirt"},{"id":"kubejs:broken"}]"#).unwrap();
        assert_eq!(pack.len(), 2);
        assert_eq!((pack[0].fluid, pack[0].record.as_str(), pack[0].drive_model.as_str()), (true, "minecraft:lava", "kubejs:block/drive/lava_cell"));
        assert_eq!((pack[1].fluid, pack[1].drive_model.as_str()), (false, "ae2:block/drive/drive_cell"));
        let translate = |key: &str| match key {
            "item.extendedae.infinity_cell_name" => Some("ME Infinity %s Cell".to_owned()),
            "block.minecraft.water" => Some("Water".to_owned()),
            _ => None,
        };
        let item = |id: &str| if id == "minecraft:cobblestone" { "Cobblestone".to_owned() } else { id.to_owned() };
        assert_eq!(infinity_cell_name(INFINITY_WATER_CELL, &translate, &item).as_deref(), Some("ME Infinity Water Cell"));
        assert_eq!(infinity_cell_name(INFINITY_COBBLESTONE_CELL, &translate, &item).as_deref(), Some("ME Infinity Cobblestone Cell"));
        let defs = crate::ae2x::defs::get();
        let void = defs.handler_cell(VOID_CELL).unwrap();
        assert_eq!((void.idle_drain, void.config_slots, defs.upgrade_slots(VOID_CELL)), (1.0, 63, Some(2)));
        assert_eq!(defs.upgrade_limit("ae2:fuzzy_card", VOID_CELL), 1);
        assert_eq!(defs.upgrade_limit("ae2:inverter_card", VOID_CELL), 1);
        assert_eq!(defs.upgrade_limit("ae2:void_card", VOID_CELL), 0);
    }
}
