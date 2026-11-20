// Iron Furnaces' furnaces in the game (support module `crate::game::ironfurnaces_world` of the `ironfurnaces`
// native bundle). The rules are the bundle's crate `ironfurnaces_core`; this module is the glue: the block entity
// and its saved compound, the nine-slot container, the world's cooking recipes (`native_cooking_recipes`), the
// settings from `config/ironfurnaces-common.toml`, the augments and the tier upgrades. Server hooks run on the
// thread of the `Game` that simulates the world, so the settings are kept per thread, read when the server has
// opened its world and dropped when it closes. Translated from Iron Furnaces 4.3.2 (Apache-2.0); see this bundle's
// README.md.

use super::Game;
use crate::game::api_values::{TryIntoApi, TryIntoEngine};
use horizon_client::{containers::ModContainer, items::ItemStack};
use horizon_core::nbt::{Compound, JavaString, Tag};
use horizon_world::level::flags;
use mod_api::{BlockUpdateFlags, ModBlockEntity, WorldPhase};
use ironfurnaces_core::{
    config::IronFurnacesConfig,
    furnace::{self, Green, Red, Step, Timers},
    tier::Tier,
    upgrade,
};
use std::cell::RefCell;

thread_local! {
    static SETTINGS: RefCell<Option<IronFurnacesConfig>> = const { RefCell::new(None) };
}

fn read_settings() -> IronFurnacesConfig {
    match Game::native_config_dir() {
        Ok(dir) => IronFurnacesConfig::load(&dir),
        Err(e) => {
            log::warn!("[ironfurnaces] no config folder ({e}): the mod's defaults");
            IronFurnacesConfig::default_config()
        }
    }
}

/// Runs `read` on the settings, read on first use if the world's opening didn't.
fn with_settings<R>(read: impl FnOnce(&IronFurnacesConfig) -> R) -> R {
    SETTINGS.with(|s| read(s.borrow_mut().get_or_insert_with(read_settings)))
}

/// `native_server_started` (where the fork's `WorldOpenReady` was): settings are read as the world opens.
pub(super) fn opened() {
    let fresh = read_settings();
    SETTINGS.with(|s| *s.borrow_mut() = Some(fresh));
}

/// `native_world_lifecycle` v2: settings are dropped as the world closes or fails to open.
pub(super) fn lifecycle(phase: WorldPhase) {
    if matches!(phase, WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed | WorldPhase::WorldOpenFailed) {
        SETTINGS.with(|s| *s.borrow_mut() = None);
    }
}

// ---- The saved compound ---------------------------------------------------

const BURN: &str = "BurnTime";
const COOK: &str = "CookTime";
const COOK_TOTAL: &str = "CookTimeTotal";
const USED: &str = "RecipesUsed";
const TYPE: &str = "StateType";

fn key(name: &str) -> JavaString {
    JavaString::from(name)
}

fn int(nbt: &Compound, name: &str) -> i32 {
    match nbt.get(&key(name)) {
        Some(Tag::Int(v)) => *v,
        Some(Tag::Short(v)) => i32::from(*v),
        Some(Tag::Byte(v)) => i32::from(*v),
        Some(Tag::Long(v)) => *v as i32,
        _ => 0,
    }
}

fn read(nbt: &Compound) -> Timers {
    Timers { burn: int(nbt, BURN), cook: int(nbt, COOK), total: int(nbt, COOK_TOTAL), used: int(nbt, USED) }
}

fn write(nbt: &mut Compound, t: &Timers) {
    nbt.insert(key(BURN), Tag::Int(t.burn));
    nbt.insert(key(COOK), Tag::Int(t.cook));
    nbt.insert(key(COOK_TOTAL), Tag::Int(t.total));
    nbt.insert(key(USED), Tag::Int(t.used));
}

// ---- Block entities -------------------------------------------------------

/// `native_block_entity`: a new, empty furnace (its type id is the block's declaration's).
pub(super) fn new_entity(block: &str, entity: &mut Option<Compound>) {
    if entity.is_some() || Tier::of_block(block).is_none() {
        return;
    }
    let mut nbt = Compound::new();
    write(&mut nbt, &Timers { total: with_settings(|c| Tier::of_block(block).map_or(0, |t| c.speed(t))), ..Timers::default() });
    nbt.insert(key(TYPE), Tag::Int(0));
    *entity = Some(nbt);
}

