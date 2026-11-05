// Mekanism's Rotary Condensentrator in the game (support module
// `crate::game::mek_rotary` of the `mekanism` native bundle). Its rules are the
// bundle's crate (`mekanism_energy::rotary` and `ore::operate`, MIT, from Mekanism
// 10.7.19, Copyright (c) 2017-2025 Aidan C. Brady); this module is its part of the
// glue: its recipes (`mekanism:rotary`), its tanks' rules by its mode, its tick
// after the shared one (`mek_machines`), its chemical and fluid item slots and its
// screen (`GuiRotaryCondensentrator`). The fluid slot helpers and the rest of the
// glue are the ore chain's (`mek_ore_machines`, which hands the rotary's parts
// here) and `mek_machines`'.

use super::{
    Game,
    mek_chem_machines::{self, Made},
    mek_machines::Saved,
    mek_ore_machines::{self as ore_glue, drain_chemical, drain_fluid_tank, fill_chemical, fill_fluid_tank, fluid_view, number},
    mek_ui::{self, Pen, Rect},
};
use horizon_client::{containers::ModBlockEntity, items::ItemStack};
use horizon_world::{fluid_ingredient::SizedFluidIngredient, fluid_stack::FluidStack};
use mekanism_energy::{
    chemical::{ChemicalRegistry, ChemicalStack, ChemicalStackIngredient, ChemicalTank, TankRules},
    config::EnergyConfig,
    container::{Automation, EnergyContainer},
    machine::{MachineKind, Upgrade},
    menu::{self, InfuserView},
    ore, rotary, separator,
    sides::DataType,
};
use serde_json::Value;
use std::{cell::RefCell, rc::Rc, sync::RwLock};

/// Its chemical tank (`gasTank`) and fluid tank, in mB.
pub(super) const CAPACITIES: &[i64] = &[rotary::TANK];
pub(super) const FLUID_CAPACITY: i32 = rotary::FLUID_TANK;

/// `RotaryRecipe`: either way or both (`hasChemicalToFluid`, `hasFluidToChemical`).
#[derive(Clone, Debug)]
struct RotaryRecipe {
    id: String,
    chemical_in: Option<ChemicalStackIngredient>,
    fluid_out: Option<FluidStack>,
    fluid_in: Option<SizedFluidIngredient>,
    chemical_out: Option<ChemicalStack>,
}

#[derive(Default)]
struct Book {
    stamp: (usize, usize),
    recipes: Vec<Rc<RotaryRecipe>>,
}

thread_local! {
    static BOOK: RefCell<Book> = RefCell::new(Book::default());
}

/// The recipes' fluids, for the fluid handler (asked with no world at hand).
static FLUIDS: RwLock<Option<Vec<SizedFluidIngredient>>> = RwLock::new(None);

/// A fluid output (`FluidStack.CODEC`: `{id, amount}`), none for an unknown fluid.
fn fluid_stack(v: &Value) -> Option<FluidStack> {
    let id = v.get("id").and_then(Value::as_str)?;
    let amount = v.get("amount").and_then(Value::as_i64).unwrap_or(1000).clamp(1, i64::from(i32::MAX)) as i32;
    FluidStack::of(id, amount)
}

/// Makes the book again when the world's table changed; a chemical or fluid
/// the game can't read leaves its recipe out.
fn refresh(game: &Game<'_>) {
    let table = game.native_recipes_of_type("mekanism:rotary");
    let stamp = (table.as_ptr() as usize, table.len());
    if BOOK.with(|b| b.borrow().stamp == stamp) {
        return;
    }
    let registry = super::mek_chemicals::registry(game);
    let recipes: Vec<Rc<RotaryRecipe>> = table
        .iter()
        .filter_map(|r| {
            let j = &*r.json;
            let chemical_in = j.get("chemical_input").map(|v| ChemicalStackIngredient::from_json(v, &registry)).transpose().ok()?;
            let chemical_out = j.get("chemical_output").map(|v| ChemicalStack::from_json(v, &registry)).transpose().ok()?;
            let fluid_in = j.get("fluid_input").map(SizedFluidIngredient::parse_flat).transpose().ok()?;
            let fluid_out = match j.get("fluid_output") {
                Some(v) => Some(fluid_stack(v)?),
                None => None,
            };
            // A direction needs both of its halves (`RotaryRecipe`'s codec).
            if chemical_in.is_some() != fluid_out.is_some() || fluid_in.is_some() != chemical_out.is_some() || (chemical_in.is_none() && fluid_in.is_none()) {
                return None;
            }
            Some(Rc::new(RotaryRecipe { id: r.id.clone(), chemical_in, fluid_out, fluid_in, chemical_out }))
        })
        .collect();
    if let Ok(mut f) = FLUIDS.write() {
        *f = Some(recipes.iter().filter_map(|r| r.fluid_in.clone()).collect());
    }
    BOOK.with(|b| *b.borrow_mut() = Book { stamp, recipes });
}

