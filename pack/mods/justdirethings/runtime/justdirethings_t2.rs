// Just Dire Things' Block Placer T2 and Block Breaker T2 in the game (support module `crate::game::justdirethings_t2` of the `justdirethings`
// native bundle), on what every T2 machine shares (`justdirethings_clicker`: the Forge Energy buffer, the area and the
// filter slots). The rules are the bundle's crate `justdirethings_core` (`t2`); this module is the glue: its saved
// compound (Java's keys), its slot and filter slots (a 9x2 chest menu: slot 0 the machine's, 1 to 9 the filter's), the
// places of its area nearest first, each placed as the Placer T1 places the one in front (`justdirethings_machines::
// PlaceSpot`), 500 FE a placement; the breaker's tracker of its area's blocks, mined one at a time at the tool's speed as
// the Block Breaker T1 mines the one in front, 500 FE a break. Translated from Just Dire Things 1.5.7 (MIT); see this bundle's README.md.

use super::{
    Game,
    justdirethings_clicker::{self as clicker, CLICKER_T2},
    justdirethings_machines::{self as machines, PlaceSpot},
};
use horizon_client::containers::ModContainer;
use horizon_core::geometry::Direction;
use justdirethings_core::{
    breaker,
    machines::Spot,
    redstone::Mode,
    t2::{BREAKER_COST, Blocks, BreakerT2, PLACER_COST, Queue, Shared},
};
use mod_api::ModBlockEntity;
use std::{cell::RefCell, collections::HashMap};

pub(super) const PLACER_T2: &str = "justdirethings:blockplacert2";
pub(super) const BREAKER_T2: &str = "justdirethings:blockbreakert2";
const SLOTS: usize = 18;

/// Whether `block` is one of this module's machines.
pub(super) fn owns(block: &str) -> bool {
    block == PLACER_T2 || block == BREAKER_T2
}

thread_local! {
    /// What each Placer T2 found and has not placed in yet (Java's transient `positionsToPlace`), by dimension and
    /// position.
    static QUEUES: RefCell<HashMap<(&'static str, [i32; 3]), Vec<[i32; 3]>>> = RefCell::new(HashMap::new());
    /// Each Breaker T2's tracker (`blockBreakingTracker`, `currentBlock`), by dimension and position.
    static TRACKERS: RefCell<HashMap<(&'static str, [i32; 3]), (Vec<([i32; 3], breaker::Tracked)>, bool)>> = RefCell::new(HashMap::new());
}

pub(super) fn reset() {
    QUEUES.with(|q| q.borrow_mut().clear());
    TRACKERS.with(|t| t.borrow_mut().clear());
}

/// `native_block_entity`: the clock's defaults, direction down; the area and filter are written on its first tick.
pub(super) fn new_entity(nbt: &mut horizon_core::nbt::Compound) {
    machines::new_entity(nbt);
}

/// `native_block_entity_container`: the T2 menu (a 9x2 chest's: the slot, then the nine filter slots).
pub(super) fn container(container: &mut Option<ModContainer>) {
    clicker::container(CLICKER_T2, container);
}

pub(super) fn slot_accepts(slot: usize, accepts: &mut bool) {
    clicker::slot_accepts(CLICKER_T2, slot, accepts);
}

/// One server tick of the Placer T2 (`BlockPlacerT2BE.tickServer`), then what its screen shows.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    if entity.id() == BREAKER_T2 {
        tick_breaker(game, pos, entity);
        return;
    }
    let Some(level) = game.fork_level_here() else { return };
    let dimension = game.native_dimension();
    let signal = game.native_neighbor_signal(dimension, pos) > 0;
    let facing = Direction::ALL[machines::facing(game, pos)].normal();
    let fresh = !entity.nbt.contains_key(&machines::key("xRadiusDouble"));
    let mut shared = clicker::read_shared(&entity.nbt, facing);
    let energy = shared.powered.energy;
    let stack = machines::slot_stack(game, entity);
    let block_item = stack.as_ref().is_some_and(|s| machines::is_block_item(game, &s.item));
    let before = machines::clock_at(dimension, pos, &entity.nbt);
    let spots = QUEUES.with(|q| q.borrow_mut().remove(&(dimension, pos))).unwrap_or_default();
    let mut q = Queue { clock: before, spots };
    let direction = machines::int(&entity.nbt, "direction").clamp(0, 5) as usize;
    let against = Direction::ALL[direction].normal();
    let filters = clicker::filter_items(game, entity);
    let view: &Game<'_> = game;
    let find = || {
        // `isBlockPosValid`: a block that can be replaced, and the filter on the block it is placed against.
        shared
            .nearest_first(pos)
            .into_iter()
            .filter(|at| {
                let replaceable = view.level_block_state(level, *at).is_some_and(|s| view.res.facts().get(s).replaceable);
                let next = [at[0] + against[0], at[1] + against[1], at[2] + against[2]];
                let name = view.level_block_state(level, next).map(|s| view.res.facts().get(s).name.to_string()).unwrap_or_default();
                replaceable && clicker::block_passes(view, &shared.filter, &filters, &name)
            })
            .collect()
    };
    let spot = q.tick(block_item, shared.powered.has(PLACER_COST), signal, find);
    if let Some(at) = spot {
        let mut place = PlaceSpot { game: &mut *game, level, pos: at, direction: Direction::ALL[direction], stack, used: None };
        place.place();
        if let Some(left) = place.used.take() {
            set_slot(entity, left);
            // A placement answers CONSUME: `extractEnergy(getStandardEnergyCost())`.
            shared.powered.take(PLACER_COST);
        }
    }
    QUEUES.with(|m| m.borrow_mut().insert((dimension, pos), q.spots));
    machines::keep_clock(dimension, pos, &mut entity.nbt, &before, &q.clock);
    if fresh {
        clicker::write_shared(&mut entity.nbt, &shared);
    } else if shared.powered.energy != energy {
        machines::put(&mut entity.nbt, "energy", shared.powered.energy);
    }
    let has_item = entity.items.first().is_some_and(|s| s.is_some());
    let data = screen_data(&q.clock, direction, has_item, &shared);
    if entity.data != data {
        entity.data = data;
    }
}

