// Hostile Neural Networks' machines in the game (support module `crate::game::hostilenetworks_world` of the
// `hostilenetworks` native bundle). The rules are the bundle's crate `hostilenetworks_core`; this module is the glue:
// the registry of data models and tiers read from the game's data, the block entities and their saved compounds,
// the item components, the containers, Forge Energy, the menu buttons and the data the screens read. Server hooks run
// on the thread of the `Game` that simulates the world and a client's screen hooks on the client's, so the settings
// and the registry are kept per thread, dropped when the server opens its world and when it closes. The hooks' block
// entities hold the API's stacks: the machines read and write the engine's (components included) and put back the
// slots they changed. Translated from Hostile Neural Networks 6.5.1 (MIT); see this bundle's README.md.

use super::Game;
use crate::game::api_values::{TryIntoApi, TryIntoEngine};
use horizon_client::{containers::ModContainer, items::ItemStack, text::Text};
use horizon_core::nbt::{Compound, JavaString, Tag};
use horizon_core::random::RandomSource;
use mod_api::{Command, CommandArgumentValue, CommandInvocation, CommandSource, ModBlockEntity, NativeFeInfo, WorldPhase};
use hostilenetworks_core::{
    config::Config,
    fab::{self, Fab, Production, Selection},
    model::{Instance, Model, Models},
    sim::{self, Chamber, Ctx, Fail, Mode, Redstone, Stack},
    tier::{Tier, Tiers},
    view::{ChamberView, FabView, QUEUE_SHOWN},
};
use serde_json::Value;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

pub(super) const SIM_CHAMBER: &str = "hostilenetworks:sim_chamber";
pub(super) const LOOT_FABRICATOR: &str = "hostilenetworks:loot_fabricator";

/// The item components (`API 65`): the model of a data model or a prediction, the data a model holds and the
/// simulations it has run.
const DATA_MODEL: &str = "hostilenetworks:data_model";
const DATA: &str = "hostilenetworks:data";
const ITERATIONS: &str = "hostilenetworks:iterations";

/// `native_component_types`: the three components, before content loads.
pub(super) fn component_types(types: &mut Vec<String>) {
    types.extend([DATA_MODEL, DATA, ITERATIONS].map(str::to_owned));
}

// ---- Settings and the registry ---------------------------------------------------------------------------------

/// What the registry holds: the tiers and the data models of every namespace, and the models' ids in order (the
/// number the screens' data names a model by).
pub(super) struct Registry {
    pub tiers: Tiers,
    pub models: Models,
    pub ids: Vec<String>,
}

thread_local! {
    static SETTINGS: RefCell<Option<Config>> = const { RefCell::new(None) };
    static REGISTRY: RefCell<Option<Rc<Registry>>> = const { RefCell::new(None) };
}

fn read_settings() -> Config {
    match Game::native_config_dir() {
        Ok(dir) => Config::load(&dir),
        Err(e) => {
            log::warn!("[hostilenetworks] no config folder ({e}): the mod's defaults");
            Config::default()
        }
    }
}

/// The settings, read on first use if the world's opening didn't.
pub(super) fn settings() -> Config {
    SETTINGS.with(|s| s.borrow_mut().get_or_insert_with(read_settings).clone())
}

/// `native_server_started` (where the fork's `WorldOpenReady` was): settings are read afresh, the registry dropped.
pub(super) fn opened() {
    let fresh = read_settings();
    SETTINGS.with(|s| *s.borrow_mut() = Some(fresh));
    REGISTRY.with(|r| *r.borrow_mut() = None);
}

/// `native_world_lifecycle` v2: settings and registry are dropped as the world closes or fails to open.
pub(super) fn lifecycle(phase: WorldPhase) {
    if matches!(phase, WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed | WorldPhase::WorldOpenFailed) {
        SETTINGS.with(|s| *s.borrow_mut() = None);
        REGISTRY.with(|r| *r.borrow_mut() = None);
    }
}

