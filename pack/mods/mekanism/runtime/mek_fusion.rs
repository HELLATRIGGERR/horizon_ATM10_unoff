// Mekanism Generators' Fusion Reactor in the game (support module `crate::game::mek_fusion` of the `mekanism` native
// bundle). The rules are `mekanism_energy::fusion` (MIT, from Mekanism Generators 10.7.19, Copyright (c) 2017-2025
// Aidan C. Brady); the reactor forms through `mek_multiblock` (`Kind::Fusion`: frames, ports, the controller, logic
// adapters, laser focus matrices and reactor glass, air inside), and its tanks, Hohlraum slot, Joules, case heat and
// core (plasma, injection rate, burning) live in the cache by its id there (`chemical_tanks`: deuterium, tritium, D-T
// fuel, steam; `fluid_tanks`: water). This module is the glue: its settings from `generators.toml`
// `[fusion_reactor]`/`[hohlraum]`, the air of its corners' biomes, its tick (`FusionReactorMultiblockData.tick`: the
// ignition by a full Hohlraum, the burn, the heat, the Joules and steam out of its output ports), the laser Joules a
// focus matrix hands it, the ports' chemical and fluid handlers and their mode (`active`, the Configurator's sneaking
// use), the logic adapters' redstone (`checkMode`), `/mek fusion` (the injection rate as the screen's field sets it,
// an adapter's mode) and `/mek probe`. Not here: its screens, the plasma's drawing and sound, entities hurt inside it
// (`kill`), heat to adjacent blocks and the ports' Forge Energy.

use super::{
    Game, mek_chemicals,
    mek_multiblock::{self as mb, Formed, Kind},
    mek_world,
};
use horizon_client::items::ItemStack;
use horizon_world::{
    fluid_stack::FluidStack,
    level::flags,
    nbt::{Compound, Tag},
};
use mekanism_energy::{
    chemical::{ChemicalHandler, ChemicalStack},
    config, evaporation,
    fusion::{self, Core, FusionConfig, Logic, Tanks},
    multiblock::{Cuboid, Failure, Formation, Inner, Node, Pos, Relative, Uuid},
    sides::Direction,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    time::{Duration, Instant},
};

pub(super) const FRAME: &str = "mekanismgenerators:fusion_reactor_frame";
pub(super) const PORT: &str = "mekanismgenerators:fusion_reactor_port";
pub(super) const CONTROLLER: &str = "mekanismgenerators:fusion_reactor_controller";
pub(super) const ADAPTER: &str = "mekanismgenerators:fusion_reactor_logic_adapter";
pub(super) const FOCUS: &str = "mekanismgenerators:laser_focus_matrix";
pub(super) const OTHERS: [&str; 3] = [CONTROLLER, ADAPTER, FOCUS];
const HOHLRAUM: &str = "mekanismgenerators:hohlraum";
const DEUTERIUM: &str = "mekanismgenerators:deuterium";
const TRITIUM: &str = "mekanismgenerators:tritium";
const FUEL: &str = "mekanismgenerators:fusion_fuel";
const STEAM: &str = "mekanism:steam";
const CHEMICALS: [&str; 4] = [DEUTERIUM, TRITIUM, FUEL, STEAM];

thread_local! {
    static SETTINGS: RefCell<Option<(FusionConfig, config::Stamp, Instant)>> = const { RefCell::new(None) };
    /// `biomeAmbientTemp` of each formed reactor.
    static AMBIENT: RefCell<HashMap<Uuid, f64>> = RefCell::new(HashMap::new());
    /// The logic adapters of formed reactors: whether each gives its signal, and its reactor's box.
    static ADAPTERS: RefCell<HashMap<(String, Pos), (bool, Cuboid)>> = RefCell::new(HashMap::new());
}

/// `generators.toml` `[fusion_reactor]` and `[hohlraum]`, read again within a second of a change.
pub(super) fn settings() -> FusionConfig {
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
                Some(d) => (FusionConfig::load(d), config::stamp(d), Instant::now()),
                None => (FusionConfig::default(), [None; config::FILES.len()], Instant::now()),
            });
        }
        s.as_ref().map(|(c, _, _)| *c).unwrap_or_default()
    })
}

