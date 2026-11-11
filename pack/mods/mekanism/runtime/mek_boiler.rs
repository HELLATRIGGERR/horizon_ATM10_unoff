// Mekanism's Thermoelectric Boiler in the game (support module `crate::game::mek_boiler` of the `mekanism` native
// bundle). The rules are `mekanism_energy::boiler` (MIT, from Mekanism 10.7.19, Copyright (c) 2017-2025 Aidan C.
// Brady); the boiler forms through `mek_multiblock` (`Kind::Boiler`: boiler casings, boiler valves and structural
// glass, pressure dispersers and superheating elements inside), and its tanks and heat live in the cache by its id
// there (`fluid_tanks`: water; `chemical_tanks`: steam, heated coolant, cooled coolant; `heat_capacitors`). This
// module is the glue: its settings from `general.toml` `[boiler]`, the air of its corners' biomes, its tick
// (`BoilerMultiblockData.tick`: the air, the heat handed to it, the coolant, the boil, the valves' output), its
// superheating elements lit while it is hot (`hotMap`), the valves' fluid handler (water in their input mode) and
// chemical handler by their mode (`BoilerValveMode`, the block's `mode`: heated coolant in, steam or cooled coolant
// out), the Configurator's turn of a valve's mode, its screen (`GuiThermoelectricBoiler`, `GuiBoilerStats` as a
// page) and `/mek probe`.

use super::{
    Game, mek_chemicals,
    mek_multiblock::{self as mb, Formed, Kind},
    mek_ui::{self, Pen, Rect},
    native_hooks::{NativeFluidTankInfo, NativeScreen},
};
use horizon_client::{hud::HudBatch, text::Text};
use horizon_core::geometry::Direction as Face;
use horizon_world::{fluid_stack::FluidStack, level::flags};
use mekanism_energy::{
    boiler::{self, BoilerConfig, Capacities, HeatedCoolant, Inside, Shape, Tanks, ValveMode},
    chemical::{ChemicalHandler, ChemicalRegistry, ChemicalStack},
    config, display,
    evaporation::{self, Heat},
    multiblock::{Failure, Formation, Node, Pos, Uuid},
    sides::Direction,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap},
    time::{Duration, Instant},
};

pub(super) const CASING: &str = "mekanism:boiler_casing";
pub(super) const VALVE: &str = "mekanism:boiler_valve";
const DISPERSER: &str = "mekanism:pressure_disperser";
const ELEMENT: &str = "mekanism:superheating_element";
const STEAM: &str = "mekanism:steam";

/// What a formed boiler keeps beside its cache (not saved: made again when it forms).
#[derive(Clone, Copy, Debug, Default)]
struct State {
    /// `biomeAmbientTemp`.
    ambient: f64,
    /// Heat handed to its capacitor too small to land yet (`heatToHandle` below `1.0E-6F`).
    pending: f64,
    last: boiler::Tick,
    /// Whether its elements were last lit.
    lit: Option<bool>,
}

thread_local! {
    static SETTINGS: RefCell<Option<(BoilerConfig, config::Stamp, Instant)>> = const { RefCell::new(None) };
    static STATES: RefCell<HashMap<Uuid, State>> = RefCell::new(HashMap::new());
    /// Elements of boilers that broke with no game at hand, darkened at the next level tick.
    static DARK: RefCell<Vec<(String, Pos)>> = const { RefCell::new(Vec::new()) };
    /// The screen's page on the client: 0 the boiler, 1 its statistics (`GuiBoilerStats`).
    static PAGE: Cell<u8> = const { Cell::new(0) };
    /// Each formed boiler's valves' modes as its last tick read their blocks, for the fluid handler (whose hook
    /// point has no game at hand).
    static MODES: RefCell<HashMap<(String, Pos), ValveMode>> = RefCell::new(HashMap::new());
}

/// `general.toml` `[boiler]`, read again within a second of a change.
pub(super) fn settings() -> BoilerConfig {
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
                Some(d) => (BoilerConfig::load(d), config::stamp(d), Instant::now()),
                None => (BoilerConfig::default(), [None; config::FILES.len()], Instant::now()),
            });
        }
        s.as_ref().map(|(c, _, _)| *c).unwrap_or_default()
    })
}

/// A casing or valve, whose screen shows the boiler.
pub(super) fn is_boiler_block(block: &str) -> bool {
    block == CASING || block == VALVE
}

/// A world opens or closes.
pub(super) fn clear() {
    STATES.with(|s| s.borrow_mut().clear());
    DARK.with(|d| d.borrow_mut().clear());
    MODES.with(|m| m.borrow_mut().clear());
}

