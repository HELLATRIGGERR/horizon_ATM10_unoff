//! Applied Flux's Flux Accessor, block and part, in the game
//! (`TileFluxAccessor`, `PartFluxAccessor`, `NetworkFEPower`,
//! `EnergyHandler`, `EnergyDistributeService`): the grid's FE storage as the
//! Forge Energy of the accessor, on all its sides (the block) or on the side
//! of the cable bus it sits on (the part); and while its node is active and
//! its IO mode lets FE out, the FE it pushes into the FE blocks beside it,
//! at the pace of each side's `EnergyTickRecord`. Derived from Applied Flux
//! by GlodBlock, https://github.com/GlodBlock/ExtendedAE branch
//! `appflux/1.21.1-neoforge` commit `523c3e5a`, LGPL-3.0; see
//! `ae2/addons/appflux/`.
//!
//! The FE dispatcher reaches the accessor through AE2's own adapters
//! (`energy_extras.rs`, `ae2_power.rs`), which ask the addons first
//! (`Game::ae2x_fe_storage`, `Game::ae2x_fe_move`).
use super::super::super::{Game, ae2_global::GridAddress, ae2_parts::PartAddress, ae2_power::FeStorage};
use horizon_client::{
    ae2_world::{Machine, MachineKind},
    ae2x::appflux::{self as rows, EnergyIo, TickRecord},
    containers::BlockEntity,
};
use horizon_core::geometry::Direction;
use horizon_world::nbt::Tag;
use std::collections::HashMap;

/// The pace of each pushing side, by the accessor's position and the side
/// (`lastTick`); none of it is saved.
#[derive(Default)]
pub(in crate::game) struct State {
    ticks: HashMap<((i32, i32, i32), u8), TickRecord>,
}

/// AE2's part faces: east, west, up, down, south, north.
const DIRECTIONS: [Direction; 6] = [Direction::East, Direction::West, Direction::Up, Direction::Down, Direction::South, Direction::North];
fn face_of(direction: Direction) -> u8 {
    DIRECTIONS.iter().position(|d| *d == direction).unwrap_or(0) as u8
}
fn offset(direction: Direction) -> [i32; 3] {
    match direction {
        Direction::East => [1, 0, 0],
        Direction::West => [-1, 0, 0],
        Direction::Up => [0, 1, 0],
        Direction::Down => [0, -1, 0],
        Direction::South => [0, 0, 1],
        Direction::North => [0, 0, -1],
    }
}
fn opposite(direction: Direction) -> Direction {
    DIRECTIONS[usize::from(face_of(direction) ^ 1)]
}

/// The accessor's saved settings (`loadTag` / `readFromNBT`): `fast`, `io`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::game) struct Settings {
    pub fast: bool,
    pub io: EnergyIo,
}
impl Settings {
    pub(in crate::game) fn of(machine: &Machine) -> Self {
        let fast = matches!(machine.config.get(&"fast".into()), Some(Tag::Byte(b)) if *b != 0);
        let io = match machine.config.get(&"io".into()) {
            Some(Tag::Int(ordinal)) => EnergyIo::by_id(*ordinal),
            _ => EnergyIo::Both,
        };
        Self { fast, io }
    }
}

/// `TileFluxAccessor`'s grid node: 1 AE/t, a channel, every side.
pub(in crate::game) fn device(machine: &Machine, device: &mut ae2_network_core::Device) -> bool {
    if machine.id() != rows::FLUX_ACCESSOR {
        return false;
    }
    device.idle_override = Some(rows::ACCESSOR_IDLE);
    device.channel_override = Some(true);
    device.faces = 0b11_1111;
    true
}

/// `IEnergyStorage` amounts are `int`.
fn clamp(amount: u64) -> i32 {
    amount.min(i32::MAX as u64) as i32
}

