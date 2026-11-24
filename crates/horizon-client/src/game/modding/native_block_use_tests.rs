//! A mod block's use for the native bundles (`items/use_hand.rs`, `native_block_use.rs`): a bundle's
//! `use_item_on` and `use_without_item` heard with no trait mod's part on the block, in
//! `ServerPlayerGameMode.useItemOn`'s order; a fluid container on a block that takes them
//! (`fluid_interact_on_use`); the menu of a mod block entity's container on a block whose
//! template opens none; and the recipes of a server's `Game`, which has no player.
use crate::game::fork_compat::test_bundle::{Bundle, with_bundle};
use crate::game::game_tests::tests::server_game;
use crate::game::*;
use horizon_client::{
    command::args::GameMode,
    containers::{BlockEntity, ModBlockEntity, ModBlockEntityFields, ModContainer, ModFluidTanks},
    fast_hash::FxHashMap,
    held::Source,
    items::ItemStack,
};
use mod_api::InteractionResult as Answer;
use std::sync::Arc;
use winit::keyboard::KeyCode;

const AT: [i32; 3] = [2, 4, 0];
/// The block a use of the south face places against.
const FRONT: [i32; 3] = [2, 4, 1];

/// A click on the south face of the block at `AT`.
fn hit() -> Hit {
    Hit {
        pos: AT,
        face: [0, 0, 1],
        point: [2.5, 4.5, 1.0],
        inside: false,
    }
}

/// A survival server game with `block` at `AT` and nothing in front of it.
fn game_with<'a>(resources: &'a mut Option<Arc<Resources>>, block: &str) -> Game<'a> {
    let mut game = server_game(resources);
    game.options.dedicated = None;
    game.set_mode(GameMode::Survival);
    let state = game.pack.assets.registry.default_state(block).unwrap();
    game.world.set(AT[0], AT[1], AT[2], state).unwrap();
    game
}

fn name_at(game: &Game<'_>, pos: [i32; 3]) -> String {
    game.res
        .facts()
        .get(game.world.block(pos[0], pos[1], pos[2]))
        .name
        .to_string()
}

/// The click starts again: no menu, nothing in front, `held` in the main hand.
fn again(game: &mut Game<'_>, held: Option<ItemStack>) {
    game.container = None;
    let air = game.pack.assets.registry.default_state("minecraft:air").unwrap();
    game.world.set(FRONT[0], FRONT[1], FRONT[2], air).unwrap();
    *game.held_mut() = held;
}

/// A mod block entity of `block` at `AT`.
fn mod_entity(game: &mut Game<'_>, block: &str) {
    game.world.block_entities.insert(
        (AT[0], AT[1], AT[2]),
        BlockEntity::Mod(Box::new(ModBlockEntity::new(
            block.into(),
            ModBlockEntityFields::default(),
        ))),
    );
}

