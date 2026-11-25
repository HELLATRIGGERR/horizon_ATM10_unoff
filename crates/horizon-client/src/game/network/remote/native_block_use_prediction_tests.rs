//! A server's client predicts a click on a mod block as the server answers it
//! (`interaction_prediction.rs`, `items/use_hand.rs`): SUCCESS, which swings the hand and sends no
//! `UseItem` after the `UseItemOn`, for a fluid container on a block a bundle names with
//! `fluid_interact_on_use` and for the menu of a mod container on a block whose template opens none.
use crate::game::fork_compat::test_bundle::{Bundle, with_bundle};
use crate::game::*;
use horizon_client::{
    chunk::Chunk,
    containers::ModContainer,
    fast_hash::FxHashMap,
    first_person::Hand,
    items::ItemStack,
};
use std::sync::Arc;

/// A window joined to a server through memory, with a carved pumpkin (a template whose use passes)
/// at the click.
fn with_client<R>(f: impl FnOnce(&mut Game<'_>) -> R) -> R {
    let root = std::path::PathBuf::from(std::env::var_os("HORIZON_TEST_RESOURCES").unwrap());
    let settings = Settings::default();
    let loaded = load_resources(
        horizon_assets::resource_source::ResourceRoot::directory(root.clone()),
        Vec::new(),
        None,
        &root,
        &settings,
        std::time::Instant::now(),
        false,
        crate::game::execution_role::ExecutionRole::IntegratedServer,
        &Progress::new(),
        &mut |_, _| unreachable!(),
    )
    .unwrap();
    let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
    options.resources = root.clone();
    let mut game = Game::new(
        &loaded.resources,
        loaded.pack,
        loaded.gui.unwrap(),
        options,
        settings,
        Audio::silent(&horizon_assets::resource_source::ResourceSource::directory(
            root.to_path_buf(),
        )),
        None,
    )
    .unwrap();
    let (pipe, _server) = horizon_protocol::connection::Pipe::pair();
    game.connect_local(pipe, "Steve").unwrap();
    game.world
        .chunks
        .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
    let pumpkin = game
        .pack
        .assets
        .registry
        .default_state("minecraft:carved_pumpkin")
        .unwrap();
    game.world.set(2, 4, 0, pumpkin).unwrap();
    game.forced_hit = Some(Hit {
        pos: [2, 4, 0],
        face: [0, 0, 1],
        point: [2.5, 4.5, 1.0],
        inside: false,
    });
    f(&mut game)
}

#[test]
#[ignore = "daily"]
fn a_client_predicts_a_fluid_container_on_a_block_that_takes_them() {
    with_client(|game| {
        let state = game.world.block(2, 4, 0);
        *game.held_mut() = Some(ItemStack::new("minecraft:water_bucket", 1));
        // The template passes: the click goes on to the bucket's own use (UseItem).
        let (ends, _) = with_bundle(Bundle::default(), || game.remote_use_item_on(Hand::Main));
        assert!(!ends, "a bucket on a block that takes none goes on to UseItem");
        // A bundle names the block: the server moves the fluid, and this client ends the click.
        let declares = Bundle {
            fluid_interact_on_use: vec!["minecraft:carved_pumpkin".into()],
            ..Default::default()
        };
        let (ends, heard) = with_bundle(declares, || game.remote_use_item_on(Hand::Main));
        assert!(ends, "SUCCESS ends the click: no UseItem pours the bucket out");
        assert!(heard.calls.contains(&("fluid_interact_on_use", "minecraft:carved_pumpkin".into())));
        // Not a fluid container: the block isn't asked to take it.
        *game.held_mut() = Some(ItemStack::new("minecraft:stick", 1));
        let declares = Bundle {
            fluid_interact_on_use: vec!["minecraft:carved_pumpkin".into()],
            ..Default::default()
        };
        let (ends, heard) = with_bundle(declares, || game.remote_use_item_on(Hand::Main));
        assert!(!ends);
        assert!(heard.calls.is_empty(), "{:?}", heard.calls);
        let _ = state;
    });
}

#[test]
#[ignore = "daily"]
fn a_client_predicts_a_mod_containers_menu_on_a_template_that_opens_none() {
    with_client(|game| {
        let state = game.world.block(2, 4, 0);
        let container = |menu: Option<&str>| {
            FxHashMap::from_iter([(
                state.0,
                Arc::new(ModContainer {
                    slots: 9,
                    menu: menu.map(Into::into),
                    ..Default::default()
                }),
            )])
        };
        *game.held_mut() = None;
        game.world.block_entities.set_mod_containers(container(None));
        assert!(!game.remote_use_item_on(Hand::Main), "an empty hand on a block that does nothing");
        game.world
            .block_entities
            .set_mod_containers(container(Some("minecraft:generic_9x1")));
        assert!(game.remote_use_item_on(Hand::Main), "the menu's SUCCESS ends the click");
        // The off hand never reaches useWithoutItem, so no menu is predicted for it.
        assert!(!game.remote_use_item_on(Hand::Off));
    });
}
