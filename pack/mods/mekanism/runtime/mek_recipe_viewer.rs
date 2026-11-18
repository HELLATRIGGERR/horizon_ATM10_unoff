// Mekani"inputs": r.inputs.iter().map(|slot| slot.iter().take(4).map(|s| format!("{}x{}", s.count, if s.is_item() { s.item.clone() } else { format!("{} {}", s.kind, s.item) }))m's recipe viewer categories, as its JEI plugin (`mekanism.client.recipe_viewer.jei.MekanismJEI`,
// `RecipeViewerRecipeType`, and Mekanism Generators' `GeneratorsJEI`) registers them, for whatever viewer reads
// `native_recipe_categories`: one category per machine of this port, its catalysts the machine's blocks (and the
// factories that run the same recipes), its recipes read from the world's tables as the machines read them.
//
// Fluids and chemicals are ingredients of their own (hook points API 440: `NativeStack::fluid`, `NativeStack::typed`
// of kind `mekanism:chemical`, each category listing its chemicals' sprites and tints), as Mekanism's JEI plugin
// registers its chemical ingredient type; energy is a note in Joules.
// The recipes are the window's own content (the `mekanism` content mod's), the same on a server's client.
use super::{Game, mek_chemicals, mek_machines};
use horizon_client::mod_api::{
    NativeCategoryRecipe, NativeChanceOutput, NativeIngredientInfo, NativeRecipeCategory, NativeStack,
};
use horizon_world::{fluid::FluidKind, fluid_ingredient::SizedFluidIngredient};
use mekanism_energy::{
    chemical::{ChemicalRegistry, ChemicalStack, ChemicalStackIngredient},
    factory::{FactoryTier, FactoryType},
    machine::MachineKind,
};
use serde_json::{Value, json};

