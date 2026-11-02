//! Content mods in the resource root (docs/modpacks.md, registries 0.2).
//!
//! Every loader reads one resource root, and most of its block and item
//! tables are exports from vanilla indexed by block-state id or keyed by
//! name. A content mod's block copies a vanilla *template*. So the mods'
//! root is an overlay, like the resource-pack one:
//! - the base root linked, with the per-state and per-name tables
//!   rewritten: each mod block gets its template's rows appended (states
//!   after vanilla's, mods and blocks in id order) with names, translation
//!   keys and in-table state ids moved over, and the declared overrides
//!   applied;
//! - each mod's own `assets/` and `data/` files;
//! - additions to tags merged into the tag files and bundles.
//!
//! The loaders need no mod code, vanilla state ids never move, and with no
//! content mod the root is the base root itself.
use crate::{Result, blocks::MAP_COLORS};
use horizon_core::geometry::{Aabb, Axis, ShapeOp, Vec3, VoxelShape};
use horizon_mods::content::{
    Block, Content, CreativeTab, EntityType, MenuType, MobEffect, Rarity, StateProperty,
};
use horizon_world::nbt::{self, Compound, JavaString, Limits, Tag};
use serde::{Deserialize, Serialize, de};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt, fs,
    io::Write,
    ops::Range,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

/// A content mod to put in the root: its id and content.
pub struct ContentMod {
    pub id: String,
    pub content: Content,
}

/// Leaves out a mod with an item whose `crafting_remainder` names an item the mod does not have:
/// a file item that named a code-declared one the declaration check then refused. Left in, the
/// overlay would fail on it and no content mod at all would load. Returns what was left out.
pub fn drop_dangling_remainders(mods: &mut Vec<ContentMod>) -> Vec<String> {
    let mut dropped = Vec::new();
    mods.retain(|m| {
        // The mod's items and its blocks that have an item (`Content::read`'s own rule).
        let ids: BTreeSet<&str> = m
            .content
            .items
            .iter()
            .map(|i| i.id.as_str())
            .chain(
                m.content
                    .blocks
                    .iter()
                    .filter(|b| b.item)
                    .map(|b| b.id.as_str()),
            )
            .collect();
        let dangling = m.content.items.iter().find(|i| {
            i.crafting_remainder
                .as_deref()
                .is_some_and(|r| !r.starts_with("minecraft:") && !ids.contains(r))
        });
        if let Some(item) = dangling {
            dropped.push(format!(
                "mod {} content not loaded: {}: crafting_remainder {:?} is not one of its items",
                m.id,
                item.id,
                item.crafting_remainder.as_deref().unwrap_or_default()
            ));
        }
        dangling.is_none()
    });
    dropped
}

/// The mods' mob effects in registry order: by id, as items are. Their
/// network ids follow vanilla's in this order.
pub fn mob_effects(mods: &[ContentMod]) -> Vec<&MobEffect> {
    let mut effects: Vec<&MobEffect> = mods.iter().flat_map(|m| &m.content.effects).collect();
    effects.sort_by(|a, b| a.id.cmp(&b.id));
    effects
}

/// the mods' entity types in registry order, by id as effects are.
pub fn entity_types(mods: &[ContentMod]) -> Vec<&EntityType> {
    let mut types: Vec<&EntityType> = mods.iter().flat_map(|m| &m.content.entity_types).collect();
    types.sort_by(|a, b| a.id.cmp(&b.id));
    types
}

/// the mods' menu types in registry order, by id as effects are.
pub fn menu_types(mods: &[ContentMod]) -> Vec<&MenuType> {
    let mut menus: Vec<&MenuType> = mods.iter().flat_map(|m| &m.content.menu_types).collect();
    menus.sort_by(|a, b| a.id.cmp(&b.id));
    menus
}

/// JSON with its object keys in file order, so a rewritten table differs
/// from the original only by what was added.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum J {
    Null,
    Bool(bool),
    Num(serde_json::Number),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}
impl<'de> Deserialize<'de> for J {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> de::Visitor<'de> for V {
            type Value = J;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("JSON")
            }
            fn visit_unit<E>(self) -> std::result::Result<J, E> {
                Ok(J::Null)
            }
            fn visit_bool<E>(self, v: bool) -> std::result::Result<J, E> {
                Ok(J::Bool(v))
            }
            fn visit_i64<E>(self, v: i64) -> std::result::Result<J, E> {
                Ok(J::Num(v.into()))
            }
            fn visit_u64<E>(self, v: u64) -> std::result::Result<J, E> {
                Ok(J::Num(v.into()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<J, E> {
                serde_json::Number::from_f64(v)
                    .map(J::Num)
                    .ok_or_else(|| E::custom("not a JSON number"))
            }
            fn visit_str<E>(self, v: &str) -> std::result::Result<J, E> {
                Ok(J::Str(v.to_owned()))
            }
            fn visit_string<E>(self, v: String) -> std::result::Result<J, E> {
                Ok(J::Str(v))
            }
            fn visit_seq<A: de::SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<J, A::Error> {
                let mut out = Vec::new();
                while let Some(v) = a.next_element()? {
                    out.push(v);
                }
                Ok(J::Arr(out))
            }
            fn visit_map<A: de::MapAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<J, A::Error> {
                let mut out = Vec::new();
                while let Some(kv) = a.next_entry()? {
                    out.push(kv);
                }
                Ok(J::Obj(out))
            }
        }
        d.deserialize_any(V)
    }
}
impl Serialize for J {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::{SerializeMap, SerializeSeq};
        match self {
            J::Null => s.serialize_unit(),
            J::Bool(b) => s.serialize_bool(*b),
            J::Num(n) => n.serialize(s),
            J::Str(t) => s.serialize_str(t),
            J::Arr(a) => {
                let mut seq = s.serialize_seq(Some(a.len()))?;
                for v in a {
                    seq.serialize_element(v)?;
                }
                seq.end()
            }
            J::Obj(o) => {
                let mut map = s.serialize_map(Some(o.len()))?;
                for (k, v) in o {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
        }
    }
}
impl J {
    fn get(&self, key: &str) -> Option<&J> {
        match self {
            J::Obj(o) => o.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    fn remove(&mut self, key: &str) {
        if let J::Obj(o) = self {
            o.retain(|(k, _)| k != key);
        }
    }
    fn get_mut(&mut self, key: &str) -> Option<&mut J> {
        match self {
            J::Obj(o) => o.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    /// Sets `key`, in place if present, else at the end.
    fn set(&mut self, key: &str, value: J) {
        if let Some(v) = self.get_mut(key) {
            *v = value;
        } else if let J::Obj(o) = self {
            o.push((key.to_owned(), value));
        }
    }
    /// The value at a `/`-separated path ("" is the document).
    fn at(&mut self, path: &str) -> Option<&mut J> {
        path.split('/')
            .filter(|p| !p.is_empty())
            .try_fold(self, |j, key| j.get_mut(key))
    }
    fn f64(v: f64) -> J {
        serde_json::Number::from_f64(v).map_or(J::Null, J::Num)
    }
    fn as_u32(&self) -> Option<u32> {
        match self {
            J::Num(n) => n.as_u64().and_then(|v| u32::try_from(v).ok()),
            _ => None,
        }
    }
    /// Renames strings and, with `ids`, moves state ids, everywhere inside.
    fn rename(&mut self, r: &Rename, ids: bool) {
        match self {
            J::Str(s) => {
                if let Some(new) = r.string(s) {
                    *s = new;
                }
            }
            J::Num(n) if ids => {
                if let Some(new) = n
                    .as_u64()
                    .and_then(|v| u32::try_from(v).ok())
                    .and_then(|id| r.id(id))
                {
                    *n = new.into();
                }
            }
            J::Arr(a) => a.iter_mut().for_each(|v| v.rename(r, ids)),
            J::Obj(o) => o.iter_mut().for_each(|(_, v)| v.rename(r, ids)),
            _ => {}
        }
    }
}

/// A [`J::Obj`] with its keys indexed: [`J::get`] and [`J::set`] scan, which is
/// quadratic for a table gaining tens of thousands of entries.
struct Keyed<'a> {
    entries: &'a mut Vec<(String, J)>,
    index: HashMap<String, usize>,
}
impl<'a> Keyed<'a> {
    fn new(map: &'a mut J) -> Option<Self> {
        let J::Obj(entries) = map else {
            return None;
        };
        // As `J::get`, the first of a repeated key.
        let index = entries
            .iter()
            .enumerate()
            .rev()
            .map(|(i, (key, _))| (key.clone(), i))
            .collect();
        Some(Keyed { entries, index })
    }
    fn get(&self, key: &str) -> Option<&J> {
        self.index.get(key).map(|&i| &self.entries[i].1)
    }
    fn get_mut(&mut self, key: &str) -> Option<&mut J> {
        self.index.get(key).map(|&i| &mut self.entries[i].1)
    }
    /// As [`J::set`].
    fn set(&mut self, key: &str, value: J) {
        match self.index.get(key) {
            Some(&i) => self.entries[i].1 = value,
            None => {
                self.index.insert(key.to_owned(), self.entries.len());
                self.entries.push((key.to_owned(), value));
            }
        }
    }
}

/// `f` over `items` on up to `threads` threads; the results in `items`' order.
pub fn par_map<T: Sync, R: Send>(
    items: &[T],
    threads: usize,
    f: impl Fn(&T) -> R + Sync,
) -> Vec<R> {
    let available = std::thread::available_parallelism().map_or(1, |n| n.get());
    let threads = threads.min(available).min(items.len()).max(1);
    let next = AtomicUsize::new(0);
    let worker = || {
        let mut done = Vec::new();
        loop {
            let i = next.fetch_add(1, Ordering::Relaxed);
            let Some(item) = items.get(i) else {
                return done;
            };
            done.push((i, f(item)));
        }
    };
    let mut done: Vec<(usize, R)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads).map(|_| scope.spawn(&worker)).collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().unwrap_or_else(|e| std::panic::resume_unwind(e)))
            .collect()
    });
    done.sort_by_key(|(i, _)| *i);
    done.into_iter().map(|(_, r)| r).collect()
}

/// One id moving to another: `minecraft:iron_block` → `ruby:ruby_block`,
/// with its state range for blocks.
#[derive(Clone)]
struct Rename {
    from: String,
    to: String,
    /// The ids as they are spelled in translation keys, `<ns>.<path>`.
    from_key: String,
    to_key: String,
    states: Range<u32>,
    first: u32,
    /// States this block gives each of its template's: one per combination
    /// of the state properties it declares (1 when it declares none). A
    /// template state moves to the first of them, which is the one every
    /// declared property is at its default for.
    fanout: u32,
    /// The lookup from a template state and a declared combination to the state's row, as an
    /// offset from `first`: `rows[template offset * fanout + combination]`, laid out as vanilla
    /// does (`state_order`). Empty: the template's states times the combinations, in that order.
    rows: Vec<u32>,
}
impl Rename {
    fn new(from: &str, to: &str) -> Self {
        let key = |id: &str| id.replacen(':', ".", 1).replace('/', ".");
        Rename {
            from: from.to_owned(),
            to: to.to_owned(),
            from_key: key(from),
            to_key: key(to),
            states: 0..0,
            first: 0,
            fanout: 1,
            rows: Vec::new(),
        }
    }
    /// The id itself, `Block{id}`, and `block.<ns>.<path>` / `item.<ns>.<path>` keys.
    fn string(&self, s: &str) -> Option<String> {
        if s == self.from {
            return Some(self.to.clone());
        }
        if s.strip_prefix("Block{").and_then(|x| x.strip_suffix('}')) == Some(&self.from) {
            return Some(format!("Block{{{}}}", self.to));
        }
        for kind in ["block", "item"] {
            if s.strip_prefix(kind).and_then(|x| x.strip_prefix('.')) == Some(&self.from_key) {
                return Some(format!("{kind}.{}", self.to_key));
            }
        }
        None
    }
    /// The row of a template state's `combination`, as an offset from `first`.
    fn row(&self, at: u32, combination: u32) -> u32 {
        let index = at * self.fanout + combination;
        self.rows.get(index as usize).copied().unwrap_or(index)
    }
    /// Where a template state moves: its combination with every declared property at its default.
    fn id(&self, id: u32) -> Option<u32> {
        self.states
            .contains(&id)
            .then(|| self.first + self.row(id - self.states.start, 0))
    }
}

/// A template's own state properties, sorted by name, each with how many values it takes across
/// the template's `states`: how `StateTable::new` reads them back, the first name slowest.
fn template_properties<'a>(
    states: &Range<u32>,
    properties: impl Fn(u32) -> Option<&'a J>,
) -> Vec<(String, u32)> {
    let Some(J::Obj(own)) = states.clone().next().and_then(&properties) else {
        return Vec::new();
    };
    let mut names: Vec<&str> = own.iter().map(|(k, _)| k.as_str()).collect();
    names.sort_unstable();
    let mut out = Vec::new();
    for name in names {
        let mut seen: Vec<&J> = Vec::new();
        for id in states.clone() {
            if let Some(v) = properties(id).and_then(|p| p.get(name))
                && !seen.contains(&v)
            {
                seen.push(v);
            }
        }
        out.push((name.to_owned(), (seen.len() as u32).max(1)));
    }
    out
}

/// A block's states: in vanilla's order when the template's own properties multiply out to its
/// states (every vanilla block's do), else the template's states times the combinations.
fn block_order<'a>(
    states: &Range<u32>,
    properties: impl Fn(u32) -> Option<&'a J>,
    declared: &[(String, u32)],
    combinations: u32,
) -> Vec<(u32, u32)> {
    let template = template_properties(states, properties);
    if template.iter().map(|p| p.1).product::<u32>() == states.len() as u32 {
        return state_order(&template, declared);
    }
    (0..states.len() as u32)
        .flat_map(|state| (0..combinations).map(move |combo| (state, combo)))
        .collect()
}

/// The states of a block in vanilla's order (`StateDefinition`): every property, the template's
/// and the declared ones, sorted by name, the first name varying slowest. Each entry is a
/// template state's offset and the index of a declared combination (`combinations`), whose own
/// order is by declared name, the first slowest. With every declared name after the template's
/// this is the template's states times the combinations, as before.
fn state_order(template: &[(String, u32)], declared: &[(String, u32)]) -> Vec<(u32, u32)> {
    // (name, values, its own side's stride, from the template)
    let mut all: Vec<(&str, u32, u32, bool)> = Vec::new();
    for (side, from_template) in [(template, true), (declared, false)] {
        let mut stride = 1;
        let mut strides = Vec::new();
        for (_, len) in side.iter().rev() {
            strides.push(stride);
            stride *= len;
        }
        for ((name, len), stride) in side.iter().zip(strides.into_iter().rev()) {
            all.push((name, *len, stride, from_template));
        }
    }
    all.sort_by(|a, b| a.0.cmp(b.0));
    let total: u32 = all.iter().map(|p| p.1).product();
    (0..total)
        .map(|position| {
            let (mut state, mut combo, mut rest) = (0, 0, total);
            for &(_, len, stride, from_template) in &all {
                rest /= len;
                let value = (position / rest) % len;
                if from_template {
                    state += value * stride;
                } else {
                    combo += value * stride;
                }
            }
            (state, combo)
        })
        .collect()
}

/// The combinations of a block's declared state properties, in a fixed
/// order: by name (a `BTreeMap`'s), each one's declared default first, and
/// the first name varying slowest. So the first combination is every
/// property at its default, which is the one a template state maps to.
fn combinations(
    declared: &BTreeMap<String, StateProperty>,
    default_state: &BTreeMap<String, String>,
) -> Result<Vec<Vec<(String, String)>>> {
    let mut orders: Vec<(&str, Vec<&str>)> = Vec::new();
    for (name, property) in declared {
        let mut values = property.values();
        if let Some(default) = default_state.get(name) {
            let Some(at) = values.iter().position(|v| v == default) else {
                return Err(
                    format!("{name}: default_state {default:?} is not one of its values").into(),
                );
            };
            values.rotate_left(at);
        }
        orders.push((name, values));
    }
    // Every combination is a state of each of the template's, so a block's run
    // is its template's states times this many: refused before it is built.
    let mut product: u64 = 1;
    for (_, values) in &orders {
        product = product.saturating_mul(values.len() as u64);
    }
    if product > MAX_STATES as u64 {
        return Err(format!(
            "{product} state property combinations, more than the {MAX_STATES} a world can hold"
        )
        .into());
    }
    let mut combos: Vec<Vec<(String, String)>> = vec![Vec::new()];
    for (name, values) in orders {
        let mut next = Vec::with_capacity(combos.len().saturating_mul(values.len()));
        for combo in &combos {
            for value in &values {
                let mut c = combo.clone();
                c.push((name.to_owned(), (*value).to_owned()));
                next.push(c);
            }
        }
        combos = next;
    }
    Ok(combos)
}

