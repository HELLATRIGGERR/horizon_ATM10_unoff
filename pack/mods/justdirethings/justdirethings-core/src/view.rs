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

/// The Paradox Machine's screen: its buffer and tank, its paradox energy (hundredths), its settings (each radius in
/// half blocks, each offset plus 9) and what a run would restore and cost now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParadoxView {
    pub energy: i32,
    pub energy_capacity: i32,
    pub fluid: i32,
    pub fluid_capacity: i32,
    pub paradox_energy: i32,
    pub paradox_max: i32,
    pub redstone: i32,
    pub target: i32,
    pub render_paradox: i32,
    pub render_area: i32,
    pub radius: [i32; 3],
    pub offset: [i32; 3],
    pub tick_speed: i32,
    pub running: i32,
    pub time_running: i32,
    pub run_time: i32,
    pub blocks: i32,
    pub entities: i32,
    pub energy_cost: i32,
    pub fluid_cost: i32,
}

impl ParadoxView {
    pub const SLOTS: usize = 2 * 26;

    pub fn pack(&self) -> Vec<i32> {
        let mut o = Vec::with_capacity(Self::SLOTS);
        let values = [
            self.energy, self.energy_capacity, self.fluid, self.fluid_capacity, self.paradox_energy, self.paradox_max, self.redstone,
            self.target, self.render_paradox, self.render_area, self.radius[0], self.radius[1], self.radius[2], self.offset[0] + 9,
            self.offset[1] + 9, self.offset[2] + 9, self.tick_speed, self.running, self.time_running, self.run_time, self.blocks,
            self.entities, self.energy_cost, self.fluid_cost, 0, 0,
        ];
        for v in values {
            put(&mut o, v);
        }
        o
    }

    /// The view in `data`; none until all of the menu's data has come.
    pub fn unpack(d: &[i32]) -> Option<Self> {
        if d.len() < Self::SLOTS {
            return None;
        }
        let v = |i: usize| get(d, 2 * i);
        Some(Self {
            energy: v(0),
            energy_capacity: v(1),
            fluid: v(2),
            fluid_capacity: v(3),
            paradox_energy: v(4),
            paradox_max: v(5),
            redstone: v(6),
            target: v(7),
            render_paradox: v(8),
            render_area: v(9),
            radius: [v(10), v(11), v(12)],
            offset: [v(13) - 9, v(14) - 9, v(15) - 9],
            tick_speed: v(16),
            running: v(17),
            time_running: v(18),
            run_time: v(19),
            blocks: v(20),
            entities: v(21),
            energy_cost: v(22),
            fluid_cost: v(23),
        })
    }

    /// Rows of a 70-row bar lit for `amount` of `max`.
    pub fn rows(amount: i32, max: i32) -> i32 {
        if max <= 0 { 0 } else { (i64::from(amount) * 70 / i64::from(max)) as i32 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_paradox_view_survives_the_menu_data() {
        let v = ParadoxView {
            energy: 10_000_000,
            energy_capacity: 10_000_000,
            fluid: 16000,
            fluid_capacity: 16000,
            paradox_energy: 50,
            paradox_max: 10000,
            redstone: 3,
            target: 2,
            render_paradox: 1,
            render_area: 0,
            radius: [10, 0, 1],
            offset: [-9, 9, 0],
            tick_speed: 20,
            running: 1,
            time_running: 7,
            run_time: 20,
            blocks: 2,
            entities: 0,
            energy_cost: 500_000,
            fluid_cost: 100,
        };
        let data = v.pack();
        assert_eq!(data.len(), ParadoxView::SLOTS);
        assert!(data.iter().all(|d| (0..=0x7FFF).contains(d)));
        assert_eq!(ParadoxView::unpack(&data), Some(v));
        assert_eq!(ParadoxView::unpack(&data[..10]), None);
        assert_eq!(ParadoxView::rows(8000, 16000), 35);
    }

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
