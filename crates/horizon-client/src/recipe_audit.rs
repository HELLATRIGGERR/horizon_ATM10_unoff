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
//! A recipe that fails says why, as one of [`Cause`]. A type of a mod's own is read through the categories the mod's
//! machines give recipe viewers (`native_recipe_categories`, see [`machine_audit`]); a type nobody reads is counted.
use crate::crafting::{Ingredient, Recipe, Recipes, qualified};
use crate::items::{ItemStack, Items};
use crate::stations::{Smithing, StationData};
use horizon_world::native_load_context::LoopContext;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};

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
    context: &'a LoopContext,
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
            Value::Array(list) if list.is_empty() => {
                Some((Cause::UnparsedField, format!("{field}: empty list")))
            }
            // A list of items is one `HolderSet`: every one must exist.
            Value::Array(list) => list.iter().find_map(|e| self.ingredient(field, e)),
            Value::Object(o) => match Ingredient::parse(v, self.items) {
                Some(i) if i.order.iter().any(|i| self.registered(i)) => None,
                Some(i) => Some((
                    Cause::UnknownItem,
                    i.order.first().cloned().unwrap_or_default(),
                )),
                None if o.contains_key("item") || o.contains_key("tag") => Some((
                    Cause::UnparsedField,
                    format!("{field}: {{item}}/{{tag}} object"),
                )),
                None => {
                    let kind = o
                        .get("neoforge:ingredient_type")
                        .or_else(|| o.get("type"))
                        .and_then(Value::as_str);
                    Some((
                        Cause::UnparsedField,
                        format!("{field}: ingredient type {}", kind.unwrap_or("none")),
                    ))
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
            Some(id) if !self.registered(id) => {
                Some((Cause::UnknownItem, format!("{id} (result)")))
            }
            Some(_) => None,
        }
    }
    /// Every problem the source shows, the first one first.
    fn source_problem(&self, kind: &str, v: &Value) -> Option<Problem> {
        let mut fields: Vec<(String, &Value)> = Vec::new();
        match kind {
            "minecraft:crafting_shaped" => {
                let rows: Vec<&str> = v["pattern"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect();
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
                    return Some((
                        Cause::UnparsedField,
                        format!("ingredients: {} of them", list.len()),
                    ));
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
            .or_else(|| {
                (kind != "minecraft:smithing_trim")
                    .then(|| self.result(v))
                    .flatten()
            })
    }
    /// The stack the test puts in a slot for an ingredient: its first registered item, with the components it asks for.
    fn probe(&self, i: &Ingredient) -> Option<ItemStack> {
        i.order
            .iter()
            .find(|id| self.registered(id))
            .map(|id| i.example(id))
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
                "minecraft:smithing_trim" if v.get("pattern").is_none() => {
                    (Cause::UnparsedField, "pattern: missing".into())
                }
                _ => (Cause::UnparsedField, "not in the tables".into()),
            })
        };
        let id = qualified(name);
        let same =
            |got: &ItemStack, want: &ItemStack| got.item == want.item && got.count == want.count;
        match kind {
            "minecraft:crafting_shaped" | "minecraft:crafting_shapeless" => {
                let Some(recipe) = self.crafting.get(name) else {
                    return fail(out, unparsed());
                };
                out.parsed = true;
                if let Some(p) = problem {
                    return fail(out, p);
                }
                let mut grid: Vec<Option<ItemStack>> = vec![None; 9];
                let want = match recipe {
                    Recipe::Shaped {
                        width,
                        height,
                        cells,
                        result,
                    } => {
                        if *width > 3 || *height > 3 {
                            return fail(
                                out,
                                (Cause::UnparsedField, "pattern: larger than 3x3".into()),
                            );
                        }
                        for y in 0..*height {
                            for x in 0..*width {
                                if let Some(i) = &cells[y * width + x] {
                                    let Some(s) = self.probe(i) else {
                                        return fail(out, (Cause::UnknownItem, "ingredient".into()));
                                    };
                                    grid[y * 3 + x] = Some(s);
                                }
                            }
                        }
                        result.create()
                    }
                    Recipe::Shapeless {
                        ingredients,
                        result,
                        ..
                    } => {
                        if ingredients.len() > 9 {
                            return fail(
                                out,
                                (
                                    Cause::UnparsedField,
                                    format!("ingredients: {} of them", ingredients.len()),
                                ),
                            );
                        }
                        for (slot, i) in ingredients.iter().enumerate() {
                            let Some(s) = self.probe(i) else {
                                return fail(out, (Cause::UnknownItem, "ingredient".into()));
                            };
                            grid[slot] = Some(s);
                        }
                        result.create()
                    }
                    Recipe::Special(_) => {
                        return fail(
                            out,
                            (Cause::UnparsedField, "read as a special recipe".into()),
                        );
                    }
                    Recipe::Mod(_) => {
                        return fail(
                            out,
                            (Cause::UnparsedField, "read as a mod recipe kind".into()),
                        );
                    }
                };
                out.resolved = true;
                out.ran = true;
                match self.recipes.find(self.context, &grid, 3) {
                    Some((_, got)) if same(&got, &want) => out.matched = true,
                    Some((other, got))
                        if other != name
                            && self.recipes.recipe_matches(self.context, &id, &grid, 3) =>
                    {
                        return fail(
                            out,
                            (
                                Cause::AnotherRecipeFirst,
                                format!("{} gives {} x{}", qualified(other), got.item, got.count),
                            ),
                        );
                    }
                    Some((_, got)) => {
                        return fail(
                            out,
                            (
                                Cause::MatcherMismatch,
                                format!("gave {} x{}", got.item, got.count),
                            ),
                        );
                    }
                    None => return fail(out, (Cause::MatcherMismatch, "no result".into())),
                }
            }
            "minecraft:smelting"
            | "minecraft:blasting"
            | "minecraft:smoking"
            | "minecraft:campfire_cooking" => {
                let Some(recipe) = self.cooking.get(name) else {
                    return fail(out, unparsed());
                };
                out.parsed = true;
                if let Some(p) = problem {
                    return fail(out, p);
                }
                let Some(input) = self.probe(&recipe.ingredient) else {
                    return fail(out, (Cause::UnknownItem, "ingredient".into()));
                };
                out.resolved = true;
                out.ran = true;
                match self.recipes.cook(self.context, kind, &input) {
                    Some(c) if same(&c.result, &recipe.result) => out.matched = true,
                    Some(c)
                        if c.id != recipe.id && recipe.ingredient.test_in(self.context, &input) =>
                    {
                        return fail(
                            out,
                            (
                                Cause::AnotherRecipeFirst,
                                format!("{} gives {}", qualified(&c.id), c.result.item),
                            ),
                        );
                    }
                    Some(c) => {
                        return fail(
                            out,
                            (Cause::MatcherMismatch, format!("gave {}", c.result.item)),
                        );
                    }
                    None => return fail(out, (Cause::MatcherMismatch, "no result".into())),
                }
            }
            "minecraft:stonecutting" => {
                let Some(recipe) = self
                    .stonecutting
                    .get(&id)
                    .filter(|r| !r.input.order.is_empty())
                else {
                    return fail(out, unparsed());
                };
                out.parsed = true;
                if let Some(p) = problem {
                    return fail(out, p);
                }
                let Some(input) = self.probe(&recipe.input) else {
                    return fail(out, (Cause::UnknownItem, "ingredient".into()));
                };
                out.resolved = true;
                out.ran = true;
                let listed = self.stations.recipes.stonecutter(self.context, &input);
                match listed.iter().find(|r| r.id == id) {
                    Some(r) if same(&r.result, &recipe.result) => out.matched = true,
                    Some(r) => {
                        return fail(
                            out,
                            (Cause::MatcherMismatch, format!("gave {}", r.result.item)),
                        );
                    }
                    None => {
                        return fail(
                            out,
                            (
                                Cause::MatcherMismatch,
                                format!("not listed ({} others)", listed.len()),
                            ),
                        );
                    }
                }
            }
            _ => {
                let Some(recipe) = self.smithing.get(&id) else {
                    return fail(out, unparsed());
                };
                out.parsed = true;
                if let Some(p) = problem {
                    return fail(out, p);
                }
                let slot = |i: Option<&Ingredient>| match i {
                    None => Ok(None),
                    Some(i) => self
                        .probe(i)
                        .map(Some)
                        .ok_or((Cause::UnknownItem, "ingredient".to_owned())),
                };
                let (slots, want) = match recipe {
                    Smithing::Transform {
                        template,
                        base,
                        addition,
                        ..
                    } => (
                        [
                            slot(template.as_ref()),
                            slot(Some(base)),
                            slot(addition.as_ref()),
                        ],
                        horizon_world::loot::recipe_result_id(&v["result"]).map(str::to_owned),
                    ),
                    Smithing::Trim {
                        template,
                        base,
                        addition,
                        ..
                    } => (
                        [slot(Some(template)), slot(Some(base)), slot(Some(addition))],
                        None,
                    ),
                };
                let [t, b, a] = match slots {
                    [Ok(t), Ok(b), Ok(a)] => [t, b, a],
                    [t, b, a] => {
                        return fail(
                            out,
                            [t, b, a]
                                .into_iter()
                                .find_map(Result::err)
                                .unwrap_or((Cause::UnknownItem, String::new())),
                        );
                    }
                };
                out.resolved = true;
                out.ran = true;
                let want = want
                    .or_else(|| b.as_ref().map(|b| b.item.to_string()))
                    .unwrap_or_default();
                match self.stations.recipes.smithing_recipe(
                    self.context,
                    &self.stations.enchantments,
                    t.as_ref(),
                    b.as_ref(),
                    a.as_ref(),
                ) {
                    Some((_, got)) if *got.item == *want => out.matched = true,
                    Some((other, got)) if other != id => {
                        return fail(
                            out,
                            (
                                Cause::AnotherRecipeFirst,
                                format!("{other} gives {}", got.item),
                            ),
                        );
                    }
                    Some((_, got)) => {
                        return fail(out, (Cause::MatcherMismatch, format!("gave {}", got.item)));
                    }
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
/// by cause (with their most common details), every failure, and the types not audited. `categories` are the mods'
/// recipe viewer categories (`native_recipe_categories`): a mod type they read is audited by [`machine_audit`] and
/// reported under `machines`, not counted as unsupported.
pub fn audit(
    context: &LoopContext,
    sources: &[(String, Value)],
    items: &Items,
    recipes: &Recipes,
    stations: &StationData,
    categories: &[Category],
    threads: usize,
) -> Value {
    use rayon::prelude::*;
    let ctx = Ctx {
        context,
        items,
        recipes,
        stations,
        crafting: recipes
            .crafting
            .iter()
            .map(|(n, r)| (n.as_str(), r))
            .collect(),
        cooking: recipes.cooking.iter().map(|c| (c.id.as_str(), c)).collect(),
        stonecutting: stations
            .recipes
            .stonecutting
            .iter()
            .map(|r| (qualified(&r.id), r))
            .collect(),
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
    let machines = Machines::new(categories, sources);
    let mut machine_sources = Vec::new();
    for (name, v) in sources {
        let kind = v["type"].as_str().unwrap_or("");
        if AUDITED.contains(&kind) {
            audited.push((name.as_str(), kind, v));
        } else if kind.starts_with("minecraft:") || !kind.contains(':') {
            *other_vanilla.entry(kind.to_owned()).or_default() += 1;
        } else if machines.reads(kind) {
            machine_sources.push((name.as_str(), kind, v));
        } else {
            *unsupported.entry(kind.to_owned()).or_default() += 1;
        }
    }
    let (machine_report, machine_failures) = machine_audit(&machine_sources, items, &machines);
    let run = || {
        audited
            .par_iter()
            .map(|(name, kind, v)| ctx.audit(name, kind, v))
            .collect::<Vec<_>>()
    };
    let outcomes = match rayon::ThreadPoolBuilder::new()
        .num_threads(threads.max(1))
        .build()
    {
        Ok(pool) => pool.install(run),
        Err(_) => run(),
    };
    let mut by_type: BTreeMap<&str, Counts> = BTreeMap::new();
    let mut by_namespace: BTreeMap<&str, Counts> = BTreeMap::new();
    let mut causes: BTreeMap<Cause, (usize, BTreeMap<String, usize>, BTreeMap<String, usize>)> =
        BTreeMap::new();
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
        v.into_iter()
            .take(n)
            .map(|(k, c)| json!([k, c]))
            .collect::<Vec<_>>()
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
        "machines": machine_report,
        "failures": failures.into_iter().chain(machine_failures).collect::<Vec<_>>(),
    })
}

/// A stack in a [`Listed`] recipe: an item (`kind` empty), a fluid (`"fluid"`) or a mod's own ingredient (`kind`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stack {
    pub kind: String,
    pub id: String,
}
impl Stack {
    pub const FLUID: &'static str = "fluid";
}

/// A recipe a category lists: the id of the recipe file it was read from (the category's `Recipe: <id>` note), what
/// each input slot can hold (an empty slot stays empty), and what comes out.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Listed {
    pub id: String,
    pub inputs: Vec<Vec<Stack>>,
    pub outputs: Vec<Stack>,
}

/// A mod's recipe viewer category (`native_recipe_categories`; `mod_api::NativeRecipeCategory` lives above this
/// crate, so the game converts them) as the audit reads it: the recipes its machine's reader accepted, and the mod's
/// own ingredients (kind, id) it lists.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Category {
    pub id: String,
    pub ingredients: Vec<(String, String)>,
    pub recipes: Vec<Listed>,
}

