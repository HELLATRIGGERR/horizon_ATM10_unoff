//! ME Requester's request logic without the game: the request rows, the
//! per-row storage buffers and the status machine an ME Requester runs on
//! every grid tick.
//!
//! Written from the behaviour of ME Requester 1.21.1-1.5.0 by AlmostReliable
//! (github.com/AlmostReliable/merequester, tag `v1.21.1-neoforge-1.5.0`,
//! LGPL-3.0): `Request`, `StorageManager.Storage`, the states under
//! `requester/status` and `RequesterBlockEntity.handleRequests`. The game
//! supplies keys (`K`, an item or fluid), crafting links (`L`) and the grid
//! through [`Host`].

/// A request's status as the mod names it (`RequestStatus`), in its order:
/// the client status is saved and sent as this ordinal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Status {
    Idle,
    Missing,
    Cpu,
    Request,
    Plan,
    Link,
    Export,
}
impl Status {
    pub const ALL: [Status; 7] = [Self::Idle, Self::Missing, Self::Cpu, Self::Request, Self::Plan, Self::Link, Self::Export];
    pub fn ordinal(self) -> i32 {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0) as i32
    }
    /// `ByIdMap.continuous(..., ZERO)`: an unknown ordinal is Idle.
    pub fn from_ordinal(ordinal: i32) -> Self {
        usize::try_from(ordinal).ok().and_then(|i| Self::ALL.get(i).copied()).unwrap_or(Self::Idle)
    }
    /// `translateToClient`: planning shows as idling.
    pub fn to_client(self) -> Self {
        match self {
            Self::Request | Self::Plan => Self::Idle,
            other => other,
        }
    }
    /// `locksRequest`: a row can't be edited while its job runs or exports.
    pub fn locks_request(self) -> bool {
        matches!(self, Self::Link | Self::Export)
    }
    /// `getSerializedName`, and the tooltip keys `tooltip.merequester.<name>`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Missing => "missing",
            Self::Cpu => "cpu",
            Self::Request => "request",
            Self::Plan => "plan",
            Self::Link => "link",
            Self::Export => "export",
        }
    }
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == name)
    }
}

/// AE2's `TickRateModulation`, least urgent first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Modulation {
    Sleep,
    Idle,
    Slower,
    Same,
    Faster,
    Urgent,
}

/// What a change to a row asks of its host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// Nothing changed.
    None,
    /// `saveChanges`.
    Saved,
    /// `requestChanged`: the key changed; the row's storage is cleared and
    /// its known amount read again.
    KeyChanged,
}

/// One request row (`Request`).
#[derive(Clone, Debug, PartialEq)]
pub struct Request<K> {
    pub state: bool,
    pub key: Option<K>,
    pub amount: i64,
    pub batch: i64,
    /// What the client shows; the server's status is the row's [`State`].
    pub client_status: Status,
}
impl<K> Default for Request<K> {
    fn default() -> Self {
        Self { state: true, key: None, amount: 0, batch: 1, client_status: Status::Idle }
    }
}
impl<K: Clone + PartialEq> Request<K> {
    pub fn update_state(&mut self, state: bool) -> Change {
        if self.state == state {
            return Change::None;
        }
        self.state = state;
        Change::Saved
    }
    /// A key-less row or an amount below one clears the row.
    pub fn update_amount(&mut self, amount: i64) -> Change {
        if self.key.is_none() || amount <= 0 {
            return self.reset();
        }
        if self.amount == amount {
            return Change::None;
        }
        self.amount = amount;
        Change::Saved
    }
    /// `Mth.clamp(batch, 1, batch)`: at least one.
    pub fn update_batch(&mut self, batch: i64) -> Change {
        let old = self.batch;
        self.batch = batch.max(1);
        if old == self.batch { Change::None } else { Change::Saved }
    }
    /// `updateKey`: a new key takes the stack's amount and its unit as the
    /// batch (1 for an item, 1,000 for a fluid); the same key only its amount.
    pub fn update_key(&mut self, stack: Option<(K, i64)>, amount_per_unit: i64) -> Change {
        let Some((key, amount)) = stack else {
            return if self.key.is_some() { self.reset() } else { Change::None };
        };
        if self.key.as_ref() == Some(&key) {
            return if self.amount != amount { self.update_amount(amount) } else { Change::None };
        }
        self.key = Some(key);
        self.amount = amount;
        self.batch = amount_per_unit;
        Change::KeyChanged
    }
    fn reset(&mut self) -> Change {
        if self.key.is_none() && self.amount == 0 {
            return Change::None;
        }
        self.key = None;
        self.amount = 0;
        self.batch = 1;
        Change::KeyChanged
    }
    pub fn is_requesting(&self) -> bool {
        self.state && self.key.is_some()
    }
}

