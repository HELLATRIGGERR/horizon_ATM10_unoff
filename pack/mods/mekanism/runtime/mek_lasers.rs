// Mekanism's Laser, Laser Amplifier and Laser Tractor Beam in the game (support module `crate::game::mek_lasers` of
// the `mekanism` native bundle). Their rules are the bundle's crate (`mekanism_energy::laser`, MIT, from Mekanism
// 10.7.19, Copyright (c) 2017-2025 Aidan C. Brady); this module is the glue: their block entities in Mekanism's saved
// form (`energy_containers`, `last_fired`, `active_state`, the amplifier's `min`, `max`, `time`, `output_mode` and `control_type`),
// their ticks (the beam traced through the blocks' outlines, the Joules handed to a receptor or gathered against a
// block's hardness, the break and its drops, the tractor beam's into its 27 slots), the Laser's Joules from cables,
// cubes and Forge Energy through its back, the amplifier's Forge Energy out of every side, the amplifier's redstone
// output, their screens (`GuiLaserAmplifier`, `GuiLaserTractorBeam`), `/mek laser` (the amplifier's settings, which
// Mekanism's screen sets in text fields) and `/mek probe`.

use super::{
    Game,
    mek_ui::{self, Pen, Rect},
    mek_world::{self, key},
    native_hooks::{NativeFeInfo, NativeScreen},
};
use horizon_client::{
    containers::{ModBlockEntity, ModContainer},
    hud::HudBatch,
    items::ItemStack,
};
use horizon_core::geometry::Direction as Face;
use horizon_world::nbt::{Compound, Tag};
use mekanism_energy::{
    config::{self, EnergyConfig},
    container::{Automation, EnergyContainer},
    display,
    fe::{self, Conversion, StrictEnergy},
    laser::{Amplifier, Beam, Fired, Laser, LaserConfig, LaserKind, RedstoneControl, RedstoneOutput, TRACTOR_SLOTS},
    sides::Direction,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::HashMap,
    time::{Duration, Instant},
};

/// `ItemAtomicDisassembler.fullyChargedStack`: what the beam's fake player breaks with (the drops' tool).
const TOOL: &str = "mekanism:atomic_disassembler";

/// What a laser keeps between ticks and doesn't save: its digging, the amplifier's delay count, whether it was
/// powered on the tick before (`redstoneLastTick`), its redstone level, and the last tick's beam.
#[derive(Clone, Copy, Debug, Default)]
struct Live {
    digging: Option<[i32; 3]>,
    progress: u64,
    ticks: i32,
    powered: bool,
    level: i32,
    fired: Fired,
}

thread_local! {
    static LIVE: RefCell<HashMap<(String, [i32; 3]), Live>> = RefCell::new(HashMap::new());
    static SETTINGS: RefCell<Option<(LaserConfig, config::Stamp, Instant)>> = const { RefCell::new(None) };
}

/// `general.toml`, `machine-usage.toml` and `machine-storage.toml`, read again within a second of a change.
fn settings() -> LaserConfig {
    let dir = Game::native_config_dir().ok();
    SETTINGS.with(|s| {
        let mut s = s.borrow_mut();
        let stale = match s.as_mut() {
            None => true,
            Some((_, stamp, looked)) if looked.elapsed() >= Duration::from_secs(1) => {
                *looked = Instant::now();
                dir.as_ref().is_some_and(|d| config::stamp(d) != *stamp)
            }
            Some(_) => false,
        };
        if stale {
            *s = Some(match &dir {
                Some(d) => (LaserConfig::load(d), config::stamp(d), Instant::now()),
                None => (LaserConfig::default(), [None; config::FILES.len()], Instant::now()),
            });
        }
        s.as_ref().map(|(c, _, _)| *c).unwrap_or_default()
    })
}

pub(super) fn kind_of(block: &str) -> Option<LaserKind> {
    LaserKind::of_block(block)
}

