//! `OritechRecipe` and its codec (`OritechRecipeType.ORI_RECIPE_CODEC`): `time` (60 when left out), `ingredients`
//! (a list of non-empty ingredients, one item each), `results` (item stacks), and the optional `fluidInput` and
//! `fluidOutputs`. The item ingredients are resolved by the caller (tags and items to item ids), so the rules here stay
//! free of the game's registries.
use serde_json::Value;

/// An ingredient as the items it takes (`Ingredient.test`); its order is the game's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ingredient {
    pub items: Vec<String>,
}

impl Ingredient {
    pub fn test(&self, stack: Option<&Stack>) -> bool {
        stack.is_some_and(|s| s.count > 0 && self.items.iter().any(|i| *i == s.item))
    }
}

/// An item stack of a recipe's result or a machine slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stack {
    pub item: String,
    pub count: u32,
}

impl Stack {
    pub fn new(item: &str, count: u32) -> Self {
        Self { item: item.to_owned(), count }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    pub id: String,
    pub kind: String,
    pub time: i32,
    pub inputs: Vec<Ingredient>,
    pub results: Vec<Stack>,
    /// A fluid input or fluid outputs (none of the item machines' recipes in ATM10 has one).
    pub fluids: bool,
}

/// Reads a recipe. `resolve` turns one ingredient's JSON (an object, or a list of alternatives) into its items, none
/// when it names nothing the game has (the recipe is left out, as Java's codec fails on an unknown item).
pub fn parse(id: &str, json: &Value, resolve: impl Fn(&Value) -> Option<Vec<String>>) -> Result<Recipe, String> {
    let kind = json.get("type").and_then(Value::as_str).ok_or("no type")?.to_owned();
    let time = match json.get("time") {
        None => 60,
        Some(v) => v.as_i64().and_then(|t| i32::try_from(t).ok()).ok_or("time is not an int")?,
    };
    let list = json.get("ingredients").and_then(Value::as_array).ok_or("no ingredients")?;
    let mut inputs = Vec::with_capacity(list.len());
    for (i, ingredient) in list.iter().enumerate() {
        let items = resolve(ingredient).filter(|items| !items.is_empty()).ok_or_else(|| format!("ingredient {i} names nothing: {ingredient}"))?;
        inputs.push(Ingredient { items });
    }
    let results = json
        .get("results")
        .and_then(Value::as_array)
        .ok_or("no results")?
        .iter()
        .map(|r| {
            let item = r.get("id").or_else(|| r.get("item")).and_then(Value::as_str).ok_or("a result without an id")?;
            let count = match r.get("count") {
                None => 1,
                Some(c) => c.as_u64().and_then(|c| u32::try_from(c).ok()).filter(|c| *c > 0).ok_or("a bad count")?,
            };
            Ok(Stack::new(item, count))
        })
        .collect::<Result<Vec<_>, &str>>()?;
    let fluid_in = json.get("fluidInput").is_some_and(|f| f.get("amount").and_then(Value::as_i64).unwrap_or(1) > 0 && f.get("fluid").or_else(|| f.get("tag")).is_some());
    let fluid_out = json.get("fluidOutputs").and_then(Value::as_array).is_some_and(|l| !l.is_empty());
    Ok(Recipe { id: id.to_owned(), kind, time, inputs, results, fluids: fluid_in || fluid_out })
}

impl Recipe {
    /// `OritechRecipe.matches`: one ingredient matches the first slot (slot by slot for a single-ingredient recipe);
    /// more are matched anywhere in the inputs, one item each (`complexMatch`).
    pub fn matches(&self, input: &[Option<Stack>]) -> bool {
        if self.inputs.len() > 1 {
            return self.complex_match(input);
        }
        if input.len() < self.inputs.len() {
            return false;
        }
        self.inputs.iter().enumerate().all(|(i, ing)| ing.test(input[i].as_ref()))
    }

    fn complex_match(&self, input: &[Option<Stack>]) -> bool {
        let mut counts: Vec<u32> = input.iter().map(|s| s.as_ref().map_or(0, |s| s.count)).collect();
        for ing in &self.inputs {
            let found = input.iter().enumerate().position(|(i, s)| {
                counts[i] > 0 && s.as_ref().is_some_and(|s| ing.items.iter().any(|it| *it == s.item))
            });
            match found {
                Some(i) => counts[i] -= 1,
                None => return false,
            }
        }
        true
    }
}

/// A machine type's recipes, in the world's order (`RecipeManager.getRecipeFor` takes the first that matches).
#[derive(Clone, Debug, Default)]
pub struct RecipeBook {
    pub recipes: Vec<Recipe>,
}

impl RecipeBook {
    pub fn new(recipes: Vec<Recipe>) -> Self {
        Self { recipes }
    }

    pub fn get(&self, id: &str) -> Option<&Recipe> {
        self.recipes.iter().find(|r| r.id == id)
    }

