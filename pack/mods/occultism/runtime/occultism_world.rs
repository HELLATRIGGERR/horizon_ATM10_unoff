// Occultism in the game (support module `crate::game::occultism_world` of the `occultism` native bundle): spirit
// fire, the sacrificial bowls and the golden bowl's rituals. The rules are the bundle's crate `occultism_core`; this
// module is the glue: block entities and their saved compounds, the bowls' slots, the clicks, the world reads.
// Translated from Occultism 1.224.6 (MIT, Copyright 2020 klikli-dev; see this bundle's LICENSE-occultism):
// `SpiritFireBlock`, `PlayerEventHandler.spiritFire`, `SacrificialBowlBlock`, `GoldenSacrificialBowlBlockEntity`,
// `Ritual` and `CraftRitual`. Server hooks run on the thread of the `Game` that simulates the world, so the parsed
// recipes are kept per thread, read on first use and dropped when the server opens its world or closes it. The golden
// bowl's tick hands the API's block entity (the API's stacks); the other bowls are read and written in the level as
// the engine's (`native_with_mod_block_entity`).

use super::Game;
use crate::game::api_values::{TryIntoApi, TryIntoEngine};
use horizon_client::{
    containers::{ModBlockEntity, ModContainer},
    items::ItemStack,
    text::Text,
};
use horizon_core::nbt::{Compound, JavaString, Tag};
use mod_api::{BlockUpdateFlags, SoundSource, WorldPhase};
use occultism_core::{
    Ingredient, Pentacle, RitualRecipe, SpiritFireRecipe,
    pentacle::Blocks,
    ritual::{self, DEFAULT_DURATION_MULTIPLIER, ITEM_USE_DETECTION_RANGE_SQUARE},
    spirit_fire,
};
use std::{cell::RefCell, collections::HashMap};

const BOWL: &str = "occultism:sacrificial_bowl";
/// Slots of a golden bowl: 0 the activation item, then the ingredients taken so far (`consumedIngredients`).
const GOLDEN_SLOTS: usize = 1 + 32;
// The golden bowl's saved keys, as `GoldenSacrificialBowlBlockEntity.saveNetwork` names them.
const RITUAL: &str = "currentRitual";
const TIME: &str = "currentTime";
const ACTIVE: &str = "ritualActive";
const SACRIFICED: &str = "sacrificeProvided";
const ITEM_USED: &str = "requiredItemUsed";
const CASTER: &str = "castingPlayerId";
/// The port's own: which of the recipe's ingredients are still to be taken (`remainingAdditionalIngredients`).
const REMAINING: &str = "remainingIngredients";

struct Cache {
    rituals: Vec<RitualRecipe>,
    fire: Vec<SpiritFireRecipe>,
    pentacles: HashMap<String, Pentacle>,
}

thread_local! {
    static CACHE: RefCell<Option<std::rc::Rc<Cache>>> = const { RefCell::new(None) };
    /// Item entities that touched a mod block this tick (`native_block_entity_inside`): level, entity, block.
    static INSIDE: RefCell<Vec<(String, u32, [i32; 3])>> = const { RefCell::new(Vec::new()) };
    /// Golden bowls with a ritual running, by level: where an item use or a sacrifice is heard.
    static ACTIVE_BOWLS: RefCell<Vec<(String, [i32; 3])>> = const { RefCell::new(Vec::new()) };
    /// The last click on a block (level time, position, taken): a second one in the same tick is the off hand's,
    /// which Java never gets once the main hand's took the click.
    static LAST_CLICK: RefCell<Option<(i64, [i32; 3], bool)>> = const { RefCell::new(None) };
}

fn key(name: &str) -> JavaString {
    JavaString::from(name)
}

/// `native_server_started` (where the fork's `WorldOpenReady` was) and `native_world_lifecycle` v2: the parsed
/// recipes and pentacles, the running bowls and the items seen in fire are dropped.
pub(super) fn lifecycle(phase: Option<WorldPhase>) {
    let _ = phase;
    CACHE.with(|c| *c.borrow_mut() = None);
    ACTIVE_BOWLS.with(|a| a.borrow_mut().clear());
    INSIDE.with(|i| i.borrow_mut().clear());
}

