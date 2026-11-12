// Mekanism's Supercritical Phase Shifter in the game (support module `crate::game::mek_sps` of the `mekanism` native
// bundle). The rules are `mekanism_energy::sps` (MIT, from Mekanism 10.7.19, Copyright (c) 2017-2025 Aidan C.
// Brady); the SPS forms through `mek_multiblock` (`Kind::Sps`: SPS casings, ports and glass, supercharged coils
// inside), and its tanks and core (progress, processed, the Joules received, the last processed) live in the cache by
// its id there (`chemical_tanks`: polonium in, antimatter out). This module is the glue: its settings from
// `general.toml` `[sps]` and `machine-storage.toml` `spsPort`, its tick (`SPSMultiblockData.tick`: the process, the
// output ports' antimatter to what they face), the ports with a coil behind them handing it their Joules
// (`TileEntitySPSPort.onUpdateServer`, after the multiblock's tick as the oracle's master casing ticks first), the
// ports' own energy store (`MachineEnergyContainer.input`, `energy_containers` in their block entity, from cubes,
// cables, machines and Forge Energy), their chemical handler (polonium in, antimatter out, in either mode), their
// mode (`active_state`, the Configurator's sneaking use), the screen (`GuiSPS`) and `/mek probe`. Not here: the coils'
// lasers and the orbit effects (`RenderSPS`), the sound, entities hurt inside it (`kill`) and its advancements.

use super::{
    Game, mek_chemicals,
    mek_multiblock::{self as mb, Kind},
    mek_ui::{self, Pen, Rect},
    mek_world,
    native_hooks::{NativeFeInfo, NativeScreen},
};
use horizon_client::{containers::ModBlockEntity, hud::HudBatch, text::Text};
use horizon_core::geometry::Direction as Face;
use horizon_world::{level::flags, nbt::Tag};
use mekanism_energy::{
    chemical::{ChemicalHandler, ChemicalStack},
    config::{self, EnergyConfig},
    container::{Automation, EnergyContainer},
    display,
    fe::{self, Conversion, StrictEnergy},
    multiblock::{Failure, Formation, Inner, Node, Pos, Uuid},
    sides::Direction,
    sps::{self, Core, SpsConfig, Tanks},
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    time::{Duration, Instant},
};

pub(super) const CASING: &str = "mekanism:sps_casing";
pub(super) const PORT: &str = "mekanism:sps_port";
const COIL: &str = "mekanism:supercharged_coil";
const POLONIUM: &str = "mekanism:polonium";
const ANTIMATTER: &str = "mekanism:antimatter";

thread_local! {
    static SETTINGS: RefCell<Option<(SpsConfig, config::Stamp, Instant)>> = const { RefCell::new(None) };
}

/// `general.toml` `[sps]` and `machine-storage.toml` `spsPort`, read again within a second of a change.
pub(super) fn settings() -> SpsConfig {
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
                Some(d) => (SpsConfig::load(d), config::stamp(d), Instant::now()),
                None => (SpsConfig::default(), [None; config::FILES.len()], Instant::now()),
            });
        }
        s.as_ref().map(|(c, _, _)| *c).unwrap_or_default()
    })
}

/// A casing or port, whose screen shows the SPS.
pub(super) fn is_sps_block(block: &str) -> bool {
    block == CASING || block == PORT
}

/// `FormationProtocol.doUpdate` with the `SPSValidator` over the structure `nodes`.
pub(super) fn form(game: &mut Game<'_>, nodes: &BTreeMap<Pos, Node>) -> Result<Formation, Failure> {
    sps::form(nodes, |p| {
        let block = game.native_block(p);
        if mb::is_air(&block) {
            Inner::Air
        } else if block == COIL {
            Inner::Internal
        } else {
            Inner::Invalid
        }
    })
}