    /// `getRecipeFor(type, input, level)`.
    pub fn find(&self, input: &[Option<Stack>]) -> Option<&Recipe> {
        self.recipes.iter().find(|r| r.matches(input))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Items for the tests' tags: `c:gems/diamond` and `c:ingots/nickel`.
    pub(crate) fn resolve(v: &Value) -> Option<Vec<String>> {
        match v {
            Value::Array(list) => {
                let all: Vec<String> = list.iter().filter_map(resolve).flatten().collect();
                (!all.is_empty()).then_some(all)
            }
            _ => {
                if let Some(item) = v.get("item").and_then(Value::as_str) {
                    return Some(vec![item.to_owned()]);
                }
                match v.get("tag").and_then(Value::as_str)? {
                    "c:gems/diamond" => Some(vec!["minecraft:diamond".into()]),
                    "c:ingots/nickel" => Some(vec!["alltheores:nickel_ingot".into(), "oritech:nickel_ingot".into()]),
                    "c:gems/amethyst" => Some(vec!["minecraft:amethyst_shard".into()]),
                    _ => None,
                }
            }
        }
    }

    fn some(item: &str, count: u32) -> Option<Stack> {
        Some(Stack::new(item, count))
    }

    #[test]
    fn the_codec_reads_oritechs_json() {
        let r = parse(
            "oritech:foundry/alloy/adamant",
            &json!({"ingredients":[{"tag":"c:gems/diamond"},{"tag":"c:ingots/nickel"}],"results":[{"count":1,"id":"oritech:adamant_ingot"}],"time":80,"type":"oritech:foundry"}),
            resolve,
        )
        .unwrap();
        assert_eq!((r.kind.as_str(), r.time, r.inputs.len(), r.fluids), ("oritech:foundry", 80, 2, false));
        assert_eq!(r.results, vec![Stack::new("oritech:adamant_ingot", 1)]);
        let untimed = parse("x", &json!({"type":"oritech:pulverizer","ingredients":[{"item":"a:b"}],"results":[{"id":"a:c"}]}), resolve).unwrap();
        assert_eq!((untimed.time, untimed.results[0].count), (60, 1), "time and count default to 60 and 1");
        let unknown = parse("y", &json!({"type":"oritech:pulverizer","ingredients":[{"tag":"c:nothing"}],"results":[{"id":"a:c"}]}), resolve);
        assert!(unknown.is_err(), "an ingredient that names nothing leaves the recipe out");
        let fluid = parse("z", &json!({"type":"oritech:centrifuge_fluid","ingredients":[{"item":"a:b"}],"results":[],"fluidOutputs":[{"fluid":"minecraft:water","amount":1000}]}), resolve).unwrap();
        assert!(fluid.fluids);
    }

    #[test]
    fn one_ingredient_matches_its_slot_and_more_match_anywhere() {
        let pulverize = parse("p", &json!({"type":"oritech:pulverizer","ingredients":[{"item":"minecraft:raw_iron"}],"results":[{"id":"oritech:iron_dust","count":2}]}), resolve).unwrap();
        assert!(pulverize.matches(&[some("minecraft:raw_iron", 1)]));
        assert!(!pulverize.matches(&[None, some("minecraft:raw_iron", 1)]), "the first slot only");
        assert!(!pulverize.matches(&[]));
        let alloy = parse("f", &json!({"type":"oritech:foundry","ingredients":[{"tag":"c:gems/diamond"},{"tag":"c:ingots/nickel"}],"results":[{"id":"oritech:adamant_ingot"}]}), resolve).unwrap();
        assert!(alloy.matches(&[some("oritech:nickel_ingot", 1), some("minecraft:diamond", 1)]), "either order");
        assert!(!alloy.matches(&[some("minecraft:diamond", 5), None]));
        let two = parse("a", &json!({"type":"oritech:assembler","ingredients":[{"tag":"c:gems/amethyst"},{"tag":"c:gems/amethyst"},{"item":"oritech:enderic_compound"}],"results":[{"id":"minecraft:budding_amethyst"}]}), resolve).unwrap();
        assert!(!two.matches(&[some("minecraft:amethyst_shard", 1), some("oritech:enderic_compound", 1), None, None]), "one item an ingredient");
        assert!(two.matches(&[some("minecraft:amethyst_shard", 2), some("oritech:enderic_compound", 1), None, None]));
        assert!(two.matches(&[some("minecraft:amethyst_shard", 1), some("oritech:enderic_compound", 1), some("minecraft:amethyst_shard", 1), None]));
        let book = RecipeBook::new(vec![pulverize.clone(), alloy]);
        assert_eq!(book.find(&[some("minecraft:raw_iron", 3)]).map(|r| r.id.as_str()), Some("p"));
        assert_eq!(book.get("f").map(|r| r.inputs.len()), Some(2));
    }

    /// The carry's own recipes, when `ORITECH_CARRY` names it: every recipe of the four ported types reads, with
    /// any tag counted as one item (the tags are the world's to resolve).
    #[test]
    fn the_carrys_recipes_read() {
        let Ok(carry) = std::env::var("ORITECH_CARRY") else { return };
        let any = |v: &Value| -> Option<Vec<String>> {
            match v {
                Value::Array(l) => Some(l.iter().filter_map(|x| x.get("item").or_else(|| x.get("tag")).and_then(Value::as_str).map(str::to_owned)).collect()),
                _ => v.get("item").or_else(|| v.get("tag")).and_then(Value::as_str).map(|s| vec![s.to_owned()]),
            }
        };
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        let mut stack = vec![std::path::PathBuf::from(carry).join("data/oritech/recipe")];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("the carry's recipes") {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let json: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
                let kind = json.get("type").and_then(Value::as_str).unwrap_or_default();
                if ["oritech:pulverizer", "oritech:grinder", "oritech:assembler", "oritech:foundry"].contains(&kind) {
                    let r = parse(&path.display().to_string(), &json, any).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                    assert!(!r.fluids && r.time > 0, "{}", r.id);
                    *counts.entry(kind.to_owned()).or_default() += 1;
                }
            }
        }
        assert_eq!(counts.get("oritech:pulverizer"), Some(&115));
        assert_eq!(counts.get("oritech:grinder"), Some(&107));
        assert_eq!(counts.get("oritech:assembler"), Some(&28));
        assert_eq!(counts.get("oritech:foundry"), Some(&26));
    }