/// A world opens or closes.
pub(super) fn clear() {
    AMBIENT.with(|a| a.borrow_mut().clear());
    ADAPTERS.with(|a| a.borrow_mut().clear());
}

fn ambient_of(id: Uuid) -> f64 {
    AMBIENT.with(|a| a.borrow().get(&id).copied()).unwrap_or(evaporation::AMBIENT)
}

/// `FormationProtocol.doUpdate` with the `FusionReactorValidator` over the structure `nodes`.
pub(super) fn form(game: &mut Game<'_>, nodes: &BTreeMap<Pos, Node>) -> Result<Formation, Failure> {
    let controllers: Vec<Pos> = nodes.keys().copied().filter(|p| game.native_block(*p) == CONTROLLER).collect();
    fusion::form(nodes, |p| controllers.contains(&p), |p| if mb::is_air(&game.native_block(p)) { Inner::Air } else { Inner::Invalid })
}

/// `onCreated` and `FusionReactorCache.apply`: the air of its eight corners, the cache's plasma, rate (a changed one
/// setting the water and steam room) and burning flag on a new multiblock, its case's heat.
pub(super) fn created(game: &mut Game<'_>, dimension: &str, id: Uuid) {
    let Some(c) = mb::with_formed(dimension, id, |f| f.cuboid) else { return };
    let corners: Vec<f32> = [c.min[0], c.max[0]]
        .into_iter()
        .flat_map(|x| [c.min[1], c.max[1]].into_iter().flat_map(move |y| [c.min[2], c.max[2]].into_iter().map(move |z| [x, y, z])))
        .filter_map(|p| game.native_biome_climate(p).map(|b| b.temperature))
        .collect();
    let ambient = evaporation::ambient(&corners);
    let config = settings();
    mb::with_cache(dimension, Kind::Fusion, id, |cache| {
        let core = match cache.fusion {
            Some(k) => Core::applied(ambient, k.plasma, k.rate, k.burning, &config),
            None => Core::new(ambient),
        };
        if let Some(h) = cache.heat.first_mut() {
            *h = Some(h.unwrap_or(evaporation::Heat { stored: ambient * fusion::CASE_HEAT_CAPACITY, capacity: fusion::CASE_HEAT_CAPACITY }));
        }
        cap_tanks(cache, &core);
        cache.fusion = Some(core);
    });
    AMBIENT.with(|a| a.borrow_mut().insert(id, ambient));
}

/// `setInjectionRate`'s cut of the water and steam to their room.
fn cap_tanks(cache: &mut mb::Cache, core: &Core) {
    if let Some(w) = cache.fluids.first_mut()
        && w.amount() > core.max_water
    {
        *w = if core.max_water > 0 { w.copy_with_amount(core.max_water) } else { FluidStack::EMPTY };
    }
    if let Some(s) = cache.chemicals.get_mut(3)
        && s.amount() > core.max_steam
    {
        *s = s.copy_with_amount(core.max_steam).normalized();
    }
}

/// The reactor broke: its adapters stop giving a signal.
pub(super) fn removed(dimension: &str, formed: &Formed) {
    AMBIENT.with(|a| a.borrow_mut().remove(&formed.id));
    ADAPTERS.with(|a| a.borrow_mut().retain(|(d, p), _| !(d == dimension && formed.locations.contains(p))));
}