pub(super) fn forget(dimension: &str, pos: [i32; 3]) {
    LIVE.with(|l| l.borrow_mut().remove(&(dimension.to_owned(), pos)));
}

pub(super) fn clear() {
    LIVE.with(|l| l.borrow_mut().clear());
}

// ---- The saved compound ---------------------------------------------------------------------

fn read_energy(nbt: &Compound) -> u64 {
    if let Some(Tag::List(list)) = nbt.get(&key("energy_containers"))
        && let Some(Tag::Compound(c)) = list.iter().find(|e| matches!(e, Tag::Compound(c) if mek_world::number(c.get(&key("container"))).unwrap_or(0) == 0))
    {
        return mek_world::number(c.get(&key("stored"))).unwrap_or(0).max(0) as u64;
    }
    0
}

fn write_energy(nbt: &mut Compound, energy: u64) {
    let mut c = Compound::new();
    c.insert(key("container"), Tag::Byte(0));
    if energy > 0 {
        c.insert(key("stored"), Tag::Long(energy.min(i64::MAX as u64) as i64));
    }
    nbt.insert(key("energy_containers"), Tag::List(vec![Tag::Compound(c)]));
}

fn int(nbt: &Compound, name: &str) -> Option<i64> {
    mek_world::number(nbt.get(&key(name)))
}

/// `readSustainedData`: `min` and `max` (capped at the store, through `updateMinThreshold` and
/// `updateMaxThreshold`), `time`, `output_mode`; `control_type` from `TileEntityMekanism`.
fn read_amplifier(nbt: &Compound, capacity: u64) -> Amplifier {
    let mut a = Amplifier::new(capacity);
    if let Some(v) = int(nbt, "min") {
        a.min = (v.max(0) as u64).min(capacity);
    }
    if let Some(v) = int(nbt, "max") {
        a.max = (v.max(0) as u64).min(capacity);
    }
    if let Some(v) = int(nbt, "time") {
        a.delay = v.clamp(0, i64::from(i32::MAX)) as i32;
    }
    if let Some(v) = int(nbt, "output_mode") {
        a.output = RedstoneOutput::by_id(v as i32);
    }
    if let Some(v) = int(nbt, "control_type") {
        a.control = RedstoneControl::by_id(v as i32);
    }
    a
}

fn write_amplifier(nbt: &mut Compound, a: &Amplifier) {
    nbt.insert(key("min"), Tag::Long(a.min.min(i64::MAX as u64) as i64));
    nbt.insert(key("max"), Tag::Long(a.max.min(i64::MAX as u64) as i64));
    nbt.insert(key("time"), Tag::Int(a.delay));
    nbt.insert(key("output_mode"), Tag::Int(a.output.id()));
    nbt.insert(key("control_type"), Tag::Int(a.control as i32));
}

fn facing_of(nbt: &Compound) -> Direction {
    mek_world::generator_facing(nbt)
}

// ---- Block entities ----------------------------------------------------------------------------

/// `native_block_entity`: an empty laser (an amplifier with its new settings).
pub(super) fn new_entity(kind: LaserKind) -> (String, Compound) {
    let mut nbt = Compound::new();
    nbt.insert(key("facing"), Tag::String(key(Direction::North.name())));
    write_energy(&mut nbt, 0);
    if kind == LaserKind::Amplifier {
        write_amplifier(&mut nbt, &Amplifier::new(settings().capacity(kind)));
    }
    (kind.block().to_owned(), nbt)
}

