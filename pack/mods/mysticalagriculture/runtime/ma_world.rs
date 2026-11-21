// Mystical Agriculture in the game (support module `crate::game::ma_world` of the `mysticalagriculture` native
// bundle). The rules are the bundle's crate `ma_core`, translated from MA's MIT source; this module is the glue:
// the crops' random ticks, bone meal and harvest, the Growth Accelerators, and the infusion altar with its
// pedestals (their block entities, the right clicks, the recipe check on the world's `mysticalagriculture:infusion`
// recipes). Settings come from ATM10's `config/mysticalagriculture-common.toml` and MysticalCustomization's crops
// and tiers (`config/mysticalcustomization/`), read when the server opens its world (`server_started`). Server hooks
// run on the thread of the `Game` that simulates the world, so the state is kept per thread. The hooks hand the API's
// block entity (`mod_api::ModBlockEntity`, slots of `mod_api::ItemStack`); the levels' maps hold the engine's.

use super::Game;
use crate::game::api_values::{TryIntoApi, TryIntoEngine};
use horizon_client::{containers::ModContainer, first_person::Hand, items::ItemStack};
use horizon_core::{
    nbt::{Compound, JavaString, Tag},
    random::RandomSource,
};
use horizon_world::level::flags;
use mod_api::{LootDrops, ModBlockEntity, WorldPhase};
use ma_core::{
    accelerator,
    config::{self, Config},
    crop_table::Accepts,
    crops::{self, Crop, Soil},
    infusion::{self, Altar, Step},
    ingredient,
};
use serde_json::Value;
use std::{cell::RefCell, collections::HashSet, rc::Rc, time::Instant};

const WAND: &str = "mysticalagriculture:wand";
const FERTILIZED: &str = "mysticalagriculture:fertilized_essence";

struct State {
    config: Config,
    custom: Vec<Crop>,
}

#[derive(Default)]
struct Book {
    stamp: (usize, usize),
    recipes: Vec<Rc<infusion::Recipe<ItemStack>>>,
    skipped: usize,
    /// The first few recipes left out, for the log.
    left_out: Vec<String>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    static BOOK: RefCell<Book> = RefCell::new(Book::default());
    static LAST_CLICK: RefCell<Option<(u128, [i32; 3], Instant)>> = const { RefCell::new(None) };
    static ROLL: RefCell<u64> = RefCell::new(seed());
}

fn seed() -> u64 {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64);
    nanos | 1
}

/// `Math.random()`: MA's drops don't draw from the level's random.
fn math_random() -> f64 {
    ROLL.with(|r| {
        let mut x = *r.borrow();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *r.borrow_mut() = x;
        (x >> 11) as f64 / (1u64 << 53) as f64
    })
}

fn json_files(dir: &std::path::Path) -> Vec<(String, Value)> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<(String, Value)> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let stem = path.file_stem()?.to_str()?.to_owned();
            (path.extension()? == "json").then_some(())?;
            let json = serde_json::from_slice(&std::fs::read(&path).ok()?).ok()?;
            Some((stem, json))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn read_state() -> State {
    let Ok(dir) = Game::native_config_dir() else {
        return State { config: Config::default(), custom: Vec::new() };
    };
    let config = std::fs::read_to_string(dir.join("mysticalagriculture-common.toml")).map_or_else(|_| Config::default(), |t| config::parse(&t));
    let custom_dir = dir.join("mysticalcustomization");
    let tiers: Vec<_> = json_files(&custom_dir.join("tiers"))
        .iter()
        .filter_map(|(stem, json)| crops::custom_tier(&format!("mysticalcustomization:{stem}"), json))
        .collect();
    let custom: Vec<Crop> = json_files(&custom_dir.join("crops")).iter().filter_map(|(stem, json)| crops::custom_crop(stem, json, &tiers)).collect();
    log::info!("[mysticalagriculture] {} custom crops, {} custom tiers, {:?}", custom.len(), tiers.len(), config);
    State { config, custom }
}