/// What the mods' categories read.
pub struct Machines<'a> {
    /// A recipe's id (as `qualified` writes it) -> the category that lists it and the entry.
    claimed: HashMap<String, (&'a Category, &'a Listed)>,
    /// The recipe types some category reads (one that lists a recipe of the type, or has the type's name), with the
    /// categories that do.
    types: BTreeMap<String, BTreeSet<&'a str>>,
}
impl<'a> Machines<'a> {
    pub fn new(categories: &'a [Category], sources: &[(String, Value)]) -> Self {
        let mut claimed = HashMap::new();
        for c in categories {
            for r in &c.recipes {
                claimed.entry(qualified(&r.id)).or_insert((c, r));
            }
        }
        let mut types: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
        for c in categories {
            types.entry(c.id.clone()).or_default().insert(&c.id);
        }
        for (name, v) in sources {
            if let (Some(kind), Some((c, _))) = (v["type"].as_str(), claimed.get(&qualified(name)))
            {
                types.entry(kind.to_owned()).or_default().insert(&c.id);
            }
        }
        Machines { claimed, types }
    }
    /// Whether some category reads recipes of type `kind`.
    pub fn reads(&self, kind: &str) -> bool {
        self.types.contains_key(kind)
    }
}

/// Whether this world has `s`: an item that is registered, a fluid, or a mod's own ingredient the category lists.
fn stack_known(items: &Items, category: &Category, s: &Stack) -> bool {
    match s.kind.as_str() {
        "" => s.id != "minecraft:air" && items.items.contains_key(s.id.as_str()),
        Stack::FLUID => horizon_world::fluid::FluidKind::from_name(&s.id).is_some(),
        kind => category
            .ingredients
            .iter()
            .any(|(k, id)| k == kind && *id == s.id),
    }
}