/// `onCreated` and `SPSCache.apply`: the tanks within their capacities, the cache's core kept.
pub(super) fn created(dimension: &str, id: Uuid) {
    let config = settings();
    mb::with_cache(dimension, Kind::Sps, id, |cache| {
        for (tank, cap) in cache.chemicals.iter_mut().zip([config.input_capacity(), config.output_tank_capacity]) {
            if tank.amount() > cap {
                tank.set_amount(cap);
            }
        }
        cache.sps.get_or_insert_with(Core::default);
    });
}

/// Its ports with a coil right behind them (`coilData.coilMap`).
fn coil_ports(dimension: &str, id: Uuid) -> Vec<Pos> {
    mb::with_formed(dimension, id, |f| {
        let internal: Vec<Pos> = f.internal.iter().copied().collect();
        sps::coils(&f.valves, &internal).into_iter().map(|(p, _)| p).collect()
    })
    .unwrap_or_default()
}

fn output_mode(entity: &ModBlockEntity) -> bool {
    mek_world::number(entity.nbt.get(&mek_world::key("active_state"))).is_some_and(|v| v != 0)
}

fn tanks_of(cache: &mb::Cache) -> Tanks {
    Tanks { input: cache.chemicals.first().map_or(0, |s| s.amount().max(0)), output: cache.chemicals.get(1).map_or(0, |s| s.amount().max(0)) }
}

fn set_tank(stack: &ChemicalStack, amount: i64, id: &str) -> ChemicalStack {
    if amount <= 0 {
        ChemicalStack::EMPTY
    } else if stack.is_empty() {
        ChemicalStack::new(id, amount)
    } else {
        stack.copy_with_amount(amount)
    }
}

// ---- The tick ---------------------------------------------------------------------------------------------------

/// `SPSMultiblockData.tick` (the process, the output ports' antimatter split among what they face), then each port
/// with a coil behind it hands over all its Joules while the SPS can operate (`canSupplyCoilEnergy`), for the next
/// tick.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, id: Uuid) {
    let config = settings();
    let Some(valves) = mb::with_formed(dimension, id, |f| f.valves.clone()) else { return };
    let Some((mut core, out_stack)) = mb::with_cache(dimension, Kind::Sps, id, |c| {
        let mut core = c.sps.unwrap_or_default();
        let mut tanks = tanks_of(c);
        core.tick(&mut tanks, &config);
        c.chemicals[0] = set_tank(&c.chemicals[0], tanks.input, POLONIUM);
        c.chemicals[1] = set_tank(&c.chemicals[1], tanks.output, ANTIMATTER);
        c.sps = Some(core);
        (core, c.chemicals[1].clone())
    }) else {
        return;
    };
    if !out_stack.is_empty() {
        let targets: Vec<(Pos, Direction)> = valves
            .iter()
            .filter(|(p, _)| game.native_with_mod_block_entity(*p, |m| m.id == PORT && output_mode(m)).unwrap_or(false))
            .map(|(p, side)| (mek_world::beside(*p, *side), side.opposite()))
            .collect();
        let sent = mek_chemicals::emit_to(game, &targets, &out_stack);
        if sent > 0 {
            mb::with_cache(dimension, Kind::Sps, id, |c| c.chemicals[1] = c.chemicals[1].copy_with_amount(c.chemicals[1].amount() - sent).normalized());
        }
    }
    for p in coil_ports(dimension, id) {
        let Some(tanks) = mb::with_cache(dimension, Kind::Sps, id, |c| tanks_of(c)) else { return };
        if !core.can_supply(tanks, &config) {
            continue;
        }
        let joules = game
            .native_with_mod_block_entity(p, |m| {
                let stored = mek_world::saved_energy(&m.nbt).unwrap_or(0);
                if m.id == PORT && stored > 0 {
                    mek_world::write_energy(&mut m.nbt, 0);
                }
                if m.id == PORT { stored } else { 0 }
            })
            .unwrap_or(0);
        if joules > 0 {
            core.supply(joules);
            mb::with_cache(dimension, Kind::Sps, id, |c| c.sps = Some(core));
        }
    }
}

// ---- The ports' energy (`MachineEnergyContainer.input`) -----------------------------------------------------------

