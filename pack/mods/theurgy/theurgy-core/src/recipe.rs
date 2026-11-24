//! The recipes of the core apparatus, parsed from their 1.21.1 JSON (the codecs of `CalcinationRecipe`,
//! `LiquefactionRecipe`, `DistillationRecipe`, `IncubationRecipe` and `AccumulationRecipe`), and their matching
//! (`matches`, and the item-only checks their `CachedCheck`s make).
use crate::{FluidIngredient, Ingredient, SizedFluidIngredient, SizedIngredient, Tags};
use serde_json::Value;

/// `DEFAULT_TIME` of every recipe type here: 100 ticks when a recipe names no `time`.
pub const DEFAULT_TIME: i32 = 100;

pub const CALCINATION: &str = "theurgy:calcination";
pub const LIQUEFACTION: &str = "theurgy:liquefaction";
pub const DISTILLATION: &str = "theurgy:distillation";
pub const INCUBATION: &str = "theurgy:incubation";
pub const ACCUMULATION: &str = "theurgy:accumulation";

/// An item result (`ItemStack.STRICT_CODEC`: `{"id", "count", "components"}`), kept as written for the engine to make.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemResult {
    pub json: Value,
    pub id: String,
    pub count: u32,
}

impl ItemResult {
    pub fn parse(json: &Value) -> Result<Self, String> {
        // `ItemRecipeResult` also takes the ingredient-like `{"item", "count"}`.
        let id = json.get("id").or_else(|| json.get("item")).and_then(Value::as_str).ok_or_else(|| format!("a result without id: {json}"))?;
        let count = json.get("count").and_then(Value::as_u64).unwrap_or(1);
        let count = u32::try_from(count).ok().filter(|c| *c > 0).ok_or_else(|| format!("result count {count}"))?;
        let mut json = json.clone();
        if let Some(o) = json.as_object_mut()
            && let Some(item) = o.remove("item")
        {
            o.insert("id".into(), item);
        }
        Ok(Self { json, id: id.to_owned(), count })
    }
}

/// Theurgy's `RecipeResult` (`"type": "theurgy:item"` or `"theurgy:tag"`): an incubation's result.
#[derive(Clone, Debug, PartialEq)]
pub enum RecipeResult {
    Item(ItemResult),
    /// `TagRecipeResult`: the tag's preferred item (`TagUtil.getItemStackForTag`), `count` of it, `components`.
    Tag { tag: String, count: u32, components: Option<Value> },
}

impl RecipeResult {
    pub fn parse(json: &Value) -> Result<Self, String> {
        match json.get("type").and_then(Value::as_str) {
            Some("theurgy:tag") => {
                let tag = json.get("tag").and_then(Value::as_str).ok_or("a tag result without tag")?;
                let count = json.get("count").and_then(Value::as_u64).ok_or("a tag result without count")?;
                let count = u32::try_from(count).map_err(|_| format!("count {count}"))?;
                Ok(Self::Tag { tag: tag.to_owned(), count, components: json.get("components").cloned() })
            }
            Some("theurgy:item") => {
                let mut plain = json.clone();
                if let Some(o) = plain.as_object_mut() {
                    o.remove("type");
                }
                Ok(Self::Item(ItemResult::parse(&plain)?))
            }
            other => Err(format!("unknown result type {other:?}")),
        }
    }
}

fn time(json: &Value) -> i32 {
    json.get("time").and_then(Value::as_i64).and_then(|t| i32::try_from(t).ok()).unwrap_or(DEFAULT_TIME)
}

fn field<'a>(json: &'a Value, name: &str) -> Result<&'a Value, String> {
    json.get(name).ok_or_else(|| format!("no {name}"))
}

/// `CalcinationRecipe`: a sized ingredient burnt to a result in the calcination oven.
#[derive(Clone, Debug, PartialEq)]
pub struct Calcination {
    pub id: String,
    pub ingredient: SizedIngredient,
    pub result: ItemResult,
    pub time: i32,
}

/// `DistillationRecipe`: the same fields as calcination, run in the distiller.
pub type Distillation = Calcination;