fn with_state<R>(read: impl FnOnce(&State) -> R) -> R {
    STATE.with(|s| read(s.borrow_mut().get_or_insert_with(read_state)))
}

/// `native_server_started`: settings and the pack's custom crops are read as the server opens its world.
pub(super) fn opened() {
    let fresh = read_state();
    STATE.with(|s| *s.borrow_mut() = Some(fresh));
    BOOK.with(|b| *b.borrow_mut() = Book::default());
}

/// `native_world_lifecycle`: settings and the recipe book are dropped as the world closes or fails to open.
pub(super) fn lifecycle(phase: WorldPhase) {
    if matches!(phase, WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed | WorldPhase::WorldOpenFailed) {
        STATE.with(|s| *s.borrow_mut() = None);
        BOOK.with(|b| *b.borrow_mut() = Book::default());
    }
}

/// The crop a block is: MA's own, or one of the pack's MysticalCustomization crops.
pub(super) fn crop_of(block: &str) -> Option<Crop> {
    if !block.starts_with("mysticalagriculture:") || !block.ends_with("_crop") {
        return None;
    }
    crops::builtin(block).or_else(|| with_state(|s| s.custom.iter().find(|c| c.block() == block).cloned()))
}

fn below(pos: [i32; 3], n: i32) -> [i32; 3] {
    [pos[0], pos[1] - n, pos[2]]
}

// ---- Crops ----------------------------------------------------------------

/// `native_random_tick` of a crop (`MysticalCropBlock.randomTick`): one that can't grow here takes no random tick;
/// one that can grows as its wheat template does (`CropBlock.randomTick`).
pub(super) fn random_tick(block: &str, below: &str, below2: &str, handled: &mut bool) {
    if let Some(crop) = crop_of(block) {
        let requires = with_state(|s| s.config.crops.requires_effective_farmland);
        if !crop.can_grow(below, below2, requires) {
            *handled = true;
        }
    }
}

/// `native_may_place_on` (`CropBlock.mayPlaceOn`: `instanceof FarmBlock`): vanilla's farmland, MA's essence
/// farmland, and other mods' farmland by name (the engine doesn't say which mod blocks are a `FarmBlock`).
pub(super) fn may_place_on(block: &str, ground: &str, may: &mut Option<bool>) {
    if crop_of(block).is_some() {
        *may = Some(ground == "minecraft:farmland" || crops::essence_farmland_tier(ground).is_some() || ground.ends_with("_farmland"));
    }
}

/// `native_is_valid_bonemeal_target` (`MysticalCropBlock.isValidBonemealTarget`): where it can grow and isn't ripe.
pub(super) fn valid_bonemeal_target(block: &str, age: Option<&str>, below: &str, below2: &str, valid: &mut Option<bool>) {
    let Some(crop) = crop_of(block) else { return };
    let requires = with_state(|s| s.config.crops.requires_effective_farmland);
    let ripe = age.and_then(|a| a.parse::<u32>().ok()).is_some_and(|a| a >= crops::MAX_AGE);
    *valid = Some(crop.can_grow(below, below2, requires) && !ripe);
}

/// `native_is_bonemeal_success`: never (`MysticalCropBlock.isBonemealSuccess`); bone meal is used up on MA's crops.
pub(super) fn bonemeal_success(block: &str, success: &mut bool) {
    if crop_of(block).is_some() {
        *success = false;
    }
}