/// `ModelTierRegistry` and `DataModelRegistry` as the data says: `data/<ns>/model_tiers/<name>.json` and
/// `data/<ns>/data_models/<path>.json`, whose id is `<ns>:<path>`. The block models (`data_models/blocks/`) hold
/// `neoforge:conditions` on `block_data_models_enabled`, which the mod's config turns off; drops of items the game
/// doesn't have go (`optional`, or not: a model a mod's items aren't there for still runs on the others).
fn load_registry(game: &Game<'_>) -> Registry {
    let read = |path: &str| game.native_data(path).and_then(|b| serde_json::from_slice::<Value>(&b).ok());
    let mut tiers = Vec::new();
    for path in game.native_data_files("model_tiers") {
        let name = path.rsplit('/').next().and_then(|f| f.strip_suffix(".json")).unwrap_or_default().to_owned();
        match read(&path).map(|v| Tier::parse(&name, &v)) {
            Some(Ok(tier)) => tiers.push(tier),
            Some(Err(e)) => log::warn!("[hostilenetworks] tier {path}: {e}"),
            None => {}
        }
    }
    let tiers = Tiers::new(tiers).unwrap_or_else(|e| {
        log::warn!("[hostilenetworks] {e}: the mod's tiers");
        Tiers::defaults()
    });
    let mut models = Models::new();
    for path in game.native_data_files("data_models") {
        let mut parts = path.strip_prefix("data/").unwrap_or_default().splitn(3, '/');
        let (Some(namespace), Some("data_models"), Some(rest)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        let rest = rest.strip_suffix(".json").unwrap_or(rest);
        if rest.starts_with("blocks/") {
            continue;
        }
        let id = format!("{namespace}:{rest}");
        match read(&path).map(|v| Model::parse(&id, &v)) {
            Some(Ok(mut model)) => {
                model.fab_drops.retain(|d| game.native_max_stack_size(&d.item).is_some());
                models.insert(id, model);
            }
            Some(Err(e)) => log::warn!("[hostilenetworks] data model {path}: {e}"),
            None => {}
        }
    }
    let ids = models.keys().cloned().collect();
    log::info!("[hostilenetworks] {} model tier(s), {} data model(s)", tiers.len(), models.len());
    Registry { tiers, models, ids }
}

pub(super) fn registry(game: &Game<'_>) -> Rc<Registry> {
    if let Some(r) = REGISTRY.with(|r| r.borrow().clone()) {
        return r;
    }
    let fresh = Rc::new(load_registry(game));
    REGISTRY.with(|r| *r.borrow_mut() = Some(fresh.clone()));
    fresh
}

/// The registry if a screen hook already made it (the tooltip hook has no `Game`).
pub(super) fn cached_registry() -> Option<Rc<Registry>> {
    REGISTRY.with(|r| r.borrow().clone())
}

// ---- The saved compound ----------------------------------------------------------------------------------------

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

fn text(nbt: &Compound, name: &str) -> Option<String> {
    match nbt.get(&key(name)) {
        Some(Tag::String(s)) => Some(String::from(s.to_string_lossy())).filter(|s| !s.is_empty()),
        _ => None,
    }
}

fn put(nbt: &mut Compound, name: &str, v: i32) {
    nbt.insert(key(name), Tag::Int(v));
}

/// The entity's slots as the engine's stacks (the hooks hand the API's).
fn engine_items(game: &Game<'_>, entity: &ModBlockEntity) -> Vec<Option<ItemStack>> {
    entity.items.iter().map(|s| s.as_ref().and_then(|s| s.try_into_engine(game.items()).ok())).collect()
}

/// Puts back into the entity the slots of `items` that differ from `held` (what `engine_items` read).
fn put_back(entity: &mut ModBlockEntity, held: &[Option<ItemStack>], items: &[Option<ItemStack>]) {
    if entity.items.len() < items.len() {
        entity.items.resize(items.len(), None);
    }
    for (i, now) in items.iter().enumerate() {
        if held.get(i) != Some(now) {
            entity.items[i] = now.as_ref().and_then(|s| s.try_into_api().ok());
        }
    }
}

fn read_chamber(n: &Compound, items: &[Option<ItemStack>], game: &Game<'_>) -> Chamber {
    Chamber {
        slots: std::array::from_fn(|i| items.get(i).and_then(Option::as_ref).and_then(|s| core_stack(game, s))),
        energy: int(n, "energy"),
        runtime: int(n, "runtime"),
        prediction_success: int(n, "predSuccess"),
        fail: Fail::from_index(int(n, "failState")),
        redstone: Redstone::from_index(int(n, "redstoneState")),
        mode: Mode::from_index(int(n, "simMode")),
        current: text(n, "model"),
    }
}

fn write_chamber(entity: &mut ModBlockEntity, c: &Chamber) {
    let n = &mut entity.nbt;
    put(n, "energy", c.energy);
    put(n, "runtime", c.runtime);
    put(n, "predSuccess", c.prediction_success);
    put(n, "failState", c.fail.index());
    put(n, "redstoneState", c.redstone.index());
    put(n, "simMode", c.mode.index());
    n.insert(key("model"), Tag::String(key(c.current.as_deref().unwrap_or(""))));
}

const SELECTIONS: &str = "saved_selections";

fn read_fab(n: &Compound, items: &[Option<ItemStack>], game: &Game<'_>) -> Fab {
    let mut selections = BTreeMap::new();
    if let Some(Tag::Compound(saved)) = n.get(&key(SELECTIONS)) {
        for (model, tag) in saved {
            let Tag::Compound(s) = tag else { continue };
            let entries = match s.get(&key("entries")) {
                Some(Tag::IntArray(v)) => v.clone(),
                _ => Vec::new(),
            };
            selections.insert(
                String::from(model.to_string_lossy()),
                Selection {
                    mode: if int(s, "mode") == 1 { Production::Queue } else { Production::Fixed },
                    entries,
                    cursor: int(s, "cursor"),
                },
            );
        }
    }
    Fab {
        slots: std::array::from_fn(|i| items.get(i).and_then(Option::as_ref).and_then(|s| core_stack(game, s))),
        energy: int(n, "energy"),
        runtime: int(n, "runtime"),
        current_sel: if n.contains_key(&key("selection")) { int(n, "selection") } else { -1 },
        redstone: Redstone::from_index(int(n, "redstoneState")),
        selections,
    }
}

fn write_fab(entity: &mut ModBlockEntity, f: &Fab) {
    let n = &mut entity.nbt;
    put(n, "energy", f.energy);
    put(n, "runtime", f.runtime);
    put(n, "selection", f.current_sel);
    put(n, "redstoneState", f.redstone.index());
    let mut saved = Compound::new();
    for (model, s) in &f.selections {
        let mut tag = Compound::new();
        put(&mut tag, "mode", i32::from(s.mode == Production::Queue));
        put(&mut tag, "cursor", s.cursor);
        tag.insert(key("entries"), Tag::IntArray(s.entries.clone()));
        saved.insert(key(model), Tag::Compound(tag));
    }
    n.insert(key(SELECTIONS), Tag::Compound(saved));
}

// ---- Item stacks and components --------------------------------------------------------------------------------

fn number(tag: Option<&Tag>) -> i32 {
    match tag {
        Some(Tag::Int(v)) => *v,
        Some(Tag::Short(v)) => i32::from(*v),
        Some(Tag::Byte(v)) => i32::from(*v),
        Some(Tag::Long(v)) => *v as i32,
        _ => 0,
    }
}

/// The rules' view of an item stack: its item and the three components.
fn core_stack(game: &Game<'_>, s: &ItemStack) -> Option<Stack> {
    if s.count == 0 {
        return None;
    }
    let component = |id: &str| game.native_component(s, id).ok().flatten();
    Some(Stack {
        item: s.item.to_string(),
        count: s.count,
        model: match component(DATA_MODEL) {
            Some(Tag::String(m)) => Some(String::from(m.to_string_lossy())),
            _ => None,
        },
        data: number(component(DATA)),
        iters: number(component(ITERATIONS)),
    })
}

/// Sets the three components on `stack`; a zero number and no model leave it off.
fn set_components(game: &Game<'_>, stack: &mut ItemStack, c: &Stack) {
    let mut set = |id: &str, tag: Tag| {
        if let Err(e) = game.native_set_component(stack, id, tag) {
            log::warn!("[hostilenetworks] component {id}: {e}");
        }
    };
    if let Some(m) = &c.model {
        set(DATA_MODEL, Tag::String(key(m)));
    }
    if c.data != 0 {
        set(DATA, Tag::Int(c.data));
    }
    if c.iters != 0 {
        set(ITERATIONS, Tag::Int(c.iters));
    }
}

/// Makes the slots hold what the rules left: a slot the rules left alone keeps its stack, a count that changed
/// changes it, anything else is made anew.
fn sync_slots(game: &Game<'_>, items: &mut Vec<Option<ItemStack>>, before: &[Option<Stack>], after: &[Option<Stack>]) {
    if items.len() < after.len() {
        items.resize(after.len(), None);
    }
    for (i, (was, now)) in before.iter().zip(after).enumerate() {
        if was == now {
            continue;
        }
        items[i] = match (was, now) {
            (_, None) => None,
            (Some(w), Some(n)) if w.same(n) => items[i].take().map(|mut s| {
                s.count = n.count;
                s
            }),
            (Some(w), Some(n)) if w.item == n.item && w.model == n.model => items[i].take().map(|mut s| {
                s.count = n.count;
                set_components(game, &mut s, n);
                s
            }),
            (_, Some(n)) => {
                let mut s = ItemStack::new(&n.item, n.count);
                set_components(game, &mut s, n);
                Some(s)
            }
        };
    }
}

// ---- Block entities --------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Chamber,
    Fabricator,
}

