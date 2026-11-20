//! Applied Mekanistics' rows: its key type and its cells. Derived from Applied Mekanistics 1.6.3 by
//! ramidzkh (`MekanismKeyType`, `MekanismKey`, `AMItems`, `ChemicalStorageCell`,
//! `ChemicalPortableCellItem`, `AppliedMekanistics.initializeUpgrades`), LGPL-3.0; read from the
//! jar ATM10 ships (ae2/addons/appmek).
use ae2_cell_core::{ItemKey, Tier};
use horizon_client::ae2_key_types::{self, KeyBehaviour, KeyCodec, KeyType, KeyTypeDef};
use horizon_client::ae2x::defs::{CellDef, Defs, add_storage_cell_cards};
use horizon_client::game::ae2_ext::{KeyStrategyKinds, add_key_strategies};
use horizon_client::items::ItemStack;
use horizon_core::nbt::{Compound, Tag};
use std::sync::{OnceLock, RwLock};

pub const NAMESPACE: &str = "appmek";
/// `MekanismKeyType.TYPE`: `AppliedMekanistics.id("chemical")`, described as `AMText.CHEMICALS`.
pub const KEY_TYPE: &str = "appmek:chemical";
/// `AMText.CHEMICALS`'s translation key ("Chemicals").
pub const DESCRIPTION: &str = "text.appmek.chemical";
/// `getAmountPerOperation`, `getAmountPerByte`, `getAmountPerUnit`, `getUnitSymbol`: a fluid's.
pub const AMOUNT_PER_OPERATION: u32 = 125;
pub const AMOUNT_PER_BYTE: u32 = 8000;
pub const AMOUNT_PER_UNIT: u32 = 1000;
pub const UNIT_SYMBOL: &str = "B";
/// `GenericSlotCapacities.register(TYPE, GenericSlotCapacities.getMap().get(AEKeyType.fluids()))`.
pub const SLOT_CAPACITY: u64 = 4000;
/// `AMItems.CHEMICAL_CELL_HOUSING`.
pub const HOUSING: &str = "appmek:chemical_cell_housing";
/// `ChemicalStorageCell`: `BasicStorageCell(..., 5, MekanismKeyType.TYPE)`.
pub const CELL_TYPES: usize = 5;
/// `ChemicalPortableCellItem(18, ...)`.
pub const PORTABLE_CELL_TYPES: usize = 18;
/// The item a chemical entry shows as in a slot until chemicals are drawn with their own sprite
/// (docs/appmek.md, slice 3): Mekanism's Basic Chemical Tank.
pub const ICON: &str = "mekanism:basic_chemical_tank";
const TIERS: [Tier; 5] = [Tier::K1, Tier::K4, Tier::K16, Tier::K64, Tier::K256];

/// `AMItems.get(tier)`.
pub fn cell_id(tier: Tier) -> String {
    format!("{NAMESPACE}:chemical_storage_cell_{}k", tier.kilobytes())
}
/// `AMItems.getPortableCell(tier)`.
pub fn portable_cell_id(tier: Tier) -> String {
    format!("{NAMESPACE}:portable_chemical_cell_{}k", tier.kilobytes())
}

/// `MekanismKey.of(chemical)`: a key is the chemical's id and nothing else.
pub fn chemical_key(id: &str) -> ItemKey {
    ItemKey { item_id: id.to_owned(), components: Vec::new() }
}

/// `MekanismKey.MAP_CODEC`: `id`, the chemical's registry id. Other fields are ignored, as a
/// record codec does.
fn key_decode(data: &Compound) -> Result<ItemKey, String> {
    let Some(Tag::String(id)) = data.get(&"id".into()) else { return Err("missing chemical id".into()) };
    let id = id.to_string().map_err(|_| "invalid chemical id")?;
    let id = if id.contains(':') { id } else { format!("minecraft:{id}") };
    if !valid_id(&id) || id == "mekanism:empty" {
        return Err("invalid chemical id".into());
    }
    Ok(chemical_key(&id))
}
fn key_encode(key: &ItemKey) -> Result<Compound, String> {
    if !valid_id(&key.item_id) || !key.components.is_empty() {
        return Err("not a chemical key".into());
    }
    let mut data = Compound::default();
    data.insert("id".into(), Tag::String(key.item_id.as_str().into()));
    Ok(data)
}
/// A resource location: one `:`, `[a-z0-9_.-]` before it and `[a-z0-9_./-]` after.
fn valid_id(id: &str) -> bool {
    let Some((namespace, path)) = id.split_once(':') else { return false };
    !namespace.is_empty()
        && !path.is_empty()
        && namespace.bytes().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-'))
        && path.bytes().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'/'))
}

