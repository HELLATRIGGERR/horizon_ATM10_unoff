//! The live networks of a world: `ServerFluxNetwork`'s device lists and its
//! transfer cycle, with the devices `TileFluxDevice` keeps in the world. The
//! cycle calls into the game for each neighbour, and the game may call back
//! (a neighbour that forwards energy into a plug), so the world sits in a
//! `RefCell` and no borrow is held across a call to the `Host`.
use crate::{
    config::FluxConfig,
    networks::{NO_NETWORK, Networks},
    transfer::{Handler, Kind, Saved},
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
};

/// The six sides, in `Direction.get3DDataValue` order: down, up, north, south,
/// west, east. A device's `connections` has a bit for each.
pub const SIDES: usize = 6;

/// A device's place: an index into `FluxWorld::dimensions` and a position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub dimension: u8,
    pub pos: [i32; 3],
}

/// `TileFluxDevice`, for as long as its chunk is loaded.
#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub handler: Handler,
    /// The network it runs on, or `NO_NETWORK`.
    pub network: i32,
    /// `mNetworkID` as saved: a network that doesn't exist counts as none.
    pub saved_network: i32,
    pub name: String,
    pub owner: u128,
    /// The energy in its saved compound, as last read or written: a compound
    /// that holds anything else was replaced (`/data`, or a new block entity).
    /// Only a write that reached the compound moves it.
    pub saved_energy: i64,
    /// The sides with a neighbour that has FE (`SIDES_CONNECTED_MASK`).
    pub connections: u8,
}

/// `ServerFluxNetwork`'s lists; a network with no loaded device has none.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Live {
    /// `getLogicalDevices(ANY)`, in the order they joined.
    pub all: Vec<Key>,
    /// Plugs and points, by falling logical priority once sorted.
    pub plugs: Vec<Key>,
    pub points: Vec<Key>,
    /// `mSortConnections`.
    pub sort: bool,
    /// `mBufferLimiter`: what all its points still want, which bounds what
    /// plugs take in the next tick. 0 during the cycle.
    pub limiter: i64,
}

/// A readback for `IEnergyStorage`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Info {
    pub stored: i64,
    pub capacity: i64,
    pub can_receive: bool,
}

/// What the cycle needs from the game.
pub trait Host {
    /// `SideTransfer.send`: offers `amount` to the neighbour on `side` of the
    /// point at `pos`, if it takes FE there: what it took. At most an `int` on
    /// each call, as `ForgeEnergyConnector.sendTo`.
    fn send(&mut self, dimension: &str, pos: [i32; 3], side: usize, amount: i64, simulate: bool) -> i64;
}

#[derive(Clone, Debug, PartialEq)]
pub struct FluxWorld {
    pub networks: Networks,
    pub config: FluxConfig,
    pub dimensions: Vec<String>,
    pub devices: HashMap<Key, Device>,
    pub live: BTreeMap<i32, Live>,
}

impl FluxWorld {
    pub fn new(networks: Networks, config: FluxConfig) -> Self {
        Self { networks, config, dimensions: Vec::new(), devices: HashMap::new(), live: BTreeMap::new() }
    }

    /// The index of a dimension, added at first sight.
    pub fn dimension_id(&mut self, name: &str) -> u8 {
        if let Some(i) = self.dimensions.iter().position(|d| d == name) {
            return i as u8;
        }
        self.dimensions.push(name.to_owned());
        (self.dimensions.len() - 1) as u8
    }

    /// The key of a position, when its dimension has been seen.
    pub fn key(&self, dimension: &str, pos: [i32; 3]) -> Option<Key> {
        let i = self.dimensions.iter().position(|d| d == dimension)?;
        Some(Key { dimension: i as u8, pos })
    }

    /// A device's tick (`TileFluxDevice.onServerTick`): the first makes it
    /// join its saved network (`onFirstTick`). Each takes the settings of its
    /// saved compound, so `/data` and a replaced block entity act at once,
    /// and where its neighbours with FE are.
    pub fn observe(&mut self, key: Key, kind: Kind, saved: &Saved, connections: u8) {
        if self.devices.get(&key).is_some_and(|d| d.handler.kind != kind) {
            self.remove(key);
        }
        let fresh = !self.devices.contains_key(&key);
        let device = self.devices.entry(key).or_insert_with(|| Device {
            handler: Handler::new(kind, saved.limit),
            network: NO_NETWORK,
            saved_network: saved.network,
            name: String::new(),
            owner: 0,
            saved_energy: saved.energy,
            connections,
        });
        if fresh {
            device.handler.buffer = saved.energy;
        } else if saved.energy != device.saved_energy {
            device.handler.buffer = saved.energy;
            device.saved_energy = saved.energy;
        }
        let order = device.handler.apply(saved);
        device.name.clone_from(&saved.name);
        device.owner = saved.owner;
        device.connections = connections;
        let (network, joined) = (device.network, device.saved_network == saved.network);
        if fresh || !joined {
            self.connect(key, saved.network);
        } else if order && let Some(live) = self.live.get_mut(&network) {
            live.sort = true;
        }
    }

