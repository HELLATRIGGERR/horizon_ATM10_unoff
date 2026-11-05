//! AE2WTLib's Magnet Card on a terminal item: its mode (`MagnetMode`,
//! `MagnetHandler.getMagnetMode`/`saveMagnetMode`), its pickup and insert
//! filters (`MagnetHost`), the terminal settings (`TerminalSettingsPacket`),
//! the hotkey's cycle (`MagnetHotkeyAction`) and the range
//! (`AE2wtlibConfig`). AE2WTLib 19.5.1 by Mari023,
//! https://github.com/Mari023/AE2WirelessTerminalLibrary tag `19.5.1`, MIT;
//! see `ae2/addons/ae2wtlib/`. The player's side is the game's
//! (`game/ae2x/ae2wtlib/magnet.rs`).
use crate::{ae2_upgrade_inventory::Inventory, items::ItemStack};
use horizon_world::nbt::{Compound, Tag};
use std::{collections::HashSet, sync::OnceLock};

/// `AE2wtlibItems.MAGNET_CARD`.
pub const MAGNET_CARD: &str = "ae2wtlib:magnet_card";
/// `AE2wtlibAdditionalComponents.MAGNET_SETTINGS`: the mode's byte.
pub const MAGNET_SETTINGS: &str = "ae2wtlib:magnet_settings";
/// `AE2wtlibComponents`: the filters (`ConfigInventory.writeToChildTag` under
/// the key ""), their modes (true: allow), and the terminal settings.
pub const PICKUP_CONFIG: &str = "ae2wtlib:pickup_config";
pub const INSERT_CONFIG: &str = "ae2wtlib:insert_config";
pub const PICKUP_MODE: &str = "ae2wtlib:pickup_mode";
pub const INSERT_MODE: &str = "ae2wtlib:insert_mode";
pub const RESTOCK: &str = "ae2wtlib:restock";
pub const PICK_BLOCK: &str = "ae2wtlib:pick_block";
pub const CRAFT_IF_MISSING: &str = "ae2wtlib:craft_if_missing";
/// `MagnetHost`: 27 filter slots each.
pub const FILTER_SLOTS: usize = 27;

/// `MagnetMode`, by its byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MagnetMode {
    /// Not an upgradeable item.
    Invalid,
    /// No Magnet Card in it.
    NoCard,
    Off,
    PickupInventory,
    PickupMe,
    PickupMeNoMagnet,
}
impl MagnetMode {
    pub fn id(self) -> i8 {
        match self {
            Self::Invalid => -2,
            Self::NoCard => -1,
            Self::Off => 0,
            Self::PickupInventory => 0b01,
            Self::PickupMe => 0b11,
            Self::PickupMeNoMagnet => 0b10,
        }
    }
    /// `fromByte`: anything else is off.
    pub fn from_byte(b: i8) -> Self {
        match b {
            1 => Self::PickupInventory,
            2 => Self::PickupMeNoMagnet,
            3 => Self::PickupMe,
            _ => Self::Off,
        }
    }
    /// Pulls items in.
    pub fn magnet(self) -> bool {
        self.id() > 0 && self.id() & 0b01 != 0
    }
    /// Picked-up items go into the network.
    pub fn pickup_to_me(self) -> bool {
        self.id() > 0 && self.id() & 0b10 != 0
    }
    /// `set`: the two bits; a card-less or invalid mode stays.
    pub fn set(self, magnet: bool, pickup_to_me: bool) -> Self {
        if self.id() < 0 {
            return self;
        }
        Self::from_byte(i8::from(magnet) + 2 * i8::from(pickup_to_me))
    }
    /// `MagnetHotkeyAction`: the next mode and what the action bar says
    /// (`gui.ae2wtlib.magnetcard.hotkey` of the mode, green, or red for off);
    /// none for a card-less or invalid one, which stays.
    pub fn hotkey_next(self) -> Option<(Self, &'static str, bool)> {
        Some(match self {
            Self::Off => (Self::PickupInventory, "gui.ae2wtlib.magnetcard.desc.inv", true),
            Self::PickupInventory => (Self::PickupMe, "gui.ae2wtlib.magnetcard.desc.me", true),
            Self::PickupMe => (Self::PickupMeNoMagnet, "gui.ae2wtlib.magnetcard.desc.me_no_magnet", true),
            Self::PickupMeNoMagnet => (Self::Off, "gui.ae2wtlib.magnetcard.desc.off", false),
            Self::Invalid | Self::NoCard => return None,
        })
    }
}