/// `native_block_entity_types` (compat hook): one type per furnace, named after its block (fork main's ids).
pub(super) fn block_entity_types(types: &mut Vec<(String, Vec<String>)>) {
    for tier in Tier::ALL {
        let id = format!("ironfurnaces:{}", tier.block_name());
        types.push((id.clone(), vec![id]));
    }
}

/// `native_block_entity_ticks`: furnaces tick.
pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if Tier::of_block(block).is_some() {
        *ticks = true;
    }
}

/// `native_block_entity_container`: nine slots of a nine-slot menu: input, fuel, output, the three augments and
/// three the screen leaves out. The default side configuration (`placeConfig`): the top reaches the input and the
/// fuel, the bottom the output, the four sides the fuel.
pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if container.is_some() || Tier::of_block(block).is_none() {
        return;
    }
    // `Direction` order: down, up, north, south, west, east.
    let side = || vec![furnace::FUEL];
    *container = Some(ModContainer {
        slots: furnace::SLOTS,
        faces: [vec![furnace::OUTPUT], vec![furnace::INPUT, furnace::FUEL], side(), side(), side(), side()],
        inputs: vec![furnace::INPUT, furnace::FUEL],
        outputs: vec![furnace::OUTPUT],
        menu: Some("minecraft:generic_9x1".into()),
    });
}

/// `native_block_entity_slot_accepts` (`IisItemValidForSlot`): fuel only in the fuel slot; nothing into the
/// output, the augments or the unused slots. The input takes any item here: the hook doesn't say which, and
/// Java asks the recipe (`hasRecipe`).
pub(super) fn slot_accepts(block: &str, slot: usize, fuel: bool, accepts: &mut bool) {
    if Tier::of_block(block).is_none() {
        return;
    }
    match slot {
        furnace::INPUT => {}
        furnace::FUEL => *accepts = *accepts && fuel,
        _ => *accepts = false,
    }
}

// ---- Cooking ----------------------------------------------------------------

/// The first of the world's recipes of `kind` that takes `item`: its cooking time and its result
/// (`RecipeManager.CachedCheck.getRecipeFor`).
fn find_recipe(game: &Game<'_>, kind: &str, item: &str) -> Option<(i32, ItemStack)> {
    game.native_cooking_recipes()
        .iter()
        .find(|c| c.kind == kind && c.result.count > 0 && c.ingredient.items.contains(item))
        .map(|c| (c.time as i32, c.result.clone()))
}

fn stack_at(items: &[Option<ItemStack>], slot: usize) -> Option<&ItemStack> {
    items.get(slot)?.as_ref().filter(|s| s.count > 0)
}

