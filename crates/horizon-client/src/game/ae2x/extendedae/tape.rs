//! ME Packing Tape and the package it makes (`ItemMEPackingTape`,
//! `ItemPackedDevice`), derived from ExtendedAE 1.21-2.2.42,
//! https://github.com/GlodBlock/ExtendedAE commit
//! 1f86f927552f02451236465c0c96a12a3de8f66c, LGPL-3.0; see `ae2/addons/extendedae/`.
//!
//! The tape (64 uses) takes a device of `item.me_packing_tape_whitelist`, a
//! block or the part on the face hit, with everything it holds, into a
//! package dropped where it stood; the package puts it back where it is used.
//! A block's record is the port's saved block entity (`saveWithFullMetadata`),
//! a part's its record on the cable (`writeToNBT`).
use crate::game::Game;
use horizon_client::{ae2_world::MachineKind, ae2x::extendedae as rows, containers::BlockEntity, items::ItemStack, save};
use horizon_world::chunk::BlockState;
use horizon_world::nbt::Tag;

const FACES: [&str; 6] = ["east", "west", "up", "down", "south", "north"];
fn face_name(face: [i32; 3]) -> Option<&'static str> {
    Some(match face {
        [1, 0, 0] => "east",
        [-1, 0, 0] => "west",
        [0, 1, 0] => "up",
        [0, -1, 0] => "down",
        [0, 0, 1] => "south",
        [0, 0, -1] => "north",
        _ => return None,
    })
}
fn text(tag: Option<&Tag>) -> Option<String> {
    match tag? {
        Tag::String(s) => s.to_string().ok(),
        _ => None,
    }
}

