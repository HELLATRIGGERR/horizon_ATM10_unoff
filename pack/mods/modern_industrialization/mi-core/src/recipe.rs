//! `MachineRecipe` and `MachineRecipeType` (machines/recipe/): the JSON MI's codec reads, the checks of
//! `MachineRecipeType.validateRecipe`, and the per-item index `getMatchingRecipes` keeps.

use serde_json::Value;
use std::collections::HashMap;
use std::rc::Rc;

/// `MachineRecipe.ItemInput`: an ingredient's items (resolved by the caller: items, tags, NeoForge's
/// custom ingredients), how many, and the chance it is used up.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemInput {
    pub items: Vec<String>,
    pub amount: u32,
    pub probability: f32,
}

impl ItemInput {
    /// `ItemInput.matches`: the ingredient takes this item.
    pub fn matches(&self, item: &str) -> bool {
        self.items.iter().any(|i| i == item)
    }
}

/// `MachineRecipe.FluidInput`: the fluids its `FluidIngredient` takes, in millibuckets.
#[derive(Clone, Debug, PartialEq)]
pub struct FluidInput {
    pub fluids: Vec<String>,
    pub amount: i64,
    pub probability: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ItemOutput {
    pub item: String,
    pub amount: u32,
    pub probability: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FluidOutput {
    pub fluid: String,
    pub amount: i64,
    pub probability: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MachineRecipe {
    pub id: String,
    /// EU a tick at the recipe's own rate (also the forge hammer's damage).
    pub eu: u32,
    pub duration: u32,
    pub item_inputs: Vec<ItemInput>,
    pub fluid_inputs: Vec<FluidInput>,
    pub item_outputs: Vec<ItemOutput>,
    pub fluid_outputs: Vec<FluidOutput>,
}

impl MachineRecipe {
    /// `getTotalEu`.
    pub fn total_eu(&self) -> i64 {
        i64::from(self.eu) * i64::from(self.duration)
    }
}

/// What a recipe type allows (`withItemInputs()`...), as `MIMachineRecipeTypes` creates it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecipeTypeRules {
    pub item_inputs: bool,
    pub fluid_inputs: bool,
    pub item_outputs: bool,
    pub fluid_outputs: bool,
}

/// `MIExtraCodecs.maybeList`: one object or a list of them; absent is empty.
fn maybe_list(v: Option<&Value>) -> Vec<&Value> {
    match v {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(a)) => a.iter().collect(),
        Some(one) => vec![one],
    }
}

/// `ExtraCodecs.POSITIVE_INT` and `MIExtraCodecs.POSITIVE_LONG`.
fn positive(v: &Value, name: &str) -> Result<i64, String> {
    let n = v.get(name).and_then(Value::as_i64).ok_or_else(|| format!("no {name}"))?;
    if n <= 0 {
        return Err(format!("{name} {n} is not positive"));
    }
    Ok(n)
}

/// `AMOUNT_CODEC`: `amount`, or `count`, or 1.
fn amount_or_count(v: &Value) -> Result<i64, String> {
    for name in ["amount", "count"] {
        if v.get(name).is_some() {
            return positive(v, name);
        }
    }
    Ok(1)
}

/// `MIExtraCodecs.FLOAT_01`, 1 when absent.
fn probability(v: &Value) -> Result<f32, String> {
    match v.get("probability") {
        None => Ok(1.0),
        Some(p) => {
            let p = p.as_f64().ok_or("probability is not a number")? as f32;
            if (0.0..=1.0).contains(&p) { Ok(p) } else { Err(format!("probability {p} outside 0..1")) }
        }
    }
}

/// The ingredient part of an input object: everything but MI's own fields, which share the map
/// (`Ingredient.MAP_CODEC_NONEMPTY` beside `amount` and `probability`).
fn ingredient_part(v: &Value) -> Value {
    let mut o = v.as_object().cloned().unwrap_or_default();
    for name in ["amount", "count", "probability"] {
        o.remove(name);
    }
    Value::Object(o)
}

/// Reads a recipe file as `MachineRecipe.codec` does, then `validateRecipe`. `items` resolves an item
/// ingredient (`{"item": ...}`, `{"tag": ...}`, a NeoForge custom ingredient) to the items it takes, none
/// when nothing is registered; `fluids` does the same for a `FluidIngredient`.
pub fn parse(
    id: &str,
    json: &Value,
    rules: RecipeTypeRules,
    items: impl Fn(&Value) -> Option<Vec<String>>,
    fluids: impl Fn(&Value) -> Option<Vec<String>>,
) -> Result<MachineRecipe, String> {
    let eu = positive(json, "eu")?.min(i64::from(i32::MAX)) as u32;
    let duration = positive(json, "duration")?.min(i64::from(i32::MAX)) as u32;
    if !maybe_list(json.get("process_conditions")).is_empty() {
        return Err("process conditions are not ported".into());
    }
    let mut item_inputs = Vec::new();
    for v in maybe_list(json.get("item_inputs")) {
        let found = items(&ingredient_part(v)).filter(|l| !l.is_empty()).ok_or_else(|| format!("no item for {v}"))?;
        item_inputs.push(ItemInput { items: found, amount: amount_or_count(v)?.min(i64::from(i32::MAX)) as u32, probability: probability(v)? });
    }
    let mut fluid_inputs = Vec::new();
    for v in maybe_list(json.get("fluid_inputs")) {
        let found = fluids(&ingredient_part(v)).filter(|l| !l.is_empty()).ok_or_else(|| format!("no fluid for {v}"))?;
        let amount = if v.get("amount").is_some() { positive(v, "amount")? } else { 1 };
        fluid_inputs.push(FluidInput { fluids: found, amount, probability: probability(v)? });
    }
    let mut item_outputs = Vec::new();
    for v in maybe_list(json.get("item_outputs")) {
        let item = v.get("item").and_then(Value::as_str).ok_or("an item output has no item")?;
        if items(&serde_json::json!({ "item": item })).is_none_or(|l| l.is_empty()) {
            return Err(format!("unknown output item {item}"));
        }
        item_outputs.push(ItemOutput { item: item.to_owned(), amount: amount_or_count(v)?.min(i64::from(i32::MAX)) as u32, probability: probability(v)? });
    }
    let mut fluid_outputs = Vec::new();
    for v in maybe_list(json.get("fluid_outputs")) {
        let fluid = v.get("fluid").and_then(Value::as_str).ok_or("a fluid output has no fluid")?;
        let amount = if v.get("amount").is_some() { positive(v, "amount")? } else { 1 };
        fluid_outputs.push(FluidOutput { fluid: fluid.to_owned(), amount, probability: probability(v)? });
    }
    let recipe = MachineRecipe { id: id.to_owned(), eu, duration, item_inputs, fluid_inputs, item_outputs, fluid_outputs };
    validate(&recipe, rules)?;
    Ok(recipe)
}

/// `MachineRecipeType.validateRecipe`.
pub fn validate(r: &MachineRecipe, rules: RecipeTypeRules) -> Result<(), String> {
    if !rules.item_inputs && !r.item_inputs.is_empty() {
        return Err("Item inputs are not allowed.".into());
    }
    if !rules.fluid_inputs && !r.fluid_inputs.is_empty() {
        return Err("Fluid inputs are not allowed.".into());
    }
    if !rules.item_outputs && !r.item_outputs.is_empty() {
        return Err("Item outputs are not allowed.".into());
    }
    if !rules.fluid_outputs && !r.fluid_outputs.is_empty() {
        return Err("Fluid outputs are not allowed.".into());
    }
    if r.item_inputs.len() + r.fluid_inputs.len() == 0 {
        return Err("Must have at least one fluid or item input.".into());
    }
    if r.item_outputs.len() + r.fluid_outputs.len() == 0 {
        return Err("Must have at least one fluid or item output.".into());
    }
    Ok(())
}

/// One recipe type's recipes with `MachineRecipeType`'s cache: the recipes by each item of their first item
/// input, and those with fluid inputs only.
#[derive(Clone, Debug, Default)]
pub struct RecipeBook {
    pub recipes: Vec<Rc<MachineRecipe>>,
    by_item: HashMap<String, Vec<usize>>,
    fluid_only: Vec<usize>,
}

impl RecipeBook {
    pub fn new(recipes: Vec<MachineRecipe>) -> Self {
        let mut book = RecipeBook { recipes: recipes.into_iter().map(Rc::new).collect(), ..Self::default() };
        for (i, r) in book.recipes.iter().enumerate() {
            match r.item_inputs.first() {
                None if !r.fluid_inputs.is_empty() => book.fluid_only.push(i),
                None => {}
                Some(first) => {
                    let mut seen: Vec<&str> = Vec::new();
                    for item in &first.items {
                        if !seen.contains(&item.as_str()) {
                            seen.push(item);
                            book.by_item.entry(item.clone()).or_default().push(i);
                        }
                    }
                }
            }
        }
        book
    }

