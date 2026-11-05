//! Mekanism's Logistical Transporters (`LogisticalTransporterBase`,
//! `TransporterStack`, `TransporterPathfinder`, `TransporterManager`,
//! `InventoryNetwork`, `TransitRequest`): item stacks travelling through
//! touching transporters as moving items, `speed` hundredths of a block a tick
//! (100 a transporter), each on a path to the cheapest inventory that takes
//! it, a transporter of a faster tier costing less (`getCost`), the stacks
//! already on their way to an inventory counting against its room; a stack
//! whose target can't take it any more finds another, goes back where it came
//! from (`HOME`), or wanders the network (`NONE`, idle) until one can. A pull
//! side takes up to the tier's pull amount of one kind of item out of the
//! inventory there every 11 ticks while that works, waiting longer each time
//! it doesn't (`delay`, `delayCount`). A transporter of a colour takes only
//! stacks of its colour and connects to no transporter of another colour; a
//! stack takes the colour of the transporter that pulled it.
//!
//! The world is the caller's (`Inventories`): what an inventory's side holds
//! and takes, putting items in and taking them out. Item stacks are the
//! caller's too (`Stack`). Positions are `network::Pos`, sides `Direction`
//! order (down, up, north, south, west, east).
use crate::network::{Connection, Pos, SIDES, offset, opposite};
use std::collections::{BTreeMap, BTreeSet, HashMap};

// ---- Colours ------------------------------------------------------------------------

/// `EnumColor`, saved by its ordinal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Color {
    Black,
    DarkBlue,
    DarkGreen,
    DarkAqua,
    DarkRed,
    Purple,
    Orange,
    Gray,
    DarkGray,
    Indigo,
    BrightGreen,
    Aqua,
    Red,
    Pink,
    Yellow,
    White,
    Brown,
    BrightPink,
}

impl Color {
    pub const ALL: [Self; 18] = [
        Self::Black,
        Self::DarkBlue,
        Self::DarkGreen,
        Self::DarkAqua,
        Self::DarkRed,
        Self::Purple,
        Self::Orange,
        Self::Gray,
        Self::DarkGray,
        Self::Indigo,
        Self::BrightGreen,
        Self::Aqua,
        Self::Red,
        Self::Pink,
        Self::Yellow,
        Self::White,
        Self::Brown,
        Self::BrightPink,
    ];

    pub fn id(self) -> i32 {
        self as i32
    }

    /// `EnumColor.BY_ID`, which wraps.
    pub fn by_id(id: i32) -> Self {
        Self::ALL[id.rem_euclid(Self::ALL.len() as i32) as usize]
    }

    /// Its English name (`EnumColor.englishName`).
    pub fn name(self) -> &'static str {
        [
            "Black", "Blue", "Green", "Cyan", "Dark Red", "Purple", "Orange", "Light Gray", "Gray", "Light Blue", "Lime", "Aqua", "Red",
            "Magenta", "Yellow", "White", "Brown", "Pink",
        ][self as usize]
    }

    /// `rgbCode`.
    pub fn rgb(self) -> [u8; 3] {
        [
            [64, 64, 64],
            [54, 107, 208],
            [89, 193, 95],
            [0, 243, 208],
            [201, 7, 31],
            [164, 96, 217],
            [255, 161, 96],
            [207, 207, 207],
            [122, 122, 122],
            [85, 158, 255],
            [117, 255, 137],
            [48, 255, 249],
            [255, 56, 60],
            [213, 94, 203],
            [255, 221, 79],
            [255, 255, 255],
            [161, 118, 73],
            [255, 188, 196],
        ][self as usize]
    }

    /// `TransporterUtils.increment`: none, then each colour, then none again.
    pub fn increment(color: Option<Self>) -> Option<Self> {
        match color {
            None => Some(Self::ALL[0]),
            Some(c) if c as usize == Self::ALL.len() - 1 => None,
            Some(c) => Some(Self::ALL[c as usize + 1]),
        }
    }

    /// `TransporterUtils.decrement`.
    pub fn decrement(color: Option<Self>) -> Option<Self> {
        match color {
            None => Some(Self::ALL[Self::ALL.len() - 1]),
            Some(Self::Black) => None,
            Some(c) => Some(Self::ALL[c as usize - 1]),
        }
    }
}

// ---- Stacks and paths -------------------------------------------------------------------

/// An item stack of the caller's.
pub trait Stack: Clone + std::fmt::Debug {
    fn count(&self) -> u32;
    fn with_count(&self, count: u32) -> Self;
    /// The same item with the same components (`HashedItem`).
    fn stacks_with(&self, other: &Self) -> bool;
    fn max_stack(&self) -> u32;
}

/// `TransporterStack.Path`, saved by its ordinal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathType {
    /// To an inventory that takes it.
    Dest,
    /// Back to where it came from.
    Home,
    /// Idle: along the network until a target turns up.
    None,
}

impl PathType {
    pub fn id(self) -> i32 {
        self as i32
    }

    pub fn by_id(id: i32) -> Self {
        [Self::Dest, Self::Home, Self::None][id.rem_euclid(3) as usize]
    }

    pub fn has_target(self) -> bool {
        self != Self::None
    }
}

/// `WorldUtils.sideDifference(a, b)`: the direction from `b` to `a` when
/// they touch.
pub fn side_difference(a: Pos, b: Pos) -> Option<usize> {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    SIDES.iter().position(|s| *s == d)
}

fn distance(a: Pos, b: Pos) -> f64 {
    let d = |i: usize| f64::from(a[i] - b[i]);
    (d(0) * d(0) + d(1) * d(1) + d(2) * d(2)).sqrt()
}

/// A `TransporterStack`: what travels, where it is in its transporter
/// (`progress`, 0 to 100) and where it goes (`path`: the target first, the
/// transporter it set out from last).
#[derive(Clone, Debug)]
pub struct TStack<S> {
    /// Count 0 is the empty stack.
    pub item: S,
    pub progress: i32,
    pub color: Option<Color>,
    /// `initiatedPath`: false for a stack read from a save, whose path is
    /// worked out again on its first tick.
    pub initiated: bool,
    pub idle_dir: Option<usize>,
    pub original: Option<Pos>,
    pub home: Option<Pos>,
    /// None is Java's null (`getPathType` is `NONE` then).
    pub path_type: Option<PathType>,
    pub path: Vec<Pos>,
    /// For the book of stacks on their way (`TransporterManager`).
    uid: u64,
}

impl<S: Stack> TStack<S> {
    /// A stack as a save or a client reads it.
    pub fn loaded(item: S, progress: i32, color: Option<Color>, original: Option<Pos>, home: Option<Pos>, path_type: Option<PathType>, idle_dir: Option<usize>) -> Self {
        Self { item, progress, color, initiated: false, idle_dir, original, home, path_type, path: Vec::new(), uid: 0 }
    }

    pub fn path_type(&self) -> PathType {
        self.path_type.unwrap_or(PathType::None)
    }

    pub fn has_path(&self) -> bool {
        self.path.len() >= 2
    }

    fn index_of(&self, pos: Pos) -> Option<usize> {
        self.path.iter().position(|p| *p == pos)
    }

    /// `isFinal`: the transporter at `pos` is its path's last.
    pub fn is_final(&self, pos: Pos) -> bool {
        self.path.get(if self.path_type().has_target() { 1 } else { 0 }) == Some(&pos)
    }

    /// `getNext`: the next position on its path from `pos`.
    pub fn next(&self, pos: Pos) -> Option<Pos> {
        let i = self.index_of(pos)?;
        i.checked_sub(1).map(|i| self.path[i])
    }

    /// `getPrev`: where it came into `pos` from (its original location at the path's start).
    pub fn prev(&self, pos: Pos) -> Option<Pos> {
        let i = self.index_of(pos).map_or(0, |i| i + 1);
        if i < self.path.len() { Some(self.path[i]) } else { self.original }
    }

