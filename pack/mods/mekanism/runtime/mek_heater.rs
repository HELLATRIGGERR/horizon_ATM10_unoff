// Mekanism's Resistive Heater in the game (support module `crate::game::mek_heater` of the `mekanism` native
// bundle). Its rules are the bundle's crate (`mekanism_energy::heater`, MIT, from Mekanism 10.7.19, Copyright (c)
// 2017-2025 Aidan C. Brady); this module is the glue: its block entity in Mekanism's saved form
// (`energy_containers` with its `energy_usage`, `heat_capacitors`, `active_state`), its tick (the energy slot, the
// heat, what it hands the heat handlers beside it: a formed boiler's casings and valves, a formed evaporation
// plant's valves, other resistive heaters), its Joules from cables, cubes and Forge Energy through its left and
// right, its screen (`GuiResistiveHeater`), `/mek heater` (its energy usage, which Mekanism's screen sets in a text
// field) and `/mek probe`.

use super::{
    Game, mek_machines,
    mek_multiblock as mb,
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
    heat::{self, Activity, Ambient},
    heater::{self, Heater, HeaterConfig},
    multiblock::Uuid,
    sides::Direction,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::HashMap,
    time::{Duration, Instant},
};

pub(super) const HEATER: &str = "mekanism:resistive_heater";
const ENERGY_SLOT: usize = 0;

/// What a heater keeps between ticks and doesn't save: the air beside it (`CachedAmbientTemperature`, read once)
/// and the heat another heater handed it.
#[derive(Clone, Copy, Debug, Default)]
struct Live {
    ambient: Option<Ambient>,
    pending: f64,
}

thread_local! {
    static LIVE: RefCell<HashMap<(String, [i32; 3]), Live>> = RefCell::new(HashMap::new());
    static SETTINGS: RefCell<Option<(HeaterConfig, config::Stamp, Instant)>> = const { RefCell::new(None) };
}

/// `general.toml` `[heater]`, read again within a second of a change.
fn settings() -> HeaterConfig {
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
                Some(d) => (HeaterConfig::load(d), config::stamp(d), Instant::now()),
                None => (HeaterConfig::default(), [None; config::FILES.len()], Instant::now()),
            });
        }
        s.as_ref().map(|(c, _, _)| *c).unwrap_or_default()
    })
}

pub(super) fn is_heater(block: &str) -> bool {
    block == HEATER
}

pub(super) fn forget(dimension: &str, pos: [i32; 3]) {
    LIVE.with(|l| l.borrow_mut().remove(&(dimension.to_owned(), pos)));
}

pub(super) fn clear() {
    LIVE.with(|l| l.borrow_mut().clear());
}

// ---- The saved compound ---------------------------------------------------------------------

/// `energy_containers: [{container: 0b, stored: 4000L, energy_usage: 100L}]` (`ResistiveHeaterEnergyContainer`
/// always saves its usage), `heat_capacitors: [{container: 0b, stored: 30000.0d, heat_capacity: 100.0d}]`.
fn read(nbt: &Compound) -> Heater {
    let mut h = Heater::default();
    if let Some(Tag::List(list)) = nbt.get(&key("energy_containers"))
        && let Some(Tag::Compound(c)) = list.iter().find(|e| matches!(e, Tag::Compound(c) if mek_world::number(c.get(&key("container"))).unwrap_or(0) == 0))
    {
        if let Some(usage) = mek_world::number(c.get(&key("energy_usage"))) {
            h.usage = usage.max(0) as u64;
        }
        h.energy = (mek_world::number(c.get(&key("stored"))).unwrap_or(0).max(0) as u64).min(h.capacity());
    }
    if let Some(Tag::List(list)) = nbt.get(&key("heat_capacitors"))
        && let Some(Tag::Compound(c)) = list.first()
        && let Some(Tag::Double(stored)) = c.get(&key("stored"))
    {
        h.heat = Some(f64::from_bits(*stored));
    }
    h
}

fn write(nbt: &mut Compound, h: &Heater) {
    let mut container = Compound::new();
    container.insert(key("container"), Tag::Byte(0));
    if h.energy > 0 {
        container.insert(key("stored"), Tag::Long(h.energy.min(i64::MAX as u64) as i64));
    }
    container.insert(key("energy_usage"), Tag::Long(h.usage.min(i64::MAX as u64) as i64));
    nbt.insert(key("energy_containers"), Tag::List(vec![Tag::Compound(container)]));
    if let Some(stored) = h.heat {
        let mut capacitor = Compound::new();
        capacitor.insert(key("container"), Tag::Byte(0));
        capacitor.insert(key("stored"), Tag::Double(stored.to_bits()));
        capacitor.insert(key("heat_capacity"), Tag::Double(heater::HEAT_CAPACITY.to_bits()));
        nbt.insert(key("heat_capacitors"), Tag::List(vec![Tag::Compound(capacitor)]));
    }
}

