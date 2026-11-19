// An upgrade or an augment `use_on` put in ends the use as `Success` (`ItemUpgrade.useOn`, `interactAugment`).
fn ironfurnaces_use_on_answer(call: mod_api::__private::UseOnCall<'_>) -> mod_api::__private::UseOnCall<'_> {
    if crate::game::ironfurnaces_world::take_use() {
        *call.result = Some(mod_api::InteractionResult::Success);
    }
    call
}
