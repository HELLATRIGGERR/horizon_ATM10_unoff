// A goo block or a Generator T1 gets its block entity (`EntityBlock.newBlockEntity`); the declaration owns its type id.
fn justdirethings_block_entity(block: &str, entity: &mut Option<horizon_core::nbt::Compound>) {
    crate::game::justdirethings_world::new_entity(block, entity);
}