    /// `getSide`: the way it moves through `pos` (from the side it came in by
    /// before half way, to the next after); down when that is no neighbour.
    pub fn side(&self, pos: Pos) -> usize {
        let side = if self.progress < 50 { self.prev(pos).and_then(|p| side_difference(pos, p)) } else { self.next(pos).and_then(|n| side_difference(n, pos)) };
        side.unwrap_or(0)
    }

    /// `getDest`.
    pub fn dest(&self) -> Option<Pos> {
        self.path.first().copied()
    }

    /// `getSideOfDest`: the target's side its last transporter touches.
    pub fn side_of_dest(&self) -> Option<usize> {
        if self.has_path() { side_difference(self.path[1], self.path[0]) } else { None }
    }

    /// `TransporterUtils.getStackPosition`: where it is drawn in the
    /// transporter at `pos`, from the block's corner.
    pub fn position(&self, pos: Pos, partial: f32) -> [f32; 3] {
        let s = SIDES[self.side(pos)];
        let p = (self.progress as f32 + partial) / 100.0 - 0.5;
        [0.5 + s[0] as f32 * p, 0.25 + s[1] as f32 * p, 0.5 + s[2] as f32 * p]
    }
}

// ---- Requests ------------------------------------------------------------------------

/// One kind of item a request offers, and the source slots it is in.
#[derive(Clone, Debug)]
pub struct ItemData<S> {
    pub kind: S,
    pub total: u32,
    /// `slotMap`: slot and how many, in the order they were met.
    pub slots: Vec<(usize, u32)>,
}

/// `TransitRequest`: what may travel, kind by kind.
#[derive(Clone, Debug)]
pub struct Request<S> {
    pub items: Vec<ItemData<S>>,
}

/// `TransitResponse`: how many of which of the request's kinds a target takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Response {
    pub index: usize,
    pub count: u32,
}

impl<S: Stack> Request<S> {
    /// `TransitRequest.simple`.
    pub fn simple(stack: &S) -> Self {
        Self { items: vec![ItemData { kind: stack.with_count(1), total: stack.count(), slots: Vec::new() }] }
    }

    /// `TransitRequest.anyItem`: from the slots a side reaches (`slots`: each
    /// slot and what may be taken from it), the last slot first, up to
    /// `amount` of each kind.
    pub fn any_item(slots: &[(usize, S)], amount: u32) -> Self {
        let mut items: Vec<ItemData<S>> = Vec::new();
        for (slot, stack) in slots.iter().rev() {
            let can = stack.count().min(amount).min(stack.max_stack());
            if can == 0 {
                continue;
            }
            let at = items.iter().position(|d| d.kind.stacks_with(stack));
            let had = at.map_or(0, |i| items[i].total);
            let to_use = can.min(amount - had);
            if to_use == 0 {
                continue;
            }
            match at {
                Some(i) => {
                    items[i].total += to_use;
                    items[i].slots.push((*slot, to_use));
                }
                None => items.push(ItemData { kind: stack.with_count(1), total: to_use, slots: vec![(*slot, to_use)] }),
            }
        }
        Self { items }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The stack a response sends.
    pub fn stack(&self, response: Response) -> S {
        self.items[response.index].kind.with_count(response.count)
    }

    /// `ItemData.use`: the source slots and counts that make up `response`.
    pub fn taken(&self, response: Response) -> Vec<(usize, u32)> {
        let mut left = response.count;
        let mut out = Vec::new();
        for &(slot, count) in &self.items[response.index].slots {
            if left == 0 {
                break;
            }
            let n = left.min(count);
            out.push((slot, n));
            left -= n;
        }
        out
    }
}

// ---- The world ------------------------------------------------------------------------

/// A slot of an inventory as a side reaches it for putting in one kind of item.
#[derive(Clone, Debug)]
pub struct Slot<S> {
    pub stack: Option<S>,
    /// `isItemValid`.
    pub accepts: bool,
    /// How many of the item it holds (`min(getSlotLimit, getMaxStackSize)`).
    pub limit: u32,
}

/// The inventories around the transporters (`Capabilities.ITEM`).
pub trait Inventories<S: Stack> {
    /// Whether the block at `pos` has an inventory on its side `face`.
    fn has_inventory(&mut self, pos: Pos, face: usize) -> bool;
    /// The slots side `face` of the inventory at `pos` reaches, for putting
    /// in `stack`; none where no inventory is.
    fn insert_slots(&mut self, pos: Pos, face: usize, stack: &S) -> Option<Vec<Slot<S>>>;
    /// Puts `stack` in through `face`, as `insertItem` over its slots: what
    /// didn't fit.
    fn insert(&mut self, pos: Pos, face: usize, stack: S) -> Option<S>;
    /// The slots `face` reaches and what may be taken out of each.
    fn take_slots(&mut self, pos: Pos, face: usize) -> Vec<(usize, S)>;
    /// Takes `count` out of slot `slot`.
    fn extract(&mut self, pos: Pos, face: usize, slot: usize, count: u32) -> Option<S>;
    /// `Block.popResource` at `pos`.
    fn drop_at(&mut self, pos: Pos, stack: S);
}

/// `InsertItem` simulated on a slot's real contents.
fn would_accept<S: Stack>(slot: &Slot<S>, stack: &S, count: u32) -> u32 {
    if !slot.accepts || count == 0 {
        return 0;
    }
    match &slot.stack {
        Some(held) if held.count() > 0 => {
            if !held.stacks_with(stack) {
                0
            } else {
                count.min(slot.limit.saturating_sub(held.count()))
            }
        }
        _ => count.min(slot.limit),
    }
}

/// `TransporterManager.InventoryInfo`: what each slot holds, as stacks on
/// their way fill it in the prediction.
struct InventoryInfo<S> {
    inventory: Vec<Option<S>>,
    sizes: Vec<u32>,
    actual: Vec<u32>,
}

/// Each kind's view of an inventory's slots, asked once a prediction.
struct Views<'w, S, W> {
    world: &'w mut W,
    pos: Pos,
    face: usize,
    seen: Vec<(S, Vec<Slot<S>>)>,
}

impl<S: Stack, W: Inventories<S>> Views<'_, S, W> {
    fn of(&mut self, stack: &S) -> Option<&[Slot<S>]> {
        let at = match self.seen.iter().position(|(k, _)| k.stacks_with(stack)) {
            Some(i) => i,
            None => {
                let slots = self.world.insert_slots(self.pos, self.face, stack)?;
                self.seen.push((stack.with_count(1), slots));
                self.seen.len() - 1
            }
        };
        Some(&self.seen[at].1)
    }
}

/// `TransporterManager.simulateInsert`: what of `count` of `stack` is left
/// over after the prediction's slots took what they would.
fn simulate_insert<S: Stack, W: Inventories<S>>(views: &mut Views<'_, S, W>, info: &mut InventoryInfo<S>, stack: &S, mut count: u32, in_flight: bool) -> u32 {
    let Some(slots) = views.of(stack).map(<[Slot<S>]>::to_vec) else {
        return count;
    };
    let max_stack = stack.max_stack();
    for (slot, view) in slots.iter().enumerate() {
        if count == 0 {
            break;
        }
        let mut max = view.limit;
        if max == 0 || !view.accepts || slot >= info.sizes.len() {
            continue;
        }
        let dest = info.sizes[slot];
        let merged = count + dest;
        let to_accept = count;
        let mut simulate = false;
        if dest > 0 {
            let stackable = info.inventory[slot].as_ref().is_some_and(|s| s.stacks_with(stack));
            if dest >= max || !stackable {
                continue;
            }
            if (max > max_stack && merged > max_stack) || !in_flight {
                simulate = true;
            }
        } else {
            simulate = true;
        }
        if simulate {
            let accepted = would_accept(view, stack, to_accept);
            if accepted == 0 {
                continue;
            }
            if accepted < to_accept {
                max = info.actual[slot] + accepted;
            }
            if dest == 0 {
                info.inventory[slot] = Some(stack.with_count(1));
            }
        }
        if merged <= max {
            info.sizes[slot] = merged;
            return 0;
        }
        info.sizes[slot] = max;
        count = merged - max;
    }
    count
}