impl Kind {
    fn of(block: &str) -> Option<Self> {
        match block {
            SIM_CHAMBER => Some(Self::Chamber),
            LOOT_FABRICATOR => Some(Self::Fabricator),
            _ => None,
        }
    }

    fn capacity(self, cfg: &Config) -> i32 {
        match self {
            Self::Chamber => cfg.sim_power_cap,
            Self::Fabricator => cfg.fab_power_cap,
        }
    }
}

/// `native_block_entity`: a new, empty machine; the fabricator's chosen drop starts at -1, as Java's (its type id is
/// the block's declaration's).
pub(super) fn new_entity(block: &str, entity: &mut Option<Compound>) {
    let Some(kind) = Kind::of(block) else { return };
    if entity.is_some() {
        return;
    }
    let mut nbt = Compound::new();
    put(&mut nbt, "energy", 0);
    put(&mut nbt, "runtime", 0);
    put(&mut nbt, "redstoneState", 0);
    match kind {
        Kind::Chamber => {
            put(&mut nbt, "simMode", 0);
            put(&mut nbt, "failState", 0);
            put(&mut nbt, "predSuccess", 0);
        }
        Kind::Fabricator => put(&mut nbt, "selection", -1),
    }
    *entity = Some(nbt);
}

/// `native_block_entity_types` (compat hook): one type per machine, named after its block (fork main's ids).
pub(super) fn block_entity_types(types: &mut Vec<(String, Vec<String>)>) {
    for block in [SIM_CHAMBER, LOOT_FABRICATOR] {
        types.push((block.to_owned(), vec![block.to_owned()]));
    }
}

