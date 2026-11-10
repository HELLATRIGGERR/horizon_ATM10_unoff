//! Mekanism Generators' Wind Generator (`TileEntityWindGenerator`): Joules
//! by the height of its top (four blocks above it, its last bounding block),
//! from `generationMin` at `minY` to `generationMax` at `maxY` in a straight
//! line, nothing when the top holds a fluid or can't see the sky; it looks
//! again every 20 of its ticks, the first on its first tick. Mekanism 10.7's
//! wind doesn't follow the weather. It puts out of its front and its bottom
//! at twice `generationMax`; a dimension tagged `c:no_wind` stops it (none is
//! on ATM10).
use crate::config::GeneratorsConfig;
use crate::container::{Automation, EnergyContainer};

/// How often it looks at its height and sky (`ticker % 20`).
pub const LOOK_EVERY: u32 = 20;
/// Its top is this far above it (`getBlockPos().above(4)`).
pub const TOP: i32 = 4;

/// The world's heights the multiplier reads: `getMinBuildHeight`,
/// `getMaxBuildHeight` (one past the top block) and the dimension type's
/// `logicalHeight`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Heights {
    pub min_build: i32,
    pub max_build: i32,
    pub logical_height: i32,
}

/// `getMultiplier`: the Joules at `top_y` over `generationMin`, for a top that
/// holds no fluid and sees the sky (`open`); 0 else. The heights clamp to
/// the world's (`minY` to its bottom, `maxY` to its logical top), and the
/// line runs in doubles as Java's (a `maxY` equal to `minY` gives no number).
pub fn multiplier(top_y: i32, open: bool, heights: Heights, config: &GeneratorsConfig) -> f64 {
    if !open {
        return 0.0;
    }
    let max_level = heights.max_build.min(heights.min_build.saturating_add(heights.logical_height)) - 1;
    let min_y = config.wind_min_y.max(heights.min_build);
    let max_y = config.wind_max_y.min(max_level);
    let clamped = max_y.min(min_y.max(top_y));
    let (min_g, max_g) = (config.wind_generation_min as i64, config.wind_generation_max as i64);
    let slope = (max_g.wrapping_sub(min_g)) as f64 / f64::from(max_y - min_y);
    let to_gen = min_g as f64 + slope * f64::from(clamped - min_y);
    to_gen / min_g as f64
}

/// `getCurrentGeneration`: `generationMin` times the multiplier, cut to a `long`.
pub fn generation(multiplier: f64, config: &GeneratorsConfig) -> u64 {
    crate::clamp_to_long(config.wind_generation_min as f64 * multiplier).max(0) as u64
}

/// `TileEntityGenerator`'s max output: twice `generationMax`.
pub fn max_output(config: &GeneratorsConfig) -> u64 {
    config.wind_generation_max.saturating_mul(2)
}

/// What a wind generator keeps between ticks and doesn't save: its
/// `ticker` and `currentMultiplier`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wind {
    pub ticker: u32,
    pub multiplier: f64,
}

/// What a tick did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Blown {
    /// `setActive`'s value when it looked this tick.
    pub active: Option<bool>,
    /// Joules that went into its store.
    pub produced: u64,
}

impl Wind {
    /// `onUpdateServer` after the ejector: on every 20th tick `look` gives
    /// the multiplier anew; while it isn't 0 and the store has room, the
    /// generation goes in (what fits). A blacklisted dimension does nothing.
    pub fn tick(&mut self, energy: &mut EnergyContainer, can_function: bool, blacklisted: bool, config: &GeneratorsConfig, look: impl FnOnce() -> f64) -> Blown {
        let mut blown = Blown { active: None, produced: 0 };
        if !blacklisted {
            if self.ticker.is_multiple_of(LOOK_EVERY) {
                self.multiplier = look();
                blown.active = Some(can_function && self.multiplier != 0.0);
            }
            if self.multiplier != 0.0 && can_function && energy.needed() > 0 {
                let generation = generation(self.multiplier, config);
                blown.produced = generation - energy.insert(generation, false, Automation::Internal);
            }
        }
        self.ticker = self.ticker.wrapping_add(1);
        blown
    }

    /// `getProductionRate`: the generation while active.
    pub fn production(&self, active: bool, config: &GeneratorsConfig) -> u64 {
        if active { generation(self.multiplier, config) } else { 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OVERWORLD: Heights = Heights { min_build: -64, max_build: 320, logical_height: 384 };

    fn atm10() -> GeneratorsConfig {
        GeneratorsConfig { wind_generation_min: 150, wind_generation_max: 900, wind_min_y: 24, wind_max_y: 2031, wind_storage: 200_000, ..GeneratorsConfig::default() }
    }

    #[test]
    fn the_wind_grows_with_height() {
        // Measured on the oracle (g2/w1): generators at y -60, 0, 100, 200 and 315 (tops 4 higher)
        // made 150, 150, 353, 607 and 900 J a tick; one with stone over its top none.
        let config = atm10();
        let at = |y: i32| generation(multiplier(y + TOP, true, OVERWORLD, &config), &config);
        assert_eq!([-60, 0, 100, 200, 315].map(at), [150, 150, 353, 607, 900]);
        assert_eq!(generation(multiplier(104, false, OVERWORLD, &config), &config), 0, "blocked");
        assert_eq!(max_output(&config), 1800);
        // The Nether's ceiling: a logical height of 128 caps maxY at 127.
        let nether = Heights { min_build: 0, max_build: 256, logical_height: 128 };
        assert_eq!(generation(multiplier(130, true, nether, &config), &config), 900);
        assert_eq!(generation(multiplier(120, true, nether, &config), &config), 849);
        // minY clamps to the world's bottom: a minY of -100 in the overworld reads -64.
        let low = GeneratorsConfig { wind_min_y: -100, ..config };
        assert_eq!(generation(multiplier(-64, true, OVERWORLD, &low), &low), 150);
    }

    #[test]
    fn it_looks_every_20_ticks() {
        // w1: stone put over a working generator's top after its 25th tick: it made 353 J a tick
        // until its 40th, and nothing from the 41st, when it looked again.
        let config = atm10();
        let mut wind = Wind::default();
        let mut store = EnergyContainer::output(config.wind_storage);
        let mut made = Vec::new();
        for t in 1..=49 {
            let open = t <= 25;
            let blown = wind.tick(&mut store, true, false, &config, || multiplier(104, open, OVERWORLD, &config));
            if t == 1 {
                assert_eq!(blown.active, Some(true));
            }
            if t == 41 {
                assert_eq!(blown.active, Some(false));
            }
            made.push(blown.produced);
        }
        assert!(made[..40].iter().all(|j| *j == 353));
        assert!(made[40..].iter().all(|j| *j == 0));
        assert_eq!(store.stored, 40 * 353);
        let mut full = EnergyContainer::output(1000);
        full.stored = 900;
        let mut w = Wind::default();
        assert_eq!(w.tick(&mut full, true, false, &config, || 1.0).produced, 100, "what fits");
        assert_eq!(w.tick(&mut full, true, false, &config, || 1.0).produced, 0);
        let mut b = Wind::default();
        assert_eq!(b.tick(&mut EnergyContainer::output(1000), true, true, &config, || 1.0), Blown { active: None, produced: 0 }, "no wind");
    }
}
