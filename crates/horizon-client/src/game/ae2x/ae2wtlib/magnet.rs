//! AE2WTLib's Magnet Card for a guest (`MagnetHandler.handle` from
//! `ItemWCT.inventoryTick`, `AE2wtlibEvents.insertStackInME` from
//! `ItemEntityPickupEvent.Pre`, `MagnetHotkeyAction`,
//! `TerminalSettingsPacket`), AE2WTLib 19.5.1 by Mari023,
//! https://github.com/Mari023/AE2WirelessTerminalLibrary tag `19.5.1`, MIT;
//! see `ae2/addons/ae2wtlib/`. The item's rules are
//! `horizon_client::ae2x::ae2wtlib::magnet`.
use crate::game::{Game, ae2_hotkeys, ae2_wireless, lan::LanHost};
use horizon_client::{
    ae2x::{
        ae2wtlib::magnet::{self, Filter},
        defs::{self, WirelessTerminal},
        wireless,
    },
    items::ItemStack,
    lan::guest::Guest,
    text::{GREEN, RED, Text},
};
use horizon_protocol::packets::play::{Clientbound, SystemChat};

/// `MagnetHotkeyAction`'s id (`HotkeyActions.register`).
pub(in crate::game) const MAGNET_HOTKEY: &str = "ae2wtlib_magnet";
/// `TerminalSettingsPacket` (five bytes: pick block, restock, magnet,
/// pickup to ME, craft if missing).
pub(in crate::game) const TERMINAL_SETTINGS: &str = "ae2wtlib:terminal_settings";
/// `MagnetMenu`'s client actions (one byte: toggle the pickup mode, the
/// insert mode, copy up, copy down, switch) and its filter slots' presses
/// (5, the filter: 0 pickup, 1 insert, the slot).
pub(in crate::game) const MAGNET_MENU: &str = "ae2wtlib:magnet_menu";
/// `AE2wtlibTags.NO_RESTOCK`.
const NO_RESTOCK: &str = "ae2wtlib:no_restock";

/// `WTDefinitions.CRAFTING`: the magnet's terminal.
fn crafting() -> Option<&'static WirelessTerminal> {
    defs::get().wireless_terminal_named("crafting")
}

/// `CraftingTerminalHandler.getCraftingTerminal`: `WUTHandler.findTerminal`
/// for the crafting terminal (a universal terminal holding it first), its
/// slot and stack.
fn located(g: &Guest) -> Option<(usize, &ItemStack)> {
    let slot = ae2_hotkeys::locate(g, crafting()?)?;
    Some((slot, ae2_wireless::source_stack(g, slot)?))
}

/// The first stack `Inventory.tick` ticks that `ItemWCT.inventoryTick`
/// runs for (the crafting terminal, or a universal terminal holding it):
/// `MagnetHandler.handle` answers once a tick, for that one.
fn ticking(g: &Guest) -> Option<&ItemStack> {
    let terminal = crafting()?;
    g.inventory.iter().flatten().chain(g.offhand.as_ref()).find(|s| wireless::has_terminal(s, terminal))
}

