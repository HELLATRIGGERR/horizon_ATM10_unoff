// Just Dire Things' Dropper T2, Sensor T2, Fluid Placer T2 and Fluid Collector T2 in the game (support module
// `crate::game::justdirethings_t2x` of the `justdirethings` native bundle), on what every T2 machine shares
// (`justdirethings_clicker`: the Forge Energy buffer, the area and the filter slots in a 9x2 chest menu). The rules are the
// bundle's crate `justdirethings_core` (`t2x`, `more`); this module is the glue: the saved compounds (Java's keys), the
// slots (the dropper's nine machine slots 0 to 8 then its filter 9 to 17; the sensor's filter alone 1 to 9; the fluid
// machines' slot 0 and filter 1 to 9), the drop of the dropper at its area's offset, the sensor's scan of its area, and the fluid
// machines' places of their area found together and used one a run, each as the T1 does its one in front. Translated from
// Just Dire Things 1.5.7 (MIT); see this bundle's README.md.

use super::{
    Game,
    justdirethings_clicker as clicker,
    justdirethings_machines::{self as machines, Around},
    justdirethings_more::{self as more, Ahead, Pail},
};
use crate::game::api_values::{TryIntoApi, TryIntoEngine};
use horizon_client::{containers::ModContainer, items::ItemStack};
use horizon_core::geometry::Direction;
use horizon_core::nbt::Compound;
use justdirethings_core::{
    machines::{Clock, Container, Source, Tank},
    more::{Dropper, FluidSpot, Sensor, Target},
    redstone::Mode,
    t2::{Queue, Shared},
    t2x::{DROPPER_COST, DROPPER_SLOTS, DropperT2, FLUID_CAPACITY, Pools, SENSOR_COST, area_volume, collector_tick, pay_all, placer_tick, populate_filtered, populate_plain},
};
use mod_api::ModBlockEntity;
use std::{cell::RefCell, collections::HashMap};

pub(super) const DROPPER_T2: &str = "justdirethings:droppert2";
pub(super) const SENSOR_T2: &str = "justdirethings:sensort2";
pub(super) const FLUID_PLACER_T2: &str = "justdirethings:fluidplacert2";
pub(super) const FLUID_COLLECTOR_T2: &str = "justdirethings:fluidcollectort2";
/// The 9x2 chest menu every T2 machine has.
pub(super) const SLOTS: usize = 18;
const FILTER_FIRST: usize = 1;
const FILTER_LAST: usize = 9;

/// Whether `block` is one of this module's machines.
pub(super) fn owns(block: &str) -> bool {
    block == DROPPER_T2 || block == SENSOR_T2 || is_fluid(block)
}

/// The T2 fluid machines (the 32,000 mB tank).
pub(super) fn is_fluid(block: &str) -> bool {
    block == FLUID_PLACER_T2 || block == FLUID_COLLECTOR_T2
}

