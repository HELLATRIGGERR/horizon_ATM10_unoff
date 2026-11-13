//! `Game` as the host of the AE2 crates above horizon-client (`ae2_ext::Ae2Host`): the one place
//! they reach the game, and the game's calls into them.
use super::{
    Game,
    ae2_ext::{Ae2Host, Ae2Snapshot, GridAddress, HandlerCell, PartAddress, Visual},
    lan::LanHost,
};
use horizon_client::{ae2_world::Machine, containers::BlockEntity, items::ItemStack, streaming::Streaming, text::Text, world::World};
use horizon_world::registry::{BlockRegistry, BlockStateId};
use std::{any::Any, path::Path};

impl Ae2Host for Game<'_> {
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
        self.keys.contains(&super::KeyCode::ShiftLeft)
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
    pub(super) fn ae2_addon_lan_main_hand_payload(&self, id: &str, host: &mut LanHost, conn: u32, channel: &str, data: &[u8]) -> bool {
        let Some(ext) = self.ae2_ext.get() else { return false };
        let Some(addon) = ext.addon(id) else { return false };
        let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) else { return true };
        let guest = &mut host.hub.guests[gi];
        let player = horizon_protocol::crypto::uuid_string(guest.uuid);
        let selected = guest.selected;
        let Some(Some(stack)) = guest.inventory.get_mut(selected) else { return true };
        if addon.lan_main_hand_payload(channel, data, &player, stack) {
            host.hub.send_inventory(gi);
        }
        true
    }
}
