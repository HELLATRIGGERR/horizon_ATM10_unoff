//! `CrafterComponent` (machines/components/CrafterComponent.java): the processing loop every MI crafting
//! machine runs each server tick, line for line where it matters: when a recipe starts, how much EU a tick
//! it takes (the efficiency ramp, `getRecipeMaxEu`), when outputs land, how output slots are reserved
//! (machine locks), and the 100-tick recheck when the inventory doesn't change.
//!
//! Left out: player statistics and advancements (`getStatsOrDummy`), process conditions (no recipe of the
//! ported types has one), overdrive's extra draw (no overdrive module yet).

use crate::inventory::{FluidSlot, Inventory, ItemSlot};
use crate::recipe::{MachineRecipe, RecipeBook};
use std::rc::Rc;

/// What the machine gives the crafter (`CrafterComponent.Behavior`).
pub trait Behavior {
    /// Redstone control allows it.
    fn is_enabled(&self) -> bool {
        true
    }
    /// `consumeEu(max, simulation)`: EU taken, at most `max`.
    fn consume_eu(&mut self, max: i64, act: bool) -> i64;
    fn base_recipe_eu(&self) -> i64;
    fn max_recipe_eu(&self) -> i64;
    /// `banRecipe`: a recipe needing more EU a tick than the machine can give.
    fn ban_recipe(&self, recipe: &MachineRecipe) -> bool {
        i64::from(recipe.eu) > self.max_recipe_eu()
    }
    fn is_overdriving(&self) -> bool {
        false
    }
    fn max_fluid_outputs(&self) -> usize {
        usize::MAX
    }
    fn one_fluid_input_per_stack(&self) -> bool {
        false
    }
    /// An item's stack size.
    fn max_stack(&self, item: &str) -> u32;
    /// `ThreadLocalRandom.current().nextFloat()`.
    fn next_float(&mut self) -> f32;
}

/// The component's saved state; `writeNbt`/`readNbt`'s fields, in MI's names.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Crafter {
    pub used_energy: i64,
    pub recipe_energy: i64,
    pub recipe_max_eu: i64,
    /// The active recipe's id (`activeRecipe`), looked up again in the book each tick
    /// (`loadDelayedActiveRecipe`).
    pub active_recipe: Option<String>,
    pub efficiency_ticks: i32,
    pub max_efficiency_ticks: i32,
    // Not saved by MI.
    pub matches_multiple_recipes: bool,
    pub previous_base_eu: i64,
    pub previous_max_eu: i64,
    pub last_inv_hash: i32,
    pub last_forced_tick: i32,
}

impl Crafter {
    pub fn new() -> Self {
        Crafter { previous_base_eu: -1, previous_max_eu: -1, ..Self::default() }
    }

    /// `readNbt`: a used energy with no recipe is reset, as MI does.
    pub fn loaded(mut self) -> Self {
        if self.active_recipe.is_none() && self.used_energy > 0 {
            self.used_energy = 0;
        }
        self.previous_base_eu = -1;
        self.previous_max_eu = -1;
        self
    }

    /// `getProgress`.
    pub fn progress(&self) -> f32 {
        if self.recipe_energy == 0 { 0.0 } else { self.used_energy as f32 / self.recipe_energy as f32 }
    }

    pub fn has_active_recipe(&self) -> bool {
        self.active_recipe.is_some()
    }

