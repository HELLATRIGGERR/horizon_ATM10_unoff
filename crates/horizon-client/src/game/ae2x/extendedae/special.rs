//! ExtendedAE's special buses, emitters and planes (slice 5): AE2's storage
//! bus, export bus, level emitter and planes (part variants, `base_id`) with
//! the mod's own filters and amounts. The rules are the library's
//! `ae2x::extendedae::special`; here is what reads the world.
//!
//! A part keeps the mod's fields under the mod's names: `oreExp` and
//! `oreExp2` (a tag bus's white and black expressions), `modid` (a mod
//! bus's list), `cmod` (the threshold export bus's mode), `upperValue`,
//! `lowerValue`, `currentState` (the threshold emitter). The amounts of the
//! precise and threshold buses' config slots, which AE2's bus state keeps as
//! keys, are `config2`, a list of longs by slot (1 where unset).
use crate::game::Game;
use ae2_machines_core::Key;
use horizon_client::ae2x::extendedae::special::{self as rows, ModFilter, TagFilter};
use horizon_world::nbt::{Compound, Tag};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub(in crate::game) fn text(part: &Compound, name: &str) -> String {
    match part.get(&name.into()) {
        Some(Tag::String(s)) => s.to_string_lossy(),
        _ => String::new(),
    }
}
pub(in crate::game) fn part_id(part: &Compound) -> String {
    text(part, "id")
}
/// The amounts of a precise or threshold bus's 63 config slots (`config2`).
pub(in crate::game) fn amounts(part: &Compound) -> Vec<u64> {
    let mut out = vec![1u64; 63];
    if let Some(Tag::List(list)) = part.get(&rows::AMOUNTS.into()) {
        for (slot, tag) in list.iter().enumerate().take(63) {
            if let Tag::Long(n) = tag {
                out[slot] = (*n).max(0) as u64;
            }
        }
    }
    out
}
pub(in crate::game) fn store_amounts(part: &mut Compound, amounts: &[u64]) {
    part.insert(rows::AMOUNTS.into(), Tag::List(amounts.iter().map(|n| Tag::Long((*n).min(i64::MAX as u64) as i64)).collect()));
}
/// `Platform.getModName` for the ids the port names (the terminal's search
/// knows the same two); none for the rest.
pub(in crate::game) fn mod_name(namespace: &str) -> Option<&'static str> {
    match namespace {
        "minecraft" => Some("Minecraft"),
        "ae2" => Some("Applied Energistics 2"),
        _ => None,
    }
}
fn addon_key(key: &Key) -> bool {
    horizon_client::ae2_key_types::parse_addon_marker(&ae2_cell_core::ItemKey::from(key)).is_some()
}

thread_local! {
    /// The ids each tag pattern covers through tags, by the resources they
    /// were read from (`TagExpParserV2`'s cache).
    static TAG_SETS: RefCell<(usize, HashMap<(String, bool), Rc<HashSet<String>>>)> = RefCell::new((0, HashMap::new()));
}

/// A compiled tag filter with the ids each of its patterns covers: what a
/// key's names are checked against (`TagExpParserV2.evaluate`: an item's
/// tags, its block's tags and its id; a fluid's tags and id).
#[derive(Clone)]
pub(in crate::game) struct TagLookup {
    filter: TagFilter,
    sets: HashMap<String, Rc<HashSet<String>>>,
}
impl TagLookup {
    pub(in crate::game) fn listed(&self, key: &Key) -> bool {
        self.filter.listed(!addon_key(key), &|pattern| {
            rows::tag_pattern_matches(pattern, &key.id) || self.sets.get(pattern).is_some_and(|ids| ids.contains(&key.id))
        })
    }
}

