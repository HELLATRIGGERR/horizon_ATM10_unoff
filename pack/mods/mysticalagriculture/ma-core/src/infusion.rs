//! The infusion ritual: the altar, its eight pedestals and the recipe check (`tileentity/InfusionAltarTileEntity.java`,
//! `crafting/recipe/InfusionRecipe.java`; Cucumber's `MultiblockPositions`). Ingredients are sets of item ids here: the
//! runtime resolves tags and lists against the game's items.

use serde_json::Value;
use std::collections::HashSet;

/// `InfusionAltarTileEntity.pedestalLocations`, in its order: the recipe's slots 1 to 8 are these pedestals' items.
pub const PEDESTALS: [[i32; 3]; 8] = [[3, 0, 0], [0, 0, 3], [-3, 0, 0], [0, 0, -3], [2, 0, 2], [2, 0, -2], [-2, 0, 2], [-2, 0, -2]];
/// Ticks of an active altar with a recipe before it crafts.
pub const CRAFT_TICKS: i32 = 100;
/// `InfusionRecipe.Serializer`: at most eight pedestal ingredients, at least one.
pub const MAX_INGREDIENTS: usize = 8;
pub const ALTAR: &str = "mysticalagriculture:infusion_altar";
pub const PEDESTAL: &str = "mysticalagriculture:infusion_pedestal";
pub const TYPE: &str = "mysticalagriculture:infusion";

/// An infusion recipe as its JSON gives it, the ingredients still as JSON.
#[derive(Clone, Debug, PartialEq)]
pub struct RecipeJson {
    pub input: Value,
    pub ingredients: Vec<Value>,
    pub result: Value,
    pub transfer_components: bool,
}

/// `InfusionRecipe.Serializer.CODEC`: `input`, 1 to 8 `ingredients`, `result`, `transfer_components` (false).
pub fn read(json: &Value) -> Result<RecipeJson, String> {
    let input = json.get("input").ok_or("no input")?.clone();
    let ingredients = json.get("ingredients").and_then(Value::as_array).ok_or("no ingredients")?.clone();
    if ingredients.is_empty() {
        return Err("No ingredients for infusion recipe".into());
    }
    if ingredients.len() > MAX_INGREDIENTS {
        return Err(format!("Too many ingredients for infusion recipe. The maximum is: {MAX_INGREDIENTS}"));
    }
    let result = json.get("result").ok_or("no result")?.clone();
    let transfer_components = json.get("transfer_components").and_then(Value::as_bool).unwrap_or(false);
    Ok(RecipeJson { input, ingredients, result, transfer_components })
}

/// A recipe with its ingredients resolved to the item ids each accepts.
#[derive(Clone, Debug, PartialEq)]
pub struct Recipe<R> {
    pub id: String,
    pub input: HashSet<String>,
    pub ingredients: Vec<HashSet<String>>,
    pub result: R,
    pub transfer_components: bool,
}

impl<R> Recipe<R> {
    /// `InfusionRecipe.matches`: the altar's item fits the input, the pedestals' non-empty items are as many as the
    /// ingredients, and each ingredient takes a different item (NeoForge's `RecipeMatcher.findMatches`).
    pub fn matches(&self, altar: &str, pedestals: &[Option<&str>]) -> bool {
        let items: Vec<&str> = pedestals.iter().flatten().copied().collect();
        if items.len() != self.ingredients.len() || !self.input.contains(altar) {
            return false;
        }
        assignment(&items, &self.ingredients).is_some()
    }
}

/// A one-to-one assignment of `items` to `ingredients` (each ingredient takes one item it accepts), as NeoForge's
/// `RecipeMatcher.findMatches` finds one: `Some(match)`, where `match[i]` is the ingredient of item `i`.
pub fn assignment(items: &[&str], ingredients: &[HashSet<String>]) -> Option<Vec<usize>> {
    if items.len() != ingredients.len() {
        return None;
    }
    let mut owner: Vec<Option<usize>> = vec![None; ingredients.len()];
    fn augment(item: usize, items: &[&str], ingredients: &[HashSet<String>], seen: &mut [bool], owner: &mut [Option<usize>]) -> bool {
        for (j, ing) in ingredients.iter().enumerate() {
            if seen[j] || !ing.contains(items[item]) {
                continue;
            }
            seen[j] = true;
            if owner[j].is_none_or(|other| augment(other, items, ingredients, seen, owner)) {
                owner[j] = Some(item);
                return true;
            }
        }
        false
    }
    for i in 0..items.len() {
        let mut seen = vec![false; ingredients.len()];
        if !augment(i, items, ingredients, &mut seen, &mut owner) {
            return None;
        }
    }
    let mut out = vec![0; items.len()];
    for (j, o) in owner.iter().enumerate() {
        out[o.expect("a full matching")] = j;
    }
    Some(out)
}

/// The altar's saved state (`Progress`, `Active`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Altar {
    pub progress: i32,
    pub active: bool,
}