/// `native_block_entity_ticks`: both tick.
pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if Kind::of(block).is_some() {
        *ticks = true;
    }
}

/// The slots of the vanilla menus the machines show their slots on; a container has as many as its menu.
const HOPPER_SLOTS: usize = 5;
const GENERIC_9X2_SLOTS: usize = 18;

/// `native_block_entity_container`: the chamber's model, matrix and two outputs on a hopper menu (five slots, the
/// fifth unused); the fabricator's prediction and sixteen outputs on a nine-by-two menu. Hoppers feed the inputs from
/// above and the sides and empty the outputs from below.
pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    let Some(kind) = Kind::of(block) else { return };
    if container.is_some() {
        return;
    }
    let (slots, inputs, outputs, menu) = match kind {
        Kind::Chamber => (HOPPER_SLOTS, vec![sim::MODEL, sim::INPUT], vec![sim::BASE_OUT, sim::PREDICTION_OUT], "minecraft:hopper"),
        Kind::Fabricator => (GENERIC_9X2_SLOTS, vec![fab::PREDICTION_IN], (1..fab::SLOTS).collect(), "minecraft:generic_9x2"),
    };
    // `Direction` order: down, up, north, south, west, east.
    *container = Some(ModContainer {
        slots,
        faces: [outputs.clone(), inputs.clone(), inputs.clone(), inputs.clone(), inputs.clone(), inputs.clone()],
        inputs,
        outputs,
        menu: Some(menu.into()),
    });
}

