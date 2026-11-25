// Just Dire Things' Block Swapper T1 in the game (support module `crate::game::justdirethings_swapper` of the
// `justdirethings` native bundle). The rules are the bundle's crate `justdirethings_core` (`swapper`); this module is the
// glue: its saved compound (Java's keys: `boundTo`, `swapBlocks`, `swap_entity_type`, the redstone and tick settings), the
// Ferricore Wrench that binds two swappers, the block and the entities in front of each trading places, and the data its
// screen reads. Both swappers must be in the dimension the game is simulating: the engine writes blocks only in the level
// in place. A block with a block entity is left where it is (no helper moves a block entity's contents between
// positions). Translated from Just Dire Things 1.5.7 (MIT); see this bundle's README.md.

use super::{Game, justdirethings_clicker as clicker, justdirethings_machines as machines};
use horizon_client::containers::ModContainer;
use horizon_core::geometry::Direction;
use horizon_core::nbt::{Compound, Tag};
use justdirethings_core::{
    machines::Clock,
    paradox::Area,
    redstone::{Mode, Redstone},
    swapper::{Bound, Kind, SwapEntity, Swapper, translate, translate_f},
    t2::Shared,
    t2x::{SWAPPER_COST, area_volume, in_both},
};
use mod_api::{BlockUpdateFlags, ModBlockEntity, SoundSource};
use std::{cell::RefCell, collections::HashMap};

pub(super) const SWAPPER: &str = "justdirethings:blockswappert1";
pub(super) const SWAPPER_T2: &str = "justdirethings:blockswappert2";
const WRENCH: &str = "justdirethings:ferricore_wrench";
const DENY_TAGS: [&str; 2] = ["c:relocation_not_supported", "justdirethings:swapperdeny"];

/// Whether `block` is this module's machine.
pub(super) fn owns(block: &str) -> bool {
    block == SWAPPER || block == SWAPPER_T2
}