/// `TransporterUtils.canInsert` (not forced): some slot of side `face` of
/// the inventory at `pos` takes some of `stack`.
fn can_insert<S: Stack, W: Inventories<S>>(world: &mut W, pos: Pos, face: usize, stack: &S) -> bool {
    world.insert_slots(pos, face, stack).is_some_and(|slots| slots.iter().any(|s| would_accept(s, stack, stack.count()) > 0))
}

// ---- Transporters ------------------------------------------------------------------------

/// One logistical transporter.
#[derive(Clone, Debug)]
pub struct Transporter<S> {
    pub speed: i32,
    pub pull: u32,
    /// `getCost`: the ultimate tier's speed over this one's.
    pub cost: f64,
    pub connections: [Connection; 6],
    pub color: Option<Color>,
    /// A bit per side: an inventory there.
    acceptors: u8,
    pub transit: BTreeMap<i32, TStack<S>>,
    pub next_id: i32,
    pub delay: i32,
    pub delay_count: i32,
    /// Changed since clients and the save last heard (`needsSync`, deletes).
    pub dirty: bool,
    /// In a network: from the end of its first tick.
    joined: bool,
}

impl<S: Stack> Transporter<S> {
    pub fn new(speed: u32, pull: u32, ultimate_speed: u32, connections: [Connection; 6], color: Option<Color>) -> Self {
        Self {
            speed: speed.min(i32::MAX as u32) as i32,
            pull,
            cost: f64::from(ultimate_speed) / f64::from(speed.max(1)),
            connections,
            color,
            acceptors: 0,
            transit: BTreeMap::new(),
            next_id: 0,
            delay: 0,
            delay_count: 0,
            dirty: false,
            joined: false,
        }
    }

    /// `canConnect`.
    pub fn can_connect(&self, side: usize) -> bool {
        self.connections[side] != Connection::None
    }

    /// A stack read from its save joins its transit (`readFromNBT`).
    pub fn add_loaded(&mut self, stack: TStack<S>) {
        let id = self.next_id;
        self.next_id += 1;
        self.transit.insert(id, stack);
    }
}

/// A path found: the target first, the start last, its cost, and what it takes.
#[derive(Clone, Debug)]
pub struct Destination {
    pub path: Vec<Pos>,
    pub score: f64,
    pub response: Option<Response>,
    pub path_type: PathType,
}

/// What a tick did that the game shows: transporters to send to clients and
/// save.
#[derive(Default, Debug)]
pub struct Ticked {
    pub changed: BTreeSet<Pos>,
}

/// The transporters of a level, in the order they tick (the order they came).
#[derive(Debug)]
pub struct Transporters<S> {
    map: HashMap<Pos, Transporter<S>>,
    order: Vec<Pos>,
    /// `TransporterManager.flowingStacks`: by target, the stacks on their
    /// way there (their item and the target's side they come to).
    flowing: BTreeMap<Pos, BTreeMap<u64, (S, Option<usize>)>>,
    next_uid: u64,
    /// The network of each transporter, worked out when asked after a change.
    networks: Option<HashMap<Pos, u32>>,
}

impl<S> Default for Transporters<S> {
    fn default() -> Self {
        Self { map: HashMap::new(), order: Vec::new(), flowing: BTreeMap::new(), next_uid: 1, networks: None }
    }
}

impl<S: Stack> Transporters<S> {
    pub fn get(&self, pos: Pos) -> Option<&Transporter<S>> {
        self.map.get(&pos)
    }

    pub fn get_mut(&mut self, pos: Pos) -> Option<&mut Transporter<S>> {
        self.networks = None;
        self.map.get_mut(&pos)
    }