/// Whether `entity` is an SPS port, which takes energy through every side.
pub(super) fn takes_energy(entity: &ModBlockEntity) -> Option<bool> {
    (entity.id == PORT).then_some(true)
}

fn store(entity: &ModBlockEntity) -> EnergyContainer {
    let mut container = EnergyContainer::basic(settings().port_storage);
    container.stored = mek_world::saved_energy(&entity.nbt).unwrap_or(0).min(container.capacity);
    container
}

/// Offers `amount` Joules to the port in `entity`: what it didn't take.
pub(super) fn offer(entity: &mut ModBlockEntity, amount: u64, simulate: bool) -> Option<u64> {
    if entity.id != PORT {
        return None;
    }
    let mut container = store(entity);
    let before = container.stored;
    let rest = container.insert(amount, simulate, Automation::External);
    if !simulate && container.stored != before {
        mek_world::write_energy(&mut entity.nbt, container.stored);
    }
    Some(rest)
}

/// A port's store as a side reaches it: it takes, and gives nothing (`input`).
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

/// `native_fe_storage` of a port: its store in whole FE.
pub(super) fn fe_storage(entity: &ModBlockEntity, config: &EnergyConfig, info: &mut Option<NativeFeInfo>) -> bool {
    if entity.id != PORT {
        return false;
    }
    if config.blacklist_forge {
        return true;
    }
    let c = store(entity);
    let rate = Conversion(config.fe_conversion_rate);
    *info = Some(NativeFeInfo {
        stored: i64::from(rate.to_fe_int(c.stored.min(i64::MAX as u64) as i64)),
        capacity: i64::from(rate.to_fe_int(c.capacity.min(i64::MAX as u64) as i64)),
        can_receive: true,
        can_extract: false,
    });
    true
}

/// `native_fe_receive` and `native_fe_extract` of a port.
#[allow(clippy::too_many_arguments)]
pub(super) fn fe_transfer(pos: Pos, entity: &mut ModBlockEntity, face: Option<Face>, config: &EnergyConfig, max: i64, simulate: bool, extract: bool, moved: &mut Option<i64>) -> bool {
    if entity.id != PORT {
        return false;
    }
    if config.blacklist_forge {
        return true;
    }
    let container = store(entity);
    let before = container.stored;
    let mut side = Side { container, open: face.is_some(), taken: 0 };
    let rate = Conversion(config.fe_conversion_rate);
    let max = max.clamp(0, i64::from(i32::MAX)) as i32;
    let fe = if extract { fe::extract_fe(&mut side, rate, max, simulate) } else { fe::receive_fe(&mut side, rate, max, simulate) };
    if !simulate && side.container.stored != before {
        mek_world::write_energy(&mut entity.nbt, side.container.stored);
    }
    if side.taken > 0 {
        mek_world::add_received(pos, side.taken);
    }
    *moved = Some(i64::from(fe));
    true
}

// ---- The ports' chemical handler and mode ---------------------------------------------------------------------------

/// A port's tanks (`getChemicalTanks`: the multiblock's two in either mode): polonium in from outside, antimatter out
/// to outside; the null side reads only.
struct PortChemical {
    tanks: [(ChemicalStack, i64); 2],
    side: bool,
}

impl ChemicalHandler for PortChemical {
    fn tanks(&self) -> usize {
        2
    }

    fn in_tank(&self, tank: usize) -> ChemicalStack {
        self.tanks.get(tank).map_or(ChemicalStack::EMPTY, |t| t.0.clone())
    }

    fn capacity(&self, tank: usize) -> i64 {
        self.tanks.get(tank).map_or(0, |t| t.1)
    }

    fn is_valid(&self, tank: usize, stack: &ChemicalStack) -> bool {
        stack.id() == Some(if tank == 0 { POLONIUM } else { ANTIMATTER })
    }

