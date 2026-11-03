// Mekanism's first processing machines in the game (support module
// `crate::game::mek_machines` of the `mekanism` native bundle): the
// Enrichment Chamber, the Crusher, the Energized Smelter and the Metallurgic
// Infuser. Their rules are the bundle's crate (`mekanism_energy::machine`,
// MIT, from Mekanism 10.7.19, Copyright (c) 2017-2025 Aidan C. Brady); this
// module is the glue: their block entities and saved compound in Mekanism's
// names, their slots (input, output, energy, the upgrade component's two),
// the recipes they read from the world's tables (`native_recipes_of_type`
// for Mekanism's types, `native_cooking_recipes` of hook points API 159 for
// the smelter), each tick in `TileEntityMekanism.tickServer` order (upgrades
// in, the active state's delay, the item ejector, the energy slot, the
// infusion slot, the recipe), Joules from cables, cubes and Forge Energy on
// the faces their energy config opens, and `/mek probe`. A machine's cached
// recipe and its upgrade slot's progress are kept here by position, not
// saved, as Mekanism keeps them in its block entity. No borrow of that state
// is held across a call into the game.

use super::{
    Game,
    mek_world::{self, ENERGY, ITEM},
    native_hooks::NativeFeInfo,
};
use horizon_client::{
    containers::{ModBlockEntity, ModContainer},
    crafting::Cooking,
    items::ItemStack,
};
use horizon_core::geometry::Direction as Face;
use horizon_world::nbt::{Compound, Tag};
use mekanism_energy::{
    config::EnergyConfig,
    container::{Automation, EnergyContainer},
    fe::{self, Conversion, StrictEnergy},
    heat::Activity,
    machine::{self, Inputs, MachineKind, SideConfig, Tank, Upgrade, UpgradeSlot, Upgrades},
    menu::{self, MachineButton, MachineView},
    sides::{DataType, Direction},
};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::RwLock,
};

/// `TransmissionType.CHEMICAL`'s ordinal in `component_config`.
const CHEMICAL: u8 = 2;

// ---- Recipes -------------------------------------------------------------------

/// An item ingredient as Mekanism's `ItemStackIngredient` reads it: the items
/// it takes and how many.
#[derive(Clone, Debug)]
struct ItemIn {
    items: HashSet<String>,
    count: u32,
}

impl ItemIn {
    fn takes(&self, stack: &ItemStack) -> bool {
        self.items.contains(&*stack.item)
    }
}

/// A machine recipe: `ItemStackToItemStackRecipe`, or with a chemical
/// (`ItemStackChemicalToItemStackRecipe`, the chemicals it takes and how many mB).
#[derive(Clone, Debug)]
struct Recipe {
    id: String,
    input: ItemIn,
    chemical: Option<(Vec<String>, u64)>,
    output: ItemStack,
}

impl Recipe {
    fn chemical_fits(&self, tank: &Tank) -> bool {
        match (&self.chemical, &tank.chemical) {
            (None, _) => true,
            (Some((kinds, _)), Some(held)) => kinds.iter().any(|k| k == held),
            (Some(_), None) => false,
        }
    }
}

/// `ItemStackToChemicalRecipe` (`mekanism:chemical_conversion`) and
/// `ItemStackToEnergyRecipe` (`mekanism:energy_conversion`).
#[derive(Clone, Debug)]
struct ChemicalConversion {
    input: ItemIn,
    chemical: String,
    amount: u64,
}

#[derive(Clone, Debug)]
struct EnergyConversion {
    input: ItemIn,
    joules: u64,
}

/// The recipes of the world's tables, made again when one of them changes.
#[derive(Default)]
struct Book {
    /// Where each table was when this was made (its slice, by address and length).
    stamp: Vec<(usize, usize)>,
    recipes: HashMap<MachineKind, Vec<Rc<Recipe>>>,
    conversions: Vec<ChemicalConversion>,
    energy: Vec<EnergyConversion>,
}

const TYPES: [&str; 5] = [
    "mekanism:enriching",
    "mekanism:crushing",
    "mekanism:metallurgic_infusing",
    "mekanism:chemical_conversion",
    "mekanism:energy_conversion",
];

thread_local! {
    static BOOK: RefCell<Book> = RefCell::new(Book::default());
    /// Which book is current: made again, every machine looks its recipe up again
    /// (`CommonWorldTickHandler.flushTagAndRecipeCaches`).
    static EDITION: Cell<u64> = const { Cell::new(0) };
    /// Each machine's cached recipe, upgrade slot and ejector wait, by
    /// dimension and position (not saved).
    static LIVE: RefCell<HashMap<(String, [i32; 3]), Live>> = RefCell::new(HashMap::new());
}

/// What the slots of each kind take, for `native_block_entity_slot_accepts`,
/// which may be asked on any thread with no world at hand: made where the
/// recipes are read.
#[derive(Default)]
struct Takes {
    inputs: HashMap<MachineKind, HashSet<String>>,
    infusion: HashSet<String>,
    energy: HashSet<String>,
}

static TAKES: RwLock<Option<Takes>> = RwLock::new(None);

