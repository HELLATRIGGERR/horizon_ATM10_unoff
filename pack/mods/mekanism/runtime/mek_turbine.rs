// Mekanism Generators' Industrial Turbine in the game (support module `crate::game::mek_turbine` of the `mekanism`
// native bundle). The rules are `mekanism_energy::turbine` (MIT, from Mekanism 10.7.19, Copyright (c) 2017-2025 Aidan
// C. Brady); the turbine forms through `mek_multiblock` (`Kind::Turbine`: turbine casings, turbine valves, turbine
// vents and structural glass, the rotors, the rotational complex, pressure dispersers, electromagnetic coils and
// saturating condensers inside), and its steam, vent water and Joules live in the cache by its id there
// (`chemical_tanks`, `fluid_tanks`, `energy_containers`, and `dumping`). This module is the glue: its settings from
// `generators.toml` `[turbine]`, its tick (`TurbineMultiblockData.tick`: the flow, the water out of its vents, the
// Joules out of its valves, the dump), the valves' chemical handler (steam in, counted as its input) and Forge Energy
// (Joules out), the vents' fluid handler (water out), the rotors' blades (`TileEntityTurbineRotor.addBlade`,
// `removeBlade`, the blade item used on them), its screen (`GuiIndustrialTurbine`, `GuiTurbineStats` as a page, the
// dump mode button) and `/mek probe`.

use super::{
    Game, mek_chemicals,
    mek_multiblock::{self as mb, Formed, Kind},
    mek_ui::{self, Pen, Rect},
    mek_world::{self, key},
    native_hooks::{NativeFeInfo, NativeFluidTankInfo, NativeScreen},
};
use horizon_client::{containers::ModBlockEntity, hud::HudBatch, items::ItemStack};
use horizon_core::geometry::Direction as Face;
use horizon_world::{
    fluid::FluidKind,
    fluid_stack::FluidStack,
    nbt::{Compound, Tag},
};
use mekanism_energy::{
    chemical::{ChemicalHandler, ChemicalRegistry, ChemicalStack},
    config::{self, EnergyConfig},
    display,
    fe::{self, Conversion, StrictEnergy},
    multiblock::{Failure, Formation, Node, Pos, Uuid},
    separator::GasMode,
    sides::Direction,
    turbine::{self, Contents, Inside, Shape, TurbineConfig},
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap},
    time::{Duration, Instant},
};

pub(super) const CASING: &str = "mekanismgenerators:turbine_casing";
pub(super) const VALVE: &str = "mekanismgenerators:turbine_valve";
pub(super) const VENT: &str = "mekanismgenerators:turbine_vent";
pub(super) const ROTOR: &str = "mekanismgenerators:turbine_rotor";
const COMPLEX: &str = "mekanismgenerators:rotational_complex";
const COIL: &str = "mekanismgenerators:electromagnetic_coil";
const CONDENSER: &str = "mekanismgenerators:saturating_condenser";
const DISPERSER: &str = "mekanism:pressure_disperser";
const BLADE: &str = "mekanismgenerators:turbine_blade";
const STEAM: &str = "mekanism:steam";

/// What a formed turbine keeps beside its cache (not saved: made again when it forms).
#[derive(Clone, Copy, Debug, Default)]
struct State {
    /// `blades`: its rotors' blades as it formed, and those added or taken since.
    blades: i32,
    /// `newSteamInput`: steam its valves took since its last tick.
    new_input: i64,
    /// `lastSteamInput`.
    last_input: i64,
    last: turbine::Tick,
}

thread_local! {
    static SETTINGS: RefCell<Option<(TurbineConfig, config::Stamp, Instant)>> = const { RefCell::new(None) };
    static STATES: RefCell<HashMap<Uuid, State>> = RefCell::new(HashMap::new());
    /// The screen's page on the client: 0 the turbine, 1 its statistics (`GuiTurbineStats`).
    static PAGE: Cell<u8> = const { Cell::new(0) };
    /// The rotor each player's last blade use acted on: Mekanism's use consumes the click, so the off hand gets no
    /// turn; this client, not knowing, sends one, which takes nothing. That turn is a packet of its own, sent after
    /// the main hand's: a busy client or server lets it miss the drain of the tick the main hand's came in, so the
    /// guard waits for it through the next two ticks.
    static USED: RefCell<HashMap<u128, Used>> = RefCell::new(HashMap::new());
}

/// A blade use that took a click, against the other hand's turn of that click (`USED`).
struct Used {
    pos: Pos,
    /// The level it was in: the ends of its ticks age the guard.
    dimension: &'static str,
    /// How many of them it has seen; it goes at the third.
    ended: u8,
}

/// `generators.toml` `[turbine]` and `general.toml`, read again within a second of a change.
pub(super) fn settings() -> TurbineConfig {
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
                Some(d) => (TurbineConfig::load(d), config::stamp(d), Instant::now()),
                None => (TurbineConfig::default(), [None; config::FILES.len()], Instant::now()),
            });
        }
        s.as_ref().map(|(c, _, _)| *c).unwrap_or_default()
    })
}

/// A casing, valve or vent, whose screen shows the turbine.
pub(super) fn is_turbine_block(block: &str) -> bool {
    block == CASING || block == VALVE || block == VENT
}

