//! Applied Flux 1.21-2.1.7's rows in the definitions registry, its key type
//! and its FE cells (feature `ae2-appflux`). Derived from Applied Flux by
//! GlodBlock (`FluxKeyType`, `FluxKey`, `EnergyType`, `AFConfig`,
//! `AFSingletons`, `AFRegistryHandler`, `ItemFECell`, `FECellHandler`,
//! `FluxCellInventory`), https://github.com/GlodBlock/ExtendedAE branch
//! `appflux/1.21.1-neoforge` commit `523c3e5a`, LGPL-3.0; see
//! `ae2/addons/appflux/`. The game's side (the cells in a grid, the storage,
//! import and export bus strategies) is in `game/ae2x/appflux/`.
use super::defs::{Defs, HandlerCellDef, PortableBattery};
use crate::ae2_key_types::{self, KeyBehaviour, KeyCodec, KeyType, KeyTypeDef};
use crate::items::ItemStack;
use ae2_cell_core::ItemKey;
use horizon_world::nbt::{Compound, Tag};
use std::sync::OnceLock;

pub const NAMESPACE: &str = "appflux";
/// `FluxKeyType`: `AppFlux.id("flux")`, described as `appflux.key.flux`.
pub const KEY_TYPE: &str = "appflux:flux";
/// `EnergyType.FE`'s key (`FluxKey.getId`: `appflux:fe`), saved as `type: "FE"`.
pub const FE: &str = "appflux:fe";
/// `EnergyType.GTEU`'s key: the codec reads it, no cell of this port holds it.
pub const GTEU: &str = "appflux:gteu";
/// `AFSingletons.FE_ENERGY`: a cell's stored FE, a long (`Codec.LONG`).
pub const FE_ENERGY: &str = "appflux:fe_energy";
/// `FluxKeyType.getAmountPerOperation`: 1024 × 16.
pub const AMOUNT_PER_OPERATION: u32 = 1024 * 16;
/// `AFSingletons`: `ItemFECell(core, kilobytes, idleDrain)`, 1k to 256M.
pub const CELLS: [(&str, u64, f64); 10] = [
    ("1k", 1, 0.5),
    ("4k", 4, 1.0),
    ("16k", 16, 1.5),
    ("64k", 64, 2.0),
    ("256k", 256, 2.5),
    ("1m", 1024, 3.0),
    ("4m", 4 * 1024, 4.0),
    ("16m", 16 * 1024, 5.0),
    ("64m", 64 * 1024, 6.0),
    ("256m", 256 * 1024, 7.0),
];
/// `AFSingletons`: `ItemPortableFECell(quartKilobytes, idleDrain, 0x80caff)`,
/// 1k to 256M: a quarter of the cell's bytes.
pub const PORTABLE_CELLS: [(&str, u64, f64); 10] = [
    ("1k", 1, 0.5),
    ("4k", 4, 1.0),
    ("16k", 16, 1.5),
    ("64k", 64, 2.0),
    ("256k", 256, 2.5),
    ("1m", 1024, 3.0),
    ("4m", 4 * 1024, 3.5),
    ("16m", 16 * 1024, 4.0),
    ("64m", 64 * 1024, 4.5),
    ("256m", 256 * 1024, 5.0),
];
/// `appflux:fe_<size>_cell`.
pub fn cell_id(size: &str) -> String {
    format!("{NAMESPACE}:fe_{size}_cell")
}
/// `appflux:fe_<size>_portable_cell`.
pub fn portable_cell_id(size: &str) -> String {
    format!("{NAMESPACE}:fe_{size}_portable_cell")
}
/// `IFluxCell.getBytes`: an `ItemFECell`'s kilobytes × 1,024, an
/// `ItemPortableFECell`'s quarter kilobytes × 256; none for any other item.
pub fn cell_bytes(id: &str) -> Option<u64> {
    let rest = id.strip_prefix("appflux:fe_")?;
    if let Some(size) = rest.strip_suffix("_portable_cell") {
        return PORTABLE_CELLS.iter().find(|(s, _, _)| *s == size).map(|(_, quarter, _)| quarter * 256);
    }
    let size = rest.strip_suffix("_cell")?;
    CELLS.iter().find(|(s, _, _)| *s == size).map(|(_, kilobytes, _)| kilobytes * 1024)
}
/// An `ItemPortableFECell`.
pub fn is_portable_cell(id: &str) -> bool {
    id.ends_with("_portable_cell") && cell_bytes(id).is_some()
}
/// `ItemPortableFECell.getAEMaxPower` (8 times AE2's from 1M up, `isMega`)
/// and `getChargeRate` ((80 + 80 × energy cards), twice that from 1M up).
pub fn portable_battery(quarter_kilobytes: u64) -> PortableBattery {
    let mega = quarter_kilobytes >= 1024;
    PortableBattery { battery_scale: if mega { 8.0 } else { 1.0 }, charge_scale: if mega { 2.0 } else { 1.0 } }
}