impl Calcination {
    pub fn parse(id: &str, json: &Value) -> Result<Self, String> {
        let wrap = |e: String| format!("{id}: {e}");
        Ok(Self {
            id: id.to_owned(),
            ingredient: SizedIngredient::parse(field(json, "ingredient").map_err(wrap)?).map_err(wrap)?,
            result: ItemResult::parse(field(json, "result").map_err(wrap)?).map_err(wrap)?,
            time: time(json),
        })
    }

    /// `matches`: the input slot's stack fits the sized ingredient (item and count).
    pub fn matches(&self, item: &str, count: u32, tags: Tags<'_>) -> bool {
        self.ingredient.test(item, count, tags)
    }

    /// The `CachedCheck`'s item-only test (`sizedIngredient().ingredient().test(stack)`): any count.
    pub fn takes(&self, item: &str, tags: Tags<'_>) -> bool {
        self.ingredient.ingredient.test(item, tags)
    }
}

/// `LiquefactionRecipe`: an item dissolved in a solvent (`SizedFluidIngredient`) to a result.
#[derive(Clone, Debug, PartialEq)]
pub struct Liquefaction {
    pub id: String,
    pub ingredient: Ingredient,
    pub solvent: SizedFluidIngredient,
    pub result: ItemResult,
    pub time: i32,
}

impl Liquefaction {
    pub fn parse(id: &str, json: &Value) -> Result<Self, String> {
        let wrap = |e: String| format!("{id}: {e}");
        Ok(Self {
            id: id.to_owned(),
            ingredient: Ingredient::parse(field(json, "ingredient").map_err(wrap)?).map_err(wrap)?,
            solvent: SizedFluidIngredient::parse(field(json, "solvent").map_err(wrap)?).map_err(wrap)?,
            result: ItemResult::parse(field(json, "result").map_err(wrap)?).map_err(wrap)?,
            time: time(json),
        })
    }

    /// `matches`: the item fits and the solvent tank holds enough of a fitting fluid.
    pub fn matches(&self, item: &str, fluid: &str, amount: i32, items: Tags<'_>, fluids: Tags<'_>) -> bool {
        self.ingredient.test(item, items) && self.solvent.test(fluid, amount, fluids)
    }

    /// The `CachedCheck`'s item-only test.
    pub fn takes(&self, item: &str, tags: Tags<'_>) -> bool {
        self.ingredient.test(item, tags)
    }
}

/// `IncubationRecipe`: a mercury, a salt and a sulfur (one each, from the three vessels) to a result.
#[derive(Clone, Debug, PartialEq)]
pub struct Incubation {
    pub id: String,
    pub mercury: Ingredient,
    pub salt: Ingredient,
    pub sulfur: Ingredient,
    pub result: RecipeResult,
    pub time: i32,
}

impl Incubation {
    pub fn parse(id: &str, json: &Value) -> Result<Self, String> {
        let wrap = |e: String| format!("{id}: {e}");
        let ingredient = |name: &str| field(json, name).map_err(wrap).and_then(|v| Ingredient::parse(v).map_err(wrap));
        Ok(Self {
            id: id.to_owned(),
            mercury: ingredient("mercury")?,
            salt: ingredient("salt")?,
            sulfur: ingredient("sulfur")?,
            result: RecipeResult::parse(field(json, "result").map_err(wrap)?).map_err(wrap)?,
            time: time(json),
        })
    }

    /// `matches`: each vessel's item fits its ingredient.
    pub fn matches(&self, mercury: &str, salt: &str, sulfur: &str, tags: Tags<'_>) -> bool {
        self.mercury.test(mercury, tags) && self.salt.test(salt, tags) && self.sulfur.test(sulfur, tags)
    }
}

/// A fluid result (`FluidStack.CODEC`: `{"id", "amount"}`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FluidResult {
    pub id: String,
    pub amount: i32,
}

/// `AccumulationRecipe`: an optional evaporant fluid and an optional solute item to a fluid.
#[derive(Clone, Debug, PartialEq)]
pub struct Accumulation {
    pub id: String,
    pub evaporant: Option<SizedFluidIngredient>,
    pub solute: Option<Ingredient>,
    pub result: FluidResult,
    pub time: i32,
}