impl Game<'_> {
    fn extendedae_tag_set(&self, pattern: &str, fluid: bool) -> Rc<HashSet<String>> {
        let items = self.res.items();
        let generation = std::ptr::from_ref(items) as usize ^ items.item_tags.len() ^ (items.block_tags.len() << 20);
        TAG_SETS.with(|cache| {
            let mut cache = cache.borrow_mut();
            if cache.0 != generation {
                *cache = (generation, HashMap::new());
            }
            if let Some(found) = cache.1.get(&(pattern.to_owned(), fluid)) {
                return found.clone();
            }
            let mut ids = HashSet::new();
            if fluid {
                for (tag, members) in &self.res.recipes().fluid_tags {
                    if rows::tag_pattern_matches(pattern, tag) {
                        ids.extend(members.iter().cloned());
                    }
                }
            } else {
                for (tag, members) in &items.item_tags {
                    if rows::tag_pattern_matches(pattern, tag) {
                        ids.extend(members.iter().cloned());
                    }
                }
                // A BlockItem's block's tags count too.
                let blocks: HashSet<&String> = items.block_tags.iter().filter(|(tag, _)| rows::tag_pattern_matches(pattern, tag)).flat_map(|(_, m)| m).collect();
                if !blocks.is_empty() {
                    for (id, info) in &items.items {
                        if info.block.is_some_and(|state| blocks.contains(&self.res.facts().get(state).name.to_string())) {
                            ids.insert(id.to_string());
                        }
                    }
                }
            }
            let ids = Rc::new(ids);
            cache.1.insert((pattern.to_owned(), fluid), ids.clone());
            ids
        })
    }
    /// A tag bus's filter, its patterns looked up on the item or fluid channel.
    pub(in crate::game) fn extendedae_tag_lookup(&self, part: &Compound, fluid: bool) -> TagLookup {
        let filter = TagFilter::new(&text(part, rows::TAG_WHITE), &text(part, rows::TAG_BLACK));
        let sets = filter.patterns().into_iter().map(|p| (p.to_owned(), self.extendedae_tag_set(p, fluid))).collect();
        TagLookup { filter, sets }
    }

    /// `PartSpecialStorageBus.createFilter` of the tag, mod and precise
    /// storage buses (`TagPriorityList`, `ModPriorityList`, `PreciseFilter`);
    /// the precise one's `PreciseInventory.insert` limit beside it.
    pub(in crate::game) fn extendedae_mount_filter(&self, part: &Compound, fluid: bool) -> Option<crate::game::ae2x::MountFilter> {
        let id = part_id(part);
        match id.as_str() {
            rows::TAG_STORAGE_BUS => {
                let lookup = self.extendedae_tag_lookup(part, fluid);
                let describe = format!("tag:{}|{}", text(part, rows::TAG_WHITE), text(part, rows::TAG_BLACK));
                Some(crate::game::ae2x::MountFilter {
                    empty: false,
                    listed: Rc::new(move |key| lookup.listed(key)),
                    cap: None,
                    describe,
                })
            }
            rows::MOD_STORAGE_BUS => {
                let ids = text(part, rows::MOD_IDS);
                let filter = ModFilter::new(&ids);
                Some(crate::game::ae2x::MountFilter {
                    empty: filter.is_empty(),
                    listed: Rc::new(move |key| {
                        let namespace = key_namespace(key);
                        filter.listed(namespace, mod_name(namespace))
                    }),
                    cap: None,
                    describe: format!("mod:{ids}"),
                })
            }
            rows::PRECISE_STORAGE_BUS => {
                let state = crate::game::ae2_buses::bus_state(part).ok()?;
                let amounts = amounts(part);
                // A KeyCounter of the config: a key twice counts twice.
                let mut wanted: Vec<(Key, u64)> = Vec::new();
                for (slot, key) in state.config.iter().enumerate() {
                    if let Some(key) = key.as_ref().filter(|_| amounts[slot] > 0) {
                        match wanted.iter_mut().find(|(k, _)| k == key) {
                            Some((_, n)) => *n += amounts[slot],
                            None => wanted.push((key.clone(), amounts[slot])),
                        }
                    }
                }
                let empty = wanted.is_empty();
                let describe = format!("precise:{:?}", amounts);
                let wanted = Rc::new(wanted);
                let listed = wanted.clone();
                Some(crate::game::ae2x::MountFilter {
                    empty,
                    listed: Rc::new(move |key| listed.iter().any(|(k, _)| k == key)),
                    cap: Some(Rc::new(move |key| if empty { 1 } else { wanted.iter().find(|(k, _)| k == key).map_or(0, |(_, n)| *n) })),
                    describe,
                })
            }
            _ => None,
        }
    }
}

/// `AEKey.getModId` of an item, fluid or addon key.
pub(in crate::game) fn key_namespace(key: &Key) -> &str {
    let id = key.id.strip_prefix("horizon:key/").and_then(|rest| rest.split_once('|')).map_or(key.id.as_str(), |(_, id)| id);
    id.split_once(':').map_or("minecraft", |(ns, _)| ns)
}

/// The export loops of the tag, mod, precise and threshold export buses.
enum Export {
    /// `PartSpecialExportBus.doBusWork`: every grid key the list has, items
    /// and fluids by their own tags.
    Tag { items: TagLookup, fluids: TagLookup },
    Mod(ModFilter),
    /// `PartPreciseExportBus.doBusWork`: each slot's amount in whole batches.
    Precise { amounts: Vec<u64>, pulse: bool, crafting: bool },
    /// `PartThresholdExportBus.doBusWork`: a slot only while the grid holds
    /// more than (or at most) its amount, and then only what is above it.
    Threshold { mode: rows::ThresholdMode, amounts: Vec<u64> },
}
use ae2_machines_core::{BusState, Schedule, TickResult};
use crate::game::ae2_buses::ExportHost;