    fn insert_tank(&mut self, tank: usize, stack: &ChemicalStack, simulate: bool) -> ChemicalStack {
        if !self.side || tank != 0 || stack.is_empty() || !self.is_valid(0, stack) {
            return stack.clone();
        }
        let t = &mut self.tanks[0];
        let took = stack.amount().min((t.1 - t.0.amount().max(0)).max(0));
        if took <= 0 {
            return stack.clone();
        }
        if !simulate {
            t.0 = if t.0.is_empty() { stack.copy_with_amount(took) } else { t.0.copy_with_amount(t.0.amount() + took) };
        }
        stack.copy_with_amount(stack.amount() - took).normalized()
    }

    fn extract_tank(&mut self, tank: usize, amount: i64, simulate: bool) -> ChemicalStack {
        if !self.side || tank != 1 || amount <= 0 || self.tanks[1].0.is_empty() {
            return ChemicalStack::EMPTY;
        }
        let t = &mut self.tanks[1];
        let n = amount.min(t.0.amount());
        let got = t.0.copy_with_amount(n);
        if !simulate {
            t.0 = t.0.copy_with_amount(t.0.amount() - n).normalized();
        }
        got
    }
}

/// `Capabilities.CHEMICAL` of an SPS port at `pos` through `face` (none while unformed): None for any other block.
pub(super) fn chemical_handler<R>(dimension: &str, pos: Pos, block: &str, face: Option<Direction>, use_it: impl FnOnce(&mut dyn ChemicalHandler) -> R) -> Option<R> {
    if block != PORT {
        return None;
    }
    let config = settings();
    let owner = mb::formed_at(dimension, Kind::Sps, pos);
    let held = owner.and_then(|id| mb::with_cache(dimension, Kind::Sps, id, |c| [c.chemicals[0].clone(), c.chemicals[1].clone()]).map(|s| (id, s)));
    let mut handler = match &held {
        Some((_, [i, o])) => PortChemical { tanks: [(i.clone(), config.input_capacity()), (o.clone(), config.output_tank_capacity)], side: face.is_some() },
        None => PortChemical { tanks: [(ChemicalStack::EMPTY, 0), (ChemicalStack::EMPTY, 0)], side: false },
    };
    let answer = use_it(&mut handler);
    if let Some((id, before)) = held
        && (handler.tanks[0].0 != before[0] || handler.tanks[1].0 != before[1])
    {
        let [(i, _), (o, _)] = handler.tanks;
        mb::with_cache(dimension, Kind::Sps, id, |c| {
            c.chemicals[0] = i;
            c.chemicals[1] = o;
        });
    }
    Some(answer)
}

/// The Configurator's sneaking use on a port (`onSneakRightClick`): input and output swap; the action bar names the
/// mode it had (`InputOutput.of(oldMode)`, as Mekanism 10.7.19 does).
pub(super) fn toggle(game: &mut Game<'_>, pos: Pos) -> Option<bool> {
    let old = game.native_with_mod_block_entity(pos, |m| {
        if m.id != PORT {
            return None;
        }
        let old = output_mode(m);
        m.nbt.insert(mek_world::key("active_state"), Tag::Byte(i8::from(!old)));
        Some(old)
    })??;
    game.native_set_block_property_with(pos, "active", if old { "false" } else { "true" }, flags::UPDATE_ALL);
    let player = game.native_acting_id();
    game.native_action_bar(player, Text::literal(format!("Toggled SPS Port mode to: {}.", if old { "Output" } else { "Input" })));
    Some(!old)
}

// ---- The screen (`GuiSPS`) --------------------------------------------------------------------------------------

const DATA: usize = 4 * 6 + 2;

fn push(out: &mut Vec<i32>, v: u64) {
    out.extend((0..4).map(|i| ((v >> (16 * i)) & 0xFFFF) as i32));
}