/// `AFSingletons.FLUX_ACCESSOR`: `BlockFluxAccessor`, `TileFluxAccessor`.
pub const FLUX_ACCESSOR: &str = "appflux:flux_accessor";
/// `AFSingletons.PART_FLUX_ACCESSOR`: `PartFluxAccessor`.
pub const PART_FLUX_ACCESSOR: &str = "appflux:part_flux_accessor";
/// `TileFluxAccessor` / `PartFluxAccessor`: `setIdlePowerUsage(1.0)`, `GridFlags.REQUIRE_CHANNEL`.
pub const ACCESSOR_IDLE: f64 = 1.0;
/// `PartFluxAccessor.getBoxes`: in sixteenths, the part on the south face.
pub const PART_BOXES: [[f64; 6]; 2] = [[2., 2., 14., 14., 14., 16.], [4., 4., 12., 12., 12., 14.]];
/// `PartFluxAccessor.RL`: its static model.
pub const PART_MODEL: &str = "appflux:part/flux_accessor";

/// `EnergyIO`: what an accessor's FE storage lets through, saved as its
/// ordinal (`io`); `BOTH` by default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EnergyIo {
    Input,
    Output,
    #[default]
    Both,
}
impl EnergyIo {
    /// `EnergyIO.byId`; an unknown ordinal throws there, it is `BOTH` here.
    pub fn by_id(ordinal: i32) -> Self {
        match ordinal {
            0 => Self::Input,
            1 => Self::Output,
            _ => Self::Both,
        }
    }
    pub fn ordinal(self) -> i32 {
        self as i32
    }
    pub fn is_input(self) -> bool {
        self != Self::Output
    }
    pub fn is_output(self) -> bool {
        self != Self::Input
    }
}

/// `EnergyTickRecord`: how often an accessor pushes on a side, every 1 to
/// 10 ticks, faster while it sends as much or more, slower while less.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TickRecord {
    last_sent: u64,
    pub rate: u32,
    next_tick: i64,
}
impl Default for TickRecord {
    fn default() -> Self {
        Self { last_sent: 0, rate: Self::THRESHOLD, next_tick: 0 }
    }
}
impl TickRecord {
    pub const MIN_RATE: u32 = 1;
    pub const MAX_RATE: u32 = 10;
    pub const THRESHOLD: u32 = 4;
    /// `sent`: the rate after a push of `sent` FE.
    pub fn sent(&mut self, sent: u64) {
        let faster = if sent == self.last_sent { sent != 0 } else { sent > self.last_sent };
        if faster && self.rate > Self::MIN_RATE {
            self.rate = if self.rate > Self::THRESHOLD { self.rate / 2 } else { self.rate - 1 };
        } else if !faster && self.rate < Self::MAX_RATE {
            self.rate = if self.rate < Self::THRESHOLD { self.rate * 2 } else { self.rate + 1 };
        }
        self.last_sent = sent;
    }
    /// `needTick`: whether tick `current` pushes; the next is `rate` later.
    pub fn need_tick(&mut self, current: i64) -> bool {
        if current >= self.next_tick {
            self.next_tick = current + i64::from(self.rate);
            return true;
        }
        false
    }
}

