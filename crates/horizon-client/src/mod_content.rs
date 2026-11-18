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
use horizon_core::{
    nbt,
    nbt::{Compound, JavaString, Limits, Tag},
};
use horizon_mod_package::content::{
    Block, BlockEntityType, BlockShapes, Content, CreativeTab, EntityType, FluidLook, MenuType,
    MobEffect, Rarity, StateProperty, StateValues,
};
use horizon_mod_package::manifest::{
    JarInputsError, JarRef, collect_jar_requirements, missing_jars,
};
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

/// A content mod to put in the root: its id, its content, and the Java jars
/// it lists in manifest order — each an asset/data layer under the mod's own
/// files, its file found by hash in `jar_store` (modding book's manifest reference).
pub struct ContentMod {
    pub id: String,
    pub content: Content,
    pub jars: Vec<JarRef>,
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

/// The mods' fluids in registry order, by id as effects are. Each takes two
/// network ids after vanilla's, its source's then its flowing form's.
pub fn fluids(mods: &[ContentMod]) -> Vec<&FluidLook> {
    let mut fluids: Vec<&FluidLook> = mods.iter().flat_map(|m| &m.content.fluids).collect();
    fluids.sort_by(|a, b| a.id.cmp(&b.id));
    fluids
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

static FLUID_LOOKS: std::sync::RwLock<BTreeMap<String, FluidLook>> =
    std::sync::RwLock::new(BTreeMap::new());

/// Remembers how the mods' fluids look (`data/<id>/horizon/fluid/`), for what the client draws.
pub fn set_fluid_looks(mods: &[ContentMod]) {
    let looks: BTreeMap<String, FluidLook> = mods
        .iter()
        .flat_map(|m| &m.content.fluids)
        .map(|f| (f.id.clone(), f.clone()))
        .collect();
    crate::fluid_models::set_world_looks(looks.values());
    *FLUID_LOOKS.write().unwrap_or_else(|e| e.into_inner()) = looks;
}

/// Registers the world fluids the mods declare, before any block table is read from the root.
/// Returns their registry names, source and flowing, for the fluid keys.
pub fn register_world_fluids(
    mods: &[ContentMod],
) -> std::result::Result<Vec<String>, horizon_world::fluid::ModFluidRegistrationError> {
    let mut names = Vec::new();
    for look in mods.iter().flat_map(|m| &m.content.fluids) {
        let Some(world) = look.world else { continue };
        let flowing_id = look.flowing_id();
        names.push(look.id.clone());
        names.push(flowing_id.clone());
        horizon_world::fluid::register_mod_fluid(horizon_world::fluid::ModFluidType {
            id: look.id.clone(),
            flowing_id,
            bucket: look.bucket_id(),
            rules: horizon_world::fluid::ModFluidRules {
                tick_delay: world.tick_delay,
                drop_off: world.drop_off,
                slope_find_distance: world.slope_find_distance,
                converts_to_source: world.source_conversion,
            },
        })?;
    }
    Ok(names)
}

#[path = "mod_content_fluid_declarations.rs"]
mod fluid_declarations;
pub use fluid_declarations::accept_world_fluids;

/// How the fluid `id` looks, when a mod declares it.
pub fn fluid_look(id: &str) -> Option<FluidLook> {
    FLUID_LOOKS
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .get(id)
        .cloned()
}

/// The mods' fluids in registry order, by id as effects are, with NeoForge's
/// milk when a mod turns it on (`milk_fluid`, `NeoForgeMod.enableMilkFluid`).
/// The carried fluids (this fork's declarations, `CarriedFluid`); upstream's template fluids are
/// [`fluids`].
pub fn carried_fluids(mods: &[ContentMod]) -> Vec<&CarriedFluid> {
    static MILK: std::sync::LazyLock<CarriedFluid> =
        std::sync::LazyLock::new(CarriedFluid::neoforge_milk);
    let mut fluids: Vec<&CarriedFluid> =
        mods.iter().flat_map(|m| &m.content.carried_fluids).collect();
    if mods.iter().any(|m| m.content.milk_fluid) {
        fluids.push(&MILK);
    }
    fluids.sort_by(|a, b| a.id.cmp(&b.id));
    fluids
}

/// A fluid declaration as the fluid registry holds it, with the fluid tags
/// `tags` gives its source and flowing fluid ([`horizon_world::mod_fluids::fluid_tags_in`]).
pub fn mod_fluid(
    decl: &CarriedFluid,
    tags: &BTreeMap<String, Vec<String>>,
) -> horizon_world::mod_fluids::ModFluid {
    use horizon_mods::content::FluidVaporizes;
    use horizon_world::mod_fluids::Vaporizes;
    let tags_of = |id: &str| tags.get(id).cloned().unwrap_or_default();
    horizon_world::mod_fluids::ModFluid {
        id: decl.id.clone(),
        flowing: decl.flowing.clone(),
        block: decl.block.clone(),
        bucket: decl.bucket.clone(),
        tick_rate: decl.tick_rate,
        slope_find_distance: decl.slope_find_distance,
        level_decrease_per_block: decl.level_decrease_per_block,
        explosion_resistance: decl.explosion_resistance,
        can_convert_to_source: decl.can_convert_to_source,
        light_level: decl.light_level,
        density: decl.density,
        temperature: decl.temperature,
        viscosity: decl.viscosity,
        still_texture: decl.still_texture.clone(),
        flowing_texture: decl.flowing_texture.clone(),
        overlay_texture: decl.overlay_texture.clone(),
        tint: decl.tint,
        translucent: decl.translucent,
        fill_sound: decl.sounds.bucket_fill.clone(),
        empty_sound: decl.sounds.bucket_empty.clone(),
        description: decl.description.clone(),
        source_tags: tags_of(&decl.id),
        flowing_tags: tags_of(&decl.flowing),
        entity: horizon_world::mod_fluids::FluidEntity {
            can_swim: decl.entity.can_swim,
            can_drown: decl.entity.can_drown,
            can_push: decl.entity.can_push,
            motion_scale: decl.entity.motion_scale,
            fall_distance_modifier: decl.entity.fall_distance_modifier,
            can_extinguish: decl.entity.can_extinguish,
            can_hydrate: decl.entity.can_hydrate,
            supports_boating: decl.entity.supports_boating,
        },
        render_overlay: decl.render_overlay.clone(),
        fog_color: decl.fog_color,
        fog_range: decl.fog_range,
        vaporize_sound: decl.sounds.fluid_vaporize.clone(),
        vaporizes: match decl.vaporizes {
            FluidVaporizes::LikeWater => Vaporizes::LikeWater,
            FluidVaporizes::Ultrawarm => Vaporizes::Ultrawarm,
            FluidVaporizes::Never => Vaporizes::Never,
            FluidVaporizes::Always => Vaporizes::Always,
        },
        cauldron: decl.cauldron.as_ref().map(|c| horizon_world::mod_fluids::ModCauldron {
            block: c.block.clone(),
            total_amount: c.total_amount,
            max_level: c.max_level,
            level_property: c.level_property.clone(),
        }),
        drip: decl.drip.as_ref().map(|d| horizon_world::mod_fluids::ModDrip {
            chance: d.chance,
            particle: d.particle.clone(),
            cauldron: d.cauldron.clone(),
        }),
        cauldron_drip_sound: decl.sounds.cauldron_drip.clone(),
        item_movement: decl.item_movement.map_or_else(Default::default, |m| {
            horizon_world::mod_fluids::ItemMovement {
                horizontal: m.horizontal,
                below: m.below,
                above: m.above,
            }
        }),
    }
}

/// The mods' fluid interactions (`fluid_interaction` files), in the mods'
/// order and each mod's path order: `FluidInteractionRegistry`'s lists keep
/// the order the mods registered them in.
pub fn fluid_interactions(mods: &[ContentMod]) -> Vec<&horizon_mods::content::FluidInteractions> {
    mods.iter()
        .flat_map(|m| &m.content.fluid_interactions)
        .collect()
}

/// Registers the interactions of a root [`load_root`] gave, after its fluids
/// ([`register_mod_fluids`]): once per process. False when they already were.
pub fn register_fluid_interactions(files: &[horizon_mods::content::FluidInteractions]) -> bool {
    horizon_world::fluid_interactions::register_interactions(files.iter().flat_map(|file| {
        file.interactions.iter().map(|rule| {
            horizon_world::fluid_interactions::DeclaredInteraction {
                fluid: file.fluid.clone(),
                below: rule.below.clone(),
                fluids: rule.fluids.clone(),
                blocks: rule.blocks.clone(),
                source: rule.source.clone(),
                flowing: rule.flowing.clone(),
                fizz: rule.fizz,
            }
        })
    }))
}

/// Registers `fluids` (those of a root [`load_root`] gave) after vanilla's,
/// with the fluid tags of `root` (theirs, and vanilla's fluids' own: the full
/// pack's `create:bottomless/allow` holds water): once per process, as the
/// mods' mob effects are. False when fluids were already registered.
pub fn register_mod_fluids(root: &Path, fluids: &[CarriedFluid]) -> bool {
    let known: BTreeSet<&str> = fluids
        .iter()
        .flat_map(|f| [f.id.as_str(), f.flowing.as_str()])
        .chain(horizon_world::fluid::FluidKind::ALL.iter().map(|k| k.name()))
        .collect();
    let lists = horizon_world::mod_fluids::fluid_tag_lists_in(root, &|id| known.contains(id))
        .unwrap_or_else(|e| {
            log::warn!("Horizon: the mods' fluid tags not read: {e}");
            BTreeMap::new()
        });
    let mut tags: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (tag, members) in &lists {
        for fluid in members {
            tags.entry(fluid.clone()).or_default().push(tag.clone());
        }
    }
    horizon_world::mod_fluids::register_fluids(
        fluids.iter().map(|f| mod_fluid(f, &tags)),
        Some(&lists),
    )
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
    fn as_str(&self) -> Option<&str> {
        match self {
            J::Str(s) => Some(s),
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
    shape
        .edges()
        .into_iter()
        .map(|edge| J::Arr(edge.into_iter().map(J::f64).collect()))
        .collect()
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

fn resolve_state_values<'a, 's, T>(
    spec: &'s StateValues<T>,
    block: &str,
    states: &Range<u32>,
    order: &[(u32, u32)],
    combos: &[Vec<(String, String)>],
    properties: impl Fn(u32) -> Option<&'a J>,
) -> Result<Vec<&'s T>> {
    let names = spec.property_names();
    let mut used = BTreeSet::new();
    let mut selected = Vec::with_capacity(order.len());
    for &(state, combo) in order {
        let values = names
            .iter()
            .map(|name| {
                combos[combo as usize]
                    .iter()
                    .find(|(key, _)| key == name)
                    .map(|(_, value)| value.as_str())
                    .or_else(|| {
                        match properties(states.start + state).and_then(|props| props.get(name)) {
                            Some(J::Str(value)) => Some(value.as_str()),
                            _ => None,
                        }
                    })
                    .ok_or_else(|| format!("{block}: state lacks property {name}"))
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let key = values.join(",");
        let value = if let Some(value) = spec.values.get(&key) {
            used.insert(key);
            value
        } else {
            spec.default
                .as_ref()
                .ok_or_else(|| format!("{block}: no state value for {key}"))?
        };
        selected.push(value);
    }
    if used.len() != spec.values.len() {
        return Err(
            format!("{block}: state values include an unknown property value tuple").into(),
        );
    }
    Ok(selected)
}

fn resolve_shapes<'a>(
    spec: &BlockShapes,
    block: &str,
    states: &Range<u32>,
    order: &[(u32, u32)],
    combos: &[Vec<(String, String)>],
    properties: impl Fn(u32) -> Option<&'a J>,
) -> Result<Vec<VoxelShape>> {
    resolve_state_values(spec, block, states, order, combos, properties)?
        .into_iter()
        .map(|boxes| {
            let mut shape = VoxelShape::empty();
            for bounds in boxes {
                let part = VoxelShape::from_box(Aabb::new(
                    Vec3::new(bounds[0], bounds[1], bounds[2]),
                    Vec3::new(bounds[3], bounds[4], bounds[5]),
                ))?;
                shape = shape.join_unoptimized(&part, ShapeOp::OR)?;
            }
            Ok(shape)
        })
        .collect()
}

/// A content farmland declaration names a block absent from the loaded registry.
#[derive(Debug)]
pub struct UnknownBaseBlock {
    /// The farmland declaration.
    pub block: String,
    /// The missing base ID.
    pub base: String,
}
impl std::fmt::Display for UnknownBaseBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: base_block {} is not a block", self.block, self.base)
    }
}
impl std::error::Error for UnknownBaseBlock {}

/// A native block entity declaration uses vanilla's reserved namespace.
#[derive(Debug)]
pub struct ReservedBlockEntityNamespace {
    /// The refused type's registry ID.
    pub kind: String,
}
impl fmt::Display for ReservedBlockEntityNamespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "block entity type {} cannot use the minecraft namespace",
            self.kind
        )
    }
}
impl std::error::Error for ReservedBlockEntityNamespace {}

