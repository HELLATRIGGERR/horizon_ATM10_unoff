//! The goo's spreading recipes (`GooSpreadRecipe`, `GooSpreadRecipeTag`) as the data writes them:
//! `{"type": "justdirethings:goospread", "input": {"Name": .., "Properties": {..}}, "output": {..},
//! "tierRequirement": 1, "craftingDuration": 2400}` (`BlockState.CODEC` both sides), and `goospread_tag`, whose input is a
//! block tag (`BlockTagIngredient`, `{"tag": "c:storage_blocks/charcoal"}`).
use serde_json::Value;

/// The recipe type of a block state's conversion.
pub const SPREAD: &str = "justdirethings:goospread";
/// The recipe type of a block tag's conversion.
pub const SPREAD_TAG: &str = "justdirethings:goospread_tag";

/// A block state as `BlockState.CODEC` writes it: the block and the properties the file names (the others keep
/// the block's defaults).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StateSpec {
    pub name: String,
    pub properties: Vec<(String, String)>,
}

impl StateSpec {
    pub fn block(name: &str) -> Self {
        Self { name: name.to_owned(), properties: Vec::new() }
    }

    /// `{"Name": "minecraft:iron_block", "Properties": {"facing": "north"}}`.
    pub fn parse(v: &Value) -> Result<Self, String> {
        let name = v.get("Name").and_then(Value::as_str).ok_or("a block state without its Name")?;
        let mut properties = Vec::new();
        if let Some(map) = v.get("Properties") {
            let map = map.as_object().ok_or("Properties is not an object")?;
            for (key, value) in map {
                let value = match value {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                properties.push((key.clone(), value));
            }
        }
        Ok(Self { name: name.to_owned(), properties })
    }

    pub fn property(&self, key: &str) -> Option<&str> {
        self.properties.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    /// This state with `key` set to `value` (`BlockState.setValue`).
    pub fn with(&self, key: &str, value: &str) -> Self {
        let mut s = self.clone();
        match s.properties.iter_mut().find(|(k, _)| k == key) {
            Some((_, v)) => *v = value.to_owned(),
            None => s.properties.push((key.to_owned(), value.to_owned())),
        }
        s
    }

    /// As `/setblock` takes it: `justdirethings:raw_ferricore_ore[facing=up]`.
    pub fn command(&self) -> String {
        if self.properties.is_empty() {
            return self.name.clone();
        }
        let props: Vec<String> = self.properties.iter().map(|(k, v)| format!("{k}={v}")).collect();
        format!("{}[{}]", self.name, props.join(","))
    }
}

/// What a recipe converts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input {
    /// One block state (`sourceState.equals(input)`).
    State(StateSpec),
    /// Any state of a block in the tag (`sourceState.is(tag)`).
    Tag(String),
}

/// One spreading recipe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpreadRecipe {
    pub id: String,
    pub input: Input,
    pub output: StateSpec,
    /// The lowest goo tier that converts it.
    pub tier: i32,
    /// Ticks of a tier 1 goo (a higher tier counts down faster).
    pub duration: i32,
}

impl SpreadRecipe {
    /// A recipe file of type `kind` (`SPREAD` or `SPREAD_TAG`); `id` names it in errors and when the file has none.
    pub fn parse(kind: &str, id: &str, v: &Value) -> Result<Self, String> {
        let input = match kind {
            SPREAD => Input::State(StateSpec::parse(&v["input"]).map_err(|e| format!("{id}: input: {e}"))?),
            SPREAD_TAG => {
                let tag = v["input"]["tag"].as_str().ok_or_else(|| format!("{id}: input without a tag"))?;
                Input::Tag(tag.trim_start_matches('#').to_owned())
            }
            other => return Err(format!("{id}: not a spreading recipe ({other})")),
        };
        let output = StateSpec::parse(&v["output"]).map_err(|e| format!("{id}: output: {e}"))?;
        let number = |key: &str| {
            v[key].as_i64().and_then(|n| i32::try_from(n).ok()).ok_or_else(|| format!("{id}: no {key}"))
        };
        Ok(Self {
            id: v["id"].as_str().unwrap_or(id).to_owned(),
            input,
            output,
            tier: number("tierRequirement")?,
            duration: number("craftingDuration")?,
        })
    }
}

