//! ExtendedAE 1.21-2.2.42's rows in the definitions registry (feature
//! `ae2-extendedae`). Derived from ExtendedAE (`TileExPatternProvider`,
//! `PartExPatternProvider`, `TileExInterface`, `PartExInterface`,
//! `TileExDrive`), https://github.com/GlodBlock/ExtendedAE commit
//! `1f86f927552f02451236465c0c96a12a3de8f66c` (2.2.42), LGPL-3.0; see
//! `ae2/addons/extendedae/`.
//!
//! Slice 1: its machines are AE2's with other sizes, so each is a
//! [`MachineVariant`] on AE2's kind. The game's side is in
//! `game/ae2x/extendedae/`.
use super::defs::Defs;
use crate::ae2_world::{MachineKind, MachineVariant};

pub const NAMESPACE: &str = "extendedae";

/// The variants, as the mod sizes them.
pub const MACHINES: &[MachineVariant] = &[
    // PatternProviderLogic(node, host, 36); its return inventory stays nine slots.
    MachineVariant {
        id: "extendedae:ex_pattern_provider",
        base: "ae2:pattern_provider",
        block: "extendedae:ex_pattern_provider",
        kind: MachineKind::PatternProvider,
        slots: 36,
        upgrades: 0,
        part: false,
    },
    MachineVariant {
        id: "extendedae:ex_pattern_provider_part",
        base: "ae2:cable_pattern_provider",
        block: "extendedae:ex_pattern_provider",
        kind: MachineKind::PatternProvider,
        slots: 36,
        upgrades: 0,
        part: true,
    },
    // InterfaceLogic(node, host, item, 36): 36 config and 36 storage slots, one card.
    MachineVariant {
        id: "extendedae:ex_interface",
        base: "ae2:interface",
        block: "extendedae:ex_interface",
        kind: MachineKind::Interface,
        slots: 36,
        upgrades: 1,
        part: false,
    },
    MachineVariant {
        id: "extendedae:ex_interface_part",
        base: "ae2:cable_interface",
        block: "extendedae:ex_interface",
        kind: MachineKind::Interface,
        slots: 36,
        upgrades: 1,
        part: true,
    },
    // getCellCount() = 20.
    MachineVariant {
        id: "extendedae:ex_drive",
        base: "ae2:drive",
        block: "extendedae:ex_drive",
        kind: MachineKind::Drive,
        slots: 20,
        upgrades: 0,
        part: false,
    },
];

pub fn register(defs: &mut Defs) {
    for machine in MACHINES {
        defs.add_variant(*machine);
    }
}

#[cfg(test)]
mod tests {
    use crate::ae2_world::{Machine, MachineKind, base_id, is_machine_block, max_provider_slots, variant};
    use crate::containers::BlockEntity;

    #[test]
    fn extendedae_rows_keep_the_mods_sizes() {
        crate::ae2x::install();
        for (id, kind, slots, upgrades) in [
            ("extendedae:ex_pattern_provider", MachineKind::PatternProvider, 36, 0),
            ("extendedae:ex_interface", MachineKind::Interface, 36, 1),
            ("extendedae:ex_drive", MachineKind::Drive, 20, 0),
        ] {
            let machine = Machine::for_block(id).unwrap();
            assert_eq!((machine.kind, machine.slots.len(), machine.upgrades.len()), (kind, slots, upgrades), "{id}");
            assert_eq!(machine.id(), id);
            assert!(is_machine_block(id));
            assert_eq!(BlockEntity::Ae2(Box::new(machine)).id(), id);
        }
        for (id, base) in [
            ("extendedae:ex_pattern_provider_part", "ae2:cable_pattern_provider"),
            ("extendedae:ex_interface_part", "ae2:cable_interface"),
        ] {
            // Parts are no blocks.
            assert!(Machine::for_block(id).is_none());
            assert!(!is_machine_block(id));
            assert_eq!(variant(id).unwrap().slots, 36);
            assert_eq!(base_id(id), base);
        }
        assert_eq!(base_id("extendedae:ex_drive"), "ae2:drive");
        assert_eq!(base_id("ae2:drive"), "ae2:drive");
        assert_eq!(max_provider_slots(), 36);
        // AE2's own machines stay as they are, and a variant is no addon kind of its own.
        assert_eq!(Machine::for_block("ae2:pattern_provider").unwrap().slots.len(), 9);
        assert_eq!(Machine::for_block("ae2:drive").unwrap().id(), "ae2:drive");
        assert!(MachineKind::from_id("extendedae:ex_drive").is_none());
    }
}
