//! Mekanism's first processing machines: the Enrichment Chamber, the Crusher
//! and the Energized Smelter (`TileEntityElectricMachine`, one item in, one
//! out) and the Metallurgic Infuser (`TileEntityMetallurgicInfuser`, an item
//! and an infuse type in, an item out). They share `TileEntityProgressMachine`:
//! a recipe takes `ticksRequired` ticks of `energyPerTick` Joules each
//! (`CachedRecipe.process`), both set by speed and energy upgrades
//! (`MekanismUtils.getTicks`, `getEnergyPerTick`, `getMaxEnergy`), which a
//! machine installs from its upgrade slot (`TileComponentUpgrade`). Their
//! side configuration (`TileComponentConfig`) says which faces take energy,
//! which take and give items and where the ejector (`TileComponentEjector`)
//! puts what they made. The infuser's tank holds one infuse type, a chemical
//! (`crate::chemical`: a `BasicChemicalTank` of 1000 mB), filled from items by
//! `mekanism:chemical_conversion` recipes and by other blocks through its
//! chemical sides. The Electrolytic Separator (`crate::separator`) shares
//! their energy, upgrades and side configuration, with a fluid and two
//! chemical tanks of its own.
use crate::container::{Automation, EnergyContainer};
use crate::sides::{DataType, Direction, RelativeSide};

/// `TileEntityElectricMachine.BASE_TICKS_REQUIRED` and the infuser's: ten seconds.
pub const BASE_TICKS: u32 = 200;
/// `TileEntityMetallurgicInfuser.MAX_INFUSE`: the infusion tank, in mB.
pub const INFUSE_TANK: u64 = 1000;
/// `TileComponentUpgrade.UPGRADE_TICKS_REQUIRED`: an upgrade in the slot goes
/// in on the tick after this many.
pub const UPGRADE_TICKS: u32 = 20;
/// `MekanismUtils.TICKS_PER_HALF_SECOND`: the ejector's wait between tries.
pub const EJECT_DELAY: u32 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MachineKind {
    /// `mekanism:enrichment_chamber`, `mekanism:enriching` recipes.
    EnrichmentChamber,
    /// `mekanism:crusher`, `mekanism:crushing` recipes.
    Crusher,
    /// `mekanism:energized_smelter`: vanilla's smelting recipes
    /// (`MekanismRecipeType.SMELTING` wraps each, one in, its result out).
    EnergizedSmelter,
    /// `mekanism:metallurgic_infuser`, `mekanism:metallurgic_infusing` recipes.
    MetallurgicInfuser,
    /// `mekanism:electrolytic_separator`, `mekanism:separating` recipes
    /// (`TileEntityElectrolyticSeparator`: no progress, operations a tick).
    ElectrolyticSeparator,
    /// `mekanism:osmium_compressor`, `mekanism:compressing` recipes
    /// (`TileEntityAdvancedElectricMachine`: `crate::advanced`).
    OsmiumCompressor,
    /// `mekanism:purification_chamber`, `mekanism:purifying` recipes.
    PurificationChamber,
    /// `mekanism:chemical_injection_chamber`, `mekanism:injecting` recipes.
    ChemicalInjectionChamber,
    /// `mekanism:chemical_oxidizer`, `mekanism:oxidizing` recipes
    /// (`TileEntityChemicalOxidizer`: an item in, a chemical out; `crate::chemical_machine`).
    ChemicalOxidizer,
    /// `mekanism:chemical_infuser`, `mekanism:chemical_infusing` recipes
    /// (`TileEntityChemicalInfuser`: two chemicals in, one out, operations a tick).
    ChemicalInfuser,
    /// `mekanism:chemical_dissolution_chamber`, `mekanism:dissolution` recipes
    /// (an item and a chemical a tick in, a chemical out; `crate::ore`).
    ChemicalDissolutionChamber,
    /// `mekanism:chemical_washer`, `mekanism:washing` recipes (a fluid and a
    /// chemical in, a chemical out, operations a tick; `crate::ore`).
    ChemicalWasher,
    /// `mekanism:chemical_crystallizer`, `mekanism:crystallizing` recipes (a
    /// chemical in, an item out; `crate::ore`).
    ChemicalCrystallizer,
    /// `mekanism:rotary_condensentrator`, `mekanism:rotary` recipes (a chemical
    /// into a fluid or back by its mode, operations a tick; `crate::rotary`).
    RotaryCondensentrator,
    /// `mekanism:combiner`, `mekanism:combining` recipes (`TileEntityCombiner`:
    /// a main and an extra item in, an item out; `crate::factory`).
    Combiner,
    /// `mekanism:precision_sawmill`, `mekanism:sawing` recipes
    /// (`TileEntityPrecisionSawmill`: an item in, an item and a chance of a
    /// secondary one out; `crate::factory`).
    PrecisionSawmill,
}

/// Where a machine keeps its items: Mekanism's own slot order (input,
/// output, energy; the infuser's infusion slot first), then the upgrade
/// component's input and output slots, which Mekanism keeps apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slots {
    /// The `EXTRA` slot: the infuser's infusion slot, the combiner's extra input.
    pub infusion: Option<usize>,
    /// The separator's fluid slot is its `input`; its outputs are chemical
    /// item slots (left `output`, right `output2`).
    pub input: usize,
    pub output: usize,
    pub output2: Option<usize>,
    pub energy: usize,
    pub upgrade: usize,
    pub upgrade_out: usize,
    pub count: usize,
}