fn cache(game: &Game<'_>) -> std::rc::Rc<Cache> {
    if let Some(c) = CACHE.with(|c| c.borrow().clone()) {
        return c;
    }
    let mut rituals = Vec::new();
    for r in game.native_recipes_of_type("occultism:ritual").iter() {
        match RitualRecipe::parse(&r.id, &r.json) {
            Ok(recipe) => rituals.push(recipe),
            Err(e) => log::warn!("[occultism] ritual {e}"),
        }
    }
    let fire: Vec<SpiritFireRecipe> = game
        .native_recipes_of_type("occultism:spirit_fire")
        .iter()
        .filter_map(|r| SpiritFireRecipe::parse(&r.id, &r.json).map_err(|e| log::warn!("[occultism] {e}")).ok())
        .collect();
    let mut pentacles = HashMap::new();
    for path in game.native_data_files("modonomicon/multiblocks") {
        // data/<ns>/modonomicon/multiblocks/<id>.json
        let Some((ns, rest)) = path.strip_prefix("data/").and_then(|p| p.split_once('/')) else { continue };
        let Some(name) = rest.strip_prefix("modonomicon/multiblocks/").and_then(|n| n.strip_suffix(".json")) else { continue };
        let id = format!("{ns}:{name}");
        let parsed = game.native_data(&path).and_then(|b| serde_json::from_slice(&b).ok());
        match parsed.map(|json| Pentacle::parse(&id, &json)) {
            Some(Ok(p)) => {
                pentacles.insert(id, p);
            }
            Some(Err(e)) => log::warn!("[occultism] pentacle {e}"),
            None => log::warn!("[occultism] pentacle {path} doesn't read"),
        }
    }
    log::info!("[occultism] {} rituals, {} spirit fire recipes, {} pentacles", rituals.len(), fire.len(), pentacles.len());
    let c = std::rc::Rc::new(Cache { rituals, fire, pentacles });
    CACHE.with(|slot| *slot.borrow_mut() = Some(c.clone()));
    c
}

/// The world as a pentacle reads it.
struct World<'a, 'g>(&'a Game<'g>);
impl Blocks for World<'_, '_> {
    fn block(&self, pos: [i32; 3]) -> String {
        self.0.native_block(pos)
    }
    fn property(&self, pos: [i32; 3], key: &str) -> Option<String> {
        self.0.native_block_property(pos, key)
    }
    fn in_tag(&self, block: &str, tag: &str) -> bool {
        self.0.native_block_name_in_tag(block, tag).unwrap_or(false)
    }
}

fn item_tags<'a>(game: &'a Game<'_>) -> impl Fn(&str, &str) -> bool + 'a {
    move |item: &str, tag: &str| game.native_server_item_in_tag(&ItemStack::new(item, 1), tag).unwrap_or(false)
}

fn fits(game: &Game<'_>, ingredient: &Ingredient, stack: Option<&ItemStack>) -> bool {
    stack.is_some_and(|s| s.count > 0 && ingredient.test(&s.item, &item_tags(game)))
}

fn sound(game: &mut Game<'_>, sound: &str, pos: [i32; 3], volume: f32, pitch: f32) {
    let at = pos.map(|v| f64::from(v) + 0.5);
    if let Some(level) = game.fork_level_here() {
        game.level_play_sound(level, sound, at, SoundSource::Blocks, volume, pitch);
    }
}

// ---- Block entities -------------------------------------------------------

/// `native_block_entity`: the sacrificial bowl and every golden bowl tier have one (its type id is the block's
/// declaration's).
pub(super) fn new_entity(block: &str, entity: &mut Option<Compound>) {
    if entity.is_none() && (block == BOWL || ritual::bowl_tier(block) > 0) {
        *entity = Some(Compound::new());
    }
}

