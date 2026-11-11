//! Applied Flux's FE in AE2's generic slots and in items, derived from
//! Applied Flux 1.21-2.1.7 by GlodBlock (`FEGenericStackInvStorage`,
//! `AppFlux`'s capability listener, `AFUtil.shouldTryCast`,
//! `FEContainerItemStrategy`), LGPL-3.0; see `ae2/addons/appflux/`.
//!
//! A block with AE2's `GENERIC_INTERNAL_INV` (the ME Interface's storage, the
//! Pattern Provider's return inventory, as blocks and as parts) answers Forge
//! Energy over the FE in its slots, when its Induction Card lets it: a machine
//! pushes FE back into a provider (which returns it to the grid, a waiting
//! crafting CPU first) or into an interface. An item's FE
//! (`Capabilities.EnergyStorage.ITEM`) is AE2's `PoweredItemCapabilities` over
//! its AE: the terminal fills it from the grid's FE.
use super::super::super::{Game, ae2_global::GridAddress, ae2_power::FeStorage};
use ae2_cell_core::ItemKey;
use horizon_client::{ae2_key_types as key_types, ae2_world::Machine, ae2x::appflux as rows, items::ItemStack};
use horizon_core::geometry::Direction;
use horizon_world::nbt::Tag;

/// `AFSingletons.INDUCTION_CARD`.
pub(in crate::game) const INDUCTION_CARD: &str = rows::INDUCTION_CARD;

/// `AFUtil.shouldTryCast`: an interface or a pattern provider (block or part,
/// each an `IUpgradeableObject` with Applied Flux) gives FE only with an
/// Induction Card installed (`isUpgradedWith`): in the interface's two
/// upgrade slots, or the provider's one (`MixinPatternProviderLogic`'s
/// inventory, the machine's upgrades here).
pub(in crate::game) fn induction_card(machine: &Machine) -> bool {
    machine.upgrades.iter().flatten().any(|card| card.item.as_ref() == INDUCTION_CARD && card.count == 1)
}
/// The FE key as the fluid lane names it.
fn fe_marker() -> ItemKey {
    key_types::addon_marker(rows::key_type(), &rows::fe_key())
}
fn clamp(amount: u64) -> i32 {
    amount.min(i32::MAX as u64) as i32
}

/// The FE storage `FEGenericStackInvStorage` reads from slots: what holds FE,
/// and how many slots are FE's or free (`getMaxEnergyStored` counts both).
fn storage_over(slots: impl Iterator<Item = Option<(ItemKey, u64)>>, occupied: impl Iterator<Item = bool>, capacity: u64) -> FeStorage {
    let fe = fe_marker();
    let (mut stored, mut open) = (0u64, 0u64);
    for (slot, taken) in slots.zip(occupied) {
        match slot {
            Some((key, n)) if key == fe => {
                stored = stored.saturating_add(n);
                open += 1;
            }
            None if !taken => open += 1,
            _ => {}
        }
    }
    FeStorage { stored: clamp(stored), capacity: clamp(open.saturating_mul(capacity)), can_receive: true, can_extract: true }
}

/// A provider's return slots: each slot's key of the fluid lane, and whether
/// an item fills it.
fn provider_slots(machine: &Machine) -> Option<(Vec<Option<(ItemKey, u64)>>, Vec<bool>)> {
    let mut lane = vec![None; 9];
    let mut items = vec![false; 9];
    if let Some(Tag::List(entries)) = machine.config.get(&"ae2_provider_return_items".into()) {
        for entry in entries {
            let Some(Tag::Byte(slot)) = entry.get("Slot") else { return None };
            *items.get_mut(*slot as usize)? = true;
        }
    }
    if let Some(Tag::List(entries)) = machine.config.get(&"ae2_provider_return_fluids".into()) {
        for entry in entries {
            let (Some(Tag::Byte(slot)), Some(key), Some(Tag::Long(n))) =
                (entry.get("Slot"), key_types::lane_entry_key(entry, "fluid"), entry.get("amount"))
            else {
                return None;
            };
            *lane.get_mut(*slot as usize)? = Some((key, u64::try_from(*n).ok()?));
        }
    }
    Some((lane, items))
}

