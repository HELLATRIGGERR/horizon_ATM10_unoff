//! ExtendedAE 1.21-2.2.42's rows in the definitions registry (feature
//! `ae2-extendedae`). Derived from ExtendedAE (`TileExPatternProvider`,
//! `PartExPatternProvider`, `TileExInterface`, `PartExInterface`,
//! `TileExDrive`, `TileExIOPort`, `TileCrystalAssembler`, `TileCircuitCutter`,
//! `TileCrystalFixer`, `TileExInscriber`, `TileExCharger`,
//! `TileExMolecularAssembler`, `TileWirelessConnector`, `TileWirelessHub`,
//! `WirelessConnect`, `EAERegistryHandler.registerUpgrades`, `MEGACommonLoad`, `EAEConfig`),
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
use ae2_cell_core::KeyType;
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
/// `TileWirelessConnector`: one wireless link, four energy cards.
pub const WIRELESS_CONNECTOR: &str = "extendedae:wireless_connect";
/// `TileWirelessHub`: [`HUB_PORTS`] links.
pub const WIRELESS_HUB: &str = "extendedae:wireless_hub";
/// `ItemWirelessConnectTool`, the Wireless Setup Kit.
pub const WIRELESS_TOOL: &str = "extendedae:wireless_tool";
/// `ItemWirelessConnectorUpgrade`: a connector into a hub.
pub const WIRELESS_HUB_KIT: &str = "extendedae:wireless_connector_upgrade";
/// `EAESingletons.WIRELESS_LOCATOR`: the kit's bound frequency and place,
/// `{a: long, b: {dimension, pos}}` (`GlodCodecs.pair`).
pub const WIRELESS_LOCATOR: &str = "extendedae:wireless_locator";
/// `TileWirelessHub.MAX_PORT`.
pub const HUB_PORTS: usize = 8;
/// The idle draw a wireless block's tick sets (`powerUse`, which the mod
/// does not save), in the machine's config for its grid node.
pub const WIRELESS_POWER: &str = "eae_wireless_power";

/// The links a wireless block has: one for the connector, eight for the hub.
pub fn wireless_ports(id: &str) -> Option<usize> {
    match id {
        WIRELESS_CONNECTOR => Some(1),
        WIRELESS_HUB => Some(HUB_PORTS),
        _ => None,
    }
}

/// The frequencies of a wireless block's links, from its saved record:
/// `freq` for the connector; `freq0` to `freq7` for the hub, or `freq` for
/// its first link alone when the record has it (`TileWirelessHub.loadTag`:
/// a connector the kit turned into a hub).
pub fn wireless_frequencies(id: &str, config: &horizon_world::nbt::Compound) -> Vec<i64> {
    use horizon_world::nbt::Tag;
    let long = |key: &str| match config.get(&key.into()) {
        Some(Tag::Long(v)) => *v,
        Some(Tag::Int(v)) => i64::from(*v),
        _ => 0,
    };
    match wireless_ports(id) {
        Some(1) => vec![long("freq")],
        Some(n) if config.contains_key(&"freq".into()) => {
            let mut all = vec![0; n];
            all[0] = long("freq");
            all
        }
        Some(n) => (0..n).map(|i| long(&format!("freq{i}"))).collect(),
        None => Vec::new(),
    }
}
/// Sets link `port`'s frequency (`setFrequency`), in the record's names
/// (`saveAdditional`: the hub writes all eight and no `freq`).
pub fn set_wireless_frequency(id: &str, config: &mut horizon_world::nbt::Compound, port: usize, frequency: i64) {
    use horizon_world::nbt::Tag;
    let mut all = wireless_frequencies(id, config);
    let Some(slot) = all.get_mut(port) else { return };
    *slot = frequency;
    if all.len() == 1 {
        config.insert("freq".into(), Tag::Long(frequency));
        return;
    }
    config.remove(&"freq".into());
    for (i, f) in all.into_iter().enumerate() {
        config.insert(format!("freq{i}").as_str().into(), Tag::Long(f));
    }
}

