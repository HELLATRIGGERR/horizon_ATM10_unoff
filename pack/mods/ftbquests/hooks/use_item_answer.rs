// The book's use ends as `Success` (`QuestBookItem.use`).
fn ftbquests_use_item_answer(call: mod_api::__private::UseItemCall<'_>) -> mod_api::__private::UseItemCall<'_> {
    if crate::game::ftbquests_world::take_book_use() {
        *call.result = Some(mod_api::InteractionResult::Success);
    }
    call
}
