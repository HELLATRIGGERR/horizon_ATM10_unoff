//! Ritual recipes (`occultism:ritual`) and the rules of `Ritual` and `GoldenSacrificialBowlBlockEntity`: which items
//! on the sacrificial bowls fulfil a recipe, the ritual's clock on each tier of bowl, and when each ingredient is
//! taken.

use crate::ingredient::Ingredient;
use serde_json::Value;

/// `Ritual.SACRIFICIAL_BOWL_RANGE`.
pub const SACRIFICIAL_BOWL_RANGE: i32 = 8;
/// `Ritual.SACRIFICE_DETECTION_RANGE_SQUARE`: a sacrifice counts within 8 blocks of the bowl.
pub const SACRIFICE_DETECTION_RANGE_SQUARE: i64 = 8 * 8;
/// `Ritual.ITEM_USE_DETECTION_RANGE_SQUARE`: an item use counts within 16 blocks.
pub const ITEM_USE_DETECTION_RANGE_SQUARE: i64 = 16 * 16;
/// `RitualRecipe.DEFAULT_DURATION`, in the ritual's own time units (seconds on a golden bowl).
pub const DEFAULT_DURATION: i32 = 30;
/// `ritualDurationMultiplier` in `occultism-server.toml`, Occultism's default.
pub const DEFAULT_DURATION_MULTIPLIER: f64 = 1.0;

#[derive(Clone, Debug)]
pub struct RitualRecipe {
    pub id: String,
    /// `occultism:craft`, `occultism:summon`... (`RitualFactory`).
    pub ritual_type: String,
    pub pentacle_id: String,
    pub ingredients: Vec<Ingredient>,
    pub activation_item: Ingredient,
    pub duration: i32,
    /// `duration / (ingredients + 1)`, as `RitualRequirementSettings` computes it in `float`.
    pub duration_per_ingredient: f32,
    /// The `result` stack's JSON, for the engine to read.
    pub result: Option<Value>,
    /// The entity tag to sacrifice and its display name.
    pub entity_to_sacrifice: Option<(String, String)>,
    pub item_to_use: Option<Ingredient>,
    pub entity_to_summon: Option<String>,
    pub condition: Option<Value>,
    pub command: Option<String>,
}

impl RitualRecipe {
    pub fn parse(id: &str, json: &Value) -> Result<RitualRecipe, String> {
        let text = |k: &str| json.get(k).and_then(Value::as_str).map(str::to_owned);
        let ingredients: Vec<Ingredient> =
            json.get("ingredients").and_then(Value::as_array).ok_or(format!("{id}: no ingredients"))?.iter().map(Ingredient::parse).collect();
        let duration = json.get("duration").and_then(Value::as_i64).map_or(DEFAULT_DURATION, |d| d as i32);
        Ok(RitualRecipe {
            id: id.to_owned(),
            ritual_type: text("ritual_type").ok_or(format!("{id}: no ritual_type"))?,
            pentacle_id: text("pentacle_id").ok_or(format!("{id}: no pentacle_id"))?,
            activation_item: Ingredient::parse(json.get("activation_item").ok_or(format!("{id}: no activation_item"))?),
            duration_per_ingredient: duration as f32 / (ingredients.len() + 1) as f32,
            ingredients,
            duration,
            result: json.get("result").filter(|r| r.get("id").is_some()).cloned(),
            entity_to_sacrifice: json.get("entity_to_sacrifice").and_then(|e| {
                Some((e.get("tag")?.as_str()?.trim_start_matches('#').to_owned(), e.get("display_name")?.as_str()?.to_owned()))
            }),
            item_to_use: json.get("item_to_use").map(Ingredient::parse),
            entity_to_summon: text("entity_to_summon"),
            condition: json.get("condition").cloned(),
            command: text("command"),
        })
    }

    pub fn requires_sacrifice(&self) -> bool {
        self.entity_to_sacrifice.is_some()
    }

    /// `requiresItemUse`: an `item_to_use` that names something.
    pub fn requires_item_use(&self) -> bool {
        self.item_to_use.as_ref().is_some_and(|i| !matches!(i, Ingredient::Any(c) if c.is_empty()))
    }

    /// The ritual's key in messages (`Ritual.getRitualID`): `occultism:ritual/craft_x` gives `occultism.craft_x`.
    pub fn ritual_key(&self) -> String {
        let (ns, path) = self.id.split_once(':').unwrap_or(("minecraft", &self.id));
        let path = path.split_once('/').map_or(path, |(_, rest)| rest);
        format!("{ns}.{path}")
    }
}

