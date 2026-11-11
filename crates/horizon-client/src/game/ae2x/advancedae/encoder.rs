//! The Advanced Pattern Encoder's menu on the server (`AdvPatternEncoderItem`,
//! `AdvPatternEncoderHost`, `AdvPatternEncoderMenu`,
//! `AdvPatternEncoderChangeDirectionPacket`, the `PATTERN_ENCODER_HOTKEY`
//! action). Derived from Advanced AE, https://github.com/pedroksl/AdvancedAE
//! commit `c322dad81e6f2ad94cb1fb861e73a7ba68e5907a`, LGPL-3.0; see
//! `ae2/addons/advancedae/`.
//!
//! The menu has two slots, the input (`RestrictedInputSlot(ENCODED_PATTERN)`)
//! and the output (`OutputSlot`), over the player's inventory, in a 9 x 3
//! window: entries 0 and 1. A pattern put in decodes to an Advanced one in the
//! output (`decodeInputPattern`); taking the output takes the input with it
//! (`onChangeInventory`). A direction button is a menu button
//! ([`BUTTONS`]). The two slots stay in the item: where the mod keeps them in
//! AE2AddonLib's NBT component, which the port does not read, the port keeps
//! them as AE2's `AppEngInternalInventory.writeToNBT` list under
//! `inOutInventory` in the item's `minecraft:custom_data`.
use super::{direction_map, encoder_output, with_direction};
use crate::game::{Game, lan::LanHost, shield::OFFHAND_SLOT};
use horizon_client::{
    ae2x::advancedae::ADV_PATTERN_ENCODER,
    items::ItemStack,
    menu::{Input, Kind, Menu},
    save,
};
use horizon_core::geometry::Direction;
use horizon_protocol::packets::{
    configuration::CustomPayload,
    play::{self, Clientbound as C},
};
use horizon_world::nbt::Tag;

/// The encoder's window, for this client to draw it (four bytes, big end first).
pub(in crate::game) const ENCODER_WINDOW: &str = "advanced_ae:encoder_window";
/// `AAEHotkeysRegistry.Keys.PATTERN_ENCODER_HOTKEY`: its key mapping is
/// `key.advanced_ae.pattern_encoder_action`, unbound at first.
pub(in crate::game) const HOTKEY: &str = "advanced_ae:pattern_encoder_action";
/// The window's entries: the input and the output slot.
pub(in crate::game) const INPUT: usize = 0;
pub(in crate::game) const OUTPUT: usize = 1;
/// A `generic_9x3` window's entries before the player's inventory.
const ENTRIES: usize = 27;
/// A direction button's menu button: `BUTTONS + 7 * row + column`, the row
/// of the input list (`getDirectionMap`'s order) and the column of
/// `DirectionInputButton.getDirection`.
pub(in crate::game) const BUTTONS: i32 = 100;
/// `DirectionInputButton.getDirection` by column: none (the face toward the
/// provider), then north, east, south, west, up, down.
pub(in crate::game) const COLUMNS: [Option<Direction>; 7] =
    [None, Some(Direction::North), Some(Direction::East), Some(Direction::South), Some(Direction::West), Some(Direction::Up), Some(Direction::Down)];
/// Where the item keeps its two slots.
const SAVED: &str = "inOutInventory";

/// One player's open encoder menu; the item stays in its slot.
pub(in crate::game) struct Session {
    window: i32,
    state: i32,
    /// The player's slot holding the encoder (`MenuLocators.forHand`, or
    /// the hotkey's `forInventorySlot`).
    source: usize,
}

/// `AdvPatternEncoderHost`'s constructor: the input and output the item keeps.
pub(in crate::game) fn slots(stack: &ItemStack) -> [Option<ItemStack>; 2] {
    let mut slots = [None, None];
    let Some(Tag::List(entries)) = stack.custom_data().and_then(|data| data.get(&SAVED.into())) else { return slots };
    for entry in entries {
        let slot = match entry.get("Slot") {
            Some(Tag::Int(n)) => *n,
            Some(Tag::Byte(n)) => i32::from(*n),
            _ => continue,
        };
        if let Some(place) = usize::try_from(slot).ok().and_then(|slot| slots.get_mut(slot)) {
            *place = save::item_from_nbt(entry).filter(|s| s.count > 0);
        }
    }
    slots
}

