//! AE2 Import Export Card 1.21.1-1.9.1's rows in the definitions registry,
//! the data its cards keep and its server config (feature
//! `ae2-importexportcard`). Derived from AE2 Import Export Card by
//! Ultramega (`AE2ImportExportCard`, `ModItems`, `ModDataComponents`,
//! `ServerConfig`, `UpgradeHost`, `AEKeyFilterUtil`, `BlockPickerCardConfig`,
//! `CardConfigManager`), https://github.com/starforcraft/AE2-Import-Export-Card
//! branch `1.21.1` (1.9.1), MIT; see `ae2/addons/ae2importexportcard/`. The
//! player's side (the cards' tick, their screens, the Block Picker) is the
//! game's, in `game/ae2x/importexportcard/`.
use super::defs::Defs;
use crate::{ae2_generic_stack::GenericStack, ae2_upgrade_inventory::Inventory, items::ItemStack, save};
use ae2_cell_core::FuzzyMode;
use horizon_world::nbt::{Compound, Tag};
use std::sync::OnceLock;

pub const NAMESPACE: &str = "ae2importexportcard";
/// `ModItems`: three upgrade cards (`Upgrades.createUpgradeCardItem`, one a stack).
pub const IMPORT_CARD: &str = "ae2importexportcard:import_card";
pub const EXPORT_CARD: &str = "ae2importexportcard:export_card";
pub const BLOCK_PICKER_CARD: &str = "ae2importexportcard:block_picker_card";
/// `ModDataComponents`: a card's filter (`ConfigInventory.writeToChildTag`
/// under ""), the inventory slots it works on (an int list: 0 off, else the
/// filter slot counted from 1), the Curios slots it fills and the Block
/// Picker's amount.
pub const FILTER_CONFIG: &str = "ae2importexportcard:filter_config";
pub const SELECTED_INVENTORY_SLOTS: &str = "ae2importexportcard:selected_inventory_slots";
pub const SELECTED_CURIO_SLOTS: &str = "ae2importexportcard:selected_curio_slots";
pub const BLOCK_PICKER_AMOUNT: &str = "ae2importexportcard:block_picker_amount";
/// AE2's components on a card: its own cards and its fuzzy mode
/// (`CardConfigManager`, `IConfigManager.exportSettings`).
pub const UPGRADES: &str = "ae2:upgrades";
pub const EXPORTED_SETTINGS: &str = "ae2:exported_settings";

/// `UpgradeHost`: the main inventory and the armour (`Inventory.getItem`
/// 36..39, feet first), 18 filter slots and 9 more for each Capacity Card
/// (up to 3), 4 card slots on the Import Card and 5 on the Export Card.
pub const SELECTED_INVENTORY_SLOT_COUNT: usize = 40;
pub const BASE_FILTER_SLOT_COUNT: usize = 18;
pub const FILTER_SLOTS_PER_CAPACITY_CARD: usize = 9;
pub const MAX_CAPACITY_CARD_COUNT: usize = 3;
pub const MAX_FILTER_SLOT_COUNT: usize = BASE_FILTER_SLOT_COUNT + MAX_CAPACITY_CARD_COUNT * FILTER_SLOTS_PER_CAPACITY_CARD;
/// `BlockPickerCardConfig`.
pub const BLOCK_PICKER_DEFAULT_AMOUNT: i32 = 1;
pub const BLOCK_PICKER_MAX_AMOUNT: i32 = 64;

/// `UpgradeType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CardType {
    Import,
    Export,
}
impl CardType {
    /// `UpgradeType.getId`.
    pub fn id(self) -> u8 {
        match self {
            Self::Import => 0,
            Self::Export => 1,
        }
    }
    pub fn from_id(id: u8) -> Option<Self> {
        match id {
            0 => Some(Self::Import),
            1 => Some(Self::Export),
            _ => None,
        }
    }
    /// `isImportOrExportCard`: the card of this type.
    pub fn card(self) -> &'static str {
        match self {
            Self::Import => IMPORT_CARD,
            Self::Export => EXPORT_CARD,
        }
    }
    /// `UpgradeHost.getUpgradeSlotCount`.
    pub fn upgrade_slots(self) -> usize {
        match self {
            Self::Import => 4,
            Self::Export => 5,
        }
    }
    pub fn of(item: &str) -> Option<Self> {
        match item {
            IMPORT_CARD => Some(Self::Import),
            EXPORT_CARD => Some(Self::Export),
            _ => None,
        }
    }
}