fn flag(stack: &ItemStack, id: &str) -> Option<bool> {
    match stack.data()?.other(id)? {
        Tag::Byte(b) => Some(*b != 0),
        _ => None,
    }
}

/// `upgrades.isInstalled(MAGNET_CARD)`; None when the item has no upgrade
/// inventory (`IUpgradeableItem`).
fn card_installed(stack: &ItemStack) -> Option<bool> {
    crate::ae2_upgrade_inventory::capacity_of(stack)?;
    let cards = Inventory::read(stack).ok()?;
    Some(cards.slots.iter().flatten().any(|card| card.item.as_ref() == MAGNET_CARD))
}

/// The component's mode, off when absent (`getOrDefault(MAGNET_SETTINGS, OFF)`).
pub fn stored_mode(stack: &ItemStack) -> MagnetMode {
    match stack.data().and_then(|d| d.other(MAGNET_SETTINGS)) {
        Some(Tag::Byte(b)) => MagnetMode::from_byte(*b),
        _ => MagnetMode::Off,
    }
}

/// `MagnetHandler.getMagnetMode`.
pub fn mode(stack: &ItemStack) -> MagnetMode {
    match card_installed(stack) {
        None => MagnetMode::Invalid,
        Some(false) => MagnetMode::NoCard,
        Some(true) => stored_mode(stack),
    }
}

/// `MagnetHandler.saveMagnetMode`: only a mode, on an item with the card.
pub fn save_mode(stack: &mut ItemStack, mode: MagnetMode) -> bool {
    if mode.id() < 0 || card_installed(stack) != Some(true) {
        return false;
    }
    stack.data_mut().set_other(MAGNET_SETTINGS, Some(Tag::Byte(mode.id())));
    crate::ae2_stack_identity::synchronize_tracked(stack);
    true
}

/// `TerminalSettingsPacket.processPacketData`: the three settings, and the
/// magnet's two bits on the stored mode (card or not).
pub fn apply_settings(stack: &mut ItemStack, pick_block: bool, restock: bool, magnet: bool, pickup_to_me: bool, craft_if_missing: bool) {
    let next = stored_mode(stack).set(magnet, pickup_to_me);
    let data = stack.data_mut();
    data.set_other(PICK_BLOCK, Some(Tag::Byte(i8::from(pick_block))));
    data.set_other(CRAFT_IF_MISSING, Some(Tag::Byte(i8::from(craft_if_missing))));
    data.set_other(RESTOCK, Some(Tag::Byte(i8::from(restock))));
    data.set_other(MAGNET_SETTINGS, Some(Tag::Byte(next.id())));
    crate::ae2_stack_identity::synchronize_tracked(stack);
}

/// `RestockHotkeyAction`: restock set on or off.
pub fn set_restock(stack: &mut ItemStack, on: bool) {
    stack.data_mut().set_other(RESTOCK, Some(Tag::Byte(i8::from(on))));
    crate::ae2_stack_identity::synchronize_tracked(stack);
}

/// What `WirelessTerminalSettingsScreen` shows: pick block, craft if
/// missing, restock, and the mode as `getMagnetMode` gives it.
pub fn settings(stack: &ItemStack) -> (bool, bool, bool, MagnetMode) {
    let get = |id| flag(stack, id).unwrap_or(false);
    (get(PICK_BLOCK), get(CRAFT_IF_MISSING), get(RESTOCK), mode(stack))
}

