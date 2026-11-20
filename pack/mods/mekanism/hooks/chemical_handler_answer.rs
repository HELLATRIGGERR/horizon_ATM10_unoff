// What `chemical_handler_insert` answered goes into the call.
fn mek_chemical_handler_answer(
    call: mod_api::__private::ChemicalHandlerInsertCall<'_>,
) -> mod_api::__private::ChemicalHandlerInsertCall<'_> {
    if let Some(answer) = crate::game::mek_chemicals::take_answer()
        && call.inserted.is_none()
    {
        *call.inserted = Some(answer);
    }
    call
}