/// `commonSetup`: each card on AE2's two wireless terminals, and with
/// AE2WTLib on the universal terminal and the wireless pattern encoding
/// terminal, one each (`Upgrades.add`, not AE2WTLib's
/// `addUpgradeToAllTerminals`: the pattern access terminal and the other
/// addons' terminals take none). On the cards: three Capacity Cards, a Fuzzy
/// Card, and an Inverter Card on the Import Card, a Crafting Card and an
/// Acceleration Card on the Export Card. The cards are no `IUpgradeableItem`
/// (AE2's `add_item_upgrade` does not take them): their own cards go in by
/// their screens alone, so they have no upgrade slots in the registry.
pub fn register(defs: &mut Defs) {
    for card in [IMPORT_CARD, EXPORT_CARD, BLOCK_PICKER_CARD] {
        for terminal in ["ae2:wireless_terminal", "ae2:wireless_crafting_terminal"] {
            defs.add_upgrade(card, terminal, 1);
        }
        #[cfg(feature = "ae2-ae2wtlib")]
        for terminal in [super::ae2wtlib::UNIVERSAL_TERMINAL, super::ae2wtlib::PATTERN_ENCODING_TERMINAL] {
            defs.add_upgrade(card, terminal, 1);
        }
    }
    for card in [IMPORT_CARD, EXPORT_CARD] {
        defs.add_upgrade("ae2:capacity_card", card, MAX_CAPACITY_CARD_COUNT);
        defs.add_upgrade("ae2:fuzzy_card", card, 1);
    }
    defs.add_upgrade("ae2:inverter_card", IMPORT_CARD, 1);
    defs.add_upgrade("ae2:crafting_card", EXPORT_CARD, 1);
    defs.add_upgrade("ae2:speed_card", EXPORT_CARD, 1);
}

/// `ServerConfig` (`ae2importexportcard-server.toml`).
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// `allowEnergyExport`, true: Applied Flux's FE (`FluxKey`), which the
    /// port has no key type for yet.
    pub allow_energy_export: bool,
    /// `blockedItems` and `blockedFluids`: registry ids no card exports.
    pub blocked_items: Vec<String>,
    pub blocked_fluids: Vec<String>,
}
impl Default for Config {
    fn default() -> Self {
        Self { allow_energy_export: true, blocked_items: Vec::new(), blocked_fluids: Vec::new() }
    }
}
/// `ServerConfig.isIdentifier`: a string with a `:` that is a resource location.
fn is_identifier(id: &str) -> bool {
    let Some((namespace, path)) = id.split_once(':') else { return false };
    let ns_ok = namespace.bytes().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.'));
    let path_ok = path.bytes().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.' | b'/'));
    ns_ok && path_ok
}
impl Config {
    /// A value of the wrong type falls back to its default; a list keeps
    /// its identifiers (`defineListAllowEmpty`, its element validator).
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut config = Self::default();
        if let Some(b) = value.get("allowEnergyExport").and_then(toml::Value::as_bool) {
            config.allow_energy_export = b;
        }
        let list = |key: &str| -> Option<Vec<String>> {
            let entries = value.get(key)?.as_array()?;
            Some(entries.iter().filter_map(toml::Value::as_str).filter(|id| is_identifier(id)).map(str::to_owned).collect())
        };
        if let Some(items) = list("blockedItems") {
            config.blocked_items = items;
        }
        if let Some(fluids) = list("blockedFluids") {
            config.blocked_fluids = fluids;
        }
        Ok(config)
    }
    /// The game's `config/ae2importexportcard-server.toml`, read once. All
    /// the Mods 10 ships none (it is a server config, written per world with
    /// the defaults), so energy export is on and nothing is blocked there.
    pub fn get() -> &'static Self {
        static CONFIG: OnceLock<Config> = OnceLock::new();
        CONFIG.get_or_init(|| {
            let path = crate::logging::game_home().join("config/ae2importexportcard-server.toml");
            match std::fs::read_to_string(&path) {
                Ok(text) => Self::parse(&text).unwrap_or_else(|error| {
                    log::warn!("invalid AE2 Import Export Card config {}: {error}", path.display());
                    Self::default()
                }),
                Err(_) => Self::default(),
            }
        })
    }
    /// `ServerConfig.allowsExport` of an item or fluid key.
    pub fn allows_export(&self, id: &str, fluid: bool) -> bool {
        let blocked = if fluid { &self.blocked_fluids } else { &self.blocked_items };
        !blocked.iter().any(|b| b == id)
    }
}

