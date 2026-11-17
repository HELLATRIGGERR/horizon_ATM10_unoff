// Mekanism's Pressurized Reaction Chamber, Isotopic Centrifuge and Solar Neutron
// Activator in the game (support module `crate::game::mek_reaction` of the
// `mekanism` native bundle). Their rules are the bundle's crate
// (`mekanism_energy::reaction`, `ore` and `machine`, MIT, from Mekanism 10.7.19,
// Copyright (c) 2017-2025 Aidan C. Brady); this module is their part of the glue,
// reached through `mek_ore_machines` as the rotary is: their recipes
// (`mekanism:reaction`, `mekanism:centrifuging`, `mekanism:activating`), their
// tank rules, their tick after the shared one (`mek_machines`: upgrades, the
// ejector, the energy slot), the activator's sky, and their screens
// (`GuiPRC`, `GuiIsotopicCentrifuge`, `GuiSolarNeutronActivator`). Their block
// entities, energy, side configuration, chemical and fluid handlers by side and
// `/mek probe` are `mek_machines`' as for the other machines.

use super::{
    Game, mek_chem_machines,
    mek_chem_machines::Made,
    mek_machines::{self, ItemIn, Saved},
    mek_ore_machines::{self as ore_glue, fill_chemical, drain_chemical, fluid_view, number},
    mek_ui::{self, Pen, Rect},
};
use horizon_client::{containers::ModBlockEntity, items::ItemStack};
use horizon_world::{fluid_ingredient::SizedFluidIngredient, fluid_stack::FluidStack};
use mekanism_energy::{
    chemical::{self, ChemicalRegistry, ChemicalStack, ChemicalStackIngredient, ChemicalTank, TankRules},
    config::EnergyConfig,
    container::{Automation, EnergyContainer},
    machine::{self, Inputs, MachineKind, Upgrade},
    menu::{self, InfuserView, MachineView},
    ore,
    reaction::{self, Converting},
    separator,
    sides::{DataType, Direction},
    sky,
    solar::{self, SolarCheck},
};
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::RwLock,
};

// ---- Recipes -------------------------------------------------------------------

/// `PressurizedReactionRecipe`: an item, a fluid and a chemical in, an item
/// and/or a chemical out, its ticks and extra Joules a tick.
#[derive(Debug)]
pub(super) struct Reaction {
    pub(super) id: String,
    item: ItemIn,
    fluid: SizedFluidIngredient,
    chemical: ChemicalStackIngredient,
    duration: u32,
    energy: u64,
    item_out: Option<ItemStack>,
    chemical_out: Option<ChemicalStack>,
}

/// `ChemicalToChemicalRecipe` (centrifuging, activating).
#[derive(Debug)]
pub(super) struct Conversion {
    pub(super) id: String,
    input: ChemicalStackIngredient,
    output: ChemicalStack,
}

#[derive(Default)]
struct Book {
    stamp: Vec<(usize, usize)>,
    reaction: Vec<Rc<Reaction>>,
    centrifuging: Vec<Rc<Conversion>>,
    activating: Vec<Rc<Conversion>>,
}

const TYPES: [&str; 3] = ["mekanism:reaction", "mekanism:centrifuging", "mekanism:activating"];

thread_local! {
    static BOOK: RefCell<Book> = RefCell::new(Book::default());
    /// The activator's ticks into an operation (`CachedRecipe.operatingTicks`,
    /// not saved), by dimension and position.
    static TICKS: RefCell<HashMap<(String, [i32; 3]), u32>> = RefCell::new(HashMap::new());
}

/// The reactions' items and fluids, for the slot and fluid validators asked
/// with no world at hand.
#[derive(Default)]
struct Takes {
    items: HashSet<String>,
    fluids: Vec<SizedFluidIngredient>,
}

static TAKES: RwLock<Option<Takes>> = RwLock::new(None);

