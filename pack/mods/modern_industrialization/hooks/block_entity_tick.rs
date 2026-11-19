// A server tick of an MI machine (`CrafterComponent.tickRecipe`).
fn mi_block_entity_tick(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut horizon_client::containers::ModBlockEntity,
) {
    crate::game::mi_world::tick(game, pos, entity);
}