/// The int list of `selected_inventory_slots` (an int array, or a list of
/// ints), as `normalizeSelectedInventorySlots` gives it: 40 entries, the
/// first ones of a list of another length, zeros after.
pub fn selected_slots(card: &ItemStack) -> [i32; SELECTED_INVENTORY_SLOT_COUNT] {
    let mut slots = [0; SELECTED_INVENTORY_SLOT_COUNT];
    let values: Vec<i32> = match card.data().and_then(|d| d.other(SELECTED_INVENTORY_SLOTS)) {
        Some(Tag::IntArray(values)) => values.clone(),
        Some(Tag::List(values)) => values.iter().map(|v| if let Tag::Int(n) = v { *n } else { 0 }).collect(),
        _ => Vec::new(),
    };
    for (slot, value) in slots.iter_mut().zip(values) {
        *slot = value;
    }
    slots
}
/// `setSelectedInventorySlots`.
pub fn set_selected_slots(card: &mut ItemStack, slots: &[i32]) {
    let mut normalized = [0; SELECTED_INVENTORY_SLOT_COUNT];
    for (slot, value) in normalized.iter_mut().zip(slots) {
        *slot = *value;
    }
    card.data_mut().set_other(SELECTED_INVENTORY_SLOTS, Some(Tag::IntArray(normalized.to_vec())));
}

/// `UpgradeInventories.forItem(card, n)`: the card's own cards, `n` slots;
/// an entry past them or unreadable is left out.
pub fn card_upgrades(card: &ItemStack, ty: CardType) -> Vec<Option<ItemStack>> {
    let mut slots = vec![None; ty.upgrade_slots()];
    if let Some(Tag::List(entries)) = card.data().and_then(|d| d.other(UPGRADES)) {
        for entry in entries {
            let Some(Tag::Int(index)) = entry.get("slot") else { continue };
            let Some(slot) = usize::try_from(*index).ok().and_then(|i| slots.get_mut(i)) else { continue };
            if let Some(item) = entry.get("item").and_then(save::item_from_nbt) {
                *slot = Some(ItemStack { count: 1, ..item });
            }
        }
    }
    slots
}
/// `onUpgradesChanged`: the cards back into the card (`ItemContainerContents`).
pub fn store_card_upgrades(card: &mut ItemStack, slots: &[Option<ItemStack>]) -> Result<(), String> {
    let list = Tag::List(crate::ae2_upgrade_inventory::encode_source(slots)?);
    let list = crate::ae2_components::normalize(UPGRADES, list)?;
    let empty = matches!(&list, Tag::List(l) if l.is_empty());
    card.data_mut().set_other(UPGRADES, (!empty).then_some(list));
    Ok(())
}
/// `IUpgradeInventory.getInstalledUpgrades(card)`.
pub fn installed(upgrades: &[Option<ItemStack>], card: &str) -> usize {
    upgrades.iter().flatten().filter(|s| s.item.as_ref() == card).count()
}
/// `UpgradeHost.getFilterSlotCount`: 18, and 9 for each Capacity Card up to 3.
pub fn filter_slot_count(upgrades: &[Option<ItemStack>]) -> usize {
    BASE_FILTER_SLOT_COUNT + installed(upgrades, "ae2:capacity_card").min(MAX_CAPACITY_CARD_COUNT) * FILTER_SLOTS_PER_CAPACITY_CARD
}
/// Whether a card of `card` fits the card's slot (`Upgrades.add` rows).
pub fn card_accepts(ty: CardType, upgrades: &[Option<ItemStack>], card: &ItemStack) -> bool {
    let limit = super::defs::get().upgrade_limit(&card.item, ty.card());
    limit > installed(upgrades, &card.item)
}

