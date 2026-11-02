// Mekanism's energy blocks in the game (support module `crate::game::mek_world`
// of the `mekanism` native bundle). The rules are the bundle's crate
// `mekanism_energy` (MIT, from Mekanism 10.7.19, Copyright (c) 2017-2025
// Aidan C. Brady); this module is the glue: the cubes' block entities and
// their saved compound, the ejector's neighbours, Forge Energy at the cube's
// edge (hook points API 81), universal cables and their networks (kept here,
// per dimension, through the block entity lifecycle hook points API 83; each
// level's tick commits them, refreshes connections, pulls, emits and marks their
// chunks changed), the settings from `config/Mekanism/`, the menu's
// data slots and buttons (its screen is `mek_ui`), placing a cube (facing the
// player, with its item's energy and config) and the drop that keeps them, and
// `/mek probe` for tests. Server hooks run on the thread of the `Game` that
// simulates the world, so the settings are kept per thread, read when a world
// opens and dropped when it closes. No borrow of that state is held across a
// call into the game.

use super::{
    Game,
    native_hooks::NativeFeInfo,
    native_lifecycle::WorldPhase,
    native_server::{NativeCommand, NativeServerPlayer},
};
use horizon_client::{
    containers::{ModBlockEntity, ModContainer},
    items::ItemStack,
    text::Text,
};
use horizon_core::geometry::Direction as Face;
use horizon_world::{
    level::flags,
    nbt::{Compound, JavaString, Tag},
};
use mekanism_energy::{
    config::{self, EnergyConfig},
    container::Automation,
    cube::{self, CubeState},
    fe::{self, Conversion, StrictEnergy},
    menu::{Button, View},
    network::{self, Connection, Networks},
    sides::{self, DataType, Direction, RelativeSide},
    tier::{CableTier, CubeTier},
};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeSet, HashMap},
    time::{Duration, Instant},
};

/// The settings of the open world, whether `tiers.toml` was there, and when
/// the files last changed, to read them again when they do.
struct Settings {
    energy: EnergyConfig,
    from_files: bool,
    stamp: [Option<std::time::SystemTime>; 3],
}

thread_local! {
    static SETTINGS: RefCell<Option<Settings>> = const { RefCell::new(None) };
    /// Joules each cube took in since its last tick (`LastEnergyTracker`), by position.
    static RECEIVED: RefCell<HashMap<[i32; 3], u64>> = RefCell::new(HashMap::new());
    /// Cubes a player is breaking: what their dropped item takes.
    static DROPS: RefCell<Vec<Drop>> = const { RefCell::new(Vec::new()) };
    /// When the config files were last looked at.
    static LOOKED_AT: Cell<Option<Instant>> = const { Cell::new(None) };
    /// The cables of each open dimension. Never borrowed across a call into
    /// the game: FE hook points may come back here from inside one.
    static CABLES: RefCell<HashMap<String, DimCables>> = RefCell::new(HashMap::new());
    /// Whether any cable is loaded, so `block_set` costs nothing without.
    static ANY_CABLES: Cell<bool> = const { Cell::new(false) };
    /// `/mek` answers so far, each carrying its number.
    static ANSWERS: Cell<u64> = const { Cell::new(0) };
    /// Blocks set since the last level tick, for the cables beside them.
    static SET_BLOCKS: RefCell<Vec<[i32; 3]>> = const { RefCell::new(Vec::new()) };
}

/// A dimension's cables.
#[derive(Default)]
struct DimCables {
    nets: Networks,
    /// Cables whose connections need a look.
    look: BTreeSet<[i32; 3]>,
    /// Ticks until every cable gets a look, as a neighbour's energy can come and
    /// go with no block changing (an AE2 part, a cube's data).
    full_look_in: u32,
    /// `native_block_entity_load` and `_unload` of cables, for `/mek probe`.
    loads: u64,
    unloads: u64,
}

/// `change` on the dimension's cables. Holds the borrow: no call into the game
/// inside.
fn with_cables<R>(dimension: &str, change: impl FnOnce(&mut DimCables) -> R) -> R {
    CABLES.with(|c| {
        let mut c = c.borrow_mut();
        if !c.contains_key(dimension) {
            c.insert(dimension.to_owned(), DimCables::default());
        }
        change(c.get_mut(dimension).expect("just made"))
    })
}

/// A broken cube's position, its item and the `mekanism` data the item takes,
/// kept until the drop joins the level (a few ticks at most).
#[derive(Clone)]
struct Drop {
    pos: [i32; 3],
    item: String,
    data: Compound,
    age: u8,
}

fn read_settings() -> Settings {
    match Game::native_config_dir() {
        Ok(dir) => Settings {
            stamp: config::stamp(&dir),
            energy: EnergyConfig::load(&dir),
            from_files: dir.join("Mekanism").join("tiers.toml").is_file(),
        },
        Err(e) => {
            log::warn!("[mekanism] no config folder ({e}): Mekanism's defaults");
            Settings { energy: EnergyConfig::default(), from_files: false, stamp: [None; 3] }
        }
    }
}

/// The energy settings, read on first use if the world's opening didn't.
fn energy_config() -> (EnergyConfig, bool) {
    SETTINGS.with(|s| {
        let mut s = s.borrow_mut();
        let s = s.get_or_insert_with(read_settings);
        (s.energy.clone(), s.from_files)
    })
}

/// `native_world_lifecycle`: settings are read as a world opens (ATM10's
/// files, copied by `pack/scripts/sync-atm10.ts`) and dropped as it closes.
pub(super) fn lifecycle(phase: WorldPhase) {
    let fresh = matches!(phase, WorldPhase::WorldOpenReady).then(read_settings);
    SETTINGS.with(|s| *s.borrow_mut() = fresh);
    RECEIVED.with(|r| r.borrow_mut().clear());
    DROPS.with(|d| d.borrow_mut().clear());
    CABLES.with(|c| c.borrow_mut().clear());
    ANY_CABLES.set(false);
    SET_BLOCKS.with(|s| s.borrow_mut().clear());
}

