//! MEGA's Bulk Item Cell: `BulkCellInventory`, a `StorageCell` of one item
//! type (the cell's one filter) in a count of units with no limit, and with
//! a Compression Card the whole compression chain of that item.
use crate::big::Big;
use crate::compression::{Chains, CompressionChain, Pattern, STACK_LIMIT};
use ae2_cell_core::{Action, ItemKey};

/// `BulkCellInventory.getIdleDrain`: 5 AE/t in a drive or chest.
pub const IDLE_DRAIN: f64 = 5.0;

/// The cell's own components on its stack (`MEGAComponents`): the stored
/// item (`megacells:bulk_item`), its unit count and the unit factor the
/// count was recorded at (`bulk_unit_count`, `bulk_unit_factor`, decimal
/// strings), and the compression cutoff (`compression_cutoff`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BulkData {
    pub item: Option<ItemKey>,
    pub unit_count: Option<Big>,
    pub unit_factor: Option<Big>,
    pub cutoff: Option<i32>,
}

/// `CellState` as a bulk cell reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellState {
    Empty,
    NotEmpty,
    Full,
}

/// `BulkCellInventory`, read from a stack's [`BulkData`], its filter
/// (`getConfigInventory(is).getKey(0)`) and whether a Compression Card is
/// installed. What it changes on the stack is in [`BulkCell::data`], with
/// [`BulkCell::changed`] set, for the caller to write back.
#[derive(Clone, Debug)]
pub struct BulkCell {
    data: BulkData,
    changed: bool,
    stored: Option<ItemKey>,
    filter: Option<ItemKey>,
    compression_enabled: bool,
    chain: CompressionChain,
    /// `CompressionService.getChain(filterItem)`, which `extract` falls back
    /// to once the cell is empty.
    filter_chain: CompressionChain,
    unit_count: Big,
    unit_factor: Big,
    cutoff: usize,
    compressed_stacks: Vec<(ItemKey, u64)>,
    needs_stack_update: bool,
}

/// `KeyCounter.add`: amounts of one key add up.
fn add(out: &mut Vec<(ItemKey, u64)>, key: &ItemKey, amount: u64) {
    match out.iter_mut().find(|(k, _)| k == key) {
        Some(entry) => entry.1 = entry.1.saturating_add(amount),
        None => out.push((key.clone(), amount)),
    }
}

