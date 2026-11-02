// Flux Networks in the game (support module `crate::game::flux_world` of the
// `fluxnetworks` native bundle). The rules are the bundle's crate
// `fluxnetworks_core` (MIT, from Flux Networks 8.0.0, Copyright (c) 2017-2021
// SonarSonic, BloCamLimb); this module is the glue: the devices' block
// entities and their saved compound, the neighbours they reach with Forge
// Energy, the world's `data/fluxnetworksdata.dat`, the chest menu that stands
// in for the mod's GUI, and `/flux` for tests.
//
// Server hooks run on the thread of the `Game` that simulates the world, so
// the world's state is kept per thread, read when a world opens and dropped
// when it closes. The transfer cycle calls into the game for each neighbour and
// the game may call back, so no borrow of that state is held across a call.

use super::{
    Dim, Game, KeyCode,
    native_hooks::NativeFeInfo,
    native_lifecycle::{ServerTickPhase, WorldPhase},
    native_server::{NativeCommand, NativeServerPlayer},
};
use fluxnetworks_core::{
    config::FluxConfig,
    menu::{self, Action, DeviceView, Slot},
    networks::{Access, CreateError, Member, NO_NETWORK, Network, Networks, Security},
    transfer::{Kind, Saved},
    world::{FluxWorld, Host, Key, SIDES, tick_all},
};
use horizon_client::{
    containers::{BlockEntity, ModBlockEntity},
    item_data,
    items::ItemStack,
    lan::data::component_nbt,
    text::Text,
    world::World,
};
use horizon_core::geometry::Direction;
use horizon_world::{
    level::flags,
    nbt::{Compound, JavaString, Tag},
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    hash::{BuildHasher, Hasher},
    path::PathBuf,
    time::{Duration, Instant},
};

/// `SavedData`'s file name, in the world's `data/`.
const DATA_FILE: &str = "fluxnetworksdata.dat";
/// A device's item components, in the mod's names (`FluxDataComponents`).
const CONFIG_COMPONENT: &str = "fluxnetworks:device_config";
const ENERGY_COMPONENT: &str = "fluxnetworks:stored_energy";
/// `ClickType.PICKUP`, the plain click: Q, number keys, shift and drags reach a
/// menu's hook too, and mean nothing here.
const PICKUP: i32 = 0;
/// A double click arrives as two plain clicks on a slot; the client's window is
/// 250 ms. The second means nothing, or it would make a second network.
const DOUBLE_CLICK: Duration = Duration::from_millis(300);
/// A device looks at its neighbours this often, each at its own tick, and at
/// its first tick. The game's FE calls are few and the mod's own check only
/// runs when a neighbour changes, which a mod's block has no cheap hook for.
const SCAN_EVERY: u64 = 8;
/// The state properties of the six sides, in `Direction::ALL`'s order.
const SIDE_NAMES: [&str; SIDES] = ["down", "up", "north", "south", "west", "east"];

/// A chest menu open on a device.
#[derive(Clone)]
struct OpenMenu {
    window: i32,
    key: Key,
    dimension: String,
    pos: [i32; 3],
    kind: Kind,
    page: usize,
    /// The last plain click: its slot, and when.
    clicked: Option<(i32, Instant)>,
}

thread_local! {
    static WORLD: RefCell<FluxWorld> = RefCell::new(FluxWorld::new(Networks::new(0), FluxConfig::default()));
    /// The world's folder, while one is open.
    static WORLD_DIR: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    /// The level being ticked, as an index of `FluxWorld::dimensions`.
    static LEVEL: Cell<u8> = const { Cell::new(0) };
    /// Server ticks run since the world opened.
    static TICK: Cell<u64> = const { Cell::new(0) };
    static MENUS: RefCell<HashMap<u128, OpenMenu>> = RefCell::new(HashMap::new());
    /// Menus to open at the tick's end, by player: a click runs with the
    /// player's state swapped in, where the server has no hub to open one with.
    static PENDING: RefCell<HashMap<u128, OpenMenu>> = RefCell::new(HashMap::new());
}

fn with_world<R>(f: impl FnOnce(&mut FluxWorld) -> R) -> R {
    WORLD.with(|w| f(&mut w.borrow_mut()))
}

fn key(name: &str) -> JavaString {
    JavaString::from(name)
}

fn is_ours(id: &str) -> bool {
    id.starts_with("fluxnetworks:")
}

