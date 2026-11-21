//! MA's own ingredient type, `mysticalagriculture:crop_component` (`crafting/ingredient/CropComponentIngredient.java`,
//! registered in `init/ModIngredientTypes.java`): a crop's essence, its crafting seed or its crafting material, which
//! the seed recipes name instead of an item. The crop table answers, as `initMatchingStacks` asks the crop registry.

use crate::crop_table::{Accepts, CROPS};
use crate::crops::NS;
use serde_json::Value;

/// The ingredient's `type`.
pub const KIND: &str = "mysticalagriculture:crop_component";

/// `CropComponentIngredient.ComponentType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Component {
    /// The essence of the crop's tier (`CropTier.getEssence`): the tier's, not the crop's own.
    Essence,
    /// The crafting seed of the crop's type (`CropType.getCraftingSeed`).
    Seed,
    /// The crop's crafting material (`Crop.getCraftingMaterial`).
    Material,
}

impl Component {
    /// `ComponentType.CODEC`: `StringRepresentable.fromEnum`, the lower case name.
    pub fn from_name(name: &str) -> Option<Component> {
        Some(match name {
            "essence" => Component::Essence,
            "seed" => Component::Seed,
            "material" => Component::Material,
            _ => return None,
        })
    }
}

/// An ingredient of this type: `crop` (a resource location) and `component`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CropComponent {
    /// The crop's id (`mysticalagriculture:<name>`), as the file wrote it.
    pub crop: String,
    pub component: Component,
}

/// `CropComponentIngredient.CODEC` on an ingredient's JSON: `None` when it is not of this type, otherwise the
/// ingredient or why it does not read.
pub fn read(json: &Value) -> Option<Result<CropComponent, String>> {
    let object = json.as_object()?;
    if object.get("type").and_then(Value::as_str) != Some(KIND) {
        return None;
    }
    let field = |key: &str| object.get(key).and_then(Value::as_str).ok_or_else(|| format!("{KIND} needs a `{key}`"));
    Some((|| {
        let crop = field("crop")?.to_owned();
        let name = field("component")?;
        let component = Component::from_name(name).ok_or_else(|| format!("{KIND}: no component `{name}`"))?;
        Ok(CropComponent { crop, component })
    })())
}