/// `native_block_entity_types` (compat hook): one type per bowl and chalice, named after its block (fork main's ids).
pub(super) fn block_entity_types(types: &mut Vec<(String, Vec<String>)>) {
    let golden = [
        "occultism:golden_sacrificial_bowl",
        "occultism:dark_golden_sacrificial_bowl",
        "occultism:iesnium_sacrificial_bowl",
        "occultism:dark_iesnium_sacrificial_bowl",
        "occultism:eldritch_chalice",
        "occultism:celestial_chalice",
    ];
    for block in std::iter::once(BOWL).chain(golden) {
        debug_assert!(block == BOWL || ritual::bowl_tier(block) > 0);
        types.push((block.to_owned(), vec![block.to_owned()]));
    }
}

/// `native_block_entity_ticks`: golden bowls tick.
pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if ritual::bowl_tier(block) > 0 {
        *ticks = true;
    }
}

/// `native_block_entity_container`: a bowl's one slot (`ItemStackHandler(1)`, a limit of one). Hoppers may take from
/// it; inserting runs through the clicks only for now. A golden bowl also keeps the ingredients a ritual took.
pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if container.is_some() {
        return;
    }
    let slots = if block == BOWL {
        1
    } else if ritual::bowl_tier(block) > 0 {
        GOLDEN_SLOTS
    } else {
        return;
    };
    *container = Some(ModContainer {
        slots,
        faces: std::array::from_fn(|_| vec![0]),
        inputs: Vec::new(),
        outputs: vec![0],
        menu: None,
    });
}

/// Slot `i`, made if the entity has fewer: the engine sizes a mod entity's slots only when something first moves items.
fn slot_mut(entity: &mut ModBlockEntity, i: usize) -> &mut Option<ItemStack> {
    if entity.items.len() <= i {
        entity.items.resize(i + 1, None);
    }
    &mut entity.items[i]
}

fn slot(entity: &ModBlockEntity, i: usize) -> Option<&ItemStack> {
    entity.items.get(i).and_then(Option::as_ref).filter(|s| s.count > 0)
}

/// What the sacrificial bowls around a golden bowl hold (`Ritual.getItemsOnSacrificialBowls`), in the order
/// `getSacrificialBowls` finds them: each bowl's position and stack.
fn bowl_items(game: &mut Game<'_>, pentacle: &Pentacle, center: [i32; 3]) -> Vec<([i32; 3], ItemStack)> {
    let mut out = Vec::new();
    for pos in pentacle.bowl_positions(center) {
        if game.native_block(pos) != BOWL {
            continue;
        }
        if let Some(Some(stack)) = game.native_with_mod_block_entity(pos, |e| slot(e, 0).cloned()) {
            out.push((pos, stack));
        }
    }
    out
}

// ---- Clicks ---------------------------------------------------------------

/// `native_right_click_block`: a bowl takes or gives its item, a golden bowl starts or stops a ritual, flint and
/// steel lights spirit fire over datura.
pub(super) fn right_click(game: &mut Game<'_>, pos: [i32; 3], face: [i32; 3], canceled: &mut bool) {
    if *canceled {
        return;
    }
    let now = game_time(game);
    if let Some((time, at, taken)) = LAST_CLICK.with(|l| *l.borrow())
        && time == now
        && at == pos
    {
        *canceled |= taken; // the off hand: the main hand's click already acted
        return;
    }
    let block = game.native_block(pos);
    if block == BOWL {
        click_bowl(game, pos, canceled);
    } else if ritual::bowl_tier(&block) > 0 {
        click_golden_bowl(game, pos, canceled);
    } else {
        light_spirit_fire(game, pos, face, canceled);
    }
    let taken = *canceled;
    LAST_CLICK.with(|l| *l.borrow_mut() = Some((now, pos, taken)));
}