#[test]
#[ignore = "daily"]
fn a_bundles_block_use_hooks_answer_in_vanillas_order() {
    let mut resources = None;
    let mut game = game_with(&mut resources, "minecraft:furnace");
    let furnace = || "minecraft:furnace".to_owned();
    let stone = || Some(ItemStack::new("minecraft:stone", 4));
    let key = Some((AT[0], AT[1], AT[2]));

    // No bundle: the furnace's own useWithoutItem opens its menu; the stone is not placed.
    again(&mut game, stone());
    assert!(game.use_hand(Some(&hit())).unwrap());
    assert_eq!(game.container, key, "the template opens its menu");
    assert_eq!(name_at(&game, FRONT), "minecraft:air");

    // A bundle hears every block's use without a part on it: useWithoutItem passes (Just Dire
    // Things' machines with the Ferricore Wrench), so the item's useOn gets the click.
    again(&mut game, stone());
    let passes = Bundle {
        use_without_item: [(furnace(), Answer::Pass)].into(),
        ..Default::default()
    };
    let (used, heard) = with_bundle(passes, || game.use_hand(Some(&hit())).unwrap());
    assert!(used);
    assert_eq!(game.container, None, "no menu");
    assert_eq!(name_at(&game, FRONT), "minecraft:stone", "the stone's useOn placed it");
    assert_eq!(
        heard.calls,
        [
            ("use_item_on", furnace()),
            ("fluid_interact_on_use", furnace()),
            ("use_without_item", furnace()),
        ]
    );

    // useItemOn answers (the Paradox Machine taking a bucket): the click ends there.
    again(&mut game, stone());
    let takes = Bundle {
        use_item_on: [(furnace(), Answer::Success)].into(),
        use_without_item: [(furnace(), Answer::Pass)].into(),
        ..Default::default()
    };
    let (used, heard) = with_bundle(takes, || game.use_hand(Some(&hit())).unwrap());
    assert!(used);
    assert_eq!(game.container, None);
    assert_eq!(name_at(&game, FRONT), "minecraft:air");
    assert_eq!(heard.calls, [("use_item_on", furnace())]);

    // TRY_WITH_EMPTY_HAND, then useWithoutItem's own answer replaces the template's menu.
    again(&mut game, stone());
    let both = Bundle {
        use_item_on: [(furnace(), Answer::TryWithEmptyHand)].into(),
        use_without_item: [(furnace(), Answer::Success)].into(),
        ..Default::default()
    };
    let (used, heard) = with_bundle(both, || game.use_hand(Some(&hit())).unwrap());
    assert!(used);
    assert_eq!(game.container, None);
    assert_eq!(name_at(&game, FRONT), "minecraft:air");
    assert_eq!(
        heard.calls,
        [("use_item_on", furnace()), ("use_without_item", furnace())]
    );

    // Sneaking with something in hand skips the block's uses: neither hook is asked.
    again(&mut game, stone());
    game.keys.press(KeyCode::ShiftLeft, Source::Keyboard);
    let takes = Bundle {
        use_item_on: [(furnace(), Answer::Success)].into(),
        ..Default::default()
    };
    let (used, heard) = with_bundle(takes, || game.use_hand(Some(&hit())).unwrap());
    game.keys.release(&KeyCode::ShiftLeft, Source::Keyboard);
    assert!(used);
    assert!(heard.calls.is_empty(), "{:?}", heard.calls);
    assert_eq!(name_at(&game, FRONT), "minecraft:stone");
}

#[test]
#[ignore = "daily"]
fn a_fluid_container_fills_a_block_that_takes_containers_on_use() {
    let mut resources = None;
    let block = "minecraft:carved_pumpkin";
    let mut game = game_with(&mut resources, block);
    let state = game.world.block(AT[0], AT[1], AT[2]);
    let mut tanks = ModFluidTanks::default();
    tanks.tanks = vec![4000];
    tanks.inputs = vec![0];
    tanks.outputs = vec![0];
    tanks.faces = std::array::from_fn(|_| vec![0]);
    game.world
        .block_entities
        .set_mod_fluids(FxHashMap::from_iter([(state.0, Arc::new(tanks))]));
    mod_entity(&mut game, block);
    let saved = |game: &Game<'_>| match game.world.block_entities.get(&(AT[0], AT[1], AT[2])) {
        Some(BlockEntity::Mod(m)) => format!("{:?}", m.nbt),
        _ => String::new(),
    };
    // The saved compound's water, as its Debug text spells a string.
    let water = format!(
        "{:?}",
        horizon_core::nbt::Tag::String(horizon_core::nbt::JavaString::from("minecraft:water"))
    );
    let holds_water = |game: &Game<'_>| saved(game).contains(&water);
    let declares = || Bundle {
        fluid_interact_on_use: vec![block.to_owned()],
        ..Default::default()
    };

    // Only a block a bundle declares takes containers.
    assert!(!with_bundle(Bundle::default(), || game.fluid_interact_on_use(state)).0);
    assert!(with_bundle(declares(), || game.fluid_interact_on_use(state)).0);
    assert!(!holds_water(&game), "{}", saved(&game));

    // Declared: the bucket empties into the tank through the side clicked, and the empty
    // bucket comes back in survival (FluidUtil.interactWithFluidHandler).
    again(&mut game, Some(ItemStack::new("minecraft:water_bucket", 1)));
    let (used, heard) = with_bundle(declares(), || game.use_hand(Some(&hit())).unwrap());
    assert!(used);
    assert!(heard.calls.contains(&("fluid_interact_on_use", block.to_owned())));
    assert!(holds_water(&game), "{}", saved(&game));
    assert!(saved(&game).contains("Int(1000)"), "a bucket: {}", saved(&game));
    assert_eq!(
        game.held().map(|s| (s.item.to_string(), s.count)),
        Some(("minecraft:bucket".into(), 1))
    );
    assert_eq!(name_at(&game, FRONT), "minecraft:air", "no water poured in front");

    // The empty bucket takes the water back out; in creative the full one stays a full one.
    again(&mut game, Some(ItemStack::new("minecraft:bucket", 1)));
    let (used, _) = with_bundle(declares(), || game.use_hand(Some(&hit())).unwrap());
    assert!(used);
    assert_eq!(
        game.held().map(|s| s.item.to_string()).as_deref(),
        Some("minecraft:water_bucket")
    );
    game.set_mode(GameMode::Creative);
    again(&mut game, Some(ItemStack::new("minecraft:water_bucket", 1)));
    let (used, _) = with_bundle(declares(), || game.use_hand(Some(&hit())).unwrap());
    assert!(used);
    assert!(holds_water(&game));
    assert_eq!(
        game.held().map(|s| s.item.to_string()).as_deref(),
        Some("minecraft:water_bucket"),
        "a creative player keeps the container"
    );
}

