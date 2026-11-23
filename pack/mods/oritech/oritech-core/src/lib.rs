//! The rules of the pack's Oritech port, free of the engine so they can be tested alone: Oritech's own recipe
//! format and matching (`OritechRecipe`, `OritechRecipeType`), the processing machines' table (slots, recipe
//! types, cores), the machine tick (`MachineBlockEntity`, `UpgradableMachineBlockEntity`), the energy storage
//! (`DynamicEnergyStorage`), the multiblock cores (`MultiblockMachineController`, `Geometry.rotatePosition`), the
//! settings of `oritech-common.toml` and the numbers the screen carries. Translated from Oritech 1.2.13
//! (Rearth/Oritech, CC0-1.0); see the bundle's README.md and LICENSE-oritech.
pub mod config;
pub mod energy;
pub mod machine;
pub mod multiblock;
pub mod process;
pub mod recipe;
pub mod view;
