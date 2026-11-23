// Just Dire Things' Block Placer T1, Fluid Collector T1 and Item Collector in the game (support module
// `crate::game::justdirethings_machines` of the `justdirethings` native bundle). The rules are the bundle's crate
// `justdirethings_core` (`machines`); this module is the glue: their saved compounds (Java's keys), the slot each has
// (the hopper menu's first, as the breaker's), the placer's fake player use of a block item on the block in front
// (`BlockItem.place` through the world's placement, as a dispenser places a shulker box), the collector's source block
// in front, its tank (`Fluid`, as `FluidTank` writes it), its fluid handler and the container in its slot, the item
// collector's filter slots, the item entities of its area and the inventory it fills, and the data their screens read. Translated from Just Dire Things 1.5.7 (MIT); see this bundle's README.md.

use super::Game;
use crate::game::api_values::{TryIntoApi, TryIntoEngine};
use horizon_client::{containers::ModContainer, items::ItemStack};
use horizon_core::geometry::{Direction, Vec3};
use horizon_core::nbt::{Compound, JavaString, Tag};
use horizon_core::position::BlockPos;
use horizon_world::fluid::FluidState;
use horizon_world::level::{ItemKind, PlaceContext, PlaceOutcome};
use justdirethings_core::{
    SIDE_NAMES,
    machines::{COLLECTOR_CAPACITY, Clock, Collector, Container, FILTER_SLOTS, ItemCollector, Placer, Source, Spot, Tank},
    redstone::{Mode, Redstone},
};
use mod_api::{BlockUpdateFlags, FluidStack, ModBlockEntity, NativeFluidTankInfo, SoundSource};
use std::{cell::RefCell, collections::HashMap};

pub(super) const PLACER: &str = "justdirethings:blockplacert1";
pub(super) const COLLECTOR: &str = "justdirethings:fluidcollectort1";
pub(super) const ITEM_COLLECTOR: &str = "justdirethings:itemcollector";

/// Whether `block` is one of this module's machines.
pub(super) fn owns(block: &str) -> bool {
    block == PLACER || block == COLLECTOR || block == ITEM_COLLECTOR
}

/// The one slot (`MACHINE_SLOTS = 1`), the hopper menu's first.
const SLOT: usize = 0;
const HOPPER_SLOTS: usize = 5;
/// `minimum_machine_tick_speed`'s default.
const MIN_TICK_SPEED: i32 = 1;

