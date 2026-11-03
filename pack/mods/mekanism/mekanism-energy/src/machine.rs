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
//! (Mekanism 10.7's `MekanismChemicals`), filled from items by
//! `mekanism:chemical_conversion` recipes.
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
}

/// Where a machine keeps its items: Mekanism's own slot order (input,
/// output, energy; the infuser's infusion slot first), then the upgrade
/// component's input and output slots, which Mekanism keeps apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slots {
    pub infusion: Option<usize>,
    pub input: usize,
    pub output: usize,
    pub energy: usize,
    pub upgrade: usize,
    pub upgrade_out: usize,
    pub count: usize,
}

impl MachineKind {
    pub const ALL: [Self; 4] = [Self::EnrichmentChamber, Self::Crusher, Self::EnergizedSmelter, Self::MetallurgicInfuser];

    pub fn id(self) -> &'static str {
        match self {
            Self::EnrichmentChamber => "mekanism:enrichment_chamber",
            Self::Crusher => "mekanism:crusher",
            Self::EnergizedSmelter => "mekanism:energized_smelter",
            Self::MetallurgicInfuser => "mekanism:metallurgic_infuser",
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
        }
    }

    /// The recipe type it reads.
    pub fn recipe_type(self) -> &'static str {
        match self {
            Self::EnrichmentChamber => "mekanism:enriching",
            Self::Crusher => "mekanism:crushing",
            Self::EnergizedSmelter => "minecraft:smelting",
            Self::MetallurgicInfuser => "mekanism:metallurgic_infusing",
        }
    }

    /// `UsageConfig`'s default Joules a tick.
    pub fn default_usage(self) -> u64 {
        50
    }

    /// `StorageConfig`'s default Joules held.
    pub fn default_storage(self) -> u64 {
        20_000
    }

    pub fn infuses(self) -> bool {
        self == Self::MetallurgicInfuser
    }

    /// Whether its block shows `active` (`AttributeStateActive`): the infuser's
    /// model has no such state.
    pub fn shows_active(self) -> bool {
        !self.infuses()
    }

    pub fn slots(self) -> Slots {
        if self.infuses() {
            Slots { infusion: Some(0), input: 1, output: 2, energy: 3, upgrade: 4, upgrade_out: 5, count: 6 }
        } else {
            Slots { infusion: None, input: 0, output: 1, energy: 2, upgrade: 3, upgrade_out: 4, count: 5 }
        }
    }

    /// The item side types it has slots for (`ConfigInfo.supports`):
    /// `setupItemIOConfig`, and `setupItemIOExtraConfig` with the infuser's
    /// infusion slot as `EXTRA`.
    pub fn item_supports(self, data: DataType) -> bool {
        match data {
            DataType::None | DataType::Input | DataType::Output | DataType::InputOutput | DataType::Energy => true,
            DataType::Extra => self.infuses(),
            _ => false,
        }
    }

    /// The item config its item comes with (`AttachedSideConfig`): `MACHINE`
    /// (right output, back energy, the rest input) and the infuser's
    /// `EXTRA_MACHINE` (its bottom the infusion slot).
    pub fn default_items(self) -> [DataType; 6] {
        RelativeSide::ALL.map(|side| match side {
            RelativeSide::Right => DataType::Output,
            RelativeSide::Back => DataType::Energy,
            RelativeSide::Bottom if self.infuses() => DataType::Extra,
            _ => DataType::Input,
        })
    }

    /// The infuser's chemical config (`OUT_NO_EJECT`: right output, the rest input).
    pub fn default_chemicals(self) -> Option<[DataType; 6]> {
        self.infuses().then(|| RelativeSide::ALL.map(|s| if s == RelativeSide::Right { DataType::Output } else { DataType::Input }))
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

/// The infuse types (`MekanismChemicals`), with their tint (`registerInfuse`).
pub const INFUSE_TYPES: [(&str, u32); 8] = [
    ("mekanism:carbon", 0x2C2C2C),
    ("mekanism:redstone", 0xB30505),
    ("mekanism:diamond", 0x6CEDD8),
    ("mekanism:refined_obsidian", 0x7C00ED),
    ("mekanism:bio", 0x5A4630),
    ("mekanism:fungi", 0x74656A),
    ("mekanism:gold", 0xF2CD67),
    ("mekanism:tin", 0xCCCCD9),
];

/// An infuse type's number on the menu's data slots: 1 and up as
/// [`INFUSE_TYPES`], 0 for none, 255 for another chemical.
pub fn infuse_number(chemical: Option<&str>) -> i32 {
    match chemical {
        None => 0,
        Some(id) => INFUSE_TYPES.iter().position(|(c, _)| *c == id).map_or(255, |i| i as i32 + 1),
    }
}

pub fn infuse_of_number(n: i32) -> Option<(&'static str, u32)> {
    usize::try_from(n - 1).ok().and_then(|i| INFUSE_TYPES.get(i)).copied()
}

/// The infuser's tank (`BasicChemicalTank`): one chemical at a time.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tank {
    pub chemical: Option<String>,
    pub amount: u64,
}

