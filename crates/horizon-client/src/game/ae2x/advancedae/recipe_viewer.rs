//! Advanced AE's recipe viewer category, as its `JEIPlugin` registers it: the Reaction Chamber
//! (`ReactionChamberCategory`), read from the world's tables as `chamber` reads them.
use crate::game::{
    Game,
    ae2_recipe_viewer::{category, fluid_name, fluid_slot, output, slot, stack},
};
use horizon_client::mod_api::{NativeCategoryRecipe, NativeRecipeCategory};
use horizon_world::{fluid::FluidKind, fluid_stack};
use serde_json::Value;

pub(in crate::game) fn categories(game: &Game<'_>, out: &mut Vec<NativeRecipeCategory>) {
    let recipes = game
        .native_recipes_of_type("advanced_ae:reaction")
        .iter()
        .filter_map(|r| {
            let v = &*r.json;
            let mut inputs = Vec::new();
            for i in v.get("input_items")?.as_array()? {
                let amount = i.get("amount").map_or(Some(1), Value::as_u64)?.clamp(1, 64) as u32;
                // An empty ingredient is a slot the recipe leaves free (`getValidInputs`).
                if let Some(items) = slot(game, i.get("ingredient")?, amount) {
                    inputs.push(items);
                }
            }
            let fluid = v.get("input_fluid")?;
            let amount = fluid.get("amount").map_or(Some(1), Value::as_u64)?;
            let (buckets, name) = fluid_slot(fluid.get("ingredient")?)?;
            if !buckets.is_empty() {
                inputs.push(buckets);
            }
            let mut notes = vec![format!("In: {name} {amount} mB")];
            // `GenericStack.CODEC`: `#t` (`ae2:i` or `ae2:f`), `id`, `#` the amount.
            let made = v.get("output")?;
            let id = made.get("id")?.as_str()?;
            let count = made.get("#")?.as_u64()?;
            let outputs = match made.get("#t")?.as_str()? {
                "ae2:f" => {
                    let fluid = FluidKind::from_name(id)?;
                    notes.push(format!("Out: {} {count} mB", fluid_name(fluid)));
                    fluid_stack::filled_bucket(fluid)
                        .map(|b| output(stack(b, 1), 1.0))
                        .into_iter()
                        .collect()
                }
                _ => {
                    game.items().get(id)?;
                    vec![output(stack(id, u32::try_from(count).ok()?), 1.0)]
                }
            };
            notes.push(format!("{} AE", v.get("input_energy")?.as_i64()?));
            notes.push(format!("Recipe: {}", r.id));
            Some(NativeCategoryRecipe {
                inputs,
                outputs,
                notes,
            })
        })
        .collect();
    if let Some(c) = category(
        game,
        "advanced_ae:reaction",
        "Reaction Chamber",
        "Advanced AE",
        &["advanced_ae:reaction_chamber"],
        recipes,
    ) && !c.recipes.is_empty()
    {
        out.push(c);
    }
}