/// The blocks with a block entity: every Flux block but `flux_block`.
fn is_device(id: &str) -> bool {
    is_ours(id) && id != "fluxnetworks:flux_block"
}

// ---- Saved compounds, in the mod's names ------------------------------------

fn number(tag: Option<&Tag>) -> Option<i64> {
    match tag? {
        Tag::Byte(v) => Some(i64::from(*v)),
        Tag::Short(v) => Some(i64::from(*v)),
        Tag::Int(v) => Some(i64::from(*v)),
        Tag::Long(v) => Some(*v),
        _ => None,
    }
}

fn string(tag: Option<&Tag>) -> String {
    match tag {
        Some(Tag::String(s)) => s.to_string_lossy(),
        _ => String::new(),
    }
}

/// `CompoundTag.putUUID`: four ints, the most significant first.
fn uuid_tag(uuid: u128) -> Tag {
    Tag::IntArray(vec![(uuid >> 96) as i32, (uuid >> 64) as i32, (uuid >> 32) as i32, uuid as i32])
}

fn uuid_of(tag: Option<&Tag>) -> u128 {
    match tag {
        Some(Tag::IntArray(v)) if v.len() == 4 => v.iter().fold(0, |out, part| (out << 32) | u128::from(*part as u32)),
        _ => 0,
    }
}

/// A device's compound (`TileFluxDevice.readCustomTag`, `NBT_SAVE_ALL`); what
/// is missing keeps a new device's value.
fn read_device(nbt: &Compound, default_limit: i64) -> Saved {
    Saved {
        network: number(nbt.get(&key("network_id"))).map_or(NO_NETWORK, |id| id as i32),
        name: string(nbt.get(&key("custom_name"))),
        owner: uuid_of(nbt.get(&key("player_uuid"))),
        priority: number(nbt.get(&key("priority"))).unwrap_or(0) as i32,
        surge: number(nbt.get(&key("surge_mode"))).unwrap_or(0) != 0,
        limit: number(nbt.get(&key("limit"))).unwrap_or(default_limit),
        disable_limit: number(nbt.get(&key("disable_limit"))).unwrap_or(0) != 0,
        energy: number(nbt.get(&key("energy"))).unwrap_or(0),
    }
}

/// A new device's compound (`writeCustomTag`, `NBT_SAVE_ALL`).
fn new_device(default_limit: i64) -> Compound {
    let mut nbt = Compound::new();
    nbt.insert(key("network_id"), Tag::Int(NO_NETWORK));
    nbt.insert(key("custom_name"), Tag::String(key("")));
    nbt.insert(key("priority"), Tag::Int(0));
    nbt.insert(key("surge_mode"), Tag::Byte(0));
    nbt.insert(key("limit"), Tag::Long(default_limit));
    nbt.insert(key("disable_limit"), Tag::Byte(0));
    nbt.insert(key("energy"), Tag::Long(0));
    nbt.insert(key("player_uuid"), uuid_tag(0));
    nbt
}

/// `FluxNetwork.writeCustomTag` with `NBT_SAVE_ALL`, without the unloaded
/// devices it lists (they come back when their chunks do; the GUI that shows
/// them is a later slice).
fn write_network(network: &Network) -> Tag {
    let mut tag = Compound::new();
    tag.insert(key("network_id"), Tag::Int(network.id));
    tag.insert(key("name"), Tag::String(key(&network.name)));
    tag.insert(key("color"), Tag::Int(network.color as i32));
    tag.insert(key("owner"), uuid_tag(network.owner));
    tag.insert(key("security"), Tag::Byte(network.security.id()));
    tag.insert(key("password"), Tag::String(key(&network.password)));
    let members = network.members.iter().map(|m| {
        let mut member = Compound::new();
        member.insert(key("playerUUID"), uuid_tag(m.uuid));
        member.insert(key("cachedName"), Tag::String(key(&m.name)));
        member.insert(key("accessLevel"), Tag::Byte(m.access.id()));
        Tag::Compound(member)
    });
    tag.insert(key("members"), Tag::List(members.collect()));
    Tag::Compound(tag)
}