// ---- The saved compound, in Mekanism's names ----------------------------

/// `TransmissionType` ordinals in `component_config`.
const ENERGY: u8 = 0;
const ITEM: u8 = 3;

fn key(name: &str) -> JavaString {
    JavaString::from(name)
}

fn number(tag: Option<&Tag>) -> Option<i64> {
    match tag? {
        Tag::Byte(v) => Some(i64::from(*v)),
        Tag::Short(v) => Some(i64::from(*v)),
        Tag::Int(v) => Some(i64::from(*v)),
        Tag::Long(v) => Some(*v),
        // Mekanism's old FloatingLong saves are strings.
        Tag::String(s) => s.to_string_lossy().split('.').next()?.parse().ok(),
        _ => None,
    }
}

/// `energy_containers`: `[{container: 0b, stored: 4000L}]`, the container
/// left out when empty (`DataHandlerUtils.writeContents`).
pub(super) fn saved_energy(nbt: &Compound) -> Option<u64> {
    let Some(Tag::List(list)) = nbt.get(&key("energy_containers")) else {
        return None;
    };
    list.iter().find_map(|entry| match entry {
        Tag::Compound(c) if number(c.get(&key("container"))).unwrap_or(0) == 0 => {
            Some(number(c.get(&key("stored"))).unwrap_or(0).max(0) as u64)
        }
        _ => None,
    })
}

fn write_energy(nbt: &mut Compound, stored: u64) {
    if stored == 0 {
        nbt.remove(&key("energy_containers"));
        return;
    }
    let mut container = Compound::new();
    container.insert(key("container"), Tag::Byte(0));
    container.insert(key("stored"), Tag::Long(stored.min(i64::MAX as u64) as i64));
    nbt.insert(key("energy_containers"), Tag::List(vec![Tag::Compound(container)]));
}

fn sides_of(config: &Compound, kind: u8, into: &mut [DataType; 6]) {
    if let Some(Tag::IntArray(ids)) = config.get(&key(&format!("config{kind}"))) {
        for (side, id) in into.iter_mut().zip(ids) {
            *side = DataType::by_id(*id);
        }
    }
}

/// The cube in `nbt`; what is missing keeps a new cube's value.
fn read_cube(nbt: &Compound, tier: CubeTier, capacity: u64) -> CubeState {
    let mut cube = CubeState::new(tier, capacity);
    cube.stored = cube::stored_from(tier, saved_energy(nbt), capacity);
    if let Some(Tag::String(facing)) = nbt.get(&key("facing"))
        && let Some(d) = Direction::from_name(&facing.to_string_lossy())
    {
        cube.facing = d;
    }
    if let Some(Tag::Compound(config)) = nbt.get(&key("component_config")) {
        sides_of(config, ENERGY, &mut cube.energy);
        sides_of(config, ITEM, &mut cube.items);
        if let Some(v) = number(config.get(&key(&format!("eject{ENERGY}")))) {
            cube.eject = v != 0;
        }
        if let Some(v) = number(config.get(&key(&format!("eject{ITEM}")))) {
            cube.items_eject = v != 0;
        }
    }
    cube
}

/// `facing` and `component_config` (`TileComponentConfig.write`).
fn write_config(nbt: &mut Compound, cube: &CubeState) {
    nbt.insert(key("facing"), Tag::String(key(cube.facing.name())));
    let mut config = Compound::new();
    for (kind, sides, eject) in [(ENERGY, &cube.energy, cube.eject), (ITEM, &cube.items, cube.items_eject)] {
        config.insert(key(&format!("eject{kind}")), Tag::Byte(i8::from(eject)));
        config.insert(key(&format!("config{kind}")), Tag::IntArray(sides.iter().map(|t| t.id()).collect()));
    }
    nbt.insert(key("component_config"), Tag::Compound(config));
}

// ---- Block entities -------------------------------------------------------

/// `native_block_entity`: a cube's entity, with the item's default config.
/// A creative cube saves no energy, so it counts as full.
pub(super) fn new_entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if entity.is_some() {
        return;
    }
    if CableTier::of_block(block).is_some() {
        let mut nbt = Compound::new();
        write_connections(&mut nbt, &[Connection::Normal; 6]);
        *entity = Some((block.to_owned(), nbt));
        return;
    }
    let Some(tier) = CubeTier::of_block(block) else {
        return;
    };
    let mut nbt = Compound::new();
    write_config(&mut nbt, &CubeState::new(tier, 0));
    *entity = Some((block.to_owned(), nbt));
}

/// `native_block_entity_ticks`: cubes tick.
pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if CubeTier::of_block(block).is_some() {
        *ticks = true;
    }
}

/// `native_block_entity_container`: the discharge slot (0, fills the cube
/// from an item) and the charge slot (1), in a hopper's five-slot menu whose
/// other three slots the screen leaves out. Every face reaches both; hoppers
/// put into the charge slot and take from both.
pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if container.is_some() || CubeTier::of_block(block).is_none() {
        return;
    }
    *container = Some(ModContainer {
        slots: 5,
        faces: std::array::from_fn(|_| vec![0, 1]),
        inputs: vec![1],
        outputs: vec![0, 1],
        menu: Some("minecraft:hopper".into()),
    });
}

fn beside(pos: [i32; 3], face: Direction) -> [i32; 3] {
    let [dx, dy, dz] = face.offset();
    [pos[0] + dx, pos[1] + dy, pos[2] + dz]
}

fn add_received(pos: [i32; 3], joules: u64) {
    RECEIVED.with(|r| {
        let mut r = r.borrow_mut();
        let sum = r.entry(pos).or_default();
        *sum = sum.saturating_add(joules);
    });
}

