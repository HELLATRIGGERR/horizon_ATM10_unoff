// A player's tick ends: on the server their radiation dose takes the level they stand in, decays and does its harm.
fn mek_player_tick_post(game: &mut crate::game::Game<'_>, call: &mod_api::__private::PlayerTickPostCall) {
    crate::game::mek_radiation::player_tick(game, call.player);
}