/// `updatePowerUsage` of one link: `max(1, d ln d (1 - discount))` times the
/// configured multiplier for a link `d` blocks long (at least `e`). Each
/// energy card takes 10% off, MEGA's Greater Energy Card 20% (`calculateDisc`).
pub fn wireless_link_power(distance: f64, energy_cards: usize, greater_cards: usize, multiplier: f64) -> f64 {
    let discount = 1.0 - (0.1 * energy_cards as f64 + 0.2 * greater_cards as f64);
    let d = distance.max(std::f64::consts::E);
    (d * d.ln() * discount).max(1.0) * multiplier
}
/// A wireless block's idle draw: its links' together, or the multiplier when
/// none is up (`TileWirelessHub.updatePowerUsage`; the connector's one link).
pub fn wireless_power(distances: &[f64], energy_cards: usize, greater_cards: usize, multiplier: f64) -> f64 {
    if distances.is_empty() {
        return multiplier;
    }
    distances.iter().map(|d| wireless_link_power(*d, energy_cards, greater_cards, multiplier)).sum()
}

/// One link of a wireless block, as `WirelessConnect.updateStatus` sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WirelessEnd {
    /// The level, by any number the caller keeps apart.
    pub level: i32,
    pub pos: (i32, i32, i32),
    pub port: usize,
    pub frequency: i64,
    /// When it registered (`CONNECTORS.register`): of the ends of one
    /// frequency, the first two pair.
    pub order: u64,
}
/// The links `WirelessConnect.updateStatus` makes: in each level, the first
/// two ends of a frequency (a third finds both of its keys taken), when no
/// further apart than `range` (`Math.sqrt(distSqr)`) and not the same block.
/// Each as the two ends' indices and the distance.
pub fn wireless_links(ends: &[WirelessEnd], range: f64) -> Vec<(usize, usize, f64)> {
    let mut by_frequency = std::collections::BTreeMap::<(i32, i64), Vec<usize>>::new();
    for (i, end) in ends.iter().enumerate().filter(|(_, e)| e.frequency != 0) {
        by_frequency.entry((end.level, end.frequency)).or_default().push(i);
    }
    let mut links = Vec::new();
    for mut group in by_frequency.into_values() {
        group.sort_by_key(|i| (ends[*i].order, *i));
        let (a, b) = match group.as_slice() {
            [a, b, ..] => (*a, *b),
            _ => continue,
        };
        let (p, q) = (ends[a].pos, ends[b].pos);
        if p == q {
            continue;
        }
        let d = (f64::from(p.0 - q.0).powi(2) + f64::from(p.1 - q.1).powi(2) + f64::from(p.2 - q.2).powi(2)).sqrt();
        if d <= range {
            links.push((a, b, d));
        }
    }
    links
}

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
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// `device.extended_io_bus_multiplier`, 2 to 128: the Extended Import and
    /// Export Buses' operations, times AE2's.
    pub bus_speed: u64,
    /// `item.infinity_cell_energy_cost`, 0.1 to 64: an infinity cell's AE/t.
    pub infinity_cell_cost: f64,
    /// `device.assembler_matrix_max_size`, 3 to 16: how far the Assembler
    /// Matrix's corners may lie apart on each axis (`checkMultiblockScale`).
    pub matrix_size: i32,
    /// `device.wireless_connector_max_range`, 10 to 10,000: the longest
    /// wireless link.
    pub wireless_range: f64,
    /// `device.wireless_connector_power_multiplier`, 0 to 100.
    pub wireless_power: f64,
}
impl Default for Config {
    fn default() -> Self {
        Self { bus_speed: 8, infinity_cell_cost: 8.0, matrix_size: 6, wireless_range: 1000.0, wireless_power: 1.0 }
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
        // A DoubleValue reads a whole number too.
        let float = |section: &str, key: &str| {
            let v = value.get(section)?.get(key)?;
            v.as_float().or_else(|| v.as_integer().map(|n| n as f64))
        };
        if let Some(n) = float("device", "wireless_connector_max_range").filter(|n| (10.0..=10000.0).contains(n)) {
            config.wireless_range = n;
        }
        if let Some(n) = float("device", "wireless_connector_power_multiplier").filter(|n| (0.0..=100.0).contains(n)) {
            config.wireless_power = n;
        }
        Ok(config)
    }
    /// The game's `config/extendedae-common.toml`, read once (`ModConfigEvent`).
    pub fn get() -> &'static Self {
        static CONFIG: OnceLock<Config> = OnceLock::new();
        CONFIG.get_or_init(|| {
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
        // TileWirelessConnector and TileWirelessHub do not save powerUse.
        Some(WIRELESS_CONNECTOR | WIRELESS_HUB) => {
            m.config.remove(&WIRELESS_POWER.into());
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
    // UpgradeInventories.forMachine(WIRELESS_CONNECTOR, 4); the hub's the same.
    ExtMachine { id: WIRELESS_CONNECTOR, physical_slots: 0, upgrade_slots: 4, key_drops: None, power_capacity: None, part: false },
    ExtMachine { id: WIRELESS_HUB, physical_slots: 0, upgrade_slots: 4, key_drops: None, power_capacity: None, part: false },
];

/// An `ItemInfinityCell` (`InfinityCellInventory`): the one key it holds
/// (`getRecord`), an item or a fluid, and the model a drive draws for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InfinityCell {
    pub id: String,
    /// The record's key type (`getRecord().what().getType()`).
    pub key_type: KeyType,
    pub record: String,
    pub drive_model: String,
}
/// The mod's two (`EAESingletons`, `registerStorageHandler`).
fn own_infinity_cells() -> [InfinityCell; 2] {
    let cell = |id: &str, key_type: KeyType, record: &str, model: &str| InfinityCell { id: id.into(), key_type, record: record.into(), drive_model: model.into() };
    [
        cell(INFINITY_WATER_CELL, KeyType::FLUIDS, "minecraft:water", "extendedae:block/drive/infinity_water_cell"),
        cell(INFINITY_COBBLESTONE_CELL, KeyType::ITEMS, "minecraft:cobblestone", "extendedae:block/drive/infinity_cobblestone_cell"),
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
            let (key_type, record) = match (row.get("item").and_then(|v| v.as_str()), row.get("fluid").and_then(|v| v.as_str())) {
                (Some(item), None) => (KeyType::ITEMS, item),
                (None, Some(fluid)) => (KeyType::FLUIDS, fluid),
                _ => return None,
            };
            // A cell with no model draws AE2's generic one (`StorageCellModels`).
            let drive_model = row.get("model").and_then(|v| v.as_str()).unwrap_or("ae2:block/drive/drive_cell").to_owned();
            Some(InfinityCell { id, key_type, record: record.to_owned(), drive_model })
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
    let record = if cell.key_type == KeyType::FLUIDS {
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
    for host in [WIRELESS_CONNECTOR, WIRELESS_HUB] {
        defs.add_upgrade("ae2:energy_card", host, 4);
        // MEGACommonLoad.init, with MEGA there.
        #[cfg(feature = "ae2-megacells")]
        defs.add_upgrade(super::megacells::GREATER_ENERGY_CARD, host, 4);
    }
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
        assert_eq!((water.key_type, water.record.as_str()), (KeyType::FLUIDS, "minecraft:water"));
        let cobble = infinity_cell(INFINITY_COBBLESTONE_CELL).unwrap();
        assert_eq!((cobble.key_type, cobble.record.as_str()), (KeyType::ITEMS, "minecraft:cobblestone"));
        assert_eq!(infinity_cell_model(INFINITY_COBBLESTONE_CELL), Some("extendedae:block/drive/infinity_cobblestone_cell"));
        assert!(infinity_cell("ae2:item_storage_cell_1k").is_none());
        let pack = parse_pack_cells(r#"[{"id":"kubejs:lava_cell","fluid":"minecraft:lava","model":"kubejs:block/drive/lava_cell"},{"id":"kubejs:dirt_cell","item":"minecraft:dirt"},{"id":"kubejs:broken"}]"#).unwrap();
        assert_eq!(pack.len(), 2);
        assert_eq!((pack[0].key_type, pack[0].record.as_str(), pack[0].drive_model.as_str()), (KeyType::FLUIDS, "minecraft:lava", "kubejs:block/drive/lava_cell"));
        assert_eq!((pack[1].key_type, pack[1].drive_model.as_str()), (KeyType::ITEMS, "ae2:block/drive/drive_cell"));
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

    #[test]
    fn wireless_blocks_link_their_first_two_ends_within_range() {
        use super::*;
        crate::ae2x::install();
        let defs = crate::ae2x::defs::get();
        for id in [WIRELESS_CONNECTOR, WIRELESS_HUB] {
            let machine = Machine::for_block(id).unwrap();
            assert_eq!((machine.slots.len(), machine.upgrades.len(), machine.id()), (0, 4, id));
            assert_eq!(defs.upgrade_limit("ae2:energy_card", id), 4);
            assert_eq!(defs.upgrade_limit("ae2:speed_card", id), 0);
        }
        #[cfg(feature = "ae2-megacells")]
        assert_eq!(defs.upgrade_limit(crate::ae2x::megacells::GREATER_ENERGY_CARD, WIRELESS_HUB), 4);
        let end = |level, x: i32, port, frequency, order| WirelessEnd { level, pos: (x, 64, 0), port, frequency, order };
        // 900 blocks: linked. A third end of the frequency finds both keys taken;
        // 1,001 blocks is out of range; another level is apart; 0 is no frequency.
        let ends = [
            end(0, 0, 0, 7, 0),
            end(0, 900, 0, 7, 1),
            end(0, 5, 0, 7, 2),
            end(0, 0, 1, 9, 3),
            end(0, 1001, 0, 9, 4),
            end(1, 0, 0, 11, 5),
            end(0, 3, 0, 11, 6),
            end(0, 1, 0, 0, 7),
            end(0, 2, 0, 0, 8),
        ];
        assert_eq!(wireless_links(&ends, 1000.0), vec![(0, 1, 900.0)]);
        // The registration order picks the pair, not the place; exactly the range still links.
        assert_eq!(wireless_links(&[end(0, 0, 0, 3, 5), end(0, 9, 0, 3, 1), end(0, 1000, 0, 3, 2)], 1000.0), vec![(1, 2, 991.0)]);
        assert_eq!(wireless_links(&[end(0, 0, 0, 3, 0), end(0, 1000, 0, 3, 1)], 1000.0).len(), 1);
        // A hub's two ports of one frequency: the same block, no link.
        assert!(wireless_links(&[end(0, 4, 0, 5, 0), end(0, 4, 1, 5, 1)], 1000.0).is_empty());

        // max(1, d ln d), at least e long; 10% off a card, 20% a greater card; the multiplier unlinked.
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        assert!(close(wireless_link_power(900.0, 0, 0, 1.0), 900.0 * 900f64.ln()));
        assert!(close(wireless_link_power(1.0, 0, 0, 1.0), std::f64::consts::E));
        assert!(close(wireless_link_power(900.0, 4, 0, 1.0), 900.0 * 900f64.ln() * 0.6));
        assert!(close(wireless_link_power(900.0, 0, 4, 2.0), 900.0 * 900f64.ln() * 0.2 * 2.0));
        assert!(close(wireless_link_power(2.0, 0, 4, 1.0), 1.0));
        assert_eq!(wireless_power(&[], 4, 0, 1.5), 1.5);
        assert!(close(wireless_power(&[10.0, 20.0], 0, 0, 1.0), 10.0 * 10f64.ln() + 20.0 * 20f64.ln()));

        // The records' names: a connector's `freq`; a hub's eight, or a connector's `freq` on its first.
        use horizon_world::nbt::{Compound, Tag};
        let mut connector = Compound::new();
        set_wireless_frequency(WIRELESS_CONNECTOR, &mut connector, 0, 42);
        assert_eq!(connector.get(&"freq".into()), Some(&Tag::Long(42)));
        assert_eq!(wireless_frequencies(WIRELESS_CONNECTOR, &connector), vec![42]);
        let mut hub = connector.clone();
        assert_eq!(wireless_frequencies(WIRELESS_HUB, &hub), vec![42, 0, 0, 0, 0, 0, 0, 0]);
        set_wireless_frequency(WIRELESS_HUB, &mut hub, 3, -5);
        assert!(!hub.contains_key(&"freq".into()));
        assert_eq!(wireless_frequencies(WIRELESS_HUB, &hub), vec![42, 0, 0, -5, 0, 0, 0, 0]);
        assert_eq!(hub.get(&"freq7".into()), Some(&Tag::Long(0)));

        assert_eq!((Config::default().wireless_range, Config::default().wireless_power), (1000.0, 1.0));
        let config = Config::parse("[device]\nwireless_connector_max_range = 2000\nwireless_connector_power_multiplier = 0.5").unwrap();
        assert_eq!((config.wireless_range, config.wireless_power), (2000.0, 0.5));
        assert_eq!(Config::parse("[device]\nwireless_connector_max_range = 5.0").unwrap().wireless_range, 1000.0);
    }
}
