//! Applied Flux 1.21-2.1.7 in the game (feature `ae2-appflux`): FE cells in
//! a grid and FE between the grid and the blocks beside its buses. Derived
//! from Applied Flux by GlodBlock (`FluxCellInventory`, `FECellHandler`,
//! `FEExternalStorageStrategy`, `FEStackExportStrategy`,
//! `FEStackImportStrategy`, `AFRegistryHandler`),
//! https://github.com/GlodBlock/ExtendedAE branch `appflux/1.21.1-neoforge`
//! commit `523c3e5a`, LGPL-3.0; see `ae2/addons/appflux/`. The key type and
//! the cells' rows are the library's (`horizon_client::ae2x::appflux`).
//!
//! A block's FE is the engine's Forge Energy (`machines/native_fe.rs`): a mod
//! block entity's `IEnergyStorage` (Mekanism's cubes), or one of the game's
//! own machines'. Applied Flux speaks `int` FE there (`AFUtil.clampLong`),
//! so the calls are narrow.
pub(in crate::game) mod accessor;
pub(in crate::game) mod generic;

use super::super::{Game, ae2_global::GridAddress, ae2_key_strategies as strategies};
use ae2_cell_core::{Action, ItemKey, KeyType};
use horizon_client::{ae2x::appflux as rows, items::ItemStack};
use horizon_core::geometry::Direction;

/// What Applied Flux keeps while the game runs.
#[derive(Default)]
pub(in crate::game) struct State {
    pub(in crate::game) accessors: accessor::State,
}

/// `FECellInventory` in a drive, chest or IO Port.
pub(in crate::game) struct FeCell(rows::FeCell);
impl super::HandlerCell for FeCell {
    fn key_type(&self) -> KeyType {
        rows::key_type()
    }
    /// `getAvailableStacks`: the cell's FE, when it holds any.
    fn available_stacks(&mut self) -> Vec<(ItemKey, u64)> {
        if self.0.stored > 0 { vec![(rows::fe_key(), self.0.stored)] } else { Vec::new() }
    }
    /// `insert` takes any flux key (`what instanceof FluxKey`) as the cell's energy.
    fn insert(&mut self, key: &ItemKey, amount: u64, action: Action) -> u64 {
        if !flux(key) {
            return 0;
        }
        self.0.insert(amount, action == Action::Modulate)
    }
    fn extract(&mut self, key: &ItemKey, amount: u64, action: Action) -> u64 {
        if !flux(key) {
            return 0;
        }
        self.0.extract(amount, action == Action::Modulate)
    }
    /// A drive's `DriveWatcher` prefers a key its cell already gives.
    fn is_preferred_storage_for(&self, key: &ItemKey) -> bool {
        flux(key) && self.0.stored > 0
    }
    fn store(&self, stack: &mut ItemStack) -> Result<(), String> {
        self.0.store(stack);
        Ok(())
    }
}
fn flux(key: &ItemKey) -> bool {
    matches!(key.item_id.as_str(), rows::FE | rows::GTEU) && key.components.is_empty()
}
fn fe(key: &ItemKey) -> bool {
    key.item_id == rows::FE && key.components.is_empty()
}
/// `IEnergyStorage` amounts are `int`.
fn clamp(amount: u64) -> i64 {
    amount.min(i32::MAX as u64) as i64
}

impl Game<'_> {
    /// `FECellHandler.getCellInventory`.
    pub(in crate::game) fn appflux_fe_cell(&self, stack: &ItemStack) -> Result<FeCell, String> {
        if stack.count != 1 {
            return Err("not one FE cell".into());
        }
        rows::fe_cell(stack).map(FeCell).ok_or_else(|| "not an FE cell".into())
    }
    /// The FE storage of the block at `target` through its side `face`.
    fn appflux_fe_info(&self, target: GridAddress, face: Direction) -> Option<super::super::native_hooks::NativeFeInfo> {
        let (dim, pos) = target.local_parts()?;
        self.fe_info_at(dim, [pos.0, pos.1, pos.2], Some(face), false)
    }
    /// `receiveEnergy` (or with `extract`, `extractEnergy`) there: what moved.
    fn appflux_fe_move(&mut self, target: GridAddress, face: Direction, amount: u64, simulate: bool, extract: bool) -> u64 {
        let Some((dim, pos)) = target.local_parts() else { return 0 };
        self.fe_move_at(dim, [pos.0, pos.1, pos.2], Some(face), clamp(amount), simulate, false, extract).max(0) as u64
    }
}

/// `FEExternalStorageStrategy.createWrapper`: a storage bus mounts a block
/// with an FE storage on that side; it lists `getEnergyStored`.
fn storage_entries(game: &Game<'_>, target: GridAddress, face: Direction) -> Option<Vec<(ItemKey, u64)>> {
    let info = game.appflux_fe_info(target, face)?;
    Some(if info.stored > 0 { vec![(rows::fe_key(), info.stored as u64)] } else { Vec::new() })
}
/// `FEStorageWrapper.insert` / `extract`: FE alone, as `int`.
fn storage_change(game: &mut Game<'_>, target: GridAddress, face: Direction, key: &ItemKey, amount: u64, insert: bool, simulate: bool) -> Result<u64, String> {
    if !fe(key) || game.appflux_fe_info(target, face).is_none() {
        return Ok(0);
    }
    Ok(game.appflux_fe_move(target, face, amount, simulate, !insert))
}

