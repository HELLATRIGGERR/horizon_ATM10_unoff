// Just Dire Things' Clicker T1 and T2 in the game (support module `crate::game::justdirethings_clicker` of the
// `justdirethings` native bundle), and what every T2 machine shares (the Forge Energy buffer, the area and the filter
// slots, `PoweredMachineBE`, `AreaAffectingBE`, `FilterableBE`). The rules are the bundle's crate `justdirethings_core`
// (`clicker`, `t2`); this module is the glue: the saved compounds (Java's keys), the slot (the hopper menu's first for
// the T1; slot 0 of a 9x2 chest menu for the T2, its nine filter slots 1 to 9), the places and creatures in front or in
// the area, and the click itself, which the game's fake player makes (`native_fake_use`: a right click, a left click
// or a held use, through the same steps as a player's, on the server). Translated from Just Dire Things 1.5.7 (MIT);
// see this bundle's README.md.

use super::{
    Game,
    justdirethings_machines as machines,
    native_fake_use::{FakeClick, FakeTarget, FakeUse, FakeUsed},
};
use crate::game::api_values::{TryIntoApi, TryIntoEngine};
use horizon_client::{containers::ModContainer, items::ItemStack};
use horizon_core::geometry::Direction;
use horizon_core::nbt::{Compound, Tag};
use justdirethings_core::{
    clicker::{self, Click, Clicker, Creature, HOLD, LEFT, NORMALS, Settings, T2_CLICK_COST, Target},
    machines::{FILTER_SLOTS, Filter},
    paradox::Area,
    redstone::Mode,
    t2::{self, Powered, Shared},
};
use mod_api::{ActionOutcome, ModBlockEntity, NativeFeInfo};
use std::{cell::RefCell, collections::HashMap};

pub(super) const CLICKER_T1: &str = "justdirethings:clickert1";
pub(super) const CLICKER_T2: &str = "justdirethings:clickert2";
/// `BaseMachineBE.defaultFakePlayerUUID`; its name `[JustDiresFakePlayer]` is longer than a player's may be.
const FAKE_UUID: u128 = 0x4191a6f5_37fe_45d9_8ba3_4549be778e54;
const FAKE_NAME: &str = "[JustDiresFake]";
/// `JustDireBlockTags.NO_AUTO_CLICK`.
const NO_AUTO_CLICK: &str = "justdirethings:no_auto_click";
/// The T2's menu: a 9x2 chest's 18 slots, the machine's first, then the filter's nine.
const T2_SLOTS: usize = 18;

/// Whether `block` is a Clicker.
pub(super) fn owns(block: &str) -> bool {
    block == CLICKER_T1 || block == CLICKER_T2
}