    pub fn positions(&self) -> &[Pos] {
        &self.order
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// A transporter placed or loaded: it ticks once it joined, at the next
    /// [`commit`](Self::commit).
    pub fn add(&mut self, pos: Pos, mut transporter: Transporter<S>) {
        if self.map.remove(&pos).is_some() {
            self.order.retain(|p| *p != pos);
        }
        transporter.joined = false;
        for stack in transporter.transit.values_mut() {
            stack.uid = self.next_uid;
            self.next_uid += 1;
        }
        self.map.insert(pos, transporter);
        self.order.push(pos);
        self.networks = None;
    }

    /// A transporter broken or unloaded: its stacks leave the book of stacks
    /// on their way (`remove`); a broken one's are given back (`blockRemoved`:
    /// `TransporterUtils.drop`).
    pub fn remove(&mut self, pos: Pos, world: Option<&mut dyn FnMut(Pos, S)>) -> Option<Transporter<S>> {
        let transporter = self.map.remove(&pos)?;
        self.order.retain(|p| *p != pos);
        self.networks = None;
        for stack in transporter.transit.values() {
            self.unflow(stack);
        }
        if let Some(drop) = world {
            for stack in transporter.transit.values() {
                if stack.item.count() > 0 {
                    drop(self.drop_pos(pos, stack), stack.item.clone());
                }
            }
        }
        Some(transporter)
    }

    /// `registerOrphanTransmitter`'s commit: those waiting join.
    pub fn commit(&mut self) {
        for t in self.map.values_mut() {
            t.joined = true;
        }
    }

    /// Says whether an inventory stands on `side`; true if that changed.
    pub fn set_acceptor(&mut self, pos: Pos, side: usize, acceptor: bool) -> bool {
        let Some(t) = self.map.get_mut(&pos) else { return false };
        let bit = 1u8 << side;
        if (t.acceptors & bit != 0) == acceptor {
            return false;
        }
        t.acceptors ^= bit;
        true
    }

    /// A side's configuration or the colour changed: networks are worked out again.
    pub fn changed(&mut self) {
        self.networks = None;
    }

    // ---- Connections ----------------------------------------------------------------

    /// `isValidTransmitter` between the transporter at `pos` and the one on
    /// `side`: both connect there, and their colours don't differ.
    pub fn linked(&self, pos: Pos, side: usize) -> bool {
        match (self.map.get(&pos), self.map.get(&offset(pos, side))) {
            (Some(a), Some(b)) => {
                a.can_connect(side) && b.can_connect(opposite(side)) && (a.color.is_none() || b.color.is_none() || a.color == b.color)
            }
            _ => false,
        }
    }

    /// `getConnectionType`: normal to a linked transporter, the side's
    /// configuration to an inventory, else none.
    pub fn shown(&self, pos: Pos, side: usize) -> Connection {
        let Some(t) = self.map.get(&pos) else { return Connection::None };
        if self.linked(pos, side) {
            Connection::Normal
        } else if t.acceptors & (1 << side) != 0 {
            t.connections[side]
        } else {
            Connection::None
        }
    }

    fn can_connect(&self, pos: Pos, side: usize) -> bool {
        self.map.get(&pos).is_some_and(|t| t.can_connect(side))
    }

    /// `canEmitTo`.
    fn can_emit_to(&self, pos: Pos, side: usize) -> bool {
        self.can_connect(pos, side) && self.shown(pos, side).can_send_to()
    }

    /// `canReceiveFrom`.
    fn can_receive_from(&self, pos: Pos, side: usize) -> bool {
        self.can_connect(pos, side) && self.shown(pos, side).can_accept()
    }

    /// `TransporterStack.canInsertToTransporter`: a stack of `color` moving
    /// `dir` from `from` enters the transporter at `to`.
    fn can_enter(&self, color: Option<Color>, to: Pos, dir: usize, from: Option<Pos>) -> bool {
        let Some(t) = self.map.get(&to) else { return false };
        (t.color.is_none() || t.color == color)
            && t.can_connect(opposite(dir))
            && from.and_then(|f| self.map.get(&f)).is_none_or(|f| f.can_connect(dir))
    }

    /// The network of the transporter at `pos`: a number shared by those its links reach.
    pub fn network_of(&mut self, pos: Pos) -> Option<u32> {
        if self.networks.is_none() {
            let mut ids = HashMap::new();
            let mut next = 0;
            for &start in &self.order {
                if ids.contains_key(&start) {
                    continue;
                }
                let mut stack = vec![start];
                ids.insert(start, next);
                while let Some(p) = stack.pop() {
                    for side in 0..6 {
                        let q = offset(p, side);
                        if !ids.contains_key(&q) && self.linked(p, side) {
                            ids.insert(q, next);
                            stack.push(q);
                        }
                    }
                }
                next += 1;
            }
            self.networks = Some(ids);
        }
        self.networks.as_ref()?.get(&pos).copied()
    }

    /// The inventories of a network (`acceptorCache`): by position, their
    /// sides that touch its transporters.
    fn acceptors_of(&mut self, network: u32) -> BTreeMap<Pos, BTreeSet<usize>> {
        let mut out: BTreeMap<Pos, BTreeSet<usize>> = BTreeMap::new();
        let members: Vec<Pos> = self.order.iter().copied().filter(|p| self.networks.as_ref().and_then(|n| n.get(p)) == Some(&network)).collect();
        for pos in members {
            let t = &self.map[&pos];
            for side in 0..6 {
                if t.acceptors & (1 << side) != 0 {
                    out.entry(offset(pos, side)).or_default().insert(opposite(side));
                }
            }
        }
        out
    }

    fn in_network(&self, network: u32, pos: Pos) -> bool {
        self.networks.as_ref().and_then(|n| n.get(&pos)) == Some(&network)
    }

    // ---- The book of stacks on their way ------------------------------------------------

    fn flow(&mut self, stack: &TStack<S>) {
        if stack.path_type().has_target()
            && let Some(dest) = stack.dest()
        {
            self.flowing.entry(dest).or_default().insert(stack.uid, (stack.item.clone(), stack.side_of_dest()));
        }
    }

    fn unflow(&mut self, stack: &TStack<S>) {
        if stack.has_path()
            && stack.path_type().has_target()
            && let Some(dest) = stack.dest()
            && let Some(set) = self.flowing.get_mut(&dest)
        {
            set.remove(&stack.uid);
            if set.is_empty() {
                self.flowing.remove(&dest);
            }
        }
    }

    /// The item of a stack on its way changed (a part went in).
    fn reflow(&mut self, stack: &TStack<S>) {
        if let Some(dest) = stack.dest()
            && let Some(entry) = self.flowing.get_mut(&dest).and_then(|set| set.get_mut(&stack.uid))
        {
            entry.0 = stack.item.clone();
        }
    }

    /// `setPath`.
    fn set_path(&mut self, stack: &mut TStack<S>, path: Vec<Pos>, path_type: PathType, update_flowing: bool) {
        if update_flowing && stack.path_type.is_none_or(PathType::has_target) {
            self.unflow(stack);
        }
        stack.path = path;
        stack.path_type = Some(path_type);
        if update_flowing && path_type.has_target() {
            self.flow(stack);
        }
    }

    /// The stacks on their way to `dest`, as `getPredictedInsert` counts them.
    pub fn flowing_to(&self, dest: Pos) -> Vec<(S, Option<usize>)> {
        self.flowing.get(&dest).map_or(Vec::new(), |set| set.values().cloned().collect())
    }

    // ---- Paths ---------------------------------------------------------------------------

    /// `TransporterManager.getPredictedInsert` on side `face` of the
    /// inventory at `dest`: the first of the request's kinds it would take some
    /// of, with the stacks on their way there counted in.
    fn predicted_insert<W: Inventories<S>>(&self, world: &mut W, dest: Pos, face: usize, request: &Request<S>) -> Option<Response> {
        let first = request.items.first()?;
        let mut views = Views { world, pos: dest, face, seen: Vec::new() };
        let slots = views.of(&first.kind)?.to_vec();
        let mut info = InventoryInfo {
            inventory: slots.iter().map(|s| s.stack.clone()).collect(),
            sizes: slots.iter().map(|s| s.stack.as_ref().map_or(0, Stack::count)).collect(),
            actual: slots.iter().map(|s| s.stack.as_ref().map_or(0, Stack::count)).collect(),
        };
        for (item, side_of_dest) in self.flowing.get(&dest).map(|s| s.values().cloned().collect::<Vec<_>>()).unwrap_or_default() {
            let left = simulate_insert(&mut views, &mut info, &item, item.count(), true);
            if left > 0 && (left != item.count() || side_of_dest == Some(face)) {
                return None;
            }
        }
        for (index, data) in request.items.iter().enumerate() {
            let left = simulate_insert(&mut views, &mut info, &data.kind, data.total, false);
            if left != data.total {
                return Some(Response { index, count: data.total - left });
            }
        }
        None
    }

    /// `TransporterPathfinder.getPaths`: a path to each inventory of the
    /// network that takes some of the request (by `calculateAcceptors`), the
    /// cheapest first.
    fn paths<W: Inventories<S>>(&mut self, world: &mut W, start: Pos, stack: &TStack<S>, request: &Request<S>, min: u32) -> Vec<Destination> {
        let Some(network) = self.network_of(start) else { return Vec::new() };
        let mut found = Vec::new();
        for (dest, faces) in self.acceptors_of(network) {
            if Some(dest) == stack.home {
                continue;
            }
            // `AcceptorData`: the same response through several sides is one target.
            let mut targets: Vec<(Response, BTreeSet<usize>)> = Vec::new();
            for face in faces {
                let Some(response) = self.predicted_insert(world, dest, face, request) else { continue };
                match targets.iter_mut().find(|(r, _)| *r == response) {
                    Some((_, sides)) => {
                        sides.insert(opposite(face));
                    }
                    None => targets.push((response, BTreeSet::from([opposite(face)]))),
                }
            }
            for (response, _) in targets {
                if response.count < min {
                    continue;
                }
                let data = request.stack(response);
                let finder = Pathfinder { nets: self, network, start, target: dest, color: stack.color, home: stack.home, force: false };
                if let Some((path, score)) = finder.find(world, &data) {
                    found.push(Destination { path, score, response: Some(response), path_type: PathType::Dest });
                }
            }
        }
        // `Collections.sort`, stable: by cost, then by length.
        found.sort_by(|a, b| a.score.total_cmp(&b.score).then(a.path.len().cmp(&b.path.len())));
        found
    }

    /// `recalculatePath`: a new path to the cheapest target, set on the
    /// stack; what it takes there.
    fn recalculate_path<W: Inventories<S>>(&mut self, world: &mut W, stack: &mut TStack<S>, request: &Request<S>, start: Pos, min: u32, update_flowing: bool) -> Option<Response> {
        let dest = self.paths(world, start, stack, request, min).into_iter().next()?;
        stack.idle_dir = None;
        self.set_path(stack, dest.path, PathType::Dest, update_flowing);
        stack.initiated = true;
        dest.response
    }

    /// `getIdlePath`: home if a path there takes it, else along the network.
    fn idle_path<W: Inventories<S>>(&mut self, world: &mut W, start: Pos, stack: &mut TStack<S>) -> Option<(Vec<Pos>, PathType)> {
        let network = self.network_of(start)?;
        if let Some(home) = stack.home {
            let finder = Pathfinder { nets: self, network, start, target: home, color: stack.color, home: stack.home, force: true };
            if let Some((path, _)) = finder.find(world, &stack.item) {
                return Some((path, PathType::Home));
            }
            stack.home = None;
        }
        self.idle_find(world, network, start, stack).map(|d| (d.path, d.path_type))
    }

    /// `IdlePath.find`.
    fn idle_find<W: Inventories<S>>(&mut self, world: &mut W, network: u32, start: Pos, stack: &mut TStack<S>) -> Option<Destination> {
        let mut ret = vec![start];
        let Some(dir) = stack.idle_dir else {
            return self.idle_destination(network, ret, start, stack);
        };
        if self.in_network(network, offset(start, dir)) && self.can_enter(stack.color, offset(start, dir), dir, Some(start)) {
            self.loop_side(network, &mut ret, start, dir, stack.color);
            return Some(idle_route(ret));
        }
        let request = Request::simple(&stack.item);
        if self.map.contains_key(&start)
            && let Some(mut dest) = self.paths(world, start, stack, &request, 0).into_iter().next()
            && dest.response.is_some()
        {
            stack.idle_dir = None;
            dest.path_type = PathType::Dest;
            return Some(dest);
        }
        self.idle_destination(network, ret, start, stack)
    }

    /// `IdlePath.getDestination`.
    fn idle_destination(&mut self, network: u32, mut ret: Vec<Pos>, start: Pos, stack: &mut TStack<S>) -> Option<Destination> {
        let enter = |nets: &Self, side: usize| nets.in_network(network, offset(start, side)) && nets.can_enter(stack.color, offset(start, side), side, Some(start));
        let found = match stack.idle_dir {
            None => (0..6).find(|&s| enter(self, s)),
            Some(dir) => {
                let back = opposite(dir);
                (0..6).filter(|&s| s != back).find(|&s| enter(self, s)).or_else(|| enter(self, back).then_some(back))
            }
        };
        if let Some(side) = found {
            stack.idle_dir = Some(side);
            self.loop_side(network, &mut ret, start, side, stack.color);
            return Some(idle_route(ret));
        }
        if let Some(t) = self.map.get(&start) {
            let closest = stack.idle_dir.unwrap_or_else(|| stack.side(start));
            if let Some(side) = (0..6).filter(|&s| s != closest).find(|&s| t.connections[s] != Connection::None && self.shown(start, s) != Connection::None) {
                stack.idle_dir = Some(side);
                ret.push(offset(start, side));
                return Some(idle_route(ret));
            }
        }
        None
    }

    /// `IdlePath.loopSide`: the transporters straight on from `start`.
    fn loop_side(&self, network: u32, list: &mut Vec<Pos>, start: Pos, side: usize, color: Option<Color>) {
        let mut last = start;
        let mut pos = offset(start, side);
        while self.in_network(network, pos) && self.can_enter(color, pos, side, Some(last)) {
            list.push(pos);
            last = pos;
            pos = offset(pos, side);
        }
    }

    /// `calculateIdle`.
    fn calculate_idle<W: Inventories<S>>(&mut self, world: &mut W, pos: Pos, stack: &mut TStack<S>) -> bool {
        let Some((path, path_type)) = self.idle_path(world, pos, stack) else { return false };
        if path_type == PathType::Home {
            stack.idle_dir = None;
        }
        self.set_path(stack, path, path_type, true);
        stack.original = Some(pos);
        stack.initiated = true;
        true
    }

    /// Where `TransporterUtils.drop` drops a stack of the transporter at `pos`.
    fn drop_pos(&self, pos: Pos, stack: &TStack<S>) -> Pos {
        if !stack.has_path() {
            return pos;
        }
        let p = stack.position(pos, 0.0);
        [pos[0] + p[0].floor() as i32, pos[1] + p[1].floor() as i32, pos[2] + p[2].floor() as i32]
    }

    /// `recalculate`: a new target, else idle, else dropped (false).
    fn recalculate<W: Inventories<S>>(&mut self, world: &mut W, pos: Pos, stack: &mut TStack<S>, from: Option<Pos>) -> bool {
        let no_path = !stack.path_type().has_target() || self.recalculate_path(world, stack, &Request::simple(&stack.item), pos, 0, true).is_none();
        if no_path && !self.calculate_idle(world, pos, stack) {
            self.unflow(stack);
            world.drop_at(self.drop_pos(pos, stack), stack.item.clone());
            return false;
        }
        if let Some(t) = self.map.get_mut(&pos) {
            t.dirty = true;
        }
        if from.is_some() {
            stack.original = from;
        }
        true
    }

    // ---- Putting stacks in ------------------------------------------------------------------

    /// `insert`: a stack of `request` from the block at `outputter` into the
    /// transporter at `pos`, on a path to the cheapest target that takes at
    /// least `min` of it. What it took (none when no target takes any); with
    /// `execute` the stack sets out.
    pub fn insert<W: Inventories<S>>(&mut self, world: &mut W, pos: Pos, outputter: Pos, request: &Request<S>, color: Option<Color>, min: u32, execute: bool) -> Option<Response> {
        let from = side_difference(pos, outputter)?;
        if !self.can_receive_from(pos, opposite(from)) || !self.map.get(&pos)?.joined {
            return None;
        }
        let mut stack = TStack::loaded(request.items.first()?.kind.with_count(0), 0, color, Some(outputter), Some(outputter), None, None);
        stack.uid = self.next_uid;
        self.next_uid += 1;
        if !self.can_enter(color, pos, from, None) {
            return None;
        }
        let response = self.recalculate_path(world, &mut stack, request, pos, min, execute)?;
        stack.item = request.stack(response);
        if execute {
            self.reflow(&stack);
            let t = self.map.get_mut(&pos)?;
            let id = t.next_id;
            t.next_id += 1;
            t.transit.insert(id, stack);
            t.dirty = true;
        }
        Some(response)
    }

    // ---- The tick ---------------------------------------------------------------------------

    /// `onUpdateServer` of every transporter that joined, in their order.
    pub fn tick<W: Inventories<S>>(&mut self, world: &mut W) -> Ticked {
        let mut ticked = Ticked::default();
        for pos in self.order.clone() {
            self.tick_one(world, pos, &mut ticked);
        }
        for (pos, t) in &mut self.map {
            if std::mem::take(&mut t.dirty) {
                ticked.changed.insert(*pos);
            }
        }
        ticked
    }

    fn tick_one<W: Inventories<S>>(&mut self, world: &mut W, pos: Pos, ticked: &mut Ticked) {
        let Some(t) = self.map.get(&pos) else { return };
        if !t.joined {
            return;
        }
        self.pull(world, pos);
        let ids: Vec<i32> = self.map[&pos].transit.keys().copied().collect();
        let mut deletes = Vec::new();
        for id in ids {
            let Some(mut stack) = self.map.get_mut(&pos).and_then(|t| t.transit.remove(&id)) else { continue };
            match self.move_stack(world, pos, &mut stack) {
                Moved::Stays => {
                    if let Some(t) = self.map.get_mut(&pos) {
                        t.transit.insert(id, stack);
                    }
                }
                Moved::Gone => deletes.push(id),
                Moved::Into(next) => {
                    deletes.push(id);
                    if let Some(t) = self.map.get_mut(&next) {
                        let nid = t.next_id;
                        t.next_id += 1;
                        t.transit.insert(nid, stack);
                        t.dirty = true;
                    }
                }
            }
        }
        if !deletes.is_empty()
            && let Some(t) = self.map.get_mut(&pos)
        {
            t.dirty = true;
            ticked.changed.insert(pos);
        }
    }

    /// The pull sides, every `delay` ticks.
    fn pull<W: Inventories<S>>(&mut self, world: &mut W, pos: Pos) {
        let t = &self.map[&pos];
        if t.delay > 0 {
            self.map.get_mut(&pos).expect("there").delay -= 1;
            return;
        }
        let (pull, color) = (t.pull, t.color);
        self.map.get_mut(&pos).expect("there").delay = 3;
        for side in 0..6 {
            if self.shown(pos, side) != Connection::Pull {
                continue;
            }
            let source = offset(pos, side);
            let face = opposite(side);
            if !world.has_inventory(source, face) {
                continue;
            }
            let request = Request::any_item(&world.take_slots(source, face), pull);
            if request.is_empty() {
                continue;
            }
            match self.insert(world, pos, source, &request, color, 0, true) {
                None => {
                    let t = self.map.get_mut(&pos).expect("there");
                    t.delay_count += 1;
                    t.delay = 40.min(f64::from(t.delay_count).exp() as i32);
                }
                Some(response) => {
                    for (slot, count) in request.taken(response) {
                        world.extract(source, face, slot, count);
                    }
                    self.map.get_mut(&pos).expect("there").delay = 10;
                }
            }
        }
    }

    /// One stack's tick in the transporter at `pos`.
    fn move_stack<W: Inventories<S>>(&mut self, world: &mut W, pos: Pos, stack: &mut TStack<S>) -> Moved {
        if !stack.initiated && (stack.item.count() == 0 || !self.recalculate(world, pos, stack, None)) {
            return Moved::Gone;
        }
        let speed = self.map[&pos].speed;
        let before = stack.progress;
        stack.progress = stack.progress.saturating_add(speed);
        if stack.progress >= 100 {
            let mut prev_set = None;
            if stack.has_path() {
                if stack.path[0] == pos {
                    return Moved::Gone;
                }
                let next = stack.next(pos);
                if let Some(next) = next {
                    if !stack.is_final(pos) {
                        let side = stack.side(pos);
                        if self.can_enter(stack.color, next, side, Some(pos)) {
                            stack.progress %= 100;
                            return Moved::Into(next);
                        }
                        prev_set = Some(next);
                    } else if stack.path_type().has_target() {
                        let face = opposite(stack.side(pos));
                        let home = stack.path_type() == PathType::Home;
                        let reached = self.map[&pos].acceptors & (1 << opposite(face)) != 0 || home;
                        if reached && stack.item.count() > 0 {
                            let offered = stack.item.count();
                            let rest = world.insert(next, face, stack.item.clone());
                            let left = rest.as_ref().map_or(0, Stack::count);
                            if left < offered {
                                match rest {
                                    None => {
                                        self.unflow(stack);
                                        return Moved::Gone;
                                    }
                                    Some(rest) => {
                                        stack.item = rest;
                                        self.reflow(stack);
                                    }
                                }
                            }
                        }
                        prev_set = Some(next);
                    }
                }
            }
            if !self.recalculate(world, pos, stack, prev_set) {
                return Moved::Gone;
            }
            stack.progress = if prev_set.is_none() { 50 } else { 0 };
        } else if before < 50 && stack.progress >= 50 {
            let retry = if stack.is_final(pos) {
                if stack.path_type().has_target() {
                    let side = stack.side(pos);
                    !self.shown(pos, side).can_send_to()
                        || !stack.dest().is_some_and(|d| {
                            if stack.path_type() == PathType::Home {
                                world.has_inventory(d, opposite(side))
                            } else {
                                can_insert(world, d, opposite(side), &stack.item)
                            }
                        })
                } else {
                    true
                }
            } else {
                match stack.next(pos) {
                    None => true,
                    Some(next) => {
                        let side = side_difference(next, pos).unwrap_or(0);
                        if !self.map.contains_key(&next) && !stack.path_type().has_target() && stack.path.len() == 2 {
                            !self.shown(pos, side).can_send_to()
                        } else {
                            !self.can_enter(stack.color, next, side, Some(pos))
                        }
                    }
                }
            };
            if retry && !self.recalculate(world, pos, stack, None) {
                return Moved::Gone;
            }
        }
        Moved::Stays
    }
}

/// What a stack's tick did.
enum Moved {
    Stays,
    Gone,
    /// It went on into the transporter there.
    Into(Pos),
}

/// `IdlePath.createDestination`: the run reversed, its far end first.
fn idle_route(mut run: Vec<Pos>) -> Destination {
    run.reverse();
    Destination { path: run, score: 0.0, response: None, path_type: PathType::None }
}

/// `TransporterPathfinder.Pathfinder`: A* over a network's transporters from
/// `start` to the inventory at `target`.
struct Pathfinder<'a, S> {
    nets: &'a Transporters<S>,
    network: u32,
    start: Pos,
    target: Pos,
    color: Option<Color>,
    home: Option<Pos>,
    /// The home check (`canInsert` forced): any inventory there.
    force: bool,
}