/// `native_block_entity_container`: the tractor beam's 27 output slots, which hoppers and pipes take from on every
/// face; the amplifier's screen has no slots (a hopper menu none of whose slots shows or takes anything). The Laser
/// has no screen.
pub(super) fn container(kind: LaserKind) -> Option<ModContainer> {
    match kind {
        LaserKind::Laser => None,
        LaserKind::Amplifier => Some(ModContainer { slots: 5, faces: std::array::from_fn(|_| vec![]), inputs: vec![], outputs: vec![], menu: Some("minecraft:hopper".into()) }),
        LaserKind::TractorBeam => {
            let all: Vec<usize> = (0..TRACTOR_SLOTS).collect();
            Some(ModContainer { slots: TRACTOR_SLOTS, faces: std::array::from_fn(|_| all.clone()), inputs: vec![], outputs: all, menu: Some("minecraft:generic_9x3".into()) })
        }
    }
}

/// `native_block_entity_slot_accepts`: nothing goes in from outside (`OutputInventorySlot`).
pub(super) fn slot_accepts(accepts: &mut bool) {
    *accepts = false;
}

/// The beam's world: the level, and the tractor beam's slots for what it breaks.
struct World<'a, 'g> {
    game: &'a mut Game<'g>,
    kind: LaserKind,
    pos: [i32; 3],
    facing: Direction,
    items: &'a mut Vec<Option<ItemStack>>,
    config: LaserConfig,
}

impl Beam for World<'_, '_> {
    fn clip(&mut self, from: [f64; 3], to: [f64; 3]) -> Option<([i32; 3], [f64; 3])> {
        self.game.native_clip_blocks(from, to).map(|(at, _, point)| (at, point))
    }

    fn receptor(&mut self, pos: [i32; 3]) -> bool {
        kind_of(&self.game.native_block(pos)).is_some_and(LaserKind::receives) && self.game.native_with_mod_block_entity(pos, |_| ()).is_some()
    }

    /// `receiveLaserEnergy`: `insert(EXECUTE, INTERNAL)` into its store; what doesn't fit is lost.
    fn give(&mut self, pos: [i32; 3], joules: u64) {
        let config = self.config;
        self.game.native_with_mod_block_entity(pos, |m| {
            let Some(kind) = kind_of(&m.id) else { return };
            let mut store = EnergyContainer::basic(config.capacity(kind));
            store.stored = read_energy(&m.nbt).min(store.capacity);
            let before = store.stored;
            store.insert(joules, false, Automation::Internal);
            if store.stored != before {
                write_energy(&mut m.nbt, store.stored);
            }
        });
    }

    fn hardness(&mut self, pos: [i32; 3]) -> Option<f32> {
        self.game.native_destroy_speed(pos)
    }

    /// `withFakePlayer` then `handleBreakBlock`: the block breaks as with a fully charged Atomic Disassembler; its
    /// drops pop where it was, or go into the tractor beam's slots, what doesn't fit popping two blocks in front of
    /// it (`popResourceFromFace`).
    fn break_block(&mut self, pos: [i32; 3]) {
        let tool = ItemStack::new(TOOL, 1);
        if self.kind != LaserKind::TractorBeam {
            self.game.native_machine_destroy_block(pos, Some(&tool), false);
            return;
        }
        let Some(drops) = self.game.native_machine_destroy_block(pos, Some(&tool), true) else { return };
        let n = self.facing.offset();
        let out = [self.pos[0] + 2 * n[0], self.pos[1] + 2 * n[1], self.pos[2] + 2 * n[2]];
        for stack in drops {
            let rest = insert_into(self.game, self.items, stack);
            if rest.count > 0 {
                self.game.native_pop_resource(out, rest);
            }
        }
    }
}

