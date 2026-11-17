// A player hurting a player: PvP in claims (`pvp_mode`, `allow_pvp`).
fn ftbchunks_living_incoming_damage(entity: i32, kind: &str, causing: i32, canceled: &mut bool) {
    crate::game::ftbchunks_world::living_damage(entity, kind, causing, canceled);
}