thread_local! {
    /// What each Dropper T2 has still to drop (Java's transient `slotsToDropList`), by dimension and position.
    static LISTS: RefCell<HashMap<(&'static str, [i32; 3]), Vec<usize>>> = RefCell::new(HashMap::new());
    /// What each fluid machine found and has not used yet (`positionsToPlace`), by dimension and position.
    static QUEUES: RefCell<HashMap<(&'static str, [i32; 3]), Vec<[i32; 3]>>> = RefCell::new(HashMap::new());
}

pub(super) fn reset() {
    LISTS.with(|l| l.borrow_mut().clear());
    QUEUES.with(|q| q.borrow_mut().clear());
}

/// `native_block_entity`: Java's defaults; the area and filter are written on the first tick.
pub(super) fn new_entity(block: &str, nbt: &mut Compound) {
    machines::new_entity(nbt);
    if block == DROPPER_T2 {
        machines::put(nbt, "dropCount", 1);
        machines::put(nbt, "pickupDelay", 0);
    } else if block == SENSOR_T2 {
        machines::put(nbt, "senseTarget", 0);
        machines::put_flag(nbt, "strongSignal", false);
        machines::put(nbt, "senseAmount", 0);
        machines::put(nbt, "equality", 0);
    }
}

/// `native_block_entity_container`: the dropper's nine slots (which hoppers fill from every side) and nine filter slots;
/// the sensor's nine filter slots (slot 0 is unused); the fluid machines' slot and filter.
pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if block == DROPPER_T2 {
        let all = || (0..DROPPER_SLOTS).collect::<Vec<_>>();
        *container = Some(ModContainer {
            slots: SLOTS,
            faces: [all(), all(), all(), all(), all(), all()],
            inputs: all(),
            outputs: Vec::new(),
            menu: Some("minecraft:generic_9x2".into()),
        });
    } else if block == SENSOR_T2 {
        filter_only(container);
    } else {
        clicker::container(clicker::CLICKER_T2, container);
    }
}

/// A menu with the nine filter slots only (no side reaches them).
pub(super) fn filter_only(container: &mut Option<ModContainer>) {
    *container = Some(ModContainer { slots: SLOTS, faces: Default::default(), inputs: Vec::new(), outputs: Vec::new(), menu: Some("minecraft:generic_9x2".into()) });
}

pub(super) fn slot_accepts(block: &str, slot: usize, accepts: &mut bool) {
    if block == DROPPER_T2 {
        *accepts = *accepts && slot < SLOTS;
    } else if block == SENSOR_T2 {
        *accepts = *accepts && (FILTER_FIRST..=FILTER_LAST).contains(&slot);
    } else {
        clicker::slot_accepts(clicker::CLICKER_T2, slot, accepts);
    }
}

fn stack_at(game: &Game<'_>, entity: &ModBlockEntity, i: usize) -> Option<ItemStack> {
    entity.items.get(i).and_then(|s| s.as_ref()).and_then(|s| s.try_into_engine(game.items()).ok()).filter(|s: &ItemStack| s.count > 0)
}

fn set_item(entity: &mut ModBlockEntity, i: usize, stack: Option<ItemStack>) {
    if entity.items.len() < SLOTS {
        entity.items.resize(SLOTS, None);
    }
    entity.items[i] = stack.filter(|s| s.count > 0).as_ref().and_then(|s| s.try_into_api().ok());
}

/// The T2 parts of a screen's data: the energy, the radius in half blocks, the offsets, allowlist, compare and render area.
pub(super) fn parts(data: &mut Vec<i32>, s: &Shared) {
    data.push(s.powered.energy);
    data.extend(s.area.radius.map(|r| (r * 2.0).round() as i32));
    data.extend(s.area.offset);
    data.extend([i32::from(s.filter.allowlist), i32::from(s.filter.compare_nbt), i32::from(s.area.render)]);
}

/// What every machine's tick starts from.
struct Start {
    level: mod_api::LevelRef,
    dimension: &'static str,
    signal: bool,
    fresh: bool,
    shared: Shared,
    energy: i32,
    before: Clock,
}

fn start(game: &mut Game<'_>, pos: [i32; 3], entity: &ModBlockEntity) -> Option<Start> {
    let level = game.fork_level_here()?;
    let dimension = game.native_dimension();
    let signal = game.native_neighbor_signal(dimension, pos) > 0;
    let facing = Direction::ALL[machines::facing(game, pos)].normal();
    let fresh = !entity.nbt.contains_key(&machines::key("xRadiusDouble"));
    let shared = clicker::read_shared(&entity.nbt, facing);
    Some(Start { level, dimension, signal, fresh, energy: shared.powered.energy, shared, before: machines::clock_at(dimension, pos, &entity.nbt) })
}

/// The shared settings are written once, then the buffer when it changed.
fn finish(entity: &mut ModBlockEntity, s: &Start, shared: &Shared) {
    if s.fresh {
        clicker::write_shared(&mut entity.nbt, shared);
    } else if shared.powered.energy != s.energy {
        machines::put(&mut entity.nbt, "energy", shared.powered.energy);
    }
}

fn show(entity: &mut ModBlockEntity, data: Vec<i32>) {
    if entity.data != data {
        entity.data = data;
    }
}

/// One server tick of a machine of this module.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let id = entity.id().to_owned();
    let Some(start) = start(game, pos, entity) else { return };
    if id == DROPPER_T2 {
        tick_dropper(game, pos, entity, start);
    } else if id == SENSOR_T2 {
        tick_sensor(game, pos, entity, start);
    } else {
        tick_fluid(game, pos, entity, start, id == FLUID_PLACER_T2);
    }
}

// ---- The Dropper T2 --------------------------------------------------------------------------------------------

