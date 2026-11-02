// A carried stack's tick: tasks that take items take them here.
fn ftbq_inventory_tick(game: &mut crate::game::Game<'_>, slot: i32) {
    crate::game::ftbq_world::inventory_tick(game, slot);
}
