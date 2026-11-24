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
use horizon_world::{fluid::FluidKind, fluid_stack::{self, FluidStack}};
use mod_api::{Command, CommandArgumentValue, CommandInvocation, CommandSource, ModBlockEntity, NativeFeInfo, WorldPhase};
use oritech_core::{
    config::Config,
    energy::Energy,
    fluid::Tank,
    fluid_machine::{self, FluidCtx, FluidMachine},
    generator::{GenCtx, Generator},
    machine::{self, Face, Kind, Spec},
    laser::{self, Behavior, Cell, Laser, LaserCtx, LaserWorld},
    multiblock::{self, Facing},
    process::{AddonData, Ctx, Machine},
    recipe::{self, RecipeBook, Stack},
    view::{self, MachineView, TankView},
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
    /// The refinery's active modules (`moduleCount`, counted every 25 ticks).
    modules: i32,
    seen: bool,
    /// Ticks the refinery has run since the world opened (its modules are counted every 25).
    age: i64,
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
    /// `oritech:centrifuge_fluid`, the recipes of a centrifuge with a fluid addon.
    fluid_centrifuge: Rc<RecipeBook>,
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
        .chain(["minecraft:crafting", "oritech:centrifuge_fluid"])
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
            fluid_in: None,
            fluid_out: Vec::new(),
                        });
                    }
                }
                continue;
            }
            match recipe::parse(&r.id, &r.json, |v| items_of(game, v)) {
                // Java's codec fails a result of an item the game doesn't have, or a fluid it lacks.
                Ok(p) if (fluid_type(spec.kind) || !p.fluids) && results_known(game, &p) => read.push(p),
                Ok(_) => log::debug!("[oritech] {}: a result or fluid it can't make", r.id),
                Err(e) => log::debug!("[oritech] {}: {e}", r.id),
            }
        }
        counts.insert(spec.recipe_type, (table.len(), read.len()));
        books.insert(spec.kind, Rc::new(RecipeBook::new(read)));
    }
    let table = game.native_recipes_of_type("oritech:centrifuge_fluid");
    let fluid_read: Vec<recipe::Recipe> = table
        .iter()
        .filter_map(|r| recipe::parse(&r.id, &r.json, |v| items_of(game, v)).ok().filter(|p| results_known(game, p)))
        .collect();
    counts.insert("oritech:centrifuge_fluid", (table.len(), fluid_read.len()));
    log::info!("[oritech] recipes read: {counts:?}");
    Books { stamp, books, fluid_centrifuge: Rc::new(RecipeBook::new(fluid_read)), counts, compact: RefCell::new(HashMap::new()) }
}

/// The machines whose recipes name fluids (their own types).
fn fluid_type(kind: Kind) -> bool {
    matches!(kind, Kind::Refinery | Kind::Cooler | Kind::FuelGenerator | Kind::LavaGenerator)
}

