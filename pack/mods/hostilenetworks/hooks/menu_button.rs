// A button of a machine's screen, on the server.
fn hostilenetworks_menu_button(
    game: &mut crate::game::Game<'_>,
    entity: &mut horizon_client::containers::ModBlockEntity,
    id: i32,
) {
    crate::game::hostilenetworks_world::button(game, entity, id);
}
