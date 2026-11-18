// Each frame on the client: Mekanism's chemicals go to the recipe viewers' ingredient lists in a world (JEI's list).
fn mek_viewer_frame(game: &mut crate::game::Game<'_>) {
    crate::game::mek_recipe_viewer::frame(game);
}
