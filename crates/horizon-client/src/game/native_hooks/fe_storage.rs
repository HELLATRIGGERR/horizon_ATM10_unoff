use super::*;

impl Game<'_> {
    /// The Forge Energy of the block entity at `pos` in `dimension`, a mod's, through `face`, as
    /// NeoForge's `IEnergyStorage` reads it (`getEnergyStored`, `getMaxEnergyStored`, `canReceive`,
    /// `canExtract`). `face` is the side of this block the caller touches (a cable east of it asks
    /// with `East`); none is the block itself, as a null side. Set `info` when `entity.id` is yours
    /// and `info` is none yet; leave it none on a side without energy, so cables don't connect
    /// there. Amounts are FE. `wide`: the caller reads 64-bit amounts (Flux Networks' long FE);
    /// otherwise the engine saturates yours at `i32::MAX`. Asked on the server, often, from inside
    /// other blocks' transfers and maybe while another level is in place: answer from `entity` and
    /// your own state, not from the world.
    pub(in crate::game) fn native_fe_storage(
        &self,
        dimension: &str,
        pos: [i32; 3],
        face: Option<Direction>,
        entity: &ModBlockEntity,
        wide: bool,
        info: &mut Option<mod_api::NativeFeInfo>,
    ) {
        self.fire_fe_storage(dimension, pos, face, entity, wide, info);
    }
}