/// What takes energy on an output face.
#[derive(Clone, Copy)]
enum Target {
    /// Another cube, in Joules.
    Cube,
    /// A cable, whose network takes Joules.
    Cable,
    /// Any other block with Forge Energy on the touching side, as Mekanism
    /// sees it through `ForgeStrictEnergyHandler`.
    Fe { can_receive: bool },
}

/// What takes energy on `face` of `pos`: a cube whose touching side isn't
/// `NONE`, or a block with Forge Energy there (none with `blacklistForge`).
fn target_on(game: &mut Game<'_>, config: &EnergyConfig, dimension: &str, pos: [i32; 3], face: Direction) -> Option<Target> {
    let at = beside(pos, face);
    let side = face.opposite();
    let cable = with_cables(dimension, |d| d.nets.cable(at).map(|_| d.nets.exposed(at, Some(side as usize)).is_some()));
    if let Some(open) = cable {
        return open.then_some(Target::Cable);
    }
    let cube = game
        .native_with_mod_block_entity(at, |m| {
            let tier = CubeTier::of_block(&m.id)?;
            Some(read_cube(&m.nbt, tier, config.cube(tier).0).exposes(side))
        })
        .flatten();
    match cube {
        Some(exposed) => exposed.then_some(Target::Cube),
        None if config.blacklist_forge => None,
        None => {
            let info = game.native_fe_info(dimension, at, Some(engine_face(side)), false)?;
            Some(Target::Fe { can_receive: info.can_receive })
        }
    }
}

/// Offers `amount` Joules to the cube on `face` of `pos`: what it didn't take.
fn offer(game: &mut Game<'_>, config: &EnergyConfig, pos: [i32; 3], face: Direction, amount: u64, simulate: bool) -> u64 {
    let rest = game.native_with_mod_block_entity(beside(pos, face), |m| {
        let tier = CubeTier::of_block(&m.id)?;
        let (capacity, output) = config.cube(tier);
        let mut cube = read_cube(&m.nbt, tier, capacity);
        let before = cube.stored;
        let rest = cube.receive(face.opposite(), amount, simulate, capacity, output)?;
        if cube.stored != before {
            write_energy(&mut m.nbt, cube.stored);
        }
        Some(rest)
    })
    .flatten()
    .unwrap_or(amount);
    if !simulate && rest < amount {
        add_received(beside(pos, face), amount - rest);
    }
    rest
}

/// Offers `amount` Joules to what stands on `face` of `pos`, a `target_on`:
/// what it didn't take, as `IStrictEnergyHandler.insertEnergy`.
#[allow(clippy::too_many_arguments)]
fn hand_over(
    game: &mut Game<'_>,
    config: &EnergyConfig,
    rate: Conversion,
    dimension: &str,
    pos: [i32; 3],
    face: Direction,
    target: Target,
    amount: u64,
    simulate: bool,
) -> u64 {
    let at = beside(pos, face);
    match target {
        Target::Cube => offer(game, config, pos, face, amount, simulate),
        Target::Cable => with_cables(dimension, |d| d.nets.insert_at(at, Some(face.opposite() as usize), amount, simulate)),
        Target::Fe { can_receive } => {
            let side = Some(engine_face(face.opposite()));
            let joules = amount.min(i64::MAX as u64) as i64;
            let rest = fe::insert_into_fe(joules, rate, can_receive, simulate, |fe, simulate| {
                game.native_fe_offer(dimension, at, side, i64::from(fe), simulate, false) as i32
            });
            rest.max(0) as u64
        }
    }
}

/// `native_block_entity_tick` of a cube (`TileEntityEnergyCube.onUpdateServer`):
/// the ejector (`CableUtils.emit`) pushes up to the tier's output out of the
/// output faces, split among the neighbours that take it, cubes and Forge
/// Energy blocks alike; then the menu's data slots. Redstone control is
/// IGNORED, the default; the slots hold no energy items yet.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let Some(tier) = CubeTier::of_block(&entity.id) else {
        return;
    };
    let (config, _) = energy_config();
    let (capacity, output) = config.cube(tier);
    let mut cube = read_cube(&entity.nbt, tier, capacity);
    if cube.eject && cube.stored > 0 {
        let before = cube.stored;
        let dimension = game.native_dimension();
        let targets: Vec<(Direction, Target)> = cube
            .output_faces()
            .into_iter()
            .filter_map(|face| Some((face, target_on(game, &config, dimension, pos, face)?)))
            .collect();
        let rate = Conversion(config.fe_conversion_rate);
        let mut container = cube.container(capacity, output);
        cube::emit(&mut container, output, targets.len(), |i, amount, simulate| {
            let (face, target) = targets[i];
            hand_over(game, &config, rate, dimension, pos, face, target, amount, simulate)
        });
        cube.stored = container.stored;
        if cube.stored != before {
            write_energy(&mut entity.nbt, cube.stored);
        }
    }
    let input = RECEIVED.with(|r| r.borrow_mut().remove(&pos)).unwrap_or(0);
    entity.data = View::of(&cube, capacity, output, input).pack();
}

/// The Joules the cube last counted as taken in, from the menu's data slots.
fn last_input(entity: &ModBlockEntity) -> u64 {
    View::unpack(&entity.data).map_or(0, |view| view.input)
}

// ---- Forge Energy (hook points API 81) --------------------------------------

fn mek_direction(face: Face) -> Direction {
    Direction::ALL[face as usize]
}

fn engine_face(direction: Direction) -> Face {
    Face::ALL[direction as usize]
}

/// A cube's energy as one side reaches it (`ProxyStrictEnergyHandler`): an
/// input side only takes, an output side only gives, and the block itself (a
/// null side) neither, as Mekanism's proxy for it is read-only.
struct Side<'a> {
    cube: &'a mut CubeState,
    face: Option<Direction>,
    capacity: u64,
    output: u64,
    /// Joules taken for real (`trackLastEnergy`).
    taken: u64,
}

