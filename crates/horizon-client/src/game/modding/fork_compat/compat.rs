//! Wrappers over upstream's analogs (`mapping.md` verdict `compat`), in the fork's shapes, and
//! the compat hooks.
//!
//! Elsewhere: `native_item_fluid`, `native_item_fluid_fill`, `native_item_fluid_drain` are the
//! fork's own again (`game/blocks/native_item_fluids.rs`), `native_client_action_bar` is in
//! `fork.rs`, and upstream kept `native_item_stack` (`native_hooks/base.rs`).
//!
//! The fork's container helpers (`native_container_slots`, `native_extract_item`,
//! `native_insert_item`) are the fork's own again, in `game/blocks/machines/native_items.rs`
//! beside upstream's transactional `container_*`, as on the fork's main; `native_force_chunk`
//! and `native_force_chunk_in` are in `fork.rs` with the ticket code.
use super::*;
use std::result::Result;

/// The fork's mod list page entry (`native_mods`), kept for its bundles' shape.
#[allow(dead_code)]
pub(in crate::game) struct NativeMod {
    /// Its id: its settings folder and its keys' lines.
    pub id: String,
    /// `Name 1.0`.
    pub title: String,
    pub description: String,
    /// Its keys: name, label and default key (`key.keyboard.z`).
    pub keys: Vec<[String; 3]>,
}

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
    /// Sends `data` on channel `channel` to player `player`, as a plain payload of that id, at
    /// the end of the tick's `tick_lan` (`flush_fork_net`), from any server hook: one fired while
    /// the host is lent to a turn as well. At most [`FORK_PER_TICK`] a player a server tick. Ok
    /// means queued: a player who is not on by then gets nothing.
    pub(in crate::game) fn native_net_send_to_player(
        &mut self,
        player: u128,
        channel: &str,
        data: &[u8],
    ) -> Result<(), String> {
        if self.remote.is_some() {
            return Err("a client has no players to send to".into());
        }
        let count = self.native_net.fork_to_counts.entry(player).or_default();
        if *count >= super::native_net::FORK_PER_TICK {
            return Err(format!(
                "past {} messages to this player this tick",
                super::native_net::FORK_PER_TICK
            ));
        }
        *count += 1;
        self.native_net
            .fork_to_players
            .push((player, channel.to_owned(), data.to_vec()));
        Ok(())
    }
    /// What `native_net_send_to_player` queued goes to the players still on; after a server
    /// tick's last level pass, the allowances start over.
    pub(in crate::game) fn flush_fork_net(&mut self) {
        let queued = std::mem::take(&mut self.native_net.fork_to_players);
        if let Some(lan) = self.lan.as_ref() {
            for (player, channel, data) in queued {
                if let Some(guest) = lan.hub.online_players().find(|g| g.uuid == player) {
                    guest.send(&horizon_protocol::packets::play::Clientbound::CustomPayload(
                        horizon_protocol::packets::configuration::CustomPayload { channel, data },
                    ));
                }
            }
        }
        if self.level_pass.last {
            self.native_net.fork_to_counts.clear();
        }
    }
    /// Sends `data` on channel `channel` to the server (`send_payload_to_server`).
    pub(in crate::game) fn native_net_send_to_server(
        &mut self,
        channel: &str,
        data: &[u8],
    ) -> Result<(), String> {
        if self.remote.is_none() {
            return Err("not connected to a server".into());
        }
        self.send(horizon_protocol::packets::play::Serverbound::CustomPayload(
            horizon_protocol::packets::configuration::CustomPayload { channel: channel.to_owned(), data: data.to_vec() },
        ));
        Ok(())
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
        self.level_update_neighbors(level, pos, source, mod_api::NeighbourUpdate::Neighbours)
            .unwrap_or(false)
    }

    // --- compat hooks: injection targets for the mixin bundles (S3), no trait-mod dispatch ---

    /// The fork's mod list pages (`native_mods`). Upstream's mod settings are declared data
    /// (`content/mod_settings.rs`); nothing in the engine asks this yet.
    pub(in crate::game) fn native_mods(&mut self, mods: &mut Vec<NativeMod>) {
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
    /// The block entity types a bundle declares (`native_block_entity_types`): push one
    /// `(type id, its blocks)` per type. Upstream makes a mod block entity only for a block a
    /// declared type names, and declares types only through a trait mod's `register()`
    /// (`declare_block_entity_type`); fork main took the type from the `block_entity` hook's
    /// answer (the block's own id). Asked once at start, with the trait mods' declarations
    /// (`native_declared`); a type keeps the blocks some loaded content declares, so a game
    /// staged with a bundle but started without its content mod declares none of its types.
    pub(in crate::game) fn native_block_entity_types(types: &mut Vec<(String, Vec<String>)>) {
        let _ = types;
    }
}

