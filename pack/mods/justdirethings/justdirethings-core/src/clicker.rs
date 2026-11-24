//! The Clicker T1 and T2 (`ClickerT1BE`, `ClickerT2BE`): on `BaseMachineBE`'s clock and the redstone control, the
//! machine's fake player clicks with the stack in its slot: a right click, a left click or a held use (`clickType`
//! 0, 1, 2), on the block in front (T1) or its area's blocks nearest first (T2), on the air there, or on the creatures
//! there of a kind (`CLICK_TARGET`), sneaking or not. The places and creatures to click are found once and clicked one
//! per run (`positionsToClick`, `entitiesToClick`); a held use runs every tick until it is let go. The T2 clicks only
//! with 250 FE in its buffer, and pays them for a click that consumed. The click itself is the game's fake player.
use crate::{machines::Clock, redstone::Mode};

/// `clickType`: 0 right click, 1 left click, 2 held use.
pub const RIGHT: i32 = 0;
pub const LEFT: i32 = 1;
pub const HOLD: i32 = 2;

/// `ClickerT2BE.getStandardEnergyCost`.
pub const T2_CLICK_COST: i32 = 250;

/// `CLICK_TARGET`, in its ordinal order (the saved `clickTarget` and the screen's button).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Target {
    #[default]
    Block,
    Air,
    Hostile,
    Passive,
    Adult,
    Child,
    Player,
    Living,
}

impl Target {
    pub const ALL: [Self; 8] = [Self::Block, Self::Air, Self::Hostile, Self::Passive, Self::Adult, Self::Child, Self::Player, Self::Living];

    pub fn index(self) -> i32 {
        self as i32
    }

    pub fn from_index(i: i32) -> Self {
        Self::ALL.get(usize::try_from(i).unwrap_or(0)).copied().unwrap_or_default()
    }

    pub fn next(self) -> Self {
        Self::from_index((self.index() + 1) % 8)
    }

    /// Blocks or the air (`positionsToClick`), not creatures (`entitiesToClick`).
    pub fn places(self) -> bool {
        matches!(self, Self::Block | Self::Air)
    }

    /// `isValidEntity` (before the T2's filter): what a living creature must be for this target.
    pub fn accepts(self, e: Creature) -> bool {
        match self {
            Self::Hostile => e.monster,
            Self::Passive => e.animal,
            Self::Adult => e.animal && !e.baby,
            Self::Child => e.animal && e.baby,
            Self::Player => e.player,
            Self::Block | Self::Air | Self::Living => true,
        }
    }

    /// The button's texture (`CLICK_TARGET_TEXTURES`) and its tooltip.
    pub fn button(self) -> (&'static str, &'static str) {
        match self {
            Self::Block => ("filter-block", "Target: Block"),
            Self::Air => ("filter-air", "Target: Air"),
            Self::Hostile => ("mobscanner", "Target: Hostile"),
            Self::Passive => ("passivemob", "Target: Passive"),
            Self::Adult => ("passivemob-adult", "Target: Adult"),
            Self::Child => ("passivemob-child", "Target: Child"),
            Self::Player => ("player", "Target: Player"),
            Self::Living => ("glowing", "Target: Living"),
        }
    }
}

/// What `isValidEntity` reads of a living creature: a `Monster`, an `Animal` (and a baby one), a `Player`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Creature {
    pub monster: bool,
    pub animal: bool,
    pub baby: bool,
    pub player: bool,
}

/// The settings the screen sends (`ClickerPayload`, `setClickerSettings`), saved under Java's keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub click_type: i32,
    pub target: Target,
    pub sneaking: bool,
    pub show_fake_player: bool,
    pub max_hold_ticks: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Self { click_type: RIGHT, target: Target::Block, sneaking: false, show_fake_player: false, max_hold_ticks: 1 }
    }
}

/// What a run clicks: one of the places, or one of the creatures, found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click<P, E> {
    Place(P),
    Creature(E),
}

/// A Clicker: its clock, settings and the places or creatures it found and has not clicked yet (Java's transient
/// lists).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clicker<P, E> {
    pub clock: Clock,
    pub settings: Settings,
    pub places: Vec<P>,
    pub creatures: Vec<E>,
}