impl crate::game::ae2x::ExportLoop for Export {
    fn run(&mut self, state: &mut BusState, active: bool, loaded: bool, host: &mut dyn ExportHost) -> Result<TickResult, String> {
        if state.sleeping() {
            return Ok(TickResult::Sleep);
        }
        if !active || !loaded {
            return Ok(TickResult::Idle);
        }
        state.pending_pulse = false;
        let initial = state.operations();
        let mut ops = initial;
        let factor = |host: &dyn ExportHost, key: &Key| host.key_type(key).amount_per_operation().max(1);
        match self {
            Export::Tag { .. } | Export::Mod(_) => {
                for (key, _) in host.grid_keys() {
                    let listed = match self {
                        Export::Tag { items, fluids } => if host.key_type(&key) == ae2_cell_core::KeyType::FLUIDS { fluids.listed(&key) } else { items.listed(&key) },
                        Export::Mod(filter) => filter.listed(key_namespace(&key), mod_name(key_namespace(&key))),
                        _ => false,
                    };
                    if !listed {
                        continue;
                    }
                    let f = factor(host, &key);
                    let moved = host.export(&key, ops.saturating_mul(f))?;
                    if moved == 0 {
                        continue;
                    }
                    ops = ops.saturating_sub((moved / f).max(1));
                    if ops == 0 {
                        break;
                    }
                }
            }
            Export::Precise { amounts, pulse, crafting } => {
                let slots = state.slots();
                let mut visited = 0;
                // The grid's amounts once a tick; the simulated push below sees what is really there.
                let grid = host.grid_keys();
                while visited < slots && ops > 0 {
                    let index = slot_index(state, host, visited, slots);
                    visited += 1;
                    let Some(key) = state.config[index].clone() else { continue };
                    let amount = amounts[index];
                    let f = factor(host, &key);
                    if host.craft_only() {
                        attempt_craft(host, index, &key, amount, &mut ops, f)?;
                        continue;
                    }
                    let before = ops;
                    let stored = grid.iter().find(|(k, _)| *k == key).map_or(0, |(_, n)| *n);
                    let mut simulated = Ok(0);
                    let batch = rows::precise_batch(amount, ops, f, *pulse, &mut |n| n.min(stored), &mut |n| {
                        simulated = host.export_simulated(&key, n);
                        *simulated.as_ref().unwrap_or(&0)
                    });
                    simulated?;
                    let Some(batch) = batch else { break };
                    if batch > 0 && host.export(&key, batch)? > 0 {
                        ops = ops.saturating_sub((batch / f).max(1));
                    }
                    if before == ops && *crafting {
                        attempt_craft(host, index, &key, amount, &mut ops, f)?;
                    }
                }
                advance(state, ops, initial, visited, slots);
            }
            Export::Threshold { mode, amounts } => {
                let slots = state.slots();
                let mut visited = 0;
                let grid = host.grid_keys();
                while visited < slots && ops > 0 {
                    let index = slot_index(state, host, visited, slots);
                    visited += 1;
                    let Some(key) = state.config[index].clone() else { continue };
                    let stored = grid.iter().find(|(k, _)| *k == key).map_or(0, |(_, n)| *n);
                    if !mode.passes(stored, amounts[index]) {
                        continue;
                    }
                    let f = factor(host, &key);
                    let limit = ops.saturating_mul(f).min(mode.max_output(stored, amounts[index]));
                    let moved = host.export(&key, limit)?;
                    if moved > 0 {
                        ops = ops.saturating_sub((moved / f).max(1));
                    }
                }
                advance(state, ops, initial, visited, slots);
            }
        }
        Ok(if state.sleeping() {
            TickResult::Sleep
        } else if ops < initial {
            TickResult::Faster
        } else {
            TickResult::Slower
        })
    }
}
/// `getStartingSlot`.
fn slot_index(state: &BusState, host: &mut dyn ExportHost, visited: usize, slots: usize) -> usize {
    match state.schedule {
        Schedule::Default => visited,
        Schedule::RoundRobin => (state.next_slot + visited) % slots,
        Schedule::Random => host.random_slot(slots) % slots,
    }
}
/// `updateSchedulingMode` once the bus did work.
fn advance(state: &mut BusState, ops: u64, initial: u64, visited: usize, slots: usize) {
    if ops < initial && state.schedule == Schedule::RoundRobin && slots > 0 {
        state.next_slot = (state.next_slot + visited) % slots;
    }
}
/// `attemptCrafting`: a batch requested when the target takes it whole.
fn attempt_craft(host: &mut dyn ExportHost, slot: usize, key: &Key, amount: u64, ops: &mut u64, factor: u64) -> Result<(), String> {
    let requested = host.request_craft(slot, key, amount)?;
    if requested == amount && amount > 0 {
        *ops = ops.saturating_sub((amount / factor).max(1));
    }
    Ok(())
}