thread_local! {
    /// The swapper a wrench was last used on while unbound (`BOUND_GLOBAL_POS`, which Java keeps on the item).
    static WRENCH_AT: RefCell<Option<Bound>> = const { RefCell::new(None) };
    /// Bindings the wrench made, for each swapper's own tick to write (`setBoundTo`): `None` unbinds.
    static BINDS: RefCell<HashMap<(String, [i32; 3]), Option<Bound>>> = RefCell::new(HashMap::new());
    /// The T2 swappers' areas as each last saw them, by dimension and position: the partner's starting point and box.
    static AREAS: RefCell<HashMap<(String, [i32; 3]), Area>> = RefCell::new(HashMap::new());
    /// The radius a T2 swapper has to take from its partner (`updatePartnerArea`), by dimension and position.
    static RADII: RefCell<HashMap<(String, [i32; 3]), [f64; 3]>> = RefCell::new(HashMap::new());
    /// The T2 swappers that have just been bound and send their radius to their partner on their next tick.
    static SYNC: RefCell<std::collections::HashSet<(String, [i32; 3])>> = RefCell::new(std::collections::HashSet::new());
    static TAKEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(super) fn reset() {
    WRENCH_AT.with(|w| *w.borrow_mut() = None);
    BINDS.with(|b| b.borrow_mut().clear());
    AREAS.with(|a| a.borrow_mut().clear());
    RADII.with(|a| a.borrow_mut().clear());
    SYNC.with(|a| a.borrow_mut().clear());
}

/// Whether the wrench took the use the hook is answering (once).
pub(super) fn take_use() -> bool {
    TAKEN.with(|t| t.replace(false))
}

/// `FerricoreWrench.useOn` on a swapper, not sneaking (`specialBlockHandling`; the swapper passes the click on to the wrench,
/// `world::use_without_item`): the first one clicked is remembered, the second is bound to it (a pair already bound is
/// unbound). The use ends as `Success`. A sneaking click would turn the block (not ported).
pub(super) fn wrench(game: &mut Game<'_>, item: &str, pos: [i32; 3]) {
    TAKEN.with(|t| t.set(false));
    if game.remote.is_some() || item != WRENCH || !owns(&game.native_block(pos)) || game.native_sneaking() {
        return;
    }
    let here = Bound { dimension: game.native_dimension().to_owned(), pos };
    let first = WRENCH_AT.with(|w| w.borrow_mut().take());
    match first {
        None => WRENCH_AT.with(|w| *w.borrow_mut() = Some(here)),
        Some(other) if other == here => {}
        // Java: `isValidPartner` is the same class, so a T1 and a T2 do not bind.
        Some(other) if game.native_block(other.pos) != game.native_block(pos) => {}
        Some(other) => {
            SYNC.with(|s| s.borrow_mut().insert((here.dimension.clone(), here.pos)));
            // The pair: bound unless it already was, then both are unbound.
            let bound = read_bound(game, other.pos).is_some_and(|b| b == here);
            BINDS.with(|b| {
                let mut b = b.borrow_mut();
                b.insert((other.dimension.clone(), other.pos), (!bound).then(|| here.clone()));
                b.insert((here.dimension.clone(), here.pos), (!bound).then_some(other));
            });
        }
    }
    TAKEN.with(|t| t.set(true));
}

/// The `boundTo` a swapper at `pos` has saved (only while this thread has its compound: the swapper that ticked).
fn read_bound(_game: &Game<'_>, pos: [i32; 3]) -> Option<Bound> {
    SAVED.with(|s| s.borrow().get(&pos).cloned())
}

thread_local! {
    /// What each swapper last saved as its `boundTo`, by position (so the wrench can tell a bound pair).
    static SAVED: RefCell<HashMap<[i32; 3], Bound>> = RefCell::new(HashMap::new());
}

fn bound_tag(b: &Bound) -> Tag {
    let mut c = Compound::new();
    c.insert(machines::key("dimension"), Tag::String(b.dimension.as_str().into()));
    c.insert(machines::key("blockpos"), Tag::IntArray(b.pos.to_vec()));
    Tag::Compound(c)
}

fn read_bound_tag(nbt: &Compound) -> Option<Bound> {
    let Some(Tag::Compound(c)) = nbt.get(&machines::key("boundTo")) else { return None };
    let Some(Tag::String(d)) = c.get(&machines::key("dimension")) else { return None };
    match c.get(&machines::key("blockpos")) {
        Some(Tag::IntArray(v)) if v.len() == 3 && v.iter().any(|n| *n != 0) => Some(Bound { dimension: d.to_string_lossy(), pos: [v[0], v[1], v[2]] }),
        _ => None,
    }
}

fn read(nbt: &Compound, dimension: &str, pos: [i32; 3]) -> Swapper {
    let mut s = Swapper::default();
    if nbt.contains_key(&machines::key("redstoneMode")) {
        s.clock = machines::read_clock(nbt);
    } else {
        s.clock = Clock { redstone: Redstone { mode: Mode::Pulse, ..Redstone::default() }, ..Clock::default() };
    }
    let _ = (dimension, pos);
    s.bound = read_bound_tag(nbt);
    s.swap_blocks = !nbt.contains_key(&machines::key("swapBlocks")) || machines::int(nbt, "swapBlocks") != 0;
    s.entities = SwapEntity::from_index(machines::int(nbt, "swap_entity_type"));
    s
}

fn write(nbt: &mut Compound, s: &Swapper) {
    machines::write_clock(nbt, &s.clock);
    match &s.bound {
        Some(b) => {
            nbt.insert(machines::key("boundTo"), bound_tag(b));
        }
        None => {
            nbt.remove(&machines::key("boundTo"));
        }
    }
    machines::put_flag(nbt, "swapBlocks", s.swap_blocks);
    machines::put(nbt, "swap_entity_type", s.entities.index());
}

/// `native_block_entity`: Java's defaults (pulse mode, swapping blocks, no entities, unbound).
pub(super) fn new_entity(nbt: &mut Compound) {
    write(nbt, &Swapper::default());
    machines::put(nbt, "direction", 0);
}

/// `native_block_entity_container`: the hopper menu the screen shows, with no slot of its own.
pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if block == SWAPPER_T2 {
        super::justdirethings_t2x::filter_only(container);
        return;
    }
    *container = Some(ModContainer { slots: 5, faces: Default::default(), inputs: Vec::new(), outputs: Vec::new(), menu: Some("minecraft:hopper".into()) });
}

