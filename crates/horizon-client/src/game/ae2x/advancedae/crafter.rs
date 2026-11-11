//! Advanced AE's Quantum Crafter in the game: the crafting patterns it runs
//! on its own from the grid's storage, with power from its own buffer or the
//! grid, its speed and redstone cards, the output to the grid or to the sides
//! allowed, its menu's own entries and its saved jobs. Derived from Advanced
//! AE (`QuantumCrafterEntity`, `QuantumCrafterBlock`, `QuantumCrafterMenu`,
//! `QuantumCrafterScreen`), https://github.com/pedroksl/AdvancedAE commit
//! `c322dad81e6f2ad94cb1fb861e73a7ba68e5907a`, LGPL-3.0; see
//! `ae2/addons/advancedae/`.
//!
//! The machine is a [`Machine`] of 27 slots (`patternInv`: nine patterns of
//! one, then `outputInv`: eighteen of 1,024) and five cards. Its config holds
//! the mod's settings as `IConfigManager.writeToNBT` names them (`me_export`,
//! `quantum_crafter_terminal`, `redstone_controlled`), `allowedOutputs` as the
//! Reaction Chamber's side flags, `enabledPatterns` a flag per slot
//! (`aae_enabled_1`..), `invalidPatterns` a mask (`aae_invalid`), each slot's
//! `CraftingJob` (`aae_job_<n>`: the pattern it is for, `listMinInput`,
//! `limitMaxOutput`) and `sendList` (`aae_send`, AE2 generic stacks). The
//! block state's `working` is not saved. A pattern's decoding is kept by
//! block entity while the game runs ([`Ctx`]).
use super::chamber;
use crate::game::{Game, ae2_buses::FormationPlan, ae2_global::GridAddress, ae2_tick_tracker::Modulation};
use horizon_client::{
    ae2_generic_stack::GenericStack,
    ae2_world::{Machine, MachineKind},
    ae2x::advancedae as rows,
    containers::BlockEntity,
    item_data::canon_stack,
    items::ItemStack,
};
use horizon_core::geometry::Direction;
use horizon_world::nbt::Tag;
use std::ops::Range;

pub(in crate::game) const PATTERNS: usize = 9;
pub(in crate::game) const SLOTS: usize = PATTERNS + 18;
/// `MAX_OUTPUT_INV_SIZE`: what an output slot holds.
pub(in crate::game) const OUTPUT_CAP: u32 = 1024;
/// `MAX_CRAFT_AMOUNT`.
const MAX_CRAFT: i64 = 1024;

/// `AAESettings.ME_EXPORT`, `QUANTUM_CRAFTER_TERMINAL`, AE2's `Settings.REDSTONE_CONTROLLED`.
pub(in crate::game) const ME_EXPORT: &str = "me_export";
pub(in crate::game) const TERMINAL: &str = "quantum_crafter_terminal";
pub(in crate::game) const REDSTONE: &str = "redstone_controlled";
/// `enabledPatterns`, a flag per pattern slot.
pub(in crate::game) const ENABLED: [&str; PATTERNS] =
    ["aae_enabled_1", "aae_enabled_2", "aae_enabled_3", "aae_enabled_4", "aae_enabled_5", "aae_enabled_6", "aae_enabled_7", "aae_enabled_8", "aae_enabled_9"];
const INVALID: &str = "aae_invalid";
const JOB: &str = "aae_job_";
const SEND: &str = "aae_send";

/// The menu's own entries after the slots (0..27) and cards (27..32), and the data slots they read.
pub(in crate::game) const REDSTONE_ENTRY: usize = 32;
pub(in crate::game) const TERMINAL_ENTRY: usize = 33;
pub(in crate::game) const ME_EXPORT_ENTRY: usize = 34;
pub(in crate::game) const SIDE_ENTRIES: Range<usize> = 35..41;
pub(in crate::game) const ENABLE_ENTRIES: Range<usize> = 41..50;
pub(in crate::game) const ME_EXPORT_DATA: usize = 2;
pub(in crate::game) const SIDE_DATA: usize = 3;
pub(in crate::game) const REDSTONE_CARD_DATA: usize = 9;
pub(in crate::game) const INVALID_DATA: usize = 10;
pub(in crate::game) const ENABLE_DATA: usize = 11;

fn crafter_kind(kind: MachineKind) -> bool {
    kind.ext().is_some_and(|row| row.id == rows::QUANTUM_CRAFTER)
}
pub(in crate::game) fn is_crafter(m: &Machine) -> bool {
    crafter_kind(m.kind)
}
/// A pattern slot holds one, an output slot 1,024.
pub(in crate::game) fn slot_capacity(kind: MachineKind, slot: usize) -> Option<u32> {
    if !crafter_kind(kind) {
        return None;
    }
    match slot {
        s if s < PATTERNS => Some(1),
        s if s < SLOTS => Some(OUTPUT_CAP),
        _ => None,
    }
}
/// `QuantumCrafterMenu.isValidForSlot`: a pattern slot takes AE2's crafting pattern; an output slot
/// (`AppEngSlot`) anything.
pub(in crate::game) fn may_place(slot: usize, item: &ItemStack) -> bool {
    slot >= PATTERNS || item.item.as_ref() == "ae2:crafting_pattern"
}
/// `invExposed`: the patterns insert-only, the outputs extract-only.
pub(in crate::game) fn slot_insert(kind: MachineKind, index: usize) -> Option<bool> {
    crafter_kind(kind).then_some(index < PATTERNS)
}
pub(in crate::game) fn slot_extract(kind: MachineKind, index: usize) -> Option<bool> {
    crafter_kind(kind).then_some((PATTERNS..SLOTS).contains(&index))
}
/// `setPowerSides(getGridConnectableSides(..))`: every side.
pub(in crate::game) fn private_power_face(kind: MachineKind) -> bool {
    crafter_kind(kind)
}
/// The node: `REQUIRE_CHANNEL`, no idle draw, every side.
pub(in crate::game) fn device(m: &Machine, device: &mut ae2_network_core::Device) -> bool {
    if !is_crafter(m) {
        return false;
    }
    device.idle_override = Some(0.0);
    device.channel_override = Some(true);
    device.faces = 63;
    true
}
/// `setWorking`: the block state's `working`.
pub(in crate::game) fn visual_props(m: &Machine, props: &mut horizon_world::block_state_properties::BlockProperties) -> bool {
    if !is_crafter(m) || !props.contains_key("working") {
        return false;
    }
    props.insert("working".into(), flag(m, rows::WORKING).to_string());
    true
}
/// Whether a redstone card is in (the redstone button shows), and whether a pattern is invalid (the alert shows).
pub(in crate::game) fn screen_data(m: &Machine, data: &mut Vec<i16>) {
    if !is_crafter(m) {
        return;
    }
    if data.len() < ENABLE_DATA + PATTERNS {
        data.resize(ENABLE_DATA + PATTERNS, 0);
    }
    data[REDSTONE_CARD_DATA] = i16::from(redstone_card(m));
    data[INVALID_DATA] = i16::from(invalid_mask(m) != 0);
}