fn conversions(game: &Game<'_>, kind: &str, registry: &ChemicalRegistry) -> Vec<Rc<Conversion>> {
    game.native_recipes_of_type(kind)
        .iter()
        .filter_map(|r| {
            let j = &*r.json;
            Some(Rc::new(Conversion {
                id: r.id.clone(),
                input: ChemicalStackIngredient::from_json(&j["input"], registry).ok()?,
                output: ChemicalStack::from_json(&j["output"], registry).ok()?,
            }))
        })
        .collect()
}

/// Makes the book again when one of the tables changed; a part the game can't
/// read leaves its recipe out, as Mekanism's codecs fail it.
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
    let registry = super::mek_chemicals::registry(game);
    let mut book = Book { stamp, ..Book::default() };
    book.reaction = game
        .native_recipes_of_type("mekanism:reaction")
        .iter()
        .filter_map(|r| {
            let j = &*r.json;
            let item_out = match j.get("item_output") {
                Some(v) => Some(game.native_recipe_result(v)?),
                None => None,
            };
            let chemical_out = match j.get("chemical_output") {
                Some(v) => Some(ChemicalStack::from_json(v, &registry).ok()?),
                None => None,
            };
            if item_out.is_none() && chemical_out.is_none() {
                return None;
            }
            Some(Rc::new(Reaction {
                id: r.id.clone(),
                item: mek_machines::item_in(game, &j["item_input"])?,
                fluid: SizedFluidIngredient::parse_flat(&j["fluid_input"]).ok()?,
                chemical: ChemicalStackIngredient::from_json(&j["chemical_input"], &registry).ok()?,
                duration: j.get("duration").and_then(Value::as_u64).map_or(reaction::PRC_TICKS, |d| d.clamp(1, u64::from(i32::MAX as u32)) as u32),
                energy: j.get("energy_required").and_then(Value::as_u64).unwrap_or(0),
                item_out,
                chemical_out,
            }))
        })
        .collect();
    book.centrifuging = conversions(game, "mekanism:centrifuging", &registry);
    book.activating = conversions(game, "mekanism:activating", &registry);
    let takes = Takes {
        items: book.reaction.iter().flat_map(|r| r.item.items.iter().cloned()).collect(),
        fluids: book.reaction.iter().map(|r| r.fluid.clone()).collect(),
    };
    if let Ok(mut t) = TAKES.write() {
        *t = Some(takes);
    }
    BOOK.with(|b| *b.borrow_mut() = book);
}

pub(super) fn clear() {
    BOOK.with(|b| *b.borrow_mut() = Book::default());
    TICKS.with(|t| t.borrow_mut().clear());
}

pub(super) fn recipe_count(kind: MachineKind) -> usize {
    BOOK.with(|b| {
        let b = b.borrow();
        match kind {
            MachineKind::PressurizedReactionChamber => b.reaction.len(),
            MachineKind::IsotopicCentrifuge => b.centrifuging.len(),
            _ => b.activating.len(),
        }
    })
}

fn converting(kind: MachineKind, chemical: Option<&str>, registry: &ChemicalRegistry) -> Option<Rc<Conversion>> {
    chemical?;
    BOOK.with(|b| {
        let b = b.borrow();
        let list = if kind == MachineKind::IsotopicCentrifuge { &b.centrifuging } else { &b.activating };
        list.iter().find(|r| r.input.test_type(chemical, registry)).cloned()
    })
}

/// `containsRecipeBAC`-like: a reaction taking `chemical` with what the item
/// slot and the fluid tank hold (an empty one takes any).
fn reaction_takes(item: Option<&ItemStack>, fluid: &FluidStack, chemical: Option<&str>, registry: &ChemicalRegistry) -> bool {
    chemical.is_some()
        && BOOK.with(|b| {
            b.borrow().reaction.iter().any(|r| {
                r.chemical.test_type(chemical, registry) && item.is_none_or(|i| r.item.takes(i)) && (fluid.is_empty() || r.fluid.ingredient.test(fluid))
            })
        })
}

// ---- Tanks ---------------------------------------------------------------------

pub(super) fn capacities(kind: MachineKind) -> &'static [i64] {
    if kind.reacts() { &[reaction::PRC_TANK, reaction::PRC_TANK] } else { &[reaction::TANK, reaction::TANK] }
}