    /// `tickRecipe`: one server tick; true when the machine worked (`isActive`).
    pub fn tick(&mut self, book: &RecipeBook, inv: &mut Inventory, behavior: &mut impl Behavior) -> bool {
        let enabled = behavior.is_enabled();
        let mut active = self.load_active(book);
        let mut started = false;
        if self.used_energy == 0 && enabled && behavior.consume_eu(1, false) == 1 {
            started = self.update_active_recipe(book, inv, behavior, &mut active);
        }
        if active.is_some() {
            self.last_forced_tick = 0;
        }
        let mut eu = 0;
        let mut finished = false;
        let is_active;
        match active.as_ref().filter(|_| enabled) {
            Some(recipe) if self.used_energy > 0 || started => {
                self.recipe_max_eu = self.recipe_max_eu_for(i64::from(recipe.eu), self.recipe_energy, self.efficiency_ticks, behavior);
                eu = behavior.consume_eu(self.recipe_max_eu.min(self.recipe_energy - self.used_energy), true);
                is_active = eu > 0;
                self.used_energy += eu;
                if self.used_energy == self.recipe_energy {
                    put_item_outputs(recipe, inv, false, false, behavior);
                    put_fluid_outputs(recipe, inv, false, false, behavior);
                    clear_locks(inv);
                    self.used_energy = 0;
                    finished = true;
                }
            }
            Some(_) if behavior.is_overdriving() => {
                eu = behavior.consume_eu(self.recipe_max_eu, true);
                is_active = eu > 0;
            }
            _ => is_active = false,
        }
        if let Some(recipe) = &active
            && (self.previous_base_eu != behavior.base_recipe_eu() || self.previous_max_eu != behavior.max_recipe_eu())
        {
            self.previous_base_eu = behavior.base_recipe_eu();
            self.previous_max_eu = behavior.max_recipe_eu();
            self.max_efficiency_ticks = self.max_efficiency_ticks_for(recipe, behavior);
            self.efficiency_ticks = self.efficiency_ticks.min(self.max_efficiency_ticks);
        }
        if finished {
            if self.efficiency_ticks < self.max_efficiency_ticks {
                self.efficiency_ticks += 1;
            }
        } else if eu < self.recipe_max_eu && self.efficiency_ticks > 0 {
            self.efficiency_ticks -= 1;
        }
        self.active_recipe = active.map(|r| r.id.clone());
        self.clear_active_recipe_if_possible();
        is_active
    }

    /// `tryContinueRecipe`: a multiblock matched again resumes its recipe once the outputs fit, and reserves their
    /// slots again; true with nothing to resume.
    pub fn try_continue(&mut self, book: &RecipeBook, inv: &mut Inventory, behavior: &mut impl Behavior) -> bool {
        let Some(recipe) = self.load_active(book) else {
            return true;
        };
        if put_item_outputs(&recipe, inv, true, false, behavior) && put_fluid_outputs(&recipe, inv, true, false, behavior) {
            put_item_outputs(&recipe, inv, true, true, behavior);
            put_fluid_outputs(&recipe, inv, true, true, behavior);
            true
        } else {
            false
        }
    }

    /// `decreaseEfficiencyTicks`.
    pub fn decrease_efficiency_ticks(&mut self) {
        self.efficiency_ticks = (self.efficiency_ticks - 1).max(0);
        self.clear_active_recipe_if_possible();
    }

    fn clear_active_recipe_if_possible(&mut self) {
        if self.efficiency_ticks == 0 && self.used_energy == 0 {
            self.active_recipe = None;
        }
    }

    /// `loadDelayedActiveRecipe`: a recipe gone from the world resets the work.
    fn load_active(&mut self, book: &RecipeBook) -> Option<Rc<MachineRecipe>> {
        let id = self.active_recipe.as_deref()?;
        let found = book.get(id).cloned();
        if found.is_none() {
            self.efficiency_ticks = 0;
            self.used_energy = 0;
            self.active_recipe = None;
        }
        found
    }

    /// `shouldUpdateActiveRecipe`: an unchanged inventory is looked at again every 100 ticks only.
    fn should_update_active_recipe(&mut self, inv: &Inventory) -> bool {
        let hash = inv.hash();
        if hash == self.last_inv_hash {
            if self.last_forced_tick == 0 {
                self.last_forced_tick = 100;
            } else {
                self.last_forced_tick -= 1;
                return false;
            }
        } else {
            self.last_inv_hash = hash;
        }
        true
    }

