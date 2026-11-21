//! The goo blocks (`GooBlock_Base`, `GooBlock_Tier1..4`, `GooBlockBE_Base`, `GooBlockBE_Tier1..4`). A living goo
//! starts converting the block on each side it has a recipe for; a side counts down from the recipe's duration by
//! the tier's step a tick, and at 0 the block there becomes the recipe's output. The goo may die as it finishes one
//! (`goo_death_chance`), and a dead goo starts nothing until it is fed one of its tier's revive items. A side whose
//! block stops matching is cancelled.
use crate::SIDES;

/// One of the four goo blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    One,
    Two,
    Three,
    Four,
}

impl Tier {
    pub const ALL: [Self; 4] = [Self::One, Self::Two, Self::Three, Self::Four];

    /// The goo block `id` is, if it is one.
    pub fn of_block(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.block() == id)
    }

    pub fn block(self) -> &'static str {
        match self {
            Self::One => "justdirethings:gooblock_tier1",
            Self::Two => "justdirethings:gooblock_tier2",
            Self::Three => "justdirethings:gooblock_tier3",
            Self::Four => "justdirethings:gooblock_tier4",
        }
    }

    /// `getTier`: what recipes' `tierRequirement` is compared with.
    pub fn number(self) -> i32 {
        match self {
            Self::One => 1,
            Self::Two => 2,
            Self::Three => 3,
            Self::Four => 4,
        }
    }

    /// `counterReducer`: ticks of a recipe's duration done each game tick.
    pub fn counter_reducer(self) -> i32 {
        match self {
            Self::One => 1,
            Self::Two => 2,
            Self::Three => 5,
            Self::Four => 10,
        }
    }

    /// `validRevivalItem`: the item tag a dead goo is revived with (sugar or rotten flesh for tier 1, nether wart or
    /// blaze powder for 2, chorus fruit or ender pearls for 3, sculk for 4, as the mod's tags say).
    pub fn revive_tag(self) -> &'static str {
        match self {
            Self::One => "justdirethings:goo_revive_tier_1",
            Self::Two => "justdirethings:goo_revive_tier_2",
            Self::Three => "justdirethings:goo_revive_tier_3",
            Self::Four => "justdirethings:goo_revive_tier_4",
        }
    }
}

/// The block entity's state: per side, the ticks left (`sideCounters`) and the recipe's duration (`sideDurations`),
/// -1 where nothing runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Goo {
    pub counters: [i32; SIDES],
    pub durations: [i32; SIDES],
}

impl Default for Goo {
    fn default() -> Self {
        Self { counters: [-1; SIDES], durations: [-1; SIDES] }
    }
}

/// The death settings (`goo_can_die`, `goo_death_chance`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Death {
    pub can_die: bool,
    pub chance: f64,
}

/// What one tick did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Step {
    /// Sides whose block became its recipe's output, in the order they did.
    pub finished: Vec<usize>,
    /// Sides that started a conversion.
    pub started: Vec<usize>,
    /// The goo died this tick.
    pub died: bool,
}

impl Goo {
    /// One server tick (`tickServer`: `checkSides`, then `tickCounters`). `recipes[side]` is the duration of the
    /// recipe that converts the block on that side now, none where there is none (or the goo's tier is too low).
    /// `finish(side)` converts that side's block (`setBlockToTarget`); `roll` is the level's `random.nextFloat`,
    /// drawn once for each finished side while the goo is alive and can die (`killGoo`). `alive` is the block's
    /// `alive` before the tick; a goo that dies on one side starts nothing on the sides after it.
    pub fn tick(
        &mut self,
        tier: Tier,
        alive: bool,
        recipes: &[Option<i32>; SIDES],
        death: Death,
        mut finish: impl FnMut(usize),
        mut roll: impl FnMut() -> f32,
    ) -> Step {
        let mut step = Step::default();
        let mut alive = alive;
        for side in 0..SIDES {
            match recipes[side] {
                Some(duration) => {
                    if self.counters[side] == -1 && alive {
                        self.counters[side] = duration;
                        self.durations[side] = duration;
                        step.started.push(side);
                    } else if self.counters[side] == 0 {
                        finish(side);
                        self.counters[side] = -1;
                        self.durations[side] = -1;
                        step.finished.push(side);
                        if death.can_die && alive && f64::from(roll()) < death.chance {
                            alive = false;
                            step.died = true;
                        }
                    }
                }
                None => {
                    if self.counters[side] != -1 {
                        self.counters[side] = -1;
                        self.durations[side] = -1;
                    }
                }
            }
        }
        let reducer = tier.counter_reducer();
        for counter in &mut self.counters {
            if *counter > 0 {
                *counter = (*counter - reducer).max(0);
            }
        }
        step
    }