pub(super) fn fluid_capacity(kind: MachineKind) -> Option<i32> {
    kind.reacts().then_some(reaction::PRC_FLUID)
}

/// The tanks' rules (`getInitialChemicalTanks`): the input `createModern`
/// (only the machine takes out; in goes what a recipe takes, the chamber's
/// with its item and fluid; `ALWAYS_ALLOW`, so radioactive chemicals too),
/// the output `output`.
pub(super) fn with_rules<R>(
    kind: MachineKind,
    item: Option<&ItemStack>,
    fluid: &FluidStack,
    registry: &ChemicalRegistry,
    use_them: impl FnOnce(&[&TankRules<'_>]) -> R,
) -> R {
    let out = separator::output_rules();
    if kind.reacts() {
        let can_insert = |s: &ChemicalStack, _: Automation| reaction_takes(item, fluid, s.id(), registry);
        let valid = |s: &ChemicalStack| reaction_takes(None, &FluidStack::EMPTY, s.id(), registry);
        let input = TankRules { can_extract: &chemical::not_external, can_insert: &can_insert, valid: &valid };
        use_them(&[&input, &out])
    } else {
        let can_insert = |_: &ChemicalStack, _: Automation| true;
        let valid = |s: &ChemicalStack| converting(kind, s.id(), registry).is_some();
        let input = TankRules { can_extract: &chemical::not_external, can_insert: &can_insert, valid: &valid };
        use_them(&[&input, &out])
    }
}

/// `setupIOConfig(CHEMICAL, input, output, side, false, true)`: an input side
/// shows the input tank, an output side the output tank (out only).
pub(super) fn chemical_side(data: DataType) -> Option<(Vec<usize>, bool, bool)> {
    Some(match data {
        DataType::Input => (vec![0], true, true),
        DataType::Output => (vec![1], false, true),
        DataType::InputOutput => (vec![0, 1], true, true),
        _ => return None,
    })
}

/// The chamber's fluid tank (`BasicFluidTank.input`): a fluid a reaction
/// takes with the chemical it holds; never drained by another block.
pub(super) fn fluid_fill_allowed(fluid: &FluidStack, chemical: &ChemicalStack, registry: Option<&ChemicalRegistry>) -> bool {
    match registry {
        Some(r) if BOOK.with(|b| !b.borrow().reaction.is_empty()) => BOOK.with(|b| {
            b.borrow().reaction.iter().any(|x| x.fluid.ingredient.test(fluid) && (chemical.is_empty() || x.chemical.test_type(chemical.id(), r)))
        }),
        _ => TAKES.read().ok().and_then(|t| t.as_ref().map(|t| t.fluids.iter().any(|i| i.ingredient.test(fluid)))).unwrap_or(true),
    }
}

// ---- Slots -----------------------------------------------------------------------

/// The chamber's input takes what a reaction takes, its output nothing; the
/// centrifuge's and activator's fill slot a chemical item holding a chemical,
/// their drain slot a chemical item.
pub(super) fn slot_takes(kind: MachineKind, slot: usize, stack: &ItemStack) -> bool {
    let s = kind.slots();
    if kind.reacts() {
        return slot == s.input && TAKES.read().ok().and_then(|t| t.as_ref().map(|t| t.items.contains(&*stack.item))).unwrap_or(true);
    }
    let chemical_item = super::mek_tanks::tier_of(&stack.item).is_some() && stack.count == 1;
    if slot == s.input { chemical_item && super::mek_tanks::item_holds(stack) } else { slot == s.output && chemical_item }
}

pub(super) fn hopper_slots(kind: MachineKind) -> (Vec<usize>, Vec<usize>) {
    let s = kind.slots();
    match kind {
        MachineKind::PressurizedReactionChamber => (vec![s.input, s.energy], vec![s.output]),
        MachineKind::IsotopicCentrifuge => (vec![s.input, s.energy, s.output], vec![]),
        _ => (vec![s.input, s.output], vec![]),
    }
}

// ---- The tick --------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub(super) fn tick(
    game: &Game<'_>,
    pos: [i32; 3],
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
    if kind.reacts() { react(entity, saved, tanks, store, memo, registry, config, game) } else { convert(game, pos, entity, kind, saved, tanks, store, memo, registry, config) }
}

/// The chamber's recipe as Mekanism finds it (`PressurizedReactionRecipe.test`):
/// all of each input there.
fn find_reaction(item: Option<&ItemStack>, fluid: &FluidStack, chemical: &ChemicalTank, registry: &ChemicalRegistry) -> Option<Rc<Reaction>> {
    let item = item?;
    BOOK.with(|b| {
        b.borrow()
            .reaction
            .iter()
            .find(|r| {
                r.item.takes(item)
                    && item.count >= r.item.count
                    && r.fluid.ingredient.test(fluid)
                    && fluid.amount() >= r.fluid.amount
                    && r.chemical.test_type(chemical.stored.id(), registry)
                    && chemical.stored() >= r.chemical.amount
            })
            .cloned()
    })
}

/// `TileEntityPressurizedReactionChamber`: the recipe over its duration
/// (`machine::process`, one operation), its base Joules plus the recipe's a
/// tick, everything taken and made on the last tick.
#[allow(clippy::too_many_arguments)]
fn react(
    entity: &mut ModBlockEntity,
    saved: &mut Saved,
    tanks: &mut [ChemicalTank],
    store: &mut EnergyContainer,
    memo: &mut Option<String>,
    registry: &ChemicalRegistry,
    config: &EnergyConfig,
    game: &Game<'_>,
) -> Made {
    let kind = MachineKind::PressurizedReactionChamber;
    let s = kind.slots();
    let [input, output] = tanks else {
        return Made { per_tick: 0, required: 0, warnings: 0, active: None, extra: Vec::new() };
    };
    let mut fluid = super::mek_world::saved_tank(&entity.nbt);
    let before = fluid.clone();
    let item = mek_machines::stack_in(entity, s.input).cloned();
    let found = find_reaction(item.as_ref(), &fluid, input, registry);
    let multiplier = config.machines.max_upgrade_multiplier;
    let (usage, _) = config.machines.of(kind);
    let (duration, extra) = found.as_ref().map_or((reaction::PRC_TICKS, 0), |r| (r.duration, r.energy));
    let required = saved.upgrades.ticks(duration, multiplier);
    let per_tick = saved.upgrades.energy_per_tick(reaction::prc_base_energy(usage, extra), multiplier);
    let mut made = Made { per_tick, required, warnings: 0, active: None, extra: Vec::new() };
    let mut used = 0;
    match found {
        None => {
            if item.is_some() || !fluid.is_empty() || !input.is_empty() {
                made.warnings |= menu::WARN_NO_RECIPE;
            }
            if memo.is_some() {
                saved.progress = 0;
                made.active = Some(false);
            }
        }
        Some(recipe) => {
            if memo.as_deref() != Some(recipe.id.as_str()) {
                saved.progress = if memo.is_none() { machine::resumed(saved.progress, required) } else { 0 };
                *memo = Some(recipe.id.clone());
            }
            let held = mek_machines::stack_in(entity, s.output);
            let item_room = recipe.item_out.as_ref().map_or(u32::MAX, |o| {
                let max = game.native_max_stack_size(&o.item).unwrap_or(64);
                machine::output_operations(held.map(|h| (h.count, mek_machines::same_kind(h, o))), o.count, max)
            });
            let chemical_room = recipe.chemical_out.as_ref().map_or(u32::MAX, |o| separator::room_for(output, o));
            let room = item_room.min(chemical_room);
            if room == 0 {
                made.warnings |= menu::WARN_OUTPUT;
            }
            if per_tick > store.stored {
                made.warnings |= menu::WARN_ENERGY;
            }
            let count = item.as_ref().map_or(0, |i| i.count);
            let inputs = Inputs::of(count, recipe.item.count)
                .both(Inputs::of(fluid.amount().max(0) as u32, recipe.fluid.amount.max(1) as u32))
                .both(ore::chemical_inputs(input.stored(), recipe.chemical.amount));
            let ran = machine::process(&mut saved.progress, required, store, per_tick, 1, inputs, room, true);
            used = ran.energy;
            if ran.finished {
                let n = ran.operations;
                mek_machines::shrink(entity, s.input, recipe.item.count * n);
                let rest = fluid.amount() - recipe.fluid.amount.saturating_mul(n as i32);
                fluid = if rest > 0 { fluid.copy_with_amount(rest) } else { FluidStack::EMPTY };
                input.shrink_stack(recipe.chemical.amount.saturating_mul(i64::from(n)), false);
                if let Some(o) = &recipe.item_out {
                    let add = o.count * n;
                    match entity.items.get_mut(s.output) {
                        Some(Some(out)) => out.count += add,
                        Some(cell) => *cell = Some(ItemStack { count: add, ..o.clone() }),
                        None => {}
                    }
                }
                if let Some(o) = &recipe.chemical_out {
                    output.insert(&o.copy_with_amount(o.amount().saturating_mul(i64::from(n))), false, Automation::Internal, &separator::output_rules());
                }
            }
            made.active = Some(ran.operations > 0);
        }
    }
    if fluid != before {
        super::mek_world::write_tank(&mut entity.nbt, &fluid);
    }
    made.extra = InfuserView { tanks: [number(input, registry), number(output, registry), fluid_view(&fluid)], energy_used: used }.pack();
    made
}

/// The activator's rate where it stands (`recheckSettings`,
/// `recalculateProductionRate`): its biome, the sky above it, the weather.
fn sna_rate(game: &Game<'_>, pos: [i32; 3], config: &EnergyConfig) -> f32 {
    let Some(biome) = game.native_biome_climate(pos) else {
        return 0.0;
    };
    let rain = biome.precipitation != super::native_hooks::NativePrecipitation::None;
    let check = SolarCheck::new(biome.temperature, biome.downfall, rain);
    let kind = game.native_dimension_type();
    let (rain_level, thunder) = game.native_weather();
    let day = game.native_day_time();
    let sky = solar::Sky { day_time: day, rain: rain_level, thunder, has_skylight: kind.has_skylight, has_ceiling: kind.has_ceiling };
    let (_, light) = game.native_light([pos[0], pos[1] + 1, pos[2]]);
    let sees = solar::sees_sun(kind.has_skylight, sky::sky_darken(day, rain_level, thunder), light);
    let peak = reaction::sna_peak(config.machines.max_sna_rate, &check);
    reaction::sna_rate(peak, &check, sees, sky::sun_brightness(day, rain_level, thunder), sky.raining())
}

/// `TileEntityIsotopicCentrifuge` and `TileEntitySolarNeutronActivator`: the
/// fill slot into the input tank, the output tank into the drain slot, then
/// the recipe: the centrifuge `2^speed` operations on its Joules, the
/// activator at its sky's rate.
#[allow(clippy::too_many_arguments)]
fn convert(
    game: &Game<'_>,
    pos: [i32; 3],
    entity: &mut ModBlockEntity,
    kind: MachineKind,
    saved: &mut Saved,
    tanks: &mut [ChemicalTank],
    store: &mut EnergyContainer,
    memo: &mut Option<String>,
    registry: &ChemicalRegistry,
    config: &EnergyConfig,
) -> Made {
    let s = kind.slots();
    let [input, output] = tanks else {
        return Made { per_tick: 0, required: 0, warnings: 0, active: None, extra: Vec::new() };
    };
    with_rules(kind, None, &FluidStack::EMPTY, registry, |rules| {
        fill_chemical(entity, s.input, input, rules[0], registry, config);
        drain_chemical(entity, s.output, output, rules[1], registry, config);
    });
    let centrifuge = kind == MachineKind::IsotopicCentrifuge;
    let multiplier = config.machines.max_upgrade_multiplier;
    let (usage, _) = config.machines.of(kind);
    let per_tick = if centrifuge { saved.upgrades.energy_per_tick(usage, multiplier) } else { 0 };
    let mut made = Made { per_tick, required: 0, warnings: 0, active: Some(false), extra: Vec::new() };
    let rate = if centrifuge { 0.0 } else { sna_rate(game, pos, config) };
    let key = (game.native_dimension().to_owned(), pos);
    let mut used = 0;
    match converting(kind, input.stored.id(), registry) {
        Some(recipe) => {
            if memo.as_deref() != Some(recipe.id.as_str()) {
                TICKS.with(|t| t.borrow_mut().remove(&key));
                *memo = Some(recipe.id.clone());
            }
            let shape = Converting { input: recipe.input.amount, output: recipe.output.clone() };
            let ran = if centrifuge {
                reaction::convert(&shape, input, output, store, per_tick, separator::speed(saved.upgrades.get(Upgrade::Speed)).0)
            } else {
                let mut ticks = TICKS.with(|t| t.borrow().get(&key).copied().unwrap_or(0));
                let ran = reaction::activate(&shape, input, output, &mut ticks, rate);
                TICKS.with(|t| t.borrow_mut().insert(key.clone(), ticks));
                made.required = reaction::sna_ticks(rate);
                ran
            };
            if ran.short_energy {
                made.warnings |= menu::WARN_ENERGY;
            }
            if ran.full {
                made.warnings |= menu::WARN_OUTPUT;
            }
            if ran.short_input {
                made.warnings |= menu::WARN_NO_RECIPE;
            }
            used = ran.energy;
            made.active = Some(ran.operations > 0);
        }
        None => {
            if !input.is_empty() {
                made.warnings |= menu::WARN_NO_RECIPE;
            }
        }
    }
    // The activator shows its rate in mB a tick where the others show Joules used.
    let shown = if centrifuge { used } else { (f64::from(rate) * 1000.0) as u64 };
    made.extra = InfuserView { tanks: [number(input, registry), number(output, registry), (0, 0)], energy_used: shown }.pack();
    made
}

/// Forgets an activator taken away.
pub(super) fn forget(dimension: &str, pos: [i32; 3]) {
    TICKS.with(|t| t.borrow_mut().remove(&(dimension.to_owned(), pos)));
}

/// The item ejector: the chamber's output slot, the others' drain slot once
/// it takes no more (`mek_chem_machines::eject_items`).
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
    faces: &[Direction],
) {
    if kind.reacts() {
        mek_machines::eject_items(game, pos, entity, kind.slots().output, faces);
    } else {
        mek_chem_machines::eject_items(game, pos, entity, kind, items, facing, tanks, registry, config);
    }
}

