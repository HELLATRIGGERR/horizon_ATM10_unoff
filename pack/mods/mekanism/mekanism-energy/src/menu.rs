//! The energy cube's menu: what its data slots carry to the screen, and its
//! buttons. Data slots are 16 bits on the wire, so a Joule amount takes four,
//! low bits first, each a `u16` (`as i16` on the wire, read back `as u16`).
//! Buttons are menu button ids (`clickMenuButton`) that name what they change,
//! so the server needs no state for which page a screen shows. The
//! machines' menus (`MachineView`) carry their progress, upgrades and tank
//! the same way.
use crate::cube::CubeState;
use crate::machine::{Upgrade, Upgrades};
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
    /// A machine's chemical sides (the infuser's tank).
    Chemicals,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    /// The screen's own pages (main, energy, item and chemical side
    /// configuration, a machine's upgrades): the server does nothing.
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
            Kind::Chemicals => 90,
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
        if (1..=5).contains(&id) {
            return Some(Self::Page((id - 1) as u8));
        }
        let (kind, rest) = match id {
            10..=32 => (Kind::Energy, id - 10),
            50..=72 => (Kind::Items, id - 50),
            90..=112 => (Kind::Chemicals, id - 90),
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

/// A machine's buttons: its side configuration, as a cube's, and the upgrade
/// window's uninstall (`GuiInteraction.REMOVE_UPGRADE`, `REMOVE_ALL_UPGRADE`)
/// and the infuser's dump (`GuiDumpButton`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MachineButton {
    Config(Button),
    Remove { upgrade: Upgrade, all: bool },
    Dump,
}

impl MachineButton {
    pub fn id(self) -> i32 {
        match self {
            Self::Config(b) => b.id(),
            Self::Remove { upgrade, all } => (if all { 130 } else { 120 }) + upgrade.ordinal(),
            Self::Dump => 140,
        }
    }

    pub fn from_id(id: i32) -> Option<Self> {
        match id {
            120..=126 => Some(Self::Remove { upgrade: Upgrade::by_id(id - 120), all: false }),
            130..=136 => Some(Self::Remove { upgrade: Upgrade::by_id(id - 130), all: true }),
            140 => Some(Self::Dump),
            _ => Button::from_id(id).map(Self::Config),
        }
    }
}

/// A machine's screen (`GuiElectricMachine`, `GuiMetallurgicInfuser`, the
/// side configuration and upgrade windows): slots 0..4 Joules stored, 4..8
/// capacity, 8..12 Joules a tick, 12..14 progress, 14..16 the ticks a recipe
/// takes, 16 active, 17..23 / 23..29 / 29..35 the energy, item and chemical
/// sides by `RelativeSide`, 35 items ejecting, 36..43 the upgrades installed by
/// ordinal, 43 the upgrade slot's install progress, 44 the infuse type
/// (`machine::infuse_number`), 45..49 its mB, 49..53 Joules taken in last
/// tick, 53 its warnings (`WARN_*`), 54 whether it has chemical sides.
pub const MACHINE_SLOTS: usize = 55;

/// Not enough energy for a tick (`RecipeError.NOT_ENOUGH_ENERGY`).
pub const WARN_ENERGY: i32 = 1;
/// No room for the output (`NOT_ENOUGH_OUTPUT_SPACE`).
pub const WARN_OUTPUT: i32 = 2;
/// The input fits no recipe with what else it holds (`NOT_ENOUGH_INPUT`).
pub const WARN_NO_RECIPE: i32 = 4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MachineView {
    pub stored: u64,
    pub capacity: u64,
    pub per_tick: u64,
    pub progress: u32,
    pub required: u32,
    pub active: bool,
    pub energy: [DataType; 6],
    pub items: [DataType; 6],
    pub chemicals: Option<[DataType; 6]>,
    pub items_eject: bool,
    pub upgrades: Upgrades,
    pub upgrade_ticks: u32,
    pub infuse: i32,
    pub infused: u64,
    pub input: u64,
    pub warnings: i32,
}

