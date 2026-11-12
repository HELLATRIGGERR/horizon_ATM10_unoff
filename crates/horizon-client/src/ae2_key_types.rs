//! The engine's half of AE2's key-type registry, derived from AE2 1.21.1
//! (`AEKeyType.codec`, `AEKeyRendering`, `ContainerItemStrategies`;
//! LGPL-3.0-or-later). `ae2_cell_core::keys` holds each type's id and amounts;
//! this holds what needs the engine: how a key of the type is saved inside a
//! `GenericStack` (`#t` names the type), what item stands for it in a slot,
//! whether an id is one the game knows, and how items hold it
//! (`ContainerItemStrategy`). The game adds the strategies that need a world
//! (`game::ae2_key_strategies`).
//!
//! AE2's items and fluids are registered first, as `AppEngBase` does; an
//! addon calls [`register`] once, before its content is used.
use crate::items::ItemStack;
use crate::save;
use ae2_cell_core::ItemKey;
pub use ae2_cell_core::{AmountFormat, KeyType, KeyTypeDef, KeyTypeSet, TypedKey};
use horizon_world::nbt::{self, Compound, Limits, Tag};
use std::sync::RwLock;

/// `AEKeyType.codec()`: the key's own fields of a `GenericStack` map, apart
/// from `#t` and `#`.
#[derive(Clone, Copy, Debug)]
pub struct KeyCodec {
    pub decode: fn(&Compound) -> Result<ItemKey, String>,
    pub encode: fn(&ItemKey) -> Result<Compound, String>,
}
/// `ContainerItemStrategy` for a carried item, one item at a time. Each
/// returns the amount moved and the item after; none when the item is no
/// container of the type or nothing moves. The caller splits stacks and
/// commits the result: a call never changes the world.
#[derive(Clone, Copy, Debug)]
pub struct ContainerStrategy {
    /// `getContainedStack`: what one item holds.
    pub contained: fn(&ItemStack) -> Option<(ItemKey, u64)>,
    /// `insert`: fill one item with up to `amount` of the key.
    pub fill: fn(&ItemStack, &ItemKey, u64) -> Option<(u64, ItemStack)>,
    /// `extract`: drain up to `amount` of the key from one item.
    pub drain: fn(&ItemStack, &ItemKey, u64) -> Option<(u64, ItemStack)>,
}
/// What the engine needs of a key type besides its amounts.
#[derive(Clone, Copy, Debug)]
pub struct KeyBehaviour {
    pub codec: KeyCodec,
    /// `AEKeyRenderHandler`, for screens that draw items: the item an entry
    /// of the type shows as (a fluid's bucket).
    pub icon: fn(&ItemKey) -> ItemStack,
    /// Whether the game knows the key's id (an item the item registry has);
    /// an unknown one loads as Missing Content.
    pub known: Option<fn(&str) -> bool>,
    /// `ContainerItemStrategies.register`: none for items, whose items are
    /// themselves the stack.
    pub container: Option<ContainerStrategy>,
}

/// The id and components fields AE2's item and fluid keys share, and that an
/// addon type without a codec of its own uses: `id` (a resource location)
/// and optional `components`, kept as canonical network NBT bytes.
pub fn id_components_decode(data: &Compound) -> Result<ItemKey, String> {
    let Some(Tag::String(id)) = data.get(&"id".into()) else { return Err("missing generic stack id".into()) };
    let id = id.to_string().map_err(|_| "invalid generic stack id")?;
    let id = if id.contains(':') { id } else { format!("minecraft:{id}") };
    if !crate::ae2_provider_queue::valid_fluid_id(&id) || id == "minecraft:air" {
        return Err("invalid generic stack id".into());
    }
    let components = match data.get(&"components".into()) {
        None => Vec::new(),
        Some(Tag::Compound(map)) if map.is_empty() => Vec::new(),
        Some(Tag::Compound(map)) => nbt::write_network(&Tag::Compound(map.clone()), Limits::default()).map_err(|error| error.to_string())?,
        Some(_) => return Err("invalid generic component patch".into()),
    };
    Ok(ItemKey { item_id: id, components })
}
pub fn id_components_encode(key: &ItemKey) -> Result<Compound, String> {
    if !crate::ae2_provider_queue::valid_fluid_id(&key.item_id) || key.item_id == "minecraft:air" {
        return Err("invalid generic stack id".into());
    }
    let mut data = Compound::default();
    data.insert("id".into(), Tag::String(key.item_id.as_str().into()));
    if !key.components.is_empty() {
        let (tag, used) = nbt::read_network(&key.components, Limits::default()).map_err(|error| error.to_string())?;
        if used != key.components.len() || !matches!(tag, Tag::Compound(_)) {
            return Err("unrepresentable component patch".into());
        }
        data.insert("components".into(), tag);
    }
    Ok(data)
}

