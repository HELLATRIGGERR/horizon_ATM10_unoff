//! The recipes of the two vats (`FermentationRecipe`, `DigestionRecipe`): a fluid in the tank and up to three item
//! stacks in the input slots make one item. They differ in how the items are read: fermentation's ingredients are
//! plain (one of each, matched shapeless-style, every stack used once), digestion's are sized (counts, matched in
//! order). Both default to 200 ticks.
use crate::recipe::{field, ItemResult};
use crate::{Ingredient, SizedFluidIngredient, SizedIngredient, Tags};
use serde_json::Value;

pub const FERMENTATION: &str = "theurgy:fermentation";
pub const DIGESTION: &str = "theurgy:digestion";
/// `DEFAULT_TIME` of both recipe types.
pub const DEFAULT_TIME: i32 = 200;
/// The vats' input slots (`InputInventory`: 3).
pub const INPUT_SLOTS: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VatKind {
    Fermentation,
    Digestion,
}

/// A stack in an input slot: its item and count (`None` for an empty slot).
pub type Stack<'a> = Option<(&'a str, u32)>;

#[derive(Clone, Debug, PartialEq)]
pub struct VatRecipe {
    pub id: String,
    pub kind: VatKind,
    pub fluid: SizedFluidIngredient,
    /// Fermentation's plain ingredients are read as count 1.
    pub ingredients: Vec<SizedIngredient>,
    pub result: ItemResult,
    pub time: i32,
}

