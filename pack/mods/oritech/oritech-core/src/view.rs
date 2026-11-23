//! What a machine's screen shows, carried in the block entity's menu data. A data slot carries 16 signed bits, so an
//! amount goes as two 15-bit pieces (30 bits, more than any buffer here), which never reads as negative. The screen's
//! text and layout come from the machine's `Spec`; only the numbers travel.

/// The most a packed amount holds.
pub const MAX: i32 = (1 << 30) - 1;

fn put(out: &mut Vec<i32>, value: i64) {
    let v = value.clamp(0, i64::from(MAX)) as i32;
    out.push(v & 0x7FFF);
    out.push((v >> 15) & 0x7FFF);
}

fn get(data: &[i32], at: usize) -> i32 {
    (data[at] & 0x7FFF) | ((data[at + 1] & 0x7FFF) << 15)
}

fn small(out: &mut Vec<i32>, value: i32) {
    out.push(value.clamp(0, 0x7FFF));
}

/// A processing machine's screen (`OritechMachineScreen` with `UpgradableOritechScreen`'s numbers).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MachineView {
    /// `energyStorage.amount` and `capacity` (RF).
    pub energy: i64,
    pub capacity: i64,
    /// `maxInsert` (`getDisplayedEnergyTransfer`).
    pub insert: i64,
    /// `calculateEnergyUsage` rounded (`getDisplayedEnergyUsage`), RF a tick of work.
    pub usage: i64,
    /// `progress` and the ticks the current recipe takes (`time * speed`, rounded up); 0 for none.
    pub progress: i32,
    pub total: i32,
    /// `getSpeedMultiplier` and `getEfficiencyMultiplier` in percent (100 is none).
    pub speed: i32,
    pub efficiency: i32,
    /// `isActive`: a multiblock's cores are in place (always 1 for the pulverizer).
    pub active: bool,
    /// The cores still missing around an unassembled multiblock.
    pub missing: i32,
}

impl MachineView {
    pub const SLOTS: usize = 2 * 4 + 6;

    pub fn pack(&self) -> Vec<i32> {
        let mut o = Vec::with_capacity(Self::SLOTS);
        for v in [self.energy, self.capacity, self.insert, self.usage] {
            put(&mut o, v);
        }
        for v in [self.progress, self.total, self.speed, self.efficiency, i32::from(self.active), self.missing] {
            small(&mut o, v);
        }
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
            energy: i64::from(get(d, 0)),
            capacity: i64::from(get(d, 2)),
            insert: i64::from(get(d, 4)),
            usage: i64::from(get(d, 6)),
            progress: s(8),
            total: s(9),
            speed: s(10),
            efficiency: s(11),
            active: s(12) != 0,
            missing: s(13),
        })
    }

    /// `ProgressDisplayWidget`: the share of the arrow filled, `progress / (time * speed)` (none without a recipe).
    pub fn progress_share(&self) -> f32 {
        if self.total <= 0 { 0.0 } else { (self.progress as f32 / self.total as f32).clamp(0.0, 1.0) }
    }

    /// `EnergyDisplayWidget`: the share of the bar filled.
    pub fn energy_share(&self) -> f32 {
        if self.capacity <= 0 { 0.0 } else { (self.energy as f32 / self.capacity as f32).clamp(0.0, 1.0) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_view_survives_the_wire() {
        let v = MachineView {
            energy: 49_744,
            capacity: 50_000,
            insert: 1024,
            usage: 256,
            progress: 3,
            total: 80,
            speed: 100,
            efficiency: 100,
            active: true,
            missing: 0,
        };
        let wire: Vec<i32> = v.pack().into_iter().map(|x| i32::from(x as i16)).collect();
        assert_eq!(wire.len(), MachineView::SLOTS);
        assert_eq!(MachineView::unpack(&wire), Some(v.clone()));
        assert_eq!(MachineView::unpack(&wire[..5]), None);
        assert!((v.progress_share() - 3.0 / 80.0).abs() < 1e-6);
        assert_eq!(MachineView { total: 0, ..v }.progress_share(), 0.0);
    }

    #[test]
    fn amounts_are_clamped_not_wrapped() {
        let v = MachineView { energy: i64::MAX, capacity: -5, ..MachineView::default() };
        let back = MachineView::unpack(&v.pack()).unwrap();
        assert_eq!((back.energy, back.capacity), (i64::from(MAX), 0));
        assert_eq!(back.energy_share(), 0.0, "no capacity: an empty bar");
    }
}
