//! A pack's fluids from the NeoForge oracle (docs/mod-fluids.md, slice 2): what the server's run writes of each fluid
//! and fluid type (`FluidFacts`, in each batch's facts) and what the client's run writes of them
//! (`ClientFactsExporter`, `client-facts.json`), and the content mod declaration
//! (`data/<ns>/horizon/fluid/<path>.json`, `horizon_mods::content::CarriedFluid`) made from both: Java's own numbers,
//! textures, tint and render layer, none guessed.
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::{collections::BTreeMap, path::Path};

/// A fluid as the server holds it: `Fluid` and, for a `FlowingFluid`, its source, flowing fluid and numbers.
#[derive(Debug, Deserialize)]
pub struct Fluid {
    /// The class and its superclasses, most derived first.
    #[serde(default)]
    pub class: Vec<String>,
    /// Its `FluidType`'s id.
    #[serde(rename = "type")]
    pub fluid_type: Option<String>,
    /// `isSource(defaultFluidState())`.
    pub is_source: bool,
    /// `FlowingFluid.getSource()`, `getFlowing()`; none for a fluid that doesn't flow.
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub flowing: Option<String>,
    /// `getSlopeFindDistance`, `getDropOff` and `canConvertToSource(state, level, pos)` on the overworld.
    #[serde(default)]
    pub slope_find_distance: Option<i32>,
    #[serde(default)]
    pub drop_off: Option<i32>,
    #[serde(default)]
    pub can_convert_to_source: Option<bool>,
    /// The block a source of it is (`createLegacyBlock`): `minecraft:air` when it has none.
    pub block: String,
    /// `getBucket`: `minecraft:air` when it has none.
    pub bucket: String,
    /// `getTickDelay`.
    pub tick_delay: i32,
    /// `getExplosionResistance`.
    pub explosion_resistance: f32,
    #[serde(default)]
    pub pickup_sound: Option<String>,
    /// What the light engine reads of its liquid block in place (NeoForge's
    /// `getLightEmission(level, pos)`): the block's own light, not its type's
    /// (slice 7b's run).
    #[serde(default)]
    pub block_light: Option<i32>,
}

/// A `FluidType` as the server holds it.
#[derive(Debug, Deserialize)]
pub struct FluidType {
    pub description: String,
    pub light_level: i32,
    pub density: i32,
    pub temperature: i32,
    pub viscosity: i32,
    /// `getSound(SoundActions.BUCKET_FILL)`... by action (`bucket_fill`, `bucket_empty`, `fluid_vaporize`).
    #[serde(default)]
    pub sounds: BTreeMap<String, String>,
    /// What it does to an entity in it, asked of a pig and a boat (`canSwim`, `canDrownIn`, `canPushEntity`,
    /// `motionScale`, `getFallDistanceModifier`, `canExtinguish`, `canHydrate`, `supportsBoating`): written into
    /// the declaration as the oracle gives it.
    #[serde(default)]
    pub entity: Option<Map<String, Value>>,
    /// `isVaporizedOnPlacement(level, pos, a bucket of it)` in each dimension of the run, by dimension id (slice 7);
    /// none in facts from before it was asked.
    #[serde(default)]
    pub vaporized_on_placement: Option<BTreeMap<String, bool>>,
    /// `getDripInfo` (slice 7b): the chance, the particle type and the cauldron it fills.
    #[serde(default)]
    pub drip: Option<Map<String, Value>>,
    /// `setItemMovement` measured (slice 7b): `horizontal`, `below`, `above` in NeoForge's default's form, or the
    /// `probes` when it has another.
    #[serde(default)]
    pub item_movement: Option<Map<String, Value>>,
}

/// NeoForge's `CauldronFluidContent` of a cauldron block, as the server's run writes it (`cauldrons`).
#[derive(Debug, Deserialize)]
pub struct Cauldron {
    pub fluid: String,
    pub total_amount: i32,
    pub max_level: i32,
    #[serde(default)]
    pub level_property: Option<String>,
}

/// What the server's run says of its world beyond each fluid and type: which of its dimensions are `ultraWarm`
/// (what tells a type that vaporizes where water evaporates from one that always or never does) and its
/// cauldrons. Empty in facts from before slice 7.
#[derive(Clone, Copy)]
pub struct FluidWorld<'a> {
    pub ultrawarm: &'a BTreeMap<String, bool>,
    pub cauldrons: &'a BTreeMap<String, Cauldron>,
}

/// The facts of a fluids-only run of the server's oracle (`horizon.oracle.fluids_only`, `fluid-facts.json`): every
/// namespace's fluids and types with the questions later slices ask, and the world's. Laid over a batch's facts
/// ([`FluidProbe::lay_over`]), its entries replace the batch's, so a fluid probe needs no run of the whole pack.
#[derive(Debug, Default, Deserialize)]
pub struct FluidProbe {
    #[serde(default)]
    pub fluids: BTreeMap<String, Fluid>,
    #[serde(default)]
    pub fluid_types: BTreeMap<String, FluidType>,
    #[serde(default)]
    pub ultrawarm: BTreeMap<String, bool>,
    #[serde(default)]
    pub cauldrons: BTreeMap<String, Cauldron>,
    /// NeoForge's `FluidInteractionRegistry`, by fluid type, each list in its order (slice 7b).
    #[serde(default)]
    pub interactions: BTreeMap<String, Vec<InteractionFacts>>,
}