/// A world opens or closes.
pub(super) fn clear() {
    STATES.with(|s| s.borrow_mut().clear());
}

fn blades_at(game: &mut Game<'_>, p: Pos) -> Option<i32> {
    game.native_with_mod_block_entity(p, |m| (m.id == ROTOR).then(|| mek_world::number(m.nbt.get(&key("blades"))).unwrap_or(0).clamp(0, 2) as i32))
        .flatten()
}

/// `FormationProtocol.doUpdate` with the `TurbineValidator` over the structure `nodes`.
pub(super) fn form(game: &mut Game<'_>, nodes: &BTreeMap<Pos, Node>) -> Result<(Formation, Shape), Failure> {
    turbine::form(nodes, |p| {
        let block = game.native_block(p);
        match block.as_str() {
            DISPERSER => Inside::Disperser,
            ROTOR => Inside::Rotor(blades_at(game, p).unwrap_or(0)),
            COMPLEX => Inside::Complex,
            COIL => Inside::Coil,
            CONDENSER => Inside::Condenser,
            b if mb::is_air(b) => Inside::Air,
            _ => Inside::Invalid,
        }
    })
}

fn shape_of(dimension: &str, id: Uuid) -> Option<Shape> {
    mb::with_formed(dimension, id, |f| f.turbine.clone())?
}

fn blades_of(id: Uuid, shape: &Shape) -> i32 {
    STATES.with(|s| s.borrow().get(&id).map(|s| s.blades)).unwrap_or(shape.blades)
}

/// `onCreated`: the tanks and the buffer capped to the new size (`shouldCap`).
pub(super) fn created(dimension: &str, id: Uuid) {
    let Some(shape) = shape_of(dimension, id) else { return };
    let config = settings();
    let (steam, water, energy) = (config.steam_capacity(&shape), config.water_capacity(&shape), config.energy_capacity(&shape));
    mb::with_cache(dimension, Kind::Turbine, id, |c| {
        if let Some(s) = c.chemicals.first_mut()
            && s.amount() > steam
        {
            s.set_amount(steam);
            *s = s.clone().normalized();
        }
        if let Some(w) = c.fluids.first_mut()
            && w.amount() > water
        {
            *w = if water > 0 { w.copy_with_amount(water) } else { FluidStack::EMPTY };
        }
        if let Some(e) = c.energy.first_mut() {
            *e = (*e).min(energy);
        }
    });
    STATES.with(|s| s.borrow_mut().insert(id, State { blades: shape.blades, ..State::default() }));
}

/// `remove`: the turbine broke.
pub(super) fn removed(formed: &Formed) {
    STATES.with(|s| s.borrow_mut().remove(&formed.id));
}

/// `FluidUtils.emit` of the vents' targets: `water` split evenly among the fluid handlers they face that take some of
/// it. What went.
fn emit_water(game: &mut Game<'_>, dimension: &str, vents: &[(Pos, Direction)], water: i32) -> i32 {
    let offered = FluidStack::new(FluidKind::Water, water);
    let at = |(p, side): (Pos, Direction)| (mek_world::beside(p, side), Some(mek_world::engine_face(side.opposite())));
    let targets: Vec<(Pos, Direction)> = vents
        .iter()
        .copied()
        .filter(|&v| {
            let (to, side) = at(v);
            game.native_fluid_fill(dimension, to, side, &offered, true) > 0
        })
        .collect();
    if targets.is_empty() {
        return 0;
    }
    let sent = mekanism_energy::split::send_to_acceptors(targets.len(), water.max(0) as u64, |t, amount, simulate| {
        let (to, side) = at(targets[t]);
        let stack = offered.copy_with_amount(amount.min(i32::MAX as u64) as i32);
        let took = game.native_fluid_fill(dimension, to, side, &stack, simulate).clamp(0, stack.amount());
        amount - took as u64
    });
    (sent.min(i32::MAX as u64) as i32).min(water)
}

/// `CableUtils.emit` of the valves' targets: every Joule split among the blocks they face that take energy. What went.
fn emit_energy(game: &mut Game<'_>, dimension: &str, valves: &[(Pos, Direction)], energy: u64) -> u64 {
    let (config, _) = mek_world::energy_config();
    let mut targets = Vec::new();
    for &(pos, side) in valves {
        if let Some(target) = mek_world::energy_target(game, &config, dimension, pos, side) {
            targets.push((pos, side, target));
        }
    }
    if targets.is_empty() {
        return 0;
    }
    let rate = Conversion(config.fe_conversion_rate);
    let sent = mekanism_energy::split::send_to_acceptors(targets.len(), energy, |i, amount, simulate| {
        let (pos, side, target) = targets[i];
        mek_world::energy_hand_over(game, &config, rate, dimension, pos, side, target, amount, simulate)
    });
    sent.min(energy)
}