/// `FormationProtocol.doUpdate` with the `BoilerValidator` over the structure `nodes`.
pub(super) fn form(game: &mut Game<'_>, nodes: &BTreeMap<Pos, Node>) -> Result<(Formation, Shape), Failure> {
    boiler::form(nodes, |p| {
        let block = game.native_block(p);
        match block.as_str() {
            DISPERSER => Inside::Disperser,
            ELEMENT => Inside::Element,
            b if mb::is_air(b) => Inside::Air,
            _ => Inside::Invalid,
        }
    })
}

fn capacities(dimension: &str, id: Uuid) -> Option<(Capacities, Shape)> {
    let shape = mb::with_formed(dimension, id, |f| f.boiler)??;
    Some((settings().capacities(&shape), shape))
}

fn set_elements(game: &mut Game<'_>, positions: impl IntoIterator<Item = Pos>, lit: bool) {
    let shown = if lit { "true" } else { "false" };
    for p in positions {
        if game.native_block(p) == ELEMENT && game.native_block_property(p, "active").as_deref() != Some(shown) {
            game.native_set_block_property_with(p, "active", shown, flags::UPDATE_ALL);
        }
    }
}

/// `onCreated`: the air of its eight corners, its heat capacity (50 a wall block, the cache's heat coming in), and
/// the tanks capped to its volumes (`shouldCap`).
pub(super) fn created(game: &mut Game<'_>, dimension: &str, id: Uuid) {
    let Some(formed) = mb::with_formed(dimension, id, Clone::clone) else { return };
    let Some((caps, _)) = capacities(dimension, id) else { return };
    let c = formed.cuboid;
    let corners: Vec<f32> = [c.min[0], c.max[0]]
        .into_iter()
        .flat_map(|x| [c.min[1], c.max[1]].into_iter().flat_map(move |y| [c.min[2], c.max[2]].into_iter().map(move |z| [x, y, z])))
        .filter_map(|p| game.native_biome_climate(p).map(|b| b.temperature))
        .collect();
    let ambient = evaporation::ambient(&corners);
    mb::with_cache(dimension, Kind::Boiler, id, |cache| {
        let heat = boiler::heat_at_formation(cache.heat.first().copied().flatten(), formed.locations.len(), ambient);
        if let Some(slot) = cache.heat.first_mut() {
            *slot = Some(heat);
        }
        if let Some(water) = cache.fluids.first_mut()
            && water.amount() > caps.water
        {
            *water = water.copy_with_amount(caps.water);
        }
        for (tank, cap) in cache.chemicals.iter_mut().zip([caps.steam, caps.heated, caps.cooled]) {
            if tank.amount() > cap {
                tank.set_amount(cap);
            }
        }
    });
    STATES.with(|s| s.borrow_mut().insert(id, State { ambient, ..State::default() }));
    let modes: Vec<(Pos, Direction, ValveMode)> = formed.valves.iter().map(|(p, side)| (*p, *side, mode_at(game, *p))).collect();
    note_modes(dimension, &modes);
}

/// `remove`: the boiler broke; its elements go dark (when the game is at hand: a block entity unloading isn't).
pub(super) fn removed(game: Option<&mut Game<'_>>, dimension: &str, formed: &Formed) {
    STATES.with(|s| s.borrow_mut().remove(&formed.id));
    MODES.with(|m| {
        let mut m = m.borrow_mut();
        for (p, _) in &formed.valves {
            m.remove(&(dimension.to_owned(), *p));
        }
    });
    match game {
        Some(game) => set_elements(game, formed.internal.iter().copied(), false),
        None => DARK.with(|d| d.borrow_mut().extend(formed.internal.iter().map(|p| (dimension.to_owned(), *p)))),
    }
}

/// `native_level_tick_post`: the elements of boilers that broke while a block entity unloaded go dark, unless a
/// boiler formed with them since.
pub(super) fn flush(game: &mut Game<'_>, dimension: &str) {
    let mine: Vec<Pos> = DARK.with(|d| {
        let mut d = d.borrow_mut();
        let (mine, rest): (Vec<_>, Vec<_>) = d.drain(..).partition(|(dim, _)| dim == dimension);
        *d = rest;
        mine.into_iter().map(|(_, p)| p).collect()
    });
    let loose: Vec<Pos> = mine.into_iter().filter(|p| mb::formed_at(dimension, Kind::Boiler, *p).is_none()).collect();
    set_elements(game, loose, false);
}