/// One `InteractionInformation` as the fluids-only run ran it on its grid of worlds (`FluidInteractionFacts`).
#[derive(Debug, Default, Deserialize)]
pub struct InteractionFacts {
    pub index: usize,
    /// What the run threw, if it did.
    #[serde(default)]
    pub error: Option<String>,
    /// The classes its predicate's and its interaction's lambdas were declared in.
    #[serde(default)]
    pub predicate: String,
    #[serde(default)]
    pub interaction: String,
    /// Its interaction is NeoForge's own (`setBlockAndUpdate` of `getState(currentState)` and the fizz).
    #[serde(default)]
    pub neoforge_interaction: bool,
    /// The fluid the grid put at `currentPos` (the type's first source with a liquid block); none for a type with
    /// none.
    #[serde(default)]
    pub current: Option<String>,
    /// What held with a source there, and with a flowing fluid.
    #[serde(default)]
    pub source: Option<InteractionGrid>,
    #[serde(default)]
    pub flowing: Option<InteractionGrid>,
    /// The block the interaction left for each current state (`source`, flowing amounts `1` to `7`, `falling`),
    /// and `<state>_changes_neighbours` where it changed more.
    #[serde(default)]
    pub results: BTreeMap<String, Value>,
}

/// Where a predicate held on the run's grid: the blocks below that matched (none: all of them), the neighbours
/// (fluids by id, blocks by id), and whether its matches were exactly every pair of the two.
#[derive(Debug, Deserialize, PartialEq)]
pub struct InteractionGrid {
    pub exact: bool,
    #[serde(default)]
    pub below: Option<Vec<String>>,
    #[serde(default)]
    pub fluids: Vec<String>,
    #[serde(default)]
    pub blocks: Vec<String>,
}

/// NeoForge's two interactions of its own, lava's first (`FluidInteractionRegistry`'s static block): the engine's.
const NEOFORGE_OWN: [(usize, &str); 2] = [
    (
        0,
        "net.neoforged.neoforge.fluids.FluidInteractionRegistry$InteractionInformation",
    ),
    (1, "net.neoforged.neoforge.fluids.FluidInteractionRegistry"),
];

/// The content declarations of the run's interactions (`horizon_mods::content::FluidInteractions`): one file a
/// type, `<ns>/<path>` of the fluid the run put at `currentPos`, its interactions in the registry's order, each the
/// rule its grid matched (a block below, when one counted, and the fluids or blocks beside) with the blocks the
/// interaction left for a source and a flowing fluid. NeoForge's own two for lava are the engine's and left out;
/// so is one the rule can't hold, with the reason (`<type>#<index>: why`).
pub fn interaction_files(
    interactions: &BTreeMap<String, Vec<InteractionFacts>>,
) -> (Vec<(String, Value)>, Vec<String>) {
    let (mut files, mut left_out) = (Vec::new(), Vec::new());
    for (ty, list) in interactions {
        let mut current = None;
        let mut rules = Vec::new();
        for i in list {
            if ty == "minecraft:lava"
                && NEOFORGE_OWN
                    .iter()
                    .any(|(at, class)| i.index == *at && i.predicate == *class)
            {
                continue;
            }
            match interaction_rule(i) {
                Ok(rule) => {
                    current = i.current.clone();
                    rules.push(rule);
                }
                Err(why) => left_out.push(format!("{ty}#{}: {why}", i.index)),
            }
        }
        let Some(current) = current else { continue };
        let Some((ns, path)) = current.split_once(':') else {
            continue;
        };
        files.push((
            format!("{ns}/{path}"),
            json!({"fluid": current, "interactions": rules}),
        ));
    }
    (files, left_out)
}

/// The declaration's rule of one interaction, or why it has none.
fn interaction_rule(i: &InteractionFacts) -> Result<Value, String> {
    if let Some(e) = &i.error {
        return Err(format!("the run threw {e}"));
    }
    if !i.neoforge_interaction {
        return Err(format!("an interaction of its own ({})", i.interaction));
    }
    let (Some(source), Some(flowing)) = (&i.source, &i.flowing) else {
        return Err("no fluid of the type with a block to run it on".into());
    };
    if !source.exact || !flowing.exact {
        return Err(format!(
            "its matches are no rule of a block below and one beside ({})",
            i.predicate
        ));
    }
    if source != flowing {
        return Err(format!(
            "it holds otherwise for a source than a flowing fluid ({})",
            i.predicate
        ));
    }
    if source.fluids.is_empty() && source.blocks.is_empty() {
        return Err(format!(
            "it held nowhere on the run's grid ({})",
            i.predicate
        ));
    }
    if i.results.keys().any(|k| k.ends_with("_changes_neighbours")) {
        return Err("its interaction changed more than the fluid's block".into());
    }
    let result = |k: &str| i.results.get(k).and_then(Value::as_str);
    let src = result("source").ok_or("no block for a source")?;
    let flows: Vec<Option<&str>> = ["1", "2", "3", "4", "5", "6", "7", "falling"]
        .iter()
        .map(|k| result(k))
        .collect();
    let flow = flows[0].ok_or("no block for a flowing fluid")?;
    if flows.iter().any(|f| *f != Some(flow)) {
        return Err("the block it leaves differs by the flowing fluid's amount".into());
    }
    if src.contains('[') || flow.contains('[') {
        return Err(format!(
            "it leaves a block state that isn't its block's default ({src}, {flow})"
        ));
    }
    let mut rule = Map::new();
    if let Some(below) = &source.below {
        rule.insert("below".into(), json!(below));
    }
    if !source.fluids.is_empty() {
        rule.insert("fluids".into(), json!(source.fluids));
    }
    if !source.blocks.is_empty() {
        rule.insert("blocks".into(), json!(source.blocks));
    }
    rule.insert("source".into(), json!(src));
    rule.insert("flowing".into(), json!(flow));
    Ok(Value::Object(rule))
}

