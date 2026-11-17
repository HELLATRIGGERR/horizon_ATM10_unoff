// Mekanism's chemicals in the game (support module `crate::game::mek_chemicals`
// of the `mekanism` native bundle). The rules are the bundle's crate
// (`mekanism_energy::chemical`, MIT, from Mekanism 10.7.19, Copyright (c)
// 2017-2025 Aidan C. Brady); this module is the glue: the `mekanism:chemical`
// registry read from the content's declarations
// (`data/<ns>/mekanism/chemical/<path>.json`, listed with hook points API
// 423's `native_data_files`), chemical stacks and tanks in Mekanism's saved
// form (`chemical_tanks: [{tank: 0b, stored: {id, amount}}]`), the chemical
// handler of the bundle's blocks by side (Mekanism's
// `Capabilities.CHEMICAL.block()`: the infuser's tank, the separator's two),
// the Chemical Tanks' (`mek_tanks`) and the Pressurized Tubes' networks
// (`mek_tubes`), the even split of a tank's contents among the handlers on
// its output faces (`ChemicalUtil.emit`), and `/mek chemical` and
// `/mek chemicals` for tests. No other native mod has
// chemicals yet, so the capability stays in the bundle; AE2's Applied
// Mekanistics reaches it through hook points API 435
// (`native_chemical_handler_*`: `tanks`, `insert`, `extract` below;
// docs/appmek.md).

use super::{
    Game, mek_machines,
    mek_world::{key, number},
};
use horizon_world::nbt::{Compound, Tag};
use mekanism_energy::{
    chemical::{Chemical, ChemicalHandler, ChemicalRegistry, ChemicalStack},
    sides::Direction,
};
use serde_json::{Value, json};
use std::sync::{Arc, RwLock};

/// Where a content mod declares its chemicals: `data/<ns>/<DIR>/<path>.json`.
const DIR: &str = "mekanism/chemical";

/// The registry, read once a process: the content root doesn't change while
/// the game runs. Both sides of a game read the same root.
static REGISTRY: RwLock<Option<Arc<ChemicalRegistry>>> = RwLock::new(None);

/// The registry, read from the content's declarations on first use. A
/// declaration that doesn't read is left out with a warning.
pub(super) fn registry(game: &Game<'_>) -> Arc<ChemicalRegistry> {
    if let Some(r) = loaded() {
        return r;
    }
    let mut list = Vec::new();
    for path in game.native_data_files(DIR) {
        let Some(id) = declared_id(&path) else {
            continue;
        };
        let read = game.native_data(&path).and_then(|b| serde_json::from_slice::<Value>(&b).ok());
        match read.map(|v| Chemical::from_declaration(&id, &v)) {
            Some(Ok(c)) => list.push(c),
            Some(Err(e)) => log::warn!("[mekanism] {path}: {e}; left out"),
            None => log::warn!("[mekanism] {path} is not JSON; left out"),
        }
    }
    let r = Arc::new(ChemicalRegistry::new(list));
    log::info!("[mekanism] {} chemicals registered", r.len());
    if let Ok(mut slot) = REGISTRY.write() {
        *slot = Some(r.clone());
    }
    r
}

/// The registry when something has read it (hooks with no game at hand: a
/// screen's tooltip).
pub(super) fn loaded() -> Option<Arc<ChemicalRegistry>> {
    REGISTRY.read().ok()?.clone()
}

/// `data/<ns>/mekanism/chemical/<path>.json` is `<ns>:<path>`.
fn declared_id(path: &str) -> Option<String> {
    let (ns, rest) = path.strip_prefix("data/")?.split_once('/')?;
    let p = rest.strip_prefix(DIR)?.strip_prefix('/')?.strip_suffix(".json")?;
    Some(format!("{ns}:{p}"))
}

/// A chemical's name: its translation (`chemical.mekanism.hydrogen`) as the
/// content's language has it, else its path in title case.
pub(super) fn name(chemical: &Chemical) -> String {
    let fallback = chemical
        .id
        .split_once(':')
        .map_or(chemical.id.as_str(), |(_, p)| p)
        .split(['_', '/'])
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ");
    horizon_client::lang::get_or(&chemical.translation_key(), &fallback).to_string()
}

