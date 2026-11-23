// The Paradox Machine's time fluid tank and the Fluid Collector T1's tank (`IFluidHandler` readback).
fn justdirethings_fluid_handler_tanks(
    entity: &mod_api::ModBlockEntity,
    tanks: &mut Option<Vec<mod_api::NativeFluidTankInfo>>,
) {
    if tanks.is_some() {
        return;
    }
    if entity.id() == crate::game::justdirethings_time::PARADOX {
        crate::game::justdirethings_time::fluid_tanks(entity, tanks);
    } else if entity.id() == crate::game::justdirethings_machines::COLLECTOR {
        crate::game::justdirethings_machines::fluid_tanks(entity, tanks);
    }
}
