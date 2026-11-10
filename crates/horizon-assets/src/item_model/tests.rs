use super::*;
use serde_json::json;

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
        fn numeric_property(&self, property: &str, stack: &StackView, components: ComponentsView<'_>) -> f32 {
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

#[test]
fn a_mods_tint_source_is_untinted_and_an_unknown_vanilla_one_fails() {
    let tint = |json: &str| TintSource::decode(&serde_json::from_str(json).unwrap());
    assert_eq!(
        tint(r#"{"type":"ae2:storage_cell_state"}"#).unwrap(),
        TintSource::Constant(-1)
    );
    assert!(tint(r#"{"type":"minecraft:nonsense"}"#).is_err());
}

#[test]
fn custom_condition_is_evaluated_per_stack_at_branch_selection() {
    struct Env;
    impl ItemEnv for Env {
        fn item_model_property(&self, property: &str, stack: &StackView) -> bool {
            assert_eq!(property, "example:charged");
            stack.count > 1 && stack.patch.get("example:charged") == Some(&Some(Value::Bool(true)))
        }
    }
    let definition = ClientItem::from_json(
        r#"{"model":{
        "type":"minecraft:condition", "property":"example:charged",
        "on_true":{"type":"minecraft:model","model":"example:item/charged"},
        "on_false":{"type":"minecraft:model","model":"example:item/idle"}
    }}"#,
    )
    .unwrap();
    let mut models = ItemModels::default();
    models.definitions.insert("example:tool".into(), definition);
    models.prototypes.insert(
        "example:tool".into(),
        BTreeMap::from([(
            "minecraft:item_model".into(),
            Value::String("example:tool".into()),
        )]),
    );
    let context = Context {
        display: DisplayContext::Gui,
        owner: None,
        level: None,
        seed: 0,
    };
    let mut stack = StackView::new("example:tool", 1);
    let chosen = |stack: &StackView, env: &dyn ItemEnv| {
        let resolved = models.resolve(stack, &context, env);
        assert_eq!(resolved.layers.len(), 1);
        let LayerContent::Model { model } = &resolved.layers[0].content else {
            panic!("expected model leaf")
        };
        model.clone()
    };
    assert_eq!(chosen(&stack, &NoEnv), "example:item/idle");
    assert_eq!(chosen(&stack, &Env), "example:item/idle");
    stack
        .patch
        .insert("example:charged".into(), Some(Value::Bool(true)));
    assert_eq!(chosen(&stack, &Env), "example:item/idle");
    stack.count = 2;
    assert_eq!(chosen(&stack, &Env), "example:item/charged");
    assert_eq!(chosen(&stack, &NoEnv), "example:item/idle");
    stack.patch.insert("example:charged".into(), None);
    assert_eq!(chosen(&stack, &Env), "example:item/idle");
}

#[test]
fn custom_condition_validates_its_name_and_both_branches() {
    let node = |property: &str, on_true: Value| {
        ItemModelNode::decode(&serde_json::json!({
            "type": "minecraft:condition", "property": property,
            "on_true": on_true, "on_false": {"type": "minecraft:empty"}
        }))
    };
    let empty = serde_json::json!({"type": "minecraft:empty"});
    for name in [
        "minecraft:unknown",
        "unknown",
        "example:Bad",
        "bad name:charged",
    ] {
        assert!(node(name, empty.clone()).is_err(), "{name}");
    }
    assert!(node("example:charged", Value::Null).is_err());
    assert!(matches!(
        node("minecraft:damaged", empty.clone()).unwrap(),
        ItemModelNode::Condition {
            property: ConditionProperty::Damaged,
            ..
        }
    ));
    assert!(matches!(node("example:charged", empty).unwrap(),
        ItemModelNode::Condition { property: ConditionProperty::Custom(name), .. } if name == "example:charged"));
}

#[test]
fn a_patched_stack_reads_its_patch_over_its_items_defaults() {
    let mut models = ItemModels::default();
    models.prototypes.insert(
        "minecraft:diamond_sword".into(),
        Components::from_iter([
            (
                "minecraft:item_model".into(),
                json!("minecraft:diamond_sword"),
            ),
            ("minecraft:max_damage".into(), json!(1561)),
            ("minecraft:rarity".into(), json!("common")),
        ]),
    );
    let mut stack = StackView::new("minecraft:diamond_sword", 1);
    stack.patch.insert(
        "minecraft:enchantments".into(),
        Some(json!({"minecraft:sharpness": 5})),
    );
    stack
        .patch
        .insert("minecraft:rarity".into(), Some(json!("rare")));
    stack.patch.insert("minecraft:max_damage".into(), None);
    let view = models.components_view(&stack);
    assert_eq!(view.get("minecraft:rarity"), Some(&json!("rare")));
    assert_eq!(view.get("minecraft:max_damage"), None, "a removed default");
    assert_eq!(
        view.get("minecraft:item_model"),
        Some(&json!("minecraft:diamond_sword"))
    );
    assert!(view.contains_key("minecraft:enchantments"));
    assert!(!view.contains_key("minecraft:custom_name"));
    // Every key reads as the merged map has it.
    let merged = models.components(&stack);
    for key in [
        "minecraft:rarity",
        "minecraft:max_damage",
        "minecraft:item_model",
        "minecraft:enchantments",
        "minecraft:custom_name",
    ] {
        assert_eq!(view.get(key), merged.get(key), "{key}");
    }
    assert_eq!(view.to_map(), *merged);
}