fn flag(m: &Machine, key: &str) -> bool {
    matches!(m.config.get(&key.into()), Some(Tag::Byte(1)))
}
fn text(m: &Machine, key: &str) -> Option<String> {
    match m.config.get(&key.into()) {
        Some(Tag::String(s)) => s.to_string().ok(),
        _ => None,
    }
}
fn redstone_card(m: &Machine) -> bool {
    m.upgrades.iter().flatten().any(|s| s.item.as_ref() == "ae2:redstone_card")
}
fn invalid_mask(m: &Machine) -> i32 {
    match m.config.get(&INVALID.into()) {
        Some(Tag::Int(n)) => *n,
        _ => 0,
    }
}
/// `ME_EXPORT` is YES (the default).
pub(in crate::game) fn me_export(m: &Machine) -> bool {
    text(m, ME_EXPORT).is_none_or(|v| v != "NO")
}
/// `isEnabled`: with a redstone card, `REDSTONE_CONTROLLED` against the signal.
pub(in crate::game) fn enabled(m: &Machine, powered: impl FnOnce() -> bool) -> bool {
    if !redstone_card(m) {
        return true;
    }
    match text(m, REDSTONE).as_deref() {
        Some("LOW_SIGNAL") => !powered(),
        Some("HIGH_SIGNAL") => powered(),
        _ => true,
    }
}
/// The progress a tick makes by the speed cards (`speedFactor`): crafts of each pattern.
pub(in crate::game) fn speed(cards: usize) -> i64 {
    match cards {
        0 => 1,
        1 => 8,
        2 => 16,
        3 => 32,
        _ => 64,
    }
}

/// What a pattern takes or gives: an item (one of it) or a fluid.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::game) enum Res {
    Item(ItemStack),
    Fluid(String),
}
impl Res {
    fn item(stack: &ItemStack) -> Self {
        let mut one = stack.clone();
        one.count = 1;
        Self::Item(one)
    }
    fn id(&self) -> Option<&str> {
        match self {
            Self::Item(s) => Some(s.item.as_ref()),
            Self::Fluid(_) => None,
        }
    }
    /// `AEKey.equals`: the item and its components, or the fluid.
    fn same(&self, other: &Res) -> bool {
        match (self, other) {
            (Self::Item(a), Self::Item(b)) => canon(a) == canon(b),
            (Self::Fluid(a), Self::Fluid(b)) => a == b,
            _ => false,
        }
    }
    fn stack(&self) -> Option<GenericStack> {
        Some(match self {
            Self::Item(s) => GenericStack::item(crate::game::ae2_lab::key(s).ok()?, 1),
            Self::Fluid(id) => GenericStack::fluid(horizon_client::ae2_cell_codec::fluid_key(id), 1),
        })
    }
    fn from_stack(stack: &GenericStack) -> Option<Self> {
        if stack.key_type == ae2_cell_core::KeyType::FLUIDS {
            return Some(Self::Fluid(stack.key.item_id.clone()));
        }
        horizon_client::ae2_cell_codec::prototype(&stack.key).ok().map(|s| Self::item(&s))
    }
}
fn canon(s: &ItemStack) -> String {
    let mut s = s.clone();
    s.count = 1;
    canon_stack(Some(&s))
}

/// One of `AECraftingPattern.getInputs()`: the slots of one item (`multiplier`), what may fill them, each of an amount.
#[derive(Clone, Debug)]
pub(in crate::game) struct Input {
    pub(in crate::game) choices: Vec<(Res, u64)>,
    pub(in crate::game) multiplier: u64,
}
/// `CraftingJob`'s pattern part: the inputs, the primary output and its amount, the condensed
/// `remainingItems` (`getRemainingItems`), and what `analyzePattern` finds.
#[derive(Clone, Debug)]
pub(in crate::game) struct Job {
    pub(in crate::game) inputs: Vec<Input>,
    pub(in crate::game) output: (ItemStack, u64),
    pub(in crate::game) remaining: Vec<ItemStack>,
    pub(in crate::game) consumes_durability: bool,
    pub(in crate::game) data_change: bool,
}

