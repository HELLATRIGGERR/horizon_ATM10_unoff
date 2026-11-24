//! Ingredients as Theurgy's recipes write them (1.21.1 JSON): vanilla's `Ingredient` (an item, a tag, or a list of
//! them), NeoForge's `SizedIngredient` (`{"ingredient": .., "count": n}`) and `SizedFluidIngredient`
//! (`{"ingredient": {"fluid"|"tag": ..}, "amount": n}`).
use crate::Tags;
use serde_json::Value;

/// An item ingredient (`Ingredient.CODEC`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ingredient {
    Item(String),
    Tag(String),
    /// A JSON list, or NeoForge's `neoforge:compound`: any member.
    Any(Vec<Ingredient>),
    /// A custom ingredient type this port doesn't read: it matches nothing.
    Unsupported(String),
}

impl Ingredient {
    pub fn parse(json: &Value) -> Result<Self, String> {
        match json {
            Value::Array(list) => Ok(Self::Any(list.iter().map(Self::parse).collect::<Result<_, _>>()?)),
            Value::String(s) => Ok(match s.strip_prefix('#') {
                Some(tag) => Self::Tag(tag.to_owned()),
                None => Self::Item(s.clone()),
            }),
            Value::Object(o) => {
                if let Some(kind) = o.get("type").and_then(Value::as_str) {
                    return match kind {
                        "neoforge:compound" => {
                            let children = o.get("children").ok_or("neoforge:compound without children")?;
                            Self::parse(children)
                        }
                        "minecraft:item" | "neoforge:item" if o.contains_key("item") => Self::parse_plain(o),
                        "minecraft:tag" | "neoforge:tag" if o.contains_key("tag") => Self::parse_plain(o),
                        other => Ok(Self::Unsupported(other.to_owned())),
                    };
                }
                Self::parse_plain(o)
            }
            other => Err(format!("not an ingredient: {other}")),
        }
    }

    fn parse_plain(o: &serde_json::Map<String, Value>) -> Result<Self, String> {
        if let Some(item) = o.get("item").and_then(Value::as_str) {
            Ok(Self::Item(item.to_owned()))
        } else if let Some(tag) = o.get("tag").and_then(Value::as_str) {
            Ok(Self::Tag(tag.to_owned()))
        } else {
            Err(format!("an ingredient names no item or tag: {}", Value::Object(o.clone())))
        }
    }

    /// `Ingredient.test(stack)`: an empty stack (`item` empty or air) never fits.
    pub fn test(&self, item: &str, tags: Tags<'_>) -> bool {
        if item.is_empty() || item == "minecraft:air" {
            return false;
        }
        match self {
            Self::Item(id) => id == item,
            Self::Tag(tag) => tags(item, tag),
            Self::Any(list) => list.iter().any(|i| i.test(item, tags)),
            Self::Unsupported(_) => false,
        }
    }

    /// Whether this port reads every part of it.
    pub fn supported(&self) -> bool {
        match self {
            Self::Unsupported(_) => false,
            Self::Any(list) => list.iter().all(Self::supported),
            _ => true,
        }
    }
}

/// NeoForge's `SizedIngredient` (`NESTED_CODEC`: `{"ingredient": .., "count": n}`, count 1 by default).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SizedIngredient {
    pub ingredient: Ingredient,
    pub count: u32,
}

impl SizedIngredient {
    pub fn parse(json: &Value) -> Result<Self, String> {
        let ingredient = Ingredient::parse(json.get("ingredient").ok_or("a sized ingredient without ingredient")?)?;
        let count = json.get("count").and_then(Value::as_u64).unwrap_or(1);
        let count = u32::try_from(count).map_err(|_| format!("count {count}"))?;
        if count == 0 {
            return Err("a sized ingredient of count 0".into());
        }
        Ok(Self { ingredient, count })
    }

    /// `SizedIngredient.test`: the item fits and the stack holds at least `count`.
    pub fn test(&self, item: &str, count: u32, tags: Tags<'_>) -> bool {
        self.ingredient.test(item, tags) && count >= self.count
    }
}

/// NeoForge's `FluidIngredient` as 1.21.1 JSON writes it: `{"fluid": id}`, `{"tag": tag}` or a list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FluidIngredient {
    Fluid(String),
    Tag(String),
    Any(Vec<FluidIngredient>),
    Unsupported(String),
}

