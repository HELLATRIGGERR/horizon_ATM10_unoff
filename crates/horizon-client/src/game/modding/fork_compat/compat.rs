//! Wrappers over upstream's analogs (`mapping.md` verdict `compat`), in the fork's shapes, and
//! the compat hooks.
//!
//! Elsewhere: `native_item_fluid`, `native_item_fluid_fill`, `native_item_fluid_drain` are the
//! fork's own again (`game/blocks/native_item_fluids.rs`), `native_client_action_bar` is in
//! `fork.rs`, and upstream kept `native_item_stack` (`native_hooks/base.rs`).
//!
//! Not here yet, as their analogs take a value the fork's shape cannot give (S3 decides, or a
//! bundle moves to upstream's helper): `native_container_slots`, `native_extract_item`,
//! `native_insert_item` (upstream's `container_*` work inside a `Transaction`),
//! `native_force_chunk`, `native_force_chunk_in` (`chunk_set_mod_forced` takes a
//! `TicketController` from the mod's own registration, not an owner string),
//! `native_server_block_in_tag` (an engine `BlockState`), `native_server_item_in_tag` (no
//! upstream item-tag read), `native_set_player_status` (experience has no setter upstream),
//! `native_teleport_player` (`teleport_to_level` takes a `Target`).
use super::*;