impl MachineKind {
    pub const ALL: [Self; 16] = [
        Self::EnrichmentChamber,
        Self::Crusher,
        Self::EnergizedSmelter,
        Self::MetallurgicInfuser,
        Self::ElectrolyticSeparator,
        Self::OsmiumCompressor,
        Self::PurificationChamber,
        Self::ChemicalInjectionChamber,
        Self::ChemicalOxidizer,
        Self::ChemicalInfuser,
        Self::ChemicalDissolutionChamber,
        Self::ChemicalWasher,
        Self::ChemicalCrystallizer,
        Self::RotaryCondensentrator,
        Self::Combiner,
        Self::PrecisionSawmill,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::EnrichmentChamber => "mekanism:enrichment_chamber",
            Self::Crusher => "mekanism:crusher",
            Self::EnergizedSmelter => "mekanism:energized_smelter",
            Self::MetallurgicInfuser => "mekanism:metallurgic_infuser",
            Self::ElectrolyticSeparator => "mekanism:electrolytic_separator",
            Self::OsmiumCompressor => "mekanism:osmium_compressor",
            Self::PurificationChamber => "mekanism:purification_chamber",
            Self::ChemicalInjectionChamber => "mekanism:chemical_injection_chamber",
            Self::ChemicalOxidizer => "mekanism:chemical_oxidizer",
            Self::ChemicalInfuser => "mekanism:chemical_infuser",
            Self::ChemicalDissolutionChamber => "mekanism:chemical_dissolution_chamber",
            Self::ChemicalWasher => "mekanism:chemical_washer",
            Self::ChemicalCrystallizer => "mekanism:chemical_crystallizer",
            Self::RotaryCondensentrator => "mekanism:rotary_condensentrator",
            Self::Combiner => "mekanism:combiner",
            Self::PrecisionSawmill => "mekanism:precision_sawmill",
        }
    }

    pub fn of_block(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.id() == id)
    }

    /// Its key in `machine-usage.toml` and `machine-storage.toml`.
    pub fn config_key(self) -> &'static str {
        match self {
            Self::EnrichmentChamber => "enrichmentChamber",
            Self::Crusher => "crusher",
            Self::EnergizedSmelter => "energizedSmelter",
            Self::MetallurgicInfuser => "metallurgicInfuser",
            Self::ElectrolyticSeparator => "electrolyticSeparator",
            Self::OsmiumCompressor => "osmiumCompressor",
            Self::PurificationChamber => "purificationChamber",
            Self::ChemicalInjectionChamber => "chemicalInjectionChamber",
            Self::ChemicalOxidizer => "chemicalOxidizer",
            Self::ChemicalInfuser => "chemicalInfuser",
            Self::ChemicalDissolutionChamber => "chemicalDissolutionChamber",
            Self::ChemicalWasher => "chemicalWasher",
            Self::ChemicalCrystallizer => "chemicalCrystallizer",
            Self::RotaryCondensentrator => "rotaryCondensentrator",
            Self::Combiner => "combiner",
            Self::PrecisionSawmill => "precisionSawmill",
        }
    }

    /// The recipe type it reads.
    pub fn recipe_type(self) -> &'static str {
        match self {
            Self::EnrichmentChamber => "mekanism:enriching",
            Self::Crusher => "mekanism:crushing",
            Self::EnergizedSmelter => "minecraft:smelting",
            Self::MetallurgicInfuser => "mekanism:metallurgic_infusing",
            Self::ElectrolyticSeparator => "mekanism:separating",
            Self::OsmiumCompressor => "mekanism:compressing",
            Self::PurificationChamber => "mekanism:purifying",
            Self::ChemicalInjectionChamber => "mekanism:injecting",
            Self::ChemicalOxidizer => "mekanism:oxidizing",
            Self::ChemicalInfuser => "mekanism:chemical_infusing",
            Self::ChemicalDissolutionChamber => "mekanism:dissolution",
            Self::ChemicalWasher => "mekanism:washing",
            Self::ChemicalCrystallizer => "mekanism:crystallizing",
            Self::RotaryCondensentrator => "mekanism:rotary",
            Self::Combiner => "mekanism:combining",
            Self::PrecisionSawmill => "mekanism:sawing",
        }
    }

    /// `UsageConfig`'s default Joules a tick (the separator's comes from
    /// hydrogen's fuel instead: `crate::separator::energy_per_tick`).
    pub fn default_usage(self) -> u64 {
        match self {
            Self::ElectrolyticSeparator | Self::ChemicalInjectionChamber | Self::ChemicalDissolutionChamber | Self::ChemicalCrystallizer => 400,
            Self::OsmiumCompressor => 100,
            Self::PurificationChamber | Self::ChemicalOxidizer | Self::ChemicalInfuser | Self::ChemicalWasher => 200,
            _ => 50,
        }
    }

    /// `StorageConfig`'s default Joules held.
    pub fn default_storage(self) -> u64 {
        match self {
            Self::ElectrolyticSeparator | Self::ChemicalInjectionChamber | Self::ChemicalDissolutionChamber | Self::ChemicalCrystallizer => 160_000,
            Self::OsmiumCompressor | Self::PurificationChamber | Self::ChemicalOxidizer | Self::ChemicalInfuser | Self::ChemicalWasher => 80_000,
            Self::Combiner => 40_000,
            _ => 20_000,
        }
    }

    pub fn separates(self) -> bool {
        self == Self::ElectrolyticSeparator
    }

    /// A machine of `TileEntityProgressMachine`, whose recipes take ticks.
    pub fn progresses(self) -> bool {
        !self.separates() && !self.infuses_chemicals() && !self.washes() && !self.rotates()
    }

    /// The advanced machines (`TileEntityAdvancedElectricMachine`, `crate::advanced`):
    /// an item and a chemical in, an item out.
    pub fn advanced(self) -> bool {
        matches!(self, Self::OsmiumCompressor | Self::PurificationChamber | Self::ChemicalInjectionChamber)
    }

    /// The purification and injection chambers: they take chemical upgrades
    /// (`DEFAULT_ADVANCED_MACHINE_UPGRADES`) and draw their chemical at random
    /// (`useStatisticalMechanics`, with `randomizedConsumption`).
    pub fn takes_chemical_upgrades(self) -> bool {
        matches!(self, Self::PurificationChamber | Self::ChemicalInjectionChamber | Self::ChemicalDissolutionChamber)
    }

    pub fn dissolves(self) -> bool {
        self == Self::ChemicalDissolutionChamber
    }

    pub fn washes(self) -> bool {
        self == Self::ChemicalWasher
    }

    pub fn crystallizes(self) -> bool {
        self == Self::ChemicalCrystallizer
    }

    pub fn rotates(self) -> bool {
        self == Self::RotaryCondensentrator
    }

    /// The machines of slice 5 (`crate::ore`, `crate::rotary`), whose ticks
    /// are the runtime's `mek_ore_machines`.
    pub fn ore_chain(self) -> bool {
        self.dissolves() || self.washes() || self.crystallizes() || self.rotates()
    }

    pub fn oxidizes(self) -> bool {
        self == Self::ChemicalOxidizer
    }

    pub fn infuses_chemicals(self) -> bool {
        self == Self::ChemicalInfuser
    }

    /// The upgrades it takes (`AttributeUpgradeSupport`).
    pub fn upgrades(self) -> &'static [Upgrade] {
        if self.takes_chemical_upgrades() { &ADVANCED_MACHINE_UPGRADES } else { &MACHINE_UPGRADES }
    }

    /// Whether its fluid config ejects by default: the rotary condensentrator's
    /// (`ROTARY`'s fluids `OUT_EJECT`, out of its output sides while it
    /// condensentrates); the separator's and the washer's fluid tanks are
    /// inputs that their ejectors leave out (`setOutputData(ITEM, CHEMICAL)`).
    pub fn fluids_eject(self) -> bool {
        self.rotates()
    }

    /// Whether its chemical config can eject (`setCanEject`): the separator's,
    /// the oxidizer's and the chemical infuser's; the metallurgic infuser's
    /// and the advanced machines' never.
    pub fn chemicals_eject(self) -> bool {
        matches!(
            self,
            Self::ElectrolyticSeparator | Self::ChemicalOxidizer | Self::ChemicalInfuser | Self::ChemicalDissolutionChamber | Self::ChemicalWasher | Self::RotaryCondensentrator
        )
    }

    pub fn infuses(self) -> bool {
        self == Self::MetallurgicInfuser
    }

    /// The Combiner and the Precision Sawmill, which `crate::factory` runs
    /// with the factories (`ProcessKind`).
    pub fn processes_like_a_factory(self) -> bool {
        matches!(self, Self::Combiner | Self::PrecisionSawmill)
    }

    /// Whether its items have an `EXTRA` slot (`setupItemIOExtraConfig`): the
    /// infuser's infusion slot, the combiner's extra input, an advanced
    /// machine's chemical slot.
    pub fn has_extra_slot(self) -> bool {
        matches!(self, Self::MetallurgicInfuser | Self::Combiner | Self::ChemicalDissolutionChamber) || self.advanced()
    }

    /// Whether its block shows `active` (`AttributeStateActive`): the
    /// infuser's and the separator's models have no such state.
    pub fn shows_active(self) -> bool {
        !self.infuses() && !self.separates() && !self.oxidizes() && !self.infuses_chemicals() && !self.ore_chain()
    }

    pub fn slots(self) -> Slots {
        if self.washes() {
            // Its fluid slot, the fluid slot's output, the slurry output's drain
            // slot and energy (`TileEntityChemicalWasher.getInitialInventory`).
            return Slots { infusion: None, input: 0, output: 2, output2: Some(1), energy: 3, upgrade: 4, upgrade_out: 5, count: 6 };
        }
        if self.rotates() {
            // The chemical tank's drain (`input`) and fill (`infusion`) slots,
            // the fluid tank's slot (`output2`) and its output, energy.
            return Slots { infusion: Some(1), input: 0, output: 3, output2: Some(2), energy: 4, upgrade: 5, upgrade_out: 6, count: 7 };
        }
        if self.infuses() || self.dissolves() {
            Slots { infusion: Some(0), input: 1, output: 2, output2: None, energy: 3, upgrade: 4, upgrade_out: 5, count: 6 }
        } else if self.separates() || self == Self::PrecisionSawmill {
            Slots { infusion: None, input: 0, output: 1, output2: Some(2), energy: 3, upgrade: 4, upgrade_out: 5, count: 6 }
        } else if self.infuses_chemicals() || self.advanced() || self == Self::Combiner {
            // The chemical infuser's left (`input`) and right (`infusion`) fill
            // slots, then its output's drain slot and energy (`getInitialInventory`);
            // an advanced machine's input, chemical slot, output and energy; the
            // combiner's main input, extra input, output, energy
            // (`TileEntityCombiner.getInitialInventory`).
            Slots { infusion: Some(1), input: 0, output: 2, output2: None, energy: 3, upgrade: 4, upgrade_out: 5, count: 6 }
        } else {
            Slots { infusion: None, input: 0, output: 1, output2: None, energy: 2, upgrade: 3, upgrade_out: 4, count: 5 }
        }
    }

    /// The item side types it has slots for (`ConfigInfo.supports`):
    /// `setupItemIOConfig`, and `setupItemIOExtraConfig` with the infuser's
    /// infusion slot as `EXTRA`.
    pub fn item_supports(self, data: DataType) -> bool {
        if self.separates() {
            // `INPUT`, `OUTPUT_1`, `OUTPUT_2`, `INPUT_OUTPUT` and `ENERGY` slot infos.
            return matches!(data, DataType::None | DataType::Input | DataType::Output1 | DataType::Output2 | DataType::InputOutput | DataType::Energy);
        }
        if self.infuses_chemicals() {
            return matches!(data, DataType::None | DataType::Input1 | DataType::Input2 | DataType::Output | DataType::InputOutput | DataType::Energy);
        }
        match data {
            DataType::None | DataType::Input | DataType::Output | DataType::InputOutput | DataType::Energy => true,
            DataType::Extra => self.has_extra_slot(),
            _ => false,
        }
    }

    /// The chemical side types it has tanks for: the infuser's
    /// `setupIOConfig(CHEMICAL, tank, RIGHT)`, the separator's `OUTPUT_1`
    /// (left tank) and `OUTPUT_2` (right tank).
    pub fn chemical_supports(self, data: DataType) -> bool {
        match self {
            Self::MetallurgicInfuser => chemical_supports(data),
            Self::ElectrolyticSeparator => matches!(data, DataType::None | DataType::Output1 | DataType::Output2),
            // `setupIOConfig(CHEMICAL, tank, RIGHT)`; the chambers' `setupInputConfig`
            // (`allowExtractingChemical` is off while they draw at random).
            Self::OsmiumCompressor => chemical_supports(data),
            Self::PurificationChamber | Self::ChemicalInjectionChamber => matches!(data, DataType::None | DataType::Input),
            // `setupOutputConfig(CHEMICAL, tank, RIGHT)`.
            Self::ChemicalOxidizer => matches!(data, DataType::None | DataType::Output),
            Self::ChemicalInfuser => matches!(data, DataType::None | DataType::Input1 | DataType::Input2 | DataType::Output | DataType::InputOutput),
            // `setupIOConfig(CHEMICAL, input, output, RIGHT)`; the rotary's one tank.
            Self::ChemicalDissolutionChamber | Self::ChemicalWasher | Self::RotaryCondensentrator => chemical_supports(data),
            // `setupInputConfig(CHEMICAL, inputTank)`.
            Self::ChemicalCrystallizer => matches!(data, DataType::None | DataType::Input),
            _ => false,
        }
    }

    /// The item config its item comes with (`AttachedSideConfig`): `MACHINE`
    /// (right output, back energy, the rest input) and the infuser's
    /// `EXTRA_MACHINE` (its bottom the infusion slot).
    pub fn default_items(self) -> [DataType; 6] {
        if self.separates() {
            // `TWO_OUTPUT_ITEM`.
            return RelativeSide::ALL.map(|side| match side {
                RelativeSide::Front => DataType::Input,
                RelativeSide::Left => DataType::Output1,
                RelativeSide::Right => DataType::Output2,
                RelativeSide::Back => DataType::Energy,
                _ => DataType::None,
            });
        }
        if self.infuses_chemicals() {
            // `TWO_INPUT_ITEM`.
            return RelativeSide::ALL.map(|side| match side {
                RelativeSide::Left => DataType::Input1,
                RelativeSide::Right => DataType::Input2,
                RelativeSide::Front => DataType::Output,
                RelativeSide::Back => DataType::Energy,
                _ => DataType::None,
            });
        }
        RelativeSide::ALL.map(|side| match side {
            RelativeSide::Right => DataType::Output,
            RelativeSide::Back => DataType::Energy,
            RelativeSide::Bottom if self.has_extra_slot() => DataType::Extra,
            _ => DataType::Input,
        })
    }

    /// The chemical config its item comes with: the infuser's `OUT_NO_EJECT`
    /// (right output, the rest input), the separator's `TWO_OUTPUT` (left
    /// `OUTPUT_1`, right `OUTPUT_2`, ejecting).
    pub fn default_chemicals(self) -> Option<[DataType; 6]> {
        match self {
            Self::MetallurgicInfuser => Some(RelativeSide::ALL.map(|s| if s == RelativeSide::Right { DataType::Output } else { DataType::Input })),
            Self::ElectrolyticSeparator => Some(RelativeSide::ALL.map(|s| match s {
                RelativeSide::Left => DataType::Output1,
                RelativeSide::Right => DataType::Output2,
                _ => DataType::None,
            })),
            // `ADVANCED_MACHINE`'s `OUT_NO_EJECT`.
            Self::OsmiumCompressor => Some(RelativeSide::ALL.map(|s| if s == RelativeSide::Right { DataType::Output } else { DataType::Input })),
            // `ADVANCED_MACHINE_INPUT_ONLY`.
            Self::PurificationChamber | Self::ChemicalInjectionChamber => Some([DataType::Input; 6]),
            // `CHEMICAL_OUT_MACHINE`'s `RIGHT_OUTPUT`, ejecting.
            Self::ChemicalOxidizer => Some(RelativeSide::ALL.map(|s| if s == RelativeSide::Right { DataType::Output } else { DataType::None })),
            // `TWO_INPUT_AND_OUT`, ejecting.
            Self::ChemicalInfuser => Some(RelativeSide::ALL.map(|s| match s {
                RelativeSide::Left => DataType::Input1,
                RelativeSide::Right => DataType::Input2,
                RelativeSide::Front => DataType::Output,
                _ => DataType::None,
            })),
            // `DISSOLUTION`'s and `WASHER`'s `OUT_EJECT`: right output, the rest input, ejecting.
            Self::ChemicalDissolutionChamber | Self::ChemicalWasher => {
                Some(RelativeSide::ALL.map(|s| if s == RelativeSide::Right { DataType::Output } else { DataType::Input }))
            }
            // `CRYSTALLIZER`'s `INPUT_ONLY`.
            Self::ChemicalCrystallizer => Some([DataType::Input; 6]),
            // `ROTARY`'s `OUT_EJECT_LEFT`.
            Self::RotaryCondensentrator => Some(RelativeSide::ALL.map(|s| if s == RelativeSide::Left { DataType::Output } else { DataType::Input })),
            _ => None,
        }
    }

    /// Its fluid config: the separator's and the washer's `INPUT_ONLY`, the
    /// rotary condensentrator's `OUT_EJECT` (right output, the rest input).
    pub fn default_fluids(self) -> Option<[DataType; 6]> {
        if self.rotates() {
            return Some(RelativeSide::ALL.map(|s| if s == RelativeSide::Right { DataType::Output } else { DataType::Input }));
        }
        (self.separates() || self.washes()).then_some([DataType::Input; 6])
    }
}