/// A row's buffer of crafted results on their way into the network
/// (`StorageManager.Storage`).
#[derive(Clone, Debug, PartialEq)]
pub struct Storage<K> {
    pub key: Option<K>,
    /// What the running job still has to deliver; not saved.
    pub total: i64,
    pub buffer: i64,
    /// Exported but not seen in the network yet.
    pub pending: i64,
    /// The network's amount of the row's key, -1 before it is known.
    pub known: i64,
}
impl<K> Default for Storage<K> {
    fn default() -> Self {
        Self { key: None, total: 0, buffer: 0, pending: 0, known: -1 }
    }
}
impl<K: Clone + PartialEq> Storage<K> {
    pub fn buffer_amount(&self) -> i64 {
        if self.key.is_none() { 0 } else { self.buffer }
    }
    /// `inserted` went into the network: true while the buffer or the job
    /// has more.
    pub fn compute(&mut self, inserted: i64) -> bool {
        self.pending += inserted;
        self.buffer = self.buffer_amount() - inserted;
        self.total -= inserted;
        if self.buffer == 0 {
            self.key = None;
        }
        self.buffer > 0 || self.total > 0
    }
    /// Crafted results arrive; false for another key than the buffer holds.
    pub fn update(&mut self, key: K, amount: i64) -> bool {
        if self.key.as_ref().is_some_and(|k| *k != key) {
            return false;
        }
        self.key = Some(key);
        self.buffer += amount;
        true
    }
}

/// A row's server status (`StatusState`). Plan, Missing and Cpu are new
/// objects each time in the mod, so they carry an id: a state equals only
/// itself, which decides whether `handleRequest` runs again.
#[derive(Clone, Debug, PartialEq)]
pub enum State<L> {
    Idle,
    Request,
    /// A crafting calculation runs for the row (`PlanState`).
    Plan(u64),
    /// A job runs on a CPU (`LinkState`).
    Link(L),
    Export,
    /// `MissingState`; `probe` while its own calculation runs.
    Missing { id: u64, probe: bool },
    /// `CpuState`, the same.
    Cpu { id: u64, probe: bool },
}
impl<L: PartialEq> State<L> {
    pub fn status(&self) -> Status {
        match self {
            Self::Idle => Status::Idle,
            Self::Request => Status::Request,
            Self::Plan(_) => Status::Plan,
            Self::Link(_) => Status::Link,
            Self::Export => Status::Export,
            Self::Missing { .. } => Status::Missing,
            Self::Cpu { .. } => Status::Cpu,
        }
    }
    /// `Objects.equals` on the mod's state objects.
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Idle, Self::Idle) | (Self::Request, Self::Request) | (Self::Export, Self::Export) => true,
            (Self::Plan(a), Self::Plan(b)) => a == b,
            (Self::Link(a), Self::Link(b)) => a == b,
            (Self::Missing { id: a, .. }, Self::Missing { id: b, .. }) | (Self::Cpu { id: a, .. }, Self::Cpu { id: b, .. }) => a == b,
            _ => false,
        }
    }
}

/// What a finished crafting calculation came to (`PlanState.handle`).
#[derive(Clone, Debug, PartialEq)]
pub enum Planned<L> {
    /// Still calculating.
    Pending,
    /// Cancelled, failed, or the submission failed for another reason than a CPU.
    Idle,
    /// The plan misses ingredients.
    Missing,
    /// No CPU took the job (`NO_CPU_FOUND`, `NO_SUITABLE_CPU_FOUND`, `CPU_BUSY`, `CPU_OFFLINE`, `CPU_TOO_SMALL`).
    Cpu,
    /// Submitted: the job's link and its final output amount.
    Linked { link: L, total: i64 },
}

/// A crafting link's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkState {
    Running,
    Done,
    Cancelled,
}

