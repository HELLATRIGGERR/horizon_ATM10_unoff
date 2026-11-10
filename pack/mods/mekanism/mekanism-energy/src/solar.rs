//! Mekanism Generators' Solar Generator (`TileEntitySolarGenerator` and its
//! `SolarCheck`): Joules from the sun, by the biome it stands in, the time of
//! day and the weather, out of its bottom face. Its energy store gives to
//! anything and takes only what it makes (`BasicEnergyContainer.output`).
//! The Advanced Solar Generator (`TileEntityAdvancedSolarGenerator`,
//! `AdvancedSolar`) reads nine checks over the three by three panel two
//! blocks above it, each looking at its sky every 10 to 30 ticks, and works
//! while more than four of them see the sun.
use crate::container::{Automation, EnergyContainer};
use crate::sky;

/// What `SolarCheck` takes from the biome where the generator stands, once,
/// on its first tick after it is placed or loaded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolarCheck {
    /// The biome has rain or snow (`getPrecipitationAt != NONE`): only then do
    /// humidity and the weather count.
    pub needs_rain_check: bool,
    /// The biome's peak: `1 + 0.3 (0.8 - temperature) - 0.3 downfall`, the
    /// downfall only where it can rain.
    pub peak: f32,
}

impl SolarCheck {
    /// `new SolarCheck(level, pos)`: `temperature` is `Biome.getTemperature(pos)`,
    /// `downfall` the biome's, `precipitation` whether it has rain or snow.
    pub fn new(temperature: f32, downfall: f32, precipitation: bool) -> Self {
        let temp_eff = 0.3_f32 * (0.8_f32 - temperature);
        let humidity_eff = if precipitation { -0.3_f32 * downfall } else { 0.0 };
        Self { needs_rain_check: precipitation, peak: 1.0_f32 + temp_eff + humidity_eff }
    }

    /// `getGenerationMultiplier`: none without the sun; a fifth of the peak
    /// while it rains or thunders, where it can rain.
    pub fn generation_multiplier(&self, sees_sun: bool, raining: bool) -> f32 {
        if !sees_sun {
            0.0
        } else if self.needs_rain_check && raining {
            self.peak * 0.2_f32
        } else {
            self.peak
        }
    }
}

/// `WorldUtils.canSeeSun`: a dimension with sky light, a sky darkened less
/// than 4 (the day, and not a thunderstorm) and full sky light at the
/// generator (`canSeeSky`).
pub fn sees_sun(has_skylight: bool, sky_darken: i32, sky_light: u8) -> bool {
    has_skylight && sky_darken < 4 && sky_light >= 15
}

/// The weather and time a solar generator reads in its tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sky {
    pub day_time: i64,
    /// `getRainLevel(1)`, 0 to 1.
    pub rain: f32,
    /// `getThunderLevel(1)`, rain included.
    pub thunder: f32,
    pub has_skylight: bool,
    pub has_ceiling: bool,
}

impl Sky {
    /// `isRaining() || isThundering()`.
    pub fn raining(&self) -> bool {
        sky::is_raining(self.rain) || sky::is_thundering(self.thunder, self.has_skylight, self.has_ceiling)
    }
}

/// `getProduction`: `solarGeneration * (brightness * multiplier)`, a `long`
/// times a `float` in float, then cut to a `long` (none below zero).
pub fn production(solar_generation: u64, brightness: f32, multiplier: f32) -> u64 {
    let joules = solar_generation as f32 * (brightness * multiplier);
    crate::clamp_to_long(f64::from(joules)).max(0) as u64
}

/// `getMaxOutput`: twice the peak's production (`updateMaxOutputRaw`), or
/// twice `solarGeneration` before the first tick has looked at the biome.
pub fn max_output(solar_generation: u64, check: Option<&SolarCheck>) -> u64 {
    let raw = match check {
        Some(c) => crate::clamp_to_long(f64::from(solar_generation as f32 * c.peak)).max(0) as u64,
        None => solar_generation,
    };
    raw.saturating_mul(2).min(i64::MAX as u64)
}

/// What one tick made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Made {
    /// `seesSun`.
    pub sees_sun: bool,
    /// `setActive`: it sees the sun and has room.
    pub active: bool,
    /// `lastProductionAmount`: the Joules that went in.
    pub produced: u64,
}