thread_local! {
    /// What each clicker found and has not clicked yet, and whether its fake still holds a use (Java's transient
    /// `positionsToClick`, `entitiesToClick` and `isUsingItem`), by dimension and position.
    static QUEUES: RefCell<HashMap<(&'static str, [i32; 3]), (Vec<[i32; 3]>, Vec<u32>, bool)>> = RefCell::new(HashMap::new());
}

pub(super) fn reset() {
    QUEUES.with(|q| q.borrow_mut().clear());
}

fn flag(nbt: &Compound, name: &str) -> bool {
    machines::int(nbt, name) != 0
}

fn double(nbt: &Compound, name: &str) -> f64 {
    match nbt.get(&machines::key(name)) {
        Some(Tag::Double(v)) => f64::from_bits(*v),
        Some(Tag::Float(v)) => f64::from(f32::from_bits(*v)),
        Some(Tag::Int(v)) => f64::from(*v),
        _ => 0.0,
    }
}

/// `clickType`, `clickTarget`, `sneaking`, `showFakePlayer`, `maxHoldTicks`.
fn read_settings(nbt: &Compound) -> Settings {
    Settings {
        click_type: machines::int(nbt, "clickType").clamp(0, 2),
        target: Target::from_index(machines::int(nbt, "clickTarget")),
        sneaking: flag(nbt, "sneaking"),
        show_fake_player: flag(nbt, "showFakePlayer"),
        max_hold_ticks: if nbt.contains_key(&machines::key("maxHoldTicks")) { machines::int(nbt, "maxHoldTicks").max(1) } else { 1 },
    }
}

fn write_settings(nbt: &mut Compound, s: &Settings) {
    machines::put(nbt, "clickType", s.click_type);
    machines::put(nbt, "clickTarget", s.target.index());
    machines::put_flag(nbt, "sneaking", s.sneaking);
    machines::put_flag(nbt, "showFakePlayer", s.show_fake_player);
    machines::put(nbt, "maxHoldTicks", s.max_hold_ticks);
}

/// A T2's shared settings: `energy`, `saveAreaSettings` and `saveFilterSettings`; Java's defaults while a new one has
/// none.
pub(super) fn read_shared(nbt: &Compound, facing: [i32; 3]) -> Shared {
    let mut s = Shared::new(facing);
    s.powered.energy = machines::int(nbt, "energy").clamp(0, t2::CAPACITY);
    if nbt.contains_key(&machines::key("xRadiusDouble")) {
        s.area = Area {
            radius: [double(nbt, "xRadiusDouble"), double(nbt, "yRadiusDouble"), double(nbt, "zRadiusDouble")],
            offset: [machines::int(nbt, "xOffset"), machines::int(nbt, "yOffset"), machines::int(nbt, "zOffset")],
            render: flag(nbt, "renderArea"),
        };
        s.filter = Filter { allowlist: flag(nbt, "allowlist"), compare_nbt: flag(nbt, "compareNBT") };
    }
    s
}

pub(super) fn write_shared(nbt: &mut Compound, s: &Shared) {
    machines::put(nbt, "energy", s.powered.energy);
    for (i, axis) in ["x", "y", "z"].into_iter().enumerate() {
        nbt.insert(machines::key(&format!("{axis}RadiusDouble")), Tag::Double(s.area.radius[i].to_bits()));
        machines::put(nbt, &format!("{axis}Offset"), s.area.offset[i]);
    }
    machines::put_flag(nbt, "renderArea", s.area.render);
    machines::put_flag(nbt, "allowlist", s.filter.allowlist);
    machines::put_flag(nbt, "compareNBT", s.filter.compare_nbt);
    machines::put(nbt, "blockitemfilter", -1);
}

/// `native_block_entity`: Java's defaults (redstone ignored, tick speed 20, direction down, a right click on the block
/// in front, held one tick).
pub(super) fn new_entity(nbt: &mut Compound) {
    machines::new_entity(nbt);
    write_settings(nbt, &Settings::default());
}

/// `native_block_entity_container`: the T1's slot, the hopper menu's first, which hoppers fill from every side; the
/// T2's slot and nine filter slots in a 9x2 chest menu (the filter is no capability: no side reaches it).
pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    let slot = || vec![0];
    let (slots, menu) = if block == CLICKER_T2 { (T2_SLOTS, "minecraft:generic_9x2") } else { (machines::HOPPER_SLOTS, "minecraft:hopper") };
    *container = Some(ModContainer {
        slots,
        faces: [slot(), slot(), slot(), slot(), slot(), slot()],
        inputs: vec![0],
        outputs: Vec::new(),
        menu: Some(menu.into()),
    });
}

/// `native_block_entity_slot_accepts`: any item in the machine's slot and, for the T2, its filter slots.
pub(super) fn slot_accepts(block: &str, slot: usize, accepts: &mut bool) {
    let last = if block == CLICKER_T2 { FILTER_SLOTS } else { 0 };
    *accepts = *accepts && slot <= last;
}

fn set_slot(entity: &mut ModBlockEntity, slots: usize, stack: Option<ItemStack>) {
    if entity.items.len() < slots {
        entity.items.resize(slots, None);
    }
    entity.items[0] = stack.filter(|s| s.count > 0).as_ref().and_then(|s| s.try_into_api().ok());
}