/// `native_block_entity_slot_accepts` of the T2: the nine filter slots.
pub(super) fn slot_accepts(block: &str, slot: usize, accepts: &mut bool) {
    *accepts = *accepts && block == SWAPPER_T2 && (1..=9).contains(&slot);
}

/// `getAABB` of the block in front and `getStartingPoint`.
fn starting_point(game: &Game<'_>, pos: [i32; 3]) -> [i32; 3] {
    machines::beside(pos, machines::facing(game, pos))
}

/// `isBlockPosValid` (T1): not a relocation-denied block, not unbreakable, not either swapper.
fn block_valid(game: &mut Game<'_>, level: mod_api::LevelRef, at: [i32; 3], this: [i32; 3], partner: [i32; 3]) -> bool {
    if at == this || at == partner {
        return false;
    }
    let Some(state) = game.level_block_state(level, at) else { return false };
    if game.res.facts().get(state).hardness < 0.0 {
        return false;
    }
    let name = game.native_block(at);
    !DENY_TAGS.iter().any(|t| game.native_block_name_in_tag(&name, t).unwrap_or(false))
}

fn has_block_entity(game: &Game<'_>, at: [i32; 3]) -> bool {
    game.world.block_entities.get(&(at[0], at[1], at[2])).is_some()
}

/// `swapBlock`: the two states trade places; two airs, or a block with a block entity, stay.
fn swap_block(game: &mut Game<'_>, level: mod_api::LevelRef, here: [i32; 3], there: [i32; 3]) -> bool {
    let (Some(a), Some(b)) = (game.level_block_state(level, here), game.level_block_state(level, there)) else { return false };
    if (game.res.facts().get(a).air && game.res.facts().get(b).air) || a == b {
        return false;
    }
    if has_block_entity(game, here) || has_block_entity(game, there) {
        return false;
    }
    let flags = BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS;
    let placed_there = game.level_set_block(level, there, a, flags).unwrap_or(false);
    let placed_here = game.level_set_block(level, here, b, flags).unwrap_or(false);
    placed_there || placed_here
}

/// `isValidEntity` kinds in the block at `front`: (the engine's entity id, its `Kind`, its position).
fn entities_at(game: &Game<'_>, level: mod_api::LevelRef, min: [f64; 3], max: [f64; 3], setting: SwapEntity) -> Vec<(mod_api::EntityRef, [f64; 3], String)> {
    let mut found = Vec::new();
    for m in game.mobs.iter().filter(|m| m.alive()) {
        let b = m.bounds();
        let (lo, hi) = (b.min_position(), b.max_position());
        if !(lo.x < max[0] && hi.x > min[0] && lo.y < max[1] && hi.y > min[1] && lo.z < max[2] && hi.z > min[2]) {
            continue;
        }
        let kind = Kind { monster: m.kind.category == "monster", animal: m.kind.category == "creature", baby: m.is_baby(), unmovable: m.kind.id == "minecraft:ender_dragon", ..Kind::default() };
        if setting.accepts(kind) {
            if let Some(e) = game.entity(m.id) {
                found.push((e, [m.position.x, m.position.y, m.position.z], m.kind.id.to_string()));
            }
        }
    }
    if matches!(setting, SwapEntity::Player | SwapEntity::All | SwapEntity::Living)
        && let Some(entities) = game.level_entities_in(level, min, max)
    {
        for e in entities {
            if game.entity_kind(e).is_some_and(|k| game.entity_type_key_id(k) == "minecraft:player")
                && setting.accepts(Kind { player: true, ..Kind::default() })
                && let Some(p) = game.entity_position(e)
            {
                found.push((e, p, "minecraft:player".to_owned()));
            }
        }
    }
    if matches!(setting, SwapEntity::Item | SwapEntity::All) {
        for d in game.dropped.iter() {
            let p = d.position;
            if p.x - 0.125 < max[0] && p.x + 0.125 > min[0] && p.y < max[1] && p.y + 0.25 > min[1] && p.z - 0.125 < max[2] && p.z + 0.125 > min[2] {
                if let Some(e) = game.entity(d.id) {
                    found.push((e, [p.x, p.y, p.z], "minecraft:item".to_owned()));
                }
            }
        }
    }
    found
}

