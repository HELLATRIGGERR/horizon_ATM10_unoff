// Oritech's processing machines in the game (support module `crate::game::oritech_world` of the `oritech` native bundle).
// The rules are the bundle's crate `oritech_core`; this module is the glue: the settings of `oritech-common.toml`, the
// recipe books read from the world's tables, the block entities and their saved compound (Java's keys), the
// containers, the multiblock cores around a controller, Forge Energy and the numbers the screen reads. Server hooks run
// on the thread of the `Game` that simulates the world and a client's screen hooks on the client's, so the settings,
// the books and the live state Java keeps unsaved (the current recipe, burst ticks) are kept per thread, dropped when
// the server opens its world and when it closes. Translated from Oritech 1.2.13 (CC0); see this bundle's README.md.

use super::Game;
use crate::game::api_values::{TryIntoApi, TryIntoEngine};
use horizon_client::{containers::ModContainer, items::ItemStack, text::Text};
use horizon_core::nbt::{Compound, JavaString, Tag};
use mod_api::{Command, CommandArgumentValue, CommandInvocation, CommandSource, ModBlockEntity, NativeFeInfo, WorldPhase};
use oritech_core::{
    config::Config,
    energy::Energy,
    machine::{self, Kind, Spec},
    multiblock::{self, Facing},
    process::{AddonData, Ctx, Machine},
    recipe::{self, RecipeBook, Stack},
    view::MachineView,
};
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    rc::Rc,
};

// ---- Settings ---------------------------------------------------------------------------------------------------

