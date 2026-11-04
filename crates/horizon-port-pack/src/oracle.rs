//! The NeoForge oracle's facts (`tools/neoforge_oracle.py export`, in
//! `<work>/oracle/facts.json`): every block, item, recipe and tag the pack's
//! mods register, read from the running game, and the vanilla blocks beside
//! them. Carry-over uses them in place of guesses: a block's template is the
//! vanilla block of the same class and states whose shapes and sounds match
//! best, and the declared properties are the mod's own numbers.
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::Path,
};

#[derive(Debug, Deserialize)]
pub struct Facts {
    pub blocks: BTreeMap<String, Block>,
    pub items: BTreeMap<String, Item>,
    #[serde(default)]
    pub tags: Tags,
    /// The game's items' default components (`items.json`), so item ports leave out what their template has.
    #[serde(skip)]
    pub vanilla_items: BTreeMap<String, Value>,
    /// What the whole pack's run says beyond these facts (`pack-facts.json`).
    #[serde(skip)]
    pub pack: PackFacts,
    /// The fluids and fluid types of the namespaces (`FluidFacts`); empty in facts from before they were exported.
    #[serde(default)]
    pub fluids: BTreeMap<String, crate::fluids::Fluid>,
    #[serde(default)]
    pub fluid_types: BTreeMap<String, crate::fluids::FluidType>,
    /// What the pack's client run says of its fluids (`client-facts.json`), or none.
    #[serde(skip)]
    pub client: Option<crate::fluids::ClientFacts>,
}

/// What `tools/neoforge_oracle.py pack` writes to `pack-facts.json`: what no vanilla table can say of an item or a
/// block in the pack, read from the running game with every data map and script applied.
#[derive(Debug, Default, Deserialize)]
pub struct PackFacts {
    #[serde(default)]
    pub items: HashMap<String, PackItem>,
    #[serde(default)]
    pub blocks: HashMap<String, PackBlock>,
}

#[derive(Debug, Default, Deserialize)]
pub struct PackItem {
    /// Ticks it burns for in a furnace; 0 when it is no fuel.
    #[serde(default)]
    pub burn: u32,
    /// The chance of a composter layer.
    #[serde(default)]
    pub compost: Option<f64>,
    /// The slot it is worn in (`Equipable`).
    #[serde(default)]
    pub worn: Option<String>,
    /// Why the game could not say.
    #[serde(default)]
    pub failed: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct PackBlock {
    /// Its default state's `PushReaction`.
    #[serde(default)]
    pub push: String,
    #[serde(default)]
    pub block_entity: bool,
    #[serde(default)]
    pub occludes: bool,
    #[serde(default)]
    pub dampening: u8,
    #[serde(default)]
    pub skylight_down: bool,
    #[serde(default)]
    pub failed: Option<String>,
}

impl PackFacts {
    /// `pack-facts.json`, or none: without it templates' fuel and compost stand.
    pub fn read(path: &Path) -> Result<PackFacts, String> {
        if !path.exists() {
            return Ok(PackFacts::default());
        }
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
    }
    fn item(&self, id: &str) -> Option<&PackItem> {
        self.items.get(id).filter(|i| i.failed.is_none())
    }
    fn block(&self, id: &str) -> Option<&PackBlock> {
        self.blocks.get(id).filter(|b| b.failed.is_none())
    }
}

impl PackBlock {
    /// The game's name for what a piston does with it. A piston moves no block entity, whatever its reaction says.
    fn push_reaction(&self) -> &'static str {
        if self.block_entity {
            return "IMMOVEABLE";
        }
        match self.push.as_str() {
            "DESTROY" => "POPPED",
            "BLOCK" => "IMMOVEABLE",
            "PUSH_ONLY" => "PUSH",
            "IGNORE" => "IGNORE_ENTITY",
            _ => "PUSH_PULL",
        }
    }
}

/// 26.3's named burn times (`data/minecraft/context_int_provider/cooking/`), which 1.21.1 has as plain ticks.
const BURN_TIMES: &[(&str, u32)] = &[
    ("minecraft:cooking/time_dry_plants", 100),
    ("minecraft:cooking/time_wool_slabs", 50),
    ("minecraft:cooking/time_bamboo", 50),
    ("minecraft:cooking/time_wool_carpets", 67),
    ("minecraft:cooking/time_wool", 100),
    ("minecraft:cooking/time_wood_items_extra_small", 100),
    ("minecraft:cooking/time_wood_slabs", 150),
    ("minecraft:cooking/time_wood_items_large", 200),
    ("minecraft:cooking/time_wood_blocks", 300),
    ("minecraft:cooking/time_wood_items_small", 300),
    ("minecraft:cooking/time_roots", 300),
    ("minecraft:cooking/time_hanging_signs", 800),
    ("minecraft:cooking/time_boats", 1200),
    ("minecraft:cooking/time_coal", 1600),
    ("minecraft:cooking/time_blaze_rod", 2400),
    ("minecraft:cooking/time_dried_kelp_block", 4001),
    ("minecraft:cooking/time_coal_block", 16000),
    ("minecraft:cooking/time_lava_bucket", 20000),
];

/// The `cooking_fuel` of an item burning for `ticks`: the template's when it already burns that long, else 26.3's
/// name for that time, else the same provider with the number in it.
fn cooking_fuel(ticks: u32, base: Option<&Value>) -> Value {
    if ticks == 0 {
        return Value::Null;
    }
    let had = base.and_then(|b| b.get("minecraft:cooking_fuel"));
    let had_ticks = had
        .and_then(|f| f["burn_time"].as_str())
        .and_then(|name| BURN_TIMES.iter().find(|(n, _)| *n == name))
        .map(|(_, t)| *t);
    if let Some(had) = had.filter(|_| had_ticks == Some(ticks)) {
        return had.clone();
    }
    let burn_time = match BURN_TIMES.iter().find(|(_, t)| *t == ticks) {
        Some((name, _)) => json!(name),
        None => json!({"type": "minecraft:div", "left": ticks, "right": {
            "type": "minecraft:conditional", "condition": "minecraft:block/fast_cooking",
            "on_false": "minecraft:cooking/normal_burn_time_reduction_factor",
            "on_true": "minecraft:cooking/fast_burn_time_reduction_factor"}}),
    };
    json!({"burn_time": burn_time, "speed_multiplier": "minecraft:cooking/speed_default"})
}

/// The `compostable` for a chance: 26.3's providers are vanilla's five, so the nearest of them.
fn compostable(chance: f64) -> Value {
    const LAYERS: [(&str, f64); 5] = [
        ("low", 0.3),
        ("low_medium", 0.5),
        ("medium", 0.65),
        ("medium_high", 0.85),
        ("always_add_one", 1.0),
    ];
    let nearest = LAYERS
        .iter()
        .min_by(|a, b| (a.1 - chance).abs().total_cmp(&(b.1 - chance).abs()))
        .map_or("low", |l| l.0);
    json!({"layers": format!("minecraft:compostable/{nearest}")})
}

#[derive(Debug, Default, Deserialize)]
pub struct Tags {
    #[serde(default)]
    pub block: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub item: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub fluid: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct Block {
    /// The class and its superclasses, most derived first.
    pub class: Vec<String>,
    pub properties: BTreeMap<String, Vec<String>>,
    pub default: String,
    pub item: String,
    pub explosion_resistance: f32,
    pub friction: f32,
    pub speed_factor: f32,
    pub jump_factor: f32,
    #[serde(default)]
    pub block_set_type: Option<SetType>,
    #[serde(default)]
    pub wood_type: Option<String>,
    pub states: BTreeMap<String, State>,
    /// Where its item put it with no player (`up`: on a floor clicked from above, `north`: on a wall clicked from
    /// the south): a state key, or none when placing needs a player.
    #[serde(default)]
    pub placement: BTreeMap<String, Option<String>>,
}

#[derive(Debug, Deserialize)]
pub struct SetType {
    pub name: String,
    pub can_open_by_hand: bool,
}

#[derive(Debug, Deserialize)]
pub struct State {
    pub hardness: f32,
    pub emission: u8,
    pub requires_tool: bool,
    pub map_color_id: u8,
    pub sound: Value,
    pub outline: Value,
    pub collision: Value,
    /// What breaking the state drops, per tool (`hand`, `diamond_pickaxe`,
    /// `shears`, `silk_touch_pickaxe`...): item id → count.
    #[serde(default)]
    pub drops: BTreeMap<String, BTreeMap<String, u32>>,
}

#[derive(Debug, Deserialize)]
pub struct Item {
    pub class: String,
    pub max_stack_size: u8,
    pub rarity: String,
    #[serde(default)]
    pub block: Option<String>,
    #[serde(default)]
    pub food: Option<Value>,
    /// The default data components in 1.21.1's form; empty in facts from before they were exported.
    #[serde(default)]
    pub components: Map<String, Value>,
    #[serde(default)]
    pub attribute_modifiers: Option<Value>,
    #[serde(default)]
    pub enchantment_value: Option<u32>,
    #[serde(default)]
    pub repair_items: Option<Vec<String>>,
    #[serde(default)]
    pub armor: Option<Armor>,
}

#[derive(Debug, Deserialize)]
pub struct Armor {
    /// `head`, `chest`, `legs` or `feet`.
    pub slot: String,
    /// The armour material's id: the 26.3 equipment asset it becomes.
    #[serde(default)]
    pub material: String,
    #[serde(default)]
    pub equip_sound: Option<String>,
    #[serde(default)]
    pub layers: Vec<ArmorLayer>,
}

/// A 1.21.1 armour layer's textures: `<ns>:textures/models/armor/<name>_layer_1.png` and `_layer_2`.
#[derive(Debug, Deserialize)]
pub struct ArmorLayer {
    pub texture: String,
    pub texture_legs: String,
    #[serde(default)]
    pub dyeable: bool,
}

impl Armor {
    /// Whether carry writes the material's equipment asset: in the mod's own namespaces, from layers it can place.
    pub fn asset_carried(&self, owned: &[&str]) -> bool {
        self.material
            .split_once(':')
            .is_some_and(|(ns, _)| owned.contains(&ns))
            && self.equipment().is_some()
    }

    /// The 26.3 equipment asset of the material (`layers` by kind), and the 1.21.1 textures to copy, as (from, to) paths in the mod.
    pub fn equipment(&self) -> Option<(Value, Vec<(String, String)>)> {
        self.material.split_once(':')?;
        let mut copies = Vec::new();
        let mut kinds = Map::new();
        for (kind, legs) in [("humanoid", false), ("humanoid_leggings", true)] {
            let mut list = Vec::new();
            for layer in &self.layers {
                let texture = if legs {
                    &layer.texture_legs
                } else {
                    &layer.texture
                };
                let (ns, path) = texture.split_once(':')?;
                let file = path.strip_prefix("textures/models/armor/")?;
                let name = file.strip_suffix(if legs { "_layer_2.png" } else { "_layer_1.png" })?;
                copies.push((
                    format!("assets/{ns}/{path}"),
                    format!("assets/{ns}/textures/entity/equipment/{kind}/{name}.png"),
                ));
                let mut entry = json!({"texture": format!("{ns}:{name}")});
                if layer.dyeable {
                    entry["dyeable"] = json!({});
                }
                list.push(entry);
            }
            kinds.insert(kind.into(), Value::Array(list));
        }
        Some((json!({"layers": kinds}), copies))
    }
}

/// What differs from the template's item, as a data component patch: Null in `want` means it has none.
fn component_patch(want: Map<String, Value>, base: Option<&Value>) -> Map<String, Value> {
    let mut patch = Map::new();
    for (key, value) in want {
        let had = base.and_then(|b| b.get(&key));
        match (value, had) {
            (Value::Null, None) => {}
            (Value::Null, Some(_)) => _ = patch.insert(format!("!{key}"), json!({})),
            (v, Some(h)) if same(&v, h) => {}
            (v, _) => _ = patch.insert(key, v),
        }
    }
    patch
}

/// The vanilla item standing in for an item of a vanilla class, for the behaviour the class gives.
fn item_template(item: &Item) -> &'static str {
    let class = item.class.strip_prefix("net.minecraft.world.item.");
    match (class, item.armor.as_ref().map(|a| a.slot.as_str())) {
        (Some("SwordItem"), _) => "minecraft:iron_sword",
        (Some("PickaxeItem"), _) => "minecraft:iron_pickaxe",
        (Some("AxeItem"), _) => "minecraft:iron_axe",
        (Some("ShovelItem"), _) => "minecraft:iron_shovel",
        (Some("HoeItem"), _) => "minecraft:iron_hoe",
        (Some("ArmorItem"), Some("head")) => "minecraft:iron_helmet",
        (Some("ArmorItem"), Some("chest")) => "minecraft:iron_chestplate",
        (Some("ArmorItem"), Some("legs")) => "minecraft:iron_leggings",
        (Some("ArmorItem"), Some("feet")) => "minecraft:iron_boots",
        _ if item.food.is_some() => FOOD_TEMPLATE,
        _ => "minecraft:stick",
    }
}

/// The vanilla food standing in for a mod's food: nothing but `food` and an empty `consumable`, so the mod's
/// numbers, eating time, effects and leftover are the whole patch.
const FOOD_TEMPLATE: &str = "minecraft:cooked_beef";