impl Accumulation {
    pub fn parse(id: &str, json: &Value) -> Result<Self, String> {
        let wrap = |e: String| format!("{id}: {e}");
        let evaporant = json.get("evaporant").map(SizedFluidIngredient::parse).transpose().map_err(wrap)?;
        let solute = json.get("solute").map(Ingredient::parse).transpose().map_err(wrap)?;
        let r = field(json, "result").map_err(wrap)?;
        let result = FluidResult {
            id: r.get("id").and_then(Value::as_str).ok_or_else(|| wrap("a fluid result without id".into()))?.to_owned(),
            amount: r.get("amount").and_then(Value::as_i64).and_then(|a| i32::try_from(a).ok()).ok_or_else(|| wrap("a fluid result without amount".into()))?,
        };
        Ok(Self { id: id.to_owned(), evaporant, solute, result, time: time(json) })
    }

    /// `matches`: the evaporant (if any) fits the water tank; a recipe with a solute needs it in the slot, and one
    /// without needs the slot empty (else water alone would always win over water and a crystal).
    pub fn matches(&self, solute: &str, fluid: &str, amount: i32, items: Tags<'_>, fluids: Tags<'_>) -> bool {
        let evaporant = self.evaporant.as_ref().is_none_or(|e| e.test(fluid, amount, fluids));
        let empty = solute.is_empty() || solute == "minecraft:air";
        let solute_ok = match &self.solute {
            None => empty,
            Some(s) => s.test(solute, items),
        };
        solute_ok && evaporant
    }

    /// The `CachedCheck`'s item-only test: a recipe with a solute that fits.
    pub fn takes(&self, item: &str, tags: Tags<'_>) -> bool {
        self.solute.as_ref().is_some_and(|s| s.test(item, tags))
    }

    /// The `CachedCheck`'s fluid-only test: a recipe with an evaporant whose fluid fits (any amount).
    pub fn takes_fluid(&self, fluid: &str, tags: Tags<'_>) -> bool {
        self.evaporant.as_ref().is_some_and(|e| e.ingredient.test(fluid, tags))
    }
}

/// `RecipeManager.getRecipeFor(type, input, level, lastRecipe)` as the `CachedCheck`s use it: the last recipe
/// found if it still fits, else the first in the table's order that does.
pub fn find<R>(recipes: &[R], last: Option<usize>, fits: impl Fn(&R) -> bool) -> Option<usize> {
    if let Some(i) = last
        && recipes.get(i).is_some_and(&fits)
    {
        return Some(i);
    }
    recipes.iter().position(fits)
}

