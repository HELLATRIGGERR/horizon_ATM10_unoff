//! The fork's `Game::native_*` helper surface on upstream's model, for the fork's engine code
//! (AE2) and the mixin bundles' modules and injections (S3), which call `self.native_<name>()`
//! as on the fork's main. Upstream split the fork's one `native_hooks.rs` into per-hook files
//! and typed Host methods (`mapping.md`, "Helpers"); this module adds back what the fork's
//! callers name and upstream no longer has, without touching upstream's own helpers:
//!
//! - `renamed`: upstream has the helper under another name or shape (`mapping.md` verdict
//!   `renamed`, and the 12 renamed hooks): a thin `native_<old>` forwarder in the fork's
//!   argument shape;
//! - `compat`: no same-shape upstream method (verdict `compat`): a wrapper over upstream's
//!   analog, or the fork's body where there is none, and the 5 compat hooks;
//! - `fork`: the fork's own helpers (verdict `fork`) and the fork's engine glue around its hooks
//!   (the block entity lifecycle tellers, the entity level gate, worn items in a hit).
//!
//! Upstream's same-name helpers (verdict `same`) need nothing here: `game/` modules are
//! flattened into `game` (`horizon-game-discovery`), so a `pub(super)` method of
//! `game/modding/<file>.rs` and one of `native_hooks/shared.rs` (included into
//! `native_hooks/mod.rs`) are both `pub(in crate::game)`, and `native_hooks/base.rs`'s are
//! `pub(in crate::game)` themselves: every module a bundle adds to `game.rs` reaches them. A
//! method cannot be re-exported, and a forwarder of the same name would clash with it.
//!
//! Nothing here is a hook point a mixin bundle injects into except the compat hooks in
//! `compat.rs`, whose bodies are empty injection targets (no trait-mod dispatch: the vocabulary
//! has no such hook).
use super::*;

mod bundle_hooks;
mod compat;
mod fork;
mod renamed;

pub(in crate::game) use bundle_hooks::{heard, injected, item_heard, part_heard};

pub(in crate::game) use fork::{biome_climate, data_files, food_entry, food_of, worn_items_hurt};

use horizon_client::{
    block_entities::Pos,
    containers::{BlockEntity, ModBlockEntity},
    crafting::{Cooking, Ingredient},
    health::Hurting,
    hud::HudBatch,
    items::ItemStack,
    text::Text,
};
use horizon_core::{geometry::Direction, position::BlockPos};
use horizon_world::registry::BlockStateId;
use mod_api::{
    BlockKey, EntityRef, ItemKey, LevelRef, NativeBiome, NativeDimensionType, NativeFood,
    NativePrecipitation, NativeRecipeCategory, NativeRecipeLookup, NativeStack, PlayerRef,
};

impl Game<'_> {
    /// The level `dimension` (`minecraft:the_nether`) as a handle, in this world's epoch: what
    /// upstream's `LevelRef` helpers take where the fork's took a dimension id. None outside a
    /// world.
    pub(in crate::game) fn fork_level(&self, dimension: &str) -> Option<LevelRef> {
        let epoch = self.native_lifecycle.epoch()?;
        Some(self.level_ref(epoch, mod_api::__private::dimension_key(dimension)))
    }
    /// The level in place as a handle (the fork's helpers without a dimension read it).
    pub(in crate::game) fn fork_level_here(&self) -> Option<LevelRef> {
        let epoch = self.native_lifecycle.epoch()?;
        Some(self.level_ref(epoch, self.dim.key()))
    }
    /// The player `uuid` as upstream's handle, the host's own player included; None when they
    /// are not on.
    pub(in crate::game) fn fork_player(&self, uuid: u128) -> Option<PlayerRef> {
        self.player(uuid)
    }
}
