//! Mekanism's radiation (`mekanism.common.lib.radiation`): a level's sources (`RadiationLevelData`, a magnitude in
//! Sv/h at a block, multiplied by `sourceDecayRate` on one server tick in 20 and gone under 1e-5), the level they give
//! a block (`computeExposure`: each source within `chunkCheckRadius` chunks of it on every axis, its magnitude over
//! the squared distance, at least 1), an entity's dose (`RadiationEntity`: on one tick in 20 an hour's share of the
//! level, less its shielding, then `targetDecayRate`), its severity and the hurt it does, and the Geiger Counter's and
//! Dosimeter's units and decay times (`UnitDisplayUtils.getDisplayShort(.., SV/SVH, 3)`, `TextUtils.getHoursMinutes`).
//! Config: `general.toml` `[radiation]` and `common.toml` `enableDecayTimers`.

use std::collections::BTreeMap;

/// `baselineRadiation`: the level and the least dose of a world with no source.
pub const BASELINE: f64 = 1.0E-7;
/// `minRadiationMagnitude`: a source below it is gone, a dose below it does nothing.
pub const MIN_MAGNITUDE: f64 = 1.0E-5;
/// One server tick in this many decays the sources, one entity tick in this many doses and decays it
/// (`nextInt(20) == 0`).
pub const ONE_IN: u32 = 20;

/// `general.toml` `[radiation]` and `common.toml` `enableDecayTimers`; Mekanism's defaults.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadiationConfig {
    pub enabled: bool,
    pub chunk_check_radius: i32,
    pub source_decay_rate: f64,
    pub target_decay_rate: f64,
    pub min_severity: f64,
    pub decay_timers: bool,
}

impl Default for RadiationConfig {
    fn default() -> Self {
        Self { enabled: true, chunk_check_radius: 5, source_decay_rate: 0.9995, target_decay_rate: 0.9995, min_severity: 0.1, decay_timers: true }
    }
}

impl RadiationConfig {
    pub fn from_tables(general: Option<&toml::Table>, common: Option<&toml::Table>) -> Self {
        let mut c = Self::default();
        let num = |t: &toml::Table, k: &str| t.get(k).and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)));
        if let Some(r) = general.and_then(|g| g.get("radiation")).and_then(toml::Value::as_table) {
            if let Some(v) = r.get("enabled").and_then(toml::Value::as_bool) {
                c.enabled = v;
            }
            if let Some(v) = r.get("chunkCheckRadius").and_then(toml::Value::as_integer).filter(|v| (1..=100).contains(v)) {
                c.chunk_check_radius = v as i32;
            }
            for (key, slot) in [
                ("sourceDecayRate", &mut c.source_decay_rate),
                ("targetDecayRate", &mut c.target_decay_rate),
                ("negativeEffectsMinSeverity", &mut c.min_severity),
            ] {
                if let Some(v) = num(r, key).filter(|v| (0.0..=1.0).contains(v)) {
                    *slot = v;
                }
            }
        }
        if let Some(v) = common.and_then(|t| t.get("enableDecayTimers")).and_then(toml::Value::as_bool) {
            c.decay_timers = v;
        }
        c
    }

    /// `MAX_BLOCK_RANGE`: how far a source reaches on each axis.
    pub fn block_range(&self) -> i32 {
        self.chunk_check_radius * 16
    }
}

/// `Vec3i.distSqr`.
pub fn dist_sqr(a: [i32; 3], b: [i32; 3]) -> f64 {
    (0..3).map(|i| (f64::from(a[i]) - f64::from(b[i])).powi(2)).sum()
}

/// `RadiationUtil.computeExposure`: what a source of `magnitude` at `source` gives the block `at`.
pub fn exposure(magnitude: f64, source: [i32; 3], at: [i32; 3]) -> f64 {
    magnitude / dist_sqr(at, source).max(1.0)
}