/// `AEItemKey.MAP_CODEC`: an item stack's fields without its count.
fn item_decode(data: &Compound) -> Result<ItemKey, String> {
    let Some(Tag::String(id)) = data.get(&"id".into()) else { return Err("missing generic stack id".into()) };
    let id = id.to_string().map_err(|_| "invalid generic stack id")?;
    let id = if id.contains(':') { id } else { format!("minecraft:{id}") };
    if !crate::ae2_provider_queue::valid_fluid_id(&id) || id == "minecraft:air" {
        return Err("invalid generic stack id".into());
    }
    let mut prototype = Compound::default();
    prototype.insert("id".into(), Tag::String(id.as_str().into()));
    prototype.insert("count".into(), Tag::Int(1));
    match data.get(&"components".into()) {
        None => {}
        Some(Tag::Compound(components)) => {
            prototype.insert("components".into(), Tag::Compound(components.clone()));
        }
        Some(_) => return Err("invalid generic component patch".into()),
    }
    let item = save::item_from_nbt(&Tag::Compound(prototype)).ok_or("invalid generic item")?;
    crate::ae2_cell_codec::key(&item)
}
fn item_encode(key: &ItemKey) -> Result<Compound, String> {
    let item = crate::ae2_cell_codec::prototype(key)?;
    let Tag::Compound(mut data) = save::item_to_nbt(&item) else { return Err("invalid generic item".into()) };
    data.remove(&"count".into());
    Ok(data)
}
fn item_icon(key: &ItemKey) -> ItemStack {
    crate::ae2_cell_codec::prototype(key).unwrap_or_else(|_| ItemStack::new("ae2:missing_content", 1))
}
/// `AEFluidKey.MAP_CODEC`: the shared fields.
fn fluid_encode(key: &ItemKey) -> Result<Compound, String> {
    id_components_encode(key).map_err(|error| if error == "unrepresentable component patch" { "unrepresentable fluid component patch".into() } else { error })
}
fn fluid_icon(key: &ItemKey) -> ItemStack {
    ItemStack::new(crate::ae2_cell_codec::fluid_bucket(&key.item_id).unwrap_or("minecraft:bucket"), 1)
}
/// `FluidContainerItemStrategy` over buckets: a filled bucket holds 1000 mB
/// of its fluid; an empty one takes 1000 mB of a fluid with a bucket. Fluid
/// tanks of other mods are the game's (`ae2_mod_fluids`).
fn bucket_contained(stack: &ItemStack) -> Option<(ItemKey, u64)> {
    let fluid = crate::ae2_cell_codec::bucket_fluid(&stack.item)?;
    Some((crate::ae2_cell_codec::fluid_key(fluid), 1000))
}
fn bucket_fill(stack: &ItemStack, key: &ItemKey, amount: u64) -> Option<(u64, ItemStack)> {
    if stack.item.as_ref() != "minecraft:bucket" || amount < 1000 || !key.components.is_empty() {
        return None;
    }
    let filled = crate::ae2_cell_codec::fluid_bucket(&key.item_id)?;
    Some((1000, ItemStack::new(filled, 1)))
}
fn bucket_drain(stack: &ItemStack, key: &ItemKey, amount: u64) -> Option<(u64, ItemStack)> {
    let (held, inside) = bucket_contained(stack)?;
    (held == *key && amount >= inside).then(|| (inside, ItemStack::new("minecraft:bucket", 1)))
}