/// `SacrificialBowlBlock.useItemOn`: not sneaking, an empty bowl takes one of the held stack, a full one gives its
/// item to the empty hand or the inventory. The click is taken either way.
fn click_bowl(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    *canceled = true;
    if game.native_sneaking() {
        return;
    }
    let held = game.native_used_stack().clone().filter(|s| s.count > 0);
    let inside = game.native_with_mod_block_entity(pos, |e| slot(e, 0).cloned()).flatten();
    match inside {
        None => {
            let Some(mut held) = held else { return };
            let mut one = held.clone();
            one.count = 1;
            if game.native_with_mod_block_entity(pos, |e| *slot_mut(e, 0) = Some(one)).is_none() {
                return;
            }
            held.count -= 1;
            *game.native_used_stack() = (held.count > 0).then_some(held);
            sound(game, "minecraft:entity.item_frame.add_item", pos, 1.0, 1.0);
        }
        Some(stack) => {
            game.native_with_mod_block_entity(pos, |e| *slot_mut(e, 0) = None);
            if held.is_none() {
                *game.native_used_stack() = Some(stack);
            } else if let Some(player) = game.native_acting_player()
                && let Some(left) = game.native_give_stack(player.uuid(), stack)
            {
                game.native_pop_resource(pos, left);
            }
            sound(game, "minecraft:entity.item_frame.remove_item", pos, 1.0, 1.0);
        }
    }
    game.native_block_entity_sync(pos);
}

fn game_time(game: &Game<'_>) -> i64 {
    game.fork_level_here().and_then(|l| game.level_game_time(l)).unwrap_or(0)
}

/// The recipe a golden bowl would start (`RitualRecipe.matches` → `Ritual.identify`): the activation item fits, the
/// bowls hold every ingredient, the pentacle is drawn.
fn identify<'a>(game: &mut Game<'_>, cache: &'a Cache, pos: [i32; 3], activation: &ItemStack) -> Option<&'a RitualRecipe> {
    let count_matching = false; // enableRemainingIngredientCountMatching, off by default
    cache.rituals.iter().find(|r| {
        let Some(pentacle) = cache.pentacles.get(&r.pentacle_id) else { return false };
        if !fits(game, &r.activation_item, Some(activation)) {
            return false;
        }
        let items: Vec<String> = bowl_items(game, pentacle, pos).into_iter().map(|(_, s)| s.item.to_string()).collect();
        ritual::matches_ingredients(&r.ingredients, &items, count_matching, &item_tags(game))
            && pentacle.validate(&World(game), pos).is_some()
    })
}

/// `GoldenSacrificialBowlBlockEntity.activate` with the main hand.
fn click_golden_bowl(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    let Some(held) = game.native_used_stack().clone().filter(|s| s.count > 0) else { return };
    let player = game.native_acting_player().map(|p| p.uuid());
    let running = game.native_with_mod_block_entity(pos, |e| e.nbt.contains_key(&key(RITUAL))).unwrap_or(false);
    if running {
        stop(game, pos, false);
        *canceled = true;
        return;
    }
    let cache = cache(game);
    let Some(recipe) = identify(game, &cache, pos, &held) else {
        if let Some(p) = player {
            let message = if held.item.starts_with("occultism:book_of_binding_") && !held.item.contains("_bound_") {
                "ritual.occultism.book_not_bound"
            } else {
                "ritual.occultism.does_not_exist"
            };
            game.native_tell(p, Text::translate(message, &[]));
        }
        return;
    };
    if recipe.ritual_type != "occultism:craft" {
        if let Some(p) = player {
            let line = format!("[Occultism port] {} rituals are not ported yet: {}", recipe.ritual_type, recipe.id);
            game.native_tell(p, Text::literal(line));
        }
        return;
    }
    // The bowl takes one activation item (`insertItem(0, activationItem.split(1))`) and starts the ritual.
    let mut one = held.clone();
    one.count = 1;
    let mut rest = held;
    rest.count -= 1;
    *game.native_used_stack() = (rest.count > 0).then_some(rest);
    let caster = player;
    let remaining: Vec<i32> = (0..recipe.ingredients.len() as i32).collect();
    game.native_with_mod_block_entity(pos, |e| {
        *slot_mut(e, 0) = Some(one);
        e.nbt.insert(key(RITUAL), Tag::String(JavaString::from(recipe.id.as_str())));
        e.nbt.insert(key(TIME), Tag::Int(0));
        e.nbt.insert(key(ACTIVE), Tag::Byte(1));
        e.nbt.insert(key(SACRIFICED), Tag::Byte(0));
        e.nbt.insert(key(ITEM_USED), Tag::Byte(0));
        e.nbt.insert(key(REMAINING), Tag::IntArray(remaining));
        if let Some(uuid) = caster {
            e.nbt.insert(key(CASTER), Tag::IntArray(uuid_ints(uuid)));
        }
    });
    sound(game, "occultism:start_ritual", pos, 1.0, 1.0);
    if let Some(p) = player {
        game.native_action_bar(p, Text::translate(&format!("ritual.{}.started", recipe.ritual_key()), &[]));
        if let Some((_, name)) = &recipe.entity_to_sacrifice {
            game.native_tell(p, Text::translate("ritual.occultism.sacrifice", &[]));
            game.native_tell(p, Text::translate(name, &[]));
        }
        if recipe.requires_item_use() {
            game.native_tell(p, Text::translate("ritual.occultism.use_item", &[]));
        }
    }
    let dimension = game.native_dimension().to_owned();
    ACTIVE_BOWLS.with(|a| a.borrow_mut().push((dimension, pos)));
    game.native_block_entity_sync(pos);
    *canceled = true;
}