impl Game<'_> {
    /// The accessor at `address` answering on side `face`: the block on any
    /// side, the part on a cable bus on its own; its settings.
    fn appflux_accessor(&self, address: GridAddress, face: Direction) -> Option<Settings> {
        let Some(BlockEntity::Ae2(machine)) = self.ae2_block_entity(address) else { return None };
        if machine.id() == rows::FLUX_ACCESSOR {
            return Some(Settings::of(machine));
        }
        if machine.kind != MachineKind::CableBus {
            return None;
        }
        let part = PartAddress { host: address, face: face_of(face) };
        let part = self.ae2_part_machine(part).ok().flatten()?;
        (part.id() == rows::PART_FLUX_ACCESSOR).then(|| Settings::of(&part))
    }
    /// The grid's FE, as `getCachedInventory().get(FluxKey.of(FE))`.
    fn appflux_grid_fe(&self, address: GridAddress) -> u64 {
        self.ae2_grid_keys_encoded(address, rows::key_type())
            .ok()
            .and_then(|keys| keys.into_iter().find(|(key, _)| *key == rows::fe_key()))
            .map_or(0, |(_, amount)| amount)
    }
    /// `getInventory().insert` / `extract` of FE: no AE paid.
    fn appflux_grid_move(&mut self, address: GridAddress, amount: u64, insert: bool, simulate: bool) -> u64 {
        if amount == 0 {
            return 0;
        }
        self.ae2_grid_typed_move_free(address, rows::key_type(), &rows::fe_key(), amount, insert, simulate).unwrap_or(0)
    }

    /// `NetworkFEPower`: the grid's FE, its room, and what the IO mode lets
    /// through; none where no accessor answers.
    pub(in crate::game) fn appflux_accessor_fe_storage(&self, address: GridAddress, face: Direction) -> Option<FeStorage> {
        let settings = self.appflux_accessor(address, face)?;
        let stored = self.appflux_grid_fe(address);
        // getMaxEnergyStored: the room a simulated insert of Long.MAX_VALUE - 1 finds, and what is stored.
        let room = self.appflux_room(address);
        Some(FeStorage {
            stored: clamp(stored),
            capacity: clamp(stored.saturating_add(room)),
            can_receive: settings.io.is_input(),
            can_extract: settings.io.is_output(),
        })
    }
    /// The room for FE a simulated insert finds in the grid's FE cells (a
    /// Void Card's cell takes all); the room of FE blocks behind storage
    /// buses is not counted.
    fn appflux_room(&self, address: GridAddress) -> u64 {
        let kind = rows::key_type();
        let Ok(cells) = self.ae2_grid_cells_encoded(address) else { return 0 };
        let mut room = 0u64;
        for stack in &cells {
            if let Some(Ok(mut cell)) = self.ae2x_handler_cell(stack)
                && cell.key_type() == kind
            {
                room = room.saturating_add(cell.insert(&rows::fe_key(), (i64::MAX - 1) as u64, ae2_cell_core::Action::Simulate));
            }
        }
        room
    }
    /// `NetworkFEPower.receiveEnergy` / `extractEnergy`: the grid's storage,
    /// as the IO mode allows; none where no accessor answers.
    pub(in crate::game) fn appflux_accessor_fe_move(&mut self, address: GridAddress, face: Direction, amount: u32, simulate: bool, extract: bool) -> Option<u32> {
        let settings = self.appflux_accessor(address, face)?;
        let allowed = if extract { settings.io.is_output() } else { settings.io.is_input() };
        if !allowed {
            return Some(0);
        }
        Some(clamp(self.appflux_grid_move(address, u64::from(amount), !extract, simulate)) as u32)
    }

    /// `EnergyHandler.send` with its default handler: what the FE block
    /// beside `pos` on side `side` would take, at most the accessor's IO limit,
    /// drawn from the grid and given; what it refuses after all goes back.
    /// None when no FE block is there (-1).
    fn appflux_send(&mut self, address: GridAddress, pos: (i32, i32, i32), side: Direction) -> Option<u64> {
        let [dx, dy, dz] = offset(side);
        let target = [pos.0 + dx, pos.1 + dy, pos.2 + dz];
        let face = Some(opposite(side));
        let dim = self.dim;
        self.fe_info_at(dim, target, face, false)?;
        let limit = rows::Config::get().accessor_limit().min(i32::MAX as u64) as i64;
        let to_add = self.fe_move_at(dim, target, face, limit, true, false, false).max(0) as u64;
        if to_add == 0 {
            return Some(0);
        }
        let drained = self.appflux_grid_move(address, to_add, false, false);
        if drained == 0 {
            return Some(0);
        }
        let added = self.fe_move_at(dim, target, face, drained as i64, false, false, false).max(0) as u64;
        if drained > added {
            self.appflux_grid_move(address, drained - added, true, false);
        }
        Some(added)
    }

    /// `EnergyDistributeService.onLevelEndTick`: each active accessor whose
    /// IO mode lets FE out pushes on its sides, every tick in fast mode,
    /// else when the side's record says.
    pub(in crate::game) fn tick_appflux_accessors(&mut self) {
        if self.remote.is_some() {
            return;
        }
        // The block's six sides, the part's own.
        let mut found: Vec<((i32, i32, i32), Option<u8>)> = Vec::new();
        for (&pos, entity) in &self.world.block_entities {
            let BlockEntity::Ae2(machine) = entity else { continue };
            if machine.id() == rows::FLUX_ACCESSOR {
                found.push((pos, None));
            } else if machine.kind == MachineKind::CableBus
                && let Some(Tag::List(parts)) = machine.config.get(&"parts".into())
            {
                for part in parts {
                    let Some(Tag::String(id)) = part.get("id") else { continue };
                    if id.to_string_lossy() != rows::PART_FLUX_ACCESSOR {
                        continue;
                    }
                    let Some(Tag::String(face)) = part.get("face") else { continue };
                    if let Some(face) = super::super::super::ae2_grid::face_index(&face.to_string_lossy()) {
                        found.push((pos, Some(face)));
                    }
                }
            }
        }
        if found.is_empty() {
            self.ae2x.appflux.accessors.ticks.clear();
            return;
        }
        let network = self.ae2_network();
        let Ok(analysis) = network.analyze() else { return };
        let now = self.world.game_time;
        let mut live = std::collections::HashSet::new();
        for (pos, part) in found {
            let Some(address) = self.ae2_local_address(pos) else { continue };
            let node = analysis.nodes.get(&address.network_pos());
            // isActive: powered, with its channel.
            let active = match part {
                None => node.is_some_and(|n| n.online),
                Some(face) => node.and_then(|n| n.parts.get(&face)).is_some_and(|p| p.online),
            };
            let sides: Vec<Direction> = match part {
                None => DIRECTIONS.to_vec(),
                Some(face) => vec![DIRECTIONS[usize::from(face)]],
            };
            let Some(settings) = self.appflux_accessor(address, sides[0]) else { continue };
            for side in sides {
                let key = (pos, face_of(side));
                live.insert(key);
                if !active || !settings.io.is_output() {
                    continue;
                }
                let mut record = self.ae2x.appflux.accessors.ticks.get(&key).copied().unwrap_or_default();
                if settings.fast || record.need_tick(now) {
                    if let Some(sent) = self.appflux_send(address, pos, side) {
                        record.sent(sent);
                    }
                }
                self.ae2x.appflux.accessors.ticks.insert(key, record);
            }
        }
        self.ae2x.appflux.accessors.ticks.retain(|key, _| live.contains(key));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_sides_and_the_node_follow_the_accessor() {
        horizon_client::ae2x::install();
        let kind = MachineKind::from_id(rows::FLUX_ACCESSOR).expect("the accessor is an addon machine");
        let mut machine = Machine::new(kind);
        assert_eq!(Settings::of(&machine), Settings { fast: false, io: EnergyIo::Both });
        machine.config.insert("fast".into(), Tag::Byte(1));
        machine.config.insert("io".into(), Tag::Int(1));
        assert_eq!(Settings::of(&machine), Settings { fast: true, io: EnergyIo::Output });
        let mut node = ae2_network_core::Device::new(ae2_network_core::DeviceKind::Interface);
        assert!(device(&machine, &mut node));
        assert_eq!((node.idle_override, node.channel_override, node.faces), (Some(1.0), Some(true), 0b11_1111));
        let part = Machine::new(MachineKind::from_id(rows::PART_FLUX_ACCESSOR).expect("the part keeps a part machine"));
        assert_eq!(part.id(), rows::PART_FLUX_ACCESSOR);
        assert!(!device(&part, &mut node), "the part's node is a part's");
        for direction in DIRECTIONS {
            assert_eq!(opposite(opposite(direction)), direction);
            let [x, y, z] = offset(direction);
            let [ox, oy, oz] = offset(opposite(direction));
            assert_eq!((x + ox, y + oy, z + oz), (0, 0, 0));
        }
        assert_eq!(face_of(Direction::North), 5);
        assert_eq!(clamp(u64::MAX), i32::MAX);
    }
}