impl MachineView {
    pub fn pack(&self) -> Vec<i32> {
        let mut out = Vec::with_capacity(MACHINE_SLOTS);
        for value in [self.stored, self.capacity, self.per_tick] {
            push_bits(&mut out, value, 4);
        }
        push_bits(&mut out, u64::from(self.progress), 2);
        push_bits(&mut out, u64::from(self.required), 2);
        out.push(i32::from(self.active));
        let chemicals = self.chemicals.unwrap_or([DataType::None; 6]);
        for sides in [&self.energy, &self.items, &chemicals] {
            out.extend(sides.iter().map(|t| t.id()));
        }
        out.push(i32::from(self.items_eject));
        out.extend(self.upgrades.0.iter().map(|&n| n.min(0x7FFF) as i32));
        out.push(self.upgrade_ticks.min(0x7FFF) as i32);
        out.push(self.infuse);
        push_bits(&mut out, self.infused, 4);
        push_bits(&mut out, self.input, 4);
        out.push(self.warnings);
        out.push(i32::from(self.chemicals.is_some()));
        out
    }

    /// The view in `data`; none until the server's values have arrived.
    pub fn unpack(data: &[i32]) -> Option<Self> {
        if data.len() < MACHINE_SLOTS {
            return None;
        }
        let small = |at: usize| i32::from(data[at] as u16 as i16);
        let sides = |at: usize| std::array::from_fn(|i| DataType::by_id(small(at + i)));
        Some(Self {
            stored: read_bits(data, 0, 4),
            capacity: read_bits(data, 4, 4),
            per_tick: read_bits(data, 8, 4),
            progress: read_bits(data, 12, 2) as u32,
            required: read_bits(data, 14, 2) as u32,
            active: data[16] != 0,
            energy: sides(17),
            items: sides(23),
            chemicals: (data[54] != 0).then(|| sides(29)),
            items_eject: data[35] != 0,
            upgrades: Upgrades(std::array::from_fn(|i| small(36 + i).max(0) as u32)),
            upgrade_ticks: small(43).max(0) as u32,
            infuse: small(44),
            infused: read_bits(data, 45, 4),
            input: read_bits(data, 49, 4),
            warnings: small(53),
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
        let chemicals = Button::Side { kind: Kind::Chemicals, side: 2, forward: false };
        assert_eq!(Button::from_id(chemicals.id()), Some(chemicals));
        for b in [
            MachineButton::Remove { upgrade: Upgrade::Energy, all: true },
            MachineButton::Remove { upgrade: Upgrade::Speed, all: false },
            MachineButton::Dump,
            MachineButton::Config(Button::Page(4)),
        ] {
            assert_eq!(MachineButton::from_id(b.id()), Some(b), "{b:?}");
        }
    }

    #[test]
    fn a_machines_view_survives_the_wire() {
        let mut upgrades = Upgrades::default();
        upgrades.set(Upgrade::Speed, 8);
        upgrades.set(Upgrade::Muffling, 1);
        let view = MachineView {
            stored: 319_200,
            capacity: 320_000,
            per_tick: 12_800,
            progress: 11,
            required: 12,
            active: true,
            energy: [DataType::Input; 6],
            items: [DataType::Input, DataType::Input, DataType::Output, DataType::Energy, DataType::Input, DataType::Extra],
            chemicals: Some([DataType::Output; 6]),
            items_eject: true,
            upgrades,
            upgrade_ticks: 20,
            infuse: 255,
            infused: 1000,
            input: 9_999_999_999,
            warnings: WARN_OUTPUT | WARN_ENERGY,
        };
        let wire: Vec<i32> = view.pack().iter().map(|&v| i32::from(v as i16)).collect();
        assert_eq!(wire.len(), MACHINE_SLOTS);
        assert_eq!(MachineView::unpack(&wire), Some(view));
        let plain = MachineView { chemicals: None, ..view };
        assert_eq!(MachineView::unpack(&plain.pack()), Some(plain));
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
