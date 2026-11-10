//! Ender Drives 1.5.23 in the game (feature `ae2-enderdrives`): its Ender
//! Item Storage Cells on AE2 networks. Derived from Ender Drives by STS15
//! (`EnderDiskInventory`, `AbstractEnderDiskInventory`, `EnderDBManager`,
//! `DuplicateCellGuard`, `AbstractEnderDiskItem.appendHoverText`,
//! `UpdateFrequencyPacket`), https://github.com/sts15/EnderDrives commit
//! 264ec0a and the 1.5.23 jar's classes, MIT; see `ae2/addons/enderdrives/`.
//! The disks, their data and the storage's rules are the registry's
//! (`horizon_client::ae2x::enderdrives`).
//!
//! A disk holds nothing itself: every disk of one scope and frequency is a
//! window on one storage of the world ([`Store`]), which this module reads
//! from and writes to the world's `data/enderdrives/` with the world's saves.
//! On one network only one disk of a storage works (`DuplicateCellGuard`):
//! of the disks in AE2's Drives, the one of the highest drive priority, then
//! the least dimension, position and slot; the others sleep, FULL and drawing
//! nothing. A disk in an ME Chest, a Cell Dock or an addon's drive is never
//! bound to the guard, so it always works.
use super::super::{
    Game, ae2_global::GridAddress, ae2_visual_sync::Visual, ae2x::HandlerCell, lan::LanHost,
};
use ae2_cell_core::{Action, ItemKey};
use horizon_client::{
    ae2_world::MachineKind,
    ae2x::enderdrives::{self as rows, Config, Scope, StorageKey, Store},
    containers::BlockEntity,
    items::ItemStack,
    text::{Text, format_translation},
};
use horizon_world::nbt::Tag;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};

/// The storage and the world it is of.
#[derive(Default)]
struct Inner {
    store: Store,
    /// The world folder the store was read from; none before the first use.
    root: Option<PathBuf>,
    /// Snapshots alive ([`Snapshot`]): while any is, changes are recorded.
    snapshots: usize,
}
type Shared = Arc<Mutex<Inner>>;
fn lock(shared: &Shared) -> MutexGuard<'_, Inner> {
    shared.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Default)]
pub(in crate::game) struct State {
    shared: Shared,
    /// The disks in a Drive that sleep (`isPrimary` false) as of the last
    /// grid tick: by their drive and slot. Their idle drain and LED read it.
    sleeping: HashSet<(GridAddress, usize)>,
}

/// The save's path in the world folder.
fn save_path(root: &Path) -> PathBuf {
    root.join("data/enderdrives/horizon_enderdrives.dat")
}

/// What the storage was when a block snapshot was taken
/// (`ae2_snapshot_blocks`): restoring it puts back every change since.
pub(in crate::game) struct Snapshot {
    shared: Shared,
    mark: usize,
}
impl Snapshot {
    fn new(shared: &Shared) -> Self {
        let mut inner = lock(shared);
        if inner.snapshots == 0 {
            inner.store.record(true);
        }
        inner.snapshots += 1;
        Self { shared: shared.clone(), mark: inner.store.mark() }
    }
    pub(in crate::game) fn restore(self) {
        lock(&self.shared).store.undo_to(self.mark);
    }
}
impl Drop for Snapshot {
    fn drop(&mut self) {
        let mut inner = lock(&self.shared);
        inner.snapshots = inner.snapshots.saturating_sub(1);
        if inner.snapshots == 0 {
            inner.store.record(false);
        }
    }
}

