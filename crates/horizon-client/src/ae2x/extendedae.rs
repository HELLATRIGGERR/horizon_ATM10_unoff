//! ExtendedAE 1.21-2.2.42's rows in the definitions registry (feature
//! `ae2-extendedae`). Derived from ExtendedAE (`TileExPatternProvider`,
//! `PartExPatternProvider`, `TileExInterface`, `PartExInterface`,
//! `TileExDrive`, `TileExIOPort`, `TileCrystalAssembler`, `TileCircuitCutter`,
//! `TileCrystalFixer`, `EAERegistryHandler.registerUpgrades`, `EAEConfig`),
//! https://github.com/GlodBlock/ExtendedAE commit
//! `1f86f927552f02451236465c0c96a12a3de8f66c` (2.2.42), LGPL-3.0; see
//! `ae2/addons/extendedae/`.
//!
//! Its bigger devices are AE2's with other sizes, so each is a
//! [`MachineVariant`] on AE2's kind. Its own machines (Crystal Assembler,
//! Circuit Cutter, Crystal Fixer) are [`ExtMachine`]s. The game's side is in
//! `game/ae2x/extendedae/`.
use super::defs::Defs;
use crate::ae2_world::{ExtMachine, MachineKind, MachineVariant};
use std::sync::OnceLock;

pub const NAMESPACE: &str = "extendedae";
/// `TileCrystalAssembler`: nine inputs, one output, a fluid tank of 16 buckets.
pub const CRYSTAL_ASSEMBLER: &str = "extendedae:crystal_assembler";
/// `TileCircuitCutter`, the Circuit Slicer: one input, one output.
pub const CIRCUIT_CUTTER: &str = "extendedae:circuit_cutter";
/// `TileCrystalFixer`: one fuel slot; it works on the block in front of it.
pub const CRYSTAL_FIXER: &str = "extendedae:crystal_fixer";
pub const EX_IO_PORT: &str = "extendedae:ex_io_port";
pub const EX_IMPORT_BUS: &str = "extendedae:ex_import_bus_part";
pub const EX_EXPORT_BUS: &str = "extendedae:ex_export_bus_part";

/// `extendedae-common.toml` (`EAEConfig`): what this port reads of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    /// `device.extended_io_bus_multiplier`, 2 to 128: the Extended Import and
    /// Export Buses' operations, times AE2's.
    pub bus_speed: u64,
}
impl Default for Config {
    fn default() -> Self {
        Self { bus_speed: 8 }
    }
}
impl Config {
    /// A value of the wrong type or outside its range falls back to its
    /// default, as `ModConfigSpec` corrects it.
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut config = Self::default();
        let speed = value.get("device").and_then(|d| d.get("extended_io_bus_multiplier")).and_then(toml::Value::as_integer);
        if let Some(n) = speed.filter(|n| (2..=128).contains(n)) {
            config.bus_speed = n as u64;
        }
        Ok(config)
    }
    /// The game's `config/extendedae-common.toml`, read once (`ModConfigEvent`).
    pub fn get() -> Self {
        static CONFIG: OnceLock<Config> = OnceLock::new();
        *CONFIG.get_or_init(|| {
            let path = crate::logging::game_home().join("config/extendedae-common.toml");
            match std::fs::read_to_string(&path) {
                Ok(text) => Self::parse(&text).unwrap_or_else(|error| {
                    log::warn!("invalid ExtendedAE config {}: {error}", path.display());
                    Self::default()
                }),
                Err(_) => Self::default(),
            }
        })
    }
}

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
    // IOPortBlockEntity's six cells in, six out; UpgradeInventories.forMachine(EX_IO_PORT, 5).
    MachineVariant {
        id: EX_IO_PORT,
        base: "ae2:io_port",
        block: EX_IO_PORT,
        kind: MachineKind::IoPort,
        slots: 12,
        upgrades: 5,
        part: false,
    },
];

/// `TileExIOPort.tickingRequest`: 2,048 items a tick, doubled by each speed
/// card up to five (AE2's own port moves 256, doubled up to three).
pub fn io_port_budget(speed_cards: usize) -> u64 {
    2048 << speed_cards.min(5)
}

/// The block state's `working` of the Crystal Assembler and Circuit Cutter
/// (`setWorking`), kept in the machine's config.
pub const WORKING: &str = "eae_working";

/// What the mod's own machines do not save (`saveAdditional`): the Crystal
/// Assembler's and Circuit Cutter's progress and `isWorking`. The Crystal
/// Fixer saves its progress.
pub fn reset_unsaved(m: &mut crate::ae2_world::Machine) {
    if matches!(m.kind.ext().map(|row| row.id), Some(CRYSTAL_ASSEMBLER | CIRCUIT_CUTTER)) {
        m.progress = 0.0;
        m.config.remove(&WORKING.into());
    }
}