/// `appflux-common.toml` (`AFConfig`). All the Mods 10 ships none, so the
/// mod's defaults hold there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    /// `flux_cell.amount`, 1 to `Integer.MAX_VALUE`: FE a byte holds (1,048,576).
    pub flux_per_byte: u32,
    /// `flux_accessor.io_limit`, 0 to `Integer.MAX_VALUE`; 0 is none.
    pub accessor_io: u64,
    /// `flux_accessor.enable`: the Flux Accessor charges the grid from stored FE (false).
    pub accessor_charges_grid: bool,
    /// `misc.enable`: the Import Bus imports FE (false).
    pub import_bus: bool,
    /// `third_party_support.enable_mi` (true).
    pub mi_eu: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self { flux_per_byte: 1024 * 1024, accessor_io: 0, accessor_charges_grid: false, import_bus: false, mi_eu: true }
    }
}
impl Config {
    /// A value of the wrong type or outside its range falls back to its
    /// default, as `ModConfigSpec` corrects it.
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut config = Self::default();
        let get = |section: &str, key: &str| value.get(section).and_then(|s| s.get(key));
        let max = i64::from(i32::MAX);
        if let Some(n) = get("flux_cell", "amount").and_then(toml::Value::as_integer).filter(|n| (1..=max).contains(n)) {
            config.flux_per_byte = n as u32;
        }
        if let Some(n) = get("flux_accessor", "io_limit").and_then(toml::Value::as_integer).filter(|n| (0..=max).contains(n)) {
            config.accessor_io = n as u64;
        }
        if let Some(b) = get("flux_accessor", "enable").and_then(toml::Value::as_bool) {
            config.accessor_charges_grid = b;
        }
        if let Some(b) = get("misc", "enable").and_then(toml::Value::as_bool) {
            config.import_bus = b;
        }
        if let Some(b) = get("third_party_support", "enable_mi").and_then(toml::Value::as_bool) {
            config.mi_eu = b;
        }
        Ok(config)
    }
    /// `getFluxAccessorIO`: no limit is `Long.MAX_VALUE`.
    pub fn accessor_limit(&self) -> u64 {
        if self.accessor_io == 0 { i64::MAX as u64 } else { self.accessor_io }
    }
    /// The game's `config/appflux-common.toml`, read once (`ModConfigEvent`).
    pub fn get() -> Self {
        static CONFIG: OnceLock<Config> = OnceLock::new();
        *CONFIG.get_or_init(|| {
            let path = crate::logging::game_home().join("config/appflux-common.toml");
            match std::fs::read_to_string(&path) {
                Ok(text) => Self::parse(&text).unwrap_or_else(|error| {
                    log::warn!("invalid Applied Flux config {}: {error}", path.display());
                    Self::default()
                }),
                Err(_) => Self::default(),
            }
        })
    }
}

/// `FluxKey.MAP_CODEC`: `type`, `EnergyType`'s serialized name.
fn key_decode(data: &Compound) -> Result<ItemKey, String> {
    let Some(Tag::String(name)) = data.get(&"type".into()) else { return Err("missing flux key type".into()) };
    let id = match name.to_string_lossy().as_str() {
        "FE" => FE,
        "GTEU" => GTEU,
        _ => return Err("unknown energy type".into()),
    };
    Ok(fe_key_of(id))
}
fn key_encode(key: &ItemKey) -> Result<Compound, String> {
    let name = match key.item_id.as_str() {
        FE => "FE",
        GTEU => "GTEU",
        _ => return Err("not a flux key".into()),
    };
    if !key.components.is_empty() {
        return Err("a flux key has no components".into());
    }
    let mut data = Compound::default();
    data.insert("type".into(), Tag::String(name.into()));
    Ok(data)
}
fn fe_key_of(id: &str) -> ItemKey {
    ItemKey { item_id: id.to_owned(), components: Vec::new() }
}
/// `FluxKey.of(EnergyType.FE)`.
pub fn fe_key() -> ItemKey {
    fe_key_of(FE)
}

