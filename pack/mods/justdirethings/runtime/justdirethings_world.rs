// Just Dire Things' goo blocks and Generator T1 in the game (support module `crate::game::justdirethings_world` of the
// `justdirethings` native bundle). The rules are the bundle's crate `justdirethings_core`; this module is the glue: the
// spreading recipes read from the world's tables, the block entities and their saved compounds (Java's keys), the
// goo's neighbours, its `alive` state and revival, the generator's fuel slot, container, Forge Energy and redstone, and
// the data its screen reads. Server hooks run on the thread of the `Game` that simulates the world, so the settings and
// the recipe book are kept per thread, dropped when the server opens its world and when it closes. Translated from
// Just Dire Things 1.5.7 (MIT); see this bundle's README.md.

use super::Game;
use crate::game::api_values::{TryIntoApi, TryIntoEngine};
use horizon_client::{containers::ModContainer, items::ItemStack};
use horizon_core::geometry::Direction;
use horizon_core::nbt::{Compound, JavaString, Tag};
use horizon_core::random::RandomSource;
use horizon_world::level::flags;
use horizon_world::registry::BlockStateId;
use justdirethings_core::{
    SIDE_NAMES, SIDES,
    breaker::{self, Breaker, Front, Tracked},
    config::Config,
    generator::{self, Fuel, FuelSlot, Generator},
    goo::{Death, Goo, Tier},
    recipe::{self, Input, SpreadRecipe, StateSpec},
    redstone::{Mode, Redstone},
    view::GeneratorView,
};
use mod_api::{BlockUpdateFlags, ModBlockEntity, NativeFeInfo, SoundSource, WorldPhase};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

pub(super) const GENERATOR: &str = "justdirethings:generatort1";
pub(super) const BREAKER: &str = "justdirethings:blockbreakert1";

// ---- Settings and the recipe book ------------------------------------------------------------------------------

/// The spreading recipes of both types, and what each block state on a goo's side converts by (per tier: the
/// state's recipe, `outputCache`/`durationCache` shared by every goo of a tier on this thread).
struct Book {
    recipes: Vec<SpreadRecipe>,
    /// Each recipe's input state, resolved (none for a tag recipe, or a state the game doesn't have).
    inputs: Vec<Option<BlockStateId>>,
    found: RefCell<HashMap<(BlockStateId, i32), Option<usize>>>,
}

