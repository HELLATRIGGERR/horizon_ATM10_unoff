//! MEGA's Bulk Item Cell in the game (`BulkCellItem` and its `ICellHandler`,
//! `BulkCellInventory`): a storage cell of its own handler that the Drive,
//! the ME Chest, the Cell Dock and the IO Port mount as they do AE2's cells,
//! with its LED (`getStatus`), its tooltip (`appendHoverText`,
//! `getTooltipImage`) and the chains it reads (`CompressionService.getChain`).
//! The arithmetic is `ae2_megacells_core::BulkCell`. Derived from MEGA Cells
//! 4.11.0, https://github.com/62832/MEGACells tag 4.11.0, LGPL-3.0; see
//! `ae2/addons/megacells/`.
use crate::game::{Game, ae2x::HandlerCell};
use ae2_cell_core::{Action, ItemKey};
use ae2_megacells_core::BulkCell;
use horizon_client::{ae2x::megacells as rows, items::ItemStack, text::Text};

/// `BulkCellInventory` over its stack, as the storage code holds a cell.
pub(in crate::game) struct BulkStack {
    pub(in crate::game) cell: BulkCell,
}

impl HandlerCell for BulkStack {
    fn available_stacks(&mut self) -> Vec<(ItemKey, u64)> {
        self.cell.available_stacks()
    }
    fn insert(&mut self, key: &ItemKey, amount: u64, action: Action) -> u64 {
        self.cell.insert(key, amount, action)
    }
    fn extract(&mut self, key: &ItemKey, amount: u64, action: Action) -> u64 {
        self.cell.extract(key, amount, action)
    }
    fn is_preferred_storage_for(&self, key: &ItemKey) -> bool {
        self.cell.is_preferred_storage_for(key)
    }
    fn store(&self, stack: &mut ItemStack) -> Result<(), String> {
        rows::store_bulk_data(stack, self.cell.data())
    }
}

/// `Tooltips.getAmount(long)`: up to 9,999 as it is, then three significant
/// decimals with k, M, G, T, P or E.
fn tooltip_amount(amount: u64) -> String {
    const UNITS: [&str; 6] = ["k", "M", "G", "T", "P", "E"];
    if amount < 10_000 {
        return amount.to_string();
    }
    let mut i = 0;
    let mut num = 1000u64;
    while i + 1 < UNITS.len() && amount / num >= 1000 {
        i += 1;
        num *= 1000;
    }
    format!("{}{}", crate::game::ae2_widgets::tooltip_number(amount as f64 / num as f64), UNITS[i])
}

/// `BulkCellItem.appendHoverText`: what the cell holds and how much, its
/// partition, its compression, its trace units and its cutoff.
fn hover_lines(inv: &BulkCell, key_name: &dyn Fn(&ItemKey) -> Text) -> Vec<Text> {
    use horizon_client::text::{GRAY, GREEN, RED, YELLOW};
    let tr = |key: &str, args: &[Text]| Text::translate(&format!("gui.tooltips.megacells.{key}"), args);
    let mut lines = Vec::new();
    let stored = inv.stored_item();
    let filter = inv.filter_item();
    if let Some(stored) = stored {
        lines.push(tr("Contains", &[key_name(stored)]).with_color(GRAY));
        let quantity = inv.stored_quantity();
        let amount = if quantity < i64::MAX as u64 { Text::literal(tooltip_amount(quantity)) } else { tr("ALot", &[]) };
        lines.push(tr("Quantity", &[amount.with_color(NUMBER)]).with_color(GRAY));
    } else {
        lines.push(tr("Empty", &[]).with_color(GRAY));
    }
    match (filter, stored) {
        (Some(filter), None) => lines.push(tr("PartitionedFor", &[key_name(filter)]).with_color(GRAY)),
        (Some(filter), Some(stored)) if stored != filter => lines.push(tr("MismatchedFilter", &[key_name(filter)]).with_color(DARK_RED)),
        (Some(_), Some(_)) => {}
        (None, Some(_)) => lines.push(tr("MismatchedFilter", &[tr("Empty", &[])]).with_color(DARK_RED)),
        (None, None) => lines.push(tr("NotPartitioned", &[]).with_color(GRAY)),
    }
    let enabled = inv.compression_enabled();
    let state = if enabled { tr("Enabled", &[]).with_color(GREEN) } else { tr("Disabled", &[]).with_color(RED) };
    lines.push(tr("Compression", &[state]).with_color(GRAY));
    let trace = inv.trace_units();
    if trace > 0 {
        let line = if enabled {
            let lowest = inv.lowest_variant().map_or_else(Text::empty, key_name);
            tr("TraceUnits", &[Text::literal(tooltip_amount(trace)).with_color(NUMBER), lowest])
        } else {
            tr("ContainsTraceUnits", &[])
        };
        lines.push(line.with_color(YELLOW));
    }
    if enabled {
        let cutoff = inv.cutoff_item();
        if cutoff != inv.highest_variant() {
            lines.push(tr("Cutoff", &[cutoff.map_or_else(|| Text::translate("block.minecraft.air", &[]), key_name)]).with_color(GRAY));
        }
    }
    lines
}

