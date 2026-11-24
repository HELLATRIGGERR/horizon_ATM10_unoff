//! The generators' tick: `UpgradableGeneratorBlockEntity.serverTick` with `BasicGeneratorEntity.tryConsumeInput` (any
//! fuel of the furnace's burn times) and the bio generator's (a recipe's ingredient), and `outputEnergy` (the buffer
//! given to each neighbour in turn). The steam boiler addon (water to steam instead of energy) is not ported.
use crate::{
    energy::Energy,
    machine::{Kind, Spec},
    recipe::{RecipeBook, Stack},
};

/// A generator's state between ticks (Java's fields): `progress` counts the burn time left and `max_burn` is
/// `currentMaxBurnTime`.
#[derive(Clone, Debug, PartialEq)]
pub struct Generator {
    pub progress: i32,
    pub max_burn: i32,
    pub energy: Energy,
    pub disabled_via_redstone: bool,
}

/// What the world gives a tick.
pub struct GenCtx<'a> {
    /// `energyPerTick` of the config: what a tick of burning produces at speed 1.
    pub energy_per_tick: i64,
    /// `getSpeedMultiplier` and `getEfficiencyMultiplier` (1 for a generator with no addons).
    pub speed: f32,
    pub efficiency: f32,
    /// `isActive`: the bio generator's core is in place (always true for the basic one).
    pub active: bool,
    /// The burn time of an item in ticks, 0 when it is no fuel.
    pub fuel: &'a dyn Fn(&str) -> i32,
    /// The bio generator's recipes.
    pub book: &'a RecipeBook,
}

impl Generator {
    pub fn new(energy: Energy) -> Self {
        Self { progress: 0, max_burn: 0, energy, disabled_via_redstone: false }
    }

    /// `calculateEnergyUsage` of a generator: what a tick of burning gives.
    pub fn produced(ctx: &GenCtx<'_>) -> f32 {
        ctx.energy_per_tick as f32 * (1.0 / ctx.speed)
    }

    /// `canFitEnergy`.
    fn can_fit(&self, ctx: &GenCtx<'_>) -> bool {
        self.energy.capacity as f32 >= self.energy.amount as f32 + Self::produced(ctx)
    }

    /// One server tick (without `outputEnergy`: see `output`). True when anything changed.
    pub fn tick(&mut self, spec: &Spec, slots: &mut [Option<Stack>], ctx: &GenCtx<'_>) -> bool {
        let before = (self.clone(), slots.to_vec());
        if ctx.active && !self.disabled_via_redstone {
            if self.progress == 0 && self.can_fit(ctx) {
                self.consume(spec, slots, ctx);
            }
            if self.progress > 0 && self.can_fit(ctx) {
                self.progress -= 1;
                self.energy.amount = (self.energy.amount as f32 + Self::produced(ctx)) as i64;
            }
        }
        (self.clone(), slots.to_vec()) != before
    }

    /// `tryConsumeInput`.
    fn consume(&mut self, spec: &Spec, slots: &mut [Option<Stack>], ctx: &GenCtx<'_>) {
        match spec.kind {
            Kind::BasicGenerator => {
                let Some(first) = slots[0].as_ref() else { return };
                if first.item.ends_with("bucket") {
                    return;
                }
                let burn = (ctx.fuel)(&first.item);
                if burn > 0 {
                    take_one(&mut slots[0]);
                    self.progress = burn;
                    self.max_burn = burn;
                }
            }
            _ => {
                let Some(recipe) = ctx.book.find(&slots[spec.slots.input_start..spec.slots.input_start + spec.slots.input_count]) else { return };
                let time = (recipe.time as f32 * ctx.speed * (1.0 / ctx.efficiency)) as i32;
                self.progress = time;
                self.max_burn = time;
                for i in 0..recipe.inputs.len() {
                    take_one(&mut slots[i]);
                }
            }
        }
    }

    /// `outputEnergy`: each target in turn gets what the buffer can give a call (`maxExtract`) and it takes.
    /// `offer(target, amount)` is how much of it that target took.
    pub fn output(&mut self, targets: usize, offer: &mut dyn FnMut(usize, i64) -> i64) -> bool {
        if self.energy.amount <= 0 {
            return false;
        }
        let mut moved = 0;
        for target in 0..targets {
            let available = self.energy.extract(i64::MAX, true);
            if available <= 0 {
                break;
            }
            let took = offer(target, available).clamp(0, available);
            moved += self.energy.extract(took, false);
        }
        moved > 0
    }
}