thread_local! {
    static SETTINGS: RefCell<Option<Config>> = const { RefCell::new(None) };
    static BOOK: RefCell<Option<Rc<Book>>> = const { RefCell::new(None) };
    /// The breakers' fields Java doesn't save (the operation clock, the block being mined), by dimension and position.
    static BREAKERS: RefCell<HashMap<(&'static str, [i32; 3]), (i32, Option<Tracked>, bool)>> = RefCell::new(HashMap::new());
}

fn read_settings() -> Config {
    match Game::native_config_dir() {
        Ok(dir) => Config::load(&dir),
        Err(e) => {
            log::warn!("[justdirethings] no config folder ({e}): the mod's defaults");
            Config::default()
        }
    }
}

/// The settings, read on first use if the world's opening didn't.
pub(super) fn settings() -> Config {
    SETTINGS.with(|s| s.borrow_mut().get_or_insert_with(read_settings).clone())
}

/// `native_server_started`: settings are read afresh, the recipe book dropped.
pub(super) fn opened() {
    let fresh = read_settings();
    SETTINGS.with(|s| *s.borrow_mut() = Some(fresh));
    BOOK.with(|b| *b.borrow_mut() = None);
    BREAKERS.with(|b| b.borrow_mut().clear());
}

/// `native_world_lifecycle` v2: settings and the recipe book are dropped as the world closes or fails to open.
pub(super) fn lifecycle(phase: WorldPhase) {
    if matches!(phase, WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed | WorldPhase::WorldOpenFailed) {
        SETTINGS.with(|s| *s.borrow_mut() = None);
        BOOK.with(|b| *b.borrow_mut() = None);
        BREAKERS.with(|b| b.borrow_mut().clear());
    }
}

/// The block state `spec` names: the block's default with the properties it gives (`BlockState.CODEC`); none for a
/// block the game doesn't have, or a property or value it doesn't.
fn resolve(game: &Game<'_>, spec: &StateSpec) -> Option<BlockStateId> {
    let registry = game.pack.assets.registry;
    let default = game.native_default_state(&spec.name)?;
    let mut state = registry.state(default)?.clone();
    for (key, value) in &spec.properties {
        if !state.properties.contains_key(key.as_str()) {
            return None;
        }
        state.properties.insert(key.clone(), value.clone());
    }
    registry.resolve(&state).ok()
}

/// The recipes of the world's tables (`justdirethings:goospread`, then `goospread_tag`, each in the table's order).
fn load_book(game: &Game<'_>) -> Book {
    let mut recipes = Vec::new();
    for kind in [recipe::SPREAD, recipe::SPREAD_TAG] {
        for r in game.native_recipes_of_type(kind).iter() {
            match SpreadRecipe::parse(kind, &r.id, &r.json) {
                Ok(parsed) => recipes.push(parsed),
                Err(e) => log::warn!("[justdirethings] {e}"),
            }
        }
    }
    let inputs = recipes
        .iter()
        .map(|r| match &r.input {
            Input::State(spec) => resolve(game, spec),
            Input::Tag(_) => None,
        })
        .collect();
    log::info!("[justdirethings] {} goo spreading recipe(s)", recipes.len());
    Book { recipes, inputs, found: RefCell::new(HashMap::new()) }
}

fn book(game: &Game<'_>) -> Rc<Book> {
    if let Some(b) = BOOK.with(|b| b.borrow().clone()) {
        return b;
    }
    let fresh = Rc::new(load_book(game));
    BOOK.with(|b| *b.borrow_mut() = Some(fresh.clone()));
    fresh
}

impl Book {
    /// `populateCaches` for the block state `state` beside a goo of tier `tier`: the recipe that converts it.
    fn find(&self, game: &Game<'_>, tier: i32, state: BlockStateId) -> Option<usize> {
        if let Some(found) = self.found.borrow().get(&(state, tier)) {
            return *found;
        }
        let name = game.pack.assets.registry.state(state).map(|s| s.name.to_string()).unwrap_or_default();
        let index = |r: &SpreadRecipe| self.recipes.iter().position(|o| std::ptr::eq(o, r));
        let found = recipe::find(
            &self.recipes,
            tier,
            |spec| {
                self.recipes
                    .iter()
                    .zip(&self.inputs)
                    .any(|(r, input)| matches!(&r.input, Input::State(s) if s == spec) && *input == Some(state))
            },
            |tag| game.native_block_name_in_tag(&name, tag).unwrap_or(false),
        )
        .and_then(index);
        self.found.borrow_mut().insert((state, tier), found);
        found
    }
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

fn flag(nbt: &Compound, name: &str) -> bool {
    int(nbt, name) != 0
}

fn put(nbt: &mut Compound, name: &str, v: i32) {
    nbt.insert(key(name), Tag::Int(v));
}

/// `sideCounters` and `sideDurations`: lists of `{side, counter}` and `{side, duration}`.
fn read_goo(nbt: &Compound) -> Goo {
    let mut goo = Goo::default();
    for (list, field, out) in [("sideCounters", "counter", &mut goo.counters), ("sideDurations", "duration", &mut goo.durations)] {
        if let Some(Tag::List(entries)) = nbt.get(&key(list)) {
            for entry in entries {
                if let Tag::Compound(c) = entry
                    && let Some(slot) = usize::try_from(int(c, "side")).ok().filter(|s| *s < SIDES)
                {
                    out[slot] = int(c, field);
                }
            }
        }
    }
    goo
}

fn write_goo(nbt: &mut Compound, goo: &Goo) {
    for (list, field, values) in [("sideCounters", "counter", &goo.counters), ("sideDurations", "duration", &goo.durations)] {
        let entries = (0..SIDES)
            .map(|side| {
                let mut c = Compound::new();
                put(&mut c, "side", side as i32);
                put(&mut c, field, values[side]);
                Tag::Compound(c)
            })
            .collect();
        nbt.insert(key(list), Tag::List(entries));
    }
}

fn read_generator(nbt: &Compound) -> Generator {
    Generator {
        energy: int(nbt, "energy"),
        max_burn: int(nbt, "maxBurn"),
        burn_remaining: int(nbt, "burnRemaining"),
        fe_remaining: int(nbt, "feRemaining"),
        fuel_multiplier: if nbt.contains_key(&key("fuelBurnMultiplier")) { int(nbt, "fuelBurnMultiplier") } else { 1 },
        redstone: Redstone {
            mode: Mode::from_index(int(nbt, "redstoneMode")),
            pulsed: flag(nbt, "pulsed"),
            receiving: flag(nbt, "receivingRedstone"),
        },
    }
}

fn write_generator(nbt: &mut Compound, g: &Generator) {
    put(nbt, "energy", g.energy);
    put(nbt, "maxBurn", g.max_burn);
    put(nbt, "burnRemaining", g.burn_remaining);
    put(nbt, "feRemaining", g.fe_remaining);
    put(nbt, "fuelBurnMultiplier", g.fuel_multiplier);
    put(nbt, "redstoneMode", g.redstone.mode.index());
    nbt.insert(key("pulsed"), Tag::Byte(i8::from(g.redstone.pulsed)));
    nbt.insert(key("receivingRedstone"), Tag::Byte(i8::from(g.redstone.receiving)));
}

/// The breaker's saved fields: `redstoneMode`, `pulsed`, `receivingRedstone`, `tickspeed`, `sneaking`.
fn read_breaker(nbt: &Compound) -> Breaker {
    Breaker {
        redstone: Redstone {
            mode: Mode::from_index(int(nbt, "redstoneMode")),
            pulsed: flag(nbt, "pulsed"),
            receiving: flag(nbt, "receivingRedstone"),
        },
        tick_speed: if nbt.contains_key(&key("tickspeed")) { int(nbt, "tickspeed").max(1) } else { 20 },
        sneaking: flag(nbt, "sneaking"),
        ..Breaker::default()
    }
}

fn write_breaker(nbt: &mut Compound, b: &Breaker) {
    put(nbt, "redstoneMode", b.redstone.mode.index());
    nbt.insert(key("pulsed"), Tag::Byte(i8::from(b.redstone.pulsed)));
    nbt.insert(key("receivingRedstone"), Tag::Byte(i8::from(b.redstone.receiving)));
    put(nbt, "tickspeed", b.tick_speed);
    nbt.insert(key("sneaking"), Tag::Byte(i8::from(b.sneaking)));
}

// ---- Block entities --------------------------------------------------------------------------------------------

/// `native_block_entity_types` (compat hook): one type per goo tier and the generator's, named after their blocks.
pub(super) fn block_entity_types(types: &mut Vec<(String, Vec<String>)>) {
    for block in Tier::ALL.map(Tier::block).into_iter().chain([GENERATOR, BREAKER]) {
        types.push((block.to_owned(), vec![block.to_owned()]));
    }
}

/// `native_block_entity`: a goo with no side running, an empty generator.
pub(super) fn new_entity(block: &str, entity: &mut Option<Compound>) {
    if entity.is_some() {
        return;
    }
    let mut nbt = Compound::new();
    if Tier::of_block(block).is_some() {
        write_goo(&mut nbt, &Goo::default());
    } else if block == GENERATOR {
        write_generator(&mut nbt, &Generator::default());
        put(&mut nbt, "tickspeed", 20);
        put(&mut nbt, "direction", 0);
    } else if block == BREAKER {
        write_breaker(&mut nbt, &Breaker::default());
        put(&mut nbt, "direction", 0);
    } else {
        return;
    }
    *entity = Some(nbt);
}

/// `native_block_entity_ticks`: goo and the generator tick.
pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if Tier::of_block(block).is_some() || block == GENERATOR || block == BREAKER {
        *ticks = true;
    }
}

/// `native_block_entity_tick`.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    if let Some(tier) = Tier::of_block(entity.id()) {
        tick_goo(game, pos, entity, tier);
    } else if entity.id() == GENERATOR {
        tick_generator(game, pos, entity);
    } else if entity.id() == BREAKER {
        tick_breaker(game, pos, entity);
    }
}

