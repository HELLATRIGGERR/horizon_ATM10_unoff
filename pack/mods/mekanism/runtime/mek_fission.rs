// Mekanism Generators' Fission Reactor in the game (support module `crate::game::mek_fission` of the `mekanism`
// native bundle). The rules are `mekanism_energy::fission` (MIT, from Mekanism Generators 10.7.19, Copyright (c)
// 2017-2025 Aidan C. Brady); the reactor forms through `mek_multiblock` (`Kind::Fission`: fission reactor casings,
// ports, logic adapters and reactor glass, fuel and control rod assemblies inside), and its tanks, heat and core
// (damage, rate limit, burn and waste remainders, active, force disabled) live in the cache by its id there
// (`chemical_tanks`: fissile fuel, heated coolant, nuclear waste, a cooled chemical coolant; `fluid_tanks`: water;
// `heat_capacitors`). This module is the glue: its settings from `generators.toml` `[fission_reactor]`, the air of
// its corners' biomes, its tick (`FissionReactorMultiblockData.tick`: the logic adapters' power, the burn, the
// coolant, the ports' output, the air, the damage), its meltdowns (`MeltdownLevelData`: a blast inside it, now and
// then for 100 ticks), the ports' fluid handler (water in their input mode) and chemical handler by their mode
// (`FissionPortMode`, the block's `mode`), the Configurator's turn of a port's mode, the logic adapters (their mode,
// redstone control, status and the signal they give out of the reactor), its screens (`GuiFissionReactor`,
// `GuiFissionReactorStats` as a page, `GuiFissionReactorLogicAdapter`), `/mek fission` and `/mek probe`.
// Waste with no room and a meltdown's radioactive tanks radiate at the reactor's centre (`mek_radiation`); the
// living entities in its core aren't irradiated yet.

use super::{
    Game, mek_chemicals,
    mek_multiblock::{self as mb, Formed, Kind},
    mek_ui::{self, Pen, Rect},
    native_explosions::NativeBlast,
    native_hooks::{NativeFluidTankInfo, NativeScreen},
};
use horizon_client::{containers::ModBlockEntity, hud::HudBatch, text::Text};
use horizon_core::geometry::Direction as Face;
use horizon_world::{
    fluid_stack::FluidStack,
    level::flags,
    nbt::{Compound, JavaString, Tag},
};
use mekanism_energy::{
    chemical::{ChemicalHandler, ChemicalRegistry, ChemicalStack},
    config, display, evaporation,
    fission::{self, Capacities, Coolant, CooledCoolant, Core, FissionConfig, Inside, Logic, PortMode, RedstoneControl, Shape, Status, Tanks, Watch},
    multiblock::{Cuboid, Failure, Formation, Node, Pos, Uuid},
    sides::Direction,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap},
    time::{Duration, Instant},
};

pub(super) const CASING: &str = "mekanismgenerators:fission_reactor_casing";
pub(super) const PORT: &str = "mekanismgenerators:fission_reactor_port";
pub(super) const ADAPTER: &str = "mekanismgenerators:fission_reactor_logic_adapter";
pub(super) const GLASS: &str = "mekanismgenerators:reactor_glass";
const FUEL_ASSEMBLY: &str = "mekanismgenerators:fission_fuel_assembly";
const ROD: &str = "mekanismgenerators:control_rod_assembly";
const STEAM: &str = "mekanism:steam";
const FUEL: &str = "mekanism:fissile_fuel";
const WASTE: &str = "mekanism:nuclear_waste";

/// What a formed reactor keeps beside its cache (not saved: made again when it forms).
#[derive(Clone, Copy, Debug, Default)]
struct State {
    /// `biomeAmbientTemp`.
    ambient: f64,
    last: fission::Tick,
}

/// A logic adapter as its reactor's last tick left it: `prevStatus`, and whether it gives a signal out of `cuboid`.
#[derive(Clone, Copy, Debug)]
struct Adapter {
    status: Status,
    cuboid: Cuboid,
}

/// `Meltdown`: blasts inside a reactor that melted down, for 100 ticks.
#[derive(Clone, Debug)]
struct Meltdown {
    dimension: String,
    min: Pos,
    max: Pos,
    magnitude: f64,
    radius: f32,
    id: Uuid,
    age: i32,
}

thread_local! {
    static SETTINGS: RefCell<Option<(FissionConfig, config::Stamp, Instant)>> = const { RefCell::new(None) };
    static STATES: RefCell<HashMap<Uuid, State>> = RefCell::new(HashMap::new());
    /// The screen's page on the client: 0 the reactor, 1 its statistics (`GuiFissionReactorStats`).
    static PAGE: Cell<u8> = const { Cell::new(0) };
    /// Each formed reactor's ports' modes as its last tick read their blocks, for the fluid handler (whose hook
    /// point has no game at hand).
    static MODES: RefCell<HashMap<(String, Pos), PortMode>> = RefCell::new(HashMap::new());
    /// The logic adapters of formed reactors, for the redstone hook points (which read no world).
    static ADAPTERS: RefCell<HashMap<(String, Pos), Adapter>> = RefCell::new(HashMap::new());
    /// Adapters whose neighbours hear of a change at the next level tick (their reactor broke with no game at hand).
    static RELEASED: RefCell<Vec<(String, Pos)>> = const { RefCell::new(Vec::new()) };
    static MELTDOWNS: RefCell<Vec<Meltdown>> = const { RefCell::new(Vec::new()) };
}

fn key(name: &str) -> JavaString {
    JavaString::from(name)
}

/// `generators.toml` `[fission_reactor]` (and `general.toml`'s `maxEnergyPerSteam`), read again within a second of a
/// change.
pub(super) fn settings() -> FissionConfig {
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
                Some(d) => (FissionConfig::load(d), config::stamp(d), Instant::now()),
                None => (FissionConfig::default(), [None; config::FILES.len()], Instant::now()),
            });
        }
        s.as_ref().map(|(c, _, _)| *c).unwrap_or_default()
    })
}

/// A casing or port, whose screen shows the reactor.
pub(super) fn is_reactor_block(block: &str) -> bool {
    block == CASING || block == PORT
}

pub(super) fn is_adapter(block: &str) -> bool {
    block == ADAPTER
}

/// A world opens or closes.
pub(super) fn clear() {
    STATES.with(|s| s.borrow_mut().clear());
    MODES.with(|m| m.borrow_mut().clear());
    ADAPTERS.with(|a| a.borrow_mut().clear());
    RELEASED.with(|r| r.borrow_mut().clear());
    MELTDOWNS.with(|m| m.borrow_mut().clear());
}

/// `native_block_entity` of a logic adapter: its mode and redstone control (`DISABLED` until it loads).
pub(super) fn new_entity(block: &str, entity: &mut Option<(String, Compound)>) -> bool {
    if block != ADAPTER {
        return false;
    }
    let mut nbt = Compound::new();
    nbt.insert(key("logic_type"), Tag::Int(0));
    nbt.insert(key("control_type"), Tag::Int(0));
    nbt.insert(key("redstone"), Tag::Byte(0));
    *entity = Some((block.to_owned(), nbt));
    true
}

/// `FormationProtocol.doUpdate` with the `FissionReactorValidator` over the structure `nodes`.
pub(super) fn form(game: &mut Game<'_>, nodes: &BTreeMap<Pos, Node>) -> Result<(Formation, Shape), Failure> {
    fission::form(nodes, |p| {
        let block = game.native_block(p);
        match block.as_str() {
            FUEL_ASSEMBLY => Inside::Fuel,
            ROD => Inside::Rod,
            b if mb::is_air(b) => Inside::Air,
            _ => Inside::Invalid,
        }
    })
}

