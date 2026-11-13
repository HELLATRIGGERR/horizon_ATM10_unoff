//! Mekanism's Antiprotonic Nucleosynthesizer (`TileEntityAntiprotonicNucleosynthesizer`, a
//! `TileEntityProgressMachine<NucleosynthesizingRecipe>`): an item and antimatter in (a tank of
//! 10 000 mB filled from its chemical slot), an item out over the recipe's `duration` ticks.
//!
//! - `ticksRequired` is 400 on a new block entity and becomes a recipe's `duration` when that recipe
//!   is cached (`onCachedRecipeChanged`, after the cache took back a saved progress: so a progress
//!   saved past the ticks the machine required before, 400 for a new one, starts over).
//! - It takes only muffling upgrades (`AttributeUpgradeSupport.MUFFLING_ONLY`); its Joules set its
//!   pace instead: `NucleosynthesizerRecipeCacheLookupMonitor.updateAndProcess` runs the cached recipe
//!   once and then `(int) sqrt(stored / perTick) - 1` more times in the tick, from the Joules it held
//!   before the first run. ATM10's full store of 10^9 J at 100 000 J a run makes 100 runs a tick, each a
//!   tick of the recipe's progress; an operation that finishes in a run leaves the next runs to the
//!   next item in the same tick.
//! - A recipe is found only with the whole of its item and antimatter in the machine
//!   (`ItemStackChemicalToItemStackRecipe.test`): short of either, nothing runs and the saved progress
//!   stays; once cached, a run short of either starts the progress over.
//! - A `per_tick_usage` recipe (`ItemStackConstantChemicalToObjectCachedRecipe`, `crate::advanced`)
//!   takes its amount every run (`constantUse` of `ticksRequired` over `ticksRequired`), with
//!   `used_so_far` saved; a run short of it waits with its progress. The others, all of ATM10's, take
//!   their antimatter with the item when the operation finishes (`TwoInputCachedRecipe.itemChemicalToItem`).
use crate::advanced;
use crate::container::EnergyContainer;
use crate::machine::{self, Inputs};

/// `TileEntityAntiprotonicNucleosynthesizer.MAX_GAS`: the antimatter tank, in mB.
pub const MAX_GAS: i64 = 10_000;
/// `BASE_TICKS_REQUIRED`: `ticksRequired` until a recipe is cached.
pub const BASE_TICKS: u32 = 400;

/// The runs of the cached recipe this tick: once, then `(int) sqrt(stored / per_tick) - 1` more.
/// A machine that takes no Joules a run runs once (Mekanism's would run without end).
pub fn runs(stored: u64, per_tick: u64) -> u32 {
    if per_tick == 0 {
        return 1;
    }
    let more = (stored as f64 / per_tick as f64).sqrt() as i32;
    more.max(1) as u32
}

/// A nucleosynthesizing recipe as a tick runs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recipe {
    /// The item it takes an operation.
    pub input: u32,
    /// The antimatter it takes an operation (a `per_tick` recipe: a run).
    pub amount: i64,
    /// The item it makes and the output's stack size.
    pub output: u32,
    pub max_stack: u32,
    pub per_tick: bool,
}

/// What the machine holds for a tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Contents {
    pub progress: u32,
    pub used_so_far: i64,
    /// The input slot's count when it holds the recipe's item (0 else).
    pub input: u32,
    /// The tank's mB when it holds the recipe's chemical (none: empty or another).
    pub chemical: Option<i64>,
    /// The output slot's count and whether it holds the recipe's output (true when empty).
    pub output: (u32, bool),
}

/// What a tick did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ticked {
    pub runs: u32,
    /// The operations finished: the items taken and made.
    pub operations: u32,
    /// Joules taken (`clientEnergyUsed`, the screen's process rate over the Joules a run).
    pub energy: u64,
    /// mB of antimatter taken.
    pub chemical: i64,
    /// The last run's `setActive`.
    pub active: bool,
}

