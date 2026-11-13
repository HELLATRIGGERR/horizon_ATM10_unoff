// A click on JEI's settings page.
fn jei_settings_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::jei_world::settings_click(game, mod_id, button);
}
