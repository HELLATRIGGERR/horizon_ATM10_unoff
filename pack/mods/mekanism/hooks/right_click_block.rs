// A player right clicks a block, before the block acts: a configurator in its rotate mode turns a Mekanism block;
// a multiblock's block opens its screen only when formed; sneaking with an empty hand takes a blade off a turbine rotor.
// A click Mekanism took ends as `Success`.
fn mek_right_click_block(
    game: &mut crate::game::Game<'_>,
    click: &mod_api::BlockClick<'_>,
    result: &mut mod_api::InteractionResult,
    canceled: &mut bool,
) {
    let was = *canceled;
    crate::game::mek_configurator::right_click(game, click.pos, canceled);
    crate::game::mek_multiblock::right_click(game, click.pos, canceled);
    crate::game::mek_turbine::right_click(game, click.pos, canceled);
    if *canceled && !was {
        *result = mod_api::InteractionResult::Success;
    }
}
