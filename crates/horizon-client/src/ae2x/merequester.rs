//! ME Requester 1.21.1-1.5.0's rows in the definitions registry and its
//! saved data (feature `ae2-merequester`). Derived from ME Requester by
//! AlmostReliable (`Registration`, `Config`, `RequesterBlockEntity`,
//! `RequestManager`, `Request`, `StorageManager`),
//! https://github.com/AlmostReliable/merequester tag `v1.21.1-neoforge-1.5.0`,
//! LGPL-3.0; see `ae2/addons/merequester/`. The request logic is in
//! `ae2-merequester-core`, the game's side in `game/ae2x/merequester/`.
use super::defs::{Defs, PartDef};
use crate::{ae2_generic_stack::{self, GenericStack}, ae2_world::{ExtMachine, Machine}, items::ItemStack};
use ae2_cell_core::ItemKey;
use horizon_world::nbt::{Compound, Tag};
use std::sync::OnceLock;

pub use ae2_merequester_core as core;

pub const NAMESPACE: &str = "merequester";
/// `MERequester.REQUESTER_ID`: the block, its item and its menu.
pub const REQUESTER: &str = "merequester:requester";
/// `MERequester.TERMINAL_ID`: the part item.
pub const TERMINAL: &str = "merequester:requester_terminal";
/// `Registration.EXPORTED_REQUESTS`: what a Memory Card copies.
pub const EXPORTED_REQUESTS: &str = "merequester:exported_requests";

/// `merequester-common.toml`, `[requester]` (`Config.CommonConfig`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    /// `requests`: rows of one requester, 1 to 64.
    pub requests: usize,
    /// `idle_energy`: AE/t drawn while idle.
    pub idle_energy: f64,
    /// `require_channel`.
    pub require_channel: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self { requests: 5, idle_energy: 5.0, require_channel: true }
    }
}
impl Config {
    /// A value of the wrong type or outside its range falls back to its
    /// default, as `ModConfigSpec` corrects it.
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut config = Self::default();
        let Some(section) = value.get("requester") else {
            return Ok(config);
        };
        if let Some(n) = section.get("requests").and_then(toml::Value::as_integer).filter(|n| (1..=64).contains(n)) {
            config.requests = n as usize;
        }
        let energy = section.get("idle_energy").and_then(|v| v.as_float().or_else(|| v.as_integer().map(|n| n as f64)));
        if let Some(e) = energy.filter(|e| e.is_finite() && *e >= 0.0) {
            config.idle_energy = e;
        }
        if let Some(b) = section.get("require_channel").and_then(toml::Value::as_bool) {
            config.require_channel = b;
        }
        Ok(config)
    }
    /// The game's `config/merequester-common.toml`, read once: like the
    /// mod's, a requester keeps the row count it was made with.
    pub fn get() -> Self {
        static CONFIG: OnceLock<Config> = OnceLock::new();
        *CONFIG.get_or_init(|| {
            let path = crate::logging::game_home().join("config/merequester-common.toml");
            match std::fs::read_to_string(&path) {
                Ok(text) => Self::parse(&text).unwrap_or_else(|error| {
                    log::warn!("invalid ME Requester config {}: {error}", path.display());
                    Self::default()
                }),
                Err(_) => Self::default(),
            }
        })
    }
}

/// `AEBaseBlock.metalProps` and the Pattern Access Terminal's part: the
/// requester block and the Requester Terminal.
pub fn register(defs: &mut Defs) {
    defs.add_machine(ExtMachine {
        id: REQUESTER,
        physical_slots: 0,
        upgrade_slots: 0,
        key_drops: Some(key_drops),
        power_capacity: None,
    });
    // AbstractDisplayPart: 0.5 AE/t, and it needs a channel.
    defs.add_part(PartDef { id: TERMINAL.into(), idle_power: 0.5, requires_channel: true });
}

/// A row's key: an item or a fluid (`AEKey`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Key {
    pub key: ItemKey,
    pub fluid: bool,
}
impl Key {
    /// `AEKey.getAmountPerUnit`: a bucket for fluids.
    pub fn amount_per_unit(&self) -> i64 {
        if self.fluid { 1000 } else { 1 }
    }
}
/// A job's crafting link: the CPU's grid address (`GridAddress.encoded`).
pub type Link = [i32; 3];
pub type Requester = core::Requester<Key, Link>;

