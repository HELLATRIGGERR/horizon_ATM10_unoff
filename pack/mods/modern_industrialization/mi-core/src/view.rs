//! What a machine's screen shows (MI syncs it with its GUI components: `EnergyBar.Data`, the progress
//! float, `RecipeEfficiencyBar.Data`, the fluid slots), sent here in a menu's data slots. A data slot
//! carries 16 signed bits, so each number goes as two 15-bit pieces (30 bits), which never read as negative.

const PIECES: usize = 2;
/// `progress` is sent in parts of this.
pub const PROGRESS_SCALE: i64 = 10_000;

/// A fluid slot as the screen draws it: fluid ids are the game's numeric ones, none as -1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FluidView {
    pub fluid: i64,
    pub amount: i64,
    pub capacity: i64,
    pub locked: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MachineView {
    pub eu: i64,
    pub eu_capacity: i64,
    /// `getProgress` in parts of `PROGRESS_SCALE`.
    pub progress: i64,
    pub efficiency_ticks: i64,
    pub max_efficiency_ticks: i64,
    pub has_active_recipe: bool,
    /// `getCurrentRecipeEu` (`recipeMaxEu`) and the recipe's own `eu`.
    pub current_recipe_eu: i64,
    pub base_recipe_eu: i64,
    /// The tier's `getMaxRecipeEu`.
    pub max_recipe_eu: i64,
    pub matches_multiple_recipes: bool,
    /// Item output slots reserved for the running recipe, by bit.
    pub machine_locked: i64,
    pub fluids: Vec<FluidView>,
}

fn put(out: &mut Vec<i32>, value: i64) {
    let value = value.clamp(-1, (1 << 30) - 2) + 1;
    for i in 0..PIECES {
        out.push(((value >> (15 * i)) & 0x7FFF) as i32);
    }
}

fn get(data: &[i32], at: usize) -> i64 {
    (0..PIECES).fold(0, |value, i| value | (i64::from(data.get(at + i).copied().unwrap_or(0) & 0x7FFF) << (15 * i))) - 1
}

const FIXED: usize = 12;

impl MachineView {
    pub fn pack_into(&self, out: &mut Vec<i32>) {
        for v in [
            self.eu,
            self.eu_capacity,
            self.progress,
            self.efficiency_ticks,
            self.max_efficiency_ticks,
            i64::from(self.has_active_recipe),
            self.current_recipe_eu,
            self.base_recipe_eu,
            self.max_recipe_eu,
            i64::from(self.matches_multiple_recipes),
            self.machine_locked,
            self.fluids.len() as i64,
        ] {
            put(out, v);
        }
        for f in &self.fluids {
            for v in [f.fluid, f.amount, f.capacity, f.locked] {
                put(out, v);
            }
        }
    }

    /// The view in `data`; an empty one until the menu's data has all come.
    pub fn unpack(data: &[i32]) -> MachineView {
        if data.len() < FIXED * PIECES {
            return MachineView::default();
        }
        let n = |i: usize| get(data, i * PIECES);
        let fluids = (0..n(11).clamp(0, 16) as usize)
            .map(|k| {
                let at = FIXED + 4 * k;
                FluidView { fluid: n(at), amount: n(at + 1), capacity: n(at + 2), locked: n(at + 3) }
            })
            .collect();
        MachineView {
            eu: n(0),
            eu_capacity: n(1),
            progress: n(2),
            efficiency_ticks: n(3),
            max_efficiency_ticks: n(4),
            has_active_recipe: n(5) != 0,
            current_recipe_eu: n(6),
            base_recipe_eu: n(7),
            max_recipe_eu: n(8),
            matches_multiple_recipes: n(9) != 0,
            machine_locked: n(10),
            fluids,
        }
    }

    pub fn progress(&self) -> f32 {
        self.progress as f32 / PROGRESS_SCALE as f32
    }
}

/// `TextHelper.getMaxedAmount`'s units: the amount and its maximum in k, M... by the maximum's size.
pub fn maxed_amount(amount: i64, max: i64) -> (String, String, &'static str) {
    let (div, unit) = if max >= 1_000_000_000 {
        (1e9, "G")
    } else if max >= 1_000_000 {
        (1e6, "M")
    } else if max >= 10_000 {
        (1e3, "k")
    } else {
        (1.0, "")
    };
    let fmt = |v: i64| {
        if div == 1.0 { v.to_string() } else { format!("{:.1}", v as f64 / div) }
    };
    (fmt(amount), fmt(max), unit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_16_bit_slots() {
        let v = MachineView {
            eu: 3200,
            eu_capacity: 3200,
            progress: 3200,
            efficiency_ticks: 1,
            max_efficiency_ticks: 72,
            has_active_recipe: true,
            current_recipe_eu: 8,
            base_recipe_eu: 2,
            max_recipe_eu: 32,
            matches_multiple_recipes: false,
            machine_locked: 0b1001,
            fluids: vec![FluidView { fluid: 40_000, amount: 16_000, capacity: 16_000, locked: -1 }],
        };
        let mut out = Vec::new();
        v.pack_into(&mut out);
        assert!(out.iter().all(|d| (0..=0x7FFF).contains(d)));
        assert_eq!(MachineView::unpack(&out), v);
        assert_eq!(MachineView::unpack(&out[..3]), MachineView::default());
        assert!((v.progress() - 0.32).abs() < 1e-6);
    }

    #[test]
    fn maxed_units() {
        assert_eq!(maxed_amount(3200, 3200), ("3200".into(), "3200".into(), ""));
        assert_eq!(maxed_amount(12_345, 51_200), ("12.3".into(), "51.2".into(), "k"));
    }
}
