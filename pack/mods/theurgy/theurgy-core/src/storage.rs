//! The slots and tanks the apparatus keep: NeoForge's `ItemStackHandler` (one stack a slot, up to 64 and the item's
//! own maximum) and `FluidTank` (one fluid, a capacity, a validator), as Theurgy's handlers use them.

/// `ItemStackHandler.insertItem` into one slot: how many of `incoming` it takes. `held` is the slot's count of the
/// same item (`None` when it holds another item), 0 when empty; `max` the item's maximum stack size.
pub fn insert(held: Option<u32>, incoming: u32, max: u32, valid: bool) -> u32 {
    if incoming == 0 || !valid {
        return 0;
    }
    let Some(held) = held else { return 0 };
    let limit = max.min(64).saturating_sub(held);
    limit.min(incoming)
}

/// `ItemHandlerHelper.insertItemStacked(.., simulate)` into a one-slot output: whether all of `count` fits.
pub fn fits_whole(held: Option<u32>, count: u32, max: u32) -> bool {
    insert(held, count, max, true) == count
}

/// A `FluidTank`: one fluid (none when empty), its amount and capacity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tank {
    pub fluid: Option<String>,
    pub amount: i32,
    pub capacity: i32,
}

impl Tank {
    pub fn new(capacity: i32) -> Self {
        Self { fluid: None, amount: 0, capacity }
    }

    pub fn is_empty(&self) -> bool {
        self.fluid.is_none() || self.amount <= 0
    }

    /// The fluid held, `""` when empty.
    pub fn fluid_id(&self) -> &str {
        if self.is_empty() { "" } else { self.fluid.as_deref().unwrap_or("") }
    }

    /// `FluidTank.fill`: what it takes of `amount` of `fluid` (`valid`: the tank's validator said yes).
    pub fn fill(&mut self, fluid: &str, amount: i32, valid: bool, simulate: bool) -> i32 {
        if amount <= 0 || fluid.is_empty() || !valid {
            return 0;
        }
        if self.is_empty() {
            let taken = amount.min(self.capacity);
            if !simulate && taken > 0 {
                self.fluid = Some(fluid.to_owned());
                self.amount = taken;
            }
            return taken;
        }
        if self.fluid.as_deref() != Some(fluid) {
            return 0;
        }
        let taken = (self.capacity - self.amount).max(0).min(amount);
        if !simulate {
            self.amount += taken;
        }
        taken
    }

    /// `FluidTank.drain(maxDrain)`: up to `max` of what it holds.
    pub fn drain(&mut self, max: i32, simulate: bool) -> Option<(String, i32)> {
        if self.is_empty() || max <= 0 {
            return None;
        }
        let fluid = self.fluid.clone()?;
        let drained = self.amount.min(max);
        if !simulate {
            self.amount -= drained;
            if self.amount <= 0 {
                self.fluid = None;
                self.amount = 0;
            }
        }
        Some((fluid, drained))
    }

    /// `FluidTank.drain(resource)`: only that fluid.
    pub fn drain_fluid(&mut self, fluid: &str, max: i32, simulate: bool) -> Option<(String, i32)> {
        if self.fluid_id() != fluid {
            return None;
        }
        self.drain(max, simulate)
    }
}

/// A bucket used on a one-tank apparatus (`OneTankFluidHandlerBehaviour` over `FluidUtil.tryFluidTransfer`): a full
/// bucket empties into the tank only if the tank takes the whole bucket; an empty bucket fills from a tank holding
/// at least a bucket.
pub const BUCKET: i32 = 1000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_take_up_to_the_stack_size() {
        assert_eq!(insert(Some(0), 64, 64, true), 64);
        assert_eq!(insert(Some(60), 10, 64, true), 4);
        assert_eq!(insert(Some(0), 10, 16, true), 10);
        assert_eq!(insert(Some(16), 1, 16, true), 0);
        assert_eq!(insert(None, 10, 64, true), 0, "another item in the slot");
        assert_eq!(insert(Some(0), 10, 64, false), 0, "refused by isItemValid");
        assert!(fits_whole(Some(63), 1, 64) && !fits_whole(Some(64), 1, 64) && !fits_whole(None, 1, 64));
    }

    #[test]
    fn tanks_fill_and_drain() {
        let mut t = Tank::new(2000);
        assert_eq!(t.fill("theurgy:sal_ammoniac", 1000, true, false), 1000);
        assert_eq!(t.fill("theurgy:sal_ammoniac", 1500, true, true), 1000, "simulated: what fits");
        assert_eq!(t.amount, 1000);
        assert_eq!(t.fill("minecraft:water", 100, true, false), 0, "one fluid a tank");
        assert_eq!(t.fill("theurgy:sal_ammoniac", 100, false, false), 0, "the validator");
        assert_eq!(t.drain(10, false), Some(("theurgy:sal_ammoniac".into(), 10)));
        assert_eq!(t.drain(5000, false), Some(("theurgy:sal_ammoniac".into(), 990)));
        assert!(t.is_empty() && t.fluid.is_none());
        assert_eq!(t.drain(1, false), None);
        assert_eq!(t.drain_fluid("minecraft:water", 1, true), None);
    }
}
