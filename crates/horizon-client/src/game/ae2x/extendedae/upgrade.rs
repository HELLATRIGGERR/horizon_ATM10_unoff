//! ExtendedAE's upgrade kits (`ItemUpgrade` and its `ItemPatternProviderUpgrade`,
//! `ItemInterfaceUpgrade`, `ItemDriveUpgrade`), derived from ExtendedAE
//! 1.21-2.2.42, https://github.com/GlodBlock/ExtendedAE commit
//! 1f86f927552f02451236465c0c96a12a3de8f66c, LGPL-3.0; see `ae2/addons/extendedae/`.
//!
//! A kit used on AE2's own device replaces it with the extended one, where it
//! stands and with all it holds. The item acts through `useOn`, which vanilla
//! reaches only past the block's own use: with a device that opens a menu,
//! while the player sneaks.
use crate::game::{Game, KeyCode};
use horizon_client::{
    ae2_world::{Machine, MachineKind, variant},
    containers::BlockEntity,
    items::ItemStack,
};
use horizon_world::registry::BlockStateId;
use horizon_world::chunk::BlockState;
use horizon_world::nbt::Tag;

/// A kit: the item, the device it replaces, and the cable part it replaces.
/// `ItemUpgrade` matches by the exact class, so an extended device takes none.
pub(in crate::game) struct Kit {
    pub item: &'static str,
    pub block: (&'static str, &'static str),
    pub part: Option<(&'static str, &'static str)>,
}

pub(in crate::game) const KITS: &[Kit] = &[
    Kit {
        item: "extendedae:pattern_provider_upgrade",
        block: ("ae2:pattern_provider", "extendedae:ex_pattern_provider"),
        part: Some(("ae2:cable_pattern_provider", "extendedae:ex_pattern_provider_part")),
    },
    Kit {
        item: "extendedae:interface_upgrade",
        block: ("ae2:interface", "extendedae:ex_interface"),
        part: Some(("ae2:cable_interface", "extendedae:ex_interface_part")),
    },
    Kit { item: "extendedae:drive_upgrade", block: ("ae2:drive", "extendedae:ex_drive"), part: None },
];

fn kit(item: &str) -> Option<&'static Kit> {
    KITS.iter().find(|kit| kit.item == item)
}

/// The machine of row `to` holding all of `old`: its slots keep their numbers,
/// its settings their values (`FCUtil.replaceTile`: the new block entity loads
/// what the old one saved).
pub(in crate::game) fn upgraded(old: &Machine, to: &str) -> Option<Machine> {
    let mut next = Machine::of_variant(variant(to)?);
    if next.kind != old.kind || next.slots.len() < old.slots.len() || next.upgrades.len() != old.upgrades.len() {
        return None;
    }
    next.slots[..old.slots.len()].clone_from_slice(&old.slots);
    next.upgrades.clone_from_slice(&old.upgrades);
    next.energy = old.energy;
    next.config = old.config.clone();
    Some(next)
}