    /// `TileFluxDevice.connect`: leaves its network and joins `network`; one
    /// that doesn't exist is none. It keeps its buffer (`onNetworkChanged`
    /// clears only `mChange`).
    pub fn connect(&mut self, key: Key, network: i32) {
        let exists = self.networks.get(network).is_some();
        let Some(device) = self.devices.get_mut(&key) else {
            return;
        };
        device.saved_network = network;
        let target = if exists { network } else { NO_NETWORK };
        if device.network == target {
            return;
        }
        let old = std::mem::replace(&mut device.network, target);
        device.handler.change = 0;
        let kind = device.handler.kind;
        self.leave(old, key);
        if target != NO_NETWORK {
            let live = self.live.entry(target).or_default();
            live.all.push(key);
            match kind {
                Kind::Plug => live.plugs.push(key),
                Kind::Point => live.points.push(key),
            }
            live.sort = true;
        }
    }

    fn leave(&mut self, network: i32, key: Key) {
        if let Some(live) = self.live.get_mut(&network) {
            live.all.retain(|k| *k != key);
            live.plugs.retain(|k| *k != key);
            live.points.retain(|k| *k != key);
            if live.all.is_empty() {
                self.live.remove(&network);
            }
        }
    }

    /// `setRemoved` or `onChunkUnloaded`: the device leaves the world. What it
    /// must keep is in its compound: what came in went there at once, what the
    /// cycle moved at the last tick's end.
    pub fn remove(&mut self, key: Key) {
        if let Some(device) = self.devices.remove(&key) {
            self.leave(device.network, key);
        }
    }

    /// Before each cycle (`handleConnectionQueue`), what the game finds of each
    /// device's block entity: the energy in its compound, or none. A device
    /// whose block entity is gone leaves, so it can't give away what its dropped
    /// item took. One still there stays, ticking or not, as a block entity does
    /// until its chunk unloads. A compound that changed wins: a new block entity
    /// at the place, or `/data`.
    pub fn reconcile(&mut self, mut compound_energy: impl FnMut(Key, Kind) -> Option<i64>) {
        let devices: Vec<(Key, Kind)> = self.devices.iter().map(|(key, d)| (*key, d.handler.kind)).collect();
        for (key, kind) in devices {
            match compound_energy(key, kind) {
                None => self.remove(key),
                Some(energy) => {
                    if let Some(device) = self.devices.get_mut(&key)
                        && energy != device.saved_energy
                    {
                        device.handler.buffer = energy;
                        device.saved_energy = energy;
                    }
                }
            }
        }
    }

    /// `receiveEnergy` of a plug on a network; nothing for any other device.
    pub fn plug_receive(&mut self, key: Key, max: i64, simulate: bool) -> i64 {
        let Some(device) = self.devices.get_mut(&key) else {
            return 0;
        };
        let Some(live) = self.live.get(&device.network) else {
            return 0;
        };
        if device.handler.kind != Kind::Plug {
            return 0;
        }
        device.handler.plug_receive(max, live.limiter, simulate)
    }

    /// `getEnergyStored` and friends of a device: a plug takes energy while it
    /// is on a network, and neither gives any out of its block.
    pub fn info(&self, key: Key) -> Option<Info> {
        let device = self.devices.get(&key)?;
        Some(Info {
            stored: device.handler.buffer,
            capacity: device.handler.capacity(),
            can_receive: device.handler.kind == Kind::Plug && device.network != NO_NETWORK,
        })
    }

    /// `canPlayerAccess` of a device: anyone while it is on no network, then
    /// its owner and whoever may use the network.
    pub fn can_access(&self, key: Key, player: u128) -> bool {
        let Some(device) = self.devices.get(&key) else {
            return true;
        };
        let Some(network) = self.networks.get(device.network) else {
            return true;
        };
        device.owner == player || network.access(player).can_use()
    }

    /// The buffers that differ from the devices' compounds, to write there.
    /// Each stays listed until `energy_saved` says its write arrived, so a write
    /// that fails is tried again and can't roll the buffer back.
    pub fn changed_energy(&self) -> Vec<(Key, i64)> {
        let changed = self.devices.iter().filter(|(_, d)| d.handler.buffer != d.saved_energy);
        changed.map(|(key, d)| (*key, d.handler.buffer)).collect()
    }

    /// The device's compound now holds `energy`.
    pub fn energy_saved(&mut self, key: Key, energy: i64) {
        if let Some(device) = self.devices.get_mut(&key) {
            device.saved_energy = energy;
        }
    }
}

