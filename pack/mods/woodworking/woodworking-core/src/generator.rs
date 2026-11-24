// Woodworking recipe generator.
//
// Runs once per world over the game's own recipe graph (crafting and stonecutting recipes) and
// item registry. Nothing about particular items or counts is stored here: the rule below derives
// every family, every output and every count from the recipes the world has loaded.
//
// 1. Wood families. A recipe with one ingredient slot whose result is `<ns>:<base>_planks` names a
//    family: its planks item, how many planks one log gives (the recipe's count, the best one wins)
//    and its log variants. The variants are the slot's items, plus the registry's items, whose name
//    reduces to the same `<ns>:<base>` once a stripped marker and a log-like suffix (`_log`, `_wood`,
//    `_stem`, `_hyphae`, `_block`, `_bark`) are removed.
// 2. Plank value. Within a family a log is worth its planks count and a plank is worth 1. A
//    recipe all of whose slots accept an item of known value costs the sum of the cheapest accepted
//    items; its result is then worth cost / count. Two passes reach the items made from items made
//    from wood (a chest boat from a boat and a chest).
// 3. Counts. The best rate (results per plank) of an output gives the station's count for one log
//    (rate x planks per log) and, when one plank makes more than half an item, for one plank. A
//    fractional count rounds up once its fraction passes 0.3, never below one; items that do not
//    stack give one.
// 4. Outputs made only from sticks (the stick itself, ladders) are the same for every family: they
//    are offered once, for any family's logs or planks.
// 5. A log variant also converts to the family's other variants, one for one, but a stripped one
//    never turns back into a bark-covered one.

use std::collections::{BTreeMap, BTreeSet, HashMap};

// ---- Public types consumed by the caller ----------------------------------------

/// A crafting or stonecutting recipe as the generator reads it: for every non-empty slot the
/// items it accepts, and the result.
#[derive(Debug, Clone)]
pub struct GenRecipe {
    pub slots: Vec<Vec<String>>,
    pub output: String,
    pub output_count: u32,
}

/// A single woodworking recipe produced by the generator (the index groups by input).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    /// Every item accepted as input (all log variants of a family, a planks item, or one variant).
    pub inputs: Vec<String>,
    pub output: String,
    pub count: u32,
}

// ---- Constants of the rule ------------------------------------------------------

/// The stick: the one material every family shares.
const STICK: &str = "minecraft:stick";
/// Planks per log used for the outputs every family shares.
const SHARED_PLANKS_PER_LOG: f64 = 4.0;
/// A plank recipe is offered when one plank makes more than this many items.
const PLANKS_MIN_RATE: f64 = 0.5;
/// A fractional count rounds up when its fraction exceeds this.
const ROUND_UP_ABOVE: f64 = 0.3;
const EPS: f64 = 1e-6;
/// Passes of the plank-value closure.
const PASSES: usize = 2;

// ---- Names ----------------------------------------------------------------------

/// The `(namespace, base)` a log-like item belongs to: `minecraft:stripped_oak_wood` → `(minecraft,
/// oak)`, `somemod:elder_stripped_log` → `(somemod, elder)`. `None` for any other item.
pub fn family_key(id: &str) -> Option<(&str, String)> {
    let (ns, local) = id.split_once(':')?;
    let mut bare = local.strip_prefix("stripped_").unwrap_or(local).to_string();
    bare = bare.replacen("_stripped_", "_", 1);
    if let Some(b) = bare.strip_suffix("_stripped") {
        bare = b.to_string();
    }
    for suffix in ["_log", "_wood", "_stem", "_hyphae", "_block", "_bark"] {
        if let Some(base) = bare.strip_suffix(suffix) {
            if base.is_empty() {
                return None;
            }
            return Some((ns, base.to_string()));
        }
    }
    None
}

fn is_stripped(id: &str) -> bool {
    id.split_once(':').is_some_and(|(_, l)| l.contains("stripped"))
}