impl<S: Stack> Pathfinder<'_, S> {
    fn transporter(&self, pos: Pos) -> bool {
        self.nets.in_network(self.network, pos)
    }

    fn enters(&self, to: Pos, dir: usize, from: Pos) -> bool {
        self.transporter(to) && self.nets.can_enter(self.color, to, dir, Some(from))
    }

    /// `isValidDestination`.
    fn valid_target<W: Inventories<S>>(&self, world: &mut W, from: Pos, dir: usize, neighbour: Pos, data: &S) -> bool {
        if !self.transporter(from) || neighbour != self.target {
            return false;
        }
        let face = opposite(dir);
        let takes = if self.force { world.has_inventory(neighbour, face) } else { can_insert(world, neighbour, face, data) };
        takes && (self.nets.can_emit_to(from, dir) || (Some(self.target) == self.home && self.nets.can_connect(from, dir)))
    }

    /// `find`: the path (the target first) and its score.
    fn find<W: Inventories<S>>(&self, world: &mut W, data: &S) -> Option<(Vec<Pos>, f64)> {
        let mut open: BTreeSet<Pos> = BTreeSet::from([self.start]);
        let mut closed: BTreeSet<Pos> = BTreeSet::new();
        let mut nav: HashMap<Pos, Pos> = HashMap::new();
        let mut g: HashMap<Pos, f64> = HashMap::from([(self.start, 0.0)]);
        let total = distance(self.start, self.target);
        let mut f: HashMap<Pos, f64> = HashMap::from([(self.start, total)]);
        let reconstruct = |nav: &HashMap<Pos, Pos>, mut node: Pos| {
            let mut path = vec![self.target, node];
            while let Some(&back) = nav.get(&node) {
                path.push(back);
                node = back;
            }
            path
        };
        let mut valid_direction = false;
        for dir in 0..6 {
            let neighbour = offset(self.start, dir);
            if self.enters(neighbour, dir, self.start) {
                valid_direction = true;
                break;
            }
            if self.valid_target(world, self.start, dir, neighbour, data) {
                return Some((reconstruct(&nav, self.start), total));
            }
        }
        if !valid_direction {
            return None;
        }
        let max_distance = (2.0 * total).max(4.0);
        while !open.is_empty() {
            // The node of the lowest score; the first of equals.
            let mut current = None;
            for node in &open {
                if current.is_none_or(|c: Pos| f[node] < f[&c]) {
                    current = Some(*node);
                }
            }
            let current = current?;
            open.remove(&current);
            closed.insert(current);
            if distance(self.start, current) > max_distance {
                continue;
            }
            let score = g[&current];
            for dir in 0..6 {
                let neighbour = offset(current, dir);
                if self.enters(neighbour, dir, current) {
                    let tentative = score + self.nets.map[&neighbour].cost;
                    let better_closed = !closed.contains(&neighbour) || tentative < g.get(&neighbour).copied().unwrap_or(f64::MAX);
                    let better_open = !open.contains(&neighbour) || tentative < g.get(&neighbour).copied().unwrap_or(f64::MAX);
                    if better_closed && better_open {
                        nav.insert(neighbour, current);
                        g.insert(neighbour, tentative);
                        f.insert(neighbour, tentative + distance(neighbour, self.target));
                        open.insert(neighbour);
                    }
                } else if self.valid_target(world, current, dir, neighbour, data) {
                    return Some((reconstruct(&nav, current), score + distance(current, self.target)));
                }
            }
        }
        None
    }
}