impl FluidIngredient {
    pub fn parse(json: &Value) -> Result<Self, String> {
        match json {
            Value::Array(list) => Ok(Self::Any(list.iter().map(Self::parse).collect::<Result<_, _>>()?)),
            Value::Object(o) => {
                if let Some(fluid) = o.get("fluid").and_then(Value::as_str) {
                    Ok(Self::Fluid(fluid.to_owned()))
                } else if let Some(tag) = o.get("tag").and_then(Value::as_str) {
                    Ok(Self::Tag(tag.to_owned()))
                } else if let Some(kind) = o.get("type").and_then(Value::as_str) {
                    Ok(Self::Unsupported(kind.to_owned()))
                } else {
                    Err(format!("a fluid ingredient names no fluid or tag: {json}"))
                }
            }
            other => Err(format!("not a fluid ingredient: {other}")),
        }
    }

    /// `FluidIngredient.test(stack)`: an empty fluid never fits.
    pub fn test(&self, fluid: &str, tags: Tags<'_>) -> bool {
        if fluid.is_empty() || fluid == "minecraft:empty" {
            return false;
        }
        match self {
            Self::Fluid(id) => id == fluid,
            Self::Tag(tag) => tags(fluid, tag),
            Self::Any(list) => list.iter().any(|i| i.test(fluid, tags)),
            Self::Unsupported(_) => false,
        }
    }

    pub fn supported(&self) -> bool {
        match self {
            Self::Unsupported(_) => false,
            Self::Any(list) => list.iter().all(Self::supported),
            _ => true,
        }
    }
}

/// NeoForge's `SizedFluidIngredient` (`NESTED_CODEC`: `{"ingredient": .., "amount": n}`, 1000 by default).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SizedFluidIngredient {
    pub ingredient: FluidIngredient,
    pub amount: i32,
}

impl SizedFluidIngredient {
    pub fn parse(json: &Value) -> Result<Self, String> {
        let ingredient = FluidIngredient::parse(json.get("ingredient").ok_or("a sized fluid ingredient without ingredient")?)?;
        let amount = json.get("amount").and_then(Value::as_i64).unwrap_or(1000);
        let amount = i32::try_from(amount).ok().filter(|a| *a > 0).ok_or_else(|| format!("amount {amount}"))?;
        Ok(Self { ingredient, amount })
    }

    /// `SizedFluidIngredient.test`: the fluid fits and there is at least `amount` of it.
    pub fn test(&self, fluid: &str, amount: i32, tags: Tags<'_>) -> bool {
        self.ingredient.test(fluid, tags) && amount >= self.amount
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tags(item: &str, tag: &str) -> bool {
        matches!((item, tag), ("minecraft:oak_log", "minecraft:logs") | ("minecraft:water", "minecraft:water"))
    }

    #[test]
    fn items_tags_lists_and_sizes() {
        let i = Ingredient::parse(&json!({"item": "minecraft:beef"})).unwrap();
        assert!(i.test("minecraft:beef", &tags));
        assert!(!i.test("minecraft:air", &tags) && !i.test("", &tags));
        let t = Ingredient::parse(&json!([{"tag": "minecraft:logs"}, {"item": "minecraft:stone"}])).unwrap();
        assert!(t.test("minecraft:oak_log", &tags) && t.test("minecraft:stone", &tags) && !t.test("minecraft:dirt", &tags));
        let sized = SizedIngredient::parse(&json!({"count": 2, "ingredient": {"item": "minecraft:beef"}})).unwrap();
        assert!(!sized.test("minecraft:beef", 1, &tags), "one beef is short of two");
        assert!(sized.test("minecraft:beef", 2, &tags) && sized.test("minecraft:beef", 64, &tags));
        assert_eq!(SizedIngredient::parse(&json!({"ingredient": {"item": "minecraft:beef"}})).unwrap().count, 1);
        let odd = Ingredient::parse(&json!({"type": "neoforge:components", "items": "minecraft:potion"})).unwrap();
        assert!(!odd.supported() && !odd.test("minecraft:potion", &tags));
    }

    #[test]
    fn fluids() {
        let f = SizedFluidIngredient::parse(&json!({"amount": 10, "ingredient": {"fluid": "theurgy:sal_ammoniac"}})).unwrap();
        assert!(f.test("theurgy:sal_ammoniac", 10, &tags) && !f.test("theurgy:sal_ammoniac", 9, &tags));
        assert!(!f.test("minecraft:water", 1000, &tags));
        let w = SizedFluidIngredient::parse(&json!({"amount": 1000, "ingredient": {"tag": "minecraft:water"}})).unwrap();
        assert!(w.test("minecraft:water", 1000, &tags) && !w.test("minecraft:water", 999, &tags));
    }
}