/// `FEStackExportStrategy.transfer`: what the grid gives (`poweredExtraction`,
/// simulated) and the block takes, then both for real; FE the block refuses
/// after all goes back into the grid.
fn export(game: &mut Game<'_>, cable: GridAddress, target: GridAddress, face: Direction, key: &ItemKey, limit: u64) -> Result<u64, String> {
    if !fe(key) || game.appflux_fe_info(target, face).is_none() {
        return Ok(0);
    }
    let kind = rows::key_type();
    let offered = game.ae2_grid_key_move_encoded(cable, kind, key, limit, false, true)?;
    let to_add = game.appflux_fe_move(target, face, offered, true, false);
    if to_add == 0 {
        return Ok(0);
    }
    let taken = clamp(game.ae2_grid_key_move_encoded(cable, kind, key, to_add, false, false)?) as u64;
    let added = game.appflux_fe_move(target, face, taken, false, false);
    if added < taken {
        game.ae2_grid_key_move_encoded(cable, kind, key, taken - added, true, false)?;
    }
    Ok(added)
}
/// `FEStackImportStrategy`: the FE the block gives on that side.
fn import_offered(game: &Game<'_>, target: GridAddress, face: Direction) -> Vec<(ItemKey, u64)> {
    match game.appflux_fe_info(target, face) {
        Some(info) if info.stored > 0 => vec![(rows::fe_key(), info.stored as u64)],
        _ => Vec::new(),
    }
}
/// `FEStackImportStrategy.transfer`: what the grid would take, drawn from the
/// block; FE the grid refuses after all goes back.
fn import(game: &mut Game<'_>, cable: GridAddress, target: GridAddress, face: Direction, key: &ItemKey, limit: u64) -> Result<u64, String> {
    if !fe(key) || game.appflux_fe_info(target, face).is_none() {
        return Ok(0);
    }
    let kind = rows::key_type();
    let room = game.ae2_grid_key_move_encoded(cable, kind, key, limit, true, true)?;
    let to_take = game.appflux_fe_move(target, face, room, true, true);
    if to_take == 0 {
        return Ok(0);
    }
    let taken = game.appflux_fe_move(target, face, to_take, false, true);
    let added = game.ae2_grid_key_move_encoded(cable, kind, key, taken, true, false)?;
    if added < taken {
        game.appflux_fe_move(target, face, taken - added, false, false);
    }
    Ok(added)
}

/// `AFRegistryHandler`: the external storage and export strategies, and the
/// import strategy only with `misc.enable` (`AFConfig.allowImport`, off in
/// All the Mods 10); `ContainerItemStrategy.register` of the FE items'
/// strategy (`FEContainerItemStrategy`).
pub(in crate::game) fn register_strategies() {
    horizon_client::ae2_key_types::register_container_strategy(rows::key_type(), generic::container_strategy());
    let import = rows::Config::get().import_bus.then_some(strategies::ImportStrategy { offered: import_offered, transfer: import });
    strategies::register(
        rows::key_type(),
        strategies::WorldStrategies {
            import,
            export: Some(strategies::ExportStrategy { transfer: export }),
            external_storage: Some(strategies::ExternalStorageStrategy { entries: storage_entries, change: storage_change }),
            placement: None,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::super::HandlerCell;
    use super::*;

    #[test]
    fn an_fe_cell_takes_flux_keys_alone() {
        let stack = ItemStack::new("appflux:fe_4k_cell", 1);
        let mut cell = FeCell(rows::fe_cell(&stack).unwrap());
        assert_eq!(cell.key_type(), rows::key_type());
        let stone = ItemKey { item_id: "minecraft:stone".into(), components: Vec::new() };
        assert_eq!(cell.insert(&stone, 5, Action::Modulate), 0);
        assert!(cell.available_stacks().is_empty());
        assert!(!cell.is_preferred_storage_for(&rows::fe_key()));
        assert_eq!(cell.insert(&rows::fe_key(), 1 << 40, Action::Simulate), 1 << 32);
        assert_eq!(cell.insert(&rows::fe_key(), 1_000, Action::Modulate), 1_000);
        // FluxCellInventory.insert takes any FluxKey as its own energy.
        let gteu = ItemKey { item_id: rows::GTEU.into(), components: Vec::new() };
        assert_eq!(cell.insert(&gteu, 24, Action::Modulate), 24);
        assert_eq!(cell.available_stacks(), vec![(rows::fe_key(), 1_024)]);
        assert!(cell.is_preferred_storage_for(&rows::fe_key()));
        assert_eq!(cell.extract(&rows::fe_key(), 2_000, Action::Simulate), 1_024);
        assert_eq!(cell.extract(&rows::fe_key(), 24, Action::Modulate), 24);
        let mut stack = stack;
        cell.store(&mut stack).unwrap();
        assert_eq!(rows::fe_cell(&stack).unwrap().stored, 1_000);
    }

    #[test]
    fn strategies_follow_the_import_setting() {
        register_strategies();
        let kind = rows::key_type();
        let added = strategies::addon(kind);
        assert!(added.export.is_some() && added.external_storage.is_some() && added.placement.is_none());
        // All the Mods 10's appflux-common.toml is the default: no FE import.
        assert_eq!(added.import.is_some(), rows::Config::get().import_bus);
        assert!(strategies::with_external_storage_strategy().contains(&kind));
        assert!(strategies::with_export_strategy().contains(&kind));
        assert_eq!(clamp(u64::MAX), i64::from(i32::MAX));
        assert!(fe(&rows::fe_key()) && !fe(&ItemKey { item_id: rows::GTEU.into(), components: Vec::new() }));
        assert!(horizon_client::ae2_key_types::has_container_strategy(kind));
    }
}
