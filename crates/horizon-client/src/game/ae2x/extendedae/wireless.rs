//! ExtendedAE's Wireless Connector and Wireless Hub, and the Wireless Setup
//! Kit that pairs them (`TileWirelessConnector`, `TileWirelessHub`,
//! `WirelessConnect`, `BlockWirelessConnector.check`, `BlockWirelessHub.check`,
//! `ItemWirelessConnectTool`), derived from ExtendedAE 1.21-2.2.42,
//! https://github.com/GlodBlock/ExtendedAE commit
//! 1f86f927552f02451236465c0c96a12a3de8f66c, LGPL-3.0; see `ae2/addons/extendedae/`.
//!
//! A link is a grid connection between two blocks' nodes (`GridHelper.createConnection`),
//! as AE2's quantum bridge makes one: in the graph, an edge of `Network.links`
//! (`ae2x_connect`); in the ordered event journal, a `MeLink` mutation each
//! time one is made or broken, so the ordered services see the same grid.
//! Each level's ends are looked at every tick (the mod: when a block's
//! frequency, node or chunk changes, which is all that can change a link).
use crate::game::{Game, KeyCode};
use ae2_network_core::{Network, NodeIdentity, Pos};
use horizon_client::{
    ae2x::extendedae::{self as rows, WirelessEnd},
    containers::BlockEntity,
    items::ItemStack,
    text::Text,
};
use horizon_world::nbt::Tag;
use std::collections::{BTreeMap, BTreeSet};

use rows::WIRELESS_POWER as POWER;
/// The first of the hub window's Disconnect entries.
pub(in crate::game) const HUB_DISCONNECT: usize = 10;

/// A link up: its two ends (node and port), where they are, how long it is,
/// and whether the two are neighbours whose faces already join them (then
/// `createConnection` fails and the cable's connection carries it).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Link {
    a: (NodeIdentity, usize),
    b: (NodeIdentity, usize),
    distance: f64,
    adjacent: bool,
}

/// The wireless links while the game runs; nothing of it is saved (the
/// blocks' frequencies are).
#[derive(Clone, Default)]
pub(in crate::game) struct Wireless {
    /// Each end's frequency and when it registered with it, by level, node and port.
    order: BTreeMap<(i32, NodeIdentity, usize), (i64, u64)>,
    next: u64,
    /// The links up, by level.
    links: BTreeMap<i32, Vec<Link>>,
    /// The grid connections the ordered event journal has been told of.
    journaled: BTreeSet<(NodeIdentity, NodeIdentity)>,
    /// `WirelessConnect.G`'s used frequencies (`FreqGenerator.markUsed`).
    used: BTreeSet<i64>,
}
impl Wireless {
    fn edges(&self) -> BTreeSet<(NodeIdentity, NodeIdentity)> {
        self.links.values().flatten().filter(|l| !l.adjacent).map(|l| ordered(l.a.0, l.b.0)).collect()
    }
    /// Whether link `port` of `node` is up (`WirelessConnect.isConnected`).
    fn connected(&self, node: NodeIdentity, port: usize) -> bool {
        self.links.values().flatten().any(|l| l.a == (node, port) || l.b == (node, port))
    }
    fn distances(&self, node: NodeIdentity) -> Vec<f64> {
        self.links.values().flatten().filter(|l| l.a.0 == node || l.b.0 == node).map(|l| l.distance).collect()
    }
}
fn ordered(a: NodeIdentity, b: NodeIdentity) -> (NodeIdentity, NodeIdentity) {
    if a <= b { (a, b) } else { (b, a) }
}
fn adjacent(p: (i32, i32, i32), q: (i32, i32, i32)) -> bool {
    (p.0 - q.0).abs() + (p.1 - q.1).abs() + (p.2 - q.2).abs() == 1
}
fn cards(machine: &horizon_client::ae2_world::Machine, card: &str) -> usize {
    machine.upgrades.iter().flatten().filter(|s| s.item.as_ref() == card).map(|s| s.count as usize).sum()
}

/// `WirelessFail`: why the kit could not pair, `chat.wireless_connect.<name>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum Fail {
    OutOfRange,
    SelfReference,
    CrossDimension,
    Missing,
    OutOfPort,
}
impl Fail {
    fn key(self) -> &'static str {
        match self {
            Self::OutOfRange => "chat.wireless_connect.out_of_range",
            Self::SelfReference => "chat.wireless_connect.self_reference",
            Self::CrossDimension => "chat.wireless_connect.cross_dimension",
            Self::Missing => "chat.wireless_connect.missing",
            Self::OutOfPort => "chat.wireless_connect.out_of_port",
        }
    }
}