/// The rest of `onUpdateServer` after the ejector (`TileEntityGenerator`) ran:
/// with the sun and room, the production goes in (`INTERNAL`); without, it
/// stops.
pub fn generate(container: &mut EnergyContainer, solar_generation: u64, check: &SolarCheck, sky: &Sky, sky_light: u8) -> Made {
    let darken = sky::sky_darken(sky.day_time, sky.rain, sky.thunder);
    let sees = sees_sun(sky.has_skylight, darken, sky_light);
    if !sees || container.needed() == 0 {
        return Made { sees_sun: sees, active: false, produced: 0 };
    }
    let brightness = sky::sun_brightness(sky.day_time, sky.rain, sky.thunder);
    let joules = production(solar_generation, brightness, check.generation_multiplier(sees, sky.raining()));
    let rest = container.insert(joules, false, Automation::Internal);
    Made { sees_sun: true, active: true, produced: joules - rest }
}

/// Where the advanced solar generator's checks stand, relative to it
/// (`recheckSettings`): the panel's centre (`solarCheck`, two above it),
/// then `solarChecks` 0 to 7: x - 1 by z, the centre column's north and
/// south, x + 1 by z.
pub const ADVANCED_CHECKS: [[i32; 3]; 9] =
    [[0, 2, 0], [-1, 2, -1], [-1, 2, 0], [-1, 2, 1], [0, 2, -1], [0, 2, 1], [1, 2, -1], [1, 2, 0], [1, 2, 1]];

/// One `AdvancedSolarCheck`: a `SolarCheck` that looks at its sky only
/// every `frequency` ticks (`Mth.nextInt(random, 10, 30)`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdvancedCheck {
    pub check: SolarCheck,
    pub frequency: u32,
    /// `lastCheckedSun`; none before its first look, which comes at once
    /// (Java's 0 against a world's game time).
    pub last_checked: Option<i64>,
    /// `canSeeSun`.
    pub sees: bool,
}

/// The advanced solar generator's nine checks, in `ADVANCED_CHECKS` order,
/// made on its first tick (`recheckSettings`) and kept until it unloads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdvancedSolar {
    pub checks: [AdvancedCheck; 9],
}

/// More than this many of its nine checks must see the sun (`count > 4`).
pub const ADVANCED_SEES_OVER: u32 = 4;

impl AdvancedSolar {
    /// The checks over the biomes at `ADVANCED_CHECKS` and their frequencies.
    pub fn new(checks: [SolarCheck; 9], frequencies: [u32; 9]) -> Self {
        let mut i = 0;
        Self {
            checks: checks.map(|check| {
                let frequency = frequencies[i];
                i += 1;
                AdvancedCheck { check, frequency, last_checked: None, sees: false }
            }),
        }
    }

    /// `recheckSettings`' raw max output: `advancedSolarGeneration` times the
    /// mean peak, the peaks summed in float in check order.
    pub fn peak_output(&self, generation: u64) -> u64 {
        let total = self.checks.iter().fold(0.0_f32, |sum, c| sum + c.check.peak);
        crate::clamp_to_long(f64::from(generation as f32 * (total / 9.0_f32))).max(0) as u64
    }

    /// `checkCanSeeSun`: each check looks again when its time came
    /// (`recheckCanSeeSun`); none sees while the dimension has no sky light
    /// or the sky is darkened 4 or more; `sees_at(i)` reads check `i`'s sky.
    /// Whether more than four see the sun.
    pub fn recheck(&mut self, time: i64, has_skylight: bool, sky_darken: i32, mut sees_at: impl FnMut(usize) -> bool) -> bool {
        let mut count = 0;
        for (i, c) in self.checks.iter_mut().enumerate() {
            if has_skylight && sky_darken < 4 {
                if c.last_checked.is_none_or(|last| time >= last + i64::from(c.frequency)) {
                    c.last_checked = Some(time);
                    c.sees = sees_at(i);
                }
            } else {
                c.sees = false;
            }
            if c.sees {
                count += 1;
            }
        }
        count > ADVANCED_SEES_OVER
    }

    /// `getProduction`: `advancedSolarGeneration * (brightness * mean)`, the
    /// mean of the checks' generation multipliers summed in float.
    pub fn production(&self, generation: u64, brightness: f32, raining: bool) -> u64 {
        let sum = self.checks.iter().fold(0.0_f32, |sum, c| sum + c.check.generation_multiplier(c.sees, raining));
        production(generation, brightness, sum / 9.0_f32)
    }
}