/// The GUI sprite of a chemical's icon (`mekanism-content` copies it there):
/// `<its namespace>:gui/chemical/<icon namespace>/<icon path>`.
pub(super) fn gui_sprite(chemical: &Chemical) -> String {
    let ns = chemical.id.split_once(':').map_or("minecraft", |(n, _)| n);
    let (tns, tpath) = chemical.texture.split_once(':').unwrap_or(("minecraft", &chemical.texture));
    format!("{ns}:gui/chemical/{tns}/{tpath}")
}

// ---- The saved form ----------------------------------------------------------------

/// `ChemicalStack.parseOptional` of a saved `{id, amount}`.
pub(super) fn stack_of(stored: &Compound, registry: &ChemicalRegistry) -> ChemicalStack {
    let id = match stored.get(&key("id")) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    };
    ChemicalStack::from_saved(id.as_deref(), number(stored.get(&key("amount"))), registry)
}

/// `ChemicalStack.save`: `{id, amount}` with a long amount; none when empty.
pub(super) fn stack_nbt(stack: &ChemicalStack) -> Option<Compound> {
    let id = stack.id()?;
    let mut c = Compound::new();
    c.insert(key("id"), Tag::String(key(id)));
    c.insert(key("amount"), Tag::Long(stack.amount()));
    Some(c)
}

/// `chemical_tanks` (`ContainerType.CHEMICAL`): each of `count` tanks by its
/// `tank` index, empty where none is saved.
pub(super) fn read_tanks(nbt: &Compound, count: usize, registry: &ChemicalRegistry) -> Vec<ChemicalStack> {
    let mut out = vec![ChemicalStack::EMPTY; count];
    if let Some(Tag::List(list)) = nbt.get(&key("chemical_tanks")) {
        for entry in list {
            if let Tag::Compound(t) = entry
                && let Some(i) = number(t.get(&key("tank"))).and_then(|i| usize::try_from(i).ok())
                && i < count
                && let Some(Tag::Compound(stored)) = t.get(&key("stored"))
            {
                out[i] = stack_of(stored, registry);
            }
        }
    }
    out
}

/// Writes `chemical_tanks` as Mekanism does: the tanks that hold something,
/// none at all when every one is empty.
pub(super) fn write_tanks(nbt: &mut Compound, tanks: &[ChemicalStack]) {
    let list: Vec<Tag> = tanks
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let mut t = Compound::new();
            t.insert(key("tank"), Tag::Byte(i as i8));
            t.insert(key("stored"), Tag::Compound(stack_nbt(s)?));
            Some(Tag::Compound(t))
        })
        .collect();
    if list.is_empty() {
        nbt.remove(&key("chemical_tanks"));
    } else {
        nbt.insert(key("chemical_tanks"), Tag::List(list));
    }
}

// ---- The handler ---------------------------------------------------------------------

/// `Capabilities.CHEMICAL.block()` of the bundle's block at `pos` through
/// `face` (none: the null side, read only): `use_it` runs on its handler there,
/// and what it changed is kept. None where the block has no handler on that
/// side (another block, a side its config closes to every tank). The engine's own blocks
/// (AE2's ME Interface and Pattern Provider, Applied Mekanistics' `GenericStackChemicalStorage`)
/// answer through the helpers of API 437, so machines, tanks and tubes push into them.
pub(super) fn with_handler<R>(game: &mut Game<'_>, pos: [i32; 3], face: Option<Direction>, use_it: impl FnOnce(&mut dyn ChemicalHandler) -> R) -> Option<R> {
    let dimension = game.native_dimension();
    let engine_face = face.map(super::mek_world::engine_face);
    if let Some(tanks) = game.native_chemical_tanks(dimension, pos, engine_face) {
        return Some(use_it(&mut EngineHandler { game, dimension, pos, face: engine_face, tanks }));
    }
    with_own_handler(game, pos, face, use_it)
}

