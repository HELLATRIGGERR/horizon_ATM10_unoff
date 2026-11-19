//! `EntityDataModel` and `BlockDataModel` as the Simulation Chamber and the Loot Fabricator use them, and
//! `DataModelInstance`: a model with the data one item holds.
use crate::tier::{Tier, Tiers};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

/// The prediction item every model's predictions are (`hostilenetworks:prediction`, holding the model's id).
pub const PREDICTION: &str = "hostilenetworks:prediction";

#[derive(Clone, Debug, PartialEq)]
pub struct Drop {
    pub item: String,
    pub count: u32,
    /// `"optional": true`: left out when the item isn't in the game (`OptionalStackCodec`).
    pub optional: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub id: String,
    /// The entity or block the model is of.
    pub target: String,
    pub block: bool,
    pub sim_cost: i32,
    /// The items a simulation takes (the prediction matrix).
    pub input: Vec<String>,
    pub base_drop: Drop,
    pub fab_drops: Vec<Drop>,
    pub required_data: HashMap<String, i32>,
    pub data_gained: HashMap<String, i32>,
}

pub type Models = BTreeMap<String, Model>;

fn drop_of(v: &Value) -> Option<Drop> {
    Some(Drop {
        item: v.get("id")?.as_str()?.to_owned(),
        count: v.get("count").and_then(Value::as_u64).map_or(1, |c| c as u32),
        optional: v.get("optional").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn overrides(v: &Value, key: &str, old: Option<&str>) -> HashMap<String, i32> {
    let found = v.get(key).or_else(|| old.and_then(|o| v.get(o)));
    found
        .and_then(Value::as_object)
        .map(|m| m.iter().filter_map(|(k, n)| Some((k.rsplit(':').next()?.to_owned(), i32::try_from(n.as_i64()?).ok()?))).collect())
        .unwrap_or_default()
}

impl Model {
    /// A data model's file (`data/<ns>/data_models/<path>.json`). `id` is `<ns>:<path>`.
    pub fn parse(id: &str, v: &Value) -> Result<Self, String> {
        let block = v.get("block").is_some();
        let target = v.get("entity").or_else(|| v.get("block"));
        let target = target.and_then(|t| t.as_str().or_else(|| t.get("id").and_then(Value::as_str)));
        let input = v.get("input").ok_or("input")?;
        let input: Vec<String> = match (input.get("item"), input.get("items")) {
            (Some(Value::String(s)), _) => vec![s.clone()],
            (_, Some(Value::Array(a))) => a.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect(),
            _ => return Err("input must name an item".into()),
        };
        Ok(Self {
            id: id.to_owned(),
            target: target.ok_or("entity")?.to_owned(),
            block,
            sim_cost: v.get("sim_cost").and_then(Value::as_i64).and_then(|c| i32::try_from(c).ok()).ok_or("sim_cost")?,
            input,
            base_drop: v.get("base_drop").and_then(drop_of).ok_or("base_drop")?,
            fab_drops: v.get("fabricator_drops").and_then(Value::as_array).ok_or("fabricator_drops")?.iter().filter_map(drop_of).collect(),
            required_data: overrides(v, "required_data", None),
            data_gained: overrides(v, "data_gained", Some("data_per_kill")),
        })
    }

    /// `DataModel.getRequiredData`: the model's own number for the tier, else the tier's.
    pub fn required_data(&self, tier: &Tier) -> i32 {
        self.required_data.get(&tier.name).copied().unwrap_or(tier.required_data)
    }

    /// `DataModel.getDataGained`.
    pub fn data_gained(&self, tier: &Tier) -> i32 {
        self.data_gained.get(&tier.name).copied().unwrap_or(tier.data_gained)
    }

    pub fn input_matches(&self, item: &str) -> bool {
        self.input.iter().any(|i| i == item)
    }
}

/// `DataModelInstance`: a model and the data of the item it is read from.
#[derive(Clone, Copy, Debug)]
pub struct Instance<'a> {
    pub model: &'a Model,
    pub tiers: &'a Tiers,
    pub continuous_accuracy: bool,
    pub data: i32,
}

impl<'a> Instance<'a> {
    pub fn new(model: &'a Model, tiers: &'a Tiers, continuous_accuracy: bool, data: i32) -> Self {
        Self { model, tiers, continuous_accuracy, data }
    }

    /// `ModelTierRegistry.getByData`: the highest tier the data reaches for this model.
    pub fn tier_index(&self) -> usize {
        (0..self.tiers.len()).rev().find(|&i| self.data >= self.model.required_data(&self.tiers.all()[i])).unwrap_or(0)
    }

    pub fn tier(&self) -> &'a Tier {
        &self.tiers.all()[self.tier_index()]
    }

    pub fn is_max(&self) -> bool {
        self.tiers.is_max(self.tier_index())
    }

    pub fn tier_data(&self) -> i32 {
        self.model.required_data(self.tier())
    }

    pub fn next_tier(&self) -> &'a Tier {
        &self.tiers.all()[self.tiers.next(self.tier_index())]
    }

    pub fn next_tier_data(&self) -> i32 {
        self.model.required_data(self.next_tier())
    }

    /// `getAccuracy`: the tier's, or with continuous accuracy a line from it to the next tier's.
    pub fn accuracy(&self) -> f32 {
        if !self.continuous_accuracy || self.is_max() {
            return self.tier().accuracy;
        }
        let diff = self.next_tier_data() - self.tier_data();
        let t_diff = self.next_tier().accuracy - self.tier().accuracy;
        self.tier().accuracy + t_diff * (diff - (self.next_tier_data() - self.data)) as f32 / diff as f32
    }

    /// `rollPredictions`: each 100% is a guaranteed prediction, the rest a chance of one. `roll` is in `[0, 1)`.
    pub fn roll_predictions(&self, roll: f32) -> i32 {
        let accuracy = self.accuracy();
        let floor = accuracy as i32;
        floor + i32::from(roll < accuracy - floor as f32)
    }

    pub fn data_gained(&self, action_upgrades_model: bool) -> i32 {
        if action_upgrades_model { self.model.data_gained(self.tier()) } else { 0 }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn zombie() -> Model {
        let v: Value = serde_json::from_str(
            r##"{"entity":"minecraft:zombie","name_color":"#3B622F","sim_cost":128,"input":{"item":"hostilenetworks:prediction_matrix"},
            "base_drop":{"id":"hostilenetworks:overworld_prediction"},"trivia":"x",
            "fabricator_drops":[{"id":"minecraft:rotten_flesh","count":64},{"id":"reliquary:zombie_heart","optional":true,"count":2}]}"##,
        )
        .unwrap();
        Model::parse("hostilenetworks:zombie", &v).unwrap()
    }

    #[test]
    fn a_model_file_reads() {
        let m = zombie();
        assert_eq!((m.sim_cost, m.fab_drops.len(), m.fab_drops[1].optional, m.fab_drops[0].count), (128, 2, true, 64));
        assert!(m.input_matches("hostilenetworks:prediction_matrix") && !m.input_matches("minecraft:stick"));
        assert_eq!(m.base_drop.count, 1);
    }

    #[test]
    fn data_picks_the_tier() {
        let (m, t) = (zombie(), Tiers::defaults());
        let tier = |d| Instance::new(&m, &t, true, d).tier().name.clone();
        assert_eq!(
            [tier(0), tier(5), tier(6), tier(53), tier(54), tier(353), tier(354), tier(1253), tier(1254), tier(9999)],
            ["faulty", "faulty", "basic", "basic", "advanced", "advanced", "superior", "superior", "self_aware", "self_aware"]
        );
    }

    #[test]
    fn a_model_overrides_the_tiers_data() {
        let mut m = zombie();
        m.required_data.insert("basic".into(), 10);
        let t = Tiers::defaults();
        assert_eq!(Instance::new(&m, &t, true, 9).tier().name, "faulty");
        assert_eq!(Instance::new(&m, &t, true, 10).tier().name, "basic");
        assert_eq!(Instance::new(&m, &t, true, 10).tier_data(), 10);
    }

    #[test]
    fn accuracy_climbs_between_tiers() {
        let (m, t) = (zombie(), Tiers::defaults());
        // Basic (6, 0.05) to advanced (54, 0.22): halfway is 30 data.
        let half = Instance::new(&m, &t, true, 30).accuracy();
        assert!((half - 0.135).abs() < 1e-6, "{half}");
        assert_eq!(Instance::new(&m, &t, false, 30).accuracy(), 0.05);
        assert_eq!(Instance::new(&m, &t, true, 6).accuracy(), 0.05);
        assert_eq!(Instance::new(&m, &t, true, 1254).accuracy(), 1.0);
    }

    #[test]
    fn predictions_roll_against_the_fraction() {
        let (m, t) = (zombie(), Tiers::defaults());
        let basic = Instance::new(&m, &t, false, 6);
        assert_eq!((basic.roll_predictions(0.04), basic.roll_predictions(0.05), basic.roll_predictions(0.9)), (1, 0, 0));
        // Self-aware is a sure one, and an accuracy above 100% adds a chance of another.
        assert_eq!(Instance::new(&m, &t, false, 5000).roll_predictions(0.99), 1);
        let t2 = Tiers::new(vec![Tier { name: "a".into(), required_data: 0, data_gained: 1, accuracy: 2.5, can_sim: true }]).unwrap();
        let i = Instance::new(&m, &t2, false, 0);
        assert_eq!((i.roll_predictions(0.4), i.roll_predictions(0.6)), (3, 2));
    }

    #[test]
    fn tiers_must_be_ordered() {
        let t = |n: &str, r| Tier { name: n.into(), required_data: r, data_gained: 1, accuracy: 0.0, can_sim: true };
        assert!(Tiers::new(vec![t("a", 1)]).is_err());
        assert!(Tiers::new(vec![t("a", 0), t("b", 0)]).is_err());
        assert_eq!(Tiers::defaults().all().len(), 5);
    }
}