/// `native_block_entity_slot_accepts`: only the input slots take anything. The hook doesn't say which item: the
/// chamber's model slot takes any (Java's takes data models only), and the rules read a slot's model from its
/// component, so another item there does nothing.
pub(super) fn slot_accepts(block: &str, slot: usize, accepts: &mut bool) {
    let Some(kind) = Kind::of(block) else { return };
    let input = match kind {
        Kind::Chamber => slot == sim::MODEL || slot == sim::INPUT,
        Kind::Fabricator => slot == fab::PREDICTION_IN,
    };
    *accepts = *accepts && input;
}

/// `native_block_entity_tick`: one tick of either machine, then what its screen shows.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let Some(kind) = Kind::of(entity.id()) else { return };
    let reg = registry(game);
    let cfg = settings();
    let dimension = game.native_dimension();
    let powered = game.native_neighbor_signal(dimension, pos) > 0;
    let held = engine_items(game, entity);
    let mut items = held.clone();
    match kind {
        Kind::Chamber => {
            let mut c = read_chamber(&entity.nbt, &items, game);
            let before = c.clone();
            // One draw of the level's random, as `rollPredictions` takes, only when a run could start.
            let roll = if c.runtime == 0 && c.slots[sim::MODEL].is_some() && c.slots[sim::INPUT].is_some() && c.energy > 0 {
                game.fork_level_here()
                    .and_then(|level| {
                        // A copy of the level's random, stored back so the draw moves the level's own.
                        let mut random = game.level_random(level)?;
                        let roll = random.next_float();
                        game.level_set_random(level, random);
                        Some(roll)
                    })
                    .unwrap_or(0.5)
            } else {
                0.5
            };
            {
                let max = |item: &str| game.native_max_stack_size(item).unwrap_or(64);
                let ctx = Ctx { cfg: &cfg, tiers: &reg.tiers, models: &reg.models, max_stack: &max };
                c.tick(&ctx, powered, roll);
            }
            if c != before {
                sync_slots(game, &mut items, &before.slots, &c.slots);
                put_back(entity, &held, &items);
                write_chamber(entity, &c);
            }
            entity.data = chamber_view(&reg, &cfg, &c).pack();
        }
        Kind::Fabricator => {
            let mut f = read_fab(&entity.nbt, &items, game);
            let before = f.clone();
            {
                let max = |item: &str| game.native_max_stack_size(item).unwrap_or(64);
                let ctx = Ctx { cfg: &cfg, tiers: &reg.tiers, models: &reg.models, max_stack: &max };
                f.tick(&ctx, powered);
            }
            if f != before {
                sync_slots(game, &mut items, &before.slots, &f.slots);
                put_back(entity, &held, &items);
                write_fab(entity, &f);
            }
            entity.data = fab_view(&reg, &cfg, &f).pack();
        }
    }
}

