//! Effective configured-carver registries for biome modifiers.
use super::{J, Result, SavedModifiers, biomes::Registry};
use horizon_world::worldgen::{carver::WorldCarver, overrides::WorldgenOverrides};
use std::collections::BTreeSet;

pub(super) fn expand(doc: &mut J, registry: &Registry<'_>) -> Result<()> {
    if let Some(J::Str(tag)) = doc.get("carvers")
        && tag.starts_with('#')
    {
        let selector = serde_json::Value::String(tag.clone());
        let ids = registry.resolve(&selector)?;
        doc.set("carvers", J::Arr(ids.into_iter().map(J::Str).collect()));
    }
    Ok(())
}

/// Leaves out the mods' configured carvers the engine can't build (a mod's own carver
/// type: Twilight Forest's `tf_caves`, Ad Astra's `crater`), each with a warning.
pub(super) fn keep_readable(entries: &mut Vec<(String, String)>, warnings: &mut Vec<String>) {
    entries.retain(|(id, text)| match WorldCarver::from_json(text) {
        Ok(_) => true,
        Err(e) => {
            warnings.push(format!("carver {id} left out: {e}"));
            false
        }
    });
}

/// A mod biome's text without the carvers that aren't in `known` (or, written inline,
/// that the engine can't build); `None` when it keeps them all.
pub(super) fn drop_unknown(
    id: &str,
    text: &str,
    known: &BTreeSet<String>,
    warnings: &mut Vec<String>,
) -> Result<Option<String>> {
    let mut doc: J = serde_json::from_str(text).map_err(|e| format!("biome {id}: {e}"))?;
    let list = match doc.get("carvers") {
        // A tag (`#ns:tag`), which 26.3's holder set allows: `expand` reads it.
        Some(J::Str(name)) if name.starts_with('#') => return Ok(None),
        Some(J::Str(name)) => vec![J::Str(name.clone())],
        Some(J::Arr(list)) => list.clone(),
        _ => return Ok(None),
    };
    let (mut kept, mut dropped) = (Vec::new(), Vec::new());
    for entry in list {
        let ok = match &entry {
            J::Str(name) if name.contains(':') => known.contains(name),
            J::Str(name) => known.contains(&format!("minecraft:{name}")),
            inline => WorldCarver::from_json(&serde_json::to_string(inline)?).is_ok(),
        };
        if ok {
            kept.push(entry);
        } else {
            dropped.push(match entry {
                J::Str(name) => name,
                _ => "(inline)".to_owned(),
            });
        }
    }
    if dropped.is_empty() {
        return Ok(None);
    }
    warnings.push(format!(
        "biome {id}: carvers left out, the engine has none such: {}",
        dropped.join(", ")
    ));
    doc.set("carvers", J::Arr(kept));
    Ok(Some(serde_json::to_string(&doc)?))
}