/// The advanced solar generator's `getMaxOutput`: twice the mean peak's
/// production, or twice `advancedSolarGeneration` before its first tick.
pub fn advanced_max_output(generation: u64, solar: Option<&AdvancedSolar>) -> u64 {
    solar.map_or(generation, |s| s.peak_output(generation)).saturating_mul(2).min(i64::MAX as u64)
}

/// `onUpdateServer` of the advanced solar generator after the ejector: its
/// checks look at the sky, and with more than four seeing the sun and room
/// in its store, its production goes in.
pub fn advanced_generate(
    container: &mut EnergyContainer,
    generation: u64,
    solar: &mut AdvancedSolar,
    sky: &Sky,
    time: i64,
    sees_at: impl FnMut(usize) -> bool,
) -> Made {
    let darken = sky::sky_darken(sky.day_time, sky.rain, sky.thunder);
    let sees = solar.recheck(time, sky.has_skylight, darken, sees_at);
    if !sees || container.needed() == 0 {
        return Made { sees_sun: sees, active: false, produced: 0 };
    }
    let brightness = sky::sun_brightness(sky.day_time, sky.rain, sky.thunder);
    let joules = solar.production(generation, brightness, sky.raining());
    let rest = container.insert(joules, false, Automation::Internal);
    Made { sees_sun: true, active: true, produced: joules - rest }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOON: Sky = Sky { day_time: 6000, rain: 0.0, thunder: 0.0, has_skylight: true, has_ceiling: false };

    fn tick(check: &SolarCheck, sky: &Sky, light: u8) -> Made {
        let mut container = EnergyContainer::output(96_000);
        generate(&mut container, 150, check, sky, light)
    }

    /// ATM10's 150 J/t in plains (0.8, downfall 0.4, rain): a peak of 0.88,
    /// the design's first number.
    #[test]
    fn plains_at_a_clear_noon_make_132_joules() {
        let plains = SolarCheck::new(0.8, 0.4, true);
        assert_eq!(plains.peak, 0.88);
        assert_eq!(tick(&plains, &NOON, 15), Made { sees_sun: true, active: true, produced: 132 });
        assert_eq!(max_output(150, Some(&plains)), 264);
        assert_eq!(max_output(150, None), 300, "before the first tick");
    }

    #[test]
    fn rain_takes_four_fifths_and_dims_the_sun() {
        let plains = SolarCheck::new(0.8, 0.4, true);
        let rain = Sky { rain: 1.0, ..NOON };
        // 150 * (0.75 * 0.176): the sun at 0.75, the peak at a fifth.
        assert_eq!(tick(&plains, &rain, 15).produced, 19);
        // A thunderstorm darkens the sky to 5: no sun at all.
        let storm = Sky { rain: 1.0, thunder: 1.0, ..NOON };
        assert_eq!(tick(&plains, &storm, 15), Made { sees_sun: false, active: false, produced: 0 });
        // Rain just starting (a level under 0.2 is not raining yet): the sun dims only.
        let starting = Sky { rain: 0.19, ..NOON };
        assert_eq!(tick(&plains, &starting, 15).produced, production(150, sky::sun_brightness(6000, 0.19, 0.0), 0.88));
        assert_eq!(tick(&plains, &starting, 15).produced, 125);
        assert_eq!(tick(&plains, &Sky { rain: 0.2, ..NOON }, 15).produced, 25, "from 0.2 it rains");
    }

    #[test]
    fn biomes_move_the_peak() {
        // Desert: hot (2.0) and dry, no rain check: 1 + 0.3 * (0.8 - 2.0).
        let desert = SolarCheck::new(2.0, 0.0, false);
        assert!(!desert.needs_rain_check);
        assert_eq!(tick(&desert, &NOON, 15).produced, 96);
        assert_eq!(tick(&desert, &Sky { rain: 1.0, ..NOON }, 15).produced, 72, "rain only dims the sun there");
        // Snowy plains: cold (0.0), downfall 0.5, snow counts as weather.
        let snowy = SolarCheck::new(0.0, 0.5, true);
        assert_eq!(tick(&snowy, &NOON, 15).produced, 163);
        assert_eq!(tick(&snowy, &Sky { rain: 1.0, ..NOON }, 15).produced, 24, "snow counts as rain");
        assert_eq!(max_output(150, Some(&snowy)), 326);
        let storm = Sky { rain: 1.0, thunder: 1.0, ..NOON };
        assert_eq!(tick(&desert, &storm, 15).produced, 0, "a storm darkens the sky over a desert too");
    }

    #[test]
    fn night_shade_and_a_full_store_make_nothing() {
        let plains = SolarCheck::new(0.8, 0.4, true);
        assert_eq!(tick(&plains, &Sky { day_time: 18000, ..NOON }, 15).produced, 0, "midnight");
        assert_eq!(tick(&plains, &NOON, 14), Made { sees_sun: false, active: false, produced: 0 }, "a roof");
        assert_eq!(tick(&plains, &Sky { has_skylight: false, ..NOON }, 15).produced, 0, "the Nether");
        let mut full = EnergyContainer::output(96_000);
        full.stored = 95_950;
        let made = generate(&mut full, 150, &plains, &NOON, 15);
        assert_eq!(made, Made { sees_sun: true, active: true, produced: 50 }, "only what fits");
        assert_eq!(generate(&mut full, 150, &plains, &NOON, 15), Made { sees_sun: true, active: false, produced: 0 });
    }

    #[test]
    fn its_store_takes_only_its_own_production() {
        let mut c = EnergyContainer::output(96_000);
        assert_eq!(c.insert(500, false, Automation::External), 500, "a neighbour can't fill it");
        assert_eq!(c.insert(500, false, Automation::Internal), 0);
        assert_eq!(c.extract(200, false, Automation::External), 200, "anything can draw from it");
        assert_eq!(c.stored, 300);
    }

    fn advanced(check: SolarCheck) -> AdvancedSolar {
        AdvancedSolar::new([check; 9], [10, 11, 12, 13, 14, 15, 16, 17, 18])
    }

    #[test]
    fn the_advanced_solar_generator_reads_nine_checks() {
        // Measured on the oracle (g2/w1, w1b): 1056 J a tick in plains at a clear noon and at 9000,
        // 2400 J out on its first tick (twice 1200), then 2112 split between a cube in front and
        // one below; three of the eight outer checks shaded 704, four 586, five none.
        let plains = SolarCheck::new(0.8, 0.4, true);
        let mut solar = advanced(plains);
        assert_eq!(advanced_max_output(1200, None), 2400);
        assert_eq!(advanced_max_output(1200, Some(&solar)), 2112);
        let mut store = EnergyContainer::output(2_500_000);
        let made = advanced_generate(&mut store, 1200, &mut solar, &NOON, 0, |_| true);
        assert_eq!(made, Made { sees_sun: true, active: true, produced: 1056 });
        let afternoon = Sky { day_time: 9000, ..NOON };
        assert_eq!(advanced_generate(&mut store, 1200, &mut solar, &afternoon, 1, |_| true).produced, 1056);
        for (shaded, made) in [(3, 704), (4, 586), (5, 0)] {
            let mut s = advanced(plains);
            let m = advanced_generate(&mut EnergyContainer::output(2_500_000), 1200, &mut s, &NOON, 0, |i| i == 0 || i > shaded);
            assert_eq!(m.produced, made, "{shaded} shaded");
            assert_eq!(m.sees_sun, made > 0);
        }
    }

    #[test]
    fn advanced_checks_look_again_on_their_own_time() {
        let plains = SolarCheck::new(0.8, 0.4, true);
        let mut solar = advanced(plains);
        assert!(solar.recheck(100, true, 0, |_| true), "the first look is at once");
        // A roof over all: each check sees it only when its time comes (10 to 18 ticks here).
        let seen: Vec<bool> = (101..=118).map(|t| solar.recheck(t, true, 0, |_| false)).collect();
        assert_eq!(seen.iter().position(|s| !s), Some(13), "the fifth check to look again leaves four");
        // Night or no sky light: none at once, whatever the times.
        let mut solar = advanced(plains);
        solar.recheck(0, true, 0, |_| true);
        assert!(!solar.recheck(1, true, 4, |_| true));
        assert!(solar.checks.iter().all(|c| !c.sees));
        assert!(!solar.recheck(2, false, 0, |_| true));
        // A desert's peak runs the max output down: 1 + 0.3 * (0.8 - 2.0) = 0.64, a hair under in
        // float, so 767 J and not 768.
        let desert = advanced(SolarCheck::new(2.0, 0.0, false));
        assert_eq!(advanced_max_output(1200, Some(&desert)), 2 * 767);
    }
}
