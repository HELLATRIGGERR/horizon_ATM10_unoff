//! AE2WTLib 19.5.1 in the game (feature `ae2-ae2wtlib`): the wireless
//! pattern encoding terminal's state in its item, the universal
//! terminal's packets, and the Magnet Card (`magnet`). Derived from AE2WTLib by Mari023 (`WETMenuHost`,
//! `CycleTerminalPacket`, `SelectTerminalPacket`),
//! https://github.com/Mari023/AE2WirelessTerminalLibrary tag `19.5.1`, MIT,
//! and AE2 19.2.18 `PatternEncodingLogic` (LGPL-3.0-or-later); see
//! `ae2/addons/ae2wtlib/`. Which items are terminals, and the universal
//! terminal's rules, are the registry's (`horizon_client::ae2x::wireless`).
use super::super::{Game, ae2_encoding::EncodingState, ae2_processing::PatternFluid, lan::LanHost};
use horizon_client::{
    ae2_generic_stack::{self, GenericStack},
    ae2_world::{Machine, MachineKind},
    ae2x::ae2wtlib::PATTERN_ENCODING_LOGIC,
    items::ItemStack,
    save,
};
use horizon_world::nbt::{Compound, Tag};

mod magnet;
pub(in crate::game) mod magnet_screen;
pub(in crate::game) mod settings_screen;

/// AE2WTLib's hotkeys after AE2's (`key.ae2.<id>`): the two terminals'
/// (`WTDefinitionBuilder.hotkeyName`, answered by the terminal locating
/// service) and the Magnet Card's (`MagnetHotkeyAction`). Restock and stow
/// come with their slice.
pub(in crate::game) const HOTKEYS: &[&str] = &["wireless_pattern_encoding_terminal", "wireless_pattern_access_terminal", magnet::MAGNET_HOTKEY];

/// `CycleTerminalPacket` (one byte: the right button, backwards).
pub(in crate::game) const CYCLE_TERMINAL: &str = "ae2wtlib:cycle_terminal";
/// `SelectTerminalPacket` (the terminal's name).
pub(in crate::game) const SELECT_TERMINAL: &str = "ae2wtlib:select_terminal";

/// `PatternEncodingLogic`'s inventories: 81 encoded inputs, 27 outputs.
const INPUTS: usize = 81;
const OUTPUTS: usize = 27;
const MODES: [&str; 4] = ["CRAFTING", "PROCESSING", "SMITHING_TABLE", "STONECUTTING"];

/// `AppEngInternalInventory.readFromNBT` of a one-slot inventory.
fn read_slot(data: &Compound, name: &str) -> Result<Option<ItemStack>, String> {
    let Some(tag) = data.get(&name.into()) else {
        return Ok(None);
    };
    let Tag::List(entries) = tag else {
        return Err(format!("invalid {name}"));
    };
    let mut slot = None;
    for entry in entries {
        if matches!(entry.get("Slot"), Some(Tag::Int(0)) | None) {
            slot = save::item_from_nbt(entry);
        }
    }
    Ok(slot)
}
/// `AppEngInternalInventory.writeToNBT`: the slot under `Slot`, the key
/// removed when empty.
fn write_slot(data: &mut Compound, name: &str, item: Option<&ItemStack>) {
    match item {
        None => {
            data.remove(&name.into());
        }
        Some(item) => {
            let mut tag = save::item_to_nbt(item);
            if let Tag::Compound(map) = &mut tag {
                map.insert("Slot".into(), Tag::Int(0));
            }
            data.insert(name.into(), Tag::List(vec![tag]));
        }
    }
}
/// `GenericStackInv.readFromChildTag`: missing clears; each slot a
/// `GenericStack` or empty.
fn read_stacks(data: &Compound, name: &str, items: &mut [Option<ItemStack>], fluids: &mut [Option<PatternFluid>]) -> Result<(), String> {
    let Some(tag) = data.get(&name.into()) else {
        return Ok(());
    };
    let Tag::List(entries) = tag else {
        return Err(format!("invalid {name}"));
    };
    for (slot, entry) in entries.iter().enumerate().take(items.len()) {
        let Some(stack) = ae2_generic_stack::decode_tolerant(entry)? else { continue };
        let amount = u64::try_from(stack.amount).map_err(|_| format!("negative {name} amount"))?;
        if amount == 0 {
            continue;
        }
        if stack.fluid {
            fluids[slot] = Some(PatternFluid { key: stack.key, amount });
        } else {
            let mut item = horizon_client::ae2_cell_codec::prototype(&stack.key)?;
            item.count = u32::try_from(amount).map_err(|_| format!("{name} amount too large"))?;
            items[slot] = Some(item);
        }
    }
    Ok(())
}
/// `GenericStackInv.writeToChildTag`: the slots as `GenericStack`s, trailing
/// empty ones left out, the key removed when all are empty.
fn write_stacks(data: &mut Compound, name: &str, items: &[Option<ItemStack>], fluids: &[Option<PatternFluid>]) -> Result<(), String> {
    let mut list = Vec::new();
    for slot in 0..items.len() {
        let stack = match (&items[slot], fluids.get(slot).and_then(Option::as_ref)) {
            (Some(item), _) => Some(GenericStack { key: horizon_client::ae2_cell_codec::key(item)?, fluid: false, amount: i64::from(item.count) }),
            (None, Some(fluid)) => Some(GenericStack { key: fluid.key.clone(), fluid: true, amount: i64::try_from(fluid.amount).map_err(|_| "fluid amount too large")? }),
            (None, None) => None,
        };
        list.push(match stack {
            Some(stack) => ae2_generic_stack::encode(&stack)?,
            None => Tag::Compound(Compound::new()),
        });
    }
    while matches!(list.last(), Some(Tag::Compound(c)) if c.is_empty()) {
        list.pop();
    }
    if list.is_empty() {
        data.remove(&name.into());
    } else {
        data.insert(name.into(), Tag::List(list));
    }
    Ok(())
}

