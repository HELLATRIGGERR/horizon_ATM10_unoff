// Mekanism's Pressurized Tubes in the game (support module `crate::game::mek_tubes`
// of the `mekanism` native bundle). The rules are the bundle's crate
// (`mekanism_energy::network`, the Universal Cables' networks with a chemical,
// MIT, from Mekanism 10.7.19, Copyright (c) 2017-2025 Aidan C. Brady); this
// module is the glue, as `mek_world` is the cables': the tubes of each
// dimension and their networks kept here through the block entity lifecycle
// hook points (API 156), each level's tick committing them, looking at their
// sides (a block with a chemical handler on the touching side, `IChemicalHandler`
// by `mek_chemicals::with_handler`), joining networks that may now meet,
// pulling through pull sides (first, as the tubes' own ticks do), emitting to
// the handlers beside them with the even split, and marking their chunks changed; a tube's share of its network
// saved as `boxed_chemical`; the network as a chemical handler on each side
// for the blocks around it; `/mek connection` and `/mek probe`. No borrow of
// this state is held across a call into the game.

use super::{Game, mek_chemicals, mek_world::key};
use horizon_world::{
    level::flags,
    nbt::{Compound, Tag},
};
use mekanism_energy::{
    chemical::{ChemicalHandler, ChemicalStack},
    network::{self, Connection, Member, Networks},
    sides::Direction,
    tier::TubeTier,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeSet, HashMap},
};

/// A dimension's tubes.
#[derive(Default)]
struct DimTubes {
    nets: Networks,
    /// Tubes whose sides need a look.
    look: BTreeSet<[i32; 3]>,
    /// Ticks until every tube gets a look: a neighbour's handler can change
    /// with no block changing (its side configuration).
    full_look_in: u32,
    loads: u64,
    unloads: u64,
}

thread_local! {
    static TUBES: RefCell<HashMap<String, DimTubes>> = RefCell::new(HashMap::new());
    static ANY: Cell<bool> = const { Cell::new(false) };
    /// Blocks set since the last level tick, for the tubes beside them.
    static SET: RefCell<Vec<[i32; 3]>> = const { RefCell::new(Vec::new()) };
}

fn with_tubes<R>(dimension: &str, change: impl FnOnce(&mut DimTubes) -> R) -> R {
    TUBES.with(|t| {
        let mut t = t.borrow_mut();
        if !t.contains_key(dimension) {
            t.insert(dimension.to_owned(), DimTubes::default());
        }
        change(t.get_mut(dimension).expect("just made"))
    })
}

pub(super) fn tier_of(block: &str) -> Option<TubeTier> {
    TubeTier::of_block(block)
}

/// A world opens or closes: no tubes.
pub(super) fn clear() {
    TUBES.with(|t| t.borrow_mut().clear());
    ANY.set(false);
    SET.with(|s| s.borrow_mut().clear());
}

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

pub(super) fn is_tube(dimension: &str, pos: [i32; 3]) -> bool {
    ANY.get() && with_tubes(dimension, |d| d.nets.cable(pos).is_some())
}

/// `boxed_chemical` (`ChemicalStack.save`): the chemical and its share, as
/// saved. Whether the registry knows it is asked once the network forms.
fn saved_share(nbt: &Compound) -> (u64, Option<String>) {
    let Some(Tag::Compound(boxed)) = nbt.get(&key("boxed_chemical")) else {
        return (0, None);
    };
    let id = match boxed.get(&key("id")) {
        Some(Tag::String(s)) => mekanism_energy::chemical::full_id(&s.to_string_lossy()),
        _ => return (0, None),
    };
    match super::mek_world::number(boxed.get(&key("amount"))) {
        Some(n) if n > 0 && id != mekanism_energy::chemical::EMPTY => (n as u64, Some(id)),
        _ => (0, None),
    }
}

