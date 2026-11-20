// A chemical taken from a Mekanism block (`IChemicalHandler.extractChemical(stack, action)`): the hook passes one call,
// answered by `chemical_handler_extract_answer`.
fn mek_chemical_handler_extract(game: &mut crate::game::Game<'_>, call: &mod_api::__private::ChemicalHandlerExtractCall<'_>) {
    if call.extracted.is_none() {
        let answer = crate::game::mek_chemicals::extract(game, call.dimension, call.pos, call.face, call.chemical, call.amount, call.simulate);
        crate::game::mek_chemicals::set_answer(answer);
    }
}