impl Job {
    /// From a crafting pattern's slots, its recipe's result and remainders (by slot), and whether its fluids
    /// substitute (`canSubstituteFluids`; a slot's fluid when its item is a bucket of one that leaves a bucket).
    pub(in crate::game) fn new(grid: &[Option<ItemStack>], result: &ItemStack, remainders: &[Option<ItemStack>], fluids: bool) -> Self {
        // AEProcessingPattern.condenseStacks: the slots' items summed, in their first slot's order.
        let mut condensed: Vec<(ItemStack, u64, usize)> = Vec::new();
        for (slot, stack) in grid.iter().enumerate() {
            let Some(stack) = stack else { continue };
            match condensed.iter_mut().find(|(s, _, _)| canon(s) == canon(stack)) {
                Some(entry) => entry.1 += u64::from(stack.count),
                None => condensed.push((stack.clone(), u64::from(stack.count), slot)),
            }
        }
        let fluid_of = |slot: usize| -> Option<&'static str> {
            let item = grid.get(slot)?.as_ref()?;
            let fluid = horizon_client::ae2_cell_codec::bucket_fluid(&item.item)?;
            let left = remainders.get(slot)?.as_ref()?;
            (left.item.as_ref() == "minecraft:bucket" && left.count == 1).then_some(fluid)
        };
        let inputs = condensed
            .iter()
            .map(|(stack, multiplier, slot)| {
                let mut choices = vec![(Res::item(stack), 1)];
                if let Some(fluid) = fluids.then(|| fluid_of(*slot)).flatten() {
                    choices.insert(0, (Res::Fluid(fluid.into()), 1000));
                }
                Input { choices, multiplier: *multiplier }
            })
            .collect();
        let buckets = if fluids { (0..grid.len()).filter(|s| fluid_of(*s).is_some()).count() as u32 } else { 0 };
        let mut remaining: Vec<ItemStack> = Vec::new();
        for left in remainders.iter().flatten() {
            match remaining.iter_mut().find(|s| s.item == left.item) {
                Some(s) => s.count += left.count,
                None => remaining.push(left.clone()),
            }
        }
        if buckets > 0 {
            remaining.retain_mut(|s| {
                if s.item.as_ref() != "minecraft:bucket" {
                    return true;
                }
                s.count = s.count.saturating_sub(buckets);
                s.count > 0
            });
        }
        let mut one = result.clone();
        one.count = 1;
        let mut job = Self { inputs, output: (one, u64::from(result.count)), remaining, consumes_durability: false, data_change: false };
        job.analyze();
        job
    }
    /// `analyzePattern`: an item that comes back damaged makes the pattern invalid; an output of an input's
    /// item with other components is a `hasDataChange`.
    fn analyze(&mut self) {
        for input in &self.inputs {
            let Some((Res::Item(item), _)) = input.choices.first() else { continue };
            if let Some(left) = self.remaining.iter().find(|s| s.item == item.item) {
                if left.damage > item.damage {
                    self.consumes_durability = true;
                    continue;
                }
            }
            if !self.data_change && self.output.0.item == item.item && canon(item) != canon(&self.output.0) {
                self.data_change = true;
                break;
            }
        }
    }
    fn remaining_of(&self, what: &Res) -> Option<&ItemStack> {
        let id = what.id()?;
        self.remaining.iter().find(|s| s.item.as_ref() == id)
    }
    /// `isInputConsumed`: not when as much comes back, nor when it is the output's item unchanged.
    pub(in crate::game) fn consumed(&self, what: &Res, amount: u64) -> bool {
        if self.remaining_of(what).is_some_and(|s| u64::from(s.count) >= amount) {
            return false;
        }
        let output = what.id() == Some(self.output.0.item.as_ref());
        !(output && !self.data_change)
    }
    /// `requiredInputTotal`: one craft's worth of what comes back, else `to_craft` crafts'.
    pub(in crate::game) fn required(&self, what: &Res, amount: u64, to_craft: i64) -> u64 {
        let multiplier = self.inputs.iter().find(|i| i.choices.iter().any(|(r, _)| r.same(what))).map_or(0, |i| i.multiplier * amount);
        let crafts = u64::try_from(to_craft).unwrap_or(0);
        match what {
            Res::Item(_) if self.remaining_of(what).is_some() => multiplier,
            _ => multiplier * crafts,
        }
    }
    /// `outputAmountPerCraft`: the output less what of it an input takes.
    pub(in crate::game) fn per_craft(&self) -> i64 {
        let out = Res::item(&self.output.0);
        let taken = self.inputs.iter().flat_map(|i| &i.choices).find(|(r, _)| r.same(&out)).map_or(0, |(_, n)| *n);
        self.output.1 as i64 - taken as i64
    }
    fn input_item(&self, id: &str) -> bool {
        self.inputs.iter().flat_map(|i| &i.choices).any(|(r, _)| r.id() == Some(id))
    }
}

/// A job's settings (`listMinInput`, `limitMaxOutput`).
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::game) struct Limits {
    pub(in crate::game) keep: Vec<u64>,
    pub(in crate::game) limit: u64,
}

/// The grid's storage as the crafter meets it.
pub(in crate::game) trait Grid {
    /// What a simulated extraction gives (`extract(.., SIMULATE)`).
    fn available(&mut self, what: &Res, amount: u64) -> u64;
    /// `StorageHelper.poweredExtraction`.
    fn extract(&mut self, what: &Res, amount: u64) -> u64;
    /// `insert(.., MODULATE)`; `powered`: `StorageHelper.poweredInsert`.
    fn insert(&mut self, what: &Res, amount: u64, powered: bool) -> u64;
}

/// `maximumCraftableAmount`: each input's choice that the grid has enough of (less what it keeps), and the
/// output's limit less what the grid and the outputs hold.
pub(in crate::game) fn max_crafts(job: &Job, limits: &Limits, outputs: &[Option<ItemStack>], grid: &mut impl Grid) -> i64 {
    let mut total = MAX_CRAFT;
    for (index, input) in job.inputs.iter().enumerate() {
        let keep = limits.keep.get(index).copied().unwrap_or(0);
        let mut found = false;
        for (what, amount) in &input.choices {
            let per = input.multiplier * amount;
            let consumed = job.consumed(what, *amount);
            let wanted = job.required(what, *amount, total) + if consumed { keep } else { 0 };
            let got = grid.available(what, wanted);
            if !consumed && got >= wanted {
                found = true;
                break;
            } else if got > keep {
                found = true;
                let got = got.min(i32::MAX as u64);
                total = total.min(((got - keep) as f64 / per as f64).floor() as i32 as i64);
                break;
            }
        }
        if !found {
            return 0;
        }
    }
    if limits.limit == 0 {
        return total;
    }
    let out = Res::item(&job.output.0);
    let stored = grid.available(&out, limits.limit);
    let held: u64 = outputs.iter().flatten().filter(|s| s.item == job.output.0.item).map(|s| u64::from(s.count)).sum();
    let room = limits.limit as f64 - stored as f64 - held as f64;
    let by_output = (room / job.per_craft() as f64).floor() as i32 as i64;
    total.min(by_output).max(0)
}

/// `hasAvailableOutputStorage`: with ME export nothing of the output waits to be sent; else a craft's output
/// fits in the outputs.
pub(in crate::game) fn output_room(job: &Job, me: bool, outputs: &[Option<ItemStack>], send: &[(Res, u64)]) -> bool {
    let out = Res::item(&job.output.0);
    if me {
        return !send.iter().any(|(r, _)| r.same(&out));
    }
    let mut stack = job.output.0.clone();
    stack.count = u32::try_from(job.output.1).unwrap_or(u32::MAX);
    let mut probe = outputs.to_vec();
    put(&mut probe, &mut stack);
    stack.count == 0
}

/// `insertItem` over the output slots in turn: `stack` keeps what did not fit.
fn put(outputs: &mut [Option<ItemStack>], stack: &mut ItemStack) {
    for slot in outputs.iter_mut() {
        if stack.count == 0 {
            return;
        }
        match slot {
            Some(held) if canon(held) == canon(stack) => {
                let n = stack.count.min(OUTPUT_CAP.saturating_sub(held.count));
                held.count += n;
                stack.count -= n;
            }
            Some(_) => {}
            None => {
                let n = stack.count.min(OUTPUT_CAP);
                let mut placed = stack.clone();
                placed.count = n;
                *slot = Some(placed);
                stack.count -= n;
            }
        }
    }
}

