//! AE2 inventory hotkeys: Hotkeys, HotkeyActions and InventoryHotkeyAction.
//! Source behavior follows the pinned AE2 19.2.18 reference, LGPL-3.0-or-later.
use super::{Game, Screen, lan::LanHost};
use horizon_client::items::ItemStack;
use horizon_protocol::packets::{configuration::CustomPayload, play::Serverbound};
use winit::{event::MouseButton, keyboard::KeyCode};

pub(super) const CHANNEL: &str = "ae2:hotkey";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action { Wireless, ItemCell, FluidCell }
impl Action {
    const ALL: [Self; 3] = [Self::Wireless, Self::ItemCell, Self::FluidCell];
    fn id(self) -> &'static str { match self {
        Self::Wireless => "wireless_terminal", Self::ItemCell => "portable_item_cell", Self::FluidCell => "portable_fluid_cell",
    }}
    fn mapping(self) -> &'static str { match self {
        Self::Wireless => "key.ae2.wireless_terminal", Self::ItemCell => "key.ae2.portable_item_cell", Self::FluidCell => "key.ae2.portable_fluid_cell",
    }}
    fn parse(data: &[u8]) -> Option<Self> {
        Self::ALL.into_iter().find(|action| data == action.id().as_bytes())
    }
    /// Later registration wins before InventoryHotkeyAction scans slots 0..35.
    fn rank(self, item: &str) -> Option<usize> {
        match self {
            Self::Wireless => match item {
                "ae2:wireless_crafting_terminal" => Some(0), "ae2:wireless_terminal" => Some(1), _ => None,
            },
            // HotkeyActions.register adds each action first: the registry's
            // portable cells of the key type, AE2's then the addons'
            // (MEGACells.initStorageCells), last registered first.
            Self::ItemCell | Self::FluidCell => {
                let channel = if self == Self::ItemCell { ae2_cell_core::KeyType::ITEMS } else { ae2_cell_core::KeyType::FLUIDS };
                let order: Vec<_> = horizon_client::ae2x::defs::get().portable_cells().filter(|cell| cell.key_type == channel).collect();
                order.iter().rev().position(|cell| cell.id == item)
            }
        }
    }
}
fn candidates(action: Action, inventory: &[Option<ItemStack>]) -> Vec<usize> {
    let mut ranked: Vec<_> = inventory.iter().take(36).enumerate().filter_map(|(slot, stack)| {
        let stack = stack.as_ref().filter(|stack| stack.count > 0)?;
        Some((action.rank(&stack.item)?, slot))
    }).collect();
    ranked.sort_unstable();ranked.into_iter().map(|(_, slot)| slot).collect()
}
/// AE2WTLib's `WUTHandler.findTerminal` for `Ae2wtlibLocatingService`: in
/// slot order, a universal terminal holding the terminal at once, else the
/// first of the terminal's own items (the main inventory, armour, off hand).
pub(super) fn locate(g: &horizon_client::lan::guest::Guest, terminal: &horizon_client::ae2x::defs::WirelessTerminal) -> Option<usize> {
    locate_in(&g.inventory, g.offhand.as_ref(), terminal)
}
/// [`locate`] over a player's inventory and off hand.
pub(super) fn locate_in(inventory: &[Option<ItemStack>], offhand: Option<&ItemStack>, terminal: &horizon_client::ae2x::defs::WirelessTerminal) -> Option<usize> {
    let stacks = (0..inventory.len()).map(|slot| (slot, inventory[slot].as_ref()))
        .chain(std::iter::once((super::shield::OFFHAND_SLOT, offhand)));
    let mut found = None;
    for (slot, stack) in stacks {
        let Some(stack) = stack.filter(|s| horizon_client::ae2x::wireless::has_terminal(s, terminal)) else { continue };
        if horizon_client::ae2x::wireless::is_universal(stack) {
            return Some(slot);
        }
        found.get_or_insert(slot);
    }
    found
}
impl Game<'_> {
    /// Named physical input is examined before vanilla's default-key mapping.
    fn ae2_hotkey_input(&mut self, input: &str, down: bool, repeat: bool) {
        if !down || repeat || self.screen != Screen::Playing { return; }
        // The addons' hotkeys, by id: the server finds what answers them.
        let addons: Vec<&str> = Self::ae2x_hotkeys().iter().copied().filter(|id| self.settings.keys.get(&super::ae2x::hotkey_mapping(id)) == input).collect();
        if self.remote.is_some() {
            for id in addons {
                self.send(Serverbound::CustomPayload(CustomPayload {channel: CHANNEL.into(), data: id.as_bytes().to_vec()}));
            }
        }
        let actions: Vec<_> = Action::ALL.into_iter().filter(|action| self.settings.keys.get(action.mapping()) == input).collect();
        for action in actions {
            if self.remote.is_some() {
                self.send(Serverbound::CustomPayload(CustomPayload {channel: CHANNEL.into(), data: action.id().as_bytes().to_vec()}));
            } else if matches!(action, Action::ItemCell | Action::FluidCell) {
                for source in candidates(action, &self.inventory) {
                    if self.ae2_open_local_portable_source(source) { break; }
                }
            }
        }
    }
    /// True when an open addon screen took the key (Advanced AE's encoder
    /// closing on its hotkey, `isCloseHotkey`: a key, not a mouse button);
    /// no other key handling sees it then.
    pub(super) fn ae2_hotkey_key(&mut self, key: KeyCode, down: bool, repeat: bool) -> bool {
        let Some(name) = horizon_client::keybinds::key_name(key) else { return false };
        if down && !repeat && self.screen == Screen::Inventory && self.ae2x_screen_hotkey(name) { return true; }
        self.ae2_hotkey_input(name, down, repeat);
        false
    }
    pub(super) fn ae2_hotkey_mouse(&mut self, button: MouseButton, down: bool) {
        self.ae2_hotkey_input(&horizon_client::keybinds::mouse_name(button), down, false);
    }
    /// The client supplies an action name only; the server chooses and validates
    /// its actual inventory owner. No client slot or item payload is trusted.
    pub(super) fn lan_ae2_hotkey(&mut self, host: &mut LanHost, conn: u32, data: &[u8]) {
        // The addons' terminals on this hotkey come first, the last registered
        // first (HotkeyActions.register): AE2WTLib's locating service makes
        // the universal terminal's current terminal the one the key names.
        if let Ok(id) = std::str::from_utf8(data) {
            let terminals: Vec<_> = horizon_client::ae2x::defs::get().wireless_terminals()
                .filter(|t| t.installed.is_some() && t.hotkey == id).collect();
            for terminal in terminals.into_iter().rev() {
                let Some(guest) = host.hub.guests.iter().find(|guest| guest.conn == conn && !guest.spectator()) else { return; };
                let Some(source) = locate(guest, terminal) else { continue };
                if let Some(stack) = host.hub.guests.iter_mut().find(|g| g.conn == conn).and_then(|g| super::ae2_wireless::source_stack_mut(g, source)) {
                    horizon_client::ae2x::wireless::set_current(stack, terminal);
                }
                self.lan_open_ae2_wireless_source(host, conn, source);
                if self.lan_wireless_menu_source(host, conn) == Some(source) { return; }
            }
            if self.lan_ae2x_hotkey(host, conn, id) { return; }
        }
        let Some(action) = Action::parse(data) else { return; };
        let Some(guest) = host.hub.guests.iter().find(|guest| guest.conn == conn && !guest.spectator()) else { return; };
        let sources = candidates(action, &guest.inventory);
        for source in sources {
            let opened = match action {
                Action::Wireless => {
                    self.lan_open_ae2_wireless_source(host, conn, source);
                    self.lan_ae2_wireless_source_open(host, conn, source)
                },
                Action::ItemCell | Action::FluidCell => self.lan_open_ae2_portable_source(host, conn, source),
            };
            if opened { break; }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inventory_hotkeys_follow_source_registration_then_slot_order() {
        let mut inventory = vec![None; 41];
        inventory[0] = Some(ItemStack::new("ae2:wireless_terminal", 1));
        inventory[24] = Some(ItemStack::new("ae2:wireless_crafting_terminal", 1));
        inventory[40] = Some(ItemStack::new("ae2:wireless_crafting_terminal", 1));
        assert_eq!(candidates(Action::Wireless, &inventory), vec![24, 0]);
        inventory[1] = Some(ItemStack::new("ae2:portable_item_cell_1k", 1));
        inventory[2] = Some(ItemStack::new("ae2:portable_item_cell_1k", 1));
        inventory[30] = Some(ItemStack::new("ae2:portable_item_cell_256k", 1));
        inventory[31] = Some(ItemStack::new("ae2:portable_fluid_cell_256k", 1));
        assert_eq!(candidates(Action::ItemCell, &inventory), vec![30, 1, 2]);
        assert_eq!(candidates(Action::FluidCell, &inventory), vec![31]);
        assert_eq!(Action::parse(b"wireless_terminal"), Some(Action::Wireless));
        assert_eq!(Action::parse(b"wireless_terminal\0"), None);
        assert_eq!(Action::parse(b"other_mod:wireless"), None);
    }
    /// MEGACells.initStorageCells registers its portable cells' hotkeys after
    /// AE2's, each first: MEGA's largest cell opens before AE2's.
    #[cfg(feature = "ae2-megacells")]
    #[test]
    fn mega_portable_cells_answer_the_hotkey_before_ae2s() {
        let mut inventory = vec![None; 41];
        inventory[0] = Some(ItemStack::new("ae2:portable_item_cell_256k", 1));
        inventory[3] = Some(ItemStack::new("megacells:portable_item_cell_1m", 1));
        inventory[7] = Some(ItemStack::new("megacells:portable_item_cell_256m", 1));
        inventory[8] = Some(ItemStack::new("megacells:portable_fluid_cell_4m", 1));
        inventory[9] = Some(ItemStack::new("ae2:portable_fluid_cell_256k", 1));
        assert_eq!(candidates(Action::ItemCell, &inventory), vec![7, 3, 0]);
        assert_eq!(candidates(Action::FluidCell, &inventory), vec![8, 9]);
    }
}
