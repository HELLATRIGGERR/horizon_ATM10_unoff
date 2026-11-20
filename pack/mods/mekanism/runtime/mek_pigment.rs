// Mekanism's Painting Machine in the game (support module `crate::game::mek_pigment` of the
// `mekanism` native bundle). Its rules are the bundle's crate (`mekanism_energy::pigment`,
// `ore` and `machine`, MIT, from Mekanism 10.7.19, Copyright (c) 2017-2025 Aidan C. Brady);
// this module is its part of the glue, reached through `mek_ore_machines` as the reaction
// chamber is: its recipes (`mekanism:painting`), its pigment tank's rules, its tick after the
// shared one (`mek_machines`: upgrades, the item ejector, the energy slot) and its screen
// (`GuiPaintingMachine`). The Pigment Extractor and the Pigment Mixer run on the oxidizer's and
// the chemical infuser's glue (`mek_chem_machines`) with their own recipes. Block entities,
// energy, side configuration, the chemical handler by side and `/mek probe` are
// `mek_machines`' as for the other machines.

use super::{
    Game,
    mek_chem_machines,
    mek_chem_machines::Made,
    mek_machines::{self, ItemIn, Saved},
    mek_ore_machines::{self as ore_glue, fill_chemical, number},
    mek_ui::{self, Pen, Rect},
};
use horizon_client::{containers::ModBlockEntity, items::ItemStack};
use mekanism_energy::{
    chemical::{self, ChemicalRegistry, ChemicalStack, ChemicalStackIngredient, ChemicalTank, TankRules},
    config::EnergyConfig,
    container::EnergyContainer,
    machine::{self, Inputs, MachineKind},
    menu::{self, InfuserView, MachineView},
    ore, pigment,
};
use std::{cell::RefCell, collections::HashSet, rc::Rc, sync::RwLock};

/// `ItemStackChemicalToItemStackRecipe` (`mekanism:painting`): an item and an
/// amount of a pigment into an item.
#[derive(Debug)]
pub(super) struct Painting {
    pub(super) id: String,
    item: ItemIn,
    pigment: ChemicalStackIngredient,
    output: ItemStack,
}

#[derive(Default)]
struct Book {
    stamp: (usize, usize),
    recipes: Vec<Rc<Painting>>,
}

thread_local! {
    static BOOK: RefCell<Book> = RefCell::new(Book::default());
}

/// The painted items, for the slot validator asked with no world at hand.
static TAKES: RwLock<Option<HashSet<String>>> = RwLock::new(None);

/// Makes the book again when the table changed. A recipe the game can't read
/// leaves out, as Mekanism's codecs fail it; one with `per_tick_usage` isn't
/// ported (ATM10 has none: every painting recipe uses its pigment on its last
/// tick).
fn refresh(game: &Game<'_>) {
    let table = game.native_recipes_of_type("mekanism:painting");
    let stamp = (table.as_ptr() as usize, table.len());
    if BOOK.with(|b| b.borrow().stamp == stamp) {
        return;
    }
    let registry = super::mek_chemicals::registry(game);
    let recipes: Vec<Rc<Painting>> = table
        .iter()
        .filter_map(|r| {
            let j = &*r.json;
            if j.get("per_tick_usage").and_then(serde_json::Value::as_bool) == Some(true) {
                return None;
            }
            Some(Rc::new(Painting {
                id: r.id.clone(),
                item: mek_machines::item_in(game, &j["item_input"])?,
                pigment: ChemicalStackIngredient::from_json(&j["chemical_input"], &registry).ok()?,
                output: game.native_recipe_result(&j["output"])?,
            }))
        })
        .collect();
    if let Ok(mut t) = TAKES.write() {
        *t = Some(recipes.iter().flat_map(|r| r.item.items.iter().cloned()).collect());
    }
    BOOK.with(|b| *b.borrow_mut() = Book { stamp, recipes });
}

pub(super) fn clear() {
    BOOK.with(|b| *b.borrow_mut() = Book::default());
}

