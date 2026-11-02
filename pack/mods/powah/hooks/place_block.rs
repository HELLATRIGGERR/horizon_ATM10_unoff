// A Furnator placed from its item takes the energy back.
fn powah_place_block(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    pos: [i32; 3],
) {
    crate::game::powah_world::placed(game, stack, pos);
}
