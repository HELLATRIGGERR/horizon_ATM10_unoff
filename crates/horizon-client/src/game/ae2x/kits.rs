//! The addons' upgrade kits: an item that, used on a device, puts another in
//! its place with all it holds. ExtendedAE's `ItemUpgrade` (`FCUtil.replaceTile`,
//! `replacePart`), derived from ExtendedAE 1.21-2.2.42,
//! https://github.com/GlodBlock/ExtendedAE commit
//! 1f86f927552f02451236465c0c96a12a3de8f66c, LGPL-3.0; see `ae2/addons/extendedae/`.
//! Advanced AE's provider upgrades (`AdvPatternProviderUpgradeItem`,
//! `AdvPatternProviderCapacityUpgradeItem`) act the same; see
//! `ae2/addons/advancedae/`.
//!
//! The item acts through `useOn`, which vanilla reaches only past the block's
//! own use: with a device that opens a menu, while the player sneaks. A kit
//! matches a device by its exact id, so a device it made takes none of it.
use crate::game::{Game, KeyCode};
use horizon_client::{
    ae2_world::{Machine, MachineKind, variant},
    containers::BlockEntity,
    items::ItemStack,
};
use horizon_world::chunk::BlockState;
use horizon_world::nbt::Tag;
use horizon_world::registry::BlockStateId;

/// A kit: the item, the devices it replaces and the cable parts it replaces,
/// each with what takes its place.
pub(in crate::game) struct Kit {
    pub item: &'static str,
    pub blocks: &'static [(&'static str, &'static str)],
    pub parts: &'static [(&'static str, &'static str)],
}

/// The machine of row `to` holding all of `old`: its slots keep their numbers,
/// its settings their values (`FCUtil.replaceTile`: the new block entity loads
/// what the old one saved).
pub(in crate::game) fn upgraded(old: &Machine, to: &str) -> Option<Machine> {
    // A variant of AE2's device, or one of an addon's own blocks into another
    // (ExtendedAE's Wireless Connector into the Hub).
    let mut next = match variant(to) {
        Some(row) => Machine::of_variant(row),
        None => Machine::for_block(to).filter(|m| m.kind.ext().is_some() && old.kind.ext().is_some())?,
    };
    let same_kind = next.kind == old.kind || next.kind.ext().is_some();
    if !same_kind || next.slots.len() < old.slots.len() || next.upgrades.len() != old.upgrades.len() {
        return None;
    }
    next.slots[..old.slots.len()].clone_from_slice(&old.slots);
    next.upgrades.clone_from_slice(&old.upgrades);
    next.energy = old.energy;
    next.config = old.config.clone();
    Some(next)
}

/// The part record for `part` as part `to`: a new part in the same place,
/// with what the old one wrote (`replacePart`, then `readFromNBT`).
pub(in crate::game) fn upgraded_part(part: &Tag, to: &str) -> Option<Tag> {
    let mut next = part.clone();
    let stack = part.get("stack").and_then(horizon_client::save::item_from_nbt)?;
    let stack = ItemStack { item: to.into(), ..stack };
    let Tag::Compound(record) = &mut next else { return None };
    record.insert("id".into(), Tag::String(to.into()));
    record.insert("stack".into(), horizon_client::save::item_to_nbt(&stack));
    record.insert("ae2_part_identity".into(), Tag::IntArray(horizon_client::save::random_uuid().to_vec()));
    Some(next)
}