pub(super) fn refresh(saved: &mut SavedModifiers, packs: &WorldgenOverrides) -> Result<()> {
    saved
        .carver_ids
        .extend(packs.entries("carvers").map(|(id, _)| id.to_owned()));
    for (id, _, text) in packs.tags("carver_tags") {
        let tag: J = serde_json::from_str(text)?;
        let (values, remove) = (
            super::tag_list(&tag, "values"),
            super::tag_list(&tag, "remove"),
        );
        let old = if matches!(tag.get("replace"), Some(J::Bool(true))) {
            None
        } else {
            saved.carver_tags.get(id).map(String::as_str)
        };
        let layers = [super::TagLayers { values, remove }];
        let merged = super::merge_tag(
            old,
            &layers,
            id,
            &horizon_world::conditions::LoadConditions::new(
                saved.enabled_mods.iter().map(String::as_str),
            ),
        )?;
        saved.carver_tags.insert(id.to_owned(), merged);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Vanilla's `minecraft:cave`.
    const CAVE: &str = r#"{"type":"minecraft:cave","count":{"type":"minecraft:very_biased_to_bottom","max_inclusive":14,"min_inclusive":0},"floor_level":{"type":"minecraft:uniform","max_exclusive":-0.4,"min_inclusive":-1.0},"horizontal_radius_multiplier":{"type":"minecraft:uniform","max_exclusive":1.4,"min_inclusive":0.7},"probability":0.15,"room_vertical_radius_multiplier":{"type":"minecraft:uniform","max_exclusive":0.9,"min_inclusive":0.1},"thickness":{"type":"minecraft:trapezoid","max":3.0,"min":0.0,"plateau":1.0},"vertical_radius_multiplier":{"type":"minecraft:uniform","max_exclusive":1.3,"min_inclusive":0.8},"weird_thickness_bias":true,"y":{"type":"minecraft:uniform","max_inclusive":{"absolute":180},"min_inclusive":{"above_bottom":8}}}"#;
    #[test]
    fn carvers_the_engine_cant_build_or_find_are_left_out() {
        let mut warnings = Vec::new();
        let mut entries = vec![
            ("zinc:cave".to_owned(), CAVE.to_owned()),
            ("zinc:own".to_owned(), r#"{"type":"zinc:own_caves"}"#.to_owned()),
        ];
        keep_readable(&mut entries, &mut warnings);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "zinc:cave");
        assert!(warnings[0].starts_with("carver zinc:own left out"), "{warnings:?}");

        let known: BTreeSet<String> = ["minecraft:cave", "zinc:cave"].map(String::from).into();
        let biome = r#"{"temperature":0.5,"carvers":["minecraft:cave","zinc:cave","zinc:own","cave"]}"#;
        let kept = drop_unknown("zinc:depths", biome, &known, &mut warnings)
            .unwrap()
            .expect("zinc:own is dropped");
        let doc: serde_json::Value = serde_json::from_str(&kept).unwrap();
        assert_eq!(
            doc["carvers"],
            serde_json::json!(["minecraft:cave", "zinc:cave", "cave"])
        );
        assert!(warnings[1].contains("zinc:own"), "{warnings:?}");
        // A single carver named alone, and one the engine has, stay as they are.
        let single = r#"{"carvers":"zinc:cave"}"#;
        assert!(drop_unknown("zinc:b", single, &known, &mut warnings).unwrap().is_none());
        let gone = drop_unknown("zinc:c", r#"{"carvers":"zinc:own"}"#, &known, &mut warnings)
            .unwrap()
            .unwrap();
        assert_eq!(serde_json::from_str::<serde_json::Value>(&gone).unwrap()["carvers"], serde_json::json!([]));
        // A tag of carvers is left as it is, even one no carver id matches.
        let tagged = r##"{"carvers":"#zinc:caves"}"##;
        assert!(drop_unknown("zinc:d", tagged, &known, &mut warnings).unwrap().is_none());
        assert_eq!(warnings.len(), 3, "{warnings:?}");
    }
    #[test]
    fn existing_tag_expands_to_effective_carvers() {
        let ids = ["test:cave".to_owned()].into_iter().collect();
        let tags = [(
            "test:all".to_owned(),
            r#"{"values":["test:cave"]}"#.to_owned(),
        )]
        .into_iter()
        .collect();
        let registry = Registry {
            what: "carver",
            ids: &ids,
            tags: &tags,
        };
        let mut doc: J = serde_json::from_str(r##"{"carvers":"#test:all"}"##).unwrap();
        expand(&mut doc, &registry).unwrap();
        assert_eq!(
            serde_json::to_string(doc.get("carvers").unwrap()).unwrap(),
            r#"["test:cave"]"#
        );
    }
    #[test]
    fn pack_carvers_and_replacement_tags_refresh() {
        let mut saved: SavedModifiers = serde_json::from_str(r#"{"enabled_mods":[],"modifiers":[],"unmodified":{},"feature_tags":{},"entity_categories":{},"entity_tags":{},"carver_tags":{"test:all":"{\"values\":[\"test:old\"]}"}}"#).unwrap();
        let mut packs = WorldgenOverrides::default();
        packs.set("carvers", "test:new", "{}".into());
        packs.add_tag(
            "carver_tags",
            "test:all",
            "test",
            r#"{"replace":true,"values":["test:new"]}"#.into(),
        );
        refresh(&mut saved, &packs).unwrap();
        let registry = Registry {
            what: "carver",
            ids: &saved.carver_ids,
            tags: &saved.carver_tags,
        };
        assert_eq!(
            registry.resolve(&serde_json::json!("#test:all")).unwrap(),
            ["test:new"]
        );
    }
    #[test]
    fn a_pack_tags_remove_list_takes_carvers_out() {
        let mut saved: SavedModifiers = serde_json::from_str(r##"{"enabled_mods":[],"modifiers":[],"unmodified":{},"feature_tags":{},"entity_categories":{},"entity_tags":{},"carver_tags":{"test:all":"{\"values\":[\"test:cave\",\"test:canyon\",\"#test:wet\"]}","test:wet":"{\"values\":[\"test:sea\"]}"}}"##).unwrap();
        saved
            .carver_ids
            .extend(["test:cave", "test:canyon", "test:sea"].map(String::from));
        let mut packs = WorldgenOverrides::default();
        packs.add_tag(
            "carver_tags",
            "test:all",
            "test",
            r##"{"values":[],"remove":["test:canyon","#test:wet","#test:absent"]}"##.into(),
        );
        refresh(&mut saved, &packs).unwrap();
        let registry = Registry {
            what: "carver",
            ids: &saved.carver_ids,
            tags: &saved.carver_tags,
        };
        assert_eq!(
            registry.resolve(&serde_json::json!("#test:all")).unwrap(),
            ["test:cave"]
        );
    }
    #[test]
    fn pack_carvers_without_modifiers_leave_world_data_unchanged() {
        let root = std::env::temp_dir().join(format!("carver-invariance-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("biome-modifiers.json"), r#"{"enabled_mods":[],"modifiers":[],"unmodified":{},"feature_tags":{},"entity_categories":{},"entity_tags":{}}"#).unwrap();
        let mut packs = WorldgenOverrides::default();
        packs.set("carvers", "test:new", "{}".into());
        packs.set("biomes", "test:plains", r#"{"carvers":"test:new"}"#.into());
        let result = super::super::biome_modifiers_over_packs(&root, &packs, &[])
            .unwrap()
            .unwrap();
        assert_eq!(result, packs);
        std::fs::remove_dir_all(root).unwrap();
    }
}
