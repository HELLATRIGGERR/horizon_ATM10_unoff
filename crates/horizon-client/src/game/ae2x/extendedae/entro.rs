//! Entro in the game: its budding blocks grow clusters and wear down, an
//! Entro Seed turns a block of fluix into a fully entroized budding block.
//! Derived from ExtendedAE 1.21-2.2.42 (`BlockBuddingEntro`,
//! `BlockEntroCluster`, `ItemEntroSeed`), https://github.com/GlodBlock/ExtendedAE
//! commit 1f86f927552f02451236465c0c96a12a3de8f66c, LGPL-3.0; see
//! `ae2/addons/extendedae/`.
//!
//! The clusters are amethyst buds and clusters to the engine (their
//! declarations' templates): they stand on the block behind them and break
//! with it (`canSurvive`, `updateShape`), and drop as their loot tables say.
use super::super::super::Game;
use horizon_core::random::RandomSource;

/// The budding blocks, fully entroized first.
pub(in crate::game) const BUDDING: [&str; 4] = [
    "extendedae:entro_budding_fully",
    "extendedae:entro_budding_mostly",
    "extendedae:entro_budding_half",
    "extendedae:entro_budding_hardly",
];
/// `BlockBuddingEntro.GROWTH_CHANCE`: one random tick in three grows.
const GROWTH_CHANCE: i32 = 3;
/// `DECAY_CHANCE`: one growth in ten wears the block down.
const DECAY_CHANCE: i32 = 10;

/// A block whose random tick grows Entro (`randomTicks()` on `BlockBuddingEntro`).
pub(in crate::game) fn random_ticks(block: &str) -> bool {
    BUDDING.contains(&block)
}
/// `degradeBudding`: a step less entroized; the hardly entroized one becomes
/// AE2's quartz block. Even the fully entroized one wears down.
pub(in crate::game) fn degraded(block: &str) -> Option<&'static str> {
    Some(match block {
        "extendedae:entro_budding_fully" => "extendedae:entro_budding_mostly",
        "extendedae:entro_budding_mostly" => "extendedae:entro_budding_half",
        "extendedae:entro_budding_half" => "extendedae:entro_budding_hardly",
        "extendedae:entro_budding_hardly" => "ae2:quartz_block",
        _ => return None,
    })
}
/// `canClusterGrowAtState`, then `canClusterGrow`: a small cluster in air or
/// a source of water; a cluster facing away from the budding block one size up.
pub(in crate::game) fn next_stage(name: &str, facing: Option<&str>, direction: &str, source_water: bool) -> Option<&'static str> {
    match name {
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air" => Some("extendedae:entro_cluster_small"),
        "minecraft:water" if source_water => Some("extendedae:entro_cluster_small"),
        "extendedae:entro_cluster_small" if facing == Some(direction) => Some("extendedae:entro_cluster_medium"),
        "extendedae:entro_cluster_medium" if facing == Some(direction) => Some("extendedae:entro_cluster_large"),
        "extendedae:entro_cluster_large" if facing == Some(direction) => Some("extendedae:entro_cluster"),
        _ => None,
    }
}

