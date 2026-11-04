//! MEGA's Portable Cell Workbench on the server (`PortableCellWorkbenchItem`,
//! `PortableCellWorkbenchMenuHost`, `PortableCellWorkbenchInventory`,
//! `PortableCellWorkbenchMenu`, `ItemMenuHostMixin`): AE2's Cell Workbench
//! carried as an item. The item keeps the workbench's cell as its
//! `minecraft:container` and its 63 filters as `ae2:exported_config_inv`; the
//! menu is `CellWorkbenchMenu`'s, on the cell, its cards (the mixin's
//! `getUpgrades` is the held cell's own upgrade inventory) and its filters, and
//! the client draws AE2's Cell Workbench screen over it
//! (`screens/megacells/portable_cell_workbench.json` includes
//! `cell_workbench.json`). Derived from MEGA Cells 4.11.0,
//! https://github.com/62832/MEGACells tag 4.11.0, LGPL-3.0; see
//! `ae2/addons/megacells/`.
use crate::game::{Game, ae2_storage_devices as devices, lan::LanHost, lan_crafting, shield::OFFHAND_SLOT};
use horizon_client::{
    ae2_world::Machine,
    ae2x::megacells::PORTABLE_CELL_WORKBENCH,
    items::ItemStack,
    menu::Input,
};
use horizon_protocol::packets::{
    configuration::CustomPayload,
    play::{self, Clientbound as C},
};
use horizon_world::nbt::Tag;
use std::collections::VecDeque;

/// `CellWorkbenchScreen` presses filter `index` as window slot 100 + index.
const CONFIG_PLACE: i16 = crate::game::ae2_workbench_screen::CONFIG_PLACE;
const CONFIG_PLACES: usize = crate::game::ae2_workbench_screen::CONFIG_PLACES;

/// One player's open Portable Cell Workbench menu; the item stays in its slot.
pub(in crate::game) struct Session {
    window: i32,
    state: i32,
    /// The player's slot holding the workbench (`MenuLocators.forHand`).
    source: usize,
    page: usize,
    /// `PortableCellWorkbenchMenu.copyMode`: the host's `IConfigManager` is
    /// built fresh with `CLEAR_ON_REMOVE` and never reads the item's
    /// `ae2:exported_settings`, so each menu starts clearing on removal.
    keep: bool,
    snapshot: ItemStack,
    /// What each sent state showed: its page and copy mode, which a click on a
    /// filter must still see (`CellWorkbenchMenu`'s fake slots).
    revisions: VecDeque<(i32, usize, bool)>,
}

/// What the game keeps for MEGA Cells while it runs.
#[derive(Default)]
pub(in crate::game) struct State {
    pub(in crate::game) workbenches: std::collections::HashMap<u32, Session>,
    /// `CompressionService`'s chains on this server, and what each client was sent.
    pub(in crate::game) compression: super::compression::Compression,
}

/// `PortableCellWorkbenchInventory.saveChanges`: the cell as the item's
/// `DataComponents.CONTAINER`, the filters as `EXPORTED_CONFIG_INV`
/// (`config.toList()`, 63 nullable stacks) and the copy mode as
/// `EXPORTED_SETTINGS` (`exportSettings`).
fn store(machine: &Machine, stack: &mut ItemStack) -> Result<(), String> {
    let (cell, filters, keep) = devices::workbench_state(machine)?;
    let entries = horizon_client::ae2_cell_codec::encode_view_filter_slots(&filters)?;
    let source = horizon_client::ae2_cell_codec::encode_source_filters(&entries)?;
    let data = stack.data_mut();
    data.container = cell.map(|cell| vec![(0, cell)]).unwrap_or_default();
    data.set_other("ae2:exported_config_inv", Some(Tag::List(source)));
    let mode = if keep { "KEEP_ON_REMOVE" } else { "CLEAR_ON_REMOVE" };
    data.set_other("ae2:exported_settings", Some(Tag::compound([("copy_mode", Tag::String(mode.into()))])));
    Ok(())
}

