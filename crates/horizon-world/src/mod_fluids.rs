//! The content mods' fluids, after vanilla's in the fluid registry: what
//! NeoForge's `FluidType` and `BaseFlowingFluid` hold for one fluid, with its
//! client extension's textures and tint (`IClientFluidTypeExtensions`). The
//! list is set once per process, from the startup content root, as the mods'
//! mob effects are; each fluid takes two registry ids after vanilla's five,
//! its flowing fluid's and its source's ([`FluidKind`]).
use crate::{compression::invalid, fluid::FluidKind, worldgen::tags::TagDefinitions};
use std::{
    collections::{BTreeMap, HashMap},
    fs, io,
    path::Path,
    sync::OnceLock,
};

/// One content mod's fluid. Its numbers default to `BaseFlowingFluid.Properties`'
/// and `FluidType.Properties`' ([`ModFluid::new`]).
#[derive(Clone, Debug, PartialEq)]
pub struct ModFluid {
    /// The source fluid, `<mod id>:<path>`.
    pub id: String,
    /// The flowing fluid (`BaseFlowingFluid.Flowing`); empty for a fluid that
    /// is no `FlowingFluid` ([`ModFluid::flows`]).
    pub flowing: String,
    /// Its `LiquidBlock`, which `createLegacyBlock` places; `minecraft:air`
    /// for a fluid with none (only tanks and buckets hold it).
    pub block: String,
    /// `getBucket`, None for `Items.AIR`.
    pub bucket: Option<String>,
    /// `getTickDelay`.
    pub tick_rate: i32,
    /// `getSlopeFindDistance`.
    pub slope_find_distance: i32,
    /// `getDropOff`.
    pub level_decrease_per_block: i32,
    pub explosion_resistance: f32,
    /// `FluidType.canConvertToSource`: two sources make a third.
    pub can_convert_to_source: bool,
    pub light_level: u8,
    pub density: i32,
    pub temperature: i32,
    pub viscosity: i32,
    /// `getStillTexture`, `getFlowingTexture`, `getOverlayTexture`: block
    /// atlas sprites, `<ns>:<path>` under `textures/`.
    pub still_texture: String,
    pub flowing_texture: String,
    pub overlay_texture: Option<String>,
    /// `getTintColor`, ARGB (`0xFFFFFFFF` leaves the texture as it is).
    pub tint: u32,
    /// `ItemBlockRenderTypes.getRenderLayer(FluidState)`: translucent, or
    /// NeoForge's default, solid.
    pub translucent: bool,
    /// `SoundActions.BUCKET_FILL` and `BUCKET_EMPTY`.
    pub fill_sound: Option<String>,
    pub empty_sound: Option<String>,
    /// `FluidType.getDescriptionId`.
    pub description: String,
    /// The fluid tags its source and its flowing fluid are in.
    pub source_tags: Vec<String>,
    pub flowing_tags: Vec<String>,
    /// What its `FluidType` does to an entity in it.
    pub entity: FluidEntity,
    /// `IClientFluidTypeExtensions.getRenderOverlayTexture`: the camera's
    /// overlay with the eye in it (a texture's resource path,
    /// `minecraft:textures/misc/underwater.png`), none for none.
    pub render_overlay: Option<String>,
    /// `IClientFluidTypeExtensions.modifyFogColor`: the fog colour (RGB, 0 to
    /// 1) with the camera in it, none to leave the fog's own.
    pub fog_color: Option<[f32; 3]>,
    /// `IClientFluidTypeExtensions.modifyFogRender` over the terrain at a
    /// 12-chunk render distance: the fog's start and end in blocks, none to
    /// leave the fog's own.
    pub fog_range: Option<[f32; 2]>,
    /// `SoundActions.FLUID_VAPORIZE`: `onVaporize`'s sound, none for
    /// `block.fire.extinguish`.
    pub vaporize_sound: Option<String>,
    /// Where its bucket vaporizes (`FluidType.isVaporizedOnPlacement`).
    pub vaporizes: Vaporizes,
    /// The cauldron that holds it (`CauldronFluidContent`), none for none.
    pub cauldron: Option<ModCauldron>,
    /// Its type's `getDripInfo`: what a pointed dripstone under a source of
    /// it drips and the cauldron that fills, none for none.
    pub drip: Option<ModDrip>,
    /// `SoundActions.CAULDRON_DRIP`, played as a drip fills a cauldron
    /// (`FluidType.handleCauldronDrip`); none is silent.
    pub cauldron_drip_sound: Option<String>,
    /// Its type's `setItemMovement` for a dropped item in it.
    pub item_movement: ItemMovement,
}

