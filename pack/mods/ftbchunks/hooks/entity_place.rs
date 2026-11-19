// An entity places a block (BlockEvent.EntityPlaceEvent): EDIT_BLOCK where it lands, for a player.
fn ftbchunks_entity_place(
    game: &mut crate::game::Game<'_>,
    placer: Option<mod_api::EntityRef>,
    pos: horizon_core::position::BlockPos,
    canceled: &mut bool,
) {
    crate::game::ftbchunks_world::place(game, placer, [pos.x, pos.y, pos.z], canceled);
}