fn read_network(tag: &Compound) -> Option<Network> {
    let id = number(tag.get(&key("network_id")))? as i32;
    if id <= 0 {
        return None;
    }
    let members = match tag.get(&key("members")) {
        Some(Tag::List(list)) => list
            .iter()
            .filter_map(|m| match m {
                Tag::Compound(m) => Some(Member {
                    uuid: uuid_of(m.get(&key("playerUUID"))),
                    name: string(m.get(&key("cachedName"))),
                    access: Access::from_id(number(m.get(&key("accessLevel"))).unwrap_or(3)),
                }),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    Some(Network {
        id,
        name: string(tag.get(&key("name"))),
        color: number(tag.get(&key("color"))).unwrap_or(0) as u32 & 0xFF_FF_FF,
        owner: uuid_of(tag.get(&key("owner"))),
        security: Security::from_id(number(tag.get(&key("security"))).unwrap_or(2)),
        password: string(tag.get(&key("password"))),
        members,
    })
}

/// The file's root as Java's `SavedData` writes it: `{data: {...}, DataVersion}`.
fn write_networks(networks: &Networks) -> Tag {
    let mut data = Compound::new();
    data.insert(key("uniqueID"), Tag::Int(networks.unique_id));
    data.insert(key("networks"), Tag::List(networks.by_id.values().map(write_network).collect()));
    let mut root = Compound::new();
    root.insert(key("data"), Tag::Compound(data));
    // Minecraft 1.21.1's, what the mod's own file says.
    root.insert(key("DataVersion"), Tag::Int(3955));
    Tag::Compound(root)
}

fn read_networks(tag: &Tag) -> Option<Networks> {
    let Tag::Compound(root) = tag else {
        return None;
    };
    let Some(Tag::Compound(data)) = root.get(&key("data")) else {
        return None;
    };
    let mut out = Networks::new(number(data.get(&key("uniqueID"))).unwrap_or(0) as i32);
    if let Some(Tag::List(list)) = data.get(&key("networks")) {
        for network in list {
            if let Tag::Compound(network) = network
                && let Some(network) = read_network(network)
            {
                out.by_id.insert(network.id, network);
            }
        }
    }
    Some(out)
}

/// `new SecureRandom().nextInt(50000) * 10000`: a new world's first id.
fn random_origin() -> i32 {
    let random = std::collections::hash_map::RandomState::new().build_hasher().finish();
    (random % 50_000) as i32 * 10_000
}

fn data_path(dir: &std::path::Path) -> PathBuf {
    dir.join("data").join(DATA_FILE)
}

/// Writes the networks. They change only on a player's action, so each change
/// is written at once (`setDirty`, without waiting for the world's save).
fn save_networks() {
    let Some(dir) = WORLD_DIR.with(|d| d.borrow().clone()) else {
        return;
    };
    let tag = with_world(|w| write_networks(&w.networks));
    let path = data_path(&dir);
    let result = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .map_err(|e| e.to_string())
        .and_then(|()| horizon_client::save::write_dat(&path, tag).map_err(|e| e.to_string()));
    if let Err(e) = result {
        log::warn!("[fluxnetworks] {}: {e}", path.display());
    }
}

// ---- The world ----------------------------------------------------------------

/// `native_world_lifecycle`: the networks and the settings are read as a world
/// opens and dropped as it closes.
pub(super) fn lifecycle(game: &mut Game<'_>, phase: WorldPhase) {
    match phase {
        WorldPhase::WorldOpenReady => {
            let config = match Game::native_config_dir() {
                Ok(dir) => FluxConfig::load(&dir),
                Err(e) => {
                    log::warn!("[fluxnetworks] no config folder ({e}): Flux Networks' defaults");
                    FluxConfig::default()
                }
            };
            let dir = game.native_world_dir();
            let networks = dir
                .as_deref()
                .and_then(|dir| horizon_client::save::read_dat(&data_path(dir)))
                .and_then(|tag| read_networks(&tag))
                .unwrap_or_else(|| Networks::new(random_origin()));
            with_world(|w| *w = FluxWorld::new(networks, config));
            WORLD_DIR.with(|d| *d.borrow_mut() = dir);
        }
        WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed | WorldPhase::WorldOpenFailed => {
            with_world(|w| *w = FluxWorld::new(Networks::new(0), FluxConfig::default()));
            WORLD_DIR.with(|d| *d.borrow_mut() = None);
        }
    }
    TICK.with(|t| t.set(0));
    MENUS.with(|m| m.borrow_mut().clear());
    PENDING.with(|m| m.borrow_mut().clear());
}

/// `native_level_tick_pre`: the level whose block entities tick next.
pub(super) fn level_pre(dimension: &str) {
    let level = with_world(|w| w.dimension_id(dimension));
    LEVEL.with(|l| l.set(level));
}

/// `native_block_entity`: a device's entity. Storages and the controller have
/// one with no rules yet (later slices), so their blocks can keep it.
pub(super) fn new_entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if entity.is_some() || !is_device(block) {
        return;
    }
    let nbt = if Kind::of_block(block).is_some() {
        new_device(with_world(|w| w.config.default_limit))
    } else {
        Compound::new()
    };
    *entity = Some((block.to_owned(), nbt));
}

/// `native_block_entity_ticks`: plugs and points tick.
pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if Kind::of_block(block).is_some() {
        *ticks = true;
    }
}

fn beside(pos: [i32; 3], face: Direction) -> [i32; 3] {
    let [dx, dy, dz] = face.normal();
    [pos[0] + dx, pos[1] + dy, pos[2] + dz]
}

/// `updateSideTransfer` of all six sides: the sides with a block that has FE
/// there and isn't a Flux device or on the blacklist (`EnergyUtils.getConnector`).
fn scan_sides(game: &Game<'_>, dimension: &str, pos: [i32; 3]) -> u8 {
    let mut connections = 0;
    for (side, face) in Direction::ALL.into_iter().enumerate() {
        let at = beside(pos, face);
        if game.native_fe_info(dimension, at, Some(face.opposite()), false).is_none() {
            continue;
        }
        let block = game.native_block(at);
        if !is_ours(&block) && !with_world(|w| w.config.blacklisted(&block)) {
            connections |= 1 << side;
        }
    }
    connections
}

/// `native_block_entity_tick` of a plug or a point: it joins its saved network
/// at its first tick, and each takes the settings in its compound and shows on
/// its block the sides it connects to (`sendBlockUpdateIfNeeded`). The energy
/// moves at the server tick's end.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let Some(kind) = Kind::of_block(&entity.id) else {
        return;
    };
    let level = LEVEL.with(Cell::get);
    let at = Key { dimension: level, pos };
    let (default_limit, known) = with_world(|w| (w.config.default_limit, w.devices.get(&at).map(|d| d.connections)));
    let saved = read_device(&entity.nbt, default_limit);
    let staggered = (pos[0] as u64)
        .wrapping_add((pos[1] as u64).wrapping_mul(31))
        .wrapping_add((pos[2] as u64).wrapping_mul(961));
    let due = known.is_none() || (TICK.with(Cell::get).wrapping_add(staggered)) % SCAN_EVERY == 0;
    let connections = if due {
        let dimension = with_world(|w| w.dimensions.get(usize::from(level)).cloned());
        dimension.map_or(known.unwrap_or(0), |d| scan_sides(game, &d, pos))
    } else {
        known.unwrap_or(0)
    };
    with_world(|w| w.observe(at, kind, &saved, connections));
    if known != Some(connections) {
        for (side, name) in SIDE_NAMES.into_iter().enumerate() {
            let value = if connections & (1 << side) != 0 { "true" } else { "false" };
            game.native_set_block_property_with(pos, name, value, flags::UPDATE_CLIENTS);
        }
    }
}

