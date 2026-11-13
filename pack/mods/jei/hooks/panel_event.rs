// The player's search or click on the panel, when it is JEI's.
fn jei_panel_event(game: &mut crate::game::Game<'_>, mod_id: &str, event: &crate::game::native_hooks::NativePanelEvent) {
    if mod_id == crate::game::jei_world::ID && game.native_on(mod_id) {
        crate::game::jei_world::event(game, event);
    }
}