/// `TurbineMultiblockData.tick`: the input of the tick before, the flow, the water out of its vents, the Joules out of
/// its valves, then the dump.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, id: Uuid) {
    let Some(formed) = mb::with_formed(dimension, id, Clone::clone) else { return };
    let Some(shape) = formed.turbine.clone() else { return };
    let Some(before) = mb::with_cache(dimension, Kind::Turbine, id, |c| c.clone()) else { return };
    if before.chemicals.is_empty() || before.fluids.is_empty() || before.energy.is_empty() {
        return;
    }
    let config = settings();
    let mut state = STATES.with(|s| s.borrow().get(&id).copied()).unwrap_or(State { blades: shape.blades, ..State::default() });
    state.last_input = std::mem::take(&mut state.new_input);
    let steam = if before.chemicals[0].id() == Some(STEAM) { before.chemicals[0].amount().max(0) } else { 0 };
    let mut contents = Contents { steam, energy: before.energy[0], water: before.fluids[0].amount().max(0) };
    state.last = turbine::produce(&mut contents, &shape, state.blades, &config);
    if contents.water > 0 {
        contents.water -= emit_water(game, dimension, &shape.vents, contents.water);
    }
    if contents.energy > 0 {
        contents.energy -= emit_energy(game, dimension, &formed.valves, contents.energy);
    }
    turbine::dump(&mut contents, &shape, GasMode::by_id(before.dumping), state.last_input, &config);
    mb::with_cache(dimension, Kind::Turbine, id, |c| {
        c.chemicals[0] = if contents.steam > 0 { ChemicalStack::new(STEAM, contents.steam) } else { ChemicalStack::EMPTY };
        c.fluids[0] = if contents.water > 0 { FluidStack::new(FluidKind::Water, contents.water) } else { FluidStack::EMPTY };
        c.energy[0] = contents.energy;
    });
    STATES.with(|s| s.borrow_mut().insert(id, state));
}

// ---- The valves: steam in, Joules out ------------------------------------------------------------------------

/// The turbine a block at `pos` is a valve or vent of: Some(None) when it forms no turbine.
fn part_of(dimension: &str, pos: Pos, block: &str, part: &str) -> Option<Option<Uuid>> {
    (block == part).then(|| mb::formed_at(dimension, Kind::Turbine, pos))
}

/// The steam tank through a valve (`TurbineChemicalTank`): it takes steam from any side once formed, counting it as
/// the turbine's input, and gives nothing to other blocks (`notExternalFormedBiPred`); the null side reads only.
struct ValveSteam {
    stack: ChemicalStack,
    capacity: i64,
    formed: bool,
    side: bool,
    took: i64,
}

impl ChemicalHandler for ValveSteam {
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
        stack.id() == Some(STEAM)
    }

    fn insert_tank(&mut self, tank: usize, stack: &ChemicalStack, simulate: bool) -> ChemicalStack {
        if !self.formed || !self.side || stack.is_empty() || !self.is_valid(tank, stack) {
            return stack.clone();
        }
        let took = stack.amount().min(self.capacity.saturating_sub(self.stack.amount().max(0)).max(0));
        if took <= 0 {
            return stack.clone();
        }
        if !simulate {
            self.stack = ChemicalStack::new(STEAM, self.stack.amount().max(0) + took);
            self.took += took;
        }
        stack.copy_with_amount(stack.amount() - took).normalized()
    }

    fn extract_tank(&mut self, _: usize, _: i64, _: bool) -> ChemicalStack {
        ChemicalStack::EMPTY
    }
}

/// `Capabilities.CHEMICAL` of a turbine valve at `pos` through `face`: None for any other block.
pub(super) fn chemical_handler<R>(
    dimension: &str,
    pos: Pos,
    block: &str,
    face: Option<Direction>,
    _registry: &ChemicalRegistry,
    use_it: impl FnOnce(&mut dyn ChemicalHandler) -> R,
) -> Option<R> {
    let owner = part_of(dimension, pos, block, VALVE)?;
    let held = owner.and_then(|id| {
        let capacity = settings().steam_capacity(&shape_of(dimension, id)?);
        let stack = mb::with_cache(dimension, Kind::Turbine, id, |c| c.chemicals.first().cloned())??;
        Some((id, capacity, stack))
    });
    let mut handler = ValveSteam {
        stack: held.as_ref().map_or(ChemicalStack::EMPTY, |h| h.2.clone()),
        capacity: held.as_ref().map_or(0, |h| h.1),
        formed: held.is_some(),
        side: face.is_some(),
        took: 0,
    };
    let answer = use_it(&mut handler);
    if let Some((id, _, stack)) = held
        && handler.stack != stack
    {
        let now = handler.stack;
        mb::with_cache(dimension, Kind::Turbine, id, |c| c.chemicals[0] = now);
        STATES.with(|s| {
            if let Some(state) = s.borrow_mut().get_mut(&id) {
                state.new_input = state.new_input.saturating_add(handler.took);
            }
        });
    }
    Some(answer)
}

/// The Joules through a valve: other blocks take them once formed (`automationType -> isFormed()`), and put none in
/// (only the turbine does, `INTERNAL`); the null side reads only.
struct ValveEnergy<'a> {
    dimension: &'a str,
    id: Option<Uuid>,
    open: bool,
}

impl StrictEnergy for ValveEnergy<'_> {
    fn insert(&mut self, joules: i64, _: bool) -> i64 {
        joules
    }

    fn extract(&mut self, joules: i64, simulate: bool) -> i64 {
        let (Some(id), true) = (self.id, self.open) else { return 0 };
        if joules <= 0 {
            return 0;
        }
        mb::with_cache(self.dimension, Kind::Turbine, id, |c| {
            let Some(e) = c.energy.first_mut() else { return 0 };
            let n = (joules as u64).min(*e);
            if !simulate {
                *e -= n;
            }
            n as i64
        })
        .unwrap_or(0)
    }
}

