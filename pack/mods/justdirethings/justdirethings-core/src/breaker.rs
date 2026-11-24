//! The Block Breaker T1 (`BlockBreakerT1BE`, with `BaseMachineBE`'s operation clock): with a tool in its slot and its
//! redstone letting it, it starts on the block in front of it every `tickSpeed` ticks (20), mines it at the tool's
//! speed as Java's fake player would (the tool's destroy speed with Efficiency, over the block's hardness, over 30 for
//! a tool correct for the block's drops and 100 for any other) and breaks it, with drops, when the progress reaches 1.
use crate::redstone::{Mode, Redstone};

/// `getDestroyProgress`: the share of the block one tick mines; -1 for an unbreakable block (hardness -1).
pub fn destroy_progress(tool_speed: f32, efficiency: u32, hardness: f32, correct: bool) -> f32 {
    if hardness == -1.0 {
        return -1.0;
    }
    let mut speed = tool_speed;
    if speed > 1.0 && efficiency > 0 {
        speed += (efficiency * efficiency + 1) as f32;
    }
    let modifier = if correct { 30.0 } else { 100.0 };
    speed / hardness / modifier
}

/// The block being mined (one entry of Java's `blockBreakingTracker`; the T1 mines only the block in front).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tracked {
    /// The block state it started on.
    pub state: u32,
    /// Ticks mined so far.
    pub ticks: i32,
    /// The crack stage last sent (-1: none yet).
    pub last_sent: i32,
    /// The share one tick mines.
    pub progress: f32,
}

/// The machine's state. `redstone`, `tick_speed` and `sneaking` are saved; the rest is Java's transient fields.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Breaker {
    pub redstone: Redstone,
    pub tick_speed: i32,
    pub sneaking: bool,
    /// `operationTicks`: ticks to the next operation.
    pub operation_ticks: i32,
    pub tracked: Option<Tracked>,
    /// `currentBlock`: the tracked block is being mined.
    pub current: bool,
}

impl Default for Breaker {
    fn default() -> Self {
        Self { redstone: Redstone::default(), tick_speed: 20, sneaking: false, operation_ticks: -1, tracked: None, current: false }
    }
}

/// The block in front, as the tick sees it.
pub trait Front {
    /// The block state there now; none for air.
    fn state(&mut self) -> Option<u32>;
    /// `isBlockValid` (not air, not a liquid, breakable).
    fn valid(&mut self) -> bool;
    /// `getDestroyProgress` of the tool on it.
    fn progress(&mut self) -> f32;
    /// `sendPackets`: the crack stage the players near it see (-1 removes it).
    fn crack(&mut self, stage: i32);
    /// `tryBreakBlock`: breaks it with the tool, drops and wear.
    fn break_block(&mut self);
}

impl Breaker {
    /// `handleTicks`.
    fn handle_ticks(&mut self) {
        if self.operation_ticks <= 0 {
            self.operation_ticks = self.tick_speed;
        }
        self.operation_ticks -= 1;
    }

    /// `canRun`: on the clock's tick, or any tick in pulse mode.
    pub fn can_run(&self) -> bool {
        self.operation_ticks == 0 || self.redstone.mode == Mode::Pulse
    }

    fn clear(&mut self, front: &mut dyn Front) {
        if self.tracked.take().is_some() {
            front.crack(-1);
        }
    }

    /// One server tick (`tickServer`: the clock, the redstone, then `doBlockBreak`). `has_tool`: the slot holds an
    /// item; `signal`: `hasNeighborSignal`.
    pub fn tick(&mut self, has_tool: bool, signal: bool, front: &mut dyn Front) {
        self.handle_ticks();
        self.redstone.evaluate(signal);
        // `clearTrackerIfNeeded`: both checks run, the redstone one asking (and in pulse mode using) the pulse.
        if self.tracked.is_some() {
            if !has_tool {
                self.clear(front);
            }
            if !self.redstone.is_active() && self.redstone.mode != Mode::Pulse {
                self.clear(front);
            }
        }
        if !has_tool {
            self.redstone.pulsed = false;
            return;
        }
        if self.redstone.is_active() && self.can_run() && self.tracked.is_none() && front.valid()
            && let Some(state) = front.state()
        {
            self.tracked = Some(Tracked { state, ticks: 0, last_sent: -1, progress: front.progress() });
        }
        if self.tracked.is_none() {
            return;
        }
        if !self.current && self.can_run() {
            self.current = true;
        }
        if self.current && self.mine(front) {
            self.clear(front);
            self.current = false;
        }
    }

    /// `mineBlock`: one tick of mining; true when the block is gone (broken, or no longer the one it started on).
    fn mine(&mut self, front: &mut dyn Front) -> bool {
        let Some(t) = self.tracked.as_mut() else { return true };
        let Some(now) = front.state() else { return true };
        if now != t.state {
            return true;
        }
        t.ticks += 1;
        let done = t.progress * t.ticks as f32;
        let stage = (done * 10.0) as i32;
        if stage != t.last_sent && stage < 10 {
            front.crack(stage);
            t.last_sent = stage;
        }
        if done >= 1.0 {
            front.break_block();
            return true;
        }
        false
    }

