//! Every fluid ingredient and fluid result of the NeoForge dump's recipes in
//! the forms slice 6 reads (docs/mod-fluids.md): Mekanism's, Industrial
//! Foregoing's, Create's, ExtendedAE's, Theurgy's and Modern
//! Industrialization's, against a registry of the pack's 733 fluids and its
//! fluid tags. Needs the dump (`HORIZON_ATM10_DUMP`: the folder with
//! `recipes.jsonl`, `tags.jsonl` and `registries.json`, never committed):
//!
//! `HORIZON_ATM10_DUMP=<dump> cargo test -p horizon-world --test fluid_recipes_dump -- --ignored --nocapture`
use horizon_world::fluid_ingredient::{FluidIngredient, SizedFluidIngredient};
use horizon_world::fluid_stack::{FluidStack, fluid_from_json};
use horizon_world::mod_fluids::{ModFluid, register_fluids};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// How a recipe type's fluids read.
enum Form {
    Flat,
    Nested,
    /// Glodium's `IngredientStack.FLUID_CODEC`: `FluidIngredient.CODEC` in `ingredient`.
    Glodium,
    Stack,
    /// Modern Industrialization's input: an ingredient's map, a long amount, a probability.
    MapLong,
    /// Modern Industrialization's output: `{fluid, amount, probability}`, a fluid by id.
    ById,
}

/// The fluid fields of `recipe` (its JSON) with the form each takes.
fn fields(recipe: &Value) -> Vec<(&Value, Form)> {
    let t = recipe["type"].as_str().unwrap_or("");
    let get = |k: &str| recipe.get(k).into_iter();
    let each = |k: &str| {
        recipe
            .get(k)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
    };
    let mut out: Vec<(&Value, Form)> = Vec::new();
    match t {
        "mekanism:evaporating" => {
            out.extend(get("input").map(|v| (v, Form::Flat)));
            out.extend(get("output").map(|v| (v, Form::Stack)));
        }
        "mekanism:separating" => out.extend(get("input").map(|v| (v, Form::Flat))),
        "mekanism:rotary" => {
            out.extend(get("fluid_input").map(|v| (v, Form::Flat)));
            out.extend(get("fluid_output").map(|v| (v, Form::Stack)));
        }
        "mekanism:washing" | "mekanism:reaction" => {
            out.extend(get("fluid_input").map(|v| (v, Form::Flat)))
        }
        "industrialforegoing:dissolution_chamber" => {
            out.extend(get("inputFluid").map(|v| (v, Form::Flat)));
            out.extend(get("outputFluid").map(|v| (v, Form::Stack)));
        }
        "industrialforegoing:fluid_extractor" => {
            out.extend(get("output").map(|v| (v, Form::Stack)))
        }
        t if t.starts_with("create:") => {
            // ProcessingRecipeParams: an ingredient with an amount is a fluid, a result with one too.
            out.extend(
                each("ingredients")
                    .filter(|v| v.get("amount").is_some())
                    .map(|v| (v, Form::Flat)),
            );
            out.extend(
                each("results")
                    .filter(|v| v.get("amount").is_some())
                    .map(|v| (v, Form::Stack)),
            );
        }
        "extendedae:crystal_assembler" => {
            out.extend(get("input_fluid").map(|v| (v, Form::Glodium)))
        }
        t if t.starts_with("theurgy:") => out.extend(
            get("fluid")
                .filter(|v| v.get("ingredient").is_some())
                .map(|v| (v, Form::Nested)),
        ),
        t if t.starts_with("modern_industrialization:") => {
            out.extend(each("fluid_inputs").map(|v| (v, Form::MapLong)));
            out.extend(each("fluid_outputs").map(|v| (v, Form::ById)));
        }
        _ => {}
    }
    out
}

#[test]
#[ignore = "needs the NeoForge dump: HORIZON_ATM10_DUMP"]
fn every_fluid_of_the_dumps_recipes_reads() {
    let Some(dump) = std::env::var_os("HORIZON_ATM10_DUMP").map(PathBuf::from) else {
        eprintln!("HORIZON_ATM10_DUMP is not set");
        return;
    };
    let registries: Value =
        serde_json::from_str(&std::fs::read_to_string(dump.join("registries.json")).unwrap())
            .unwrap();
    let mut tags: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in std::fs::read_to_string(dump.join("tags.jsonl"))
        .unwrap()
        .lines()
    {
        let v: Value = serde_json::from_str(line).unwrap();
        if v["registry"] == "minecraft:fluid" {
            let members = v["values"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned);
            tags.insert(v["id"].as_str().unwrap().to_owned(), members.collect());
        }
    }
    // Every fluid of the pack's registry but vanilla's five, each on its own (pairs don't matter here).
    let vanilla = [
        "minecraft:empty",
        "minecraft:water",
        "minecraft:flowing_water",
        "minecraft:lava",
        "minecraft:flowing_lava",
    ];
    let mut by_fluid: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for (tag, members) in &tags {
        for m in members {
            by_fluid.entry(m.as_str()).or_default().push(tag.clone());
        }
    }
    let fluids: Vec<ModFluid> = registries["minecraft:fluid"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .filter(|id| !vanilla.contains(id))
        .map(|id| ModFluid {
            flowing: String::new(),
            block: "minecraft:air".into(),
            source_tags: by_fluid.get(id).cloned().unwrap_or_default(),
            ..ModFluid::new(id)
        })
        .collect();
    let count = fluids.len();
    assert!(
        register_fluids(fluids, Some(&tags)),
        "a registry of this process's own"
    );
    let mut read: BTreeMap<String, usize> = BTreeMap::new();
    let mut failed: Vec<String> = Vec::new();
    let text = std::fs::read_to_string(dump.join("recipes.jsonl")).unwrap();
    for line in text.lines() {
        let entry: Value = serde_json::from_str(line).unwrap();
        let recipe = &entry["json"];
        for (v, form) in fields(recipe) {
            let result = match form {
                Form::Flat => SizedFluidIngredient::parse_flat(v).map(drop),
                Form::Nested => SizedFluidIngredient::parse_nested(v).map(drop),
                Form::Glodium => FluidIngredient::parse(&v["ingredient"]).map(drop),
                Form::Stack => FluidStack::from_json(v).map(drop),
                Form::MapLong => FluidIngredient::parse_map(v.as_object().unwrap()).map(drop),
                Form::ById => fluid_from_json(&v["fluid"]).map(drop),
            };
            let t = recipe["type"].as_str().unwrap_or("").to_owned();
            match result {
                Ok(()) => *read.entry(t).or_default() += 1,
                Err(e) => failed.push(format!("{} ({t}): {e}: {v}", entry["id"])),
            }
        }
    }
    eprintln!("{count} fluids registered; read by recipe type: {read:?}");
    eprintln!(
        "{} read, {} failed",
        read.values().sum::<usize>(),
        failed.len()
    );
    for f in &failed {
        eprintln!("  {f}");
    }
    assert!(read.values().sum::<usize>() > 1000);
    assert!(
        failed.is_empty(),
        "{} fluid fields don't read",
        failed.len()
    );
}
