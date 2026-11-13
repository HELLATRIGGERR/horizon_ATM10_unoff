// A player uses an item on a block (a placement): EDIT_BLOCK where it lands.
fn ftbchunks_use_item_on(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    face: [i32; 3],
    claimed: &mut bool,
) {
    crate::game::ftbchunks_world::use_item_on(game, pos, face, claimed);
}