// ---- The screens -----------------------------------------------------------------

/// `GuiPRC`: the fluid and chemical gauges (STANDARD at (5, 15) and (28, 15)),
/// the output's SMALL gauge at (140, 45), the vertical power bar at (163, 21),
/// the RIGHT progress at (77, 43).
const PRC_GAUGES: [Rect; 3] = [[28.0, 15.0, 18.0, 60.0], [140.0, 45.0, 18.0, 30.0], [5.0, 15.0, 18.0, 60.0]];
const PRC_POWER: Rect = [163.0, 21.0, 6.0, 54.0];
const PRC_PROGRESS: Rect = [77.0, 43.0, 32.0, 8.0];
/// `GuiIsotopicCentrifuge` and `GuiSolarNeutronActivator`: STANDARD gauges at
/// (25, 13) and (133, 13), LARGE_RIGHT progress at (64, 39), the centrifuge's
/// horizontal power bar at (115, 75).
const GAUGES: [Rect; 2] = [[25.0, 13.0, 18.0, 60.0], [133.0, 13.0, 18.0, 60.0]];
const PROGRESS: Rect = [64.0, 39.0, 48.0, 8.0];

pub(super) fn power_bar(kind: MachineKind) -> Rect {
    if kind.reacts() { PRC_POWER } else { mek_chem_machines::POWER_BAR }
}