/// `native_loot_drops` of a crop broken: `MysticalCropBlock.getDrops` (MA's crops have no loot table). The broken
/// state is the API's: its block and `age` are read through the block metadata.
pub(super) fn loot_drops(game: &Game<'_>, context: &LootDrops<'_>, drops: &mut Vec<ItemStack>) {
    let Some((_, state)) = context.block else { return };
    let Ok(block) = game.block_metadata_name(state.block()).map(str::to_owned) else { return };
    let Some(crop) = crop_of(&block) else { return };
    let age = game.block_metadata_property((&state).into(), "age").ok().flatten().and_then(|a| a.parse::<u32>().ok()).unwrap_or(0);
    let origin = context.origin.map(|v| v.floor() as i32);
    let under = game.native_block(below(origin, 1));
    let settings = with_state(|s| s.config.crops);
    let d = crops::drops(&crop, age, Some(&under), &settings, math_random);
    drops.clear();
    if d.essence > 0 {
        drops.push(ItemStack::new(&crop.essence(), d.essence));
    }
    drops.push(ItemStack::new(&crop.seeds(), d.seeds));
    if d.fertilized {
        drops.push(ItemStack::new(FERTILIZED, 1));
    }
}

/// A crop's `randomTick` from a Growth Accelerator: `canGrow`, then `CropBlock.randomTick` (light 9 or more, the
/// growth speed's odds), drawing from the level's random.
fn grow_by_accelerator(game: &mut Game<'_>, pos: [i32; 3], crop: &Crop) {
    let (requires, block) = (with_state(|s| s.config.crops.requires_effective_farmland), crop.block());
    if !crop.can_grow(&game.native_block(below(pos, 1)), &game.native_block(below(pos, 2)), requires) {
        return;
    }
    let (block_light, sky) = game.native_light(pos);
    if block_light.max(sky) < 9 {
        return;
    }
    let age = game.native_block_property(pos, "age").and_then(|a| a.parse::<u32>().ok()).unwrap_or(0);
    if age >= crops::MAX_AGE {
        return;
    }
    let mut soil = [Soil::default(); 9];
    for (i, s) in soil.iter_mut().enumerate() {
        let at = [pos[0] + (i % 3) as i32 - 1, pos[1] - 1, pos[2] + (i / 3) as i32 - 1];
        let name = game.native_block(at);
        s.farmland = name == "minecraft:farmland" || crops::essence_farmland_tier(&name).is_some();
        s.moist = s.farmland && game.native_block_property(at, "moisture").is_some_and(|m| m != "0");
    }
    let offsets = [[-1, 0], [1, 0], [0, -1], [0, 1], [-1, -1], [1, -1], [1, 1], [-1, 1]];
    let same = offsets.map(|[dx, dz]| game.native_block([pos[0] + dx, pos[1], pos[2] + dz]) == block);
    let bound = crops::growth_bound(crops::growth_speed(&soil, &same));
    // The level's own random, drawn from a copy that is stored back (`level_random` / `level_set_random`).
    let Some(level) = game.fork_level_here() else { return };
    let Some(mut random) = game.level_random(level) else { return };
    let roll = random.next_int_bound(bound).unwrap_or(1);
    game.level_set_random(level, random);
    if roll == 0 {
        game.native_set_block_property_with(pos, "age", &(age + 1).to_string(), flags::UPDATE_CLIENTS);
    }
}

// ---- Block entities -------------------------------------------------------

const PROGRESS: &str = "Progress";
const ACTIVE: &str = "Active";
const COOLDOWN: &str = "Cooldown";

fn key(name: &str) -> JavaString {
    JavaString::from(name)
}

fn int(nbt: &Compound, name: &str) -> i32 {
    match nbt.get(&key(name)) {
        Some(Tag::Int(v)) => *v,
        Some(Tag::Byte(v)) => i32::from(*v),
        _ => 0,
    }
}

fn read_altar(nbt: &Compound) -> Altar {
    Altar { progress: int(nbt, PROGRESS), active: int(nbt, ACTIVE) != 0 }
}

fn write_altar(nbt: &mut Compound, a: &Altar) {
    nbt.insert(key(PROGRESS), Tag::Int(a.progress));
    nbt.insert(key(ACTIVE), Tag::Byte(i8::from(a.active)));
}

