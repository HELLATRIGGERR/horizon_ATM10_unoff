//! `Game` as the host of the AE2 crates above horizon-client (`ae2_ext::Ae2Host`): the one place
//! they reach the game, and the game's calls into them.
use super::{
    Game, Screen,
    ae2_ext::{Ae2Host, Ae2Snapshot, GridAddress, HandlerCell, PartAddress, Visual},
    lan::LanHost,
};
use horizon_client::{ae2_world::Machine, containers::BlockEntity, items::ItemStack, streaming::Streaming, text::Text, world::World};
use horizon_world::registry::{BlockRegistry, BlockStateId};
use std::{any::Any, path::Path};

/// A direction's index (`Direction as u8`), back; none past the six.
fn direction(face: u8) -> Option<horizon_core::geometry::Direction> {
    horizon_core::geometry::Direction::ALL.get(usize::from(face)).copied()
}

impl Game<'_> {
    /// A chemical handler's insertion or extraction at a grid address of the level the game holds
    /// (hook points `native_chemical_handler_insert`/`_extract`).
    fn ae2_chemical_move(&mut self, at: GridAddress, face: u8, chemical: &str, amount: u64, simulate: bool, extract: bool) -> u64 {
        let (Some((dim, pos)), Some(face)) = (at.local_parts(), direction(face)) else { return 0 };
        if amount == 0 || chemical.is_empty() {
            return 0;
        }
        let amount = amount.min(i64::MAX as u64) as i64;
        let mut moved = None;
        if extract {
            self.native_chemical_handler_extract(dim.id(), [pos.0, pos.1, pos.2], Some(face), chemical, amount, simulate, &mut moved);
        } else {
            self.native_chemical_handler_insert(dim.id(), [pos.0, pos.1, pos.2], Some(face), chemical, amount, simulate, &mut moved);
        }
        moved.unwrap_or(0).clamp(0, amount) as u64
    }
}

