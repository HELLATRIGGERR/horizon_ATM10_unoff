// A revived goo ends the use as `Success`.
fn justdirethings_use_on_answer(call: mod_api::__private::UseOnCall<'_>) -> mod_api::__private::UseOnCall<'_> {
    if crate::game::justdirethings_world::take_use() {
        *call.result = Some(mod_api::InteractionResult::Success);
    }
    call
}