/// The data slots of a formed SPS: polonium and antimatter with their capacities, the Joules of its last tick, the
/// bits of its last processed and of its progress (four 16-bit pieces each), its coils and its active.
pub(super) fn view(dimension: &str, id: Uuid) -> Vec<i32> {
    let config = settings();
    let Some((tanks, core)) = mb::with_cache(dimension, Kind::Sps, id, |c| (tanks_of(c), c.sps.unwrap_or_default())) else { return Vec::new() };
    let mut out = Vec::with_capacity(DATA);
    push(&mut out, tanks.input as u64);
    push(&mut out, config.input_capacity() as u64);
    push(&mut out, tanks.output as u64);
    push(&mut out, config.output_tank_capacity as u64);
    push(&mut out, core.last_received);
    push(&mut out, core.last_processed.to_bits());
    out.push(coil_ports(dimension, id).len() as i32);
    out.push(i32::from(core.active()));
    out.extend((0..4).map(|i| ((core.scaled_progress(&config).to_bits() >> (16 * i)) & 0xFFFF) as i32));
    out
}

struct View {
    input: u64,
    input_cap: u64,
    output: u64,
    output_cap: u64,
    energy: u64,
    last_processed: f64,
    progress: f64,
}

fn unpack(data: &[i32]) -> Option<View> {
    if data.len() < DATA + 4 {
        return None;
    }
    let long = |at: usize| (0..4).fold(0u64, |v, i| v | (u64::from(data[at + i] as u16) << (16 * i)));
    Some(View {
        input: long(0),
        input_cap: long(4),
        output: long(8),
        output_cap: long(12),
        energy: long(16),
        last_processed: f64::from_bits(long(20)),
        progress: f64::from_bits(long(26)),
    })
}

const WIDTH: f32 = 176.0;
const HEIGHT: f32 = 182.0;
const INPUT_GAUGE: Rect = [7.0, 17.0, 18.0, 60.0];
const OUTPUT_GAUGE: Rect = [151.0, 17.0, 18.0, 60.0];
const SCREEN: Rect = [27.0, 17.0, 122.0, 60.0];
const BAR: Rect = [7.0, 79.0, 160.0, 8.0];

pub(super) fn screen() -> NativeScreen {
    NativeScreen { width: WIDTH, height: HEIGHT, slots: vec![None; 5], inventory_y: 100.0, inventory_x: 8.0, own_labels: true, own_background: false }
}

/// The inner screen's lines: its status, and while active the Joules it took and its rate.
fn lines(view: &View, settings: &EnergyConfig) -> Vec<String> {
    let config = self::settings();
    let core = Core { last_processed: view.last_processed, ..Core::default() };
    let active = core.active();
    let mut out = vec![format!("Status: {}", if active { "Active" } else { "Idle" })];
    if active {
        out.push(format!("Energy Input: {}/t", display::energy(view.energy, settings)));
        out.push(format!("Process Rate: {} mB/t", core.process_rate(&config)));
    }
    out
}

fn chemical_tip(name: &str, amount: u64) -> String {
    if amount == 0 { "Empty".into() } else { format!("{name}: {} mB", display::thousands(amount)) }
}

pub(super) fn tooltip(data: &[i32], at: [f32; 2], tips: &mut Vec<String>) {
    let Some(view) = unpack(data) else { return };
    if mek_ui::inside(INPUT_GAUGE, at) {
        tips.push(chemical_tip("Polonium", view.input));
    } else if mek_ui::inside(OUTPUT_GAUGE, at) {
        tips.push(chemical_tip("Antimatter", view.output));
    } else if mek_ui::inside(BAR, at) {
        tips.push(format!("Progress: {}%", (view.progress * 100.0).floor()));
    }
}

fn gauge(pen: &mut Pen<'_, '_>, r: Rect, amount: u64, capacity: u64, argb: u32) {
    pen.nine("gauge/normal", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], r);
    let inner = r[3] - 2.0;
    let level = if capacity == 0 { 0.0 } else { (amount as f64 / capacity as f64).min(1.0) };
    let h = (level * f64::from(inner)) as i32 as f32;
    if h > 0.0 {
        pen.fill([r[0] + 1.0, r[1] + 1.0 + inner - h, r[2] - 2.0, h], argb);
    }
    pen.whole("gauge/standard", [16.0, 58.0], [r[0] + 1.0, r[1] + 1.0, 16.0, 58.0]);
}