/// `InventoryUtils.insertItem` over the 27 slots: stacks of the same item first, then empty slots.
fn insert_into(game: &Game<'_>, items: &mut Vec<Option<ItemStack>>, mut stack: ItemStack) -> ItemStack {
    if items.len() < TRACTOR_SLOTS {
        items.resize(TRACTOR_SLOTS, None);
    }
    let max = game.native_max_stack_size(&stack.item).unwrap_or(64).clamp(1, 64);
    for pass in 0..2 {
        for cell in items.iter_mut().take(TRACTOR_SLOTS) {
            if stack.count == 0 {
                return stack;
            }
            let room = match cell {
                Some(h) if pass == 0 && super::mek_machines::same_kind(h, &stack) => max.saturating_sub(h.count),
                None if pass == 1 => max,
                _ => 0,
            };
            let moved = room.min(stack.count);
            if moved == 0 {
                continue;
            }
            *cell = Some(match cell.take() {
                Some(h) => ItemStack { count: h.count + moved, ..h },
                None => ItemStack { count: moved, ..stack.clone() },
            });
            stack.count -= moved;
        }
    }
    stack
}

/// `native_block_entity_tick`: `TileEntityBasicLaser.onUpdateServer` (the amplifier's delay count and redstone
/// control first), then the amplifier's redstone level, which tells its neighbours when it changes.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, kind: LaserKind) {
    let config = settings();
    let capacity = config.capacity(kind);
    let dimension = game.native_dimension().to_owned();
    let facing = game.native_block_property(pos, "facing").and_then(|f| Direction::from_name(&f)).unwrap_or(Direction::North);
    if facing_of(&entity.nbt) != facing || !entity.nbt.contains_key(&key("facing")) {
        entity.nbt.insert(key("facing"), Tag::String(key(facing.name())));
    }
    let id = (dimension.clone(), pos);
    let mut live = LIVE.with(|l| l.borrow_mut().remove(&id)).unwrap_or_default();
    let before = (read_energy(&entity.nbt), int(&entity.nbt, "last_fired").unwrap_or(0).max(0) as u64);
    let was_active = kind.activates() && int(&entity.nbt, "active_state").unwrap_or(0) != 0;
    let mut l = Laser { energy: before.0.min(capacity), last_fired: before.1, activates: kind.activates(), active: was_active, digging: live.digging, progress: live.progress };
    let mut amp = (kind == LaserKind::Amplifier).then(|| read_amplifier(&entity.nbt, capacity));
    let to_fire = match (kind, amp.as_mut()) {
        (LaserKind::Laser, _) => config.usage,
        (LaserKind::Amplifier, Some(a)) => {
            let powered = game.native_neighbor_signal(&dimension, pos) > 0;
            let can = a.control.allows(powered, live.powered);
            live.powered = powered;
            a.ticks = live.ticks;
            let f = a.to_fire(l.energy, can);
            live.ticks = a.ticks;
            f
        }
        _ => u64::MAX,
    };
    let fired = {
        let mut world = World { game: &mut *game, kind, pos, facing, items: &mut entity.items, config };
        l.tick(pos, facing, to_fire, &config, &mut world)
    };
    live.digging = l.digging;
    live.progress = l.progress;
    live.fired = fired;
    if (l.energy, l.last_fired) != before {
        write_energy(&mut entity.nbt, l.energy);
        entity.nbt.insert(key("last_fired"), Tag::Long(l.last_fired.min(i64::MAX as u64) as i64));
    }
    if kind.activates() && (l.active != was_active || !entity.nbt.contains_key(&key("active_state"))) {
        entity.nbt.insert(key("active_state"), Tag::Byte(i8::from(l.active)));
    }
    let mut data = Vec::with_capacity(DATA);
    let mut signal_changed = false;
    push(&mut data, l.energy, 4);
    push(&mut data, capacity, 4);
    if let Some(a) = amp {
        if !entity.nbt.contains_key(&key("time")) {
            write_amplifier(&mut entity.nbt, &a);
        }
        // Entities in the beam aren't looked at: `ENTITY_DETECTION` gives nothing.
        let level = a.redstone_level(l.energy, capacity, false);
        if level != live.level {
            live.level = level;
            signal_changed = true;
        }
        push(&mut data, a.min, 4);
        push(&mut data, a.max, 4);
        push(&mut data, a.delay.max(0) as u64, 2);
        push(&mut data, a.output.id() as u64, 1);
        push(&mut data, a.control as u64, 1);
    }
    entity.data = data;
    LIVE.with(|l| l.borrow_mut().insert(id, live));
    // After the level is kept: the neighbours read it back through `block_signal`.
    if signal_changed {
        game.native_update_signal_neighbors(&dimension, pos);
    }
}