impl<P, E> Default for Clicker<P, E> {
    fn default() -> Self {
        Self { clock: Clock::default(), settings: Settings::default(), places: Vec::new(), creatures: Vec::new() }
    }
}

impl<P, E> Clicker<P, E> {
    /// `setClickerSettings`: the new settings, and nothing queued any more.
    pub fn set(&mut self, settings: Settings) {
        self.settings = settings;
        self.places.clear();
        self.creatures.clear();
    }

    /// One server tick (`tickServer`: the clock and the redstone, then `doClick`). `can_click`: the T2 has the power
    /// for a click (the T1 always can); `holding`: the fake player still uses an item (`isUsingItem`). `places` and
    /// `creatures` find what to click (`findSpotsToClick`, `findEntitiesToClick`) when nothing is queued. What to click
    /// this tick, if anything.
    pub fn tick(
        &mut self,
        signal: bool,
        can_click: bool,
        holding: bool,
        places: impl FnOnce() -> Vec<P>,
        creatures: impl FnOnce() -> Vec<E>,
    ) -> Option<Click<P, E>> {
        self.clock.tick(signal);
        // The overridden `canRun`: a held use goes on every tick.
        let held = self.settings.click_type == HOLD && holding;
        let can_run = self.clock.can_run() || held;
        // `clearTrackerIfNeeded` (a pulse is used up by asking, as Java's `isActiveRedstone`).
        if !(self.places.is_empty() && self.creatures.is_empty())
            && (!can_click || (!self.clock.redstone.is_active() && self.clock.redstone.mode != Mode::Pulse))
        {
            self.places.clear();
            self.creatures.clear();
            return None;
        }
        if !can_click {
            return None;
        }
        if self.settings.target.places() {
            if self.clock.redstone.is_active() && can_run && self.places.is_empty() {
                self.places = places();
            }
            if self.places.is_empty() || !can_run {
                return None;
            }
            return Some(Click::Place(self.places.remove(0)));
        }
        if self.clock.redstone.is_active() && can_run && self.creatures.is_empty() {
            self.creatures = creatures();
        }
        if self.creatures.is_empty() || !(can_run || held) {
            return None;
        }
        Some(Click::Creature(self.creatures.remove(0)))
    }
}

/// `Direction.values()`' normals: down, up, north, south, west, east.
pub const NORMALS: [[i32; 3]; 6] = [[0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]];

/// `FakePlayerUtil.setupFakePlayerForUse` for a click on the block at `pos` in `direction` (an index of
/// `Direction.values()`): the fake's eye just inside that block on the side it comes from (0.05 from the face, 0.5
/// across), its look along `direction` (yaw `toYRot`, pitch 90 down, -90 up).
pub fn viewpoint(pos: [i32; 3], direction: usize) -> ([f64; 3], f32, f32) {
    let normal = NORMALS[direction.min(5)];
    let eye = std::array::from_fn(|a| {
        let along = match normal[a] {
            -1 => 0.95,
            1 => 0.05,
            _ => 0.5,
        };
        f64::from(pos[a]) + along
    });
    let yaw = match direction {
        2 => 180.0,
        3 => 0.0,
        4 => 90.0,
        5 => 270.0,
        // `Direction.toYRot` of down and up: their 2D value is -1.
        _ => 270.0,
    };
    let pitch = match direction {
        0 => 90.0,
        1 => -90.0,
        _ => 0.0,
    };
    (eye, yaw, pitch)
}

