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
use horizon_core::geometry::Direction;
use horizon_core::nbt::{Compound, JavaString, Tag};
use mod_api::{Command, CommandArgumentValue, CommandInvocation, CommandSource, ModBlockEntity, NativeFeInfo, WorldPhase};
use oritech_core::{
    config::Config,
    energy::Energy,
    generator::{GenCtx, Generator},
    machine::{self, Face, Kind, Spec},
    laser::{self, Behavior, Cell, Laser, LaserCtx, LaserWorld},
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
        .filter(|t| !t.is_empty())
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
    for spec in machine::ALL.iter().filter(|s| !s.recipe_type.is_empty()) {
        let table = game.native_recipes_of_type(spec.recipe_type);
        let mut read = Vec::new();
        for r in table.iter() {
            if spec.kind == Kind::PoweredFurnace {
                // `RecipeType.SMELTING`: one ingredient, one result, the cooking time for the time.
                if let ([ingredient], [result]) = (r.ingredients.as_slice(), r.results.as_slice()) {
                    let mut items: Vec<String> = ingredient.items.iter().cloned().collect();
                    items.sort();
                    if !items.is_empty() {
                        read.push(recipe::Recipe {
                            id: r.id.clone(),
                            kind: r.kind.clone(),
                            time: r.cooking_time.unwrap_or(200) as i32,
                            inputs: vec![recipe::Ingredient { items }],
                            results: vec![Stack::new(&result.item.to_string(), result.count)],
                            fluids: false,
                        });
                    }
                }
                continue;
            }
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
/// `UpgradableGeneratorBlockEntity`'s `currentMaxBurnTime`.
const STORED_BURN: &str = "storedBurn";
const CORES: &str = "connectedCores";
const QUALITY: &str = "coreQuality";
/// The atomic forge's buffer, which its recipe sizes (Java keeps it in the energy storage's capacity).
const FORGE_CAP: &str = "oritech.forge_capacity";
/// The laser arm's search direction and target (`BlockPos.asLong`; absent for none), the break energy the target needs,
/// and a flag set when a new direction came in, so the next tick looks for a target at once.
const LASER_DIRECTION: &str = "oritech.laser_direction";
const LASER_TARGET: &str = "oritech.laser_target";
const LASER_NEEDED: &str = "oritech.laser_needed";
const LASER_FRESH: &str = "oritech.laser_fresh";

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
    if spec.generator {
        nbt.insert(key(STORED_BURN), Tag::Int(0));
    }
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
    let mut capacity = m.energy_capacity;
    if spec.kind == Kind::AtomicForge && number(nbt, FORGE_CAP) > 0 {
        capacity = number(nbt, FORGE_CAP);
    }
    Energy { amount: number(nbt, ENERGY), capacity, max_insert: m.max_energy_insertion, max_extract: m.max_energy_extraction }
}

/// A multiblock's cores: `(assembled, missing)`. The controller assembles once every core position holds a machine
/// core (`initMultiblock`, the quality their average) and comes apart when one is gone (`onCoreBroken`).
fn check_cores(game: &Game<'_>, pos: [i32; 3], spec: &Spec, nbt: &mut Compound) -> (bool, i32) {
    let facing = game.native_block_property(pos, "facing").as_deref().and_then(Facing::from_name).unwrap_or(Facing::North);
    // `getFacingForMultiblock` of the laser arm is the opposite of its `FACING`.
    let facing = if spec.kind == Kind::LaserArm { opposite(facing) } else { facing };
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
    if spec.generator {
        return generator_tick(game, pos, entity, spec);
    }
    if spec.kind == Kind::LaserArm {
        return laser_tick(game, pos, entity, spec);
    }
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
    if spec.kind == Kind::PoweredFurnace {
        // `getSpeedMultiplier` is the addons' times `furnaceData.speedMultiplier`.
        m.addons.speed = cfg.furnace_speed as f32;
    }
    let held = engine_items(game, entity);
    let before = core_slots(&held, spec.inventory_size);
    let mut slots = before.clone();
    let changed = {
        let max = |item: &str| game.native_max_stack_size(item).unwrap_or(64);
        let compact = |small: &str, base: &str| compacts(game, &books, small, base);
        let ctx = Ctx {
            book: &book,
            // `calculateEnergyUsage` of the furnace halves it (its speed multiplier already counts in the division).
            energy_per_tick: cfg.machine(spec.kind).energy_per_tick / if spec.kind == Kind::PoweredFurnace { 2 } else { 1 },
            burst_speed_multiplier: cfg.burst_speed_multiplier as f32,
            burst_throttle_multiplier: cfg.burst_throttle_multiplier as f32,
            game_time: 0,
            active,
            max_stack: &max,
            compact: &compact,
        };
        let changed = if spec.kind == Kind::AtomicForge { m.tick_forge(spec, &mut slots, &ctx) } else { m.tick(spec, &mut slots, &ctx) };
        let total = m.current.as_deref().and_then(|id| book.get(id)).map_or(0, |r| (r.time as f32 * m.speed_multiplier(&ctx)).ceil() as i32);
        entity.data = MachineView {
            energy: m.energy.amount,
            capacity: m.energy.capacity,
            insert: m.energy.max_insert,
            usage: m.energy_usage(&ctx).round() as i64,
            // The forge's bar is its buffer: a recipe's energy is its time.
            progress: if spec.kind == Kind::AtomicForge { forge_percent(&m) } else { m.progress },
            total: if spec.kind == Kind::AtomicForge { i32::from(m.energy.capacity > 10) * 100 } else { total },
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
        if spec.kind == Kind::AtomicForge {
            entity.nbt.insert(key(FORGE_CAP), Tag::Long(m.energy.capacity));
        }
    }
    LIVE.with(|l| {
        l.borrow_mut().insert(at, Live { current: m.current, burst: m.burst, last_worked_at: m.last_worked_at });
    });
}

fn core_quality_of(block: &str) -> Option<f32> {
    multiblock::core_quality(block)
}

fn forge_percent(m: &Machine) -> i32 {
    if m.energy.capacity > 10 { (m.energy.amount * 100 / m.energy.capacity).clamp(0, 100) as i32 } else { 0 }
}

fn opposite(f: Facing) -> Facing {
    match f {
        Facing::North => Facing::South,
        Facing::South => Facing::North,
        Facing::West => Facing::East,
        Facing::East => Facing::West,
    }
}

// ---- The laser arm ----------------------------------------------------------------------------------------------

fn long_pos(nbt: &Compound, name: &str) -> Option<[i32; 3]> {
    match nbt.get(&key(name)) {
        Some(Tag::Long(v)) => Some(laser::unpack(*v)),
        _ => None,
    }
}

fn put_pos(nbt: &mut Compound, name: &str, pos: Option<[i32; 3]>) {
    match pos {
        Some(p) => nbt.insert(key(name), Tag::Long(laser::pack(p))),
        None => nbt.insert(key(name), Tag::Long(0)),
    };
}

fn is_air(block: &str) -> bool {
    matches!(block, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
}

/// The level a laser reads and acts on (`LaserWorld`), with the drops of what it broke.
struct Beam<'a, 'g> {
    game: &'a mut Game<'g>,
    book: &'a RecipeBook,
    drops: Vec<Stack>,
}

impl Beam<'_, '_> {
    fn passes(&self, block: &str) -> bool {
        is_air(block)
            || matches!(block, "minecraft:water" | "minecraft:lava" | "minecraft:bubble_column")
            || self.game.native_block_name_in_tag(block, "oritech:laser_passthrough").unwrap_or(false)
    }

    /// The controller of the machine whose assembled cores include `pos`.
    fn controller(&self, pos: [i32; 3]) -> Option<[i32; 3]> {
        core_quality_of(&self.game.native_block(pos))?;
        for dx in -2..=2 {
            for dy in -2..=2 {
                for dz in -2..=2 {
                    let at = [pos[0] + dx, pos[1] + dy, pos[2] + dz];
                    let Some((spec, nbt)) = self.machine_at(at) else { continue };
                    if !spec.is_multiblock() || cores(&nbt).is_empty() {
                        continue;
                    }
                    let facing = self.game.native_block_property(at, "facing").as_deref().and_then(Facing::from_name).unwrap_or(Facing::North);
                    let facing = if spec.kind == Kind::LaserArm { opposite(facing) } else { facing };
                    if multiblock::core_positions(at, facing, spec.cores).contains(&pos) {
                        return Some(at);
                    }
                }
            }
        }
        None
    }

    fn machine_at(&self, pos: [i32; 3]) -> Option<(&'static Spec, Compound)> {
        let entity = self.game.native_mod_block_entity(pos)?;
        Some((machine::of_block(entity.id())?, entity.nbt.clone()))
    }
}

impl LaserWorld for Beam<'_, '_> {
    fn cell(&self, pos: [i32; 3]) -> Cell {
        let block = self.game.native_block(pos);
        if block == "minecraft:target" {
            Cell::Terminator
        } else if self.passes(&block) {
            Cell::Pass
        } else {
            Cell::Solid
        }
    }
    fn controller_of(&self, pos: [i32; 3]) -> Option<[i32; 3]> {
        self.controller(pos)
    }
    fn hardness(&self, pos: [i32; 3]) -> f32 {
        self.game.native_destroy_speed(pos).unwrap_or(-1.0)
    }
    fn is_air(&self, pos: [i32; 3]) -> bool {
        is_air(&self.game.native_block(pos))
    }
    fn behavior(&self, pos: [i32; 3]) -> Behavior {
        let block = self.game.native_block(pos);
        if block == "minecraft:target" || block == "minecraft:bedrock" {
            Behavior::Noop
        } else if self.machine_at(pos).is_some_and(|(s, _)| s.kind != Kind::LaserArm) {
            Behavior::Energy
        } else if block == "minecraft:budding_amethyst" || self.game.native_block_name_in_tag(&block, "oritech:laser_accelerated").unwrap_or(false) {
            Behavior::Accelerated
        } else if self.passes(&block) {
            Behavior::PassThrough
        } else {
            Behavior::Break
        }
    }
    fn energy_of(&self, pos: [i32; 3]) -> Option<(i64, i64)> {
        let (spec, nbt) = self.machine_at(pos)?;
        let e = energy_of(&settings(), spec, &nbt);
        Some((e.amount, e.capacity))
    }
    fn insert_energy(&mut self, pos: [i32; 3], amount: i64) -> i64 {
        let cfg = settings();
        self.game
            .native_with_mod_block_entity(pos, |m| {
                let spec = machine::of_block(m.id())?;
                let e = energy_of(&cfg, spec, &m.nbt);
                // `insertIgnoringLimit`: only the room limits it.
                let took = amount.min(e.capacity - e.amount).max(0);
                m.nbt.insert(key(ENERGY), Tag::Long(e.amount + took));
                Some(took)
            })
            .flatten()
            .unwrap_or(0)
    }
    fn break_block(&mut self, pos: [i32; 3]) {
        let block = self.game.native_block(pos);
        let mut dropped: Vec<Stack> = self
            .game
            .native_machine_destroy_block(pos, None, true)
            .unwrap_or_default()
            .iter()
            .map(|s| Stack::new(&s.item.to_string(), s.count))
            .collect();
        // `tryGetRecipeOfBlock`: a block with a laser recipe gives that recipe's item instead.
        if let Some(r) = self.book.recipes.iter().find(|r| r.inputs.first().is_some_and(|i| i.items.contains(&block))) {
            if let Some(result) = r.results.first() {
                dropped = vec![Stack::new(&result.item, 1)];
            }
        }
        self.drops.extend(dropped);
    }
}

/// The laser's inventory takes a stack into the first stack of its item with room, else an empty slot; what doesn't
/// fit is lost (`finishBlockBreaking`: "this will discard items that wont fit anymore").
fn store(slots: &mut [Option<Stack>], stack: Stack, max: u32) {
    let mut left = stack.count;
    for slot in slots.iter_mut().flatten() {
        if slot.item == stack.item && slot.count < max {
            let moved = left.min(max - slot.count);
            slot.count += moved;
            left -= moved;
        }
    }
    for slot in slots.iter_mut() {
        if left == 0 {
            break;
        }
        if slot.is_none() {
            let moved = left.min(max);
            *slot = Some(Stack::new(&stack.item, moved));
            left -= moved;
        }
    }
}

/// One server tick of a laser arm (`LaserArmBlockEntity.serverTick`).
fn laser_tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, spec: &'static Spec) {
    let cfg = settings();
    let books = books(game);
    let book = books.books.get(&spec.kind).cloned().unwrap_or_default();
    let (active, missing) = check_cores(game, pos, spec, &mut entity.nbt);
    let facing = game.native_block_property(pos, "facing").as_deref().and_then(Facing::from_name).unwrap_or(Facing::North);
    let off = multiblock::rotate([-1, 0, 0], facing);
    let head = [pos[0] + off[0], pos[1] + off[1], pos[2] + off[2]];
    let mut l = Laser::new(energy_of(&cfg, spec, &entity.nbt));
    l.progress = number(&entity.nbt, PROGRESS) as i32;
    l.target = long_pos(&entity.nbt, LASER_TARGET).filter(|p| *p != [0, 0, 0]);
    l.direction = long_pos(&entity.nbt, LASER_DIRECTION).filter(|p| *p != [0, 0, 0]);
    l.needed = match number(&entity.nbt, LASER_NEEDED) {
        0 => 1024,
        n => n as i32,
    };
    l.disabled_via_redstone = number(&entity.nbt, REDSTONE) != 0;
    let held = engine_items(game, entity);
    let before = core_slots(&held, spec.inventory_size);
    let mut slots = before.clone();
    let fresh = number(&entity.nbt, LASER_FRESH) != 0;
    let time = game.native_day_time();
    let ctx = LaserCtx { settings: &cfg.laser, hardness_exponent: cfg.hardness_exponent, pos, head, game_time: time, speed: 1.0, efficiency: 1.0 };
    let mut beam = Beam { game, book: &book, drops: Vec::new() };
    let mut changed = false;
    if active {
        if fresh {
            l.find_next(&ctx, &beam);
            changed = true;
        }
        changed |= l.tick(&ctx, &mut beam);
    }
    let drops = std::mem::take(&mut beam.drops);
    for stack in drops {
        let max = game.native_max_stack_size(&stack.item).unwrap_or(64);
        store(&mut slots, stack, max);
        changed = true;
    }
    entity.data = MachineView {
        energy: l.energy.amount,
        capacity: l.energy.capacity,
        insert: l.energy.max_insert,
        usage: Laser::to_fire(&ctx),
        progress: l.progress,
        total: l.needed,
        speed: 100,
        efficiency: 100,
        active,
        missing,
    }
    .pack();
    if changed {
        if slots != before {
            put_back(entity, &held, &before, &slots);
        }
        entity.nbt.insert(key(PROGRESS), Tag::Int(l.progress));
        entity.nbt.insert(key(ENERGY), Tag::Long(l.energy.amount));
        entity.nbt.insert(key(LASER_NEEDED), Tag::Int(l.needed));
        put_pos(&mut entity.nbt, LASER_TARGET, l.target);
        entity.nbt.insert(key(LASER_FRESH), Tag::Byte(0));
    }
}

/// The direction of a neighbour's face the energy goes in by.
fn direction(face: Face) -> Direction {
    match face {
        Face::Down => Direction::Down,
        Face::North => Direction::North,
        Face::South => Direction::South,
        Face::West => Direction::West,
        Face::East => Direction::East,
    }
}

/// One server tick of a generator (`UpgradableGeneratorBlockEntity.serverTick`): burn, then give the buffer to the
/// blocks around (`outputEnergy`), then what its screen shows.
fn generator_tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, spec: &'static Spec) {
    let cfg = settings();
    let books = books(game);
    let book = books.books.get(&spec.kind).cloned().unwrap_or_default();
    let (active, missing) = if spec.is_multiblock() { check_cores(game, pos, spec, &mut entity.nbt) } else { (true, 0) };
    let mut g = Generator::new(energy_of(&cfg, spec, &entity.nbt));
    g.progress = number(&entity.nbt, PROGRESS) as i32;
    g.max_burn = number(&entity.nbt, STORED_BURN) as i32;
    g.disabled_via_redstone = number(&entity.nbt, REDSTONE) != 0;
    let held = engine_items(game, entity);
    let before = core_slots(&held, spec.inventory_size);
    let mut slots = before.clone();
    let fuel = |item: &str| game.native_burn_time(&ItemStack::new(item, 1));
    let ctx = GenCtx { energy_per_tick: cfg.machine(spec.kind).energy_per_tick, speed: 1.0, efficiency: 1.0, active, fuel: &fuel, book: &book };
    let burnt = g.tick(spec, &mut slots, &ctx);
    let produced = Generator::produced(&ctx).round() as i64;
    let dimension = game.native_dimension();
    let moved = g.output(spec.outputs.len(), &mut |target, amount| {
        let (offset, face) = spec.outputs[target];
        let at = [pos[0] + offset[0], pos[1] + offset[1], pos[2] + offset[2]];
        game.native_fe_offer(dimension, at, Some(direction(face)), amount, false, false)
    });
    entity.data = MachineView {
        energy: g.energy.amount,
        capacity: g.energy.capacity,
        insert: g.energy.max_extract,
        usage: produced,
        progress: g.max_burn - g.progress,
        total: g.max_burn,
        speed: 100,
        efficiency: 100,
        active,
        missing,
    }
    .pack();
    if burnt || moved {
        if slots != before {
            put_back(entity, &held, &before, &slots);
        }
        entity.nbt.insert(key(PROGRESS), Tag::Int(g.progress));
        entity.nbt.insert(key(STORED_BURN), Tag::Int(g.max_burn));
        entity.nbt.insert(key(ENERGY), Tag::Long(g.energy.amount));
    }
}

// ---- Forge Energy -----------------------------------------------------------------------------------------------

/// `native_fe_storage`: every side takes (`getEnergyStorage(direction)` is the one buffer) and none gives; a
/// generator gives and takes none.
pub(super) fn fe_storage(entity: &ModBlockEntity, info: &mut Option<NativeFeInfo>) {
    let Some(spec) = machine::of_block(entity.id()) else { return };
    if info.is_some() {
        return;
    }
    let e = energy_of(&settings(), spec, &entity.nbt);
    *info = Some(NativeFeInfo { stored: e.amount, capacity: e.capacity, can_receive: e.max_insert > 0, can_extract: e.max_extract > 0 });
}

/// `native_fe_extract`: `DynamicEnergyStorage.extract`, up to `maxExtract` a call (a generator's).
pub(super) fn fe_extract(entity: &mut ModBlockEntity, max: i64, simulate: bool, given: &mut Option<i64>) {
    let Some(spec) = machine::of_block(entity.id()) else { return };
    if given.is_some() {
        return;
    }
    let mut e = energy_of(&settings(), spec, &entity.nbt);
    let gave = e.extract(max.max(0), simulate);
    if gave > 0 && !simulate {
        entity.nbt.insert(key(ENERGY), Tag::Long(e.amount));
    }
    *given = Some(gave);
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
            // The powered furnace's vanilla smelting recipes are not Oritech's own.
            let own = b.counts.iter().filter(|(t, _)| t.starts_with("oritech:"));
            let (total, read) = own.fold((0, 0), |a, (_, c)| (a.0 + c.0, a.1 + c.1));
            let parts: Vec<String> = b.counts.iter().map(|(t, (total, read))| format!("{t} {read}/{total}")).collect();
            format!("Oritech recipes read: {read} of {total} ({})", parts.join(", "))
        }
        // The target designator's job: `/oritech aim <laser x y z> <target x y z>` points a laser arm at a block.
        Some("aim") => {
            let n: Vec<i32> = words.filter_map(|w| w.parse().ok()).collect();
            match n.as_slice() {
                [x, y, z, tx, ty, tz] => {
                    let set = game.native_with_mod_block_entity([*x, *y, *z], |m| {
                        let laser = machine::of_block(m.id()).is_some_and(|s| s.kind == Kind::LaserArm);
                        if laser {
                            put_pos(&mut m.nbt, LASER_DIRECTION, Some([*tx, *ty, *tz]));
                            m.nbt.insert(key(LASER_FRESH), Tag::Byte(1));
                        }
                        laser
                    });
                    if set == Some(true) { "laser arm aimed".to_owned() } else { "no laser arm there".to_owned() }
                }
                _ => "usage: /oritech aim <x y z> <target x y z>".to_owned(),
            }
        }
        _ => "usage: /oritech recipes | aim".to_owned(),
    };
    game.native_feedback(Text::literal(reply));
}