/// The transfer cycle's way to the neighbours: `SideTransfer.send`.
struct Neighbours<'a, 'g> {
    game: &'a mut Game<'g>,
}

impl Host for Neighbours<'_, '_> {
    /// Narrow, as `ForgeEnergyConnector.sendTo`: at most an `int` a call. Flux's
    /// own long capability has nobody to speak to: Flux devices aren't
    /// neighbours, and Applied Flux isn't ported.
    fn send(&mut self, dimension: &str, pos: [i32; 3], side: usize, amount: i64, simulate: bool) -> i64 {
        let face = Direction::ALL[side];
        self.game.native_fe_offer(dimension, beside(pos, face), Some(face.opposite()), amount, simulate, false)
    }
}

/// The open level named `dimension`.
fn level<'a>(game: &'a Game<'_>, dimension: &str) -> Option<&'a World> {
    let dim = Dim::from_id(dimension);
    if dim.id() != dimension {
        return None;
    }
    if dim == game.dim { Some(&game.world) } else { game.levels.parked_world(dim) }
}

/// The energy in the compound of the `kind` of device at `pos`; none when
/// there is no such block entity now.
fn compound_energy(game: &Game<'_>, dimension: &str, pos: [i32; 3], kind: Kind) -> Option<i64> {
    let world = level(game, dimension)?;
    let BlockEntity::Mod(m) = world.block_entities.get(&(pos[0], pos[1], pos[2]))? else {
        return None;
    };
    (Kind::of_block(&m.id) == Some(kind)).then(|| number(m.nbt.get(&key("energy"))).unwrap_or(0))
}

