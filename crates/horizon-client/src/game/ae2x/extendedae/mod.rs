//! ExtendedAE 1.21-2.2.42 in the game (feature `ae2-extendedae`). Its bigger
//! devices are AE2's with other sizes (rows in the library's
//! `ae2x::extendedae`); their screens are layouts of `ae2_screens`
//! (`screens.rs`, a child module of it). Its own machines are in `machines.rs`,
//! the Extended Inscriber, Charger and Molecular Assembler in `devices.rs`,
//! Entro's growth in `entro.rs`, the Infinity and Void Cells in `cells.rs`,
//! the Assembler Matrix in `matrix.rs`, the Wireless Connector and Hub in
//! `wireless.rs`, the Pattern and Config Modifiers in `tools.rs`, ME Packing
//! Tape in `tape.rs`.
//! What needs the game's own code is here, reached through `game/ae2x/mod.rs`.
pub(in crate::game) mod cells;
pub(in crate::game) mod devices;
pub(in crate::game) mod entro;
pub(in crate::game) mod interfaces;
pub(in crate::game) mod machines;
pub(in crate::game) mod matrix;
pub(in crate::game) mod special;
pub(in crate::game) mod tape;
pub(in crate::game) mod tools;
pub(in crate::game) mod upgrade;
pub(in crate::game) mod wireless;

use horizon_client::{ae2_world::{Machine, MachineKind}, items::ItemStack};
use horizon_core::geometry::Direction;
use devices::Device;
use machines::Kind;
pub(in crate::game) use machines::State;

/// The Crystal Assembler's tank in its window (`SlotSemantics.STORAGE`).
pub(in crate::game) const TANK_ENTRY: usize = 20;
/// The AUTO_EXPORT toggle's entry, as the Inscriber's.
pub(in crate::game) const AUTO_EXPORT_ENTRY: usize = 24;
/// The entries of the output side buttons (`OutputSideConfig`), in Direction order.
pub(in crate::game) const SIDE_ENTRIES: std::ops::Range<usize> = 14..20;

/// `PartExPatternProvider`, `PartExInterface` and the Extended Import and
/// Export Buses: their base models.
pub(in crate::game) fn part_model(id: &str) -> Option<&'static str> {
    match id {
        "extendedae:ex_pattern_provider_part" => Some("extendedae:part/ex_pattern_provider_base"),
        "extendedae:ex_interface_part" => Some("extendedae:part/ex_interface"),
        "extendedae:ex_import_bus_part" => Some("extendedae:part/ex_import_bus_base"),
        "extendedae:ex_export_bus_part" => Some("extendedae:part/ex_export_bus_base"),
        // Slice 5's buses (`MODEL_BASE`); their status models are AE2's.
        "extendedae:tag_storage_bus" => Some("extendedae:part/tag_storage_bus_base"),
        "extendedae:tag_export_bus" => Some("extendedae:part/tag_export_bus_base"),
        "extendedae:mod_storage_bus" => Some("extendedae:part/mod_storage_bus_base"),
        "extendedae:mod_export_bus" => Some("extendedae:part/mod_export_bus_base"),
        "extendedae:precise_storage_bus" => Some("extendedae:part/precise_storage_bus_base"),
        "extendedae:precise_export_bus" => Some("extendedae:part/precise_export_bus_base"),
        "extendedae:threshold_export_bus" => Some("extendedae:part/threshold_export_bus_base"),
        "extendedae:oversize_interface_part" => Some("extendedae:part/oversize_interface"),
        _ => None,
    }
}