fn shape_of(dimension: &str, id: Uuid) -> Option<Shape> {
    mb::with_formed(dimension, id, |f| f.fission.clone())?
}

fn mode_at(game: &Game<'_>, pos: Pos) -> PortMode {
    game.native_block_property(pos, "mode").and_then(|m| PortMode::from_name(&m)).unwrap_or_default()
}

/// A port's mode as its reactor last read it.
fn known_mode(dimension: &str, pos: Pos) -> PortMode {
    MODES.with(|m| m.borrow().get(&(dimension.to_owned(), pos)).copied()).unwrap_or_default()
}

fn note_modes(game: &Game<'_>, dimension: &str, valves: &[(Pos, Direction)]) -> Vec<(Pos, Direction, PortMode)> {
    let modes: Vec<(Pos, Direction, PortMode)> = valves.iter().map(|(p, side)| (*p, *side, mode_at(game, *p))).collect();
    MODES.with(|m| {
        let mut m = m.borrow_mut();
        for (p, _, mode) in &modes {
            m.insert((dimension.to_owned(), *p), *mode);
        }
    });
    modes
}

fn adapters_of(game: &Game<'_>, formed: &Formed) -> Vec<Pos> {
    formed.locations.iter().copied().filter(|p| game.native_block(*p) == ADAPTER).collect()
}

/// `onCreated`: the air of its eight corners, its heat capacity (`casingHeatCapacity` a wall block, the cache's heat
/// coming in), the tanks capped to its size, and the cache's core applied (`FissionReactorCache.apply`: the rate
/// limit within its max burn rate, the force disable before the active flag).
pub(super) fn created(game: &mut Game<'_>, dimension: &str, id: Uuid) {
    let Some(formed) = mb::with_formed(dimension, id, Clone::clone) else { return };
    let Some(shape) = formed.fission.clone() else { return };
    let config = settings();
    let caps = config.capacities(&shape);
    let c = formed.cuboid;
    let corners: Vec<f32> = [c.min[0], c.max[0]]
        .into_iter()
        .flat_map(|x| [c.min[1], c.max[1]].into_iter().flat_map(move |y| [c.min[2], c.max[2]].into_iter().map(move |z| [x, y, z])))
        .filter_map(|p| game.native_biome_climate(p).map(|b| b.temperature))
        .collect();
    let ambient = evaporation::ambient(&corners);
    mb::with_cache(dimension, Kind::Fission, id, |cache| {
        let heat = evaporation::Heat::resized(cache.heat.first().copied().flatten(), config.heat_capacity(formed.locations.len()), ambient);
        if let Some(slot) = cache.heat.first_mut() {
            *slot = Some(heat);
        }
        if let Some(water) = cache.fluids.first_mut()
            && water.amount() > caps.cooled
        {
            *water = water.copy_with_amount(caps.cooled);
        }
        for (tank, cap) in cache.chemicals.iter_mut().zip([caps.fuel, caps.heated, caps.waste, i64::from(caps.cooled)]) {
            if tank.amount() > cap {
                tank.set_amount(cap);
            }
        }
        cache.reactor = Some(cache.reactor.unwrap_or_else(Core::new).applied(config.max_burn_rate(shape.assemblies), &config));
    });
    STATES.with(|s| s.borrow_mut().insert(id, State { ambient, ..State::default() }));
    note_modes(game, dimension, &formed.valves);
    let adapters = adapters_of(game, &formed);
    ADAPTERS.with(|a| {
        let mut a = a.borrow_mut();
        for p in adapters {
            a.insert((dimension.to_owned(), p), Adapter { status: Status::Idle, cuboid: formed.cuboid });
        }
    });
}

/// `remove`: the reactor broke; its adapters stop giving a signal (their neighbours hear of it at the next level
/// tick).
pub(super) fn removed(dimension: &str, formed: &Formed) {
    STATES.with(|s| s.borrow_mut().remove(&formed.id));
    MODES.with(|m| {
        let mut m = m.borrow_mut();
        for (p, _) in &formed.valves {
            m.remove(&(dimension.to_owned(), *p));
        }
    });
    let gone: Vec<(String, Pos)> = ADAPTERS.with(|a| {
        let mut a = a.borrow_mut();
        let mine: Vec<(String, Pos)> = a.keys().filter(|(d, p)| d == dimension && formed.locations.contains(p)).cloned().collect();
        for k in &mine {
            a.remove(k);
        }
        mine
    });
    RELEASED.with(|r| r.borrow_mut().extend(gone));
}

/// A logic adapter's mode, redstone control (`supportedOrNextType`: `DISABLED` reads as `HIGH`, as it does once the
/// adapter has loaded) and whether it was powered (`redstone`).
fn adapter_nbt(game: &mut Game<'_>, p: Pos) -> (Logic, RedstoneControl, bool) {
    game.native_with_mod_block_entity(p, |m| {
        let number = |k: &str| super::mek_world::number(m.nbt.get(&key(k))).unwrap_or(0);
        (Logic::by_id(number("logic_type") as i32), RedstoneControl::by_id(number("control_type") as i32).supported(), number("redstone") != 0)
    })
    .unwrap_or_default()
}

/// The chemical coolant a stack is (`CooledCoolant`) and the heated variant it boils into.
fn cooled_coolant(registry: &ChemicalRegistry, stack: &ChemicalStack) -> Option<(CooledCoolant, String)> {
    let c = registry.get(stack.id()?)?.cooled.as_ref()?;
    Some((CooledCoolant { thermal_enthalpy: c.thermal_enthalpy, conductivity: c.conductivity }, c.other.clone()))
}

fn is_cooled_coolant(registry: &ChemicalRegistry, stack: &ChemicalStack) -> bool {
    stack.id().and_then(|i| registry.get(i)).is_some_and(|c| c.cooled.is_some())
}