/// One server tick of the swapper (`BlockSwapperT1BE.tickServer`, `doSwap`), then what its screen shows.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    if entity.id() == SWAPPER_T2 {
        tick_t2(game, pos, entity);
        return;
    }
    let Some(level) = game.fork_level_here() else { return };
    let dimension = game.native_dimension();
    let signal = game.native_neighbor_signal(dimension, pos) > 0;
    let mut s = read(&entity.nbt, dimension, pos);
    let before = machines::clock_at(dimension, pos, &entity.nbt);
    s.clock = before;
    if let Some(change) = BINDS.with(|b| b.borrow_mut().remove(&(dimension.to_owned(), pos))) {
        s.bound = change;
        write(&mut entity.nbt, &s);
    }
    match &s.bound {
        Some(b) => SAVED.with(|m| m.borrow_mut().insert(pos, b.clone())),
        None => SAVED.with(|m| m.borrow_mut().remove(&pos)),
    };
    // The partner exists while a swapper stands where this one is bound, in this dimension.
    let partner = s.bound.clone().filter(|b| b.dimension == dimension && game.native_block(b.pos) == SWAPPER);
    let run = s.tick(signal, partner.is_some());
    if run && let Some(partner) = &partner {
        swap(game, level, pos, partner.pos, &s);
    }
    machines::keep_clock(dimension, pos, &mut entity.nbt, &before, &s.clock);
    let data = vec![s.clock.redstone.mode.index(), i32::from(s.swap_blocks), s.entities.index(), i32::from(partner.is_some()), i32::from(s.bound.is_some())];
    if entity.data != data {
        entity.data = data;
    }
}

/// `doSwap` between the swapper at `pos` and its partner.
fn swap(game: &mut Game<'_>, level: mod_api::LevelRef, pos: [i32; 3], partner: [i32; 3], s: &Swapper) {
    let here = starting_point(game, pos);
    let there = starting_point(game, partner);
    let mut swapped = 0;
    if s.swap_blocks && block_valid(game, level, here, pos, partner) && block_valid(game, level, there, pos, partner) && swap_block(game, level, here, there) {
        swapped += 2;
    }
    if s.entities != SwapEntity::None {
        // The entities of this side go over, and the partner's come back.
        let moves: Vec<_> = [(here, there), (there, here)]
            .into_iter()
            .flat_map(|(from, to)| {
                entities_at(game, level, from.map(f64::from), from.map(|v| f64::from(v) + 1.0), s.entities).into_iter().map(move |(handle, at, _)| (handle, translate_f(at, from, to)))
            })
            .collect();
        for (handle, target) in moves {
            if game.entity_teleport_to(handle, target, None) {
                swapped += 1;
            }
        }
    }
    if swapped > 0 {
        game.level_play_sound(level, "minecraft:entity.shulker.teleport", machines::center(pos), SoundSource::Blocks, 0.33, 1.0);
        game.level_play_sound(level, "minecraft:entity.shulker.teleport", machines::center(partner), SoundSource::Blocks, 0.33, 1.0);
    }
}

/// The screen's buttons (`native_menu_button`): 0 to 3 the redstone mode, 4 swap blocks on or off, 5 to 13 the entity
/// setting.
pub(super) fn button(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, id: i32) {
    if entity.id() == SWAPPER_T2 {
        button_t2(game, pos, entity, id);
        return;
    }
    let dimension = game.native_dimension();
    let mut s = read(&entity.nbt, dimension, pos);
    let before = machines::clock_at(dimension, pos, &entity.nbt);
    s.clock = before;
    match id {
        0..=3 => s.clock.redstone.mode = Mode::from_index(id),
        4 => s.swap_blocks = !s.swap_blocks,
        5..=13 => s.entities = SwapEntity::from_index(id - 5),
        _ => return,
    }
    machines::keep_clock(dimension, pos, &mut entity.nbt, &before, &s.clock);
    machines::put_flag(&mut entity.nbt, "swapBlocks", s.swap_blocks);
    machines::put(&mut entity.nbt, "swap_entity_type", s.entities.index());
}