/// `RadiationLevelData`: a level's sources, a magnitude a position.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sources {
    sources: BTreeMap<[i32; 3], f64>,
}

impl Sources {
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    pub fn len(&self) -> usize {
        self.sources.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = ([i32; 3], f64)> + '_ {
        self.sources.iter().map(|(p, m)| (*p, *m))
    }

    /// The source at `pos`, if any.
    pub fn at(&self, pos: [i32; 3]) -> Option<f64> {
        self.sources.get(&pos).copied()
    }

    /// `radiate(pos, magnitude)`: a new source, or more for the one there.
    pub fn radiate(&mut self, pos: [i32; 3], magnitude: f64) {
        match self.sources.get_mut(&pos) {
            Some(m) => *m += magnitude,
            None => {
                self.sources.insert(pos, magnitude);
            }
        }
    }

    /// `getRadiationLevelAndMaxMagnitude`: the level at `at` (the baseline and every source within `range` blocks on
    /// each axis) and the greatest of those sources (at least the baseline).
    pub fn level_and_max(&self, at: [i32; 3], range: i32) -> (f64, f64) {
        let (mut level, mut max) = (BASELINE, BASELINE);
        for (p, m) in &self.sources {
            if (0..3).all(|i| (i64::from(at[i]) - i64::from(p[i])).abs() <= i64::from(range)) {
                level += exposure(*m, *p, at);
                max = max.max(*m);
            }
        }
        (level, max)
    }

    /// `decay`: each source times `rate`, those under `MIN_MAGNITUDE` gone.
    pub fn decay(&mut self, rate: f64) {
        self.sources.retain(|_, m| {
            *m *= rate;
            *m >= MIN_MAGNITUDE
        });
    }

    /// `removeRadiationSource`: whether there was one.
    pub fn remove_at(&mut self, pos: [i32; 3]) -> bool {
        self.sources.remove(&pos).is_some()
    }

    /// `removeRadiationSources(chunkX, chunkZ)`: those whose position is in the chunk.
    pub fn remove_chunk(&mut self, chunk: [i32; 2]) -> bool {
        let before = self.sources.len();
        self.sources.retain(|p, _| [p[0].div_euclid(16), p[2].div_euclid(16)] != chunk);
        before != self.sources.len()
    }

    pub fn clear(&mut self) {
        self.sources.clear();
    }
}

// ---- Doses ---------------------------------------------------------------------------------------------------------

/// `RadiationEntity.radiate`: `magnitude` more (none for 0 or less).
pub fn radiated(dose: f64, magnitude: f64) -> f64 {
    if magnitude > 0.0 { dose + magnitude } else { dose }
}

/// `RadiationManager.radiate(entity, magnitude)`: the part of `magnitude` its shielding lets through.
pub fn shielded(magnitude: f64, resistance: f64) -> f64 {
    magnitude * (1.0 - resistance.min(1.0))
}

/// `PlayerExposure.updateEntityRadiation`'s one tick in 20: an hour's share of the level the entity stands in (when it
/// is above the baseline and the entity takes radiation), then the dose decays (`RadiationEntity.decay`, never under
/// the baseline).
pub fn dose_event(dose: f64, level: f64, resistance: f64, takes: bool, decay_rate: f64) -> f64 {
    let dose = if level > BASELINE && takes { radiated(dose, shielded(level / 3600.0, resistance)) } else { dose };
    decayed(dose, decay_rate)
}

/// `RadiationEntity.decay` (`set`, at least the baseline).
pub fn decayed(dose: f64, rate: f64) -> f64 {
    (dose * rate).max(BASELINE)
}

/// `ItemHazmatSuitArmor.getShieldingByArmor`: the shielding of a hazmat piece.
pub fn hazmat_shielding(item: &str) -> Option<f64> {
    match item {
        "mekanism:hazmat_mask" => Some(0.25),
        "mekanism:hazmat_gown" => Some(0.4),
        "mekanism:hazmat_pants" => Some(0.2),
        "mekanism:hazmat_boots" => Some(0.15),
        _ => None,
    }
}

