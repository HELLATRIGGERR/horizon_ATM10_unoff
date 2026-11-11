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
//! processing pattern of its own item and component. The Reaction Chamber
//! (`ReactionChamberBlock`, `ReactionChamberEntity`) is a machine of its own
//! ([`ExtMachine`]): nine inputs and an output, four speed cards, a buffer of
//! 500,000 AE, its two tanks in its config. The game's side is in
//! `game/ae2x/advancedae/`.
use super::defs::Defs;
use crate::ae2_world::{ExtMachine, MachineKind, MachineVariant};

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

/// `AAEBlocks.REACTION_CHAMBER`.
pub const REACTION_CHAMBER: &str = "advanced_ae:reaction_chamber";
/// `ReactionChamberRecipe.TYPE_ID`.
pub const REACTION: &str = "advanced_ae:reaction";
/// The block state's `working` (`ReactionChamberBlock.WORKING`, `setWorking`),
/// in the machine's config.
pub const WORKING: &str = "aae_working";
/// `showWarning`: the last step had too little power for full speed.
pub const WARNING: &str = "aae_warning";

/// `ReactionChamberEntity`: `inputInv` (9) and `outputInv` (1), `UpgradeInventories.forMachine(REACTION_CHAMBER, 4)`,
/// `setInternalMaxPower(MAX_POWER_STORAGE)`. Its tanks' fluids drop nothing (`AEFluidKey.addDrops`).
pub const OWN_MACHINES: &[ExtMachine] =
    &[ExtMachine { id: REACTION_CHAMBER, physical_slots: 10, upgrade_slots: 4, key_drops: None, power_capacity: Some(500_000.0), part: false }];

/// What the Reaction Chamber does not save (`saveAdditional`): its
/// `processingTime`, `working` and `showWarning`.
pub fn reset_unsaved(m: &mut crate::ae2_world::Machine) {
    if m.kind.ext().is_some_and(|row| row.id == REACTION_CHAMBER) {
        m.progress = 0.0;
        m.config.remove(&WORKING.into());
        m.config.remove(&WARNING.into());
    }
}

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
    for machine in OWN_MACHINES {
        defs.add_machine(*machine);
    }
    // AdvancedAE.initUpgrades, for the machines carried so far.
    defs.add_upgrade("ae2:speed_card", REACTION_CHAMBER, 4);
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
    fn the_reaction_chamber_is_a_machine_of_nine_inputs_an_output_and_four_speed_cards() {
        crate::ae2x::install();
        let mut machine = crate::ae2_world::Machine::for_block(REACTION_CHAMBER).unwrap();
        assert_eq!(machine.kind.ext().map(|row| row.id), Some(REACTION_CHAMBER));
        assert_eq!((machine.slots.len(), machine.upgrades.len()), (10, 4));
        assert_eq!(machine.kind.ext().and_then(|row| row.power_capacity), Some(500_000.0));
        assert_eq!(crate::ae2x::defs::get().upgrade_limit("ae2:speed_card", REACTION_CHAMBER), 4);
        machine.progress = 120.0;
        machine.config.insert(WORKING.into(), horizon_world::nbt::Tag::Byte(1));
        machine.config.insert("auto_export".into(), horizon_world::nbt::Tag::String("YES".into()));
        crate::ae2x::reset_unsaved(&mut machine);
        assert_eq!(machine.progress, 0.0, "processingTime is not saved");
        assert!(machine.config.get(&WORKING.into()).is_none());
        assert!(machine.config.get(&"auto_export".into()).is_some(), "the settings are");
    }

    #[test]
    fn nullable_directions_round_trip_by_name() {
        for value in [None, Some(0), Some(1), Some(2), Some(3), Some(4), Some(5)] {
            assert_eq!(nullable_direction(nullable_direction_name(value)), Some(value));
        }
        assert_eq!(nullable_direction("north"), None, "Direction.name() is upper case");
    }
}