thread_local! {
    static SETTINGS: RefCell<Option<Config>> = const { RefCell::new(None) };
    static BOOKS: RefCell<Option<Rc<Books>>> = const { RefCell::new(None) };
    /// What Java keeps in the block entity unsaved, by dimension and position: the current recipe and burst ticks.
    static LIVE: RefCell<HashMap<(&'static str, [i32; 3]), Live>> = RefCell::new(HashMap::new());
}

#[derive(Clone, Default)]
struct Live {
    current: Option<String>,
    burst: i32,
    last_worked_at: i64,
}

fn read_settings() -> Config {
    match Game::native_config_dir() {
        Ok(dir) => Config::load(&dir),
        Err(e) => {
            log::warn!("[oritech] no config folder ({e}): the mod's defaults");
            Config::default()
        }
    }
}

/// The settings, read on first use if the world's opening didn't.
pub(super) fn settings() -> Config {
    SETTINGS.with(|s| s.borrow_mut().get_or_insert_with(read_settings).clone())
}

fn drop_state() {
    BOOKS.with(|b| *b.borrow_mut() = None);
    LIVE.with(|l| l.borrow_mut().clear());
}

/// `native_server_started`: the settings are read afresh, the books and the live state dropped.
pub(super) fn opened() {
    let fresh = read_settings();
    SETTINGS.with(|s| *s.borrow_mut() = Some(fresh));
    drop_state();
}

/// `native_world_lifecycle` v2: all of it is dropped as the world closes or fails to open.
pub(super) fn lifecycle(phase: WorldPhase) {
    if matches!(phase, WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed | WorldPhase::WorldOpenFailed) {
        SETTINGS.with(|s| *s.borrow_mut() = None);
        drop_state();
    }
}

// ---- Recipe books -----------------------------------------------------------------------------------------------

/// Each ported machine's recipes as the world's table has them, with what was left out, and the crafting lookup of
/// `combineSmallDusts` memoised.
struct Books {
    stamp: Vec<(usize, usize)>,
    books: HashMap<Kind, Rc<RecipeBook>>,
    /// Per recipe type: recipes in the world's table, and those the machine reads.
    counts: BTreeMap<&'static str, (usize, usize)>,
    compact: RefCell<HashMap<(String, String), bool>>,
}

/// The items an ingredient names (`{"item"}`, `{"tag"}`, a list of them, or 26.3's strings); none when it names
/// nothing the game has, or is a mod's own ingredient type (Java's codec would fail on what it can't read).
fn items_of(game: &Game<'_>, v: &Value) -> Option<Vec<String>> {
    let one = |s: String| {
        game.native_recipe_ingredient(&Value::String(s)).map(|i| {
            let mut items: Vec<String> = i.items.into_iter().collect();
            items.sort();
            items
        })
    };
    let found = match v {
        Value::String(s) => one(s.clone()),
        Value::Array(list) => {
            let mut all: Vec<String> = list.iter().filter_map(|c| items_of(game, c)).flatten().collect();
            all.sort();
            all.dedup();
            (!all.is_empty()).then_some(all)
        }
        Value::Object(o) if o.contains_key("type") => None,
        Value::Object(o) => {
            if let Some(item) = o.get("item").and_then(Value::as_str) {
                return one(item.to_owned());
            }
            one(format!("#{}", o.get("tag").and_then(Value::as_str)?))
        }
        _ => None,
    };
    found.filter(|items| !items.is_empty())
}

fn stamp(game: &Game<'_>) -> Vec<(usize, usize)> {
    machine::ALL
        .iter()
        .map(|s| s.recipe_type)
        .chain(["minecraft:crafting"])
        .map(|t| {
            let s = game.native_recipes_of_type(t);
            (s.as_ptr() as *const u8 as usize, s.len())
        })
        .collect()
}

fn load_books(game: &Game<'_>, stamp: Vec<(usize, usize)>) -> Books {
    let mut books = HashMap::new();
    let mut counts = BTreeMap::new();
    for spec in machine::ALL {
        let table = game.native_recipes_of_type(spec.recipe_type);
        let mut read = Vec::new();
        for r in table.iter() {
            match recipe::parse(&r.id, &r.json, |v| items_of(game, v)) {
                // Java's codec fails a result of an item the game doesn't have.
                Ok(p) if !p.fluids && p.results.iter().all(|s| game.native_max_stack_size(&s.item).is_some()) => read.push(p),
                Ok(_) => log::debug!("[oritech] {}: a result or fluid it can't make", r.id),
                Err(e) => log::debug!("[oritech] {}: {e}", r.id),
            }
        }
        counts.insert(spec.recipe_type, (table.len(), read.len()));
        books.insert(spec.kind, Rc::new(RecipeBook::new(read)));
    }
    log::info!("[oritech] recipes read: {counts:?}");
    Books { stamp, books, counts, compact: RefCell::new(HashMap::new()) }
}

/// The books, made again when a table changed (a reload).
fn books(game: &Game<'_>) -> Rc<Books> {
    let now = stamp(game);
    if let Some(b) = BOOKS.with(|b| b.borrow().clone()).filter(|b| b.stamp == now) {
        return b;
    }
    let fresh = Rc::new(load_books(game, now));
    BOOKS.with(|b| *b.borrow_mut() = Some(fresh.clone()));
    fresh
}

/// `combineSmallDusts`' lookup: `getRecipesFor(CRAFTING, 3x3 of small)` and any result of `base`. A grid of nine
/// is a shaped recipe of nine cells or a shapeless one of nine ingredients, each taking the item.
fn compacts(game: &Game<'_>, books: &Books, small: &str, base: &str) -> bool {
    let key = (small.to_owned(), base.to_owned());
    if let Some(known) = books.compact.borrow().get(&key) {
        return *known;
    }
    let found = game
        .native_recipes_of_type("minecraft:crafting")
        .iter()
        .filter(|r| r.ingredients.len() == 9 && r.ingredients.iter().all(|i| i.items.contains(small)))
        .any(|r| r.results.first().is_some_and(|s| AsRef::<str>::as_ref(&s.item) == base));
    books.compact.borrow_mut().insert(key, found);
    found
}

// ---- The saved compound -----------------------------------------------------------------------------------------

const PROGRESS: &str = "oritech.machine_progress";
const ENERGY: &str = "oritech.machine_energy";
const INPUT_MODE: &str = "oritech.machine_input_mode";
const REDSTONE: &str = "oritech.redstone";
const CORES: &str = "connectedCores";
const QUALITY: &str = "coreQuality";

fn key(name: &str) -> JavaString {
    JavaString::from(name)
}

fn number(nbt: &Compound, name: &str) -> i64 {
    match nbt.get(&key(name)) {
        Some(Tag::Long(v)) => *v,
        Some(Tag::Int(v)) => i64::from(*v),
        Some(Tag::Short(v)) => i64::from(*v),
        Some(Tag::Byte(v)) => i64::from(*v),
        _ => 0,
    }
}

/// `connectedCores`: the cores' offsets from the controller (`RelativePosition.toOffset`).
fn cores(nbt: &Compound) -> Vec<[i32; 3]> {
    let Some(Tag::List(list)) = nbt.get(&key(CORES)) else { return Vec::new() };
    list.iter()
        .filter_map(|t| match t {
            Tag::Compound(c) => Some(["x", "y", "z"].map(|k| number(c, k) as i32)),
            _ => None,
        })
        .collect()
}

fn put_cores(nbt: &mut Compound, offsets: &[[i32; 3]], quality: f32) {
    let list = offsets
        .iter()
        .map(|o| {
            let mut c = Compound::new();
            for (k, v) in ["x", "y", "z"].iter().zip(o) {
                c.insert(key(k), Tag::Int(*v));
            }
            Tag::Compound(c)
        })
        .collect();
    nbt.insert(key(CORES), Tag::List(list));
    nbt.insert(key(QUALITY), Tag::Float(quality.to_bits()));
}

/// The entity's slots as the engine's stacks (the hooks hand the API's).
fn engine_items(game: &Game<'_>, entity: &ModBlockEntity) -> Vec<Option<ItemStack>> {
    entity.items.iter().map(|s| s.as_ref().and_then(|s| s.try_into_engine(game.items()).ok())).collect()
}

fn core_slots(items: &[Option<ItemStack>], size: usize) -> Vec<Option<Stack>> {
    (0..size)
        .map(|i| items.get(i).and_then(Option::as_ref).filter(|s| s.count > 0).map(|s| Stack::new(&s.item.to_string(), s.count)))
        .collect()
}

/// Puts back the slots the rules changed: a count that changed on the same item keeps the stack (its components),
/// anything else is made anew.
fn put_back(entity: &mut ModBlockEntity, held: &[Option<ItemStack>], before: &[Option<Stack>], after: &[Option<Stack>]) {
    if entity.items.len() < after.len() {
        entity.items.resize(after.len(), None);
    }
    for (i, (was, now)) in before.iter().zip(after).enumerate() {
        if was == now {
            continue;
        }
        let stack = match (was, now) {
            (_, None) => None,
            (Some(w), Some(n)) if w.item == n.item => held.get(i).cloned().flatten().map(|mut s| {
                s.count = n.count;
                s
            }),
            (_, Some(n)) => Some(ItemStack::new(&n.item, n.count)),
        };
        entity.items[i] = stack.and_then(|s| s.try_into_api().ok());
    }
}

// ---- Block entities ---------------------------------------------------------------------------------------------

/// `native_block_entity_types` (compat hook): one type per machine, named after its block.
pub(super) fn block_entity_types(types: &mut Vec<(String, Vec<String>)>) {
    for spec in machine::ALL {
        types.push((spec.block.to_owned(), vec![spec.block.to_owned()]));
    }
}

/// `native_block_entity`: a new machine, its compound as Java's first save writes it.
pub(super) fn new_entity(block: &str, entity: &mut Option<Compound>) {
    let Some(spec) = machine::of_block(block) else { return };
    if entity.is_some() {
        return;
    }
    let mut nbt = Compound::new();
    nbt.insert(key(PROGRESS), Tag::Int(0));
    nbt.insert(key(ENERGY), Tag::Long(0));
    nbt.insert(key(INPUT_MODE), Tag::Short(0));
    nbt.insert(key(REDSTONE), Tag::Byte(0));
    if spec.is_multiblock() {
        put_cores(&mut nbt, &[], 1.0);
    }
    *entity = Some(nbt);
}

/// `native_block_entity_ticks`: every machine ticks.
pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if machine::of_block(block).is_some() {
        *ticks = true;
    }
}