/// The model of the chamber's slot 0, with its data, when the game has it.
fn chamber_model<'a>(reg: &'a Registry, c: &Chamber) -> Option<(&'a Model, i32, i32)> {
    let s = c.slots[sim::MODEL].as_ref()?;
    Some((reg.models.get(s.model.as_deref()?)?, s.data, s.iters))
}

fn chamber_view(reg: &Registry, cfg: &Config, c: &Chamber) -> ChamberView {
    let mut v = ChamberView {
        energy: c.energy,
        capacity: cfg.sim_power_cap,
        runtime: c.runtime,
        mode: c.mode.index(),
        redstone: c.redstone.index(),
        fail: c.fail.index(),
        success: c.prediction_success,
        can_train: cfg.sim_model_upgrade != 0,
        ..ChamberView::default()
    };
    if let Some((model, data, iters)) = chamber_model(reg, c) {
        let inst = Instance::new(model, &reg.tiers, cfg.continuous_accuracy, data);
        v.model = reg.ids.iter().position(|i| *i == model.id).map_or(0, |p| p as i32 + 1);
        v.data = data;
        v.iters = iters;
        v.tier = inst.tier_index() as i32;
        v.is_max = inst.is_max();
        v.tier_data = inst.tier_data();
        v.next_tier_data = inst.next_tier_data();
        v.cost = c.mode.adjust_cost(model.sim_cost);
        v.accuracy = (inst.accuracy() * 10000.0).round() as i32;
    }
    v
}

fn fab_view(reg: &Registry, cfg: &Config, f: &Fab) -> FabView {
    let mut v = FabView {
        energy: f.energy,
        capacity: cfg.fab_power_cap,
        runtime: f.runtime,
        redstone: f.redstone.index(),
        cost: cfg.fab_power_cost,
        ..FabView::default()
    };
    let model = f.slots[fab::PREDICTION_IN].as_ref().filter(|s| s.item == hostilenetworks_core::model::PREDICTION).and_then(|s| reg.models.get(s.model.as_deref()?));
    if let Some(model) = model {
        v.model = reg.ids.iter().position(|i| *i == model.id).map_or(0, |p| p as i32 + 1);
        v.selected = f.selected_drop(model) + 1;
        let sel = f.selection(&model.id);
        v.mode = i32::from(sel.mode == Production::Queue);
        v.queue_len = sel.entries.len() as i32;
        v.cursor = if sel.entries.is_empty() { 0 } else { sel.cursor.rem_euclid(sel.entries.len() as i32) };
        for (slot, entry) in v.queue.iter_mut().zip(&sel.entries).take(QUEUE_SHOWN) {
            *slot = entry + 1;
        }
    }
    v
}

// ---- Menu buttons ----------------------------------------------------------------------------------------------

/// `native_menu_button`: the chamber's redstone (0 to 2) and mode (3, 4) buttons, the fabricator's (`Fab::button`).
pub(super) fn button(game: &mut Game<'_>, entity: &mut ModBlockEntity, id: i32) {
    let Some(kind) = Kind::of(entity.id()) else { return };
    let cfg = settings();
    let items = engine_items(game, entity);
    match kind {
        Kind::Chamber => {
            let mut c = read_chamber(&entity.nbt, &items, game);
            match id {
                0..=2 => c.redstone = Redstone::from_index(id),
                // Training is refused while the config has model upgrades off.
                3..=4 if Mode::from_index(id - 3) == Mode::Training && cfg.sim_model_upgrade == 0 => {}
                3..=4 => c.mode = Mode::from_index(id - 3),
                _ => return,
            }
            write_chamber(entity, &c);
        }
        Kind::Fabricator => {
            let reg = registry(game);
            let mut f = read_fab(&entity.nbt, &items, game);
            if f.button(&reg.models, &cfg, id) {
                write_fab(entity, &f);
            }
        }
    }
}

