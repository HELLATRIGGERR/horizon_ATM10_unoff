// Mekanism's ore chain in the game (support module `crate::game::mek_ore_machines`
// of the `mekanism` native bundle): the Chemical Dissolution Chamber, the Chemical
// Washer and the Chemical Crystallizer, and the way to the Rotary Condensentrator
// (`mek_rotary`), which shares their glue. Their rules are the bundle's crate
// (`mekanism_energy::ore`, `advanced` and `machine`, MIT, from Mekanism 10.7.19,
// Copyright (c) 2017-2025 Aidan C. Brady); this module is their part of the glue:
// their recipes (`mekanism:dissolution`, `mekanism:washing`,
// `mekanism:crystallizing`), their tank rules, their tick after the shared one
// (`mek_machines`: upgrades, the ejector, the energy slot), their chemical and
// fluid item slots (`ChemicalInventorySlot`, `FluidInventorySlot`), their item
// ejectors and their screens (`GuiChemicalDissolutionChamber`,
// `GuiChemicalWasher`, `GuiChemicalCrystallizer`). Their block entities, energy,
// side configuration, chemical and fluid handlers by side and `/mek probe` are
// `mek_machines`' as for the other machines.

use super::{
    Game, mek_chemicals,
    mek_chem_machines::{self, Made},
    mek_machines::{self, ItemIn, Saved},
    mek_ui::{self, Pen, Rect},
};
use horizon_client::{containers::ModBlockEntity, items::ItemStack};
use horizon_world::{fluid::FluidKind, fluid_ingredient::SizedFluidIngredient, fluid_stack::FluidStack};
use mekanism_energy::{
    advanced::{self, Rng},
    chemical::{self, ChemicalRegistry, ChemicalStack, ChemicalStackIngredient, ChemicalTank, TankRules},
    chemical_tank::{self, OwnTank},
    config::EnergyConfig,
    container::{Automation, EnergyContainer},
    display,
    machine::{self, Inputs, MachineKind, Upgrade},
    menu::{self, InfuserView, MachineView},
    ore::{self, Washing},
    separator,
    sides::{DataType, Direction},
};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    rc::Rc,
    sync::RwLock,
};

// ---- Recipes -------------------------------------------------------------------

/// `ChemicalDissolutionRecipe`: an item and a chemical in, a chemical out.
#[derive(Clone, Debug)]
pub(super) struct Dissolution {
    pub(super) id: String,
    input: ItemIn,
    chemical: ChemicalStackIngredient,
    output: ChemicalStack,
    /// `perTickUsage` (all of ATM10's).
    per_tick: bool,
}

/// `FluidChemicalToChemicalRecipe` (`mekanism:washing`).
#[derive(Clone, Debug)]
pub(super) struct WashingRecipe {
    pub(super) id: String,
    fluid: SizedFluidIngredient,
    chemical: ChemicalStackIngredient,
    output: ChemicalStack,
}

/// `ChemicalCrystallizerRecipe`: a chemical in, an item out.
#[derive(Clone, Debug)]
pub(super) struct Crystallizing {
    pub(super) id: String,
    input: ChemicalStackIngredient,
    output: ItemStack,
}

#[derive(Default)]
struct Book {
    stamp: Vec<(usize, usize)>,
    dissolution: Vec<Rc<Dissolution>>,
    washing: Vec<Rc<WashingRecipe>>,
    crystallizing: Vec<Rc<Crystallizing>>,
}

const TYPES: [&str; 3] = ["mekanism:dissolution", "mekanism:washing", "mekanism:crystallizing"];

thread_local! {
    static BOOK: RefCell<Book> = RefCell::new(Book::default());
    /// The dissolution chamber's draws (`StatUtils`' own `RandomSource`).
    static RNG: Cell<Rng> = Cell::new(Rng::new(
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0x2545_F491, |d| d.as_nanos() as u64 ^ 0x5DEE_CE66),
    ));
}

/// What the slots take, for `native_block_entity_slot_accepts` (asked with no
/// world at hand): made where the recipes are read.
#[derive(Default)]
struct Takes {
    /// The dissolution recipes' items.
    dissolved: HashSet<String>,
    /// What converts to a chemical a dissolution recipe takes (`fillOrConvert`).
    conversions: HashSet<String>,
    /// The washing recipes' fluids.
    washed: Vec<SizedFluidIngredient>,
}

static TAKES: RwLock<Option<Takes>> = RwLock::new(None);

fn draw_random() -> f64 {
    RNG.with(|r| {
        let mut rng = r.get();
        let v = rng.next_f64();
        r.set(rng);
        v
    })
}

