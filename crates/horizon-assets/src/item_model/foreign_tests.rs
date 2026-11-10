use super::*;

fn node(json: &str) -> Result<ItemModelNode> {
    ItemModelNode::decode(&serde_json::from_str(json).unwrap())
}
fn model_id(n: &ItemModelNode) -> &str {
    match n {
        ItemModelNode::Model(leaf) => &leaf.model,
        other => panic!("not a model: {other:?}"),
    }
}

#[test]
fn node_with_a_model_id_draws_that_model() {
    let n = node(r#"{"type":"mod:thing","model":"mod:item/thing","tints":[{"type":"mod:tint"}]}"#)
        .unwrap();
    assert_eq!(model_id(&n), "mod:item/thing");
    let ItemModelNode::Model(leaf) = n else {
        unreachable!()
    };
    assert_eq!(leaf.tints, vec![TintSource::Constant(-1)]);
    let n = node(r#"{"type":"mod:thing","base_model":"mod:item/base"}"#).unwrap();
    assert_eq!(model_id(&n), "mod:item/base");
    let n = node(r#"{"type":"mod:thing","on_model":"mod:item/a"}"#).unwrap();
    assert_eq!(model_id(&n), "mod:item/a");
}

#[test]
fn node_with_a_nested_node_decodes_it() {
    let n = node(
        r#"{"type":"neoforge:trimmed_armor","base_model":{"type":"minecraft:model","model":"mod:item/boots"}}"#,
    )
    .unwrap();
    assert_eq!(model_id(&n), "mod:item/boots");
    let n = node(
        r#"{"type":"mod:x","model":{"type":"mod:y","model":"mod:item/inner"},"transformation":[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]}"#,
    )
    .unwrap();
    let ItemModelNode::Composite {
        models,
        transformation,
    } = n
    else {
        panic!("a transformation wraps the nested node")
    };
    assert!(transformation.is_some());
    assert_eq!(model_id(&models[0]), "mod:item/inner");
}

#[test]
fn node_with_nothing_usable_is_empty() {
    assert!(matches!(
        node(r#"{"type":"mod:facade"}"#).unwrap(),
        ItemModelNode::Empty
    ));
    assert!(matches!(
        node(r#"{"type":"mod:x","model":7}"#).unwrap(),
        ItemModelNode::Empty
    ));
    assert!(ClientItem::from_json(r#"{"model":{"type":"ae2:facade"}}"#).is_ok());
}

#[test]
fn unknown_vanilla_names_still_fail() {
    assert!(node(r#"{"type":"minecraft:nope"}"#).is_err());
    assert!(node(r#"{"type":"nope","model":"a:b"}"#).is_err());
    let leaf = r#"{"type":"minecraft:model","model":"a:b"}"#;
    for json in [
        format!(
            r#"{{"type":"minecraft:condition","property":"minecraft:nope","on_true":{leaf},"on_false":{leaf}}}"#
        ),
        format!(
            r#"{{"type":"minecraft:select","property":"minecraft:nope","cases":[],"fallback":{leaf}}}"#
        ),
        format!(
            r#"{{"type":"minecraft:range_dispatch","property":"minecraft:nope","entries":[],"fallback":{leaf}}}"#
        ),
    ] {
        assert!(node(&json).is_err(), "{json}");
    }
    assert!(
        node(r#"{"type":"minecraft:special","base":"a:b","model":{"type":"minecraft:nope"}}"#)
            .is_err()
    );
    assert!(ClientItem::from_json(r#"{"model":{"type":"minecraft:nope"}}"#).is_err());
}

#[test]
fn condition_property_is_false() {
    let n = node(
        r#"{"type":"minecraft:condition","property":"mod:flag",
            "on_true":{"type":"minecraft:model","model":"a:yes"},
            "on_false":{"type":"minecraft:model","model":"a:no"}}"#,
    )
    .unwrap();
    // A mod's property stays a condition its mod answers per stack
    // (`item_model_property`); with no mod answering it, `on_false` is drawn.
    let ItemModelNode::Condition {
        property: ConditionProperty::Custom(property),
        on_false,
        ..
    } = &n
    else {
        panic!("not a custom condition: {n:?}")
    };
    assert_eq!(property, "mod:flag");
    assert_eq!(model_id(on_false), "a:no");
}

#[test]
fn select_property_takes_the_fallback_or_nothing() {
    let n = node(
        r#"{"type":"minecraft:select","property":"mod:level",
            "cases":[{"when":"low","model":{"type":"minecraft:model","model":"a:low"}}],
            "fallback":{"type":"minecraft:model","model":"a:other"}}"#,
    )
    .unwrap();
    assert_eq!(model_id(&n), "a:other");
    let n = node(r#"{"type":"minecraft:select","property":"mod:level","cases":[]}"#).unwrap();
    assert!(matches!(n, ItemModelNode::Empty));
}

#[test]
fn range_property_reads_zero() {
    let entry = |t: f32, m: &str| {
        format!(r#"{{"threshold":{t},"model":{{"type":"minecraft:model","model":"{m}"}}}}"#)
    };
    let fallback = r#","fallback":{"type":"minecraft:model","model":"a:fb"}"#;
    let make = |entries: &[String], fallback: &str| {
        node(&format!(
            r#"{{"type":"minecraft:range_dispatch","property":"mod:fill","entries":[{}]{fallback}}}"#,
            entries.join(",")
        ))
        .unwrap()
    };
    // The range stays the host's to answer (`NumericProperty::Host`); one that
    // knows no such property (`NoEnv`) answers 0.
    let drawn = |n: ItemModelNode| {
        let models = ItemModels {
            definitions: BTreeMap::from([(
                "a:item".to_owned(),
                ClientItem {
                    model: n,
                    hand_animation_on_swap: true,
                    oversized_in_gui: false,
                    swap_animation_scale: 1.0,
                },
            )]),
            prototypes: BTreeMap::from([(
                "a:item".to_owned(),
                Components::from([(
                    "minecraft:item_model".to_owned(),
                    Value::String("a:item".into()),
                )]),
            )]),
        };
        let ctx = Context {
            display: DisplayContext::Gui,
            owner: None,
            level: None,
            seed: 0,
        };
        let layers = models
            .resolve(&StackView::new("a:item", 1), &ctx, &NoEnv)
            .layers;
        layers
            .into_iter()
            .map(|layer| match layer.content {
                LayerContent::Model { model } => model,
                other => panic!("not a model: {other:?}"),
            })
            .collect::<Vec<_>>()
    };
    let n = make(&[entry(0.2, "a:one"), entry(0.4, "a:two")], fallback);
    assert!(
        matches!(&n, ItemModelNode::RangeDispatch { property: NumericProperty::Host(p), .. } if p == "mod:fill")
    );
    assert_eq!(drawn(n), ["a:fb"]);
    let n = make(
        &[
            entry(0.0, "a:zero"),
            entry(-1.0, "a:neg"),
            entry(0.5, "a:half"),
        ],
        fallback,
    );
    assert_eq!(drawn(n), ["a:zero"]);
    let n = make(&[entry(0.2, "a:one")], "");
    assert!(drawn(n).is_empty());
}

#[test]
fn special_type_keeps_its_base_model() {
    let n = node(
        r#"{"type":"minecraft:special","base":"mod:item/cube","model":{"type":"mod:energy_cube"}}"#,
    )
    .unwrap();
    assert_eq!(model_id(&n), "mod:item/cube");
    let n =
        node(r#"{"type":"minecraft:special","base":"a:b","model":{"type":"minecraft:shield"}}"#)
            .unwrap();
    assert!(matches!(n, ItemModelNode::Special { .. }));
}

#[test]
fn fluid_container_draws_its_base_without_the_fluid() {
    let n = node(
        r#"{"type":"neoforge:fluid_container","fluid":"mod:gas","cover_is_mask":false,"flip_gas":true,
            "textures":{"base":"minecraft:item/bucket","fluid":"neoforge:item/mask/bucket_fluid"}}"#,
    )
    .unwrap();
    assert_eq!(model_id(&n), "minecraft:item/bucket");
    let n = node(
        r#"{"type":"neoforge:fluid_container","cover_is_mask":false,
            "textures":{"base":"a:item/flask","cover":"a:item/flask_cover"}}"#,
    )
    .unwrap();
    let ItemModelNode::Composite { models, .. } = n else {
        panic!("base and cover")
    };
    assert_eq!(models.len(), 2);
    assert!(matches!(
        node(r#"{"type":"neoforge:fluid_container"}"#).unwrap(),
        ItemModelNode::Empty
    ));
}