pub(super) fn recipe_count() -> usize {
    BOOK.with(|b| b.borrow().recipes.len())
}

/// `ItemStackChemicalToItemStackRecipe.test`: the recipe taking this item and
/// this pigment (their types: how many each needs is the process's check).
fn find(item: Option<&ItemStack>, pigment: Option<&str>, registry: &ChemicalRegistry) -> Option<Rc<Painting>> {
    let item = item?;
    BOOK.with(|b| b.borrow().recipes.iter().find(|r| r.item.takes(item) && r.pigment.test_type(pigment, registry)).cloned())
}

/// Whether a recipe takes `pigment` with what the item slot holds (an empty
/// slot takes any).
fn takes(item: Option<&ItemStack>, pigment: Option<&str>, registry: &ChemicalRegistry) -> bool {
    pigment.is_some() && BOOK.with(|b| b.borrow().recipes.iter().any(|r| r.pigment.test_type(pigment, registry) && item.is_none_or(|i| r.item.takes(i))))
}

/// The pigment tank (`BasicChemicalTank.createModern`: only the machine takes
/// out; in goes what a recipe takes with the item in the slot).
pub(super) fn with_rules<R>(item: Option<&ItemStack>, registry: &ChemicalRegistry, use_them: impl FnOnce(&[&TankRules<'_>]) -> R) -> R {
    let can_insert = |s: &ChemicalStack, _: mekanism_energy::container::Automation| takes(item, s.id(), registry);
    let valid = |s: &ChemicalStack| takes(None, s.id(), registry);
    let input = TankRules { can_extract: &chemical::not_external, can_insert: &can_insert, valid: &valid };
    use_them(&[&input])
}

/// The input slot takes what a recipe paints, the pigment slot a chemical
/// item holding a chemical, the output nothing.
pub(super) fn slot_takes(slot: usize, stack: &ItemStack) -> bool {
    let s = MachineKind::PaintingMachine.slots();
    if slot == s.input {
        return TAKES.read().ok().and_then(|t| t.as_ref().map(|t| t.contains(&*stack.item))).unwrap_or(true);
    }
    slot == s.infusion.unwrap_or(usize::MAX) && super::mek_tanks::tier_of(&stack.item).is_some() && stack.count == 1 && super::mek_tanks::item_holds(stack)
}

pub(super) fn hopper_slots() -> (Vec<usize>, Vec<usize>) {
    let s = MachineKind::PaintingMachine.slots();
    (vec![s.input, s.energy, s.infusion.unwrap_or(0)], vec![s.output])
}

/// `TileEntityPaintingMachine.onUpdateServer`: the pigment slot into the tank,
/// then the recipe over its ticks (`machine::process`, one operation), the
/// pigment and the item taken and the painted item made on the last tick.
#[allow(clippy::too_many_arguments)]
pub(super) fn tick(
    entity: &mut ModBlockEntity,
    saved: &mut Saved,
    tanks: &mut [ChemicalTank],
    store: &mut EnergyContainer,
    memo: &mut Option<String>,
    registry: &ChemicalRegistry,
    config: &EnergyConfig,
    game: &Game<'_>,
) -> Made {
    refresh(game);
    let kind = MachineKind::PaintingMachine;
    let s = kind.slots();
    let [tank] = tanks else {
        return Made { per_tick: 0, required: 0, warnings: 0, active: None, extra: Vec::new() };
    };
    let item = mek_machines::stack_in(entity, s.input).cloned();
    with_rules(item.as_ref(), registry, |rules| {
        fill_chemical(entity, s.infusion.unwrap_or(0), tank, rules[0], registry, config);
    });
    let multiplier = config.machines.max_upgrade_multiplier;
    let (usage, _) = config.machines.of(kind);
    let required = saved.upgrades.ticks(pigment::PAINTING_TICKS, multiplier);
    let per_tick = saved.upgrades.energy_per_tick(usage, multiplier);
    let mut made = Made { per_tick, required, warnings: 0, active: None, extra: Vec::new() };
    let mut used = 0;
    match find(item.as_ref(), tank.stored.id(), registry) {
        None => {
            if item.is_some() || !tank.is_empty() {
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
            let max = game.native_max_stack_size(&recipe.output.item).unwrap_or(64);
            let room = machine::output_operations(held.map(|h| (h.count, mek_machines::same_kind(h, &recipe.output))), recipe.output.count, max);
            if room == 0 {
                made.warnings |= menu::WARN_OUTPUT;
            }
            if per_tick > store.stored {
                made.warnings |= menu::WARN_ENERGY;
            }
            let count = item.as_ref().map_or(0, |i| i.count);
            let inputs = Inputs::of(count, recipe.item.count).both(ore::chemical_inputs(tank.stored(), recipe.pigment.amount));
            let ran = machine::process(&mut saved.progress, required, store, per_tick, 1, inputs, room, true);
            used = ran.energy;
            if ran.finished {
                let n = ran.operations;
                mek_machines::shrink(entity, s.input, recipe.item.count * n);
                tank.shrink_stack(recipe.pigment.amount.saturating_mul(i64::from(n)), false);
                let add = recipe.output.count * n;
                match entity.items.get_mut(s.output) {
                    Some(Some(out)) => out.count += add,
                    Some(cell) => *cell = Some(ItemStack { count: add, ..recipe.output.clone() }),
                    None => {}
                }
            }
            made.active = Some(ran.operations > 0);
        }
    }
    made.extra = InfuserView { tanks: [number(tank, registry), (0, 0), (0, 0)], energy_used: used }.pack();
    made
}

// ---- The screen ------------------------------------------------------------------

/// `GuiPaintingMachine`: the pigment `GuiChemicalGauge` STANDARD at (25, 13), the
/// `LARGE_RIGHT` progress at (64, 39), the `GuiVerticalPowerBar` at (164, 15).
const GAUGE: Rect = [25.0, 13.0, 18.0, 60.0];
const PROGRESS: Rect = [64.0, 39.0, 48.0, 8.0];
pub(super) const POWER_BAR: Rect = [164.0, 15.0, 6.0, 54.0];

/// Mekanism's slots (`getInitialInventory`): the pigment slot (6, 56), the input
/// (45, 35), the output (116, 35), the energy slot (144, 35).
pub(super) fn slots() -> Vec<Option<[f32; 2]>> {
    vec![Some([6.0, 56.0]), Some([45.0, 35.0]), Some([116.0, 35.0]), Some([144.0, 35.0])]
}

pub(super) fn draw(pen: &mut Pen<'_, '_>, view: &MachineView, data: &[i32]) {
    let level = if view.capacity == 0 { 0.0 } else { view.stored as f64 / view.capacity as f64 };
    let extra = InfuserView::unpack(data).unwrap_or_default();
    pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], POWER_BAR);
    let h = (level * 52.0) as i32 as f32;
    if h > 0.0 {
        pen.sprite("bar/vertical_power", [0.0, 0.0, 4.0, h], [POWER_BAR[0] + 1.0, POWER_BAR[1] + POWER_BAR[3] - 1.0 - h, 4.0, h]);
    }
    mek_chem_machines::gauge(pen, GAUGE, extra.tanks[0].0, extra.tanks[0].1, pigment::PAINTING_TANK);
    let share = if view.required == 0 { 0.0 } else { f64::from(view.progress) / f64::from(view.required) };
    ore_glue::progress(pen, PROGRESS, share, "progress/large_right");
}

pub(super) fn tip(view: &MachineView, data: &[i32], at: [f32; 2]) -> Option<Vec<String>> {
    let extra = InfuserView::unpack(data)?;
    if mek_ui::inside(GAUGE, at) {
        return Some(vec![mek_chem_machines::tank_line(extra.tanks[0].0, extra.tanks[0].1)]);
    }
    let share = if view.required == 0 { 0.0 } else { f64::from(view.progress) / f64::from(view.required) };
    mek_ui::inside(PROGRESS, at).then(|| vec![format!("Progress: {}%", (share * 100.0).floor())])
}
