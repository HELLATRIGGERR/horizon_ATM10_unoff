//! Mekanism Generators' blocks this port has (`GeneratorsBlocks`), and what
//! they share (`TileEntityGenerator`): a store that only they fill, emptied
//! out of their energy face each tick at up to their max output.
use crate::config::GeneratorsConfig;
use crate::sides::{Direction, RelativeSide};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GeneratorKind {
    /// `mekanismgenerators:solar_generator`.
    Solar,
    /// `mekanismgenerators:heat_generator`.
    Heat,
}

impl GeneratorKind {
    pub const ALL: [Self; 2] = [Self::Solar, Self::Heat];

    pub fn id(self) -> &'static str {
        match self {
            Self::Solar => "mekanismgenerators:solar_generator",
            Self::Heat => "mekanismgenerators:heat_generator",
        }
    }

    pub fn of_block(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.id() == id)
    }

    /// `getEnergySides`: the solar generator's bottom, the heat generator's
    /// front (`TileEntityGenerator`'s default).
    pub fn energy_side(self) -> RelativeSide {
        match self {
            Self::Solar => RelativeSide::Bottom,
            Self::Heat => RelativeSide::Front,
        }
    }

    /// The absolute face its energy is on, facing `facing`.
    pub fn energy_face(self, facing: Direction) -> Direction {
        self.energy_side().direction(facing)
    }

    /// Joules it holds (`generator-storage.toml`).
    pub fn capacity(self, config: &GeneratorsConfig) -> u64 {
        match self {
            Self::Solar => config.solar_storage,
            Self::Heat => config.heat_storage,
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
    }
}