/// What a tick of the altar does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Nothing on the altar, or no recipe: back to rest.
    Reset,
    /// Not active: the progress goes to 0, the altar stays armed for a signal.
    Idle,
    /// Crafting: pedestals send their items' particles.
    Working,
    /// The 100th tick: the pedestals' items are used up and the result goes to the output slot.
    Craft,
}

impl Altar {
    /// `InfusionAltarTileEntity.tick`. `signal` is `hasNeighborSignal` (asked only while not yet active, as
    /// `isActive` does); `recipe` whether a recipe matches now.
    pub fn tick(&mut self, has_input: bool, signal: impl FnOnce() -> bool, recipe: impl FnOnce() -> bool) -> Step {
        if !has_input {
            *self = Altar::default();
            return Step::Reset;
        }
        if !self.active {
            self.active = signal();
        }
        if !self.active {
            self.progress = 0;
            return Step::Idle;
        }
        if !recipe() {
            *self = Altar::default();
            return Step::Reset;
        }
        self.progress += 1;
        if self.progress >= CRAFT_TICKS {
            *self = Altar::default();
            Step::Craft
        } else {
            Step::Working
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn augment() -> Recipe<&'static str> {
        // ATM10's `mysticalagriculture:augment/absorption_i`.
        let apple = set(&["minecraft:golden_apple"]);
        let inferium = set(&["mysticalagriculture:inferium_essence"]);
        Recipe {
            id: "mysticalagriculture:augment/absorption_i".into(),
            input: set(&["mysticalagriculture:unattuned_augment"]),
            ingredients: vec![apple.clone(), inferium.clone(), apple.clone(), inferium.clone(), apple.clone(), inferium.clone(), apple, inferium],
            result: "mysticalagriculture:absorption_i_augment",
            transfer_components: false,
        }
    }

    #[test]
    fn reads_the_recipe_codec() {
        let r = read(&json!({"type": TYPE, "input": {"item": "a"}, "ingredients": [{"item": "b"}], "result": {"id": "c"}})).unwrap();
        assert_eq!(r.ingredients.len(), 1);
        assert!(!r.transfer_components);
        assert!(read(&json!({"input": {"item": "a"}, "ingredients": [], "result": {"id": "c"}})).is_err());
        assert!(read(&json!({"input": {"item": "a"}, "ingredients": vec![json!({"item": "b"}); 9], "result": {"id": "c"}})).is_err());
    }

    #[test]
    fn pedestals_in_any_order() {
        let r = augment();
        let a = Some("minecraft:golden_apple");
        let e = Some("mysticalagriculture:inferium_essence");
        assert!(r.matches("mysticalagriculture:unattuned_augment", &[a, a, a, a, e, e, e, e]));
        assert!(r.matches("mysticalagriculture:unattuned_augment", &[e, a, e, a, e, a, e, a]));
        // One essence short, or the wrong altar item, or an extra item: no match.
        assert!(!r.matches("mysticalagriculture:unattuned_augment", &[a, a, a, a, e, e, e, None]));
        assert!(!r.matches("minecraft:stick", &[a, a, a, a, e, e, e, e]));
        assert!(!r.matches("mysticalagriculture:unattuned_augment", &[a, a, a, a, a, e, e, e]));
    }

    #[test]
    fn empty_pedestals_are_left_out_of_the_count() {
        let mut r = augment();
        r.ingredients.truncate(2);
        let a = Some("minecraft:golden_apple");
        let e = Some("mysticalagriculture:inferium_essence");
        assert!(r.matches("mysticalagriculture:unattuned_augment", &[None, a, None, None, None, None, e, None]));
    }

    #[test]
    fn assignment_takes_the_narrow_ingredient_first_when_it_must() {
        // The first item fits both; only a full matching gives the second item its ingredient.
        let wide = set(&["x", "y"]);
        let narrow = set(&["x"]);
        assert!(assignment(&["x", "y"], &[narrow.clone(), wide.clone()]).is_some());
        assert_eq!(assignment(&["x", "y"], &[wide, narrow]), Some(vec![1, 0]));
        assert!(assignment(&["y", "y"], &[set(&["x", "y"]), set(&["x"])]).is_none());
    }

    #[test]
    fn altar_crafts_on_the_hundredth_active_tick() {
        let mut altar = Altar::default();
        assert_eq!(altar.tick(true, || false, || true), Step::Idle);
        assert_eq!(altar.progress, 0);
        for n in 1..CRAFT_TICKS {
            assert_eq!(altar.tick(true, || n == 1, || true), Step::Working);
            assert_eq!(altar.progress, n);
        }
        // Active stays on once set: the signal isn't asked again.
        assert_eq!(altar.tick(true, || panic!("asked"), || true), Step::Craft);
        assert_eq!(altar, Altar::default());
    }

    #[test]
    fn altar_resets_without_its_item_or_its_recipe() {
        let mut altar = Altar { progress: 40, active: true };
        assert_eq!(altar.tick(true, || false, || false), Step::Reset);
        assert_eq!(altar, Altar::default());
        let mut altar = Altar { progress: 40, active: true };
        assert_eq!(altar.tick(false, || true, || true), Step::Reset);
        assert_eq!(altar, Altar::default());
    }
}