/// Puts a declared state property's values on one of a block's state rows.
/// Only `facts.json` keeps a state's own row, whose fifth field is
/// `{"id": …, "properties": {…}}`; the other files' rows are per block, so
/// they have no property map to extend.
fn set_state_properties(row: &mut J, combo: &[(String, String)]) {
    let J::Arr(parts) = row else {
        return;
    };
    let Some(J::Obj(state)) = parts.get_mut(4) else {
        return;
    };
    if !state.iter().any(|(k, _)| k == "properties") {
        state.push(("properties".into(), J::Obj(Vec::new())));
    }
    let Some((_, properties)) = state.iter_mut().find(|(k, _)| k == "properties") else {
        return;
    };
    for (name, value) in combo {
        properties.set(name, J::Str(value.clone()));
    }
}

/// The same for an NBT table's state, which spells a state the way
/// `BlockState::to_nbt` does: `{id: "…", properties: {"<name>": "<value>"}}`,
/// with the compound left out when there are no properties.
fn set_nbt_properties(state: &mut Tag, combo: &[(String, String)]) {
    if combo.is_empty() {
        return;
    }
    let Tag::Compound(c) = state else {
        return;
    };
    let properties = c
        .entry(JavaString::from("properties"))
        .or_insert_with(|| Tag::Compound(Compound::new()));
    let Tag::Compound(properties) = properties else {
        return;
    };
    for (name, value) in combo {
        properties.insert(
            JavaString::from(name.as_str()),
            Tag::String(JavaString::from(value.as_str())),
        );
    }
}

// DiscreteVoxelShape.forAllEdges: merge consecutive boundary edges.
fn shape_edges(shape: &VoxelShape) -> Vec<J> {
    let sizes = shape.sizes();
    let mut result = Vec::new();
    for axis in [2, 0, 1] {
        let a = (axis + 1) % 3;
        let b = (axis + 2) % 3;
        for u in 0..=sizes[a] {
            for v in 0..=sizes[b] {
                let mut start = None;
                for t in 0..=sizes[axis] {
                    let mut count = 0;
                    let mut odd = 0;
                    for du in 0..=1 {
                        for dv in 0..=1 {
                            let mut p = [0i32; 3];
                            p[axis] = t as i32;
                            p[a] = u as i32 + du - 1;
                            p[b] = v as i32 + dv - 1;
                            if shape.is_full(p[0], p[1], p[2]) {
                                count += 1;
                                odd ^= du ^ dv;
                            }
                        }
                    }
                    let edge = count == 1 || count == 3 || (count == 2 && odd == 0);
                    if edge {
                        if start.is_none() {
                            start = Some(t);
                        }
                    } else if let Some(first) = start.take() {
                        let mut from = [0.0; 3];
                        let mut to = [0.0; 3];
                        from[a] = shape.coordinates(Axis::ALL[a])[u];
                        from[b] = shape.coordinates(Axis::ALL[b])[v];
                        to[a] = from[a];
                        to[b] = from[b];
                        from[axis] = shape.coordinates(Axis::ALL[axis])[first];
                        to[axis] = shape.coordinates(Axis::ALL[axis])[t];
                        result.push(J::Arr(from.into_iter().chain(to).map(J::f64).collect()));
                    }
                }
            }
        }
    }
    result
}

fn encode_shape(shape: &VoxelShape) -> Tag {
    let mut c = nbt::Compound::new();
    let (min, max) = shape.discrete_bounds();
    c.insert("canonical".into(), Tag::Byte(0));
    c.insert("cube".into(), Tag::Byte(i8::from(shape.is_cube())));
    let axes = shape.cube_coordinate_axes();
    c.insert(
        "cube_axes".into(),
        Tag::Byte((axes[0] as i8) | ((axes[1] as i8) << 1) | ((axes[2] as i8) << 2)),
    );
    for (name, axis) in ["x", "y", "z"].into_iter().zip(Axis::ALL) {
        c.insert(
            name.into(),
            Tag::LongArray(
                shape
                    .coordinates(axis)
                    .iter()
                    .map(|n| n.to_bits() as i64)
                    .collect(),
            ),
        );
    }
    let [x, y, z] = shape.sizes();
    let mut occupied = Vec::with_capacity(x * y * z);
    for i in 0..x {
        for j in 0..y {
            for k in 0..z {
                occupied.push(u8::from(shape.is_full(i as i32, j as i32, k as i32)));
            }
        }
    }
    c.insert("occupied".into(), Tag::ByteArray(occupied));
    c.insert("discrete_min".into(), Tag::IntArray(min.to_vec()));
    c.insert("discrete_max".into(), Tag::IntArray(max.to_vec()));
    Tag::Compound(c)
}

/// A block being added: its declaration and where its states go.
struct NewBlock<'a> {
    decl: &'a Block,
    rename: Rename,
    /// The template's item, if it has one.
    item: Option<String>,
    shapes: Option<Vec<VoxelShape>>,
    default: u32,
    /// The declared state properties' combinations, every property at its
    /// default first (one empty combination when the block declares none):
    /// the block's states are its template's states times these.
    combos: Vec<Vec<(String, String)>>,
    /// `combos` by name, for the selectors that pick a state's box or light
    /// (`collision_boxes`, `light_emission`, ...).
    combo_maps: Vec<BTreeMap<String, String>>,
    /// Each state's template state offset and combination index, in registry order
    /// (`state_order`).
    order: Vec<(u32, u32)>,
    /// The sign whose item places this block on a wall, which makes that item
    /// this block's too (`StandingAndWallBlockItem.registerBlocks`).
    wall_of: Option<String>,
}
impl NewBlock<'_> {
    /// Whether it declares state properties of its own.
    fn declares(&self) -> bool {
        !self.decl.state_properties.is_empty()
    }
    /// Its states: each of the template's times each combination.
    fn state_count(&self) -> usize {
        self.order.len()
    }
    /// The template state its `index`th state (in registry order) copies.
    fn template_id(&self, index: usize) -> u32 {
        self.rename.states.start + self.order[index].0
    }
    /// The declared properties of its `index`th state, if it declares any.
    fn declared(&self, index: usize) -> Option<&BTreeMap<String, String>> {
        self.declares()
            .then(|| &self.combo_maps[self.order[index].1 as usize])
    }
    fn collision_box(&self, index: usize) -> Option<[f64; 6]> {
        self.selected_box(index,&self.decl.collision_boxes)
    }
    fn outline_box(&self,index:usize)->Option<[f64;6]> {
        self.selected_box(index,&self.decl.outline_boxes).or_else(||self.collision_box(index))
    }
    fn selected_box(&self,index:usize,boxes:&BTreeMap<String,[f64;6]>)->Option<[f64;6]> {
        let state = self.declared(index);
        boxes
            .iter()
            .filter_map(|(selector, bounds)| {
                if selector == "*" {
                    return Some((0, *bounds));
                }
                let state = state?;
                let pairs = selector
                    .split(',')
                    .map(|part| part.split_once('='))
                    .collect::<Option<Vec<_>>>()?;
                pairs
                    .iter()
                    .all(|(k, v)| state.get(*k).is_some_and(|x| x == v))
                    .then_some((pairs.len(), *bounds))
            })
            .max_by_key(|(specificity, _)| *specificity)
            .map(|(_, bounds)| bounds)
    }
    fn emission(&self, index: usize) -> Option<u8> {
        self.selected_light(index, &self.decl.light_emission)
    }
    fn selected_light(&self, index: usize, values: &BTreeMap<String,u8>) -> Option<u8> {
        let state = self.declared(index);
        values.iter()
            .filter_map(|(selector, light)| {
                if selector == "*" {
                    return Some((0, *light));
                }
                let pairs = selector
                    .split(',')
                    .map(|p| p.split_once('='))
                    .collect::<Option<Vec<_>>>()?;
                pairs
                    .iter()
                    .all(|(k, v)| state.and_then(|s| s.get(*k)).is_some_and(|x| x == v))
                    .then_some((pairs.len(), *light))
            })
            .max_by_key(|(n, _)| *n)
            .map(|(_, light)| light)
    }
    /// The state its `index`th state turns into by `structure-blocks.json`'s
    /// `rotate` or `mirror` (`amount` its column), when the template's own
    /// turn lands on template state `at` (an offset in `rename.states`): the
    /// declared `facing`, `forward` and `up` turn with it.
    fn transformed(&self, index: usize, operation: &str, amount: usize, at: u32) -> u32 {
        let combo = self.order[index].1 as usize;
        let mut props = self.combo_maps[combo].clone();
        for key in ["facing", "forward", "up"] {
            let Some(value) = props.get_mut(key) else {
                continue;
            };
            let dirs = ["north", "east", "south", "west"];
            let Some(at) = dirs.iter().position(|d| *d == value) else {
                continue;
            };
            let next = match operation {
                "rotate" => (at + [1, 2, 3][amount]) % 4,
                "mirror" if amount == 0 => (6 - at) % 4,
                "mirror" => (4 - at) % 4,
                _ => at,
            };
            *value = dirs[next].into();
        }
        let turned = self.combo_maps.iter().position(|s| *s == props).unwrap_or(combo) as u32;
        self.rename.first + self.rename.row(at, turned)
    }
}
impl NewBlock<'_> {
    /// Template states and declared combinations in registry order.
    fn state_combinations(&self) -> impl Iterator<Item = (u32, &[(String, String)])> {
        let start = self.rename.states.start;
        self.order
            .iter()
            .map(move |&(state, combo)| (start + state, self.combos[combo as usize].as_slice()))
    }
}
/// An item being added (declared, or a block's).
struct NewItem {
    rename: Rename,
    max_stack_size: Option<u8>,
    rarity: Option<Rarity>,
    components: Vec<(String, J)>,
    crafting_remainder: Option<String>,
    /// `StandingAndWallBlockItem.wallBlock`, when its block declares it.
    wall_block: Option<String>,
}

struct Plan<'a> {
    blocks: Vec<NewBlock<'a>>,
    items: Vec<NewItem>,
    /// Every vanilla item id, including items omitted from the search tab.
    vanilla_items: BTreeSet<String>,
    effects: Vec<&'a MobEffect>,
    entity_types: Vec<&'a EntityType>,
    menu_types: Vec<&'a MenuType>,
    /// Vanilla's state count (the tables' length before).
    states: usize,
    /// Each vanilla block's default state.
    defaults: BTreeMap<String, u32>,
    /// The gameplay rows' sound objects, by block, for `sounds` overrides.
    sounds: BTreeMap<String, J>,
    /// The mods' creative tabs, in mod order.
    tabs: Vec<&'a CreativeTab>,
    /// The mods' English names of their items and blocks (`item.<ns>.<path>`,
    /// `block.<ns>.<path>`), for the search tab.
    names: BTreeMap<String, String>,
}

/// Component types present in the vanilla item table.
pub fn known_components(base: &Path) -> Result<BTreeSet<String>> {
    let path = base.join("items.json");
    if !path.exists() {
        return Ok(BTreeSet::new());
    }
    Ok(component_types(&read_json(&path)?))
}

fn component_types(map: &J) -> BTreeSet<String> {
    match map {
        J::Obj(entries) => entries
            .iter()
            .filter_map(|(_, v)| match v {
                J::Obj(c) => Some(c.iter().map(|(k, _)| k.clone())),
                _ => None,
            })
            .flatten()
            .collect(),
        _ => BTreeSet::new(),
    }
}

/// Blocks and items native mods declare in code (`native_blocks`,
/// `native_items`), joined to the content mods: each namespace's go into
/// the content mod of that id when one is on (a native mod's own `.hmod`,
/// with its models and textures), else into a content mod with no files of
/// its own. Checked as files are ([`horizon_mods::Content::declare`],
/// [`accept_mod`]); a namespace whose declarations are refused is left out
/// whole, and its error returned.
pub fn accept_declared(
    mods: &mut Vec<ContentMod>,
    blocks: &[horizon_mods::content::Block],
    items: &[horizon_mods::content::Item],
    known: &BTreeSet<String>,
) -> Vec<String> {
    let namespace = |id: &str| id.split_once(':').map_or("", |(ns, _)| ns).to_owned();
    let mut by_namespace: std::collections::BTreeMap<String, (Vec<_>, Vec<_>)> = Default::default();
    for block in blocks {
        by_namespace
            .entry(namespace(&block.id))
            .or_default()
            .0
            .push(block.clone());
    }
    for item in items {
        by_namespace
            .entry(namespace(&item.id))
            .or_default()
            .1
            .push(item.clone());
    }
    let mut errors = Vec::new();
    for (ns, (blocks, items)) in by_namespace {
        let result = match mods.iter().position(|m| m.id == ns) {
            Some(i) => {
                let mut content = horizon_mods::Content::declared(&ns);
                content.blocks.clone_from(&mods[i].content.blocks);
                content.items.clone_from(&mods[i].content.items);
                content
                    .declare(&blocks, &items)
                    .and_then(|()| {
                        let mut added = horizon_mods::Content::declared(&ns);
                        added.blocks.clone_from(&blocks);
                        added.items.clone_from(&items);
                        validate_components(&added, known).map_err(|e| e.to_string())
                    })
                    .map(|()| {
                        mods[i].content.blocks.extend(blocks);
                        mods[i].content.items.extend(items);
                    })
            }
            None => {
                let mut content = horizon_mods::Content::declared(&ns);
                content.declare(&blocks, &items).and_then(|()| {
                    let candidate = ContentMod {
                        id: ns.clone(),
                        content,
                    };
                    accept_mod(mods, candidate, known).map_err(|e| e.to_string())
                })
            }
        };
        if let Err(e) = result {
            errors.push(format!("native blocks and items of {ns}: {e}"));
        }
    }
    errors
}

/// Check and accept one mod without disturbing previously accepted mods.
pub fn accept_mod(
    mods: &mut Vec<ContentMod>,
    candidate: ContentMod,
    known: &BTreeSet<String>,
) -> Result<()> {
    validate_components(&candidate.content, known)?;
    let own = (
        candidate.id.as_str(),
        candidate.content.namespaces.as_slice(),
    );
    if let Some(error) = mods
        .iter()
        .find_map(|c| horizon_mods::content::clash((&c.id, &c.content.namespaces), own))
    {
        return Err(error.into());
    }
    mods.push(candidate);
    Ok(())
}

fn validate_components(content: &horizon_mods::Content, known: &BTreeSet<String>) -> Result<()> {
    for (id, components) in content.items.iter().map(|i| (&i.id, &i.components)).chain(
        content
            .blocks
            .iter()
            .filter(|b| b.item)
            .map(|b| (&b.id, &b.components)),
    ) {
        let patch = component_patch(id, components)?;
        patch_components(&mut J::Obj(Vec::new()), &patch, known)
            .map_err(|e| format!("{id}: {e}"))?;
    }
    Ok(())
}

/// An item's component patch on its entry; `known` holds every type a vanilla item has, so typos fail.
fn patch_components(
    entry: &mut J,
    patch: &[(String, J)],
    known: &BTreeSet<String>,
) -> std::result::Result<(), String> {
    for (key, value) in patch {
        let kind = key.strip_prefix('!').unwrap_or(key);
        if !known.contains(kind) && horizon_protocol::item::custom::id(kind).is_none() {
            return Err(format!("{kind} is not a known component type"));
        }
        if kind == key {
            entry.set(key, value.clone());
        } else {
            entry.remove(kind);
        }
    }
    Ok(())
}

/// A declared component patch, as the overlay's JSON.
fn component_patch(
    id: &str,
    components: &BTreeMap<String, serde_json::Value>,
) -> Result<Vec<(String, J)>> {
    components
        .iter()
        .map(|(key, value)| {
            if !key.starts_with('!') && horizon_protocol::item::custom::id(key).is_some() {
                let tag = horizon_client_core::scoreboard::json_to_tag(value);
                horizon_protocol::item::custom::validate(&tag)
                    .map_err(|error| format!("{id}: component {key}: {error}"))?;
            }
            Ok((
                key.clone(),
                serde_json::from_value(value.clone()).map_err(|error| format!("{id}: {error}"))?,
            ))
        })
        .collect()
}