fn mode_at(game: &Game<'_>, pos: Pos) -> ValveMode {
    game.native_block_property(pos, "mode").and_then(|m| ValveMode::from_name(&m)).unwrap_or_default()
}

/// A valve's mode as its boiler last read it.
fn known_mode(dimension: &str, pos: Pos) -> ValveMode {
    MODES.with(|m| m.borrow().get(&(dimension.to_owned(), pos)).copied()).unwrap_or_default()
}

fn note_modes(dimension: &str, modes: &[(Pos, Direction, ValveMode)]) {
    MODES.with(|m| {
        let mut m = m.borrow_mut();
        for (p, _, mode) in modes {
            m.insert((dimension.to_owned(), *p), *mode);
        }
    });
}

/// The coolant a chemical is as heated coolant, and its cooled variant (`HeatedCoolant`, `IS_HEATED_COOLANT`).
fn heated_coolant(registry: &ChemicalRegistry, stack: &ChemicalStack) -> Option<(HeatedCoolant, String)> {
    let c = registry.get(stack.id()?)?.heated.as_ref()?;
    Some((HeatedCoolant { temperature: c.temperature.unwrap_or(100_000.0), thermal_enthalpy: c.thermal_enthalpy, conductivity: c.conductivity }, c.other.clone()))
}

fn is_cooled_coolant(registry: &ChemicalRegistry, id: Option<&str>) -> bool {
    id.and_then(|i| registry.get(i)).is_some_and(|c| c.cooled.is_some())
}

/// `BoilerMultiblockData.tick` with the master's `updateHeatCapacitors`: the heat (the air, what heaters handed it,
/// the coolant), the boil, then the steam out of its steam valves and the cooled coolant out of its coolant valves
/// (`ChemicalUtil.emit`, split among the blocks they face), and its elements lit by whether it was hot.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, id: Uuid) {
    let Some(formed) = mb::with_formed(dimension, id, Clone::clone) else { return };
    let Some(shape) = formed.boiler else { return };
    let Some(before) = mb::with_cache(dimension, Kind::Boiler, id, |c| c.clone()) else { return };
    if before.fluids.is_empty() || before.chemicals.len() < 3 || before.heat.is_empty() {
        return;
    }
    let config = settings();
    let caps = config.capacities(&shape);
    let registry = mek_chemicals::registry(game);
    let mut state = STATES.with(|s| s.borrow().get(&id).copied()).unwrap_or(State { ambient: evaporation::AMBIENT, ..State::default() });
    let mut cache = before.clone();
    let mut heat = cache.heat[0].unwrap_or_else(|| boiler::heat_at_formation(None, formed.locations.len(), state.ambient));
    let mut tanks = Tanks {
        water: cache.fluids[0].amount().max(0),
        steam: cache.chemicals[0].amount().max(0),
        heated: cache.chemicals[1].amount().max(0),
        cooled: cache.chemicals[2].amount().max(0),
    };
    let coolant = heated_coolant(&registry, &cache.chemicals[1]);
    let cooled_takes = coolant.as_ref().is_some_and(|(_, cooled)| {
        is_cooled_coolant(&registry, Some(cooled)) && (cache.chemicals[2].is_empty() || cache.chemicals[2].id() == Some(cooled.as_str()))
    });
    let mut pending = state.pending + mb::take_heat(id);
    state.last = boiler::tick(&mut tanks, &caps, &mut heat, &mut pending, state.ambient, shape.elements, coolant.as_ref().map(|c| c.0), cooled_takes, &config);
    state.pending = pending;
    cache.heat[0] = Some(heat);
    cache.fluids[0] = if tanks.water > 0 { cache.fluids[0].copy_with_amount(tanks.water) } else { FluidStack::EMPTY };
    cache.chemicals[0] = if tanks.steam > 0 { ChemicalStack::new(STEAM, tanks.steam) } else { ChemicalStack::EMPTY };
    cache.chemicals[1] = cache.chemicals[1].copy_with_amount(tanks.heated).normalized();
    cache.chemicals[2] = match (&coolant, cache.chemicals[2].is_empty()) {
        (Some((_, cooled)), true) if tanks.cooled > 0 => ChemicalStack::new(cooled, tanks.cooled),
        _ => cache.chemicals[2].copy_with_amount(tanks.cooled).normalized(),
    };
    if cache != before {
        mb::with_cache(dimension, Kind::Boiler, id, |c| *c = cache.clone());
    }
    // The valves' output, steam first.
    let modes: Vec<(Pos, Direction, ValveMode)> = formed.valves.iter().map(|(p, side)| (*p, *side, mode_at(game, *p))).collect();
    note_modes(dimension, &modes);
    for (tank, mode) in [(0, ValveMode::OutputSteam), (2, ValveMode::OutputCoolant)] {
        let stack = cache.chemicals[tank].clone();
        if stack.is_empty() {
            continue;
        }
        let targets: Vec<(Pos, Direction)> = modes.iter().filter(|(_, _, m)| *m == mode).map(|(p, side, _)| (super::mek_world::beside(*p, *side), side.opposite())).collect();
        let sent = mek_chemicals::emit_to(game, &targets, &stack);
        if sent > 0 {
            mb::with_cache(dimension, Kind::Boiler, id, |c| {
                if let Some(s) = c.chemicals.get_mut(tank) {
                    *s = s.copy_with_amount(s.amount() - sent).normalized();
                }
            });
        }
    }
    if state.lit != Some(state.last.hot) {
        set_elements(game, formed.internal.iter().copied(), state.last.hot);
        state.lit = Some(state.last.hot);
    }
    STATES.with(|s| s.borrow_mut().insert(id, state));
}

