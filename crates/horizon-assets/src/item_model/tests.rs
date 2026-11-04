use super::*;

#[test]
fn identifier_adds_default_namespace() {
    assert_eq!(identifier("stone").unwrap(), "minecraft:stone");
    assert_eq!(identifier("a:b").unwrap(), "a:b");
}

#[test]
fn positive_modulo_wraps_negatives() {
    assert!((positive_modulo(-0.25, 1.0) - 0.75).abs() < 1e-6);
    assert!((positive_modulo(1.5, 1.0) - 0.5).abs() < 1e-6);
}

#[test]
fn display_context_names_round_trip() {
    for name in ["gui", "ground", "fixed", "head"] {
        assert_eq!(DisplayContext::by_name(name).unwrap().name(), name);
    }
}

/// A mod's own range dispatch property (AE2's `ae2:fill_level` on an energy
/// cell) is the host's to answer; vanilla's namespace keeps its closed set.
#[test]
fn mod_range_dispatch_properties_ask_the_host() {
    struct Fill;
    impl ItemEnv for Fill {
        fn numeric_property(&self, property: &str, stack: &StackView, components: &Components) -> f32 {
            let stored = components.get("ae2:stored_energy").and_then(Value::as_f64).unwrap_or(0.0);
            if property == "ae2:fill_level" && stack.item == "ae2:energy_cell" { (stored / 200_000.0) as f32 } else { 0.0 }
        }
    }
    let entry = |threshold: f32, n: u8| serde_json::json!({"threshold": threshold, "model": {"type": "minecraft:model", "model": format!("ae2:block/energy_cell_{n}")}});
    let item = ClientItem::decode(&serde_json::json!({"model": {"type": "minecraft:range_dispatch", "property": "ae2:fill_level",
        "entries": [entry(0.2, 1), entry(0.4, 2), entry(0.6, 3), entry(0.8, 4)],
        "fallback": {"type": "minecraft:model", "model": "ae2:item/energy_cell"}}})).unwrap();
    let prototype = Components::from([("minecraft:item_model".to_owned(), serde_json::json!("ae2:energy_cell"))]);
    let models = ItemModels {
        definitions: BTreeMap::from([("ae2:energy_cell".to_owned(), item)]),
        prototypes: BTreeMap::from([("ae2:energy_cell".to_owned(), prototype)]),
    };
    let ctx = Context { display: DisplayContext::Gui, owner: None, level: None, seed: 0 };
    let drawn = |stored: Option<f64>, env: &dyn ItemEnv| {
        let mut stack = StackView::new("ae2:energy_cell", 1);
        if let Some(stored) = stored {
            stack.patch.insert("ae2:stored_energy".into(), Some(serde_json::json!(stored)));
        }
        match &models.resolve(&stack, &ctx, env).layers[0].content {
            LayerContent::Model { model } => model.clone(),
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(drawn(None, &Fill), "ae2:item/energy_cell");
    assert_eq!(drawn(Some(39_999.0), &Fill), "ae2:item/energy_cell");
    assert_eq!(drawn(Some(40_000.0), &Fill), "ae2:block/energy_cell_1");
    assert_eq!(drawn(Some(130_000.0), &Fill), "ae2:block/energy_cell_3");
    assert_eq!(drawn(Some(200_000.0), &Fill), "ae2:block/energy_cell_4");
    assert_eq!(drawn(Some(200_000.0), &NoEnv), "ae2:item/energy_cell", "a host that knows no such property answers 0");
    let unknown = serde_json::json!({"model": {"type": "minecraft:range_dispatch", "property": "minecraft:fill_level", "entries": []}});
    assert!(ClientItem::decode(&unknown).is_err());
}