/// Runs `write` on the plug or point at `pos` in any open level, and marks its
/// chunk for saving. False when no such block entity is there now (another
/// block took the place), and nothing else is touched.
fn write_entity(game: &mut Game<'_>, dimension: &str, pos: [i32; 3], write: impl FnOnce(&mut ModBlockEntity)) -> bool {
    let dim = Dim::from_id(dimension);
    if dim.id() != dimension {
        return false;
    }
    let world = if dim == game.dim { Some(&mut game.world) } else { game.levels.parked_world_mut(dim) };
    let Some(world) = world else {
        return false;
    };
    let at = (pos[0], pos[1], pos[2]);
    let Some(BlockEntity::Mod(m)) = world.block_entities.get_mut(&at) else {
        return false;
    };
    if Kind::of_block(&m.id).is_none() {
        return false;
    }
    write(m);
    world.modified.insert((at.0 >> 4, at.2 >> 4));
    true
}

/// `native_server_tick`: `ServerTickEvent.Post` runs every network once
/// (`FluxNetworkData`), then what moved goes to the compounds, where a save
/// finds it. A frozen world's devices don't tick, so nothing happens there.
pub(super) fn server_tick(game: &mut Game<'_>, phase: ServerTickPhase) {
    let ServerTickPhase::ServerTickEnd { world_running } = phase else {
        return;
    };
    if world_running {
        run_networks(game);
    }
    open_pending(game);
}

fn run_networks(game: &mut Game<'_>) {
    TICK.with(|t| t.set(t.get().wrapping_add(1)));
    WORLD.with(|world| {
        let dimensions = world.borrow().dimensions.clone();
        world.borrow_mut().reconcile(|at, kind| compound_energy(game, &dimensions[usize::from(at.dimension)], at.pos, kind));
        tick_all(world, &mut Neighbours { game: &mut *game });
    });
    let changed = with_world(|w| {
        let changed = w.changed_energy();
        changed.into_iter().map(|(at, energy)| (at, w.dimensions[usize::from(at.dimension)].clone(), energy)).collect::<Vec<_>>()
    });
    for (at, dimension, energy) in changed {
        let wrote = write_entity(game, &dimension, at.pos, |m| {
            m.nbt.insert(key("energy"), Tag::Long(energy));
        });
        if wrote {
            with_world(|w| w.energy_saved(at, energy));
        }
    }
    // A menu on a device that left the world closes (`stillValid`).
    let gone: Vec<u128> = MENUS.with(|menus| {
        let menus = menus.borrow();
        with_world(|w| menus.iter().filter(|(_, m)| !w.devices.contains_key(&m.key)).map(|(uuid, _)| *uuid).collect())
    });
    for uuid in gone {
        game.native_close_menu(uuid);
    }
}

// ---- Forge Energy ---------------------------------------------------------------

/// `native_fe_storage`: `IEnergyStorage` of a plug or a point, on every side:
/// its buffer. A plug takes energy while on a network and neither gives any
/// out of its block.
pub(super) fn fe_storage(dimension: &str, pos: [i32; 3], entity: &ModBlockEntity, info: &mut Option<NativeFeInfo>) {
    if info.is_some() || !is_ours(&entity.id) || Kind::of_block(&entity.id).is_none() {
        return;
    }
    let device = with_world(|w| w.key(dimension, pos).and_then(|at| w.info(at)));
    *info = Some(device.map_or_else(NativeFeInfo::default, |d| NativeFeInfo {
        stored: d.stored,
        capacity: d.capacity,
        can_receive: d.can_receive,
        can_extract: false,
    }));
}

/// `native_fe_receive`: `TileFluxPlug`'s `receiveEnergy`, up to its limit and
/// what the network's points asked for last tick; a point takes none. What
/// comes in goes into the compound at once, so a plug's intake is there whatever
/// happens to the plug before the tick's end.
pub(super) fn fe_receive(dimension: &str, pos: [i32; 3], entity: &mut ModBlockEntity, max: i64, simulate: bool, accepted: &mut Option<i64>) {
    if accepted.is_some() || Kind::of_block(&entity.id).is_none() {
        return;
    }
    let (taken, buffer) = with_world(|w| {
        let Some(at) = w.key(dimension, pos) else {
            return (0, None);
        };
        let taken = w.plug_receive(at, max, simulate);
        let buffer = (taken > 0 && !simulate).then(|| w.devices.get(&at).map(|d| d.handler.buffer)).flatten();
        if let Some(buffer) = buffer {
            w.energy_saved(at, buffer);
        }
        (taken, buffer)
    });
    if let Some(buffer) = buffer {
        entity.nbt.insert(key("energy"), Tag::Long(buffer));
    }
    *accepted = Some(taken);
}

