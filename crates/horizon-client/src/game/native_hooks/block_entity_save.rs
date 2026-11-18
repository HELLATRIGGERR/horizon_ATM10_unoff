use super::*;

impl Game<'_> {
    /// A block entity of a mod is written for saving (autosave, unload, close), when `id` is yours
    /// (`BlockEntity.saveAdditional`): change `nbt`, the copy that goes to disk. The world's own
    /// entity keeps its compound. A chunk is saved only when it changed:
    /// `native_block_entity_changed` says so for what's kept outside `nbt`.
    pub(in crate::game) fn native_block_entity_save(
        &self,
        dimension: &str,
        pos: [i32; 3],
        id: &str,
        nbt: &mut Compound,
    ) {
        self.fire_block_entity_save(dimension, pos, id, nbt);
    }
}