/// `performCraft` of `to_craft` crafts: each input's first choice the grid gives a craft of, then as many crafts
/// as the consumed inputs allow; the output to the send list (ME export) or the outputs, the remainders that are
/// no input to the outputs, the rest back to the grid (what it refuses to the send list).
pub(in crate::game) fn perform(job: &Job, to_craft: i64, me: bool, outputs: &mut [Option<ItemStack>], send: &mut Vec<(Res, u64)>, grid: &mut impl Grid) {
    let mut taken: Vec<(Res, u64, u64)> = Vec::new();
    for input in &job.inputs {
        for (what, amount) in &input.choices {
            let per = input.multiplier * amount;
            let got = grid.extract(what, job.required(what, *amount, to_craft));
            if got >= per {
                taken.push((what.clone(), per, got));
                break;
            }
            // The mod keeps a short extraction; the port gives it back.
            if got > 0 && grid.insert(what, got, false) < got {
                send.push((what.clone(), got));
            }
        }
    }
    let mut complete = if taken.len() == job.inputs.len() { to_craft.max(0) as u64 } else { 0 };
    for (what, per, got) in &taken {
        if job.consumed(what, *got) {
            complete = complete.min(got / per);
        }
    }
    let made = job.per_craft() * complete as i64;
    if made > 0 {
        let out = Res::item(&job.output.0);
        if me {
            send.push((out, made as u64));
        } else {
            let mut stack = job.output.0.clone();
            stack.count = u32::try_from(made).unwrap_or(u32::MAX);
            put(outputs, &mut stack);
            // The mod loses what does not fit; the port sends it to the grid.
            if stack.count > 0 {
                send.push((out, u64::from(stack.count)));
            }
        }
    }
    for left in &job.remaining {
        if job.input_item(left.item.as_ref()) || complete == 0 {
            continue;
        }
        let mut stack = left.clone();
        stack.count = left.count.saturating_mul(u32::try_from(complete).unwrap_or(u32::MAX));
        put(outputs, &mut stack);
        if stack.count > 0 {
            send.push((Res::item(&stack), u64::from(stack.count)));
        }
    }
    for (what, per, got) in taken {
        let back = got - if job.consumed(&what, got) { per * complete } else { 0 };
        if back > 0 {
            let stored = grid.insert(&what, back, false);
            if stored < back {
                send.push((what, back - stored));
            }
        }
    }
}

/// The send list as the config keeps it.
fn read_send(m: &Machine) -> Vec<(Res, u64)> {
    let Some(Tag::List(list)) = m.config.get(&SEND.into()) else { return Vec::new() };
    list.iter()
        .filter_map(|tag| horizon_client::ae2_generic_stack::decode(tag).ok().flatten())
        .filter_map(|s| Some((Res::from_stack(&s)?, u64::try_from(s.amount).ok()?)))
        .collect()
}
fn write_send(m: &mut Machine, send: &[(Res, u64)]) {
    let list: Vec<Tag> = send
        .iter()
        .filter(|(_, n)| *n > 0)
        .filter_map(|(r, n)| {
            let mut stack = r.stack()?;
            stack.amount = i64::try_from(*n).ok()?;
            horizon_client::ae2_generic_stack::encode(&stack).ok()
        })
        .collect();
    if list.is_empty() {
        m.config.remove(&SEND.into());
    } else {
        m.config.insert(SEND.into(), Tag::List(list));
    }
}
/// Slot `slot`'s job settings, when they are for the pattern of fingerprint `pattern`.
fn read_limits(m: &Machine, slot: usize, pattern: &str) -> Option<Limits> {
    let Some(Tag::Compound(job)) = m.config.get(&format!("{JOB}{slot}").as_str().into()) else { return None };
    match job.get(&"pattern".into()) {
        Some(Tag::String(p)) if p.to_string().ok().as_deref() == Some(pattern) => {}
        _ => return None,
    }
    let keep = match job.get(&"listMinInput".into()) {
        Some(Tag::LongArray(v)) => v.iter().map(|n| u64::try_from(*n).unwrap_or(0)).collect(),
        _ => Vec::new(),
    };
    let limit = match job.get(&"limitMaxOutput".into()) {
        Some(Tag::Long(n)) => u64::try_from(*n).unwrap_or(0),
        _ => 0,
    };
    Some(Limits { keep, limit })
}
fn write_limits(m: &mut Machine, slot: usize, pattern: &str, limits: &Limits) {
    let keep = limits.keep.iter().map(|n| i64::try_from(*n).unwrap_or(i64::MAX)).collect();
    let job = Tag::compound([
        ("pattern", Tag::String(pattern.into())),
        ("listMinInput", Tag::LongArray(keep)),
        ("limitMaxOutput", Tag::Long(i64::try_from(limits.limit).unwrap_or(i64::MAX))),
    ]);
    m.config.insert(format!("{JOB}{slot}").as_str().into(), job);
}

/// A crafter's decoded patterns by slot (their fingerprint and job), while the game runs.
#[derive(Clone, Debug, Default)]
pub(in crate::game) struct Ctx {
    patterns: Vec<Option<(String, Option<Job>)>>,
}

/// `makeCraftingRecipeList`: each slot's job follows its pattern (new settings for a new one, none for an empty
/// slot or another item), and `invalidPatterns` those that consume durability.
fn sync_jobs(m: &mut Machine, ctx: &mut Ctx, decode: &dyn Fn(&ItemStack) -> Option<Job>) -> Vec<Option<(Job, Limits)>> {
    ctx.patterns.resize(PATTERNS, None);
    let mut jobs = Vec::with_capacity(PATTERNS);
    let mut invalid = 0;
    for slot in 0..PATTERNS {
        let Some(pattern) = m.slots.get(slot).cloned().flatten() else {
            ctx.patterns[slot] = None;
            m.config.remove(&format!("{JOB}{slot}").as_str().into());
            jobs.push(None);
            continue;
        };
        let print = canon_stack(Some(&pattern));
        if ctx.patterns[slot].as_ref().is_none_or(|(p, _)| *p != print) {
            ctx.patterns[slot] = Some((print.clone(), decode(&pattern)));
        }
        let Some(job) = ctx.patterns[slot].as_ref().and_then(|(_, j)| j.clone()) else {
            m.config.remove(&format!("{JOB}{slot}").as_str().into());
            jobs.push(None);
            continue;
        };
        let limits = read_limits(m, slot, &print).unwrap_or_else(|| Limits { keep: vec![0; job.inputs.len()], limit: 0 });
        if read_limits(m, slot, &print).as_ref() != Some(&limits) {
            write_limits(m, slot, &print, &limits);
        }
        if job.consumes_durability {
            invalid |= 1 << slot;
        }
        jobs.push(Some((job, limits)));
    }
    if invalid == 0 {
        m.config.remove(&INVALID.into());
    } else {
        m.config.insert(INVALID.into(), Tag::Int(invalid));
    }
    jobs
}
/// The jobs `performCrafts` runs: valid and enabled.
fn runnable(m: &Machine, jobs: &[Option<(Job, Limits)>]) -> Vec<usize> {
    (0..PATTERNS).filter(|n| jobs.get(*n).is_some_and(Option::is_some) && invalid_mask(m) & (1 << n) == 0 && flag(m, ENABLED[*n])).collect()
}
/// `hasCraftWork`: a runnable job with a craft and room for its output.
fn craft_work(m: &Machine, jobs: &[Option<(Job, Limits)>], send: &[(Res, u64)], grid: &mut impl Grid) -> bool {
    let me = me_export(m);
    runnable(m, jobs).into_iter().any(|n| {
        let Some((job, limits)) = &jobs[n] else { return false };
        max_crafts(job, limits, &m.slots[PATTERNS..], grid) > 0 && output_room(job, me, &m.slots[PATTERNS..], send)
    })
}