/// A block ID resolved against the complete content and vanilla registry.
struct KnownBlock<'a>(&'a str);

/// The template class's rule for downward skylight through a declared outline.
#[derive(Clone, Copy)]
enum SkylightRule {
    Shape,
    Fluid,
    Template,
}

/// A block being added: its declaration and where its states go.
struct NewBlock<'a> {
    decl: &'a Block,
    base_block: Option<KnownBlock<'a>>,
    rename: Rename,
    /// The template's item, if it has one.
    item: Option<String>,
    shapes: Option<Vec<VoxelShape>>,
    collision_shapes: Option<Vec<VoxelShape>>,
    /// Whether each shaped state contains fluid, including intrinsic template fluids.
    has_fluid: Vec<bool>,
    skylight: SkylightRule,
    custom_dampening: bool,
    context_shapes: BTreeMap<String, Vec<VoxelShape>>,
    state_light: Option<Vec<u8>>,
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
    /// The item its gameplay row names when that is not the template's item
    /// renamed: `block_item`, or `minecraft:air` for a block with no item
    /// (a sign's wall form keeps the paired sign's). `Block.asItem` and, unless
    /// the block has a `pick_item` row of its own, pick-block read this value.
    fn item_row(&self) -> Option<&str> {
        match &self.decl.block_item {
            Some(item) => Some(item),
            None => (!self.decl.item && self.wall_of.is_none()).then_some("minecraft:air"),
        }
    }

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
    block: Option<String>,
    arm_pose: Option<horizon_mod_package::content::ArmPose>,
    /// `StandingAndWallBlockItem.wallBlock`, when its block declares it.
    wall_block: Option<String>,
    template: Option<String>,
}

struct Plan<'a> {
    blocks: Vec<NewBlock<'a>>,
    items: Vec<NewItem>,
    /// Every vanilla item id, including items omitted from the search tab.
    vanilla_items: BTreeSet<String>,
    effects: Vec<&'a MobEffect>,
    fluids: Vec<&'a FluidLook>,
    entity_types: Vec<&'a EntityType>,
    block_entity_types: Vec<&'a BlockEntityType>,
    block_entity_factories: BTreeMap<&'a str, &'a str>,
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

/// Code declarations, joined to their manifest owner. Each owner's namespaces
/// validate together, independent of namespace order, with all-or-nothing refusal.
/// Unknown namespaces become synthetic content-only mods. Checked as files are
/// ([`horizon_mod_package::Content::declare`], [`accept_mod`]).
pub fn accept_declared(
    mods: &mut Vec<ContentMod>,
    blocks: &[horizon_mod_package::content::Block],
    items: &[horizon_mod_package::content::Item],
    types: &[BlockEntityType],
    known: &BTreeSet<String>,
) -> Vec<String> {
    let owners: BTreeMap<_, _> = mods
        .iter()
        .flat_map(|m| {
            m.content
                .namespaces
                .iter()
                .map(|ns| (ns.clone(), m.id.clone()))
        })
        .collect();
    let owner = |id: &str| {
        let ns = id.split_once(':').map_or("", |(ns, _)| ns);
        owners.get(ns).cloned().unwrap_or_else(|| ns.to_owned())
    };
    let mut by_owner: BTreeMap<String, (Vec<_>, Vec<_>)> = Default::default();
    for block in blocks {
        by_owner
            .entry(owner(&block.id))
            .or_default()
            .0
            .push(block.clone());
    }
    for item in items {
        by_owner
            .entry(owner(&item.id))
            .or_default()
            .1
            .push(item.clone());
    }
    for kind in types {
        by_owner.entry(owner(&kind.id)).or_default();
    }
    let mut errors = Vec::new();
    for (id, (blocks, items)) in by_owner {
        let types = types.iter().filter(|t| owner(&t.id) == id).cloned();
        let result = match mods.iter().position(|m| m.id == id) {
            Some(i) => {
                let mut content = horizon_mod_package::Content::declared(&mods[i].id);
                content.namespaces.clone_from(&mods[i].content.namespaces);
                content.blocks.clone_from(&mods[i].content.blocks);
                content.items.clone_from(&mods[i].content.items);
                content
                    .declare(&blocks, &items)
                    .and_then(|()| {
                        let mut added = horizon_mod_package::Content::declared(&id);
                        added.blocks.clone_from(&blocks);
                        added.items.clone_from(&items);
                        validate_components(&added, known).map_err(|e| e.to_string())
                    })
                    .map(|()| {
                        mods[i].content.blocks.extend(blocks);
                        mods[i].content.items.extend(items);
                        mods[i].content.block_entity_types.extend(types);
                    })
            }
            None => {
                let mut content = horizon_mod_package::Content::declared(&id);
                content.declare(&blocks, &items).and_then(|()| {
                    content.block_entity_types.extend(types);
                    let candidate = ContentMod {
                        id: id.clone(),
                        content,
                        jars: Vec::new(),
                    };
                    accept_mod(mods, candidate, known).map_err(|e| e.to_string())
                })
            }
        };
        if let Err(e) = result {
            errors.push(format!("native blocks and items of {id}: {e}"));
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
        .find_map(|c| horizon_mod_package::content::clash((&c.id, &c.content.namespaces), own))
    {
        return Err(error.into());
    }
    mods.push(candidate);
    Ok(())
}

fn validate_components(
    content: &horizon_mod_package::Content,
    known: &BTreeSet<String>,
) -> Result<()> {
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
                horizon_core::nbt::validate_component_value(&tag)
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
    /// size, eyes and category remain the template's (eyes, spawners and `reg.entity_types`),
    /// with `template`, attachments and attribute bases (`MobTypes`
    /// makes the mob from it). Template values stay raw text: rewriting a float
    /// such as a zombie's `movement_speed` can come back one ulp off.
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
            out.extend(mod_entity_row(row, t)?);
        }
        out.push(b'}');
        Ok(out)
    }
}