thread_local! {
    /// The machines' operation clocks (`operationTicks`, which Java doesn't save), by dimension and position.
    static CLOCKS: RefCell<HashMap<(&'static str, [i32; 3]), i32>> = RefCell::new(HashMap::new());
    /// Whether an item is a block item (`instanceof BlockItem`), by its name.
    static BLOCK_ITEMS: RefCell<HashMap<String, bool>> = RefCell::new(HashMap::new());
}

/// The world opened or closed.
pub(super) fn reset() {
    CLOCKS.with(|c| c.borrow_mut().clear());
    BLOCK_ITEMS.with(|b| b.borrow_mut().clear());
}

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

fn put(nbt: &mut Compound, name: &str, v: i32) {
    nbt.insert(key(name), Tag::Int(v));
}

fn put_flag(nbt: &mut Compound, name: &str, v: bool) {
    nbt.insert(key(name), Tag::Byte(i8::from(v)));
}

fn double(nbt: &Compound, name: &str) -> f64 {
    match nbt.get(&key(name)) {
        Some(Tag::Double(v)) => f64::from_bits(*v),
        Some(Tag::Float(v)) => f64::from(f32::from_bits(*v)),
        Some(Tag::Int(v)) => f64::from(*v),
        _ => 0.0,
    }
}

fn center(pos: [i32; 3]) -> [f64; 3] {
    [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.5, f64::from(pos[2]) + 0.5]
}

/// `saveRedstoneSettings` and `tickspeed`: the clock's saved half.
fn read_clock(nbt: &Compound) -> Clock {
    Clock {
        redstone: Redstone {
            mode: Mode::from_index(int(nbt, "redstoneMode")),
            pulsed: int(nbt, "pulsed") != 0,
            receiving: int(nbt, "receivingRedstone") != 0,
        },
        tick_speed: if nbt.contains_key(&key("tickspeed")) { int(nbt, "tickspeed").max(1) } else { 20 },
        operation_ticks: -1,
    }
}

fn write_clock(nbt: &mut Compound, c: &Clock) {
    put(nbt, "redstoneMode", c.redstone.mode.index());
    nbt.insert(key("pulsed"), Tag::Byte(i8::from(c.redstone.pulsed)));
    nbt.insert(key("receivingRedstone"), Tag::Byte(i8::from(c.redstone.receiving)));
    put(nbt, "tickspeed", c.tick_speed);
}

/// The collector's tank, `{Fluid: {id, amount}}` as `FluidTank.writeToNBT` writes it (no `Fluid` while empty).
fn read_tank(nbt: &Compound) -> Tank {
    let Some(Tag::Compound(f)) = nbt.get(&key("Fluid")) else { return Tank::default() };
    let Some(Tag::String(id)) = f.get(&key("id")) else { return Tank::default() };
    let amount = int(f, "amount").clamp(0, COLLECTOR_CAPACITY);
    if amount == 0 {
        return Tank::default();
    }
    Tank { fluid: Some(id.to_string_lossy()), amount }
}

fn write_tank(nbt: &mut Compound, t: &Tank) {
    match t.fluid.as_deref().filter(|_| t.amount > 0) {
        Some(fluid) => {
            let mut f = Compound::new();
            f.insert(key("id"), Tag::String(JavaString::from(fluid)));
            put(&mut f, "amount", t.amount);
            nbt.insert(key("Fluid"), Tag::Compound(f));
        }
        None => {
            nbt.remove(&key("Fluid"));
        }
    }
}

/// `native_block_entity`: Java's defaults (redstone ignored, tick speed 20, direction down, an empty tank).
pub(super) fn new_entity(nbt: &mut Compound) {
    write_clock(nbt, &Clock::default());
    put(nbt, "direction", 0);
}

/// `native_block_entity_container`: the hopper menu with the machine's slot first, which hoppers fill from every side;
/// the item collector's nine filter slots in the 3x3 menu, which no side reaches (`FilterBasicHandler` is no
/// capability).
pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if block == ITEM_COLLECTOR {
        *container = Some(ModContainer {
            slots: FILTER_SLOTS,
            faces: Default::default(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            menu: Some("minecraft:generic_3x3".into()),
        });
        return;
    }
    let slot = || vec![SLOT];
    *container = Some(ModContainer {
        slots: HOPPER_SLOTS,
        faces: [slot(), slot(), slot(), slot(), slot(), slot()],
        inputs: vec![SLOT],
        outputs: Vec::new(),
        menu: Some("minecraft:hopper".into()),
    });
}

/// `native_block_entity_slot_accepts`: any item in the machine's slot (Java's machine handler checks none), nothing in
/// the menu's others; any item in the item collector's filter slots.
pub(super) fn slot_accepts(block: &str, slot: usize, accepts: &mut bool) {
    *accepts = *accepts && if block == ITEM_COLLECTOR { slot < FILTER_SLOTS } else { slot == SLOT };
}

fn facing(game: &Game<'_>, pos: [i32; 3]) -> usize {
    game.native_block_property(pos, "facing").and_then(|f| SIDE_NAMES.iter().position(|n| *n == f)).unwrap_or(2)
}

fn beside(pos: [i32; 3], side: usize) -> [i32; 3] {
    let [dx, dy, dz] = Direction::ALL[side].normal();
    [pos[0] + dx, pos[1] + dy, pos[2] + dz]
}

fn slot_stack(game: &Game<'_>, entity: &ModBlockEntity) -> Option<ItemStack> {
    entity.items.get(SLOT).and_then(|s| s.as_ref()).and_then(|s| s.try_into_engine(game.items()).ok()).filter(|s| s.count > 0)
}

fn set_slot(entity: &mut ModBlockEntity, stack: Option<ItemStack>) {
    if entity.items.len() < HOPPER_SLOTS {
        entity.items.resize(HOPPER_SLOTS, None);
    }
    entity.items[SLOT] = stack.filter(|s| s.count > 0).as_ref().and_then(|s| s.try_into_api().ok());
}

/// The clock of the machine at `pos` with its transient tick count.
fn clock_at(dimension: &'static str, pos: [i32; 3], nbt: &Compound) -> Clock {
    let mut c = read_clock(nbt);
    if let Some(ticks) = CLOCKS.with(|m| m.borrow().get(&(dimension, pos)).copied()) {
        c.operation_ticks = ticks;
    }
    c
}

fn keep_clock(dimension: &'static str, pos: [i32; 3], nbt: &mut Compound, before: &Clock, c: &Clock) {
    CLOCKS.with(|m| m.borrow_mut().insert((dimension, pos), c.operation_ticks));
    if (c.redstone, c.tick_speed) != (before.redstone, before.tick_speed) {
        write_clock(nbt, c);
    }
}

/// One server tick of a machine of this module.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    if entity.id() == PLACER {
        tick_placer(game, pos, entity);
    } else if entity.id() == COLLECTOR {
        tick_collector(game, pos, entity);
    } else if entity.id() == ITEM_COLLECTOR {
        tick_item_collector(game, pos, entity);
    }
}

