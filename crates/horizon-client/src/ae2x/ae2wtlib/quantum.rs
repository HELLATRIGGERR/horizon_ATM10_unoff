//! AE2WTLib's Quantum Bridge Card on a terminal item: the card
//! (`AE2wtlibAPI.hasQuantumBridgeCard`), the singularity
//! (`AE2wtlibComponents.SINGULARITY`, `WTMenuHost.getQEFrequency`), the
//! link's results (`Status`) and drain (`getPowerDrainPerTick`), and the
//! recharge's amount (`recharge`). AE2WTLib 19.5.1 by Mari023,
//! https://github.com/Mari023/AE2WirelessTerminalLibrary tag `19.5.1`, MIT;
//! see `ae2/addons/ae2wtlib/`. The bridge and the grid are the game's
//! (`game/ae2x/ae2wtlib/quantum.rs`).
use crate::{ae2_upgrade_inventory::Inventory, items::ItemStack};

/// `AE2wtlibItems.QUANTUM_BRIDGE_CARD`: one on every terminal of the
/// registry and on the universal terminal (`addUpgradeToAllTerminals(card, 1)`).
pub const QUANTUM_BRIDGE_CARD: &str = "ae2wtlib:quantum_bridge_card";
/// `AE2wtlibComponents.SINGULARITY` (`ItemStack.OPTIONAL_CODEC`): the
/// terminal's singularity slot.
pub const SINGULARITY: &str = "ae2wtlib:singularity";
/// The menu's drain while the bridge links it, AE a tick: the bridge's 22
/// and a terminal part's 0.5, not scaled by the distance.
pub const DRAIN: f64 = 22.5;

/// `Status`: why a terminal is not linked through a bridge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Shown nowhere: no card and no singularity, or a singularity without a frequency.
    GenericInvalid,
    NoUpgrade,
    BridgeNotFound,
    DifferentNetworks,
    NotPowered,
    NoSingularity,
}
impl Status {
    /// The error it shows (`Status.error`), from the bundled API's language file.
    pub fn key(self) -> Option<&'static str> {
        match self {
            Self::GenericInvalid => None,
            Self::NoUpgrade => Some("chat.ae2wtlib.NoQuantumBridgeCard"),
            Self::BridgeNotFound => Some("chat.ae2wtlib.NoQuantumBridge"),
            Self::DifferentNetworks => Some("chat.ae2wtlib.NetworkMismatch"),
            Self::NotPowered => Some("chat.ae2wtlib.NetworkNotPowered"),
            Self::NoSingularity => Some("chat.ae2wtlib.SingularityNotPresent"),
        }
    }
}

/// `hasQuantumBridgeCard`: the card in the item's upgrade inventory.
pub fn has_card(stack: &ItemStack) -> bool {
    Inventory::read(stack).is_ok_and(|cards| cards.slots.iter().flatten().any(|card| card.item.as_ref() == QUANTUM_BRIDGE_CARD))
}

/// The stack in the singularity slot, none when it is empty
/// (`getOrDefault(SINGULARITY, EMPTY)`).
pub fn singularity(stack: &ItemStack) -> Option<ItemStack> {
    let tag = stack.data()?.other(SINGULARITY)?;
    crate::save::item_from_nbt(tag).filter(|s| s.count > 0)
}
/// Puts `singularity` in the slot, or empties it.
pub fn set_singularity(stack: &mut ItemStack, singularity: Option<&ItemStack>) {
    stack.data_mut().set_other(SINGULARITY, singularity.map(crate::save::item_to_nbt));
}

/// `isQuantumLinked` up to the bridge: the frequency the terminal's
/// singularity names (`id`: its `ae2:entangled_singularity_id`), or the
/// status. Without the card a missing or unreadable singularity is
/// `GenericInvalid`, as the first failure keeps its status only alone.
pub fn frequency(stack: &ItemStack, id: impl FnOnce(&ItemStack) -> Option<i64>) -> Result<i64, Status> {
    let card = has_card(stack);
    let frequency = match singularity(stack) {
        None => Err(Status::NoSingularity),
        Some(singularity) => id(&singularity).ok_or(Status::GenericInvalid),
    };
    match (card, frequency) {
        (true, frequency) => frequency,
        (false, Ok(_)) => Err(Status::NoUpgrade),
        (false, Err(_)) => Err(Status::GenericInvalid),
    }
}

