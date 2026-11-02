//! Machine upgrade policy and transfer helpers, based on AE2 v19.2.18
//! InitUpgrades.java. Cards remain physical ItemStacks owned by the machine.
use horizon_client::{
    ae2_world::{Machine, MachineKind},
    items::ItemStack,
};

/// The chest window a machine's menu rides on, and where its entries are.
/// AE2's own fit 27 entries. A bigger Pattern Provider takes 54: its patterns,
/// its nine return slots, its three settings, the priority last. A bigger
/// Interface shows a page of 18 config and 18 storage slots at a time, as
/// ExtendedAE's `ContainerExInterface` does; the page is the machine's, shared
/// by all who look at it. Its entries: storage 0..18, config 18..36, the card
/// 36, the fuzzy setting 41, the page 43, the priority 53. `logical` numbers
/// them as AE2's Interface menu does for the whole machine: storage 0..n,
/// config n..2n, the card 2n, the settings 2n + 1..=2n + 5.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MenuShape {
    /// The entries before the player's inventory.
    pub size: usize,
    /// The machine's physical slots.
    pub slots: usize,
    /// The first slot of the page shown, when the slots come in pages.
    pub page: Option<usize>,
}
/// Slots a page of the Extended Interface shows, of each kind.
pub(super) const PAGE_SLOTS: usize = 18;
/// The page setting's entry, and the machine config it sets.
pub(super) const PAGE_ENTRY: i16 = 43;
pub(super) const PAGE_KEY: &str = "ae2x_page";
impl Default for MenuShape {
    fn default() -> Self {
        Self { size: 27, slots: 9, page: None }
    }
}
impl MenuShape {
    pub fn of(m: &Machine) -> Self {
        let slots = m.slots.len();
        match m.kind {
            MachineKind::PatternProvider if slots > 9 => Self { size: 54, slots, page: None },
            MachineKind::Interface if slots > 9 => {
                let page = match m.config.get(&PAGE_KEY.into()) {
                    Some(horizon_world::nbt::Tag::Int(page)) => usize::try_from(*page).unwrap_or(0),
                    _ => 0,
                };
                Self { size: 54, slots, page: Some(page.min(slots / PAGE_SLOTS - 1) * PAGE_SLOTS) }
            }
            _ => Self { size: 27, slots, page: None },
        }
    }
    pub fn window(self) -> &'static str {
        if self.size == 54 { "minecraft:generic_9x6" } else { "minecraft:generic_9x3" }
    }
    /// The Pattern Provider's return slots.
    pub fn returns(self) -> std::ops::Range<i16> {
        self.slots as i16..self.slots as i16 + 9
    }
    /// The first of the Pattern Provider's three settings.
    pub fn settings(self) -> i16 {
        self.slots as i16 + 9
    }
    pub fn priority(self) -> usize {
        self.size - 1
    }
    /// The logical number of entry or player slot `window`: itself, but for
    /// a paged menu's. Its player slots come after every logical number.
    pub fn logical(self, window: i16) -> i16 {
        let Some(first) = self.page else { return window };
        let (n, e) = (self.slots as i16, window);
        match e {
            0..18 => first as i16 + e,
            18..36 => n + first as i16 + e - 18,
            36..54 => 2 * n + e - 36,
            _ if e < 0 => e,
            _ => 4 * n + e,
        }
    }
    /// The Interface's config slots, in logical numbers.
    pub fn config(self) -> std::ops::Range<i16> {
        self.slots as i16..2 * self.slots as i16
    }
    /// The entries holding config slots (their fake clicks).
    pub fn config_entries(self) -> std::ops::Range<u8> {
        if self.page.is_some() { 18..36 } else { 9..18 }
    }
    /// Where window slot `window` lands in the click code: 300 + the machine's
    /// slot, or the player's slot.
    pub fn internal(self, window: i16, interface: bool) -> Option<usize> {
        let w = usize::try_from(window).ok()?;
        if w >= self.size {
            return match w - self.size {
                player @ 0..27 => Some(9 + player),
                player @ 27..36 => Some(player - 27),
                _ => None,
            };
        }
        if let Some(first) = self.page {
            return match w {
                0..18 => Some(300 + first + w),
                36 => Some(300 + self.slots),
                _ => None,
            };
        }
        Some(300 + if interface && w == 2 * self.slots { self.slots } else { w })
    }
}
/// What a paged Interface's window shows: the page's storage and config slots
/// (fluids as their buckets), and the card.
pub(super) fn paged_interface_items(m: &Machine, shape: MenuShape) -> Vec<Option<ItemStack>> {
    let mut items = vec![None; shape.size];
    let Some(first) = shape.page else { return items };
    let bucket = |key: &ae2_cell_core::ItemKey, amount: u64| {
        let mut icon = ItemStack::new(super::ae2_fluids::fluid_bucket(&key.item_id).unwrap_or("minecraft:bucket"), 1);
        icon.data_mut().custom_name = Some(horizon_client::item_data::text_name(format!("{}: {} mB", key.item_id, amount)));
        icon
    };
    let stock = super::ae2_interface::interface_fluid_stock(m).unwrap_or_default();
    let config = super::ae2_interface::interface_config(m).unwrap_or_default();
    let fluid_config = super::ae2_interface::interface_fluid_config(m).unwrap_or_default();
    for i in 0..PAGE_SLOTS {
        let at = first + i;
        items[i] = m.slots.get(at).cloned().flatten()
            .or_else(|| stock.get(at).cloned().flatten().map(|(key, n)| bucket(&key, n)));
        items[PAGE_SLOTS + i] = config.get(at).cloned().flatten()
            .or_else(|| fluid_config.get(at).cloned().flatten().map(|(key, n)| bucket(&key, n)));
    }
    items[2 * PAGE_SLOTS] = m.upgrades.first().cloned().flatten();
    items
}
#[cfg(test)]
mod menu_shape_tests {
    use super::*;