impl Side<'_> {
    fn io(&self) -> (bool, bool) {
        self.face.and_then(|d| sides::io(self.cube.energy_at(d))).unwrap_or((false, false))
    }
}

impl StrictEnergy for Side<'_> {
    fn insert(&mut self, joules: i64, simulate: bool) -> i64 {
        if !self.io().0 || joules <= 0 {
            return joules;
        }
        let mut container = self.cube.container(self.capacity, self.output);
        let rest = container.insert(joules as u64, simulate, Automation::External);
        self.cube.stored = container.stored;
        if !simulate {
            self.taken = self.taken.saturating_add(joules as u64 - rest);
        }
        rest as i64
    }

    fn extract(&mut self, joules: i64, simulate: bool) -> i64 {
        if !self.io().1 || joules <= 0 {
            return 0;
        }
        let mut container = self.cube.container(self.capacity, self.output);
        let given = container.extract(joules as u64, simulate, Automation::External);
        self.cube.stored = container.stored;
        given as i64
    }
}

/// The cube in `entity` and its tier's size, when it is one and `face`
/// exposes its energy: no FE with `blacklistForge`, none on a `NONE` side.
fn exposed(entity: &ModBlockEntity, face: Option<Face>, config: &EnergyConfig) -> Option<(CubeState, u64, u64)> {
    let tier = CubeTier::of_block(&entity.id)?;
    if config.blacklist_forge {
        return None;
    }
    let (capacity, output) = config.cube(tier);
    let cube = read_cube(&entity.nbt, tier, capacity);
    if let Some(f) = face
        && !cube.exposes(mek_direction(f))
    {
        return None;
    }
    Some((cube, capacity, output))
}

/// `native_fe_storage`: `ForgeEnergyIntegration`'s readback, in whole FE;
/// Mekanism answers `canReceive` and `canExtract` true on every side it
/// exposes.
pub(super) fn fe_storage(
    dimension: &str,
    pos: [i32; 3],
    face: Option<Face>,
    entity: &ModBlockEntity,
    info: &mut Option<NativeFeInfo>,
) {
    if info.is_some() {
        return;
    }
    let (config, _) = energy_config();
    if CableTier::of_block(&entity.id).is_some() {
        if config.blacklist_forge {
            return;
        }
        let rate = Conversion(config.fe_conversion_rate);
        let side = face.map(|f| mek_direction(f) as usize);
        *info = with_cables(dimension, |d| {
            let network = d.nets.exposed(pos, side)?;
            Some(NativeFeInfo {
                stored: i64::from(rate.to_fe_int(network.buffer.min(i64::MAX as u64) as i64)),
                capacity: i64::from(rate.to_fe_int(network.capacity.min(i64::MAX as u64) as i64)),
                can_receive: true,
                can_extract: true,
            })
        });
        return;
    }
    let Some((cube, capacity, _)) = exposed(entity, face, &config) else {
        return;
    };
    let rate = Conversion(config.fe_conversion_rate);
    *info = Some(NativeFeInfo {
        stored: i64::from(rate.to_fe_int(cube.stored.min(i64::MAX as u64) as i64)),
        capacity: i64::from(rate.to_fe_int(capacity.min(i64::MAX as u64) as i64)),
        can_receive: true,
        can_extract: true,
    });
}

/// `native_fe_receive` and `native_fe_extract`: `ForgeEnergyIntegration`'s
/// `receiveEnergy` and `extractEnergy` on the side's handler. What it takes
/// counts as the cube's input, as what another cube gives it does.
pub(super) fn fe_transfer(
    dimension: &str,
    pos: [i32; 3],
    face: Option<Face>,
    entity: &mut ModBlockEntity,
    max: i64,
    simulate: bool,
    extract: bool,
    moved: &mut Option<i64>,
) {
    if moved.is_some() {
        return;
    }
    let (config, _) = energy_config();
    if CableTier::of_block(&entity.id).is_some() {
        if config.blacklist_forge {
            return;
        }
        let rate = Conversion(config.fe_conversion_rate);
        let side = face.map(|f| mek_direction(f) as usize);
        let fe = with_cables(dimension, |d| {
            d.nets.exposed(pos, side)?;
            let mut handler = CableSide { nets: &mut d.nets, pos, side };
            let max = max.clamp(0, i64::from(i32::MAX)) as i32;
            Some(if extract {
                fe::extract_fe(&mut handler, rate, max, simulate)
            } else {
                fe::receive_fe(&mut handler, rate, max, simulate)
            })
        });
        if let Some(fe) = fe {
            *moved = Some(i64::from(fe));
        }
        return;
    }
    let Some((mut cube, capacity, output)) = exposed(entity, face, &config) else {
        return;
    };
    let rate = Conversion(config.fe_conversion_rate);
    let before = cube.stored;
    let mut side = Side { cube: &mut cube, face: face.map(mek_direction), capacity, output, taken: 0 };
    let max = max.clamp(0, i64::from(i32::MAX)) as i32;
    let fe = if extract {
        fe::extract_fe(&mut side, rate, max, simulate)
    } else {
        fe::receive_fe(&mut side, rate, max, simulate)
    };
    let taken = side.taken;
    if !simulate && cube.stored != before {
        write_energy(&mut entity.nbt, cube.stored);
    }
    if taken > 0 {
        add_received(pos, taken);
    }
    *moved = Some(i64::from(fe));
}

/// A cable's energy as one side reaches it (`DynamicStrictEnergyHandler` over
/// its network's container).
struct CableSide<'a> {
    nets: &'a mut Networks,
    pos: [i32; 3],
    side: Option<usize>,
}

