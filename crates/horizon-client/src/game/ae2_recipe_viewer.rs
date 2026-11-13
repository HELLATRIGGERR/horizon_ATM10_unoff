//! AE2's recipe viewer categories, as `ae2jeiintegration`'s `JEIPlugin` registers them for JEI (the Inscriber, the
//! Charger, the Condenser, item transformation, the Entropy Manipulator, certus growth and P2P attunement), for any
//! viewer that reads `native_recipe_categories`, and the addons' (`Ae2x`'s `recipe_categories`). The recipes are the
//! world's tables (`native_recipes_of_type`), read as AE2's machines read them.
//!
//! A viewer's slots hold items: a fluid shows as its bucket, with the amount in a note; energy is a note in AE.
use super::Game;
use horizon_client::mod_api::{
    NativeCategoryRecipe, NativeChanceOutput, NativeRecipeCategory, NativeStack,
};
use horizon_world::{fluid::FluidKind, fluid_ingredient::FluidIngredient, fluid_stack};
use serde_json::{Value, json};

pub(in crate::game) fn stack(item: &str, count: u32) -> NativeStack {
    NativeStack {
        item: item.to_owned(),
        count: count.max(1),
        components: String::new(),
    }
}

pub(in crate::game) fn output(stack: NativeStack, chance: f32) -> NativeChanceOutput {
    let count = stack.count;
    NativeChanceOutput {
        stack,
        chance,
        min: count,
        max: count,
        notes: Vec::new(),
    }
}

/// An ingredient as the engine reads one (an id, a `#tag`, or a list of them) from the recipe files' `{"item": id}`
/// and `{"tag": tag}` objects.
fn plain_ingredient(v: &Value) -> Value {
    match v {
        Value::Object(m) => match (
            m.get("item").and_then(Value::as_str),
            m.get("tag").and_then(Value::as_str),
        ) {
            (Some(item), _) => Value::String(item.to_owned()),
            (None, Some(tag)) => Value::String(format!("#{tag}")),
            _ => v.clone(),
        },
        Value::Array(list) => Value::Array(list.iter().map(plain_ingredient).collect()),
        _ => v.clone(),
    }
}

/// One slot's items for an ingredient's JSON (vanilla's `Ingredient`), each `count` of them; none for an
/// ingredient that names nothing this world has.
pub(in crate::game) fn slot(game: &Game<'_>, v: &Value, count: u32) -> Option<Vec<NativeStack>> {
    let mut items: Vec<String> = game
        .native_recipe_ingredient(&plain_ingredient(v))?
        .items
        .into_iter()
        .collect();
    items.sort();
    Some(items.iter().map(|i| stack(i, count)).collect())
}

/// An `ItemStack.CODEC` result.
pub(in crate::game) fn result(game: &Game<'_>, v: &Value) -> Option<NativeStack> {
    let out = game.native_recipe_result(v)?;
    Some(stack(&out.item, out.count))
}

/// A fluid's display name from its id (`minecraft:water` -> `Water`).
pub(in crate::game) fn fluid_name(fluid: FluidKind) -> String {
    let path = fluid.name().rsplit(':').next().unwrap_or_default();
    path.split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A NeoForge `FluidIngredient`'s buckets and its fluids' names.
pub(in crate::game) fn fluid_slot(v: &Value) -> Option<(Vec<NativeStack>, String)> {
    let fluids = FluidIngredient::parse(v).ok()?.stacks();
    let mut buckets: Vec<NativeStack> = Vec::new();
    for f in &fluids {
        if let Some(b) = fluid_stack::filled_bucket(f.fluid())
            && !buckets.iter().any(|s| s.item == b)
        {
            buckets.push(stack(b, 1));
        }
    }
    let name = fluids.first().map(|f| fluid_name(f.fluid()))?;
    Some((buckets, name))
}

fn exists(game: &Game<'_>, item: &str) -> bool {
    game.items().get(item).is_some()
}

/// A category of `catalysts` (those this world has; the first is the icon), none when it has none of them.
pub(in crate::game) fn category(
    game: &Game<'_>,
    id: &str,
    title: &str,
    subtitle: &str,
    catalysts: &[&str],
    recipes: Vec<NativeCategoryRecipe>,
) -> Option<NativeRecipeCategory> {
    let catalysts: Vec<NativeStack> = catalysts
        .iter()
        .filter(|c| exists(game, c))
        .map(|c| stack(c, 1))
        .collect();
    Some(NativeRecipeCategory {
        id: id.into(),
        title: title.into(),
        subtitle: subtitle.into(),
        icon: catalysts.first()?.clone(),
        catalysts,
        recipes,
    })
}

fn recipes_of<'g>(game: &'g Game<'_>, kind: &str) -> impl Iterator<Item = (&'g str, &'g Value)> {
    game.native_recipes_of_type(kind)
        .iter()
        .map(|r| (r.id.as_str(), &*r.json))
}

