// Just Dire Things' fluid drop, time crystals and Paradox Machine in the game (support module
// `crate::game::justdirethings_time` of the `justdirethings` native bundle). The rules are the bundle's crate
// `justdirethings_core`; this module is the glue: the fluid drop recipes read from the world's tables and the item
// entities that tick in a liquid, the budding block's random tick, and the Paradox Machine's block entity (Java's
// keys), its Forge Energy, its time fluid tank and buckets, its snapshot of blocks and mobs, its runs and the data
// its screen reads. Translated from Just Dire Things 1.5.7 (MIT); see this bundle's README.md.

use super::{Game, GameLevel, justdirethings_world as world};
use horizon_client::containers::ModContainer;
use horizon_core::geometry::Direction;
use horizon_core::nbt::{Compound, JavaString, Tag};
use horizon_core::position::BlockPos;
use horizon_core::random::RandomSource;
use horizon_world::fluid::{FluidKind, FluidState};
use horizon_world::level::{LevelAccess, LevelEvent, Ticks, flags};
use horizon_world::registry::BlockStateId;
use justdirethings_core::{
    SIDE_NAMES, SIDES,
    config::ParadoxSettings,
    crystal::{self, Beside},
    fluiddrop::{self, FluidDrop},
    paradox::{self, Machine, Paradox, Sound, Target},
    recipe::StateSpec,
    redstone::Mode,
    view::ParadoxView,
};
use mod_api::{BlockUpdateFlags, FluidStack, InteractionResult, ModBlockEntity, NativeFeInfo, NativeFluidTankInfo, SoundSource};
use std::{cell::RefCell, collections::HashSet, rc::Rc};

pub(super) const PARADOX: &str = paradox::BLOCK;

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

fn double(nbt: &Compound, name: &str) -> f64 {
    match nbt.get(&key(name)) {
        Some(Tag::Double(v)) => f64::from_bits(*v),
        Some(Tag::Float(v)) => f64::from(f32::from_bits(*v)),
        Some(Tag::Int(v)) => f64::from(*v),
        _ => 0.0,
    }
}

fn string(s: &str) -> Tag {
    Tag::String(JavaString::from(s))
}

fn put(nbt: &mut Compound, name: &str, v: i32) {
    nbt.insert(key(name), Tag::Int(v));
}

fn put_flag(nbt: &mut Compound, name: &str, v: bool) {
    nbt.insert(key(name), Tag::Byte(i8::from(v)));
}

fn center(pos: [i32; 3]) -> [f64; 3] {
    [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.5, f64::from(pos[2]) + 0.5]
}

thread_local! {
    static DROPS: RefCell<Option<Rc<Drops>>> = const { RefCell::new(None) };
}

/// The world opened or closed: the fluid drop recipes are read again on first use.
pub(super) fn reset() {
    DROPS.with(|d| *d.borrow_mut() = None);
}

// ---- The fluid drop --------------------------------------------------------------------------------------------

/// The fluid drop recipes (`justdirethings:fluiddrop`, in the table's order), each input's fluid and output state.
struct Drops {
    recipes: Vec<FluidDrop>,
    inputs: Vec<Option<FluidKind>>,
    outputs: Vec<Option<BlockStateId>>,
    catalysts: HashSet<String>,
}

/// The fluid of block state `state` (`BlockState.getFluidState`).
fn fluid_of(game: &Game<'_>, state: BlockStateId) -> Option<FluidState> {
    let info = game.pack.assets.registry.state(state)?;
    FluidState::from_block(info).ok()
}

fn drops(game: &Game<'_>) -> Rc<Drops> {
    if let Some(d) = DROPS.with(|d| d.borrow().clone()) {
        return d;
    }
    let mut recipes = Vec::new();
    for r in game.native_recipes_of_type(fluiddrop::FLUID_DROP).iter() {
        match FluidDrop::parse(&r.id, &r.json) {
            Ok(parsed) => recipes.push(parsed),
            Err(e) => log::warn!("[justdirethings] {e}"),
        }
    }
    let inputs = recipes.iter().map(|r| world::resolve(game, &r.input).and_then(|s| fluid_of(game, s)).map(FluidState::kind)).collect();
    let outputs = recipes.iter().map(|r| world::resolve(game, &r.output)).collect();
    let catalysts = recipes.iter().map(|r| r.catalyst.clone()).collect();
    log::info!("[justdirethings] {} fluid drop recipe(s)", recipes.len());
    let fresh = Rc::new(Drops { recipes, inputs, outputs, catalysts });
    DROPS.with(|d| *d.borrow_mut() = Some(fresh.clone()));
    fresh
}

