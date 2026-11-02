//! The sun and the weather as Minecraft 1.21.1 computes them, which is what
//! Mekanism 10.7 reads on ATM10: `DimensionType.timeOfDay`, `Mth.cos` (a
//! 65536-entry sine table), `Level.updateSkyBrightness` (`skyDarken`),
//! `Level.isRaining` and `isThundering`, and Mekanism's
//! `WorldUtils.getSunBrightness`, a copy of `ClientLevel.getSkyDarken`.
//! Floats stay floats and doubles doubles, in Java's order, so the numbers
//! come out to the bit.
use std::sync::OnceLock;

/// `Mth.SIN`: `(float) Math.sin(i * PI * 2 / 65536)`.
fn sin_table() -> &'static [f32] {
    static TABLE: OnceLock<Vec<f32>> = OnceLock::new();
    TABLE.get_or_init(|| (0..65536).map(|i| (f64::from(i) * std::f64::consts::PI * 2.0 / 65536.0).sin() as f32).collect())
}

/// `Mth.cos(float)`: `SIN[(int) (value * 10430.378F + 16384.0F) & 65535]`
/// (Java's `f2i` saturates and takes NaN to 0, as `as i32` does).
pub fn mth_cos(value: f32) -> f32 {
    let index = (value * 10430.378_f32 + 16384.0_f32) as i32 & 65535;
    sin_table()[index as usize]
}

/// `(float) (Math.PI * 2)`, `Mth.TWO_PI`.
const TWO_PI: f32 = std::f64::consts::TAU as f32;

/// `DimensionType.timeOfDay(dayTime)` for a dimension without a fixed time
/// (the overworld): 0 at noon (6000), 0.5 at midnight.
pub fn time_of_day(day_time: i64) -> f32 {
    let d = day_time as f64 / 24000.0 - 0.25;
    let d0 = d - d.floor();
    let d1 = 0.5 - (d0 * std::f64::consts::PI).cos() / 2.0;
    (d0 * 2.0 + d1) as f32 / 3.0_f32
}

/// `Level.updateSkyBrightness`: how far the sky light is darkened, 0 (clear
/// day) to 11 (night). Java computes it at the start of a level's tick, before
/// the day time moves on.
pub fn sky_darken(day_time: i64, rain: f32, thunder: f32) -> i32 {
    let d0 = 1.0 - f64::from(rain * 5.0_f32) / 16.0;
    let d1 = 1.0 - f64::from(thunder * 5.0_f32) / 16.0;
    let d2 = 0.5 + 2.0 * f64::from(mth_cos(time_of_day(day_time) * TWO_PI)).clamp(-0.25, 0.25);
    ((1.0 - d2 * d0 * d1) * 11.0) as i32
}

/// `WorldUtils.getSunBrightness(level, 1.0F)`: 0.2 at night to 1.0 at a
/// clear noon, dimmed by rain and thunder. `thunder` is `getThunderLevel`,
/// which already counts the rain.
pub fn sun_brightness(day_time: i64, rain: f32, thunder: f32) -> f32 {
    let f = time_of_day(day_time);
    let mut f1 = 1.0_f32 - (mth_cos(f * TWO_PI) * 2.0_f32 + 0.2_f32);
    f1 = f1.clamp(0.0, 1.0);
    f1 = 1.0_f32 - f1;
    f1 = (f64::from(f1) * (1.0 - f64::from(rain * 5.0_f32) / 16.0)) as f32;
    f1 = (f64::from(f1) * (1.0 - f64::from(thunder * 5.0_f32) / 16.0)) as f32;
    f1 * 0.8_f32 + 0.2_f32
}

/// `Level.isRaining`: the rain level over 0.2.
pub fn is_raining(rain: f32) -> bool {
    f64::from(rain) > 0.2
}

/// `Level.isThundering`: in a dimension with sky light and no ceiling, the
/// thunder level (rain included) over 0.9.
pub fn is_thundering(thunder: f32, has_skylight: bool, has_ceiling: bool) -> bool {
    has_skylight && !has_ceiling && f64::from(thunder) > 0.9
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sine_table_is_vanillas() {
        assert_eq!(mth_cos(0.0), 1.0);
        assert_eq!(mth_cos(std::f32::consts::PI), -1.0);
        assert!(mth_cos(TWO_PI / 4.0).abs() < 1e-3);
        // The table's step: values between entries read the lower one.
        assert_eq!(mth_cos(0.0001), mth_cos(0.00005));
    }

    #[test]
    fn the_day_runs_from_noon_to_midnight() {
        assert_eq!(time_of_day(6000), 0.0);
        assert_eq!(time_of_day(18000), 0.5);
        assert_eq!(time_of_day(30000), 0.0, "a day later");
        assert_eq!(time_of_day(-18000), 0.0, "/time set can go negative");
        // Sunrise is a little before a quarter: the curve is eased.
        let dawn = time_of_day(0);
        assert!(dawn > 0.75 && dawn < 0.8, "{dawn}");
    }

    #[test]
    fn the_sky_darkens_at_night_and_in_storms() {
        assert_eq!(sky_darken(6000, 0.0, 0.0), 0, "clear noon");
        assert_eq!(sky_darken(6000, 1.0, 0.0), 3, "rain alone keeps the sun");
        assert_eq!(sky_darken(6000, 1.0, 1.0), 5, "a thunderstorm hides it");
        assert_eq!(sky_darken(18000, 0.0, 0.0), 11, "midnight");
        assert!(sky_darken(13000, 0.0, 0.0) >= 4, "dusk");
        assert!(sky_darken(11000, 0.0, 0.0) < 4, "late afternoon");
    }

    #[test]
    fn the_sun_is_full_at_noon_and_dim_at_night() {
        assert_eq!(sun_brightness(6000, 0.0, 0.0), 1.0);
        assert_eq!(sun_brightness(6001, 0.0, 0.0), 1.0, "a plateau around noon");
        assert_eq!(sun_brightness(18000, 0.0, 0.0), 0.2);
        // Rain: 1 - 5/16; a thunderstorm twice that.
        assert_eq!(sun_brightness(6000, 1.0, 0.0), 0.75);
        assert_eq!(sun_brightness(6000, 1.0, 1.0), 0.578125);
        // The float 0.2 is a hair over the double 0.2: it counts as rain.
        assert!(is_raining(0.2) && !is_raining(0.19));
        assert!(is_thundering(0.95, true, false));
        assert!(!is_thundering(0.95, true, true), "a ceiling: no thunder");
        assert!(!is_thundering(0.9, true, false));
    }
}