/// `native_fe_storage` of a valve: the turbine's Joules in FE. Whether `entity` was a valve.
pub(super) fn fe_storage(dimension: &str, pos: Pos, entity: &ModBlockEntity, config: &EnergyConfig, info: &mut Option<NativeFeInfo>) -> bool {
    if entity.id != VALVE {
        return false;
    }
    if config.blacklist_forge {
        return true;
    }
    let rate = Conversion(config.fe_conversion_rate);
    let held = mb::formed_at(dimension, Kind::Turbine, pos).and_then(|id| {
        let capacity = settings().energy_capacity(&shape_of(dimension, id)?);
        Some((mb::with_cache(dimension, Kind::Turbine, id, |c| c.energy.first().copied().unwrap_or(0))?, capacity))
    });
    let (stored, capacity) = held.unwrap_or((0, 0));
    *info = Some(NativeFeInfo {
        stored: i64::from(rate.to_fe_int(stored.min(i64::MAX as u64) as i64)),
        capacity: i64::from(rate.to_fe_int(capacity.min(i64::MAX as u64) as i64)),
        can_receive: true,
        can_extract: true,
    });
    true
}

/// `native_fe_receive` and `native_fe_extract` of a valve. Whether `entity` was a valve.
#[allow(clippy::too_many_arguments)]
pub(super) fn fe_transfer(dimension: &str, pos: Pos, face: Option<Face>, entity: &ModBlockEntity, config: &EnergyConfig, max: i64, simulate: bool, extract: bool, moved: &mut Option<i64>) -> bool {
    if entity.id != VALVE {
        return false;
    }
    if config.blacklist_forge {
        return true;
    }
    let rate = Conversion(config.fe_conversion_rate);
    let mut side = ValveEnergy { dimension, id: mb::formed_at(dimension, Kind::Turbine, pos), open: face.is_some() };
    let max = max.clamp(0, i64::from(i32::MAX)) as i32;
    let fe = if extract { fe::extract_fe(&mut side, rate, max, simulate) } else { fe::receive_fe(&mut side, rate, max, simulate) };
    *moved = Some(i64::from(fe));
    true
}

/// A turbine valve: cables connect to it and nothing hands it Joules.
pub(super) fn is_valve(block: &str) -> bool {
    block == VALVE
}

// ---- The vents: water out (hook points API 422) ------------------------------------------------------------

/// `native_fluid_handler_tanks` of a vent: once formed, the vent tank, which other blocks drain.
pub(super) fn fluid_tanks(dimension: &str, pos: Pos, face: Option<Face>, block: &str, tanks: &mut Option<Vec<NativeFluidTankInfo>>) -> bool {
    let Some(owner) = part_of(dimension, pos, block, VENT) else { return false };
    let held = owner.and_then(|id| Some((settings().water_capacity(&shape_of(dimension, id)?), mb::with_cache(dimension, Kind::Turbine, id, |c| c.fluids.first().cloned())??)));
    *tanks = Some(match held {
        Some((capacity, water)) => vec![NativeFluidTankInfo { drainable: !water.is_empty() && face.is_some(), stack: water, capacity }],
        None => Vec::new(),
    });
    true
}

/// `native_fluid_handler_fill`: the vent tank takes nothing from other blocks (`VariableCapacityFluidTank.output`).
pub(super) fn fluid_fill(dimension: &str, pos: Pos, block: &str, filled: &mut Option<i32>) -> bool {
    if part_of(dimension, pos, block, VENT).is_none() {
        return false;
    }
    *filled = Some(0);
    true
}

/// `native_fluid_handler_drain`: the vent tank's water; the null side reads only.
#[allow(clippy::too_many_arguments)]
pub(super) fn fluid_drain(
    dimension: &str,
    pos: Pos,
    face: Option<Face>,
    block: &str,
    resource: Option<&FluidStack>,
    max: i32,
    simulate: bool,
    drained: &mut Option<FluidStack>,
) -> bool {
    let Some(owner) = part_of(dimension, pos, block, VENT) else { return false };
    *drained = Some(FluidStack::EMPTY);
    let (Some(id), Some(_)) = (owner, face) else { return true };
    let got = mb::with_cache(dimension, Kind::Turbine, id, |c| {
        let Some(tank) = c.fluids.get_mut(0) else { return FluidStack::EMPTY };
        if tank.is_empty() || resource.is_some_and(|r| !FluidStack::is_same_fluid_same_components(r, tank)) {
            return FluidStack::EMPTY;
        }
        let n = resource.map_or(max, FluidStack::amount).min(tank.amount()).max(0);
        let got = tank.copy_with_amount(n);
        if !simulate && n > 0 {
            let left = tank.amount() - n;
            *tank = if left > 0 { tank.copy_with_amount(left) } else { FluidStack::EMPTY };
        }
        got
    })
    .unwrap_or(FluidStack::EMPTY);
    *drained = Some(got);
    true
}

// ---- The rotors' blades ------------------------------------------------------------------------------------

/// `native_block_entity` of a rotor: no blades, no place in a column yet.
pub(super) fn new_entity(block: &str, entity: &mut Option<(String, Compound)>) -> bool {
    if block != ROTOR {
        return false;
    }
    let mut nbt = Compound::new();
    nbt.insert(key("blades"), Tag::Int(0));
    nbt.insert(key("position"), Tag::Int(-1));
    *entity = Some((block.to_owned(), nbt));
    true
}