/// What the requester needs of the grid.
pub trait Host<K, L> {
    /// `beginCraftingCalculation(key, amount, REPORT_MISSING_ITEMS)` for row `slot`.
    fn begin_calculation(&mut self, slot: usize, key: &K, amount: i64);
    /// Whether the row's calculation has finished (`future.isDone() && !isCancelled()`).
    fn calculation_done(&self, slot: usize) -> bool;
    /// The finished calculation: submitted if it can be.
    fn finish_calculation(&mut self, slot: usize) -> Planned<L>;
    /// The state of row `slot`'s job.
    fn link_state(&self, slot: usize, link: &L) -> LinkState;
    /// `StorageHelper.poweredInsert`: how much of `amount` the network took.
    fn insert(&mut self, key: &K, amount: i64) -> i64;
}

/// What one grid tick did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tick {
    /// A row's state changed (`setChanged`).
    pub changed: bool,
    /// A row's client status changed (`markForUpdate`).
    pub client_changed: bool,
    pub modulation: Modulation,
}

enum Next<L> {
    Same,
    New(State<L>),
}

/// An ME Requester's rows (`RequestManager`), their buffers
/// (`StorageManager`) and statuses.
#[derive(Clone, Debug, PartialEq)]
pub struct Requester<K, L> {
    pub requests: Vec<Request<K>>,
    pub storages: Vec<Storage<K>>,
    pub states: Vec<State<L>>,
    /// The last id given to a Plan, Missing or Cpu state; a host that keeps
    /// the states between ticks keeps this with them.
    pub serial: u64,
}
impl<K: Clone + PartialEq, L: Clone + PartialEq> Requester<K, L> {
    /// `size` rows (the `requests` setting).
    pub fn new(size: usize) -> Self {
        Self {
            requests: (0..size).map(|_| Request::default()).collect(),
            storages: (0..size).map(|_| Storage::default()).collect(),
            states: (0..size).map(|_| State::Idle).collect(),
            serial: 1,
        }
    }
    pub fn size(&self) -> usize {
        self.requests.len()
    }
    /// The first row without a key (`firstAvailableIndex`).
    pub fn first_available(&self) -> Option<usize> {
        self.requests.iter().position(|r| r.key.is_none())
    }
    /// The stack watcher saw the network's amount of `key` change.
    pub fn stock_changed(&mut self, key: &K, amount: i64) {
        for (request, storage) in self.requests.iter().zip(&mut self.storages) {
            if request.key.as_ref() == Some(key) {
                storage.known = amount;
                storage.pending = 0;
            }
        }
    }
    /// `requestChanged`/`clear`: the row's key changed; `known` is the
    /// network's amount of the new key.
    pub fn key_changed(&mut self, slot: usize, known: Option<i64>) {
        if let Some(storage) = self.storages.get_mut(slot) {
            storage.known = -1;
            if self.requests[slot].key.is_some() {
                if let Some(known) = known {
                    storage.known = known;
                }
            }
        }
    }
    /// `computeAmountToCraft`: a whole batch once the stock is below the amount.
    pub fn amount_to_craft(&self, slot: usize) -> i64 {
        let request = &self.requests[slot];
        if request.key.is_none() {
            return 0;
        }
        let storage = &self.storages[slot];
        if storage.known + storage.pending < request.amount { request.batch } else { 0 }
    }
    /// `isActive`: a row shows something else than idling.
    pub fn active(&self) -> bool {
        self.states.iter().any(|s| s.status().to_client() != Status::Idle)
    }
    /// The running jobs (`getRequestedJobs`).
    pub fn links(&self) -> impl Iterator<Item = (usize, &L)> {
        self.states.iter().enumerate().filter_map(|(i, s)| match s {
            State::Link(link) => Some((i, link)),
            _ => None,
        })
    }
    /// `insertCraftedItems`: the job of `link` delivers; none when no row
    /// runs it.
    pub fn insert_crafted(&mut self, link: &L, key: K, amount: i64, simulate: bool) -> Option<i64> {
        let slot = self.states.iter().position(|s| matches!(s, State::Link(l) if l == link))?;
        if !simulate && !self.storages[slot].update(key, amount) {
            return Some(0);
        }
        Some(amount)
    }
    fn fresh(&mut self) -> u64 {
        self.serial += 1;
        self.serial
    }
    /// One grid tick (`handleRequests`).
    pub fn tick(&mut self, host: &mut impl Host<K, L>) -> Tick {
        let mut tick = Tick { changed: false, client_changed: false, modulation: Modulation::Idle };
        for slot in 0..self.size() {
            let old = self.states[slot].clone();
            self.handle_request(slot, host, &mut tick, 0);
            // `updateRequestStatus` of the result: a row whose saved client
            // status disagrees (a Memory Card's copy) shows its own again.
            let client = self.states[slot].status().to_client();
            if self.requests[slot].client_status != client {
                self.requests[slot].client_status = client;
                tick.client_changed = true;
            }
            if !old.same(&self.states[slot]) {
                tick.changed = true;
            }
            let modulation = self.modulation(slot, host);
            if modulation > tick.modulation {
                tick.modulation = modulation;
            }
        }
        tick
    }
    fn modulation(&self, slot: usize, host: &impl Host<K, L>) -> Modulation {
        match &self.states[slot] {
            State::Idle | State::Missing { .. } | State::Cpu { .. } => Modulation::Idle,
            State::Request => Modulation::Slower,
            State::Plan(_) => if host.calculation_done(slot) { Modulation::Urgent } else { Modulation::Slower },
            State::Link(_) => Modulation::Same,
            State::Export => Modulation::Urgent,
        }
    }
    /// `handleRequest`: a state that moves on to something else than idling
    /// is handled again in the same tick.
    fn handle_request(&mut self, slot: usize, host: &mut impl Host<K, L>, tick: &mut Tick, depth: u32) {
        let next = self.handle(slot, host);
        let Next::New(state) = next else {
            return;
        };
        let again = state.status() != Status::Idle && !state.same(&self.states[slot]);
        self.set_state(slot, state, tick);
        if again && depth < 64 {
            self.handle_request(slot, host, tick, depth + 1);
        }
    }
    fn set_state(&mut self, slot: usize, state: State<L>, tick: &mut Tick) {
        let client = state.status().to_client();
        self.states[slot] = state;
        if self.requests[slot].client_status != client {
            self.requests[slot].client_status = client;
            tick.client_changed = true;
        }
    }
    fn handle(&mut self, slot: usize, host: &mut impl Host<K, L>) -> Next<L> {
        match self.states[slot].clone() {
            State::Idle => self.idle(slot),
            State::Request => self.request(slot, host),
            State::Plan(_) => self.plan(slot, host),
            State::Link(link) => match host.link_state(slot, &link) {
                LinkState::Running => Next::Same,
                LinkState::Done => Next::New(State::Export),
                LinkState::Cancelled => Next::New(State::Idle),
            },
            State::Export => self.export(slot, host),
            State::Missing { probe, .. } | State::Cpu { probe, .. } => self.blocking(slot, probe, host),
        }
    }
    /// `IdleState`.
    fn idle(&self, slot: usize) -> Next<L> {
        if self.storages[slot].buffer_amount() > 0 {
            return Next::New(State::Export);
        }
        let request = &self.requests[slot];
        if request.is_requesting() && request.amount > self.storages[slot].known {
            return Next::New(State::Request);
        }
        Next::Same
    }
    /// `RequestState`.
    fn request(&mut self, slot: usize, host: &mut impl Host<K, L>) -> Next<L> {
        if !self.requests[slot].state {
            return Next::New(State::Idle);
        }
        let amount = self.amount_to_craft(slot);
        if amount <= 0 {
            return Next::New(State::Idle);
        }
        let Some(key) = self.requests[slot].key.clone() else {
            return Next::New(State::Idle);
        };
        host.begin_calculation(slot, &key, amount);
        let id = self.fresh();
        Next::New(State::Plan(id))
    }
    /// `PlanState`.
    fn plan(&mut self, slot: usize, host: &mut impl Host<K, L>) -> Next<L> {
        match host.finish_calculation(slot) {
            Planned::Pending => Next::Same,
            Planned::Idle => Next::New(State::Idle),
            Planned::Missing => {
                let id = self.fresh();
                Next::New(State::Missing { id, probe: false })
            }
            Planned::Cpu => {
                let id = self.fresh();
                Next::New(State::Cpu { id, probe: false })
            }
            Planned::Linked { link, total } => {
                self.storages[slot].total = total;
                Next::New(State::Link(link))
            }
        }
    }
    /// `ExportState`.
    fn export(&mut self, slot: usize, host: &mut impl Host<K, L>) -> Next<L> {
        let storage = &self.storages[slot];
        let Some(key) = storage.key.clone() else {
            return Next::New(State::Idle);
        };
        let buffered = storage.buffer_amount();
        let inserted = host.insert(&key, buffered).clamp(0, buffered);
        if self.storages[slot].compute(inserted) {
            return Next::Same;
        }
        Next::New(if inserted > 0 { State::Request } else { State::Idle })
    }
    /// `BlockingState`: Missing Ingredients and No CPU try the request again
    /// on each tick they get, and go back to idling once it isn't needed.
    fn blocking(&mut self, slot: usize, probe: bool, host: &mut impl Host<K, L>) -> Next<L> {
        if probe {
            if let State::Missing { probe, .. } | State::Cpu { probe, .. } = &mut self.states[slot] {
                *probe = false;
            }
            // The simulated PlanState: still calculating shows as planning.
            return match self.plan(slot, host) {
                Next::Same => {
                    let id = self.fresh();
                    Next::New(State::Plan(id))
                }
                next => next,
            };
        }
        if !matches!(self.idle(slot), Next::New(State::Request)) {
            return Next::New(State::Idle);
        }
        match self.request(slot, host) {
            Next::New(State::Plan(_)) => {
                if let State::Missing { probe, .. } | State::Cpu { probe, .. } = &mut self.states[slot] {
                    *probe = true;
                }
                Next::Same
            }
            _ => Next::New(State::Idle),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grid with one pattern and a stock the test sets.
    #[derive(Default)]
    struct Grid {
        stock: i64,
        calculations: Vec<(usize, i64)>,
        /// What the next finished calculation comes to, and how many polls it takes.
        outcome: Option<Planned<u32>>,
        polls_left: u32,
        pending: Vec<usize>,
        links: Vec<(u32, LinkState)>,
        next_link: u32,
        /// How much an insert takes at most.
        room: i64,
        inserted: i64,
    }
    impl Host<&'static str, u32> for Grid {
        fn begin_calculation(&mut self, slot: usize, _key: &&'static str, amount: i64) {
            self.calculations.push((slot, amount));
            self.pending.push(slot);
        }
        fn calculation_done(&self, slot: usize) -> bool {
            self.pending.contains(&slot) && self.polls_left == 0
        }
        fn finish_calculation(&mut self, slot: usize) -> Planned<u32> {
            if !self.pending.contains(&slot) {
                return Planned::Idle;
            }
            if self.polls_left > 0 {
                self.polls_left -= 1;
                return Planned::Pending;
            }
            self.pending.retain(|s| *s != slot);
            match self.outcome.clone() {
                Some(Planned::Linked { total, .. }) => {
                    self.next_link += 1;
                    self.links.push((self.next_link, LinkState::Running));
                    Planned::Linked { link: self.next_link, total }
                }
                Some(other) => other,
                None => Planned::Idle,
            }
        }
        fn link_state(&self, _slot: usize, link: &u32) -> LinkState {
            self.links.iter().find(|(l, _)| l == link).map_or(LinkState::Cancelled, |(_, s)| *s)
        }
        fn insert(&mut self, _key: &&'static str, amount: i64) -> i64 {
            let taken = amount.min(self.room);
            self.room -= taken;
            self.stock += taken;
            self.inserted += taken;
            taken
        }
    }
    fn requester(amount: i64, batch: i64) -> Requester<&'static str, u32> {
        let mut r = Requester::new(5);
        assert_eq!(r.requests[0].update_key(Some(("minecraft:stick", amount)), 1), Change::KeyChanged);
        r.key_changed(0, Some(0));
        assert_eq!(r.requests[0].update_batch(batch), if batch == 1 { Change::None } else { Change::Saved });
        r
    }
    fn tick(r: &mut Requester<&'static str, u32>, grid: &mut Grid) -> Tick {
        r.stock_changed(&"minecraft:stick", grid.stock);
        r.tick(grid)
    }

