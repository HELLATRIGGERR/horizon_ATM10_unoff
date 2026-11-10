//! Ender Drives 1.5.23's rows in the definitions registry, the data its
//! disks keep, the storage they share and its server config (feature
//! `ae2-enderdrives`). Derived from Ender Drives by STS15 (`ItemInit`,
//! `AbstractEnderDiskItem`, `EnderDiskItem`, `EnderDiskInventory`,
//! `AbstractEnderDiskInventory`, `EnderDBManager`, `FrequencyScope`,
//! `TransferMode`, `serverConfig`), https://github.com/sts15/EnderDrives
//! commit 264ec0a (1.5.0, the newest public 1.21.1 source) and the 1.5.23
//! jar's classes where they differ, MIT; see `ae2/addons/enderdrives/`. The
//! game's side (the cells on a network, the duplicate guard, the save) is in
//! `game/ae2x/enderdrives/`.
use super::defs::{Defs, HandlerCellDef};
use crate::{ae2_generic_stack, items::ItemStack, save};
use ae2_cell_core::{ItemKey, KeyType};
use horizon_world::nbt::{Compound, Tag};
use std::{collections::BTreeMap, sync::OnceLock};

pub const NAMESPACE: &str = "enderdrives";
/// `ItemInit`: the Ender Item Storage Cells, in `getDriveIndex` order.
pub const ENDER_DISK_1K: &str = "enderdrives:ender_disk_1k";
pub const ENDER_DISK_4K: &str = "enderdrives:ender_disk_4k";
pub const ENDER_DISK_16K: &str = "enderdrives:ender_disk_16k";
pub const ENDER_DISK_64K: &str = "enderdrives:ender_disk_64k";
pub const ENDER_DISK_256K: &str = "enderdrives:ender_disk_256k";
pub const ENDER_DISK_CREATIVE: &str = "enderdrives:ender_disk_creative";
pub const ENDER_DISKS: [&str; 6] = [ENDER_DISK_1K, ENDER_DISK_4K, ENDER_DISK_16K, ENDER_DISK_64K, ENDER_DISK_256K, ENDER_DISK_CREATIVE];

/// `AbstractEnderDiskItem`'s keys in the stack's `minecraft:custom_data`.
pub const FREQ_KEY: &str = "ender_freq";
pub const SCOPE_KEY: &str = "ender_scope";
pub const OWNER_KEY: &str = "ender_owner";
pub const TEAM_KEY: &str = "ender_team";
pub const TEAM_NAME_KEY: &str = "ender_team_name";
pub const TRANSFER_MODE_KEY: &str = "ender_transfer_mode";
const CUSTOM_DATA: &str = "minecraft:custom_data";

/// `EnderDrivesClient`: `StorageCellModels.registerModel(disk, block/drive/ender_cell)`.
pub const DRIVE_MODEL: &str = "enderdrives:block/drive/ender_cell";
/// `CellConfig.create(Set.of(items), stack)`: AE2's 63 filter slots.
pub const CONFIG_SLOTS: usize = 63;
/// `UpdateFrequencyPacket`: the frequency screen's frequency, scope id and
/// transfer mode, each a VarInt, for the main hand's disk.
pub const UPDATE_FREQUENCY: &str = "enderdrives:update_frequency";

/// `TransferMode`: what a disk lets through.
pub const BIDIRECTIONAL: i32 = 0;
pub const INPUT_ONLY: i32 = 1;
pub const OUTPUT_ONLY: i32 = 2;

/// `getDriveIndex` of an ender item disk; none for any other item.
pub fn disk_index(id: &str) -> Option<usize> {
    ENDER_DISKS.iter().position(|d| *d == id)
}
pub fn is_disk(id: &str) -> bool {
    disk_index(id).is_some()
}

/// `FrequencyScope`: whose storage a frequency is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Global,
    Personal,
    Team,
}
impl Scope {
    pub const ALL: [Scope; 3] = [Scope::Global, Scope::Personal, Scope::Team];
    pub fn id(self) -> i32 {
        match self {
            Self::Global => 0,
            Self::Personal => 1,
            Self::Team => 2,
        }
    }
    /// The name `default_scope` and the port's command use.
    pub fn name(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Personal => "private",
            Self::Team => "team",
        }
    }
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "global" => Some(Self::Global),
            "private" | "personal" => Some(Self::Personal),
            "team" => Some(Self::Team),
            _ => None,
        }
    }
    /// `FrequencyScope.fromId`: the scope of that id if the config enables
    /// it, else the config's default.
    pub fn from_id(id: i32, config: &Config) -> Self {
        Self::ALL.into_iter().find(|s| s.id() == id && config.scope_enabled(*s)).unwrap_or(config.default_scope)
    }
}