/// `PatternEncodingLogic.readFromNBT`: the encoding state, the blank and the
/// encoded pattern slots. An unknown mode is crafting.
pub(in crate::game) fn read_logic(data: &Compound) -> Result<(EncodingState, [Option<ItemStack>; 2]), String> {
    let mut state = EncodingState::default();
    let flag = |key: &str| matches!(data.get(&key.into()), Some(Tag::Byte(1)));
    let mode = match data.get(&"mode".into()) {
        Some(Tag::String(mode)) => MODES.iter().position(|m| mode.to_string().is_ok_and(|s| s == *m)).unwrap_or(0),
        _ => 0,
    };
    state.processing = mode == 1;
    state.special_mode = match mode {
        2 => 1,
        3 => 2,
        _ => 0,
    };
    state.substitutes = flag("substitute");
    state.fluid_substitutes = flag("substituteFluids");
    state.stone_recipe = match data.get(&"stonecuttingRecipeId".into()) {
        Some(Tag::String(id)) => Some(id.to_string().map_err(|_| "invalid stonecutting recipe")?),
        _ => None,
    };
    read_stacks(data, "encodedInputs", &mut state.inputs, &mut state.fluid_inputs)?;
    read_stacks(data, "encodedOutputs", &mut state.outputs, &mut state.fluid_outputs)?;
    Ok((state, [read_slot(data, "blankPattern")?, read_slot(data, "encodedPattern")?]))
}
/// `PatternEncodingLogic.writeToNBT` into the tag the item already has
/// (`WETMenuHost.markForSave`: a stonecutting recipe once written stays
/// until another replaces it).
pub(in crate::game) fn write_logic(data: &mut Compound, state: &EncodingState, slots: &[Option<ItemStack>]) -> Result<(), String> {
    if state.inputs.len() != INPUTS || state.outputs.len() != OUTPUTS || slots.len() != 2 {
        return Err("invalid encoding terminal state".into());
    }
    let mode = if state.processing { 1 } else { match state.special_mode { 1 => 2, 2 => 3, _ => 0 } };
    data.insert("mode".into(), Tag::String(MODES[mode].into()));
    data.insert("substitute".into(), Tag::Byte(i8::from(state.substitutes)));
    data.insert("substituteFluids".into(), Tag::Byte(i8::from(state.fluid_substitutes)));
    if let Some(recipe) = &state.stone_recipe {
        data.insert("stonecuttingRecipeId".into(), Tag::String(recipe.as_str().into()));
    }
    write_slot(data, "blankPattern", slots[0].as_ref());
    write_slot(data, "encodedPattern", slots[1].as_ref());
    write_stacks(data, "encodedInputs", &state.inputs, &state.fluid_inputs)?;
    write_stacks(data, "encodedOutputs", &state.outputs, &state.fluid_outputs)
}

/// The wireless pattern encoding terminal as the encoding terminal's machine:
/// its two pattern slots and its state, from `pattern_encoding_logic`.
pub(in crate::game) fn encoding_machine(stack: &ItemStack) -> Result<Machine, String> {
    let (state, slots) = match stack.data().and_then(|d| d.other(PATTERN_ENCODING_LOGIC)) {
        None => (EncodingState::default(), [None, None]),
        Some(Tag::Compound(data)) => read_logic(data)?,
        Some(_) => return Err("invalid pattern encoding logic".into()),
    };
    let mut machine = Machine::new(MachineKind::PatternEncodingTerminal);
    machine.slots = slots.to_vec();
    state.save(&mut machine)?;
    Ok(machine)
}
/// `WETMenuHost.markForSave`: the machine's state back into the item.
pub(in crate::game) fn store_encoding(stack: &mut ItemStack, machine: &Machine) -> Result<(), String> {
    let state = EncodingState::load(machine)?;
    let mut data = match stack.data().and_then(|d| d.other(PATTERN_ENCODING_LOGIC)) {
        Some(Tag::Compound(data)) => data.clone(),
        _ => Compound::new(),
    };
    write_logic(&mut data, &state, &machine.slots)?;
    stack.data_mut().set_other(PATTERN_ENCODING_LOGIC, Some(Tag::Compound(data)));
    horizon_client::ae2_stack_identity::synchronize_tracked(stack);
    Ok(())
}