/// `native_block_entity_load` of a tube: it queues to join with its share
/// (`registerOrphanTransmitter`).
pub(super) fn loaded(dimension: &str, pos: [i32; 3], nbt: &Compound, tier: TubeTier) {
    let (config, _) = super::mek_world::energy_config();
    let (capacity, pull) = config.tube(tier);
    let (share, content) = saved_share(nbt);
    let member = Member { capacity: capacity.max(0) as u64, pull: pull.max(0) as u64, connections: connections_of(nbt), share, content };
    with_tubes(dimension, |d| {
        d.nets.queue_add_with(pos, member);
        d.loads += 1;
    });
    ANY.set(true);
}

/// `native_block_entity_unload` of a tube: unloading, it takes its saved
/// share with it; broken, the rest of its network keeps the chemical.
pub(super) fn unloaded(dimension: &str, pos: [i32; 3], removed: bool) {
    with_tubes(dimension, |d| {
        if !removed {
            d.nets.take_share(pos);
        }
        d.nets.queue_remove(pos);
        d.unloads += 1;
    });
}

/// A block with chemicals beside tubes came or went: they look again.
pub(super) fn neighbour(dimension: &str, pos: [i32; 3]) {
    if ANY.get() {
        with_tubes(dimension, |d| d.look.extend((0..6).map(|s| network::offset(pos, s))));
    }
}

/// `native_block_changed`.
pub(super) fn block_set(pos: [i32; 3]) {
    if ANY.get() {
        SET.with(|s| s.borrow_mut().push(pos));
    }
}

/// `native_block_entity_save` of a tube: its share of its network as
/// `boxed_chemical` (`PressurizedTube.write`), left out when none.
pub(super) fn saved(dimension: &str, pos: [i32; 3], nbt: &mut Compound) {
    let share = with_tubes(dimension, |d| {
        let share = d.nets.save_share(pos)?;
        Some((share, d.nets.network_of(pos).and_then(|(_, n)| n.content.clone())))
    });
    match share {
        Some((share, Some(id))) if share > 0 => {
            let mut boxed = Compound::new();
            boxed.insert(key("id"), Tag::String(key(&id)));
            boxed.insert(key("amount"), Tag::Long(share.min(i64::MAX as u64) as i64));
            nbt.insert(key("boxed_chemical"), Tag::Compound(boxed));
        }
        Some(_) => {
            nbt.remove(&key("boxed_chemical"));
        }
        None => {}
    }
}

// ---- The handler ------------------------------------------------------------------

/// A tube's chemical handler on one side (`TileEntityPressurizedTube`'s
/// `DynamicChemicalHandler` over its network's tank): what comes in on a
/// side that accepts (normal, pull) and goes out of one that sends (normal,
/// push), of the network's one chemical; the null side reads only.
struct TubeSide<'a> {
    nets: &'a mut Networks,
    pos: [i32; 3],
    side: Option<usize>,
}

impl TubeSide<'_> {
    fn held(&self) -> (u64, u64, Option<String>) {
        self.nets.network_of(self.pos).map_or((0, 0, None), |(_, n)| (n.buffer, n.capacity, n.content.clone()))
    }
}

impl ChemicalHandler for TubeSide<'_> {
    fn tanks(&self) -> usize {
        1
    }

    fn in_tank(&self, _: usize) -> ChemicalStack {
        match self.held() {
            (amount, _, Some(id)) if amount > 0 => ChemicalStack::new(&id, amount.min(i64::MAX as u64) as i64),
            _ => ChemicalStack::EMPTY,
        }
    }

    fn capacity(&self, _: usize) -> i64 {
        self.held().1.min(i64::MAX as u64) as i64
    }

    fn is_valid(&self, _: usize, _: &ChemicalStack) -> bool {
        true
    }

    fn insert_tank(&mut self, _: usize, stack: &ChemicalStack, simulate: bool) -> ChemicalStack {
        if stack.is_empty() {
            return ChemicalStack::EMPTY;
        }
        let rest = self.nets.insert_of_at(self.pos, self.side, stack.id(), stack.amount() as u64, simulate);
        stack.copy_with_amount(rest.min(i64::MAX as u64) as i64)
    }

    fn extract_tank(&mut self, _: usize, amount: i64, simulate: bool) -> ChemicalStack {
        let held = self.in_tank(0);
        if held.is_empty() || amount < 1 {
            return ChemicalStack::EMPTY;
        }
        let got = self.nets.extract_at(self.pos, self.side, amount as u64, simulate);
        held.copy_with_amount(got.min(i64::MAX as u64) as i64)
    }
}

