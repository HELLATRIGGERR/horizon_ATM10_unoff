// JEI's settings page: cheat mode, the overlay, the search modes.
fn jei_settings(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    crate::game::jei_world::settings(game, mod_id, buttons);
}