pub(super) fn draw(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let settings = mek_ui::settings();
    let cursor = game.native_cursor();
    let cursor = [cursor[0] - origin[0], cursor[1] - origin[1]];
    let mut pen = Pen { game, batch, origin, cursor };
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [0.0, 0.0, WIDTH, HEIGHT]);
    let registry = mek_chemicals::registry(game);
    let color = |id: &str| 0xFF00_0000 | registry.get(id).map_or(0x808080, |c| c.color & 0xFF_FFFF);
    let view = unpack(data);
    if let Some(v) = &view {
        gauge(&mut pen, INPUT_GAUGE, v.input, v.input_cap, color(POLONIUM));
        gauge(&mut pen, OUTPUT_GAUGE, v.output, v.output_cap, color(ANTIMATTER));
    }
    pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], SCREEN);
    if let Some(v) = &view {
        for (i, line) in lines(v, &settings).iter().enumerate() {
            pen.text(line, [SCREEN[0] + 3.0, SCREEN[1] + 3.0 + 10.0 * i as f32], mek_ui::SCREEN_TEXT);
        }
        // `GuiDynamicHorizontalRateBar`: the progress, from purple-grey to purple.
        pen.fill(BAR, 0xFF00_0000);
        let w = ((v.progress.min(1.0) * f64::from(BAR[2] - 2.0)) as i32) as f32;
        if w > 0.0 {
            pen.fill([BAR[0] + 1.0, BAR[1] + 1.0, w, BAR[3] - 2.0], 0xFF64_1EAA);
        }
    }
    for row in 0..4 {
        let y = if row == 3 { 157.0 } else { 99.0 + 18.0 * row as f32 };
        for col in 0..9 {
            pen.slot("normal", None, 7.0 + 18.0 * col as f32, y);
        }
    }
    let title = game.native_screen_title().unwrap_or_else(|| "Supercritical Phase Shifter".into());
    pen.centered(&title, 0.0, WIDTH, 6.0, mek_ui::TITLE);
    pen.text("Inventory", [8.0, 90.0], mek_ui::TITLE);
}

// ---- /mek probe -------------------------------------------------------------------------------------------------

pub(super) fn probe(game: &mut Game<'_>, dimension: &str, id: Uuid) -> Value {
    let config = settings();
    let Some((input, output, core)) = mb::with_cache(dimension, Kind::Sps, id, |c| (c.chemicals[0].clone(), c.chemicals[1].clone(), c.sps.unwrap_or_default())) else {
        return json!({});
    };
    let ports: Vec<Value> = mb::with_formed(dimension, id, |f| f.valves.clone())
        .unwrap_or_default()
        .into_iter()
        .map(|(p, side)| {
            let (output, energy) = game.native_with_mod_block_entity(p, |m| (output_mode(m), mek_world::saved_energy(&m.nbt).unwrap_or(0))).unwrap_or((false, 0));
            json!({ "pos": p, "side": side.name(), "output": output, "energy": energy })
        })
        .collect();
    json!({
        "polonium": input.amount().max(0), "antimatter": output.amount().max(0),
        "input_capacity": config.input_capacity(), "output_capacity": config.output_tank_capacity,
        "progress": core.progress, "processed": core.input_processed, "could_operate": core.could_operate,
        "received_energy": core.received_energy, "last_received": core.last_received, "last_processed": core.last_processed,
        "process_rate": core.process_rate(&config), "scaled_progress": core.scaled_progress(&config),
        "coils": coil_ports(dimension, id), "ports": ports,
    })
}

/// `/mek probe` of an SPS port standing alone: its store.
pub(super) fn port_probe(game: &mut Game<'_>, pos: Pos) -> Option<Value> {
    game.native_with_mod_block_entity(pos, |m| (m.id == PORT).then(|| json!({ "sps_port": { "energy": mek_world::saved_energy(&m.nbt).unwrap_or(0), "output": output_mode(m) } })))
        .flatten()
}
