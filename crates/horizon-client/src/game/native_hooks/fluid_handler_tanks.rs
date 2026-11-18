use super::*;

impl Game<'_> {
    /// The fluid handler of the block entity at `pos` in `dimension`, a mod's, through `face`, as
    /// NeoForge's `IFluidHandler` reads it (`getTanks`, `getFluidInTank`, `getTankCapacity`).
    /// `face` is the side of this block the caller touches, as in `fe_storage`; none is the block
    /// itself, a null side. Set `tanks` when `entity.id` is yours and `tanks` is none yet; leave it
    /// none on a side without a handler (`getCapability` answering null), so pipes and buses see
    /// nothing there. Asked on the server, often, from inside other blocks' transfers and maybe
    /// while another level is in place: answer from `entity` and your own state, not from the
    /// world.
    pub(in crate::game) fn native_fluid_handler_tanks(
        &self,
        dimension: &str,
        pos: [i32; 3],
        face: Option<Direction>,
        entity: &ModBlockEntity,
        tanks: &mut Option<Vec<mod_api::NativeFluidTankInfo>>,
    ) {
        self.fire_fluid_handler_tanks(dimension, pos, face, entity, tanks);
    }
}
