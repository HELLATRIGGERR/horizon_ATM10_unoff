// Modern Industrialization's machines in the game (support module `crate::game::mi_world` of the
// `modern_industrialization` native bundle). The rules are the bundle's crate `mi_core`, translated from
// MI 2.5.11's source; this module is the glue: the block entity and its saved compound (MI's own names:
// `storedEu`, `usedEnergy`, `activeRecipe`...), the item slots in the engine's container, the fluid
// slots in the compound, the world's MI recipes, Forge Energy at ATM10's `forgeEnergyPerEu`, the menu's
// data (the screen is `mi_ui`) and `/mi probe` for tests. The hooks hand the API's block entity
// (`mod_api::ModBlockEntity`, slots of `mod_api::ItemStack`): its slots are read and written as the engine's stacks.
// Modern Industrialization is Copyright (c) 2020 Azercoco & Technici4n, MIT; see this bundle's README.md.

use super::Game;
use crate::game::api_values::{TryIntoApi, TryIntoEngine};
use horizon_client::{containers::ModContainer, items::ItemStack, text::Text};
use horizon_core::nbt::{Compound, JavaString, Tag};
use horizon_world::fluid::FluidKind;
use mod_api::{Command, CommandArgumentValue, CommandInvocation, CommandSource, ModBlockEntity, NativeFeInfo, WorldPhase};
use mi_core::{
    crafter::{Behavior, Crafter, put_fluid_outputs, put_item_outputs, take_fluid_inputs, take_item_inputs},
    energy::{self, EnergyStore},
    inventory::{FluidSlot, Inventory, ItemSlot},
    machines::{self, Machine},
    recipe::{self, RecipeBook},
    view::{FluidView, MachineView, PROGRESS_SCALE},
};
use serde_json::Value;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// Slots of the hopper menu every ported machine uses; those past the machine's are hidden.
const MENU_SLOTS: usize = 5;

thread_local! {
    /// `forgeEnergyPerEu` of the open world.
    static FE_PER_EU: RefCell<Option<i64>> = const { RefCell::new(None) };
    /// Each recipe type's book, with the world table it was read from.
    static BOOKS: RefCell<HashMap<&'static str, ((usize, usize), Rc<RecipeBook>)>> = RefCell::new(HashMap::new());
    static RNG: RefCell<u64> = RefCell::new(seed());
}

fn seed() -> u64 {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
    t | 1
}

/// `ThreadLocalRandom.nextFloat`'s stand-in: xorshift64*, 24 bits.
fn next_float() -> f32 {
    RNG.with(|r| {
        let mut x = *r.borrow();
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        *r.borrow_mut() = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32 / (1u64 << 24) as f32
    })
}

fn read_fe_per_eu() -> i64 {
    let text = Game::native_config_dir().ok().and_then(|d| std::fs::read_to_string(d.join("modern_industrialization-server.toml")).ok());
    energy::fe_per_eu(text.as_deref())
}

/// ATM10's `forgeEnergyPerEu` (16), read once a world.
pub(super) fn fe_per_eu() -> i64 {
    FE_PER_EU.with(|f| *f.borrow_mut().get_or_insert_with(read_fe_per_eu))
}

/// `native_server_started`: settings are read and the recipe books start again as the server opens its world.
pub(super) fn opened() {
    let fresh = read_fe_per_eu();
    FE_PER_EU.with(|f| *f.borrow_mut() = Some(fresh));
    BOOKS.with(|b| b.borrow_mut().clear());
}

/// `native_world_lifecycle`: settings and recipe books are dropped as the world closes or fails to open.
pub(super) fn lifecycle(phase: WorldPhase) {
    if matches!(phase, WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed | WorldPhase::WorldOpenFailed) {
        FE_PER_EU.with(|f| *f.borrow_mut() = None);
        BOOKS.with(|b| b.borrow_mut().clear());
    }
}

// ---- Recipes ----------------------------------------------------------------