/// `new PortableCellWorkbenchInventory(stack)`: the cell of the item's
/// container (`fromItemContainerContents`) and its filters
/// (`config.readFromList`, keys the game does not know kept as AE2's missing
/// content), as a Cell Workbench of copy mode `keep`.
fn read(stack: &ItemStack, keep: bool, known: &dyn Fn(&str, bool) -> bool) -> Result<Machine, String> {
    if stack.item.as_ref() != PORTABLE_CELL_WORKBENCH || stack.count != 1 {
        return Err("not a portable cell workbench".into());
    }
    let data = stack.data();
    let cell = data.and_then(|d| d.container.iter().find(|(slot, _)| *slot == 0)).map(|(_, cell)| cell.clone());
    let mut filters = vec![None; CONFIG_PLACES];
    match data.and_then(|d| d.other("ae2:exported_config_inv")) {
        None => {}
        Some(Tag::List(entries)) => {
            for (slot, entry) in filters.iter_mut().zip(entries) {
                let Some(value) = horizon_client::ae2_generic_stack::decode_tolerant_checked(entry, known)? else { continue };
                *slot = Some(if value.fluid {
                    Tag::compound([("fluid", Tag::String(value.key.item_id.as_str().into())), ("components", Tag::ByteArray(value.key.components))])
                } else {
                    Tag::compound([("item", horizon_client::save::item_to_nbt(&horizon_client::ae2_cell_codec::prototype(&value.key)?))])
                });
            }
        }
        Some(_) => return Err("invalid portable workbench config".into()),
    }
    devices::workbench_machine(cell, &filters, keep)
}