// ---- The Block Swapper T2 ---------------------------------------------------------------------------------------

/// `BlockSwapperT2BE.getStartingPoint`: the machine's position moved by its area's offset.
fn start_t2(pos: [i32; 3], area: &Area) -> [i32; 3] {
    [pos[0] + area.offset[0], pos[1] + area.offset[1], pos[2] + area.offset[2]]
}

/// The area of the swapper at `pos` in `dimension` as it last saved it (the machine's own when it hasn't ticked yet).
fn area_of(dimension: &str, pos: [i32; 3]) -> Area {
    AREAS.with(|a| a.borrow().get(&(dimension.to_owned(), pos)).copied()).unwrap_or(Area { radius: [0.0; 3], offset: [0; 3], render: false })
}

/// One server tick of the T2 swapper (`BlockSwapperT2BE`): the swap of `BlockSwapperT1BE.doSwap` over its area (the places
/// nearest first that both areas don't share and the filter lets through, the entities of its area), if the buffer has the
/// area's volume at 50 FE, then 50 FE for each block (on both sides) and entity moved.
fn tick_t2(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let Some(level) = game.fork_level_here() else { return };
    let dimension = game.native_dimension();
    let signal = game.native_neighbor_signal(dimension, pos) > 0;
    let facing = Direction::ALL[machines::facing(game, pos)].normal();
    let mut fresh = !entity.nbt.contains_key(&machines::key("xRadiusDouble"));
    let mut shared = clicker::read_shared(&entity.nbt, facing);
    let energy = shared.powered.energy;
    let key = (dimension.to_owned(), pos);
    // The radius the partner sent (`updatePartnerArea`).
    if let Some(radius) = RADII.with(|r| r.borrow_mut().remove(&key)) {
        shared.area.radius = radius;
        fresh = true;
    }
    let mut s = read(&entity.nbt, dimension, pos);
    let before = machines::clock_at(dimension, pos, &entity.nbt);
    s.clock = before;
    if let Some(change) = BINDS.with(|b| b.borrow_mut().remove(&(dimension.to_owned(), pos))) {
        s.bound = change;
        write(&mut entity.nbt, &s);
    }
    match &s.bound {
        Some(b) => SAVED.with(|m| m.borrow_mut().insert(pos, b.clone())),
        None => SAVED.with(|m| m.borrow_mut().remove(&pos)),
    };
    AREAS.with(|a| a.borrow_mut().insert(key.clone(), shared.area));
    // `addPartnerConnection`: the swapper the wrench bound last gives its radius to its partner.
    if SYNC.with(|y| y.borrow_mut().remove(&key))
        && let Some(b) = &s.bound
    {
        RADII.with(|r| r.borrow_mut().insert((b.dimension.clone(), b.pos), shared.area.radius));
    }
    let partner = s.bound.clone().filter(|b| b.dimension == dimension && game.native_block(b.pos) == SWAPPER_T2);
    let run = s.tick(signal, partner.is_some());
    if run && let Some(partner) = &partner {
        let (min, max) = shared.area.bounds(pos);
        if shared.powered.has(area_volume(min, max) * SWAPPER_COST) {
            let filters = clicker::filter_items(game, entity);
            let moved = swap_t2(game, level, pos, partner.pos, &shared, &filters, &s);
            shared.powered.take(moved * SWAPPER_COST);
        }
    }
    machines::keep_clock(dimension, pos, &mut entity.nbt, &before, &s.clock);
    if fresh {
        clicker::write_shared(&mut entity.nbt, &shared);
    } else if shared.powered.energy != energy {
        machines::put(&mut entity.nbt, "energy", shared.powered.energy);
    }
    let mut data = vec![s.clock.redstone.mode.index(), i32::from(s.swap_blocks), s.entities.index(), i32::from(partner.is_some()), i32::from(s.bound.is_some())];
    super::justdirethings_t2x::parts(&mut data, &shared);
    if entity.data != data {
        entity.data = data;
    }
}