/// An item ingredient's items, as Mekanism's bundle reads NeoForge's ingredient forms.
fn ingredient_items(game: &Game<'_>, v: &Value) -> Option<Vec<String>> {
    let one = |s: String| game.native_recipe_ingredient(&Value::String(s)).map(|i| i.order);
    match v {
        Value::String(s) => one(s.clone()),
        Value::Array(list) => {
            let mut all: Vec<String> = Vec::new();
            for item in list.iter().filter_map(|c| ingredient_items(game, c)).flatten() {
                if !all.contains(&item) {
                    all.push(item);
                }
            }
            (!all.is_empty()).then_some(all)
        }
        Value::Object(o) => {
            if let Some(item) = o.get("item").and_then(Value::as_str) {
                return one(item.to_owned());
            }
            if let Some(tag) = o.get("tag").and_then(Value::as_str) {
                return one(format!("#{tag}"));
            }
            let kind = o.get("type").or_else(|| o.get("neoforge:ingredient_type")).and_then(Value::as_str)?;
            let children = o.get("children").or_else(|| o.get("ingredients")).and_then(Value::as_array)?;
            match kind {
                "neoforge:compound" => ingredient_items(game, &Value::Array(children.clone())),
                _ => None,
            }
        }
        _ => None,
    }
}

/// A `FluidIngredient`'s fluids: `{"fluid": id}`, or a tag the game can't list yet.
fn ingredient_fluids(v: &Value) -> Option<Vec<String>> {
    let id = v.get("fluid").and_then(Value::as_str)?;
    FluidKind::from_name(id).is_some().then(|| vec![id.to_owned()])
}

/// The recipe book of `machine`'s type, read again when the world's table changes (a `/reload`).
pub(super) fn book(game: &Game<'_>, machine: &Machine) -> Rc<RecipeBook> {
    let table = game.native_recipes_of_type(machine.recipe_type);
    let stamp = (table.as_ptr() as usize, table.len());
    if let Some(book) = BOOKS.with(|b| b.borrow().get(machine.recipe_type).filter(|(s, _)| *s == stamp).map(|(_, b)| b.clone())) {
        return book;
    }
    let mut refused = 0;
    let recipes: Vec<_> = table
        .iter()
        .filter_map(|r| match recipe::parse(&r.id, &r.json, machine.rules, |v| ingredient_items(game, v), ingredient_fluids) {
            Ok(recipe) => Some(recipe),
            Err(e) => {
                refused += 1;
                log::debug!("[mi] {} left out: {e}", r.id);
                None
            }
        })
        .collect();
    log::info!("[mi] {}: {} recipes, {refused} left out", machine.recipe_type, recipes.len());
    let book = Rc::new(RecipeBook::new(recipes));
    BOOKS.with(|b| b.borrow_mut().insert(machine.recipe_type, (stamp, book.clone())));
    book
}

// ---- The saved compound, in MI's names -------------------------------------

fn key(name: &str) -> JavaString {
    JavaString::from(name)
}

fn number(tag: Option<&Tag>) -> i64 {
    match tag {
        Some(Tag::Byte(v)) => i64::from(*v),
        Some(Tag::Short(v)) => i64::from(*v),
        Some(Tag::Int(v)) => i64::from(*v),
        Some(Tag::Long(v)) => *v,
        Some(Tag::Float(bits)) => f32::from_bits(*bits) as i64,
        Some(Tag::Double(bits)) => f64::from_bits(*bits) as i64,
        _ => 0,
    }
}

fn string(tag: Option<&Tag>) -> Option<String> {
    match tag {
        Some(Tag::String(s)) => Some(s.to_string_lossy()).filter(|s| !s.is_empty()),
        _ => None,
    }
}

const STORED_EU: &str = "storedEu";
const FLUIDS: &str = "inventory_fluids";
const LOCKS: &str = "inventory_locks";

/// The crafter's fields (`CrafterComponent.readNbt`).
fn read_crafter(nbt: &Compound) -> Crafter {
    let get = |name: &str| number(nbt.get(&key(name)));
    Crafter {
        used_energy: get("usedEnergy").max(0),
        recipe_energy: get("recipeEnergy").max(0),
        recipe_max_eu: get("recipeMaxEu").max(0),
        active_recipe: string(nbt.get(&key("activeRecipe"))),
        efficiency_ticks: get("efficiencyTicks").clamp(0, i64::from(i32::MAX)) as i32,
        max_efficiency_ticks: get("maxEfficiencyTicks").clamp(0, i64::from(i32::MAX)) as i32,
        ..Crafter::new()
    }
    .loaded()
}