fn read_json(path: &Path) -> Result<J> {
    Ok(serde_json::from_slice(&fs::read(path)?).map_err(|e| format!("{}: {e}", path.display()))?)
}

impl Plan<'_> {
    /// `entity-types.json` with a row for each mod type: its template's, so its
    /// size, eyes and category are the template's (`execute anchored eyes`,
    /// spawner checks and the commands' entity type list, `reg.entity_types`).
    /// The rows are appended as text: parsed and written back, a float such as
    /// a zombie's `movement_speed` can come back one ulp off.
    fn entity_types_json(&self, bytes: &[u8]) -> std::result::Result<Vec<u8>, String> {
        use serde_json::value::RawValue;
        let rows: BTreeMap<String, Box<RawValue>> =
            serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        let mut out = bytes.trim_ascii_end().to_vec();
        if out.pop() != Some(b'}') {
            return Err("not an object".into());
        }
        let mut empty = rows.is_empty();
        for t in &self.entity_types {
            if rows.contains_key(&t.id) {
                continue;
            }
            let Some(row) = rows.get(&t.template) else {
                continue;
            };
            if !std::mem::take(&mut empty) {
                out.push(b',');
            }
            out.extend(serde_json::to_vec(&t.id).map_err(|e| e.to_string())?);
            out.push(b':');
            out.extend(row.get().as_bytes());
        }
        out.push(b'}');
        Ok(out)
    }
}