// ---- Block entities ----------------------------------------------------------------------------

/// `native_block_entity`: a fresh heater facing north, using 100 J/t.
pub(super) fn new_entity() -> (String, Compound) {
    let mut nbt = Compound::new();
    nbt.insert(key("facing"), Tag::String(key(Direction::North.name())));
    write(&mut nbt, &Heater::default());
    (HEATER.to_owned(), nbt)
}

/// `native_block_entity_container`: its energy slot, which hoppers reach from every face.
pub(super) fn container() -> ModContainer {
    ModContainer { slots: 5, faces: std::array::from_fn(|_| vec![ENERGY_SLOT]), inputs: vec![ENERGY_SLOT], outputs: vec![ENERGY_SLOT], menu: Some("minecraft:hopper".into()) }
}

/// `native_block_entity_slot_accepts`: an energy cube's item or what converts to Joules.
pub(super) fn slot_accepts(slot: usize, stack: &ItemStack, accepts: &mut bool) {
    let ok = slot == ENERGY_SLOT && (mek_machines::energy_item(&stack.item) || mekanism_energy::tier::CubeTier::of_block(&stack.item).is_some());
    if !ok {
        *accepts = false;
    }
}

/// Who takes the heat a heater hands through a side.
#[derive(Clone, Copy)]
enum Sink {
    None,
    Multiblock(Uuid),
    Heater([i32; 3]),
}

/// `native_block_entity_tick`: `tickServer`'s activity delay, then `onUpdateServer` (the energy slot, the Joules into
/// heat, `simulate` against the heat handlers beside it and the air) and `updateHeatCapacitors`. Redstone control is
/// IGNORED.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let config = settings();
    let (energy, _) = mek_world::energy_config();
    mek_machines::refresh_book(game);
    let dimension = game.native_dimension().to_owned();
    let facing = game.native_block_property(pos, "facing").and_then(|f| Direction::from_name(&f)).unwrap_or(Direction::North);
    if mek_world::generator_facing(&entity.nbt) != facing || !entity.nbt.contains_key(&key("facing")) {
        entity.nbt.insert(key("facing"), Tag::String(key(facing.name())));
    }
    let id = (dimension.clone(), pos);
    let mut live = LIVE.with(|l| l.borrow_mut().remove(&id)).unwrap_or_default();
    let mut activity = mek_world::read_activity(&entity.nbt);
    let mut shown = game.native_block_property(pos, "active").as_deref() == Some("true");
    let changed = activity.tick(shown);
    mek_world::show_active(game, pos, changed, &mut shown);

    let before = read(&entity.nbt);
    let mut h = before;
    h.pending = live.pending;
    // The energy slot (`fillContainerOrConvert`): an energy cube's item, else an item that converts to Joules.
    let mut store = EnergyContainer::basic(h.capacity());
    store.stored = h.energy;
    let mut item = mek_machines::stack_in(entity, ENERGY_SLOT).cloned();
    if let Some(cube) = item.as_mut()
        && store.needed() > 0
        && let Some(taken) = mek_world::drain_energy_item(cube, store.needed()).filter(|t| *t > 0)
    {
        store.insert(taken, false, Automation::Internal);
        if let Some(cell) = entity.items.get_mut(ENERGY_SLOT) {
            *cell = item;
        }
    } else if let Some(stack) = mek_machines::stack_in(entity, ENERGY_SLOT).cloned()
        && store.needed() > 0
        && let Some((count, joules)) = mek_machines::energy_conversion(&stack)
        && stack.count >= count
        && store.needed() >= joules
    {
        store.insert(joules, false, Automation::Manual);
        mek_machines::shrink(entity, ENERGY_SLOT, count);
    }
    h.energy = store.stored;

    let ambient = *live.ambient.get_or_insert_with(|| {
        let air = |p: [i32; 3]| game.native_biome_climate(p).map(|b| heat::ambient_temperature(b.temperature));
        let own = air(pos).unwrap_or(heat::AMBIENT_TEMP);
        Ambient { own, sides: Direction::ALL.map(|d| air(mek_world::beside(pos, d)).unwrap_or(own)) }
    });
    let mut sinks = [None; 6];
    let mut takers = [Sink::None; 6];
    for (i, d) in Direction::ALL.into_iter().enumerate() {
        let at = mek_world::beside(pos, d);
        let block = game.native_block(at);
        if let Some((owner, inverse)) = mb::heat_sink(&dimension, at, &block) {
            sinks[i] = Some(inverse);
            takers[i] = Sink::Multiblock(owner);
        } else if block == HEATER && game.native_with_mod_block_entity(at, |_| ()).is_some() {
            sinks[i] = Some(heater::INVERSE_CONDUCTION);
            takers[i] = Sink::Heater(at);
        }
    }
    let made = h.tick(&config, &ambient, sinks);
    for (i, taker) in takers.iter().enumerate() {
        match *taker {
            Sink::None => {}
            Sink::Multiblock(owner) => mb::hand_heat(owner, made.handed[i]),
            Sink::Heater(at) => LIVE.with(|l| l.borrow_mut().entry((dimension.clone(), at)).or_default().pending += made.handed[i]),
        }
    }
    live.pending = h.pending;
    let changed = activity.set(made.used > 0, shown, energy.deactivation_delay);
    mek_world::show_active(game, pos, changed, &mut shown);
    if (h.energy, h.usage, h.heat) != (before.energy, before.usage, before.heat) || !entity.nbt.contains_key(&key("heat_capacitors")) {
        write(&mut entity.nbt, &h);
    }
    if activity != mek_world::read_activity(&entity.nbt) || !entity.nbt.contains_key(&key("active_state")) {
        mek_world::write_activity(&mut entity.nbt, activity);
    }
    let mut data = Vec::with_capacity(DATA);
    push(&mut data, h.energy, 4);
    push(&mut data, h.capacity(), 4);
    push(&mut data, h.usage, 4);
    push(&mut data, made.used, 4);
    for v in [h.temperature(&ambient), made.transfer_loss, made.environment_loss] {
        push(&mut data, u64::from((v as f32).to_bits()), 2);
    }
    entity.data = data;
    LIVE.with(|l| l.borrow_mut().insert(id, live));
}

