//! Heat: the pyromantic brazier burns furnace fuel and is lit while it burns (`PyromanticBrazierBlockEntity`), and an
//! apparatus above a lit brazier is heated (`HeatConsumerBehaviour`), looked at every 20 game ticks.

/// `HeatConsumerBehaviour.CHECK_HEAT_TICK_INTERVAL`.
pub const CHECK_HEAT_TICK_INTERVAL: i64 = 20;

/// `HeatConsumerBehaviour`: `heatedCache`, which Java doesn't save (false after a load until the next look).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeatConsumer {
    pub heated: bool,
}

impl HeatConsumer {
    /// `isHeated()`: on a game tick divisible by 20, the heat provider below (a lit brazier), else the heat
    /// receiver's `isHotUntil` (a caloric flux emitter's) against the game time. Returns whether it is heated and
    /// whether that changed this tick (then Java sets the block's `lit` and marks the entity changed).
    pub fn is_heated(&mut self, game_time: i64, hot_below: impl FnOnce() -> bool, hot_until: i64) -> (bool, bool) {
        if game_time % CHECK_HEAT_TICK_INTERVAL == 0 {
            let was = self.heated;
            let heated = hot_below() || hot_until > game_time;
            self.heated = heated;
            return (heated, was != heated);
        }
        (self.heated, false)
    }
}

/// The brazier's fuel slot as a tick sees it: the stack's burn time (`getBurnTime`, furnace fuel data) and whether
/// using one leaves a remainder (a lava bucket's bucket).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fuel {
    pub burn_time: i32,
    pub has_remainder: bool,
}

/// What a brazier tick does to its fuel slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Consume {
    None,
    /// `hasCraftingRemainingItem`: the slot becomes the remainder (the whole stack, as Java sets it).
    Remainder,
    /// One item used (`shrink(1)`; an emptied slot takes the item's remainder, which is none here).
    One,
}

/// One tick of a brazier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BrazierStep {
    pub consume: Consume,
    /// The block's `lit` changed to this (`setBlock(.., 3)`).
    pub lit: Option<bool>,
    /// `setChanged`.
    pub changed: bool,
}

/// `PyromanticBrazierBlockEntity`'s saved `remainingLitTime`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Brazier {
    pub remaining_lit_time: i32,
}

impl Brazier {
    pub fn is_lit(&self) -> bool {
        self.remaining_lit_time > 0
    }

    /// `tickServer`: a lit tick burns one; an unlit brazier with fuel lights for the fuel's burn time and uses it.
    pub fn tick(&mut self, fuel: Option<Fuel>) -> BrazierStep {
        let mut turned_on = false;
        let was_lit = self.is_lit();
        if was_lit {
            self.remaining_lit_time -= 1;
        }
        let mut consume = Consume::None;
        if !self.is_lit()
            && let Some(f) = fuel
        {
            self.remaining_lit_time = f.burn_time.max(0);
            if self.is_lit() {
                turned_on = true;
                consume = if f.has_remainder { Consume::Remainder } else { Consume::One };
            }
        }
        let mut lit = None;
        if was_lit != self.is_lit() {
            turned_on = true;
            lit = Some(self.is_lit());
        }
        BrazierStep { consume, lit, changed: turned_on }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COAL: Fuel = Fuel { burn_time: 1600, has_remainder: false };

    #[test]
    fn a_coal_burns_1600_ticks_then_the_next() {
        let mut b = Brazier::default();
        let first = b.tick(Some(COAL));
        assert_eq!(first, BrazierStep { consume: Consume::One, lit: Some(true), changed: true });
        let mut ticks = 1;
        loop {
            let s = b.tick(None);
            ticks += 1;
            if s.lit == Some(false) {
                break;
            }
            assert_eq!(s.consume, Consume::None);
        }
        assert_eq!(ticks, 1601, "lit on the first tick, out 1600 ticks later");
        // With fuel left, the tick that would go out lights the next one instead: no flicker.
        let mut b = Brazier::default();
        b.tick(Some(COAL));
        for _ in 0..1599 {
            assert_eq!(b.tick(Some(COAL)).consume, Consume::None);
        }
        let s = b.tick(Some(COAL));
        assert_eq!((s.consume, s.lit), (Consume::One, None));
    }

    #[test]
    fn lava_leaves_its_bucket_and_non_fuel_lights_nothing() {
        let mut b = Brazier::default();
        assert_eq!(b.tick(Some(Fuel { burn_time: 20000, has_remainder: true })).consume, Consume::Remainder);
        assert_eq!(b.remaining_lit_time, 20000);
        let mut b = Brazier::default();
        assert_eq!(b.tick(Some(Fuel { burn_time: 0, has_remainder: false })), BrazierStep { consume: Consume::None, lit: None, changed: false });
    }

    #[test]
    fn heat_is_looked_at_every_20_ticks() {
        let mut h = HeatConsumer::default();
        assert_eq!(h.is_heated(19, || true, 0), (false, false), "not a 20th tick: the cache");
        assert_eq!(h.is_heated(20, || true, 0), (true, true));
        assert_eq!(h.is_heated(21, || false, 0), (true, false));
        assert_eq!(h.is_heated(40, || false, 0), (false, true));
        assert_eq!(h.is_heated(60, || false, 61), (true, true), "a heat receiver hot until later");
        assert_eq!(h.is_heated(80, || false, 61), (false, true));
    }
}