fn beside(pos: [i32; 3], side: usize) -> [i32; 3] {
    let [dx, dy, dz] = Direction::ALL[side].normal();
    [pos[0] + dx, pos[1] + dy, pos[2] + dz]
}

fn center(pos: [i32; 3]) -> [f64; 3] {
    [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.5, f64::from(pos[2]) + 0.5]
}

/// One server tick of a goo (`GooBlockBE_Base.tickServer`). The sides it finishes are written after the death rolls
/// of the tick: a conversion changes only its own side, which no other side reads.
fn tick_goo(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, tier: Tier) {
    let Some(level) = game.fork_level_here() else { return };
    let alive = game.native_block_property(pos, "alive").as_deref() == Some("true");
    let book = book(game);
    let mut found = [None; SIDES];
    for (side, slot) in found.iter_mut().enumerate() {
        if let Some(state) = game.level_block_state(level, beside(pos, side)) {
            *slot = book.find(game, tier.number(), state);
        }
    }
    let durations = found.map(|f| f.map(|i| book.recipes[i].duration));
    let cfg = settings();
    let before = read_goo(&entity.nbt);
    let mut goo = before;
    let step = goo.tick(
        tier,
        alive,
        &durations,
        Death { can_die: cfg.goo_can_die, chance: cfg.goo_death_chance },
        |_| {},
        || {
            // `level.random.nextFloat()`: a copy of the level's random, stored back so the draw moves the level's own.
            let Some(mut random) = game.level_random(level) else { return 1.0 };
            let roll = random.next_float();
            game.level_set_random(level, random);
            roll
        },
    );
    for &side in &step.finished {
        let Some(index) = found[side] else { continue };
        let output = &book.recipes[index].output;
        // `setBlockToTarget`: an output with a facing faces away from the goo.
        let state = resolve(game, &output.with("facing", SIDE_NAMES[side])).or_else(|| resolve(game, output));
        if let Some(state) = state {
            let _ = game.level_set_block(level, beside(pos, side), state, BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS);
        }
        game.level_play_sound(level, "minecraft:block.sculk.break", center(pos), SoundSource::Blocks, 1.0, 1.0);
    }
    if step.died {
        game.native_set_block_property_with(pos, "alive", "false", flags::UPDATE_ALL);
        game.level_play_sound(level, "minecraft:entity.vex.death", center(pos), SoundSource::Blocks, 1.0, 0.25);
    }
    if goo != before {
        write_goo(&mut entity.nbt, &goo);
    }
}