/// `MagnetHost`'s pickup or insert filter: `IPartitionList` with
/// `FuzzyMode.IGNORE_ALL` over its 27 item keys (any components of a listed
/// item match), and its include/exclude mode (exclude when absent).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    pub items: HashSet<String>,
    pub allow: bool,
}
impl Filter {
    /// `ConfigInventory.readFromChildTag(tag, "")` of the item's component:
    /// the item keys of its first 27 entries (`supportedTypes(items)`).
    pub fn read(stack: &ItemStack, config: &str, mode: &str) -> Self {
        let mut items = HashSet::new();
        if let Some(Tag::Compound(data)) = stack.data().and_then(|d| d.other(config))
            && let Some(Tag::List(entries)) = data.get(&"".into())
        {
            for entry in entries.iter().take(FILTER_SLOTS) {
                if let Ok(Some(key)) = crate::ae2_generic_stack::decode_tolerant(entry)
                    && key.key_type == crate::ae2_key_types::KeyType::ITEMS
                {
                    items.insert(key.key.item_id);
                }
            }
        }
        Self { items, allow: flag(stack, mode).unwrap_or(false) }
    }
    pub fn pickup(stack: &ItemStack) -> Self {
        Self::read(stack, PICKUP_CONFIG, PICKUP_MODE)
    }
    pub fn insert(stack: &ItemStack) -> Self {
        Self::read(stack, INSERT_CONFIG, INSERT_MODE)
    }
    /// An empty allow list matches anything (`IPartitionList`), so the
    /// magnet treats it as allowing nothing.
    pub fn allows_nothing(&self) -> bool {
        self.allow && self.items.is_empty()
    }
    /// `matchesFilter(key, mode)`.
    pub fn matches(&self, item: &str) -> bool {
        self.items.contains(item) == self.allow
    }
}

/// The 27 slots of a filter component (`ConfigInventory.readFromChildTag`
/// under ""): each `GenericStack` tag, an empty compound for an empty slot.
pub fn filter_slots(stack: &ItemStack, config: &str) -> Vec<Tag> {
    let mut slots = vec![Tag::Compound(Compound::new()); FILTER_SLOTS];
    if let Some(Tag::Compound(data)) = stack.data().and_then(|d| d.other(config))
        && let Some(Tag::List(entries)) = data.get(&"".into())
    {
        for (slot, entry) in slots.iter_mut().zip(entries) {
            if matches!(crate::ae2_generic_stack::decode_tolerant(entry), Ok(Some(key)) if key.key_type == crate::ae2_key_types::KeyType::ITEMS) {
                *slot = entry.clone();
            }
        }
    }
    slots
}

/// `MagnetHost.updatePickupFilter`/`updateInsertFilter`: the slots written
/// into the component's tag (`writeToChildTag`: trailing empty slots left
/// out, the key removed when all are empty).
pub fn store_filter(stack: &mut ItemStack, config: &str, slots: &[Tag]) {
    let mut data = match stack.data().and_then(|d| d.other(config)) {
        Some(Tag::Compound(data)) => data.clone(),
        _ => Compound::new(),
    };
    let mut list: Vec<Tag> = slots.iter().take(FILTER_SLOTS).cloned().collect();
    while matches!(list.last(), Some(Tag::Compound(c)) if c.is_empty()) {
        list.pop();
    }
    if list.is_empty() {
        data.remove(&"".into());
    } else {
        data.insert("".into(), Tag::List(list));
    }
    stack.data_mut().set_other(config, Some(Tag::Compound(data)));
    crate::ae2_stack_identity::synchronize_tracked(stack);
}

/// A filter slot pressed (`FakeSlot`): the item on the cursor, as a key of
/// no amount (`ConfigInventory.configTypes`), or nothing with an empty cursor.
pub fn set_filter_slot(stack: &mut ItemStack, config: &str, index: usize, carried: Option<&ItemStack>) -> Result<(), String> {
    if index >= FILTER_SLOTS {
        return Err("no such filter slot".into());
    }
    let mut slots = filter_slots(stack, config);
    slots[index] = match carried.filter(|c| c.count > 0) {
        Some(item) => crate::ae2_generic_stack::encode(&crate::ae2_generic_stack::GenericStack::item(crate::ae2_cell_codec::key(item)?, 0))?,
        None => Tag::Compound(Compound::new()),
    };
    store_filter(stack, config, &slots);
    Ok(())
}

/// `togglePickupMode`/`toggleInsertMode`.
pub fn toggle_filter_mode(stack: &mut ItemStack, mode: &str) {
    let allow = flag(stack, mode).unwrap_or(false);
    stack.data_mut().set_other(mode, Some(Tag::Byte(i8::from(!allow))));
    crate::ae2_stack_identity::synchronize_tracked(stack);
}

/// `copyUp` (the insert filter into the pickup filter), `copyDown` (the
/// other way) and `switchInsertPickup`.
pub fn copy_filter(stack: &mut ItemStack, from: &str, to: &str) {
    let slots = filter_slots(stack, from);
    store_filter(stack, to, &slots);
}
pub fn switch_filters(stack: &mut ItemStack) {
    let (pickup, insert) = (filter_slots(stack, PICKUP_CONFIG), filter_slots(stack, INSERT_CONFIG));
    store_filter(stack, PICKUP_CONFIG, &insert);
    store_filter(stack, INSERT_CONFIG, &pickup);
}