const REQUESTS_ID: &str = "requests";
const REQUEST_STATUS_ID: &str = "request_status";
const STORAGE_MANAGER_ID: &str = "storage_manager";

/// `AEKey.toTagGeneric`: the generic stack's compound without its amount.
pub fn key_tag(key: &Key) -> Option<Tag> {
    let tag = ae2_generic_stack::encode(&GenericStack { key: key.key.clone(), fluid: key.fluid, amount: 0 }).ok()?;
    let Tag::Compound(mut data) = tag else { return None };
    data.remove(&"#".into());
    Some(Tag::Compound(data))
}
/// `AEKey.fromTagGeneric`.
pub fn key_from_tag(tag: &Tag) -> Option<Key> {
    let Tag::Compound(data) = tag else { return None };
    let mut data = data.clone();
    data.insert("#".into(), Tag::Long(0));
    let stack = ae2_generic_stack::decode(&Tag::Compound(data)).ok()??;
    Some(Key { key: stack.key, fluid: stack.fluid })
}
fn long(tag: Option<&Tag>) -> i64 {
    crate::save::number(tag).unwrap_or(0)
}

/// The requester saved in a machine's config (`loadTag`): its rows, their
/// buffers and the running jobs' links. Other statuses are not saved; a
/// loaded row without a job is idle.
pub fn read(config: &Compound, size: usize) -> Requester {
    let mut r = Requester::new(size);
    if let Some(Tag::Compound(requests)) = config.get(&REQUESTS_ID.into()) {
        for (i, request) in r.requests.iter_mut().enumerate() {
            let Some(tag) = requests.get(&i.to_string().as_str().into()) else { continue };
            request.state = matches!(tag.get("state"), Some(Tag::Byte(b)) if *b != 0);
            request.key = tag.get("key").and_then(key_from_tag);
            request.amount = long(tag.get("amount"));
            request.batch = long(tag.get("batch"));
            request.client_status = core::Status::from_ordinal(long(tag.get("status")) as i32);
        }
    }
    if let Some(Tag::Compound(storages)) = config.get(&STORAGE_MANAGER_ID.into()) {
        for (i, storage) in r.storages.iter_mut().enumerate() {
            let Some(tag) = storages.get(&i.to_string().as_str().into()) else { continue };
            storage.key = tag.get("key").and_then(key_from_tag);
            storage.buffer = long(tag.get("buffer_amount"));
            storage.pending = long(tag.get("pending_amount"));
            storage.known = long(tag.get("known_amount"));
        }
    }
    for (slot, link) in links(config) {
        if let Some(state) = r.states.get_mut(slot) {
            *state = core::State::Link(link.0);
        }
    }
    r
}
/// `RequesterBlockEntity.isActive` from the saved client statuses: a row
/// shows something else than idling. The block's `active` property.
pub fn active(config: &Compound) -> bool {
    let Some(Tag::Compound(requests)) = config.get(&REQUESTS_ID.into()) else { return false };
    requests.values().any(|tag| core::Status::from_ordinal(long(tag.get("status")) as i32) != core::Status::Idle)
}
/// The saved links (`deserializeStatus`): each slot's CPU and whether the
/// CPU has finished the job and let go of it.
pub fn links(config: &Compound) -> Vec<(usize, (Link, bool))> {
    let Some(Tag::Compound(status)) = config.get(&REQUEST_STATUS_ID.into()) else { return Vec::new() };
    status
        .iter()
        .filter_map(|(slot, tag)| {
            let slot = slot.to_string().ok()?.parse::<usize>().ok()?;
            let Some(Tag::IntArray(cpu)) = tag.get("cpu") else { return None };
            let cpu: Link = cpu.as_slice().try_into().ok()?;
            Some((slot, (cpu, matches!(tag.get("done"), Some(Tag::Byte(1))))))
        })
        .collect()
}
/// The link of `slot`, if a job runs or ran for it.
pub fn link(config: &Compound, slot: usize) -> Option<(Link, bool)> {
    links(config).into_iter().find(|(s, _)| *s == slot).map(|(_, l)| l)
}
/// The CPU of `slot`'s job finished with it: the row moves on to exporting.
pub fn set_link_done(config: &mut Compound, slot: usize, cpu: Link) -> bool {
    let Some(Tag::Compound(status)) = config.get_mut(&REQUEST_STATUS_ID.into()) else { return false };
    let Some(Tag::Compound(entry)) = status.get_mut(&slot.to_string().as_str().into()) else { return false };
    if entry.get(&"cpu".into()) != Some(&Tag::IntArray(cpu.to_vec())) {
        return false;
    }
    entry.insert("done".into(), Tag::Byte(1));
    true
}
fn request_tag(request: &core::Request<Key>) -> Tag {
    let mut tag = Compound::new();
    tag.insert("state".into(), Tag::Byte(i8::from(request.state)));
    if let Some(key) = request.key.as_ref().and_then(key_tag) {
        tag.insert("key".into(), key);
    }
    tag.insert("amount".into(), Tag::Long(request.amount));
    tag.insert("batch".into(), Tag::Long(request.batch));
    tag.insert("status".into(), Tag::Int(request.client_status.ordinal()));
    Tag::Compound(tag)
}
/// `saveAdditional`: the rows, the jobs' links (keeping the finished mark
/// of a link that stays) and the buffers.
pub fn write(config: &mut Compound, r: &Requester) {
    let mut requests = Compound::new();
    for (i, request) in r.requests.iter().enumerate() {
        requests.insert(i.to_string().as_str().into(), request_tag(request));
    }
    config.insert(REQUESTS_ID.into(), Tag::Compound(requests));
    let old = links(config);
    let mut status = Compound::new();
    for (slot, cpu) in r.links() {
        let done = old.iter().any(|(s, (c, d))| *s == slot && c == cpu && *d);
        let mut entry = Compound::new();
        entry.insert("cpu".into(), Tag::IntArray(cpu.to_vec()));
        if done {
            entry.insert("done".into(), Tag::Byte(1));
        }
        status.insert(slot.to_string().as_str().into(), Tag::Compound(entry));
    }
    config.insert(REQUEST_STATUS_ID.into(), Tag::Compound(status));
    let mut storages = Compound::new();
    for (i, storage) in r.storages.iter().enumerate() {
        let mut tag = Compound::new();
        if let Some(key) = storage.key.as_ref().and_then(key_tag) {
            tag.insert("key".into(), key);
        }
        tag.insert("buffer_amount".into(), Tag::Long(storage.buffer));
        tag.insert("pending_amount".into(), Tag::Long(storage.pending));
        tag.insert("known_amount".into(), Tag::Long(storage.known));
        storages.insert(i.to_string().as_str().into(), Tag::Compound(tag));
    }
    config.insert(STORAGE_MANAGER_ID.into(), Tag::Compound(storages));
}

