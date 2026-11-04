//! Fluid ingredients and results of real ATM10 8.4 recipes (the NeoForge
//! dump's final recipes, by id), read and matched against the fluid registry
//! with the full pack's fluid tags: Mekanism's, Industrial Foregoing's,
//! Modern Industrialization's, Create's, ExtendedAE's and Theurgy's forms
//! (docs/mod-fluids.md, slice 6). Its own process: the registry holds these
//! mods' fluids by their real ids.
use horizon_world::fluid::FluidKind;
use horizon_world::fluid_ingredient::{FluidIngredient, SizedFluidIngredient};
use horizon_world::fluid_stack::{FluidStack, fluid_from_json, positive_int};
use horizon_world::mod_fluids::{ModFluid, fluids_in_tag, register_fluids};
use horizon_world::nbt::{Compound, Tag};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Once;

/// The fluids these recipes name, by the ids and flowing ids the pack
/// registers (`registries.json`), and the full pack's tags over them
/// (`tags.jsonl`), with `c:milk` naming NeoForge's milk, which Horizon's
/// registry lacks.
fn registry() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let fluid = |id: &str, flowing: &str| ModFluid {
            flowing: flowing.into(),
            block: "minecraft:air".into(),
            ..ModFluid::new(id)
        };
        let fluids = [
            fluid("mekanism:brine", "mekanism:flowing_brine"),
            fluid("mekanism:lithium", "mekanism:flowing_lithium"),
            fluid("mekanism:heavy_water", "mekanism:flowing_heavy_water"),
            fluid(
                "mekanism:superheated_sodium",
                "mekanism:flowing_superheated_sodium",
            ),
            fluid(
                "industrialforegoing:latex",
                "industrialforegoing:latex_flowing",
            ),
            fluid(
                "industrialforegoing:pink_slime",
                "industrialforegoing:pink_slime_flowing",
            ),
            fluid("create:tea", "create:flowing_tea"),
            fluid("create:potion", "create:flowing_potion"),
            fluid(
                "the_bumblezone:sugar_water_still",
                "the_bumblezone:sugar_water_flowing",
            ),
            // Modern Industrialization's fluids don't flow.
            fluid("modern_industrialization:heavy_water", ""),
            fluid("modern_industrialization:deuterium", ""),
            fluid("modern_industrialization:oxygen", ""),
        ];
        let lists = |pairs: &[(&str, &[&str])]| -> BTreeMap<String, Vec<String>> {
            pairs
                .iter()
                .map(|(t, m)| ((*t).to_owned(), m.iter().map(|s| (*s).to_owned()).collect()))
                .collect()
        };
        let tags = lists(&[
            (
                "minecraft:water",
                &[
                    "minecraft:water",
                    "minecraft:flowing_water",
                    "the_bumblezone:sugar_water_still",
                    "the_bumblezone:sugar_water_flowing",
                ],
            ),
            ("c:brine", &["mekanism:brine", "mekanism:flowing_brine"]),
            (
                "c:heavy_water",
                &[
                    "mekanism:heavy_water",
                    "mekanism:flowing_heavy_water",
                    "modern_industrialization:heavy_water",
                ],
            ),
            (
                "c:superheated_sodium",
                &[
                    "mekanism:superheated_sodium",
                    "mekanism:flowing_superheated_sodium",
                ],
            ),
            ("c:milk", &["minecraft:milk", "minecraft:flowing_milk"]),
            (
                "create:bottomless/allow",
                &["minecraft:water", "minecraft:lava"],
            ),
        ]);
        let mut by_fluid: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (tag, members) in &tags {
            for m in members {
                by_fluid.entry(m.clone()).or_default().push(tag.clone());
            }
        }
        let fluids = fluids.map(|f| ModFluid {
            source_tags: by_fluid.get(&f.id).cloned().unwrap_or_default(),
            flowing_tags: by_fluid.get(&f.flowing).cloned().unwrap_or_default(),
            ..f
        });
        assert!(register_fluids(fluids, Some(&tags)));
    });
}

fn kind(name: &str) -> FluidKind {
    registry();
    FluidKind::from_name(name).unwrap_or_else(|| panic!("{name} registered"))
}

fn stack(name: &str, amount: i32) -> FluidStack {
    FluidStack::new(kind(name), amount)
}

fn recipe(text: &str) -> Value {
    registry();
    serde_json::from_str(text).unwrap()
}