/// One tick of the cached recipe: [`runs`] runs of `CachedRecipe.process`, each on what the runs
/// before it left. `required` is `ticksRequired`.
pub fn tick(c: &mut Contents, recipe: &Recipe, required: u32, energy: &mut EnergyContainer, per_tick: u64) -> Ticked {
    let before = energy.stored;
    let mut out = Ticked { runs: runs(before, per_tick), ..Ticked::default() };
    for _ in 0..out.runs {
        let held = (c.output.0 > 0).then_some(c.output);
        let room = machine::output_operations(held, recipe.output, recipe.max_stack);
        let item = Inputs::of(c.input, recipe.input);
        let ran = if recipe.per_tick {
            let draw = advanced::constant_use(i64::from(required), c.used_so_far, required, c.progress);
            let stored = c.chemical.unwrap_or(0);
            let ran = advanced::process(&mut c.progress, &mut c.used_so_far, required, energy, per_tick, 1, item, (stored, recipe.amount), draw, room);
            if let Some(tank) = c.chemical.as_mut() {
                *tank -= ran.chemical;
            }
            out.chemical += ran.chemical;
            machine::Ran { operations: ran.operations, finished: ran.finished, energy: ran.energy }
        } else {
            let chemical = match c.chemical {
                Some(stored) => Inputs::of(stored.clamp(0, i64::from(u32::MAX)) as u32, recipe.amount.clamp(0, i64::from(u32::MAX)) as u32),
                None => Inputs::Mismatched,
            };
            let ran = machine::process(&mut c.progress, required, energy, per_tick, 1, item.both(chemical), room, true);
            if ran.finished {
                let used = recipe.amount.saturating_mul(i64::from(ran.operations));
                if let Some(tank) = c.chemical.as_mut() {
                    *tank -= used;
                }
                out.chemical += used;
            }
            ran
        };
        if ran.finished {
            c.input -= recipe.input * ran.operations;
            c.output = (c.output.0 + recipe.output * ran.operations, true);
            out.operations += ran.operations;
        }
        out.active = ran.active();
    }
    out.energy = before - energy.stored;
    out
}

/// The screen's data slots after a machine's (`menu::MACHINE_SLOTS`): the Joules the last tick
/// took (`clientEnergyUsed`, a `SyncableLong`), in four 16-bit pieces.
pub fn pack_used(energy: u64) -> Vec<i32> {
    let mut out = Vec::with_capacity(4);
    crate::menu::push_bits(&mut out, energy, 4);
    out
}

pub fn unpack_used(data: &[i32]) -> Option<u64> {
    let at = crate::menu::MACHINE_SLOTS;
    (data.len() >= at + 4).then(|| crate::menu::read_bits(data, at, 4))
}

/// `GuiAntiprotonicNucleosynthesizer`'s "Process Rate": the Joules the last tick took over a run's,
/// in percent (`TextUtils.getPercent`, rounded): 10000% at 100 runs a tick.
pub fn process_rate(used: u64, per_tick: u64) -> String {
    let rate = if per_tick == 0 { 0.0 } else { used as f64 / per_tick as f64 };
    format!("{}%", (rate * 100.0).round() as i64)
}

