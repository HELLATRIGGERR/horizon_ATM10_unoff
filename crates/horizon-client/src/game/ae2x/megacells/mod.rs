//! MEGA Cells 4.11.0 in the game (feature `ae2-megacells`). Its cells and
//! components are rows of the definitions registry (`horizon_client::ae2x::
//! megacells`); what needs the game's own code is here.
pub(in crate::game) mod lava;
pub(in crate::game) mod portable_workbench;

pub(in crate::game) use portable_workbench::State;

#[cfg(test)]
mod tests {
    use crate::game::ae2_interactions::{portable_charge_rate, portable_energy, portable_insert_energy};
    use horizon_client::{ae2_upgrade_inventory::Inventory, ae2x::megacells::GREATER_ENERGY_CARD, items::ItemStack};

    /// `MEGAPortableCell.getAEMaxPower` (8 × AE2's) and `getChargeRate` (2 ×)
    /// over the battery the cards set (`AbstractPortableCell.onUpgradesChanged`:
    /// 20,000 × (1 + 8 × multiplier)), and `setAEMaxPower`'s clamp, which is
    /// to that battery before the × 8.
    #[test]
    fn portable_m_cells_hold_eight_times_and_charge_twice_as_fast() {
        horizon_client::ae2x::install();
        let greater = ItemStack::new(GREATER_ENERGY_CARD, 1);
        let mut cell = ItemStack::new("megacells:portable_item_cell_1m", 1);
        assert_eq!(portable_energy(&cell).unwrap(), (0.0, 160_000.0));
        assert_eq!(portable_charge_rate(&cell).unwrap(), 160.0);
        let mut cards = Inventory::read(&cell).unwrap();
        assert!(!cards.insert_one(&ItemStack::new("ae2:energy_card", 1)), "AE2's energy card is not registered on MEGA's cells");
        assert!(cards.insert_one(&greater));
        cards.store(&mut cell).unwrap();
        // 20,000 × 65 × 8, at 80 × 9 × 2.
        assert_eq!(portable_energy(&cell).unwrap(), (0.0, 10_400_000.0));
        assert_eq!(portable_charge_rate(&cell).unwrap(), 1_440.0);
        assert!(cards.insert_one(&greater));
        assert!(!cards.insert_one(&greater), "two at most");
        cards.store(&mut cell).unwrap();
        // 20,000 × 129 × 8, at 80 × 17 × 2.
        assert_eq!(portable_energy(&cell).unwrap(), (0.0, 20_640_000.0));
        assert_eq!(portable_charge_rate(&cell).unwrap(), 2_720.0);
        assert_eq!(portable_insert_energy(&mut cell, 30_000_000.0, false).unwrap(), 20_640_000.0);
        // Both cards out: the battery is 20,000 again and the charge clamps to it, not to 160,000.
        let mut cards = Inventory::read(&cell).unwrap();
        assert!(cards.remove_first().is_some() && cards.remove_first().is_some());
        cards.store(&mut cell).unwrap();
        assert_eq!(portable_energy(&cell).unwrap(), (20_000.0, 160_000.0));
        // A fluid cell alike; AE2's portable cells keep 20,000 at 80.
        assert_eq!(portable_energy(&ItemStack::new("megacells:portable_fluid_cell_256m", 1)).unwrap(), (0.0, 160_000.0));
        assert_eq!(portable_energy(&ItemStack::new("ae2:portable_item_cell_1k", 1)).unwrap(), (0.0, 20_000.0));
        assert_eq!(portable_charge_rate(&ItemStack::new("ae2:portable_fluid_cell_256k", 1)).unwrap(), 80.0);
        assert!(portable_energy(&ItemStack::new("megacells:item_storage_cell_1m", 1)).is_err());
    }
}