    /// `updateActiveRecipe`.
    fn update_active_recipe(
        &mut self,
        book: &RecipeBook,
        inv: &mut Inventory,
        behavior: &mut impl Behavior,
        active: &mut Option<Rc<MachineRecipe>>,
    ) -> bool {
        if !self.should_update_active_recipe(inv) {
            return false;
        }
        let outputs_locked = all_locked_items(&inv.item_outputs) && all_locked_fluids(&inv.fluid_outputs);
        // `getRecipes()`: the active one alone while the machine is still warm.
        let recipes: Vec<Rc<MachineRecipe>> = match active.as_ref().filter(|_| self.efficiency_ticks > 0) {
            Some(r) => vec![r.clone()],
            None => book.candidates(inv.item_inputs.iter().filter(|s| !s.is_blank()).filter_map(|s| s.item.as_deref())),
        };
        let mut picked: Option<Rc<MachineRecipe>> = None;
        for recipe in recipes {
            if behavior.ban_recipe(&recipe) {
                continue;
            }
            if can_start_recipe(&recipe, inv, behavior) {
                if let Some(first) = &picked {
                    if overlaps(inv, first, &recipe) {
                        self.matches_multiple_recipes = true;
                        return false;
                    }
                    continue;
                }
                picked = Some(recipe);
                if outputs_locked {
                    break;
                }
            }
        }
        self.matches_multiple_recipes = false;
        if let Some(recipe) = picked
            && try_start_recipe(&recipe, inv, behavior)
        {
            let same = active.as_ref().is_some_and(|a| Rc::ptr_eq(a, &recipe));
            if !same || self.efficiency_ticks == 0 {
                self.max_efficiency_ticks = self.max_efficiency_ticks_for(&recipe, behavior);
            }
            self.used_energy = 0;
            self.recipe_energy = recipe.total_eu();
            self.recipe_max_eu = self.recipe_max_eu_for(i64::from(recipe.eu), self.recipe_energy, self.efficiency_ticks, behavior);
            *active = Some(recipe);
            return true;
        }
        false
    }

    /// `getRecipeMaxEu`: the base rate, plus a 600th of the recipe's total for each efficiency tick, up to
    /// the machine's maximum and the whole recipe.
    pub fn recipe_max_eu_for(&self, recipe_eu: i64, total_eu: i64, efficiency_ticks: i32, behavior: &impl Behavior) -> i64 {
        recipe_max_eu(behavior.base_recipe_eu(), behavior.max_recipe_eu(), recipe_eu, total_eu, efficiency_ticks)
    }

    fn max_efficiency_ticks_for(&self, recipe: &MachineRecipe, behavior: &impl Behavior) -> i32 {
        max_efficiency_ticks(behavior.base_recipe_eu(), behavior.max_recipe_eu(), recipe)
    }
}

/// `getRecipeMaxEu`, with the machine's base and maximum EU a tick.
pub fn recipe_max_eu(base: i64, max: i64, recipe_eu: i64, total_eu: i64, efficiency_ticks: i32) -> i64 {
    let base_eu = base.max(recipe_eu);
    let overclocked = base_eu + i64::from(efficiency_ticks) * total_eu / (20 * 30);
    total_eu.min(overclocked.min(max))
}

/// `getRecipeMaxEfficiencyTicks`: the first efficiency at which the recipe runs at full speed.
pub fn max_efficiency_ticks(base: i64, max: i64, recipe: &MachineRecipe) -> i32 {
    let (eu, total) = (i64::from(recipe.eu), recipe.total_eu());
    let full = max.min(total);
    let mut ticks = 0;
    while recipe_max_eu(base, max, eu, total, ticks) != full {
        ticks += 1;
        if ticks == i32::MAX {
            break;
        }
    }
    ticks
}

/// `areAllSlotsLocked`: every slot (all of MI's standard slots are player-lockable) locked by the player.
fn all_locked_items(slots: &[ItemSlot]) -> bool {
    slots.iter().all(|s| s.player_locked)
}

fn all_locked_fluids(slots: &[FluidSlot]) -> bool {
    slots.iter().all(|s| s.player_locked)
}

