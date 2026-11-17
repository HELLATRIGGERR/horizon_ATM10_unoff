// The chemical tanks of a Mekanism block on one side (`IChemicalHandler` readback, for AE2's
// Applied Mekanistics).
fn mek_chemical_handler_tanks(
    game: &crate::game::Game<'_>,
    dimension: &str,
    pos: [i32; 3],
    face: Option<horizon_core::geometry::Direction>,
    tanks: &mut Option<Vec<horizon_client::mod_api::NativeChemicalTankInfo>>,
) {
    if tanks.is_none() {
        *tanks = crate::game::mek_chemicals::tanks(game, dimension, pos, face);
    }
}