impl Game<'_> {
    /// `useOn` of the held item, when it is one of `kits`, at block `pos`,
    /// side `face`. True when it replaced a device.
    pub(in crate::game) fn ae2x_kit_use(&mut self, kits: &[Kit], pos: [i32; 3], face: [i32; 3]) -> bool {
        if self.remote.is_some() || self.spectator() || !self.keys.contains(&KeyCode::ShiftLeft) {
            return false;
        }
        let Some(kit) = self.held().and_then(|held| kits.iter().find(|kit| kit.item == held.item.as_ref())) else {
            return false;
        };
        let key = (pos[0], pos[1], pos[2]);
        let Some(BlockEntity::Ae2(machine)) = self.world.block_entities.get(&key) else {
            return false;
        };
        let done = if machine.kind == MachineKind::CableBus {
            self.ae2x_part_kit(kit, key, face)
        } else {
            self.ae2x_block_kit(kit, pos)
        };
        if done {
            // `getItemInHand().shrink(1)`, which `ServerPlayerGameMode.useItemOn`
            // puts back in creative.
            self.consume_held();
        }
        done
    }

    fn ae2x_block_kit(&mut self, kit: &Kit, pos: [i32; 3]) -> bool {
        let key = (pos[0], pos[1], pos[2]);
        let old = self.world.block(pos[0], pos[1], pos[2]);
        let Some(BlockEntity::Ae2(machine)) = self.world.block_entities.get(&key) else {
            return false;
        };
        let Some(&(_, to)) = kit.blocks.iter().find(|(from, _)| machine.id() == *from) else {
            return false;
        };
        if self.res.facts().get(old).name.as_ref() != machine.id() {
            return false;
        }
        let Some(next_machine) = upgraded(machine, to) else {
            return false;
        };
        let Some(next) = self.ae2x_state_like(old, to) else {
            return false;
        };
        let name = self.world.block_names.get(&key).cloned();
        let entity = self.world.block_entities.remove(&key);
        // A chunk can keep an older saved copy; it must not come back or spill.
        let saved = self.world.chunks.get_mut(&(pos[0] >> 4, pos[2] >> 4)).and_then(|chunk| {
            let saved = std::sync::Arc::make_mut(chunk).saved.as_mut()?;
            let list = &mut std::sync::Arc::make_mut(saved).block_entities;
            let at = list.iter().position(|tag| {
                matches!(tag.get("x"), Some(Tag::Int(x)) if *x == pos[0])
                    && matches!(tag.get("y"), Some(Tag::Int(y)) if *y == pos[1])
                    && matches!(tag.get("z"), Some(Tag::Int(z)) if *z == pos[2])
            })?;
            Some(list.remove(at))
        });
        let set = self.level_set(pos, next);
        if set.is_err() || self.world.block(pos[0], pos[1], pos[2]) != next {
            if let Some(entity) = entity {
                self.world.block_entities.insert(key, entity);
            }
            if let (Some(saved), Some(chunk)) = (saved, self.world.chunks.get_mut(&(pos[0] >> 4, pos[2] >> 4)))
                && let Some(list) = std::sync::Arc::make_mut(chunk).saved.as_mut()
            {
                std::sync::Arc::make_mut(list).block_entities.push(saved);
            }
            return false;
        }
        let provider = next_machine.kind == MachineKind::PatternProvider;
        self.world.block_entities.insert(key, BlockEntity::Ae2(Box::new(next_machine)));
        if let Some(name) = name {
            self.world.block_names.insert(key, name);
        }
        self.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
        if provider {
            self.ae2_provider_patterns_changed_local(key, None);
        }
        true
    }

    /// The default state of `to`, with each property the old state shares
    /// (`getStateForPlacement`, then the old state's values).
    fn ae2x_state_like(&self, old: BlockStateId, to: &str) -> Option<BlockStateId> {
        let registry = self.pack.assets.registry;
        let old: &BlockState = registry.state(old)?;
        let mut target = registry.state(registry.default_state(to)?)?.clone();
        for (name, value) in &old.properties {
            if target.properties.contains_key(name) {
                target.properties.insert(name.clone(), value.clone());
            }
        }
        registry.resolve(&target).ok()
    }

    fn ae2x_part_kit(&mut self, kit: &Kit, key: (i32, i32, i32), face: [i32; 3]) -> bool {
        let side = match face {
            [0, -1, 0] => "down",
            [0, 1, 0] => "up",
            [0, 0, -1] => "north",
            [0, 0, 1] => "south",
            [-1, 0, 0] => "west",
            [1, 0, 0] => "east",
            _ => return false,
        };
        let Some(host) = self.ae2_local_address(key) else {
            return false;
        };
        let Some(BlockEntity::Ae2(machine)) = self.world.block_entities.get_mut(&key) else {
            return false;
        };
        let Some(Tag::List(parts)) = machine.config.get_mut(&"parts".into()) else {
            return false;
        };
        let Some((index, to)) = parts.iter().enumerate().find_map(|(i, part)| {
            if part.get("face") != Some(&Tag::String(side.into())) {
                return None;
            }
            kit.parts.iter().find(|(from, _)| part.get("id") == Some(&Tag::String((*from).into()))).map(|(_, to)| (i, *to))
        }) else {
            return false;
        };
        let old = parts[index].clone();
        let Some(next) = upgraded_part(&old, to) else {
            return false;
        };
        parts[index] = next.clone();
        self.world.block_entities.record_ae2_part(key, old, false);
        self.world.block_entities.record_ae2_part(key, next, true);
        self.world.modified.insert((key.0 >> 4, key.2 >> 4));
        if variant(to).is_some_and(|row| row.kind == MachineKind::PatternProvider) {
            let face = ["east", "west", "up", "down", "south", "north"].iter().position(|name| *name == side);
            self.ae2_provider_patterns_changed(host, face.map(|face| face as u8));
        }
        true
    }
}