/// `overlaps`: two recipes that both take a stack in the input slots.
fn overlaps(inv: &Inventory, first: &MachineRecipe, second: &MachineRecipe) -> bool {
    for stack in inv.fluid_inputs.iter().filter_map(|s| s.fluid.as_deref()) {
        if first.fluid_inputs.iter().any(|i| i.fluids.iter().any(|f| f == stack))
            && second.fluid_inputs.iter().any(|i| i.fluids.iter().any(|f| f == stack))
        {
            return true;
        }
    }
    for stack in inv.item_inputs.iter().filter_map(|s| s.item.as_deref()) {
        if first.item_inputs.iter().any(|i| i.matches(stack)) && second.item_inputs.iter().any(|i| i.matches(stack)) {
            return true;
        }
    }
    false
}

fn can_start_recipe(r: &MachineRecipe, inv: &mut Inventory, b: &mut impl Behavior) -> bool {
    take_item_inputs(r, inv, true, b) && take_fluid_inputs(r, inv, true, b) && put_item_outputs(r, inv, true, false, b) && put_fluid_outputs(r, inv, true, false, b)
}

fn try_start_recipe(r: &MachineRecipe, inv: &mut Inventory, b: &mut impl Behavior) -> bool {
    if !can_start_recipe(r, inv, b) {
        return false;
    }
    take_item_inputs(r, inv, false, b);
    take_fluid_inputs(r, inv, false, b);
    put_item_outputs(r, inv, true, true, b);
    put_fluid_outputs(r, inv, true, true, b);
    true
}

/// `takeItemInputs`: each input from the slots in order; a chance input is skipped by a roll when acting.
pub fn take_item_inputs(r: &MachineRecipe, inv: &mut Inventory, simulate: bool, b: &mut impl Behavior) -> bool {
    let mut copy;
    let stacks = if simulate {
        copy = inv.item_inputs.clone();
        &mut copy
    } else {
        &mut inv.item_inputs
    };
    let mut ok = true;
    for input in &r.item_inputs {
        if !simulate && input.probability < 1.0 && b.next_float() >= input.probability {
            continue;
        }
        let mut remaining = input.amount;
        for stack in stacks.iter_mut() {
            if stack.count > 0 && stack.item.as_deref().is_some_and(|i| input.matches(i)) {
                let taken = stack.count.min(remaining);
                stack.decrement(taken);
                remaining -= taken;
                if remaining == 0 {
                    break;
                }
            }
        }
        if remaining > 0 {
            ok = false;
        }
    }
    ok
}

/// `takeFluidInputs`.
pub fn take_fluid_inputs(r: &MachineRecipe, inv: &mut Inventory, simulate: bool, b: &mut impl Behavior) -> bool {
    let one_per_stack = b.one_fluid_input_per_stack();
    let mut copy;
    let stacks = if simulate {
        copy = inv.fluid_inputs.clone();
        &mut copy
    } else {
        &mut inv.fluid_inputs
    };
    let mut used = vec![false; stacks.len()];
    let mut ok = true;
    for input in &r.fluid_inputs {
        if !simulate && input.probability < 1.0 && b.next_float() >= input.probability {
            continue;
        }
        let mut remaining = input.amount;
        for (i, stack) in stacks.iter_mut().enumerate() {
            if one_per_stack && used[i] {
                continue;
            }
            if stack.fluid.as_deref().is_some_and(|f| input.fluids.iter().any(|x| x == f)) {
                let taken = remaining.min(stack.amount);
                if taken > 0 {
                    used[i] = true;
                    stack.decrement(taken);
                    remaining -= taken;
                    if remaining == 0 {
                        break;
                    }
                }
            }
        }
        if remaining > 0 {
            ok = false;
        }
    }
    ok
}

