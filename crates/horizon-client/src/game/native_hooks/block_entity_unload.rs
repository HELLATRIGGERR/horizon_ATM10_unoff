use super::*;

impl Game<'_> {
    /// A block entity of a mod is leaving the world. `removed`: its block was broken or replaced
    /// (`setRemoved`), told as `block_entity_load` is, with `entity` as it last was. Otherwise its
    /// chunk is unloading (`onChunkUnloaded`), told at once, with the chunk's last save snapshot
    /// taken already and `dimension` the level in place. Closing a world tells nothing:
    /// `native_world_lifecycle` does.
    pub(in crate::game) fn native_block_entity_unload(
        &mut self,
        dimension: &str,
        pos: [i32; 3],
        entity: &ModBlockEntity,
        removed: bool,
    ) {
        self.fire_block_entity_unload(dimension, pos, entity, removed);
    }
}
