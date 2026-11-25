//! A machine's fake player (`native_fake_use.rs`): a right click that fills a bucket from the
//! water in front, a left click that hits a mob and breaks what one hit breaks (and digs the
//! rest when told how long), a use held and let go, each machine's held use its own, and no
//! click where no server runs the level. Invented fixtures: a stone floor, a fake standing at the
//! face of the block it clicks.
use crate::game::game_tests::tests::server_game;
use crate::game::native_fake_use::{FakeClick, FakeTarget, FakeUse};
use crate::game::*;
use horizon_client::{command::args::GameMode, items::ItemStack};
use mod_api::{ActionOutcome, ActionRefused};
use std::sync::Arc;

/// The block the fake clicks, and the machine above it.
const FRONT: [i32; 3] = [1, 64, 0];
const MACHINE: [i32; 3] = [1, 65, 0];

/// A survival server with a guest, a stone floor at y 63 and nothing else.
fn game_with<'a>(resources: &'a mut Option<Arc<Resources>>) -> Game<'a> {
    let mut game = server_game(resources);
    crate::game::bossbars::audience_tests::add_real_and_fake(&mut game);
    game.options.dedicated = Some(Vec::new());
    game.set_mode(GameMode::Survival);
    for (x, z) in (-1..=1).flat_map(|x| (-1..=1).map(move |z| (x, z))) {
        let mut chunk = horizon_client::chunk::Chunk::empty(x, z);
        chunk.lit = true;
        game.world.chunks.insert((x, z), Arc::new(chunk));
    }
    let stone = state(&game, "minecraft:stone");
    for x in -3..=3 {
        for z in -3..=3 {
            game.world.set(x, 63, z, stone).unwrap();
        }
    }
    game
}

fn state(game: &Game<'_>, name: &str) -> horizon_world::registry::BlockStateId {
    game.pack.assets.registry.default_state(name).unwrap()
}

fn name_at(game: &Game<'_>, pos: [i32; 3]) -> String {
    game.res
        .facts()
        .get(game.world.block(pos[0], pos[1], pos[2]))
        .name
        .to_string()
}

/// A click of the fake of `machine` looking down on `FRONT` from just inside its top
/// (`setupFakePlayerForUse` for the direction down), holding `stack`.
fn click(
    machine: [i32; 3],
    stack: Option<ItemStack>,
    target: FakeTarget,
    click: FakeClick,
) -> FakeUse<'static> {
    FakeUse {
        uuid: 0x4191a6f5_37fe_45d9_8ba3_4549be778e54,
        name: "[JustDiresFake]",
        machine,
        eye: [1.5, 64.95, 0.5],
        yaw: 270.0,
        pitch: 90.0,
        sneaking: false,
        charged: true,
        stack,
        target,
        click,
    }
}

/// The top of `FRONT`, hit at `y` (a fluid's surface is lower than a block's).
fn top(y: f64) -> FakeTarget {
    FakeTarget::Block {
        pos: FRONT,
        face: [0, 1, 0],
        point: [1.5, f64::from(FRONT[1]) + y, 0.5],
        inside: false,
    }
}

#[test]
#[ignore = "daily"]
fn a_fakes_right_click_fills_a_bucket_from_the_water_in_front() {
    let mut resources = None;
    let mut game = game_with(&mut resources);
    let water = state(&game, "minecraft:water");
    game.world.set(FRONT[0], FRONT[1], FRONT[2], water).unwrap();
    let used = game.native_fake_use(click(
        MACHINE,
        Some(ItemStack::new("minecraft:bucket", 1)),
        top(8.0 / 9.0),
        FakeClick::Use,
    ));
    assert_eq!(used.outcome, Ok(ActionOutcome::Consumed), "{used:?}");
    assert_eq!(
        used.stack,
        Some(ItemStack::new("minecraft:water_bucket", 1))
    );
    assert!(used.other.is_empty() && !used.holding);
    assert_eq!(
        name_at(&game, FRONT),
        "minecraft:air",
        "the water was taken"
    );
    // The fake's own state is back: nothing of the click stays in its hand.
    let host = game.lan.as_ref().unwrap();
    let fake = host
        .hub
        .guests
        .iter()
        .find(|g| g.uuid == 0x4191a6f5_37fe_45d9_8ba3_4549be778e54)
        .unwrap();
    assert!(fake.inventory.iter().all(Option::is_none));
}