/// `AdvPatternEncoderHost.saveChangedInventory`: the two slots into the item,
/// nothing kept when both are empty.
pub(in crate::game) fn store(stack: &mut ItemStack, slots: &[Option<ItemStack>; 2]) {
    let entries: Vec<Tag> = slots
        .iter()
        .enumerate()
        .filter_map(|(slot, item)| {
            let mut tag = save::item_to_nbt(item.as_ref()?);
            if let Tag::Compound(map) = &mut tag {
                map.insert("Slot".into(), Tag::Int(slot as i32));
            }
            Some(tag)
        })
        .collect();
    let mut data = stack.custom_data().cloned().unwrap_or_default();
    if entries.is_empty() {
        data.remove(&SAVED.into());
    } else {
        data.insert(SAVED.into(), Tag::List(entries));
    }
    stack.data_mut().set_other("minecraft:custom_data", (!data.is_empty()).then_some(Tag::Compound(data)));
    stack.tidy();
}

/// What a click left in the two slots, as the menu's slots take it:
/// `before` and `after` the click, or none when no slot would have taken the
/// change (the input takes encoded patterns, the output nothing). Then
/// `onChangeInventory`: a changed input decodes into the output (none for a
/// pattern that is no processing pattern, where the mod would leave the old
/// output for the new input), a taken output takes the input.
pub(in crate::game) fn after_click(before: &[Option<ItemStack>; 2], after: [Option<ItemStack>; 2], is_pattern: &dyn Fn(&ItemStack) -> bool) -> Option<[Option<ItemStack>; 2]> {
    let [input, output] = after;
    if input.as_ref().is_some_and(|item| !is_pattern(item)) || (output.is_some() && output != before[OUTPUT]) {
        return None;
    }
    if input != before[INPUT] {
        let output = input.as_ref().and_then(encoder_output);
        return Some([input, output]);
    }
    if output.is_none() && before[OUTPUT].is_some() {
        return Some([None, None]);
    }
    Some([input, output])
}

/// `AdvPatternEncoderMenu.update`: the slots after the press on direction
/// button `id`, the row's input sent to the column's face; none for no change.
pub(in crate::game) fn after_button(slots: &[Option<ItemStack>; 2], id: i32) -> Option<[Option<ItemStack>; 2]> {
    let index = usize::try_from(id.checked_sub(BUTTONS)?).ok()?;
    let (row, face) = (index / COLUMNS.len(), COLUMNS[index % COLUMNS.len()]);
    // copyItemToOutputSlot when the output is empty.
    let output = slots[OUTPUT].clone().or_else(|| slots[INPUT].as_ref().and_then(encoder_output))?;
    let (key, _) = direction_map(&output)?.into_iter().nth(row)?;
    let next = with_direction(&output, &key, face)?;
    Some([slots[INPUT].clone(), Some(next)])
}

