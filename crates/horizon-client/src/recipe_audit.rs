//! The recipe audit (docs/atm10-port.md, "Recipes that run"): every recipe of vanilla's own types among a world's
//! recipe sources, checked the way the game reads and runs it.
//!
//! 1. *parsed*: the game's tables hold it (`Recipes` for crafting and cooking, `StationRecipes` for the stonecutter
//!    and the smithing table);
//! 2. *resolved*: every ingredient and the result name items this world registers, tags expanded;
//! 3. *ran*: its ingredients, the first registered item of each, go where the game's menu puts them (a 3x3 crafting
//!    grid, a furnace's input, the stonecutter's, the smithing table's three slots) and the engine's own matcher
//!    answers (`Recipes::find`, `Recipes::cook`, `StationRecipes::stonecutter`, `StationRecipes::smithing_recipe`);
//! 4. *matched*: that answer is the recipe's result (item and count).
//!
//! A recipe that fails says why, as one of [`Cause`]. A type of a mod's own is counted, not run.
use crate::crafting::{Ingredient, Recipe, Recipes, qualified};
use crate::items::{ItemStack, Items};
use crate::stations::{Smithing, StationData};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};

/// The types audited: vanilla's crafting, cooking, stonecutting and smithing recipes.
pub const AUDITED: &[&str] = &[
    "minecraft:crafting_shaped",
    "minecraft:crafting_shapeless",
    "minecraft:smelting",
    "minecraft:blasting",
    "minecraft:smoking",
    "minecraft:campfire_cooking",
    "minecraft:stonecutting",
    "minecraft:smithing_transform",
    "minecraft:smithing_trim",
];

/// Why a recipe doesn't run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Cause {
    /// An ingredient or the result names an item this world doesn't register.
    UnknownItem,
    /// An ingredient names a tag this world doesn't have, or one with no item here.
    UnknownTag,
    /// A field the parser doesn't read: a shape it doesn't know, a missing key.
    UnparsedField,
    /// The matcher, given the recipe's own ingredients, answers nothing or another result.
    MatcherMismatch,
    /// It matches, but an earlier recipe with another result takes the same input first (the pack's own conflict:
    /// the first match in registration order wins, as `RecipeManager.getRecipeFor`'s `findFirst`).
    AnotherRecipeFirst,
}
impl Cause {
    pub fn name(self) -> &'static str {
        match self {
            Cause::UnknownItem => "unknown item",
            Cause::UnknownTag => "unknown tag",
            Cause::UnparsedField => "unparsed field",
            Cause::MatcherMismatch => "matcher mismatch",
            Cause::AnotherRecipeFirst => "another recipe first",
        }
    }
}
type Problem = (Cause, String);

/// What one recipe came to.
#[derive(Clone, Debug, Default)]
pub struct Outcome {
    pub parsed: bool,
    pub resolved: bool,
    pub ran: bool,
    pub matched: bool,
    pub failure: Option<Problem>,
}

struct Ctx<'a> {
    items: &'a Items,
    recipes: &'a Recipes,
    stations: &'a StationData,
    crafting: HashMap<&'a str, &'a Recipe>,
    cooking: HashMap<&'a str, &'a crate::crafting::Cooking>,
    stonecutting: HashMap<String, &'a crate::stations::Stonecutting>,
    smithing: HashMap<String, &'a Smithing>,
}