/// `block_entity_types`: the altar, pedestals and growth accelerators, one type per block and named after it.
pub(super) fn block_entity_types(types: &mut Vec<(String, Vec<String>)>) {
    for block in [infusion::ALTAR, infusion::PEDESTAL] {
        types.push((block.to_owned(), vec![block.to_owned()]));
    }
    for tier in ["inferium", "prudentium", "tertium", "imperium", "supremium", "awakened_supremium"] {
        let block = format!("{}:{tier}_growth_accelerator", crops::NS);
        debug_assert!(accelerator::range(&block).is_some());
        types.push((block.clone(), vec![block]));
    }
}

/// `native_block_entity`: the altar and the pedestals (Cucumber's `BaseInventoryTileEntity`), and a Growth
/// Accelerator's timer (Java schedules a block tick instead; a block entity starts on any placement here). The
/// declaration owns the type id.
pub(super) fn new_entity(block: &str, entity: &mut Option<Compound>) {
    if entity.is_some() {
        return;
    }
    let mut nbt = Compound::new();
    if block == infusion::ALTAR {
        write_altar(&mut nbt, &Altar::default());
    } else if block != infusion::PEDESTAL && accelerator::range(block).is_some() {
        let cooldown = with_state(|s| s.config.growth_accelerator_cooldown);
        nbt.insert(key(COOLDOWN), Tag::Int(accelerator::tick_rate(cooldown, math_random())));
    } else if block != infusion::PEDESTAL {
        return;
    }
    *entity = Some(nbt);
}

/// `native_block_entity_ticks`: the altar and the accelerators tick, and the pedestals too (their tick does nothing;
/// the engine reaches a mod block entity's slots from outside it only on a ticking one).
pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if block == infusion::ALTAR || block == infusion::PEDESTAL || accelerator::range(block).is_some() {
        *ticks = true;
    }
}

/// `native_block_entity_container`: the altar's input (0) and output (1), a pedestal's one slot, every face
/// (`RegisterCapabilityHandler`: the whole inventory on each side). No menu: MA's altar has no screen.
pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if container.is_some() {
        return;
    }
    let (slots, inputs, outputs) = match block {
        infusion::ALTAR => (2, vec![0], vec![1]),
        infusion::PEDESTAL => (1, vec![0], vec![0]),
        _ => return,
    };
    let all: Vec<usize> = (0..slots).collect();
    *container = Some(ModContainer { slots, faces: std::array::from_fn(|_| all.clone()), inputs, outputs, menu: None });
}

/// `native_block_entity_slot_accepts`: nothing goes into the altar's output slot (Cucumber's `setOutputSlots`).
pub(super) fn slot_accepts(block: &str, slot: usize, accepts: &mut bool) {
    if block == infusion::ALTAR && slot == 1 {
        *accepts = false;
    }
}

// ---- Recipe load conditions -----------------------------------------------------------------------------------