/// The start of `onEndServerTick`: the lists in order, the limiter at 0, and
/// the devices to go through.
fn begin(world: &RefCell<FluxWorld>, id: i32) -> Vec<Key> {
    let mut guard = world.borrow_mut();
    let w = &mut *guard;
    let Some(live) = w.live.get_mut(&id) else {
        return Vec::new();
    };
    live.limiter = 0;
    if std::mem::take(&mut live.sort) {
        // A stable sort, as `List.sort`.
        let devices = &w.devices;
        let by = |k: &Key| std::cmp::Reverse(devices.get(k).map_or(i32::MIN, |d| d.handler.logical_priority()));
        live.plugs.sort_by_key(by);
        live.points.sort_by_key(by);
    }
    live.all.clone()
}

/// `FluxPointHandler.sendToConsumers`.
fn send_to_consumers(world: &RefCell<FluxWorld>, host: &mut impl Host, key: Key, energy: i64, simulate: bool) -> i64 {
    let (dimension, connections) = {
        let w = world.borrow();
        let Some(device) = w.devices.get(&key) else {
            return 0;
        };
        (w.dimensions[key.dimension as usize].clone(), device.connections)
    };
    let mut leftover = energy;
    for side in 0..SIDES {
        if connections & (1 << side) == 0 {
            continue;
        }
        leftover -= host.send(&dimension, key.pos, side, leftover, simulate);
        if leftover <= 0 {
            return energy;
        }
    }
    energy - leftover
}

/// The plugs' energy into the points, as `onEndServerTick`'s two iterators
/// walk them: points by priority, each served from the plugs by priority,
/// a plug moved past once it has nothing it may give.
fn internal_cycle(world: &RefCell<FluxWorld>, id: i32) {
    let mut guard = world.borrow_mut();
    let w = &mut *guard;
    let Some(live) = w.live.get(&id) else {
        return;
    };
    let (mut plug, mut point) = (0, 0);
    loop {
        while let Some(key) = live.points.get(point) {
            if w.devices.get(key).is_some_and(|d| d.handler.request() > 0) {
                break;
            }
            point += 1;
        }
        while let Some(key) = live.plugs.get(plug) {
            if w.devices.get(key).is_some_and(|d| d.handler.buffer > 0) {
                break;
            }
            plug += 1;
        }
        let (Some(point_key), Some(plug_key)) = (live.points.get(point), live.plugs.get(plug)) else {
            return;
        };
        let request = w.devices[point_key].handler.request();
        let Some(source) = w.devices.get_mut(plug_key) else {
            return;
        };
        let actual = source.handler.plug_remove(request);
        if actual > 0 {
            if let Some(sink) = w.devices.get_mut(point_key) {
                sink.handler.buffer += actual;
            }
        } else {
            // It has energy, but reached its limit: the next plug.
            plug += 1;
        }
    }
}

/// `ServerFluxNetwork.onEndServerTick` of one network.
fn tick_network(world: &RefCell<FluxWorld>, id: i32, host: &mut impl Host) {
    let all = begin(world, id);
    // onCycleStart: what the points' neighbours would take, simulated.
    for &key in &all {
        let limit = {
            let w = world.borrow();
            let Some(device) = w.devices.get(&key) else {
                continue;
            };
            if device.handler.kind != Kind::Point {
                continue;
            }
            device.handler.logical_limit()
        };
        let desired = send_to_consumers(world, host, key, limit, true);
        if let Some(device) = world.borrow_mut().devices.get_mut(&key) {
            device.handler.desired = desired;
        }
    }
    internal_cycle(world, id);
    // onCycleEnd: points hand on what they hold; what they still want next
    // tick bounds what plugs take.
    let mut limiter = 0_i64;
    for &key in &all {
        let amount = {
            let mut w = world.borrow_mut();
            let Some(device) = w.devices.get_mut(&key) else {
                continue;
            };
            match device.handler.kind {
                Kind::Plug => {
                    device.handler.plug_end();
                    None
                }
                Kind::Point => Some(device.handler.buffer.min(device.handler.logical_limit())),
            }
        };
        if let Some(amount) = amount {
            let sent = send_to_consumers(world, host, key, amount, false);
            if let Some(device) = world.borrow_mut().devices.get_mut(&key) {
                device.handler.buffer -= sent;
                device.handler.change = -sent;
            }
        }
        if let Some(device) = world.borrow().devices.get(&key) {
            limiter = limiter.saturating_add(device.handler.request());
        }
    }
    if let Some(live) = world.borrow_mut().live.get_mut(&id) {
        live.limiter = limiter;
    }
}