/// The station's count for `x` results: the fraction rounds up past `ROUND_UP_ABOVE`, at least 1.
fn round_count(x: f64) -> u32 {
    let whole = (x + EPS).floor();
    let up = if x - whole > ROUND_UP_ABOVE + EPS { 1.0 } else { 0.0 };
    ((whole + up) as u32).max(1)
}

/// Counts for one log and, if offered, one plank.
fn counts(rate: f64, per_log: f64, max_stack: u32) -> (u32, Option<u32>) {
    let planks = rate > PLANKS_MIN_RATE + EPS;
    if max_stack == 1 {
        return (1, planks.then_some(1));
    }
    (round_count(rate * per_log), planks.then(|| round_count(rate)))
}

// ---- The recipe graph, interned ---------------------------------------------------

struct Graph<'a> {
    names: Vec<&'a str>,
    ids: HashMap<&'a str, u32>,
    /// (slots of accepted item ids, output, count)
    recipes: Vec<(Vec<Vec<u32>>, u32, u32)>,
}

impl<'a> Graph<'a> {
    fn new(items: &'a [String], recipes: &'a [GenRecipe]) -> Self {
        let mut g = Graph { names: Vec::new(), ids: HashMap::new(), recipes: Vec::new() };
        for id in items {
            g.intern(id);
        }
        for r in recipes {
            if r.slots.is_empty() || r.slots.iter().any(Vec::is_empty) || r.output_count == 0 {
                continue;
            }
            let slots = r
                .slots
                .iter()
                .map(|s| {
                    let mut v: Vec<u32> = s.iter().map(|i| g.intern(i)).collect();
                    v.sort_unstable();
                    v.dedup();
                    v
                })
                .collect();
            let out = g.intern(&r.output);
            g.recipes.push((slots, out, r.output_count));
        }
        g
    }

    fn intern(&mut self, id: &'a str) -> u32 {
        if let Some(&n) = self.ids.get(id) {
            return n;
        }
        let n = self.names.len() as u32;
        self.names.push(id);
        self.ids.insert(id, n);
        n
    }
}

struct Family {
    planks: u32,
    per_log: u32,
    variants: BTreeSet<u32>,
}

fn families(g: &Graph<'_>, items: &[String]) -> Vec<Family> {
    // Registry items by family key, for the variants no planks recipe lists.
    let mut by_key: HashMap<(&str, String), Vec<u32>> = HashMap::new();
    for id in items {
        if let Some(key) = family_key(id) {
            by_key.entry(key).or_default().push(g.ids[id.as_str()]);
        }
    }
    let mut found: BTreeMap<u32, Family> = BTreeMap::new();
    for (slots, out, count) in &g.recipes {
        let out_name = g.names[*out as usize];
        let [slot] = slots.as_slice() else { continue };
        let Some(base_id) = out_name.strip_suffix("_planks") else { continue };
        if slot.iter().any(|&i| g.names[i as usize].ends_with("_planks")) {
            continue;
        }
        let Some((ns, base)) = base_id.split_once(':') else { continue };
        let fam = found.entry(*out).or_insert_with(|| Family { planks: *out, per_log: 0, variants: BTreeSet::new() });
        fam.per_log = fam.per_log.max(*count);
        for &i in slot {
            if family_key(g.names[i as usize]).is_some_and(|(n, b)| n == ns && b == base) {
                fam.variants.insert(i);
            }
        }
        if let Some(more) = by_key.get(&(ns, base.to_string())) {
            fam.variants.extend(more.iter().copied());
        }
    }
    found.into_values().filter(|f| !f.variants.is_empty() && f.per_log > 0).collect()
}

