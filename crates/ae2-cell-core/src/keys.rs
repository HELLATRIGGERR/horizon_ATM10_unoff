// Derived from AE2 AEKeyType, AEKeyTypes, AEKeyTypesInternal, AEItemKeys,
// AEFluidKeys and ReadableNumberConverter, commit db17504a86128fdf3dae31f5fb7a112a646e0b93.
// Copyright (c) 2021 TeamAppliedEnergistics. LGPL-3.0-or-later; see LICENSE.
//! AE2's key types: the registry `ae2:keytypes` (`AEKeyTypes.register`), each
//! type's amounts and display. AE2 registers items (`ae2:i`) and fluids
//! (`ae2:f`) first, in that order (`AppEngBase.registerKeyTypes`); an addon
//! registers its own after them (Applied Flux's `appflux:flux`).
//!
//! A key's identity stays an [`ItemKey`](crate::ItemKey) (an id and canonical
//! component bytes); its [`KeyType`] travels beside it, as AE2's `AEKey`
//! carries `getType()`. Behaviour that needs the game (codecs, containers,
//! import, export, storage buses, formation) is registered by the host
//! against the same [`KeyType`].
use std::sync::RwLock;

/// A registered key type, by its raw id in AE2's `ae2:keytypes` registry
/// (`AEKeyType.getRawId`): 0 for items, 1 for fluids, then an addon's in the
/// order they register, at most 127 (`RegistryBuilder.maxId(127)`). The raw
/// id is never saved: saves and the wire name a type by its [`id`](Self::id).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyType(u8);

/// What an `AEKeyType` subclass declares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyTypeDef {
    /// `getId`: `ae2:i`, `ae2:f`, an addon's own.
    pub id: &'static str,
    /// `getDescription`, as a translation key (`GuiText.Items`: `gui.ae2.Items`).
    pub description: &'static str,
    /// `getAmountPerOperation`: what one bus or IO Port operation moves, and
    /// the amount one AE of `StorageHelper`'s power pays for.
    pub amount_per_operation: u32,
    /// `getAmountPerByte`: what a storage cell holds per byte.
    pub amount_per_byte: u32,
    /// `getAmountPerUnit`: what one displayed unit holds (1000 mB a bucket).
    pub amount_per_unit: u32,
    /// `getUnitSymbol`: `B` for buckets.
    pub unit_symbol: Option<&'static str>,
    /// `supportsFuzzyRangeSearch`: damage-based fuzzy partitions (items only).
    pub fuzzy_range_search: bool,
}
impl KeyTypeDef {
    /// A type with `AEKeyType`'s defaults: 1 per operation, 8 per byte, 1 per
    /// unit, no unit symbol, no fuzzy range search.
    pub const fn new(id: &'static str, description: &'static str) -> Self {
        Self {
            id,
            description,
            amount_per_operation: 1,
            amount_per_byte: 8,
            amount_per_unit: 1,
            unit_symbol: None,
            fuzzy_range_search: false,
        }
    }
}

/// `AEItemKeys`.
static ITEMS: KeyTypeDef = KeyTypeDef {
    fuzzy_range_search: true,
    ..KeyTypeDef::new("ae2:i", "gui.ae2.Items")
};
/// `AEFluidKeys`: `AEFluidKey.AMOUNT_BUCKET` is 1000.
static FLUIDS: KeyTypeDef = KeyTypeDef {
    amount_per_operation: 1000 * 125 / 1000,
    amount_per_byte: 8 * 1000,
    amount_per_unit: 1000,
    unit_symbol: Some("B"),
    ..KeyTypeDef::new("ae2:f", "gui.ae2.Fluids")
};
const BUILT_IN: u8 = 2;
/// The highest raw id (`RegistryBuilder.maxId(127)`).
pub const MAX_RAW_ID: u8 = 127;
/// Addon types, from raw id 2 on. Each is leaked once at registration.
static ADDED: RwLock<Vec<&'static KeyTypeDef>> = RwLock::new(Vec::new());

