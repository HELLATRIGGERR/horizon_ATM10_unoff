// Mekanism's Mechanical Pipes in the game (support module `crate::game::mek_pipes`
// of the `mekanism` native bundle). The rules are the bundle's crate
// (`mekanism_energy::network`, the tubes' networks with a fluid, and
// `transmitter`'s tiers; MIT, from Mekanism 10.7.19, Copyright (c) 2017-2025
// Aidan C. Brady); this module is the glue, as `mek_tubes` is the tubes': the
// pipes of each dimension and their networks kept here through the block
// entity lifecycle hook points (API 418), each level's tick committing them,
// looking at their sides (any block with a fluid handler on the touching
// side, through the fluid handler helpers of API 422), joining networks that
// may now meet, pulling through pull sides, emitting to the handlers beside
// them with the even split, and marking their chunks changed; a pipe's share
// of its network saved as `fluid`; the network as a fluid handler on each side
// for the blocks around it (hook points API 422); `/mek connection` and
// `/mek probe`. A network's fluid is a key: the fluid's id, or the id and a
// number for one with components, kept here with a stack of it. The
// transmitters' settings (`tiers.toml`) are read here for the pipes, the
// transporters and the fluid tanks. No borrow of this state is held across a
// call into the game.

use super::{Game, mek_world::key, native_hooks::NativeFluidTankInfo};
use horizon_core::geometry::Direction as Face;
use horizon_world::{
    fluid_stack::FluidStack,
    level::flags,
    nbt::{Compound, Tag},
};
use mekanism_energy::{
    config,
    network::{self, Connection, Member, Networks},
    sides::Direction,
    transmitter::{PipeTier, TransmitterConfig},
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeSet, HashMap},
    time::{Duration, Instant},
};

/// A dimension's pipes.
#[derive(Default)]
struct DimPipes {
    nets: Networks,
    /// Pipes whose sides need a look.
    look: BTreeSet<[i32; 3]>,
    /// Ticks until every pipe gets a look: a neighbour's handler can change
    /// with no block changing.
    full_look_in: u32,
    loads: u64,
    unloads: u64,
}

thread_local! {
    static PIPES: RefCell<HashMap<String, DimPipes>> = RefCell::new(HashMap::new());
    static ANY: Cell<bool> = const { Cell::new(false) };
    /// Blocks set since the last level tick, for the pipes beside them.
    static SET: RefCell<Vec<[i32; 3]>> = const { RefCell::new(Vec::new()) };
    /// The fluids with components that networks hold, by their key's number.
    static KINDS: RefCell<Vec<FluidStack>> = const { RefCell::new(Vec::new()) };
    /// `tiers.toml`'s transmitter settings, when its file last changed and when that was looked at.
    static SETTINGS: RefCell<Option<(TransmitterConfig, config::Stamp, Instant)>> = const { RefCell::new(None) };
}

fn with_pipes<R>(dimension: &str, change: impl FnOnce(&mut DimPipes) -> R) -> R {
    PIPES.with(|t| {
        let mut t = t.borrow_mut();
        if !t.contains_key(dimension) {
            t.insert(dimension.to_owned(), DimPipes::default());
        }
        change(t.get_mut(dimension).expect("just made"))
    })
}

/// `[transmitters.fluid]`, `[transmitters.items]` and `[fluid_tanks]`, read
/// when first asked and again within a second of the file changing.
pub(super) fn settings() -> TransmitterConfig {
    let dir = Game::native_config_dir().ok();
    SETTINGS.with(|s| {
        let mut s = s.borrow_mut();
        let stale = match s.as_mut() {
            None => true,
            Some((_, stamp, looked)) if looked.elapsed() >= Duration::from_secs(1) => {
                *looked = Instant::now();
                dir.as_ref().is_some_and(|d| config::stamp(d) != *stamp)
            }
            Some(_) => false,
        };
        if stale {
            let fresh = match &dir {
                Some(d) => (TransmitterConfig::load(d), config::stamp(d), Instant::now()),
                None => (TransmitterConfig::default(), [None; config::FILES.len()], Instant::now()),
            };
            *s = Some(fresh);
        }
        s.as_ref().map(|(c, _, _)| c.clone()).unwrap_or_default()
    })
}

