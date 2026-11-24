//! The mercurial reformation array and the mercury that runs it: `ReformationRecipe` (source pedestals, a target
//! pedestal, a result, a mercury flux cost), `CatalysationRecipe` (an item the mercury catalyst turns into flux) and
//! the flux storages (`DefaultMercuryFluxStorage`, a copy of NeoForge's `EnergyStorage`).
use crate::recipe::{field, ItemResult, DEFAULT_TIME};
use crate::{Ingredient, SizedIngredient, Tags};
use serde_json::Value;

pub const REFORMATION: &str = "theurgy:reformation";
pub const CATALYSATION: &str = "theurgy:catalysation";

/// `SulfuricFluxEmitterBlockEntity.CAPACITY`.
pub const EMITTER_CAPACITY: i32 = 1000;
/// `MercuryCatalystBlockEntity.CAPACITY`.
pub const CATALYST_CAPACITY: i32 = 50000;
/// `MercuryCatalystBlockEntity.PUSH_TICK_INTERVAL` and `PUSH_RATE_PER_TICK`: every 20 ticks it offers 40.
pub const PUSH_INTERVAL: i64 = 20;
pub const PUSH_AMOUNT: i32 = 2 * 20;
/// `SulfuricFluxEmitterSelectionBehaviour.getBlockRange`.
pub const SELECTION_RANGE: i32 = 16;

/// A stack in a pedestal: its item and count (`None` for an empty pedestal).
pub type Stack<'a> = Option<(&'a str, u32)>;

/// `ReformationRecipe`: sized sources spread over the source pedestals, a target that stays, a result for the
/// result pedestal, and the flux the emitter spends.
#[derive(Clone, Debug, PartialEq)]
pub struct Reformation {
    pub id: String,
    pub sources: Vec<SizedIngredient>,
    pub target: Ingredient,
    pub result: ItemResult,
    pub mercury_flux: i32,
    pub time: i32,
}

impl Reformation {
    pub fn parse(id: &str, json: &Value) -> Result<Self, String> {
        let wrap = |e: String| format!("{id}: {e}");
        let sources = field(json, "sources")
            .map_err(wrap)?
            .as_array()
            .ok_or_else(|| wrap("sources is not a list".into()))?
            .iter()
            .map(SizedIngredient::parse)
            .collect::<Result<Vec<_>, _>>()
            .map_err(wrap)?;
        let target = Ingredient::parse(field(json, "target").map_err(wrap)?).map_err(wrap)?;
        // `ItemStack.STRICT_CODEC`, written by the datagen with a `type` of its own.
        let mut result = field(json, "result").map_err(wrap)?.clone();
        if let Some(o) = result.as_object_mut() {
            o.remove("type");
        }
        let result = ItemResult::parse(&result).map_err(wrap)?;
        let mercury_flux = field(json, "mercuryFlux").map_err(wrap)?.as_i64().and_then(|f| i32::try_from(f).ok()).ok_or_else(|| wrap("mercuryFlux".into()))?;
        let time = json.get("time").and_then(Value::as_i64).and_then(|t| i32::try_from(t).ok()).unwrap_or(DEFAULT_TIME);
        Ok(Self { id: id.to_owned(), sources, target, result, mercury_flux, time })
    }