/// `enderdrives-server.toml` (`serverConfig`), the settings slice 1 reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// `[general] enable_ender_disk_<tier>`: a disk turned off is FULL,
    /// draws nothing and stores nothing (`isDisabled`).
    pub enabled: [bool; 6],
    /// `[ender_disk] type_limit_<tier>`: 1, 31, 63, 127, 255 (1 to 1024) and
    /// `Integer.MAX_VALUE` for the creative disk.
    pub type_limits: [i32; 6],
    /// `[ender_scope] default_scope`: `global`, `private` or `team`.
    pub default_scope: Scope,
    pub global_enabled: bool,
    pub private_enabled: bool,
    pub team_enabled: bool,
    /// `frequency_min` (0 to 4095) and `frequency_max` (1 to 4095): the
    /// frequency screen's range.
    pub frequency_min: i32,
    pub frequency_max: i32,
}
const TIERS: [&str; 6] = ["1k", "4k", "16k", "64k", "256k", "creative"];
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: [true; 6],
            type_limits: [1, 31, 63, 127, 255, i32::MAX],
            default_scope: Scope::Global,
            global_enabled: true,
            private_enabled: true,
            team_enabled: true,
            frequency_min: 0,
            frequency_max: 4095,
        }
    }
}
impl Config {
    /// A value of the wrong type or outside its range falls back to its
    /// default, as `ModConfigSpec` corrects it.
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut config = Self::default();
        let section = |name: &str| value.get(name);
        let int = |section: Option<&toml::Value>, key: &str, min: i64, max: i64| {
            section.and_then(|s| s.get(key)).and_then(toml::Value::as_integer).filter(|n| (min..=max).contains(n)).map(|n| n as i32)
        };
        let boolean = |section: Option<&toml::Value>, key: &str| section.and_then(|s| s.get(key)).and_then(toml::Value::as_bool);
        for (i, tier) in TIERS.iter().enumerate() {
            if let Some(on) = boolean(section("general"), &format!("enable_ender_disk_{tier}")) {
                config.enabled[i] = on;
            }
            let max = if i == 5 { i64::from(i32::MAX) } else { 1024 };
            if let Some(n) = int(section("ender_disk"), &format!("type_limit_{tier}"), 1, max) {
                config.type_limits[i] = n;
            }
        }
        let scope = section("ender_scope");
        // `FrequencyScope.getDefault`: anything else is global.
        if let Some(name) = scope.and_then(|s| s.get("default_scope")).and_then(toml::Value::as_str) {
            config.default_scope = match name.to_ascii_lowercase().as_str() {
                "private" | "personal" => Scope::Personal,
                "team" => Scope::Team,
                _ => Scope::Global,
            };
        }
        for (key, slot) in [("enable_scope_global", &mut config.global_enabled), ("enable_scope_private", &mut config.private_enabled), ("enable_scope_team", &mut config.team_enabled)] {
            if let Some(on) = boolean(scope, key) {
                *slot = on;
            }
        }
        if let Some(n) = int(scope, "frequency_min", 0, 4095) {
            config.frequency_min = n;
        }
        if let Some(n) = int(scope, "frequency_max", 1, 4095) {
            config.frequency_max = n;
        }
        Ok(config)
    }
    /// The game's `config/enderdrives-server.toml`, read once. All the Mods
    /// 10 ships none (a server config, written per world with its defaults).
    pub fn get() -> &'static Self {
        static CONFIG: OnceLock<Config> = OnceLock::new();
        CONFIG.get_or_init(|| {
            let path = crate::logging::game_home().join("config/enderdrives-server.toml");
            match std::fs::read_to_string(&path) {
                Ok(text) => Self::parse(&text).unwrap_or_else(|error| {
                    log::warn!("invalid Ender Drives config {}: {error}", path.display());
                    Self::default()
                }),
                Err(_) => Self::default(),
            }
        })
    }
    /// `FrequencyScope.isEnabled`.
    pub fn scope_enabled(&self, scope: Scope) -> bool {
        match scope {
            Scope::Global => self.global_enabled,
            Scope::Personal => self.private_enabled,
            Scope::Team => self.team_enabled,
        }
    }
    /// `EnderDiskItem.getTypeLimit`.
    pub fn type_limit(&self, id: &str) -> Option<i32> {
        disk_index(id).map(|i| self.type_limits[i])
    }
    /// `isDisabled`: the disk's toggle is off.
    pub fn disabled(&self, id: &str) -> bool {
        disk_index(id).is_some_and(|i| !self.enabled[i])
    }
}

