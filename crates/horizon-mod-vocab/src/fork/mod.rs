//! The ATM10 fork's own hook and helper types, beside upstream's: what the fork's hooks
//! (`hooks/fe_storage.rs` and the others without a `// catalog-order`) take and what the fork's
//! `Game::native_*` helpers (`game/modding/fork_compat.rs`) give. Kept under the names the fork's
//! bundles and AE2 code use (`mod_api::NativeFeInfo`), re-exported at the crate root.

/// Forge Energy, fluid and chemical handlers on a mod block's block entity, by side.
pub mod capabilities;
/// Recipe viewers: mods' own recipe categories, lookups, transfers and the item panel's grid.
pub mod recipe_viewer;
/// What a mod's machine reads of the world: food, a biome's climate, the dimension type.
pub mod world_reads;