impl<'a> Plan<'a> {
    fn new(base: &Path, mods: &'a [ContentMod]) -> Result<Self> {
        let vanilla_items = match base.join("items.json").exists() {
            true => match read_json(&base.join("items.json"))? {
                J::Obj(entries) => entries.into_iter().map(|(id, _)| id).collect(),
                _ => return Err("items.json is not an object".into()),
            },
            false => BTreeSet::new(),
        };
        let mut facts = read_json(&base.join("facts.json"))?;
        let names: Vec<String> = match facts.at("states") {
            Some(J::Arr(states)) => states
                .iter()
                .map(|s| match s {
                    J::Arr(row) => match row.first() {
                        Some(J::Str(name)) => Ok(name.clone()),
                        _ => Err("facts.json: a state without a name"),
                    },
                    _ => Err("facts.json: a state that is not a row"),
                })
                .collect::<std::result::Result<_, _>>()?,
            _ => return Err("facts.json: no states".into()),
        };
        let defaults: BTreeMap<String, u32> = match facts.at("defaults") {
            Some(J::Obj(o)) => o
                .iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_u32()?)))
                .collect(),
            _ => return Err("facts.json: no defaults".into()),
        };
        // A block's states are one run in registry order; a name seen in two
        // runs has none.
        let mut runs: HashMap<&str, Option<Range<u32>>> = HashMap::new();
        for (i, name) in names.iter().enumerate() {
            let i = i as u32;
            runs.entry(name.as_str())
                .and_modify(|run| match run {
                    Some(r) if r.end == i => r.end = i + 1,
                    _ => *run = None,
                })
                .or_insert(Some(i..i + 1));
        }
        let range = |name: &str| -> Option<Range<u32>> { runs.get(name)?.clone() };
        let mut blocks = Vec::new();
        let mut next = names.len() as u32;
        let mut sorted: Vec<&ContentMod> = mods.iter().collect();
        sorted.sort_by(|a, b| a.id.cmp(&b.id));
        let mut wanted_sounds = Vec::new();
        for m in &sorted {
            let mut declared: Vec<&Block> = m.content.blocks.iter().collect();
            declared.sort_by(|a, b| a.id.cmp(&b.id));
            for decl in declared {
                if defaults.contains_key(&decl.id) {
                    return Err(format!("{}: {} is already a block", m.id, decl.id).into());
                }
                let states = range(&decl.template).ok_or_else(|| {
                    format!("{}: template {} is not a block", decl.id, decl.template)
                })?;
                let shapes = if let Some(spec) = &decl.shapes {
                    let Some(J::Arr(rows)) = facts.get("states") else {
                        unreachable!()
                    };
                    let mut used = BTreeSet::new();
                    let mut out = Vec::new();
                    for id in states.clone() {
                        let J::Arr(row) = &rows[id as usize] else {
                            unreachable!()
                        };
                        let Some(J::Str(value)) = row
                            .get(4)
                            .and_then(|x| x.get("properties"))
                            .and_then(|x| x.get(&spec.property))
                        else {
                            return Err(format!(
                                "{}: template state lacks property {}",
                                decl.id, spec.property
                            )
                            .into());
                        };
                        let boxes = spec.values.get(value).ok_or_else(|| {
                            format!("{}: no shape for {}={value}", decl.id, spec.property)
                        })?;
                        used.insert(value.as_str());
                        let mut shape = VoxelShape::empty();
                        for b in boxes {
                            let part = VoxelShape::from_box(Aabb::new(
                                Vec3::new(b[0], b[1], b[2]),
                                Vec3::new(b[3], b[4], b[5]),
                            ))?;
                            shape = shape.join_unoptimized(&part, ShapeOp::OR)?;
                        }
                        out.push(shape);
                    }
                    if used.len() != spec.values.len() {
                        return Err(format!(
                            "{}: shape values include an unknown {} value",
                            decl.id, spec.property
                        )
                        .into());
                    }
                    Some(out)
                } else if let Some(size) = decl.thickness {
                    let Some(J::Arr(rows)) = facts.get("states") else {
                        unreachable!()
                    };
                    let shapes = states.clone().map(|id| {
                        let props = match &rows[id as usize] {
                            J::Arr(row) => row.get(4).and_then(|p| p.get("properties")),
                            _ => None,
                        };
                        let sides = ["down", "up", "north", "south", "west", "east"]
                            .map(|k| matches!(props.and_then(|p| p.get(k)), Some(J::Str(v)) if v == "true"));
                        horizon_world::collision::pipe_shape(size.into(), sides)
                    });
                    Some(shapes.collect::<std::io::Result<_>>()?)
                } else {
                    None
                };
                let mut default = *defaults.get(&decl.template).ok_or_else(|| {
                    format!(
                        "{}: template {} has no default state",
                        decl.id, decl.template
                    )
                })?;
                let Some(J::Arr(rows)) = facts.get("states") else {
                    unreachable!()
                };
                let properties = |id: u32| -> Option<&J> {
                    let J::Arr(row) = rows.get(id as usize)? else {
                        return None;
                    };
                    row.get(4)?.get("properties")
                };
                for key in decl.state_properties.keys() {
                    if properties(default).is_some_and(|p| p.get(key).is_some()) {
                        return Err(format!(
                            "{}: state property {key} is already on {}",
                            decl.id, decl.template
                        )
                        .into());
                    }
                }
                let combos = combinations(&decl.state_properties, &decl.default_state)
                    .map_err(|e| format!("{}: {e}", decl.id))?;
                let combo_maps = combos
                    .iter()
                    .map(|c| c.iter().cloned().collect())
                    .collect();
                // A `default_state` key the block itself declares is its own,
                // and `combinations` already checks that value against the
                // property. Only a key the template already has narrows which
                // of the template's states is the default, and only that needs
                // the template's own properties: a template with none (`stone`
                // has a single state) can still be given a property and have
                // it defaulted.
                let on_template = decl
                    .default_state
                    .keys()
                    .any(|key| !decl.state_properties.contains_key(key));
                if on_template {
                    let mut wanted = properties(default).cloned().ok_or_else(|| {
                        format!("{}: template default has no state properties", decl.id)
                    })?;
                    for (key, value) in &decl.default_state {
                        if decl.state_properties.contains_key(key) {
                            continue;
                        }
                        if wanted.get(key).is_none() {
                            return Err(format!(
                                "{}: unknown state property {key} on {}",
                                decl.id, decl.template
                            )
                            .into());
                        }
                        wanted.set(key, J::Str(value.clone()));
                    }
                    let matches: Vec<u32> = states
                        .clone()
                        .filter(|id| {
                            properties(*id).is_some_and(|p| {
                                let (J::Obj(a), J::Obj(b)) = (p, &wanted) else {
                                    return false;
                                };
                                a.len() == b.len() && b.iter().all(|(k, v)| p.get(k) == Some(v))
                            })
                        })
                        .collect();
                    if matches.len() != 1 {
                        return Err(format!(
                            "{}: default_state {:?} matches {} states of {}",
                            decl.id,
                            decl.default_state,
                            matches.len(),
                            decl.template
                        )
                        .into());
                    }
                    default = matches[0];
                }
                // The game reads a state table as vanilla's `StateDefinition` does: every
                // property, the template's and the declared ones, sorted by name, the first
                // name slowest. A declared name that sorts before the template's has to
                // interleave with them, so the states are laid out in that order.
                let declared: Vec<(String, u32)> = decl
                    .state_properties
                    .iter()
                    .map(|(name, property)| (name.clone(), property.values().len() as u32))
                    .collect();
                let order = block_order(&states, properties, &declared, combos.len() as u32);
                // The lookup from (template state, combination) to the state's row.
                let mut rows = vec![0; order.len()];
                for (position, &(state, combo)) in order.iter().enumerate() {
                    rows[state as usize * combos.len() + combo as usize] = position as u32;
                }
                let mut rename = Rename::new(&decl.template, &decl.id);
                rename.first = next;
                rename.fanout = combos.len() as u32;
                rename.rows = rows;
                let count =
                    u32::try_from(states.len() as u64 * combos.len() as u64).unwrap_or(u32::MAX);
                next = next.saturating_add(count);
                rename.states = states;
                if let Some(s) = &decl.properties.sounds {
                    if !defaults.contains_key(s) {
                        return Err(format!("{}: sounds {s} is not a block", decl.id).into());
                    }
                    wanted_sounds.push(s.clone());
                }
                blocks.push(NewBlock {
                    decl,
                    rename,
                    item: None,
                    shapes,
                    default,
                    combos,
                    combo_maps,
                    order,
                    wall_of: None,
                });
            }
        }
        // A sign's wall form, which `Content` checked against its mod's blocks.
        let pairs: Vec<_> = blocks
            .iter()
            .filter_map(|b| Some((b.decl.wall_block.clone()?, b.decl.id.clone())))
            .collect();
        for (wall, sign) in pairs {
            if let Some(block) = blocks.iter_mut().find(|b| b.decl.id == wall) {
                block.wall_of = Some(sign);
            }
        }
        if next > MAX_STATES {
            let mut by_mod: BTreeMap<&str, u32> = BTreeMap::new();
            for b in &blocks {
                *by_mod
                    .entry(&b.decl.id[..b.decl.id.find(':').unwrap_or(0)])
                    .or_default() += (b.rename.states.len() as u32).saturating_mul(b.rename.fanout);
            }
            let list: Vec<String> = by_mod.iter().map(|(m, n)| format!("{m} {n}")).collect();
            return Err(format!(
                "{next} block states, more than the {MAX_STATES} a world can hold ({} in the game; \
                 the mods: {})",
                names.len(),
                list.join(", ")
            )
            .into());
        }
        // Block items copy the template block's item. Only these two fields:
        // the table's whole tree is several hundred MB, and it is written
        // later anyway, one table at a time.
        drop(facts);
        #[derive(Deserialize)]
        struct GameplayRow {
            item: Option<J>,
            sound: Option<J>,
        }
        #[derive(Deserialize)]
        struct Gameplay {
            states: Option<Vec<GameplayRow>>,
        }
        let gameplay: Gameplay =
            serde_json::from_slice(&fs::read(base.join("gameplay-facts.json"))?)
                .map_err(|e| format!("gameplay-facts.json: {e}"))?;
        let rows = match &gameplay.states {
            Some(rows) if rows.len() == names.len() => rows,
            _ => return Err("gameplay-facts.json does not match facts.json".into()),
        };
        let mut sounds = BTreeMap::new();
        for s in wanted_sounds {
            let row = &rows[defaults[&s] as usize];
            sounds.insert(s, row.sound.clone().unwrap_or(J::Null));
        }
        let mut items = Vec::new();
        for b in &mut blocks {
            let row = &rows[b.rename.states.start as usize];
            b.item = match &row.item {
                Some(J::Str(i)) if i != "minecraft:air" => Some(i.clone()),
                _ => None,
            };
            if b.decl.item {
                let item = b.item.as_deref().ok_or_else(|| {
                    format!("{}: template {} has no item", b.decl.id, b.decl.template)
                })?;
                items.push(NewItem {
                    rename: Rename::new(item, &b.decl.id),
                    max_stack_size: None,
                    rarity: None,
                    components: component_patch(&b.decl.id, &b.decl.components)?,
                    crafting_remainder: None,
                    wall_block: b.decl.wall_block.clone(),
                });
            }
        }
        for m in &sorted {
            for i in &m.content.items {
                items.push(NewItem {
                    rename: Rename::new(&i.template, &i.id),
                    max_stack_size: i.max_stack_size,
                    rarity: i.rarity,
                    components: component_patch(&i.id, &i.components)?,
                    crafting_remainder: i.crafting_remainder.clone(),
                    wall_block: None,
                });
            }
        }
        // Block items and standalone items share the same registry. Sort
        // them together by mod ID and entry name before assigning IDs.
        items.sort_by(|a, b| a.rename.to.cmp(&b.rename.to));
        let mut tabs = Vec::new();
        let mut lang = BTreeMap::new();
        for m in &sorted {
            tabs.extend(m.content.creative_tabs.iter());
            let mut reader = None;
            for ns in &m.content.namespaces {
                let file = format!("assets/{ns}/lang/en_us.json");
                if m.content.files.binary_search(&file).is_err() {
                    continue;
                }
                if reader.is_none() {
                    reader = Some(m.content.reader()?);
                }
                let Some(Ok(bytes)) = reader.as_mut().map(|r| r.read(&file)) else {
                    continue;
                };
                if let Ok(map) = serde_json::from_slice::<BTreeMap<String, String>>(&bytes) {
                    // A mod's text is mostly not its items' and blocks' names.
                    lang.extend(
                        map.into_iter()
                            .filter(|(key, _)| key.starts_with("item.") || key.starts_with("block.")),
                    );
                }
            }
        }
        Ok(Plan {
            blocks,
            items,
            vanilla_items,
            effects: mob_effects(mods),
            entity_types: entity_types(mods),
            menu_types: menu_types(mods),
            states: names.len(),
            defaults,
            sounds,
            tabs,
            names: lang,
        })
    }

    /// What moves in a new block's copies of its template's rows: the
    /// block, and the template's item when it has another name. A block
    /// without an item gets `minecraft:air` where rows name one, except a
    /// sign's wall form, whose item is the paired sign's.
    fn renames(&self, b: &NewBlock) -> Vec<Rename> {
        let mut out = vec![b.rename.clone()];
        let own = b.decl.item.then_some(b.decl.id.as_str());
        if let Some(item) = b.item.as_deref().filter(|i| *i != b.decl.template)
            && let Some(to) = own.or(b.wall_of.as_deref())
        {
            out.push(Rename::new(item, to));
        }
        out
    }

    /// A new block's `i`th state's row, one per template state per combination
    /// the block declares ([`NewBlock::state_combinations`]) so its states stay
    /// one run in registry order, copied from its template's in `list` and
    /// rewritten for `table`: `gameplay` rows get their index and the declared
    /// overrides.
    fn state_row(
        &self,
        b: &NewBlock,
        renames: &[Rename],
        i: usize,
        (id, combo): (u32, &[(String, String)]),
        list: &[J],
        table: StateList<'_>,
    ) -> Result<J> {
        let StateList {
            ids,
            gameplay,
            table,
            ..
        } = table;
        let mut row = list[id as usize].clone();
        for r in renames {
            row.rename(r, ids);
        }
        if !combo.is_empty() {
            set_state_properties(&mut row, combo);
        }
        if let Some(declared) = b.declared(i) {
            if table == "facts.json"
                && let J::Arr(parts) = &mut row
            {
                // facts.json also names each state with its
                // properties, and a declared `waterlogged` holds
                // water. Other tables copy the template's behaviour.
                if let Some(value) = declared.get("waterlogged")
                    && let Some(fluid) = parts.get_mut(2)
                {
                    *fluid = J::Bool(value == "true");
                }
                let properties: BTreeMap<&str, &str> =
                    match parts.get(4).and_then(|state| state.get("properties")) {
                        Some(J::Obj(p)) => p
                            .iter()
                            .filter_map(|(k, v)| match v {
                                J::Str(v) => Some((k.as_str(), v.as_str())),
                                _ => None,
                            })
                            .collect(),
                        _ => BTreeMap::new(),
                    };
                let values = properties
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(",");
                if let Some(J::Str(name)) = parts.get_mut(3)
                    && name.starts_with("Block{")
                {
                    *name = format!("Block{{{}}}[{values}]", b.decl.id);
                }
            }
            if let Some(waterlogged) = declared.get("waterlogged") {
                let water = waterlogged == "true";
                if let J::Arr(parts) = &mut row {
                    match table {
                        "block-behaviours.json" if parts.len() > 4 => {
                            parts[2] = J::Num((if water { 2 } else { 0 }).into());
                            parts[3] = J::Num((if water { 8 } else { 0 }).into());
                            parts[4] = J::Bool(false);
                        }
                        "worldgen/block_flags.json" if parts.len() > 1 => {
                            parts[1] = J::Bool(water)
                        }
                        _ => {}
                    }
                }
                if gameplay && water {
                    row.set("skylight_down", J::Bool(false));
                    // A non-occluding, non-colliding waterlogged fixture
                    // attenuates by one, just like vanilla lanterns.
                    if b.decl.properties.no_occlusion == Some(true)
                        && b.decl.properties.no_collision == Some(true)
                    {
                        row.set("dampening", J::Num(1u8.into()));
                    }
                }
            }
        }
        if gameplay {
            row.set("id", J::Num((b.rename.first + i as u32).into()));
            if !b.decl.item && b.wall_of.is_none() {
                row.set("item", J::Str("minecraft:air".into()));
            }
            self.override_gameplay(b, combo, &mut row);
            if let Some(shapes) = &b.shapes {
                let shape = &shapes[b.order[i].0 as usize];
                let boxes = shape
                    .boxes()
                    .iter()
                    .map(|b| {
                        J::Arr(
                            [
                                b.min(Axis::X),
                                b.min(Axis::Y),
                                b.min(Axis::Z),
                                b.max(Axis::X),
                                b.max(Axis::Y),
                                b.max(Axis::Z),
                            ]
                            .into_iter()
                            .map(J::f64)
                            .collect(),
                        )
                    })
                    .collect();
                row.set("outline", J::Arr(boxes));
                row.set("outline_edges", J::Arr(shape_edges(shape)));
                row.set(
                    "full_collision",
                    J::Bool(!shape.join_is_not_empty(&VoxelShape::block(), ShapeOp::XOR)?),
                );
            }
            if let Some(light) = b.emission(i) {
                row.set("emission", J::Num(light.into()));
            }
            if let Some(light) = b.selected_light(i,&b.decl.light_dampening_states) {
                row.set("dampening", J::Num(light.into()));
            }
            if b.decl.properties.outline_empty==Some(true) {
                row.set("outline",J::Arr(vec![]));row.set("outline_edges",J::Arr(vec![]));
            } else if let Some(bounds) = b.outline_box(i) {
                row.set(
                    "outline",
                    J::Arr(vec![J::Arr(bounds.iter().map(|v| J::f64(*v)).collect())]),
                );
                let mut edges = Vec::new();
                for axis in 0..3 {
                    for a in 0..2 {
                        for c in 0..2 {
                            let other: Vec<_> = (0..3).filter(|&v| v != axis).collect();
                            let mut from = [0.0; 3];
                            let mut to = [0.0; 3];
                            from[axis] = bounds[axis];
                            to[axis] = bounds[axis + 3];
                            from[other[0]] = bounds[other[0] + 3 * a];
                            to[other[0]] = from[other[0]];
                            from[other[1]] = bounds[other[1] + 3 * c];
                            to[other[1]] = from[other[1]];
                            edges.push(J::Arr(
                                from.into_iter().chain(to).map(J::f64).collect(),
                            ));
                        }
                    }
                }
                row.set("outline_edges", J::Arr(edges));
            }
            if b.decl.properties.no_collision==Some(true) || b.collision_box(i).is_some() {row.set("full_collision",J::Bool(false));}
            if b.decl.properties.no_occlusion==Some(true) || b.collision_box(i).is_some() {
                row.set("can_occlude",J::Bool(false));row.set("solid_render",J::Bool(false));
            }
            // Collision geometry and Java's canOcclude property are
            // independent (e.g. a certus bud has a partial shape but
            // does not call noOcclusion).
            if let Some(v)=b.decl.properties.can_occlude { row.set("can_occlude",J::Bool(v)); }
        }
        if table=="block-behaviours.json" && let J::Arr(parts)=&mut row && parts.len()>1 && let J::Num(n)=&parts[1] && let Some(mut flags)=n.as_u64() {
            if b.decl.properties.no_collision==Some(true){flags|=1<<5;}
            if b.decl.properties.no_support==Some(true) {parts[0]=J::Num(0u8.into());}
            if let Some(conductor)=b.decl.properties.redstone_conductor {if conductor {flags|=1<<2;}else {flags&=!(1<<2);}}
            if let Some(solid)=b.decl.properties.solid {if solid {flags|=1;} else {flags&=!1;}}
            if let Some(suffocating)=b.decl.properties.suffocating {if suffocating {flags|=1<<6;} else {flags&=!(1<<6);}}
            parts[1]=J::Num(flags.into());
        }
        Ok(row)
    }

    /// `list` and the new blocks' rows after it, as JSON. Blocks are turned into
    /// rows a chunk at a time on several threads and written in order, so only
    /// the chunks in hand are in memory, whatever the table's size.
    fn write_list(&self, list: &[J], table: StateList<'_>, out: &mut dyn Write) -> Result<()> {
        if list.len() != self.states {
            return Err(format!("{} states, not {}", list.len(), self.states).into());
        }
        out.write_all(b"[")?;
        for (i, row) in list.iter().enumerate() {
            if i > 0 {
                out.write_all(b",")?;
            }
            serde_json::to_writer(&mut *out, row)?;
        }
        let mut chunks = Vec::new();
        let (mut start, mut states) = (0, 0);
        for (i, b) in self.blocks.iter().enumerate() {
            states += b.state_count();
            if states >= ROW_CHUNK {
                chunks.push(start..i + 1);
                (start, states) = (i + 1, 0);
            }
        }
        if start < self.blocks.len() {
            chunks.push(start..self.blocks.len());
        }
        let mut comma = !list.is_empty();
        for batch in chunks.chunks(ROW_THREADS) {
            let encoded = par_map(batch, ROW_THREADS, |range| -> std::result::Result<Vec<u8>, String> {
                let mut bytes = Vec::new();
                for b in &self.blocks[range.clone()] {
                    let renames = self.renames(b);
                    let defaults = Self::turned_defaults(b, table);
                    for (i, state) in b.state_combinations().enumerate() {
                        let mut row = self
                            .state_row(b, &renames, i, state, list, table)
                            .map_err(|e| e.to_string())?;
                        Self::turn_declared(b, i, &mut row, table, defaults.as_ref());
                        bytes.push(b',');
                        serde_json::to_writer(&mut bytes, &row).map_err(|e| e.to_string())?;
                    }
                }
                Ok(bytes)
            });
            for bytes in encoded {
                let bytes = bytes?;
                let bytes = if comma { &bytes[..] } else { bytes.get(1..).unwrap_or_default() };
                comma |= !bytes.is_empty();
                out.write_all(bytes)?;
            }
        }
        out.write_all(b"]")?;
        Ok(())
    }

    /// For [`Self::turn_declared`] on `structure-blocks.json`'s turns: the row each of a declaring
    /// block's template states maps to (every declared property at its default), back to the
    /// template state.
    fn turned_defaults(b: &NewBlock, table: StateList<'_>) -> Option<BTreeMap<u32, u32>> {
        (table.table == "worldgen/blocks/structure-blocks.json"
            && matches!(table.key, "rotate" | "mirror")
            && b.declares())
        .then(|| {
            (0..b.rename.states.len() as u32)
                .map(|at| (b.rename.first + b.rename.row(at, 0), at))
                .collect()
        })
    }

    /// `structure-blocks.json`: a declared `facing`, `forward` or `up` turns with
    /// the block: the template's own turn picks the template state, the declared
    /// properties the state of it.
    fn turn_declared(
        b: &NewBlock,
        i: usize,
        row: &mut J,
        table: StateList<'_>,
        defaults: Option<&BTreeMap<u32, u32>>,
    ) {
        let (Some(defaults), J::Arr(row)) = (defaults, row) else {
            return;
        };
        for (n, target) in row.iter_mut().enumerate() {
            if let Some(&at) = target.as_u32().and_then(|t| defaults.get(&t)) {
                *target = J::Num(b.transformed(i, table.key, n, at).into());
            }
        }
    }

    /// The declared properties, on a gameplay row.
    fn override_gameplay(&self, b: &NewBlock, combo: &[(String, String)], row: &mut J) {
        #[cfg(feature = "ae2-lab")]
        if b.decl.id.starts_with("ae2:") && b.decl.id.ends_with("_budding_quartz") {
            row.set("random_ticks", J::Bool(true));
        }
        // AE2's native machines are block entities even when their visual
        // content declaration inherits an iron block template. The world
        // engine uses this flag to run removal side effects on every path.
        #[cfg(feature = "ae2-lab")]
        if crate::ae2_world::is_machine_block(&b.decl.id) {
            row.set("block_entity", J::Bool(true));
        }
        // These source blocks explicitly return two from getLightBlock.
        #[cfg(feature = "ae2-lab")]
        if matches!(b.decl.id.as_str(), "ae2:tiny_tnt" | "ae2:inscriber") {
            row.set("dampening", J::Num(2u8.into()));
            row.set("skylight_down", J::Bool(false));
        }
        let p = &b.decl.properties;
        if let Some(v)=p.replaceable {row.set("replaceable",J::Bool(v));}
        if let Some(v)=p.skylight_down {row.set("skylight_down",J::Bool(v));}
        if let Some(v)=p.light_dampening {row.set("dampening",J::Num(v.into()));}
        if let Some(v)=&p.push_reaction {row.set("push",J::Str(v.clone()));}
        if let Some(ticks) = &p.random_ticks {
            row.set("random_ticks", J::Bool(ticks.applies(combo)));
        }
        if let Some(v) = p.hardness {
            row.set("hardness", J::f64(v as f64));
        }
        if let Some(v) = p.explosion_resistance {
            row.set("explosion_resistance", J::f64(v as f64));
        }
        if let Some(v) = p.light {
            row.set("emission", J::Num(v.into()));
        }
        if let Some(v) = p.requires_tool {
            row.set("requires_tool", J::Bool(v));
        }
        if let Some(v) = p.friction {
            row.set("friction", J::f64(v as f64));
        }
        if let Some(v) = p.map_color {
            let col = if v == 0 { 0 } else { MAP_COLORS[v as usize] };
            row.set("map_color", J::Num(col.into()));
        }
        if let Some(s) = &p.sounds {
            row.set("sound", self.sounds[s].clone());
        }
    }

    /// Adds each new block's copy of the template's entry to a map by name.
    fn names(&self, map: &mut J, ids: bool) {
        let Some(mut map) = Keyed::new(map) else {
            return;
        };
        for b in &self.blocks {
            if let Some(v) = map.get(&b.decl.template) {
                let mut v = v.clone();
                for r in &self.renames(b) {
                    v.rename(r, ids);
                }
                map.set(&b.decl.id, v);
            }
        }
    }

    /// Adds each new item's copy of its template's entry to a map by name;
    /// `components` applies the overrides (items.json).
    fn items(&self, map: &mut J, components: bool) -> Result<()> {
        let known = if components {
            component_types(map)
        } else {
            BTreeSet::new()
        };
        let Some(mut map) = Keyed::new(map) else {
            return Ok(());
        };
        for i in &self.items {
            let Some(v) = map.get(&i.rename.from) else {
                continue;
            };
            let mut v = v.clone();
            v.rename(&i.rename, false);
            if let Some(wall) = &i.wall_block
                && let Some(fields) = v.get_mut("fields")
            {
                fields.set("wallBlock", J::Str(wall.clone()));
            }
            if components {
                #[cfg(feature = "ae2-lab")]
                if matches!(i.rename.to.as_str(), "ae2:certus_quartz_cutting_knife" | "ae2:nether_quartz_cutting_knife") {
                    v.set("minecraft:max_stack_size", J::Num(1u8.into()));
                    v.set("minecraft:max_damage", J::Num(50u8.into()));
                    v.set("minecraft:damage", J::Num(0u8.into()));
                }
                // FluixToolType: iron's mining level with 3x durability,
                // 1.2x speed and +0.4 attack damage. Write actual components
                // so networked clients observe the same defaults as the host.
                #[cfg(feature = "ae2-lab")]
                if matches!(i.rename.to.as_str(), "ae2:fluix_axe" | "ae2:fluix_hoe" | "ae2:fluix_pickaxe" | "ae2:fluix_shovel" | "ae2:fluix_sword") {
                    v.set("minecraft:max_damage", J::Num(750u32.into()));
                    if let Some(J::Arr(modifiers)) = v.get_mut("minecraft:attribute_modifiers") {
                        for modifier in modifiers {
                            if modifier.get("type") == Some(&J::Str("minecraft:attack_damage".into())) {
                                if let Some(J::Num(amount)) = modifier.get("amount") {
                                    if let Some(amount) = amount.as_f64() { modifier.set("amount", J::f64(amount + 0.4)); }
                                }
                            }
                        }
                    }
                    if let Some(J::Arr(rules)) = v.at("minecraft:tool/rules") {
                        for rule in rules {
                            if let Some(J::Num(speed)) = rule.get("speed") {
                                if speed.as_f64().is_some_and(|n| (n - 6.0).abs() < 0.001) { rule.set("speed", J::f64(7.2)); }
                            }
                        }
                    }
                }
                if let Some(n) = i.max_stack_size {
                    v.set("minecraft:max_stack_size", J::Num(n.into()));
                }
                if let Some(r) = i.rarity {
                    let r = match r {
                        Rarity::Common => "common",
                        Rarity::Uncommon => "uncommon",
                        Rarity::Rare => "rare",
                        Rarity::Epic => "epic",
                    };
                    v.set("minecraft:rarity", J::Str(r.into()));
                }
                patch_components(&mut v, &i.components, &known)
                    .map_err(|e| format!("{}: {e}", i.rename.to))?;
            }
            map.set(&i.rename.to, v);
        }
        if components {
            for i in &self.items {
                if let Some(rest) = &i.crafting_remainder
                    && map.get(rest).is_none()
                {
                    return Err(format!(
                        "{}: crafting_remainder {rest:?} is not an item",
                        i.rename.to
                    )
                    .into());
                }
            }
        }
        Ok(())
    }

    /// One JSON table, rewritten, into `out`. Its new state rows go straight
    /// there and are never in `doc`: with ATM10's 650,000 new states they would
    /// take several GB as a tree.
    fn write_json(&self, name: &str, doc: &mut J, out: &mut dyn Write) -> Result<()> {
        self.edit_json(name, doc)?;
        let lists = state_lists(name);
        let table = |key: &'static str, ids: bool, gameplay: bool| StateList {
            table: name,
            key,
            ids,
            gameplay,
        };
        if let [("", ids, gameplay)] = lists {
            let J::Arr(list) = &*doc else {
                return Err("no list at /".into());
            };
            return self.write_list(list, table("", *ids, *gameplay), out);
        }
        if lists.is_empty() {
            serde_json::to_writer(&mut *out, &*doc)?;
            return Ok(());
        }
        for (key, ..) in lists {
            if !matches!(doc.get(key), Some(J::Arr(_))) {
                return Err(format!("no list at /{key}").into());
            }
        }
        let J::Obj(entries) = &*doc else {
            return Err("not an object".into());
        };
        out.write_all(b"{")?;
        for (n, (key, value)) in entries.iter().enumerate() {
            if n > 0 {
                out.write_all(b",")?;
            }
            serde_json::to_writer(&mut *out, key)?;
            out.write_all(b":")?;
            match (lists.iter().find(|(list, ..)| list == key), value) {
                (Some((list, ids, gameplay)), J::Arr(rows)) => {
                    self.write_list(rows, table(list, *ids, *gameplay), out)?
                }
                _ => serde_json::to_writer(&mut *out, value)?,
            }
        }
        out.write_all(b"}")?;
        Ok(())
    }

    /// [`Plan::write_json`], for a table held as a tree.
    #[cfg(test)]
    fn json(&self, name: &str, doc: &mut J) -> Result<()> {
        let mut bytes = Vec::new();
        self.write_json(name, doc, &mut bytes)?;
        *doc = serde_json::from_slice(&bytes)?;
        Ok(())
    }

    /// What changes in one JSON table besides its new state rows.
    fn edit_json(&self, name: &str, doc: &mut J) -> Result<()> {
        match name {
            "facts.json" => {
                let defaults = doc.at("defaults").ok_or("no defaults")?;
                for b in &self.blocks {
                    let default = b.default;
                    let id = b.rename.id(default).ok_or("a default outside its block")?;
                    defaults.set(&b.decl.id, J::Num(id.into()));
                }
            }
            "gameplay-facts.json" => {}
            "creative-inventory.json" => self.creative_tabs(doc)?,
            "block-behaviours.json" => {
                let Some(J::Arr(blocks)) = doc.at("blocks") else {
                    return Err("no blocks".into());
                };
                for b in &self.blocks {
                    let template = blocks.iter().find(
                        |x| matches!(x.get("name"), Some(J::Str(n)) if *n == b.decl.template),
                    );
                    if let Some(mut v) = template.cloned() {
                        for r in &self.renames(b) {
                            v.rename(r, false);
                        }
                        if !b.decl.item && b.wall_of.is_none() {
                            v.set("item", J::Str("minecraft:air".into()));
                        }
                        // The block it copies, for vanilla's tests of a block
                        // by identity (a cart on a powered rail), and how a
                        // player's placement sets its declared properties.
                        v.set("template", J::Str(b.decl.template.clone()));
                        let placement: Vec<_> = b
                            .decl
                            .placement()
                            .into_iter()
                            .map(|(name, rule)| (name.to_owned(), J::Str(rule.name().into())))
                            .collect();
                        if !placement.is_empty() {
                            v.set("placement", J::Obj(placement));
                        }
                        if b.decl.state_properties.contains_key("waterlogged") {
                            if let Some(J::Arr(interfaces)) = v.get_mut("interfaces") {
                                interfaces.push(J::Str("SimpleWaterloggedBlock".into()));
                            }
                            if let Some(fields) = v.get_mut("fields") {
                                fields.set("scheduleFluidTicks", J::Bool(true));
                            }
                        }
                        // `SignBlock.getWoodType`: the wood's name is what
                        // the editor reads of it.
                        if let Some(wood) = &b.decl.wood_type
                            && let Some(name) = v.at("fields/type/name")
                        {
                            *name = J::Str(wood.clone());
                        }
                        // A connecting block: the chorus plant's PipeBlock
                        // without its rules, joining the declared tag.
                        if let Some(tag) = &b.decl.connects_to {
                            let classes = ["PipeBlock", "Block"].map(|c| J::Str(c.into()));
                            v.set("class", J::Arr(classes.into()));
                            let mut fields = v
                                .get("fields")
                                .cloned()
                                .unwrap_or(J::Obj(Default::default()));
                            fields.set("connectsTo", J::Str(tag.clone()));
                            v.set("fields", fields);
                        }
                        blocks.push(v);
                    }
                }
                self.items(doc.at("items").ok_or("no items")?, false)?;
                for odds in ["fire/burnOdds", "fire/igniteOdds"] {
                    if let Some(map) = doc.at(odds) {
                        self.names(map, false);
                    }
                }
            }
            "model-states.json" | "render-definitions.json" => {
                let model = name == "model-states.json";
                self.names(doc, !model);
                let mut doc = Keyed::new(doc).ok_or("not an object")?;
                for b in &self.blocks {
                    let Some(entry) = doc.get_mut(&b.decl.id) else {
                        continue;
                    };
                    // A block's own properties on a one-state template that lists no
                    // schema or states of its own.
                    if b.declares() {
                        if !matches!(entry.get("states"), Some(J::Arr(s)) if !s.is_empty()) {
                            let one = vec![J::Obj(Vec::new()); b.rename.states.len()];
                            entry.set("states", J::Arr(one));
                        }
                        if model && entry.get("schema").is_none() {
                            entry.set("schema", J::Obj(Vec::new()));
                        }
                    }
                    if model && let Some(schema) = entry.get_mut("schema") {
                        for (property_name, property) in &b.decl.state_properties {
                            schema.set(
                                property_name,
                                J::Obj(vec![(
                                    "named".into(),
                                    J::Arr(
                                        property
                                            .values()
                                            .into_iter()
                                            .map(|v| J::Str(v.into()))
                                            .collect(),
                                    ),
                                )]),
                            );
                        }
                    }
                    if let Some(J::Arr(states)) = entry.get_mut("states") {
                        let template = states.clone();
                        let mut expanded = Vec::new();
                        for (index, (id, combo)) in b.state_combinations().enumerate() {
                            if model {
                                let mut state = template
                                    .get((id - b.rename.states.start) as usize)
                                    .ok_or("model states do not match the template")?
                                    .clone();
                                for (name, value) in combo {
                                    state.set(name, J::Str(value.clone()));
                                }
                                expanded.push(state);
                            } else {
                                expanded.push(J::Num((b.rename.first + index as u32).into()));
                            }
                        }
                        *states = expanded;
                    }
                }
            }
            "landing-definitions.json" | "worldgen/blocks/survival-rules.json" => {
                self.names(doc, false)
            }
            "worldgen/blocks/survival-extra-definitions.json" => {
                if let Some(map) = doc.at("ignition_odds") {
                    self.names(map, false);
                }
            }
            "items.json" => self.items(doc, true)?,
            "registries.json" => {
                // Network ids: the game and its server map items, blocks, mob
                // effects and entity types through this report, so new ones get
                // the ids after vanilla's, in plan order. A block entity type for
                // each block too, named after it: a mod names its types only when
                // one is made (`native_block_entity`), and the chunk packet
                // carries the type of each block's entity.
                let new_items = self.items.iter().map(|i| i.rename.to.clone());
                let new_blocks: Vec<_> = self.blocks.iter().map(|b| b.decl.id.clone()).collect();
                let mut registries = vec![
                    ("minecraft:item", new_items.collect::<Vec<_>>()),
                    ("minecraft:block", new_blocks.clone()),
                    ("minecraft:block_entity_type", new_blocks),
                ];
                if !self.effects.is_empty() {
                    let new_effects = self.effects.iter().map(|e| e.id.clone());
                    registries.push(("minecraft:mob_effect", new_effects.collect()));
                }
                if !self.entity_types.is_empty() {
                    let new_types = self.entity_types.iter().map(|t| t.id.clone());
                    registries.push(("minecraft:entity_type", new_types.collect()));
                }
                // `ClientboundOpenScreenPacket` carries a menu type's id.
                if !self.menu_types.is_empty() {
                    let new_menus = self.menu_types.iter().map(|m| m.id.clone());
                    registries.push(("minecraft:menu", new_menus.collect()));
                }
                for (registry, names) in registries {
                    let Some(J::Obj(entries)) =
                        doc.get_mut(registry).and_then(|r| r.get_mut("entries"))
                    else {
                        // A report without block entity types gets none.
                        if registry == "minecraft:block_entity_type" {
                            continue;
                        }
                        return Err(format!("no {registry} entries").into());
                    };
                    let next = entries
                        .iter()
                        .filter_map(|(_, e)| match e {
                            J::Obj(o) => o.iter().find(|(k, _)| k == "protocol_id")?.1.as_u32(),
                            _ => None,
                        })
                        .max()
                        .map_or(0, |m| m + 1);
                    let have: HashSet<String> = entries.iter().map(|(k, _)| k.clone()).collect();
                    for (i, name) in names.into_iter().enumerate() {
                        if have.contains(&name) {
                            continue;
                        }
                        let id = J::Num((next + i as u32).into());
                        entries.push((name, J::Obj(vec![("protocol_id".into(), id)])));
                    }
                }
            }
            "command-registries.json" => {
                self.names(doc.at("blocks").ok_or("no blocks")?, false);
                let new_items = || self.items.iter().map(|i| J::Str(i.rename.to.clone()));
                let new_blocks = || self.blocks.iter().map(|b| J::Str(b.decl.id.clone()));
                let Some(J::Arr(items)) = doc.at("items") else {
                    return Err("no items".into());
                };
                items.extend(new_items());
                // The registries commands check ids against.
                if let Some(J::Arr(ids)) = doc.at("by_registry/minecraft:item/ids") {
                    ids.extend(new_items());
                }
                if let Some(J::Arr(ids)) = doc.at("by_registry/minecraft:block/ids") {
                    ids.extend(new_blocks());
                }
                // `/effect` and the mob_effect resource arguments.
                let new_effects = || self.effects.iter().map(|e| J::Str(e.id.clone()));
                if let Some(J::Arr(ids)) = doc.at("effects") {
                    ids.extend(new_effects());
                }
                if let Some(J::Arr(ids)) = doc.at("by_registry/minecraft:mob_effect/ids") {
                    ids.extend(new_effects());
                }
                // entity type arguments and `type=` in selectors.
                let new_types = || self.entity_types.iter().map(|t| J::Str(t.id.clone()));
                if let Some(J::Arr(ids)) = doc.at("entity_types") {
                    ids.extend(new_types());
                }
                if let Some(J::Arr(ids)) = doc.at("by_registry/minecraft:entity_type/ids") {
                    ids.extend(new_types());
                }
            }
            "mob-data.json" | "worldgen/block_flags.json" => {}
            "worldgen/blocks/structure-blocks.json" => {
                let added: usize = self
                    .blocks
                    .iter()
                    .map(|b| b.rename.states.len() * b.combos.len())
                    .sum();
                doc.set("states", J::Num(((self.states + added) as u64).into()));
            }
            _ => return Err("no rule for this table".into()),
        }
        Ok(())
    }

    fn collision_shapes(&self, tag: &mut Tag) -> Result<()> {
        let Tag::Compound(root) = tag else {
            return Err("collision table is not a compound".into());
        };
        let mut indices = Vec::new();
        let Some(Tag::List(entries)) = root.get_mut(&JavaString::from("shapes")) else {
            return Err("collision table has no shapes".into());
        };
        for block in &self.blocks {
            if let Some(shapes) = &block.shapes {
                // One shape per template state, shared by every combination
                // of it: the geometry is the template's, the properties are
                // the block's.
                let ids: Vec<i32> = shapes
                    .iter()
                    .map(|shape| {
                        entries.push(encode_shape(shape));
                        entries.len() as i32 - 1
                    })
                    .collect();
                for (at, &shape) in ids.iter().enumerate() {
                    for combination in 0..block.rename.fanout {
                        let row = block.rename.row(at as u32, combination);
                        indices.push((block.rename.first as usize + row as usize, shape));
                    }
                }
            }
        }
        let Some(Tag::IntArray(mapping)) = root.get_mut(&JavaString::from("cached_shapes")) else {
            return Err("collision table has no mapping".into());
        };
        for (state, id) in indices {
            mapping[state] = id;
        }
        Ok(())
    }

    /// The creative inventory (`creative-inventory.json`): each mod tab after
    /// vanilla's, ten to a page as NeoForge pages them (the top row's first
    /// five columns, then the bottom row's), with only the items the game has;
    /// the mods' new items join the search tab under their English names.
    fn creative_tabs(&self, doc: &mut J) -> Result<()> {
        let Some(J::Arr(tabs)) = doc.at("tabs") else {
            return Err("no tabs".into());
        };
        let stack = |id: &str| J::Str(format!("{{count:1,id:\"{id}\"}}"));
        // Blocks' items are among `items` already.
        let new: Vec<&str> = self.items.iter().map(|i| i.rename.to.as_str()).collect();
        let search = tabs
            .iter()
            .position(|t| matches!(t.get("type"), Some(J::Str(k)) if k == "SEARCH"));
        let new_ids: HashSet<&str> = new.iter().copied().collect();
        let known = |id: &str| new_ids.contains(id) || self.vanilla_items.contains(id);
        for (i, tab) in self.tabs.iter().enumerate() {
            let slot = i % 10;
            let mut row = J::Obj(Vec::new());
            row.set("id", J::Str(tab.id.clone()));
            row.set("type", J::Str("CATEGORY".into()));
            row.set(
                "row",
                J::Str(if slot < 5 { "TOP" } else { "BOTTOM" }.into()),
            );
            row.set("column", J::Num((slot % 5).into()));
            row.set("page", J::Num((1 + i / 10).into()));
            row.set("title", J::Str(tab.title.clone()));
            row.set("icon", stack(&tab.icon));
            row.set("visible", J::Bool(true));
            let items = tab
                .items
                .iter()
                .filter(|id| known(id))
                .map(|id| stack(id))
                .collect();
            row.set("items", J::Arr(items));
            tabs.push(row);
        }
        if let Some(search) = search {
            let name = |id: &str| {
                let (ns, path) = id.split_once(':').unwrap_or(("", id));
                ["item", "block"]
                    .iter()
                    .find_map(|kind| self.names.get(&format!("{kind}.{ns}.{path}")))
                    .cloned()
                    .unwrap_or_else(|| path.to_owned())
            };
            let tab = &mut tabs[search];
            for (key, add) in [
                ("items", new.iter().map(|id| stack(id)).collect::<Vec<_>>()),
                (
                    "lines",
                    new.iter()
                        .map(|id| J::Arr(vec![J::Str(name(id))]))
                        .collect(),
                ),
                // shortcut: a mod item's tags aren't searched by (#tag) yet.
                ("tags", new.iter().map(|_| J::Arr(Vec::new())).collect()),
            ] {
                if let Some(J::Arr(list)) = tab.get_mut(key) {
                    list.extend(add);
                }
            }
        }
        Ok(())
    }
    /// One NBT table held as a tree: `states` lists, `cached_shapes` arrays and
    /// `blocks` compounds, wherever they are.
    #[cfg(test)]
    fn nbt(&self, tag: &mut Tag) {
        use nbt::ListExtension;
        fn grow(plan: &Plan, tag: &mut Tag) {
            let Tag::Compound(c) = tag else {
                return;
            };
            for (key, value) in c.iter_mut() {
                match value {
                    Tag::List(list) if key.to_string_lossy() == "states" => {
                        let mut more = Vec::new();
                        plan.elements(list, &mut |row| {
                            more.push(row.clone());
                            Ok(())
                        })
                        .unwrap();
                        list.extend(more);
                    }
                    inner @ Tag::Compound(_) => grow(plan, inner),
                    _ => {}
                }
            }
        }
        self.nbt_table(tag, false);
        grow(self, tag);
    }
    /// What changes in one NBT file besides its `states` lists' new rows, which
    /// [`nbt::ListExtension`] writes; `collision` (the cached collision tables)
    /// also gets each declared `collision_boxes` box as a shape of its own.
    fn nbt_table(&self, tag: &mut Tag, collision: bool) {
        let Tag::Compound(c) = tag else {
            return;
        };
        // Append explicit single-box collision shapes, then refer to their
        // IDs from each corresponding state.
        let mut explicit = BTreeMap::new();
        if collision && c.contains_key(&JavaString::from("cached_shapes")) {
            if let Some(Tag::List(shapes)) = c.get_mut(&JavaString::from("shapes")) {
                for b in &self.blocks {
                    for i in 0..b.state_count() {
                        if let Some(bounds) = b.collision_box(i) {
                            let id = shapes.len() as i32;
                            let axis = |i: usize| {
                                Tag::LongArray(vec![
                                    bounds[i].to_bits() as i64,
                                    bounds[i + 3].to_bits() as i64,
                                ])
                            };
                            shapes.push(Tag::compound([
                                ("canonical", Tag::Byte(0)),
                                ("x", axis(0)),
                                ("y", axis(1)),
                                ("z", axis(2)),
                                ("occupied", Tag::ByteArray(vec![1])),
                                ("cube_axes", Tag::Byte(0)),
                                ("cube", Tag::Byte(0)),
                                ("discrete_min", Tag::IntArray(vec![0, 0, 0])),
                                ("discrete_max", Tag::IntArray(vec![1, 1, 1])),
                            ]));
                            explicit.insert((b.rename.first, i), id);
                        }
                    }
                }
            }
        }
        for (key, value) in c.iter_mut() {
            match (key.to_string_lossy().as_str(), value) {
                ("cached_shapes", Tag::IntArray(a)) if a.len() == self.states => {
                    for b in &self.blocks {
                        // Parallel to `states`: every combination of a template
                        // state keeps that state's shape, unless the block has
                        // no collision or declares a box for it.
                        let copy = (0..b.state_count())
                            .map(|i| {
                                if b.decl.properties.no_collision==Some(true) {return a[self.defaults["minecraft:air"] as usize];}
                                explicit
                                    .get(&(b.rename.first, i))
                                    .copied()
                                    .unwrap_or(a[b.template_id(i) as usize])
                            })
                            .collect::<Vec<_>>();
                        a.extend(copy);
                    }
                }
                ("blocks", Tag::Compound(map)) => {
                    for b in &self.blocks {
                        let Some(mut v) = map
                            .get(&JavaString::from(b.decl.template.as_str()))
                            .cloned()
                        else {
                            continue;
                        };
                        if let (Some(f), Some(slot)) =
                            (b.decl.properties.friction, v.get_mut("friction"))
                        {
                            *slot = Tag::Float(f.to_bits());
                        }
                        map.insert(b.decl.id.as_str().into(), v);
                    }
                }
                (_, inner @ Tag::Compound(_)) => self.nbt_table(inner, collision),
                _ => {}
            }
        }
    }
}

