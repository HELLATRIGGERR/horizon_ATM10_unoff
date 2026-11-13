// A player died: FTB Chunks keeps the spot for their death waypoint.
fn ftbchunks_player_died(game: &mut crate::game::Game<'_>, player: u128) {
    crate::game::ftbchunks_world::player_died(game, player);
}