/// `putItemOutputs`: each output first into slots already holding or reserved for it (run 0), then into
/// any empty slot (run 1). A chance output isn't checked for room when simulating. `toggle_lock` reserves
/// the slots the outputs went to (machine locks), as a recipe's start does.
pub fn put_item_outputs(r: &MachineRecipe, inv: &mut Inventory, simulate: bool, toggle_lock: bool, b: &mut impl Behavior) -> bool {
    let mut copy;
    let stacks = if simulate {
        copy = inv.item_outputs.clone();
        &mut copy
    } else {
        &mut inv.item_outputs
    };
    let mut locks: Vec<(usize, String)> = Vec::new();
    let mut ok = true;
    for output in &r.item_outputs {
        if output.probability < 1.0 {
            if simulate {
                continue;
            }
            if b.next_float() > output.probability {
                continue;
            }
        }
        let max_stack = b.max_stack(&output.item);
        let mut remaining = output.amount;
        'runs: for run in 0..2 {
            for (id, stack) in stacks.iter_mut().enumerate() {
                let blank = stack.is_blank();
                if stack.allowed_by_lock(&output.item) && (blank || stack.item.as_deref() == Some(&output.item)) {
                    let room = if simulate || output.probability < 1.0 || run == 0 {
                        stack.room_for(max_stack)
                    } else {
                        max_stack.saturating_sub(stack.count)
                    };
                    let mut ins = remaining.min(room);
                    if ins > 0 {
                        if blank {
                            if stack.machine_locked || stack.player_locked || run == 1 {
                                stack.count = ins;
                                stack.item = Some(output.item.clone());
                                stack.max_stack = max_stack;
                            } else {
                                ins = 0;
                            }
                        } else {
                            stack.count += ins;
                        }
                    }
                    remaining -= ins;
                    if ins > 0 {
                        locks.push((id, output.item.clone()));
                    }
                    if remaining == 0 {
                        break 'runs;
                    }
                }
            }
        }
        if remaining > 0 {
            ok = false;
        }
    }
    if toggle_lock {
        for (i, item) in locks {
            inv.item_outputs[i].enable_machine_lock(&item);
        }
    }
    ok
}

/// `putFluidOutputs`: each output into the first slot that holds its fluid, else the first empty one.
pub fn put_fluid_outputs(r: &MachineRecipe, inv: &mut Inventory, simulate: bool, toggle_lock: bool, b: &mut impl Behavior) -> bool {
    let limit = r.fluid_outputs.len().min(b.max_fluid_outputs());
    let mut copy;
    let stacks = if simulate {
        copy = inv.fluid_outputs.clone();
        &mut copy
    } else {
        &mut inv.fluid_outputs
    };
    let mut locks: Vec<(usize, String)> = Vec::new();
    let mut ok = true;
    for output in &r.fluid_outputs[..limit] {
        if output.probability < 1.0 {
            if simulate {
                continue;
            }
            if b.next_float() > output.probability {
                continue;
            }
        }
        let mut placed = false;
        'tries: for tries in 0..2 {
            for (j, stack) in stacks.iter_mut().enumerate() {
                if stack.allowed_by_lock(&output.fluid)
                    && ((tries == 1 && stack.is_blank()) || stack.fluid.as_deref() == Some(&output.fluid))
                {
                    let inserted = output.amount.min(stack.remaining_space());
                    if inserted > 0 {
                        stack.fluid = Some(output.fluid.clone());
                        stack.amount += inserted;
                        locks.push((j, output.fluid.clone()));
                    }
                    if inserted < output.amount {
                        ok = false;
                    }
                    placed = true;
                    break 'tries;
                }
            }
        }
        if !placed {
            ok = false;
        }
    }
    if toggle_lock {
        for (i, fluid) in locks {
            inv.fluid_outputs[i].enable_machine_lock(&fluid);
        }
    }
    ok
}