fn tanks_of(cache: &mb::Cache, ambient: f64) -> Tanks {
    let amount = |i: usize| cache.chemicals.get(i).map_or(0, |s| s.amount().max(0));
    Tanks {
        deuterium: amount(0),
        tritium: amount(1),
        fuel: amount(2),
        steam: amount(3),
        water: cache.fluids.first().map_or(0, |f| f.amount().max(0)),
        energy: cache.energy.first().copied().unwrap_or(0),
        heat: cache.heat.first().copied().flatten().map_or(ambient, |h| h.stored),
    }
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

/// `hasHohlraum`: the D-T fuel a Hohlraum in the slot holds when it is full. As every Mekanism item here keeps its
/// chemical, it lives in the item's custom data (`mekanism.chemical_tanks`, the form `mek_tanks` reads).
fn full_hohlraum(stack: Option<&ItemStack>, config: &FusionConfig) -> Option<i64> {
    let stack = stack.filter(|s| s.count > 0 && &*s.item == HOHLRAUM)?;
    let data = mek_world::item_data(stack)?;
    let Some(Tag::List(tanks)) = data.get(&mek_world::key("chemical_tanks")) else { return None };
    let Some(Tag::Compound(first)) = tanks.first() else { return None };
    let Some(Tag::Compound(stored)) = first.get(&mek_world::key("stored")) else { return None };
    let amount = mek_world::number(stored.get(&mek_world::key("amount")))?;
    let fuel = matches!(stored.get(&mek_world::key("id")), Some(Tag::String(s)) if s.to_string_lossy() == FUEL);
    (fuel && amount > 0 && amount == config.hohlraum_capacity).then_some(amount)
}

/// `native_block_entity_slot_accepts` of the controller's slot (`ReactorInventorySlot`): a Hohlraum.
pub(super) fn slot_accepts(stack: &ItemStack) -> bool {
    &*stack.item == HOHLRAUM
}

fn output_mode(game: &mut Game<'_>, pos: Pos) -> bool {
    game.native_with_mod_block_entity(pos, |m| mek_world::number(m.nbt.get(&mek_world::key("active_state"))).is_some_and(|v| v != 0)).unwrap_or(false)
}

/// `FusionReactorMultiblockData.tick`: the rules' tick, then the Joules and the steam out of its output ports
/// (`CableUtils.emit`, `ChemicalUtil.emit`), then its logic adapters (`onUpdateServer`: a signal that changed tells
/// the neighbours).
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, id: Uuid) {
    let Some(formed) = mb::with_formed(dimension, id, Clone::clone) else { return };
    let Some(before) = mb::with_cache(dimension, Kind::Fusion, id, |c| c.clone()) else { return };
    if before.chemicals.len() < 4 || before.fluids.is_empty() || before.energy.is_empty() || before.heat.is_empty() || before.items.is_empty() {
        return;
    }
    let config = settings();
    let ambient = ambient_of(id);
    let mut cache = before.clone();
    let mut core = cache.fusion.unwrap_or_else(|| Core::new(ambient));
    let mut t = tanks_of(&cache, ambient);
    let hohlraum = full_hohlraum(cache.items[0].as_ref(), &config);
    let out = fusion::tick(&mut t, &mut core, hohlraum, formed.valves.len(), ambient, &config);
    if out.hohlraum_used {
        cache.items[0] = None;
    }
    for (i, (amount, chemical)) in [t.deuterium, t.tritium, t.fuel, t.steam].into_iter().zip(CHEMICALS).enumerate() {
        cache.chemicals[i] = set_tank(&cache.chemicals[i], amount, chemical);
    }
    cache.fluids[0] = if t.water > 0 { cache.fluids[0].copy_with_amount(t.water) } else { FluidStack::EMPTY };
    cache.energy[0] = t.energy;
    cache.heat[0] = Some(evaporation::Heat { stored: t.heat, capacity: fusion::CASE_HEAT_CAPACITY });
    cache.fusion = Some(core);
    if cache != before {
        mb::with_cache(dimension, Kind::Fusion, id, |c| *c = cache.clone());
    }
    let outputs: Vec<(Pos, Direction)> = formed.valves.iter().copied().filter(|(p, _)| output_mode(game, *p)).collect();
    if !outputs.is_empty() {
        if t.energy > 0 {
            let sent = super::mek_turbine::emit_energy(game, dimension, &outputs, t.energy);
            if sent > 0 {
                mb::with_cache(dimension, Kind::Fusion, id, |c| c.energy[0] = c.energy[0].saturating_sub(sent));
            }
        }
        let steam = cache.chemicals[3].clone();
        if !steam.is_empty() {
            let targets: Vec<(Pos, Direction)> = outputs.iter().map(|(p, side)| (mek_world::beside(*p, *side), side.opposite())).collect();
            let sent = mek_chemicals::emit_to(game, &targets, &steam);
            if sent > 0 {
                mb::with_cache(dimension, Kind::Fusion, id, |c| c.chemicals[3] = c.chemicals[3].copy_with_amount(c.chemicals[3].amount() - sent).normalized());
            }
        }
    }
    let t = mb::with_cache(dimension, Kind::Fusion, id, |c| tanks_of(c, ambient)).unwrap_or(t);
    let adapters: Vec<Pos> = formed.locations.iter().copied().filter(|p| game.native_block(*p) == ADAPTER).collect();
    for p in adapters {
        let (logic, cooled) = adapter_nbt(game, p);
        let on = fusion::check(logic, cooled, &core, &t, &config);
        let changed = ADAPTERS.with(|a| a.borrow_mut().insert((dimension.to_owned(), p), (on, formed.cuboid)).is_none_or(|(was, _)| was != on));
        if changed {
            game.native_update_signal_neighbors(dimension, p);
        }
    }
}

