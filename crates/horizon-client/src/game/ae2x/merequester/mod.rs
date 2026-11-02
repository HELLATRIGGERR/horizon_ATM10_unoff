//! ME Requester 1.21.1-1.5.0 in the game (feature `ae2-merequester`): the
//! requester block on the grid, its tick, its crafting jobs and its rows'
//! edits. Derived from ME Requester by AlmostReliable
//! (`RequesterBlockEntity`, `StorageManager`, `AbstractRequesterMenu`),
//! https://github.com/AlmostReliable/merequester tag `v1.21.1-neoforge-1.5.0`,
//! LGPL-3.0; see `ae2/addons/merequester/`. The request logic itself is
//! `ae2-merequester-core`, the saved data `horizon_client::ae2x::merequester`.
use super::super::{Game, ae2_global::GridAddress, ae2_lab};
use horizon_client::{
    ae2_world::{Machine, MachineKind},
    ae2x::merequester::{self as rows, Config, Key, Link, Requester, core},
    containers::BlockEntity,
};
use horizon_world::nbt::Tag;
use std::collections::{BTreeMap, HashMap};

mod lab;
pub(in crate::game) mod menu;
pub(in crate::game) mod screen;

/// A requester's identity for its crafting jobs: the block's grid address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(in crate::game) struct RequesterOwner {
    pub(in crate::game) host: GridAddress,
}

/// What the requesters keep while the game runs: the rows' statuses the mod
/// does not save, the open menus and what this client shows.
#[derive(Default)]
pub(in crate::game) struct State {
    runtime: HashMap<GridAddress, Runtime>,
    pub(in crate::game) sessions: HashMap<u32, menu::Session>,
    pub(in crate::game) client: Option<screen::View>,
    /// Why each requester's last request came to nothing (`/ae2 requester status`).
    pub(in crate::game) notes: HashMap<GridAddress, String>,
}
struct Runtime {
    incarnation: u64,
    states: Vec<core::State<Link>>,
    serial: u64,
    /// Jobs submitted since the requester loaded (for `/ae2 requester status`).
    jobs: u64,
}

/// The requester block's machine kind (`MachineKind::Ext` of its row).
pub(in crate::game) fn kind() -> Option<MachineKind> {
    MachineKind::from_id(rows::REQUESTER)
}
pub(in crate::game) fn is_requester(machine: &Machine) -> bool {
    machine.kind.ext().is_some_and(|row| row.id == rows::REQUESTER)
}

/// The requester's grid node: `idle_energy`, `require_channel`, and every
/// side but its front (`getExposedSides`).
pub(super) fn device(machine: &Machine, properties: Option<&horizon_world::block_state_properties::BlockProperties>, device: &mut ae2_network_core::Device) -> bool {
    if !is_requester(machine) {
        return false;
    }
    let config = Config::get();
    device.idle_override = Some(config.idle_energy);
    device.channel_override = Some(config.require_channel);
    let front = properties.and_then(|p| p.get("facing")).map_or("north", String::as_str);
    let bit = ["east", "west", "up", "down", "south", "north"].iter().position(|f| *f == front).unwrap_or(5);
    device.faces = 0b11_1111 & !(1 << bit);
    true
}

/// `RequesterBlock.updateBlockStateFromBlockEntity`: `active` while a row
/// shows something else than idling.
pub(super) fn visual_props(machine: &Machine, props: &mut horizon_world::block_state_properties::BlockProperties) -> bool {
    if !is_requester(machine) || !props.contains_key("active") {
        return false;
    }
    props.insert("active".into(), rows::active(&machine.config).to_string());
    true
}