impl StrictEnergy for CableSide<'_> {
    fn insert(&mut self, joules: i64, simulate: bool) -> i64 {
        if joules <= 0 {
            return joules;
        }
        self.nets.insert_at(self.pos, self.side, joules as u64, simulate) as i64
    }

    fn extract(&mut self, joules: i64, simulate: bool) -> i64 {
        if joules <= 0 {
            return 0;
        }
        self.nets.extract_at(self.pos, self.side, joules as u64, simulate) as i64
    }
}

// ---- Universal cables (hook points API 83) ----------------------------------

/// `connection`: the raw `ConnectionType` ordinal of each side, by `Direction`.
fn connections_of(nbt: &Compound) -> [Connection; 6] {
    let mut out = [Connection::Normal; 6];
    if let Some(Tag::IntArray(ids)) = nbt.get(&key("connection")) {
        for (side, id) in out.iter_mut().zip(ids) {
            *side = Connection::by_id(*id);
        }
    }
    out
}

fn write_connections(nbt: &mut Compound, connections: &[Connection; 6]) {
    nbt.insert(key("connection"), Tag::IntArray(connections.iter().map(|c| c.id()).collect()));
}

/// `native_block_entity_load`: a cable placed or loaded queues to join with
/// the share it saved (`registerOrphanTransmitter`); the cables around a cube
/// that came look at it again.
pub(super) fn entity_loaded(dimension: &str, pos: [i32; 3], entity: &ModBlockEntity) {
    let Some(tier) = CableTier::of_block(&entity.id) else {
        if CubeTier::of_block(&entity.id).is_some() && ANY_CABLES.get() {
            with_cables(dimension, |d| d.look.extend((0..6).map(|s| network::offset(pos, s))));
        }
        return;
    };
    let (config, _) = energy_config();
    let share = number(entity.nbt.get(&key("energy"))).unwrap_or(0).max(0) as u64;
    with_cables(dimension, |d| {
        d.nets.queue_add(pos, config.cable(tier), connections_of(&entity.nbt), share);
        d.loads += 1;
    });
    ANY_CABLES.set(true);
}

/// `native_block_entity_unload`: a cable leaves its network at the next commit.
/// Unloading, it takes what its save holds with it (`onChunkUnloaded`); broken,
/// it takes nothing and the rest of the network keeps the energy.
pub(super) fn entity_unloaded(dimension: &str, pos: [i32; 3], entity: &ModBlockEntity, removed: bool) {
    if CableTier::of_block(&entity.id).is_none() {
        if CubeTier::of_block(&entity.id).is_some() && ANY_CABLES.get() {
            with_cables(dimension, |d| d.look.extend((0..6).map(|s| network::offset(pos, s))));
        }
        return;
    }
    with_cables(dimension, |d| {
        if !removed {
            d.nets.take_share(pos);
        }
        d.nets.queue_remove(pos);
        d.unloads += 1;
    });
}

/// `native_block_entity_save`: a cable's share goes in `energy`
/// (`UniversalCable.write`), left out at zero.
pub(super) fn entity_saved(dimension: &str, pos: [i32; 3], id: &str, nbt: &mut Compound) {
    if CableTier::of_block(id).is_none() {
        return;
    }
    let share = with_cables(dimension, |d| d.nets.save_share(pos));
    match share {
        Some(0) => {
            nbt.remove(&key("energy"));
        }
        Some(share) => {
            nbt.insert(key("energy"), Tag::Long(share.min(i64::MAX as u64) as i64));
        }
        None => {}
    }
}

/// `native_block_changed`: every block set; the cables beside it look again.
pub(super) fn block_set(pos: [i32; 3]) {
    if ANY_CABLES.get() {
        SET_BLOCKS.with(|s| s.borrow_mut().push(pos));
    }
}

/// `Transmitter.refreshConnections`: which sides of the cable at `pos` have a
/// block with energy, and what the block shows. A cable beside it is the
/// network's business, as `isValidAcceptor` says.
fn refresh_cable(game: &mut Game<'_>, config: &EnergyConfig, dimension: &str, pos: [i32; 3]) {
    let Some(raw) = with_cables(dimension, |d| d.nets.cable(pos).map(|c| c.connections)) else {
        return;
    };
    for face in Direction::ALL {
        let side = face as usize;
        let at = network::offset(pos, side);
        let neighbour_cable = with_cables(dimension, |d| d.nets.cable(at).is_some());
        let acceptor = raw[side] != Connection::None
            && !neighbour_cable
            && target_on(game, config, dimension, pos, face).is_some();
        let shown = with_cables(dimension, |d| {
            d.nets.set_acceptor(pos, side, acceptor);
            d.nets.shown(pos, side)
        });
        if game.native_block_property(pos, face.name()).as_deref() != Some(shown.name()) {
            game.native_set_block_property_with(pos, face.name(), shown.name(), flags::UPDATE_CLIENTS);
        }
    }
}

/// What the block on `face` of the cable at `pos` would give (`extractEnergy`).
fn draw(game: &mut Game<'_>, config: &EnergyConfig, dimension: &str, pos: [i32; 3], face: Direction, amount: u64, simulate: bool) -> u64 {
    let at = beside(pos, face);
    let side = face.opposite();
    let cube = game
        .native_with_mod_block_entity(at, |m| {
            let tier = CubeTier::of_block(&m.id)?;
            let (capacity, output) = config.cube(tier);
            let mut cube = read_cube(&m.nbt, tier, capacity);
            let before = cube.stored;
            let mut handler = Side { cube: &mut cube, face: Some(side), capacity, output, taken: 0 };
            let given = handler.extract(amount.min(i64::MAX as u64) as i64, simulate).max(0) as u64;
            if !simulate && cube.stored != before {
                write_energy(&mut m.nbt, cube.stored);
            }
            Some(given)
        })
        .flatten();
    if let Some(given) = cube {
        return given;
    }
    if config.blacklist_forge {
        return 0;
    }
    let engine = Some(engine_face(side));
    let Some(info) = game.native_fe_info(dimension, at, engine, false) else {
        return 0;
    };
    let rate = Conversion(config.fe_conversion_rate);
    let joules = amount.min(i64::MAX as u64) as i64;
    fe::extract_from_fe(joules, rate, info.can_extract, simulate, |fe, simulate| {
        game.native_fe_draw(dimension, at, engine, i64::from(fe), simulate, false) as i32
    })
    .max(0) as u64
}