/// 1.21.1's `food` component as 26.3's `food`, `consumable` (over `consumable`, the template's) and
/// `use_remainder` (null for none), and the effects left out: only effects in vanilla's namespace are kept,
/// since the game runs only effects it has.
fn food_26_3(food: &Value, consumable: Option<&Value>) -> (Value, Value, Value, Vec<String>) {
    let mut food_26 = json!({
        "nutrition": food["nutrition"].as_u64().unwrap_or(0),
        "saturation": food["saturation"].as_f64().unwrap_or(0.0),
    });
    if food["can_always_eat"].as_bool() == Some(true) {
        food_26["can_always_eat"] = json!(true);
    }
    let mut consumable = consumable.cloned().unwrap_or_else(|| json!({}));
    if let Some(seconds) = food["eat_seconds"].as_f64().filter(|s| (s - 1.6).abs() > 1e-6) {
        consumable["consume_seconds"] = json!(seconds);
    }
    let (mut effects, mut dropped) = (Vec::new(), Vec::new());
    for entry in food["effects"].as_array().into_iter().flatten() {
        let effect = &entry["effect"];
        let id = effect["id"].as_str().unwrap_or_default();
        if !id.starts_with("minecraft:") {
            dropped.push(format!("eating gives {id}, which is not carried"));
            continue;
        }
        let mut instance = json!({"id": id});
        for key in ["amplifier", "duration", "ambient", "show_particles", "show_icon"] {
            if let Some(v) = effect.get(key) {
                instance[key] = v.clone();
            }
        }
        let mut apply = json!({"type": "minecraft:apply_effects", "effects": [instance]});
        if let Some(p) = entry["probability"].as_f64().filter(|p| *p < 1.0) {
            apply["probability"] = json!(p);
        }
        effects.push(apply);
    }
    if !effects.is_empty() {
        consumable["on_consume_effects"] = Value::Array(effects);
    }
    let remainder = food.get("using_converts_to").cloned().unwrap_or(Value::Null);
    (food_26, consumable, remainder, dropped)
}

/// 1.21.1's `{"modifiers": [...]}` as 26.3's list, with attribute ids' `generic.` and `player.` dropped.
fn modifiers_26_3(v: &Value) -> Value {
    let list = v["modifiers"].as_array().cloned().unwrap_or_default();
    Value::Array(
        list.into_iter()
            .map(|mut m| {
                if let Some(t) = m["type"].as_str() {
                    let t = t.replacen(":generic.", ":", 1).replacen(":player.", ":", 1);
                    m["type"] = json!(t);
                }
                m
            })
            .collect(),
    )
}

const VANILLA_PACKAGE: &str = "net.minecraft.";

/// A state key with its properties sorted, so keys compare across blocks.
/// Which states of a block drop the block itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelfDrop {
    /// Every state: `dropSelf`.
    Always,
    /// Only the states where this property has this value.
    When(String, String),
}

/// A state key's `property=value` pairs.
fn pairs(key: &str) -> BTreeSet<(String, String)> {
    key.split(',')
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

pub fn state_key(key: &str) -> String {
    let mut parts: Vec<&str> = key.split(',').filter(|p| !p.is_empty()).collect();
    parts.sort_unstable();
    parts.join(",")
}

impl Block {
    /// The first vanilla class in the chain.
    pub fn vanilla_class(&self) -> &str {
        self.class
            .iter()
            .find(|c| c.starts_with(VANILLA_PACKAGE))
            .map_or("", String::as_str)
    }
    /// Its own class is a mod class: behaviour code the template may not have.
    pub fn custom_class(&self) -> bool {
        self.class
            .first()
            .is_some_and(|c| !c.starts_with(VANILLA_PACKAGE))
    }
    /// Every state drops one `item` (the block's own) with every tool, except
    /// where the state needs a tool and gets none: vanilla's `dropSelf`. Or
    /// only the states where one property has one value do, and the others
    /// drop nothing with any tool: a door's lower half (`createDoorTable`).
    /// Mods often register these tables in code, so their jar has no file.
    pub fn drops_itself(&self, item: &str) -> Option<SelfDrop> {
        let one = BTreeMap::from([(item.to_owned(), 1)]);
        let (mut dropping, mut nothing) = (Vec::new(), Vec::new());
        for (key, state) in &self.states {
            if state.drops.is_empty() {
                continue;
            }
            let mut drops = false;
            let mut short = false;
            for got in state.drops.values() {
                if *got == one {
                    drops = true;
                } else if got.is_empty() {
                    short = true;
                } else {
                    return None;
                }
            }
            match (drops, short) {
                (true, true) if !state.requires_tool => return None,
                (true, _) => dropping.push(pairs(key)),
                (false, _) => nothing.push(pairs(key)),
            }
        }
        let (first, rest) = dropping.split_first()?;
        if nothing.is_empty() {
            return Some(SelfDrop::Always);
        }
        first
            .iter()
            .filter(|pair| rest.iter().all(|s| s.contains(*pair)))
            .find(|pair| nothing.iter().all(|s| !s.contains(*pair)))
            .map(|(p, v)| SelfDrop::When(p.clone(), v.clone()))
    }
    fn default_state(&self) -> Option<&State> {
        self.states
            .get(&self.default)
            .or_else(|| self.states.values().next())
    }
    fn same_states(&self, other: &Block) -> bool {
        self.properties.len() == other.properties.len()
            && self.properties.iter().all(|(k, v)| {
                other.properties.get(k).is_some_and(|w| {
                    v.iter().collect::<BTreeSet<_>>() == w.iter().collect::<BTreeSet<_>>()
                })
            })
    }
    fn by_key(&self) -> BTreeMap<String, &State> {
        self.states.iter().map(|(k, s)| (state_key(k), s)).collect()
    }
    /// Every property of this block is one of `other`'s, with the same values.
    fn properties_within(&self, other: &Block) -> bool {
        self.properties.iter().all(|(k, v)| {
            other.properties.get(k).is_some_and(|w| {
                v.iter().collect::<BTreeSet<_>>() == w.iter().collect::<BTreeSet<_>>()
            })
        })
    }
    /// The `property=value` pairs of its default state.
    fn default_pairs(&self) -> BTreeMap<String, String> {
        self.default
            .split(',')
            .filter_map(|p| p.split_once('='))
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect()
    }
}

/// The template carry-over gives a mod block.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub template: String,
    /// `data/<mod>/horizon/block/<name>.json`'s `properties`.
    pub properties: Map<String, Value>,
    /// What the template does not reproduce, for the report.
    pub differences: Vec<String>,
    /// The declaration's other keys: `state_properties`, their `state_defaults`, the `default_state` on the
    /// template's own, and the shapes and light of the block's states where the template's do not stand in.
    pub declared: Map<String, Value>,
    /// How many states the block has in the game: the template's times the declared combinations.
    pub states: usize,
}

/// Equal JSON, numbers compared as f64 (the oracle writes `6.0` where the game's table has `6`).
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| same(x, y))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w)))
        }
        _ => a == b,
    }
}

/// A block's first vanilla class that says nothing of how it behaves: a template of a class below it would bring
/// behaviour the block may not have (a loom's menu under `HorizontalDirectionalBlock`).
const GENERAL_CLASSES: &[&str] = &[
    "Block",
    "BaseEntityBlock",
    "HorizontalDirectionalBlock",
    "DirectionalBlock",
    "FaceAttachedHorizontalDirectionalBlock",
    "TransparentBlock",
    "HalfTransparentBlock",
    "FallingBlock",
    "RotatedPillarBlock",
];

/// Templates whose behaviour any block can have: placed by their properties, and no menu or ticking. The ones not
/// a full cube are for blocks of about their shape: a pressure plate falls off without a floor. The redstone lamp is
/// for what calls itself a lamp ([`behaviour_fits`]).
const PLAIN: &[&str] = &[
    "minecraft:stone",
    "minecraft:carved_pumpkin",
    "minecraft:oak_log",
    "minecraft:redstone_lamp",
    "minecraft:oak_slab",
    "minecraft:oak_stairs",
    "minecraft:oak_door",
    "minecraft:oak_trapdoor",
    "minecraft:oak_fence",
    "minecraft:oak_fence_gate",
    "minecraft:glass_pane",
    "minecraft:cobblestone_wall",
    "minecraft:oak_button",
    "minecraft:oak_pressure_plate",
    "minecraft:chain",
    "minecraft:end_rod",
];

/// The most entries a selector map gets: the game tries each per state.
const MAX_SELECTORS: usize = 128;

/// 1 when `template` is the activator rail and the block's name doesn't say it is one: 1.21.1 builds the activator
/// rail from `PoweredRailBlock` too, so a mod's booster rail ties between the two, and it boosts.
fn rail_kind(id: &str, template: &str) -> i64 {
    (template == "minecraft:activator_rail" && !id.contains("activator")) as i64
}

/// A box the game's collision can't meet, for a state with no shape among states with one: under `COLLISION_EPSILON`
/// (1e-7) on every axis, in a corner. `collision_boxes` and `outline_boxes` hold boxes, and refuse a flat one.
const NOTHING: [f64; 6] = [0.0, 0.0, 0.0, 1e-8, 1e-8, 1e-8];

fn full_cube(outline: &Value) -> bool {
    same(outline, &json!([[0, 0, 0, 1, 1, 1]]))
}

/// A shape's bounding box, inside the block space and not flat, as `collision_boxes` needs it.
fn bounding_box(shape: &Value) -> Option<[f64; 6]> {
    let mut out: Option<[f64; 6]> = None;
    for b in shape.as_array()? {
        let n: Vec<f64> = b.as_array()?.iter().filter_map(Value::as_f64).collect();
        let [a, c, e, x, y, z] = n[..] else {
            return None;
        };
        let o = out.get_or_insert([a, c, e, x, y, z]);
        for (i, v) in [a, c, e].into_iter().enumerate() {
            o[i] = o[i].min(v);
        }
        for (i, v) in [x, y, z].into_iter().enumerate() {
            o[i + 3] = o[i + 3].max(v);
        }
    }
    let mut o = out?;
    for v in &mut o {
        *v = v.clamp(0.0, 1.0);
    }
    (0..3).all(|i| o[i] < o[i + 3]).then_some(o)
}

/// How many of a block's states have the shapes of `t`'s state with the same values of `t`'s properties.
fn shapes_fitting(block: &Block, t: &Block) -> usize {
    let theirs = t.by_key();
    block
        .states
        .iter()
        .filter(|(key, s)| {
            let mut parts: Vec<&str> = key
                .split(',')
                .filter(|p| p.split_once('=').is_some_and(|(k, _)| t.properties.contains_key(k)))
                .collect();
            parts.sort_unstable();
            theirs
                .get(&parts.join(","))
                .is_some_and(|v| same(&v.outline, &s.outline) && same(&v.collision, &s.collision))
        })
        .count()
}

/// One of a block's states as `property=value` pairs.
type LiveState<'a> = (BTreeMap<&'a str, &'a str>, &'a State);

/// The states a block can be in here: a property neither its template has nor it declares stays at its default.
fn live_states<'a>(block: &'a Block, t: &Block, names: &[&str]) -> Vec<LiveState<'a>> {
    let default = block.default_pairs();
    block
        .states
        .iter()
        .filter_map(|(key, s)| {
            let pairs: BTreeMap<&str, &str> = key.split(',').filter_map(|p| p.split_once('=')).collect();
            pairs
                .iter()
                .all(|(k, v)| {
                    t.properties.contains_key(*k)
                        || names.contains(k)
                        || default.get(*k).is_some_and(|d| d == v)
                })
                .then_some((pairs, s))
        })
        .collect()
}

/// `t`'s state with the same values of its properties.
fn state_of<'a>(
    pairs: &BTreeMap<&str, &str>,
    t: &Block,
    theirs: &BTreeMap<String, &'a State>,
) -> Option<&'a State> {
    let key = pairs
        .iter()
        .filter(|(k, _)| t.properties.contains_key(**k))
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(",");
    theirs.get(&key).copied()
}

/// Whether a block should have the behaviour a template brings when its class doesn't say so. Redstone switches a
/// lamp on and off and a furnace has a menu and burns things: that fits what calls itself one, not every block with
/// `lit`. A lamp also has to start off, as placing one does, in the template's light: selectors can't name `lit` on it.
fn behaviour_fits(template: &str, id: &str, block: &Block, t: &Block) -> bool {
    let says = |word: &str| {
        id.split(':').nth(1).is_some_and(|path| path.contains(word))
            || block
                .class
                .iter()
                .filter(|c| !c.starts_with(VANILLA_PACKAGE))
                .filter_map(|c| c.rsplit('.').next())
                .any(|class| class.to_ascii_lowercase().contains(word))
    };
    match template {
        "minecraft:redstone_lamp" => {
            let off = block.default_pairs().get("lit").is_some_and(|l| l == "false");
            let theirs = t.by_key();
            let same_light = block.states.iter().all(|(key, s)| {
                let pairs = key.split(',').filter_map(|p| p.split_once('=')).collect();
                state_of(&pairs, t, &theirs).is_some_and(|v| v.emission == s.emission)
            });
            says("lamp") && off && same_light
        }
        "minecraft:furnace" => says("furnace"),
        _ => true,
    }
}

/// The fewest of `names` a value of each state depends on, as a selector map: the commonest value under `*`, and
/// a selector for each combination with another. `None` when they don't determine it, or it takes too many.
fn selectors<V: Clone + Ord>(
    live: &[LiveState],
    names: &[&str],
    value: impl Fn(&State) -> V,
) -> Option<BTreeMap<String, V>> {
    let values: Vec<V> = live.iter().map(|(_, s)| value(s)).collect();
    let mut counts: BTreeMap<&V, usize> = BTreeMap::new();
    for v in &values {
        *counts.entry(v).or_default() += 1;
    }
    let common = counts.iter().max_by_key(|(_, n)| **n).map(|(v, _)| (*v).clone())?;
    if counts.len() == 1 {
        return Some(BTreeMap::from([("*".to_owned(), common)]));
    }
    let size = names.len();
    let mut masks: Vec<u32> = (1..1u32.checked_shl(size as u32).unwrap_or(0)).collect();
    masks.retain(|m| m.count_ones() <= 3 || m.count_ones() as usize == size);
    masks.sort_by_key(|m| m.count_ones());
    'subset: for mask in masks {
        let picked: Vec<&str> =
            names.iter().enumerate().filter(|(i, _)| mask >> i & 1 == 1).map(|(_, n)| *n).collect();
        let mut map: BTreeMap<String, V> = BTreeMap::new();
        for ((pairs, _), v) in live.iter().zip(&values) {
            let selector = picked
                .iter()
                .map(|n| format!("{n}={}", pairs.get(n).copied().unwrap_or("")))
                .collect::<Vec<_>>()
                .join(",");
            match map.get(&selector) {
                Some(seen) if seen != v => continue 'subset,
                _ => _ = map.insert(selector, v.clone()),
            }
        }
        map.retain(|_, v| *v != common);
        if map.len() >= MAX_SELECTORS {
            return None;
        }
        map.insert("*".to_owned(), common);
        return Some(map);
    }
    None
}