/// `EnderDiskInventory` over a disk: the storage of its scope and frequency.
pub(in crate::game) struct EnderCell {
    shared: Shared,
    key: StorageKey,
    type_limit: i32,
    disabled: bool,
    mode: i32,
    /// The partition's item keys (`CellConfig`): none lets everything in.
    filters: Vec<ItemKey>,
}
impl EnderCell {
    /// `passesFilter`: a key the partition names, or any with no partition.
    fn passes(&self, key: &ItemKey) -> bool {
        self.filters.is_empty() || self.filters.contains(key)
    }
}
impl HandlerCell for EnderCell {
    /// `getAvailableStacks`: all the frequency holds.
    fn available_stacks(&mut self) -> Vec<(ItemKey, u64)> {
        if self.disabled {
            return Vec::new();
        }
        lock(&self.shared).store.entries(&self.key).into_iter().map(|(k, n)| (k, n as u64)).collect()
    }
    /// `insert`: none into an output-only disk or past its partition; a new
    /// type only below the type limit. A simulation takes it all.
    fn insert(&mut self, key: &ItemKey, amount: u64, action: Action) -> u64 {
        if self.disabled || self.mode == rows::OUTPUT_ONLY || amount == 0 || !self.passes(key) {
            return 0;
        }
        let amount = amount.min(i64::MAX as u64) as i64;
        let mut inner = lock(&self.shared);
        if inner.store.count(&self.key, key) == 0 && inner.store.type_count(&self.key) >= self.type_limit.max(0) as usize {
            return 0;
        }
        match action {
            Action::Simulate => amount as u64,
            Action::Modulate => inner.store.insert(&self.key, key, amount, self.type_limit) as u64,
        }
    }
    /// `extract`: none from an input-only disk; at most what is stored.
    fn extract(&mut self, key: &ItemKey, amount: u64, action: Action) -> u64 {
        if self.disabled || self.mode == rows::INPUT_ONLY {
            return 0;
        }
        let amount = amount.min(i64::MAX as u64) as i64;
        let mut inner = lock(&self.shared);
        let taken = inner.store.count(&self.key, key).min(amount);
        match action {
            Action::Simulate => taken as u64,
            Action::Modulate => inner.store.extract(&self.key, key, taken) as u64,
        }
    }
    /// `isPreferredStorageFor`: what the frequency already holds.
    fn is_preferred_storage_for(&self, key: &ItemKey) -> bool {
        !self.disabled && lock(&self.shared).store.count(&self.key, key) > 0
    }
    /// `persist`: nothing, the stack keeps no contents.
    fn store(&self, _stack: &mut ItemStack) -> Result<(), String> {
        Ok(())
    }
}

/// `UpdateFrequencyPacket`'s body: the frequency, the scope's id and the
/// transfer mode, VarInts.
fn parse_update_frequency(data: &[u8]) -> Option<(i32, i32, i32)> {
    let mut reader = horizon_protocol::buf::Reader::new(data);
    let values = (reader.varint().ok()?, reader.varint().ok()?, reader.varint().ok()?);
    (reader.remaining() == 0).then_some(values)
}
/// `UpdateFrequencyPacket.handle` on the main hand's disk: its frequency,
/// scope (`FrequencyScope.fromId`) and mode; a private disk is the sender's.
/// False when the stack is no ender item disk.
fn apply_update_frequency(stack: &mut ItemStack, frequency: i32, scope: i32, mode: i32, player: &str, config: &Config) -> bool {
    if !rows::is_disk(&stack.item) {
        return false;
    }
    let scope = Scope::from_id(scope, config);
    rows::set_frequency(stack, frequency);
    rows::set_scope(stack, scope);
    rows::set_transfer_mode(stack, mode);
    if scope == Scope::Personal {
        rows::set_owner(stack, player);
    }
    true
}