/// `ConfigInventory.readFromChildTag(filter_config, "")` of `size` slots:
/// each a key (an item or a fluid), or none.
pub fn filter(card: &ItemStack, size: usize) -> Vec<Option<GenericStack>> {
    let mut slots = vec![None; size];
    if let Some(Tag::Compound(data)) = card.data().and_then(|d| d.other(FILTER_CONFIG))
        && let Some(Tag::List(entries)) = data.get(&"".into())
    {
        for (slot, entry) in slots.iter_mut().zip(entries) {
            *slot = crate::ae2_generic_stack::decode_tolerant(entry).ok().flatten();
        }
    }
    slots
}
/// `UpgradeHost.updateFilter`: the 45 slots written back (`writeToChildTag`:
/// trailing empty slots left out, the key removed when all are).
pub fn store_filter(card: &mut ItemStack, slots: &[Option<GenericStack>]) -> Result<(), String> {
    let mut data = match card.data().and_then(|d| d.other(FILTER_CONFIG)) {
        Some(Tag::Compound(data)) => data.clone(),
        _ => Compound::new(),
    };
    let mut list = Vec::new();
    for slot in slots.iter().take(MAX_FILTER_SLOT_COUNT) {
        list.push(match slot {
            Some(stack) => crate::ae2_generic_stack::encode(&GenericStack { amount: 0, ..stack.clone() })?,
            None => Tag::Compound(Compound::new()),
        });
    }
    while matches!(list.last(), Some(Tag::Compound(c)) if c.is_empty()) {
        list.pop();
    }
    if list.is_empty() {
        data.remove(&"".into());
    } else {
        data.insert("".into(), Tag::List(list));
    }
    card.data_mut().set_other(FILTER_CONFIG, Some(Tag::Compound(data)));
    Ok(())
}

/// The setting's names (`FuzzyMode`), as `exportSettings` writes them.
const FUZZY_MODES: [(FuzzyMode, &str); 5] = [
    (FuzzyMode::IgnoreAll, "IGNORE_ALL"),
    (FuzzyMode::Percent99, "PERCENT_99"),
    (FuzzyMode::Percent75, "PERCENT_75"),
    (FuzzyMode::Percent50, "PERCENT_50"),
    (FuzzyMode::Percent25, "PERCENT_25"),
];
pub fn fuzzy_name(mode: FuzzyMode) -> &'static str {
    FUZZY_MODES.iter().find(|(m, _)| *m == mode).map_or("IGNORE_ALL", |(_, n)| n)
}
/// The `percent_99` style `fuzzy_key_match` takes.
pub fn fuzzy_lower(mode: FuzzyMode) -> &'static str {
    match mode {
        FuzzyMode::IgnoreAll => "ignore_all",
        FuzzyMode::Percent99 => "percent_99",
        FuzzyMode::Percent75 => "percent_75",
        FuzzyMode::Percent50 => "percent_50",
        FuzzyMode::Percent25 => "percent_25",
    }
}
/// `IConfigManager.builder(card).registerSetting(FUZZY_MODE, IGNORE_ALL)`:
/// the card's `fuzzy_mode`, IGNORE_ALL unless set.
pub fn fuzzy_mode(card: &ItemStack) -> FuzzyMode {
    let Some(Tag::Compound(settings)) = card.data().and_then(|d| d.other(EXPORTED_SETTINGS)) else { return FuzzyMode::IgnoreAll };
    let Some(Tag::String(name)) = settings.get(&"fuzzy_mode".into()) else { return FuzzyMode::IgnoreAll };
    let name = name.to_string_lossy();
    FUZZY_MODES.iter().find(|(_, n)| *n == name).map_or(FuzzyMode::IgnoreAll, |(m, _)| *m)
}
/// `ServerSettingToggleButton` on the card's `FUZZY_MODE`: the next mode
/// (the one before, backwards), exported into the card.
pub fn cycle_fuzzy_mode(card: &mut ItemStack, backwards: bool) -> FuzzyMode {
    let at = FUZZY_MODES.iter().position(|(m, _)| *m == fuzzy_mode(card)).unwrap_or(0);
    let next = if backwards { (at + FUZZY_MODES.len() - 1) % FUZZY_MODES.len() } else { (at + 1) % FUZZY_MODES.len() };
    let mode = FUZZY_MODES[next].0;
    let mut settings = match card.data().and_then(|d| d.other(EXPORTED_SETTINGS)) {
        Some(Tag::Compound(settings)) => settings.clone(),
        _ => Compound::new(),
    };
    settings.insert("fuzzy_mode".into(), Tag::String(fuzzy_name(mode).into()));
    card.data_mut().set_other(EXPORTED_SETTINGS, Some(Tag::Compound(settings)));
    mode
}