struct GameGrid<'a, 'g> {
    game: &'a mut Game<'g>,
    address: GridAddress,
}
impl Grid for GameGrid<'_, '_> {
    fn available(&mut self, what: &Res, amount: u64) -> u64 {
        match what {
            Res::Item(s) => crate::game::ae2_lab::key(s)
                .ok()
                .and_then(|key| self.game.ae2_grid_extract_uncharged_encoded(self.address, &key, amount, true).ok().flatten())
                .map_or(0, |s| u64::from(s.count)),
            Res::Fluid(id) => self.game.ae2_grid_fluid_extract_encoded(self.address, &horizon_client::ae2_cell_codec::fluid_key(id), amount, true).unwrap_or(0),
        }
    }
    fn extract(&mut self, what: &Res, amount: u64) -> u64 {
        if amount == 0 {
            return 0;
        }
        match what {
            Res::Item(s) => crate::game::ae2_lab::key(s)
                .ok()
                .and_then(|key| self.game.ae2_grid_extract_encoded(self.address, &key, amount).ok().flatten())
                .map_or(0, |s| u64::from(s.count)),
            Res::Fluid(id) => self.game.ae2_grid_fluid_extract_encoded(self.address, &horizon_client::ae2_cell_codec::fluid_key(id), amount, false).unwrap_or(0),
        }
    }
    fn insert(&mut self, what: &Res, amount: u64, powered: bool) -> u64 {
        match what {
            Res::Item(s) => {
                let mut stack = s.clone();
                stack.count = u32::try_from(amount).unwrap_or(u32::MAX);
                let result = if powered {
                    self.game.ae2_grid_insert_encoded(self.address, &stack, amount)
                } else {
                    self.game.ae2_grid_insert_uncharged_storage_encoded(self.address, &stack, amount, false)
                };
                result.unwrap_or(0)
            }
            Res::Fluid(id) => {
                let key = horizon_client::ae2_cell_codec::fluid_key(id);
                let result = if powered {
                    self.game.ae2_grid_fluid_insert_encoded(self.address, &key, amount, false)
                } else {
                    self.game.ae2_grid_fluid_insert_uncharged_encoded(self.address, &key, amount)
                };
                result.unwrap_or(0)
            }
        }
    }
}

/// `side` of a block facing north with spin 0, for one facing `facing` with spin `spin`
/// (`BlockOrientation.getSide`): front to the facing, top to its up, right = front x up.
fn turned(side: Direction, facing: &str, spin: usize) -> Direction {
    let front = match facing {
        "down" => Direction::Down,
        "up" => Direction::Up,
        "south" => Direction::South,
        "west" => Direction::West,
        "east" => Direction::East,
        _ => Direction::North,
    };
    let up = crate::game::machines::ae2_capabilities::orientation_up(facing, spin).unwrap_or(Direction::Up);
    let [f, u] = [front.normal(), up.normal()];
    let right = [f[1] * u[2] - f[2] * u[1], f[2] * u[0] - f[0] * u[2], f[0] * u[1] - f[1] * u[0]];
    let of = |v: [i32; 3]| Direction::ALL.into_iter().find(|d| d.normal() == v).unwrap_or(Direction::North);
    match side {
        Direction::North => front,
        Direction::South => front.opposite(),
        Direction::Up => up,
        Direction::Down => up.opposite(),
        Direction::East => of(right),
        Direction::West => of(right).opposite(),
    }
}