impl Ae2Host for Game<'_> {
    fn grid_key_move(&mut self, cable: GridAddress, kind: ae2_cell_core::KeyType, key: &ae2_cell_core::ItemKey, amount: u64, insert: bool, simulate: bool) -> Result<u64, String> {
        self.ae2_grid_key_move_encoded(cable, kind, key, amount, insert, simulate)
    }

    fn chemical_tanks(&self, at: GridAddress, face: u8) -> Option<Vec<mod_api::NativeChemicalTankInfo>> {
        let ((dim, pos), face) = (at.local_parts()?, direction(face)?);
        let mut tanks = None;
        self.native_chemical_handler_tanks(dim.id(), [pos.0, pos.1, pos.2], Some(face), &mut tanks);
        tanks
    }

    fn chemical_insert(&mut self, at: GridAddress, face: u8, chemical: &str, amount: u64, simulate: bool) -> u64 {
        self.ae2_chemical_move(at, face, chemical, amount, simulate, false)
    }

    fn chemical_extract(&mut self, at: GridAddress, face: u8, chemical: &str, amount: u64, simulate: bool) -> u64 {
        self.ae2_chemical_move(at, face, chemical, amount, simulate, true)
    }

    fn is_remote(&self) -> bool {
        self.remote.is_some()
    }

    fn dimension(&self) -> &str {
        self.dim.id()
    }

    fn world(&self) -> &World {
        &self.world
    }

    fn streaming(&mut self) -> &mut Streaming {
        &mut self.streaming
    }

    fn world_root(&self) -> &Path {
        self.world_root.as_path()
    }

    fn known_cell_key(&self, id: &str, kind: ae2_cell_core::KeyType) -> bool {
        self.ae2_known_cell_key(id, kind)
    }

    fn block_entity(&self, address: GridAddress) -> Option<&BlockEntity> {
        self.ae2_block_entity(address)
    }

    fn address_parts(&self, address: GridAddress) -> Option<(&'static str, (i32, i32, i32))> {
        self.ae2_address_parts(address).map(|(dim, pos)| (dim.id(), pos))
    }

    fn local_address(&self, pos: (i32, i32, i32)) -> Option<GridAddress> {
        self.ae2_local_address(pos)
    }

    fn storage_priority(&self, address: GridAddress) -> Result<i32, String> {
        self.ae2_storage_priority(address)
    }

    fn part_machine(&self, part: PartAddress) -> Result<Option<Machine>, String> {
        self.ae2_part_machine(part)
    }

    fn face_index(&self, face: &str) -> Option<u8> {
        super::ae2_grid::face_index(face)
    }

    fn machine_addresses(&self, wanted: &dyn Fn(&Machine) -> bool) -> Vec<GridAddress> {
        let mut found = Vec::new();
        for (dim, world) in self.ae2_loaded_worlds() {
            for (&pos, entity) in world.block_entities.iter() {
                if let BlockEntity::Ae2(machine) = entity
                    && wanted(machine)
                    && let Some(address) = self.ae2_dimension_address(dim, pos)
                {
                    found.push(address);
                }
            }
        }
        found
    }

    fn cell_hosts(&self, anchor: GridAddress) -> Result<(Vec<GridAddress>, Vec<PartAddress>), String> {
        self.ae2_cell_hosts_encoded(anchor)
    }

    fn local_player_uuid(&self) -> u128 {
        self.own_uuid()
    }

    fn acting_player_uuid(&self) -> [i32; 4] {
        self.acting_uuid()
    }

    fn join_name(&self) -> String {
        self.options.player_name()
    }

    fn main_hand_stack(&self) -> Option<&ItemStack> {
        self.stack_in(self.selected)
    }

    fn set_main_hand_stack(&mut self, stack: ItemStack) {
        let selected = self.selected;
        *self.stack_in_mut(selected) = Some(stack);
    }

    fn call_addon(&mut self, id: &str, what: &str, args: &dyn Any) -> Option<Box<dyn Any>> {
        let ext = self.ae2_ext.get()?;
        ext.addon(id)?.call(self, what, args)
    }

    fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    fn spectator(&self) -> bool {
        Game::spectator(self)
    }

    fn shift_down(&self) -> bool {
        self.keys.raw(&super::KeyCode::ShiftLeft)
    }

    fn held(&self) -> Option<&ItemStack> {
        Game::held(self)
    }

    fn consume_held(&mut self) {
        Game::consume_held(self);
    }

    fn block_state_name(&self, state: BlockStateId) -> &str {
        self.res.facts().get(state).name.as_ref()
    }

    fn block_registry(&self) -> &'static BlockRegistry {
        self.pack.assets.registry
    }

    fn level_set(&mut self, pos: [i32; 3], state: BlockStateId) -> Result<(), String> {
        Game::level_set(self, pos, state).map_err(|e| e.to_string())
    }

    fn provider_patterns_changed(&self, address: GridAddress, face: Option<u8>) {
        self.ae2_provider_patterns_changed(address, face);
    }

    fn provider_patterns_changed_local(&self, pos: (i32, i32, i32), face: Option<u8>) {
        self.ae2_provider_patterns_changed_local(pos, face);
    }

    fn hud_atlas(&self) -> &horizon_client::hud::HudAtlas {
        &self.gui.atlas
    }

    fn gui_cursor(&self) -> [f32; 2] {
        [self.cursor.0, self.cursor.1]
    }

    fn translate(&self, key: &str) -> Option<String> {
        self.pack.names.display_get(key)
    }

    fn draw_tooltip(&self, batch: &mut horizon_client::hud::HudBatch, lines: &[Text], at: [f32; 2]) {
        let lines: Vec<(Text, u32)> = lines.iter().map(|line| (line.clone(), u32::MAX)).collect();
        self.draw_tooltip_lines(batch, &lines, at[0], at[1]);
    }

    fn open_addon_screen(&mut self, id: &'static str) {
        self.ae2x.screen = Some(id);
        self.screen = Screen::Ae2Addon;
        self.capture(false);
    }

    fn close_addon_screen(&mut self) {
        self.ae2x.screen = None;
        if self.screen == Screen::Ae2Addon {
            self.screen = Screen::Playing;
            if self.player.is_some() {
                self.capture(true);
            }
        }
    }

    fn send_to_server(&mut self, channel: &str, data: &[u8]) -> bool {
        if self.remote.is_none() {
            return false;
        }
        use horizon_protocol::packets::{configuration::CustomPayload, play::Serverbound};
        self.send(Serverbound::CustomPayload(CustomPayload { channel: channel.into(), data: data.to_vec() }));
        true
    }

    fn action_bar(&mut self, text: Text) {
        self.native_client_action_bar(text, false);
    }
}