/// Why [`register`] refused a type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegisterError {
    /// The id is taken: by this same definition (registering again is a
    /// no-op that returns it) or by another one.
    Duplicate(KeyType),
    /// The id is no namespaced resource location.
    InvalidId,
    /// 128 types are registered already.
    Full,
    /// An amount factor is zero.
    InvalidAmount,
}

/// `AEKeyTypes.register`: add a key type after AE2's own. Registering the
/// same definition twice returns the first registration's type (so a
/// registry built in several places, tests included, agrees on it); another
/// definition under a taken id is refused, as `Registry.register` refuses it.
pub fn register(def: KeyTypeDef) -> Result<KeyType, RegisterError> {
    if !valid_id(def.id) {
        return Err(RegisterError::InvalidId);
    }
    if def.amount_per_byte == 0 || def.amount_per_operation == 0 || def.amount_per_unit == 0 {
        return Err(RegisterError::InvalidAmount);
    }
    let mut added = ADDED.write().unwrap_or_else(|poisoned| poisoned.into_inner());
    let existing = [&ITEMS, &FLUIDS]
        .into_iter()
        .chain(added.iter().copied())
        .position(|known| known.id == def.id);
    if let Some(index) = existing {
        let kind = KeyType(index as u8);
        let known = if index < usize::from(BUILT_IN) { [&ITEMS, &FLUIDS][index] } else { added[index - usize::from(BUILT_IN)] };
        return if *known == def { Ok(kind) } else { Err(RegisterError::Duplicate(kind)) };
    }
    let raw = usize::from(BUILT_IN) + added.len();
    if raw > usize::from(MAX_RAW_ID) {
        return Err(RegisterError::Full);
    }
    added.push(Box::leak(Box::new(def)));
    Ok(KeyType(raw as u8))
}

/// A namespaced resource location: `namespace:path` of `[a-z0-9_.-]` and,
/// in the path, `/`.
fn valid_id(id: &str) -> bool {
    let Some((namespace, path)) = id.split_once(':') else { return false };
    !namespace.is_empty()
        && !path.is_empty()
        && namespace.bytes().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-'))
        && path.bytes().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'/'))
}

impl KeyType {
    /// `AEKeyType.items()`.
    pub const ITEMS: Self = Self(0);
    /// `AEKeyType.fluids()`.
    pub const FLUIDS: Self = Self(1);