/// `shapes`, the outline and the collision both by one of the template's properties, when they follow that alone.
fn shapes_by_template(live: &[LiveState], t: &Block) -> Option<Value> {
    t.properties.keys().find_map(|p| {
        let mut values: BTreeMap<&str, &Value> = BTreeMap::new();
        for (pairs, s) in live {
            let v = *pairs.get(p.as_str())?;
            if !same(&s.outline, &s.collision) || values.get(v).is_some_and(|o| !same(o, &s.outline)) {
                return None;
            }
            values.insert(v, &s.outline);
        }
        shapes_of(p, values)
    })
}

/// `shapes` when a block's shapes follow more than the template's properties: by the one of them that moves the
/// outline most, every other property at its default. A machine's facing, whatever it is doing.
fn shapes_at_defaults(block: &Block, live: &[LiveState], t: &Block) -> Option<Value> {
    let default = block.default_pairs();
    let mut best: Option<(usize, &str, BTreeMap<&str, &Value>)> = None;
    for p in t.properties.keys() {
        let mut values: BTreeMap<&str, &Value> = BTreeMap::new();
        for (pairs, s) in live {
            let others_default = pairs
                .iter()
                .all(|(k, v)| *k == p.as_str() || default.get(*k).is_some_and(|d| d == v));
            if let (true, Some(v)) = (others_default, pairs.get(p.as_str())) {
                values.insert(v, &s.outline);
            }
        }
        let distinct = values.values().map(|v| v.to_string()).collect::<BTreeSet<_>>().len();
        if distinct > 1 && best.as_ref().is_none_or(|(d, _, _)| distinct > *d) {
            best = Some((distinct, p, values));
        }
    }
    let (_, p, values) = best?;
    (values.len() == t.properties[p].len()).then(|| shapes_of(p, values)).flatten()
}

/// The `shapes` value: a property's values and their boxes, at most 16 of them as the game reads them.
fn shapes_of(property: &str, values: BTreeMap<&str, &Value>) -> Option<Value> {
    let values: Option<Map<String, Value>> = values
        .into_iter()
        .map(|(v, shape)| {
            let list = shape.as_array()?;
            if list.len() <= 16 {
                return Some((v.to_owned(), shape.clone()));
            }
            Some((v.to_owned(), json!([bounding_box(shape)?])))
        })
        .collect();
    Some(json!({"property": property, "values": values?}))
}

/// The game's placement rule (`placement` in a block file) a property of the block follows, from where Java's
/// `getStateForPlacement` put it with no player: then `UseOnContext.getHorizontalDirection` is north, so a block
/// facing away from the player faces south, one facing it north; the clicked face is up, then north. Only rules
/// whose values the property has, and only where the two placements tell the rule from the default state.
fn placement_rule(block: &Block, name: &str) -> Option<&'static str> {
    let values = block.properties.get(name)?;
    let default = block.default_pairs().remove(name)?;
    let at = |face: &str| -> Option<&str> {
        let key = block.placement.get(face)?.as_deref()?;
        key.split(',').filter_map(|p| p.split_once('=')).find(|(k, _)| *k == name).map(|(_, v)| v)
    };
    let (up, north) = (at("up")?, at("north")?);
    const HORIZONTAL: &[&str] = &["north", "east", "south", "west"];
    const DIRECTIONS: &[&str] = &["down", "up", "north", "south", "west", "east"];
    let (rule, needs): (&'static str, &[&str]) = match (up, north) {
        ("south", "south") => ("horizontal_look_opposite", HORIZONTAL),
        ("north", "north") => ("horizontal_look", HORIZONTAL),
        ("up", "north") => ("clicked_face", DIRECTIONS),
        ("y", "z") => ("clicked_axis", &["x", "y", "z"]),
        _ => return None,
    };
    let has_all = needs.iter().all(|v| values.iter().any(|w| w == v));
    (has_all && (up != default || north != default)).then_some(rule)
}

/// Puts `names` (the block's properties the template lacks) into the declaration with their values and defaults,
/// and what the template's states then get wrong: their shapes and light.
fn declare_states(block: &Block, t: &Block, names: &[&str], pack: Option<&PackBlock>, choice: &mut Choice) {
    let default = block.default_pairs();
    let mut combos = 1usize;
    let mut declared = Map::new();
    let mut defaults = Map::new();
    for name in names {
        let values = &block.properties[*name];
        combos = combos.saturating_mul(values.len());
        // The list spelling and `state_defaults`: every version of the engine reads those.
        declared.insert((*name).to_owned(), json!(values));
        if let Some(v) = default.get(*name) {
            defaults.insert((*name).to_owned(), json!(v));
        }
    }
    choice.states = t.states.len().saturating_mul(combos);
    if !declared.is_empty() {
        choice.declared.insert("state_properties".into(), Value::Object(declared));
        if !defaults.is_empty() {
            choice.declared.insert("state_defaults".into(), Value::Object(defaults));
        }
    }
    // How a player's placement sets them, where Java's own placement shows it.
    let rules: Map<String, Value> = names
        .iter()
        .filter_map(|name| Some(((*name).to_owned(), json!(placement_rule(block, name)?))))
        .collect();
    if !rules.is_empty() {
        choice.declared.insert("placement".into(), Value::Object(rules));
    }
    let live = live_states(block, t, names);
    // Light by the declared properties: a machine's `lit`. By the template's it can only be the template's.
    let theirs = t.by_key();
    match selectors(&live, names, |s| s.emission) {
        Some(light) if light.len() > 1 => {
            choice.declared.insert("light_emission".into(), json!(light));
        }
        Some(light) => {
            choice.properties.insert("light".into(), json!(light.get("*").copied().unwrap_or(0)));
        }
        None if !live.iter().all(|(pairs, s)| {
            state_of(pairs, t, &theirs).is_some_and(|v| v.emission == s.emission)
        }) =>
        {
            choice
                .differences
                .push("its light follows the template's properties, at other levels than the template's".into());
        }
        None => {}
    }
    // Under water a state that lets light through takes one off (`getLightBlock`).
    let lets_light_through = pack.map(|p| p.dampening == 0);
    let under_water = |choice: &mut Choice| {
        if names.contains(&"waterlogged") {
            choice
                .declared
                .insert("light_dampening_states".into(), json!({"waterlogged=true": 1}));
        }
    };
    if lets_light_through == Some(true) {
        under_water(choice);
    }
    if live.iter().all(|(pairs, s)| {
        state_of(pairs, t, &theirs)
            .is_some_and(|v| same(&v.outline, &s.outline) && same(&v.collision, &s.collision))
    }) {
        return;
    }
    let any_outline = live.iter().any(|(_, s)| bounding_box(&s.outline).is_some());
    let any_collision = live.iter().any(|(_, s)| bounding_box(&s.collision).is_some());
    if live.iter().any(|(_, s)| !full_cube(&s.outline)) {
        choice.properties.insert("no_occlusion".into(), json!(true));
        // Without the pack's run: what `BlockBehaviour` works out of a shape that isn't a full block.
        if lets_light_through.is_none() && t.default_state().is_some_and(|s| full_cube(&s.outline)) {
            choice.properties.insert("light_dampening".into(), json!(0));
            choice.properties.insert("skylight_down".into(), json!(true));
            under_water(choice);
        }
    }
    if let Some(shapes) = shapes_by_template(&live, t) {
        choice.declared.insert("shapes".into(), shapes);
        return;
    }
    let boxes = |of: &dyn Fn(&State) -> &Value| {
        selectors(&live, names, |s| bounding_box(of(s)).map(|b| b.map(|v| (v * 1e6) as i64)))
    };
    let (outline, collision) = (boxes(&|s| &s.outline), boxes(&|s| &s.collision));
    if (any_outline && outline.is_none()) || (any_collision && collision.is_none()) {
        if let Some(shapes) = shapes_at_defaults(block, &live, t) {
            choice.declared.insert("shapes".into(), shapes);
            if !any_collision {
                choice.properties.insert("no_collision".into(), json!(true));
            }
            choice
                .differences
                .push("its shapes follow more than one property: by the template's, the rest at their defaults".into());
            return;
        }
    }
    // A state with no shape where the others have one takes a box too small to meet: the game has no way to say
    // none for one state, and the template's cube would stand. Failing selectors, the default state's for all.
    let default_state = block.default_state();
    let mut declare = |key: &str, empty_key: &str, any: bool, map: Option<BTreeMap<String, Option<[i64; 6]>>>, of: &dyn Fn(&State) -> &Value| {
        if !any {
            choice.properties.insert(empty_key.into(), json!(true));
            return;
        }
        let map = map.unwrap_or_else(|| {
            choice.differences.push(format!(
                "its {key} follow properties it can't select on: the default state's for all"
            ));
            let shape = default_state.and_then(|s| bounding_box(of(s))).map(|b| b.map(|v| (v * 1e6) as i64));
            BTreeMap::from([("*".to_owned(), shape)])
        });
        let map: BTreeMap<String, [f64; 6]> = map
            .into_iter()
            .map(|(k, v)| (k, v.map_or(NOTHING, |b| b.map(|n| n as f64 / 1e6))))
            .collect();
        choice.declared.insert(key.into(), json!(map));
    };
    declare("outline_boxes", "outline_empty", any_outline, outline, &|s| &s.outline);
    declare("collision_boxes", "no_collision", any_collision, collision, &|s| &s.collision);
}

impl Facts {
    pub fn read(path: &Path) -> Result<Facts, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The vanilla block that best stands in for `id`: the same vanilla
    /// class and states, then the most states with the same outline and
    /// collision, then the same sounds, set type, wood type and hardness.
    /// `None` when no vanilla block has its class and states.
    pub fn template(&self, id: &str, allowed: &dyn Fn(&str) -> bool) -> Option<Choice> {
        let block = self.blocks.get(id)?;
        let class = block.vanilla_class();
        let own = block.by_key();
        let ours = block.default_state()?;
        let mut best: Option<(i64, &str, &Block)> = None;
        for (name, v) in &self.blocks {
            if !name.starts_with("minecraft:")
                || v.vanilla_class() != class
                || v.custom_class()
                || !block.same_states(v)
                || !allowed(name)
            {
                continue;
            }
            let theirs = v.by_key();
            let shapes = own
                .iter()
                .filter(|(k, s)| {
                    theirs
                        .get(*k)
                        .is_some_and(|t| t.outline == s.outline && t.collision == s.collision)
                })
                .count() as i64;
            let Some(td) = v.default_state() else {
                continue;
            };
            let same_set = match (&block.block_set_type, &v.block_set_type) {
                (Some(a), Some(b)) => {
                    (a.name == b.name) as i64 * 2
                        + (a.can_open_by_hand == b.can_open_by_hand) as i64 * 8
                }
                _ => 0,
            };
            let score = shapes * 1000
                + (td.sound == ours.sound) as i64 * 100
                + same_set * 10
                + (block.wood_type.is_some() && block.wood_type == v.wood_type) as i64 * 20
                + (td.hardness == ours.hardness) as i64 * 5
                + (td.map_color_id == ours.map_color_id) as i64
                - rail_kind(id, name) * 3;
            if best.is_none_or(|(s, n, _)| score > s || (score == s && name.as_str() < n)) {
                best = Some((score, name, v));
            }
        }
        let (_, template, t) = best?;
        Some(self.choice(id, block, template, t, allowed))
    }

    /// `template`, a vanilla block guessed from the blockstate as carry does without the oracle, standing in
    /// for `id` when no vanilla block has its class and states, with the mod's numbers over it.
    pub fn guessed(&self, id: &str, template: &str, allowed: &dyn Fn(&str) -> bool) -> Option<Choice> {
        let (block, t) = (self.blocks.get(id)?, self.blocks.get(template)?);
        let mut choice = self.choice(id, block, template, t, allowed);
        choice.differences.insert(
            0,
            format!(
                "no vanilla {} has its states: {template} guessed from its blockstate",
                block.vanilla_class().rsplit('.').next().unwrap_or("")
            ),
        );
        Some(choice)
    }

