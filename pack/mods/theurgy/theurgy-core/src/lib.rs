//! The rules of the pack's Theurgy port, free of the engine so they can be tested alone: the recipes of the core
//! spagyrics apparatus (calcination, liquefaction, distillation, incubation, accumulation) and their ingredients, the
//! crafting clock every apparatus shares (`CraftingBehaviour`), heat from a pyromantic brazier
//! (`HeatConsumerBehaviour`, `PyromanticBrazierBlockEntity`), and the slots and tanks they keep (NeoForge's
//! `ItemStackHandler` and `FluidTank` as the apparatus use them). Translated from Theurgy 1.78.0
//! (klikli-dev/theurgy, MIT); see the bundle's README.md and LICENSE-theurgy.
pub mod crafting;
pub mod heat;
pub mod ingredient;
pub mod recipe;
pub mod storage;

pub use ingredient::{FluidIngredient, Ingredient, SizedFluidIngredient, SizedIngredient};
pub use recipe::{Accumulation, Calcination, Distillation, Incubation, Liquefaction, RecipeResult};

/// What a tag test asks: is item (or fluid) `id` in tag `tag` (without `#`).
pub type Tags<'a> = &'a dyn Fn(&str, &str) -> bool;