impl FluidProbe {
    /// `fluid-facts.json`, or none.
    pub fn read(path: &Path) -> Result<Option<FluidProbe>, String> {
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Its fluids, types, dimensions and cauldrons in place of `facts`' own.
    pub fn lay_over(self, facts: &mut crate::oracle::Facts) {
        facts.fluids.extend(self.fluids);
        facts.fluid_types.extend(self.fluid_types);
        if !self.ultrawarm.is_empty() {
            facts.ultrawarm = self.ultrawarm;
        }
        facts.cauldrons.extend(self.cauldrons);
    }
}

/// The declaration's `vaporizes` from what the oracle answered in each dimension: `always`, `never`, or
/// `ultrawarm` when it vaporizes exactly where water evaporates. None for an answer no rule names (Just Dire Things'
/// portal fluid asks the biome), and for facts without the question: the declaration keeps `FluidType`'s own rule.
fn vaporizes(ty: &FluidType, ultrawarm: &BTreeMap<String, bool>) -> Option<&'static str> {
    let asked = ty
        .vaporized_on_placement
        .as_ref()
        .filter(|a| !a.is_empty())?;
    if asked.values().all(|v| *v) {
        Some("always")
    } else if asked.values().all(|v| !*v) {
        Some("never")
    } else if asked.iter().all(|(dim, v)| ultrawarm.get(dim) == Some(v)) {
        Some("ultrawarm")
    } else {
        None
    }
}

/// What only the pack's client says of its fluids (`client-facts.json`).
#[derive(Debug, Default, Deserialize)]
pub struct ClientFacts {
    /// Each type's `IClientFluidTypeExtensions`.
    #[serde(default)]
    pub fluid_types: BTreeMap<String, ClientFluidType>,
    /// Each fluid's render layer.
    #[serde(default)]
    pub fluids: BTreeMap<String, ClientFluid>,
    /// Each bucket's item colours.
    #[serde(default)]
    pub items: BTreeMap<String, ClientItem>,
}

#[derive(Debug, Deserialize)]
pub struct ClientFluidType {
    /// `getStillTexture()`, `getFlowingTexture()`, `getOverlayTexture()`: sprite ids, none where Java has null.
    #[serde(default)]
    pub still: Option<String>,
    #[serde(default)]
    pub flowing: Option<String>,
    #[serde(default)]
    pub overlay: Option<String>,
    /// `getTintColor()`, ARGB as Java's int.
    pub tint: i32,
    /// `getRenderOverlayTexture(minecraft)`: the camera's overlay with the eyes in it (a texture's resource
    /// path), none where Java has null.
    #[serde(default)]
    pub render_overlay: Option<String>,
    /// `modifyFogColor` when it gives another colour than the fog's: RGB, 0 to 1.
    #[serde(default)]
    pub fog_color: Option<[f32; 3]>,
    /// `modifyFogRender` over the terrain at a 12-chunk render distance, when it sets the shader's fog: its start
    /// and end.
    #[serde(default)]
    pub fog_range: Option<[f32; 2]>,
}

#[derive(Debug, Deserialize)]
pub struct ClientFluid {
    /// `ItemBlockRenderTypes.getRenderLayer(defaultFluidState())`: `solid`, `translucent`...
    pub render_layer: String,
}

#[derive(Debug, Deserialize)]
pub struct ClientItem {
    /// `ItemColors.getColor(stack, i)` for tint indexes 0, 1 and 2: -1 where no colour handler tints.
    #[serde(default)]
    pub tints: Vec<i32>,
}

