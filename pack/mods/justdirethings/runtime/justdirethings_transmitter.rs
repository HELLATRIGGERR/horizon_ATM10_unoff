// Just Dire Things' Energy Transmitter in the game (support module `crate::game::justdirethings_transmitter` of the
// `justdirethings` native bundle), on what every T2 machine shares (`justdirethings_clicker`: the area, the nine filter slots
// in a 9x2 chest menu; here the area starts as the block above). The rules are the bundle's crate `justdirethings_core`
// (`transmitter`); this module is the glue: its saved compound (Java's keys, `energy` its own buffer), the area scanned every
// 50 ticks for blocks that take Forge Energy and for other transmitters, what each tick sends to them
// (`native_fe_offer`), and the network of transmitters one sees: each one's energy is kept here (the transmitters in an
// area change one another's), written to its compound on its next tick. Translated from Just Dire Things 1.5.7 (MIT); see
// this bundle's README.md.

use super::{
    Game,
    justdirethings_clicker as clicker,
    justdirethings_machines as machines,
    justdirethings_world as world,
};
use horizon_client::containers::ModContainer;
use horizon_core::geometry::Direction;
use horizon_core::nbt::{Compound, Tag};
use justdirethings_core::{
    machines::Clock,
    redstone::Mode,
    transmitter::{Network, new_clock},
};
use mod_api::{ModBlockEntity, NativeFeInfo};
use std::{cell::RefCell, collections::HashMap};

pub(super) const TRANSMITTER: &str = "justdirethings:energytransmitter";
/// The 9x2 chest menu: slot 0 (the battery slot, unused here) and the nine filter slots.
const SLOTS: usize = 18;

pub(super) fn owns(block: &str) -> bool {
    block == TRANSMITTER
}

