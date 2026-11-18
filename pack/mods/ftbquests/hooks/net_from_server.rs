// The team's progress, and the word to open the book.
fn ftbquests_net_from_server(
    game: &mut crate::game::Game<'_>,
    channel: &str,
    data: &[u8],
) {
    let _ = game;
    crate::game::ftbquests_book::from_server(channel, data);
}