/// FE a generic slot holds (an ME Interface's, a Pattern Provider's return slot).
pub const SLOT_CAPACITY: u64 = 1_000_000;
/// `AEKeyTypes.register(FluxKeyType.TYPE)`, once: its amounts (FE a byte
/// from the config), its codec, and the item that draws it in a slot
/// (`FluxKeyRenderHandler`'s `appflux:energy/fe` sprite, the content's
/// `appflux:fe` icon item); `GenericSlotCapacities.register(FluxKeyType.TYPE,
/// 1000000L)` beside it. The container strategy over an item's energy is the
/// game's (`FEContainerItemStrategy`).
pub fn key_type() -> KeyType {
    static KIND: OnceLock<KeyType> = OnceLock::new();
    *KIND.get_or_init(|| {
        let def = KeyTypeDef {
            amount_per_operation: AMOUNT_PER_OPERATION,
            amount_per_byte: Config::get().flux_per_byte,
            ..KeyTypeDef::new(KEY_TYPE, "appflux.key.flux")
        };
        let behaviour = KeyBehaviour {
            codec: KeyCodec { decode: key_decode, encode: key_encode },
            icon: |key| ItemStack::new(key.item_id.as_str(), 1),
            known: Some(|id| id == FE || id == GTEU),
            container: None,
        };
        let kind = ae2_key_types::register(def, behaviour).expect("Applied Flux's key type registers");
        ae2_key_types::register_slot_capacity(kind, SLOT_CAPACITY);
        kind
    })
}

/// `FluxCellInventory`: one FE amount in the item's `appflux:fe_energy`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeCell {
    /// `getTotalBytes`.
    pub bytes: u64,
    /// FE a byte holds (`FluxKeyType.getAmountPerByte`).
    pub per_byte: u64,
    pub stored: u64,
    /// A Void Card: everything offered is taken (`hasVoidUpgrade`).
    pub void: bool,
}
/// `CellState` as the LED reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeCellState {
    Empty,
    NotEmpty,
    Full,
}
impl FeCell {
    /// `getMaxEnergy`: bytes × FE a byte (2^48 FE for a 256M cell at most).
    pub fn max_energy(&self) -> u64 {
        self.bytes.saturating_mul(self.per_byte)
    }
    /// `getUsedBytes`: the stored FE in whole bytes, rounded up.
    pub fn used_bytes(&self) -> u64 {
        self.stored.div_ceil(self.per_byte.max(1))
    }
    pub fn status(&self) -> FeCellState {
        if self.stored == 0 {
            FeCellState::Empty
        } else if self.stored >= self.max_energy() {
            FeCellState::Full
        } else {
            FeCellState::NotEmpty
        }
    }
    /// `insert` of FE: what fits, or with a Void Card all of `amount`.
    pub fn insert(&mut self, amount: u64, modulate: bool) -> u64 {
        let inserted = self.max_energy().saturating_sub(self.stored).min(amount);
        if modulate {
            self.stored += inserted;
        }
        if self.void { amount } else { inserted }
    }
    /// `extract` of FE.
    pub fn extract(&mut self, amount: u64, modulate: bool) -> u64 {
        let extracted = self.stored.min(amount);
        if modulate {
            self.stored -= extracted;
        }
        extracted
    }
    /// `persist`: the component is removed when empty.
    pub fn store(&self, stack: &mut ItemStack) {
        let value = (self.stored > 0).then(|| Tag::Long(self.stored.min(i64::MAX as u64) as i64));
        stack.data_mut().set_other(FE_ENERGY, value);
    }
}
/// `FECellHandler.getCellInventory`: an FE cell's inventory; none for any
/// other item. A negative or missing amount is none stored.
pub fn fe_cell(stack: &ItemStack) -> Option<FeCell> {
    let bytes = cell_bytes(&stack.item)?;
    let stored = match stack.data().and_then(|data| data.other(FE_ENERGY)) {
        Some(Tag::Long(n)) => (*n).max(0) as u64,
        _ => 0,
    };
    let void = crate::ae2_upgrade_inventory::Inventory::read(stack)
        .is_ok_and(|cards| cards.slots.iter().flatten().any(|card| card.item.as_ref() == "ae2:void_card"));
    Some(FeCell { bytes, per_byte: key_type().amount_per_byte(), stored, void })
}