/// `CropComponentIngredient.initMatchingStacks`: what the ingredient takes. A crop outside MA's own table (the
/// pack's MysticalCustomization crops among them) is not known here.
pub fn resolve(ingredient: &CropComponent) -> Result<Accepts, String> {
    let name = ingredient.crop.strip_prefix(NS).and_then(|rest| rest.strip_prefix(':'));
    let row = name.and_then(|name| CROPS.iter().find(|row| row.name == name)).ok_or_else(|| format!("{KIND}: no crop {}", ingredient.crop))?;
    Ok(match ingredient.component {
        Component::Essence => Accepts::Item(row.tier.info().essence.ok_or_else(|| format!("{KIND}: the tier of {} has no essence", ingredient.crop))?),
        Component::Seed => Accepts::Item(row.kind.crafting_seed().into()),
        Component::Material => row.material.accepts(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crop_table::{Kind, Material};
    use crate::infusion::{self, Recipe};
    use serde_json::json;
    use std::collections::HashSet;

    fn of(crop: &str, component: Component) -> CropComponent {
        CropComponent { crop: format!("mysticalagriculture:{crop}"), component }
    }

    fn item(id: &str) -> Accepts {
        Accepts::Item(id.into())
    }

    #[test]
    fn reads_the_codec() {
        let json = json!({"type": KIND, "component": "material", "crop": "mysticalagriculture:iron"});
        assert_eq!(read(&json), Some(Ok(of("iron", Component::Material))));
        // Another type is not this one; a missing or unknown field does not read.
        assert_eq!(read(&json!({"item": "minecraft:stick"})), None);
        assert_eq!(read(&json!({"type": "neoforge:components"})), None);
        assert!(read(&json!({"type": KIND, "crop": "mysticalagriculture:iron"})).unwrap().is_err());
        assert!(read(&json!({"type": KIND, "component": "essence"})).unwrap().is_err());
        assert!(read(&json!({"type": KIND, "component": "dust", "crop": "mysticalagriculture:iron"})).unwrap().is_err());
    }

    #[test]
    fn essence_is_the_tiers_and_the_seed_the_types() {
        // Iron is a tier 3 resource crop: Tertium Essence and the Prosperity Seed Base.
        assert_eq!(resolve(&of("iron", Component::Essence)), Ok(item("mysticalagriculture:tertium_essence")));
        assert_eq!(resolve(&of("iron", Component::Seed)), Ok(item("mysticalagriculture:prosperity_seed_base")));
        // The elemental crops share Inferium Essence with tier 1; a tier 5 crop takes Supremium.
        assert_eq!(resolve(&of("air", Component::Essence)), Ok(item("mysticalagriculture:inferium_essence")));
        assert_eq!(resolve(&of("diamond", Component::Essence)), Ok(item("mysticalagriculture:supremium_essence")));
        // A mob crop's seed is made on the Soulium Seed Base.
        assert_eq!(resolve(&of("zombie", Component::Seed)), Ok(item("mysticalagriculture:soulium_seed_base")));
    }

    #[test]
    fn material_is_an_item_a_tag_or_a_filled_jar() {
        assert_eq!(resolve(&of("stone", Component::Material)), Ok(item("minecraft:stone")));
        assert_eq!(resolve(&of("wood", Component::Material)), Ok(Accepts::Tag("minecraft:logs".into())));
        assert_eq!(resolve(&of("air", Component::Material)), Ok(item("mysticalagriculture:air_agglomeratio")));
        assert_eq!(resolve(&of("pig", Component::Material)), Ok(item("mysticalagriculture:soul_jar")));
        assert_eq!(resolve(&of("experience", Component::Material)), Ok(item("mysticalagriculture:experience_capsule")));
    }

    #[test]
    fn unknown_crops_do_not_resolve() {
        assert!(resolve(&of("no_such_crop", Component::Essence)).is_err());
        // `ResourceLocation.parse("iron")` is `minecraft:iron`.
        let bare = CropComponent { crop: "iron".into(), component: Component::Seed };
        assert!(resolve(&bare).is_err());
        let other = CropComponent { crop: "minecraft:iron".into(), component: Component::Seed };
        assert!(resolve(&other).is_err());
    }

    #[test]
    fn every_crop_resolves_every_component() {
        for row in CROPS {
            for component in [Component::Essence, Component::Seed, Component::Material] {
                let crop = format!("{NS}:{}", row.name);
                assert!(resolve(&CropComponent { crop: crop.clone(), component }).is_ok(), "{crop} {component:?}");
            }
            // The Soul Jar is the mob crops' material, and only theirs.
            assert_eq!(row.kind == Kind::Mob, row.material == Material::SoulJar, "{}", row.name);
        }
        assert_eq!(CROPS.iter().filter(|row| row.kind == Kind::Mob).count(), 23);
        assert_eq!(CROPS.iter().filter(|row| row.material == Material::Capsule).count(), 1);
    }

    /// ATM10's `mysticalagriculture:seed/infusion/iron`, whose ingredients are all of this type: read as the altar
    /// reads a recipe, resolved here, and matched against pedestals holding what the crop table says.
    #[test]
    fn a_seed_recipe_reads_resolves_and_matches() {
        let slot = |component: &str| json!({"type": KIND, "component": component, "crop": "mysticalagriculture:iron"});
        let recipe = infusion::read(&json!({
            "type": infusion::TYPE,
            "input": slot("seed"),
            "ingredients": [slot("material"), slot("essence"), slot("material"), slot("essence"),
                            slot("material"), slot("essence"), slot("material"), slot("essence")],
            "result": {"count": 1, "id": "mysticalagriculture:iron_seeds"},
        }))
        .unwrap();
        // The runtime asks the game for a tag's items: iron's material is `c:ingots/iron` here.
        let items = |json: &Value| -> HashSet<String> {
            match resolve(&read(json).unwrap().unwrap()).unwrap() {
                Accepts::Item(id) => HashSet::from([id]),
                Accepts::Tag(tag) => {
                    assert_eq!(tag, "c:ingots/iron");
                    HashSet::from(["minecraft:iron_ingot".to_owned()])
                }
            }
        };
        let resolved = Recipe {
            id: "mysticalagriculture:seed/infusion/iron".into(),
            input: items(&recipe.input),
            ingredients: recipe.ingredients.iter().map(items).collect(),
            result: (),
            transfer_components: recipe.transfer_components,
        };
        let seed = "mysticalagriculture:prosperity_seed_base";
        let (ingot, essence) = (Some("minecraft:iron_ingot"), Some("mysticalagriculture:tertium_essence"));
        assert!(resolved.matches(seed, &[ingot, essence, ingot, essence, ingot, essence, ingot, essence]));
        assert!(resolved.matches(seed, &[essence, essence, essence, essence, ingot, ingot, ingot, ingot]));
        // Tier 2's essence does not do, nor the mob crops' seed base, nor a pedestal short.
        let prudentium = Some("mysticalagriculture:prudentium_essence");
        assert!(!resolved.matches(seed, &[ingot, prudentium, ingot, essence, ingot, essence, ingot, essence]));
        assert!(!resolved.matches("mysticalagriculture:soulium_seed_base", &[ingot, essence, ingot, essence, ingot, essence, ingot, essence]));
        assert!(!resolved.matches(seed, &[ingot, essence, ingot, essence, ingot, essence, ingot, None]));
    }

    /// Every ingredient of the carry's seed infusion recipes (`MA_CARRY`, the carry of the player's jar: not
    /// committed, so this passes without it) is of this type, names the crop the file is named for and resolves.
    #[test]
    fn the_carrys_seed_recipes_resolve() {
        let Some(carry) = std::env::var_os("MA_CARRY") else { return };
        let dir = std::path::Path::new(&carry).join("data/mysticalagriculture/recipe/seed/infusion");
        let Ok(entries) = std::fs::read_dir(&dir) else { return };
        let mut seen = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            let stem = path.file_stem().unwrap().to_str().unwrap().to_owned();
            let json: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            let recipe = infusion::read(&json).unwrap_or_else(|e| panic!("{stem}: {e}"));
            for ingredient in std::iter::once(&recipe.input).chain(&recipe.ingredients) {
                let read = read(ingredient).unwrap_or_else(|| panic!("{stem}: not a crop component: {ingredient}")).unwrap();
                assert_eq!(read.crop, format!("{NS}:{stem}"));
                assert!(resolve(&read).is_ok(), "{stem}");
            }
            let seeds = recipe.result.get("id").and_then(Value::as_str);
            assert_eq!(seeds, Some(format!("{NS}:{stem}_seeds").as_str()), "{stem}");
            seen += 1;
        }
        assert!(seen > 0);
    }
}