// ---- The goo's revival -----------------------------------------------------------------------------------------

thread_local! {
    /// `use_on` took the use in progress: `take_use` answers it.
    static USE_TAKEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether `use_on` took the use the hook is answering (once).
pub(super) fn take_use() -> bool {
    USE_TAKEN.with(|u| u.replace(false))
}

/// `GooBlock_Base.useItemOn`: a dead goo fed an item of its tier's revive tag comes alive, with the sculk's spread
/// sound, and the item is used up unless the player is in creative. On the server only: a client leaves the world to
/// the server.
pub(super) fn use_on(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3]) {
    USE_TAKEN.with(|u| u.set(false));
    if game.remote.is_some() {
        return;
    }
    let Some(tier) = Tier::of_block(&game.native_block(pos)) else { return };
    if game.native_block_property(pos, "alive").as_deref() != Some("false") {
        return;
    }
    if !game.native_server_item_in_tag(stack, tier.revive_tag()).unwrap_or(false) {
        return;
    }
    game.native_set_block_property_with(pos, "alive", "true", flags::UPDATE_ALL);
    if let Some(level) = game.fork_level_here() {
        game.level_play_sound(level, "minecraft:block.sculk.spread", center(pos), SoundSource::Blocks, 1.0, 0.5);
    }
    if !game.native_instabuild() {
        let hand = game.native_used_stack();
        if let Some(held) = hand.as_mut() {
            held.count = held.count.saturating_sub(1);
        }
        if hand.as_ref().is_some_and(|s| s.count == 0) {
            *hand = None;
        }
    }
    USE_TAKEN.with(|u| u.set(true));
}

