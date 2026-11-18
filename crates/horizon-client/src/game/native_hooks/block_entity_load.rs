use super::*;

impl Game<'_> {
    /// A block entity of a mod entered the world: placed, or loaded with its chunk
    /// (`BlockEntity.onLoad`). Told on the server at the start of its level's next running block
    /// entity pass, before any of them tick, oldest first, after `block_entity_unload` of those
    /// that left with their block since. Check `entity.id`; it is out of the world during the call,
    /// as in `native_block_entity_tick`, so an old save's data can be changed here, and `dimension`
    /// is the level in place. One that came and left with its chunk before then isn't told at all.
    pub(in crate::game) fn native_block_entity_load(
        &mut self,
        dimension: &str,
        pos: [i32; 3],
        entity: &mut ModBlockEntity,
    ) {
        self.fire_block_entity_load(dimension, pos, entity);
    }
}