/// `RadiationUtil.getRadiationResistance` over the armour (head, chest, legs, feet, as `ARMOR_SLOTS`).
pub fn resistance<'a>(armour: impl IntoIterator<Item = &'a str>) -> f64 {
    armour.into_iter().filter_map(hazmat_shielding).sum()
}

const LOG_BASELINE: f64 = -5.0;
const SCALE: f64 = 2.0 - LOG_BASELINE;

/// `RadiationScale.getScaledDoseSeverity`: 0 under `MIN_MAGNITUDE`, 1 from 100 Sv, logarithmic between.
pub fn severity(dose: f64) -> f64 {
    if dose < MIN_MAGNITUDE { 0.0 } else { ((-LOG_BASELINE + dose.log10()) / SCALE).clamp(0.0, 1.0) }
}

/// `RadiationEntity.update`'s roll, `draw` its `nextDouble`: the strength of the hurt (and the exhaustion) a dose
/// does this tick, or none (its severity at the baseline, or under the chance drawn above `negativeEffectsMinSeverity`).
pub fn effect(dose: f64, min_severity: f64, draw: f64) -> Option<f32> {
    let s = severity(dose);
    if s <= BASELINE {
        return None;
    }
    let chance = min_severity + draw * (1.0 - min_severity);
    (s > chance).then(|| 1.0f32.max(dose.ln_1p() as f32))
}

/// `RadiationScale.getSeverityColor`, as the chat colour of the Geiger Counter's and Dosimeter's number.
pub fn severity_color(magnitude: f64) -> &'static str {
    if magnitude <= BASELINE {
        "green"
    } else if magnitude < 1.0E-5 {
        "gray"
    } else if magnitude < 0.001 {
        "yellow"
    } else if magnitude < 0.1 {
        "gold"
    } else if magnitude < 10.0 {
        "red"
    } else {
        "dark_red"
    }
}

/// `getDisplayShort(value, SVH or SV, 3)`: `11.111 Sv/h`, `99.999 nSv/h`.
pub fn display(value: f64, per_hour: bool) -> String {
    crate::display::short_places(value, if per_hour { "Sv/h" } else { "Sv" }, 3)
}

/// `RadiationUtil.getDecayTime`: ticks until `magnitude` decays under `MIN_MAGNITUDE`, 20 a decay.
pub fn decay_ticks(magnitude: f64, rate: f64) -> i64 {
    let (mut m, mut ticks) = (magnitude, 0i64);
    // A rate of 1 never decays; Mekanism would loop for ever.
    while m > MIN_MAGNITUDE && ticks < 20 * 100_000_000 {
        m *= rate;
        ticks += 20;
    }
    ticks
}

/// `TextUtils.getHoursMinutes`: `1h 5m`, `12m`.
pub fn hours_minutes(ticks: i64, tick_rate: f32) -> String {
    let minutes = (ticks as f64 / (60.0 * f64::from(tick_rate.max(1.0)))).ceil() as i64;
    let hours = minutes / 60;
    if hours > 0 { format!("{hours}h {}m", minutes % 60) } else { format!("{minutes}m") }
}