/// The game's calls into the addons built above it. Each asks for one addon by its id and does
/// nothing (or answers as the game does without it) when it is not built in. The extension is
/// cloned out of the game for the call, so what the addon asks back may reach the addons again.
impl Game<'_> {
    /// The tick of the addon `id` (`Ae2Addon::tick`).
    pub(super) fn ae2_addon_tick(&mut self, id: &str) {
        let Some(ext) = self.ae2_ext.get() else { return };
        if let Some(addon) = ext.addon(id) {
            addon.tick(self);
        }
    }

    /// The tick of the addon `id` while the server's ticks are frozen (`Ae2Addon::tick_frozen`).
    pub(super) fn ae2_addon_tick_frozen(&mut self, id: &str) {
        let Some(ext) = self.ae2_ext.get() else { return };
        if let Some(addon) = ext.addon(id) {
            addon.tick_frozen(self);
        }
    }

    /// The addon `id`'s snapshot of its own storage (`Ae2Addon::snapshot`).
    pub(super) fn ae2_addon_snapshot(&self, id: &str) -> Option<Box<dyn Ae2Snapshot>> {
        let ext = self.ae2_ext.get()?;
        ext.addon(id)?.snapshot(self)
    }

    /// The addon `id`'s save with the world (`Ae2Addon::save`).
    pub(super) fn ae2_addon_save(&mut self, id: &str) {
        let Some(ext) = self.ae2_ext.get() else { return };
        if let Some(addon) = ext.addon(id) {
            addon.save(self);
        }
    }

    /// Whether the addon `id` leaves the cell mounted (`Ae2Addon::cell_mounted`); yes when it is not built in.
    pub(super) fn ae2_addon_cell_mounted(&self, id: &str, at: GridAddress, slot: usize, stack: &ItemStack, hosts: &[GridAddress]) -> bool {
        let Some(ext) = self.ae2_ext.get() else { return true };
        ext.addon(id).is_none_or(|addon| addon.cell_mounted(self, at, slot, stack, hosts))
    }

    /// The addon `id`'s idle drain of a cell (`Ae2Addon::cell_idle_drain`).
    pub(super) fn ae2_addon_cell_idle_drain(&self, id: &str, at: GridAddress, slot: usize, stack: &ItemStack) -> Option<f64> {
        let ext = self.ae2_ext.get()?;
        ext.addon(id)?.cell_idle_drain(self, at, slot, stack)
    }

    /// The addon `id`'s part of a block's visual (`Ae2Addon::enrich_visual`).
    pub(super) fn ae2_addon_enrich_visual(&self, id: &str, visual: &mut Visual) {
        let Some(ext) = self.ae2_ext.get() else { return };
        if let Some(addon) = ext.addon(id) {
            addon.enrich_visual(self, visual);
        }
    }

    /// The addon `id`'s cell of its own handler (`Ae2Addon::handler_cell`).
    pub(super) fn ae2_addon_handler_cell(&self, id: &str, stack: &ItemStack) -> Option<Result<Box<dyn HandlerCell>, String>> {
        let ext = self.ae2_ext.get()?;
        ext.addon(id)?.handler_cell(self, stack)
    }

    /// The addon `id`'s tooltip lines of an item (`Ae2Addon::item_hover_lines`).
    pub(super) fn ae2_addon_item_hover_lines(&self, id: &str, stack: &ItemStack, translate: &dyn Fn(&str) -> Option<String>) -> Option<Vec<Text>> {
        let ext = self.ae2_ext.get()?;
        ext.addon(id)?.item_hover_lines(self, stack, translate)
    }

    /// The addon `id`'s `/ae2` command (`Ae2Addon::command`).
    pub(super) fn ae2_addon_command(&mut self, id: &str, words: &[&str]) -> Option<Result<serde_json::Value, String>> {
        let ext = self.ae2_ext.get()?;
        ext.addon(id)?.command(self, words)
    }

    /// `useOn` of the held item when it is one of `kits`, at block `pos`, side `face`: the kits
    /// crate's (horizon-ae2x-kits), called as `"kits"`'s `"use"`. True when it replaced a device;
    /// false when the kits are not built in.
    #[cfg(any(feature = "ae2-extendedae", feature = "ae2-advancedae"))]
    pub(super) fn ae2_addon_kit_use(&mut self, kits: &'static [horizon_client::ae2x::kits::Kit], pos: [i32; 3], face: [i32; 3]) -> bool {
        let args: (&'static [horizon_client::ae2x::kits::Kit], [i32; 3], [i32; 3]) = (kits, pos, face);
        Ae2Host::call_addon(self, "kits", "use", &args).and_then(|done| done.downcast::<bool>().ok()).is_some_and(|done| *done)
    }

    /// A LAN guest's custom payload on the addon `id`'s channel, which acts on the stack in the
    /// guest's selected slot (`Ae2Addon::lan_main_hand_payload`); the guest's inventory is sent
    /// again when it changed it. False when the addon is not built in.
    pub(super) fn ae2_addon_lan_main_hand_payload(&self, id: &str, host: &mut crate::game::lan_host_turn::HostTurn, conn: u32, channel: &str, data: &[u8]) -> bool {
        let Some(ext) = self.ae2_ext.get() else { return false };
        let Some(addon) = ext.addon(id) else { return false };
        let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) else { return true };
        let changed = host.hub.guests.update(gi, |guest| {
            let player = horizon_protocol::crypto::uuid_string(guest.uuid);
            let selected = guest.selected;
            let Some(Some(stack)) = guest.inventory.get_mut(selected) else { return false };
            addon.lan_main_hand_payload(channel, data, &player, stack)
        });
        if changed {
            host.hub.send_inventory(gi);
        }
        true
    }
}