/// `clearLocks`: a finished recipe frees the slots it reserved.
pub fn clear_locks(inv: &mut Inventory) {
    for s in &mut inv.item_outputs {
        if s.machine_locked {
            s.disable_machine_lock();
        }
    }
    for s in &mut inv.fluid_outputs {
        if s.machine_locked {
            s.disable_machine_lock();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe::{FluidInput, ItemInput, ItemOutput};

    /// An LV electric machine (`MachineTier.LV`: base 8, max 32) with `eu` stored.
    struct Lv {
        eu: i64,
        rolls: Vec<f32>,
    }
    impl Behavior for Lv {
        fn consume_eu(&mut self, max: i64, act: bool) -> i64 {
            let ext = max.min(self.eu);
            if act {
                self.eu -= ext;
            }
            ext
        }
        fn base_recipe_eu(&self) -> i64 {
            8
        }
        fn max_recipe_eu(&self) -> i64 {
            32
        }
        fn max_stack(&self, _: &str) -> u32 {
            64
        }
        fn next_float(&mut self) -> f32 {
            self.rolls.pop().unwrap_or(0.0)
        }
    }

    fn macerate(id: &str, input: &str, outputs: &[(&str, u32, f32)]) -> MachineRecipe {
        MachineRecipe {
            id: id.into(),
            eu: 2,
            duration: 100,
            item_inputs: vec![ItemInput { items: vec![input.into()], amount: 1, probability: 1.0 }],
            fluid_inputs: vec![],
            item_outputs: outputs.iter().map(|(i, n, p)| ItemOutput { item: (*i).into(), amount: *n, probability: *p }).collect(),
            fluid_outputs: vec![],
        }
    }
    fn macerator_inv(input: &str, count: u32) -> Inventory {
        Inventory {
            item_inputs: vec![ItemSlot::new(Some(input), count, 64)],
            item_outputs: (0..4).map(|_| ItemSlot::new(None, 0, 64)).collect(),
            ..Inventory::default()
        }
    }
    fn outputs(inv: &Inventory) -> Vec<(Option<String>, u32)> {
        inv.item_outputs.iter().map(|s| (s.item.clone(), s.count)).collect()
    }

    #[test]
    fn efficiency_ramp_numbers() {
        // 2 EU/t x 100 t = 200 EU: an LV machine starts at its base 8 EU/t, and reaches its max 32 after
        // 72 crafts (8 + 72 * 200 / 600 = 32).
        let r = macerate("r", "a:ore", &[("a:dust", 1, 1.0)]);
        assert_eq!(recipe_max_eu(8, 32, 2, 200, 0), 8);
        assert_eq!(recipe_max_eu(8, 32, 2, 200, 3), 9);
        assert_eq!(max_efficiency_ticks(8, 32, &r), 72);
        // 2 EU/t x 10 t = 20 EU ramps by a 30th of an EU a craft: 360 crafts to run it whole in a tick.
        let mut short = r.clone();
        short.duration = 10;
        assert_eq!(max_efficiency_ticks(8, 32, &short), 360);
        // A recipe of 4 EU in all runs in one tick from the start: no ramp.
        short.duration = 2;
        assert_eq!(recipe_max_eu(8, 32, 2, 4, 0), 4);
        assert_eq!(max_efficiency_ticks(8, 32, &short), 0);
    }

    #[test]
    fn one_macerator_recipe_takes_25_ticks_at_8_eu() {
        let book = RecipeBook::new(vec![macerate("mi:copper", "a:ore", &[("a:dust", 2, 1.0)])]);
        let mut inv = macerator_inv("a:ore", 2);
        let mut b = Lv { eu: 3200, rolls: vec![] };
        let mut c = Crafter::new();
        // Tick 1 starts the recipe: the ore is taken and the output slot reserved.
        assert!(c.tick(&book, &mut inv, &mut b));
        assert_eq!(inv.item_inputs[0].count, 1);
        assert_eq!(c.used_energy, 8);
        assert_eq!(inv.item_outputs[0].locked.as_deref(), Some("a:dust"));
        for _ in 2..25 {
            assert!(c.tick(&book, &mut inv, &mut b));
        }
        assert_eq!(c.used_energy, 192);
        assert_eq!(outputs(&inv)[0], (None, 0));
        assert!(c.tick(&book, &mut inv, &mut b)); // tick 25
        assert_eq!(outputs(&inv)[0], (Some("a:dust".into()), 2));
        assert_eq!(inv.item_outputs[0].locked, None);
        assert_eq!((c.used_energy, c.efficiency_ticks, c.max_efficiency_ticks), (0, 1, 72));
        assert_eq!(b.eu, 3200 - 200);
        // The next ore starts on the next tick, still at 8 EU/t (1 * 200 / 600 = 0).
        assert!(c.tick(&book, &mut inv, &mut b));
        assert_eq!((c.used_energy, c.recipe_max_eu, inv.item_inputs[0].count), (8, 8, 0));
        for _ in 0..24 {
            c.tick(&book, &mut inv, &mut b);
        }
        assert_eq!(outputs(&inv)[0], (Some("a:dust".into()), 4));
        assert_eq!(c.efficiency_ticks, 2);
        // Nothing left: the efficiency falls one a tick, then the recipe is forgotten.
        assert!(!c.tick(&book, &mut inv, &mut b));
        assert_eq!((c.efficiency_ticks, c.active_recipe.as_deref()), (1, Some("mi:copper")));
        assert!(!c.tick(&book, &mut inv, &mut b));
        assert_eq!((c.efficiency_ticks, c.active_recipe.as_deref()), (0, None));
        assert_eq!(b.eu, 3200 - 400);
    }

    #[test]
    fn no_energy_no_start_and_partial_energy_stalls() {
        let book = RecipeBook::new(vec![macerate("r", "a:ore", &[("a:dust", 1, 1.0)])]);
        let mut inv = macerator_inv("a:ore", 1);
        let mut b = Lv { eu: 0, rolls: vec![] };
        let mut c = Crafter::new();
        assert!(!c.tick(&book, &mut inv, &mut b));
        assert_eq!(inv.item_inputs[0].count, 1);
        b.eu = 20;
        assert!(c.tick(&book, &mut inv, &mut b));
        assert!(c.tick(&book, &mut inv, &mut b));
        assert!(c.tick(&book, &mut inv, &mut b)); // 4 of the last 8
        assert_eq!((c.used_energy, b.eu), (20, 0));
        assert!(!c.tick(&book, &mut inv, &mut b));
        assert_eq!(c.used_energy, 20);
    }

    #[test]
    fn full_output_blocks_start_and_recheck_waits_100_ticks() {
        let book = RecipeBook::new(vec![macerate("r", "a:ore", &[("a:dust", 1, 1.0)])]);
        let mut inv = macerator_inv("a:ore", 1);
        for s in &mut inv.item_outputs {
            *s = ItemSlot::new(Some("x:full"), 64, 64);
        }
        let mut b = Lv { eu: 3200, rolls: vec![] };
        let mut c = Crafter::new();
        assert!(!c.tick(&book, &mut inv, &mut b)); // hash changed: looked at
        assert!(!c.tick(&book, &mut inv, &mut b)); // same hash, forced tick 0 -> 100: looked at
        inv.item_outputs[3] = ItemSlot::new(None, 0, 64);
        // Same item counts in a different slot change the hash, so it starts at once.
        assert!(c.tick(&book, &mut inv, &mut b));
        assert_eq!(inv.item_inputs[0].count, 0);
        // A probability output isn't checked for room, and lands by its roll.
        let book = RecipeBook::new(vec![macerate("p", "a:ore", &[("a:dust", 1, 1.0), ("a:gold", 1, 0.25)])]);
        let mut inv = macerator_inv("a:ore", 1);
        let mut b = Lv { eu: 3200, rolls: vec![0.2] };
        let mut c = Crafter::new();
        for _ in 0..25 {
            c.tick(&book, &mut inv, &mut b);
        }
        assert_eq!(outputs(&inv)[..2], [(Some("a:dust".into()), 1), (Some("a:gold".into()), 1)]);
    }

    #[test]
    fn unchanged_inventory_is_rechecked_every_100_ticks() {
        let book = RecipeBook::new(vec![macerate("r", "a:ore", &[("a:dust", 1, 1.0)])]);
        let mut inv = macerator_inv("b:other", 1);
        let mut b = Lv { eu: 3200, rolls: vec![] };
        let mut c = Crafter::new();
        c.tick(&book, &mut inv, &mut b);
        c.tick(&book, &mut inv, &mut b);
        // Now 100 ticks pass without a look, even if a recipe would start (the slot changes item in place
        // keeping its hash is impossible here, so simulate: put the ore and restore the old hash).
        let old = c.last_inv_hash;
        inv.item_inputs[0] = ItemSlot::new(Some("a:ore"), 1, 64);
        c.last_inv_hash = inv.hash();
        let mut looked = 0;
        for t in 0..102 {
            if c.tick(&book, &mut inv, &mut b) {
                looked = t;
                break;
            }
        }
        assert_ne!(old, c.last_inv_hash);
        assert_eq!(looked, 100);
    }

    #[test]
    fn overlapping_recipes_start_nothing() {
        let book = RecipeBook::new(vec![macerate("one", "a:ore", &[("a:dust", 1, 1.0)]), macerate("two", "a:ore", &[("b:dust", 1, 1.0)])]);
        let mut inv = macerator_inv("a:ore", 1);
        let mut b = Lv { eu: 3200, rolls: vec![] };
        let mut c = Crafter::new();
        assert!(!c.tick(&book, &mut inv, &mut b));
        assert!(c.matches_multiple_recipes);
        assert_eq!(inv.item_inputs[0].count, 1);
    }

    #[test]
    fn banned_by_tier_and_fluid_inputs() {
        let mut hot = macerate("hot", "a:ore", &[("a:dust", 1, 1.0)]);
        hot.eu = 33;
        let book = RecipeBook::new(vec![hot]);
        let mut inv = macerator_inv("a:ore", 1);
        let mut b = Lv { eu: 3200, rolls: vec![] };
        assert!(!Crafter::new().tick(&book, &mut inv, &mut b));
        // The cutting machine: an item and 1 mB of lubricant.
        let mut cut = macerate("cut", "a:log", &[("a:plank", 6, 1.0)]);
        cut.duration = 200;
        cut.fluid_inputs = vec![FluidInput { fluids: vec!["mi:lubricant".into()], amount: 1, probability: 1.0 }];
        let book = RecipeBook::new(vec![cut]);
        let mut inv = macerator_inv("a:log", 1);
        inv.item_outputs.truncate(1);
        inv.fluid_inputs = vec![FluidSlot::standard(16_000)];
        let mut c = Crafter::new();
        assert!(!c.tick(&book, &mut inv, &mut b));
        inv.fluid_inputs[0].fluid = Some("mi:lubricant".into());
        inv.fluid_inputs[0].amount = 1;
        assert!(c.tick(&book, &mut inv, &mut b));
        assert!(inv.fluid_inputs[0].is_blank());
        for _ in 1..50 {
            c.tick(&book, &mut inv, &mut b);
        }
        assert_eq!(outputs(&inv)[0], (Some("a:plank".into()), 6));
    }

    #[test]
    fn machine_lock_reserves_an_empty_slot_on_run_zero() {
        // A slot reserved for an output takes it on run 0 even though it is empty.
        let r = macerate("r", "a:ore", &[("a:dust", 1, 1.0)]);
        let mut inv = macerator_inv("a:ore", 1);
        let mut b = Lv { eu: 0, rolls: vec![] };
        inv.item_outputs[2].enable_machine_lock("a:dust");
        assert!(put_item_outputs(&r, &mut inv, false, false, &mut b));
        assert_eq!(outputs(&inv)[2], (Some("a:dust".into()), 1));
        assert_eq!(outputs(&inv)[0], (None, 0));
        // A slot locked to another item takes nothing.
        let mut inv = macerator_inv("a:ore", 1);
        for s in &mut inv.item_outputs {
            s.enable_machine_lock("z:z");
        }
        assert!(!put_item_outputs(&r, &mut inv, true, false, &mut b));
    }
}