pub(super) fn clear() {
    BOOK.with(|b| *b.borrow_mut() = Book::default());
}

pub(super) fn recipe_count() -> usize {
    BOOK.with(|b| b.borrow().recipes.len())
}

/// `RotaryInputRecipeCache.containsInput` of the chemical tank.
fn takes_chemical(chemical: Option<&str>, registry: &ChemicalRegistry) -> bool {
    chemical.is_some() && BOOK.with(|b| b.borrow().recipes.iter().any(|r| r.chemical_in.as_ref().is_some_and(|i| i.test_type(chemical, registry))))
}

/// `RotaryInputRecipeCache.containsInput` of the fluid tank.
fn takes_fluid(fluid: &FluidStack) -> bool {
    !fluid.is_empty() && BOOK.with(|b| b.borrow().recipes.iter().any(|r| r.fluid_in.as_ref().is_some_and(|i| i.ingredient.test(fluid))))
}

/// The chemical tank's rules (`createModern(canExtract: a player or
/// decondensentrating, canInsert: the machine or condensentrating,
/// containsInput, ALWAYS_ALLOW)`), run with `use_them`.
pub(super) fn with_rules<R>(mode: bool, registry: &ChemicalRegistry, use_them: impl FnOnce(&[&TankRules<'_>]) -> R) -> R {
    let can_extract = |_: &ChemicalStack, a: Automation| rotary::chemical_out(mode, a);
    let can_insert = |_: &ChemicalStack, a: Automation| rotary::chemical_in(mode, a);
    let valid = |s: &ChemicalStack| takes_chemical(s.id(), registry);
    use_them(&[&TankRules { can_extract: &can_extract, can_insert: &can_insert, valid: &valid }])
}

/// `setupIOConfig(CHEMICAL, gasTank, LEFT, true)`: every type both ways, the
/// tank's rules deciding.
pub(super) fn chemical_side(data: DataType) -> Option<(Vec<usize>, bool, bool)> {
    matches!(data, DataType::Input | DataType::Output | DataType::InputOutput).then(|| (vec![0], true, true))
}

/// Another block fills its fluid tank while it decondensentrates, with a fluid
/// a recipe takes (any before a world has read its recipes).
pub(super) fn fluid_fill_allowed(fluid: &FluidStack, mode: bool) -> bool {
    let known = FLUIDS.read().ok().and_then(|f| f.as_ref().map(|list| list.iter().any(|i| i.ingredient.test(fluid)))).unwrap_or(true);
    rotary::fluid_in(mode, Automation::External) && known
}

/// Another block drains its fluid tank while it condensentrates.
pub(super) fn fluid_drain_allowed(mode: bool) -> bool {
    rotary::fluid_out(mode, Automation::External)
}

/// Its slots: the chemical drain slot a chemical item, the fill slot one
/// holding a chemical, the fluid slot a bucket; its fluid output nothing.
pub(super) fn slot_takes(slot: usize, stack: &ItemStack) -> bool {
    let item = &*stack.item;
    let s = MachineKind::RotaryCondensentrator.slots();
    let chemical_item = super::mek_tanks::tier_of(item).is_some() && stack.count == 1;
    if slot == s.input {
        chemical_item
    } else if Some(slot) == s.infusion {
        chemical_item && super::mek_tanks::item_holds(stack)
    } else if Some(slot) == s.output2 {
        item == "minecraft:bucket" || horizon_world::fluid_stack::bucket_fluid(item).is_some()
    } else {
        false
    }
}

/// The slots hoppers reach: its chemical slots, its fluid slot and energy slot
/// in, its fluid output out.
pub(super) fn hopper_slots() -> (Vec<usize>, Vec<usize>) {
    let s = MachineKind::RotaryCondensentrator.slots();
    (vec![s.input, s.infusion.unwrap_or(1), s.output2.unwrap_or(2), s.energy], vec![s.output])
}

/// `setCanEject`: its chemical while decondensentrating.
pub(super) fn ejects_chemicals(mode: bool) -> bool {
    mode
}

/// `TileEntityRotaryCondensentrator.onUpdateServer` from its slots on:
/// decondensentrating, its fluid slot empties a container into the fluid tank
/// and its chemical drain slot fills a chemical item; condensentrating, its
/// chemical fill slot empties one into the chemical tank and its fluid slot
/// fills a container; then `2^speed` operations of the recipe its input tank
/// holds (`RotaryCachedRecipe`).
#[allow(clippy::too_many_arguments)]
pub(super) fn tick(
    game: &Game<'_>,
    entity: &mut ModBlockEntity,
    saved: &mut Saved,
    tanks: &mut [ChemicalTank],
    store: &mut EnergyContainer,
    memo: &mut Option<String>,
    registry: &ChemicalRegistry,
    config: &EnergyConfig,
) -> Made {
    refresh(game);
    let kind = MachineKind::RotaryCondensentrator;
    let slots = kind.slots();
    let (fluid_slot, fluid_out) = (slots.output2.unwrap_or(2), slots.output);
    let [gas] = tanks else {
        return Made { per_tick: 0, required: 0, warnings: 0, active: None, extra: Vec::new() };
    };
    let mode = saved.mode;
    let mut fluid = super::mek_world::saved_tank(&entity.nbt);
    let before = fluid.clone();
    with_rules(mode, registry, |rules| {
        if mode {
            fill_fluid_tank(game, entity, fluid_slot, fluid_out, &mut fluid, rotary::FLUID_TANK, &takes_fluid);
            drain_chemical(entity, slots.input, gas, rules[0], registry, config);
        } else {
            fill_chemical(entity, slots.infusion.unwrap_or(1), gas, rules[0], registry, config);
            drain_fluid_tank(game, entity, fluid_slot, fluid_out, &mut fluid);
        }
    });

    let multiplier = config.machines.max_upgrade_multiplier;
    let (usage, _) = config.machines.of(kind);
    let per_tick = saved.upgrades.energy_per_tick(usage, multiplier);
    let baseline = separator::speed(saved.upgrades.get(Upgrade::Speed)).0;
    let mut made = Made { per_tick, required: 0, warnings: 0, active: Some(false), extra: Vec::new() };
    let found = BOOK.with(|b| {
        b.borrow()
            .recipes
            .iter()
            .find(|r| {
                if mode {
                    r.fluid_in.as_ref().is_some_and(|i| i.ingredient.test(&fluid))
                } else {
                    r.chemical_in.as_ref().is_some_and(|i| i.test_type(gas.stored.id(), registry))
                }
            })
            .cloned()
    });
    let mut used = 0;
    let fits = |stored: i64, each: i64| if each <= 0 { 0 } else { (stored / each).clamp(0, i64::from(i32::MAX)) as u32 };
    match found.as_deref() {
        Some(RotaryRecipe { id, fluid_in: Some(input), chemical_out: Some(out), .. }) if mode => {
            *memo = Some(id.clone());
            let ran = ore::operate(store, per_tick, baseline, &[fits(i64::from(fluid.amount()), i64::from(input.amount))], separator::room_for(gas, out));
            if ran.operations > 0 {
                let n = i64::from(ran.operations);
                let rest = (i64::from(fluid.amount()) - i64::from(input.amount) * n).max(0) as i32;
                fluid = if rest > 0 { fluid.copy_with_amount(rest) } else { FluidStack::EMPTY };
                gas.insert(&out.copy_with_amount(out.amount().saturating_mul(n)), false, Automation::Internal, &separator::output_rules());
            }
            used = ran.energy;
            made.warnings |= warnings(&ran);
            made.active = Some(ran.operations > 0);
        }
        Some(RotaryRecipe { id, chemical_in: Some(input), fluid_out: Some(out), .. }) if !mode => {
            *memo = Some(id.clone());
            let same = fluid.is_empty() || FluidStack::is_same_fluid_same_components(&fluid, out);
            let room = if same { fits(i64::from(rotary::FLUID_TANK - fluid.amount().max(0)), i64::from(out.amount())) } else { 0 };
            let ran = ore::operate(store, per_tick, baseline, &[fits(gas.stored(), input.amount)], room);
            if ran.operations > 0 {
                let n = ran.operations as i32;
                gas.shrink_stack(input.amount.saturating_mul(i64::from(n)), false);
                let add = out.amount().saturating_mul(n);
                fluid = if fluid.is_empty() { out.copy_with_amount(add) } else { fluid.copy_with_amount(fluid.amount() + add) };
            }
            used = ran.energy;
            made.warnings |= warnings(&ran);
            made.active = Some(ran.operations > 0);
        }
        _ => {
            if (mode && !fluid.is_empty()) || (!mode && !gas.is_empty()) {
                made.warnings |= menu::WARN_NO_RECIPE;
            }
        }
    }
    if fluid != before {
        super::mek_world::write_tank(&mut entity.nbt, &fluid);
    }
    made.extra = InfuserView { tanks: [number(gas, registry), fluid_view(&fluid), (i32::from(mode), 0)], energy_used: used }.pack();
    made
}

fn warnings(ran: &ore::Operated) -> i32 {
    let mut w = 0;
    if ran.short_energy {
        w |= menu::WARN_ENERGY;
    }
    if ran.full {
        w |= menu::WARN_OUTPUT;
    }
    if ran.short_input {
        w |= menu::WARN_NO_RECIPE;
    }
    w
}

// ---- The screen ------------------------------------------------------------------

/// `GuiChemicalGauge` at (25, 13) and `GuiFluidGauge` at (133, 13), STANDARD.
const GAUGES: [Rect; 2] = [[25.0, 13.0, 18.0, 60.0], [133.0, 13.0, 18.0, 60.0]];
/// The mode button (`ToggleButton` at (4, 4)).
pub(super) const MODE: Rect = [4.0, 4.0, 18.0, 18.0];

/// Where Mekanism puts its slots (`getInitialInventory`).
pub(super) fn slots() -> Vec<Option<[f32; 2]>> {
    vec![Some([5.0, 25.0]), Some([5.0, 56.0]), Some([155.0, 25.0]), Some([155.0, 56.0]), Some([155.0, 5.0])]
}

/// Its gauges, its progress (`LARGE_RIGHT` condensentrating, `LARGE_LEFT`
/// decondensentrating, full while it runs) and its mode button.
pub(super) fn draw(pen: &mut Pen<'_, '_>, extra: &InfuserView, on: f64) {
    mek_chem_machines::gauge(pen, GAUGES[0], extra.tanks[0].0, extra.tanks[0].1, rotary::TANK);
    ore_glue::fluid_gauge(pen, GAUGES[1], extra.tanks[1].0, extra.tanks[1].1, rotary::FLUID_TANK);
    let mode = extra.tanks[2].0 != 0;
    ore_glue::progress(pen, [64.0, 39.0, 48.0, 8.0], on, if mode { "progress/large_left" } else { "progress/large_right" });
    pen.button(MODE, true);
    pen.centered(if mode { "<" } else { ">" }, MODE[0], MODE[2], MODE[1] + 5.0, 0xE0E0E0);
}

/// Its gauges' and mode button's tooltips.
pub(super) fn tip(extra: &InfuserView, at: [f32; 2]) -> Option<Vec<String>> {
    if mek_ui::inside(GAUGES[0], at) {
        return Some(vec![mek_chem_machines::tank_line(extra.tanks[0].0, extra.tanks[0].1)]);
    }
    if mek_ui::inside(GAUGES[1], at) {
        return Some(vec![ore_glue::fluid_line(extra.tanks[1].0, extra.tanks[1].1)]);
    }
    let mode = extra.tanks[2].0 != 0;
    mek_ui::inside(MODE, at).then(|| vec![if mode { "Decondensentrating" } else { "Condensentrating" }.to_owned(), "Toggle operation".to_owned()])
}