/// The vanilla menu the machines show their slots on: a hopper's five hold every machine's.
const HOPPER_SLOTS: usize = 5;

/// `native_block_entity_container`: the machine's slots on a hopper menu (the slots past its inventory unused).
/// Java's default input mode (`FILL_LEFT_TO_RIGHT`, `InOutInventoryStorage`) takes into the inputs and gives from
/// the outputs on every side.
pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    let Some(spec) = machine::of_block(block) else { return };
    if container.is_some() {
        return;
    }
    let s = spec.slots;
    let inputs: Vec<usize> = (s.input_start..s.input_start + s.input_count).collect();
    let outputs: Vec<usize> = (s.output_start..s.output_start + s.output_count).collect();
    let all: Vec<usize> = (0..spec.inventory_size).collect();
    *container = Some(ModContainer {
        slots: HOPPER_SLOTS,
        faces: std::array::from_fn(|_| all.clone()),
        inputs,
        outputs,
        menu: Some("minecraft:hopper".into()),
    });
}

/// `native_block_entity_slot_accepts`: only the input slots take items.
pub(super) fn slot_accepts(block: &str, slot: usize, accepts: &mut bool) {
    let Some(spec) = machine::of_block(block) else { return };
    *accepts = *accepts && spec.slots.is_input(slot);
}