impl nbt::ListExtension for Plan<'_> {
    fn extra(&self, list: &[Tag]) -> usize {
        if list.len() == self.states {
            self.blocks.iter().map(NewBlock::state_count).sum()
        } else {
            0
        }
    }
    /// One row per template state per combination, the same order `states`
    /// numbers them in (`Rename::id`): a template state's combinations are one
    /// run, and the first of the run is where the template state maps.
    fn elements(
        &self,
        list: &[Tag],
        write: &mut dyn FnMut(&Tag) -> std::result::Result<(), nbt::Error>,
    ) -> std::result::Result<(), nbt::Error> {
        if list.len() != self.states {
            return Ok(());
        }
        for b in &self.blocks {
            for (id, combo) in b.state_combinations() {
                let mut state = list[id as usize].clone();
                if let Some(slot) = state.get_mut("id") {
                    *slot = Tag::String(b.decl.id.as_str().into());
                }
                set_nbt_properties(&mut state, combo);
                write(&state)?;
            }
        }
        Ok(())
    }
}

/// A list of per-state rows in a JSON table.
#[derive(Clone, Copy)]
struct StateList<'a> {
    table: &'a str,
    /// Its key in the document ("" when the document is the list).
    key: &'a str,
    /// Rows hold state ids, which move with the block.
    ids: bool,
    /// Gameplay rows, which get their index and the declared overrides.
    gameplay: bool,
}

