// A chemical offered to a Mekanism block (`IChemicalHandler.insertChemical`): the hook passes one call, answered by
// `chemical_handler_answer`.
fn mek_chemical_handler_insert(game: &mut crate::game::Game<'_>, call: &mod_api::__private::ChemicalHandlerInsertCall<'_>) {
    if call.inserted.is_none() {
        let answer = crate::game::mek_chemicals::insert(game, call.dimension, call.pos, call.face, call.chemical, call.amount, call.simulate);
        crate::game::mek_chemicals::set_answer(answer);
    }
}