/// `TextConstants.currentTerminal`: "Current Terminal: <item name>", the name grey.
fn current_terminal_text(terminal: &horizon_client::ae2x::defs::WirelessTerminal) -> horizon_client::text::Text {
    let (namespace, path) = terminal.item.split_once(':').unwrap_or(("minecraft", &terminal.item));
    let name = horizon_client::text::Text::translate(&format!("item.{namespace}.{path}"), &[]).with_color(horizon_client::text::GRAY);
    horizon_client::text::Text::translate("gui.ae2wtlib.current_terminal", &[name])
}
impl Game<'_> {
    /// `AE2wtlibClient.mouseScroll`: shift and the wheel with no screen open
    /// turn a universal terminal in the main or off hand to its next terminal
    /// (down) or the one before (up), on the server (`CycleTerminalPacket`).
    pub(in crate::game) fn ae2wtlib_item_wheel(&mut self, delta: f32) -> bool {
        let held = |stack: Option<&ItemStack>| stack.is_some_and(horizon_client::ae2x::wireless::is_universal);
        if self.remote.is_none() || !(held(self.inventory.get(self.selected).and_then(Option::as_ref)) || held(self.offhand.as_ref())) {
            return false;
        }
        self.ae2_send_payload(CYCLE_TERMINAL, vec![u8::from(delta < 0.0)]);
        true
    }
    /// `CycleTerminalPacket.processPacketData`: in a universal terminal's
    /// menu, the next (or previous) terminal it holds, opened in its place;
    /// with no menu open, the universal terminal in the main hand, else the
    /// off hand, turns to it and says which it is now.
    fn lan_cycle_universal_terminal(&mut self, host: &mut LanHost, conn: u32, reverse: bool) {
        let in_menu = self.lan_wireless_menu_source(host, conn);
        let any_menu = in_menu.is_some() || host.play.get(&conn).is_some_and(|p| p.ae2_terminal.is_some() || p.ae2_pattern_access.is_some());
        let Some(g) = host.hub.guests.iter().find(|g| g.conn == conn && !g.spectator()) else { return };
        let source = match in_menu {
            Some(source) => source,
            None if any_menu => return,
            None if g.inventory.get(g.selected).and_then(Option::as_ref).is_some_and(horizon_client::ae2x::wireless::is_universal) => g.selected,
            None if g.offhand.as_ref().is_some_and(horizon_client::ae2x::wireless::is_universal) => super::super::shield::OFFHAND_SLOT,
            None => return,
        };
        let Some(stack) = super::super::ae2_wireless::source_stack(g, source).filter(|s| horizon_client::ae2x::wireless::is_universal(s)) else { return };
        let Some(next) = horizon_client::ae2x::wireless::next_terminal(stack, reverse) else { return };
        let Some(stack) = host.hub.guests.iter_mut().find(|g| g.conn == conn).and_then(|g| super::super::ae2_wireless::source_stack_mut(g, source)) else { return };
        horizon_client::ae2x::wireless::set_current(stack, next);
        if in_menu.is_some() {
            self.lan_open_ae2_wireless_source(host, conn, source);
        } else {
            let text = current_terminal_text(next);
            host.hub.send_to(conn, &horizon_protocol::packets::play::Clientbound::SystemChat(horizon_protocol::packets::play::SystemChat {
                content: horizon_client::lan::data::component_nbt(&text.component()),
                overlay: true,
            }));
            if let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) {
                host.hub.send_inventory(gi);
            }
        }
    }
    /// `SelectTerminalPacket`: in a universal terminal's menu, a terminal it
    /// holds becomes the current one and opens in its place.
    fn lan_select_universal_terminal(&mut self, host: &mut LanHost, conn: u32, name: &str) {
        let Some(source) = self.lan_wireless_menu_source(host, conn) else { return };
        let Some(terminal) = horizon_client::ae2x::defs::get().wireless_terminal_named(name) else { return };
        let Some(stack) = host.hub.guests.iter_mut().find(|g| g.conn == conn).and_then(|g| super::super::ae2_wireless::source_stack_mut(g, source)) else { return };
        if !horizon_client::ae2x::wireless::is_universal(stack) || !horizon_client::ae2x::wireless::has_terminal(stack, terminal) {
            return;
        }
        horizon_client::ae2x::wireless::set_current(stack, terminal);
        self.lan_open_ae2_wireless_source(host, conn, source);
    }
    /// AE2WTLib's own hotkey actions (the terminals' are the locating
    /// service's, before them): true when one ran.
    pub(in crate::game) fn lan_ae2wtlib_hotkey(&mut self, host: &mut LanHost, conn: u32, id: &str) -> bool {
        id == magnet::MAGNET_HOTKEY && self.lan_ae2wtlib_magnet_hotkey(host, conn)
    }
    /// The universal terminal's packets and the terminal settings.
    pub(in crate::game) fn lan_ae2wtlib_payload(&mut self, host: &mut LanHost, conn: u32, channel: &str, data: &[u8]) -> bool {
        match channel {
            magnet::TERMINAL_SETTINGS => {
                self.lan_ae2wtlib_terminal_settings(host, conn, data);
                true
            }
            magnet::MAGNET_MENU => {
                self.lan_ae2wtlib_magnet_menu(host, conn, data);
                true
            }
            CYCLE_TERMINAL => {
                self.lan_cycle_universal_terminal(host, conn, data.first().is_some_and(|b| *b != 0));
                true
            }
            SELECT_TERMINAL => {
                if let Some(name) = std::str::from_utf8(data).ok().filter(|n| n.len() <= 256) {
                    self.lan_select_universal_terminal(host, conn, name);
                }
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding_logic_roundtrips_in_the_source_format() {
        let mut state = EncodingState::default();
        state.processing = true;
        state.substitutes = true;
        state.inputs[0] = Some(ItemStack::new("minecraft:oak_log", 3));
        state.fluid_inputs[4] = Some(PatternFluid { key: ae2_cell_core::ItemKey { item_id: "minecraft:water".into(), components: Vec::new() }, amount: 250 });
        state.outputs[1] = Some(ItemStack::new("minecraft:oak_planks", 12));
        let slots = [Some(ItemStack::new("ae2:blank_pattern", 5)), None];
        let mut data = Compound::new();
        write_logic(&mut data, &state, &slots).unwrap();
        assert_eq!(data.get(&"mode".into()), Some(&Tag::String("PROCESSING".into())));
        let Some(Tag::List(inputs)) = data.get(&"encodedInputs".into()) else { panic!() };
        assert_eq!(inputs.len(), 5, "trailing empty slots are left out");
        assert_eq!(inputs[0].get("#t"), Some(&Tag::String("ae2:i".into())));
        assert_eq!(inputs[4].get("#t"), Some(&Tag::String("ae2:f".into())));
        assert!(data.get(&"encodedPattern".into()).is_none());
        let (back, back_slots) = read_logic(&data).unwrap();
        assert_eq!(back.inputs[0], state.inputs[0]);
        assert_eq!(back.fluid_inputs[4], state.fluid_inputs[4]);
        assert_eq!(back.outputs[1], state.outputs[1]);
        assert!(back.processing && back.substitutes && back.fluid_substitutes, "the default fluid substitution (on) kept");
        assert_eq!(back_slots[0].as_ref().map(|s| s.count), Some(5));

        // An item keeps it across a save, as the machine the terminal edits.
        let mut terminal = ItemStack::new(horizon_client::ae2x::ae2wtlib::PATTERN_ENCODING_TERMINAL, 1);
        let mut machine = encoding_machine(&terminal).unwrap();
        assert_eq!(machine.slots.len(), 2);
        machine.slots[0] = slots[0].clone();
        state.save(&mut machine).unwrap();
        store_encoding(&mut terminal, &machine).unwrap();
        let restored = save::item_from_nbt(&save::item_to_nbt(&terminal)).unwrap();
        let again = encoding_machine(&restored).unwrap();
        assert_eq!(again.slots, machine.slots);
        assert_eq!(EncodingState::load(&again).unwrap().inputs, state.inputs);
        // Crafting with a stonecutting recipe once set: the id stays, as
        // `markForSave` writes into the item's existing tag.
        let mut stone = EncodingState { special_mode: 2, stone_recipe: Some("minecraft:stone_bricks_from_stone_stonecutting".into()), ..EncodingState::default() };
        write_logic(&mut data, &stone, &slots).unwrap();
        stone.stone_recipe = None;
        stone.special_mode = 0;
        write_logic(&mut data, &stone, &slots).unwrap();
        assert_eq!(data.get(&"mode".into()), Some(&Tag::String("CRAFTING".into())));
        assert!(data.get(&"stonecuttingRecipeId".into()).is_some());
        assert!(data.get(&"encodedInputs".into()).is_none(), "all empty: removed");
    }
}
