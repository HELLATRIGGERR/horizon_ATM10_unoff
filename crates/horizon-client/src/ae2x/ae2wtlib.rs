//! AE2WTLib 19.5.1's rows in the definitions registry (feature
//! `ae2-ae2wtlib`). Derived from AE2WTLib by Mari023 (`AE2wtlib`,
//! `AE2wtlibItems`, `AE2wtlibComponents`, `WTDefinitionBuilder`,
//! `UpgradeHelper`), https://github.com/Mari023/AE2WirelessTerminalLibrary
//! tag `19.5.1`, MIT; see `ae2/addons/ae2wtlib/`. The universal terminal's
//! rules are the registry's (`ae2x::wireless`), the game's side in
//! `game/ae2x/ae2wtlib/`.
use super::defs::{Defs, TerminalKind, UniversalTerminal, WirelessTerminal};

pub mod magnet;
pub mod recipes;
pub mod restock;

pub const NAMESPACE: &str = "ae2wtlib";
/// `AE2wtlibItems.UNIVERSAL_TERMINAL`.
pub const UNIVERSAL_TERMINAL: &str = "ae2wtlib:wireless_universal_terminal";
pub const PATTERN_ENCODING_TERMINAL: &str = "ae2wtlib:wireless_pattern_encoding_terminal";
pub const PATTERN_ACCESS_TERMINAL: &str = "ae2wtlib:wireless_pattern_access_terminal";
/// `AE2wtlibComponents.CURRENT_TERMINAL`: the terminal's name (`WTDefinition.CODEC`).
pub const CURRENT_TERMINAL: &str = "ae2wtlib:current_terminal";
/// `AE2wtlibComponents.PATTERN_ENCODING_LOGIC`: the wireless encoding
/// terminal's `PatternEncodingLogic.writeToNBT`.
pub const PATTERN_ENCODING_LOGIC: &str = "ae2wtlib:pattern_encoding_logic";

/// `AE2wtlib.onAe2Initialized`'s terminals in their order, each by
/// `AddTerminalEvent.builder(name, host, menu, item, icon)`: the hotkey
/// `wireless_<name>_terminal` (the crafting terminal's is AE2's
/// `HotkeyAction.WIRELESS_TERMINAL`), two upgrade slots, the component
/// `has_<name>_terminal`.
pub const TERMINALS: [(&str, &str, TerminalKind, &str); 3] = [
    ("crafting", "ae2:wireless_crafting_terminal", TerminalKind::Crafting, "wireless_terminal"),
    ("pattern_encoding", PATTERN_ENCODING_TERMINAL, TerminalKind::PatternEncoding, "wireless_pattern_encoding_terminal"),
    ("pattern_access", PATTERN_ACCESS_TERMINAL, TerminalKind::PatternAccess, "wireless_pattern_access_terminal"),
];