/// What the filter screen shows of a filter: each slot's item, if any.
pub fn filter_items(stack: &ItemStack, config: &str) -> Vec<Option<String>> {
    filter_slots(stack, config)
        .iter()
        .map(|tag| crate::ae2_generic_stack::decode_tolerant(tag).ok().flatten().map(|key| key.key.item_id))
        .collect()
}

/// `MagnetHandler.handleMagnet`'s box: the player's grown by the range.
pub fn range() -> f64 {
    Config::get().magnet_card_range
}

/// `ae2wtlib.toml` (`AE2wtlibConfig`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    /// `magnet_card_range`, 16.0.
    pub magnet_card_range: f64,
}
impl Default for Config {
    fn default() -> Self {
        Self { magnet_card_range: 16.0 }
    }
}
impl Config {
    /// A value that is not a decimal falls back to the default, as
    /// `ModConfigSpec.define(path, 16.0)` corrects it.
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut config = Self::default();
        if let Some(range) = value.get("magnet_card_range").and_then(toml::Value::as_float) {
            config.magnet_card_range = range;
        }
        Ok(config)
    }
    /// The game's `config/ae2wtlib.toml`, read once. All the Mods 10 ships
    /// none, so the range is 16 there.
    pub fn get() -> Self {
        static CONFIG: OnceLock<Config> = OnceLock::new();
        *CONFIG.get_or_init(|| {
            let path = crate::logging::game_home().join("config/ae2wtlib.toml");
            match std::fs::read_to_string(&path) {
                Ok(text) => Self::parse(&text).unwrap_or_else(|error| {
                    log::warn!("invalid AE2WTLib config {}: {error}", path.display());
                    Self::default()
                }),
                Err(_) => Self::default(),
            }
        })
    }
}