/// `Tooltips.NUMBER_TEXT`: 0x886eff.
const NUMBER: u32 = 0x886eff;
/// `ChatFormatting.DARK_RED`.
const DARK_RED: u32 = 0xAA0000;

impl Game<'_> {
    /// `BulkCellItem.Handler.getCellInventory`: the cell of a Bulk Item Cell
    /// stack, its stored item's id checked against the game's items.
    pub(in crate::game) fn megacells_bulk_cell(&self, stack: &ItemStack) -> Result<BulkStack, String> {
        let chains = self.megacells_chains();
        let known = |id: &str, fluid: bool| self.ae2_known_cell_key(id, fluid);
        Ok(BulkStack { cell: rows::bulk_cell(stack, &chains, &known)? })
    }

    /// `BulkCellItem.appendHoverText` of a stack; `name` gives an item's hover name.
    pub(in crate::game) fn megacells_bulk_hover_lines(&self, stack: &ItemStack, name: &dyn Fn(&ItemStack) -> Text) -> Vec<Text> {
        let Ok(BulkStack { cell }) = self.megacells_bulk_cell(stack) else { return Vec::new() };
        hover_lines(&cell, &|key| horizon_client::ae2_cell_codec::prototype(key).map_or_else(|_| Text::literal(key.item_id.clone()), |item| name(&item)))
    }

    /// `BulkCellItem.getTooltipImage`: the Compression Card when compressing,
    /// and the stored item with its quantity, or the filter at 0.
    pub(in crate::game) fn megacells_bulk_preview(&self, stack: &ItemStack, show_upgrades: bool, show_content: bool) -> Option<(Vec<ItemStack>, Vec<(ItemKey, u64)>)> {
        let BulkStack { cell } = self.megacells_bulk_cell(stack).ok()?;
        let mut upgrades = Vec::new();
        if show_upgrades && cell.compression_enabled() {
            upgrades.push(ItemStack::new(rows::COMPRESSION_CARD, 1));
        }
        let mut content = Vec::new();
        if show_content {
            if let Some(stored) = cell.stored_item() {
                content.push((stored.clone(), cell.stored_quantity()));
            } else if let Some(filter) = cell.filter_item() {
                content.push((filter.clone(), 0));
            }
        }
        Some((upgrades, content))
    }
}

#[cfg(test)]
pub(in crate::game) mod tests {
    use super::{hover_lines, tooltip_amount};
    use ae2_cell_core::{Action, ItemKey};
    use ae2_megacells_core::{Chains, CraftingRecipe, load_recipes};
    use horizon_client::{ae2x::megacells as rows, items::ItemStack, text::Text};
    use horizon_world::nbt::Tag;

