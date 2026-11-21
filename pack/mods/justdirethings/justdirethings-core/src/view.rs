//! What the Generator T1's screen shows, carried in its menu's data (`GeneratorT1BE.poweredMachineData` and the
//! numbers its tooltips read). A data slot carries 16 signed bits, so an amount goes as two 15-bit pieces (30 bits),
//! which never reads as negative.

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

/// The Generator T1's screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratorView {
    pub energy: i32,
    pub capacity: i32,
    pub burn_remaining: i32,
    pub max_burn: i32,
    /// FE a tick while it burns (`fePerTick`).
    pub fe_per_tick: i32,
    /// The redstone mode's ordinal.
    pub redstone: i32,
}

impl GeneratorView {
    pub const SLOTS: usize = 12;

    pub fn pack(&self) -> Vec<i32> {
        let mut o = Vec::with_capacity(Self::SLOTS);
        for v in [self.energy, self.capacity, self.burn_remaining, self.max_burn, self.fe_per_tick, self.redstone] {
            put(&mut o, v);
        }
        o
    }

    /// The view in `data`; none until all of the menu's data has come.
    pub fn unpack(d: &[i32]) -> Option<Self> {
        (d.len() >= Self::SLOTS).then(|| Self {
            energy: get(d, 0),
            capacity: get(d, 2),
            burn_remaining: get(d, 4),
            max_burn: get(d, 6),
            fe_per_tick: get(d, 8),
            redstone: get(d, 10),
        })
    }

    /// Rows of the 70-row energy bar that are lit (`energy * 70 / max`).
    pub fn energy_rows(&self) -> i32 {
        if self.capacity <= 0 { 0 } else { (i64::from(self.energy) * 70 / i64::from(self.capacity)) as i32 }
    }

    /// Rows of the 18-row flame that are lit (`burnRemaining * 18 / maxBurn`); none while nothing burns.
    pub fn flame_rows(&self) -> Option<i32> {
        (self.max_burn > 0).then(|| (i64::from(self.burn_remaining) * 18 / i64::from(self.max_burn)) as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_view_survives_the_menu_data() {
        let v = GeneratorView { energy: 1_000_000, capacity: 1_000_000, burn_remaining: 401, max_burn: 400, fe_per_tick: 60, redstone: 3 };
        let data = v.pack();
        assert_eq!(data.len(), GeneratorView::SLOTS);
        assert!(data.iter().all(|d| (0..=0x7FFF).contains(d)));
        assert_eq!(GeneratorView::unpack(&data), Some(v));
        assert_eq!(GeneratorView::unpack(&data[..5]), None);
        assert_eq!(v.energy_rows(), 70);
        assert_eq!(v.flame_rows(), Some(18));
        assert_eq!(GeneratorView::default().flame_rows(), None);
    }
}
