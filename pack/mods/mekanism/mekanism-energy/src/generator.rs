//! Mekanism Generators' blocks this port has (`GeneratorsBlocks`): the Solar,
//! Heat, Gas-Burning, Bio, Wind and Advanced Solar Generators, and what
//! they share (`TileEntityGenerator`): a store that only they fill, emptied
//! out of their energy faces each tick at up to their max output, split
//! among the faces that take it.
use crate::config::GeneratorsConfig;
use crate::sides::{Direction, RelativeSide};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GeneratorKind {
    /// `mekanismgenerators:solar_generator`.
    Solar,
    /// `mekanismgenerators:heat_generator`.
    Heat,
    /// `mekanismgenerators:gas_burning_generator` (`crate::gas`).
    GasBurning,
    /// `mekanismgenerators:bio_generator` (`crate::bio`).
    Bio,
    /// `mekanismgenerators:wind_generator` (`crate::wind`), four bounding
    /// blocks above it.
    Wind,
    /// `mekanismgenerators:advanced_solar_generator` (`crate::solar`'s
    /// `AdvancedSolar`), a bounding block above it and nine over that.
    AdvancedSolar,
}

/// `TileEntityGenerator`'s energy side, the front.
const FRONT: [RelativeSide; 1] = [RelativeSide::Front];
/// `TileEntitySolarGenerator`'s, the bottom.
const BOTTOM: [RelativeSide; 1] = [RelativeSide::Bottom];
/// The wind and advanced solar generators' `ENERGY_SIDES`.
const FRONT_AND_BOTTOM: [RelativeSide; 2] = [RelativeSide::Front, RelativeSide::Bottom];

impl GeneratorKind {
    pub const ALL: [Self; 6] = [Self::Solar, Self::Heat, Self::GasBurning, Self::Bio, Self::Wind, Self::AdvancedSolar];