/// `native_entity_tick_listened`: the fluid drop hears the item entities' ticks.
pub(super) fn tick_listened(listened: &mut bool) {
    *listened = true;
}

/// `native_entity_tick_post` (`EntityEvents.entityTick`, `EntityTickEvent.Post`): an item entity of a recipe's
/// catalyst in a source block of its input fluid turns the block into the output, using one item up, with the fire's
/// extinguish sound.
pub(super) fn entity_tick_post(game: &mut Game<'_>, entity: u32, kind: &str) {
    if kind != "minecraft:item" || game.remote.is_some() {
        return;
    }
    let Some((stack, at)) = game.dropped.iter().find(|d| d.id == entity).map(|d| (d.stack.clone(), d.position)) else { return };
    if stack.count == 0 {
        return;
    }
    let drops = drops(game);
    if !drops.catalysts.contains(&*stack.item) {
        return;
    }
    let pos = [at.x.floor() as i32, at.y.floor() as i32, at.z.floor() as i32];
    let Some(level) = game.fork_level_here() else { return };
    let Some(state) = game.level_block_state(level, pos) else { return };
    // `blockState.getBlock() instanceof LiquidBlock`.
    if !game.res.facts().get(state).classes.iter().any(|c| c.ends_with("LiquidBlock")) {
        return;
    }
    let here = fluid_of(game, state).filter(|f| f.is_source()).map(FluidState::kind);
    let Some(found) = fluiddrop::find(&drops.recipes, &stack.item, here, |r| {
        let index = drops.recipes.iter().position(|o| std::ptr::eq(o, r))?;
        drops.inputs[index]
    }) else {
        return;
    };
    let index = drops.recipes.iter().position(|o| std::ptr::eq(o, found)).unwrap_or(0);
    let Some(output) = drops.outputs[index] else { return };
    if !matches!(game.level_set_block(level, pos, output, BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS), Ok(true)) {
        return;
    }
    let Some(handle) = game.entity(entity) else { return };
    if stack.count > 1 {
        let mut left = stack.clone();
        left.count -= 1;
        if let Some(item) = game.item_entity(handle) {
            game.item_entity_set_stack(item, left);
        }
    } else {
        // The stack is empty: Java's item entity discards itself on its next tick.
        game.entity_remove(handle, mod_api::RemovalReason::Discarded);
    }
    game.level_play_sound(level, "minecraft:block.fire.extinguish", center(pos), SoundSource::Blocks, 1.0, 1.0);
}

// ---- The time crystal's budding block --------------------------------------------------------------------------

struct LevelDraw<'a>(&'a mut horizon_core::random::LegacyRandom);

impl crystal::Draw for LevelDraw<'_> {
    fn next_int(&mut self, bound: i32) -> i32 {
        self.0.next_int_bound(bound).unwrap_or(0)
    }
    fn next_float(&mut self) -> f32 {
        self.0.next_float()
    }
}

fn level_property(level: &GameLevel<'_>, state: BlockStateId, name: &str) -> Option<String> {
    level.registry.state(state)?.properties.get(name).cloned()
}

fn beside(pos: BlockPos, side: usize) -> BlockPos {
    let [dx, dy, dz] = Direction::ALL[side].normal();
    BlockPos::new(pos.x + dx, pos.y + dy, pos.z + dz)
}

/// What is on side `side` of the budding block, as `randomTick` reads it.
fn read_beside(level: &GameLevel<'_>, pos: BlockPos) -> Beside {
    let state = level.block_state(pos);
    let Some(info) = level.registry.state(state) else { return Beside::Other };
    if level.facts.get(state).air {
        return Beside::Room { water: false };
    }
    if &*info.name == "minecraft:water" {
        return if info.properties.get("level").map(String::as_str) == Some("0") { Beside::Room { water: true } } else { Beside::Other };
    }
    let Some(size) = crystal::CLUSTERS.iter().position(|c| *c == &*info.name) else { return Beside::Other };
    let facing = info.properties.get("facing").and_then(|f| SIDE_NAMES.iter().position(|n| n == f)).unwrap_or(1);
    let water = info.properties.get("waterlogged").map(String::as_str) == Some("true");
    Beside::Cluster { size, facing, water }
}