/// The terminals, the universal terminal, and their cards
/// (`UpgradeHelper.addUpgrades`: AE2's energy card up to each terminal's
/// `upgradeCount`, up to `WUTHandler.getUpgradeCardCount` on the universal
/// terminal; `onAe2Initialized`: one Magnet Card on AE2's wireless crafting
/// terminal and one on the universal terminal). The Quantum Bridge Card
/// comes with its slice.
pub fn register(defs: &mut Defs) {
    for (name, item, kind, hotkey) in TERMINALS {
        defs.add_wireless_terminal(WirelessTerminal {
            name: name.to_owned(),
            item: item.to_owned(),
            kind,
            hotkey: hotkey.to_owned(),
            upgrade_slots: 2,
            installed: Some(format!("{NAMESPACE}:has_{name}_terminal")),
        });
    }
    defs.set_universal_terminal(UniversalTerminal { item: UNIVERSAL_TERMINAL.to_owned(), current: CURRENT_TERMINAL.to_owned() });
    defs.add_upgrade_to_all_terminals("ae2:energy_card", 0);
    defs.add_upgrade(magnet::MAGNET_CARD, "ae2:wireless_crafting_terminal", 1);
    defs.add_upgrade(magnet::MAGNET_CARD, UNIVERSAL_TERMINAL, 1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ae2_upgrade_inventory::Inventory,
        ae2x::{defs, wireless},
        items::ItemStack,
    };
    use horizon_world::nbt::{Compound, Tag};

    fn universal(terminals: &[&str]) -> ItemStack {
        let mut wut = ItemStack::new(UNIVERSAL_TERMINAL, 1);
        for name in terminals {
            wut.data_mut().set_other(&format!("ae2wtlib:has_{name}_terminal"), Some(Tag::Compound(Compound::new())));
        }
        wut
    }

    #[test]
    fn rows_replace_ae2s_crafting_terminal_and_list_in_registration_order() {
        let defs = defs::get();
        let names: Vec<_> = defs.wireless_terminals().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["ae2:wireless_terminal", "crafting", "pattern_encoding", "pattern_access"]);
        let crafting = defs.wireless_terminal("ae2:wireless_crafting_terminal").unwrap();
        assert_eq!((crafting.kind, crafting.hotkey.as_str()), (TerminalKind::Crafting, "wireless_terminal"));
        assert_eq!(crafting.installed.as_deref(), Some("ae2wtlib:has_crafting_terminal"));
        let access = defs.wireless_terminal(PATTERN_ACCESS_TERMINAL).unwrap();
        assert_eq!(access.hotkey, "wireless_pattern_access_terminal");
        assert!(defs.is_wireless_terminal(UNIVERSAL_TERMINAL));
        assert_eq!(defs.upgrade_limit("ae2:energy_card", PATTERN_ENCODING_TERMINAL), 2);
        assert_eq!(defs.upgrade_limit("ae2:energy_card", UNIVERSAL_TERMINAL), 6);
        assert_eq!(defs.wireless_terminal("ae2:wireless_terminal").unwrap().installed, None, "AE2's own terminal can't be combined");
        for (item, magnet) in [("ae2:wireless_crafting_terminal", 1), (UNIVERSAL_TERMINAL, 1), (PATTERN_ACCESS_TERMINAL, 0), ("ae2:wireless_terminal", 0)] {
            assert_eq!(defs.upgrade_limit(magnet::MAGNET_CARD, item), magnet, "{item}");
        }
    }

    /// MEGA's `AE2WTIntegration`: its Greater Energy Card on every terminal
    /// the universal terminal can hold, and six on the universal terminal.
    #[cfg(feature = "ae2-megacells")]
    #[test]
    fn megas_greater_energy_card_fits_every_terminal() {
        let defs = defs::get();
        let card = crate::ae2x::megacells::GREATER_ENERGY_CARD;
        for item in [PATTERN_ENCODING_TERMINAL, PATTERN_ACCESS_TERMINAL, "ae2:wireless_crafting_terminal", "ae2:wireless_terminal"] {
            assert_eq!(defs.upgrade_limit(card, item), 2, "{item}");
        }
        assert_eq!(defs.upgrade_limit(card, UNIVERSAL_TERMINAL), 6);
    }

    #[test]
    fn universal_terminal_opens_current_or_first_and_cycles_its_own() {
        let empty = universal(&[]);
        assert!(wireless::host(&empty));
        assert_eq!(wireless::terminal(&empty), None);
        assert_eq!(wireless::next_terminal(&empty, false), None);
        assert_eq!(wireless::upgrade_slots(&empty), Some(0));

        let mut wut = universal(&["pattern_access", "crafting"]);
        assert_eq!(wireless::terminal(&wut).unwrap().name, "crafting", "the first held in registration order");
        assert_eq!(wireless::upgrade_slots(&wut), Some(4), "two slots for each terminal held");
        assert_eq!(wireless::next_terminal(&wut, false).unwrap().name, "pattern_access", "pattern_encoding is not held");
        assert_eq!(wireless::next_terminal(&wut, true).unwrap().name, "pattern_access", "backwards round the end");
        let access = defs::get().wireless_terminal_named("pattern_access").unwrap();
        assert!(wireless::set_current(&mut wut, access));
        assert_eq!(wireless::kind(&wut), Some(TerminalKind::PatternAccess));
        assert_eq!(wireless::next_terminal(&wut, false).unwrap().name, "crafting");
        let encoding = defs::get().wireless_terminal_named("pattern_encoding").unwrap();
        assert!(!wireless::set_current(&mut wut, encoding), "only a terminal it holds");
        let mut plain = ItemStack::new(PATTERN_ACCESS_TERMINAL, 1);
        assert!(!wireless::set_current(&mut plain, access), "only a universal terminal has a current one");
        // A name no terminal has: the first held, as an absent one.
        wut.data_mut().set_other(CURRENT_TERMINAL, Some(Tag::String("restock".into())));
        assert_eq!(wireless::terminal(&wut).unwrap().name, "crafting");
    }

    #[test]
    fn energy_adds_up_for_the_terminals_held_and_their_cards() {
        let base = crate::ae2_common_config::wireless().terminal_capacity;
        let mut crafting = ItemStack::new("ae2:wireless_crafting_terminal", 1);
        let mut cards = Inventory::read(&crafting).unwrap();
        assert!(cards.insert_one(&ItemStack::new("ae2:energy_card", 1)));
        cards.store(&mut crafting).unwrap();
        assert_eq!(wireless::energy(&crafting).unwrap().1, 2.0 * base);
        wireless::transfer_energy(&mut crafting, 1_000_000.0, true, false).unwrap();
        let mut encoding = ItemStack::new(PATTERN_ENCODING_TERMINAL, 1);
        wireless::transfer_energy(&mut encoding, 300_000.0, true, false).unwrap();
        encoding.data_mut().set_other("ae2:wireless_link_target", Some(Tag::compound([
            ("dimension", Tag::String("minecraft:overworld".into())),
            ("pos", Tag::IntArray(vec![1, 64, 2])),
        ])));

        let defs = defs::get();
        let wut = wireless::merge_terminal(&universal(&[]), &crafting, defs.wireless_terminal_named("crafting").unwrap()).unwrap();
        // One terminal and its card: two batteries; its energy came along.
        assert_eq!(wireless::energy(&wut).unwrap(), (1_000_000.0, 2.0 * base));
        assert_eq!(wireless::charge_rate(&wut).unwrap(), 800.0 * 3.0);
        let wut = wireless::merge_terminal(&wut, &encoding, defs.wireless_terminal_named("pattern_encoding").unwrap()).unwrap();
        assert_eq!(wireless::installed(&wut).len(), 2);
        assert_eq!(wireless::energy(&wut).unwrap(), (1_300_000.0, 3.0 * base), "two terminals and one card");
        assert_eq!(Inventory::read(&wut).unwrap().slots.len(), 4);
        assert!(wut.data().unwrap().other("ae2:wireless_link_target").is_some(), "the merged terminal's other components");
        assert!(wireless::merge_terminal(&wut, &ItemStack::new("ae2:wireless_terminal", 1), defs.wireless_terminal("ae2:wireless_terminal").unwrap()).is_err());
        assert!(wireless::merge_terminal(&crafting, &encoding, defs.wireless_terminal_named("pattern_encoding").unwrap()).is_err());
    }
}