/// The items in a T2's filter slots (1 to 9), by name.
pub(super) fn filter_items(game: &Game<'_>, entity: &ModBlockEntity) -> Vec<String> {
    entity
        .items
        .iter()
        .skip(1)
        .take(FILTER_SLOTS)
        .flatten()
        .filter_map(|s| s.try_into_engine(game.items()).ok())
        .map(|s: ItemStack| s.item.to_string())
        .collect()
}

/// `isStackValidFilter` of the item a block gives (`getCloneItemStack`: the item of its name; a liquid's or a block
/// without one is the empty stack, which no filter item matches).
pub(super) fn block_passes(game: &Game<'_>, filter: &Filter, filters: &[String], name: &str) -> bool {
    let item = game.items().get(name).map(|_| name);
    filter.accepts(item.is_some_and(|i| filters.iter().any(|f| f == i)))
}

/// `isEntityValidFilter`: a spawn egg of the creature's type in the filter.
fn creature_passes(filter: &Filter, filters: &[String], kind: &str) -> bool {
    let egg = format!("{kind}_spawn_egg");
    filter.accepts(filters.iter().any(|f| *f == egg))
}

/// What the clicker looks at: the machine's own block, its facing, and the T2's area and filter.
struct Around<'a> {
    pos: [i32; 3],
    facing: [i32; 3],
    t2: Option<&'a Shared>,
    filters: &'a [String],
    target: Target,
}

/// `findSpotsToClick`: the block in front (T1), or the area's blocks nearest first (T2), each `isBlockPosValid`: not
/// air for a block target, air for the air one, not tagged `no_auto_click`, and, for the T2, what the filter lets
/// through. Java's `mayInteract` and the place event are not asked.
fn find_places(game: &Game<'_>, level: mod_api::LevelRef, a: &Around<'_>) -> Vec<[i32; 3]> {
    let spots = match a.t2 {
        Some(s) => s.nearest_first(a.pos),
        None => vec![[a.pos[0] + a.facing[0], a.pos[1] + a.facing[1], a.pos[2] + a.facing[2]]],
    };
    spots
        .into_iter()
        .filter(|at| {
            let Some(state) = game.level_block_state(level, *at) else { return false };
            let air = game.res.facts().get(state).air;
            if air != (a.target == Target::Air) {
                return false;
            }
            let name = game.res.facts().get(state).name.to_string();
            if game.native_block_name_in_tag(&name, NO_AUTO_CLICK).unwrap_or(false) {
                return false;
            }
            a.t2.is_none_or(|s| block_passes(game, &s.filter, a.filters, &name))
        })
        .collect()
}

/// `findEntitiesToClick`: the living creatures whose box meets the block in front (T1) or the area (T2), of the kind
/// the target names and, for the T2, that the filter lets through: mobs, then players (not fake ones).
fn find_creatures(game: &Game<'_>, level: mod_api::LevelRef, a: &Around<'_>) -> Vec<u32> {
    let (min, max) = match a.t2 {
        Some(s) => s.area.bounds(a.pos),
        None => {
            let front = [a.pos[0] + a.facing[0], a.pos[1] + a.facing[1], a.pos[2] + a.facing[2]];
            (front.map(f64::from), front.map(|v| f64::from(v) + 1.0))
        }
    };
    let meets = |lo: [f64; 3], hi: [f64; 3]| (0..3).all(|k| lo[k] < max[k] && hi[k] > min[k]);
    let mut found = Vec::new();
    for m in game.mobs.iter().filter(|m| m.alive()) {
        let b = m.bounds();
        let (lo, hi) = (b.min_position(), b.max_position());
        if !meets([lo.x, lo.y, lo.z], [hi.x, hi.y, hi.z]) {
            continue;
        }
        let kind = Creature { monster: m.kind.category == "monster", animal: m.kind.category == "creature", baby: m.is_baby(), player: false };
        if a.target.accepts(kind) && a.t2.is_none_or(|s| creature_passes(&s.filter, a.filters, &m.kind.id)) {
            found.push(m.id);
        }
    }
    if matches!(a.target, Target::Player | Target::Living) {
        let fakes: Vec<u32> = game.lan.as_ref().map(|h| h.hub.guests.iter().filter(|g| g.is_fake()).map(|g| g.entity_id as u32).collect()).unwrap_or_default();
        for e in game.level_entities_in(level, min, max).unwrap_or_default() {
            if !fakes.contains(&e.id())
                && game.entity_kind(e).is_some_and(|k| game.entity_type_key_id(k) == "minecraft:player")
                && a.t2.is_none_or(|s| s.filter.accepts(false))
            {
                found.push(e.id());
            }
        }
    }
    found
}