    pub fn get(&self, id: &str) -> Option<&Rc<MachineRecipe>> {
        self.recipes.iter().find(|r| r.id == id)
    }

    /// `CrafterComponent.getRecipes(level, type, itemInputs)`: the fluid-only recipes and those whose
    /// first input takes an item in an input slot. MI collects them in a `HashSet`, whose order Java
    /// doesn't fix; here they come in file order.
    pub fn candidates<'a>(&self, input_items: impl Iterator<Item = &'a str>) -> Vec<Rc<MachineRecipe>> {
        let mut picked: Vec<usize> = self.fluid_only.clone();
        for item in input_items {
            if let Some(list) = self.by_item.get(item) {
                picked.extend(list);
            }
        }
        picked.sort_unstable();
        picked.dedup();
        picked.into_iter().map(|i| self.recipes[i].clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const MACERATOR: RecipeTypeRules = RecipeTypeRules { item_inputs: true, fluid_inputs: false, item_outputs: true, fluid_outputs: false };
    const CUTTING: RecipeTypeRules = RecipeTypeRules { item_inputs: true, fluid_inputs: true, item_outputs: true, fluid_outputs: false };

    fn items(v: &Value) -> Option<Vec<String>> {
        if let Some(i) = v.get("item").and_then(Value::as_str) {
            return (i != "missing:item").then(|| vec![i.to_owned()]);
        }
        match v.get("tag").and_then(Value::as_str)? {
            "c:ores/copper" => Some(vec!["minecraft:copper_ore".into(), "minecraft:deepslate_copper_ore".into()]),
            _ => None,
        }
    }
    fn fluids(v: &Value) -> Option<Vec<String>> {
        v.get("fluid").and_then(Value::as_str).map(|f| vec![f.to_owned()])
    }

    #[test]
    fn reads_atm10_macerator_recipe() {
        // compat/create/macerator/asurine.json's shape: a probability output.
        let j = json!({"duration":100,"eu":2,"item_inputs":[{"amount":1,"tag":"c:ores/copper"}],
            "item_outputs":[{"amount":2,"item":"mi:copper_dust"},{"amount":1,"item":"mi:gold_dust","probability":0.25}],
            "type":"modern_industrialization:macerator"});
        let r = parse("mi:t", &j, MACERATOR, items, fluids).unwrap();
        assert_eq!((r.eu, r.duration, r.total_eu()), (2, 100, 200));
        assert_eq!(r.item_inputs[0].items.len(), 2);
        assert!(r.item_inputs[0].matches("minecraft:deepslate_copper_ore"));
        assert_eq!(r.item_outputs[1].probability, 0.25);
        assert_eq!(r.item_outputs[0].amount, 2);
    }

    #[test]
    fn single_objects_count_alias_and_fluids() {
        let j = json!({"eu":2,"duration":200,"item_inputs":{"count":3,"item":"a:b"},
            "fluid_inputs":{"fluid":"modern_industrialization:lubricant","amount":1},"item_outputs":{"item":"a:c"}});
        let r = parse("mi:c", &j, CUTTING, items, fluids).unwrap();
        assert_eq!(r.item_inputs[0].amount, 3);
        assert_eq!(r.fluid_inputs[0].fluids, vec!["modern_industrialization:lubricant".to_owned()]);
        assert_eq!(r.item_outputs[0].amount, 1);
    }

    #[test]
    fn refuses_what_mi_refuses() {
        let fluid_in = json!({"eu":2,"duration":1,"item_inputs":[{"item":"a:b"}],"fluid_inputs":[{"fluid":"x:y","amount":1}],"item_outputs":[{"item":"a:c"}]});
        assert_eq!(parse("x", &fluid_in, MACERATOR, items, fluids).unwrap_err(), "Fluid inputs are not allowed.");
        let zero = json!({"eu":0,"duration":1,"item_inputs":[{"item":"a:b"}],"item_outputs":[{"item":"a:c"}]});
        assert!(parse("x", &zero, MACERATOR, items, fluids).is_err());
        let unknown = json!({"eu":2,"duration":1,"item_inputs":[{"item":"missing:item"}],"item_outputs":[{"item":"a:c"}]});
        assert!(parse("x", &unknown, MACERATOR, items, fluids).is_err());
        let no_out = json!({"eu":2,"duration":1,"item_inputs":[{"item":"a:b"}]});
        assert_eq!(parse("x", &no_out, MACERATOR, items, fluids).unwrap_err(), "Must have at least one fluid or item output.");
        let half = json!({"eu":2,"duration":1,"item_inputs":[{"item":"a:b","probability":1.5}],"item_outputs":[{"item":"a:c"}]});
        assert!(parse("x", &half, MACERATOR, items, fluids).is_err());
    }

    #[test]
    fn book_indexes_first_input_items() {
        let mk = |id: &str, item: &str| MachineRecipe {
            id: id.into(), eu: 2, duration: 1,
            item_inputs: vec![ItemInput { items: vec![item.into()], amount: 1, probability: 1.0 }],
            fluid_inputs: vec![], item_outputs: vec![ItemOutput { item: "o:o".into(), amount: 1, probability: 1.0 }], fluid_outputs: vec![],
        };
        let book = RecipeBook::new(vec![mk("r0", "a:a"), mk("r1", "b:b"), mk("r2", "a:a")]);
        let ids: Vec<String> = book.candidates(["a:a", "c:c"].into_iter()).iter().map(|r| r.id.clone()).collect();
        assert_eq!(ids, ["r0", "r2"]);
        assert!(book.candidates(std::iter::empty()).is_empty());
    }
}