pub(super) fn tier_of(block: &str) -> Option<PipeTier> {
    PipeTier::of_block(block)
}

/// A world opens or closes: no pipes, and the settings are read again.
pub(super) fn clear() {
    PIPES.with(|t| t.borrow_mut().clear());
    ANY.set(false);
    SET.with(|s| s.borrow_mut().clear());
    KINDS.with(|k| k.borrow_mut().clear());
    SETTINGS.with(|s| *s.borrow_mut() = None);
}

// ---- Fluids as network contents ---------------------------------------------------

/// The key of `stack`'s fluid: its id, or with components `<id>#<n>`.
pub(super) fn fluid_key(stack: &FluidStack) -> String {
    if stack.components().is_empty() {
        return stack.name().to_owned();
    }
    KINDS.with(|k| {
        let mut k = k.borrow_mut();
        let n = match k.iter().position(|s| FluidStack::is_same_fluid_same_components(s, stack)) {
            Some(n) => n,
            None => {
                k.push(stack.copy_with_amount(1));
                k.len() - 1
            }
        };
        format!("{}#{n}", stack.name())
    })
}

/// `amount` of the fluid of `key`; empty for a key no fluid has.
pub(super) fn fluid_of(key: &str, amount: u64) -> FluidStack {
    let amount = amount.min(i32::MAX as u64) as i32;
    match key.split_once('#') {
        Some((_, n)) => KINDS.with(|k| k.borrow().get(n.parse::<usize>().unwrap_or(usize::MAX)).map(|s| s.copy_with_amount(amount))).unwrap_or(FluidStack::EMPTY),
        None => FluidStack::of(key, amount).unwrap_or(FluidStack::EMPTY),
    }
}

// ---- The block entity ---------------------------------------------------------------

/// `connection`: each side's `ConnectionType` ordinal.
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

/// `native_block_entity`: every side normal.
pub(super) fn new_entity(block: &str) -> (String, Compound) {
    let mut nbt = Compound::new();
    write_connections(&mut nbt, &[Connection::Normal; 6]);
    (block.to_owned(), nbt)
}

/// `fluid` (`FluidStack.save`): the fluid and its share, as saved; a fluid the
/// game lacks reads as nothing (`parseOptional`).
fn saved_share(nbt: &Compound) -> (u64, Option<String>) {
    let Some(Tag::Compound(fluid)) = nbt.get(&key("fluid")) else {
        return (0, None);
    };
    let stack = FluidStack::parse_optional(fluid);
    if stack.is_empty() {
        return (0, None);
    }
    (stack.amount().max(0) as u64, Some(fluid_key(&stack)))
}

/// `native_block_entity_load` of a pipe: it queues to join with its share
/// (`registerOrphanTransmitter`).
pub(super) fn loaded(dimension: &str, pos: [i32; 3], nbt: &Compound, tier: PipeTier) {
    let (capacity, pull) = settings().pipe(tier);
    let (share, content) = saved_share(nbt);
    let member = Member { capacity: capacity.max(0) as u64, pull: pull.max(0) as u64, connections: connections_of(nbt), share, content };
    with_pipes(dimension, |d| {
        d.nets.queue_add_with(pos, member);
        d.loads += 1;
    });
    ANY.set(true);
}

/// `native_block_entity_unload` of a pipe: unloading, it takes its saved
/// share with it; broken, the rest of its network keeps the fluid.
pub(super) fn unloaded(dimension: &str, pos: [i32; 3], removed: bool) {
    with_pipes(dimension, |d| {
        if !removed {
            d.nets.take_share(pos);
        }
        d.nets.queue_remove(pos);
        d.unloads += 1;
    });
}

/// A block with fluids beside pipes came or went: they look again.
pub(super) fn neighbour(dimension: &str, pos: [i32; 3]) {
    if ANY.get() {
        with_pipes(dimension, |d| d.look.extend((0..6).map(|s| network::offset(pos, s))));
    }
}