/// `with_handler` of the bundle's own blocks alone (API 435's hook points answer for them only).
fn with_own_handler<R>(game: &mut Game<'_>, pos: [i32; 3], face: Option<Direction>, use_it: impl FnOnce(&mut dyn ChemicalHandler) -> R) -> Option<R> {
    let registry = registry(game);
    match shared_handler(game, pos, face, &registry, use_it) {
        Ok(found) => found,
        Err(use_it) => game.native_with_mod_block_entity(pos, |m| entity_handler(m, face, &registry, use_it)).flatten(),
    }
}

/// `with_handler` with the game only read (`native_chemical_handler_tanks`, AE2's storage bus
/// listing a tank): a block entity's handler runs on a copy of it, so nothing `use_it` does is
/// kept; ask it only to read or simulate.
fn with_handler_read<R>(game: &Game<'_>, pos: [i32; 3], face: Option<Direction>, use_it: impl FnOnce(&mut dyn ChemicalHandler) -> R) -> Option<R> {
    let registry = registry(game);
    match shared_handler(game, pos, face, &registry, use_it) {
        Ok(found) => found,
        Err(use_it) => entity_handler(&mut game.native_mod_block_entity(pos)?, face, &registry, use_it),
    }
}

/// The handlers kept outside a block entity (the tubes' networks, the multiblocks' valves and
/// ports), `use_it` given back for any other block.
fn shared_handler<R, F: FnOnce(&mut dyn ChemicalHandler) -> R>(game: &Game<'_>, pos: [i32; 3], face: Option<Direction>, registry: &ChemicalRegistry, use_it: F) -> Result<Option<R>, F> {
    let dimension = game.native_dimension();
    if super::mek_tubes::is_tube(dimension, pos) {
        return Ok(super::mek_tubes::with_handler(dimension, pos, face, use_it));
    }
    let block = game.native_block(pos);
    Ok(if block == super::mek_dynamic_tank::VALVE {
        super::mek_dynamic_tank::chemical_handler(dimension, pos, super::mek_dynamic_tank::VALVE, face, registry, use_it)
    } else if block == super::mek_boiler::VALVE {
        super::mek_boiler::chemical_handler(game, dimension, pos, super::mek_boiler::VALVE, face, registry, use_it)
    } else if block == super::mek_turbine::VALVE {
        super::mek_turbine::chemical_handler(dimension, pos, super::mek_turbine::VALVE, face, registry, use_it)
    } else if block == super::mek_fission::PORT {
        super::mek_fission::chemical_handler(game, dimension, pos, super::mek_fission::PORT, face, registry, use_it)
    } else if block == super::mek_fusion::PORT {
        super::mek_fusion::chemical_handler(dimension, pos, &block, face, use_it)
    } else if block == super::mek_sps::PORT {
        super::mek_sps::chemical_handler(dimension, pos, &block, face, use_it)
    } else {
        return Err(use_it);
    })
}

/// The handler of a block entity of the bundle's: a tank's, a gas machine's, a factory's or
/// another machine's.
fn entity_handler<R>(m: &mut horizon_client::containers::ModBlockEntity, face: Option<Direction>, registry: &ChemicalRegistry, use_it: impl FnOnce(&mut dyn ChemicalHandler) -> R) -> Option<R> {
    if super::mek_tanks::tier_of(&m.id).is_some() {
        super::mek_tanks::chemical_handler(m, face, registry, use_it)
    } else if super::mek_gas::is_gas(&m.id) {
        super::mek_gas::chemical_handler(m, face, registry, use_it)
    } else if super::mek_factories::has_chemicals(&m.id) {
        super::mek_factories::chemical_handler(m, face, registry, use_it)
    } else {
        mek_machines::chemical_handler(m, face, registry, use_it)
    }
}

// ---- The engine's blocks (helpers API 437) ------------------------------------------------------

/// The chemical handler of an engine's block (AE2's generic inventories) as Mekanism meets it: its tanks as
/// `native_chemical_tanks` lists them, a move through `native_chemical_insert` / `native_chemical_extract`, which
/// place a chemical slot by slot as AE2's inventory does. Its tanks are read again after each real move.
struct EngineHandler<'a, 'g> {
    game: &'a mut Game<'g>,
    dimension: &'static str,
    pos: [i32; 3],
    face: Option<horizon_core::geometry::Direction>,
    tanks: Vec<horizon_client::mod_api::NativeChemicalTankInfo>,
}