/// `InscriberRecipeCategory`: top, middle and bottom, the presses kept unless the recipe presses them.
fn inscriber(game: &Game<'_>) -> Vec<NativeCategoryRecipe> {
    recipes_of(game, "ae2:inscriber")
        .filter_map(|(id, v)| {
            let slots = &v["ingredients"];
            let middle = slot(game, &slots["middle"], 1)?;
            let side = |key: &str| match slots.get(key) {
                Some(i) if !i.is_null() => slot(game, i, 1),
                _ => Some(Vec::new()),
            };
            let pressed = v["mode"].as_str() == Some("press");
            Some(NativeCategoryRecipe {
                inputs: vec![side("top")?, middle, side("bottom")?],
                outputs: vec![output(result(game, &v["result"])?, 1.0)],
                notes: vec![
                    if pressed {
                        "Uses up the top and bottom items"
                    } else {
                        "Keeps the presses"
                    }
                    .into(),
                    format!("Recipe: {id}"),
                ],
            })
        })
        .collect()
}

/// `ChargerCategory`: one item charged by the Charger or a crank.
fn charger(game: &Game<'_>) -> Vec<NativeCategoryRecipe> {
    recipes_of(game, "ae2:charger")
        .filter_map(|(id, v)| {
            Some(NativeCategoryRecipe {
                inputs: vec![slot(game, &v["ingredient"], 1)?],
                outputs: vec![output(result(game, &v["result"])?, 1.0)],
                notes: vec!["1,600 AE".into(), format!("Recipe: {id}")],
            })
        })
        .collect()
}

/// `CondenserCategory`: matter balls and singularities, the storage components that let it make them.
fn condenser(game: &Game<'_>) -> Vec<NativeCategoryRecipe> {
    let components = |list: &[&str]| -> Vec<NativeStack> {
        list.iter()
            .filter(|c| exists(game, c))
            .map(|c| stack(c, 1))
            .collect()
    };
    let all = components(&[
        "ae2:cell_component_1k",
        "ae2:cell_component_4k",
        "ae2:cell_component_16k",
        "ae2:cell_component_64k",
        "ae2:cell_component_256k",
    ]);
    let big = components(&["ae2:cell_component_64k", "ae2:cell_component_256k"]);
    [
        ("ae2:matter_ball", 256u32, all),
        ("ae2:singularity", 256_000, big),
    ]
    .into_iter()
    .filter(|(item, ..)| exists(game, item))
    .map(|(item, energy, cells)| NativeCategoryRecipe {
        inputs: vec![cells],
        outputs: vec![output(stack(item, 1), 1.0)],
        notes: vec![format!("{} items or mB destroyed each", thousands(energy))],
    })
    .collect()
}

/// `TransformCategory`: items thrown into a fluid, or caught in an explosion.
fn transform(game: &Game<'_>) -> Vec<NativeCategoryRecipe> {
    recipes_of(game, "ae2:transform")
        .filter_map(|(id, v)| {
            let mut inputs: Vec<Vec<NativeStack>> = Vec::new();
            for i in v["ingredients"].as_array()? {
                inputs.push(slot(game, i, 1)?);
            }
            let circumstance = &v["circumstance"];
            let how = match circumstance["type"].as_str().unwrap_or("fluid") {
                "explosion" => {
                    inputs.push(vec![stack("minecraft:tnt", 1)]);
                    "Caught in an explosion".to_owned()
                }
                _ => {
                    let tag = circumstance["tag"].as_str().unwrap_or("minecraft:water");
                    let (buckets, name) = fluid_slot(&json!({ "tag": tag })).unwrap_or_else(|| {
                        (vec![stack("minecraft:water_bucket", 1)], "Water".into())
                    });
                    inputs.push(buckets);
                    format!("Thrown into {name}")
                }
            };
            Some(NativeCategoryRecipe {
                inputs,
                outputs: vec![output(result(game, &v["result"])?, 1.0)],
                notes: vec![how, format!("Recipe: {id}")],
            })
        })
        .collect()
}