/// `native_block_changed`.
pub(super) fn block_set(pos: [i32; 3]) {
    if ANY.get() {
        SET.with(|s| s.borrow_mut().push(pos));
    }
}

/// `native_block_entity_save` of a pipe: its share of its network as `fluid`
/// (`MechanicalPipe.write`), left out when none.
pub(super) fn saved(dimension: &str, pos: [i32; 3], nbt: &mut Compound) {
    let share = with_pipes(dimension, |d| {
        let share = d.nets.save_share(pos)?;
        Some((share, d.nets.network_of(pos).and_then(|(_, n)| n.content.clone())))
    });
    match share {
        Some((share, Some(id))) if share > 0 => match fluid_of(&id, share).save() {
            Some(fluid) => {
                nbt.insert(key("fluid"), Tag::Compound(fluid));
            }
            None => {
                nbt.remove(&key("fluid"));
            }
        },
        Some(_) => {
            nbt.remove(&key("fluid"));
        }
        None => {}
    }
}

// ---- The handler ------------------------------------------------------------------

/// The network's fluid as a stack, and its capacity.
fn held(nets: &Networks, pos: [i32; 3]) -> (FluidStack, i32) {
    nets.network_of(pos).map_or((FluidStack::EMPTY, 0), |(_, n)| {
        let stack = match &n.content {
            Some(id) if n.buffer > 0 => fluid_of(id, n.buffer),
            _ => FluidStack::EMPTY,
        };
        (stack, n.capacity.min(i32::MAX as u64) as i32)
    })
}

/// `native_fluid_handler_tanks` of a pipe (`TileEntityMechanicalPipe`'s
/// `DynamicFluidHandler` over its network's tank): one tank, none on a side
/// cut with the Configurator. Whether `entity` is a pipe.
pub(super) fn fluid_tanks(dimension: &str, pos: [i32; 3], face: Option<Face>, entity: &horizon_client::containers::ModBlockEntity, tanks: &mut Option<Vec<NativeFluidTankInfo>>) -> bool {
    if tier_of(&entity.id).is_none() {
        return false;
    }
    let side = face.map(|f| super::mek_world::mek_direction(f) as usize);
    *tanks = with_pipes(dimension, |d| {
        d.nets.exposed(pos, side)?;
        let (stack, capacity) = held(&d.nets, pos);
        let drainable = !stack.is_empty() && side.is_some_and(|s| d.nets.shown(pos, s).can_send_to());
        Some(vec![NativeFluidTankInfo { stack, capacity, drainable }])
    });
    true
}

/// `native_fluid_handler_fill` of a pipe: into its network through a side that
/// accepts (normal, pull), of the network's one fluid (any while it holds
/// none). Whether `entity` is a pipe.
pub(super) fn fluid_fill(dimension: &str, pos: [i32; 3], face: Option<Face>, entity: &horizon_client::containers::ModBlockEntity, resource: &FluidStack, simulate: bool, filled: &mut Option<i32>) -> bool {
    if tier_of(&entity.id).is_none() {
        return false;
    }
    let side = face.map(|f| super::mek_world::mek_direction(f) as usize);
    let fluid = fluid_key(resource);
    let amount = resource.amount().max(0) as u64;
    *filled = with_pipes(dimension, |d| {
        d.nets.exposed(pos, side)?;
        let rest = d.nets.insert_of_at(pos, side, Some(&fluid), amount, simulate);
        Some((amount - rest) as i32)
    });
    true
}

/// `native_fluid_handler_drain` of a pipe: out of its network through a side
/// that sends (normal, push); `drain(FluidStack)` only of its fluid. Whether
/// `entity` is a pipe.
#[allow(clippy::too_many_arguments)]
pub(super) fn fluid_drain(
    dimension: &str,
    pos: [i32; 3],
    face: Option<Face>,
    entity: &horizon_client::containers::ModBlockEntity,
    resource: Option<&FluidStack>,
    max: i32,
    simulate: bool,
    drained: &mut Option<FluidStack>,
) -> bool {
    if tier_of(&entity.id).is_none() {
        return false;
    }
    let side = face.map(|f| super::mek_world::mek_direction(f) as usize);
    *drained = with_pipes(dimension, |d| {
        d.nets.exposed(pos, side)?;
        let (stack, _) = held(&d.nets, pos);
        if stack.is_empty() || resource.is_some_and(|r| !FluidStack::is_same_fluid_same_components(r, &stack)) {
            return Some(FluidStack::EMPTY);
        }
        let wanted = resource.map_or(max, FluidStack::amount).max(0) as u64;
        let got = d.nets.extract_at(pos, side, wanted, simulate);
        Some(stack.copy_with_amount(got as i32))
    });
    true
}