fn set_blades(game: &mut Game<'_>, p: Pos, n: i32, delta: i32) {
    game.native_with_mod_block_entity(p, |m| {
        m.nbt.insert(key("blades"), Tag::Int(n));
    });
    let dimension = game.native_dimension();
    if let Some(id) = mb::formed_at(dimension, Kind::Turbine, p) {
        STATES.with(|s| {
            if let Some(state) = s.borrow_mut().get_mut(&id) {
                state.blades += delta;
            }
        });
    }
}

fn below(p: Pos) -> Pos {
    [p[0], p[1] - 1, p[2]]
}

fn above(p: Pos) -> Pos {
    [p[0], p[1] + 1, p[2]]
}

/// `addBlade`: the lowest rotor below with room first, then this one, then the ones above.
fn add_blade(game: &mut Game<'_>, pos: Pos, check_below: bool) -> bool {
    if check_below && blades_at(game, below(pos)).is_some_and(|b| b < 2) {
        return add_blade(game, below(pos), true);
    }
    let Some(n) = blades_at(game, pos) else { return false };
    if n < 2 {
        set_blades(game, pos, n + 1, 1);
        return true;
    }
    blades_at(game, above(pos)).is_some() && add_blade(game, above(pos), false)
}

/// `removeBlade`: the highest rotor above with blades first, then this one, then the ones below.
fn remove_blade(game: &mut Game<'_>, pos: Pos) -> bool {
    if blades_at(game, above(pos)).is_some_and(|b| b > 0) {
        return remove_blade(game, above(pos));
    }
    let Some(n) = blades_at(game, pos) else { return false };
    if n > 0 {
        set_blades(game, pos, n - 1, -1);
        return true;
    }
    blades_at(game, below(pos)).is_some() && remove_blade(game, below(pos))
}

/// `BlockTurbineRotor.useItemOn` with an item: a blade goes onto the column (one used up unless the player builds
/// freely); sneaking with blades that have room, one comes back off.
pub(super) fn use_on(game: &mut Game<'_>, stack: &ItemStack, pos: Pos, claimed: &mut bool) {
    if *claimed || &*stack.item != BLADE || game.native_block(pos) != ROTOR {
        return;
    }
    let player = game.native_acting_id();
    if other_hand(player, pos) {
        *claimed = true;
        return;
    }
    let creative = game.native_instabuild();
    if !game.native_sneaking() {
        if add_blade(game, pos, true) {
            *claimed = true;
            took_click(game, player, pos);
            if !creative {
                let hand = game.native_used_stack();
                if let Some(held) = hand.as_mut() {
                    held.count = held.count.saturating_sub(1);
                }
                if hand.as_ref().is_some_and(|s| s.count == 0) {
                    *hand = None;
                }
            }
        }
        return;
    }
    if stack.count < 64 && remove_blade(game, pos) {
        *claimed = true;
        took_click(game, player, pos);
        if !creative && let Some(held) = game.native_used_stack().as_mut() {
            held.count += 1;
        }
    }
}

/// `BlockTurbineRotor.useItemOn` with an empty hand while sneaking: a blade comes off into the hand.
pub(super) fn right_click(game: &mut Game<'_>, pos: Pos, canceled: &mut bool) {
    if *canceled || !game.native_sneaking() || game.native_block(pos) != ROTOR {
        return;
    }
    if game.native_used_stack().as_ref().is_some_and(|s| s.count > 0) {
        return;
    }
    let player = game.native_acting_id();
    if other_hand(player, pos) {
        *canceled = true;
        return;
    }
    if remove_blade(game, pos) {
        *canceled = true;
        took_click(game, player, pos);
        if !game.native_instabuild() {
            *game.native_used_stack() = Some(ItemStack::new(BLADE, 1));
        }
    }
}

/// A blade use of `player` on the rotor at `pos` took the click: the other hand's turn of it takes nothing.
fn took_click(game: &Game<'_>, player: u128, pos: Pos) {
    let dimension = game.native_dimension();
    USED.with(|u| u.borrow_mut().insert(player, Used { pos, dimension, ended: 0 }));
}

/// Whether this turn of `player` on the rotor at `pos` is the other hand's turn of a click a blade use took, in the
/// same tick or one of the next two: it uses the guard up and takes nothing.
fn other_hand(player: u128, pos: Pos) -> bool {
    USED.with(|u| {
        let mut u = u.borrow_mut();
        let ours = u.get(&player).is_some_and(|used| used.pos == pos);
        if ours {
            u.remove(&player);
        }
        ours
    })
}

/// `native_level_tick_post` of `dimension`: a blade use's guard lasts until the end of the second tick after its own,
/// for an off hand's packet that came too late for that tick's drain; a held use key clicks again only
/// rightClickDelay's 4 ticks later.
pub(super) fn level_ticked(dimension: &str) {
    USED.with(|u| {
        u.borrow_mut().retain(|_, used| {
            if used.dimension == dimension {
                used.ended += 1;
            }
            used.ended < 3
        });
    });
}