    #[test]
    fn rows_follow_the_mods_update_rules() {
        let mut r: Request<&str> = Request::default();
        assert_eq!((r.state, r.amount, r.batch, r.client_status), (true, 0, 1, Status::Idle));
        assert_eq!(r.update_amount(10), Change::None, "no key: nothing to reset");
        assert_eq!(r.update_key(Some(("minecraft:water", 1000)), 1000), Change::KeyChanged);
        assert_eq!((r.amount, r.batch), (1000, 1000), "a fluid's batch is a bucket");
        assert_eq!(r.update_key(Some(("minecraft:water", 2000)), 1000), Change::Saved, "same key: only the amount");
        assert_eq!(r.update_batch(0), Change::Saved);
        assert_eq!(r.batch, 1, "Mth.clamp(batch, 1, batch)");
        assert_eq!(r.update_batch(-5), Change::None);
        assert_eq!(r.update_amount(0), Change::KeyChanged, "an amount below one clears the row");
        assert_eq!((r.key, r.amount, r.batch), (None, 0, 1));
        assert_eq!(r.update_key(None, 1), Change::None);
        assert!(!r.is_requesting());
        assert_eq!(Status::from_ordinal(5), Status::Link);
        assert_eq!(Status::from_ordinal(99), Status::Idle);
        assert_eq!(Status::Plan.to_client(), Status::Idle);
        assert!(Status::Export.locks_request() && !Status::Cpu.locks_request());
    }

