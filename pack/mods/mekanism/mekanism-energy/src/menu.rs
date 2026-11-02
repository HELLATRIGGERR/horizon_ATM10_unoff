//! The energy cube's menu: what its data slots carry to the screen, and its
//! buttons. Data slots are 16 bits on the wire, so a Joule amount takes four,
//! low bits first, each a `u16` (`as i16` on the wire, read back `as u16`).
//! Buttons are menu button ids (`clickMenuButton`) that name what they change,
//! so the server needs no state for which page a screen shows.
use crate::cube::CubeState;
use crate::sides::DataType;

/// Slots 0..4: Joules stored; 4..8: capacity; 8..12: Joules received in the
/// last tick (`getInputRate`); 12..16: the tier's output a tick; 16..22: the
/// energy side config by `RelativeSide`; 22: energy ejecting; 23..29: the item
/// side config; 29: items ejecting (always off: a cube can't eject items).
pub const SLOTS: usize = 30;

/// What the screen shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct View {
    pub stored: u64,
    pub capacity: u64,
    pub input: u64,
    pub output: u64,
    pub energy: [DataType; 6],
    pub eject: bool,
    pub items: [DataType; 6],
    pub items_eject: bool,
}

impl View {
    pub fn of(cube: &CubeState, capacity: u64, output: u64, input: u64) -> Self {
        Self {
            stored: cube.stored,
            capacity,
            input,
            output,
            energy: cube.energy,
            eject: cube.eject,
            items: cube.items,
            items_eject: cube.items_eject,
        }
    }

    pub fn pack(&self) -> Vec<i32> {
        let mut out = Vec::with_capacity(SLOTS);
        for value in [self.stored, self.capacity, self.input, self.output] {
            out.extend((0..4).map(|i| ((value >> (16 * i)) & 0xFFFF) as i32));
        }
        out.extend(self.energy.iter().map(|t| t.id()));
        out.push(i32::from(self.eject));
        out.extend(self.items.iter().map(|t| t.id()));
        out.push(i32::from(self.items_eject));
        out
    }

    /// The view in `data`; none until the server's values have arrived.
    pub fn unpack(data: &[i32]) -> Option<Self> {
        if data.len() < SLOTS {
            return None;
        }
        let long = |at: usize| (0..4).fold(0u64, |v, i| v | (u64::from(data[at + i] as u16) << (16 * i)));
        let sides = |at: usize| std::array::from_fn(|i| DataType::by_id(i32::from(data[at + i] as u16 as i16)));
        Some(Self {
            stored: long(0),
            capacity: long(4),
            input: long(8),
            output: long(12),
            energy: sides(16),
            eject: data[22] != 0,
            items: sides(23),
            items_eject: data[29] != 0,
        })
    }
}

/// What a side configuration button changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Energy,
    Items,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    /// The screen's own pages (main, energy and item side configuration):
    /// the server does nothing.
    Page(u8),
    /// A side's next (left click) or previous (right click) type
    /// (`PacketSideData`); `side` is a `RelativeSide` ordinal.
    Side { kind: Kind, side: usize, forward: bool },
    /// The auto-eject button (`PacketEjectConfiguration`).
    Eject(Kind),
    /// Clear sides (`PacketBatchConfiguration`): every side to the next (or
    /// previous) type of the type they all share, else to `NONE`.
    Clear { kind: Kind, forward: bool },
}

impl Button {
    fn base(kind: Kind) -> i32 {
        match kind {
            Kind::Energy => 10,
            Kind::Items => 50,
        }
    }

    pub fn id(self) -> i32 {
        match self {
            Self::Page(page) => 1 + i32::from(page),
            Self::Side { kind, side, forward } => Self::base(kind) + if forward { 0 } else { 10 } + side as i32,
            Self::Eject(kind) => Self::base(kind) + 20,
            Self::Clear { kind, forward } => Self::base(kind) + if forward { 21 } else { 22 },
        }
    }

    pub fn from_id(id: i32) -> Option<Self> {
        if (1..=3).contains(&id) {
            return Some(Self::Page((id - 1) as u8));
        }
        let (kind, rest) = match id {
            10..=32 => (Kind::Energy, id - 10),
            50..=72 => (Kind::Items, id - 50),
            _ => return None,
        };
        Some(match rest {
            0..=5 => Self::Side { kind, side: rest as usize, forward: true },
            10..=15 => Self::Side { kind, side: (rest - 10) as usize, forward: false },
            20 => Self::Eject(kind),
            21 => Self::Clear { kind, forward: true },
            22 => Self::Clear { kind, forward: false },
            _ => return None,
        })
    }
}

