//! Universal cable networks (`EnergyNetwork`, a `DynamicBufferedNetwork`, and
//! `TransmitterNetworkRegistry`'s queues): touching cables form one network
//! whose buffer holds as much as all its cables together (`CableTier` capacity
//! each). Energy put into any cable goes into the buffer; each tick the buffer
//! is split among the blocks the cables touch (`tickEmit`). Cables keep their
//! part of the buffer, their share, for saving (`updateSaveShares`); a network
//! made of cables that were saved, merged or split takes their shares back
//! (`absorbBuffer`).
//!
//! The game goes through three steps a tick: cables that came and went are
//! queued as they do and [`Networks::commit`]ted once; a network's emission is
//! [`Networks::begin_emit`], [`send`] with no borrow of this state held (a
//! target may call back into a network's buffer), and [`Networks::finish_emit`].
use crate::split::send_to_acceptors;
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub type Pos = [i32; 3];

/// `Direction` order: down, up, north, south, west, east.
pub const SIDES: [Pos; 6] = [[0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]];

pub fn offset(pos: Pos, side: usize) -> Pos {
    let d = SIDES[side];
    [pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]]
}

/// The side facing back: down and up, north and south, west and east.
pub fn opposite(side: usize) -> usize {
    side ^ 1
}

/// `ConnectionType` of one side of a cable, set with the Configurator; saved by
/// its ordinal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Connection {
    /// Takes energy in and gives it out.
    #[default]
    Normal,
    /// Only gives energy out.
    Push,
    /// Takes energy out of the block on that side (`pullFromAcceptors`).
    Pull,
    /// Not connected.
    None,
}

impl Connection {
    pub const ALL: [Self; 4] = [Self::Normal, Self::Push, Self::Pull, Self::None];

    /// The ordinal, as `connection` saves it.
    pub fn id(self) -> i32 {
        self as i32
    }

    /// `ConnectionType.BY_ID`, which wraps around.
    pub fn by_id(id: i32) -> Self {
        Self::ALL[id.rem_euclid(4) as usize]
    }

    /// The value of the block's property for this side.
    pub fn name(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Push => "push",
            Self::Pull => "pull",
            Self::None => "none",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }

    /// `canAccept`.
    pub fn can_accept(self) -> bool {
        matches!(self, Self::Normal | Self::Pull)
    }

    /// `canSendTo`.
    pub fn can_send_to(self) -> bool {
        matches!(self, Self::Normal | Self::Push)
    }
}

#[derive(Clone, Debug)]
pub struct Cable {
    pub capacity: u64,
    /// The sides as configured (`getConnectionTypesRaw`).
    pub connections: [Connection; 6],
    /// The cable's part of its network's buffer as last written for saving
    /// (`lastWrite`), or the energy it was loaded with.
    pub share: u64,
    /// `currentAcceptorConnections`, a bit per side: a block with energy there.
    acceptors: u8,
    network: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Network {
    pub cables: BTreeSet<Pos>,
    pub buffer: u64,
    pub capacity: u64,
    /// Joules the last emission sent (`prevTransferAmount`).
    pub last_sent: u64,
    /// A cable of each chunk the network is in, to mark them changed.
    chunks: BTreeMap<(i32, i32), Pos>,
    /// Shares are as `buffer` and the cables are now.
    shares_fresh: bool,
    /// Changed since the chunks were last marked.
    changed: bool,
    /// What `outputs` answers, until an acceptor or a cable changes.
    outputs: Option<Vec<(Pos, usize)>>,
}

impl Network {
    pub fn needed(&self) -> u64 {
        self.capacity.saturating_sub(self.buffer)
    }

    fn touched(&mut self) {
        self.shares_fresh = false;
        self.changed = true;
    }
}

fn chunk_of(pos: Pos) -> (i32, i32) {
    (pos[0] >> 4, pos[2] >> 4)
}

/// A cable waiting to join (`registerOrphanTransmitter`).
#[derive(Clone, Copy, Debug)]
struct Joining {
    capacity: u64,
    connections: [Connection; 6],
    share: u64,
}

/// What a network's emission goes through: the buffer, and the sides it may
/// hand energy to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Emission {
    pub buffer: u64,
    pub targets: Vec<(Pos, usize)>,
}

/// `EnergyNetwork.tickEmit`: `buffer` split among the `targets` that take any
/// of it when offered all of it. `insert(target, amount, simulate)` returns
/// what the block did not take. Returns what was sent.
pub fn send(buffer: u64, targets: &[(Pos, usize)], mut insert: impl FnMut((Pos, usize), u64, bool) -> u64) -> u64 {
    if buffer == 0 {
        return 0;
    }
    let takers: Vec<(Pos, usize)> = targets.iter().copied().filter(|t| insert(*t, buffer, true) < buffer).collect();
    send_to_acceptors(takers.len(), buffer, |i, amount, simulate| insert(takers[i], amount, simulate))
}