/// `Entity.getUUID` as `NbtUtils.createUUID` writes it: four ints, most significant first.
fn uuid_ints(uuid: u128) -> Vec<i32> {
    (0..4).map(|i| (uuid >> (96 - 32 * i)) as u32 as i32).collect()
}

fn uuid_of(nbt: &Compound) -> Option<u128> {
    let Some(Tag::IntArray(v)) = nbt.get(&key(CASTER)) else { return None };
    (v.len() == 4).then(|| v.iter().fold(0u128, |acc, i| (acc << 32) | u128::from(*i as u32)))
}

fn flag(nbt: &Compound, name: &str) -> bool {
    matches!(nbt.get(&key(name)), Some(Tag::Byte(b)) if *b != 0)
}

/// `stopRitual(finished)`: a finished craft ritual uses the activation item up and drops the result; an interrupted
/// one gives back the activation item and what it took. The bowl forgets the ritual either way.
fn stop(game: &mut Game<'_>, pos: [i32; 3], finished: bool) {
    let Some((recipe_id, caster, activation, consumed)) = game.native_with_mod_block_entity(pos, |e| {
        let id = match e.nbt.get(&key(RITUAL)) {
            Some(Tag::String(s)) => s.to_string_lossy(),
            _ => String::new(),
        };
        let caster = uuid_of(&e.nbt);
        let activation = e.items.first_mut().and_then(Option::take);
        let consumed: Vec<ItemStack> = e.items.iter_mut().skip(1).filter_map(Option::take).collect();
        for k in [RITUAL, TIME, ACTIVE, SACRIFICED, ITEM_USED, CASTER, REMAINING] {
            e.nbt.remove(&key(k));
        }
        (id, caster, activation, consumed)
    }) else {
        return;
    };
    ACTIVE_BOWLS.with(|a| a.borrow_mut().retain(|(_, p)| *p != pos));
    let cache = cache(game);
    let recipe = cache.rituals.iter().find(|r| r.id == recipe_id);
    let ritual_key = recipe.map_or_else(|| "occultism.unknown".to_owned(), RitualRecipe::ritual_key);
    if finished {
        // `Ritual.finish`, then `CraftRitual.finish`: the activation item is used up, the result drops.
        sound(game, "occultism:poof", pos, 0.7, 0.7);
        if let Some(mut a) = activation
            && a.count > 1
        {
            a.count -= 1;
            game.native_with_mod_block_entity(pos, |e| *slot_mut(e, 0) = Some(a));
        }
        if let Some(result) = recipe.and_then(|r| r.result.as_ref()).and_then(|j| game.native_recipe_result(j)) {
            drop_result(game, pos, result);
        }
        if let Some(uuid) = caster {
            game.native_action_bar(uuid, Text::translate(&format!("ritual.{ritual_key}.finished"), &[]));
        }
    } else {
        sound(game, "minecraft:entity.chicken.egg", pos, 0.7, 0.7);
        if let Some(uuid) = caster {
            game.native_action_bar(uuid, Text::translate(&format!("ritual.{ritual_key}.interrupted"), &[]));
        }
        for stack in activation.into_iter().chain(consumed) {
            game.native_pop_resource(pos, stack);
        }
    }
    game.native_block_entity_sync(pos);
}

