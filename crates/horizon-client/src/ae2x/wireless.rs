//! Wireless terminal items as the definitions registry lists them: which
//! terminal an item opens (a universal terminal its current one), the
//! terminals a universal terminal holds and their order, its upgrade slots,
//! battery and charge rate, and merging a terminal into it.
//!
//! Derived from AE2 19.2.18 `WirelessTerminalItem` and `AEBasePoweredItem`
//! (LGPL-3.0-or-later) and AE2WTLib 19.5.1 `WTDefinition`, `ItemWUT`,
//! `WUTHandler` and `Common.mergeTerminal` (MIT, Mari023,
//! https://github.com/Mari023/AE2WirelessTerminalLibrary tag `19.5.1`).
use super::defs::{self, TerminalKind, WirelessTerminal};
use crate::{ae2_powered_items, ae2_upgrade_inventory::Inventory, items::ItemStack, save};
use horizon_world::nbt::{Compound, Tag};

/// The stored energy and its legacy alias in the port's own custom data.
const STORED: &str = "ae2:stored_energy";
const STORED_ALIAS: &str = "ae2_wireless_energy";
const CAPACITY: &str = "ae2:energy_capacity";

/// One wireless terminal item, the universal terminal included
/// (`WirelessTerminalItem`).
pub fn host(stack: &ItemStack) -> bool {
    stack.count == 1 && defs::get().is_wireless_terminal(&stack.item)
}

/// The universal terminal (`ItemWUT`).
pub fn is_universal(stack: &ItemStack) -> bool {
    defs::get().universal_terminal().is_some_and(|u| u.item == *stack.item)
}

/// The terminals a universal terminal holds, in registration order
/// (`wirelessTerminalList`); none for any other item.
pub fn installed(stack: &ItemStack) -> Vec<&'static WirelessTerminal> {
    if !is_universal(stack) {
        return Vec::new();
    }
    defs::get()
        .wireless_terminals()
        .filter(|t| t.installed.as_deref().is_some_and(|c| stack.data().and_then(|d| d.other(c)).is_some()))
        .collect()
}

/// `WUTHandler.hasTerminal`: a universal terminal holding it, or the
/// terminal's own item.
pub fn has_terminal(stack: &ItemStack, terminal: &WirelessTerminal) -> bool {
    if stack.count == 0 {
        return false;
    }
    if is_universal(stack) {
        return terminal.installed.as_deref().is_some_and(|c| stack.data().and_then(|d| d.other(c)).is_some());
    }
    *stack.item == terminal.item
}

/// The name `ae2wtlib:current_terminal` holds, when it names a terminal.
fn current_name(stack: &ItemStack) -> Option<&'static WirelessTerminal> {
    let universal = defs::get().universal_terminal()?;
    let Tag::String(name) = stack.data()?.other(&universal.current)? else {
        return None;
    };
    defs::get().wireless_terminal_named(&name.to_string().ok()?)
}

/// `WTDefinition.ofOrNull`: the terminal a wireless item opens. A universal
/// terminal opens the one `current_terminal` names, else the first it
/// holds; none when it holds none.
pub fn terminal(stack: &ItemStack) -> Option<&'static WirelessTerminal> {
    if is_universal(stack) {
        return current_name(stack).or_else(|| installed(stack).into_iter().next());
    }
    defs::get().wireless_terminal(&stack.item)
}

/// What the item's menu is.
pub fn kind(stack: &ItemStack) -> Option<TerminalKind> {
    terminal(stack).map(|t| t.kind)
}

/// `WUTHandler.nextTerminal`: from the current terminal, the next (or with
/// `reverse` the previous) one the universal terminal holds, round the
/// registration order. None when it holds none.
pub fn next_terminal(stack: &ItemStack, reverse: bool) -> Option<&'static WirelessTerminal> {
    let list: Vec<_> = defs::get().wireless_terminals().filter(|t| t.installed.is_some()).collect();
    let held = installed(stack);
    if held.is_empty() || list.is_empty() {
        return None;
    }
    let current = terminal(stack)?;
    let mut index = list.iter().position(|t| t.name == current.name).unwrap_or(0);
    loop {
        index = if reverse { (index + list.len() - 1) % list.len() } else { (index + 1) % list.len() };
        if held.iter().any(|t| t.name == list[index].name) {
            return Some(list[index]);
        }
    }
}

