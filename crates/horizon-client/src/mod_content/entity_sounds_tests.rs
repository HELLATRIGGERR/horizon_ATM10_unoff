use super::super::EntityType;
use super::mod_entity_row;

#[test]
fn a_living_mod_types_row_is_its_templates_with_its_attributes() {
    let zombie = r#"{"width":0.6,"height":1.95,"eye_height":1.74,"category":"monster","fire_immune":false,"attributes":{"minecraft:max_health":20.0,"minecraft:movement_speed":0.23000000417232513,"minecraft:armor":2.0}}"#;
    let row = serde_json::value::RawValue::from_string(zombie.to_owned()).unwrap();
    let ghoul = |attributes: &[(&str, f64)], attachments: &[[f32; 3]]| EntityType {
        id: "ruby:ghoul".into(),
        template: "minecraft:zombie".into(),
        shadow_radius: Some(0.2),
        shadow_strength: Some(0.6),
        item: None,
        damage: 0.0,
        sounds: Default::default(),
        attributes: attributes.iter().map(|&(a, v)| (a.into(), v)).collect(),
        passenger_attachments: attachments.to_vec(),
        texture: None,
        category: None,
        width: None,
        height: None,
        eye_height: None,
        fire_immune: None,
    };
    let modded = ghoul(&[("minecraft:max_health", 40.0)], &[[0.0, 0.7, -0.1]]);
    let written: serde_json::Value =
        serde_json::from_slice(&mod_entity_row(&row, &modded).unwrap()).unwrap();
    let mut expected: serde_json::Value = serde_json::from_str(zombie).unwrap();
    expected["attributes"]["minecraft:max_health"] = 40.0.into();
    expected["shadow_radius"] = serde_json::json!(0.2);
    expected["shadow_strength"] = serde_json::json!(0.6);
    expected["template"] = "minecraft:zombie".into();
    expected["passenger_attachments"] = serde_json::json!([[0.0, 0.7, -0.1]]);
    assert_eq!(written, expected);
    // the untouched speed keeps the template's text, not a reparsed float.
    let text = String::from_utf8(mod_entity_row(&row, &ghoul(&[], &[])).unwrap()).unwrap();
    assert!(text.contains("0.23000000417232513"), "{text}");
    assert!(!text.contains("passenger_attachments"), "{text}");
    let e = mod_entity_row(&row, &ghoul(&[("minecraft:flying_speed", 1.0)], &[])).unwrap_err();
    assert!(e.contains("not one of minecraft:zombie's"), "{e}");
    // its own texture, drawn on the template's model, goes in the row.
    let textured = EntityType {
        texture: Some("ruby:entity/ghoul".into()),
        ..ghoul(&[], &[])
    };
    let written: serde_json::Value =
        serde_json::from_slice(&mod_entity_row(&row, &textured).unwrap()).unwrap();
    assert_eq!(written["texture"], "ruby:entity/ghoul");
    assert!(written.get("texture").is_some());
    // its own category, size and fire immunity replace the template's; the rest stays.
    let sized = EntityType {
        category: Some("creature".into()),
        width: Some(1.4),
        eye_height: Some(1.6915001),
        fire_immune: Some(true),
        ..ghoul(&[], &[])
    };
    let written: serde_json::Value =
        serde_json::from_slice(&mod_entity_row(&row, &sized).unwrap()).unwrap();
    assert_eq!(written["category"], "creature");
    assert_eq!(written["width"], serde_json::json!(1.4));
    assert_eq!(written["height"], serde_json::json!(1.95));
    assert_eq!(written["eye_height"], serde_json::json!(1.6915001));
    assert_eq!(written["fire_immune"], true);
    let text = String::from_utf8(mod_entity_row(&row, &sized).unwrap()).unwrap();
    assert!(text.contains(r#""eye_height":1.6915001"#), "{text}");
}

#[test]
fn a_living_mod_type_spawns_with_its_own_category_size_and_fire_immunity() {
    // the template's row, and the row a content root writes for an imp declared on it.
    let zombie = r#"{"width":0.6,"height":1.95,"eye_height":1.74,"category":"monster","fire_immune":false,"attributes":{"minecraft:max_health":20.0,"minecraft:movement_speed":0.23000000417232513}}"#;
    let row = serde_json::value::RawValue::from_string(zombie.to_owned()).unwrap();
    let imp = EntityType {
        id: "ruby:imp".into(),
        template: "minecraft:zombie".into(),
        shadow_radius: None,
        shadow_strength: None,
        item: None,
        damage: 0.0,
        sounds: Default::default(),
        attributes: [("minecraft:max_health".to_owned(), 12.0)].into(),
        passenger_attachments: Vec::new(),
        texture: Some("ruby:entity/imp".into()),
        category: Some("creature".into()),
        width: Some(0.4),
        height: Some(0.9),
        eye_height: Some(0.8),
        fire_immune: Some(true),
    };
    let imp_row = String::from_utf8(mod_entity_row(&row, &imp).unwrap()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("entity-types.json"),
        format!(r#"{{"minecraft:zombie":{zombie},"ruby:imp":{imp_row}}}"#),
    )
    .unwrap();
    // No attribute catalog: the rows' values stay as written.
    let attributes = horizon_world::attributes::entity::Catalog::from_json(&serde_json::json!({
        "data_version": horizon_world::DATA_VERSION,
        "attributes": [],
        "entities": {},
    }))
    .unwrap();
    let types = crate::mobs::MobTypes::load_for_server(dir.path(), &attributes).unwrap();
    let (zombie, imp) = (&types.types["minecraft:zombie"], &types.types["ruby:imp"]);
    assert_eq!(imp.species, zombie.species);
    assert_eq!(imp.texture, "ruby:entity/imp");
    assert_eq!(imp.max_health, 12.0);
    assert_eq!(imp.category, "creature");
    assert_eq!((imp.width, imp.height, imp.eye_height), (0.4, 0.9, 0.8));
    assert!(imp.flags.fire_immune && !zombie.flags.fire_immune);
    assert_eq!((zombie.category.as_str(), zombie.width), ("monster", 0.6));
}
