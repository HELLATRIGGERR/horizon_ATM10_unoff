//! `AtomicForgeBlockEntity`: the recipe's time is the size of the energy buffer (`energyPerTick` times the time, and 1 with no
//! recipe), the buffer takes energy only from the laser arm (`maxEnergyInsertion` 0), and when it is full the craft
//! happens in one tick and the buffer empties.
use crate::{
    machine::Spec,
    process::{can_output, craft_once, Ctx, Machine},
    recipe::Stack,
};

/// One server tick (`MachineBlockEntity.serverTick` with the forge's overrides). `m.energy.capacity` is set from the recipe
/// found (`getRecipe`), and a full buffer crafts. True when anything changed.
pub fn tick(m: &mut Machine, spec: &Spec, slots: &mut [Option<Stack>], ctx: &Ctx<'_>) -> bool {
    let before = (m.clone(), slots.to_vec());
    if ctx.active && !m.disabled_via_redstone {
        let candidate = m.recipe(spec, slots, ctx.book).cloned();
        match &candidate {
            Some(recipe) => m.energy.capacity = ctx.energy_per_tick * i64::from(recipe.time),
            None => {
                m.energy.capacity = 1;
                m.energy.amount = 0;
                m.current = None;
            }
        }
        if let Some(recipe) = candidate {
            let full = m.energy.capacity > 10 && m.energy.amount >= m.energy.capacity;
            if full && can_output(spec, slots, &recipe, ctx.max_stack) {
                m.current = Some(recipe.id.clone());
                m.energy.amount = 0;
                craft_once(spec, slots, &recipe);
                m.progress = 0;
            }
        }
    }
    (m.clone(), slots.to_vec()) != before
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        energy::Energy,
        machine::ATOMIC_FORGE,
        recipe::{Ingredient, Recipe, RecipeBook},
    };

    fn book() -> RecipeBook {
        RecipeBook::new(vec![Recipe {
            id: "compensator".into(),
            kind: "oritech:atomic_forge".into(),
            time: 5,
            inputs: vec![Ingredient { items: vec!["a:unit".into()] }, Ingredient { items: vec!["a:wafer".into()] }, Ingredient { items: vec!["a:wafer".into()] }],
            results: vec![Stack::new("a:engine", 1)],
            fluids: false,
            fluid_in: None,
            fluid_out: Vec::new(),
        }])
    }

    fn ctx(book: &RecipeBook) -> Ctx<'_> {
        Ctx { book, energy_per_tick: 1024, burst_speed_multiplier: 8.0, burst_throttle_multiplier: 1.2, game_time: 0, active: true, max_stack: &|_| 64, compact: &|_, _| false }
    }

    fn st(item: &str, count: u32) -> Option<Stack> {
        Some(Stack::new(item, count))
    }

    /// A recipe of 5 ticks sizes the buffer at 5 x 1024; the laser fills it, the craft comes the tick it is full and
    /// the buffer empties.
    #[test]
    fn the_recipe_sizes_the_buffer_and_a_full_one_crafts() {
        let book = book();
        let c = ctx(&book);
        let mut m = Machine::new(Energy { amount: 0, capacity: 1024, max_insert: 0, max_extract: 0 });
        let mut slots = vec![st("a:unit", 1), st("a:wafer", 1), st("a:wafer", 1), None];
        assert!(m.tick_forge(&ATOMIC_FORGE, &mut slots, &c));
        assert_eq!(m.energy.capacity, 5 * 1024);
        m.energy.amount = 5 * 1024 - 1;
        m.tick_forge(&ATOMIC_FORGE, &mut slots, &c);
        assert_eq!(slots[3], None, "one short: nothing yet");
        m.energy.amount = 5 * 1024;
        assert!(m.tick_forge(&ATOMIC_FORGE, &mut slots, &c));
        assert_eq!(slots, vec![None, None, None, st("a:engine", 1)]);
        assert_eq!(m.energy.amount, 0);
    }

    /// With no recipe the buffer is 1 and the energy a laser gave is lost.
    #[test]
    fn without_a_recipe_the_buffer_is_one_and_empty() {
        let book = book();
        let c = ctx(&book);
        let mut m = Machine::new(Energy { amount: 700, capacity: 5120, max_insert: 0, max_extract: 0 });
        let mut slots = vec![st("a:unit", 1), None, None, None];
        m.tick_forge(&ATOMIC_FORGE, &mut slots, &c);
        assert_eq!((m.energy.capacity, m.energy.amount), (1, 0));
    }

    #[test]
    fn a_blocked_output_waits_with_a_full_buffer() {
        let book = book();
        let c = ctx(&book);
        let mut m = Machine::new(Energy { amount: 5 * 1024, capacity: 5 * 1024, max_insert: 0, max_extract: 0 });
        let mut slots = vec![st("a:unit", 1), st("a:wafer", 1), st("a:wafer", 1), st("a:other", 1)];
        assert!(!m.tick_forge(&ATOMIC_FORGE, &mut slots, &c));
        assert_eq!(m.energy.amount, 5 * 1024);
    }
}

impl Machine {
    /// `tick` of the atomic forge.
    pub fn tick_forge(&mut self, spec: &Spec, slots: &mut [Option<Stack>], ctx: &Ctx<'_>) -> bool {
        tick(self, spec, slots, ctx)
    }
}
