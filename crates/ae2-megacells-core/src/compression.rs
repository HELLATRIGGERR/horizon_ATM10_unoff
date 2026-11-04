//! MEGA's bulk compression: `CompressionChain`, and the chains
//! `CompressionService.loadRecipes` gathers from the crafting recipes and the
//! `megacells:compression_overrides` data map (`CompressionOverride`).
use crate::big::Big;
use ae2_cell_core::ItemKey;
use std::fmt;

/// `CompressionChain.STACK_LIMIT`: `(long) Math.pow(2, 42)`, the most of one
/// variant a bulk cell lists.
pub const STACK_LIMIT: u64 = 1 << 42;

/// One variant of a chain: the item, and how many of the previous variant's
/// make one of it (1 for the smallest, else 1 to 9 for real recipes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variant {
    pub key: ItemKey,
    pub count: u32,
}

/// `DecompressionPattern(from, to)`: `from` (an item and its amount) becomes
/// `to` in one push.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    pub from: Variant,
    pub to: Variant,
}

/// `CompressionChain`: the variants of one item, smallest first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompressionChain {
    variants: Vec<Variant>,
}

impl CompressionChain {
    pub fn new(variants: Vec<Variant>) -> Self {
        Self { variants }
    }
    pub fn variants(&self) -> &[Variant] {
        &self.variants
    }
    /// `isEmpty`.
    pub fn is_empty(&self) -> bool {
        self.variants.is_empty()
    }
    /// `size`.
    pub fn size(&self) -> usize {
        self.variants.len()
    }
    /// `containsVariant`: `ItemStack.isSameItemSameComponents` with any variant.
    pub fn contains_variant(&self, item: &ItemKey) -> bool {
        self.variants.iter().any(|variant| variant.key == *item)
    }
    /// `getItem(index)`.
    pub fn item(&self, index: usize) -> &Variant {
        &self.variants[index]
    }
    /// `unitFactor`: the product of the counts up to and including `item`'s
    /// variant, the number of smallest units one of it is; 1 for none or an
    /// item not in the chain.
    pub fn unit_factor(&self, item: Option<&ItemKey>) -> Big {
        let Some(item) = item else { return Big::one() };
        let mut potential = Big::one();
        for variant in &self.variants {
            potential = potential.mul(&Big::from(u64::from(variant.count)));
            if variant.key == *item {
                return potential;
            }
        }
        Big::one()
    }
    /// `gatherPatterns`: per pair of neighbouring variants, the compression
    /// (`count` smaller to one larger) and the decompression (back).
    fn gather_patterns(&self) -> Vec<(Pattern, Pattern)> {
        let mut patterns = Vec::new();
        for i in 0..self.variants.len().saturating_sub(1) {
            let smaller = Variant { key: self.variants[i].key.clone(), count: self.variants[i + 1].count };
            let larger = Variant { key: self.variants[i + 1].key.clone(), count: 1 };
            let compression = Pattern { from: smaller.clone(), to: larger.clone() };
            let decompression = Pattern { from: larger, to: smaller };
            patterns.push((compression, decompression));
        }
        patterns
    }
    /// `getDecompressionPatterns(cutoff)`: below the cutoff, each larger
    /// variant decompressed into the one under it; from the cutoff up, each
    /// compressed into the next.
    pub fn decompression_patterns(&self, cutoff: usize) -> Vec<Pattern> {
        if self.is_empty() {
            return Vec::new();
        }
        let available = self.gather_patterns();
        let mut patterns = Vec::new();
        for i in 0..cutoff.min(self.variants.len()) {
            patterns.push(available[i].1.clone());
        }
        for i in cutoff..self.variants.len().saturating_sub(1) {
            patterns.push(available[i].0.clone());
        }
        patterns
    }
    /// `initStacks(unitCount, cutoff, fallback)`: what a compressing bulk cell
    /// lists, in order: each variant under the cutoff the remainder that does
    /// not make one of the next, the cutoff variant the rest (at most
    /// [`STACK_LIMIT`]); with no chain, the fallback item and the count.
    /// A variant met twice keeps its first place and its last amount (the
    /// `Object2LongLinkedOpenHashMap`).
    pub fn init_stacks(&self, unit_count: &Big, cutoff: usize, fallback: Option<&ItemKey>) -> Vec<(ItemKey, u64)> {
        let mut stacks: Vec<(ItemKey, u64)> = Vec::new();
        let mut put = |key: &ItemKey, amount: u64| match stacks.iter_mut().find(|(k, _)| k == key) {
            Some(entry) => entry.1 = amount,
            None => stacks.push((key.clone(), amount)),
        };
        if !self.is_empty() {
            let mut units = unit_count.clone();
            for i in 0..cutoff {
                // The factor of the next variant along: how many of this one fit into it.
                let factor = Big::from(u64::from(self.variants[(i + 1) % self.variants.len()].count));
                let (quotient, remainder) = units.div_rem(&factor);
                put(&self.variants[i].key, remainder.clamp_to(u64::MAX));
                units = quotient;
            }
            put(&self.variants[cutoff].key, units.clamp_to(STACK_LIMIT));
        } else if let Some(fallback) = fallback {
            put(fallback, unit_count.clamp_to(STACK_LIMIT));
        }
        stacks
    }
}