// ---- The Block Placer T1 ---------------------------------------------------------------------------------------

/// `isStackValid`: an item the world places as a block (`instanceof BlockItem`).
fn is_block_item(game: &mut Game<'_>, item: &str) -> bool {
    if let Some(known) = BLOCK_ITEMS.with(|b| b.borrow().get(item).copied()) {
        return known;
    }
    let found = game.with_level(|b, _, _| b.item_binding(item).is_some_and(|i| i.kind != ItemKind::Other));
    BLOCK_ITEMS.with(|b| b.borrow_mut().insert(item.to_owned(), found));
    found
}

/// `Direction.toYRot`: down and up read as 270, as their 2D value is -1.
fn y_rot(d: Direction) -> f32 {
    match d {
        Direction::South => 0.0,
        Direction::West => 90.0,
        Direction::North => 180.0,
        _ => 270.0,
    }
}

/// The block in front of a placer and the stack its fake player holds.
struct PlaceSpot<'a, 'g> {
    game: &'a mut Game<'g>,
    level: mod_api::LevelRef,
    pos: [i32; 3],
    /// The face the fake player clicks (`getDirectionValue`).
    direction: Direction,
    stack: Option<ItemStack>,
    /// The stack after the placement, when it placed.
    used: Option<Option<ItemStack>>,
}

impl Spot for PlaceSpot<'_, '_> {
    fn valid(&mut self) -> bool {
        self.game.level_block_state(self.level, self.pos).is_some_and(|s| self.game.res.facts().get(s).replaceable)
    }

    /// `placeBlock`: `ItemStack.useOn` with the hit at the block's centre on face `direction`, by a fake player
    /// turned that way (`setupFakePlayerForUse`: pitch 90 down, -90 up), not sneaking; the item is used up as a
    /// survival player's.
    fn place(&mut self) {
        let Some(stack) = self.stack.clone() else { return };
        let [x, y, z] = self.pos;
        let pitch = match self.direction {
            Direction::Down => 90.0,
            Direction::Up => -90.0,
            _ => 0.0,
        };
        let ctx = PlaceContext {
            clicked_pos: BlockPos::new(x, y, z),
            face: self.direction,
            hit: Vec3::new(f64::from(x) + 0.5, f64::from(y) + 0.5, f64::from(z) + 0.5),
            inside: false,
            yaw: y_rot(self.direction),
            pitch,
            sneaking: false,
            item: stack.item.to_string(),
            can_use_game_master_blocks: false,
        };
        let outcome = self.game.with_level(|b, level, ticks| b.place_block(level, ticks, &ctx));
        if let PlaceOutcome::Placed { pos: placed, state } = outcome {
            let target = (placed.x, placed.y, placed.z);
            if !self.game.world.block_entities.stateful_entity(state.0)
                && let Some(entity) = self.game.new_block_entity_for_state(state)
            {
                self.game.world.block_entities.insert(target, entity);
            }
            self.game.block_item_placed(&stack, None, placed, &ctx);
            let mut left = stack;
            left.count = left.count.saturating_sub(1);
            self.used = Some((left.count > 0).then_some(left));
        }
        let _ = self.game.process_level_events();
    }
}

