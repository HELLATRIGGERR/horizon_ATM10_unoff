//! The fluid drop (`FluidDropRecipe`, `EntityEvents.entityTick`): an item entity of a recipe's catalyst inside a
//! source block of the recipe's input fluid turns that block into the recipe's output (water and the polymorphic
//! catalyst make polymorphic fluid; polymorphic fluid and a time crystal make time fluid), using one item up.
//! `{"type": "justdirethings:fluiddrop", "input": {"Name": "minecraft:water", "Properties": {"level": "0"}},
//! "output": {..}, "catalyst": "justdirethings:polymorphic_catalyst"}`.
use crate::recipe::StateSpec;
use serde_json::Value;

/// The recipe type.
pub const FLUID_DROP: &str = "justdirethings:fluiddrop";

/// One fluid drop recipe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FluidDrop {
    pub id: String,
    /// A state of the liquid block whose fluid the recipe takes (only its fluid counts).
    pub input: StateSpec,
    /// The block state the source becomes.
    pub output: StateSpec,
    /// The item that has to fall in (`ItemStack.ITEM_NON_AIR_CODEC`: an item id).
    pub catalyst: String,
}

impl FluidDrop {
    /// A recipe file; `id` names it in errors and when the file has none.
    pub fn parse(id: &str, v: &Value) -> Result<Self, String> {
        let input = StateSpec::parse(&v["input"]).map_err(|e| format!("{id}: input: {e}"))?;
        let output = StateSpec::parse(&v["output"]).map_err(|e| format!("{id}: output: {e}"))?;
        let catalyst = match &v["catalyst"] {
            Value::String(s) => s.clone(),
            Value::Object(o) => o.get("id").or_else(|| o.get("item")).and_then(Value::as_str).ok_or_else(|| format!("{id}: catalyst without an id"))?.to_owned(),
            _ => return Err(format!("{id}: no catalyst")),
        };
        Ok(Self { id: v["id"].as_str().unwrap_or(id).to_owned(), input, output, catalyst })
    }
}

/// `findRecipe` and `FluidDropRecipe.matches`: the first recipe in the table's order whose catalyst is `item` and
/// whose input's fluid is the fluid of the block the item is in, which has to be a source. `fluid_of` gives a
/// recipe's input fluid (none for a state the game doesn't have); `here` is the fluid of the block, none for a
/// block without one or one that isn't a source.
pub fn find<'a, F: PartialEq>(recipes: &'a [FluidDrop], item: &str, here: Option<F>, fluid_of: impl Fn(&FluidDrop) -> Option<F>) -> Option<&'a FluidDrop> {
    let here = here?;
    recipes.iter().find(|r| r.catalyst == item && fluid_of(r).is_some_and(|f| f == here))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn time() -> FluidDrop {
        let v = json!({"catalyst": "justdirethings:time_crystal", "id": "justdirethings:time_fluid",
            "input": {"Name": "justdirethings:polymorphic_fluid_block", "Properties": {"level": "0"}},
            "output": {"Name": "justdirethings:time_fluid_block", "Properties": {"level": "0"}}, "type": "justdirethings:fluiddrop"});
        FluidDrop::parse("justdirethings:time_fluid_block-fluiddrop", &v).unwrap()
    }

    fn polymorphic() -> FluidDrop {
        let v = json!({"catalyst": "justdirethings:polymorphic_catalyst", "id": "justdirethings:polymorphic_fluid",
            "input": {"Name": "minecraft:water", "Properties": {"level": "0"}},
            "output": {"Name": "justdirethings:polymorphic_fluid_block", "Properties": {"level": "0"}}});
        FluidDrop::parse("x", &v).unwrap()
    }

    #[test]
    fn a_recipe_reads_as_the_mod_writes_it() {
        let r = time();
        assert_eq!(r.id, "justdirethings:time_fluid");
        assert_eq!(r.input.command(), "justdirethings:polymorphic_fluid_block[level=0]");
        assert_eq!(r.output.command(), "justdirethings:time_fluid_block[level=0]");
        assert_eq!(r.catalyst, "justdirethings:time_crystal");
        let object = json!({"catalyst": {"id": "minecraft:stick"}, "input": {"Name": "minecraft:water"}, "output": {"Name": "minecraft:lava"}});
        assert_eq!(FluidDrop::parse("y", &object).unwrap().catalyst, "minecraft:stick");
        assert!(FluidDrop::parse("z", &json!({"input": {"Name": "minecraft:water"}, "output": {"Name": "minecraft:lava"}})).is_err());
    }

    #[test]
    fn the_catalyst_and_a_source_of_the_inputs_fluid_match() {
        let recipes = vec![polymorphic(), time()];
        let fluid = |r: &FluidDrop| Some(if r.input.name == "minecraft:water" { "water" } else { "polymorphic" });
        let found = |item: &str, here: Option<&'static str>| find(&recipes, item, here, fluid).map(|r| r.id.clone());
        assert_eq!(found("justdirethings:polymorphic_catalyst", Some("water")).as_deref(), Some("justdirethings:polymorphic_fluid"));
        assert_eq!(found("justdirethings:time_crystal", Some("polymorphic")).as_deref(), Some("justdirethings:time_fluid"));
        assert_eq!(found("justdirethings:time_crystal", Some("water")), None, "a time crystal in water makes nothing");
        assert_eq!(found("justdirethings:time_crystal", None), None, "flowing fluid, or no fluid, makes nothing");
        assert_eq!(found("minecraft:stick", Some("water")), None);
    }
}
