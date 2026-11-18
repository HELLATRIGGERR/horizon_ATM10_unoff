//! Forwarders from the fork's `native_<old>` names to upstream's (`mapping.md` verdict
//! `renamed`), in the fork's argument shapes: a dimension id or the level in place for a
//! `LevelRef`, a UUID for a `PlayerRef`, a registry id for a `BlockKey`/`ItemKey`. Where
//! upstream's shape carries more (a refusal reason, a modifier), the fork's answer is derived
//! from it as the fork's helper gave it.
//!
//! Renamed hooks: `native_destroy_block`, `native_frame`, `native_hud` and `native_tick` call
//! upstream's points, for fork code that fires them. A mixin bundle injects into upstream's
//! point itself (S3 rebinds its injections; `mapping.md` names each target). The other eight
//! (`commands`, `interact_entity`, `key`, `menu_button_v2`, `perform_command`, `piston_move`,
//! `place_block`, `player_tick`) changed their arguments' types (`mod_api::Command`,
//! `InteractionResult`, `KeyAction`, `MenuButtonContext`, `CommandSource`, `PistonPreCall`,
//! `EntityRef`/`Placement`, `PlayerTickPostCall`) so that no fork-shaped forwarder can build
//! them; nothing in the engine fires them by the old name.
use super::*;
use std::result::Result;
use std::path::PathBuf;

