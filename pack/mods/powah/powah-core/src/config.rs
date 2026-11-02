//! `config/powah.json5`, the numbers Powah plays by. All the Mods 10 ships it
//! with the mod's defaults, which are the fallbacks here. Only what the ported
//! blocks read is kept; the file is JSON5 (comments, trailing commas), which
//! `json5_to_json` turns into JSON for `serde_json`.
use crate::tier::Tier;
use serde_json::Value;
use std::path::Path;

/// One generator family's numbers by tier, in FE and FE/t.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratorTable {
    pub capacities: [i64; 7],
    pub transfer_rates: [i64; 7],
    pub generation_rates: [i64; 7],
}

/// What one tier of a generator runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratorSpec {
    pub capacity: i64,
    pub transfer: i64,
    pub generation: i64,
}

impl GeneratorTable {
    pub fn spec(&self, tier: Tier) -> GeneratorSpec {
        let i = tier.index();
        GeneratorSpec {
            capacity: self.capacities[i],
            transfer: self.transfer_rates[i],
            generation: self.generation_rates[i],
        }
    }
}

/// The settings the ported blocks read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PowahConfig {
    /// `general.energy_per_fuel_tick`: FE one tick of furnace fuel is worth.
    pub energy_per_fuel_tick: i64,
    /// `generators.furnators`.
    pub furnators: GeneratorTable,
}

impl Default for PowahConfig {
    fn default() -> Self {
        PowahConfig {
            energy_per_fuel_tick: 30,
            furnators: GeneratorTable {
                capacities: [20_000, 80_000, 200_000, 800_000, 2_000_000, 8_000_000, 40_000_000],
                transfer_rates: [240, 480, 1_600, 4_000, 8_000, 32_000, 160_000],
                generation_rates: [80, 160, 400, 1_000, 2_000, 8_000, 20_000],
            },
        }
    }
}

impl PowahConfig {
    /// The settings in `<config dir>/powah.json5`; the defaults without the file,
    /// and for a value it doesn't hold or holds badly.
    pub fn load(config_dir: &Path) -> PowahConfig {
        match std::fs::read_to_string(config_dir.join("powah.json5")) {
            Ok(text) => PowahConfig::parse(&text),
            Err(_) => PowahConfig::default(),
        }
    }

    pub fn parse(text: &str) -> PowahConfig {
        let mut config = PowahConfig::default();
        let Ok(root) = serde_json::from_str::<Value>(&json5_to_json(text)) else {
            return config;
        };
        if let Some(n) = positive(&root["general"]["energy_per_fuel_tick"]) {
            config.energy_per_fuel_tick = n;
        }
        let table = &root["generators"]["furnators"];
        for tier in Tier::ALL {
            let i = tier.index();
            let t = tier.name();
            if let Some(n) = positive(&table["capacities"][t]) {
                config.furnators.capacities[i] = n;
            }
            if let Some(n) = positive(&table["transfer_rates"][t]) {
                config.furnators.transfer_rates[i] = n;
            }
            if let Some(n) = positive(&table["generation_rates"][t]) {
                config.furnators.generation_rates[i] = n;
            }
        }
        config
    }
}

fn positive(value: &Value) -> Option<i64> {
    value.as_i64().filter(|n| *n > 0)
}

/// Skips a comment starting at `i`, if one does; where the text goes on.
fn skip_comment(chars: &[char], mut i: usize) -> Option<usize> {
    if chars.get(i) != Some(&'/') {
        return None;
    }
    match chars.get(i + 1) {
        Some('/') => {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            Some(i)
        }
        Some('*') => {
            i += 2;
            while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                i += 1;
            }
            Some(i + 2)
        }
        _ => None,
    }
}