/// NeoForge's `FluidType.DripstoneDripInfo`.
#[derive(Clone, Debug, PartialEq)]
pub struct ModDrip {
    /// `chance`: a random tick's `randChance` below it drips.
    pub chance: f32,
    /// `dripParticle`, a particle type id; none draws nothing.
    pub particle: Option<String>,
    /// `filledCauldron`: what an empty cauldron under the tip becomes.
    pub cauldron: String,
}

/// `FluidType.setItemMovement` in the form NeoForge's default has
/// (`x * horizontal`, `y + below` under `0.06F` else `y + above`,
/// `z * horizontal`), with its numbers ([`ItemMovement::default`]: water's).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemMovement {
    pub horizontal: f64,
    pub below: f64,
    pub above: f64,
}

impl Default for ItemMovement {
    /// `FluidType.setItemMovement`: `x * 0.99F`, `y + 5.0E-4F` below
    /// `0.06F`, `z * 0.99F`.
    fn default() -> Self {
        Self {
            horizontal: f64::from(0.99f32),
            below: f64::from(5.0e-4f32),
            above: 0.0,
        }
    }
}

impl ItemMovement {
    /// The item's velocity after `setItemMovement`.
    pub fn apply(&self, [x, y, z]: [f64; 3]) -> [f64; 3] {
        let add = if y < f64::from(0.06f32) {
            self.below
        } else {
            self.above
        };
        [x * self.horizontal, y + add, z * self.horizontal]
    }
}

/// Where a bucket of a fluid vaporizes instead of being placed: what its
/// type's `FluidType.isVaporizedOnPlacement(level, pos, stack)` answers, by
/// the NeoForge oracle (`ModFluid::is_vaporized_on_placement`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Vaporizes {
    /// `FluidType`'s own: in an `ultraWarm` dimension (26.3's
    /// `water_evaporates`) when the fluid it places is in `minecraft:water`.
    #[default]
    LikeWater,
    /// In an `ultraWarm` dimension, whatever its tags.
    Ultrawarm,
    Never,
    Always,
}

/// A mod's cauldron for its fluid, as `RegisterCauldronFluidContentEvent`
/// registers one: `total_amount` millibuckets over `max_level` levels of
/// `level_property` (one level without one).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModCauldron {
    pub block: String,
    pub total_amount: i32,
    pub max_level: i32,
    pub level_property: Option<String>,
}

/// NeoForge's `CauldronFluidContent`: a cauldron block, the fluid it holds
/// (the empty one for the empty cauldron), the amount of a full one and its
/// levels. Vanilla's three are `CauldronFluidContent.init`'s, the rest the
/// mods' ([`ModCauldron`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CauldronFluidContent {
    pub block: &'static str,
    pub fluid: FluidKind,
    pub total_amount: i32,
    pub max_level: i32,
    pub level_property: Option<&'static str>,
}

impl CauldronFluidContent {
    /// `currentLevel(state)`: none for the empty cauldron, 1 without a level
    /// property, else `level` (the state's value of [`Self::level_property`]).
    pub fn current_level(&self, level: Option<i32>) -> i32 {
        if self.fluid == FluidKind::Empty {
            0
        } else if self.level_property.is_none() {
            1
        } else {
            level.unwrap_or(0)
        }
    }
}

/// `CauldronFluidContent.init`: the empty cauldron, water's in thirds and
/// lava's whole, a bucket each.
const VANILLA_CAULDRONS: [CauldronFluidContent; 3] = [
    CauldronFluidContent {
        block: "minecraft:cauldron",
        fluid: FluidKind::Empty,
        total_amount: 1000,
        max_level: 1,
        level_property: None,
    },
    CauldronFluidContent {
        block: "minecraft:water_cauldron",
        fluid: FluidKind::Water,
        total_amount: 1000,
        // `LayeredCauldronBlock.LEVEL`: 1 to 3.
        max_level: 3,
        level_property: Some("level"),
    },
    CauldronFluidContent {
        block: "minecraft:lava_cauldron",
        fluid: FluidKind::Lava,
        total_amount: 1000,
        max_level: 1,
        level_property: None,
    },
];

/// `CauldronFluidContent.getForBlock`: the cauldron `block` is, if any.
pub fn cauldron_for_block(block: &str) -> Option<CauldronFluidContent> {
    VANILLA_CAULDRONS
        .iter()
        .copied()
        .find(|c| c.block == block)
        .or_else(|| mod_cauldrons().find(|c| c.block == block))
}