impl EngineHandler<'_, '_> {
    fn moved(&mut self, chemical: &str, amount: i64, insert: bool, simulate: bool) -> i64 {
        if amount <= 0 {
            return 0;
        }
        let (dimension, pos, face) = (self.dimension, self.pos, self.face);
        let moved = if insert {
            self.game.native_chemical_insert(dimension, pos, face, chemical, amount, simulate)
        } else {
            self.game.native_chemical_extract(dimension, pos, face, chemical, amount, simulate)
        }
        .clamp(0, amount);
        if moved > 0 && !simulate {
            self.tanks = self.game.native_chemical_tanks(dimension, pos, face).unwrap_or_default();
        }
        moved
    }
}

impl ChemicalHandler for EngineHandler<'_, '_> {
    fn tanks(&self) -> usize {
        self.tanks.len()
    }
    fn in_tank(&self, tank: usize) -> ChemicalStack {
        match self.tanks.get(tank) {
            Some(t) if !t.chemical.is_empty() && t.amount > 0 => ChemicalStack::new(&t.chemical, t.amount),
            _ => ChemicalStack::EMPTY,
        }
    }
    fn capacity(&self, tank: usize) -> i64 {
        self.tanks.get(tank).map_or(0, |t| t.capacity)
    }
    fn is_valid(&self, tank: usize, _stack: &ChemicalStack) -> bool {
        tank < self.tanks.len()
    }
    /// The inventory places a chemical itself: a tank's insert is the whole handler's.
    fn insert_tank(&mut self, _tank: usize, stack: &ChemicalStack, simulate: bool) -> ChemicalStack {
        self.insert(stack, simulate)
    }
    fn extract_tank(&mut self, tank: usize, amount: i64, simulate: bool) -> ChemicalStack {
        let held = self.in_tank(tank);
        if held.is_empty() || !self.tanks[tank].extractable {
            return ChemicalStack::EMPTY;
        }
        self.extract_stack(&held.copy_with_amount(amount.min(held.amount())), simulate)
    }
    fn insert(&mut self, stack: &ChemicalStack, simulate: bool) -> ChemicalStack {
        let Some(id) = stack.id().map(str::to_owned) else { return ChemicalStack::EMPTY };
        let took = self.moved(&id, stack.amount(), true, simulate);
        if took >= stack.amount() { ChemicalStack::EMPTY } else { stack.copy_with_amount(stack.amount() - took) }
    }
    fn extract(&mut self, amount: i64, simulate: bool) -> ChemicalStack {
        match (0..self.tanks.len()).find(|&t| self.tanks[t].extractable && !self.in_tank(t).is_empty()) {
            Some(t) => {
                let first = self.in_tank(t);
                self.extract_stack(&first.copy_with_amount(amount), simulate)
            }
            None => ChemicalStack::EMPTY,
        }
    }
    fn extract_stack(&mut self, stack: &ChemicalStack, simulate: bool) -> ChemicalStack {
        let Some(id) = stack.id().map(str::to_owned) else { return ChemicalStack::EMPTY };
        let left = self.moved(&id, stack.amount(), false, simulate);
        if left > 0 { stack.copy_with_amount(left) } else { ChemicalStack::EMPTY }
    }
}

// ---- AE2's Applied Mekanistics (hook points API 435) ----------------------------------------

/// `native_chemical_handler_tanks`: the tanks of the block at `pos` through `face` (the engine's
/// side), each with whether a simulated extraction of its chemical gives any. None on another
/// level, a block without a handler there.
pub(super) fn tanks(game: &Game<'_>, dimension: &str, pos: [i32; 3], face: Option<horizon_core::geometry::Direction>) -> Option<Vec<horizon_client::mod_api::NativeChemicalTankInfo>> {
    if dimension != game.native_dimension() {
        return None;
    }
    with_handler_read(game, pos, face.map(super::mek_world::mek_direction), |h| {
        (0..h.tanks())
            .map(|tank| {
                let held = h.in_tank(tank);
                let extractable = !held.is_empty() && !h.extract_stack(&held.copy_with_amount(1), true).is_empty();
                horizon_client::mod_api::NativeChemicalTankInfo {
                    chemical: held.id().unwrap_or_default().to_string(),
                    amount: held.amount().max(0),
                    capacity: h.capacity(tank),
                    extractable,
                }
            })
            .collect()
    })
}