/// `setupInputConfig(ENERGY, ...)`: an energy side takes or doesn't.
pub fn energy_supports(data: DataType) -> bool {
    matches!(data, DataType::None | DataType::Input)
}

/// `setupIOConfig(CHEMICAL, tank, RIGHT)`.
pub fn chemical_supports(data: DataType) -> bool {
    matches!(data, DataType::None | DataType::Input | DataType::Output | DataType::InputOutput)
}

/// `mekanism.api.Upgrade`, by ordinal (a save's `type`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Upgrade {
    Speed,
    Energy,
    Filter,
    Chemical,
    Muffling,
    Anchor,
    StoneGenerator,
}

impl Upgrade {
    pub const ALL: [Self; 7] = [
        Self::Speed,
        Self::Energy,
        Self::Filter,
        Self::Chemical,
        Self::Muffling,
        Self::Anchor,
        Self::StoneGenerator,
    ];

    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// `BY_ID`, which wraps.
    pub fn by_id(id: i32) -> Self {
        Self::ALL[id.rem_euclid(Self::ALL.len() as i32) as usize]
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Speed => "speed",
            Self::Energy => "energy",
            Self::Filter => "filter",
            Self::Chemical => "chemical",
            Self::Muffling => "muffling",
            Self::Anchor => "anchor",
            Self::StoneGenerator => "stone_generator",
        }
    }

    /// `getMax`.
    pub fn max(self) -> u32 {
        match self {
            Self::Speed | Self::Energy | Self::Chemical => 8,
            _ => 1,
        }
    }

    /// Its item (`ItemUpgrade`): `mekanism:upgrade_speed`.
    pub fn item(self) -> String {
        format!("mekanism:upgrade_{}", self.name())
    }

    pub fn of_item(item: &str) -> Option<Self> {
        let name = item.strip_prefix("mekanism:upgrade_")?;
        Self::ALL.into_iter().find(|u| u.name() == name)
    }
}