/// `native_block_entity_tick`: one tick of the furnace branch of `BlockIronFurnaceTileBase.tick`. The hook's slots
/// hold the API's stacks: the furnace works on the engine's and writes back the slots it changed.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let Some(tier) = Tier::of_block(entity.id()) else {
        return;
    };
    let held: Vec<Option<ItemStack>> =
        entity.items.iter().map(|s| s.as_ref().and_then(|s| s.try_into_engine(game.items()).ok())).collect();
    let mut items = held.clone();
    let (regular, mult) = with_settings(|c| (c.speed(tier), c.mult(tier)));
    let before = read(&entity.nbt);
    let mut t = before;
    let red = stack_at(&items, furnace::AUGMENT_RED).map_or(Red::None, |s| Red::of_item(&s.item));
    let green = stack_at(&items, furnace::AUGMENT_GREEN).map_or(Green::None, |s| Green::of_item(&s.item));
    let input = stack_at(&items, furnace::INPUT).cloned();
    let recipe = input.as_ref().and_then(|s| find_recipe(game, red.recipe_kind(), &s.item));
    // `getCookTime`: with no input the total stays; with one, the recipe's time at this tier and these augments.
    if input.is_some() {
        t.total = furnace::cook_time(recipe.as_ref().map_or(0, |(time, _)| furnace::recipe_speed(regular, *time)), green);
    }
    // `canSmelt`: a recipe, and room for its result on the output stack.
    let valid = recipe.as_ref().is_some_and(|(_, result)| match stack_at(&items, furnace::OUTPUT) {
        None => true,
        Some(out) => {
            let max = game.native_max_stack_size(&result.item).unwrap_or(64) as i32;
            furnace::fits(out.count as i32, result.count as i32, out.item == result.item, max)
        }
    });
    let fuel = stack_at(&items, furnace::FUEL).cloned();
    let fuel_burn = fuel.as_ref().map_or(0, |s| game.native_burn_time(s));
    let done = t.tick(&Step {
        has_input: input.is_some(),
        has_fuel: fuel.is_some(),
        valid,
        fuel_burn,
        cook_time: t.total,
        green,
    });
    if done.used_fuel {
        // `itemstack.shrink(1)`, or the remainder (a lava bucket leaves its bucket).
        let rest = fuel.as_ref().filter(|s| s.count == 1).and_then(|s| {
            let mut rest = None;
            game.native_crafting_remainder(s, &mut rest);
            rest
        });
        if let Some(slot) = items.get_mut(furnace::FUEL) {
            if let Some(stack) = slot.as_mut() {
                stack.count -= 1;
                if stack.count == 0 {
                    *slot = rest;
                }
            }
        }
    }
    if done.smelted
        && let Some((_, result)) = recipe
    {
        let held = stack_at(&items, furnace::OUTPUT).map_or(0, |s| s.count as i32);
        let (used, made) = furnace::smelt_count(mult, input.as_ref().map_or(0, |s| s.count as i32), held, result.count as i32);
        if made > 0 {
            if let Some(slot) = items.get_mut(furnace::OUTPUT) {
                match slot.as_mut() {
                    Some(out) => out.count += made as u32,
                    None => {
                        let mut out = result.clone();
                        out.count = made as u32;
                        *slot = Some(out);
                    }
                }
            }
            if let Some(slot) = items.get_mut(furnace::INPUT) {
                if let Some(stack) = slot.as_mut() {
                    stack.count = stack.count.saturating_sub(used as u32);
                    if stack.count == 0 {
                        *slot = None;
                    }
                }
            }
        }
    }
    for (i, (now, was)) in items.iter().zip(&held).enumerate() {
        if now != was
            && let Some(slot) = entity.items.get_mut(i)
        {
            *slot = now.as_ref().and_then(|s| s.try_into_api().ok());
        }
    }
    if t != before {
        write(&mut entity.nbt, &t);
    }
    if t.burning() != before.burning() {
        let lit = if t.burning() { "true" } else { "false" };
        game.native_set_block_property_with(pos, "lit", lit, flags::UPDATE_CLIENTS);
    }
    // The block state's `type` follows the red augment (smoking and blasting draw their own faces).
    if int(&entity.nbt, TYPE) != red.state_type() {
        entity.nbt.insert(key(TYPE), Tag::Int(red.state_type()));
        game.native_set_block_property_with(pos, "type", &red.state_type().to_string(), flags::UPDATE_CLIENTS);
    }
    let data = [t.burn, t.used, t.cook, t.total];
    if entity.data != data {
        entity.data = data.to_vec();
    }
}

// ---- Items used on a furnace ----------------------------------------------

thread_local! {
    /// `use_on` took the use in progress: `take_use` answers it.
    static USE_TAKEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether `use_on` took the use the hook is answering (once).
pub(super) fn take_use() -> bool {
    USE_TAKEN.with(|u| u.replace(false))
}

/// `native_use_on` (the fork's `native_use_item_on`): a tier upgrade turns the furnace into the next one, an augment
/// goes into its slot; the use is then answered `Success` (`take_use`). Sneaking, so the furnace's screen doesn't
/// open first (Java's block takes a plain click for its screen, and the item's `useOn` runs for a sneaking one). On
/// the server only: a client's `use_on` leaves the world to the server.
pub(super) fn use_on(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3]) {
    USE_TAKEN.with(|u| u.set(false));
    if game.remote.is_some() {
        return;
    }
    let block = game.native_block(pos);
    if Tier::of_block(&block).is_none() {
        return;
    }
    let used = if let Some(to) = upgrade::upgraded(&stack.item, &block) {
        install(game, pos, to).is_ok()
    } else if let Some(slot) = augment_slot(&stack.item) {
        put_augment(game, pos, slot, stack)
    } else {
        false
    };
    if used {
        USE_TAKEN.with(|u| u.set(true));
        if !game.native_instabuild() {
            let hand = game.native_used_stack();
            if let Some(held) = hand.as_mut() {
                held.count = held.count.saturating_sub(1);
            }
            if hand.as_ref().is_some_and(|s| s.count == 0) {
                *hand = None;
            }
        }
    }
}

