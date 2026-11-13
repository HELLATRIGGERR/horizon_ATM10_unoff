// A click on JEI's recipe window: a stack's recipes or uses, a tab, an arrow; nothing under it reaches the screen.
fn jei_screen_mouse_button_pressed_pre(
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    crate::game::jei_window::pressed(screen, button, canceled);
}