/// The settings' energy buffer of a machine with what its compound holds.
fn energy_of(cfg: &Config, spec: &Spec, nbt: &Compound) -> Energy {
    let m = cfg.machine(spec.kind);
    Energy { amount: number(nbt, ENERGY), capacity: m.energy_capacity, max_insert: m.max_energy_insertion, max_extract: m.max_energy_extraction }
}

/// A multiblock's cores: `(assembled, missing)`. The controller assembles once every core position holds a machine
/// core (`initMultiblock`, the quality their average) and comes apart when one is gone (`onCoreBroken`).
fn check_cores(game: &Game<'_>, pos: [i32; 3], spec: &Spec, nbt: &mut Compound) -> (bool, i32) {
    let facing = game.native_block_property(pos, "facing").as_deref().and_then(Facing::from_name).unwrap_or(Facing::North);
    let positions = multiblock::core_positions(pos, facing, spec.cores);
    let found: Vec<Option<f32>> = positions.iter().map(|p| multiblock::core_quality(&game.native_block(*p))).collect();
    let missing = found.iter().filter(|q| q.is_none()).count() as i32;
    let connected = !cores(nbt).is_empty();
    match multiblock::assemble(&found) {
        Some(quality) => {
            if !connected {
                let offsets: Vec<[i32; 3]> = positions.iter().map(|p| [p[0] - pos[0], p[1] - pos[1], p[2] - pos[2]]).collect();
                put_cores(nbt, &offsets, quality);
            }
            (true, 0)
        }
        None => {
            if connected {
                put_cores(nbt, &[], 1.0);
            }
            (false, missing)
        }
    }
}

