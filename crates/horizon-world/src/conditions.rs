//! NeoForge's load conditions (`neoforge:conditions`): a data file that has
//! the key loads only when every condition in it holds, and is silently
//! absent otherwise, as NeoForge's `ICondition` does. Mods' packs lean on it
//! for cross-mod recipes — a recipe for another mod's material only makes
//! sense to load when that mod's data is in the root.
use serde_json::Value;

/// What a condition asks: whether a mod's data is loaded. Callers answer from
/// the content root's namespaces (`content_namespaces::namespaces`), where a
/// mod that is off leaves no namespace behind, plus `minecraft`, always.
pub type ModLoaded<'a> = dyn Fn(&str) -> bool + 'a;

/// What `neoforge:tag_empty` asks: whether an item tag has no items. Only a
/// caller with the resolved item tags can answer it.
pub type TagEmpty<'a> = dyn Fn(&str) -> bool + 'a;

/// Whether `v`'s `neoforge:conditions` all hold; a file without the key
/// loads, one with an unknown condition type doesn't.
pub fn conditions_hold(v: &Value, loaded: &ModLoaded) -> bool {
    holds(v, loaded, None)
}

/// `conditions_hold`, also answering `neoforge:tag_empty`.
pub fn conditions_hold_with_tags(v: &Value, loaded: &ModLoaded, tag_empty: &TagEmpty) -> bool {
    holds(v, loaded, Some(tag_empty))
}

fn holds(v: &Value, loaded: &ModLoaded, tag_empty: Option<&TagEmpty>) -> bool {
    fn hold(c: &Value, loaded: &ModLoaded, tag_empty: Option<&TagEmpty>) -> bool {
        let tags = tag_empty.is_some();
        let all = |key: &str| {
            c[key]
                .as_array()
                .into_iter()
                .flatten()
                .all(|c| hold(c, loaded, tag_empty))
        };
        match c["type"].as_str() {
            Some("neoforge:mod_loaded") => c["modid"].as_str().is_some_and(loaded),
            Some("neoforge:tag_empty") => match (tag_empty, c["tag"].as_str()) {
                (Some(empty), Some(tag)) => empty(tag),
                _ => false,
            },
            Some("neoforge:true") => true,
            // An unknown child fails the file: `not` of it is not a pass.
            Some("neoforge:not") => known(&c["value"], tags) && !hold(&c["value"], loaded, tag_empty),
            Some("neoforge:and") => all("values"),
            Some("neoforge:or") => {
                let values: Vec<&Value> = c["values"].as_array().into_iter().flatten().collect();
                values.iter().all(|v| known(v, tags))
                    && values.iter().any(|v| hold(v, loaded, tag_empty))
            }
            _ => false,
        }
    }
    v["neoforge:conditions"]
        .as_array()
        .into_iter()
        .flatten()
        .all(|c| hold(c, loaded, tag_empty))
}

/// Whether the condition's kind is one the game knows; an unknown kind
/// anywhere in the tree leaves the file out, through `not` and the
/// combinators alike. `tags`: the caller can answer `neoforge:tag_empty`.
fn known(c: &Value, tags: bool) -> bool {
    if !matches!(
        c["type"].as_str(),
        Some("neoforge:mod_loaded")
            | Some("neoforge:true")
            | Some("neoforge:not")
            | Some("neoforge:and")
            | Some("neoforge:or")
    ) && !(tags && c["type"].as_str() == Some("neoforge:tag_empty"))
    {
        return false;
    }
    // The whole tree must be known, not only this node: an unknown kind
    // nested inside `not` or a combinator leaves the file out too.
    match c["type"].as_str() {
        Some("neoforge:not") => known(&c["value"], tags),
        Some("neoforge:and") | Some("neoforge:or") => c["values"]
            .as_array()
            .into_iter()
            .flatten()
            .all(|c| known(c, tags)),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn loaded(id: &str) -> bool {
        matches!(id, "minecraft" | "create")
    }

    #[test]
    fn a_file_without_conditions_loads() {
        assert!(conditions_hold(
            &json!({"type": "minecraft:crafting"}),
            &loaded
        ));
    }

    #[test]
    fn mod_loaded_follows_the_answer_and_minecraft_always_holds() {
        assert!(conditions_hold(
            &json!({"neoforge:conditions": [{"type": "neoforge:mod_loaded", "modid": "create"}]}),
            &loaded
        ));
        assert!(!conditions_hold(
            &json!({"neoforge:conditions": [{"type": "neoforge:mod_loaded", "modid": "ic2"}]}),
            &loaded
        ));
        assert!(conditions_hold(
            &json!({"neoforge:conditions": [{"type": "neoforge:mod_loaded", "modid": "minecraft"}]}),
            &loaded
        ));
    }

    #[test]
    fn not_and_or_combine_and_all_must_hold() {
        let not = json!({"type": "neoforge:not", "value": {"type": "neoforge:mod_loaded", "modid": "ic2"}});
        let mut c = serde_json::Map::new();
        c.insert("neoforge:conditions".to_owned(), json!([not]));
        assert!(conditions_hold(&Value::Object(c.clone()), &loaded));
        let both = json!([
            {"type": "neoforge:mod_loaded", "modid": "create"},
            {"type": "neoforge:or", "values": [
                {"type": "neoforge:mod_loaded", "modid": "ic2"},
                {"type": "neoforge:true"}
            ]}
        ]);
        c.insert("neoforge:conditions".to_owned(), both);
        assert!(conditions_hold(&Value::Object(c), &loaded));
        // One failing condition in the list fails the file.
        let failing = json!([
            {"type": "neoforge:true"},
            {"type": "neoforge:mod_loaded", "modid": "ic2"}
        ]);
        let mut f = serde_json::Map::new();
        f.insert("neoforge:conditions".to_owned(), failing);
        assert!(!conditions_hold(&Value::Object(f), &loaded));
    }

    #[test]
    fn an_unknown_kind_nested_in_the_tree_leaves_the_file_out() {
        // `not` of `not` of an unknown: the unknown is two levels down.
        let nested_not = json!({
            "neoforge:conditions": [
                {"type": "neoforge:not", "value":
                    {"type": "neoforge:not", "value":
                        {"type": "example:unknown"}}}
            ]
        });
        assert!(!conditions_hold(&nested_not, &loaded));
        // `or` with a `true` beside a `not` of an unknown: one child is
        // known-true, but the other's subtree holds an unknown.
        let nested_or = json!({
            "neoforge:conditions": [
                {"type": "neoforge:or", "values": [
                    {"type": "neoforge:true"},
                    {"type": "neoforge:not", "value":
                        {"type": "example:unknown"}}
                ]}
            ]
        });
        assert!(!conditions_hold(&nested_or, &loaded));
    }

    #[test]
    fn an_unknown_condition_does_not_load_the_file() {
        let mut c = serde_json::Map::new();
        c.insert(
            "neoforge:conditions".to_owned(),
            json!([{"type": "neoforge:future_thing"}]),
        );
        assert!(!conditions_hold(&Value::Object(c), &loaded));
    }
}
