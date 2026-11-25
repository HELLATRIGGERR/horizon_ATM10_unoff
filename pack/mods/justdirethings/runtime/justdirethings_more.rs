// Just Dire Things' Dropper T1, Fluid Placer T1, Sensor T1 and Fluid Generator T1 in the game (support module
// `crate::game::justdirethings_more` of the `justdirethings` native bundle). The rules are the bundle's crate
// `justdirethings_core` (`more`); this module is the glue: their saved compounds (Java's keys), the slot each has (the
// hopper menu's first), the dropper's item entity thrown in front of it, the fluid placer's bucket and the fluid block
// it sets, the sensor's block and entity counts and the redstone it gives (`block_signal`), the fluid generator's tank,
// buffer and Forge Energy, and the data their screens read. Translated from Just Dire Things 1.5.7 (MIT); see this
// bundle's README.md.

use super::{
    Game,
    justdirethings_machines::{self as machines, HOPPER_SLOTS, MIN_TICK_SPEED, SLOT},
    justdirethings_world as world,
};
use horizon_client::{containers::ModContainer, items::ItemStack};
use horizon_core::geometry::Direction;
use horizon_core::nbt::{Compound, Tag};
use justdirethings_core::{
    machines::{COLLECTOR_CAPACITY, Clock, Tank},
    more::{Bucket, Dropper, FluidGenerator, FluidPlacer, FluidSpot, GENERATOR_CAPACITY, Sensor, Target, fe_per_mb},
    redstone::Mode,
};
use mod_api::{BlockUpdateFlags, FluidStack, ModBlockEntity, NativeFeInfo, NativeFluidTankInfo, NeighbourUpdate, SoundSource};
use std::{cell::RefCell, collections::HashMap};

pub(super) const DROPPER: &str = "justdirethings:droppert1";
pub(super) const FLUID_PLACER: &str = "justdirethings:fluidplacert1";
pub(super) const SENSOR: &str = "justdirethings:sensort1";
pub(super) const FLUID_GEN: &str = "justdirethings:generatorfluidt1";

/// Whether `block` is one of this module's machines.
pub(super) fn owns(block: &str) -> bool {
    block == DROPPER || block == FLUID_PLACER || block == SENSOR || block == FLUID_GEN
}

thread_local! {
    /// What each sensor gives: whether it emits, whether strongly, and the side it faces (`SensorT1BE.emitRedstone`,
    /// `strongSignal`; Java doesn't save the first), by dimension and position.
    static SENSORS: RefCell<HashMap<(String, [i32; 3]), (bool, bool, usize)>> = RefCell::new(HashMap::new());
}

/// The world opened or closed.
pub(super) fn reset() {
    SENSORS.with(|s| s.borrow_mut().clear());
}

/// `native_block_entity`: Java's defaults.
pub(super) fn new_entity(block: &str, nbt: &mut Compound) {
    machines::write_clock(nbt, &Clock::default());
    machines::put(nbt, "direction", 0);
    if block == DROPPER {
        machines::put(nbt, "dropCount", 1);
        machines::put(nbt, "pickupDelay", 0);
    } else if block == SENSOR {
        machines::put(nbt, "senseTarget", 0);
        machines::put_flag(nbt, "strongSignal", false);
        machines::put(nbt, "senseAmount", 0);
        machines::put(nbt, "equality", 0);
        machines::put_flag(nbt, "allowlist", false);
        machines::put_flag(nbt, "compareNBT", false);
        nbt.insert(machines::key("blockStateProps"), Tag::Compound(Compound::new()));
    } else if block == FLUID_GEN {
        machines::put(nbt, "energy", 0);
    }
}

/// `native_block_entity_container`: the hopper menu with the machine's slot first (the sensor's filter slot no side
/// reaches).
pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if block == SENSOR {
        *container = Some(ModContainer {
            slots: HOPPER_SLOTS,
            faces: Default::default(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            menu: Some("minecraft:hopper".into()),
        });
        return;
    }
    machines::container(block, container);
}

/// `native_block_entity_slot_accepts`: any item in the one slot.
pub(super) fn slot_accepts(slot: usize, accepts: &mut bool) {
    *accepts = *accepts && slot == SLOT;
}