/// `CauldronFluidContent.getForFluid`: the cauldron that holds `fluid` (that
/// fluid itself: flowing water has none).
pub fn cauldron_for_fluid(fluid: FluidKind) -> Option<CauldronFluidContent> {
    VANILLA_CAULDRONS
        .iter()
        .copied()
        .find(|c| c.fluid == fluid)
        .or_else(|| mod_cauldrons().find(|c| c.fluid == fluid))
}

fn mod_cauldrons() -> impl Iterator<Item = CauldronFluidContent> {
    mod_fluids().iter().filter_map(|m| {
        let c = m.cauldron.as_ref()?;
        Some(CauldronFluidContent {
            block: c.block.as_str(),
            fluid: kind_by_name(&m.id)?,
            total_amount: c.total_amount,
            max_level: c.max_level,
            level_property: c.level_property.as_deref(),
        })
    })
}

/// `FluidType`'s say over an entity in it (each one `FluidType.canSwim(Entity)`
/// and the others, asked of a pig by the NeoForge oracle), with
/// `FluidType.Properties`' defaults ([`FluidEntity::default`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FluidEntity {
    /// `canSwim`: a sprinting entity swims in it, a mob floats up.
    pub can_swim: bool,
    /// `canDrownIn`: an eye in it uses up air.
    pub can_drown: bool,
    /// `canPushEntity`: its flow pushes.
    pub can_push: bool,
    /// `motionScale`: how hard its flow pushes.
    pub motion_scale: f64,
    /// `getFallDistanceModifier`: what an entity's fall distance is
    /// multiplied by each tick it is in it.
    pub fall_distance_modifier: f32,
    /// `canExtinguish`: it puts a burning entity out.
    pub can_extinguish: bool,
    /// `canHydrate`: asked of a pig, the same as `canHydrate(FluidState,
    /// ...)` (what farmland, sponges and coral blocks ask) for every type of
    /// ATM10 8.4 by the oracle's run.
    pub can_hydrate: bool,
    /// `supportsBoating`: asked with a boat, the same as its `FluidState`
    /// form (`IBoatExtension.canBoatInFluid`) for every type of ATM10 8.4.
    pub supports_boating: bool,
}

impl Default for FluidEntity {
    /// `FluidType.Properties`: swim, drown, push at 0.014, fall distance
    /// halved, no extinguishing, hydrating or boats.
    fn default() -> Self {
        Self {
            can_swim: true,
            can_drown: true,
            can_push: true,
            motion_scale: 0.014,
            fall_distance_modifier: 0.5,
            can_extinguish: false,
            can_hydrate: false,
            supports_boating: false,
        }
    }
}

impl ModFluid {
    /// `id` with `BaseFlowingFluid.Properties`' and `FluidType.Properties`'
    /// defaults (tick rate 5, slope distance 4, drop-off 1, density and
    /// viscosity 1000, 300 K), `flowing_<path>` for its flowing fluid and its
    /// own path for its block, as `FluidDeferredRegister` names them.
    pub fn new(id: &str) -> Self {
        let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
        Self {
            id: id.to_owned(),
            flowing: format!("{ns}:flowing_{path}"),
            block: id.to_owned(),
            bucket: None,
            tick_rate: 5,
            slope_find_distance: 4,
            level_decrease_per_block: 1,
            explosion_resistance: 1.0,
            can_convert_to_source: false,
            light_level: 0,
            density: 1000,
            temperature: 300,
            viscosity: 1000,
            still_texture: String::new(),
            flowing_texture: String::new(),
            overlay_texture: None,
            tint: 0xFFFF_FFFF,
            translucent: false,
            fill_sound: None,
            empty_sound: None,
            description: format!("block.{ns}.{path}"),
            source_tags: Vec::new(),
            flowing_tags: Vec::new(),
            entity: FluidEntity::default(),
            render_overlay: None,
            fog_color: None,
            fog_range: None,
            vaporize_sound: None,
            vaporizes: Vaporizes::LikeWater,
            cauldron: None,
            drip: None,
            cauldron_drip_sound: None,
            item_movement: ItemMovement::default(),
        }
    }
    /// Whether it is a `FlowingFluid`: one that doesn't (Modern
    /// Industrialization's) has no flowing fluid and no `LiquidBlock`, and
    /// its bucket places nothing.
    pub fn flows(&self) -> bool {
        !self.flowing.is_empty()
    }
    /// Whether `kind`'s fluid tags hold `tag`.
    pub fn in_tag(&self, kind: FluidKind, tag: &str) -> bool {
        let tags = if kind.is_source_kind() {
            &self.source_tags
        } else {
            &self.flowing_tags
        };
        tags.iter().any(|t| t == tag)
    }
    /// `FluidType.isVaporizedOnPlacement(level, pos, stack)` for a bucket of
    /// it (its source) in a level where water evaporates or not (26.3's
    /// `water_evaporates`, NeoForge's `dimensionType().ultraWarm()`).
    pub fn is_vaporized_on_placement(&self, water_evaporates: bool) -> bool {
        match self.vaporizes {
            // `getStateForPlacement(level, pos, stack).is(FluidTags.WATER)`:
            // the stack's fluid's default state, its source.
            Vaporizes::LikeWater => {
                water_evaporates && self.source_tags.iter().any(|t| t == "minecraft:water")
            }
            Vaporizes::Ultrawarm => water_evaporates,
            Vaporizes::Never => false,
            Vaporizes::Always => true,
        }
    }
    /// `FluidType.onVaporize`'s sound: `FLUID_VAPORIZE`, else
    /// `SoundEvents.FIRE_EXTINGUISH`.
    pub fn vaporize_sound(&self) -> &str {
        self.vaporize_sound
            .as_deref()
            .unwrap_or("minecraft:block.fire.extinguish")
    }
}

