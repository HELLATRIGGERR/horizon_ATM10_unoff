//! AE2WTLib's Restock, Pick Block and stow for the acting player
//! (`AE2wtlibEvents.restock`, `pickBlock`, `MagnetHandler.sendRestockAble`,
//! `RestockHotkeyAction`, `StowHotkeyAction`), and the hotbar's counts on
//! this client (`RestockAmountPacket`, `GuiMixin`). AE2WTLib 19.5.1 by
//! Mari023, https://github.com/Mari023/AE2WirelessTerminalLibrary tag
//! `19.5.1`, MIT; see `ae2/addons/ae2wtlib/`. The arithmetic is
//! `horizon_client::ae2x::ae2wtlib::restock`.
//!
//! The restock runs where AE2WTLib's events fire, with the player's state in
//! the game's (a guest's turn, `as_guest`): `useItemOn` and the
//! `RightClickBlock` before it, `EntityInteractSpecific`, `LivingEntityUseItemEvent.Finish`,
//! `ArrowNockEvent`, `ArrowLooseEvent` and `ServerPlayer.drop`.
use super::magnet::{crafting, located};
use crate::game::{Game, ae2_global::GridAddress, ae2_hotkeys, ae2_lab, ae2_wireless, lan::LanHost};
use horizon_client::{
    ae2x::{
        ae2wtlib::{magnet, restock},
        defs::TerminalKind,
        wireless,
    },
    items::ItemStack,
    lan::guest::Guest,
    text::{GREEN, RED, Text},
};
use horizon_protocol::packets::{
    configuration::CustomPayload,
    play::{Clientbound, SystemChat},
};
use std::collections::BTreeMap;

/// `RestockHotkeyAction`'s and `StowHotkeyAction`'s ids (`HotkeyActions.register`).
pub(in crate::game) const RESTOCK_HOTKEY: &str = "ae2wtlib_restock";
pub(in crate::game) const STOW_HOTKEY: &str = "ae2wtlib_stow";
/// `RestockAmountPacket`: the network's amount of each item the player
/// carries, a JSON object of item id to amount.
pub(in crate::game) const RESTOCK_AMOUNTS: &str = "ae2wtlib:restock_amounts";

/// The player a restock or a pick is for (`CraftingTerminalHandler`): its
/// crafting terminal, where it stands, and whether it is creative.
pub(in crate::game) struct Holder {
    terminal: Option<ItemStack>,
    position: [f64; 3],
    creative: bool,
}
impl Holder {
    fn of_guest(g: &Guest) -> Self {
        let terminal = if g.spectator() { None } else { located(g).map(|(_, stack)| stack.clone()) };
        Self { terminal, position: g.position, creative: g.creative() }
    }
}

/// What a pick from the network came to.
pub(in crate::game) enum Pick {
    /// The stack is in the hotbar, selected.
    Picked,
    /// None in the network and craft if missing: `CraftAmountMenu` for this key.
    Craft(ae2_cell_core::ItemKey),
    Nothing,
}