/// One of MA's recipe load conditions (`ma_core::conditions`).
struct LoadCondition(&'static str);

impl mod_api::LoadCondition for LoadCondition {
    fn test(&self, args: &mod_api::serde_json::Value) -> Result<bool, mod_api::ConditionError> {
        ma_core::conditions::test(self.0, args).map_err(mod_api::ConditionError::new)
    }
}

/// `register_load_conditions`: MA's conditions under its namespace (`ModConditionSerializers`), so its recipe files
/// load instead of refusing the data packs. The registrar names the owner from the native mod dispatch scope, which
/// only a trait mod's dispatch opens: the bundle opens its own for the call.
pub(super) fn register_conditions(conditions: &mut dyn mod_api::LoadConditions) {
    let _scope = mod_api::content::crash::enter_native_mod(mod_api::content::crash::ModCall {
        id: crops::NS,
        hook: "register_load_conditions",
        part: None,
    });
    let mut registered = 0;
    for name in ma_core::conditions::NAMES {
        let Ok(id) = mod_api::Identifier::parse(&format!("{}:{name}", crops::NS)) else { continue };
        match conditions.register(id, LoadCondition(name)) {
            Ok(()) => registered += 1,
            Err(error) => log::warn!("[mysticalagriculture] load condition {name}: {}", error.message),
        }
    }
    log::info!("[mysticalagriculture] {registered} load conditions registered");
}

// ---- The recipes ------------------------------------------------------------

/// An ingredient's JSON as MA's 1.21.1 recipes write it, resolved to the items it accepts: `{item}`, `{tag}`, a
/// list, MA's own `mysticalagriculture:crop_component` (a crop's essence, seed or material, from its crop table) or
/// NeoForge's shapes as the engine's own recipe loader reads them. One that takes no item here (an unknown crop, an
/// absent item, an empty tag) is none, as the recipe could never match.
fn items_of(game: &Game<'_>, v: &Value) -> Option<HashSet<String>> {
    let one = |s: String| game.native_recipe_ingredient(&Value::String(s)).map(|i| i.items);
    if let Some(read) = ingredient::read(v) {
        let accepts = read.and_then(|c| ingredient::resolve(&c));
        let items = match accepts {
            Ok(Accepts::Item(id)) => one(id),
            Ok(Accepts::Tag(tag)) => one(format!("#{tag}")),
            Err(error) => {
                log::warn!("[mysticalagriculture] {error}");
                None
            }
        };
        return items.filter(|items| !items.is_empty());
    }
    match v {
        Value::String(s) => one(s.clone()),
        Value::Array(list) => {
            let all: HashSet<String> = list.iter().filter_map(|c| items_of(game, c)).flatten().collect();
            (!all.is_empty()).then_some(all)
        }
        Value::Object(o) => {
            if let Some(item) = o.get("item").and_then(Value::as_str) {
                return one(item.to_owned());
            }
            if let Some(tag) = o.get("tag").and_then(Value::as_str) {
                return one(format!("#{tag}"));
            }
            let parsed = horizon_client::crafting::Ingredient::parse(v, game.items())?;
            (!parsed.items.is_empty()).then_some(parsed.items)
        }
        _ => None,
    }
}

/// The world's infusion recipes, read again when the table changes.
fn recipes(game: &Game<'_>) -> Vec<Rc<infusion::Recipe<ItemStack>>> {
    let table = game.native_recipes_of_type(infusion::TYPE);
    let stamp = (table.as_ptr() as usize, table.len());
    if let Some(r) = BOOK.with(|b| (b.borrow().stamp == stamp && !b.borrow().recipes.is_empty()).then(|| b.borrow().recipes.clone())) {
        return r;
    }
    let mut book = Book { stamp, ..Book::default() };
    // The world's table holds only some of the files (2 of the carry's 123: the ones in sub-folders such as
    // `recipe/augment/` are missing), so the data files of the type are read too, once per world.
    let mut all: Vec<(String, Value)> = table.iter().map(|r| (r.id.clone(), (*r.json).clone())).collect();
    for path in game.native_data_files("recipe") {
        let Some(rest) = path.strip_prefix("data/") else { continue };
        let Some((ns, file)) = rest.split_once("/recipe/") else { continue };
        let id = format!("{ns}:{}", file.trim_end_matches(".json"));
        if all.iter().any(|(i, _)| *i == id) {
            continue;
        }
        let Some(json) = game.native_data(&path).and_then(|b| serde_json::from_slice::<Value>(&b).ok()) else { continue };
        if json.get("type").and_then(Value::as_str) == Some(infusion::TYPE) {
            all.push((id, json));
        }
    }
    for (id, json) in &all {
        let read = infusion::read(json).ok().and_then(|j| {
            Some(infusion::Recipe {
                id: id.clone(),
                input: items_of(game, &j.input)?,
                ingredients: j.ingredients.iter().map(|i| items_of(game, i)).collect::<Option<_>>()?,
                result: game.native_recipe_result(&j.result)?,
                transfer_components: j.transfer_components,
            })
        });
        match read {
            Some(recipe) => book.recipes.push(Rc::new(recipe)),
            None => {
                book.skipped += 1;
                if book.left_out.len() < 8 {
                    book.left_out.push(id.clone());
                }
            }
        }
    }
    log::info!("[mysticalagriculture] {} infusion recipes, {} left out {:?}", book.recipes.len(), book.skipped, book.left_out);
    let out = book.recipes.clone();
    BOOK.with(|b| *b.borrow_mut() = book);
    out
}

/// The counts of infusion recipes read and left out, for `/mysticalagriculture probe`.
pub(super) fn recipe_counts(game: &Game<'_>) -> (usize, usize) {
    recipes(game);
    BOOK.with(|b| (b.borrow().recipes.len(), b.borrow().skipped))
}

/// The pedestals around the altar at `pos` that are there, in `PEDESTALS`' order, with their items.
fn pedestals(game: &Game<'_>, pos: [i32; 3]) -> Vec<([i32; 3], Option<ItemStack>)> {
    infusion::PEDESTALS
        .iter()
        .map(|[dx, dy, dz]| [pos[0] + dx, pos[1] + dy, pos[2] + dz])
        .filter_map(|at| {
            let m = game.native_mod_block_entity(at)?;
            (m.id() == infusion::PEDESTAL).then(|| (at, m.items.first().cloned().flatten()))
        })
        .collect()
}

/// `Item.getCraftingRemainder` of one item of `stack` (with the mods' `crafting_remainder` hook).
fn remainder(game: &Game<'_>, stack: &ItemStack) -> Option<ItemStack> {
    let mut one = stack.clone();
    one.count = 1;
    game.crafting_remainder(&one)
}

/// `InfusionAltarTileEntity.tick`. The altar's own slots hold the API's stacks: they are read as the engine's.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    if let Some(range) = accelerator::range(entity.id()) {
        return accelerator_tick(game, pos, range, entity);
    }
    if entity.id() != infusion::ALTAR {
        return;
    }
    let input = entity.items.first().cloned().flatten().and_then(|s| s.try_into_engine(game.items()).ok());
    let before = read_altar(&entity.nbt);
    let mut altar = before;
    let signal = input.is_some() && !altar.active && game.native_neighbor_signal(game.native_dimension(), pos) > 0;
    let around = pedestals(game, pos);
    let mut found = None;
    let step = altar.tick(input.is_some(), || signal, || {
        let altar_item = input.as_ref().map_or("", |s| &*s.item);
        let items: Vec<Option<&str>> = around.iter().map(|(_, s)| s.as_ref().map(|s| &*s.item)).collect();
        found = recipes(game).into_iter().find(|r| r.matches(altar_item, &items));
        found.is_some()
    });
    if step == Step::Craft
        && let Some(recipe) = found
    {
        for (at, stack) in &around {
            let rest = stack.as_ref().and_then(|s| remainder(game, s));
            game.native_with_mod_block_entity(*at, |m| {
                if let Some(slot) = m.items.first_mut() {
                    *slot = rest;
                }
            });
        }
        let rest = input.as_ref().and_then(|s| remainder(game, s));
        if entity.items.len() >= 2 {
            entity.items[0] = rest.and_then(|r| r.try_into_api().ok());
            entity.items[1] = recipe.result.try_into_api().ok();
        }
        log::info!("[mysticalagriculture] infusion {} at {pos:?}: {} x{}", recipe.id, recipe.result.item, recipe.result.count);
    }
    if altar != before {
        write_altar(&mut entity.nbt, &altar);
    }
    entity.data.clear();
    entity.data.extend([altar.progress, i32::from(altar.active)]);
}