/// `TileEntityTurbineRotor.blockRemoved`: a rotor a player breaks drops its blades.
pub(super) fn breaking(game: &mut Game<'_>, pos: Pos) {
    let Some(n) = blades_at(game, pos) else { return };
    if n > 0 {
        game.native_pop_resource(pos, ItemStack::new(BLADE, n as u32));
    }
}

// ---- The screen (`GuiIndustrialTurbine`, `GuiTurbineStats`) ------------------------------------------------------

/// The dump mode button (`GuiGasMode`): the next mode, on the server.
pub(super) const DUMP: i32 = 7300;
/// The statistics tab switches the page on the client; the press is taken.
pub(super) const STATS: i32 = 7301;

/// `native_menu_button` of a casing, valve or vent: the dump mode button.
pub(super) fn button(game: &mut Game<'_>, pos: Pos, id: i32) {
    if id != DUMP {
        return;
    }
    let dimension = game.native_dimension();
    let Some(owner) = mb::formed_at(dimension, Kind::Turbine, pos) else { return };
    mb::with_cache(dimension, Kind::Turbine, owner, |c| c.dumping = GasMode::by_id(c.dumping).next().id());
}

struct View {
    steam: u64,
    steam_cap: u64,
    energy: u64,
    energy_cap: u64,
    last_input: u64,
    flow: u64,
    production: u64,
    max_flow: u64,
    max_production: u64,
    max_water: u64,
    lower_volume: u64,
    dispersers: u64,
    vents: u64,
    blades: u64,
    coils: u64,
    dumping: i32,
    input_level: f64,
    dispersers_limiting: bool,
    vents_limiting: bool,
}

/// Each value in four 16-bit pieces, the dump mode, the input bar's level (two pieces) and the two limits (one).
const DATA: usize = 15 * 4 + 1 + 2 + 1;

fn push(out: &mut Vec<i32>, v: u64, words: usize) {
    out.extend((0..words).map(|i| ((v >> (16 * i)) & 0xFFFF) as i32));
}

fn read(data: &[i32], at: usize, words: usize) -> u64 {
    (0..words).fold(0u64, |v, i| v | (u64::from(data[at + i] as u16) << (16 * i)))
}

fn unpack(data: &[i32]) -> Option<View> {
    if data.len() < DATA {
        return None;
    }
    let v = |i: usize| read(data, 4 * i, 4);
    Some(View {
        steam: v(0),
        steam_cap: v(1),
        energy: v(2),
        energy_cap: v(3),
        last_input: v(4),
        flow: v(5),
        production: v(6),
        max_flow: v(7),
        max_production: v(8),
        max_water: v(9),
        lower_volume: v(10),
        dispersers: v(11),
        vents: v(12),
        blades: v(13),
        coils: v(14),
        dumping: data[60],
        input_level: f64::from(f32::from_bits(read(data, 61, 2) as u32)),
        dispersers_limiting: data[63] & 1 != 0,
        vents_limiting: data[63] & 2 != 0,
    })
}

/// The data slots of a formed turbine, for `mek_multiblock` to show on its casings, valves and vents.
pub(super) fn view(dimension: &str, id: Uuid) -> Vec<i32> {
    let Some(shape) = shape_of(dimension, id) else { return Vec::new() };
    let Some((steam, energy, dumping)) = mb::with_cache(dimension, Kind::Turbine, id, |c| {
        (c.chemicals.first().map_or(0, |s| s.amount().max(0)), c.energy.first().copied().unwrap_or(0), c.dumping)
    }) else {
        return Vec::new();
    };
    let config = settings();
    let state = STATES.with(|s| s.borrow().get(&id).copied()).unwrap_or(State { blades: shape.blades, ..State::default() });
    let rate = config.max_rate(&shape);
    let disperser_flow = f64::from(shape.lower_volume) * f64::from(shape.dispersers()) * config.disperser_chemical_flow;
    let vent_flow = f64::from(shape.vents()) * config.vent_chemical_flow;
    let mut out = Vec::with_capacity(DATA);
    for v in [
        steam as u64,
        config.steam_capacity(&shape).max(0) as u64,
        energy,
        config.energy_capacity(&shape),
        state.last_input.max(0) as u64,
        state.last.flow.max(0) as u64,
        config.production(state.last.flow, state.blades, shape.coils).max(0) as u64,
        config.max_flow(&shape).max(0) as u64,
        config.max_production(&shape, state.blades).max(0) as u64,
        config.max_water_output(&shape).max(0) as u64,
        shape.lower_volume.max(0) as u64,
        shape.dispersers().max(0) as u64,
        shape.vents() as u64,
        state.blades.max(0) as u64,
        shape.coils.max(0) as u64,
    ] {
        push(&mut out, v, 4);
    }
    out.push(dumping);
    let level = if rate == 0.0 { 0.0 } else { (state.last_input as f64 / rate).min(1.0) };
    push(&mut out, u64::from((level as f32).to_bits()), 2);
    out.push(i32::from(disperser_flow < vent_flow) | (i32::from(disperser_flow > vent_flow) << 1));
    out
}