// ---- Energy from outside --------------------------------------------------------------------

fn facing(entity: &ModBlockEntity) -> Direction {
    mek_world::generator_facing(&entity.nbt)
}

/// Whether the heater in `entity` takes energy through `face`: its left and right.
pub(super) fn takes_energy(entity: &ModBlockEntity, face: Direction) -> Option<bool> {
    is_heater(&entity.id).then(|| heater::takes_energy(facing(entity), face))
}

/// Offers `amount` Joules through `face`: what it didn't take.
pub(super) fn offer(entity: &mut ModBlockEntity, face: Direction, amount: u64, simulate: bool) -> Option<u64> {
    if !is_heater(&entity.id) {
        return None;
    }
    if !heater::takes_energy(facing(entity), face) {
        return Some(amount);
    }
    let mut h = read(&entity.nbt);
    let mut container = EnergyContainer::basic(h.capacity());
    container.stored = h.energy;
    let rest = container.insert(amount, simulate, Automation::External);
    if !simulate && container.stored != h.energy {
        h.energy = container.stored;
        write(&mut entity.nbt, &h);
    }
    Some(rest)
}

/// Its store as a side reaches it: it takes, and gives nothing (`notExternal`).
struct Side {
    container: EnergyContainer,
    open: bool,
    taken: u64,
}

impl StrictEnergy for Side {
    fn insert(&mut self, joules: i64, simulate: bool) -> i64 {
        if !self.open || joules <= 0 {
            return joules;
        }
        let rest = self.container.insert(joules as u64, simulate, Automation::External);
        if !simulate {
            self.taken += joules as u64 - rest;
        }
        rest as i64
    }

    fn extract(&mut self, _joules: i64, _simulate: bool) -> i64 {
        0
    }
}

/// `native_fe_storage`: its store in whole FE through its left and right.
pub(super) fn fe_storage(entity: &ModBlockEntity, face: Option<Face>, config: &EnergyConfig, info: &mut Option<NativeFeInfo>) -> bool {
    if !is_heater(&entity.id) {
        return false;
    }
    if config.blacklist_forge || face.is_some_and(|f| !heater::takes_energy(facing(entity), mek_world::mek_direction(f))) {
        return true;
    }
    let h = read(&entity.nbt);
    let rate = Conversion(config.fe_conversion_rate);
    *info = Some(NativeFeInfo {
        stored: i64::from(rate.to_fe_int(h.energy.min(i64::MAX as u64) as i64)),
        capacity: i64::from(rate.to_fe_int(h.capacity().min(i64::MAX as u64) as i64)),
        can_receive: true,
        can_extract: true,
    });
    true
}