    fn choice(
        &self,
        id: &str,
        block: &Block,
        template: &str,
        t: &Block,
        allowed: &dyn Fn(&str) -> bool,
    ) -> Choice {
        let (ours, theirs) = (block.default_state(), t.default_state());
        let mut properties = Map::new();
        let mut differences = Vec::new();
        if let Some(s) = ours {
            properties.insert("hardness".into(), json!(s.hardness));
            properties.insert(
                "explosion_resistance".into(),
                json!(block.explosion_resistance),
            );
            // `light` is every state's: one that changes (a lamp's `lit`) keeps the template's per state, or
            // gets `light_emission`.
            if block.states.values().all(|o| o.emission == s.emission) {
                properties.insert("light".into(), json!(s.emission));
            }
            properties.insert("requires_tool".into(), json!(s.requires_tool));
            properties.insert("friction".into(), json!(block.friction));
            // Vanilla's colours are 0 to 61; a mod may add its own past them (Mekanism's fluids), which the game
            // refuses: the template's then stands.
            if s.map_color_id <= 61 {
                properties.insert("map_color".into(), json!(s.map_color_id));
            }
            if theirs.is_none_or(|t| t.sound != s.sound) {
                match self.sound_block(&s.sound, allowed) {
                    Some(b) => {
                        properties.insert("sounds".into(), json!(b));
                    }
                    None => differences.push("sounds: no vanilla block has them".into()),
                }
            }
        }
        if (block.speed_factor, block.jump_factor) != (t.speed_factor, t.jump_factor) {
            differences.push(format!(
                "speed and jump factors {} {} (template {} {})",
                block.speed_factor, block.jump_factor, t.speed_factor, t.jump_factor
            ));
        }
        let theirs = t.by_key();
        let mut shapes = 0;
        let mut varying = BTreeSet::new();
        let first = ours.map(|s| (s.hardness, s.emission, s.requires_tool, s.map_color_id));
        for (k, s) in block.by_key() {
            let Some(v) = theirs.get(&k) else {
                continue;
            };
            if v.outline != s.outline || v.collision != s.collision {
                shapes += 1;
            }
            if Some((s.hardness, s.emission, s.requires_tool, s.map_color_id)) != first {
                varying.insert("per-state hardness, light or map colour");
            }
        }
        if shapes > 0 {
            differences.push(format!(
                "shapes differ from {template} in {shapes} of {} states",
                block.states.len()
            ));
        }
        differences.extend(varying.into_iter().map(str::to_owned));
        if block.custom_class() {
            differences.push(format!(
                "class {} (behaviour beyond {})",
                block.class[0],
                block.vanilla_class()
            ));
        }
        // From Java's own blocks, where the template's differ: what a piston does to it (a carved pumpkin pops, a
        // machine doesn't move) and what it does to light.
        if let (Some(ours), Some(theirs)) = (self.pack.block(id), self.pack.block(template)) {
            if ours.push_reaction() != theirs.push_reaction() {
                properties.insert("push_reaction".into(), json!(ours.push_reaction()));
            }
            if ours.dampening != theirs.dampening {
                properties.insert("light_dampening".into(), json!(ours.dampening));
            }
            if ours.skylight_down != theirs.skylight_down {
                properties.insert("skylight_down".into(), json!(ours.skylight_down));
            }
            if !ours.occludes && theirs.occludes {
                properties.insert("no_occlusion".into(), json!(true));
            }
        }
        // Where the mod's default differs on a property the template has.
        let mut declared = Map::new();
        let theirs_default = t.default_pairs();
        let default_state: Map<String, Value> = block
            .default_pairs()
            .into_iter()
            .filter(|(k, v)| theirs_default.get(k).is_some_and(|w| w != v))
            .map(|(k, v)| (k, json!(v)))
            .collect();
        if !default_state.is_empty() {
            declared.insert("default_state".into(), Value::Object(default_state));
        }
        Choice {
            template: template.to_owned(),
            properties,
            differences,
            declared,
            states: t.states.len(),
        }
    }

    /// The vanilla block for `id` when none has its class and states: one whose properties are all among its own
    /// with the same values, so the rest can be declared. Of those a block of a class in its own chain, one of a
    /// class below its first vanilla class (a rail on a powered rail) unless that says nothing (`Block`), `guess`
    /// (from its blockstate), or one of the [`PLAIN`] templates that fits; the most properties shared, then the
    /// class, then the most states with the same shapes, sounds and numbers. Plain `Block`s have no properties, so
    /// every block has one.
    ///
    /// `modelled` holds the properties its blockstate names, when only those are to be declared: the game holds 2^20
    /// states in all, ATM10's blocks have more in Java, and until a port reads it a property no model reads does
    /// nothing but multiply a block's states.
    pub fn extended(
        &self,
        id: &str,
        guess: Option<&str>,
        modelled: Option<&BTreeSet<String>>,
        allowed: &dyn Fn(&str) -> bool,
    ) -> Option<Choice> {
        let block = self.blocks.get(id)?;
        let first = block.vanilla_class();
        let chain: BTreeSet<&str> = block
            .class
            .iter()
            .filter(|c| c.starts_with(VANILLA_PACKAGE))
            .map(String::as_str)
            .collect();
        let says_nothing = GENERAL_CLASSES.contains(&first.rsplit('.').next().unwrap_or(""));
        let ours = block.default_state()?;
        let mut best: Option<(i64, &str, &Block)> = None;
        for (name, v) in &self.blocks {
            if !name.starts_with("minecraft:")
                || v.custom_class()
                || !v.properties_within(block)
                || !allowed(name)
            {
                continue;
            }
            let plain_class = v.vanilla_class().rsplit('.').next() == Some("Block");
            let kin = chain.contains(v.vanilla_class());
            let below = !says_nothing && v.class.iter().any(|c| c == first);
            let fits = shapes_fitting(block, v);
            let cube = v.default_state().is_some_and(|s| full_cube(&s.outline));
            // A template with a shape of its own brings what goes with it: a pressure plate falls off without a
            // floor, a button has no collision. So only for a block of about that shape, even if its blockstate
            // names the same properties.
            let shape_ok = cube || fits * 2 >= block.states.len();
            let fitting = shape_ok && behaviour_fits(name, id, block, v);
            let guessed = guess == Some(name.as_str()) && fitting;
            let plain = PLAIN.contains(&name.as_str()) && fitting;
            if !(kin || below || guessed || plain) {
                continue;
            }
            let Some(td) = v.default_state() else {
                continue;
            };
            // A template without an item costs the block its own.
            let no_item = v.item == "minecraft:air" && block.item != "minecraft:air";
            let score = v.properties.len() as i64 * 1000
                + ((kin && !plain_class) as i64) * 800
                + (below as i64) * 600
                + (guessed as i64) * 300
                + (fits * 500 / block.states.len().max(1)) as i64
                + (td.sound == ours.sound) as i64 * 100
                + (block.wood_type.is_some() && block.wood_type == v.wood_type) as i64 * 20
                + (td.hardness == ours.hardness) as i64 * 5
                + (td.map_color_id == ours.map_color_id) as i64
                - rail_kind(id, name) * 3
                - (no_item as i64) * 100_000;
            if best.is_none_or(|(s, n, _)| score > s || (score == s && name.as_str() < n)) {
                best = Some((score, name, v));
            }
        }
        let (_, template, t) = best?;
        let mut choice = self.choice(id, block, template, t, allowed);
        let (names, unread): (Vec<&str>, Vec<&str>) = block
            .properties
            .keys()
            .filter(|k| !t.properties.contains_key(*k))
            .map(String::as_str)
            .partition(|k| modelled.is_none_or(|m| m.contains(*k)));
        choice.differences.insert(
            0,
            format!(
                "no vanilla {} has its states: {template}, declaring [{}]",
                first.rsplit('.').next().unwrap_or(""),
                names.join(", ")
            ),
        );
        if !unread.is_empty() {
            choice.differences.insert(1, format!("no model reads [{}]: not declared", unread.join(", ")));
        }
        declare_states(block, t, &names, self.pack.block(id), &mut choice);
        Some(choice)
    }

    /// A vanilla block whose default state has these sounds.
    fn sound_block(&self, sound: &Value, allowed: &dyn Fn(&str) -> bool) -> Option<&str> {
        self.blocks
            .iter()
            .filter(|(n, _)| n.starts_with("minecraft:") && allowed(n))
            .find(|(_, b)| b.default_state().is_some_and(|s| &s.sound == sound))
            .map(|(n, _)| n.as_str())
    }

    /// An item declaration for a mod item that is not a declared block's:
    /// the vanilla item of its class with its stack size, rarity and numbers,
    /// and what it does not reproduce.
    pub fn item(&self, id: &str, owned: &[&str]) -> Result<(Value, Vec<String>), String> {
        let item = self.items.get(id).ok_or("not in the oracle")?;
        let mut differences = Vec::new();
        // The item of a block with another id (Pam's seeds plant `<name>crop`): an item that does not place
        // it, as carry declares it without the oracle.
        if let Some(block) = &item.block {
            differences.push(format!("places {block}, which the template does not"));
        }
        if !item.class.starts_with(VANILLA_PACKAGE) {
            differences.push(format!("class {} (behaviour beyond Item)", item.class));
        }
        if let Some(food) = item.components.get("minecraft:food") {
            differences.extend(food_26_3(food, None).3);
        }
        let template = item_template(item);
        let mut decl = json!({"template": template});
        // Against the template's own stack, where the game's tables say it (a sword's is 1).
        let had_stack = self
            .vanilla_items
            .get(template)
            .and_then(|b| b["minecraft:max_stack_size"].as_u64())
            .unwrap_or(64);
        if u64::from(item.max_stack_size) != had_stack {
            decl["max_stack_size"] = json!(item.max_stack_size.clamp(1, 99));
        }
        // NeoForge lets mods add rarities (`aether:loot`); the game has vanilla's four, and refuses the item.
        match item.rarity.as_str() {
            "common" => {}
            "uncommon" | "rare" | "epic" => decl["rarity"] = json!(item.rarity),
            other => differences.push(format!("rarity {other} (the template's stays)")),
        }
        if let Some(armor) = item.armor.as_ref().filter(|a| !a.material.is_empty())
            && !self.armor_asset_exists(armor, owned)
        {
            differences.push(format!(
                "armour material {} has no equipment asset: it draws the template's",
                armor.material
            ));
        }
        let components = self.item_components(id, item, template, owned);
        if !components.is_empty() {
            decl["components"] = Value::Object(components);
        }
        Ok((decl, differences))
    }

    /// The component patch of a declared block's item over its template's item: what Java's item says, not what
    /// the template's happens to have (a carved pumpkin's head slot, a coal block's burn time). A block file has no
    /// `max_stack_size` or `rarity` keys, so they go here.
    pub fn block_item_components(
        &self,
        id: &str,
        template_item: &str,
        owned: &[&str],
    ) -> (Map<String, Value>, Vec<String>) {
        let Some(item) = self.items.get(id) else {
            return (Map::new(), Vec::new());
        };
        let mut patch = self.item_components(id, item, template_item, owned);
        let mut differences = Vec::new();
        let base = self.vanilla_items.get(template_item);
        let had_stack = base.and_then(|b| b["minecraft:max_stack_size"].as_u64());
        let stack = u64::from(item.max_stack_size).clamp(1, 99);
        if had_stack.is_some_and(|h| h != stack) {
            patch.insert("minecraft:max_stack_size".into(), json!(stack));
        }
        let had_rarity = base.and_then(|b| b["minecraft:rarity"].as_str());
        match item.rarity.as_str() {
            "common" | "uncommon" | "rare" | "epic" => {
                if had_rarity.is_some_and(|h| h != item.rarity) {
                    patch.insert("minecraft:rarity".into(), json!(item.rarity));
                }
            }
            other => differences.push(format!("rarity {other} (the template's stays)")),
        }
        (patch, differences)
    }

    /// The item's numbers as a 26.3 component patch over `template`'s, leaving out what already matches.
    fn armor_asset_exists(&self, armor: &Armor, owned: &[&str]) -> bool {
        armor.asset_carried(owned)
            || self.vanilla_items.values().any(|i| {
                i["minecraft:equippable"]["asset_id"].as_str() == Some(armor.material.as_str())
            })
    }