impl Game<'_> {
    /// `ItemMEPackingTape.onItemUseFirst` on block `pos`, side `face`. True
    /// when it packed something.
    pub(in crate::game) fn extendedae_tape_use(&mut self, pos: [i32; 3], face: [i32; 3]) -> bool {
        if self.remote.is_some() || self.spectator() || !self.held().is_some_and(|s| s.item.as_ref() == rows::PACKING_TAPE) {
            return false;
        }
        let key = (pos[0], pos[1], pos[2]);
        let Some(BlockEntity::Ae2(machine)) = self.world.block_entities.get(&key) else { return false };
        let config = rows::Config::get();
        let mut pack = ItemStack::new(rows::PACKAGE, 1);
        if machine.kind == MachineKind::CableBus {
            // selectPartLocal: the part on the face hit.
            let Some(side) = face_name(face) else { return false };
            let Some(Tag::List(parts)) = machine.config.get(&"parts".into()) else { return false };
            let Some(index) = parts.iter().position(|p| p.get("face") == Some(&Tag::String(side.into()))) else { return false };
            let part = parts[index].clone();
            let Some(id) = text(part.get("id")) else { return false };
            if !config.tape_takes(&id) {
                return false;
            }
            pack.data_mut().set_other(rows::IS_PART, Some(Tag::Byte(1)));
            pack.data_mut().set_other(rows::TAPE_PART_DATA, Some(Tag::compound([("id", Tag::String(id.as_str().into())), ("ctx", part.clone())])));
            self.extendedae_take_part(key, index, part);
        } else {
            let block = self.res.facts().get(self.world.block(pos[0], pos[1], pos[2])).name.to_string();
            if !config.tape_takes(&block) {
                return false;
            }
            let Some(state) = self.pack.assets.registry.state(self.world.block(pos[0], pos[1], pos[2])).cloned() else { return false };
            let entity = BlockEntity::Ae2(machine.clone());
            let mut ctx = save::encode_block_entity(key, &entity, &block);
            if let (Tag::Compound(map), Some(name)) = (&mut ctx, self.world.block_names.get(&key)) {
                map.insert("CustomName".into(), Tag::String(name.as_str().into()));
            }
            let properties = Tag::compound(state.properties.iter().map(|(k, v)| (k.as_str(), Tag::String(v.as_str().into()))).collect::<Vec<_>>());
            let state_tag = Tag::compound([("Name", Tag::String(state.name.as_str().into())), ("Properties", properties)]);
            pack.data_mut().set_other(rows::IS_PART, Some(Tag::Byte(0)));
            pack.data_mut().set_other(
                rows::TAPE_TILE_DATA,
                Some(Tag::compound([("id", Tag::String(entity.id().into())), ("block_id", Tag::String(block.as_str().into())), ("state", state_tag), ("ctx", ctx)])),
            );
            // removeBlockEntity, removeBlock: nothing drops.
            self.world.block_entities.remove(&key);
            self.world.block_names.remove(&key);
            self.extendedae_forget_saved_entity(pos);
            let Some(air) = self.pack.assets.registry.default_state("minecraft:air") else { return true };
            let _ = self.level_set(pos, air);
        }
        // Platform.spawnDrops, then hurtAndBreak(1).
        self.pop_resource(pos, pack);
        self.damage_held(1);
        true
    }
    /// A chunk can keep an older saved copy of the block entity; it must not
    /// come back or spill.
    fn extendedae_forget_saved_entity(&mut self, pos: [i32; 3]) {
        if let Some(chunk) = self.world.chunks.get_mut(&(pos[0] >> 4, pos[2] >> 4))
            && let Some(saved) = std::sync::Arc::make_mut(chunk).saved.as_mut()
        {
            let list = &mut std::sync::Arc::make_mut(saved).block_entities;
            list.retain(|tag| {
                !(matches!(tag.get("x"), Some(Tag::Int(x)) if *x == pos[0])
                    && matches!(tag.get("y"), Some(Tag::Int(y)) if *y == pos[1])
                    && matches!(tag.get("z"), Some(Tag::Int(z)) if *z == pos[2]))
            });
        }
    }
    /// `CableBusContainer.removePart`: the part goes, dropping nothing; a
    /// cable bus with nothing left goes with it.
    fn extendedae_take_part(&mut self, key: (i32, i32, i32), index: usize, part: Tag) {
        let Some(BlockEntity::Ae2(machine)) = self.world.block_entities.get_mut(&key) else { return };
        let Some(Tag::List(parts)) = machine.config.get_mut(&"parts".into()) else { return };
        parts.remove(index);
        let has_terminal = parts.iter().any(|p| text(p.get("id")).is_some_and(|id| id.ends_with("terminal")));
        let no_parts = parts.is_empty();
        if has_terminal {
            machine.config.insert("terminal_part".into(), Tag::Byte(1));
        } else {
            machine.config.remove(&"terminal_part".into());
        }
        let no_facades = match machine.config.get(&"facades".into()) {
            None => true,
            Some(Tag::List(v)) => v.is_empty(),
            _ => false,
        };
        let remove_host = machine.config.get(&"ae2_centerless".into()) == Some(&Tag::Byte(1)) && !machine.config.contains_key(&"cable".into()) && no_parts && no_facades;
        self.world.block_entities.record_ae2_part(key, part, false);
        self.world.modified.insert((key.0 >> 4, key.2 >> 4));
        if remove_host {
            let _ = self.set_state_named([key.0, key.1, key.2], "minecraft:air", &[]);
        }
        if self.ae2_has_anchor_owners() {
            if let Err(error) = self.ae2_apply_anchor_callbacks() {
                log::debug!("AE2 anchor callbacks deferred: {error}");
            }
        }
    }

    /// The package used on a block: past the block's own use, which an AE2
    /// machine's menu takes (`useItemOn`, `useWithoutItem`) unless the player sneaks.
    pub(in crate::game) fn extendedae_package_on(&mut self, pos: [i32; 3], face: [i32; 3]) -> bool {
        let menu = matches!(self.world.block_entities.get(&(pos[0], pos[1], pos[2])), Some(BlockEntity::Ae2(m)) if m.kind != MachineKind::CableBus);
        if menu && !self.keys.contains(&crate::game::KeyCode::ShiftLeft) {
            return false;
        }
        self.extendedae_package_use(pos, face)
    }
    /// `ItemPackedDevice.useOn`: the packed part placed on the cable (or
    /// where a part would go, `PartPlacement`), or the packed block placed as
    /// a block item would be, each with what it held. True when it acted.
    pub(in crate::game) fn extendedae_package_use(&mut self, pos: [i32; 3], face: [i32; 3]) -> bool {
        if self.remote.is_some() || self.spectator() {
            return false;
        }
        let Some(pack) = self.held().filter(|s| s.item.as_ref() == rows::PACKAGE).cloned() else { return false };
        let Some(data) = pack.data() else { return true };
        let is_part = match data.other(rows::IS_PART) {
            Some(Tag::Byte(b)) => *b != 0,
            // No IS_PART: FAIL.
            _ => return true,
        };
        let placed = if is_part {
            let Some(record) = data.other(rows::TAPE_PART_DATA).cloned() else { return true };
            self.extendedae_unpack_part(pos, face, &pack, &record)
        } else {
            let Some(record) = data.other(rows::TAPE_TILE_DATA).cloned() else { return true };
            self.extendedae_unpack_block(pos, face, &record)
        };
        if placed {
            if let Some(held) = self.held_mut().as_mut() {
                *held = pack;
            }
            // pack.shrink(1), which creative puts back.
            if !self.creative {
                self.consume_held();
            }
        }
        true
    }
    fn extendedae_unpack_part(&mut self, pos: [i32; 3], face: [i32; 3], pack: &ItemStack, record: &Tag) -> bool {
        let (Some(id), Some(ctx)) = (text(record.get("id")), record.get("ctx")) else { return false };
        let Some(side) = face_name(face) else { return false };
        // PartPlacement.placePart with the part's item, then readFromNBT.
        let mut part_item = ctx.get("stack").and_then(save::item_from_nbt).filter(|s| s.item.as_ref() == id).unwrap_or_else(|| ItemStack::new(&id, 1));
        part_item.count = 1;
        let creative = self.creative;
        self.creative = true;
        *self.held_mut() = Some(part_item);
        let before = self.extendedae_parts_by_place(pos, side);
        let attached = self.ae2_try_attach_part((pos[0], pos[1], pos[2]), side);
        self.creative = creative;
        *self.held_mut() = Some(pack.clone());
        if !attached {
            return false;
        }
        let Some((host, host_face)) = before else { return false };
        let Some(BlockEntity::Ae2(machine)) = self.world.block_entities.get_mut(&host) else { return false };
        let Some(Tag::List(parts)) = machine.config.get_mut(&"parts".into()) else { return false };
        let Some(Tag::Compound(placed)) = parts.iter_mut().find(|p| p.get("face") == Some(&Tag::String(host_face.into()))) else { return false };
        if text(placed.get(&"id".into())).as_deref() != Some(id.as_str()) {
            return false;
        }
        // The new part keeps its place and node; the rest is the packed record's.
        let Tag::Compound(packed) = ctx else { return false };
        let old = Tag::Compound(placed.clone());
        for (k, v) in packed {
            if !matches!(k.to_string_lossy().as_str(), "face" | "ae2_part_identity") {
                placed.insert(k.clone(), v.clone());
            }
        }
        let new = Tag::Compound(placed.clone());
        self.world.block_entities.record_ae2_part(host, old, false);
        self.world.block_entities.record_ae2_part(host, new, true);
        self.world.modified.insert((host.0 >> 4, host.2 >> 4));
        if matches!(horizon_client::ae2_world::base_id(&id), "ae2:cable_pattern_provider")
            && let Some(address) = self.ae2_local_address(host)
        {
            self.ae2_provider_patterns_changed(address, FACES.iter().position(|f| *f == host_face).map(|f| f as u8));
        }
        true
    }
    /// Where a part used on `pos`, side `side`, goes: the cable bus there, or
    /// the block beyond that side, on the opposite face.
    fn extendedae_parts_by_place(&self, pos: [i32; 3], side: &'static str) -> Option<((i32, i32, i32), &'static str)> {
        let key = (pos[0], pos[1], pos[2]);
        if matches!(self.world.block_entities.get(&key), Some(BlockEntity::Ae2(m)) if m.kind == MachineKind::CableBus) {
            return Some((key, side));
        }
        let (d, opposite) = match side {
            "east" => ([1, 0, 0], "west"),
            "west" => ([-1, 0, 0], "east"),
            "up" => ([0, 1, 0], "down"),
            "down" => ([0, -1, 0], "up"),
            "south" => ([0, 0, 1], "north"),
            _ => ([0, 0, -1], "south"),
        };
        Some(((pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]), opposite))
    }
    fn extendedae_unpack_block(&mut self, pos: [i32; 3], face: [i32; 3], record: &Tag) -> bool {
        let (Some(state), Some(ctx)) = (record.get("state"), record.get("ctx")) else { return false };
        let Some(name) = text(state.get("Name")) else { return false };
        let mut properties = horizon_world::block_state_properties::BlockProperties::new();
        if let Some(Tag::Compound(map)) = state.get("Properties") {
            for (k, v) in map {
                if let Tag::String(v) = v {
                    properties.insert(k.to_string_lossy(), v.to_string_lossy());
                }
            }
        }
        let Ok(state) = self.pack.assets.registry.resolve(&BlockState { name: name.clone(), properties }) else { return false };
        // BlockPlaceContext: the block clicked when it can be replaced, else the one beyond.
        let replaceable = |game: &Self, at: [i32; 3]| {
            let id = &game.res.facts().get(game.world.block(at[0], at[1], at[2])).name;
            matches!(id.as_ref(), "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air" | "minecraft:water" | "minecraft:short_grass" | "minecraft:snow")
        };
        let target = if replaceable(self, pos) { pos } else { [pos[0] + face[0], pos[1] + face[1], pos[2] + face[2]] };
        if !replaceable(self, target) || self.world.block_entities.get(&(target[0], target[1], target[2])).is_some() {
            return false;
        }
        let mut ctx = ctx.clone();
        let custom_name = match &mut ctx {
            Tag::Compound(map) => {
                for (k, v) in [("x", target[0]), ("y", target[1]), ("z", target[2])] {
                    map.insert(k.into(), Tag::Int(v));
                }
                match map.remove(&"CustomName".into()) {
                    Some(Tag::String(n)) => n.to_string().ok(),
                    _ => None,
                }
            }
            _ => return false,
        };
        let Some((_, entity)) = save::decode_block_entity(&ctx) else { return false };
        if self.level_set(target, state).is_err() {
            return false;
        }
        let key = (target[0], target[1], target[2]);
        // setBlockEntity, loadWithComponents.
        let provider = matches!(&entity, BlockEntity::Ae2(m) if m.kind == MachineKind::PatternProvider);
        self.world.block_entities.insert(key, entity);
        if let Some(name) = custom_name {
            self.world.block_names.insert(key, name);
        }
        self.world.modified.insert((key.0 >> 4, key.2 >> 4));
        if provider {
            self.ae2_provider_patterns_changed_local(key, None);
        }
        true
    }
}