impl Game<'_> {
    pub(in crate::game) fn extendedae_export_loop(&self, part: &Compound) -> Option<Box<dyn crate::game::ae2x::ExportLoop>> {
        let cards = |card: &str| matches!(part.get(&"upgrades".into()), Some(Tag::List(list)) if list.iter().any(|c| c.get("id") == Some(&Tag::String(card.into()))));
        Some(Box::new(match part_id(part).as_str() {
            rows::TAG_EXPORT_BUS => Export::Tag { items: self.extendedae_tag_lookup(part, false), fluids: self.extendedae_tag_lookup(part, true) },
            rows::MOD_EXPORT_BUS => Export::Mod(ModFilter::new(&text(part, rows::MOD_IDS))),
            rows::PRECISE_EXPORT_BUS => {
                let state = crate::game::ae2_buses::bus_state(part).ok()?;
                // checkPulseRS: a redstone card set to pulse.
                Export::Precise { amounts: amounts(part), pulse: state.redstone_card && state.redstone == ae2_machines_core::Redstone::Pulse, crafting: cards("ae2:crafting_card") }
            }
            rows::THRESHOLD_EXPORT_BUS => Export::Threshold {
                mode: match part.get(&rows::THRESHOLD_MODE.into()) { Some(Tag::Byte(n)) => rows::ThresholdMode::from_ordinal(*n), _ => rows::ThresholdMode::Greater },
                amounts: amounts(part),
            },
            _ => return None,
        }))
    }
}

/// The Threshold Level Emitter's band: its upper bound is the level
/// emitter's threshold (`emitter_threshold`, what AE2's screen sets), its
/// lower bound and state the mod's fields.
pub(in crate::game) fn threshold(part: &Compound) -> rows::Threshold {
    let long = |name: &str| match part.get(&name.into()) {
        Some(Tag::Long(n)) => *n,
        _ => 0,
    };
    rows::Threshold { upper: long("emitter_threshold"), lower: long(rows::LOWER), state: matches!(part.get(&rows::CURRENT_STATE.into()), Some(Tag::Byte(1))) }
}
pub(in crate::game) fn store_threshold(part: &mut Compound, t: rows::Threshold) {
    part.insert("emitter_threshold".into(), Tag::Long(t.upper));
    part.insert(rows::LOWER.into(), Tag::Long(t.lower));
    part.insert(rows::CURRENT_STATE.into(), Tag::Byte(i8::from(t.state)));
}
/// `PartThresholdLevelEmitter.isLevelEmitterOn` on an active node.
pub(in crate::game) fn emitter_signal(part: &mut Compound, low: bool, count: i64) -> Option<bool> {
    if part_id(part) != rows::THRESHOLD_LEVEL_EMITTER {
        return None;
    }
    let mut t = threshold(part);
    let on = t.update(count, low, true);
    store_threshold(part, t);
    Some(on)
}

/// A `configTypes` inventory of 63 keys (`config`: the Smart Annihilation
/// and Active Formation Planes', the Vacuum Interface's), items or fluids,
/// saved as `{Slot, id, components}` entries.
pub(in crate::game) const CONFIG: &str = "config";
pub(in crate::game) fn config_keys(tag: Option<&Tag>) -> Vec<Option<Key>> {
    let mut out = vec![None; 63];
    if let Some(Tag::List(list)) = tag {
        for entry in list {
            let (Some(Tag::Byte(slot)), Some(Tag::String(id))) = (entry.get("Slot"), entry.get("id")) else { continue };
            let components = match entry.get("components") {
                Some(Tag::ByteArray(bytes)) => bytes.clone(),
                _ => Vec::new(),
            };
            if let Some(place) = out.get_mut(*slot as u8 as usize) {
                *place = Some(Key { id: id.to_string_lossy(), components });
            }
        }
    }
    out
}
pub(in crate::game) fn config_tag(keys: &[Option<Key>]) -> Tag {
    Tag::List(keys.iter().enumerate().filter_map(|(slot, key)| {
        let key = key.as_ref()?;
        Some(Tag::compound([("Slot", Tag::Byte(slot as i8)), ("id", Tag::String(key.id.as_str().into())), ("components", Tag::ByteArray(key.components.clone()))]))
    }).collect())
}
/// How many of a card a part's or machine's `upgrades` hold.
pub(in crate::game) fn card_count(upgrades: Option<&Tag>, card: &str) -> usize {
    match upgrades {
        Some(Tag::List(list)) => list.iter().filter(|c| c.get("id") == Some(&Tag::String(card.into()))).count(),
        _ => 0,
    }
}