    #[test]
    fn ae2s_own_windows_keep_their_numbers() {
        let plain = MenuShape::of(&Machine::new(MachineKind::Interface));
        assert_eq!(plain, MenuShape { size: 27, slots: 9, page: None });
        assert_eq!(plain.window(), "minecraft:generic_9x3");
        assert_eq!((plain.priority(), plain.settings(), plain.returns(), plain.config()), (26, 18, 9..18, 9..18));
        assert_eq!(plain.logical(17), 17);
        // The card's entry is the slot after the physical ones.
        assert_eq!(plain.internal(18, true), Some(309));
        assert_eq!(plain.internal(18, false), Some(318));
        assert_eq!(plain.internal(26, false), Some(326));
        assert_eq!(plain.internal(27, false), Some(9));
        assert_eq!(plain.internal(53, false), Some(35));
        assert_eq!(plain.internal(54, false), Some(0));
        assert_eq!(plain.internal(62, false), Some(8));
        assert_eq!(plain.internal(63, false), None);
        assert_eq!(plain.internal(-999, false), None);
        let drive = MenuShape::of(&Machine::new(MachineKind::Drive));
        assert_eq!((drive.size, drive.slots, drive.priority()), (27, 10, 26));
    }

    #[cfg(feature = "ae2-extendedae")]
    #[test]
    fn extended_devices_have_their_windows() {
        horizon_client::ae2x::install();
        let machine = |id: &str| Machine::for_block(id).unwrap();
        let provider = MenuShape::of(&machine("extendedae:ex_pattern_provider"));
        assert_eq!(provider.window(), "minecraft:generic_9x6");
        assert_eq!((provider.priority(), provider.settings(), provider.returns()), (53, 45, 36..45));
        assert_eq!(provider.internal(35, false), Some(335));
        assert_eq!(provider.internal(54, false), Some(9));
        assert_eq!(provider.internal(89, false), Some(8));

        let drive = MenuShape::of(&machine("extendedae:ex_drive"));
        assert_eq!((drive.size, drive.slots, drive.priority()), (27, 20, 26));
        assert_eq!(drive.internal(19, false), Some(319));

        let mut interface = machine("extendedae:ex_interface");
        let first = MenuShape::of(&interface);
        assert_eq!(first, MenuShape { size: 54, slots: 36, page: Some(0) });
        assert_eq!((first.logical(0), first.logical(17)), (0, 17));
        assert_eq!((first.logical(18), first.logical(35)), (36, 53));
        // The card, then the settings as AE2's Interface numbers them.
        assert_eq!(first.logical(36), 72);
        assert_eq!(first.logical(41), 77);
        assert_eq!(first.config(), 36..72);
        assert_eq!(first.config_entries(), 18..36);
        assert_eq!(first.internal(17, true), Some(317));
        assert_eq!(first.internal(18, true), None);
        assert_eq!(first.internal(36, true), Some(336));
        assert_eq!(first.internal(54, true), Some(9));
        // Player slots lie past every logical number.
        assert!(first.logical(54) > first.logical(53));

        interface.config.insert(PAGE_KEY.into(), horizon_world::nbt::Tag::Int(1));
        let second = MenuShape::of(&interface);
        assert_eq!(second.page, Some(18));
        assert_eq!((second.logical(0), second.logical(17)), (18, 35));
        assert_eq!((second.logical(18), second.logical(35)), (54, 71));
        assert_eq!(second.internal(17, true), Some(335));

        // A saved page past the last one shows the last.
        interface.config.insert(PAGE_KEY.into(), horizon_world::nbt::Tag::Int(9));
        assert_eq!(MenuShape::of(&interface).page, Some(18));
    }