    /// `isActiveRedstoneTestOnly`: the block's `active` state.
    pub fn active(&self) -> bool {
        self.redstone.is_active_test()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A block in front that `breaks` ticks of mining remove.
    struct Stone {
        state: Option<u32>,
        progress: f32,
        cracks: Vec<i32>,
        broken: Vec<u32>,
        tick: u32,
    }

    impl Front for Stone {
        fn state(&mut self) -> Option<u32> {
            self.state
        }
        fn valid(&mut self) -> bool {
            self.state.is_some()
        }
        fn progress(&mut self) -> f32 {
            self.progress
        }
        fn crack(&mut self, stage: i32) {
            self.cracks.push(stage);
        }
        fn break_block(&mut self) {
            self.broken.push(self.tick);
            self.state = None;
        }
    }

    fn stone(progress: f32) -> Stone {
        Stone { state: Some(1), progress, cracks: Vec::new(), broken: Vec::new(), tick: 0 }
    }

    #[test]
    fn progress_is_javas() {
        // An iron pickaxe (6) on stone (1.5), correct: 6 / 1.5 / 30.
        assert!((destroy_progress(6.0, 0, 1.5, true) - 6.0 / 1.5 / 30.0).abs() < 1e-6);
        // Efficiency II adds 5 to a speed above 1; a hand (1) gets none.
        assert!((destroy_progress(6.0, 2, 1.5, true) - 11.0 / 1.5 / 30.0).abs() < 1e-6);
        assert!((destroy_progress(1.0, 3, 0.5, false) - 1.0 / 0.5 / 100.0).abs() < 1e-6);
        assert_eq!(destroy_progress(8.0, 0, -1.0, true), -1.0);
        assert!(destroy_progress(1.0, 0, 0.0, false).is_infinite(), "a block of no hardness breaks at once");
    }

    #[test]
    fn it_starts_on_the_clocks_tick_and_breaks_when_the_progress_reaches_one() {
        let mut b = Breaker::default();
        let mut front = stone(6.0 / 1.5 / 30.0);
        for t in 1..=40 {
            front.tick = t;
            b.tick(true, false, &mut front);
        }
        // The clock's first operation is tick 20; 8 ticks of 2/15 mine it, the 8th breaking it.
        assert_eq!(front.broken, [27]);
        assert_eq!(front.cracks, [1, 2, 4, 5, 6, 8, 9, -1]);
        assert_eq!(b.tracked, None);
    }

    #[test]
    fn nothing_without_a_tool_or_with_the_redstone_against_it() {
        let mut b = Breaker::default();
        let mut front = stone(1.0);
        for t in 1..=40 {
            front.tick = t;
            b.tick(false, false, &mut front);
        }
        assert!(front.broken.is_empty());
        b.redstone.mode = Mode::High;
        for t in 41..=80 {
            front.tick = t;
            b.tick(true, false, &mut front);
        }
        assert!(front.broken.is_empty());
        assert!(!b.active());
        for t in 81..=100 {
            front.tick = t;
            b.tick(true, true, &mut front);
        }
        assert_eq!(front.broken, [100], "the clock's tick after the signal came");
    }

    #[test]
    fn a_changed_block_or_a_lost_signal_stops_the_mining() {
        let mut b = Breaker::default();
        let mut front = stone(0.05);
        for t in 1..=22 {
            front.tick = t;
            b.tick(true, false, &mut front);
        }
        assert!(b.tracked.is_some());
        front.state = Some(2);
        b.tick(true, false, &mut front);
        assert_eq!(b.tracked, None, "another block is no longer mined");
        assert_eq!(front.cracks.last(), Some(&-1));
        let mut b = Breaker { redstone: Redstone { mode: Mode::High, ..Redstone::default() }, ..Breaker::default() };
        let mut front = stone(0.05);
        for t in 1..=22 {
            front.tick = t;
            b.tick(true, true, &mut front);
        }
        assert!(b.tracked.is_some());
        b.tick(true, false, &mut front);
        assert_eq!(b.tracked, None, "the signal went: the progress is dropped");
        assert!(front.broken.is_empty());
    }

    #[test]
    fn a_pulse_mines_once_per_edge_on_any_tick() {
        let mut b = Breaker { redstone: Redstone { mode: Mode::Pulse, ..Redstone::default() }, ..Breaker::default() };
        let mut front = stone(1.0);
        front.tick = 1;
        b.tick(true, false, &mut front);
        front.tick = 2;
        b.tick(true, true, &mut front);
        assert_eq!(front.broken, [2], "pulse mode runs on any tick");
        front.state = Some(1);
        for t in 3..=10 {
            front.tick = t;
            b.tick(true, true, &mut front);
        }
        assert_eq!(front.broken, [2], "a held signal is one pulse");
    }
}