impl Game<'_> {
    /// The block at `pos` in the level in place, by name (`minecraft:stone`).
    pub(in crate::game) fn native_block(&self, pos: [i32; 3]) -> String {
        self.res
            .facts()
            .get(self.world.block(pos[0], pos[1], pos[2]))
            .name
            .to_string()
    }
    /// What the block at `pos` drops, as upstream's `level_block_drops` loots it, into
    /// `drops` (left as it is when the block is not loaded). `state` and `explosion` were the
    /// fork's own; upstream loots the state in the world, as a tool or player breaks it.
    pub(in crate::game) fn native_block_drops(
        &mut self,
        pos: [i32; 3],
        _state: BlockStateId,
        player: Option<u128>,
        tool: Option<&ItemStack>,
        _explosion: Option<f32>,
        drops: &mut Vec<ItemStack>,
    ) {
        let Some(level) = self.fork_level_here() else {
            return;
        };
        let entity = player.and_then(|uuid| self.fork_player(uuid)).map(PlayerRef::entity);
        if let Some(looted) = self.level_block_drops(level, pos, tool, entity) {
            *drops = looted;
        }
    }
    /// A property of the block at `pos` in the level in place (`facing`).
    pub(in crate::game) fn native_block_property(&self, pos: [i32; 3], key: &str) -> Option<String> {
        self.level_block_property(self.fork_level_here()?, pos, key)
    }
    /// Hurts entity `target` of level `dimension` by `amount` of `damage_type`, from
    /// `attacker`: whether it took the hit.
    pub(in crate::game) fn native_hurt_entity(
        &mut self,
        dimension: &str,
        target: u32,
        amount: f32,
        damage_type: &str,
        attacker: Option<u32>,
    ) -> bool {
        let _ = dimension;
        let Some(epoch) = self.native_lifecycle.epoch() else {
            return false;
        };
        let attacker = attacker.map(|id| EntityRef::new(epoch, id));
        self.entity_hurt(
            EntityRef::new(epoch, target),
            damage_type,
            amount,
            attacker,
            attacker,
        )
        .unwrap_or(false)
    }
    /// The block light and the sky light at `pos` in the level in place.
    pub(in crate::game) fn native_light(&self, pos: [i32; 3]) -> (u8, u8) {
        let Some(level) = self.fork_level_here() else {
            return (0, 15);
        };
        let layer = |layer| {
            self.level_brightness(level, layer, pos)
                .map_or(0, |light| light.clamp(0, 15) as u8)
        };
        (layer(mod_api::LightLayer::Block), layer(mod_api::LightLayer::Sky))
    }
    /// Breaks the block at `pos` in the level in place as a machine does, with its drops
    /// (`collect`: handed back rather than dropped).
    pub(in crate::game) fn native_machine_destroy_block(
        &mut self,
        pos: [i32; 3],
        tool: Option<&ItemStack>,
        collect: bool,
    ) -> Option<Vec<ItemStack>> {
        let level = self.fork_level_here()?;
        self.level_destroy_block(level, pos, true, tool, collect)
    }
    /// A copy of the mod block entity at `pos` in the level in place.
    pub(in crate::game) fn native_mod_block_entity(&self, pos: [i32; 3]) -> Option<ModBlockEntity> {
        match self.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
            Some(BlockEntity::Mod(m)) => Some((**m).clone()),
            _ => None,
        }
    }
    /// Sends `data` on channel `channel` to player `player`, as a payload of that id
    /// (`player_send_payload`: the channel must be declared as one).
    pub(in crate::game) fn native_net_send_to_player(
        &mut self,
        player: u128,
        channel: &str,
        data: &[u8],
    ) -> Result<(), String> {
        let target = self
            .fork_player(player)
            .ok_or_else(|| format!("player {player:032x} is not on"))?;
        self.player_send_payload(
            target,
            &mod_api::PayloadId(channel.to_owned()),
            mod_api::PayloadBytes(data.to_vec()),
        )
        .map(|_| ())
        .map_err(|e| format!("{e:?}"))
    }
    /// Sends `data` on channel `channel` to the server (`send_payload_to_server`).
    pub(in crate::game) fn native_net_send_to_server(
        &mut self,
        channel: &str,
        data: &[u8],
    ) -> Result<(), String> {
        self.send_payload_to_server(
            &mod_api::PayloadId(channel.to_owned()),
            mod_api::PayloadBytes(data.to_vec()),
        )
        .map(|_| ())
        .map_err(|e| format!("{e:?}"))
    }
    /// What player `uuid` wears or holds in `slot`.
    pub(in crate::game) fn native_player_equipment(
        &self,
        uuid: u128,
        slot: mod_api::EquipmentSlot,
    ) -> Option<ItemStack> {
        let player = self.fork_player(uuid)?;
        self.living_equipment(mod_api::LivingRef::new(player.entity()), slot)
    }
    /// The UUID and name of the online player named `name` (any case). Upstream reads online
    /// players only; the fork's profile cache is gone.
    pub(in crate::game) fn native_profile_by_name(&self, name: &str) -> Option<(u128, String)> {
        let profile = self.player_profile(self.native_player_by_name(name)?)?;
        Some((profile.uuid, profile.name))
    }
    /// The name of online player `uuid`.
    pub(in crate::game) fn native_profile_by_uuid(&self, uuid: u128) -> Option<String> {
        Some(self.player_profile(self.fork_player(uuid)?)?.name)
    }
    /// Whether the acting player sneaks.
    pub(in crate::game) fn native_sneaking(&self) -> bool {
        self.native_acting_player()
            .and_then(|player| self.entity_is_shift_key_down(player.entity()))
            .unwrap_or(false)
    }
    /// A GUI sprite's size in its own pixels (`gui_sprite`).
    pub(in crate::game) fn native_sprite_size(&self, name: &str) -> Option<[f32; 2]> {
        let id = mod_api::Identifier::parse(name).ok()?;
        let region = self.gui_sprite(&id)?;
        Some([region.size[0] as f32, region.size[1] as f32])
    }
    /// Updates the neighbours of `pos` in level `dimension` as a signal change there does.
    pub(in crate::game) fn native_update_signal_neighbors(
        &mut self,
        dimension: &str,
        pos: [i32; 3],
    ) -> bool {
        let Some(level) = self.fork_level(dimension) else {
            return false;
        };
        let Some(source) = self.level_block_state(level, pos) else {
            return false;
        };
        self.level_update_neighbors(level, pos, source, mod_api::NeighbourUpdate::default())
            .unwrap_or(false)
    }

    // --- compat hooks: injection targets for the mixin bundles (S3), no trait-mod dispatch ---

    /// The fork's mod list pages (`native_mods`). Upstream's mod settings are declared data
    /// (`content/mod_settings.rs`); nothing in the engine asks this yet.
    pub(in crate::game) fn native_mods(&mut self, mods: &mut Vec<mod_api::NativeMod>) {
        let _ = mods;
    }
    /// A mod's settings buttons (`native_settings`); upstream declares settings instead.
    pub(in crate::game) fn native_settings(&mut self, mod_id: &str, buttons: &mut Vec<String>) {
        let _ = (mod_id, buttons);
    }
    /// A click on a mod's settings button (`native_settings_click`); upstream hears
    /// `setting_changed`.
    pub(in crate::game) fn native_settings_click(&mut self, mod_id: &str, button: usize) {
        let _ = (mod_id, button);
    }
    /// Bytes on channel `channel` from player `player` (`native_net_from_client`); upstream's
    /// payloads go to the handlers a mod registers (`PayloadRegistrar`).
    pub(in crate::game) fn native_net_from_client(&mut self, player: u128, channel: &str, data: &[u8]) {
        let _ = (player, channel, data);
    }
    /// Bytes on channel `channel` from the server (`native_net_from_server`).
    pub(in crate::game) fn native_net_from_server(&mut self, channel: &str, data: &[u8]) {
        let _ = (channel, data);
    }
    /// A mod item as a fluid container: its capacity in millibuckets (`native_item_fluid_tank`,
    /// API 147 at the merge base, gone upstream). `native_item_tank_capacity` asks it.
    pub(in crate::game) fn native_item_fluid_tank(&self, item: &str, capacity: &mut Option<i32>) {
        let _ = (item, capacity);
    }
}