impl BulkCell {
    /// `new BulkCellInventory(stack, container)`: the chain of the stored
    /// item (or of the filter), the count rescaled when the item's unit
    /// factor changed since it was recorded (and the stack updated), the
    /// cutoff dropped from an empty unfiltered cell and clamped to the chain.
    pub fn new(mut data: BulkData, filter: Option<ItemKey>, compression_enabled: bool, chains: &Chains) -> Self {
        let mut changed = false;
        let stored = data.item.clone();
        let mut unit_count = data.unit_count.clone().unwrap_or_default();
        let determining = stored.clone().or_else(|| filter.clone());
        let chain = chains.chain(determining.as_ref()).cloned().unwrap_or_default();
        let filter_chain = chains.chain(filter.as_ref()).cloned().unwrap_or_default();
        let unit_factor = chain.unit_factor(determining.as_ref());
        let recorded = data.unit_factor.clone().unwrap_or_else(|| unit_factor.clone());
        if unit_factor != recorded {
            unit_count = unit_count.mul(&unit_factor).div(&recorded);
            data.unit_count = Some(unit_count.clone());
            data.unit_factor = Some(unit_factor.clone());
            changed = true;
        }
        if determining.is_none() && data.cutoff.is_some() {
            data.cutoff = None;
            changed = true;
        }
        let max_cutoff = chain.size().saturating_sub(1) as i32;
        let recorded_cutoff = data.cutoff.unwrap_or(max_cutoff);
        let cutoff = if recorded_cutoff < 0 { max_cutoff } else { recorded_cutoff.min(max_cutoff) } as usize;
        let compressed_stacks = chain.init_stacks(&unit_count, cutoff, determining.as_ref());
        Self {
            data,
            changed,
            stored,
            filter,
            compression_enabled,
            chain,
            filter_chain,
            unit_count,
            unit_factor,
            cutoff,
            compressed_stacks,
            needs_stack_update: false,
        }
    }
    /// The stack's components as the cell left them.
    pub fn data(&self) -> &BulkData {
        &self.data
    }
    /// Whether the cell changed its stack since it was read.
    pub fn changed(&self) -> bool {
        self.changed
    }
    /// `getStatus`: empty with no item or no units; full when the filter no
    /// longer matches what it holds; not empty otherwise.
    pub fn status(&mut self) -> CellState {
        if self.stored.is_none() || self.unit_count.is_zero() {
            return CellState::Empty;
        }
        if self.is_filter_mismatched() {
            return CellState::Full;
        }
        CellState::NotEmpty
    }
    /// `getStoredItem`.
    pub fn stored_item(&self) -> Option<&ItemKey> {
        self.stored.as_ref()
    }
    /// `getStoredQuantity`: whole stored items, at most `Long.MAX_VALUE`.
    pub fn stored_quantity(&self) -> u64 {
        self.unit_count.div(&self.unit_factor).clamp_to(i64::MAX as u64)
    }
    /// `getFilterItem`.
    pub fn filter_item(&self) -> Option<&ItemKey> {
        self.filter.as_ref()
    }
    pub fn unit_count(&self) -> &Big {
        &self.unit_count
    }
    /// `isFilterMismatched`: the stored item is not the filter's. When the
    /// filter is another variant of its chain, the cell takes the filter as
    /// its stored item (its units unchanged) and matches again.
    fn is_filter_mismatched(&mut self) -> bool {
        let Some(stored) = &self.stored else { return false };
        if Some(stored) == self.filter.as_ref() {
            return false;
        }
        let Some(filter) = self.filter.clone() else { return true };
        if self.chain.contains_variant(&filter) {
            self.unit_factor = self.chain.unit_factor(Some(&filter));
            self.stored = Some(filter);
            self.save_changes();
            return false;
        }
        true
    }
    /// `isCompressionEnabled`: a Compression Card is installed.
    pub fn compression_enabled(&self) -> bool {
        self.compression_enabled
    }
    /// `hasCompressionChain`.
    pub fn has_compression_chain(&self) -> bool {
        !self.chain.is_empty()
    }
    pub fn chain(&self) -> &CompressionChain {
        &self.chain
    }
    /// `getTraceUnits`: the units that make no whole stored item.
    pub fn trace_units(&self) -> u64 {
        self.unit_count.rem(&self.unit_factor).clamp_to(i64::MAX as u64)
    }
    /// `getDecompressionPatterns`: none without a filter, a card or a chain,
    /// or with a mismatched filter; else the chain's at the cutoff.
    pub fn decompression_patterns(&mut self) -> Vec<Pattern> {
        if self.filter.is_none() || !self.compression_enabled || !self.has_compression_chain() || self.is_filter_mismatched() {
            return Vec::new();
        }
        self.chain.decompression_patterns(self.cutoff)
    }
    /// `insert`: the filter's item, or with a Compression Card any variant of
    /// its chain, all of it, as units; nothing while the filter mismatches.
    pub fn insert(&mut self, what: &ItemKey, amount: u64, mode: Action) -> u64 {
        if amount == 0 || self.is_filter_mismatched() {
            return 0;
        }
        if Some(what) != self.filter.as_ref() && (!self.compression_enabled || !self.chain.contains_variant(what)) {
            return 0;
        }
        let factor = self.chain.unit_factor(Some(what));
        let units = Big::from(amount).mul(&factor);
        if mode == Action::Modulate {
            if self.stored.is_none() {
                self.stored = self.filter.clone();
            }
            self.unit_count = self.unit_count.add(&units);
            self.save_changes();
            self.needs_stack_update = true;
        }
        amount
    }
    /// `extract`: the stored item, or with a Compression Card any variant of
    /// its chain, while units last. A request past the units takes them all,
    /// the trace units with them, and gives the whole items they make.
    pub fn extract(&mut self, what: &ItemKey, mut amount: u64, mode: Action) -> u64 {
        if self.stored.is_none() || self.unit_count.is_zero() {
            return 0;
        }
        if !self.chain.contains_variant(what) && Some(what) != self.stored.as_ref() {
            return 0;
        }
        if self.is_filter_mismatched() {
            let listed = self.available_stacks().into_iter().find(|(k, _)| k == what).map_or(0, |(_, n)| n);
            amount = amount.min(listed);
        } else if !self.compression_enabled && Some(what) != self.stored.as_ref() {
            return 0;
        }
        let factor = self.chain.unit_factor(Some(what));
        let units = Big::from(amount).mul(&factor).min(self.unit_count.clone());
        if mode == Action::Modulate {
            self.unit_count = self.unit_count.saturating_sub(&units);
            if self.unit_count.is_zero() {
                self.stored = None;
                if self.chain != self.filter_chain {
                    self.chain = self.filter_chain.clone();
                }
            }
            self.save_changes();
            self.needs_stack_update = true;
        }
        units.div(&factor).clamp_to(i64::MAX as u64)
    }
    /// `saveChanges` with `persist` right after: the stored item, its count
    /// and factor on the stack, or none of them once empty.
    fn save_changes(&mut self) {
        match &self.stored {
            Some(item) if !self.unit_count.is_zero() => {
                self.data.item = Some(item.clone());
                self.data.unit_count = Some(self.unit_count.clone());
                self.data.unit_factor = Some(self.unit_factor.clone());
            }
            _ => {
                self.data.item = None;
                self.data.unit_count = None;
                self.data.unit_factor = None;
            }
        }
        self.changed = true;
    }
    /// `switchCompressionCutoff`: one variant down (or up, `backwards`), as
    /// Java's `%` leaves it: below the smallest it records -1, which the next
    /// read takes for the largest. The menu reads the cell again afterwards.
    pub fn switch_compression_cutoff(&mut self, backwards: bool) {
        if !self.has_compression_chain() {
            return;
        }
        let next = (self.cutoff as i32 + if backwards { 1 } else { -1 }) % self.chain.size() as i32;
        self.data.cutoff = Some(next);
        self.changed = true;
    }
    /// `getCutoffItem`: the variant the cell lists its count at.
    pub fn cutoff_item(&self) -> Option<&ItemKey> {
        self.has_compression_chain().then(|| &self.chain.item(self.cutoff).key)
    }
    /// `getHighestVariant`.
    pub fn highest_variant(&self) -> Option<&ItemKey> {
        self.chain.variants().last().map(|variant| &variant.key)
    }
    /// `getLowestVariant`.
    pub fn lowest_variant(&self) -> Option<&ItemKey> {
        self.chain.variants().first().map(|variant| &variant.key)
    }
    /// `getAvailableStacks`: with a Compression Card, the chain's stacks at
    /// the cutoff; without, the stored item (at most 2^42), and while the
    /// filter mismatches, the variants under it too.
    pub fn available_stacks(&mut self) -> Vec<(ItemKey, u64)> {
        if self.needs_stack_update {
            let determining = self.stored.clone().or_else(|| self.filter.clone());
            self.compressed_stacks = self.chain.init_stacks(&self.unit_count, self.cutoff, determining.as_ref());
            self.needs_stack_update = false;
        }
        let mut out = Vec::new();
        let Some(stored) = self.stored.clone() else { return out };
        if self.compression_enabled {
            for (key, amount) in &self.compressed_stacks {
                add(&mut out, key, *amount);
            }
        } else {
            add(&mut out, &stored, self.unit_count.div(&self.unit_factor).clamp_to(STACK_LIMIT));
            if self.is_filter_mismatched() {
                let stored = self.stored.clone();
                for (key, amount) in self.compressed_stacks.iter().take_while(|(key, _)| Some(key) != stored.as_ref()) {
                    add(&mut out, key, *amount);
                }
            }
        }
        out
    }
    /// `isPreferredStorageFor`: its stored item, its filter or a variant of its chain.
    pub fn is_preferred_storage_for(&self, what: &ItemKey) -> bool {
        Some(what) == self.stored.as_ref() || Some(what) == self.filter.as_ref() || self.chain.contains_variant(what)
    }
    /// `canFitInsideCell`: an empty, unfiltered bulk cell may go into another cell.
    pub fn can_fit_inside_cell(&self) -> bool {
        self.filter.is_none() && self.stored.is_none() && self.unit_count.is_zero()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compression::{Variant, load_recipes, CraftingRecipe};

    fn key(id: &str) -> ItemKey {
        ItemKey { item_id: format!("minecraft:{id}"), components: Vec::new() }
    }
    /// Vanilla's iron chain from its four recipes.
    fn iron() -> Chains {
        let one = |id: &str| vec![key(id)];
        let recipes = vec![
            CraftingRecipe { id: "iron_block".into(), ingredients: vec![one("iron_ingot"); 9], result: key("iron_block"), count: 1 },
            CraftingRecipe { id: "iron_ingot_from_iron_block".into(), ingredients: vec![one("iron_block")], result: key("iron_ingot"), count: 9 },
            CraftingRecipe { id: "iron_ingot_from_nuggets".into(), ingredients: vec![one("iron_nugget"); 9], result: key("iron_ingot"), count: 1 },
            CraftingRecipe { id: "iron_nugget".into(), ingredients: vec![one("iron_ingot")], result: key("iron_nugget"), count: 9 },
        ];
        load_recipes(&recipes, &|_| None)
    }
    fn cell(data: &BulkData, filter: &str, compression: bool) -> BulkCell {
        BulkCell::new(data.clone(), Some(key(filter)), compression, &iron())
    }

    /// An unfiltered cell takes nothing; a filtered one takes its item and
    /// nothing else, without limit, and gives it back.
    #[test]
    fn one_filtered_type_with_no_limit() {
        let mut empty = BulkCell::new(BulkData::default(), None, false, &iron());
        assert_eq!(empty.insert(&key("diamond"), 64, Action::Modulate), 0);
        assert!(empty.can_fit_inside_cell());
        assert_eq!(empty.status(), CellState::Empty);
        let mut diamonds = cell(&BulkData::default(), "diamond", false);
        assert!(!diamonds.can_fit_inside_cell(), "a filtered cell does not go into another cell");
        assert_eq!(diamonds.insert(&key("emerald"), 5, Action::Modulate), 0);
        assert_eq!(diamonds.insert(&key("diamond"), i64::MAX as u64, Action::Simulate), i64::MAX as u64);
        assert!(!diamonds.changed(), "a simulation changes nothing");
        for _ in 0..4 {
            assert_eq!(diamonds.insert(&key("diamond"), i64::MAX as u64, Action::Modulate), i64::MAX as u64);
        }
        // 4 × (2^63 - 1) units: past a long, listed at 2^42.
        assert_eq!(diamonds.data().unit_count.as_ref().unwrap().to_string(), "36893488147419103228");
        assert_eq!(diamonds.data().item, Some(key("diamond")));
        assert_eq!(diamonds.stored_quantity(), i64::MAX as u64);
        assert_eq!(diamonds.available_stacks(), vec![(key("diamond"), STACK_LIMIT)]);
        assert_eq!(diamonds.status(), CellState::NotEmpty);
        let mut again = BulkCell::new(diamonds.data().clone(), Some(key("diamond")), false, &iron());
        assert_eq!(again.extract(&key("diamond"), 1_000, Action::Modulate), 1_000);
        assert_eq!(again.data().unit_count.as_ref().unwrap().to_string(), "36893488147419102228");
        assert_eq!(again.extract(&key("iron_ingot"), 1, Action::Modulate), 0);
    }

    /// Iron ingots on a cell filtered for ingots are 9 units each; with a
    /// Compression Card the cell takes and gives nuggets and blocks too, and
    /// lists the chain at its cutoff; without, ingots only.
    #[test]
    fn compression_takes_and_gives_the_whole_chain() {
        let mut ingots = cell(&BulkData::default(), "iron_ingot", false);
        assert_eq!(ingots.insert(&key("iron_block"), 1, Action::Modulate), 0, "no card, no blocks");
        assert_eq!(ingots.insert(&key("iron_ingot"), 10, Action::Modulate), 10);
        assert_eq!(ingots.data().unit_count, Some(Big::from(90)));
        assert_eq!(ingots.data().unit_factor, Some(Big::from(9)));
        assert_eq!(ingots.extract(&key("iron_nugget"), 9, Action::Simulate), 0);
        assert_eq!(ingots.available_stacks(), vec![(key("iron_ingot"), 10)]);
        let mut compressing = cell(ingots.data(), "iron_ingot", true);
        assert_eq!(compressing.insert(&key("iron_nugget"), 3, Action::Modulate), 3);
        assert_eq!(compressing.insert(&key("iron_block"), 1, Action::Modulate), 1);
        // 90 + 3 + 81 = 174 units: 2 blocks, 1 ingot, 3 nuggets at the
        // cutoff (the largest variant when none is recorded).
        assert_eq!(compressing.available_stacks(), vec![(key("iron_nugget"), 3), (key("iron_ingot"), 1), (key("iron_block"), 2)]);
        assert_eq!((compressing.stored_quantity(), compressing.trace_units()), (19, 3));
        assert_eq!(compressing.extract(&key("iron_nugget"), 20, Action::Modulate), 20);
        assert_eq!(compressing.data().unit_count, Some(Big::from(154)));
        // A block more than the units: all of them go, the trace with them.
        assert_eq!(compressing.extract(&key("iron_block"), 3, Action::Modulate), 1);
        assert_eq!(compressing.data(), &BulkData::default());
        assert_eq!(compressing.status(), CellState::Empty);
    }

    /// The cutoff lists the chain at another variant and changes the
    /// patterns; switching wraps as MEGA's `%` does.
    #[test]
    fn the_cutoff_moves_and_wraps() {
        let mut data = BulkData { item: Some(key("iron_ingot")), unit_count: Some(Big::from(100)), unit_factor: Some(Big::from(9)), cutoff: None };
        let mut iron = cell(&data, "iron_ingot", true);
        assert_eq!(iron.cutoff_item(), Some(&key("iron_block")));
        assert_eq!(iron.highest_variant(), Some(&key("iron_block")));
        assert_eq!(iron.lowest_variant(), Some(&key("iron_nugget")));
        assert_eq!(iron.decompression_patterns().len(), 2);
        let mut seen = Vec::new();
        for _ in 0..4 {
            iron.switch_compression_cutoff(false);
            data = iron.data().clone();
            seen.push(data.cutoff);
            iron = cell(&data, "iron_ingot", true);
        }
        assert_eq!(seen, vec![Some(1), Some(0), Some(-1), Some(1)], "down from the block to the nugget, then -1, read as the block");
        iron.switch_compression_cutoff(true);
        let mut up = cell(iron.data(), "iron_ingot", true);
        assert_eq!(up.cutoff_item(), Some(&key("iron_block")));
        up.switch_compression_cutoff(true);
        let mut wrapped = cell(up.data(), "iron_ingot", true);
        assert_eq!(wrapped.cutoff_item(), Some(&key("iron_nugget")));
        assert_eq!(wrapped.available_stacks(), vec![(key("iron_nugget"), 100)]);
        let nugget = |n| Variant { key: key("iron_nugget"), count: n };
        assert_eq!(wrapped.decompression_patterns()[0], Pattern { from: nugget(9), to: Variant { key: key("iron_ingot"), count: 1 } });
        assert!(cell(&data, "iron_ingot", false).decompression_patterns().is_empty(), "no card, no patterns");
    }

    /// A filter changed to another variant of the chain takes over the
    /// stored item; one outside it leaves the cell full, giving what it has
    /// and taking nothing.
    #[test]
    fn a_mismatched_filter() {
        let data = BulkData { item: Some(key("iron_ingot")), unit_count: Some(Big::from(90)), unit_factor: Some(Big::from(9)), cutoff: None };
        let mut blocks = cell(&data, "iron_block", false);
        assert_eq!(blocks.status(), CellState::NotEmpty);
        assert_eq!(blocks.stored_item(), Some(&key("iron_block")));
        assert_eq!((blocks.stored_quantity(), blocks.trace_units()), (1, 9));
        assert_eq!(blocks.data().unit_factor, Some(Big::from(81)));
        let mut diamonds = cell(&data, "diamond", false);
        assert_eq!(diamonds.status(), CellState::Full);
        assert_eq!(diamonds.insert(&key("iron_ingot"), 1, Action::Simulate), 0);
        assert_eq!(diamonds.insert(&key("diamond"), 1, Action::Simulate), 0);
        // The variants under the stored one come along, a nugget remainder of 0 among them.
        assert_eq!(diamonds.available_stacks(), vec![(key("iron_ingot"), 10), (key("iron_nugget"), 0)]);
        assert_eq!(diamonds.extract(&key("iron_ingot"), 4, Action::Modulate), 4);
        assert!(!diamonds.is_preferred_storage_for(&key("emerald")));
        assert!(diamonds.is_preferred_storage_for(&key("iron_nugget")));
    }

    /// A count recorded at another factor is rescaled when read (a chain that
    /// changed with the recipes), and the stack takes the new numbers.
    #[test]
    fn a_count_recorded_at_another_factor_is_rescaled() {
        let data = BulkData { item: Some(key("iron_ingot")), unit_count: Some(Big::from(20)), unit_factor: Some(Big::from(1)), cutoff: Some(7) };
        let ingots = cell(&data, "iron_ingot", true);
        assert!(ingots.changed());
        assert_eq!(ingots.data().unit_count, Some(Big::from(180)));
        assert_eq!(ingots.data().unit_factor, Some(Big::from(9)));
        assert_eq!(ingots.cutoff_item(), Some(&key("iron_block")), "a cutoff past the chain is its largest");
        let unfiltered = BulkCell::new(BulkData { cutoff: Some(1), ..BulkData::default() }, None, true, &iron());
        assert_eq!(unfiltered.data().cutoff, None, "no item and no filter: the cutoff goes");
    }
}
