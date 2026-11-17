// Mekanism's Nutritional Liquifier in the game (support module `crate::game::mek_liquifier` of the
// `mekanism` native bundle). Its rules are the bundle's crate (`mekanism_energy::liquifier` and
// `machine`, MIT, from Mekanism 10.7.19, Copyright (c) 2017-2025 Aidan C. Brady); this module is its
// part of the glue, reached through `mek_ore_machines` as the painting machine is: its recipe, which
// Mekanism builds from the food in the slot (`TileEntityNutritionalLiquifier.getRecipe`, read from the
// engine's food table: `native_food`, hook points API 436), its tick after the shared one
// (`mek_machines`: upgrades, the energy slot), its fluid slot and its screen
// (`GuiNutritionalLiquifier`). Its paste leaves by the rotary's fluid ejector (`mek_rotary::eject_fluid`,
// out of the right) and through its fluid handler (`mek_machines`). Block entities, energy, side
// configuration and `/mek probe` are `mek_machines`' as for the other machines.

use super::{
    Game,
    mek_chem_machines::{self, Made},
    mek_machines::{self, Saved},
    mek_ore_machines::{self as ore_glue, drain_fluid_tank, fluid_view},
    mek_ui::{self, Pen, Rect},
};
use horizon_client::{containers::ModBlockEntity, items::ItemStack};
use horizon_world::fluid_stack::FluidStack;
use mekanism_energy::{
    config::EnergyConfig,
    container::EnergyContainer,
    liquifier,
    machine::{self, Inputs, MachineKind},
    menu::{self, InfuserView, MachineView},
};
use std::{collections::HashSet, sync::RwLock};

/// The fluid slot's output (`getInitialInventory`: input, output, fluid slot, its output, energy).
const FLUID_OUT: usize = 3;

/// The foods with nutrition (`isValidInput`), for the slot validator asked with no world at
/// hand; none until a world's table is read.
static TAKES: RwLock<Option<HashSet<String>>> = RwLock::new(None);

/// Reads the food table once a world is at hand.
fn refresh(game: &Game<'_>) {
    if TAKES.read().is_ok_and(|t| t.is_some()) {
        return;
    }
    let foods: HashSet<String> = game.native_food_table().into_iter().filter(|f| f.nutrition > 0).map(|f| f.item).collect();
    if !foods.is_empty()
        && let Ok(mut t) = TAKES.write()
    {
        *t = Some(foods);
    }
}

pub(super) fn clear() {
    if let Ok(mut t) = TAKES.write() {
        *t = None;
    }
}

/// The number of foods it takes, for the probe.
pub(super) fn recipe_count() -> usize {
    TAKES.read().ok().and_then(|t| t.as_ref().map(HashSet::len)).unwrap_or(0)
}

/// The input slot takes a food with nutrition, the fluid slot a bucket or a filled one
/// (`FluidInventorySlot.drain`), the outputs nothing.
pub(super) fn slot_takes(slot: usize, stack: &ItemStack) -> bool {
    let s = MachineKind::NutritionalLiquifier.slots();
    let item = &*stack.item;
    if slot == s.input {
        TAKES.read().ok().and_then(|t| t.as_ref().map(|t| t.contains(item))).unwrap_or(true)
    } else if Some(slot) == s.output2 {
        item == "minecraft:bucket" || horizon_world::fluid_stack::bucket_fluid(item).is_some()
    } else {
        false
    }
}

/// The slots hoppers reach: the input, the fluid slot and the energy slot in, the outputs out.
pub(super) fn hopper_slots() -> (Vec<usize>, Vec<usize>) {
    let s = MachineKind::NutritionalLiquifier.slots();
    (vec![s.input, s.output2.unwrap_or(2), s.energy], vec![s.output, FLUID_OUT])
}