/// `ChemicalStack.getRadioactivity`: the radiation `amount` mB of a chemical of `radioactivity` a mB spread.
pub fn radioactivity(radioactivity: f64, amount: i64) -> f64 {
    radioactivity * amount as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atm10_config() {
        let general: toml::Table = "[radiation]\nenabled = true\nchunkCheckRadius = 5\nsourceDecayRate = 0.9975\n\
                                    targetDecayRate = 0.9975\nnegativeEffectsMinSeverity = 0.1\n"
            .parse()
            .unwrap();
        let common: toml::Table = "enableDecayTimers = false\n".parse().unwrap();
        let c = RadiationConfig::from_tables(Some(&general), Some(&common));
        let got = (c.enabled, c.block_range(), c.source_decay_rate, c.target_decay_rate, c.min_severity, c.decay_timers);
        assert_eq!(got, (true, 80, 0.9975, 0.9975, 0.1, false));
    }

    #[test]
    fn levels_by_distance_and_range() {
        let mut s = Sources::default();
        s.radiate([0, 0, 0], 100.0);
        assert_eq!(s.level_and_max([0, 0, 0], 80), (100.0000001, 100.0));
        assert_eq!(s.level_and_max([1, 0, 0], 80).0, 100.0000001);
        assert_eq!(s.level_and_max([3, 0, 0], 80).0, 100.0 / 9.0 + BASELINE);
        assert_eq!(s.level_and_max([80, 0, 0], 80).0, 100.0 / 6400.0 + BASELINE);
        assert_eq!(s.level_and_max([0, 81, 0], 80), (BASELINE, BASELINE));
        s.radiate([0, 0, 0], 1.0);
        assert_eq!(s.at([0, 0, 0]), Some(101.0));
        assert!(s.remove_chunk([0, 0]) && s.is_empty());
    }

    #[test]
    fn sources_decay_and_go() {
        let mut s = Sources::default();
        s.radiate([5, 0, 5], 100.0);
        s.radiate([-5, 0, 5], 1.0E-5);
        s.decay(0.9975);
        assert_eq!(s.iter().collect::<Vec<_>>(), vec![([5, 0, 5], 99.75)]);
        s.decay(0.9975);
        assert_eq!(s.at([5, 0, 5]), Some(99.75 * 0.9975));
    }

    #[test]
    fn doses() {
        // An hour's share of 25 Sv/h, then the decay; a mask lets 0.75 through, the whole suit none.
        let level = 100.0 / 4.0 + BASELINE;
        assert_eq!(dose_event(BASELINE, level, 0.0, true, 0.9975), (BASELINE + level / 3600.0) * 0.9975);
        let mask = resistance(["mekanism:hazmat_mask"]);
        assert_eq!(dose_event(BASELINE, level, mask, true, 0.9975), (BASELINE + level / 3600.0 * 0.75) * 0.9975);
        let suit = resistance(["mekanism:hazmat_mask", "mekanism:hazmat_gown", "mekanism:hazmat_pants", "mekanism:hazmat_boots"]);
        assert!(suit >= 1.0);
        assert_eq!(dose_event(BASELINE, level, suit, true, 0.9975), BASELINE);
        assert_eq!(dose_event(BASELINE, level, 0.0, false, 0.9975), BASELINE);
        // The oracle's 10.0000001 Sv after a decay.
        assert_eq!(decayed(10.0000001, 0.9975), 9.97500009975);
    }

    #[test]
    fn severity_and_effects() {
        assert_eq!(severity(1.0E-6), 0.0);
        assert_eq!(severity(100.0), 1.0);
        assert!((severity(1.0) - 5.0 / 7.0).abs() < 1e-15);
        assert_eq!(effect(1.0E-6, 0.1, 0.0), None);
        assert_eq!(effect(100.0, 0.1, 0.999), Some(100f64.ln_1p() as f32));
        assert_eq!(effect(1.0, 0.1, 0.0), Some(1.0));
        assert_eq!(effect(0.001, 0.1, 0.9), None);
    }

    #[test]
    fn displays() {
        assert_eq!(display(100.0000001, true), "100.0 Sv/h");
        assert_eq!(display(100.0 / 9.0 + BASELINE, true), "11.111 Sv/h");
        assert_eq!(display(100.0 / 6400.0 + BASELINE, true), "15.625 mSv/h");
        assert_eq!(display(0.0, false), "0.0 Sv");
        assert_eq!(hours_minutes(decay_ticks(1000.0, 0.9975), 20.0), "2h 3m");
        assert_eq!(hours_minutes(1200, 20.0), "1m");
    }
}