/// Mekanism's slots (`getInitialInventory`): the chamber's input (54, 40),
/// output (116, 40), energy (141, 22); the centrifuge's fill (5, 56), drain
/// (155, 56), energy (155, 14); the activator's fill and drain.
pub(super) fn slots(kind: MachineKind) -> Vec<Option<[f32; 2]>> {
    match kind {
        MachineKind::PressurizedReactionChamber => vec![Some([54.0, 40.0]), Some([116.0, 40.0]), Some([141.0, 22.0])],
        MachineKind::IsotopicCentrifuge => vec![Some([5.0, 56.0]), Some([155.0, 56.0]), Some([155.0, 14.0])],
        _ => vec![Some([5.0, 56.0]), Some([155.0, 56.0])],
    }
}

pub(super) fn draw(pen: &mut Pen<'_, '_>, kind: MachineKind, view: &MachineView, data: &[i32]) {
    let level = if view.capacity == 0 { 0.0 } else { view.stored as f64 / view.capacity as f64 };
    let extra = InfuserView::unpack(data).unwrap_or_default();
    if kind.reacts() {
        let bar = PRC_POWER;
        pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], bar);
        let h = (level * 52.0) as i32 as f32;
        if h > 0.0 {
            pen.sprite("bar/vertical_power", [0.0, 0.0, 4.0, h], [bar[0] + 1.0, bar[1] + bar[3] - 1.0 - h, 4.0, h]);
        }
        mek_chem_machines::gauge(pen, PRC_GAUGES[0], extra.tanks[0].0, extra.tanks[0].1, reaction::PRC_TANK);
        mek_chem_machines::gauge(pen, PRC_GAUGES[1], extra.tanks[1].0, extra.tanks[1].1, reaction::PRC_TANK);
        ore_glue::fluid_gauge(pen, PRC_GAUGES[2], extra.tanks[2].0, extra.tanks[2].1, reaction::PRC_FLUID);
        let share = if view.required == 0 { 0.0 } else { f64::from(view.progress) / f64::from(view.required) };
        ore_glue::progress(pen, PRC_PROGRESS, share, "progress/right");
        return;
    }
    if kind == MachineKind::IsotopicCentrifuge {
        let bar = mek_chem_machines::POWER_BAR;
        pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], bar);
        let w = (level * 52.0) as i32 as f32;
        if w > 0.0 {
            pen.sprite("bar/horizontal_power", [0.0, 0.0, w, 4.0], [bar[0] + 1.0, bar[1] + 1.0, w, 4.0]);
        }
    }
    for (r, (n, a)) in GAUGES.into_iter().zip(extra.tanks) {
        mek_chem_machines::gauge(pen, r, n, a, reaction::TANK);
    }
    ore_glue::progress(pen, PROGRESS, if view.active { 1.0 } else { 0.0 }, "progress/large_right");
}