/// Content entity rows preserve template numbers and carry sound selections.
mod entity_sounds;
mod light_rows;
use entity_sounds::mod_entity_row;

impl<'a> Plan<'a> {
    fn new(base: &Path, mods: &'a [ContentMod]) -> Result<Self> {
        let vanilla_items = match base.join("items.json").exists() {
            true => match read_json(&base.join("items.json"))? {
                J::Obj(entries) => entries.into_iter().map(|(id, _)| id).collect(),
                _ => return Err("items.json is not an object".into()),
            },
            false => BTreeSet::new(),
        };
        // Class chains are most-derived first. Fluid-dependent overrides must
        // use the resolved state's fluid, not the template's cached dry answer.
        let mut skylight_rules = BTreeMap::new();
        let mut custom_dampening = BTreeSet::new();
        if base.join("block-behaviours.json").exists() {
            let behaviors = read_json(&base.join("block-behaviours.json"))?;
            let Some(J::Arr(blocks)) = behaviors.get("blocks") else {
                return Err("block-behaviours.json has no blocks".into());
            };
            for block in blocks {
                let (Some(J::Str(name)), Some(J::Arr(classes))) =
                    (block.get("name"), block.get("class"))
                else {
                    continue;
                };
                let classes: Vec<&str> = classes
                    .iter()
                    .filter_map(|class| match class {
                        J::Str(name) => Some(name.as_str()),
                        _ => None,
                    })
                    .collect();
                if let Some(rule) = classes.iter().find_map(|class| match *class {
                    "BarrierBlock"
                    | "CrossCollisionBlock"
                    | "GlowLichenBlock"
                    | "LightBlock"
                    | "VegetationBlock"
                    | "WallBlock" => Some(SkylightRule::Fluid),
                    "BambooStalkBlock" | "HangingMossBlock" | "LiquidBlock"
                    | "MossyCarpetBlock" | "PipeBlock" | "ShulkerBoxBlock" | "TintedGlassBlock"
                    | "TransparentBlock" | "VineBlock" => Some(SkylightRule::Template),
                    _ => None,
                }) {
                    skylight_rules.insert(name.clone(), rule);
                }
                if classes
                    .iter()
                    .any(|class| matches!(*class, "LeavesBlock" | "TintedGlassBlock"))
                {
                    custom_dampening.insert(name.clone());
                }
            }
        }
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
        let declared_ids: BTreeSet<&str> = mods
            .iter()
            .flat_map(|m| m.content.blocks.iter().map(|b| b.id.as_str()))
            .collect();
        let mut block_entity_types: Vec<(&str, &BlockEntityType)> = mods
            .iter()
            .flat_map(|m| {
                m.content
                    .block_entity_types
                    .iter()
                    .map(move |kind| (m.id.as_str(), kind))
            })
            .collect();
        block_entity_types.sort_by(|(_, a), (_, b)| a.id.cmp(&b.id));
        let mut block_entity_factories = BTreeMap::new();
        if !block_entity_types.is_empty() {
            let registry = read_json(&base.join("registries.json"))?;
            let Some(J::Obj(entries)) = registry
                .get("minecraft:block_entity_type")
                .and_then(|r| r.get("entries"))
            else {
                return Err("no minecraft:block_entity_type entries".into());
            };
            let mut ids: BTreeSet<&str> = entries.iter().map(|(id, _)| id.as_str()).collect();
            for (mod_id, kind) in &block_entity_types {
                if kind.id.starts_with("minecraft:") {
                    return Err(ReservedBlockEntityNamespace {
                        kind: kind.id.clone(),
                    }
                    .into());
                }
                if !ids.insert(&kind.id) {
                    return Err(format!("block entity type {} is already declared", kind.id).into());
                }
                for block in &kind.blocks {
                    if block.starts_with("minecraft:") {
                        return Err(format!(
                            "mod {mod_id}: block entity type {} cannot own vanilla block {block}",
                            kind.id
                        )
                        .into());
                    }
                    if !declared_ids.contains(block.as_str()) {
                        return Err(format!(
                            "block entity type {}: {block} is not a block",
                            kind.id
                        )
                        .into());
                    }
                    if let Some(previous) =
                        block_entity_factories.insert(block.as_str(), kind.id.as_str())
                    {
                        return Err(format!(
                            "block {block} belongs to both block entity types {previous} and {}",
                            kind.id
                        )
                        .into());
                    }
                }
            }
        }
        let mut blocks = Vec::new();
        let mut next = names.len() as u32;
        let mut sorted: Vec<&ContentMod> = mods.iter().collect();
        sorted.sort_by(|a, b| a.id.cmp(&b.id));
        let mut wanted_sounds = Vec::new();
        for m in &sorted {
            let mut declared: Vec<&Block> = m.content.blocks.iter().collect();
            declared.sort_by(|a, b| a.id.cmp(&b.id));
            for decl in declared {
                let base_block = match decl.base_block.as_deref() {
                    Some(base) if defaults.contains_key(base) || declared_ids.contains(base) => {
                        Some(KnownBlock(base))
                    }
                    Some(base) => {
                        return Err(UnknownBaseBlock {
                            block: decl.id.clone(),
                            base: base.to_owned(),
                        }
                        .into());
                    }
                    None => None,
                };
                if defaults.contains_key(&decl.id) {
                    return Err(format!("{}: {} is already a block", m.id, decl.id).into());
                }
                let states = range(&decl.template).ok_or_else(|| {
                    format!("{}: template {} is not a block", decl.id, decl.template)
                })?;
                let pipe_shapes = if let Some(size) = decl.thickness {
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
                let resolve = |spec: &BlockShapes| {
                    resolve_shapes(spec, &decl.id, &states, &order, &combos, properties)
                };
                let shapes = if let Some(spec) = &decl.shapes {
                    Some(resolve(spec)?)
                } else {
                    pipe_shapes.map(|shapes: Vec<VoxelShape>| {
                        order
                            .iter()
                            .map(|&(state, _)| shapes[state as usize].clone())
                            .collect()
                    })
                };
                let collision_shapes = decl.collision_shapes.as_ref().map(resolve).transpose()?;
                let mut has_fluid = Vec::new();
                if shapes.is_some() {
                    for &(state, combo) in &order {
                        let J::Arr(row) = &rows[(states.start + state) as usize] else {
                            unreachable!()
                        };
                        // facts.json exports !BlockState.getFluidState().isEmpty(),
                        // including water in bubble columns and aquatic plants.
                        let Some(J::Bool(fluid)) = row.get(2) else {
                            return Err("facts.json: a state without a fluid flag".into());
                        };
                        has_fluid.push(
                            *fluid
                                || combos[combo as usize]
                                    .iter()
                                    .any(|(name, value)| name == "waterlogged" && value == "true"),
                        );
                    }
                }
                let mut context_shapes = BTreeMap::new();
                for (channel, specs) in [
                    ("outline", &decl.outline_context_shapes),
                    ("collision", &decl.collision_context_shapes),
                ] {
                    for (kind, spec) in specs {
                        context_shapes.insert(format!("{channel}_{}", kind.name()), resolve(spec)?);
                    }
                }
                if let Some(spec) = &decl.interaction_shapes {
                    context_shapes.insert("interaction".into(), resolve(spec)?);
                }
                let state_light = decl
                    .state_light
                    .as_ref()
                    .map(|spec| {
                        resolve_state_values(spec, &decl.id, &states, &order, &combos, properties)
                            .map(|values| values.into_iter().copied().collect())
                    })
                    .transpose()?;
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
                    base_block,
                    rename,
                    item: None,
                    shapes,
                    collision_shapes,
                    has_fluid,
                    skylight: skylight_rules
                        .get(&decl.template)
                        .copied()
                        .unwrap_or(SkylightRule::Shape),
                    custom_dampening: custom_dampening.contains(&decl.template),
                    context_shapes,
                    state_light,
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
                    block: None,
                    arm_pose: None,
                    wall_block: b.decl.wall_block.clone(),
                    template: None,
                });
            }
        }
        for m in &sorted {
            for i in &m.content.items {
                if let Some(block) = &i.block
                    && !defaults.contains_key(block)
                    && !blocks.iter().any(|b| b.decl.id == *block)
                {
                    return Err(format!("{}: block {block:?} is not a block", i.id).into());
                }
                items.push(NewItem {
                    rename: Rename::new(&i.template, &i.id),
                    max_stack_size: i.max_stack_size,
                    rarity: i.rarity,
                    components: component_patch(&i.id, &i.components)?,
                    crafting_remainder: i.crafting_remainder.clone(),
                    block: i.block.clone(),
                    arm_pose: i.arm_pose,
                    wall_block: None,
                    template: i.block.is_none().then(|| i.template.clone()),
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
            fluids: fluids(mods),
            entity_types: entity_types(mods),
            block_entity_types: block_entity_types
                .into_iter()
                .map(|(_, kind)| kind)
                .collect(),
            block_entity_factories,
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
            if let Some(item) = b.item_row() {
                row.set("item", J::Str(item.into()));
            }
            if let Some(item) = &b.decl.pick_item {
                row.set("pick_item", J::Str(item.clone()));
            }
            self.override_gameplay(b, i, combo, &mut row);
            if let Some(shapes) = &b.shapes {
                let shape = &shapes[i];
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
                // BlockState.initCache and BlockBehaviour.getLightDampening:
                // the declared outline is the block's occlusion shape.
                let full = !shape.join_is_not_empty(&VoxelShape::block(), ShapeOp::XOR)?;
                let can_occlude = matches!(row.get("can_occlude"), Some(J::Bool(true)));
                let solid = can_occlude && full;
                let down = match b.skylight {
                    SkylightRule::Shape => !full && !b.has_fluid[i],
                    SkylightRule::Fluid => !b.has_fluid[i],
                    SkylightRule::Template => {
                        matches!(row.get("skylight_down"), Some(J::Bool(true)))
                    }
                };
                // A partial outline needs directional light occlusion even
                // when its full-cube template bypasses shape checks. Keep
                // the template's participation for special full shapes.
                let shape_occlusion = can_occlude
                    && (!full || matches!(row.get("shape_occlusion"), Some(J::Bool(true))));
                row.set("shape_occlusion", J::Bool(shape_occlusion));
                row.set("solid_render", J::Bool(solid));
                row.set("skylight_down", J::Bool(down));
                if !b.custom_dampening {
                    row.set(
                        "dampening",
                        J::Num(
                            (if solid {
                                15
                            } else if down {
                                0
                            } else {
                                1
                            })
                            .into(),
                        ),
                    );
                }
                row.set("outline", J::Arr(boxes));
                row.set("outline_edges", J::Arr(shape_edges(shape)));
            }
            if let Some(shapes) = b.collision_shapes.as_ref().or(b.shapes.as_ref()) {
                let shape = &shapes[i];
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
    fn override_gameplay(
        &self,
        b: &NewBlock,
        state_index: usize,
        combo: &[(String, String)],
        row: &mut J,
    ) {
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
        if p.destroy_overlay == Some(false) {
            row.set("no_destroy_overlay", J::Bool(true));
        }
        if let Some(shape) = p.render_shape {
            row.set("render_shape", J::Str(shape.name().into()));
        }
        if let Some(reaction) = p.push_reaction {
            row.set("push", J::Str(reaction.name().into()));
        }
        // `noOcclusion`: a block that can't occlude is never a solid render either, and a solid
        // render's full light dampening becomes a non-solid block's (`getLightDampening`: 0 when
        // skylight passes down, else 1, as for leaves and spawners).
        if p.occludes == Some(false) {
            let solid = matches!(row.get("solid_render"), Some(J::Bool(true)));
            if solid {
                let down = matches!(row.get("skylight_down"), Some(J::Bool(true)));
                row.set("dampening", J::Num(i64::from(!down).into()));
            }
            row.set("can_occlude", J::Bool(false));
            row.set("solid_render", J::Bool(false));
        }
        if let Some(v) = p.hardness {
            row.set("hardness", J::f64(v as f64));
        }
        if let Some(v) = p.explosion_resistance {
            row.set("explosion_resistance", J::f64(v as f64));
        }
        light_rows::override_light(b, state_index, row);
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
            if !components && let Some(block) = &i.block {
                map.set(
                    &i.rename.to,
                    J::Obj(vec![
                        (
                            "class".into(),
                            J::Arr(vec![J::Str("BlockItem".into()), J::Str("Item".into())]),
                        ),
                        (
                            "fields".into(),
                            J::Obj(vec![("block".into(), J::Str(block.clone()))]),
                        ),
                    ]),
                );
                continue;
            }
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
            for b in &self.blocks {
                for (key, item) in [
                    ("block_item", &b.decl.block_item),
                    ("pick_item", &b.decl.pick_item),
                ] {
                    if let Some(item) = item
                        && map.get(item).is_none()
                    {
                        return Err(format!("{}: {key} {item:?} is not an item", b.decl.id).into());
                    }
                }
            }
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
                        if let Some(item) = b.item_row() {
                            v.set("item", J::Str(item.into()));
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
                        // `new FenceGateBlock(woodType, ..)`: the gate's own sounds replace its wood's.
                        if let Some(fields) = v.at("fields/type") {
                            if let Some(sound) = &b.decl.open_sound {
                                fields.set("fenceGateOpen", J::Str(sound.clone()));
                            }
                            if let Some(sound) = &b.decl.close_sound {
                                fields.set("fenceGateClose", J::Str(sound.clone()));
                            }
                        }
                        // `createDefaultSignText`: the colour and glow a new sign's text starts with.
                        if (b.decl.sign_text_color.is_some() || b.decl.sign_text_glow.is_some())
                            && let Some(fields) = v.at("fields")
                        {
                            let color = b.decl.sign_text_color.as_deref().unwrap_or("black");
                            let id = horizon_mod_package::content::DYE_COLORS
                                .iter()
                                .position(|name| *name == color)
                                .unwrap_or_default();
                            fields.set("sign_text_color", J::Num((id as u64).into()));
                            fields.set(
                                "sign_text_glow",
                                J::Bool(b.decl.sign_text_glow.unwrap_or(false)),
                            );
                        }
                        if let Some(base) = &b.base_block {
                            let field = v
                                .at("fields/baseBlock")
                                .ok_or("farmland template has no base block")?;
                            *field = J::Str(base.0.to_owned());
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
                    // Faces beside a block that occludes nothing are drawn (`Block.shouldRenderFace`).
                    if !model && b.decl.properties.occludes == Some(false) {
                        entry.set("can_occlude", J::Bool(false));
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
            "landing-definitions.json" => {
                self.names(doc, false);
                for b in &self.blocks {
                    if let Some(base) = &b.base_block {
                        let entry = doc
                            .get_mut(&b.decl.id)
                            .ok_or("farmland template has no landing definition")?;
                        entry.set("base", J::Str(base.0.to_owned()));
                    }
                }
            }
            "worldgen/blocks/survival-rules.json" => self.names(doc, false),
            "worldgen/blocks/survival-extra-definitions.json" => {
                if let Some(map) = doc.at("ignition_odds") {
                    self.names(map, false);
                }
                // The way a mod's growing plant grows comes from its template.
                let plants = self
                    .blocks
                    .iter()
                    .filter(|b| GROWING_PLANTS.contains(&b.decl.template.as_str()))
                    .map(|b| (b.decl.id.clone(), J::Str(b.decl.template.clone())))
                    .collect::<Vec<_>>();
                if !plants.is_empty()
                    && let J::Obj(entries) = doc
                {
                    entries.push(("growing_plants".to_owned(), J::Obj(plants)));
                }
            }
            "items.json" => self.items(doc, true)?,
            "registries.json" => {
                // Network ids: the game and its server map items, blocks, mob
                // effects and entity types through this report, so new ones get
                // the ids after vanilla's, in plan order. Chunk packets carry
                // the declared block entity type, not its block's id.
                let new_items = self.items.iter().map(|i| i.rename.to.clone());
                let new_blocks = self.blocks.iter().map(|b| b.decl.id.clone());
                let mut registries = vec![
                    ("minecraft:item", new_items.collect::<Vec<_>>()),
                    ("minecraft:block", new_blocks.collect()),
                ];
                if !self.block_entity_types.is_empty() {
                    let new_types = self.block_entity_types.iter().map(|t| t.id.clone());
                    registries.push(("minecraft:block_entity_type", new_types.collect()));
                }
                if !self.effects.is_empty() {
                    let new_effects = self.effects.iter().map(|e| e.id.clone());
                    registries.push(("minecraft:mob_effect", new_effects.collect()));
                }
                // Fluid tags go by these ids: a mod fluid's source, then its flowing form.
                if !self.fluids.is_empty() {
                    let new_fluids = self
                        .fluids
                        .iter()
                        .flat_map(|f| [f.id.clone(), f.flowing_id()]);
                    registries.push(("minecraft:fluid", new_fluids.collect()));
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
                // Fluid tag files and fluid arguments.
                if let Some(J::Arr(ids)) = doc.at("by_registry/minecraft:fluid/ids") {
                    ids.extend(
                        self.fluids
                            .iter()
                            .flat_map(|f| [J::Str(f.id.clone()), J::Str(f.flowing_id())]),
                    );
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

    fn declared_shapes(&self, tag: &mut Tag, collision: bool) -> Result<()> {
        let Tag::Compound(root) = tag else {
            return Err("collision table is not a compound".into());
        };
        let mut indices = Vec::new();
        let Some(Tag::List(entries)) = root.get_mut(&JavaString::from("shapes")) else {
            return Err("collision table has no shapes".into());
        };
        for block in &self.blocks {
            let shapes = if collision {
                block.collision_shapes.as_ref().or(block.shapes.as_ref())
            } else {
                block.shapes.as_ref()
            };
            if let Some(shapes) = shapes {
                let mut encoded = BTreeMap::new();
                for (offset, shape) in shapes.iter().enumerate() {
                    let key: Vec<_> = shape
                        .boxes()
                        .iter()
                        .map(|bounds| {
                            [
                                bounds.min(Axis::X),
                                bounds.min(Axis::Y),
                                bounds.min(Axis::Z),
                                bounds.max(Axis::X),
                                bounds.max(Axis::Y),
                                bounds.max(Axis::Z),
                            ]
                            .map(f64::to_bits)
                        })
                        .collect();
                    let id = *encoded.entry(key).or_insert_with(|| {
                        entries.push(encode_shape(shape));
                        entries.len() as i32 - 1
                    });
                    indices.push((block.rename.first as usize + offset, id));
                }
            }
        }
        // Variant references live in the same checked per-state table as the defaults.
        // No runtime content scan, callback, or world-dependent registration is needed.
        let mut contexts = Vec::new();
        for block in &self.blocks {
            if block.context_shapes.is_empty() {
                continue;
            }
            let mut encoded = BTreeMap::new();
            for offset in 0..block.order.len() {
                let mut row = Compound::new();
                row.insert(
                    "state".into(),
                    Tag::Int((block.rename.first as usize + offset) as i32),
                );
                for (channel, variants) in &block.context_shapes {
                    let shape = &variants[offset];
                    let key: Vec<_> = shape
                        .boxes()
                        .iter()
                        .map(|bounds| {
                            [
                                bounds.min(Axis::X),
                                bounds.min(Axis::Y),
                                bounds.min(Axis::Z),
                                bounds.max(Axis::X),
                                bounds.max(Axis::Y),
                                bounds.max(Axis::Z),
                            ]
                            .map(f64::to_bits)
                        })
                        .collect();
                    let id = *encoded.entry(key).or_insert_with(|| {
                        entries.push(encode_shape(shape));
                        entries.len() as i32 - 1
                    });
                    row.insert(channel.as_str().into(), Tag::Int(id));
                }
                contexts.push(Tag::Compound(row));
            }
        }
        if !contexts.is_empty() {
            root.insert("context_shapes".into(), Tag::List(contexts));
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
    /// vanilla's, ten to a page (the top row's first
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
                        if b.decl.properties.occludes == Some(false)
                            && let Tag::Byte(flags) = &mut v
                        {
                            *flags &= !16;
                        }
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

/// The vanilla blocks whose survival rule is `GrowingPlant`.
const GROWING_PLANTS: [&str; 8] = [
    "minecraft:kelp",
    "minecraft:kelp_plant",
    "minecraft:cave_vines",
    "minecraft:cave_vines_plant",
    "minecraft:twisting_vines",
    "minecraft:twisting_vines_plant",
    "minecraft:weeping_vines",
    "minecraft:weeping_vines_plant",
];

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
/// mods hold, and a mod's own biomes (never vanilla's: `worldgen_entry`).
/// Noise and density functions change the terrain and are not read from mods.
const WORLDGEN_BUNDLES: [(&str, &str); 7] = [
    ("biome", BIOMES),
    ("structure", "worldgen/structures.json"),
    ("structure_set", "worldgen/structure_sets.json"),
    ("template_pool", "worldgen/template_pools.json"),
    ("processor_list", "worldgen/processor_lists.json"),
    ("feature", "worldgen/features.json"),
    ("placed_feature", "worldgen/placed_features.json"),
];

/// The registry of `command-registries.json` that each bundle's ids join,
/// for the commands that suggest them (`/locate`, `/place`).
const COMMAND_REGISTRIES: [(&str, &str); 7] = [
    (BIOMES, "minecraft:worldgen/biome"),
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
    if kind == "biome" && ns == "minecraft" {
        return None;
    }
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

/// Applies `additions` to the bundle `doc` by priority (lower first); equal priorities keep the
/// order given, which is mod order, then file order.
fn apply_additions(doc: &mut J, additions: &mut [Addition]) -> Result<()> {
    additions.sort_by_key(|a| a.priority);
    additions.iter().try_for_each(|a| a.apply(doc))
}

/// The worldgen loader's bundles of biomes and of placed feature tags.
const BIOMES: &str = "worldgen/biomes.json";
const FEATURE_TAGS: &str = "worldgen/feature_tags.json";
const PLACED_FEATURES: &str = "worldgen/placed_features.json";
const CARVERS: &str = "worldgen/carvers.json";

/// Rewrites the biome bundle with the mods' biome modifiers. `biome_doc`,
/// `biome_tags` and `placed` are the bundles as the mods left them (`None`: the base's).
fn place_in_biomes(
    base: &Path,
    overlay: &Overlay<'_>,
    modifiers: &[biomes::Modifier],
    enabled_mods: Vec<String>,
    biome_doc: Option<J>,
    biome_tags: Option<J>,
    placed: Option<J>,
    tags: &BTreeMap<String, TagMerge>,
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
    let mut doc = match biome_doc {
        Some(doc) => doc,
        None => read_json(&base.join(BIOMES))?,
    };
    let biome_tags = bundle(biome_tags, BIOME_TAGS)?;
    let placed: BTreeSet<String> = bundle(placed, PLACED_FEATURES)?.into_keys().collect();
    let feature_tags = bundle(None, FEATURE_TAGS)?;
    let (entity_categories, entity_tags) = modifier_entities(base, overlay, tags)?;
    let carver_ids = biomes::carver_ids(&base.join(CARVERS))?;
    let mut saved = SavedModifiers {
        carver_ids,
        carver_tags: modifier_tags(base, overlay, tags, "worldgen/carver")?,
        enabled_mods,
        modifiers: modifiers.iter().map(|m| m.source.clone()).collect(),
        unmodified: BTreeMap::new(),
        feature_tags,
        entity_categories,
        entity_tags,
    };
    let before = doc.clone();
    for (id, text) in saved.apply(&mut doc, modifiers, &biome_tags, &placed, warnings)? {
        if let Some(J::Str(old)) = before.get(&id) {
            saved.unmodified.insert(id.clone(), old.clone());
        }
        if let Some((ns, path)) = id.split_once(':') {
            let file = format!("data/{ns}/worldgen/biome/{path}.json");
            overlay.write(&file, text.as_bytes())?;
        }
    }
    biomes::write_state(overlay, &saved, &doc, modifiers)
}

fn modifier_entities(
    base: &Path,
    overlay: &Overlay<'_>,
    tags: &BTreeMap<String, TagMerge>,
) -> Result<(BTreeMap<String, String>, BTreeMap<String, String>)> {
    let entities_path = out_path(overlay, "entity-types.json");
    let entity_categories: BTreeMap<String, String> = match entities_path.exists() {
        true => match read_json(&entities_path)? {
            J::Obj(entries) => entries
                .into_iter()
                .map(|(k, v)| {
                    let cat = v
                        .get("category")
                        .and_then(J::as_str)
                        .unwrap_or("misc")
                        .to_owned();
                    (k, cat)
                })
                .collect(),
            _ => return Err("entity-types.json: not an object".into()),
        },
        false => BTreeMap::new(),
    };
    let entity_tags = modifier_tags(base, overlay, tags, "entity_type")?;
    Ok((entity_categories, entity_tags))
}

fn modifier_tags(
    base: &Path,
    overlay: &Overlay<'_>,
    tags: &BTreeMap<String, TagMerge>,
    registry: &str,
) -> Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    for ns in fs::read_dir(base.join("data"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let dir = ns.path().join("tags").join(registry);
        if !dir.is_dir() {
            continue;
        }
        for file in &crate::json_folder::read(&dir)?.files {
            result.insert(
                format!("{}:{}", ns.file_name().to_string_lossy(), file.name),
                std::str::from_utf8(file.bytes()?)?.to_owned(),
            );
        }
    }
    let prefix = format!("/tags/{registry}/");
    for file in tags.keys() {
        if let Some((ns, path)) = file
            .strip_prefix("data/")
            .and_then(|s| s.split_once(&prefix))
            .and_then(|(ns, p)| Some((ns, p.strip_suffix(".json")?)))
        {
            result.insert(
                format!("{ns}:{path}"),
                fs::read_to_string(out_path(overlay, file))?,
            );
        }
    }
    Ok(result)
}

#[path = "mod_content_structures.rs"]
mod structures;

/// In a content root with biome modifiers: what applying them again over a
/// world's data packs takes (`biome_modifiers_over_packs`).
const MODIFIERS: &str = "biome-modifiers.json";

#[derive(Serialize, Deserialize)]
struct SavedModifiers {
    enabled_mods: Vec<String>,
    /// Each modifier's mod, file and text, in the order they apply.
    modifiers: Vec<[String; 3]>,
    /// The biomes the modifiers changed, as they were before.
    unmodified: BTreeMap<String, String>,
    feature_tags: BTreeMap<String, String>,
    entity_categories: BTreeMap<String, String>,
    entity_tags: BTreeMap<String, String>,
    #[serde(default)]
    carver_ids: BTreeSet<String>,
    #[serde(default)]
    carver_tags: BTreeMap<String, String>,
}

impl SavedModifiers {
    fn apply(
        &self,
        doc: &mut J,
        modifiers: &[biomes::Modifier],
        biome_tags: &BTreeMap<String, String>,
        placed: &BTreeSet<String>,
        warnings: &mut Vec<String>,
    ) -> Result<Vec<(String, String)>> {
        let biome_ids: BTreeSet<String> = match &*doc {
            J::Obj(o) => o.iter().map(|(k, _)| k.clone()).collect(),
            _ => return Err(format!("{BIOMES}: not an object").into()),
        };
        let entity_ids: BTreeSet<String> = self.entity_categories.keys().cloned().collect();
        let registries = biomes::Registries::of(
            &biome_ids,
            biome_tags,
            placed,
            &self.feature_tags,
            (&entity_ids, &self.entity_tags),
            (&self.carver_ids, &self.carver_tags),
        );
        biomes::apply(
            doc,
            modifiers,
            &registries,
            &self.entity_categories,
            warnings,
        )
    }
}

/// A world's data packs (`packs`) with the content root `root`'s biome
/// modifiers applied over them. Modifiers apply once every data
/// pack is in, to the final biomes and tags: a pack's file for a biome
/// (Terralith's `minecraft:plains`) keeps the mods' features and spawns, and
/// a pack's biome its tags put under a modifier's selector gets them too.
/// `None` when the root's biomes stand: no modifiers, or nothing declared and packs that bring
/// no biome and no tag. `declared` are mods' code modifiers (mod, file, text), applied last.
pub fn biome_modifiers_over_packs(
    root: &Path,
    packs: &horizon_world::worldgen::overrides::WorldgenOverrides,
    declared: &[[String; 3]],
) -> Result<Option<horizon_world::worldgen::overrides::WorldgenOverrides>> {
    let Some(bytes) = biomes::saved_bytes(root, packs, declared) else {
        return Ok(None);
    };
    let mut saved: SavedModifiers =
        serde_json::from_slice(&bytes).map_err(|e| format!("{MODIFIERS}: {e}"))?;
    let read = |name: &str| -> Result<BTreeMap<String, String>> {
        match fs::read(root.join(name)) {
            Ok(b) => Ok(serde_json::from_slice(&b).map_err(|e| format!("{name}: {e}"))?),
            Err(_) => Ok(BTreeMap::new()),
        }
    };
    let mut warnings = Vec::new();
    let mut modifiers = Vec::new();
    for [m, file, text] in saved.modifiers.iter().chain(declared) {
        modifiers.extend(biomes::Modifier::read(m, file, text, &mut warnings)?);
    }
    // What the generator reads without the modifiers over the packs.
    let packed = packs.apply("biomes", read(BIOMES)?);
    let mut start = packed.clone();
    for (id, text) in &saved.unmodified {
        if packs.entries("biomes").all(|(p, _)| p != id) {
            start.insert(id.clone(), text.clone());
        }
    }
    let mut doc = J::Obj(start.into_iter().map(|(id, t)| (id, J::Str(t))).collect());
    // Pack tag files after the root's, as the worldgen loader merges them.
    let mut biome_tags = read(BIOME_TAGS)?;
    for (id, pack, json) in packs.tags("biome_tags") {
        let tag: J = match serde_json::from_str(json) {
            Ok(tag) => tag,
            Err(e) => {
                warnings.push(format!("{pack}: tag {id}: {e}"));
                continue;
            }
        };
        let (values, remove) = (tag_list(&tag, "values"), tag_list(&tag, "remove"));
        let old = match tag.get("replace") {
            Some(J::Bool(true)) => None,
            _ => biome_tags.get(id).map(String::as_str),
        };
        let layers = [load_conditions::TagLayers { values, remove }];
        let merged = merge_tag(
            old,
            &layers,
            id,
            &horizon_world::conditions::LoadConditions::new(
                saved.enabled_mods.iter().map(String::as_str),
            ),
        )?;
        biome_tags.insert(id.to_owned(), merged);
    }
    let placed = packs
        .apply("placed_features", read(PLACED_FEATURES)?)
        .into_keys()
        .collect();
    carver_registry::refresh(&mut saved, packs)?;
    saved.apply(&mut doc, &modifiers, &biome_tags, &placed, &mut warnings)?;
    for warning in warnings {
        log::warn!("Horizon: biome modifiers over data packs: {warning}");
    }
    let mut out = packs.clone();
    if let J::Obj(entries) = doc {
        for (id, text) in entries {
            if let J::Str(text) = text
                && packed.get(&id) != Some(&text)
            {
                out.set("biomes", &id, text);
            }
        }
    }
    Ok(Some(out))
}

#[path = "mod_content_carvers.rs"]
mod carver_registry;

#[path = "mod_content_biomes.rs"]
pub mod biomes;

/// The overlay being written: `out` mirrors `base` through links, and a
/// directory becomes real only when something inside it is written.
struct Overlay<'a> {
    written: std::sync::Mutex<std::collections::BTreeSet<String>>,
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
            written: Default::default(),
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
        let name = rel;
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
        self.written
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(name.to_owned());
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
        link_file(target, at, |target, at| fs::hard_link(target, at))
    }
}
/// A link [`link`] made to a directory or a file. Windows removes a junction
/// or a directory symlink as a directory.
fn remove_link(path: &Path) -> std::io::Result<()> {
    fs::remove_file(path).or_else(|e| fs::remove_dir(path).map_err(|_| e))
}
/// A file of the overlay where symlinks need a privilege: a hard link to the base's file, which costs a name where
/// a copy costs the file's bytes (the base is ~28,000 files, 0.6 GiB), or a copy when no link can be made (another
/// volume, a filesystem without links, the NTFS limit on links to one file). A link shares the base file's data,
/// which is safe because the overlay is only ever replaced name by name (`Overlay::write` removes the name before it
/// writes), never written in place. A symlink is copied through, as before: linked, it would stay a symlink.
#[cfg(any(not(unix), test))]
fn link_file(
    target: &Path,
    at: &Path,
    hard_link: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let symlink = fs::symlink_metadata(target).is_ok_and(|m| m.file_type().is_symlink());
    if !symlink && hard_link(target, at).is_ok() {
        return Ok(());
    }
    fs::copy(target, at).map(|_| ())
}

mod root_lifetime;
use root_lifetime::prune_roots;
pub use root_lifetime::retain_root;
mod load_conditions;
use load_conditions::{TagLayers, TagMerge, merge_tag, tag_list};
pub use load_conditions::{build_with_conditions, resource_root_with_conditions};

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

/// Threads rewriting the tables at once. Each holds a whole table as a tree
/// (gameplay-facts.json's is over a gigabyte with ATM10's mods), so more
/// would mostly add memory.
const TABLE_THREADS: usize = 3;

/// Rewrites the base's per-state tables for the mods' states into the overlay,
/// one table at a time: each holds vanilla's rows as a tree while its new rows
/// go straight to its file. [`build`] runs it on a thread of its own while the
/// mods' files are written: the tables need only the plan, and no mod file goes
/// where they do.
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
    /// Sprite sources it adds to vanilla's atlases: (file, sources).
    atlases: Vec<(String, Vec<J>)>,
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
        if is_atlas_file(file) {
            let bytes = reader.read(file)?;
            let doc: J = serde_json::from_slice(&bytes).map_err(|e| format!("{file}: {e}"))?;
            let Some(J::Arr(sources)) = doc.get("sources") else {
                return Err(format!("{}: {file}: no sources", m.id).into());
            };
            gathered.atlases.push((file.clone(), sources.clone()));
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

/// A mod's sprite sources for one of vanilla's atlases, appended to the
/// atlas's own as a resource reload does.
fn is_atlas_file(file: &str) -> bool {
    file.starts_with("assets/minecraft/atlases/") && file.ends_with(".json")
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
            || is_atlas_file(f)
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
/// The base's per-state tables, the largest first, with their directories
/// made real in the overlay: [`tables`]' threads then only write files.
fn table_jobs(base: &Path, overlay: &Overlay<'_>) -> Result<Vec<(&'static str, u64)>> {
    let mut jobs: Vec<(&str, u64)> = JSON_TABLES
        .iter()
        .chain(&NBT_TABLES)
        .filter_map(|name| Some((*name, fs::metadata(base.join(name)).ok()?.len())))
        .collect();
    jobs.sort_by_key(|(_, size)| std::cmp::Reverse(*size));
    for (name, _) in &jobs {
        if let Some(parent) = Path::new(name).parent() {
            overlay.dir(parent)?;
        }
    }
    Ok(jobs)
}

/// Rewrites the base's per-state tables (`jobs`, from [`table_jobs`]) for
/// the mods' states into the overlay. They are independent:
/// [`TABLE_THREADS`] threads take them, the largest first.
fn tables(base: &Path, plan: &Plan, overlay: &Overlay<'_>, jobs: &[(&str, u64)]) -> Result<()> {
    let table = |name: &str| -> Result<()> {
        let path = base.join(name);
        let bytes = if name == "entity-types.json" {
            plan.entity_types_json(&fs::read(&path)?)
                .map_err(|e| format!("{name}: {e}"))?
        } else if JSON_TABLES.contains(&name) {
            let mut doc = read_json(&path)?;
            plan.json(name, &mut doc)
                .map_err(|e| format!("{name}: {e}"))?;
            serde_json::to_vec(&doc)?
        } else {
            let (mut named, _) = nbt::read_named(&fs::read(&path)?, Limits::tables())
                .map_err(|e| format!("{name}: {e}"))?;
            plan.nbt(&mut named.tag);
            if name == "cached-collision.nbt" || name == "worldgen/blocks/cached-collision.nbt" {
                plan.declared_shapes(&mut named.tag, true)?;
            } else if name == "occlusion.nbt" {
                plan.declared_shapes(&mut named.tag, false)?;
            } else if name == "worldgen/blocks/block-properties.nbt" {
                let occlusion = named
                    .tag
                    .get_mut("occlusion")
                    .ok_or("block properties have no occlusion")?;
                plan.declared_shapes(occlusion, false)?;
            }
            nbt::write_named(&named, Limits::tables()).map_err(|e| format!("{name}: {e}"))?
        };
        overlay.write(name, &bytes)
    };
    let next = std::sync::atomic::AtomicUsize::new(0);
    let threads = horizon_threads::worker_count();
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads.min(TABLE_THREADS))
            .map(|_| {
                scope.spawn(|| -> std::result::Result<(), String> {
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some((name, _)) = jobs.get(i) else {
                            return Ok(());
                        };
                        table(name).map_err(|e| e.to_string())?;
                    }
                })
            })
            .collect();
        workers.into_iter().try_for_each(|w| {
            w.join()
                .unwrap_or_else(|_| Err("a table thread panicked".into()))
        })
    })?;
    Ok(())
}

struct ModLayer<'a> {
    assets: crate::packs::Pack,
    data: crate::datapack::DataPack,
    jar: Option<&'a JarRef>,
}

impl<'a> ModLayer<'a> {
    fn new(id: String, source: crate::packs::Source, jar: Option<&'a JarRef>) -> Result<Self> {
        let source = source.open()?;
        let data = crate::datapack::DataPack::from_source(id.clone(), source.clone());
        Ok(Self {
            assets: crate::packs::Pack::mod_layer(id, crate::packs::Source::Opened(source)),
            data,
            jar,
        })
    }
}

struct ModLayers<'a> {
    source: &'a ContentMod,
    layers: Vec<ModLayer<'a>>,
}

/// Open every input before writing any decoded output.
fn load_layers<'a>(jar_store: &Path, mods: &'a [ContentMod]) -> Result<Vec<ModLayers<'a>>> {
    let requirements =
        collect_jar_requirements(mods.iter().map(|m| (m.id.as_str(), m.jars.as_slice())))?;
    if let Some(missing) = missing_jars(jar_store, &requirements)?.into_iter().next() {
        return Err(missing.into());
    }
    let mut loaded = Vec::new();
    for m in mods {
        let mut layers = Vec::new();
        for jar in &m.jars {
            layers.push(ModLayer::new(
                format!("{}/{}", m.id, jar.id()),
                crate::packs::Source::Zip(jar.cache_path(jar_store)),
                Some(jar),
            )?);
        }
        if !m.content.files.is_empty() {
            let source = m.content.source().to_owned();
            layers.push(ModLayer::new(
                m.id.clone(),
                if source.is_dir() {
                    crate::packs::Source::Dir(source)
                } else {
                    crate::packs::Source::Zip(source)
                },
                None,
            )?);
        }
        loaded.push(ModLayers { source: m, layers });
    }
    Ok(loaded)
}

/// Builds generated content tables for `mods` over `base` in `out`.
/// Asset layers remain in their sources and are resolved by `resolved_resources`.
/// Each jar's file is `<sha512>.jar` in `jar_store`, where the launcher
/// puts it. A missing file refuses the load before decoded output is written.
/// Returns warnings for shared tag replacements and duplicate asset paths.
pub fn build(
    base: &Path,
    jar_store: &Path,
    mods: &[ContentMod],
    out: &Path,
    inventory: &horizon_mod_package::enabled::EnabledMods,
) -> Result<Vec<String>> {
    build_with_conditions(
        base,
        jar_store,
        mods,
        out,
        inventory,
        &horizon_world::conditions::LoadConditions::new(inventory.server_ids()),
    )
}

const GENERATED_FILES: &str = "horizon-generated-content-files.json";

/// Open the passive layers for one load. Assets and data retain the same jar
/// readers; only files the table builder generated override the raw data.
pub fn resolved_resources(
    base: &Path,
    prepared: PathBuf,
    jar_store: &Path,
    mods: &[ContentMod],
    cache: PathBuf,
) -> Result<horizon_assets::resource_source::ResourceRoot> {
    use horizon_assets::resource_source::{Resource, ResourceRoot, ResourceSource};
    let base_source = ResourceSource::directory(base.to_path_buf());
    let loaded = load_layers(jar_store, mods)?;
    let mut data = base_source
        .list("data/")
        .into_iter()
        .map(|name| {
            let resource = Resource::file(base_source.clone(), name.clone());
            (name, resource)
        })
        .collect::<BTreeMap<_, _>>();
    let mut changes = BTreeMap::new();
    for group in &loaded {
        let declarations = format!("data/{}/horizon/", group.source.id);
        for layer in &group.layers {
            for name in layer.data.list("data/") {
                if !name.starts_with(&declarations) {
                    changes.insert(
                        name.clone(),
                        Resource::file(layer.data.source().clone(), name),
                    );
                }
            }
        }
    }
    let mut assets_base = base_source.clone();
    if prepared != base {
        let generated: Vec<String> =
            serde_json::from_slice(&fs::read(prepared.join(GENERATED_FILES))?)?;
        let generated_source = ResourceSource::directory(prepared.clone());
        root_lifetime::retain_source(&prepared, &generated_source);
        let mut generated_assets = BTreeMap::new();
        for name in generated {
            if !horizon_assets::resource_source::stays_inside(&name) {
                return Err(format!("invalid generated resource name: {name}").into());
            }
            let resource = Resource::file(generated_source.clone(), name.clone());
            if name.starts_with("data/") {
                changes.insert(name, resource);
            } else if name.starts_with("assets/") {
                generated_assets.insert(name, resource);
            }
        }
        if !generated_assets.is_empty() {
            let mut files = base_source
                .list("")
                .into_iter()
                .map(|name| {
                    let resource = Resource::file(base_source.clone(), name.clone());
                    (name, resource)
                })
                .collect::<BTreeMap<_, _>>();
            files.extend(generated_assets);
            assets_base = ResourceSource::indexed(files);
        }
    }
    let layers = loaded
        .into_iter()
        .flat_map(|group| group.layers.into_iter().map(|layer| layer.assets))
        .collect::<Vec<_>>();
    // Client resource readers also consume passive registry data (for
    // example trim materials and dimension effects). Selected asset packs
    // remain above the asset names without changing the server data source.
    if !changes.is_empty() {
        let mut files = assets_base
            .list("")
            .into_iter()
            .map(|name| {
                let resource = Resource::file(assets_base.clone(), name.clone());
                (name, resource)
            })
            .collect::<BTreeMap<_, _>>();
        files.extend(changes.clone());
        assets_base = ResourceSource::indexed(files);
    }
    let assets = crate::packs::resources(assets_base, &layers)?;
    data.extend(changes.clone());
    Ok(ResourceRoot::new(prepared, cache, assets).with_data(
        ResourceSource::indexed(data),
        ResourceSource::indexed(changes),
    ))
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
    /// Their carried fluids, in registry order ([`carried_fluids`]).
    pub fluids: Vec<CarriedFluid>,
    /// Their fluid interactions, in order ([`fluid_interactions`]).
    pub fluid_interactions: Vec<horizon_mods::content::FluidInteractions>,
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
    #[serde(default)]
    fluids: Vec<(String, CarriedFluid)>,
    /// (id, file) of the fluid interactions; none in a summary from before
    /// them, which is built again.
    #[serde(default)]
    fluid_interactions: Option<Vec<(String, horizon_mods::content::FluidInteractions)>>,
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
        fluids: Vec::new(),
        fluid_interactions: Vec::new(),
    };
    if candidates.is_empty() && blocks.is_empty() && items.is_empty() {
        return Ok(base_root(Vec::new()));
    }
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
    // One root per mod set, as [`resource_root`]'s.
    let roots = cache.join("mod-content");
    let out = roots.join(&stamp[..ROOT_NAME]);
    if fs::read(out.join(STAMP)).is_ok_and(|s| s == stamp.as_bytes())
        && let Ok(bytes) = fs::read(out.join(SUMMARY))
        && let Ok(summary) = serde_json::from_slice::<Summary>(&bytes)
        && let Some(interactions) = summary.fluid_interactions
    {
        log_reused(&out);
        // Used again: kept another day (beside the root, not in it).
        let _ = fs::write(used_path(&out), b"");
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
        let fluids = summary.fluids.into_iter();
        return Ok(LoadedRoot {
            root: out,
            mods: summary.mods,
            accepted: summary.accepted,
            effects: effects.map(|(id, e)| MobEffect { id, ..e }).collect(),
            entity_types: types.map(|(id, t)| EntityType { id, ..t }).collect(),
            menu_types: menus.map(|(id, m)| MenuType { id, ..m }).collect(),
            fluids: fluids.map(|(id, f)| CarriedFluid { id, ..f }).collect(),
            fluid_interactions: interactions
                .into_iter()
                .map(|(id, f)| horizon_mods::content::FluidInteractions { id, ..f })
                .collect(),
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
    let mod_fluids: Vec<CarriedFluid> = carried_fluids(&mods).into_iter().cloned().collect();
    let interactions: Vec<horizon_mods::content::FluidInteractions> =
        fluid_interactions(&mods).into_iter().cloned().collect();
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
        fluids: mod_fluids.iter().map(|f| (f.id.clone(), f.clone())).collect(),
        fluid_interactions: Some(
            interactions
                .iter()
                .map(|f| (f.id.clone(), f.clone()))
                .collect(),
        ),
    };
    fs::write(out.join(SUMMARY), serde_json::to_vec(&summary)?)?;
    fs::write(out.join(STAMP), stamp)?;
    prune_roots(&roots, &out, KEEP_ROOTS);
    Ok(LoadedRoot {
        root: out,
        mods: mods.len(),
        accepted,
        effects,
        entity_types: types,
        menu_types: menus,
        fluids: mod_fluids,
        fluid_interactions: interactions,
    })
}

/// A reused root warns as the build did.
fn log_reused(out: &Path) {
    let warnings = fs::read_to_string(out.join(WARNINGS)).unwrap_or_default();
    warnings.lines().for_each(|w| log::warn!("Horizon: {w}"));
}

/// The root to load: `base` itself with no content, else the mods' overlay
/// over it, built in `cache/mod-content/<stamp>`: one folder per mod set, so
/// a running game keeps reading the root it was given while a switch builds
/// another (docs/design/mod-hotload.md). Roots of other sets unused for a day
/// are removed.
///
/// The root is built once and kept: later starts with the same build, base
/// and mod files reuse it. With ATM10's carried mods a build takes about
/// 25 s and peaks above 3 GB, more than everything the game keeps.
pub fn resource_root(
    base: &Path,
    jar_store: &Path,
    mods: &[ContentMod],
    cache: &Path,
    inventory: &horizon_mod_package::enabled::EnabledMods,
) -> Result<PathBuf> {
    resource_root_with_conditions(
        base,
        jar_store,
        mods,
        cache,
        inventory,
        &horizon_world::conditions::LoadConditions::new(inventory.server_ids()),
    )
}

/// Declared native factory assignments, sorted by block id. Validity is extended
/// after content resolution in the canonical `block_entity_types` table.
pub const BLOCK_ENTITY_TYPES: &str = "horizon-block-entity-types.json";

/// The length of a root's folder name: the start of its stamp.
const ROOT_NAME: usize = 16;
/// How long a root no start or switch used is kept.
const KEEP_ROOTS: std::time::Duration = std::time::Duration::from_secs(24 * 3600);
/// The file beside `root` whose time is its last reuse (`<root>.used`).
fn used_path(root: &Path) -> PathBuf {
    root.with_extension("used")
}

/// Whether turning a content mod on or off can apply in a running game
/// (docs/design/mod-hotload.md): its assets and data can, through the
/// resource reload and the server's data reload. What a world numbers or
/// generates from cannot yet: then the translation key of why.
pub fn live_switch(content: &Content) -> std::result::Result<(), &'static str> {
    if !content.blocks.is_empty()
        || !content.items.is_empty()
        || !content.effects.is_empty()
        || !content.fluids.is_empty()
        || !content.carried_fluids.is_empty()
        || !content.entity_types.is_empty()
        || !content.menu_types.is_empty()
        || !content.creative_tabs.is_empty()
        || !content.block_entity_types.is_empty()
    {
        return Err("horizon.mods.switch.registries");
    }
    let worldgen = |file: &&String| {
        let mut parts = file.split('/');
        parts.next() == Some("data")
            && parts.nth(1).is_some_and(|folder| {
                matches!(folder, "worldgen" | "dimension" | "dimension_type")
                    || folder == "tags" && file.contains("/tags/worldgen/")
                    || matches!(folder, "neoforge" | "forge")
                        && (file.contains("/biome_modifier/")
                            || file.contains("/structure_modifier/"))
            })
    };
    if content.files.iter().any(|f| worldgen(&f)) {
        return Err("horizon.mods.switch.worldgen");
    }
    Ok(())
}

/// [`resource_root`]'s [`stamp`] of `mods`: their files' stamps, and what each declares, so
/// blocks and items from code count too.
fn content_stamp(base: &Path, mods: &[ContentMod]) -> Result<String> {
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
    stamp(base, stamps, &declared)
}

/// In a root built by [`build`]: its warnings, one per line.
const WARNINGS: &str = "horizon-mod-content-warnings.txt";

/// In a root built by [`resource_root`]: what it was built from.
const STAMP: &str = "horizon-mod-content.stamp";
const JAR_INPUTS: &str = "horizon-mod-jar-inputs.json";

/// Check the accepted inputs of a composed resource root before server reload.
/// This reads no candidate packages and never reopens or rehashes an archive.
pub fn check_jars(root: &Path, jar_store: &Path) -> std::result::Result<(), JarInputsError> {
    let marker = root.join(MARKER);
    if !marker.try_exists().map_err(|source| JarInputsError::Io {
        path: marker,
        source,
    })? {
        return Ok(());
    }
    let path = root.join(JAR_INPUTS);
    let bytes = fs::read(&path).map_err(|source| JarInputsError::Io {
        path: path.clone(),
        source,
    })?;
    let requirements: Vec<horizon_mod_package::manifest::JarRequirement> =
        serde_json::from_slice(&bytes)
            .map_err(|source| JarInputsError::Invalid { path, source })?;
    let missing = missing_jars(jar_store, &requirements).map_err(|source| JarInputsError::Io {
        path: jar_store.to_owned(),
        source,
    })?;
    if let Some(missing) = missing.into_iter().next() {
        return Err(JarInputsError::Missing(Box::new(missing)));
    }
    Ok(())
}

/// What a content root is built from: this build of the game, the base's
/// manifest and the tables [`build`] rewrites, and every file of every mod
/// in order. Files count by size and modification time, as build tools do.
fn stamp(
    base: &Path,
    jar_store: &Path,
    mods: &[ContentMod],
    inventory: &horizon_mod_package::enabled::EnabledMods,
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
    let mut parts = vec![
        "horizon mod content 7".to_owned(),
        serde_json::to_string(inventory)?,
    ];
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
    for m in mods {
        parts.push(m.id.clone());
        parts.push(format!("jars {}", serde_json::to_string(&m.jars)?));
        for jar in &m.jars {
            parts.push(meta(&jar.cache_path(jar_store)).unwrap_or_else(|_| "not cached".into()));
        }
        // What it declares, so blocks and items from code count too.
        parts.push(format!(
            "{:?} {:?} {:?}",
            m.content.blocks, m.content.items, m.content.block_entity_types
        ));
        let source = m.content.source();
        if m.content.files.is_empty() {
            // Declared in code only: nothing to read.
        } else if source.is_dir() {
            for file in &m.content.files {
                parts.push(meta(&source.join(file))?);
            }
        } else {
            parts.push(meta(source)?);
        }
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
