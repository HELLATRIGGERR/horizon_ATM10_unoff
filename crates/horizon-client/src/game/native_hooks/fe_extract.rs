use super::*;

impl Game<'_> {
    /// `IEnergyStorage.extractEnergy` on the block entity at `pos`, a mod's, as `fe_receive`: set
    /// `given` (0 to `max`).
    pub(in crate::game) fn native_fe_extract(
        &mut self,
        call: mod_api::__private::FeExtractCall<'_>,
    ) {
        self.fire_fe_extract(call);
    }
}