impl Game<'_> {
    // --- helpers ---

    /// `player_action_bar` by UUID.
    pub(in crate::game) fn native_action_bar(&mut self, uuid: u128, line: Text) -> bool {
        match self.fork_player(uuid) {
            Some(player) => self.player_action_bar(player, line),
            None => false,
        }
    }
    /// A holder's attachments, as upstream's `AttachmentHost::attachments` reads them for a write.
    pub(in crate::game) fn native_attachments(
        &mut self,
        holder: mod_api::Holder,
    ) -> Result<&mut mod_api::Attachments, mod_api::AttachmentError> {
        self.attachment_map(holder, mod_api::AttachmentAccess::Store)
    }
    /// `level_biome_at` in the level in place.
    pub(in crate::game) fn native_biome(&self, pos: [i32; 3]) -> Option<String> {
        self.level_biome_at(self.fork_level_here()?, pos)
    }
    /// `block_entity_sync`.
    pub(in crate::game) fn native_block_entity_sync(&mut self, pos: [i32; 3]) {
        self.block_entity_sync(pos);
    }
    /// `block_in_tag` by block id; an unknown block or tag is in none.
    pub(in crate::game) fn native_block_name_in_tag(
        &self,
        block: &str,
        tag: &str,
    ) -> Result<bool, String> {
        let key = mod_api::__private::block_key(block).ok_or_else(|| format!("unknown block {block}"))?;
        Ok(self.block_in_tag(key, tag).unwrap_or(false))
    }
    /// `close_menu`.
    pub(in crate::game) fn native_close_menu(&mut self, uuid: u128) {
        self.close_menu(uuid);
    }
    /// `cursor`.
    pub(in crate::game) fn native_cursor(&self) -> [f32; 2] {
        self.cursor()
    }
    /// `level_day_time` in the level in place; 0 outside a world.
    pub(in crate::game) fn native_day_time(&self) -> i64 {
        self.fork_level_here()
            .and_then(|level| self.level_day_time(level))
            .unwrap_or(0)
    }
    /// `block_state_default` by block id.
    pub(in crate::game) fn native_default_state(&self, block: &str) -> Option<BlockStateId> {
        self.block_state_default(mod_api::__private::block_key(block)?)
    }
    /// `dimension`: the level in place.
    pub(in crate::game) fn native_dimension(&self) -> &'static str {
        self.dimension()
    }
    /// `give` by item id.
    pub(in crate::game) fn native_give(&mut self, item: &str, count: u32) -> Result<(), String> {
        let key = mod_api::__private::item_key(item).ok_or_else(|| format!("unknown item {item}"))?;
        self.give(key, count)
    }
    /// `player_give_stack` by UUID: what did not fit back (all of it when they are not on), as
    /// the fork's helper answered.
    pub(in crate::game) fn native_give_stack(
        &mut self,
        uuid: u128,
        stack: ItemStack,
    ) -> Option<ItemStack> {
        let Some(player) = self.fork_player(uuid) else {
            return Some(stack);
        };
        match self.player_give_stack(player, stack.clone()) {
            Ok(()) => None,
            Err(_) => Some(stack),
        }
    }
    /// `has_control_down`.
    pub(in crate::game) fn native_has_control_down(&self) -> bool {
        self.has_control_down()
    }
    /// `has_shift_down`.
    pub(in crate::game) fn native_has_shift_down(&self) -> bool {
        self.has_shift_down()
    }
    /// `held` as the fork's tuple: the item id, the count, and whether it is plain (no damage,
    /// no components).
    pub(in crate::game) fn native_held(&self) -> Option<(String, u32, bool)> {
        let stack = self.held()?;
        let plain = stack.damage == 0 && stack.extra.is_none();
        Some((stack.item.to_string(), stack.count, plain))
    }
    /// `instance_dir`.
    pub(in crate::game) fn native_instance_dir(&self) -> PathBuf {
        self.instance_dir()
    }
    /// `max_stack_size` by item id.
    pub(in crate::game) fn native_max_stack_size(&self, item: &str) -> Option<u32> {
        self.max_stack_size(mod_api::__private::item_key(item)?)
    }
    /// `mod_image`.
    pub(in crate::game) fn native_mod_image(&mut self, width: u32, height: u32, rgba: &[u8]) -> bool {
        self.mod_image(width, height, rgba)
    }
    /// `mod_key`: the bound key's name, `default` when the mapping has none.
    pub(in crate::game) fn native_mod_key(&self, mod_id: &str, name: &str, default: &str) -> String {
        self.mod_key(mod_id, name)
            .map(|(key, _)| key)
            .unwrap_or_else(|| default.to_owned())
    }
    /// `mod_screen`.
    pub(in crate::game) fn native_mod_screen(&self) -> Option<&str> {
        self.mod_screen()
    }
    /// `mod_screen_close`.
    pub(in crate::game) fn native_mod_screen_close(&mut self, mod_id: &str) {
        self.mod_screen_close(mod_id);
    }
    /// `mod_screen_open`.
    pub(in crate::game) fn native_mod_screen_open(&mut self, mod_id: &str) -> bool {
        self.mod_screen_open(mod_id)
    }
    /// `level_best_neighbor_signal` in level `dimension`; 0 where it is not loaded.
    pub(in crate::game) fn native_neighbor_signal(&mut self, dimension: &str, pos: [i32; 3]) -> i32 {
        self.fork_level(dimension)
            .and_then(|level| self.level_best_neighbor_signal(level, pos))
            .unwrap_or(0)
    }
    /// `on`.
    pub(in crate::game) fn native_on(&self, mod_id: &str) -> bool {
        self.on(mod_id)
    }
    /// `open_menu`, closing the old screen first as vanilla does.
    pub(in crate::game) fn native_open_menu(
        &mut self,
        uuid: u128,
        rows: u8,
        title: Text,
        items: Vec<Option<ItemStack>>,
    ) -> Option<i32> {
        self.open_menu(uuid, rows, title, items, mod_api::MenuReplace::CloseFirst)
    }
    /// `panel_items` from the fork's stacks (ids resolved; an unknown id is left out).
    pub(in crate::game) fn native_panel_items(&mut self, mod_id: &str, items: Vec<NativeStack>) {
        let items = items
            .into_iter()
            .filter_map(|s| {
                Some(mod_api::Stack {
                    item: mod_api::__private::item_key(&s.item)?,
                    count: s.count,
                    components: s.components,
                })
            })
            .collect();
        self.panel_items(mod_id, items);
    }
    /// `panel_style` (upstream's `PanelStyle` is the fork's `NativePanelStyle`).
    pub(in crate::game) fn native_panel_style(&mut self, mod_id: &str, style: mod_api::PanelStyle) {
        self.panel_style(mod_id, style);
    }
    /// `player_food` by UUID, as the fork's tuple (level, saturation, exhaustion).
    pub(in crate::game) fn native_player_food(&self, uuid: u128) -> Option<(i32, f32, f32)> {
        let food = self.player_food(self.fork_player(uuid)?)?;
        Some((food.level, food.saturation, food.exhaustion))
    }
    /// `player_stat` by UUID and stat type path (`minecraft:mined` or `mined`).
    pub(in crate::game) fn native_player_stat(&self, uuid: u128, stat_type: &str, key: &str) -> Option<i32> {
        use mod_api::StatType as S;
        let path = stat_type.strip_prefix("minecraft:").unwrap_or(stat_type);
        let stat_type = [
            S::Mined,
            S::Crafted,
            S::Used,
            S::Broken,
            S::PickedUp,
            S::Dropped,
            S::Killed,
            S::KilledBy,
            S::Custom,
        ]
        .into_iter()
        .find(|t| t.path() == path)?;
        self.player_stat(self.fork_player(uuid)?, stat_type, key)
    }
    /// `playing`.
    pub(in crate::game) fn native_playing(&self) -> bool {
        self.playing()
    }
    /// `recipe_ingredient` of a JSON value; none for one that does not read.
    pub(in crate::game) fn native_recipe_ingredient(&self, json: &serde_json::Value) -> Option<Ingredient> {
        self.recipe_ingredient(&json.to_string()).ok().flatten()
    }
    /// `recipe_result` of a JSON value; none for one that does not read.
    pub(in crate::game) fn native_recipe_result(&self, json: &serde_json::Value) -> Option<ItemStack> {
        self.recipe_result(&json.to_string()).ok().flatten()
    }
    /// `recipes_of_type`: upstream hands out a shared snapshot rather than a borrow of the table.
    pub(in crate::game) fn native_recipes_of_type(
        &self,
        kind: &str,
    ) -> std::sync::Arc<[crate::crafting::RecipeSnapshot]> {
        self.recipes_of_type(kind)
    }
    /// `screen_pos`.
    pub(in crate::game) fn native_screen_pos(&self) -> Option<[i32; 3]> {
        self.screen_pos()
    }
    /// `screen_title`.
    pub(in crate::game) fn native_screen_title(&self) -> Option<String> {
        self.screen_title()
    }
    /// `set_block_property_with`.
    pub(in crate::game) fn native_set_block_property_with(
        &mut self,
        pos: [i32; 3],
        key: &str,
        value: &str,
        flags: i32,
    ) -> bool {
        self.set_block_property_with(pos, key, value, flags)
    }
    /// `menu_set_items` on the menu the player has open (window 0 is their inventory).
    pub(in crate::game) fn native_set_menu_items(&mut self, uuid: u128, items: Vec<Option<ItemStack>>) {
        let window = self
            .lan
            .as_ref()
            .and_then(|host| host.hub.online_players().find(|g| g.uuid == uuid))
            .and_then(|g| g.menu.as_ref().map(|open| open.window))
            .unwrap_or(0);
        let _ = self.menu_set_items(uuid, window, items);
    }
    /// `surface_heights` (upstream's `Heightmap` is the fork's `NativeHeightmap`).
    pub(in crate::game) fn native_surface_heights(
        &self,
        dimension: &str,
        columns: &[(i32, i32)],
        heightmap: mod_api::Heightmap,
    ) -> Result<Vec<Option<i32>>, String> {
        self.surface_heights(dimension, columns, heightmap)
    }
    /// `player_tell` by UUID.
    pub(in crate::game) fn native_tell(&mut self, uuid: u128, line: Text) -> bool {
        match self.fork_player(uuid) {
            Some(player) => self.player_tell(player, line),
            None => false,
        }
    }
    /// `level_weather` in the level in place: (rain, thunder), none outside a world.
    pub(in crate::game) fn native_weather(&self) -> (f32, f32) {
        self.fork_level_here()
            .and_then(|level| self.level_weather(level))
            .unwrap_or((0.0, 0.0))
    }
    /// `world_dir`.
    pub(in crate::game) fn native_world_dir(&self) -> Option<PathBuf> {
        self.world_dir()
    }
    /// `level_height` in the level in place: (lowest y, height).
    pub(in crate::game) fn native_world_height(&self) -> (i32, i32) {
        self.fork_level_here()
            .and_then(|level| self.level_height(level))
            .unwrap_or((-64, 384))
    }

    // --- hooks, for fork code that fires them ---

    /// `native_break_block`, the experience it drops left as the game's.
    pub(in crate::game) fn native_destroy_block(
        &mut self,
        pos: [i32; 3],
        state: BlockStateId,
        canceled: &mut bool,
    ) {
        let mut exp_to_drop = None;
        self.native_break_block(pos, state, canceled, &mut exp_to_drop);
    }
    /// `native_render_frame_pre` with whole ticks.
    pub(in crate::game) fn native_frame(&mut self, elapsed: std::time::Duration) {
        self.native_render_frame_pre(elapsed, 1.0, 1.0);
    }
    /// `native_render_gui_post`.
    pub(in crate::game) fn native_hud(
        &mut self,
        batch: &mut HudBatch,
        corners: &mut horizon_client::extras_hud::Corners,
    ) {
        self.native_render_gui_post(batch, corners);
    }
    /// `native_client_tick_post`: the window's tick (the server's is `native_server_tick`).
    pub(in crate::game) fn native_tick(&mut self) {
        self.native_client_tick_post();
    }
}
