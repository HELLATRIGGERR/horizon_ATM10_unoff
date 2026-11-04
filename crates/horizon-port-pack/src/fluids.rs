//! A pack's fluids from the NeoForge oracle (docs/mod-fluids.md, slice 2): what the server's run writes of each fluid
//! and fluid type (`FluidFacts`, in each batch's facts) and what the client's run writes of them
//! (`ClientFactsExporter`, `client-facts.json`), and the content mod declaration
//! (`data/<ns>/horizon/fluid/<path>.json`, `horizon_mods::content::Fluid`) made from both: Java's own numbers,
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
    for action in ["bucket_fill", "bucket_empty"] {
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
        let heavy = declaration(&fluids, &types, &client, "mekanism:heavy_water", &yes).unwrap();
        // The file slice 1 wrote by hand from Mekanism's source, now from the two runs.
        let file: horizon_mods::content::Fluid = serde_json::from_value(heavy.clone()).unwrap();
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
        let latex =
            declaration(&fluids, &types, &client, "industrialforegoing:latex", &yes).unwrap();
        let file: horizon_mods::content::Fluid = serde_json::from_value(latex).unwrap();
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
        let without = declaration(&fluids, &types, &client, "mekanism:heavy_water", &no).unwrap();
        assert!(without.get("bucket").is_none());
        let flowing = declaration(
            &fluids,
            &types,
            &client,
            "mekanism:flowing_heavy_water",
            &yes,
        );
        assert!(flowing.unwrap_err().contains("flowing fluid"));
        // A fluid that doesn't flow (Modern Industrialization's `MIFluid`): no flowing fluid, no liquid block.
        let virtual_ = declaration(&fluids, &types, &client, "test:virtual", &yes).unwrap();
        let file: horizon_mods::content::Fluid = serde_json::from_value(virtual_).unwrap();
        assert!(!file.flows);
        assert_eq!(
            (file.flowing.as_str(), file.block.as_str()),
            ("", "minecraft:air")
        );
        assert_eq!(file.still_texture, "test:block/virtual");
        // Nor one whose flowing fluid is itself (Iron's Spellbooks' inks).
        let ink = declaration(&fluids, &types, &client, "test:ink", &yes).unwrap();
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
}