/// `clickBlock`: the fake (`setupFakePlayerForUse` at the place, turned `direction`) clicks it: a block target's
/// right click uses the item on it (then in the air), its left click hits it (`START_DESTROY_BLOCK`), a hold holds
/// the use; an air target uses the item on the block past it within reach (1) or in the air. None when the place no
/// longer is what the target wants (Java's FAIL, the stack kept).
fn click_place(game: &mut Game<'_>, level: mod_api::LevelRef, machine: [i32; 3], at: [i32; 3], direction: usize, s: &Settings, stack: Option<ItemStack>) -> Option<FakeUsed> {
    let (eye, yaw, pitch) = clicker::viewpoint(at, direction);
    let look = NORMALS[direction.min(5)];
    let back = look.map(|v| -v);
    let air_at = |game: &Game<'_>, p: [i32; 3]| game.level_block_state(level, p).is_none_or(|st| game.res.facts().get(st).air);
    let along = |d: f64| std::array::from_fn(|k| eye[k] + f64::from(look[k]) * d);
    let target = match s.target {
        Target::Block if !air_at(game, at) => FakeTarget::Block { pos: at, face: back, point: along(0.001), inside: true },
        Target::Air if air_at(game, at) => {
            let past = std::array::from_fn(|k| at[k] + look[k]);
            if air_at(game, past) { FakeTarget::Air } else { FakeTarget::Block { pos: past, face: back, point: along(0.95), inside: false } }
        }
        _ => return None,
    };
    let click = match s.click_type {
        HOLD => FakeClick::Hold { max: s.max_hold_ticks.max(1) as u32 },
        LEFT if s.target == Target::Block => FakeClick::Attack { dug: 0 },
        _ => FakeClick::Use,
    };
    Some(game.native_fake_use(FakeUse { uuid: FAKE_UUID, name: FAKE_NAME, machine, eye, yaw, pitch, sneaking: s.sneaking, charged: true, stack, target, click }))
}

/// `clickEntity`: the fake stands in the creature's block as for a place there, turned to look at it, and right
/// clicks it (`interactOn`), attacks it, or holds the use. None when the creature is gone.
fn click_creature(game: &mut Game<'_>, level: mod_api::LevelRef, machine: [i32; 3], id: u32, direction: usize, s: &Settings, stack: Option<ItemStack>) -> Option<FakeUsed> {
    let at = match game.mobs.iter().find(|m| m.id == id && m.alive()) {
        Some(m) => [m.position.x, m.position.y, m.position.z],
        None => game.entity(id).and_then(|e| game.entity_position(e))?,
    };
    let _ = level;
    let block = at.map(|v| v.floor() as i32);
    let (eye, _, _) = clicker::viewpoint(block, direction);
    let (yaw, pitch) = clicker::look_at(eye, at);
    let location = std::array::from_fn(|k| eye[k] - at[k]);
    let click = match s.click_type {
        HOLD => FakeClick::Hold { max: s.max_hold_ticks.max(1) as u32 },
        LEFT => FakeClick::Attack { dug: 0 },
        _ => FakeClick::Use,
    };
    let target = FakeTarget::Entity { id, location };
    Some(game.native_fake_use(FakeUse { uuid: FAKE_UUID, name: FAKE_NAME, machine, eye, yaw, pitch, sneaking: s.sneaking, charged: true, stack, target, click }))
}

