// A chemical offered to a Mekanism block (`IChemicalHandler.insertChemical`).
fn mek_chemical_handler_insert(
    game: &mut crate::game::Game<'_>,
    dimension: &str,
    pos: [i32; 3],
    face: Option<horizon_core::geometry::Direction>,
    chemical: &str,
    amount: i64,
    simulate: bool,
    inserted: &mut Option<i64>,
) {
    if inserted.is_none() {
        *inserted = crate::game::mek_chemicals::insert(game, dimension, pos, face, chemical, amount, simulate);
    }
}