impl Game<'_> {
    /// A crafting pattern as the crafter runs it: AE2's decoding and recipe (the result must be the encoded output).
    fn advancedae_crafter_job(&self, pattern: &ItemStack) -> Option<Job> {
        let (grid, encoded) = crate::game::ae2_processing::decode_pattern(pattern)?;
        let recipes = self.res.recipes();
        let (name, result) = match crate::game::ae2_source_patterns::crafting(pattern).and_then(Result::ok) {
            Some(source) => (source.recipe.clone(), recipes.assemble_named(&source.recipe, &grid, 3)?),
            None => {
                let (name, result) = recipes.find(&grid, 3)?;
                if result != encoded {
                    return None;
                }
                (name.to_owned(), result)
            }
        };
        let fluids = crate::game::ae2_processing::ae2_crafting_pattern_options(pattern).is_some_and(|o| o.1);
        Some(Job::new(&grid, &result, &recipes.remainders(&name, &grid, 3), fluids))
    }
    fn advancedae_crafter_sync(&self, m: &mut Machine, ctx: &mut Ctx) -> Vec<Option<(Job, Limits)>> {
        sync_jobs(m, ctx, &|p| self.advancedae_crafter_job(p))
    }

    /// The Quantum Crafters' grid ticks (`TickingRequest(1, 20, !hasWork())`).
    pub(in crate::game) fn tick_advancedae_crafters(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let mut found: Vec<_> = self
            .world
            .block_entities
            .iter()
            .filter_map(|(p, e)| match e {
                BlockEntity::Ae2(m) if is_crafter(m) => Some(*p),
                _ => None,
            })
            .collect();
        found.sort_unstable();
        let live: std::collections::BTreeSet<u64> = found.iter().filter_map(|p| self.world.block_entities.incarnation(p)).collect();
        self.ae2x.advancedae.crafters.retain(|id, _| live.contains(id));
        for pos in found {
            let Some(incarnation) = self.world.block_entities.incarnation(&pos) else { continue };
            let Some(BlockEntity::Ae2(original)) = self.world.block_entities.get(&pos) else { continue };
            let mut m = (**original).clone();
            let before = m.clone();
            let mut ctx = self.ae2x.advancedae.crafters.remove(&incarnation).unwrap_or_default();
            let jobs = self.advancedae_crafter_sync(&mut m, &mut ctx);
            let at = horizon_core::position::BlockPos::new(pos.0, pos.1, pos.2);
            let on = enabled(&m, || self.with_level(|b, level, ticks| b.has_neighbor_signal(level, ticks, at)));
            // hasWork: enabled with a pattern in.
            let work = on && m.slots[..PATTERNS].iter().any(Option::is_some);
            if !work {
                m.config.insert(rows::WORKING.into(), Tag::Byte(0));
            }
            let wake = format!("{:?}|{:?}|{:?}|{on}", m.slots, m.upgrades, m.config);
            let due = self.ae2_variable_processor_due(pos, 1, 20, !work, wake.clone());
            let mut plan = FormationPlan::default();
            if due {
                let modulation = self.advancedae_crafter_step(pos, &mut m, &before, &jobs, on, &mut plan);
                let wake = format!("{:?}|{:?}|{:?}|{on}", m.slots, m.upgrades, m.config);
                self.ae2_variable_processor_completed(pos, modulation, wake);
            }
            self.ae2x.advancedae.crafters.insert(incarnation, ctx);
            if m != before {
                if let Some(BlockEntity::Ae2(target)) = self.world.block_entities.get_mut(&pos) {
                    **target = m;
                    self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
                }
                if let Err(error) = self.ae2_commit_formation(plan) {
                    log::warn!("Quantum Crafter notification: {error}");
                }
            }
        }
    }

    /// `tickingRequest`: with craft work, `10 x speedFactor` AE from the own buffer when it has it all, else from
    /// the grid, then `performCrafts`; then `pushOutResult`.
    fn advancedae_crafter_step(&mut self, pos: (i32, i32, i32), m: &mut Machine, original: &Machine, jobs: &[Option<(Job, Limits)>], on: bool, plan: &mut FormationPlan) -> Modulation {
        let address = self.ae2_local_address(pos);
        let Some(address) = address.filter(|_| self.ae2_online(pos)) else {
            m.config.insert(rows::WORKING.into(), Tag::Byte(0));
            return Modulation::Idle;
        };
        let mut send = read_send(m);
        let craft = on && craft_work(m, jobs, &send, &mut GameGrid { game: self, address });
        m.config.insert(rows::WORKING.into(), Tag::Byte(i8::from(craft)));
        if craft {
            let speed = speed(m.installed_speed_cards());
            let cost = 10.0 * speed as f64;
            let threshold = cost - 0.01;
            let usage = horizon_client::ae2_common_config::gameplay().usage_multiplier;
            let own = m.private_energy_extract(cost * usage, false) / usage;
            let paid = if own > threshold {
                m.private_energy_extract(cost * usage, true);
                true
            } else if self.ae2_take_power(pos, cost, true) > threshold {
                self.ae2_take_power(pos, cost, false);
                true
            } else {
                false
            };
            if paid {
                let me = me_export(m);
                let mut grid = GameGrid { game: self, address };
                for n in runnable(m, jobs) {
                    let Some((job, limits)) = &jobs[n] else { continue };
                    let crafts = max_crafts(job, limits, &m.slots[PATTERNS..], &mut grid).min(speed);
                    perform(job, crafts, me, &mut m.slots[PATTERNS..], &mut send, &mut grid);
                }
            }
        }
        let pushed = self.advancedae_crafter_push(pos, m, original, address, &mut send, plan);
        let modulation = if pushed || on && craft_work(m, jobs, &send, &mut GameGrid { game: self, address }) {
            Modulation::Urgent
        } else if !send.is_empty() || m.slots[PATTERNS..].iter().any(Option::is_some) {
            Modulation::Slower
        } else {
            Modulation::Idle
        };
        write_send(m, &send);
        modulation
    }

    /// `pushOutResult`: the send list into the grid, then (when that moved nothing) the outputs into the grid
    /// with ME export, else into the blocks on the allowed sides, each output slot to them in turn.
    fn advancedae_crafter_push(&mut self, pos: (i32, i32, i32), m: &mut Machine, original: &Machine, address: GridAddress, send: &mut Vec<(Res, u64)>, plan: &mut FormationPlan) -> bool {
        let mut moved = false;
        let mut grid = GameGrid { game: self, address };
        for (what, amount) in send.iter_mut() {
            let stored = grid.insert(what, *amount, false);
            moved |= stored > 0;
            *amount -= stored.min(*amount);
        }
        send.retain(|(_, n)| *n > 0);
        if moved {
            return true;
        }
        if me_export(m) {
            for slot in PATTERNS..SLOTS {
                let Some(stack) = m.slots[slot].clone() else { continue };
                let stored = grid.insert(&Res::item(&stack), u64::from(stack.count), true).min(u64::from(stack.count)) as u32;
                if stored > 0 {
                    let mut rest = stack;
                    rest.count -= stored;
                    m.slots[slot] = (rest.count > 0).then_some(rest);
                    moved = true;
                }
            }
            return moved;
        }
        let state = self.pack.assets.registry.state(self.world.block(pos.0, pos.1, pos.2));
        let facing = state.and_then(|s| s.properties.get("facing")).cloned().unwrap_or_else(|| "north".into());
        let spin = state.and_then(|s| s.properties.get("spin")).and_then(|s| s.parse().ok()).unwrap_or(0);
        let mut targets = Vec::new();
        for (_, key, side) in chamber::SIDES {
            if !flag(m, key) {
                continue;
            }
            let face = turned(side, &facing, spin);
            let d = face.normal();
            let neighbour = (pos.0 + d[0], pos.1 + d[1], pos.2 + d[2]);
            match self.world.block_entities.get(&neighbour) {
                None => continue,
                Some(BlockEntity::Ae2(other)) if chamber::is_chamber(other) => continue,
                Some(_) => {}
            }
            if let Some(target) = self.ae2_local_address(neighbour) {
                targets.push((target, face.opposite()));
            }
        }
        for slot in PATTERNS..SLOTS {
            for (target, side) in &targets {
                let Some(stack) = m.slots[slot].clone() else { break };
                let inserted = self.advancedae_crafter_offer(pos, original, *target, *side, &stack, plan);
                if inserted > 0 {
                    let mut rest = stack;
                    rest.count -= inserted;
                    horizon_client::ae2_stack_identity::synchronize_tracked(&rest);
                    m.slots[slot] = (rest.count > 0).then_some(rest);
                    moved = true;
                }
            }
        }
        moved
    }
    /// `target.addItems`: what the handler at `target` on its side `side` takes of `stack`.
    fn advancedae_crafter_offer(&mut self, own: (i32, i32, i32), original: &Machine, target: GridAddress, side: Direction, stack: &ItemStack, plan: &mut FormationPlan) -> u32 {
        let mut offered = stack.clone();
        horizon_client::ae2_stack_identity::detach_if_tracked(&mut offered);
        let offer = offered.count;
        let accepted = match self.ae2_item_p2p_insert(target, side, &offered, true) {
            Some(n) => n.min(offer),
            None => self.ae2_native_item_insert(target, side, &offered, offer, true),
        };
        if accepted == 0 {
            return 0;
        }
        let snapshot = self.ae2_snapshot_blocks();
        let previous = plan.len();
        let inserted = match self.ae2_item_p2p_insert_staged(target, side, &offered, false, plan) {
            Some(n) => n,
            None => self.ae2_native_item_insert_staged(target, side, &offered, offer, false, plan),
        };
        let unchanged = matches!(self.world.block_entities.get(&own), Some(BlockEntity::Ae2(current)) if **current == *original);
        if inserted == 0 || inserted > offer || !unchanged {
            self.ae2_restore_blocks(snapshot);
            plan.truncate(previous);
            return 0;
        }
        inserted
    }

    /// A press on the crafter's own entries (its settings, sides and radios): the setting steps on.
    pub(in crate::game) fn advancedae_crafter_click(&mut self, pos: (i32, i32, i32), slot: usize, button: i32, input: horizon_client::menu::Input) -> bool {
        let Some(BlockEntity::Ae2(m)) = self.world.block_entities.get_mut(&pos) else { return false };
        if !is_crafter(m) || !(300 + REDSTONE_ENTRY..300 + ENABLE_ENTRIES.end).contains(&slot) {
            return false;
        }
        if input == horizon_client::menu::Input::Pickup && (0..=1).contains(&button) {
            crate::game::ae2_screens::step_setting(m, slot, button == 1);
            self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
        }
        true
    }

    /// `/ae2 advae crafter <x> <y> <z> [keep <slot> <input> <amount> | limit <slot> <amount>]`: the jobs, and
    /// `setStockAmount` / `setMaxCrafted` (the mod's pattern configuration screen).
    pub(in crate::game) fn advancedae_crafter_command(&mut self, pos: (i32, i32, i32), rest: &[&str]) -> Result<serde_json::Value, String> {
        let Some(BlockEntity::Ae2(m)) = self.world.block_entities.get(&pos) else { return Err("no Quantum Crafter there".into()) };
        if !is_crafter(m) {
            return Err("no Quantum Crafter there".into());
        }
        let mut m = (**m).clone();
        let incarnation = self.world.block_entities.incarnation(&pos).unwrap_or_default();
        let mut ctx = self.ae2x.advancedae.crafters.remove(&incarnation).unwrap_or_default();
        let mut jobs = self.advancedae_crafter_sync(&mut m, &mut ctx);
        self.ae2x.advancedae.crafters.insert(incarnation, ctx);
        let number = |w: &str| w.parse::<u64>().map_err(|_| format!("bad number: {w}"));
        let edit = match rest {
            ["keep", slot, input, amount] => Some((number(slot)? as usize, Some(number(input)? as usize), number(amount)?)),
            ["limit", slot, amount] => Some((number(slot)? as usize, None, number(amount)?)),
            [] => None,
            _ => return Err("usage: crafter <x> <y> <z> [keep <slot> <input> <amount> | limit <slot> <amount>]".into()),
        };
        if let Some((slot, input, amount)) = edit {
            let Some(Some((job, limits))) = jobs.get_mut(slot) else { return Err("no crafting pattern in that slot".into()) };
            match input {
                Some(i) if i < job.inputs.len() => limits.keep[i] = amount,
                Some(_) => return Err("no such input".into()),
                None => limits.limit = amount,
            }
            let print = canon_stack(m.slots[slot].as_ref());
            write_limits(&mut m, slot, &print, limits);
            if let Some(BlockEntity::Ae2(target)) = self.world.block_entities.get_mut(&pos) {
                **target = m.clone();
            }
            self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
        }
        let name = |r: &Res| match r {
            Res::Item(s) => s.item.to_string(),
            Res::Fluid(f) => format!("fluid:{f}"),
        };
        let patterns: Vec<serde_json::Value> = jobs
            .iter()
            .enumerate()
            .map(|(n, job)| match job {
                None => serde_json::Value::Null,
                Some((job, limits)) => serde_json::json!({
                    "output": job.output.0.item.as_ref(), "count": job.output.1, "limit": limits.limit,
                    "inputs": job.inputs.iter().zip(&limits.keep).map(|(i, keep)| serde_json::json!({
                        "choices": i.choices.iter().map(|(r, a)| serde_json::json!({"what": name(r), "amount": a})).collect::<Vec<_>>(),
                        "multiplier": i.multiplier, "keep": keep})).collect::<Vec<_>>(),
                    "enabled": flag(&m, ENABLED[n]), "invalid": invalid_mask(&m) & (1 << n) != 0,
                }),
            })
            .collect();
        let send: Vec<serde_json::Value> = read_send(&m).iter().map(|(r, n)| serde_json::json!({"what": name(r), "amount": n})).collect();
        Ok(serde_json::json!({"patterns": patterns, "working": flag(&m, rows::WORKING), "me_export": me_export(&m), "send": send}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(id: &str, n: u32) -> ItemStack {
        ItemStack::new(id, n)
    }
    /// A grid of counted items and fluids.
    #[derive(Default)]
    struct Store(Vec<(Res, u64)>, u64);
    impl Grid for Store {
        fn available(&mut self, what: &Res, amount: u64) -> u64 {
            self.0.iter().find(|(r, _)| r.same(what)).map_or(0, |(_, n)| (*n).min(amount))
        }
        fn extract(&mut self, what: &Res, amount: u64) -> u64 {
            let Some(entry) = self.0.iter_mut().find(|(r, _)| r.same(what)) else { return 0 };
            let n = entry.1.min(amount);
            entry.1 -= n;
            n
        }
        fn insert(&mut self, what: &Res, amount: u64, _: bool) -> u64 {
            if self.1 > 0 {
                return 0;
            }
            match self.0.iter_mut().find(|(r, _)| r.same(what)) {
                Some(entry) => entry.1 += amount,
                None => self.0.push((what.clone(), amount)),
            }
            amount
        }
    }
    fn count(store: &Store, id: &str) -> u64 {
        store.0.iter().filter(|(r, _)| r.id() == Some(id)).map(|(_, n)| n).sum()
    }
    /// Four planks from a log; a bucket of water and sand to clay with the bucket back.
    fn planks() -> Job {
        let mut grid = vec![None; 9];
        grid[0] = Some(stack("minecraft:oak_log", 1));
        Job::new(&grid, &stack("minecraft:oak_planks", 4), &vec![None; 9], false)
    }
    fn mud(fluids: bool) -> Job {
        let mut grid = vec![None; 9];
        grid[0] = Some(stack("minecraft:dirt", 1));
        grid[1] = Some(stack("minecraft:water_bucket", 1));
        let mut left = vec![None; 9];
        left[1] = Some(stack("minecraft:bucket", 1));
        Job::new(&grid, &stack("minecraft:mud", 1), &left, fluids)
    }

    #[test]
    fn a_pattern_condenses_its_slots_and_puts_a_fluid_first_when_it_substitutes() {
        let mut grid = vec![None; 9];
        for s in [0, 1, 3, 4] {
            grid[s] = Some(stack("minecraft:oak_planks", 1));
        }
        let table = Job::new(&grid, &stack("minecraft:crafting_table", 1), &vec![None; 9], false);
        assert_eq!((table.inputs.len(), table.inputs[0].multiplier), (1, 4));
        let job = mud(true);
        assert_eq!(job.inputs[1].choices[0], (Res::Fluid("minecraft:water".into()), 1000));
        assert!(job.remaining.is_empty(), "the bucket of the fluid's slot comes back as no item");
        assert_eq!(mud(false).remaining, vec![stack("minecraft:bucket", 1)]);
    }

    #[test]
    fn the_crafts_follow_the_grid_the_kept_stock_and_the_output_limit() {
        let job = planks();
        let log = Res::item(&stack("minecraft:oak_log", 1));
        let mut store = Store(vec![(log.clone(), 10)], 0);
        let mut limits = Limits { keep: vec![0], limit: 0 };
        assert_eq!(max_crafts(&job, &limits, &[], &mut store), 10);
        limits.keep[0] = 4;
        assert_eq!(max_crafts(&job, &limits, &[], &mut store), 6, "four logs kept");
        limits.limit = 20;
        let held = [Some(stack("minecraft:oak_planks", 8))];
        assert_eq!(max_crafts(&job, &limits, &held, &mut store), 3, "20 planks at most, 8 held: three crafts of four");
        store.0[0].1 = 4;
        assert_eq!(max_crafts(&job, &limits, &[], &mut store), 0, "no more than the kept stock");
    }

    #[test]
    fn a_craft_takes_the_inputs_and_sends_or_stores_the_output() {
        let job = planks();
        let log = Res::item(&stack("minecraft:oak_log", 1));
        let mut store = Store(vec![(log, 10)], 0);
        let mut outputs = vec![None; 18];
        let mut send = Vec::new();
        perform(&job, 8, true, &mut outputs, &mut send, &mut store);
        assert_eq!(count(&store, "minecraft:oak_log"), 2);
        assert_eq!(send, vec![(Res::item(&stack("minecraft:oak_planks", 1)), 32)]);
        assert!(!output_room(&job, true, &outputs, &send), "a waiting output stops the job with ME export");
        perform(&job, 2, false, &mut outputs, &mut send, &mut store);
        assert_eq!(outputs[0], Some(stack("minecraft:oak_planks", 8)));
        assert!(output_room(&job, false, &outputs, &send));
    }

    #[test]
    fn a_fluid_substitutes_and_a_remainder_goes_to_the_outputs() {
        let water = Res::Fluid("minecraft:water".into());
        let dirt = Res::item(&stack("minecraft:dirt", 1));
        let mut store = Store(vec![(dirt.clone(), 3), (water, 2500)], 0);
        let job = mud(true);
        let limits = Limits { keep: vec![0, 0], limit: 0 };
        assert_eq!(max_crafts(&job, &limits, &[], &mut store), 2, "2,500 mB is two crafts");
        let (mut outputs, mut send) = (vec![None; 18], Vec::new());
        perform(&job, 2, false, &mut outputs, &mut send, &mut store);
        assert_eq!((count(&store, "minecraft:dirt"), outputs[0].clone()), (1, Some(stack("minecraft:mud", 2))));
        assert_eq!(store.0[1].1, 500);
        // Without substitution the water bucket is the item, and its bucket comes back to the outputs.
        let mut store = Store(vec![(dirt, 1), (Res::item(&stack("minecraft:water_bucket", 1)), 1)], 0);
        let mut outputs = vec![None; 18];
        perform(&mud(false), 1, false, &mut outputs, &mut send, &mut store);
        assert_eq!((outputs[0].clone(), outputs[1].clone()), (Some(stack("minecraft:mud", 1)), Some(stack("minecraft:bucket", 1))));
    }

    #[test]
    fn what_the_grid_refuses_back_waits_in_the_send_list() {
        let water = Res::Fluid("minecraft:water".into());
        let mut store = Store(vec![(Res::item(&stack("minecraft:dirt", 1)), 1), (water.clone(), 2500)], 1);
        let mut send = Vec::new();
        perform(&mud(true), 2, false, &mut vec![None; 18], &mut send, &mut store);
        assert_eq!(send, vec![(water, 1000)], "one craft of two: the second craft's water waits");
        assert_eq!((store.0[0].1, store.0[1].1), (0, 500));
    }

    #[test]
    fn the_settings_follow_the_mods_defaults_and_the_sides_turn_with_facing_and_spin() {
        horizon_client::ae2x::install();
        let mut m = Machine::for_block(rows::QUANTUM_CRAFTER).unwrap();
        assert!(me_export(&m) && enabled(&m, || false));
        m.upgrades[0] = Some(stack("ae2:redstone_card", 1));
        m.config.insert(REDSTONE.into(), Tag::String("HIGH_SIGNAL".into()));
        assert!(!enabled(&m, || false) && enabled(&m, || true));
        assert_eq!((speed(0), speed(1), speed(4)), (1, 8, 64));
        assert_eq!(turned(Direction::North, "east", 0), Direction::East);
        assert_eq!(turned(Direction::East, "north", 0), Direction::East);
        assert_eq!(turned(Direction::East, "south", 0), Direction::West);
        assert_eq!((turned(Direction::Up, "north", 1), turned(Direction::East, "north", 1)), (Direction::West, Direction::Up));
        assert_eq!((slot_capacity(m.kind, 0), slot_capacity(m.kind, 9), slot_capacity(m.kind, 27)), (Some(1), Some(1024), None));
        let print = "p".to_string();
        let limits = Limits { keep: vec![3, 0], limit: 64 };
        write_limits(&mut m, 2, &print, &limits);
        assert_eq!(read_limits(&m, 2, &print), Some(limits));
        assert_eq!(read_limits(&m, 2, "another"), None, "a new pattern starts with new settings");
        let send = vec![(Res::item(&stack("minecraft:stone", 1)), 1500), (Res::Fluid("minecraft:water".into()), 250)];
        write_send(&mut m, &send);
        assert_eq!(read_send(&m), send);
    }
}