/// A logic adapter's mode (`logic_type`) and `active_cooled`.
fn adapter_nbt(game: &mut Game<'_>, p: Pos) -> (Logic, bool) {
    game.native_with_mod_block_entity(p, |m| {
        let n = |k: &str| mek_world::number(m.nbt.get(&mek_world::key(k))).unwrap_or(0);
        (Logic::by_id(n("logic_type") as i32), n("active_cooled") != 0)
    })
    .unwrap_or_default()
}

/// `TileEntityLaserFocusMatrix.receiveLaserEnergy`: a formed reactor's plasma takes laser Joules. Whether `pos` was a
/// focus matrix.
pub(super) fn laser(game: &mut Game<'_>, pos: Pos, joules: u64) -> bool {
    if game.native_block(pos) != FOCUS {
        return false;
    }
    let dimension = game.native_dimension().to_owned();
    if let Some(id) = mb::formed_at(&dimension, Kind::Fusion, pos) {
        mb::with_cache(&dimension, Kind::Fusion, id, |c| {
            if let Some(core) = c.fusion.as_mut() {
                core.add_energy(joules);
            }
        });
    }
    true
}

// ---- The redstone hook points (API 38) --------------------------------------------------------------------------

/// `native_is_signal_source`: a logic adapter.
pub(super) fn is_signal_source(block: Option<&str>, source: &mut bool) {
    if block == Some(ADAPTER) {
        *source = true;
    }
}

/// `native_block_signal`: 15 to a block outside its reactor while its mode holds (`getRedstoneLevel`).
pub(super) fn block_signal(dimension: &str, block: Option<&str>, pos: Pos, toward: [i32; 3], direct: bool, signal: &mut i32) {
    if block != Some(ADAPTER) {
        return;
    }
    let reader = [pos[0] + toward[0], pos[1] + toward[1], pos[2] + toward[2]];
    let out = ADAPTERS.with(|a| a.borrow().get(&(dimension.to_owned(), pos)).copied());
    *signal = match out {
        Some((true, cuboid)) if !direct && cuboid.relative(reader) == Relative::Outside => 15,
        _ => 0,
    };
}

// ---- The ports: chemicals, water, mode ---------------------------------------------------------------------------

/// A port's tanks (`getChemicalTanks`, every mode): deuterium, tritium and D-T fuel in (which give nothing out),
/// steam out (which takes nothing in); the null side reads only.
struct PortChemical {
    tanks: [(ChemicalStack, i64); 4],
    side: bool,
}

impl ChemicalHandler for PortChemical {
    fn tanks(&self) -> usize {
        4
    }

    fn in_tank(&self, tank: usize) -> ChemicalStack {
        self.tanks.get(tank).map_or(ChemicalStack::EMPTY, |t| t.0.clone())
    }

    fn capacity(&self, tank: usize) -> i64 {
        self.tanks.get(tank).map_or(0, |t| t.1)
    }

    fn is_valid(&self, tank: usize, stack: &ChemicalStack) -> bool {
        CHEMICALS.get(tank).is_some_and(|c| stack.id() == Some(c))
    }