#[test]
fn mekanism_evaporating_takes_the_full_packs_water_tag() {
    // mekanism:evaporating/brine (SizedFluidIngredient.FLAT_CODEC in, FluidStack.CODEC out).
    let r = recipe(
        r#"{"input": {"tag": "minecraft:water", "amount": 10}, "output": {"id": "mekanism:brine", "amount": 1}, "type": "mekanism:evaporating"}"#,
    );
    let input = SizedFluidIngredient::parse_flat(&r["input"]).unwrap();
    for water in [
        "minecraft:water",
        "minecraft:flowing_water",
        "the_bumblezone:sugar_water_still",
    ] {
        assert!(
            input.test(&stack(water, 10)),
            "{water} is in the pack's minecraft:water"
        );
    }
    assert!(!input.test(&stack("minecraft:water", 9)));
    assert!(!input.test(&stack("minecraft:lava", 1000)) && !input.test(&FluidStack::EMPTY));
    // getFluids, in the tag's order: vanilla's first.
    assert_eq!(
        input
            .fluids()
            .iter()
            .map(FluidStack::name)
            .collect::<Vec<_>>(),
        [
            "minecraft:water",
            "minecraft:flowing_water",
            "the_bumblezone:sugar_water_still",
            "the_bumblezone:sugar_water_flowing"
        ]
    );
    assert_eq!(
        FluidStack::from_json(&r["output"]),
        Ok(stack("mekanism:brine", 1))
    );
    // mekanism:evaporating/lithium: brine by c:brine, either form.
    let r = recipe(
        r#"{"input": {"tag": "c:brine", "amount": 10}, "output": {"id": "mekanism:lithium", "amount": 1}, "type": "mekanism:evaporating"}"#,
    );
    let input = SizedFluidIngredient::parse_flat(&r["input"]).unwrap();
    assert!(
        input.test(&stack("mekanism:brine", 10))
            && input.test(&stack("mekanism:flowing_brine", 64))
    );
    assert!(!input.test(&stack("minecraft:water", 10)));
    assert_eq!(
        FluidStack::from_json(&r["output"]).unwrap().name(),
        "mekanism:lithium"
    );
}

#[test]
fn mekanisms_fluid_inputs_beside_its_chemicals() {
    // mekanism:rotary/superheated_sodium: a fluid in and a fluid out beside the chemicals.
    let r = recipe(
        r#"{"fluid_input": {"tag": "c:superheated_sodium", "amount": 1}, "chemical_input": {"chemical": "mekanism:superheated_sodium", "amount": 1}, "chemical_output": {"id": "mekanism:superheated_sodium", "amount": 1}, "fluid_output": {"id": "mekanism:superheated_sodium", "amount": 1}, "type": "mekanism:rotary"}"#,
    );
    assert!(
        SizedFluidIngredient::parse_flat(&r["fluid_input"])
            .unwrap()
            .test(&stack("mekanism:superheated_sodium", 1))
    );
    assert_eq!(
        FluidStack::from_json(&r["fluid_output"]),
        Ok(stack("mekanism:superheated_sodium", 1))
    );
    // mekanismgenerators:separator/heavy_water: the pack's c:heavy_water also holds Modern Industrialization's.
    let r = recipe(
        r#"{"input": {"tag": "c:heavy_water", "amount": 2}, "energy_multiplier": 2, "left_chemical_output": {"id": "mekanismgenerators:deuterium", "amount": 2}, "right_chemical_output": {"id": "mekanism:oxygen", "amount": 1}, "type": "mekanism:separating"}"#,
    );
    let input = SizedFluidIngredient::parse_flat(&r["input"]).unwrap();
    assert!(
        input.test(&stack("mekanism:heavy_water", 2))
            && input.test(&stack("modern_industrialization:heavy_water", 2))
    );
    assert!(!input.test(&stack("mekanism:heavy_water", 1)));
    // mekanism:processing/lategame/polonium_pellet/from_reaction: a bucket of water.
    let r = recipe(
        r#"{"duration": 100, "item_output": {"id": "mekanism:pellet_polonium", "count": 1}, "chemical_output": {"id": "mekanism:spent_nuclear_waste", "amount": 1000}, "item_input": {"tag": "c:dusts/fluorite", "count": 1}, "fluid_input": {"tag": "minecraft:water", "amount": 1000}, "chemical_input": {"chemical": "mekanism:polonium", "amount": 1000}, "type": "mekanism:reaction"}"#,
    );
    let water = SizedFluidIngredient::parse_flat(&r["fluid_input"]).unwrap();
    assert_eq!(water.amount, 1000);
    assert!(water.test(&stack("the_bumblezone:sugar_water_flowing", 1000)));
}

