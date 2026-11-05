//! Mekanism's Fluid Tank (`TileEntityFluidTank` with its `FluidTankFluidTank`
//! over `BasicFluidTank`): one tank of the tier's mB on every side, which
//! other blocks (`AutomationType.EXTERNAL`) fill and drain without a rate
//! limit and its own moves (`INTERNAL`) at the tier's output a tick; while
//! active it gives what it holds down (`FluidUtils.emit` into the block below,
//! at most what a fluid tank there still takes). A creative tank simulates
//! every move: it keeps what it holds, gives without losing and voids what
//! another block puts in, and only its own moves fill an empty one to its
//! capacity. Amounts are mB in Java's `int`; which fluid it is is the
//! caller's (`same`: the tank is empty or holds the offered fluid).
use crate::container::Automation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FluidTank {
    pub capacity: i32,
    /// The tier's output: the rate of its own moves.
    pub output: i32,
    pub creative: bool,
}

/// What a move does: how much went, and what the tank then holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Moved {
    pub amount: i32,
    pub stored: i32,
}

impl FluidTank {
    fn rate(&self, automation: Automation) -> i32 {
        if automation == Automation::Internal { self.output } else { i32::MAX }
    }

    /// `insert` of `amount` into a tank holding `stored`: none of another
    /// fluid (`same` false), what fits the rate and the room. `execute`
    /// false only simulates.
    pub fn insert(&self, stored: i32, same: bool, amount: i32, automation: Automation, execute: bool) -> Moved {
        let unchanged = Moved { amount: 0, stored };
        if amount <= 0 || !same {
            return unchanged;
        }
        let needed = self.rate(automation).min(self.capacity.saturating_sub(stored));
        if needed <= 0 {
            return unchanged;
        }
        let took = amount.min(needed);
        if self.creative {
            // Only its own move fills an empty creative tank, and only when all of it fits.
            let fills = stored == 0 && execute && automation != Automation::External && took == amount;
            return Moved { amount: took, stored: if fills { self.capacity } else { stored } };
        }
        Moved { amount: took, stored: if execute { stored + took } else { stored } }
    }

    /// `extract` of up to `amount` out of a tank holding `stored`.
    pub fn extract(&self, stored: i32, amount: i32, automation: Automation, execute: bool) -> Moved {
        let took = amount.min(stored).min(self.rate(automation)).max(0);
        let keeps = self.creative || !execute;
        Moved { amount: took, stored: if keeps { stored } else { stored - took } }
    }

    /// What an active tank offers the block below a tick: its output, at most
    /// what a fluid tank there still takes (`getBelowTank().getNeeded()`).
    pub fn eject_limit(&self, below_tank_needed: Option<i32>) -> i32 {
        below_tank_needed.map_or(self.output, |needed| needed.min(self.output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Automation::{External, Internal};

    const BASIC: FluidTank = FluidTank { capacity: 32_000, output: 1_000, creative: false };
    const ULTIMATE: FluidTank = FluidTank { capacity: 256_000, output: 64_000, creative: false };
    const CREATIVE: FluidTank = FluidTank { capacity: i32::MAX, output: i32::MAX / 2, creative: true };

    #[test]
    fn other_blocks_move_any_amount_and_its_own_moves_the_output() {
        assert_eq!(BASIC.insert(0, true, 50_000, External, true), Moved { amount: 32_000, stored: 32_000 });
        assert_eq!(BASIC.insert(0, true, 50_000, Internal, true), Moved { amount: 1_000, stored: 1_000 });
        assert_eq!(BASIC.insert(100, false, 50, External, true), Moved { amount: 0, stored: 100 }, "another fluid");
        assert_eq!(BASIC.insert(100, true, 50, External, false), Moved { amount: 50, stored: 100 }, "simulated");
        assert_eq!(ULTIMATE.extract(100_000, 250, External, true), Moved { amount: 250, stored: 99_750 });
        assert_eq!(ULTIMATE.extract(100_000, 200_000, Internal, true), Moved { amount: 64_000, stored: 36_000 });
    }

    /// The oracle (`mekm2-work/oracle`, x3b P4): an active ultimate tank of 100 000 above an empty basic tank gives
    /// it 32 000 a tick, above one of 31 500 the 500 it still takes.
    #[test]
    fn an_active_tank_gives_down_what_a_tank_below_takes() {
        assert_eq!(ULTIMATE.eject_limit(Some(32_000)), 32_000);
        assert_eq!(ULTIMATE.eject_limit(Some(500)), 500);
        assert_eq!(ULTIMATE.eject_limit(None), 64_000, "a pipe below: the output");
        let given = ULTIMATE.extract(100_000, ULTIMATE.eject_limit(Some(32_000)), Internal, true);
        assert_eq!(given, Moved { amount: 32_000, stored: 68_000 });
    }

    /// The oracle (x3 P7): a creative tank holding 10 mB gives a pipe its 10 a tick and keeps them.
    #[test]
    fn a_creative_tank_keeps_what_it_gives_and_voids_what_it_gets() {
        assert_eq!(CREATIVE.extract(10, 250, External, true), Moved { amount: 10, stored: 10 });
        assert_eq!(CREATIVE.insert(10, true, 500, External, true), Moved { amount: 500, stored: 10 }, "voided");
        assert_eq!(CREATIVE.insert(0, true, 500, External, true), Moved { amount: 500, stored: 0 }, "an empty one stays empty");
        assert_eq!(CREATIVE.insert(0, true, 500, Internal, true), Moved { amount: 500, stored: i32::MAX }, "its own slot fills it");
    }
}