/// `FissionReactorMultiblockData.tick`, with its logic adapters' `updatePower` first (`onPowerChange`: an
/// adapter in activation mode turns the reactor on or off as its redstone control says) and their `onUpdateServer`
/// after (a status that changed tells the neighbours): the burn while active, the coolant, the heated coolant out of
/// its output coolant ports and the waste out of its output waste ports (`ChemicalUtil.emit`, split among the
/// blocks they face), the air, the heat landing, the damage, and a meltdown when it comes.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, id: Uuid) {
    let Some(formed) = mb::with_formed(dimension, id, Clone::clone) else { return };
    let Some(shape) = formed.fission.clone() else { return };
    let Some(before) = mb::with_cache(dimension, Kind::Fission, id, |c| c.clone()) else { return };
    if before.fluids.is_empty() || before.chemicals.len() < 4 || before.heat.is_empty() {
        return;
    }
    let config = settings();
    let caps = config.capacities(&shape);
    let registry = mek_chemicals::registry(game);
    let mut state = STATES.with(|s| s.borrow().get(&id).copied()).unwrap_or(State { ambient: evaporation::AMBIENT, ..State::default() });
    let mut cache = before.clone();
    let mut core = cache.reactor.unwrap_or_else(Core::new);
    let adapters = adapters_of(game, &formed);
    for &p in &adapters {
        let powered = game.native_neighbor_signal(dimension, p) > 0;
        let (logic, control, was) = adapter_nbt(game, p);
        if powered != was {
            game.native_with_mod_block_entity(p, |m| {
                m.nbt.insert(key("redstone"), Tag::Byte(i8::from(powered)));
            });
            if logic == Logic::Activation {
                core.set_active(control.activated(powered, was));
            }
        }
    }
    let saved = cache.heat[0].unwrap_or_else(|| evaporation::Heat::resized(None, config.heat_capacity(formed.locations.len()), state.ambient));
    let mut heat = fission::Heat { stored: saved.stored, capacity: saved.capacity };
    let chemicals = &cache.chemicals;
    let mut tanks = Tanks {
        fuel: chemicals[0].amount().max(0),
        heated: chemicals[1].amount().max(0),
        waste: chemicals[2].amount().max(0),
        coolant: chemicals[3].amount().max(0),
        water: cache.fluids[0].amount().max(0),
    };
    let chemical = cooled_coolant(&registry, &chemicals[3]);
    let product = if tanks.water > 0 { Some(STEAM.to_owned()) } else { chemical.as_ref().map(|c| c.1.clone()) };
    let heated_takes = product.as_deref().is_some_and(|p| chemicals[1].is_empty() || chemicals[1].id() == Some(p));
    let coolant = Coolant { chemical: chemical.as_ref().map(|c| c.0), heated_takes };
    let handed = mb::take_heat(id);
    let last = fission::tick(&mut tanks, &mut core, &mut heat, &caps, &shape, coolant, state.ambient, handed, super::mek_machines::draw_random, &config);
    state.last = last;
    cache.heat[0] = Some(evaporation::Heat { stored: heat.stored, capacity: heat.capacity });
    cache.fluids[0] = if tanks.water > 0 { cache.fluids[0].copy_with_amount(tanks.water) } else { FluidStack::EMPTY };
    let set = |stack: &ChemicalStack, amount: i64, id: Option<&str>| match (stack.is_empty(), id) {
        _ if amount <= 0 => ChemicalStack::EMPTY,
        (true, Some(id)) => ChemicalStack::new(id, amount),
        _ => stack.copy_with_amount(amount),
    };
    cache.chemicals[0] = set(&cache.chemicals[0], tanks.fuel, Some(FUEL));
    cache.chemicals[1] = set(&cache.chemicals[1], tanks.heated, product.as_deref());
    cache.chemicals[2] = set(&cache.chemicals[2], tanks.waste, Some(WASTE));
    cache.chemicals[3] = set(&cache.chemicals[3], tanks.coolant, None);
    cache.reactor = Some(core);
    if cache != before {
        mb::with_cache(dimension, Kind::Fission, id, |c| *c = cache.clone());
    }
    // The ports' output: the heated coolant, then the waste.
    let modes = note_modes(game, dimension, &formed.valves);
    for (tank, mode) in [(1, PortMode::OutputCoolant), (2, PortMode::OutputWaste)] {
        let stack = cache.chemicals[tank].clone();
        if stack.is_empty() {
            continue;
        }
        let targets: Vec<(Pos, Direction)> =
            modes.iter().filter(|(_, _, m)| *m == mode).map(|(p, side, _)| (super::mek_world::beside(*p, *side), side.opposite())).collect();
        let sent = mek_chemicals::emit_to(game, &targets, &stack);
        if sent > 0 {
            mb::with_cache(dimension, Kind::Fission, id, |c| {
                if let Some(s) = c.chemicals.get_mut(tank) {
                    *s = s.copy_with_amount(s.amount() - sent).normalized();
                }
            });
        }
    }
    // Waste with no room in its tank radiates at the reactor's centre.
    if last.waste_overflow > 0
        && super::mek_radiation::enabled()
        && let Some(waste) = registry.get(WASTE)
        && waste.radioactivity > 0.0
    {
        super::mek_radiation::radiate(dimension, center(&formed.cuboid), last.waste_overflow as f64 * waste.radioactivity);
    }
    if last.meltdown {
        MELTDOWNS.with(|m| {
            m.borrow_mut().push(Meltdown {
                dimension: dimension.to_owned(),
                min: formed.cuboid.min,
                max: formed.cuboid.max,
                magnitude: heat.stored,
                radius: config.meltdown_radius,
                id,
                age: 0,
            });
        });
    }
    STATES.with(|s| s.borrow_mut().insert(id, state));
    // The adapters' statuses.
    let (waste, fuel_empty) =
        mb::with_cache(dimension, Kind::Fission, id, |c| (c.chemicals[2].amount().max(0), c.chemicals[0].is_empty())).unwrap_or((0, true));
    let watch = Watch { temperature: heat.temperature(), waste, waste_capacity: caps.waste, damage: core.damage, fuel_empty };
    for &p in &adapters {
        let (logic, control, powered) = adapter_nbt(game, p);
        let status = fission::status(logic, &watch, control.activated(powered, powered), &config);
        let changed = ADAPTERS.with(|a| {
            let mut a = a.borrow_mut();
            let entry = a.entry((dimension.to_owned(), p)).or_insert(Adapter { status: Status::Idle, cuboid: formed.cuboid });
            let changed = entry.status != status;
            entry.status = status;
            entry.cuboid = formed.cuboid;
            changed
        });
        if changed {
            game.native_update_signal_neighbors(dimension, p);
        }
    }
}

/// `native_level_tick_post`: the neighbours of adapters whose reactor broke hear of it; meltdowns blast now and then
/// (`Meltdown.update`: one draw in ten, then the heat it held times `EXPLOSION_CHANCE`), each at a random block of its
/// box with fire (`meltdowns.radius`), the reactor it was taking its meltdown (`meltdownHappened`) when the blast
/// breaks one of its blocks; a meltdown ends after 100 ticks.
pub(super) fn level_ticked(game: &mut Game<'_>, dimension: &str) {
    let released: Vec<Pos> = RELEASED.with(|r| {
        let mut r = r.borrow_mut();
        let (mine, rest): (Vec<_>, Vec<_>) = r.drain(..).partition(|(d, _)| d == dimension);
        *r = rest;
        mine.into_iter().map(|(_, p)| p).collect()
    });
    for p in released {
        game.native_update_signal_neighbors(dimension, p);
    }
    let mine: Vec<Meltdown> = MELTDOWNS.with(|m| {
        let mut m = m.borrow_mut();
        let (mine, rest): (Vec<_>, Vec<_>) = m.drain(..).partition(|x| x.dimension == dimension);
        *m = rest;
        mine
    });
    let mut kept = Vec::new();
    for mut m in mine {
        m.age += 1;
        let draw = super::mek_machines::draw_random;
        let next_int = (draw() * 4_294_967_296.0) as i64 - 2_147_483_648;
        if fission::meltdown_blasts(next_int as i32, draw(), m.magnitude) {
            let at = |lo: i32, hi: i32| lo + (draw() * f64::from(hi - lo + 1)) as i32;
            let center = [f64::from(at(m.min[0], m.max[0])), f64::from(at(m.min[1], m.max[1])), f64::from(at(m.min[2], m.max[2]))];
            let parts = mb::with_formed(dimension, m.id, |f| f.locations.clone());
            game.native_explode(dimension, center, m.radius, true, NativeBlast::Block);
            // `BlockMekanism.onBlockExploded`: a block of the reactor the blast took gives it its meltdown, and it
            // breaks then (`setRemoved`), not when its block entities go at the end of the tick.
            if let Some(parts) = parts
                && parts.iter().any(|p| !is_part(&game.native_block(*p)))
            {
                meltdown_happened(dimension, m.id);
                mb::break_now(game, dimension, m.id);
            }
        }
        if m.age < fission::MELTDOWN_DURATION {
            kept.push(m);
        }
    }
    MELTDOWNS.with(|m| m.borrow_mut().extend(kept));
}

