//! Mystical Agriculture's rules for the pack's port, translated from its source (MIT, see the bundle's
//! `LICENSE-mysticalagriculture` and `LICENSE-cucumber`). Nothing here depends on the engine.

pub mod crop_table;
pub mod crops;
pub mod infusion;

pub use crops::Tier;

/// The Growth Accelerators (`block/GrowthAcceleratorBlock.java`, `init/ModBlocks.java`).
pub mod accelerator {
    /// The accelerator's range: how many blocks above `pos + 2` it reaches.
    pub fn range(block: &str) -> Option<i32> {
        Some(match block {
            "mysticalagriculture:inferium_growth_accelerator" => 9,
            "mysticalagriculture:prudentium_growth_accelerator" => 18,
            "mysticalagriculture:tertium_growth_accelerator" => 27,
            "mysticalagriculture:imperium_growth_accelerator" => 36,
            "mysticalagriculture:supremium_growth_accelerator" => 45,
            "mysticalagriculture:awakened_supremium_growth_accelerator" => 54,
            _ => return None,
        })
    }

    /// The positions it looks at, bottom up: `pos.above(2)` to `pos.offset(0, range + 2, 0)`. The first
    /// bonemealable block there gets a random tick.
    pub fn reach(pos: [i32; 3], range: i32) -> impl Iterator<Item = [i32; 3]> {
        (pos[1] + 2..=pos[1] + range + 2).map(move |y| [pos[0], y, pos[2]])
    }

    /// `getTickRate`: `(int) (cooldown * variance) * 20`, the variance `Math.random() * 0.2 + 0.9`.
    pub fn tick_rate(cooldown_seconds: i32, random: f64) -> i32 {
        let variance = random * (1.1 - 0.9) + 0.9;
        (f64::from(cooldown_seconds) * variance) as i32 * 20
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn ranges_and_rates() {
            assert_eq!(range("mysticalagriculture:inferium_growth_accelerator"), Some(9));
            assert_eq!(range("mysticalagriculture:awakened_supremium_growth_accelerator"), Some(54));
            let r: Vec<_> = reach([0, 64, 0], 9).collect();
            assert_eq!((r[0], r[r.len() - 1], r.len()), ([0, 66, 0], [0, 75, 0], 10));
            // ATM10's 10 s: 9 to 10.99 s, whole seconds, in ticks.
            assert_eq!(tick_rate(10, 0.0), 180);
            assert_eq!(tick_rate(10, 0.5), 200);
            assert_eq!(tick_rate(10, 0.999), 200);
        }
    }
}

/// `mysticalagriculture-common.toml`, the keys the port reads (`config/ModConfigs.java`, defaults there).
pub mod config {
    use crate::crops::CropSettings;

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Config {
        pub crops: CropSettings,
        pub growth_accelerator_cooldown: i32,
    }

    impl Default for Config {
        fn default() -> Self {
            Config { crops: CropSettings::default(), growth_accelerator_cooldown: 10 }
        }
    }

    /// Reads `key = value` lines of the file's `[General]` section; anything else keeps the default.
    pub fn parse(text: &str) -> Config {
        let mut c = Config::default();
        for line in text.lines() {
            let line = line.trim();
            let Some((key, value)) = line.split_once('=') else { continue };
            let (key, value) = (key.trim(), value.trim());
            let flag = || value.parse::<bool>().ok();
            match key {
                "secondarySeedDrops" => c.crops.secondary_seed_drops = flag().unwrap_or(c.crops.secondary_seed_drops),
                "requiresEffectiveFarmland" => {
                    c.crops.requires_effective_farmland = flag().unwrap_or(c.crops.requires_effective_farmland)
                }
                "fertilizedEssenceChance" => {
                    if let Ok(v) = value.parse::<f64>() {
                        c.crops.fertilized_essence_chance = v.clamp(0.0, 1.0);
                    }
                }
                "growthAcceleratorCooldown" => {
                    if let Ok(v) = value.parse::<i32>() {
                        c.growth_accelerator_cooldown = v.max(1);
                    }
                }
                _ => {}
            }
        }
        c
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn reads_atm10s_values() {
            let text = "[General]\n\t#comment\n\tgrowthAcceleratorCooldown = 10\n\tfertilizedEssenceChance = 0.1\n\tsecondarySeedDrops = false\n";
            let c = parse(text);
            assert!(!c.crops.secondary_seed_drops);
            assert!(!c.crops.requires_effective_farmland);
            assert_eq!(c.crops.fertilized_essence_chance, 0.1);
            assert_eq!(c.growth_accelerator_cooldown, 10);
            assert_eq!(parse(""), Config::default());
        }
    }
}
