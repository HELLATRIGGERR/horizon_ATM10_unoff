//! The grid's storage of one key type (`NetworkStorage`, which keeps a
//! storage per `AEKeyTypes.getAll`), derived from AE2 1.21.1 `NetworkStorage`
//! and `StorageHelper` (LGPL-3.0-or-later).
//!
//! Items and fluids keep their own paths (`ae2_grid`, `ae2_fluid_grid`):
//! cells, storage buses, formation planes, crafting receipts. A key type an
//! addon registers reaches the grid's cells of that type (basic cells and an
//! addon's own handler's), the creative and fixed cells that name its keys,
//! and storage buses through its `ExternalStorageStrategy`; it has no
//! formation (`ae2_key_strategies`).
use super::Game;
use super::ae2_buses::StorageMount;
use super::ae2_global::GridAddress;
use super::ae2_key_strategies::ExternalStorageStrategy;
use ae2_machines_core::Key;
use ae2_cell_core::{ItemKey, KeyType, TypedKey};
use std::collections::{BTreeMap, BTreeSet};
type Pos = (i32, i32, i32);

impl Game<'_> {
    /// `getAvailableStacks` of `kind`: what the terminal lists of it.
    pub(super) fn ae2_grid_keys_encoded(&self, pos: GridAddress, kind: KeyType) -> Result<Vec<(ItemKey, u64)>, String> {
        match kind {
            KeyType::ITEMS => self.ae2_grid_inventory_encoded(pos),
            KeyType::FLUIDS => self.ae2_grid_fluids_encoded(pos),
            _ => self.ae2_grid_typed_keys_encoded(pos, kind),
        }
    }
    pub(super) fn ae2_grid_keys(&self, pos: Pos, kind: KeyType) -> Result<Vec<(ItemKey, u64)>, String> {
        self.ae2_grid_keys_encoded(self.ae2_local_address(pos).ok_or("invalid AE2 anchor")?, kind)
    }
    /// Every key type's stored keys, in registration order.
    pub(super) fn ae2_grid_all_keys(&self, pos: Pos) -> Result<Vec<(TypedKey, u64)>, String> {
        let mut out = Vec::new();
        for kind in KeyType::all() {
            out.extend(self.ae2_grid_keys(pos, kind)?.into_iter().map(|(key, amount)| (TypedKey::new(kind, key), amount)));
        }
        Ok(out)
    }
    fn ae2_grid_typed_keys_encoded(&self, pos: GridAddress, kind: KeyType) -> Result<Vec<(ItemKey, u64)>, String> {
        let mut totals = BTreeMap::<ItemKey, u64>::new();
        for stack in &self.ae2_grid_cells_encoded(pos)? {
            if let Ok(keys) = self.ae2_creative_cell_keys(stack) {
                for typed in keys.into_iter().filter(|typed| typed.key_type == kind) {
                    let total = totals.entry(typed.key).or_default();
                    *total = total.checked_add(horizon_client::ae2_cell_codec::creative_cell_amount(stack)).ok_or("network amount overflow")?;
                }
            }
            if let Some(Ok(mut cell)) = self.ae2x_handler_cell(stack) {
                // A cell of an addon's own handler of this type (Applied Flux's FE cells).
                if cell.key_type() == kind {
                    for (key, amount) in cell.available_stacks().into_iter().filter(|(_, amount)| *amount > 0) {
                        let total = totals.entry(key).or_default();
                        *total = total.checked_add(amount).ok_or("network amount overflow")?;
                    }
                }
                continue;
            }
            let Ok(cell) = self.ae2_read_cell(stack) else { continue };
            for (key, amount) in cell.entries_for(kind) {
                let total = totals.entry(key.clone()).or_default();
                *total = total.checked_add(*amount).ok_or("network amount overflow")?;
            }
        }
        // Storage buses on blocks the type's `ExternalStorageStrategy` reads.
        if let Some(strategy) = super::ae2_key_strategies::addon(kind).external_storage {
            for mount in self.ae2_typed_storage_mounts_encoded(pos, kind) {
                for (key, amount) in (strategy.entries)(self, mount.target, mount.face).unwrap_or_default() {
                    if amount > 0 && mount.visible(&bus_key(kind, &key)) {
                        let total = totals.entry(key).or_default();
                        *total = total.checked_add(amount).ok_or("network amount overflow")?;
                    }
                }
            }
        }
        Ok(totals.into_iter().collect())
    }
    /// `StorageHelper.poweredInsert` / `poweredExtraction` of `amount` of a
    /// key of `kind`: the amount moved. A simulation changes nothing.
    pub(super) fn ae2_grid_key_move(&mut self, pos: Pos, kind: KeyType, key: &ItemKey, amount: u64, insert: bool, simulate: bool) -> Result<u64, String> {
        let address = self.ae2_local_address(pos).ok_or("invalid AE2 anchor")?;
        self.ae2_grid_key_move_encoded(address, kind, key, amount, insert, simulate)
    }
    pub(super) fn ae2_grid_key_move_encoded(&mut self, pos: GridAddress, kind: KeyType, key: &ItemKey, amount: u64, insert: bool, simulate: bool) -> Result<u64, String> {
        match kind {
            KeyType::FLUIDS if insert => self.ae2_grid_fluid_insert_encoded(pos, key, amount, simulate),
            KeyType::FLUIDS => self.ae2_grid_fluid_extract_encoded(pos, key, amount, simulate),
            KeyType::ITEMS => {
                let mut stack = super::ae2_lab::prototype(key)?;
                stack.count = amount.min(u64::from(u32::MAX)) as u32;
                let limit = u64::from(stack.count);
                if insert {
                    if simulate { self.ae2_grid_insert_simulate_encoded(pos, &stack, limit) } else { self.ae2_grid_insert_encoded(pos, &stack, limit) }
                } else {
                    let before = simulate.then(|| self.ae2_snapshot_blocks());
                    let moved = self.ae2_grid_extract_encoded(pos, key, limit).map(|stack| stack.map_or(0, |stack| u64::from(stack.count)));
                    if let Some(before) = before {
                        self.ae2_restore_blocks(before);
                    }
                    moved
                }
            }
            _ => self.ae2_grid_typed_move(pos, kind, key, amount, insert, simulate),
        }
    }
    /// `NetworkStorage` for an addon's type, its cells and storage buses: by
    /// priority, highest first to insert and lowest first to extract, the
    /// preferred storage (a bus's partition or what it gives, a drive's
    /// cells that hold the key) before the others, buses before cells; one
    /// AE pays for `getAmountPerOperation` of the key (`StorageHelper`). A
    /// bus moves at once, the cells when the whole move is done.
    fn ae2_grid_typed_move(&mut self, pos: GridAddress, kind: KeyType, key: &ItemKey, amount: u64, insert: bool, simulate: bool) -> Result<u64, String> {
        let cells = self.ae2_cell_mounts_encoded(pos)?;
        let buses = self.ae2_typed_storage_mounts_encoded(pos, kind);
        let strategy = super::ae2_key_strategies::addon(kind).external_storage;
        let factor = kind.amount_per_operation().max(1) as f64;
        let energy = self.ae2_take_power_encoded(pos, amount as f64 / factor, true);
        let mut left = amount.min((energy * factor + 0.9).floor() as u64);
        let wanted = left;
        let priorities: BTreeSet<i32> = cells.iter().map(|mount| mount.priority).chain(buses.iter().map(|mount| mount.priority)).collect();
        let priorities: Vec<i32> = if insert { priorities.into_iter().rev().collect() } else { priorities.into_iter().collect() };
        let marker = bus_key(kind, key);
        let mut changes = Vec::new();
        for priority in priorities {
            let mut late_buses = Vec::new();
            if let Some(strategy) = strategy {
                for mount in buses.iter().filter(|mount| mount.priority == priority) {
                    if left == 0 {
                        break;
                    }
                    if insert && !self.ae2_typed_bus_preferred(mount, strategy, key, &marker)? {
                        late_buses.push(mount);
                        continue;
                    }
                    left -= self.ae2_typed_bus_change(mount, strategy, key, &marker, left, insert, simulate)?;
                }
            }
            let mut late = Vec::new();
            for mount in cells.iter().filter(|mount| mount.priority == priority) {
                if left == 0 {
                    break;
                }
                if insert && !self.ae2_cell_preferred(mount, key, kind)? {
                    late.push(mount);
                    continue;
                }
                let (moved, updated) = self.ae2_cell_storage_change(mount, key, left, kind, insert, simulate)?;
                left -= moved;
                if let Some(updated) = updated {
                    changes.push((mount.clone(), updated));
                }
            }
            if let Some(strategy) = strategy {
                for mount in late_buses {
                    if left == 0 {
                        break;
                    }
                    left -= self.ae2_typed_bus_change(mount, strategy, key, &marker, left, insert, simulate)?;
                }
            }
            for mount in late {
                if left == 0 {
                    break;
                }
                let (moved, updated) = self.ae2_cell_storage_change(mount, key, left, kind, insert, simulate)?;
                left -= moved;
                if let Some(updated) = updated {
                    changes.push((mount.clone(), updated));
                }
            }
            if left == 0 {
                break;
            }
        }
        let moved = wanted - left;
        if !simulate && moved > 0 {
            self.ae2_take_power_encoded(pos, moved as f64 / factor, false);
            for (mount, stack) in changes {
                self.ae2_store_mounted_cell(&mount, stack);
            }
        }
        Ok(moved)
    }
    /// `MEInventoryHandler.isPreferredStorageFor` of a storage bus: its
    /// partition allows the key, or the block would give some of it.
    fn ae2_typed_bus_preferred(&mut self, mount: &StorageMount, strategy: ExternalStorageStrategy, key: &ItemKey, marker: &Key) -> Result<bool, String> {
        let mut delegate = mount.clone();
        delegate.access = ae2_machines_core::Access::ReadWrite;
        if !mount.inverted && !mount.filter.is_empty() && delegate.allows(marker, false) {
            return Ok(true);
        }
        Ok((strategy.change)(self, mount.target, mount.face, key, 1, false, true)? > 0)
    }
    /// A storage bus's insert or extract through the type's strategy, within
    /// its access and filter; with a Void Card an insert takes it all.
    #[allow(clippy::too_many_arguments)]
    fn ae2_typed_bus_change(&mut self, mount: &StorageMount, strategy: ExternalStorageStrategy, key: &ItemKey, marker: &Key, amount: u64, insert: bool, simulate: bool) -> Result<u64, String> {
        if !mount.allows(marker, !insert) {
            return Ok(0);
        }
        let moved = (strategy.change)(self, mount.target, mount.face, key, amount, insert, simulate)?.min(amount);
        Ok(if insert && mount.void_overflow { amount } else { moved })
    }
}

/// How a bus names a key of an addon's type (`addon_marker`).
fn bus_key(kind: KeyType, key: &ItemKey) -> Key {
    Key::from(&horizon_client::ae2_key_types::addon_marker(kind, key))
}