/// `AttributeUpgradeSupport.DEFAULT_MACHINE_UPGRADES`, which all four take.
pub const MACHINE_UPGRADES: [Upgrade; 3] = [Upgrade::Speed, Upgrade::Energy, Upgrade::Muffling];
/// `DEFAULT_ADVANCED_MACHINE_UPGRADES`: the purification and injection chambers'.
pub const ADVANCED_MACHINE_UPGRADES: [Upgrade; 4] = [Upgrade::Speed, Upgrade::Energy, Upgrade::Muffling, Upgrade::Chemical];

/// The upgrades a machine has installed (`TileComponentUpgrade.upgrades`), by
/// ordinal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Upgrades(pub [u32; 7]);

impl Upgrades {
    pub fn get(&self, upgrade: Upgrade) -> u32 {
        self.0[upgrade as usize]
    }

    pub fn set(&mut self, upgrade: Upgrade, count: u32) {
        self.0[upgrade as usize] = count.min(upgrade.max());
    }

    /// `fractionUpgrades`: installed over the most it takes.
    fn fraction(&self, upgrade: Upgrade) -> f64 {
        f64::from(self.get(upgrade)) / f64::from(upgrade.max())
    }

    /// `addUpgrades(upgrade, installed, available)`: how many of `available`
    /// go in.
    pub fn install(&mut self, upgrade: Upgrade, available: u32) -> u32 {
        let installed = self.get(upgrade);
        let add = upgrade.max().saturating_sub(installed).min(available);
        self.set(upgrade, installed + add);
        add
    }