    /// Whether any side is converting.
    pub fn busy(&self) -> bool {
        self.counters.iter().any(|c| *c >= 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NEVER: Death = Death { can_die: false, chance: 0.1 };

    /// Ticks a goo with the recipe durations `recipes` until it finishes a side; the ticks it took.
    fn run(tier: Tier, recipes: [Option<i32>; SIDES]) -> (u32, Vec<usize>) {
        let mut goo = Goo::default();
        for t in 1..100_000 {
            let mut done = Vec::new();
            goo.tick(tier, true, &recipes, NEVER, |s| done.push(s), || 0.5);
            if !done.is_empty() {
                return (t, done);
            }
        }
        panic!("never finished");
    }

    #[test]
    fn tiers_name_their_blocks_and_steps() {
        assert_eq!(Tier::of_block("justdirethings:gooblock_tier3"), Some(Tier::Three));
        assert_eq!(Tier::of_block("justdirethings:goosoil_tier3"), None);
        let steps: Vec<_> = Tier::ALL.iter().map(|t| t.counter_reducer()).collect();
        assert_eq!(steps, [1, 2, 5, 10]);
        assert_eq!(Tier::Four.revive_tag(), "justdirethings:goo_revive_tier_4");
    }

    #[test]
    fn a_conversion_takes_its_duration_over_the_tiers_step_plus_one_tick() {
        // The tick a side starts sets its counter; the counter then falls by the step each tick and the block
        // changes on the tick after it reaches 0.
        let iron = [None, None, Some(2400), None, None, None];
        assert_eq!(run(Tier::One, iron), (2401, vec![2]));
        assert_eq!(run(Tier::Two, iron), (1201, vec![2]));
        assert_eq!(run(Tier::Three, iron), (481, vec![2]));
        assert_eq!(run(Tier::Four, iron), (241, vec![2]));
        // A duration that isn't a multiple of the step ends at 0 all the same.
        assert_eq!(run(Tier::Four, [Some(4800), Some(25), None, None, None, None]), (4, vec![1]));
    }

    #[test]
    fn a_dead_goo_starts_nothing_but_finishes_what_runs() {
        let recipes = [Some(4), None, None, None, None, None];
        let mut goo = Goo::default();
        let step = goo.tick(Tier::One, false, &recipes, NEVER, |_| {}, || 0.0);
        assert!(step.started.is_empty());
        assert_eq!(goo, Goo::default());
        goo.tick(Tier::One, true, &recipes, NEVER, |_| {}, || 0.0);
        assert_eq!((goo.counters[0], goo.durations[0]), (3, 4));
        for _ in 0..3 {
            goo.tick(Tier::One, false, &recipes, NEVER, |_| {}, || 0.0);
        }
        let mut done = Vec::new();
        goo.tick(Tier::One, false, &recipes, NEVER, |s| done.push(s), || 0.0);
        assert_eq!(done, [0], "a running side finishes even after the goo died");
        assert_eq!(goo, Goo::default());
    }

    #[test]
    fn a_side_whose_block_changed_is_cancelled() {
        let mut goo = Goo::default();
        goo.tick(Tier::One, true, &[None, Some(100), None, None, None, None], NEVER, |_| {}, || 0.0);
        assert!(goo.busy());
        goo.tick(Tier::One, true, &[None; SIDES], NEVER, |_| {}, || 0.0);
        assert_eq!(goo, Goo::default());
    }

    #[test]
    fn finishing_rolls_death_once_per_side_while_alive() {
        let chance = Death { can_die: true, chance: 0.1 };
        let mut goo = Goo { counters: [0, 0, -1, -1, -1, -1], durations: [5, 5, -1, -1, -1, -1] };
        let recipes = [Some(5), Some(5), Some(5), None, None, None];
        let mut rolls = vec![0.05_f32, 0.9];
        let mut drawn = 0;
        let step = goo.tick(Tier::One, true, &recipes, chance, |_| {}, || {
            drawn += 1;
            rolls.remove(0)
        });
        assert_eq!(step.finished, [0, 1]);
        assert!(step.died);
        assert_eq!(drawn, 1, "a dead goo rolls no more");
        assert!(step.started.is_empty(), "the goo died before side 2 could start");
        // A roll at the chance or above leaves it alive; a goo that can't die never draws.
        let mut goo = Goo { counters: [0, -1, -1, -1, -1, -1], durations: [5, -1, -1, -1, -1, -1] };
        let step = goo.tick(Tier::One, true, &recipes, chance, |_| {}, || 0.1);
        assert!(!step.died);
        assert_eq!(step.started, [1, 2]);
        let mut goo = Goo { counters: [0, -1, -1, -1, -1, -1], durations: [5, -1, -1, -1, -1, -1] };
        goo.tick(Tier::One, true, &recipes, NEVER, |_| {}, || panic!("no draw"));
    }
}