impl Game<'_> {
    /// `BlockBuddingEntro.randomTick`: one tick in three, a cluster grows on a
    /// random side (`Util.getRandom(Direction.values())`), waterlogged in
    /// water; then one growth in ten wears the block down.
    pub(in crate::game) fn extendedae_entro_random_tick(&mut self, pos: [i32; 3], block: &str) {
        use horizon_world::chunk::BlockState;
        if self.remote.is_some() || !self.world.loaded(pos[0], pos[2]) || self.world.random.next_int_bound(GROWTH_CHANCE) != Ok(0) {
            return;
        }
        // Direction.values(): down, up, north, south, west, east.
        let directions = [("down", [0, -1, 0]), ("up", [0, 1, 0]), ("north", [0, 0, -1]), ("south", [0, 0, 1]), ("west", [-1, 0, 0]), ("east", [1, 0, 0])];
        let Ok(n) = self.world.random.next_int_bound(6) else { return };
        let (face, offset) = directions[n as usize];
        let target = [pos[0] + offset[0], pos[1] + offset[1], pos[2] + offset[2]];
        if !self.world.loaded(target[0], target[2]) {
            return;
        }
        let Some(state) = self.pack.assets.registry.state(self.world.block(target[0], target[1], target[2])) else { return };
        let water = state.name == "minecraft:water" && state.properties.get("level").is_none_or(|v| v == "0");
        let Some(cluster) = next_stage(&state.name, state.properties.get("facing").map(String::as_str), face, water) else { return };
        // getFluidState().getType() == Fluids.WATER: a source, or a waterlogged cluster.
        let waterlogged = water || state.properties.get("waterlogged").is_some_and(|v| v == "true");
        let mut properties = std::collections::BTreeMap::<String, String>::new();
        properties.insert("facing".into(), face.into());
        properties.insert("waterlogged".into(), waterlogged.to_string());
        let Ok(grown) = self.pack.assets.registry.resolve(&BlockState { name: cluster.into(), properties: properties.into() }) else { return };
        if self.level_set(target, grown).is_err() || self.world.random.next_int_bound(DECAY_CHANCE) != Ok(0) {
            return;
        }
        if let Some(next) = degraded(block).and_then(|b| self.pack.assets.registry.default_state(b)) {
            let _ = self.level_set(pos, next);
        }
    }

    /// `ItemEntroSeed.useOn`: on a block of fluix, one seed makes it a fully
    /// entroized budding block (`shrink(1)`, put back in creative).
    pub(in crate::game) fn extendedae_seed_use(&mut self, pos: [i32; 3]) -> bool {
        if self.remote.is_some() || !self.held().is_some_and(|s| s.item.as_ref() == "extendedae:entro_seed") {
            return false;
        }
        let here = self.world.block(pos[0], pos[1], pos[2]);
        if self.res.facts().get(here).name.as_ref() != "ae2:fluix_block" {
            return false;
        }
        let Some(budding) = self.pack.assets.registry.default_state(BUDDING[0]) else { return false };
        if self.level_set(pos, budding).is_err() {
            return false;
        }
        self.consume_held();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budding_entro_grows_up_from_air_and_water_and_wears_down() {
        assert!(BUDDING.iter().all(|b| random_ticks(b)));
        assert!(!random_ticks("ae2:flawless_budding_quartz") && !random_ticks("extendedae:entro_block"));
        assert_eq!(next_stage("minecraft:air", None, "up", false), Some("extendedae:entro_cluster_small"));
        assert_eq!(next_stage("minecraft:water", None, "up", true), Some("extendedae:entro_cluster_small"));
        assert_eq!(next_stage("minecraft:water", None, "up", false), None, "flowing water");
        assert_eq!(next_stage("extendedae:entro_cluster_small", Some("up"), "up", false), Some("extendedae:entro_cluster_medium"));
        assert_eq!(next_stage("extendedae:entro_cluster_medium", Some("north"), "north", false), Some("extendedae:entro_cluster_large"));
        assert_eq!(next_stage("extendedae:entro_cluster_large", Some("down"), "down", false), Some("extendedae:entro_cluster"));
        // A cluster on another budding block's face does not grow from this one.
        assert_eq!(next_stage("extendedae:entro_cluster_small", Some("up"), "down", false), None);
        assert_eq!(next_stage("extendedae:entro_cluster", Some("up"), "up", false), None, "a whole cluster is done");
        assert_eq!(next_stage("ae2:small_quartz_bud", Some("up"), "up", false), None);
        assert_eq!(next_stage("minecraft:stone", None, "up", false), None);
        let order: Vec<_> = std::iter::successors(Some("extendedae:entro_budding_fully"), |b| degraded(b)).collect();
        assert_eq!(order, ["extendedae:entro_budding_fully", "extendedae:entro_budding_mostly", "extendedae:entro_budding_half", "extendedae:entro_budding_hardly", "ae2:quartz_block"]);
    }
}