// ---- The item ---------------------------------------------------------------------

/// `native_block_entity_drop`: `copy_components` in the mod's loot tables, from
/// `TileFluxDevice.collectImplicitComponents`. It takes any drop of the loot
/// table, a player's or not, as the mod's does.
pub(super) fn drop_components(block: &str, entity: &ModBlockEntity, components: &mut Compound) {
    if Kind::of_block(block).is_none() || Kind::of_block(&entity.id).is_none() {
        return;
    }
    let mut config = Compound::new();
    for name in ["network_id", "custom_name", "priority", "surge_mode", "limit", "disable_limit"] {
        if let Some(tag) = entity.nbt.get(&key(name)) {
            config.insert(key(name), tag.clone());
        }
    }
    components.insert(key(CONFIG_COMPONENT), Tag::Compound(config));
    components.insert(key(ENERGY_COMPONENT), Tag::Long(number(entity.nbt.get(&key("energy"))).unwrap_or(0)));
}

/// `TileFluxDevice.applyImplicitComponents`: what the item kept goes into a new
/// block entity's compound, where its first tick finds it.
fn apply_components(stack: &ItemStack, entity: &mut ModBlockEntity) {
    let Some(Tag::Compound(components)) = item_data::encode_components(stack) else {
        return;
    };
    let Some(Tag::Compound(config)) = components.get(&key(CONFIG_COMPONENT)) else {
        return;
    };
    for name in ["network_id", "custom_name", "priority", "surge_mode", "limit", "disable_limit"] {
        if let Some(tag) = config.get(&key(name)) {
            entity.nbt.insert(key(name), tag.clone());
        }
    }
    if let Some(energy) = number(components.get(&key(ENERGY_COMPONENT))) {
        entity.nbt.insert(key("energy"), Tag::Long(energy));
    }
}

// ---- The chest menu ---------------------------------------------------------------

fn stack_of(slot: &Slot) -> ItemStack {
    let mut stack = ItemStack { item: slot.item.as_str().into(), count: 1, damage: 0, extra: None };
    let name = serde_json::json!({ "text": slot.name, "italic": false, "color": format!("#{:06X}", slot.color) });
    let data = stack.data_mut();
    data.custom_name = Some(component_nbt(&name));
    if !slot.lore.is_empty() {
        let lines = slot.lore.iter().map(|line| component_nbt(&serde_json::json!({ "text": line, "italic": false, "color": "gray" })));
        data.set_other("minecraft:lore", Some(Tag::List(lines.collect())));
    }
    if slot.shine {
        data.set_other("minecraft:enchantment_glint_override", Some(Tag::Byte(1)));
    }
    stack
}

/// The networks `player` can connect to, sorted as the menu lists them.
fn usable_ids(world: &FluxWorld, player: u128) -> Vec<i32> {
    world.networks.usable_by(player).iter().map(|n| n.id).collect()
}

fn menu_stacks(player: u128, menu: &OpenMenu) -> Vec<Option<ItemStack>> {
    with_world(|w| {
        let view = match w.devices.get(&menu.key) {
            Some(d) => DeviceView {
                item: menu.kind.id().to_owned(),
                network: d.network,
                priority: d.handler.priority,
                surge: d.handler.surge,
                limit: d.handler.limit,
                disable_limit: d.handler.disable_limit,
                buffer: d.handler.buffer,
            },
            None => DeviceView {
                item: menu.kind.id().to_owned(),
                network: NO_NETWORK,
                priority: 0,
                surge: false,
                limit: w.config.default_limit,
                disable_limit: false,
                buffer: 0,
            },
        };
        let list: Vec<(&Network, &str)> = w.networks.usable_by(player).into_iter().map(|n| (n, n.access(player).name())).collect();
        menu::layout(&list, menu.page, &view).iter().map(|slot| slot.as_ref().map(stack_of)).collect()
    })
}

