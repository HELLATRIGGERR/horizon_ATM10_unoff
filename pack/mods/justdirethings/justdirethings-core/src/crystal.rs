//! The time crystal's budding block (`TimeCrystalBuddingBlock`) and its clusters (`TimeCrystalCluster`). Made by a
//! tier 4 goo from budding amethyst or a time crystal block (the goo's recipes `budding_time_amethyst` and
//! `budding_time_timeblock`), it starts at stage 0 and a random tick moves it on by where it is: to 1 outside the
//! Nether and the End, to 2 in the Nether, to 3 in the End (or the dimensions the config names). At stage 3 it grows
//! time crystal clusters as budding amethyst grows amethyst, one random tick in five, and each cluster it grows has
//! a 5% chance to send it back to stage 0.
use crate::config::CrystalSettings;

pub const BUDDING: &str = "justdirethings:time_crystal_budding_block";
/// The cluster sizes in the order they grow (`TimeCrystalCluster_Small`, `_Medium`, `_Large`, `TimeCrystalCluster`).
pub const CLUSTERS: [&str; 4] = [
    "justdirethings:time_crystal_cluster_small",
    "justdirethings:time_crystal_cluster_medium",
    "justdirethings:time_crystal_cluster_large",
    "justdirethings:time_crystal_cluster",
];
pub const OVERWORLD: &str = "minecraft:overworld";
pub const NETHER: &str = "minecraft:the_nether";
pub const END: &str = "minecraft:the_end";

/// The draws a random tick takes from the level's random, in Java's order.
pub trait Draw {
    /// `nextInt(bound)`.
    fn next_int(&mut self, bound: i32) -> i32;
    /// `nextFloat()`.
    fn next_float(&mut self) -> f32;
}

/// `canAdvanceTo` / `canAdvanceToCustom`: the stage a block at `stage` in `dimension` moves on to, if any.
pub fn advance_to(stage: i32, dimension: &str, settings: &CrystalSettings) -> Option<i32> {
    if settings.custom_dimensions {
        let lists = [&settings.stage1, &settings.stage2, &settings.stage3];
        return match stage {
            0..=2 if lists[stage as usize].iter().any(|d| d == dimension) => Some(stage + 1),
            _ => None,
        };
    }
    match stage {
        0 if dimension != NETHER && dimension != END => Some(1),
        1 if dimension == NETHER => Some(2),
        2 if dimension == END => Some(3),
        _ => None,
    }
}

/// What is beside the budding block on one side, as the growth reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Beside {
    /// `canClusterGrowAtState`: air, or a water source block.
    Room { water: bool },
    /// A cluster of size `size` (an index of `CLUSTERS`) facing `facing` (a side index), and whether it is waterlogged.
    Cluster { size: usize, facing: usize, water: bool },
    /// Anything else.
    Other,
}

/// What one random tick does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tick {
    /// The stage the block moves on to (with the respawn anchor's charge sound).
    pub advance: Option<i32>,
    /// A cluster placed on side `side`: its size, facing that side, waterlogged if the room held water.
    pub grow: Option<Grow>,
    /// The block falls back to stage 0 (with the respawn anchor's deplete sound).
    pub depleted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grow {
    pub side: usize,
    pub size: usize,
    pub water: bool,
}

/// `randomTick` of a budding block at `stage` in `dimension`; `beside` reads the block on a side (in
/// `Direction.values()` order). The block's stage as the tick began decides the growth, as in Java.
pub fn random_tick(stage: i32, dimension: &str, settings: &CrystalSettings, beside: impl Fn(usize) -> Beside, random: &mut impl Draw) -> Tick {
    let mut tick = Tick { advance: advance_to(stage, dimension, settings), ..Tick::default() };
    if stage != 3 || random.next_int(5) != 0 {
        return tick;
    }
    let side = random.next_int(crate::SIDES as i32) as usize;
    let grow = match beside(side) {
        Beside::Room { water } => Some(Grow { side, size: 0, water }),
        Beside::Cluster { size, facing, water } if facing == side && size + 1 < CLUSTERS.len() => Some(Grow { side, size: size + 1, water }),
        _ => None,
    };
    if grow.is_some() {
        tick.grow = grow;
        tick.depleted = random.next_float() < 0.05;
    }
    tick
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(Vec<i32>, f32);
    impl Draw for Fixed {
        fn next_int(&mut self, bound: i32) -> i32 {
            let v = self.0.remove(0);
            assert!(v < bound);
            v
        }
        fn next_float(&mut self) -> f32 {
            self.1
        }
    }

    #[test]
    fn stages_follow_the_dimension() {
        let s = CrystalSettings::default();
        assert_eq!(advance_to(0, OVERWORLD, &s), Some(1));
        assert_eq!(advance_to(0, "twilightforest:twilight_forest", &s), Some(1));
        assert_eq!(advance_to(0, NETHER, &s), None);
        assert_eq!(advance_to(1, OVERWORLD, &s), None);
        assert_eq!(advance_to(1, NETHER, &s), Some(2));
        assert_eq!(advance_to(2, END, &s), Some(3));
        assert_eq!(advance_to(3, END, &s), None);
        let custom = CrystalSettings { custom_dimensions: true, stage1: vec!["a:b".into()], stage2: vec![], stage3: vec![OVERWORLD.into()] };
        assert_eq!(advance_to(0, "a:b", &custom), Some(1));
        assert_eq!(advance_to(0, OVERWORLD, &custom), None);
        assert_eq!(advance_to(2, OVERWORLD, &custom), Some(3));
    }

    #[test]
    fn a_stage_3_block_grows_clusters_one_tick_in_five() {
        let s = CrystalSettings::default();
        let room = |_| Beside::Room { water: false };
        let t = random_tick(3, OVERWORLD, &s, room, &mut Fixed(vec![1], 0.5));
        assert_eq!(t, Tick::default(), "four ticks in five grow nothing");
        let t = random_tick(3, OVERWORLD, &s, room, &mut Fixed(vec![0, 1], 0.5));
        assert_eq!(t.grow, Some(Grow { side: 1, size: 0, water: false }));
        assert!(!t.depleted);
        let t = random_tick(3, OVERWORLD, &s, |_| Beside::Room { water: true }, &mut Fixed(vec![0, 4], 0.01));
        assert_eq!(t.grow, Some(Grow { side: 4, size: 0, water: true }));
        assert!(t.depleted, "a 5% roll sends it back to stage 0");
        let grown = |facing| move |_| Beside::Cluster { size: 1, facing, water: false };
        let t = random_tick(3, OVERWORLD, &s, grown(2), &mut Fixed(vec![0, 2], 0.5));
        assert_eq!(t.grow, Some(Grow { side: 2, size: 2, water: false }), "a medium cluster facing out grows large");
        let t = random_tick(3, OVERWORLD, &s, grown(3), &mut Fixed(vec![0, 2], 0.5));
        assert_eq!(t.grow, None, "a cluster facing elsewhere doesn't");
        let full = |_| Beside::Cluster { size: 3, facing: 0, water: false };
        assert_eq!(random_tick(3, OVERWORLD, &s, full, &mut Fixed(vec![0, 0], 0.01)), Tick::default());
        let t = random_tick(0, OVERWORLD, &s, room, &mut Fixed(vec![], 0.0));
        assert_eq!(t, Tick { advance: Some(1), ..Tick::default() }, "below stage 3 nothing grows and nothing is drawn");
    }
}