/// The kit's locator: its frequency and the place it was bound at.
fn locator(stack: &ItemStack) -> Option<(i64, Option<(String, (i32, i32, i32))>)> {
    let tag = stack.data()?.other(rows::WIRELESS_LOCATOR)?;
    let frequency = match tag.get("a")? {
        Tag::Long(v) => *v,
        _ => return None,
    };
    let place = (|| {
        let b = tag.get("b")?;
        let Tag::String(dim) = b.get("dimension")? else { return None };
        let Tag::IntArray(pos) = b.get("pos")? else { return None };
        let [x, y, z] = pos.as_slice() else { return None };
        Some((dim.to_string().ok()?, (*x, *y, *z)))
    })();
    Some((frequency, place))
}
fn locator_tag(frequency: i64, dim: &str, pos: (i32, i32, i32)) -> Tag {
    Tag::compound([
        ("a", Tag::Long(frequency)),
        ("b", Tag::compound([("dimension", Tag::String(dim.into())), ("pos", Tag::IntArray(vec![pos.0, pos.1, pos.2]))])),
    ])
}

/// The node of a Wireless Connector or Hub: all sides, 32 channels
/// (`DENSE_CAPACITY`), no channel of its own, the idle draw its tick set.
pub(in crate::game) fn device(machine: &horizon_client::ae2_world::Machine, device: &mut ae2_network_core::Device) -> bool {
    if rows::wireless_ports(machine.id()).is_none() {
        return false;
    }
    device.faces = 63;
    device.channel_override = Some(false);
    device.capacity_override = Some(32);
    let power = match machine.config.get(&POWER.into()) {
        Some(Tag::Double(bits)) => f64::from_bits(*bits),
        // The constructor's 1.0, until the first tick.
        _ => 1.0,
    };
    device.idle_override = Some(power);
    true
}