// ---- Redstone (API 38) ------------------------------------------------------------------------

/// `native_is_signal_source`: the amplifier (`AttributeRedstoneEmitter`).
pub(super) fn is_signal_source(block: Option<&str>, source: &mut bool) {
    if block == Some(LaserKind::Amplifier.block()) {
        *source = true;
    }
}

/// `native_block_signal`: the amplifier's `getRedstoneLevel` to every side, none through blocks.
pub(super) fn block_signal(dimension: &str, block: Option<&str>, pos: [i32; 3], direct: bool, signal: &mut i32) {
    if block != Some(LaserKind::Amplifier.block()) {
        return;
    }
    *signal = if direct { 0 } else { LIVE.with(|l| l.borrow().get(&(dimension.to_owned(), pos)).map_or(0, |v| v.level)) };
}

// ---- Energy from outside --------------------------------------------------------------------

/// Whether the laser in `entity` takes energy through `face`: the Laser its back.
pub(super) fn takes_energy(entity: &ModBlockEntity, face: Direction) -> Option<bool> {
    kind_of(&entity.id).map(|k| k.takes_energy(facing_of(&entity.nbt), face))
}

/// Offers `amount` Joules through `face`: what it didn't take.
pub(super) fn offer(entity: &mut ModBlockEntity, face: Direction, amount: u64, simulate: bool) -> Option<u64> {
    let kind = kind_of(&entity.id)?;
    if !kind.takes_energy(facing_of(&entity.nbt), face) {
        return Some(amount);
    }
    let mut store = EnergyContainer::basic(settings().capacity(kind));
    store.stored = read_energy(&entity.nbt).min(store.capacity);
    let before = store.stored;
    let rest = store.insert(amount, simulate, Automation::External);
    if !simulate && store.stored != before {
        write_energy(&mut entity.nbt, store.stored);
    }
    Some(rest)
}

/// Its store as a side reaches it: the Laser takes through its back, the amplifier gives on every side.
struct Side {
    container: EnergyContainer,
    take: bool,
    give: bool,
    taken: u64,
}

impl StrictEnergy for Side {
    fn insert(&mut self, joules: i64, simulate: bool) -> i64 {
        if !self.take || joules <= 0 {
            return joules;
        }
        let rest = self.container.insert(joules as u64, simulate, Automation::External);
        if !simulate {
            self.taken += joules as u64 - rest;
        }
        rest as i64
    }

    fn extract(&mut self, joules: i64, simulate: bool) -> i64 {
        if !self.give || joules <= 0 {
            return 0;
        }
        self.container.extract(joules as u64, simulate, Automation::External) as i64
    }
}

fn side(entity: &ModBlockEntity, kind: LaserKind, face: Option<Face>) -> Side {
    let mut container = EnergyContainer::basic(settings().capacity(kind));
    container.stored = read_energy(&entity.nbt).min(container.capacity);
    let take = face.is_some_and(|f| kind.takes_energy(facing_of(&entity.nbt), mek_world::mek_direction(f)));
    Side { container, take, give: kind.gives_energy(), taken: 0 }
}

/// `native_fe_storage`: the Laser's store through its back, the amplifier's through every side.
pub(super) fn fe_storage(entity: &ModBlockEntity, face: Option<Face>, config: &EnergyConfig, info: &mut Option<NativeFeInfo>) -> bool {
    let Some(kind) = kind_of(&entity.id) else { return false };
    let s = side(entity, kind, face);
    if config.blacklist_forge || face.is_some() && !s.take && !s.give || kind == LaserKind::TractorBeam {
        return true;
    }
    let rate = Conversion(config.fe_conversion_rate);
    *info = Some(NativeFeInfo {
        stored: i64::from(rate.to_fe_int(s.container.stored.min(i64::MAX as u64) as i64)),
        capacity: i64::from(rate.to_fe_int(s.container.capacity.min(i64::MAX as u64) as i64)),
        can_receive: s.take,
        can_extract: s.give,
    });
    true
}