/// `native_block_entity_tick`: one tick of a machine (`UpgradableMachineBlockEntity.serverTick`), then what its
/// screen shows.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let Some(spec) = machine::of_block(entity.id()) else { return };
    let cfg = settings();
    let books = books(game);
    let Some(book) = books.books.get(&spec.kind).cloned() else { return };
    let (active, missing) = if spec.is_multiblock() { check_cores(game, pos, spec, &mut entity.nbt) } else { (true, 0) };
    let at = (game.native_dimension(), pos);
    let live = LIVE.with(|l| l.borrow().get(&at).cloned()).unwrap_or_default();
    let mut m = Machine::new(energy_of(&cfg, spec, &entity.nbt));
    m.progress = number(&entity.nbt, PROGRESS) as i32;
    m.disabled_via_redstone = number(&entity.nbt, REDSTONE) != 0;
    m.current = live.current;
    m.burst = live.burst;
    m.last_worked_at = live.last_worked_at;
    m.addons = AddonData::default();
    let held = engine_items(game, entity);
    let before = core_slots(&held, spec.inventory_size);
    let mut slots = before.clone();
    let changed = {
        let max = |item: &str| game.native_max_stack_size(item).unwrap_or(64);
        let compact = |small: &str, base: &str| compacts(game, &books, small, base);
        let ctx = Ctx {
            book: &book,
            energy_per_tick: cfg.machine(spec.kind).energy_per_tick,
            burst_speed_multiplier: cfg.burst_speed_multiplier as f32,
            burst_throttle_multiplier: cfg.burst_throttle_multiplier as f32,
            game_time: 0,
            active,
            max_stack: &max,
            compact: &compact,
        };
        let changed = m.tick(spec, &mut slots, &ctx);
        let total = m.current.as_deref().and_then(|id| book.get(id)).map_or(0, |r| (r.time as f32 * m.speed_multiplier(&ctx)).ceil() as i32);
        entity.data = MachineView {
            energy: m.energy.amount,
            capacity: m.energy.capacity,
            insert: m.energy.max_insert,
            usage: m.energy_usage(&ctx).round() as i64,
            progress: m.progress,
            total,
            speed: (m.speed_multiplier(&ctx) * 100.0).round() as i32,
            efficiency: (m.efficiency_multiplier(&ctx) * 100.0).round() as i32,
            active,
            missing,
        }
        .pack();
        changed
    };
    if changed {
        if slots != before {
            put_back(entity, &held, &before, &slots);
        }
        entity.nbt.insert(key(PROGRESS), Tag::Int(m.progress));
        entity.nbt.insert(key(ENERGY), Tag::Long(m.energy.amount));
    }
    LIVE.with(|l| {
        l.borrow_mut().insert(at, Live { current: m.current, burst: m.burst, last_worked_at: m.last_worked_at });
    });
}

// ---- Forge Energy -----------------------------------------------------------------------------------------------

/// `native_fe_storage`: every side takes (`getEnergyStorage(direction)` is the one buffer) and none gives.
pub(super) fn fe_storage(entity: &ModBlockEntity, info: &mut Option<NativeFeInfo>) {
    let Some(spec) = machine::of_block(entity.id()) else { return };
    if info.is_some() {
        return;
    }
    let e = energy_of(&settings(), spec, &entity.nbt);
    *info = Some(NativeFeInfo { stored: e.amount, capacity: e.capacity, can_receive: e.max_insert > 0, can_extract: e.max_extract > 0 });
}

/// `native_fe_receive`: `DynamicEnergyStorage.insert`, up to `maxInsert` a call and the room left.
pub(super) fn fe_receive(entity: &mut ModBlockEntity, max: i64, simulate: bool, accepted: &mut Option<i64>) {
    let Some(spec) = machine::of_block(entity.id()) else { return };
    if accepted.is_some() {
        return;
    }
    let mut e = energy_of(&settings(), spec, &entity.nbt);
    let took = e.insert(max.max(0), simulate);
    if took > 0 && !simulate {
        entity.nbt.insert(key(ENERGY), Tag::Long(e.amount));
    }
    *accepted = Some(took);
}

// ---- /oritech ---------------------------------------------------------------------------------------------------

/// `native_register_commands`: `/oritech recipes` (operators) tells how many of the world's recipes each ported
/// machine reads. Upstream's `word` takes no `:`: one `text` line the rules read.
pub(super) fn commands(commands: &mut Vec<Command>) {
    commands.push(Command::op("oritech [line:text]", 2));
}

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

/// `native_run_mod_command`.
pub(super) fn perform(game: &mut Game<'_>, source: &CommandSource, call: &CommandInvocation, ran: &mut bool) {
    let CommandSource::Player { .. } = source else { return };
    let command = line(call);
    let mut words = command.split_whitespace();
    if *ran || words.next() != Some("oritech") {
        return;
    }
    *ran = true;
    let reply = match words.next() {
        Some("recipes") => {
            let b = books(game);
            let parts: Vec<String> = b.counts.iter().map(|(t, (total, read))| format!("{t} {read}/{total}")).collect();
            let (total, read) = b.counts.values().fold((0, 0), |a, c| (a.0 + c.0, a.1 + c.1));
            format!("Oritech recipes read: {read} of {total} ({})", parts.join(", "))
        }
        _ => "usage: /oritech recipes".to_owned(),
    };
    game.native_feedback(Text::literal(reply));
}
