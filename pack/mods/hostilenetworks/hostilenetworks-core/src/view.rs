//! What the two screens show, carried in a block entity's menu data. A data slot carries 16 signed bits, so an
//! amount goes as two 15-bit pieces (30 bits, more than any energy or data here), which never reads as negative.
//! The screens' text and drop lists come from the registry both sides share; only the numbers travel.

/// The most a packed amount holds.
pub const MAX: i32 = (1 << 30) - 1;

fn put(out: &mut Vec<i32>, value: i32) {
    let v = value.clamp(0, MAX);
    out.push(v & 0x7FFF);
    out.push((v >> 15) & 0x7FFF);
}

fn get(data: &[i32], at: usize) -> i32 {
    (data[at] & 0x7FFF) | ((data[at + 1] & 0x7FFF) << 15)
}

/// A small value: one slot.
fn small(out: &mut Vec<i32>, value: i32) {
    out.push(value.clamp(0, 0x7FFF));
}

/// The Simulation Chamber's screen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChamberView {
    pub energy: i32,
    pub capacity: i32,
    pub runtime: i32,
    pub mode: i32,
    pub redstone: i32,
    pub fail: i32,
    pub success: i32,
    /// The model's place among the sorted model ids, plus one; 0 is none.
    pub model: i32,
    pub data: i32,
    pub tier_data: i32,
    pub next_tier_data: i32,
    pub tier: i32,
    pub is_max: bool,
    /// The model's FE a tick in the chamber's mode.
    pub cost: i32,
    pub iters: i32,
    /// The accuracy in ten-thousandths (1.0 is 10000).
    pub accuracy: i32,
    pub can_train: bool,
}

impl ChamberView {
    pub const SLOTS: usize = 24;

    pub fn pack(&self) -> Vec<i32> {
        let mut o = Vec::with_capacity(Self::SLOTS);
        put(&mut o, self.energy);
        put(&mut o, self.capacity);
        for v in [self.runtime, self.mode, self.redstone, self.fail, self.success, self.model] {
            small(&mut o, v);
        }
        put(&mut o, self.data);
        put(&mut o, self.tier_data);
        put(&mut o, self.next_tier_data);
        small(&mut o, self.tier);
        small(&mut o, i32::from(self.is_max));
        put(&mut o, self.cost);
        small(&mut o, self.iters);
        put(&mut o, self.accuracy);
        small(&mut o, i32::from(self.can_train));
        debug_assert_eq!(o.len(), Self::SLOTS);
        o
    }

    /// The view in `data`; none until all of the menu's data has come.
    pub fn unpack(d: &[i32]) -> Option<Self> {
        if d.len() < Self::SLOTS {
            return None;
        }
        let s = |i: usize| d[i] & 0x7FFF;
        Some(Self {
            energy: get(d, 0),
            capacity: get(d, 2),
            runtime: s(4),
            mode: s(5),
            redstone: s(6),
            fail: s(7),
            success: s(8),
            model: s(9),
            data: get(d, 10),
            tier_data: get(d, 12),
            next_tier_data: get(d, 14),
            tier: s(16),
            is_max: s(17) != 0,
            cost: get(d, 18),
            iters: s(20),
            accuracy: get(d, 21),
            can_train: s(23) != 0,
        })
    }

    /// Rows of an 87-row bar left dark (`87 - ceil(87 * part / whole)`).
    pub fn dark_rows(part: i32, whole: i32) -> i32 {
        if whole <= 0 {
            return 87;
        }
        87 - (87.0 * part as f32 / whole as f32).ceil() as i32
    }
}

/// How many queue entries the fabricator's screen carries.
pub const QUEUE_SHOWN: usize = 9;

/// The Loot Fabricator's screen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FabView {
    pub energy: i32,
    pub capacity: i32,
    pub runtime: i32,
    pub redstone: i32,
    pub model: i32,
    /// The drop chosen now, plus one; 0 is none.
    pub selected: i32,
    /// 0 fixed, 1 queue.
    pub mode: i32,
    pub queue_len: i32,
    pub cursor: i32,
    /// The first entries of the queue, each plus one; 0 is empty.
    pub queue: [i32; QUEUE_SHOWN],
    pub cost: i32,
}

impl FabView {
    pub const SLOTS: usize = 4 + 7 + QUEUE_SHOWN + 2;

    pub fn pack(&self) -> Vec<i32> {
        let mut o = Vec::with_capacity(Self::SLOTS);
        put(&mut o, self.energy);
        put(&mut o, self.capacity);
        for v in [self.runtime, self.redstone, self.model, self.selected, self.mode, self.queue_len, self.cursor] {
            small(&mut o, v);
        }
        for v in self.queue {
            small(&mut o, v);
        }
        put(&mut o, self.cost);
        debug_assert_eq!(o.len(), Self::SLOTS);
        o
    }

    pub fn unpack(d: &[i32]) -> Option<Self> {
        if d.len() < Self::SLOTS {
            return None;
        }
        let s = |i: usize| d[i] & 0x7FFF;
        Some(Self {
            energy: get(d, 0),
            capacity: get(d, 2),
            runtime: s(4),
            redstone: s(5),
            model: s(6),
            selected: s(7),
            mode: s(8),
            queue_len: s(9),
            cursor: s(10),
            queue: std::array::from_fn(|i| s(11 + i)),
            cost: get(d, 11 + QUEUE_SHOWN),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chamber_view_survives_the_wire() {
        let v = ChamberView {
            energy: 1_999_999,
            capacity: 2_000_000,
            runtime: 240,
            mode: 1,
            redstone: 2,
            fail: 6,
            success: 2,
            model: 17,
            data: 1254,
            tier_data: 354,
            next_tier_data: 1254,
            tier: 3,
            is_max: false,
            cost: 154,
            iters: 99,
            accuracy: 6500,
            can_train: true,
        };
        let wire: Vec<i32> = v.pack().into_iter().map(|x| i32::from(x as i16)).collect();
        assert_eq!(wire.len(), ChamberView::SLOTS);
        assert_eq!(ChamberView::unpack(&wire), Some(v));
        assert_eq!(ChamberView::unpack(&wire[..10]), None);
    }

    #[test]
    fn a_fab_view_survives_the_wire() {
        let v = FabView {
            energy: 1_000_000,
            capacity: 1_000_000,
            runtime: 59,
            redstone: 1,
            model: 3,
            selected: 2,
            mode: 1,
            queue_len: 11,
            cursor: 4,
            queue: [1, 2, 3, 4, 5, 6, 7, 8, 0],
            cost: 256,
        };
        let wire: Vec<i32> = v.pack().into_iter().map(|x| i32::from(x as i16)).collect();
        assert_eq!(wire.len(), FabView::SLOTS);
        assert_eq!(FabView::unpack(&wire), Some(v));
    }

    #[test]
    fn the_bars_go_dark_from_the_top() {
        assert_eq!((ChamberView::dark_rows(0, 100), ChamberView::dark_rows(100, 100), ChamberView::dark_rows(1, 100)), (87, 0, 86));
        assert_eq!(ChamberView::dark_rows(5, 0), 87);
    }
}