/// `native_fe_receive` and `native_fe_extract`.
#[allow(clippy::too_many_arguments)]
pub(super) fn fe_transfer(pos: [i32; 3], entity: &mut ModBlockEntity, face: Option<Face>, config: &EnergyConfig, max: i64, simulate: bool, extract: bool, moved: &mut Option<i64>) -> bool {
    let Some(kind) = kind_of(&entity.id) else { return false };
    let mut s = side(entity, kind, face);
    if config.blacklist_forge {
        return true;
    }
    let before = s.container.stored;
    let rate = Conversion(config.fe_conversion_rate);
    let max = max.clamp(0, i64::from(i32::MAX)) as i32;
    let fe = if extract { fe::extract_fe(&mut s, rate, max, simulate) } else { fe::receive_fe(&mut s, rate, max, simulate) };
    if !simulate && s.container.stored != before {
        write_energy(&mut entity.nbt, s.container.stored);
    }
    if s.taken > 0 {
        mek_world::add_received(pos, s.taken);
    }
    *moved = Some(i64::from(fe));
    true
}

// ---- /mek laser -------------------------------------------------------------------------------

/// `/mek laser x y z min|max <J> | delay <ticks> | output next|previous|<0-2> | control <0-3>`: an amplifier's
/// settings, as its screen's text fields and buttons set them (`setMinThresholdFromPacket`, ...).
pub(super) fn command(game: &mut Game<'_>, pos: [i32; 3], words: &[&str]) -> Value {
    let capacity = settings().capacity(LaserKind::Amplifier);
    let done = game.native_with_mod_block_entity(pos, |m| {
        if kind_of(&m.id) != Some(LaserKind::Amplifier) {
            return None;
        }
        let mut a = read_amplifier(&m.nbt, capacity);
        let n = words.get(1).and_then(|w| w.parse::<i64>().ok());
        match (words.first().copied(), n) {
            (Some("min"), Some(v)) => {
                a.set_min(v.max(0) as u64, capacity);
            }
            (Some("max"), Some(v)) => {
                a.set_max(v.max(0) as u64, capacity);
            }
            (Some("delay"), Some(v)) => {
                a.set_delay(v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32);
            }
            (Some("output"), Some(v)) => a.output = RedstoneOutput::by_id(v as i32),
            (Some("output"), None) if words.get(1) == Some(&"next") => a.output = a.output.next(),
            (Some("output"), None) if words.get(1) == Some(&"previous") => a.output = a.output.previous(),
            (Some("control"), Some(v)) => a.control = RedstoneControl::by_id(v as i32),
            _ => return Some(Err(())),
        }
        write_amplifier(&mut m.nbt, &a);
        Some(Ok(a))
    });
    match done.flatten() {
        Some(Ok(a)) => json!({ "pos": pos, "min": a.min, "max": a.max, "delay": a.delay, "output": a.output.id(), "control": a.control as i32 }),
        Some(Err(())) => json!({ "pos": pos, "error": "usage: /mek laser <x> <y> <z> min|max <joules> | delay <ticks> | output next|previous|<0-2> | control <0-3>" }),
        None => json!({ "pos": pos, "error": "not a laser amplifier" }),
    }
}

/// `native_menu_button`: the amplifier's output mode button, 0 on to the next mode, 1 back to the previous one.
pub(super) fn button(entity: &mut ModBlockEntity, id: i32) {
    if kind_of(&entity.id) != Some(LaserKind::Amplifier) {
        return;
    }
    let mut a = read_amplifier(&entity.nbt, settings().capacity(LaserKind::Amplifier));
    a.output = if id == 1 { a.output.previous() } else { a.output.next() };
    write_amplifier(&mut entity.nbt, &a);
}