/// `native_random_tick` (`TimeCrystalBuddingBlock.randomTick`): the budding block's stage moves on by its dimension
/// and at stage 3 it grows clusters; its template's own growth (budding amethyst's) never runs.
pub(super) fn random_tick(level: &mut GameLevel<'_>, ticks: &mut Ticks, dimension: horizon_core::dimension::Dimension, pos: BlockPos, state: BlockStateId, handled: &mut bool) {
    if level.registry.state(state).map(|s| &*s.name) != Some(crystal::BUDDING) {
        return;
    }
    *handled = true;
    let stage = level_property(level, state, "stage").and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
    let settings = world::settings().crystal;
    let sides: Vec<Beside> = (0..SIDES).map(|side| read_beside(level, beside(pos, side))).collect();
    let tick = crystal::random_tick(stage, dimension.key().id(), &settings, |side| sides[side], &mut LevelDraw(level.random()));
    let at = [pos.x, pos.y, pos.z];
    if let Some(next) = tick.advance {
        level.native_set_block_property(ticks, at, "stage", &next.to_string(), flags::UPDATE_ALL);
        level.emit(LevelEvent::Sound { pos, sound: "minecraft:block.respawn_anchor.charge".into(), volume: 1.0, pitch: 0.25 });
    }
    if let Some(grow) = tick.grow {
        let mut cluster = match level.registry.default_state(crystal::CLUSTERS[grow.size]).and_then(|d| level.registry.state(d)) {
            Some(s) => s.clone(),
            None => return,
        };
        cluster.properties.insert("facing".into(), SIDE_NAMES[grow.side].into());
        cluster.properties.insert("waterlogged".into(), grow.water.to_string());
        if let Ok(id) = level.registry.resolve(&cluster) {
            let b = level.behaviours;
            b.set_block(level, ticks, beside(pos, grow.side), id, flags::UPDATE_ALL);
        }
        if tick.depleted {
            level.native_set_block_property(ticks, at, "stage", "0", flags::UPDATE_ALL);
            level.emit(LevelEvent::Sound { pos, sound: "minecraft:block.respawn_anchor.deplete".into(), volume: 1.0, pitch: 0.25 });
        }
    }
}

// ---- The Paradox Machine: its saved compound -------------------------------------------------------------------

/// The facing's normal of the machine at `pos` (`BlockStateProperties.FACING`).
fn facing_normal(game: &Game<'_>, pos: [i32; 3]) -> [i32; 3] {
    let side = game.native_block_property(pos, "facing").and_then(|f| SIDE_NAMES.iter().position(|n| *n == f)).unwrap_or(2);
    Direction::ALL[side].normal()
}

fn read_paradox(nbt: &Compound, normal: [i32; 3]) -> Paradox {
    let mut p = Paradox::new(normal);
    p.energy = int(nbt, "energy");
    p.fluid = int(nbt, "timeFluid");
    if nbt.contains_key(&key("redstoneMode")) {
        p.redstone.mode = Mode::from_index(int(nbt, "redstoneMode"));
        p.redstone.pulsed = int(nbt, "pulsed") != 0;
        p.redstone.receiving = int(nbt, "receivingRedstone") != 0;
    }
    if nbt.contains_key(&key("tickspeed")) {
        p.tick_speed = int(nbt, "tickspeed").max(1);
    }
    p.operation_ticks = if nbt.contains_key(&key("operationTicks")) { int(nbt, "operationTicks") } else { -1 };
    if nbt.contains_key(&key("xRadiusDouble")) {
        p.area.radius = [double(nbt, "xRadiusDouble"), double(nbt, "yRadiusDouble"), double(nbt, "zRadiusDouble")];
        p.area.offset = [int(nbt, "xOffset"), int(nbt, "yOffset"), int(nbt, "zOffset")];
        p.area.render = int(nbt, "renderArea") != 0;
    }
    p.target = Target::from_index(int(nbt, "targetType"));
    p.render_paradox = int(nbt, "renderParadox") != 0;
    p.running = int(nbt, "isRunning") != 0;
    p.time_running = int(nbt, "timeRunning");
    p.fe_per_tick = int(nbt, "fePerTick");
    p.fluid_per_tick = int(nbt, "fluidPerTick");
    p.paradox_energy = match nbt.get(&key("paradoxEnergy")) {
        Some(Tag::Float(v)) => f32::from_bits(*v),
        _ => 0.0,
    };
    let count = |name: &str| match nbt.get(&key(name)) {
        Some(Tag::List(l)) => l.len(),
        _ => 0,
    };
    p.planned = (count("restoringBlocks"), count("restoringEntities"));
    p
}

