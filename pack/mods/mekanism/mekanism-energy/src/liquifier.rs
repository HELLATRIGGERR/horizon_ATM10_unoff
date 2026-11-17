//! Mekanism's Nutritional Liquifier (MIT, from Mekanism 10.7.19, Copyright (c) 2017-2025
//! Aidan C. Brady; `TileEntityNutritionalLiquifier`, `NutritionalLiquifierIRecipe`): a food
//! into nutritional paste. It has no recipe files: `getRecipe` builds one from the food
//! in its slot (`ItemStack.getFoodProperties(null)`), one food in, `nutrition * 50` mB of
//! `mekanism:nutritional_paste` out into its 10 000 mB tank and what eating the food
//! leaves (`FoodProperties.usingConvertsTo`, a stew's bowl) into its output slot, over
//! `crate::machine::process` as a progress machine of 100 ticks. Its recipe viewer lists
//! one recipe for each food of the item registry (`RecipeViewerUtils.getLiquificationRecipes`).

/// `TileEntityNutritionalLiquifier`'s `TileEntityProgressMachine` base ticks.
pub const TICKS: u32 = 100;
/// `BasicFluidTank.output(10_000)`: its paste tank, in mB.
pub const TANK: i32 = 10_000;
/// `MekanismFluids.NUTRITIONAL_PASTE`.
pub const PASTE: &str = "mekanism:nutritional_paste";
/// mB of paste a point of nutrition makes (`nutrition() * 50`).
pub const PASTE_PER_NUTRITION: i32 = 50;
/// The recipe viewer's category (`RecipeViewerRecipeType.NUTRITIONAL_LIQUIFICATION`, named
/// after the block).
pub const CATEGORY: &str = "mekanism:nutritional_liquifier";

/// The recipe `TileEntityNutritionalLiquifier.getRecipe` builds for a food.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Liquifying {
    /// mB of nutritional paste one food makes.
    pub paste: i32,
    /// The item eating it leaves, put in the output slot.
    pub remainder: Option<String>,
}

/// The recipe of a food with `nutrition` that leaves `remainder`: none without
/// nutrition (`isValidInput` and `getRecipe` want `nutrition() > 0`).
pub fn liquify(nutrition: u32, remainder: Option<&str>) -> Option<Liquifying> {
    let nutrition = i32::try_from(nutrition).ok().filter(|n| *n > 0)?;
    Some(Liquifying { paste: nutrition.saturating_mul(PASTE_PER_NUTRITION), remainder: remainder.map(str::to_owned) })
}

/// The id the recipe viewer gives a food's recipe: `synthetic(item, "liquification",
/// "mekanism")`, the item id's `:` as `_`.
pub fn recipe_id(item: &str) -> String {
    format!("mekanism:/liquification/{}", item.replace(':', "_"))
}

/// How many foods the tank still takes, `held` mB of `fluid` in it (an empty tank
/// any; another fluid none): `OutputHelper.getOutputHandler(fluidTank)`'s room.
pub fn paste_room(held: Option<(&str, i32)>, paste: i32) -> u32 {
    let held = match held {
        None => 0,
        Some((fluid, amount)) if fluid == PASTE => amount.max(0),
        Some(_) => return 0,
    };
    u32::try_from((TANK - held).max(0) / paste.max(1)).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::MachineKind as K;

    #[test]
    fn a_food_makes_fifty_millibuckets_a_point_and_its_remainder() {
        assert_eq!(liquify(4, None), Some(Liquifying { paste: 200, remainder: None }));
        let stew = liquify(6, Some("minecraft:bowl")).unwrap();
        assert_eq!((stew.paste, stew.remainder.as_deref()), (300, Some("minecraft:bowl")));
        assert_eq!(liquify(0, None), None, "no nutrition, no recipe");
        assert_eq!(recipe_id("minecraft:apple"), "mekanism:/liquification/minecraft_apple");
    }

    #[test]
    fn the_tank_takes_what_fits_of_paste_only() {
        assert_eq!(paste_room(None, 200), 50);
        assert_eq!(paste_room(Some((PASTE, 9_900)), 200), 0);
        assert_eq!(paste_room(Some((PASTE, 9_800)), 200), 1);
        assert_eq!(paste_room(Some(("minecraft:water", 10)), 200), 0);
    }

    #[test]
    fn the_liquifier_is_a_progress_machine_with_a_paste_tank() {
        let k = K::NutritionalLiquifier;
        assert_eq!(K::of_block("mekanism:nutritional_liquifier"), Some(k));
        assert_eq!((k.config_key(), k.default_usage(), k.default_storage()), ("nutritionalLiquifier", 200, 40_000));
        assert!(k.progresses() && k.ore_chain() && k.liquifies() && k.fluids_eject() && k.shows_active());
        assert!(!k.chemicals_eject() && k.default_chemicals().is_none());
        let s = k.slots();
        assert_eq!((s.input, s.output, s.output2, s.energy, s.count), (0, 1, Some(2), 4, 7));
        use crate::sides::{DataType, RelativeSide};
        let fluids = k.default_fluids().expect("a fluid config");
        for (side, data) in RelativeSide::ALL.into_iter().zip(fluids) {
            let want = if side == RelativeSide::Right { DataType::Output } else { DataType::None };
            assert_eq!(data, want, "{side:?}");
        }
    }
}