    pub(in crate::game) fn key(id: &str) -> ItemKey {
        horizon_client::ae2_cell_codec::key(&ItemStack::new(id, 1)).unwrap()
    }
    /// Vanilla's iron chain, from its four recipes.
    pub(in crate::game) fn iron() -> Chains {
        let one = |id: &str| vec![key(id)];
        load_recipes(
            &[
                CraftingRecipe { id: "iron_block".into(), ingredients: vec![one("minecraft:iron_ingot"); 9], result: key("minecraft:iron_block"), count: 1 },
                CraftingRecipe { id: "iron_ingot_from_iron_block".into(), ingredients: vec![one("minecraft:iron_block")], result: key("minecraft:iron_ingot"), count: 9 },
                CraftingRecipe { id: "iron_ingot_from_nuggets".into(), ingredients: vec![one("minecraft:iron_nugget"); 9], result: key("minecraft:iron_ingot"), count: 1 },
                CraftingRecipe { id: "iron_nugget".into(), ingredients: vec![one("minecraft:iron_ingot")], result: key("minecraft:iron_nugget"), count: 9 },
            ],
            &|_| None,
        )
    }
    /// A Bulk Item Cell partitioned for `filter` (its `ae2:storage_cell_config_inv`).
    pub(in crate::game) fn bulk(filter: Option<&str>) -> ItemStack {
        let mut stack = ItemStack::new(rows::BULK_ITEM_CELL, 1);
        if let Some(filter) = filter {
            let entry = Tag::compound([("#t", Tag::String("ae2:i".into())), ("id", Tag::String(filter.into())), ("#", Tag::Long(0))]);
            stack.data_mut().set_other("ae2:storage_cell_config_inv", Some(Tag::List(vec![entry])));
        }
        stack
    }

    /// What the cell stores goes on its stack as MEGA's components, which the
    /// save and the command codec keep, and comes back; an empty cell has none.
    #[test]
    fn the_bulk_cell_keeps_its_count_in_megas_components() {
        horizon_client::ae2x::install();
        let known = |_: &str, _: bool| true;
        let cobble = key("minecraft:cobblestone");
        let mut stack = bulk(Some("minecraft:cobblestone"));
        let mut cell = rows::bulk_cell(&stack, &Chains::default(), &known).unwrap();
        assert_eq!(cell.filter_item(), Some(&cobble));
        for _ in 0..3 {
            assert_eq!(cell.insert(&cobble, i64::MAX as u64, Action::Modulate), i64::MAX as u64);
        }
        rows::store_bulk_data(&mut stack, cell.data()).unwrap();
        let saved = horizon_client::save::item_from_nbt(&horizon_client::save::item_to_nbt(&stack)).unwrap();
        let data = saved.data().unwrap();
        assert_eq!(data.other(rows::BULK_UNIT_COUNT), Some(&Tag::String("27670116110564327421".into())));
        assert_eq!(data.other(rows::BULK_UNIT_FACTOR), Some(&Tag::String("1".into())));
        assert_eq!(data.other(rows::BULK_ITEM), Some(&Tag::compound([("#t", Tag::String("ae2:i".into())), ("id", Tag::String("minecraft:cobblestone".into()))])));
        for id in [rows::BULK_ITEM, rows::BULK_UNIT_COUNT, rows::BULK_UNIT_FACTOR] {
            horizon_client::ae2_components::validate(id, data.other(id).unwrap()).unwrap();
        }
        assert!(horizon_client::ae2_components::validate(rows::BULK_UNIT_COUNT, &Tag::String("12a".into())).is_err());
        let mut cell = rows::bulk_cell(&saved, &Chains::default(), &known).unwrap();
        assert_eq!(cell.stored_quantity(), i64::MAX as u64);
        for _ in 0..3 {
            assert_eq!(cell.extract(&cobble, i64::MAX as u64, Action::Modulate), i64::MAX as u64);
        }
        let mut emptied = saved.clone();
        rows::store_bulk_data(&mut emptied, cell.data()).unwrap();
        for id in [rows::BULK_ITEM, rows::BULK_UNIT_COUNT, rows::BULK_UNIT_FACTOR, rows::COMPRESSION_CUTOFF] {
            assert!(emptied.data().unwrap().other(id).is_none(), "{id} goes with the last unit");
        }
    }

