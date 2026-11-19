// The use that `use_item` refused ends as `Fail`.
fn ftbchunks_use_item_answer(call: mod_api::__private::UseItemCall<'_>) -> mod_api::__private::UseItemCall<'_> {
    if crate::game::ftbchunks_world::take_item_refusal() {
        *call.result = Some(mod_api::InteractionResult::Fail);
    }
    call
}