#[test]
#[ignore = "daily"]
fn a_mod_container_opens_its_menu_on_a_template_that_opens_none() {
    let mut resources = None;
    let block = "minecraft:carved_pumpkin";
    let mut game = game_with(&mut resources, block);
    let state = game.world.block(AT[0], AT[1], AT[2]);
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
    mod_entity(&mut game, block);
    let logs = || Some(ItemStack::new("minecraft:oak_log", 4));

    // The container has a menu: the click opens it, and the logs in hand are not placed.
    game.world
        .block_entities
        .set_mod_containers(container(Some("minecraft:generic_9x1")));
    again(&mut game, logs());
    assert!(game.use_hand(Some(&hit())).unwrap());
    assert_eq!(game.container, Some((AT[0], AT[1], AT[2])), "the menu opens");
    assert_eq!(name_at(&game, FRONT), "minecraft:air", "no log placed");

    // No menu: the template passes and the log is placed.
    game.world.block_entities.set_mod_containers(container(None));
    again(&mut game, logs());
    assert!(game.use_hand(Some(&hit())).unwrap());
    assert_eq!(game.container, None);
    assert_eq!(name_at(&game, FRONT), "minecraft:oak_log");

    // A menu, but sneaking with the logs: the block's uses are skipped and the log is placed.
    game.world
        .block_entities
        .set_mod_containers(container(Some("minecraft:generic_9x1")));
    again(&mut game, logs());
    game.keys.press(KeyCode::ShiftLeft, Source::Keyboard);
    assert!(game.use_hand(Some(&hit())).unwrap());
    game.keys.release(&KeyCode::ShiftLeft, Source::Keyboard);
    assert_eq!(game.container, None);
    assert_eq!(name_at(&game, FRONT), "minecraft:oak_log");
}

#[test]
#[ignore = "daily"]
fn a_server_reads_its_recipes_without_a_player() {
    let mut resources = None;
    let mut game = server_game(&mut resources);
    assert!(game.player.is_none(), "a server's Game has no player of its own");
    assert!(game.native_recipes().is_none(), "native_recipes still asks for one");
    let recipes = game.native_world_recipes().expect("the server's recipes");
    assert!(!recipes.recipes.is_empty());
    let table = recipes
        .items
        .iter()
        .position(|item| item.id == "minecraft:crafting_table")
        .expect("the crafting table is an item") as u32;
    assert!(
        recipes
            .recipes
            .iter()
            .any(|r| r.results.iter().any(|s| s.item == table)),
        "its recipe is read"
    );
    // The same tables again.
    assert!(Arc::ptr_eq(&recipes, &game.native_world_recipes().unwrap()));
}