/// One server tick of the placer (`BlockPlacerT1BE.tickServer`), then what its screen shows.
fn tick_placer(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let Some(level) = game.fork_level_here() else { return };
    let dimension = game.native_dimension();
    let signal = game.native_neighbor_signal(dimension, pos) > 0;
    let stack = slot_stack(game, entity);
    let block_item = stack.as_ref().is_some_and(|s| is_block_item(game, &s.item));
    let before = clock_at(dimension, pos, &entity.nbt);
    let direction = int(&entity.nbt, "direction").clamp(0, 5);
    let mut p = Placer { clock: before, direction };
    let front = beside(pos, facing(game, pos));
    let mut spot = PlaceSpot {
        game: &mut *game,
        level,
        pos: front,
        direction: Direction::ALL[direction as usize],
        stack,
        used: None,
    };
    p.tick(block_item, signal, &mut spot);
    if let Some(left) = spot.used.take() {
        set_slot(entity, left);
    }
    keep_clock(dimension, pos, &mut entity.nbt, &before, &p.clock);
    let has_item = entity.items.get(SLOT).is_some_and(|s| s.is_some());
    let data = vec![p.clock.redstone.mode.index(), i32::from(has_item), direction, p.clock.tick_speed];
    if entity.data != data {
        entity.data = data;
    }
}

// ---- The Fluid Collector T1 ------------------------------------------------------------------------------------

/// What is around a collector: the source block in front and the fluid container in its slot, one item of which takes
/// fluid (a bucket's `FluidBucketWrapper` fills only alone).
struct Around<'a, 'g> {
    game: &'a mut Game<'g>,
    level: mod_api::LevelRef,
    pos: [i32; 3],
    stack: Option<ItemStack>,
    changed: bool,
}

impl Source for Around<'_, '_> {
    fn source(&mut self) -> Option<String> {
        let state = self.game.level_block_state(self.level, self.pos)?;
        // `getBlock() instanceof LiquidBlock` and `LEVEL` 0.
        if !self.game.res.facts().get(state).classes.iter().any(|c| c.ends_with("LiquidBlock")) {
            return None;
        }
        let info = self.game.pack.assets.registry.state(state)?;
        if info.properties.get("level").map(String::as_str) != Some("0") {
            return None;
        }
        let fluid = FluidState::from_block(info).ok()?;
        Some(fluid.kind().name().to_owned())
    }

    fn take(&mut self) -> bool {
        let Some(air) = self.game.native_default_state("minecraft:air") else { return false };
        let changed = matches!(
            self.game.level_set_block(self.level, self.pos, air, BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS),
            Ok(true)
        );
        if changed {
            self.game.level_play_sound(self.level, "minecraft:item.bucket.fill", center(self.pos), SoundSource::Blocks, 1.0, 1.0);
        }
        changed
    }
}

impl Container for Around<'_, '_> {
    fn would_take(&mut self, fluid: &str, amount: i32) -> i32 {
        let Some(stack) = self.stack.as_ref().filter(|s| s.count == 1) else { return 0 };
        self.game.native_item_fluid_fill(stack, fluid, amount).map_or(0, |(took, _)| took)
    }

    fn fill(&mut self, fluid: &str, amount: i32) {
        let Some(stack) = self.stack.as_ref() else { return };
        if let Some((took, after)) = self.game.native_item_fluid_fill(stack, fluid, amount)
            && took > 0
        {
            self.stack = Some(after);
            self.changed = true;
        }
    }
}

/// What the screen's fluid bar draws: 0 nothing, 1 water, 2 lava, 3 another fluid.
fn fluid_code(t: &Tank) -> i32 {
    match t.fluid.as_deref().filter(|_| t.amount > 0) {
        None => 0,
        Some("minecraft:water") => 1,
        Some("minecraft:lava") => 2,
        Some(_) => 3,
    }
}