impl Game<'_> {
    /// `FEGenericStackInvStorage` on the generic inventory that side reaches
    /// (an interface's storage, a provider's return slots), when
    /// `shouldTryCast` lets it; none elsewhere.
    pub(in crate::game) fn appflux_generic_fe_storage(&self, address: GridAddress, face: Direction) -> Option<FeStorage> {
        if let Some(machine) = self.ae2_interface_machine_at(address, face) {
            if !induction_card(&machine) {
                return None;
            }
            let stock = super::super::super::ae2_interface::interface_fluid_stock(&machine).ok()?;
            let capacity = super::super::super::ae2_interface::interface_key_capacity(&machine, &fe_marker());
            let taken: Vec<bool> = machine.slots.iter().map(Option::is_some).collect();
            return Some(storage_over(stock.into_iter(), taken.into_iter(), capacity));
        }
        let machine = self.ae2_provider_return_machine_at(address, face)?;
        if !induction_card(&machine) {
            return None;
        }
        let (lane, items) = provider_slots(&machine)?;
        Some(storage_over(lane.into_iter(), items.into_iter(), key_types::lane_capacity(&fe_marker())))
    }
    /// `receiveEnergy` / `extractEnergy` there, slot by slot: what moved. A
    /// provider's return inventory takes FE and gives none back
    /// (`PatternProviderReturnInventory.canExtract`); its tick returns it to the
    /// grid.
    pub(in crate::game) fn appflux_generic_fe_move(&mut self, address: GridAddress, face: Direction, amount: u32, simulate: bool, extract: bool) -> Option<u32> {
        self.appflux_generic_fe_storage(address, face)?;
        let fe = fe_marker();
        let amount = u64::from(amount);
        let moved = if self.ae2_interface_machine_at(address, face).is_some() {
            self.ae2_interface_storage_move(address, face, &fe, amount, !extract, simulate)?
        } else if extract {
            0
        } else {
            self.ae2_provider_return_fluid_at(address, face, &fe, amount, simulate)?
        };
        Some(clamp(moved) as u32)
    }
}