/// One server tick of a machine of this module.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let id = entity.id();
    if id == DROPPER {
        tick_dropper(game, pos, entity);
    } else if id == FLUID_PLACER {
        tick_placer(game, pos, entity);
    } else if id == SENSOR {
        tick_sensor(game, pos, entity);
    } else if id == FLUID_GEN {
        tick_generator(game, pos, entity);
    }
}

fn has_item(entity: &ModBlockEntity) -> bool {
    entity.items.get(SLOT).is_some_and(|s| s.is_some())
}

// ---- The Dropper T1 --------------------------------------------------------------------------------------------

/// `DropperT1BE.doDrop`: `dropCount` of the slot's item thrown from the centre of the block in front, 0.3 blocks a tick
/// the way its direction setting points, with its pickup delay.
fn tick_dropper(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let Some(level) = game.fork_level_here() else { return };
    let dimension = game.native_dimension();
    let signal = game.native_neighbor_signal(dimension, pos) > 0;
    let before = machines::clock_at(dimension, pos, &entity.nbt);
    let mut d = Dropper {
        clock: before,
        direction: machines::int(&entity.nbt, "direction").clamp(0, 5),
        count: if entity.nbt.contains_key(&machines::key("dropCount")) { machines::int(&entity.nbt, "dropCount").max(1) } else { 1 },
        delay: machines::int(&entity.nbt, "pickupDelay").max(0),
    };
    let stack = machines::slot_stack(game, entity);
    if d.tick(stack.is_some(), signal)
        && let Some(stack) = stack
    {
        let want = d.count.try_into().unwrap_or(stack.count);
        let n = stack.count.min(want);
        let mut give = stack.clone();
        give.count = n;
        let mut left = stack.clone();
        left.count = stack.count - n;
        let at = machines::center(machines::beside(pos, machines::facing(game, pos)));
        let [dx, dy, dz] = Direction::ALL[d.direction as usize].normal();
        let motion = [f64::from(dx) * 0.3, f64::from(dy) * 0.3, f64::from(dz) * 0.3];
        let _ = game.level_spawn_item_with_motion(level, at[0], at[1], at[2], give, motion, d.delay.unsigned_abs());
        machines::set_slot(entity, Some(left));
    }
    machines::keep_clock(dimension, pos, &mut entity.nbt, &before, &d.clock);
    let data = vec![d.clock.redstone.mode.index(), i32::from(has_item(entity)), d.direction, d.clock.tick_speed, d.count, d.delay];
    if entity.data != data {
        entity.data = data;
    }
}

// ---- The Fluid Placer T1 ---------------------------------------------------------------------------------------

/// What the slot's container gives, read before the tick (one bucket at a time; a container that holds less than a
/// bucket or fills only part isn't drained in parts).
pub(super) struct Pail {
    probe: Option<(String, i32, ItemStack)>,
    pub(super) after: Option<ItemStack>,
}

impl Pail {
    pub(super) fn new(game: &Game<'_>, stack: Option<&ItemStack>) -> Self {
        let probe = stack.filter(|s| s.count == 1).and_then(|s| game.native_item_fluid_drain(s, 1000)).filter(|(_, amount, _)| *amount > 0);
        Self { probe, after: None }
    }
}

impl Bucket for Pail {
    fn peek(&mut self) -> Option<(String, i32)> {
        self.probe.as_ref().map(|(f, a, _)| (f.clone(), *a))
    }

    fn drain(&mut self, amount: i32) {
        if let Some((_, took, after)) = self.probe.take()
            && took == amount
        {
            self.after = Some(after);
        }
    }
}

/// The fluid block's name of a fluid (`Fluid.defaultFluidState().createLegacyBlock()`): a water or lava as they are, a
/// content fluid's `..._source` as its `..._block`.
fn fluid_block(fluid: &str) -> String {
    match fluid.strip_suffix("_source").or_else(|| fluid.strip_suffix("_flowing")) {
        Some(base) => format!("{base}_block"),
        None => fluid.to_owned(),
    }
}

/// The block in front of a fluid placer.
pub(super) struct Ahead<'a, 'g> {
    pub(super) game: &'a mut Game<'g>,
    pub(super) level: mod_api::LevelRef,
    pub(super) pos: [i32; 3],
    pub(super) dimension: &'static str,
}