/// The mod's own machines: physical slots, cards, and the AE buffer
/// (`POWER_MAXIMUM_AMOUNT`, 8,000 on each).
pub const OWN_MACHINES: &[ExtMachine] = &[
    ExtMachine { id: CRYSTAL_ASSEMBLER, physical_slots: 10, upgrade_slots: 4, key_drops: None, power_capacity: Some(8000.0) },
    ExtMachine { id: CIRCUIT_CUTTER, physical_slots: 2, upgrade_slots: 4, key_drops: None, power_capacity: Some(8000.0) },
    ExtMachine { id: CRYSTAL_FIXER, physical_slots: 1, upgrade_slots: 0, key_drops: None, power_capacity: Some(8000.0) },
];

pub fn register(defs: &mut Defs) {
    for machine in MACHINES {
        defs.add_variant(*machine);
    }
    for machine in OWN_MACHINES {
        defs.add_machine(*machine);
    }
    // EAERegistryHandler.registerUpgrades, for the devices carried so far.
    for host in ["extendedae:ex_interface", "extendedae:ex_interface_part"] {
        defs.add_upgrade("ae2:fuzzy_card", host, 1);
        defs.add_upgrade("ae2:crafting_card", host, 1);
    }
    for (card, max) in [("ae2:capacity_card", 5), ("ae2:redstone_card", 1), ("ae2:speed_card", 4), ("ae2:fuzzy_card", 1), ("ae2:crafting_card", 1)] {
        defs.add_upgrade(card, EX_EXPORT_BUS, max);
    }
    for (card, max) in [("ae2:capacity_card", 5), ("ae2:redstone_card", 1), ("ae2:speed_card", 4), ("ae2:inverter_card", 1), ("ae2:fuzzy_card", 1)] {
        defs.add_upgrade(card, EX_IMPORT_BUS, max);
    }
    defs.add_upgrade("ae2:speed_card", EX_IO_PORT, 5);
    defs.add_upgrade("ae2:redstone_card", EX_IO_PORT, 1);
    defs.add_upgrade("ae2:speed_card", CRYSTAL_ASSEMBLER, 4);
    defs.add_upgrade("ae2:speed_card", CIRCUIT_CUTTER, 4);
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
        let port = Machine::for_block(super::EX_IO_PORT).unwrap();
        assert_eq!((port.kind, port.slots.len(), port.upgrades.len()), (MachineKind::IoPort, 12, 5));
        assert_eq!(base_id(super::EX_IO_PORT), "ae2:io_port");
    }

    #[test]
    fn extendedae_machines_have_their_inventories_and_cards() {
        crate::ae2x::install();
        let defs = crate::ae2x::defs::get();
        for (id, slots, upgrades) in [(super::CRYSTAL_ASSEMBLER, 10, 4), (super::CIRCUIT_CUTTER, 2, 4), (super::CRYSTAL_FIXER, 1, 0)] {
            assert!(matches!(MachineKind::from_id(id), Some(MachineKind::Ext(_))));
            let machine = Machine::for_block(id).unwrap();
            assert_eq!((machine.slots.len(), machine.upgrades.len(), machine.id()), (slots, upgrades, id));
            assert_eq!(machine.private_power_capacity(), Some(8000.0));
            assert!(is_machine_block(id));
        }
        assert_eq!(defs.upgrade_limit("ae2:speed_card", super::CRYSTAL_ASSEMBLER), 4);
        assert_eq!(defs.upgrade_limit("ae2:speed_card", super::EX_IO_PORT), 5);
        assert_eq!(defs.upgrade_limit("ae2:capacity_card", super::EX_IO_PORT), 0);
        assert_eq!(defs.upgrade_limit("ae2:inverter_card", super::EX_IMPORT_BUS), 1);
        assert_eq!(defs.upgrade_limit("ae2:inverter_card", super::EX_EXPORT_BUS), 0);
        assert_eq!(defs.upgrade_limit("ae2:crafting_card", super::EX_EXPORT_BUS), 1);
        // 2048, then x2, x4, x8, x16, x32; AE2's port moves 256.
        assert_eq!((0..=6).map(super::io_port_budget).collect::<Vec<_>>(), [2048, 4096, 8192, 16384, 32768, 65536, 65536]);
    }

    #[test]
    fn extendedae_config_reads_the_bus_multiplier() {
        use super::Config;
        assert_eq!(Config::parse("[device]\n\textended_io_bus_multiplier = 8\n").unwrap().bus_speed, 8);
        assert_eq!(Config::parse("[device]\nextended_io_bus_multiplier = 16").unwrap().bus_speed, 16);
        // Outside 2..=128: the default, as ModConfigSpec corrects it.
        assert_eq!(Config::parse("[device]\nextended_io_bus_multiplier = 1").unwrap().bus_speed, 8);
        assert_eq!(Config::parse("").unwrap(), Config::default());
    }
}