/// `toString`: `[1x → minecraft:iron_nugget, 9x → minecraft:iron_ingot, ...]`
/// (`variantString` marks an item with components `(*)`).
impl fmt::Display for CompressionChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[")?;
        for (i, variant) in self.variants.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{}x → {}", variant.count, variant.key.item_id)?;
        }
        f.write_str("]")
    }
}

/// A crafting recipe as `CompressionService` reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CraftingRecipe {
    /// The recipe's id, for logs.
    pub id: String,
    /// `getIngredients()`: each ingredient's `getItems()`, in order; an empty
    /// list is `Ingredient.EMPTY`, a shaped pattern's space.
    pub ingredients: Vec<Vec<ItemKey>>,
    /// `getResultItem`: the result and its count.
    pub result: ItemKey,
    pub count: u32,
}

/// A `megacells:compression_overrides` value for an item (`CompressionOverride.CODEC`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OverrideValue {
    /// `"NONE"`, or an id that is air: the item takes no part in any chain.
    None,
    /// The item this one compresses into, whatever the recipes' reversibility.
    Variant(String),
}

/// `CompressionOverride(larger, smaller)`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Override {
    larger: Variant,
    smaller: Variant,
}

/// What [`load_recipes`] gathered: the chains, and how many overrides went into them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Chains {
    pub chains: Vec<CompressionChain>,
    pub overrides: usize,
}

impl Chains {
    /// `CompressionService.getChain`: the first chain with `item` as a
    /// variant; none (the `EMPTY` chain) for no item or one in no chain.
    pub fn chain(&self, item: Option<&ItemKey>) -> Option<&CompressionChain> {
        let item = item?;
        self.chains.iter().find(|chain| chain.contains_variant(item))
    }
}

/// The data map's value for an item, by its id.
pub type Overrides<'a> = &'a dyn Fn(&str) -> Option<OverrideValue>;

/// `isBlacklisted`: the data map says `NONE` for the item.
fn blacklisted(item: &ItemKey, overrides: Overrides<'_>) -> bool {
    overrides(&item.item_id) == Some(OverrideValue::None)
}

fn non_empty(recipe: &CraftingRecipe) -> impl Iterator<Item = &Vec<ItemKey>> {
    recipe.ingredients.iter().filter(|ingredient| !ingredient.is_empty())
}

/// `getIngredients().getFirst().getItems()`: nothing for a recipe of none.
fn first_items(recipe: &CraftingRecipe) -> &[ItemKey] {
    recipe.ingredients.first().map_or(&[][..], Vec::as_slice)
}

/// `isDecompressionRecipe`: a single non-empty ingredient.
fn is_decompression_recipe(recipe: &CraftingRecipe) -> bool {
    non_empty(recipe).count() == 1
}

/// `isCompressionRecipe`: one of the result, and every non-empty ingredient
/// the same (as an `Ingredient`, or by the items it lists, in order).
fn is_compression_recipe(recipe: &CraftingRecipe) -> bool {
    if recipe.count != 1 {
        return false;
    }
    let mut ingredients = non_empty(recipe);
    let Some(first) = ingredients.next() else { return false };
    ingredients.all(|ingredient| ingredient == first)
}