impl Game<'_> {
    fn wireless_level(&self) -> i32 {
        crate::game::ae2_event_live::ordinal(self.dim)
    }
    /// `serverTick` of this level's Wireless Connectors and Hubs: the links
    /// `updateStatus` makes and breaks, their grid connections and journal
    /// events, and each block's `updatePowerUsage`.
    pub(in crate::game) fn tick_extendedae_wireless(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let level = self.wireless_level();
        let mut ends = Vec::new();
        let mut nodes = Vec::new();
        let mut seen = BTreeSet::new();
        // A hub made from a connector reads its `freq` as its first link and
        // saves all eight (`TileWirelessHub.loadTag`, `saveAdditional`).
        let legacy: Vec<_> = self.world.block_entities.iter().filter_map(|(&pos, entity)| match entity {
            BlockEntity::Ae2(m) if m.id() == rows::WIRELESS_HUB && m.config.contains_key(&"freq".into()) => Some(pos),
            _ => None,
        }).collect();
        for pos in legacy {
            if let Some(BlockEntity::Ae2(machine)) = self.world.block_entities.get_mut(&pos) {
                let first = rows::wireless_frequencies(rows::WIRELESS_HUB, &machine.config)[0];
                rows::set_wireless_frequency(rows::WIRELESS_HUB, &mut machine.config, 0, first);
                self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
            }
        }
        for (&pos, entity) in self.world.block_entities.iter() {
            let BlockEntity::Ae2(machine) = entity else { continue };
            if rows::wireless_ports(machine.id()).is_none() || self.res.facts().get(self.world.block(pos.0, pos.1, pos.2)).name.as_ref() != machine.id() {
                continue;
            }
            let Some(host) = self.world.block_entities.incarnation(&pos) else { continue };
            let node = NodeIdentity { host, part: None, outer: false };
            for (port, frequency) in rows::wireless_frequencies(machine.id(), &machine.config).into_iter().enumerate() {
                let state = &mut self.ae2x.extendedae.wireless;
                state.used.insert(frequency);
                // A new frequency registers again (CONNECTORS.register), after the others.
                let entry = state.order.entry((level, node, port)).or_insert((frequency, u64::MAX));
                if entry.0 != frequency || entry.1 == u64::MAX {
                    *entry = (frequency, state.next);
                    state.next += 1;
                }
                let order = entry.1;
                seen.insert((level, node, port));
                ends.push(WirelessEnd { level, pos, port, frequency, order });
                nodes.push(node);
            }
        }
        let state = &mut self.ae2x.extendedae.wireless;
        state.order.retain(|key, _| key.0 != level || seen.contains(key));
        let links: Vec<Link> = rows::wireless_links(&ends, rows::Config::get().wireless_range)
            .into_iter()
            .filter(|(a, b, _)| nodes[*a] != nodes[*b])
            .map(|(a, b, distance)| Link {
                a: (nodes[a], ends[a].port),
                b: (nodes[b], ends[b].port),
                distance,
                adjacent: adjacent(ends[a].pos, ends[b].pos),
            })
            .collect();
        if links.is_empty() {
            state.links.remove(&level);
        } else {
            state.links.insert(level, links);
        }
        let after = state.edges();
        let present: BTreeSet<NodeIdentity> = nodes.iter().copied().collect();
        // What the journal must hear: a connection broken between two blocks
        // still there (a block taken away tears its own down with its node),
        // one made between two nodes it has ready. The other levels' links
        // stay as their own ticks left them.
        let mut changes = Vec::new();
        for edge in state.journaled.clone() {
            if after.contains(&edge) {
                continue;
            }
            state.journaled.remove(&edge);
            if present.contains(&edge.0) && present.contains(&edge.1) {
                changes.push((edge, false));
            }
        }
        let unjournaled: Vec<_> = after.iter().filter(|e| !state.journaled.contains(e) && present.contains(&e.0) && present.contains(&e.1)).copied().collect();
        for edge in unjournaled {
            if self.ae2_event_node_is_ready(edge.0) && self.ae2_event_node_is_ready(edge.1) {
                self.ae2x.extendedae.wireless.journaled.insert(edge);
                changes.push((edge, true));
            }
        }
        let positions: BTreeMap<NodeIdentity, (i32, i32, i32)> = nodes.iter().copied().zip(ends.iter().map(|e| e.pos)).collect();
        for ((a, b), added) in changes {
            if let Some(pos) = positions.get(&a).or_else(|| positions.get(&b)) {
                self.world.block_entities.record_ae2_me_link(*pos, a, b, added);
            }
        }
        // updatePowerUsage: what each block's links cost, by its cards.
        let multiplier = rows::Config::get().wireless_power;
        #[cfg(feature = "ae2-megacells")]
        let greater_card = horizon_client::ae2x::megacells::GREATER_ENERGY_CARD;
        #[cfg(not(feature = "ae2-megacells"))]
        let greater_card = "";
        for (node, pos) in positions {
            let distances = self.ae2x.extendedae.wireless.distances(node);
            let Some(BlockEntity::Ae2(machine)) = self.world.block_entities.get_mut(&pos) else { continue };
            let greater = if greater_card.is_empty() { 0 } else { cards(machine, greater_card) };
            let power = rows::wireless_power(&distances, cards(machine, "ae2:energy_card"), greater, multiplier);
            if machine.config.get(&POWER.into()) != Some(&Tag::Double(power.to_bits())) {
                machine.config.insert(POWER.into(), Tag::Double(power.to_bits()));
            }
        }
    }

    /// The wireless links' grid connections (`GridHelper.createConnection`).
    pub(in crate::game) fn extendedae_wireless_connect(&self, network: &mut Network) {
        let edges = self.ae2x.extendedae.wireless.edges();
        if edges.is_empty() {
            return;
        }
        let at: BTreeMap<NodeIdentity, Pos> = network.node_identities.iter().map(|(p, id)| (*id, *p)).collect();
        for (a, b) in edges {
            if let (Some(a), Some(b)) = (at.get(&a), at.get(&b)) {
                let _ = network.connect(*a, *b);
            }
        }
    }

    /// `updateBlockStateFromBlockEntity`: `connected` while any of its links is up.
    pub(in crate::game) fn extendedae_wireless_visual(&self, pos: (i32, i32, i32), machine: &horizon_client::ae2_world::Machine, props: &mut horizon_world::block_state_properties::BlockProperties) -> bool {
        let Some(ports) = rows::wireless_ports(machine.id()) else { return false };
        let Some(host) = self.world.block_entities.incarnation(&pos) else { return false };
        let node = NodeIdentity { host, part: None, outer: false };
        let state = &self.ae2x.extendedae.wireless;
        props.insert("connected".into(), (0..ports).any(|port| state.connected(node, port)).to_string());
        true
    }

    /// `TileWirelessHub.allocatePort`: the first link not up; none for a
    /// connector, whose one link is always the one set.
    fn wireless_free_port(&self, pos: (i32, i32, i32)) -> Option<usize> {
        let Some(BlockEntity::Ae2(machine)) = self.world.block_entities.get(&pos) else { return None };
        let ports = rows::wireless_ports(machine.id())?;
        let node = NodeIdentity { host: self.world.block_entities.incarnation(&pos)?, part: None, outer: false };
        (0..ports).find(|port| !self.ae2x.extendedae.wireless.connected(node, *port))
    }
    fn set_wireless_frequency(&mut self, pos: (i32, i32, i32), port: usize, frequency: i64) {
        let Some(BlockEntity::Ae2(machine)) = self.world.block_entities.get_mut(&pos) else { return };
        let id = machine.id().to_owned();
        rows::set_wireless_frequency(&id, &mut machine.config, port, frequency);
        self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
    }
    /// `FreqGenerator.genFreq`: a random frequency, never 0 nor one used.
    fn new_wireless_frequency(&mut self) -> i64 {
        loop {
            let u = horizon_client::save::random_uuid();
            let f = (i64::from(u[0]) << 32) | i64::from(u[3] as u32);
            if f != 0 && self.ae2x.extendedae.wireless.used.insert(f) {
                return f;
            }
        }
    }
    /// The hub's Disconnect buttons, window entries 10 to 17
    /// (`ContainerWirelessHub.disconnect`, `TileWirelessHub.killPort`): that
    /// link forgotten. True when it was one.
    pub(in crate::game) fn extendedae_wireless_click(&mut self, pos: (i32, i32, i32), entry: usize, button: i32, input: horizon_client::menu::Input) -> bool {
        let Some(BlockEntity::Ae2(machine)) = self.world.block_entities.get(&pos) else { return false };
        if machine.id() != rows::WIRELESS_HUB || !(HUB_DISCONNECT..HUB_DISCONNECT + rows::HUB_PORTS).contains(&entry) {
            return false;
        }
        if input == horizon_client::menu::Input::Pickup && (0..=1).contains(&button) {
            self.set_wireless_frequency(pos, entry - HUB_DISCONNECT, 0);
        }
        true
    }

    /// The Wireless Setup Kit used on a Wireless Connector or Hub
    /// (`BlockWirelessConnector.check`, `BlockWirelessHub.check`, which the
    /// block's `useItemOn` asks before it opens its menu): the first use binds
    /// the kit to the block with a new frequency, the second gives it to both
    /// blocks. Sneaking, the block passes the use to the item. True when the
    /// kit acted, whether it paired or failed.
    pub(in crate::game) fn extendedae_wireless_tool_use(&mut self, pos: [i32; 3]) -> bool {
        if self.remote.is_some() || self.spectator() || self.keys.contains(&KeyCode::ShiftLeft) {
            return false;
        }
        if !self.held().is_some_and(|held| held.item.as_ref() == rows::WIRELESS_TOOL) {
            return false;
        }
        let key = (pos[0], pos[1], pos[2]);
        let Some(BlockEntity::Ae2(machine)) = self.world.block_entities.get(&key) else { return false };
        let Some(ports) = rows::wireless_ports(machine.id()) else { return false };
        let hub = ports > 1;
        // The hub needs a free link before anything else.
        let this_port = if hub {
            match self.wireless_free_port(key) {
                Some(port) => port,
                None => {
                    self.wireless_message(Fail::OutOfPort.key(), &[]);
                    return true;
                }
            }
        } else {
            0
        };
        let dim = self.dim.id().to_owned();
        let held = self.held().cloned().unwrap_or_else(|| ItemStack::new(rows::WIRELESS_TOOL, 1));
        if let Some((frequency, place)) = locator(&held).filter(|(f, _)| *f != 0) {
            match self.wireless_pair(key, this_port, frequency, place, &dim) {
                Ok(()) => {
                    if let Some(stack) = self.held_mut().as_mut() {
                        stack.data_mut().set_other(rows::WIRELESS_LOCATOR, None);
                    }
                    self.wireless_message("chat.wireless_connect", &[pos[0], pos[1], pos[2]]);
                }
                Err(fail) => self.wireless_message(fail.key(), &[]),
            }
            return true;
        }
        let frequency = self.new_wireless_frequency();
        if let Some(stack) = self.held_mut().as_mut() {
            stack.data_mut().set_other(rows::WIRELESS_LOCATOR, Some(locator_tag(frequency, &dim, key)));
        }
        self.wireless_message("chat.wireless_bind", &[pos[0], pos[1], pos[2]]);
        true
    }
    fn wireless_pair(&mut self, key: (i32, i32, i32), this_port: usize, frequency: i64, place: Option<(String, (i32, i32, i32))>, dim: &str) -> Result<(), Fail> {
        let (other_dim, other) = place.ok_or(Fail::Missing)?;
        if other == key && other_dim == dim {
            return Err(Fail::SelfReference);
        }
        if other_dim != dim {
            return Err(Fail::CrossDimension);
        }
        let d = (f64::from(other.0 - key.0).powi(2) + f64::from(other.1 - key.1).powi(2) + f64::from(other.2 - key.2).powi(2)).sqrt();
        if d > rows::Config::get().wireless_range {
            return Err(Fail::OutOfRange);
        }
        let Some(BlockEntity::Ae2(machine)) = self.world.block_entities.get(&other) else { return Err(Fail::Missing) };
        let other_port = match rows::wireless_ports(machine.id()) {
            Some(1) => 0,
            Some(_) => self.wireless_free_port(other).ok_or(Fail::OutOfPort)?,
            None => return Err(Fail::Missing),
        };
        self.set_wireless_frequency(other, other_port, frequency);
        self.set_wireless_frequency(key, this_port, frequency);
        Ok(())
    }
    fn wireless_message(&mut self, key: &str, args: &[i32]) {
        let args: Vec<Text> = args.iter().map(|n| Text::literal(n.to_string())).collect();
        self.system_message(Text::translate(key, &args));
    }
    /// `ItemWirelessConnectTool.use`, sneaking: the kit forgets its block.
    /// True when the held item is the kit.
    pub(in crate::game) fn extendedae_wireless_tool_clear(&mut self) -> bool {
        if !self.keys.contains(&KeyCode::ShiftLeft) || !self.held().is_some_and(|held| held.item.as_ref() == rows::WIRELESS_TOOL) {
            return false;
        }
        if let Some(stack) = self.held_mut().as_mut() {
            stack.data_mut().set_other(rows::WIRELESS_LOCATOR, None);
        }
        self.system_message(Text::translate("chat.wireless_connect.clear", &[]));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kit_keeps_its_locator_as_the_mod_writes_it() {
        let mut kit = ItemStack::new(rows::WIRELESS_TOOL, 1);
        assert_eq!(locator(&kit), None);
        kit.data_mut().set_other(rows::WIRELESS_LOCATOR, Some(locator_tag(-77, "minecraft:overworld", (1, 64, -3))));
        assert_eq!(locator(&kit), Some((-77, Some(("minecraft:overworld".to_owned(), (1, 64, -3))))));
        // A locator with no place: WirelessFail.MISSING when used.
        kit.data_mut().set_other(rows::WIRELESS_LOCATOR, Some(Tag::compound([("a", Tag::Long(5))])));
        assert_eq!(locator(&kit), Some((5, None)));
    }

    #[test]
    fn a_wireless_node_is_dense_and_takes_no_channel() {
        horizon_client::ae2x::install();
        let mut machine = horizon_client::ae2_world::Machine::for_block(rows::WIRELESS_HUB).unwrap();
        let mut node = ae2_network_core::Device::new(ae2_network_core::DeviceKind::Interface);
        node.faces = 0;
        assert!(device(&machine, &mut node));
        assert_eq!((node.faces, node.channel_override, node.capacity_override, node.idle_override), (63, Some(false), Some(32), Some(1.0)));
        machine.config.insert(POWER.into(), Tag::Double(12.5f64.to_bits()));
        assert!(device(&machine, &mut node));
        assert_eq!(node.idle_override, Some(12.5));
        let drive = horizon_client::ae2_world::Machine::for_block("ae2:drive").unwrap();
        assert!(!device(&drive, &mut node));
    }

    #[test]
    fn links_by_node_answer_for_both_ends() {
        let id = |host| NodeIdentity { host, part: None, outer: false };
        let mut state = Wireless::default();
        state.links.insert(0, vec![
            Link { a: (id(1), 0), b: (id(2), 3), distance: 900.0, adjacent: false },
            Link { a: (id(1), 1), b: (id(3), 0), distance: 1.0, adjacent: true },
        ]);
        assert!(state.connected(id(2), 3) && !state.connected(id(2), 0));
        assert_eq!(state.distances(id(1)), vec![900.0, 1.0]);
        // Neighbours are joined by their faces: no edge of their own.
        assert_eq!(state.edges(), BTreeSet::from([(id(1), id(2))]));
    }
}