/// `DropperT2BE` (`DropperT1BE.doDrop`): the slots to drop are listed on a run (the first with an item, or one for each
/// filter item, all of which must be there), one dropped each operation, at the area's offset, 25 FE each.
fn tick_dropper(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, s: Start) {
    let mut shared = s.shared;
    let list = LISTS.with(|l| l.borrow_mut().remove(&(s.dimension, pos))).unwrap_or_default();
    let mut d = DropperT2 { clock: s.before, list };
    let direction = machines::int(&entity.nbt, "direction").clamp(0, 5);
    let count = if entity.nbt.contains_key(&machines::key("dropCount")) { machines::int(&entity.nbt, "dropCount").max(1) } else { 1 };
    let delay = machines::int(&entity.nbt, "pickupDelay").max(0);
    let stacks: Vec<Option<ItemStack>> = (0..DROPPER_SLOTS).map(|i| stack_at(game, entity, i)).collect();
    let filters: Vec<ItemStack> = (DROPPER_SLOTS..SLOTS).filter_map(|i| stack_at(game, entity, i)).collect();
    let compare = shared.filter.compare_nbt;
    let same = |a: &ItemStack, b: &ItemStack| if compare { a.same_item_same_components(b) } else { a.item == b.item };
    let populate = || {
        if filters.is_empty() {
            populate_plain(&stacks.iter().map(Option::is_some).collect::<Vec<_>>())
        } else {
            populate_filtered(&filters, &stacks, same)
        }
    };
    let dropped = d.tick(s.signal, shared.powered.has(DROPPER_COST), populate, |i| stacks.get(i).is_some_and(Option::is_some));
    if let Some(slot) = dropped
        && let Some(stack) = stacks[slot].clone()
    {
        let n = stack.count.min(count.try_into().unwrap_or(stack.count));
        let mut give = stack.clone();
        give.count = n;
        let mut left = stack.clone();
        left.count = stack.count - n;
        let [ox, oy, oz] = shared.area.offset;
        let at = machines::center([pos[0] + ox, pos[1] + oy, pos[2] + oz]);
        let [dx, dy, dz] = Direction::ALL[direction as usize].normal();
        let motion = [f64::from(dx) * 0.3, f64::from(dy) * 0.3, f64::from(dz) * 0.3];
        let _ = game.level_spawn_item_with_motion(s.level, at[0], at[1], at[2], give, motion, delay.unsigned_abs());
        set_item(entity, slot, Some(left));
        // `spawnItem`: 25 FE after each.
        shared.powered.take(DROPPER_COST);
    }
    LISTS.with(|l| l.borrow_mut().insert((s.dimension, pos), d.list));
    machines::keep_clock(s.dimension, pos, &mut entity.nbt, &s.before, &d.clock);
    finish(entity, &s, &shared);
    let has_item = (0..DROPPER_SLOTS).any(|i| entity.items.get(i).is_some_and(|x| x.is_some()));
    let mut data = vec![d.clock.redstone.mode.index(), i32::from(has_item), direction, d.clock.tick_speed, count, delay];
    parts(&mut data, &shared);
    show(entity, data);
}

// ---- The Sensor T2 ---------------------------------------------------------------------------------------------

/// `SensorT2BE`: on its clock it pays 2 FE for each block of its area (all it has, when that is short, and then it senses
/// nothing), counts the matching blocks or the entities of the area, and sets its signal.
fn tick_sensor(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, s: Start) {
    let mut shared = s.shared;
    let side = machines::facing(game, pos);
    let mut sensor = Sensor {
        clock: s.before,
        target: Target::from_index(machines::int(&entity.nbt, "senseTarget")),
        strong: machines::int(&entity.nbt, "strongSignal") != 0,
        amount: machines::int(&entity.nbt, "senseAmount"),
        equality: machines::int(&entity.nbt, "equality"),
    };
    let filters = clicker::filter_items(game, entity);
    let filter = shared.filter;
    let sensed = if sensor.tick() {
        let (min, max) = shared.area.bounds(pos);
        if pay_all(&mut shared.powered, area_volume(min, max) * SENSOR_COST) {
            let accepts = |item: &str| filter.accepts(filters.iter().any(|f| f == item));
            let view: &Game<'_> = game;
            let found = if sensor.target.is_block() {
                shared.area.blocks(pos).into_iter().map(|at| more::sense_block(view, s.level, at, sensor.target, &accepts)).sum()
            } else {
                more::sense_entities(view, s.level, min, max, sensor.target, &accepts)
            };
            Some(sensor.check_count(found))
        } else {
            None
        }
    } else {
        None
    };
    more::sensor_signal(game, s.level, s.dimension, pos, side, sensed, sensor.strong);
    machines::keep_clock(s.dimension, pos, &mut entity.nbt, &s.before, &sensor.clock);
    finish(entity, &s, &shared);
    let mut data = vec![sensor.target.index(), i32::from(sensor.strong), sensor.equality, sensor.amount, sensor.clock.tick_speed];
    parts(&mut data, &shared);
    show(entity, data);
}