    /// `getTicks(tile, def)`: `max(1, (int) (def * multiplier ^ -speed/8))`.
    pub fn ticks(&self, base: u32, multiplier: f64) -> u32 {
        let ticks = f64::from(base) * multiplier.powf(-self.fraction(Upgrade::Speed));
        clamp_to_int(ticks).max(1) as u32
    }

    /// `getOperationsPerTick(tile, defTicks, 1)`: more than one operation a
    /// tick only when the ticks would be fewer than one.
    pub fn operations_per_tick(&self, base: u32, multiplier: f64) -> u32 {
        let ticks = f64::from(base) * multiplier.powf(-self.fraction(Upgrade::Speed));
        if ticks >= 1.0 { 1 } else { clamp_to_int((1.0 / ticks).max(1.0)).max(1) as u32 }
    }

    /// `getEnergyPerTick(tile, def)`: `ceil(def * multiplier ^ (2 speed/8 - energy/8))`.
    pub fn energy_per_tick(&self, base: u64, multiplier: f64) -> u64 {
        let exponent = 2.0 * self.fraction(Upgrade::Speed) - self.fraction(Upgrade::Energy);
        crate::clamp_to_long((base as f64 * multiplier.powf(exponent)).ceil()).max(0) as u64
    }

    /// `getMaxEnergy(tile, def)`: `(long) (def * multiplier ^ energy/8)`.
    pub fn max_energy(&self, base: u64, multiplier: f64) -> u64 {
        crate::clamp_to_long(base as f64 * multiplier.powf(self.fraction(Upgrade::Energy))).max(0) as u64
    }
}

/// `MathUtils.clampToInt(double)`.
fn clamp_to_int(d: f64) -> i32 {
    if d.is_nan() {
        0
    } else if d < f64::from(i32::MAX) {
        d as i32
    } else {
        i32::MAX
    }
}

/// `TileComponentUpgrade.tickServer`: what its upgrade slot does this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpgradeSlot {
    /// `upgradeTicks`, the install's progress (not saved).
    pub ticks: u32,
    /// `canCheckUpgrades`: the slot changed since it was last looked at.
    pub check: bool,
}

impl Default for UpgradeSlot {
    fn default() -> Self {
        Self { ticks: 0, check: true }
    }
}

impl UpgradeSlot {
    /// One tick with `stack` (the upgrade and how many) in the slot: how many
    /// to take out of it, installed into `upgrades`.
    pub fn tick(&mut self, upgrades: &mut Upgrades, supported: &[Upgrade], stack: Option<(Upgrade, u32)>) -> u32 {
        if !self.check {
            return 0;
        }
        let mut taken = 0;
        if let Some((upgrade, count)) = stack
            && count > 0
            && supported.contains(&upgrade)
            && upgrades.get(upgrade) < upgrade.max()
        {
            if self.ticks < UPGRADE_TICKS {
                self.ticks += 1;
                return 0;
            }
            taken = upgrades.install(upgrade, count);
        }
        self.ticks = 0;
        self.check = false;
        taken
    }
}

/// What a machine's inputs allow this tick (`IInputHandler.calculateOperationsCanSupport`
/// after `getRecipeInput`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inputs {
    /// An empty input, or one the recipe doesn't take (`mismatchedRecipe`):
    /// the progress starts over.
    Mismatched,
    /// Less than the recipe takes (`resetProgress`): the progress starts over.
    Short,
    /// Enough for this many operations.
    Enough(u32),
}

impl Inputs {
    /// An item input: `count` held where the recipe takes `needed`; the
    /// infuser also needs `chemical` (held, needed) of the right type.
    pub fn of(count: u32, needed: u32) -> Self {
        if count == 0 || needed == 0 {
            return Self::Mismatched;
        }
        match count / needed {
            0 => Self::Short,
            n => Self::Enough(n),
        }
    }

    /// Both of a two-input recipe (`TwoInputCachedRecipe`): the item, then
    /// the chemical.
    pub fn both(self, other: Self) -> Self {
        match (self, other) {
            (Self::Enough(a), Self::Enough(b)) => Self::Enough(a.min(b)),
            (Self::Mismatched, _) | (_, Self::Mismatched) => Self::Mismatched,
            _ => Self::Short,
        }
    }
}

/// How many operations' output `out_count` of an item fits in the output
/// slot (`OutputHelper.calculateOperationsCanSupport`): `held` is what the
/// slot holds (its count, and whether it is the same item), `max_stack` the
/// output's stack size (a slot holds up to 64).
pub fn output_operations(held: Option<(u32, bool)>, out_count: u32, max_stack: u32) -> u32 {
    if out_count == 0 {
        return u32::MAX;
    }
    let limit = max_stack.min(64);
    let room = match held {
        None => limit,
        Some((_, false)) => 0,
        Some((count, true)) => limit.saturating_sub(count),
    };
    room / out_count
}

/// What `CachedRecipe.process` did in one tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ran {
    /// Operations this tick (at most one but for speed beyond a tick an
    /// operation); none: idle.
    pub operations: u32,
    /// The operations finished this tick: take the inputs, add the output.
    pub finished: bool,
    /// Joules taken from the machine's store.
    pub energy: u64,
}

impl Ran {
    pub fn active(&self) -> bool {
        self.operations > 0
    }
}