/// A Growth Accelerator's timer: when it runs out, the first block within reach that is one of MA's crops takes a
/// random tick (`GrowthAcceleratorBlock.tick`; Java ticks any `BonemealableBlock`, only MA's crops are reached here).
fn accelerator_tick(game: &mut Game<'_>, pos: [i32; 3], range: i32, entity: &mut ModBlockEntity) {
    let left = int(&entity.nbt, COOLDOWN) - 1;
    if left > 0 {
        entity.nbt.insert(key(COOLDOWN), Tag::Int(left));
        return;
    }
    let cooldown = with_state(|s| s.config.growth_accelerator_cooldown);
    entity.nbt.insert(key(COOLDOWN), Tag::Int(accelerator::tick_rate(cooldown, math_random())));
    for at in accelerator::reach(pos, range) {
        let name = game.native_block(at);
        if let Some(crop) = crop_of(&name) {
            grow_by_accelerator(game, at, &crop);
            return;
        }
    }
}

// ---- Right clicks -----------------------------------------------------------

/// `InfusionAltarBlock.useItemOn` and `InfusionPedestalBlock.useItemOn`, before the block or the item acts: the
/// held item goes on (one), or what is on comes off to the player; the altar gives its output first. The wand on an
/// altar with an item activates it (`WandItem.useOn`). Each answers the main hand; the off hand's click is taken
/// too, as Java's `SUCCESS` on the main hand ends the use. Sneaking with an item in either hand skips the block, as
/// vanilla's `isSecondaryUseActive`.
pub(super) fn right_click(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if *canceled {
        return;
    }
    let block = game.native_block(pos);
    if block != infusion::ALTAR && block != infusion::PEDESTAL {
        return;
    }
    if game.native_sneaking() && game.native_used_stack().is_some() {
        return;
    }
    *canceled = true;
    if matches!(game.hand, Hand::Off) {
        return;
    }
    let player = game.native_acting_id();
    let now = Instant::now();
    let repeat = LAST_CLICK.with(|c| {
        let mut c = c.borrow_mut();
        let again = c.is_some_and(|(p, at, when)| p == player && at == pos && now.duration_since(when).as_millis() < 100);
        *c = Some((player, pos, now));
        again
    });
    if repeat {
        return;
    }
    let held = game.native_used_stack().clone();
    let slots = game.native_mod_block_entity(pos).map(|m| m.items.clone()).unwrap_or_default();
    let (input, output) = (slots.first().cloned().flatten(), slots.get(1).cloned().flatten());
    let mut take = None;
    if block == infusion::ALTAR && output.is_some() {
        take = Some(1);
    } else if input.is_none() {
        if let Some(mut hand) = held {
            let mut one = hand.clone();
            one.count = 1;
            let put = game.native_with_mod_block_entity(pos, |m| m.items.first_mut().map(|s| *s = Some(one)).is_some());
            if put == Some(true) {
                hand.count -= 1;
                *game.native_used_stack() = (hand.count > 0).then_some(hand);
            } else {
                log::warn!("[mysticalagriculture] no slot to put on at {pos:?}");
            }
        }
    } else if block == infusion::ALTAR && held.as_ref().is_some_and(|h| &*h.item == WAND) {
        game.native_with_mod_block_entity(pos, |m| {
            let mut a = read_altar(&m.nbt);
            a.active = true;
            write_altar(&mut m.nbt, &a);
        });
    } else {
        take = Some(0);
    }
    if let Some(slot) = take {
        let Some(stack) = game.native_with_mod_block_entity(pos, |m| m.items.get_mut(slot).and_then(Option::take)).flatten() else { return };
        if let Some(rest) = game.native_give_stack(player, stack) {
            game.native_pop_resource(pos, rest);
        }
    }
}

/// The block at `pos` in the world, for the hooks that hold a `GameLevel` (the random tick and bone meal hooks read
/// below through it): the level's `LevelHost` reads.
pub(super) fn level_block(level: &super::GameLevel<'_>, pos: [i32; 3]) -> String {
    mod_api::LevelHost::block_state(level, pos)
        .and_then(|state| mod_api::LevelHost::state_block(level, state))
        .unwrap_or("minecraft:air")
        .to_owned()
}