/// `VoxelCuboid.getCenter`: the middle block of the reactor's box (Java's division, toward zero).
fn center(cuboid: &Cuboid) -> [i32; 3] {
    [0, 1, 2].map(|i| (cuboid.min[i] + cuboid.max[i]) / 2)
}

/// A wall block of a reactor: a casing, a port, a logic adapter or reactor glass.
fn is_part(block: &str) -> bool {
    matches!(block, CASING | PORT | ADAPTER | GLASS)
}

/// `meltdownHappened` on the reactor `id` while it is formed (the blast hits its blocks).
fn meltdown_happened(dimension: &str, id: Uuid) {
    let Some(shape) = shape_of(dimension, id) else { return };
    let Some(cuboid) = mb::with_formed(dimension, id, |f| f.cuboid) else { return };
    let config = settings();
    let ambient = STATES.with(|s| s.borrow().get(&id).map_or(evaporation::AMBIENT, |s| s.ambient));
    let _ = shape;
    mb::with_cache(dimension, Kind::Fission, id, |c| {
        let Some(h) = c.heat.first().copied().flatten() else { return };
        let mut heat = fission::Heat { stored: h.stored, capacity: h.capacity };
        let mut core = c.reactor.unwrap_or_else(Core::new);
        let mut tanks = Tanks { fuel: c.chemicals[0].amount(), heated: c.chemicals[1].amount(), waste: c.chemicals[2].amount(), ..Tanks::default() };
        // With radiation on (as in ATM10) the radioactive tanks are dumped, radiating at the reactor's centre.
        let mut dump = [false; 2];
        if let Some(registry) = mek_chemicals::loaded()
            && super::mek_radiation::enabled()
        {
            let of = |s: &ChemicalStack| super::mek_radiation::stack_radioactivity(&registry, s);
            let waste_rate = c.chemicals[2].id().or(Some(WASTE)).and_then(|id| registry.get(id)).map_or(0.0, |w| w.radioactivity);
            dump = [of(&c.chemicals[0]) > 0.0, waste_rate > 0.0];
            let radiation = fission::meltdown_radiation(
                of(&c.chemicals[0]),
                waste_rate,
                c.chemicals[2].amount(),
                core.partial_waste,
                [of(&c.chemicals[3]), of(&c.chemicals[1])],
                &config,
            );
            super::mek_radiation::radiate(dimension, center(&cuboid), radiation);
        }
        fission::meltdown_happened(&mut tanks, &mut core, &mut heat, ambient, dump, &config);
        c.chemicals[0] = if tanks.fuel > 0 { c.chemicals[0].copy_with_amount(tanks.fuel) } else { ChemicalStack::EMPTY };
        c.chemicals[1] = ChemicalStack::EMPTY;
        c.chemicals[2] = if tanks.waste > 0 { c.chemicals[2].copy_with_amount(tanks.waste) } else { ChemicalStack::EMPTY };
        c.heat[0] = Some(evaporation::Heat { stored: heat.stored, capacity: heat.capacity });
        c.reactor = Some(core);
    });
}

// ---- The redstone hook points (API 38) --------------------------------------------------------------------------

/// `native_is_signal_source`: a logic adapter (`AttributeRedstoneEmitter`).
pub(super) fn is_signal_source(block: Option<&str>, source: &mut bool) {
    if block == Some(ADAPTER) {
        *source = true;
    }
}

/// `native_block_signal`: an outputting adapter gives 15 to the block it faces outside its reactor
/// (`getRedstoneLevel`), none through blocks (no direct signal).
pub(super) fn block_signal(dimension: &str, block: Option<&str>, pos: Pos, toward: [i32; 3], direct: bool, signal: &mut i32) {
    if block != Some(ADAPTER) {
        return;
    }
    if direct {
        *signal = 0;
        return;
    }
    let reader = [pos[0] + toward[0], pos[1] + toward[1], pos[2] + toward[2]];
    let out = ADAPTERS.with(|a| a.borrow().get(&(dimension.to_owned(), pos)).copied());
    *signal = match out {
        Some(a) if a.status == Status::Outputting && a.cuboid.relative(reader) == mekanism_energy::multiblock::Relative::Outside => 15,
        _ => 0,
    };
}

// ---- The ports' fluid handler (hook points API 422) -------------------------------------------------------------

/// The reactor a port at `pos` reaches: Some(None) for a port of no formed reactor.
fn port_of(dimension: &str, pos: Pos, block: &str) -> Option<Option<Uuid>> {
    (block == PORT).then(|| mb::formed_at(dimension, Kind::Fission, pos))
}

fn capacities(dimension: &str, id: Uuid) -> Option<Capacities> {
    Some(settings().capacities(&shape_of(dimension, id)?))
}

/// `native_fluid_handler_tanks` of a port: in its input mode, once formed, the coolant tank as water (which gives
/// nothing out); none in its output modes.
pub(super) fn fluid_tanks(dimension: &str, pos: Pos, block: &str, tanks: &mut Option<Vec<NativeFluidTankInfo>>) -> bool {
    let Some(owner) = port_of(dimension, pos, block) else { return false };
    let input = known_mode(dimension, pos) == PortMode::Input;
    let held = owner
        .filter(|_| input)
        .and_then(|id| Some((capacities(dimension, id)?, mb::with_cache(dimension, Kind::Fission, id, |c| c.fluids.first().cloned())??)));
    *tanks = Some(match held {
        Some((caps, water)) => vec![NativeFluidTankInfo { stack: water, capacity: caps.cooled, drainable: false }],
        None => Vec::new(),
    });
    true
}

/// `native_fluid_handler_fill`: what fits of water (`FluidTags.WATER`) in the input mode while no chemical coolant
/// is in (`MergedTank`); the null side reads only.
pub(super) fn fluid_fill(dimension: &str, pos: Pos, face: Option<Face>, block: &str, resource: &FluidStack, simulate: bool, filled: &mut Option<i32>) -> bool {
    let Some(owner) = port_of(dimension, pos, block) else { return false };
    *filled = Some(0);
    let (Some(id), Some(_)) = (owner, face) else { return true };
    if known_mode(dimension, pos) != PortMode::Input || resource.is_empty() || !resource.is_tag("minecraft:water") {
        return true;
    }
    let Some(caps) = capacities(dimension, id) else { return true };
    let took = mb::with_cache(dimension, Kind::Fission, id, |c| {
        if c.chemicals.get(3).is_some_and(|s| !s.is_empty()) {
            return 0;
        }
        let Some(tank) = c.fluids.get_mut(0) else { return 0 };
        let same = tank.is_empty() || FluidStack::is_same_fluid_same_components(tank, resource);
        let took = mekanism_energy::dynamic_tank::fluid_room(tank.amount().max(0), caps.cooled, same, false, resource.amount());
        if !simulate && took > 0 {
            *tank = if tank.is_empty() { resource.copy_with_amount(took) } else { tank.copy_with_amount(tank.amount() + took) };
        }
        took
    })
    .unwrap_or(0);
    *filled = Some(took);
    true
}

/// `native_fluid_handler_drain`: the coolant tank gives nothing to other blocks (`VariableCapacityFluidTank.input`).
pub(super) fn fluid_drain(dimension: &str, pos: Pos, block: &str, drained: &mut Option<FluidStack>) -> bool {
    if port_of(dimension, pos, block).is_none() {
        return false;
    }
    *drained = Some(FluidStack::EMPTY);
    true
}