// ---- The Generator T1 ------------------------------------------------------------------------------------------

/// The slots of the hopper menu the generator shows its one fuel slot on (the other four unused).
const HOPPER_SLOTS: usize = 5;
const FUEL: usize = 0;

/// `native_block_entity_container`: the generator's fuel slot or the breaker's tool slot, which hoppers fill from every
/// side.
pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if !(block == GENERATOR || block == BREAKER) || container.is_some() {
        return;
    }
    let fuel = || vec![FUEL];
    *container = Some(ModContainer {
        slots: HOPPER_SLOTS,
        faces: [fuel(), fuel(), fuel(), fuel(), fuel(), fuel()],
        inputs: vec![FUEL],
        outputs: Vec::new(),
        menu: Some("minecraft:hopper".into()),
    });
}

/// `native_block_entity_slot_accepts`: fuel in the generator's fuel slot (`FuelSlot`), anything in the breaker's tool
/// slot, nothing in the menu's others.
pub(super) fn slot_accepts(block: &str, slot: usize, fuel: bool, accepts: &mut bool) {
    if block == GENERATOR {
        *accepts = *accepts && slot == FUEL && fuel;
    } else if block == BREAKER {
        // The tool slot takes any item, as Java's machine handler does.
        *accepts = *accepts && slot == TOOL;
    }
}

/// The fuel slot over the engine's stack: its burn time, its multiplier and what using one leaves.
struct Slot {
    stack: Option<ItemStack>,
    fuel: Option<Fuel>,
    remainder: Option<ItemStack>,
}

impl FuelSlot for Slot {
    fn peek(&self) -> Option<Fuel> {
        self.stack.as_ref().filter(|s| s.count > 0).and(self.fuel)
    }

    fn consume(&mut self) {
        // `hasCraftingRemainingItem`: the remainder replaces the stack (a lava bucket's bucket burns no more).
        if let Some(rest) = self.remainder.take() {
            self.stack = Some(rest);
            self.fuel = None;
            return;
        }
        if let Some(s) = self.stack.as_mut() {
            s.count = s.count.saturating_sub(1);
            if s.count == 0 {
                self.stack = None;
            }
        }
    }
}