/// `WUTHandler.setCurrentTerminal`: only a universal terminal, only to a
/// terminal it holds.
pub fn set_current(stack: &mut ItemStack, terminal: &WirelessTerminal) -> bool {
    let Some(universal) = defs::get().universal_terminal() else {
        return false;
    };
    if !is_universal(stack) || !has_terminal(stack, terminal) {
        return false;
    }
    stack.data_mut().set_other(&universal.current, Some(Tag::String(terminal.name.as_str().into())));
    crate::ae2_stack_identity::synchronize_tracked(stack);
    true
}

/// `IUpgradeableItem.getUpgrades`' size: a universal terminal has two slots
/// for each terminal it holds (`ItemWUT.getUpgrades`), any other item its row's.
pub fn upgrade_slots(stack: &ItemStack) -> Option<usize> {
    if is_universal(stack) {
        return Some(2 * installed(stack).len());
    }
    defs::get().upgrade_slots(&stack.item)
}

/// `setAEMaxPowerMultiplier` in `onUpgradesChanged`, clamped to 1..=100:
/// 1 + the energy cards on a wireless terminal, the terminals held + the
/// energy cards on a universal terminal (`ItemWUT.onUpgradesChanged`).
pub fn energy_multiplier(stack: &ItemStack, energy_cards: u32) -> f64 {
    let terminals = if is_universal(stack) { installed(stack).len() as u32 } else { 1 };
    f64::from((terminals + energy_cards).clamp(1, 100))
}

/// `Upgrades.getEnergyCardMultiplier` of the item's cards.
fn cards(stack: &ItemStack) -> Result<u32, String> {
    if !host(stack) {
        return Err("not a wireless terminal".into());
    }
    Ok(Inventory::read(stack)?.energy_card_multiplier())
}

/// The stored energy and the battery's size (`getAEMaxPower`: the
/// `ae2:energy_capacity` component, else the configured battery).
pub fn energy(stack: &ItemStack) -> Result<(f64, f64), String> {
    let base = crate::ae2_common_config::wireless().terminal_capacity;
    // Native saves from before the source components kept their cards in
    // custom data, with the capacity they gave left implicit.
    let legacy_max = base * energy_multiplier(stack, cards(stack)?);
    let default = if stack.data().and_then(|data| data.other("ae2:upgrades")).is_some() { base } else { legacy_max };
    let max = ae2_powered_items::component_power_value(stack, CAPACITY, CAPACITY, default)?;
    let power = ae2_powered_items::component_power_value(stack, STORED, STORED_ALIAS, 0.0)?;
    Ok((power, max))
}

/// `getChargeRate`: 800 + 800 × the energy cards
/// (`WirelessTerminalItem`); on a universal terminal 800 × (the terminals
/// held + 1 + the energy cards of AE2's own card, `ItemWUT.getChargeRate`).
pub fn charge_rate(stack: &ItemStack) -> Result<f64, String> {
    if is_universal(stack) {
        let own = Inventory::read(stack)?.slots.iter().flatten().filter(|card| card.item.as_ref() == "ae2:energy_card").count();
        return Ok(800.0 * (installed(stack).len() + 1 + own) as f64);
    }
    Ok(800.0 * (1.0 + f64::from(cards(stack)?)))
}

/// `injectAEPower` / `extractAEPower`: moves up to `amount`; an insert into
/// a battery already past its size clamps it (`AEBasePoweredItem`).
pub fn transfer_energy(stack: &mut ItemStack, amount: f64, insert: bool, simulate: bool) -> Result<f64, String> {
    if !amount.is_finite() || amount < 0.0 {
        return Err("invalid wireless power amount".into());
    }
    let (current, max) = energy(stack)?;
    let moved = amount.min(if insert { (max - current).max(0.0) } else { current });
    if !simulate && (moved > 0.0 || (insert && current > max)) {
        let next = (current + if insert { amount.min(max - current) } else { -moved }).max(0.0);
        ae2_powered_items::set_component_power_value(stack, STORED, STORED_ALIAS, (next >= 0.0001).then_some(next))?;
    }
    Ok(moved)
}

