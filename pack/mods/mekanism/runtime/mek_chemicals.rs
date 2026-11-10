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
// Mekanistics will need it as a hook point (docs/mekanism-chemicals.md).

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
/// side (another block, a side its config closes to every tank).
pub(super) fn with_handler<R>(game: &mut Game<'_>, pos: [i32; 3], face: Option<Direction>, use_it: impl FnOnce(&mut dyn ChemicalHandler) -> R) -> Option<R> {
    let dimension = game.native_dimension();
    if super::mek_tubes::is_tube(dimension, pos) {
        return super::mek_tubes::with_handler(dimension, pos, face, use_it);
    }
    let registry = registry(game);
    if game.native_block(pos) == super::mek_dynamic_tank::VALVE {
        return super::mek_dynamic_tank::chemical_handler(dimension, pos, super::mek_dynamic_tank::VALVE, face, &registry, use_it);
    }
    game.native_with_mod_block_entity(pos, |m| {
        if super::mek_tanks::tier_of(&m.id).is_some() {
            super::mek_tanks::chemical_handler(m, face, &registry, use_it)
        } else if super::mek_gas::is_gas(&m.id) {
            super::mek_gas::chemical_handler(m, face, &registry, use_it)
        } else if super::mek_factories::has_chemicals(&m.id) {
            super::mek_factories::chemical_handler(m, face, &registry, use_it)
        } else {
            mek_machines::chemical_handler(m, face, &registry, use_it)
        }
    })
    .flatten()
}

/// `ChemicalUtil.emit` (`EmitUtils.sendToAcceptors`): `offered` split evenly
/// among the chemical handlers on `faces` of `pos` (each through its touching
/// side) that take some of it when offered all; what they took, for the
/// caller to take out of its tank.
pub(super) fn emit(game: &mut Game<'_>, pos: [i32; 3], faces: &[Direction], offered: &ChemicalStack) -> i64 {
    if offered.is_empty() || faces.is_empty() {
        return 0;
    }
    let beside = |f: Direction| super::mek_world::beside(pos, f);
    let targets: Vec<Direction> = faces
        .iter()
        .copied()
        .filter(|&f| with_handler(game, beside(f), Some(f.opposite()), |h| h.insert(offered, true).amount() < offered.amount()) == Some(true))
        .collect();
    if targets.is_empty() {
        return 0;
    }
    let sent = mekanism_energy::split::send_to_acceptors(targets.len(), offered.amount().max(0) as u64, |t, amount, simulate| {
        let face = targets[t];
        let stack = offered.copy_with_amount(amount.min(i64::MAX as u64) as i64);
        with_handler(game, beside(face), Some(face.opposite()), |h| h.insert(&stack, simulate).amount().max(0) as u64).unwrap_or(amount)
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