/// A machine's state that Mekanism keeps in its block entity but doesn't save.
#[derive(Clone, Debug, Default)]
struct Live {
    /// The cached recipe (`RecipeCacheLookupMonitor.cachedRecipe`) and the book it came from.
    recipe: Option<(u64, Rc<Recipe>)>,
    upgrade: UpgradeSlot,
    /// The upgrade slot as last looked at, to notice it changed.
    upgrade_stack: Option<(String, u32)>,
    /// `TileComponentEjector.tickDelay`.
    eject_delay: u32,
}

/// An ingredient's JSON as Mekanism's 1.21.1 recipes write it: `{item}` or
/// `{tag}` with a `count`, NeoForge's `compound` (any child) and
/// `intersection` (all children), or a list. Items the game doesn't know are
/// left out; a child that names none is skipped in a compound.
fn items_of(game: &Game<'_>, v: &Value) -> Option<HashSet<String>> {
    let one = |s: String| game.native_recipe_ingredient(&Value::String(s)).map(|i| i.0);
    match v {
        Value::String(s) => one(s.clone()),
        Value::Array(list) => {
            let all: HashSet<String> = list.iter().filter_map(|c| items_of(game, c)).flatten().collect();
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
                "neoforge:compound" => items_of(game, &Value::Array(children.clone())),
                "neoforge:intersection" => {
                    let sets: Vec<HashSet<String>> = children.iter().map(|c| items_of(game, c)).collect::<Option<_>>()?;
                    let (first, rest) = sets.split_first()?;
                    let common: HashSet<String> = first.iter().filter(|i| rest.iter().all(|s| s.contains(*i))).cloned().collect();
                    (!common.is_empty()).then_some(common)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn item_in(game: &Game<'_>, v: &Value) -> Option<ItemIn> {
    let count = v.get("count").and_then(Value::as_u64).unwrap_or(1).clamp(1, 99) as u32;
    Some(ItemIn { items: items_of(game, v)?, count })
}

/// A chemical ingredient (`ChemicalStackIngredient`): `{chemical}` or a chemical
/// `{tag}`, with an `amount`. Mekanism's infuse type tags each hold the
/// chemical of their own name (`data/mekanism/tags/mekanism/chemical/`), which
/// ATM10 leaves as they are, so a tag is read as that chemical.
fn chemical_in(v: &Value) -> Option<(Vec<String>, u64)> {
    let amount = v.get("amount").and_then(Value::as_u64).filter(|a| *a > 0)?;
    let id = v.get("chemical").or_else(|| v.get("tag")).and_then(Value::as_str)?;
    Some((vec![id.to_owned()], amount))
}

/// Makes the book again from the world's tables when one changed.
fn refresh_book(game: &Game<'_>) {
    let cooking = game.native_cooking_recipes();
    let mut stamp: Vec<(usize, usize)> = TYPES
        .iter()
        .map(|t| {
            let s = game.native_recipes_of_type(t);
            (s.as_ptr() as usize, s.len())
        })
        .collect();
    stamp.push((cooking.as_ptr() as usize, cooking.len()));
    if BOOK.with(|b| b.borrow().stamp == stamp) {
        return;
    }
    let mut book = Book { stamp, ..Book::default() };
    for kind in MachineKind::ALL {
        let list: Vec<Rc<Recipe>> = if kind == MachineKind::EnergizedSmelter {
            smelting(cooking)
        } else {
            game.native_recipes_of_type(kind.recipe_type())
                .iter()
                .filter_map(|r| {
                    let j = &*r.json;
                    let (input, chemical) = if kind.infuses() {
                        // Per-tick infusing (`per_tick_usage`) isn't ported; ATM10 has none.
                        if j.get("per_tick_usage").and_then(Value::as_bool) == Some(true) {
                            return None;
                        }
                        (item_in(game, &j["item_input"])?, Some(chemical_in(&j["chemical_input"])?))
                    } else {
                        (item_in(game, &j["input"])?, None)
                    };
                    let output = game.native_recipe_result(&j["output"])?;
                    Some(Rc::new(Recipe { id: r.id.clone(), input, chemical, output }))
                })
                .collect()
        };
        book.recipes.insert(kind, list);
    }
    book.conversions = game
        .native_recipes_of_type("mekanism:chemical_conversion")
        .iter()
        .filter_map(|r| {
            let j = &*r.json;
            let out = &j["output"];
            Some(ChemicalConversion {
                input: item_in(game, &j["input"])?,
                chemical: out.get("id").and_then(Value::as_str)?.to_owned(),
                amount: out.get("amount").and_then(Value::as_u64).filter(|a| *a > 0)?,
            })
        })
        .collect();
    book.energy = game
        .native_recipes_of_type("mekanism:energy_conversion")
        .iter()
        .filter_map(|r| {
            let j = &*r.json;
            Some(EnergyConversion { input: item_in(game, &j["input"])?, joules: j["output"].as_u64().filter(|o| *o > 0)? })
        })
        .collect();
    let infused: HashSet<&str> = book.recipes[&MachineKind::MetallurgicInfuser]
        .iter()
        .flat_map(|r| r.chemical.iter().flat_map(|(k, _)| k.iter().map(String::as_str)))
        .collect();
    let takes = Takes {
        inputs: book
            .recipes
            .iter()
            .map(|(kind, list)| (*kind, list.iter().flat_map(|r| r.input.items.iter().cloned()).collect()))
            .collect(),
        infusion: book
            .conversions
            .iter()
            .filter(|c| infused.contains(c.chemical.as_str()))
            .flat_map(|c| c.input.items.iter().cloned())
            .collect(),
        energy: book.energy.iter().flat_map(|e| e.input.items.iter().cloned()).collect(),
    };
    if let Ok(mut t) = TAKES.write() {
        *t = Some(takes);
    }
    BOOK.with(|b| *b.borrow_mut() = book);
    EDITION.set(EDITION.get() + 1);
}

/// `MekanismRecipeType.SMELTING`: each of vanilla's smelting recipes, one of
/// its ingredient in and its result out (the cooking time isn't Mekanism's).
fn smelting(cooking: &[Cooking]) -> Vec<Rc<Recipe>> {
    cooking
        .iter()
        .filter(|c| c.kind == "minecraft:smelting" && c.result.count > 0 && !c.ingredient.0.is_empty())
        .map(|c| {
            Rc::new(Recipe {
                id: format!("minecraft:{}", c.id),
                input: ItemIn { items: c.ingredient.0.clone(), count: 1 },
                chemical: None,
                output: c.result.clone(),
            })
        })
        .collect()
}

/// `findFirstRecipe`: the first recipe of `kind` (in file order) that takes
/// `input`, and the infuser's that also takes what its tank holds.
fn find(kind: MachineKind, input: &ItemStack, tank: &Tank) -> Option<Rc<Recipe>> {
    BOOK.with(|b| {
        b.borrow()
            .recipes
            .get(&kind)?
            .iter()
            .find(|r| r.input.takes(input) && (!kind.infuses() || tank.amount > 0 && r.chemical_fits(tank)))
            .cloned()
    })
}

/// `containsRecipeBA(input, chemical)`: whether a recipe takes `chemical` with
/// the item in the input slot (any recipe when it is empty).
fn infuser_takes(input: Option<&ItemStack>, chemical: &str) -> bool {
    BOOK.with(|b| {
        b.borrow().recipes.get(&MachineKind::MetallurgicInfuser).is_some_and(|list| {
            list.iter().any(|r| {
                r.chemical.as_ref().is_some_and(|(k, _)| k.iter().any(|c| c == chemical)) && input.is_none_or(|s| r.input.takes(s))
            })
        })
    })
}

// ---- The saved compound -----------------------------------------------------------

/// A machine as its block entity saves it.
#[derive(Clone, Debug, PartialEq)]
struct Saved {
    energy: u64,
    progress: u32,
    upgrades: Upgrades,
    sides: SideConfig,
    facing: Direction,
    tank: Tank,
    activity: Activity,
}

fn key(name: &str) -> horizon_world::nbt::JavaString {
    mek_world::key(name)
}

fn read(nbt: &Compound, kind: MachineKind) -> Saved {
    let mut sides = SideConfig::new(kind);
    if let Some(Tag::Compound(config)) = nbt.get(&key("component_config")) {
        mek_world::sides_of(config, ENERGY, &mut sides.energy);
        mek_world::sides_of(config, ITEM, &mut sides.items);
        if let Some(chemicals) = sides.chemicals.as_mut() {
            mek_world::sides_of(config, CHEMICAL, chemicals);
        }
        if let Some(v) = mek_world::number(config.get(&key(&format!("eject{ITEM}")))) {
            sides.items_eject = v != 0;
        }
    }
    let mut upgrades = Upgrades::default();
    if let Some(Tag::Compound(c)) = nbt.get(&key("component_upgrade"))
        && let Some(Tag::List(list)) = c.get(&key("upgrades"))
    {
        for entry in list {
            if let Tag::Compound(u) = entry {
                let kind = Upgrade::by_id(mek_world::number(u.get(&key("type"))).unwrap_or(0) as i32);
                let amount = mek_world::number(u.get(&key("amount"))).unwrap_or(0).max(0) as u32;
                upgrades.set(kind, amount);
            }
        }
    }
    let mut tank = Tank::default();
    if let Some(Tag::List(list)) = nbt.get(&key("chemical_tanks")) {
        for entry in list {
            if let Tag::Compound(t) = entry
                && mek_world::number(t.get(&key("tank"))).unwrap_or(0) == 0
                && let Some(Tag::Compound(stored)) = t.get(&key("stored"))
                && let Some(Tag::String(id)) = stored.get(&key("id"))
            {
                let amount = mek_world::number(stored.get(&key("amount"))).unwrap_or(0).max(0) as u64;
                if amount > 0 {
                    tank = Tank { chemical: Some(id.to_string_lossy()), amount };
                }
            }
        }
    }
    Saved {
        energy: mek_world::saved_energy(nbt).unwrap_or(0),
        progress: mek_world::number(nbt.get(&key("progress"))).unwrap_or(0).clamp(0, i64::from(i32::MAX)) as u32,
        upgrades,
        sides,
        facing: mek_world::generator_facing(nbt),
        tank,
        activity: mek_world::read_activity(nbt),
    }
}

/// `component_config` (`TileComponentConfig.write`) and `facing`.
fn write_sides(nbt: &mut Compound, saved: &Saved) {
    nbt.insert(key("facing"), Tag::String(key(saved.facing.name())));
    let mut config = Compound::new();
    let mut put = |kind: u8, sides: &[DataType; 6], eject: bool| {
        config.insert(key(&format!("eject{kind}")), Tag::Byte(i8::from(eject)));
        config.insert(key(&format!("config{kind}")), Tag::IntArray(sides.iter().map(|t| t.id()).collect()));
    };
    put(ENERGY, &saved.sides.energy, false);
    put(ITEM, &saved.sides.items, saved.sides.items_eject);
    if let Some(chemicals) = &saved.sides.chemicals {
        put(CHEMICAL, chemicals, false);
    }
    nbt.insert(key("component_config"), Tag::Compound(config));
}

/// Everything else that changed since `before`, in Mekanism's names.
fn write(nbt: &mut Compound, saved: &Saved, before: &Saved) {
    if saved.energy != before.energy || !nbt.contains_key(&key("energy_containers")) && saved.energy > 0 {
        mek_world::write_energy(nbt, saved.energy);
    }
    if saved.progress != before.progress || !nbt.contains_key(&key("progress")) {
        nbt.insert(key("progress"), Tag::Int(saved.progress.min(i32::MAX as u32) as i32));
    }
    if saved.upgrades != before.upgrades {
        let list: Vec<Tag> = Upgrade::ALL
            .iter()
            .filter(|u| saved.upgrades.get(**u) > 0)
            .map(|u| {
                let mut c = Compound::new();
                c.insert(key("type"), Tag::Int(u.ordinal()));
                c.insert(key("amount"), Tag::Int(saved.upgrades.get(*u) as i32));
                Tag::Compound(c)
            })
            .collect();
        let mut component = Compound::new();
        if !list.is_empty() {
            component.insert(key("upgrades"), Tag::List(list));
        }
        nbt.insert(key("component_upgrade"), Tag::Compound(component));
    }
    if saved.sides != before.sides || saved.facing != before.facing || !nbt.contains_key(&key("component_config")) {
        write_sides(nbt, saved);
    }
    if saved.tank != before.tank {
        match &saved.tank.chemical {
            Some(id) if saved.tank.amount > 0 => {
                let mut stored = Compound::new();
                stored.insert(key("id"), Tag::String(key(id)));
                stored.insert(key("amount"), Tag::Long(saved.tank.amount.min(i64::MAX as u64) as i64));
                let mut tank = Compound::new();
                tank.insert(key("tank"), Tag::Byte(0));
                tank.insert(key("stored"), Tag::Compound(stored));
                nbt.insert(key("chemical_tanks"), Tag::List(vec![Tag::Compound(tank)]));
            }
            _ => {
                nbt.remove(&key("chemical_tanks"));
            }
        }
    }
    if saved.activity != before.activity || !nbt.contains_key(&key("active_state")) {
        mek_world::write_activity(nbt, saved.activity);
    }
}

/// The machine's energy store: its capacity by its energy upgrades
/// (`MachineEnergyContainer.input`: anything may put in, only the machine takes out).
fn store(saved: &Saved, config: &EnergyConfig, kind: MachineKind) -> EnergyContainer {
    let (_, storage) = config.machines.of(kind);
    let mut container = EnergyContainer::basic(saved.upgrades.max_energy(storage, config.machines.max_upgrade_multiplier));
    container.stored = saved.energy.min(container.capacity);
    container
}

// ---- Block entities and slots -----------------------------------------------------

pub(super) fn kind_of(block: &str) -> Option<MachineKind> {
    MachineKind::of_block(block)
}

/// `native_block_entity` for a machine: its item's side config
/// (`AttachedSideConfig`, which Mekanism's `/setblock` leaves out: its sides
/// are all `NONE` there).
pub(super) fn new_entity(kind: MachineKind) -> (String, Compound) {
    let saved = Saved {
        energy: 0,
        progress: 0,
        upgrades: Upgrades::default(),
        sides: SideConfig::new(kind),
        facing: Direction::North,
        tank: Tank::default(),
        activity: Activity::default(),
    };
    let mut nbt = Compound::new();
    write_sides(&mut nbt, &saved);
    (kind.id().to_owned(), nbt)
}

/// `native_block_entity_container`: Mekanism's slots, then the upgrade
/// component's input and output, in a 3x3 menu whose other slots the screen
/// leaves out. Container faces are a block's, not a machine's facing: every
/// face reaches the input, energy and output slots (and the infuser's
/// infusion slot); hoppers put into the first ones, as `slot_accepts` lets
/// them, and take from the output.
pub(super) fn container(kind: MachineKind) -> ModContainer {
    let s = kind.slots();
    let mut inputs = vec![s.input, s.energy];
    inputs.extend(s.infusion);
    let mut faces = inputs.clone();
    faces.push(s.output);
    faces.sort_unstable();
    ModContainer { slots: 9, faces: std::array::from_fn(|_| faces.clone()), inputs, outputs: vec![s.output], menu: Some("minecraft:generic_3x3".into()) }
}

/// `native_block_entity_slot_accepts`: the input slot takes what a recipe
/// takes (`InputInventorySlot`'s `containsRecipe`), the energy slot what
/// converts to energy (`EnergyInventorySlot.fillOrConvert`), the infusion slot
/// what converts to an infuse type a recipe uses, the upgrade slot the
/// upgrades the machine takes; nothing goes into an output slot. Before a world
/// has read its recipes (a LAN guest's prediction), inputs pass.
pub(super) fn slot_accepts(kind: MachineKind, slot: usize, stack: &ItemStack, accepts: &mut bool) {
    let s = kind.slots();
    let item = &*stack.item;
    let known = |pick: &dyn Fn(&Takes) -> bool| TAKES.read().ok().and_then(|t| t.as_ref().map(pick)).unwrap_or(true);
    let ok = if slot == s.input {
        known(&|t| t.inputs.get(&kind).is_some_and(|i| i.contains(item)))
    } else if Some(slot) == s.infusion {
        known(&|t| t.infusion.contains(item))
    } else if slot == s.energy {
        known(&|t| t.energy.contains(item))
    } else if slot == s.upgrade {
        Upgrade::of_item(item).is_some_and(|u| machine::MACHINE_UPGRADES.contains(&u))
    } else {
        false
    };
    if !ok {
        *accepts = false;
    }
}

fn stack_in(entity: &ModBlockEntity, slot: usize) -> Option<&ItemStack> {
    entity.items.get(slot).and_then(Option::as_ref).filter(|s| s.count > 0)
}

fn shrink(entity: &mut ModBlockEntity, slot: usize, by: u32) {
    if let Some(cell) = entity.items.get_mut(slot) {
        if let Some(stack) = cell.as_mut() {
            stack.count = stack.count.saturating_sub(by);
        }
        if cell.as_ref().is_some_and(|s| s.count == 0) {
            *cell = None;
        }
    }
}

fn same_kind(a: &ItemStack, b: &ItemStack) -> bool {
    a.item == b.item && a.damage == b.damage && a.extra == b.extra
}

fn live_key(dimension: &str, pos: [i32; 3]) -> (String, [i32; 3]) {
    (dimension.to_owned(), pos)
}

/// `native_block_entity_unload` of a machine whose chunk unloads: its unsaved
/// state starts over when it comes back, as a new block entity's. A machine
/// replaced in place (`/data merge`, which Java loads into the same block
/// entity) keeps its cached recipe; one broken and placed again starts with
/// no progress anyway.
pub(super) fn forget(dimension: &str, pos: [i32; 3]) {
    LIVE.with(|l| l.borrow_mut().remove(&live_key(dimension, pos)));
}

pub(super) fn clear() {
    LIVE.with(|l| l.borrow_mut().clear());
    BOOK.with(|b| *b.borrow_mut() = Book::default());
}

// ---- The tick ---------------------------------------------------------------------

/// `native_block_entity_tick` of a machine: `TileEntityMekanism.tickServer`
/// (the upgrade component, the active state's delay) and `onUpdateServer`
/// (`TileEntityConfigurableMachine`'s ejector, then the energy slot, the
/// infuser's infusion slot and the recipe), then the menu's data slots.
/// Redstone control is IGNORED, the default.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, kind: MachineKind) {
    let (config, _) = mek_world::energy_config();
    let dimension = game.native_dimension();
    refresh_book(game);
    let slots = kind.slots();
    let multiplier = config.machines.max_upgrade_multiplier;
    let (usage, _) = config.machines.of(kind);
    let before = read(&entity.nbt, kind);
    let mut saved = before.clone();
    saved.facing = game.native_block_property(pos, "facing").and_then(|f| Direction::from_name(&f)).unwrap_or(Direction::North);
    let id = live_key(dimension, pos);
    let mut live = LIVE.with(|l| l.borrow_mut().remove(&id)).unwrap_or_default();

    // The upgrade component: an upgrade in its slot goes in after a second.
    let upgrade_stack = stack_in(entity, slots.upgrade).map(|s| (s.item.to_string(), s.count));
    if upgrade_stack != live.upgrade_stack {
        live.upgrade.check = true;
    }
    let offered = upgrade_stack.as_ref().and_then(|(item, count)| Some((Upgrade::of_item(item)?, *count)));
    let taken = live.upgrade.tick(&mut saved.upgrades, &machine::MACHINE_UPGRADES, offered);
    if taken > 0 {
        shrink(entity, slots.upgrade, taken);
    }
    live.upgrade_stack = stack_in(entity, slots.upgrade).map(|s| (s.item.to_string(), s.count));

    // The active state's delay runs down.
    let mut shown = kind.shows_active() && game.native_block_property(pos, "active").as_deref() == Some("true");
    if kind.shows_active() {
        let changed = saved.activity.tick(shown);
        mek_world::show_active(game, pos, changed, &mut shown);
    }

    // The ejector: what the output slot holds, out of the faces whose item type outputs.
    if saved.sides.items_eject {
        if live.eject_delay == 0 {
            eject_items(game, pos, entity, slots.output, &machine::eject_faces(&saved.sides.items, saved.facing));
            live.eject_delay = machine::EJECT_DELAY;
        } else {
            live.eject_delay -= 1;
        }
    }

    let mut store = store(&saved, &config, kind);
    // The energy slot: an item that converts to energy, when all of it fits.
    if let Some(stack) = stack_in(entity, slots.energy).cloned()
        && store.needed() > 0
    {
        let found = BOOK.with(|b| b.borrow().energy.iter().find(|e| e.input.takes(&stack)).cloned());
        if let Some(conversion) = found
            && stack.count >= conversion.input.count
            && store.needed() >= conversion.joules
        {
            store.insert(conversion.joules, false, Automation::Manual);
            shrink(entity, slots.energy, conversion.input.count);
        }
    }
    // The infusion slot: an item that converts to an infuse type a recipe takes.
    if let Some(infusion) = slots.infusion
        && let Some(stack) = stack_in(entity, infusion).cloned()
        && saved.tank.amount < machine::INFUSE_TANK
    {
        let found = BOOK.with(|b| b.borrow().conversions.iter().find(|c| c.input.takes(&stack)).cloned());
        if let Some(conversion) = found
            && stack.count >= conversion.input.count
            && infuser_takes(stack_in(entity, slots.input), &conversion.chemical)
            && saved.tank.fill(&conversion.chemical, conversion.amount, machine::INFUSE_TANK)
        {
            shrink(entity, infusion, conversion.input.count);
        }
    }

    // The recipe (`RecipeCacheLookupMonitor.updateAndProcess`).
    let required = saved.upgrades.ticks(machine::BASE_TICKS, multiplier);
    let per_tick = saved.upgrades.energy_per_tick(usage, multiplier);
    let baseline = if kind.infuses() { saved.upgrades.operations_per_tick(machine::BASE_TICKS, multiplier) } else { 1 };
    let input = stack_in(entity, slots.input).cloned();
    let edition = EDITION.get();
    let cached = live.recipe.as_ref().filter(|(e, _)| *e == edition).map(|(_, r)| r.clone());
    let valid = |r: &Recipe| input.as_ref().is_some_and(|i| r.input.takes(i)) && r.chemical_fits(&saved.tank);
    let mut warnings = 0;
    let recipe = match cached {
        Some(r) if valid(r.as_ref()) => Some(r),
        old => match input.as_ref().and_then(|i| find(kind, i, &saved.tank)) {
            Some(found) => {
                // A first cache takes the saved progress (`loadSavedData`); another starts at 0.
                saved.progress = if old.is_none() { machine::resumed(saved.progress, required) } else { 0 };
                live.recipe = Some((edition, found.clone()));
                Some(found)
            }
            None => {
                if input.is_some() {
                    warnings |= menu::WARN_NO_RECIPE;
                }
                if old.is_none() {
                    live.recipe = None;
                }
                old
            }
        },
    };
    let mut active = None;
    if let Some(recipe) = recipe {
        let held = input.as_ref().filter(|i| recipe.input.takes(i)).map_or(0, |i| i.count);
        let mut inputs = Inputs::of(held, recipe.input.count);
        if let Some((_, amount)) = &recipe.chemical {
            let fits = recipe.chemical_fits(&saved.tank);
            let chemical = if fits { Inputs::of(saved.tank.amount.min(u64::from(u32::MAX)) as u32, (*amount).min(u64::from(u32::MAX)) as u32) } else { Inputs::Mismatched };
            inputs = inputs.both(chemical);
        }
        let output = stack_in(entity, slots.output);
        let max_stack = game.native_max_stack_size(&recipe.output.item).unwrap_or(64);
        let room = machine::output_operations(output.map(|o| (o.count, same_kind(o, &recipe.output))), recipe.output.count, max_stack);
        if room == 0 {
            warnings |= menu::WARN_OUTPUT;
        }
        if per_tick > store.stored {
            warnings |= menu::WARN_ENERGY;
        }
        let ran = machine::process(&mut saved.progress, required, &mut store, per_tick, baseline, inputs, room, true);
        if ran.finished {
            shrink(entity, slots.input, recipe.input.count * ran.operations);
            if let Some((_, amount)) = &recipe.chemical {
                saved.tank.drain(amount * u64::from(ran.operations));
            }
            let made = recipe.output.count * ran.operations;
            match entity.items.get_mut(slots.output) {
                Some(Some(out)) => out.count += made,
                Some(cell) => *cell = Some(ItemStack { count: made, ..recipe.output.clone() }),
                None => {}
            }
        }
        active = Some(ran.active());
    }
    saved.energy = store.stored;
    if let Some(active) = active {
        let changed = saved.activity.set(active, shown, config.deactivation_delay);
        if kind.shows_active() {
            mek_world::show_active(game, pos, changed, &mut shown);
        }
    }
    write(&mut entity.nbt, &saved, &before);
    let input_joules = mek_world::take_received(pos);
    entity.data = MachineView {
        stored: saved.energy,
        capacity: store.capacity,
        per_tick,
        progress: saved.progress,
        required,
        active: saved.activity.current,
        energy: saved.sides.energy,
        items: saved.sides.items,
        chemicals: saved.sides.chemicals,
        items_eject: saved.sides.items_eject,
        upgrades: saved.upgrades,
        upgrade_ticks: live.upgrade.ticks,
        infuse: machine::infuse_number(saved.tank.chemical.as_deref()),
        infused: saved.tank.amount,
        input: input_joules,
        warnings,
    }
    .pack();
    LIVE.with(|l| l.borrow_mut().insert(id, live));
}

/// `TileComponentEjector.outputItems`: the output slot's stack goes to each
/// face in turn, as much as fits (`native_insert_item` through the
/// neighbour's touching face), until none is left.
fn eject_items(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, slot: usize, faces: &[Direction]) {
    for &face in faces {
        let Some(stack) = stack_in(entity, slot).cloned() else {
            return;
        };
        let rest = game.native_insert_item(mek_world::beside(pos, face), mek_world::engine_face(face.opposite()), stack);
        if let Some(cell) = entity.items.get_mut(slot) {
            *cell = rest.filter(|r| r.count > 0);
        }
    }
}

// ---- Energy from outside ------------------------------------------------------------

/// Whether the machine in `entity` takes energy through its absolute face
/// `face` (its energy config's `INPUT` there).
pub(super) fn takes_energy(entity: &ModBlockEntity, face: Direction) -> Option<bool> {
    let kind = kind_of(&entity.id)?;
    let saved = read(&entity.nbt, kind);
    Some(saved.sides.takes_energy(saved.facing, face))
}

/// Offers `amount` Joules to the machine in `entity` through `face`: what it
/// didn't take (`insertEnergy` from outside, up to its capacity).
pub(super) fn offer(entity: &mut ModBlockEntity, config: &EnergyConfig, face: Direction, amount: u64, simulate: bool) -> Option<u64> {
    let kind = kind_of(&entity.id)?;
    let saved = read(&entity.nbt, kind);
    if !saved.sides.takes_energy(saved.facing, face) {
        return Some(amount);
    }
    let mut container = store(&saved, config, kind);
    let rest = container.insert(amount, simulate, Automation::External);
    if !simulate && container.stored != saved.energy {
        mek_world::write_energy(&mut entity.nbt, container.stored);
    }
    Some(rest)
}

/// A machine's energy as one side reaches it: an input side takes, nothing
/// gives (`MachineEnergyContainer.input`'s `notExternal` extraction).
struct MachineSide {
    container: EnergyContainer,
    open: bool,
    taken: u64,
}

impl StrictEnergy for MachineSide {
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

/// `native_fe_storage` of a machine: its store in whole FE on a face its
/// energy config opens.
pub(super) fn fe_storage(entity: &ModBlockEntity, face: Option<Face>, config: &EnergyConfig, info: &mut Option<NativeFeInfo>) {
    let Some(kind) = kind_of(&entity.id) else {
        return;
    };
    let saved = read(&entity.nbt, kind);
    if config.blacklist_forge || face.is_some_and(|f| !saved.sides.takes_energy(saved.facing, mek_world::mek_direction(f))) {
        return;
    }
    let container = store(&saved, config, kind);
    let rate = Conversion(config.fe_conversion_rate);
    *info = Some(NativeFeInfo {
        stored: i64::from(rate.to_fe_int(container.stored.min(i64::MAX as u64) as i64)),
        capacity: i64::from(rate.to_fe_int(container.capacity.min(i64::MAX as u64) as i64)),
        can_receive: true,
        can_extract: true,
    });
}

/// `native_fe_receive` and `native_fe_extract` of a machine.
#[allow(clippy::too_many_arguments)]
pub(super) fn fe_transfer(
    pos: [i32; 3],
    entity: &mut ModBlockEntity,
    face: Option<Face>,
    config: &EnergyConfig,
    max: i64,
    simulate: bool,
    extract: bool,
    moved: &mut Option<i64>,
) {
    let Some(kind) = kind_of(&entity.id) else {
        return;
    };
    let saved = read(&entity.nbt, kind);
    let open = face.is_some_and(|f| saved.sides.takes_energy(saved.facing, mek_world::mek_direction(f)));
    if config.blacklist_forge || face.is_some() && !open {
        return;
    }
    let mut side = MachineSide { container: store(&saved, config, kind), open, taken: 0 };
    let rate = Conversion(config.fe_conversion_rate);
    let max = max.clamp(0, i64::from(i32::MAX)) as i32;
    let fe = if extract { fe::extract_fe(&mut side, rate, max, simulate) } else { fe::receive_fe(&mut side, rate, max, simulate) };
    if !simulate && side.container.stored != saved.energy {
        mek_world::write_energy(&mut entity.nbt, side.container.stored);
    }
    if side.taken > 0 {
        mek_world::add_received(pos, side.taken);
    }
    *moved = Some(i64::from(fe));
}

// ---- Buttons and probes ---------------------------------------------------------------

/// `native_menu_button` of a machine: its side configuration (energy, items,
/// the infuser's chemicals), the upgrade window's uninstall, which puts the
/// upgrades in the upgrade output slot as far as they fit
/// (`TileComponentUpgrade.removeUpgrade`), and the infuser's dump.
pub(super) fn button(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, id: i32) {
    let (Some(kind), Some(button)) = (kind_of(&entity.id), MachineButton::from_id(id)) else {
        return;
    };
    let before = read(&entity.nbt, kind);
    let mut saved = before.clone();
    match button {
        MachineButton::Config(b) => {
            saved.sides.press(kind, b);
        }
        MachineButton::Remove { upgrade, all } => {
            let installed = saved.upgrades.get(upgrade);
            if installed > 0 {
                let out = kind.slots().upgrade_out;
                let item = upgrade.item();
                let max = game.native_max_stack_size(&item).unwrap_or(64).min(64);
                let held = stack_in(entity, out);
                let room = match held {
                    None => max,
                    Some(s) if &*s.item == item => max.saturating_sub(s.count),
                    Some(_) => 0,
                };
                let remove = (if all { installed } else { 1 }).min(room);
                if remove > 0 {
                    saved.upgrades.set(upgrade, installed - remove);
                    let count = held.map_or(0, |s| s.count) + remove;
                    if let Some(cell) = entity.items.get_mut(out) {
                        *cell = Some(ItemStack::new(&item, count));
                    }
                    LIVE.with(|l| {
                        if let Some(live) = l.borrow_mut().get_mut(&live_key(game.native_dimension(), pos)) {
                            live.upgrade.check = true;
                        }
                    });
                }
            }
        }
        MachineButton::Dump => saved.tank = Tank::default(),
    }
    if saved != before {
        write(&mut entity.nbt, &saved, &before);
        let mut view = MachineView::unpack(&entity.data).unwrap_or_default();
        view.energy = saved.sides.energy;
        view.items = saved.sides.items;
        view.chemicals = saved.sides.chemicals;
        view.items_eject = saved.sides.items_eject;
        view.upgrades = saved.upgrades;
        view.infuse = machine::infuse_number(saved.tank.chemical.as_deref());
        view.infused = saved.tank.amount;
        entity.data = view.pack();
    }
}

/// What `/mek probe` answers for a machine.
pub(super) fn probe(game: &Game<'_>, pos: [i32; 3], id: &str, nbt: &Compound, data: &[i32], items: &[Option<ItemStack>]) -> serde_json::Value {
    let Some(kind) = kind_of(id) else {
        return serde_json::json!({ "pos": pos, "id": id, "error": "not a machine" });
    };
    let (config, from_files) = mek_world::energy_config();
    let saved = read(nbt, kind);
    let view = MachineView::unpack(data).unwrap_or_default();
    let live = LIVE.with(|l| l.borrow().get(&live_key(game.native_dimension(), pos)).cloned()).unwrap_or_default();
    let names = |sides: &[DataType; 6]| -> serde_json::Map<String, serde_json::Value> {
        mekanism_energy::sides::RelativeSide::ALL.iter().map(|s| (s.name().to_owned(), sides[s.index()].name().into())).collect()
    };
    let slots: Vec<serde_json::Value> = items
        .iter()
        .take(kind.slots().count)
        .map(|s| s.as_ref().map_or(serde_json::Value::Null, |s| serde_json::json!({ "item": &*s.item, "count": s.count })))
        .collect();
    let (usage, storage) = config.machines.of(kind);
    serde_json::json!({
        "pos": pos,
        "id": id,
        "energy": saved.energy,
        "capacity": store(&saved, &config, kind).capacity,
        "per_tick": view.per_tick,
        "progress": saved.progress,
        "required": view.required,
        "active": saved.activity.current,
        "shown_active": game.native_block_property(pos, "active").as_deref() == Some("true"),
        "facing": saved.facing.name(),
        "recipe": live.recipe.as_ref().map(|(_, r)| r.id.clone()),
        "upgrades": Upgrade::ALL.iter().filter(|u| saved.upgrades.get(**u) > 0).map(|u| (u.name().to_owned(), saved.upgrades.get(*u).into())).collect::<serde_json::Map<String, serde_json::Value>>(),
        "upgrade_ticks": live.upgrade.ticks,
        "energy_config": names(&saved.sides.energy),
        "item_config": names(&saved.sides.items),
        "item_eject": saved.sides.items_eject,
        "chemical_config": saved.sides.chemicals.as_ref().map(names),
        "tank": { "chemical": saved.tank.chemical, "amount": saved.tank.amount },
        "slots": slots,
        "warnings": view.warnings,
        "config": { "from_files": from_files, "usage": usage, "storage": storage, "max_upgrade_multiplier": config.machines.max_upgrade_multiplier },
        "recipes": BOOK.with(|b| b.borrow().recipes.get(&kind).map_or(0, Vec::len)),
    })
}
