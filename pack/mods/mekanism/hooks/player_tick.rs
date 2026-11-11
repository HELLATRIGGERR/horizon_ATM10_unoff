// A server player's tick ends: their radiation dose takes the level they stand in, decays and does its harm.
fn mek_player_tick(game: &mut crate::game::Game<'_>) {
    crate::game::mek_radiation::player_tick(game);
}
