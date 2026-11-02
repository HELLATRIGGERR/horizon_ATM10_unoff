//! `TransferHandler`, `FluxPlugHandler` and `FluxPointHandler`: one device's
//! buffer, limits and priority. What a device does to its neighbours and the
//! network is in `world`.

/// `TransferHandler`'s bounds of a user's priority.
pub const PRI_USER_MIN: i32 = -9999;
pub const PRI_USER_MAX: i32 = 9999;
/// The priority a device in surge mode has, whatever its own.
pub const PRI_GAIN_MAX: i32 = 100_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Plug,
    Point,
}

impl Kind {
    /// The block and block entity id.
    pub fn id(self) -> &'static str {
        match self {
            Self::Plug => "fluxnetworks:flux_plug",
            Self::Point => "fluxnetworks:flux_point",
        }
    }

    pub fn of_block(id: &str) -> Option<Self> {
        match id {
            "fluxnetworks:flux_plug" => Some(Self::Plug),
            "fluxnetworks:flux_point" => Some(Self::Point),
            _ => None,
        }
    }
}

/// A device's settings and buffer as saved in its block entity.
#[derive(Clone, Debug, PartialEq)]
pub struct Saved {
    pub network: i32,
    pub name: String,
    pub owner: u128,
    pub priority: i32,
    pub surge: bool,
    pub limit: i64,
    pub disable_limit: bool,
    pub energy: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Handler {
    pub kind: Kind,
    /// `mBuffer`: at most a tick's worth, moved on at the next cycle.
    pub buffer: i64,
    /// `mChange`: what left (a point, negative) or came in (a plug) from
    /// outside in the last cycle.
    pub change: i64,
    /// `mPriority`, the user's.
    pub priority: i32,
    /// `mSurgeMode`: served first, whatever the priority.
    pub surge: bool,
    /// `mLimit`, the user's; never negative.
    pub limit: i64,
    /// `mDisableLimit`: no limit at all.
    pub disable_limit: bool,
    /// A plug's `mReceived`, from outside since the last cycle.
    pub received: i64,
    /// A plug's `mRemoved`, given to points in this cycle.
    pub removed: i64,
    /// A point's `mDesired`: what its neighbours would take this cycle.
    pub desired: i64,
}

impl Handler {
    pub fn new(kind: Kind, limit: i64) -> Self {
        Self {
            kind,
            buffer: 0,
            change: 0,
            priority: 0,
            surge: false,
            limit: limit.max(0),
            disable_limit: false,
            received: 0,
            removed: 0,
            desired: 0,
        }
    }

    /// `getPriority`: surge wins over any priority a user can set.
    pub fn logical_priority(&self) -> i32 {
        if self.surge { PRI_GAIN_MAX } else { self.priority }
    }

    /// `getLimit`.
    pub fn logical_limit(&self) -> i64 {
        if self.disable_limit { i64::MAX } else { self.limit }
    }

    /// `setPriority`, clamped to what a user can set.
    pub fn set_priority(&mut self, priority: i32) {
        self.priority = priority.clamp(PRI_USER_MIN, PRI_USER_MAX);
    }

    /// `getRequest`: what a point still wants from the network.
    pub fn request(&self) -> i64 {
        match self.kind {
            Kind::Plug => 0,
            Kind::Point => (self.desired - self.buffer).max(0),
        }
    }

    /// `getMaxEnergyStoredL`.
    pub fn capacity(&self) -> i64 {
        self.buffer.max(self.logical_limit())
    }

    /// `FluxPlugHandler.receive`, against the network's `limiter`, as written
    /// there: the plug's buffer counts twice against both bounds.
    pub fn plug_receive(&mut self, max: i64, limiter: i64, simulate: bool) -> i64 {
        let op = (self.logical_limit().min(limiter.saturating_sub(self.buffer)))
            .saturating_sub(self.buffer)
            .min(max);
        if op <= 0 {
            return 0;
        }
        if !simulate {
            self.buffer += op;
            self.received += op;
        }
        op
    }