// ---- The fluid machines ----------------------------------------------------------------------------------------

/// The Fluid Placer T2's area: the places whose block can be replaced (and isn't a source) and whose neighbour in the
/// direction setting the filter lets through (`isBlockPosValid`).
struct PlaceWorld<'a, 'g> {
    game: &'a mut Game<'g>,
    level: mod_api::LevelRef,
    dimension: &'static str,
    pos: [i32; 3],
    against: [i32; 3],
    shared: &'a Shared,
    filters: &'a [String],
}

impl Pools<[i32; 3]> for PlaceWorld<'_, '_> {
    fn find(&mut self) -> Vec<[i32; 3]> {
        let mut found = Vec::new();
        for at in self.shared.nearest_first(self.pos) {
            if !(Ahead { game: &mut *self.game, level: self.level, pos: at, dimension: self.dimension }).valid() {
                continue;
            }
            let next = [at[0] + self.against[0], at[1] + self.against[1], at[2] + self.against[2]];
            let game: &Game<'_> = &*self.game;
            let name = game.level_block_state(self.level, next).map(|s| game.res.facts().get(s).name.to_string()).unwrap_or_default();
            if clicker::block_passes(game, &self.shared.filter, self.filters, &name) {
                found.push(at);
            }
        }
        found
    }

    fn source(&mut self, _: &[i32; 3]) -> Option<String> {
        None
    }

    fn take(&mut self, _: &[i32; 3]) -> bool {
        false
    }

    fn place(&mut self, at: &[i32; 3], fluid: &str) -> bool {
        (Ahead { game: &mut *self.game, level: self.level, pos: *at, dimension: self.dimension }).place(fluid)
    }
}

/// The Fluid Collector T2's area: the source blocks of a fluid the tank takes and the filter (the fluid's bucket) lets through.
struct CollectWorld<'a, 'g> {
    around: Around<'a, 'g>,
    pos: [i32; 3],
    shared: &'a Shared,
    filters: &'a [String],
    tank_fluid: Option<String>,
}

impl Pools<[i32; 3]> for CollectWorld<'_, '_> {
    fn find(&mut self) -> Vec<[i32; 3]> {
        let mut found = Vec::new();
        for at in self.shared.nearest_first(self.pos) {
            self.around.pos = at;
            let Some(fluid) = Source::source(&mut self.around) else { continue };
            if self.tank_fluid.as_deref().is_some_and(|t| t != fluid) {
                continue;
            }
            let bucket = format!("{}_bucket", fluid.strip_suffix("_source").unwrap_or(&fluid));
            if self.shared.filter.accepts(self.filters.iter().any(|f| *f == bucket)) {
                found.push(at);
            }
        }
        found
    }

    fn source(&mut self, at: &[i32; 3]) -> Option<String> {
        self.around.pos = *at;
        Source::source(&mut self.around)
    }

    fn take(&mut self, at: &[i32; 3]) -> bool {
        self.around.pos = *at;
        Source::take(&mut self.around)
    }

    fn place(&mut self, _: &[i32; 3], _: &str) -> bool {
        false
    }
}

impl Container for CollectWorld<'_, '_> {
    fn would_take(&mut self, fluid: &str, amount: i32) -> i32 {
        self.around.would_take(fluid, amount)
    }

    fn fill(&mut self, fluid: &str, amount: i32) {
        self.around.fill(fluid, amount);
    }
}

