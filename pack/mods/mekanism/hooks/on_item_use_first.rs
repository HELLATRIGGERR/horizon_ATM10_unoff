// A player uses an item on a block (`onItemUseFirst`: before the block, sneaking or not): a tier installer turns a machine
// into a factory, a factory into the next tier;
// a configurator configures, empties, turns or takes up a Mekanism block; a wind or advanced solar generator isn't
// placed where its bounding blocks don't fit; a turbine blade goes onto a turbine rotor, or comes off it. A claimed
// use ends as `Success`; the click's stack is the API's.
fn mek_on_item_use_first(
    game: &mut crate::game::Game<'_>,
    click: &mod_api::BlockClick<'_>,
    answer: &mut Option<mod_api::InteractionResult>,
) {
    if answer.is_some() {
        return;
    }
    let Some(held) = click.stack else { return };
    let Ok(stack) = crate::game::api_values::TryIntoEngine::try_into_engine(held, game.items()) else { return };
    let (pos, face) = (click.pos, click.hit.face);
    let mut claimed = false;
    crate::game::mek_factories::use_installer(game, &stack, pos, &mut claimed);
    crate::game::mek_configurator::use_on(game, &stack, pos, face, &mut claimed);
    crate::game::mek_gen2::use_on(game, &stack, pos, face, &mut claimed);
    crate::game::mek_turbine::use_on(game, &stack, pos, &mut claimed);
    if claimed {
        *answer = Some(mod_api::InteractionResult::Success);
    }
}
