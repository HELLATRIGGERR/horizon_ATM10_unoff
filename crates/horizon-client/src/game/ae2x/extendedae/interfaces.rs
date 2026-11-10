//! ExtendedAE's Oversize Interface (AE2's interface whose slots hold each key
//! type's capacity times `oversize_interface_multiplier`, `OversizeConfigInv`)
//! and Vacuum Interface (`TileVacuumInterface`: dropped items in its work area
//! into the grid, by its 63 filters and five cards).
use super::special::{self, KeyList};
use crate::game::Game;
use ae2_machines_core::Key;
use horizon_client::ae2_world::Machine;
use horizon_client::ae2x::extendedae::{Config, special as rows};
use horizon_client::containers::BlockEntity;
use horizon_world::nbt::Tag;

/// `EAEConfig.getOversizeMultiplier` of an Oversize Interface's key type;
/// none for any other machine.
pub(in crate::game) fn oversize_multiplier(machine: &Machine, key_type: ae2_cell_core::KeyType) -> Option<u64> {
    matches!(machine.id(), rows::OVERSIZE_INTERFACE | rows::OVERSIZE_INTERFACE_PART).then(|| Config::get().oversize_multiplier(key_type.def().id))
}

/// The Vacuum Interface's node: `setFlags()` (no channel), 1 AE/t idle, every side.
pub(in crate::game) fn device(machine: &Machine, device: &mut ae2_network_core::Device) -> bool {
    if machine.id() != rows::VACUUM_INTERFACE {
        return false;
    }
    device.faces = 63;
    device.idle_override = Some(1.0);
    device.channel_override = Some(false);
    true
}

fn long(machine: &Machine, name: &str) -> Option<i64> {
    match machine.config.get(&name.into()) {
        Some(Tag::Long(n)) => Some(*n),
        _ => None,
    }
}
/// The work area's size (5, 5, 5 unless set) and offset.
pub(in crate::game) fn vacuum_shape(machine: &Machine) -> ([i32; 3], [i32; 3]) {
    let size = long(machine, rows::VACUUM_SIZE).map_or([5, 5, 5], rows::block_pos_of);
    let offset = long(machine, rows::VACUUM_OFFSET).map_or([0, 0, 0], rows::block_pos_of);
    rows::vacuum_clamp(size, offset)
}
pub(in crate::game) fn set_vacuum_shape(machine: &mut Machine, size: [i32; 3], offset: [i32; 3]) {
    let (size, offset) = rows::vacuum_clamp(size, offset);
    machine.config.insert(rows::VACUUM_SIZE.into(), Tag::Long(rows::block_pos_long(size)));
    machine.config.insert(rows::VACUUM_OFFSET.into(), Tag::Long(rows::block_pos_long(offset)));
}

impl Game<'_> {
    /// `TileVacuumInterface.doWork`, each tick while it works (no Redstone
    /// Card, or its signal as `REDSTONE_CONTROLLED` asks): every dropped item
    /// whose box meets the work area and its list passes goes into the grid,
    /// what does not fit stays on the ground.
    pub(in crate::game) fn tick_extendedae_vacuum(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let mut found: Vec<(i32, i32, i32)> = self.world.block_entities.iter()
            .filter_map(|(p, e)| matches!(e, BlockEntity::Ae2(m) if m.id() == rows::VACUUM_INTERFACE).then_some(*p))
            .collect();
        found.sort_unstable();
        for pos in found {
            let Some(BlockEntity::Ae2(machine)) = self.world.block_entities.get(&pos) else { continue };
            let machine = (**machine).clone();
            if !self.ae2_online(pos) || !self.extendedae_vacuum_enabled(pos, &machine) {
                continue;
            }
            let list = self.extendedae_vacuum_list(&machine);
            let (size, offset) = vacuum_shape(&machine);
            let (min, max) = rows::vacuum_area([pos.0, pos.1, pos.2], size, offset);
            let hits: Vec<_> = self.dropped.iter()
                .filter(|e| {
                    let p = e.position;
                    let (lo, hi) = ([p.x - 0.125, p.y, p.z - 0.125], [p.x + 0.125, p.y + 0.25, p.z + 0.125]);
                    (0..3).all(|i| lo[i] < max[i] && hi[i] > min[i])
                })
                .filter(|e| crate::game::ae2_lab::key(&e.stack).is_ok_and(|k| list.matches(&Key::from(&k))))
                .map(|e| (e.id, e.stack.clone()))
                .collect();
            for (id, stack) in hits {
                let added = self.ae2_grid_insert(pos, &stack, u64::from(stack.count)).unwrap_or(0).min(u64::from(stack.count)) as u32;
                if added > 0 {
                    if let Some(entity) = self.dropped.iter_mut().find(|e| e.id == id) {
                        entity.stack.count = entity.stack.count.saturating_sub(added);
                    }
                }
            }
            self.dropped.retain(|e| e.stack.count > 0);
        }
    }
    /// `checkEnable`.
    fn extendedae_vacuum_enabled(&mut self, pos: (i32, i32, i32), machine: &Machine) -> bool {
        if !machine.upgrades.iter().flatten().any(|c| c.item.as_ref() == "ae2:redstone_card") {
            return true;
        }
        let at = horizon_core::position::BlockPos::new(pos.0, pos.1, pos.2);
        let signal = self.with_level(|b, level, ticks| b.has_neighbor_signal(level, ticks, at));
        if special::text(&machine.config, "redstone_controlled") == "high_signal" { signal } else { !signal }
    }
    /// `createFilter`: the first 18 + 9 a capacity card filters, fuzzy with
    /// its card. (It takes no Inverter Card: always a whitelist.)
    pub(in crate::game) fn extendedae_vacuum_list(&self, machine: &Machine) -> KeyList {
        let upgrades = Tag::List(machine.upgrades.iter().flatten().map(horizon_client::save::item_to_nbt).collect());
        let fuzzy = Some(special::text(&machine.config, "fuzzy_mode")).filter(|s| !s.is_empty());
        self.extendedae_key_list(machine.config.get(&special::CONFIG.into()), Some(&upgrades), fuzzy)
    }
}