impl FluidSpot for Ahead<'_, '_> {
    /// `isBlockPosValid`: the block can be replaced and isn't a source block of a liquid.
    fn valid(&mut self) -> bool {
        let Some(state) = self.game.level_block_state(self.level, self.pos) else { return false };
        let facts = self.game.res.facts().get(state);
        if !facts.replaceable {
            return false;
        }
        if facts.classes.iter().any(|c| c.ends_with("LiquidBlock"))
            && let Some(info) = self.game.pack.assets.registry.state(state)
            && info.properties.get("level").map(String::as_str) == Some("0")
        {
            return false;
        }
        true
    }

    /// `FluidUtil.tryPlaceFluid`: the fluid's block as a source, with the bucket's sound; water in the Nether hisses away
    /// (the fluid is used, the block isn't placed).
    fn place(&mut self, fluid: &str) -> bool {
        let c = machines::center(self.pos);
        if fluid == "minecraft:water" && self.dimension.contains("nether") {
            self.game.level_play_sound(self.level, "minecraft:block.fire.extinguish", c, SoundSource::Blocks, 0.5, 2.6);
            return true;
        }
        let Some(state) = self.game.native_default_state(&fluid_block(fluid)) else { return false };
        let placed = matches!(
            self.game.level_set_block(self.level, self.pos, state, BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS),
            Ok(true)
        );
        if placed {
            let sound = if fluid == "minecraft:lava" { "minecraft:item.bucket.empty_lava" } else { "minecraft:item.bucket.empty" };
            self.game.level_play_sound(self.level, sound, c, SoundSource::Blocks, 1.0, 1.0);
        }
        placed
    }
}

/// One server tick of the fluid placer (`FluidPlacerT1BE.tickServer`), then what its screen shows (the collector's data:
/// the redstone mode, whether the slot holds an item, the tick speed, the fluid, the amount and the capacity).
fn tick_placer(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let Some(level) = game.fork_level_here() else { return };
    let dimension = game.native_dimension();
    let signal = game.native_neighbor_signal(dimension, pos) > 0;
    let before = machines::clock_at(dimension, pos, &entity.nbt);
    let tank = machines::read_tank(&entity.nbt);
    let mut p = FluidPlacer { clock: before, tank: tank.clone() };
    let stack = machines::slot_stack(game, entity);
    let mut pail = Pail::new(game, stack.as_ref());
    let front = machines::beside(pos, machines::facing(game, pos));
    let mut ahead = Ahead { game: &mut *game, level, pos: front, dimension };
    p.tick(signal, &mut pail, &mut ahead);
    if let Some(after) = pail.after.take() {
        machines::set_slot(entity, Some(after));
    }
    if p.tank != tank {
        machines::write_tank(&mut entity.nbt, &p.tank);
    }
    machines::keep_clock(dimension, pos, &mut entity.nbt, &before, &p.clock);
    let data = vec![
        p.clock.redstone.mode.index(),
        i32::from(has_item(entity)),
        p.clock.tick_speed,
        machines::fluid_code(&p.tank),
        p.tank.amount,
        COLLECTOR_CAPACITY,
    ];
    if entity.data != data {
        entity.data = data;
    }
}

// ---- The Sensor T1 ---------------------------------------------------------------------------------------------

/// `level.neighborChanged` and `updateNeighborsAtExceptFromFacing` of each side's block: what a changed signal does.
fn notify(game: &mut Game<'_>, level: mod_api::LevelRef, pos: [i32; 3]) {
    let Some(state) = game.level_block_state(level, pos) else { return };
    for side in 0..6 {
        let at = machines::beside(pos, side);
        let _ = game.level_neighbor_changed(level, at, state);
        let _ = game.level_update_neighbors(level, at, state, NeighbourUpdate::NeighboursExcept(Direction::ALL[side].opposite()));
    }
}

/// `isStackValidFilter` of one filter item against `name`: its list's verdict when it is that item, the opposite else.
fn filter_passes(filter: Option<&ItemStack>, allowlist: bool, name: &str) -> bool {
    match filter {
        Some(f) if f.item.to_string() == name => allowlist,
        _ => !allowlist,
    }
}