/// The grid seen from a requester: the CPU jobs it starts and the network it
/// fills.
struct GridHost<'a, 'g> {
    game: &'a mut Game<'g>,
    owner: RequesterOwner,
    pos: (i32, i32, i32),
    links: Vec<(usize, (Link, bool))>,
    submitted: u64,
}
impl core::Host<Key, Link> for GridHost<'_, '_> {
    fn begin_calculation(&mut self, slot: usize, key: &Key, amount: i64) {
        let amount = u64::try_from(amount).unwrap_or(0);
        if let Err(error) = self.game.ae2_requester_begin_calculation(self.owner, slot, &key.key, key.fluid, amount, self.pos) {
            self.game.ae2x.merequester.notes.insert(self.owner.host, format!("row {slot}: no calculation: {error}"));
        }
    }
    fn calculation_done(&self, slot: usize) -> bool {
        self.game.ae2_requester_calculation_done(self.owner, slot)
    }
    fn finish_calculation(&mut self, slot: usize) -> core::Planned<Link> {
        let planned = self.game.ae2_requester_finish_calculation(self.owner, slot, self.pos);
        if matches!(planned, core::Planned::Linked { .. }) {
            self.submitted += 1;
        }
        planned
    }
    fn link_state(&self, slot: usize, link: &Link) -> core::LinkState {
        if self.links.iter().any(|(s, (l, done))| *s == slot && l == link && *done) {
            return core::LinkState::Done;
        }
        let cpu = GridAddress::from_network(ae2_network_core::Pos { x: link[0], y: link[1], z: link[2] });
        if cpu.is_some_and(|cpu| self.game.ae2_requester_cpu_runs(self.owner, slot, cpu)) {
            core::LinkState::Running
        } else {
            core::LinkState::Cancelled
        }
    }
    fn insert(&mut self, key: &Key, amount: i64) -> i64 {
        let Ok(amount) = u64::try_from(amount) else { return 0 };
        let address = self.owner.host;
        let inserted = if key.fluid {
            self.game.ae2_grid_fluid_insert_encoded(address, &key.key, amount, false).unwrap_or(0)
        } else {
            let Ok(mut item) = ae2_lab::prototype(&key.key) else { return 0 };
            item.count = u32::try_from(amount).unwrap_or(u32::MAX);
            self.game.ae2_grid_insert_encoded(address, &item, u64::from(item.count)).unwrap_or(0)
        };
        i64::try_from(inserted).unwrap_or(i64::MAX)
    }
}

/// The ATM10 tick bounds: `TickingRequest(1, 20, false)`.
const TICK_MIN: u64 = 1;
const TICK_MAX: u64 = 20;