fn write_crafter(nbt: &mut Compound, c: &Crafter) {
    nbt.insert(key("usedEnergy"), Tag::Long(c.used_energy));
    nbt.insert(key("recipeEnergy"), Tag::Long(c.recipe_energy));
    nbt.insert(key("recipeMaxEu"), Tag::Long(c.recipe_max_eu));
    match &c.active_recipe {
        Some(id) => nbt.insert(key("activeRecipe"), Tag::String(JavaString::from(id.as_str()))),
        None => nbt.remove(&key("activeRecipe")),
    };
    nbt.insert(key("efficiencyTicks"), Tag::Int(c.efficiency_ticks));
    nbt.insert(key("maxEfficiencyTicks"), Tag::Int(c.max_efficiency_ticks));
}

fn compound_list(nbt: &Compound, name: &str) -> Vec<Compound> {
    match nbt.get(&key(name)) {
        Some(Tag::List(list)) => list.iter().filter_map(|t| if let Tag::Compound(c) = t { Some(c.clone()) } else { None }).collect(),
        _ => Vec::new(),
    }
}

/// The fluid slots (inputs, then outputs), each `{fluid, amount, locked, machineLocked}`.
fn read_fluids(nbt: &Compound, machine: &Machine) -> Vec<FluidSlot> {
    let saved = compound_list(nbt, FLUIDS);
    (0..machine.fluid_slots())
        .map(|i| {
            let mut slot = FluidSlot::standard(machine.fluid_capacity());
            if let Some(c) = saved.get(i) {
                slot.fluid = string(c.get(&key("fluid"))).filter(|f| FluidKind::from_name(f).is_some());
                slot.amount = if slot.fluid.is_some() { number(c.get(&key("amount"))).clamp(0, slot.capacity) } else { 0 };
                if slot.amount == 0 {
                    slot.fluid = None;
                }
                slot.locked = string(c.get(&key("locked")));
                slot.machine_locked = number(c.get(&key("machineLocked"))) != 0 && slot.locked.is_some();
            }
            slot
        })
        .collect()
}

fn write_fluids(nbt: &mut Compound, slots: &[FluidSlot]) {
    let list = slots
        .iter()
        .map(|s| {
            let mut c = Compound::new();
            c.insert(key("fluid"), Tag::String(JavaString::from(s.fluid.as_deref().unwrap_or(""))));
            c.insert(key("amount"), Tag::Long(s.amount));
            c.insert(key("capacity"), Tag::Long(s.capacity));
            if let Some(l) = &s.locked {
                c.insert(key("locked"), Tag::String(JavaString::from(l.as_str())));
            }
            c.insert(key("machineLocked"), Tag::Byte(i8::from(s.machine_locked)));
            Tag::Compound(c)
        })
        .collect();
    nbt.insert(key(FLUIDS), Tag::List(list));
}

/// The item output slots' machine locks: `{slot, item}` for each reserved slot.
fn read_locks(nbt: &Compound) -> Vec<(usize, String)> {
    compound_list(nbt, LOCKS)
        .iter()
        .filter_map(|c| Some((number(c.get(&key("slot"))).clamp(0, 63) as usize, string(c.get(&key("item")))?)))
        .collect()
}

fn write_locks(nbt: &mut Compound, inv: &Inventory, first_output: usize) {
    let list: Vec<Tag> = inv
        .item_outputs
        .iter()
        .enumerate()
        .filter(|(_, s)| s.machine_locked)
        .filter_map(|(i, s)| {
            let mut c = Compound::new();
            c.insert(key("slot"), Tag::Int((first_output + i) as i32));
            c.insert(key("item"), Tag::String(JavaString::from(s.locked.as_deref()?)));
            Some(Tag::Compound(c))
        })
        .collect();
    if list.is_empty() {
        nbt.remove(&key(LOCKS));
    } else {
        nbt.insert(key(LOCKS), Tag::List(list));
    }
}