/// `FluxNetworkData.getAllNetworks().forEach(onEndServerTick)`, at the end of
/// a server tick.
pub fn tick_all(world: &RefCell<FluxWorld>, host: &mut impl Host) {
    let ids: Vec<i32> = world.borrow().live.keys().copied().collect();
    for id in ids {
        tick_network(world, id, host);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::networks::Networks;

    const OVERWORLD: &str = "minecraft:overworld";

    fn saved(network: i32) -> Saved {
        Saved {
            network,
            name: String::new(),
            owner: 1,
            priority: 0,
            surge: false,
            limit: 800_000,
            disable_limit: false,
            energy: 0,
        }
    }

    /// Neighbours by point and side: what each would take in a tick, and what
    /// each has taken.
    #[derive(Default)]
    struct Sinks {
        room: HashMap<([i32; 3], usize), i64>,
        taken: HashMap<([i32; 3], usize), i64>,
        /// The order of real sends.
        order: Vec<[i32; 3]>,
    }

    impl Host for Sinks {
        fn send(&mut self, _: &str, pos: [i32; 3], side: usize, amount: i64, simulate: bool) -> i64 {
            let room = self.room.entry((pos, side)).or_insert(0);
            // An `int` on the way out, as `ForgeEnergyConnector.sendTo`.
            let op = amount.min(i64::from(i32::MAX)).min(*room);
            if !simulate && op > 0 {
                *room -= op;
                *self.taken.entry((pos, side)).or_insert(0) += op;
                self.order.push(pos);
            }
            op
        }
    }

    struct Rig {
        world: RefCell<FluxWorld>,
        sinks: Sinks,
        net: i32,
    }

    impl Rig {
        fn new() -> Self {
            let mut world = FluxWorld::new(Networks::new(0), FluxConfig::default());
            let net = world.networks.create_default(1, "Alex", 5).unwrap();
            Self { world: RefCell::new(world), sinks: Sinks::default(), net }
        }

        fn key(&self, n: i32) -> Key {
            Key { dimension: self.world.borrow_mut().dimension_id(OVERWORLD), pos: [n, 0, 0] }
        }

        fn add(&self, n: i32, kind: Kind, change: impl FnOnce(&mut Saved)) -> Key {
            let key = self.key(n);
            let mut s = saved(self.net);
            change(&mut s);
            self.world.borrow_mut().observe(key, kind, &s, if kind == Kind::Point { 1 } else { 0 });
            key
        }

        fn room(&mut self, key: Key, room: i64) {
            self.sinks.room.insert((key.pos, 0), room);
        }

        fn taken(&self, key: Key) -> i64 {
            self.sinks.taken.get(&(key.pos, 0)).copied().unwrap_or(0)
        }

        fn tick(&mut self) {
            tick_all(&self.world, &mut self.sinks);
        }

        fn receive(&self, key: Key, max: i64) -> i64 {
            self.world.borrow_mut().plug_receive(key, max, false)
        }

        fn buffer(&self, key: Key) -> i64 {
            self.world.borrow().devices[&key].handler.buffer
        }

        fn change(&self, key: Key) -> i64 {
            self.world.borrow().devices[&key].handler.change
        }

        fn limiter(&self) -> i64 {
            self.world.borrow().live[&self.net].limiter
        }

        /// A tick of the world: the sources push, then the cycle runs. What
        /// each point's neighbour took in it.
        fn tick_with(&mut self, sources: &[(Key, i64)], points: &[Key]) -> Vec<i64> {
            let before: Vec<i64> = points.iter().map(|p| self.taken(*p)).collect();
            for (plug, amount) in sources {
                self.receive(*plug, *amount);
            }
            self.tick();
            points.iter().zip(before).map(|(p, b)| self.taken(*p) - b).collect()
        }
    }

    #[test]
    fn energy_flows_with_one_tick_of_lag_at_the_rate_offered() {
        let mut r = Rig::new();
        let plug = r.add(0, Kind::Plug, |_| ());
        let point = r.add(1, Kind::Point, |_| ());
        r.room(point, i64::MAX);
        // The first tick: no request known yet, so the plug refuses.
        assert_eq!(r.receive(plug, 1000), 0);
        r.tick();
        assert_eq!(r.limiter(), 800_000, "the point wants its limit");
        // Then what comes in leaves in the same tick's end.
        assert_eq!(r.tick_with(&[(plug, 1000)], &[point]), [1000]);
        assert_eq!(r.buffer(plug), 0);
        assert_eq!(r.buffer(point), 0);
        assert_eq!(r.change(plug), 1000);
        assert_eq!(r.change(point), -1000);
        assert_eq!(r.tick_with(&[(plug, 1000)], &[point]), [1000]);
    }

    #[test]
    fn the_plug_takes_no_more_than_the_limiter_says() {
        let mut r = Rig::new();
        let plug = r.add(0, Kind::Plug, |_| ());
        let point = r.add(1, Kind::Point, |_| ());
        r.room(point, 300);
        r.tick();
        assert_eq!(r.limiter(), 300);
        assert_eq!(r.receive(plug, 1000), 300);
        r.tick();
        assert_eq!(r.taken(point), 300);
        // The limiter is what the point wanted at the cycle's start, less what
        // it holds, as `getRequest`: the plug takes one more tick's worth...
        assert_eq!(r.limiter(), 300);
        assert_eq!(r.receive(plug, 1000), 300);
        // ...which stays in its buffer, as the full neighbour now wants nothing.
        r.tick();
        assert_eq!(r.limiter(), 0);
        assert_eq!(r.buffer(plug), 300);
        assert_eq!(r.receive(plug, 1000), 0);
    }

    #[test]
    fn a_plugs_limit_caps_what_it_takes_and_gives() {
        let mut r = Rig::new();
        let plug = r.add(0, Kind::Plug, |s| s.limit = 600);
        let point = r.add(1, Kind::Point, |_| ());
        r.room(point, i64::MAX);
        r.tick();
        assert_eq!(r.tick_with(&[(plug, 1000)], &[point]), [600]);
        assert_eq!(r.change(plug), 600);
    }

    #[test]
    fn a_points_limit_caps_what_it_gives_and_wants() {
        let mut r = Rig::new();
        let plug = r.add(0, Kind::Plug, |_| ());
        let point = r.add(1, Kind::Point, |s| s.limit = 250);
        r.room(point, i64::MAX);
        r.tick();
        assert_eq!(r.limiter(), 250);
        assert_eq!(r.tick_with(&[(plug, 1000)], &[point]), [250]);
    }

    #[test]
    fn a_limit_of_zero_stops_a_device() {
        let mut r = Rig::new();
        let plug = r.add(0, Kind::Plug, |_| ());
        let point = r.add(1, Kind::Point, |s| s.limit = 0);
        r.room(point, i64::MAX);
        r.tick();
        assert_eq!(r.limiter(), 0);
        assert_eq!(r.tick_with(&[(plug, 1000)], &[point]), [0]);
    }

    #[test]
    fn higher_priority_points_are_served_first() {
        let mut r = Rig::new();
        let plug = r.add(0, Kind::Plug, |_| ());
        let low = r.add(1, Kind::Point, |_| ());
        let high = r.add(2, Kind::Point, |s| s.priority = 5);
        r.room(low, i64::MAX);
        r.room(high, 700);
        r.tick();
        // The high point has room for 700 in all: it gets what it wants first.
        assert_eq!(r.tick_with(&[(plug, 1000)], &[high, low]), [700, 300]);
        assert_eq!(r.tick_with(&[(plug, 1000)], &[high, low]), [0, 1000]);
    }

    #[test]
    fn a_limit_on_the_first_point_lets_the_rest_through() {
        let mut r = Rig::new();
        let plug = r.add(0, Kind::Plug, |_| ());
        let low = r.add(1, Kind::Point, |_| ());
        let high = r.add(2, Kind::Point, |s| {
            s.priority = 5;
            s.limit = 300;
        });
        r.room(low, i64::MAX);
        r.room(high, i64::MAX);
        r.tick();
        assert_eq!(r.tick_with(&[(plug, 1000)], &[high, low]), [300, 700]);
    }

    #[test]
    fn surge_beats_any_priority() {
        let mut r = Rig::new();
        let plug = r.add(0, Kind::Plug, |_| ());
        let surge = r.add(1, Kind::Point, |s| s.surge = true);
        let high = r.add(2, Kind::Point, |s| s.priority = 9999);
        r.room(surge, i64::MAX);
        r.room(high, i64::MAX);
        r.tick();
        assert_eq!(r.tick_with(&[(plug, 1000)], &[surge, high]), [1000, 0]);
    }

    #[test]
    fn equal_priorities_keep_the_order_they_joined() {
        let mut r = Rig::new();
        let plug = r.add(0, Kind::Plug, |_| ());
        let first = r.add(1, Kind::Point, |_| ());
        let second = r.add(2, Kind::Point, |_| ());
        r.room(first, i64::MAX);
        r.room(second, i64::MAX);
        r.tick();
        assert_eq!(r.tick_with(&[(plug, 1000)], &[first, second]), [1000, 0]);
    }

    #[test]
    fn bypass_ignores_the_limit_and_amounts_pass_the_int_range() {
        let mut r = Rig::new();
        let plug = r.add(0, Kind::Plug, |s| s.disable_limit = true);
        let point = r.add(1, Kind::Point, |s| s.disable_limit = true);
        r.sinks.room.insert((point.pos, 0), i64::MAX);
        // A second neighbour: Java's `int` per side is what caps a tick.
        let two = 0b11;
        let s = saved(r.net);
        let mut s2 = s.clone();
        s2.disable_limit = true;
        r.world.borrow_mut().observe(point, Kind::Point, &s2, two);
        r.sinks.room.insert((point.pos, 1), i64::MAX);
        r.tick();
        assert_eq!(r.limiter(), 2 * i64::from(i32::MAX));
        let amount = 3_000_000_000_i64;
        assert_eq!(r.receive(plug, amount), amount);
        r.tick();
        assert_eq!(r.buffer(point), 0);
        assert_eq!(r.taken(point) + r.sinks.taken[&(point.pos, 1)], amount);
    }

    #[test]
    fn two_plugs_feed_three_points() {
        let mut r = Rig::new();
        let a = r.add(0, Kind::Plug, |s| s.limit = 500);
        let b = r.add(1, Kind::Plug, |s| s.limit = 800);
        let p1 = r.add(2, Kind::Point, |s| {
            s.priority = 3;
            s.limit = 400;
        });
        let p2 = r.add(3, Kind::Point, |s| {
            s.priority = 2;
            s.limit = 400;
        });
        let p3 = r.add(4, Kind::Point, |s| {
            s.priority = 1;
            s.limit = 1000;
        });
        for p in [p1, p2, p3] {
            r.room(p, i64::MAX);
        }
        r.tick();
        assert_eq!(r.limiter(), 1800);
        // 500 + 700 = 1200 in: 400, 400, then the rest.
        assert_eq!(r.tick_with(&[(a, 500), (b, 700)], &[p1, p2, p3]), [400, 400, 400]);
        assert_eq!(r.buffer(a) + r.buffer(b), 0);
        assert_eq!((r.change(a), r.change(b)), (500, 700));
    }

    #[test]
    fn a_plug_at_its_limit_gives_way_to_the_next_within_a_cycle() {
        let mut r = Rig::new();
        let a = r.add(0, Kind::Plug, |s| {
            s.priority = 1;
            s.limit = 100;
        });
        let b = r.add(1, Kind::Plug, |_| ());
        let p = r.add(2, Kind::Point, |_| ());
        r.room(p, i64::MAX);
        r.tick();
        // More in a's buffer than its limit lets out (a limit lowered after it filled).
        r.world.borrow_mut().devices.get_mut(&a).unwrap().handler.buffer = 300;
        assert_eq!(r.tick_with(&[(b, 50)], &[p]), [150], "100 of a's, then b's 50");
        assert_eq!(r.buffer(a), 200);
    }

    #[test]
    fn what_a_neighbour_refuses_stays_in_the_point_and_lowers_its_request() {
        let mut r = Rig::new();
        let plug = r.add(0, Kind::Plug, |_| ());
        let point = r.add(1, Kind::Point, |_| ());
        r.room(point, 1000);
        r.tick();
        assert_eq!(r.limiter(), 1000);
        assert_eq!(r.receive(plug, 1000), 1000);
        // The neighbour fills up by itself before the cycle.
        r.sinks.room.insert((point.pos, 0), 0);
        r.tick();
        // Simulated before the cycle: it wants nothing, so the point gets nothing.
        assert_eq!(r.buffer(plug), 1000);
        r.room(point, 400);
        r.tick();
        assert_eq!(r.taken(point), 400);
        assert_eq!(r.buffer(plug), 600);
        r.world.borrow_mut().devices.get_mut(&point).unwrap().handler.buffer = 50;
        r.room(point, 30);
        r.tick();
        assert_eq!(r.buffer(point), 20, "50 held, 30 taken");
        assert_eq!(r.limiter(), 10, "it wanted 30 at the cycle's start, and holds 20");
    }

    #[test]
    fn a_device_on_no_network_takes_nothing_and_keeps_its_buffer() {
        let mut r = Rig::new();
        let plug = r.add(0, Kind::Plug, |_| ());
        let point = r.add(1, Kind::Point, |_| ());
        r.room(point, i64::MAX);
        r.tick();
        assert_eq!(r.receive(plug, 500), 500);
        r.world.borrow_mut().connect(plug, NO_NETWORK);
        assert_eq!(r.receive(plug, 500), 0);
        assert_eq!(r.buffer(plug), 500);
        assert!(!r.world.borrow().info(plug).unwrap().can_receive);
        assert_eq!(r.tick_with(&[], &[point]), [0]);
        // Back on the network, it gives what it kept.
        r.world.borrow_mut().connect(plug, r.net);
        assert!(r.world.borrow().info(plug).unwrap().can_receive);
        assert_eq!(r.tick_with(&[], &[point]), [500]);
    }

    #[test]
    fn a_saved_network_that_does_not_exist_counts_as_none() {
        let mut r = Rig::new();
        let key = r.add(0, Kind::Plug, |s| s.network = 12345);
        let w = r.world.borrow();
        assert_eq!(w.devices[&key].network, NO_NETWORK);
        assert_eq!(w.devices[&key].saved_network, 12345);
        assert!(w.live.is_empty());
        drop(w);
        assert_eq!(r.receive(key, 10), 0);
        r.tick();
    }

    #[test]
    fn devices_in_other_networks_are_apart() {
        let mut r = Rig::new();
        let other = r.world.borrow_mut().networks.create_default(1, "Alex", 5).unwrap();
        let plug = r.add(0, Kind::Plug, |_| ());
        let point = r.add(1, Kind::Point, |s| s.network = other);
        r.room(point, i64::MAX);
        r.tick();
        assert_eq!(r.limiter(), 0, "this network has no point");
        assert_eq!(r.receive(plug, 500), 0);
        assert_eq!(r.world.borrow().live[&other].limiter, 800_000);
    }

    #[test]
    fn a_removed_device_is_gone_from_the_cycle() {
        let mut r = Rig::new();
        let plug = r.add(0, Kind::Plug, |_| ());
        let a = r.add(1, Kind::Point, |_| ());
        let b = r.add(2, Kind::Point, |_| ());
        r.room(a, i64::MAX);
        r.room(b, i64::MAX);
        r.tick();
        r.world.borrow_mut().remove(a);
        assert_eq!(r.tick_with(&[(plug, 1000)], &[b]), [1000]);
        r.world.borrow_mut().remove(plug);
        r.world.borrow_mut().remove(b);
        assert!(r.world.borrow().live.is_empty());
        r.tick();
    }

    #[test]
    fn a_device_whose_block_entity_is_gone_leaves_before_the_cycle() {
        let mut r = Rig::new();
        let plug = r.add(0, Kind::Plug, |_| ());
        let point = r.add(1, Kind::Point, |_| ());
        r.room(point, i64::MAX);
        r.world.borrow_mut().reconcile(|key, _| (key != point).then_some(0));
        assert!(!r.world.borrow().devices.contains_key(&point));
        assert_eq!(r.world.borrow().live[&r.net].points, []);
        r.tick();
        assert_eq!(r.limiter(), 0, "no point, nothing wanted");
        assert_eq!(r.receive(plug, 500), 0);
    }

    #[test]
    fn a_device_that_stops_ticking_stays_while_its_block_entity_is_there() {
        let mut r = Rig::new();
        let plug = r.add(0, Kind::Plug, |_| ());
        let point = r.add(1, Kind::Point, |_| ());
        r.room(point, i64::MAX);
        r.tick();
        // No `observe` from here on: their chunks are loaded and don't tick.
        for _ in 0..3 {
            r.world.borrow_mut().reconcile(|_, _| Some(0));
            assert_eq!(r.tick_with(&[(plug, 1000)], &[point]), [1000]);
        }
        assert_eq!(r.world.borrow().devices.len(), 2);
    }

    #[test]
    fn a_broken_plug_gives_nothing_of_what_its_item_took() {
        let mut r = Rig::new();
        let plug = r.add(0, Kind::Plug, |_| ());
        let point = r.add(1, Kind::Point, |_| ());
        r.room(point, i64::MAX);
        r.tick();
        assert_eq!(r.receive(plug, 400), 400);
        // Broken after its tick and before the cycle: the item has the 400.
        r.world.borrow_mut().reconcile(|key, _| (key != plug).then_some(0));
        assert_eq!(r.tick_with(&[], &[point]), [0]);
    }

    #[test]
    fn a_new_block_entity_at_the_place_wins_before_the_cycle() {
        let mut r = Rig::new();
        let plug = r.add(0, Kind::Plug, |_| ());
        let point = r.add(1, Kind::Point, |_| ());
        r.room(point, i64::MAX);
        r.tick();
        assert_eq!(r.receive(plug, 400), 400);
        r.world.borrow_mut().energy_saved(plug, 400);
        // Replaced by a new plug within the tick: its compound has no energy.
        r.world.borrow_mut().reconcile(|_, _| Some(0));
        assert_eq!(r.buffer(plug), 0);
        assert_eq!(r.tick_with(&[], &[point]), [0]);
    }

    #[test]
    fn a_compound_that_changed_wins_over_the_buffer() {
        let mut r = Rig::new();
        let plug = r.add(0, Kind::Plug, |_| ());
        r.world.borrow_mut().devices.get_mut(&plug).unwrap().handler.buffer = 40;
        assert_eq!(r.world.borrow().changed_energy(), [(plug, 40)]);
        r.world.borrow_mut().energy_saved(plug, 40);
        // The next tick's compound carries it: nothing happens.
        let mut s = saved(r.net);
        s.energy = 40;
        r.world.borrow_mut().observe(plug, Kind::Plug, &s, 0);
        assert_eq!(r.buffer(plug), 40);
        // A new block entity at the place (no energy): it starts empty.
        s.energy = 0;
        r.world.borrow_mut().observe(plug, Kind::Plug, &s, 0);
        assert_eq!(r.buffer(plug), 0);
        assert!(r.world.borrow().changed_energy().is_empty());
    }

    #[test]
    fn a_write_that_fails_leaves_the_buffer_and_is_tried_again() {
        let mut r = Rig::new();
        let plug = r.add(0, Kind::Plug, |_| ());
        r.world.borrow_mut().devices.get_mut(&plug).unwrap().handler.buffer = 40;
        assert_eq!(r.world.borrow().changed_energy(), [(plug, 40)]);
        // No `energy_saved`: the compound still holds 0, and nobody reads that
        // as a change.
        r.world.borrow_mut().observe(plug, Kind::Plug, &saved(r.net), 0);
        assert_eq!(r.buffer(plug), 40);
        r.world.borrow_mut().reconcile(|_, _| Some(0));
        assert_eq!(r.buffer(plug), 40);
        assert_eq!(r.world.borrow().changed_energy(), [(plug, 40)], "tried again");
        r.world.borrow_mut().energy_saved(plug, 40);
        assert!(r.world.borrow().changed_energy().is_empty());
    }

    #[test]
    fn a_compound_of_another_kind_starts_a_new_device() {
        let mut r = Rig::new();
        let key = r.add(0, Kind::Plug, |_| ());
        r.world.borrow_mut().devices.get_mut(&key).unwrap().handler.buffer = 40;
        let s = saved(r.net);
        r.world.borrow_mut().observe(key, Kind::Point, &s, 1);
        let w = r.world.borrow();
        assert_eq!(w.devices[&key].handler.kind, Kind::Point);
        assert_eq!(w.devices[&key].handler.buffer, 0);
        assert_eq!(w.live[&r.net].plugs, []);
        assert_eq!(w.live[&r.net].points, [key]);
    }

    #[test]
    fn a_changed_priority_sorts_again() {
        let mut r = Rig::new();
        let plug = r.add(0, Kind::Plug, |_| ());
        let a = r.add(1, Kind::Point, |_| ());
        let b = r.add(2, Kind::Point, |_| ());
        r.room(a, i64::MAX);
        r.room(b, i64::MAX);
        r.tick();
        assert_eq!(r.tick_with(&[(plug, 1000)], &[a, b]), [1000, 0]);
        let mut s = saved(r.net);
        s.priority = 7;
        r.world.borrow_mut().observe(b, Kind::Point, &s, 1);
        assert_eq!(r.tick_with(&[(plug, 1000)], &[a, b]), [0, 1000]);
    }

    /// A plug offered energy during the cycle (a neighbour of a point that
    /// forwards it back) takes none: the limiter is 0 until the cycle ends.
    #[test]
    fn a_plug_refuses_energy_during_the_cycle() {
        struct Loop<'a> {
            world: &'a RefCell<FluxWorld>,
            plug: Key,
            got: Vec<i64>,
        }
        impl Host for Loop<'_> {
            fn send(&mut self, _: &str, _: [i32; 3], _: usize, amount: i64, _: bool) -> i64 {
                self.got.push(self.world.borrow_mut().plug_receive(self.plug, amount, false));
                0
            }
        }
        let r = Rig::new();
        let plug = r.add(0, Kind::Plug, |_| ());
        let point = r.add(1, Kind::Point, |_| ());
        r.world.borrow_mut().live.get_mut(&r.net).unwrap().limiter = 1000;
        assert_eq!(r.world.borrow_mut().plug_receive(plug, 10, true), 10);
        let mut host = Loop { world: &r.world, plug, got: Vec::new() };
        tick_all(&r.world, &mut host);
        assert!(host.got.iter().all(|&moved| moved == 0), "{:?}", host.got);
        assert!(!host.got.is_empty());
        let _ = point;
    }

    #[test]
    fn access_is_the_owners_and_the_networks() {
        let r = Rig::new();
        let key = r.add(0, Kind::Plug, |s| s.owner = 2);
        let free = r.add(1, Kind::Plug, |s| s.network = NO_NETWORK);
        let w = r.world.borrow();
        assert!(w.can_access(key, 2), "the device's owner");
        assert!(w.can_access(key, 1), "the network's owner");
        assert!(!w.can_access(key, 3), "a stranger, on a private network");
        assert!(w.can_access(free, 3), "no network protects nothing");
    }

    #[test]
    fn the_readback_is_the_buffer_and_the_limit() {
        let r = Rig::new();
        let plug = r.add(0, Kind::Plug, |s| s.limit = 100);
        let point = r.add(1, Kind::Point, |s| s.limit = 200);
        r.world.borrow_mut().devices.get_mut(&plug).unwrap().handler.buffer = 150;
        let w = r.world.borrow();
        assert_eq!(w.info(plug), Some(Info { stored: 150, capacity: 150, can_receive: true }));
        assert_eq!(w.info(point), Some(Info { stored: 0, capacity: 200, can_receive: false }));
        assert_eq!(w.info(Key { dimension: 0, pos: [9, 9, 9] }), None);
    }
}