impl Game<'_> {
    /// `AdvPatternEncoderItem.use`: the menu of the encoder in hand `hand`
    /// (0 main, 1 off). False when the hand holds none.
    pub(in crate::game) fn lan_open_advancedae_encoder_hand(&mut self, host: &mut LanHost, conn: u32, hand: i32) -> bool {
        let Some(guest) = host.hub.guests.iter().find(|g| g.conn == conn && !g.spectator()) else { return false };
        let source = if hand == 1 { OFFHAND_SLOT } else { guest.selected };
        self.lan_open_advancedae_encoder(host, conn, source)
    }

    /// The hotkey (`InventoryHotkeyAction`): the first encoder of the
    /// player's inventory, `openFromInventory`.
    pub(in crate::game) fn lan_advancedae_hotkey(&mut self, host: &mut LanHost, conn: u32) -> bool {
        let Some(guest) = host.hub.guests.iter().find(|g| g.conn == conn && !g.spectator()) else { return false };
        let Some(source) = guest.inventory.iter().take(36).position(|s| s.as_ref().is_some_and(|s| s.item.as_ref() == ADV_PATTERN_ENCODER && s.count > 0)) else {
            return false;
        };
        self.lan_open_advancedae_encoder(host, conn, source)
    }

    /// `MenuOpener.open(ADV_PATTERN_ENCODER, ...)` on the encoder in the
    /// player's slot `source`; the menu decodes the input it holds
    /// (`AdvPatternEncoderMenu`'s constructor).
    fn lan_open_advancedae_encoder(&mut self, host: &mut LanHost, conn: u32, source: usize) -> bool {
        let held = self.as_guest(host, conn, |g| g.stack_in(source).is_some_and(|s| s.item.as_ref() == ADV_PATTERN_ENCODER && s.count == 1)).unwrap_or(false);
        if !held {
            return false;
        }
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
        self.lan_advancedae_encoder_closed(host, conn, None);
        let title = horizon_protocol::text::translatable("gui.advanced_ae.AdvPatternEncoder", &[]);
        let Some(window) = host.hub.open_window(conn, "minecraft:generic_9x3", title) else { return true };
        host.hub.send_to(conn, &C::CustomPayload(CustomPayload { channel: ENCODER_WINDOW.into(), data: window.to_be_bytes().to_vec() }));
        self.ae2x.advancedae.encoders.insert(conn, Session { window, state: 0, source });
        let _ = self.as_guest(host, conn, |g| {
            if let Some(stack) = g.stack_in_mut(source).as_mut() {
                let mut held = slots(stack);
                if held[INPUT].is_some() {
                    held[OUTPUT] = held[INPUT].as_ref().and_then(encoder_output);
                    store(stack, &held);
                }
            }
        });
        self.lan_send_advancedae_encoder(host, conn);
        true
    }

    pub(in crate::game) fn lan_advancedae_encoder_closed(&mut self, host: &mut LanHost, conn: u32, window: Option<i32>) -> bool {
        if !self.ae2x.advancedae.encoders.get(&conn).is_some_and(|s| window.is_none_or(|w| w == s.window)) {
            return false;
        }
        self.ae2x.advancedae.encoders.remove(&conn);
        if let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) {
            self.guest_told_window_closed(host, gi);
        }
        true
    }

    /// `ItemMenuHost.isValid`: the encoder still in its slot, the window still
    /// the player's, a player who is no spectator; else the menu closes.
    fn lan_advancedae_encoder_valid(&mut self, host: &mut LanHost, conn: u32) -> Option<ItemStack> {
        let (window, source) = self.ae2x.advancedae.encoders.get(&conn).map(|s| (s.window, s.source))?;
        if !host.hub.guests.iter().any(|g| g.conn == conn && !g.spectator() && g.next_window == window) {
            self.ae2x.advancedae.encoders.remove(&conn);
            return None;
        }
        let stack = self.as_guest(host, conn, |g| g.stack_in(source).filter(|s| s.item.as_ref() == ADV_PATTERN_ENCODER && s.count == 1).cloned()).flatten();
        if stack.is_none() {
            self.lan_advancedae_encoder_closed(host, conn, None);
            host.hub.send_to(conn, &C::ContainerClose(play::ContainerClose { container: window }));
        }
        stack
    }

    /// `broadcastChanges`: the two slots, then the player's inventory.
    fn lan_send_advancedae_encoder(&mut self, host: &mut LanHost, conn: u32) {
        let Some(stack) = self.lan_advancedae_encoder_valid(host, conn) else { return };
        let Some(s) = self.ae2x.advancedae.encoders.get_mut(&conn) else { return };
        let Some(g) = host.hub.guests.iter().find(|g| g.conn == conn) else { return };
        s.state = (s.state + 1) & 0x7fff;
        let mut items: Vec<Option<ItemStack>> = vec![None; ENTRIES];
        let [input, output] = slots(&stack);
        items[INPUT] = input;
        items[OUTPUT] = output;
        items.extend((9..36).chain(0..9).map(|i| g.inventory[i].clone()));
        let carried = host.hub.stack(&g.carried.clone());
        let packet = C::ContainerSetContent(play::ContainerSetContent { container: s.window, state: s.state, items: items.iter().map(|i| host.hub.stack(i)).collect(), carried });
        host.hub.send_to(conn, &packet);
        host.hub.inventory_told_by(conn, &items);
    }

    /// Each tick: a menu whose encoder left its slot closes.
    pub(in crate::game) fn lan_refresh_advancedae_encoders(&mut self, host: &mut LanHost) {
        let conns: Vec<u32> = self.ae2x.advancedae.encoders.keys().copied().collect();
        for conn in conns {
            if host.hub.guests.iter().any(|g| g.conn == conn && !host.hub.here(g)) {
                continue;
            }
            let _ = self.lan_advancedae_encoder_valid(host, conn);
        }
    }

    /// A click in the encoder's window: its two slots as their
    /// `RestrictedInputSlot` and `OutputSlot`, the player's inventory around
    /// them; the encoder's own slot takes no click (`lockPlayerInventorySlot`).
    pub(in crate::game) fn lan_advancedae_encoder_click(&mut self, host: &mut LanHost, conn: u32, p: &play::ContainerClick) -> bool {
        let Some((window, source)) = self.ae2x.advancedae.encoders.get(&conn).map(|s| (s.window, s.source)) else { return false };
        if p.container != window {
            return false;
        }
        let Some(stack) = self.lan_advancedae_encoder_valid(host, conn) else { return true };
        let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) else { return true };
        let input = crate::game::lan_crafting::input(p.input);
        let button = i32::from(p.button);
        let source_entry = if source < 9 { (ENTRIES + 27 + source) as i16 } else if source < 36 { (ENTRIES + source - 9) as i16 } else { -1 };
        let locked = p.slot == source_entry || (input == Input::Swap && (button as usize == source || (button == 40 && source == OFFHAND_SLOT)));
        let before = slots(&stack);
        let is_pattern = |item: &ItemStack| crate::game::ae2_pattern_access::encoded_pattern_item(item);
        if locked || usize::try_from(p.slot).is_ok_and(|slot| (2..ENTRIES).contains(&slot)) {
            self.lan_send_advancedae_encoder(host, conn);
            return true;
        }
        let mut next = None;
        if input == Input::QuickMove && usize::try_from(p.slot).is_ok_and(|slot| slot >= ENTRIES) {
            // quickMoveStack from the player's side: a pattern into the empty input.
            let index = match p.slot as usize - ENTRIES {
                s @ 0..27 => Some(9 + s),
                s @ 27..36 => Some(s - 27),
                _ => None,
            };
            if let Some(index) = index
                && before[INPUT].is_none()
                && let Some(moving) = host.hub.guests[gi].inventory[index].clone().filter(|item| is_pattern(item))
            {
                let mut rest = moving.clone();
                rest.count -= 1;
                host.hub.guests[gi].inventory[index] = (rest.count > 0).then_some(rest);
                next = after_click(&before, [Some(ItemStack { count: 1, ..moving }), before[OUTPUT].clone()], &is_pattern);
            }
        } else {
            // Any other click as a chest's; a change no slot takes puts everything back.
            let mut menu = Menu::new(Kind::Chest(3));
            {
                let env = self.menu_env();
                menu.set_slot(&env, INPUT, before[INPUT].clone());
                menu.set_slot(&env, OUTPUT, before[OUTPUT].clone());
            }
            let before_inventory = host.hub.guests[gi].inventory.clone();
            let before_carried = host.hub.guests[gi].carried.clone();
            self.guest_menu_op(host, gi, &mut menu, |m, env| {
                let _ = m.click(env, i32::from(p.slot), button, input);
            });
            let stray = (2..ENTRIES).any(|entry| menu.slot(entry).is_some());
            next = (!stray).then(|| after_click(&before, [menu.slot(INPUT).cloned(), menu.slot(OUTPUT).cloned()], &is_pattern)).flatten();
            if next.is_none() {
                host.hub.guests[gi].inventory = before_inventory;
                host.hub.guests[gi].carried = before_carried;
            }
        }
        if let Some(next) = next.filter(|next| *next != before) {
            let _ = self.as_guest(host, conn, |g| g.stack_in_mut(source).as_mut().map(|s| store(s, &next)));
        }
        self.lan_send_advancedae_encoder(host, conn);
        true
    }

    /// A press on a direction button (`AdvPatternEncoderChangeDirectionPacket`).
    pub(in crate::game) fn lan_advancedae_encoder_button(&mut self, host: &mut LanHost, conn: u32, window: i32, id: i32) -> bool {
        let Some(source) = self.ae2x.advancedae.encoders.get(&conn).filter(|s| s.window == window).map(|s| s.source) else { return false };
        let Some(stack) = self.lan_advancedae_encoder_valid(host, conn) else { return true };
        if let Some(next) = after_button(&slots(&stack), id) {
            let _ = self.as_guest(host, conn, |g| g.stack_in_mut(source).as_mut().map(|s| store(s, &next)));
        }
        self.lan_send_advancedae_encoder(host, conn);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ae2_source_patterns as source;
    use horizon_client::ae2_generic_stack::GenericStack;

    fn item(id: &str, amount: i64) -> Option<GenericStack> {
        Some(GenericStack::item(horizon_client::ae2_cell_codec::key(&ItemStack::new(id, 1)).unwrap(), amount))
    }
    fn ae2_pattern(inputs: &[Option<GenericStack>]) -> ItemStack {
        let mut pattern = ItemStack::new("ae2:processing_pattern", 1);
        pattern.data_mut().set_other("ae2:encoded_processing_pattern", Some(source::encode_processing(inputs, &[item("minecraft:iron_ingot", 1)]).unwrap()));
        pattern
    }
    fn is_pattern(item: &ItemStack) -> bool {
        item.item.as_ref().ends_with("processing_pattern") || item.item.as_ref() == "ae2:crafting_pattern"
    }

    /// `saveChangedInventory` and the host's constructor: the two slots in the
    /// item's custom data, beside what else it holds, gone when both are empty.
    #[test]
    fn the_encoder_keeps_its_slots_in_the_item() {
        horizon_client::ae2x::install();
        let mut encoder = ItemStack::new(ADV_PATTERN_ENCODER, 1);
        assert_eq!(slots(&encoder), [None, None]);
        let pattern = ae2_pattern(&[item("minecraft:raw_iron", 1)]);
        let advanced = encoder_output(&pattern).unwrap();
        store(&mut encoder, &[Some(pattern.clone()), Some(advanced.clone())]);
        assert_eq!(slots(&encoder), [Some(pattern.clone()), Some(advanced)]);
        let Some(Tag::List(list)) = encoder.custom_data().unwrap().get(&SAVED.into()) else { panic!("no inventory") };
        assert_eq!(list.iter().map(|e| e.get("Slot").cloned()).collect::<Vec<_>>(), [Some(Tag::Int(0)), Some(Tag::Int(1))]);
        store(&mut encoder, &[None, None]);
        assert_eq!(encoder, ItemStack::new(ADV_PATTERN_ENCODER, 1), "an empty encoder keeps nothing");
    }

    /// `onChangeInventory`: a pattern put in decodes to an Advanced one;
    /// taking the output takes the input; the output takes nothing put in,
    /// the input only patterns.
    #[test]
    fn the_menus_slots_follow_onchangeinventory() {
        horizon_client::ae2x::install();
        let pattern = ae2_pattern(&[item("minecraft:raw_iron", 1), item("minecraft:coal", 1)]);
        let advanced = encoder_output(&pattern).unwrap();
        let empty = [None, None];
        assert_eq!(after_click(&empty, [Some(pattern.clone()), None], &is_pattern), Some([Some(pattern.clone()), Some(advanced.clone())]));
        let full = [Some(pattern.clone()), Some(advanced.clone())];
        assert_eq!(after_click(&full, [Some(pattern.clone()), None], &is_pattern), Some([None, None]), "the output taken");
        assert_eq!(after_click(&full, [None, Some(advanced.clone())], &is_pattern), Some([None, None]), "the input taken");
        assert_eq!(after_click(&empty, [None, Some(advanced.clone())], &is_pattern), None, "nothing goes into the output");
        assert_eq!(after_click(&empty, [Some(ItemStack::new("minecraft:stone", 1)), None], &is_pattern), None);
        // A crafting pattern decodes to nothing.
        assert_eq!(after_click(&empty, [Some(ItemStack::new("ae2:crafting_pattern", 1)), None], &is_pattern), Some([Some(ItemStack::new("ae2:crafting_pattern", 1)), None]));
    }

    /// `update` from a direction button: the row's key to the column's face,
    /// the input left as it is; a row past the list does nothing.
    #[test]
    fn a_direction_button_sets_its_rows_face() {
        horizon_client::ae2x::install();
        let pattern = ae2_pattern(&[item("minecraft:raw_iron", 1), item("minecraft:coal", 1)]);
        let full = [Some(pattern.clone()), encoder_output(&pattern)];
        // Row 1 (coal), column 5 (up).
        let next = after_button(&full, BUTTONS + 7 + 5).unwrap();
        assert_eq!(next[INPUT], Some(pattern.clone()));
        assert_eq!(super::super::directions(next[OUTPUT].as_ref().unwrap()).unwrap(), vec![None, Some(Direction::Up)]);
        // Column 0 clears it; a missing output is decoded from the input first.
        let cleared = after_button(&next, BUTTONS + 7).unwrap();
        assert_eq!(super::super::directions(cleared[OUTPUT].as_ref().unwrap()).unwrap(), vec![None, None]);
        let decoded = after_button(&[Some(pattern), None], BUTTONS + 2).unwrap();
        assert_eq!(super::super::directions(decoded[OUTPUT].as_ref().unwrap()).unwrap(), vec![Some(Direction::East), None]);
        assert_eq!(after_button(&full, BUTTONS + 14), None, "no third row");
        assert_eq!(after_button(&full, 5), None);
    }
}