impl Game<'_> {
    fn merequester_machine(&self, address: GridAddress) -> Option<&Machine> {
        match self.ae2_block_entity(address)? {
            BlockEntity::Ae2(machine) if is_requester(machine) => Some(machine),
            _ => None,
        }
    }
    fn merequester_incarnation(&self, address: GridAddress) -> Option<u64> {
        let (dim, pos) = self.ae2_address_parts(address)?;
        self.ae2_world_for(dim)?.block_entities.incarnation(&pos)
    }
    /// The requester at `address`: its saved rows and buffers, the running
    /// jobs, and the statuses of this run.
    pub(in crate::game) fn merequester_load(&self, address: GridAddress) -> Option<Requester> {
        let machine = self.merequester_machine(address)?;
        let mut r = rows::read(&machine.config, Config::get().requests);
        let incarnation = self.merequester_incarnation(address)?;
        if let Some(runtime) = self.ae2x.merequester.runtime.get(&address).filter(|rt| rt.incarnation == incarnation) {
            r.serial = runtime.serial;
            for (state, kept) in r.states.iter_mut().zip(&runtime.states) {
                if !matches!(state, core::State::Link(_)) && !matches!(kept, core::State::Link(_)) {
                    *state = kept.clone();
                }
            }
        }
        Some(r)
    }
    /// Saves `r` (`saveChanges`) and keeps its statuses for the next tick.
    pub(in crate::game) fn merequester_store(&mut self, address: GridAddress, r: &Requester) -> bool {
        let Some(incarnation) = self.merequester_incarnation(address) else { return false };
        let Some(BlockEntity::Ae2(machine)) = self.ae2_block_entity_mut(address) else { return false };
        if !is_requester(machine) {
            return false;
        }
        let before = machine.config.clone();
        rows::write(&mut machine.config, r);
        if machine.config != before {
            self.ae2_mark_modified(address);
        }
        let runtime = self.ae2x.merequester.runtime.entry(address).or_insert(Runtime { incarnation, states: Vec::new(), serial: 1, jobs: 0 });
        if runtime.incarnation != incarnation {
            runtime.jobs = 0;
        }
        runtime.incarnation = incarnation;
        runtime.states = r.states.clone();
        runtime.serial = r.serial;
        true
    }
    /// The network's amount of each row's key (`IStackWatcher`).
    fn merequester_stock(&self, address: GridAddress, r: &Requester) -> Vec<Option<i64>> {
        let wants = |fluid: bool| r.requests.iter().any(|q| q.key.as_ref().is_some_and(|k| k.fluid == fluid));
        let mut stock = BTreeMap::new();
        if wants(false) {
            for (key, n) in self.ae2_grid_inventory_encoded(address).unwrap_or_default() {
                *stock.entry((key, false)).or_insert(0u64) += n;
            }
        }
        if wants(true) {
            for (key, n) in self.ae2_grid_fluids_encoded(address).unwrap_or_default() {
                *stock.entry((key, true)).or_insert(0u64) += n;
            }
        }
        r.requests
            .iter()
            .map(|q| {
                let key = q.key.as_ref()?;
                let n = stock.get(&(key.key.clone(), key.fluid)).copied().unwrap_or(0);
                Some(i64::try_from(n).unwrap_or(i64::MAX))
            })
            .collect()
    }
    /// The stack watcher's notifications: rows whose key's amount changed.
    fn merequester_watch(&self, address: GridAddress, r: &mut Requester) {
        let stock = self.merequester_stock(address, r);
        for (slot, amount) in stock.into_iter().enumerate() {
            let Some(amount) = amount else { continue };
            if r.storages[slot].known != amount {
                let key = r.requests[slot].key.clone().expect("a stocked row has a key");
                r.stock_changed(&key, amount);
            }
        }
    }

    /// `tickingRequest` of every requester in this level whose grid node is
    /// active and whose tick is due.
    pub(in crate::game) fn tick_merequester(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let Some(kind) = kind() else { return };
        let positions: Vec<_> = self
            .world
            .block_entities
            .iter()
            .filter_map(|(&p, e)| matches!(e, BlockEntity::Ae2(m) if m.kind == kind).then_some(p))
            .collect();
        if positions.is_empty() {
            return;
        }
        let network = self.ae2_network();
        let Ok(analysis) = network.analyze() else { return };
        let services = self.ae2_event_service_ids(&network);
        let now = self.ae2_server_tick();
        for pos in positions {
            let Some(address) = self.ae2_local_address(pos) else { continue };
            let node = address.network_pos();
            let Some(identity) = network.node_identities.get(&node).copied() else { continue };
            // `getMainNode().isActive()`: powered, with its channel.
            if !analysis.nodes.get(&node).is_some_and(|s| s.online) {
                continue;
            }
            let service = services.as_ref().and_then(|s| s.get(&identity).copied());
            if !self.ae2_device_scheduler.borrow_mut().due(identity, service, now, TICK_MIN, TICK_MAX, false, false) {
                continue;
            }
            let modulation = self.merequester_tick_one(address, pos);
            use super::super::ae2_tick_tracker::Modulation as M;
            let modulation = match modulation {
                core::Modulation::Urgent => M::Urgent,
                core::Modulation::Faster => M::Faster,
                core::Modulation::Same => M::Same,
                core::Modulation::Slower => M::Slower,
                core::Modulation::Idle => M::Idle,
                core::Modulation::Sleep => M::Sleep,
            };
            self.ae2_device_scheduler.borrow_mut().completed(identity, now, modulation);
        }
    }
    fn merequester_tick_one(&mut self, address: GridAddress, pos: (i32, i32, i32)) -> core::Modulation {
        let Some(mut r) = self.merequester_load(address) else { return core::Modulation::Idle };
        self.merequester_watch(address, &mut r);
        let links = self.merequester_machine(address).map(|m| rows::links(&m.config)).unwrap_or_default();
        let mut host = GridHost { game: self, owner: RequesterOwner { host: address }, pos, links, submitted: 0 };
        let tick = r.tick(&mut host);
        let submitted = host.submitted;
        // The watcher sees what the tick exported before the next one.
        self.merequester_watch(address, &mut r);
        self.merequester_store(address, &r);
        if let Some(runtime) = self.ae2x.merequester.runtime.get_mut(&address) {
            runtime.jobs += submitted;
        }
        tick.modulation
    }

    /// Edits the rows of the requester at `address` as its menu does, then
    /// reads the known amount of a changed key (`requestChanged`).
    pub(in crate::game) fn merequester_edit(&mut self, address: GridAddress, edit: impl FnOnce(&mut Requester) -> Vec<(usize, core::Change)>) -> bool {
        let Some(mut r) = self.merequester_load(address) else { return false };
        let changes = edit(&mut r);
        if changes.iter().any(|(_, c)| *c == core::Change::KeyChanged) {
            let stock = self.merequester_stock(address, &r);
            for (slot, change) in &changes {
                if *change == core::Change::KeyChanged {
                    r.key_changed(*slot, stock.get(*slot).copied().flatten());
                }
            }
        }
        self.merequester_store(address, &r)
    }

    // The CPU side of a row's job (`ICraftingRequester`).

    /// Whether row `slot` still waits for `cpu`'s job.
    pub(in crate::game) fn ae2_requester_link_valid(&self, owner: RequesterOwner, slot: usize, cpu: GridAddress) -> bool {
        let (x, y, z) = cpu.encoded();
        self.merequester_machine(owner.host)
            .and_then(|m| rows::link(&m.config, slot))
            .is_some_and(|(link, done)| link == [x, y, z] && !done)
    }
    /// The CPU is done with row `slot`'s job (`ICraftingLink.isDone`).
    pub(in crate::game) fn ae2_requester_link_done(&mut self, owner: RequesterOwner, slot: usize, cpu: GridAddress) {
        let (x, y, z) = cpu.encoded();
        let Some(BlockEntity::Ae2(machine)) = self.ae2_block_entity_mut(owner.host) else { return };
        if is_requester(machine) && rows::set_link_done(&mut machine.config, slot, [x, y, z]) {
            self.ae2_mark_modified(owner.host);
        }
    }
    /// `insertCraftedItems`: the job's results go to the row's buffer, all of
    /// them; none for a row that does not wait for `cpu`.
    pub(in crate::game) fn ae2_requester_deliver(&mut self, owner: RequesterOwner, slot: usize, cpu: GridAddress, key: &ae2_cell_core::ItemKey, fluid: bool, amount: u64) -> u64 {
        if !self.ae2_requester_link_valid(owner, slot, cpu) {
            return 0;
        }
        let Some(mut r) = self.merequester_load(owner.host) else { return 0 };
        let Ok(n) = i64::try_from(amount) else { return 0 };
        let Some(storage) = r.storages.get_mut(slot) else { return 0 };
        if !storage.update(Key { key: key.clone(), fluid }, n) {
            return 0;
        }
        if self.merequester_store(owner.host, &r) { amount } else { 0 }
    }
    /// A pending calculation of row `slot` stays valid while the row asks
    /// for the same key and has no job.
    pub(in crate::game) fn ae2_requester_pending_generation(&self, owner: RequesterOwner, slot: usize, key: &ae2_cell_core::ItemKey, fluid: bool) -> Option<i64> {
        let machine = self.merequester_machine(owner.host)?;
        if rows::link(&machine.config, slot).is_some() {
            return None;
        }
        let r = rows::read(&machine.config, Config::get().requests);
        let row = r.requests.get(slot)?.key.as_ref()?;
        (row.key == *key && row.fluid == fluid).then_some(0)
    }
    /// A Memory Card used on a requester (`AEBaseEntityBlock.useItemOn`,
    /// `exportSettings`, `importSettings`): with sneak it copies the rows, else
    /// it pastes a requester's. True when the card and a requester are there.
    pub(in crate::game) fn ae2_try_requester_memory(&mut self, pos: [i32; 3]) -> bool {
        use horizon_client::text::Text;
        const SOURCE: &str = "ae2:exported_settings_source";
        let Some(mut card) = self.held().filter(|s| s.item.as_ref() == "ae2:memory_card" && s.count == 1).cloned() else { return false };
        let key = (pos[0], pos[1], pos[2]);
        if !matches!(self.world.block_entities.get(&key), Some(BlockEntity::Ae2(m)) if is_requester(m)) {
            return false;
        }
        let Some(address) = self.ae2_local_address(key) else { return false };
        let source = Tag::compound([("translate", Tag::String("block.merequester.requester".into()))]);
        if self.keys.contains(&crate::game::KeyCode::ShiftLeft) {
            let Some(r) = self.merequester_load(address) else { return true };
            if crate::game::ae2_memory::clear(&mut card).is_err() {
                return true;
            }
            card.data_mut().set_other(rows::EXPORTED_REQUESTS, Some(rows::export_component(&r)));
            card.data_mut().set_other(SOURCE, Some(source));
            horizon_client::ae2_stack_identity::synchronize_tracked(&card);
            *self.held_mut() = Some(card);
            self.system_message(Text::literal("Settings saved"));
            return true;
        }
        let saved = card.data().and_then(|d| d.other(rows::EXPORTED_REQUESTS).filter(|_| d.other(SOURCE) == Some(&source))).cloned();
        let Some(saved) = saved else {
            self.system_message(Text::literal("Invalid machine"));
            return true;
        };
        self.merequester_edit(address, |r| {
            rows::import_component(r, &saved);
            (0..r.size()).map(|slot| (slot, core::Change::KeyChanged)).collect()
        });
        self.system_message(Text::literal("Settings loaded"));
        true
    }
    /// Jobs the requester at `address` submitted in this run.
    pub(in crate::game) fn merequester_jobs(&self, address: GridAddress) -> u64 {
        self.ae2x.merequester.runtime.get(&address).map_or(0, |rt| rt.jobs)
    }
}