impl Tank {
    /// A conversion's output, `amount` of `chemical`, goes in whole or not at
    /// all (`ChemicalInventorySlot.fillTankOrConvert`: `insert` must leave
    /// nothing): the same chemical or an empty tank, and room for all of it.
    pub fn fill(&mut self, chemical: &str, amount: u64, capacity: u64) -> bool {
        if amount == 0 || self.chemical.as_deref().is_some_and(|c| c != chemical) && self.amount > 0 {
            return false;
        }
        if capacity.saturating_sub(self.amount) < amount {
            return false;
        }
        self.chemical = Some(chemical.to_owned());
        self.amount += amount;
        true
    }

    /// `shrinkStack`: what an operation used; an empty tank holds nothing.
    pub fn drain(&mut self, amount: u64) {
        self.amount = self.amount.saturating_sub(amount);
        if self.amount == 0 {
            self.chemical = None;
        }
    }
}

/// A machine's side configuration (`TileComponentConfig`, the ejector's
/// switch): energy, items and the infuser's chemicals by `RelativeSide`.
/// Only items can eject: energy and chemical sides have `setCanEject(false)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SideConfig {
    pub energy: [DataType; 6],
    pub items: [DataType; 6],
    pub chemicals: Option<[DataType; 6]>,
    pub items_eject: bool,
}

impl SideConfig {
    /// The machine's item's config (`AttachedSideConfig`, `AttachedEjector.DEFAULT`):
    /// energy in on every side, items as `default_items`, not ejecting.
    pub fn new(kind: MachineKind) -> Self {
        Self {
            energy: [DataType::Input; 6],
            items: kind.default_items(),
            chemicals: kind.default_chemicals(),
            items_eject: false,
        }
    }

    /// A side configuration button (`PacketSideData`, `PacketEjectConfiguration`,
    /// `PacketBatchConfiguration`, as `CubeState::press`): whether anything changed.
    pub fn press(&mut self, kind: MachineKind, button: crate::menu::Button) -> bool {
        use crate::menu::{Button, Kind};
        let before = *self;
        let item_supports = |d: DataType| kind.item_supports(d);
        let (sides, supports): (&mut [DataType; 6], &dyn Fn(DataType) -> bool) = match button {
            Button::Page(_) => return false,
            Button::Side { kind: k, .. } | Button::Eject(k) | Button::Clear { kind: k, .. } => match k {
                Kind::Energy => (&mut self.energy, &energy_supports),
                Kind::Items => (&mut self.items, &item_supports),
                Kind::Chemicals => match self.chemicals.as_mut() {
                    Some(c) => (c, &chemical_supports),
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
    fn the_infusion_tank_holds_one_type_and_fills_whole() {
        let mut tank = Tank::default();
        assert!(tank.fill("mekanism:carbon", 20, INFUSE_TANK));
        assert!(!tank.fill("mekanism:fungi", 10, INFUSE_TANK), "another type waits");
        tank.amount = 990;
        assert!(!tank.fill("mekanism:carbon", 20, INFUSE_TANK), "no partial fill");
        tank.amount = 980;
        assert!(tank.fill("mekanism:carbon", 20, INFUSE_TANK));
        tank.drain(1000);
        assert_eq!(tank, Tank::default());
        assert_eq!(infuse_number(Some("mekanism:fungi")), 6);
        assert_eq!(infuse_of_number(6).map(|t| t.0), Some("mekanism:fungi"));
        assert_eq!(infuse_number(Some("mekanism:oxygen")), 255);
    }
}