/// `FEContainerItemStrategy` over `Capabilities.EnergyStorage.ITEM`, which is
/// AE2's `PoweredItemCapabilities` on its chargeable items (portable cells,
/// Applied Flux's among them, wireless terminals, tools, energy cells):
/// `getEnergyStored` the AE held in FE, `receiveEnergy` through
/// `injectAEPower`, `extractEnergy` none.
pub(in crate::game) fn container_strategy() -> key_types::ContainerStrategy {
    key_types::ContainerStrategy { contained: item_contained, fill: item_fill, drain: |_, _, _| None }
}
fn fe_ratio() -> f64 {
    horizon_client::ae2_common_config::gameplay().forge_energy
}
/// One item of `stack`, if AE2 charges it.
fn powered_one(stack: &ItemStack) -> Option<(ItemStack, f64)> {
    let one = ItemStack { count: 1, ..stack.clone() };
    let (current, _) = super::super::super::ae2_processing::chargeable_energy(&one).ok()?;
    Some((one, current))
}
/// `getContainedStack`: the item's FE when it holds any.
fn item_contained(stack: &ItemStack) -> Option<(ItemKey, u64)> {
    let (_, current) = powered_one(stack)?;
    let stored = (current / fe_ratio()) as i32;
    (stored > 0).then(|| (rows::fe_key(), stored as u64))
}
/// `insert`: `receiveEnergy` of one item: the FE offered less the AE it
/// refuses, in FE (`PoweredItemCapabilities.receiveEnergy`).
fn item_fill(stack: &ItemStack, key: &ItemKey, amount: u64) -> Option<(u64, ItemStack)> {
    if key.item_id != rows::FE || !key.components.is_empty() {
        return None;
    }
    let (mut one, _) = powered_one(stack)?;
    let offered_fe = amount.min(i32::MAX as u64) as i32;
    let offered = f64::from(offered_fe) * fe_ratio();
    let accepted = super::super::super::ae2_processing::chargeable_insert(&mut one, offered).ok()?;
    let filled = offered_fe - ((offered - accepted) / fe_ratio()) as i32;
    (filled > 0).then_some((filled as u64, one))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_induction_card_opens_an_interface_or_a_provider() {
        use horizon_client::ae2_world::MachineKind;
        horizon_client::ae2x::install();
        let mut interface = Machine::new(MachineKind::Interface);
        assert!(!induction_card(&interface));
        interface.upgrades[0] = Some(ItemStack::new(INDUCTION_CARD, 1));
        assert!(induction_card(&interface));
        interface.upgrades[0] = Some(ItemStack::new("ae2:fuzzy_card", 1));
        assert!(!induction_card(&interface));
        // Applied Flux's second slot of the interface.
        interface.upgrades[1] = Some(ItemStack::new(INDUCTION_CARD, 1));
        assert!(induction_card(&interface));
        let mut provider = Machine::new(MachineKind::PatternProvider);
        assert!(!induction_card(&provider));
        provider.upgrades[0] = Some(ItemStack::new(INDUCTION_CARD, 1));
        assert!(induction_card(&provider));
    }

    #[test]
    fn generic_slots_count_fe_and_free_slots() {
        rows::key_type();
        let fe = fe_marker();
        let water = ItemKey { item_id: "minecraft:water".into(), components: Vec::new() };
        let slots = vec![Some((fe.clone(), 400_000)), None, Some((water, 1000)), None];
        let storage = storage_over(slots.into_iter(), [false, false, false, true].into_iter(), 1_000_000);
        // getEnergyStored: the FE; getMaxEnergyStored: FE's slot and the free one, 1,000,000 each.
        assert_eq!((storage.stored, storage.capacity), (400_000, 2_000_000));
        assert_eq!(key_types::lane_capacity(&fe), rows::SLOT_CAPACITY);
    }

    #[test]
    fn an_interface_slot_holds_the_types_capacity() {
        use super::super::super::super::ae2_interface::{interface_fluid_capacity, interface_key_capacity};
        use horizon_client::ae2_world::MachineKind;
        rows::key_type();
        let interface = Machine::new(MachineKind::Interface);
        assert_eq!(interface_key_capacity(&interface, &fe_marker()), rows::SLOT_CAPACITY);
        let water = ItemKey { item_id: "minecraft:water".into(), components: Vec::new() };
        assert_eq!(interface_key_capacity(&interface, &water), interface_fluid_capacity(&interface));
        // ExtendedAE's Oversize Interface: OversizeConfigInv's multiplier for the type, on the type's capacity.
        #[cfg(feature = "ae2-extendedae")]
        {
            use horizon_client::ae2x::extendedae::{Config, special};
            horizon_client::ae2x::defs::get();
            let oversize = Machine::of_variant(horizon_client::ae2_world::variant(special::OVERSIZE_INTERFACE).unwrap());
            let multiplier = Config::get().oversize_multiplier(rows::KEY_TYPE);
            assert!(multiplier > 1);
            assert_eq!(interface_key_capacity(&oversize, &fe_marker()), special::oversize_capacity(rows::SLOT_CAPACITY, multiplier));
        }
    }

    #[test]
    fn a_chargeable_item_takes_fe_and_gives_none() {
        rows::key_type();
        let cell = ItemStack::new("ae2:energy_cell", 1);
        assert_eq!(item_contained(&cell), None, "an empty energy cell holds no FE");
        let (filled, charged) = item_fill(&cell, &rows::fe_key(), 1_000).unwrap();
        assert_eq!(filled, 1_000);
        assert_eq!(item_contained(&charged), Some((rows::fe_key(), 1_000)));
        assert!(item_fill(&cell, &ItemKey { item_id: rows::GTEU.into(), components: Vec::new() }, 1_000).is_none());
        assert!((container_strategy().drain)(&charged, &rows::fe_key(), 1_000).is_none(), "PoweredItemCapabilities.extractEnergy is 0");
        assert!(item_contained(&ItemStack::new("minecraft:stone", 1)).is_none());
    }
}