static ITEMS: KeyBehaviour = KeyBehaviour {
    codec: KeyCodec { decode: item_decode, encode: item_encode },
    icon: item_icon,
    known: None,
    container: None,
};
static FLUIDS: KeyBehaviour = KeyBehaviour {
    codec: KeyCodec { decode: id_components_decode, encode: fluid_encode },
    icon: fluid_icon,
    known: None,
    container: Some(ContainerStrategy { contained: bucket_contained, fill: bucket_fill, drain: bucket_drain }),
};
/// Addon types' behaviour, by raw id from 2 on.
static ADDED: RwLock<Vec<Option<KeyBehaviour>>> = RwLock::new(Vec::new());

/// `AEKeyTypes.register` with the engine's half: the type's amounts, then
/// its codec, icon and container strategy. Registering the same definition
/// again returns its type and keeps the first behaviour.
pub fn register(def: KeyTypeDef, behaviour: KeyBehaviour) -> Result<KeyType, ae2_cell_core::keys::RegisterError> {
    // The commands' key type codec (horizon-command's `ae2_components`) asks this registry from now on.
    crate::ae2_components::install_key_types(|id| KeyType::get(id).is_some());
    let kind = ae2_cell_core::keys::register(def)?;
    let index = usize::from(kind.raw_id() - 2);
    let mut added = ADDED.write().unwrap_or_else(|poisoned| poisoned.into_inner());
    if added.len() <= index {
        added.resize(index + 1, None);
    }
    if added[index].is_none() {
        added[index] = Some(behaviour);
    }
    Ok(kind)
}
/// The behaviour of `kind`; an addon type registered with the core alone has
/// the id-and-components codec, its id as icon and no container strategy.
pub fn behaviour(kind: KeyType) -> KeyBehaviour {
    match kind {
        KeyType::ITEMS => ITEMS,
        KeyType::FLUIDS => FLUIDS,
        other => ADDED
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(usize::from(other.raw_id() - 2))
            .copied()
            .flatten()
            .unwrap_or(KeyBehaviour {
                codec: KeyCodec { decode: id_components_decode, encode: id_components_encode },
                icon: |key| ItemStack::new(key.item_id.as_str(), 1),
                known: None,
                container: None,
            }),
    }
}
/// Where the port names a key by an id alone (the terminal's listing, a
/// bus's keys): a key of an addon's type as `horizon:key/<type id>|<id>`,
/// its components its own. Items and fluids keep their own forms.
pub fn addon_marker(kind: KeyType, key: &ItemKey) -> ItemKey {
    ItemKey { item_id: format!("horizon:key/{}|{}", kind.id(), key.item_id), components: key.components.clone() }
}
/// The addon type's key an [`addon_marker`] names; none for any other id.
pub fn parse_addon_marker(key: &ItemKey) -> Option<TypedKey> {
    let (kind, id) = marker_parts(key)?;
    let kind = KeyType::get(kind).filter(|kind| !matches!(*kind, KeyType::ITEMS | KeyType::FLUIDS))?;
    Some(TypedKey::new(kind, ItemKey { item_id: id.to_owned(), components: key.components.clone() }))
}
/// The type id and key id of an [`addon_marker`], its type registered or
/// not: the port's own records save them apart (`#t` and the id), so a key
/// of a type no longer registered saves back as it was.
pub fn marker_parts(key: &ItemKey) -> Option<(&str, &str)> {
    key.item_id.strip_prefix("horizon:key/")?.split_once('|')
}
/// The marker of a key saved with its type's id ([`marker_parts`] back).
pub fn marker_by_id(kind: &str, id: &str, components: Vec<u8>) -> ItemKey {
    ItemKey { item_id: format!("horizon:key/{kind}|{id}"), components }
}
/// `ContainerItemStrategies.getContainedStack`: the first type, in
/// registration order, whose strategy finds something in `stack`.
pub fn contained_stack(stack: &ItemStack) -> Option<(TypedKey, u64)> {
    if stack.count == 0 {
        return None;
    }
    KeyType::all().into_iter().find_map(|kind| {
        let (key, amount) = (behaviour(kind).container?.contained)(stack)?;
        Some((TypedKey::new(kind, key), amount))
    })
}
/// `ContainerItemStrategies.isTypeSupported`.
pub fn has_container_strategy(kind: KeyType) -> bool {
    behaviour(kind).container.is_some()
}
/// `ContainerItemStrategies.register` for an addon's type registered before:
/// how items hold its keys. AE2 keeps these apart from the type's
/// registration, so an addon may add them later (Applied Flux's
/// `FEContainerItemStrategy` reads the game's item energy). The first one
/// stays.
pub fn register_container_strategy(kind: KeyType, strategy: ContainerStrategy) {
    if matches!(kind, KeyType::ITEMS | KeyType::FLUIDS) {
        return;
    }
    let mut added = ADDED.write().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(Some(behaviour)) = added.get_mut(usize::from(kind.raw_id() - 2))
        && behaviour.container.is_none()
    {
        behaviour.container = Some(strategy);
    }
}
/// `ContainerItemStrategies.getContainedStack` as the fluid lane names a key
/// (a fluid's own key, an addon type's [`addon_marker`]); none for items.
pub fn contained_lane(stack: &ItemStack) -> Option<(ItemKey, u64)> {
    let (typed, amount) = contained_stack(stack)?;
    match typed.key_type {
        KeyType::ITEMS => None,
        KeyType::FLUIDS => Some((typed.key, amount)),
        other => Some((addon_marker(other, &typed.key), amount)),
    }
}

