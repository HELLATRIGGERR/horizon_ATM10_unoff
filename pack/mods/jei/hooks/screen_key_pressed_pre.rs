// Ctrl+O on a screen: show or hide the item list.
fn jei_screen_key_pressed_pre(game: &crate::game::Game<'_>, key: &str, repeat: bool, modifiers: u32, canceled: &mut bool) {
    crate::game::jei_world::key_pressed(game, key, repeat, modifiers, canceled);
}
