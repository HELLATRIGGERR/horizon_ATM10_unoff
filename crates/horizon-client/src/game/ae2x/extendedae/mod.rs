//! ExtendedAE 1.21-2.2.42 in the game (feature `ae2-extendedae`). Its bigger
//! devices are AE2's with other sizes (rows in the library's
//! `ae2x::extendedae`); their screens are layouts of `ae2_screens`
//! (`screens.rs`, a child module of it). Its own machines are in `machines.rs`.
//! What needs the game's own code is here, reached through `game/ae2x/mod.rs`.
pub(in crate::game) mod machines;
pub(in crate::game) mod upgrade;

use horizon_client::{ae2_world::{Machine, MachineKind}, items::ItemStack};
use horizon_core::geometry::Direction;
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
        _ => None,
    }
}

/// The orientation strategy of the mod's own blocks: `full()` for the
/// Crystal Assembler and Circuit Cutter, `facing()` for the Crystal Fixer.
pub(in crate::game) fn orientation(block: &str) -> Option<super::Orientation> {
    use horizon_client::ae2x::extendedae as rows;
    match block {
        rows::CRYSTAL_ASSEMBLER | rows::CIRCUIT_CUTTER => Some(super::Orientation::FacingWithSpin),
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

/// The node of the mod's own machines: no channel, no idle draw
/// (`setFlags()`, `setIdlePowerUsage(0)`), on their connectable sides.
pub(in crate::game) fn device(machine: &Machine, properties: Option<&horizon_world::block_state_properties::BlockProperties>, device: &mut ae2_network_core::Device) -> bool {
    let Some(kind) = Kind::of(machine) else { return false };
    device.idle_override = Some(0.0);
    device.channel_override = Some(false);
    let facing = properties.and_then(|p| p.get("facing")).map_or("north", String::as_str);
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
    device.faces = connectable(kind, facing, spin).into_iter().fold(0, |faces, d| faces | 1 << bit(d));
    true
}
/// Forge Energy goes into the machine's own buffer on its power sides.
pub(in crate::game) fn private_power_face(kind: MachineKind, state: &horizon_world::chunk::BlockState, face: Direction) -> bool {
    let Some(kind) = kind_of(kind) else { return false };
    let facing = state.properties.get("facing").map_or("north", String::as_str);
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

/// The Crystal Assembler and Circuit Cutter open AE2-style machine menus
/// (`UpgradeableMenu`); the Crystal Fixer has none.
pub(in crate::game) fn machine_menu(m: &Machine) -> bool {
    matches!(Kind::of(m), Some(Kind::Assembler | Kind::Cutter))
}
/// What a menu slot may take: anything in an input (`AppEngSlot`), nothing
/// in the output (`OutputSlot`).
pub(in crate::game) fn may_place(m: &Machine, slot: usize) -> Option<bool> {
    let kind = Kind::of(m)?;
    Some(slot < kind.inputs())
}
/// `invExposed`: inputs insert-only, the output extract-only; the fixer's
/// fuel both ways.
pub(in crate::game) fn slot_insert(kind: MachineKind, index: usize) -> Option<bool> {
    let kind = kind_of(kind)?;
    Some(index < kind.inputs())
}
pub(in crate::game) fn slot_extract(kind: MachineKind, index: usize) -> Option<bool> {
    let kind = kind_of(kind)?;
    Some(kind == Kind::Fixer || index >= kind.inputs())
}
/// The menu's progress bar (`IProgressProvider`): processingTime of MAX_PROGRESS.
pub(in crate::game) fn progress(m: &Machine) -> Option<(f64, f64)> {
    let kind = Kind::of(m).filter(|k| *k != Kind::Fixer)?;
    Some((m.progress.min(kind.max_progress()), kind.max_progress()))
}
/// Entries the window shows past the machine's slots: the tank.
pub(in crate::game) fn menu_items(m: &Machine, items: &mut [Option<ItemStack>]) {
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