    /// `FluxPlugHandler.removeFromBuffer`.
    pub fn plug_remove(&mut self, energy: i64) -> i64 {
        let op = energy.min(self.buffer).min(self.logical_limit() - self.removed).max(0);
        self.buffer -= op;
        self.removed += op;
        op
    }

    /// `onCycleEnd` of a plug.
    pub fn plug_end(&mut self) {
        self.change = self.received;
        self.received = 0;
        self.removed = 0;
    }

    /// Takes the settings of a save. Whether the order among the network's
    /// devices may have changed.
    pub fn apply(&mut self, saved: &Saved) -> bool {
        let before = self.logical_priority();
        self.set_priority(saved.priority);
        self.surge = saved.surge;
        self.limit = saved.limit.max(0);
        self.disable_limit = saved.disable_limit;
        before != self.logical_priority()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plug_takes_nothing_while_the_network_asks_for_nothing() {
        let mut plug = Handler::new(Kind::Plug, 800_000);
        assert_eq!(plug.plug_receive(1000, 0, false), 0);
        assert_eq!(plug.buffer, 0);
    }

    #[test]
    fn a_plug_takes_up_to_its_limit_and_the_network_request() {
        let mut plug = Handler::new(Kind::Plug, 800);
        assert_eq!(plug.plug_receive(1000, 500, true), 500, "the request bounds it");
        assert_eq!(plug.buffer, 0, "simulated");
        let mut plug = Handler::new(Kind::Plug, 800);
        assert_eq!(plug.plug_receive(1000, 5000, false), 800, "the limit bounds it");
        assert_eq!((plug.buffer, plug.received), (800, 800));
        assert_eq!(plug.plug_receive(1000, 5000, false), 0, "buffer full");
        let mut plug = Handler::new(Kind::Plug, 800);
        assert_eq!(plug.plug_receive(100, 5000, false), 100, "the offer bounds it");
    }

    #[test]
    fn the_buffer_counts_twice_as_in_the_mod() {
        let mut plug = Handler::new(Kind::Plug, 1000);
        plug.buffer = 100;
        // min(min(1000, 500 - 100) - 100, 9999)
        assert_eq!(plug.plug_receive(9999, 500, false), 300);
    }

    #[test]
    fn removed_energy_counts_against_the_limit_within_a_cycle() {
        let mut plug = Handler::new(Kind::Plug, 100);
        plug.buffer = 500;
        assert_eq!(plug.plug_remove(70), 70);
        assert_eq!(plug.plug_remove(70), 30);
        assert_eq!(plug.plug_remove(70), 0);
        assert_eq!(plug.buffer, 400);
        plug.plug_end();
        assert_eq!(plug.plug_remove(70), 70);
    }

    #[test]
    fn surge_and_bypass_change_only_the_logical_values() {
        let mut h = Handler::new(Kind::Point, 5);
        h.set_priority(20000);
        assert_eq!(h.priority, PRI_USER_MAX);
        h.surge = true;
        assert_eq!(h.logical_priority(), PRI_GAIN_MAX);
        h.disable_limit = true;
        assert_eq!(h.logical_limit(), i64::MAX);
        assert_eq!(h.limit, 5);
    }

    #[test]
    fn a_point_requests_what_its_neighbours_want_less_its_buffer() {
        let mut h = Handler::new(Kind::Point, 100);
        h.desired = 90;
        h.buffer = 30;
        assert_eq!(h.request(), 60);
        h.buffer = 120;
        assert_eq!(h.request(), 0);
        assert_eq!(h.capacity(), 120);
    }

    #[test]
    fn long_amounts_pass() {
        let mut plug = Handler::new(Kind::Plug, 0);
        plug.disable_limit = true;
        let amount = 5_000_000_000_000_i64;
        assert_eq!(plug.plug_receive(amount, i64::MAX, false), amount);
        assert_eq!(plug.plug_remove(i64::MAX), amount);
    }
}
