//! Mekanism's Configurator (`ItemConfigurator` and its `ConfiguratorMode`):
//! a tool of eight modes, kept in the item. Configurating a transmission type,
//! a sneaking use bumps the side of a block's configuration of that type to
//! its next supported data type (a plain use says what it is); on a
//! transmitter it bumps the side's connection type, on a transporter its
//! colour (`onSneakRightClick`), on a Fluid Tank whether it gives down. Empty
//! takes a machine's items out, rotate turns a directional block (a plain use
//! clockwise, as the wrench of `tryWrenchRotate`; a sneaking one to face away
//! from the side used), and wrench takes a block up into the player's
//! inventory (a sneaking use, `dismantleBlock`). In Mekanism 10.7 it holds no
//! energy: 10.7.19 has no Configurator energy settings, and its uses cost
//! none.
use crate::network::Connection;
use crate::sides::Direction;

/// `TransmissionType`, by its ordinal (as `component_config` numbers its configs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transmission {
    Energy,
    Fluid,
    Chemical,
    Item,
    Heat,
}

impl Transmission {
    pub fn id(self) -> u8 {
        self as u8
    }

    /// `transmission.mekanism.*`.
    pub fn name(self) -> &'static str {
        ["Energy", "Fluids", "Chemicals", "Items", "Heat"][self as usize]
    }
}

/// `ConfiguratorMode`, kept by its ordinal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    ConfigurateItems,
    ConfigurateFluids,
    ConfigurateChemicals,
    ConfigurateEnergy,
    ConfigurateHeat,
    Empty,
    Rotate,
    Wrench,
}

impl Mode {
    pub const ALL: [Self; 8] = [
        Self::ConfigurateItems,
        Self::ConfigurateFluids,
        Self::ConfigurateChemicals,
        Self::ConfigurateEnergy,
        Self::ConfigurateHeat,
        Self::Empty,
        Self::Rotate,
        Self::Wrench,
    ];

    pub fn id(self) -> i32 {
        self as i32
    }

    /// `BY_ID`, which wraps.
    pub fn by_id(id: i32) -> Self {
        Self::ALL[id.rem_euclid(Self::ALL.len() as i32) as usize]
    }

    /// `IIncrementalEnum.adjust`: `shift` modes on, around.
    pub fn adjust(self, shift: i32) -> Self {
        Self::by_id(self.id() + shift)
    }

    /// The transmission type it configurates.
    pub fn transmission(self) -> Option<Transmission> {
        match self {
            Self::ConfigurateItems => Some(Transmission::Item),
            Self::ConfigurateFluids => Some(Transmission::Fluid),
            Self::ConfigurateChemicals => Some(Transmission::Chemical),
            Self::ConfigurateEnergy => Some(Transmission::Energy),
            Self::ConfigurateHeat => Some(Transmission::Heat),
            _ => None,
        }
    }

    /// `getTextComponent`, in English.
    pub fn name(self) -> String {
        match self.transmission() {
            Some(t) => format!("Configurate ({})", t.name()),
            None => ["Empty", "Rotate", "Wrench"][self as usize - 5].to_owned(),
        }
    }

    /// The model it shows (the carried model's `mekanism:mode` predicate):
    /// 0 configurating, 1 empty, 2 rotate, 3 wrench.
    pub fn model(self) -> i32 {
        (self as i32 - 4).max(0)
    }

    /// The name `/mek configurator` takes.
    pub fn from_name(name: &str) -> Option<Self> {
        let all = ["items", "fluids", "chemicals", "energy", "heat", "empty", "rotate", "wrench"];
        all.iter().position(|n| *n == name).map(|i| Self::ALL[i])
    }
}

/// `ConnectionType.getNext`: normal, push, pull, none, around.
pub fn next_connection(connection: Connection) -> Connection {
    Connection::by_id(connection.id() + 1)
}

/// `MekanismUtils.rotate`: a block that faces any way turns up, north, east,
/// down, south, west; one that faces only horizontally turns clockwise.
pub fn rotate(facing: Direction, support_y: bool) -> Direction {
    if support_y {
        match facing {
            Direction::Up => Direction::North,
            Direction::Down => Direction::South,
            Direction::North => Direction::East,
            Direction::South => Direction::West,
            Direction::West => Direction::Up,
            Direction::East => Direction::Down,
        }
    } else {
        facing.clockwise()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_go_around_and_name_their_transmission() {
        assert_eq!(Mode::Wrench.adjust(1), Mode::ConfigurateItems);
        assert_eq!(Mode::ConfigurateItems.adjust(-1), Mode::Wrench);
        assert_eq!(Mode::by_id(5), Mode::Empty);
        assert_eq!(Mode::ConfigurateChemicals.name(), "Configurate (Chemicals)");
        assert_eq!(Mode::Rotate.name(), "Rotate");
        assert_eq!(Mode::ALL.map(Mode::model), [0, 0, 0, 0, 0, 1, 2, 3]);
        assert_eq!(Mode::ConfigurateEnergy.transmission().map(Transmission::id), Some(0));
        assert_eq!(Mode::ConfigurateItems.transmission().map(Transmission::id), Some(3));
        assert_eq!(Mode::from_name("wrench"), Some(Mode::Wrench));
    }

    #[test]
    fn connections_and_facings_turn_as_mekanism_turns_them() {
        assert_eq!(next_connection(Connection::Normal), Connection::Push);
        assert_eq!(next_connection(Connection::Pull), Connection::None);
        assert_eq!(next_connection(Connection::None), Connection::Normal);
        assert_eq!(rotate(Direction::North, false), Direction::East);
        assert_eq!(rotate(Direction::West, false), Direction::North);
        assert_eq!(rotate(Direction::East, true), Direction::Down);
    }
}
