//! `ModelTier` and `ModelTierRegistry`: the tiers sorted by the data they need, and which one a model's data is.
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Tier {
    /// The file's name: `faulty`, `basic`, `advanced`, `superior`, `self_aware`.
    pub name: String,
    pub required_data: i32,
    pub data_gained: i32,
    /// 0.0 to 64.0; 1.0 is a guaranteed prediction.
    pub accuracy: f32,
    pub can_sim: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tiers {
    sorted: Vec<Tier>,
}

fn int(v: &Value, key: &str) -> Option<i32> {
    i32::try_from(v.get(key)?.as_i64()?).ok().filter(|n| *n >= 0)
}

impl Tier {
    /// `ModelTier.CODEC`: `data_gained` is `data_per_kill` in the old files.
    pub fn parse(name: &str, v: &Value) -> Result<Self, String> {
        let accuracy = v.get("accuracy").and_then(Value::as_f64).filter(|a| (0.0..=64.0).contains(a));
        Ok(Self {
            name: name.to_owned(),
            required_data: int(v, "required_data").ok_or("required_data")?,
            data_gained: int(v, "data_gained").or_else(|| int(v, "data_per_kill")).ok_or("data_gained")?,
            accuracy: accuracy.ok_or("accuracy")? as f32,
            can_sim: v.get("can_sim").and_then(Value::as_bool).unwrap_or(true),
        })
    }
}

impl Tiers {
    /// `ModelTierRegistry.onReload`: sorted by required data; the lowest needs none, the data strictly rises and one
    /// tier can simulate.
    pub fn new(mut tiers: Vec<Tier>) -> Result<Self, String> {
        tiers.sort_by_key(|t| t.required_data);
        match tiers.first() {
            None => return Err("no model tiers".into()),
            Some(min) if min.required_data != 0 => return Err(format!("the lowest model tier ({}) must need 0 data", min.name)),
            _ => {}
        }
        if tiers.windows(2).any(|w| w[0].required_data >= w[1].required_data) {
            return Err("model tiers must have rising required data".into());
        }
        if !tiers.iter().any(|t| t.can_sim) {
            return Err("at least one model tier must be able to simulate".into());
        }
        Ok(Self { sorted: tiers })
    }

    /// The mod's own five tiers (`data/hostilenetworks/model_tiers`, 6.5.1).
    pub fn defaults() -> Self {
        let t = |name: &str, required_data, data_gained, accuracy, can_sim| Tier { name: name.into(), required_data, data_gained, accuracy, can_sim };
        Self::new(vec![
            t("faulty", 0, 1, 0.0, false),
            t("basic", 6, 4, 0.05, true),
            t("advanced", 54, 10, 0.22, true),
            t("superior", 354, 18, 0.65, true),
            t("self_aware", 1254, 0, 1.0, true),
        ])
        .expect("the mod's tiers are valid")
    }

    pub fn all(&self) -> &[Tier] {
        &self.sorted
    }

    pub fn len(&self) -> usize {
        self.sorted.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sorted.is_empty()
    }

    pub fn is_max(&self, tier: usize) -> bool {
        tier + 1 >= self.sorted.len()
    }

    /// `ModelTierRegistry.next`: the next tier, or the same one at the top.
    pub fn next(&self, tier: usize) -> usize {
        if self.is_max(tier) { tier } else { tier + 1 }
    }

    pub fn position(&self, name: &str) -> Option<usize> {
        self.sorted.iter().position(|t| t.name == name)
    }
}