// ---- The valves' fluid handler (hook points API 422) --------------------------------------------------------

/// The boiler a valve at `pos` reaches, and the valve's mode: Some(None) for a valve of no formed boiler.
fn valve_of(dimension: &str, pos: Pos, block: &str) -> Option<Option<Uuid>> {
    (block == VALVE).then(|| mb::formed_at(dimension, Kind::Boiler, pos))
}

/// `native_fluid_handler_tanks` of a valve: in its input mode, once formed, the water tank (which gives nothing
/// out); none in its output modes.
pub(super) fn fluid_tanks(dimension: &str, pos: Pos, block: &str, tanks: &mut Option<Vec<NativeFluidTankInfo>>) -> bool {
    let Some(owner) = valve_of(dimension, pos, block) else { return false };
    let input = known_mode(dimension, pos) == ValveMode::Input;
    let held = owner.filter(|_| input).and_then(|id| Some((capacities(dimension, id)?.0, mb::with_cache(dimension, Kind::Boiler, id, |c| c.fluids.first().cloned())??)));
    *tanks = Some(match held {
        Some((caps, water)) => vec![NativeFluidTankInfo { stack: water, capacity: caps.water, drainable: false }],
        None => Vec::new(),
    });
    true
}

/// `native_fluid_handler_fill`: what fits of water (`FluidTags.WATER`) in the input mode; the null side reads only.
pub(super) fn fluid_fill(dimension: &str, pos: Pos, face: Option<Face>, block: &str, resource: &FluidStack, simulate: bool, filled: &mut Option<i32>) -> bool {
    let Some(owner) = valve_of(dimension, pos, block) else { return false };
    *filled = Some(0);
    let (Some(id), Some(_)) = (owner, face) else { return true };
    if known_mode(dimension, pos) != ValveMode::Input || resource.is_empty() || !resource.is_tag("minecraft:water") {
        return true;
    }
    let Some((caps, _)) = capacities(dimension, id) else { return true };
    let took = mb::with_cache(dimension, Kind::Boiler, id, |c| {
        let Some(tank) = c.fluids.get_mut(0) else { return 0 };
        let same = tank.is_empty() || FluidStack::is_same_fluid_same_components(tank, resource);
        let took = mekanism_energy::dynamic_tank::fluid_room(tank.amount().max(0), caps.water, same, false, resource.amount());
        if !simulate && took > 0 {
            *tank = if tank.is_empty() { resource.copy_with_amount(took) } else { tank.copy_with_amount(tank.amount() + took) };
        }
        took
    })
    .unwrap_or(0);
    *filled = Some(took);
    true
}

/// `native_fluid_handler_drain`: the water tank gives nothing to other blocks (`VariableCapacityFluidTank.input`).
pub(super) fn fluid_drain(dimension: &str, pos: Pos, block: &str, drained: &mut Option<FluidStack>) -> bool {
    if valve_of(dimension, pos, block).is_none() {
        return false;
    }
    *drained = Some(FluidStack::EMPTY);
    true
}

// ---- The valves' chemical handler --------------------------------------------------------------------------------

/// A valve's tank by its mode (`getChemicalTanks(mode)`): the heated coolant tank in its input mode (which takes
/// heated coolant and gives nothing to other blocks), the steam or cooled coolant tank in its output modes (which
/// give and take nothing from other blocks); the null side reads only.
struct ValveChemical<'r> {
    stack: ChemicalStack,
    capacity: i64,
    mode: ValveMode,
    formed: bool,
    side: bool,
    registry: &'r ChemicalRegistry,
}