/// The Memory Card's copy (`RequestManager.toComponent`, the codec of
/// `Request.Component`): every row, its key without amount.
pub fn export_component(r: &Requester) -> Tag {
    Tag::List(
        r.requests
            .iter()
            .map(|request| {
                let mut tag = Compound::new();
                tag.insert("state".into(), Tag::Byte(i8::from(request.state)));
                if let Some(key) = request.key.as_ref().and_then(key_tag) {
                    tag.insert("key".into(), key);
                }
                tag.insert("amount".into(), Tag::Long(request.amount));
                tag.insert("batch".into(), Tag::Long(request.batch));
                tag.insert("client_status".into(), Tag::String(request.client_status.name().into()));
                Tag::Compound(tag)
            })
            .collect(),
    )
}
/// `RequestManager.fromComponent`: rows past the copy's are cleared. False
/// for a component that is not a list of rows.
pub fn import_component(r: &mut Requester, tag: &Tag) -> bool {
    let Tag::List(rows) = tag else { return false };
    for (i, request) in r.requests.iter_mut().enumerate() {
        match rows.get(i) {
            Some(row) => {
                request.state = matches!(row.get("state"), Some(Tag::Byte(b)) if *b != 0);
                request.key = row.get("key").and_then(key_from_tag);
                request.amount = long(row.get("amount"));
                request.batch = long(row.get("batch"));
                request.client_status = match row.get("client_status") {
                    Some(Tag::String(s)) => s.to_string().ok().and_then(|s| core::Status::from_name(&s)).unwrap_or(core::Status::Idle),
                    _ => core::Status::Idle,
                };
            }
            None => {
                request.update_key(None, 1);
            }
        }
    }
    true
}