type Key = (&'static str, [i32; 3]);

thread_local! {
    /// Each transmitter's energy as the network last left it, and what its compound held when it was last written
    /// (a compound that differs was changed from outside and wins).
    static REAL: RefCell<HashMap<Key, (i32, i32)>> = RefCell::new(HashMap::new());
    /// The transmitters each one found in its area (itself first), and the blocks it charges with the side that takes energy.
    static MEMBERS: RefCell<HashMap<Key, Vec<[i32; 3]>>> = RefCell::new(HashMap::new());
    static TARGETS: RefCell<HashMap<Key, Vec<([i32; 3], usize)>>> = RefCell::new(HashMap::new());
    /// The dimension names seen (a compound names its dimension; the keys hold the game's own strings).
    static DIMENSIONS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

pub(super) fn reset() {
    REAL.with(|r| r.borrow_mut().clear());
    MEMBERS.with(|r| r.borrow_mut().clear());
    TARGETS.with(|r| r.borrow_mut().clear());
    DIMENSIONS.with(|r| r.borrow_mut().clear());
}

/// `native_block_entity`: Java's defaults (scan every 50 ticks, show particles, an empty buffer).
pub(super) fn new_entity(nbt: &mut Compound) {
    machines::write_clock(nbt, &new_clock());
    machines::put(nbt, "direction", 0);
    machines::put_flag(nbt, "showParticles", true);
    machines::put(nbt, "energy", 0);
}

/// `native_block_entity_container`: the menu with the nine filter slots (slot 0 holds nothing: no item is charged from it).
pub(super) fn container(container: &mut Option<ModContainer>) {
    super::justdirethings_t2x::filter_only(container);
}

pub(super) fn slot_accepts(slot: usize, accepts: &mut bool) {
    *accepts = *accepts && (1..=9).contains(&slot) && slot < SLOTS;
}

/// Where a compound says its transmitter stands (written on each tick, for the capability calls that carry no position).
fn key_of(nbt: &Compound) -> Option<Key> {
    let Some(Tag::String(d)) = nbt.get(&machines::key("jdtDim")) else { return None };
    let name = d.to_string_lossy();
    let dimension = DIMENSIONS.with(|n| n.borrow().iter().copied().find(|n| *n == name))?;
    Some((dimension, [machines::int(nbt, "jdtX"), machines::int(nbt, "jdtY"), machines::int(nbt, "jdtZ")]))
}

fn tag_position(nbt: &mut Compound, key: Key) {
    DIMENSIONS.with(|n| {
        let mut n = n.borrow_mut();
        if !n.contains(&key.0) {
            n.push(key.0);
        }
    });
    if key_of(nbt) != Some(key) {
        nbt.insert(machines::key("jdtDim"), Tag::String(key.0.into()));
        machines::put(nbt, "jdtX", key.1[0]);
        machines::put(nbt, "jdtY", key.1[1]);
        machines::put(nbt, "jdtZ", key.1[2]);
    }
}

/// One transmitter's energy: the network's value, unless its compound was changed from outside since it was written.
fn energy_of(key: Key, nbt_energy: i32) -> i32 {
    REAL.with(|r| {
        let mut r = r.borrow_mut();
        match r.get(&key) {
            Some((real, written)) if *written == nbt_energy => *real,
            _ => {
                r.insert(key, (nbt_energy, nbt_energy));
                nbt_energy
            }
        }
    })
}

/// The network `key` sees: its transmitters that have an energy here, itself among them.
fn network(key: Key, cap: i32) -> (Network, Vec<[i32; 3]>) {
    let members = MEMBERS.with(|m| m.borrow().get(&key).cloned()).unwrap_or_else(|| vec![key.1]);
    let mut list: Vec<[i32; 3]> = Vec::new();
    let mut energies = Vec::new();
    let mut me = 0;
    REAL.with(|r| {
        let r = r.borrow();
        for p in members {
            if let Some((e, _)) = r.get(&(key.0, p)) {
                if p == key.1 {
                    me = list.len();
                }
                list.push(p);
                energies.push(*e);
            }
        }
    });
    (Network { energies, cap, me }, list)
}

fn store(key: Key, list: &[[i32; 3]], net: &Network) {
    REAL.with(|r| {
        let mut r = r.borrow_mut();
        for (p, e) in list.iter().zip(&net.energies) {
            if let Some(entry) = r.get_mut(&(key.0, *p)) {
                entry.0 = *e;
            }
        }
    });
}

/// `isStackValidFilter` of the block's item for the scan, and `level.getBlockEntity != null`: a block entity that takes
/// energy on some side.
fn charge_side(game: &Game<'_>, dimension: &'static str, at: [i32; 3]) -> Option<usize> {
    (0..6).find(|side| game.native_fe_info(dimension, at, Some(Direction::ALL[*side]), false).is_some_and(|i| i.can_receive))
}

/// `getBlocksToCharge`: the blocks of the area nearest first that have a block entity, take energy and pass the filter;
/// transmitters among them join the network.
fn scan(game: &mut Game<'_>, level: mod_api::LevelRef, key: Key, shared: &justdirethings_core::t2::Shared, filters: &[String]) {
    let (dimension, pos) = key;
    let mut members = vec![pos];
    let mut targets = Vec::new();
    for at in shared.nearest_first(pos) {
        if at == pos {
            continue;
        }
        let Some(state) = game.level_block_state(level, at) else { continue };
        let facts = game.res.facts().get(state);
        if facts.air || game.world.block_entities.get(&(at[0], at[1], at[2])).is_none() {
            continue;
        }
        let name = facts.name.to_string();
        let Some(side) = charge_side(game, dimension, at) else { continue };
        if !clicker::block_passes(game, &shared.filter, filters, &name) {
            continue;
        }
        if name == TRANSMITTER {
            members.push(at);
        } else {
            targets.push((at, side));
        }
    }
    MEMBERS.with(|m| m.borrow_mut().insert(key, members));
    TARGETS.with(|t| t.borrow_mut().insert(key, targets));
}

fn manhattan(a: [i32; 3], b: [i32; 3]) -> i32 {
    (0..3).map(|i| (a[i] - b[i]).abs()).sum()
}

/// One server tick (`EnergyTransmitterBE.tickServer`), then what its screen shows.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let Some(level) = game.fork_level_here() else { return };
    let dimension = game.native_dimension();
    let signal = game.native_neighbor_signal(dimension, pos) > 0;
    let cfg = world::settings().transmitter;
    let key: Key = (dimension, pos);
    // The area starts as the block above (`new AreaAffectingData()`).
    let fresh = !entity.nbt.contains_key(&machines::key("xRadiusDouble"));
    let mut shared = clicker::read_shared(&entity.nbt, [0, 1, 0]);
    tag_position(&mut entity.nbt, key);
    let own = energy_of(key, machines::int(&entity.nbt, "energy").clamp(0, cfg.max_rf));
    let before = machines::clock_at(dimension, pos, &entity.nbt);
    let mut clock: Clock = before;
    clock.tick(signal);
    let active = clock.redstone.is_active();
    if active {
        if clock.can_run() {
            let filters = clicker::filter_items(game, entity);
            scan(game, level, key, &shared, &filters);
        }
        // Verify the members still stand (a broken transmitter's energy goes with it).
        let members = MEMBERS.with(|m| m.borrow().get(&key).cloned()).unwrap_or_else(|| vec![pos]);
        for p in members.iter().filter(|p| **p != pos) {
            if game.native_block(*p) != TRANSMITTER {
                REAL.with(|r| r.borrow_mut().remove(&(dimension, *p)));
            }
        }
        provide(game, key, &cfg);
    }
    // What the network left this transmitter.
    let (net, _) = network(key, cfg.max_rf);
    let energy = net.energies.get(net.me).copied().unwrap_or(own).clamp(0, cfg.max_rf);
    REAL.with(|r| r.borrow_mut().insert(key, (energy, energy)));
    if energy != machines::int(&entity.nbt, "energy") || fresh {
        machines::put(&mut entity.nbt, "energy", energy);
    }
    machines::keep_clock(dimension, pos, &mut entity.nbt, &before, &clock);
    shared.powered.energy = net.total().clamp(i32::MIN, i32::MAX);
    if fresh {
        clicker::write_shared(&mut entity.nbt, &shared);
        machines::put(&mut entity.nbt, "energy", energy);
    }
    let particles = i32::from(machines::int(&entity.nbt, "showParticles") != 0 || !entity.nbt.contains_key(&machines::key("showParticles")));
    let mut data = vec![clock.redstone.mode.index(), particles];
    super::justdirethings_t2x::parts(&mut data, &shared);
    data.push(cfg.max_rf);
    if entity.data != data {
        entity.data = data;
    }
}

/// `providePower`: while the network holds energy, each block to charge gets what it takes of `rf_per_tick`, less the loss
/// for its distance; then the network balances.
fn provide(game: &mut Game<'_>, key: Key, cfg: &justdirethings_core::config::TransmitterSettings) {
    let (dimension, pos) = key;
    let (mut net, list) = network(key, cfg.max_rf);
    if net.total() <= 0 {
        return;
    }
    let targets = TARGETS.with(|t| t.borrow().get(&key).cloned()).unwrap_or_default();
    for (at, side) in targets {
        net.send(cfg, manhattan(pos, at), |amount, simulate| {
            game.native_fe_offer(dimension, at, Some(Direction::ALL[side]), i64::from(amount), simulate, false).clamp(0, i64::from(i32::MAX)) as i32
        });
    }
    net.balance();
    store(key, &list, &net);
}

/// The network of the transmitter whose compound this is (or its own buffer while it has not ticked).
fn network_of(nbt: &Compound) -> Option<(Key, Network, Vec<[i32; 3]>)> {
    let key = key_of(nbt)?;
    let (net, list) = network(key, world::settings().transmitter.max_rf);
    (!list.is_empty() && list.contains(&key.1)).then_some((key, net, list))
}

/// `native_fe_storage`: the network's total and its buffers.
pub(super) fn fe_storage(entity: &ModBlockEntity, info: &mut Option<NativeFeInfo>) {
    let cap = world::settings().transmitter.max_rf;
    let (stored, capacity) = match network_of(&entity.nbt) {
        Some((_, net, _)) => (i64::from(net.total()), i64::from(net.max_total())),
        None => (i64::from(machines::int(&entity.nbt, "energy")), i64::from(cap)),
    };
    *info = Some(NativeFeInfo { stored, capacity, can_receive: true, can_extract: true });
}

/// `native_fe_receive`: what the network takes, shared out among its transmitters.
pub(super) fn fe_receive(entity: &mut ModBlockEntity, max: i64, simulate: bool, accepted: &mut Option<i64>) {
    let amount = max.clamp(0, i64::from(i32::MAX)) as i32;
    let took = match network_of(&entity.nbt) {
        Some((key, mut net, list)) => {
            let took = net.receive(amount, simulate);
            if took > 0 && !simulate {
                store(key, &list, &net);
                // This one's compound is up to date now; the others' follow on their ticks.
                let mine = net.energies[net.me];
                machines::put(&mut entity.nbt, "energy", mine);
                REAL.with(|r| r.borrow_mut().insert(key, (mine, mine)));
            }
            took
        }
        None => {
            let cap = world::settings().transmitter.max_rf;
            let have = machines::int(&entity.nbt, "energy");
            let took = (cap - have).min(cap.min(amount)).max(0);
            if took > 0 && !simulate {
                machines::put(&mut entity.nbt, "energy", have + took);
            }
            took
        }
    };
    *accepted = Some(i64::from(took));
}

/// `native_fe_extract`: up to what the network holds, from this transmitter's own energy.
pub(super) fn fe_extract(entity: &mut ModBlockEntity, max: i64, simulate: bool, given: &mut Option<i64>) {
    let amount = max.clamp(0, i64::from(i32::MAX)) as i32;
    let took = match network_of(&entity.nbt) {
        Some((key, mut net, list)) => {
            let took = net.extract(amount, simulate);
            if took > 0 && !simulate {
                store(key, &list, &net);
                let mine = net.energies[net.me].max(0);
                machines::put(&mut entity.nbt, "energy", mine);
                REAL.with(|r| r.borrow_mut().insert(key, (mine, mine)));
            }
            took
        }
        None => {
            let cap = world::settings().transmitter.max_rf;
            let have = machines::int(&entity.nbt, "energy");
            let took = have.min(cap.min(amount)).max(0);
            if took > 0 && !simulate {
                machines::put(&mut entity.nbt, "energy", have - took);
            }
            took
        }
    };
    *given = Some(i64::from(took));
}

/// `native_menu_button`: 0 to 3 the redstone mode, 27 show particles, and the shared area and filter buttons.
pub(super) fn button(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, id: i32) {
    let dimension = game.native_dimension();
    let before = machines::clock_at(dimension, pos, &entity.nbt);
    let mut clock = before;
    match id {
        0..=3 => clock.redstone.mode = Mode::from_index(id),
        27 => {
            let show = machines::int(&entity.nbt, "showParticles") == 0;
            machines::put_flag(&mut entity.nbt, "showParticles", show);
        }
        _ => {
            let mut shared = clicker::read_shared(&entity.nbt, [0, 1, 0]);
            if shared.button(id) {
                // The energy key holds the buffer, which the shared writer would overwrite with its own zero.
                let energy = machines::int(&entity.nbt, "energy");
                clicker::write_shared(&mut entity.nbt, &shared);
                machines::put(&mut entity.nbt, "energy", energy);
            }
            return;
        }
    }
    machines::keep_clock(dimension, pos, &mut entity.nbt, &before, &clock);
}