impl ClientFacts {
    /// `client-facts.json`, or none: without it no fluid is declared (a declaration wants its textures).
    pub fn read(path: &Path) -> Result<Option<ClientFacts>, String> {
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The item definition's tints of `item` (`ItemColors.getColor` by tint index, as 26.3's
    /// `minecraft:constant` sources), or none when no handler tints it.
    pub fn item_tints(&self, item: &str) -> Option<Vec<Value>> {
        let tints = &self.items.get(item)?.tints;
        let last = tints.iter().rposition(|t| *t != -1)?;
        Some(
            tints[..=last]
                .iter()
                .map(|t| json!({"type": "minecraft:constant", "value": t}))
                .collect(),
        )
    }
}

/// The sprite Java's atlas gives a null texture (`TextureAtlas.getSprite` of an unknown name).
const MISSING: &str = "minecraft:missingno";

/// The declaration of the source fluid `id` (`data/<ns>/horizon/fluid/<path>.json`) from the facts: its flowing
/// fluid, `LiquidBlock` and bucket, `BaseFlowingFluid`'s numbers (`getTickDelay`, `getSlopeFindDistance`,
/// `getDropOff`, `getExplosionResistance`, `canConvertToSource`), its type's (`FluidType.getLightLevel`,
/// `getDensity`, `getTemperature`, `getViscosity`, `getDescriptionId`, `getSound`), and its client extension's
/// textures and tint and its render layer. `bucket_declared` says whether the content declares an item: a bucket
/// it doesn't is left out of the declaration. An error says why the fluid can't be declared.
pub fn declaration(
    fluids: &BTreeMap<String, Fluid>,
    types: &BTreeMap<String, FluidType>,
    world: FluidWorld,
    client: &ClientFacts,
    id: &str,
    bucket_declared: &dyn Fn(&str) -> bool,
) -> Result<Value, String> {
    let fluid = fluids.get(id).ok_or("not in the facts")?;
    if !fluid.is_source {
        return Err("a flowing fluid: its source declares it".into());
    }
    // A fluid that is no `FlowingFluid` (Modern Industrialization's), or one whose flowing fluid is itself (Iron's
    // Spellbooks' inks and elixirs), has no flowing fluid, and no `LiquidBlock` to bind: its block (if any) is an
    // ordinary block of the carry.
    let flows = fluid.flowing.as_deref().is_some_and(|f| f != id);
    let flowing = if flows {
        fluid.flowing.as_deref().unwrap_or("")
    } else {
        ""
    };
    let (ns, _) = id.split_once(':').ok_or("not a namespaced id")?;
    if flows && flowing.split_once(':').map(|(n, _)| n) != Some(ns) {
        return Err(format!(
            "its flowing fluid {flowing} is not another of its namespace"
        ));
    }
    let type_id = fluid.fluid_type.as_deref().ok_or("no fluid type")?;
    let ty = types
        .get(type_id)
        .ok_or_else(|| format!("its type {type_id} is not in the facts"))?;
    let ext = client
        .fluid_types
        .get(type_id)
        .ok_or_else(|| format!("its type {type_id} is not in the client's facts"))?;
    if !(0..=15).contains(&ty.light_level) {
        return Err(format!("light level {} is not 0 to 15", ty.light_level));
    }
    let translucent = client
        .fluids
        .get(id)
        .is_some_and(|f| f.render_layer == "translucent");
    let texture = |t: &Option<String>| t.clone().unwrap_or_else(|| MISSING.to_owned());
    let mut v = Map::new();
    if flows {
        v.insert("flowing".into(), json!(flowing));
        v.insert("block".into(), json!(fluid.block));
    } else {
        v.insert("flows".into(), json!(false));
        v.insert("block".into(), json!("minecraft:air"));
    }
    if fluid.bucket != "minecraft:air" && bucket_declared(&fluid.bucket) {
        v.insert("bucket".into(), json!(fluid.bucket));
    }
    v.insert("still_texture".into(), json!(texture(&ext.still)));
    v.insert("flowing_texture".into(), json!(texture(&ext.flowing)));
    if let Some(overlay) = &ext.overlay {
        v.insert("overlay_texture".into(), json!(overlay));
    }
    v.insert("tint".into(), json!(format!("0x{:08X}", ext.tint as u32)));
    v.insert("translucent".into(), json!(translucent));
    if flows {
        let slope = fluid.slope_find_distance.ok_or("no slope find distance")?;
        let drop_off = fluid.drop_off.ok_or("no drop-off")?;
        // What the engine's flow port can run (a declaration it refuses would refuse the whole mod).
        if fluid.tick_delay < 1
            || drop_off < 1
            || slope < 0
            || !fluid.explosion_resistance.is_finite()
        {
            return Err(format!(
                "tick delay {}, drop-off {drop_off}, slope distance {slope}: not numbers the flow takes",
                fluid.tick_delay
            ));
        }
        v.insert("tick_rate".into(), json!(fluid.tick_delay));
        v.insert("slope_find_distance".into(), json!(slope));
        v.insert("level_decrease_per_block".into(), json!(drop_off));
        v.insert(
            "explosion_resistance".into(),
            json!(fluid.explosion_resistance),
        );
    }
    v.insert(
        "can_convert_to_source".into(),
        json!(fluid.can_convert_to_source.unwrap_or(false)),
    );
    v.insert("light_level".into(), json!(ty.light_level));
    v.insert("density".into(), json!(ty.density));
    v.insert("temperature".into(), json!(ty.temperature));
    v.insert("viscosity".into(), json!(ty.viscosity));
    let mut sounds = Map::new();
    for action in [
        "bucket_fill",
        "bucket_empty",
        "fluid_vaporize",
        "cauldron_drip",
    ] {
        if let Some(sound) = ty.sounds.get(action) {
            sounds.insert(action.into(), json!(sound));
        }
    }
    if !sounds.is_empty() {
        v.insert("sounds".into(), Value::Object(sounds));
    }
    v.insert("description".into(), json!(ty.description));
    // FluidType's say over entities (slice 4), and its client extension's camera overlay and fog.
    if let Some(entity) = &ty.entity {
        let known = [
            "can_swim",
            "can_drown",
            "can_push",
            "motion_scale",
            "fall_distance_modifier",
            "can_extinguish",
            "can_hydrate",
            "supports_boating",
        ];
        let kept: Map<String, Value> = entity
            .iter()
            .filter(|(k, _)| known.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        v.insert("entity".into(), Value::Object(kept));
    }
    if let Some(overlay) = &ext.render_overlay {
        v.insert("render_overlay".into(), json!(overlay));
    }
    if let Some(color) = ext.fog_color {
        v.insert("fog_color".into(), json!(color));
    }
    if let Some(range) = ext.fog_range {
        v.insert("fog_range".into(), json!(range));
    }
    // Slice 7b: its type's dripstone hook, and a dropped item's movement in it when it isn't NeoForge's default.
    // (Its `canHydrate` and `supportsBoating` asked of a fluid state are the entity facts' for every type of ATM10
    // 8.4: the run's `hydrates` and `boating`.)
    if let Some(drip) = &ty.drip {
        v.insert("drip".into(), Value::Object(drip.clone()));
    }
    if let Some(m) = &ty.item_movement
        && ["horizontal", "below", "above"]
            .iter()
            .all(|k| m.get(*k).is_some_and(Value::is_f64))
    {
        let default = horizon_mods::content::FluidItemMovement::default();
        let n = |k: &str| m[k].as_f64().unwrap_or(0.0);
        if (n("horizontal"), n("below"), n("above"))
            != (default.horizontal, default.below, default.above)
        {
            v.insert(
                "item_movement".into(),
                json!({"horizontal": n("horizontal"), "below": n("below"), "above": n("above")}),
            );
        }
    }
    // Slice 7: where its bucket vaporizes, and the cauldron of its namespace that holds it.
    if let Some(rule) = vaporizes(ty, world.ultrawarm) {
        v.insert("vaporizes".into(), json!(rule));
    }
    let own = |b: &str| b.split_once(':').map(|(n, _)| n) == Some(ns);
    if let Some((block, c)) = world
        .cauldrons
        .iter()
        .find(|(b, c)| c.fluid == id && own(b))
    {
        let mut cauldron = Map::new();
        cauldron.insert("block".into(), json!(block));
        cauldron.insert("total_amount".into(), json!(c.total_amount));
        cauldron.insert("max_level".into(), json!(c.max_level));
        if let Some(p) = &c.level_property {
            cauldron.insert("level_property".into(), json!(p));
        }
        v.insert("cauldron".into(), Value::Object(cauldron));
    }
    Ok(Value::Object(v))
}

/// The liquid blocks of the namespace `ns`'s source fluids: blocks a carry keeps although its jar has no
/// blockstate for them (`LiquidBlock`'s render shape is invisible: the fluid renderer draws them).
pub fn liquid_blocks<'a>(fluids: &'a BTreeMap<String, Fluid>, ns: &str) -> Vec<&'a str> {
    let prefix = format!("{ns}:");
    fluids
        .range(prefix.clone()..)
        .take_while(|(id, _)| id.starts_with(&prefix))
        .filter(|(_, f)| f.is_source && f.block.starts_with(&prefix))
        .map(|(_, f)| f.block.as_str())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Facts as the two oracle runs write them, for Mekanism's heavy water (Mekanism 10.7.19's `MekanismFluids`:
    /// water's textures, tint 0xFF0D1455, translucent) and Industrial Foregoing's latex (Titanium's fluid: its own
    /// textures, `latex_flowing`, no sounds, solid).
    fn facts() -> (
        BTreeMap<String, Fluid>,
        BTreeMap<String, FluidType>,
        ClientFacts,
    ) {
        let fluids = serde_json::from_str(
            r#"{
            "mekanism:heavy_water": {"type": "mekanism:heavy_water", "is_source": true,
                "source": "mekanism:heavy_water", "flowing": "mekanism:flowing_heavy_water",
                "slope_find_distance": 4, "drop_off": 1, "can_convert_to_source": false,
                "block": "mekanism:heavy_water", "bucket": "mekanism:heavy_water_bucket", "tick_delay": 5,
                "explosion_resistance": 1.0, "pickup_sound": "minecraft:item.bucket.fill"},
            "mekanism:flowing_heavy_water": {"type": "mekanism:heavy_water", "is_source": false,
                "source": "mekanism:heavy_water", "flowing": "mekanism:flowing_heavy_water",
                "slope_find_distance": 4, "drop_off": 1, "block": "mekanism:heavy_water",
                "bucket": "mekanism:heavy_water_bucket", "tick_delay": 5, "explosion_resistance": 1.0},
            "industrialforegoing:latex": {"type": "industrialforegoing:latex", "is_source": true,
                "source": "industrialforegoing:latex", "flowing": "industrialforegoing:latex_flowing",
                "slope_find_distance": 4, "drop_off": 1, "can_convert_to_source": false,
                "block": "industrialforegoing:latex", "bucket": "industrialforegoing:latex_bucket",
                "tick_delay": 5, "explosion_resistance": 1.0},
            "test:virtual": {"type": "test:virtual", "is_source": true, "block": "minecraft:air",
                "bucket": "minecraft:air", "tick_delay": 0, "explosion_resistance": 0.0},
            "test:ink": {"type": "test:virtual", "is_source": true, "source": "test:ink", "flowing": "test:ink",
                "slope_find_distance": 4, "drop_off": 1, "block": "minecraft:air", "bucket": "minecraft:air",
                "tick_delay": 5, "explosion_resistance": 1.0}
        }"#,
        )
        .unwrap();
        let types = serde_json::from_str(
            r#"{
            "mekanism:heavy_water": {"description": "block.mekanism.heavy_water", "light_level": 0,
                "density": 1000, "temperature": 300, "viscosity": 1000,
                "sounds": {"bucket_fill": "minecraft:item.bucket.fill", "bucket_empty": "minecraft:item.bucket.empty"},
                "entity": {"can_swim": true, "can_drown": true, "can_push": true, "motion_scale": 0.014,
                    "fall_distance_modifier": 0.5, "can_extinguish": false, "can_hydrate": false,
                    "supports_boating": false}},
            "industrialforegoing:latex": {"description": "fluid_type.industrialforegoing.latex", "light_level": 0,
                "density": 1000, "temperature": 300, "viscosity": 1000, "sounds": {},
                "entity": {"can_swim": false, "can_drown": false, "can_push": true, "motion_scale": 0.002335,
                    "fall_distance_modifier": 0.5, "can_extinguish": false, "can_hydrate": false,
                    "supports_boating": false}},
            "test:virtual": {"description": "fluid.test.virtual", "light_level": 0, "density": 1000,
                "temperature": 300, "viscosity": 1000}
        }"#,
        )
        .unwrap();
        let client = serde_json::from_str(
            r#"{
            "fluid_types": {
                "mekanism:heavy_water": {"still": "minecraft:block/water_still", "flowing": "minecraft:block/water_flow",
                    "overlay": "minecraft:block/water_overlay", "tint": -15920043,
                    "render_overlay": "minecraft:textures/misc/underwater.png",
                    "fog_color": [0.050980393, 0.078431375, 0.33333334], "fog_range": [-8.0, 24.0]},
                "industrialforegoing:latex": {"still": "industrialforegoing:block/fluids/latex_still",
                    "flowing": "industrialforegoing:block/fluids/latex_flow", "overlay": null, "tint": -1},
                "test:virtual": {"still": "test:block/virtual", "flowing": "test:block/virtual", "tint": -1}
            },
            "fluids": {
                "mekanism:heavy_water": {"render_layer": "translucent"},
                "industrialforegoing:latex": {"render_layer": "solid"}
            },
            "items": {
                "mekanism:heavy_water_bucket": {"tints": [-1, -15920043, -1]},
                "industrialforegoing:latex_bucket": {"tints": [-1, -1, -1]}
            }
        }"#,
        )
        .unwrap();
        (fluids, types, client)
    }

    #[test]
    fn a_declaration_holds_the_oracles_values() {
        let (fluids, types, client) = facts();
        let yes = |_: &str| true;
        let heavy = declaration(
            &fluids,
            &types,
            world(),
            &client,
            "mekanism:heavy_water",
            &yes,
        )
        .unwrap();
        // The file slice 1 wrote by hand from Mekanism's source, now from the two runs.
        let file: horizon_mods::content::CarriedFluid = serde_json::from_value(heavy.clone()).unwrap();
        assert_eq!(file.tint, 0xFF0D_1455);
        assert!(file.translucent);
        assert_eq!(file.flowing, "mekanism:flowing_heavy_water");
        assert_eq!(file.bucket.as_deref(), Some("mekanism:heavy_water_bucket"));
        assert_eq!(file.still_texture, "minecraft:block/water_still");
        assert_eq!(
            file.overlay_texture.as_deref(),
            Some("minecraft:block/water_overlay")
        );
        assert_eq!(
            file.sounds.bucket_empty.as_deref(),
            Some("minecraft:item.bucket.empty")
        );
        assert_eq!(file.description, "block.mekanism.heavy_water");
        // Its type's entity facts and its client extension's overlay and fog (Mekanism's: its tint, 24 blocks).
        assert_eq!(file.entity, horizon_mods::content::FluidEntity::default());
        assert_eq!(
            file.render_overlay.as_deref(),
            Some("minecraft:textures/misc/underwater.png")
        );
        assert_eq!(file.fog_color, Some([0.050980393, 0.078431375, 0.33333334]));
        assert_eq!(file.fog_range, Some([-8.0, 24.0]));
        // Titanium names its flowing fluid `<path>_flowing`, has no sounds and no overlay, and draws solid.
        let latex = declaration(
            &fluids,
            &types,
            world(),
            &client,
            "industrialforegoing:latex",
            &yes,
        )
        .unwrap();
        let file: horizon_mods::content::CarriedFluid = serde_json::from_value(latex).unwrap();
        assert_eq!(file.flowing, "industrialforegoing:latex_flowing");
        assert_eq!((file.tint, file.translucent), (0xFFFF_FFFF, false));
        assert_eq!(file.overlay_texture, None);
        assert_eq!(file.sounds, horizon_mods::content::FluidSounds::default());
        // A type that is no water to an entity (the facts are the oracle's, whatever they say).
        assert!(!file.entity.can_swim && !file.entity.can_drown);
        assert_eq!(file.entity.motion_scale, 0.002335);
        assert_eq!(
            (file.render_overlay, file.fog_color, file.fog_range),
            (None, None, None)
        );
        // A bucket the content doesn't declare is left out; a flowing fluid and one that doesn't flow aren't
        // declared.
        let no = |_: &str| false;
        let without = declaration(
            &fluids,
            &types,
            world(),
            &client,
            "mekanism:heavy_water",
            &no,
        )
        .unwrap();
        assert!(without.get("bucket").is_none());
        let flowing = declaration(
            &fluids,
            &types,
            world(),
            &client,
            "mekanism:flowing_heavy_water",
            &yes,
        );
        assert!(flowing.unwrap_err().contains("flowing fluid"));
        // A fluid that doesn't flow (Modern Industrialization's `MIFluid`): no flowing fluid, no liquid block.
        let virtual_ =
            declaration(&fluids, &types, world(), &client, "test:virtual", &yes).unwrap();
        let file: horizon_mods::content::CarriedFluid = serde_json::from_value(virtual_).unwrap();
        assert!(!file.flows);
        assert_eq!(
            (file.flowing.as_str(), file.block.as_str()),
            ("", "minecraft:air")
        );
        assert_eq!(file.still_texture, "test:block/virtual");
        // Nor one whose flowing fluid is itself (Iron's Spellbooks' inks).
        let ink = declaration(&fluids, &types, world(), &client, "test:ink", &yes).unwrap();
        assert_eq!(ink["flows"], json!(false));
    }

    #[test]
    fn buckets_take_their_item_colours_and_liquid_blocks_are_known() {
        let (fluids, _, client) = facts();
        let tints = client.item_tints("mekanism:heavy_water_bucket").unwrap();
        assert_eq!(
            tints,
            [
                json!({"type": "minecraft:constant", "value": -1}),
                json!({"type": "minecraft:constant", "value": -15920043})
            ]
        );
        assert_eq!(client.item_tints("industrialforegoing:latex_bucket"), None);
        assert_eq!(liquid_blocks(&fluids, "mekanism"), ["mekanism:heavy_water"]);
        assert!(liquid_blocks(&fluids, "test").is_empty());
    }

    /// Facts from before slice 7: no dimensions, no cauldrons.
    fn world() -> FluidWorld<'static> {
        static NONE: (BTreeMap<String, bool>, BTreeMap<String, Cauldron>) =
            (BTreeMap::new(), BTreeMap::new());
        FluidWorld {
            ultrawarm: &NONE.0,
            cauldrons: &NONE.1,
        }
    }

    #[test]
    fn the_runs_fluid_interactions_become_the_pack_mods_files() {
        // A run's interactions as `FluidInteractionFacts` writes them: lava's list with NeoForge's own two first,
        // Create's honey (a type of its own beside it: obsidian or limestone), the Aether's holystone over
        // quicksoil beside magma on water, an interaction of its own (Astral Sorcery's), one whose matches no rule
        // holds, and one leaving a block's other state.
        let grid = |below: &str, fluids: &str, blocks: &str, exact: bool| {
            format!(
                r#"{{"exact": {exact}, "below": {below}, "fluids": {fluids}, "blocks": {blocks}}}"#
            )
        };
        let neo = "net.neoforged.neoforge.fluids.FluidInteractionRegistry$InteractionInformation";
        let flowing = |b: &str| {
            (1..8)
                .map(|n| format!(r#""{n}": "{b}""#))
                .chain([format!(r#""falling": "{b}""#)])
                .collect::<Vec<_>>()
                .join(", ")
        };
        let water = grid(
            "null",
            r#"["minecraft:water", "minecraft:flowing_water"]"#,
            "[]",
            true,
        );
        let honey = grid(
            "null",
            r#"["create:honey", "create:flowing_honey"]"#,
            "[]",
            true,
        );
        let quicksoil = grid(
            r#"["aether:quicksoil"]"#,
            "[]",
            r#"["minecraft:magma_block"]"#,
            true,
        );
        let ragged = grid("null", r#"["minecraft:water"]"#, "[]", false);
        let json = format!(
            r#"{{
            "minecraft:lava": [
                {{"index": 0, "predicate": "{neo}", "interaction": "{neo}", "neoforge_interaction": true,
                  "current": "minecraft:lava", "source": {water}, "flowing": {water},
                  "results": {{"source": "minecraft:obsidian", {}}}}},
                {{"index": 1, "predicate": "net.neoforged.neoforge.fluids.FluidInteractionRegistry", "interaction": "{neo}",
                  "neoforge_interaction": true, "current": "minecraft:lava",
                  "source": {}, "flowing": {},
                  "results": {{"source": "minecraft:basalt[axis=y]", {}}}}},
                {{"index": 2, "predicate": "{neo}", "interaction": "{neo}", "neoforge_interaction": true,
                  "current": "minecraft:lava", "source": {honey}, "flowing": {honey},
                  "results": {{"source": "minecraft:obsidian", {}}}}},
                {{"index": 3, "predicate": "x.Y", "interaction": "{neo}", "neoforge_interaction": true,
                  "current": "minecraft:lava", "source": {ragged}, "flowing": {ragged}, "results": {{}}}}
            ],
            "minecraft:water": [
                {{"index": 0, "predicate": "com.aetherteam.aether.block.AetherBlocks", "interaction": "{neo}",
                  "neoforge_interaction": true, "current": "minecraft:water", "source": {quicksoil}, "flowing": {quicksoil},
                  "results": {{"source": "aether:holystone", {}}}}}
            ],
            "astralsorcery:liquid_starlight": [
                {{"index": 0, "predicate": "hellfirepvp.astralsorcery.common.lib.FluidsAS",
                  "interaction": "hellfirepvp.astralsorcery.common.lib.FluidsAS", "neoforge_interaction": false,
                  "current": "astralsorcery:liquid_starlight", "source": {water}, "flowing": {water}, "results": {{}}}},
                {{"index": 1, "predicate": "{neo}", "interaction": "{neo}", "neoforge_interaction": true,
                  "current": "astralsorcery:liquid_starlight", "source": {water}, "flowing": {water},
                  "results": {{"source": "minecraft:sand[snowy=true]", {}}}}}
            ]
        }}"#,
            flowing("minecraft:cobblestone"),
            grid(
                r#"["minecraft:soul_soil"]"#,
                "[]",
                r#"["minecraft:blue_ice"]"#,
                true
            ),
            grid(
                r#"["minecraft:soul_soil"]"#,
                "[]",
                r#"["minecraft:blue_ice"]"#,
                true
            ),
            flowing("minecraft:basalt[axis=y]"),
            flowing("create:limestone"),
            flowing("aether:holystone"),
            flowing("minecraft:sand"),
        );
        let interactions: BTreeMap<String, Vec<InteractionFacts>> =
            serde_json::from_str(&json).unwrap();
        let (files, left_out) = interaction_files(&interactions);
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            ["minecraft/lava", "minecraft/water"],
            "no file for a type whose interactions all left"
        );
        // NeoForge's own two are the engine's: honey is lava's first.
        assert_eq!(
            files[0].1,
            json!({"fluid": "minecraft:lava", "interactions": [{"fluids": ["create:honey", "create:flowing_honey"],
                "source": "minecraft:obsidian", "flowing": "create:limestone"}]})
        );
        assert_eq!(
            files[1].1["interactions"][0],
            json!({"below": ["aether:quicksoil"], "blocks": ["minecraft:magma_block"], "source": "aether:holystone",
                "flowing": "aether:holystone"})
        );
        let why: Vec<&str> = left_out.iter().map(String::as_str).collect();
        assert_eq!(why.len(), 3, "{why:?}");
        assert!(
            why[0].starts_with("astralsorcery:liquid_starlight#0: an interaction of its own"),
            "{why:?}"
        );
        assert!(
            why[1].starts_with("astralsorcery:liquid_starlight#1: it leaves a block state"),
            "{why:?}"
        );
        assert!(
            why[2].starts_with("minecraft:lava#3: its matches are no rule"),
            "{why:?}"
        );
        // What the files are is what a content mod declares.
        let file: horizon_mods::content::FluidInteractions =
            serde_json::from_value(files[1].1.clone()).unwrap();
        assert!(file.interactions[0].fizz);
    }

    #[test]
    fn a_fluid_probe_names_where_a_type_vaporizes_and_its_cauldron() {
        // ATM10 8.4's fluids-only run: three of its 29 dimensions, Mekanism's type (never), Create Dragons Plus'
        // dragon's breath (where water evaporates, with its own cauldron of four levels), Create Enchantment
        // Industry's experience (always, by its config) and Just Dire Things' portal fluid (by biome).
        let (mut fluids, mut types, mut client) = facts();
        let probe: FluidProbe = serde_json::from_str(
            r#"{
            "fluids": {"create_dragons_plus:dragon_breath": {"type": "create_dragons_plus:dragon_breath",
                "is_source": true, "source": "create_dragons_plus:dragon_breath",
                "flowing": "create_dragons_plus:flowing_dragon_breath", "slope_find_distance": 2, "drop_off": 2,
                "block": "create_dragons_plus:dragon_breath", "bucket": "create_dragons_plus:dragon_breath_bucket",
                "tick_delay": 30, "explosion_resistance": 100.0}},
            "fluid_types": {
                "mekanism:heavy_water": {"description": "block.mekanism.heavy_water", "light_level": 0,
                    "density": 1000, "temperature": 300, "viscosity": 1000,
                    "vaporized_on_placement": {"minecraft:overworld": false, "minecraft:the_nether": false,
                        "allthemodium:the_other": false}},
                "create_dragons_plus:dragon_breath": {"description": "fluid.create_dragons_plus.dragon_breath",
                    "light_level": 15, "density": 3000, "temperature": 300, "viscosity": 6000,
                    "sounds": {"fluid_vaporize": "minecraft:block.fire.extinguish"},
                    "vaporized_on_placement": {"minecraft:overworld": false, "minecraft:the_nether": true,
                        "allthemodium:the_other": true}},
                "test:experience": {"description": "x", "light_level": 0, "density": 1000, "temperature": 300,
                    "viscosity": 1000, "vaporized_on_placement": {"minecraft:overworld": true,
                    "minecraft:the_nether": true, "allthemodium:the_other": true}},
                "test:portal": {"description": "p", "light_level": 0, "density": 1000, "temperature": 300,
                    "viscosity": 1000, "vaporized_on_placement": {"minecraft:overworld": true,
                    "minecraft:the_nether": true, "allthemodium:the_other": false}}
            },
            "ultrawarm": {"minecraft:overworld": false, "minecraft:the_nether": true, "allthemodium:the_other": true},
            "cauldrons": {
                "minecraft:water_cauldron": {"fluid": "minecraft:water", "total_amount": 1000, "max_level": 3,
                    "level_property": "level"},
                "create_dragons_plus:dragon_breath_cauldron": {"fluid": "create_dragons_plus:dragon_breath",
                    "total_amount": 1000, "max_level": 4, "level_property": "level"}
            }
        }"#,
        )
        .unwrap();
        let mut facts = crate::oracle::Facts::default();
        facts.fluids = std::mem::take(&mut fluids);
        facts.fluid_types = std::mem::take(&mut types);
        probe.lay_over(&mut facts);
        client.fluid_types.insert(
            "create_dragons_plus:dragon_breath".into(),
            serde_json::from_str(
                r#"{"still": "create_dragons_plus:fluid/dragon_breath_still",
                "flowing": "create_dragons_plus:fluid/dragon_breath_flow", "tint": -1}"#,
            )
            .unwrap(),
        );
        let yes = |_: &str| true;
        let decl = |id: &str| {
            declaration(
                &facts.fluids,
                &facts.fluid_types,
                facts.fluid_world(),
                &client,
                id,
                &yes,
            )
            .unwrap()
        };
        let heavy = decl("mekanism:heavy_water");
        assert_eq!(heavy["vaporizes"], json!("never"));
        assert_eq!(
            heavy["sounds"],
            Value::Null,
            "the probe's type has no sounds: the batch's are replaced"
        );
        assert_eq!(heavy.get("cauldron"), None);
        let breath = decl("create_dragons_plus:dragon_breath");
        assert_eq!(breath["vaporizes"], json!("ultrawarm"));
        assert_eq!(
            breath["sounds"]["fluid_vaporize"],
            json!("minecraft:block.fire.extinguish")
        );
        assert_eq!(
            breath["cauldron"],
            json!({"block": "create_dragons_plus:dragon_breath_cauldron", "total_amount": 1000, "max_level": 4,
                "level_property": "level"})
        );
        let file: horizon_mods::content::CarriedFluid = serde_json::from_value(breath).unwrap();
        assert_eq!(
            file.vaporizes,
            horizon_mods::content::FluidVaporizes::Ultrawarm
        );
        assert_eq!(file.cauldron.unwrap().max_level, 4);
        let (exp, portal) = (
            &facts.fluid_types["test:experience"],
            &facts.fluid_types["test:portal"],
        );
        assert_eq!(vaporizes(exp, &facts.ultrawarm), Some("always"));
        assert_eq!(
            vaporizes(portal, &facts.ultrawarm),
            None,
            "a rule of its own: FluidType's stays"
        );
    }
}