    fn insert_tank(&mut self, tank: usize, stack: &ChemicalStack, simulate: bool) -> ChemicalStack {
        if !self.side || tank > 2 || stack.is_empty() || !self.is_valid(tank, stack) {
            return stack.clone();
        }
        let t = &mut self.tanks[tank];
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
        if !self.side || tank != 3 || amount <= 0 || self.tanks[3].0.is_empty() {
            return ChemicalStack::EMPTY;
        }
        let t = &mut self.tanks[3];
        let n = amount.min(t.0.amount());
        let got = t.0.copy_with_amount(n);
        if !simulate {
            t.0 = t.0.copy_with_amount(t.0.amount() - n).normalized();
        }
        got
    }
}

/// `Capabilities.CHEMICAL` of a port at `pos` through `face` (none while unformed): None for any other block.
pub(super) fn chemical_handler<R>(dimension: &str, pos: Pos, block: &str, face: Option<Direction>, use_it: impl FnOnce(&mut dyn ChemicalHandler) -> R) -> Option<R> {
    if block != PORT {
        return None;
    }
    let config = settings();
    let held = mb::formed_at(dimension, Kind::Fusion, pos).and_then(|id| mb::with_cache(dimension, Kind::Fusion, id, |c| (c.chemicals.clone(), c.fusion)).map(|s| (id, s)));
    let caps = |core: Option<Core>| [config.fuel_capacity, config.fuel_capacity, config.fuel_capacity, core.map_or(0, |k| k.max_steam)];
    let mut handler = match &held {
        Some((_, (s, core))) if s.len() >= 4 => {
            let caps = caps(*core);
            PortChemical { tanks: std::array::from_fn(|i| (s[i].clone(), caps[i])), side: face.is_some() }
        }
        _ => PortChemical { tanks: std::array::from_fn(|_| (ChemicalStack::EMPTY, 0)), side: false },
    };
    let answer = use_it(&mut handler);
    if let Some((id, (before, _))) = held
        && before.len() >= 4
        && (0..4).any(|i| handler.tanks[i].0 != before[i])
    {
        mb::with_cache(dimension, Kind::Fusion, id, |c| {
            for (i, (s, _)) in handler.tanks.into_iter().enumerate() {
                c.chemicals[i] = s;
            }
        });
    }
    Some(answer)
}

/// `native_fluid_handler_fill` of a port: water up to the room its injection rate set (`VariableCapacityFluidTank.input`);
/// the null side reads only. Whether `block` was a port.
pub(super) fn fluid_fill(dimension: &str, pos: Pos, face: bool, block: &str, resource: &FluidStack, simulate: bool, filled: &mut Option<i32>) -> bool {
    if block != PORT {
        return false;
    }
    *filled = Some(0);
    let Some(id) = mb::formed_at(dimension, Kind::Fusion, pos).filter(|_| face) else { return true };
    if resource.is_empty() || !resource.is_tag("minecraft:water") {
        return true;
    }
    let took = mb::with_cache(dimension, Kind::Fusion, id, |c| {
        let room = c.fusion.map_or(0, |k| k.max_water);
        let Some(tank) = c.fluids.get_mut(0) else { return 0 };
        let same = tank.is_empty() || FluidStack::is_same_fluid_same_components(tank, resource);
        let took = mekanism_energy::dynamic_tank::fluid_room(tank.amount().max(0), room, same, false, resource.amount());
        if !simulate && took > 0 {
            *tank = if tank.is_empty() { resource.copy_with_amount(took) } else { tank.copy_with_amount(tank.amount() + took) };
        }
        took
    })
    .unwrap_or(0);
    *filled = Some(took);
    true
}

/// The Configurator's sneaking use on a port (`onSneakRightClick`): input and output swap; the action bar names the
/// mode it had.
pub(super) fn toggle(game: &mut Game<'_>, pos: Pos) -> Option<bool> {
    let old = game.native_with_mod_block_entity(pos, |m| {
        let old = mek_world::number(m.nbt.get(&mek_world::key("active_state"))).is_some_and(|v| v != 0);
        m.nbt.insert(mek_world::key("active_state"), Tag::Byte(i8::from(!old)));
        old
    })?;
    game.native_set_block_property_with(pos, "active", if old { "false" } else { "true" }, flags::UPDATE_ALL);
    let player = game.native_acting_id();
    game.native_action_bar(player, horizon_client::text::Text::literal(format!("Toggled Reactor Port eject mode to: {}.", if old { "Output" } else { "Input" })));
    Some(!old)
}