/// `AEKeyFilterUtil.passesFilter`: the filter's keys that `findFuzzy` finds
/// for `what` (its primary key's), matched exactly, or with the Fuzzy Card
/// by `fuzzyEquals`; the Inverter Card turns the answer. An empty filter
/// matches nothing, so without the inverter nothing passes.
pub fn passes_filter(what: &GenericStack, filter: &[Option<GenericStack>], fuzzy: bool, invert: bool, fuzzy_equals: impl Fn(&GenericStack, &GenericStack) -> bool) -> bool {
    let matches = filter.iter().flatten().any(|entry| {
        entry.fluid == what.fluid
            && entry.key.item_id == what.key.item_id
            && if fuzzy { fuzzy_equals(what, entry) } else { entry.key == what.key }
    });
    invert != matches
}

/// The cards in a terminal's slots (`AEComponents.UPGRADES`), by slot.
pub fn cards(terminal: &ItemStack) -> Vec<(usize, ItemStack)> {
    let Ok(inventory) = Inventory::read(terminal) else { return Vec::new() };
    inventory.slots.into_iter().enumerate().filter_map(|(i, s)| s.map(|s| (i, s))).collect()
}
/// The terminal's first card of `card` (`BlockPickerCardConfig.findCard`,
/// `UpgradeHost`'s loop).
pub fn find_card(terminal: &ItemStack, card: &str) -> Option<ItemStack> {
    cards(terminal).into_iter().map(|(_, s)| s).find(|s| s.item.as_ref() == card)
}
/// The terminal's cards of `card` changed by `edit` and written back
/// (`UpgradeHost`'s `itemStack.set(UPGRADES, ...)`); false without one.
pub fn edit_card(terminal: &mut ItemStack, card: &str, edit: impl Fn(&mut ItemStack)) -> bool {
    let Ok(mut inventory) = Inventory::read(terminal) else { return false };
    let mut found = false;
    for slot in inventory.slots.iter_mut().flatten().filter(|s| s.item.as_ref() == card) {
        edit(slot);
        found = true;
    }
    found && inventory.store(terminal).is_ok() && {
        crate::ae2_stack_identity::synchronize_tracked(terminal);
        true
    }
}

/// `BlockPickerCardConfig.getAmount`: the card's amount, 1 to 64 (1 unset).
pub fn block_picker_amount(card: &ItemStack) -> i32 {
    let amount = match card.data().and_then(|d| d.other(BLOCK_PICKER_AMOUNT)) {
        Some(Tag::Int(n)) => *n,
        _ => BLOCK_PICKER_DEFAULT_AMOUNT,
    };
    amount.clamp(BLOCK_PICKER_DEFAULT_AMOUNT, BLOCK_PICKER_MAX_AMOUNT)
}
/// `getAmountFromTerminal`: the terminal's Block Picker Card's, 1 without one.
pub fn terminal_block_picker_amount(terminal: &ItemStack) -> i32 {
    find_card(terminal, BLOCK_PICKER_CARD).map_or(BLOCK_PICKER_DEFAULT_AMOUNT, |card| block_picker_amount(&card))
}
/// `setAmountOnTerminal`: every Block Picker Card of the terminal, clamped.
pub fn set_block_picker_amount(terminal: &mut ItemStack, amount: i32) -> bool {
    let amount = amount.clamp(BLOCK_PICKER_DEFAULT_AMOUNT, BLOCK_PICKER_MAX_AMOUNT);
    edit_card(terminal, BLOCK_PICKER_CARD, |card| card.data_mut().set_other(BLOCK_PICKER_AMOUNT, Some(Tag::Int(amount))))
}

/// `UpgradeScreen.increaseSelectedInventorySlot`: the Import Card checks or
/// unchecks a slot; the Export Card counts it through its active filter
/// slots, then off.
pub fn increase_selected(ty: CardType, value: i32, filter_slots: usize) -> i32 {
    match ty {
        CardType::Export if value >= filter_slots as i32 => 0,
        CardType::Export => value + 1,
        CardType::Import if value == 0 => 1,
        CardType::Import => 0,
    }
}

/// `UpgradeHost.onUpgradesChanged` on the Export Card: a slot pointing past
/// the active filter slots is turned off.
pub fn clamp_selected(slots: &mut [i32], filter_slots: usize) -> bool {
    let mut changed = false;
    for slot in slots.iter_mut().filter(|s| **s > filter_slots as i32) {
        *slot = 0;
        changed = true;
    }
    changed
}