    fn item_components(&self, id: &str, item: &Item, template: &str, owned: &[&str]) -> Map<String, Value> {
        let base = self.vanilla_items.get(template);
        let mut want = Map::new();
        // Fuel, compost and the slot it is worn in are components in 26.3, and data maps or code in 1.21.1.
        if let Some(pack) = self.pack.item(id) {
            want.insert("minecraft:cooking_fuel".into(), cooking_fuel(pack.burn, base));
            want.insert(
                "minecraft:compostable".into(),
                pack.compost.map_or(Value::Null, compostable),
            );
            let had_slot = base.and_then(|b| b["minecraft:equippable"]["slot"].as_str());
            // Armour with a material gets its slot below; one without (a jetpack, mage robes) still goes on.
            let armour_below = item.armor.as_ref().is_some_and(|a| !a.material.is_empty());
            if !armour_below && had_slot != pack.worn.as_deref() {
                let worn = pack.worn.as_ref().map(|slot| json!({"slot": slot}));
                want.insert("minecraft:equippable".into(), worn.unwrap_or(Value::Null));
            }
        }
        if item.components.is_empty() {
            return component_patch(want, base);
        }
        want.insert(
            "minecraft:max_damage".into(),
            item.components
                .get("minecraft:max_damage")
                .cloned()
                .unwrap_or(Value::Null),
        );
        match &item.attribute_modifiers {
            Some(m) => _ = want.insert("minecraft:attribute_modifiers".into(), modifiers_26_3(m)),
            // None is none: a carved pumpkin's head modifier is no block's.
            None if base.is_some_and(|b| {
                b["minecraft:attribute_modifiers"].as_array().is_some_and(|m| !m.is_empty())
            }) =>
            {
                want.insert("minecraft:attribute_modifiers".into(), json!([]));
            }
            None => {}
        }
        // 1.21.1's fire resistance is 26.3's resistance to one damage tag.
        let fireproof = item.components.contains_key("minecraft:fire_resistant");
        want.insert(
            "minecraft:damage_resistant".into(),
            if fireproof { json!({"types": "#minecraft:is_fire"}) } else { Value::Null },
        );
        // What a template's block entity keeps in its item, where Java's item has no such thing.
        for key in [
            "minecraft:container",
            "minecraft:banner_patterns",
            "minecraft:bees",
            "minecraft:block_state",
            "minecraft:pot_decorations",
        ] {
            if !item.components.contains_key(key) {
                want.insert(key.into(), Value::Null);
            }
        }
        // A creeper head's: 1.21.1 tests vanilla's ids for this, so no mod's head has it.
        want.insert("minecraft:mob_visibility".into(), Value::Null);
        // The mod's rules as they are; what 1.21.1 kept in the class (can_destroy_blocks_in_creative) stays the template's.
        let tool = item.components.get("minecraft:tool").map(|t| {
            let mut tool = base
                .and_then(|b| b.get("minecraft:tool"))
                .cloned()
                .unwrap_or_else(|| json!({}));
            for key in ["rules", "default_mining_speed", "damage_per_block"] {
                match t.get(key) {
                    Some(v) => tool[key] = v.clone(),
                    None => _ = tool.as_object_mut().map(|o| o.remove(key)),
                }
            }
            tool
        });
        want.insert("minecraft:tool".into(), tool.unwrap_or(Value::Null));
        if let Some(food) = item.components.get("minecraft:food") {
            let (food, consumable, remainder, _) =
                food_26_3(food, base.and_then(|b| b.get("minecraft:consumable")));
            want.insert("minecraft:food".into(), food);
            want.insert("minecraft:consumable".into(), consumable);
            want.insert("minecraft:use_remainder".into(), remainder);
        }
        if let Some(value) = item.enchantment_value {
            // 1.21.1 enchants only a stack of one (`Item.isEnchantable`), whatever the value says: in the pack's run
            // Apothic Enchanting gives every item at least 1, and a block item stacking to 64 must not gain it.
            let enchantable =
                (value > 0 && item.max_stack_size == 1).then(|| json!({"value": value}));
            want.insert(
                "minecraft:enchantable".into(),
                enchantable.unwrap_or(Value::Null),
            );
        }
        if let Some(repair) = &item.repair_items {
            let items = match repair.as_slice() {
                [] => Value::Null,
                [one] => json!({"items": one}),
                many => json!({"items": many}),
            };
            want.insert("minecraft:repairable".into(), items);
        }
        if let Some(armor) = item.armor.as_ref().filter(|a| !a.material.is_empty()) {
            let mut equippable = base.and_then(|b| b.get("minecraft:equippable")).cloned();
            if self.armor_asset_exists(armor, owned) {
                equippable.get_or_insert_with(|| json!({"slot": armor.slot}))["asset_id"] =
                    json!(armor.material);
            } else if equippable.is_none() && base.is_some() {
                // On a template that isn't worn (a jetpack on a stick): worn all the same, drawn with no asset.
                equippable = Some(json!({"slot": armor.slot}));
            }
            if let Some(mut equippable) = equippable {
                if let Some(sound) = &armor.equip_sound {
                    equippable["equip_sound"] = json!(sound);
                }
                want.insert("minecraft:equippable".into(), equippable);
            }
        }
        component_patch(want, base)
    }

