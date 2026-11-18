//! Blocks and items as the client knows them, below gameplay: block facts
//! (`blocks`), item stacks and definitions (`items`), their data components
//! (`item_data`), mining speed (`mining`), enchantments, cooldowns, bundles and
//! item stacks in NBT.
//! horizon-client re-exports each module under its old path
//! (`horizon_client::items`, ...).
#![forbid(unsafe_code)]

/// AE2's item component codecs (`horizon_command::ae2_components`: whether it knows an id, and
/// its normalizer), for the component text the items parse (`item_data::command_components`).
/// horizon-command depends on horizon-items, so it installs them here when AE2 starts
/// (`install_key_types`) or its commands register; the first install holds.
#[allow(clippy::type_complexity)]
pub static AE2_COMPONENTS: std::sync::OnceLock<(
    fn(&str) -> bool,
    fn(&str, horizon_core::nbt::Tag) -> std::result::Result<horizon_core::nbt::Tag, String>,
)> = std::sync::OnceLock::new();
#[cfg(feature = "ae2-lab")]
pub mod ae2_stack_identity;
pub mod banner_layer;
pub mod blocks;
pub mod bundle;
#[cfg(test)]
mod component_removal_tests;
pub mod content_namespaces;
pub mod cooldowns;
pub mod durability;
pub mod enchant;
pub mod equipment;
pub mod firework;
pub mod flowers;
pub mod item_abilities;
pub mod item_data;
pub mod item_nbt;
pub mod items;
pub mod kinetic_weapon;
#[cfg(test)]
mod loot_predicate_tests;
mod loot_tags;
pub mod mining;
pub mod native_enchantment_hooks;
pub mod primary_items;
pub mod properties;
pub mod random_sequences;
pub mod recipe_filters;
pub mod recipe_sources;
pub mod use_animation;
pub use horizon_client_core::Result;