/// One server tick of a clicker (`ClickerT1BE.tickServer`, `ClickerT2BE.tickServer`), then what its screen shows.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let Some(level) = game.fork_level_here() else { return };
    let dimension = game.native_dimension();
    let t2 = entity.id() == CLICKER_T2;
    let signal = game.native_neighbor_signal(dimension, pos) > 0;
    let facing = Direction::ALL[machines::facing(game, pos)].normal();
    let fresh = t2 && !entity.nbt.contains_key(&machines::key("xRadiusDouble"));
    let mut shared = read_shared(&entity.nbt, facing);
    let energy = shared.powered.energy;
    let before = machines::clock_at(dimension, pos, &entity.nbt);
    let (places, creatures, mut holding) = QUEUES.with(|q| q.borrow_mut().remove(&(dimension, pos))).unwrap_or_default();
    let settings = read_settings(&entity.nbt);
    let mut c = Clicker { clock: before, settings, places, creatures };
    let direction = machines::int(&entity.nbt, "direction").clamp(0, 5) as usize;
    let filters = if t2 { filter_items(game, entity) } else { Vec::new() };
    let can_click = !t2 || shared.powered.has(T2_CLICK_COST);
    let around = Around { pos, facing, t2: t2.then_some(&shared), filters: &filters, target: settings.target };
    let view: &Game<'_> = game;
    let click = c.tick(signal, can_click, holding, || find_places(view, level, &around), || find_creatures(view, level, &around));
    if let Some(click) = click {
        let stack = entity.items.first().and_then(|s| s.as_ref()).and_then(|s| s.try_into_engine(game.items()).ok()).filter(|s: &ItemStack| s.count > 0);
        let used = match click {
            Click::Place(at) => click_place(game, level, pos, at, direction, &settings, stack),
            Click::Creature(id) => click_creature(game, level, pos, id, direction, &settings, stack),
        };
        if let Some(used) = used {
            holding = used.holding;
            set_slot(entity, if t2 { T2_SLOTS } else { machines::HOPPER_SLOTS }, used.stack);
            // `cleanupFakePlayerFromUse`: what else the fake holds is dropped where it stands.
            let (eye, _, _) = clicker::viewpoint(pos, direction);
            for stack in used.other {
                game.level_spawn_item(level, eye[0], eye[1], eye[2], stack, 40);
            }
            // The T2 pays for a click Java's fake answered SUCCESS to (a menu it stopped still did).
            if t2 && matches!(used.outcome, Ok(ActionOutcome::Consumed | ActionOutcome::EffectBlocked(_))) {
                shared.powered.take(T2_CLICK_COST);
            }
        }
    }
    QUEUES.with(|q| q.borrow_mut().insert((dimension, pos), (c.places, c.creatures, holding)));
    machines::keep_clock(dimension, pos, &mut entity.nbt, &before, &c.clock);
    if fresh {
        write_shared(&mut entity.nbt, &shared);
    } else if t2 && shared.powered.energy != energy {
        machines::put(&mut entity.nbt, "energy", shared.powered.energy);
    }
    let data = screen_data(&c.clock, &settings, direction, entity.items.first().is_some_and(|s| s.is_some()), t2.then_some(&shared));
    if entity.data != data {
        entity.data = data;
    }
}

/// What a clicker's screen reads: 0 redstone, 1 whether the slot holds an item, 2 direction, 3 tick speed, 4 click
/// type, 5 target, 6 sneaking, 7 show fake player, 8 hold ticks; a T2's 9 energy, 10 to 12 the radius in half blocks,
/// 13 to 15 the offsets, 16 allowlist, 17 compare components, 18 render area.
fn screen_data(clock: &justdirethings_core::machines::Clock, s: &Settings, direction: usize, has_item: bool, shared: Option<&Shared>) -> Vec<i32> {
    let mut data = vec![
        clock.redstone.mode.index(),
        i32::from(has_item),
        direction as i32,
        clock.tick_speed,
        s.click_type,
        s.target.index(),
        i32::from(s.sneaking),
        i32::from(s.show_fake_player),
        s.max_hold_ticks,
    ];
    if let Some(sh) = shared {
        data.push(sh.powered.energy);
        data.extend(sh.area.radius.map(|r| (r * 2.0).round() as i32));
        data.extend(sh.area.offset);
        data.extend([i32::from(sh.filter.allowlist), i32::from(sh.filter.compare_nbt), i32::from(sh.area.render)]);
    }
    data
}