    /// `AEKeyType.fromRawId`: none for an id no type holds.
    pub fn from_raw_id(raw: u8) -> Option<Self> {
        (raw < BUILT_IN || usize::from(raw - BUILT_IN) < added().len()).then_some(Self(raw))
    }
    /// `AEKeyType.getRawId`.
    pub const fn raw_id(self) -> u8 {
        self.0
    }
    /// `AEKeyTypes.get`: the type registered under `id`, none for an unknown
    /// one (where AE2 throws).
    pub fn get(id: &str) -> Option<Self> {
        match id {
            "ae2:i" => Some(Self::ITEMS),
            "ae2:f" => Some(Self::FLUIDS),
            _ => added().iter().position(|def| def.id == id).map(|index| Self(BUILT_IN + index as u8)),
        }
    }
    /// `AEKeyTypes.getAll`, in registration order.
    pub fn all() -> Vec<Self> {
        (0..BUILT_IN + added().len() as u8).map(Self).collect()
    }
    pub fn def(self) -> &'static KeyTypeDef {
        match self.0 {
            0 => &ITEMS,
            1 => &FLUIDS,
            raw => added()[usize::from(raw - BUILT_IN)],
        }
    }
    /// `getId`.
    pub fn id(self) -> &'static str {
        self.def().id
    }
    /// `getDescription`'s translation key.
    pub fn description(self) -> &'static str {
        self.def().description
    }
    /// `getAmountPerOperation`.
    pub fn amount_per_operation(self) -> u64 {
        u64::from(self.def().amount_per_operation)
    }
    /// `getAmountPerByte`.
    pub fn amount_per_byte(self) -> u64 {
        u64::from(self.def().amount_per_byte)
    }
    /// `getAmountPerUnit`.
    pub fn amount_per_unit(self) -> u64 {
        u64::from(self.def().amount_per_unit)
    }
    /// `getUnitSymbol`.
    pub fn unit_symbol(self) -> Option<&'static str> {
        self.def().unit_symbol
    }
    /// `supportsFuzzyRangeSearch`.
    pub fn supports_fuzzy_range_search(self) -> bool {
        self.def().fuzzy_range_search
    }
    /// `AEKeyType.formatAmount`: the amount in units, with the unit symbol
    /// in `FULL` (`1,234.5 B`), shortened to the slot's width otherwise.
    pub fn format_amount(self, amount: u64, format: AmountFormat) -> String {
        let per_unit = self.amount_per_unit();
        match format {
            AmountFormat::Full => {
                let mut text = if per_unit > 1 { full_units(amount, per_unit) } else { grouped(amount) };
                if let Some(unit) = self.unit_symbol() {
                    text.push(' ');
                    text.push_str(unit);
                }
                text
            }
            AmountFormat::Slot | AmountFormat::SlotLargeFont => {
                let width = if format == AmountFormat::Slot { 4 } else { 3 };
                if per_unit > 1 {
                    readable_units(amount as f64 / per_unit as f64, width)
                } else {
                    readable(amount, width)
                }
            }
        }
    }
}
fn added() -> std::sync::RwLockReadGuard<'static, Vec<&'static KeyTypeDef>> {
    ADDED.read().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// `AmountFormat`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmountFormat {
    /// Every digit, grouped, with the unit symbol.
    Full,
    /// At most 4 characters (`ReadableNumberConverter.format(n, 4)`).
    Slot,
    /// At most 3 characters, for the large font.
    SlotLargeFont,
}

/// A key of any type: AE2's `AEKey`, whose class is its type.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypedKey {
    pub key_type: KeyType,
    pub key: crate::ItemKey,
}
impl TypedKey {
    pub fn new(key_type: KeyType, key: crate::ItemKey) -> Self {
        Self { key_type, key }
    }
    pub fn item(key: crate::ItemKey) -> Self {
        Self::new(KeyType::ITEMS, key)
    }
    pub fn fluid(key: crate::ItemKey) -> Self {
        Self::new(KeyType::FLUIDS, key)
    }
}

/// A set of key types (`KeyTypeSelection`, `ae2:enabled_key_types`), by raw id.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct KeyTypeSet(u128);
impl KeyTypeSet {
    pub const EMPTY: Self = Self(0);
    /// Every registered type.
    pub fn all() -> Self {
        KeyType::all().into_iter().collect()
    }
    pub fn of(types: &[KeyType]) -> Self {
        types.iter().copied().collect()
    }
    pub fn contains(self, kind: KeyType) -> bool {
        self.0 & (1u128 << kind.0) != 0
    }
    pub fn insert(&mut self, kind: KeyType) {
        self.0 |= 1u128 << kind.0;
    }
    pub fn remove(&mut self, kind: KeyType) {
        self.0 &= !(1u128 << kind.0);
    }
    pub fn with(mut self, kind: KeyType, on: bool) -> Self {
        if on { self.insert(kind) } else { self.remove(kind) }
        self
    }
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub fn len(self) -> usize {
        self.0.count_ones() as usize
    }
    /// The types in raw-id (registration) order.
    pub fn iter(self) -> impl Iterator<Item = KeyType> {
        (0..=MAX_RAW_ID).filter(move |raw| self.0 & (1u128 << raw) != 0).map(KeyType)
    }
    /// The raw-id bits (bit 0 items, bit 1 fluids, ...).
    pub const fn bits(self) -> u128 {
        self.0
    }
    /// The registered types among `bits`.
    pub fn from_bits(bits: u128) -> Self {
        Self(bits & Self::all().0)
    }
}
impl FromIterator<KeyType> for KeyTypeSet {
    fn from_iter<I: IntoIterator<Item = KeyType>>(iter: I) -> Self {
        let mut set = Self::EMPTY;
        for kind in iter {
            set.insert(kind);
        }
        set
    }
}