impl Ctx<'_> {
    fn registered(&self, id: &str) -> bool {
        id != "minecraft:air" && self.items.items.contains_key(id)
    }
    /// The first problem of one ingredient as the source names it.
    fn ingredient(&self, field: &str, v: &Value) -> Option<Problem> {
        match v {
            Value::String(s) => match s.strip_prefix('#') {
                Some(tag) => match self.items.item_tag_order.get(tag) {
                    None => Some((Cause::UnknownTag, format!("#{tag}"))),
                    Some(list) if !list.iter().any(|i| self.registered(i)) => {
                        Some((Cause::UnknownTag, format!("#{tag} (no item here)")))
                    }
                    Some(_) => None,
                },
                None => (!self.registered(s)).then(|| (Cause::UnknownItem, s.clone())),
            },
            Value::Array(list) if list.is_empty() => Some((Cause::UnparsedField, format!("{field}: empty list"))),
            // A list of items is one `HolderSet`: every one must exist.
            Value::Array(list) => list.iter().find_map(|e| self.ingredient(field, e)),
            Value::Object(o) => match Ingredient::parse(v, self.items) {
                Some(i) if i.order.iter().any(|i| self.registered(i)) => None,
                Some(i) => Some((Cause::UnknownItem, i.order.first().cloned().unwrap_or_default())),
                None if o.contains_key("item") || o.contains_key("tag") => {
                    Some((Cause::UnparsedField, format!("{field}: {{item}}/{{tag}} object")))
                }
                None => {
                    let kind = o.get("neoforge:ingredient_type").or_else(|| o.get("type")).and_then(Value::as_str);
                    Some((Cause::UnparsedField, format!("{field}: ingredient type {}", kind.unwrap_or("none"))))
                }
            },
            Value::Null => Some((Cause::UnparsedField, format!("{field}: missing"))),
            _ => Some((Cause::UnparsedField, format!("{field}: not an ingredient"))),
        }
    }
    fn result(&self, v: &Value) -> Option<Problem> {
        let r = &v["result"];
        match horizon_world::loot::recipe_result_id(r) {
            None => Some((Cause::UnparsedField, "result: no item".into())),
            Some(_) if crate::crafting::template_count(r).is_none() => {
                Some((Cause::UnparsedField, "result: count outside 1..99".into()))
            }
            Some(id) if !self.registered(id) => Some((Cause::UnknownItem, format!("{id} (result)"))),
            Some(_) => None,
        }
    }
    /// Every problem the source shows, the first one first.
    fn source_problem(&self, kind: &str, v: &Value) -> Option<Problem> {
        let mut fields: Vec<(String, &Value)> = Vec::new();
        match kind {
            "minecraft:crafting_shaped" => {
                let rows: Vec<&str> = v["pattern"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                if rows.is_empty() {
                    return Some((Cause::UnparsedField, "pattern: missing".into()));
                }
                if rows.len() > 3 || rows.iter().any(|r| r.chars().count() > 3) {
                    return Some((Cause::UnparsedField, "pattern: larger than 3x3".into()));
                }
                for c in rows.iter().flat_map(|r| r.chars()).filter(|c| *c != ' ') {
                    fields.push((format!("key.{c}"), &v["key"][c.to_string()]));
                }
            }
            "minecraft:crafting_shapeless" => match v["ingredients"].as_array() {
                None => return Some((Cause::UnparsedField, "ingredients: missing".into())),
                Some(list) if list.is_empty() || list.len() > 9 => {
                    return Some((Cause::UnparsedField, format!("ingredients: {} of them", list.len())));
                }
                Some(list) => fields.extend(list.iter().map(|i| ("ingredients[]".to_owned(), i))),
            },
            "minecraft:smithing_transform" => {
                fields.push(("base".into(), &v["base"]));
                for key in ["template", "addition"] {
                    if let Some(i) = v.get(key) {
                        fields.push((key.into(), i));
                    }
                }
            }
            "minecraft:smithing_trim" => {
                for key in ["template", "base", "addition"] {
                    fields.push((key.into(), &v[key]));
                }
            }
            _ => fields.push(("ingredient".into(), &v["ingredient"])),
        }
        fields
            .iter()
            .find_map(|(field, i)| self.ingredient(field, i))
            .or_else(|| (kind != "minecraft:smithing_trim").then(|| self.result(v)).flatten())
    }
    /// The stack the test puts in a slot for an ingredient: its first registered item, with the components it asks for.
    fn probe(&self, i: &Ingredient) -> Option<ItemStack> {
        i.order.iter().find(|id| self.registered(id)).map(|id| i.example(id))
    }
    fn audit(&self, name: &str, kind: &str, v: &Value) -> Outcome {
        let mut out = Outcome::default();
        let problem = self.source_problem(kind, v);
        let fail = |mut out: Outcome, p: Problem| {
            out.failure = Some(p);
            out
        };
        // 26.3's trim recipe names its pattern; 1.21.1's took it from the template.
        let unparsed = || {
            problem.clone().unwrap_or_else(|| match kind {
                "minecraft:smithing_trim" if v.get("pattern").is_none() => (Cause::UnparsedField, "pattern: missing".into()),
                _ => (Cause::UnparsedField, "not in the tables".into()),
            })
        };
        let id = qualified(name);
        let same = |got: &ItemStack, want: &ItemStack| got.item == want.item && got.count == want.count;
        match kind {
            "minecraft:crafting_shaped" | "minecraft:crafting_shapeless" => {
                let Some(recipe) = self.crafting.get(name) else { return fail(out, unparsed()) };
                out.parsed = true;
                if let Some(p) = problem {
                    return fail(out, p);
                }
                let mut grid: Vec<Option<ItemStack>> = vec![None; 9];
                let want = match recipe {
                    Recipe::Shaped { width, height, cells, result } => {
                        if *width > 3 || *height > 3 {
                            return fail(out, (Cause::UnparsedField, "pattern: larger than 3x3".into()));
                        }
                        for y in 0..*height {
                            for x in 0..*width {
                                if let Some(i) = &cells[y * width + x] {
                                    let Some(s) = self.probe(i) else { return fail(out, (Cause::UnknownItem, "ingredient".into())) };
                                    grid[y * 3 + x] = Some(s);
                                }
                            }
                        }
                        result.create()
                    }
                    Recipe::Shapeless { ingredients, result, .. } => {
                        if ingredients.len() > 9 {
                            return fail(out, (Cause::UnparsedField, format!("ingredients: {} of them", ingredients.len())));
                        }
                        for (slot, i) in ingredients.iter().enumerate() {
                            let Some(s) = self.probe(i) else { return fail(out, (Cause::UnknownItem, "ingredient".into())) };
                            grid[slot] = Some(s);
                        }
                        result.create()
                    }
                    Recipe::Special(_) => return fail(out, (Cause::UnparsedField, "read as a special recipe".into())),
                };
                out.resolved = true;
                out.ran = true;
                match self.recipes.find(&grid, 3) {
                    Some((_, got)) if same(&got, &want) => out.matched = true,
                    Some((other, got)) if other != name && self.recipes.recipe_matches(&id, &grid, 3) => {
                        return fail(out, (Cause::AnotherRecipeFirst, format!("{} gives {} x{}", qualified(other), got.item, got.count)));
                    }
                    Some((_, got)) => return fail(out, (Cause::MatcherMismatch, format!("gave {} x{}", got.item, got.count))),
                    None => return fail(out, (Cause::MatcherMismatch, "no result".into())),
                }
            }
            "minecraft:smelting" | "minecraft:blasting" | "minecraft:smoking" | "minecraft:campfire_cooking" => {
                let Some(recipe) = self.cooking.get(name) else { return fail(out, unparsed()) };
                out.parsed = true;
                if let Some(p) = problem {
                    return fail(out, p);
                }
                let Some(input) = self.probe(&recipe.ingredient) else { return fail(out, (Cause::UnknownItem, "ingredient".into())) };
                out.resolved = true;
                out.ran = true;
                match self.recipes.cook(kind, &input) {
                    Some(c) if same(&c.result, &recipe.result) => out.matched = true,
                    Some(c) if c.id != recipe.id && recipe.ingredient.test(&input) => {
                        return fail(out, (Cause::AnotherRecipeFirst, format!("{} gives {}", qualified(&c.id), c.result.item)));
                    }
                    Some(c) => return fail(out, (Cause::MatcherMismatch, format!("gave {}", c.result.item))),
                    None => return fail(out, (Cause::MatcherMismatch, "no result".into())),
                }
            }
            "minecraft:stonecutting" => {
                let Some(recipe) = self.stonecutting.get(&id).filter(|r| !r.input.order.is_empty()) else {
                    return fail(out, unparsed());
                };
                out.parsed = true;
                if let Some(p) = problem {
                    return fail(out, p);
                }
                let Some(input) = self.probe(&recipe.input) else { return fail(out, (Cause::UnknownItem, "ingredient".into())) };
                out.resolved = true;
                out.ran = true;
                let listed = self.stations.recipes.stonecutter(&input);
                match listed.iter().find(|r| r.id == id) {
                    Some(r) if same(&r.result, &recipe.result) => out.matched = true,
                    Some(r) => return fail(out, (Cause::MatcherMismatch, format!("gave {}", r.result.item))),
                    None => return fail(out, (Cause::MatcherMismatch, format!("not listed ({} others)", listed.len()))),
                }
            }
            _ => {
                let Some(recipe) = self.smithing.get(&id) else { return fail(out, unparsed()) };
                out.parsed = true;
                if let Some(p) = problem {
                    return fail(out, p);
                }
                let slot = |i: Option<&Ingredient>| match i {
                    None => Ok(None),
                    Some(i) => self.probe(i).map(Some).ok_or((Cause::UnknownItem, "ingredient".to_owned())),
                };
                let (slots, want) = match recipe {
                    Smithing::Transform { template, base, addition, .. } => (
                        [slot(template.as_ref()), slot(Some(base)), slot(addition.as_ref())],
                        horizon_world::loot::recipe_result_id(&v["result"]).map(str::to_owned),
                    ),
                    Smithing::Trim { template, base, addition, .. } => {
                        ([slot(Some(template)), slot(Some(base)), slot(Some(addition))], None)
                    }
                };
                let [t, b, a] = match slots {
                    [Ok(t), Ok(b), Ok(a)] => [t, b, a],
                    [t, b, a] => return fail(out, [t, b, a].into_iter().find_map(Result::err).unwrap_or((Cause::UnknownItem, String::new()))),
                };
                out.resolved = true;
                out.ran = true;
                let want = want.or_else(|| b.as_ref().map(|b| b.item.to_string())).unwrap_or_default();
                match self.stations.recipes.smithing_recipe(&self.stations.enchantments, t.as_ref(), b.as_ref(), a.as_ref()) {
                    Some((_, got)) if *got.item == *want => out.matched = true,
                    Some((other, got)) if other != id => {
                        return fail(out, (Cause::AnotherRecipeFirst, format!("{other} gives {}", got.item)));
                    }
                    Some((_, got)) => return fail(out, (Cause::MatcherMismatch, format!("gave {}", got.item))),
                    None => return fail(out, (Cause::MatcherMismatch, "no result".into())),
                }
            }
        }
        out
    }
}