/// `senseBlock` of the block in front: 1 when it counts. A liquid is tested by its bucket.
pub(super) fn sense_block(game: &Game<'_>, level: mod_api::LevelRef, front: [i32; 3], target: Target, accepts: &dyn Fn(&str) -> bool) -> i32 {
    let Some(state) = game.level_block_state(level, front) else { return 0 };
    let facts = game.res.facts().get(state);
    if target == Target::Air {
        return i32::from(facts.air);
    }
    if facts.air {
        return 0;
    }
    let name = facts.name.to_string();
    let item = if facts.classes.iter().any(|c| c.ends_with("LiquidBlock")) {
        format!("{}_bucket", name.strip_suffix("_block").unwrap_or(&name))
    } else {
        name
    };
    i32::from(accepts(&item))
}

/// `findEntities` in the block in front, each kept by `isValidEntity`: its kind, then the filter (spawn eggs).
pub(super) fn sense_entities(game: &Game<'_>, level: mod_api::LevelRef, min: [f64; 3], max: [f64; 3], target: Target, accepts: &dyn Fn(&str) -> bool) -> i32 {
    let egg = |kind: &str| accepts(&format!("{kind}_spawn_egg"));
    let mut found = 0;
    if matches!(target, Target::Hostile | Target::Passive | Target::Adult | Target::Child | Target::Living) {
        for m in game.mobs.iter().filter(|m| m.alive()) {
            let b = m.bounds();
            let (lo, hi) = (b.min_position(), b.max_position());
            if !(lo.x < max[0] && hi.x > min[0] && lo.y < max[1] && hi.y > min[1] && lo.z < max[2] && hi.z > min[2]) {
                continue;
            }
            let monster = m.kind.category == "monster";
            let animal = m.kind.category == "creature";
            let kept = match target {
                Target::Hostile => monster,
                Target::Passive => animal,
                Target::Adult => animal && !m.is_baby(),
                Target::Child => animal && m.is_baby(),
                _ => true,
            };
            if kept && egg(&m.kind.id) {
                found += 1;
            }
        }
    }
    if matches!(target, Target::Player | Target::Living)
        && let Some(entities) = game.level_entities_in(level, min, max)
    {
        for e in entities {
            if game.entity_kind(e).is_some_and(|k| game.entity_type_key_id(k) == "minecraft:player") && egg("minecraft:player") {
                found += 1;
            }
        }
    }
    if target == Target::Item {
        for d in game.dropped.iter() {
            let p = d.position;
            if p.x - 0.125 < max[0] && p.x + 0.125 > min[0] && p.y < max[1] && p.y + 0.25 > min[1] && p.z - 0.125 < max[2] && p.z + 0.125 > min[2] && egg("minecraft:item") {
                found += 1;
            }
        }
    }
    found
}

/// One server tick of the sensor (`SensorT1BE.tickServer`, `sense`): on its clock it counts what is in front of it and
/// sets its signal; a changed signal (or a newly loaded sensor) tells the neighbours.
fn tick_sensor(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let Some(level) = game.fork_level_here() else { return };
    let dimension = game.native_dimension();
    let side = machines::facing(game, pos);
    let before = machines::clock_at(dimension, pos, &entity.nbt);
    let mut s = Sensor {
        clock: before,
        target: Target::from_index(machines::int(&entity.nbt, "senseTarget")),
        strong: machines::int(&entity.nbt, "strongSignal") != 0,
        amount: machines::int(&entity.nbt, "senseAmount"),
        equality: machines::int(&entity.nbt, "equality"),
    };
    let allowlist = machines::int(&entity.nbt, "allowlist") != 0;
    let filter = machines::slot_stack(game, entity);
    let accepts = |item: &str| filter_passes(filter.as_ref(), allowlist, item);
    let sensed = s.tick().then(|| {
        let front = machines::beside(pos, side);
        let found = if s.target.is_block() {
            sense_block(game, level, front, s.target, &accepts)
        } else {
            sense_entities(game, level, front.map(f64::from), front.map(|v| f64::from(v) + 1.0), s.target, &accepts)
        };
        s.check_count(found)
    });
    sensor_signal(game, level, dimension, pos, side, sensed, s.strong);
    machines::keep_clock(dimension, pos, &mut entity.nbt, &before, &s.clock);
    let data = vec![s.target.index(), i32::from(s.strong), i32::from(allowlist), i32::from(has_item(entity)), s.clock.tick_speed];
    if entity.data != data {
        entity.data = data;
    }
}