/// One server tick of the collector (`FluidCollectorT1BE.tickServer`), then what its screen shows.
fn tick_collector(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let Some(level) = game.fork_level_here() else { return };
    let dimension = game.native_dimension();
    let signal = game.native_neighbor_signal(dimension, pos) > 0;
    let before = clock_at(dimension, pos, &entity.nbt);
    let tank = read_tank(&entity.nbt);
    let mut c = Collector { clock: before, tank: tank.clone() };
    let front = beside(pos, facing(game, pos));
    let stack = slot_stack(game, entity);
    let mut around = Around { game: &mut *game, level, pos: front, stack, changed: false };
    c.tick(signal, &mut around);
    if around.changed {
        let filled = around.stack.take();
        set_slot(entity, filled);
    }
    if c.tank != tank {
        write_tank(&mut entity.nbt, &c.tank);
    }
    keep_clock(dimension, pos, &mut entity.nbt, &before, &c.clock);
    let has_item = entity.items.get(SLOT).is_some_and(|s| s.is_some());
    let data = vec![c.clock.redstone.mode.index(), i32::from(has_item), c.clock.tick_speed, fluid_code(&c.tank), c.tank.amount, COLLECTOR_CAPACITY];
    if entity.data != data {
        entity.data = data;
    }
}

/// The collector's tank as a fluid handler (`MACHINE_FLUID_HANDLER` on every side): one tank of any fluid.
pub(super) fn fluid_tanks(entity: &ModBlockEntity, tanks: &mut Option<Vec<NativeFluidTankInfo>>) {
    let t = read_tank(&entity.nbt);
    let stack = t.fluid.as_deref().and_then(mod_api::__private::fluid_key).map(|k| FluidStack::new(k, t.amount.unsigned_abs())).filter(|_| t.amount > 0);
    *tanks = Some(vec![NativeFluidTankInfo { stack, capacity: COLLECTOR_CAPACITY, drainable: t.amount > 0 }]);
}

/// `IFluidHandler.fill` on the collector's tank.
pub(super) fn fluid_fill(entity: &mut ModBlockEntity, resource: &FluidStack, simulate: bool, filled: &mut Option<i32>) {
    let Some(fluid) = resource.key().and_then(mod_api::__private::fluid_id) else {
        *filled = Some(0);
        return;
    };
    let mut t = read_tank(&entity.nbt);
    let took = t.fill(fluid, i32::try_from(resource.amount).unwrap_or(i32::MAX), COLLECTOR_CAPACITY, simulate);
    if took > 0 && !simulate {
        write_tank(&mut entity.nbt, &t);
    }
    *filled = Some(took);
}

/// `IFluidHandler.drain` on the collector's tank: of the fluid asked (`resource`), or of whatever it holds.
pub(super) fn fluid_drain(entity: &mut ModBlockEntity, resource: Option<&FluidStack>, max: i32, simulate: bool, drained: &mut Option<FluidStack>) {
    let mut t = read_tank(&entity.nbt);
    let asked = resource.and_then(|r| r.key()).and_then(mod_api::__private::fluid_id);
    if let Some(r) = resource
        && (asked.is_none() || asked != t.fluid.as_deref())
    {
        *drained = Some(r.with_amount(0));
        return;
    }
    let max = resource.map_or(max, |r| i32::try_from(r.amount).unwrap_or(i32::MAX));
    let out = t.drain(max, simulate);
    if out.is_some() && !simulate {
        write_tank(&mut entity.nbt, &t);
    }
    *drained = out
        .and_then(|(fluid, amount)| mod_api::__private::fluid_key(&fluid).map(|k| FluidStack::new(k, amount.unsigned_abs())))
        .or_else(|| resource.map(|r| r.with_amount(0)));
}