/// `NumberFormat.getNumberInstance().format(long)` in English: grouped by
/// thousands with commas.
pub fn grouped(amount: u64) -> String {
    let digits = amount.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}
/// `NumberFormat.getNumberInstance().format(amount / (double) perUnit)`: at
/// most three fraction digits, rounded half-even, trailing zeros dropped.
/// Exact in integers when a unit is a divisor of 1000 (a bucket).
fn full_units(amount: u64, per_unit: u64) -> String {
    let thousandths = if 1000 % per_unit == 0 {
        u128::from(amount) * u128::from(1000 / per_unit)
    } else {
        let value = amount as f64 / per_unit as f64 * 1000.0;
        let floor = value.floor();
        let rest = value - floor;
        let up = rest > 0.5 || (rest == 0.5 && floor % 2.0 != 0.0);
        (floor as u128) + u128::from(up)
    };
    let whole = grouped((thousandths / 1000) as u64);
    let rest = thousandths % 1000;
    if rest == 0 {
        whole
    } else {
        format!("{whole}.{}", format!("{rest:03}").trim_end_matches('0'))
    }
}
/// `ReadableNumberConverter.format(long, width)`.
pub fn readable(number: u64, width: usize) -> String {
    let text = number.to_string();
    if text.len() <= width {
        return text;
    }
    let (mut base, mut last, mut exponent, mut size) = (number, number as f64 * 1000.0, 0usize, text.len());
    let mut postfix = 'K';
    while size > width {
        last = base as f64;
        base /= 1000;
        postfix = ['K', 'M', 'G', 'T', 'P', 'E'][exponent.min(5)];
        exponent += 1;
        size = base.to_string().len() + 1;
    }
    let precise = format!("{}{postfix}", decimal_down(last / 1000.0, 1));
    if precise.len() <= width { precise } else { format!("{base}{postfix}") }
}
/// `ReadableNumberConverter.format(double, width)`.
pub fn readable_units(number: f64, width: usize) -> String {
    let integer_digits = (number.log10() + 1.0).max(0.0) as i32;
    let fraction_digits = width as i32 - integer_digits - 1;
    let min_fraction = 10f64.powi(-fraction_digits);
    let fraction = number - number.floor();
    if fraction < 1e-9 || integer_digits > width as i32 - 1 {
        return readable(number as u64, width);
    }
    if fraction + 1e-9 < min_fraction && integer_digits - 1 <= width as i32 {
        return format!("~{}", readable(number as u64, width.saturating_sub(1)));
    }
    decimal_down(number, fraction_digits.max(0) as usize)
}
/// `DecimalFormat(".#")` rounding down with `digits` fraction digits at
/// most: no leading zero, no trailing zeros.
fn decimal_down(value: f64, digits: usize) -> String {
    let scale = 10u64.pow(digits as u32);
    let scaled = (value * scale as f64).floor() as u64;
    let (whole, fraction) = (scaled / scale.max(1), scaled % scale.max(1));
    let mut out = if whole == 0 && fraction != 0 { String::new() } else { whole.to_string() };
    if fraction != 0 {
        let digits = format!("{fraction:0width$}", width = digits);
        out.push('.');
        out.push_str(digits.trim_end_matches('0'));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_types_match_ae2_items_and_fluids() {
        assert_eq!(KeyType::get("ae2:i"), Some(KeyType::ITEMS));
        assert_eq!(KeyType::get("ae2:f"), Some(KeyType::FLUIDS));
        assert_eq!(KeyType::ITEMS.raw_id(), 0);
        assert_eq!(KeyType::FLUIDS.raw_id(), 1);
        assert_eq!(KeyType::from_raw_id(1), Some(KeyType::FLUIDS));
        assert_eq!(KeyType::get("ae2:x"), None);
        let (items, fluids) = (KeyType::ITEMS, KeyType::FLUIDS);
        assert_eq!((items.amount_per_byte(), items.amount_per_operation(), items.amount_per_unit()), (8, 1, 1));
        assert_eq!((fluids.amount_per_byte(), fluids.amount_per_operation(), fluids.amount_per_unit()), (8000, 125, 1000));
        assert_eq!((items.unit_symbol(), fluids.unit_symbol()), (None, Some("B")));
        assert!(items.supports_fuzzy_range_search() && !fluids.supports_fuzzy_range_search());
        assert_eq!((items.description(), fluids.description()), ("gui.ae2.Items", "gui.ae2.Fluids"));
        assert_eq!(&KeyType::all()[..2], &[items, fluids]);
    }

    #[test]
    fn an_addon_type_registers_once_after_ae2s_and_conflicts_are_refused() {
        let def = KeyTypeDef { amount_per_byte: 1 << 20, ..KeyTypeDef::new("keytypes_test:energy", "gui.keytypes_test.energy") };
        let kind = register(def.clone()).unwrap();
        assert!(kind.raw_id() >= 2);
        assert_eq!(register(def.clone()), Ok(kind), "the same definition again is the same type");
        assert_eq!(register(KeyTypeDef::new("keytypes_test:energy", "other")), Err(RegisterError::Duplicate(kind)));
        assert_eq!(register(KeyTypeDef::new("ae2:i", "gui.ae2.Items")), Err(RegisterError::Duplicate(KeyType::ITEMS)));
        assert_eq!(register(KeyTypeDef::new("Bad Id", "x")), Err(RegisterError::InvalidId));
        assert_eq!(register(KeyTypeDef { amount_per_byte: 0, ..KeyTypeDef::new("keytypes_test:zero", "x") }), Err(RegisterError::InvalidAmount));
        assert_eq!(KeyType::get("keytypes_test:energy"), Some(kind));
        assert_eq!(KeyType::from_raw_id(kind.raw_id()), Some(kind));
        assert_eq!(kind.amount_per_byte(), 1 << 20);
        assert!(KeyType::all().contains(&kind));
        assert!(KeyTypeSet::all().contains(kind));
    }

    #[test]
    fn sets_keep_registration_order_and_only_registered_bits() {
        let set = KeyTypeSet::of(&[KeyType::FLUIDS, KeyType::ITEMS]);
        assert_eq!(set.iter().collect::<Vec<_>>(), vec![KeyType::ITEMS, KeyType::FLUIDS]);
        assert_eq!(set.bits(), 3);
        assert_eq!(set.with(KeyType::ITEMS, false).iter().collect::<Vec<_>>(), vec![KeyType::FLUIDS]);
        assert!(KeyTypeSet::from_bits(1 << 120).is_empty());
        assert_eq!(KeyTypeSet::from_bits(2), KeyTypeSet::of(&[KeyType::FLUIDS]));
    }

    #[test]
    fn amounts_format_as_ae2s_key_types_print_them() {
        let (items, fluids) = (KeyType::ITEMS, KeyType::FLUIDS);
        assert_eq!(items.format_amount(1_234_567, AmountFormat::Full), "1,234,567");
        assert_eq!(fluids.format_amount(1_234_567, AmountFormat::Full), "1,234.567 B");
        assert_eq!(fluids.format_amount(1_500, AmountFormat::Full), "1.5 B");
        assert_eq!(fluids.format_amount(2_000, AmountFormat::Full), "2 B");
        assert_eq!(items.format_amount(12_345, AmountFormat::Slot), "12K");
        assert_eq!(items.format_amount(999, AmountFormat::SlotLargeFont), "999");
        assert_eq!(items.format_amount(1_500, AmountFormat::SlotLargeFont), "1K");
        assert_eq!(items.format_amount(1_500, AmountFormat::Slot), "1500");
        assert_eq!(fluids.format_amount(1_500, AmountFormat::Slot), "1.5");
        assert_eq!(fluids.format_amount(250, AmountFormat::Slot), ".25");
    }
}
