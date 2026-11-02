// A player breaks a block: a cube's drop will keep its energy.
fn mek_destroy_block(game: &mut crate::game::Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if !*canceled {
        crate::game::mek_world::breaking(game, pos);
    }
}