// ---- Forge Energy ----------------------------------------------------------------------------------------------

/// `native_fe_storage`: every side takes and none gives.
pub(super) fn fe_storage(entity: &ModBlockEntity, info: &mut Option<NativeFeInfo>) {
    let Some(kind) = Kind::of(entity.id()) else { return };
    if info.is_some() {
        return;
    }
    *info = Some(NativeFeInfo {
        stored: i64::from(int(&entity.nbt, "energy")),
        capacity: i64::from(kind.capacity(&settings())),
        can_receive: true,
        can_extract: false,
    });
}

/// `native_fe_receive`: up to what the buffer has room for.
pub(super) fn fe_receive(entity: &mut ModBlockEntity, max: i64, simulate: bool, accepted: &mut Option<i64>) {
    let Some(kind) = Kind::of(entity.id()) else { return };
    if accepted.is_some() {
        return;
    }
    let stored = int(&entity.nbt, "energy");
    let room = (kind.capacity(&settings()) - stored).max(0);
    let take = max.clamp(0, i64::from(room));
    if take > 0 && !simulate {
        put(&mut entity.nbt, "energy", stored + take as i32);
    }
    *accepted = Some(take);
}

// ---- /hnn give -------------------------------------------------------------------------------------------------

/// `native_register_commands`. A data model carries its model and its data in item components, which the game's
/// `/give` doesn't know of (it parses against the components of the vanilla registry), so `/hnn give` makes one.
/// Upstream's dispatcher parses a usage's arguments itself and its `word` takes no `:`: `/hnn` takes one `text` line
/// the rules read (`give <model> [data]`), operators only.
pub(super) fn commands(commands: &mut Vec<Command>) {
    commands.push(Command::op("hnn [line:text]", 2));
}

/// The command line a parsed invocation of `/hnn` was typed as: its words, then the `text`.
fn line(call: &CommandInvocation) -> String {
    let mut line = call.literals.join(" ");
    for argument in &call.arguments {
        if let CommandArgumentValue::String(text) = &argument.value {
            line.push(' ');
            line.push_str(text);
        }
    }
    line
}

/// `native_run_mod_command`: `/hnn give <model> [data]` gives the player a data model of `model`
/// (`hostilenetworks:zombie` or just `zombie`) holding `data`.
pub(super) fn perform(game: &mut Game<'_>, source: &CommandSource, call: &CommandInvocation, ran: &mut bool) {
    let CommandSource::Player { player, .. } = source else { return };
    let player = player.uuid();
    let command = line(call);
    let mut words = command.split_whitespace();
    if *ran || words.next() != Some("hnn") {
        return;
    }
    *ran = true;
    let reply = match (words.next(), words.next()) {
        (Some("give"), Some(model)) => {
            let reg = registry(game);
            let id = if model.contains(':') { model.to_owned() } else { format!("hostilenetworks:{model}") };
            let data = words.next().and_then(|d| d.parse::<i32>().ok()).unwrap_or(0).max(0);
            if reg.models.contains_key(&id) {
                let mut stack = ItemStack::new("hostilenetworks:data_model", 1);
                set_components(game, &mut stack, &Stack { model: Some(id.clone()), data, ..Stack::default() });
                match game.native_give_stack(player, stack) {
                    None => format!("Gave a {id} data model with {data} data"),
                    Some(_) => "No room in the inventory".to_owned(),
                }
            } else {
                format!("No data model {id} ({} known)", reg.models.len())
            }
        }
        _ => "usage: /hnn give <model> [data]".to_owned(),
    };
    game.native_feedback(Text::literal(reply));
}