/// Every loaded cable of a level and the networks they make.
#[derive(Debug, Default)]
pub struct Networks {
    cables: HashMap<Pos, Cable>,
    networks: BTreeMap<u32, Network>,
    next: u32,
    adding: BTreeMap<Pos, Joining>,
    leaving: BTreeSet<Pos>,
}

impl Networks {
    pub fn cable(&self, pos: Pos) -> Option<&Cable> {
        self.cables.get(&pos)
    }

    pub fn cable_positions(&self) -> impl Iterator<Item = Pos> + '_ {
        self.cables.keys().copied()
    }

    pub fn network_of(&self, pos: Pos) -> Option<(u32, &Network)> {
        let id = self.cables.get(&pos)?.network;
        Some((id, self.networks.get(&id)?))
    }

    pub fn networks(&self) -> impl Iterator<Item = (u32, &Network)> {
        self.networks.iter().map(|(id, n)| (*id, n))
    }

    pub fn is_empty(&self) -> bool {
        self.cables.is_empty() && self.adding.is_empty()
    }

    // ---- Coming and going ------------------------------------------------

    /// A cable placed or loaded, with the energy its saved share holds: it
    /// joins at the next [`commit`](Self::commit).
    pub fn queue_add(&mut self, pos: Pos, capacity: u64, connections: [Connection; 6], share: u64) {
        self.adding.insert(pos, Joining { capacity, connections, share });
    }

    /// A cable broken or unloaded: it leaves at the next
    /// [`commit`](Self::commit). If it was still waiting to join, it never does.
    pub fn queue_remove(&mut self, pos: Pos) {
        self.adding.remove(&pos);
        if self.cables.contains_key(&pos) {
            self.leaving.insert(pos);
        }
    }

    /// `takeShare`, as a chunk unloads (`onChunkUnloaded`): what the cable last
    /// wrote to its save leaves the network now, since the save has it. A broken
    /// cable takes nothing; what's left of its network keeps the energy.
    pub fn take_share(&mut self, pos: Pos) {
        let Some(cable) = self.cables.get_mut(&pos) else { return };
        let Some(network) = self.networks.get_mut(&cable.network) else { return };
        cable.share = cable.share.min(network.buffer);
        network.buffer -= cable.share;
        network.touched();
    }

    /// `removeInvalidTransmitters`, then `assignOrphans`: those leaving go, and
    /// those waiting join. Returns the cables around which connections need a
    /// look: those that joined and those beside any that went or came.
    pub fn commit(&mut self) -> Vec<Pos> {
        let mut around = BTreeSet::new();
        let leaving = std::mem::take(&mut self.leaving);
        let mut by_network: BTreeMap<u32, Vec<Pos>> = BTreeMap::new();
        for pos in leaving {
            if let Some(cable) = self.cables.get(&pos) {
                by_network.entry(cable.network).or_default().push(pos);
                for side in 0..6 {
                    around.insert(offset(pos, side));
                }
            }
        }
        for (id, gone) in by_network {
            self.invalidate(id, &gone);
        }
        for (pos, joining) in std::mem::take(&mut self.adding) {
            self.add(pos, joining.capacity, joining.connections, joining.share);
            around.insert(pos);
            for side in 0..6 {
                around.insert(offset(pos, side));
            }
        }
        around.retain(|p| self.cables.contains_key(p));
        around.into_iter().collect()
    }

    /// Whether two neighbouring cables join: neither side is cut.
    fn joined(&self, a: Pos, side: usize) -> bool {
        let b = offset(a, side);
        match (self.cables.get(&a), self.cables.get(&b)) {
            (Some(x), Some(y)) => {
                x.connections[side] != Connection::None && y.connections[opposite(side)] != Connection::None
            }
            _ => false,
        }
    }

    /// A cable joins now, with its share, and the networks it touches become
    /// one: the biggest takes in the others, so a chunk's cables joining a long
    /// line cost what they add and not the line's length each.
    pub fn add(&mut self, pos: Pos, capacity: u64, connections: [Connection; 6], share: u64) {
        if self.cables.contains_key(&pos) {
            let gone = [pos];
            let id = self.cables[&pos].network;
            self.invalidate(id, &gone);
        }
        self.cables.insert(pos, Cable { capacity, connections, share, acceptors: 0, network: u32::MAX });
        let touching: BTreeSet<u32> = (0..6)
            .filter(|&s| self.joined(pos, s))
            .map(|s| self.cables[&offset(pos, s)].network)
            .collect();
        let id = match touching.iter().copied().max_by_key(|id| self.networks.get(id).map_or(0, |n| n.cables.len())) {
            Some(biggest) => {
                for other in touching {
                    if other != biggest {
                        self.merge(biggest, other);
                    }
                }
                biggest
            }
            None => {
                let id = self.next_id();
                self.networks.insert(id, Network::default());
                id
            }
        };
        if let Some(cable) = self.cables.get_mut(&pos) {
            cable.network = id;
        }
        if let Some(network) = self.networks.get_mut(&id) {
            network.cables.insert(pos);
            network.chunks.entry(chunk_of(pos)).or_insert(pos);
            network.capacity = network.capacity.saturating_add(capacity);
            // absorbBuffer, then clampBuffer: a buffer over the capacity is cut.
            network.buffer = network.buffer.saturating_add(share).min(network.capacity);
            network.outputs = None;
            network.touched();
        }
    }

    fn next_id(&mut self) -> u32 {
        let id = self.next;
        self.next = self.next.wrapping_add(1);
        id
    }

    /// `adoptTransmittersAndAcceptorsFrom`: `from` becomes part of `into`.
    fn merge(&mut self, into: u32, from: u32) {
        let Some(other) = self.networks.remove(&from) else { return };
        for pos in &other.cables {
            if let Some(cable) = self.cables.get_mut(pos) {
                cable.network = into;
            }
        }
        if let Some(network) = self.networks.get_mut(&into) {
            network.buffer = network.buffer.saturating_add(other.buffer);
            network.capacity = network.capacity.saturating_add(other.capacity);
            network.cables.extend(other.cables);
            for (chunk, pos) in other.chunks {
                network.chunks.entry(chunk).or_insert(pos);
            }
            network.outputs = None;
            network.touched();
        }
    }

    /// `DynamicNetwork.invalidate`: the cables `gone` leave network `id`. Each
    /// one left takes its share of the whole buffer, up to its capacity, and
    /// the parts still touching become networks of what they took. So breaking
    /// a cable loses energy only when the rest can't hold it.
    fn invalidate(&mut self, id: u32, gone: &[Pos]) {
        let Some(mut network) = self.networks.remove(&id) else {
            for pos in gone {
                self.cables.remove(pos);
            }
            return;
        };
        for pos in gone {
            network.cables.remove(pos);
            self.cables.remove(pos);
        }
        let shares = self.split_buffer(&network);
        let mut left = network.cables;
        while let Some(&start) = left.iter().next() {
            let mut part = Vec::new();
            let mut stack = vec![start];
            left.remove(&start);
            while let Some(p) = stack.pop() {
                part.push(p);
                for s in 0..6 {
                    let q = offset(p, s);
                    if left.contains(&q) && self.joined(p, s) {
                        left.remove(&q);
                        stack.push(q);
                    }
                }
            }
            let buffer = part.iter().map(|p| shares.get(p).copied().unwrap_or(0)).sum();
            self.form(part, buffer);
        }
    }

    fn form(&mut self, cables: Vec<Pos>, buffer: u64) -> u32 {
        let id = self.next_id();
        let mut network = Network::default();
        for pos in cables {
            if let Some(cable) = self.cables.get_mut(&pos) {
                cable.network = id;
                network.capacity = network.capacity.saturating_add(cable.capacity);
                network.chunks.entry(chunk_of(pos)).or_insert(pos);
                network.cables.insert(pos);
            }
        }
        network.buffer = buffer.min(network.capacity);
        network.touched();
        self.networks.insert(id, network);
        id
    }

    /// A side's configuration changed (the Configurator): networks split or
    /// join when it cuts or mends a cable-to-cable side.
    pub fn set_connection(&mut self, pos: Pos, side: usize, connection: Connection) {
        let was_joined = self.joined(pos, side);
        let Some(cable) = self.cables.get_mut(&pos) else { return };
        cable.connections[side] = connection;
        let id = cable.network;
        if let Some(network) = self.networks.get_mut(&id) {
            network.outputs = None;
            network.touched();
        }
        let joined = self.joined(pos, side);
        if was_joined && !joined {
            self.invalidate(id, &[]);
        } else if joined && !was_joined {
            let other = self.cables[&offset(pos, side)].network;
            if other != id {
                self.merge(id, other);
            }
        }
    }

    // ---- What a side does ------------------------------------------------

    /// Says whether a block with energy stands on `side` of the cable. True if
    /// that changed its network's outputs.
    pub fn set_acceptor(&mut self, pos: Pos, side: usize, acceptor: bool) -> bool {
        let Some(cable) = self.cables.get_mut(&pos) else { return false };
        let bit = 1u8 << side;
        if (cable.acceptors & bit != 0) == acceptor {
            return false;
        }
        cable.acceptors ^= bit;
        if let Some(network) = self.networks.get_mut(&cable.network) {
            network.outputs = None;
        }
        true
    }

    /// `getConnectionType`, what the block shows on that side: none when nothing
    /// connects there, normal to another cable, else the side's configuration.
    pub fn shown(&self, pos: Pos, side: usize) -> Connection {
        let Some(cable) = self.cables.get(&pos) else {
            return Connection::None;
        };
        if self.joined(pos, side) {
            Connection::Normal
        } else if cable.acceptors & (1 << side) != 0 {
            cable.connections[side]
        } else {
            Connection::None
        }
    }

    /// `getEnergyContainers(side)`: there is energy on `side` of the cable
    /// unless it is cut there; `None` is the block itself. Stored and capacity
    /// are the network's.
    pub fn exposed(&self, pos: Pos, side: Option<usize>) -> Option<&Network> {
        let cable = self.cables.get(&pos)?;
        if side.is_some_and(|s| cable.connections[s] == Connection::None) {
            return None;
        }
        self.networks.get(&cable.network)
    }

    /// Energy offered through `side` of the cable at `pos` by the block there
    /// (`getInsertPredicate`): what did not fit. Nothing goes in through a side
    /// that doesn't accept, nor without a side.
    pub fn insert_at(&mut self, pos: Pos, side: Option<usize>, amount: u64, simulate: bool) -> u64 {
        match side {
            Some(side) if self.shown(pos, side).can_accept() => self.insert(pos, amount, simulate),
            _ => amount,
        }
    }

    /// Energy drawn through `side` of the cable at `pos` by the block there
    /// (`getExtractPredicate`): what it gave.
    pub fn extract_at(&mut self, pos: Pos, side: Option<usize>, amount: u64, simulate: bool) -> u64 {
        match side {
            Some(side) if self.shown(pos, side).can_send_to() => self.extract(pos, amount, simulate),
            _ => 0,
        }
    }

    /// Energy put into the cable at `pos` (its handler is its network's
    /// buffer): returns what did not fit. While a network emits, its buffer
    /// still holds what it is sending, so what fits is what the full buffer
    /// leaves room for, as in Mekanism.
    pub fn insert(&mut self, pos: Pos, amount: u64, simulate: bool) -> u64 {
        let Some(id) = self.cables.get(&pos).map(|c| c.network) else {
            return amount;
        };
        let Some(network) = self.networks.get_mut(&id) else {
            return amount;
        };
        let taken = amount.min(network.needed());
        if !simulate && taken > 0 {
            network.buffer += taken;
            network.touched();
        }
        amount - taken
    }

    /// Energy taken out of the cable at `pos`'s network: returns what was taken.
    pub fn extract(&mut self, pos: Pos, amount: u64, simulate: bool) -> u64 {
        let Some(network) = self.cables.get(&pos).and_then(|c| self.networks.get_mut(&c.network)) else {
            return 0;
        };
        let taken = amount.min(network.buffer);
        if !simulate && taken > 0 {
            network.buffer -= taken;
            network.touched();
        }
        taken
    }

    /// `getAvailablePull`: what the cable at `pos` may pull now.
    pub fn available_pull(&self, pos: Pos) -> u64 {
        let Some((_, network)) = self.network_of(pos) else { return 0 };
        self.cables[&pos].capacity.min(network.needed())
    }

    /// The blocks network `id` hands energy to: each (cable, side) with a block
    /// there and a configuration that gives out (`canConnectToAcceptor`).
    fn outputs(&self, network: &Network) -> Vec<(Pos, usize)> {
        let mut out = Vec::new();
        for &pos in &network.cables {
            let cable = &self.cables[&pos];
            for side in 0..6 {
                if cable.acceptors & (1 << side) != 0 && cable.connections[side].can_send_to() {
                    out.push((pos, side));
                }
            }
        }
        out
    }

    /// The pull sides of network `id`'s cables with a block there.
    pub fn pulls(&self, id: u32) -> Vec<(Pos, usize)> {
        let Some(network) = self.networks.get(&id) else { return Vec::new() };
        let mut out = Vec::new();
        for &pos in &network.cables {
            let cable = &self.cables[&pos];
            for side in 0..6 {
                if cable.acceptors & (1 << side) != 0 && cable.connections[side] == Connection::Pull {
                    out.push((pos, side));
                }
            }
        }
        out
    }

    pub fn ids(&self) -> Vec<u32> {
        self.networks.keys().copied().collect()
    }

    // ---- Emission --------------------------------------------------------

    /// `EnergyNetwork.onUpdate`'s start: what to send, none with an empty
    /// buffer. The buffer stays as it is until [`finish_emit`](Self::finish_emit).
    pub fn begin_emit(&mut self, id: u32) -> Option<Emission> {
        let network = self.networks.get(&id)?;
        if network.buffer == 0 {
            if let Some(network) = self.networks.get_mut(&id) {
                network.last_sent = 0;
            }
            return None;
        }
        let buffer = network.buffer;
        let targets = match &network.outputs {
            Some(targets) => targets.clone(),
            None => {
                let targets = self.outputs(network);
                if let Some(network) = self.networks.get_mut(&id) {
                    network.outputs = Some(targets.clone());
                }
                targets
            }
        };
        Some(Emission { buffer, targets })
    }

    /// What was sent leaves the buffer; what came in during the sending stays.
    pub fn finish_emit(&mut self, id: u32, sent: u64) {
        if let Some(network) = self.networks.get_mut(&id) {
            network.last_sent = sent;
            if sent > 0 {
                network.buffer = network.buffer.saturating_sub(sent);
                network.touched();
            }
        }
    }

    // ---- Saving ----------------------------------------------------------

    /// `EnergyTransmitterSaveTarget`: the buffer split evenly among the
    /// network's cables, none over its capacity.
    fn split_buffer(&self, network: &Network) -> HashMap<Pos, u64> {
        let cables: Vec<Pos> = network.cables.iter().copied().collect();
        let mut held = vec![0u64; cables.len()];
        send_to_acceptors(cables.len(), network.buffer, |i, offered, simulate| {
            let room = self.cables[&cables[i]].capacity - held[i];
            let taken = offered.min(room);
            if !simulate {
                held[i] += taken;
            }
            offered - taken
        });
        cables.into_iter().zip(held).collect()
    }

    /// `validateSaveShares` then `lastWrite`: what the cable at `pos` saves.
    /// Worked out once for its whole network however many cables ask, until
    /// something changes. None for a cable that is only waiting to join, whose
    /// save stays as it was loaded.
    pub fn save_share(&mut self, pos: Pos) -> Option<u64> {
        let id = self.cables.get(&pos)?.network;
        if let Some(network) = self.networks.get(&id)
            && !network.shares_fresh
        {
            let shares = self.split_buffer(network);
            for (p, share) in shares {
                if let Some(cable) = self.cables.get_mut(&p) {
                    cable.share = share;
                }
            }
            if let Some(network) = self.networks.get_mut(&id) {
                network.shares_fresh = true;
            }
        }
        Some(self.cables[&pos].share)
    }

    /// `markDirty`: a cable in each chunk of each network that changed since
    /// the last call, for their chunks to be saved next time.
    pub fn take_changed(&mut self) -> Vec<Pos> {
        let mut out = Vec::new();
        for network in self.networks.values_mut() {
            if std::mem::take(&mut network.changed) {
                out.extend(network.chunks.values().copied());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: [Connection; 6] = [Connection::Normal; 6];
    const EAST: usize = 5;
    const WEST: usize = 4;

    /// A whole emission of network `id` as the game runs it.
    fn emit(nets: &mut Networks, id: u32, insert: impl FnMut((Pos, usize), u64, bool) -> u64) -> u64 {
        let Some(plan) = nets.begin_emit(id) else { return 0 };
        let sent = send(plan.buffer, &plan.targets, insert);
        nets.finish_emit(id, sent);
        sent
    }

    #[test]
    fn touching_cables_share_one_buffer() {
        let mut nets = Networks::default();
        nets.add([0, 0, 0], 8_000, N, 0);
        nets.add([2, 0, 0], 8_000, N, 5_000);
        assert_eq!(nets.networks().count(), 2);
        // The middle cable joins them: one network of three cables.
        nets.add([1, 0, 0], 128_000, N, 1_000);
        assert_eq!(nets.networks().count(), 1);
        let (_, net) = nets.network_of([0, 0, 0]).unwrap();
        assert_eq!((net.capacity, net.buffer), (144_000, 6_000));
        assert_eq!(nets.insert([2, 0, 0], 200_000, false), 62_000);
        assert_eq!(nets.network_of([0, 0, 0]).unwrap().1.buffer, 144_000);
    }

    #[test]
    fn breaking_a_cable_keeps_what_the_rest_can_hold() {
        // A full line: only 8 000 each is left when the 128 000 cable goes.
        let mut nets = Networks::default();
        nets.add([0, 0, 0], 8_000, N, 8_000);
        nets.add([1, 0, 0], 128_000, N, 128_000);
        nets.add([2, 0, 0], 8_000, N, 8_000);
        nets.queue_remove([1, 0, 0]);
        let around = nets.commit();
        assert_eq!(around, vec![[0, 0, 0], [2, 0, 0]]);
        assert_eq!(nets.networks().count(), 2);
        assert_eq!(nets.network_of([0, 0, 0]).unwrap().1.buffer, 8_000);
        assert_eq!(nets.network_of([2, 0, 0]).unwrap().1.buffer, 8_000);
        // Half full: nothing is lost, as the even split fits in what's left.
        let mut nets = Networks::default();
        nets.add([0, 0, 0], 8_000, N, 0);
        nets.add([1, 0, 0], 8_000, N, 0);
        nets.add([2, 0, 0], 8_000, N, 6_000);
        nets.queue_remove([1, 0, 0]);
        nets.commit();
        assert_eq!(nets.network_of([0, 0, 0]).unwrap().1.buffer, 3_000);
        assert_eq!(nets.network_of([2, 0, 0]).unwrap().1.buffer, 3_000);
    }

    #[test]
    fn an_unloaded_cable_takes_what_its_save_holds() {
        let mut nets = Networks::default();
        for x in 0..3 {
            nets.add([x, 0, 0], 8_000, N, 0);
        }
        nets.insert([0, 0, 0], 9_000, false);
        // The chunk is saved, then unloads; more arrives before the commit.
        assert_eq!(nets.save_share([2, 0, 0]), Some(3_000));
        nets.take_share([2, 0, 0]);
        nets.queue_remove([2, 0, 0]);
        assert_eq!(nets.network_of([0, 0, 0]).unwrap().1.buffer, 6_000, "at once");
        nets.insert([0, 0, 0], 100, false);
        nets.commit();
        assert_eq!(nets.network_of([0, 0, 0]).unwrap().1.buffer, 6_100);
        // It comes back with it: nothing lost or made.
        nets.queue_add([2, 0, 0], 8_000, N, 3_000);
        nets.commit();
        let (_, net) = nets.network_of([2, 0, 0]).unwrap();
        assert_eq!((net.buffer, net.capacity), (9_100, 24_000));
    }

    #[test]
    fn shares_survive_a_save_and_load_of_every_cable() {
        let mut nets = Networks::default();
        for x in 0..4 {
            nets.add([x, 0, 0], 8_000, N, 0);
        }
        nets.insert([0, 0, 0], 10_003, false);
        let shares: Vec<u64> = (0..4).map(|x| nets.save_share([x, 0, 0]).unwrap()).collect();
        assert_eq!(shares.iter().sum::<u64>(), 10_003);
        assert_eq!(shares.iter().max().unwrap() - shares.iter().min().unwrap(), 1, "even");
        let mut loaded = Networks::default();
        for (x, share) in shares.iter().enumerate() {
            loaded.queue_add([x as i32, 0, 0], 8_000, N, *share);
        }
        loaded.commit();
        assert_eq!(loaded.networks().count(), 1);
        assert_eq!(loaded.network_of([3, 0, 0]).unwrap().1.buffer, 10_003);
        // A smaller tier configured since: clampBuffer cuts it.
        let mut smaller = Networks::default();
        for (x, share) in shares.iter().enumerate() {
            smaller.queue_add([x as i32, 0, 0], 1_000, N, *share);
        }
        smaller.commit();
        assert_eq!(smaller.network_of([0, 0, 0]).unwrap().1.buffer, 4_000);
    }

    #[test]
    fn shares_are_worked_out_again_only_after_a_change() {
        let mut nets = Networks::default();
        nets.add([0, 0, 0], 8_000, N, 0);
        nets.add([1, 0, 0], 8_000, N, 0);
        nets.insert([0, 0, 0], 9_001, false);
        assert_eq!(nets.save_share([0, 0, 0]), Some(4_501));
        assert_eq!(nets.save_share([1, 0, 0]), Some(4_500));
        nets.insert([0, 0, 0], 999, false);
        assert_eq!(nets.save_share([1, 0, 0]), Some(5_000));
        assert_eq!(nets.save_share([9, 9, 9]), None);
    }

    #[test]
    fn queues_commit_removals_before_additions() {
        let mut nets = Networks::default();
        // Placed and gone before a commit: it never joins.
        nets.queue_add([0, 0, 0], 8_000, N, 0);
        nets.queue_remove([0, 0, 0]);
        assert!(nets.commit().is_empty());
        assert_eq!(nets.networks().count(), 0);
        // A chunk reloaded between commits: out with the old, in with the new.
        nets.queue_add([0, 0, 0], 8_000, N, 500);
        nets.commit();
        nets.save_share([0, 0, 0]);
        nets.take_share([0, 0, 0]);
        nets.queue_remove([0, 0, 0]);
        nets.queue_add([0, 0, 0], 8_000, N, 500);
        assert_eq!(nets.commit(), vec![[0, 0, 0]]);
        assert_eq!(nets.network_of([0, 0, 0]).unwrap().1.buffer, 500);
        // Unloaded for good.
        nets.save_share([0, 0, 0]);
        nets.take_share([0, 0, 0]);
        nets.queue_remove([0, 0, 0]);
        nets.commit();
        assert!(nets.is_empty());
    }

    #[test]
    fn many_chunks_leaving_at_once_split_one_network_once() {
        let mut nets = Networks::default();
        for x in 0..48 {
            nets.add([x, 0, 0], 8_000, N, 1_000);
        }
        assert_eq!(nets.network_of([0, 0, 0]).unwrap().1.chunks.len(), 3);
        // The middle chunk (16..32) unloads, each taking its 1000.
        for x in 16..32 {
            assert_eq!(nets.save_share([x, 0, 0]), Some(1_000));
        }
        for x in 16..32 {
            nets.take_share([x, 0, 0]);
            nets.queue_remove([x, 0, 0]);
        }
        nets.commit();
        assert_eq!(nets.networks().count(), 2);
        assert_eq!(nets.network_of([0, 0, 0]).unwrap().1.buffer, 16_000);
        assert_eq!(nets.network_of([47, 0, 0]).unwrap().1.buffer, 16_000);
        // And back: the line is whole, with all its energy.
        for x in 16..32 {
            nets.queue_add([x, 0, 0], 8_000, N, 1_000);
        }
        nets.commit();
        assert_eq!(nets.networks().count(), 1);
        assert_eq!(nets.network_of([0, 0, 0]).unwrap().1.buffer, 48_000);
    }

    #[test]
    fn a_cut_side_keeps_networks_apart_and_outputs_skip_cables() {
        let mut nets = Networks::default();
        let mut cut = N;
        cut[EAST] = Connection::None;
        nets.add([0, 0, 0], 8_000, cut, 0);
        nets.add([1, 0, 0], 8_000, N, 2_000);
        assert_eq!(nets.networks().count(), 2);
        assert_eq!(nets.shown([0, 0, 0], EAST), Connection::None);
        nets.set_connection([0, 0, 0], EAST, Connection::Normal);
        assert_eq!(nets.networks().count(), 1);
        assert_eq!(nets.network_of([0, 0, 0]).unwrap().1.buffer, 2_000, "joined");
        assert_eq!(nets.shown([0, 0, 0], EAST), Connection::Normal);
        // Cut again: each side keeps its even part.
        nets.set_connection([1, 0, 0], WEST, Connection::None);
        assert_eq!(nets.networks().count(), 2);
        assert_eq!(nets.network_of([0, 0, 0]).unwrap().1.buffer, 1_000);
        assert_eq!(nets.network_of([1, 0, 0]).unwrap().1.buffer, 1_000);
    }

    #[test]
    fn a_side_shows_and_passes_what_it_is_configured_to() {
        let mut nets = Networks::default();
        let mut sides = N;
        sides[0] = Connection::Pull;
        sides[1] = Connection::Push;
        sides[2] = Connection::None;
        nets.add([0, 0, 0], 8_000, sides, 100);
        // Nothing beside it: no connection shows and no energy passes there yet.
        assert_eq!(nets.shown([0, 0, 0], 0), Connection::None);
        assert_eq!(nets.insert_at([0, 0, 0], Some(3), 50, false), 50);
        for side in 0..6 {
            nets.set_acceptor([0, 0, 0], side, true);
        }
        assert_eq!(nets.shown([0, 0, 0], 0), Connection::Pull);
        assert_eq!(nets.shown([0, 0, 0], 1), Connection::Push);
        assert_eq!(nets.shown([0, 0, 0], 2), Connection::None);
        assert_eq!(nets.shown([0, 0, 0], 3), Connection::Normal);
        // Into: normal and pull. Out of: normal and push. None without a side.
        assert_eq!(nets.insert_at([0, 0, 0], Some(0), 10, false), 0);
        assert_eq!(nets.insert_at([0, 0, 0], Some(1), 10, false), 10);
        assert_eq!(nets.insert_at([0, 0, 0], Some(3), 10, false), 0);
        assert_eq!(nets.insert_at([0, 0, 0], None, 10, false), 10);
        assert_eq!(nets.extract_at([0, 0, 0], Some(0), 10, false), 0);
        assert_eq!(nets.extract_at([0, 0, 0], Some(1), 10, false), 10);
        assert_eq!(nets.extract_at([0, 0, 0], None, 10, false), 0);
        assert!(nets.exposed([0, 0, 0], Some(2)).is_none(), "no capability on a cut side");
        assert_eq!(nets.exposed([0, 0, 0], None).unwrap().buffer, 110);
        assert_eq!(nets.pulls(nets.network_of([0, 0, 0]).unwrap().0), vec![([0, 0, 0], 0)]);
        assert_eq!(nets.available_pull([0, 0, 0]), 7_890);
    }

    #[test]
    fn emission_splits_the_buffer_among_takers() {
        let mut nets = Networks::default();
        nets.add([0, 0, 0], 8_000, N, 3_000);
        nets.set_acceptor([0, 0, 0], WEST, true);
        nets.set_acceptor([0, 0, 0], EAST, true);
        nets.set_acceptor([0, 0, 0], 0, true);
        let (id, _) = nets.network_of([0, 0, 0]).unwrap();
        let mut room = HashMap::from([(([0, 0, 0], WEST), 1_000u64), (([0, 0, 0], EAST), 10_000), (([0, 0, 0], 0), 0)]);
        let sent = emit(&mut nets, id, |t, amount, simulate| {
            let r = room.get(&t).copied().unwrap_or(0);
            let taken = amount.min(r);
            if !simulate && taken > 0 {
                room.insert(t, r - taken);
            }
            amount - taken
        });
        assert_eq!(sent, 3_000);
        assert_eq!(room[&([0, 0, 0], WEST)], 0);
        assert_eq!(room[&([0, 0, 0], EAST)], 8_000);
        let net = nets.network_of([0, 0, 0]).unwrap().1;
        assert_eq!((net.buffer, net.last_sent), (0, 3_000));
    }

    #[test]
    fn an_insert_during_emission_lands_in_the_buffer() {
        let mut nets = Networks::default();
        nets.add([0, 0, 0], 8_000, N, 3_000);
        nets.set_acceptor([0, 0, 0], EAST, true);
        let (id, _) = nets.network_of([0, 0, 0]).unwrap();
        let plan = nets.begin_emit(id).unwrap();
        assert_eq!(plan, Emission { buffer: 3_000, targets: vec![([0, 0, 0], EAST)] });
        // No borrow is held: the target hands 6000 back into this network, of
        // which what the still-counted 3000 leaves room for fits.
        let mut back = 0;
        let sent = send(plan.buffer, &plan.targets, |_, amount, simulate| {
            if !simulate {
                back = 6_000 - nets.insert([0, 0, 0], 6_000, false);
            }
            let _ = amount;
            0
        });
        assert_eq!((sent, back), (3_000, 5_000));
        nets.finish_emit(id, sent);
        assert_eq!(nets.network_of([0, 0, 0]).unwrap().1.buffer, 5_000);
    }

    #[test]
    fn outputs_follow_acceptors_and_configuration() {
        let mut nets = Networks::default();
        nets.add([0, 0, 0], 8_000, N, 100);
        nets.add([1, 0, 0], 8_000, N, 0);
        let (id, _) = nets.network_of([0, 0, 0]).unwrap();
        assert_eq!(nets.begin_emit(id).unwrap().targets, vec![], "nothing touches them");
        assert!(nets.set_acceptor([1, 0, 0], EAST, true));
        assert!(!nets.set_acceptor([1, 0, 0], EAST, true));
        assert_eq!(nets.begin_emit(id).unwrap().targets, vec![([1, 0, 0], EAST)]);
        nets.set_connection([1, 0, 0], EAST, Connection::Pull);
        assert_eq!(nets.begin_emit(id).unwrap().targets, vec![], "pull doesn't give out");
        nets.set_connection([1, 0, 0], EAST, Connection::Push);
        assert_eq!(nets.begin_emit(id).unwrap().targets, vec![([1, 0, 0], EAST)]);
    }

    #[test]
    fn changes_mark_each_chunk_of_the_network_once() {
        let mut nets = Networks::default();
        for x in 14..18 {
            nets.add([x, 0, 0], 8_000, N, 0);
        }
        assert_eq!(nets.take_changed().len(), 2, "two chunks");
        assert!(nets.take_changed().is_empty());
        nets.insert([14, 0, 0], 10, true);
        assert!(nets.take_changed().is_empty(), "a simulation changes nothing");
        nets.insert([14, 0, 0], 10, false);
        assert_eq!(nets.take_changed().len(), 2);
        let (id, _) = nets.network_of([14, 0, 0]).unwrap();
        nets.finish_emit(id, 0);
        assert!(nets.take_changed().is_empty(), "nothing sent");
    }

    #[test]
    fn connections_are_saved_by_ordinal() {
        assert_eq!(Connection::ALL.map(Connection::id), [0, 1, 2, 3]);
        assert_eq!(Connection::by_id(2), Connection::Pull);
        assert_eq!(Connection::by_id(7), Connection::None, "BY_ID wraps");
        assert_eq!(Connection::from_name("push"), Some(Connection::Push));
    }
}