/// `TransporterPathfinder.getNewRRPath`'s choice, for a block that sends
/// round robin (the Logistical Sorter, not ported): among `destinations`
/// (the cheapest first), the one after the last target if it is still
/// there, else the first; and the next target.
pub fn round_robin(destinations: &[Destination], last: Option<(Pos, usize)>) -> Option<(usize, (Pos, usize))> {
    let target = |d: &Destination| -> Option<(Pos, usize)> { Some((*d.path.first()?, side_difference(*d.path.get(1)?, d.path[0])?)) };
    let n = destinations.len();
    if n == 0 {
        return None;
    }
    if n > 1
        && let Some(last) = last
    {
        for (i, d) in destinations.iter().enumerate() {
            if target(d) == Some(last) {
                let next = if i == n - 1 { 0 } else { i + 1 };
                return Some((i, target(&destinations[next])?));
            }
        }
    }
    Some((0, target(&destinations[if n > 1 { 1 } else { 0 }])?))
}

/// `BlockPos.asLong`, as a stack saves its locations.
pub fn pack_pos(pos: Pos) -> i64 {
    ((i64::from(pos[0]) & 0x3FF_FFFF) << 38) | ((i64::from(pos[2]) & 0x3FF_FFFF) << 12) | (i64::from(pos[1]) & 0xFFF)
}

