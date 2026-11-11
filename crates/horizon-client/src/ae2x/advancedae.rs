//! Advanced AE 1.6.12-1.21.1's rows in the definitions registry (feature
//! `ae2-advancedae`). Derived from Advanced AE (`AAEItems`, `AAEBlocks`,
//! `AAEComponents`, `SmallAdvPatternProviderBlock`,
//! `SmallAdvPatternProviderEntity`, `AdvPatternProviderBlock`,
//! `AdvPatternProviderEntity`, `AdvPatternProviderPart`,
//! `SmallAdvPatternProviderPart`, `AdvPatternProviderLogic`,
//! `AdvProcessingPattern`, `EncodedAdvProcessingPattern`),
//! https://github.com/pedroksl/AdvancedAE commit
//! `c322dad81e6f2ad94cb1fb861e73a7ba68e5907a` (1.6.12-1.21.1), LGPL-3.0; see
//! `ae2/addons/advancedae/`.
//!
//! The Small Advanced Pattern Provider is AE2's Pattern Provider of nine
//! patterns ([`MachineVariant`]) whose pushes send each input to the face of
//! the target its pattern names, the (Extended) Advanced Pattern Provider the
//! same of 36, each also a part on a cable; the Advanced Processing Pattern is a
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

/// `AAEBlocks.ADV_PATTERN_PROVIDER`: "Advanced Extended Pattern Provider".
pub const ADV_PATTERN_PROVIDER: &str = "advanced_ae:adv_pattern_provider";
/// `AAEItems.ADV_PATTERN_PROVIDER` and `SMALL_ADV_PATTERN_PROVIDER`: the part forms.
pub const ADV_PATTERN_PROVIDER_PART: &str = "advanced_ae:adv_pattern_provider_part";
pub const SMALL_ADV_PATTERN_PROVIDER_PART: &str = "advanced_ae:small_adv_pattern_provider_part";
/// `AAEItems.ADV_PATTERN_PROVIDER_UPGRADE` (`AdvPatternProviderUpgradeItem`) and
/// `ADV_PATTERN_PROVIDER_CAPACITY_UPGRADE` (`AdvPatternProviderCapacityUpgradeItem`).
pub const ADV_PATTERN_PROVIDER_UPGRADE: &str = "advanced_ae:adv_pattern_provider_upgrade";
pub const ADV_PATTERN_PROVIDER_CAPACITY_UPGRADE: &str = "advanced_ae:adv_pattern_provider_capacity_upgrade";

/// The providers, as AE2's Pattern Provider block and part with AE2's nine
/// return slots and no cards: `SmallAdvPatternProviderEntity` and
/// `SmallAdvPatternProviderPart` (`AdvPatternProviderLogic(node, this, 9)`),
/// `AdvPatternProviderEntity` and `AdvPatternProviderPart` (36). A part's
/// menu is its block's (`AdvPatternProviderLogicHost.openMenu`, by the size).
pub const MACHINES: &[MachineVariant] = &[
    provider(SMALL_ADV_PATTERN_PROVIDER, SMALL_ADV_PATTERN_PROVIDER, 9, false),
    provider(ADV_PATTERN_PROVIDER, ADV_PATTERN_PROVIDER, 36, false),
    provider(SMALL_ADV_PATTERN_PROVIDER_PART, SMALL_ADV_PATTERN_PROVIDER, 9, true),
    provider(ADV_PATTERN_PROVIDER_PART, ADV_PATTERN_PROVIDER, 36, true),
];
const fn provider(id: &'static str, block: &'static str, slots: usize, part: bool) -> MachineVariant {
    MachineVariant {
        id,
        base: if part { "ae2:cable_pattern_provider" } else { "ae2:pattern_provider" },
        block,
        kind: MachineKind::PatternProvider,
        slots,
        upgrades: 0,
        part,
    }
}

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
/// The Quantum Computer's blocks (`AdvCraftingBlockEntity`) have no inventory of their own.
pub const OWN_MACHINES: &[ExtMachine] = &[
    ExtMachine { id: REACTION_CHAMBER, physical_slots: 10, upgrade_slots: 4, key_drops: None, power_capacity: Some(500_000.0), part: false },
    quantum_machine(QUANTUM_UNIT),
    quantum_machine(QUANTUM_CORE),
    quantum_machine(QUANTUM_STORAGE_128),
    quantum_machine(QUANTUM_STORAGE_256),
    quantum_machine(DATA_ENTANGLER),
    quantum_machine(QUANTUM_ACCELERATOR),
    quantum_machine(QUANTUM_MULTI_THREADER),
    quantum_machine(QUANTUM_STRUCTURE),
];
const fn quantum_machine(id: &'static str) -> ExtMachine {
    ExtMachine { id, physical_slots: 0, upgrade_slots: 0, key_drops: None, power_capacity: None, part: false }
}