// ---- /mek fusion and /mek probe ----------------------------------------------------------------------------------

/// `/mek fusion x y z rate <n>` (`setInjectionRateFromPacket`: even, 0 to 98), `logic <mode> [cooled]` of a logic
/// adapter there.
pub(super) fn command(game: &mut Game<'_>, pos: Pos, words: &[&str]) -> Value {
    let dimension = game.native_dimension().to_owned();
    if let ["logic", mode, rest @ ..] = words {
        let Some(logic) = Logic::from_name(mode) else { return json!({ "error": "logic: disabled | ready | capacity | depleted" }) };
        let cooled = rest.first() == Some(&"cooled");
        let done = game.native_with_mod_block_entity(pos, |m| {
            (m.id == ADAPTER).then(|| {
                m.nbt.insert(mek_world::key("logic_type"), Tag::Int(logic.id()));
                m.nbt.insert(mek_world::key("active_cooled"), Tag::Byte(i8::from(cooled)));
            })
        });
        return match done.flatten() {
            Some(()) => json!({ "pos": pos, "logic": logic.name(), "cooled": cooled }),
            None => json!({ "pos": pos, "error": "no fusion reactor logic adapter here" }),
        };
    }
    let Some(owner) = mb::formed_at(&dimension, Kind::Fusion, pos) else { return json!({ "pos": pos, "error": "no formed fusion reactor here" }) };
    let config = settings();
    let done = match words {
        ["rate", r] => match r.parse::<i32>() {
            Ok(v) => mb::with_cache(&dimension, Kind::Fusion, owner, |c| {
                let Some(mut core) = c.fusion else { return };
                core.set_rate(Core::rate_from_packet(v), &config);
                cap_tanks(c, &core);
                c.fusion = Some(core);
            })
            .ok_or("no cache"),
            Err(_) => Err("rate: a number"),
        },
        _ => Err("usage: /mek fusion <x> <y> <z> rate <n> | logic <mode> [cooled]"),
    };
    match done {
        Ok(()) => probe(game, &dimension, owner),
        Err(e) => json!({ "pos": pos, "error": e }),
    }
}

/// The controller's screen data: none yet (its screens aren't ported).
pub(super) fn view(_dimension: &str, _id: Uuid) -> Vec<i32> {
    Vec::new()
}

pub(super) fn probe(game: &mut Game<'_>, dimension: &str, id: Uuid) -> Value {
    let config = settings();
    let ambient = ambient_of(id);
    let Some((core, t, hohlraum)) = mb::with_cache(dimension, Kind::Fusion, id, |c| {
        (c.fusion.unwrap_or_else(|| Core::new(ambient)), tanks_of(c, ambient), c.items.first().cloned().flatten().map(|s| s.item.to_string()))
    }) else {
        return json!({});
    };
    let ports: Vec<Value> = mb::with_formed(dimension, id, |f| f.valves.clone())
        .unwrap_or_default()
        .into_iter()
        .map(|(p, side)| json!({ "pos": p, "side": side.name(), "output": output_mode(game, p) }))
        .collect();
    let adapters: Vec<Value> = ADAPTERS.with(|a| {
        a.borrow().iter().filter(|((d, _), _)| d == dimension).map(|((_, p), (on, _))| json!({ "pos": p, "on": on })).collect()
    });
    json!({
        "plasma": core.plasma, "case": t.heat, "burning": core.burning, "rate": core.rate, "last_burned": core.last_burned,
        "last_plasma": core.last_plasma, "last_case": core.last_case, "ambient": ambient,
        "deuterium": t.deuterium, "tritium": t.tritium, "fuel": t.fuel, "steam": t.steam, "water": t.water, "energy": t.energy,
        "max_water": core.max_water, "max_steam": core.max_steam, "hohlraum": hohlraum,
        "environment_loss": core.environment_loss,
        "ignition": fusion::ignition_temperature(false, &config), "ignition_cooled": fusion::ignition_temperature(true, &config),
        "min_injection": fusion::min_injection_rate(false, &config), "passive": fusion::passive_generation(&core, false, true, &config),
        "ports": ports, "adapters": adapters,
    })
}