/// `PartExPatternAccessTerminal` and `ItemWirelessExPAT`: AE2's Pattern
/// Access Terminal with `ContainerExPatternTerminal.quickMovePattern`.
pub(in crate::game) fn pattern_terminal_extended(id: &str) -> bool {
    use horizon_client::ae2x::extendedae as rows;
    id == rows::EX_PATTERN_TERMINAL || id == rows::WIRELESS_EX_PAT
}
/// `ContainerExPatternTerminal.quickMovePattern`'s targets: a pattern the
/// Molecular Assembler crafts (`IMolecularAssemblerSupportedPattern`:
/// crafting, smithing table, stonecutting) goes to a group an assembler
/// shows (AE2's, the Extended Molecular Assembler, the Assembler Matrix's
/// pattern core), any other to the rest. Unlike AE2's, the targets need not
/// share a group.
pub(in crate::game) fn pattern_quick_move_target(pattern: &ItemStack, icon: &ae2_cell_core::ItemKey) -> bool {
    use horizon_client::ae2x::extendedae as rows;
    let assembler_pattern = crate::game::ae2_processing::decode_pattern(pattern).is_some() || crate::game::ae2_processing::ae2_decode_special_pattern(pattern).is_some();
    let assembler = matches!(icon.item_id.as_str(), "ae2:molecular_assembler" | rows::MATRIX_PATTERN | rows::EX_ASSEMBLER);
    assembler_pattern == assembler
}
/// `PartExPatternAccessTerminal.MODELS`: its screen over AE2's display base.
pub(in crate::game) fn display_part_screen(id: &str) -> Option<&'static str> {
    (id == horizon_client::ae2x::extendedae::EX_PATTERN_TERMINAL).then_some("extendedae:part/ex_pattern_access_terminal")
}

/// `PartExImportBus` and `PartExExportBus` open `ContainerExIOBus` with the
/// styles `ex_import_bus.json` and `ex_export_bus.json` (over AE2's `io_bus.json`).
pub(in crate::game) fn part_style(id: &str) -> Option<(&'static str, &'static str)> {
    match id {
        "extendedae:ex_import_bus_part" => Some(("extendedae", "ex_import_bus.json")),
        "extendedae:ex_export_bus_part" => Some(("extendedae", "ex_export_bus.json")),
        _ => None,
    }
}

/// The orientation strategy of the mod's own blocks: `full()` for the
/// Crystal Assembler, Circuit Cutter and the Extended Inscriber and Charger,
/// `facing()` for the Crystal Fixer.
pub(in crate::game) fn orientation(block: &str) -> Option<super::Orientation> {
    use horizon_client::ae2x::extendedae as rows;
    match block {
        rows::CRYSTAL_ASSEMBLER | rows::CIRCUIT_CUTTER | rows::EX_INSCRIBER | rows::EX_CHARGER => Some(super::Orientation::FacingWithSpin),
        rows::CRYSTAL_FIXER => Some(super::Orientation::Facing),
        _ => None,
    }
}

fn kind_of(kind: MachineKind) -> Option<Kind> {
    use horizon_client::ae2x::extendedae as rows;
    Some(match kind.ext()?.id {
        rows::CRYSTAL_ASSEMBLER => Kind::Assembler,
        rows::CIRCUIT_CUTTER => Kind::Cutter,
        rows::CRYSTAL_FIXER => Kind::Fixer,
        _ => return None,
    })
}
fn direction(name: &str) -> Option<Direction> {
    Direction::ALL.into_iter().find(|d| machines::direction_name(*d) == name)
}
/// `getGridConnectableSides`, which are also its power sides: all but the
/// top for the Crystal Assembler, all but front and back for the Circuit
/// Cutter, only the back for the Crystal Fixer.
fn connectable(kind: Kind, facing: &str, spin: usize) -> Vec<Direction> {
    let front = direction(facing).unwrap_or(Direction::North);
    let closed: Vec<Direction> = match kind {
        Kind::Assembler => super::super::machines::ae2_capabilities::orientation_up(facing, spin).into_iter().collect(),
        Kind::Cutter => vec![front, front.opposite()],
        Kind::Fixer => return vec![front.opposite()],
    };
    Direction::ALL.into_iter().filter(|d| !closed.contains(d)).collect()
}