/// Makes the book again from the world's tables when one changed; a chemical
/// or fluid the game can't read leaves its recipe out, as Mekanism's codecs
/// fail it. `mek_machines`' book (the conversions) is fresh when this runs.
fn refresh(game: &Game<'_>) {
    let stamp: Vec<(usize, usize)> = TYPES
        .iter()
        .map(|t| {
            let s = game.native_recipes_of_type(t);
            (s.as_ptr() as usize, s.len())
        })
        .collect();
    if BOOK.with(|b| b.borrow().stamp == stamp) {
        return;
    }
    let registry = mek_chemicals::registry(game);
    let mut book = Book { stamp, ..Book::default() };
    book.dissolution = game
        .native_recipes_of_type("mekanism:dissolution")
        .iter()
        .filter_map(|r| {
            let j = &*r.json;
            Some(Rc::new(Dissolution {
                id: r.id.clone(),
                input: mek_machines::item_in(game, &j["item_input"])?,
                chemical: ChemicalStackIngredient::from_json(&j["chemical_input"], &registry).ok()?,
                output: ChemicalStack::from_json(&j["output"], &registry).ok()?,
                per_tick: j.get("per_tick_usage").and_then(Value::as_bool) == Some(true),
            }))
        })
        .collect();
    book.washing = game
        .native_recipes_of_type("mekanism:washing")
        .iter()
        .filter_map(|r| {
            let j = &*r.json;
            Some(Rc::new(WashingRecipe {
                id: r.id.clone(),
                fluid: SizedFluidIngredient::parse_flat(&j["fluid_input"]).ok()?,
                chemical: ChemicalStackIngredient::from_json(&j["chemical_input"], &registry).ok()?,
                output: ChemicalStack::from_json(&j["output"], &registry).ok()?,
            }))
        })
        .collect();
    book.crystallizing = game
        .native_recipes_of_type("mekanism:crystallizing")
        .iter()
        .filter_map(|r| {
            let j = &*r.json;
            Some(Rc::new(Crystallizing {
                id: r.id.clone(),
                input: ChemicalStackIngredient::from_json(&j["input"], &registry).ok()?,
                output: game.native_recipe_result(&j["output"])?,
            }))
        })
        .collect();
    let takes = Takes {
        dissolved: book.dissolution.iter().flat_map(|r| r.input.items.iter().cloned()).collect(),
        conversions: mek_machines::conversion_inputs(&|out| book.dissolution.iter().any(|r| r.chemical.test_type(out.id(), &registry))),
        washed: book.washing.iter().map(|r| r.fluid.clone()).collect(),
    };
    if let Ok(mut t) = TAKES.write() {
        *t = Some(takes);
    }
    BOOK.with(|b| *b.borrow_mut() = book);
}

pub(super) fn clear() {
    BOOK.with(|b| *b.borrow_mut() = Book::default());
    super::mek_rotary::clear();
}

/// The number of each kind's recipes, for the probe.
pub(super) fn recipe_count(kind: MachineKind) -> usize {
    BOOK.with(|b| {
        let b = b.borrow();
        match kind {
            MachineKind::ChemicalDissolutionChamber => b.dissolution.len(),
            MachineKind::ChemicalWasher => b.washing.len(),
            MachineKind::ChemicalCrystallizer => b.crystallizing.len(),
            _ => super::mek_rotary::recipe_count(),
        }
    })
}

/// `containsRecipeBA(item, chemical)` / `containsRecipeB` of the dissolution chamber.
fn dissolution_takes(item: Option<&ItemStack>, chemical: Option<&str>, registry: &ChemicalRegistry) -> bool {
    chemical.is_some()
        && BOOK.with(|b| b.borrow().dissolution.iter().any(|r| r.chemical.test_type(chemical, registry) && item.is_none_or(|i| r.input.takes(i))))
}

/// `containsRecipeBA(fluid, slurry)` / `containsRecipeB` of the washer: a
/// washing recipe taking the slurry with the fluid in its tank (any, empty).
fn washing_takes(fluid: &FluidStack, chemical: Option<&str>, registry: &ChemicalRegistry) -> bool {
    chemical.is_some()
        && BOOK.with(|b| b.borrow().washing.iter().any(|r| r.chemical.test_type(chemical, registry) && (fluid.is_empty() || r.fluid.ingredient.test(fluid))))
}

/// `containsRecipeAB(fluid, slurry)` of the washer's water tank: a washing
/// recipe taking the fluid with the slurry in its tank (any, empty).
fn washing_takes_fluid(fluid: &FluidStack, slurry: &ChemicalStack, registry: &ChemicalRegistry) -> bool {
    !fluid.is_empty()
        && BOOK.with(|b| b.borrow().washing.iter().any(|r| r.fluid.ingredient.test(fluid) && (slurry.is_empty() || r.chemical.test_type(slurry.id(), registry))))
}

fn crystallizing_takes(chemical: Option<&str>, registry: &ChemicalRegistry) -> bool {
    chemical.is_some() && BOOK.with(|b| b.borrow().crystallizing.iter().any(|r| r.input.test_type(chemical, registry)))
}

/// Before a world has read its recipes, any fluid; else whether a washing
/// recipe takes `fluid` (the fluid handler's validator, which may be asked
/// with no world at hand).
fn washes_fluid(fluid: &FluidStack) -> bool {
    TAKES.read().ok().and_then(|t| t.as_ref().map(|t| t.washed.iter().any(|i| i.ingredient.test(fluid)))).unwrap_or(true)
}

// ---- Tanks ---------------------------------------------------------------------

/// Each chemical tank's capacity, in `chemical_tanks` order.
pub(super) fn capacities(kind: MachineKind) -> &'static [i64] {
    match kind {
        MachineKind::ChemicalDissolutionChamber => &[ore::DISSOLUTION_TANK, ore::DISSOLUTION_TANK],
        MachineKind::ChemicalWasher => &[ore::WASHER_TANK, ore::WASHER_TANK],
        MachineKind::ChemicalCrystallizer => &[ore::CRYSTALLIZER_TANK],
        _ => super::mek_rotary::CAPACITIES,
    }
}

/// The fluid tank's capacity (the washer's water, the rotary's), none without one.
pub(super) fn fluid_capacity(kind: MachineKind) -> Option<i32> {
    match kind {
        MachineKind::ChemicalWasher => Some(ore::WASHER_FLUID),
        MachineKind::RotaryCondensentrator => Some(super::mek_rotary::FLUID_CAPACITY),
        _ => None,
    }
}