/// `UniversalCable.pullFromAcceptors` of every pull side in the dimension.
fn pull(game: &mut Game<'_>, config: &EnergyConfig, dimension: &str) {
    let sides: Vec<([i32; 3], usize)> = with_cables(dimension, |d| {
        d.nets.ids().into_iter().flat_map(|id| d.nets.pulls(id)).collect()
    });
    for (pos, side) in sides {
        let face = Direction::ALL[side];
        let room = with_cables(dimension, |d| d.nets.available_pull(pos));
        if room == 0 {
            continue;
        }
        let received = draw(game, config, dimension, pos, face, room, true);
        let fits = received > 0 && with_cables(dimension, |d| d.nets.insert(pos, received, true)) == 0;
        if fits {
            let rest = with_cables(dimension, |d| d.nets.insert(pos, received, false));
            draw(game, config, dimension, pos, face, received - rest, false);
        }
    }
}

/// `EnergyNetwork.onUpdate` of every network with energy: `tickEmit`. No borrow
/// of the networks is held while it goes out, as any target may come back.
fn emit_networks(game: &mut Game<'_>, config: &EnergyConfig, dimension: &str) {
    let rate = Conversion(config.fe_conversion_rate);
    for id in with_cables(dimension, |d| d.nets.ids()) {
        let Some(plan) = with_cables(dimension, |d| d.nets.begin_emit(id)) else {
            continue;
        };
        let targets: Vec<(([i32; 3], usize), Target)> = plan
            .targets
            .iter()
            .filter_map(|&(pos, side)| Some(((pos, side), target_on(game, config, dimension, pos, Direction::ALL[side])?)))
            .collect();
        let sides: Vec<([i32; 3], usize)> = targets.iter().map(|(t, _)| *t).collect();
        let lookup: HashMap<([i32; 3], usize), Target> = targets.iter().copied().collect();
        let sent = network::send(plan.buffer, &sides, |t, amount, simulate| {
            hand_over(game, config, rate, dimension, t.0, Direction::ALL[t.1], lookup[&t], amount, simulate)
        });
        with_cables(dimension, |d| d.nets.finish_emit(id, sent));
    }
}

/// `native_level_tick_post`: the dimension's cables, as Mekanism's
/// `TransmitterNetworkRegistry` runs after a tick (and `pullFromAcceptors`,
/// which it runs in the cables' own ticks).
fn tick_cables(game: &mut Game<'_>, dimension: &str) {
    let set: Vec<[i32; 3]> = SET_BLOCKS.with(|s| std::mem::take(&mut *s.borrow_mut()));
    let looks = with_cables(dimension, |d| {
        if d.nets.is_empty() {
            d.look.clear();
            return None;
        }
        let around = d.nets.commit();
        d.look.extend(around);
        for pos in &set {
            d.look.extend((0..6).map(|s| network::offset(*pos, s)));
        }
        if d.full_look_in == 0 {
            d.full_look_in = 20;
            let all: Vec<[i32; 3]> = d.nets.cable_positions().collect();
            d.look.extend(all);
        }
        d.full_look_in -= 1;
        let nets = &d.nets;
        d.look.retain(|p| nets.cable(*p).is_some());
        Some(std::mem::take(&mut d.look))
    });
    let Some(looks) = looks else {
        return;
    };
    let (config, _) = energy_config();
    for pos in looks {
        refresh_cable(game, &config, dimension, pos);
    }
    pull(game, &config, dimension);
    emit_networks(game, &config, dimension);
    for pos in with_cables(dimension, |d| d.nets.take_changed()) {
        game.native_block_entity_changed(pos);
    }
}

/// `/mek connection x y z <side> <normal|push|pull|none>`, the Configurator's
/// side setting until it is ported: changes the side and saves it.
fn set_connection(game: &mut Game<'_>, pos: [i32; 3], side: &str, connection: &str) -> serde_json::Value {
    let Some(face) = Direction::from_name(side) else {
        return serde_json::json!({ "error": "no such side" });
    };
    let Some(connection) = Connection::from_name(connection) else {
        return serde_json::json!({ "error": "no such connection" });
    };
    let dimension = game.native_dimension();
    let written = game
        .native_with_mod_block_entity(pos, |m| {
            CableTier::of_block(&m.id)?;
            let mut connections = connections_of(&m.nbt);
            connections[face as usize] = connection;
            write_connections(&mut m.nbt, &connections);
            Some(())
        })
        .flatten();
    if written.is_none() {
        return serde_json::json!({ "pos": pos, "error": "not a cable" });
    }
    let (config, _) = energy_config();
    with_cables(dimension, |d| {
        d.nets.set_connection(pos, face as usize, connection);
        d.look.insert(pos);
        d.look.insert(network::offset(pos, face as usize));
    });
    refresh_cable(game, &config, dimension, pos);
    probe(game, pos)
}