/// `Capabilities.CHEMICAL.block()` of the tube at `pos` through `face`: none
/// on a side cut with the Configurator.
pub(super) fn with_handler<R>(dimension: &str, pos: [i32; 3], face: Option<Direction>, use_it: impl FnOnce(&mut dyn ChemicalHandler) -> R) -> Option<R> {
    let side = face.map(|f| f as usize);
    with_tubes(dimension, |d| {
        d.nets.exposed(pos, side)?;
        let mut handler = TubeSide { nets: &mut d.nets, pos, side };
        Some(use_it(&mut handler))
    })
}

// ---- The level's tick --------------------------------------------------------------

/// `Transmitter.refreshConnections` and `recheckConnection`: the tube at
/// `pos` joins the networks beside it that may now meet its own, and each
/// side shows a connection to a block with a chemical handler there.
fn refresh(game: &mut Game<'_>, dimension: &str, pos: [i32; 3]) {
    let Some(raw) = with_tubes(dimension, |d| d.nets.cable(pos).map(|c| c.connections)) else {
        return;
    };
    with_tubes(dimension, |d| {
        if d.nets.mend(pos) {
            d.look.extend((0..6).map(|s| network::offset(pos, s)));
        }
    });
    for face in Direction::ALL {
        let side = face as usize;
        let at = network::offset(pos, side);
        let tube = with_tubes(dimension, |d| d.nets.cable(at).is_some());
        let acceptor = raw[side] != Connection::None && !tube && mek_chemicals::with_handler(game, at, Some(face.opposite()), |_| ()).is_some();
        let shown = with_tubes(dimension, |d| {
            d.nets.set_acceptor(pos, side, acceptor);
            d.nets.shown(pos, side)
        });
        if game.native_block_property(pos, face.name()).as_deref() != Some(shown.name()) {
            game.native_set_block_property_with(pos, face.name(), shown.name(), flags::UPDATE_CLIENTS);
        }
    }
}

/// `PressurizedTube.pullFromAcceptors` of every pull side: up to the tube's
/// pull amount and what its network still takes, of the network's chemical
/// (any while it holds none), when all of it fits.
fn pull(game: &mut Game<'_>, dimension: &str) {
    let sides: Vec<([i32; 3], usize)> = with_tubes(dimension, |d| d.nets.ids().into_iter().flat_map(|id| d.nets.pulls(id)).collect());
    for (pos, side) in sides {
        let (room, content) = with_tubes(dimension, |d| (d.nets.available_pull(pos), d.nets.network_of(pos).and_then(|(_, n)| n.content.clone())));
        if room == 0 {
            continue;
        }
        let face = Direction::ALL[side];
        let at = network::offset(pos, side);
        let room = room.min(i64::MAX as u64) as i64;
        let received = mek_chemicals::with_handler(game, at, Some(face.opposite()), |h| match &content {
            None => h.extract(room, true),
            Some(id) => h.extract_stack(&ChemicalStack::new(id, room), true),
        })
        .unwrap_or(ChemicalStack::EMPTY);
        if received.is_empty() || with_tubes(dimension, |d| d.nets.insert_of(pos, received.id(), received.amount() as u64, true)) > 0 {
            continue;
        }
        let got = mek_chemicals::with_handler(game, at, Some(face.opposite()), |h| h.extract_stack(&received, false)).unwrap_or(ChemicalStack::EMPTY);
        if !got.is_empty() {
            with_tubes(dimension, |d| d.nets.insert_of(pos, got.id(), got.amount() as u64, false));
        }
    }
}