/// Recipes that can take part in a family's closure: every slot accepts something wooden.
fn wooden_recipes(g: &Graph<'_>, fams: &[Family], stick: Option<u32>) -> Vec<usize> {
    let mut wooden = vec![false; g.names.len()];
    for f in fams {
        wooden[f.planks as usize] = true;
        for &v in &f.variants {
            wooden[v as usize] = true;
        }
    }
    if let Some(s) = stick {
        wooden[s as usize] = true;
    }
    let mut keep = Vec::new();
    for _ in 0..PASSES {
        keep.clear();
        let mut grown = Vec::new();
        for (n, (slots, out, _)) in g.recipes.iter().enumerate() {
            if slots.iter().all(|s| s.iter().any(|&i| wooden[i as usize])) {
                keep.push(n);
                grown.push(*out);
            }
        }
        for o in grown {
            wooden[o as usize] = true;
        }
    }
    keep
}

// ---- Public entry point ---------------------------------------------------------

/// Generate the station's recipes from the world's recipes.
///
/// `items`: every registered item id. `recipes`: the world's crafting and stonecutting recipes.
/// `max_stack`: an item's stack size.
pub fn generate<S>(items: &[String], recipes: &[GenRecipe], max_stack: S) -> Vec<Recipe>
where
    S: Fn(&str) -> u32,
{
    let g = Graph::new(items, recipes);
    let fams = families(&g, items);
    let stick = g.ids.get(STICK).copied();
    let candidates = wooden_recipes(&g, &fams, stick);
    let name = |i: u32| g.names[i as usize].to_string();

    let mut out: Vec<Recipe> = Vec::new();
    // Outputs every family shares: output → (best rate).
    let mut shared: BTreeMap<u32, f64> = BTreeMap::new();
    // Per-family offers, kept until the shared outputs are known.
    let mut offers: Vec<(usize, u32, f64)> = Vec::new();

    let mut value = vec![f64::INFINITY; g.names.len()];
    let mut touched: Vec<u32> = Vec::new();
    for (fi, fam) in fams.iter().enumerate() {
        for &t in &touched {
            value[t as usize] = f64::INFINITY;
        }
        touched.clear();
        for &v in &fam.variants {
            value[v as usize] = fam.per_log as f64;
            touched.push(v);
        }
        value[fam.planks as usize] = 1.0;
        touched.push(fam.planks);

        // output → (rate, made from sticks only)
        let mut best: BTreeMap<u32, (f64, bool)> = BTreeMap::new();
        for _ in 0..PASSES {
            let mut cheaper: Vec<(u32, f64)> = Vec::new();
            for &n in &candidates {
                let (slots, o, count) = &g.recipes[n];
                let mut cost = 0.0;
                let mut sticks_only = true;
                let mut ok = true;
                for slot in slots {
                    let mut cheapest: Option<(f64, u32)> = None;
                    for &i in slot {
                        let v = value[i as usize];
                        if v.is_finite() && cheapest.is_none_or(|(c, _)| v < c) {
                            cheapest = Some((v, i));
                        }
                    }
                    let Some((v, i)) = cheapest else {
                        ok = false;
                        break;
                    };
                    cost += v;
                    sticks_only &= Some(i) == stick;
                }
                if !ok || cost <= 0.0 {
                    continue;
                }
                let rate = *count as f64 / cost;
                match best.get(o) {
                    Some((r, _)) if rate <= r + 1e-9 => {}
                    _ => {
                        best.insert(*o, (rate, sticks_only));
                    }
                }
                let unit = cost / *count as f64;
                if unit < value[*o as usize] - 1e-9 {
                    cheaper.push((*o, unit));
                }
            }
            for (o, v) in cheaper {
                if v < value[o as usize] {
                    value[o as usize] = v;
                    touched.push(o);
                }
            }
        }

        for (o, (rate, sticks_only)) in best {
            if fam.variants.contains(&o) {
                continue;
            }
            if sticks_only || Some(o) == stick {
                let r = shared.entry(o).or_insert(rate);
                *r = r.max(rate);
                continue;
            }
            offers.push((fi, o, rate));
        }

        // Variant conversions.
        for &from in &fam.variants {
            for &to in &fam.variants {
                if from != to && (is_stripped(g.names[to as usize]) || !is_stripped(g.names[from as usize])) {
                    out.push(Recipe { inputs: vec![name(from)], output: name(to), count: 1 });
                }
            }
        }
    }

    for (fi, o, rate) in offers {
        if shared.contains_key(&o) {
            continue;
        }
        let fam = &fams[fi];
        let (logs, planks) = counts(rate, fam.per_log as f64, max_stack(g.names[o as usize]));
        out.push(Recipe { inputs: fam.variants.iter().map(|&v| name(v)).collect(), output: name(o), count: logs });
        if let Some(p) = planks.filter(|_| o != fam.planks) {
            out.push(Recipe { inputs: vec![name(fam.planks)], output: name(o), count: p });
        }
    }

    if !shared.is_empty() {
        let all_logs: Vec<String> =
            fams.iter().flat_map(|f| f.variants.iter().map(|&v| name(v))).collect::<BTreeSet<_>>().into_iter().collect();
        let all_planks: Vec<String> = fams.iter().map(|f| name(f.planks)).collect::<BTreeSet<_>>().into_iter().collect();
        for (o, rate) in shared {
            let (logs, planks) = counts(rate, SHARED_PLANKS_PER_LOG, max_stack(g.names[o as usize]));
            out.push(Recipe { inputs: all_logs.clone(), output: name(o), count: logs });
            if let Some(p) = planks {
                out.push(Recipe { inputs: all_planks.clone(), output: name(o), count: p });
            }
        }
    }
    out
}