/// One server tick of the generator (`GeneratorT1BE.tickServer`): the redstone (each tick, where Java reads it again
/// after a neighbour changed), burning, then the push to the six neighbours.
fn tick_generator(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let cfg = settings().generator;
    let dimension = game.native_dimension();
    let signal = game.native_neighbor_signal(dimension, pos) > 0;
    let held: Option<ItemStack> = entity.items.get(FUEL).and_then(|s| s.as_ref()).and_then(|s| s.try_into_engine(game.items()).ok());
    let fuel = held.as_ref().filter(|s| s.count > 0).map(|s| Fuel {
        burn_time: game.native_burn_time(s),
        multiplier: generator::fuel_multiplier(&s.item),
    });
    let remainder = held.as_ref().and_then(|s| game.crafting_remainder(s));
    let mut slot = Slot { stack: held.clone(), fuel, remainder };
    let before = read_generator(&entity.nbt);
    let mut g = before;
    g.redstone.evaluate(signal);
    g.generate(&cfg, &mut slot);
    g.push(&cfg, |side, amount, simulate| {
        let face = Direction::ALL[side];
        game.native_fe_offer(dimension, beside(pos, side), Some(face.opposite()), amount, simulate, false)
    });
    if slot.stack != held {
        if entity.items.len() < HOPPER_SLOTS {
            entity.items.resize(HOPPER_SLOTS, None);
        }
        entity.items[FUEL] = slot.stack.as_ref().and_then(|s| s.try_into_api().ok());
    }
    if g != before {
        write_generator(&mut entity.nbt, &g);
    }
    let view = GeneratorView {
        energy: g.energy,
        capacity: cfg.max_fe,
        burn_remaining: g.burn_remaining,
        max_burn: g.max_burn,
        fe_per_tick: if g.burn_remaining > 0 { g.fe_per_tick(&cfg) } else { 0 },
        redstone: g.redstone.mode.index(),
    }
    .pack();
    if entity.data != view {
        entity.data = view;
    }
}

/// `native_menu_button`: the redstone button (ids 0 to 3, the mode it sets; `setRedstoneSettings`).
pub(super) fn button(_game: &mut Game<'_>, entity: &mut ModBlockEntity, id: i32) {
    if !(0..4).contains(&id) {
        return;
    }
    if entity.id() == GENERATOR {
        let mut g = read_generator(&entity.nbt);
        g.redstone.mode = Mode::from_index(id);
        write_generator(&mut entity.nbt, &g);
    } else if entity.id() == BREAKER {
        let mut b = read_breaker(&entity.nbt);
        b.redstone.mode = Mode::from_index(id);
        write_breaker(&mut entity.nbt, &b);
    }
}

/// `native_fe_storage`: every side gives and none takes (`EnergyStorageNoReceive`).
pub(super) fn fe_storage(entity: &ModBlockEntity, info: &mut Option<NativeFeInfo>) {
    if entity.id() != GENERATOR || info.is_some() {
        return;
    }
    *info = Some(NativeFeInfo {
        stored: i64::from(int(&entity.nbt, "energy")),
        capacity: i64::from(settings().generator.max_fe),
        can_receive: false,
        can_extract: true,
    });
}

/// `native_fe_extract`: what the buffer has, up to `max`.
pub(super) fn fe_extract(entity: &mut ModBlockEntity, max: i64, simulate: bool, given: &mut Option<i64>) {
    if entity.id() != GENERATOR || given.is_some() {
        return;
    }
    let cfg = settings().generator;
    let mut g = read_generator(&entity.nbt);
    let out = g.extract(&cfg, max, simulate);
    if out > 0 && !simulate {
        put(&mut entity.nbt, "energy", g.energy);
    }
    *given = Some(out);
}


// ---- The Block Breaker T1 -------------------------------------------------------------------------------------

const TOOL: usize = 0;

/// The block in front of a breaker, in the level in place.
struct FrontBlock<'a, 'g> {
    game: &'a mut Game<'g>,
    level: mod_api::LevelRef,
    pos: [i32; 3],
    tool: Option<&'a ItemStack>,
    /// A stable negative breaker id for the crack (`ServerLevel.destroyBlockProgress`).
    breaker: i32,
    /// The hardness of the block it broke, if it broke one.
    broke: Option<f32>,
}

impl Front for FrontBlock<'_, '_> {
    fn state(&mut self) -> Option<u32> {
        let state = self.game.level_block_state(self.level, self.pos)?;
        (!self.game.res.facts().get(state).air).then_some(state.0 as u32)
    }