/// A JSON table's per-state lists: (key, ids, gameplay), as [`StateList`].
fn state_lists(table: &str) -> &'static [(&'static str, bool, bool)] {
    match table {
        "facts.json" | "block-behaviours.json" | "mob-data.json" => &[("states", false, false)],
        "gameplay-facts.json" => &[("states", false, true)],
        "worldgen/block_flags.json" => &[("", false, false)],
        "worldgen/blocks/structure-blocks.json" => &[
            ("rotate", true, false),
            ("mirror", true, false),
            ("full_block", true, false),
        ],
        _ => &[],
    }
}

/// States turned into rows at a time by each of [`ROW_THREADS`] threads. A
/// gameplay row is about 1.5 KB, so these hold about 40 MB at once.
const ROW_CHUNK: usize = 4096;
const ROW_THREADS: usize = 6;

/// Mods read, and their files written to the root, at a time. Most of the work
/// is waiting for the file system, one call per file.
const FILE_THREADS: usize = 8;

/// The JSON and NBT tables rewritten for new blocks and items.
const JSON_TABLES: [&str; 16] = [
    "facts.json",
    "entity-types.json",
    "creative-inventory.json",
    "registries.json",
    "gameplay-facts.json",
    "block-behaviours.json",
    "model-states.json",
    "render-definitions.json",
    "landing-definitions.json",
    "items.json",
    "command-registries.json",
    "mob-data.json",
    "worldgen/block_flags.json",
    "worldgen/blocks/structure-blocks.json",
    "worldgen/blocks/survival-rules.json",
    "worldgen/blocks/survival-extra-definitions.json",
];
const NBT_TABLES: [&str; 6] = [
    "occlusion.nbt",
    "cached-collision.nbt",
    "movement-properties.nbt",
    "worldgen/blocks/block-properties.nbt",
    "worldgen/blocks/cached-support.nbt",
    "worldgen/blocks/cached-collision.nbt",
];
use horizon_items::content_namespaces::MARKER;
pub use horizon_items::content_namespaces::{namespaces, tag_namespaces};
/// Block states a world can hold. Chunk sections widen their global palette
/// past vanilla's 16 bits to fit the registry (crate::chunk::set_block_states);
/// this caps it at 20 bits, about 29 times vanilla's 35,723 states. Mods past
/// it are refused, not wrapped onto other blocks.
pub const MAX_STATES: u32 = 1 << 20;

/// Bundles of block tag files (tag id → the tag file's text).
const TAG_BUNDLES: [&str; 2] = ["block-tags.json", "worldgen/block_tags.json"];
/// The worldgen loader's bundle of biome tags.
const BIOME_TAGS: &str = "worldgen/biome_tags.json";
/// Worldgen registries a content mod may add to, and the bundle the
/// worldgen loader reads each from (id → the file's text): what structure
/// mods hold. Biomes, noise and density functions change the terrain and
/// are not read from mods.
const WORLDGEN_BUNDLES: [(&str, &str); 6] = [
    ("structure", "worldgen/structures.json"),
    ("structure_set", "worldgen/structure_sets.json"),
    ("template_pool", "worldgen/template_pools.json"),
    ("processor_list", "worldgen/processor_lists.json"),
    ("configured_feature", "worldgen/features.json"),
    ("placed_feature", "worldgen/placed_features.json"),
];

/// The registry of `command-registries.json` that each bundle's ids join,
/// for the commands that suggest them (`/locate`, `/place`).
const COMMAND_REGISTRIES: [(&str, &str); 6] = [
    ("worldgen/structures.json", "minecraft:worldgen/structure"),
    (
        "worldgen/structure_sets.json",
        "minecraft:worldgen/structure_set",
    ),
    (
        "worldgen/template_pools.json",
        "minecraft:worldgen/template_pool",
    ),
    (
        "worldgen/processor_lists.json",
        "minecraft:worldgen/processor_list",
    ),
    ("worldgen/features.json", "minecraft:worldgen/feature"),
    (
        "worldgen/placed_features.json",
        "minecraft:worldgen/placed_feature",
    ),
];

/// A mod file that adds to a worldgen bundle: (bundle, id).
fn worldgen_entry(file: &str) -> Option<(&'static str, String)> {
    let (ns, rest) = file.strip_prefix("data/")?.split_once("/worldgen/")?;
    let (kind, path) = rest.split_once('/')?;
    let (_, bundle) = WORLDGEN_BUNDLES.iter().find(|(k, _)| *k == kind)?;
    Some((bundle, format!("{ns}:{}", path.strip_suffix(".json")?)))
}

/// The key of a mod's structure set, template pool or processor list file
/// that adds its entries to other sets, pools or lists (vanilla's, another
/// mod's) instead of being one of its own. Vanilla data never has it.
const ADD_TO: &str = "horizon:add_to";
/// An addition's place among the others (lower first; 1000 by default), as
/// Lithostitched's modifiers' `priority`.
const PRIORITY: &str = "horizon:priority";
/// The bundles a mod may add to, and the list each entry goes into.
const ADDABLE: [(&str, &str); 3] = [
    ("worldgen/structure_sets.json", "structures"),
    ("worldgen/template_pools.json", "elements"),
    ("worldgen/processor_lists.json", "processors"),
];

/// A mod file's additions (`horizon:add_to`): its entries appended to the
/// list of each set, pool or processor list it names, as Lithostitched's
/// `add_structure_set_entries`, `add_template_pool_elements` and
/// `add_processor_list_processors` do for NeoForge and Fabric packs.
struct Addition {
    targets: Vec<String>,
    list: &'static str,
    entries: Vec<J>,
    priority: i64,
}
impl Addition {
    /// What a mod file of `bundle` adds, if it is an addition file.
    fn read(bundle: &str, text: &str) -> Result<Option<Self>> {
        let Some((_, list)) = ADDABLE.iter().find(|(b, _)| *b == bundle) else {
            return Ok(None);
        };
        if !text.contains(ADD_TO) {
            return Ok(None);
        }
        let doc: J = serde_json::from_str(text)?;
        let Some(targets) = doc.get(ADD_TO) else {
            return Ok(None);
        };
        let targets = match targets {
            J::Str(s) => vec![s.clone()],
            J::Arr(a) => a
                .iter()
                .map(|v| match v {
                    J::Str(s) => Some(s.clone()),
                    _ => None,
                })
                .collect::<Option<_>>()
                .ok_or(format!("{ADD_TO}: not a list of ids"))?,
            _ => return Err(format!("{ADD_TO}: not an id or a list of ids").into()),
        };
        let Some(J::Arr(entries)) = doc.get(list) else {
            return Err(format!("{ADD_TO} without a {list} list").into());
        };
        let priority = match doc.get(PRIORITY) {
            None => 1000,
            Some(J::Num(n)) => n.as_i64().ok_or(format!("{PRIORITY}: not an integer"))?,
            Some(_) => return Err(format!("{PRIORITY}: not an integer").into()),
        };
        Ok(Some(Self {
            targets,
            list,
            entries: entries.clone(),
            priority,
        }))
    }
    /// Appends the entries to each target in the bundle `doc` (id → the
    /// file's text). A target nobody defines is skipped, as a tag entry
    /// naming an unknown id is.
    fn apply(&self, doc: &mut J) -> Result<()> {
        for target in &self.targets {
            let Some(J::Str(text)) = doc.get(target) else {
                continue;
            };
            let mut v: J = serde_json::from_str(text).map_err(|e| format!("{target}: {e}"))?;
            let Some(J::Arr(list)) = v.get_mut(self.list) else {
                continue;
            };
            list.extend(self.entries.iter().cloned());
            doc.set(target, J::Str(serde_json::to_string(&v)?));
        }
        Ok(())
    }
}

/// The worldgen loader's bundles of biomes and of placed feature tags.
const BIOMES: &str = "worldgen/biomes.json";
const FEATURE_TAGS: &str = "worldgen/feature_tags.json";
const PLACED_FEATURES: &str = "worldgen/placed_features.json";

/// Rewrites the biome bundle with the mods' biome modifiers. `biome_tags`
/// and `placed` are the bundles as the mods left them (`None`: the base's).
fn place_in_biomes(
    base: &Path,
    overlay: &Overlay<'_>,
    modifiers: &[biomes::Modifier],
    biome_tags: Option<J>,
    placed: Option<J>,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let bundle = |doc: Option<J>, name: &str| -> Result<BTreeMap<String, String>> {
        let doc = match doc {
            Some(doc) => doc,
            None if base.join(name).exists() => read_json(&base.join(name))?,
            None => return Ok(BTreeMap::new()),
        };
        let J::Obj(entries) = doc else {
            return Err(format!("{name}: not an object").into());
        };
        Ok(entries
            .into_iter()
            .filter_map(|(k, v)| match v {
                J::Str(text) => Some((k, text)),
                _ => None,
            })
            .collect())
    };
    if !base.join(BIOMES).exists() {
        return Err(format!("biome modifiers need {BIOMES}").into());
    }
    let mut doc = read_json(&base.join(BIOMES))?;
    let biome_ids: BTreeSet<String> = match &doc {
        J::Obj(o) => o.iter().map(|(k, _)| k.clone()).collect(),
        _ => return Err(format!("{BIOMES}: not an object").into()),
    };
    let biome_tags = bundle(biome_tags, BIOME_TAGS)?;
    let placed: BTreeSet<String> = bundle(placed, PLACED_FEATURES)?.into_keys().collect();
    let feature_tags = bundle(None, FEATURE_TAGS)?;
    biomes::apply(
        &mut doc,
        modifiers,
        &biomes::Registry {
            what: "biome",
            ids: &biome_ids,
            tags: &biome_tags,
        },
        &biomes::Registry {
            what: "placed feature",
            ids: &placed,
            tags: &feature_tags,
        },
        warnings,
    )?;
    overlay.write(BIOMES, &serde_json::to_vec(&doc)?)
}

#[path = "mod_content_biomes.rs"]
mod biomes;