/// `native_fe_receive` and `native_fe_extract`.
#[allow(clippy::too_many_arguments)]
pub(super) fn fe_transfer(pos: [i32; 3], entity: &mut ModBlockEntity, face: Option<Face>, config: &EnergyConfig, max: i64, simulate: bool, extract: bool, moved: &mut Option<i64>) -> bool {
    if !is_heater(&entity.id) {
        return false;
    }
    let open = face.is_some_and(|f| heater::takes_energy(facing(entity), mek_world::mek_direction(f)));
    if config.blacklist_forge || face.is_some() && !open {
        return true;
    }
    let mut h = read(&entity.nbt);
    let mut container = EnergyContainer::basic(h.capacity());
    container.stored = h.energy;
    let mut side = Side { container, open, taken: 0 };
    let rate = Conversion(config.fe_conversion_rate);
    let max = max.clamp(0, i64::from(i32::MAX)) as i32;
    let fe = if extract { fe::extract_fe(&mut side, rate, max, simulate) } else { fe::receive_fe(&mut side, rate, max, simulate) };
    if !simulate && side.container.stored != h.energy {
        h.energy = side.container.stored;
        write(&mut entity.nbt, &h);
    }
    if side.taken > 0 {
        mek_world::add_received(pos, side.taken);
    }
    *moved = Some(i64::from(fe));
    true
}

/// `/mek heater x y z <usage>`: its energy usage (`setEnergyUsageFromPacket`, what its screen's text field sends):
/// its store holds 400 ticks of it, and what it held beyond that is lost.
pub(super) fn set_usage(game: &mut Game<'_>, pos: [i32; 3], usage: u64) -> Value {
    let done = game.native_with_mod_block_entity(pos, |m| {
        if !is_heater(&m.id) {
            return None;
        }
        let mut h = read(&m.nbt);
        h.usage = usage;
        h.energy = h.energy.min(h.capacity());
        write(&mut m.nbt, &h);
        Some((h.usage, h.capacity(), h.energy))
    });
    match done.flatten() {
        Some((usage, capacity, energy)) => json!({ "pos": pos, "usage": usage, "capacity": capacity, "energy": energy }),
        None => json!({ "pos": pos, "error": "not a resistive heater" }),
    }
}

// ---- The screen (`GuiResistiveHeater`) -------------------------------------------------------------------------

const DATA: usize = 4 * 4 + 3 * 2;

fn push(out: &mut Vec<i32>, v: u64, words: usize) {
    out.extend((0..words).map(|i| ((v >> (16 * i)) & 0xFFFF) as i32));
}

fn read_word(data: &[i32], at: usize, words: usize) -> u64 {
    (0..words).fold(0u64, |v, i| v | (u64::from(data[at + i] as u16) << (16 * i)))
}

struct View {
    energy: u64,
    capacity: u64,
    usage: u64,
    used: u64,
    temperature: f64,
    transfer: f64,
    loss: f64,
}

fn unpack(data: &[i32]) -> Option<View> {
    (data.len() >= DATA).then(|| {
        let float = |at: usize| f64::from(f32::from_bits(read_word(data, at, 2) as u32));
        View {
            energy: read_word(data, 0, 4),
            capacity: read_word(data, 4, 4),
            usage: read_word(data, 8, 4),
            used: read_word(data, 12, 4),
            temperature: float(16),
            transfer: float(18),
            loss: float(20),
        }
    })
}

const SCREEN: Rect = [48.0, 23.0, 80.0, 42.0];
const POWER_BAR: Rect = [164.0, 15.0, 6.0, 54.0];
const HEAT_TAB: Rect = [-26.0, 109.0, 26.0, 26.0];
const ENERGY_SLOT_AT: [f32; 2] = [15.0, 35.0];

pub(super) fn screen() -> NativeScreen {
    NativeScreen { width: 176.0, height: 166.0, slots: vec![Some(ENERGY_SLOT_AT), None, None, None, None], inventory_y: 84.0, inventory_x: 8.0, own_labels: true, own_background: false }
}