impl Game<'_> {
    /// [`read`] with the keys this game knows.
    pub(in crate::game) fn megacells_item_workbench(&self, stack: &ItemStack, keep: bool) -> Result<Machine, String> {
        read(stack, keep, &|id, fluid| self.ae2_known_cell_key(id, fluid))
    }

    /// `PortableCellWorkbenchItem.use`: on the server, the menu of the
    /// workbench in hand `hand` (0 main, 1 off), whether the player sneaks or not.
    pub(in crate::game) fn lan_open_portable_workbench(&mut self, host: &mut LanHost, conn: u32, hand: i32) -> bool {
        let Some(guest) = host.hub.guests.iter().find(|g| g.conn == conn && !g.spectator()) else { return false };
        let source = if hand == 1 { OFFHAND_SLOT } else { guest.selected };
        let Some(snapshot) = self
            .as_guest(host, conn, |g| g.stack_in(source).filter(|s| s.item.as_ref() == PORTABLE_CELL_WORKBENCH && s.count == 1).cloned())
            .flatten()
        else {
            return false;
        };
        self.lan_ae2_portable_closed(host, conn, None);
        self.lan_ae2_bus_closed(host, conn, None);
        self.lan_ae2_network_tool_closed(host, conn, None);
        self.lan_ae2_anchor_closed(host, conn, None);
        self.lan_craft_confirm_closed(host, conn, None);
        self.lan_pattern_access_closed(host, conn, None);
        self.lan_ae2_terminal_closed(host, conn, None);
        self.lan_ae2_machine_closed(host, conn, None);
        self.lan_station_closed(host, conn, None);
        self.lan_crafting_closed(host, conn, None);
        self.guest_container_closed(host, conn);
        self.lan_portable_workbench_closed(host, conn, None);
        // The window has no block: the screen goes by the item.
        host.hub.send_to(conn, &C::CustomPayload(CustomPayload { channel: "ae2:menu_block".into(), data: PORTABLE_CELL_WORKBENCH.as_bytes().to_vec() }));
        let title = self.pack.names.display_item(PORTABLE_CELL_WORKBENCH);
        let Some(window) = host.hub.open_window(conn, "minecraft:generic_9x3", horizon_protocol::text::literal(&title)) else {
            return true;
        };
        Self::lan_mark_ae2_base_screen(host, conn, window);
        let session = Session { window, state: 0, source, page: 0, keep: false, snapshot, revisions: VecDeque::new() };
        self.ae2x.megacells.workbenches.insert(conn, session);
        self.lan_send_portable_workbench(host, conn);
        true
    }

    pub(in crate::game) fn lan_portable_workbench_closed(&mut self, host: &mut LanHost, conn: u32, window: Option<i32>) -> bool {
        let open = self.ae2x.megacells.workbenches.get(&conn).is_some_and(|s| window.is_none_or(|w| w == s.window));
        if !open {
            return false;
        }
        self.ae2x.megacells.workbenches.remove(&conn);
        if let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) {
            self.guest_told_window_closed(host, gi);
        }
        true
    }

    /// Whether the player's window is still this menu's: one who opened
    /// another (or left) has no menu here any more, and loses nothing to it.
    fn lan_portable_workbench_current(&mut self, host: &LanHost, conn: u32) -> bool {
        let Some(window) = self.ae2x.megacells.workbenches.get(&conn).map(|s| s.window) else { return false };
        if host.hub.guests.iter().any(|g| g.conn == conn && g.next_window == window) {
            return true;
        }
        self.ae2x.megacells.workbenches.remove(&conn);
        false
    }

    /// `ItemMenuHost.isValid`: the workbench still in its slot, whatever it
    /// holds, and a player who is no spectator.
    fn lan_portable_workbench_valid(&mut self, host: &mut LanHost, conn: u32) -> Option<ItemStack> {
        let source = self.ae2x.megacells.workbenches.get(&conn).map(|s| s.source)?;
        host.hub.guests.iter().find(|g| g.conn == conn && !g.spectator())?;
        self.as_guest(host, conn, |g| g.stack_in(source).filter(|s| s.item.as_ref() == PORTABLE_CELL_WORKBENCH && s.count == 1).cloned())
            .flatten()
    }

    /// `AbstractContainerMenu.stillValid` failing: the menu closes, its cursor
    /// goes back to the inventory.
    fn lan_close_portable_workbench(&mut self, host: &mut LanHost, conn: u32) {
        let Some(window) = self.ae2x.megacells.workbenches.get(&conn).map(|s| s.window) else { return };
        self.lan_portable_workbench_closed(host, conn, None);
        host.hub.send_to(conn, &C::ContainerClose(play::ContainerClose { container: window }));
    }

    /// `broadcastChanges`: the menu's slots (the cell, its cards, a page of
    /// filters, the toolbar's entries), what the screen shows of them
    /// (`ae2:workbench_view`) and the player's inventory.
    fn lan_send_portable_workbench(&mut self, host: &mut LanHost, conn: u32) {
        if !self.lan_portable_workbench_current(host, conn) {
            return;
        }
        let Some(stack) = self.lan_portable_workbench_valid(host, conn) else {
            self.lan_close_portable_workbench(host, conn);
            return;
        };
        let Some(s) = self.ae2x.megacells.workbenches.get(&conn) else { return };
        let (window, page, keep) = (s.window, s.page, s.keep);
        let Ok(machine) = self.megacells_item_workbench(&stack, keep) else {
            self.lan_close_portable_workbench(host, conn);
            return;
        };
        let Some(mut items) = self.ae2_workbench_menu_slots_on(&machine, page) else { return };
        let view = self.ae2_workbench_view_on(&machine, window);
        let Some(g) = host.hub.guests.iter().find(|g| g.conn == conn) else { return };
        items.extend((9..36).chain(0..9).map(|i| g.inventory[i].clone()));
        let carried = g.carried.clone();
        let fake: Vec<u8> = (0..CONFIG_PLACES.saturating_sub(page * 18).min(18)).map(|i| (5 + i) as u8).collect();
        Self::lan_ae2_fake_slots(host, conn, window, &fake);
        let Some(s) = self.ae2x.megacells.workbenches.get_mut(&conn) else { return };
        s.snapshot = stack;
        s.state = (s.state + 1) & 0x7fff;
        let state = s.state;
        s.revisions.push_back((state, page, keep));
        while s.revisions.len() > 64 {
            s.revisions.pop_front();
        }
        if let Some(data) = view.as_ref().and_then(|view| serde_json::to_vec(view).ok()) {
            host.hub.send_to(conn, &C::CustomPayload(CustomPayload { channel: "ae2:workbench_view".into(), data }));
        }
        let packet = C::ContainerSetContent(play::ContainerSetContent {
            container: window,
            state,
            items: items.iter().map(|s| host.hub.stack(s)).collect(),
            carried: host.hub.stack(&carried),
        });
        host.hub.send_to(conn, &packet);
        host.hub.inventory_told_by(conn, &items);
    }

    /// Each tick: a menu whose workbench left its slot closes; one whose item
    /// changed is sent again.
    pub(in crate::game) fn lan_refresh_portable_workbenches(&mut self, host: &mut LanHost) {
        let conns: Vec<u32> = self.ae2x.megacells.workbenches.keys().copied().collect();
        for conn in conns {
            if host.hub.guests.iter().any(|g| g.conn == conn && !host.hub.here(g)) || !self.lan_portable_workbench_current(host, conn) {
                continue;
            }
            match self.lan_portable_workbench_valid(host, conn) {
                None => self.lan_close_portable_workbench(host, conn),
                Some(stack) if self.ae2x.megacells.workbenches.get(&conn).is_some_and(|s| s.snapshot != stack) => {
                    self.lan_send_portable_workbench(host, conn);
                }
                Some(_) => {}
            }
        }
    }

    /// A click in the menu's window (`AEBaseMenu.clicked` on
    /// `PortableCellWorkbenchMenu`'s slots, `CellWorkbenchScreen`'s filter
    /// presses by index): true when it was this menu's.
    pub(in crate::game) fn lan_portable_workbench_click(&mut self, host: &mut LanHost, conn: u32, p: &play::ContainerClick) -> bool {
        let Some((source, page, keep)) =
            self.ae2x.megacells.workbenches.get(&conn).filter(|s| s.window == p.container).map(|s| (s.source, s.page, s.keep))
        else {
            return false;
        };
        if !self.lan_portable_workbench_current(host, conn) {
            return true;
        }
        if self.lan_portable_workbench_valid(host, conn).is_none() {
            self.lan_close_portable_workbench(host, conn);
            return true;
        }
        // PortableCellWorkbenchMenu.mega$nextCompressionLimit: the held cell's cutoff.
        if p.slot == super::compression::CUTOFF_PLACE {
            let backwards = p.button == 1;
            let _ = self.as_guest(host, conn, |g| g.portable_workbench_cutoff(source, keep, backwards));
            self.lan_send_portable_workbench(host, conn);
            return true;
        }
        let input = lan_crafting::input(p.input);
        let mut button = i32::from(p.button);
        // A filter pressed by its index is on its own page, whatever page the menu shows.
        let by_index = (CONFIG_PLACE..CONFIG_PLACE + CONFIG_PLACES as i16).contains(&p.slot);
        let mut page = page;
        let slot = if by_index {
            let index = (p.slot - CONFIG_PLACE) as usize;
            page = index / 18;
            Some(305 + index % 18)
        } else {
            match p.slot {
                0..=26 => Some(300 + p.slot as usize),
                27..=53 => Some(9 + p.slot as usize - 27),
                54..=62 => Some(p.slot as usize - 54),
                _ => None,
            }
        };
        let filter = slot.is_some_and(|slot| (305..323).contains(&slot));
        let mut input = input;
        if filter && !by_index {
            // `FakeSlot`: an empty cursor's middle click and a hotbar key do nothing;
            // a filter pressed on another page or copy mode than the one shown is stale.
            let seen = self.ae2x.megacells.workbenches.get(&conn).is_some_and(|s| {
                s.revisions.iter().rev().find(|(state, ..)| *state == p.state).is_some_and(|(_, shown, kept)| *shown == page && *kept == keep)
            });
            let empty = host.hub.guests.iter().find(|g| g.conn == conn).is_none_or(|g| g.carried.is_none());
            if !seen || matches!(input, Input::Swap | Input::Throw) || (matches!(button, 2 | 3) && empty) {
                self.lan_send_portable_workbench(host, conn);
                return true;
            }
            if input == Input::QuickMove {
                input = Input::Pickup;
            }
            if matches!(button, 2 | 3) {
                button = 0;
            }
        }
        // AEBaseMenu.lockPlayerInventorySlot: the workbench's own slot takes no click.
        let locked = slot == Some(source) || (input == Input::Swap && (button as usize == source || (button == 40 && source == OFFHAND_SLOT)));
        if locked {
            self.lan_send_portable_workbench(host, conn);
            return true;
        }
        let next = self.as_guest(host, conn, |g| g.portable_workbench_click(source, keep, &mut page, slot, button, input));
        if let Some(s) = self.ae2x.megacells.workbenches.get_mut(&conn) {
            if !by_index {
                s.page = page;
            }
            if let Some(Ok(keep)) = next {
                s.keep = keep;
            }
        }
        self.lan_send_portable_workbench(host, conn);
        true
    }

    /// `mega$nextCompressionLimit` on the workbench in the player's slot
    /// `source`: its bulk cell switches its cutoff, and the item saves it.
    fn portable_workbench_cutoff(&mut self, source: usize, keep: bool, backwards: bool) -> Result<(), String> {
        let stack = self.stack_in(source).cloned().ok_or("portable cell workbench moved")?;
        let mut machine = self.megacells_item_workbench(&stack, keep)?;
        if !self.megacells_next_compression_limit(&mut machine, backwards) {
            return Ok(());
        }
        let mut next = stack;
        store(&machine, &mut next)?;
        *self.stack_in_mut(source) = Some(next);
        Ok(())
    }

    /// The click on the player's side, as the player: the cell slot
    /// (`RestrictedInputSlot(WORKBENCH_CELL)` on the item's inventory, one
    /// `ICellWorkbenchItem` at most, then `onChangeInventory`), the menu's
    /// cards, filters and actions (`CellWorkbenchMenu`'s), or the player's
    /// own inventory. Returns the copy mode after it.
    fn portable_workbench_click(&mut self, source: usize, keep: bool, page: &mut usize, slot: Option<usize>, button: i32, input: Input) -> Result<bool, String> {
        let Some(slot) = slot else { return Ok(keep) };
        let stack = self.stack_in(source).cloned().ok_or("portable cell workbench moved")?;
        let before = self.megacells_item_workbench(&stack, keep)?;
        let mut after = before.clone();
        if (301..327).contains(&slot) {
            let Some((next, cursor)) = self.ae2_workbench_menu_click_on(&before, page, slot, button, input) else { return Ok(keep) };
            after = next;
            self.carried = cursor;
        } else {
            let (inventory, carried) = (self.inventory.clone(), self.carried.clone());
            let mut cell = after.slots.first().cloned().flatten();
            if slot == 300 {
                match input {
                    Input::QuickMove => {
                        if let Some(item) = cell.take() {
                            cell = self.give(item);
                        }
                    }
                    Input::Pickup if (0..=1).contains(&button) => match (self.carried.take(), cell.take()) {
                        (None, item) => self.carried = item,
                        (Some(mut held), None) if devices::is_valid_workbench_cell(&ItemStack { count: 1, ..held.clone() }) => {
                            cell = Some(ItemStack { count: 1, ..held.clone() });
                            held.count -= 1;
                            self.carried = (held.count > 0).then_some(held);
                        }
                        (Some(held), Some(item)) if held.count == 1 && devices::is_valid_workbench_cell(&held) => {
                            cell = Some(held);
                            self.carried = Some(item);
                        }
                        (held, item) => {
                            self.carried = held;
                            cell = item;
                        }
                    },
                    _ => {}
                }
            } else if slot < 36 && input == Input::QuickMove {
                // quickMoveStack: a cell goes into the empty cell slot.
                if cell.is_none() && self.inventory[slot].as_ref().is_some_and(devices::is_valid_workbench_cell) {
                    cell = self.inventory[slot].take();
                }
            } else if slot < 36 {
                self.ae2_machine_player_menu_click(slot, button, input);
                return Ok(keep);
            } else {
                return Ok(keep);
            }
            if cell == after.slots.first().cloned().flatten() {
                return Ok(keep);
            }
            if let Some(first) = after.slots.first_mut() {
                *first = cell;
            }
            // onChangeInventory: the filters follow the cell put in or taken out.
            if self.ae2_workbench_cell_changed(&before, &mut after).is_err() {
                self.inventory = inventory;
                self.carried = carried;
                return Ok(keep);
            }
        }
        let keep = devices::workbench_state(&after)?.2;
        let mut next = stack;
        store(&after, &mut next)?;
        *self.stack_in_mut(source) = Some(next);
        Ok(keep)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item_filter(id: &str) -> Option<Tag> {
        Some(Tag::compound([("item", horizon_client::save::item_to_nbt(&ItemStack::new(id, 1)))]))
    }

    /// `PortableCellWorkbenchInventory.saveChanges`, then its constructor on
    /// the saved item: the cell in the item's container, the filters as a
    /// 63-entry `exported_config_inv` read back slot for slot, the copy mode
    /// exported but not read back (a new menu clears on removal).
    #[test]
    fn the_item_keeps_its_cell_and_filters() {
        let mut filters = vec![None; CONFIG_PLACES];
        filters[0] = item_filter("minecraft:diamond");
        filters[62] = item_filter("minecraft:cobblestone");
        let cell = ItemStack::new("megacells:portable_item_cell_1m", 1);
        let machine = devices::workbench_machine(Some(cell.clone()), &filters, true).unwrap();
        let mut stack = ItemStack::new(PORTABLE_CELL_WORKBENCH, 1);
        store(&machine, &mut stack).unwrap();
        let data = stack.data().unwrap();
        assert_eq!(data.container, vec![(0, cell.clone())]);
        let Some(Tag::List(config)) = data.other("ae2:exported_config_inv") else { panic!("no exported config") };
        assert_eq!(config.len(), 63);
        assert_eq!(data.other("ae2:exported_settings"), Some(&Tag::compound([("copy_mode", Tag::String("KEEP_ON_REMOVE".into()))])));
        let (back, read_filters, keep) = devices::workbench_state(&read(&stack, false, &|_, _| true).unwrap()).unwrap();
        assert_eq!((back, keep), (Some(cell), false));
        assert_eq!(read_filters, filters);
        // An emptied workbench keeps no container entry; no other item is one.
        let empty = devices::workbench_machine(None, &vec![None; CONFIG_PLACES], false).unwrap();
        store(&empty, &mut stack).unwrap();
        assert!(stack.data().unwrap().container.is_empty());
        assert_eq!(devices::workbench_state(&read(&stack, false, &|_, _| true).unwrap()).unwrap().0, None);
        assert!(read(&ItemStack::new("ae2:cell_workbench", 1), false, &|_, _| true).is_err());
    }
}