/// `doSwap` of the T2 between the swapper at `pos` and its partner: how many blocks (each counts for both sides) and
/// entities moved.
fn swap_t2(game: &mut Game<'_>, level: mod_api::LevelRef, pos: [i32; 3], partner: [i32; 3], shared: &Shared, filters: &[String], s: &Swapper) -> i32 {
    let dimension = game.native_dimension();
    let here0 = start_t2(pos, &shared.area);
    let there_area = area_of(dimension, partner);
    let there0 = start_t2(partner, &there_area);
    let mine = shared.area.bounds(pos);
    let theirs = there_area.bounds(partner);
    let both = |p: [f64; 3]| in_both(mine, theirs, p);
    let filter = shared.filter;
    let valid = |game: &mut Game<'_>, at: [i32; 3]| {
        if !block_valid(game, level, at, pos, partner) || both(at.map(f64::from)) {
            return false;
        }
        let air = game.level_block_state(level, at).is_some_and(|st| game.res.facts().get(st).air);
        if air {
            return true;
        }
        let name = game.native_block(at);
        clicker::block_passes(game, &filter, filters, &name)
    };
    let mut moved = 0;
    if s.swap_blocks {
        for at in shared.nearest_first(pos) {
            let remote = translate(at, here0, there0);
            if valid(game, at) && valid(game, remote) && swap_block(game, level, at, remote) {
                moved += 2;
            }
        }
    }
    if s.entities != SwapEntity::None {
        let (min, max) = mine;
        let egg_ok = |kind: &str| filter.accepts(filters.iter().any(|f| *f == format!("{kind}_spawn_egg")));
        let found: Vec<_> = entities_at(game, level, min, max, s.entities).into_iter().filter(|(_, at, kind)| !both(*at) && egg_ok(kind)).collect();
        for (handle, at, _) in found {
            if game.entity_teleport_to(handle, translate_f(at, here0, there0), None) {
                moved += 1;
            }
        }
    }
    if moved > 0 {
        game.level_play_sound(level, "minecraft:entity.shulker.teleport", machines::center(pos), SoundSource::Blocks, 0.33, 1.0);
        game.level_play_sound(level, "minecraft:entity.shulker.teleport", machines::center(partner), SoundSource::Blocks, 0.33, 1.0);
    }
    moved
}

/// The T2 screen's buttons (`native_menu_button`): 0 to 3 the redstone mode, 4 swap blocks on or off, 70 to 78 the entity
/// setting, and the shared area and filter buttons; a changed radius goes to the partner.
fn button_t2(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, id: i32) {
    let dimension = game.native_dimension();
    let mut s = read(&entity.nbt, dimension, pos);
    let before = machines::clock_at(dimension, pos, &entity.nbt);
    s.clock = before;
    match id {
        0..=3 => s.clock.redstone.mode = Mode::from_index(id),
        4 => s.swap_blocks = !s.swap_blocks,
        70..=78 => s.entities = SwapEntity::from_index(id - 70),
        _ => {
            let facing = Direction::ALL[machines::facing(game, pos)].normal();
            let mut shared = clicker::read_shared(&entity.nbt, facing);
            let radius = shared.area.radius;
            if shared.button(id) {
                clicker::write_shared(&mut entity.nbt, &shared);
                if shared.area.radius != radius
                    && let Some(b) = read_bound_tag(&entity.nbt)
                {
                    RADII.with(|r| r.borrow_mut().insert((b.dimension, b.pos), shared.area.radius));
                }
            }
            return;
        }
    }
    machines::keep_clock(dimension, pos, &mut entity.nbt, &before, &s.clock);
    machines::put_flag(&mut entity.nbt, "swapBlocks", s.swap_blocks);
    machines::put(&mut entity.nbt, "swap_entity_type", s.entities.index());
}
