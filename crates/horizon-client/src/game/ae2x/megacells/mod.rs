//! MEGA Cells 4.11.0 in the game (feature `ae2-megacells`). Its cells and
//! components are rows of the definitions registry (`horizon_client::ae2x::
//! megacells`); what needs the game's own code is here.
pub(in crate::game) mod bulk;
pub(in crate::game) mod cell_dock;
pub(in crate::game) mod compression;
pub(in crate::game) mod decompression;
pub(in crate::game) mod lava;
pub(in crate::game) mod portable_workbench;

use horizon_client::{ae2x::megacells as rows, items::ItemStack};
pub(in crate::game) use portable_workbench::State;

/// `MEGAPatternProviderPart.MODEL_BASE` and `MEGAInterfacePart.MODEL_BASE`:
/// the parts' base models, over AE2's interface status models; the Cell
/// Dock's `MODEL`.
pub(in crate::game) fn part_model(id: &str) -> Option<&'static str> {
    match id {
        rows::MEGA_PATTERN_PROVIDER_PART => Some("megacells:part/mega_pattern_provider"),
        rows::MEGA_INTERFACE_PART => Some("megacells:part/mega_interface"),
        // CellDockPart.MODEL, its static model; the cell is drawn on it.
        rows::CELL_DOCK => Some("megacells:part/cell_dock"),
        // DecompressionModulePart.MODEL.
        rows::DECOMPRESSION_MODULE => Some("megacells:part/decompression_module"),
        _ => None,
    }
}

/// `AEItemDefinitionFilter.allowInsert` on a MEGA provider's pattern
/// inventory (`MEGAPatternProviderBlock.createLogic`): the processing pattern
/// alone. None for a machine of no MEGA provider.
pub(in crate::game) fn pattern_allowed(machine_id: &str, item: &ItemStack) -> Option<bool> {
    rows::pattern_filter(machine_id).map(|only| item.item.as_ref() == only)
}

/// `MEGAPatternProviderBlockItem.addCheckedInformation` and
/// `MEGAPatternProviderPartItem.appendHoverText`: "Supports processing
/// patterns only.", in `NOTICE` (0xffde7d, italic).
pub(in crate::game) fn hover_lines(id: &str, translate: &dyn Fn(&str) -> Option<String>) -> Vec<horizon_client::text::Text> {
    let Some((key, colour)) = rows::hover_line(id) else { return Vec::new() };
    let text = translate(key).unwrap_or_else(|| key.to_owned());
    vec![horizon_client::text::Text::literal(text).with_color(colour).italic()]
}

#[cfg(test)]
mod tests {
    use super::{hover_lines, part_model, pattern_allowed};
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

    /// The MEGA providers take processing patterns alone; the parts draw
    /// MEGA's base models; both provider items carry the notice.
    #[test]
    fn mega_providers_take_processing_patterns_only() {
        let processing = ItemStack::new("ae2:processing_pattern", 1);
        for id in ["megacells:mega_pattern_provider", "megacells:cable_mega_pattern_provider"] {
            assert_eq!(pattern_allowed(id, &processing), Some(true), "{id}");
            for refused in ["ae2:crafting_pattern", "ae2:smithing_table_pattern", "ae2:stonecutting_pattern", "ae2:blank_pattern"] {
                assert_eq!(pattern_allowed(id, &ItemStack::new(refused, 1)), Some(false), "{refused} in {id}");
            }
        }
        assert_eq!(pattern_allowed("ae2:pattern_provider", &ItemStack::new("ae2:crafting_pattern", 1)), None);
        assert_eq!(pattern_allowed("megacells:mega_interface", &processing), None);
        assert_eq!(part_model("megacells:cable_mega_pattern_provider"), Some("megacells:part/mega_pattern_provider"));
        assert_eq!(part_model("megacells:cable_mega_interface"), Some("megacells:part/mega_interface"));
        assert_eq!(part_model("ae2:cable_interface"), None);
        let lang = |key: &str| (key == "gui.tooltips.megacells.ProcessingOnly").then(|| "Supports processing patterns only.".to_owned());
        for id in ["megacells:mega_pattern_provider", "megacells:cable_mega_pattern_provider"] {
            let lines = hover_lines(id, &lang);
            assert_eq!(lines.len(), 1, "{id}");
            assert_eq!(lines[0].plain(), "Supports processing patterns only.");
            let style = lines[0].spans[0].style.clone();
            assert_eq!((style.color, style.italic), (Some(0xffde7d), Some(true)));
        }
        assert!(hover_lines("megacells:cable_mega_interface", &lang).is_empty());
    }
}