#[test]
fn industrial_foregoing_names_one_fluid_and_gives_a_stack() {
    // industrialforegoing:dissolution_chamber/pink_slime_ball: a single fluid in, a stack out.
    let r = recipe(
        r#"{"processingTime": 200, "output": {"id": "industrialforegoing:pink_slime", "count": 1}, "outputFluid": {"id": "minecraft:water", "amount": 150}, "input": [{"item": "minecraft:glass_pane"}], "inputFluid": {"fluid": "industrialforegoing:pink_slime", "amount": 300}, "type": "industrialforegoing:dissolution_chamber"}"#,
    );
    let input = SizedFluidIngredient::parse_flat(&r["inputFluid"]).unwrap();
    assert_eq!(
        input.ingredient,
        FluidIngredient::Single(kind("industrialforegoing:pink_slime"))
    );
    assert!(input.test(&stack("industrialforegoing:pink_slime", 300)));
    assert!(
        !input.test(&stack("industrialforegoing:pink_slime_flowing", 300)),
        "a single fluid by identity"
    );
    assert_eq!(
        FluidStack::from_json(&r["outputFluid"]),
        Ok(stack("minecraft:water", 150))
    );
    // industrialforegoing:fluid_extractor/default: latex, 1 mB a step.
    let r = recipe(
        r#"{"breakChance": 0.01, "output": {"id": "industrialforegoing:latex", "amount": 1}, "defaultRecipe": true, "input": {"tag": "minecraft:logs"}, "result": {"Name": "minecraft:air"}, "type": "industrialforegoing:fluid_extractor"}"#,
    );
    assert_eq!(
        FluidStack::from_json(&r["output"]),
        Ok(stack("industrialforegoing:latex", 1))
    );
}

/// Modern Industrialization's own `MachineRecipe.FluidInput`: NeoForge's
/// ingredient map (`FluidIngredient.MAP_CODEC_NONEMPTY`: a fluid or a tag)
/// beside a long amount and a probability (1 when absent).
fn mi_input(v: &Value) -> (FluidIngredient, i64, f64) {
    (
        FluidIngredient::parse_map(v.as_object().unwrap()).unwrap(),
        v["amount"].as_i64().unwrap(),
        v.get("probability").and_then(Value::as_f64).unwrap_or(1.0),
    )
}

/// Its `FluidOutput`: a fluid by id, an amount, a probability.
fn mi_output(v: &Value) -> (FluidKind, i64, f64) {
    (
        fluid_from_json(&v["fluid"]).unwrap(),
        v["amount"].as_i64().unwrap(),
        v.get("probability").and_then(Value::as_f64).unwrap_or(1.0),
    )
}