impl ChemicalHandler for ValveChemical<'_> {
    fn tanks(&self) -> usize {
        usize::from(self.formed)
    }

    fn in_tank(&self, _: usize) -> ChemicalStack {
        self.stack.clone()
    }

    fn capacity(&self, _: usize) -> i64 {
        self.capacity
    }

    fn is_valid(&self, _: usize, stack: &ChemicalStack) -> bool {
        match self.mode {
            ValveMode::Input => heated_coolant(self.registry, stack).is_some(),
            ValveMode::OutputSteam => stack.id() == Some(STEAM),
            ValveMode::OutputCoolant => is_cooled_coolant(self.registry, stack.id()),
        }
    }

    fn insert_tank(&mut self, tank: usize, stack: &ChemicalStack, simulate: bool) -> ChemicalStack {
        if !self.formed || !self.side || self.mode != ValveMode::Input || stack.is_empty() || !self.is_valid(tank, stack) {
            return stack.clone();
        }
        if !self.stack.is_empty() && !ChemicalStack::is_same_chemical(&self.stack, stack) {
            return stack.clone();
        }
        let took = stack.amount().min(self.capacity.saturating_sub(self.stack.amount().max(0)).max(0));
        if took <= 0 {
            return stack.clone();
        }
        if !simulate {
            self.stack = if self.stack.is_empty() { stack.copy_with_amount(took) } else { self.stack.copy_with_amount(self.stack.amount() + took) };
        }
        stack.copy_with_amount(stack.amount() - took).normalized()
    }

    fn extract_tank(&mut self, _: usize, amount: i64, simulate: bool) -> ChemicalStack {
        if !self.formed || !self.side || self.mode == ValveMode::Input || self.stack.is_empty() || amount <= 0 {
            return ChemicalStack::EMPTY;
        }
        let n = amount.min(self.stack.amount());
        let got = self.stack.copy_with_amount(n);
        if !simulate {
            self.stack = self.stack.copy_with_amount(self.stack.amount() - n).normalized();
        }
        got
    }
}

/// `Capabilities.CHEMICAL` of a boiler valve at `pos` through `face`: None for any other block.
pub(super) fn chemical_handler<R>(
    game: &Game<'_>,
    dimension: &str,
    pos: Pos,
    block: &str,
    face: Option<Direction>,
    registry: &ChemicalRegistry,
    use_it: impl FnOnce(&mut dyn ChemicalHandler) -> R,
) -> Option<R> {
    let owner = valve_of(dimension, pos, block)?;
    let mode = mode_at(game, pos);
    let tank = match mode {
        ValveMode::Input => 1,
        ValveMode::OutputSteam => 0,
        ValveMode::OutputCoolant => 2,
    };
    let held = owner.and_then(|id| {
        let (caps, _) = capacities(dimension, id)?;
        let stack = mb::with_cache(dimension, Kind::Boiler, id, |c| c.chemicals.get(tank).cloned())??;
        Some((id, [caps.steam, caps.heated, caps.cooled][tank], stack))
    });
    let mut handler = ValveChemical {
        stack: held.as_ref().map_or(ChemicalStack::EMPTY, |h| h.2.clone()),
        capacity: held.as_ref().map_or(0, |h| h.1),
        mode,
        formed: held.is_some(),
        side: face.is_some(),
        registry,
    };
    let answer = use_it(&mut handler);
    if let Some((id, _, stack)) = held
        && handler.stack != stack
    {
        let now = handler.stack;
        mb::with_cache(dimension, Kind::Boiler, id, |c| {
            if let Some(s) = c.chemicals.get_mut(tank) {
                *s = now;
            }
        });
    }
    Some(answer)
}

/// The Configurator's sneaking use on a valve (`onSneakRightClick`): its next mode, said on the action bar.
pub(super) fn turn_valve(game: &mut Game<'_>, pos: Pos) -> Option<ValveMode> {
    if game.native_block(pos) != VALVE {
        return None;
    }
    let next = mode_at(game, pos).next();
    game.native_set_block_property_with(pos, "mode", next.name(), flags::UPDATE_ALL);
    let dimension = game.native_dimension().to_owned();
    MODES.with(|m| m.borrow_mut().insert((dimension, pos), next));
    let player = game.native_acting_id();
    game.native_action_bar(player, Text::literal(format!("Valve mode changed to: {}", next.text())));
    Some(next)
}

// ---- The screen (`GuiThermoelectricBoiler`, `GuiBoilerStats`) ---------------------------------------------------

