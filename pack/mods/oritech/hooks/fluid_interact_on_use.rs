// A fluid container used on an Oritech machine fills from its tanks or empties into them (`MachineBlock.useItemOn`).
fn oritech_fluid_interact_on_use(block: &str, takes: &mut bool) {
    crate::game::oritech_world::fluid_interact_on_use(block, takes);
}