/// `Ritual.dropResult`: into an empty sacrificial bowl hanging (facing down) one, two or three blocks above, else as
/// an item at the bowl.
fn drop_result(game: &mut Game<'_>, pos: [i32; 3], result: ItemStack) {
    for up in 1..=3 {
        let above = [pos[0], pos[1] + up, pos[2]];
        if game.native_block(above) == BOWL
            && game.native_block_property(above, "facing").as_deref() == Some("down")
            && game.native_with_mod_block_entity(above, |e| slot(e, 0).is_none()) == Some(true)
        {
            game.native_with_mod_block_entity(above, |e| *slot_mut(e, 0) = Some(result));
            game.native_block_entity_sync(above);
            return;
        }
    }
    game.native_pop_resource(pos, result);
}

/// `GoldenSacrificialBowlBlockEntity.tick`, on the API's block entity (its slots hold the API's stacks).
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut mod_api::ModBlockEntity) {
    let Some(Tag::String(id)) = entity.nbt.get(&key(RITUAL)) else { return };
    let id = id.to_string_lossy();
    let cache = cache(game);
    let dimension = game.native_dimension().to_owned();
    ACTIVE_BOWLS.with(|a| {
        let mut a = a.borrow_mut();
        if !a.iter().any(|(d, p)| *d == dimension && *p == pos) {
            a.push((dimension, pos));
        }
    });
    let finish_later = |game: &mut Game<'_>, entity: &mut mod_api::ModBlockEntity, finished: bool| {
        // The ticking entity is out of the world for this call: `stop` reads it back after its tick, so mark it.
        entity.nbt.insert(key("occultismStop"), Tag::Byte(if finished { 2 } else { 1 }));
        let _ = game;
    };
    let Some(recipe) = cache.rituals.iter().find(|r| r.id == id) else {
        finish_later(game, entity, false);
        return;
    };
    let Some(pentacle) = cache.pentacles.get(&recipe.pentacle_id) else {
        finish_later(game, entity, false);
        return;
    };
    let mut remaining: Vec<usize> = match entity.nbt.get(&key(REMAINING)) {
        Some(Tag::IntArray(v)) => v.iter().filter_map(|i| usize::try_from(*i).ok()).filter(|i| *i < recipe.ingredients.len()).collect(),
        _ => Vec::new(),
    };
    // `Ritual.isValid`: the pentacle, the activation item, and the ingredients still to be taken.
    let mut bowls = bowl_items(game, pentacle, pos);
    let items: Vec<String> = bowls.iter().map(|(_, s)| s.item.to_string()).collect();
    let wanted: Vec<Ingredient> = remaining.iter().map(|i| recipe.ingredients[*i].clone()).collect();
    let activation = entity.items.first().and_then(Option::as_ref).and_then(|s| s.try_into_engine(game.items()).ok());
    let valid = pentacle.validate(&World(game), pos).is_some()
        && fits(game, &recipe.activation_item, activation.as_ref())
        && ritual::matches_ingredients(&wanted, &items, false, &item_tags(game));
    if !valid {
        finish_later(game, entity, false);
        return;
    }
    if (recipe.requires_sacrifice() && !flag(&entity.nbt, SACRIFICED)) || (recipe.requires_item_use() && !flag(&entity.nbt, ITEM_USED)) {
        return;
    }
    let tier = ritual::bowl_tier(entity.id());
    let before = match entity.nbt.get(&key(TIME)) {
        Some(Tag::Int(t)) => *t,
        _ => 0,
    };
    let time = ritual::advance(tier, game_time(game), DEFAULT_DURATION_MULTIPLIER, recipe.duration, before);
    entity.nbt.insert(key(TIME), Tag::Int(time));
    if entity.items.len() < GOLDEN_SLOTS {
        entity.items.resize(GOLDEN_SLOTS, None);
    }
    // `Ritual.consumeAdditionalIngredients`: the due ingredients, in order, each from the first bowl that holds it.
    let consumed = entity.items.iter().skip(1).filter(|s| s.is_some()).count();
    let due = ritual::ingredients_due(time, recipe.duration_per_ingredient, consumed);
    let mut taken = 0;
    while taken < due && !remaining.is_empty() {
        let ingredient = &recipe.ingredients[remaining[0]];
        let Some(at) = bowls.iter().position(|(_, s)| ingredient.test(&s.item, &item_tags(game))) else {
            finish_later(game, entity, false);
            return;
        };
        let (bowl, _) = bowls.remove(at);
        let Some(Some(stack)) = game.native_with_mod_block_entity(bowl, |e| slot_mut(e, 0).take()) else {
            finish_later(game, entity, false);
            return;
        };
        if let Some(free) = entity.items.iter_mut().skip(1).find(|s| s.is_none()) {
            *free = stack.try_into_api().ok();
        }
        remaining.remove(0);
        game.native_block_entity_sync(bowl);
        sound(game, "occultism:poof", bowl, 0.7, 0.7);
        taken += 1;
    }
    entity.nbt.insert(key(REMAINING), Tag::IntArray(remaining.iter().map(|i| *i as i32).collect()));
    if ritual::finished(recipe.duration, time) {
        finish_later(game, entity, true);
    }
}