#[test]
#[ignore = "daily"]
fn a_fakes_left_click_hits_a_mob_and_breaks_what_one_hit_breaks() {
    let mut resources = None;
    let mut game = game_with(&mut resources);
    let kind = game
        .res
        .mob_types()
        .get("minecraft:zombie")
        .unwrap()
        .clone();
    game.mobs.push(horizon_client::mobs::Mob::new(
        5000,
        kind,
        horizon_core::geometry::Vec3::new(1.5, 64.0, 0.5),
        0.0,
    ));
    let health = |game: &Game<'_>| game.mobs.iter().find(|m| m.id == 5000).unwrap().health;
    let full = health(&game);
    let sword = || Some(ItemStack::new("minecraft:diamond_sword", 1));
    let mob = FakeTarget::Entity {
        id: 5000,
        location: [0.0, 1.0, 0.0],
    };
    let used = game.native_fake_use(click(MACHINE, sword(), mob, FakeClick::Attack { dug: 0 }));
    assert_eq!(used.outcome, Ok(ActionOutcome::Consumed), "{used:?}");
    // A charged diamond sword: 7 less the zombie's 2 armour (1 - (2 - 7 / 4) / 25 of it).
    let dealt = full - health(&game);
    assert!(dealt > 6.0, "a full-strength blow, {dealt}");
    game.mobs.clear();

    // Short grass breaks at the first hit; stone stays, cracked, until told it was dug long enough.
    let grass = state(&game, "minecraft:short_grass");
    game.world.set(FRONT[0], FRONT[1], FRONT[2], grass).unwrap();
    let used = game.native_fake_use(click(
        MACHINE,
        sword(),
        top(0.5),
        FakeClick::Attack { dug: 0 },
    ));
    assert_eq!(used.outcome, Ok(ActionOutcome::Consumed), "{used:?}");
    assert_eq!(name_at(&game, FRONT), "minecraft:air");
    let stone = state(&game, "minecraft:stone");
    game.world.set(FRONT[0], FRONT[1], FRONT[2], stone).unwrap();
    let used = game.native_fake_use(click(MACHINE, None, top(1.0), FakeClick::Attack { dug: 0 }));
    assert_eq!(used.outcome, Ok(ActionOutcome::Consumed), "{used:?}");
    assert_eq!(
        name_at(&game, FRONT),
        "minecraft:stone",
        "a fist doesn't break stone at once"
    );
    let used = game.native_fake_use(click(
        MACHINE,
        None,
        top(1.0),
        FakeClick::Attack { dug: 1000 },
    ));
    assert_eq!(used.outcome, Ok(ActionOutcome::Consumed), "{used:?}");
    assert_eq!(name_at(&game, FRONT), "minecraft:air", "dug long enough");
}

#[test]
#[ignore = "daily"]
fn a_fakes_held_use_is_let_go_after_its_ticks_and_each_machine_holds_its_own() {
    let mut resources = None;
    let mut game = game_with(&mut resources);
    let bow = || Some(ItemStack::new("minecraft:bow", 1));
    let other = [5, 65, 0];
    let hold = |game: &mut Game<'_>, machine| {
        let used =
            game.native_fake_use(click(machine, bow(), top(1.0), FakeClick::Hold { max: 3 }));
        assert_eq!(used.outcome, Ok(ActionOutcome::Consumed), "{used:?}");
        assert_eq!(used.stack, bow());
        used.holding
    };
    assert!(hold(&mut game, MACHINE), "held 1 of 3 ticks");
    assert!(
        hold(&mut game, other),
        "the other machine's use starts on its own"
    );
    assert!(hold(&mut game, MACHINE), "held 2 of 3");
    assert!(!hold(&mut game, MACHINE), "let go at 3");
    assert!(hold(&mut game, other), "the other machine at 2");
    assert!(hold(&mut game, MACHINE), "a new use starts");
    // Any other click lets a held use go.
    let used = game.native_fake_use(click(
        MACHINE,
        bow(),
        FakeTarget::Air,
        FakeClick::Attack { dug: 0 },
    ));
    assert_eq!(used.outcome, Ok(ActionOutcome::Passed));
    assert!(hold(&mut game, MACHINE) && hold(&mut game, MACHINE) && !hold(&mut game, MACHINE));
}

#[test]
#[ignore = "daily"]
fn no_fake_clicks_where_no_server_runs_the_level() {
    let mut resources = None;
    let mut game = server_game(&mut resources);
    let stack = Some(ItemStack::new("minecraft:bucket", 3));
    let used = game.native_fake_use(click(MACHINE, stack.clone(), top(1.0), FakeClick::Use));
    assert_eq!(used.outcome, Err(ActionRefused::Stale));
    assert_eq!(used.stack, stack, "the stack comes back as it went");
    let mut game = game_with(&mut resources);
    let mut bad = click(MACHINE, stack.clone(), top(1.0), FakeClick::Use);
    bad.name = "a name longer than sixteen";
    let used = game.native_fake_use(bad);
    assert_eq!(
        (used.outcome, used.stack),
        (Err(ActionRefused::Stale), stack)
    );
}
