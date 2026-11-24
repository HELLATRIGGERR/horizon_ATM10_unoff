//! The machines that hold fluids: the centrifuge with a fluid addon (`CentrifugeBlockEntity`), the refinery
//! (`RefineryBlockEntity`, its two modules' tanks) and the cooler (`CoolerBlockEntity`), and the fluid generators'
//! bucket slot (`FluidMultiblockGeneratorBlockEntity.processBuckets`). Each is `MachineBlockEntity.serverTick` with
//! its own recipe lookup, `canProceed`, `canOutputRecipe` and `craftItem`. Translated from Oritech 1.2.13 (CC0).
use crate::{
    energy::Energy,
    fluid::{Tank, BUCKET},
    machine::{Kind, Spec},
    process::{can_output, craft_once},
    recipe::{Recipe, RecipeBook, Stack},
};

/// The tank sizes of a machine: its input tank (the cooler's only one) and its output tanks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    pub input: i32,
    pub outputs: Vec<i32>,
}

/// `SimpleInOutFluidStorage(tankSizeInBuckets)` of the centrifuge; the refinery's own 8 buckets in and out and its
/// modules' 4 each (`nodeA`, `nodeB`); the cooler's 4.
pub fn layout(kind: Kind, centrifuge_buckets: i64) -> Layout {
    let cent = (centrifuge_buckets.clamp(0, 1_000_000) as i32) * BUCKET;
    match kind {
        Kind::Centrifuge => Layout { input: cent, outputs: vec![cent] },
        Kind::Refinery => Layout { input: 8 * BUCKET, outputs: vec![8 * BUCKET, 4 * BUCKET, 4 * BUCKET] },
        Kind::Cooler => Layout { input: 4 * BUCKET, outputs: vec![] },
        _ => Layout { input: 4 * BUCKET, outputs: vec![] },
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FluidMachine {
    pub progress: i32,
    pub current: Option<String>,
    pub energy: Energy,
    pub disabled_via_redstone: bool,
    pub input: Tank,
    pub outputs: Vec<Tank>,
    pub layout: Layout,
    /// The refinery's active modules (0 to 2).
    pub modules: i32,
}

/// What the world gives a tick.
pub struct FluidCtx<'a> {
    /// The machine's own recipe type (`centrifuge_fluid`, `refinery`, `cooler`).
    pub book: &'a RecipeBook,
    /// The centrifuge's item recipes, the fallback of a fluid centrifuge (`getNormalRecipe`).
    pub item_book: Option<&'a RecipeBook>,
    pub energy_per_tick: i64,
    pub speed: f32,
    pub efficiency: f32,
    pub game_time: i64,
    pub active: bool,
    pub max_stack: &'a dyn Fn(&str) -> u32,
    /// Whether a fluid is in a fluid tag.
    pub tag: &'a dyn Fn(&str, &str) -> bool,
}

impl FluidMachine {
    pub fn new(energy: Energy, layout: Layout) -> Self {
        let outputs = vec![Tank::default(); layout.outputs.len()];
        Self { progress: 0, current: None, energy, disabled_via_redstone: false, input: Tank::default(), outputs, layout, modules: 0 }
    }

    /// `calculateEnergyUsage`.
    pub fn energy_usage(&self, ctx: &FluidCtx<'_>) -> f32 {
        ctx.energy_per_tick as f32 * ctx.efficiency * (1.0 / ctx.speed)
    }

    /// `RefineryBlockEntity.calculateOutputFluids`: no module doubles output A (and drops B), one doubles B, two give
    /// them as they are.
    pub fn output_fluids(&self, recipe: &Recipe) -> Vec<(String, i32)> {
        let out = &recipe.fluid_out;
        if out.is_empty() {
            return Vec::new();
        }
        if out.len() == 1 {
            return vec![out[0].clone()];
        }
        match self.modules {
            0 => vec![(out[0].0.clone(), out[0].1 * 2)],
            1 => vec![out[0].clone(), (out[1].0.clone(), out[1].1 * 2)],
            _ => out.clone(),
        }
    }

    /// The recipe as the machine crafts it: the refinery's first item result is doubled with no module when it has
    /// two fluid outputs (`getCraftingResults`).
    fn crafted(&self, spec: &Spec, recipe: &Recipe) -> Recipe {
        let mut r = recipe.clone();
        if spec.kind == Kind::Refinery {
            let multiplier = if recipe.fluid_out.len() <= 1 || self.modules != 0 { 1 } else { 2 };
            r.results.truncate(1);
            for s in &mut r.results {
                s.count *= multiplier;
            }
        }
        r
    }

    /// `getRecipe` of each machine; none for empty inputs.
    pub fn recipe<'b>(&self, spec: &Spec, slots: &[Option<Stack>], ctx: &FluidCtx<'b>) -> Option<&'b Recipe> {
        let input = &slots[spec.slots.input_start..spec.slots.input_start + spec.slots.input_count];
        if self.input.is_empty() && input.iter().all(Option::is_none) {
            return None;
        }
        let fits = |r: &&Recipe| r.matches(input) && r.fluid_matches(&self.input, ctx.tag);
        match spec.kind {
            Kind::Refinery => {
                let mut found: Vec<&Recipe> = ctx.book.recipes.iter().filter(fits).collect();
                found.sort_by_key(|r| -(r.inputs.len() as i64));
                found.first().copied()
            }
            Kind::Centrifuge => ctx.book.recipes.iter().find(fits).or_else(|| ctx.item_book.and_then(|b| b.find(input))),
            _ => ctx.book.recipes.iter().find(fits),
        }
    }

    /// `canProceed` of the fluid centrifuge: the output fluid fits and is the same fluid.
    fn can_proceed(&self, spec: &Spec, recipe: &Recipe) -> bool {
        if spec.kind != Kind::Centrifuge {
            return true;
        }
        if let Some((fluid, amount)) = recipe.fluid_out.first().filter(|(f, a)| !f.is_empty() && *a > 0) {
            let out = &self.outputs[0];
            if out.amount + amount > self.layout.outputs[0] {
                return false;
            }
            if !out.is_empty() && out.fluid != *fluid {
                return false;
            }
        }
        true
    }

    /// `canOutputRecipe`: the refinery's fluid outputs of the active modules fit whole, then the items.
    fn can_output(&self, spec: &Spec, slots: &[Option<Stack>], recipe: &Recipe, ctx: &FluidCtx<'_>) -> bool {
        if spec.kind == Kind::Refinery {
            let outputs = self.output_fluids(recipe);
            for i in 0..=self.modules.max(0) as usize {
                let Some((fluid, amount)) = outputs.get(i) else { break };
                if fluid.is_empty() || *amount <= 0 {
                    continue;
                }
                let Some(tank) = self.outputs.get(i) else { return false };
                let mut probe = tank.clone();
                if probe.insert(fluid, *amount, self.layout.outputs[i], true) != *amount {
                    return false;
                }
            }
        }
        can_output(spec, slots, &self.crafted(spec, recipe), ctx.max_stack)
    }

    /// One server tick. True when anything changed.
    pub fn tick(&mut self, spec: &Spec, slots: &mut [Option<Stack>], ctx: &FluidCtx<'_>) -> bool {
        let before = (self.clone(), slots.to_vec());
        self.step(spec, slots, ctx);
        (self.clone(), slots.to_vec()) != before
    }

    fn step(&mut self, spec: &Spec, slots: &mut [Option<Stack>], ctx: &FluidCtx<'_>) {
        if !ctx.active || self.disabled_via_redstone {
            return;
        }
        let candidate = self.recipe(spec, slots, ctx).cloned();
        if candidate.is_none() {
            self.current = None;
        }
        match candidate {
            Some(recipe) if self.can_output(spec, slots, &recipe, ctx) && self.can_proceed(spec, &recipe) => {
                if self.current.as_deref() != Some(recipe.id.as_str()) {
                    self.progress = 0;
                }
                if self.energy.amount as f32 >= self.energy_usage(ctx) {
                    self.current = Some(recipe.id.clone());
                    self.energy.amount = (self.energy.amount as f32 - self.energy_usage(ctx)) as i64;
                    self.progress += 1;
                    if self.progress as f32 >= recipe.time as f32 * ctx.speed {
                        self.craft(spec, slots, &recipe);
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

    fn craft(&mut self, spec: &Spec, slots: &mut [Option<Stack>], recipe: &Recipe) {
        match spec.kind {
            Kind::Cooler => {
                // `processCraftInstance`: the fluid and the room for the item (a stack of 64 at most; another item on
                // the slot is refused here, where Java would add to it).
                let Some(result) = recipe.results.first() else { return };
                let held = slots[0].as_ref().map_or(0, |s| s.count);
                if slots[0].as_ref().is_some_and(|s| s.item != result.item) || held + result.count > 64 {
                    return;
                }
                let need = recipe.fluid_in.as_ref().map_or(0, |f| f.amount);
                if self.input.take(need, true) != need {
                    return;
                }
                self.input.take(need, false);
                match &mut slots[0] {
                    Some(s) => s.count += result.count,
                    None => slots[0] = Some(result.clone()),
                }
            }
            _ => {
                let crafted = self.crafted(spec, recipe);
                craft_once(spec, slots, &crafted);
                let need = recipe.fluid_in.as_ref().map_or(0, |f| f.amount);
                if need > 0 {
                    self.input.take(need, false);
                }
                if spec.kind == Kind::Refinery {
                    for (i, (fluid, amount)) in self.output_fluids(recipe).iter().enumerate().take(self.outputs.len()) {
                        let cap = self.layout.outputs[i];
                        self.outputs[i].insert(fluid, *amount, cap, false);
                    }
                } else if let Some((fluid, amount)) = recipe.fluid_out.first().filter(|(_, a)| *a > 0) {
                    let cap = self.layout.outputs[0];
                    self.outputs[0].insert(fluid, *amount, cap, false);
                }
            }
        }
    }
}

/// `FluidMultiblockGeneratorBlockEntity.processBuckets`: a single bucket in the input slot is emptied into the tank when
/// the tank takes the whole bucket; one that gives nothing (empty, wrong fluid, no room, water) moves to the output slot.
/// `bucket(item)` is what the item holds (`None`: no fluid handler) and the item left when emptied. True on any change.
pub fn process_buckets(
    slots: &mut [Option<Stack>],
    tank: &mut Tank,
    capacity: i32,
    bucket: &dyn Fn(&str) -> Option<(Option<String>, String)>,
    max_stack: &dyn Fn(&str) -> u32,
) -> bool {
    if tank.amount >= capacity {
        return false;
    }
    let Some(held) = slots[0].clone().filter(|s| s.count == 1) else { return false };
    let Some((fluid, left)) = bucket(&held.item) else { return false };
    let moved = match fluid.filter(|f| f != "minecraft:water") {
        Some(f) if tank.insert(&f, BUCKET, capacity, true) == BUCKET => {
            tank.insert(&f, BUCKET, capacity, false);
            slots[0] = Some(Stack::new(&left, 1));
            return true;
        }
        _ => 0,
    };
    if moved == 0 {
        match &mut slots[1] {
            None => {
                slots[1] = slots[0].take();
                return true;
            }
            Some(out) if out.item == held.item && out.count < max_stack(&out.item) => {
                out.count += 1;
                slots[0] = None;
                return true;
            }
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::{CENTRIFUGE, COOLER, REFINERY};
    use crate::recipe::{FluidIn, Ingredient};

    fn st(item: &str, count: u32) -> Option<Stack> {
        Some(Stack::new(item, count))
    }

    fn recipe(id: &str, time: i32, items: &[&str], results: &[(&str, u32)], fin: Option<(&str, bool, i32)>, fout: &[(&str, i32)]) -> Recipe {
        Recipe {
            id: id.into(),
            kind: "oritech:test".into(),
            time,
            inputs: items.iter().map(|i| Ingredient { items: vec![(*i).into()] }).collect(),
            results: results.iter().map(|(i, c)| Stack::new(i, *c)).collect(),
            fluids: true,
            fluid_in: fin.map(|(f, tag, amount)| FluidIn { fluid: f.into(), tag, amount }),
            fluid_out: fout.iter().map(|(f, a)| ((*f).into(), *a)).collect(),
        }
    }

    fn sixty_four(_: &str) -> u32 {
        64
    }

    fn tags(fluid: &str, tag: &str) -> bool {
        tag == "c:biofuel" && fluid == "oritech:still_biofuel"
    }

    fn ctx<'a>(book: &'a RecipeBook, item_book: Option<&'a RecipeBook>) -> FluidCtx<'a> {
        FluidCtx { book, item_book, energy_per_tick: 10, speed: 1.0, efficiency: 1.0, game_time: 0, active: true, max_stack: &sixty_four, tag: &tags }
    }

    fn energy() -> Energy {
        Energy { amount: 100_000, capacity: 100_000, max_insert: 1000, max_extract: 0 }
    }

    #[test]
    fn the_fluid_centrifuge_takes_fluid_and_item_and_gives_fluid() {
        let book = RecipeBook::new(vec![recipe("wash", 4, &["a:ore"], &[("a:clean", 2)], Some(("minecraft:water", false, 500)), &[("a:slurry", 300)])]);
        let c = ctx(&book, None);
        let mut m = FluidMachine::new(energy(), layout(Kind::Centrifuge, 8));
        let mut slots = vec![st("a:ore", 2), None, None];
        for _ in 0..8 {
            m.tick(&CENTRIFUGE, &mut slots, &c);
        }
        assert_eq!(slots[0], st("a:ore", 2), "no fluid: nothing runs");
        m.input = Tank::new("minecraft:water", 1200);
        for _ in 0..4 {
            m.tick(&CENTRIFUGE, &mut slots, &c);
        }
        assert_eq!((slots[0].clone(), slots[1].clone()), (st("a:ore", 1), st("a:clean", 2)));
        assert_eq!((m.input.amount, m.outputs[0].clone()), (700, Tank::new("a:slurry", 300)));
        assert_eq!(m.energy.amount, 100_000 - 4 * 10);
    }

    #[test]
    fn a_fluid_centrifuge_falls_back_to_its_item_recipes_and_waits_on_a_full_output() {
        let fluid = RecipeBook::new(vec![recipe("f", 2, &["a:ore"], &[], Some(("c:biofuel", true, 100)), &[("a:oil", 900)])]);
        let items = RecipeBook::new(vec![recipe("i", 2, &["a:rock"], &[("a:dust", 1)], None, &[])]);
        let c = ctx(&fluid, Some(&items));
        let mut m = FluidMachine::new(energy(), layout(Kind::Centrifuge, 1));
        let mut slots = vec![st("a:rock", 1), None, None];
        m.tick(&CENTRIFUGE, &mut slots, &c);
        m.tick(&CENTRIFUGE, &mut slots, &c);
        assert_eq!(slots[1], st("a:dust", 1), "an item recipe still runs");
        m.input = Tank::new("oritech:still_biofuel", 300);
        slots = vec![st("a:ore", 3), None, None];
        for _ in 0..2 {
            m.tick(&CENTRIFUGE, &mut slots, &c);
        }
        assert_eq!(m.outputs[0], Tank::new("a:oil", 900));
        for _ in 0..4 {
            m.tick(&CENTRIFUGE, &mut slots, &c);
        }
        assert_eq!((m.outputs[0].amount, m.input.amount, slots[0].clone()), (900, 200, st("a:ore", 2)), "900 + 900 would not fit a 1,000 tank");
    }

    /// The refinery's outputs by its modules: none doubles the first fluid and the items and drops the second.
    #[test]
    fn the_refinery_outputs_by_its_modules() {
        let r = recipe("diesel", 3, &["oritech:clay_catalyst_beads"], &[("a:residue", 1)], Some(("c:biofuel", true, 1000)), &[("a:diesel", 500), ("a:naphtha", 200)]);
        let book = RecipeBook::new(vec![r]);
        let c = ctx(&book, None);
        for (modules, expect) in [(0, (1000, 0, 0, 2)), (1, (500, 400, 0, 1)), (2, (500, 200, 0, 1))] {
            let mut m = FluidMachine::new(energy(), layout(Kind::Refinery, 8));
            m.modules = modules;
            m.input = Tank::new("oritech:still_biofuel", 2500);
            let mut slots = vec![st("oritech:clay_catalyst_beads", 5), None];
            for _ in 0..3 {
                m.tick(&REFINERY, &mut slots, &c);
            }
            let got = (m.outputs[0].amount, m.outputs[1].amount, m.outputs[2].amount, slots[1].as_ref().map_or(0, |s| s.count));
            assert_eq!(got, (expect.0, expect.1, expect.2, expect.3 as u32), "{modules} modules");
            assert_eq!(m.input.amount, 1500);
        }
    }

    #[test]
    fn a_full_refinery_tank_blocks_the_craft() {
        let r = recipe("d", 1, &["a:cat"], &[], Some(("a:oil", false, 100)), &[("a:diesel", 500)]);
        let book = RecipeBook::new(vec![r]);
        let c = ctx(&book, None);
        let mut m = FluidMachine::new(energy(), layout(Kind::Refinery, 8));
        m.input = Tank::new("a:oil", 1000);
        m.outputs[0] = Tank::new("a:diesel", 7800);
        let mut slots = vec![st("a:cat", 2), None];
        m.tick(&REFINERY, &mut slots, &c);
        assert_eq!((m.outputs[0].amount, m.input.amount, m.progress), (7800, 1000, 0), "7,800 + 500 does not fit 8,000");
        m.outputs[0].amount = 7500;
        m.tick(&REFINERY, &mut slots, &c);
        assert_eq!((m.outputs[0].amount, m.input.amount), (8000, 900));
    }

    #[test]
    fn the_cooler_turns_fluid_into_items() {
        let book = RecipeBook::new(vec![recipe("ice", 3, &[], &[("minecraft:ice", 3)], Some(("minecraft:water", false, 1000)), &[])]);
        let c = ctx(&book, None);
        let mut m = FluidMachine::new(energy(), layout(Kind::Cooler, 8));
        let mut slots = vec![None];
        m.tick(&COOLER, &mut slots, &c);
        assert_eq!(m.progress, 0, "an empty tank does nothing");
        m.input = Tank::new("minecraft:water", 2500);
        for _ in 0..6 {
            m.tick(&COOLER, &mut slots, &c);
        }
        assert_eq!((slots[0].clone(), m.input.amount), (st("minecraft:ice", 6), 500));
        for _ in 0..6 {
            m.tick(&COOLER, &mut slots, &c);
        }
        assert_eq!((slots[0].clone(), m.input.amount), (st("minecraft:ice", 6), 500), "500 mB is not a bucket");
    }

    fn bucket(item: &str) -> Option<(Option<String>, String)> {
        match item {
            "minecraft:lava_bucket" => Some((Some("minecraft:lava".into()), "minecraft:bucket".into())),
            "minecraft:water_bucket" => Some((Some("minecraft:water".into()), "minecraft:bucket".into())),
            "minecraft:bucket" => Some((None, "minecraft:bucket".into())),
            _ => None,
        }
    }

    #[test]
    fn a_bucket_empties_into_the_tank_and_then_moves_out() {
        let mut tank = Tank::default();
        let mut slots = vec![st("minecraft:lava_bucket", 1), None];
        assert!(process_buckets(&mut slots, &mut tank, 4000, &bucket, &sixty_four));
        assert_eq!((tank.clone(), slots[0].clone()), (Tank::new("minecraft:lava", 1000), st("minecraft:bucket", 1)));
        assert!(process_buckets(&mut slots, &mut tank, 4000, &bucket, &sixty_four));
        assert_eq!((slots[0].clone(), slots[1].clone()), (None, st("minecraft:bucket", 1)), "the empty bucket moves out");
        let mut slots = vec![st("minecraft:water_bucket", 1), None];
        assert!(process_buckets(&mut slots, &mut tank, 4000, &bucket, &sixty_four));
        assert_eq!((tank.amount, slots[1].clone()), (1000, st("minecraft:water_bucket", 1)), "water is refused and the bucket moves out");
        let mut slots = vec![st("minecraft:lava_bucket", 2), None];
        assert!(!process_buckets(&mut slots, &mut tank, 4000, &bucket, &sixty_four), "a stack of two is left alone");
    }
}