pub fn register(defs: &mut Defs) {
    key_type();
    // TileFluxAccessor: a grid node of its own kind, no inventory; its `fast`
    // and `io` in its config. PartFluxAccessor: a part of 1 AE/t with a
    // channel, its settings a part machine's.
    use crate::ae2_world::ExtMachine;
    for (id, part) in [(FLUX_ACCESSOR, false), (PART_FLUX_ACCESSOR, true)] {
        defs.add_machine(ExtMachine { id, physical_slots: 0, upgrade_slots: 0, key_drops: None, power_capacity: None, part });
    }
    defs.add_part(super::defs::PartDef { id: PART_FLUX_ACCESSOR.to_owned(), idle_power: ACCESSOR_IDLE, requires_channel: true, cells: 0 });
    for (size, _, drain) in CELLS {
        let id = cell_id(size);
        // StorageCells.addCellHandler(FECellHandler.HANDLER); UpgradeInventories.forItem(is, 1);
        // StorageCellModels.registerModel(cell, AppFlux.id("block/drive/fe_<size>_cell")).
        defs.add_handler_cell(
            HandlerCellDef { id: id.clone(), idle_drain: drain, drive_model: format!("{NAMESPACE}:block/drive/fe_{size}_cell"), config_slots: 0 },
            1,
        );
        // AFRegistryHandler.registerCellUpgrades: a Void Card on each cell.
        defs.add_upgrade("ae2:void_card", &id, 1);
    }
    for (size, quarter, drain) in PORTABLE_CELLS {
        let id = portable_cell_id(size);
        // FECellHandler takes any IFluxCell; UpgradeInventories.forItem(is, 4);
        // the drive model is the tier's cell's.
        defs.add_portable_handler_cell(
            HandlerCellDef { id: id.clone(), idle_drain: drain, drive_model: format!("{NAMESPACE}:block/drive/fe_{size}_cell"), config_slots: 0 },
            4,
            portable_battery(quarter),
        );
        // registerPortableCellUpgrades: a Void Card, two Energy Cards (and an
        // Induction Card, a later slice's item).
        defs.add_upgrade("ae2:void_card", &id, 1);
        defs.add_upgrade("ae2:energy_card", &id, 2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ae2_generic_stack::{self as generic, GenericStack};

    #[test]
    fn the_flux_key_type_has_applied_fluxs_amounts_and_saved_form() {
        let kind = key_type();
        assert_eq!(KeyType::get(KEY_TYPE), Some(kind));
        assert!(kind.raw_id() >= 2);
        assert_eq!((kind.amount_per_operation(), kind.amount_per_byte(), kind.amount_per_unit()), (16_384, 1_048_576, 1));
        assert_eq!(kind.unit_symbol(), None);
        assert_eq!(kind.description(), "appflux.key.flux");
        // AEKeyType.formatAmount without a unit: grouped digits.
        assert_eq!(kind.format_amount(1_234_567, ae2_key_types::AmountFormat::Full), "1,234,567");
        // A GenericStack of FE, as AE2 saves it: `#t`, the key's `type`, `#`.
        let stack = GenericStack { key: fe_key(), key_type: kind, amount: 5_000 };
        let tag = generic::encode(&stack).unwrap();
        assert_eq!(tag, Tag::compound([("#t", Tag::String("appflux:flux".into())), ("type", Tag::String("FE".into())), ("#", Tag::Long(5_000))]));
        assert_eq!(generic::decode(&tag).unwrap().unwrap(), stack);
        let gteu = Tag::compound([("#t", Tag::String("appflux:flux".into())), ("type", Tag::String("GTEU".into())), ("#", Tag::Long(1))]);
        assert_eq!(generic::decode(&gteu).unwrap().unwrap().key.item_id, GTEU);
        assert!(key_decode(&Compound::from_iter([("type".into(), Tag::String("RF".into()))])).is_err());
        assert!(key_encode(&ItemKey { item_id: "minecraft:stone".into(), components: Vec::new() }).is_err());
        assert_eq!((ae2_key_types::behaviour(kind).icon)(&fe_key()).item.as_ref(), FE);
    }

    #[test]
    fn fe_cells_hold_bytes_times_fe_per_byte_with_no_type_overhead() {
        // 2^30 FE in a 1k cell, 2^48 FE in a 256M cell.
        let expected: [u64; 10] = [1 << 30, 1 << 32, 1 << 34, 1 << 36, 1 << 38, 1 << 40, 1 << 42, 1 << 44, 1 << 46, 1 << 48];
        for ((size, kilobytes, _), max) in CELLS.iter().zip(expected) {
            let stack = ItemStack::new(cell_id(size).as_str(), 1);
            let cell = fe_cell(&stack).unwrap();
            assert_eq!(cell.bytes, kilobytes * 1024);
            assert_eq!(cell.max_energy(), max, "{size}");
            assert_eq!(cell.status(), FeCellState::Empty);
        }
        assert!(fe_cell(&ItemStack::new("ae2:item_storage_cell_1k", 1)).is_none());
        assert!(cell_bytes("appflux:fe_2k_cell").is_none());
        let mut stack = ItemStack::new("appflux:fe_1k_cell", 1);
        let mut cell = fe_cell(&stack).unwrap();
        assert_eq!(cell.insert(1, false), 1);
        assert_eq!(cell.stored, 0, "a simulation changes nothing");
        assert_eq!(cell.insert(1_048_577, true), 1_048_577);
        assert_eq!(cell.used_bytes(), 2, "a byte and one FE take two bytes");
        assert_eq!(cell.insert(u64::MAX, true), (1 << 30) - 1_048_577);
        assert_eq!(cell.status(), FeCellState::Full);
        assert_eq!(cell.insert(5, true), 0);
        assert_eq!(cell.extract(1 << 31, true), 1 << 30);
        assert_eq!(cell.status(), FeCellState::Empty);
        cell.insert(777, true);
        cell.store(&mut stack);
        assert_eq!(stack.data().unwrap().other(FE_ENERGY), Some(&Tag::Long(777)));
        assert_eq!(fe_cell(&stack).unwrap().stored, 777);
        cell.extract(777, true);
        cell.store(&mut stack);
        assert_eq!(stack.data().and_then(|data| data.other(FE_ENERGY)), None, "an empty cell drops the component");
        // A Void Card takes what doesn't fit.
        let mut void = FeCell { bytes: 1024, per_byte: 1_048_576, stored: 1 << 30, void: true };
        assert_eq!(void.insert(99, true), 99);
        assert_eq!(void.stored, 1 << 30);
    }

    #[test]
    fn the_config_reads_applied_fluxs_keys_and_corrects_bad_values() {
        assert_eq!(Config::parse("").unwrap(), Config::default());
        let config = Config::parse("[flux_cell]\namount = 1000\n[flux_accessor]\nio_limit = 64\nenable = true\n[misc]\nenable = true\n[third_party_support]\nenable_mi = false\n").unwrap();
        assert_eq!(config, Config { flux_per_byte: 1000, accessor_io: 64, accessor_charges_grid: true, import_bus: true, mi_eu: false });
        assert_eq!(config.accessor_limit(), 64);
        let bad = Config::parse("[flux_cell]\namount = 0\n[flux_accessor]\nio_limit = -1\nenable = \"yes\"\n").unwrap();
        assert_eq!(bad, Config::default());
        assert_eq!(bad.accessor_limit(), i64::MAX as u64);
    }

    #[test]
    fn portable_fe_cells_hold_a_quarter_of_the_bytes_with_appflux_batteries() {
        // 2^28 FE in a 1k portable cell, 2^46 FE in a 256M one.
        for ((size, quarter, _), (cell_size, kilobytes, _)) in PORTABLE_CELLS.iter().zip(CELLS) {
            assert_eq!(size, &cell_size);
            let id = portable_cell_id(size);
            assert!(is_portable_cell(&id) && !is_portable_cell(&cell_id(size)));
            let cell = fe_cell(&ItemStack::new(id.as_str(), 1)).unwrap();
            assert_eq!(cell.bytes, quarter * 256);
            assert_eq!(cell.max_energy(), kilobytes * 1024 * 1_048_576 / 4, "{size}");
        }
        assert_eq!(portable_battery(256), PortableBattery { battery_scale: 1.0, charge_scale: 1.0 });
        assert_eq!(portable_battery(1024), PortableBattery { battery_scale: 8.0, charge_scale: 2.0 });
        assert!(cell_bytes("appflux:fe_2k_portable_cell").is_none());
        crate::ae2x::install();
        let defs = crate::ae2x::defs::get();
        for (size, _, drain) in PORTABLE_CELLS {
            let id = portable_cell_id(size);
            assert_eq!(defs.handler_cell(&id).unwrap().idle_drain, drain);
            assert_eq!(defs.drive_model(&id), Some(format!("appflux:block/drive/fe_{size}_cell").as_str()));
            assert_eq!(defs.upgrade_slots(&id), Some(4));
            assert_eq!((defs.upgrade_limit("ae2:void_card", &id), defs.upgrade_limit("ae2:energy_card", &id)), (1, 2));
            assert!(defs.portable_cell(&id).is_none(), "no basic cell");
        }
        assert_eq!(defs.portable_battery("appflux:fe_16m_portable_cell").unwrap().battery_scale, 8.0);
        assert_eq!(defs.portable_battery("ae2:portable_item_cell_1k").unwrap().battery_scale, 1.0);
        assert!(defs.portable_battery("appflux:fe_1k_cell").is_none());
    }

    #[test]
    fn the_accessor_tick_record_and_io_follow_applied_flux() {
        let mut record = TickRecord::default();
        assert!(record.need_tick(0) && !record.need_tick(3) && record.need_tick(4));
        // Sending more speeds it up by one below the threshold, halves above.
        record.sent(100);
        assert_eq!(record.rate, 3);
        record.sent(100);
        record.sent(100);
        assert_eq!(record.rate, 1);
        record.sent(100);
        assert_eq!(record.rate, 1, "no faster than every tick");
        // Nothing sent slows it: doubling below the threshold, one above.
        for expected in [2, 4, 5, 6, 7, 8, 9, 10, 10] {
            record.sent(0);
            assert_eq!(record.rate, expected);
        }
        record.sent(5);
        assert_eq!(record.rate, 5, "more than last time halves it");
        record.sent(4);
        assert_eq!(record.rate, 6, "less slows it");
        assert_eq!(EnergyIo::by_id(0), EnergyIo::Input);
        assert_eq!(EnergyIo::by_id(7), EnergyIo::Both);
        assert!(EnergyIo::Input.is_input() && !EnergyIo::Input.is_output());
        assert!(EnergyIo::Output.is_output() && !EnergyIo::Output.is_input());
        assert_eq!(EnergyIo::Output.ordinal(), 1);
        crate::ae2x::install();
        let defs = crate::ae2x::defs::get();
        let part = defs.part(PART_FLUX_ACCESSOR).unwrap();
        assert!(part.requires_channel && part.idle_power == 1.0 && part.cells == 0);
        assert!(defs.machines().iter().any(|m| m.id == FLUX_ACCESSOR && !m.part));
        assert!(defs.machines().iter().any(|m| m.id == PART_FLUX_ACCESSOR && m.part));
    }

    #[test]
    fn the_registry_has_the_ten_fe_cells_with_a_void_card_slot() {
        crate::ae2x::install();
        let defs = crate::ae2x::defs::get();
        for (size, _, drain) in CELLS {
            let id = cell_id(size);
            let row = defs.handler_cell(&id).unwrap();
            assert_eq!(row.idle_drain, drain);
            assert_eq!(defs.drive_model(&id), Some(format!("appflux:block/drive/fe_{size}_cell").as_str()));
            assert!(crate::ae2_cell_codec::is_cell(&id));
        }
    }
}
