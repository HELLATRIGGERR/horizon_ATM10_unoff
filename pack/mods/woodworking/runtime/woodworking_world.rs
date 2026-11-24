// Woodworking station in the game (support module `crate::game::woodworking_world` of the
// `woodworking` native bundle). The bundle intercepts a pack-configured block and turns it into
// a stonecutter-like station: one input slot, a list of recipes derived at world-open time from the
// game's own crafting and stonecutting recipes, and a one-click craft that consumes
// one input item and places the chosen output in the second slot.
//
// The server builds the index when it opens its world (`native_world_recipes` needs no player); a client that is not in
// the same process builds its own on first use. It is cleared whenever the world closes or fails to open. The block's
// template opens no menu, and the engine opens its container's (`native_block_entity_container`) on a click.

use super::{Game, native_hooks::{NativeRecipeKind, NativeRecipes}};
use crate::game::api_values::{TryIntoApi, TryIntoEngine};
use horizon_client::{containers::ModContainer, items::ItemStack};
use horizon_core::nbt::Compound;
use mod_api::{ModBlockEntity, WorldPhase};
use woodworking_core::{
    generator::{generate, GenRecipe},
    index::{decode_input, encode_input, Entry, WoodIndex},
};
use std::sync::{Arc, Mutex, OnceLock};

// ---- Station block id (read once from pack.toml) ------------------------------------------------

static STATION: OnceLock<String> = OnceLock::new();

fn read_pack_toml() -> String {
    let path = std::env::current_dir()
        .ok()
        .map(|d| d.join("pack").join("pack.toml"))
        .filter(|p| p.exists());
    let content = match path {
        Some(p) => std::fs::read_to_string(p).unwrap_or_default(),
        None => return String::new(),
    };
    // Minimal TOML scan: find "[mods.woodworking]" then "station_block = ..."
    let mut in_section = false;
    for line in content.lines() {
        let line = line.trim();
        if line == "[mods.woodworking]" {
            in_section = true;
        } else if line.starts_with('[') {
            in_section = false;
        } else if in_section {
            if let Some(rest) = line.strip_prefix("station_block") {
                if let Some(val) = rest.splitn(2, '=').nth(1) {
                    return val.trim().trim_matches('"').trim_matches('\'').to_string();
                }
            }
        }
    }
    String::new()
}

fn station_block() -> &'static str {
    STATION.get_or_init(|| {
        // Test harnesses start the game in a temp dir; env var takes precedence.
        if let Ok(v) = std::env::var("WOODWORKING_STATION") {
            if !v.is_empty() {
                return v;
            }
        }
        read_pack_toml()
    })
}

// ---- Wood index (built on first use, shared by client and server) -----------------------------------------

/// The recipe index, shared by the window's client and its server thread: both read the same world
/// data, and the server builds it at world open.
static WOOD_INDEX: Mutex<Option<Arc<WoodIndex>>> = Mutex::new(None);

fn index_slot() -> std::sync::MutexGuard<'static, Option<Arc<WoodIndex>>> {
    WOOD_INDEX.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The world's crafting and stonecutting recipes as the generator reads them: for each non-empty
/// slot the items it accepts, and the first result. Other recipe kinds are not wood shapes.
fn collect_gen_recipes(tables: &NativeRecipes) -> Vec<GenRecipe> {
    let mut result = Vec::new();
    for recipe in &tables.recipes {
        if !matches!(
            recipe.kind,
            NativeRecipeKind::CraftingShaped
                | NativeRecipeKind::CraftingShapeless
                | NativeRecipeKind::Stonecutting
        ) {
            continue;
        }
        let Some(out_stack) = recipe.results.first() else { continue };
        let slots: Vec<Vec<String>> = recipe
            .slots
            .iter()
            .filter(|slot| !slot.is_empty())
            .map(|slot| slot.iter().map(|stack| tables.items[stack.item as usize].id.clone()).collect())
            .collect();
        if slots.is_empty() {
            continue;
        }
        result.push(GenRecipe {
            slots,
            output: tables.items[out_stack.item as usize].id.clone(),
            output_count: out_stack.count.max(1),
        });
    }
    result
}