/// The screen's data slots, 16-bit pieces.
const DATA: usize = 2 + 2 + 4 + 4 + 1 + 4 + 4 + 1 + 4 + 4 + 2 + 2 + 2 + 2 + 2 + 4;

fn push(out: &mut Vec<i32>, v: u64, words: usize) {
    out.extend((0..words).map(|i| ((v >> (16 * i)) & 0xFFFF) as i32));
}

fn read(data: &[i32], at: usize, words: usize) -> u64 {
    (0..words).fold(0u64, |v, i| v | (u64::from(data[at + i] as u16) << (16 * i)))
}

struct View {
    water: u64,
    water_cap: u64,
    steam: u64,
    steam_cap: u64,
    heated: (i32, u64, u64),
    cooled: (i32, u64, u64),
    temperature: f64,
    loss: f64,
    boil_rate: u64,
    max_boil: u64,
    elements: u64,
    boil_capacity: u64,
}

fn unpack(data: &[i32]) -> Option<View> {
    if data.len() < DATA {
        return None;
    }
    let float = |at: usize| f64::from(f32::from_bits(read(data, at, 2) as u32));
    Some(View {
        water: read(data, 0, 2),
        water_cap: read(data, 2, 2),
        steam: read(data, 4, 4),
        steam_cap: read(data, 8, 4),
        heated: (data[12], read(data, 13, 4), read(data, 17, 4)),
        cooled: (data[21], read(data, 22, 4), read(data, 26, 4)),
        temperature: float(30),
        loss: float(32),
        boil_rate: read(data, 34, 2),
        max_boil: read(data, 36, 2),
        elements: read(data, 38, 2),
        boil_capacity: read(data, 40, 4),
    })
}

/// The data slots of a formed boiler, for `mek_multiblock` to show on its casings and valves.
pub(super) fn view(game: &Game<'_>, dimension: &str, id: Uuid) -> Vec<i32> {
    let Some((caps, shape)) = capacities(dimension, id) else { return Vec::new() };
    let Some((water, chemicals, heat)) = mb::with_cache(dimension, Kind::Boiler, id, |c| (c.fluids.first().cloned(), c.chemicals.clone(), c.heat.first().copied().flatten())) else {
        return Vec::new();
    };
    if chemicals.len() < 3 {
        return Vec::new();
    }
    let registry = mek_chemicals::registry(game);
    let state = STATES.with(|s| s.borrow().get(&id).copied()).unwrap_or_default();
    let temperature = heat.map_or(state.ambient, |h: Heat| h.temperature());
    let mut out = Vec::with_capacity(DATA);
    push(&mut out, water.map_or(0, |w| w.amount().max(0) as u64), 2);
    push(&mut out, caps.water.max(0) as u64, 2);
    push(&mut out, chemicals[0].amount().max(0) as u64, 4);
    push(&mut out, caps.steam.max(0) as u64, 4);
    for (stack, cap) in [(&chemicals[1], caps.heated), (&chemicals[2], caps.cooled)] {
        out.push(registry.number(stack.id()));
        push(&mut out, stack.amount().max(0) as u64, 4);
        push(&mut out, cap.max(0) as u64, 4);
    }
    push(&mut out, u64::from((temperature as f32).to_bits()), 2);
    push(&mut out, u64::from((state.last.environment_loss as f32).to_bits()), 2);
    push(&mut out, state.last.boil_rate.max(0) as u64, 2);
    push(&mut out, state.last.max_boil.max(0) as u64, 2);
    push(&mut out, shape.elements.max(0) as u64, 2);
    push(&mut out, settings().boil_capacity(shape.elements).max(0) as u64, 4);
    out
}

const WIDTH: f32 = 218.0;
const HEATED_GAUGE: Rect = [6.0, 13.0, 18.0, 60.0];
const WATER_GAUGE: Rect = [26.0, 13.0, 18.0, 60.0];
const STEAM_GAUGE: Rect = [174.0, 13.0, 18.0, 60.0];
const COOLED_GAUGE: Rect = [194.0, 13.0, 18.0, 60.0];
const BOIL_BAR: Rect = [44.0, 13.0, 8.0, 60.0];
const MAX_BAR: Rect = [166.0, 13.0, 8.0, 60.0];
const SCREEN: Rect = [54.0, 23.0, 110.0, 40.0];
const HEAT_TAB: Rect = [-26.0, 109.0, 26.0, 26.0];
/// `GuiBoilerTab`: the statistics tab, left of the panel.
const STATS_TAB: Rect = [-26.0, 6.0, 26.0, 26.0];
const STATS_BUTTON: Rect = [-21.0, 10.0, 18.0, 18.0];
/// The statistics tab switches the page on the client (Mekanism opens `GuiBoilerStats`); the press is taken.
pub(super) const STATS: i32 = 7200;

