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
use super::{
    Game,
    ae2_ext::{self, Ae2Ext, Ae2KeyStrategies, KeyStrategyKinds},
    ae2_global::GridAddress,
};
use ae2_cell_core::{ItemKey, KeyType};
use horizon_core::geometry::Direction;
use std::{rc::Rc, sync::RwLock};

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
/// keys and a change of `amount` (insert, then simulate). `entries` is none
/// where the block has no storage of the type on that side: no mount. The
/// target is the block the bus faces, `face` its side towards the bus.
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
/// The built-in addons' strategies, registered on the first read.
static ADDONS: std::sync::Once = std::sync::Once::new();

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
/// An addon type's strategies; AE2's two are the port's own code, never here. A type an addon
/// crate above registered (`ae2_ext::add_key_strategies`) gets forwarders to its owner's
/// `Ae2KeyStrategies` for what the game's own addons left empty.
pub(super) fn addon(kind: KeyType) -> WorldStrategies {
    if matches!(kind, KeyType::ITEMS | KeyType::FLUIDS) {
        return WorldStrategies::default();
    }
    ADDONS.call_once(super::ae2x::register_key_strategies);
    let own = ADDED.read().unwrap_or_else(|poisoned| poisoned.into_inner()).get(usize::from(kind.raw_id() - 2)).copied().unwrap_or_default();
    let Some((_, kinds)) = ae2_ext::key_strategies(kind) else { return own };
    let forwarded = forwarders(kind, kinds);
    WorldStrategies {
        import: own.import.or(forwarded.import),
        export: own.export.or(forwarded.export),
        external_storage: own.external_storage.or(forwarded.external_storage),
        placement: own.placement.or(forwarded.placement),
    }
}

// ---- Forwarders to an addon crate's strategies -------------------------------------------
//
// The strategies are plain fn pointers, which carry no type: one set per raw id (from 2, the
// first addon type, to 15) finds the type's owner with `ae2_ext::key_strategies` and calls its
// `Ae2KeyStrategies` with the game as the host, as `Ae2Host::call_addon` reaches an addon. The
// trait's `at` is the block the bus faces (the strategies' `target`), its `target`/`source` the
// bus's `cable`, its `face` the side's index.

/// The forwarders of the type of raw id `kind`, for the strategies `kinds` says it has.
fn forwarders(kind: KeyType, kinds: KeyStrategyKinds) -> WorldStrategies {
    match kind.raw_id() {
        2 => forwarders_of::<2>(kinds),
        3 => forwarders_of::<3>(kinds),
        4 => forwarders_of::<4>(kinds),
        5 => forwarders_of::<5>(kinds),
        6 => forwarders_of::<6>(kinds),
        7 => forwarders_of::<7>(kinds),
        8 => forwarders_of::<8>(kinds),
        9 => forwarders_of::<9>(kinds),
        10 => forwarders_of::<10>(kinds),
        11 => forwarders_of::<11>(kinds),
        12 => forwarders_of::<12>(kinds),
        13 => forwarders_of::<13>(kinds),
        14 => forwarders_of::<14>(kinds),
        15 => forwarders_of::<15>(kinds),
        _ => WorldStrategies::default(),
    }
}
fn forwarders_of<const RAW: u8>(kinds: KeyStrategyKinds) -> WorldStrategies {
    WorldStrategies {
        import: kinds.import.then_some(ImportStrategy { offered: forward_offered::<RAW>, transfer: forward_import::<RAW> }),
        export: kinds.export.then_some(ExportStrategy { transfer: forward_export::<RAW> }),
        external_storage: kinds.external_storage.then_some(ExternalStorageStrategy { entries: forward_entries::<RAW>, change: forward_change::<RAW> }),
        placement: kinds.placement.then_some(PlacementStrategy { place: forward_place::<RAW> }),
    }
}
/// `call` on the strategies of the owner of the type of raw id `RAW`, in the game's extension
/// `ext`; none when the type, the extension or the owner is missing.
fn with_owner<const RAW: u8, R>(ext: Option<Rc<dyn Ae2Ext>>, call: impl FnOnce(&dyn Ae2KeyStrategies) -> R) -> Option<R> {
    let (owner, _) = ae2_ext::key_strategies(KeyType::from_raw_id(RAW)?)?;
    let ext = ext?;
    let strategies = ext.addon(owner)?.key_strategies()?;
    Some(call(strategies))
}
fn forward_offered<const RAW: u8>(game: &Game<'_>, target: GridAddress, face: Direction) -> Vec<(ItemKey, u64)> {
    with_owner::<RAW, _>(game.ae2_ext.get(), |s| s.import_offered(game, target, face as u8)).unwrap_or_default()
}
fn forward_import<const RAW: u8>(game: &mut Game<'_>, cable: GridAddress, target: GridAddress, face: Direction, key: &ItemKey, limit: u64) -> Result<u64, String> {
    with_owner::<RAW, _>(game.ae2_ext.get(), |s| s.import(game, target, cable, face as u8, key, limit)).unwrap_or(Ok(0))
}
fn forward_export<const RAW: u8>(game: &mut Game<'_>, cable: GridAddress, target: GridAddress, face: Direction, key: &ItemKey, limit: u64) -> Result<u64, String> {
    with_owner::<RAW, _>(game.ae2_ext.get(), |s| s.export(game, target, cable, face as u8, key, limit)).unwrap_or(Ok(0))
}
fn forward_entries<const RAW: u8>(game: &Game<'_>, target: GridAddress, face: Direction) -> Option<Vec<(ItemKey, u64)>> {
    with_owner::<RAW, _>(game.ae2_ext.get(), |s| s.storage_entries(game, target, face as u8)).flatten()
}
#[allow(clippy::too_many_arguments)]
fn forward_change<const RAW: u8>(game: &mut Game<'_>, target: GridAddress, face: Direction, key: &ItemKey, amount: u64, insert: bool, simulate: bool) -> Result<u64, String> {
    with_owner::<RAW, _>(game.ae2_ext.get(), |s| s.storage_change(game, target, face as u8, key, amount, insert, simulate)).unwrap_or(Ok(0))
}
fn forward_place<const RAW: u8>(game: &mut Game<'_>, target: GridAddress, face: Direction, key: &ItemKey, amount: u64, simulate: bool) -> Result<u64, String> {
    with_owner::<RAW, _>(game.ae2_ext.get(), |s| s.place(game, target, face as u8, key, amount, simulate)).unwrap_or(Ok(0))
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