fn build_index(game: &mut Game<'_>) -> Option<WoodIndex> {
    let tables = game.native_world_recipes()?;
    let recipes = collect_gen_recipes(&tables);
    let items: Vec<String> = tables.items.iter().map(|item| item.id.clone()).collect();
    let generated = generate(&items, &recipes, |id| game.native_max_stack_size(id).unwrap_or(64));
    Some(WoodIndex::build(generated))
}

/// The index, built on first use when the game can read its recipes (a server's always; a client's once it has a player).
fn index(game: &mut Game<'_>) -> Option<Arc<WoodIndex>> {
    if let Some(built) = index_slot().clone() {
        return Some(built);
    }
    let built = Arc::new(build_index(game)?);
    Some(index_slot().get_or_insert(built).clone())
}

/// Run `f` with the index, building it on first use.
fn with_index<R>(game: &mut Game<'_>, f: impl FnOnce(Option<&WoodIndex>) -> R) -> R {
    let built = index(game);
    f(built.as_deref())
}

/// `native_server_started`: the old index goes, and the new world's is built from its recipes and tags.
pub(super) fn opened(game: &mut Game<'_>) {
    *index_slot() = None;
    if !station_block().is_empty() {
        index(game);
    }
}

/// `native_world_lifecycle` v2: drop on close.
pub(super) fn lifecycle(phase: WorldPhase) {
    if matches!(phase, WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed | WorldPhase::WorldOpenFailed) {
        *index_slot() = None;
    }
}

// ---- Block entity types -------------------------------------------------------------------------

/// `native_block_entity_types` (compat hook): the station's entity type, one entry named after the
/// block. Skipped when the station block id is not configured.
pub(super) fn block_entity_types(types: &mut Vec<(String, Vec<String>)>) {
    let id = station_block();
    if !id.is_empty() {
        types.push((id.to_string(), vec![id.to_string()]));
    }
}

// ---- Block entity lifecycle ---------------------------------------------------------------------

/// `native_block_entity`: an empty compound for the station block.
pub(super) fn new_entity(block: &str, entity: &mut Option<Compound>) {
    if entity.is_some() || block != station_block() {
        return;
    }
    *entity = Some(Compound::new());
}

/// `native_block_entity_ticks`: the station ticks.
pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if block == station_block() {
        *ticks = true;
    }
}

// ---- Container ----------------------------------------------------------------------------------

pub(super) const INPUT: usize = 0;
pub(super) const OUTPUT: usize = 1;
// Must equal the slot count of minecraft:generic_9x1 so the menu-type filter in
// native_containers() passes (it checks slots == menu_size).
pub(super) const SLOTS: usize = 9;

/// `native_block_entity_container`: nine slots (matches generic_9x1), only 0 and 1 are real;
/// the screen hook hides 2-8 by giving them no pixel position.
pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if container.is_some() || block != station_block() {
        return;
    }
    let sides = || vec![INPUT];
    *container = Some(ModContainer {
        slots: SLOTS,
        faces: [vec![OUTPUT], vec![INPUT], sides(), sides(), sides(), sides()],
        inputs: vec![INPUT],
        outputs: vec![OUTPUT],
        menu: Some("minecraft:generic_9x1".into()),
    });
}

/// `native_block_entity_slot_accepts`: only the input slot accepts player-placed items.
/// Slots 1-8 are read-only or invisible; slot 1 (output) is filled by the button handler.
pub(super) fn slot_accepts(block: &str, slot: usize, accepts: &mut bool) {
    if block != station_block() {
        return;
    }
    if slot != INPUT {
        *accepts = false;
    }
}

// ---- Tick ---------------------------------------------------------------------------------------

fn get_input_id(game: &Game<'_>, entity: &ModBlockEntity) -> Option<String> {
    let api_stack = entity.items.get(INPUT)?.as_ref()?;
    let stack: ItemStack = api_stack.try_into_engine(game.items()).ok()?;
    Some(stack.item.to_string())
}