/// A recipe a category's reader accepted: can each input slot be filled with some stack this world has, and is each
/// output one it has?
fn unresolved(items: &Items, category: &Category, r: &Listed) -> Option<Problem> {
    for slot in r.inputs.iter().filter(|s| !s.is_empty()) {
        if !slot.iter().any(|s| stack_known(items, category, s)) {
            return Some((Cause::UnknownItem, format!("{} (input)", slot[0].id)));
        }
    }
    r.outputs
        .iter()
        .find(|o| !stack_known(items, category, o))
        .map(|o| (Cause::UnknownItem, format!("{} (output)", o.id)))
}

/// Why a category's reader left a recipe out, as far as its source says: an item or fluid it names that this world
/// lacks, a tag no item has, else only that the reader dropped it (a chemical it can't read, a shape).
fn left_out(items: &Items, v: &Value) -> Problem {
    fn walk(items: &Items, v: &Value, tag: &mut Option<String>) -> Option<Problem> {
        match v {
            Value::Object(o) => {
                let text = |k: &str| o.get(k).and_then(Value::as_str);
                if let Some(i) =
                    text("item").filter(|i| i.contains(':') && !items.items.contains_key(*i))
                {
                    return Some((Cause::UnknownItem, i.to_owned()));
                }
                if let Some(f) = text("fluid")
                    .filter(|f| horizon_world::fluid::FluidKind::from_name(f).is_none())
                {
                    return Some((Cause::UnknownItem, format!("{f} (fluid)")));
                }
                // A result with a count is an item; with an amount, a fluid or a chemical (not told apart here).
                if let Some(i) = text("id").filter(|i| {
                    !o.contains_key("amount")
                        && o.contains_key("count")
                        && !items.items.contains_key(*i)
                }) {
                    return Some((Cause::UnknownItem, format!("{i} (result)")));
                }
                if let Some(t) = text("tag").filter(|t| !items.item_tag_order.contains_key(*t)) {
                    tag.get_or_insert_with(|| t.to_owned());
                }
                o.values().find_map(|c| walk(items, c, tag))
            }
            Value::Array(list) => list.iter().find_map(|c| walk(items, c, tag)),
            _ => None,
        }
    }
    let mut tag = None;
    walk(items, v, &mut tag).unwrap_or_else(|| match tag {
        Some(t) => (
            Cause::UnparsedField,
            format!(
                "left out by the reader (#{t} is no item tag: a fluid or chemical tag, or none)"
            ),
        ),
        None => (
            Cause::UnparsedField,
            "left out by the reader (a chemical it can't read, or a shape)".into(),
        ),
    })
}