/// One tick of `CachedRecipe.process` with the machine's cached recipe:
/// with less than a tick's energy it waits (`NOT_ENOUGH_ENERGY`, progress
/// kept); an input that no longer fits the recipe, or too little of it, starts
/// the progress over; a full output waits. Otherwise it takes `per_tick`
/// Joules an operation and moves on a tick, and on the last one finishes.
/// `baseline` is `setBaselineMaxOperations` (the infuser's
/// `getOperationsPerTick`; one for the electric machines). `room` is
/// [`output_operations`]. Mekanism pauses a recipe on an error until its
/// slots or energy change; checking again every tick comes to the same.
#[allow(clippy::too_many_arguments)]
pub fn process(
    progress: &mut u32,
    required: u32,
    energy: &mut EnergyContainer,
    per_tick: u64,
    baseline: u32,
    inputs: Inputs,
    room: u32,
    can_function: bool,
) -> Ran {
    if !can_function {
        return Ran::default();
    }
    let mut max_for_energy = baseline;
    if per_tick != 0 {
        let affordable = (energy.stored / per_tick).min(i32::MAX as u64) as u32;
        if affordable == 0 {
            return Ran::default();
        }
        max_for_energy = affordable;
    }
    let operations = match inputs {
        Inputs::Mismatched | Inputs::Short => {
            *progress = 0;
            return Ran::default();
        }
        Inputs::Enough(n) => baseline.min(n),
    };
    let operations = operations.min(room).min(max_for_energy);
    if operations == 0 {
        return Ran::default();
    }
    let used = energy.extract(per_tick.saturating_mul(u64::from(operations)), false, Automation::Internal);
    *progress += 1;
    let finished = *progress >= required;
    if finished {
        *progress = 0;
    }
    Ran { operations, finished, energy: used }
}

/// `loadSavedOperatingTicks`: saved progress carries over when a recipe is
/// first cached, if it is short of the recipe's ticks.
pub fn resumed(saved: u32, required: u32) -> u32 {
    if saved > 0 && saved < required { saved } else { 0 }
}

/// The ejector's sides for items (`getSidesForData`): every face whose type
/// can output (`DataType.canOutput`), in `Direction` order. Only the output
/// slot gives to the ejector (the input slot refuses external extraction).
pub fn eject_faces(items: &[DataType; 6], facing: Direction) -> Vec<Direction> {
    Direction::ALL
        .into_iter()
        .filter(|&d| {
            let side = RelativeSide::from_directions(facing, d);
            matches!(items[side.index()], DataType::Output | DataType::InputOutput | DataType::Output1 | DataType::Output2)
        })
        .collect()
}

/// A machine's side configuration (`TileComponentConfig`, the ejector's
/// switch): energy, items, chemicals (the infuser's, the separator's) and
/// fluids (the separator's) by `RelativeSide`. Energy sides and the infuser's
/// chemical sides never eject (`setCanEject(false)`); the separator's
/// chemicals do by default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SideConfig {
    pub energy: [DataType; 6],
    pub items: [DataType; 6],
    pub chemicals: Option<[DataType; 6]>,
    pub fluids: Option<[DataType; 6]>,
    pub items_eject: bool,
    pub chemicals_eject: bool,
    /// The fluid config's ejecting (`eject1`): the rotary condensentrator's.
    pub fluids_eject: bool,
}

impl SideConfig {
    /// The machine's item's config (`AttachedSideConfig`, `AttachedEjector.DEFAULT`):
    /// energy in on every side, items as `default_items`, not ejecting.
    pub fn new(kind: MachineKind) -> Self {
        Self {
            energy: [DataType::Input; 6],
            items: kind.default_items(),
            chemicals: kind.default_chemicals(),
            fluids: kind.default_fluids(),
            items_eject: false,
            chemicals_eject: kind.chemicals_eject(),
            fluids_eject: kind.fluids_eject(),
        }
    }

    /// A side configuration button (`PacketSideData`, `PacketEjectConfiguration`,
    /// `PacketBatchConfiguration`, as `CubeState::press`): whether anything changed.
    pub fn press(&mut self, kind: MachineKind, button: crate::menu::Button) -> bool {
        use crate::menu::{Button, Kind};
        let before = *self;
        let item_supports = |d: DataType| kind.item_supports(d);
        let chemical_kind_supports = |d: DataType| kind.chemical_supports(d);
        let (sides, supports): (&mut [DataType; 6], &dyn Fn(DataType) -> bool) = match button {
            Button::Page(_) => return false,
            Button::Side { kind: k, .. } | Button::Eject(k) | Button::Clear { kind: k, .. } => match k {
                Kind::Energy => (&mut self.energy, &energy_supports),
                Kind::Items => (&mut self.items, &item_supports),
                Kind::Chemicals => match self.chemicals.as_mut() {
                    Some(c) => (c, &chemical_kind_supports),
                    None => return false,
                },
            },
        };
        match button {
            Button::Side { side, forward, .. } => {
                if let Some(t) = sides.get_mut(side) {
                    *t = if forward { t.next(supports) } else { t.previous(supports) };
                }
            }
            Button::Eject(Kind::Items) => self.items_eject = !self.items_eject,
            Button::Eject(Kind::Chemicals) if kind.chemicals_eject() => self.chemicals_eject = !self.chemicals_eject,
            Button::Clear { forward, .. } => {
                let shared = sides.iter().all(|t| *t == sides[0]).then_some(sides[0]);
                let target = match shared {
                    Some(t) if forward => t.next(supports),
                    Some(t) => t.previous(supports),
                    None => DataType::None,
                };
                *sides = [target; 6];
            }
            _ => {}
        }
        *self != before
    }

