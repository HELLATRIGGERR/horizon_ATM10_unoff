//! Mekanism Generators' Solar Generator (`TileEntitySolarGenerator` and its
//! `SolarCheck`): Joules from the sun, by the biome it stands in, the time of
//! day and the weather, out of its bottom face. Its energy store gives to
//! anything and takes only what it makes (`BasicEnergyContainer.output`).
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
}