/// `AEKeyTypes.register(MekanismKeyType.TYPE)`, once, with
/// `GenericSlotCapacities.register` beside it.
pub fn key_type() -> KeyType {
    static KIND: OnceLock<KeyType> = OnceLock::new();
    *KIND.get_or_init(|| {
        let def = KeyTypeDef {
            amount_per_operation: AMOUNT_PER_OPERATION,
            amount_per_byte: AMOUNT_PER_BYTE,
            amount_per_unit: AMOUNT_PER_UNIT,
            unit_symbol: Some(UNIT_SYMBOL),
            ..KeyTypeDef::new(KEY_TYPE, DESCRIPTION)
        };
        let behaviour = KeyBehaviour {
            codec: KeyCodec { decode: key_decode, encode: key_encode },
            icon: |_| ItemStack::new(ICON, 1),
            known: None,
            container: None,
        };
        let kind = ae2_key_types::register(def, behaviour).expect("Applied Mekanistics' key type registers");
        ae2_key_types::register_slot_capacity(kind, SLOT_CAPACITY);
        ae2_key_types::register_chemical_type(kind);
        ae2_key_types::register_sprite(kind, chemical_sprite);
        kind
    })
}

/// `ChemicalRenderer.drawInGui` (`AppliedMekanisticsClient`): the chemical's icon in its tint, from its declaration
/// (`data/<ns>/mekanism/chemical/<path>.json`: `texture`, `tint` as Java's `0xAARRGGBB`; docs/mekanism-chemicals.md),
/// as the GUI atlas copy `mekanism-content` makes of the icon (`<ns>:gui/chemical/<icon ns>/<icon path>`), which
/// Mekanism's own gauges draw. Read once a chemical; none without a declaration.
fn chemical_sprite(key: &ItemKey, read: &dyn Fn(&str) -> Option<Vec<u8>>) -> Option<(String, u32)> {
    static KNOWN: RwLock<Vec<(String, Option<(String, u32)>)>> = RwLock::new(Vec::new());
    if let Some((_, known)) = KNOWN.read().unwrap_or_else(|p| p.into_inner()).iter().find(|(id, _)| *id == key.item_id) {
        return known.clone();
    }
    let found = key.components.is_empty().then(|| declared_sprite(&key.item_id, read)).flatten();
    KNOWN.write().unwrap_or_else(|p| p.into_inner()).push((key.item_id.clone(), found.clone()));
    found
}

/// The sprite and tint a chemical's declaration gives.
fn declared_sprite(id: &str, read: &dyn Fn(&str) -> Option<Vec<u8>>) -> Option<(String, u32)> {
    let (ns, path) = id.split_once(':')?;
    if !valid_id(id) {
        return None;
    }
    let declared: serde_json::Value = serde_json::from_slice(&read(&format!("data/{ns}/mekanism/chemical/{path}.json"))?).ok()?;
    let texture = declared.get("texture")?.as_str()?;
    let (tns, tpath) = texture.split_once(':').unwrap_or(("minecraft", texture));
    let tint = match declared.get("tint") {
        Some(serde_json::Value::String(text)) => u32::from_str_radix(text.trim_start_matches("0x").trim_start_matches("0X"), 16).ok()?,
        Some(serde_json::Value::Number(n)) => n.as_i64()? as u32,
        _ => 0xffff_ffff,
    };
    Some((format!("{ns}:gui/chemical/{tns}/{tpath}"), tint))
}