/// A block or fluid of an entropy recipe as an item: the block's own item, a fluid's bucket.
fn entropy_side(game: &Game<'_>, v: &Value) -> Option<(NativeStack, String)> {
    if let Some(id) = v["block"]["id"].as_str() {
        return exists(game, id).then(|| (stack(id, 1), id.to_owned()));
    }
    let id = v["fluid"]["id"].as_str()?;
    let fluid = FluidKind::from_name(id)?;
    Some((
        stack(fluid_stack::filled_bucket(fluid)?, 1),
        format!("{} (fluid)", fluid_name(fluid)),
    ))
}

/// `EntropyManipulatorCategory`: a block or fluid heated or cooled into another, with its drops.
fn entropy(game: &Game<'_>) -> Vec<NativeCategoryRecipe> {
    recipes_of(game, "ae2:entropy")
        .filter_map(|(id, v)| {
            let (input, _) = entropy_side(game, &v["input"])?;
            let mut outputs = Vec::new();
            if let Some((out, _)) = entropy_side(game, &v["output"]) {
                outputs.push(output(out, 1.0));
            }
            for drop in v["output"]["drops"].as_array().into_iter().flatten() {
                outputs.push(output(result(game, drop)?, 1.0));
            }
            let cool = v["mode"].as_str() == Some("cool");
            Some(NativeCategoryRecipe {
                inputs: vec![vec![input]],
                outputs,
                notes: vec![
                    if cool {
                        "Cool (sneak + use)"
                    } else {
                        "Heat (use)"
                    }
                    .into(),
                    format!("Recipe: {id}"),
                ],
            })
        })
        .collect()
}

/// `CertusGrowthCategory`: budding certus grows buds, then a cluster that breaks into crystals; budding blocks
/// lose quality as they grow (a flawless one doesn't).
fn certus_growth(game: &Game<'_>) -> Vec<NativeCategoryRecipe> {
    let buds = [
        "ae2:small_quartz_bud",
        "ae2:medium_quartz_bud",
        "ae2:large_quartz_bud",
        "ae2:quartz_cluster",
    ];
    let budding = [
        "ae2:flawless_budding_quartz",
        "ae2:flawed_budding_quartz",
        "ae2:chipped_budding_quartz",
        "ae2:damaged_budding_quartz",
    ];
    let mut recipes = vec![NativeCategoryRecipe {
        inputs: vec![
            budding
                .iter()
                .filter(|b| exists(game, b))
                .map(|b| stack(b, 1))
                .collect(),
        ],
        outputs: buds
            .iter()
            .filter(|b| exists(game, b))
            .map(|b| output(stack(b, 1), 1.0))
            .collect(),
        notes: vec!["Grows on a budding certus quartz block".into()],
    }];
    if exists(game, "ae2:quartz_cluster") && exists(game, "ae2:certus_quartz_crystal") {
        recipes.push(NativeCategoryRecipe {
            inputs: vec![vec![stack("ae2:quartz_cluster", 1)]],
            outputs: vec![NativeChanceOutput {
                stack: stack("ae2:certus_quartz_crystal", 4),
                chance: 1.0,
                min: 4,
                max: 4,
                notes: vec!["More with Fortune".into()],
            }],
            notes: vec!["Broken".into()],
        });
    }
    recipes
}

