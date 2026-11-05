// Mekanism's first processing machines in the game (support module
// `crate::game::mek_machines` of the `mekanism` native bundle): the
// Enrichment Chamber, the Crusher, the Energized Smelter, the Metallurgic
// Infuser, the Electrolytic Separator and the advanced machines (the Osmium
// Compressor, Purification Chamber and Chemical Injection Chamber, with
// their chemical tank and per-tick use: `mekanism_energy::advanced`), and the
// glue the Chemical Oxidizer and Chemical Infuser (their own ticks are
// `mek_chem_machines`') and the ore chain's Chemical Dissolution Chamber,
// Chemical Washer and Chemical Crystallizer (theirs are `mek_ore_machines`')
// share with them. Their rules are the bundle's crate
// (`mekanism_energy::machine`, `chemical` and `separator`, MIT, from Mekanism
// 10.7.19, Copyright (c) 2017-2025 Aidan C. Brady); this module is the glue:
// their block entities and saved compound in Mekanism's names, their slots
// (input, output, energy, the upgrade component's two), the recipes they read
// from the world's tables (`native_recipes_of_type` for Mekanism's types,
// `native_cooking_recipes` of hook points API 159 for the smelter), each tick
// in `TileEntityMekanism.tickServer` order (upgrades in, the active state's
// delay, the ejector, the energy slot, the infusion or fluid slot, the
// recipe, the separator's dumps), Joules from cables, cubes and Forge Energy
// on the faces their energy config opens, their chemical tanks as a chemical
// handler by side (`mek_chemicals`), the separator's fluid tank as a fluid
// handler (hook points API 160), and `/mek probe`. A machine's cached recipe
// and its upgrade slot's progress are kept here by position, not saved, as
// Mekanism keeps them in its block entity. No borrow of that state is held
// across a call into the game.

use super::{
    Game, mek_chemicals,
    mek_world::{self, ENERGY, ITEM},
    native_hooks::{NativeFeInfo, NativeFluidTankInfo},
};
use horizon_client::{
    containers::{ModBlockEntity, ModContainer},
    crafting::Cooking,
    items::ItemStack,
};
use horizon_core::geometry::Direction as Face;
use horizon_world::{
    fluid_ingredient::SizedFluidIngredient,
    fluid_stack::FluidStack,
    nbt::{Compound, Tag},
};
use mekanism_energy::{
    advanced::{self, ChemicalUse, Rng},
    chemical::{self, ChemicalHandler, ChemicalRegistry, ChemicalStack, ChemicalStackIngredient, ChemicalTank, SidedTanks, TankRules},
    chemical_machine,
    chemical_tank::{self, OwnTank},
    config::EnergyConfig,
    container::{Automation, EnergyContainer},
    fe::{self, Conversion, StrictEnergy},
    heat::Activity,
    machine::{self, Inputs, MachineKind, SideConfig, Upgrade, UpgradeSlot, Upgrades},
    menu::{self, MachineButton, MachineView, SeparatorView},
    separator::{self, GasMode, Separation},
    sides::{DataType, Direction, RelativeSide},
};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::RwLock,
};

/// `TransmissionType`'s ordinals in `component_config`: `FLUID` and `CHEMICAL`.
const FLUID: u8 = 1;
const CHEMICAL: u8 = 2;

/// `TileEntityMetallurgicInfuser.MAX_INFUSE`: the infusion tank, in mB.
const INFUSE_TANK: i64 = 1000;

// ---- Recipes -------------------------------------------------------------------

/// An item ingredient as Mekanism's `ItemStackIngredient` reads it: the items
/// it takes and how many.
#[derive(Clone, Debug)]
pub(super) struct ItemIn {
    pub(super) items: HashSet<String>,
    pub(super) count: u32,
}

impl ItemIn {
    pub(super) fn takes(&self, stack: &ItemStack) -> bool {
        self.items.contains(&*stack.item)
    }
}

/// A machine recipe: `ItemStackToItemStackRecipe`, or with a chemical
/// (`ItemStackChemicalToItemStackRecipe`, its `ChemicalStackIngredient`).
#[derive(Clone, Debug)]
struct Recipe {
    id: String,
    input: ItemIn,
    chemical: Option<ChemicalStackIngredient>,
    output: ItemStack,
    /// `perTickUsage`: the chemical taken every tick (`ItemStackConstantChemicalToObjectCachedRecipe`).
    per_tick: bool,
}

impl Recipe {
    /// Whether the chemical `held` (none: an empty tank) is the recipe's.
    fn chemical_fits(&self, held: Option<&str>, registry: &ChemicalRegistry) -> bool {
        match &self.chemical {
            None => true,
            Some(c) => held.is_some() && c.test_type(held, registry),
        }
    }
}

/// `ElectrolysisRecipe` (`mekanism:separating`): a fluid in, two chemicals
/// out, and its energy multiplier.
#[derive(Clone, Debug)]
struct SeparatingRecipe {
    id: String,
    input: SizedFluidIngredient,
    left: ChemicalStack,
    right: ChemicalStack,
    energy_multiplier: u64,
}

/// `ItemStackToChemicalRecipe` (`mekanism:chemical_conversion`) and
/// `ItemStackToEnergyRecipe` (`mekanism:energy_conversion`).
#[derive(Clone, Debug)]
struct ChemicalConversion {
    input: ItemIn,
    output: ChemicalStack,
}

#[derive(Clone, Debug)]
struct EnergyConversion {
    input: ItemIn,
    joules: u64,
}

/// `ItemStackToChemicalRecipe` (`mekanism:oxidizing`): an item in, a chemical out.
#[derive(Clone, Debug)]
pub(super) struct Oxidizing {
    pub(super) id: String,
    pub(super) input: ItemIn,
    pub(super) output: ChemicalStack,
}

/// `ChemicalChemicalToChemicalRecipe` (`mekanism:chemical_infusing`).
#[derive(Clone, Debug)]
pub(super) struct ChemicalInfusing {
    pub(super) id: String,
    pub(super) left: ChemicalStackIngredient,
    pub(super) right: ChemicalStackIngredient,
    pub(super) output: ChemicalStack,
}

/// The recipes of the world's tables, made again when one of them changes.
#[derive(Default)]
struct Book {
    /// Where each table was when this was made (its slice, by address and length).
    stamp: Vec<(usize, usize)>,
    recipes: HashMap<MachineKind, Vec<Rc<Recipe>>>,
    separating: Vec<Rc<SeparatingRecipe>>,
    conversions: Vec<ChemicalConversion>,
    energy: Vec<EnergyConversion>,
    oxidizing: Vec<Rc<Oxidizing>>,
    chemical_infusing: Vec<Rc<ChemicalInfusing>>,
}