    #[test]
    fn keeps_a_stock_crafting_then_idling() {
        // Keep 100 sticks with batch 64; 50 are in the network.
        let mut r = requester(100, 64);
        let mut grid = Grid { stock: 50, outcome: Some(Planned::Linked { link: 0, total: 64 }), polls_left: 2, room: i64::MAX, ..Default::default() };
        let t = tick(&mut r, &mut grid);
        assert_eq!(grid.calculations, [(0, 64)], "one whole batch is asked for");
        assert_eq!(r.states[0].status(), Status::Plan);
        assert_eq!(r.requests[0].client_status, Status::Idle, "planning shows as idling");
        assert_eq!(t.modulation, Modulation::Slower);
        let t = tick(&mut r, &mut grid);
        assert_eq!(r.states[0].status(), Status::Plan);
        assert!(!t.changed);
        let t = tick(&mut r, &mut grid);
        assert_eq!(r.states[0], State::Link(1));
        assert!(t.changed && t.client_changed && r.active());
        assert_eq!(r.requests[0].client_status, Status::Link);
        assert_eq!(r.storages[0].total, 64);
        // The CPU delivers the job into the row's buffer, then finishes.
        assert_eq!(r.insert_crafted(&1, "minecraft:stick", 64, false), Some(64));
        assert_eq!(r.insert_crafted(&7, "minecraft:stick", 1, false), None);
        grid.links[0].1 = LinkState::Done;
        let t = tick(&mut r, &mut grid);
        assert_eq!(grid.stock, 114);
        assert_eq!(r.storages[0].buffer_amount(), 0);
        assert_eq!(r.storages[0].pending, 64, "exported, not seen yet");
        assert_eq!(r.states[0], State::Idle, "114 >= 100: no further job");
        assert_eq!(grid.calculations.len(), 1);
        assert!(!r.active() && t.client_changed);
        assert_eq!(t.modulation, Modulation::Idle);
        tick(&mut r, &mut grid);
        assert_eq!(r.storages[0].pending, 0, "the watcher saw the stock");
        assert_eq!(grid.calculations.len(), 1);
    }