/// The overlay being written: `out` mirrors `base` through links, and a
/// directory becomes real only when something inside it is written.
struct Overlay<'a> {
    base: &'a Path,
    out: &'a Path,
    /// The directories known to be real, not to ask the file system for each
    /// file: mods write hundreds of thousands, from several threads.
    real: Mutex<HashSet<PathBuf>>,
}
impl Overlay<'_> {
    fn new<'a>(base: &'a Path, out: &'a Path) -> Result<Overlay<'a>> {
        if fs::symlink_metadata(out).is_ok() {
            fs::remove_dir_all(out)?;
        }
        fs::create_dir_all(out)?;
        let overlay = Overlay {
            base,
            out,
            real: Mutex::default(),
        };
        overlay.link_children(Path::new(""))?;
        Ok(overlay)
    }
    fn link_children(&self, rel: &Path) -> Result<()> {
        let Ok(read) = fs::read_dir(self.base.join(rel)) else {
            return Ok(());
        };
        for entry in read.flatten() {
            let rel = rel.join(entry.file_name());
            // The base's files in one file: it describes the base, not this root (and
            // would be copied where links are not made).
            if rel == Path::new(horizon_core::prepared_files::FILE_NAME) {
                continue;
            }
            link(&self.base.join(&rel), &self.out.join(&rel))?;
        }
        Ok(())
    }
    /// Makes `rel`'s directories real, each holding links to the base's.
    fn dir(&self, rel: &Path) -> Result<()> {
        let mut real = self.real.lock().unwrap_or_else(|e| e.into_inner());
        if real.contains(rel) {
            return Ok(());
        }
        let mut at = PathBuf::new();
        for part in rel.iter() {
            at.push(part);
            if real.contains(&at) {
                continue;
            }
            let path = self.out.join(&at);
            let meta = fs::symlink_metadata(&path);
            if !meta
                .as_ref()
                .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
            {
                if meta.is_ok() {
                    remove_link(&path)?;
                }
                fs::create_dir(&path)?;
                self.link_children(&at)?;
            }
            real.insert(at.clone());
        }
        Ok(())
    }
    /// A new file at `rel`, where a link to the base's may be. The link goes
    /// first: writing through it would change the base's file.
    fn create(&self, rel: &str) -> Result<fs::File> {
        let rel = Path::new(rel);
        if let Some(parent) = rel.parent() {
            self.dir(parent)?;
        }
        let path = self.out.join(rel);
        let open = || fs::OpenOptions::new().write(true).create_new(true).open(&path);
        Ok(match open() {
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                fs::remove_file(&path)?;
                open()?
            }
            file => file?,
        })
    }
    fn write(&self, rel: &str, bytes: &[u8]) -> Result<()> {
        self.create(rel)?.write_all(bytes)?;
        Ok(())
    }
    /// A link at `rel` to the file `target`, where a link to the base's may be.
    /// What is there goes first: [`link`] falls back to a copy, and a copy onto
    /// a hard link would change the base's file.
    fn link_file(&self, target: &Path, rel: &str) -> Result<()> {
        let rel = Path::new(rel);
        if let Some(parent) = rel.parent() {
            self.dir(parent)?;
        }
        let path = self.out.join(rel);
        match fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
        link(target, &path)?;
        Ok(())
    }
}
fn out_path(overlay: &Overlay<'_>, rel: &str) -> PathBuf {
    overlay.out.join(rel)
}
#[cfg(unix)]
fn link(target: &Path, at: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, at)
}
/// Windows: a directory junction and a hard link need no privileges. Only on
/// another volume does a file need a copy, and only if junctions fail a tree.
#[cfg(windows)]
fn link(target: &Path, at: &Path) -> std::io::Result<()> {
    if target.is_dir() {
        if horizon_threads::junction(target, at).is_ok() {
            return Ok(());
        }
        fs::create_dir_all(at)?;
        for entry in fs::read_dir(target)?.flatten() {
            link(&entry.path(), &at.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        fs::hard_link(target, at).or_else(|_| fs::copy(target, at).map(|_| ()))
    }
}
/// A link [`link`] made to a directory or a file. Windows removes a junction
/// or a directory symlink as a directory.
fn remove_link(path: &Path) -> std::io::Result<()> {
    fs::remove_file(path).or_else(|e| fs::remove_dir(path).map_err(|_| e))
}

/// A tag file's JSON text with `more` appended to its values (no repeats).
fn merge_tag(text: Option<&str>, more: &[J], what: &str) -> Result<String> {
    let mut doc = match text {
        Some(t) => serde_json::from_str(t).map_err(|e| format!("{what}: {e}"))?,
        None => J::Obj(vec![("values".into(), J::Arr(Vec::new()))]),
    };
    let Some(J::Arr(values)) = doc.get_mut("values") else {
        return Err(format!("{what}: no values").into());
    };
    // A set, not `values.contains`: ATM10's big tags (`minecraft:mineable/pickaxe`, `c:...`) hold
    // thousands of entries from hundreds of mods, and the scan made the merge quadratic (about 40 s of
    // the content root's build).
    let mut seen: std::collections::HashSet<J> = values.iter().cloned().collect();
    for v in more {
        if seen.insert(v.clone()) {
            values.push(v.clone());
        }
    }
    Ok(serde_json::to_string_pretty(&doc)?)
}

/// The block and item ids and tags a content root has, for tag entries.
struct Known {
    blocks: std::collections::BTreeSet<String>,
    items: std::collections::BTreeSet<String>,
    /// The worldgen loader's biomes and biome tags.
    biomes: std::collections::BTreeSet<String>,
    biome_tags: std::collections::BTreeSet<String>,
    /// `data/<ns>/tags/<kind>/<path>.json` files the mods add.
    tag_files: std::collections::BTreeSet<String>,
}
impl Known {
    fn new(base: &Path, plan: &Plan, tags: &BTreeMap<String, TagMerge>) -> Result<Self> {
        let mut blocks: std::collections::BTreeSet<String> =
            plan.defaults.keys().cloned().collect();
        blocks.extend(plan.blocks.iter().map(|b| b.decl.id.clone()));
        let mut items = std::collections::BTreeSet::new();
        if let J::Obj(o) = read_json(&base.join("items.json"))? {
            items.extend(o.into_iter().map(|(k, _)| k));
        }
        items.extend(plan.items.iter().map(|i| i.rename.to.clone()));
        let keys = |name: &str| -> Result<std::collections::BTreeSet<String>> {
            Ok(match base.join(name).exists() {
                true => match read_json(&base.join(name))? {
                    J::Obj(o) => o.into_iter().map(|(k, _)| k).collect(),
                    _ => Default::default(),
                },
                false => Default::default(),
            })
        };
        Ok(Known {
            blocks,
            items,
            biomes: keys("worldgen/biomes.json")?,
            biome_tags: keys(BIOME_TAGS)?,
            tag_files: tags.keys().cloned().collect(),
        })
    }
    /// Whether tag `file` keeps entry `v`: a known id or an existing tag,
    /// for block, item and biome tags (other registries are kept as they
    /// are). The worldgen loader refuses a biome tag naming what it lacks
    /// (another mod's biomes, a loader's tags), so those entries go.
    fn keeps(&self, file: &str, v: &J) -> bool {
        let id = match v {
            J::Str(s) => s.as_str(),
            J::Obj(_) => match v.get("id") {
                Some(J::Str(s)) => s.as_str(),
                _ => return true,
            },
            _ => return true,
        };
        let Some(rest) = file
            .strip_prefix("data/")
            .and_then(|r| r.split_once("/tags/"))
            .map(|(_, rest)| rest)
        else {
            return true;
        };
        let kind = match rest.strip_prefix("worldgen/biome/") {
            Some(_) => "worldgen/biome",
            None => rest.split_once('/').map_or("", |(k, _)| k),
        };
        let ids = match kind {
            "block" => &self.blocks,
            "item" => &self.items,
            "worldgen/biome" => &self.biomes,
            _ => return true,
        };
        match id.strip_prefix('#') {
            Some(tag) => {
                let (tns, path) = tag.split_once(':').unwrap_or(("minecraft", tag));
                let rel = format!("data/{tns}/tags/{kind}/{path}.json");
                self.tag_files.contains(&rel)
                    || match kind {
                        "worldgen/biome" => self.biome_tags.contains(&format!("{tns}:{path}")),
                        _ => tns == "minecraft",
                    }
            }
            None => ids.contains(id),
        }
    }
}

/// What the mods do to one tag file, in load order (a data pack's
/// TagLoader): each adds its values, and one that replaces the tag first
/// drops vanilla's entries and the mods' before it.
#[derive(Default)]
struct TagMerge {
    /// A mod replaced the tag: vanilla's entries are left out.
    replaced: bool,
    /// The mods whose values are in `values`.
    mods: Vec<String>,
    values: Vec<J>,
}

impl TagMerge {
    /// Adds one mod's tag file. A replace that drops entries (vanilla's in a
    /// `shared` tag, one not in the mod's namespaces, or other mods') warns.
    fn add(
        &mut self,
        id: &str,
        file: &str,
        replace: bool,
        shared: bool,
        values: &[J],
    ) -> Option<String> {
        let mut warning = None;
        if replace {
            let mut dropped: Vec<String> = Vec::new();
            if shared && !self.replaced {
                dropped.push("vanilla's entries".into());
            }
            if !self.mods.is_empty() {
                dropped.push(format!("those of {}", self.mods.join(", ")));
            }
            if !dropped.is_empty() {
                warning = Some(format!(
                    "mod {id} replaces {file}: {} are left out, as a data pack's replace \
                     does; the mods after it in load order still add to it",
                    dropped.join(" and ")
                ));
            }
            self.replaced = true;
            self.mods.clear();
            self.values.clear();
        }
        if !self.mods.iter().any(|m| m == id) {
            self.mods.push(id.to_owned());
        }
        self.values.extend(values.iter().cloned());
        warning
    }
}

/// Rewrites the base's per-state tables for the mods' states into the overlay,
/// one table at a time: each holds vanilla's rows as a tree while its new rows
/// go straight to its file.
fn tables(base: &Path, plan: &Plan, overlay: &Overlay<'_>) -> Result<()> {
    for name in JSON_TABLES.iter().chain(&NBT_TABLES) {
        let path = base.join(name);
        if !path.exists() {
            continue;
        }
        let mut out = std::io::BufWriter::with_capacity(1 << 20, overlay.create(name)?);
        if *name == "entity-types.json" {
            let bytes = plan
                .entity_types_json(&fs::read(&path)?)
                .map_err(|e| format!("{name}: {e}"))?;
            out.write_all(&bytes)?;
        } else if JSON_TABLES.contains(name) {
            let mut doc = read_json(&path)?;
            plan.write_json(name, &mut doc, &mut out)
                .map_err(|e| format!("{name}: {e}"))?;
        } else {
            let (mut named, _) = nbt::read_named(&fs::read(&path)?, Limits::tables())
                .map_err(|e| format!("{name}: {e}"))?;
            plan.nbt_table(&mut named.tag, name.ends_with("cached-collision.nbt"));
            if *name == "cached-collision.nbt" || *name == "worldgen/blocks/cached-collision.nbt" {
                plan.collision_shapes(&mut named.tag)?;
            }
            nbt::write_named_to(&named, Limits::tables(), &mut out, "states", plan)
                .map_err(|e| format!("{name}: {e}"))?;
        }
        out.flush()?;
    }
    Ok(())
}

/// What one mod's files add to the root besides themselves.
#[derive(Default)]
struct ModFiles {
    /// Tag files: (file, replace, shared, values).
    tags: Vec<(String, bool, bool, Vec<J>)>,
    /// Worldgen files: (bundle, id, text).
    worldgen: Vec<(&'static str, String, String)>,
    additions: Vec<(&'static str, Addition)>,
    modifiers: Vec<biomes::Modifier>,
    /// What reading its biome modifiers warned of.
    warnings: Vec<String>,
}

/// Writes one mod's files to the overlay and gathers what must be merged in
/// mod order. A mod's files are in its own namespaces (shared tags are only
/// merged), so mods can go side by side.
fn write_mod_files(m: &ContentMod, overlay: &Overlay<'_>) -> Result<ModFiles> {
    let mut gathered = ModFiles::default();
    let declarations = format!("data/{}/horizon/", m.id);
    // A mod in a folder is linked, not copied: a large pack is over half a
    // million small files, and a copy took 18 minutes of a 20 minute build.
    let folder = match m.content.source() {
        source if source.is_dir() => Some(std::path::absolute(source)?),
        _ => None,
    };
    let whole = match &folder {
        Some(folder) => link_whole_dirs(m, folder, overlay)?,
        None => HashSet::new(),
    };
    let mut reader = m.content.reader()?;
    for file in &m.content.files {
        if is_tag_file(file) && file.ends_with(".json") {
            let bytes = reader.read(file)?;
            let doc: J = serde_json::from_slice(&bytes).map_err(|e| format!("{file}: {e}"))?;
            let Some(J::Arr(values)) = doc.get("values") else {
                return Err(format!("{}: {file}: no values", m.id).into());
            };
            let replace = doc.get("replace") == Some(&J::Bool(true));
            let ns = file.split('/').nth(1).unwrap_or_default();
            let shared = !m.content.namespaces.iter().any(|n| n == ns);
            gathered
                .tags
                .push((file.clone(), replace, shared, values.clone()));
            continue;
        }
        if file.starts_with(&declarations)
            || whole_dir(file).is_some_and(|dir| whole.contains(dir))
        {
            continue;
        }
        let modifier = biomes::is_modifier(file);
        let worldgen = worldgen_entry(file);
        // Only a biome modifier's or a worldgen file's text is needed here.
        let bytes = if folder.is_none() || modifier || worldgen.is_some() {
            Some(reader.read(file)?)
        } else {
            None
        };
        let place = |rel: &str| -> Result<()> {
            match (&folder, &bytes) {
                (Some(folder), _) => overlay.link_file(&folder.join(file), rel),
                (None, Some(bytes)) => overlay.write(rel, bytes),
                (None, None) => Ok(()),
            }
        };
        place(file)?;
        if modifier {
            let bytes = bytes.ok_or("a biome modifier was not read")?;
            let text = String::from_utf8(bytes).map_err(|e| format!("{file}: {e}"))?;
            let read = biomes::Modifier::read(&m.id, file, &text, &mut gathered.warnings)?;
            gathered.modifiers.extend(read);
        } else if let Some((bundle, id)) = worldgen {
            let bytes = bytes.ok_or("a worldgen file was not read")?;
            let text = String::from_utf8(bytes).map_err(|e| format!("{file}: {e}"))?;
            match Addition::read(bundle, &text).map_err(|e| format!("{file}: {e}"))? {
                Some(addition) => gathered.additions.push((bundle, addition)),
                None => gathered.worldgen.push((bundle, id, text)),
            }
        } else if let Some(template) = structure_template(file) {
            place(&template)?;
        }
    }
    Ok(gathered)
}

/// A tag file, whose values are merged and whose text is not kept.
fn is_tag_file(file: &str) -> bool {
    file.starts_with("data/") && file.contains("/tags/")
}

/// Where the worldgen loader wants a mod's structure file:
/// `worldgen/structure/<ns>/<path>.nbt`.
fn structure_template(file: &str) -> Option<String> {
    file.strip_prefix("data/")
        .and_then(|r| r.split_once("/structure/"))
        .filter(|(_, path)| path.ends_with(".nbt"))
        .map(|(ns, path)| format!("worldgen/structure/{ns}/{path}"))
}

/// The directory a mod's `file` is linked with: `assets/<ns>/<kind>` or
/// `data/<ns>/<kind>`, unless the build reads or writes in that kind.
fn whole_dir(file: &str) -> Option<&str> {
    let mut parts = file.splitn(4, '/');
    let (top, _ns, kind, rest) = (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    let linked = match top {
        // A block without a blockstate gets one written.
        "assets" => kind != "blockstates",
        // Merged, gathered, copied a second time and declarations.
        "data" => !matches!(kind, "tags" | "worldgen" | "structure" | "horizon"),
        _ => false,
    };
    linked.then(|| &file[..file.len() - rest.len() - 1])
}

/// Links each [`whole_dir`] of a folder mod into the overlay in one go and
/// returns them. The base's own directories stay out, and so does any holding a
/// file the build takes in as more than a file: a tag is written back, which
/// must never reach through to the mod, and a biome modifier, a worldgen file or
/// a structure has to be gone through file by file.
fn link_whole_dirs<'f>(
    m: &'f ContentMod,
    folder: &Path,
    overlay: &Overlay<'_>,
) -> Result<HashSet<&'f str>> {
    let mut dirs: HashSet<&str> = m.content.files.iter().filter_map(|f| whole_dir(f)).collect();
    let taken_in = |f: &&String| {
        is_tag_file(f)
            || biomes::is_modifier(f)
            || worldgen_entry(f).is_some()
            || structure_template(f).is_some()
    };
    for file in m.content.files.iter().filter(taken_in) {
        if let Some(dir) = whole_dir(file) {
            dirs.remove(dir);
        }
    }
    dirs.retain(|dir| !overlay.base.join(dir).exists());
    for dir in &dirs {
        let rel = Path::new(dir);
        if let Some(parent) = rel.parent() {
            overlay.dir(parent)?;
        }
        link(&folder.join(rel), &overlay.out.join(rel))?;
    }
    Ok(dirs)
}

/// Builds the content root for `mods` over `base` in `out`. Returns the
/// warnings for the log: the shared tags a mod replaced.
pub fn build(base: &Path, mods: &[ContentMod], out: &Path) -> Result<Vec<String>> {
    // Two mods owning one namespace would mix their files.
    for (i, a) in mods.iter().enumerate() {
        for b in &mods[..i] {
            let (b, a) = (
                (b.id.as_str(), b.content.namespaces.as_slice()),
                (a.id.as_str(), a.content.namespaces.as_slice()),
            );
            if let Some(e) = horizon_mods::content::clash(b, a) {
                return Err(e.into());
            }
        }
    }
    // The overlay links to the base: a relative base would leave them dangling.
    let base = &std::path::absolute(base)?;
    let plan = Plan::new(base, mods)?;
    let overlay = Overlay::new(base, out)?;
    tables(base, &plan, &overlay)?;
    // Mod files, and every tag addition gathered per tag file.
    let mut tags: BTreeMap<String, TagMerge> = BTreeMap::new();
    let mut warnings = Vec::new();
    let mut worldgen: BTreeMap<&str, Vec<(String, String)>> = BTreeMap::new();
    let mut additions: BTreeMap<&str, Vec<Addition>> = BTreeMap::new();
    let mut modifiers = Vec::new();
    let files = par_map(mods, FILE_THREADS, |m| {
        write_mod_files(m, &overlay).map_err(|e| e.to_string())
    });
    for (m, files) in mods.iter().zip(files) {
        let files = files?;
        for (file, replace, shared, values) in files.tags {
            let merge = tags.entry(file.clone()).or_default();
            warnings.extend(merge.add(&m.id, &file, replace, shared, &values));
        }
        for (bundle, id, text) in files.worldgen {
            worldgen.entry(bundle).or_default().push((id, text));
        }
        for (bundle, addition) in files.additions {
            // A bundle that is only added to is rewritten too.
            worldgen.entry(bundle).or_default();
            additions.entry(bundle).or_default().push(addition);
        }
        modifiers.extend(files.modifiers);
        warnings.extend(files.warnings);
    }
    // Only their values are needed from here on.
    horizon_threads::release_free_memory();
    // A tag naming an id nobody registers fails as a whole in vanilla, and
    // carried-over mods name blocks left out or other mods' ids: such
    // entries are dropped instead, so the rest of the tag holds.
    let known = Known::new(base, &plan, &tags)?;
    for (file, merge) in &mut tags {
        merge.values.retain(|v| known.keeps(file, v));
    }
    let mut bundles: Vec<(&str, J)> = Vec::new();
    for name in TAG_BUNDLES {
        if base.join(name).exists() {
            bundles.push((name, read_json(&base.join(name))?));
        }
    }
    let mut bundle_keys: Vec<(&str, Keyed)> = bundles
        .iter_mut()
        .filter_map(|(name, doc)| Some((*name, Keyed::new(doc)?)))
        .collect();
    for (file, merge) in &tags {
        let vanilla = fs::read_to_string(base.join(file))
            .ok()
            .filter(|_| !merge.replaced);
        overlay.write(
            file,
            merge_tag(vanilla.as_deref(), &merge.values, file)?.as_bytes(),
        )?;
        // data/<ns>/tags/block/<path>.json is the block tag <ns>:<path>.
        let Some((ns, path)) = file
            .strip_prefix("data/")
            .and_then(|r| r.split_once("/tags/block/"))
            .and_then(|(ns, p)| Some((ns, p.strip_suffix(".json")?)))
        else {
            continue;
        };
        let id = format!("{ns}:{path}");
        for (name, doc) in &mut bundle_keys {
            let text = match doc.get(&id) {
                Some(J::Str(t)) if !merge.replaced => Some(t.clone()),
                _ => None,
            };
            let merged = merge_tag(text.as_deref(), &merge.values, name)?;
            doc.set(&id, J::Str(merged));
        }
    }
    drop(bundle_keys);
    for (name, doc) in &bundles {
        overlay.write(name, &serde_json::to_vec(doc)?)?;
    }
    // Biome tags, merged into the worldgen loader's bundle as block tags are.
    let biome_tags: Vec<(String, &TagMerge)> = tags
        .iter()
        .filter_map(|(file, merge)| {
            let (ns, path) = file
                .strip_prefix("data/")?
                .split_once("/tags/worldgen/biome/")?;
            Some((format!("{ns}:{}", path.strip_suffix(".json")?), merge))
        })
        .collect();
    let mut biome_tag_doc = None;
    if !biome_tags.is_empty() && base.join(BIOME_TAGS).exists() {
        let mut doc = read_json(&base.join(BIOME_TAGS))?;
        let mut keyed = Keyed::new(&mut doc).ok_or("the biome tags are not an object")?;
        for (id, merge) in biome_tags {
            let text = match keyed.get(&id) {
                Some(J::Str(t)) if !merge.replaced => Some(t.clone()),
                _ => None,
            };
            let merged = merge_tag(text.as_deref(), &merge.values, BIOME_TAGS)?;
            keyed.set(&id, J::Str(merged));
        }
        overlay.write(BIOME_TAGS, &serde_json::to_vec(&doc)?)?;
        biome_tag_doc = Some(doc);
    }
    // Structures and the pools, sets, processors and features they use, then
    // what mods add to sets, pools and lists, by priority, then in mod and
    // file order.
    for list in additions.values_mut() {
        list.sort_by_key(|a| a.priority);
    }
    let mut placed_doc = None;
    for (bundle, entries) in &worldgen {
        if !base.join(bundle).exists() {
            continue;
        }
        let mut doc = read_json(&base.join(bundle))?;
        let mut keyed = Keyed::new(&mut doc).ok_or("a worldgen bundle is not an object")?;
        for (id, text) in entries {
            keyed.set(id, J::Str(text.clone()));
        }
        for addition in additions.get(bundle).into_iter().flatten() {
            addition
                .apply(&mut doc)
                .map_err(|e| format!("{bundle}: {e}"))?;
        }
        overlay.write(bundle, &serde_json::to_vec(&doc)?)?;
        if *bundle == PLACED_FEATURES {
            placed_doc = Some(doc);
        }
    }
    if !modifiers.is_empty() {
        place_in_biomes(
            base,
            &overlay,
            &modifiers,
            biome_tag_doc,
            placed_doc,
            &mut warnings,
        )?;
    }
    // The commands suggest ids from `command-registries.json`: add the
    // structures, pools, sets and features the mods bring.
    let registries = out_path(&overlay, "command-registries.json");
    if let Ok(mut doc) = read_json(&registries) {
        // Registry names hold `/`, so each key is looked up by itself.
        let mut add = |keys: &[&str], new: &[J]| {
            let found = keys.iter().try_fold(&mut doc, |j, k| j.get_mut(k));
            if let Some(J::Arr(ids)) = found {
                // Ids are strings: only a `J::Str` is looked up by its text.
                let text = |j: &J| match j {
                    J::Str(s) => Some(s.clone()),
                    _ => None,
                };
                let mut have: HashSet<String> = ids.iter().filter_map(text).collect();
                for id in new {
                    if text(id).is_none_or(|t| have.insert(t)) {
                        ids.push(id.clone());
                    }
                }
            }
        };
        for (bundle, registry) in COMMAND_REGISTRIES {
            let new: Vec<J> = worldgen
                .get(bundle)
                .into_iter()
                .flatten()
                .map(|(id, _)| J::Str(id.clone()))
                .collect();
            add(&["by_registry", registry, "ids"], &new);
        }
        overlay.write("command-registries.json", &serde_json::to_vec(&doc)?)?;
    }
    // A block without its own blockstate looks like its template, so every
    // state still has a model.
    let mut templates: HashMap<&str, Vec<u8>> = HashMap::new();
    for m in mods {
        for block in &m.content.blocks {
            let (ns, path) = block.id.split_once(':').unwrap_or((&m.id, &block.id));
            let own = format!("assets/{ns}/blockstates/{path}.json");
            if m.content.files.binary_search(&own).is_ok() {
                continue;
            }
            let template = block.template.trim_start_matches("minecraft:");
            if !templates.contains_key(template) {
                let vanilla = base.join(format!("assets/minecraft/blockstates/{template}.json"));
                templates.insert(template, fs::read(&vanilla)?);
            }
            overlay.write(&own, &templates[template])?;
        }
    }
    let mut namespaces: Vec<&str> = mods
        .iter()
        .flat_map(|m| m.content.namespaces.iter().map(String::as_str))
        .collect();
    namespaces.sort();
    overlay.write(MARKER, &serde_json::to_vec(&namespaces)?)?;
    let mod_ids: Vec<&str> = mods.iter().map(|m| m.id.as_str()).collect();
    overlay.write(
        horizon_items::content_namespaces::MOD_IDS_MARKER,
        &serde_json::to_vec(&mod_ids)?,
    )?;
    let remainders: BTreeMap<&str, &str> = plan
        .items
        .iter()
        .filter_map(|i| Some((i.rename.to.as_str(), i.crafting_remainder.as_deref()?)))
        .collect();
    if !remainders.is_empty() {
        overlay.write(
            horizon_items::items::MOD_CRAFT_REMAINDERS,
            &serde_json::to_vec(&remainders)?,
        )?;
    }
    // clients must agree on the block states and the items', mob effects',
    // entity types' and menu types' network ids.
    if !plan.blocks.is_empty()
        || !plan.items.is_empty()
        || !plan.effects.is_empty()
        || !plan.entity_types.is_empty()
        || !plan.menu_types.is_empty()
    {
        overlay.write(crate::content_identity::MARKER, b"1")?;
    }
    overlay.write(WARNINGS, warnings.join("\n").as_bytes())?;
    Ok(warnings)
}

/// A mod that wants its content in the root.
pub struct Candidate {
    pub path: PathBuf,
    pub manifest: horizon_mods::Manifest,
}

/// The root [`load_root`] found or built.
pub struct LoadedRoot {
    pub root: PathBuf,
    /// The mods in it.
    pub mods: usize,
    /// The ids of the mods in it, for what a server announces to joining clients.
    pub accepted: Vec<String>,
    /// Their mob effects, entity types and menu types, in registry order.
    pub effects: Vec<MobEffect>,
    pub entity_types: Vec<EntityType>,
    pub menu_types: Vec<MenuType>,
}

/// What a built root's start needs to know without reading the mods again.
#[derive(Serialize, Deserialize)]
struct Summary {
    mods: usize,
    /// The ids of the mods in it.
    accepted: Vec<String>,
    /// The mods left out: (id, why).
    refused: Vec<(String, String)>,
    /// (id, declaration), as [`mob_effects`], [`entity_types`] and [`menu_types`]
    /// list them: a declaration's id isn't part of its file.
    effects: Vec<(String, MobEffect)>,
    entity_types: Vec<(String, EntityType)>,
    menu_types: Vec<(String, MenuType)>,
}

/// In a root built by [`load_root`]: its [`Summary`].
const SUMMARY: &str = "horizon-mod-content-summary.json";

/// The root for `candidates` over `base`, in `cache/mod-content`: as
/// [`resource_root`], and a root still good is used without reading the mods'
/// files at all. A mod whose content is refused is left out and logged.
pub fn load_root(
    base: &Path,
    candidates: &[Candidate],
    blocks: &[horizon_mods::content::Block],
    items: &[horizon_mods::content::Item],
    cache: &Path,
) -> Result<LoadedRoot> {
    let base_root = |accepted: Vec<String>| LoadedRoot {
        root: base.to_owned(),
        mods: 0,
        accepted,
        effects: Vec::new(),
        entity_types: Vec::new(),
        menu_types: Vec::new(),
    };
    if candidates.is_empty() && blocks.is_empty() && items.is_empty() {
        return Ok(base_root(Vec::new()));
    }
    let out = cache.join("mod-content");
    let started = std::time::Instant::now();
    let stamps = par_map(candidates, FILE_THREADS, |c| {
        horizon_mods::content::source_stamp(&c.path).unwrap_or_default()
    });
    let stamp = stamp(
        base,
        candidates
            .iter()
            .zip(stamps)
            .map(|(c, files)| (c.manifest.id.as_str(), c.path.as_path(), files)),
        &format!("{blocks:?} {items:?}"),
    )?;
    if fs::read(out.join(STAMP)).is_ok_and(|s| s == stamp.as_bytes())
        && let Ok(bytes) = fs::read(out.join(SUMMARY))
        && let Ok(summary) = serde_json::from_slice::<Summary>(&bytes)
    {
        log_reused(&out);
        for (id, why) in &summary.refused {
            log::warn!("Horizon: mod {id} content not loaded: {why}");
        }
        log::info!(
            "Horizon: content root of {} mod(s) reused, checked in {:.2} s",
            summary.mods,
            started.elapsed().as_secs_f64()
        );
        let effects = summary.effects.into_iter();
        let types = summary.entity_types.into_iter();
        let menus = summary.menu_types.into_iter();
        return Ok(LoadedRoot {
            root: out,
            mods: summary.mods,
            accepted: summary.accepted,
            effects: effects.map(|(id, e)| MobEffect { id, ..e }).collect(),
            entity_types: types.map(|(id, t)| EntityType { id, ..t }).collect(),
            menu_types: menus.map(|(id, m)| MenuType { id, ..m }).collect(),
        });
    }
    let read = par_map(candidates, FILE_THREADS, |c| {
        // The items its native code declares may be a file item's crafting remainder.
        let declared: Vec<String> = items
            .iter()
            .filter(|i| i.id.split_once(':').is_some_and(|(ns, _)| ns == c.manifest.id))
            .map(|i| i.id.clone())
            .collect();
        horizon_mods::Content::read_declaring(&c.path, &c.manifest, &declared)
    });
    log::info!(
        "Horizon: read the content of {} mod(s) in {:.1} s",
        candidates.len(),
        started.elapsed().as_secs_f64()
    );
    let known = known_components(base)?;
    let mut mods: Vec<ContentMod> = Vec::new();
    let mut refused = Vec::new();
    for (c, content) in candidates.iter().zip(read) {
        let id = &c.manifest.id;
        // The first of two mods owning one namespace stays on.
        let accepted = match content {
            Ok(content) => accept_mod(
                &mut mods,
                ContentMod {
                    id: id.clone(),
                    content,
                },
                &known,
            )
            .map_err(|e| e.to_string()),
            Err(why) => Err(why),
        };
        if let Err(why) = accepted {
            log::warn!("Horizon: mod {id} content not loaded: {why}");
            refused.push((id.clone(), why));
        }
    }
    if !blocks.is_empty() || !items.is_empty() {
        for e in accept_declared(&mut mods, blocks, items, &known) {
            log::warn!("Horizon: {e}");
        }
        // A file item's remainder named a code-declared item that was refused: that mod alone
        // is left out, not every content mod through the overlay's check.
        for e in drop_dangling_remainders(&mut mods) {
            log::warn!("Horizon: {e}");
        }
    }
    let accepted: Vec<String> = mods.iter().map(|m| m.id.clone()).collect();
    if mods
        .iter()
        .all(|m| m.content.is_empty() && m.content.blocks.is_empty() && m.content.items.is_empty())
    {
        return Ok(base_root(accepted));
    }
    let effects: Vec<MobEffect> = mob_effects(&mods).into_iter().cloned().collect();
    let types: Vec<EntityType> = entity_types(&mods).into_iter().cloned().collect();
    let menus: Vec<MenuType> = menu_types(&mods).into_iter().cloned().collect();
    let started = std::time::Instant::now();
    for warning in build(base, &mods, &out)? {
        log::warn!("Horizon: {warning}");
    }
    log::info!(
        "Horizon: content root built in {:.1} s",
        started.elapsed().as_secs_f64()
    );
    let summary = Summary {
        mods: mods.len(),
        accepted: accepted.clone(),
        refused,
        effects: effects.iter().map(|e| (e.id.clone(), e.clone())).collect(),
        entity_types: types.iter().map(|t| (t.id.clone(), t.clone())).collect(),
        menu_types: menus.iter().map(|m| (m.id.clone(), m.clone())).collect(),
    };
    fs::write(out.join(SUMMARY), serde_json::to_vec(&summary)?)?;
    fs::write(out.join(STAMP), stamp)?;
    Ok(LoadedRoot {
        root: out,
        mods: mods.len(),
        accepted,
        effects,
        entity_types: types,
        menu_types: menus,
    })
}

/// A reused root warns as the build did.
fn log_reused(out: &Path) {
    let warnings = fs::read_to_string(out.join(WARNINGS)).unwrap_or_default();
    warnings.lines().for_each(|w| log::warn!("Horizon: {w}"));
}

/// The root to load: `base` itself with no content, else the mods' overlay
/// over it, built in `cache/mod-content`.
///
/// The root is built once and kept: later starts with the same build, base
/// and mod files reuse it.
pub fn resource_root(base: &Path, mods: &[ContentMod], cache: &Path) -> Result<PathBuf> {
    if mods
        .iter()
        .all(|m| m.content.is_empty() && m.content.blocks.is_empty() && m.content.items.is_empty())
    {
        return Ok(base.to_owned());
    }
    let out = cache.join("mod-content");
    let stamps = mods
        .iter()
        .map(|m| {
            let source = m.content.source();
            // Declared in code only: nothing to read.
            let files = match m.content.files.is_empty() {
                true => 0,
                false => horizon_mods::content::source_stamp(source)?,
            };
            Ok((m.id.as_str(), source, files))
        })
        .collect::<std::result::Result<Vec<_>, String>>()?;
    // What each declares, so blocks and items from code count too.
    let declared: String = mods
        .iter()
        .map(|m| format!("{:?} {:?}", m.content.blocks, m.content.items))
        .collect();
    let stamp = stamp(base, stamps, &declared)?;
    if fs::read(out.join(STAMP)).is_ok_and(|s| s == stamp.as_bytes()) {
        log_reused(&out);
        return Ok(out);
    }
    for warning in build(base, mods, &out)? {
        log::warn!("Horizon: {warning}");
    }
    fs::write(out.join(STAMP), stamp)?;
    Ok(out)
}

/// In a root built by [`build`]: its warnings, one per line.
const WARNINGS: &str = "horizon-mod-content-warnings.txt";

/// In a root built by [`resource_root`]: what it was built from.
const STAMP: &str = "horizon-mod-content.stamp";

/// What a content root is built from: this build of the game, the base's
/// manifest and the tables [`build`] rewrites, and every mod in order with its
/// place and its files' stamp ([`horizon_mods::content::source_stamp`]): a root
/// links to a mod in a folder, so a moved mod needs a new root.
fn stamp<'a>(
    base: &Path,
    mods: impl IntoIterator<Item = (&'a str, &'a Path, u64)>,
    declared: &str,
) -> Result<String> {
    use sha2::{Digest, Sha256};
    let meta = |path: &Path| -> Result<String> {
        let m = fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let modified = m
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        Ok(format!(
            "{} {} {}",
            path.display(),
            m.len(),
            modified.as_nanos()
        ))
    };
    let mut parts = vec!["horizon mod content 4".to_owned()];
    parts.push(meta(&std::env::current_exe()?)?);
    let base = std::path::absolute(base)?;
    parts.push(
        String::from_utf8_lossy(&fs::read(base.join("horizon-resources.json"))?).into_owned(),
    );
    for name in JSON_TABLES.iter().chain(&NBT_TABLES).chain(&TAG_BUNDLES) {
        if base.join(name).exists() {
            parts.push(meta(&base.join(name))?);
        }
    }
    for (id, path, files) in mods {
        // A mod declared in code only is nowhere.
        let place = match path.as_os_str().is_empty() {
            true => PathBuf::new(),
            false => std::path::absolute(path)?,
        };
        parts.push(format!("{id} {} {files}", place.display()));
    }
    // What native code declares (the mods' files have their own stamps above).
    parts.push(declared.to_owned());
    let mut hasher = Sha256::new();
    for part in &parts {
        hasher.update((part.len() as u64).to_le_bytes());
        hasher.update(part.as_bytes());
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
#[path = "mod_content_tests.rs"]
mod tests;