/// The chemical tanks' rules (`getInitialChemicalTanks`), in their order, run
/// with `use_them`: `item` is the input slot's (the dissolution chamber's),
/// `fluid` the fluid tank's (the washer's), `mode` the rotary's (`mek_rotary`):
/// the dissolution chamber's `inputModern(containsRecipeBA(item, gas), containsRecipeB)`
/// and `output`, the washer's `inputModern(containsRecipeBA(fluid, slurry), containsRecipeB)`
/// and `output`, the crystallizer's `inputModern(containsRecipe)`: only the machine
/// takes out, in goes what a recipe takes, with the default attribute validator.
pub(super) fn with_rules<R>(
    kind: MachineKind,
    item: Option<&ItemStack>,
    fluid: &FluidStack,
    mode: bool,
    registry: &ChemicalRegistry,
    use_them: impl FnOnce(&[&TankRules<'_>]) -> R,
) -> R {
    let out = separator::output_rules();
    match kind {
        MachineKind::ChemicalDissolutionChamber => {
            let can_insert = |s: &ChemicalStack, _: Automation| dissolution_takes(item, s.id(), registry);
            let valid = |s: &ChemicalStack| dissolution_takes(None, s.id(), registry) && registry.passes_default(s);
            let inject = TankRules { can_extract: &chemical::not_external, can_insert: &can_insert, valid: &valid };
            use_them(&[&inject, &out])
        }
        MachineKind::ChemicalWasher => {
            let can_insert = |s: &ChemicalStack, _: Automation| washing_takes(fluid, s.id(), registry);
            let valid = |s: &ChemicalStack| washing_takes(&FluidStack::EMPTY, s.id(), registry) && registry.passes_default(s);
            let input = TankRules { can_extract: &chemical::not_external, can_insert: &can_insert, valid: &valid };
            use_them(&[&input, &out])
        }
        MachineKind::ChemicalCrystallizer => {
            let can_insert = |s: &ChemicalStack, _: Automation| crystallizing_takes(s.id(), registry);
            let valid = |s: &ChemicalStack| crystallizing_takes(s.id(), registry) && registry.passes_default(s);
            let input = TankRules { can_extract: &chemical::not_external, can_insert: &can_insert, valid: &valid };
            use_them(&[&input])
        }
        _ => super::mek_rotary::with_rules(mode, registry, use_them),
    }
}

/// The chemical sides of each kind (`ChemicalSlotInfo` by data type): the
/// tanks a side shows and whether it takes in and gives out, none for a side
/// without a handler.
pub(super) fn chemical_side(kind: MachineKind, data: DataType) -> Option<(Vec<usize>, bool, bool)> {
    Some(match (kind, data) {
        (MachineKind::ChemicalDissolutionChamber | MachineKind::ChemicalWasher, DataType::Input) => (vec![0], true, false),
        (MachineKind::ChemicalDissolutionChamber | MachineKind::ChemicalWasher, DataType::Output) => (vec![1], false, true),
        (MachineKind::ChemicalDissolutionChamber | MachineKind::ChemicalWasher, DataType::InputOutput) => (vec![0, 1], true, true),
        (MachineKind::ChemicalCrystallizer, DataType::Input) => (vec![0], true, false),
        (MachineKind::RotaryCondensentrator, _) => return super::mek_rotary::chemical_side(data),
        _ => return None,
    })
}

/// Whether another block may fill the fluid tank with `fluid` (`canInsert`
/// as `EXTERNAL` and the validators): the washer's `input(containsRecipeAB(fluid,
/// slurry), containsRecipeA)`, the rotary's (`mek_rotary`).
pub(super) fn fluid_fill_allowed(kind: MachineKind, fluid: &FluidStack, slurry: &ChemicalStack, mode: bool, registry: Option<&ChemicalRegistry>) -> bool {
    match kind {
        MachineKind::ChemicalWasher => match registry {
            Some(r) if BOOK.with(|b| !b.borrow().washing.is_empty()) => washing_takes_fluid(fluid, slurry, r),
            _ => washes_fluid(fluid),
        },
        MachineKind::RotaryCondensentrator => super::mek_rotary::fluid_fill_allowed(fluid, mode),
        _ => false,
    }
}

/// Whether another block may drain the fluid tank (`canExtract` as `EXTERNAL`):
/// the rotary's (`mek_rotary`); never the washer's (`notExternal`).
pub(super) fn fluid_drain_allowed(kind: MachineKind, mode: bool) -> bool {
    kind.rotates() && super::mek_rotary::fluid_drain_allowed(mode)
}

// ---- Slots -----------------------------------------------------------------------

/// `native_block_entity_slot_accepts` of their own slots: the dissolution
/// chamber's chemical slot a chemical item holding a chemical or what
/// converts, its input what a recipe takes, its output a chemical item; the
/// washer's fluid slot a container of a fluid it washes with, its slurry
/// output a chemical item; the crystallizer's input a chemical item holding a
/// chemical (the rotary's: `mek_rotary`). Their other output slots take
/// nothing. Before a world has read its recipes, inputs pass.
pub(super) fn slot_takes(kind: MachineKind, slot: usize, stack: &ItemStack) -> bool {
    let item = &*stack.item;
    let known = |pick: &dyn Fn(&Takes) -> bool| TAKES.read().ok().and_then(|t| t.as_ref().map(pick)).unwrap_or(true);
    let chemical_item = super::mek_tanks::tier_of(item).is_some() && stack.count == 1;
    let holding = chemical_item && super::mek_tanks::item_holds(stack);
    let s = kind.slots();
    match kind {
        MachineKind::ChemicalDissolutionChamber => {
            if Some(slot) == s.infusion {
                holding || known(&|t| t.conversions.contains(item))
            } else if slot == s.input {
                known(&|t| t.dissolved.contains(item))
            } else {
                slot == s.output && chemical_item
            }
        }
        MachineKind::ChemicalWasher => {
            if slot == s.input {
                known(&|t| horizon_world::fluid_stack::bucket_fluid(item).is_some_and(|f| t.washed.iter().any(|i| i.ingredient.test(&FluidStack::new(f, 1000)))))
            } else {
                slot == s.output && chemical_item
            }
        }
        MachineKind::ChemicalCrystallizer => slot == s.input && holding,
        _ => super::mek_rotary::slot_takes(slot, stack),
    }
}

/// The slots hoppers reach: their inputs (with the energy slot and the chemical
/// item slots, whose items leave by the item ejector once done) and outputs.
pub(super) fn hopper_slots(kind: MachineKind) -> (Vec<usize>, Vec<usize>) {
    let s = kind.slots();
    match kind {
        MachineKind::ChemicalDissolutionChamber => (vec![s.input, s.energy, s.infusion.unwrap_or(0), s.output], vec![]),
        MachineKind::ChemicalWasher => (vec![s.input, s.energy, s.output], s.output2.into_iter().collect()),
        MachineKind::ChemicalCrystallizer => (vec![s.input, s.energy], vec![s.output]),
        _ => super::mek_rotary::hopper_slots(),
    }
}

pub(super) fn put_one(game: &Game<'_>, entity: &mut ModBlockEntity, slot: usize, item: ItemStack) -> bool {
    let max = game.native_max_stack_size(&item.item).unwrap_or(64).min(64);
    match entity.items.get_mut(slot) {
        Some(cell @ None) => {
            *cell = Some(ItemStack { count: 1, ..item });
            true
        }
        Some(Some(held)) if mek_machines::same_kind(held, &item) && held.count < max => {
            held.count += 1;
            true
        }
        _ => false,
    }
}

/// `IFluidHandlerSlot.fillTank(outputSlot)`: the container in `slot` empties
/// into `tank` (a fluid `valid` takes, into an empty tank or one of the same
/// fluid, what fits; a bucket only whole); a container left holding stays when
/// it is alone, else what it became goes to `out` one at a time.
pub(super) fn fill_fluid_tank(game: &Game<'_>, entity: &mut ModBlockEntity, slot: usize, out: usize, tank: &mut FluidStack, capacity: i32, valid: &dyn Fn(&FluidStack) -> bool) -> bool {
    let Some(stack) = mek_machines::stack_in(entity, slot).cloned() else {
        return false;
    };
    let Some(held) = game.native_item_fluid(&stack) else {
        return false;
    };
    let Some(held_stack) = held.fluid.as_deref().and_then(|f| FluidStack::of(f, held.amount)) else {
        return false;
    };
    if !valid(&held_stack) || !(tank.is_empty() || FluidStack::is_same_fluid_same_components(tank, &held_stack)) {
        return false;
    }
    let room = capacity - tank.amount().max(0);
    let Some((_, took, after)) = game.native_item_fluid_drain(&stack, room).filter(|(_, took, _)| *took > 0) else {
        return false;
    };
    let after_holds = game.native_item_fluid(&after).is_some_and(|h| h.fluid.is_some() && h.amount > 0);
    if stack.count == 1 && after_holds {
        if let Some(cell) = entity.items.get_mut(slot) {
            *cell = Some(after);
        }
    } else if put_one(game, entity, out, after) {
        mek_machines::shrink(entity, slot, 1);
    } else {
        return false;
    }
    *tank = if tank.is_empty() { held_stack.copy_with_amount(took) } else { tank.copy_with_amount(tank.amount() + took) };
    true
}

/// `IFluidHandlerSlot.drainTank(outputSlot)`: `tank` fills the container in
/// `slot` (a bucket takes a whole bucket); one that could take more stays when
/// it is alone, else the filled one goes to `out`.
pub(super) fn drain_fluid_tank(game: &Game<'_>, entity: &mut ModBlockEntity, slot: usize, out: usize, tank: &mut FluidStack) {
    if tank.is_empty() {
        return;
    }
    let Some(stack) = mek_machines::stack_in(entity, slot).cloned() else {
        return;
    };
    let Some((took, filled)) = game.native_item_fluid_fill(&stack, tank.name(), tank.amount()).filter(|(took, _)| *took > 0) else {
        return;
    };
    let more = tank.amount() - took > 0 && game.native_item_fluid_fill(&filled, tank.name(), tank.amount() - took).is_some_and(|(t, _)| t > 0);
    if stack.count == 1 && more {
        if let Some(cell) = entity.items.get_mut(slot) {
            *cell = Some(filled);
        }
    } else if put_one(game, entity, out, filled) {
        mek_machines::shrink(entity, slot, 1);
    } else {
        return;
    }
    let rest = tank.amount() - took;
    *tank = if rest > 0 { tank.copy_with_amount(rest) } else { FluidStack::EMPTY };
}

/// A chemical item slot filling tank `i` (`ChemicalInventorySlot.fillTank`).
pub(super) fn fill_chemical(entity: &mut ModBlockEntity, slot: usize, tank: &mut ChemicalTank, rules: &TankRules<'_>, registry: &ChemicalRegistry, config: &EnergyConfig) -> bool {
    super::mek_tanks::with_slot_item(entity, slot, registry, config, |item| chemical_tank::fill_from_item(item, &mut OwnTank { tank, rules, block: None })) == Some(true)
}

/// A chemical item slot draining a tank (`ChemicalInventorySlot.drainTank`).
pub(super) fn drain_chemical(entity: &mut ModBlockEntity, slot: usize, tank: &mut ChemicalTank, rules: &TankRules<'_>, registry: &ChemicalRegistry, config: &EnergyConfig) {
    super::mek_tanks::with_slot_item(entity, slot, registry, config, |item| chemical_tank::drain_into_item(item, &mut OwnTank { tank, rules, block: None }));
}

// ---- The tick --------------------------------------------------------------------

/// `onUpdateServer` from their own slots on.
#[allow(clippy::too_many_arguments)]
pub(super) fn tick(
    game: &Game<'_>,
    entity: &mut ModBlockEntity,
    kind: MachineKind,
    saved: &mut Saved,
    tanks: &mut [ChemicalTank],
    store: &mut EnergyContainer,
    memo: &mut Option<String>,
    registry: &ChemicalRegistry,
    config: &EnergyConfig,
) -> Made {
    refresh(game);
    match kind {
        MachineKind::ChemicalDissolutionChamber => dissolve(entity, saved, tanks, store, memo, registry, config),
        MachineKind::ChemicalWasher => wash(game, entity, saved, tanks, store, memo, registry, config),
        MachineKind::ChemicalCrystallizer => crystallize(game, entity, saved, tanks, store, memo, registry, config),
        _ => super::mek_rotary::tick(game, entity, saved, tanks, store, memo, registry, config),
    }
}

pub(super) fn number(t: &ChemicalTank, registry: &ChemicalRegistry) -> (i32, u64) {
    (registry.number(t.stored.id()), t.stored().max(0) as u64)
}

pub(super) fn fluid_view(fluid: &FluidStack) -> (i32, u64) {
    if fluid.is_empty() { (0, 0) } else { (fluid.fluid().id() as i32, fluid.amount().max(0) as u64) }
}

/// A first cache takes the saved progress (`loadSavedData`); another recipe starts at 0.
fn cache(memo: &mut Option<String>, id: &str, progress: &mut u32, required: u32) {
    if memo.as_deref() != Some(id) {
        *progress = if memo.is_none() { machine::resumed(*progress, required) } else { 0 };
        *memo = Some(id.to_owned());
    }
}

/// `TileEntityChemicalDissolutionChamber`: its chemical slot fills the input
/// tank (`fillTankOrConvert`: a chemical item, else a conversion that fits
/// whole), its output slot drains the output tank into a chemical item, then
/// the recipe: 100 ticks, the chemical drawn each tick (`crate::ore`), the
/// output into the output tank on the last.
fn dissolve(
    entity: &mut ModBlockEntity,
    saved: &mut Saved,
    tanks: &mut [ChemicalTank],
    store: &mut EnergyContainer,
    memo: &mut Option<String>,
    registry: &ChemicalRegistry,
    config: &EnergyConfig,
) -> Made {
    let kind = MachineKind::ChemicalDissolutionChamber;
    let slots = kind.slots();
    let extra = slots.infusion.unwrap_or(0);
    let input = mek_machines::stack_in(entity, slots.input).cloned();
    let [inject, output] = tanks else {
        return Made { per_tick: 0, required: 0, warnings: 0, active: None, extra: Vec::new() };
    };
    with_rules(kind, input.as_ref(), &FluidStack::EMPTY, false, registry, |rules| {
        let filled = inject.needed() > 0 && fill_chemical(entity, extra, inject, rules[0], registry, config);
        if !filled
            && inject.needed() > 0
            && let Some(stack) = mek_machines::stack_in(entity, extra).cloned()
            && let Some((count, made)) = mek_machines::chemical_conversion(&stack)
            && stack.count >= count
            && inject.insert(&made, true, Automation::Internal, rules[0]).is_empty()
        {
            inject.insert(&made, false, Automation::Internal, rules[0]);
            mek_machines::shrink(entity, extra, count);
        }
        drain_chemical(entity, slots.output, output, rules[1], registry, config);
    });

    let multiplier = config.machines.max_upgrade_multiplier;
    let (usage, _) = config.machines.of(kind);
    let required = saved.upgrades.ticks(ore::DISSOLUTION_TICKS, multiplier);
    let per_tick = saved.upgrades.energy_per_tick(usage, multiplier);
    let baseline = saved.upgrades.operations_per_tick(ore::DISSOLUTION_TICKS, multiplier);
    let mut made = Made { per_tick, required, warnings: 0, active: None, extra: Vec::new() };
    let found = input.as_ref().and_then(|i| {
        BOOK.with(|b| b.borrow().dissolution.iter().find(|r| r.input.takes(i) && r.chemical.test_type(inject.stored.id(), registry)).cloned())
    });
    match found {
        None => {
            if input.is_some() {
                made.warnings |= menu::WARN_NO_RECIPE;
            }
            if memo.is_some() {
                saved.progress = 0;
                made.active = Some(false);
            }
        }
        Some(recipe) => {
            cache(memo, &recipe.id, &mut saved.progress, required);
            let count = input.as_ref().map_or(0, |i| i.count);
            let room = separator::room_for(output, &recipe.output);
            if room == 0 {
                made.warnings |= menu::WARN_OUTPUT;
            }
            if per_tick > store.stored {
                made.warnings |= menu::WARN_ENERGY;
            }
            let item = Inputs::of(count, recipe.input.count);
            let (operations, finished) = if recipe.per_tick {
                let draw = ore::dissolution_use(&saved.upgrades, multiplier).draw(saved.used_so_far, required, saved.progress, draw_random());
                let stored = inject.stored();
                let ran = advanced::process(&mut saved.progress, &mut saved.used_so_far, required, store, per_tick, baseline, item, (stored, recipe.chemical.amount), draw, room);
                if ran.chemical > 0 {
                    inject.shrink_stack(ran.chemical, false);
                }
                if ran.short_chemical {
                    made.warnings |= menu::WARN_NO_RECIPE;
                }
                (ran.operations, ran.finished)
            } else {
                // `TwoInputCachedRecipe.itemChemicalToChemical`: the chemical with the item.
                let inputs = item.both(ore::chemical_inputs(inject.stored(), recipe.chemical.amount));
                let ran = machine::process(&mut saved.progress, required, store, per_tick, baseline, inputs, room, true);
                if ran.finished {
                    inject.shrink_stack(recipe.chemical.amount.saturating_mul(i64::from(ran.operations)), false);
                }
                (ran.operations, ran.finished)
            };
            if finished {
                mek_machines::shrink(entity, slots.input, recipe.input.count * operations);
                let out = recipe.output.copy_with_amount(recipe.output.amount().saturating_mul(i64::from(operations)));
                output.insert(&out, false, Automation::Internal, &separator::output_rules());
            }
            made.active = Some(operations > 0);
        }
    }
    made.extra = InfuserView { tanks: [number(inject, registry), number(output, registry), (0, 0)], energy_used: 0 }.pack();
    made
}

/// `TileEntityChemicalWasher`: its fluid slot empties a container into the
/// water tank (the empty one to the slot below), its output slot drains the
/// output tank into a chemical item, then `2^speed` operations of the recipe.
#[allow(clippy::too_many_arguments)]
fn wash(
    game: &Game<'_>,
    entity: &mut ModBlockEntity,
    saved: &mut Saved,
    tanks: &mut [ChemicalTank],
    store: &mut EnergyContainer,
    memo: &mut Option<String>,
    registry: &ChemicalRegistry,
    config: &EnergyConfig,
) -> Made {
    let kind = MachineKind::ChemicalWasher;
    let slots = kind.slots();
    let [input, output] = tanks else {
        return Made { per_tick: 0, required: 0, warnings: 0, active: None, extra: Vec::new() };
    };
    let mut fluid = super::mek_world::saved_tank(&entity.nbt);
    let before = fluid.clone();
    let slurry = input.stored.clone();
    fill_fluid_tank(game, entity, slots.input, slots.output2.unwrap_or(1), &mut fluid, ore::WASHER_FLUID, &|f| washing_takes_fluid(f, &slurry, registry));
    let rules = separator::output_rules();
    drain_chemical(entity, slots.output, output, &rules, registry, config);

    let multiplier = config.machines.max_upgrade_multiplier;
    let (usage, _) = config.machines.of(kind);
    let per_tick = saved.upgrades.energy_per_tick(usage, multiplier);
    let baseline = separator::speed(saved.upgrades.get(Upgrade::Speed)).0;
    let mut made = Made { per_tick, required: 0, warnings: 0, active: Some(false), extra: Vec::new() };
    let found = BOOK.with(|b| b.borrow().washing.iter().find(|r| r.fluid.ingredient.test(&fluid) && r.chemical.test_type(input.stored.id(), registry)).cloned());
    let mut used = 0;
    match found {
        Some(recipe) => {
            *memo = Some(recipe.id.clone());
            let washing = Washing { fluid: i64::from(recipe.fluid.amount), slurry: recipe.chemical.amount, output: recipe.output.clone() };
            let mut water = i64::from(fluid.amount());
            let ran = ore::wash(&washing, &mut water, input, output, store, per_tick, baseline);
            if ran.operations > 0 {
                let rest = water.clamp(0, i64::from(i32::MAX)) as i32;
                fluid = if rest > 0 { fluid.copy_with_amount(rest) } else { FluidStack::EMPTY };
            }
            if ran.short_energy {
                made.warnings |= menu::WARN_ENERGY;
            }
            if ran.full {
                made.warnings |= menu::WARN_OUTPUT;
            }
            if ran.short_input || ran.short_secondary {
                made.warnings |= menu::WARN_NO_RECIPE;
            }
            used = ran.energy;
            made.active = Some(ran.operations > 0);
        }
        None => {
            if !input.is_empty() || !fluid.is_empty() {
                made.warnings |= menu::WARN_NO_RECIPE;
            }
        }
    }
    if fluid != before {
        super::mek_world::write_tank(&mut entity.nbt, &fluid);
    }
    made.extra = InfuserView { tanks: [number(input, registry), number(output, registry), fluid_view(&fluid)], energy_used: used }.pack();
    made
}

/// `TileEntityChemicalCrystallizer`: its chemical slot fills the tank, then
/// the recipe: 200 ticks, the slurry taken with the crystals on the last.
#[allow(clippy::too_many_arguments)]
fn crystallize(
    game: &Game<'_>,
    entity: &mut ModBlockEntity,
    saved: &mut Saved,
    tanks: &mut [ChemicalTank],
    store: &mut EnergyContainer,
    memo: &mut Option<String>,
    registry: &ChemicalRegistry,
    config: &EnergyConfig,
) -> Made {
    let kind = MachineKind::ChemicalCrystallizer;
    let slots = kind.slots();
    let [tank] = tanks else {
        return Made { per_tick: 0, required: 0, warnings: 0, active: None, extra: Vec::new() };
    };
    with_rules(kind, None, &FluidStack::EMPTY, false, registry, |rules| {
        fill_chemical(entity, slots.input, tank, rules[0], registry, config);
    });
    let multiplier = config.machines.max_upgrade_multiplier;
    let (usage, _) = config.machines.of(kind);
    let required = saved.upgrades.ticks(ore::CRYSTALLIZER_TICKS, multiplier);
    let per_tick = saved.upgrades.energy_per_tick(usage, multiplier);
    let baseline = saved.upgrades.operations_per_tick(ore::CRYSTALLIZER_TICKS, multiplier);
    let mut made = Made { per_tick, required, warnings: 0, active: None, extra: Vec::new() };
    let found = BOOK.with(|b| b.borrow().crystallizing.iter().find(|r| r.input.test_type(tank.stored.id(), registry)).cloned());
    match found {
        None => {
            if !tank.is_empty() {
                made.warnings |= menu::WARN_NO_RECIPE;
            }
            if memo.is_some() {
                saved.progress = 0;
                made.active = Some(false);
            }
        }
        Some(recipe) => {
            cache(memo, &recipe.id, &mut saved.progress, required);
            let held = mek_machines::stack_in(entity, slots.output);
            let max_stack = game.native_max_stack_size(&recipe.output.item).unwrap_or(64);
            let room = machine::output_operations(held.map(|o| (o.count, mek_machines::same_kind(o, &recipe.output))), recipe.output.count, max_stack);
            if room == 0 {
                made.warnings |= menu::WARN_OUTPUT;
            }
            if per_tick > store.stored {
                made.warnings |= menu::WARN_ENERGY;
            }
            let inputs = ore::chemical_inputs(tank.stored(), recipe.input.amount);
            let ran = machine::process(&mut saved.progress, required, store, per_tick, baseline, inputs, room, true);
            if ran.finished {
                tank.shrink_stack(recipe.input.amount.saturating_mul(i64::from(ran.operations)), false);
                let n = recipe.output.count * ran.operations;
                match entity.items.get_mut(slots.output) {
                    Some(Some(out)) => out.count += n,
                    Some(cell) => *cell = Some(ItemStack { count: n, ..recipe.output.clone() }),
                    None => {}
                }
            }
            made.active = Some(ran.active());
        }
    }
    made.extra = InfuserView { tanks: [number(tank, registry), (0, 0), (0, 0)], energy_used: 0 }.pack();
    made
}

// ---- Ejecting --------------------------------------------------------------------

/// The faces of an item config type that gives out.
fn output_faces(sides: &[DataType; 6], facing: Direction) -> Vec<Direction> {
    machine::eject_faces(sides, facing)
}

/// `TileComponentEjector.outputItems`: the slots of their `OUTPUT` type to the
/// faces whose item type gives out: a chemical item once it takes no more of
/// its tank (the dissolution chamber's and the washer's drain slots), the
/// washer's fluid output, the crystallizer's crystals and the rotary's fluid output.
#[allow(clippy::too_many_arguments)]
pub(super) fn eject_items(
    game: &mut Game<'_>,
    pos: [i32; 3],
    entity: &mut ModBlockEntity,
    kind: MachineKind,
    items: &[DataType; 6],
    facing: Direction,
    tanks: &[ChemicalTank],
    registry: &ChemicalRegistry,
    config: &EnergyConfig,
) {
    let s = kind.slots();
    let faces = output_faces(items, facing);
    if faces.is_empty() {
        return;
    }
    if kind.dissolves() || kind.washes() {
        mek_chem_machines::eject_items(game, pos, entity, kind, items, facing, tanks, registry, config);
    }
    match kind {
        MachineKind::ChemicalWasher => mek_machines::eject_items(game, pos, entity, s.output2.unwrap_or(1), &faces),
        MachineKind::ChemicalCrystallizer | MachineKind::RotaryCondensentrator => mek_machines::eject_items(game, pos, entity, s.output, &faces),
        _ => {}
    }
}

/// Whether the ejector puts the last chemical tank out (`setCanTankEject`,
/// `setCanEject`): the dissolution chamber's and the washer's output, the
/// rotary's tank while decondensentrating.
pub(super) fn ejects_chemicals(kind: MachineKind, mode: bool) -> bool {
    kind.dissolves() || kind.washes() || (kind.rotates() && super::mek_rotary::ejects_chemicals(mode))
}

// ---- The screens -----------------------------------------------------------------

/// `GuiChemicalGauge` / `GuiFluidGauge` STANDARD of each (18x60).
const DISSOLUTION_GAUGES: [Rect; 2] = [[7.0, 4.0, 18.0, 60.0], [131.0, 13.0, 18.0, 60.0]];
const WASHER_GAUGES: [Rect; 3] = [[28.0, 13.0, 18.0, 60.0], [131.0, 13.0, 18.0, 60.0], [7.0, 13.0, 18.0, 60.0]];
const CRYSTALLIZER_GAUGE: Rect = [7.0, 4.0, 18.0, 60.0];
/// `GuiProgress` LARGE_RIGHT at (64, 40) (the washer's at (64, 39),
/// the crystallizer's at (53, 61)).
const PROGRESS: Rect = [64.0, 40.0, 48.0, 8.0];
const CRYSTALLIZER_PROGRESS: Rect = [53.0, 61.0, 48.0, 8.0];
/// The crystallizer's `GuiVerticalPowerBar` at (157, 23).
const CRYSTALLIZER_POWER: Rect = [157.0, 23.0, 6.0, 54.0];
/// The washer's fluid slots' holder (`GuiSideHolder` at the panel's right), below the upgrade holder.
pub(super) const WASHER_HOLDER: Rect = [176.0, 84.0, 26.0, 57.0];

/// The power bar of each: the crystallizer's vertical one, the others' horizontal.
pub(super) fn power_bar(kind: MachineKind) -> Rect {
    if kind.crystallizes() { CRYSTALLIZER_POWER } else { mek_chem_machines::POWER_BAR }
}

/// Where Mekanism puts their slots (`getInitialInventory`).
pub(super) fn slots(kind: MachineKind) -> Vec<Option<[f32; 2]>> {
    match kind {
        MachineKind::ChemicalDissolutionChamber => vec![Some([8.0, 65.0]), Some([28.0, 36.0]), Some([152.0, 55.0]), Some([152.0, 14.0])],
        // The fluid slots sit below the upgrade holder (Mekanism: at (180, 71) and (180, 102)).
        MachineKind::ChemicalWasher => vec![Some([180.0, 89.0]), Some([180.0, 120.0]), Some([152.0, 56.0]), Some([152.0, 14.0])],
        MachineKind::ChemicalCrystallizer => vec![Some([8.0, 65.0]), Some([129.0, 57.0]), Some([152.0, 5.0])],
        _ => super::mek_rotary::slots(),
    }
}

pub(super) fn fluid_gauge(pen: &mut Pen<'_, '_>, r: Rect, id: i32, amount: u64, capacity: i32) {
    pen.nine("gauge/normal", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], r);
    let inner = r[3] - 2.0;
    let h = ((amount as f64 / f64::from(capacity.max(1))).clamp(0.0, 1.0) * f64::from(inner)) as i32 as f32;
    if h > 0.0 {
        pen.fill([r[0] + 1.0, r[1] + 1.0 + inner - h, r[2] - 2.0, h], 0xFF00_0000 | mek_ui::fluid_color(id as u16));
    }
    pen.whole("gauge/standard", [16.0, r[3] - 2.0], [r[0] + 1.0, r[1] + 1.0, 16.0, r[3] - 2.0]);
}

pub(super) fn progress(pen: &mut Pen<'_, '_>, r: Rect, share: f64, name: &str) {
    let [px, py, pw, ph] = r;
    pen.sprite(name, [0.0, 0.0, pw, ph], r);
    let done = (share.clamp(0.0, 1.0) * f64::from(pw)) as i32 as f32;
    if done > 0.0 {
        pen.sprite(name, [0.0, ph, done, ph], [px, py, done, ph]);
    }
}

/// Their own elements over the machine panel: the power bar, the gauges, the
/// progress (the rotary's: `mek_rotary`).
pub(super) fn draw(pen: &mut Pen<'_, '_>, kind: MachineKind, view: &MachineView, data: &[i32]) {
    let level = if view.capacity == 0 { 0.0 } else { view.stored as f64 / view.capacity as f64 };
    if kind.crystallizes() {
        let bar = CRYSTALLIZER_POWER;
        pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], bar);
        let h = (level * 52.0) as i32 as f32;
        if h > 0.0 {
            pen.sprite("bar/vertical_power", [0.0, 0.0, 4.0, h], [bar[0] + 1.0, bar[1] + bar[3] - 1.0 - h, 4.0, h]);
        }
    } else {
        let bar = mek_chem_machines::POWER_BAR;
        pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], bar);
        let w = (level * 52.0) as i32 as f32;
        if w > 0.0 {
            pen.sprite("bar/horizontal_power", [0.0, 0.0, w, 4.0], [bar[0] + 1.0, bar[1] + 1.0, w, 4.0]);
        }
    }
    let extra = InfuserView::unpack(data).unwrap_or_default();
    let share = if view.required == 0 { 0.0 } else { f64::from(view.progress) / f64::from(view.required) };
    let on = if view.active { 1.0 } else { 0.0 };
    match kind {
        MachineKind::ChemicalDissolutionChamber => {
            for (r, (n, a)) in DISSOLUTION_GAUGES.into_iter().zip(extra.tanks) {
                mek_chem_machines::gauge(pen, r, n, a, ore::DISSOLUTION_TANK);
            }
            progress(pen, PROGRESS, share, "progress/large_right");
        }
        MachineKind::ChemicalWasher => {
            mek_chem_machines::gauge(pen, WASHER_GAUGES[0], extra.tanks[0].0, extra.tanks[0].1, ore::WASHER_TANK);
            mek_chem_machines::gauge(pen, WASHER_GAUGES[1], extra.tanks[1].0, extra.tanks[1].1, ore::WASHER_TANK);
            fluid_gauge(pen, WASHER_GAUGES[2], extra.tanks[2].0, extra.tanks[2].1, ore::WASHER_FLUID);
            progress(pen, [64.0, 39.0, 48.0, 8.0], on, "progress/large_right");
        }
        MachineKind::ChemicalCrystallizer => {
            mek_chem_machines::gauge(pen, CRYSTALLIZER_GAUGE, extra.tanks[0].0, extra.tanks[0].1, ore::CRYSTALLIZER_TANK);
            progress(pen, CRYSTALLIZER_PROGRESS, share, "progress/large_right");
        }
        _ => super::mek_rotary::draw(pen, &extra, on),
    }
}