    #[test]
    fn a_short_stock_asks_again_right_after_exporting() {
        let mut r = requester(100, 10);
        let mut grid = Grid { stock: 0, outcome: Some(Planned::Linked { link: 0, total: 10 }), room: i64::MAX, ..Default::default() };
        tick(&mut r, &mut grid);
        assert_eq!(r.states[0], State::Link(1), "a calculation done at once links in the same tick");
        r.insert_crafted(&1, "minecraft:stick", 10, false);
        grid.links[0].1 = LinkState::Done;
        tick(&mut r, &mut grid);
        assert_eq!(grid.stock, 10);
        assert_eq!(grid.calculations, [(0, 10), (0, 10)], "export, then request again in the same tick");
        assert_eq!(r.states[0], State::Link(2));
    }

    #[test]
    fn missing_ingredients_retry_and_recover() {
        let mut r = requester(10, 1);
        let mut grid = Grid { outcome: Some(Planned::Missing), ..Default::default() };
        let t = tick(&mut r, &mut grid);
        assert_eq!(r.requests[0].client_status, Status::Missing);
        assert_eq!(t.modulation, Modulation::Idle);
        // A new Missing state tries again at once, with a calculation of its own.
        assert!(matches!(r.states[0], State::Missing { probe: true, .. }));
        assert_eq!(grid.calculations.len(), 2);
        // Each later tick takes that result and starts the next one.
        tick(&mut r, &mut grid);
        assert!(matches!(r.states[0], State::Missing { probe: true, .. }));
        assert_eq!(grid.calculations.len(), 3);
        // The ingredients arrive: the probe's plan links.
        grid.outcome = Some(Planned::Linked { link: 0, total: 1 });
        tick(&mut r, &mut grid);
        assert!(matches!(r.states[0], State::Link(_)));
        // No CPU: the same, with its own status.
        let mut r = requester(10, 1);
        let mut grid = Grid { outcome: Some(Planned::Cpu), ..Default::default() };
        tick(&mut r, &mut grid);
        assert_eq!(r.requests[0].client_status, Status::Cpu);
        // The stock filled up meanwhile: back to idling without asking.
        grid.stock = 10;
        tick(&mut r, &mut grid);
        assert_eq!(r.states[0], State::Idle);
        assert_eq!(grid.calculations.len(), 2, "the probe's result is taken, nothing new is asked");
    }