impl Game<'_> {
    /// Whether a block state's name is in the server's merged block tag `tag` (the fork's
    /// `BlockState`, upstream's `block_in_tag` takes a resolved `BlockKey`). An error while
    /// the tags are not built yet.
    pub(in crate::game) fn native_server_block_in_tag(
        &self,
        state: &horizon_world::chunk::BlockState,
        tag: &str,
    ) -> Result<bool, super::native_world_read::NativeBlockReadError> {
        let items = self
            .native_tag_items()
            .ok_or(super::native_world_read::NativeBlockReadError::InactiveContext)?;
        Ok(items.block_in_tag(tag, &state.name))
    }
    /// Whether an item stack is in the server's merged item tag `tag` (an empty stack is
    /// `minecraft:air`); upstream has no item tag read. An error while the tags are not built.
    pub(in crate::game) fn native_server_item_in_tag(
        &self,
        stack: &ItemStack,
        tag: &str,
    ) -> Result<bool, super::native_world_read::NativeBlockReadError> {
        let items = self
            .native_tag_items()
            .ok_or(super::native_world_read::NativeBlockReadError::InactiveContext)?;
        let item = if stack.count == 0 {
            "minecraft:air"
        } else {
            &stack.item
        };
        Ok(items.item_in_tag(tag, item))
    }
    /// Sets what is given of the player `uuid`'s health, food (level 0 to 20, saturation,
    /// exhaustion) and experience (level, progress 0 to 1, total points); None leaves it.
    /// Health is capped at their maximum; 0 or less kills them as `/kill` does. Food and
    /// experience come before health. False, and nothing set, for a player not reached, a dead
    /// one, a value that is not a number, a progress outside 0 to 1, or a negative level or
    /// total. Upstream splits it (`player_set_food_level` ...; no experience setter): the fork's
    /// body, in the player's state through `native_as_player`.
    pub(in crate::game) fn native_set_player_status(
        &mut self,
        uuid: u128,
        health: Option<f32>,
        food: Option<(i32, f32, f32)>,
        xp: Option<(i32, f32, i32)>,
    ) -> bool {
        let bad_food = food.is_some_and(|(_, s, e)| !s.is_finite() || !e.is_finite());
        let bad_xp = xp.is_some_and(|(l, p, t)| l < 0 || t < 0 || !(0.0..=1.0).contains(&p));
        if health.is_some_and(|h| !h.is_finite()) || bad_food || bad_xp {
            return false;
        }
        let food = food.map(|(level, saturation, exhaustion)| {
            // FoodData.addExhaustion's cap.
            (level.clamp(0, 20), saturation.max(0.0), exhaustion.clamp(0.0, 40.0))
        });
        self.native_as_player(uuid, |game| {
            if let (Some((level, saturation, exhaustion)), Some(p)) = (food, game.player.as_mut()) {
                p.health.food.food = level;
                p.health.food.saturation = saturation;
                p.health.food.exhaustion = exhaustion;
            }
            if let Some((level, progress, total)) = xp {
                game.experience = horizon_client::entities::Experience {
                    level,
                    progress,
                    total,
                    score: game.experience.score,
                };
            }
            if let Some(health) = health {
                game.journal_player_health();
                if let Some(p) = game.player.as_mut() {
                    if health > 0.0 {
                        let _ = p.set_health(health, false);
                        let _ = p.clamp_health_to_max();
                    } else {
                        // LivingEntity.die(damageSources().generic()).
                        let _ = p.set_health(0.0, false);
                        game.ui.death_cause = Some("death.attack.generic".into());
                    }
                }
            }
        })
    }
    /// Teleports the server player `uuid` to `pos` in `dimension`, facing `yaw` and `pitch`
    /// in degrees, as `/tp` does: upstream's `entity_teleport_to_level` (into another level,
    /// after the current tick). False for a player not on the server, a dimension that isn't
    /// registered, a position outside `/tp`'s bounds, an angle that is not finite.
    pub(in crate::game) fn native_teleport_player(
        &mut self,
        uuid: u128,
        dimension: &str,
        pos: [f64; 3],
        yaw: f32,
        pitch: f32,
    ) -> bool {
        if !yaw.is_finite() || !pitch.is_finite() {
            return false;
        }
        let Some(player) = self.fork_player(uuid) else {
            return false;
        };
        self.entity_teleport_to_level(
            player.entity(),
            dimension,
            pos,
            Some([yaw, pitch.clamp(-90.0, 90.0)]),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::game::native_entity_refs::tests::with_game;

    /// N1: a bundle's message to a player is queued from any hook, the host lent or not, at
    /// most `FORK_PER_TICK` a player a tick, and leaves at the end of the tick, when the
    /// allowance starts over.
    #[test]
    #[ignore = "daily"]
    fn bundle_messages_to_a_player_wait_for_the_tick_end_within_an_allowance() {
        with_game(|game| {
            let per_tick = crate::game::native_net::FORK_PER_TICK;
            for i in 0..per_tick {
                assert_eq!(
                    game.native_net_send_to_player(7, "ftbteams:sync", &[i as u8]),
                    Ok(())
                );
            }
            assert!(game.native_net_send_to_player(7, "ftbteams:sync", &[]).is_err());
            assert!(game.native_net_send_to_player(8, "ftbteams:sync", &[]).is_ok());
            assert_eq!(
                game.native_net.fork_to_players.len(),
                per_tick as usize + 1
            );
            game.flush_fork_net();
            assert!(game.native_net.fork_to_players.is_empty());
            assert!(game.native_net_send_to_player(7, "ftbteams:sync", &[]).is_ok());
        });
    }
}