/// The screens' buttons (`native_menu_button`): 0 to 3 the redstone mode, 4 to 9 the placer's direction, 22 and 23
/// the tick speed up or down; the item collector's in `item_collector_button`.
pub(super) fn button(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, id: i32) {
    if entity.id() == ITEM_COLLECTOR {
        item_collector_button(game, pos, entity, id);
        return;
    }
    let dimension = game.native_dimension();
    let before = clock_at(dimension, pos, &entity.nbt);
    let mut c = before;
    match id {
        0..=3 => c.redstone.mode = Mode::from_index(id),
        4..=9 if entity.id() == PLACER => put(&mut entity.nbt, "direction", id - 4),
        22 => c.set_tick_speed(c.tick_speed + 1, MIN_TICK_SPEED),
        23 => c.set_tick_speed(c.tick_speed - 1, MIN_TICK_SPEED),
        _ => return,
    }
    keep_clock(dimension, pos, &mut entity.nbt, &before, &c);
}

// ---- The Item Collector ----------------------------------------------------------------------------------------

/// The item collector's saved settings: the clock's (`saveRedstoneSettings`, `tickspeed`), `saveAreaSettings`,
/// `saveFilterSettings` and its own two. A compound without the area (a new one) has Java's defaults.
fn read_item_collector(nbt: &Compound, facing: [i32; 3]) -> ItemCollector {
    let mut c = ItemCollector::new(facing);
    c.clock = read_clock(nbt);
    if nbt.contains_key(&key("xRadiusDouble")) {
        c.area.radius = [double(nbt, "xRadiusDouble"), double(nbt, "yRadiusDouble"), double(nbt, "zRadiusDouble")];
        c.area.offset = [int(nbt, "xOffset"), int(nbt, "yOffset"), int(nbt, "zOffset")];
        c.area.render = int(nbt, "renderArea") != 0;
        c.filter.allowlist = int(nbt, "allowlist") != 0;
        c.filter.compare_nbt = int(nbt, "compareNBT") != 0;
        c.respect_pickup_delay = int(nbt, "respectPickupDelay") != 0;
        c.show_particles = int(nbt, "showParticles") != 0;
    }
    c
}

fn write_item_collector(nbt: &mut Compound, c: &ItemCollector) {
    write_clock(nbt, &c.clock);
    for (i, axis) in ["x", "y", "z"].into_iter().enumerate() {
        nbt.insert(key(&format!("{axis}RadiusDouble")), Tag::Double(c.area.radius[i].to_bits()));
        put(nbt, &format!("{axis}Offset"), c.area.offset[i]);
    }
    put_flag(nbt, "renderArea", c.area.render);
    put_flag(nbt, "allowlist", c.filter.allowlist);
    put_flag(nbt, "compareNBT", c.filter.compare_nbt);
    put(nbt, "blockitemfilter", -1);
    put_flag(nbt, "respectPickupDelay", c.respect_pickup_delay);
    put_flag(nbt, "showParticles", c.show_particles);
}

/// What the item collector's screen reads: the redstone mode, the tick speed, the allowlist, compare NBT, respect
/// pickup delay, show particles and render area flags, the radius of each axis in half blocks and the offsets.
fn item_collector_data(c: &ItemCollector) -> Vec<i32> {
    let mut data = vec![
        c.clock.redstone.mode.index(),
        c.clock.tick_speed,
        i32::from(c.filter.allowlist),
        i32::from(c.filter.compare_nbt),
        i32::from(c.respect_pickup_delay),
        i32::from(c.show_particles),
        i32::from(c.area.render),
    ];
    data.extend(c.area.radius.map(|r| (r * 2.0).round() as i32));
    data.extend(c.area.offset);
    data
}

