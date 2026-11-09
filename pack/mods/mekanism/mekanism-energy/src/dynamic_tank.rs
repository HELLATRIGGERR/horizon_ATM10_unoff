//! Mekanism's Dynamic Tank (`TankMultiblockData`, `TankValidator`, `TankCache`): a cuboid of dynamic tank casings
//! with dynamic valves and structural glass in its sides and air inside, holding one fluid or one chemical
//! (`MergedTank`: a fluid goes in only while no chemical is there, and the other way round) of `volume *
//! fluidPerTank` mB or `volume * chemicalPerTank` mB (`general.toml` `[dynamic_tank]`), the volume being the whole
//! cuboid's. Its two slots fill and empty containers as its edit mode says (`ContainerEditMode`), kept with its
//! contents in the cache (`edit_mode`). Every valve is its fluid and chemical handler on every side, without a rate;
//! the casings have no handler, and nothing is put out on its own.
use std::path::Path;

/// `general.toml` `[dynamic_tank]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TankConfig {
    /// `fluidPerTank`, 1..=`Integer.MAX_VALUE / 5832`.
    pub fluid_per_tank: i32,
    /// `chemicalPerTank`, 1..=`Long.MAX_VALUE / 5832`.
    pub chemical_per_tank: i64,
}

impl Default for TankConfig {
    fn default() -> Self {
        Self { fluid_per_tank: 350_000, chemical_per_tank: 16_000_000 }
    }
}

const MAX_VOLUME: i64 = 5832;

impl TankConfig {
    pub fn from_general(general: Option<&toml::Table>) -> Self {
        let mut config = Self::default();
        let Some(section) = general.and_then(|g| g.get("dynamic_tank")).and_then(toml::Value::as_table) else {
            return config;
        };
        if let Some(v) = section.get("fluidPerTank").and_then(toml::Value::as_integer)
            && (1..=i64::from(i32::MAX) / MAX_VOLUME).contains(&v)
        {
            config.fluid_per_tank = v as i32;
        }
        if let Some(v) = section.get("chemicalPerTank").and_then(toml::Value::as_integer)
            && (1..=i64::MAX / MAX_VOLUME).contains(&v)
        {
            config.chemical_per_tank = v;
        }
        config
    }

    /// `<config>/Mekanism/general.toml`.
    pub fn load(config: &Path) -> Self {
        let general = std::fs::read_to_string(config.join("Mekanism").join("general.toml")).ok().and_then(|t| t.parse::<toml::Table>().ok());
        Self::from_general(general.as_ref())
    }

    /// `setVolume`: the fluid tank's mB.
    pub fn fluid_capacity(&self, volume: i32) -> i32 {
        volume.max(0).saturating_mul(self.fluid_per_tank)
    }

    /// The chemical tank's mB.
    pub fn chemical_capacity(&self, volume: i32) -> i64 {
        i64::from(volume.max(0)).saturating_mul(self.chemical_per_tank)
    }
}

/// `IFluidContainerManager.ContainerEditMode`, by ordinal: both ways, only filling containers, only emptying them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EditMode {
    #[default]
    Both,
    Fill,
    Empty,
}

impl EditMode {
    pub const ALL: [Self; 3] = [Self::Both, Self::Fill, Self::Empty];

    pub fn id(self) -> i32 {
        self as i32
    }

    /// `BY_ID` wraps.
    pub fn by_id(id: i32) -> Self {
        Self::ALL[id.rem_euclid(3) as usize]
    }

    pub fn next(self) -> Self {
        Self::by_id(self.id() + 1)
    }

    pub fn previous(self) -> Self {
        Self::by_id(self.id() - 1)
    }

    /// `MekanismLang.FLUID_CONTAINER_*`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Both => "Both",
            Self::Fill => "Fill",
            Self::Empty => "Empty",
        }
    }
}

/// `MergedTank.CurrentType`: what the tank holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Current {
    Empty,
    Fluid,
    Chemical,
}

impl Current {
    pub fn of(fluid: i64, chemical: i64) -> Self {
        if fluid > 0 {
            Self::Fluid
        } else if chemical > 0 {
            Self::Chemical
        } else {
            Self::Empty
        }
    }
}

/// The fluid side of the merged tank (`FluidTankWrapper` over a `VariableCapacityFluidTank` with every move allowed
/// while formed): what of `amount` goes in, when it is the fluid held (`same`, or the tank is empty) and no chemical
/// is there.
pub fn fluid_room(stored: i32, capacity: i32, same: bool, chemical_held: bool, amount: i32) -> i32 {
    if amount <= 0 || !same || chemical_held {
        return 0;
    }
    amount.min(capacity.saturating_sub(stored).max(0))
}

/// `MekanismUtils.redstoneLevelFromContents`: a comparator's signal from how full it is.
pub fn redstone_level(stored: i64, capacity: i64) -> i32 {
    if stored <= 0 || capacity <= 0 {
        return 0;
    }
    1 + ((stored as f64 / capacity as f64) * 14.0) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ATM10 8.4's general.toml (Mekanism's defaults): 350 000 mB and 16 000 000 mB a block of volume.
    #[test]
    fn capacity_is_the_whole_cuboid_times_the_setting() {
        let atm10: toml::Table = "[dynamic_tank]\nfluidPerTank = 350000\nchemicalPerTank = 16000000\n".parse().unwrap();
        let c = TankConfig::from_general(Some(&atm10));
        assert_eq!(c, TankConfig::default());
        assert_eq!(c.fluid_capacity(27), 9_450_000, "a 3x3x3");
        assert_eq!(c.chemical_capacity(27), 432_000_000);
        assert_eq!(c.fluid_capacity(18 * 18 * 18), 2_041_200_000, "an 18x18x18 still fits an int");
        assert_eq!(c.chemical_capacity(5832), 93_312_000_000);
        let odd: toml::Table = "[dynamic_tank]\nfluidPerTank = 400000\nchemicalPerTank = 0\n".parse().unwrap();
        assert_eq!(TankConfig::from_general(Some(&odd)), TankConfig::default(), "out of range keeps the default");
        let small: toml::Table = "[dynamic_tank]\nfluidPerTank = 1000\n".parse().unwrap();
        assert_eq!(TankConfig::from_general(Some(&small)).fluid_capacity(36), 36_000);
    }

    #[test]
    fn a_fluid_goes_in_only_without_a_chemical() {
        assert_eq!(fluid_room(0, 9_450_000, true, false, 1000), 1000);
        assert_eq!(fluid_room(9_449_500, 9_450_000, true, false, 1000), 500);
        assert_eq!(fluid_room(0, 9_450_000, true, true, 1000), 0, "a chemical is held");
        assert_eq!(fluid_room(10, 9_450_000, false, false, 1000), 0, "another fluid");
    }

    #[test]
    fn edit_modes_go_round() {
        assert_eq!(EditMode::Both.next(), EditMode::Fill);
        assert_eq!(EditMode::Empty.next(), EditMode::Both);
        assert_eq!(EditMode::Both.previous(), EditMode::Empty);
        assert_eq!(EditMode::by_id(7), EditMode::Fill);
        assert_eq!(Current::of(0, 5), Current::Chemical);
        assert_eq!(Current::of(5, 0), Current::Fluid);
        assert_eq!(redstone_level(9_450_000, 9_450_000), 15);
        assert_eq!(redstone_level(1, 9_450_000), 1);
    }
}
