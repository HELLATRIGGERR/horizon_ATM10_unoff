// An MI machine gives no Forge Energy.
fn mi_fe_extract(entity: &mut horizon_client::containers::ModBlockEntity, given: &mut Option<i64>) {
    crate::game::mi_world::fe_extract(entity, given);
}