/// The screen's buttons (`native_menu_button`): 0 to 3 the redstone mode, 22 and 23 the tick speed, 34 to 39 the
/// direction, 40 to 42 the click type, 43 to 50 the target, 51 sneaking, 52 show the fake player, 53 and 54 the ticks a
/// use is held; a T2's area and filter buttons (`t2::Shared::button`). A held use keeps the tick speed above its
/// ticks (`Math.max(tickSpeed, maxHoldTicks + 1)`).
pub(super) fn button(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, id: i32) {
    let dimension = game.native_dimension();
    let before = machines::clock_at(dimension, pos, &entity.nbt);
    let mut clock = before;
    let mut s = read_settings(&entity.nbt);
    let settings_before = s;
    let min = |s: &Settings| if s.click_type == HOLD { s.max_hold_ticks + 1 } else { machines::MIN_TICK_SPEED };
    match id {
        0..=3 => clock.redstone.mode = Mode::from_index(id),
        22 => clock.set_tick_speed(clock.tick_speed + 1, min(&s)),
        23 => clock.set_tick_speed(clock.tick_speed - 1, min(&s)),
        34..=39 => machines::put(&mut entity.nbt, "direction", id - 34),
        40..=42 => s.click_type = id - 40,
        43..=50 => s.target = Target::from_index(id - 43),
        51 => s.sneaking = !s.sneaking,
        52 => s.show_fake_player = !s.show_fake_player,
        53 => s.max_hold_ticks = (s.max_hold_ticks + 1).min(1200),
        54 => s.max_hold_ticks = (s.max_hold_ticks - 1).max(1),
        _ if entity.id() == CLICKER_T2 => {
            let facing = Direction::ALL[machines::facing(game, pos)].normal();
            let mut shared = read_shared(&entity.nbt, facing);
            if shared.button(id) {
                write_shared(&mut entity.nbt, &shared);
                QUEUES.with(|q| q.borrow_mut().remove(&(dimension, pos)));
            }
            return;
        }
        _ => return,
    }
    if s.click_type == HOLD && clock.tick_speed < s.max_hold_ticks + 1 {
        clock.set_tick_speed(s.max_hold_ticks + 1, s.max_hold_ticks + 1);
    }
    if s != settings_before {
        // `setClickerSettings`: what was queued is dropped.
        write_settings(&mut entity.nbt, &s);
        QUEUES.with(|q| q.borrow_mut().remove(&(dimension, pos)));
    }
    machines::keep_clock(dimension, pos, &mut entity.nbt, &before, &clock);
}

/// `native_fe_storage` of a T2 machine: its buffer takes from every side and gives to none.
pub(super) fn fe_storage(entity: &ModBlockEntity, info: &mut Option<NativeFeInfo>) {
    *info = Some(NativeFeInfo {
        stored: i64::from(machines::int(&entity.nbt, "energy")),
        capacity: i64::from(t2::CAPACITY),
        can_receive: true,
        can_extract: false,
    });
}

/// `native_fe_receive` of a T2 machine.
pub(super) fn fe_receive(entity: &mut ModBlockEntity, max: i64, simulate: bool, accepted: &mut Option<i64>) {
    let mut p = Powered { energy: machines::int(&entity.nbt, "energy").clamp(0, t2::CAPACITY) };
    let took = p.receive(max, simulate);
    if took > 0 && !simulate {
        machines::put(&mut entity.nbt, "energy", p.energy);
    }
    *accepted = Some(took);
}