#[test]
fn modern_industrialization_takes_ingredients_and_gives_fluids_by_id() {
    // modern_industrialization:materials/electrolyzer/heavy_water.
    let r = recipe(
        r#"{"fluid_inputs": [{"fluid": "modern_industrialization:heavy_water", "amount": 3000}], "fluid_outputs": [{"fluid": "modern_industrialization:deuterium", "amount": 2000}, {"fluid": "modern_industrialization:oxygen", "amount": 1000}], "eu": 8, "duration": 600, "type": "modern_industrialization:electrolyzer"}"#,
    );
    let (heavy, amount, chance) = mi_input(&r["fluid_inputs"][0]);
    assert_eq!((amount, chance), (3000, 1.0));
    assert!(heavy.test(&stack("modern_industrialization:heavy_water", 1)));
    assert!(
        !heavy.test(&stack("mekanism:heavy_water", 1)),
        "Mekanism's heavy water is another fluid"
    );
    let outputs: Vec<_> = r["fluid_outputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(mi_output)
        .collect();
    assert_eq!(outputs[0].0.name(), "modern_industrialization:deuterium");
    assert_eq!(
        outputs[1],
        (kind("modern_industrialization:oxygen"), 1000, 1.0)
    );
    // A fluid that doesn't flow has no flowing fluid by name.
    assert_eq!(FluidKind::from_name(""), None);
    // modern_industrialization:petrochem/distillation/crude_oil_full: an input by tag
    // (here c:heavy_water for a fluid this registry has), so either heavy water.
    let (by_tag, amount, _) = mi_input(&json!({"amount": 1000, "tag": "c:heavy_water"}));
    assert_eq!(amount, 1000);
    assert!(
        by_tag.test(&stack("mekanism:flowing_heavy_water", 1))
            && by_tag.test(&stack("modern_industrialization:heavy_water", 1))
    );
}

#[test]
fn create_mixes_typed_ingredients_and_fills_by_components() {
    // create:mixing/tea: a typed single fluid and a typed tag; a fluid result.
    let r = recipe(
        r#"{"ingredients": [{"tag": "minecraft:leaves"}, {"type": "neoforge:single", "fluid": "minecraft:water", "amount": 250}, {"type": "neoforge:tag", "tag": "c:milk", "amount": 250}], "results": [{"id": "create:tea", "amount": 500}], "heat_requirement": "heated", "type": "create:mixing"}"#,
    );
    let water = SizedFluidIngredient::parse_flat(&r["ingredients"][1]).unwrap();
    assert!(
        water.test(&stack("minecraft:water", 250))
            && !water.test(&stack("the_bumblezone:sugar_water_still", 250))
    );
    // NeoForge's milk is in the pack's c:milk; Horizon's registry has no milk, so the tag names nothing.
    let milk = SizedFluidIngredient::parse_flat(&r["ingredients"][2]).unwrap();
    assert!(milk.ingredient.has_no_fluids() && fluids_in_tag("c:milk").is_empty());
    assert_eq!(
        FluidStack::from_json(&r["results"][0]),
        Ok(stack("create:tea", 500))
    );
    // create:compacting/andesite_from_flint: 100 mB of lava.
    let r = recipe(
        r#"{"ingredients": [{"item": "minecraft:flint"}, {"item": "minecraft:flint"}, {"item": "minecraft:gravel"}, {"type": "neoforge:single", "fluid": "minecraft:lava", "amount": 100}], "results": [{"id": "minecraft:andesite"}], "type": "create:compacting"}"#,
    );
    let lava = SizedFluidIngredient::parse_flat(&r["ingredients"][3]).unwrap();
    assert!(
        lava.test(&stack("minecraft:lava", 100))
            && !lava.test(&stack("minecraft:flowing_lava", 100))
    );
    // create:filling/glowstone: a potion fluid with its components (`neoforge:components`).
    let r = recipe(
        r#"{"ingredients": [{"item": "create:cinder_flour"}, {"type": "neoforge:components", "fluids": "create:potion", "components": {"minecraft:potion_contents": {"potion": "minecraft:night_vision"}, "create:potion_fluid_bottle_type": "regular"}, "amount": 25}], "results": [{"id": "minecraft:glowstone_dust"}], "type": "create:filling"}"#,
    );
    let potion = SizedFluidIngredient::parse_flat(&r["ingredients"][1]).unwrap();
    let brewed = |name: &str, extra: bool| {
        let mut contents = Compound::new();
        contents.insert("potion".into(), Tag::String(name.into()));
        let mut c = Compound::new();
        c.insert("minecraft:potion_contents".into(), Tag::Compound(contents));
        c.insert(
            "create:potion_fluid_bottle_type".into(),
            Tag::String("regular".into()),
        );
        if extra {
            c.insert("minecraft:custom_name".into(), Tag::String("\"x\"".into()));
        }
        FluidStack::with_components(kind("create:potion"), 25, c)
    };
    assert!(potion.test(&brewed("minecraft:night_vision", false)));
    assert!(
        potion.test(&brewed("minecraft:night_vision", true)),
        "not strict: more components are fine"
    );
    assert!(!potion.test(&brewed("minecraft:swiftness", false)));
    assert!(
        !potion.test(&stack("create:potion", 25)),
        "a potion with no contents"
    );
    assert_eq!(potion.fluids()[0], brewed("minecraft:night_vision", false));
}

#[test]
fn extendedae_and_theurgy_nest_their_ingredient() {
    // extendedae:assembler/budding: Glodium's `IngredientStack.FLUID_CODEC`, `{ingredient, amount}`
    // (`FluidIngredient.CODEC`, 1 mB when absent; the Crystal Assembler reads it so).
    let r = recipe(
        r#"{"output": {"id": "extendedae:entro_budding_mostly", "count": 1}, "input_items": [{"ingredient": {"item": "extendedae:entro_seed"}}, {"ingredient": {"item": "ae2:fluix_block"}}], "input_fluid": {"ingredient": {"fluid": "minecraft:water"}, "amount": 200}, "type": "extendedae:crystal_assembler"}"#,
    );
    let water = FluidIngredient::parse(&r["input_fluid"]["ingredient"]).unwrap();
    assert_eq!(water, FluidIngredient::Single(FluidKind::Water));
    assert_eq!(positive_int(&r["input_fluid"]["amount"]), Ok(200));
    // A theurgy:fermentation recipe's fluid: SizedFluidIngredient.NESTED_CODEC.
    let water = SizedFluidIngredient::parse_nested(
        &json!({"ingredient": {"fluid": "minecraft:water"}, "amount": 500}),
    )
    .unwrap();
    assert!(
        water.test(&stack("minecraft:water", 500)) && !water.test(&stack("minecraft:water", 499))
    );
}

#[test]
fn vanilla_fluids_carry_the_full_packs_tags() {
    registry();
    assert!(FluidStack::new(FluidKind::Water, 1).is_tag("create:bottomless/allow"));
    assert!(FluidStack::new(FluidKind::Lava, 1).is_tag("create:bottomless/allow"));
    assert!(!FluidStack::new(FluidKind::FlowingWater, 1).is_tag("create:bottomless/allow"));
    // NeoForge's own tags hold vanilla's fluids whatever the root says.
    assert!(FluidStack::new(FluidKind::FlowingLava, 1).is_tag("c:lava"));
    assert!(!FluidStack::new(kind("the_bumblezone:sugar_water_still"), 1).is_tag("c:water"));
}