/// A sensor's signal after a tick: `sensed` is what it found this tick (none when it did not sense); a changed signal (or a
/// newly loaded sensor) tells the neighbours.
pub(super) fn sensor_signal(game: &mut Game<'_>, level: mod_api::LevelRef, dimension: &'static str, pos: [i32; 3], side: usize, sensed: Option<bool>, strong: bool) {
    let key = (dimension.to_owned(), pos);
    let known = SENSORS.with(|m| m.borrow().get(&key).copied());
    let emit = sensed.unwrap_or_else(|| known.is_some_and(|(e, _, _)| e));
    let state = (emit, strong, side);
    SENSORS.with(|m| m.borrow_mut().insert(key, state));
    if known != Some(state) && (known.is_some() || emit || strong) {
        notify(game, level, pos);
    }
}

/// `native_is_signal_source`.
pub(super) fn is_signal_source(block: Option<&str>, source: &mut bool) {
    if block == Some(SENSOR) || block == Some(super::justdirethings_t2x::SENSOR_T2) {
        *source = true;
    }
}

/// `SensorT1.getSignal` and `getDirectSignal`: 15 on every side but the one it faces while it senses, the direct
/// (strong) signal only when its strong setting is on. `face` is the side the reader asks from.
pub(super) fn block_signal(dimension: &str, block: Option<&str>, pos: [i32; 3], face: Direction, direct: bool, signal: &mut i32) {
    if block != Some(SENSOR) && block != Some(super::justdirethings_t2x::SENSOR_T2) {
        return;
    }
    let state = SENSORS.with(|m| m.borrow().get(&(dimension.to_owned(), pos)).copied());
    *signal = match state {
        Some((emit, strong, facing)) if face != Direction::ALL[facing].opposite() && emit && (strong || !direct) => 15,
        _ => 0,
    };
}

// ---- The Fluid Generator T1 ------------------------------------------------------------------------------------

/// One server tick of the fluid generator (`GeneratorFluidT1BE.tickServer`): the slot's container into the tank
/// (refined fuel only), burning, then the push to the six neighbours.
fn tick_generator(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let cfg = world::settings().fluid_generator;
    let dimension = game.native_dimension();
    let signal = game.native_neighbor_signal(dimension, pos) > 0;
    let tank = machines::read_tank(&entity.nbt);
    let redstone = machines::read_clock(&entity.nbt).redstone;
    let before = FluidGenerator { energy: machines::int(&entity.nbt, "energy"), tank, redstone };
    let mut g = before.clone();
    let stack = machines::slot_stack(game, entity);
    let mut pail = Pail::new(game, stack.as_ref());
    g.tick(&cfg, signal, &mut pail);
    if let Some(after) = pail.after.take() {
        machines::set_slot(entity, Some(after));
    }
    g.push(&cfg, |side, amount, simulate| {
        game.native_fe_offer(dimension, machines::beside(pos, side), Some(Direction::ALL[side].opposite()), amount, simulate, false)
    });
    if g.tank != before.tank {
        machines::write_tank(&mut entity.nbt, &g.tank);
    }
    if g.energy != before.energy {
        machines::put(&mut entity.nbt, "energy", g.energy);
    }
    if g.redstone != before.redstone {
        let mut c = machines::read_clock(&entity.nbt);
        c.redstone = g.redstone;
        machines::write_clock(&mut entity.nbt, &c);
    }
    let data = vec![
        g.redstone.mode.index(),
        i32::from(has_item(entity)),
        machines::fluid_code(&g.tank),
        g.tank.amount,
        GENERATOR_CAPACITY,
        g.energy,
        cfg.max_fe,
    ];
    if entity.data != data {
        entity.data = data;
    }
}

/// `native_fe_storage`: every side gives and none takes (`EnergyStorageNoReceive`).
pub(super) fn fe_storage(entity: &ModBlockEntity, info: &mut Option<NativeFeInfo>) {
    *info = Some(NativeFeInfo {
        stored: i64::from(machines::int(&entity.nbt, "energy")),
        capacity: i64::from(world::settings().fluid_generator.max_fe),
        can_receive: false,
        can_extract: true,
    });
}

