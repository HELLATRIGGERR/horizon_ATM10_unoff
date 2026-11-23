//! The processing machine's tick: `MachineBlockEntity.serverTick` with `UpgradableMachineBlockEntity`'s burst ticks
//! and extra chambers, `getRecipe`, `canOutputRecipe`, `craftItem` (inputs taken one item an ingredient, spread over
//! the input slots) and the pulverizer's and fragment forge's `combineSmallDusts`.
use crate::{
    energy::Energy,
    machine::Spec,
    recipe::{Recipe, RecipeBook, Stack},
};

/// `MachineAddonController.BaseAddonData`: what a machine's addons add up to. A machine with none has the default.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AddonData {
    pub speed: f32,
    pub efficiency: f32,
    pub energy_bonus_capacity: i64,
    pub energy_bonus_transfer: i64,
    pub extra_chambers: i32,
    pub max_burst_ticks: i32,
}

impl Default for AddonData {
    /// `DEFAULT_ADDON_DATA`.
    fn default() -> Self {
        Self { speed: 1.0, efficiency: 1.0, energy_bonus_capacity: 0, energy_bonus_transfer: 0, extra_chambers: 0, max_burst_ticks: 0 }
    }
}

/// A machine's state between ticks (Java's fields; `current` is `currentRecipe`, `None` for its `DUMMY`).
#[derive(Clone, Debug, PartialEq)]
pub struct Machine {
    pub progress: i32,
    pub current: Option<String>,
    pub energy: Energy,
    pub disabled_via_redstone: bool,
    pub addons: AddonData,
    /// `remainingBurstTicks`.
    pub burst: i32,
    pub last_worked_at: i64,
}

impl Machine {
    pub fn new(energy: Energy) -> Self {
        Self { progress: 0, current: None, energy, disabled_via_redstone: false, addons: AddonData::default(), burst: 0, last_worked_at: 0 }
    }
}

/// What the world gives a tick.
pub struct Ctx<'a> {
    pub book: &'a RecipeBook,
    /// `energyPerTick` of the config.
    pub energy_per_tick: i64,
    pub burst_speed_multiplier: f32,
    pub burst_throttle_multiplier: f32,
    pub game_time: i64,
    /// `isActive`: a multiblock is assembled (always true for the pulverizer).
    pub active: bool,
    /// An item's `getMaxStackSize`.
    pub max_stack: &'a dyn Fn(&str) -> u32,
    /// What nine of an item make in a 3x3 crafting grid (`combineSmallDusts`' crafting lookup), if anything.
    pub compact: &'a dyn Fn(&str) -> Option<String>,
}

impl Machine {
    /// `getBurstBonus`.
    pub fn burst_bonus(&self, ctx: &Ctx<'_>) -> f32 {
        if self.burst > 0 {
            1.0 / ctx.burst_speed_multiplier
        } else if self.burst < 0 {
            ctx.burst_throttle_multiplier
        } else {
            1.0
        }
    }

    /// `getSpeedMultiplier`: lower is faster.
    pub fn speed_multiplier(&self, ctx: &Ctx<'_>) -> f32 {
        self.addons.speed * self.burst_bonus(ctx)
    }

    /// `getEfficiencyMultiplier`: lower is better.
    pub fn efficiency_multiplier(&self, ctx: &Ctx<'_>) -> f32 {
        self.addons.efficiency * self.burst_bonus(ctx)
    }

    /// `calculateEnergyUsage`: RF a tick of work.
    pub fn energy_usage(&self, ctx: &Ctx<'_>) -> f32 {
        ctx.energy_per_tick as f32 * self.efficiency_multiplier(ctx) * (1.0 / self.speed_multiplier(ctx))
    }

    /// `getProgress`: the share of the current recipe done.
    pub fn progress_share(&self, book: &RecipeBook, ctx: &Ctx<'_>) -> f32 {
        let time = self.current.as_deref().and_then(|id| book.get(id)).map_or(-1, |r| r.time);
        self.progress as f32 / (time as f32 * self.speed_multiplier(ctx))
    }

    /// One server tick (`UpgradableMachineBlockEntity.serverTick`). `slots` is the machine's inventory, inputs then
    /// outputs. True when anything changed.
    pub fn tick(&mut self, spec: &Spec, slots: &mut [Option<Stack>], ctx: &Ctx<'_>) -> bool {
        let before = (self.clone(), slots.to_vec());
        self.machine_tick(spec, slots, ctx);
        // addBurstTicks
        self.burst = (self.burst + 1).min(self.addons.max_burst_ticks);
        (self.clone(), slots.to_vec()) != before
    }