/// A generator's screen (`GuiSolarGenerator`, `GuiHeatGenerator`): slots 0..4
/// Joules stored, 4..8 capacity, 8..12 produced last tick, 12..16 max output,
/// 16 whether it sees the sun, 17..19 lava (mB), 19..21 the tank, 21..25 the
/// temperature (an `f64`'s bits), 25..29 heat to neighbours, 29..33 heat to
/// the air (degrees a tick).
pub const GENERATOR_SLOTS: usize = 33;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GeneratorView {
    pub stored: u64,
    pub capacity: u64,
    pub produced: u64,
    pub max_output: u64,
    pub sees_sun: bool,
    pub lava: u32,
    pub tank: u32,
    pub temperature: f64,
    pub transfer_loss: f64,
    pub environment_loss: f64,
}

fn push_bits(out: &mut Vec<i32>, value: u64, pieces: usize) {
    out.extend((0..pieces).map(|i| ((value >> (16 * i)) & 0xFFFF) as i32));
}

fn read_bits(data: &[i32], at: usize, pieces: usize) -> u64 {
    (0..pieces).fold(0u64, |v, i| v | (u64::from(data[at + i] as u16) << (16 * i)))
}

impl GeneratorView {
    pub fn pack(&self) -> Vec<i32> {
        let mut out = Vec::with_capacity(GENERATOR_SLOTS);
        for value in [self.stored, self.capacity, self.produced, self.max_output] {
            push_bits(&mut out, value, 4);
        }
        out.push(i32::from(self.sees_sun));
        push_bits(&mut out, u64::from(self.lava), 2);
        push_bits(&mut out, u64::from(self.tank), 2);
        for value in [self.temperature, self.transfer_loss, self.environment_loss] {
            push_bits(&mut out, value.to_bits(), 4);
        }
        out
    }

    /// The view in `data`; none until the server's values have arrived.
    pub fn unpack(data: &[i32]) -> Option<Self> {
        if data.len() < GENERATOR_SLOTS {
            return None;
        }
        Some(Self {
            stored: read_bits(data, 0, 4),
            capacity: read_bits(data, 4, 4),
            produced: read_bits(data, 8, 4),
            max_output: read_bits(data, 12, 4),
            sees_sun: data[16] != 0,
            lava: read_bits(data, 17, 2) as u32,
            tank: read_bits(data, 19, 2) as u32,
            temperature: f64::from_bits(read_bits(data, 21, 4)),
            transfer_loss: f64::from_bits(read_bits(data, 25, 4)),
            environment_loss: f64::from_bits(read_bits(data, 29, 4)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tier::CubeTier;

    #[test]
    fn data_slots_carry_longs_in_sixteen_bit_pieces() {
        let mut cube = CubeState::new(CubeTier::Creative, i64::MAX as u64);
        cube.energy[3] = DataType::InputOutput;
        let view = View::of(&cube, i64::MAX as u64, i64::MAX as u64, 4_000);
        let data = view.pack();
        assert_eq!(data.len(), SLOTS);
        // What the client gets: each slot as a Java short.
        let wire: Vec<i32> = data.iter().map(|&v| i32::from(v as i16)).collect();
        assert_eq!(View::unpack(&wire), Some(view));
        assert_eq!(View::unpack(&data), Some(view));
        assert_eq!(View::unpack(&wire[..10]), None);
    }

    #[test]
    fn button_ids_name_what_they_change() {
        let all = [
            Button::Page(0),
            Button::Page(2),
            Button::Side { kind: Kind::Energy, side: 5, forward: false },
            Button::Side { kind: Kind::Items, side: 0, forward: true },
            Button::Eject(Kind::Energy),
            Button::Clear { kind: Kind::Items, forward: false },
        ];
        for b in all {
            assert_eq!(Button::from_id(b.id()), Some(b), "{b:?}");
        }
        assert_eq!(Button::Side { kind: Kind::Energy, side: 0, forward: true }.id(), 10);
        assert_eq!(Button::Eject(Kind::Energy).id(), 30);
        assert_eq!(Button::from_id(0), None);
        assert_eq!(Button::from_id(41), None);
    }

    #[test]
    fn a_generators_view_survives_the_wire() {
        let view = GeneratorView {
            stored: 96_000,
            capacity: 96_000,
            produced: 132,
            max_output: 264,
            sees_sun: true,
            lava: 23_990,
            tank: 24_000,
            temperature: 1881.270_5,
            transfer_loss: 0.0,
            environment_loss: 0.937,
        };
        let wire: Vec<i32> = view.pack().iter().map(|&v| i32::from(v as i16)).collect();
        assert_eq!(wire.len(), GENERATOR_SLOTS);
        assert_eq!(GeneratorView::unpack(&wire), Some(view));
        assert_eq!(GeneratorView::unpack(&wire[..20]), None);
    }
}