/// After a level's tick: golden bowls that their tick stopped (`occultismStop`), then items in spirit fire.
pub(super) fn level_ticked(game: &mut Game<'_>, dimension: &str) {
    let bowls: Vec<[i32; 3]> = ACTIVE_BOWLS.with(|a| a.borrow().iter().filter(|(d, _)| d == dimension).map(|(_, p)| *p).collect());
    for pos in bowls {
        let stop_as = game
            .native_with_mod_block_entity(pos, |e| match e.nbt.remove(&key("occultismStop")) {
                Some(Tag::Byte(b)) => Some(b),
                _ => None,
            })
            .flatten();
        match stop_as {
            Some(2) => stop(game, pos, true),
            Some(_) => stop(game, pos, false),
            None if game.native_with_mod_block_entity(pos, |e| e.nbt.contains_key(&key(RITUAL))) != Some(true) => {
                ACTIVE_BOWLS.with(|a| a.borrow_mut().retain(|(d, p)| !(d == dimension && *p == pos)));
            }
            None => {}
        }
    }
    burn(game, dimension);
}

// ---- Spirit fire ----------------------------------------------------------

/// `native_entity_inside` (the fork's `native_block_entity_inside`): an item entity in a mod block, kept for the
/// level's end of tick.
pub(super) fn inside(dimension: &str, entity: i32, kind: &str, position: [i32; 3]) {
    if kind == "minecraft:item"
        && let Ok(id) = u32::try_from(entity)
    {
        INSIDE.with(|i| i.borrow_mut().push((dimension.to_owned(), id, position)));
    }
}

/// `SpiritFireBlock.entityInside`: each item in the fire becomes its recipe's result, the whole stack at once.
fn burn(game: &mut Game<'_>, dimension: &str) {
    let mut touched: Vec<(u32, [i32; 3])> = INSIDE.with(|i| {
        let mut all = i.borrow_mut();
        let (here, rest): (Vec<_>, Vec<_>) = all.drain(..).partition(|(d, _, _)| d == dimension);
        *all = rest;
        here.into_iter().map(|(_, id, pos)| (id, pos)).collect()
    });
    touched.dedup();
    if touched.is_empty() {
        return;
    }
    let cache = cache(game);
    let mut done = Vec::new();
    for (id, pos) in touched {
        if done.contains(&id) || game.native_block(pos) != spirit_fire::FIRE {
            continue;
        }
        let min = pos.map(f64::from);
        let max = [min[0] + 1.0, min[1] + 1.0, min[2] + 1.0];
        let Some(item) = game.native_item_entities(dimension, min, max).into_iter().find(|e| e.id == id) else { continue };
        let Some(recipe) = spirit_fire::find(&cache.fire, &item.stack.item, &item_tags(game)) else { continue };
        let Some(result) = game.native_recipe_result(&recipe.assemble(item.stack.count)) else { continue };
        // Java discards the entity and drops the result at the fire's center; the entity takes the result here.
        if game.native_set_item_entity(dimension, id, result) {
            sound(game, "occultism:start_ritual", pos, 1.0, 1.0);
            done.push(id);
        }
    }
}

