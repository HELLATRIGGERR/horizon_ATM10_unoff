//! Snapshot position-owned loot with clone's selected block entities.
use super::*;
use horizon_client::{containers::BlockEntity, save::PendingContainerLoot};
use horizon_core::nbt::Tag;

pub(super) struct CloneBlockPayload {
    entity: Option<BlockEntity>,
    name: Option<Tag>,
    pending: Option<PendingContainerLoot>,
    /// AE2's raw vanilla metadata of the entity (`remember_vanilla_metadata`).
    #[cfg(feature = "ae2-lab")]
    metadata: Option<Tag>,
}
impl Game<'_> {
    pub(super) fn clone_block_payload(&self, key: (i32, i32, i32)) -> CloneBlockPayload {
        CloneBlockPayload {
            entity: self.world.block_entities.get(&key).cloned(),
            name: self.world.block_names.get(&key).cloned(),
            pending: self.world.block_entity_extras(key).pending_loot,
            #[cfg(feature = "ae2-lab")]
            metadata: self.world.block_entities.vanilla_metadata(&key).cloned(),
        }
    }
    /// Replacements discard the previous marker before removal can access it.
    pub(super) fn discard_clone_loot(&mut self, pos: [i32; 3]) {
        crate::game::container_loot::take_block_pending(&mut self.world, (pos[0], pos[1], pos[2]));
    }
    pub(super) fn apply_clone_payload(&mut self, pos: [i32; 3], payload: CloneBlockPayload) {
        let key = (pos[0], pos[1], pos[2]);
        if let Some(entity) = payload.entity {
            self.world.block_entities.insert(key, entity);
            #[cfg(feature = "ae2-lab")]
            if let Some(raw) = payload.metadata.as_ref() {
                self.world.block_entities.remember_vanilla_metadata(key, raw);
            }
            match payload.name {
                Some(name) => self.world.block_names.insert(key, name),
                None => self.world.block_names.remove(&key),
            };
            self.world.with_chunk_extras(key, |extras| {
                extras.pending_loot.remove(&key);
                extras
                    .pending_loot
                    .extend(payload.pending.map(|pending| (key, pending)));
            });
            // Copying into the same block state can still replace the
            // entity's contents and components.
            self.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
        }
    }
}