/// `native_chemical_handler_insert`: `insertChemical` of `amount` of `chemical` (a registered
/// one) through `face`: what it took. None on another level, a block without a handler there.
pub(super) fn insert(game: &mut Game<'_>, dimension: &str, pos: [i32; 3], face: Option<horizon_core::geometry::Direction>, chemical: &str, amount: i64, simulate: bool) -> Option<i64> {
    let stack = offered(game, dimension, chemical, amount)?;
    with_own_handler(game, pos, face.map(super::mek_world::mek_direction), |h| amount - h.insert(&stack, simulate).amount().clamp(0, amount))
}

/// `native_chemical_handler_extract`: `extractChemical(stack)` of at most `amount` of `chemical`
/// through `face`: what left.
pub(super) fn extract(game: &mut Game<'_>, dimension: &str, pos: [i32; 3], face: Option<horizon_core::geometry::Direction>, chemical: &str, amount: i64, simulate: bool) -> Option<i64> {
    let stack = offered(game, dimension, chemical, amount)?;
    with_own_handler(game, pos, face.map(super::mek_world::mek_direction), |h| h.extract_stack(&stack, simulate).amount().clamp(0, amount))
}

/// A stack of `amount` of the registered `chemical`, on the level the game holds.
fn offered(game: &Game<'_>, dimension: &str, chemical: &str, amount: i64) -> Option<ChemicalStack> {
    if dimension != game.native_dimension() || amount <= 0 || registry(game).get(chemical).is_none() {
        return None;
    }
    Some(ChemicalStack::new(chemical, amount))
}

/// `ChemicalUtil.emit` (`EmitUtils.sendToAcceptors`): `offered` split evenly
/// among the chemical handlers on `faces` of `pos` (each through its touching
/// side) that take some of it when offered all; what they took, for the
/// caller to take out of its tank.
pub(super) fn emit(game: &mut Game<'_>, pos: [i32; 3], faces: &[Direction], offered: &ChemicalStack) -> i64 {
    let targets: Vec<([i32; 3], Direction)> = faces.iter().map(|&f| (super::mek_world::beside(pos, f), f.opposite())).collect();
    emit_to(game, &targets, offered)
}

/// `ChemicalUtil.emit` over targets of several blocks (a multiblock's output valves): each the position of a
/// chemical handler and the side it is touched through.
pub(super) fn emit_to(game: &mut Game<'_>, targets: &[([i32; 3], Direction)], offered: &ChemicalStack) -> i64 {
    if offered.is_empty() || targets.is_empty() {
        return 0;
    }
    let takers: Vec<([i32; 3], Direction)> = targets
        .iter()
        .copied()
        .filter(|&(at, side)| with_handler(game, at, Some(side), |h| h.insert(offered, true).amount() < offered.amount()) == Some(true))
        .collect();
    if takers.is_empty() {
        return 0;
    }
    let sent = mekanism_energy::split::send_to_acceptors(takers.len(), offered.amount().max(0) as u64, |t, amount, simulate| {
        let (at, side) = takers[t];
        let stack = offered.copy_with_amount(amount.min(i64::MAX as u64) as i64);
        with_handler(game, at, Some(side), |h| h.insert(&stack, simulate).amount().max(0) as u64).unwrap_or(amount)
    });
    sent.min(i64::MAX as u64) as i64
}

// ---- /mek chemical, /mek chemicals ------------------------------------------------------

fn stack_json(stack: &ChemicalStack) -> Value {
    match stack.id() {
        Some(id) => json!({ "id": id, "amount": stack.amount() }),
        None => json!({ "id": null, "amount": 0 }),
    }
}