/// `findItemsAndStore`: the item entities whose box meets the area (`getEntitiesOfClass(ItemEntity, getAABB)`), each the
/// filter lets through put into the inventory in front through the side facing the collector
/// (`ItemHandlerHelper.insertItemStacked`): an item that all went in is discarded, one that partly did keeps the rest.
/// No inventory in front, nothing moves. Java's flow particles (`showParticles`) are not drawn.
fn collect_items(game: &mut Game<'_>, pos: [i32; 3], side: usize, c: &ItemCollector, entity: &ModBlockEntity) {
    let (min, max) = c.area.bounds(pos);
    // An item entity is 0.25 wide and high, standing on its position (`AABB.intersects`).
    let found: Vec<(u32, ItemStack, bool)> = game
        .dropped
        .iter()
        .filter(|d| {
            let p = d.position;
            p.x - 0.125 < max[0] && p.x + 0.125 > min[0] && p.y < max[1] && p.y + 0.25 > min[1] && p.z - 0.125 < max[2] && p.z + 0.125 > min[2]
        })
        .map(|d| (d.id, d.stack.clone(), d.pickup_delay > 0))
        .collect();
    let Some(first) = found.first() else { return };
    let front = beside(pos, side);
    let face = Direction::ALL[side].opposite();
    let dimension = game.native_dimension();
    if game.native_insert_slots(dimension, front, face, &first.1).is_none() {
        return;
    }
    let filters: Vec<ItemStack> = (0..FILTER_SLOTS)
        .filter_map(|i| entity.items.get(i).and_then(|s| s.as_ref()).and_then(|s| s.try_into_engine(game.items()).ok()))
        .filter(|s| s.count > 0)
        .collect();
    for (id, stack, delay) in found {
        if stack.count == 0 {
            continue;
        }
        let matched = filters.iter().any(|f| if c.filter.compare_nbt { f.same_item_same_components(&stack) } else { f.item == stack.item });
        if !c.takes(delay, matched) {
            continue;
        }
        let left = game.native_insert_item(front, face, stack.clone());
        let Some(handle) = game.entity(id) else { continue };
        match left {
            None => {
                game.entity_remove(handle, mod_api::RemovalReason::Discarded);
            }
            Some(rest) if rest.count < stack.count => {
                if let Some(item) = game.item_entity(handle) {
                    game.item_entity_set_stack(item, rest);
                }
            }
            Some(_) => {}
        }
    }
}

/// One server tick of the item collector (`ItemCollectorBE.tickServer`), then what its screen shows.
fn tick_item_collector(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let dimension = game.native_dimension();
    let signal = game.native_neighbor_signal(dimension, pos) > 0;
    let side = facing(game, pos);
    let fresh = !entity.nbt.contains_key(&key("xRadiusDouble"));
    let mut c = read_item_collector(&entity.nbt, Direction::ALL[side].normal());
    c.clock = clock_at(dimension, pos, &entity.nbt);
    let before = c.clock;
    if c.tick(signal) {
        collect_items(game, pos, side, &c, entity);
    }
    if fresh {
        write_item_collector(&mut entity.nbt, &c);
    }
    keep_clock(dimension, pos, &mut entity.nbt, &before, &c.clock);
    let data = item_collector_data(&c);
    if entity.data != data {
        entity.data = data;
    }
}

/// The item collector's buttons: 0 to 3 the redstone mode, 9 render area, 10 to 15 an axis's radius up or down
/// (`ValueButtonsDouble`), 16 to 21 its offset (`ValueButtons`), 22 and 23 the tick speed, 24 allowlist or denylist,
/// 25 compare NBT, 26 respect pickup delay, 27 show particles.
fn item_collector_button(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, id: i32) {
    let dimension = game.native_dimension();
    let side = facing(game, pos);
    let mut c = read_item_collector(&entity.nbt, Direction::ALL[side].normal());
    c.clock = clock_at(dimension, pos, &entity.nbt);
    match id {
        0..=3 => c.clock.redstone.mode = Mode::from_index(id),
        9 => c.area.render = !c.area.render,
        10..=15 => c.area.step_radius(((id - 10) / 2) as usize, (id - 10) % 2 == 0),
        16..=21 => c.area.step_offset(((id - 16) / 2) as usize, (id - 16) % 2 == 0),
        22 => c.clock.set_tick_speed(c.clock.tick_speed + 1, MIN_TICK_SPEED),
        23 => c.clock.set_tick_speed(c.clock.tick_speed - 1, MIN_TICK_SPEED),
        24 => c.filter.allowlist = !c.filter.allowlist,
        25 => c.filter.compare_nbt = !c.filter.compare_nbt,
        26 => c.respect_pickup_delay = !c.respect_pickup_delay,
        27 => c.show_particles = !c.show_particles,
        _ => return,
    }
    CLOCKS.with(|m| m.borrow_mut().insert((dimension, pos), c.clock.operation_ticks));
    write_item_collector(&mut entity.nbt, &c);
    entity.data = item_collector_data(&c);
}