impl Game<'_> {
    /// `MagnetHandler.handleMagnet`: unless the guest sneaks, with the
    /// magnet on, the item entities in its box grown by the range that the
    /// pickup filter takes are touched as if the guest stood on them.
    pub(in crate::game) fn lan_ae2wtlib_magnet_tick(&mut self, host: &mut LanHost, gi: usize) {
        let Some(g) = host.hub.guests.get(gi) else { return };
        if g.spectator() || g.sneaking || self.dropped.is_empty() {
            return;
        }
        if !ticking(g).is_some_and(|stack| magnet::mode(stack).magnet()) {
            return;
        }
        let Some((_, terminal)) = located(g).filter(|(_, stack)| magnet::mode(stack).magnet()) else { return };
        let filter = Filter::pickup(terminal);
        // An empty allow list would take anything (`IPartitionList`).
        if filter.allows_nothing() {
            return;
        }
        let r = magnet::range();
        let [x, y, z] = g.position;
        let (min, max) = ([x - 0.3 - r, y - r, z - 0.3 - r], [x + 0.3 + r, y + 1.8 + r, z + 0.3 + r]);
        let uuid = horizon_client::lan::playerdata::uuid_ints(g.uuid);
        let items: Vec<u32> = self
            .dropped
            .iter()
            .filter(|d| {
                let b = d.bounds();
                let (lo, hi) = (b.min_position(), b.max_position());
                lo.x < max[0] && hi.x > min[0] && lo.y < max[1] && hi.y > min[1] && lo.z < max[2] && hi.z > min[2]
            })
            // `playerTouch`: no pickup delay, no other target; an empty
            // stack has no key.
            .filter(|d| d.pickup_delay == 0 && d.target.is_none_or(|t| t == uuid) && d.stack.count > 0)
            .filter(|d| filter.matches(&d.stack.item))
            .map(|d| d.id)
            .collect();
        if !items.is_empty() {
            self.guest_touch_items(host, gi, items);
        }
    }

    /// `AE2wtlibEvents.canInsert`: restocking a hotbar item, or (not
    /// sneaking) a terminal picking up into the network whose insert filter
    /// takes it.
    fn ae2wtlib_can_insert(&self, g: &Guest, stack: &ItemStack) -> bool {
        let Some((_, terminal)) = located(g) else { return false };
        if magnet::settings(terminal).2 && self.ae2wtlib_restocking(g, stack) {
            return true;
        }
        if g.sneaking || !magnet::mode(terminal).pickup_to_me() {
            return false;
        }
        let filter = Filter::insert(terminal);
        !filter.allows_nothing() && filter.matches(&stack.item)
    }
    /// `AE2wtlibEvents.isRestocking`: a stackable item, not tagged
    /// `no_restock`, the same as one in the hotbar.
    fn ae2wtlib_restocking(&self, g: &Guest, stack: &ItemStack) -> bool {
        let items = self.res.items();
        if items.item_tags.get(NO_RESTOCK).is_some_and(|tag| tag.contains(&*stack.item)) || items.stack_max(stack) == 1 {
            return false;
        }
        g.inventory.iter().take(9).flatten().any(|held| held.same_item_same_components(stack))
    }

    /// `AE2wtlibEvents.insertStackInME` for item entity `index` touching
    /// guest `conn`: what the network linked to its terminal takes leaves the
    /// stack (no power drawn, as `StorageService.getInventory` is not
    /// powered), counted as picked up. True when it got that far.
    pub(in crate::game) fn lan_ae2wtlib_item_pickup(&mut self, host: &mut LanHost, conn: u32, index: usize) -> bool {
        let Some(stack) = self.dropped.get(index).map(|d| d.stack.clone()).filter(|s| s.count > 0) else { return false };
        let Some(g) = host.hub.guests.iter().find(|g| g.conn == conn) else { return false };
        if !self.ae2wtlib_can_insert(g, &stack) {
            return false;
        }
        // `inRange` and `getTargetGrid`: the access point the terminal reaches.
        let Some((_, terminal)) = located(g) else { return false };
        let Some(anchor) = self.ae2_wireless_item_link(host, conn, terminal) else { return false };
        let Some(address) = self.ae2_local_address(anchor) else { return false };
        let snapshot = self.ae2_snapshot_blocks();
        let mut plan = crate::game::ae2_buses::FormationPlan::default();
        let inserted = match self
            .ae2_grid_insert_uncharged_staged_encoded(address, &stack, u64::from(stack.count), &mut plan)
            .and_then(|moved| self.ae2_commit_formation(plan).map(|_| moved))
        {
            Ok(moved) => moved.min(u64::from(stack.count)) as u32,
            Err(error) => {
                log::warn!("AE2WTLib pickup into the network: {error}");
                self.ae2_restore_blocks(snapshot);
                0
            }
        };
        let thrower = self.dropped[index].thrower;
        self.as_guest(host, conn, |g| {
            if inserted > 0 {
                g.award_stat("picked_up", &stack.item, inserted as i32);
            }
            // `onItemPickup`, with the whole stack.
            if let Some(thrower) = thrower {
                g.trigger_thrown_picked_up_by_player(&stack, thrower);
            }
        });
        if let Some(item) = self.dropped.get_mut(index) {
            item.stack.count -= inserted;
        }
        true
    }

    /// `MagnetHotkeyAction.run`: the located terminal's mode, cycled when it
    /// has the card, said in the action bar. False when the guest has no
    /// crafting terminal.
    pub(in crate::game) fn lan_ae2wtlib_magnet_hotkey(&mut self, host: &mut LanHost, conn: u32) -> bool {
        let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn && !g.spectator()) else { return false };
        let Some((slot, terminal)) = located(&host.hub.guests[gi]) else { return false };
        let Some((next, key, on)) = magnet::mode(terminal).hotkey_next() else { return true };
        let mode = Text::translate(key, &[]).with_color(if on { GREEN } else { RED });
        let text = Text::translate("gui.ae2wtlib.magnetcard.hotkey", &[mode]);
        host.hub.send_to(conn, &Clientbound::SystemChat(SystemChat { content: horizon_client::lan::data::component_nbt(&text.component()), overlay: true }));
        if let Some(stack) = ae2_wireless::source_stack_mut(&mut host.hub.guests[gi], slot) {
            magnet::save_mode(stack, next);
        }
        host.hub.send_inventory(gi);
        true
    }

    /// `TerminalSettingsPacket.processPacketData`, on the item of the open
    /// wireless menu.
    pub(in crate::game) fn lan_ae2wtlib_terminal_settings(&mut self, host: &mut LanHost, conn: u32, data: &[u8]) {
        let [pick_block, restock, magnet, pickup_to_me, craft_if_missing] = data else { return };
        let Some(source) = self.lan_wireless_menu_source(host, conn) else { return };
        let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) else { return };
        let Some(stack) = ae2_wireless::source_stack_mut(&mut host.hub.guests[gi], source).filter(|s| wireless::host(s)) else { return };
        magnet::apply_settings(stack, *pick_block != 0, *restock != 0, *magnet != 0, *pickup_to_me != 0, *craft_if_missing != 0);
        host.hub.send_inventory(gi);
    }

    /// `MagnetMenu`'s actions on the magnet's terminal (`MagnetHost`, of
    /// `CraftingTerminalHandler`), with the open wireless crafting terminal's
    /// menu standing for `WCTMenu` (`openMagnetMenu`).
    pub(in crate::game) fn lan_ae2wtlib_magnet_menu(&mut self, host: &mut LanHost, conn: u32, data: &[u8]) {
        let Some(source) = self.lan_wireless_menu_source(host, conn) else { return };
        let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) else { return };
        let g = &host.hub.guests[gi];
        if !ae2_wireless::source_stack(g, source).is_some_and(|s| crafting().is_some_and(|t| wireless::has_terminal(s, t))) {
            return;
        }
        let Some((slot, _)) = located(g) else { return };
        let carried = g.carried.clone();
        let Some(stack) = ae2_wireless::source_stack_mut(&mut host.hub.guests[gi], slot) else { return };
        let config = |which: u8| if which == 0 { magnet::PICKUP_CONFIG } else { magnet::INSERT_CONFIG };
        match data {
            [0] => magnet::toggle_filter_mode(stack, magnet::PICKUP_MODE),
            [1] => magnet::toggle_filter_mode(stack, magnet::INSERT_MODE),
            [2] => magnet::copy_filter(stack, magnet::INSERT_CONFIG, magnet::PICKUP_CONFIG),
            [3] => magnet::copy_filter(stack, magnet::PICKUP_CONFIG, magnet::INSERT_CONFIG),
            [4] => magnet::switch_filters(stack),
            [5, which @ (0 | 1), index] => {
                if magnet::set_filter_slot(stack, config(*which), usize::from(*index), carried.as_ref()).is_err() {
                    return;
                }
            }
            _ => return,
        }
        host.hub.send_inventory(gi);
    }

    /// `WCTScreen`'s settings: on a wireless crafting terminal's menu (the
    /// terminal, or a universal terminal turned to it), the item's terminal
    /// settings and its magnet mode (`getMagnetMode`).
    pub(in crate::game) fn lan_ae2wtlib_wireless_view(&self, host: &LanHost, conn: u32, ctx: &ae2_wireless::WirelessContext) -> Option<serde_json::Value> {
        if ctx.kind != defs::TerminalKind::Crafting {
            return None;
        }
        let g = host.hub.guests.iter().find(|g| g.conn == conn)?;
        let stack = ae2_wireless::source_stack(g, ctx.source)?;
        let (pick_block, craft_if_missing, restock, mode) = magnet::settings(stack);
        // MagnetScreen's: the magnet's terminal's filters.
        let filters = located(g).map(|(_, terminal)| {
            serde_json::json!({
                "pickup": magnet::filter_items(terminal, magnet::PICKUP_CONFIG),
                "insert": magnet::filter_items(terminal, magnet::INSERT_CONFIG),
                "pickup_allow": Filter::pickup(terminal).allow,
                "insert_allow": Filter::insert(terminal).allow,
            })
        });
        Some(serde_json::json!({"pick_block": pick_block, "craft_if_missing": craft_if_missing, "restock": restock, "magnet": mode.id(), "stored": magnet::stored_mode(stack).id(), "filters": filters}))
    }
}