/// Whether every ingredient of a recipe is one this port reads (for the count of recipes the machines run).
pub fn supported_fluid(f: &FluidIngredient) -> bool {
    f.supported()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn no_tags(_: &str, _: &str) -> bool {
        false
    }
    fn water_tag(f: &str, t: &str) -> bool {
        t == "minecraft:water" && (f == "minecraft:water" || f == "minecraft:flowing_water")
    }

    #[test]
    fn calcination_and_distillation() {
        let c = Calcination::parse(
            "theurgy:calcination/x",
            &json!({"ingredient":{"count":1,"ingredient":{"item":"minecraft:armadillo_scute"}},"result":{"count":1,"id":"theurgy:alchemical_salt_creature"},"type":"theurgy:calcination"}),
        )
        .unwrap();
        assert_eq!((c.time, c.result.id.as_str(), c.result.count), (DEFAULT_TIME, "theurgy:alchemical_salt_creature", 1));
        assert!(c.matches("minecraft:armadillo_scute", 1, &no_tags));
        let d = Distillation::parse(
            "theurgy:distillation/beef",
            &json!({"ingredient":{"count":2,"ingredient":{"item":"minecraft:beef"}},"result":{"count":1,"id":"theurgy:mercury_shard"},"time":300}),
        )
        .unwrap();
        assert_eq!(d.time, 300);
        assert!(!d.matches("minecraft:beef", 1, &no_tags) && d.takes("minecraft:beef", &no_tags), "one beef: found by item, not run");
        assert!(d.matches("minecraft:beef", 2, &no_tags));
    }

    #[test]
    fn liquefaction_needs_solvent() {
        let l = Liquefaction::parse(
            "theurgy:liquefaction/acacia",
            &json!({"ingredient":{"item":"minecraft:acacia_log"},"result":{"count":1,"id":"theurgy:alchemical_sulfur_acacia_log"},"solvent":{"amount":10,"ingredient":{"fluid":"theurgy:sal_ammoniac"}}}),
        )
        .unwrap();
        assert!(l.matches("minecraft:acacia_log", "theurgy:sal_ammoniac", 10, &no_tags, &no_tags));
        assert!(!l.matches("minecraft:acacia_log", "theurgy:sal_ammoniac", 9, &no_tags, &no_tags));
        assert!(!l.matches("minecraft:acacia_log", "", 0, &no_tags, &no_tags));
        assert!(l.takes("minecraft:acacia_log", &no_tags));
    }

    #[test]
    fn incubation_results() {
        let i = Incubation::parse(
            "theurgy:incubation/acacia_log",
            &json!({"mercury":{"item":"theurgy:mercury_shard"},"result":{"count":1,"id":"minecraft:acacia_log","type":"theurgy:item"},"salt":{"item":"theurgy:alchemical_salt_plant"},"sulfur":{"item":"theurgy:alchemical_sulfur_acacia_log"}}),
        )
        .unwrap();
        assert!(matches!(&i.result, RecipeResult::Item(r) if r.id == "minecraft:acacia_log" && r.json == json!({"count":1,"id":"minecraft:acacia_log"})));
        assert!(i.matches("theurgy:mercury_shard", "theurgy:alchemical_salt_plant", "theurgy:alchemical_sulfur_acacia_log", &no_tags));
        assert!(!i.matches("theurgy:mercury_shard", "", "theurgy:alchemical_sulfur_acacia_log", &no_tags));
        let t = RecipeResult::parse(&json!({"type":"theurgy:tag","tag":"c:ingots/iron","count":2})).unwrap();
        assert_eq!(t, RecipeResult::Tag { tag: "c:ingots/iron".into(), count: 2, components: None });
    }

    #[test]
    fn accumulation_solute_rules() {
        let water = Accumulation::parse("a", &json!({"evaporant":{"amount":1000,"ingredient":{"tag":"minecraft:water"}},"result":{"amount":100,"id":"theurgy:sal_ammoniac"},"time":100})).unwrap();
        let crystal = Accumulation::parse("b", &json!({"evaporant":{"amount":1000,"ingredient":{"tag":"minecraft:water"}},"solute":{"tag":"c:gems/sal_ammoniac"},"result":{"amount":1000,"id":"theurgy:sal_ammoniac"},"time":10})).unwrap();
        let items = |i: &str, t: &str| i == "theurgy:sal_ammoniac_crystal" && t == "c:gems/sal_ammoniac";
        assert!(water.matches("", "minecraft:water", 1000, &items, &water_tag));
        assert!(!water.matches("theurgy:sal_ammoniac_crystal", "minecraft:water", 1000, &items, &water_tag), "a solute in the slot rules water alone out");
        assert!(crystal.matches("theurgy:sal_ammoniac_crystal", "minecraft:water", 1000, &items, &water_tag));
        assert!(!crystal.matches("", "minecraft:water", 1000, &items, &water_tag));
        assert!(!water.matches("", "minecraft:water", 999, &items, &water_tag), "less than a bucket");
        let lava = Accumulation::parse("c", &json!({"result":{"amount":1000,"id":"minecraft:lava"},"solute":{"item":"theurgy:crystallized_lava"}})).unwrap();
        assert!(lava.matches("theurgy:crystallized_lava", "", 0, &items, &water_tag), "no evaporant: any tank");
        assert!(water.takes_fluid("minecraft:water", &water_tag) && !lava.takes_fluid("minecraft:water", &water_tag));
    }

    #[test]
    fn find_prefers_the_last_recipe() {
        let v = [1, 2, 3, 2];
        assert_eq!(find(&v, None, |x| *x == 2), Some(1));
        assert_eq!(find(&v, Some(3), |x| *x == 2), Some(3));
        assert_eq!(find(&v, Some(0), |x| *x == 2), Some(1));
        assert_eq!(find(&v, None, |x| *x == 9), None);
    }
}