impl Game<'_> {
    /// The world's storage, read from its save on the first use in a world.
    fn enderdrives_shared(&self) -> Shared {
        let shared = self.ae2x.enderdrives.shared.clone();
        if self.remote.is_some() {
            return shared;
        }
        let mut inner = lock(&shared);
        if inner.root.as_deref() != Some(self.world_root.as_path()) {
            let path = save_path(&self.world_root);
            let known = |id: &str, kind: ae2_cell_core::KeyType| self.ae2_known_cell_key(id, kind);
            inner.store = match horizon_client::save::read_dat(&path) {
                Some(tag) => Store::from_nbt(&tag, &known).unwrap_or_else(|error| {
                    log::warn!("Ender Drives: {} does not read ({error}); the storage starts empty", path.display());
                    Store::default()
                }),
                None => Store::default(),
            };
            inner.root = Some(self.world_root.clone());
        }
        drop(inner);
        shared
    }
    pub(in crate::game) fn enderdrives_snapshot(&self) -> Snapshot {
        Snapshot::new(&self.enderdrives_shared())
    }
    /// The storage written to the world's save, when it changed.
    pub(in crate::game) fn enderdrives_save(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let shared = self.ae2x.enderdrives.shared.clone();
        let mut inner = lock(&shared);
        let Some(root) = inner.root.clone().filter(|root| *root == self.world_root) else { return };
        if !inner.store.dirty {
            return;
        }
        match inner.store.to_nbt().and_then(|tag| horizon_client::save::write_dat(&save_path(&root), tag).map_err(|e| e.to_string())) {
            Ok(()) => inner.store.dirty = false,
            Err(error) => log::warn!("Ender Drives: the storage is not saved: {error}"),
        }
    }

    /// `EnderDiskInventory.HANDLER.getCellInventory`: the disk's window on
    /// its storage, its partition checked against the game's items.
    pub(in crate::game) fn enderdrives_cell(&self, stack: &ItemStack) -> Result<EnderCell, String> {
        let config = Config::get();
        let key = StorageKey::of(stack, config).ok_or("not an ender disk")?;
        if stack.count != 1 {
            return Err("not one ender disk".into());
        }
        let known = |id: &str, kind: ae2_cell_core::KeyType| self.ae2_known_cell_key(id, kind);
        Ok(EnderCell {
            shared: self.enderdrives_shared(),
            key,
            type_limit: config.type_limit(&stack.item).unwrap_or(0),
            disabled: config.disabled(&stack.item),
            mode: rows::transfer_mode(stack),
            filters: partition(stack, &known)?,
        })
    }

    /// `DuplicateCellGuard.isPrimary` of the disk in slot `slot` of the cell
    /// host `host`, among the online cell hosts `hosts` of its grid. Only
    /// AE2's own Drives take part (`getMachines(DriveBlockEntity.class)`);
    /// a disk in another host is bound only in a drive of an addon, which
    /// then sleeps when an AE2 Drive holds the same storage.
    pub(in crate::game) fn enderdrives_primary(&self, host: GridAddress, slot: usize, stack: &ItemStack, hosts: &[GridAddress]) -> bool {
        let config = Config::get();
        let Some(key) = StorageKey::of(stack, config) else { return true };
        match self.ae2_block_entity(host) {
            Some(BlockEntity::Ae2(m)) if m.kind == MachineKind::Drive => {}
            _ => return true,
        }
        // compare: the higher priority, then the dimension's id, x, y, z and slot.
        type Rank = (std::cmp::Reverse<i32>, &'static str, (i32, i32, i32), usize);
        let mut best: Option<(Rank, GridAddress)> = None;
        for &drive in hosts {
            let Some(BlockEntity::Ae2(m)) = self.ae2_block_entity(drive) else { continue };
            if m.kind != MachineKind::Drive || m.variant.is_some() {
                continue;
            }
            let Some((dim, pos)) = self.ae2_address_parts(drive) else { continue };
            let priority = self.ae2_storage_priority(drive).unwrap_or(0);
            for (i, cell) in m.slots.iter().enumerate() {
                let Some(cell) = cell else { continue };
                if StorageKey::of(cell, config).as_ref() != Some(&key) {
                    continue;
                }
                let rank = (std::cmp::Reverse(priority), dim.id(), pos, i);
                if best.as_ref().is_none_or(|(b, _)| rank < *b) {
                    best = Some((rank, drive));
                }
            }
        }
        best.is_none_or(|((_, _, _, i), drive)| drive == host && i == slot)
    }
    /// `getIdleDrain` of a disk in slot `slot` of the Drive or ME Chest at
    /// `host`; none for any other stack.
    pub(in crate::game) fn enderdrives_idle_drain(&self, host: GridAddress, slot: usize, stack: &ItemStack) -> Option<f64> {
        let config = Config::get();
        let key = StorageKey::of(stack, config)?;
        if config.disabled(&stack.item) || self.ae2x.enderdrives.sleeping.contains(&(host, slot)) {
            return Some(0.0);
        }
        Some(rows::idle_drain(lock(&self.enderdrives_shared()).store.total(&key)))
    }
    /// `getStatus` as `CellState`'s LED number: FULL for a disk turned off or
    /// asleep, else by its frequency's types.
    fn enderdrives_state(&self, stack: &ItemStack, sleeping: bool) -> Option<u8> {
        let config = Config::get();
        let key = StorageKey::of(stack, config)?;
        if sleeping || config.disabled(&stack.item) {
            return Some(4);
        }
        let types = lock(&self.enderdrives_shared()).store.type_count(&key);
        Some(rows::cell_state(types, config.type_limit(&stack.item).unwrap_or(0)))
    }
    /// The LEDs of the disks a Drive or a cell part shows (`getCellStatus`).
    pub(in crate::game) fn enderdrives_visual(&self, v: &mut Visual) {
        if self.remote.is_some() {
            return;
        }
        let Some(address) = self.ae2_local_address((v.pos[0], v.pos[1], v.pos[2])) else { return };
        let Some(BlockEntity::Ae2(m)) = self.ae2_block_entity(address) else { return };
        if m.kind == MachineKind::Drive {
            for (slot, cell) in v.slots.iter_mut().enumerate() {
                let (Some(cell), Some(Some(stack))) = (cell.as_mut(), m.slots.get(slot)) else { continue };
                if let Some(state) = self.enderdrives_state(stack, self.ae2x.enderdrives.sleeping.contains(&(address, slot))) {
                    cell.state = state;
                }
            }
        }
        for part in &mut v.parts {
            let Some(cell) = part.cell.as_mut().filter(|c| rows::is_disk(&c.id)) else { continue };
            let Some(face) = super::super::ae2_grid::face_index(&part.face) else { continue };
            let Ok(Some(machine)) = self.ae2_part_machine(super::super::ae2_parts::PartAddress { host: address, face }) else { continue };
            if let Some(state) = machine.slots.first().and_then(Option::as_ref).and_then(|stack| self.enderdrives_state(stack, false)) {
                cell.state = state;
            }
        }
    }
    /// Once a grid tick: which disks in Drives sleep (`DuplicateCellGuard`).
    pub(in crate::game) fn tick_enderdrives(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let mut drives = Vec::new();
        for (dim, world) in self.ae2_loaded_worlds() {
            for (&p, entity) in world.block_entities.iter() {
                if let BlockEntity::Ae2(m) = entity
                    && m.kind == MachineKind::Drive
                    && m.slots.iter().flatten().any(|s| rows::is_disk(&s.item))
                    && let Some(address) = self.ae2_dimension_address(dim, p)
                {
                    drives.push(address);
                }
            }
        }
        let mut sleeping = HashSet::new();
        let mut seen = HashSet::new();
        for address in drives {
            if seen.contains(&address) {
                continue;
            }
            // An offline drive's disks draw nothing anyway.
            let Ok((hosts, _)) = self.ae2_cell_hosts_encoded(address) else { continue };
            seen.extend(hosts.iter().copied());
            for &host in &hosts {
                let Some(BlockEntity::Ae2(m)) = self.ae2_block_entity(host) else { continue };
                for (slot, stack) in m.slots.iter().enumerate() {
                    if let Some(stack) = stack
                        && rows::is_disk(&stack.item)
                        && !self.enderdrives_primary(host, slot, stack, &hosts)
                    {
                        sleeping.insert((host, slot));
                    }
                }
            }
        }
        self.ae2x.enderdrives.sleeping = sleeping;
    }

    /// `AbstractEnderDiskItem.appendHoverText`: the types used of the limit,
    /// the frequency, the mode, the scope and the partition; only that it is
    /// off when the config turns the disk off.
    pub(in crate::game) fn enderdrives_hover_lines(&self, stack: &ItemStack, translate: &dyn Fn(&str) -> Option<String>) -> Option<Vec<Text>> {
        let config = Config::get();
        let key = StorageKey::of(stack, config)?;
        let template = |k: &str| translate(k).unwrap_or_else(|| k.to_owned());
        if config.disabled(&stack.item) {
            return Some(vec![Text::legacy(&template("tooltip.enderdrives.disabled"))]);
        }
        let limit = config.type_limit(&stack.item).unwrap_or(0);
        // A client of a remote server has no storage (the mod asks the server, RequestDiskTypeCountPacket).
        let used = if self.remote.is_some() { 0 } else { lock(&self.enderdrives_shared()).store.type_count(&key).min(i32::MAX as usize) as i32 };
        let percent = if limit == 0 { 0 } else { (i64::from(used) * 100 / i64::from(limit)) as i32 };
        let usage = if used <= 0 {
            0x55FF55
        } else if limit > 0 && used >= limit {
            0xFF5555
        } else if limit > 0 && percent >= 75 {
            0xFFAA00
        } else {
            0x0000FF
        };
        let mut lines = vec![format_translation(&template("tooltip.enderdrives.types"), &[Text::colored(used.to_string(), usage), Text::colored(limit.to_string(), 0x866dfc)])];
        lines.push(format_translation(&template("tooltip.enderdrives.frequency"), &[Text::literal(rows::frequency(stack).to_string())]).with_color(0xFFFF55));
        let mode = match rows::transfer_mode(stack) {
            rows::INPUT_ONLY => "tooltip.enderdrives.mode.input",
            rows::OUTPUT_ONLY => "tooltip.enderdrives.mode.output",
            _ => "tooltip.enderdrives.mode.bidirectional",
        };
        lines.push(Text::legacy(&template(mode)));
        let unknown = || template("tooltip.enderdrives.unknown");
        lines.push(match rows::scope(stack, config) {
            Scope::Personal => {
                let own = horizon_protocol::crypto::uuid_string(self.own_uuid());
                let name = match rows::owner(stack) {
                    Some(owner) if owner == own => self.options.player_name(),
                    Some(owner) => owner,
                    None => unknown(),
                };
                format_translation(&template("tooltip.enderdrives.scope.private"), &[Text::literal(name)])
            }
            Scope::Team => format_translation(&template("tooltip.enderdrives.scope.team"), &[Text::literal(rows::team_name(stack).unwrap_or_else(unknown))]),
            Scope::Global => Text::legacy(&template("tooltip.enderdrives.scope.global")),
        });
        let known = |id: &str, kind: ae2_cell_core::KeyType| self.ae2_known_cell_key(id, kind);
        let partitioned = partition(stack, &known).map_or(0, |filters| filters.len());
        if partitioned > 0 {
            let plural = if partitioned == 1 { "" } else { "s" };
            lines.push(format_translation(&template("tooltip.enderdrives.partitioned_item"), &[Text::literal(partitioned.to_string()), Text::literal(plural)]));
        }
        Some(lines)
    }

    /// `/ae2 enderdrives ...`: `set <frequency> [global|private|team]
    /// [bidirectional|input|output]` on the main hand's disk (the frequency
    /// screen's `UpdateFrequencyPacket`, in its range), `info` of that disk,
    /// `store <scope> <frequency>` of a storage.
    pub(in crate::game) fn ae2_enderdrives_command(&mut self, words: &[&str]) -> Result<serde_json::Value, String> {
        let config = Config::get();
        match words {
            ["set", frequency, rest @ ..] => {
                let frequency: i32 = frequency.parse().map_err(|_| "bad frequency")?;
                if !(config.frequency_min..=config.frequency_max).contains(&frequency) {
                    return Err(format!("the frequency is from {} to {}", config.frequency_min, config.frequency_max));
                }
                let selected = self.selected;
                let current = self.stack_in(selected).cloned().ok_or("no ender disk in the main hand")?;
                let scope = match rest.first() {
                    Some(word) => Scope::parse(word).ok_or("not a scope")?,
                    None => rows::scope(&current, config),
                };
                let mode = match rest.get(1).copied() {
                    Some("bidirectional") => rows::BIDIRECTIONAL,
                    Some("input") => rows::INPUT_ONLY,
                    Some("output") => rows::OUTPUT_ONLY,
                    Some(_) => return Err("not a transfer mode".into()),
                    None => rows::transfer_mode(&current),
                };
                let player = horizon_protocol::crypto::uuid_string(horizon_client::lan::playerdata::ints_uuid(self.acting_uuid()));
                let mut stack = current;
                if !apply_update_frequency(&mut stack, frequency, scope.id(), mode, &player, config) {
                    return Err("no ender disk in the main hand".into());
                }
                *self.stack_in_mut(selected) = Some(stack);
                self.ae2_enderdrives_command(&["info"])
            }
            ["info"] => {
                let stack = self.stack_in(self.selected).cloned().ok_or("no ender disk in the main hand")?;
                let key = StorageKey::of(&stack, config).ok_or("no ender disk in the main hand")?;
                let mut value = self.enderdrives_storage_json(&key);
                value["item"] = stack.item.to_string().into();
                value["frequency"] = rows::frequency(&stack).into();
                value["scope"] = rows::scope(&stack, config).name().into();
                value["owner"] = rows::owner(&stack).into();
                value["mode"] = rows::transfer_mode(&stack).into();
                value["limit"] = config.type_limit(&stack.item).into();
                Ok(value)
            }
            ["store", scope, frequency] => {
                let frequency: i32 = frequency.parse().map_err(|_| "bad frequency")?;
                Ok(self.enderdrives_storage_json(&StorageKey { scope: (*scope).to_owned(), frequency }))
            }
            _ => Err("usage: enderdrives set <frequency> [scope] [mode] | info | store <scope> <frequency>".into()),
        }
    }
    fn enderdrives_storage_json(&self, key: &StorageKey) -> serde_json::Value {
        let shared = self.enderdrives_shared();
        let inner = lock(&shared);
        serde_json::json!({
            "prefix": key.scope,
            "types": inner.store.type_count(key),
            "total": inner.store.total(key),
            "items": inner.store.entries(key).into_iter().map(|(k, n)| serde_json::json!({"item": k.item_id, "count": n})).collect::<Vec<_>>(),
        })
    }

    /// A guest's `enderdrives:update_frequency`: its main hand's disk set as
    /// the packet says.
    pub(in crate::game) fn lan_enderdrives_payload(&mut self, host: &mut LanHost, conn: u32, data: &[u8]) {
        let Some((frequency, scope, mode)) = parse_update_frequency(data) else { return };
        let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) else { return };
        let guest = &mut host.hub.guests[gi];
        let player = horizon_protocol::crypto::uuid_string(guest.uuid);
        let selected = guest.selected;
        let Some(Some(stack)) = guest.inventory.get_mut(selected) else { return };
        if apply_update_frequency(stack, frequency, scope, mode, &player, Config::get()) {
            host.hub.send_inventory(gi);
        }
    }
}