/// One server tick of a Fluid Placer T2 or Collector T2.
fn tick_fluid(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, s: Start, placer: bool) {
    let mut shared = s.shared;
    let spots = QUEUES.with(|q| q.borrow_mut().remove(&(s.dimension, pos))).unwrap_or_default();
    let mut q = Queue { clock: s.before, spots };
    let tank = machines::read_tank(&entity.nbt);
    let mut now: Tank = tank.clone();
    let direction = machines::int(&entity.nbt, "direction").clamp(0, 5);
    let filters = clicker::filter_items(game, entity);
    let stack = machines::slot_stack(game, entity);
    let mut slot = None;
    if placer {
        let mut pail = Pail::new(game, stack.as_ref());
        let against = Direction::ALL[direction as usize].normal();
        let mut world = PlaceWorld { game: &mut *game, level: s.level, dimension: s.dimension, pos, against, shared: &shared, filters: &filters };
        let mut powered = shared.powered;
        placer_tick(&mut q, &mut now, &mut powered, s.signal, &mut pail, &mut world);
        shared.powered = powered;
        slot = pail.after.take();
    } else {
        let mut powered = shared.powered;
        let around = Around { game: &mut *game, level: s.level, pos, stack, changed: false };
        let mut world = CollectWorld { around, pos, shared: &shared, filters: &filters, tank_fluid: tank.fluid.clone().filter(|_| tank.amount > 0) };
        collector_tick(&mut q, &mut now, &mut powered, s.signal, &mut world);
        if world.around.changed {
            slot = world.around.stack.take();
        }
        shared.powered = powered;
    }
    if let Some(after) = slot {
        machines::set_slot(entity, Some(after));
    }
    if now != tank {
        machines::write_tank(&mut entity.nbt, &now);
    }
    QUEUES.with(|m| m.borrow_mut().insert((s.dimension, pos), q.spots));
    machines::keep_clock(s.dimension, pos, &mut entity.nbt, &s.before, &q.clock);
    finish(entity, &s, &shared);
    let has_item = entity.items.first().is_some_and(|x| x.is_some());
    let mut data = vec![q.clock.redstone.mode.index(), i32::from(has_item), q.clock.tick_speed, machines::fluid_code(&now), now.amount, FLUID_CAPACITY, direction];
    parts(&mut data, &shared);
    show(entity, data);
}

// ---- The screens' buttons --------------------------------------------------------------------------------------

/// `native_menu_button`: 0 to 3 the redstone mode, 22 and 23 the tick speed, 34 to 39 the direction (dropper and fluid
/// placer), the dropper's 28 and 29 drop count and 30 and 31 pickup delay, the sensor's 32 target, 33 strong or weak, 60 to 62
/// the count test, 63 and 64 the amount, and the shared area and filter buttons (`t2::Shared::button`).
pub(super) fn button(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, id: i32) {
    let dimension = game.native_dimension();
    let before = machines::clock_at(dimension, pos, &entity.nbt);
    let mut clock = before;
    let block = entity.id().to_owned();
    match id {
        0..=3 if block != SENSOR_T2 => clock.redstone.mode = Mode::from_index(id),
        22 => clock.set_tick_speed(clock.tick_speed + 1, machines::MIN_TICK_SPEED),
        23 => clock.set_tick_speed(clock.tick_speed - 1, machines::MIN_TICK_SPEED),
        34..=39 if block == DROPPER_T2 || block == FLUID_PLACER_T2 => machines::put(&mut entity.nbt, "direction", id - 34),
        28..=31 if block == DROPPER_T2 => {
            let mut d = Dropper { count: machines::int(&entity.nbt, "dropCount").max(1), delay: machines::int(&entity.nbt, "pickupDelay"), ..Dropper::default() };
            match id {
                28 => d.set_count(d.count + 1),
                29 => d.set_count(d.count - 1),
                30 => d.set_delay(d.delay + 1),
                _ => d.set_delay(d.delay - 1),
            }
            machines::put(&mut entity.nbt, "dropCount", d.count);
            machines::put(&mut entity.nbt, "pickupDelay", d.delay);
        }
        32 if block == SENSOR_T2 => {
            let next = Target::from_index(machines::int(&entity.nbt, "senseTarget")).next();
            machines::put(&mut entity.nbt, "senseTarget", next.index());
        }
        33 if block == SENSOR_T2 => {
            let strong = machines::int(&entity.nbt, "strongSignal") == 0;
            machines::put_flag(&mut entity.nbt, "strongSignal", strong);
        }
        60..=62 if block == SENSOR_T2 => machines::put(&mut entity.nbt, "equality", id - 60),
        63 | 64 if block == SENSOR_T2 => {
            let amount = (machines::int(&entity.nbt, "senseAmount") + if id == 63 { 1 } else { -1 }).clamp(0, 9999);
            machines::put(&mut entity.nbt, "senseAmount", amount);
        }
        _ => {
            let facing = Direction::ALL[machines::facing(game, pos)].normal();
            let mut shared = clicker::read_shared(&entity.nbt, facing);
            if shared.button(id) {
                clicker::write_shared(&mut entity.nbt, &shared);
                QUEUES.with(|q| q.borrow_mut().remove(&(dimension, pos)));
            }
            return;
        }
    }
    machines::keep_clock(dimension, pos, &mut entity.nbt, &before, &clock);
}