/// The rows: the key type (registered here, after Applied Flux's, as ATM10's mod order has it),
/// `AMItems`' cells and portable cells, their drive models (`StorageCellModels.registerModel`)
/// and `initializeUpgrades`' cards: on a cell an Inverter, Equal Distribution and Void Card; on a
/// portable cell an Inverter Card and two Energy Cards.
pub fn register(defs: &mut Defs) {
    let kind = key_type();
    // AppliedMekanistics.initializeModels' StackWorldBehaviors: import, export, external storage
    // (no placement); the game asks `game.rs` through AE2's extension.
    add_key_strategies(kind, NAMESPACE, KeyStrategyKinds { import: true, export: true, external_storage: true, placement: false });
    for tier in TIERS {
        let model = format!("{NAMESPACE}:block/drive/cells/chemical_storage_cell_{}k", tier.kilobytes());
        let cell = CellDef {
            id: cell_id(tier),
            key_type: kind,
            tier,
            types: CELL_TYPES,
            // StorageTier.idleDrain: 0.5 AE/t per index.
            idle_drain: 0.5 * f64::from(tier.index()),
            portable: false,
            drive_model: model.clone(),
            battery_scale: 1.0,
            charge_scale: 1.0,
        };
        add_storage_cell_cards(defs, &cell);
        defs.add_cell(cell);
        let portable = CellDef {
            id: portable_cell_id(tier),
            key_type: kind,
            tier,
            types: PORTABLE_CELL_TYPES,
            // PortableCellItem: 0.5 AE/t whatever the tier.
            idle_drain: 0.5,
            portable: true,
            drive_model: model,
            battery_scale: 1.0,
            charge_scale: 1.0,
        };
        defs.add_upgrade("ae2:inverter_card", &portable.id, 1);
        defs.add_upgrade("ae2:energy_card", &portable.id, 2);
        defs.add_cell(portable);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chemical_key_type_has_applied_mekanistics_amounts_and_saved_form() {
        let kind = key_type();
        assert_eq!(kind.id(), KEY_TYPE);
        assert_eq!(KeyType::get(KEY_TYPE), Some(kind));
        let def = kind.def();
        assert_eq!((def.amount_per_operation, def.amount_per_byte, def.amount_per_unit), (125, 8000, 1000));
        assert_eq!(def.unit_symbol, Some("B"));
        assert_eq!(ae2_key_types::slot_capacity(kind), 4000);
        let codec = ae2_key_types::behaviour(kind).codec;
        let saved = (codec.encode)(&chemical_key("mekanism:hydrogen")).unwrap();
        assert_eq!(saved.get(&"id".into()), Some(&Tag::String("mekanism:hydrogen".into())));
        assert_eq!((codec.decode)(&saved).unwrap(), chemical_key("mekanism:hydrogen"));
        let mut bare = Compound::default();
        bare.insert("id".into(), Tag::String("oxygen".into()));
        assert_eq!((codec.decode)(&bare).unwrap().item_id, "minecraft:oxygen");
        assert!((codec.decode)(&Compound::default()).is_err());
        let mut empty = Compound::default();
        empty.insert("id".into(), Tag::String("mekanism:empty".into()));
        assert!((codec.decode)(&empty).is_err());
        assert!((codec.encode)(&ItemKey { item_id: "mekanism:hydrogen".into(), components: vec![1] }).is_err());
        assert_eq!((ae2_key_types::behaviour(kind).icon)(&chemical_key("mekanism:hydrogen")).item.as_ref(), ICON);
    }

    #[test]
    fn a_chemical_draws_its_declared_icon_in_its_tint_and_counts_as_fluids_do() {
        let kind = key_type();
        assert_eq!(ae2_key_types::chemical_type(), Some(kind));
        let read = |path: &str| {
            (path == "data/mekanism/mekanism/chemical/hydrogen.json")
                .then(|| br#"{"texture":"mekanism:liquid/liquid","tint":"0xFFFFFFFF","kind":"gas"}"#.to_vec())
        };
        let sprite = ae2_key_types::sprite(kind).unwrap();
        assert_eq!(sprite(&chemical_key("mekanism:hydrogen"), &read), Some(("mekanism:gui/chemical/mekanism/liquid/liquid".into(), 0xffff_ffff)));
        assert_eq!(declared_sprite("mekanism:oxygen", &read), None);
        // `AEKeyType.formatAmount`: buckets of 1000 mB, `B` in full, as AE2 shows a fluid.
        use ae2_cell_core::AmountFormat::{Full, Slot, SlotLargeFont};
        for amount in [0, 1, 250, 1000, 1500, 2000, 64_000, 1_234_567, 999_999_999] {
            for format in [Slot, SlotLargeFont, Full] {
                assert_eq!(kind.format_amount(amount, format), KeyType::FLUIDS.format_amount(amount, format), "{amount}");
            }
        }
        assert!(kind.format_amount(2000, Full).ends_with(" B"), "{}", kind.format_amount(2000, Full));
    }

    #[test]
    fn the_cells_hold_five_types_and_the_portable_ones_eighteen() {
        let mut defs = Defs::default();
        register(&mut defs);
        let kind = key_type();
        for (tier, drain) in TIERS.into_iter().zip([0.5, 1.0, 1.5, 2.0, 2.5]) {
            let cell = defs.cell(&cell_id(tier)).expect("cell");
            assert_eq!((cell.key_type, cell.types, cell.idle_drain, cell.portable), (kind, 5, drain, false));
            assert_eq!(cell.drive_model, format!("appmek:block/drive/cells/chemical_storage_cell_{}k", tier.kilobytes()));
            let config = cell.config();
            assert_eq!(config.total_bytes(), tier.kilobytes() * 1024);
            assert_eq!(config.bytes_per_type(), tier.kilobytes() * 8);
            let portable = defs.portable_cell(&portable_cell_id(tier)).expect("portable cell");
            assert_eq!((portable.key_type, portable.types, portable.idle_drain), (kind, 18, 0.5));
            for (card, max) in [("ae2:inverter_card", 1), ("ae2:equal_distribution_card", 1), ("ae2:void_card", 1), ("ae2:fuzzy_card", 0)] {
                assert_eq!(defs.upgrade_limit(card, &cell.id), max, "{card} on {}", cell.id);
            }
            for (card, max) in [("ae2:inverter_card", 1), ("ae2:energy_card", 2), ("ae2:void_card", 0)] {
                assert_eq!(defs.upgrade_limit(card, &portable.id), max, "{card} on {}", portable.id);
            }
            assert_eq!(defs.upgrade_slots(&cell.id), Some(3));
        }
        assert_eq!(defs.portable_cells().count(), 5);
    }

    #[test]
    fn the_rows_register_import_export_and_storage_bus_strategies() {
        register(&mut Defs::default());
        let kinds = KeyStrategyKinds { import: true, export: true, external_storage: true, placement: false };
        assert_eq!(horizon_client::game::ae2_ext::key_strategies(key_type()), Some((NAMESPACE, kinds)));
    }
}