/// JSON5 as JSON: comments go, single-quoted strings get double quotes, bare
/// keys get quotes and trailing commas go. Numbers stay as they are written.
pub fn json5_to_json(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if let Some(next) = skip_comment(&chars, i) {
            i = next;
            continue;
        }
        let c = chars[i];
        match c {
            '"' | '\'' => {
                out.push('"');
                i += 1;
                while i < chars.len() && chars[i] != c {
                    match chars[i] {
                        '\\' if chars.get(i + 1) == Some(&'\'') => {
                            out.push('\'');
                            i += 2;
                        }
                        '\\' => {
                            out.push('\\');
                            if let Some(next) = chars.get(i + 1) {
                                out.push(*next);
                            }
                            i += 2;
                        }
                        '"' => {
                            out.push_str("\\\"");
                            i += 1;
                        }
                        other => {
                            out.push(other);
                            i += 1;
                        }
                    }
                }
                out.push('"');
                i += 1;
            }
            ',' => {
                let mut j = i + 1;
                loop {
                    while chars.get(j).is_some_and(|c| c.is_whitespace()) {
                        j += 1;
                    }
                    match skip_comment(&chars, j) {
                        Some(next) => j = next,
                        None => break,
                    }
                }
                if !matches!(chars.get(j), Some('}' | ']')) {
                    out.push(',');
                }
                i += 1;
            }
            c if c.is_alphabetic() || c == '_' || c == '$' => {
                let start = i;
                while chars.get(i).is_some_and(|c| c.is_alphanumeric() || *c == '_' || *c == '$') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                let mut j = i;
                while chars.get(j).is_some_and(|c| c.is_whitespace()) {
                    j += 1;
                }
                if chars.get(j) == Some(&':') {
                    out.push('"');
                    out.push_str(&word);
                    out.push('"');
                } else {
                    out.push_str(&word);
                }
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALL: &str = r#"{
        // A line comment with a "quote", and 'another'.
        "general": { /* a block comment, here too */
            "energy_per_fuel_tick": 40,
            bare_key: 'single, quoted',
        },
        "generators": {
            "furnators": {
                "capacities": { "starter": 1000, "nitro": 5000000000, },
                "transfer_rates": { "basic": 12 },
                /* Spans
                   lines. */
                "generation_rates": { "nitro": 33, "starter": -1, "basic": "many" },
            },
        },
    }"#;

    #[test]
    fn json5_becomes_json() {
        assert_eq!(json5_to_json("{a: 1, // no\n 'b': 'it\\'s',}").replace([' ', '\n'], ""), r#"{"a":1,"b":"it's"}"#);
        assert_eq!(json5_to_json("[1, 2, /* x */ ]").replace(' ', ""), "[1,2]");
        assert_eq!(json5_to_json(r#"{"url": "http://x", "s": "a, }"}"#), r#"{"url": "http://x", "s": "a, }"}"#);
        assert_eq!(json5_to_json("{v: true, n: null}"), r#"{"v": true, "n": null}"#);
    }

    #[test]
    fn a_small_file_overrides_what_it_holds() {
        let config = PowahConfig::parse(SMALL);
        assert_eq!(config.energy_per_fuel_tick, 40);
        assert_eq!(config.furnators.spec(Tier::Starter).capacity, 1000);
        assert_eq!(config.furnators.spec(Tier::Nitro).capacity, 5_000_000_000);
        assert_eq!(config.furnators.spec(Tier::Basic).transfer, 12);
        assert_eq!(config.furnators.spec(Tier::Nitro).generation, 33);
        // Bad and missing values keep the mod's defaults.
        assert_eq!(config.furnators.spec(Tier::Starter).generation, 80);
        assert_eq!(config.furnators.spec(Tier::Basic).generation, 160);
        assert_eq!(config.furnators.spec(Tier::Hardened).capacity, 200_000);
    }

    #[test]
    fn a_broken_file_is_the_defaults() {
        assert_eq!(PowahConfig::parse("not json at all {"), PowahConfig::default());
        assert_eq!(PowahConfig::load(Path::new("no/such/folder")), PowahConfig::default());
    }

    #[test]
    fn the_defaults_are_atm10s_numbers() {
        let config = PowahConfig::default();
        assert_eq!(config.energy_per_fuel_tick, 30);
        let table: Vec<_> = Tier::ALL.iter().map(|t| config.furnators.spec(*t)).collect();
        let row = |f: fn(&GeneratorSpec) -> i64| table.iter().map(f).collect::<Vec<_>>();
        assert_eq!(row(|s| s.capacity), [20_000, 80_000, 200_000, 800_000, 2_000_000, 8_000_000, 40_000_000]);
        assert_eq!(row(|s| s.transfer), [240, 480, 1_600, 4_000, 8_000, 32_000, 160_000]);
        assert_eq!(row(|s| s.generation), [80, 160, 400, 1_000, 2_000, 8_000, 20_000]);
    }
}