/// `IPartitionList.builder()` over the first 18 + 9 per capacity card
/// config keys, fuzzy with a Fuzzy Card; `matchesFilter` in the list mode an
/// Inverter Card sets.
#[derive(Clone, Debug)]
pub(in crate::game) struct KeyList {
    keys: Vec<(Key, u32)>,
    fuzzy: Option<String>,
    inverted: bool,
}
impl KeyList {
    pub(in crate::game) fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
    pub(in crate::game) fn listed(&self, key: &Key) -> bool {
        self.keys.iter().any(|(f, max)| self.matches_one(f, *max, key))
    }
    /// `AEKey.fuzzyEquals` with the Fuzzy Card, else equality.
    fn matches_one(&self, f: &Key, max: u32, key: &Key) -> bool {
        match &self.fuzzy {
            None => f == key,
            Some(_) if crate::game::ae2_fluids::valid_fluid_id(&f.id) && crate::game::ae2_lab::prototype(&ae2_cell_core::ItemKey::from(f)).is_err() => f.id == key.id,
            Some(mode) => crate::game::ae2_buses::fuzzy_key_match(f, key, mode, max),
        }
    }
    /// The whitelist's keys in slot order, each as the grid's keys it
    /// fuzzily matches with a Fuzzy Card (`findFuzzy`), else itself.
    pub(in crate::game) fn wanted(&self, grid: &[Key]) -> Vec<Key> {
        self.keys.iter().flat_map(|(f, max)| match self.fuzzy {
            None => vec![f.clone()],
            Some(_) => grid.iter().filter(|k| self.matches_one(f, *max, k)).cloned().collect(),
        }).collect()
    }
    /// `matchesFilter(key, listMode)`: an empty list passes all.
    pub(in crate::game) fn matches(&self, key: &Key) -> bool {
        self.keys.is_empty() || self.listed(key) != self.inverted
    }
    pub(in crate::game) fn inverted(&self) -> bool {
        self.inverted
    }
}

impl Game<'_> {
    /// The key list of a device with `config` keys, `upgrades` and
    /// `fuzzy_mode` (none: AE2's IGNORE_ALL).
    pub(in crate::game) fn extendedae_key_list(&self, config: Option<&Tag>, upgrades: Option<&Tag>, fuzzy_mode: Option<String>) -> KeyList {
        let slots = rows::capacity_slots(card_count(upgrades, "ae2:capacity_card").min(5) as u8);
        let keys = config_keys(config).into_iter().take(slots).flatten()
            .map(|key| {
                let max = self.res.items().get(&key.id).map_or(0, |i| i.max_damage);
                (key, max)
            })
            .collect();
        KeyList {
            keys,
            fuzzy: (card_count(upgrades, "ae2:fuzzy_card") > 0).then(|| fuzzy_mode.unwrap_or_else(|| "ignore_all".into())),
            inverted: card_count(upgrades, "ae2:inverter_card") > 0,
        }
    }
    /// `PartSmartAnnihilationPlane.insertIntoGrid`: only what its list passes.
    pub(in crate::game) fn extendedae_plane_filter(&self, part: &Compound) -> Option<Box<dyn Fn(&horizon_client::items::ItemStack) -> bool>> {
        if part_id(part) != rows::SMART_ANNIHILATION_PLANE {
            return None;
        }
        let fuzzy = (!text(part, "fuzzy_mode").is_empty()).then(|| text(part, "fuzzy_mode"));
        let list = self.extendedae_key_list(part.get(&CONFIG.into()), part.get(&"upgrades".into()), fuzzy);
        Some(Box::new(move |stack| crate::game::ae2_lab::key(stack).is_ok_and(|key| list.matches(&Key::from(&key)))))
    }
}