    pub fn id(self) -> &'static str {
        match self {
            Self::Solar => "mekanismgenerators:solar_generator",
            Self::Heat => "mekanismgenerators:heat_generator",
            Self::GasBurning => "mekanismgenerators:gas_burning_generator",
            Self::Bio => "mekanismgenerators:bio_generator",
            Self::Wind => "mekanismgenerators:wind_generator",
            Self::AdvancedSolar => "mekanismgenerators:advanced_solar_generator",
        }
    }

    pub fn of_block(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.id() == id)
    }

    /// `getEnergySides`: the solar generator's bottom, the heat, gas-burning
    /// and bio generators' front (`TileEntityGenerator`'s default), the wind
    /// and advanced solar generators' front and bottom, in the order the
    /// ejector looks at them.
    pub fn energy_sides(self) -> &'static [RelativeSide] {
        match self {
            Self::Solar => &BOTTOM,
            Self::Heat | Self::GasBurning | Self::Bio => &FRONT,
            Self::Wind | Self::AdvancedSolar => &FRONT_AND_BOTTOM,
        }
    }

    /// The first of its energy sides.
    pub fn energy_side(self) -> RelativeSide {
        self.energy_sides()[0]
    }

    /// The absolute face its (first) energy side is on, facing `facing`.
    pub fn energy_face(self, facing: Direction) -> Direction {
        self.energy_side().direction(facing)
    }

    /// The absolute faces its energy is on, facing `facing`.
    pub fn energy_faces(self, facing: Direction) -> Vec<Direction> {
        self.energy_sides().iter().map(|s| s.direction(facing)).collect()
    }

    /// Whether its energy is on the absolute face `face`, facing `facing`.
    pub fn exposes(self, facing: Direction, face: Direction) -> bool {
        self.energy_sides().iter().any(|s| s.direction(facing) == face)
    }

    /// The positions of its bounding blocks (`AttributeHasBounding`), relative
    /// to it: the wind generator's four above it, the advanced solar
    /// generator's one above it and the three by three over that.
    pub fn bounding(self) -> Vec<[i32; 3]> {
        match self {
            Self::Wind => (1..=4).map(|y| [0, y, 0]).collect(),
            Self::AdvancedSolar => {
                let mut at = vec![[0, 1, 0]];
                for x in -1..=1 {
                    for z in -1..=1 {
                        at.push([x, 2, z]);
                    }
                }
                at
            }
            _ => Vec::new(),
        }
    }

    /// Joules it holds (`generator-storage.toml`); the gas-burning
    /// generator's are 1000 times hydrogen's energy density, a chemical fact
    /// (`crate::gas::storage`), 200 000 J with hydrogen's 200 J/mB.
    pub fn capacity(self, config: &GeneratorsConfig) -> u64 {
        match self {
            Self::Solar => config.solar_storage,
            Self::Heat => config.heat_storage,
            Self::GasBurning => crate::gas::storage(200),
            Self::Bio => config.bio_storage,
            Self::Wind => config.wind_storage,
            Self::AdvancedSolar => config.advanced_solar_storage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generators_put_out_through_their_energy_face() {
        assert_eq!(GeneratorKind::of_block("mekanismgenerators:heat_generator"), Some(GeneratorKind::Heat));
        assert_eq!(GeneratorKind::of_block("mekanism:heat_generator"), None);
        for facing in [Direction::North, Direction::East, Direction::South, Direction::West] {
            assert_eq!(GeneratorKind::Solar.energy_face(facing), Direction::Down);
            assert_eq!(GeneratorKind::Heat.energy_face(facing), facing);
        }
        let atm10 = GeneratorsConfig { solar_storage: 96_000, heat_storage: 160_000, ..GeneratorsConfig::default() };
        assert_eq!(GeneratorKind::Solar.capacity(&atm10), 96_000);
        assert_eq!(GeneratorKind::Heat.capacity(&atm10), 160_000);
        assert_eq!(GeneratorKind::of_block("mekanismgenerators:gas_burning_generator"), Some(GeneratorKind::GasBurning));
        assert_eq!(GeneratorKind::GasBurning.energy_face(Direction::East), Direction::East);
        assert_eq!(GeneratorKind::GasBurning.capacity(&atm10), 200_000);
    }

    #[test]
    fn wind_and_advanced_solar_put_out_through_front_and_bottom() {
        // Measured on the oracle (g2/w1b): 1800 J a tick of the wind generator, 900 into a cube in
        // front and 900 into one below; none into one on a side.
        assert_eq!(GeneratorKind::Wind.energy_faces(Direction::North), [Direction::North, Direction::Down]);
        assert!(GeneratorKind::AdvancedSolar.exposes(Direction::East, Direction::Down));
        assert!(GeneratorKind::AdvancedSolar.exposes(Direction::East, Direction::East));
        assert!(!GeneratorKind::AdvancedSolar.exposes(Direction::East, Direction::North));
        assert_eq!(GeneratorKind::Bio.energy_faces(Direction::West), [Direction::West]);
        assert_eq!(GeneratorKind::Solar.energy_faces(Direction::West), [Direction::Down]);
        assert_eq!(GeneratorKind::Wind.bounding(), [[0, 1, 0], [0, 2, 0], [0, 3, 0], [0, 4, 0]]);
        let solar = GeneratorKind::AdvancedSolar.bounding();
        assert_eq!(solar.len(), 10);
        assert_eq!(solar[0], [0, 1, 0]);
        assert!(solar[1..].iter().all(|p| p[1] == 2));
        assert!(GeneratorKind::Bio.bounding().is_empty());
        let atm10 = GeneratorsConfig { bio_storage: 160_000, advanced_solar_storage: 2_500_000, wind_storage: 200_000, ..GeneratorsConfig::default() };
        assert_eq!(
            [GeneratorKind::Bio, GeneratorKind::Wind, GeneratorKind::AdvancedSolar].map(|k| k.capacity(&atm10)),
            [160_000, 200_000, 2_500_000]
        );
    }
}