fn energy_of(nbt: &Compound, machine: &Machine) -> EnergyStore {
    EnergyStore::new(number(nbt.get(&key(STORED_EU))), machine.tier.eu_capacity)
}

// ---- Block entities -------------------------------------------------------

/// `block_entity_types`: the electric machines, one type per block and named after it.
pub(super) fn block_entity_types(types: &mut Vec<(String, Vec<String>)>) {
    for machine in machines::ALL {
        let block = format!("modern_industrialization:{}", machine.id);
        types.push((block.clone(), vec![block]));
    }
}

/// `native_block_entity`: a new, empty machine (`MachineBlockEntity`); the declaration owns its type id.
pub(super) fn new_entity(block: &str, entity: &mut Option<Compound>) {
    let Some(machine) = machines::of_block(block) else {
        return;
    };
    if entity.is_some() {
        return;
    }
    let mut nbt = Compound::new();
    nbt.insert(key(STORED_EU), Tag::Long(0));
    write_crafter(&mut nbt, &Crafter::new());
    write_fluids(&mut nbt, &read_fluids(&Compound::new(), machine));
    *entity = Some(nbt);
}

/// `native_block_entity_ticks`.
pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if machines::of_block(block).is_some() {
        *ticks = true;
    }
}

/// `native_block_entity_container`: the machine's item slots (inputs, then outputs) in a hopper's
/// five-slot menu, the rest hidden. Pipes and hoppers put into inputs (`pipesInsert`) and take from
/// outputs (`pipesExtract`), through every face.
pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    let Some(machine) = machines::of_block(block) else {
        return;
    };
    if container.is_some() {
        return;
    }
    let inputs: Vec<usize> = (0..machine.item_inputs).collect();
    let outputs: Vec<usize> = (machine.item_inputs..machine.item_slots()).collect();
    let faces: Vec<usize> = (0..machine.item_slots()).collect();
    *container = Some(ModContainer {
        slots: MENU_SLOTS,
        faces: std::array::from_fn(|_| faces.clone()),
        inputs,
        outputs,
        menu: Some("minecraft:hopper".into()),
    });
}

/// `native_block_entity_slot_accepts`: inputs take any item (`standardInputSlot`); outputs and the
/// hidden slots take none (`playerInsert` and `pipesInsert` are off).
pub(super) fn slot_accepts(block: &str, slot: usize, _fuel: bool, accepts: &mut bool) {
    if let Some(machine) = machines::of_block(block)
        && slot >= machine.item_inputs
    {
        *accepts = false;
    }
}

/// The machine for the crafter: an LV tier, its EU, and the game for stack sizes.
struct Electric<'a, 'g> {
    game: &'a Game<'g>,
    energy: EnergyStore,
    machine: &'static Machine,
}

impl Behavior for Electric<'_, '_> {
    fn consume_eu(&mut self, max: i64, act: bool) -> i64 {
        self.energy.consume(max, act)
    }
    fn base_recipe_eu(&self) -> i64 {
        self.machine.tier.base_eu
    }
    fn max_recipe_eu(&self) -> i64 {
        self.machine.tier.max_eu
    }
    fn max_stack(&self, item: &str) -> u32 {
        self.game.native_max_stack_size(item).unwrap_or(64)
    }
    fn next_float(&mut self) -> f32 {
        next_float()
    }
}

/// The engine's item slots as MI's: each slot's item, count and stack size, with the reserved outputs.
fn inventory(game: &Game<'_>, items: &[Option<ItemStack>], nbt: &Compound, machine: &Machine) -> Inventory {
    let slot = |i: usize| {
        let stack = items.get(i).and_then(Option::as_ref);
        let item = stack.map(|s| &*s.item);
        ItemSlot::new(item, stack.map_or(0, |s| s.count), item.and_then(|i| game.native_max_stack_size(i)).unwrap_or(64))
    };
    let mut inv = Inventory {
        item_inputs: (0..machine.item_inputs).map(slot).collect(),
        item_outputs: (machine.item_inputs..machine.item_slots()).map(slot).collect(),
        fluid_inputs: Vec::new(),
        fluid_outputs: Vec::new(),
    };
    for (i, item) in read_locks(nbt) {
        if let Some(s) = i.checked_sub(machine.item_inputs).and_then(|o| inv.item_outputs.get_mut(o)) {
            s.enable_machine_lock(&item);
        }
    }
    let mut fluids = read_fluids(nbt, machine);
    inv.fluid_outputs = fluids.split_off(machine.fluid_inputs);
    inv.fluid_inputs = fluids;
    inv
}