impl Game<'_> {
    /// `PlacementStrategy.placeInWorld` of items at the Active Formation
    /// Plane's own target: one block, or a dropped stack of what the place
    /// takes. What it placed.
    fn extendedae_place(&mut self, target: &crate::game::ae2_formation::PlaneTarget, stack: &horizon_client::items::ItemStack) -> u64 {
        use crate::game::ae2_formation::{self as formation, FormationAction, FormationPlan};
        let mut plan = FormationPlan::default();
        let pos = target.target;
        if !self.world.loaded(pos.0, pos.2) {
            return 0;
        }
        // onNeighborChanged's clearBlocked: the target as it stands now.
        self.ae2_item_update_blocked(target.owner, pos);
        let placed = if target.entity {
            let Ok(Some((old, actual))) = self.ae2_preview_formation_entity(&plan, target) else { return 0 };
            let max = formation::max_stack(stack, self.res.items().get(&stack.item).map_or(64, |i| i.max_stack));
            let accepted = formation::accepted(actual, horizon_client::ae2_common_config::formation_entity_limit(), u64::from(stack.count), max);
            if accepted == 0 {
                return 0;
            }
            let direction = [pos.0 - target.host.0, pos.1 - target.host.1, pos.2 - target.host.2];
            let mut rng = self.world.random.clone();
            use horizon_core::random::RandomSource;
            let position = formation::spawn_position(target.host, direction, [rng.next_float(), rng.next_float(), rng.next_float()]);
            let mut spawned = stack.clone();
            spawned.count = accepted;
            plan.actions.push(FormationAction::Entity { dim: self.dim, target: pos, old, position, direction, stack: spawned, plane: target.owner });
            u64::from(accepted)
        } else {
            let mut one = stack.clone();
            one.count = 1;
            match self.ae2_preview_formation_block(&plan, target, &one) {
                Ok(Some(old)) => {
                    plan.actions.push(FormationAction::Place { dim: self.dim, pos, old, stack: one, plane: target.owner, player_owner: target.player_owner });
                    1
                }
                _ => 0,
            }
        };
        if placed > 0 && self.ae2_commit_formation(plan).is_err() {
            return 0;
        }
        placed
    }

    /// `PartActiveFormationPlane.tickingRequest`: AE2's export bus pace
    /// (2 ticks at the fastest), its cards and redstone; `doWork` takes what
    /// its list names from the grid and places it, one block, or a dropped
    /// stack of the speed cards' multiple (`getExtractAmount`), and puts back
    /// what did not go out.
    pub(in crate::game) fn extendedae_active_plane_tick(&mut self, pos: (i32, i32, i32), data: &mut Compound, identity: ae2_network_core::NodeIdentity, service: Option<u64>, now: u64) {
        use crate::game::ae2_tick_tracker::Modulation;
        if part_id(data) != rows::ACTIVE_FORMATION_PLANE {
            return;
        }
        let face = text(data, "face");
        let Ok(mut state) = crate::game::ae2_buses::bus_state(data) else { return };
        if state.redstone_card && state.redstone != ae2_machines_core::Redstone::Ignore {
            let at = horizon_core::position::BlockPos::new(pos.0, pos.1, pos.2);
            let signal = self.with_level(|b, level, ticks| b.has_neighbor_signal(level, ticks, at));
            state.observe_redstone(signal);
        }
        let alert = matches!(data.remove(&"bus_next_tick".into()), Some(Tag::Long(0))) || (state.redstone == ae2_machines_core::Redstone::Pulse && state.pending_pulse);
        let (_, max) = horizon_client::ae2_tick_config::rates(horizon_client::ae2_tick_config::Kind::ExportBus);
        if self.ae2_device_scheduler.borrow_mut().due(identity, service, now, 2, max, state.sleeping(), alert) {
            let modulation = if state.sleeping() {
                Modulation::Sleep
            } else {
                match self.extendedae_active_plane_work(pos, &face, data, &mut state) {
                    None => Modulation::Idle,
                    Some(true) => Modulation::Faster,
                    Some(false) => Modulation::Slower,
                }
            };
            self.ae2_device_scheduler.borrow_mut().completed(identity, now, modulation);
        }
        if let Ok(json) = serde_json::to_string(&state) {
            data.insert("state_json".into(), Tag::String(json.as_str().into()));
        }
    }
    /// `canWork` (none when it cannot) and `doWork`.
    fn extendedae_active_plane_work(&mut self, pos: (i32, i32, i32), face: &str, data: &Compound, state: &mut BusState) -> Option<bool> {
        let place_block = !matches!(data.get(&"formation_entity".into()), Some(Tag::Byte(1)));
        let front = front(pos, face)?;
        if !self.ae2_part_online(pos, face) || !self.world.loaded(front.0, front.2) {
            return None;
        }
        if place_block && !matches!(self.res.facts().get(self.world.block(front.0, front.1, front.2)).name.as_ref(), "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air") {
            return None;
        }
        state.pending_pulse = false;
        let address = self.ae2_local_address(pos)?;
        let side = crate::game::ae2_grid::face_index(face)?;
        let network = self.ae2_network();
        let identity = network.part_identities.get(&(address.network_pos(), side)).copied()?;
        let player_owner = match data.get(&"ae2_node_owner_uuid".into()) {
            Some(Tag::IntArray(words)) => words.as_slice().try_into().ok(),
            _ => None,
        };
        let target = crate::game::ae2_formation::PlaneTarget {
            owner: crate::game::ae2_formation::PlaneOwner { address, identity, face: side },
            host: pos,
            target: front,
            entity: !place_block,
            player_owner,
        };
        let fuzzy = (!text(data, "fuzzy_mode").is_empty()).then(|| text(data, "fuzzy_mode"));
        let config = config_tag(&state.config);
        let list = self.extendedae_key_list(Some(&config), data.get(&"upgrades".into()), fuzzy);
        let multiplier = if place_block { 1 } else { rows::speed_operations(state.speed_cards) };
        let grid: Vec<Key> = self.ae2_grid_inventory(pos).unwrap_or_default().iter().map(|(k, _)| Key::from(k)).collect();
        let candidates: Vec<Key> = if list.inverted() {
            grid.into_iter().filter(|key| !list.is_empty() && !list.listed(key)).collect()
        } else {
            list.wanted(&grid)
        };
        for key in candidates {
            let item = ae2_cell_core::ItemKey::from(&key);
            let Ok(Some(taken)) = self.ae2_grid_extract(pos, &item, multiplier) else { continue };
            let placed = self.extendedae_place(&target, &taken);
            let back = u64::from(taken.count).saturating_sub(placed);
            if back > 0 {
                let mut rest = taken.clone();
                rest.count = back as u32;
                let _ = self.ae2_grid_insert(pos, &rest, back);
            }
            if placed > 0 {
                return Some(true);
            }
        }
        Some(false)
    }
}