/// `CellConfig.create(Set.of(items), stack)`'s keys: the item filters the
/// workbench set.
fn partition(stack: &ItemStack, known: &dyn Fn(&str, ae2_cell_core::KeyType) -> bool) -> Result<Vec<ItemKey>, String> {
    let settings = horizon_client::ae2_cell_codec::cell_settings_checked(stack, known)?;
    let Some(Tag::List(entries)) = settings.get(&"filters".into()) else { return Err("invalid ender disk filters".into()) };
    let mut filters = Vec::new();
    for entry in horizon_client::ae2_cell_codec::decode_view_filter_slots(entries)?.into_iter().flatten() {
        if let Some(item) = entry.get("item") {
            let item = horizon_client::save::item_from_nbt(item).ok_or("invalid ender disk filter")?;
            let key = horizon_client::ae2_cell_codec::key(&item)?;
            if !filters.contains(&key) {
                filters.push(key);
            }
        }
    }
    Ok(filters)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(id: &str) -> ItemKey {
        horizon_client::ae2_cell_codec::key(&ItemStack::new(id, 1)).unwrap()
    }
    fn cell(shared: &Shared, frequency: i32, limit: i32, mode: i32, filters: &[&str]) -> EnderCell {
        EnderCell {
            shared: shared.clone(),
            key: StorageKey { scope: "global".into(), frequency },
            type_limit: limit,
            disabled: false,
            mode,
            filters: filters.iter().map(|id| key(id)).collect(),
        }
    }

    #[test]
    fn two_disks_of_one_frequency_share_one_storage() {
        let shared = Shared::default();
        let (mut a, mut b, mut other) = (cell(&shared, 0, 31, 0, &[]), cell(&shared, 0, 31, 0, &[]), cell(&shared, 1, 31, 0, &[]));
        let cobble = key("minecraft:cobblestone");
        assert_eq!(a.insert(&cobble, 64, Action::Modulate), 64);
        assert_eq!(b.available_stacks(), vec![(cobble.clone(), 64)]);
        assert!(b.is_preferred_storage_for(&cobble));
        assert!(other.available_stacks().is_empty() && !other.is_preferred_storage_for(&cobble));
        assert_eq!(b.extract(&cobble, 100, Action::Simulate), 64);
        assert_eq!(b.extract(&cobble, 10, Action::Modulate), 10);
        assert_eq!(a.available_stacks(), vec![(cobble, 54)]);
    }

    #[test]
    fn modes_partitions_and_type_limits() {
        let shared = Shared::default();
        let (cobble, dirt) = (key("minecraft:cobblestone"), key("minecraft:dirt"));
        let mut one_type = cell(&shared, 0, 1, rows::BIDIRECTIONAL, &[]);
        assert_eq!(one_type.insert(&cobble, 5, Action::Modulate), 5);
        assert_eq!(one_type.insert(&dirt, 5, Action::Simulate), 0, "a second type past a limit of one");
        let mut input = cell(&shared, 0, 31, rows::INPUT_ONLY, &[]);
        assert_eq!(input.extract(&cobble, 5, Action::Simulate), 0, "an input-only disk gives nothing");
        assert_eq!(input.insert(&dirt, 2, Action::Modulate), 2);
        let mut output = cell(&shared, 0, 31, rows::OUTPUT_ONLY, &[]);
        assert_eq!(output.insert(&cobble, 1, Action::Simulate), 0, "an output-only disk takes nothing");
        assert_eq!(output.extract(&dirt, 9, Action::Modulate), 2);
        let mut partitioned = cell(&shared, 0, 31, rows::BIDIRECTIONAL, &["minecraft:dirt"]);
        assert_eq!(partitioned.insert(&cobble, 1, Action::Simulate), 0, "outside the partition");
        assert_eq!(partitioned.insert(&dirt, 1, Action::Simulate), 1);
        assert_eq!(partitioned.extract(&cobble, 1, Action::Simulate), 1, "the partition stops inserts only");
        let mut off = cell(&shared, 0, 31, rows::BIDIRECTIONAL, &[]);
        off.disabled = true;
        assert!(off.available_stacks().is_empty() && off.insert(&cobble, 1, Action::Simulate) == 0 && off.extract(&cobble, 1, Action::Simulate) == 0);
    }

    #[test]
    fn a_failed_transaction_puts_the_storage_back() {
        let shared = Shared::default();
        let mut disk = cell(&shared, 0, 31, 0, &[]);
        let cobble = key("minecraft:cobblestone");
        disk.insert(&cobble, 10, Action::Modulate);
        let outer = Snapshot::new(&shared);
        disk.insert(&cobble, 5, Action::Modulate);
        {
            let inner = Snapshot::new(&shared);
            disk.extract(&cobble, 15, Action::Modulate);
            inner.restore();
        }
        assert_eq!(disk.available_stacks(), vec![(cobble.clone(), 15)]);
        outer.restore();
        assert_eq!(disk.available_stacks(), vec![(cobble.clone(), 10)]);
        // With no snapshot alive nothing is recorded.
        assert_eq!(lock(&shared).snapshots, 0);
        disk.insert(&cobble, 1, Action::Modulate);
        assert_eq!(lock(&shared).store.mark(), 0);
        // A snapshot dropped without a restore keeps the changes.
        drop(Snapshot::new(&shared));
        assert_eq!(disk.available_stacks(), vec![(cobble, 11)]);
    }

    #[test]
    fn the_frequency_packet_sets_the_disk() {
        let config = Config::default();
        let mut data = horizon_protocol::buf::Writer::new();
        data.varint(300);
        data.varint(1);
        data.varint(2);
        let (frequency, scope, mode) = parse_update_frequency(&data.data).unwrap();
        assert_eq!((frequency, scope, mode), (300, 1, 2));
        assert_eq!(parse_update_frequency(&[1, 1]), None);
        let mut disk = ItemStack::new(rows::ENDER_DISK_1K, 1);
        assert!(apply_update_frequency(&mut disk, frequency, scope, mode, "00000001-0002-0003-0004-000000000005", &config));
        assert_eq!(StorageKey::of(&disk, &config).unwrap(), StorageKey { scope: "player_00000001-0002-0003-0004-000000000005".into(), frequency: 300 });
        assert_eq!(rows::transfer_mode(&disk), rows::OUTPUT_ONLY);
        assert!(!apply_update_frequency(&mut ItemStack::new("minecraft:stone", 1), 1, 0, 0, "", &config));
    }
}