/// What `/mek probe` answers for a cable.
fn probe_cable(game: &mut Game<'_>, pos: [i32; 3], id: &str, tier: CableTier) -> serde_json::Value {
    let dimension = game.native_dimension();
    let faces: serde_json::Map<String, serde_json::Value> = Direction::ALL
        .iter()
        .map(|&d| (d.name().to_owned(), game.native_block_property(pos, d.name()).unwrap_or_default().into()))
        .collect();
    let (config, _) = energy_config();
    with_cables(dimension, |d| {
        let network = d.nets.network_of(pos).map(|(id, n)| {
            serde_json::json!({
                "id": id,
                "cables": n.cables.len(),
                "buffer": n.buffer,
                "capacity": n.capacity,
                "last_sent": n.last_sent,
            })
        });
        let raw: Vec<&str> = d.nets.cable(pos).map_or(Vec::new(), |c| c.connections.iter().map(|c| c.name()).collect());
        serde_json::json!({
            "pos": pos,
            "id": id,
            "tier": tier.name(),
            "capacity": config.cable(tier),
            "network": network,
            "share": d.nets.cable(pos).map(|c| c.share),
            "connections": raw,
            "faces": faces,
            "loads": d.loads,
            "unloads": d.unloads,
            "networks": d.nets.networks().count(),
        })
    })
}

/// `native_menu_button`: a side configuration button of the cube's screen
/// (`mek_ui`). A changed energy side shows on the block at once: the face's
/// property is set with `UPDATE_CLIENTS`, as Mekanism updates its model.
pub(super) fn button(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, id: i32) {
    let (Some(tier), Some(button)) = (CubeTier::of_block(&entity.id), Button::from_id(id)) else {
        return;
    };
    let (config, _) = energy_config();
    let (capacity, output) = config.cube(tier);
    let mut cube = read_cube(&entity.nbt, tier, capacity);
    let before = cube.clone();
    let faces = cube.press(button);
    if cube == before {
        return;
    }
    write_config(&mut entity.nbt, &cube);
    if faces {
        show_faces(game, pos, &cube);
    }
    let input = last_input(entity);
    entity.data = View::of(&cube, capacity, output, input).pack();
}

/// The six face properties the model shows, with `UPDATE_CLIENTS`, as
/// Mekanism updates its model data.
fn show_faces(game: &mut Game<'_>, pos: [i32; 3], cube: &CubeState) {
    for face in Direction::ALL {
        game.native_set_block_property_with(pos, face.name(), cube.side_state(face).name(), flags::UPDATE_CLIENTS);
    }
}

// ---- Items: a placed cube and its drop keep its energy and config ---------

const CUSTOM_DATA: &str = "minecraft:custom_data";

/// The `mekanism` compound of a stack's custom data, where a cube's item
/// keeps its saved energy and config (Mekanism's own item components, which
/// the game doesn't take).
pub(super) fn item_data(stack: &ItemStack) -> Option<Compound> {
    let Tag::Compound(data) = stack.data()?.other(CUSTOM_DATA)? else {
        return None;
    };
    match data.get(&key("mekanism"))? {
        Tag::Compound(c) => Some(c.clone()),
        _ => None,
    }
}

/// What a cube's item keeps of its block entity: its energy, and its side
/// config when it isn't the default one, so an untouched cube's drop stacks
/// with new ones (Mekanism's copied components equal the item's defaults).
fn drop_data(nbt: &Compound, tier: CubeTier, capacity: u64) -> Option<Compound> {
    let mut data = Compound::new();
    if let Some(energy) = nbt.get(&key("energy_containers")) {
        data.insert(key("energy_containers"), energy.clone());
    }
    let cube = read_cube(nbt, tier, capacity);
    let fresh = CubeState::new(tier, capacity);
    let changed = (cube.energy, cube.eject, cube.items, cube.items_eject) != (fresh.energy, fresh.eject, fresh.items, fresh.items_eject);
    if changed && let Some(config) = nbt.get(&key("component_config")) {
        data.insert(key("component_config"), config.clone());
    }
    (!data.is_empty()).then_some(data)
}

/// `native_place_block`: a cube faces the player who placed it
/// (`AttributeStateFacing`), takes its item's energy and config, and shows
/// its faces.
pub(super) fn placed(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3]) {
    let Some(tier) = CubeTier::of_block(&stack.item) else {
        return;
    };
    let facing = game.native_player().map_or(Direction::North, |p| sides::placement_facing(p.yaw, p.pitch));
    let saved = item_data(stack);
    let (config, _) = energy_config();
    let capacity = config.cube(tier).0;
    let cube = game.native_with_mod_block_entity(pos, |m| {
        (CubeTier::of_block(&m.id) == Some(tier)).then(|| {
            for name in ["energy_containers", "component_config"] {
                if let Some(tag) = saved.as_ref().and_then(|s| s.get(&key(name))) {
                    m.nbt.insert(key(name), tag.clone());
                }
            }
            let mut cube = read_cube(&m.nbt, tier, capacity);
            cube.facing = facing;
            write_config(&mut m.nbt, &cube);
            cube
        })
    });
    if let Some(Some(cube)) = cube {
        show_faces(game, pos, &cube);
    }
}

/// `native_destroy_block`: a player breaks a cube; its drop, when it comes,
/// takes the cube's energy and config (Mekanism's `copy_components` loot).
pub(super) fn breaking(game: &mut Game<'_>, pos: [i32; 3]) {
    let (config, _) = energy_config();
    let found = game.native_with_mod_block_entity(pos, |m| {
        let tier = CubeTier::of_block(&m.id)?;
        Some((m.id.clone(), drop_data(&m.nbt, tier, config.cube(tier).0)?))
    });
    if let Some(Some((item, data))) = found {
        DROPS.with(|d| d.borrow_mut().push(Drop { pos, item, data, age: 0 }));
    }
}