/// `getGridConnectableSides` of the Extended Inscriber and Charger: all but
/// their front.
fn device_connectable(facing: &str) -> Vec<Direction> {
    let front = direction(facing).unwrap_or(Direction::North);
    Direction::ALL.into_iter().filter(|d| *d != front).collect()
}
/// The node of the mod's own machines: no channel, no idle draw
/// (`setFlags()`, `setIdlePowerUsage(0)`; the Extended Inscriber sets no flags
/// either), on their connectable sides.
pub(in crate::game) fn device(machine: &Machine, properties: Option<&horizon_world::block_state_properties::BlockProperties>, device: &mut ae2_network_core::Device) -> bool {
    if wireless::device(machine, device) || interfaces::device(machine, device) {
        return true;
    }
    // The Assembler Matrix's blocks (`TileAssemblerMatrixBase`): MULTIBLOCK
    // and REQUIRE_CHANNEL, one channel for the formed cluster; every side
    // while formed (its block state says so), none before.
    if let Some(role) = horizon_client::ae2x::extendedae::MatrixRole::of(machine.id()) {
        let formed = properties.and_then(|p| p.get("formed")).is_some_and(|v| v == "true");
        device.faces = if formed { 63 } else { 0 };
        device.idle_override = Some(role.idle_power());
        device.channel_override = Some(true);
        device.multiblock = if formed { matrix::group(machine) } else { None };
        return true;
    }
    let facing = properties.and_then(|p| p.get("facing")).map_or("north", String::as_str);
    let kind = Kind::of(machine);
    if kind.is_none() && Device::of(machine).is_none() {
        return false;
    }
    device.idle_override = Some(0.0);
    device.channel_override = Some(false);
    let spin = properties.and_then(|p| p.get("spin")).and_then(|s| s.parse().ok()).unwrap_or(0);
    // The node's face bits: east, west, up, down, south, north.
    let bit = |d: Direction| match d {
        Direction::East => 0,
        Direction::West => 1,
        Direction::Up => 2,
        Direction::Down => 3,
        Direction::South => 4,
        Direction::North => 5,
    };
    let sides = match kind {
        Some(kind) => connectable(kind, facing, spin),
        None => device_connectable(facing),
    };
    device.faces = sides.into_iter().fold(0, |faces, d| faces | 1 << bit(d));
    true
}
/// Forge Energy goes into the machine's own buffer on its power sides.
pub(in crate::game) fn private_power_face(kind: MachineKind, state: &horizon_world::chunk::BlockState, face: Direction) -> bool {
    let facing = state.properties.get("facing").map_or("north", String::as_str);
    if Device::of_kind(kind).is_some() {
        // setPowerSides(getGridConnectableSides).
        return device_connectable(facing).contains(&face);
    }
    let Some(kind) = kind_of(kind) else { return false };
    let spin = state.properties.get("spin").and_then(|s| s.parse().ok()).unwrap_or(0);
    connectable(kind, facing, spin).contains(&face)
}

/// `updateBlockStateFromBlockEntity`: `working` while a recipe runs.
pub(in crate::game) fn visual_props(machine: &Machine, props: &mut horizon_world::block_state_properties::BlockProperties) -> bool {
    if Kind::of(machine).is_none() || !props.contains_key("working") {
        return false;
    }
    let working = matches!(machine.config.get(&machines::WORKING.into()), Some(horizon_world::nbt::Tag::Byte(1)));
    props.insert("working".into(), working.to_string());
    true
}