fn take_one(slot: &mut Option<Stack>) {
    if let Some(stack) = slot {
        stack.count -= 1;
        if stack.count == 0 {
            *slot = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::{BASIC_GENERATOR, BIO_GENERATOR};
    use crate::recipe::{Ingredient, Recipe};

    fn coal(item: &str) -> i32 {
        match item {
            "minecraft:coal" => 1600,
            "minecraft:stick" => 100,
            _ => 0,
        }
    }

    fn st(item: &str, count: u32) -> Option<Stack> {
        Some(Stack::new(item, count))
    }

    fn basic() -> Generator {
        Generator::new(Energy { amount: 0, capacity: 50_000, max_insert: 0, max_extract: 256 })
    }

    fn ctx<'a>(book: &'a RecipeBook, per_tick: i64, active: bool) -> GenCtx<'a> {
        GenCtx { energy_per_tick: per_tick, speed: 1.0, efficiency: 1.0, active, fuel: &coal, book }
    }

    /// ATM10's basic generator (64 RF a tick): a coal (1600 ticks) takes one item, then each tick adds 64 RF.
    #[test]
    fn a_fuel_burns_for_its_time_and_gives_energy_each_tick() {
        let book = RecipeBook::default();
        let c = ctx(&book, 64, true);
        let mut g = basic();
        let mut slots = vec![st("minecraft:coal", 2)];
        assert!(g.tick(&BASIC_GENERATOR, &mut slots, &c));
        assert_eq!((slots[0].clone(), g.progress, g.max_burn, g.energy.amount), (st("minecraft:coal", 1), 1599, 1600, 64));
        for _ in 0..9 {
            g.tick(&BASIC_GENERATOR, &mut slots, &c);
        }
        assert_eq!((g.progress, g.energy.amount), (1590, 640));
    }

    /// The buffer stops the burn (the progress waits) and a bucket or a non-fuel is not taken.
    #[test]
    fn a_full_buffer_waits_and_a_non_fuel_is_not_burnt() {
        let book = RecipeBook::default();
        let c = ctx(&book, 64, true);
        let mut g = basic();
        g.energy.amount = 50_000 - 63;
        let mut slots = vec![st("minecraft:coal", 1)];
        assert!(!g.tick(&BASIC_GENERATOR, &mut slots, &c), "no room for a tick: the coal stays");
        g.energy.amount = 50_000 - 64;
        g.tick(&BASIC_GENERATOR, &mut slots, &c);
        assert_eq!((g.energy.amount, g.progress), (50_000, 1599));
        assert!(!g.tick(&BASIC_GENERATOR, &mut slots, &c), "full: the progress waits");
        let mut other = basic();
        let mut dirt = vec![st("minecraft:dirt", 3)];
        assert!(!other.tick(&BASIC_GENERATOR, &mut dirt, &c));
        let mut lava = vec![st("minecraft:lava_bucket", 1)];
        let lava_fuel = |_: &str| 20_000;
        let c2 = GenCtx { fuel: &lava_fuel, ..ctx(&book, 64, true) };
        assert!(!other.tick(&BASIC_GENERATOR, &mut lava, &c2), "a bucket is never burnt");
    }

    #[test]
    fn a_disabled_generator_does_nothing() {
        let book = RecipeBook::default();
        let mut g = basic();
        g.disabled_via_redstone = true;
        let mut slots = vec![st("minecraft:coal", 1)];
        assert!(!g.tick(&BASIC_GENERATOR, &mut slots, &ctx(&book, 64, true)));
        g.disabled_via_redstone = false;
        assert!(!g.tick(&BASIC_GENERATOR, &mut slots, &ctx(&book, 64, false)), "an unassembled bio generator");
    }

    /// The bio generator burns a recipe's ingredient for the recipe's time: 4000 ticks of 256 RF for packed wheat.
    #[test]
    fn the_bio_generator_burns_a_recipe() {
        let book = RecipeBook::new(vec![Recipe {
            id: "packedwheat".into(),
            kind: "oritech:bio_generator".into(),
            time: 4000,
            inputs: vec![Ingredient { items: vec!["oritech:packed_wheat".into()] }],
            results: vec![],
            fluids: false,
        }]);
        let c = ctx(&book, 256, true);
        let mut g = Generator::new(Energy { amount: 0, capacity: 400_000, max_insert: 0, max_extract: 2048 });
        let mut slots = vec![st("oritech:packed_wheat", 1)];
        g.tick(&BIO_GENERATOR, &mut slots, &c);
        assert_eq!((slots[0].clone(), g.progress, g.max_burn, g.energy.amount), (None, 3999, 4000, 256));
    }

    /// Each neighbour in turn gets what the buffer can give a call and takes; what none takes stays.
    #[test]
    fn the_energy_goes_to_each_neighbour_in_turn() {
        let mut g = Generator::new(Energy { amount: 1000, capacity: 50_000, max_insert: 0, max_extract: 256 });
        let mut offered = Vec::new();
        let moved = g.output(3, &mut |t, amount| {
            offered.push((t, amount));
            [100, 0, 1000][t].min(amount)
        });
        assert!(moved);
        assert_eq!(offered, vec![(0, 256), (1, 256), (2, 256)], "a call gives at most maxExtract");
        assert_eq!(g.energy.amount, 1000 - 100 - 256);
    }
}