struct Registry {
    fluids: Vec<ModFluid>,
    by_name: HashMap<String, FluidKind>,
    /// Source kinds by liquid block and by bucket.
    by_block: HashMap<String, FluidKind>,
    by_bucket: HashMap<String, FluidKind>,
    /// The fluid tags of vanilla's own fluids, by [`FluidKind::id`]: the
    /// root's, over [`VANILLA_TAGS`].
    vanilla_tags: [Vec<String>; 5],
    /// Each tag's fluids in the tag's own order, when the root's tags were
    /// given ([`register_fluids`]).
    members: HashMap<String, Vec<FluidKind>>,
}

/// The tags vanilla's own data (`minecraft:water`, `minecraft:lava`) and
/// NeoForge's (`Tags.Fluids.WATER`, `LAVA`: `c:water`, `c:lava`) give its
/// fluids, whatever the mods' data adds.
const VANILLA_TAGS: [(&str, [FluidKind; 2]); 4] = [
    (
        "minecraft:water",
        [FluidKind::Water, FluidKind::FlowingWater],
    ),
    ("minecraft:lava", [FluidKind::Lava, FluidKind::FlowingLava]),
    ("c:water", [FluidKind::Water, FluidKind::FlowingWater]),
    ("c:lava", [FluidKind::Lava, FluidKind::FlowingLava]),
];

fn default_vanilla_tags() -> [Vec<String>; 5] {
    std::array::from_fn(|id| {
        VANILLA_TAGS
            .iter()
            .filter(|(_, kinds)| kinds.iter().any(|k| usize::from(k.id()) == id))
            .map(|(tag, _)| (*tag).to_owned())
            .collect()
    })
}

static REGISTRY: OnceLock<Registry> = OnceLock::new();

/// Registers the content mods' fluids, by id (the registry order, as the mods'
/// mob effects). The list is set once per process: false when it already
/// was, and the fluids stay those of the first call until the game restarts.
/// A fluid whose id, flowing id, block or bucket another one already took is
/// left out.
pub fn register_mod_fluids(fluids: impl IntoIterator<Item = ModFluid>) -> bool {
    register_fluids(fluids, None)
}