/// The block a part on `face` of `pos` faces.
pub(in crate::game) fn front(pos: (i32, i32, i32), face: &str) -> Option<(i32, i32, i32)> {
    let (x, y, z) = pos;
    Some(match face {
        "down" => (x, y - 1, z),
        "up" => (x, y + 1, z),
        "north" => (x, y, z - 1),
        "south" => (x, y, z + 1),
        "west" => (x - 1, y, z),
        "east" => (x + 1, y, z),
        _ => return None,
    })
}

/// `/ae2 eae ...`: what the mod's screens set, by command (the world tests'
/// way in, and an operator's).
pub(in crate::game) const USAGE: &str = "usage: /ae2 eae (tag <x y z> <face> white|black <expression|->) | (mod <x y z> <face> <ids|->) | (amount <x y z> <face> <slot> <n>) | (mode <x y z> <face> greater|lower) | (band <x y z> <face> <upper> <lower>) | (config <x y z> <face|block> <slot> <id|fluid:<id>|none>) | (area <x y z> <sx sy sz> <ox oy oz>) | (show <x y z> <face|block>)";

fn key_of(word: &str) -> Result<Option<Key>, String> {
    if word == "none" {
        return Ok(None);
    }
    if let Some(fluid) = word.strip_prefix("fluid:") {
        return Ok(Some(Key::from(&horizon_client::ae2_cell_codec::fluid_key(fluid))));
    }
    let stack = horizon_client::items::ItemStack::new(word, 1);
    Ok(Some(Key::from(&crate::game::ae2_lab::key(&stack)?)))
}

