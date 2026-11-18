//! Spirit fire (`occultism:spirit_fire` recipes, `SpiritFireBlock.entityInside`): an item entity in the fire turns,
//! whole, into the first recipe's result whose ingredient fits it, the result's item with the stack's count
//! (`SpiritFireRecipe.assemble`). Lighting it (`PlayerEventHandler.spiritFire`): flint and steel or a fire charge
//! used on a block with an item of `#occultism:start_spiritfire` (datura) within the 2x2x2 box around the block's
//! center.

use crate::ingredient::Ingredient;
use serde_json::Value;

/// `OccultismTags.Items.START_SPIRIT_FIRE`.
pub const START_TAG: &str = "occultism:start_spiritfire";
/// The fire block.
pub const FIRE: &str = "occultism:spirit_fire";

#[derive(Clone, Debug)]
pub struct SpiritFireRecipe {
    pub id: String,
    pub ingredient: Ingredient,
    /// The result's item and count, as the recipe's `result` names them.
    pub result: Value,
}

impl SpiritFireRecipe {
    pub fn parse(id: &str, json: &Value) -> Result<SpiritFireRecipe, String> {
        let result = json.get("result").filter(|r| r.get("id").is_some()).ok_or(format!("{id}: no result"))?;
        Ok(SpiritFireRecipe {
            id: id.to_owned(),
            ingredient: Ingredient::parse(json.get("ingredient").ok_or(format!("{id}: no ingredient"))?),
            result: result.clone(),
        })
    }

    /// `assemble`: the result with the input's count, whatever the recipe's own count is.
    pub fn assemble(&self, input_count: u32) -> Value {
        let mut out = self.result.clone();
        out["count"] = input_count.into();
        out
    }
}

/// `RecipeManager.getRecipeFor`: the first recipe whose ingredient fits `item`.
pub fn find<'a>(recipes: &'a [SpiritFireRecipe], item: &str, in_tag: &dyn Fn(&str, &str) -> bool) -> Option<&'a SpiritFireRecipe> {
    recipes.iter().find(|r| r.ingredient.test(item, in_tag))
}

/// The box datura is looked for in when the fire is lit at `pos`: `new AABB(-1, -1, -1, 1, 1, 1)` moved to the block's
/// center.
pub fn start_box(pos: [i32; 3]) -> ([f64; 3], [f64; 3]) {
    let c = pos.map(|v| f64::from(v) + 0.5);
    ([c[0] - 1.0, c[1] - 1.0, c[2] - 1.0], [c[0] + 1.0, c[1] + 1.0, c[2] + 1.0])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_whole_stack_turns_and_keeps_its_count() {
        let recipes = vec![
            SpiritFireRecipe::parse("occultism:spirit_fire/otherstone", &json!({"ingredient": {"item": "minecraft:andesite"}, "result": {"count": 1, "id": "occultism:otherstone"}})).unwrap(),
            SpiritFireRecipe::parse("occultism:spirit_fire/awakened_feather", &json!({"ingredient": {"tag": "c:feathers"}, "result": {"count": 1, "id": "occultism:awakened_feather"}})).unwrap(),
        ];
        let tags = |item: &str, tag: &str| item == "minecraft:feather" && tag == "c:feathers";
        let r = find(&recipes, "minecraft:andesite", &tags).unwrap();
        assert_eq!(r.assemble(37), json!({"count": 37, "id": "occultism:otherstone"}));
        assert_eq!(find(&recipes, "minecraft:feather", &tags).unwrap().id, "occultism:spirit_fire/awakened_feather");
        assert!(find(&recipes, "minecraft:stone", &tags).is_none());
        assert_eq!(start_box([10, 64, -3]), ([9.5, 63.5, -3.5], [11.5, 65.5, -1.5]));
    }

    /// Every spirit fire recipe of a carry parses; `OCCULTISM_CARRY` names it (skipped without it).
    #[test]
    fn the_carrys_spirit_fire_recipes_parse() {
        let Some(carry) = std::env::var_os("OCCULTISM_CARRY") else { return };
        let dir = std::path::Path::new(&carry).join("data/occultism/recipe/spirit_fire");
        let n = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| {
                let path = e.unwrap().path();
                let json: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
                SpiritFireRecipe::parse(&path.display().to_string(), &json).unwrap()
            })
            .count();
        assert_eq!(n, 27);
    }
}