/// `native_block_entity_tick`: keep entity.data in sync with the current input item.
/// data[0] = recipe count, data[1] = selected index, data[2] = scroll, data[3..] = input id.
pub(super) fn tick(game: &mut Game<'_>, _pos: [i32; 3], entity: &mut ModBlockEntity) {
    if entity.id() != station_block() {
        return;
    }

    let new_input = get_input_id(game, entity);

    // Skip if input hasn't changed and data is already set.
    let old_input = if entity.data.is_empty() { None } else { decode_input(&entity.data) };
    let counted = entity.data.first().is_some_and(|&n| n > 0) || new_input.is_none();
    if new_input == old_input && !entity.data.is_empty() && counted {
        return;
    }

    let n: i32 = new_input.as_deref().map_or(0, |id| {
        with_index(game, |idx| idx.map_or(0, |i| i.recipes_for(id).len() as i32))
    });

    let mut data = vec![n, -1i32, 0i32];
    encode_input(new_input.as_deref().unwrap_or(""), &mut data);
    entity.data = data;
}

// ---- Button (recipe selection) ------------------------------------------------------------------

fn stack_at(items: &[Option<ItemStack>], slot: usize) -> Option<&ItemStack> {
    items.get(slot)?.as_ref().filter(|s| s.count > 0)
}

/// `native_menu_button`: execute recipe `id` for the entity at `pos`.
/// Consumes 1 input and places the recipe's output in the output slot.
pub(super) fn button(game: &mut Game<'_>, entity: &mut ModBlockEntity, id: i32) {
    if entity.id() != station_block() {
        return;
    }
    let recipe_idx = usize::try_from(id).unwrap_or(usize::MAX);

    // Convert all slots to engine items for manipulation.
    let held: Vec<Option<ItemStack>> = entity
        .items
        .iter()
        .map(|s| s.as_ref().and_then(|s| s.try_into_engine(game.items()).ok()))
        .collect();
    let mut items = held.clone();

    // Find the recipe.
    let input_id = stack_at(&items, INPUT).map(|s| s.item.to_string());
    let Some(input_id) = input_id else { return; };
    let entry: Option<Entry> = with_index(game, |idx| {
        idx.and_then(|i| i.recipes_for(&input_id).get(recipe_idx).cloned())
    });
    let Some(entry) = entry else { return; };

    // Check the output slot: must be empty, or same item with room.
    let max = game.native_max_stack_size(&entry.output).unwrap_or(64) as u32;
    let output_ok = match stack_at(&items, OUTPUT) {
        None => true,
        Some(out) => out.item.as_ref() == entry.output && out.count + entry.count <= max,
    };
    if !output_ok {
        return;
    }

    // Consume 1 input.
    if let Some(slot) = items.get_mut(INPUT) {
        if let Some(stack) = slot.as_mut() {
            stack.count -= 1;
            if stack.count == 0 {
                *slot = None;
            }
        }
    }

    // Place output.
    if let Some(slot) = items.get_mut(OUTPUT) {
        match slot.as_mut() {
            Some(out) => out.count += entry.count,
            None => {
                *slot = Some(ItemStack {
                    item: Arc::from(entry.output.as_str()),
                    count: entry.count,
                    damage: 0,
                    extra: None,
                });
            }
        }
    }

    // Write changed slots back.
    for (i, (now, was)) in items.iter().zip(&held).enumerate() {
        if now != was {
            if let Some(slot) = entity.items.get_mut(i) {
                *slot = now.as_ref().and_then(|s| s.try_into_api().ok());
            }
        }
    }

    // Record selected recipe in data.
    if entity.data.len() > 1 {
        entity.data[1] = id;
    }
}

// ---- Public re-export for ui module -------------------------------------------------------------

/// Let the ui module access the already-built index for drawing.
/// Screen draw has only `&Game`, so this does not build the index; the click that opened the
/// station built it.
pub(super) fn with_index_for_draw<R>(_game: &Game<'_>, f: impl FnOnce(Option<&WoodIndex>) -> R) -> R {
    let built = index_slot().clone();
    f(built.as_deref())
}

pub(super) fn station() -> &'static str {
    station_block()
}