impl VatRecipe {
    pub fn parse(kind: VatKind, id: &str, json: &Value) -> Result<Self, String> {
        let wrap = |e: String| format!("{id}: {e}");
        let fluid = SizedFluidIngredient::parse(field(json, "fluid").map_err(wrap)?).map_err(wrap)?;
        let list = field(json, "ingredients").map_err(wrap)?.as_array().ok_or_else(|| wrap("ingredients is not a list".into()))?;
        let ingredients = list
            .iter()
            .map(|i| match kind {
                VatKind::Fermentation => Ingredient::parse(i).map(|ingredient| SizedIngredient { ingredient, count: 1 }),
                VatKind::Digestion => SizedIngredient::parse(i),
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(wrap)?;
        let result = ItemResult::parse(field(json, "result").map_err(wrap)?).map_err(wrap)?;
        let time = json.get("time").and_then(Value::as_i64).and_then(|t| i32::try_from(t).ok()).unwrap_or(DEFAULT_TIME);
        Ok(Self { id: id.to_owned(), kind, fluid, ingredients, result, time })
    }

    /// `matches`: the fluid in the tank fits (kind and amount) and the input slots hold the ingredients and nothing
    /// else.
    pub fn matches(&self, inputs: &[Stack<'_>], fluid: &str, amount: i32, items: Tags<'_>, fluids: Tags<'_>) -> bool {
        if !self.fluid.test(fluid, amount, fluids) {
            return false;
        }
        match self.kind {
            VatKind::Fermentation => {
                let stacks: Vec<&str> = inputs.iter().flatten().map(|(i, _)| *i).collect();
                stacks.len() == self.ingredients.len() && self.shapeless(&stacks, items)
            }
            VatKind::Digestion => {
                let mut visited = vec![false; inputs.len()];
                for ingredient in &self.ingredients {
                    let Some(at) = inputs.iter().enumerate().position(|(i, s)| !visited[i] && s.is_some_and(|(item, count)| ingredient.test(item, count, items))) else {
                        return false;
                    };
                    visited[at] = true;
                }
                inputs.iter().enumerate().all(|(i, s)| visited[i] || s.is_none())
            }
        }
    }

    /// `RecipeMatcher.findMatches`: every stack to its own ingredient.
    fn shapeless(&self, stacks: &[&str], items: Tags<'_>) -> bool {
        fn assign(r: &VatRecipe, stacks: &[&str], items: Tags<'_>, at: usize, used: &mut Vec<bool>) -> bool {
            let Some(item) = stacks.get(at) else { return true };
            for (j, ingredient) in r.ingredients.iter().enumerate() {
                if !used[j] && ingredient.ingredient.test(item, items) {
                    used[j] = true;
                    if assign(r, stacks, items, at + 1, used) {
                        return true;
                    }
                    used[j] = false;
                }
            }
            false
        }
        assign(self, stacks, items, 0, &mut vec![false; self.ingredients.len()])
    }

    /// The `CachedCheck`'s item-only test of one stack: it fits some ingredient.
    pub fn takes(&self, item: &str, items: Tags<'_>) -> bool {
        self.ingredients.iter().any(|i| i.ingredient.test(item, items))
    }

    /// `matchesRecipe`: every given stack fits some ingredient (fluid and counts aside).
    pub fn takes_all(&self, stacks: &[&str], items: Tags<'_>) -> bool {
        stacks.iter().all(|s| self.takes(s, items))
    }

    /// The `CachedCheck`'s fluid-only test: the fluid fits (any amount).
    pub fn takes_fluid(&self, fluid: &str, fluids: Tags<'_>) -> bool {
        self.fluid.ingredient.test(fluid, fluids)
    }

    /// `craft`: for each ingredient in order, the first slot holding a fitting item gives its count (or what is
    /// there). Returns the slots and what was taken from each.
    pub fn consumption(&self, inputs: &[Stack<'_>], items: Tags<'_>) -> Vec<(usize, u32)> {
        let mut left: Vec<Stack<'_>> = inputs.to_vec();
        let mut out = Vec::new();
        for ingredient in &self.ingredients {
            if let Some(i) = left.iter().position(|s| s.is_some_and(|(item, _)| ingredient.ingredient.test(item, items))) {
                let (item, count) = left[i].expect("the slot just matched");
                let taken = ingredient.count.min(count);
                left[i] = (count > taken).then_some((item, count - taken));
                out.push((i, taken));
            }
        }
        out
    }
}

/// `canProcess(ItemStack)` of a vat: the stack is already in a slot; or the recipe running takes it; or (stacks in
/// the slots) some recipe takes them all with it; or (empty slots) some recipe has it as an ingredient.
pub fn can_process_item(recipes: &[VatRecipe], current: Option<usize>, inputs: &[Stack<'_>], item: &str, items: Tags<'_>) -> bool {
    if inputs.iter().flatten().any(|(i, _)| *i == item) {
        return true;
    }
    if let Some(r) = current.and_then(|i| recipes.get(i)) {
        return r.takes(item, items);
    }
    let mut stacks: Vec<&str> = inputs.iter().flatten().map(|(i, _)| *i).collect();
    stacks.push(item);
    if stacks.len() > 1 {
        return recipes.iter().any(|r| r.takes_all(&stacks, items));
    }
    recipes.iter().any(|r| r.takes(item, items))
}

/// `canProcess(FluidStack)`: the fluid in the tank, or any recipe's fluid.
pub fn can_process_fluid(recipes: &[VatRecipe], held: &str, fluid: &str, fluids: Tags<'_>) -> bool {
    held == fluid || recipes.iter().any(|r| r.takes_fluid(fluid, fluids))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn none(_: &str, _: &str) -> bool {
        false
    }
    fn tags(i: &str, t: &str) -> bool {
        matches!((i, t), ("minecraft:wheat", "c:crops") | ("minecraft:carrot", "c:crops") | ("minecraft:water", "minecraft:water"))
    }

    fn fermentation() -> VatRecipe {
        VatRecipe::parse(
            VatKind::Fermentation,
            "f",
            &json!({"fluid":{"amount":125,"ingredient":{"fluid":"minecraft:water"}},"ingredients":[{"item":"theurgy:sulfur"},{"tag":"c:crops"}],"result":{"count":1,"id":"theurgy:niter"},"type":"theurgy:fermentation"}),
        )
        .unwrap()
    }

    fn digestion() -> VatRecipe {
        VatRecipe::parse(
            VatKind::Digestion,
            "d",
            &json!({"fluid":{"amount":10,"ingredient":{"fluid":"theurgy:sal_ammoniac"}},"ingredients":[{"count":2,"ingredient":{"item":"theurgy:niter"}}],"result":{"count":4,"id":"theurgy:niter2"},"time":50}),
        )
        .unwrap()
    }

    #[test]
    fn fermentation_matches_shapeless_with_the_fluid() {
        let r = fermentation();
        assert_eq!(r.time, DEFAULT_TIME);
        let both = [Some(("minecraft:wheat", 3)), Some(("theurgy:sulfur", 1)), None];
        assert!(r.matches(&both, "minecraft:water", 125, &tags, &tags), "slot order does not matter");
        assert!(!r.matches(&both, "minecraft:water", 124, &tags, &tags), "not enough water");
        assert!(!r.matches(&both, "", 0, &tags, &tags));
        assert!(!r.matches(&[Some(("theurgy:sulfur", 1)), None, None], "minecraft:water", 125, &tags, &tags), "one short");
        let three = [Some(("minecraft:wheat", 1)), Some(("theurgy:sulfur", 1)), Some(("minecraft:carrot", 1))];
        assert!(!r.matches(&three, "minecraft:water", 125, &tags, &tags), "one too many");
        let two_crops = [Some(("minecraft:wheat", 1)), Some(("minecraft:carrot", 1)), None];
        assert!(!r.matches(&two_crops, "minecraft:water", 125, &tags, &tags), "each ingredient once");
        assert_eq!(r.consumption(&both, &tags), vec![(1, 1), (0, 1)], "in the recipe's order");
    }

    #[test]
    fn digestion_matches_counts_and_nothing_else() {
        let r = digestion();
        assert_eq!(r.time, 50);
        let two = [None, Some(("theurgy:niter", 2)), None];
        assert!(r.matches(&two, "theurgy:sal_ammoniac", 10, &none, &none));
        assert!(!r.matches(&[Some(("theurgy:niter", 1)), None, None], "theurgy:sal_ammoniac", 10, &none, &none), "count");
        assert!(!r.matches(&[Some(("theurgy:niter", 2)), Some(("minecraft:dirt", 1)), None], "theurgy:sal_ammoniac", 10, &none, &none), "extra");
        assert_eq!(r.consumption(&[Some(("theurgy:niter", 5)), None, None], &none), vec![(0, 2)]);
    }

    #[test]
    fn can_process_follows_what_is_there() {
        let rs = vec![fermentation(), digestion()];
        let empty = [None, None, None];
        assert!(can_process_item(&rs, None, &empty, "minecraft:wheat", &tags), "an ingredient of some recipe");
        assert!(!can_process_item(&rs, None, &empty, "minecraft:dirt", &tags));
        let sulfur = [Some(("theurgy:sulfur", 1)), None, None];
        assert!(can_process_item(&rs, None, &sulfur, "minecraft:carrot", &tags), "shares a recipe with the sulfur");
        assert!(!can_process_item(&rs, None, &sulfur, "theurgy:niter", &tags), "digestion's niter is another recipe");
        assert!(can_process_item(&rs, None, &sulfur, "theurgy:sulfur", &tags), "already there");
        assert!(can_process_fluid(&rs, "", "minecraft:water", &tags) && can_process_fluid(&rs, "", "theurgy:sal_ammoniac", &tags));
        assert!(!can_process_fluid(&rs, "", "minecraft:lava", &tags));
        assert!(can_process_fluid(&rs, "minecraft:lava", "minecraft:lava", &tags), "what the tank holds");
    }
}
