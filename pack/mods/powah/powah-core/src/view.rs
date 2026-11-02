//! What a Furnator's screen shows, sent in a menu's data slots. A data slot
//! carries 16 signed bits, so each amount goes as four 15-bit pieces (60 bits,
//! more than any energy here), which never reads as negative.

/// The numbers the screen draws.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FurnatorView {
    pub stored: i64,
    pub capacity: i64,
    /// FE the burning fuel still gives.
    pub fuel_left: i64,
    /// FE the burning fuel gave in all.
    pub fuel_total: i64,
    /// FE/t while burning.
    pub generation: i64,
    /// FE/t it can push.
    pub transfer: i64,
    pub burning: bool,
}

const PIECES: usize = 4;
const AMOUNTS: usize = 6;
/// Data slots a view takes.
pub const SLOTS: usize = PIECES * AMOUNTS + 1;

fn put(out: &mut Vec<i32>, value: i64) {
    let value = value.max(0);
    for i in 0..PIECES {
        out.push(((value >> (15 * i)) & 0x7FFF) as i32);
    }
}

fn get(data: &[i32], at: usize) -> i64 {
    (0..PIECES).fold(0, |value, i| value | (i64::from(data[at + i] & 0x7FFF) << (15 * i)))
}

impl FurnatorView {
    pub fn pack(&self) -> Vec<i32> {
        let mut out = Vec::with_capacity(SLOTS);
        self.pack_into(&mut out);
        out
    }

    /// The data slots, after what `out` holds.
    pub fn pack_into(&self, out: &mut Vec<i32>) {
        for value in [self.stored, self.capacity, self.fuel_left, self.fuel_total, self.generation, self.transfer] {
            put(out, value);
        }
        out.push(i32::from(self.burning));
    }

    /// The view in `data`; an empty one until the menu's data has all come.
    pub fn unpack(data: &[i32]) -> FurnatorView {
        if data.len() < SLOTS {
            return FurnatorView::default();
        }
        FurnatorView {
            stored: get(data, 0),
            capacity: get(data, PIECES),
            fuel_left: get(data, PIECES * 2),
            fuel_total: get(data, PIECES * 3),
            generation: get(data, PIECES * 4),
            transfer: get(data, PIECES * 5),
            burning: data[PIECES * AMOUNTS] != 0,
        }
    }

    /// Rows of the energy gauge to fill: none when empty, at least one when not.
    pub fn energy_rows(&self, height: i64) -> i64 {
        if self.stored <= 0 || self.capacity <= 0 {
            return 0;
        }
        (self.stored.min(self.capacity) as i128 * i128::from(height) / i128::from(self.capacity)).max(1) as i64
    }

    /// Rows of the fuel bar to fill, as the burning fuel runs down.
    pub fn fuel_rows(&self, height: i64) -> i64 {
        if self.fuel_left <= 0 || self.fuel_total <= 0 {
            return 0;
        }
        (self.fuel_left.min(self.fuel_total) as i128 * i128::from(height) / i128::from(self.fuel_total)).max(1) as i64
    }
}

/// `1234567` as `1,234,567`, as Powah prints amounts.
pub fn commas(value: i64) -> String {
    let digits = value.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if value < 0 {
        out.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `240`, `1.6k`, `80k`, `2M`: one decimal when it has one, as Powah prints capacities and rates.
pub fn short(value: i64) -> String {
    const UNITS: [(i64, &str); 6] = [
        (1_000_000_000_000_000_000, "E"),
        (1_000_000_000_000_000, "P"),
        (1_000_000_000_000, "T"),
        (1_000_000_000, "B"),
        (1_000_000, "M"),
        (1_000, "k"),
    ];
    if value < 0 {
        return format!("-{}", short(value.saturating_neg()));
    }
    let Some(&(unit, suffix)) = UNITS.iter().find(|(unit, _)| value >= *unit) else {
        return value.to_string();
    };
    let tenths = value / (unit / 10);
    if tenths < 100 && tenths % 10 != 0 {
        format!("{}.{}{suffix}", tenths / 10, tenths % 10)
    } else {
        format!("{}{suffix}", tenths / 10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_amounts_print_as_powah_does() {
        for (value, text) in [
            (0, "0"),
            (999, "999"),
            (1_000, "1k"),
            (1_600, "1.6k"),
            (20_000, "20k"),
            (160_000, "160k"),
            (199_999, "199k"),
            (2_000_000, "2M"),
            (40_000_000, "40M"),
            (2_500_000_000, "2.5B"),
            (-1_600, "-1.6k"),
        ] {
            assert_eq!(short(value), text);
        }
    }

    #[test]
    fn a_view_survives_data_slots_and_never_reads_negative() {
        let view = FurnatorView {
            stored: 39_999_999,
            capacity: 40_000_000,
            fuel_left: 0x8000,
            fuel_total: 48_000,
            generation: 20_000,
            transfer: (1 << 59) + 7,
            burning: true,
        };
        let data = view.pack();
        assert_eq!(data.len(), SLOTS);
        // As the wire carries them: 16 signed bits, and all pieces are 15 bits.
        for value in &data {
            assert!((0..=0x7FFF).contains(value), "{value}");
        }
        assert_eq!(FurnatorView::unpack(&data), view);
    }

    #[test]
    fn an_unfinished_menu_reads_empty() {
        assert_eq!(FurnatorView::unpack(&[1, 2, 3]), FurnatorView::default());
        assert_eq!(FurnatorView::unpack(&[]), FurnatorView::default());
    }

    #[test]
    fn gauges_fill_by_rows() {
        let view = |stored, capacity| FurnatorView { stored, capacity, ..Default::default() };
        assert_eq!(view(0, 20_000).energy_rows(39), 0);
        assert_eq!(view(1, 20_000).energy_rows(39), 1, "a little shows");
        assert_eq!(view(10_000, 20_000).energy_rows(39), 19);
        assert_eq!(view(20_000, 20_000).energy_rows(39), 39);
        assert_eq!(view(5, 0).energy_rows(39), 0);
        // Amounts far past the int range don't overflow.
        assert_eq!(view(i64::MAX / 2, i64::MAX).energy_rows(39), 19);
        let fuel = |fuel_left, fuel_total| FurnatorView { fuel_left, fuel_total, ..Default::default() };
        assert_eq!(fuel(48_000, 48_000).fuel_rows(16), 16);
        assert_eq!(fuel(24_000, 48_000).fuel_rows(16), 8);
        assert_eq!(fuel(1, 48_000).fuel_rows(16), 1);
        assert_eq!(fuel(0, 48_000).fuel_rows(16), 0);
    }

    #[test]
    fn amounts_print_with_commas() {
        assert_eq!(commas(0), "0");
        assert_eq!(commas(999), "999");
        assert_eq!(commas(1000), "1,000");
        assert_eq!(commas(48_000), "48,000");
        assert_eq!(commas(40_000_000), "40,000,000");
        assert_eq!(commas(-1234), "-1,234");
    }
}
