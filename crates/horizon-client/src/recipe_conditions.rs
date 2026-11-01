//! NeoForge's load conditions on recipes, with the item tags at hand: the
//! ones `horizon_world::conditions` answers from the loaded mods, and
//! `neoforge:tag_empty`, which the pinned AE2 19.2.18 recipe data asks at
//! data-load time against the resolved item tags.
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// The mods a `neoforge:mod_loaded` can name: `minecraft`, always, and the
/// content mods in this root.
pub(crate) fn loaded_mods(root: &Path) -> Vec<String> {
    let mut mods = horizon_items::content_namespaces::mod_ids(root);
    if !mods.iter().any(|m| m == "minecraft") {
        mods.push("minecraft".to_owned());
    }
    mods
}

/// Whether the recipe loads: a file without `neoforge:conditions` does, one
/// with an unknown condition type doesn't.
pub(crate) fn enabled(
    recipe: &Value,
    mods: &[String],
    tags: &HashMap<String, HashSet<String>>,
) -> bool {
    horizon_world::conditions::conditions_hold_with_tags(
        recipe,
        &|id: &str| mods.iter().any(|m| m == id),
        &|tag: &str| tags.get(tag).is_none_or(HashSet::is_empty),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ae2_optional_ammunition_tracks_resolved_tags_and_rejects_unknown_conditions() {
        let tags = HashMap::from([
            (
                "c:nuggets/iron".to_owned(),
                HashSet::from(["minecraft:iron_nugget".to_owned()]),
            ),
            ("c:nuggets/empty".to_owned(), HashSet::new()),
        ]);
        let mods = ["minecraft".to_owned()];
        let recipe = |tag: &str| {
            serde_json::json!({"neoforge:conditions":[{"type":"neoforge:not","value":{"type":"neoforge:tag_empty","tag":tag}}]})
        };
        assert!(enabled(&recipe("c:nuggets/iron"), &mods, &tags));
        assert!(!enabled(&recipe("c:nuggets/zinc"), &mods, &tags));
        assert!(!enabled(&recipe("c:nuggets/empty"), &mods, &tags));
        assert!(enabled(&serde_json::json!({}), &mods, &tags));
        assert!(!enabled(
            &serde_json::json!({"neoforge:conditions":[{"type":"unknown:condition"}]}),
            &mods,
            &tags
        ));
    }
    #[test]
    fn mod_conditions_and_tag_conditions_combine() {
        let tags = HashMap::from([(
            "c:ingots/iron".to_owned(),
            HashSet::from(["minecraft:iron_ingot".to_owned()]),
        )]);
        let mods = ["minecraft".to_owned(), "ae2".to_owned()];
        let both = |modid: &str| {
            serde_json::json!({"neoforge:conditions":[
                {"type":"neoforge:mod_loaded","modid":modid},
                {"type":"neoforge:not","value":{"type":"neoforge:tag_empty","tag":"c:ingots/iron"}}
            ]})
        };
        assert!(enabled(&both("ae2"), &mods, &tags));
        assert!(!enabled(&both("mekanism"), &mods, &tags));
    }
}