pub(super) fn tip(kind: MachineKind, view: &MachineView, data: &[i32], at: [f32; 2]) -> Option<Vec<String>> {
    let extra = InfuserView::unpack(data)?;
    let chemical = |i: usize| vec![mek_chem_machines::tank_line(extra.tanks[i].0, extra.tanks[i].1)];
    if kind.reacts() {
        if mek_ui::inside(PRC_GAUGES[2], at) {
            return Some(vec![ore_glue::fluid_line(extra.tanks[2].0, extra.tanks[2].1)]);
        }
        if let Some(i) = PRC_GAUGES[..2].iter().position(|r| mek_ui::inside(*r, at)) {
            return Some(chemical(i));
        }
        let share = if view.required == 0 { 0.0 } else { f64::from(view.progress) / f64::from(view.required) };
        return mek_ui::inside(PRC_PROGRESS, at).then(|| vec![format!("Progress: {}%", (share * 100.0).floor())]);
    }
    if kind == MachineKind::SolarNeutronActivator && mek_ui::inside(PROGRESS, at) {
        return Some(vec![format!("Production rate: {:.2} mB/t", extra.energy_used as f64 / 1000.0)]);
    }
    GAUGES.iter().position(|r| mek_ui::inside(*r, at)).map(chemical)
}
