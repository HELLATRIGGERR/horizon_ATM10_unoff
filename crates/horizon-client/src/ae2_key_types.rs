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
    let (kind, id) = key.item_id.strip_prefix("horizon:key/")?.split_once('|')?;
    let kind = KeyType::get(kind).filter(|kind| !matches!(*kind, KeyType::ITEMS | KeyType::FLUIDS))?;
    Some(TypedKey::new(kind, ItemKey { item_id: id.to_owned(), components: key.components.clone() }))
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
}
