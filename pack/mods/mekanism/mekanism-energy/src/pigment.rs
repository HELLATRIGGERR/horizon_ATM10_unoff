//! Mekanism's pigment machines (Mekanism 10.7.19, MIT, Copyright (c) 2017-2025
//! Aidan C. Brady): the Pigment Extractor (`TileEntityPigmentExtractor`, an
//! item into a pigment as the oxidizer does), the Pigment Mixer
//! (`TileEntityPigmentMixer`, two pigments into one as the chemical infuser
//! does) and the Painting Machine (`TileEntityPaintingMachine`, a
//! `TileEntityProgressMachine` taking an item and a pigment for an item).
//! Only their sizes are here; the rules are the oxidizer's, the infuser's and
//! `crate::machine::process` (the runtime's `mek_pigment` runs the painter).

/// `TileEntityPigmentExtractor.MAX_PIGMENT`.
pub const EXTRACTOR_TANK: i64 = 20_000;
/// `TileEntityPigmentExtractor.BASE_TICKS_REQUIRED`.
pub const EXTRACTOR_TICKS: u32 = 100;
/// `TileEntityPigmentMixer.MAX_INPUT_PIGMENT`, each of its two input tanks.
pub const MIXER_INPUT: i64 = 1_000;
/// `TileEntityPigmentMixer.MAX_OUTPUT_PIGMENT`.
pub const MIXER_OUTPUT: i64 = 2_000;
/// `TileEntityPaintingMachine.MAX_PIGMENT`.
pub const PAINTING_TANK: i64 = 15_000;
/// `TileEntityPaintingMachine.BASE_TICKS_REQUIRED`.
pub const PAINTING_TICKS: u32 = 200;

/// A pigment machine's tanks, in mB.
pub fn tanks(kind: crate::machine::MachineKind) -> &'static [i64] {
    use crate::machine::MachineKind as K;
    match kind {
        K::PigmentExtractor => &[EXTRACTOR_TANK],
        K::PigmentMixer => &[MIXER_INPUT, MIXER_INPUT, MIXER_OUTPUT],
        K::PaintingMachine => &[PAINTING_TANK],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::MachineKind as K;

    #[test]
    fn the_pigment_machines_have_mekanisms_sizes_and_sides() {
        assert_eq!(tanks(K::PigmentMixer), [1000, 1000, 2000]);
        assert_eq!(tanks(K::PigmentExtractor), [20_000]);
        assert_eq!(K::of_block("mekanism:painting_machine"), Some(K::PaintingMachine));
        assert_eq!(K::PigmentExtractor.recipe_type(), "mekanism:pigment_extracting");
        for k in [K::PigmentExtractor, K::PigmentMixer, K::PaintingMachine] {
            assert!(k.pigments() && k.shows_active(), "{k:?}");
        }
        assert!(K::PigmentExtractor.oxidizes() && K::PigmentMixer.infuses_chemicals() && K::PaintingMachine.ore_chain());
        assert!(K::PigmentMixer.bounded_above() && !K::PigmentExtractor.bounded_above());
        let p = K::PaintingMachine.slots();
        assert_eq!((p.infusion, p.input, p.output, p.energy), (Some(0), 1, 2, 3));
    }
}