/// One part of a recipe's JSON and what it is.
#[derive(Clone, Copy)]
enum Part {
    /// An item ingredient (`{item|tag, count}`) or an item result (`{id, count}`).
    Item(&'static str),
    /// A chemical ingredient (`{chemical|tag, amount}`) or result (`{id, amount}`).
    Chemical(&'static str),
    /// A fluid ingredient (`{fluid|tag, amount}`) or result (`{id, amount}`).
    Fluid(&'static str),
    /// Joules (`energy_conversion`'s `output`).
    Energy(&'static str),
    /// The sawmill's secondary output and the field of its chance (the
    /// reaction chamber's item output, which a recipe may leave out).
    Chance(&'static str, &'static str),
    /// A chemical result a recipe may leave out (the reaction chamber's).
    MaybeChemical(&'static str),
}

/// What the catalysts are.
enum Catalysts {
    /// A machine and, when it has them, its four factories.
    Machine(MachineKind, Option<FactoryType>),
    /// Blocks by id.
    Blocks(&'static [&'static str]),
}

struct Spec {
    /// The category's id, Mekanism's own (`mekanism:crushing`).
    id: &'static str,
    title: &'static str,
    recipe_type: &'static str,
    inputs: &'static [Part],
    outputs: &'static [Part],
    catalysts: Catalysts,
}

use Part::{Chance, Chemical, Energy, Fluid, Item, MaybeChemical};

/// Mekanism's categories for the machines this port has, in `MekanismJEI.registerCategories`' order.
const SPECS: &[Spec] = &[
    Spec {
        id: "mekanism:crushing",
        title: "Crusher",
        recipe_type: "mekanism:crushing",
        inputs: &[Item("input")],
        outputs: &[Item("output")],
        catalysts: Catalysts::Machine(MachineKind::Crusher, Some(FactoryType::Crushing)),
    },
    Spec {
        id: "mekanism:enriching",
        title: "Enrichment Chamber",
        recipe_type: "mekanism:enriching",
        inputs: &[Item("input")],
        outputs: &[Item("output")],
        catalysts: Catalysts::Machine(MachineKind::EnrichmentChamber, Some(FactoryType::Enriching)),
    },
    Spec {
        id: "mekanism:smelting",
        title: "Energized Smelter",
        recipe_type: "minecraft:smelting",
        inputs: &[Item("input")],
        outputs: &[Item("output")],
        catalysts: Catalysts::Machine(MachineKind::EnergizedSmelter, Some(FactoryType::Smelting)),
    },
    Spec {
        id: "mekanism:chemical_infusing",
        title: "Chemical Infuser",
        recipe_type: "mekanism:chemical_infusing",
        inputs: &[Chemical("left_input"), Chemical("right_input")],
        outputs: &[Chemical("output")],
        catalysts: Catalysts::Machine(MachineKind::ChemicalInfuser, None),
    },
    Spec {
        id: "mekanism:combining",
        title: "Combiner",
        recipe_type: "mekanism:combining",
        inputs: &[Item("main_input"), Item("extra_input")],
        outputs: &[Item("output")],
        catalysts: Catalysts::Machine(MachineKind::Combiner, Some(FactoryType::Combining)),
    },
    Spec {
        id: "mekanism:separating",
        title: "Electrolytic Separator",
        recipe_type: "mekanism:separating",
        inputs: &[Fluid("input")],
        outputs: &[
            Chemical("left_chemical_output"),
            Chemical("right_chemical_output"),
        ],
        catalysts: Catalysts::Machine(MachineKind::ElectrolyticSeparator, None),
    },
    Spec {
        id: "mekanism:washing",
        title: "Chemical Washer",
        recipe_type: "mekanism:washing",
        inputs: &[Fluid("fluid_input"), Chemical("chemical_input")],
        outputs: &[Chemical("output")],
        catalysts: Catalysts::Machine(MachineKind::ChemicalWasher, None),
    },
    Spec {
        id: "mekanism:evaporating",
        title: "Thermal Evaporation Plant",
        recipe_type: "mekanism:evaporating",
        inputs: &[Fluid("input")],
        outputs: &[Fluid("output")],
        catalysts: Catalysts::Blocks(&[
            "mekanism:thermal_evaporation_controller",
            "mekanism:thermal_evaporation_valve",
            "mekanism:thermal_evaporation_block",
        ]),
    },
    Spec {
        id: "mekanism:crystallizing",
        title: "Chemical Crystallizer",
        recipe_type: "mekanism:crystallizing",
        inputs: &[Chemical("input")],
        outputs: &[Item("output")],
        catalysts: Catalysts::Machine(MachineKind::ChemicalCrystallizer, None),
    },
    Spec {
        id: "mekanism:dissolution",
        title: "Chemical Dissolution Chamber",
        recipe_type: "mekanism:dissolution",
        inputs: &[Item("item_input"), Chemical("chemical_input")],
        outputs: &[Chemical("output")],
        catalysts: Catalysts::Machine(MachineKind::ChemicalDissolutionChamber, None),
    },
    Spec {
        id: "mekanism:compressing",
        title: "Osmium Compressor",
        recipe_type: "mekanism:compressing",
        inputs: &[Item("item_input"), Chemical("chemical_input")],
        outputs: &[Item("output")],
        catalysts: Catalysts::Machine(
            MachineKind::OsmiumCompressor,
            Some(FactoryType::Compressing),
        ),
    },
    Spec {
        id: "mekanism:purifying",
        title: "Purification Chamber",
        recipe_type: "mekanism:purifying",
        inputs: &[Item("item_input"), Chemical("chemical_input")],
        outputs: &[Item("output")],
        catalysts: Catalysts::Machine(
            MachineKind::PurificationChamber,
            Some(FactoryType::Purifying),
        ),
    },
    Spec {
        id: "mekanism:injecting",
        title: "Chemical Injection Chamber",
        recipe_type: "mekanism:injecting",
        inputs: &[Item("item_input"), Chemical("chemical_input")],
        outputs: &[Item("output")],
        catalysts: Catalysts::Machine(
            MachineKind::ChemicalInjectionChamber,
            Some(FactoryType::Injecting),
        ),
    },
    Spec {
        id: "mekanism:activating",
        title: "Solar Neutron Activator",
        recipe_type: "mekanism:activating",
        inputs: &[Chemical("input")],
        outputs: &[Chemical("output")],
        catalysts: Catalysts::Machine(MachineKind::SolarNeutronActivator, None),
    },
    Spec {
        id: "mekanism:centrifuging",
        title: "Isotopic Centrifuge",
        recipe_type: "mekanism:centrifuging",
        inputs: &[Chemical("input")],
        outputs: &[Chemical("output")],
        catalysts: Catalysts::Machine(MachineKind::IsotopicCentrifuge, None),
    },
    Spec {
        id: "mekanism:reaction",
        title: "Pressurized Reaction Chamber",
        recipe_type: "mekanism:reaction",
        inputs: &[Item("item_input"), Fluid("fluid_input"), Chemical("chemical_input")],
        outputs: &[Chance("item_output", "item_output_chance"), MaybeChemical("chemical_output")],
        catalysts: Catalysts::Machine(MachineKind::PressurizedReactionChamber, None),
    },
    Spec {
        id: "mekanism:pigment_extracting",
        title: "Pigment Extractor",
        recipe_type: "mekanism:pigment_extracting",
        inputs: &[Item("input")],
        outputs: &[Chemical("output")],
        catalysts: Catalysts::Machine(MachineKind::PigmentExtractor, None),
    },
    Spec {
        id: "mekanism:pigment_mixing",
        title: "Pigment Mixer",
        recipe_type: "mekanism:pigment_mixing",
        inputs: &[Chemical("left_input"), Chemical("right_input")],
        outputs: &[Chemical("output")],
        catalysts: Catalysts::Machine(MachineKind::PigmentMixer, None),
    },
    Spec {
        id: "mekanism:painting",
        title: "Painting Machine",
        recipe_type: "mekanism:painting",
        inputs: &[Item("item_input"), Chemical("chemical_input")],
        outputs: &[Item("output")],
        catalysts: Catalysts::Machine(MachineKind::PaintingMachine, None),
    },
    // No recipe files: one recipe for each food of the item registry (`liquification`).
    Spec {
        id: mekanism_energy::liquifier::CATEGORY,
        title: "Nutritional Liquifier",
        recipe_type: mekanism_energy::liquifier::CATEGORY,
        inputs: &[],
        outputs: &[],
        catalysts: Catalysts::Machine(MachineKind::NutritionalLiquifier, None),
    },
    Spec {
        id: "mekanism:nucleosynthesizing",
        title: "Antiprotonic Nucleosynthesizer",
        recipe_type: "mekanism:nucleosynthesizing",
        inputs: &[Item("item_input"), Chemical("chemical_input")],
        outputs: &[Item("output")],
        catalysts: Catalysts::Machine(MachineKind::AntiprotonicNucleosynthesizer, None),
    },
    Spec {
        id: "mekanism:energy_conversion",
        title: "Energy Conversion",
        recipe_type: "mekanism:energy_conversion",
        inputs: &[Item("input")],
        outputs: &[Energy("output")],
        catalysts: Catalysts::Blocks(&[
            "mekanism:basic_energy_cube",
            "mekanism:advanced_energy_cube",
            "mekanism:elite_energy_cube",
            "mekanism:ultimate_energy_cube",
        ]),
    },
    Spec {
        id: "mekanism:chemical_conversion",
        title: "Chemical Conversion",
        recipe_type: "mekanism:chemical_conversion",
        inputs: &[Item("input")],
        outputs: &[Chemical("output")],
        catalysts: Catalysts::Blocks(&[
            "mekanism:metallurgic_infuser",
            "mekanism:osmium_compressor",
            "mekanism:purification_chamber",
            "mekanism:chemical_injection_chamber",
            "mekanism:antiprotonic_nucleosynthesizer",
            "mekanism:basic_chemical_tank",
        ]),
    },
    Spec {
        id: "mekanism:oxidizing",
        title: "Chemical Oxidizer",
        recipe_type: "mekanism:oxidizing",
        inputs: &[Item("input")],
        outputs: &[Chemical("output")],
        catalysts: Catalysts::Machine(MachineKind::ChemicalOxidizer, None),
    },
    Spec {
        id: "mekanism:metallurgic_infusing",
        title: "Metallurgic Infuser",
        recipe_type: "mekanism:metallurgic_infusing",
        inputs: &[Item("item_input"), Chemical("chemical_input")],
        outputs: &[Item("output")],
        catalysts: Catalysts::Machine(MachineKind::MetallurgicInfuser, Some(FactoryType::Infusing)),
    },
    Spec {
        id: "mekanism:condensentrating",
        title: "Condensentrating",
        recipe_type: "mekanism:rotary",
        inputs: &[Chemical("chemical_input")],
        outputs: &[Fluid("fluid_output")],
        catalysts: Catalysts::Machine(MachineKind::RotaryCondensentrator, None),
    },
    Spec {
        id: "mekanism:decondensentrating",
        title: "Decondensentrating",
        recipe_type: "mekanism:rotary",
        inputs: &[Fluid("fluid_input")],
        outputs: &[Chemical("chemical_output")],
        catalysts: Catalysts::Machine(MachineKind::RotaryCondensentrator, None),
    },
    Spec {
        id: "mekanism:sawing",
        title: "Precision Sawmill",
        recipe_type: "mekanism:sawing",
        inputs: &[Item("input")],
        outputs: &[
            Item("main_output"),
            Chance("secondary_output", "secondary_chance"),
        ],
        catalysts: Catalysts::Machine(MachineKind::PrecisionSawmill, Some(FactoryType::Sawing)),
    },
];

fn stack(item: &str, count: u32) -> NativeStack {
    NativeStack {
        item: item.to_owned(),
        count: count.max(1),
        components: String::new(),
        kind: String::new(),
    }
}

fn output(stack: NativeStack, chance: f32, notes: Vec<String>) -> NativeChanceOutput {
    let count = stack.count;
    NativeChanceOutput {
        stack,
        chance,
        min: count,
        max: count,
        notes,
    }
}

fn catalysts(c: &Catalysts) -> Vec<NativeStack> {
    match c {
        Catalysts::Machine(kind, factory) => {
            let mut list = vec![stack(kind.id(), 1)];
            if let Some(factory) = factory {
                list.extend(FactoryTier::ALL.iter().map(|t| {
                    stack(
                        &format!("mekanism:{}_{}_factory", t.name(), factory.name()),
                        1,
                    )
                }));
            }
            list
        }
        Catalysts::Blocks(ids) => ids.iter().map(|id| stack(id, 1)).collect(),
    }
}

/// The ingredient type Mekanism's JEI plugin registers for its chemicals (`MekanismJEI.TYPE_CHEMICAL`).
pub(super) const CHEMICAL: &str = "mekanism:chemical";

fn amount(n: i64) -> u32 {
    n.clamp(1, i64::from(u32::MAX)) as u32
}

/// A chemical ingredient's slot: each chemical it takes (a tag's all), with its amount.
fn chemical_in(registry: &ChemicalRegistry, v: &Value) -> Option<Vec<NativeStack>> {
    let ingredient = ChemicalStackIngredient::from_json(v, registry).ok()?;
    let slot: Vec<NativeStack> = ingredient
        .ingredient
        .chemicals(registry)
        .into_iter()
        .map(|c| NativeStack::typed(CHEMICAL, &c.id, amount(ingredient.amount)))
        .collect();
    (!slot.is_empty()).then_some(slot)
}

fn chemical_out(registry: &ChemicalRegistry, v: &Value) -> Option<NativeStack> {
    let out = ChemicalStack::from_json(v, registry).ok()?;
    Some(NativeStack::typed(CHEMICAL, out.id()?, amount(out.amount())))
}

/// How the chemicals `recipes` use look: their names, icons and tints (`ChemicalStackRenderer`).
fn chemical_infos(registry: &ChemicalRegistry, recipes: &[NativeCategoryRecipe]) -> Vec<NativeIngredientInfo> {
    let mut infos: Vec<NativeIngredientInfo> = Vec::new();
    let outputs = recipes.iter().flat_map(|r| r.outputs.iter().map(|o| &o.stack));
    for s in recipes.iter().flat_map(|r| r.inputs.iter().flatten()).chain(outputs) {
        if s.kind != CHEMICAL || infos.iter().any(|i| i.id == s.item) {
            continue;
        }
        if let Some(c) = registry.get(&s.item) {
            infos.push(NativeIngredientInfo {
                kind: CHEMICAL.into(),
                id: c.id.clone(),
                name: mek_chemicals::name(c),
                sprite: mek_chemicals::gui_sprite(c),
                tint: c.tint,
            });
        }
    }
    infos
}

/// A fluid ingredient's slot: each fluid it takes, with its amount.
fn fluid_in(v: &Value) -> Option<Vec<NativeStack>> {
    let ingredient = SizedFluidIngredient::parse_flat(v).ok()?;
    let mut slot: Vec<NativeStack> = Vec::new();
    for f in ingredient.fluids() {
        let id = f.fluid().source().name();
        if !slot.iter().any(|s| s.item == id) {
            slot.push(NativeStack::fluid(id, amount(i64::from(ingredient.amount))));
        }
    }
    (!slot.is_empty()).then_some(slot)
}

fn fluid_out(v: &Value) -> Option<NativeStack> {
    let fluid = FluidKind::from_name(v.get("id")?.as_str()?)?;
    let amount = v.get("amount").and_then(Value::as_i64).unwrap_or(1000);
    Some(NativeStack::fluid(fluid.name(), self::amount(amount)))
}

fn item_slot(game: &Game<'_>, v: &Value) -> Option<Vec<NativeStack>> {
    let input = mek_machines::item_in(game, v)?;
    let mut items: Vec<&String> = input.items.iter().collect();
    items.sort();
    Some(items.into_iter().map(|i| stack(i, input.count)).collect())
}

/// One recipe of `spec` from its JSON; none when a part can't be read (an item or chemical this world lacks), as
/// Mekanism's codecs leave such a recipe out.
fn recipe(
    game: &Game<'_>,
    registry: &ChemicalRegistry,
    spec: &Spec,
    id: &str,
    j: &Value,
) -> Option<NativeCategoryRecipe> {
    let mut r = NativeCategoryRecipe::default();
    for part in spec.inputs {
        match *part {
            Item(field) => r.inputs.push(item_slot(game, &j[field])?),
            Chemical(field) => r.inputs.push(chemical_in(registry, &j[field])?),
            Fluid(field) => r.inputs.push(fluid_in(&j[field])?),
            Energy(_) | Chance(..) | MaybeChemical(_) => {}
        }
    }
    for part in spec.outputs {
        match *part {
            Item(field) => {
                let out = game.native_recipe_result(&j[field])?;
                r.outputs
                    .push(output(stack(&out.item, out.count), 1.0, Vec::new()));
            }
            Chance(field, chance) => {
                if let Some(out) = j.get(field).and_then(|v| game.native_recipe_result(v)) {
                    let chance = j
                        .get(chance)
                        .and_then(Value::as_f64)
                        .unwrap_or(1.0)
                        .clamp(0.0, 1.0) as f32;
                    r.outputs
                        .push(output(stack(&out.item, out.count), chance, Vec::new()));
                }
            }
            Chemical(field) => r.outputs.push(output(chemical_out(registry, &j[field])?, 1.0, Vec::new())),
            MaybeChemical(field) => {
                if let Some(v) = j.get(field) {
                    r.outputs.push(output(chemical_out(registry, v)?, 1.0, Vec::new()));
                }
            }
            Fluid(field) => r.outputs.push(output(fluid_out(&j[field])?, 1.0, Vec::new())),
            Energy(field) => {
                let joules = j[field]
                    .as_f64()
                    .or_else(|| j[field].as_str()?.parse().ok())?;
                r.notes.push(format!("Out: {} J", thousands(joules as u64)));
            }
        }
    }
    if j.get("per_tick_usage").and_then(Value::as_bool) == Some(true) {
        r.notes.push("The chemical is used every tick".into());
    }
    if let Some(joules) = j.get("energy_required").and_then(Value::as_u64).filter(|j| *j > 0) {
        r.notes.push(format!("{} J a tick more", thousands(joules)));
    }
    if let Some(ticks) = j.get("duration").and_then(Value::as_u64) {
        r.notes.push(format!("{ticks} ticks"));
    }
    r.notes.push(format!("Recipe: {id}"));
    Some(r)
}

fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The multiblocks' and generators' recipes Mekanism makes up for its viewer (`BoilerRecipeViewerRecipe`,
/// `SPSRecipeViewerRecipe`, `FissionRecipeViewerRecipe`): what goes in and comes out, as notes.
fn made_up(game: &Game<'_>) -> Vec<NativeRecipeCategory> {
    let note = |lines: &[&str]| NativeCategoryRecipe {
        inputs: Vec::new(),
        outputs: Vec::new(),
        notes: lines.iter().map(|l| (*l).to_owned()).collect(),
    };
    let water = vec![NativeStack::fluid("minecraft:water", 1000)];
    let sps = super::mek_sps::settings().input_per_antimatter;
    let made = [
        (
            "mekanism:boiler",
            "Thermoelectric Boiler",
            &[
                "mekanism:boiler_casing",
                "mekanism:boiler_valve",
                "mekanism:pressure_disperser",
                "mekanism:superheating_element",
            ][..],
            vec![
                NativeCategoryRecipe {
                    inputs: vec![water.clone()],
                    outputs: Vec::new(),
                    notes: vec![
                        "In: Water".into(),
                        "Out: Steam".into(),
                        "Heated by superheating elements".into(),
                    ],
                },
                NativeCategoryRecipe {
                    inputs: vec![water.clone()],
                    outputs: Vec::new(),
                    notes: vec![
                        "In: Water, Superheated Sodium".into(),
                        "Out: Steam, Sodium".into(),
                    ],
                },
            ],
        ),
        (
            "mekanism:sps",
            "Supercritical Phase Shifter",
            &[
                "mekanism:sps_casing",
                "mekanism:sps_port",
                "mekanism:supercharged_coil",
            ][..],
            vec![note(&[
                &format!("In: Polonium {sps} mB"),
                "Out: Antimatter 1 mB",
            ])],
        ),
        (
            "mekanismgenerators:fission",
            "Fission Reactor",
            &[
                "mekanismgenerators:fission_reactor_casing",
                "mekanismgenerators:fission_reactor_port",
                "mekanismgenerators:fission_reactor_logic_adapter",
                "mekanismgenerators:fission_fuel_assembly",
                "mekanismgenerators:control_rod_assembly",
            ][..],
            vec![
                NativeCategoryRecipe {
                    inputs: vec![water],
                    outputs: Vec::new(),
                    notes: vec![
                        "In: Fissile Fuel 1 mB, Water".into(),
                        "Out: Nuclear Waste 1 mB, Steam".into(),
                    ],
                },
                note(&[
                    "In: Fissile Fuel 1 mB, Sodium",
                    "Out: Nuclear Waste 1 mB, Superheated Sodium",
                ]),
            ],
        ),
    ];
    made.into_iter()
        .map(|(id, title, blocks, recipes)| NativeRecipeCategory {
            id: id.into(),
            title: title.into(),
            subtitle: "Mekanism".into(),
            icon: stack(blocks[0], 1),
            catalysts: blocks
                .iter()
                .map(|b| stack(b, 1))
                .filter(|s| exists(game, &s.item))
                .collect(),
            recipes,
            ingredients: Vec::new(),
        })
        .collect()
}

/// `RecipeViewerUtils.getLiquificationRecipes`: for each item of the registry whose default
/// stack is a food with nutrition (the engine's food table, hook points API 436), the recipe
/// `TileEntityNutritionalLiquifier.getRecipe` builds: the food in, its paste and what it leaves out.
fn liquification(game: &Game<'_>) -> Vec<NativeCategoryRecipe> {
    use mekanism_energy::liquifier;
    let Some(paste) = FluidKind::from_name(liquifier::PASTE) else {
        return Vec::new();
    };
    game.native_food_table()
        .into_iter()
        .filter_map(|food| {
            let r = liquifier::liquify(food.nutrition, food.remainder.as_deref())?;
            let mut outputs = vec![output(NativeStack::fluid(paste.name(), amount(i64::from(r.paste))), 1.0, Vec::new())];
            outputs.extend(r.remainder.as_deref().map(|rest| output(stack(rest, 1), 1.0, Vec::new())));
            Some(NativeCategoryRecipe {
                inputs: vec![vec![stack(&food.item, 1)]],
                outputs,
                notes: vec![format!("{} ticks", liquifier::TICKS), format!("Recipe: {}", liquifier::recipe_id(&food.item))],
            })
        })
        .collect()
}

/// `native_recipe_categories`: Mekanism's categories. A category whose machine block this world lacks is left out.
pub(super) fn categories(game: &Game<'_>, out: &mut Vec<NativeRecipeCategory>) {
    if !exists(game, "mekanism:crusher") {
        return;
    }
    let registry = mek_chemicals::registry(game);
    for spec in SPECS {
        let catalysts: Vec<NativeStack> = catalysts(&spec.catalysts)
            .into_iter()
            .filter(|s| exists(game, &s.item))
            .collect();
        let Some(icon) = catalysts.first().cloned() else {
            continue;
        };
        let recipes: Vec<NativeCategoryRecipe> = if spec.recipe_type == mekanism_energy::liquifier::CATEGORY {
            liquification(game)
        } else if spec.recipe_type == "minecraft:smelting" {
            game.native_cooking_recipes()
                .iter()
                .filter(|c| {
                    c.kind == "minecraft:smelting"
                        && c.result.count > 0
                        && !c.ingredient.items.is_empty()
                })
                .map(|c| {
                    let mut items: Vec<&String> = c.ingredient.items.iter().collect();
                    items.sort();
                    NativeCategoryRecipe {
                        inputs: vec![items.into_iter().map(|i| stack(i, 1)).collect()],
                        outputs: vec![output(
                            stack(&c.result.item, c.result.count),
                            1.0,
                            Vec::new(),
                        )],
                        notes: vec![format!("Recipe: minecraft:{}", c.id)],
                    }
                })
                .collect()
        } else {
            game.native_recipes_of_type(spec.recipe_type)
                .iter()
                .filter_map(|r| recipe(game, &registry, spec, &r.id, &r.json))
                .collect()
        };
        let ingredients = chemical_infos(&registry, &recipes);
        out.push(NativeRecipeCategory {
            id: spec.id.into(),
            title: spec.title.into(),
            subtitle: "Mekanism".into(),
            icon,
            catalysts,
            recipes,
            ingredients,
        });
    }
    out.extend(made_up(game));
}

/// `/mek recipes [category] [item]`: each category's recipe count, or one category's recipes (those making `item`).
pub(super) fn command(game: &mut Game<'_>, category: Option<&str>, item: Option<&str>) -> Value {
    let all = game.native_collect_recipe_categories();
    let Some(category) = category else {
        return json!({
            "categories": all.iter().map(|c| json!({ "id": c.id, "title": c.title, "recipes": c.recipes.len(), "catalysts": c.catalysts.iter().map(|s| s.item.clone()).collect::<Vec<_>>() })).collect::<Vec<_>>()
        });
    };
    let Some(c) = all.iter().find(|c| c.id == category) else {
        return json!({ "error": format!("no category {category}") });
    };
    let makes = |r: &&NativeCategoryRecipe, i: &str| r.outputs.iter().any(|o| o.stack.item == i);
    let takes = |r: &&NativeCategoryRecipe, i: &str| r.inputs.iter().flatten().any(|s| s.item == i);
    let recipes: Vec<Value> = match item {
        None => c.recipes.iter().collect::<Vec<_>>(),
        Some(i) => c.recipes.iter().filter(|r| makes(r, i)).chain(c.recipes.iter().filter(|r| !makes(r, i) && takes(r, i))).collect(),
    }
    .into_iter()
        .take(8)
        .map(|r| {
            json!({
                "inputs": r.inputs.iter().map(|slot| slot.iter().take(4).map(|s| format!("{}x{}", s.count, if s.is_item() { s.item.clone() } else { format!("{} {}", s.kind, s.item) })).collect::<Vec<_>>()).collect::<Vec<_>>(),
                "outputs": r.outputs.iter().map(|o| json!({ "item": o.stack.item, "kind": o.stack.kind, "count": o.stack.count, "chance": o.chance })).collect::<Vec<_>>(),
                "notes": r.notes,
            })
        })
        .collect();
    json!({ "id": c.id, "count": c.recipes.len(), "recipes": recipes })
}

fn exists(game: &Game<'_>, item: &str) -> bool {
    game.items().get(item).is_some()
}