/// `GenericSlotCapacities`: the most of a type's key one generic slot holds
/// (an ME Interface's, a Pattern Provider's return slot), registered by an
/// addon for its type (Applied Flux: 1,000,000 FE). AE2 registers its items
/// (99) and fluids (four buckets); a type nobody registers has no limit
/// (`GenericStackInv.getCapacity`: `Long.MAX_VALUE`).
static SLOT_CAPACITIES: RwLock<Vec<(KeyType, u64)>> = RwLock::new(Vec::new());
pub fn register_slot_capacity(kind: KeyType, capacity: u64) {
    let mut capacities = SLOT_CAPACITIES.write().unwrap_or_else(|poisoned| poisoned.into_inner());
    capacities.retain(|(registered, _)| *registered != kind);
    capacities.push((kind, capacity));
}
pub fn slot_capacity(kind: KeyType) -> u64 {
    match kind {
        KeyType::ITEMS => 99,
        KeyType::FLUIDS => 4000,
        other => SLOT_CAPACITIES
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .find(|(registered, _)| *registered == other)
            .map_or(i64::MAX as u64, |(_, capacity)| *capacity),
    }
}
/// The slot capacity of a key as the fluid lane names it: a fluid's, or the
/// type's of an addon marker (none for a type no longer registered, whose
/// stack is kept as it was).
pub fn lane_capacity(key: &ItemKey) -> u64 {
    match marker_parts(key) {
        None => slot_capacity(KeyType::FLUIDS),
        Some((kind, _)) => KeyType::get(kind).map_or(i64::MAX as u64, slot_capacity),
    }
}
/// The port's saved generic slot of the fluid lane: the key's id under
/// `field` and its `components`; an addon type's key keeps its own id there
/// with the type's id in `#t`, as a `GenericStack` names it. None when the
/// entry has no such key.
pub fn lane_entry_key(entry: &Tag, field: &str) -> Option<ItemKey> {
    let (Some(Tag::String(id)), Some(Tag::ByteArray(components))) = (entry.get(field), entry.get("components")) else {
        return None;
    };
    let id = id.to_string().ok()?;
    match entry.get("#t") {
        None => Some(ItemKey { item_id: id, components: components.clone() }),
        Some(Tag::String(kind)) => Some(marker_by_id(&kind.to_string().ok()?, &id, components.clone())),
        Some(_) => None,
    }
}
/// [`lane_entry_key`] back: the fields that save `key`.
pub fn lane_entry_fields(key: &ItemKey, field: &'static str) -> Vec<(&'static str, Tag)> {
    let (kind, id) = match marker_parts(key) {
        Some((kind, id)) => (Some(kind), id),
        None => (None, key.item_id.as_str()),
    };
    let mut fields = vec![(field, Tag::String(id.into())), ("components", Tag::ByteArray(key.components.clone()))];
    fields.extend(kind.map(|kind| ("#t", Tag::String(kind.into()))));
    fields
}
/// Whether the game knows `id` as a key of `kind`: the item registry's
/// answer for items (the caller's `items`), the type's own for an addon's,
/// and always for fluids (an unregistered fluid stays stored as it is).
pub fn known_id(kind: KeyType, id: &str, items: impl Fn(&str) -> bool) -> bool {
    match kind {
        KeyType::ITEMS => items(id),
        KeyType::FLUIDS => true,
        other => behaviour(other).known.is_none_or(|known| known(id)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ae2_generic_stack::{self as generic, GenericStack};
    use ae2_cell_core::{Action, Cell, CellConfig, Tier};

    /// A third key type only these tests register, as an addon would: its own
    /// amounts, unit and icon. Registering it again is the same type.
    fn spark() -> KeyType {
        let def = KeyTypeDef {
            amount_per_operation: 50,
            amount_per_byte: 1000,
            amount_per_unit: 100,
            unit_symbol: Some("Sp"),
            ..KeyTypeDef::new("keytypes_test:spark", "gui.keytypes_test.sparks")
        };
        let behaviour = KeyBehaviour {
            codec: KeyCodec { decode: id_components_decode, encode: id_components_encode },
            icon: |_| ItemStack::new("minecraft:glowstone_dust", 1),
            known: Some(|id| id.starts_with("keytypes_test:")),
            container: None,
        };
        register(def, behaviour).unwrap()
    }
    fn spark_key(name: &str) -> ItemKey {
        ItemKey { item_id: format!("keytypes_test:{name}"), components: Vec::new() }
    }
    /// An item through its whole saved path: NBT, network bytes and back.
    fn saved(stack: &ItemStack) -> ItemStack {
        let bytes = nbt::write_network(&save::item_to_nbt(stack), Limits::default()).unwrap();
        save::item_from_nbt(&nbt::read_network(&bytes, Limits::default()).unwrap().0).unwrap()
    }

    #[test]
    fn an_addon_key_type_flows_through_a_cell_and_its_saved_form() {
        let kind = spark();
        assert_eq!(spark(), kind, "registered once");
        assert!(KeyType::all().contains(&kind) && kind.raw_id() >= 2);
        assert_eq!((behaviour(kind).icon)(&spark_key("red")).item.as_ref(), "minecraft:glowstone_dust");
        // A 1K cell of the type: 1,024 bytes, 8 a type, 1,000 sparks a byte.
        let mut cell = Cell::new(CellConfig::basic(Tier::K1, kind, 7));
        let red = spark_key("red");
        assert_eq!(cell.insert(&red, 2_000_000, Action::Simulate), 1_016_000);
        assert_eq!(cell.insert(&red, 1_500, Action::Modulate), 1_500);
        assert_eq!(cell.used_bytes(), 8 + 2, "1,500 sparks take two bytes");
        assert_eq!(cell.extract(&red, 400, Action::Modulate), 400);
        assert_eq!(cell.count(&red), 1_100);
        // AE2's form names the type in `#t`, and a stored item of another
        // type stays extractable beside it.
        let stone = crate::ae2_cell_codec::key(&ItemStack::new("minecraft:stone", 1)).unwrap();
        let mut source = crate::ae2_cell_codec::cell_source(&cell).unwrap();
        source.push(generic::encode(&GenericStack::item(stone.clone(), 3)).unwrap());
        assert_eq!(source[0], Tag::compound([("#t", Tag::String("keytypes_test:spark".into())), ("id", Tag::String("keytypes_test:red".into())), ("#", Tag::Long(1_100))]));
        let mut holder = ItemStack::new("ae2:item_storage_cell_1k", 1);
        holder.data_mut().set_other("ae2:storage_cell_inv", Some(Tag::List(source.clone())));
        let restored = saved(&holder);
        let list = restored.data().unwrap().other("ae2:storage_cell_inv").unwrap().clone();
        let known = |id: &str, kind: KeyType| known_id(kind, id, |_| true);
        let mut cell = crate::ae2_cell_codec::cell_from_source(CellConfig::basic(Tier::K1, kind, 7), &list, &known).unwrap();
        assert_eq!(cell.count(&red), 1_100);
        assert_eq!(cell.other_entries().collect::<Vec<_>>(), vec![(KeyType::ITEMS, &stone, &3)]);
        assert_eq!(cell.extract_of(KeyType::ITEMS, &stone, 3, Action::Modulate), 3);
        assert_eq!(cell.insert(&stone, 1, Action::Simulate), 1, "the same key bytes, but this type's");
        // A wrapped stack of the type, as menus carry it.
        let wrapped = generic::wrap(&GenericStack { key: red.clone(), key_type: kind, amount: 250 }).unwrap();
        let unwrapped = generic::unwrap_item_stack(&saved(&wrapped)).unwrap().unwrap();
        assert_eq!((unwrapped.key_type, unwrapped.key, unwrapped.amount), (kind, red.clone(), 250));
        // Its amounts in its units.
        assert_eq!(kind.format_amount(1_250, AmountFormat::Full), "12.5 Sp");
        assert_eq!(kind.format_amount(1_250, AmountFormat::Slot), "12.5");
        // Its own `known`: an id it does not know is Missing Content.
        let foreign = Tag::compound([("#t", Tag::String("keytypes_test:spark".into())), ("id", Tag::String("other:thing".into())), ("#", Tag::Long(9))]);
        let missing = generic::decode_tolerant_checked(&foreign, known).unwrap().unwrap();
        assert_eq!((missing.key_type, missing.key.item_id.as_str()), (KeyType::ITEMS, "ae2:missing_content"));
        assert_eq!(generic::encode(&missing).unwrap(), foreign, "saved back as it was");
        // Its marker, where the port names a key by an id.
        let marker = addon_marker(kind, &red);
        assert_eq!(parse_addon_marker(&marker), Some(TypedKey::new(kind, red)));
        assert_eq!(parse_addon_marker(&spark_key("plain")), None);
        assert_eq!(marker_parts(&marker), Some(("keytypes_test:spark", "keytypes_test:red")));
        assert_eq!(marker_by_id("keytypes_test:spark", "keytypes_test:red", Vec::new()), marker);
        let gone = marker_by_id("removed:energy", "removed:fe", Vec::new());
        assert_eq!((marker_parts(&gone), parse_addon_marker(&gone)), (Some(("removed:energy", "removed:fe")), None));
    }

    #[test]
    fn saved_key_types_round_trip_and_an_unknown_one_is_kept_byte_for_byte() {
        let water = crate::ae2_cell_codec::fluid_key("minecraft:water");
        let stone = crate::ae2_cell_codec::key(&ItemStack::new("minecraft:stone", 1)).unwrap();
        let red = spark_key("red");
        let stacks = [GenericStack::item(stone.clone(), 64), GenericStack::fluid(water.clone(), 1_500), GenericStack { key: red, key_type: spark(), amount: 7 }];
        for stack in &stacks {
            let tag = generic::encode(stack).unwrap();
            assert_eq!(tag.get("#t"), Some(&Tag::String(stack.key_type.id().into())));
            let bytes = nbt::write_network(&tag, Limits::default()).unwrap();
            let back = generic::decode(&nbt::read_network(&bytes, Limits::default()).unwrap().0).unwrap().unwrap();
            assert_eq!(&back, stack);
            assert_eq!(generic::encode(&back).unwrap(), tag);
        }
        // A world saved with an addon's type, loaded without the addon: the
        // stack is Missing Content that saves back unchanged (AE2's codec).
        let gone = Tag::compound([("#t", Tag::String("removed:energy".into())), ("id", Tag::String("removed:fe".into())), ("#", Tag::Long(12))]);
        let missing = generic::decode_tolerant(&gone).unwrap().unwrap();
        assert_eq!((missing.key_type, missing.key.item_id.as_str(), missing.amount), (KeyType::ITEMS, "ae2:missing_content", 12));
        assert_eq!(generic::encode(&missing).unwrap(), gone);
        // In a cell, beside an item and a fluid, through the item's own save.
        let mut cell = ItemStack::new("ae2:item_storage_cell_1k", 1);
        let source: Vec<Tag> = [generic::encode(&stacks[0]).unwrap(), generic::encode(&stacks[1]).unwrap(), gone.clone()].into();
        cell.data_mut().set_other("ae2:storage_cell_inv", Some(Tag::List(source.clone())));
        let mut contents = crate::ae2_cell_codec::read_cell(&saved(&cell)).unwrap();
        assert_eq!(contents.count(&stone), 64);
        assert_eq!(contents.other_count(KeyType::FLUIDS, &water), 1_500);
        assert_eq!(contents.insert(&stone, 1, Action::Modulate), 1);
        crate::ae2_cell_codec::write_cell(&mut cell, &contents).unwrap();
        let Some(Tag::List(entries)) = saved(&cell).data().and_then(|data| data.other("ae2:storage_cell_inv")).cloned() else { panic!("no cell inventory") };
        assert!(entries.iter().any(|entry| entry.get("id") == Some(&Tag::String("minecraft:stone".into())) && entry.get("#") == Some(&Tag::Long(65))));
        assert!(entries.contains(&source[1]), "the fluid stays as saved");
        assert!(entries.contains(&gone), "the unknown type's stack stays as saved");
    }

    #[test]
    fn built_in_behaviour_is_items_and_buckets() {
        let water = crate::ae2_cell_codec::fluid_key("minecraft:water");
        let bucket = ItemStack::new("minecraft:water_bucket", 1);
        assert_eq!(contained_stack(&bucket), Some((TypedKey::fluid(water.clone()), 1000)));
        assert_eq!(contained_stack(&ItemStack::new("minecraft:stone", 1)), None);
        let (moved, filled) = (FLUIDS.container.unwrap().fill)(&ItemStack::new("minecraft:bucket", 1), &water, 1000).unwrap();
        assert_eq!((moved, filled.item.as_ref()), (1000, "minecraft:water_bucket"));
        assert!((FLUIDS.container.unwrap().fill)(&ItemStack::new("minecraft:bucket", 1), &water, 999).is_none());
        let (moved, empty) = (FLUIDS.container.unwrap().drain)(&bucket, &water, 1000).unwrap();
        assert_eq!((moved, empty.item.as_ref()), (1000, "minecraft:bucket"));
        assert!(has_container_strategy(KeyType::FLUIDS) && !has_container_strategy(KeyType::ITEMS));
        assert_eq!(fluid_icon(&water).item.as_ref(), "minecraft:water_bucket");
        assert!(known_id(KeyType::FLUIDS, "removed:goo", |_| false));
        assert!(!known_id(KeyType::ITEMS, "removed:ore", |_| false));
    }

    #[test]
    fn an_addon_types_slots_and_containers_are_its_own() {
        let kind = spark();
        let marker = addon_marker(kind, &spark_key("red"));
        // GenericSlotCapacities: AE2's two, none for a type nobody registers, then the addon's.
        assert_eq!((slot_capacity(KeyType::ITEMS), slot_capacity(KeyType::FLUIDS)), (99, 4000));
        let water = ItemKey { item_id: "minecraft:water".into(), components: Vec::new() };
        assert_eq!(lane_capacity(&water), 4000);
        assert_eq!(lane_capacity(&marker_by_id("removed:type", "removed:key", Vec::new())), i64::MAX as u64);
        register_slot_capacity(kind, 25_000);
        assert_eq!(lane_capacity(&marker), 25_000);
        // A lane slot saves an addon key's own id with its type in `#t`, a fluid as before.
        for key in [marker.clone(), water] {
            let mut fields = vec![("Slot", Tag::Byte(3))];
            fields.extend(lane_entry_fields(&key, "fluid"));
            let entry = Tag::compound(fields);
            assert_eq!(entry.get("#t").is_some(), key == marker);
            assert_eq!(lane_entry_key(&entry, "fluid"), Some(key));
        }
        // ContainerItemStrategies.register after the type: the first strategy stays.
        fn holds(stack: &ItemStack) -> Option<(ItemKey, u64)> {
            (stack.item.as_ref() == "minecraft:glowstone").then(|| (ItemKey { item_id: "keytypes_test:red".into(), components: Vec::new() }, 400))
        }
        register_container_strategy(kind, ContainerStrategy { contained: holds, fill: |_, _, _| None, drain: |_, _, _| None });
        register_container_strategy(kind, ContainerStrategy { contained: |_| None, fill: |_, _, _| None, drain: |_, _, _| None });
        assert!(has_container_strategy(kind));
        assert_eq!(contained_lane(&ItemStack::new("minecraft:glowstone", 1)), Some((marker, 400)));
        assert_eq!(contained_lane(&ItemStack::new("minecraft:water_bucket", 1)).map(|(key, n)| (key.item_id, n)), Some(("minecraft:water".into(), 1000)));
        assert_eq!(contained_lane(&ItemStack::new("minecraft:stone", 1)), None);
    }
}