/// A recipe's result items and fluids all exist in this game (Java's codec fails the recipe otherwise); a fluid tag
/// needs no check.
fn results_known(game: &Game<'_>, p: &recipe::Recipe) -> bool {
    p.results.iter().all(|s| game.native_max_stack_size(&s.item).is_some())
        && p.fluid_out.iter().all(|(f, _)| FluidStack::of(f, 1).is_some())
        && p.fluid_in.as_ref().is_none_or(|f| f.tag || FluidStack::of(&f.fluid, 1).is_some())
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
/// `CentrifugeBlockEntity.hasFluidAddon` (saved as `fluidAddon`) and the block that sets it.
const FLUID_ADDON_FLAG: &str = "fluidAddon";
const FLUID_ADDON: &str = "oritech:machine_fluid_addon";
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
/// the outputs on every side. A refinery module has no inventory and no menu.
pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    let Some(spec) = machine::of_block(block) else { return };
    if container.is_some() || spec.kind == Kind::RefineryModule {
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
    if spec.kind == Kind::RefineryModule {
        check_cores(game, pos, spec, &mut entity.nbt);
        return;
    }
    if spec.kind == Kind::Centrifuge {
        // A fluid addon beside it (`hasFluidAddon`) makes it a fluid machine; without one it runs its item recipes.
        let facing = game.native_block_property(pos, "facing").as_deref().and_then(Facing::from_name).unwrap_or(Facing::North);
        let has = spec.addon_slots.iter().any(|o| {
            let r = multiblock::rotate(*o, facing);
            game.native_block([pos[0] + r[0], pos[1] + r[1], pos[2] + r[2]]) == FLUID_ADDON
        });
        let was = number(&entity.nbt, FLUID_ADDON_FLAG) != 0;
        if has != was {
            entity.nbt.insert(key(FLUID_ADDON_FLAG), Tag::Byte(i8::from(has)));
        }
        if has {
            return fluid_tick(game, pos, entity, spec);
        }
    } else if matches!(spec.kind, Kind::Refinery | Kind::Cooler) {
        return fluid_tick(game, pos, entity, spec);
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
        l.borrow_mut().insert(at, Live { current: m.current, burst: m.burst, last_worked_at: m.last_worked_at, modules: 0, seen: true, age: 0 });
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
    if matches!(spec.kind, Kind::FuelGenerator | Kind::LavaGenerator) {
        return fluid_generator_tick(game, pos, entity, spec);
    }
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

// ---- Fluids -----------------------------------------------------------------------------------------------------

/// A tank of a block entity as it is saved: the `FluidStack` codec's compound under a key (no key when empty).
fn read_tank(nbt: &Compound, name: &str) -> Tank {
    match nbt.get(&key(name)).and_then(FluidStack::parse) {
        Some(s) if !s.is_empty() => Tank::new(s.name(), s.amount()),
        _ => Tank::default(),
    }
}

fn write_tank(nbt: &mut Compound, name: &str, tank: &Tank) {
    match FluidStack::of(&tank.fluid, tank.amount).filter(|s| !s.is_empty()).and_then(|s| s.save()) {
        Some(saved) => nbt.insert(key(name), Tag::Compound(saved)),
        None => nbt.remove(&key(name)),
    };
}

/// A tank on the screen: the game's number for its fluid (`FluidKind::tick_id`, good within the process the screen
/// is drawn in).
fn tank_view(tank: &Tank, capacity: i32) -> TankView {
    let fluid = if tank.is_empty() { 0 } else { FluidKind::from_name(&tank.fluid).map_or(0, |k| k.tick_id() as i32) };
    TankView { amount: tank.amount.max(0), capacity, fluid }
}

/// The tags a fluid is in (`FluidStack.is(tag)`).
fn in_tag(fluid: &str, tag: &str) -> bool {
    FluidStack::of(fluid, 1).is_some_and(|s| s.is_tag(tag))
}

/// The keys a machine saves its tanks under (`SimpleInOutFluidStorage.writeNbt`: `fluidin` and `fluidout` and the
/// suffix; `SimpleFluidStorage`: `fluid`).
fn tank_keys(kind: Kind) -> (&'static str, &'static [&'static str]) {
    match kind {
        Kind::Centrifuge => ("fluidin", &["fluidout"]),
        Kind::Refinery => ("fluidinmain", &["fluidoutmain"]),
        _ => ("fluid", &[]),
    }
}

/// The refinery module's tank (`nodeA` and `nodeB` of the refinery, kept in the module).
const MODULE_TANK: &str = "fluid";
const MODULE_CAPACITY: i32 = 4000;

/// A refinery's active modules: the blocks 2 and 3 above it that are assembled modules, the second counting only
/// behind the first (`refreshModules`).
fn count_modules(game: &mut Game<'_>, pos: [i32; 3]) -> i32 {
    let mut count = 0;
    for i in 0..=1 {
        let at = [pos[0], pos[1] + 2 + i, pos[2]];
        let active = game
            .native_with_mod_block_entity(at, |m| m.id() == "oritech:refinery_module_block" && !cores(&m.nbt).is_empty())
            .unwrap_or(false);
        if !active {
            break;
        }
        count += 1;
    }
    count
}

/// One server tick of a machine that holds fluids: the refinery, the cooler and a centrifuge with a fluid addon.
fn fluid_tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, spec: &'static Spec) {
    let cfg = settings();
    let books = books(game);
    let own = books.books.get(&spec.kind).cloned().unwrap_or_default();
    let (book, item_book) = if spec.kind == Kind::Centrifuge { (books.fluid_centrifuge.clone(), Some(own.clone())) } else { (own.clone(), None) };
    let (active, missing) = check_cores(game, pos, spec, &mut entity.nbt);
    let layout = fluid_machine::layout(spec.kind, cfg.centrifuge_tank_buckets);
    let at = (game.native_dimension(), pos);
    let live = LIVE.with(|l| l.borrow().get(&at).cloned()).unwrap_or_default();
    let mut m = FluidMachine::new(energy_of(&cfg, spec, &entity.nbt), layout.clone());
    m.progress = number(&entity.nbt, PROGRESS) as i32;
    m.disabled_via_redstone = number(&entity.nbt, REDSTONE) != 0;
    m.current = live.current;
    let (in_key, out_keys) = tank_keys(spec.kind);
    m.input = read_tank(&entity.nbt, in_key);
    for (i, k) in out_keys.iter().enumerate() {
        m.outputs[i] = read_tank(&entity.nbt, k);
    }
    // The refinery's modules: counted every 25 ticks, their tanks its second and third output.
    let mut modules = live.modules;
    let age = live.age + 1;
    if spec.kind == Kind::Refinery {
        if !live.seen || age % 25 == 0 {
            modules = count_modules(game, pos);
        }
        m.modules = modules;
        for i in 0..modules.clamp(0, 2) as usize {
            let at = [pos[0], pos[1] + 2 + i as i32, pos[2]];
            m.outputs[1 + i] = game.native_with_mod_block_entity(at, |b| read_tank(&b.nbt, MODULE_TANK)).unwrap_or_default();
        }
    }
    let held = engine_items(game, entity);
    let before = core_slots(&held, spec.inventory_size);
    let mut slots = before.clone();
    let before_tanks = (m.input.clone(), m.outputs.clone());
    let changed = {
        let max = |item: &str| game.native_max_stack_size(item).unwrap_or(64);
        let ctx = FluidCtx {
            book: &book,
            item_book: item_book.as_deref(),
            energy_per_tick: cfg.machine(spec.kind).energy_per_tick,
            speed: 1.0,
            efficiency: 1.0,
            game_time: 0,
            active,
            max_stack: &max,
            tag: &in_tag,
        };
        let changed = m.tick(spec, &mut slots, &ctx);
        let total = m
            .current
            .as_deref()
            .and_then(|id| book.get(id).or_else(|| item_book.as_deref().and_then(|b| b.get(id))))
            .map_or(0, |r| (r.time as f32 * ctx.speed).ceil() as i32);
        let mut data = MachineView {
            energy: m.energy.amount,
            capacity: m.energy.capacity,
            insert: m.energy.max_insert,
            usage: m.energy_usage(&ctx).round() as i64,
            progress: m.progress,
            total,
            speed: 100,
            efficiency: 100,
            active,
            missing,
        }
        .pack();
        let mut tanks = vec![tank_view(&m.input, layout.input)];
        for (i, cap) in layout.outputs.iter().enumerate() {
            // A module's tank only shows with its module.
            let shown = if spec.kind == Kind::Refinery && i > 0 && i as i32 > modules { 0 } else { *cap };
            tanks.push(tank_view(&m.outputs[i], shown));
        }
        if spec.kind == Kind::Cooler {
            tanks.truncate(1);
        }
        view::pack_tanks(&mut data, &tanks);
        entity.data = data;
        changed
    };
    if changed {
        if slots != before {
            put_back(entity, &held, &before, &slots);
        }
        entity.nbt.insert(key(PROGRESS), Tag::Int(m.progress));
        entity.nbt.insert(key(ENERGY), Tag::Long(m.energy.amount));
    }
    if (m.input.clone(), m.outputs.clone()) != before_tanks {
        write_tank(&mut entity.nbt, in_key, &m.input);
        for (i, k) in out_keys.iter().enumerate() {
            write_tank(&mut entity.nbt, k, &m.outputs[i]);
        }
        for i in 0..modules.clamp(0, 2) as usize {
            if m.outputs[1 + i] != before_tanks.1[1 + i] {
                let at = [pos[0], pos[1] + 2 + i as i32, pos[2]];
                let tank = m.outputs[1 + i].clone();
                game.native_with_mod_block_entity(at, |b| write_tank(&mut b.nbt, MODULE_TANK, &tank));
            }
        }
    }
    LIVE.with(|l| {
        l.borrow_mut().insert(at, Live { current: m.current, burst: 0, last_worked_at: 0, modules, seen: true, age });
    });
}

/// `FuelGeneratorEntity.getOutputTargets`: the two blocks behind the cores beside its back, each given the face
/// pointing away from the machine (as Java does).
fn fuel_targets(game: &Game<'_>, pos: [i32; 3]) -> Vec<([i32; 3], Face)> {
    let facing = game.native_block_property(pos, "facing").as_deref().and_then(Facing::from_name).unwrap_or(Facing::North);
    let a = multiblock::rotate([1, 0, -1], facing);
    let core = multiblock::rotate([0, 0, -1], facing);
    let b = multiblock::rotate([1, 0, 1], facing);
    let away = [a[0] - core[0], a[1] - core[1], a[2] - core[2]];
    let face = match away {
        [1, 0, 0] => Face::East,
        [-1, 0, 0] => Face::West,
        [0, 0, 1] => Face::South,
        _ => Face::North,
    };
    vec![(a, face), (b, face)]
}

/// One server tick of the fuel and lava generators (`FluidMultiblockGeneratorBlockEntity`): the bucket in the input
/// slot, then the burn of the tank's fluid, then the energy out.
fn fluid_generator_tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, spec: &'static Spec) {
    let cfg = settings();
    let books = books(game);
    let book = books.books.get(&spec.kind).cloned().unwrap_or_default();
    let (active, missing) = check_cores(game, pos, spec, &mut entity.nbt);
    let mut g = Generator::new(energy_of(&cfg, spec, &entity.nbt));
    g.progress = number(&entity.nbt, PROGRESS) as i32;
    g.max_burn = number(&entity.nbt, STORED_BURN) as i32;
    g.disabled_via_redstone = number(&entity.nbt, REDSTONE) != 0;
    let held = engine_items(game, entity);
    let before = core_slots(&held, spec.inventory_size);
    let mut slots = before.clone();
    let tank_before = read_tank(&entity.nbt, MODULE_TANK);
    let mut tank = tank_before.clone();
    let capacity = fluid_machine::layout(spec.kind, 0).input;
    let max = |item: &str| game.native_max_stack_size(item).unwrap_or(64);
    // `FluidApi.ITEM.find`: a fluid bucket holds its fluid, the empty one holds none.
    let bucket = |item: &str| -> Option<(Option<String>, String)> {
        if item == "minecraft:bucket" {
            return Some((None, item.to_owned()));
        }
        fluid_stack::bucket_fluid(item).map(|k| (Some(k.name().to_owned()), "minecraft:bucket".to_owned()))
    };
    let mut changed = false;
    if active {
        changed |= fluid_machine::process_buckets(&mut slots, &mut tank, capacity, &bucket, &max);
    }
    let fuel = |_: &str| 0;
    let ctx = GenCtx { energy_per_tick: cfg.machine(spec.kind).energy_per_tick, speed: 1.0, efficiency: 1.0, active, fuel: &fuel, book: &book };
    // `getRecipe(fluidStorage)`: the first recipe whose fluid the tank holds enough of.
    let find = |t: &Tank| {
        book.recipes
            .iter()
            .find(|r| r.fluid_in.as_ref().is_some_and(|f| f.amount > 0) && r.fluid_matches(t, &in_tag))
            .map(|r| (r.time, r.fluid_in.as_ref().map_or(0, |f| f.amount)))
    };
    let burnt = g.tick_fluid(&mut tank, &ctx, &find);
    let produced = Generator::produced(&ctx).round() as i64;
    let dimension = game.native_dimension();
    let targets: Vec<([i32; 3], Face)> = if spec.kind == Kind::FuelGenerator { fuel_targets(game, pos) } else { spec.outputs.to_vec() };
    let moved = g.output(targets.len(), &mut |target, amount| {
        let (offset, face) = targets[target];
        let at = [pos[0] + offset[0], pos[1] + offset[1], pos[2] + offset[2]];
        game.native_fe_offer(dimension, at, Some(direction(face)), amount, false, false)
    });
    let mut data = MachineView {
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
    view::pack_tanks(&mut data, &[tank_view(&tank, capacity)]);
    entity.data = data;
    if changed || burnt || moved {
        if slots != before {
            put_back(entity, &held, &before, &slots);
        }
        entity.nbt.insert(key(PROGRESS), Tag::Int(g.progress));
        entity.nbt.insert(key(STORED_BURN), Tag::Int(g.max_burn));
        entity.nbt.insert(key(ENERGY), Tag::Long(g.energy.amount));
    }
    if tank != tank_before {
        write_tank(&mut entity.nbt, MODULE_TANK, &tank);
    }
}

/// A tank a block exposes to fluid handlers: where it is saved, its size, and whether handlers fill and drain it.
struct Exposed {
    key: &'static str,
    capacity: i32,
    fill: bool,
    drain: bool,
}

/// `getFluidStorage(direction)` of each machine (the same on every side): the in-out storage of a fluid centrifuge
/// and of the refinery (fill the input, drain the output), the single tank of the cooler, the generators and an
/// assembled refinery module.
fn exposed(spec: &Spec, nbt: &Compound) -> Vec<Exposed> {
    let layout = fluid_machine::layout(spec.kind, settings().centrifuge_tank_buckets);
    let io = |input: &'static str, output: &'static str| {
        vec![
            Exposed { key: input, capacity: layout.input, fill: true, drain: false },
            Exposed { key: output, capacity: layout.outputs[0], fill: false, drain: true },
        ]
    };
    match spec.kind {
        Kind::Centrifuge if number(nbt, FLUID_ADDON_FLAG) != 0 => io("fluidin", "fluidout"),
        Kind::Refinery => io("fluidinmain", "fluidoutmain"),
        Kind::RefineryModule if !cores(nbt).is_empty() => vec![Exposed { key: MODULE_TANK, capacity: MODULE_CAPACITY, fill: true, drain: true }],
        Kind::Cooler | Kind::FuelGenerator | Kind::LavaGenerator => vec![Exposed { key: "fluid", capacity: layout.input, fill: true, drain: true }],
        _ => Vec::new(),
    }
}

/// The API's stack of a tank (none when empty or the fluid has no key).
fn api_stack(tank: &Tank) -> Option<mod_api::FluidStack> {
    if tank.is_empty() {
        return None;
    }
    Some(mod_api::FluidStack::new(mod_api::__private::fluid_key(&tank.fluid)?, tank.amount.max(0) as u32))
}

/// `native_fluid_handler_tanks`.
pub(super) fn fluid_tanks(entity: &ModBlockEntity, tanks: &mut Option<Vec<mod_api::NativeFluidTankInfo>>) {
    let Some(spec) = machine::of_block(entity.id()) else { return };
    if tanks.is_some() {
        return;
    }
    let list = exposed(spec, &entity.nbt);
    if list.is_empty() {
        return;
    }
    *tanks = Some(
        list.iter()
            .map(|e| {
                let held = read_tank(&entity.nbt, e.key);
                mod_api::NativeFluidTankInfo { drainable: e.drain && !held.is_empty(), stack: api_stack(&held), capacity: e.capacity }
            })
            .collect(),
    );
}

/// `native_fluid_handler_fill`: into the first tank that takes fills, as `insertTo` does. A generator refuses water
/// (`FluidMultiblockGeneratorBlockEntity.fluidStorage`: it is the boiler's). A stack with components is another fluid to a
/// tank that holds none.
pub(super) fn fluid_fill(entity: &mut ModBlockEntity, resource: &mod_api::FluidStack, simulate: bool, filled: &mut Option<i32>) {
    let Some(spec) = machine::of_block(entity.id()) else { return };
    if filled.is_some() {
        return;
    }
    let list = exposed(spec, &entity.nbt);
    let Some(e) = list.iter().find(|e| e.fill) else { return };
    let name = resource.key().and_then(mod_api::__private::fluid_id).unwrap_or_default();
    let amount = i32::try_from(resource.amount).unwrap_or(i32::MAX);
    if name.is_empty() || amount <= 0 || resource.has_components() || (matches!(spec.kind, Kind::FuelGenerator | Kind::LavaGenerator) && name == "minecraft:water") {
        *filled = Some(0);
        return;
    }
    let mut tank = read_tank(&entity.nbt, e.key);
    let put = tank.insert(name, amount, e.capacity, simulate);
    if put > 0 && !simulate {
        write_tank(&mut entity.nbt, e.key, &tank);
    }
    *filled = Some(put);
}

/// `native_fluid_handler_drain`: from the first tank that gives (`extractFrom`: the fluid asked for, or whatever
/// is held, up to `max`).
pub(super) fn fluid_drain(entity: &mut ModBlockEntity, resource: Option<&mod_api::FluidStack>, max: i32, simulate: bool, drained: &mut Option<mod_api::FluidStack>) {
    let Some(spec) = machine::of_block(entity.id()) else { return };
    if drained.is_some() {
        return;
    }
    let list = exposed(spec, &entity.nbt);
    let Some(e) = list.iter().find(|e| e.drain) else { return };
    let mut tank = read_tank(&entity.nbt, e.key);
    let (fluid, wanted) = match resource {
        Some(r) if r.has_components() => return,
        Some(r) => (r.key().and_then(mod_api::__private::fluid_id).unwrap_or_default().to_owned(), i32::try_from(r.amount).unwrap_or(i32::MAX)),
        None => (tank.fluid.clone(), max),
    };
    let took = tank.extract(&fluid, wanted, simulate);
    if took > 0 && !simulate {
        write_tank(&mut entity.nbt, e.key, &tank);
    }
    *drained = api_stack(&Tank::new(&fluid, took));
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