/// The look from `eye` to `target` (the entity's position), as `setupFakePlayerForUse` turns the fake to a creature.
pub fn look_at(eye: [f64; 3], target: [f64; 3]) -> (f32, f32) {
    let d = [target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if len < 1e-9 {
        return (0.0, 0.0);
    }
    let n = d.map(|v| v / len);
    let yaw = n[2].atan2(n[0]).to_degrees() as f32 - 90.0;
    let pitch = (-(n[1].atan2((n[0] * n[0] + n[2] * n[2]).sqrt()))).to_degrees() as f32;
    (yaw, pitch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::redstone::Redstone;

    fn ticks(c: &mut Clicker<i32, i32>, n: usize, signal: bool, holding: bool) -> Vec<Option<Click<i32, i32>>> {
        (0..n).map(|_| c.tick(signal, true, holding, || vec![7], || vec![1, 2])).collect()
    }

    #[test]
    fn a_right_click_on_the_block_in_front_every_run() {
        let mut c = Clicker::<i32, i32>::default();
        let runs = ticks(&mut c, 40, false, false);
        let at: Vec<usize> = runs.iter().enumerate().filter(|(_, r)| r.is_some()).map(|(i, _)| i).collect();
        assert_eq!(at, [19, 39], "every 20 ticks");
        assert_eq!(runs[19], Some(Click::Place(7)));
        // Nothing in front: no click, and the queue stays empty.
        let none = c.tick(false, true, false, Vec::new, Vec::new);
        assert_eq!(none, None);
        assert!(c.places.is_empty());
    }

    #[test]
    fn creatures_are_found_together_and_clicked_one_a_run() {
        let mut c = Clicker::<i32, i32> { settings: Settings { target: Target::Hostile, ..Settings::default() }, ..Clicker::default() };
        let runs = ticks(&mut c, 40, false, false);
        let clicked: Vec<_> = runs.into_iter().flatten().collect();
        assert_eq!(clicked, [Click::Creature(1), Click::Creature(2)], "the second waits for the next run");
        assert!(c.creatures.is_empty());
    }

    #[test]
    fn a_held_use_runs_every_tick_and_redstone_clears_the_queue() {
        let mut c = Clicker::<i32, i32> { settings: Settings { click_type: HOLD, max_hold_ticks: 5, ..Settings::default() }, ..Clicker::default() };
        assert_eq!(ticks(&mut c, 19, false, false).iter().flatten().count(), 0);
        assert_eq!(ticks(&mut c, 1, false, false), [Some(Click::Place(7))]);
        // While the fake holds the use, every tick clicks (the place is found again each time).
        assert_eq!(ticks(&mut c, 4, false, true).iter().flatten().count(), 4);
        // High mode without a signal: the queue is cleared and nothing runs.
        c.clock.redstone = Redstone { mode: Mode::High, ..Redstone::default() };
        c.places = vec![3];
        assert_eq!(c.tick(false, true, false, || vec![7], Vec::new), None);
        assert!(c.places.is_empty());
        // A T2 without the power clicks nothing and keeps nothing queued.
        let mut t2 = Clicker::<i32, i32>::default();
        t2.places = vec![1];
        assert_eq!(t2.tick(false, false, false, || vec![7], Vec::new), None);
        assert!(t2.places.is_empty());
    }

    #[test]
    fn targets_and_settings() {
        let animal = Creature { animal: true, ..Creature::default() };
        let baby = Creature { animal: true, baby: true, ..Creature::default() };
        let zombie = Creature { monster: true, ..Creature::default() };
        assert!(Target::Hostile.accepts(zombie) && !Target::Hostile.accepts(animal));
        assert!(Target::Adult.accepts(animal) && !Target::Adult.accepts(baby) && Target::Child.accepts(baby));
        assert!(Target::Living.accepts(zombie) && !Target::Player.accepts(zombie));
        assert_eq!(Target::Living.next(), Target::Block);
        assert!(Target::Air.places() && !Target::Player.places());
        let mut c = Clicker::<i32, i32> { places: vec![1], creatures: vec![2], ..Clicker::default() };
        c.set(Settings { click_type: LEFT, ..Settings::default() });
        assert!(c.places.is_empty() && c.creatures.is_empty() && c.settings.click_type == LEFT);
    }

    #[test]
    fn the_fake_stands_inside_the_block_it_clicks() {
        // Down (the default direction): near the top, looking down.
        assert_eq!(viewpoint([1, 64, 0], 0), ([1.5, 64.95, 0.5], 270.0, 90.0));
        // East (+x): near the west face, looking east.
        assert_eq!(viewpoint([1, 64, 0], 5), ([1.05, 64.5, 0.5], 270.0, 0.0));
        assert_eq!(viewpoint([1, 64, 0], 2).1, 180.0);
        let (yaw, pitch) = look_at([0.5, 65.0, 0.5], [0.5, 64.0, 1.5]);
        assert!((yaw - 0.0).abs() < 1e-4 && (pitch - 45.0).abs() < 1e-4, "{yaw} {pitch}");
    }
}