/// `Common.mergeTerminal`: `terminal`'s item put into a universal terminal.
/// The universal terminal takes the terminal's component, its cards as far
/// as they fit (`addItems`), a battery for one terminal more, its stored
/// energy, and its other components (crafting grid, link, settings, name).
pub fn merge_terminal(universal: &ItemStack, merged: &ItemStack, terminal: &WirelessTerminal) -> Result<ItemStack, String> {
    let defs = defs::get();
    if !is_universal(universal) {
        return Err("not a universal terminal".into());
    }
    let component = terminal.installed.as_deref().ok_or("this terminal can't be combined")?;
    // `toMerge.getItem() instanceof ItemWT`: AE2WTLib's terminals and the universal one.
    if !(defs.wireless_terminal(&merged.item).is_some_and(|t| t.installed.is_some()) || is_universal(merged)) {
        return Err("not a combinable wireless terminal".into());
    }
    // The other components first, as the patch; the battery, its size and
    // the cards are recounted below.
    let Tag::Compound(mut out) = save::item_to_nbt(universal) else {
        return Err("invalid universal terminal".into());
    };
    let mut components = match out.get(&"components".into()) {
        Some(Tag::Compound(c)) => c.clone(),
        _ => Compound::new(),
    };
    if let Some(Tag::Compound(patch)) = save::item_to_nbt(merged).get("components") {
        for (key, value) in patch {
            let id = key.to_string_lossy();
            if matches!(id.as_ref(), STORED | CAPACITY | "ae2:upgrades" | "!ae2:stored_energy" | "!ae2:energy_capacity" | "!ae2:upgrades") {
                continue;
            }
            let mut value = value.clone();
            if id == "minecraft:custom_data"
                && let Tag::Compound(custom) = &mut value
            {
                for legacy in [STORED_ALIAS, "ae2_wireless_upgrades", CAPACITY] {
                    custom.remove(&legacy.into());
                }
                if let Some(Tag::Compound(own)) = components.get(&key.clone()) {
                    for legacy in [STORED_ALIAS, "ae2_wireless_upgrades", CAPACITY] {
                        if let Some(kept) = own.get(&legacy.into()) {
                            custom.insert(legacy.into(), kept.clone());
                        }
                    }
                }
            }
            components.insert(key.clone(), value);
        }
    }
    components.insert(component.into(), Tag::Compound(Compound::new()));
    out.insert("components".into(), Tag::Compound(components));
    let mut wut = save::item_from_nbt(&Tag::Compound(out)).ok_or("invalid universal terminal")?;
    // The cards: AE2's `addItems` into the larger inventory; what doesn't
    // fit is lost, as in the recipe. Storing them sets the battery's size
    // for the terminals held (`onUpgradesChanged`).
    let mut upgrades = Inventory::read(&wut)?;
    if let Ok(cards) = Inventory::read(merged) {
        for card in cards.slots.iter().flatten() {
            upgrades.insert_one(card);
        }
    }
    upgrades.store(&mut wut)?;
    let (power, _) = energy(merged)?;
    transfer_energy(&mut wut, power, true, false)?;
    Ok(wut)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ae2_terminals_open_their_own_menus_and_are_not_universal() {
        let terminal = ItemStack::new("ae2:wireless_terminal", 1);
        let crafting = ItemStack::new("ae2:wireless_crafting_terminal", 1);
        assert!(host(&terminal) && host(&crafting));
        assert!(!host(&ItemStack::new("ae2:wireless_terminal", 2)), "a stack of two is no menu host");
        assert!(!host(&ItemStack::new("ae2:terminal", 1)));
        assert_eq!(kind(&terminal), Some(TerminalKind::Me));
        assert_eq!(kind(&crafting), Some(TerminalKind::Crafting));
        assert!(installed(&crafting).is_empty() && next_terminal(&crafting, false).is_none());
        assert_eq!(upgrade_slots(&crafting), Some(2));
        assert_eq!(energy_multiplier(&terminal, 2), 3.0);
        assert_eq!(charge_rate(&terminal).unwrap(), 800.0);
    }
}