/// The slot an augment item goes into (`interactAugment`): red for blasting and smoking, green for speed and
/// fuel. The blue ones (factory and generator) change what the furnace is and aren't ported yet.
fn augment_slot(item: &str) -> Option<usize> {
    match item {
        "ironfurnaces:augment_blasting" | "ironfurnaces:augment_smoking" => Some(furnace::AUGMENT_RED),
        "ironfurnaces:augment_speed" | "ironfurnaces:augment_fuel" => Some(furnace::AUGMENT_GREEN),
        _ => None,
    }
}

/// `interactAugment`: the one in the slot comes out, a copy of one of the held ones goes in, and what burns
/// stops (`currentAugment` changed: `furnaceBurnTime = 0`).
fn put_augment(game: &mut Game<'_>, pos: [i32; 3], slot: usize, held: &ItemStack) -> bool {
    let mut one = held.clone();
    one.count = 1;
    let old = game.native_with_mod_block_entity(pos, |m| {
        if m.items.len() < furnace::SLOTS {
            m.items.resize(furnace::SLOTS, None);
        }
        let old = m.items[slot].replace(one);
        let mut t = read(&m.nbt);
        t.burn = 0;
        write(&mut m.nbt, &t);
        old
    });
    let Some(old) = old else {
        return false;
    };
    if let Some(old) = old
        && !game.native_instabuild()
    {
        game.native_pop_resource([pos[0], pos[1] + 1, pos[2]], old);
    }
    game.native_block_entity_changed(pos);
    true
}

/// `ItemUpgrade.useOn`: the block becomes `to`, with the old one's contents and timers and its facing.
/// (Java faces the new block towards the player; this keeps the old facing.)
pub(super) fn install(game: &mut Game<'_>, pos: [i32; 3], to: &str) -> Result<(), String> {
    // Take the old block's contents out first, so replacing it drops nothing.
    let (nbt, items) = game
        .native_with_mod_block_entity(pos, |m| (m.nbt.clone(), std::mem::take(&mut m.items)))
        .ok_or("no block entity")?;
    let facing = game.native_block_property(pos, "facing").unwrap_or_else(|| "north".into());
    let kind = game.native_block_property(pos, "type");
    let jovial = game.native_block_property(pos, "jovial");
    let state = game.native_default_state(to).ok_or("no such block")?;
    let level = game.fork_level_here().ok_or("no level")?;
    if !matches!(game.level_set_block(level, pos, state, BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS), Ok(true)) {
        game.native_with_mod_block_entity(pos, |m| m.items = items);
        return Err("the block didn't change".into());
    }
    game.native_set_block_property_with(pos, "facing", &facing, flags::UPDATE_ALL);
    for (name, value) in [("type", kind), ("jovial", jovial)] {
        if let Some(value) = value {
            game.native_set_block_property_with(pos, name, &value, flags::UPDATE_ALL);
        }
    }
    let timers = read(&nbt);
    if timers.burning() {
        game.native_set_block_property_with(pos, "lit", "true", flags::UPDATE_ALL);
    }
    let moved = game.native_with_mod_block_entity(pos, |m| {
        for name in [BURN, COOK, COOK_TOTAL, USED, TYPE] {
            if let Some(tag) = nbt.get(&key(name)) {
                m.nbt.insert(key(name), tag.clone());
            }
        }
        m.items = items;
    });
    if moved.is_none() {
        return Err("the new block has no block entity".into());
    }
    game.native_block_entity_changed(pos);
    Ok(())
}
