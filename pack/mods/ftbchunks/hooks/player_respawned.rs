// A player respawned: their client hears where they died (`player_death`).
fn ftbchunks_player_respawned(game: &mut crate::game::Game<'_>, player: u128) {
    crate::game::ftbchunks_world::player_respawned(game, player);
}
