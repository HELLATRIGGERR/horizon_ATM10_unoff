// A piston moves blocks: none crosses into another team's claim. The hook passes one call.
fn ftbchunks_piston_pre(call: mod_api::__private::PistonPreCall<'_>) -> mod_api::__private::PistonPreCall<'_> {
    crate::game::ftbchunks_world::piston_move(
        call.dimension.key().id(),
        call.piston,
        call.direction,
        call.push,
        call.destroy,
        &mut *call.canceled,
    );
    call
}