// ---- The level's tick --------------------------------------------------------------

/// `Transmitter.refreshConnections` and `recheckConnection`: the pipe at `pos`
/// joins the networks beside it that may now meet its own, and each side shows
/// a connection to a block with a fluid handler there.
fn refresh(game: &mut Game<'_>, dimension: &str, pos: [i32; 3]) {
    let Some(raw) = with_pipes(dimension, |d| d.nets.cable(pos).map(|c| c.connections)) else {
        return;
    };
    with_pipes(dimension, |d| {
        if d.nets.mend(pos) {
            d.look.extend((0..6).map(|s| network::offset(pos, s)));
        }
    });
    for face in Direction::ALL {
        let side = face as usize;
        let at = network::offset(pos, side);
        let pipe = with_pipes(dimension, |d| d.nets.cable(at).is_some());
        let acceptor = raw[side] != Connection::None
            && !pipe
            && game.native_fluid_handler(dimension, at, Some(super::mek_world::engine_face(face.opposite()))).is_some();
        let shown = with_pipes(dimension, |d| {
            d.nets.set_acceptor(pos, side, acceptor);
            d.nets.shown(pos, side)
        });
        if game.native_block_property(pos, face.name()).as_deref() != Some(shown.name()) {
            game.native_set_block_property_with(pos, face.name(), shown.name(), flags::UPDATE_CLIENTS);
        }
    }
}

/// `MechanicalPipe.pullFromAcceptors` of every pull side: up to the pipe's
/// pull amount and what its network still takes, of the network's fluid (any
/// while it holds none), when all of it fits.
fn pull(game: &mut Game<'_>, dimension: &str) {
    let sides: Vec<([i32; 3], usize)> = with_pipes(dimension, |d| d.nets.ids().into_iter().flat_map(|id| d.nets.pulls(id)).collect());
    for (pos, side) in sides {
        let (room, content) = with_pipes(dimension, |d| (d.nets.available_pull(pos), d.nets.network_of(pos).and_then(|(_, n)| n.content.clone())));
        if room == 0 {
            continue;
        }
        let face = Some(super::mek_world::engine_face(Direction::ALL[side].opposite()));
        let at = network::offset(pos, side);
        let room = room.min(i32::MAX as u64) as i32;
        let wanted = content.as_deref().map(|id| fluid_of(id, room as u64)).filter(|s| !s.is_empty());
        let received = game.native_fluid_drain(dimension, at, face, wanted.as_ref(), room, true);
        if received.is_empty() {
            continue;
        }
        let fluid = fluid_key(&received);
        if with_pipes(dimension, |d| d.nets.insert_of(pos, Some(&fluid), received.amount() as u64, true)) > 0 {
            continue;
        }
        let got = game.native_fluid_drain(dimension, at, face, Some(&received), received.amount(), false);
        if !got.is_empty() {
            let fluid = fluid_key(&got);
            with_pipes(dimension, |d| d.nets.insert_of(pos, Some(&fluid), got.amount() as u64, false));
        }
    }
}