    /// `matches`: enough flux, the target pedestal's item fits, and every source is met by a pedestal of its own
    /// (one source may need several pedestals' worth only through its count, never by adding stacks up).
    pub fn matches(&self, target: Option<&str>, sources: &[Stack<'_>], flux: i32, tags: Tags<'_>) -> bool {
        if flux < self.mercury_flux {
            return false;
        }
        if !target.is_some_and(|t| self.target.test(t, tags)) {
            return false;
        }
        let mut used = vec![false; sources.len()];
        for source in &self.sources {
            let Some(at) = sources.iter().enumerate().position(|(i, s)| !used[i] && s.is_some_and(|(item, count)| source.test(item, count, tags))) else {
                return false;
            };
            used[at] = true;
        }
        true
    }

    /// `craft`: for each source in the recipe's order, the first pedestal not yet used that fits it gives its count.
    /// (pedestal index, count taken).
    pub fn consumption(&self, sources: &[Stack<'_>], tags: Tags<'_>) -> Vec<(usize, u32)> {
        let mut used = vec![false; sources.len()];
        let mut taken = Vec::new();
        for source in &self.sources {
            if let Some(at) = sources.iter().enumerate().position(|(i, s)| !used[i] && s.is_some_and(|(item, count)| source.test(item, count, tags))) {
                used[at] = true;
                taken.push((at, source.count));
            }
        }
        taken
    }
}

/// `CatalysationRecipe`: an item the mercury catalyst consumes, and the flux it turns into.
#[derive(Clone, Debug, PartialEq)]
pub struct Catalysation {
    pub id: String,
    pub ingredient: Ingredient,
    pub total_flux: i32,
    pub flux_per_tick: i32,
}

impl Catalysation {
    pub fn parse(id: &str, json: &Value) -> Result<Self, String> {
        let wrap = |e: String| format!("{id}: {e}");
        let int = |name: &str| field(json, name).map_err(wrap).and_then(|v| v.as_i64().and_then(|n| i32::try_from(n).ok()).ok_or_else(|| wrap(name.to_owned())));
        Ok(Self {
            id: id.to_owned(),
            ingredient: Ingredient::parse(field(json, "ingredient").map_err(wrap)?).map_err(wrap)?,
            total_flux: int("totalMercuryFlux")?,
            flux_per_tick: int("mercuryFluxPerTick")?,
        })
    }

    /// `matches` and `isIngredient`: the item fits.
    pub fn takes(&self, item: &str, tags: Tags<'_>) -> bool {
        self.ingredient.test(item, tags)
    }
}

/// `DefaultMercuryFluxStorage.receiveEnergy`: what a storage of `capacity` holding `stored` takes of `offered`.
pub fn receive(stored: i32, capacity: i32, offered: i32) -> i32 {
    (capacity - stored).min(capacity.min(offered)).max(0)
}

/// `DefaultMercuryFluxStorage.extractEnergy`: what a storage holding `stored` gives of `asked`.
pub fn extract(stored: i32, capacity: i32, asked: i32) -> i32 {
    stored.min(capacity.min(asked)).max(0)
}

/// What the catalyst keeps beside its stored flux: the flux still to convert and how fast
/// (`MercuryCatalystCraftingBehaviour`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Catalyst {
    pub stored: i32,
    pub to_convert: i32,
    pub per_tick: i32,
}

impl Catalyst {
    /// `tickServer(true, hasInput)`: leftover flux converts first; with none left and an item the recipe takes, and
    /// room for some of its flux, the item is consumed (true) and its whole flux is queued.
    pub fn tick(&mut self, has_input: bool, recipe: impl FnOnce() -> Option<(i32, i32)>) -> bool {
        if self.to_convert > 0 {
            let max = self.to_convert.min(self.per_tick);
            let accepted = receive(self.stored, CATALYST_CAPACITY, max);
            self.stored += accepted;
            self.to_convert -= accepted;
            false
        } else if has_input && let Some((total, per_tick)) = recipe() && receive(self.stored, CATALYST_CAPACITY, total) > 0 {
            self.to_convert = total;
            self.per_tick = per_tick;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tags(_: &str, _: &str) -> bool {
        false
    }

    fn two_crops() -> Reformation {
        Reformation::parse(
            "theurgy:reformation/test",
            &json!({"type": "theurgy:reformation", "category": "misc", "mercuryFlux": 50,
                "result": {"type": "theurgy:item", "count": 1, "id": "theurgy:animals"},
                "sources": [{"count": 1, "ingredient": {"item": "theurgy:crops"}}, {"count": 1, "ingredient": {"item": "theurgy:crops"}}],
                "target": {"item": "theurgy:animals"}, "time": 100}),
        )
        .unwrap()
    }

    #[test]
    fn a_recipe_reads_and_matches_with_a_pedestal_for_each_source() {
        let r = two_crops();
        assert_eq!((r.mercury_flux, r.time, r.result.id.as_str(), r.result.count), (50, 100, "theurgy:animals", 1));
        assert!(r.result.json.get("type").is_none());
        let both = [Some(("theurgy:crops", 1)), Some(("theurgy:crops", 1))];
        assert!(r.matches(Some("theurgy:animals"), &both, 50, &tags));
        assert!(!r.matches(Some("theurgy:animals"), &both, 49, &tags), "short of flux");
        assert!(!r.matches(Some("theurgy:crops"), &both, 50, &tags), "another target");
        assert!(!r.matches(None, &both, 50, &tags), "no target");
        assert!(!r.matches(Some("theurgy:animals"), &[Some(("theurgy:crops", 2))], 50, &tags), "a pedestal is used once");
        assert!(!r.matches(Some("theurgy:animals"), &[Some(("theurgy:crops", 1)), None], 50, &tags));
        assert!(r.matches(Some("theurgy:animals"), &[None, Some(("theurgy:crops", 3)), Some(("theurgy:crops", 1))], 50, &tags), "empty pedestals are skipped");
        assert_eq!(r.consumption(&[None, Some(("theurgy:crops", 3)), Some(("theurgy:crops", 1))], &tags), vec![(1, 1), (2, 1)]);
    }

    #[test]
    fn a_source_with_a_count_needs_that_many_on_one_pedestal() {
        let r = Reformation::parse(
            "t",
            &json!({"mercuryFlux": 10, "result": {"id": "a:b"}, "target": {"item": "a:t"}, "sources": [{"count": 3, "ingredient": {"item": "a:s"}}]}),
        )
        .unwrap();
        assert_eq!(r.time, DEFAULT_TIME);
        assert!(!r.matches(Some("a:t"), &[Some(("a:s", 2)), Some(("a:s", 2))], 10, &tags));
        assert!(r.matches(Some("a:t"), &[Some(("a:s", 2)), Some(("a:s", 5))], 10, &tags));
        assert_eq!(r.consumption(&[Some(("a:s", 2)), Some(("a:s", 5))], &tags), vec![(1, 3)]);
    }

    #[test]
    fn storages_take_and_give_within_capacity() {
        assert_eq!(receive(990, 1000, 40), 10);
        assert_eq!(receive(0, 1000, 5000), 1000);
        assert_eq!(extract(30, 1000, 50), 30);
        assert_eq!(extract(500, 1000, 40), 40);
    }

    #[test]
    fn the_catalyst_converts_an_item_over_ticks() {
        let c = Catalysation::parse("t", &json!({"ingredient": {"item": "theurgy:mercury_shard"}, "mercuryFluxPerTick": 20, "totalMercuryFlux": 800})).unwrap();
        assert!(c.takes("theurgy:mercury_shard", &tags) && !c.takes("minecraft:dirt", &tags));
        let mut k = Catalyst::default();
        assert!(!k.tick(false, || unreachable!()));
        assert!(k.tick(true, || Some((c.total_flux, c.flux_per_tick))), "the item is consumed");
        assert_eq!((k.stored, k.to_convert), (0, 800));
        for n in 1..=40 {
            assert!(!k.tick(true, || unreachable!("no lookup while converting")));
            assert_eq!(k.stored, 20 * n);
        }
        assert_eq!((k.stored, k.to_convert), (800, 0));
        let mut full = Catalyst { stored: CATALYST_CAPACITY, ..Catalyst::default() };
        assert!(!full.tick(true, || Some((800, 20))), "no room: not consumed");
    }
}