/// The block entity's slots as the engine's stacks (a slot the engine can't read is empty).
fn engine_items(game: &Game<'_>, entity: &ModBlockEntity) -> Vec<Option<ItemStack>> {
    entity.items.iter().map(|s| s.as_ref().and_then(|s| s.try_into_engine(game.items()).ok())).collect()
}

/// Writes MI's slots back into the block entity's (`items`: them as the engine's stacks before the tick); a stack
/// that only changed count keeps its components.
fn write_items(entity: &mut ModBlockEntity, items: &[Option<ItemStack>], inv: &Inventory) {
    let slots: Vec<&ItemSlot> = inv.item_inputs.iter().chain(&inv.item_outputs).collect();
    for (i, s) in slots.into_iter().enumerate() {
        let old = items.get(i).and_then(Option::as_ref);
        let same = match (old, &s.item) {
            (None, None) => true,
            (Some(o), Some(item)) => &*o.item == item && o.count == s.count,
            _ => false,
        };
        if same {
            continue;
        }
        let new = match (&s.item, old) {
            (None, _) => None,
            (Some(item), Some(o)) if &*o.item == item => Some(ItemStack { count: s.count, ..o.clone() }),
            (Some(item), _) => Some(ItemStack::new(item, s.count)),
        };
        if entity.items.len() <= i {
            entity.items.resize(i + 1, None);
        }
        entity.items[i] = new.and_then(|stack| stack.try_into_api().ok());
    }
}

/// A fluid's number in the menu's data: its `tick_id` (vanilla's registry id, then the mods' fluids), which the
/// screen reads back with `from_tick_id`; -1 for none.
fn fluid_id(name: Option<&str>) -> i64 {
    name.and_then(FluidKind::from_name).map_or(-1, |f| i64::from(f.tick_id()))
}

/// `native_block_entity_tick`: `AbstractCraftingMachineBlockEntity.tick` without auto-extract.
pub(super) fn tick(game: &mut Game<'_>, _pos: [i32; 3], entity: &mut ModBlockEntity) {
    let Some(machine) = machines::of_block(entity.id()) else {
        return;
    };
    let book = book(game, machine);
    let items = engine_items(game, entity);
    let mut inv = inventory(game, &items, &entity.nbt, machine);
    let before = inv.clone();
    let mut crafter = read_crafter(&entity.nbt);
    let crafter_before = crafter.clone();
    let mut electric = Electric { game, energy: energy_of(&entity.nbt, machine), machine };
    let eu_before = electric.energy.eu;
    crafter.tick(&book, &mut inv, &mut electric);
    let energy = electric.energy;
    if inv != before {
        write_items(entity, &items, &inv);
        let fluids: Vec<FluidSlot> = inv.fluid_inputs.iter().chain(&inv.fluid_outputs).cloned().collect();
        write_fluids(&mut entity.nbt, &fluids);
        write_locks(&mut entity.nbt, &inv, machine.item_inputs);
    }
    if crafter != crafter_before {
        write_crafter(&mut entity.nbt, &crafter);
    }
    if energy.eu != eu_before {
        entity.nbt.insert(key(STORED_EU), Tag::Long(energy.eu));
    }
    let base = crafter.active_recipe.as_deref().and_then(|id| book.get(id)).map_or(0, |r| i64::from(r.eu));
    let view = MachineView {
        eu: energy.eu,
        eu_capacity: energy.capacity,
        progress: (f64::from(crafter.progress()) * PROGRESS_SCALE as f64) as i64,
        efficiency_ticks: i64::from(crafter.efficiency_ticks),
        max_efficiency_ticks: i64::from(crafter.max_efficiency_ticks),
        has_active_recipe: crafter.has_active_recipe(),
        current_recipe_eu: crafter.recipe_max_eu,
        base_recipe_eu: base,
        max_recipe_eu: machine.tier.max_eu,
        matches_multiple_recipes: crafter.matches_multiple_recipes,
        machine_locked: inv.item_outputs.iter().enumerate().filter(|(_, s)| s.machine_locked).fold(0, |m, (i, _)| m | 1 << i),
        fluids: inv
            .fluid_inputs
            .iter()
            .chain(&inv.fluid_outputs)
            .map(|s| FluidView { fluid: fluid_id(s.fluid.as_deref()), amount: s.amount, capacity: s.capacity, locked: fluid_id(s.locked.as_deref()) })
            .collect(),
    };
    let mut data = Vec::new();
    view.pack_into(&mut data);
    if entity.data != data {
        entity.data = data;
    }
}

