//! An ingredient as Occultism's recipes name it: 1.21.1's `Ingredient.CODEC` with NeoForge's compound type, the
//! forms ATM10's Occultism recipes use (`{"item": ..}`, `{"tag": ..}`, a list, `neoforge:compound`), and 26.3's bare
//! strings (`"minecraft:stone"`, `"#c:ingots/iron"`).

use serde_json::Value;

/// What an ingredient asks of a stack. `test` takes the stack's item id and a tag lookup, so the engine's merged tags
/// answer `#c:ingots/iron`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ingredient {
    Item(String),
    Tag(String),
    /// A list (`[..]`) or `neoforge:compound` (its `children`): any one of them.
    Any(Vec<Ingredient>),
    /// A type this port doesn't read: it matches nothing.
    Unsupported(String),
}

/// A bare id is `minecraft:`'s, as `ResourceLocation.parse` reads it.
pub fn qualified(id: &str) -> String {
    if id.contains(':') { id.to_owned() } else { format!("minecraft:{id}") }
}

impl Ingredient {
    pub fn parse(json: &Value) -> Ingredient {
        match json {
            Value::Array(list) => Ingredient::Any(list.iter().map(Ingredient::parse).collect()),
            Value::String(s) => match s.strip_prefix('#') {
                Some(tag) => Ingredient::Tag(qualified(tag)),
                None => Ingredient::Item(qualified(s)),
            },
            Value::Object(map) => match map.get("type").and_then(Value::as_str) {
                Some("neoforge:compound") => Ingredient::Any(
                    map.get("children").and_then(Value::as_array).map_or_else(Vec::new, |c| c.iter().map(Ingredient::parse).collect()),
                ),
                None | Some("minecraft:item") | Some("minecraft:tag") => {
                    if let Some(item) = map.get("item").and_then(Value::as_str) {
                        Ingredient::Item(qualified(item))
                    } else if let Some(tag) = map.get("tag").and_then(Value::as_str) {
                        Ingredient::Tag(qualified(tag.trim_start_matches('#')))
                    } else {
                        Ingredient::Unsupported("no item or tag".into())
                    }
                }
                Some(other) => Ingredient::Unsupported(other.to_owned()),
            },
            _ => Ingredient::Unsupported(json.to_string()),
        }
    }

    /// `Ingredient.test(stack)`: an empty stack (`minecraft:air`, `""`) never fits.
    pub fn test(&self, item: &str, in_tag: &dyn Fn(&str, &str) -> bool) -> bool {
        if item.is_empty() || item == "minecraft:air" {
            return false;
        }
        match self {
            Ingredient::Item(id) => id == item,
            Ingredient::Tag(tag) => in_tag(item, tag),
            Ingredient::Any(children) => children.iter().any(|c| c.test(item, in_tag)),
            Ingredient::Unsupported(_) => false,
        }
    }

    /// The first item it names, for messages (`Ingredient.getItems()[0]`); tags give none here.
    pub fn first_item(&self) -> Option<&str> {
        match self {
            Ingredient::Item(id) => Some(id),
            Ingredient::Any(children) => children.iter().find_map(Ingredient::first_item),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tags(item: &str, tag: &str) -> bool {
        matches!((item, tag), ("minecraft:iron_ingot", "c:ingots/iron") | ("minecraft:glass", "c:glass_blocks"))
    }

    #[test]
    fn the_forms_of_1_21_1_and_26_3_parse_and_test() {
        let compound = json!({"type": "neoforge:compound", "children": [{"item": "occultism:otherstone"}, {"item": "occultism:otherrock"}]});
        let any = Ingredient::parse(&compound);
        assert!(any.test("occultism:otherrock", &tags));
        assert!(!any.test("minecraft:stone", &tags));
        assert!(Ingredient::parse(&json!({"tag": "c:ingots/iron"})).test("minecraft:iron_ingot", &tags));
        assert!(Ingredient::parse(&json!("#c:glass_blocks")).test("minecraft:glass", &tags));
        assert!(Ingredient::parse(&json!("glass")).test("minecraft:glass", &tags));
        assert!(Ingredient::parse(&json!([{"item": "minecraft:egg"}, "#c:ingots/iron"])).test("minecraft:egg", &tags));
        let other = Ingredient::parse(&json!({"type": "neoforge:components", "items": "minecraft:egg"}));
        assert_eq!(other, Ingredient::Unsupported("neoforge:components".into()));
        assert!(!other.test("minecraft:egg", &tags));
        assert!(!Ingredient::parse(&json!({"item": "minecraft:air"})).test("minecraft:air", &tags), "an empty stack never fits");
    }
}