    #[cfg(feature = "ae2-extendedae")]
    #[test]
    fn a_page_shows_its_slots_and_the_card() {
        horizon_client::ae2x::install();
        let mut interface = Machine::for_block("extendedae:ex_interface").unwrap();
        interface.slots[20] = Some(ItemStack::new("minecraft:dirt", 3));
        interface.upgrades[0] = Some(ItemStack::new("ae2:fuzzy_card", 1));
        super::super::ae2_interface::set_interface_config(&mut interface, 35, Some(&ItemStack::new("minecraft:sand", 5))).unwrap();
        interface.config.insert(PAGE_KEY.into(), horizon_world::nbt::Tag::Int(1));
        let items = paged_interface_items(&interface, MenuShape::of(&interface));
        assert_eq!(items.len(), 54);
        assert_eq!(items[2].as_ref().unwrap().item.as_ref(), "minecraft:dirt");
        assert_eq!(items[35].as_ref().unwrap().item.as_ref(), "minecraft:sand");
        assert_eq!(items[36].as_ref().unwrap().item.as_ref(), "ae2:fuzzy_card");
        assert_eq!(items.iter().flatten().count(), 3);
        interface.config.remove(&PAGE_KEY.into());
        let items = paged_interface_items(&interface, MenuShape::of(&interface));
        assert_eq!(items.iter().flatten().count(), 1);
    }
}
fn max_card(kind: MachineKind, id: &str) -> usize {
    use MachineKind::*;
    match (kind, id) {
        (MolecularAssembler, "ae2:speed_card") => 5,
        (Inscriber, "ae2:speed_card") => 4,
        (VibrationChamber, "ae2:speed_card" | "ae2:energy_card") => 3,
        (IoPort, "ae2:speed_card") => 3,
        (IoPort, "ae2:redstone_card") => 1,
        (Interface, "ae2:crafting_card" | "ae2:fuzzy_card") => 1,
        _ => 0,
    }
}
/// May replace this upgrade slot with one actual card. Per-type limits are
/// checked with the prior contents of the target slot excluded.
pub(super) fn machine_upgrade_allowed(m: &Machine, slot: usize, item: &ItemStack) -> bool {
    if slot >= m.upgrades.len() || item.count == 0 {
        return false;
    }
    let max = max_card(m.kind, &item.item);
    if max == 0 {
        return false;
    }
    let used: usize = m
        .upgrades
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != slot)
        .filter_map(|(_, s)| s.as_ref())
        .filter(|s| s.item == item.item)
        .map(|s| s.count as usize)
        .sum();
    used < max
}
/// Cursor pickup/place: each slot holds one actual card. Invalid inputs leave
/// both sides untouched; no client prediction or copied physical stacks.
pub(super) fn machine_upgrade_pickup(
    m: &mut Machine,
    slot: usize,
    carried: &mut Option<ItemStack>,
) -> bool {
    if slot >= m.upgrades.len() {
        return false;
    }
    if carried.is_none() {
        *carried = m.upgrades[slot].take();
        return true;
    }
    let Some(held) = carried.as_ref() else {
        return false;
    };
    if !machine_upgrade_allowed(m, slot, held) {
        return false;
    }
    if m.upgrades[slot].is_some() && held.count != 1 {
        return false;
    }
    let mut held = carried.take().unwrap();
    let replacement = ItemStack {
        count: 1,
        ..held.clone()
    };
    held.count -= 1;
    let prior = m.upgrades[slot].replace(replacement);
    *carried = if let Some(prior) = prior {
        Some(prior)
    } else if held.count > 0 {
        Some(held)
    } else {
        None
    };
    true
}
/// Shift-insert cards into empty machine upgrade slots and return transferred
/// count; caller commits player inventory and machine under one server turn.
pub(super) fn machine_upgrade_quick_insert(m: &mut Machine, item: &mut ItemStack) -> u32 {
    let mut moved = 0;
    for i in 0..m.upgrades.len() {
        if item.count == 0 {
            break;
        }
        if m.upgrades[i].is_none() && machine_upgrade_allowed(m, i, item) {
            m.upgrades[i] = Some(ItemStack {
                count: 1,
                ..item.clone()
            });
            item.count -= 1;
            moved += 1;
        }
    }
    moved
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn io_port_cards_obey_limits_and_conserve_cursor() {
        let mut m = Machine::new(MachineKind::IoPort);
        let mut held = Some(ItemStack::new("ae2:redstone_card", 2));
        assert!(machine_upgrade_pickup(&mut m, 0, &mut held));
        assert_eq!(held.as_ref().unwrap().count, 1);
        assert!(!machine_upgrade_pickup(&mut m, 1, &mut held));
        assert!(m.upgrades[1].is_none());
        let mut speed = ItemStack::new("ae2:speed_card", 3);
        assert_eq!(machine_upgrade_quick_insert(&mut m, &mut speed), 2);
        assert_eq!(speed.count, 1);
        assert_eq!(m.installed_speed_cards(), 2);
        assert_eq!(m.physical_contents().len(), 3);
        let mut cursor = None;
        assert!(machine_upgrade_pickup(&mut m, 0, &mut cursor));
        assert_eq!(cursor.unwrap().item.as_ref(), "ae2:redstone_card");
    }
    #[test]
    fn mismatched_upgrade_and_non_card_cannot_replace_existing() {
        let mut m = Machine::new(MachineKind::Inscriber);
        m.upgrades[0] = Some(ItemStack::new("ae2:speed_card", 1));
        let mut held = Some(ItemStack::new("ae2:energy_card", 1));
        assert!(!machine_upgrade_pickup(&mut m, 0, &mut held));
        assert_eq!(
            m.upgrades[0].as_ref().unwrap().item.as_ref(),
            "ae2:speed_card"
        );
        assert_eq!(held.unwrap().item.as_ref(), "ae2:energy_card");
    }
}