const TYPES: [&str; 11] = [
    "mekanism:enriching",
    "mekanism:crushing",
    "mekanism:metallurgic_infusing",
    "mekanism:chemical_conversion",
    "mekanism:energy_conversion",
    "mekanism:separating",
    "mekanism:oxidizing",
    "mekanism:chemical_infusing",
    "mekanism:compressing",
    "mekanism:purifying",
    "mekanism:injecting",
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

/// What the slots and tanks of each kind take, for
/// `native_block_entity_slot_accepts` and the fluid handler, which may be
/// asked with no world at hand: made where the recipes are read.
#[derive(Default)]
struct Takes {
    inputs: HashMap<MachineKind, HashSet<String>>,
    infusion: HashSet<String>,
    /// What each advanced machine's chemical slot converts (`fillOrConvert`).
    extras: HashMap<MachineKind, HashSet<String>>,
    energy: HashSet<String>,
    /// The separating recipes' inputs (`containsRecipe` of the fluid tank).
    separating: Vec<SizedFluidIngredient>,
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
    /// The separator's recipe last used, for the probe.
    separating: Option<String>,
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

pub(super) fn item_in(game: &Game<'_>, v: &Value) -> Option<ItemIn> {
    let count = v.get("count").and_then(Value::as_u64).unwrap_or(1).clamp(1, 99) as u32;
    Some(ItemIn { items: items_of(game, v)?, count })
}

/// Makes the book again from the world's tables when one changed. A chemical
/// ingredient or stack the registry can't read leaves its recipe out, as
/// Mekanism's codecs fail the recipe.
pub(super) fn refresh_book(game: &Game<'_>) {
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
    let registry = mek_chemicals::registry(game);
    let mut book = Book { stamp, ..Book::default() };
    for kind in MachineKind::ALL {
        let list: Vec<Rc<Recipe>> = if kind == MachineKind::EnergizedSmelter {
            smelting(cooking)
        } else if kind.separates() || kind.oxidizes() || kind.infuses_chemicals() || kind.ore_chain() {
            Vec::new()
        } else {
            game.native_recipes_of_type(kind.recipe_type())
                .iter()
                .filter_map(|r| {
                    let j = &*r.json;
                    let per_tick = j.get("per_tick_usage").and_then(Value::as_bool) == Some(true);
                    let (input, chemical) = if kind.infuses() || kind.advanced() {
                        // Per-tick infusing isn't ported; ATM10 has none.
                        if kind.infuses() && per_tick {
                            return None;
                        }
                        (item_in(game, &j["item_input"])?, Some(ChemicalStackIngredient::from_json(&j["chemical_input"], &registry).ok()?))
                    } else {
                        (item_in(game, &j["input"])?, None)
                    };
                    let output = game.native_recipe_result(&j["output"])?;
                    Some(Rc::new(Recipe { id: r.id.clone(), input, chemical, output, per_tick }))
                })
                .collect()
        };
        book.recipes.insert(kind, list);
    }
    book.separating = game
        .native_recipes_of_type("mekanism:separating")
        .iter()
        .filter_map(|r| {
            let j = &*r.json;
            Some(Rc::new(SeparatingRecipe {
                id: r.id.clone(),
                input: SizedFluidIngredient::parse_flat(&j["input"]).ok()?,
                left: ChemicalStack::from_json(&j["left_chemical_output"], &registry).ok()?,
                right: ChemicalStack::from_json(&j["right_chemical_output"], &registry).ok()?,
                energy_multiplier: j.get("energy_multiplier").map_or(Some(1), Value::as_u64)?.max(1),
            }))
        })
        .collect();
    book.conversions = game
        .native_recipes_of_type("mekanism:chemical_conversion")
        .iter()
        .filter_map(|r| {
            let j = &*r.json;
            Some(ChemicalConversion { input: item_in(game, &j["input"])?, output: ChemicalStack::from_json(&j["output"], &registry).ok()? })
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
    book.oxidizing = game
        .native_recipes_of_type("mekanism:oxidizing")
        .iter()
        .filter_map(|r| {
            let j = &*r.json;
            Some(Rc::new(Oxidizing { id: r.id.clone(), input: item_in(game, &j["input"])?, output: ChemicalStack::from_json(&j["output"], &registry).ok()? }))
        })
        .collect();
    book.chemical_infusing = game
        .native_recipes_of_type("mekanism:chemical_infusing")
        .iter()
        .filter_map(|r| {
            let j = &*r.json;
            Some(Rc::new(ChemicalInfusing {
                id: r.id.clone(),
                left: ChemicalStackIngredient::from_json(&j["left_input"], &registry).ok()?,
                right: ChemicalStackIngredient::from_json(&j["right_input"], &registry).ok()?,
                output: ChemicalStack::from_json(&j["output"], &registry).ok()?,
            }))
        })
        .collect();
    let infusing = &book.recipes[&MachineKind::MetallurgicInfuser];
    let mut inputs: HashMap<MachineKind, HashSet<String>> = book
        .recipes
        .iter()
        .map(|(kind, list)| (*kind, list.iter().flat_map(|r| r.input.items.iter().cloned()).collect()))
        .collect();
    inputs.insert(MachineKind::ChemicalOxidizer, book.oxidizing.iter().flat_map(|r| r.input.items.iter().cloned()).collect());
    let takes = Takes {
        inputs,
        infusion: book
            .conversions
            .iter()
            .filter(|c| infusing.iter().any(|r| r.chemical_fits(c.output.id(), &registry)))
            .flat_map(|c| c.input.items.iter().cloned())
            .collect(),
        extras: MachineKind::ALL
            .into_iter()
            .filter(|k| k.advanced())
            .map(|k| {
                let list = &book.recipes[&k];
                let items = book
                    .conversions
                    .iter()
                    .filter(|c| list.iter().any(|r| r.chemical_fits(c.output.id(), &registry)))
                    .flat_map(|c| c.input.items.iter().cloned())
                    .collect();
                (k, items)
            })
            .collect(),
        energy: book.energy.iter().flat_map(|e| e.input.items.iter().cloned()).collect(),
        separating: book.separating.iter().map(|r| r.input.clone()).collect(),
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
                per_tick: false,
            })
        })
        .collect()
}

/// `findFirstRecipe`: the first recipe of `kind` (in file order) that takes
/// `input`, and the infuser's that also takes what its tank holds.
fn find(kind: MachineKind, input: &ItemStack, held: Option<&str>, registry: &ChemicalRegistry) -> Option<Rc<Recipe>> {
    BOOK.with(|b| {
        b.borrow()
            .recipes
            .get(&kind)?
            .iter()
            .find(|r| r.input.takes(input) && (!(kind.infuses() || kind.advanced()) || r.chemical_fits(held, registry)))
            .cloned()
    })
}

/// `containsRecipeBA(input, chemical)`: whether a recipe takes `chemical` with
/// the item in the input slot (any recipe when it is empty); with no input
/// that is `containsRecipeB`.
fn infuser_takes(input: Option<&ItemStack>, chemical: Option<&str>, registry: &ChemicalRegistry) -> bool {
    takes_chemical(MachineKind::MetallurgicInfuser, input, chemical, registry)
}

/// `containsRecipeBA` / `containsRecipeB` of a machine with an item and a
/// chemical recipe (the metallurgic infuser, the advanced machines).
fn takes_chemical(kind: MachineKind, input: Option<&ItemStack>, chemical: Option<&str>, registry: &ChemicalRegistry) -> bool {
    BOOK.with(|b| {
        b.borrow().recipes.get(&kind).is_some_and(|list| {
            list.iter().any(|r| r.chemical_fits(chemical, registry) && input.is_none_or(|s| r.input.takes(s)))
        })
    })
}

/// `findFirstRecipe` of the separator: the first separating recipe whose
/// ingredient takes the fluid in its tank.
fn find_separating(fluid: &FluidStack) -> Option<Rc<SeparatingRecipe>> {
    if fluid.is_empty() {
        return None;
    }
    BOOK.with(|b| b.borrow().separating.iter().find(|r| r.input.ingredient.test(fluid)).cloned())
}

/// The chemical conversion of `stack` (`mekanism:chemical_conversion`): how
/// many it takes and the chemical they give, for `mek_ore_machines`.
pub(super) fn chemical_conversion(stack: &ItemStack) -> Option<(u32, ChemicalStack)> {
    BOOK.with(|b| b.borrow().conversions.iter().find(|c| c.input.takes(stack)).map(|c| (c.input.count, c.output.clone())))
}

/// The items whose chemical conversion gives a chemical `wanted` takes.
pub(super) fn conversion_inputs(wanted: &dyn Fn(&ChemicalStack) -> bool) -> HashSet<String> {
    BOOK.with(|b| b.borrow().conversions.iter().filter(|c| wanted(&c.output)).flat_map(|c| c.input.items.iter().cloned()).collect())
}

/// The energy slot's conversion of `stack` (`mekanism:energy_conversion`):
/// how many it takes and the Joules they give, for `mek_factories`.
pub(super) fn energy_conversion(stack: &ItemStack) -> Option<(u32, u64)> {
    BOOK.with(|b| b.borrow().energy.iter().find(|e| e.input.takes(stack)).map(|e| (e.input.count, e.joules)))
}

/// Whether an energy slot takes `item` (`EnergyInventorySlot.fillOrConvert`);
/// before a world has read its recipes, anything.
pub(super) fn energy_item(item: &str) -> bool {
    TAKES.read().ok().and_then(|t| t.as_ref().map(|t| t.energy.contains(item))).unwrap_or(true)
}

/// `findFirstRecipe` of the oxidizer: the first oxidizing recipe taking `input`.
pub(super) fn find_oxidizing(input: &ItemStack) -> Option<Rc<Oxidizing>> {
    BOOK.with(|b| b.borrow().oxidizing.iter().find(|r| r.input.takes(input)).cloned())
}

/// The number of oxidizing and chemical infusing recipes, for the probe.
pub(super) fn chemical_recipe_counts() -> (usize, usize) {
    BOOK.with(|b| (b.borrow().oxidizing.len(), b.borrow().chemical_infusing.len()))
}

/// `EitherSideChemical.findFirstRecipe`: the first chemical infusing recipe
/// taking `a` and `b`, one on each side, either way round.
pub(super) fn find_chemical_infusing(a: &ChemicalStack, b: &ChemicalStack, registry: &ChemicalRegistry) -> Option<Rc<ChemicalInfusing>> {
    if a.is_empty() || b.is_empty() {
        return None;
    }
    BOOK.with(|book| {
        book.borrow()
            .chemical_infusing
            .iter()
            .find(|r| {
                let (left, right) = (|s: &ChemicalStack| r.left.test_type(s.id(), registry), |s: &ChemicalStack| r.right.test_type(s.id(), registry));
                (left(a) && right(b)) || (left(b) && right(a))
            })
            .cloned()
    })
}

/// `EitherSideChemical.containsRecipe(a, b)`: whether a chemical infusing
/// recipe takes `a` on one side and `b` (any, when it is empty) on the other.
pub(super) fn chemical_infusing_takes(a: &ChemicalStack, b: &ChemicalStack, registry: &ChemicalRegistry) -> bool {
    BOOK.with(|book| {
        book.borrow().chemical_infusing.iter().any(|r| {
            let other = |i: &ChemicalStackIngredient| b.is_empty() || i.test_type(b.id(), registry);
            (r.left.test_type(a.id(), registry) && other(&r.right)) || (r.right.test_type(a.id(), registry) && other(&r.left))
        })
    })
}

/// The fluid tank's validator (`containsRecipe`): a fluid a separating recipe takes.
fn separates_fluid(fluid: &FluidStack) -> bool {
    TAKES.read().ok().and_then(|t| t.as_ref().map(|t| t.separating.iter().any(|i| i.ingredient.test(fluid)))).unwrap_or(false)
}

// ---- The saved compound -----------------------------------------------------------

/// A machine as its block entity saves it, but for its tanks.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Saved {
    pub(super) energy: u64,
    pub(super) progress: u32,
    pub(super) upgrades: Upgrades,
    pub(super) sides: SideConfig,
    pub(super) facing: Direction,
    pub(super) activity: Activity,
    /// The separator's `dump_left` and `dump_right`.
    pub(super) dumps: [GasMode; 2],
    /// An advanced machine's `used_so_far`: its chemical taken this operation.
    pub(super) used_so_far: i64,
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
        if let Some(fluids) = sides.fluids.as_mut() {
            mek_world::sides_of(config, FLUID, fluids);
        }
        if let Some(v) = mek_world::number(config.get(&key(&format!("eject{ITEM}")))) {
            sides.items_eject = v != 0;
        }
        if kind.chemicals_eject()
            && let Some(v) = mek_world::number(config.get(&key(&format!("eject{CHEMICAL}"))))
        {
            sides.chemicals_eject = v != 0;
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
    let dump = |name: &str| mek_world::number(nbt.get(&key(name))).map_or(GasMode::Idle, |v| GasMode::by_id(v as i32));
    Saved {
        energy: mek_world::saved_energy(nbt).unwrap_or(0),
        progress: mek_world::number(nbt.get(&key("progress"))).unwrap_or(0).clamp(0, i64::from(i32::MAX)) as u32,
        upgrades,
        sides,
        facing: mek_world::generator_facing(nbt),
        activity: mek_world::read_activity(nbt),
        dumps: [dump("dump_left"), dump("dump_right")],
        used_so_far: mek_world::number(nbt.get(&key("used_so_far"))).unwrap_or(0).max(0),
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
    if let Some(fluids) = &saved.sides.fluids {
        put(FLUID, fluids, false);
    }
    if let Some(chemicals) = &saved.sides.chemicals {
        put(CHEMICAL, chemicals, saved.sides.chemicals_eject);
    }
    put(ITEM, &saved.sides.items, saved.sides.items_eject);
    nbt.insert(key("component_config"), Tag::Compound(config));
}

/// Everything else that changed since `before`, in Mekanism's names.
fn write(nbt: &mut Compound, saved: &Saved, before: &Saved, kind: MachineKind) {
    if saved.energy != before.energy || !nbt.contains_key(&key("energy_containers")) && saved.energy > 0 {
        mek_world::write_energy(nbt, saved.energy);
    }
    if kind.separates() {
        // `writeSustainedData`: the dump modes as their ordinals; no progress.
        if saved.dumps != before.dumps || !nbt.contains_key(&key("dump_left")) {
            nbt.insert(key("dump_left"), Tag::Int(saved.dumps[0].id()));
            nbt.insert(key("dump_right"), Tag::Int(saved.dumps[1].id()));
        }
    } else if kind.progresses() && (saved.progress != before.progress || !nbt.contains_key(&key("progress"))) {
        nbt.insert(key("progress"), Tag::Int(saved.progress.min(i32::MAX as u32) as i32));
    }
    if (kind.advanced() || kind.dissolves()) && (saved.used_so_far != before.used_so_far || !nbt.contains_key(&key("used_so_far"))) {
        nbt.insert(key("used_so_far"), Tag::Long(saved.used_so_far));
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
    if saved.activity != before.activity || !nbt.contains_key(&key("active_state")) {
        mek_world::write_activity(nbt, saved.activity);
    }
}

/// A machine's chemical tanks, as `chemical_tanks` holds them: the infuser's
/// infusion tank (1000 mB), the separator's left and right (2400 each).
pub(super) fn read_chemicals(nbt: &Compound, kind: MachineKind, registry: &ChemicalRegistry) -> Vec<ChemicalTank> {
    let capacities: &[i64] = match kind {
        MachineKind::MetallurgicInfuser => &[INFUSE_TANK],
        MachineKind::ElectrolyticSeparator => &[separator::MAX_GAS, separator::MAX_GAS],
        MachineKind::ChemicalOxidizer => &[chemical_machine::OXIDIZER_TANK],
        k if k.advanced() => &[advanced::MAX_GAS],
        MachineKind::ChemicalInfuser => &[chemical_machine::INFUSER_TANK; 3],
        k if k.ore_chain() => super::mek_ore_machines::capacities(k),
        _ => &[],
    };
    mek_chemicals::read_tanks(nbt, capacities.len(), registry)
        .into_iter()
        .zip(capacities)
        .map(|(stored, &capacity)| ChemicalTank::with(capacity, stored))
        .collect()
}

pub(super) fn write_chemicals(nbt: &mut Compound, tanks: &[ChemicalTank]) {
    let stacks: Vec<ChemicalStack> = tanks.iter().map(|t| t.stored.clone()).collect();
    mek_chemicals::write_tanks(nbt, &stacks);
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

/// The machines this module runs: not the Combiner and the Precision Sawmill,
/// which `mek_factories` runs with the factories.
pub(super) fn kind_of(block: &str) -> Option<MachineKind> {
    MachineKind::of_block(block).filter(|k| !k.processes_like_a_factory())
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
        activity: Activity::default(),
        dumps: [GasMode::Idle; 2],
        used_so_far: 0,
    };
    let mut nbt = Compound::new();
    write_sides(&mut nbt, &saved);
    (kind.id().to_owned(), nbt)
}

/// `native_block_entity_container`: Mekanism's slots, then the upgrade
/// component's input and output, in a 3x3 menu whose other slots the screen
/// leaves out. Container faces are a block's, not a machine's facing: every
/// face reaches the input, energy and output slots (and the infuser's
/// infusion slot, the separator's fluid slot); hoppers put into the first
/// ones, as `slot_accepts` lets them, and take from the outputs.
pub(super) fn container(kind: MachineKind) -> ModContainer {
    let s = kind.slots();
    if kind.ore_chain() {
        let (mut inputs, outputs) = super::mek_ore_machines::hopper_slots(kind);
        let mut faces = inputs.clone();
        faces.extend(&outputs);
        faces.sort_unstable();
        faces.dedup();
        inputs.dedup();
        return ModContainer { slots: 9, faces: std::array::from_fn(|_| faces.clone()), inputs, outputs, menu: Some("minecraft:generic_3x3".into()) };
    }
    let mut inputs = vec![s.input, s.energy];
    // Chemical item slots: the separator's outputs, the oxidizer's output and
    // the chemical infuser's three.
    let chemical_out = kind.separates() || kind.oxidizes() || kind.infuses_chemicals();
    if chemical_out {
        // Its output slots take chemical items, which leave by the item
        // ejector once full (`ChemicalInventorySlot.drain`'s extract predicate).
        inputs.push(s.output);
        inputs.extend(s.output2);
    }
    inputs.extend(s.infusion);
    let mut outputs = vec![s.output];
    outputs.extend(s.output2);
    let mut faces = inputs.clone();
    faces.extend(&outputs);
    faces.sort_unstable();
    faces.dedup();
    if chemical_out {
        outputs.clear();
    }
    ModContainer { slots: 9, faces: std::array::from_fn(|_| faces.clone()), inputs, outputs, menu: Some("minecraft:generic_3x3".into()) }
}

/// `native_block_entity_slot_accepts`: the input slot takes what a recipe
/// takes (`InputInventorySlot`'s `containsRecipe`; the separator's fluid slot
/// a bucket of a fluid it separates, `FluidInventorySlot.fill`), the energy
/// slot what converts to energy (`EnergyInventorySlot.fillOrConvert`), the
/// infusion slot what converts to an infuse type a recipe uses, the upgrade
/// slot the upgrades the machine takes; nothing goes into an output slot (the
/// separator's take chemical items, which aren't ported). Before a world has
/// read its recipes (a LAN guest's prediction), inputs pass.
pub(super) fn slot_accepts(kind: MachineKind, slot: usize, stack: &ItemStack, accepts: &mut bool) {
    let s = kind.slots();
    let item = &*stack.item;
    let known = |pick: &dyn Fn(&Takes) -> bool| TAKES.read().ok().and_then(|t| t.as_ref().map(pick)).unwrap_or(true);
    let chemical_item = super::mek_tanks::tier_of(item).is_some() && stack.count == 1;
    let ok = if kind.ore_chain() && slot != s.energy && slot != s.upgrade {
        super::mek_ore_machines::slot_takes(kind, slot, stack)
    } else if kind.infuses_chemicals() && (slot == s.input || Some(slot) == s.infusion) {
        // `ChemicalInventorySlot.fill`: a chemical item holding a chemical, alone.
        chemical_item && super::mek_tanks::item_holds(stack)
    } else if (kind.infuses_chemicals() || kind.oxidizes()) && slot == s.output {
        // `ChemicalInventorySlot.drain`: a chemical item, alone.
        chemical_item
    } else if slot == s.input && kind.separates() {
        known(&|t| {
            horizon_world::fluid_stack::bucket_fluid(item)
                .is_some_and(|f| t.separating.iter().any(|i| i.ingredient.test(&FluidStack::new(f, 1000))))
        })
    } else if slot == s.input {
        known(&|t| t.inputs.get(&kind).is_some_and(|i| i.contains(item)))
    } else if Some(slot) == s.infusion && kind.advanced() {
        // `ChemicalInventorySlot.fillOrConvert`: a chemical item holding a chemical, or what converts.
        let chemical_item = super::mek_tanks::tier_of(item).is_some() && stack.count == 1 && super::mek_tanks::item_holds(stack);
        chemical_item || known(&|t| t.extras.get(&kind).is_some_and(|e| e.contains(item)))
    } else if Some(slot) == s.infusion {
        // A chemical tank item holding a chemical fills the tank (`fillTankOrConvert`).
        let chemical_item = super::mek_tanks::tier_of(item).is_some() && stack.count == 1 && super::mek_tanks::item_holds(stack);
        chemical_item || known(&|t| t.infusion.contains(item))
    } else if kind.separates() && (slot == s.output || Some(slot) == s.output2) {
        // `ChemicalInventorySlot.drain`: a chemical item, alone.
        super::mek_tanks::tier_of(item).is_some() && stack.count == 1
    } else if slot == s.energy {
        known(&|t| t.energy.contains(item)) || mekanism_energy::tier::CubeTier::of_block(item).is_some()
    } else if slot == s.upgrade {
        Upgrade::of_item(item).is_some_and(|u| kind.upgrades().contains(&u))
    } else {
        false
    };
    if !ok {
        *accepts = false;
    }
}

pub(super) fn stack_in(entity: &ModBlockEntity, slot: usize) -> Option<&ItemStack> {
    entity.items.get(slot).and_then(Option::as_ref).filter(|s| s.count > 0)
}

pub(super) fn shrink(entity: &mut ModBlockEntity, slot: usize, by: u32) {
    if let Some(cell) = entity.items.get_mut(slot) {
        if let Some(stack) = cell.as_mut() {
            stack.count = stack.count.saturating_sub(by);
        }
        if cell.as_ref().is_some_and(|s| s.count == 0) {
            *cell = None;
        }
    }
}

pub(super) fn same_kind(a: &ItemStack, b: &ItemStack) -> bool {
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
    super::mek_ore_machines::clear();
}

// ---- The tick ---------------------------------------------------------------------

/// The infuser's tank rules (`TileEntityMetallurgicInfuser.getInitialChemicalTanks`:
/// `BasicChemicalTank.createModern(1000, alwaysTrueBi, containsRecipeBA(input, type),
/// containsRecipeB, ...)` with the default attribute validator), run with `use_them`.
fn with_infuser_rules<R>(input: Option<&ItemStack>, registry: &ChemicalRegistry, use_them: impl FnOnce(&TankRules<'_>) -> R) -> R {
    let can_insert = |s: &ChemicalStack, _: Automation| infuser_takes(input, s.id(), registry);
    let valid = |s: &ChemicalStack| infuser_takes(None, s.id(), registry) && registry.passes_default(s);
    use_them(&TankRules { can_extract: &chemical::always, can_insert: &can_insert, valid: &valid })
}

/// The tank rules of a machine with an item and a chemical recipe: the
/// infuser's, or an advanced machine's (`BasicChemicalTank.createModern(210,
/// canExtract, containsRecipeBA(input, gas), containsRecipeB, ...)`: the
/// compressor's chemical leaves on its output sides, the chambers' only to
/// the machine, `notExternal`).
fn with_tank_rules<R>(kind: MachineKind, input: Option<&ItemStack>, registry: &ChemicalRegistry, use_it: impl FnOnce(&TankRules<'_>) -> R) -> R {
    if !kind.advanced() {
        return with_infuser_rules(input, registry, use_it);
    }
    let can_insert = |s: &ChemicalStack, _: Automation| takes_chemical(kind, input, s.id(), registry);
    let valid = |s: &ChemicalStack| takes_chemical(kind, None, s.id(), registry) && registry.passes_default(s);
    let can_extract: &dyn Fn(&ChemicalStack, Automation) -> bool = if kind.takes_chemical_upgrades() { &chemical::not_external } else { &chemical::always };
    use_it(&TankRules { can_extract, can_insert: &can_insert, valid: &valid })
}

thread_local! {
    /// The advanced chambers' draws (`StatUtils`' own `RandomSource`).
    static RNG: Cell<Rng> = Cell::new(Rng::new(
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0x9E37_79B9, |d| d.as_nanos() as u64),
    ));
}

fn draw_random() -> f64 {
    RNG.with(|r| {
        let mut rng = r.get();
        let v = rng.next_f64();
        r.set(rng);
        v
    })
}

/// `native_block_entity_tick` of a machine: `TileEntityMekanism.tickServer`
/// (the upgrade component, the active state's delay) and `onUpdateServer`
/// (`TileEntityConfigurableMachine`'s ejector, then the energy slot, the
/// infuser's infusion slot and the recipe, or the separator's), then the
/// menu's data slots. Redstone control is IGNORED, the default.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, kind: MachineKind) {
    let (config, _) = mek_world::energy_config();
    let dimension = game.native_dimension();
    refresh_book(game);
    let registry = mek_chemicals::registry(game);
    let slots = kind.slots();
    let multiplier = config.machines.max_upgrade_multiplier;
    let (usage, _) = config.machines.of(kind);
    let before = read(&entity.nbt, kind);
    let mut saved = before.clone();
    saved.facing = game.native_block_property(pos, "facing").and_then(|f| Direction::from_name(&f)).unwrap_or(Direction::North);
    let tanks_before = read_chemicals(&entity.nbt, kind, &registry);
    let mut tanks = tanks_before.clone();
    let id = live_key(dimension, pos);
    let mut live = LIVE.with(|l| l.borrow_mut().remove(&id)).unwrap_or_default();

    // The upgrade component: an upgrade in its slot goes in after a second.
    let upgrade_stack = stack_in(entity, slots.upgrade).map(|s| (s.item.to_string(), s.count));
    if upgrade_stack != live.upgrade_stack {
        live.upgrade.check = true;
    }
    let offered = upgrade_stack.as_ref().and_then(|(item, count)| Some((Upgrade::of_item(item)?, *count)));
    let taken = live.upgrade.tick(&mut saved.upgrades, kind.upgrades(), offered);
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

    // The ejector: the separator's chemicals every tick (CHEMICAL comes before
    // ITEM), then what the output slot holds, out of the faces whose item type
    // outputs, every half second.
    if kind.separates() && saved.sides.chemicals_eject
        && let Some(sides) = saved.sides.chemicals
    {
        eject_chemicals(game, pos, saved.facing, &sides, &mut tanks, saved.dumps, config.machines.chemical_auto_eject);
    }
    let makes_chemicals = kind.oxidizes() || kind.infuses_chemicals();
    let ejects_tank = makes_chemicals || super::mek_ore_machines::ejects_chemicals(kind);
    if ejects_tank
        && saved.sides.chemicals_eject
        && let Some(sides) = saved.sides.chemicals
    {
        super::mek_chem_machines::eject(game, pos, saved.facing, &sides, &mut tanks, config.machines.chemical_auto_eject);
    }
    if (makes_chemicals || kind.ore_chain()) && saved.sides.items_eject {
        if live.eject_delay == 0 {
            if kind.ore_chain() {
                super::mek_ore_machines::eject_items(game, pos, entity, kind, &saved.sides.items, saved.facing, &tanks, &registry, &config);
            } else {
                super::mek_chem_machines::eject_items(game, pos, entity, kind, &saved.sides.items, saved.facing, &tanks, &registry, &config);
            }
            live.eject_delay = machine::EJECT_DELAY;
        } else {
            live.eject_delay -= 1;
        }
    }
    if saved.sides.items_eject && !kind.separates() && !makes_chemicals && !kind.ore_chain() {
        if live.eject_delay == 0 {
            eject_items(game, pos, entity, slots.output, &machine::eject_faces(&saved.sides.items, saved.facing));
            live.eject_delay = machine::EJECT_DELAY;
        } else {
            live.eject_delay -= 1;
        }
    }
    if saved.sides.items_eject && kind.separates() {
        if live.eject_delay == 0 {
            eject_separator_items(game, pos, entity, &saved, &tanks, &registry, &config);
            live.eject_delay = machine::EJECT_DELAY;
        } else {
            live.eject_delay -= 1;
        }
    }

    let mut store = store(&saved, &config, kind);
    // The energy slot (`fillContainerOrConvert`): an energy cube's item gives
    // what it can, else an item that converts to energy, when all of it fits.
    let mut item = stack_in(entity, slots.energy).cloned();
    if let Some(cube) = item.as_mut()
        && store.needed() > 0
        && let Some(taken) = mek_world::drain_energy_item(cube, store.needed()).filter(|t| *t > 0)
    {
        store.insert(taken, false, Automation::Internal);
        if let Some(cell) = entity.items.get_mut(slots.energy) {
            *cell = item;
        }
    } else if let Some(stack) = stack_in(entity, slots.energy).cloned()
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

    if kind.separates() {
        let used = tick_separator(game, entity, &saved, &mut tanks, &mut store, &mut live, &registry, &config);
        finish_tick(game, pos, entity, kind, saved, &before, &tanks, &tanks_before, store, used, &registry, id, live);
        return;
    }
    if makes_chemicals || kind.ore_chain() {
        let made = if kind.ore_chain() {
            super::mek_ore_machines::tick(game, entity, kind, &mut saved, &mut tanks, &mut store, &mut live.separating, &registry, &config)
        } else {
            super::mek_chem_machines::tick(entity, kind, &mut saved, &mut tanks, &mut store, &mut live.separating, &registry, &config)
        };
        saved.energy = store.stored;
        if let Some(active) = made.active {
            saved.activity.set(active, false, config.deactivation_delay);
        }
        write(&mut entity.nbt, &saved, &before, kind);
        if tanks != tanks_before {
            write_chemicals(&mut entity.nbt, &tanks);
        }
        let input = mek_world::take_received(pos);
        let mut view = machine_view(&saved, &store, made.per_tick, made.required, &tanks, &registry, input, made.warnings);
        view.upgrade_ticks = live.upgrade.ticks;
        let mut data = view.pack();
        data.extend(made.extra);
        entity.data = data;
        LIVE.with(|l| l.borrow_mut().insert(id, live));
        return;
    }

    // The infusion slot: a chemical item empties into the tank, else an item
    // that converts to an infuse type a recipe takes
    // (`ChemicalInventorySlot.fillTankOrConvert`: all of it fits, or nothing).
    let mut filled = false;
    if let Some(infusion) = slots.infusion
        && let Some(tank) = tanks.first_mut()
        && tank.needed() > 0
    {
        let input = stack_in(entity, slots.input).cloned();
        filled = with_tank_rules(kind, input.as_ref(), &registry, |rules| {
            super::mek_tanks::with_slot_item(entity, infusion, &registry, &config, |item| {
                chemical_tank::fill_from_item(item, &mut OwnTank { tank, rules, block: None })
            })
        }) == Some(true);
    }
    if !filled
        && let Some(infusion) = slots.infusion
        && let Some(stack) = stack_in(entity, infusion).cloned()
        && let Some(tank) = tanks.first_mut()
        && tank.needed() > 0
    {
        let found = BOOK.with(|b| b.borrow().conversions.iter().find(|c| c.input.takes(&stack)).cloned());
        let input = stack_in(entity, slots.input).cloned();
        if let Some(conversion) = found
            && stack.count >= conversion.input.count
        {
            let fits = with_tank_rules(kind, input.as_ref(), &registry, |rules| {
                if tank.insert(&conversion.output, true, Automation::Internal, rules).is_empty() {
                    tank.insert(&conversion.output, false, Automation::Internal, rules);
                    true
                } else {
                    false
                }
            });
            if fits {
                shrink(entity, infusion, conversion.input.count);
            }
        }
    }

    // The recipe (`RecipeCacheLookupMonitor.updateAndProcess`).
    let required = saved.upgrades.ticks(machine::BASE_TICKS, multiplier);
    let per_tick = saved.upgrades.energy_per_tick(usage, multiplier);
    let baseline = if kind.infuses() || kind.advanced() { saved.upgrades.operations_per_tick(machine::BASE_TICKS, multiplier) } else { 1 };
    let input = stack_in(entity, slots.input).cloned();
    let held = tanks.first().and_then(|t| t.stored.id().map(str::to_owned));
    let edition = EDITION.get();
    let cached = live.recipe.as_ref().filter(|(e, _)| *e == edition).map(|(_, r)| r.clone());
    let valid = |r: &Recipe| input.as_ref().is_some_and(|i| r.input.takes(i)) && r.chemical_fits(held.as_deref(), &registry);
    let mut warnings = 0;
    let recipe = match cached {
        Some(r) if valid(r.as_ref()) => Some(r),
        old => match input.as_ref().and_then(|i| find(kind, i, held.as_deref(), &registry)) {
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
        let count = input.as_ref().filter(|i| recipe.input.takes(i)).map_or(0, |i| i.count);
        let mut inputs = Inputs::of(count, recipe.input.count);
        if let Some(ingredient) = &recipe.chemical {
            let tank = tanks.first();
            let chemical = match tank {
                Some(t) if recipe.chemical_fits(t.stored.id(), &registry) => {
                    Inputs::of(t.stored().clamp(0, i64::from(u32::MAX)) as u32, ingredient.amount.clamp(0, i64::from(u32::MAX)) as u32)
                }
                _ => Inputs::Mismatched,
            };
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
        let ran = if kind.advanced() && recipe.per_tick {
            // `ItemStackConstantChemicalToObjectCachedRecipe`: the chemical a tick.
            let usage = ChemicalUse::of(&saved.upgrades, kind.takes_chemical_upgrades(), config.machines.randomized_consumption, multiplier);
            let draw = usage.draw(saved.used_so_far, required, saved.progress, draw_random());
            let amount = recipe.chemical.as_ref().map_or(0, |c| c.amount);
            let stored = tanks.first().filter(|t| recipe.chemical_fits(t.stored.id(), &registry)).map_or(0, ChemicalTank::stored);
            let item = Inputs::of(count, recipe.input.count);
            let ran = advanced::process(&mut saved.progress, &mut saved.used_so_far, required, &mut store, per_tick, baseline, item, (stored, amount), draw, room);
            if ran.chemical > 0
                && let Some(tank) = tanks.first_mut()
            {
                tank.shrink_stack(ran.chemical, false);
            }
            if ran.finished {
                shrink(entity, slots.input, recipe.input.count * ran.operations);
                let made = recipe.output.count * ran.operations;
                match entity.items.get_mut(slots.output) {
                    Some(Some(out)) => out.count += made,
                    Some(cell) => *cell = Some(ItemStack { count: made, ..recipe.output.clone() }),
                    None => {}
                }
            }
            machine::Ran { operations: ran.operations, finished: false, energy: ran.energy }
        } else {
            machine::process(&mut saved.progress, required, &mut store, per_tick, baseline, inputs, room, true)
        };
        if ran.finished {
            shrink(entity, slots.input, recipe.input.count * ran.operations);
            // `infusionInputHandler.use`: the tank's own extract.
            if let Some(ingredient) = &recipe.chemical
                && let Some(tank) = tanks.first_mut()
            {
                let amount = ingredient.amount.saturating_mul(i64::from(ran.operations));
                with_tank_rules(kind, input.as_ref(), &registry, |rules| tank.extract(amount, false, Automation::Internal, rules));
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
    write(&mut entity.nbt, &saved, &before, kind);
    if tanks != tanks_before {
        write_chemicals(&mut entity.nbt, &tanks);
    }
    let input_joules = mek_world::take_received(pos);
    let mut view = machine_view(&saved, &store, per_tick, required, &tanks, &registry, input_joules, warnings);
    view.upgrade_ticks = live.upgrade.ticks;
    entity.data = view.pack();
    LIVE.with(|l| l.borrow_mut().insert(id, live));
}

/// A machine's view on its menu's data slots.
#[allow(clippy::too_many_arguments)]
fn machine_view(saved: &Saved, store: &EnergyContainer, per_tick: u64, required: u32, tanks: &[ChemicalTank], registry: &ChemicalRegistry, input: u64, warnings: i32) -> MachineView {
    let infusion = tanks.first().filter(|_| saved.sides.fluids.is_none());
    MachineView {
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
        upgrade_ticks: 0,
        infuse: infusion.map_or(0, |t| registry.number(t.stored.id())),
        infused: infusion.map_or(0, |t| t.stored().max(0) as u64),
        input,
        warnings,
    }
}

/// What the separator's tick did, for its view.
struct Separated {
    per_tick: u64,
    energy_used: u64,
    warnings: i32,
    active: bool,
    fluid: FluidStack,
}

/// `TileEntityElectrolyticSeparator.onUpdateServer` after the ejector and the
/// energy slot: the fluid slot fills the tank (`FluidInventorySlot.fillTank`),
/// the recipe runs (`OneInputCachedRecipe.separating`), then each output tank
/// dumps by its mode (`handleTank`). Its output slots hold chemical items,
/// which aren't ported: they drain nothing.
#[allow(clippy::too_many_arguments)]
fn tick_separator(
    game: &mut Game<'_>,
    entity: &mut ModBlockEntity,
    saved: &Saved,
    tanks: &mut [ChemicalTank],
    store: &mut EnergyContainer,
    live: &mut Live,
    registry: &ChemicalRegistry,
    config: &EnergyConfig,
) -> Separated {
    let slots = MachineKind::ElectrolyticSeparator.slots();
    let mut fluid = mek_world::saved_tank(&entity.nbt);
    // The fluid slot: a container of a fluid it separates empties into the tank,
    // a bucket only whole, when the tank is empty or holds the same fluid.
    if let Some(stack) = stack_in(entity, slots.input).cloned()
        && stack.count == 1
        && fluid.amount() < separator::MAX_FLUID
        && let Some(held) = game.native_item_fluid(&stack)
        && let Some(held_stack) = held.fluid.as_deref().and_then(|f| FluidStack::of(f, held.amount))
        && separates_fluid(&held_stack)
        && (fluid.is_empty() || FluidStack::is_same_fluid_same_components(&fluid, &held_stack))
    {
        let room = separator::MAX_FLUID - fluid.amount().max(0);
        let whole = horizon_world::fluid_stack::bucket_fluid(&stack.item).is_some();
        let n = if whole { if held.amount <= room { held.amount } else { 0 } } else { held.amount.min(room) };
        if n > 0
            && let Some((_, took, after)) = game.native_item_fluid_drain(&stack, n)
            && took > 0
        {
            fluid = if fluid.is_empty() { held_stack.copy_with_amount(took) } else { fluid.copy_with_amount(fluid.amount() + took) };
            if let Some(cell) = entity.items.get_mut(slots.input) {
                *cell = Some(after).filter(|s| s.count > 0);
            }
        }
    }
    // The left and right output slots (`leftOutputSlot.drainTank`, `rightOutputSlot.drainTank`).
    let rules = separator::output_rules();
    for (slot, i) in [(slots.output, 0), (slots.output2.unwrap_or(slots.output), 1)] {
        if let Some(tank) = tanks.get_mut(i) {
            super::mek_tanks::with_slot_item(entity, slot, registry, config, |item| {
                chemical_tank::drain_into_item(item, &mut OwnTank { tank, rules: &rules, block: None })
            });
        }
    }
    let (baseline, dump_rate) = separator::speed(saved.upgrades.get(Upgrade::Speed));
    let ratio = config.machines.dump_excess_keep_ratio;
    let density = registry.get("mekanism:hydrogen").and_then(|c| c.fuel).map_or(0, |f| f.energy_density());
    let recipe = find_separating(&fluid);
    live.separating = recipe.as_ref().map(|r| r.id.clone());
    let per_tick = separator::energy_per_tick(density, recipe.as_ref().map_or(1, |r| r.energy_multiplier));
    let mut out = Separated { per_tick, energy_used: 0, warnings: 0, active: false, fluid: FluidStack::EMPTY };
    if let Some(recipe) = &recipe {
        let can = separator::can_function((saved.dumps[0], &tanks[0]), (saved.dumps[1], &tanks[1]), ratio);
        let separation = Separation { input: i64::from(recipe.input.amount), left: recipe.left.clone(), right: recipe.right.clone() };
        let (left, right) = tanks.split_at_mut(1);
        let (ran, used) = separator::process(&separation, i64::from(fluid.amount()), &mut left[0], &mut right[0], store, per_tick, baseline, can);
        if used > 0 {
            let rest = fluid.amount() - used.min(i64::from(i32::MAX)) as i32;
            fluid = if rest > 0 { fluid.copy_with_amount(rest) } else { FluidStack::EMPTY };
        }
        if ran.short_energy {
            out.warnings |= menu::WARN_ENERGY;
        }
        if ran.left_full || ran.right_full {
            out.warnings |= menu::WARN_OUTPUT;
        }
        if ran.short_input {
            out.warnings |= menu::WARN_NO_RECIPE;
        }
        out.energy_used = ran.energy;
        out.active = ran.operations > 0;
    } else if !fluid.is_empty() {
        out.warnings |= menu::WARN_NO_RECIPE;
    }
    for (tank, mode) in tanks.iter_mut().zip(saved.dumps) {
        separator::handle_tank(tank, mode, dump_rate, config.machines.chemical_auto_eject, ratio);
    }
    out.fluid = fluid;
    out
}

/// The separator's tick from its recipe on: the store, the activity, the saved
/// compound and its view.
#[allow(clippy::too_many_arguments)]
fn finish_tick(
    _game: &mut Game<'_>,
    pos: [i32; 3],
    entity: &mut ModBlockEntity,
    kind: MachineKind,
    mut saved: Saved,
    before: &Saved,
    tanks: &[ChemicalTank],
    tanks_before: &[ChemicalTank],
    store: EnergyContainer,
    separated: Separated,
    registry: &ChemicalRegistry,
    id: (String, [i32; 3]),
    live: Live,
) {
    let (config, _) = mek_world::energy_config();
    saved.energy = store.stored;
    saved.activity.set(separated.active, false, config.deactivation_delay);
    write(&mut entity.nbt, &saved, before, kind);
    if tanks != tanks_before {
        write_chemicals(&mut entity.nbt, tanks);
    }
    if separated.fluid != mek_world::saved_tank(&entity.nbt) {
        mek_world::write_tank(&mut entity.nbt, &separated.fluid);
    }
    let input = mek_world::take_received(pos);
    let mut data = machine_view(&saved, &store, separated.per_tick, 0, tanks, registry, input, separated.warnings);
    data.upgrade_ticks = live.upgrade.ticks;
    let mut data = data.pack();
    data.extend(separator_view(&saved, tanks, &separated.fluid, registry, separated.energy_used).pack());
    entity.data = data;
    LIVE.with(|l| l.borrow_mut().insert(id, live));
}

fn separator_view(saved: &Saved, tanks: &[ChemicalTank], fluid: &FluidStack, registry: &ChemicalRegistry, energy_used: u64) -> SeparatorView {
    let tank = |i: usize| tanks.get(i).map_or((0, 0), |t| (registry.number(t.stored.id()), t.stored().max(0) as u64));
    let ((left, left_amount), (right, right_amount)) = (tank(0), tank(1));
    SeparatorView {
        fluid: if fluid.is_empty() { 0 } else { fluid.fluid().id() },
        fluid_amount: fluid.amount().max(0) as u32,
        left,
        left_amount,
        right,
        right_amount,
        dump_left: saved.dumps[0].id(),
        dump_right: saved.dumps[1].id(),
        energy_used,
    }
}

/// The separator's chemical ejector (`TileComponentEjector.eject` with
/// `ChemicalUtil.emit`): each output tank that isn't dumping
/// (`setCanTankEject`) gives up to `rate` a tick, split evenly among the
/// chemical handlers on the faces its config type (`OUTPUT_1` the left tank,
/// `OUTPUT_2` the right) is on, as the separator itself (`INTERNAL`).
fn eject_chemicals(game: &mut Game<'_>, pos: [i32; 3], facing: Direction, sides: &[DataType; 6], tanks: &mut [ChemicalTank], dumps: [GasMode; 2], rate: i64) {
    let rules = separator::output_rules();
    for (i, data) in [(0, DataType::Output1), (1, DataType::Output2)] {
        let Some(tank) = tanks.get_mut(i) else {
            continue;
        };
        if tank.is_empty() || dumps[i] == GasMode::Dumping {
            continue;
        }
        let faces: Vec<Direction> = Direction::ALL
            .into_iter()
            .filter(|&d| sides[RelativeSide::from_directions(facing, d).index()] == data)
            .collect();
        if faces.is_empty() {
            continue;
        }
        let offered = tank.extract(rate, true, Automation::Internal, &rules);
        let sent = mek_chemicals::emit(game, pos, &faces, &offered);
        if sent > 0 {
            tank.extract(sent, false, Automation::Internal, &rules);
        }
    }
}

/// `TileComponentEjector.outputItems` of the separator: its left output
/// slot's item to the faces whose item type is `OUTPUT_1` (or both), the
/// right's to `OUTPUT_2`'s, once it may leave (`EXTERNAL`): when it takes no
/// more of its tank (`getDrainInsertPredicate` negated).
fn eject_separator_items(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, saved: &Saved, tanks: &[ChemicalTank], registry: &ChemicalRegistry, config: &EnergyConfig) {
    let slots = MachineKind::ElectrolyticSeparator.slots();
    let rules = separator::output_rules();
    for (slot, i, data) in [(slots.output, 0, DataType::Output1), (slots.output2.unwrap_or(slots.output), 1, DataType::Output2)] {
        let faces: Vec<Direction> = Direction::ALL
            .into_iter()
            .filter(|&d| {
                let t = saved.sides.items[RelativeSide::from_directions(saved.facing, d).index()];
                t == data || t == DataType::InputOutput
            })
            .collect();
        let Some(mut tank) = tanks.get(i).cloned() else {
            continue;
        };
        let leaves = super::mek_tanks::with_slot_item(entity, slot, registry, config, |item| {
            !chemical_tank::drain_accepts(item, &OwnTank { tank: &mut tank, rules: &rules, block: None })
        });
        if leaves == Some(true) {
            eject_items(game, pos, entity, slot, &faces);
        }
    }
}

/// `TileComponentEjector.outputItems`: the output slot's stack goes to each
/// face in turn, as much as fits (`native_insert_item` through the
/// neighbour's touching face), until none is left.
pub(super) fn eject_items(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, slot: usize, faces: &[Direction]) {
    for &face in faces {
        let Some(stack) = stack_in(entity, slot).cloned() else {
            return;
        };
        // A logistical transporter takes it on a path (its `CursedTransporterItemHandler`), any other block as a hopper's target.
        let at = mek_world::beside(pos, face);
        let rest = match super::mek_transporters::insert_from(game, at, pos, stack.clone()) {
            Some(rest) => Some(rest),
            None => game.native_insert_item(at, mek_world::engine_face(face.opposite()), stack),
        };
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

// ---- Chemicals and fluids from outside ---------------------------------------------

/// The chemical handler of a machine through `face` (`Capabilities.CHEMICAL`
/// over `ConfigChemicalTankHolder` and `ProxyChemicalHandler`): the tanks its
/// chemical config puts on that side, taking in on an input side, giving out on
/// an output side (`ChemicalSlotInfo`'s `canInput`, `canOutput`), as another
/// block (`EXTERNAL`); the null side reads every tank and changes nothing. None
/// for a side whose type is `NONE` and for a machine without chemicals. What
/// `use_it` changed is saved.
pub(super) fn chemical_handler<R>(
    entity: &mut ModBlockEntity,
    face: Option<Direction>,
    registry: &ChemicalRegistry,
    use_it: impl FnOnce(&mut dyn ChemicalHandler) -> R,
) -> Option<R> {
    let kind = kind_of(&entity.id)?;
    let saved = read(&entity.nbt, kind);
    let config = saved.sides.chemicals?;
    let mut tanks = read_chemicals(&entity.nbt, kind, registry);
    let (shown, can_insert, can_extract): (Vec<usize>, bool, bool) = match face {
        None => ((0..tanks.len()).collect(), false, false),
        Some(f) => match (kind, config[RelativeSide::from_directions(saved.facing, f).index()]) {
            (MachineKind::MetallurgicInfuser, DataType::Input) => (vec![0], true, false),
            (MachineKind::MetallurgicInfuser, DataType::Output) => (vec![0], false, true),
            (MachineKind::MetallurgicInfuser, DataType::InputOutput) => (vec![0], true, true),
            (MachineKind::ElectrolyticSeparator, DataType::Output1) => (vec![0], false, true),
            (MachineKind::ElectrolyticSeparator, DataType::Output2) => (vec![1], false, true),
            (MachineKind::ChemicalOxidizer, DataType::Output) => (vec![0], false, true),
            (MachineKind::OsmiumCompressor, DataType::Input) => (vec![0], true, false),
            (MachineKind::OsmiumCompressor, DataType::Output) => (vec![0], false, true),
            (MachineKind::OsmiumCompressor, DataType::InputOutput) => (vec![0], true, true),
            (MachineKind::PurificationChamber | MachineKind::ChemicalInjectionChamber, DataType::Input) => (vec![0], true, false),
            (MachineKind::ChemicalInfuser, DataType::Input1) => (vec![0], true, false),
            (MachineKind::ChemicalInfuser, DataType::Input2) => (vec![1], true, false),
            (MachineKind::ChemicalInfuser, DataType::Output) => (vec![2], false, true),
            (MachineKind::ChemicalInfuser, DataType::InputOutput) => (vec![0, 1, 2], true, true),
            (k, data) if k.ore_chain() => super::mek_ore_machines::chemical_side(k, data)?,
            _ => return None,
        },
    };
    let before = tanks.clone();
    let input = stack_in(entity, kind.slots().input).cloned();
    let answer = if kind.infuses_chemicals() {
        // Each tank's own rules: what goes in the left or right depends on the other.
        let (left, right) = (tanks[0].stored.clone(), tanks[1].stored.clone());
        super::mek_chem_machines::with_infuser_rules(&left, &right, registry, |l, r, c| {
            let all = [l, r, c];
            let list = tanks.iter_mut().enumerate().filter(|(i, _)| shown.contains(i)).map(|(i, t)| (t, all[i])).collect();
            let mut handler = SidedTanks { tanks: list, side: face.is_some(), can_insert, can_extract };
            use_it(&mut handler)
        })
    } else if kind.ore_chain() {
        let fluid = mek_world::saved_tank(&entity.nbt);
        super::mek_ore_machines::with_rules(kind, input.as_ref(), &fluid, registry, |all| {
            let list = tanks.iter_mut().enumerate().filter(|(i, _)| shown.contains(i)).map(|(i, t)| (t, all[i])).collect();
            let mut handler = SidedTanks { tanks: list, side: face.is_some(), can_insert, can_extract };
            use_it(&mut handler)
        })
    } else {
        let run = |rules: &TankRules<'_>| {
            let list = tanks.iter_mut().enumerate().filter(|(i, _)| shown.contains(i)).map(|(_, t)| (t, rules)).collect();
            let mut handler = SidedTanks { tanks: list, side: face.is_some(), can_insert, can_extract };
            use_it(&mut handler)
        };
        if kind.infuses() || kind.advanced() { with_tank_rules(kind, input.as_ref(), registry, |rules| run(rules)) } else { run(&separator::output_rules()) }
    };
    if tanks != before {
        write_chemicals(&mut entity.nbt, &tanks);
    }
    Some(answer)
}

/// The fluid tank through `face` of a machine with one
/// (`FluidTankHelper.forSideWithConfig`): the separator's and the washer's on
/// their fluid config's `INPUT` sides (every side by default). None for any
/// other machine or a closed side; else its kind, its saved state and whether
/// that side is a real one (not the null side, which reads and changes nothing).
fn fluid_side(entity: &ModBlockEntity, face: Option<Face>) -> Option<(MachineKind, Saved, bool)> {
    let kind = kind_of(&entity.id).filter(|k| k.separates() || k.washes())?;
    let saved = read(&entity.nbt, kind);
    let fluids = saved.sides.fluids?;
    match face {
        None => Some((kind, saved, false)),
        Some(f) => {
            let data = fluids[RelativeSide::from_directions(saved.facing, mek_world::mek_direction(f)).index()];
            (data == DataType::Input).then_some((kind, saved, true))
        }
    }
}

fn fluid_capacity(kind: MachineKind) -> i32 {
    if kind.separates() { separator::MAX_FLUID } else { super::mek_ore_machines::fluid_capacity(kind).unwrap_or(0) }
}

/// `native_fluid_handler_tanks` of a machine's fluid tank: the separator's
/// (24 000 mB) and the washer's (10 000), which no other block drains
/// (`BasicFluidTank.input`: `notExternal`).
pub(super) fn fluid_tanks(entity: &ModBlockEntity, face: Option<Face>, tanks: &mut Option<Vec<NativeFluidTankInfo>>) -> bool {
    let Some((kind, _, _)) = fluid_side(entity, face) else {
        return false;
    };
    *tanks = Some(vec![NativeFluidTankInfo { stack: mek_world::saved_tank(&entity.nbt), capacity: fluid_capacity(kind), drainable: false }]);
    true
}

/// `native_fluid_handler_fill` of a machine's fluid tank: a fluid a recipe
/// takes (the separator's `containsRecipe`, the washer's `containsRecipeAB`
/// with the slurry it holds), into an
/// empty tank or one of the same fluid, what fits; the null side's proxy takes
/// nothing.
pub(super) fn fluid_fill(entity: &mut ModBlockEntity, face: Option<Face>, resource: &FluidStack, simulate: bool, filled: &mut Option<i32>) -> bool {
    let Some((kind, _, side)) = fluid_side(entity, face) else {
        return false;
    };
    let tank = mek_world::saved_tank(&entity.nbt);
    let same = tank.is_empty() || FluidStack::is_same_fluid_same_components(&tank, resource);
    let valid = if kind.separates() {
        separates_fluid(resource)
    } else {
        let registry = mek_chemicals::loaded();
        let slurry = registry.as_deref().and_then(|r| read_chemicals(&entity.nbt, kind, r).into_iter().next()).map_or(ChemicalStack::EMPTY, |t| t.stored);
        super::mek_ore_machines::fluid_fill_allowed(kind, resource, &slurry, registry.as_deref())
    };
    let took = if side && same && !resource.is_empty() && valid { resource.amount().min(fluid_capacity(kind) - tank.amount().max(0)).max(0) } else { 0 };
    if took > 0 && !simulate {
        let after = if tank.is_empty() { resource.copy_with_amount(took) } else { tank.copy_with_amount(tank.amount() + took) };
        mek_world::write_tank(&mut entity.nbt, &after);
    }
    *filled = Some(took);
    true
}

/// `native_fluid_handler_drain` of a machine's fluid tank: nothing leaves the
/// separator's or the washer's (`notExternal`).
pub(super) fn fluid_drain(entity: &ModBlockEntity, face: Option<Face>, drained: &mut Option<FluidStack>) -> bool {
    if fluid_side(entity, face).is_none() {
        return false;
    }
    *drained = Some(FluidStack::EMPTY);
    true
}

// ---- Buttons and probes ---------------------------------------------------------------

/// `native_menu_button` of a machine: its side configuration (energy, items,
/// chemicals), the upgrade window's uninstall, which puts the upgrades in the
/// upgrade output slot as far as they fit (`TileComponentUpgrade.removeUpgrade`),
/// the infuser's dump (`IHasDumpButton.dump`: its tank emptied) and the
/// separator's dump modes (`nextMode`).
pub(super) fn button(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, id: i32) {
    let (Some(kind), Some(button)) = (kind_of(&entity.id), MachineButton::from_id(id)) else {
        return;
    };
    let before = read(&entity.nbt, kind);
    let mut saved = before.clone();
    let mut dumped = false;
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
        MachineButton::Dump if kind.infuses() => dumped = entity.nbt.remove(&key("chemical_tanks")).is_some(),
        MachineButton::NextMode(tank) if kind.separates() => {
            let t = usize::from(tank.min(1));
            saved.dumps[t] = saved.dumps[t].next();
        }
        _ => {}
    }
    if saved != before || dumped {
        write(&mut entity.nbt, &saved, &before, kind);
        let mut view = MachineView::unpack(&entity.data).unwrap_or_default();
        let extra = SeparatorView::unpack(&entity.data).filter(|_| kind.separates());
        let tail = (!kind.separates()).then(|| entity.data.get(menu::MACHINE_SLOTS..).map(<[i32]>::to_vec)).flatten();
        view.energy = saved.sides.energy;
        view.items = saved.sides.items;
        view.chemicals = saved.sides.chemicals;
        view.items_eject = saved.sides.items_eject;
        view.upgrades = saved.upgrades;
        if dumped {
            view.infuse = 0;
            view.infused = 0;
        }
        let mut data = view.pack();
        if let Some(mut extra) = extra {
            extra.dump_left = saved.dumps[0].id();
            extra.dump_right = saved.dumps[1].id();
            data.extend(extra.pack());
        }
        data.extend(tail.unwrap_or_default());
        entity.data = data;
    }
}

/// What `/mek probe` answers for a machine.
pub(super) fn probe(game: &Game<'_>, pos: [i32; 3], id: &str, nbt: &Compound, data: &[i32], items: &[Option<ItemStack>]) -> serde_json::Value {
    let Some(kind) = kind_of(id) else {
        return serde_json::json!({ "pos": pos, "id": id, "error": "not a machine" });
    };
    let (config, from_files) = mek_world::energy_config();
    let registry = mek_chemicals::registry(game);
    let saved = read(nbt, kind);
    let view = MachineView::unpack(data).unwrap_or_default();
    let live = LIVE.with(|l| l.borrow().get(&live_key(game.native_dimension(), pos)).cloned()).unwrap_or_default();
    let names = |sides: &[DataType; 6]| -> serde_json::Map<String, serde_json::Value> {
        RelativeSide::ALL.iter().map(|s| (s.name().to_owned(), sides[s.index()].name().into())).collect()
    };
    let slots: Vec<serde_json::Value> = items
        .iter()
        .take(kind.slots().count)
        .map(|s| {
            s.as_ref().map_or(serde_json::Value::Null, |s| {
                let held = super::mek_tanks::item_stored(s, &registry).map(|(_, c)| serde_json::json!({ "id": c.id(), "amount": c.amount() }));
                serde_json::json!({ "item": &*s.item, "count": s.count, "chemical": held })
            })
        })
        .collect();
    let tanks: Vec<serde_json::Value> = read_chemicals(nbt, kind, &registry)
        .iter()
        .map(|t| serde_json::json!({ "chemical": t.stored.id(), "amount": t.stored(), "capacity": t.capacity }))
        .collect();
    let (usage, storage) = config.machines.of(kind);
    let mut out = serde_json::json!({
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
        "recipe": live.recipe.as_ref().map(|(_, r)| r.id.clone()).or(live.separating.clone()),
        "upgrades": Upgrade::ALL.iter().filter(|u| saved.upgrades.get(**u) > 0).map(|u| (u.name().to_owned(), saved.upgrades.get(*u).into())).collect::<serde_json::Map<String, serde_json::Value>>(),
        "upgrade_ticks": live.upgrade.ticks,
        "energy_config": names(&saved.sides.energy),
        "item_config": names(&saved.sides.items),
        "item_eject": saved.sides.items_eject,
        "chemical_config": saved.sides.chemicals.as_ref().map(names),
        "chemical_eject": saved.sides.chemicals_eject,
        "fluid_config": saved.sides.fluids.as_ref().map(names),
        "tank": tanks.first().cloned(),
        "tanks": tanks,
        "slots": slots,
        "warnings": view.warnings,
        "config": { "from_files": from_files, "usage": usage, "storage": storage, "max_upgrade_multiplier": config.machines.max_upgrade_multiplier,
                    "chemical_auto_eject": config.machines.chemical_auto_eject, "dump_excess_keep_ratio": config.machines.dump_excess_keep_ratio },
        "recipes": BOOK.with(|b| {
            let b = b.borrow();
            match kind {
                MachineKind::ElectrolyticSeparator => b.separating.len(),
                MachineKind::ChemicalOxidizer => b.oxidizing.len(),
                MachineKind::ChemicalInfuser => b.chemical_infusing.len(),
                k if k.ore_chain() => super::mek_ore_machines::recipe_count(k),
                _ => b.recipes.get(&kind).map_or(0, Vec::len),
            }
        }),
    });
    if kind.advanced() || kind.dissolves() {
        out["used_so_far"] = saved.used_so_far.into();
        out["randomized"] = (kind.dissolves() || kind.takes_chemical_upgrades() && config.machines.randomized_consumption).into();
    }
    if kind.washes() {
        out["fluid"] = super::mek_ore_machines::probe_fluid(nbt);
    }
    if (kind.infuses_chemicals() || kind.washes())
        && let Some(extra) = menu::InfuserView::unpack(data)
    {
        out["energy_used"] = extra.energy_used.into();
    }
    if kind.separates() {
        let fluid = mek_world::saved_tank(nbt);
        let extra = SeparatorView::unpack(data).unwrap_or_default();
        out["fluid"] = serde_json::json!({ "id": (!fluid.is_empty()).then(|| fluid.fluid().name()), "amount": fluid.amount() });
        out["dumps"] = serde_json::json!([saved.dumps[0].name(), saved.dumps[1].name()]);
        out["energy_used"] = extra.energy_used.into();
    }
    out
}