// ---- The screens (`GuiLaserAmplifier`, `GuiLaserTractorBeam`) -----------------------------------------------------

const DATA: usize = 4 * 4 + 2 + 1 + 1;

fn push(out: &mut Vec<i32>, v: u64, words: usize) {
    out.extend((0..words).map(|i| ((v >> (16 * i)) & 0xFFFF) as i32));
}

fn word(data: &[i32], at: usize, words: usize) -> u64 {
    (0..words).fold(0u64, |v, i| v | (u64::from(data.get(at + i).copied().unwrap_or(0) as u16) << (16 * i)))
}

struct View {
    energy: u64,
    capacity: u64,
    min: u64,
    max: u64,
    delay: u64,
    output: RedstoneOutput,
}

fn unpack(data: &[i32]) -> Option<View> {
    (data.len() >= 8).then(|| View {
        energy: word(data, 0, 4),
        capacity: word(data, 4, 4),
        min: word(data, 8, 4),
        max: word(data, 12, 4),
        delay: word(data, 16, 2),
        output: RedstoneOutput::by_id(word(data, 18, 1) as i32),
    })
}

const SCREEN: Rect = [7.0, 15.0, 150.0, 54.0];
const POWER_BAR: Rect = [164.0, 15.0, 6.0, 54.0];
const OUTPUT_BUTTON: Rect = [140.0, 54.0, 14.0, 14.0];

pub(super) fn screen(kind: LaserKind) -> NativeScreen {
    let slots = match kind {
        LaserKind::TractorBeam => (0..TRACTOR_SLOTS).map(|i| Some([8.0 + 18.0 * (i % 9) as f32, 16.0 + 18.0 * (i / 9) as f32])).collect(),
        _ => vec![None; 5],
    };
    NativeScreen { width: 176.0, height: 166.0, slots, inventory_y: 84.0, inventory_x: 8.0, own_labels: true, own_background: false }
}

pub(super) fn click(kind: LaserKind, at: [f32; 2], right: bool, button: &mut Option<i32>) {
    if kind == LaserKind::Amplifier && mek_ui::inside(OUTPUT_BUTTON, at) {
        *button = Some(i32::from(right));
    }
}

pub(super) fn tooltip(kind: LaserKind, data: &[i32], at: [f32; 2], tips: &mut Vec<String>) {
    let Some(v) = unpack(data) else { return };
    let settings = mek_ui::settings();
    if kind == LaserKind::Amplifier && mek_ui::inside(POWER_BAR, at) {
        tips.push(display::fraction(v.energy, v.capacity, &settings));
    } else if kind == LaserKind::Amplifier && mek_ui::inside(OUTPUT_BUTTON, at) {
        tips.push(format!("Redstone Output: {}", v.output.label()));
    }
}