/// A filter's component as `writeToChildTag` writes it: the keys under "",
/// trailing empty slots left out. For tests and commands.
pub fn filter_component(items: &[&str]) -> Tag {
    let entries = items
        .iter()
        .map(|id| Tag::compound([("#t", Tag::String("ae2:i".into())), ("id", Tag::String((*id).into())), ("#", Tag::Long(0))]))
        .collect();
    let mut data = Compound::new();
    data.insert("".into(), Tag::List(entries));
    Tag::Compound(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terminal(card: bool) -> ItemStack {
        let mut stack = ItemStack::new("ae2:wireless_crafting_terminal", 1);
        if card {
            let mut cards = Inventory::read(&stack).unwrap();
            assert!(cards.insert_one(&ItemStack::new(MAGNET_CARD, 1)));
            assert!(!cards.insert_one(&ItemStack::new(MAGNET_CARD, 1)), "one card on a terminal");
            cards.store(&mut stack).unwrap();
        }
        stack
    }

    #[test]
    fn modes_follow_the_card_and_the_byte() {
        assert_eq!(mode(&ItemStack::new("minecraft:stick", 1)), MagnetMode::Invalid);
        let mut bare = terminal(false);
        assert_eq!(mode(&bare), MagnetMode::NoCard);
        assert!(!save_mode(&mut bare, MagnetMode::PickupMe), "no card: not saved");
        let mut stack = terminal(true);
        assert_eq!(mode(&stack), MagnetMode::Off);
        // The hotkey's cycle, back to off.
        let mut seen = Vec::new();
        for _ in 0..4 {
            let (next, _, _) = mode(&stack).hotkey_next().unwrap();
            assert!(save_mode(&mut stack, next));
            seen.push(mode(&stack));
        }
        assert_eq!(seen, [MagnetMode::PickupInventory, MagnetMode::PickupMe, MagnetMode::PickupMeNoMagnet, MagnetMode::Off]);
        assert!(MagnetMode::PickupMe.magnet() && MagnetMode::PickupMe.pickup_to_me());
        assert!(!MagnetMode::PickupMeNoMagnet.magnet() && MagnetMode::PickupMeNoMagnet.pickup_to_me());
        assert!(MagnetMode::PickupInventory.magnet() && !MagnetMode::PickupInventory.pickup_to_me());
        assert!(MagnetMode::NoCard.hotkey_next().is_none());
        assert_eq!(MagnetMode::NoCard.set(true, true), MagnetMode::NoCard);
        assert_eq!(MagnetMode::from_byte(7), MagnetMode::Off);
    }

    /// The settings screen writes the magnet's bits even without a card,
    /// which `getMagnetMode` then hides until one goes in.
    #[test]
    fn settings_set_the_bits_on_the_stored_mode() {
        let mut bare = terminal(false);
        apply_settings(&mut bare, true, false, true, true, false);
        assert_eq!(stored_mode(&bare), MagnetMode::PickupMe);
        assert_eq!(settings(&bare), (true, false, false, MagnetMode::NoCard));
        let mut stack = terminal(true);
        apply_settings(&mut stack, false, true, false, true, true);
        assert_eq!(settings(&stack), (false, true, true, MagnetMode::PickupMeNoMagnet));
    }

    #[test]
    fn filters_list_items_by_id_and_default_to_deny() {
        let mut stack = terminal(true);
        let pickup = Filter::pickup(&stack);
        assert!(!pickup.allow && pickup.items.is_empty());
        assert!(pickup.matches("minecraft:stone"), "an empty deny list takes anything");
        stack.data_mut().set_other(PICKUP_CONFIG, Some(filter_component(&["minecraft:cobblestone", "minecraft:dirt"])));
        stack.data_mut().set_other(PICKUP_MODE, Some(Tag::Byte(1)));
        let pickup = Filter::pickup(&stack);
        assert!(pickup.allow && pickup.matches("minecraft:dirt") && !pickup.matches("minecraft:stone"));
        stack.data_mut().set_other(INSERT_CONFIG, Some(filter_component(&["minecraft:dirt"])));
        let insert = Filter::insert(&stack);
        assert!(!insert.allow && !insert.matches("minecraft:dirt") && insert.matches("minecraft:cobblestone"));
        stack.data_mut().set_other(INSERT_MODE, Some(Tag::Byte(1)));
        stack.data_mut().set_other(INSERT_CONFIG, Some(Tag::Compound(Compound::new())));
        assert!(Filter::insert(&stack).allows_nothing());
    }

    #[test]
    fn filter_screen_edits_the_components_as_the_menu_does() {
        let mut stack = terminal(true);
        set_filter_slot(&mut stack, PICKUP_CONFIG, 3, Some(&ItemStack::new("minecraft:dirt", 12))).unwrap();
        let Some(Tag::Compound(data)) = stack.data().unwrap().other(PICKUP_CONFIG) else { panic!() };
        let Some(Tag::List(list)) = data.get(&"".into()) else { panic!() };
        assert_eq!(list.len(), 4, "trailing empty slots left out");
        assert_eq!(list[3].get("#"), Some(&Tag::Long(0)), "a key of no amount");
        assert_eq!(filter_items(&stack, PICKUP_CONFIG)[3].as_deref(), Some("minecraft:dirt"));
        assert!(Filter::pickup(&stack).items.contains("minecraft:dirt"));
        copy_filter(&mut stack, PICKUP_CONFIG, INSERT_CONFIG);
        assert_eq!(filter_items(&stack, INSERT_CONFIG)[3].as_deref(), Some("minecraft:dirt"));
        set_filter_slot(&mut stack, PICKUP_CONFIG, 3, None).unwrap();
        let Some(Tag::Compound(data)) = stack.data().unwrap().other(PICKUP_CONFIG) else { panic!() };
        assert!(data.is_empty(), "all empty: the key goes");
        switch_filters(&mut stack);
        assert_eq!(filter_items(&stack, PICKUP_CONFIG)[3].as_deref(), Some("minecraft:dirt"));
        assert!(filter_items(&stack, INSERT_CONFIG).iter().all(Option::is_none));
        toggle_filter_mode(&mut stack, INSERT_MODE);
        assert!(Filter::insert(&stack).allow);
        toggle_filter_mode(&mut stack, INSERT_MODE);
        assert!(!Filter::insert(&stack).allow);
        assert!(set_filter_slot(&mut stack, INSERT_CONFIG, 27, None).is_err());
    }

    #[test]
    fn config_range() {
        assert_eq!(Config::parse("").unwrap().magnet_card_range, 16.0);
        assert_eq!(Config::parse("magnet_card_range = 8.5").unwrap().magnet_card_range, 8.5);
        assert_eq!(Config::parse("magnet_card_range = 8").unwrap().magnet_card_range, 16.0, "not a decimal: corrected");
    }
}
