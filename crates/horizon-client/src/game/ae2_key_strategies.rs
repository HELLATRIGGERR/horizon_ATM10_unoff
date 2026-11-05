//! A key type's behaviour in the world: AE2's `StackWorldBehaviors`
//! (`registerImportStrategy`, `registerExportStrategy`,
//! `registerExternalStorageStrategy`, `registerPlacementStrategy`), derived
//! from AE2 1.21.1 (LGPL-3.0-or-later).
//!
//! AE2 registers all four for items and fluids, and the port keeps those in
//! its buses, storage buses and formation planes (`ae2_buses`,
//! `ae2_fluid_grid`, `ae2_formation`). An addon registers its own for its key
//! type here, before a world loads; registering one again keeps the first, as
//! `putIfAbsent` does. Which types have a strategy decides, as in AE2, what an
//! import bus may select (`hasImportStrategyTypeFilter`) and which keys a bus
//! or plane takes (`hasExportStrategyFilter`, `hasPlacementStrategy`).
use super::{Game, ae2_global::GridAddress};
use ae2_cell_core::{ItemKey, KeyType};
use horizon_core::geometry::Direction;
use std::sync::RwLock;

/// `StackImportStrategy`: what the block on `face` of `target` offers, and
/// moving up to `limit` of a key from it into the grid of `cable` (moved).
#[derive(Clone, Copy)]
pub(super) struct ImportStrategy {
    pub offered: for<'g> fn(&Game<'g>, GridAddress, Direction) -> Vec<(ItemKey, u64)>,
    pub transfer: for<'g> fn(&mut Game<'g>, GridAddress, GridAddress, Direction, &ItemKey, u64) -> Result<u64, String>,
}
/// `StackExportStrategy`: move up to `limit` of a key from the grid of
/// `cable` into the block on `face` of `target` (moved).
#[derive(Clone, Copy)]
pub(super) struct ExportStrategy {
    pub transfer: for<'g> fn(&mut Game<'g>, GridAddress, GridAddress, Direction, &ItemKey, u64) -> Result<u64, String>,
}
/// `ExternalStorageStrategy`: a storage bus's view of the block, its stored
/// keys and a change of `amount` (insert or extract, simulated or not). The
/// port's storage buses mount items and fluids only so far; this is where an
/// addon type's will be read.
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub(super) struct ExternalStorageStrategy {
    pub entries: for<'g> fn(&Game<'g>, GridAddress, Direction) -> Option<Vec<(ItemKey, u64)>>,
    pub change: for<'g> fn(&mut Game<'g>, GridAddress, Direction, &ItemKey, u64, bool, bool) -> Result<u64, String>,
}
/// `PlacementStrategy`: a formation plane placing `amount` of a key in front
/// of it (placed). The port's planes place items and fluids only so far.
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub(super) struct PlacementStrategy {
    pub place: for<'g> fn(&mut Game<'g>, GridAddress, Direction, &ItemKey, u64, bool) -> Result<u64, String>,
}
#[derive(Clone, Copy, Default)]
pub(super) struct WorldStrategies {
    pub import: Option<ImportStrategy>,
    pub export: Option<ExportStrategy>,
    pub external_storage: Option<ExternalStorageStrategy>,
    pub placement: Option<PlacementStrategy>,
}
/// An addon type's strategies, by raw id from 2 on.
static ADDED: RwLock<Vec<WorldStrategies>> = RwLock::new(Vec::new());

/// Register `strategies` for an addon's `kind`; a slot it already has stays.
#[allow(dead_code)]
pub(super) fn register(kind: KeyType, strategies: WorldStrategies) {
    if matches!(kind, KeyType::ITEMS | KeyType::FLUIDS) {
        return;
    }
    let index = usize::from(kind.raw_id() - 2);
    let mut added = ADDED.write().unwrap_or_else(|poisoned| poisoned.into_inner());
    if added.len() <= index {
        added.resize(index + 1, WorldStrategies::default());
    }
    let slot = &mut added[index];
    slot.import = slot.import.or(strategies.import);
    slot.export = slot.export.or(strategies.export);
    slot.external_storage = slot.external_storage.or(strategies.external_storage);
    slot.placement = slot.placement.or(strategies.placement);
}
/// An addon type's strategies; AE2's two are the port's own code, never here.
pub(super) fn addon(kind: KeyType) -> WorldStrategies {
    if matches!(kind, KeyType::ITEMS | KeyType::FLUIDS) {
        return WorldStrategies::default();
    }
    ADDED.read().unwrap_or_else(|poisoned| poisoned.into_inner()).get(usize::from(kind.raw_id() - 2)).copied().unwrap_or_default()
}
fn built_in(kind: KeyType) -> bool {
    matches!(kind, KeyType::ITEMS | KeyType::FLUIDS)
}
/// `StackWorldBehaviors.withImportStrategy`, in registration order.
pub(super) fn with_import_strategy() -> Vec<KeyType> {
    KeyType::all().into_iter().filter(|kind| built_in(*kind) || addon(*kind).import.is_some()).collect()
}
/// `StackWorldBehaviors.withExportStrategy`.
#[allow(dead_code)]
pub(super) fn with_export_strategy() -> Vec<KeyType> {
    KeyType::all().into_iter().filter(|kind| built_in(*kind) || addon(*kind).export.is_some()).collect()
}
/// `StackWorldBehaviors.withPlacementStrategy`.
#[allow(dead_code)]
pub(super) fn with_placement_strategy() -> Vec<KeyType> {
    KeyType::all().into_iter().filter(|kind| built_in(*kind) || addon(*kind).placement.is_some()).collect()
}
/// The types a storage bus mounts (`createExternalStorageStrategies`).
#[allow(dead_code)]
pub(super) fn with_external_storage_strategy() -> Vec<KeyType> {
    KeyType::all().into_iter().filter(|kind| built_in(*kind) || addon(*kind).external_storage.is_some()).collect()
}