/// [`register_mod_fluids`] with the root's fluid tags (`tags`: each tag's
/// fluid ids in order, [`fluid_tag_lists_in`]), which also give vanilla's
/// own fluids theirs (the full pack's `create:bottomless/allow` holds water)
/// and each tag's order (`TagFluidIngredient.generateStacks`). A mod
/// fluid's own tags stay its [`ModFluid::source_tags`] and `flowing_tags`.
pub fn register_fluids(
    fluids: impl IntoIterator<Item = ModFluid>,
    tags: Option<&BTreeMap<String, Vec<String>>>,
) -> bool {
    if REGISTRY.get().is_some() {
        return false;
    }
    let mut fluids: Vec<ModFluid> = fluids.into_iter().collect();
    fluids.sort_by(|a, b| a.id.cmp(&b.id));
    let mut registry = Registry {
        fluids: Vec::with_capacity(fluids.len()),
        by_name: HashMap::new(),
        by_block: HashMap::new(),
        by_bucket: HashMap::new(),
        vanilla_tags: default_vanilla_tags(),
        members: HashMap::new(),
    };
    for fluid in fluids {
        let taken = |name: &str| {
            FluidKind::ALL.iter().any(|k| k.name() == name)
                || registry.by_name.contains_key(name)
                || registry.by_block.contains_key(name)
        };
        if fluid.id == fluid.flowing
            || taken(&fluid.id)
            || (fluid.flows() && taken(&fluid.flowing))
            || (fluid.block != "minecraft:air" && registry.by_block.contains_key(&fluid.block))
            || fluid
                .bucket
                .as_ref()
                .is_some_and(|b| registry.by_bucket.contains_key(b))
        {
            log::warn!(
                "Horizon: mod fluid {} left out: an id it uses is taken",
                fluid.id
            );
            continue;
        }
        let (source, flowing) = FluidKind::of_mod(registry.fluids.len());
        registry.by_name.insert(fluid.id.clone(), source);
        if fluid.flows() {
            registry.by_name.insert(fluid.flowing.clone(), flowing);
        }
        if fluid.block != "minecraft:air" {
            registry.by_block.insert(fluid.block.clone(), source);
        }
        if let Some(bucket) = &fluid.bucket {
            registry.by_bucket.insert(bucket.clone(), source);
        }
        registry.fluids.push(fluid);
    }
    if let Some(tags) = tags {
        let kind = |name: &str| {
            FluidKind::ALL
                .into_iter()
                .find(|k| k.name() == name)
                .or_else(|| registry.by_name.get(name).copied())
        };
        for (tag, names) in tags {
            // Vanilla's data pack loads before the mods': its members first.
            let mut members: Vec<FluidKind> = VANILLA_TAGS
                .iter()
                .filter(|(t, _)| t == tag)
                .flat_map(|(_, kinds)| *kinds)
                .collect();
            for k in names.iter().filter_map(|n| kind(n)) {
                if k != FluidKind::Empty && !members.contains(&k) {
                    members.push(k);
                }
            }
            for k in members.iter().filter(|k| k.id() < 5) {
                let held = &mut registry.vanilla_tags[usize::from(k.id())];
                if !held.contains(tag) {
                    held.push(tag.clone());
                }
            }
            registry.members.insert(tag.clone(), members);
        }
    }
    REGISTRY.set(registry).is_ok()
}

/// Whether vanilla's fluid `kind` is in `tag`: the root's tags when the
/// fluids were registered with them, else vanilla's and NeoForge's water and
/// lava tags ([`VANILLA_TAGS`]). False for a mod fluid and the empty fluid.
pub fn vanilla_in_tag(kind: FluidKind, tag: &str) -> bool {
    if kind == FluidKind::Empty || kind.id() >= 5 {
        return false;
    }
    match REGISTRY.get() {
        Some(r) => r.vanilla_tags[usize::from(kind.id())]
            .iter()
            .any(|t| t == tag),
        None => VANILLA_TAGS
            .iter()
            .any(|(t, kinds)| *t == tag && kinds.contains(&kind)),
    }
}

/// `BuiltInRegistries.FLUID.getTag(tag)`: the fluids of `tag` in the tag's
/// order when the root's tags were registered, else (and for a tag the root
/// lacks) every fluid in the tag in registry order. Empty for an unknown tag.
pub fn fluids_in_tag(tag: &str) -> Vec<FluidKind> {
    if let Some(members) = REGISTRY.get().and_then(|r| r.members.get(tag)) {
        return members.clone();
    }
    FluidKind::all()
        .filter(|&k| match k.mod_fluid() {
            Some(m) => !k.name().is_empty() && m.in_tag(k, tag),
            None => vanilla_in_tag(k, tag),
        })
        .collect()
}

/// The content mods' fluids, in registry order.
pub fn mod_fluids() -> &'static [ModFluid] {
    REGISTRY.get().map_or(&[], |r| r.fluids.as_slice())
}

/// A mod's source or flowing fluid by id.
pub fn kind_by_name(name: &str) -> Option<FluidKind> {
    REGISTRY.get()?.by_name.get(name).copied()
}

/// The source fluid whose `LiquidBlock` is `block`.
pub fn kind_by_block(block: &str) -> Option<FluidKind> {
    let registry = REGISTRY.get()?;
    if registry.by_block.is_empty() {
        return None;
    }
    registry.by_block.get(block).copied()
}