/// `overrideRecipe`: when the data map names the result as the variant of
/// the first non-empty ingredient's item, the recipe makes an override and
/// counts as reversible.
fn override_recipe(recipe: &CraftingRecipe, overrides_found: &mut Vec<Override>, overrides: Overrides<'_>) -> bool {
    if blacklisted(&recipe.result, overrides) {
        return false;
    }
    let ingredients: Vec<&Vec<ItemKey>> = non_empty(recipe).collect();
    let Some(first) = ingredients.first() else { return false };
    for input in first.iter() {
        let Some(variant) = overrides(&input.item_id) else { continue };
        if variant != OverrideValue::Variant(recipe.result.item_id.clone()) {
            continue;
        }
        let decompressed = is_decompression_recipe(recipe);
        let larger = if decompressed { Variant { key: input.clone(), count: 1 } } else { Variant { key: recipe.result.clone(), count: recipe.count } };
        let smaller = if decompressed {
            Variant { key: recipe.result.clone(), count: recipe.count }
        } else {
            Variant { key: input.clone(), count: ingredients.len() as u32 }
        };
        overrides_found.push(Override { larger, smaller });
        return true;
    }
    false
}

/// `isIrreversible`: no candidate of the other direction undoes the recipe
/// with the same quantities, and the data map makes no override of it.
fn is_irreversible(recipe: &CraftingRecipe, candidates: &[CraftingRecipe], overrides_found: &mut Vec<Override>, overrides: Overrides<'_>) -> bool {
    if override_recipe(recipe, overrides_found, overrides) {
        return false;
    }
    let test_input = first_items(recipe);
    let test_output = &recipe.result.item_id;
    for candidate in candidates {
        let input = first_items(candidate);
        let output = &candidate.result.item_id;
        let compressible = input.iter().any(|i| i.item_id == *test_output && !blacklisted(i, overrides));
        let decompressible = test_input.iter().any(|i| i.item_id == *output && !blacklisted(i, overrides));
        let same_quantity = candidate.count as usize == recipe.ingredients.len() && recipe.count as usize == candidate.ingredients.len();
        if compressible && decompressible && same_quantity {
            return false;
        }
    }
    true
}

/// `getNextVariant`: the next variant up (`compressed`) or down from `item`,
/// from an override first, then from the first recipe whose first
/// ingredient lists the item; the override or recipe is used up.
fn next_variant(item: &ItemKey, recipes: &mut Vec<CraftingRecipe>, overrides_found: &mut Vec<Override>, compressed: bool) -> Option<Variant> {
    for i in 0..overrides_found.len() {
        let found = &overrides_found[i];
        if compressed && found.smaller.key == *item {
            return Some(overrides_found.remove(i).larger);
        }
        if !compressed && found.larger.key == *item {
            return Some(overrides_found.remove(i).smaller);
        }
    }
    for i in 0..recipes.len() {
        if first_items(&recipes[i]).iter().any(|input| input == item) {
            let recipe = recipes.remove(i);
            let count = if compressed { recipe.ingredients.len() as u32 } else { recipe.count };
            return Some(Variant { key: recipe.result, count });
        }
    }
    None
}

/// `generateChain`: the variants under `base` by decompression, then those
/// over it by compression; recipes making a variant already taken are
/// dropped from the other direction's list. A variant equal to the base ends
/// that direction (MEGA warns of it unless its count is 1).
fn generate_chain(base: &ItemKey, compressed: &mut Vec<CraftingRecipe>, decompressed: &mut Vec<CraftingRecipe>, overrides_found: &mut Vec<Override>) -> CompressionChain {
    let mut lower_list = vec![Variant { key: base.clone(), count: 1 }];
    let mut lower = next_variant(base, decompressed, overrides_found, false);
    while let Some(stack) = lower {
        if stack.key == *base {
            break;
        }
        compressed.retain(|recipe| recipe.result != stack.key);
        lower = next_variant(&stack.key, decompressed, overrides_found, false);
        lower_list.push(stack);
    }
    let mut variants = Vec::new();
    for i in (1..=lower_list.len()).rev() {
        variants.push(Variant { key: lower_list[i - 1].key.clone(), count: lower_list[i % lower_list.len()].count });
    }
    let mut higher = next_variant(base, compressed, overrides_found, true);
    while let Some(stack) = higher {
        if stack.key == *base {
            break;
        }
        decompressed.retain(|recipe| recipe.result != stack.key);
        higher = next_variant(&stack.key, compressed, overrides_found, true);
        variants.push(stack);
    }
    CompressionChain::new(variants)
}