/// The audit of the mod types the categories read ([`Machines`]): each recipe of such a type is
/// 1. *parsed*: a category's reader accepted it (the category lists it);
/// 2. *resolved*: the entry's input slots and outputs name items and fluids this world registers, and chemicals (a
///    mod's own ingredient kind) the category lists.
///
/// No matcher is run: the machines' matchers take a live block entity. So the deepest level is *resolved*; `matched`
/// stays 0 and each type says `"checked": "resolved"`. The report has the shape of [`audit`]'s: counts by type (with
/// the categories that read it) and by namespace, the failures by cause, and every failure (marked `machine`).
fn machine_audit(
    sources: &[(&str, &str, &Value)],
    items: &Items,
    machines: &Machines<'_>,
) -> (Value, Vec<Value>) {
    type Detail = BTreeMap<String, usize>;
    let mut by_type: BTreeMap<&str, Counts> = BTreeMap::new();
    let mut by_namespace: BTreeMap<&str, Counts> = BTreeMap::new();
    let mut causes: BTreeMap<Cause, (usize, Detail, Detail)> = BTreeMap::new();
    let mut failures = Vec::new();
    let mut all = Counts::default();
    for &(name, kind, v) in sources {
        let mut o = Outcome::default();
        match machines.claimed.get(&qualified(name)) {
            None => o.failure = Some(left_out(items, v)),
            Some((category, entry)) => {
                o.parsed = true;
                match unresolved(items, category, entry) {
                    Some(p) => o.failure = Some(p),
                    None => o.resolved = true,
                }
            }
        }
        all.add(&o);
        by_type.entry(kind).or_default().add(&o);
        by_namespace.entry(namespace(name)).or_default().add(&o);
        if let Some((cause, detail)) = &o.failure {
            let entry = causes.entry(*cause).or_default();
            entry.0 += 1;
            *entry.1.entry(detail.clone()).or_default() += 1;
            *entry.2.entry(kind.to_owned()).or_default() += 1;
            failures.push(json!({"id": qualified(name), "type": kind, "cause": cause.name(), "detail": detail, "machine": true}));
        }
    }
    let top = |m: &Detail, n: usize| {
        let mut v: Vec<_> = m.iter().collect();
        v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        v.into_iter()
            .take(n)
            .map(|(k, c)| json!([k, c]))
            .collect::<Vec<_>>()
    };
    let typed = |kind: &str, c: &Counts| {
        let mut j = c.json();
        j["categories"] = json!(machines.types.get(kind));
        j["checked"] = json!("resolved");
        j
    };
    let report = json!({
        "audited": all.json(),
        "by_type": by_type.iter().map(|(k, c)| (k.to_string(), typed(k, c))).collect::<serde_json::Map<_, _>>(),
        "by_namespace": by_namespace.iter().map(|(k, c)| (k.to_string(), c.json())).collect::<serde_json::Map<_, _>>(),
        "causes": causes.iter().map(|(cause, (n, details, kinds))| (cause.name().to_owned(), json!({
            "recipes": n, "details": details.len(), "top_details": top(details, 40), "by_type": top(kinds, 40),
        }))).collect::<serde_json::Map<_, _>>(),
        "categories": machines.types.iter().map(|(k, v)| (k.clone(), json!(v))).collect::<serde_json::Map<_, _>>(),
    });
    (report, failures)
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
        let items = Items::catalog(&root, &PackOverlay::NONE, &horizon_world::item_defaults::read(&root).unwrap()).unwrap();
        let load = crate::test_resources::server_load();
        let recipes = Recipes::load(&root, &items, &PackOverlay::NONE, &load).unwrap();
        let defaults = horizon_world::item_defaults::read(&root).unwrap();
        let stations = StationData::load(&root, &items, &PackOverlay::NONE, &defaults, &load);
        let sources = horizon_items::recipe_sources::sources(&root, &PackOverlay::NONE).unwrap();
        let report = audit(
            crate::test_resources::server_loop_context(),
            &sources,
            &items,
            &recipes,
            &stations,
            &[],
            2,
        );
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
        let items = Items::catalog(&root, &PackOverlay::NONE, &horizon_world::item_defaults::read(&root).unwrap()).unwrap();
        let load = crate::test_resources::server_load();
        let defaults = horizon_world::item_defaults::read(&root).unwrap();
        let stations = StationData::load(&root, &items, &PackOverlay::NONE, &defaults, &load);
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
        let recipes =
            Recipes::from_sources(&root, &items, &PackOverlay::NONE, sources.clone(), &load)
                .unwrap();
        let report = audit(
            crate::test_resources::server_loop_context(),
            &sources,
            &items,
            &recipes,
            &stations,
            &[],
            1,
        );
        let cause = |id: &str| {
            report["failures"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["id"] == id)
                .map(|f| f["cause"].as_str().unwrap().to_owned())
        };
        assert_eq!(report["audited"]["total"], 4);
        assert_eq!(report["audited"]["matched"], 1, "{report:#}");
        assert_eq!(cause("t:good"), None);
        assert_eq!(cause("t:item").as_deref(), Some("unknown item"));
        assert_eq!(cause("t:tag").as_deref(), Some("unknown tag"));
        assert_eq!(cause("t:result").as_deref(), Some("unknown item"));
        assert_eq!(report["unsupported_types"]["by_type"]["nomod:grinding"], 1);
    }

    /// A mod type read through the mods' recipe viewer categories: parsed when a category lists it (its
    /// `Recipe: <id>` note), resolved when the entry's stacks are registered; the ones the reader left out say why
    /// from their source. A type no category reads stays counted, not audited.
    #[test]
    #[ignore = "needs the prepared 26.3 resources"]
    fn machine_types_are_read_through_categories() {
        let root = root();
        let items = Items::catalog(&root, &PackOverlay::NONE, &horizon_world::item_defaults::read(&root).unwrap()).unwrap();
        let load = crate::test_resources::server_load();
        let defaults = horizon_world::item_defaults::read(&root).unwrap();
        let stations = StationData::load(&root, &items, &PackOverlay::NONE, &defaults, &load);
        let sources: Vec<(String, Value)> = [
            ("m:ok", json!({"type": "m:grind", "input": "minecraft:stone", "output": {"id": "minecraft:gravel", "count": 1}})),
            ("m:gone", json!({"type": "m:grind", "input": {"item": "nomod:rock"}, "output": {"id": "minecraft:gravel", "count": 1}})),
            ("m:out", json!({"type": "m:grind", "input": "minecraft:dirt", "output": {"id": "nomod:dust", "count": 1}})),
            ("m:left", json!({"type": "m:grind", "input": "minecraft:sand"})),
            ("m:other", json!({"type": "m:unread"})),
        ]
        .into_iter()
        .map(|(n, v)| (n.to_owned(), v))
        .collect();
        let item = |id: &str| Stack {
            kind: String::new(),
            id: id.to_owned(),
        };
        let entry = |id: &str, input: &str, out: &str| Listed {
            id: id.to_owned(),
            inputs: vec![vec![item(input)]],
            outputs: vec![item(out)],
        };
        let categories = vec![Category {
            id: "m:grinding".into(),
            recipes: vec![
                entry("m:ok", "minecraft:stone", "minecraft:gravel"),
                entry("m:out", "minecraft:dirt", "nomod:dust"),
            ],
            ..Default::default()
        }];
        let recipes =
            Recipes::from_sources(&root, &items, &PackOverlay::NONE, sources.clone(), &load)
                .unwrap();
        let report = audit(
            crate::test_resources::server_loop_context(),
            &sources,
            &items,
            &recipes,
            &stations,
            &categories,
            1,
        );
        let grind = &report["machines"]["by_type"]["m:grind"];
        assert_eq!(
            (
                &grind["total"],
                &grind["parsed"],
                &grind["resolved"],
                &grind["matched"]
            ),
            (&json!(4), &json!(2), &json!(1), &json!(0)),
            "{report:#}"
        );
        assert_eq!(grind["categories"], json!(["m:grinding"]));
        let cause = |id: &str| {
            report["failures"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["id"] == id)
                .map(|f| f["cause"].as_str().unwrap().to_owned())
        };
        assert_eq!(cause("m:ok"), None);
        assert_eq!(cause("m:gone").as_deref(), Some("unknown item"));
        assert_eq!(cause("m:out").as_deref(), Some("unknown item"));
        assert_eq!(cause("m:left").as_deref(), Some("unparsed field"));
        assert_eq!(report["audited"]["total"], 0);
        assert_eq!(report["unsupported_types"]["by_type"]["m:unread"], 1);
    }
}
