//! MA's recipe load conditions (`crafting/condition/*.java`, registered in `init/ModConditionSerializers.java`):
//! `crop_enabled` and `crop_has_material` on a crop id, `augment_enabled` on an augment id.

use crate::crop_table::CROPS;
use crate::crops::NS;
use serde_json::Value;

/// The conditions under MA's namespace.
pub const NAMES: [&str; 3] = ["crop_enabled", "crop_has_material", "augment_enabled"];

/// MA's augments (`lib/ModAugments.java`): every one is registered and none is disabled.
pub const AUGMENTS: &[&str] = &[
    "absorption_i", "luck_i", "health_boost_i", "pathing_aoe_i", "nausea_resistance", "night_vision",
    "water_breathing", "absorption_ii", "jump_boost_i", "health_boost_ii", "speed_i", "mining_aoe_i",
    "tilling_aoe_i", "pathing_aoe_ii", "blindness_resistance", "absorption_iii", "luck_ii", "fire_resistance",
    "jump_boost_ii", "step_assist", "health_boost_iii", "strength_i", "speed_ii", "haste_i",
    "no_fall_damage", "slow_falling", "mining_aoe_ii", "attack_aoe_i", "tilling_aoe_ii", "pathing_aoe_iii",
    "weakness_resistance", "slowness_resistance", "absorption_iv", "poison_resistance", "jump_boost_iii", "health_boost_iv",
    "strength_ii", "speed_iii", "haste_ii", "mining_aoe_iii", "attack_aoe_ii", "tilling_aoe_iii",
    "pathing_aoe_iv", "mining_fatigue_resistance", "hunger_resistance", "absorption_v", "luck_iii", "wither_resistance",
    "health_boost_v", "strength_iii", "haste_iii", "flight", "mining_aoe_iv", "attack_aoe_iii",
    "tilling_aoe_iv",
];

/// The path of `id` when it is in MA's namespace (`ResourceLocation.parse`: a bare path is `minecraft:`).
fn own(id: &str) -> Option<&str> {
    id.strip_prefix(NS)?.strip_prefix(':')
}

/// A crop of MA's registry: one of its own (`ModCrops`). A crop that waits for another mod (`withRequiredMods`)
/// counts as enabled, since the loaded mods are not known here; the pack's MysticalCustomization crops are read
/// with the world, after the data packs.
fn crop(id: &str) -> bool {
    own(id).is_some_and(|name| CROPS.iter().any(|row| row.name == name))
}

/// `ICondition.test` of `name` on its entry's arguments (the JSON object without `type`).
pub fn test(name: &str, args: &Value) -> Result<bool, String> {
    let arg = |key: &str| args.get(key).and_then(Value::as_str).ok_or_else(|| format!("{NS}:{name} needs a `{key}` id"));
    match name {
        // `CropEnabledCondition`: the crop is registered and enabled.
        "crop_enabled" => Ok(crop(arg("crop")?)),
        // `CropHasMaterialCondition`: its crafting material is not empty. Every crop of `ModCrops` declares one
        // (none is `LazyIngredient.EMPTY`); an item material whose item is absent is not told apart here.
        "crop_has_material" => Ok(crop(arg("crop")?)),
        // `AugmentEnabledCondition`: the augment is registered and enabled.
        "augment_enabled" => Ok(own(arg("augment")?).is_some_and(|a| AUGMENTS.contains(&a))),
        _ => Err(format!("{NS}:{name} is not a condition of Mystical Agriculture")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arg(key: &str, id: &str) -> Value {
        let mut map = serde_json::Map::new();
        map.insert(key.into(), id.into());
        Value::Object(map)
    }

    #[test]
    fn conditions_answer_by_their_ids() {
        assert_eq!(test("crop_enabled", &arg("crop", "mysticalagriculture:creeper")), Ok(true));
        assert_eq!(test("crop_enabled", &arg("crop", "mysticalagriculture:no_such_crop")), Ok(false));
        assert_eq!(test("crop_enabled", &arg("crop", "creeper")), Ok(false));
        assert_eq!(test("crop_has_material", &arg("crop", "mysticalagriculture:diamond")), Ok(true));
        assert_eq!(test("augment_enabled", &arg("augment", "mysticalagriculture:absorption_i")), Ok(true));
        assert_eq!(test("augment_enabled", &arg("augment", "mysticalagriculture:absorption_vi")), Ok(false));
        assert!(test("crop_enabled", &arg("augment", "mysticalagriculture:creeper")).is_err());
        assert!(test("seed_crafting_recipes_enabled", &Value::Null).is_err());
        assert_eq!(AUGMENTS.len(), 55);
    }
}