/// `native_right_click_block`: `FluxDeviceBlock.useItemOn`. A plug or a point
/// opens its menu for whoever may use it; crouching with something in hand
/// places that instead, as for any block. Storages and the controller have no
/// rules yet, and their click stays their own so it opens nothing.
pub(super) fn right_click(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    let block = game.native_block(pos);
    if !is_device(&block) {
        return;
    }
    if game.keys.contains(&KeyCode::ShiftLeft) && (game.native_held().is_some() || game.offhand.is_some()) {
        return;
    }
    *canceled = true;
    let (Some(kind), Some(player)) = (Kind::of_block(&block), game.native_acting_player()) else {
        return;
    };
    if MENUS.with(|m| m.borrow().get(&player.uuid).is_some_and(|m| m.pos == pos)) {
        return; // The same click, with the other hand, once it opened.
    }
    let dimension = player.dimension.clone();
    let at = with_world(|w| Key { dimension: w.dimension_id(&dimension), pos });
    if !with_world(|w| w.can_access(at, player.uuid)) {
        game.native_action_bar(player.uuid, Text::literal("Access denied: No permission"));
        return;
    }
    let menu = OpenMenu { window: 0, key: at, dimension, pos, kind, page: 0, clicked: None };
    PENDING.with(|m| m.borrow_mut().insert(player.uuid, menu));
}

/// Opens the menus players asked for since the last tick's end.
fn open_pending(game: &mut Game<'_>) {
    let pending: Vec<(u128, OpenMenu)> = PENDING.with(|m| m.borrow_mut().drain().collect());
    for (uuid, mut menu) in pending {
        let items = menu_stacks(uuid, &menu);
        let title = Text::literal(match menu.kind {
            Kind::Plug => "Flux Plug",
            Kind::Point => "Flux Point",
        });
        if let Some(window) = game.native_open_menu(uuid, menu::ROWS, title, items) {
            menu.window = window;
            MENUS.with(|m| m.borrow_mut().insert(uuid, menu));
        }
    }
}

/// The player's choice on a device: `TileFluxDevice.connect`, its compound and
/// the live device together. The first to connect a device with no owner owns
/// it (one placed by a command has none).
fn connect(game: &mut Game<'_>, menu: &OpenMenu, player: u128, network: i32) {
    let wrote = write_entity(game, &menu.dimension, menu.pos, |m| {
        m.nbt.insert(key("network_id"), Tag::Int(network));
        if uuid_of(m.nbt.get(&key("player_uuid"))) == 0 {
            m.nbt.insert(key("player_uuid"), uuid_tag(player));
        }
    });
    if wrote {
        with_world(|w| {
            w.connect(menu.key, network);
            if let Some(device) = w.devices.get_mut(&menu.key)
                && device.owner == 0
            {
                device.owner = player;
            }
        });
    }
}

/// `native_menu_clicked`: what the slot under a plain click means, for this
/// menu.
pub(super) fn menu_clicked(game: &mut Game<'_>, uuid: u128, window: i32, slot: i32, mode: i32) {
    if mode != PICKUP {
        return;
    }
    let now = Instant::now();
    let Some(mut menu) = MENUS.with(|m| {
        let mut menus = m.borrow_mut();
        let menu = menus.get_mut(&uuid).filter(|m| m.window == window)?;
        let again = menu.clicked.is_some_and(|(at, when)| at == slot && now.duration_since(when) < DOUBLE_CLICK);
        menu.clicked = Some((slot, now));
        (!again).then(|| menu.clone())
    }) else {
        return;
    };
    let ids = with_world(|w| usable_ids(w, uuid));
    let Some(action) = menu::action(&ids, menu.page, slot) else {
        return;
    };
    if !with_world(|w| w.can_access(menu.key, uuid)) {
        game.native_action_bar(uuid, Text::literal("Access denied: No permission"));
        return;
    }
    match action {
        Action::Connect(network) => connect(game, &menu, uuid, network),
        Action::Disconnect => connect(game, &menu, uuid, NO_NETWORK),
        Action::Create => {
            let name = game.native_server_players().into_iter().find(|p| p.uuid == uuid).map_or_else(String::new, |p| p.name);
            let made = with_world(|w| {
                let maximum = w.config.maximum_per_player;
                w.networks.create_default(uuid, &name, maximum)
            });
            match made {
                Ok(_) => save_networks(),
                Err(CreateError::NoSpace) => {
                    game.native_action_bar(uuid, Text::literal("You have reached the maximum number of networks you can create"));
                }
            }
        }
        Action::Page(page) => {
            menu.page = page;
            MENUS.with(|m| {
                if let Some(open) = m.borrow_mut().get_mut(&uuid) {
                    open.page = page;
                }
            });
        }
    }
    let items = menu_stacks(uuid, &menu);
    game.native_set_menu_items(uuid, items);
}