pub(super) fn tooltip(data: &[i32], at: [f32; 2], tips: &mut Vec<String>) {
    let Some(v) = unpack(data) else { return };
    let settings = mek_ui::settings();
    if mek_ui::inside(POWER_BAR, at) {
        tips.push(display::fraction(v.energy, v.capacity, &settings));
    } else if mek_ui::inside(mek_ui::ENERGY_TAB, at) {
        tips.push(format!("Using: {}/t", display::energy(v.used, &settings)));
        tips.push(format!("Needed: {}", display::energy(v.capacity.saturating_sub(v.energy), &settings)));
    } else if mek_ui::inside(HEAT_TAB, at) {
        tips.push(format!("Temp: {}", display::temperature(v.temperature)));
        tips.push(format!("Transferred: {}/t", display::temperature(v.transfer)));
        tips.push(format!("Dissipated: {}/t", display::temperature(v.loss)));
    }
}

pub(super) fn draw(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let settings = mek_ui::settings();
    let cursor = game.native_cursor();
    let cursor = [cursor[0] - origin[0], cursor[1] - origin[1]];
    let mut pen = Pen { game, batch, origin, cursor };
    let v = unpack(data);
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [0.0, 0.0, 176.0, 166.0]);
    let unit = if settings.show_fe { "fe" } else { "j" };
    pen.whole(&format!("tabs/energy_info_{unit}"), [26.0, 26.0], mek_ui::ENERGY_TAB);
    pen.whole("tabs/heat_info_k", [26.0, 26.0], HEAT_TAB);
    pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], POWER_BAR);
    if let Some(v) = &v {
        let level = if v.capacity == 0 { 0.0 } else { v.energy as f64 / v.capacity as f64 };
        let h = (level * 52.0) as i32 as f32;
        if h > 0.0 {
            pen.sprite("bar/vertical_power", [0.0, 0.0, 4.0, h], [POWER_BAR[0] + 1.0, POWER_BAR[1] + POWER_BAR[3] - 1.0 - h, 4.0, h]);
        }
    }
    pen.slot("power", Some("overlay_power"), ENERGY_SLOT_AT[0] - 1.0, ENERGY_SLOT_AT[1] - 1.0);
    pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], SCREEN);
    if let Some(v) = &v {
        let usage = format!("Usage: {}/t", display::energy(v.usage, &settings));
        let mut lines = vec![format!("Temp: {}", display::temperature(v.temperature))];
        // Mekanism scales a line down to fit its inner screen; here a long usage goes on the next line (the text
        // field that sets it below isn't there).
        if game.native_text_width(&usage) > SCREEN[2] - 6.0 {
            lines.extend(["Usage:".to_owned(), format!("{}/t", display::energy(v.usage, &settings))]);
        } else {
            lines.push(usage);
        }
        for (i, line) in lines.iter().enumerate() {
            pen.text(line, [SCREEN[0] + 3.0, SCREEN[1] + 4.0 + 11.0 * i as f32], mek_ui::SCREEN_TEXT);
        }
    }
    for row in 0..4 {
        let y = if row == 3 { 141.0 } else { 83.0 + 18.0 * row as f32 };
        for col in 0..9 {
            pen.slot("normal", None, 7.0 + 18.0 * col as f32, y);
        }
    }
    let title = game.native_screen_title().unwrap_or_else(|| "Resistive Heater".into());
    pen.centered(&title, 0.0, 176.0, 5.0, mek_ui::TITLE);
    pen.text("Inventory", [8.0, 74.0], mek_ui::TITLE);
}

// ---- /mek probe -------------------------------------------------------------------------------------------------

pub(super) fn probe(game: &Game<'_>, pos: [i32; 3], nbt: &Compound, data: &[i32], items: &[Option<ItemStack>]) -> Value {
    let h = read(nbt);
    let v = unpack(data);
    let activity: Activity = mek_world::read_activity(nbt);
    let live = LIVE.with(|l| l.borrow().get(&(game.native_dimension().to_owned(), pos)).copied()).unwrap_or_default();
    json!({
        "pos": pos, "id": HEATER, "energy": h.energy, "capacity": h.capacity(), "usage": h.usage,
        "heat": h.heat, "temperature": v.as_ref().map(|v| v.temperature), "used": v.as_ref().map(|v| v.used),
        "transfer_loss": v.as_ref().map(|v| v.transfer), "environment_loss": v.as_ref().map(|v| v.loss),
        "active": activity.current, "facing": facing_of_nbt(nbt).name(), "pending": live.pending,
        "ambient": live.ambient.map(|a| a.own), "efficiency": settings().efficiency,
        "slot": items.get(ENERGY_SLOT).and_then(Option::as_ref).map(|s| json!({ "item": &*s.item, "count": s.count })),
    })
}

fn facing_of_nbt(nbt: &Compound) -> Direction {
    mek_world::generator_facing(nbt)
}