pub(super) fn fluid_line(id: i32, amount: u64) -> String {
    if amount == 0 {
        return "Empty".to_owned();
    }
    let name = FluidKind::from_id(id as u32).map_or("Fluid", |f| if f == FluidKind::Water { "Water" } else { f.name() });
    format!("{name}: {} mB", display::thousands(amount))
}

/// The gauges' and progress's tooltips (the rotary's: `mek_rotary`).
pub(super) fn tip(kind: MachineKind, view: &MachineView, data: &[i32], at: [f32; 2]) -> Option<Vec<String>> {
    let extra = InfuserView::unpack(data)?;
    let chemical = |i: usize| vec![mek_chem_machines::tank_line(extra.tanks[i].0, extra.tanks[i].1)];
    let share = || {
        let s = if view.required == 0 { 0.0 } else { f64::from(view.progress) / f64::from(view.required) };
        vec![format!("Progress: {}%", (s * 100.0).floor())]
    };
    match kind {
        MachineKind::ChemicalDissolutionChamber => {
            if let Some(i) = DISSOLUTION_GAUGES.iter().position(|r| mek_ui::inside(*r, at)) {
                return Some(chemical(i));
            }
            mek_ui::inside(PROGRESS, at).then(share)
        }
        MachineKind::ChemicalWasher => {
            if mek_ui::inside(WASHER_GAUGES[2], at) {
                return Some(vec![fluid_line(extra.tanks[2].0, extra.tanks[2].1)]);
            }
            WASHER_GAUGES[..2].iter().position(|r| mek_ui::inside(*r, at)).map(chemical)
        }
        MachineKind::ChemicalCrystallizer => {
            if mek_ui::inside(CRYSTALLIZER_GAUGE, at) {
                return Some(chemical(0));
            }
            mek_ui::inside(CRYSTALLIZER_PROGRESS, at).then(share)
        }
        _ => super::mek_rotary::tip(&extra, at),
    }
}

/// The fluid tank, for the probe.
pub(super) fn probe_fluid(nbt: &horizon_world::nbt::Compound) -> serde_json::Value {
    let fluid = super::mek_world::saved_tank(nbt);
    serde_json::json!({ "id": (!fluid.is_empty()).then(|| fluid.fluid().name()), "amount": fluid.amount() })
}
