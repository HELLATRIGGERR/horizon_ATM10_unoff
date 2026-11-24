//! Oritech's fluid storage (`SimpleFluidStorage`, `SimpleInOutFluidStorage`): a tank holds one fluid and an amount in
//! millibuckets (NeoForge's bucket is 1,000). Components of a stack are not kept: a fluid of Oritech's own never has any.
//! Translated from Oritech 1.2.13 (CC0).

/// One tank's content: a registry name (empty for none) and an amount.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tank {
    pub fluid: String,
    pub amount: i32,
}

/// `FluidStackHooks.bucketAmount()` on NeoForge.
pub const BUCKET: i32 = 1000;

impl Tank {
    pub fn new(fluid: &str, amount: i32) -> Self {
        Self { fluid: fluid.to_owned(), amount }
    }

    pub fn is_empty(&self) -> bool {
        self.fluid.is_empty() || self.amount <= 0
    }

    /// `insertTo`: an empty tank takes up to its capacity, one with the same fluid the room left, another fluid
    /// nothing. Returns what went in.
    pub fn insert(&mut self, fluid: &str, amount: i32, capacity: i32, simulate: bool) -> i32 {
        if fluid.is_empty() || amount <= 0 {
            return 0;
        }
        if self.is_empty() {
            let put = amount.min(capacity).max(0);
            if !simulate && put > 0 {
                *self = Tank::new(fluid, put);
            }
            return put;
        }
        if self.fluid != fluid {
            return 0;
        }
        let put = amount.min((capacity - self.amount).max(0));
        if !simulate {
            self.amount += put;
        }
        put
    }

    /// `extractFrom`: the same fluid, up to what is held. An emptied tank is cleared.
    pub fn extract(&mut self, fluid: &str, amount: i32, simulate: bool) -> i32 {
        if self.is_empty() || self.fluid != fluid {
            return 0;
        }
        let took = amount.min(self.amount).max(0);
        if !simulate {
            self.amount -= took;
            if self.amount <= 0 {
                *self = Tank::default();
            }
        }
        took
    }

    /// Takes `amount` of whatever is held (`extract(getStack().copyWithAmount(n))`).
    pub fn take(&mut self, amount: i32, simulate: bool) -> i32 {
        let fluid = self.fluid.clone();
        self.extract(&fluid, amount, simulate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tank_takes_one_fluid_up_to_its_capacity() {
        let mut t = Tank::default();
        assert_eq!(t.insert("oritech:still_diesel", 3000, 4000, true), 3000);
        assert!(t.is_empty(), "a simulation changes nothing");
        assert_eq!(t.insert("oritech:still_diesel", 3000, 4000, false), 3000);
        assert_eq!(t.insert("oritech:still_diesel", 3000, 4000, false), 1000, "the room left");
        assert_eq!(t.insert("minecraft:lava", 1000, 4000, false), 0, "another fluid");
        assert_eq!(t.amount, 4000);
        assert_eq!(t.extract("minecraft:lava", 10, false), 0);
        assert_eq!(t.take(1500, false), 1500);
        assert_eq!(t.take(9000, false), 2500);
        assert_eq!(t, Tank::default(), "an emptied tank is cleared");
        assert_eq!(Tank::new("a:b", 5).insert("a:b", 9, 4, false), 0, "a tank over its capacity takes nothing");
    }
}
