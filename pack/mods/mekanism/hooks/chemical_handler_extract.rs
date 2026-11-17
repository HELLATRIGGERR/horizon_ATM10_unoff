// A chemical taken from a Mekanism block (`IChemicalHandler.extractChemical(stack, action)`).
fn mek_chemical_handler_extract(
    game: &mut crate::game::Game<'_>,
    dimension: &str,
    pos: [i32; 3],
    face: Option<horizon_core::geometry::Direction>,
    chemical: &str,
    amount: i64,
    simulate: bool,
    extracted: &mut Option<i64>,
) {
    if extracted.is_none() {
        *extracted = crate::game::mek_chemicals::extract(game, dimension, pos, face, chemical, amount, simulate);
    }
}