const WIDTH: f32 = 190.0;
const GAUGE: Rect = [6.0, 13.0, 34.0, 56.0];
const RATE_BAR: Rect = [40.0, 13.0, 8.0, 60.0];
const SCREEN: Rect = [50.0, 18.0, 126.0, 50.0];
const POWER_BAR: Rect = [178.0, 16.0, 6.0, 54.0];
const MODE: Rect = [173.0, 72.0, 10.0, 10.0];
/// `GuiTurbineTab`: the statistics tab, left of the panel.
const STATS_TAB: Rect = [-26.0, 6.0, 26.0, 26.0];
const STATS_BUTTON: Rect = [-21.0, 10.0, 18.0, 18.0];

/// `native_screen` of a casing, valve or vent: no slots of its own, the inventory moved right by its wider panel.
pub(super) fn screen() -> NativeScreen {
    PAGE.with(|p| p.set(0));
    NativeScreen { width: WIDTH, height: 166.0, slots: vec![None; 5], inventory_y: 84.0, inventory_x: 15.0, own_labels: true, own_background: false }
}

pub(super) fn click(at: [f32; 2], button: &mut Option<i32>) {
    if mek_ui::inside(STATS_BUTTON, at) {
        PAGE.with(|p| p.set(1 - p.get().min(1)));
        *button = Some(STATS);
    } else if PAGE.with(Cell::get) == 0 && mek_ui::inside(MODE, at) {
        *button = Some(DUMP);
    }
}

fn mode_lines(mode: i32) -> Vec<String> {
    match GasMode::by_id(mode) {
        GasMode::Idle => vec!["Idle".into()],
        GasMode::DumpingExcess => vec!["Dumping excess Steam".into(), "Water will NOT be recycled".into()],
        GasMode::Dumping => vec!["Dumping Steam".into(), "Water will NOT be recycled".into()],
    }
}

fn lines(view: &View, settings: &EnergyConfig) -> Vec<String> {
    vec![
        format!("Production: {}", display::energy(view.production, settings)),
        format!("Flow rate: {} mB/t", display::thousands(view.flow)),
        format!("Capacity: {} mB", display::thousands(view.steam_cap)),
        format!("Max flow: {} mB/t", display::thousands(view.max_flow)),
    ]
}

fn stats_lines(view: &View, settings: &EnergyConfig) -> Vec<(String, bool)> {
    let limiting = |l: bool| if l { " (Limiting)" } else { "" };
    vec![
        (format!("Tank Volume: {}", view.lower_volume), false),
        ("Steam Flow".to_owned(), true),
        (format!("  Dispersers: {}{}", view.dispersers, limiting(view.dispersers_limiting)), false),
        (format!("  Vents: {}{}", view.vents, limiting(view.vents_limiting)), false),
        ("Production".to_owned(), true),
        // `GuiTurbineStats` compares with four blades a coil, whatever `bladesPerCoil` says.
        (format!("  Blades: {}{}", view.blades, limiting(view.coils * 4 > view.blades)), false),
        (format!("  Coils: {}{}", view.coils, limiting(view.coils * 4 < view.blades)), false),
        (format!("Max Production: {}", display::energy(view.max_production, settings)), false),
        (format!("Max Water Output: {} mB/t", display::thousands(view.max_water)), false),
    ]
}

pub(super) fn tooltip(data: &[i32], at: [f32; 2], tips: &mut Vec<String>) {
    if mek_ui::inside(STATS_BUTTON, at) {
        tips.push(if PAGE.with(Cell::get) == 0 { "Turbine Statistics".into() } else { "Main".into() });
        return;
    }
    let Some(view) = unpack(data) else { return };
    let settings = mek_ui::settings();
    if mek_ui::inside(mek_ui::ENERGY_TAB, at) {
        tips.push(format!("Storing: {}", display::energy(view.energy, &settings)));
        tips.push(format!("Producing: {}/t", display::energy(view.production, &settings)));
        return;
    }
    if PAGE.with(Cell::get) == 1 {
        return;
    }
    if mek_ui::inside(GAUGE, at) {
        tips.push(if view.steam == 0 { "Empty".into() } else { format!("Steam: {} mB", display::thousands(view.steam)) });
    } else if mek_ui::inside(RATE_BAR, at) {
        tips.push(format!("Steam Input: {} mB/t", display::thousands(view.last_input)));
    } else if mek_ui::inside(POWER_BAR, at) {
        tips.push(display::fraction(view.energy, view.energy_cap, &settings));
    } else if mek_ui::inside(MODE, at) {
        tips.extend(mode_lines(view.dumping));
    }
}