fn set_slot(entity: &mut ModBlockEntity, stack: Option<horizon_client::items::ItemStack>) {
    use crate::game::api_values::TryIntoApi;
    if entity.items.len() < SLOTS {
        entity.items.resize(SLOTS, None);
    }
    entity.items[0] = stack.filter(|s| s.count > 0).as_ref().and_then(|s| s.try_into_api().ok());
}

/// What the Placer T2's screen reads: 0 redstone, 1 whether the slot holds an item, 2 direction, 3 tick speed, 4 energy,
/// 5 to 7 the radius in half blocks, 8 to 10 the offsets, 11 allowlist, 12 compare components, 13 render area.
fn screen_data(clock: &justdirethings_core::machines::Clock, direction: usize, has_item: bool, s: &Shared) -> Vec<i32> {
    let mut data = vec![clock.redstone.mode.index(), i32::from(has_item), direction as i32, clock.tick_speed, s.powered.energy];
    data.extend(s.area.radius.map(|r| (r * 2.0).round() as i32));
    data.extend(s.area.offset);
    data.extend([i32::from(s.filter.allowlist), i32::from(s.filter.compare_nbt), i32::from(s.area.render)]);
    data
}

// ---- The Block Breaker T2 ----------------------------------------------------------------------------------------

/// The Breaker T2's area in the level in place, its tool and what it broke.
struct AreaBlocks<'a, 'g> {
    game: &'a mut Game<'g>,
    level: mod_api::LevelRef,
    tool: Option<&'a horizon_client::items::ItemStack>,
    /// The breaker ids of its cracks: its own base, one more for each entry of the tracker.
    breaker: i32,
    energy: &'a mut i32,
    /// The hardness of each block it broke.
    broke: Vec<f32>,
}

impl Blocks<[i32; 3]> for AreaBlocks<'_, '_> {
    fn state(&mut self, at: &[i32; 3]) -> Option<u32> {
        let state = self.game.level_block_state(self.level, *at)?;
        (!self.game.res.facts().get(state).air).then_some(state.0 as u32)
    }

    fn progress(&mut self, at: &[i32; 3]) -> f32 {
        let Some(state) = self.game.level_block_state(self.level, *at) else { return 0.0 };
        let facts = self.game.res.facts().get(state);
        let (name, hardness) = (facts.name.to_string(), facts.hardness);
        let items = self.game.items();
        let speed = items.mining_speed(self.tool, &name);
        let correct = items.correct_for_drops(self.tool, &name);
        let efficiency = self.tool.map_or(0, |t| t.enchantment_level("minecraft:efficiency"));
        breaker::destroy_progress(speed, efficiency, hardness, correct)
    }

    fn crack(&mut self, at: &[i32; 3], index: usize, stage: i32) {
        self.game.level_set_break_progress(self.level, self.breaker - index as i32, *at, stage);
    }

    /// `tryBreakBlock`: 500 FE first, then the block broken by the tool with its drops.
    fn break_block(&mut self, at: &[i32; 3]) -> bool {
        if *self.energy < BREAKER_COST {
            return false;
        }
        *self.energy -= BREAKER_COST;
        let hardness = self.game.native_destroy_speed(*at).unwrap_or(0.0);
        if self.game.native_machine_destroy_block(*at, self.tool, false).is_some() {
            self.broke.push(hardness);
        }
        true
    }
}