/// The source fluid a filled bucket `item` holds (`BucketItem.content`).
pub fn kind_by_bucket(item: &str) -> Option<FluidKind> {
    REGISTRY.get()?.by_bucket.get(item).copied()
}

/// The fluid tags of a resource root, by fluid id ([`fluid_tag_lists_in`]
/// turned round): what a [`ModFluid`]'s source and flowing tags take.
pub fn fluid_tags_in(
    root: &Path,
    known: &dyn Fn(&str) -> bool,
) -> io::Result<BTreeMap<String, Vec<String>>> {
    let mut by_fluid: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (tag, members) in fluid_tag_lists_in(root, known)? {
        for fluid in members {
            by_fluid.entry(fluid).or_default().push(tag.clone());
        }
    }
    Ok(by_fluid)
}

/// The fluid tags of a resource root (`data/<ns>/tags/fluid/**.json`, a
/// content root's merged files), resolved as `TagLoader` does: each tag's
/// fluid ids in its order. `known` names the fluids the game has. An entry
/// naming another (a fluid of a mod not carried: ATM10's `c:heavy_water` also
/// names Modern Industrialization's) is left out, not the whole tag, as
/// damage tags lose only their missing references; a missing tag reference
/// still leaves the tag out.
pub fn fluid_tag_lists_in(
    root: &Path,
    known: &dyn Fn(&str) -> bool,
) -> io::Result<BTreeMap<String, Vec<String>>> {
    let mut definitions = TagDefinitions::default();
    let data = root.join("data");
    let mut namespaces: Vec<_> = match fs::read_dir(&data) {
        Ok(dir) => dir.flatten().map(|e| e.path()).collect(),
        Err(_) => return Ok(BTreeMap::new()),
    };
    namespaces.sort();
    for ns_dir in namespaces {
        let Some(ns) = ns_dir
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_owned)
        else {
            continue;
        };
        let base = ns_dir.join("tags/fluid");
        let mut stack = vec![base.clone()];
        let mut files = Vec::new();
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "json") {
                    files.push(path);
                }
            }
        }
        files.sort();
        for file in files {
            let Some(rel) = file
                .strip_prefix(&base)
                .ok()
                .and_then(|p| p.with_extension("").to_str().map(|s| s.replace('\\', "/")))
            else {
                continue;
            };
            // A file that doesn't decode is left out, as vanilla's loader logs and skips it.
            let _ =
                definitions.apply_json(&format!("{ns}:{rel}"), &ns, &fs::read_to_string(&file)?);
        }
    }
    let built = definitions
        .build(|name, _| Some(name.to_owned()))
        .map_err(|e| invalid(e.to_string()))?;
    Ok(built
        .tags
        .into_iter()
        .map(|(tag, members)| (tag, members.into_iter().filter(|f| known(f)).collect()))
        .collect())
}