/// `ChemicalNetwork.onUpdate` of every network holding a chemical: `tickEmit`,
/// the buffer split evenly among the handlers beside it that take some.
fn emit(game: &mut Game<'_>, dimension: &str) {
    for id in with_tubes(dimension, |d| d.nets.ids()) {
        let Some(plan) = with_tubes(dimension, |d| d.nets.begin_emit(id)) else {
            continue;
        };
        let Some(content) = plan.content.clone() else {
            with_tubes(dimension, |d| d.nets.finish_emit(id, 0));
            continue;
        };
        let sent = network::send(plan.buffer, &plan.targets, |(pos, side), amount, simulate| {
            let face = Direction::ALL[side];
            let stack = ChemicalStack::new(&content, amount.min(i64::MAX as u64) as i64);
            mek_chemicals::with_handler(game, network::offset(pos, side), Some(face.opposite()), |h| h.insert(&stack, simulate).amount().max(0) as u64)
                .unwrap_or(amount)
        });
        with_tubes(dimension, |d| d.nets.finish_emit(id, sent));
    }
}

/// `native_level_tick_post`: the dimension's tubes, as Mekanism's
/// `TransmitterNetworkRegistry` runs after a tick (with the tubes' own
/// `pullFromAcceptors`).
pub(super) fn level_ticked(game: &mut Game<'_>, dimension: &str) {
    if !ANY.get() {
        return;
    }
    // The tubes' own ticks pull first (`TileEntityPressurizedTube.onUpdateServer`), before the
    // registry's tick-end commits the new ones: a tube placed this tick pulls from the next.
    pull(game, dimension);
    let set: Vec<[i32; 3]> = SET.with(|s| std::mem::take(&mut *s.borrow_mut()));
    let looks = with_tubes(dimension, |d| {
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
    // A saved share of a chemical the registry lacks reads as nothing (`parseOptional`).
    let registry = mek_chemicals::registry(game);
    with_tubes(dimension, |d| {
        let unknown: Vec<[i32; 3]> = d
            .nets
            .networks()
            .filter(|(_, n)| n.content.as_deref().is_some_and(|c| registry.get(c).is_none()))
            .filter_map(|(_, n)| n.cables.iter().next().copied())
            .collect();
        for pos in unknown {
            d.nets.extract(pos, u64::MAX, false);
        }
    });
    for pos in looks {
        refresh(game, dimension, pos);
    }
    emit(game, dimension);
    for pos in with_tubes(dimension, |d| d.nets.take_changed()) {
        game.native_block_entity_changed(pos);
    }
}

// ---- /mek connection, /mek probe --------------------------------------------------------

/// `/mek connection` on a tube: its side set and saved; none when `pos` holds no tube.
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
    with_tubes(dimension, |d| {
        d.nets.set_connection(pos, face as usize, connection);
        d.look.insert(pos);
        d.look.insert(network::offset(pos, face as usize));
    });
    refresh(game, dimension, pos);
    Some(probe(game, pos))
}

/// What `/mek probe` answers for a tube.
pub(super) fn probe(game: &mut Game<'_>, pos: [i32; 3]) -> Value {
    let dimension = game.native_dimension();
    let id = game.native_with_mod_block_entity(pos, |m| m.id.clone()).unwrap_or_default();
    let Some(tier) = tier_of(&id) else {
        return json!({ "pos": pos, "id": id, "error": "not a tube" });
    };
    let faces: serde_json::Map<String, Value> =
        Direction::ALL.iter().map(|&d| (d.name().to_owned(), game.native_block_property(pos, d.name()).unwrap_or_default().into())).collect();
    let (config, _) = super::mek_world::energy_config();
    let (capacity, pull) = config.tube(tier);
    with_tubes(dimension, |d| {
        let network = d.nets.network_of(pos).map(|(id, n)| {
            json!({ "id": id, "tubes": n.cables.len(), "buffer": n.buffer, "chemical": n.content, "capacity": n.capacity, "last_sent": n.last_sent })
        });
        let raw: Vec<&str> = d.nets.cable(pos).map_or(Vec::new(), |c| c.connections.iter().map(|c| c.name()).collect());
        json!({
            "pos": pos, "id": id, "tier": tier.name(), "capacity": capacity, "pull": pull, "network": network,
            "share": d.nets.cable(pos).map(|c| c.share), "saved_share": d.nets.save_share(pos), "connections": raw, "faces": faces,
            "loads": d.loads, "unloads": d.unloads, "networks": d.nets.networks().count(),
        })
    })
}