    /// `StorageCells.isCellHandled`, `getIdleDrain` 5, one filter, one card
    /// slot, its drive model; only an empty, unfiltered one fits in a cell.
    #[test]
    fn the_bulk_cell_is_a_cell_of_its_own_handler() {
        horizon_client::ae2x::install();
        let defs = horizon_client::ae2x::defs::get();
        let row = defs.handler_cell(rows::BULK_ITEM_CELL).unwrap();
        assert_eq!((row.idle_drain, row.config_slots), (5.0, 1));
        assert_eq!(defs.drive_model(rows::BULK_ITEM_CELL), Some("megacells:block/drive/cells/bulk_item_cell"));
        assert_eq!(defs.upgrade_slots(rows::BULK_ITEM_CELL), Some(1));
        assert!(defs.cell(rows::BULK_ITEM_CELL).is_none(), "no basic cell");
        assert!(horizon_client::ae2_cell_codec::is_cell(rows::BULK_ITEM_CELL));
        assert_eq!(horizon_client::ae2_cell_codec::idle_drain(rows::BULK_ITEM_CELL), Some(5.0));
        assert!(crate::game::ae2_storage_devices::is_valid_storage_cell(&bulk(None)));
        assert!(!crate::game::ae2_storage_devices::is_valid_storage_cell(&ItemStack::new(rows::BULK_ITEM_CELL, 2)));
        assert!(horizon_client::ae2_cell_codec::can_store_item(&bulk(None)));
        assert!(!horizon_client::ae2_cell_codec::can_store_item(&bulk(Some("minecraft:stone"))), "a partitioned bulk cell stays out of cells");
        let mut full = bulk(None);
        let data = ae2_megacells_core::BulkData { item: Some(key("minecraft:stone")), unit_count: Some(1.into()), unit_factor: None, cutoff: None };
        rows::store_bulk_data(&mut full, &data).unwrap();
        assert!(!horizon_client::ae2_cell_codec::can_store_item(&full));
        assert_eq!(horizon_client::ae2x::handler_cell_state(&full), Some(4), "stone with no filter: a mismatch, FULL");
        assert_eq!(horizon_client::ae2x::handler_cell_state(&bulk(Some("minecraft:stone"))), Some(1));
    }

    fn lines(cell: &ae2_megacells_core::BulkCell) -> Vec<(String, Option<u32>)> {
        hover_lines(cell, &|key| Text::literal(key.item_id.clone()))
            .iter()
            .map(|line| (line.translation_key().unwrap_or_default().trim_start_matches("gui.tooltips.megacells.").to_owned(), line.spans.first().and_then(|span| span.style.color)))
            .collect()
    }

    /// `appendHoverText`'s lines, in order and colour.
    #[test]
    fn the_tooltip_follows_append_hover_text() {
        use horizon_client::text::{GRAY, YELLOW};
        let known = |_: &str, _: bool| true;
        let empty = rows::bulk_cell(&bulk(None), &Chains::default(), &known).unwrap();
        let expected: Vec<(String, Option<u32>)> = ["Empty", "NotPartitioned", "Compression"].iter().map(|key| (key.to_string(), Some(GRAY))).collect();
        assert_eq!(lines(&empty), expected);
        let mut cell = rows::bulk_cell(&bulk(Some("minecraft:iron_ingot")), &iron(), &known).unwrap();
        assert_eq!(lines(&cell)[1].0, "PartitionedFor");
        cell.insert(&key("minecraft:iron_ingot"), 12_345, Action::Modulate);
        // Four units more than whole ingots, and a filter of diamonds: a mismatch with trace units.
        let mut data = cell.data().clone();
        data.unit_count = Some(data.unit_count.unwrap().add(&4u64.into()));
        let mut stack = bulk(Some("minecraft:diamond"));
        rows::store_bulk_data(&mut stack, &data).unwrap();
        let mismatched = rows::bulk_cell(&stack, &iron(), &known).unwrap();
        let shown = lines(&mismatched);
        assert_eq!(shown.iter().map(|(key, _)| key.as_str()).collect::<Vec<_>>(), ["Contains", "Quantity", "MismatchedFilter", "Compression", "ContainsTraceUnits"]);
        assert_eq!(shown[2].1, Some(super::DARK_RED));
        assert_eq!(shown[4].1, Some(YELLOW));
        let quantity = hover_lines(&mismatched, &|key| Text::literal(key.item_id.clone()))[1].component().to_string();
        assert!(quantity.contains("\"12.35k\""), "two decimals under 100: {quantity}");
    }

    /// `Tooltips.getAmount(long)` as AE2 prints it.
    #[test]
    fn amounts_print_as_ae2_tooltips_do() {
        assert_eq!(tooltip_amount(9_999), "9999");
        assert_eq!(tooltip_amount(10_000), "10k");
        assert_eq!(tooltip_amount(123_456), "123.5k");
        assert_eq!(tooltip_amount(4_398_046_511_104), "4.398T");
        assert_eq!(tooltip_amount(i64::MAX as u64 - 1), "9.223E");
    }
}