/// Whether a block can be replaced by the fire (`BlockState.canBeReplaced`), for the blocks it's lit on.
fn replaceable(block: &str) -> bool {
    matches!(
        block,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air" | "minecraft:short_grass" | "minecraft:fern" | "minecraft:snow" | "minecraft:fire"
    )
}

/// `PlayerEventHandler.spiritFire`: flint and steel or a fire charge on a block with datura around lights spirit fire
/// there (or on the face clicked), using the datura up.
fn light_spirit_fire(game: &mut Game<'_>, pos: [i32; 3], face: [i32; 3], canceled: &mut bool) {
    let Some(held) = game.native_used_stack().clone().filter(|s| s.count > 0) else { return };
    let flint = &*held.item == "minecraft:flint_and_steel";
    if !flint && &*held.item != "minecraft:fire_charge" {
        return;
    }
    let dimension = game.native_dimension();
    let (min, max) = spirit_fire::start_box(pos);
    let datura: Vec<u32> = game
        .native_item_entities(dimension, min, max)
        .into_iter()
        .filter(|e| game.native_server_item_in_tag(&e.stack, spirit_fire::START_TAG).unwrap_or(false))
        .map(|e| e.id)
        .collect();
    if datura.is_empty() {
        return;
    }
    let target = if replaceable(&game.native_block(pos)) { pos } else { [pos[0] + face[0], pos[1] + face[1], pos[2] + face[2]] };
    for id in datura {
        game.native_set_item_entity(dimension, id, ItemStack::new("minecraft:air", 0));
    }
    if replaceable(&game.native_block(target))
        && let Some(state) = game.native_default_state(spirit_fire::FIRE)
    {
        let sound_id = if flint { "minecraft:item.flintandsteel.use" } else { "minecraft:item.firecharge.use" };
        sound(game, sound_id, target, 1.0, 1.0);
        if let Some(level) = game.fork_level(dimension) {
            let _ = game.level_set_block(level, target, state, BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS);
        }
        if !game.native_instabuild() || !flint {
            let used = game.native_used_stack();
            if let Some(s) = used.as_mut() {
                if flint {
                    s.damage += 1;
                    if s.damage >= 64 {
                        *used = None;
                    }
                } else {
                    s.count -= 1;
                    if s.count == 0 {
                        *used = None;
                    }
                }
            }
        }
    }
    *canceled = true;
}

// ---- Item use -------------------------------------------------------------

/// `onPlayerRightClickItem`: an item used within 16 blocks of a running ritual that asks for it.
pub(super) fn use_item(game: &mut Game<'_>, stack: &ItemStack) {
    let dimension = game.native_dimension();
    let bowls: Vec<[i32; 3]> = ACTIVE_BOWLS.with(|a| a.borrow().iter().filter(|(d, _)| d == dimension).map(|(_, p)| *p).collect());
    let Some(player) = game.native_acting_player() else { return };
    let Some(position) = game.entity_position(player.entity()) else { return };
    let at = position.map(|v| v.floor() as i64);
    let cache = cache(game);
    for pos in bowls {
        let d = [i64::from(pos[0]) - at[0], i64::from(pos[1]) - at[1], i64::from(pos[2]) - at[2]];
        if d.iter().map(|v| v * v).sum::<i64>() > ITEM_USE_DETECTION_RANGE_SQUARE {
            continue;
        }
        let id = game.native_with_mod_block_entity(pos, |e| match e.nbt.get(&key(RITUAL)) {
            Some(Tag::String(s)) => Some(s.to_string_lossy()),
            _ => None,
        });
        let Some(Some(id)) = id else { continue };
        let Some(item) = cache.rituals.iter().find(|r| r.id == id).and_then(|r| r.item_to_use.as_ref()) else { continue };
        if fits(game, item, Some(stack)) {
            game.native_with_mod_block_entity(pos, |e| e.nbt.insert(key(ITEM_USED), Tag::Byte(1)));
        }
    }
}