    /// ATM10's own recipes, when `ORITECH_ATM10_DUMP` names the full pack's dump (`recipes.jsonl`, `tags.jsonl` and
    /// `registries.json` of the oracle's run; ATM10's data, read in place and never committed): every recipe of the
    /// ported types reads with its tags resolved to registered items, except Productive Bees' five bee
    /// conversions, whose `productivebees:component` ingredients match a spawn egg's components.
    #[test]
    fn atm10s_recipes_resolve() {
        let Ok(dump) = std::env::var("ORITECH_ATM10_DUMP") else { return };
        let dump = std::path::PathBuf::from(dump);
        let registries: Value = serde_json::from_str(&std::fs::read_to_string(dump.join("registries.json")).unwrap()).unwrap();
        let items: std::collections::HashSet<&str> = registries["minecraft:item"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        let mut tags = std::collections::HashMap::<String, Vec<String>>::new();
        for line in std::fs::read_to_string(dump.join("tags.jsonl")).unwrap().lines() {
            let t: Value = serde_json::from_str(line).unwrap();
            if t["registry"] == "minecraft:item" {
                tags.insert(t["id"].as_str().unwrap().to_owned(), t["values"].as_array().unwrap().iter().filter_map(Value::as_str).map(str::to_owned).collect());
            }
        }
        fn resolve(v: &Value, items: &std::collections::HashSet<&str>, tags: &std::collections::HashMap<String, Vec<String>>) -> Option<Vec<String>> {
            let known = |list: Vec<String>| -> Option<Vec<String>> {
                let list: Vec<String> = list.into_iter().filter(|i| items.contains(i.as_str())).collect();
                (!list.is_empty()).then_some(list)
            };
            match v {
                Value::Array(list) => known(list.iter().filter_map(|x| resolve(x, items, tags)).flatten().collect()),
                Value::Object(o) if o.contains_key("type") => None,
                _ => match (v.get("item").and_then(Value::as_str), v.get("tag").and_then(Value::as_str)) {
                    (Some(item), _) => known(vec![item.to_owned()]),
                    (None, Some(tag)) => known(tags.get(tag).cloned().unwrap_or_default()),
                    _ => None,
                },
            }
        }
        let mut counts = std::collections::BTreeMap::<String, (usize, usize)>::new();
        let mut all = 0;
        for line in std::fs::read_to_string(dump.join("recipes.jsonl")).unwrap().lines() {
            let r: Value = serde_json::from_str(line).unwrap();
            let kind = r["type"].as_str().unwrap();
            if !kind.starts_with("oritech:") {
                continue;
            }
            all += 1;
            if !crate::machine::ALL.iter().any(|s| s.recipe_type == kind) {
                continue;
            }
            let id = r["id"].as_str().unwrap();
            let read = parse(id, &r["json"], |v| resolve(v, &items, &tags)).ok().filter(|p| p.results.iter().all(|s| items.contains(s.item.as_str())));
            let entry = counts.entry(kind.to_owned()).or_default();
            entry.0 += 1;
            entry.1 += usize::from(read.is_some());
            if read.is_none() {
                assert!(id.starts_with("productivebees:"), "{id} doesn't read");
            }
        }
        let got: Vec<(&str, usize, usize)> = counts.iter().map(|(k, (t, p))| (k.as_str(), *t, *p)).collect();
        assert_eq!(
            got,
            vec![
                ("oritech:assembler", 28, 28),
                ("oritech:bio_generator", 7, 7),
                ("oritech:centrifuge", 15, 15),
                ("oritech:foundry", 30, 26),
                ("oritech:grinder", 108, 107),
                ("oritech:pulverizer", 115, 115)
            ]
        );
        assert_eq!(all, 469, "ATM10's recipes of Oritech's own types");
        println!("ATM10: {} of {all} recipes of Oritech's types run on a ported machine", got.iter().map(|g| g.2).sum::<usize>());
    }
}