pub(super) fn draw(game: &Game<'_>, kind: LaserKind, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let settings = mek_ui::settings();
    let cursor = game.native_cursor();
    let cursor = [cursor[0] - origin[0], cursor[1] - origin[1]];
    let mut pen = Pen { game, batch, origin, cursor };
    let v = unpack(data);
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [0.0, 0.0, 176.0, 166.0]);
    if kind == LaserKind::TractorBeam {
        for i in 0..TRACTOR_SLOTS {
            pen.slot("normal", None, 7.0 + 18.0 * (i % 9) as f32, 15.0 + 18.0 * (i / 9) as f32);
        }
    } else {
        pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], POWER_BAR);
        if let Some(v) = &v {
            let level = if v.capacity == 0 { 0.0 } else { v.energy as f64 / v.capacity as f64 };
            let h = (level * 52.0) as i32 as f32;
            if h > 0.0 {
                pen.sprite("bar/vertical_power", [0.0, 0.0, 4.0, h], [POWER_BAR[0] + 1.0, POWER_BAR[1] + POWER_BAR[3] - 1.0 - h, 4.0, h]);
            }
        }
        pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], SCREEN);
        if let Some(v) = &v {
            let lines = [
                format!("Stored: {}", display::energy(v.energy, &settings)),
                format!("Min: {}", display::energy(v.min, &settings)),
                format!("Max: {}", display::energy(v.max, &settings)),
                format!("Delay: {}", v.delay),
            ];
            for (i, line) in lines.iter().enumerate() {
                pen.text(line, [SCREEN[0] + 3.0, SCREEN[1] + 3.0 + 12.5 * i as f32], mek_ui::SCREEN_TEXT);
            }
        }
        pen.nine("button/button", [0.0, 0.0, 200.0, 20.0], [2.0, 2.0], OUTPUT_BUTTON);
        let mark = match v.as_ref().map_or(RedstoneOutput::Off, |v| v.output) {
            RedstoneOutput::Off => "-",
            RedstoneOutput::EntityDetection => "E",
            RedstoneOutput::EnergyContents => "J",
        };
        pen.centered(mark, OUTPUT_BUTTON[0], OUTPUT_BUTTON[0] + OUTPUT_BUTTON[2], OUTPUT_BUTTON[1] + 3.0, mek_ui::SCREEN_TEXT);
    }
    for row in 0..4 {
        let y = if row == 3 { 141.0 } else { 83.0 + 18.0 * row as f32 };
        for col in 0..9 {
            pen.slot("normal", None, 7.0 + 18.0 * col as f32, y);
        }
    }
    let fallback = if kind == LaserKind::TractorBeam { "Laser Tractor Beam" } else { "Laser Amplifier" };
    let title = game.native_screen_title().unwrap_or_else(|| fallback.into());
    pen.centered(&title, 0.0, 176.0, 5.0, mek_ui::TITLE);
    pen.text("Inventory", [8.0, 74.0], mek_ui::TITLE);
}

// ---- /mek probe -------------------------------------------------------------------------------------------------

pub(super) fn probe(game: &Game<'_>, pos: [i32; 3], id: &str, nbt: &Compound, items: &[Option<ItemStack>]) -> Value {
    let Some(kind) = kind_of(id) else { return json!({ "pos": pos, "error": "not a laser" }) };
    let config = settings();
    let live = LIVE.with(|l| l.borrow().get(&(game.native_dimension().to_owned(), pos)).copied()).unwrap_or_default();
    let mut out = json!({
        "pos": pos, "id": id, "energy": read_energy(nbt), "capacity": config.capacity(kind),
        "last_fired": int(nbt, "last_fired").unwrap_or(0), "active": int(nbt, "active_state").unwrap_or(0) != 0, "facing": facing_of(nbt).name(),
        "digging": live.digging, "progress": live.progress,
        "fired": live.fired.firing, "length": live.fired.length, "hit": live.fired.hit, "handed": live.fired.handed,
        "broke": live.fired.broke, "usage": config.usage, "per_hardness": config.energy_per_hardness, "range": config.range,
    });
    if kind == LaserKind::Amplifier {
        let a = read_amplifier(nbt, config.capacity(kind));
        out["min"] = json!(a.min);
        out["max"] = json!(a.max);
        out["delay"] = json!(a.delay);
        out["ticks"] = json!(live.ticks);
        out["output"] = json!(a.output.id());
        out["control"] = json!(a.control as i32);
        out["redstone"] = json!(live.level);
    }
    if kind == LaserKind::TractorBeam {
        out["slots"] = items.iter().enumerate().filter_map(|(i, s)| s.as_ref().map(|s| json!({ "slot": i, "item": &*s.item, "count": s.count }))).collect();
    }
    out
}