pub(super) fn draw(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let view = unpack(data);
    let page = PAGE.with(Cell::get);
    let settings = mek_ui::settings();
    let cursor = game.native_cursor();
    let cursor = [cursor[0] - origin[0], cursor[1] - origin[1]];
    let mut pen = Pen { game, batch, origin, cursor };
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [0.0, 0.0, WIDTH, 166.0]);
    pen.holder(STATS_TAB, true);
    pen.image_button("stats", 18.0, STATS_BUTTON, true);
    let unit = if settings.show_fe { "fe" } else { "j" };
    pen.whole(&format!("tabs/energy_info_{unit}"), [26.0, 26.0], mek_ui::ENERGY_TAB);
    pen.centered("Industrial Turbine", 0.0, WIDTH, 5.0, mek_ui::TITLE);
    if page == 1 {
        if let Some(v) = &view {
            for (i, (line, heading)) in stats_lines(v, &settings).iter().enumerate() {
                pen.text(line, [6.0, 17.0 + 7.0 * i as f32], if *heading { 0x202020 } else { mek_ui::TITLE });
            }
        }
    } else {
        // The medium steam gauge.
        pen.nine("gauge/normal", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], GAUGE);
        if let Some(v) = &view {
            let inner = GAUGE[3] - 2.0;
            let level = if v.steam_cap == 0 { 0.0 } else { (v.steam as f64 / v.steam_cap as f64).min(1.0) };
            let h = (level * f64::from(inner)) as i32 as f32;
            if h > 0.0 {
                let tint = mek_chemicals::registry(game).get(STEAM).map_or(0xC0C0C0, |c| c.color & 0xFF_FFFF);
                pen.fill([GAUGE[0] + 1.0, GAUGE[1] + 1.0 + inner - h, GAUGE[2] - 2.0, h], 0xFF00_0000 | tint);
            }
        }
        pen.whole("gauge/medium", [32.0, 54.0], [GAUGE[0] + 1.0, GAUGE[1] + 1.0, 32.0, 54.0]);
        // `GuiVerticalRateBar`: the steam input against the most that flows.
        pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], RATE_BAR);
        let h = (view.as_ref().map_or(0.0, |v| v.input_level.clamp(0.0, 1.0)) * 58.0) as i32 as f32;
        if h > 0.0 {
            pen.sprite("bar/vertical_rate", [0.0, 58.0 - h, 6.0, h], [RATE_BAR[0] + 1.0, RATE_BAR[1] + RATE_BAR[3] - 1.0 - h, 6.0, h]);
        }
        // `GuiVerticalPowerBar`.
        pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], POWER_BAR);
        if let Some(v) = &view {
            let level = if v.energy_cap == 0 { 0.0 } else { v.energy as f64 / v.energy_cap as f64 };
            let h = (level * 52.0) as i32 as f32;
            if h > 0.0 {
                pen.sprite("bar/vertical_power", [0.0, 0.0, 4.0, h], [POWER_BAR[0] + 1.0, POWER_BAR[1] + POWER_BAR[3] - 1.0 - h, 4.0, h]);
            }
        }
        pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], SCREEN);
        if let Some(v) = &view {
            for (i, line) in lines(v, &settings).iter().enumerate() {
                pen.text(line, [SCREEN[0] + 3.0, SCREEN[1] + 3.0 + 10.0 * i as f32], mek_ui::SCREEN_TEXT);
            }
            let icon = match GasMode::by_id(v.dumping) {
                GasMode::Idle => "gas_mode_idle",
                GasMode::DumpingExcess => "gas_mode_excess",
                GasMode::Dumping => "gas_mode_dump",
            };
            pen.whole(icon, [10.0, 10.0], MODE);
        }
    }
    for row in 0..4 {
        let y = if row == 3 { 141.0 } else { 83.0 + 18.0 * row as f32 };
        for col in 0..9 {
            pen.slot("normal", None, 14.0 + 18.0 * col as f32, y);
        }
    }
    // `GuiTurbineStats` has no inventory; its lines here run down to where the label would be.
    if page == 0 {
        pen.text("Inventory", [15.0, 74.0], mek_ui::TITLE);
    }
}

// ---- /mek probe -------------------------------------------------------------------------------------------------

pub(super) fn probe(dimension: &str, id: Uuid) -> Value {
    let shape = shape_of(dimension, id).unwrap_or_default();
    let config = settings();
    let state = STATES.with(|s| s.borrow().get(&id).copied()).unwrap_or_default();
    let (steam, water, energy, dumping) = mb::with_cache(dimension, Kind::Turbine, id, |c| {
        (
            c.chemicals.first().map(|s| json!({ "chemical": s.id(), "amount": s.amount().max(0) })),
            c.fluids.first().map(|f| json!({ "fluid": (!f.is_empty()).then(|| f.name()), "amount": f.amount().max(0) })),
            c.energy.first().copied().unwrap_or(0),
            GasMode::by_id(c.dumping).name(),
        )
    })
    .unwrap_or_default();
    json!({
        "steam": steam, "water": water, "energy": energy, "dumping": dumping,
        "capacities": { "steam": config.steam_capacity(&shape), "water": config.water_capacity(&shape), "energy": config.energy_capacity(&shape) },
        "lower_volume": shape.lower_volume, "rotors": shape.rotors, "blades": state.blades, "coils": shape.coils,
        "condensers": shape.condensers, "dispersers": shape.dispersers(), "complex": shape.complex,
        "vents": shape.vents.iter().map(|(p, s)| json!({ "pos": p, "side": s.name() })).collect::<Vec<_>>(),
        "flow": state.last.flow, "produced": state.last.energy, "water_made": state.last.water, "rotation": state.last.rotation,
        "last_input": state.last_input, "max_flow": config.max_flow(&shape), "max_production": config.max_production(&shape, state.blades),
        "max_water_output": config.max_water_output(&shape),
        "config": { "blades_per_coil": config.blades_per_coil, "vent_chemical_flow": config.vent_chemical_flow,
            "disperser_chemical_flow": config.disperser_chemical_flow, "condenser_rate": config.condenser_rate,
            "energy_capacity_per_volume": config.energy_capacity_per_volume, "chemical_per_tank": config.chemical_per_tank,
            "max_energy_per_steam": config.max_energy_per_steam },
    })
}
