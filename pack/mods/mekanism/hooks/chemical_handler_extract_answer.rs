// What `chemical_handler_extract` answered goes into the call.
fn mek_chemical_handler_extract_answer(
    call: mod_api::__private::ChemicalHandlerExtractCall<'_>,
) -> mod_api::__private::ChemicalHandlerExtractCall<'_> {
    if let Some(answer) = crate::game::mek_chemicals::take_answer()
        && call.extracted.is_none()
    {
        *call.extracted = Some(answer);
    }
    call
}