/// `FluidNetwork.onUpdate` of every network holding a fluid: `tickEmit`, the
/// buffer split evenly among the handlers beside it that take some.
fn emit(game: &mut Game<'_>, dimension: &str) {
    for id in with_pipes(dimension, |d| d.nets.ids()) {
        let Some(plan) = with_pipes(dimension, |d| d.nets.begin_emit(id)) else {
            continue;
        };
        let Some(content) = plan.content.clone() else {
            with_pipes(dimension, |d| d.nets.finish_emit(id, 0));
            continue;
        };
        let template = fluid_of(&content, 1);
        if template.is_empty() {
            with_pipes(dimension, |d| d.nets.finish_emit(id, 0));
            continue;
        }
        let sent = network::send(plan.buffer.min(i32::MAX as u64), &plan.targets, |(pos, side), amount, simulate| {
            let face = Some(super::mek_world::engine_face(Direction::ALL[side].opposite()));
            let stack = template.copy_with_amount(amount.min(i32::MAX as u64) as i32);
            let filled = game.native_fluid_fill(dimension, network::offset(pos, side), face, &stack, simulate).max(0) as u64;
            amount - filled.min(amount)
        });
        with_pipes(dimension, |d| d.nets.finish_emit(id, sent));
    }
}

/// `native_level_tick_post`: the dimension's pipes, as Mekanism's
/// `TransmitterNetworkRegistry` runs after a tick (with the pipes' own
/// `pullFromAcceptors`).
pub(super) fn level_ticked(game: &mut Game<'_>, dimension: &str) {
    if !ANY.get() {
        return;
    }
    // The pipes' own ticks pull first (`TileEntityMechanicalPipe.onUpdateServer`), before the registry's
    // tick-end commits the new ones: a pipe placed this tick pulls from the next.
    pull(game, dimension);
    let set: Vec<[i32; 3]> = SET.with(|s| std::mem::take(&mut *s.borrow_mut()));
    let looks = with_pipes(dimension, |d| {
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
    for pos in looks {
        refresh(game, dimension, pos);
    }
    emit(game, dimension);
    for pos in with_pipes(dimension, |d| d.nets.take_changed()) {
        game.native_block_entity_changed(pos);
    }
}

// ---- /mek connection, /mek probe --------------------------------------------------------

/// A pipe's side set (the Configurator, `/mek connection`) and saved; none
/// when `pos` holds no pipe.
pub(super) fn set_connection(game: &mut Game<'_>, pos: [i32; 3], face: Direction, connection: Connection) -> Option<Value> {
    let dimension = game.native_dimension();
    game.native_with_mod_block_entity(pos, |m| {
        tier_of(&m.id)?;
        let mut connections = connections_of(&m.nbt);
        connections[face as usize] = connection;
        write_connections(&mut m.nbt, &connections);
        Some(())
    })
    .flatten()?;
    with_pipes(dimension, |d| {
        d.nets.set_connection(pos, face as usize, connection);
        d.look.insert(pos);
        d.look.insert(network::offset(pos, face as usize));
    });
    refresh(game, dimension, pos);
    Some(probe(game, pos))
}

/// What `/mek probe` answers for a pipe.
pub(super) fn probe(game: &mut Game<'_>, pos: [i32; 3]) -> Value {
    let dimension = game.native_dimension();
    let id = game.native_with_mod_block_entity(pos, |m| m.id.clone()).unwrap_or_default();
    let Some(tier) = tier_of(&id) else {
        return json!({ "pos": pos, "id": id, "error": "not a pipe" });
    };
    let faces: serde_json::Map<String, Value> =
        Direction::ALL.iter().map(|&d| (d.name().to_owned(), game.native_block_property(pos, d.name()).unwrap_or_default().into())).collect();
    let (capacity, pull) = settings().pipe(tier);
    with_pipes(dimension, |d| {
        let network = d.nets.network_of(pos).map(|(id, n)| {
            let fluid = n.content.as_deref().map(|c| fluid_of(c, 1).name());
            json!({ "id": id, "pipes": n.cables.len(), "buffer": n.buffer, "fluid": fluid, "capacity": n.capacity, "last_sent": n.last_sent })
        });
        let raw: Vec<&str> = d.nets.cable(pos).map_or(Vec::new(), |c| c.connections.iter().map(|c| c.name()).collect());
        json!({
            "pos": pos, "id": id, "tier": tier.name(), "capacity": capacity, "pull": pull, "network": network,
            "share": d.nets.cable(pos).map(|c| c.share), "saved_share": d.nets.save_share(pos), "connections": raw, "faces": faces,
            "loads": d.loads, "unloads": d.unloads, "networks": d.nets.networks().count(),
        })
    })
}