/// The Crystal Assembler, Circuit Cutter and Extended Inscriber open AE2-style
/// machine menus (`UpgradeableMenu`); the Crystal Fixer and Extended Charger
/// have none (a use refuels or loads them).
pub(in crate::game) fn machine_menu(m: &Machine) -> bool {
    matches!(Kind::of(m), Some(Kind::Assembler | Kind::Cutter)) || Device::of(m) == Some(Device::Inscriber) || horizon_client::ae2x::extendedae::wireless_ports(m.id()).is_some()
}
/// What a menu slot may take: anything in an input (`AppEngSlot`), nothing
/// in the output (`OutputSlot`).
pub(in crate::game) fn may_place(m: &Machine, slot: usize) -> Option<bool> {
    let kind = Kind::of(m)?;
    Some(slot < kind.inputs())
}
/// `invExposed`: inputs insert-only, the output extract-only; the fixer's
/// fuel both ways. The Extended Inscriber's and Charger's filters are their
/// item handlers' (`devices.rs`), which automation meets first.
pub(in crate::game) fn slot_insert(kind: MachineKind, index: usize) -> Option<bool> {
    if Device::of_kind(kind).is_some() {
        return Some(true);
    }
    let kind = kind_of(kind)?;
    Some(index < kind.inputs())
}
pub(in crate::game) fn slot_extract(kind: MachineKind, index: usize) -> Option<bool> {
    if Device::of_kind(kind).is_some() {
        return Some(true);
    }
    let kind = kind_of(kind)?;
    Some(kind == Kind::Fixer || index >= kind.inputs())
}
/// The most a menu slot holds: the Extended Inscriber's inputs its stack
/// size, the Extended Charger's one.
pub(in crate::game) fn slot_capacity(m: &Machine, slot: usize) -> Option<u32> {
    match Device::of(m)? {
        Device::Inscriber => Some(if slot % devices::THREAD_SLOTS < 3 { devices::stack_size(m) } else { 64 }),
        Device::Charger => Some(1),
    }
}
/// An assembler's progress a tick and AE a point: the Extended Molecular
/// Assembler's by its cards, a crafter core's by its matrix's speed cores.
pub(in crate::game) fn assembler_speed(m: &Machine) -> Option<(f64, f64)> {
    devices::assembler_speed(m).or_else(|| matrix::crafter_speed(m))
}
/// `TileAssemblerMatrixPattern.Filter` on a pattern core's patterns; none
/// for any other provider.
pub(in crate::game) fn pattern_allowed(machine: &Machine, item: &ItemStack) -> Option<bool> {
    (machine.id() == horizon_client::ae2x::extendedae::MATRIX_PATTERN).then(|| matrix::pattern_allowed(item))
}
/// The crafting threads of the Extended Molecular Assembler and of a crafter core.
pub(in crate::game) fn assembler_threads(m: &Machine) -> Option<usize> {
    devices::assembler_threads(m).or_else(|| (m.id() == horizon_client::ae2x::extendedae::MATRIX_CRAFTER).then_some(horizon_client::ae2x::extendedae::ASSEMBLER_THREADS))
}
/// The menu's progress bar (`IProgressProvider`): processingTime of
/// MAX_PROGRESS; the Extended Inscriber's of the thread shown, of 200.
pub(in crate::game) fn progress(m: &Machine) -> Option<(f64, f64)> {
    if Device::of(m) == Some(Device::Inscriber) {
        return Some((devices::thread(m, devices::page(m)).progress.min(200.0), 200.0));
    }
    let kind = Kind::of(m).filter(|k| *k != Kind::Fixer)?;
    Some((m.progress.min(kind.max_progress()), kind.max_progress()))
}
/// Entries the window shows past the machine's slots: the tank; the
/// Extended Inscriber's shown thread.
pub(in crate::game) fn menu_items(m: &Machine, items: &mut [Option<ItemStack>]) {
    if Device::of(m) == Some(Device::Inscriber) {
        devices::inscriber_menu_items(m, items);
        return;
    }
    if Kind::of(m) != Some(Kind::Assembler) {
        return;
    }
    if let Some(slot) = items.get_mut(TANK_ENTRY) {
        *slot = machines::tank(m).map(|(fluid, amount)| {
            let bucket = horizon_client::ae2_cell_codec::fluid_bucket(&fluid).unwrap_or("minecraft:bucket");
            let mut icon = ItemStack::new(bucket, 1);
            icon.data_mut().custom_name = Some(horizon_client::item_data::text_name(format!("{fluid}: {amount} / {} mB", machines::TANK_CAP)));
            icon
        });
    }
}
/// The output side button at window entry `entry`: its side.
pub(in crate::game) fn side_entry(entry: usize) -> Option<Direction> {
    SIDE_ENTRIES.contains(&entry).then(|| Direction::ALL[entry - SIDE_ENTRIES.start])
}
