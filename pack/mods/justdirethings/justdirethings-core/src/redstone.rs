//! A machine's redstone control (`RedstoneControlledBE`, `RedstoneControlData`, `MiscHelpers.RedstoneMode`).

/// `RedstoneMode`, in its ordinal order (the saved `redstoneMode` and the screen's button).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// Runs whatever the signal.
    #[default]
    Ignored,
    /// Runs while unpowered.
    Low,
    /// Runs while powered.
    High,
    /// Runs once for each rising edge.
    Pulse,
}

impl Mode {
    pub const ALL: [Self; 4] = [Self::Ignored, Self::Low, Self::High, Self::Pulse];

    pub fn index(self) -> i32 {
        self as i32
    }

    pub fn from_index(i: i32) -> Self {
        Self::ALL.get(usize::try_from(i).unwrap_or(0)).copied().unwrap_or_default()
    }

    /// The next one, as the screen's toggle button cycles.
    pub fn next(self) -> Self {
        Self::from_index((self.index() + 1) % 4)
    }
}

/// `RedstoneControlData`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Redstone {
    pub mode: Mode,
    pub receiving: bool,
    pub pulsed: bool,
}

impl Redstone {
    /// `evaluateRedstone` with the block's `hasNeighborSignal`: a rising edge in pulse mode arms one run.
    pub fn evaluate(&mut self, signal: bool) {
        if self.mode == Mode::Pulse && !self.receiving && signal {
            self.pulsed = true;
        }
        self.receiving = signal;
    }

    /// `isActiveRedstone`: whether the machine runs now; a pulse is used up by asking.
    pub fn is_active(&mut self) -> bool {
        match self.mode {
            Mode::Ignored => true,
            Mode::Low => !self.receiving,
            Mode::High => self.receiving,
            Mode::Pulse if self.pulsed => {
                self.pulsed = false;
                true
            }
            Mode::Pulse => false,
        }
    }

    /// `isActiveRedstoneTestOnly`: the same without using the pulse up.
    pub fn is_active_test(&self) -> bool {
        match self.mode {
            Mode::Ignored => true,
            Mode::Low => !self.receiving,
            Mode::High => self.receiving,
            Mode::Pulse => self.pulsed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_follow_the_signal() {
        let mut r = Redstone::default();
        assert!(r.is_active());
        r.mode = Mode::Low;
        r.evaluate(true);
        assert!(!r.is_active());
        r.evaluate(false);
        assert!(r.is_active());
        r.mode = Mode::High;
        assert!(!r.is_active());
        r.evaluate(true);
        assert!(r.is_active() && r.is_active());
    }

    #[test]
    fn a_pulse_runs_once_per_rising_edge() {
        let mut r = Redstone { mode: Mode::Pulse, ..Redstone::default() };
        r.evaluate(false);
        assert!(!r.is_active());
        r.evaluate(true);
        assert!(r.is_active_test());
        assert!(r.is_active());
        assert!(!r.is_active(), "asking uses the pulse up");
        r.evaluate(true);
        assert!(!r.is_active(), "a held signal is no new edge");
        r.evaluate(false);
        r.evaluate(true);
        assert!(r.is_active());
        assert_eq!(Mode::Pulse.next(), Mode::Ignored);
        assert_eq!(Mode::from_index(9), Mode::Ignored);
    }
}