/// One server tick of the Breaker T2 (`BlockBreakerT2BE.tickServer`), then its tool's wear (a point for each block of
/// any hardness but 0) and what its screen shows.
fn tick_breaker(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    use crate::game::api_values::TryIntoEngine;
    let Some(level) = game.fork_level_here() else { return };
    let dimension = game.native_dimension();
    let signal = game.native_neighbor_signal(dimension, pos) > 0;
    let facing = Direction::ALL[machines::facing(game, pos)].normal();
    let fresh = !entity.nbt.contains_key(&machines::key("xRadiusDouble"));
    let mut shared = clicker::read_shared(&entity.nbt, facing);
    let energy = shared.powered.energy;
    let tool: Option<horizon_client::items::ItemStack> = entity.items.first().and_then(|s| s.as_ref()).and_then(|s| s.try_into_engine(game.items()).ok()).filter(|s: &horizon_client::items::ItemStack| s.count > 0);
    let before = machines::clock_at(dimension, pos, &entity.nbt);
    let (tracked, current) = TRACKERS.with(|t| t.borrow_mut().remove(&(dimension, pos))).unwrap_or_default();
    let mut b = BreakerT2 { clock: before, sneaking: machines::int(&entity.nbt, "sneaking") != 0, tracked, current };
    let filters = clicker::filter_items(game, entity);
    // `isBlockValid`: not air, not a liquid, not the breaker, breakable, and what the filter lets through (the block's
    // item: `blockItemFilter` 0).
    let found: Vec<[i32; 3]> = {
        let view: &Game<'_> = game;
        shared
            .nearest_first(pos)
            .into_iter()
            .filter(|at| {
                *at != pos
                    && view.level_block_state(level, *at).is_some_and(|s| {
                        let facts = view.res.facts().get(s);
                        !facts.air && !facts.classes.iter().any(|c| c.ends_with("LiquidBlock")) && facts.hardness >= 0.0 && clicker::block_passes(view, &shared.filter, &filters, &facts.name)
                    })
            })
            .collect()
    };
    let powered = shared.powered.has(BREAKER_COST);
    let breaker_id = -2_000_000 - (pos[0].wrapping_add(pos[1]).wrapping_add(pos[2])).rem_euclid(1 << 20) * 64;
    let mut stored = shared.powered.energy;
    let mut blocks = AreaBlocks { game: &mut *game, level, tool: tool.as_ref(), breaker: breaker_id, energy: &mut stored, broke: Vec::new() };
    b.tick(tool.is_some(), powered, signal, || found, &mut blocks);
    let broke = std::mem::take(&mut blocks.broke);
    shared.powered.energy = stored;
    TRACKERS.with(|t| t.borrow_mut().insert((dimension, pos), (b.tracked, b.current)));
    if let Some(t) = tool.as_ref()
        && broke.iter().any(|h| *h != 0.0)
    {
        let (left, _) = game.damage_living_stack(t, broke.iter().filter(|h| **h != 0.0).count() as u32, 0);
        set_slot(entity, left);
    }
    machines::keep_clock(dimension, pos, &mut entity.nbt, &before, &b.clock);
    if fresh {
        clicker::write_shared(&mut entity.nbt, &shared);
        // The breaker's filter compares blocks (`FilterData(false, false, 0)`).
        machines::put(&mut entity.nbt, "blockitemfilter", 0);
    } else if shared.powered.energy != energy {
        machines::put(&mut entity.nbt, "energy", shared.powered.energy);
    }
    let has_tool = entity.items.first().is_some_and(|s| s.is_some());
    let mut data = vec![b.clock.redstone.mode.index(), i32::from(has_tool), b.clock.tick_speed, shared.powered.energy];
    data.extend(shared.area.radius.map(|r| (r * 2.0).round() as i32));
    data.extend(shared.area.offset);
    data.extend([i32::from(shared.filter.allowlist), i32::from(shared.filter.compare_nbt), i32::from(shared.area.render)]);
    if entity.data != data {
        entity.data = data;
    }
}

/// The screen's buttons: 0 to 3 the redstone mode, 22 and 23 the tick speed, 34 to 39 the direction, and the area's and
/// filter's (`t2::Shared::button`).
pub(super) fn button(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, id: i32) {
    let dimension = game.native_dimension();
    let before = machines::clock_at(dimension, pos, &entity.nbt);
    let mut clock = before;
    match id {
        0..=3 => clock.redstone.mode = Mode::from_index(id),
        22 => clock.set_tick_speed(clock.tick_speed + 1, machines::MIN_TICK_SPEED),
        23 => clock.set_tick_speed(clock.tick_speed - 1, machines::MIN_TICK_SPEED),
        34..=39 if entity.id() == PLACER_T2 => machines::put(&mut entity.nbt, "direction", id - 34),
        _ => {
            let facing = Direction::ALL[machines::facing(game, pos)].normal();
            let mut shared = clicker::read_shared(&entity.nbt, facing);
            if shared.button(id) {
                clicker::write_shared(&mut entity.nbt, &shared);
                if entity.id() == BREAKER_T2 {
                    machines::put(&mut entity.nbt, "blockitemfilter", 0);
                }
                QUEUES.with(|q| q.borrow_mut().remove(&(dimension, pos)));
            }
            return;
        }
    }
    machines::keep_clock(dimension, pos, &mut entity.nbt, &before, &clock);
}