/// `/mek chemical x y z <face|none> tanks | insert <chemical> <amount> [simulate] |
/// extract [<chemical>] <amount> [simulate]`: the handler of that side as
/// another block meets it.
pub(super) fn command(game: &mut Game<'_>, pos: [i32; 3], words: &[&str]) -> Value {
    let usage = json!({ "error": "usage: /mek chemical <x> <y> <z> <face|none> tanks|insert <chemical> <amount> [simulate]|extract [<chemical>] <amount> [simulate]" });
    let Some((&face, rest)) = words.split_first() else {
        return usage;
    };
    let face = match face {
        "none" | "null" => None,
        f => match Direction::from_name(f) {
            Some(d) => Some(d),
            None => return usage,
        },
    };
    let simulate = rest.last() == Some(&"simulate");
    let rest: Vec<&str> = rest.iter().copied().filter(|w| *w != "simulate").collect();
    let registry = registry(game);
    let full = |id: &str| mekanism_energy::chemical::full_id(id);
    let answer = match rest.as_slice() {
        ["tanks"] => with_handler(game, pos, face, |h| {
            let tanks: Vec<Value> = (0..h.tanks())
                .map(|i| {
                    let mut t = stack_json(&h.in_tank(i));
                    t["capacity"] = h.capacity(i).into();
                    t
                })
                .collect();
            json!({ "tanks": tanks })
        }),
        ["insert", chemical, amount] => {
            let Ok(amount) = amount.parse::<i64>() else {
                return usage;
            };
            let id = full(chemical);
            if registry.get(&id).is_none() {
                return json!({ "error": format!("no chemical {id}") });
            }
            let stack = ChemicalStack::new(&id, amount);
            with_handler(game, pos, face, |h| json!({ "rest": stack_json(&h.insert(&stack, simulate)) }))
        }
        ["extract", amount] => {
            let Ok(amount) = amount.parse::<i64>() else {
                return usage;
            };
            with_handler(game, pos, face, |h| json!({ "got": stack_json(&h.extract(amount, simulate)) }))
        }
        ["extract", chemical, amount] => {
            let Ok(amount) = amount.parse::<i64>() else {
                return usage;
            };
            let stack = ChemicalStack::new(&full(chemical), amount);
            with_handler(game, pos, face, |h| json!({ "got": stack_json(&h.extract_stack(&stack, simulate)) }))
        }
        _ => return usage,
    };
    let mut answer = answer.unwrap_or_else(|| json!({ "handler": false }));
    answer["pos"] = json!(pos);
    answer["face"] = json!(face.map(Direction::name));
    answer
}

/// `/mek chemicals [<id>]`: how many chemicals are registered, by kind and
/// namespace, and one chemical's facts.
pub(super) fn registry_command(game: &Game<'_>, id: Option<&str>) -> Value {
    let registry = registry(game);
    let mut namespaces = std::collections::BTreeMap::<String, usize>::new();
    let mut kinds = std::collections::BTreeMap::<&str, usize>::new();
    for c in registry.all() {
        *namespaces.entry(c.id.split(':').next().unwrap_or_default().to_owned()).or_default() += 1;
        *kinds.entry(c.kind.name()).or_default() += 1;
    }
    let chemical = id.map(mekanism_energy::chemical::full_id).map(|id| match registry.get(&id) {
        Some(c) => json!({
            "id": c.id, "number": registry.number(Some(&c.id)), "texture": c.texture, "tint": c.tint, "color": c.color,
            "kind": c.kind.name(), "gaseous": c.gaseous, "radioactivity": c.radioactivity, "validated": c.validated,
            "solid_tag": c.solid_tag, "fuel": c.fuel.map(|f| json!({ "burn_ticks": f.burn_ticks, "energy_per_tick": f.energy_per_tick, "density": f.energy_density() })),
            "cooled": c.cooled.as_ref().map(|x| x.other.clone()), "heated": c.heated.as_ref().map(|x| x.other.clone()),
            "tags": c.tags, "name": name(c), "sprite": gui_sprite(c),
        }),
        None => json!({ "id": id, "error": "not registered" }),
    });
    json!({ "count": registry.len(), "namespaces": namespaces, "kinds": kinds, "chemical": chemical })
}