/// `Ritual.matchesAdditionalIngredients`: each ingredient takes the first item left that fits it, so no item counts
/// twice; items beyond the ingredients don't spoil it. With `count_matching` (`enableRemainingIngredientCountMatching`,
/// off by default) the numbers must also agree.
pub fn matches_ingredients(ingredients: &[Ingredient], items: &[String], count_matching: bool, in_tag: &dyn Fn(&str, &str) -> bool) -> bool {
    if count_matching && ingredients.len() != items.len() {
        return false;
    }
    let mut left: Vec<&String> = items.iter().collect();
    for ingredient in ingredients {
        let Some(i) = left.iter().position(|item| ingredient.test(item, in_tag)) else {
            return false;
        };
        left.remove(i);
    }
    true
}

/// `Ritual.getRemainingAdditionalIngredients`: the ingredients (their indices) that `consumed` doesn't cover yet.
pub fn remaining_ingredients(ingredients: &[Ingredient], consumed: &[String], in_tag: &dyn Fn(&str, &str) -> bool) -> Vec<usize> {
    let mut left: Vec<&String> = consumed.iter().collect();
    let mut out = Vec::new();
    for (index, ingredient) in ingredients.iter().enumerate() {
        match left.iter().position(|item| ingredient.test(item, in_tag)) {
            Some(i) => {
                left.remove(i);
            }
            None => out.push(index),
        }
    }
    out
}

/// How many ingredients `Ritual.consumeAdditionalIngredients` takes at ritual time `time`, with `consumed` taken so
/// far: `floor(time / durationPerIngredient) - consumed` (never below 0).
pub fn ingredients_due(time: i32, duration_per_ingredient: f32, consumed: usize) -> usize {
    let total = (time as f32 / duration_per_ingredient).floor() as i64;
    (total - consumed as i64).max(0) as usize
}

/// The tier of a golden bowl block (`GoldenSacrificialBowlBlockEntity.getTier`): 1 golden, 2 iesnium, 3 the chalices,
/// 0 for anything else.
pub fn bowl_tier(block: &str) -> u8 {
    match block {
        "occultism:golden_sacrificial_bowl" | "occultism:dark_golden_sacrificial_bowl" => 1,
        "occultism:iesnium_sacrificial_bowl" | "occultism:dark_iesnium_sacrificial_bowl" => 2,
        "occultism:eldritch_chalice" | "occultism:celestial_chalice" => 3,
        _ => 0,
    }
}

/// The ritual's time after one tick at `game_time` (`GoldenSacrificialBowlBlockEntity.tick`): a golden bowl adds one
/// every `(int) (20 * multiplier)` ticks, an iesnium bowl every `(int) (5 * multiplier)` (every tick below 0.2), and
/// the chalices finish at once.
pub fn advance(tier: u8, game_time: i64, multiplier: f64, duration: i32, time: i32) -> i32 {
    let every = |n: f64| {
        let n = (n * multiplier) as i64;
        n > 0 && game_time % n == 0
    };
    match tier {
        1 if every(20.0) => time + 1,
        1 => time,
        2 if multiplier < 0.2 || every(5.0) => time + 1,
        2 => time,
        _ => duration,
    }
}

