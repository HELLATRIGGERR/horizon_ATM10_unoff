use super::*;

impl Game<'_> {
    /// `IEnergyStorage.receiveEnergy` on the block entity at `pos`, a mod's: `max` FE (1 to
    /// `i32::MAX` unless `wide`) offered through `face`, as in `fe_storage`. Set `accepted` (0 to
    /// `max`) when `entity.id` is yours and `accepted` is none yet; with `simulate`, answer what
    /// you would take and change nothing. The entity is out of the world during the call, as in
    /// `native_block_entity_tick`; the engine puts it back and, when it took something for real,
    /// marks it changed (`setChanged`). A forwarder that holds nothing (a cable) may hand the
    /// energy on with `native_fe_offer` from here: the same block, side and direction asked again
    /// inside its own call gets 0, and so does a chain 16 calls deep.
    pub(in crate::game) fn native_fe_receive(
        &mut self,
        call: mod_api::__private::FeReceiveCall<'_>,
    ) {
        self.fire_fe_receive(call);
    }
}