/// `AttunementCategory`: what a P2P tunnel turns into when used with an item, from the item tags
/// `ae2:p2p_attunements/<tunnel>` (items that hold energy or fluids attune too).
fn attunement(game: &Game<'_>) -> Vec<NativeCategoryRecipe> {
    let tunnels = [
        ("me_p2p_tunnel", ""),
        ("fe_p2p_tunnel", "Any item that holds energy"),
        ("redstone_p2p_tunnel", ""),
        ("fluid_p2p_tunnel", "Any item that holds fluids"),
        ("item_p2p_tunnel", ""),
        ("light_p2p_tunnel", ""),
    ];
    let tags = &game.items().item_tags;
    tunnels
        .into_iter()
        .filter_map(|(tunnel, also)| {
            let id = format!("ae2:{tunnel}");
            let mut items: Vec<&String> = tags
                .get(format!("ae2:p2p_attunements/{tunnel}").as_str())?
                .iter()
                .collect();
            items.sort();
            let items: Vec<NativeStack> = items
                .into_iter()
                .filter(|i| exists(game, i))
                .map(|i| stack(i, 1))
                .collect();
            if items.is_empty() || !exists(game, &id) {
                return None;
            }
            let mut notes = vec!["Use the item on a P2P tunnel".to_owned()];
            if !also.is_empty() {
                notes.push(also.into());
            }
            Some(NativeCategoryRecipe {
                inputs: vec![vec![stack("ae2:me_p2p_tunnel", 1)], items],
                outputs: vec![output(stack(&id, 1), 1.0)],
                notes,
            })
        })
        .collect()
}

fn thousands(n: u32) -> String {
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

impl Game<'_> {
    /// `native_recipe_categories`: AE2's categories, then its addons'.
    pub(super) fn ae2_recipe_categories(&self, out: &mut Vec<NativeRecipeCategory>) {
        if !exists(self, "ae2:inscriber") {
            return;
        }
        let ae2 = "Applied Energistics 2";
        let made = [
            category(
                self,
                "ae2:inscriber",
                "Inscriber",
                ae2,
                &["ae2:inscriber"],
                inscriber(self),
            ),
            category(
                self,
                "ae2:charger",
                "Charger",
                ae2,
                &["ae2:charger", "ae2:crank"],
                charger(self),
            ),
            category(
                self,
                "ae2:condenser",
                "Condenser",
                ae2,
                &["ae2:condenser"],
                condenser(self),
            ),
            category(
                self,
                "ae2:item_transformation",
                "Item Transformation",
                ae2,
                &["minecraft:water_bucket"],
                transform(self),
            ),
            category(
                self,
                "ae2:entropy",
                "Entropy Manipulator",
                ae2,
                &["ae2:entropy_manipulator"],
                entropy(self),
            ),
            category(
                self,
                "ae2:certus_growth",
                "Certus Quartz Growth",
                ae2,
                &["ae2:flawless_budding_quartz", "ae2:growth_accelerator"],
                certus_growth(self),
            ),
            category(
                self,
                "ae2:attunement",
                "P2P Tunnel Attunement",
                ae2,
                &["ae2:me_p2p_tunnel"],
                attunement(self),
            ),
        ];
        out.extend(made.into_iter().flatten().filter(|c| !c.recipes.is_empty()));
        self.ae2x_recipe_categories(out);
    }

    /// `/ae2 recipes [category] [item]`: every category's recipe count (all mods'), or one category's recipes
    /// (those making or taking `item`).
    pub(super) fn ae2_recipes_command(&mut self, w: &[&str]) -> Result<Value, String> {
        Ok(categories_json(
            &self.native_collect_recipe_categories(),
            w.first().copied(),
            w.get(1).copied(),
        ))
    }
}

/// The categories as JSON for the `/ae2 recipes` command: their counts, or one category's first recipes.
pub(in crate::game) fn categories_json(
    all: &[NativeRecipeCategory],
    category: Option<&str>,
    item: Option<&str>,
) -> Value {
    let Some(category) = category else {
        return json!({ "categories": all.iter().map(|c| json!({
            "id": c.id, "title": c.title, "recipes": c.recipes.len(),
            "catalysts": c.catalysts.iter().map(|s| s.item.clone()).collect::<Vec<_>>(),
        })).collect::<Vec<_>>() });
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
                "inputs": r.inputs.iter().map(|slot| slot.iter().take(4).map(|s| format!("{}x{}", s.count, s.item)).collect::<Vec<_>>()).collect::<Vec<_>>(),
                "outputs": r.outputs.iter().map(|o| json!({ "item": o.stack.item, "count": o.stack.count, "chance": o.chance })).collect::<Vec<_>>(),
                "notes": r.notes,
            })
        })
        .collect();
    json!({ "id": c.id, "count": c.recipes.len(), "recipes": recipes })
}
