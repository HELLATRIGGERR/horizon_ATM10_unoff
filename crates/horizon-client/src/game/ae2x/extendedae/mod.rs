//! ExtendedAE 1.21-2.2.42 in the game (feature `ae2-extendedae`). Its machines
//! are AE2's with other sizes (rows in the library's `ae2x::extendedae`);
//! their screens are layouts of `ae2_screens` (`screens.rs`, a child module
//! of it). What needs the game's own code is here, reached through
//! `game/ae2x/mod.rs`.
pub(in crate::game) mod upgrade;

/// `PartExPatternProvider` and `PartExInterface`: their base models.
pub(in crate::game) fn part_model(id: &str) -> Option<&'static str> {
    match id {
        "extendedae:ex_pattern_provider_part" => Some("extendedae:part/ex_pattern_provider_base"),
        "extendedae:ex_interface_part" => Some("extendedae:part/ex_interface"),
        _ => None,
    }
}