/// `native_fe_extract`: what the buffer has, up to `max`.
pub(super) fn fe_extract(entity: &mut ModBlockEntity, max: i64, simulate: bool, given: &mut Option<i64>) {
    let cfg = world::settings().fluid_generator;
    let mut g = FluidGenerator { energy: machines::int(&entity.nbt, "energy"), ..FluidGenerator::default() };
    let out = g.extract(&cfg, max, simulate);
    if out > 0 && !simulate {
        machines::put(&mut entity.nbt, "energy", g.energy);
    }
    *given = Some(out);
}

/// The generator's tank as a fluid handler: one tank of refined fuel, 4,000 mB.
pub(super) fn fluid_tanks(entity: &ModBlockEntity, tanks: &mut Option<Vec<NativeFluidTankInfo>>) {
    let t = machines::read_tank(&entity.nbt);
    let stack = t.fluid.as_deref().and_then(mod_api::__private::fluid_key).map(|k| FluidStack::new(k, t.amount.unsigned_abs())).filter(|_| t.amount > 0);
    *tanks = Some(vec![NativeFluidTankInfo { stack, capacity: GENERATOR_CAPACITY, drainable: t.amount > 0 }]);
}

/// `IFluidHandler.fill` on the generator's tank: refined fuel only (`isFluidValid`).
pub(super) fn fluid_fill(entity: &mut ModBlockEntity, resource: &FluidStack, simulate: bool, filled: &mut Option<i32>) {
    let cfg = world::settings().fluid_generator;
    let Some(fluid) = resource.key().and_then(mod_api::__private::fluid_id).filter(|f| fe_per_mb(&cfg, f) > 0) else {
        *filled = Some(0);
        return;
    };
    let mut t: Tank = machines::read_tank(&entity.nbt);
    let took = t.fill(fluid, i32::try_from(resource.amount).unwrap_or(i32::MAX), GENERATOR_CAPACITY, simulate);
    if took > 0 && !simulate {
        machines::write_tank(&mut entity.nbt, &t);
    }
    *filled = Some(took);
}

// ---- The screens' buttons --------------------------------------------------------------------------------------

/// `native_menu_button`: 0 to 3 the redstone mode; the dropper's 4 to 9 direction, 22 and 23 tick speed, 28 and 29 drop
/// count, 30 and 31 pickup delay; the sensor's 24 allowlist, 32 target (the next), 33 strong or weak, 22 and 23 tick speed.
pub(super) fn button(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, id: i32) {
    let dimension = game.native_dimension();
    let before = machines::clock_at(dimension, pos, &entity.nbt);
    let mut c = before;
    let block = entity.id().to_owned();
    match id {
        0..=3 if block != SENSOR => c.redstone.mode = Mode::from_index(id),
        4..=9 if block == DROPPER => machines::put(&mut entity.nbt, "direction", id - 4),
        22 if block != FLUID_GEN => c.set_tick_speed(c.tick_speed + 1, MIN_TICK_SPEED),
        23 if block != FLUID_GEN => c.set_tick_speed(c.tick_speed - 1, MIN_TICK_SPEED),
        28..=31 if block == DROPPER => {
            let mut d = Dropper {
                count: machines::int(&entity.nbt, "dropCount").max(1),
                delay: machines::int(&entity.nbt, "pickupDelay"),
                ..Dropper::default()
            };
            match id {
                28 => d.set_count(d.count + 1),
                29 => d.set_count(d.count - 1),
                30 => d.set_delay(d.delay + 1),
                _ => d.set_delay(d.delay - 1),
            }
            machines::put(&mut entity.nbt, "dropCount", d.count);
            machines::put(&mut entity.nbt, "pickupDelay", d.delay);
        }
        24 if block == SENSOR => {
            let allow = machines::int(&entity.nbt, "allowlist") == 0;
            machines::put_flag(&mut entity.nbt, "allowlist", allow);
        }
        32 if block == SENSOR => {
            let next = Target::from_index(machines::int(&entity.nbt, "senseTarget")).next();
            machines::put(&mut entity.nbt, "senseTarget", next.index());
        }
        33 if block == SENSOR => {
            let strong = machines::int(&entity.nbt, "strongSignal") == 0;
            machines::put_flag(&mut entity.nbt, "strongSignal", strong);
        }
        _ => return,
    }
    machines::keep_clock(dimension, pos, &mut entity.nbt, &before, &c);
}