/// `native_screen` of a casing or valve: no slots of its own, the inventory moved right by its wider panel.
pub(super) fn screen() -> NativeScreen {
    PAGE.with(|p| p.set(0));
    NativeScreen { width: WIDTH, height: 166.0, slots: vec![None; 5], inventory_y: 84.0, inventory_x: 29.0, own_labels: true, own_background: false }
}

pub(super) fn click(at: [f32; 2], button: &mut Option<i32>) {
    if mek_ui::inside(STATS_BUTTON, at) {
        PAGE.with(|p| p.set(1 - p.get().min(1)));
        *button = Some(STATS);
    }
}

fn lines(view: &View, page: u8) -> Vec<String> {
    if page == 1 {
        return vec![
            format!("Max Water: {} mB", display::thousands(view.water_cap)),
            format!("Max Steam: {} mB", display::thousands(view.steam_cap)),
            format!("Superheaters: {}", view.elements),
            format!("Boil Capacity: {} mB/t", display::thousands(view.boil_capacity)),
        ];
    }
    vec![
        format!("Temp: {}", display::temperature(view.temperature)),
        format!("Boil Rate: {} mB/t", display::thousands(view.boil_rate)),
        format!("Max Boil: {} mB/t", display::thousands(view.max_boil)),
    ]
}

pub(super) fn tooltip(data: &[i32], at: [f32; 2], tips: &mut Vec<String>) {
    if mek_ui::inside(STATS_BUTTON, at) {
        tips.push(if PAGE.with(Cell::get) == 0 { "Boiler Statistics".into() } else { "Main".into() });
        return;
    }
    let Some(view) = unpack(data) else { return };
    let water = || if view.water == 0 { "Empty".to_owned() } else { format!("Water: {} mB", display::thousands(view.water)) };
    let steam = || if view.steam == 0 { "Empty".to_owned() } else { format!("Steam: {} mB", display::thousands(view.steam)) };
    if mek_ui::inside(HEATED_GAUGE, at) {
        tips.extend(["Heated Coolant Tank".to_owned(), mek_ui::chemical_line(view.heated.0, view.heated.1)]);
    } else if mek_ui::inside(WATER_GAUGE, at) {
        tips.extend(["Water Tank".to_owned(), water()]);
    } else if mek_ui::inside(STEAM_GAUGE, at) {
        tips.extend(["Steam Tank".to_owned(), steam()]);
    } else if mek_ui::inside(COOLED_GAUGE, at) {
        tips.extend(["Coolant Tank".to_owned(), mek_ui::chemical_line(view.cooled.0, view.cooled.1)]);
    } else if mek_ui::inside(BOIL_BAR, at) {
        tips.push(format!("Boil Rate: {} mB/t", display::thousands(view.boil_rate)));
    } else if mek_ui::inside(MAX_BAR, at) {
        tips.push(format!("Max Boil: {} mB/t", display::thousands(view.max_boil)));
    } else if mek_ui::inside(HEAT_TAB, at) {
        tips.push(format!("Dissipated: {}/t", display::temperature(view.loss)));
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

/// `GuiVerticalRateBar`: the share from the bottom.
fn rate_bar(pen: &mut Pen<'_, '_>, r: Rect, level: f64) {
    pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], r);
    let h = (level.clamp(0.0, 1.0) * 58.0) as i32 as f32;
    if h > 0.0 {
        pen.sprite("bar/vertical_rate", [0.0, 58.0 - h, 6.0, h], [r[0] + 1.0, r[1] + r[3] - 1.0 - h, 6.0, h]);
    }
}