/// `AAEBlocks`: the Quantum Computer's blocks (`AAECraftingUnitBlock` of each `AAECraftingUnitType`).
pub const QUANTUM_UNIT: &str = "advanced_ae:quantum_unit";
pub const QUANTUM_CORE: &str = "advanced_ae:quantum_core";
pub const QUANTUM_STORAGE_128: &str = "advanced_ae:quantum_storage_128";
pub const QUANTUM_STORAGE_256: &str = "advanced_ae:quantum_storage_256";
pub const DATA_ENTANGLER: &str = "advanced_ae:data_entangler";
pub const QUANTUM_ACCELERATOR: &str = "advanced_ae:quantum_accelerator";
pub const QUANTUM_MULTI_THREADER: &str = "advanced_ae:quantum_multi_threader";
pub const QUANTUM_STRUCTURE: &str = "advanced_ae:quantum_structure";

/// `AAECraftingUnitType`: what each block of the Quantum Computer adds to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum QuantumPart {
    /// `QUANTUM_UNIT`: nothing; fills the inside.
    Unit,
    /// `QUANTUM_CORE`: 256 MB and the accelerator's threads; one inside.
    Core,
    /// `STORAGE_128M`, `STORAGE_256M`.
    Storage128,
    Storage256,
    /// `STORAGE_MULTIPLIER` (the Data Entangler): the storage times its factor.
    DataEntangler,
    /// `QUANTUM_ACCELERATOR`: threads.
    Accelerator,
    /// `MULTI_THREADER`: the threads times its factor.
    MultiThreader,
    /// `STRUCTURE`: the shell, on the box's sides only.
    Structure,
}
impl QuantumPart {
    pub const ALL: [(&'static str, Self); 8] = [
        (QUANTUM_UNIT, Self::Unit),
        (QUANTUM_CORE, Self::Core),
        (QUANTUM_STORAGE_128, Self::Storage128),
        (QUANTUM_STORAGE_256, Self::Storage256),
        (DATA_ENTANGLER, Self::DataEntangler),
        (QUANTUM_ACCELERATOR, Self::Accelerator),
        (QUANTUM_MULTI_THREADER, Self::MultiThreader),
        (QUANTUM_STRUCTURE, Self::Structure),
    ];
    pub fn of(id: &str) -> Option<Self> {
        Self::ALL.iter().find(|(name, _)| *name == id).map(|(_, part)| *part)
    }
    /// `getStorageBytes`: 1024 × 1024 × `storageMb`.
    pub fn storage_bytes(self) -> u64 {
        const MIB: u64 = 1024 * 1024;
        match self {
            Self::Core | Self::Storage256 => 256 * MIB,
            Self::Storage128 => 128 * MIB,
            _ => 0,
        }
    }
    /// `getStorageMultiplier`.
    pub fn storage_multiplier(self, config: &QuantumConfig) -> u64 {
        if self == Self::DataEntangler { u64::from(config.data_entangler_multiplication) } else { 0 }
    }
    /// `getAcceleratorThreads`: the accelerator's and the core's.
    pub fn accelerator_threads(self, config: &QuantumConfig) -> u32 {
        if matches!(self, Self::Accelerator | Self::Core) { config.accelerator_threads } else { 0 }
    }
    /// `getAccelerationMultiplier`.
    pub fn acceleration_multiplier(self, config: &QuantumConfig) -> u32 {
        if self == Self::MultiThreader { config.multi_threader_multiplication } else { 0 }
    }
}

/// `advanced_ae-common.toml`'s `["quantum computer"]` (`AAEConfig`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuantumConfig {
    /// `quantumComputerMaxSize`, 5 to 12: the box's longest side.
    pub max_size: i32,
    /// `quantumComputerAcceleratorThreads`, 4 to 16.
    pub accelerator_threads: u32,
    /// `quantumComputerMaxMultiThreaders`, 1 to 2.
    pub max_multi_threaders: u32,
    /// `quantumComputermaxDataEntanglers`, 1 to 2.
    pub max_data_entanglers: u32,
    /// `quantumComputerMultiThreaderMultiplication`, 2 to 8.
    pub multi_threader_multiplication: u32,
    /// `quantumComputerDataEntanglerMultiplication`, 2 to 8.
    pub data_entangler_multiplication: u32,
}
impl Default for QuantumConfig {
    fn default() -> Self {
        Self {
            max_size: 7,
            accelerator_threads: 8,
            max_multi_threaders: 1,
            max_data_entanglers: 1,
            multi_threader_multiplication: 4,
            data_entangler_multiplication: 4,
        }
    }
}
impl QuantumConfig {
    /// A value of the wrong type or outside its range falls back to its
    /// default, as `ModConfigSpec` corrects it.
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut config = Self::default();
        let section = value.get("quantum computer");
        let int = |key: &str, lo: i64, hi: i64| section.and_then(|s| s.get(key)).and_then(toml::Value::as_integer).filter(|n| (lo..=hi).contains(n));
        if let Some(n) = int("quantumComputerMaxSize", 5, 12) {
            config.max_size = n as i32;
        }
        if let Some(n) = int("quantumComputerAcceleratorThreads", 4, 16) {
            config.accelerator_threads = n as u32;
        }
        if let Some(n) = int("quantumComputerMaxMultiThreaders", 1, 2) {
            config.max_multi_threaders = n as u32;
        }
        if let Some(n) = int("quantumComputermaxDataEntanglers", 1, 2) {
            config.max_data_entanglers = n as u32;
        }
        if let Some(n) = int("quantumComputerMultiThreaderMultiplication", 2, 8) {
            config.multi_threader_multiplication = n as u32;
        }
        if let Some(n) = int("quantumComputerDataEntanglerMultiplication", 2, 8) {
            config.data_entangler_multiplication = n as u32;
        }
        Ok(config)
    }
    /// The game's `config/advanced_ae-common.toml`, read once (`ModConfigEvent`).
    pub fn get() -> &'static Self {
        static CONFIG: std::sync::OnceLock<QuantumConfig> = std::sync::OnceLock::new();
        CONFIG.get_or_init(|| {
            let path = crate::logging::game_home().join("config/advanced_ae-common.toml");
            match std::fs::read_to_string(&path) {
                Ok(text) => Self::parse(&text).unwrap_or_else(|error| {
                    log::warn!("invalid Advanced AE config {}: {error}", path.display());
                    Self::default()
                }),
                Err(_) => Self::default(),
            }
        })
    }
}

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
    MACHINES.iter().any(|row| row.id == id)
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
    fn the_large_provider_and_the_parts_are_ae2s_of_their_sizes() {
        crate::ae2x::install();
        let large = crate::ae2_world::Machine::for_block(ADV_PATTERN_PROVIDER).unwrap();
        assert_eq!((large.kind, large.slots.len(), large.upgrades.len()), (MachineKind::PatternProvider, 36, 0));
        for (part, block, slots) in [(SMALL_ADV_PATTERN_PROVIDER_PART, SMALL_ADV_PATTERN_PROVIDER, 9), (ADV_PATTERN_PROVIDER_PART, ADV_PATTERN_PROVIDER, 36)] {
            let row = crate::ae2_world::variant(part).unwrap();
            assert!(row.part && row.base == "ae2:cable_pattern_provider" && row.block == block && row.slots == slots);
            assert!(crate::ae2_world::Machine::for_block(part).is_none(), "a part is no block");
            assert!(directional_provider(part));
        }
        assert!(directional_provider(ADV_PATTERN_PROVIDER) && !directional_provider("extendedae:ex_pattern_provider"));
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
    fn the_quantum_computer_blocks_are_machines_of_their_parts() {
        crate::ae2x::install();
        for (id, part) in QuantumPart::ALL {
            let machine = crate::ae2_world::Machine::for_block(id).unwrap();
            assert_eq!(machine.kind.ext().map(|row| row.id), Some(id));
            assert!(machine.slots.is_empty() && machine.upgrades.is_empty());
            assert_eq!(QuantumPart::of(id), Some(part));
        }
        let config = QuantumConfig::default();
        assert_eq!(QuantumPart::Core.storage_bytes(), 256 << 20);
        assert_eq!(QuantumPart::Storage128.storage_bytes(), 128 << 20);
        assert_eq!((QuantumPart::Core.accelerator_threads(&config), QuantumPart::Unit.accelerator_threads(&config)), (8, 0));
        assert_eq!(QuantumPart::DataEntangler.storage_multiplier(&config), 4);
        assert_eq!(QuantumPart::MultiThreader.acceleration_multiplier(&config), 4);
    }

    #[test]
    fn the_quantum_config_reads_its_section_within_the_ranges() {
        // All the Mods 10's file.
        let atm10 = "[\"quantum computer\"]\nquantumComputerMaxSize = 7\nquantumComputerAcceleratorThreads = 12\nquantumComputerMaxMultiThreaders = 1\nquantumComputermaxDataEntanglers = 1\nquantumComputerMultiThreaderMultiplication = 6\nquantumComputerDataEntanglerMultiplication = 6\n";
        let config = QuantumConfig::parse(atm10).unwrap();
        assert_eq!((config.max_size, config.accelerator_threads, config.multi_threader_multiplication, config.data_entangler_multiplication), (7, 12, 6, 6));
        let wrong = QuantumConfig::parse("[\"quantum computer\"]\nquantumComputerMaxSize = 40\nquantumComputerAcceleratorThreads = \"x\"\n").unwrap();
        assert_eq!(wrong, QuantumConfig::default());
    }

    #[test]
    fn nullable_directions_round_trip_by_name() {
        for value in [None, Some(0), Some(1), Some(2), Some(3), Some(4), Some(5)] {
            assert_eq!(nullable_direction(nullable_direction_name(value)), Some(value));
        }
        assert_eq!(nullable_direction("north"), None, "Direction.name() is upper case");
    }
}