/// The fluids unit tests share, registered once per test process (any test
/// that needs a mod fluid calls this first): `test:brine` with
/// `BaseFlowingFluid`'s and `FluidType`'s defaults (to an entity, Mekanism's
/// heavy water's: its overlay and fog too), and `test:sludge`, slower and
/// thicker, which makes sources and is a molten metal to an entity (All the
/// Ores': no swimming or drowning, lava's push), puts fires out and keeps a
/// fifth of a fall, vaporizes where water evaporates with a sound of its own
/// (Create Dragons Plus' dyes) and has a cauldron of four levels (its dragon's
/// breath). Not for the game.
#[doc(hidden)]
pub fn register_test_fluids() {
    let brine = ModFluid {
        bucket: Some("test:brine_bucket".into()),
        still_texture: "test:liquid/liquid".into(),
        flowing_texture: "test:liquid/liquid_flow".into(),
        overlay_texture: Some("minecraft:block/water_overlay".into()),
        tint: 0xFFFE_EF9C,
        translucent: true,
        fill_sound: Some("minecraft:item.bucket.fill".into()),
        empty_sound: Some("minecraft:item.bucket.empty".into()),
        source_tags: vec!["c:brine".into()],
        flowing_tags: vec!["c:brine".into()],
        render_overlay: Some("minecraft:textures/misc/underwater.png".into()),
        fog_color: Some([0.050_980_393, 0.078_431_375, 0.333_333_34]),
        fog_range: Some([-8.0, 24.0]),
        // Water's say over blocks and boats (the Bumblezone's sugar water's).
        entity: FluidEntity {
            can_hydrate: true,
            supports_boating: true,
            ..FluidEntity::default()
        },
        ..ModFluid::new("test:brine")
    };
    let sludge = ModFluid {
        bucket: Some("test:sludge_bucket".into()),
        tick_rate: 10,
        slope_find_distance: 2,
        level_decrease_per_block: 2,
        can_convert_to_source: true,
        still_texture: "test:liquid/sludge".into(),
        flowing_texture: "test:liquid/sludge_flow".into(),
        entity: FluidEntity {
            can_swim: false,
            can_drown: false,
            motion_scale: 0.002_333_333_333_333_333_5,
            fall_distance_modifier: 0.2,
            can_extinguish: true,
            ..FluidEntity::default()
        },
        vaporize_sound: Some("minecraft:block.lava.extinguish".into()),
        vaporizes: Vaporizes::Ultrawarm,
        cauldron: Some(ModCauldron {
            block: "test:sludge_cauldron".into(),
            total_amount: 1000,
            max_level: 4,
            level_property: Some("level".into()),
        }),
        // The Undergarden's virulent mix's: drips as lava does into its own
        // cauldron with lava's sound; Create Dragons Plus' dragon's breath's
        // dropped items.
        drip: Some(ModDrip {
            chance: 0.058_593_75,
            particle: Some("test:dripping_sludge".into()),
            cauldron: "test:sludge_cauldron".into(),
        }),
        cauldron_drip_sound: Some(
            "minecraft:block.pointed_dripstone.drip_lava_into_cauldron".into(),
        ),
        item_movement: ItemMovement {
            horizontal: f64::from(0.95f32),
            ..ItemMovement::default()
        },
        ..ModFluid::new("test:sludge")
    };
    if !register_mod_fluids([brine.clone(), sludge.clone()]) {
        assert_eq!(
            mod_fluids(),
            [brine, sludge],
            "another set of mod fluids is registered in this test process"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluid::FluidState;

    #[test]
    fn mod_fluids_follow_vanilla_in_the_registry() {
        register_test_fluids();
        let brine = FluidKind::from_name("test:brine").unwrap();
        let flowing = FluidKind::from_name("test:flowing_brine").unwrap();
        let sludge = FluidKind::from_name("test:sludge").unwrap();
        // flowing then source, after vanilla's five; by id.
        assert_eq!((flowing.id(), brine.id(), sludge.id()), (5, 6, 8));
        assert!(brine.same_family(flowing) && !brine.same_family(sludge));
        assert!(!brine.same_family(FluidKind::Water) && !flowing.same_family(FluidKind::Lava));
        assert_eq!((flowing.source(), brine.flowing()), (brine, flowing));
        assert!(brine.is_source_kind() && !flowing.is_source_kind());
        assert_eq!(brine.name(), "test:brine");
        assert_eq!(flowing.name(), "test:flowing_brine");
        assert_eq!(FluidKind::from_id(9), None);
        assert_eq!(FluidKind::all().count(), 9);
        assert_eq!(kind_by_bucket("test:brine_bucket"), Some(brine));
        assert_eq!(kind_by_block("test:sludge"), Some(sludge));
        assert_eq!(brine.mask(), 0, "a mod fluid is in no vanilla tag mask");
        assert!(brine.mod_fluid().unwrap().in_tag(flowing, "c:brine"));
        // Vanilla's ids and families are as they were.
        assert_eq!(FluidKind::Water.flowing(), FluidKind::FlowingWater);
        assert_eq!(FluidKind::FlowingLava.source(), FluidKind::Lava);
        assert_eq!(FluidKind::Empty.source(), FluidKind::Empty);
    }

    #[test]
    fn a_type_vaporizes_where_its_oracle_says_and_cauldrons_hold_their_fluid() {
        register_test_fluids();
        let brine = FluidKind::from_name("test:brine").unwrap();
        let sludge = FluidKind::from_name("test:sludge").unwrap();
        // FluidType's own rule: only a fluid in minecraft:water, and only where water evaporates.
        let mut water_tagged = brine.mod_fluid().unwrap().clone();
        assert!(
            !water_tagged.is_vaporized_on_placement(true),
            "brine is in no water tag"
        );
        water_tagged.source_tags.push("minecraft:water".into());
        assert!(water_tagged.is_vaporized_on_placement(true));
        assert!(!water_tagged.is_vaporized_on_placement(false));
        // A type of its own: where water evaporates, whatever its tags, with its own sound.
        let m = sludge.mod_fluid().unwrap();
        assert!(m.is_vaporized_on_placement(true) && !m.is_vaporized_on_placement(false));
        assert_eq!(m.vaporize_sound(), "minecraft:block.lava.extinguish");
        assert_eq!(
            brine.mod_fluid().unwrap().vaporize_sound(),
            "minecraft:block.fire.extinguish"
        );
        for (rule, nether, overworld) in [
            (Vaporizes::Never, false, false),
            (Vaporizes::Always, true, true),
        ] {
            let f = ModFluid {
                vaporizes: rule,
                ..m.clone()
            };
            assert_eq!(
                (
                    f.is_vaporized_on_placement(true),
                    f.is_vaporized_on_placement(false)
                ),
                (nether, overworld)
            );
        }
        // CauldronFluidContent: vanilla's three, then the mods'.
        let water = cauldron_for_block("minecraft:water_cauldron").unwrap();
        assert_eq!(
            (
                water.fluid,
                water.total_amount,
                water.max_level,
                water.level_property
            ),
            (FluidKind::Water, 1000, 3, Some("level"))
        );
        assert_eq!(
            cauldron_for_fluid(FluidKind::Lava).unwrap().block,
            "minecraft:lava_cauldron"
        );
        assert_eq!(
            cauldron_for_fluid(FluidKind::FlowingWater),
            None,
            "keyed by the fluid itself"
        );
        let empty = cauldron_for_block("minecraft:cauldron").unwrap();
        assert_eq!(
            (empty.fluid, empty.current_level(None)),
            (FluidKind::Empty, 0)
        );
        let own = cauldron_for_fluid(sludge).unwrap();
        assert_eq!(
            (own.block, own.max_level, own.current_level(Some(2))),
            ("test:sludge_cauldron", 4, 2)
        );
        assert_eq!(cauldron_for_block("test:sludge_cauldron"), Some(own));
        assert_eq!(cauldron_for_fluid(brine), None);
        assert_eq!(
            cauldron_for_block("minecraft:powder_snow_cauldron"),
            None,
            "powder snow holds no fluid"
        );
    }

    #[test]
    fn a_mod_liquid_block_holds_its_fluid_by_level() {
        register_test_fluids();
        let brine = FluidKind::from_name("test:brine").unwrap();
        let block = |level: u8| crate::chunk::BlockState {
            name: "test:brine".into(),
            properties: [("level".to_owned(), level.to_string())]
                .into_iter()
                .collect(),
        };
        let source = FluidState::from_block(&block(0)).unwrap();
        assert_eq!(
            (source.kind(), source.amount(), source.falling()),
            (brine, 8, false)
        );
        assert!(source.is_source());
        let flowing = FluidState::from_block(&block(3)).unwrap();
        assert_eq!(
            (flowing.kind(), flowing.amount(), flowing.falling()),
            (brine.flowing(), 5, false)
        );
        let falling = FluidState::from_block(&block(9)).unwrap();
        assert_eq!((falling.amount(), falling.falling()), (8, true));
        // The JSON form names the mod's fluids.
        let json = flowing.to_json().unwrap();
        assert_eq!(FluidState::from_json(&json).unwrap(), flowing);
        assert!(json.contains("test:flowing_brine"), "{json}");
    }

    #[test]
    fn fluid_tags_resolve_references_and_drop_unknown_fluids() {
        let dir = std::env::temp_dir().join(format!("horizon-fluid-tags-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let write = |rel: &str, text: &str| {
            let path = dir.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        };
        write(
            "data/c/tags/fluid/brine.json",
            r#"{"values":["test:brine","test:flowing_brine"]}"#,
        );
        write(
            "data/c/tags/fluid/salty.json",
            r##"{"values":["#c:brine",{"id":"other:gone","required":false},"other:absent"]}"##,
        );
        write(
            "data/minecraft/tags/fluid/water.json",
            r#"{"values":["minecraft:water","minecraft:flowing_water"]}"#,
        );
        let known = |id: &str| id.starts_with("test:") || id.starts_with("minecraft:");
        let tags = fluid_tags_in(&dir, &known).unwrap();
        assert_eq!(tags["test:brine"], ["c:brine", "c:salty"]);
        assert_eq!(tags["test:flowing_brine"], ["c:brine", "c:salty"]);
        assert_eq!(tags["minecraft:water"], ["minecraft:water"]);
        assert!(!tags.contains_key("other:gone") && !tags.contains_key("other:absent"));
        let _ = fs::remove_dir_all(&dir);
    }
}