/// The namespace a recipe's id is in.
fn namespace(name: &str) -> &str {
    name.split_once(':').map_or("minecraft", |(ns, _)| ns)
}

#[derive(Default)]
struct Counts {
    total: usize,
    parsed: usize,
    resolved: usize,
    ran: usize,
    matched: usize,
}
impl Counts {
    fn add(&mut self, o: &Outcome) {
        self.total += 1;
        self.parsed += o.parsed as usize;
        self.resolved += o.resolved as usize;
        self.ran += o.ran as usize;
        self.matched += o.matched as usize;
    }
    fn json(&self) -> Value {
        json!({"total": self.total, "parsed": self.parsed, "resolved": self.resolved, "ran": self.ran, "matched": self.matched})
    }
}

/// The audit of `sources` (`Recipes::sources`' shape: vanilla's by bare name, the rest as `<ns>:<name>`) against the
/// tables the world built from them, on `threads` threads. The report: counts by type and by namespace, the failures
/// by cause (with their most common details), every failure, and the types not audited.
pub fn audit(sources: &[(String, Value)], items: &Items, recipes: &Recipes, stations: &StationData, threads: usize) -> Value {
    use rayon::prelude::*;
    let ctx = Ctx {
        items,
        recipes,
        stations,
        crafting: recipes.crafting.iter().map(|(n, r)| (n.as_str(), r)).collect(),
        cooking: recipes.cooking.iter().map(|c| (c.id.as_str(), c)).collect(),
        stonecutting: stations.recipes.stonecutting.iter().map(|r| (qualified(&r.id), r)).collect(),
        smithing: stations
            .recipes
            .smithing
            .iter()
            .map(|r| match r {
                Smithing::Transform { id, .. } | Smithing::Trim { id, .. } => (qualified(id), r),
            })
            .collect(),
    };
    let mut other_vanilla: BTreeMap<String, usize> = BTreeMap::new();
    let mut unsupported: BTreeMap<String, usize> = BTreeMap::new();
    let mut audited = Vec::new();
    for (name, v) in sources {
        let kind = v["type"].as_str().unwrap_or("");
        if AUDITED.contains(&kind) {
            audited.push((name.as_str(), kind, v));
        } else if kind.starts_with("minecraft:") || !kind.contains(':') {
            *other_vanilla.entry(kind.to_owned()).or_default() += 1;
        } else {
            *unsupported.entry(kind.to_owned()).or_default() += 1;
        }
    }
    let run = || audited.par_iter().map(|(name, kind, v)| ctx.audit(name, kind, v)).collect::<Vec<_>>();
    let outcomes = match rayon::ThreadPoolBuilder::new().num_threads(threads.max(1)).build() {
        Ok(pool) => pool.install(run),
        Err(_) => run(),
    };
    let mut by_type: BTreeMap<&str, Counts> = BTreeMap::new();
    let mut by_namespace: BTreeMap<&str, Counts> = BTreeMap::new();
    let mut causes: BTreeMap<Cause, (usize, BTreeMap<String, usize>, BTreeMap<String, usize>)> = BTreeMap::new();
    let mut failures = Vec::new();
    let mut all = Counts::default();
    for ((name, kind, _), o) in audited.iter().zip(&outcomes) {
        all.add(o);
        by_type.entry(kind).or_default().add(o);
        by_namespace.entry(namespace(name)).or_default().add(o);
        if let Some((cause, detail)) = &o.failure {
            let entry = causes.entry(*cause).or_default();
            entry.0 += 1;
            // Details by what they name, without the recipe's own words: `#c:ingots/tin`, `ingredients[]: ...`.
            *entry.1.entry(detail.clone()).or_default() += 1;
            *entry.2.entry(namespace(name).to_owned()).or_default() += 1;
            failures.push(json!({"id": qualified(name), "type": kind, "cause": cause.name(), "detail": detail}));
        }
    }
    let top = |m: &BTreeMap<String, usize>, n: usize| {
        let mut v: Vec<_> = m.iter().collect();
        v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        v.into_iter().take(n).map(|(k, c)| json!([k, c])).collect::<Vec<_>>()
    };
    json!({
        "sources": sources.len(),
        "audited": all.json(),
        "by_type": by_type.iter().map(|(k, c)| (k.to_string(), c.json())).collect::<serde_json::Map<_, _>>(),
        "by_namespace": by_namespace.iter().map(|(k, c)| (k.to_string(), c.json())).collect::<serde_json::Map<_, _>>(),
        "causes": causes.iter().map(|(cause, (n, details, namespaces))| (cause.name().to_owned(), json!({
            "recipes": n, "details": details.len(), "top_details": top(details, 40), "by_namespace": top(namespaces, 40),
        }))).collect::<serde_json::Map<_, _>>(),
        "other_vanilla_types": other_vanilla,
        "unsupported_types": {"recipes": unsupported.values().sum::<usize>(), "types": unsupported.len(), "by_type": unsupported},
        "failures": failures,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use horizon_world::loot::PackOverlay;
    use std::path::Path;

    fn root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.local/resources/26.3")
    }

    /// Vanilla's own recipes all run: every one of the audited types parses, resolves, and the matcher gives its
    /// result (or an earlier recipe's, as vanilla's own order does).
    #[test]
    #[ignore = "needs the prepared 26.3 resources"]
    fn vanilla_recipes_all_run() {
        let root = root();
        let items = Items::catalog(&root, &PackOverlay::NONE).unwrap();
        let recipes = Recipes::load(&root, &items, &PackOverlay::NONE).unwrap();
        let stations = StationData::load(&root, &items, &PackOverlay::NONE);
        let sources = Recipes::sources(&root, &PackOverlay::NONE).unwrap();
        let report = audit(&sources, &items, &recipes, &stations, 2);
        let a = &report["audited"];
        assert!(a["total"].as_u64().unwrap() > 1000, "{a}");
        let failed: Vec<_> = report["failures"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["cause"] != "another recipe first")
            .collect();
        assert!(failed.is_empty(), "{failed:#?}");
    }

    /// Each cause from its own source: an unknown item, an unknown tag, a 1.21.1 `{item}` object a plain build
    /// doesn't read, a type of a mod's own counted, not run.
    #[test]
    #[ignore = "needs the prepared 26.3 resources"]
    fn causes_are_told_apart() {
        let root = root();
        let items = Items::catalog(&root, &PackOverlay::NONE).unwrap();
        let stations = StationData::load(&root, &items, &PackOverlay::NONE);
        let sources: Vec<(String, Value)> = [
            ("t:good", json!({"type": "minecraft:crafting_shapeless", "ingredients": ["minecraft:dirt"], "result": {"id": "minecraft:diamond"}})),
            ("t:item", json!({"type": "minecraft:crafting_shapeless", "ingredients": ["nomod:thing"], "result": {"id": "minecraft:diamond"}})),
            ("t:tag", json!({"type": "minecraft:smelting", "ingredient": "#c:ingots/nothing", "result": {"id": "minecraft:diamond"}})),
            ("t:result", json!({"type": "minecraft:stonecutting", "ingredient": "minecraft:stone", "result": {"id": "nomod:thing"}})),
            ("t:mod", json!({"type": "nomod:grinding", "input": "minecraft:stone"})),
        ]
        .into_iter()
        .map(|(n, v)| (n.to_owned(), v))
        .collect();
        let recipes = Recipes::from_sources(&root, &items, &PackOverlay::NONE, sources.clone()).unwrap();
        let report = audit(&sources, &items, &recipes, &stations, 1);
        let cause = |id: &str| {
            report["failures"].as_array().unwrap().iter().find(|f| f["id"] == id).map(|f| f["cause"].as_str().unwrap().to_owned())
        };
        assert_eq!(report["audited"]["total"], 4);
        assert_eq!(report["audited"]["matched"], 1, "{report:#}");
        assert_eq!(cause("t:good"), None);
        assert_eq!(cause("t:item").as_deref(), Some("unknown item"));
        assert_eq!(cause("t:tag").as_deref(), Some("unknown tag"));
        assert_eq!(cause("t:result").as_deref(), Some("unknown item"));
        assert_eq!(report["unsupported_types"]["by_type"]["nomod:grinding"], 1);
    }
}
