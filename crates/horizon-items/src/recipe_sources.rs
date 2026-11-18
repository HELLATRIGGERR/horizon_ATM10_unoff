//! Shared recipe resource merge, conditions and filters.
use crate::Result;
use horizon_world::loot::PackOverlay;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

/// A recipe's whole id from its name in the tables: vanilla's are kept bare
/// (`oak_planks`), everyone else's whole (`mymod:crusher/iron`). The book,
/// the network copy, the advancements' JSON and brewing know a recipe by
/// this id.
pub fn qualified(id: &str) -> String {
    if id.contains(':') {
        id.to_owned()
    } else {
        format!("minecraft:{id}")
    }
}

/// The recipe files the tables are built from, each id once: vanilla's by
/// bare name, then the content mods' as `<ns>:<name>`, the world's packs'
/// over them (`overlay`), parsed, and only those whose load conditions
/// hold. A server sends some of them to its clients (`recipe_content`).
pub fn sources(root: &Path, overlay: &PackOverlay) -> Result<Vec<(String, Value)>> {
    sources_tagged(root, overlay, None)
}

/// [`sources`], with the item tags a load condition may ask about at hand:
/// `neoforge:tag_empty`, which AE2's recipe data asks at data-load time against
/// the resolved item tags. Without them, the root's and packs' item tags are
/// read the first time a recipe asks.
pub fn sources_tagged(
    root: &Path,
    overlay: &PackOverlay,
    tags: Option<&HashMap<String, HashSet<String>>>,
) -> Result<Vec<(String, Value)>> {
    let mut read_tags = None;
    // Vanilla's recipes by bare name, then content mods' as `<ns>:<name>`.
    let mut sources: Vec<(String, Vec<u8>)> = Vec::new();
    // Load conditions: `mod_loaded` asks which mods' data is
    // in this root (`minecraft`, always, and the content mods').
    // `mod_loaded` asks which mods are loaded, not which namespaces
    // they own: a mod that also owns a namespace doesn't make that
    // namespace another mod.
    let inventory = horizon_mod_package::enabled::EnabledMods::read(root)?;
    let conditions = overlay
        .conditions
        .with_mod_inventory(inventory.server_ids())
        .with_diagnostics();
    for file in &horizon_world::json_folder::read(&root.join("data/minecraft/recipe"))?.files {
        sources.push((file.name.clone(), file.bytes()?.to_vec()));
    }
    for namespace in crate::content_namespaces::namespaces(root) {
        let dir = root.join(format!("data/{namespace}/recipe"));
        if dir.is_dir() {
            for file in &horizon_world::json_folder::read(&dir)?.files {
                sources.push((format!("{namespace}:{}", file.name), file.bytes()?.to_vec()));
            }
        }
    }
    // The world's packs' recipes over the game's own: the top pack's
    // file for each id replaces it (`SimpleJsonResourceReloadListener`
    // sees one resource per id, the resource stack's top).
    for (id, files) in overlay.kind("recipe") {
        let Some((ns, path)) = id.split_once(':') else {
            continue;
        };
        let name = if ns == "minecraft" {
            path.to_owned()
        } else {
            id.to_owned()
        };
        let bytes = PackOverlay::top(files).to_vec();
        match sources.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = bytes,
            None => sources.push((name, bytes)),
        }
    }
    let filters = crate::recipe_filters::RecipeFilters::load(root, overlay);
    let mut parsed = Vec::with_capacity(sources.len());
    for (name, bytes) in sources {
        // Vanilla's RecipeManager logs a file that does not parse and
        // goes on ("Couldn't parse data file").
        let mut v: Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(e) => {
                log::warn!("Horizon: recipe {name} not loaded: {e}");
                continue;
            }
        };
        // A file whose load conditions don't hold is silently absent.
        let holds = if asks_tag_empty(&v) {
            let tags = match tags {
                Some(tags) => tags,
                None => read_tags.get_or_insert_with(|| {
                    crate::items::load_tags(root, "item", overlay).unwrap_or_default()
                }),
            };
            horizon_world::conditions::conditions_hold_with_tags(
                &v,
                &|id: &str| conditions.mod_loaded(id),
                &|tag: &str| tags.get(tag).is_none_or(HashSet::is_empty),
            )
        } else {
            conditions.allows_id(&v, "recipe", &qualified(&name))
        };
        if !holds {
            continue;
        }
        // The data's recipe filters, over the final set.
        if !filters.apply(&qualified(&name), &mut v) {
            continue;
        }
        parsed.push((name, v));
    }
    conditions.check_resource_errors("recipe")?;
    Ok(parsed)
}

/// Whether a file's load conditions name `neoforge:tag_empty` anywhere.
fn asks_tag_empty(file: &Value) -> bool {
    fn names(v: &Value) -> bool {
        match v {
            Value::Object(o) => {
                o.get("type").and_then(Value::as_str) == Some("neoforge:tag_empty")
                    || o.values().any(names)
            }
            Value::Array(a) => a.iter().any(names),
            _ => false,
        }
    }
    file.get("neoforge:conditions").is_some_and(names)
}