// ---- The ports' chemical handler ----------------------------------------------------------------------------------

/// A port's tanks by its mode (`getChemicalTanks(mode)`): the fuel and the chemical coolant tanks in its input mode
/// (which take and give nothing to other blocks), the waste or the heated coolant tank in its output modes (which
/// give and take nothing from other blocks); the null side reads only.
struct PortChemical<'r> {
    /// (cache tank, stack, capacity)
    tanks: Vec<(usize, ChemicalStack, i64)>,
    mode: PortMode,
    side: bool,
    /// The coolant tank holds water (`MergedTank`: no chemical coolant goes in).
    water: bool,
    registry: &'r ChemicalRegistry,
}

impl ChemicalHandler for PortChemical<'_> {
    fn tanks(&self) -> usize {
        self.tanks.len()
    }

    fn in_tank(&self, tank: usize) -> ChemicalStack {
        self.tanks.get(tank).map_or(ChemicalStack::EMPTY, |t| t.1.clone())
    }

    fn capacity(&self, tank: usize) -> i64 {
        self.tanks.get(tank).map_or(0, |t| t.2)
    }

    fn is_valid(&self, tank: usize, stack: &ChemicalStack) -> bool {
        match self.tanks.get(tank).map(|t| t.0) {
            Some(0) => stack.id() == Some(FUEL),
            Some(1) => stack.id() == Some(STEAM) || self.registry.get(stack.id().unwrap_or_default()).is_some_and(|c| c.heated.is_some()),
            Some(2) => stack.id() == Some(WASTE),
            Some(3) => is_cooled_coolant(self.registry, stack),
            _ => false,
        }
    }

    fn insert_tank(&mut self, tank: usize, stack: &ChemicalStack, simulate: bool) -> ChemicalStack {
        if !self.side || self.mode != PortMode::Input || stack.is_empty() || !self.is_valid(tank, stack) {
            return stack.clone();
        }
        let water = self.water;
        let Some(t) = self.tanks.get_mut(tank) else { return stack.clone() };
        if (t.0 == 3 && water) || (!t.1.is_empty() && !ChemicalStack::is_same_chemical(&t.1, stack)) {
            return stack.clone();
        }
        let took = stack.amount().min(t.2.saturating_sub(t.1.amount().max(0)).max(0));
        if took <= 0 {
            return stack.clone();
        }
        if !simulate {
            t.1 = if t.1.is_empty() { stack.copy_with_amount(took) } else { t.1.copy_with_amount(t.1.amount() + took) };
        }
        stack.copy_with_amount(stack.amount() - took).normalized()
    }

    fn extract_tank(&mut self, tank: usize, amount: i64, simulate: bool) -> ChemicalStack {
        if !self.side || self.mode == PortMode::Input || amount <= 0 {
            return ChemicalStack::EMPTY;
        }
        let Some(t) = self.tanks.get_mut(tank) else { return ChemicalStack::EMPTY };
        if t.1.is_empty() {
            return ChemicalStack::EMPTY;
        }
        let n = amount.min(t.1.amount());
        let got = t.1.copy_with_amount(n);
        if !simulate {
            t.1 = t.1.copy_with_amount(t.1.amount() - n).normalized();
        }
        got
    }
}

/// `Capabilities.CHEMICAL` of a fission reactor port at `pos` through `face`: None for any other block.
pub(super) fn chemical_handler<R>(
    game: &Game<'_>,
    dimension: &str,
    pos: Pos,
    block: &str,
    face: Option<Direction>,
    registry: &ChemicalRegistry,
    use_it: impl FnOnce(&mut dyn ChemicalHandler) -> R,
) -> Option<R> {
    let owner = port_of(dimension, pos, block)?;
    let mode = mode_at(game, pos);
    let indices: &[usize] = match mode {
        PortMode::Input => &[0, 3],
        PortMode::OutputWaste => &[2],
        PortMode::OutputCoolant => &[1],
    };
    let held = owner.and_then(|id| {
        let caps = capacities(dimension, id)?;
        let (stacks, water) = mb::with_cache(dimension, Kind::Fission, id, |c| (c.chemicals.clone(), c.fluids.first().is_some_and(|f| !f.is_empty())))?;
        let cap = |i: usize| [caps.fuel, caps.heated, caps.waste, i64::from(caps.cooled)][i];
        Some((id, indices.iter().map(|&i| (i, stacks.get(i).cloned().unwrap_or(ChemicalStack::EMPTY), cap(i))).collect::<Vec<_>>(), water))
    });
    let mut handler = PortChemical {
        tanks: held.as_ref().map_or(Vec::new(), |h| h.1.clone()),
        mode,
        side: face.is_some(),
        water: held.as_ref().is_some_and(|h| h.2),
        registry,
    };
    let answer = use_it(&mut handler);
    if let Some((id, before, _)) = held
        && handler.tanks != before
    {
        let now = handler.tanks;
        mb::with_cache(dimension, Kind::Fission, id, |c| {
            for (i, stack, _) in now {
                if let Some(s) = c.chemicals.get_mut(i) {
                    *s = stack;
                }
            }
        });
    }
    Some(answer)
}