/// `StorageCells.addCellHandler(EnderDiskInventory.HANDLER)`: each disk a cell
/// of the mod's own handler, drawn in a drive as `block/drive/ender_cell`,
/// with AE2's 63 filter slots and no cards (`ICellWorkbenchItem.getUpgrades`'
/// default). Its idle drain is the game's, by what its frequency holds
/// (`getIdleDrain`); the row keeps the least, 100 AE/t.
pub fn register(defs: &mut Defs) {
    for id in ENDER_DISKS {
        defs.add_handler_cell(HandlerCellDef { id: id.to_owned(), idle_drain: idle_drain(0), drive_model: DRIVE_MODEL.to_owned(), config_slots: CONFIG_SLOTS }, 0);
    }
}

fn custom_data(stack: &ItemStack) -> Option<&Compound> {
    stack.custom_data()
}
/// `CompoundTag.getInt`: any number, else 0.
fn int(stack: &ItemStack, key: &str) -> i32 {
    save::number(custom_data(stack).and_then(|d| d.get(&key.into()))).map_or(0, |n| n as i32)
}
fn string(stack: &ItemStack, key: &str) -> Option<String> {
    match custom_data(stack)?.get(&key.into())? {
        Tag::String(s) => Some(s.to_string_lossy()),
        _ => None,
    }
}
/// `stack.update(CUSTOM_DATA, ...)`: one key of the custom data set.
fn set(stack: &mut ItemStack, key: &str, value: Tag) {
    let mut data = custom_data(stack).cloned().unwrap_or_default();
    data.insert(key.into(), value);
    stack.data_mut().set_other(CUSTOM_DATA, Some(Tag::Compound(data)));
}

/// `getFrequency`.
pub fn frequency(stack: &ItemStack) -> i32 {
    int(stack, FREQ_KEY)
}
pub fn set_frequency(stack: &mut ItemStack, frequency: i32) {
    set(stack, FREQ_KEY, Tag::Int(frequency));
}
/// `getScope`: the default with no custom data at all.
pub fn scope(stack: &ItemStack, config: &Config) -> Scope {
    if custom_data(stack).is_none() {
        return config.default_scope;
    }
    Scope::from_id(int(stack, SCOPE_KEY), config)
}
pub fn set_scope(stack: &mut ItemStack, scope: Scope) {
    set(stack, SCOPE_KEY, Tag::Int(scope.id()));
}
/// `getOwnerUUID`, in `UUID.toString`'s form; none when it is no UUID.
pub fn owner(stack: &ItemStack) -> Option<String> {
    canonical_uuid(&string(stack, OWNER_KEY)?)
}
pub fn set_owner(stack: &mut ItemStack, uuid: &str) {
    set(stack, OWNER_KEY, Tag::String(uuid.into()));
}
/// `getStoredTeamId`, `getStoredTeamName`: FTB Teams' team, which the port
/// has not got, so only a disk that came with one carries it.
pub fn team_id(stack: &ItemStack) -> Option<String> {
    string(stack, TEAM_KEY)
}
pub fn team_name(stack: &ItemStack) -> Option<String> {
    string(stack, TEAM_NAME_KEY)
}
/// `getTransferMode`: 0 both ways, 1 input only, 2 output only.
pub fn transfer_mode(stack: &ItemStack) -> i32 {
    int(stack, TRANSFER_MODE_KEY)
}
pub fn set_transfer_mode(stack: &mut ItemStack, mode: i32) {
    set(stack, TRANSFER_MODE_KEY, Tag::Int(mode));
}