    fn valid(&mut self) -> bool {
        let Some(state) = self.game.level_block_state(self.level, self.pos) else { return false };
        let facts = self.game.res.facts().get(state);
        !facts.air && !facts.classes.iter().any(|c| c.ends_with("LiquidBlock")) && facts.hardness >= 0.0
    }

    fn progress(&mut self) -> f32 {
        let Some(state) = self.game.level_block_state(self.level, self.pos) else { return 0.0 };
        let facts = self.game.res.facts().get(state);
        let (name, hardness) = (facts.name.to_string(), facts.hardness);
        let items = self.game.items();
        let speed = items.mining_speed(self.tool, &name);
        let correct = items.correct_for_drops(self.tool, &name);
        let efficiency = self.tool.map_or(0, |t| t.enchantment_level("minecraft:efficiency"));
        breaker::destroy_progress(speed, efficiency, hardness, correct)
    }

    fn crack(&mut self, stage: i32) {
        self.game.level_set_break_progress(self.level, self.breaker, self.pos, stage);
    }

    fn break_block(&mut self) {
        let hardness = self.game.native_destroy_speed(self.pos).unwrap_or(0.0);
        if self.game.native_machine_destroy_block(self.pos, self.tool, false).is_some() {
            self.broke = Some(hardness);
        }
    }
}

/// One server tick of a breaker (`BlockBreakerT1BE.tickServer`), then its tool's wear (`hurtAndBreak(1)` for a
/// block of any hardness but 0), its `active` state and what its screen shows.
fn tick_breaker(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let Some(level) = game.fork_level_here() else { return };
    let dimension = game.native_dimension();
    let facing = game
        .native_block_property(pos, "facing")
        .and_then(|f| SIDE_NAMES.iter().position(|n| *n == f))
        .unwrap_or(2);
    let signal = game.native_neighbor_signal(dimension, pos) > 0;
    let tool: Option<ItemStack> = entity
        .items
        .get(TOOL)
        .and_then(|s| s.as_ref())
        .and_then(|s| s.try_into_engine(game.items()).ok())
        .filter(|s| s.count > 0);
    let saved = read_breaker(&entity.nbt);
    let mut b = saved;
    if let Some((clock, tracked, current)) = BREAKERS.with(|m| m.borrow().get(&(dimension, pos)).copied()) {
        (b.operation_ticks, b.tracked, b.current) = (clock, tracked, current);
    }
    let breaker_id = -1000 - (pos[0].wrapping_add(pos[1]).wrapping_add(pos[2])).rem_euclid(1 << 20);
    let mut front = FrontBlock { game: &mut *game, level, pos: beside(pos, facing), tool: tool.as_ref(), breaker: breaker_id, broke: None };
    b.tick(tool.is_some(), signal, &mut front);
    let broke = front.broke;
    BREAKERS.with(|m| m.borrow_mut().insert((dimension, pos), (b.operation_ticks, b.tracked, b.current)));
    if let (Some(hardness), Some(t)) = (broke, tool.as_ref())
        && hardness != 0.0
    {
        let (left, _) = game.damage_living_stack(t, 1, 0);
        if entity.items.len() < HOPPER_SLOTS {
            entity.items.resize(HOPPER_SLOTS, None);
        }
        entity.items[TOOL] = left.as_ref().and_then(|s| s.try_into_api().ok());
    }
    if (b.redstone, b.tick_speed, b.sneaking) != (saved.redstone, saved.tick_speed, saved.sneaking) {
        write_breaker(&mut entity.nbt, &b);
    }
    let active = if b.active() { "true" } else { "false" };
    if game.native_block_property(pos, "active").is_some_and(|a| a != active) {
        game.native_set_block_property_with(pos, "active", active, flags::UPDATE_ALL);
    }
    let has_tool = entity.items.get(TOOL).is_some_and(|s| s.is_some());
    let data = vec![b.redstone.mode.index(), i32::from(has_tool)];
    if entity.data != data {
        entity.data = data;
    }
}