/// `recharge`: what moves from the grid into the item, `missing` AE short
/// of full, the grid holding `stored` of `max`: as much as it misses, never
/// taking the grid below half. None when nothing moves.
pub fn recharge_amount(missing: f64, stored: f64, max: f64) -> Option<f64> {
    let safe = stored - max / 2.0;
    (missing > 0.0 && safe > 0.0).then(|| missing.min(safe))
}

#[cfg(test)]
mod tests {
    use super::*;
    use horizon_world::nbt::Tag;

    fn terminal(card: bool, singularity: Option<Option<i64>>) -> ItemStack {
        let mut stack = ItemStack::new("ae2:wireless_crafting_terminal", 1);
        if card {
            let mut cards = Inventory::read(&stack).unwrap();
            assert!(cards.insert_one(&ItemStack::new(QUANTUM_BRIDGE_CARD, 1)));
            cards.store(&mut stack).unwrap();
        }
        if let Some(id) = singularity {
            let mut s = ItemStack::new("ae2:quantum_entangled_singularity", 1);
            if let Some(id) = id {
                s.data_mut().set_other("ae2:entangled_singularity_id", Some(Tag::Long(id)));
            }
            set_singularity(&mut stack, Some(&s));
        }
        stack
    }
    fn id(s: &ItemStack) -> Option<i64> {
        match s.data()?.other("ae2:entangled_singularity_id")? {
            Tag::Long(n) => Some(*n),
            _ => None,
        }
    }

    #[test]
    fn the_card_and_the_singularity_give_the_frequency_or_the_first_status() {
        assert_eq!(frequency(&terminal(true, Some(Some(-731))), id), Ok(-731));
        assert_eq!(frequency(&terminal(true, None), id), Err(Status::NoSingularity));
        assert_eq!(frequency(&terminal(true, Some(None)), id), Err(Status::GenericInvalid));
        assert_eq!(frequency(&terminal(false, Some(Some(5))), id), Err(Status::NoUpgrade));
        assert_eq!(frequency(&terminal(false, None), id), Err(Status::GenericInvalid), "a second failure shows nothing");
        assert_eq!(Status::BridgeNotFound.key(), Some("chat.ae2wtlib.NoQuantumBridge"));
        assert_eq!(Status::GenericInvalid.key(), None);
    }

    #[test]
    fn the_singularity_slot_keeps_its_stack_through_a_save() {
        let stack = terminal(true, Some(Some(42)));
        let saved = crate::save::item_from_nbt(&crate::save::item_to_nbt(&stack)).unwrap();
        assert_eq!(singularity(&saved).and_then(|s| id(&s)), Some(42));
        let mut emptied = saved;
        set_singularity(&mut emptied, None);
        assert_eq!(singularity(&emptied), None);
        assert!(has_card(&emptied) && !has_card(&ItemStack::new("ae2:wireless_crafting_terminal", 1)));
    }

    #[test]
    fn the_recharge_never_takes_the_grid_below_half() {
        assert_eq!(recharge_amount(1_599_000.0, 1_600_000.0, 1_600_000.0), Some(800_000.0));
        assert_eq!(recharge_amount(100.0, 1_600_000.0, 1_600_000.0), Some(100.0));
        assert_eq!(recharge_amount(100.0, 800_000.0, 1_600_000.0), None, "at half nothing moves");
        assert_eq!(recharge_amount(0.0, 1_600_000.0, 1_600_000.0), None, "a full battery takes nothing");
    }

    #[test]
    fn the_card_fits_every_terminal_of_the_registry_once() {
        let defs = crate::ae2x::defs::get();
        for item in ["ae2:wireless_crafting_terminal", super::super::PATTERN_ENCODING_TERMINAL, super::super::PATTERN_ACCESS_TERMINAL, super::super::UNIVERSAL_TERMINAL] {
            assert_eq!(defs.upgrade_limit(QUANTUM_BRIDGE_CARD, item), 1, "{item}");
        }
        assert_eq!(defs.upgrade_limit(QUANTUM_BRIDGE_CARD, "ae2:wireless_terminal"), 0, "AE2's own terminal is not one of the registry's");
    }
}