/// `UUID.fromString(...).toString()`: at most 36 characters, five groups
/// each read as a hex `long` (`Long.parseLong`) and cut to its width;
/// none for anything else.
pub fn canonical_uuid(text: &str) -> Option<String> {
    let parts: Vec<&str> = text.split('-').collect();
    if parts.len() != 5 || text.len() > 36 {
        return None;
    }
    let widths = [8, 4, 4, 4, 12];
    let mut out = Vec::with_capacity(5);
    for (part, width) in parts.iter().zip(widths) {
        let value = i64::from_str_radix(part, 16).ok()? as u64 & ((1u64 << (width * 4)) - 1);
        out.push(format!("{value:0width$x}"));
    }
    Some(out.join("-"))
}

/// `getSafeScopePrefix`: whose storage the disk reaches. A private disk's is
/// its owner's (`player_unknown` with none), a team disk's its team's
/// (global without one).
pub fn scope_prefix(stack: &ItemStack, config: &Config) -> String {
    match scope(stack, config) {
        Scope::Personal => owner(stack).map_or_else(|| "player_unknown".to_owned(), |o| format!("player_{o}")),
        Scope::Team => match team_id(stack) {
            Some(team) if !team.is_empty() => format!("team_{team}"),
            _ => "global".to_owned(),
        },
        Scope::Global => "global".to_owned(),
    }
}
/// The storage a disk reaches: its scope prefix and frequency (`AEKey`'s
/// first two fields; `DuplicateCellGuard.StorageKey` with the `item` channel).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StorageKey {
    pub scope: String,
    pub frequency: i32,
}
impl StorageKey {
    pub fn of(stack: &ItemStack, config: &Config) -> Option<Self> {
        is_disk(&stack.item).then(|| Self { scope: scope_prefix(stack, config), frequency: frequency(stack) })
    }
}

/// `AbstractEnderDiskInventory.calculateCellState` as `CellState`'s LED
/// number: 1 empty, 4 at the type limit, 3 at three quarters of it, else 2.
pub fn cell_state(types: usize, limit: i32) -> u8 {
    let types = types.min(i32::MAX as usize) as i32;
    if types == 0 {
        1
    } else if types >= limit {
        4
    } else if types as f32 / limit as f32 >= 0.75 {
        3
    } else {
        2
    }
}
/// `EnderDiskInventory.getIdleDrain` of a disk whose frequency holds `total`
/// items: 100 + 0.015 × max(1, total)^0.8 AE/t.
pub fn idle_drain(total: i64) -> f64 {
    100.0 + 0.015 * (total.max(1) as f64).powf(0.8)
}