// ---- Tests ----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    fn r(slots: Vec<Vec<&str>>, output: &str, count: u32) -> GenRecipe {
        GenRecipe { slots: slots.iter().map(|sl| s(sl)).collect(), output: output.into(), output_count: count }
    }

    /// A small world: two families, a few shapes, a shared stick and a ladder.
    fn world() -> (Vec<String>, Vec<GenRecipe>) {
        let items = s(&[
            "minecraft:oak_log", "minecraft:oak_wood", "minecraft:stripped_oak_log", "minecraft:stripped_oak_wood",
            "minecraft:oak_planks", "minecraft:oak_slab", "minecraft:oak_stairs", "minecraft:oak_door",
            "minecraft:barrel", "minecraft:bowl", "minecraft:stick", "minecraft:ladder", "minecraft:wooden_sword",
            "minecraft:composter", "minecraft:oak_boat", "minecraft:chest", "minecraft:oak_chest_boat",
            "wood:elder_log", "wood:elder_stripped_log", "wood:elder_planks", "wood:elder_slab", "minecraft:stone",
        ]);
        let planks = vec!["minecraft:oak_planks", "wood:elder_planks"];
        let slabs = vec!["minecraft:oak_slab", "wood:elder_slab"];
        let one = |id: &'static str, n: usize| vec![vec![id]; n];
        let recipes = vec![
            r(vec![vec!["minecraft:oak_log", "minecraft:oak_wood", "minecraft:stripped_oak_log", "minecraft:stripped_oak_wood"]], "minecraft:oak_planks", 4),
            r(vec![vec!["wood:elder_log", "wood:elder_stripped_log"]], "wood:elder_planks", 4),
            r(one("minecraft:oak_planks", 3), "minecraft:oak_slab", 6),
            r(one("wood:elder_planks", 3), "wood:elder_slab", 6),
            r(one("minecraft:oak_planks", 6), "minecraft:oak_stairs", 4),
            r(one("minecraft:oak_planks", 6), "minecraft:oak_door", 3),
            r(vec![planks.clone(), planks.clone(), planks.clone(), planks.clone(), planks.clone(), planks.clone(), slabs.clone(), slabs.clone()], "minecraft:barrel", 1),
            r(vec![planks.clone(), planks.clone(), planks.clone()], "minecraft:bowl", 4),
            r(vec![planks.clone(), planks.clone()], "minecraft:stick", 4),
            r(one("minecraft:stick", 7), "minecraft:ladder", 3),
            r(vec![planks.clone(), planks.clone(), vec!["minecraft:stick"]], "minecraft:wooden_sword", 1),
            r(vec![slabs.clone(), slabs.clone(), slabs.clone(), slabs.clone(), slabs.clone(), slabs.clone(), slabs.clone()], "minecraft:composter", 1),
            r(one("minecraft:oak_planks", 5), "minecraft:oak_boat", 1),
            r(vec![planks.clone(), planks.clone(), planks.clone(), planks.clone(), planks.clone(), planks.clone(), planks.clone(), planks.clone()], "minecraft:chest", 1),
            r(vec![vec!["minecraft:oak_boat"], vec!["minecraft:chest"]], "minecraft:oak_chest_boat", 1),
            r(vec![vec!["minecraft:stone"]], "minecraft:oak_slab", 2),
        ];
        (items, recipes)
    }

    fn stack(id: &str) -> u32 {
        if id.ends_with("_sword") || id.ends_with("_boat") { 1 } else { 64 }
    }

    fn find<'a>(all: &'a [Recipe], input: &str, output: &str) -> Option<&'a Recipe> {
        all.iter().find(|r| r.output == output && r.inputs.iter().any(|i| i == input) && (r.inputs.len() > 1 || r.inputs[0] == input))
    }

    fn run() -> Vec<Recipe> {
        let (items, recipes) = world();
        generate(&items, &recipes, stack)
    }

    #[test]
    fn family_key_names() {
        assert_eq!(family_key("minecraft:oak_log"), Some(("minecraft", "oak".into())));
        assert_eq!(family_key("minecraft:stripped_cherry_wood"), Some(("minecraft", "cherry".into())));
        assert_eq!(family_key("minecraft:crimson_hyphae"), Some(("minecraft", "crimson".into())));
        assert_eq!(family_key("minecraft:bamboo_block"), Some(("minecraft", "bamboo".into())));
        assert_eq!(family_key("wood:elder_stripped_log"), Some(("wood", "elder".into())));
        assert_eq!(family_key("wood:menril_log_stripped"), Some(("wood", "menril".into())));
        assert_eq!(family_key("minecraft:oak_planks"), None);
    }

    #[test]
    fn logs_give_planks_by_the_planks_recipe() {
        let all = run();
        let planks = find(&all, "minecraft:oak_log", "minecraft:oak_planks").expect("oak planks");
        assert_eq!(planks.count, 4);
        assert_eq!(planks.inputs.len(), 4, "every oak variant is accepted");
        assert!(find(&all, "minecraft:oak_planks", "minecraft:oak_planks").is_none());
    }

    #[test]
    fn counts_follow_the_rate() {
        let all = run();
        // 3 planks → 6 slabs: 2 per plank, 8 per log.
        assert_eq!(find(&all, "minecraft:oak_log", "minecraft:oak_slab").unwrap().count, 8);
        assert_eq!(find(&all, "minecraft:oak_planks", "minecraft:oak_slab").unwrap().count, 2);
        // 6 planks → 4 stairs: 2.67 per log rounds to 3; one plank makes 0.67, offered as 1.
        assert_eq!(find(&all, "minecraft:oak_log", "minecraft:oak_stairs").unwrap().count, 3);
        assert_eq!(find(&all, "minecraft:oak_planks", "minecraft:oak_stairs").unwrap().count, 1);
        // 6 planks → 3 doors: 2 per log; a plank makes only half a door, so no plank recipe.
        assert_eq!(find(&all, "minecraft:oak_log", "minecraft:oak_door").unwrap().count, 2);
        assert!(find(&all, "minecraft:oak_planks", "minecraft:oak_door").is_none());
        // 3 planks → 4 bowls: 5.33 per log → 6; 1.33 per plank → 2.
        assert_eq!(find(&all, "minecraft:oak_log", "minecraft:bowl").unwrap().count, 6);
        assert_eq!(find(&all, "minecraft:oak_planks", "minecraft:bowl").unwrap().count, 2);
        // 7 slabs = 3.5 planks → 1 composter: 1.14 per log stays 1.
        assert_eq!(find(&all, "minecraft:oak_log", "minecraft:composter").unwrap().count, 1);
    }

    #[test]
    fn mixed_ingredients_use_plank_values() {
        let all = run();
        // 6 planks + 2 slabs = 7 planks → 1 barrel.
        assert_eq!(find(&all, "minecraft:oak_log", "minecraft:barrel").unwrap().count, 1);
        assert_eq!(find(&all, "wood:elder_log", "minecraft:barrel").unwrap().count, 1);
    }

    #[test]
    fn a_non_wood_recipe_does_not_count() {
        // stone → 2 oak slabs must not make slabs cheaper than wood.
        let all = run();
        assert_eq!(find(&all, "minecraft:oak_log", "minecraft:oak_slab").unwrap().count, 8);
    }

    #[test]
    fn second_pass_reaches_crafts_of_crafts() {
        let all = run();
        assert!(find(&all, "minecraft:oak_log", "minecraft:oak_chest_boat").is_some());
    }

    #[test]
    fn unstackable_outputs_give_one() {
        let all = run();
        assert_eq!(find(&all, "minecraft:oak_log", "minecraft:wooden_sword").unwrap().count, 1);
        assert_eq!(find(&all, "minecraft:oak_log", "minecraft:oak_boat").unwrap().count, 1);
    }

    #[test]
    fn family_specific_outputs_stay_in_their_family() {
        let all = run();
        assert!(find(&all, "wood:elder_log", "minecraft:oak_stairs").is_none());
        assert!(find(&all, "wood:elder_log", "wood:elder_slab").is_some());
    }

    #[test]
    fn stick_outputs_are_shared_once() {
        let all = run();
        let sticks: Vec<_> = all.iter().filter(|r| r.output == "minecraft:stick").collect();
        assert_eq!(sticks.len(), 2, "one log recipe and one planks recipe");
        let logs = sticks.iter().find(|r| r.inputs.contains(&"minecraft:oak_log".to_string())).unwrap();
        assert!(logs.inputs.contains(&"wood:elder_log".to_string()));
        assert_eq!(logs.count, 8);
        let ladders: Vec<_> = all.iter().filter(|r| r.output == "minecraft:ladder").collect();
        assert_eq!(ladders.len(), 2);
    }

    #[test]
    fn variants_convert_but_never_unstrip() {
        let all = run();
        let one = |from: &str, to: &str| all.iter().any(|r| r.inputs == vec![from.to_string()] && r.output == to && r.count == 1);
        assert!(one("minecraft:oak_log", "minecraft:oak_wood"));
        assert!(one("minecraft:oak_log", "minecraft:stripped_oak_log"));
        assert!(one("minecraft:stripped_oak_log", "minecraft:stripped_oak_wood"));
        assert!(!one("minecraft:stripped_oak_log", "minecraft:oak_log"));
        assert!(one("wood:elder_log", "wood:elder_stripped_log"));
    }

    #[test]
    fn registry_variants_join_their_family() {
        // The elder planks recipe lists no wood; none exists. Oak's wood comes from the recipe.
        let all = run();
        let planks = find(&all, "wood:elder_log", "wood:elder_planks").unwrap();
        assert_eq!(planks.inputs, s(&["wood:elder_log", "wood:elder_stripped_log"]));
    }

    #[test]
    fn rounding() {
        assert_eq!(round_count(0.2), 1);
        assert_eq!(round_count(1.14), 1);
        assert_eq!(round_count(1.33), 2);
        assert_eq!(round_count(2.0), 2);
        assert_eq!(round_count(2.4), 3);
    }
}
