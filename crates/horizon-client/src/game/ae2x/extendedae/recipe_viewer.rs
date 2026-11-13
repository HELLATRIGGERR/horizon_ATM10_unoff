//! ExtendedAE's recipe viewer categories, as its `JEIPlugin` registers them: the Circuit Cutter
//! (`CircuitCutterCategory`), the Crystal Assembler (`CrystalAssemblerCategory`) and the Crystal Fixer
//! (`CrystalFixerCategory`), read from the world's tables as `machines` reads them.
use crate::game::{
    Game,
    ae2_recipe_viewer::{category, fluid_slot, output, result, slot, stack},
};
use horizon_client::mod_api::{NativeCategoryRecipe, NativeChanceOutput, NativeRecipeCategory};
use serde_json::Value;

/// `IngredientStack.ITEM_CODEC`: `ingredient`, `amount` (1 when absent).
fn item_stack(game: &Game<'_>, v: &Value) -> Option<Vec<horizon_client::mod_api::NativeStack>> {
    let amount = v.get("amount").map_or(Some(1), Value::as_u64)?.clamp(1, 64) as u32;
    slot(game, v.get("ingredient")?, amount)
}

fn of<'g>(game: &'g Game<'_>, kind: &str) -> impl Iterator<Item = (&'g str, &'g Value)> {
    game.native_recipes_of_type(kind)
        .iter()
        .map(|r| (r.id.as_str(), &*r.json))
}

pub(in crate::game) fn categories(game: &Game<'_>, out: &mut Vec<NativeRecipeCategory>) {
    let eae = "ExtendedAE";
    let cutter = of(game, "extendedae:circuit_cutter")
        .filter_map(|(id, v)| {
            Some(NativeCategoryRecipe {
                inputs: vec![item_stack(game, v.get("input")?)?],
                outputs: vec![output(result(game, v.get("output")?)?, 1.0)],
                notes: vec![format!("Recipe: {id}")],
            })
        })
        .collect();
    let assembler = of(game, "extendedae:crystal_assembler")
        .filter_map(|(id, v)| {
            let mut inputs = Vec::new();
            for i in v.get("input_items")?.as_array()? {
                inputs.push(item_stack(game, i)?);
            }
            let mut notes = Vec::new();
            if let Some(f) = v.get("input_fluid") {
                let amount = f.get("amount").map_or(Some(1), Value::as_u64)?;
                let (buckets, name) = fluid_slot(f.get("ingredient")?)?;
                if !buckets.is_empty() {
                    inputs.push(buckets);
                }
                notes.push(format!("In: {name} {amount} mB"));
            }
            notes.push(format!("Recipe: {id}"));
            Some(NativeCategoryRecipe {
                inputs,
                outputs: vec![output(result(game, v.get("output")?)?, 1.0)],
                notes,
            })
        })
        .collect();
    let fixer = of(game, "extendedae:crystal_fixer")
        .filter_map(|(id, v)| {
            let input = v.get("input")?.get("id")?.as_str()?;
            let made = v.get("output")?.get("id")?.as_str()?;
            let chance = v.get("chance")?.as_u64()?.min(10_000) as f32 / 10_000.0;
            Some(NativeCategoryRecipe {
                inputs: vec![vec![stack(input, 1)], item_stack(game, v.get("fuel")?)?],
                outputs: vec![NativeChanceOutput {
                    notes: vec!["The block in front of it".into()],
                    ..output(stack(made, 1), chance)
                }],
                notes: vec![format!("Recipe: {id}")],
            })
        })
        .collect();
    out.extend(
        [
            category(
                game,
                "extendedae:circuit_cutter",
                "Circuit Cutter",
                eae,
                &["extendedae:circuit_cutter"],
                cutter,
            ),
            category(
                game,
                "extendedae:crystal_assembler",
                "Crystal Assembler",
                eae,
                &["extendedae:crystal_assembler"],
                assembler,
            ),
            category(
                game,
                "extendedae:crystal_fixer",
                "Crystal Fixer",
                eae,
                &["extendedae:crystal_fixer"],
                fixer,
            ),
        ]
        .into_iter()
        .flatten()
        .filter(|c| !c.recipes.is_empty()),
    );
}
