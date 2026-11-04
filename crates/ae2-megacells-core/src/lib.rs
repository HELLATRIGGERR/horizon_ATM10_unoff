//! MEGA Cells' bulk storage without the game: the Bulk Item Cell's inventory
//! (`BulkCellInventory`), the compression chains of `CompressionService`
//! and `CompressionChain`, and the patterns a Decompression Module offers
//! (`DecompressionPattern`).
//!
//! Derived from MEGA Cells 4.11.0 by 90 (github.com/62832/MEGACells, tag
//! `4.11.0`, LGPL-3.0): `item/cell/BulkCellInventory`, `misc/CompressionChain`,
//! `misc/CompressionService`, `misc/CompressionOverride`. Keys are AE2 item
//! keys (`ae2_cell_core::ItemKey`); the game supplies the recipes and the
//! `megacells:compression_overrides` data map.
pub mod big;
pub mod bulk;
pub mod compression;

pub use big::Big;
pub use bulk::{BulkCell, BulkData, CellState, IDLE_DRAIN};
pub use compression::{Chains, CompressionChain, CraftingRecipe, OverrideValue, Pattern, Variant, STACK_LIMIT, load_recipes};