/// What a frequency holds: `EnderDBManager.dbMap`'s entries of one scope and
/// frequency, each a key and its count (more than 0, at most `Long.MAX_VALUE`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frequency {
    items: BTreeMap<ItemKey, i64>,
    /// The sum of the counts (`getTotalItemCount`, past `Long.MAX_VALUE` too).
    total: i128,
}
/// A count as it was before a change: what an undo puts back.
#[derive(Clone, Debug, PartialEq)]
struct Undo {
    key: StorageKey,
    item: ItemKey,
    before: i64,
}
/// The ender disks' storage of one world (`EnderDBManager`): every frequency
/// of every scope. A change can be recorded ([`Store::record`]) so that a
/// transaction of the game's that fails puts the counts back ([`Store::undo_to`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Store {
    frequencies: BTreeMap<StorageKey, Frequency>,
    journal: Vec<Undo>,
    recording: bool,
    /// Changed since it was last saved.
    pub dirty: bool,
}
impl Store {
    /// `getItemCount`.
    pub fn count(&self, key: &StorageKey, item: &ItemKey) -> i64 {
        self.frequencies.get(key).and_then(|f| f.items.get(item)).copied().unwrap_or(0)
    }
    /// `getTypeCount` (`frequencySizeLocked`).
    pub fn type_count(&self, key: &StorageKey) -> usize {
        self.frequencies.get(key).map_or(0, |f| f.items.len())
    }
    /// `getTotalItemCount`, at most `Long.MAX_VALUE`.
    pub fn total(&self, key: &StorageKey) -> i64 {
        self.frequencies.get(key).map_or(0, |f| f.total.min(i128::from(i64::MAX)) as i64)
    }
    /// `queryItemsByFrequency`: the keys and counts, in key order.
    pub fn entries(&self, key: &StorageKey) -> Vec<(ItemKey, i64)> {
        self.frequencies.get(key).map_or_else(Vec::new, |f| f.items.iter().map(|(k, n)| (k.clone(), *n)).collect())
    }
    /// The scopes and frequencies that hold anything.
    pub fn keys(&self) -> impl Iterator<Item = &StorageKey> {
        self.frequencies.keys()
    }
    fn put(&mut self, key: &StorageKey, item: &ItemKey, count: i64) {
        let before = self.count(key, item);
        if before == count {
            return;
        }
        if self.recording {
            self.journal.push(Undo { key: key.clone(), item: item.clone(), before });
        }
        let frequency = self.frequencies.entry(key.clone()).or_default();
        frequency.total += i128::from(count) - i128::from(before);
        if count == 0 {
            frequency.items.remove(item);
            if frequency.items.is_empty() {
                self.frequencies.remove(key);
            }
        } else {
            frequency.items.insert(item.clone(), count);
        }
        self.dirty = true;
    }
    /// `insertItem` (`mutateItem` with a type limit): `amount` more of
    /// `item`, none of a new type once the frequency holds `type_limit`
    /// types, none past `Long.MAX_VALUE`. What went in.
    pub fn insert(&mut self, key: &StorageKey, item: &ItemKey, amount: i64, type_limit: i32) -> i64 {
        if amount <= 0 {
            return 0;
        }
        let current = self.count(key, item);
        if current == 0 && self.type_count(key) >= type_limit.max(0) as usize {
            return 0;
        }
        // clampDelta: up to Long.MAX_VALUE.
        let added = amount.min(i64::MAX - current);
        if added > 0 {
            self.put(key, item, current + added);
        }
        added
    }
    /// `saveItem` with a negative delta: at most what is there. What came out.
    pub fn extract(&mut self, key: &StorageKey, item: &ItemKey, amount: i64) -> i64 {
        let current = self.count(key, item);
        let taken = amount.max(0).min(current);
        if taken > 0 {
            self.put(key, item, current - taken);
        }
        taken
    }
    /// `clearFrequency`: everything of one frequency gone.
    pub fn clear(&mut self, key: &StorageKey) {
        for (item, _) in self.entries(key) {
            self.put(key, &item, 0);
        }
    }
    /// Changes are recorded while `on`; turning it off forgets the record.
    pub fn record(&mut self, on: bool) {
        self.recording = on;
        if !on {
            self.journal.clear();
        }
    }
    /// How many changes the record holds: a point [`Store::undo_to`] goes back to.
    pub fn mark(&self) -> usize {
        self.journal.len()
    }
    /// Puts back every recorded change since `mark`, the last first.
    pub fn undo_to(&mut self, mark: usize) {
        while self.journal.len() > mark {
            let Some(undo) = self.journal.pop() else { break };
            let recording = std::mem::replace(&mut self.recording, false);
            self.put(&undo.key, &undo.item, undo.before);
            self.recording = recording;
        }
    }