    #[test]
    fn a_full_network_keeps_exporting() {
        let mut r = requester(100, 64);
        let mut grid = Grid { outcome: Some(Planned::Linked { link: 0, total: 64 }), room: 40, ..Default::default() };
        tick(&mut r, &mut grid);
        r.insert_crafted(&1, "minecraft:stick", 64, false);
        grid.links[0].1 = LinkState::Done;
        let t = tick(&mut r, &mut grid);
        assert_eq!(r.states[0], State::Export);
        assert_eq!(r.requests[0].client_status, Status::Export);
        assert_eq!(r.storages[0].buffer_amount(), 24);
        assert_eq!(t.modulation, Modulation::Urgent);
        tick(&mut r, &mut grid);
        assert_eq!(r.states[0], State::Export, "nothing fits");
        grid.room = 100;
        tick(&mut r, &mut grid);
        assert_eq!(grid.inserted, 64);
        assert_eq!(r.storages[0].key, None);
    }

    #[test]
    fn disabled_rows_and_cancelled_jobs_idle() {
        let mut r = requester(10, 1);
        r.requests[0].update_state(false);
        let mut grid = Grid { outcome: Some(Planned::Linked { link: 0, total: 1 }), ..Default::default() };
        tick(&mut r, &mut grid);
        assert_eq!(r.states[0], State::Idle);
        assert!(grid.calculations.is_empty(), "a switched-off row asks for nothing");
        r.requests[0].update_state(true);
        tick(&mut r, &mut grid);
        assert_eq!(r.states[0], State::Link(1));
        grid.links[0].1 = LinkState::Cancelled;
        let t = tick(&mut r, &mut grid);
        assert_eq!(r.states[0], State::Idle);
        assert!(t.changed && t.client_changed);
        assert_eq!(grid.calculations.len(), 1);
        // The next tick asks again for the stock still missing.
        tick(&mut r, &mut grid);
        assert_eq!(grid.calculations.len(), 2);
        // A failed submission idles.
        let mut r = requester(10, 1);
        let mut grid = Grid { outcome: Some(Planned::Idle), ..Default::default() };
        tick(&mut r, &mut grid);
        assert_eq!(r.states[0], State::Idle);
    }

    #[test]
    fn storage_buffers_one_key() {
        let mut s: Storage<&str> = Storage::default();
        assert_eq!(s.known, -1);
        assert!(s.update("a", 5));
        assert!(!s.update("b", 1));
        s.total = 5;
        assert!(s.compute(3));
        assert_eq!((s.buffer, s.pending, s.total), (2, 3, 2));
        assert!(!s.compute(2));
        assert_eq!(s.key, None);
    }
}
