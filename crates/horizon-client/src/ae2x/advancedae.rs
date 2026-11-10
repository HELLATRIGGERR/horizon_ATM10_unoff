//! Advanced AE 1.6.12-1.21.1's rows in the definitions registry (feature
//! `ae2-advancedae`). Derived from Advanced AE (`AAEItems`, `AAEBlocks`,
//! `AAEComponents`, `SmallAdvPatternProviderBlock`,
//! `SmallAdvPatternProviderEntity`, `AdvPatternProviderLogic`,
//! `AdvProcessingPattern`, `EncodedAdvProcessingPattern`),
//! https://github.com/pedroksl/AdvancedAE commit
//! `c322dad81e6f2ad94cb1fb861e73a7ba68e5907a` (1.6.12-1.21.1), LGPL-3.0; see
//! `ae2/addons/advancedae/`.
//!
//! The Small Advanced Pattern Provider is AE2's Pattern Provider of nine
//! patterns ([`MachineVariant`]) whose pushes send each input to the face of
//! the target its pattern names; the Advanced Processing Pattern is a
//! processing pattern of its own item and component. The game's side is in
//! `game/ae2x/advancedae/`.
use super::defs::Defs;
use crate::ae2_world::{MachineKind, MachineVariant};

pub const NAMESPACE: &str = "advanced_ae";
/// `AAEBlocks.SMALL_ADV_PATTERN_PROVIDER`: "Advanced Pattern Provider".
pub const SMALL_ADV_PATTERN_PROVIDER: &str = "advanced_ae:small_adv_pattern_provider";
/// `AAEItems.ADV_PROCESSING_PATTERN`.
pub const ADV_PROCESSING_PATTERN: &str = "advanced_ae:adv_processing_pattern";
/// `AAEComponents.ENCODED_ADV_PROCESSING_PATTERN`: `sparseInputs`,
/// `sparseOutputs` and `directionMap` (`EncodedAdvProcessingPattern.CODEC`).
pub const ENCODED_ADV_PROCESSING_PATTERN: &str = "advanced_ae:encoded_adv_processing_pattern";
/// `AAEItems.ADV_PATTERN_ENCODER`.
pub const ADV_PATTERN_ENCODER: &str = "advanced_ae:adv_pattern_encoder";

/// `SmallAdvPatternProviderEntity.createLogic`: `AdvPatternProviderLogic(node,
/// this, 9)`, AE2's nine return slots, no cards.
pub const MACHINES: &[MachineVariant] = &[MachineVariant {
    id: SMALL_ADV_PATTERN_PROVIDER,
    base: "ae2:pattern_provider",
    block: SMALL_ADV_PATTERN_PROVIDER,
    kind: MachineKind::PatternProvider,
    slots: 9,
    upgrades: 0,
    part: false,
}];

/// Whether `id` is an Advanced AE provider, whose pushes are directional
/// (`AdvPatternProviderLogic.pushPattern`).
pub fn directional_provider(id: &str) -> bool {
    id == SMALL_ADV_PATTERN_PROVIDER
}

/// `NullableDirection` as `EncodedAdvProcessingPattern`'s `directionMap`
/// saves it (`getSerializedName`: `Direction.name()`, or `null`): the
/// direction's 3D data value (`Direction.get3DDataValue`), none for `null`.
/// None for a name it does not know.
pub fn nullable_direction(name: &str) -> Option<Option<u8>> {
    Some(match name {
        "DOWN" => Some(0),
        "UP" => Some(1),
        "NORTH" => Some(2),
        "SOUTH" => Some(3),
        "WEST" => Some(4),
        "EAST" => Some(5),
        "null" => None,
        _ => return None,
    })
}
/// The name [`nullable_direction`] reads.
pub fn nullable_direction_name(direction: Option<u8>) -> &'static str {
    match direction {
        Some(0) => "DOWN",
        Some(1) => "UP",
        Some(2) => "NORTH",
        Some(3) => "SOUTH",
        Some(4) => "WEST",
        Some(5) => "EAST",
        _ => "null",
    }
}

pub fn register(defs: &mut Defs) {
    for variant in MACHINES {
        defs.add_variant(*variant);
    }
    // PatternDetailsHelper.encodedPatternItemBuilder(AdvProcessingPattern::new).
    defs.add_processing_pattern(ADV_PROCESSING_PATTERN, ENCODED_ADV_PROCESSING_PATTERN);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_small_provider_is_ae2s_of_nine_patterns() {
        crate::ae2x::install();
        let machine = crate::ae2_world::Machine::for_block(SMALL_ADV_PATTERN_PROVIDER).unwrap();
        assert_eq!(machine.kind, MachineKind::PatternProvider);
        assert_eq!(machine.slots.len(), 9);
        assert_eq!(machine.id(), SMALL_ADV_PATTERN_PROVIDER);
        assert!(directional_provider(SMALL_ADV_PATTERN_PROVIDER) && !directional_provider("ae2:pattern_provider"));
    }

    #[test]
    fn the_advanced_pattern_is_a_processing_pattern_of_its_own_component() {
        let defs = crate::ae2x::defs::get();
        assert_eq!(defs.processing_pattern_component(ADV_PROCESSING_PATTERN), Some(ENCODED_ADV_PROCESSING_PATTERN));
        assert_eq!(defs.processing_pattern_component("ae2:processing_pattern"), Some("ae2:encoded_processing_pattern"));
        assert_eq!(defs.processing_pattern_component("ae2:crafting_pattern"), None);
        assert!(defs.processing_pattern_items().any(|id| id == ADV_PROCESSING_PATTERN));
    }

    #[test]
    fn nullable_directions_round_trip_by_name() {
        for value in [None, Some(0), Some(1), Some(2), Some(3), Some(4), Some(5)] {
            assert_eq!(nullable_direction(nullable_direction_name(value)), Some(value));
        }
        assert_eq!(nullable_direction("north"), None, "Direction.name() is upper case");
    }
}