/// The Configurator's sneaking use on a port (`onSneakRightClick`): its next mode, said on the action bar.
pub(super) fn turn_port(game: &mut Game<'_>, pos: Pos) -> Option<PortMode> {
    if game.native_block(pos) != PORT {
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

// ---- The screen's buttons and /mek fission ---------------------------------------------------------------------

/// The screen's buttons: activate and SCRAM (`PacketGeneratorsGuiInteract.FISSION_ACTIVE`), the statistics tab
/// (the client's page), the adapter's modes (`LOGIC_TYPE`) and its redstone control.
pub(super) const ACTIVATE: i32 = 7400;
pub(super) const SCRAM: i32 = 7401;
pub(super) const STATS: i32 = 7402;
pub(super) const LOGIC: i32 = 7410;
pub(super) const CONTROL: i32 = 7420;

/// `native_menu_button` of a reactor's or an adapter's screen.
pub(super) fn button(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity, id: i32) {
    let dimension = game.native_dimension().to_owned();
    match id {
        ACTIVATE | SCRAM => {
            let Some(owner) = mb::formed_at(&dimension, Kind::Fission, pos) else { return };
            mb::with_cache(&dimension, Kind::Fission, owner, |c| {
                if let Some(core) = c.reactor.as_mut() {
                    core.set_active(id == ACTIVATE);
                }
            });
        }
        // The adapter's own block entity is the one the hook point lends.
        _ if (LOGIC..LOGIC + 6).contains(&id) && entity.id == ADAPTER => {
            entity.nbt.insert(key("logic_type"), Tag::Int(id - LOGIC));
        }
        CONTROL if entity.id == ADAPTER => {
            let now = RedstoneControl::by_id(super::mek_world::number(entity.nbt.get(&key("control_type"))).unwrap_or(0) as i32).supported();
            entity.nbt.insert(key("control_type"), Tag::Int(now.next().id()));
        }
        _ => {}
    }
}

/// `/mek fission x y z rate <mB/t> | activate | scram`: the burn rate the stats page's text field sets
/// (`setRateLimitFromPacket`, within 0 and its max burn rate), and the screen's two buttons.
pub(super) fn command(game: &mut Game<'_>, pos: Pos, words: &[&str]) -> Value {
    let dimension = game.native_dimension().to_owned();
    let Some(owner) = mb::formed_at(&dimension, Kind::Fission, pos) else { return json!({ "pos": pos, "error": "no formed fission reactor here" }) };
    let config = settings();
    let max = shape_of(&dimension, owner).map_or(0, |s| config.max_burn_rate(s.assemblies));
    let done = mb::with_cache(&dimension, Kind::Fission, owner, |c| {
        let core = c.reactor.get_or_insert_with(Core::new);
        match words {
            ["rate", r] => match r.parse::<f64>() {
                // `UnitDisplayUtils.roundDecimals`: two decimals.
                Ok(v) if v.is_finite() => {
                    core.set_rate_limit((v * 100.0).round() / 100.0, max);
                    Ok(())
                }
                _ => Err("rate: a number"),
            },
            ["activate"] => {
                core.set_active(true);
                Ok(())
            }
            ["scram"] => {
                core.set_active(false);
                Ok(())
            }
            _ => Err("usage: /mek fission <x> <y> <z> rate <mB/t> | activate | scram"),
        }
    });
    match done {
        Some(Ok(())) => probe(game, &dimension, owner),
        Some(Err(e)) => json!({ "pos": pos, "error": e }),
        None => json!({ "pos": pos, "error": "no cache" }),
    }
}

// ---- The screens (`GuiFissionReactor`, `GuiFissionReactorStats`, `GuiFissionReactorLogicAdapter`) --------------

/// The screen's data slots, 16-bit pieces: the reactor's, then an adapter's own three.
const DATA: usize = 2 + 2 + 1 + 4 + 4 + 4 + 1 + 4 + 4 + 4 + 4 + 2 + 2 + 2 + 4 + 2 + 1 + 1 + 2 + 4 + 4 + 2 + 2 + 2;

fn push(out: &mut Vec<i32>, v: u64, words: usize) {
    out.extend((0..words).map(|i| ((v >> (16 * i)) & 0xFFFF) as i32));
}

fn read(data: &[i32], at: usize, words: usize) -> u64 {
    (0..words).fold(0u64, |v, i| v | (u64::from(data[at + i] as u16) << (16 * i)))
}

fn float(v: f64) -> u64 {
    u64::from((v as f32).to_bits())
}

#[derive(Default)]
struct View {
    water: u64,
    cooled_cap: u64,
    coolant: (i32, u64),
    fuel: u64,
    fuel_cap: u64,
    heated: (i32, u64),
    heated_cap: u64,
    waste: u64,
    waste_cap: u64,
    temperature: f64,
    loss: f64,
    burn_rate: f64,
    boil_rate: u64,
    damage: f64,
    active: bool,
    force_disabled: bool,
    rate_limit: f64,
    max_burn: u64,
    heat_capacity: u64,
    surface_area: u64,
    assemblies: u64,
    efficiency: f64,
    /// An adapter's mode, status and redstone control.
    adapter: Option<(i32, i32, i32)>,
}

fn unpack(data: &[i32]) -> Option<View> {
    if data.len() < DATA {
        return None;
    }
    let f = |at: usize| f64::from(f32::from_bits(read(data, at, 2) as u32));
    Some(View {
        water: read(data, 0, 2),
        cooled_cap: read(data, 2, 2),
        coolant: (data[4], read(data, 5, 4)),
        fuel: read(data, 9, 4),
        fuel_cap: read(data, 13, 4),
        heated: (data[17], read(data, 18, 4)),
        heated_cap: read(data, 22, 4),
        waste: read(data, 26, 4),
        waste_cap: read(data, 30, 4),
        temperature: f(34),
        loss: f(36),
        burn_rate: f(38),
        boil_rate: read(data, 40, 4),
        damage: f(44),
        active: data[46] != 0,
        force_disabled: data[47] != 0,
        rate_limit: f(48),
        max_burn: read(data, 50, 4),
        heat_capacity: read(data, 54, 4),
        surface_area: read(data, 58, 2),
        assemblies: read(data, 60, 2),
        efficiency: f(62),
        adapter: (data.len() >= DATA + 3).then(|| (data[DATA], data[DATA + 1], data[DATA + 2])),
    })
}

/// The data slots of a formed reactor, for `mek_multiblock` to show on its casings, ports and adapters.
pub(super) fn view(game: &Game<'_>, dimension: &str, id: Uuid) -> Vec<i32> {
    let Some(shape) = shape_of(dimension, id) else { return Vec::new() };
    let config = settings();
    let caps = config.capacities(&shape);
    let Some((water, chemicals, heat, core)) =
        mb::with_cache(dimension, Kind::Fission, id, |c| (c.fluids.first().cloned(), c.chemicals.clone(), c.heat.first().copied().flatten(), c.reactor))
    else {
        return Vec::new();
    };
    if chemicals.len() < 4 {
        return Vec::new();
    }
    let core = core.unwrap_or_else(Core::new);
    let registry = mek_chemicals::registry(game);
    let state = STATES.with(|s| s.borrow().get(&id).copied()).unwrap_or_default();
    let temperature = heat.map_or(state.ambient, |h| h.temperature());
    let mut out = Vec::with_capacity(DATA + 3);
    push(&mut out, water.map_or(0, |w| w.amount().max(0) as u64), 2);
    push(&mut out, caps.cooled.max(0) as u64, 2);
    out.push(registry.number(chemicals[3].id()));
    push(&mut out, chemicals[3].amount().max(0) as u64, 4);
    push(&mut out, chemicals[0].amount().max(0) as u64, 4);
    push(&mut out, caps.fuel.max(0) as u64, 4);
    out.push(registry.number(chemicals[1].id()));
    push(&mut out, chemicals[1].amount().max(0) as u64, 4);
    push(&mut out, caps.heated.max(0) as u64, 4);
    push(&mut out, chemicals[2].amount().max(0) as u64, 4);
    push(&mut out, caps.waste.max(0) as u64, 4);
    push(&mut out, float(temperature), 2);
    push(&mut out, float(state.last.environment_loss), 2);
    push(&mut out, float(state.last.burn_rate), 2);
    push(&mut out, state.last.boil_rate.max(0) as u64, 4);
    push(&mut out, float(core.damage), 2);
    out.push(i32::from(core.active));
    out.push(i32::from(core.force_disabled));
    push(&mut out, float(core.rate(&config)), 2);
    push(&mut out, config.max_burn_rate(shape.assemblies).max(0) as u64, 4);
    push(&mut out, heat.map_or(0, |h| h.capacity.max(0.0) as u64), 4);
    push(&mut out, shape.surface_area.max(0) as u64, 2);
    push(&mut out, shape.assemblies.max(0) as u64, 2);
    push(&mut out, float(config.boil_efficiency(&shape)), 2);
    out
}

/// The data an adapter's screen shows: the reactor's and its own mode, status and redstone control.
pub(super) fn view_at(game: &mut Game<'_>, dimension: &str, pos: Pos, data: &[i32]) -> Vec<i32> {
    let mut out = data.to_vec();
    if game.native_block(pos) == ADAPTER {
        let (logic, control, _) = adapter_nbt(game, pos);
        let status = ADAPTERS.with(|a| a.borrow().get(&(dimension.to_owned(), pos)).map_or(Status::Idle, |a| a.status));
        out.extend([logic.id(), status as i32, control.id()]);
    }
    out
}

const WIDTH: f32 = 195.0;
const HEIGHT: f32 = 255.0;
const COOLANT_GAUGE: Rect = [6.0, 13.0, 18.0, 60.0];
const FUEL_GAUGE: Rect = [25.0, 13.0, 18.0, 60.0];
const HEATED_GAUGE: Rect = [152.0, 13.0, 18.0, 60.0];
const WASTE_GAUGE: Rect = [171.0, 13.0, 18.0, 60.0];
const SCREEN: Rect = [45.0, 17.0, 105.0, 56.0];
const ACTIVATE_BUTTON: Rect = [6.0, 75.0, 81.0, 16.0];
const SCRAM_BUTTON: Rect = [89.0, 75.0, 81.0, 16.0];
const LIGHT: Rect = [173.0, 76.0, 14.0, 14.0];
const TEMP_BAR: Rect = [5.0, 102.0, 185.0, 8.0];
const GRAPH: Rect = [5.0, 123.0, 185.0, 38.0];
const HEAT_TAB: Rect = [-26.0, 109.0, 26.0, 26.0];
/// `GuiFissionReactorTab`: the statistics tab, left of the panel.
const STATS_TAB: Rect = [-26.0, 6.0, 26.0, 26.0];
const STATS_BUTTON: Rect = [-21.0, 10.0, 18.0, 18.0];
const ADAPTER_WIDTH: f32 = 196.0;
/// The adapter's mode list (`GuiElementHolder` at (26, 31), 130 by 90): one row of 15 a mode.
const MODES_BOX: Rect = [26.0, 31.0, 130.0, 90.0];
const CONTROL_BUTTON: Rect = [160.0, 31.0, 30.0, 16.0];

/// `native_screen` of a casing or port: no slots of its own, a taller panel.
pub(super) fn screen() -> NativeScreen {
    PAGE.with(|p| p.set(0));
    NativeScreen { width: WIDTH, height: HEIGHT, slots: vec![None; 5], inventory_y: 173.0, inventory_x: 17.0, own_labels: true, own_background: false }
}

/// `native_screen` of a logic adapter.
pub(super) fn adapter_screen() -> NativeScreen {
    NativeScreen { width: ADAPTER_WIDTH, height: 166.0, slots: vec![None; 5], inventory_y: 84.0, inventory_x: 18.0, own_labels: true, own_background: false }
}

fn mode_row(i: usize) -> Rect {
    [MODES_BOX[0] + 1.0, MODES_BOX[1] + 1.0 + 14.8 * i as f32, MODES_BOX[2] - 2.0, 14.0]
}

pub(super) fn click(block: &str, at: [f32; 2], button: &mut Option<i32>) {
    if block == ADAPTER {
        if let Some(i) = (0..6).find(|&i| mek_ui::inside(mode_row(i), at)) {
            *button = Some(LOGIC + i as i32);
        } else if mek_ui::inside(CONTROL_BUTTON, at) {
            *button = Some(CONTROL);
        }
        return;
    }
    if mek_ui::inside(STATS_BUTTON, at) {
        PAGE.with(|p| p.set(1 - p.get().min(1)));
        *button = Some(STATS);
    } else if PAGE.with(Cell::get) == 0 && mek_ui::inside(ACTIVATE_BUTTON, at) {
        *button = Some(ACTIVATE);
    } else if PAGE.with(Cell::get) == 0 && mek_ui::inside(SCRAM_BUTTON, at) {
        *button = Some(SCRAM);
    }
}

fn percent(damage: f64) -> String {
    format!("{}%", (damage / fission::MAX_DAMAGE * 100.0).round() as i64)
}

fn lines(view: &View, page: u8) -> Vec<String> {
    if page == 1 {
        return vec![
            "Heat Statistics".into(),
            format!("Heat Capacity: {} J/K", display::thousands(view.heat_capacity)),
            format!("Fuel Surface Area: {} m2", display::thousands(view.surface_area)),
            format!("Boil Efficiency: {}", (view.efficiency * 1000.0).round() / 1000.0),
            "Fuel Statistics".into(),
            format!("Max Burn Rate: {} mB/t", display::thousands(view.max_burn)),
            format!("Rate Limit: {} mB/t", (view.rate_limit * 100.0).round() / 100.0),
            format!("Current Burn Rate: {} mB/t", (view.burn_rate * 100.0).round() / 100.0),
        ];
    }
    vec![
        format!("Status: {}", if view.active { "Active" } else { "Disabled" }),
        format!("Burn Rate: {} mB/t", (view.burn_rate * 100.0).round() / 100.0),
        format!("Heating Rate: {} mB/t", display::thousands(view.boil_rate)),
        format!("Temp: {}", display::temperature(view.temperature)),
        format!("Damage: {}", percent(view.damage)),
    ]
}

pub(super) fn tooltip(block: &str, data: &[i32], at: [f32; 2], tips: &mut Vec<String>) {
    let Some(view) = unpack(data) else { return };
    if block == ADAPTER {
        if let Some(i) = (0..6).find(|&i| mek_ui::inside(mode_row(i), at)) {
            tips.push(Logic::ALL[i].description(settings().excess_waste));
        } else if mek_ui::inside(CONTROL_BUTTON, at) {
            tips.push("Redstone Control".into());
        }
        return;
    }
    if mek_ui::inside(STATS_BUTTON, at) {
        tips.push(if PAGE.with(Cell::get) == 0 { "Fission Reactor Statistics".into() } else { "Fission Reactor".into() });
        return;
    }
    if PAGE.with(Cell::get) != 0 {
        return;
    }
    if mek_ui::inside(COOLANT_GAUGE, at) {
        tips.push("Coolant Tank".into());
        tips.push(if view.water > 0 { format!("Water: {} mB", display::thousands(view.water)) } else { mek_ui::chemical_line(view.coolant.0, view.coolant.1) });
    } else if mek_ui::inside(FUEL_GAUGE, at) {
        tips.extend(["Fuel Tank".to_owned(), if view.fuel == 0 { "Empty".into() } else { format!("Fissile Fuel: {} mB", display::thousands(view.fuel)) }]);
    } else if mek_ui::inside(HEATED_GAUGE, at) {
        tips.extend(["Heated Coolant Tank".to_owned(), mek_ui::chemical_line(view.heated.0, view.heated.1)]);
    } else if mek_ui::inside(WASTE_GAUGE, at) {
        tips.extend(["Waste Tank".to_owned(), if view.waste == 0 { "Empty".into() } else { format!("Nuclear Waste: {} mB", display::thousands(view.waste)) }]);
    } else if mek_ui::inside(ACTIVATE_BUTTON, at) && view.force_disabled {
        tips.push("Reactor must reach safe damage and temperature levels before it can be reactivated.".into());
    } else if mek_ui::inside(TEMP_BAR, at) {
        tips.push(display::temperature(view.temperature));
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

fn text_button(pen: &mut Pen<'_, '_>, r: Rect, label: &str, active: bool, rgb: u32) {
    pen.button(r, active);
    pen.centered(label, r[0], r[2], r[1] + (r[3] - 8.0) / 2.0, if active { rgb } else { 0xA0A0A0 });
}

fn inventory(pen: &mut Pen<'_, '_>, x: f32, y: f32) {
    for row in 0..4 {
        let ry = if row == 3 { y + 58.0 } else { y + 18.0 * row as f32 };
        for col in 0..9 {
            pen.slot("normal", None, x + 18.0 * col as f32, ry);
        }
    }
}

pub(super) fn draw(game: &Game<'_>, block: &str, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let view = unpack(data);
    let cursor = game.native_cursor();
    let cursor = [cursor[0] - origin[0], cursor[1] - origin[1]];
    let mut pen = Pen { game, batch, origin, cursor };
    if block == ADAPTER {
        draw_adapter(&mut pen, view.as_ref());
        return;
    }
    let page = PAGE.with(Cell::get);
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [0.0, 0.0, WIDTH, HEIGHT]);
    pen.holder(STATS_TAB, true);
    pen.image_button("stats", 18.0, STATS_BUTTON, true);
    pen.whole("tabs/heat_info_k", [26.0, 26.0], HEAT_TAB);
    let shown = view.is_some();
    let v = view.unwrap_or_default();
    if page == 0 {
        let registry = mek_chemicals::registry(game);
        let color = |id: &str| 0xFF00_0000 | registry.get(id).map_or(0x808080, |c| c.color & 0xFF_FFFF);
        if v.water > 0 {
            gauge(&mut pen, COOLANT_GAUGE, v.water, v.cooled_cap, 0xFF3F_76E4);
        } else {
            gauge(&mut pen, COOLANT_GAUGE, v.coolant.1, v.cooled_cap, 0xFF00_0000 | mek_ui::chemical_color(game, v.coolant.0));
        }
        gauge(&mut pen, FUEL_GAUGE, v.fuel, v.fuel_cap, color(FUEL));
        gauge(&mut pen, HEATED_GAUGE, v.heated.1, v.heated_cap, 0xFF00_0000 | mek_ui::chemical_color(game, v.heated.0));
        gauge(&mut pen, WASTE_GAUGE, v.waste, v.waste_cap, color(WASTE));
        pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], SCREEN);
        if shown {
            for (i, line) in lines(&v, 0).iter().enumerate() {
                pen.text(line, [SCREEN[0] + 3.0, SCREEN[1] + 3.0 + 10.0 * i as f32], mek_ui::SCREEN_TEXT);
            }
        }
        text_button(&mut pen, ACTIVATE_BUTTON, "Activate", !v.active && !v.force_disabled, 0x00AA00);
        text_button(&mut pen, SCRAM_BUTTON, "SCRAM", v.active, 0xAA0000);
        pen.fill(LIGHT, if v.active { 0xFF3C_FE9A } else { 0xFF40_4040 });
        pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], TEMP_BAR);
        let w = ((v.temperature / fission::MAX_DAMAGE_TEMPERATURE).clamp(0.0, 1.0) * f64::from(TEMP_BAR[2] - 2.0)) as i32 as f32;
        if w > 0.0 {
            let rgb = if v.temperature < 600.0 {
                0xFF55_FF55
            } else if v.temperature < 1000.0 {
                0xFFFF_FF55
            } else if v.temperature < 1200.0 {
                0xFFFF_AA00
            } else {
                0xFFFF_5555
            };
            pen.fill([TEMP_BAR[0] + 1.0, TEMP_BAR[1] + 1.0, w, TEMP_BAR[3] - 2.0], rgb);
        }
        pen.text("Heat Graph:", [GRAPH[0], GRAPH[1] - 10.0], mek_ui::TITLE);
        pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], GRAPH);
    } else {
        pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], [6.0, 18.0, WIDTH - 12.0, 120.0]);
        if shown {
            for (i, line) in lines(&v, 1).iter().enumerate() {
                pen.text(line, [9.0, 21.0 + 12.0 * i as f32], mek_ui::SCREEN_TEXT);
            }
        }
        pen.text("Set Rate Limit: /mek fission <pos> rate", [6.0, 145.0], mek_ui::TITLE);
    }
    inventory(&mut pen, 16.0, 172.0);
    let title = if page == 0 { "Fission Reactor" } else { "Fission Reactor Statistics" };
    pen.centered(title, 0.0, WIDTH, 5.0, mek_ui::TITLE);
    pen.text("Inventory", [6.0, 163.0], mek_ui::TITLE);
}