/// The part record for `part` as extended part `to`: a new part in the same
/// place, with what the old one wrote (`replacePart`, then `readFromNBT`).
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
    /// `ItemUpgrade.useOn` for the held item at block `pos`, side `face`.
    /// True when it replaced a device.
    pub(in crate::game) fn extendedae_kit_use(&mut self, pos: [i32; 3], face: [i32; 3]) -> bool {
        if self.remote.is_some() || self.spectator() || !self.keys.contains(&KeyCode::ShiftLeft) {
            return false;
        }
        let Some(kit) = self.held().and_then(|held| kit(&held.item)) else {
            return false;
        };
        let key = (pos[0], pos[1], pos[2]);
        let Some(BlockEntity::Ae2(machine)) = self.world.block_entities.get(&key) else {
            return false;
        };
        let done = if machine.kind == MachineKind::CableBus {
            self.extendedae_part_kit(kit, key, face)
        } else {
            self.extendedae_block_kit(kit, pos)
        };
        if done {
            // `getItemInHand().shrink(1)`, which `ServerPlayerGameMode.useItemOn`
            // puts back in creative.
            self.consume_held();
        }
        done
    }

    fn extendedae_block_kit(&mut self, kit: &Kit, pos: [i32; 3]) -> bool {
        let key = (pos[0], pos[1], pos[2]);
        let old = self.world.block(pos[0], pos[1], pos[2]);
        let Some(BlockEntity::Ae2(machine)) = self.world.block_entities.get(&key) else {
            return false;
        };
        if machine.variant.is_some()
            || machine.id() != kit.block.0
            || self.res.facts().get(old).name.as_ref() != kit.block.0
        {
            return false;
        }
        let Some(next_machine) = upgraded(machine, kit.block.1) else {
            return false;
        };
        let Some(next) = self.extendedae_state_like(old, kit.block.1) else {
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

    /// The default state of `to`, with each property the old state shares.
    fn extendedae_state_like(&self, old: BlockStateId, to: &str) -> Option<BlockStateId> {
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

    fn extendedae_part_kit(&mut self, kit: &Kit, key: (i32, i32, i32), face: [i32; 3]) -> bool {
        let Some((from, to)) = kit.part else {
            return false;
        };
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
        let Some(index) = parts.iter().position(|part| {
            part.get("face") == Some(&Tag::String(side.into())) && part.get("id") == Some(&Tag::String(from.into()))
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
        if from == "ae2:cable_pattern_provider" {
            let face = ["east", "west", "up", "down", "south", "north"].iter().position(|name| *name == side);
            self.ae2_provider_patterns_changed(host, face.map(|face| face as u8));
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kits_name_the_mods_devices() {
        horizon_client::ae2x::install();
        for kit in KITS {
            assert_eq!(variant(kit.block.1).unwrap().base, kit.block.0);
            if let Some((from, to)) = kit.part {
                assert_eq!(variant(to).unwrap().base, from);
            }
        }
        // ItemDriveUpgrade has no part.
        assert!(super::kit("extendedae:drive_upgrade").unwrap().part.is_none());
        assert!(super::kit("extendedae:io_bus_upgrade").is_none());
    }

    #[test]
    fn a_kit_keeps_what_the_device_holds() {
        horizon_client::ae2x::install();
        let mut provider = Machine::for_block("ae2:pattern_provider").unwrap();
        provider.slots[8] = Some(ItemStack::new("ae2:crafting_pattern", 1));
        provider.config.insert("priority".into(), Tag::Int(7));
        let next = upgraded(&provider, "extendedae:ex_pattern_provider").unwrap();
        assert_eq!(next.slots.len(), 36);
        assert_eq!(next.slots[8], provider.slots[8]);
        assert_eq!(next.config.get(&"priority".into()), Some(&Tag::Int(7)));
        assert_eq!(next.id(), "extendedae:ex_pattern_provider");

        let mut drive = Machine::for_block("ae2:drive").unwrap();
        drive.slots[9] = Some(ItemStack::new("ae2:item_storage_cell_1k", 1));
        let next = upgraded(&drive, "extendedae:ex_drive").unwrap();
        assert_eq!((next.slots.len(), next.slots[9].clone()), (20, drive.slots[9].clone()));

        let mut interface = Machine::for_block("ae2:interface").unwrap();
        interface.upgrades[0] = Some(ItemStack::new("ae2:fuzzy_card", 1));
        let next = upgraded(&interface, "extendedae:ex_interface").unwrap();
        assert_eq!((next.slots.len(), next.upgrades.clone()), (36, interface.upgrades.clone()));

        // Never into another kind of machine.
        assert!(upgraded(&drive, "extendedae:ex_interface").is_none());
    }

    #[test]
    fn a_part_keeps_its_record_under_the_new_id() {
        let mut stack = ItemStack::new("ae2:cable_interface", 1);
        stack.damage = 0;
        let part = Tag::compound([
            ("id", Tag::String("ae2:cable_interface".into())),
            ("face", Tag::String("east".into())),
            ("ae2_part_identity", Tag::IntArray(vec![1, 2, 3, 4])),
            ("stack", horizon_client::save::item_to_nbt(&stack)),
            ("machine", Tag::compound([("config", Tag::compound([("priority", Tag::Int(5))]))])),
        ]);
        let next = upgraded_part(&part, "extendedae:ex_interface_part").unwrap();
        assert_eq!(next.get("id"), Some(&Tag::String("extendedae:ex_interface_part".into())));
        assert_eq!(next.get("face"), part.get("face"));
        assert_eq!(next.get("machine"), part.get("machine"));
        assert_ne!(next.get("ae2_part_identity"), part.get("ae2_part_identity"));
        assert_eq!(horizon_client::save::item_from_nbt(next.get("stack").unwrap()).unwrap().item.as_ref(), "extendedae:ex_interface_part");
    }
}