/// `ticksRequired` once `duration`'s recipe is cached, and the progress the cache starts from:
/// a first cache (none before, or the recipes read again) takes the saved progress back while it is
/// short of `before` (`loadSavedOperatingTicks`), another starts at 0.
pub fn cached(saved: u32, before: u32, duration: u32, first: bool) -> (u32, u32) {
    let progress = if first { machine::resumed(saved, before) } else { 0 };
    (duration.max(1), progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    const USAGE: u64 = 100_000;

    fn store(stored: u64) -> EnergyContainer {
        let mut c = EnergyContainer::basic(1_000_000_000);
        c.stored = stored;
        c
    }

    /// `minecraft:coal` into `minecraft:diamond` (`mekanism:nucleosynthesizing/diamond`): 4 mB, 1000 ticks.
    const DIAMOND: Recipe = Recipe { input: 1, amount: 4, output: 1, max_stack: 64, per_tick: false };
    /// Tin into iron (`iron`): 1 mB, 200 ticks.
    const IRON: Recipe = Recipe { input: 1, amount: 1, output: 1, max_stack: 64, per_tick: false };
    /// The oracle's own `per_tick_usage` recipe: stone into dirt, 2 mB a run, 50 ticks.
    const PER_TICK: Recipe = Recipe { input: 1, amount: 2, output: 1, max_stack: 64, per_tick: true };

    /// (progress, Joules, tank, input, output, used_so_far, active) after each tick.
    fn run(recipe: Recipe, required: u32, mut c: Contents, joules: u64, ticks: usize) -> Vec<(u32, u64, i64, u32, u32, i64, bool)> {
        let mut e = store(joules);
        (0..ticks)
            .map(|_| {
                let t = tick(&mut c, &recipe, required, &mut e, USAGE);
                (c.progress, e.stored, c.chemical.unwrap_or(0), c.input, c.output.0, c.used_so_far, t.active)
            })
            .collect()
    }

    fn holding(input: u32, tank: i64) -> Contents {
        Contents { input, chemical: Some(tank), output: (0, true), ..Contents::default() }
    }

    #[test]
    fn its_joules_set_how_many_runs_a_tick() {
        assert_eq!(runs(1_000_000_000, USAGE), 100);
        assert_eq!(runs(990_000_000, USAGE), 99, "sqrt(9900) = 99.5");
        assert_eq!(runs(1_000_000, USAGE), 3);
        assert_eq!(runs(399_999, USAGE), 1);
        assert_eq!(runs(150_000, USAGE), 1);
        assert_eq!(runs(0, USAGE), 1, "the first run comes anyway (and finds no Joules)");
        assert_eq!(runs(5, 0), 1);
    }

    #[test]
    fn a_full_store_runs_a_hundred_ticks_a_tick() {
        // Measured on the NeoForge oracle (meknucleo-work n1, B): two coal, 10 mB, 10^9 J.
        let rows = run(DIAMOND, 1000, holding(2, 10), 1_000_000_000, 11);
        let progress: Vec<u32> = rows.iter().map(|r| r.0).collect();
        assert_eq!(progress, [100, 199, 298, 396, 494, 591, 688, 784, 880, 975, 70]);
        let joules: Vec<u64> = rows.iter().map(|r| r.1).collect();
        assert_eq!(joules[..3], [990_000_000, 980_100_000, 970_200_000]);
        // The 11th tick: 95 runs, 25 finish the diamond and take 4 mB, 70 go on with the second coal.
        assert_eq!(rows[10], (70, 893_000_000, 6, 1, 1, 0, true));
    }

    #[test]
    fn an_operation_finished_in_a_run_lets_the_next_item_go_on_in_the_same_tick() {
        // n1 A: three tin ingots: 200 ticks each, 1 mB.
        let rows = run(IRON, 200, holding(3, 10), 1_000_000_000, 7);
        let at = |t: usize| (rows[t].0, rows[t].1, rows[t].2, rows[t].3, rows[t].4, rows[t].6);
        assert_eq!(at(0), (100, 990_000_000, 10, 3, 0, true));
        assert_eq!(at(1), (199, 980_100_000, 10, 3, 0, true));
        assert_eq!(at(2), (98, 970_200_000, 9, 2, 1, true));
        assert_eq!(at(4), (94, 950_600_000, 8, 1, 2, true));
        assert_eq!(at(5), (191, 940_900_000, 8, 1, 2, true));
        // 97 runs, 9 finish the last ingot; the rest find no input, take nothing and leave it inactive.
        assert_eq!(at(6), (0, 940_000_000, 7, 0, 3, false));
        // I: a saved 150 finishes in 50 runs and the next ingot gets the other 50.
        let rows = run(IRON, 200, Contents { progress: 150, ..holding(3, 10) }, 1_000_000_000, 1);
        assert_eq!((rows[0].0, rows[0].2, rows[0].3, rows[0].4), (50, 9, 2, 1));
    }

    #[test]
    fn fewer_joules_run_fewer_times() {
        // C: 10^6 J: 3, 2, 2, 1, 1, 1 runs, then none.
        let rows = run(IRON, 200, holding(3, 10), 1_000_000, 7);
        let got: Vec<(u32, u64, bool)> = rows.iter().map(|r| (r.0, r.1, r.6)).collect();
        assert_eq!(got, [(3, 700_000, true), (5, 500_000, true), (7, 300_000, true), (8, 200_000, true), (9, 100_000, true), (10, 0, true), (10, 0, false)]);
        // D: 150 000 J: one run, then 50 000 J wait.
        let rows = run(IRON, 200, holding(3, 10), 150_000, 2);
        assert_eq!((rows[0].0, rows[0].1, rows[1].0, rows[1].1, rows[1].6), (1, 50_000, 1, 50_000, false));
    }

    #[test]
    fn a_full_output_or_a_short_tank_stops_it() {
        // M: 63 iron in the output and a saved 150: 50 runs finish one, the rest find no room.
        let c = Contents { progress: 150, output: (63, true), ..holding(3, 10) };
        let rows = run(IRON, 200, c, 1_000_000_000, 2);
        assert_eq!(rows[0], (0, 995_000_000, 9, 2, 64, 0, false));
        assert_eq!(rows[1], rows[0]);
        // G: a bow, and a crossbow (which stacks to one) already out: nothing runs.
        let crossbow = Recipe { max_stack: 1, ..IRON };
        let c = Contents { output: (1, true), ..holding(1, 10) };
        assert_eq!(run(crossbow, 200, c, 1_000_000_000, 1)[0], (0, 1_000_000_000, 10, 1, 1, 0, false));
        // K: the tank cut to 3 mB under a cached diamond: the progress starts over.
        let mut c = holding(2, 4);
        let mut e = store(1_000_000_000);
        tick(&mut c, &DIAMOND, 1000, &mut e, USAGE);
        assert_eq!(c.progress, 100);
        c.chemical = Some(3);
        let t = tick(&mut c, &DIAMOND, 1000, &mut e, USAGE);
        assert_eq!((c.progress, e.stored, t.energy, t.active), (0, 990_000_000, 0, false));
    }

    #[test]
    fn a_per_tick_recipe_takes_its_amount_every_run() {
        // J1: four stone, 500 mB, 10^9 J: 100 runs make two dirt (100 mB each); 99 make one and 49 ticks of the next.
        let rows = run(PER_TICK, 50, holding(4, 500), 1_000_000_000, 3);
        assert_eq!(rows[0], (0, 990_000_000, 300, 2, 2, 0, true));
        assert_eq!(rows[1], (49, 980_100_000, 102, 1, 3, 49, true));
        assert_eq!(rows[2], (0, 980_000_000, 100, 0, 4, 0, false));
        // J2: 10^6 J: 3 runs, 6 mB.
        let rows = run(PER_TICK, 50, holding(2, 500), 1_000_000, 2);
        assert_eq!((rows[0].0, rows[0].2, rows[0].5, rows[1].0, rows[1].2), (3, 494, 3, 5, 490));
        // J3: 5 mB: two runs, then 1 mB is short of a run's 2: it waits with its progress.
        let rows = run(PER_TICK, 50, holding(2, 5), 1_000_000_000, 2);
        assert_eq!(rows[0], (2, 999_800_000, 1, 2, 0, 2, false));
        assert_eq!(rows[1], rows[0]);
        // J4: a saved 30 ticks and 10 used: what is left of 50 over the ticks left, 2 a run (4 mB).
        let c = Contents { progress: 30, used_so_far: 10, ..holding(2, 500) };
        let rows = run(PER_TICK, 50, c, 150_000, 1);
        assert_eq!((rows[0].0, rows[0].2, rows[0].5), (31, 496, 12));
    }

    #[test]
    fn the_screen_reads_the_joules_used() {
        let mut data = vec![0; crate::menu::MACHINE_SLOTS];
        data.extend(pack_used(10_000_000));
        assert_eq!(unpack_used(&data), Some(10_000_000));
        assert_eq!(unpack_used(&data[..crate::menu::MACHINE_SLOTS]), None);
        assert_eq!(process_rate(10_000_000, USAGE), "10000%");
        assert_eq!(process_rate(0, USAGE), "0%");
    }

    #[test]
    fn a_cached_recipe_sets_the_ticks_and_takes_back_a_progress_short_of_the_old_ones() {
        // H1 and H2: a saved 500 on a new machine (400 ticks) starts over; 300 goes on.
        assert_eq!(cached(500, BASE_TICKS, 1000, true), (1000, 0));
        assert_eq!(cached(300, BASE_TICKS, 1000, true), (1000, 300));
        assert_eq!(cached(150, BASE_TICKS, 200, true), (200, 150));
        assert_eq!(cached(150, 1000, 200, false), (200, 0), "another recipe starts at 0");
    }
}