fn draw_adapter(pen: &mut Pen<'_, '_>, view: Option<&View>) {
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [0.0, 0.0, ADAPTER_WIDTH, 166.0]);
    pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], MODES_BOX);
    let (logic, status, control) = view.and_then(|v| v.adapter).unwrap_or((0, 0, 0));
    for (i, mode) in Logic::ALL.iter().enumerate() {
        let r = mode_row(i);
        text_button(pen, r, mode.text(), i as i32 != logic, 0x404040);
    }
    let control = RedstoneControl::by_id(control).supported();
    text_button(pen, CONTROL_BUTTON, control.text(), true, 0x404040);
    let status = match status {
        1 => Status::Outputting,
        2 => Status::Powered,
        _ => Status::Idle,
    };
    pen.text(&format!("Redstone mode: {}", Logic::by_id(logic).text()), [26.0, 123.0], mek_ui::TITLE);
    pen.text(&format!("Status: {}", status.text()), [26.0, 136.0], mek_ui::TITLE);
    pen.centered("Fission Reactor Logic Adapter", 0.0, ADAPTER_WIDTH, 5.0, mek_ui::TITLE);
    let _ = inventory;
}

// ---- /mek probe -------------------------------------------------------------------------------------------------

pub(super) fn probe(game: &Game<'_>, dimension: &str, id: Uuid) -> Value {
    let shape = shape_of(dimension, id).unwrap_or_default();
    let config = settings();
    let caps = config.capacities(&shape);
    let state = STATES.with(|s| s.borrow().get(&id).copied()).unwrap_or_default();
    let (water, chemicals, heat, core) = mb::with_cache(dimension, Kind::Fission, id, |c| {
        (
            c.fluids.first().map(|f| json!({ "fluid": (!f.is_empty()).then(|| f.name()), "amount": f.amount().max(0) })),
            c.chemicals.iter().map(|s| json!({ "chemical": s.id(), "amount": s.amount().max(0) })).collect::<Vec<_>>(),
            c.heat.first().copied().flatten(),
            c.reactor,
        )
    })
    .unwrap_or_default();
    let core = core.unwrap_or_else(Core::new);
    let ports: Vec<Value> = mb::with_formed(dimension, id, |f| f.valves.clone())
        .unwrap_or_default()
        .into_iter()
        .map(|(p, side)| json!({ "pos": p, "side": side.name(), "mode": mode_at(game, p).name() }))
        .collect();
    let adapters: Vec<Value> = ADAPTERS.with(|a| {
        a.borrow()
            .iter()
            .filter(|((d, p), _)| d == dimension && mb::formed_at(dimension, Kind::Fission, *p) == Some(id))
            .map(|((_, p), a)| json!({ "pos": p, "status": a.status.text() }))
            .collect()
    });
    json!({
        "fuel": chemicals.first(), "heated_coolant": chemicals.get(1), "waste": chemicals.get(2), "coolant": chemicals.get(3), "water": water,
        "capacities": { "fuel": caps.fuel, "waste": caps.waste, "cooled": caps.cooled, "heated": caps.heated },
        "assemblies": shape.assemblies, "surface_area": shape.surface_area, "boil_efficiency": config.boil_efficiency(&shape),
        "max_burn_rate": config.max_burn_rate(shape.assemblies),
        "heat": heat.map(|h| h.stored), "heat_capacity": heat.map(|h| h.capacity), "temperature": heat.map(|h| h.temperature()),
        "ambient": state.ambient, "burn_rate": state.last.burn_rate, "boil_rate": state.last.boil_rate,
        "environment_loss": state.last.environment_loss, "waste_overflow": state.last.waste_overflow,
        "damage": core.damage, "rate_limit": core.rate(&config), "burn_remaining": core.burn_remaining, "partial_waste": core.partial_waste,
        "active": core.active, "force_disabled": core.force_disabled, "ports": ports, "adapters": adapters,
        "meltdowns": MELTDOWNS.with(|m| m.borrow().iter().filter(|x| x.id == id).count()),
    })
}