fn write_paradox(nbt: &mut Compound, p: &Paradox) {
    put(nbt, "energy", p.energy);
    put(nbt, "timeFluid", p.fluid);
    put(nbt, "redstoneMode", p.redstone.mode.index());
    put_flag(nbt, "pulsed", p.redstone.pulsed);
    put_flag(nbt, "receivingRedstone", p.redstone.receiving);
    put(nbt, "tickspeed", p.tick_speed);
    put(nbt, "operationTicks", p.operation_ticks);
    for (i, axis) in ["x", "y", "z"].into_iter().enumerate() {
        nbt.insert(key(&format!("{axis}RadiusDouble")), Tag::Double(p.area.radius[i].to_bits()));
        put(nbt, &format!("{axis}Offset"), p.area.offset[i]);
    }
    put_flag(nbt, "renderArea", p.area.render);
    put(nbt, "targetType", p.target.index());
    put_flag(nbt, "renderParadox", p.render_paradox);
    put_flag(nbt, "isRunning", p.running);
    put(nbt, "timeRunning", p.time_running);
    put(nbt, "fePerTick", p.fe_per_tick);
    put(nbt, "fluidPerTick", p.fluid_per_tick);
    nbt.insert(key("paradoxEnergy"), Tag::Float(p.paradox_energy.to_bits()));
}

/// `NbtUtils.writeBlockPos`: an int array.
fn block_pos_tag(pos: [i32; 3]) -> Tag {
    Tag::IntArray(pos.to_vec())
}

fn read_block_pos(tag: Option<&Tag>) -> Option<[i32; 3]> {
    match tag? {
        Tag::IntArray(v) if v.len() == 3 => Some([v[0], v[1], v[2]]),
        Tag::Compound(c) => Some([int(c, "X"), int(c, "Y"), int(c, "Z")]),
        _ => None,
    }
}

/// `NBTHelpers.vec3ToNBT`.
fn vec3_tag(v: [f64; 3]) -> Tag {
    let mut c = Compound::new();
    c.insert(key("x"), Tag::Double(v[0].to_bits()));
    c.insert(key("y"), Tag::Double(v[1].to_bits()));
    c.insert(key("z"), Tag::Double(v[2].to_bits()));
    Tag::Compound(c)
}

fn read_vec3(c: &Compound) -> [f64; 3] {
    [double(c, "x"), double(c, "y"), double(c, "z")]
}

/// `NbtUtils.writeBlockState`: `{Name, Properties}`.
fn state_tag(game: &Game<'_>, state: BlockStateId) -> Option<Tag> {
    let info = game.pack.assets.registry.state(state)?;
    let mut c = Compound::new();
    c.insert(key("Name"), string(&info.name));
    if !info.properties.is_empty() {
        let mut props = Compound::new();
        for (k, v) in info.properties.iter() {
            props.insert(key(k), string(v));
        }
        c.insert(key("Properties"), Tag::Compound(props));
    }
    Some(Tag::Compound(c))
}

fn read_state(game: &Game<'_>, tag: Option<&Tag>) -> Option<BlockStateId> {
    let Some(Tag::Compound(c)) = tag else { return None };
    let Some(Tag::String(name)) = c.get(&key("Name")) else { return None };
    let mut spec = StateSpec::block(&name.to_string_lossy());
    if let Some(Tag::Compound(props)) = c.get(&key("Properties")) {
        for (k, v) in props.iter() {
            if let Tag::String(v) = v {
                spec = spec.with(&k.to_string_lossy(), &v.to_string_lossy());
            }
        }
    }
    world::resolve(game, &spec)
}

fn list<'a>(nbt: &'a Compound, name: &str) -> &'a [Tag] {
    match nbt.get(&key(name)) {
        Some(Tag::List(l)) => l,
        _ => &[],
    }
}

fn snapshot(nbt: &Compound) -> Option<&Compound> {
    match nbt.get(&key("snapshotData")) {
        Some(Tag::Compound(c)) if !c.is_empty() => Some(c),
        _ => None,
    }
}

// ---- The Paradox Machine in the world --------------------------------------------------------------------------