    /// The port's save of the storage (`data/enderdrives/horizon_enderdrives.dat`;
    /// the mod's own `enderdrives.bin` and its WAL hold 1.21.1 item NBT):
    /// each frequency's scope, number and stacks, AE2's `GenericStack` form
    /// with the count as `#`.
    pub fn to_nbt(&self) -> Result<Tag, String> {
        let mut frequencies = Vec::new();
        for (key, frequency) in &self.frequencies {
            let items = frequency
                .items
                .iter()
                .map(|(item, n)| ae2_generic_stack::encode(&ae2_generic_stack::GenericStack::item(item.clone(), *n)))
                .collect::<Result<Vec<_>, _>>()?;
            frequencies.push(Tag::compound([("scope", Tag::String(key.scope.as_str().into())), ("frequency", Tag::Int(key.frequency)), ("items", Tag::List(items))]));
        }
        Ok(Tag::compound([
            ("data", Tag::compound([("format", Tag::Int(1)), ("frequencies", Tag::List(frequencies))])),
            ("DataVersion", Tag::Int(horizon_world::DATA_VERSION)),
        ]))
    }
    /// The save read back. An item `known` does not take stays as AE2's
    /// Missing Content, which saves back unchanged.
    pub fn from_nbt(tag: &Tag, known: &dyn Fn(&str, KeyType) -> bool) -> Result<Self, String> {
        let mut store = Self::default();
        let data = tag.get("data").ok_or("no ender drives data")?;
        if save::number(data.get("format")) != Some(1) {
            return Err("unknown ender drives data format".into());
        }
        let Some(Tag::List(frequencies)) = data.get("frequencies") else { return Err("no ender drives frequencies".into()) };
        for entry in frequencies {
            let Some(Tag::String(scope)) = entry.get("scope") else { return Err("ender drives frequency without a scope".into()) };
            let frequency = save::number(entry.get("frequency")).ok_or("ender drives frequency without a number")? as i32;
            let key = StorageKey { scope: scope.to_string_lossy(), frequency };
            let Some(Tag::List(items)) = entry.get("items") else { return Err("ender drives frequency without items".into()) };
            for item in items {
                let Some(stack) = ae2_generic_stack::decode_tolerant_checked(item, known)? else { continue };
                if stack.key_type != KeyType::ITEMS || stack.amount <= 0 {
                    continue;
                }
                let count = store.count(&key, &stack.key).saturating_add(stack.amount);
                store.put(&key, &stack.key, count);
            }
        }
        store.dirty = false;
        Ok(store)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(id: &str) -> ItemKey {
        crate::ae2_cell_codec::key(&ItemStack::new(id, 1)).unwrap()
    }
    fn global(frequency: i32) -> StorageKey {
        StorageKey { scope: "global".into(), frequency }
    }

    #[test]
    fn the_config_defaults_and_corrections() {
        let config = Config::default();
        assert_eq!(config.type_limit(ENDER_DISK_4K), Some(31));
        assert_eq!(config.type_limit(ENDER_DISK_CREATIVE), Some(i32::MAX));
        assert_eq!(config.type_limit("ae2:item_storage_cell_1k"), None);
        let parsed = Config::parse(
            "[general]\nenable_ender_disk_1k = false\n[ender_disk]\ntype_limit_4k = 9\ntype_limit_16k = 5000\n[ender_scope]\ndefault_scope = \"PRIVATE\"\nenable_scope_team = false\nfrequency_max = 0\n",
        )
        .unwrap();
        assert!(parsed.disabled(ENDER_DISK_1K) && !parsed.disabled(ENDER_DISK_4K));
        assert_eq!(parsed.type_limits[1], 9);
        assert_eq!(parsed.type_limits[2], 63, "out of range: the default");
        assert_eq!(parsed.default_scope, Scope::Personal);
        assert_eq!(parsed.frequency_max, 4095, "below its minimum of 1: the default");
        // fromId: a scope turned off falls back to the default.
        assert_eq!(Scope::from_id(2, &parsed), Scope::Personal);
        assert_eq!(Scope::from_id(0, &parsed), Scope::Global);
        assert_eq!(Scope::from_id(9, &config), Scope::Global);
    }

    #[test]
    fn a_disk_keeps_its_frequency_scope_owner_and_mode_in_custom_data() {
        let config = Config::default();
        let mut disk = ItemStack::new(ENDER_DISK_4K, 1);
        assert_eq!((frequency(&disk), scope(&disk, &config), transfer_mode(&disk)), (0, Scope::Global, BIDIRECTIONAL));
        assert_eq!(StorageKey::of(&disk, &config), Some(global(0)));
        set_frequency(&mut disk, 42);
        set_scope(&mut disk, Scope::Personal);
        set_transfer_mode(&mut disk, OUTPUT_ONLY);
        assert_eq!(scope_prefix(&disk, &config), "player_unknown");
        set_owner(&mut disk, "0F8FAD5B-D9CB-469F-A165-70867728950E");
        assert_eq!(scope_prefix(&disk, &config), "player_0f8fad5b-d9cb-469f-a165-70867728950e");
        assert_eq!(StorageKey::of(&disk, &config), Some(StorageKey { scope: "player_0f8fad5b-d9cb-469f-a165-70867728950e".into(), frequency: 42 }));
        assert_eq!(transfer_mode(&disk), OUTPUT_ONLY);
        let data = disk.custom_data().unwrap();
        assert_eq!(data.get(&FREQ_KEY.into()), Some(&Tag::Int(42)));
        assert_eq!(data.get(&SCOPE_KEY.into()), Some(&Tag::Int(1)));
        // A team disk without FTB Teams' team is global.
        set_scope(&mut disk, Scope::Team);
        assert_eq!(scope_prefix(&disk, &config), "global");
        assert_eq!(StorageKey::of(&ItemStack::new("ae2:item_storage_cell_1k", 1), &config), None);
    }

    #[test]
    fn uuids_read_as_java_reads_them() {
        assert_eq!(canonical_uuid("1-2-3-4-5").as_deref(), Some("00000001-0002-0003-0004-000000000005"));
        assert_eq!(canonical_uuid("not a uuid"), None);
        assert_eq!(canonical_uuid("1-2-3-4"), None);
    }

    #[test]
    fn cell_states_and_idle_drain() {
        assert_eq!(cell_state(0, 31), 1);
        // 24 / 31 is past three quarters, 23 / 31 not.
        assert_eq!(cell_state(24, 31), 3);
        assert_eq!(cell_state(23, 31), 2);
        assert_eq!(cell_state(31, 31), 4);
        assert_eq!(cell_state(1, 1), 4);
        assert!((idle_drain(0) - 100.015).abs() < 1e-9);
        assert!((idle_drain(1000) - (100.0 + 0.015 * 1000f64.powf(0.8))).abs() < 1e-9);
    }

    #[test]
    fn a_frequency_counts_types_and_items_and_stops_at_its_limits() {
        let mut store = Store::default();
        let (cobble, dirt) = (key("minecraft:cobblestone"), key("minecraft:dirt"));
        assert_eq!(store.insert(&global(0), &cobble, 64, 1), 64);
        assert_eq!(store.insert(&global(0), &dirt, 5, 1), 0, "a new type past the limit");
        assert_eq!(store.insert(&global(0), &cobble, 6, 1), 6, "more of a stored type");
        assert_eq!(store.insert(&global(1), &dirt, 5, 1), 5, "another frequency");
        assert_eq!((store.count(&global(0), &cobble), store.type_count(&global(0)), store.total(&global(0))), (70, 1, 70));
        assert_eq!(store.insert(&global(0), &cobble, i64::MAX, 1), i64::MAX - 70);
        assert_eq!(store.total(&global(0)), i64::MAX);
        assert_eq!(store.extract(&global(0), &cobble, i64::MAX), i64::MAX);
        assert_eq!(store.extract(&global(0), &cobble, 10), 0);
        assert_eq!(store.type_count(&global(0)), 0);
        assert_eq!(store.keys().cloned().collect::<Vec<_>>(), vec![global(1)]);
    }

    #[test]
    fn recorded_changes_go_back_and_the_store_saves_and_reads_back() {
        let mut store = Store::default();
        let (cobble, dirt) = (key("minecraft:cobblestone"), key("minecraft:dirt"));
        store.insert(&global(0), &cobble, 10, 31);
        store.record(true);
        let outer = store.mark();
        store.insert(&global(0), &dirt, 3, 31);
        let inner = store.mark();
        store.extract(&global(0), &cobble, 4);
        store.undo_to(inner);
        assert_eq!((store.count(&global(0), &cobble), store.count(&global(0), &dirt)), (10, 3));
        store.undo_to(outer);
        assert_eq!((store.count(&global(0), &dirt), store.type_count(&global(0)), store.total(&global(0))), (0, 1, 10));
        store.record(false);
        assert_eq!(store.mark(), 0);
        let private = StorageKey { scope: "player_00000001-0002-0003-0004-000000000005".into(), frequency: 4095 };
        store.insert(&private, &dirt, 7, 31);
        let read = Store::from_nbt(&store.to_nbt().unwrap(), &|_, _| true).unwrap();
        assert_eq!(read.entries(&global(0)), store.entries(&global(0)));
        assert_eq!(read.entries(&private), vec![(dirt.clone(), 7)]);
        assert!(!read.dirty);
        // An item the game has not got any more stays, as Missing Content.
        let missing = Store::from_nbt(&store.to_nbt().unwrap(), &|id, _| id != "minecraft:dirt").unwrap();
        assert_eq!(missing.entries(&private)[0].0.item_id, "ae2:missing_content");
        assert_eq!(missing.to_nbt().unwrap(), store.to_nbt().unwrap());
    }

    #[test]
    fn the_disks_are_cells_of_their_own_handler() {
        let mut defs = Defs::ae2();
        register(&mut defs);
        for id in ENDER_DISKS {
            let row = defs.handler_cell(id).unwrap();
            assert_eq!((row.drive_model.as_str(), row.config_slots), (DRIVE_MODEL, 63));
            assert_eq!(defs.drive_model(id), Some(DRIVE_MODEL));
        }
    }
}