/// `BlockPos.of`.
pub fn unpack_pos(v: i64) -> Pos {
    [(v >> 38) as i32, ((v << 52) >> 52) as i32, ((v << 26) >> 38) as i32]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    struct Item(&'static str, u32);

    impl Stack for Item {
        fn count(&self) -> u32 {
            self.1
        }
        fn with_count(&self, count: u32) -> Self {
            Item(self.0, count)
        }
        fn stacks_with(&self, other: &Self) -> bool {
            self.0 == other.0
        }
        fn max_stack(&self) -> u32 {
            64
        }
    }

    /// Chests of 27 slots by position.
    #[derive(Default)]
    struct Chests {
        chests: HashMap<Pos, Vec<Option<Item>>>,
        dropped: Vec<(Pos, Item)>,
    }

    impl Chests {
        fn chest(&mut self, pos: Pos, stacks: &[(usize, Item)]) {
            let mut slots = vec![None; 27];
            for (slot, item) in stacks {
                slots[*slot] = Some(item.clone());
            }
            self.chests.insert(pos, slots);
        }
        fn count(&self, pos: Pos) -> u32 {
            self.chests.get(&pos).map_or(0, |c| c.iter().flatten().map(|i| i.1).sum())
        }
    }

    impl Inventories<Item> for Chests {
        fn has_inventory(&mut self, pos: Pos, _: usize) -> bool {
            self.chests.contains_key(&pos)
        }
        fn insert_slots(&mut self, pos: Pos, _: usize, _: &Item) -> Option<Vec<Slot<Item>>> {
            Some(self.chests.get(&pos)?.iter().map(|s| Slot { stack: s.clone(), accepts: true, limit: 64 }).collect())
        }
        fn insert(&mut self, pos: Pos, _: usize, mut stack: Item) -> Option<Item> {
            let slots = self.chests.get_mut(&pos)?;
            for slot in slots.iter_mut() {
                match slot {
                    None => {
                        *slot = Some(stack);
                        return None;
                    }
                    Some(held) if held.0 == stack.0 && held.1 < 64 => {
                        let n = stack.1.min(64 - held.1);
                        held.1 += n;
                        stack.1 -= n;
                        if stack.1 == 0 {
                            return None;
                        }
                    }
                    _ => {}
                }
            }
            Some(stack)
        }
        fn take_slots(&mut self, pos: Pos, _: usize) -> Vec<(usize, Item)> {
            self.chests.get(&pos).map_or(Vec::new(), |c| c.iter().enumerate().filter_map(|(i, s)| Some((i, s.clone()?))).collect())
        }
        fn extract(&mut self, pos: Pos, _: usize, slot: usize, count: u32) -> Option<Item> {
            let s = self.chests.get_mut(&pos)?.get_mut(slot)?;
            let held = s.as_mut()?;
            let n = count.min(held.1);
            held.1 -= n;
            let out = Item(held.0, n);
            if held.1 == 0 {
                *s = None;
            }
            Some(out)
        }
        fn drop_at(&mut self, pos: Pos, stack: Item) {
            self.dropped.push((pos, stack));
        }
    }

    const N: [Connection; 6] = [Connection::Normal; 6];
    const WEST: usize = 4;
    const EAST: usize = 5;

    fn pull_west() -> [Connection; 6] {
        let mut c = N;
        c[WEST] = Connection::Pull;
        c
    }

    /// A line of transporters from x = 1 with chests at x = 0 and `end`, every
    /// side looked at, then their first tick (they join at its end).
    fn line(speed: u32, pull: u32, n: i32, world: &mut Chests) -> Transporters<Item> {
        let mut nets = Transporters::default();
        for x in 1..=n {
            let c = if x == 1 { pull_west() } else { N };
            nets.add([x, 0, 0], Transporter::new(speed, pull, 50, c, None));
        }
        look(&mut nets, world);
        nets.tick(world);
        nets.commit();
        nets
    }

    fn look(nets: &mut Transporters<Item>, world: &mut Chests) {
        for pos in nets.positions().to_vec() {
            for side in 0..6 {
                let at = offset(pos, side);
                let inventory = nets.get(pos).unwrap().can_connect(side) && nets.get(at).is_none() && world.has_inventory(at, opposite(side));
                nets.set_acceptor(pos, side, inventory);
            }
        }
        nets.changed();
    }

    /// The oracle (`mekm2-work/oracle`, x4 T1): two stacks of 64 in a chest, three transporters, a chest: the first
    /// pull on the tick after they are placed, then every 11 ticks; a basic stack (1 item, 5 a tick) reaches the far
    /// chest 57 ticks after it set out, an advanced one (16, 10 a tick) 27, an elite (32, 20) 12, an ultimate (64, 50)
    /// 3.
    #[test]
    fn stacks_travel_at_the_tier_speed_and_pulls_come_every_11_ticks() {
        for (speed, pull, arrivals, ticks) in [
            (5, 1, vec![(59, 1), (70, 2), (81, 3)], 90),
            (10, 16, vec![(29, 16), (40, 32)], 45),
            (20, 32, vec![(14, 32), (25, 64)], 30),
            (50, 64, vec![(5, 64), (16, 128)], 25),
        ] {
            let mut world = Chests::default();
            world.chest([0, 0, 0], &[(0, Item("cobblestone", 64)), (1, Item("cobblestone", 64))]);
            world.chest([4, 0, 0], &[]);
            let mut nets = line(speed, pull, 3, &mut world);
            let mut seen = Vec::new();
            let mut last = 0;
            let mut pulls = Vec::new();
            let mut source = 128;
            for t in 2..=ticks {
                nets.tick(&mut world);
                let (a, b) = (world.count([0, 0, 0]), world.count([4, 0, 0]));
                if b != last {
                    seen.push((t, b));
                    last = b;
                }
                if a != source {
                    pulls.push(t);
                    source = a;
                }
            }
            assert_eq!(seen, arrivals, "speed {speed}");
            assert_eq!(&pulls[..2], &[2, 13], "speed {speed}");
        }
    }

    /// The oracle (x4 T1): the basic stack's progress through the first transporter, 5 a tick from the tick it is
    /// pulled, and on into the second at 100.
    #[test]
    fn progress_counts_and_carries_into_the_next_transporter() {
        let mut world = Chests::default();
        world.chest([0, 0, 0], &[(0, Item("cobblestone", 64))]);
        world.chest([4, 0, 0], &[]);
        let mut nets = line(5, 1, 3, &mut world);
        let progress = |nets: &Transporters<Item>, x: i32| -> Vec<i32> { nets.get([x, 0, 0]).unwrap().transit.values().map(|s| s.progress).collect() };
        nets.tick(&mut world);
        assert_eq!(progress(&nets, 1), [5]);
        for _ in 3..=20 {
            nets.tick(&mut world);
        }
        assert_eq!(progress(&nets, 1), [95, 40], "t=20");
        nets.tick(&mut world);
        assert_eq!((progress(&nets, 1), progress(&nets, 2)), (vec![45], vec![5]), "t=21: in the second, ticking after the first");
        let stack = nets.get([2, 0, 0]).unwrap().transit.values().next().unwrap();
        assert_eq!((stack.path_type(), stack.dest(), stack.home), (PathType::Dest, Some([4, 0, 0]), Some([0, 0, 0])));
    }

    /// The oracle (x4 T2): an elite line pulling 32 at a time with a chest above its second transporter that has
    /// one free slot and one at its end: the near chest takes two pulls, the third (with 64 on their way there) goes
    /// to the far one.
    #[test]
    fn the_cheapest_target_first_and_stacks_on_their_way_count() {
        let mut world = Chests::default();
        world.chest([0, 0, 0], &[(0, Item("cobblestone", 64)), (1, Item("cobblestone", 64))]);
        let near: Vec<(usize, Item)> = (0..26).map(|s| (s, Item("stone", 64))).collect();
        world.chest([2, 1, 0], &near);
        world.chest([6, 0, 0], &[]);
        let mut nets = line(20, 32, 5, &mut world);
        let mut at = BTreeMap::new();
        for t in 2..=60 {
            nets.tick(&mut world);
            if t % 5 == 0 {
                at.insert(t, (world.count([0, 0, 0]), world.count([2, 1, 0]) - 26 * 64, world.count([6, 0, 0])));
            }
        }
        assert_eq!(at[&10], (96, 32, 0));
        assert_eq!(at[&25], (32, 64, 0));
        assert_eq!(at[&35], (0, 64, 0));
        assert_eq!(at[&45], (0, 64, 32));
        assert_eq!(at[&55], (0, 64, 64));
    }

    /// The oracle (x4 T3, T3b, x5): the target chest taken away while a stack travels: past half way in the last
    /// transporter it turns for home (and back into the second at 0), in its chest on tick 49; with no home either
    /// it goes idle, westward, and turns at the line's end; nothing is dropped.
    #[test]
    fn a_blocked_stack_goes_home_or_wanders() {
        for home in [true, false] {
            let mut world = Chests::default();
            world.chest([0, 0, 0], &[(0, Item("cobblestone", if home { 64 } else { 16 }))]);
            world.chest([4, 0, 0], &[]);
            let mut nets = line(10, 16, 3, &mut world);
            for _ in 2..=6 {
                nets.tick(&mut world);
            }
            world.chests.remove(&[4, 0, 0]);
            if !home {
                world.chests.remove(&[0, 0, 0]);
            }
            look(&mut nets, &mut world);
            let mut seen = BTreeMap::new();
            for t in 7..=48 {
                nets.tick(&mut world);
                for x in 1..=3 {
                    for s in nets.get([x, 0, 0]).unwrap().transit.values() {
                        seen.insert(t, (x, s.progress, s.path_type(), s.idle_dir));
                    }
                }
            }
            let kind = if home { PathType::Home } else { PathType::None };
            assert_eq!(seen[&20], (3, 10, PathType::Dest, None));
            assert_eq!((seen[&24].0, seen[&24].1, seen[&24].2), (3, 50, kind), "turned past half way in the last");
            assert_eq!((seen[&32].0, seen[&32].1), (2, 30), "back west, at 0 in the second (ticked before)");
            if home {
                // x5 T3c: back in its chest on tick 49.
                assert_eq!(world.count([0, 0, 0]), 48);
                nets.tick(&mut world);
                assert_eq!(world.count([0, 0, 0]), 64, "t=49");
            } else {
                assert_eq!(seen[&24].3, Some(WEST));
                assert_eq!((seen[&40].0, seen[&40].1), (1, 10));
                assert_eq!((seen[&44].0, seen[&44].1, seen[&44].3), (1, 50, Some(EAST)), "turned at the end");
            }
            assert!(world.dropped.is_empty());
        }
    }

    /// The oracle (x4 T4): a red puller beside a blue transporter pulls nothing, a plain puller beside a red one
    /// nothing; a red puller beside a plain one sends red stacks through it.
    #[test]
    fn colours_keep_transporters_and_stacks_apart() {
        for (first, second, passes) in [(Some(Color::Red), Some(Color::DarkBlue), false), (None, Some(Color::Red), false), (Some(Color::Red), None, true)] {
            let mut world = Chests::default();
            world.chest([0, 0, 0], &[(0, Item("cobblestone", 64))]);
            world.chest([3, 0, 0], &[]);
            let mut nets = Transporters::default();
            nets.add([1, 0, 0], Transporter::new(20, 32, 50, pull_west(), first));
            nets.add([2, 0, 0], Transporter::new(20, 32, 50, N, second));
            look(&mut nets, &mut world);
            nets.tick(&mut world);
            nets.commit();
            for _ in 2..=20 {
                nets.tick(&mut world);
            }
            assert_eq!(world.count([0, 0, 0]) < 64, passes, "{first:?} {second:?}");
            assert_eq!(world.count([3, 0, 0]) > 0, passes);
            assert_eq!(nets.linked([1, 0, 0], EAST), first.is_none() || second.is_none());
            if passes {
                assert!(nets.get([2, 0, 0]).unwrap().transit.values().all(|s| s.color == Some(Color::Red)));
            }
        }
        assert_eq!(Color::increment(None), Some(Color::Black));
        assert_eq!(Color::increment(Some(Color::BrightPink)), None);
        assert_eq!(Color::decrement(None), Some(Color::BrightPink));
        assert_eq!(Color::by_id(12), Color::Red);
    }

    /// The oracle (x4 T5): a chest of 10 cobblestone in slot 0, 10 dirt in 1 and 10 cobblestone in 5: a pull of 16
    /// takes one kind, the last slot's first: 10 and then 6 cobblestone.
    #[test]
    fn a_pull_takes_one_kind_the_last_slot_first() {
        let slots = vec![(0, Item("cobblestone", 10)), (1, Item("dirt", 10)), (5, Item("cobblestone", 10))];
        let request = Request::any_item(&slots, 16);
        assert_eq!(request.items.len(), 2);
        assert_eq!((request.items[0].kind.0, request.items[0].total), ("cobblestone", 16));
        assert_eq!(request.items[0].slots, [(5, 10), (0, 6)]);
        assert_eq!(request.taken(Response { index: 0, count: 12 }), [(5, 10), (0, 2)]);
        assert_eq!((request.items[1].kind.0, request.items[1].total), ("dirt", 10));
    }

    /// `RestrictiveTransporter.getCost`: 1000 a step, so a stack takes a longer path of basic transporters (10 a
    /// step) round one to the same kind of target, and goes through it only when there is no other way.
    #[test]
    fn a_restrictive_transporter_is_the_last_way() {
        let mut world = Chests::default();
        world.chest([0, 0, 0], &[(0, Item("cobblestone", 64))]);
        world.chest([3, 0, 0], &[]);
        world.chest([1, 3, 0], &[]);
        let mut nets = Transporters::default();
        nets.add([1, 0, 0], Transporter::new(50, 64, 50, pull_west(), None));
        let mut restrictive = Transporter::new(5, 1, 50, N, None);
        restrictive.cost = 1000.0;
        nets.add([2, 0, 0], restrictive);
        for y in 1..=2 {
            nets.add([1, y, 0], Transporter::new(50, 64, 50, N, None));
        }
        look(&mut nets, &mut world);
        nets.tick(&mut world);
        nets.commit();
        nets.tick(&mut world);
        let stack = nets.get([1, 0, 0]).unwrap().transit.values().next().unwrap();
        assert_eq!(stack.dest(), Some([1, 3, 0]), "two ultimate steps, not one restrictive");
        world.chests.remove(&[1, 3, 0]);
        look(&mut nets, &mut world);
        for _ in 0..80 {
            nets.tick(&mut world);
        }
        assert_eq!(world.count([3, 0, 0]), 64, "through it (at the basic speed) when there is no other way");
    }

    #[test]
    fn round_robin_takes_the_target_after_the_last() {
        let d = |x: i32| Destination { path: vec![[x, 1, 0], [x, 0, 0]], score: f64::from(x), response: None, path_type: PathType::Dest };
        let all = [d(1), d(2), d(3)];
        let down = 0;
        assert_eq!(round_robin(&all, None), Some((0, ([2, 1, 0], down))));
        assert_eq!(round_robin(&all, Some(([2, 1, 0], down))), Some((1, ([3, 1, 0], down))));
        assert_eq!(round_robin(&all, Some(([3, 1, 0], down))), Some((2, ([1, 1, 0], down))), "wraps");
        assert_eq!(round_robin(&all, Some(([9, 1, 0], down))), Some((0, ([2, 1, 0], down))), "gone: the first");
        assert_eq!(round_robin(&all[..1], Some(([1, 1, 0], down))), Some((0, ([1, 1, 0], down))));
    }

    #[test]
    fn positions_pack_as_block_pos_as_long() {
        // The oracle's T6: original_location 720838 is (0, -58, 175).
        assert_eq!(unpack_pos(720_838), [0, -58, 175]);
        for p in [[0, -58, 175], [-30_000_000, -2048, 29_999_999], [1, 2, 3]] {
            assert_eq!(unpack_pos(pack_pos(p)), p);
        }
    }
}