/// Whether a ritual at `time` is done (`duration >= 0 && time >= duration`).
pub fn finished(duration: i32, time: i32) -> bool {
    duration >= 0 && time >= duration
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tags(item: &str, tag: &str) -> bool {
        matches!(
            (item, tag),
            ("minecraft:iron_ingot", "c:ingots/iron") | ("minecraft:egg", "c:eggs") | ("minecraft:glass", "c:glass_blocks")
        )
    }

    /// The shape of ATM10's `occultism:ritual/craft_fragile_soul_gem` (no `duration`: the default 30).
    fn soul_gem() -> RitualRecipe {
        let json = json!({
            "type": "occultism:ritual", "ritual_type": "occultism:craft", "pentacle_id": "occultism:craft_foliot",
            "activation_item": {"item": "occultism:book_of_binding_bound_foliot"},
            "ingredients": [{"tag": "c:ingots/iron"}, {"tag": "c:eggs"}, {"tag": "c:glass_blocks"}],
            "result": {"count": 1, "id": "occultism:fragile_soul_gem"},
            "ritual_dummy": {"count": 1, "id": "occultism:ritual_dummy/craft_fragile_soul_gem"}
        });
        RitualRecipe::parse("occultism:ritual/craft_fragile_soul_gem", &json).unwrap()
    }

    #[test]
    fn a_recipe_reads_with_its_defaults() {
        let r = soul_gem();
        assert_eq!((r.duration, r.duration_per_ingredient), (30, 7.5));
        assert_eq!(r.ritual_key(), "occultism.craft_fragile_soul_gem");
        assert!(!r.requires_sacrifice() && !r.requires_item_use());
        assert_eq!(r.result.as_ref().unwrap()["id"], "occultism:fragile_soul_gem");
        let summon = RitualRecipe::parse(
            "occultism:ritual/summon_x",
            &json!({"ritual_type": "occultism:summon", "pentacle_id": "p:x", "activation_item": {"item": "a:b"}, "ingredients": [],
                   "duration": 60, "entity_to_sacrifice": {"tag": "c:cows", "display_name": "ritual.occultism.sacrifice.cows"},
                   "item_to_use": {"item": "minecraft:egg"}, "result": {"id": "minecraft:air"}}),
        )
        .unwrap();
        assert_eq!(summon.duration_per_ingredient, 60.0, "no ingredients: the whole duration");
        assert_eq!(summon.entity_to_sacrifice, Some(("c:cows".into(), "ritual.occultism.sacrifice.cows".into())));
        assert!(summon.requires_sacrifice() && summon.requires_item_use());
    }

    #[test]
    fn bowls_fulfil_a_recipe_one_item_per_ingredient_in_any_order() {
        let r = soul_gem();
        let items = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(matches_ingredients(&r.ingredients, &items(&["minecraft:glass", "minecraft:iron_ingot", "minecraft:egg"]), false, &tags));
        assert!(!matches_ingredients(&r.ingredients, &items(&["minecraft:glass", "minecraft:iron_ingot"]), false, &tags));
        assert!(!matches_ingredients(&r.ingredients, &items(&["minecraft:egg", "minecraft:egg", "minecraft:egg"]), false, &tags));
        let extra = items(&["minecraft:glass", "minecraft:iron_ingot", "minecraft:egg", "minecraft:stone"]);
        assert!(matches_ingredients(&r.ingredients, &extra, false, &tags), "an extra item doesn't spoil it");
        assert!(!matches_ingredients(&r.ingredients, &extra, true, &tags), "unless the counts must agree");
        assert_eq!(remaining_ingredients(&r.ingredients, &items(&["minecraft:egg"]), &tags), vec![0, 2]);
    }

    #[test]
    fn a_golden_bowl_takes_an_ingredient_every_seven_and_a_half_seconds_and_finishes_at_thirty() {
        let r = soul_gem();
        // The clock: one unit each 20 game ticks, on game times that are multiples of 20.
        let (mut time, mut consumed, mut taken_at, mut done_at) = (0, 0usize, Vec::new(), None);
        for game_time in 1..=700i64 {
            time = advance(1, game_time, DEFAULT_DURATION_MULTIPLIER, r.duration, time);
            for _ in 0..ingredients_due(time, r.duration_per_ingredient, consumed).min(r.ingredients.len() - consumed) {
                consumed += 1;
                taken_at.push(game_time);
            }
            if finished(r.duration, time) {
                done_at = Some(game_time);
                break;
            }
        }
        // floor(t / 7.5) reaches 1, 2, 3 at t = 8, 15, 23 seconds.
        assert_eq!(taken_at, vec![160, 300, 460]);
        assert_eq!(done_at, Some(600), "30 units of 20 ticks");
    }

    #[test]
    fn the_other_tiers_and_the_multiplier() {
        assert_eq!(advance(2, 10, 1.0, 150, 3), 4, "iesnium: every 5 ticks");
        assert_eq!(advance(2, 11, 1.0, 150, 3), 3);
        assert_eq!(advance(2, 11, 0.1, 150, 3), 4, "below 0.2 every tick");
        assert_eq!(advance(3, 11, 1.0, 150, 3), 150, "a chalice finishes at once");
        assert_eq!(advance(1, 10, 0.5, 30, 0), 1, "a multiplier of 0.5: every 10 ticks");
        assert_eq!(bowl_tier("occultism:dark_golden_sacrificial_bowl"), 1);
        assert_eq!(bowl_tier("occultism:sacrificial_bowl"), 0);
        assert_eq!(ingredients_due(7, 7.5, 0), 0);
        assert_eq!(ingredients_due(30, 7.5, 3), 1, "a fourth is due, though only three exist: the caller stops at the list's end");
        assert!(finished(30, 30) && !finished(30, 29) && !finished(-1, 1000));
    }

    /// Every ritual of a carry parses; `OCCULTISM_CARRY` names it (skipped without it).
    #[test]
    fn the_carrys_rituals_parse() {
        let Some(carry) = std::env::var_os("OCCULTISM_CARRY") else { return };
        let dir = std::path::Path::new(&carry).join("data/occultism/recipe/ritual");
        let mut kinds = std::collections::BTreeMap::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let json: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            let id = format!("occultism:ritual/{}", path.file_stem().unwrap().to_string_lossy());
            let r = RitualRecipe::parse(&id, &json).unwrap();
            assert!(r.ingredients.iter().all(|i| !matches!(i, Ingredient::Unsupported(_))), "{id}: {:?}", r.ingredients);
            *kinds.entry(r.ritual_type).or_insert(0) += 1;
        }
        assert_eq!(kinds.get("occultism:craft"), Some(&47), "{kinds:?}");
    }
}
