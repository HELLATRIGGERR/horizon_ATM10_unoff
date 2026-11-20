// A use `use_item` claimed ends as `Success`.
fn mek_use_item_answer(call: mod_api::__private::UseItemCall<'_>) -> mod_api::__private::UseItemCall<'_> {
    if crate::game::mek_radiation::take_use_claimed() && call.result.is_none() {
        *call.result = Some(mod_api::InteractionResult::Success);
    }
    call
}
