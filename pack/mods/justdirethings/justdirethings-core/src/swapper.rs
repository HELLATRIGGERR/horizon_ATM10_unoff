//! The Block Swapper T1 (`BlockSwapperT1BE`): two bound swappers trade what stands in front of them, a block each (and
//! optionally the entities of that block) on the operation clock, in pulse mode by default. Positions map by their offset
//! from each swapper's starting point.
use crate::{
    machines::Clock,
    redstone::{Mode, Redstone},
};

/// `SWAP_ENTITY_TYPE`, in its ordinal order (the saved `swap_entity_type` and the screen's button).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SwapEntity {
    #[default]
    None,
    Hostile,
    Passive,
    Adult,
    Child,
    Player,
    Living,
    Item,
    All,
}

impl SwapEntity {
    pub const ALL: [Self; 9] =
        [Self::None, Self::Hostile, Self::Passive, Self::Adult, Self::Child, Self::Player, Self::Living, Self::Item, Self::All];

    pub fn index(self) -> i32 {
        self as i32
    }

    /// `SWAP_ENTITY_TYPE.values()[i]`; out of range reads as none.
    pub fn from_index(i: i32) -> Self {
        Self::ALL.get(usize::try_from(i).unwrap_or(0)).copied().unwrap_or_default()
    }

    pub fn next(self) -> Self {
        Self::from_index((self.index() + 1) % 9)
    }
}

/// What `isValidEntity` asks of an entity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Kind {
    pub monster: bool,
    pub animal: bool,
    pub baby: bool,
    pub player: bool,
    pub item: bool,
    /// A multipart entity or one of its parts, or one a teleport can't carry (`TELEPORTING_NOT_SUPPORTED`).
    pub unmovable: bool,
}

impl SwapEntity {
    /// `isValidEntity` for this setting.
    pub fn accepts(self, e: Kind) -> bool {
        if e.unmovable {
            return false;
        }
        match self {
            Self::None | Self::Living | Self::All => true,
            Self::Hostile => e.monster,
            Self::Passive => e.animal,
            Self::Adult => e.animal && !e.baby,
            Self::Child => e.animal && e.baby,
            Self::Player => e.player,
            Self::Item => e.item,
        }
    }
}

/// A bound partner: its dimension and block position (`GlobalPos`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bound {
    pub dimension: String,
    pub pos: [i32; 3],
}

/// The swapper's settings; redstone defaults to pulse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Swapper {
    pub clock: Clock,
    pub bound: Option<Bound>,
    pub swap_blocks: bool,
    pub entities: SwapEntity,
}

impl Default for Swapper {
    fn default() -> Self {
        Self {
            clock: Clock { redstone: Redstone { mode: Mode::Pulse, ..Redstone::default() }, ..Clock::default() },
            bound: None,
            swap_blocks: true,
            entities: SwapEntity::None,
        }
    }
}

impl Swapper {
    /// One server tick (`tickServer` then the start of `doSwap`): whether it swaps this tick. The partner must exist.
    pub fn tick(&mut self, signal: bool, partner_exists: bool) -> bool {
        self.clock.tick(signal);
        self.clock.redstone.is_active() && self.clock.can_run() && partner_exists
    }
}

/// `getWorldPos(getRelativePos(pos))`: where `pos` of the swapper at `from` lands at the swapper at `to`.
pub fn translate(pos: [i32; 3], from: [i32; 3], to: [i32; 3]) -> [i32; 3] {
    [pos[0] - from[0] + to[0], pos[1] - from[1] + to[1], pos[2] - from[2] + to[2]]
}

/// The same for a point.
pub fn translate_f(pos: [f64; 3], from: [i32; 3], to: [i32; 3]) -> [f64; 3] {
    [pos[0] - f64::from(from[0]) + f64::from(to[0]), pos[1] - f64::from(from[1]) + f64::from(to[1]), pos[2] - f64::from(from[2]) + f64::from(to[2])]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_swapper_waits_for_a_pulse_and_a_partner() {
        let mut s = Swapper::default();
        assert!((1..=60).all(|_| !s.tick(false, true)), "pulse mode: nothing without a signal");
        assert!(!s.tick(true, false), "a pulse with no partner does nothing (and is used up)");
        assert!(!s.tick(true, true), "a held signal is no new pulse");
        s.tick(false, true);
        assert!(s.tick(true, true));
    }

    #[test]
    fn on_ignore_it_runs_on_the_clock() {
        let mut s = Swapper::default();
        s.clock.redstone.mode = Mode::Ignored;
        let runs: Vec<u32> = (1..=45).filter(|_| s.tick(false, true)).collect();
        assert_eq!(runs, vec![20, 40]);
    }

    #[test]
    fn entity_settings_filter_as_the_machine_does() {
        let cow = Kind { animal: true, ..Kind::default() };
        let calf = Kind { animal: true, baby: true, ..Kind::default() };
        let zombie = Kind { monster: true, ..Kind::default() };
        let player = Kind { player: true, ..Kind::default() };
        let item = Kind { item: true, ..Kind::default() };
        let dragon = Kind { unmovable: true, monster: true, ..Kind::default() };
        assert!(SwapEntity::Passive.accepts(cow) && SwapEntity::Passive.accepts(calf) && !SwapEntity::Passive.accepts(zombie));
        assert!(SwapEntity::Adult.accepts(cow) && !SwapEntity::Adult.accepts(calf));
        assert!(SwapEntity::Child.accepts(calf) && !SwapEntity::Child.accepts(cow));
        assert!(SwapEntity::Hostile.accepts(zombie) && !SwapEntity::Hostile.accepts(cow));
        assert!(SwapEntity::Player.accepts(player) && !SwapEntity::Player.accepts(cow));
        assert!(SwapEntity::Item.accepts(item) && !SwapEntity::Item.accepts(zombie));
        assert!(SwapEntity::All.accepts(item) && SwapEntity::Living.accepts(cow));
        assert!(!SwapEntity::All.accepts(dragon) && !SwapEntity::Hostile.accepts(dragon));
        assert_eq!(SwapEntity::All.next(), SwapEntity::None);
        assert_eq!(SwapEntity::from_index(99), SwapEntity::None);
    }

    #[test]
    fn positions_keep_their_offset() {
        assert_eq!(translate([5, 64, 5], [5, 64, 5], [100, 70, -3]), [100, 70, -3]);
        assert_eq!(translate([6, 65, 4], [5, 64, 5], [100, 70, -3]), [101, 71, -4]);
        assert_eq!(translate_f([5.5, 64.0, 5.5], [5, 64, 5], [0, 0, 0]), [0.5, 0.0, 0.5]);
    }
}