impl Game<'_> {
    /// The acting player as the restock sees it.
    fn ae2wtlib_acting(&self) -> Holder {
        let terminal = (!self.spectator())
            .then(|| crafting().and_then(|t| ae2_hotkeys::locate_in(&self.inventory, self.offhand.as_ref(), t)))
            .flatten()
            .and_then(|slot| self.stack_in(slot).cloned());
        let position = self.player.as_ref().map_or([0.0; 3], |p| [p.position.x, p.position.y, p.position.z]);
        Holder { terminal, position, creative: self.creative }
    }
    /// `CraftingTerminalHandler.getTargetGrid` with `inRange` (`powered`: the
    /// link status, which wants power unless the range costs none or the
    /// player is creative) or without (Pick Block asks the grid alone).
    fn ae2wtlib_grid(&self, holder: &Holder, powered: bool) -> Option<GridAddress> {
        let terminal = holder.terminal.as_ref()?;
        let (dim, target) = ae2_wireless::target(terminal)?;
        let (anchor, distance) = self.ae2_wireless_reach_from(holder.position, &dim, target)?;
        if powered {
            let free = horizon_client::ae2_common_config::wireless().link.drain(distance) <= 0.0 || holder.creative;
            if !free && !ae2_wireless::energy(terminal).is_ok_and(|(power, _)| power > 0.0) {
                return None;
            }
        }
        self.ae2_local_address(anchor)
    }
    /// `getStorageService().getInventory().insert`, unpowered, the formation
    /// planes included: what the network took.
    fn ae2wtlib_insert(&mut self, address: GridAddress, stack: &ItemStack, simulate: bool) -> u64 {
        let snapshot = self.ae2_snapshot_blocks();
        let mut plan = crate::game::ae2_buses::FormationPlan::default();
        let moved = self.ae2_grid_insert_uncharged_staged_encoded(address, stack, u64::from(stack.count), &mut plan);
        match moved {
            Ok(moved) if simulate => {
                self.ae2_restore_blocks(snapshot);
                moved
            }
            Ok(moved) if self.ae2_commit_formation(plan).is_ok() => moved,
            Ok(_) | Err(_) => {
                self.ae2_restore_blocks(snapshot);
                0
            }
        }
    }
    /// `getInventory().extract`, unpowered: how many came.
    fn ae2wtlib_extract(&mut self, address: GridAddress, item: &ItemStack, amount: u64, simulate: bool) -> u64 {
        let Ok(key) = ae2_lab::key(item) else { return 0 };
        self.ae2_grid_extract_uncharged_encoded(address, &key, amount, simulate).ok().flatten().map_or(0, |s| u64::from(s.count))
    }
    fn ae2wtlib_no_restock(&self, stack: &ItemStack) -> bool {
        self.res.items().item_tags.get(restock::NO_RESTOCK).is_some_and(|tag| tag.contains(&*stack.item))
    }

    /// `AE2wtlibEvents.restock(player, item, now, setStack)`: not creative,
    /// a restockable item, the crafting terminal in range with restock on;
    /// the network fills the slot that now holds `now` to a full stack of
    /// `item`, or takes what is past one. The slot's new stack when anything
    /// moved.
    fn ae2wtlib_restock(&mut self, holder: &Holder, item: &ItemStack, now: Option<&ItemStack>) -> Option<Option<ItemStack>> {
        if self.remote.is_some() || holder.creative || item.count == 0 || self.ae2wtlib_no_restock(item) {
            return None;
        }
        if !magnet::settings(holder.terminal.as_ref()?).2 {
            return None;
        }
        let address = self.ae2wtlib_grid(holder, true)?;
        let change = restock::change(item, now, self.res.items().stack_max(item))?;
        let moved = if change > 0 {
            i64::try_from(self.ae2wtlib_extract(address, item, change.unsigned_abs(), false)).unwrap_or(0)
        } else {
            let excess = ItemStack { count: u32::try_from(-change).unwrap_or(u32::MAX), ..item.clone() };
            -i64::try_from(self.ae2wtlib_insert(address, &excess, false)).unwrap_or(0)
        };
        (moved != 0).then(|| restock::restocked(item, now, moved))
    }
    /// `UpdateRestockPacket`: the guest whose turn it is gets its restocked
    /// slot even where its client already counted the use (a slot back at
    /// the count its client last heard of is no change to the tick's sync).
    fn ae2wtlib_restocked(&mut self) {
        if let Some(conn) = self.lan_acting {
            self.ae2x.ae2wtlib.resync.insert(conn);
        }
    }
    /// The inventories of guests whose slots a restock changed, sent.
    pub(in crate::game) fn lan_ae2wtlib_resync(&mut self, host: &mut LanHost, gi: usize) {
        let Some(conn) = host.hub.guests.get(gi).map(|g| g.conn) else { return };
        if self.ae2x.ae2wtlib.resync.remove(&conn) {
            host.hub.send_inventory(gi);
        }
    }
    /// A restock of the acting player's slot as it stands (`restock(item,
    /// item, ...)`: `RightClickBlock`, `EntityInteractSpecific`, the arrows,
    /// the drop key).
    pub(in crate::game) fn ae2wtlib_restock_slot(&mut self, slot: usize) {
        let Some(item) = self.stack_in(slot).cloned() else { return };
        let holder = self.ae2wtlib_acting();
        if let Some(next) = self.ae2wtlib_restock(&holder, &item, Some(&item)) {
            *self.stack_in_mut(slot) = next;
            self.ae2wtlib_restocked();
        }
    }
    /// `ServerPlayerGameModeMixin`: as `useItemOn` returns, the stack it was
    /// given (`original.copy()`), which the hand still holds unless the use
    /// replaced or used it up.
    pub(in crate::game) fn ae2wtlib_use_on_returned(&mut self, slot: usize, before: Option<&ItemStack>) {
        let Some(now) = self.stack_in(slot).cloned() else { return };
        if before.is_some_and(|b| b.same_item_same_components(&now)) {
            self.ae2wtlib_restock_slot(slot);
        }
    }
    /// `LivingEntityUseItemEvent.Finish`: `item` as it was before its use
    /// finished, the hand's result now (a stack used up is filled again, a
    /// bowl is left).
    pub(in crate::game) fn ae2wtlib_use_finished(&mut self, item: &ItemStack) {
        let slot = self.held_slot();
        let now = self.stack_in(slot).cloned();
        let holder = self.ae2wtlib_acting();
        if let Some(next) = self.ae2wtlib_restock(&holder, item, now.as_ref()) {
            *self.stack_in_mut(slot) = next;
            self.ae2wtlib_restocked();
        }
    }
    /// `ServerPlayerMixin.restockDrop`: after the drop key dropped something,
    /// the selected stack left (`dropped` is how many were there before).
    pub(in crate::game) fn ae2wtlib_dropped(&mut self, before: u32) {
        let slot = self.selected;
        if self.inventory[slot].as_ref().is_some_and(|s| s.count > 0 && s.count < before) {
            self.ae2wtlib_restock_slot(slot);
        }
    }

    /// `AE2wtlibEvents.pickBlock(player, stack)` for a block (or entity)
    /// the acting player picks and holds nowhere (`MinecraftMixin`: not
    /// creative, not a spectator): with pick block on, the network's stack
    /// goes into the suitable hotbar slot, whose stack the network takes
    /// first; with none there and craft if missing, the item's craft amount.
    pub(in crate::game) fn ae2wtlib_pick_block(&mut self, stack: &ItemStack) -> Pick {
        if self.remote.is_some() || self.creative || self.spectator() {
            return Pick::Nothing;
        }
        let holder = self.ae2wtlib_acting();
        let Some(terminal) = holder.terminal.as_ref() else { return Pick::Nothing };
        let (pick_block, craft_if_missing, ..) = magnet::settings(terminal);
        if !pick_block {
            return Pick::Nothing;
        }
        let Some(address) = self.ae2wtlib_grid(&holder, false) else { return Pick::Nothing };
        let target = horizon_client::pick_item::suitable_hotbar_slot(&self.inventory[..9], self.selected);
        let to_replace = self.inventory[target].clone();
        if let Some(old) = &to_replace
            && self.ae2wtlib_insert(address, old, true) < u64::from(old.count)
        {
            return Pick::Nothing;
        }
        let max = u64::from(self.res.items().stack_max(stack));
        if self.ae2wtlib_extract(address, stack, max, true) == 0 {
            let Ok(key) = ae2_lab::key(stack) else { return Pick::Nothing };
            let craftable = self.ae2_address_parts(address).is_some_and(|(_, pos)| self.ae2_craftable_outputs(pos).contains(&key));
            return if craft_if_missing && craftable { Pick::Craft(key) } else { Pick::Nothing };
        }
        if let Some(old) = to_replace {
            let inserted = u32::try_from(self.ae2wtlib_insert(address, &old, false)).unwrap_or(u32::MAX);
            if inserted < old.count {
                self.inventory[target] = Some(ItemStack { count: old.count - inserted, ..old });
                return Pick::Nothing;
            }
        }
        let extracted = u32::try_from(self.ae2wtlib_extract(address, stack, max, false)).unwrap_or(0);
        if extracted == 0 {
            self.inventory[target] = None;
            return Pick::Nothing;
        }
        self.inventory[target] = Some(ItemStack { count: extracted, ..stack.clone() });
        self.selected = target;
        Pick::Picked
    }

    /// `CraftAmountMenu.open(player, locator, what, 1)` over the guest's
    /// crafting terminal: AE2's craft amount screen, its back button the
    /// terminal. A universal terminal turns to its crafting terminal.
    pub(in crate::game) fn lan_ae2wtlib_pick_craft(&mut self, host: &mut LanHost, conn: u32, key: ae2_cell_core::ItemKey) {
        let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) else { return };
        let Some((slot, stack)) = located(&host.hub.guests[gi]) else { return };
        let Some((target_dim, target)) = ae2_wireless::target(stack) else { return };
        let Some((anchor, _)) = self.ae2_wireless_reach(host, conn, &target_dim, target) else { return };
        let Some(stack) = ae2_wireless::source_stack_mut(&mut host.hub.guests[gi], slot) else { return };
        if let Some(terminal) = crafting() {
            wireless::set_current(stack, terminal);
        }
        let source_owner = horizon_client::ae2_stack_identity::StackRef::capture(stack);
        let ctx = ae2_wireless::WirelessContext { source: slot, target_dim, target, kind: TerminalKind::Crafting, idle: 0, source_owner };
        self.lan_open_terminal_craft_confirm(host, conn, anchor, key, false, None, Some(ctx));
    }

    /// `RestockHotkeyAction.run`: the crafting terminal's restock turns, and
    /// the action bar says how it stands. False without a crafting terminal.
    pub(in crate::game) fn lan_ae2wtlib_restock_hotkey(&mut self, host: &mut LanHost, conn: u32) -> bool {
        let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn && !g.spectator()) else { return false };
        let Some((slot, _)) = located(&host.hub.guests[gi]) else { return false };
        let Some(stack) = ae2_wireless::source_stack_mut(&mut host.hub.guests[gi], slot) else { return false };
        let on = !magnet::settings(stack).2;
        magnet::set_restock(stack, on);
        let state = Text::translate(if on { "gui.ae2wtlib.on" } else { "gui.ae2wtlib.off" }, &[]).with_color(if on { GREEN } else { RED });
        let text = Text::translate("gui.ae2wtlib.restock", &[state]);
        host.hub.send_to(conn, &Clientbound::SystemChat(SystemChat { content: horizon_client::lan::data::component_nbt(&text.component()), overlay: true }));
        host.hub.send_inventory(gi);
        true
    }
    /// `StowHotkeyAction.run`: the main hand's stack into the network the
    /// crafting terminal reaches, as much as it takes; not an enchanted one
    /// (`isNotReplaceableByPickAction`).
    pub(in crate::game) fn lan_ae2wtlib_stow_hotkey(&mut self, host: &mut LanHost, conn: u32) -> bool {
        let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn && !g.spectator()) else { return false };
        let holder = Holder::of_guest(&host.hub.guests[gi]);
        let Some(address) = self.ae2wtlib_grid(&holder, true) else { return false };
        let g = &host.hub.guests[gi];
        let Some(stack) = g.inventory.get(g.selected).cloned().flatten().filter(|s| s.count > 0) else { return false };
        if !stack.enchantments().is_empty() {
            return false;
        }
        let inserted = u32::try_from(self.ae2wtlib_insert(address, &stack, false)).unwrap_or(u32::MAX).min(stack.count);
        let g = &mut host.hub.guests[gi];
        let slot = g.selected;
        g.inventory[slot] = (stack.count > inserted).then(|| ItemStack { count: stack.count - inserted, ..stack });
        host.hub.send_inventory(gi);
        true
    }

    /// `MagnetHandler.sendRestockAble`, once a tick with the magnet: with
    /// restock on (on the ticking stack and the crafting terminal) and in
    /// range, the network's amount of each item the guest carries (the first
    /// stack of each item, as its key) goes to its client.
    pub(in crate::game) fn lan_ae2wtlib_restock_amounts(&mut self, host: &mut LanHost, gi: usize, ticking: &ItemStack) {
        let Some(g) = host.hub.guests.get(gi) else { return };
        if g.creative() || !magnet::settings(ticking).2 {
            return;
        }
        let holder = Holder::of_guest(g);
        if !holder.terminal.as_ref().is_some_and(|t| magnet::settings(t).2) {
            return;
        }
        let Some(address) = self.ae2wtlib_grid(&holder, true) else { return };
        let Ok(stock) = self.ae2_grid_inventory_encoded(address) else { return };
        let stock: BTreeMap<_, _> = stock.into_iter().collect();
        let g = &host.hub.guests[gi];
        let mut items = BTreeMap::new();
        for stack in g.inventory.iter().chain(g.armor.iter()).chain(std::iter::once(&g.offhand)).flatten() {
            if stack.count == 0 || items.contains_key(&*stack.item) || self.ae2wtlib_no_restock(stack) {
                continue;
            }
            let amount = ae2_lab::key(stack).ok().and_then(|key| stock.get(&key).copied()).unwrap_or(0);
            items.insert(stack.item.to_string(), amount);
        }
        let data = serde_json::to_vec(&items).unwrap_or_default();
        let conn = g.conn;
        host.hub.send_to(conn, &Clientbound::CustomPayload(CustomPayload { channel: RESTOCK_AMOUNTS.into(), data }));
    }

    /// `RestockAmountPacket` on this client (`setRestockAbleItems`).
    pub(in crate::game) fn ae2wtlib_restock_amounts_payload(&mut self, data: &[u8]) {
        if let Ok(items) = serde_json::from_slice::<BTreeMap<String, u64>>(data) {
            self.ae2x.ae2wtlib.restock_amounts = items;
        }
    }
    /// `GuiMixin.restockOverlay` for this client's hotbar slot holding
    /// `stack`: the count it shows in place of the stack's, when restock is
    /// on for the crafting terminal this player carries (`checkTerminal`).
    /// Without one the counts are forgotten (`invalidateCache`).
    pub(in crate::game) fn ae2wtlib_hotbar_count(&self, stack: &ItemStack) -> Option<String> {
        if self.creative || self.ae2x.ae2wtlib.restock_amounts.is_empty() {
            return None;
        }
        let terminal = crafting().and_then(|t| ae2_hotkeys::locate_in(&self.inventory, self.offhand.as_ref(), t)).and_then(|slot| self.stack_in(slot))?;
        if !magnet::settings(terminal).2 {
            return None;
        }
        let network = self.ae2x.ae2wtlib.restock_amounts.get(&*stack.item).copied();
        restock::hotbar_count(stack, network, |n| crate::game::ae2_terminal_screen::readable(n, 3))
    }
}