impl Game<'_> {
    /// One part of the cable bus at `pos` on `face`, changed by `f`.
    fn extendedae_part_mut<R>(&mut self, pos: (i32, i32, i32), face: &str, f: impl FnOnce(&mut Compound) -> Result<R, String>) -> Result<R, String> {
        let Some(horizon_client::containers::BlockEntity::Ae2(m)) = self.world.block_entities.get_mut(&pos) else { return Err("no cable bus there".into()) };
        let Some(Tag::List(parts)) = m.config.get_mut(&"parts".into()) else { return Err("no parts there".into()) };
        let Some(Tag::Compound(part)) = parts.iter_mut().find(|p| p.get("face") == Some(&Tag::String(face.into()))) else { return Err("no part on that face".into()) };
        let out = f(part)?;
        self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
        Ok(out)
    }
    fn extendedae_vacuum_mut(&mut self, pos: (i32, i32, i32)) -> Result<&mut horizon_client::ae2_world::Machine, String> {
        self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
        match self.world.block_entities.get_mut(&pos) {
            Some(horizon_client::containers::BlockEntity::Ae2(m)) if m.id() == rows::VACUUM_INTERFACE => Ok(&mut **m),
            _ => Err("not a vacuum interface".into()),
        }
    }
    pub(in crate::game) fn extendedae_command(&mut self, w: &[&str]) -> Result<serde_json::Value, String> {
        use serde_json::json;
        if w.len() < 5 {
            return Err(USAGE.into());
        }
        let n = |s: &str| s.parse::<i64>().map_err(|_| format!("not a number: {s}"));
        let pos = (n(w[1])? as i32, n(w[2])? as i32, n(w[3])? as i32);
        let face = w[4];
        let rest = &w[5..];
        let joined = |words: &[&str]| {
            let value = words.join(" ");
            if value == "-" { String::new() } else { value }
        };
        match (w[0], rest) {
            ("tag", [side, expression @ ..]) if matches!(*side, "white" | "black") => {
                let value = joined(expression);
                let field = if *side == "white" { rows::TAG_WHITE } else { rows::TAG_BLACK };
                self.extendedae_part_mut(pos, face, |part| {
                    if !matches!(part_id(part).as_str(), rows::TAG_STORAGE_BUS | rows::TAG_EXPORT_BUS) {
                        return Err("not a tag bus".into());
                    }
                    part.insert(field.into(), Tag::String(value.as_str().into()));
                    Ok(())
                })?;
                Ok(json!({"ok": true}))
            }
            ("mod", ids) => {
                let value = joined(ids);
                self.extendedae_part_mut(pos, face, |part| {
                    if !matches!(part_id(part).as_str(), rows::MOD_STORAGE_BUS | rows::MOD_EXPORT_BUS) {
                        return Err("not a mod bus".into());
                    }
                    part.insert(rows::MOD_IDS.into(), Tag::String(value.as_str().into()));
                    Ok(())
                })?;
                Ok(json!({"ok": true}))
            }
            ("amount", [slot, amount]) => {
                let (slot, amount) = (n(slot)? as usize, n(amount)?.max(0) as u64);
                self.extendedae_part_mut(pos, face, |part| {
                    let id = part_id(part);
                    if !matches!(id.as_str(), rows::PRECISE_STORAGE_BUS | rows::PRECISE_EXPORT_BUS | rows::THRESHOLD_EXPORT_BUS) || slot >= 63 {
                        return Err("not a precise or threshold bus slot".into());
                    }
                    let mut all = amounts(part);
                    all[slot] = amount;
                    store_amounts(part, &all);
                    Ok(())
                })?;
                Ok(json!({"ok": true}))
            }
            ("mode", [mode]) => {
                let mode = match *mode {
                    "greater" => rows::ThresholdMode::Greater,
                    "lower" => rows::ThresholdMode::Lower,
                    _ => return Err(USAGE.into()),
                };
                self.extendedae_part_mut(pos, face, |part| {
                    if part_id(part) != rows::THRESHOLD_EXPORT_BUS {
                        return Err("not a threshold export bus".into());
                    }
                    part.insert(rows::THRESHOLD_MODE.into(), Tag::Byte(mode.ordinal()));
                    Ok(())
                })?;
                Ok(json!({"ok": true}))
            }
            ("band", [upper, lower]) => {
                let (upper, lower) = (n(upper)?, n(lower)?);
                self.extendedae_part_mut(pos, face, |part| {
                    if part_id(part) != rows::THRESHOLD_LEVEL_EMITTER {
                        return Err("not a threshold level emitter".into());
                    }
                    let mut t = threshold(part);
                    t.upper = upper;
                    t.lower = lower;
                    store_threshold(part, t);
                    Ok(())
                })?;
                Ok(json!({"ok": true}))
            }
            ("config", [slot, key]) => {
                let (slot, key) = (n(slot)? as usize, key_of(key)?);
                if slot >= 63 {
                    return Err("slot out of range".into());
                }
                if face == "block" {
                    let m = self.extendedae_vacuum_mut(pos)?;
                    let mut keys = config_keys(m.config.get(&CONFIG.into()));
                    keys[slot] = key;
                    m.config.insert(CONFIG.into(), config_tag(&keys));
                    return Ok(json!({"ok": true}));
                }
                self.extendedae_part_mut(pos, face, |part| {
                    if part_id(part) == rows::SMART_ANNIHILATION_PLANE {
                        let mut keys = config_keys(part.get(&CONFIG.into()));
                        keys[slot] = key;
                        part.insert(CONFIG.into(), config_tag(&keys));
                    } else {
                        let mut state = crate::game::ae2_buses::bus_state(part)?;
                        state.config[slot] = key;
                        let json = serde_json::to_string(&state).map_err(|e| e.to_string())?;
                        part.insert("state_json".into(), Tag::String(json.as_str().into()));
                    }
                    Ok(())
                })?;
                Ok(json!({"ok": true}))
            }
            ("area", [sy, sz, ox, oy, oz]) => {
                // `face` is the size's x here.
                let size = [n(face)? as i32, n(sy)? as i32, n(sz)? as i32];
                let offset = [n(ox)? as i32, n(oy)? as i32, n(oz)? as i32];
                let m = self.extendedae_vacuum_mut(pos)?;
                super::interfaces::set_vacuum_shape(m, size, offset);
                let (size, offset) = super::interfaces::vacuum_shape(m);
                Ok(json!({"size": size, "offset": offset}))
            }
            ("show", []) => {
                if face == "block" {
                    let m = self.extendedae_vacuum_mut(pos)?;
                    let (size, offset) = super::interfaces::vacuum_shape(m);
                    let keys: Vec<_> = config_keys(m.config.get(&CONFIG.into())).into_iter().map(|k| k.map(|k| k.id)).collect();
                    return Ok(json!({"id": m.id(), "size": size, "offset": offset, "config": keys}));
                }
                let part = self.extendedae_part_mut(pos, face, |part| Ok(part.clone()))?;
                let t = threshold(&part);
                Ok(json!({
                    "id": part_id(&part),
                    "white": text(&part, rows::TAG_WHITE),
                    "black": text(&part, rows::TAG_BLACK),
                    "mod": text(&part, rows::MOD_IDS),
                    "amounts": amounts(&part).into_iter().take(9).collect::<Vec<_>>(),
                    "upper": t.upper,
                    "lower": t.lower,
                    "state": t.state,
                    "active": part.get(&"emitter_active".into()) == Some(&Tag::Byte(1)),
                }))
            }
            _ => Err(USAGE.into()),
        }
    }
}

/// A precise or threshold bus's config slot set from the hand (`CONFIG_STACKS`
/// with overstacking): the held amount, a bucket for a fluid; the Precise
/// Export Bus's at most 64 units (`PreciseInventory.getMaxAmount`).
pub(in crate::game) fn config_amount(part: &mut Compound, slot: usize, amount: Option<u64>) {
    let id = part_id(part);
    if !matches!(id.as_str(), rows::PRECISE_STORAGE_BUS | rows::PRECISE_EXPORT_BUS | rows::THRESHOLD_EXPORT_BUS) || slot >= 63 {
        return;
    }
    let mut all = amounts(part);
    all[slot] = amount.unwrap_or(1).max(1);
    store_amounts(part, &all);
}