/// `GooBlockBE_Base.populateCaches`: the first recipe of the block-state type whose input is the state, else the first
/// of the tag type whose tag holds it, each only for a goo of at least the recipe's tier. `recipes` holds both
/// types; `is_state` says whether the block there is in a recipe's state, `in_tag` whether it is in a tag.
pub fn find<'a>(
    recipes: &'a [SpreadRecipe],
    tier: i32,
    is_state: impl Fn(&StateSpec) -> bool,
    in_tag: impl Fn(&str) -> bool,
) -> Option<&'a SpreadRecipe> {
    let plain = recipes.iter().find(|r| matches!(&r.input, Input::State(s) if is_state(s)) && tier >= r.tier);
    plain.or_else(|| recipes.iter().find(|r| matches!(&r.input, Input::Tag(t) if in_tag(t)) && tier >= r.tier))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn iron() -> SpreadRecipe {
        let v = json!({"craftingDuration": 2400, "id": "justdirethings:dire_iron_block", "input": {"Name": "minecraft:iron_block"},
            "output": {"Name": "justdirethings:raw_ferricore_ore", "Properties": {"facing": "north"}}, "tierRequirement": 1,
            "type": "justdirethings:goospread"});
        SpreadRecipe::parse(SPREAD, "justdirethings:raw_ferricore_ore-goospread", &v).unwrap()
    }

    #[test]
    fn a_state_recipe_reads_as_the_mod_writes_it() {
        let r = iron();
        assert_eq!(r.id, "justdirethings:dire_iron_block");
        assert_eq!(r.input, Input::State(StateSpec::block("minecraft:iron_block")));
        assert_eq!(r.output.command(), "justdirethings:raw_ferricore_ore[facing=north]");
        assert_eq!((r.tier, r.duration), (1, 2400));
        assert_eq!(r.output.with("facing", "up").command(), "justdirethings:raw_ferricore_ore[facing=up]");
    }

    #[test]
    fn a_tag_recipe_reads_its_tag() {
        let v = json!({"craftingDuration": 2400, "id": "justdirethings:coal_block_t1", "input": {"tag": "c:storage_blocks/charcoal"},
            "output": {"Name": "justdirethings:raw_coal_t1_ore", "Properties": {"facing": "north"}}, "tierRequirement": 1});
        let r = SpreadRecipe::parse(SPREAD_TAG, "x", &v).unwrap();
        assert_eq!(r.input, Input::Tag("c:storage_blocks/charcoal".into()));
        assert!(SpreadRecipe::parse(SPREAD, "x", &json!({"input": {}})).is_err());
    }

    #[test]
    fn the_state_type_wins_and_the_tier_must_reach() {
        let portal = SpreadRecipe {
            id: "portal".into(),
            input: Input::State(StateSpec::block("justdirethings:unstable_portal_fluid_block").with("level", "0")),
            output: StateSpec::block("justdirethings:portal_fluid_block"),
            tier: 3,
            duration: 2400,
        };
        let tagged = SpreadRecipe {
            id: "tagged".into(),
            input: Input::Tag("c:storage_blocks/iron".into()),
            output: StateSpec::block("minecraft:stone"),
            tier: 1,
            duration: 10,
        };
        let recipes = vec![tagged.clone(), iron(), portal.clone()];
        fn is(name: &str) -> impl Fn(&StateSpec) -> bool + '_ {
            move |s: &StateSpec| s.name == name
        }
        let iron_tags = |t: &str| t == "c:storage_blocks/iron";
        assert_eq!(find(&recipes, 1, is("minecraft:iron_block"), iron_tags).map(|r| r.id.as_str()), Some("justdirethings:dire_iron_block"));
        assert_eq!(find(&recipes, 1, is("minecraft:gold_block"), iron_tags).map(|r| r.id.as_str()), Some("tagged"));
        assert_eq!(find(&recipes, 2, is("justdirethings:unstable_portal_fluid_block"), |_| false), None, "tier 3 needs a tier 3 goo");
        assert_eq!(find(&recipes, 4, is("justdirethings:unstable_portal_fluid_block"), |_| false), Some(&portal));
        assert_eq!(find(&recipes, 4, is("minecraft:dirt"), |_| false), None);
    }
}