    /// A tag's members the content mod may name: vanilla's and its own.
    pub fn tag_members(&self, kind: &str, tag: &str, own: &str) -> Option<Vec<String>> {
        let map = if kind == "block" {
            &self.tags.block
        } else {
            &self.tags.item
        };
        let members = map.get(tag)?;
        Some(
            members
                .iter()
                .filter(|m| {
                    m.split_once(':')
                        .is_some_and(|(ns, _)| ns == "minecraft" || ns == own)
                })
                .cloned()
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(hardness: f32, sound: &str, outline: Value) -> Value {
        json!({"hardness": hardness, "emission": 0, "requires_tool": false, "map_color_id": 13,
            "sound": {"break": sound}, "outline": outline, "collision": outline})
    }
    fn block(class: &[&str], props: Value, states: Value) -> Value {
        let default = states
            .as_object()
            .unwrap()
            .keys()
            .next()
            .unwrap()
            .to_owned();
        json!({"class": class, "properties": props, "default": default, "item": "x",
            "explosion_resistance": 3.0, "friction": 0.6, "speed_factor": 1.0,
            "jump_factor": 1.0, "states": states})
    }

    #[test]
    fn templates_follow_class_shapes_and_sounds() {
        let cube = json!([[0, 0, 0, 1, 1, 1]]);
        let low = json!([[0, 0, 0, 1, 0.9375, 1]]);
        let block_class = ["net.minecraft.world.level.block.Block"];
        let path = ["net.minecraft.world.level.block.DirtPathBlock"];
        let facts: Facts = serde_json::from_value(json!({
            "blocks": {
                "minecraft:stone": block(&block_class, json!({}), json!({"": state(1.5, "stone", cube.clone())})),
                "minecraft:oak_planks": block(&block_class, json!({}), json!({"": state(2.0, "wood", cube.clone())})),
                "minecraft:dirt_path": block(&path, json!({}), json!({"": state(0.65, "grass", low.clone())})),
                "m:wood_tile": block(&["m.Tile", block_class[0]], json!({}), json!({"": state(2.5, "wood", cube.clone())})),
                "m:path": block(&["m.Path", block_class[0]], json!({}), json!({"": state(1.0, "stone", low.clone())})),
                "m:gadget": block(&["m.Gadget", block_class[0]], json!({"mode": ["a", "b"]}),
                    json!({"mode=a": state(1.0, "stone", cube.clone()), "mode=b": state(1.0, "stone", cube)})),
            },
            "items": {}
        }))
        .unwrap();
        let any = |_: &str| true;
        let tile = facts.template("m:wood_tile", &any).unwrap();
        assert_eq!(tile.template, "minecraft:oak_planks");
        assert_eq!(tile.properties["hardness"], json!(2.5));
        assert!(!tile.properties.contains_key("sounds"));
        assert_eq!(
            tile.differences,
            ["class m.Tile (behaviour beyond net.minecraft.world.level.block.Block)"]
        );
        // A plain Block is never templated on a DirtPathBlock: shapes differ, and say so.
        let path = facts.template("m:path", &any).unwrap();
        assert_eq!(path.template, "minecraft:stone");
        assert!(
            path.differences[0].starts_with("shapes differ"),
            "{:?}",
            path.differences
        );
        assert_eq!(facts.template("m:gadget", &any), None);
        // Without one of its class, a guess from the blockstate takes the mod's numbers all the same.
        let gadget = facts.guessed("m:gadget", "minecraft:stone", &any).unwrap();
        assert_eq!(gadget.properties["hardness"], json!(1.0));
        assert_eq!(
            gadget.differences[0],
            "no vanilla Block has its states: minecraft:stone guessed from its blockstate"
        );
        let tile = facts
            .template("m:wood_tile", &|n: &str| n != "minecraft:oak_planks")
            .unwrap();
        assert_eq!(tile.template, "minecraft:stone");
        assert_eq!(tile.differences[0], "sounds: no vanilla block has them");
    }

    #[test]
    fn drops_itself_is_vanillas_drop_self() {
        let cube = json!([[0, 0, 0, 1, 1, 1]]);
        let with = |requires_tool: bool, drops: Value| {
            let mut s = state(1.5, "stone", cube.clone());
            s["requires_tool"] = json!(requires_tool);
            s["drops"] = drops;
            let b: Block = serde_json::from_value(block(&[], json!({}), json!({"": s}))).unwrap();
            b.drops_itself("m:tile")
        };
        let me = json!({"m:tile": 1});
        let always = Some(SelfDrop::Always);
        assert_eq!(with(false, json!({"hand": me, "shears": me})), always);
        // A state that needs a tool drops nothing by hand, as dropSelf does.
        assert_eq!(
            with(true, json!({"hand": {}, "diamond_pickaxe": me})),
            always
        );
        assert_eq!(
            with(false, json!({"hand": {}, "diamond_pickaxe": me})),
            None
        );
        // Two of itself (a double slab), or something else, is not dropSelf.
        assert_eq!(with(false, json!({"hand": {"m:tile": 2}})), None);
        assert_eq!(with(false, json!({"hand": {"m:other": 1}})), None);
        assert_eq!(with(false, json!({})), None);
    }

    #[test]
    fn a_door_drops_from_its_lower_half() {
        let cube = json!([[0, 0, 0, 1, 1, 1]]);
        let me = json!({"hand": {"m:door": 1}, "shears": {"m:door": 1}});
        let none = json!({"hand": {}, "shears": {}});
        let door = |upper: &Value| {
            let mut states = serde_json::Map::new();
            for (key, drops) in [
                ("half=lower,open=false", &me),
                ("half=lower,open=true", &me),
                ("half=upper,open=false", upper),
                ("half=upper,open=true", upper),
            ] {
                let mut s = state(3.0, "wood", cube.clone());
                s["drops"] = drops.clone();
                states.insert(key.to_owned(), s);
            }
            let props = json!({"half": ["upper", "lower"], "open": ["true", "false"]});
            let b: Block =
                serde_json::from_value(block(&[], props, Value::Object(states))).unwrap();
            b.drops_itself("m:door")
        };
        assert_eq!(
            door(&none),
            Some(SelfDrop::When("half".to_owned(), "lower".to_owned()))
        );
        assert_eq!(door(&me), Some(SelfDrop::Always));
        // Something else from the upper half: not a door table.
        assert_eq!(door(&json!({"hand": {"m:other": 1}})), None);
    }

    #[test]
    fn items_of_vanilla_classes_take_their_template_and_numbers() {
        let mut facts: Facts = serde_json::from_value(json!({
            "blocks": {},
            "items": {
                "m:sword": {"class": "net.minecraft.world.item.SwordItem", "max_stack_size": 1, "rarity": "common",
                    "components": {"minecraft:max_damage": 905, "minecraft:tool": {"rules": [
                        {"blocks": "minecraft:cobweb", "speed": 15.0, "correct_for_drops": true}], "damage_per_block": 2}},
                    "attribute_modifiers": {"modifiers": [{"type": "minecraft:generic.attack_damage",
                        "id": "minecraft:base_attack_damage", "amount": 5.5, "operation": "add_value", "slot": "mainhand"}]},
                    "enchantment_value": 14, "repair_items": ["minecraft:emerald"]},
                "m:boots": {"class": "net.minecraft.world.item.ArmorItem", "max_stack_size": 1, "rarity": "common",
                    "components": {"minecraft:max_damage": 1}, "enchantment_value": 0, "repair_items": [],
                    "armor": {"slot": "feet"}},
                "m:gem": {"class": "net.minecraft.world.item.Item", "max_stack_size": 64, "rarity": "common"},
            }
        }))
        .unwrap();
        facts.vanilla_items = serde_json::from_value(json!({
            "minecraft:iron_sword": {"minecraft:max_damage": 250, "minecraft:enchantable": {"value": 14},
                "minecraft:repairable": {"items": "#minecraft:iron_tool_materials"},
                "minecraft:tool": {"can_destroy_blocks_in_creative": false, "damage_per_block": 2,
                    "rules": [{"blocks": "#minecraft:sword_efficient", "speed": 1.5}]}},
            "minecraft:iron_boots": {"minecraft:max_damage": 195, "minecraft:enchantable": {"value": 9},
                "minecraft:repairable": {"items": "#minecraft:repairs_iron_armor"}},
        }))
        .unwrap();
        let (sword, _) = facts.item("m:sword", &["m"]).unwrap();
        assert_eq!(sword["template"], "minecraft:iron_sword");
        let c = &sword["components"];
        assert_eq!(c["minecraft:max_damage"], 905);
        assert_eq!(
            c["minecraft:attribute_modifiers"][0]["type"],
            "minecraft:attack_damage"
        );
        assert_eq!(
            c["minecraft:repairable"],
            json!({"items": "minecraft:emerald"})
        );
        // The mod's rules replace the template's; the creative flag the class gave stays.
        assert_eq!(
            c["minecraft:tool"],
            json!({"can_destroy_blocks_in_creative": false, "damage_per_block": 2,
                "rules": [{"blocks": "minecraft:cobweb", "speed": 15.0, "correct_for_drops": true}]})
        );
        // Equal to the template's: left out.
        assert!(c.get("minecraft:enchantable").is_none());
        let (boots, _) = facts.item("m:boots", &["m"]).unwrap();
        assert_eq!(boots["template"], "minecraft:iron_boots");
        assert_eq!(boots["components"]["!minecraft:enchantable"], json!({}));
        assert_eq!(boots["components"]["!minecraft:repairable"], json!({}));
        // Facts without components (exported before they were) keep the plain template.
        let (gem, _) = facts.item("m:gem", &["m"]).unwrap();
        assert_eq!(gem, json!({"template": "minecraft:stick"}));
    }

    #[test]
    fn armour_materials_become_equipment_assets() {
        let armor: Armor = serde_json::from_value(json!({"slot": "legs", "material": "m:ruby",
            "equip_sound": "minecraft:item.armor.equip_diamond", "layers": [
                {"texture": "m:textures/models/armor/ruby_layer_1.png",
                 "texture_legs": "m:textures/models/armor/ruby_layer_2.png", "dyeable": false},
                {"texture": "m:textures/models/armor/ruby_overlay_layer_1.png",
                 "texture_legs": "m:textures/models/armor/ruby_overlay_layer_2.png", "dyeable": true}]}))
        .unwrap();
        let (asset, copies) = armor.equipment().unwrap();
        assert_eq!(
            asset["layers"]["humanoid_leggings"],
            json!([{"texture": "m:ruby"}, {"texture": "m:ruby_overlay", "dyeable": {}}])
        );
        assert_eq!(
            copies[2],
            (
                "assets/m/textures/models/armor/ruby_layer_2.png".to_owned(),
                "assets/m/textures/entity/equipment/humanoid_leggings/ruby.png".to_owned()
            )
        );
        let mut facts: Facts =
            serde_json::from_value(json!({"blocks": {}, "items": {"m:ruby_leggings": {
            "class": "net.minecraft.world.item.ArmorItem", "max_stack_size": 1, "rarity": "common",
            "components": {"minecraft:max_damage": 300}, "armor": armor_json()}}}))
            .unwrap();
        facts.vanilla_items = serde_json::from_value(json!({"minecraft:iron_leggings": {
            "minecraft:equippable": {"asset_id": "minecraft:iron", "equip_sound": "minecraft:item.armor.equip_iron", "slot": "legs"}}}))
        .unwrap();
        let (decl, _) = facts.item("m:ruby_leggings", &["m"]).unwrap();
        assert_eq!(
            decl["components"]["minecraft:equippable"],
            json!({"asset_id": "m:ruby", "equip_sound": "minecraft:item.armor.equip_diamond", "slot": "legs"})
        );
        // Layers outside models/armor/ have no 26.3 place to go.
        let odd: Armor =
            serde_json::from_value(json!({"slot": "head", "material": "m:odd", "layers": [
            {"texture": "m:textures/odd.png", "texture_legs": "m:textures/odd.png"}]}))
            .unwrap();
        assert!(odd.equipment().is_none());
    }

    #[test]
    fn armour_only_names_an_equipment_asset_that_exists() {
        let leggings = |material: &str, layers: Value| {
            json!({"class": "net.minecraft.world.item.ArmorItem", "max_stack_size": 1, "rarity": "common",
                "components": {"minecraft:max_damage": 300},
                "armor": {"slot": "legs", "material": material,
                    "equip_sound": "minecraft:item.armor.equip_diamond", "layers": layers}})
        };
        let odd = json!([{"texture": "m:textures/odd.png", "texture_legs": "m:textures/odd.png"}]);
        let mut facts: Facts = serde_json::from_value(json!({"blocks": {}, "items": {
            "m:ruby": leggings("m:ruby", json!([])),
            "m:odd": leggings("m:odd", odd),
            "m:other": leggings("other:steel", json!([])),
            "m:unknown": leggings("minecraft:steel", json!([])),
            "m:diamond": leggings("minecraft:diamond", json!([]))}}))
        .unwrap();
        facts.vanilla_items = serde_json::from_value(json!({
            "minecraft:iron_leggings": {"minecraft:equippable": {"asset_id": "minecraft:iron", "slot": "legs"}},
            "minecraft:diamond_leggings": {"minecraft:equippable": {"asset_id": "minecraft:diamond", "slot": "legs"}}}))
        .unwrap();
        let asset = |id: &str| {
            let (decl, differences) = facts.item(id, &["m"]).unwrap();
            let asset = decl["components"]["minecraft:equippable"]["asset_id"].clone();
            (asset, !differences.is_empty())
        };
        assert_eq!(asset("m:ruby"), (json!("m:ruby"), false));
        assert_eq!(asset("m:diamond"), (json!("minecraft:diamond"), false));
        for id in ["m:odd", "m:other", "m:unknown"] {
            assert_eq!(asset(id), (json!("minecraft:iron"), true), "{id}");
        }
        // Without the game's tables the template's own component stays whole.
        facts.vanilla_items.clear();
        let (decl, _) = facts.item("m:odd", &["m"]).unwrap();
        assert!(decl["components"].get("minecraft:equippable").is_none());
    }

    #[test]
    fn foods_take_a_food_template_and_their_numbers() {
        let food = |components: Value| {
            json!({"class": "net.minecraft.world.item.Item", "max_stack_size": 64, "rarity": "common",
                "food": {}, "components": {"minecraft:food": components}})
        };
        let mut facts: Facts = serde_json::from_value(json!({"blocks": {}, "items": {
            "m:pie": food(json!({"nutrition": 9, "saturation": 21.6})),
            "m:stew": food(json!({"nutrition": 6, "saturation": 7.2, "eat_seconds": 0.8,
                "using_converts_to": {"id": "minecraft:bowl", "count": 1},
                "effects": [{"effect": {"id": "minecraft:regeneration", "amplifier": 1, "duration": 100,
                    "neoforge:cures": ["milk"]}, "probability": 0.5},
                    {"effect": {"id": "m:sour", "duration": 20}}]})),
            "m:seed": {"class": "net.minecraft.world.item.ItemNameBlockItem", "max_stack_size": 64,
                "rarity": "common", "block": "m:crop"},
            "m:relic": {"class": "net.minecraft.world.item.Item", "max_stack_size": 1, "rarity": "m:relic"}}}))
        .unwrap();
        facts.vanilla_items = serde_json::from_value(json!({"minecraft:cooked_beef": {
            "minecraft:consumable": {}, "minecraft:food": {"nutrition": 8, "saturation": 12.8}}}))
        .unwrap();
        let (pie, differences) = facts.item("m:pie", &["m"]).unwrap();
        assert_eq!(pie["template"], "minecraft:cooked_beef");
        assert_eq!(
            pie["components"],
            json!({"minecraft:food": {"nutrition": 9, "saturation": 21.6}})
        );
        assert!(differences.is_empty());
        let (stew, differences) = facts.item("m:stew", &["m"]).unwrap();
        let c = &stew["components"];
        assert_eq!(c["minecraft:use_remainder"], json!({"id": "minecraft:bowl", "count": 1}));
        assert_eq!(
            c["minecraft:consumable"],
            json!({"consume_seconds": 0.8, "on_consume_effects": [{"type": "minecraft:apply_effects",
                "effects": [{"id": "minecraft:regeneration", "amplifier": 1, "duration": 100}],
                "probability": 0.5}]})
        );
        assert_eq!(differences, ["eating gives m:sour, which is not carried"]);
        // An item placing a block under another id stays an item, as without the oracle.
        let (seed, differences) = facts.item("m:seed", &["m"]).unwrap();
        assert_eq!(seed, json!({"template": "minecraft:stick"}));
        assert_eq!(differences[0], "places m:crop, which the template does not");
        // A rarity a mod added is no rarity the game knows.
        let (relic, differences) = facts.item("m:relic", &["m"]).unwrap();
        assert_eq!(relic, json!({"template": "minecraft:stick", "max_stack_size": 1}));
        assert_eq!(differences, ["rarity m:relic (the template's stays)"]);
    }

    fn armor_json() -> Value {
        json!({"slot": "legs", "material": "m:ruby", "equip_sound": "minecraft:item.armor.equip_diamond"})
    }

    /// A family of blocks for [`Facts::extended`]: vanilla's plain ones and the mod's, with every state cubic
    /// unless `outline` says otherwise.
    fn states_of(props: &[(&str, &[&str])], shape: &dyn Fn(&str) -> Value) -> Value {
        let mut keys = vec![String::new()];
        for (name, values) in props {
            let mut next = Vec::new();
            for k in &keys {
                for v in *values {
                    let sep = if k.is_empty() { "" } else { "," };
                    next.push(format!("{k}{sep}{name}={v}"));
                }
            }
            keys = next;
        }
        let mut out = Map::new();
        for k in keys {
            let mut s = state(1.5, "stone", shape(&k));
            s["emission"] = json!(if k.contains("lit=true") { 13 } else { 0 });
            out.insert(k, s);
        }
        Value::Object(out)
    }

    fn with_props(class: &[&str], props: &[(&str, &[&str])], default: &str, shape: &dyn Fn(&str) -> Value) -> Value {
        let map: Map<String, Value> =
            props.iter().map(|(k, v)| ((*k).to_owned(), json!(v))).collect();
        let mut b = block(class, Value::Object(map), states_of(props, shape));
        b["default"] = json!(default);
        b["item"] = json!("some:item");
        b
    }

    const BLOCK: &str = "net.minecraft.world.level.block.Block";
    const FACING: (&str, &[&str]) = ("facing", &["north", "east", "south", "west"]);
    const BOOL: &[&str] = &["true", "false"];

    fn family(mods: Value) -> Facts {
        let cube = |_: &str| json!([[0, 0, 0, 1, 1, 1]]);
        let low = |_: &str| json!([[0, 0, 0, 1, 0.0625, 1]]);
        let rail = "net.minecraft.world.level.block.BaseRailBlock";
        let powered = "net.minecraft.world.level.block.PoweredRailBlock";
        let leaves = "net.minecraft.world.level.block.LeavesBlock";
        let rail_shape: (&str, &[&str]) = ("shape", &["north_south", "east_west"]);
        let mut blocks = json!({
            "minecraft:stone": with_props(&[BLOCK], &[], "", &cube),
            "minecraft:carved_pumpkin": with_props(
                &["net.minecraft.world.level.block.EquipableCarvedPumpkinBlock",
                  "net.minecraft.world.level.block.CarvedPumpkinBlock",
                  "net.minecraft.world.level.block.HorizontalDirectionalBlock", BLOCK],
                &[FACING], "facing=north", &cube),
            "minecraft:oak_pressure_plate": with_props(
                &["net.minecraft.world.level.block.PressurePlateBlock", BLOCK],
                &[("powered", BOOL)], "powered=false", &low),
            "minecraft:loom": with_props(
                &["net.minecraft.world.level.block.LoomBlock",
                  "net.minecraft.world.level.block.HorizontalDirectionalBlock", BLOCK],
                &[FACING], "facing=north", &cube),
            "minecraft:powered_rail": with_props(&[powered, rail, BLOCK],
                &[("powered", BOOL), rail_shape, ("waterlogged", BOOL)],
                "powered=false,shape=north_south,waterlogged=false", &low),
            // 1.21.1 builds it from `PoweredRailBlock` too.
            "minecraft:activator_rail": with_props(&[powered, rail, BLOCK],
                &[("powered", BOOL), rail_shape, ("waterlogged", BOOL)],
                "powered=false,shape=north_south,waterlogged=false", &low),
            "minecraft:oak_leaves": with_props(&[leaves, BLOCK],
                &[("distance", &["1", "2"]), ("persistent", BOOL), ("waterlogged", BOOL)],
                "distance=2,persistent=false,waterlogged=false", &cube),
            "minecraft:wheat": with_props(&["net.minecraft.world.level.block.CropBlock", BLOCK],
                &[("age", &["0", "1"])], "age=0", &low),
        });
        blocks.as_object_mut().unwrap().extend(mods.as_object().unwrap().clone());
        serde_json::from_value(json!({"blocks": blocks, "items": {}})).unwrap()
    }

    #[test]
    fn a_block_no_template_fits_declares_its_properties() {
        let cube = |_: &str| json!([[0, 0, 0, 1, 1, 1]]);
        let props: &[(&str, &[&str])] = &[("active", BOOL), ("direction", &["up", "down", "left"])];
        let facts = family(json!({"m:machine": with_props(
            &["m.Machine", "net.minecraft.world.level.block.BaseEntityBlock", BLOCK],
            props, "active=false,direction=left", &cube)}));
        let any = |_: &str| true;
        assert_eq!(facts.template("m:machine", &any), None);
        let c = facts.extended("m:machine", None, None, &any).unwrap();
        assert_eq!(c.template, "minecraft:stone");
        assert_eq!(
            Value::Object(c.declared),
            json!({"state_properties": {"active": ["true", "false"], "direction": ["up", "down", "left"]},
                "state_defaults": {"active": "false", "direction": "left"}})
        );
        assert_eq!(c.states, 6);
        assert_eq!(c.differences[0], "no vanilla BaseEntityBlock has its states: minecraft:stone, declaring [active, direction]");
        // Only what its blockstate names, when that is what's asked: the rest would only multiply its states.
        let named = BTreeSet::from(["direction".to_owned()]);
        let c = facts.extended("m:machine", None, Some(&named), &any).unwrap();
        assert_eq!(c.declared["state_properties"], json!({"direction": ["up", "down", "left"]}));
        assert_eq!(c.states, 3);
        assert_eq!(c.differences[1], "no model reads [active]: not declared");
    }

    #[test]
    fn declared_properties_place_as_java_placed_them() {
        let cube = |_: &str| json!([[0, 0, 0, 1, 1, 1]]);
        let dir: (&str, &[&str]) = ("direction", &["north", "east", "south", "west"]);
        let side: (&str, &[&str]) = ("side", &["down", "up", "north", "south", "west", "east"]);
        let placed = |default: &str, up: &str, north: &str| {
            let mut b = with_props(&["m.Chair", "net.minecraft.world.level.block.BaseEntityBlock", BLOCK], &[dir, side],
                default, &cube);
            b["placement"] = json!({"up": up, "north": north});
            b
        };
        let facts = family(json!({
            // Faces the player, and sticks to the face clicked.
            "m:chair": placed("direction=south,side=down", "direction=north,side=up", "direction=north,side=north"),
            // Where the default state is what it gives: nothing to tell.
            "m:stool": placed("direction=north,side=up", "direction=north,side=up", "direction=north,side=north"),
            "m:bench": placed("direction=north,side=down", "direction=south,side=down", "direction=south,side=down"),
        }));
        let any = |_: &str| true;
        let rules = |id: &str| facts.extended(id, None, None, &any).unwrap().declared.get("placement").cloned();
        assert_eq!(rules("m:chair"), Some(json!({"direction": "horizontal_look", "side": "clicked_face"})));
        assert_eq!(rules("m:stool"), Some(json!({"side": "clicked_face"})));
        assert_eq!(rules("m:bench"), Some(json!({"direction": "horizontal_look_opposite"})));
    }

    #[test]
    fn a_template_gives_the_properties_it_has() {
        let cube = |_: &str| json!([[0, 0, 0, 1, 1, 1]]);
        let props: &[(&str, &[&str])] = &[FACING, ("lit", BOOL)];
        let facts = family(json!({"m:oven": with_props(&["m.Oven", BLOCK], props, "facing=east,lit=false", &cube)}));
        let c = facts.extended("m:oven", None, None, &|_| true).unwrap();
        // Placed by its facing, never on the loom, whose menu it does not have.
        assert_eq!(c.template, "minecraft:carved_pumpkin");
        assert_eq!(
            Value::Object(c.declared),
            json!({"default_state": {"facing": "east"}, "state_properties": {"lit": ["true", "false"]},
                "state_defaults": {"lit": "false"}, "light_emission": {"*": 13, "lit=false": 0}})
        );
        assert_eq!(c.states, 8);
    }

    #[test]
    fn a_class_finds_its_kin_and_a_flat_template_only_its_shape() {
        let cube = |_: &str| json!([[0, 0, 0, 1, 1, 1]]);
        let low = |_: &str| json!([[0, 0, 0, 1, 0.0625, 1]]);
        let rail = "net.minecraft.world.level.block.BaseRailBlock";
        let leaves = "net.minecraft.world.level.block.LeavesBlock";
        let facts = family(json!({
            "m:rail": with_props(&["m.Rail", rail, BLOCK],
                &[("powered", BOOL), ("shape", &["north_south", "east_west"]), ("waterlogged", BOOL), ("speed", &["1", "2"])],
                "powered=false,shape=north_south,speed=1,waterlogged=false", &low),
            "m:leaf": with_props(&["m.Leaf", leaves, BLOCK],
                &[("age", &["0", "1"]), ("distance", &["1", "2"]), ("persistent", BOOL), ("waterlogged", BOOL)],
                "age=0,distance=2,persistent=false,waterlogged=false", &cube),
            "m:activator_rail": with_props(&["m.Rail", rail, BLOCK],
                &[("powered", BOOL), ("shape", &["north_south", "east_west"]), ("waterlogged", BOOL), ("speed", &["1", "2"])],
                "powered=false,shape=north_south,speed=1,waterlogged=false", &low),
            "m:wired": with_props(&["m.Wired", BLOCK], &[("powered", BOOL)], "powered=false", &cube),
            "m:mat": with_props(&["m.Mat", BLOCK], &[("powered", BOOL)], "powered=false", &low),
        }));
        let any = |_: &str| true;
        let rail = facts.extended("m:rail", None, None, &any).unwrap();
        assert_eq!(rail.template, "minecraft:powered_rail");
        assert_eq!(rail.declared["state_properties"], json!({"speed": ["1", "2"]}));
        // A rail of the powered rail's class boosts, unless its name says it activates.
        let activator = facts.extended("m:activator_rail", None, None, &any).unwrap();
        assert_eq!(activator.template, "minecraft:activator_rail");
        // A guess of wheat from its `age` loses to the leaves its class is.
        let leaf = facts.extended("m:leaf", Some("minecraft:wheat"), None, &any).unwrap();
        assert_eq!(leaf.template, "minecraft:oak_leaves");
        assert_eq!(leaf.declared["state_properties"], json!({"age": ["0", "1"]}));
        // A cube with `powered` is no pressure plate, which would fall off without a floor.
        let wired = facts.extended("m:wired", None, None, &any).unwrap();
        assert_eq!(wired.template, "minecraft:stone");
        // Not when its blockstate says `powered` either; a plate-shaped block may be one.
        let plate = Some("minecraft:oak_pressure_plate");
        let wired = facts.extended("m:wired", plate, None, &any).unwrap();
        assert_eq!(wired.template, "minecraft:stone");
        let mat = facts.extended("m:mat", plate, None, &any).unwrap();
        assert_eq!(mat.template, "minecraft:oak_pressure_plate");
        // A game without the template: the next that fits.
        let stoneless = facts.extended("m:wired", None, None, &|n: &str| n != "minecraft:stone");
        assert_eq!(stoneless, None);
    }

    #[test]
    fn a_declared_block_says_how_its_shapes_differ() {
        let half = |k: &str| {
            if k.contains("tall=true") { json!([[0, 0, 0, 1, 1, 1]]) } else { json!([[0, 0, 0, 1, 0.5, 1]]) }
        };
        let facing = |k: &str| {
            if k.contains("facing=north") { json!([[0, 0, 0, 1, 1, 0.5]]) } else { json!([[0, 0, 0.5, 1, 1, 1]]) }
        };
        let tall_shape = |k: &str| {
            // Past the block's top, and a flat state: the game takes boxes inside the block and not flat.
            if k.contains("tall=true") { json!([[0, 0, 0, 1, 1.5, 1]]) } else { json!([[0, 0, 0, 0, 0, 0]]) }
        };
        let empty = |_: &str| json!([]);
        let tall: (&str, &[&str]) = ("tall", BOOL);
        let facts = family(json!({
            "m:half": with_props(&["m.Half", BLOCK], &[tall], "tall=false", &half),
            "m:panel": with_props(&["m.Panel", BLOCK], &[FACING, tall], "facing=north,tall=false", &facing),
            "m:odd": with_props(&["m.Odd", BLOCK], &[tall], "tall=false", &tall_shape),
            "m:ghost": with_props(&["m.Ghost", BLOCK], &[tall], "tall=false", &empty),
        }));
        let any = |_: &str| true;
        let c = facts.extended("m:half", None, None, &any).unwrap();
        assert_eq!(c.template, "minecraft:stone");
        assert_eq!(c.properties["no_occlusion"], json!(true));
        let half_box = json!([0.0, 0.0, 0.0, 1.0, 0.5, 1.0]);
        assert_eq!(c.declared["outline_boxes"]["tall=false"], half_box);
        assert_eq!(c.declared["collision_boxes"]["*"], json!([0.0, 0.0, 0.0, 1.0, 1.0, 1.0]));
        // On a template's property: boxes per value of it.
        let c = facts.extended("m:panel", None, None, &any).unwrap();
        assert_eq!(c.template, "minecraft:carved_pumpkin");
        assert_eq!(c.declared["shapes"]["property"], json!("facing"));
        assert_eq!(c.declared["shapes"]["values"]["south"], json!([[0, 0, 0.5, 1, 1, 1]]));
        assert_eq!(c.declared["shapes"]["values"].as_object().unwrap().len(), 4);
        let c = facts.extended("m:odd", None, None, &any).unwrap();
        // The flat state has no shape, and says so.
        assert_eq!(
            c.declared["outline_boxes"],
            json!({"*": [0.0, 0.0, 0.0, 1.0, 1.0, 1.0], "tall=false": NOTHING})
        );
        let c = facts.extended("m:ghost", None, None, &any).unwrap();
        assert_eq!(c.properties["outline_empty"], json!(true));
        assert_eq!(c.properties["no_collision"], json!(true));
        assert!(!c.declared.contains_key("outline_boxes"));
    }

    #[test]
    fn a_block_with_all_its_templates_states_only_moves_the_default() {
        let cube = |_: &str| json!([[0, 0, 0, 1, 1, 1]]);
        let pumpkin = "net.minecraft.world.level.block.CarvedPumpkinBlock";
        let facts = family(json!({"m:head": with_props(&[pumpkin, BLOCK], &[FACING], "facing=west", &cube)}));
        let any = |_: &str| true;
        let c = facts.extended("m:head", None, None, &any).unwrap();
        assert_eq!(c.template, "minecraft:carved_pumpkin");
        assert_eq!(Value::Object(c.declared), json!({"default_state": {"facing": "west"}}));
        assert_eq!(c.states, 4);
    }

    /// A block on a carved pumpkin, a block on a coal block and a plain item, with the game's items as they are.
    fn items_facts(pack: Value) -> Facts {
        let cube = |_: &str| json!([[0, 0, 0, 1, 1, 1]]);
        let carved: &[&str] = &["net.minecraft.world.level.block.CarvedPumpkinBlock", BLOCK];
        let item = |class: &str, stack: u8, rarity: &str| {
            json!({"class": class, "max_stack_size": stack, "rarity": rarity, "components": {"minecraft:max_stack_size": stack},
                "attribute_modifiers": {"modifiers": []}, "enchantment_value": 0})
        };
        let block_item = "net.minecraft.world.item.BlockItem";
        let mut facts: Facts = serde_json::from_value(json!({
            "blocks": {
                "minecraft:carved_pumpkin": with_props(carved, &[FACING], "facing=north", &cube),
                "m:machine": with_props(&["m.Machine", BLOCK], &[FACING, ("lit", BOOL)], "facing=north,lit=false", &cube),
            },
            "items": {
                "m:machine": item(block_item, 64, "common"),
                "m:coal": item(block_item, 16, "rare"),
                "m:gem": item("net.minecraft.world.item.Item", 64, "common"),
                "m:sawdust": item("net.minecraft.world.item.Item", 64, "common"),
                "m:hat": item(block_item, 64, "common"),
                "m:relic": item(block_item, 64, "m:relic"),
                "m:safe": {"class": block_item, "max_stack_size": 64, "rarity": "common",
                    "components": {"minecraft:fire_resistant": {}}},
            }
        }))
        .unwrap();
        facts.vanilla_items = serde_json::from_value(json!({
            "minecraft:carved_pumpkin": {"minecraft:max_stack_size": 64, "minecraft:rarity": "common",
                "minecraft:attribute_modifiers": [{"type": "minecraft:waypoint_transmit_range", "slot": "head"}],
                "minecraft:compostable": {"layers": "minecraft:compostable/medium"},
                "minecraft:equippable": {"camera_overlay": "minecraft:misc/pumpkinblur", "slot": "head", "swappable": false}},
            "minecraft:coal_block": {"minecraft:max_stack_size": 64, "minecraft:rarity": "common",
                "minecraft:attribute_modifiers": [],
                "minecraft:cooking_fuel": {"burn_time": "minecraft:cooking/time_coal_block",
                    "speed_multiplier": "minecraft:cooking/speed_default"}},
            "minecraft:stick": {"minecraft:max_stack_size": 64, "minecraft:rarity": "common",
                "minecraft:attribute_modifiers": [],
                "minecraft:cooking_fuel": {"burn_time": "minecraft:cooking/time_dry_plants",
                    "speed_multiplier": "minecraft:cooking/speed_default"}},
            "minecraft:netherite_block": {"minecraft:max_stack_size": 64, "minecraft:rarity": "common",
                "minecraft:attribute_modifiers": [],
                "minecraft:damage_resistant": {"types": "#minecraft:is_fire"}},
        }))
        .unwrap();
        facts.pack = serde_json::from_value(pack).unwrap();
        facts
    }

    #[test]
    fn a_block_item_carries_javas_facts_over_its_templates() {
        let facts = items_facts(json!({"items": {
            "m:machine": {"burn": 0},
            "m:coal": {"burn": 16000},
            "m:hat": {"burn": 0, "compost": 0.65, "worn": "head"},
            "m:relic": {"burn": 0},
            "m:safe": {"burn": 0},
        }}));
        // Nothing of a carved pumpkin's item stays on a machine: not the head slot, its modifier or its compost.
        let (patch, differences) = facts.block_item_components("m:machine", "minecraft:carved_pumpkin", &["m"]);
        assert_eq!(
            Value::Object(patch),
            json!({"!minecraft:equippable": {}, "!minecraft:compostable": {}, "minecraft:attribute_modifiers": []})
        );
        assert!(differences.is_empty());
        // A block that is worn and composts keeps what it shares with the template.
        let (patch, _) = facts.block_item_components("m:hat", "minecraft:carved_pumpkin", &["m"]);
        assert_eq!(Value::Object(patch), json!({"minecraft:attribute_modifiers": []}));
        // No fuel unless Java says so; the same time keeps the template's; stack size and rarity go in the patch.
        let (patch, _) = facts.block_item_components("m:machine", "minecraft:coal_block", &["m"]);
        assert_eq!(Value::Object(patch), json!({"!minecraft:cooking_fuel": {}}));
        let (patch, _) = facts.block_item_components("m:coal", "minecraft:coal_block", &["m"]);
        assert_eq!(
            Value::Object(patch),
            json!({"minecraft:max_stack_size": 16, "minecraft:rarity": "rare"})
        );
        // A rarity the game hasn't is left and said.
        let (patch, differences) = facts.block_item_components("m:relic", "minecraft:coal_block", &["m"]);
        assert_eq!(Value::Object(patch), json!({"!minecraft:cooking_fuel": {}}));
        assert_eq!(differences, ["rarity m:relic (the template's stays)"]);
        // Fire resistance goes by Java's item.
        let (patch, _) = facts.block_item_components("m:safe", "minecraft:netherite_block", &["m"]);
        assert_eq!(Value::Object(patch), json!({}));
        let (patch, _) = facts.block_item_components("m:machine", "minecraft:netherite_block", &["m"]);
        assert_eq!(Value::Object(patch), json!({"!minecraft:damage_resistant": {}}));
        // An enchantment value on a stack of 64 (Apothic Enchanting gives every item one) enchants nothing.
        let mut facts = facts;
        facts.items.get_mut("m:machine").unwrap().enchantment_value = Some(1);
        let (patch, _) = facts.block_item_components("m:machine", "minecraft:netherite_block", &["m"]);
        assert_eq!(Value::Object(patch), json!({"!minecraft:damage_resistant": {}}));
    }

    #[test]
    fn an_item_burns_and_composts_as_java_says() {
        let facts = items_facts(json!({"items": {
            "m:gem": {"burn": 0},
            "m:sawdust": {"burn": 450, "compost": 0.35},
            "m:hat": {"burn": 1600, "worn": "chest"},
        }}));
        // A stick burns; a gem on a stick doesn't.
        let (gem, _) = facts.item("m:gem", &["m"]).unwrap();
        assert_eq!(gem["components"], json!({"!minecraft:cooking_fuel": {}}));
        // A stack of one on a stick's 64 (a pen, a bucket): the declaration's own key.
        let mut facts = facts;
        facts.items.get_mut("m:gem").unwrap().max_stack_size = 1;
        let (gem, _) = facts.item("m:gem", &["m"]).unwrap();
        assert_eq!(gem["max_stack_size"], json!(1));
        // A time 26.3 has no name for is the same provider with the number in it, the chance the nearest of five.
        let (dust, _) = facts.item("m:sawdust", &["m"]).unwrap();
        assert_eq!(dust["components"]["minecraft:cooking_fuel"]["burn_time"]["left"], json!(450));
        assert_eq!(
            dust["components"]["minecraft:cooking_fuel"]["burn_time"]["right"]["on_true"],
            json!("minecraft:cooking/fast_burn_time_reduction_factor")
        );
        assert_eq!(
            dust["components"]["minecraft:compostable"],
            json!({"layers": "minecraft:compostable/low"})
        );
        let (patch, _) = facts.block_item_components("m:hat", "minecraft:coal_block", &["m"]);
        assert_eq!(
            patch["minecraft:cooking_fuel"]["burn_time"],
            json!("minecraft:cooking/time_coal")
        );
        assert_eq!(patch["minecraft:equippable"], json!({"slot": "chest"}));
        // Armour whose material names no asset (a jetpack) still goes on its slot.
        let mut facts = facts;
        facts.items.get_mut("m:hat").unwrap().armor =
            Some(serde_json::from_value(json!({"slot": "chest"})).unwrap());
        let (patch, _) = facts.block_item_components("m:hat", "minecraft:coal_block", &["m"]);
        assert_eq!(patch["minecraft:equippable"], json!({"slot": "chest"}));
        // So does armour whose material has no asset to draw it with.
        facts.items.get_mut("m:hat").unwrap().armor =
            Some(serde_json::from_value(json!({"slot": "chest", "material": "other:jet"})).unwrap());
        let (patch, _) = facts.block_item_components("m:hat", "minecraft:coal_block", &["m"]);
        assert_eq!(patch["minecraft:equippable"], json!({"slot": "chest"}));
        // Without the pack's run the template's stand, as before.
        let bare = items_facts(json!({}));
        let (gem, _) = bare.item("m:gem", &["m"]).unwrap();
        assert!(gem.get("components").is_none());
    }

    #[test]
    fn pistons_do_with_a_block_what_java_does() {
        let cube = |_: &str| json!([[0, 0, 0, 1, 1, 1]]);
        let mut facts = family(json!({
            "m:machine": with_props(&["m.Machine", BLOCK], &[FACING], "facing=north", &cube),
            "m:plain": with_props(&["m.Plain", BLOCK], &[FACING], "facing=north", &cube),
            "m:pop": with_props(&["m.Pop", BLOCK], &[FACING], "facing=north", &cube),
        }));
        facts.pack = serde_json::from_value(json!({"blocks": {
            "minecraft:carved_pumpkin": {"push": "DESTROY"},
            "m:machine": {"push": "NORMAL", "block_entity": true},
            "m:plain": {"push": "NORMAL"},
            "m:pop": {"push": "DESTROY"},
        }}))
        .unwrap();
        let reaction = |id: &str| {
            let c = facts.extended(id, None, None, &|_| true).unwrap();
            assert_eq!(c.template, "minecraft:carved_pumpkin");
            c.properties.get("push_reaction").cloned()
        };
        assert_eq!(reaction("m:machine"), Some(json!("IMMOVEABLE")));
        assert_eq!(reaction("m:plain"), Some(json!("PUSH_PULL")));
        assert_eq!(reaction("m:pop"), None);
    }

    #[test]
    fn a_state_with_no_collision_has_none() {
        // As a pile of gold: one layer is walked through, in Java and here, and it is the default.
        let outline = |k: &str| {
            let n: f64 = k.trim_start_matches("layers=").parse().unwrap();
            json!([[0, 0, 0, 1, n / 8.0, 1]])
        };
        let mut pile = with_props(&["m.Pile", BLOCK], &[("layers", &["1", "2", "3"])], "layers=1", &outline);
        pile["states"]["layers=1"]["collision"] = json!([]);
        pile["states"]["layers=3"]["collision"] = json!([[0, 0, 0, 1, 0.25, 1]]);
        pile["states"]["layers=2"]["collision"] = json!([[0, 0, 0, 1, 0.125, 1]]);
        let facts = family(json!({"m:pile": pile}));
        let c = facts.extended("m:pile", None, None, &|_| true).unwrap();
        let boxes = &c.declared["collision_boxes"];
        assert_eq!(selected(boxes, "layers=1"), json!(NOTHING));
        assert_eq!(selected(boxes, "layers=2"), json!([0.0, 0.0, 0.0, 1.0, 0.125, 1.0]));
        assert_eq!(selected(boxes, "layers=3"), json!([0.0, 0.0, 0.0, 1.0, 0.25, 1.0]));
    }

    /// What the game takes for `key=value`: its selector, else `*`.
    fn selected(boxes: &Value, state: &str) -> Value {
        boxes.get(state).unwrap_or(&boxes["*"]).clone()
    }

    #[test]
    fn a_shape_that_cant_be_selected_is_still_the_blocks() {
        // As a pipe whose sides no model reads: not declared, so the core stands for all, not the template's cube.
        let sides: &[&str] = &["none", "normal"];
        let pipe = |k: &str| {
            if k.contains("normal") {
                json!([[0.3, 0.3, 0.3, 0.7, 0.7, 0.7], [0.3, 0.7, 0.3, 0.7, 1, 0.7]])
            } else {
                json!([[0.3, 0.3, 0.3, 0.7, 0.7, 0.7]])
            }
        };
        let core = json!([0.3, 0.3, 0.3, 0.7, 0.7, 0.7]);
        let props: &[(&str, &[&str])] = &[("down", sides), ("up", sides)];
        let facts = family(json!({
            "m:pipe": with_props(&["m.Pipe", BLOCK], props, "down=none,up=none", &pipe),
            // Its shape follows its facing and what it is doing: by facing, and the shape it has when idle.
            "m:press": with_props(&["m.Press", BLOCK], &[FACING, ("on", BOOL)], "facing=north,on=false", &|k: &str| {
                let y = if k.contains("on=true") { 1.0 } else { 0.5 };
                if k.contains("facing=north") { json!([[0, 0, 0, 1, y, 0.5]]) } else { json!([[0, 0, 0.5, 1, y, 1]]) }
            }),
        }));
        let none = BTreeSet::new();
        let c = facts.extended("m:pipe", None, Some(&none), &|_| true).unwrap();
        assert_eq!(c.template, "minecraft:stone");
        assert_eq!(c.states, 1);
        // With its sides at their defaults its one state has one shape, and it fits no selector: the core.
        assert_eq!(c.declared["outline_boxes"], json!({"*": core}));
        assert_eq!(c.declared["collision_boxes"], json!({"*": core}));
        assert_eq!(c.properties["no_occlusion"], json!(true));
        // Declaring only `on`, the template's facing moves it too.
        let named = BTreeSet::from(["on".to_owned()]);
        let c = facts.extended("m:press", None, Some(&named), &|_| true).unwrap();
        assert_eq!(c.template, "minecraft:carved_pumpkin");
        assert_eq!(c.declared["shapes"]["property"], json!("facing"));
        assert_eq!(c.declared["shapes"]["values"]["north"], json!([[0, 0, 0, 1, 0.5, 0.5]]));
        assert_eq!(c.declared["shapes"]["values"]["east"], json!([[0, 0, 0.5, 1, 0.5, 1]]));
        assert!(c.differences.iter().any(|d| d.starts_with("its shapes follow more than one")));
    }

    #[test]
    fn light_follows_each_state() {
        let cube = |_: &str| json!([[0, 0, 0, 1, 1, 1]]);
        let lit: (&str, &[&str]) = ("lit", BOOL);
        let lamp = "net.minecraft.world.level.block.RedstoneLampBlock";
        let furnace = "net.minecraft.world.level.block.FurnaceBlock";
        let mut dark = with_props(&["m.Dark", lamp, BLOCK], &[lit], "lit=false", &cube);
        dark["states"]["lit=true"]["emission"] = json!(0);
        let mut bright = with_props(&["m.BrightLamp", BLOCK], &[lit], "lit=false", &cube);
        bright["states"]["lit=true"]["emission"] = json!(7);
        let facts = family(json!({
            "minecraft:redstone_lamp": with_props(&[lamp, BLOCK], &[lit], "lit=false", &cube),
            "minecraft:furnace": with_props(&[furnace, BLOCK], &[FACING, lit], "facing=north,lit=false", &cube),
            "m:real_lamp": with_props(&["m.RealLamp", lamp, BLOCK], &[lit], "lit=false", &cube),
            "m:dark": dark,
            "m:wall_lamp": with_props(&["m.WallLampBlock", BLOCK], &[lit], "lit=false", &cube),
            "m:bright": bright,
            "m:on_lamp": with_props(&["m.OnLamp", BLOCK], &[lit], "lit=true", &cube),
            "m:fan": with_props(&["m.CeilingFan", BLOCK], &[lit], "lit=false", &cube),
            "m:trap": with_props(&["m.DartTrap", BLOCK], &[FACING, lit], "facing=north,lit=false", &cube),
            "m:iron_furnace": with_props(&["m.Base", BLOCK], &[FACING, lit], "facing=north,lit=false", &cube),
        }));
        let any = |_: &str| true;
        // Of its class, with the template's per-state light: no flat `light` to put it out.
        let c = facts.template("m:real_lamp", &any).unwrap();
        assert_eq!(c.template, "minecraft:redstone_lamp");
        assert!(!c.properties.contains_key("light"));
        // Java's own numbers: this lamp's class gives no light in either state, so it says that.
        let c = facts.template("m:dark", &any).unwrap();
        assert_eq!(c.properties["light"], json!(0));
        // A lamp by its name, starting off, in the template's light: redstone switches it, as in Java.
        let lamp_template = Some("minecraft:redstone_lamp");
        let c = facts.extended("m:wall_lamp", lamp_template, None, &any).unwrap();
        assert_eq!(c.template, "minecraft:redstone_lamp");
        assert!(!c.properties.contains_key("light") && !c.declared.contains_key("light_emission"));
        // Another light, or starting on: `lit` is its own, its light right in each state, and redstone leaves it.
        for id in ["m:bright", "m:on_lamp", "m:fan"] {
            let c = facts.extended(id, lamp_template, None, &any).unwrap();
            assert_eq!(c.template, "minecraft:stone", "{id}");
            assert!(c.declared["state_properties"].get("lit").is_some(), "{id}");
            assert!(c.declared.contains_key("light_emission"), "{id}");
            assert!(!c.properties.contains_key("light"), "{id}");
        }
        // A blockstate's `facing` and `lit` make no trap a furnace; what calls itself one may stand on it.
        let furnace_template = Some("minecraft:furnace");
        let c = facts.extended("m:trap", furnace_template, None, &any).unwrap();
        assert_eq!(c.template, "minecraft:carved_pumpkin");
        let c = facts.extended("m:iron_furnace", furnace_template, None, &any).unwrap();
        assert_eq!(c.template, "minecraft:furnace");
    }

    #[test]
    fn a_block_that_lets_light_through_does_so_on_a_solid_template() {
        let cube = |_: &str| json!([[0, 0, 0, 1, 1, 1]]);
        let thin = |_: &str| json!([[0.25, 0, 0.25, 0.75, 1, 0.75]]);
        let water: (&str, &[&str]) = ("waterlogged", BOOL);
        let mut facts = family(json!({
            "m:relay": with_props(&["m.Relay", BLOCK], &[("side", &["a", "b"])], "side=a", &thin),
            "m:wet": with_props(&["m.Wet", BLOCK], &[water], "waterlogged=false", &thin),
            "m:pane": with_props(&["m.Pane", BLOCK], &[], "", &cube),
        }));
        let any = |_: &str| true;
        // Without the pack's run it follows from the shape.
        let c = facts.extended("m:relay", None, None, &any).unwrap();
        assert_eq!(c.template, "minecraft:stone");
        assert_eq!(c.properties["no_occlusion"], json!(true));
        assert_eq!(c.properties["light_dampening"], json!(0));
        assert_eq!(c.properties["skylight_down"], json!(true));
        let c = facts.extended("m:wet", None, None, &any).unwrap();
        assert_eq!(c.declared["light_dampening_states"], json!({"waterlogged=true": 1}));
        facts.pack = serde_json::from_value(json!({"blocks": {
            "minecraft:stone": {"push": "NORMAL", "occludes": true, "dampening": 15},
            "m:relay": {"push": "NORMAL", "occludes": true, "dampening": 0, "skylight_down": true},
            "m:wet": {"push": "NORMAL", "occludes": false, "dampening": 0, "skylight_down": true},
            "m:pane": {"push": "NORMAL", "occludes": false, "dampening": 1},
        }}))
        .unwrap();
        // With it Java's numbers say, for any block: a full cube that doesn't occlude, on a template that does.
        let c = facts.extended("m:relay", None, None, &any).unwrap();
        assert_eq!(c.properties["light_dampening"], json!(0));
        assert_eq!(c.properties["skylight_down"], json!(true));
        let c = facts.extended("m:wet", None, None, &any).unwrap();
        assert_eq!(c.declared["light_dampening_states"], json!({"waterlogged=true": 1}));
        let c = facts.template("m:pane", &any).unwrap();
        assert_eq!(c.template, "minecraft:stone");
        assert_eq!(c.properties["no_occlusion"], json!(true));
        assert_eq!(c.properties["light_dampening"], json!(1));
        assert!(!c.properties.contains_key("skylight_down"));
    }

    #[test]
    fn state_keys_sort() {
        assert_eq!(
            state_key("half=lower,facing=north"),
            "facing=north,half=lower"
        );
        assert_eq!(state_key(""), "");
    }
}