    /// `MachineBlockEntity.serverTick`.
    fn machine_tick(&mut self, spec: &Spec, slots: &mut [Option<Stack>], ctx: &Ctx<'_>) {
        if !ctx.active || self.disabled_via_redstone {
            return;
        }
        let candidate = self.recipe(spec, slots, ctx.book).cloned();
        if candidate.is_none() {
            self.current = None;
        }
        match candidate {
            Some(recipe) if can_output(spec, slots, &recipe, ctx.max_stack) => {
                if self.current.as_deref() != Some(recipe.id.as_str()) {
                    self.progress = 0;
                }
                if self.has_enough_energy(ctx) {
                    self.current = Some(recipe.id.clone());
                    self.last_worked_at = ctx.game_time;
                    self.use_energy(ctx);
                    self.progress += 1;
                    if self.progress as f32 >= recipe.time as f32 * self.speed_multiplier(ctx) {
                        self.craft(spec, slots, &recipe, ctx);
                        self.progress = 0;
                    }
                }
            }
            _ => {
                if self.progress > 0 {
                    self.progress = 0;
                }
            }
        }
    }

    fn has_enough_energy(&self, ctx: &Ctx<'_>) -> bool {
        self.energy.amount as f32 >= self.energy_usage(ctx)
    }

    /// `useEnergy` (`amount -= calculateEnergyUsage()`: a long less a float, in floats) and the burst ticks it uses.
    fn use_energy(&mut self, ctx: &Ctx<'_>) {
        self.energy.amount = (self.energy.amount as f32 - self.energy_usage(ctx)) as i64;
        // consumeBurstTicks
        self.burst -= 2;
        self.burst = self.burst.max(-self.addons.max_burst_ticks);
    }

    /// `getRecipe`: nothing for empty inputs; the current recipe while it still matches; else the first that does.
    pub fn recipe<'b>(&self, spec: &Spec, slots: &[Option<Stack>], book: &'b RecipeBook) -> Option<&'b Recipe> {
        let input = &slots[spec.slots.input_start..spec.slots.input_start + spec.slots.input_count];
        if input.iter().all(Option::is_none) {
            return None;
        }
        if let Some(current) = self.current.as_deref().and_then(|id| book.get(id))
            && current.matches(input)
        {
            return Some(current);
        }
        book.find(input)
    }

    /// `UpgradableMachineBlockEntity.craftItem`: one craft, then one more for each extra chamber while the same recipe
    /// still matches and fits; then the small dusts are combined (the pulverizer and the fragment forge).
    fn craft(&mut self, spec: &Spec, slots: &mut [Option<Stack>], recipe: &Recipe, ctx: &Ctx<'_>) {
        craft_once(spec, slots, recipe);
        for _ in 0..self.addons.extra_chambers {
            let again = self.recipe(spec, slots, ctx.book).is_some_and(|r| r.id == recipe.id);
            if !again || !can_output(spec, slots, recipe, ctx.max_stack) {
                break;
            }
            craft_once(spec, slots, recipe);
        }
        if spec.compacts_dust {
            combine_small_dusts(spec, slots, ctx);
        }
    }
}

/// `canOutputRecipe`: empty outputs take anything; otherwise each result goes on an empty slot or on its own item
/// with room under the stack size. A result past the machine's output slots isn't checked (Java reads an empty stack).
pub fn can_output(spec: &Spec, slots: &[Option<Stack>], recipe: &Recipe, max_stack: &dyn Fn(&str) -> u32) -> bool {
    let out = &slots[spec.slots.output_start..spec.slots.output_start + spec.slots.output_count];
    if out.iter().all(Option::is_none) {
        return true;
    }
    for (i, result) in recipe.results.iter().enumerate() {
        let Some(Some(held)) = out.get(i) else { continue };
        if held.item != result.item || held.count + result.count > max_stack(&held.item) {
            return false;
        }
    }
    true
}