/// `StorageManager.addDrops`: each buffer's key, its buffered and pending
/// amount. A fluid drops nothing.
fn key_drops(machine: &Machine) -> Vec<(ItemStack, u64)> {
    let r = read(&machine.config, Config::get().requests);
    r.storages
        .iter()
        .filter_map(|s| {
            let key = s.key.as_ref().filter(|k| !k.fluid)?;
            let amount = u64::try_from(s.buffer_amount() + s.pending).ok().filter(|n| *n > 0)?;
            Some((crate::ae2_cell_codec::prototype(&key.key).ok()?, amount))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str) -> Key {
        Key { key: crate::ae2_cell_codec::key(&ItemStack::new(id, 1)).unwrap(), fluid: false }
    }

    #[test]
    fn config_reads_atm10s_file_and_corrects_bad_values() {
        let atm10 = "[requester]\n\t#The amount of requests a single ME Requester can hold.\n\trequests = 5\n\tidle_energy = 5.0\n\trequire_channel = true\n";
        assert_eq!(Config::parse(atm10).unwrap(), Config::default());
        let odd = Config::parse("[requester]\nrequests = 65\nidle_energy = 2\nrequire_channel = false\n").unwrap();
        assert_eq!(odd, Config { requests: 5, idle_energy: 2.0, require_channel: false });
        assert_eq!(Config::parse("").unwrap(), Config::default());
    }

    #[test]
    fn saved_requests_round_trip_in_the_mods_tags() {
        let mut r = Requester::new(5);
        r.requests[0].update_key(Some((item("minecraft:stick"), 100)), 1);
        r.requests[0].update_batch(64);
        r.requests[1].update_state(false);
        let water = Key { key: ItemKey { item_id: "minecraft:water".into(), components: Vec::new() }, fluid: true };
        r.requests[2].update_key(Some((water.clone(), 5000)), water.amount_per_unit());
        r.storages[0].update(item("minecraft:stick"), 12);
        r.storages[0].pending = 3;
        r.storages[0].known = 40;
        r.states[0] = core::State::Link([1, 2, 3]);
        r.requests[0].client_status = core::Status::Link;
        let mut config = Compound::new();
        write(&mut config, &r);
        let request = config.get(&"requests".into()).unwrap().get("0").unwrap();
        assert_eq!(request.get("key").unwrap().get("#t"), Some(&Tag::String("ae2:i".into())));
        assert_eq!(request.get("key").unwrap().get("id"), Some(&Tag::String("minecraft:stick".into())));
        assert_eq!(request.get("amount"), Some(&Tag::Long(100)));
        assert_eq!(request.get("batch"), Some(&Tag::Long(64)));
        assert_eq!(request.get("status"), Some(&Tag::Int(5)));
        let back = read(&config, 5);
        assert_eq!(back.requests, r.requests);
        assert_eq!(back.storages[0].buffer_amount(), 12);
        assert_eq!((back.storages[0].pending, back.storages[0].known), (3, 40));
        assert_eq!(back.states[0], core::State::Link([1, 2, 3]));
        assert_eq!(back.requests[2].batch, 1000);
        // The CPU finishes: the mark stays with the link.
        assert!(set_link_done(&mut config, 0, [1, 2, 3]));
        assert!(!set_link_done(&mut config, 0, [9, 9, 9]));
        write(&mut config, &back);
        assert_eq!(link(&config, 0), Some(([1, 2, 3], true)));
        // A requester read with fewer rows keeps the first ones.
        assert_eq!(read(&config, 2).requests.len(), 2);
        // The exported component.
        let exported = export_component(&back);
        let mut copy = Requester::new(5);
        copy.requests[4].update_key(Some((item("minecraft:dirt"), 1)), 1);
        assert!(import_component(&mut copy, &exported));
        assert_eq!(copy.requests, back.requests);
        let Tag::List(rows) = exported else { unreachable!() };
        assert_eq!(rows[0].get("client_status"), Some(&Tag::String("link".into())));
    }

    #[test]
    fn buffers_drop_as_items() {
        let mut m = Machine::new(crate::ae2_world::MachineKind::Ext(0));
        let mut r = Requester::new(5);
        r.storages[1].update(item("minecraft:stick"), 70);
        r.storages[1].pending = 2;
        write(&mut m.config, &r);
        let drops = key_drops(&m);
        assert_eq!(drops.len(), 1);
        assert_eq!((drops[0].0.item.as_ref(), drops[0].1), ("minecraft:stick", 72));
    }
}