/// The snapshot's blocks (`getBlocksFromNBT`): world position and state, unless the target is entities only.
fn snapshot_blocks(game: &Game<'_>, nbt: &Compound, pos: [i32; 3], target: Target) -> Vec<([i32; 3], BlockStateId)> {
    let Some(snap) = snapshot(nbt).filter(|_| target.blocks()) else { return Vec::new() };
    list(snap, "blocks")
        .iter()
        .filter_map(|t| {
            let Tag::Compound(c) = t else { return None };
            let rel = read_block_pos(c.get(&key("pos")))?;
            let state = read_state(game, c.get(&key("state")))?;
            Some(([pos[0] + rel[0], pos[1] + rel[1], pos[2] + rel[2]], state))
        })
        .collect()
}

/// The snapshot's entities (`getEntitiesFromNBT`): world position, type and data, unless the target is blocks only,
/// and without those whose UUID is in the level (`restoreEntityFromNBT`).
fn snapshot_entities(game: &Game<'_>, nbt: &Compound, pos: [i32; 3], target: Target) -> Vec<([f64; 3], String, Compound)> {
    let Some(snap) = snapshot(nbt).filter(|_| target.entities()) else { return Vec::new() };
    let c = center(pos);
    list(snap, "entities")
        .iter()
        .filter_map(|t| {
            let Tag::Compound(e) = t else { return None };
            let Some(Tag::Compound(rel)) = e.get(&key("relativePos")) else { return None };
            let rel = read_vec3(rel);
            let Some(Tag::Compound(data)) = e.get(&key("data")) else { return None };
            let Some(Tag::String(id)) = data.get(&key("id")) else { return None };
            if let Some(Tag::IntArray(uuid)) = data.get(&key("UUID"))
                && uuid.len() == 4
                && game.target_with_uuid([uuid[0], uuid[1], uuid[2], uuid[3]]).is_some()
            {
                return None;
            }
            Some(([c[0] + rel[0], c[1] + rel[1], c[2] + rel[2]], id.to_string_lossy(), data.clone()))
        })
        .collect()
}

/// `canPlace` without the fake player's checks: the block there can be replaced.
fn replaceable(game: &Game<'_>, level: mod_api::LevelRef, pos: [i32; 3]) -> bool {
    game.level_block_state(level, pos).is_some_and(|s| game.res.facts().get(s).replaceable)
}

/// The machine's world side for one tick.
struct World<'a, 'g> {
    game: &'a mut Game<'g>,
    level: mod_api::LevelRef,
    pos: [i32; 3],
    nbt: &'a mut Compound,
    cfg: ParadoxSettings,
}

impl Machine for World<'_, '_> {
    fn has_snapshot(&self) -> bool {
        snapshot(self.nbt).is_some()
    }

    fn plan(&mut self, target: Target) -> (usize, usize) {
        let blocks: Vec<Tag> = snapshot_blocks(self.game, self.nbt, self.pos, target)
            .into_iter()
            .filter(|(at, _)| replaceable(self.game, self.level, *at))
            .filter_map(|(at, state)| {
                let mut c = Compound::new();
                c.insert(key("pos"), block_pos_tag(at));
                c.insert(key("state"), state_tag(self.game, state)?);
                Some(Tag::Compound(c))
            })
            .collect();
        let entities: Vec<Tag> = snapshot_entities(self.game, self.nbt, self.pos, target).into_iter().map(|(at, _, _)| vec3_tag(at)).collect();
        let counts = (blocks.len(), entities.len());
        self.nbt.insert(key("restoringBlocks"), Tag::List(blocks));
        self.nbt.insert(key("restoringEntities"), Tag::List(entities));
        counts
    }

    fn restore(&mut self) {
        let mut restored = 0;
        for t in list(self.nbt, "restoringBlocks").to_vec() {
            let Tag::Compound(c) = t else { continue };
            let (Some(at), Some(state)) = (read_block_pos(c.get(&key("pos"))), read_state(self.game, c.get(&key("state")))) else { continue };
            if !replaceable(self.game, self.level, at) {
                continue;
            }
            if matches!(self.game.level_set_block(self.level, at, state, BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS), Ok(true)) {
                restored += 1;
            }
        }
        let planned: Vec<[f64; 3]> = list(self.nbt, "restoringEntities")
            .iter()
            .filter_map(|t| match t {
                Tag::Compound(c) => Some(read_vec3(c)),
                _ => None,
            })
            .collect();
        let dimension = self.game.native_dimension();
        let mut summoned = 0;
        for (at, kind, data) in snapshot_entities(self.game, self.nbt, self.pos, Target::Entities) {
            if !planned.contains(&at) {
                continue;
            }
            if self.game.summon(dimension, &kind, at, sanitize(&data, self.cfg.restricted_mobs)).is_some() {
                summoned += 1;
            }
        }
        log::info!("[justdirethings] paradox machine at {:?} restored {restored} block(s) and {summoned} entit(ies)", self.pos);
    }