/// `native_entity_join_level`: the item a broken cube drops gets its data.
pub(super) fn joined(game: &mut Game<'_>, dimension: &str, entity: u32, kind: &str, at: [f64; 3], from_disk: bool) {
    if from_disk || kind != "minecraft:item" || DROPS.with(|d| d.borrow().is_empty()) {
        return;
    }
    let near = |p: [i32; 3]| (0..3).all(|i| (at[i] - (f64::from(p[i]) + 0.5)).abs() <= 1.0);
    let Some(drop) = DROPS.with(|d| d.borrow().iter().find(|d| near(d.pos)).cloned()) else {
        return;
    };
    let min = [at[0] - 0.5, at[1] - 0.5, at[2] - 0.5];
    let max = [at[0] + 0.5, at[1] + 0.5, at[2] + 0.5];
    let Some(item) = game.native_item_entities(dimension, min, max).into_iter().find(|e| e.id == entity) else {
        return;
    };
    if *item.stack.item != *drop.item || item_data(&item.stack).is_some() {
        return;
    }
    let mut stack = item.stack;
    let mut custom = Compound::new();
    custom.insert(key("mekanism"), Tag::Compound(drop.data));
    stack.data_mut().set_other(CUSTOM_DATA, Some(Tag::Compound(custom)));
    if game.native_set_item_entity(dimension, entity, stack) {
        DROPS.with(|d| d.borrow_mut().retain(|d| d.pos != drop.pos));
    }
}

/// `native_level_tick_post` (once per level): a break that dropped nothing
/// (creative mode, a canceled break) is forgotten after a few ticks; and once
/// a second the config files are looked at, and read again when one changed,
/// as Mekanism's configs reload live.
pub(super) fn level_ticked(game: &mut Game<'_>, dimension: &str) {
    tick_cables(game, dimension);
    let due = LOOKED_AT.get().is_none_or(|at| at.elapsed() >= Duration::from_secs(1));
    if due {
        LOOKED_AT.set(Some(Instant::now()));
        let changed = Game::native_config_dir()
            .ok()
            .is_some_and(|dir| SETTINGS.with(|s| s.borrow().as_ref().is_some_and(|s| s.stamp != config::stamp(&dir))));
        if changed {
            log::info!("[mekanism] config/Mekanism changed: read again");
            let fresh = read_settings();
            SETTINGS.with(|s| *s.borrow_mut() = Some(fresh));
        }
    }
    DROPS.with(|d| {
        let mut d = d.borrow_mut();
        d.retain_mut(|drop| {
            drop.age += 1;
            drop.age < 3
        });
    });
}

// ---- /mek probe -----------------------------------------------------------

/// `native_register_commands`.
pub(super) fn commands(commands: &mut Vec<NativeCommand>) {
    commands.push(NativeCommand::op("mek probe <pos:block_pos>", 2));
    commands.push(NativeCommand::op("mek connection <pos:block_pos> <side:word> <type:word>", 2));
}

/// A coordinate of the command line: a number, or `~`/`~n` from the player.
fn coordinate(word: Option<&str>, from: f64) -> Option<i32> {
    let word = word?;
    match word.strip_prefix('~') {
        Some("") => Some(from.floor() as i32),
        Some(rest) => Some((from + rest.parse::<f64>().ok()?).floor() as i32),
        None => word.parse().ok(),
    }
}

/// `native_perform_command`: `/mek probe x y z` answers `MEK {json}` with
/// what the Mekanism block entity there holds.
pub(super) fn perform(game: &mut Game<'_>, player: &NativeServerPlayer, command: &str, ran: &mut bool) {
    let mut words = command.split_whitespace();
    if *ran || words.next() != Some("mek") {
        return;
    }
    *ran = true;
    let answer = match words.next() {
        Some("probe") => {
            let p = player.position;
            match (coordinate(words.next(), p[0]), coordinate(words.next(), p[1]), coordinate(words.next(), p[2])) {
                (Some(x), Some(y), Some(z)) => probe(game, [x, y, z]),
                _ => serde_json::json!({ "error": "usage: /mek probe <x> <y> <z>" }),
            }
        }
        Some("connection") => {
            let p = player.position;
            let at = (coordinate(words.next(), p[0]), coordinate(words.next(), p[1]), coordinate(words.next(), p[2]));
            match (at, words.next(), words.next()) {
                ((Some(x), Some(y), Some(z)), Some(side), Some(connection)) => set_connection(game, [x, y, z], side, connection),
                _ => serde_json::json!({ "error": "usage: /mek connection <x> <y> <z> <side> <type>" }),
            }
        }
        _ => serde_json::json!({ "error": "usage: /mek probe <x> <y> <z>" }),
    };
    let mut answer = answer;
    let n = ANSWERS.get() + 1;
    ANSWERS.set(n);
    answer["n"] = n.into();
    game.native_feedback(Text::literal(format!("MEK {answer}")));
}

fn probe(game: &mut Game<'_>, pos: [i32; 3]) -> serde_json::Value {
    let (config, from_files) = energy_config();
    let found = game.native_with_mod_block_entity(pos, |m| (m.id.clone(), m.nbt.clone(), last_input(m)));
    let Some((id, nbt, input)) = found else {
        return serde_json::json!({ "pos": pos, "error": "no mod block entity here" });
    };
    if let Some(tier) = CableTier::of_block(&id) {
        return probe_cable(game, pos, &id, tier);
    }
    let Some(tier) = CubeTier::of_block(&id) else {
        return serde_json::json!({ "pos": pos, "id": id, "error": "not a Mekanism block" });
    };
    let (capacity, output) = config.cube(tier);
    let cube = read_cube(&nbt, tier, capacity);
    let faces: serde_json::Map<String, serde_json::Value> = Direction::ALL
        .iter()
        .map(|&d| (d.name().to_owned(), cube.side_state(d).name().into()))
        .collect();
    let energy_config: serde_json::Map<String, serde_json::Value> = RelativeSide::ALL
        .iter()
        .map(|&s| (s.name().to_owned(), cube.energy[s.index()].name().into()))
        .collect();
    serde_json::json!({
        "pos": pos,
        "id": id,
        "tier": tier.name(),
        "energy": cube.stored,
        "capacity": capacity,
        "output": output,
        "input": input,
        "facing": cube.facing.name(),
        "eject": cube.eject,
        "faces": faces,
        "energy_config": energy_config,
        "config": { "from_files": from_files, "fe_conversion_rate": config.fe_conversion_rate },
    })
}