// ---- Forge Energy ---------------------------------------------------------

/// `native_fe_storage`: every side takes FE and gives none (`buildInsertable` wrapped as
/// `IEnergyStorage`), at `forgeEnergyPerEu` FE to the EU.
pub(super) fn fe_storage(entity: &ModBlockEntity, info: &mut Option<NativeFeInfo>) {
    let Some(machine) = machines::of_block(entity.id()) else {
        return;
    };
    if info.is_some() {
        return;
    }
    let (stored, capacity) = energy_of(&entity.nbt, machine).fe(fe_per_eu());
    *info = Some(NativeFeInfo { stored, capacity, can_receive: true, can_extract: false });
}

/// `native_fe_receive`: `WrappedMIStorage.receive`, whole EU only.
pub(super) fn fe_receive(entity: &mut ModBlockEntity, max: i64, simulate: bool, accepted: &mut Option<i64>) {
    let Some(machine) = machines::of_block(entity.id()) else {
        return;
    };
    if accepted.is_some() {
        return;
    }
    let mut energy = energy_of(&entity.nbt, machine);
    let taken = energy.receive_fe(max, fe_per_eu(), !simulate);
    if taken > 0 && !simulate {
        entity.nbt.insert(key(STORED_EU), Tag::Long(energy.eu));
    }
    *accepted = Some(taken);
}

/// `native_fe_extract`: a machine gives no energy.
pub(super) fn fe_extract(entity: &ModBlockEntity, given: &mut Option<i64>) {
    if given.is_none() && machines::of_block(entity.id()).is_some() {
        *given = Some(0);
    }
}

// ---- /mi --------------------------------------------------------------------

/// `native_register_commands`: `/mi` with one `text` line (upstream's `word` takes no `~`), operators only.
pub(super) fn commands(commands: &mut Vec<Command>) {
    commands.push(Command::op("mi [line:text]", 2));
}

/// The command line a parsed invocation of `/mi` was typed as: its words, then the `text`.
fn line(call: &CommandInvocation) -> String {
    let mut line = call.literals.join(" ");
    for argument in &call.arguments {
        if let CommandArgumentValue::String(text) = &argument.value {
            line.push(' ');
            line.push_str(text);
        }
    }
    line
}

fn coordinate(word: Option<&str>, from: f64) -> Option<i32> {
    let word = word?;
    match word.strip_prefix('~') {
        Some("") => Some(from.floor() as i32),
        Some(rest) => Some((from + rest.parse::<f64>().ok()?).floor() as i32),
        None => word.parse().ok(),
    }
}