    fn clear(&mut self) {
        self.nbt.insert(key("restoringBlocks"), Tag::List(Vec::new()));
        self.nbt.insert(key("restoringEntities"), Tag::List(Vec::new()));
    }

    fn sound(&mut self, sound: Sound) {
        self.game.level_play_sound(self.level, sound.id(), center(self.pos), SoundSource::Blocks, 0.5, 0.25);
    }
}

/// `sanitizeEntityDataDeny` (or `sanitizeEntityData` with `paradox_restricted_mobs`): a mob's saved data without
/// what it carries, so a restore makes no items.
fn sanitize(data: &Compound, restricted: bool) -> Compound {
    let mut out = Compound::new();
    for (k, v) in data.iter() {
        let name = k.to_string_lossy();
        let keep = if restricted { paradox::RESTRICTED_FIELDS.contains(&name.as_str()) } else { !paradox::DENIED_FIELDS.contains(&name.as_str()) };
        if keep && name != "id" && name != "Pos" {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

/// `snapshotArea`: the blocks of the allow tag and the living entities (mobs: no player, no entity of the deny
/// tags, no ender dragon) in the area, relative to the machine.
fn snapshot_area(game: &mut Game<'_>, level: mod_api::LevelRef, pos: [i32; 3], p: &Paradox, nbt: &mut Compound) {
    let mut blocks = Vec::new();
    for at in p.area.blocks(pos) {
        let Some(state) = game.level_block_state(level, at) else { continue };
        let facts = game.res.facts().get(state);
        if facts.air {
            continue;
        }
        let name = facts.name.to_string();
        if !game.native_block_name_in_tag(&name, paradox::ALLOW_TAG).unwrap_or(false) {
            continue;
        }
        let Some(state_tag) = state_tag(game, state) else { continue };
        let mut c = Compound::new();
        c.insert(key("pos"), block_pos_tag([at[0] - pos[0], at[1] - pos[1], at[2] - pos[2]]));
        c.insert(key("state"), state_tag);
        blocks.push(Tag::Compound(c));
    }
    let (min, max) = p.area.bounds(pos);
    let found: Vec<(u32, String, [f64; 3])> = game
        .mobs
        .iter()
        .filter(|m| m.alive())
        .filter(|m| {
            let b = m.bounds();
            let (lo, hi) = (b.min_position(), b.max_position());
            lo.x < max[0] && hi.x > min[0] && lo.y < max[1] && hi.y > min[1] && lo.z < max[2] && hi.z > min[2]
        })
        .map(|m| (m.id, m.kind.id.to_string(), [m.position.x, m.position.y, m.position.z]))
        .collect();
    let c = center(pos);
    let mut entities = Vec::new();
    for (id, kind, at) in found {
        if kind == "minecraft:ender_dragon" || paradox::DENY_TAGS.iter().any(|t| game.entity_type_in_tag(&kind, t)) {
            continue;
        }
        let Some(handle) = game.entity(id) else { continue };
        let Some(mut data) = game.entity_nbt(handle) else { continue };
        data.insert(key("id"), string(&kind));
        let mut e = Compound::new();
        e.insert(key("relativePos"), vec3_tag([at[0] - c[0], at[1] - c[1], at[2] - c[2]]));
        e.insert(key("data"), Tag::Compound(data));
        entities.push(Tag::Compound(e));
    }
    log::info!("[justdirethings] paradox machine at {pos:?}: snapshot of {} block(s) and {} entit(ies)", blocks.len(), entities.len());
    let mut snap = Compound::new();
    snap.insert(key("blocks"), Tag::List(blocks));
    snap.insert(key("entities"), Tag::List(entities));
    nbt.insert(key("snapshotData"), Tag::Compound(snap));
}

/// `native_block_entity`: a new machine has no snapshot; its area is set on its first tick from its facing.
pub(super) fn new_entity(nbt: &mut Compound) {
    nbt.insert(key("snapshotData"), Tag::Compound(Compound::new()));
    put(nbt, "direction", 0);
}

/// `native_block_entity_container`: the hopper menu the screen shows, with no slot of its own (the menu is only the
/// player's inventory, as `ParadoxMachineContainer`).
pub(super) fn container(container: &mut Option<ModContainer>) {
    *container = Some(ModContainer {
        slots: 5,
        faces: Default::default(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        menu: Some("minecraft:hopper".into()),
    });
}

thread_local! {
    /// What a run would restore now, by machine, read again every 20 ticks for the screen's tooltips (with the
    /// ticks since).
    static COUNTS: RefCell<std::collections::HashMap<(&'static str, [i32; 3]), (usize, usize, u32)>> = RefCell::new(std::collections::HashMap::new());
}

/// One server tick of the machine (`ParadoxMachineBE.tickServer`), then what its screen shows.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let Some(level) = game.fork_level_here() else { return };
    let dimension = game.native_dimension();
    let signal = game.native_neighbor_signal(dimension, pos) > 0;
    let cfg = world::settings().paradox;
    let normal = facing_normal(game, pos);
    let mut p = read_paradox(&entity.nbt, normal);
    let mut w = World { game: &mut *game, level, pos, nbt: &mut entity.nbt, cfg };
    p.tick(&cfg, signal, &mut w);
    write_paradox(&mut entity.nbt, &p);
    let cached = COUNTS.with(|c| c.borrow().get(&(dimension, pos)).copied());
    let counts = match cached {
        Some((blocks, entities, age)) if age < 20 => {
            COUNTS.with(|c| c.borrow_mut().insert((dimension, pos), (blocks, entities, age + 1)));
            (blocks, entities)
        }
        _ => {
            let blocks = snapshot_blocks(game, &entity.nbt, pos, p.target).into_iter().filter(|(at, _)| replaceable(game, level, *at)).count();
            let entities = snapshot_entities(game, &entity.nbt, pos, p.target).len();
            COUNTS.with(|c| c.borrow_mut().insert((dimension, pos), (blocks, entities, 0)));
            (blocks, entities)
        }
    };
    let view = ParadoxView {
        energy: p.energy,
        energy_capacity: cfg.rf_capacity,
        fluid: p.fluid,
        fluid_capacity: cfg.fluid_capacity,
        paradox_energy: (p.paradox_energy * 100.0).round() as i32,
        paradox_max: (cfg.energy_max * 100.0).round().min(f64::from(i32::MAX)) as i32,
        redstone: p.redstone.mode.index(),
        target: p.target.index(),
        render_paradox: i32::from(p.render_paradox),
        render_area: i32::from(p.area.render),
        radius: p.area.radius.map(|r| (r * 2.0).round() as i32),
        offset: p.area.offset,
        tick_speed: p.tick_speed,
        running: i32::from(p.running),
        time_running: p.time_running,
        run_time: if p.running { p.run_time() } else { 0 },
        blocks: counts.0 as i32,
        entities: counts.1 as i32,
        energy_cost: paradox::energy_cost(&cfg, counts.0, counts.1),
        fluid_cost: paradox::fluid_cost(&cfg, counts.0, counts.1),
    }
    .pack();
    if entity.data != view {
        entity.data = view;
    }
}

/// The screen's buttons (`native_menu_button`): 0 to 3 the redstone mode, 4 the snapshot, 5 the paradox's render
/// toggle, 6 to 8 the target, 9 the area's render toggle, 10 to 15 a radius up or down (x, y, z), 16 to 21 an offset
/// up or down, 22 and 23 the tick speed up or down.
pub(super) fn button(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, id: i32) {
    let normal = facing_normal(game, pos);
    let mut p = read_paradox(&entity.nbt, normal);
    match id {
        0..=3 => p.redstone.mode = Mode::from_index(id),
        4 => {
            let Some(level) = game.fork_level_here() else { return };
            snapshot_area(game, level, pos, &p, &mut entity.nbt);
        }
        5 => p.render_paradox = !p.render_paradox,
        6..=8 => p.target = Target::from_index(id - 6),
        9 => p.area.render = !p.area.render,
        10..=15 => p.area.step_radius(((id - 10) / 2) as usize, (id - 10) % 2 == 0),
        16..=21 => p.area.step_offset(((id - 16) / 2) as usize, (id - 16) % 2 == 0),
        22 => p.tick_speed = (p.tick_speed + 1).min(1200),
        23 => p.tick_speed = (p.tick_speed - 1).max(1),
        _ => return,
    }
    if p.operation_ticks > p.tick_speed {
        p.operation_ticks = p.tick_speed;
    }
    write_paradox(&mut entity.nbt, &p);
}

/// `native_fe_storage`: the buffer takes from every side and gives to none.
pub(super) fn fe_storage(entity: &ModBlockEntity, info: &mut Option<NativeFeInfo>) {
    *info = Some(NativeFeInfo {
        stored: i64::from(int(&entity.nbt, "energy")),
        capacity: i64::from(world::settings().paradox.rf_capacity),
        can_receive: true,
        can_extract: false,
    });
}

/// `native_fe_receive`.
pub(super) fn fe_receive(entity: &mut ModBlockEntity, max: i64, simulate: bool, accepted: &mut Option<i64>) {
    let cfg = world::settings().paradox;
    let mut p = Paradox::new([0, 0, 0]);
    p.energy = int(&entity.nbt, "energy");
    let took = p.receive_energy(&cfg, max, simulate);
    if took > 0 && !simulate {
        put(&mut entity.nbt, "energy", p.energy);
    }
    *accepted = Some(took);
}

/// Whether `stack` is time fluid (its source or flowing form).
fn is_time_fluid(stack: &FluidStack) -> bool {
    stack.key().and_then(mod_api::__private::fluid_id).is_some_and(|id| paradox::TIME_FLUIDS.contains(&id))
}

/// `amount` of time fluid; none for none, or a game without the fluid.
fn time_fluid(amount: i32) -> Option<FluidStack> {
    let key = mod_api::__private::fluid_key(paradox::TIME_FLUIDS[0])?;
    (amount > 0).then(|| FluidStack::new(key, amount.unsigned_abs()))
}

/// `native_fluid_handler_tanks`: one tank of time fluid on every side.
pub(super) fn fluid_tanks(entity: &ModBlockEntity, tanks: &mut Option<Vec<NativeFluidTankInfo>>) {
    let amount = int(&entity.nbt, "timeFluid");
    *tanks = Some(vec![NativeFluidTankInfo { stack: time_fluid(amount), capacity: world::settings().paradox.fluid_capacity, drainable: amount > 0 }]);
}

/// `native_fluid_handler_fill`: time fluid only (`JustDireFluidTank`'s filter).
pub(super) fn fluid_fill(entity: &mut ModBlockEntity, resource: &FluidStack, simulate: bool, filled: &mut Option<i32>) {
    if !is_time_fluid(resource) {
        *filled = Some(0);
        return;
    }
    let cfg = world::settings().paradox;
    let mut p = Paradox::new([0, 0, 0]);
    p.fluid = int(&entity.nbt, "timeFluid");
    let took = p.fill(&cfg, i32::try_from(resource.amount).unwrap_or(i32::MAX), simulate);
    if took > 0 && !simulate {
        put(&mut entity.nbt, "timeFluid", p.fluid);
    }
    *filled = Some(took);
}

/// `native_fluid_handler_drain`.
pub(super) fn fluid_drain(entity: &mut ModBlockEntity, resource: Option<&FluidStack>, max: i32, simulate: bool, drained: &mut Option<FluidStack>) {
    if let Some(r) = resource.filter(|r| !is_time_fluid(r)) {
        *drained = Some(r.with_amount(0));
        return;
    }
    let mut p = Paradox::new([0, 0, 0]);
    p.fluid = int(&entity.nbt, "timeFluid");
    let took = p.drain(resource.map_or(max, |r| i32::try_from(r.amount).unwrap_or(i32::MAX)), simulate);
    if took > 0 && !simulate {
        put(&mut entity.nbt, "timeFluid", p.fluid);
    }
    *drained = time_fluid(took).or_else(|| resource.map(|r| r.with_amount(0))).or_else(|| time_fluid(1).map(|s| s.with_amount(0)));
}

/// `ParadoxMachine.useItemOn`: a bucket or another fluid container fills from the tank or empties into it, with
/// the bucket's sound; anything else opens the screen.
pub(super) fn use_item_on(game: &mut Game<'_>, state: BlockStateId, click: &mod_api::BlockClick<'_>, answer: &mut Option<InteractionResult>) {
    if answer.is_some() || game.remote.is_some() || game.native_state_block(state) != Some(PARADOX) || click.stack.is_none() {
        return;
    }
    let face = Direction::ALL.into_iter().find(|d| d.normal() == click.hit.face);
    let dimension = game.native_dimension();
    if game.native_fluid_interact(click.hand, dimension, click.pos, face) {
        *answer = Some(InteractionResult::Success);
    }
}