/// `MachineBlockEntity.craftItem`: each result onto its output slot, then one item of each ingredient from the inputs,
/// starting one slot further along after each (so equal stacks drain evenly).
fn craft_once(spec: &Spec, slots: &mut [Option<Stack>], recipe: &Recipe) {
    let s = spec.slots;
    for (i, result) in recipe.results.iter().enumerate().take(s.output_count) {
        let slot = &mut slots[s.output_start + i];
        match slot {
            Some(held) => held.count += result.count,
            None => *slot = Some(result.clone()),
        }
    }
    let mut offset = 0;
    for ingredient in &recipe.inputs {
        for i in 0..s.input_count {
            let at = s.input_start + (i + offset) % s.input_count;
            if ingredient.test(slots[at].as_ref()) {
                let stack = slots[at].as_mut().expect("tested");
                stack.count -= 1;
                if stack.count == 0 {
                    slots[at] = None;
                }
                offset += 1;
                break;
            }
        }
    }
}

/// `PulverizerBlockEntity.combineSmallDusts`: nine of the second output that craft into the first output's item become
/// one more of it.
fn combine_small_dusts(spec: &Spec, slots: &mut [Option<Stack>], ctx: &Ctx<'_>) {
    let s = spec.slots;
    let (Some(small), Some(base)) = (slots[s.output_start + 1].clone(), slots[s.output_start].clone()) else { return };
    if small.count < 9 || base.count >= (ctx.max_stack)(&base.item) {
        return;
    }
    if (ctx.compact)(&small.item).as_deref() == Some(base.item.as_str()) {
        let small_slot = slots[s.output_start + 1].as_mut().expect("held");
        small_slot.count -= 9;
        if small_slot.count == 0 {
            slots[s.output_start + 1] = None;
        }
        slots[s.output_start].as_mut().expect("held").count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::{ASSEMBLER, FOUNDRY, PULVERIZER};
    use crate::recipe::{Ingredient, Recipe};

    fn recipe(id: &str, time: i32, inputs: &[&[&str]], results: &[(&str, u32)]) -> Recipe {
        Recipe {
            id: id.into(),
            kind: "oritech:test".into(),
            time,
            inputs: inputs.iter().map(|i| Ingredient { items: i.iter().map(|s| s.to_string()).collect() }).collect(),
            results: results.iter().map(|(i, c)| Stack::new(i, *c)).collect(),
            fluids: false,
        }
    }

    fn st(item: &str, count: u32) -> Option<Stack> {
        Some(Stack::new(item, count))
    }

    fn sixty_four(_: &str) -> u32 {
        64
    }

    fn nothing(_: &str) -> Option<String> {
        None
    }

    fn ctx<'a>(book: &'a RecipeBook, per_tick: i64) -> Ctx<'a> {
        Ctx { book, energy_per_tick: per_tick, burst_speed_multiplier: 8.0, burst_throttle_multiplier: 1.2, game_time: 0, active: true, max_stack: &sixty_four, compact: &nothing }
    }

    fn pulverizer(amount: i64) -> Machine {
        Machine::new(Energy { amount, capacity: 25_000, max_insert: 256, max_extract: 0 })
    }

    /// ATM10's pulverizer (64 RF a tick) on a 100-tick recipe: the output comes on the hundredth tick of work, which
    /// cost 6,400 RF.
    #[test]
    fn a_recipe_takes_its_time_and_its_energy() {
        let book = RecipeBook::new(vec![recipe("raw_iron", 100, &[&["minecraft:raw_iron"]], &[("oritech:iron_dust", 2), ("oritech:small_iron_dust", 3)])]);
        let c = ctx(&book, 64);
        let mut m = pulverizer(25_000);
        let mut slots = vec![st("minecraft:raw_iron", 2), None, None];
        for _ in 0..99 {
            assert!(m.tick(&PULVERIZER, &mut slots, &c));
        }
        assert_eq!((m.progress, slots[1].clone()), (99, None));
        m.tick(&PULVERIZER, &mut slots, &c);
        assert_eq!(slots, vec![st("minecraft:raw_iron", 1), st("oritech:iron_dust", 2), st("oritech:small_iron_dust", 3)]);
        assert_eq!((m.progress, m.energy.amount), (0, 25_000 - 100 * 64));
        assert_eq!(m.current.as_deref(), Some("raw_iron"));
    }

    #[test]
    fn without_energy_the_progress_waits_and_without_input_it_resets() {
        let book = RecipeBook::new(vec![recipe("r", 10, &[&["a:ore"]], &[("a:dust", 1)])]);
        let c = ctx(&book, 64);
        let mut m = pulverizer(64 * 3 + 10);
        let mut slots = vec![st("a:ore", 1), None, None];
        for _ in 0..5 {
            m.tick(&PULVERIZER, &mut slots, &c);
        }
        assert_eq!((m.progress, m.energy.amount), (3, 10), "three ticks of energy, then it waits");
        m.energy.amount = 1000;
        m.tick(&PULVERIZER, &mut slots, &c);
        assert_eq!(m.progress, 4, "the progress was kept");
        slots[0] = None;
        m.tick(&PULVERIZER, &mut slots, &c);
        assert_eq!((m.progress, m.current.clone()), (0, None), "no input: reset, and no current recipe");
    }

    #[test]
    fn a_blocked_output_stops_the_machine() {
        let book = RecipeBook::new(vec![recipe("r", 5, &[&["a:ore"]], &[("a:dust", 2)])]);
        let c = ctx(&book, 1);
        let mut m = pulverizer(1000);
        let mut slots = vec![st("a:ore", 4), st("a:other", 1), None];
        assert!(!m.tick(&PULVERIZER, &mut slots, &c), "another item on the first output");
        slots[1] = st("a:dust", 63);
        assert!(!m.tick(&PULVERIZER, &mut slots, &c), "no room for two more");
        slots[1] = st("a:dust", 62);
        for _ in 0..5 {
            m.tick(&PULVERIZER, &mut slots, &c);
        }
        assert_eq!(slots[1], st("a:dust", 64));
        assert_eq!(slots[0], st("a:ore", 3));
    }

    /// An assembler recipe of two of the same tag and two others: four items go, one from each slot, whichever slots
    /// the ingredients are in.
    #[test]
    fn inputs_are_taken_one_item_an_ingredient() {
        let book = RecipeBook::new(vec![recipe(
            "battery",
            2,
            &[&["oritech:plastic_sheet"], &["oritech:electrum_ingot"], &["oritech:electrum_ingot"], &["oritech:steel_ingot"]],
            &[("oritech:basic_battery", 1)],
        )]);
        let c = ctx(&book, 128);
        let mut m = Machine::new(Energy { amount: 50_000, capacity: 50_000, max_insert: 1024, max_extract: 0 });
        let mut slots = vec![st("oritech:electrum_ingot", 5), st("oritech:steel_ingot", 2), st("oritech:plastic_sheet", 1), st("oritech:electrum_ingot", 1), None];
        m.tick(&ASSEMBLER, &mut slots, &c);
        m.tick(&ASSEMBLER, &mut slots, &c);
        assert_eq!(slots, vec![st("oritech:electrum_ingot", 4), st("oritech:steel_ingot", 1), None, None, st("oritech:basic_battery", 1)]);
        assert_eq!(m.energy.amount, 50_000 - 2 * 128);
        m.tick(&ASSEMBLER, &mut slots, &c);
        assert_eq!(m.progress, 0, "no more plastic: nothing matches");
    }

    #[test]
    fn a_two_input_foundry_matches_either_order() {
        let book = RecipeBook::new(vec![recipe("adamant", 3, &[&["minecraft:diamond"], &["oritech:nickel_ingot"]], &[("oritech:adamant_ingot", 1)])]);
        let c = ctx(&book, 256);
        let mut m = Machine::new(Energy { amount: 50_000, capacity: 50_000, max_insert: 1024, max_extract: 0 });
        let mut slots = vec![st("oritech:nickel_ingot", 2), st("minecraft:diamond", 1), None];
        for _ in 0..3 {
            m.tick(&FOUNDRY, &mut slots, &c);
        }
        assert_eq!(slots, vec![st("oritech:nickel_ingot", 1), None, st("oritech:adamant_ingot", 1)]);
        assert_eq!(m.energy.amount, 50_000 - 3 * 256);
    }

    #[test]
    fn an_inactive_multiblock_does_nothing() {
        let book = RecipeBook::new(vec![recipe("adamant", 3, &[&["minecraft:diamond"], &["oritech:nickel_ingot"]], &[("oritech:adamant_ingot", 1)])]);
        let mut c = ctx(&book, 256);
        c.active = false;
        let mut m = Machine::new(Energy { amount: 50_000, capacity: 50_000, max_insert: 1024, max_extract: 0 });
        let mut slots = vec![st("oritech:nickel_ingot", 2), st("minecraft:diamond", 1), None];
        assert!(!m.tick(&FOUNDRY, &mut slots, &c));
    }

    /// Nine small dusts of the base output's own dust become one more of it, when the crafting grid says so.
    #[test]
    fn small_dusts_are_combined() {
        let book = RecipeBook::new(vec![recipe("r", 1, &[&["a:ore"]], &[("a:dust", 1), ("a:small_dust", 5)])]);
        let compact = |item: &str| (item == "a:small_dust").then(|| "a:dust".to_owned());
        let mut c = ctx(&book, 1);
        c.compact = &compact;
        let mut m = pulverizer(1000);
        let mut slots = vec![st("a:ore", 3), None, None];
        m.tick(&PULVERIZER, &mut slots, &c);
        assert_eq!((slots[1].clone(), slots[2].clone()), (st("a:dust", 1), st("a:small_dust", 5)));
        m.tick(&PULVERIZER, &mut slots, &c);
        assert_eq!((slots[1].clone(), slots[2].clone()), (st("a:dust", 3), st("a:small_dust", 1)), "ten small dusts: nine became one dust");
    }

    /// The current recipe keeps its place while it matches, though another matches too; a switch starts again.
    #[test]
    fn the_current_recipe_sticks() {
        let book = RecipeBook::new(vec![recipe("first", 10, &[&["a:ore"]], &[("a:x", 1)]), recipe("second", 10, &[&["a:ore", "a:other"]], &[("a:y", 1)])]);
        let c = ctx(&book, 1);
        let mut m = pulverizer(1000);
        m.current = Some("second".into());
        let mut slots = vec![st("a:ore", 1), None, None];
        m.tick(&PULVERIZER, &mut slots, &c);
        assert_eq!((m.current.as_deref(), m.progress), (Some("second"), 1));
        m.current = Some("first".into());
        m.progress = 5;
        slots[0] = st("a:other", 1);
        m.tick(&PULVERIZER, &mut slots, &c);
        assert_eq!((m.current.as_deref(), m.progress), (Some("second"), 1), "the switch reset the progress");
    }

    /// Extra chambers craft again at once while the inputs last; a speed addon of 0.5 halves the time and doubles the
    /// energy a tick (efficiency 1 over speed 0.5).
    #[test]
    fn addons_change_time_energy_and_count() {
        let book = RecipeBook::new(vec![recipe("r", 10, &[&["a:ore"]], &[("a:dust", 1)])]);
        let c = ctx(&book, 10);
        let mut m = pulverizer(10_000);
        m.addons = AddonData { speed: 0.5, extra_chambers: 2, ..AddonData::default() };
        let mut slots = vec![st("a:ore", 4), None, None];
        for _ in 0..5 {
            m.tick(&PULVERIZER, &mut slots, &c);
        }
        assert_eq!(slots[1], st("a:dust", 3), "one craft and two chambers in five ticks");
        assert_eq!(slots[0], st("a:ore", 1));
        assert_eq!(m.energy.amount, 10_000 - 5 * 20);
    }

    /// Burst: a stored tick makes work 8 times faster (the config's multiplier) at the same RF per item, and each
    /// tick of work costs two stored ticks; idle ticks store one up to the maximum.
    #[test]
    fn burst_ticks_speed_up_and_throttle() {
        let book = RecipeBook::new(vec![recipe("r", 16, &[&["a:ore"]], &[("a:dust", 1)])]);
        let c = ctx(&book, 8);
        let mut m = pulverizer(10_000);
        m.addons.max_burst_ticks = 4;
        let mut idle = vec![None, None, None];
        for _ in 0..10 {
            m.tick(&PULVERIZER, &mut idle, &c);
        }
        assert_eq!(m.burst, 4, "stored up to the maximum");
        assert_eq!(m.speed_multiplier(&c), 1.0 / 8.0);
        assert_eq!(m.energy_usage(&c), 8.0, "efficiency and speed both take the bonus: the same RF a tick");
        let mut slots = vec![st("a:ore", 1), None, None];
        m.tick(&PULVERIZER, &mut slots, &c);
        assert_eq!(m.burst, 3, "a tick of work uses two, then one comes back");
        m.tick(&PULVERIZER, &mut slots, &c);
        assert_eq!(slots[1], st("a:dust", 1), "16 ticks at an eighth: two ticks");
        m.burst = -1;
        assert!((m.energy_usage(&c) - 8.0).abs() < 1e-4 && (m.speed_multiplier(&c) - 1.2).abs() < 1e-6, "throttled: 1.2 times slower");
    }
}
