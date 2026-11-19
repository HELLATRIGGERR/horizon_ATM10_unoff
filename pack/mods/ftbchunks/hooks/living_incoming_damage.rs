// A player hurting a player: PvP in claims (`pvp_mode`, `allow_pvp`). The hook passes one call.
fn ftbchunks_living_incoming_damage(
    call: mod_api::__private::LivingIncomingDamageCall<'_>,
) -> mod_api::__private::LivingIncomingDamageCall<'_> {
    crate::game::ftbchunks_world::living_damage(call.entity, call.kind, call.source.causing, &mut *call.canceled);
    call
}
