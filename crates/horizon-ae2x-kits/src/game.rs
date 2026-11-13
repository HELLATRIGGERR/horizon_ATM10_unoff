//! A kit's use on a device: the held kit puts the device it names in the device's place, with all
//! it holds (ExtendedAE's `ItemUpgrade.useOn`, Advanced AE's provider upgrades; the rows and what a
//! kit makes of a device are the lib's `ae2x::kits`).
//!
//! The item acts through `useOn`, which vanilla reaches only past the block's
//! own use: with a device that opens a menu, while the player sneaks. A kit
//! matches a device by its exact id, so a device it made takes none of it.
use horizon_client::{
    ae2_world::{MachineKind, variant},
    ae2x::kits::{Kit, upgraded, upgraded_part},
    containers::BlockEntity,
    game::ae2_ext::{Ae2Addon, Ae2Host},
};
use horizon_world::chunk::BlockState;
use horizon_world::nbt::Tag;
use horizon_world::registry::BlockStateId;
use std::any::Any;

/// What the addons still inside horizon-client ask by [`Ae2Host::call_addon`]: `"use"` with
/// `(kits, pos, face)`, answered with whether a device was replaced.
pub const USE: &str = "use";

/// The arguments of [`USE`].
pub type UseArgs = (&'static [Kit], [i32; 3], [i32; 3]);

struct Kits;

pub(crate) fn make() -> Box<dyn Ae2Addon> {
    Box::new(Kits)
}

impl Ae2Addon for Kits {
    fn id(&self) -> &'static str {
        crate::ID
    }

    fn call(&self, host: &mut dyn Ae2Host, what: &str, args: &dyn Any) -> Option<Box<dyn Any>> {
        let &(kits, pos, face) = args.downcast_ref::<UseArgs>().filter(|_| what == USE)?;
        Some(Box::new(kit_use(host, kits, pos, face)))
    }
}

/// `useOn` of the held item, when it is one of `kits`, at block `pos`,
/// side `face`. True when it replaced a device.
pub fn kit_use(host: &mut dyn Ae2Host, kits: &[Kit], pos: [i32; 3], face: [i32; 3]) -> bool {
    if host.is_remote() || host.spectator() || !host.shift_down() {
        return false;
    }
    let Some(kit) = host.held().and_then(|held| kits.iter().find(|kit| kit.item == held.item.as_ref())) else {
        return false;
    };
    let key = (pos[0], pos[1], pos[2]);
    let Some(BlockEntity::Ae2(machine)) = host.world().block_entities.get(&key) else {
        return false;
    };
    let done = if machine.kind == MachineKind::CableBus {
        part_kit(host, kit, key, face)
    } else {
        block_kit(host, kit, pos)
    };
    if done {
        // `getItemInHand().shrink(1)`, which `ServerPlayerGameMode.useItemOn`
        // puts back in creative.
        host.consume_held();
    }
    done
}

fn block_kit(host: &mut dyn Ae2Host, kit: &Kit, pos: [i32; 3]) -> bool {
    let key = (pos[0], pos[1], pos[2]);
    let old = host.world().block(pos[0], pos[1], pos[2]);
    let Some(BlockEntity::Ae2(machine)) = host.world().block_entities.get(&key) else {
        return false;
    };
    let Some(&(_, to)) = kit.blocks.iter().find(|(from, _)| machine.id() == *from) else {
        return false;
    };
    if host.block_state_name(old) != machine.id() {
        return false;
    }
    let Some(next_machine) = upgraded(machine, to) else {
        return false;
    };
    let Some(next) = state_like(host, old, to) else {
        return false;
    };
    let world = host.world_mut();
    let name = world.block_names.get(&key).cloned();
    let entity = world.block_entities.remove(&key);
    // A chunk can keep an older saved copy; it must not come back or spill.
    let saved = world.chunks.get_mut(&(pos[0] >> 4, pos[2] >> 4)).and_then(|chunk| {
        let saved = std::sync::Arc::make_mut(chunk).saved.as_mut()?;
        let list = &mut std::sync::Arc::make_mut(saved).block_entities;
        let at = list.iter().position(|tag| {
            matches!(tag.get("x"), Some(Tag::Int(x)) if *x == pos[0])
                && matches!(tag.get("y"), Some(Tag::Int(y)) if *y == pos[1])
                && matches!(tag.get("z"), Some(Tag::Int(z)) if *z == pos[2])
        })?;
        Some(list.remove(at))
    });
    let set = host.level_set(pos, next);
    if set.is_err() || host.world().block(pos[0], pos[1], pos[2]) != next {
        let world = host.world_mut();
        if let Some(entity) = entity {
            world.block_entities.insert(key, entity);
        }
        if let (Some(saved), Some(chunk)) = (saved, world.chunks.get_mut(&(pos[0] >> 4, pos[2] >> 4)))
            && let Some(list) = std::sync::Arc::make_mut(chunk).saved.as_mut()
        {
            std::sync::Arc::make_mut(list).block_entities.push(saved);
        }
        return false;
    }
    let provider = next_machine.kind == MachineKind::PatternProvider;
    let world = host.world_mut();
    world.block_entities.insert(key, BlockEntity::Ae2(Box::new(next_machine)));
    if let Some(name) = name {
        world.block_names.insert(key, name);
    }
    world.modified.insert((pos[0] >> 4, pos[2] >> 4));
    if provider {
        host.provider_patterns_changed_local(key, None);
    }
    true
}

/// The default state of `to`, with each property the old state shares
/// (`getStateForPlacement`, then the old state's values).
fn state_like(host: &dyn Ae2Host, old: BlockStateId, to: &str) -> Option<BlockStateId> {
    let registry = host.block_registry();
    let old: &BlockState = registry.state(old)?;
    let mut target = registry.state(registry.default_state(to)?)?.clone();
    for (name, value) in &old.properties {
        if target.properties.contains_key(name) {
            target.properties.insert(name.clone(), value.clone());
        }
    }
    registry.resolve(&target).ok()
}

fn part_kit(host: &mut dyn Ae2Host, kit: &Kit, key: (i32, i32, i32), face: [i32; 3]) -> bool {
    let side = match face {
        [0, -1, 0] => "down",
        [0, 1, 0] => "up",
        [0, 0, -1] => "north",
        [0, 0, 1] => "south",
        [-1, 0, 0] => "west",
        [1, 0, 0] => "east",
        _ => return false,
    };
    let Some(address) = host.local_address(key) else {
        return false;
    };
    let world = host.world_mut();
    let Some(BlockEntity::Ae2(machine)) = world.block_entities.get_mut(&key) else {
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
    world.block_entities.record_ae2_part(key, old, false);
    world.block_entities.record_ae2_part(key, next, true);
    world.modified.insert((key.0 >> 4, key.2 >> 4));
    if variant(to).is_some_and(|row| row.kind == MachineKind::PatternProvider) {
        let face = ["east", "west", "up", "down", "south", "north"].iter().position(|name| *name == side);
        host.provider_patterns_changed(address, face.map(|face| face as u8));
    }
    true
}