/// `native_menu_closed`.
pub(super) fn menu_closed(uuid: u128, window: i32) {
    MENUS.with(|m| {
        let mut menus = m.borrow_mut();
        if menus.get(&uuid).is_some_and(|m| m.window == window) {
            menus.remove(&uuid);
        }
    });
}

/// `native_place_block`: `setPlacedBy`, the device takes what its item kept and
/// the placer owns it.
pub(super) fn placed(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3]) {
    if Kind::of_block(&game.native_block(pos)).is_none() {
        return;
    }
    let owner = game.native_acting_player().map(|player| player.uuid);
    game.native_with_mod_block_entity(pos, |m| {
        if Kind::of_block(&m.id).is_some() {
            apply_components(stack, m);
            if let Some(owner) = owner {
                m.nbt.insert(key("player_uuid"), uuid_tag(owner));
            }
        }
    });
}

// ---- /flux ------------------------------------------------------------------------

/// `native_register_commands`.
pub(super) fn commands(commands: &mut Vec<NativeCommand>) {
    commands.push(NativeCommand::op("flux probe <pos:block_pos>", 2));
    commands.push(NativeCommand::op("flux networks", 2));
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

fn probe(player: &NativeServerPlayer, pos: [i32; 3]) -> serde_json::Value {
    with_world(|w| {
        let device = w.key(&player.dimension, pos).and_then(|at| w.devices.get(&at).map(|d| (at, d)));
        let Some((at, d)) = device else {
            return serde_json::json!({ "pos": pos, "error": "no live device here" });
        };
        let live = w.live.get(&d.network);
        let info = w.info(at);
        serde_json::json!({
            "pos": pos,
            "kind": d.handler.kind.id(),
            "network": d.network,
            "saved_network": d.saved_network,
            "owner": format!("{:032x}", d.owner),
            "priority": d.handler.priority,
            "logical_priority": d.handler.logical_priority(),
            "surge": d.handler.surge,
            "limit": d.handler.limit,
            "disable_limit": d.handler.disable_limit,
            "buffer": d.handler.buffer,
            "change": d.handler.change,
            "desired": d.handler.desired,
            "connections": d.connections,
            "limiter": live.map(|l| l.limiter),
            "members": live.map(|l| l.all.len()),
            "can_receive": info.map(|i| i.can_receive),
        })
    })
}

fn networks_json() -> serde_json::Value {
    with_world(|w| {
        let networks: Vec<serde_json::Value> = w
            .networks
            .by_id
            .values()
            .map(|n| {
                serde_json::json!({
                    "id": n.id,
                    "name": n.name,
                    "color": n.color,
                    "owner": format!("{:032x}", n.owner),
                    "security": n.security.name(),
                    "members": n.members.len(),
                    "devices": w.live.get(&n.id).map_or(0, |l| l.all.len()),
                })
            })
            .collect();
        serde_json::json!({
            "unique_id": w.networks.unique_id,
            "networks": networks,
            "maximum_per_player": w.config.maximum_per_player,
            "default_limit": w.config.default_limit,
            "from_file": w.config.from_file,
        })
    })
}

/// `native_perform_command`: `/flux probe x y z` answers `FLUX {json}` with the
/// live device there, `/flux networks` with the world's networks.
pub(super) fn perform(game: &mut Game<'_>, player: &NativeServerPlayer, command: &str, ran: &mut bool) {
    let mut words = command.split_whitespace();
    if *ran || words.next() != Some("flux") {
        return;
    }
    *ran = true;
    let answer = match words.next() {
        Some("probe") => {
            let p = player.position;
            match (coordinate(words.next(), p[0]), coordinate(words.next(), p[1]), coordinate(words.next(), p[2])) {
                (Some(x), Some(y), Some(z)) => probe(player, [x, y, z]),
                _ => serde_json::json!({ "error": "usage: /flux probe <x> <y> <z>" }),
            }
        }
        Some("networks") => networks_json(),
        _ => serde_json::json!({ "error": "usage: /flux probe <x> <y> <z> | /flux networks" }),
    };
    game.native_feedback(Text::literal(format!("FLUX {answer}")));
}