pub(super) fn draw(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let view = unpack(data);
    let page = PAGE.with(Cell::get);
    let cursor = game.native_cursor();
    let cursor = [cursor[0] - origin[0], cursor[1] - origin[1]];
    let mut pen = Pen { game, batch, origin, cursor };
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [0.0, 0.0, WIDTH, 166.0]);
    pen.holder(STATS_TAB, true);
    pen.image_button("stats", 18.0, STATS_BUTTON, true);
    pen.whole("tabs/heat_info_k", [26.0, 26.0], HEAT_TAB);
    if let Some(v) = &view {
        gauge(&mut pen, HEATED_GAUGE, v.heated.1, v.heated.2, 0xFF00_0000 | mek_ui::chemical_color(game, v.heated.0));
        gauge(&mut pen, WATER_GAUGE, v.water, v.water_cap, 0xFF3F_76E4);
        gauge(&mut pen, STEAM_GAUGE, v.steam, v.steam_cap, 0xFF00_0000 | mek_chemicals::registry(game).get(STEAM).map_or(0xC0C0C0, |c| c.color & 0xFF_FFFF));
        gauge(&mut pen, COOLED_GAUGE, v.cooled.1, v.cooled.2, 0xFF00_0000 | mek_ui::chemical_color(game, v.cooled.0));
        rate_bar(&mut pen, BOIL_BAR, if v.max_boil == 0 { 0.0 } else { v.boil_rate as f64 / v.max_boil as f64 });
        let config = settings();
        let full = v.elements as f64 * config.superheating_heat_transfer;
        rate_bar(&mut pen, MAX_BAR, if full <= 0.0 { 0.0 } else { v.max_boil as f64 * config.water_thermal_enthalpy / full });
    } else {
        for r in [HEATED_GAUGE, WATER_GAUGE, STEAM_GAUGE, COOLED_GAUGE] {
            gauge(&mut pen, r, 0, 0, 0);
        }
        rate_bar(&mut pen, BOIL_BAR, 0.0);
        rate_bar(&mut pen, MAX_BAR, 0.0);
    }
    pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], SCREEN);
    if let Some(v) = &view {
        for (i, line) in lines(v, page).iter().enumerate() {
            pen.text(line, [SCREEN[0] + 3.0, SCREEN[1] + 3.0 + 9.0 * i as f32], mek_ui::SCREEN_TEXT);
        }
    }
    for row in 0..4 {
        let y = if row == 3 { 141.0 } else { 83.0 + 18.0 * row as f32 };
        for col in 0..9 {
            pen.slot("normal", None, 28.0 + 18.0 * col as f32, y);
        }
    }
    pen.centered("Thermoelectric Boiler", 0.0, WIDTH, 5.0, mek_ui::TITLE);
    pen.text("Inventory", [29.0, 74.0], mek_ui::TITLE);
}

// ---- /mek probe -------------------------------------------------------------------------------------------------

pub(super) fn probe(game: &Game<'_>, dimension: &str, id: Uuid) -> Value {
    let (caps, shape) = capacities(dimension, id).unwrap_or_default();
    let state = STATES.with(|s| s.borrow().get(&id).copied()).unwrap_or_default();
    let (water, chemicals, heat) = mb::with_cache(dimension, Kind::Boiler, id, |c| {
        (
            c.fluids.first().map(|f| json!({ "fluid": (!f.is_empty()).then(|| f.name()), "amount": f.amount().max(0) })),
            c.chemicals.iter().map(|s| json!({ "chemical": s.id(), "amount": s.amount().max(0) })).collect::<Vec<_>>(),
            c.heat.first().copied().flatten(),
        )
    })
    .unwrap_or_default();
    let valves: Vec<Value> = mb::with_formed(dimension, id, |f| f.valves.clone())
        .unwrap_or_default()
        .into_iter()
        .map(|(p, side)| json!({ "pos": p, "side": side.name(), "mode": mode_at(game, p).name() }))
        .collect();
    let config = settings();
    json!({
        "water": water, "steam": chemicals.first(), "heated_coolant": chemicals.get(1), "cooled_coolant": chemicals.get(2),
        "capacities": { "water": caps.water, "steam": caps.steam, "heated_coolant": caps.heated, "cooled_coolant": caps.cooled },
        "water_volume": shape.water_volume, "steam_volume": shape.steam_volume, "superheaters": shape.elements,
        "disperser_y": shape.disperser_y, "boil_capacity": config.boil_capacity(shape.elements),
        "heat": heat.map(|h| h.stored), "heat_capacity": heat.map(|h| h.capacity), "temperature": heat.map(|h| h.temperature()),
        "ambient": state.ambient, "hot": state.last.hot, "boil_rate": state.last.boil_rate, "max_boil": state.last.max_boil,
        "environment_loss": state.last.environment_loss, "cooled": state.last.cooled, "valve_modes": valves,
        "config": { "water_per_tank": config.water_per_tank, "steam_per_tank": config.steam_per_tank,
            "heated_coolant_per_tank": config.heated_coolant_per_tank, "cooled_coolant_per_tank": config.cooled_coolant_per_tank,
            "water_conductivity": config.water_conductivity, "superheating_heat_transfer": config.superheating_heat_transfer,
            "water_thermal_enthalpy": config.water_thermal_enthalpy },
    })
}