/// `CompressionService.loadRecipes`: from the crafting recipes, in the
/// recipe manager's order, the compression and decompression candidates;
/// those no recipe of the other direction reverses dropped (unless the data
/// map overrides them); then, recipes whose first ingredient lists fewer
/// items first, a chain around the result of each compression recipe left.
pub fn load_recipes(recipes: &[CraftingRecipe], overrides: Overrides<'_>) -> Chains {
    let mut compressed = Vec::new();
    let mut decompressed = Vec::new();
    let mut overrides_found = Vec::new();
    for recipe in recipes {
        if is_compression_recipe(recipe) {
            compressed.push(recipe.clone());
        } else if is_decompression_recipe(recipe) {
            decompressed.push(recipe.clone());
        }
    }
    // ArrayList.removeIf tests every element in order before removing.
    let keep: Vec<bool> = compressed.iter().map(|recipe| !is_irreversible(recipe, &decompressed, &mut overrides_found, overrides)).collect();
    let mut keep = keep.into_iter();
    compressed.retain(|_| keep.next().unwrap_or(false));
    let keep: Vec<bool> = decompressed.iter().map(|recipe| !is_irreversible(recipe, &compressed, &mut overrides_found, overrides)).collect();
    let mut keep = keep.into_iter();
    decompressed.retain(|_| keep.next().unwrap_or(false));
    let initial_overrides = overrides_found.len();
    // List.sort is stable: by the item count of the first ingredient.
    compressed.sort_by_key(|recipe| first_items(recipe).len());
    decompressed.sort_by_key(|recipe| first_items(recipe).len());
    let mut chains = Vec::new();
    while !compressed.is_empty() {
        let base = compressed.remove(0).result;
        decompressed.retain(|recipe| recipe.result != base);
        chains.push(generate_chain(&base, &mut compressed, &mut decompressed, &mut overrides_found));
    }
    Chains { chains, overrides: initial_overrides - overrides_found.len() }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn key(id: &str) -> ItemKey {
        ItemKey { item_id: format!("minecraft:{id}"), components: Vec::new() }
    }
    fn items(ids: &[&str]) -> Vec<ItemKey> {
        ids.iter().map(|id| key(id)).collect()
    }
    /// A 3×3 of one item into one result.
    fn nine(name: &str, input: &str, result: &str) -> CraftingRecipe {
        CraftingRecipe { id: name.into(), ingredients: vec![items(&[input]); 9], result: key(result), count: 1 }
    }
    fn four(name: &str, input: &str, result: &str) -> CraftingRecipe {
        CraftingRecipe { id: name.into(), ingredients: vec![items(&[input]); 4], result: key(result), count: 1 }
    }
    fn split(name: &str, input: &str, result: &str, count: u32) -> CraftingRecipe {
        CraftingRecipe { id: name.into(), ingredients: vec![items(&[input])], result: key(result), count }
    }
    fn chain(ids: &[(&str, u32)]) -> CompressionChain {
        CompressionChain::new(ids.iter().map(|(id, count)| Variant { key: key(id), count: *count }).collect())
    }
    fn none(_: &str) -> Option<OverrideValue> {
        None
    }

    /// Vanilla's iron: nugget, ingot and block both ways make one chain,
    /// smallest first, whatever recipe comes first; an irreversible 3×3
    /// (nether wart) and a two-ingot plate make none.
    #[test]
    fn reversible_recipes_make_one_chain_smallest_first() {
        let recipes = vec![
            nine("iron_block", "iron_ingot", "iron_block"),
            split("iron_ingot_from_iron_block", "iron_block", "iron_ingot", 9),
            nine("iron_ingot_from_nuggets", "iron_nugget", "iron_ingot"),
            split("iron_nugget", "iron_ingot", "iron_nugget", 9),
            nine("nether_wart_block", "nether_wart", "nether_wart_block"),
            CraftingRecipe { id: "heavy_weighted_pressure_plate".into(), ingredients: items(&["iron_ingot", "iron_ingot"]).into_iter().map(|k| vec![k]).collect(), result: key("heavy_weighted_pressure_plate"), count: 1 },
        ];
        let expected = chain(&[("iron_nugget", 1), ("iron_ingot", 9), ("iron_block", 9)]);
        let loaded = load_recipes(&recipes, &none);
        assert_eq!(loaded.chains, vec![expected.clone()]);
        assert_eq!(loaded.overrides, 0);
        let mut reversed = recipes.clone();
        reversed.reverse();
        assert_eq!(load_recipes(&reversed, &none).chains, vec![expected.clone()]);
        assert_eq!(expected.to_string(), "[1x → minecraft:iron_nugget, 9x → minecraft:iron_ingot, 9x → minecraft:iron_block]");
        assert!(loaded.chain(Some(&key("iron_ingot"))).is_some());
        assert!(loaded.chain(Some(&key("nether_wart"))).is_none());
        assert!(loaded.chain(None).is_none());
    }

    /// `compression_overrides`: snowballs have no recipe back from a snow
    /// block, so only the data map makes their chain; ice to packed ice to
    /// blue ice chains through two overrides; "NONE" keeps an item out even
    /// with recipes both ways.
    #[test]
    fn overrides_make_irreversible_recipes_chains_and_none_keeps_items_out() {
        let recipes = vec![
            four("snow_block", "snowball", "snow_block"),
            nine("blue_ice", "packed_ice", "blue_ice"),
            nine("packed_ice", "ice", "packed_ice"),
            nine("hay_block", "wheat", "hay_block"),
            split("wheat", "hay_block", "wheat", 9),
        ];
        let overrides = |id: &str| match id {
            "minecraft:snowball" => Some(OverrideValue::Variant("minecraft:snow_block".into())),
            "minecraft:ice" => Some(OverrideValue::Variant("minecraft:packed_ice".into())),
            "minecraft:packed_ice" => Some(OverrideValue::Variant("minecraft:blue_ice".into())),
            "minecraft:wheat" => Some(OverrideValue::None),
            _ => None,
        };
        let loaded = load_recipes(&recipes, &overrides);
        assert_eq!(loaded.chains, vec![
            chain(&[("snowball", 1), ("snow_block", 4)]),
            chain(&[("ice", 1), ("packed_ice", 9), ("blue_ice", 9)]),
        ]);
        assert_eq!(loaded.overrides, 3);
        assert_eq!(load_recipes(&recipes, &none).chains, vec![chain(&[("wheat", 1), ("hay_block", 9)])], "without the data map only hay chains");
        // getNextVariant gives an override's larger stack as it is, one of
        // it: a chain built up through an override (packed ice's recipe
        // first) has blue ice at 1, and blue ice's own recipe, left over,
        // makes a chain of its own, which getChain never reaches.
        let mut packed_first = recipes.clone();
        packed_first.swap(1, 2);
        assert_eq!(load_recipes(&packed_first, &overrides).chains, vec![
            chain(&[("snowball", 1), ("snow_block", 4)]),
            chain(&[("ice", 1), ("packed_ice", 9), ("blue_ice", 1)]),
            chain(&[("blue_ice", 1)]),
        ]);
    }

    /// The patterns a compressing cell offers: decompression under the
    /// cutoff, compression from it up; `initStacks` remainders under the
    /// cutoff and the rest at it.
    #[test]
    fn patterns_and_stacks_follow_the_cutoff() {
        let iron = chain(&[("iron_nugget", 1), ("iron_ingot", 9), ("iron_block", 9)]);
        let nugget = |n| Variant { key: key("iron_nugget"), count: n };
        let ingot = |n| Variant { key: key("iron_ingot"), count: n };
        let block = |n| Variant { key: key("iron_block"), count: n };
        assert_eq!(iron.decompression_patterns(2), vec![Pattern { from: ingot(1), to: nugget(9) }, Pattern { from: block(1), to: ingot(9) }]);
        assert_eq!(iron.decompression_patterns(1), vec![Pattern { from: ingot(1), to: nugget(9) }, Pattern { from: ingot(9), to: block(1) }]);
        assert_eq!(iron.decompression_patterns(0), vec![Pattern { from: nugget(9), to: ingot(1) }, Pattern { from: ingot(9), to: block(1) }]);
        // 1 block, 2 ingots, 3 nuggets: 81 + 18 + 3 units.
        let units = Big::from(102);
        assert_eq!(iron.init_stacks(&units, 2, None), vec![(key("iron_nugget"), 3), (key("iron_ingot"), 2), (key("iron_block"), 1)]);
        assert_eq!(iron.init_stacks(&units, 1, None), vec![(key("iron_nugget"), 3), (key("iron_ingot"), 11)]);
        assert_eq!(iron.init_stacks(&units, 0, None), vec![(key("iron_nugget"), 102)]);
        assert_eq!(iron.unit_factor(Some(&key("iron_block"))), Big::from(81));
        assert_eq!(iron.unit_factor(Some(&key("iron_ingot"))), Big::from(9));
        assert_eq!(iron.unit_factor(Some(&key("diamond"))), Big::one());
        let empty = CompressionChain::default();
        assert_eq!(empty.init_stacks(&Big::parse("9000000000000000000000").unwrap(), 0, Some(&key("diamond"))), vec![(key("diamond"), STACK_LIMIT)]);
        assert!(empty.decompression_patterns(0).is_empty());
    }
}