/// `native_run_mod_command`: `/mi probe x y z` answers `MI {json}` with the machine's state;
/// `/mi offer_fe x y z n` offers `n` FE through the engine's Forge Energy path and answers what was taken.
pub(super) fn perform(game: &mut Game<'_>, source: &CommandSource, call: &CommandInvocation, ran: &mut bool) {
    let CommandSource::Player { player, .. } = source else { return };
    let position = game.entity_position(player.entity()).unwrap_or_default();
    let command = line(call);
    let mut words = command.split_whitespace();
    if *ran || words.next() != Some("mi") {
        return;
    }
    *ran = true;
    let verb = words.next();
    let p = position;
    let pos = match (coordinate(words.next(), p[0]), coordinate(words.next(), p[1]), coordinate(words.next(), p[2])) {
        (Some(x), Some(y), Some(z)) => Some([x, y, z]),
        _ => None,
    };
    let answer = match (verb, pos) {
        (Some("probe"), Some(pos)) => probe(game, pos),
        (Some("offer_fe"), Some(pos)) => match words.next().and_then(|n| n.parse::<i64>().ok()) {
            Some(amount) => {
                let dimension = game.native_dimension();
                let taken = game.native_fe_offer(dimension, pos, None, amount, false, false);
                serde_json::json!({ "pos": pos, "offered": amount, "taken": taken })
            }
            None => serde_json::json!({ "error": "usage: /mi offer_fe <x> <y> <z> <amount>" }),
        },
        _ => serde_json::json!({ "error": "usage: /mi probe <x> <y> <z>" }),
    };
    game.native_feedback(Text::literal(format!("MI {answer}")));
}

fn probe(game: &mut Game<'_>, pos: [i32; 3]) -> Value {
    let found = game.native_with_mod_block_entity(pos, |m| (m.id().to_owned(), m.nbt.clone(), m.items.clone(), m.data.clone()));
    let Some((id, nbt, items, data)) = found else {
        return serde_json::json!({ "pos": pos, "error": "no mod block entity here" });
    };
    let Some(machine) = machines::of_block(&id) else {
        return serde_json::json!({ "pos": pos, "id": id, "error": "not a ported MI machine" });
    };
    let c = read_crafter(&nbt);
    let energy = energy_of(&nbt, machine);
    let ratio = fe_per_eu();
    let slots: Vec<Value> = (0..machine.item_slots())
        .map(|i| match items.get(i).and_then(Option::as_ref) {
            Some(s) => serde_json::json!({ "item": &*s.item, "count": s.count }),
            None => Value::Null,
        })
        .collect();
    let fluids: Vec<Value> = read_fluids(&nbt, machine)
        .iter()
        .map(|f| serde_json::json!({ "fluid": f.fluid, "amount": f.amount, "capacity": f.capacity }))
        .collect();
    let book = book(game, machine);
    let recipes = book.recipes.len();
    // Why a recipe does or doesn't start: each candidate for the input items, and whether its item inputs,
    // fluid inputs and outputs fit (`canStartRecipe`).
    let mut inv = inventory(game, &items, &nbt, machine);
    let mut electric = Electric { game, energy: energy_of(&nbt, machine), machine };
    let held: Vec<String> = inv.item_inputs.iter().filter_map(|s| s.item.clone()).collect();
    let candidates: Vec<Value> = book
        .candidates(held.iter().map(String::as_str))
        .iter()
        .map(|r| {
            let (i, f) = (take_item_inputs(r, &mut inv, true, &mut electric), take_fluid_inputs(r, &mut inv, true, &mut electric));
            let (o, fo) = (put_item_outputs(r, &mut inv, true, false, &mut electric), put_fluid_outputs(r, &mut inv, true, false, &mut electric));
            serde_json::json!({ "id": r.id, "items": i, "fluids": f, "outputs": o, "fluid_outputs": fo })
        })
        .collect();
    serde_json::json!({
        "pos": pos,
        "id": id,
        "recipe_type": machine.recipe_type,
        "recipes": recipes,
        "candidates": candidates,
        "eu": energy.eu,
        "eu_capacity": energy.capacity,
        "fe": energy.fe(ratio).0,
        "fe_capacity": energy.fe(ratio).1,
        "fe_per_eu": ratio,
        "base_eu": machine.tier.base_eu,
        "max_eu": machine.tier.max_eu,
        "used_energy": c.used_energy,
        "recipe_energy": c.recipe_energy,
        "recipe_max_eu": c.recipe_max_eu,
        "matches_multiple_recipes": MachineView::unpack(&data).matches_multiple_recipes,
        "active_recipe": c.active_recipe,
        "efficiency_ticks": c.efficiency_ticks,
        "max_efficiency_ticks": c.max_efficiency_ticks,
        "slots": slots,
        "fluids": fluids,
    })
}