/// The addons' item use and own screens (`Screen::Ae2Addon`).
impl Game<'_> {
    /// `Item.use` of the held `stack` by the addons built above, before AE2's cell use
    /// (`Ae2Addon::item_use`): whether the use ends there; None when no addon takes it.
    pub(super) fn ae2_addon_item_use(&mut self, id: &str, stack: &ItemStack) -> Option<bool> {
        let ext = self.ae2_ext.get()?;
        let client = self.serving.is_none() && self.lan_acting.is_none();
        ext.addon(id)?.item_use(self, stack, client)
    }
    /// `Ae2Addon::handler_cell_disassembly` of the addon `id`.
    pub(super) fn ae2_addon_handler_cell_disassembly(&self, id: &str, stack: &ItemStack) -> Option<bool> {
        let ext = self.ae2_ext.get()?;
        ext.addon(id)?.handler_cell_disassembly(self, stack)
    }
    /// The addon screen open now, when `Screen::Ae2Addon` is.
    fn ae2_addon_screen_id(&self) -> Option<&'static str> {
        self.ae2x.screen.filter(|_| self.screen == Screen::Ae2Addon)
    }
    pub(super) fn ae2_addon_screen_active(&self) -> bool {
        self.ae2_addon_screen_id().is_some()
    }
    /// Draws the open addon screen, centred, then its tooltip.
    pub(super) fn ae2_addon_screen_draw(&self, batch: &mut horizon_client::hud::HudBatch) {
        let (Some(id), Some(ext)) = (self.ae2_addon_screen_id(), self.ae2_ext.get()) else { return };
        let Some(screen) = ext.addon(id).and_then(|addon| addon.screen()) else { return };
        let Some((w, h)) = screen.size(self) else { return };
        let (ox, oy) = batch.centered_origin(w, h);
        screen.draw(self, batch, ox, oy);
        screen.tooltip(self, batch);
    }
    /// A mouse press on the open addon screen, at the cursor.
    pub(super) fn ae2_addon_screen_press(&mut self, right: bool) {
        let (Some(id), Some(ext)) = (self.ae2_addon_screen_id(), self.ae2_ext.get()) else { return };
        let Some(screen) = ext.addon(id).and_then(|addon| addon.screen()) else { return };
        let Some((w, h)) = screen.size(self) else { return };
        let Some(size) = self.renderer.as_ref().map(|r| [r.config.width, r.config.height]) else { return };
        let (ox, oy) = self.screen_batch(size).centered_origin(w, h);
        let at = [self.cursor.0 - ox, self.cursor.1 - oy];
        screen.press(self, at, right);
    }
    /// A key on the open addon screen: whether it took it.
    pub(super) fn ae2_addon_screen_key(&mut self, key: winit::keyboard::KeyCode) -> bool {
        let (Some(id), Some(ext)) = (self.ae2_addon_screen_id(), self.ae2_ext.get()) else { return false };
        let Some(screen) = ext.addon(id).and_then(|addon| addon.screen()) else { return false };
        screen.key(self, &format!("{key:?}"))
    }
    /// Text typed on the open addon screen.
    pub(super) fn ae2_addon_screen_text(&mut self, text: &str) {
        let (Some(id), Some(ext)) = (self.ae2_addon_screen_id(), self.ae2_ext.get()) else { return };
        if let Some(screen) = ext.addon(id).and_then(|addon| addon.screen()) {
            screen.text(self, text);
        }
    }
    /// Escape on the open addon screen: it closes (`Screen.onClose`).
    pub(super) fn ae2_addon_screen_close(&mut self) {
        let id = self.ae2x.screen;
        Ae2Host::close_addon_screen(self);
        let (Some(id), Some(ext)) = (id, self.ae2_ext.get()) else { return };
        if let Some(screen) = ext.addon(id).and_then(|addon| addon.screen()) {
            screen.closed(self);
        }
    }
    /// What `horizon-ctl state` reports of the open addon screen.
    pub(super) fn ae2_addon_screen_agent(&self) -> Option<serde_json::Value> {
        let ext = self.ae2_ext.get()?;
        let id = self.ae2_addon_screen_id()?;
        let screen = ext.addon(id)?.screen()?;
        let mut value = screen.agent(self).unwrap_or_else(|| serde_json::json!({}));
        if let Some((w, h)) = screen.size(self)
            && let Some(r) = self.renderer.as_ref()
        {
            let (ox, oy) = self.screen_batch([r.config.width, r.config.height]).centered_origin(w, h);
            value["origin"] = serde_json::json!([ox, oy]);
        }
        value["addon"] = serde_json::json!(id);
        Some(value)
    }
}