    /// Whether `face` of a machine facing `facing` takes energy.
    pub fn takes_energy(&self, facing: Direction, face: Direction) -> bool {
        self.energy[RelativeSide::from_directions(facing, face).index()] == DataType::Input
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ATM10's `general.toml` `maxUpgradeMultiplier`.
    const ATM10: f64 = 16.0;

    fn with(speed: u32, energy: u32) -> Upgrades {
        let mut u = Upgrades::default();
        u.set(Upgrade::Speed, speed);
        u.set(Upgrade::Energy, energy);
        u
    }

    #[test]
    fn upgrades_scale_ticks_energy_and_capacity_as_on_atm10() {
        // Measured on NeoForge 1.21.1 with Mekanism 10.7.19 and ATM10's configs.
        let none = Upgrades::default();
        assert_eq!((none.ticks(200, ATM10), none.energy_per_tick(50, ATM10), none.max_energy(20_000, ATM10)), (200, 50, 20_000));
        let speed = with(8, 0);
        assert_eq!((speed.ticks(200, ATM10), speed.energy_per_tick(50, ATM10), speed.max_energy(20_000, ATM10)), (12, 12_800, 20_000));
        let energy = with(0, 8);
        assert_eq!((energy.ticks(200, ATM10), energy.energy_per_tick(50, ATM10), energy.max_energy(20_000, ATM10)), (200, 4, 320_000));
        let half = with(4, 0);
        assert_eq!((half.ticks(200, ATM10), half.energy_per_tick(50, ATM10)), (50, 800));
        let both = with(8, 8);
        assert_eq!((both.ticks(200, ATM10), both.energy_per_tick(50, ATM10), both.max_energy(20_000, ATM10)), (12, 800, 320_000));
        assert_eq!(speed.operations_per_tick(200, ATM10), 1);
        // Mekanism's own multiplier, 10: 20 ticks.
        assert_eq!(speed.ticks(200, 10.0), 20);
        // A recipe of 1 tick base with 8 speed: 16 operations a tick.
        assert_eq!(speed.operations_per_tick(1, ATM10), 16);
        assert_eq!(speed.ticks(1, ATM10), 1);
    }

    #[test]
    fn the_upgrade_slot_installs_on_the_twenty_first_tick() {
        let mut upgrades = Upgrades::default();
        let mut slot = UpgradeSlot::default();
        for _ in 0..20 {
            assert_eq!(slot.tick(&mut upgrades, &MACHINE_UPGRADES, Some((Upgrade::Speed, 10))), 0);
        }
        assert_eq!(slot.tick(&mut upgrades, &MACHINE_UPGRADES, Some((Upgrade::Speed, 10))), 8, "eight of ten go in");
        assert_eq!(upgrades.get(Upgrade::Speed), 8);
        assert_eq!(slot.tick(&mut upgrades, &MACHINE_UPGRADES, Some((Upgrade::Speed, 2))), 0, "the slot didn't change since");
        slot.check = true;
        assert_eq!(slot.tick(&mut upgrades, &MACHINE_UPGRADES, Some((Upgrade::Speed, 2))), 0, "full: nothing more");
        assert!(!slot.check);
        slot.check = true;
        assert_eq!(slot.tick(&mut upgrades, &MACHINE_UPGRADES, Some((Upgrade::Chemical, 2))), 0, "not one of a machine's");
        assert_eq!(Upgrade::of_item("mekanism:upgrade_muffling"), Some(Upgrade::Muffling));
        assert_eq!(Upgrade::Muffling.max(), 1);
        assert_eq!(Upgrade::by_id(1), Upgrade::Energy);
    }

    fn full() -> EnergyContainer {
        let mut c = EnergyContainer::basic(20_000);
        c.stored = 20_000;
        c
    }

    #[test]
    fn a_recipe_takes_its_ticks_of_energy_and_finishes_on_the_last() {
        let (mut progress, mut energy) = (0, full());
        for tick in 1..200 {
            let ran = process(&mut progress, 200, &mut energy, 50, 1, Inputs::of(1, 1), output_operations(None, 1, 64), true);
            assert_eq!((ran.operations, ran.finished, progress), (1, false, tick));
        }
        let ran = process(&mut progress, 200, &mut energy, 50, 1, Inputs::of(1, 1), 64, true);
        assert!(ran.finished && ran.active());
        assert_eq!((progress, energy.stored), (0, 10_000), "10 000 J, as on Java");
    }

    #[test]
    fn short_energy_waits_and_a_changed_input_starts_over() {
        let (mut progress, mut energy) = (10, EnergyContainer::basic(20_000));
        energy.stored = 30;
        let ran = process(&mut progress, 200, &mut energy, 50, 1, Inputs::Mismatched, 64, true);
        assert!(!ran.active());
        assert_eq!((progress, energy.stored), (10, 30), "less than a tick's energy: nothing moves, nothing is used");
        energy.stored = 20_000;
        process(&mut progress, 200, &mut energy, 50, 1, Inputs::of(1, 1), 64, true);
        assert_eq!(progress, 11, "it goes on where it was");
        process(&mut progress, 200, &mut energy, 50, 1, Inputs::Mismatched, 64, true);
        assert_eq!(progress, 0, "the input taken out: from the start");
        progress = 5;
        process(&mut progress, 200, &mut energy, 50, 1, Inputs::of(1, 2), 64, true);
        assert_eq!(progress, 0, "too little input");
        progress = 5;
        let ran = process(&mut progress, 200, &mut energy, 50, 1, Inputs::of(1, 1), 0, true);
        assert_eq!((ran.active(), progress), (false, 5), "a full output waits");
        let ran = process(&mut progress, 200, &mut energy, 50, 1, Inputs::of(1, 1), 1, false);
        assert_eq!((ran.active(), progress), (false, 5), "redstone off");
        assert_eq!(resumed(5, 200), 5);
        assert_eq!(resumed(200, 12), 0);
    }

    #[test]
    fn the_output_slot_takes_what_fits() {
        assert_eq!(output_operations(None, 6, 64), 10);
        assert_eq!(output_operations(Some((60, true)), 6, 64), 0);
        assert_eq!(output_operations(Some((58, true)), 6, 64), 1);
        assert_eq!(output_operations(Some((1, false)), 1, 64), 0);
        assert_eq!(output_operations(Some((15, true)), 1, 16), 1);
        assert_eq!(Inputs::of(3, 1).both(Inputs::of(100, 10)), Inputs::Enough(3));
        assert_eq!(Inputs::of(3, 1).both(Inputs::of(5, 10)), Inputs::Short);
        assert_eq!(Inputs::of(3, 1).both(Inputs::Mismatched), Inputs::Mismatched);
    }

    #[test]
    fn machines_come_with_mekanisms_side_configs() {
        use DataType::*;
        let chamber = MachineKind::EnrichmentChamber;
        assert_eq!(chamber.default_items(), [Input, Input, Output, Energy, Input, Input]);
        assert_eq!(MachineKind::MetallurgicInfuser.default_items(), [Input, Input, Output, Energy, Input, Extra]);
        assert_eq!(MachineKind::MetallurgicInfuser.default_chemicals(), Some([Input, Input, Output, Input, Input, Input]));
        assert!(chamber.default_chemicals().is_none());
        // Facing north, its right is west; facing east, north (as Java's ejector does).
        assert_eq!(eject_faces(&chamber.default_items(), Direction::North), [Direction::West]);
        assert_eq!(eject_faces(&chamber.default_items(), Direction::East), [Direction::North]);
        let cycle: Vec<DataType> =
            std::iter::successors(Some(DataType::None), |t: &DataType| Some(t.next(|d| chamber.item_supports(d)))).take(6).collect();
        assert_eq!(cycle, [DataType::None, Input, Output, InputOutput, Energy, DataType::None]);
        // Buttons: items cycle and eject, energy only takes or not, chemicals don't eject.
        use crate::menu::{Button, Kind};
        let mut config = SideConfig::new(chamber);
        assert!(config.press(chamber, Button::Eject(Kind::Items)));
        assert!(config.items_eject);
        assert!(!config.press(chamber, Button::Eject(Kind::Energy)));
        assert!(config.press(chamber, Button::Side { kind: Kind::Energy, side: 0, forward: true }));
        assert_eq!(config.energy[0], DataType::None);
        assert!(!config.takes_energy(Direction::North, Direction::North));
        assert!(config.takes_energy(Direction::North, Direction::South));
        assert!(!config.press(chamber, Button::Side { kind: Kind::Chemicals, side: 0, forward: true }), "no chemical sides");
        assert!(config.press(chamber, Button::Clear { kind: Kind::Items, forward: true }));
        assert_eq!(config.items, [DataType::None; 6]);
        let infuser = MachineKind::MetallurgicInfuser;
        let mut config = SideConfig::new(infuser);
        assert!(config.press(infuser, Button::Side { kind: Kind::Items, side: 0, forward: false }));
        assert_eq!(config.items[0], DataType::None);
        assert!(config.press(infuser, Button::Side { kind: Kind::Items, side: 0, forward: false }));
        assert_eq!(config.items[0], Extra, "the infuser's items go round to EXTRA");
        assert_eq!(MachineKind::of_block("mekanism:crusher"), Some(MachineKind::Crusher));
        assert_eq!(chamber.slots().count, 5);
        assert_eq!(MachineKind::MetallurgicInfuser.slots().output, 2);
    }

    #[test]
    fn the_separator_has_mekanisms_side_configs() {
        use DataType::*;
        let sep = MachineKind::ElectrolyticSeparator;
        assert_eq!(MachineKind::of_block("mekanism:electrolytic_separator"), Some(sep));
        // Front, left, right, back, top, bottom.
        assert_eq!(sep.default_items(), [Input, Output1, Output2, Energy, None, None]);
        assert_eq!(sep.default_chemicals(), Some([None, Output1, Output2, None, None, None]));
        assert_eq!(sep.default_fluids(), Some([Input; 6]));
        let config = SideConfig::new(sep);
        assert!(config.chemicals_eject && !config.items_eject);
        assert!(!SideConfig::new(MachineKind::MetallurgicInfuser).chemicals_eject);
        let cycle: Vec<DataType> = std::iter::successors(Some(None), |t: &DataType| Some(t.next(|d| sep.chemical_supports(d)))).take(4).collect();
        assert_eq!(cycle, [None, Output1, Output2, None]);
        use crate::menu::{Button, Kind};
        let mut config = SideConfig::new(sep);
        assert!(config.press(sep, Button::Eject(Kind::Chemicals)));
        assert!(!config.chemicals_eject);
        let infuser = MachineKind::MetallurgicInfuser;
        let mut config = SideConfig::new(infuser);
        assert!(!config.press(infuser, Button::Eject(Kind::Chemicals)), "the infuser's chemicals never eject");
        assert_eq!((sep.slots().output2, sep.slots().count, sep.shows_active()), (Some(2), 6, false));
        assert_eq!((sep.default_storage(), sep.config_key()), (160_000, "electrolyticSeparator"));
    }

    #[test]
    fn the_advanced_machines_have_mekanisms_side_configs() {
        use DataType::*;
        let co = MachineKind::OsmiumCompressor;
        let pu = MachineKind::PurificationChamber;
        assert_eq!(MachineKind::of_block("mekanism:chemical_injection_chamber"), Some(MachineKind::ChemicalInjectionChamber));
        // `EXTRA_MACHINE` items; the compressor's `OUT_NO_EJECT`, the chambers' `INPUT_ONLY` chemicals.
        assert_eq!(co.default_items(), [Input, Input, Output, Energy, Input, Extra]);
        assert_eq!(co.default_chemicals(), Some([Input, Input, Output, Input, Input, Input]));
        assert_eq!(pu.default_chemicals(), Some([Input; 6]));
        assert!(!SideConfig::new(co).chemicals_eject && co.shows_active() && co.progresses());
        assert_eq!((co.slots().input, co.slots().infusion, co.slots().output, co.slots().energy, co.slots().count), (0, Some(1), 2, 3, 6));
        let cycle: Vec<DataType> = std::iter::successors(Some(None), |t: &DataType| Some(t.next(|d| pu.chemical_supports(d)))).take(3).collect();
        assert_eq!(cycle, [None, Input, None], "the chambers' chemicals only go in");
        assert_eq!((co.upgrades().len(), pu.upgrades().len()), (3, 4), "the chambers take chemical upgrades");
        assert_eq!(
            [co, pu, MachineKind::ChemicalInjectionChamber].map(|k| (k.default_usage(), k.default_storage())),
            [(100, 80_000), (200, 80_000), (400, 160_000)]
        );
    }

    #[test]
    fn the_oxidizer_and_chemical_infuser_have_mekanisms_side_configs() {
        use DataType::*;
        let ox = MachineKind::ChemicalOxidizer;
        assert_eq!(MachineKind::of_block("mekanism:chemical_oxidizer"), Some(ox));
        // `CHEMICAL_OUT_MACHINE`: items as an electric machine's, chemicals out of the right, ejecting.
        assert_eq!(ox.default_items(), [Input, Input, Output, Energy, Input, Input]);
        assert_eq!(ox.default_chemicals(), Some([None, None, Output, None, None, None]));
        assert!(SideConfig::new(ox).chemicals_eject);
        assert!(ox.progresses() && !ox.shows_active());
        assert_eq!((ox.slots().output, ox.slots().count, ox.default_usage(), ox.default_storage()), (1, 5, 200, 80_000));
        let cycle: Vec<DataType> = std::iter::successors(Some(None), |t: &DataType| Some(t.next(|d| ox.chemical_supports(d)))).take(3).collect();
        assert_eq!(cycle, [None, Output, None]);
        // `CHEMICAL_INFUSING`: `TWO_INPUT_ITEM` and `TWO_INPUT_AND_OUT`.
        let inf = MachineKind::ChemicalInfuser;
        assert_eq!(inf.default_items(), [Output, Input1, Input2, Energy, None, None]);
        assert_eq!(inf.default_chemicals(), Some([Output, Input1, Input2, None, None, None]));
        assert!(SideConfig::new(inf).chemicals_eject && !inf.progresses() && !inf.shows_active());
        assert_eq!((inf.slots().input, inf.slots().infusion, inf.slots().output, inf.slots().energy), (0, Some(1), 2, 3));
        assert_eq!((inf.recipe_type(), inf.config_key()), ("mekanism:chemical_infusing", "chemicalInfuser"));
        use crate::menu::{Button, Kind};
        let mut config = SideConfig::new(inf);
        assert!(config.press(inf, Button::Eject(Kind::Chemicals)));
        assert!(!config.chemicals_eject);
        assert!(config.press(inf, Button::Side { kind: Kind::Items, side: 4, forward: true }));
        assert_eq!(config.items[4], Input1, "its items go round INPUT_1, INPUT_2, ...");
    }
}