/// `TileEntityNutritionalLiquifier.onUpdateServer`: the tank fills the container in the fluid
/// slot (`drainTank`), then the food's recipe over its ticks (`machine::process`, one operation),
/// the food taken, its paste made and its remainder put out on the last tick.
pub(super) fn tick(
    game: &Game<'_>,
    entity: &mut ModBlockEntity,
    saved: &mut Saved,
    store: &mut EnergyContainer,
    memo: &mut Option<String>,
    config: &EnergyConfig,
) -> Made {
    refresh(game);
    let kind = MachineKind::NutritionalLiquifier;
    let s = kind.slots();
    let mut fluid = super::mek_world::saved_tank(&entity.nbt);
    let before = fluid.clone();
    drain_fluid_tank(game, entity, s.output2.unwrap_or(2), FLUID_OUT, &mut fluid);
    let multiplier = config.machines.max_upgrade_multiplier;
    let (usage, _) = config.machines.of(kind);
    let required = saved.upgrades.ticks(liquifier::TICKS, multiplier);
    let per_tick = saved.upgrades.energy_per_tick(usage, multiplier);
    let mut made = Made { per_tick, required, warnings: 0, active: None, extra: Vec::new() };
    let item = mek_machines::stack_in(entity, s.input).cloned();
    let recipe = item.as_ref().and_then(|i| {
        let food = game.native_food(i)?;
        liquifier::liquify(food.nutrition, food.remainder.as_deref()).map(|r| (liquifier::recipe_id(&i.item), r))
    });
    let mut used = 0;
    match recipe {
        None => {
            if item.is_some() {
                made.warnings |= menu::WARN_NO_RECIPE;
            }
            if memo.is_some() {
                saved.progress = 0;
                made.active = Some(false);
            }
        }
        Some((id, recipe)) => {
            if memo.as_deref() != Some(id.as_str()) {
                saved.progress = if memo.is_none() { machine::resumed(saved.progress, required) } else { 0 };
                *memo = Some(id);
            }
            let held = (!fluid.is_empty()).then(|| (fluid.name(), fluid.amount()));
            let mut room = liquifier::paste_room(held, recipe.paste);
            if let Some(rest) = &recipe.remainder {
                let out = ItemStack::new(rest.as_str(), 1);
                let max = game.native_max_stack_size(rest).unwrap_or(64);
                let there = mek_machines::stack_in(entity, s.output).map(|h| (h.count, mek_machines::same_kind(h, &out)));
                room = room.min(machine::output_operations(there, 1, max));
            }
            if room == 0 {
                made.warnings |= menu::WARN_OUTPUT;
            }
            if per_tick > store.stored {
                made.warnings |= menu::WARN_ENERGY;
            }
            let count = item.as_ref().map_or(0, |i| i.count);
            let ran = machine::process(&mut saved.progress, required, store, per_tick, 1, Inputs::of(count, 1), room, true);
            used = ran.energy;
            if ran.finished {
                for _ in 0..ran.operations {
                    mek_machines::shrink(entity, s.input, 1);
                    let amount = fluid.amount().max(0).saturating_add(recipe.paste);
                    if let Some(paste) = FluidStack::of(liquifier::PASTE, amount) {
                        fluid = paste;
                    }
                    if let Some(rest) = &recipe.remainder {
                        ore_glue::put_one(game, entity, s.output, ItemStack::new(rest.as_str(), 1));
                    }
                }
            }
            made.active = Some(ran.operations > 0);
        }
    }
    if fluid != before {
        super::mek_world::write_tank(&mut entity.nbt, &fluid);
    }
    made.extra = InfuserView { tanks: [fluid_view(&fluid), (0, 0), (0, 0)], energy_used: used }.pack();
    made
}

// ---- The screen ------------------------------------------------------------------

/// `GuiNutritionalLiquifier`: the `GuiFluidGauge` STANDARD at (133, 13), the `GuiDownArrow` at
/// (159, 44), the `LARGE_RIGHT` progress at (54, 40); the `GuiHorizontalPowerBar` at (115, 75) is
/// the chemical machines' (`mek_chem_machines::POWER_BAR`).
const GAUGE: Rect = [133.0, 13.0, 18.0, 60.0];
const ARROW: Rect = [159.0, 44.0, 8.0, 9.0];
const PROGRESS: Rect = [54.0, 40.0, 48.0, 8.0];

/// Mekanism's slots (`getInitialInventory`): the input (26, 36), the output (110, 36), the fluid
/// slot (155, 25) and its output (155, 56), the energy slot (155, 5).
pub(super) fn slots() -> Vec<Option<[f32; 2]>> {
    vec![Some([26.0, 36.0]), Some([110.0, 36.0]), Some([155.0, 25.0]), Some([155.0, 56.0]), Some([155.0, 5.0])]
}

pub(super) fn draw(pen: &mut Pen<'_, '_>, view: &MachineView, data: &[i32]) {
    let level = if view.capacity == 0 { 0.0 } else { view.stored as f64 / view.capacity as f64 };
    let bar = mek_chem_machines::POWER_BAR;
    pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], bar);
    let w = (level * 52.0) as i32 as f32;
    if w > 0.0 {
        pen.sprite("bar/horizontal_power", [0.0, 0.0, w, 4.0], [bar[0] + 1.0, bar[1] + 1.0, w, 4.0]);
    }
    let extra = InfuserView::unpack(data).unwrap_or_default();
    ore_glue::fluid_gauge(pen, GAUGE, extra.tanks[0].0, extra.tanks[0].1, liquifier::TANK);
    pen.whole("down_arrow", [ARROW[2], ARROW[3]], ARROW);
    let share = if view.required == 0 { 0.0 } else { f64::from(view.progress) / f64::from(view.required) };
    ore_glue::progress(pen, PROGRESS, share, "progress/large_right");
}

pub(super) fn tip(view: &MachineView, data: &[i32], at: [f32; 2]) -> Option<Vec<String>> {
    let extra = InfuserView::unpack(data)?;
    if mek_ui::inside(GAUGE, at) {
        return Some(vec![ore_glue::fluid_line(extra.tanks[0].0, extra.tanks[0].1)]);
    }
    let share = if view.required == 0 { 0.0 } else { f64::from(view.progress) / f64::from(view.required) };
    mek_ui::inside(PROGRESS, at).then(|| vec![format!("Progress: {}%", (share * 100.0).floor())])
}