/// The canonical form `item_data::canon_stack` gives a stack of one.
pub fn canon(stack: &ItemStack) -> String {
    crate::item_data::canon_stack(Some(&ItemStack { count: 1, ..stack.clone() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str) -> GenericStack {
        GenericStack { key: crate::ae2_cell_codec::key(&ItemStack::new(id, 1)).unwrap(), fluid: false, amount: 0 }
    }

    #[test]
    fn rows_put_the_cards_on_the_source_terminals_and_their_cards_on_them() {
        let defs = super::super::defs::get();
        for card in [IMPORT_CARD, EXPORT_CARD, BLOCK_PICKER_CARD] {
            assert_eq!(defs.upgrade_limit(card, "ae2:wireless_terminal"), 1, "{card}");
            assert_eq!(defs.upgrade_limit(card, "ae2:wireless_crafting_terminal"), 1, "{card}");
            #[cfg(feature = "ae2-ae2wtlib")]
            {
                assert_eq!(defs.upgrade_limit(card, super::super::ae2wtlib::UNIVERSAL_TERMINAL), 1);
                assert_eq!(defs.upgrade_limit(card, super::super::ae2wtlib::PATTERN_ENCODING_TERMINAL), 1);
                assert_eq!(defs.upgrade_limit(card, super::super::ae2wtlib::PATTERN_ACCESS_TERMINAL), 0, "Upgrades.add, not addUpgradeToAllTerminals");
            }
        }
        assert_eq!(defs.upgrade_limit("ae2:capacity_card", IMPORT_CARD), 3);
        assert_eq!(defs.upgrade_limit("ae2:inverter_card", IMPORT_CARD), 1);
        assert_eq!(defs.upgrade_limit("ae2:inverter_card", EXPORT_CARD), 0);
        assert_eq!(defs.upgrade_limit("ae2:crafting_card", EXPORT_CARD), 1);
        assert_eq!(defs.upgrade_limit("ae2:speed_card", EXPORT_CARD), 1);
        assert_eq!(defs.upgrade_limit("ae2:speed_card", IMPORT_CARD), 0);
        assert_eq!(defs.upgrade_slots(IMPORT_CARD), None, "no IUpgradeableItem: AE2's add_item_upgrade leaves it");
    }

    #[test]
    fn config_defaults_and_corrections() {
        assert_eq!(Config::parse("").unwrap(), Config::default());
        let config = Config::parse("allowEnergyExport = false\nblockedItems = [\"minecraft:diamond\", \"nocolon\", 3]\nblockedFluids = \"x\"\n").unwrap();
        assert!(!config.allow_energy_export);
        assert_eq!(config.blocked_items, ["minecraft:diamond"]);
        assert!(config.blocked_fluids.is_empty());
        assert!(!config.allows_export("minecraft:diamond", false));
        assert!(config.allows_export("minecraft:diamond", true), "the fluids' list is another");
    }

    #[test]
    fn selection_filter_cards_and_settings_roundtrip_in_the_source_form() {
        let mut card = ItemStack::new(EXPORT_CARD, 1);
        assert_eq!(selected_slots(&card), [0; 40]);
        // An old card of 36 entries (before the armour) reads as 40.
        card.data_mut().set_other(SELECTED_INVENTORY_SLOTS, Some(Tag::IntArray(vec![2; 36])));
        assert_eq!(selected_slots(&card)[35], 2);
        assert_eq!(selected_slots(&card)[36], 0);
        set_selected_slots(&mut card, &[1, 0, 3]);
        assert_eq!(card.data().unwrap().other(SELECTED_INVENTORY_SLOTS), Some(&Tag::IntArray({
            let mut v = vec![1, 0, 3];
            v.resize(40, 0);
            v
        })));

        let mut slots = vec![None; MAX_FILTER_SLOT_COUNT];
        slots[2] = Some(item("minecraft:cobblestone"));
        store_filter(&mut card, &slots).unwrap();
        let Some(Tag::Compound(data)) = card.data().unwrap().other(FILTER_CONFIG) else { panic!() };
        let Some(Tag::List(list)) = data.get(&"".into()) else { panic!() };
        assert_eq!(list.len(), 3, "trailing empty slots left out");
        assert_eq!(filter(&card, 18)[2].as_ref().unwrap().key.item_id, "minecraft:cobblestone");
        assert_eq!(filter(&card, 2), [None, None], "a smaller ConfigInventory reads its slots only");
        store_filter(&mut card, &vec![None; MAX_FILTER_SLOT_COUNT]).unwrap();
        assert_eq!(card.data().unwrap().other(FILTER_CONFIG), Some(&Tag::Compound(Compound::new())));

        let mut cards = vec![None; 5];
        cards[1] = Some(ItemStack::new("ae2:capacity_card", 1));
        cards[3] = Some(ItemStack::new("ae2:capacity_card", 1));
        store_card_upgrades(&mut card, &cards).unwrap();
        assert_eq!(card_upgrades(&card, CardType::Export), cards);
        assert_eq!(filter_slot_count(&cards), 36);
        assert!(card_accepts(CardType::Export, &cards, &ItemStack::new("ae2:capacity_card", 1)));
        cards[0] = Some(ItemStack::new("ae2:capacity_card", 1));
        assert!(!card_accepts(CardType::Export, &cards, &ItemStack::new("ae2:capacity_card", 1)), "three at most");
        assert!(!card_accepts(CardType::Export, &cards, &ItemStack::new("ae2:inverter_card", 1)), "the inverter is the Import Card's");
        assert_eq!(card_upgrades(&card, CardType::Import).len(), 4);

        assert_eq!(fuzzy_mode(&card), FuzzyMode::IgnoreAll);
        assert_eq!(cycle_fuzzy_mode(&mut card, false), FuzzyMode::Percent99);
        assert_eq!(fuzzy_mode(&card), FuzzyMode::Percent99);
        assert_eq!(cycle_fuzzy_mode(&mut card, true), FuzzyMode::IgnoreAll);
        assert_eq!(cycle_fuzzy_mode(&mut card, true), FuzzyMode::Percent25, "backwards round the start");
    }

    #[test]
    fn the_filter_passes_exact_keys_and_the_inverter_turns_it() {
        let cobble = item("minecraft:cobblestone");
        let filter = vec![Some(cobble.clone()), None];
        let never = |_: &GenericStack, _: &GenericStack| false;
        assert!(passes_filter(&cobble, &filter, false, false, never));
        assert!(!passes_filter(&item("minecraft:dirt"), &filter, false, false, never));
        assert!(passes_filter(&item("minecraft:dirt"), &filter, false, true, never), "inverted: what the filter lacks");
        assert!(!passes_filter(&cobble, &[], false, false, never), "an empty filter passes nothing");
        assert!(passes_filter(&cobble, &[], false, true, never), "inverted, everything");
        let mut named = cobble.clone();
        named.key.components = vec![1, 2];
        assert!(!passes_filter(&named, &filter, false, false, never), "components count without the Fuzzy Card");
        assert!(passes_filter(&named, &filter, true, false, |_, _| true));
        let fluid = GenericStack { fluid: true, ..cobble.clone() };
        assert!(!passes_filter(&fluid, &filter, false, false, never), "a fluid is not the item");
    }

    #[test]
    fn selections_count_through_the_filters_and_the_block_picker_clamps() {
        assert_eq!(increase_selected(CardType::Import, 0, 18), 1);
        assert_eq!(increase_selected(CardType::Import, 1, 18), 0);
        assert_eq!(increase_selected(CardType::Export, 0, 18), 1);
        assert_eq!(increase_selected(CardType::Export, 17, 18), 18);
        assert_eq!(increase_selected(CardType::Export, 18, 18), 0);
        let mut slots = [0, 20, 18, 27];
        assert!(clamp_selected(&mut slots, 18));
        assert_eq!(slots, [0, 0, 18, 0]);

        let mut terminal = ItemStack::new("ae2:wireless_terminal", 1);
        assert_eq!(terminal_block_picker_amount(&terminal), 1);
        assert!(!set_block_picker_amount(&mut terminal, 9), "no card");
        let mut cards = Inventory::read(&terminal).unwrap();
        assert!(cards.insert_one(&ItemStack::new(BLOCK_PICKER_CARD, 1)));
        cards.store(&mut terminal).unwrap();
        assert!(set_block_picker_amount(&mut terminal, 100));
        assert_eq!(terminal_block_picker_amount(&terminal), 64);
        assert!(set_block_picker_amount(&mut terminal, 0));
        assert_eq!(terminal_block_picker_amount(&terminal), 1);
        assert_eq!(find_card(&terminal, BLOCK_PICKER_CARD).unwrap().data().unwrap().other(BLOCK_PICKER_AMOUNT), Some(&Tag::Int(1)));
    }
}
