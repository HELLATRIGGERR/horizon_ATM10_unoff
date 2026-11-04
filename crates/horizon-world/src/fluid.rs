//! Fluid states carried by the pinned vanilla block-state registry.
//! Flow updates, spreading and entity interaction are separate simulation work.
use crate::{
    chunk::BlockState,
    compression::invalid,
    registry::{BlockRegistry, BlockStateId},
};
use std::io;

/// A fluid's registry id (`BuiltInRegistries.FLUID`): vanilla's five, then
/// the content mods' fluids ([`crate::mod_fluids`]), each a flowing and a
/// source fluid (`BaseFlowingFluid.Flowing` and `Source`) in that order, as
/// vanilla's water and lava are. Vanilla's keep their old names as constants,
/// so `FluidKind::Water` reads and matches as it did.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FluidKind(u16);
#[allow(non_upper_case_globals)]
impl FluidKind {
    pub const Empty: Self = Self(0);
    pub const FlowingWater: Self = Self(1);
    pub const Water: Self = Self(2);
    pub const FlowingLava: Self = Self(3);
    pub const Lava: Self = Self(4);
}
impl std::fmt::Debug for FluidKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            0 => f.write_str("Empty"),
            1 => f.write_str("FlowingWater"),
            2 => f.write_str("Water"),
            3 => f.write_str("FlowingLava"),
            4 => f.write_str("Lava"),
            _ => write!(f, "Mod({})", self.name()),
        }
    }
}
impl FluidKind {
    /// Vanilla's fluids, in registry order.
    pub const ALL: [Self; 5] = [
        Self::Empty,
        Self::FlowingWater,
        Self::Water,
        Self::FlowingLava,
        Self::Lava,
    ];
    /// The registry id: a tick type's, a saved tick's.
    pub const fn id(self) -> u16 {
        self.0
    }
    /// The fluid with registry id `id`: vanilla's, or a registered mod fluid's.
    pub fn from_id(id: u32) -> Option<Self> {
        let id = u16::try_from(id).ok()?;
        (usize::from(id) < Self::ALL.len() + 2 * crate::mod_fluids::mod_fluids().len())
            .then_some(Self(id))
    }
    /// Every registered fluid: vanilla's, then the mods'.
    pub fn all() -> impl Iterator<Item = Self> {
        let count = Self::ALL.len() + 2 * crate::mod_fluids::mod_fluids().len();
        (0..count as u16).map(Self)
    }
    pub fn name(self) -> &'static str {
        match self.0 {
            0 => "minecraft:empty",
            1 => "minecraft:flowing_water",
            2 => "minecraft:water",
            3 => "minecraft:flowing_lava",
            4 => "minecraft:lava",
            _ => match self.mod_fluid() {
                Some(m) if self.is_source_kind() => m.id.as_str(),
                Some(m) => m.flowing.as_str(),
                None => "minecraft:empty",
            },
        }
    }
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.name() == name)
            .or_else(|| crate::mod_fluids::kind_by_name(name))
    }
    /// `Fluid.isSame`: the source and flowing fluid of one family.
    pub const fn same_family(self, other: Self) -> bool {
        self.0.div_ceil(2) == other.0.div_ceil(2)
    }
    /// The bit of a vanilla fluid in a vanilla tag's mask; a mod fluid has
    /// none (its tags are its registry entry's, [`crate::mod_fluids::ModFluid`]).
    pub const fn mask(self) -> u8 {
        if self.0 < 5 { 1 << self.0 } else { 0 }
    }
    /// Whether it is a family's source fluid (`FlowingFluid.getSource()`).
    pub const fn is_source_kind(self) -> bool {
        self.0 != 0 && self.0 % 2 == 0
    }
    /// `FlowingFluid.getSource()` of its family (the empty fluid stays empty).
    pub const fn source(self) -> Self {
        Self(self.0.div_ceil(2) * 2)
    }
    /// `FlowingFluid.getFlowing()` of its family (the empty fluid stays empty).
    pub const fn flowing(self) -> Self {
        if self.0 == 0 {
            self
        } else {
            Self(self.0.div_ceil(2) * 2 - 1)
        }
    }
    /// Water or flowing water (by identity, not the `minecraft:water` tag).
    pub const fn is_water(self) -> bool {
        self.same_family(Self::Water)
    }
    /// Lava or flowing lava (by identity, not the `minecraft:lava` tag).
    pub const fn is_lava(self) -> bool {
        self.same_family(Self::Lava)
    }
    /// The content mod's fluid this is one of, None for vanilla's.
    pub fn mod_fluid(self) -> Option<&'static crate::mod_fluids::ModFluid> {
        let index = usize::from(self.0.checked_sub(5)?) / 2;
        crate::mod_fluids::mod_fluids().get(index)
    }
    /// The source and flowing fluid of the `index`th registered mod fluid.
    pub(crate) fn of_mod(index: usize) -> (Self, Self) {
        let flowing = Self(5 + 2 * index as u16);
        (Self(flowing.0 + 1), flowing)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FluidState {
    kind: FluidKind,
    amount: u8,
    falling: bool,
}
impl FluidState {
    /// Native fluid states require an object; properties decode leniently.
    pub fn from_json(json: &str) -> io::Result<Self> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| invalid(e.to_string()))?;
        let name = value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| invalid("missing fluid identifier"))?;
        let (namespace, path) = name.split_once(':').unwrap_or(("minecraft", name));
        let name = format!(
            "{}:{path}",
            if namespace.is_empty() {
                "minecraft"
            } else {
                namespace
            }
        );
        let kind =
            FluidKind::from_name(&name).ok_or_else(|| invalid("unknown fluid identifier"))?;
        if kind == FluidKind::Empty {
            return Ok(Self::EMPTY);
        }
        let flowing = !kind.is_source_kind();
        let mut state = Self {
            kind,
            amount: if flowing { 1 } else { 8 },
            falling: true,
        };
        if let Some(properties) = value
            .get("properties")
            .and_then(serde_json::Value::as_object)
        {
            if let Some(value) = properties
                .get("falling")
                .and_then(serde_json::Value::as_str)
            {
                match value {
                    "true" => state.falling = true,
                    "false" => state.falling = false,
                    _ => {}
                }
            }
            if flowing {
                if let Some(value) = properties
                    .get("level")
                    .and_then(serde_json::Value::as_str)
                    .and_then(crate::registry::java_integer)
                    .filter(|v| (1..=8).contains(v))
                {
                    state.amount = value as u8;
                }
            }
        }
        Ok(state)
    }
    pub fn to_json(self) -> io::Result<String> {
        let mut value = serde_json::json!({"id": self.kind.name()});
        if self.kind != FluidKind::Empty {
            let mut properties = serde_json::json!({"falling": self.falling.to_string()});
            if !self.is_source() {
                properties["level"] = self.amount.to_string().into();
            }
            value["properties"] = properties;
        }
        serde_json::to_string(&value).map_err(|e| invalid(e.to_string()))
    }
    /// A fluid state from its parts; the empty fluid ignores amount and falling.
    pub const fn new(kind: FluidKind, amount: u8, falling: bool) -> Self {
        if matches!(kind, FluidKind::Empty) {
            return Self::EMPTY;
        }
        Self {
            kind,
            amount,
            falling,
        }
    }
    pub const EMPTY: Self = Self {
        kind: FluidKind::Empty,
        amount: 0,
        falling: false,
    };
    pub const WATER: Self = Self {
        kind: FluidKind::Water,
        amount: 8,
        falling: false,
    };
    pub const fn kind(self) -> FluidKind {
        self.kind
    }
    pub const fn amount(self) -> u8 {
        self.amount
    }
    pub const fn falling(self) -> bool {
        self.falling
    }
    /// `Fluid.isSource`: a family's source fluid (vanilla's or a mod's).
    pub const fn is_source(self) -> bool {
        self.kind.is_source_kind()
    }
    pub const fn is_full(self) -> bool {
        self.amount == 8
    }
    pub fn height_at<W: FluidHeightWorld>(
        self,
        world: &mut W,
        position: [i32; 3],
    ) -> Result<f32, W::Error> {
        if self.kind == FluidKind::Empty {
            return Ok(0.0);
        }
        let above = [position[0], position[1].wrapping_add(1), position[2]];
        Ok(if self.kind.same_family(world.fluid(above)?.kind) {
            1.0
        } else {
            self.own_height()
        })
    }
    pub fn camera_height_at<W: FluidHeightWorld>(
        self,
        world: &mut W,
        position: [i32; 3],
    ) -> Result<f32, W::Error> {
        if self.is_source() {
            let above = [position[0], position[1].wrapping_add(1), position[2]];
            if world.sturdy_down(above)? {
                return Ok(1.0);
            }
        }
        self.height_at(world, position)
    }
    pub fn own_height(self) -> f32 {
        f32::from(self.amount) / 9.0
    }
    /// `LiquidBlock.getFluidState`: level 0 is the source, 1 to 7 flowing
    /// with 8 - level, 8 and up falling (`fluid` is any of the family).
    fn of_liquid_block(state: &BlockState, fluid: FluidKind) -> io::Result<Self> {
        let level = state
            .properties
            .get("level")
            .and_then(|v| v.parse::<u8>().ok())
            .filter(|v| *v <= 15)
            .ok_or_else(|| invalid("invalid liquid block level"))?;
        Ok(Self {
            kind: if level == 0 {
                fluid.source()
            } else {
                fluid.flowing()
            },
            amount: if level == 0 || level >= 8 {
                8
            } else {
                8 - level
            },
            falling: level >= 8,
        })
    }
    pub fn from_block(state: &BlockState) -> io::Result<Self> {
        match state.name.as_str() {
            "minecraft:water" => return Self::of_liquid_block(state, FluidKind::Water),
            "minecraft:lava" => return Self::of_liquid_block(state, FluidKind::Lava),
            _ => {}
        }
        // A content mod's `LiquidBlock` holds its fluid as water's holds water.
        if let Some(fluid) = crate::mod_fluids::kind_by_block(&state.name) {
            return Self::of_liquid_block(state, fluid);
        }
        if matches!(
            state.name.as_str(),
            "minecraft:kelp"
                | "minecraft:kelp_plant"
                | "minecraft:seagrass"
                | "minecraft:tall_seagrass"
                | "minecraft:bubble_column"
        ) {
            return Ok(Self::WATER);
        }
        match state.properties.get("waterlogged").map(String::as_str) {
            Some("true") => {
                // The pinned copper-grate blocks use falling source water.
                let falling = matches!(
                    state.name.as_str(),
                    "minecraft:copper_grate"
                        | "minecraft:exposed_copper_grate"
                        | "minecraft:weathered_copper_grate"
                        | "minecraft:oxidized_copper_grate"
                        | "minecraft:waxed_copper_grate"
                        | "minecraft:waxed_exposed_copper_grate"
                        | "minecraft:waxed_weathered_copper_grate"
                        | "minecraft:waxed_oxidized_copper_grate"
                );
                Ok(Self {
                    falling,
                    ..Self::WATER
                })
            }
            None | Some("false") => Ok(Self::EMPTY),
            _ => Err(invalid("invalid waterlogged block property")),
        }
    }
}
/// Queries required by fluid surface height and camera immersion. The caller
/// owns block shape/support behavior and world-query failures.
pub trait FluidHeightWorld {
    type Error;
    fn fluid(&mut self, position: [i32; 3]) -> Result<FluidState, Self::Error>;
    fn sturdy_down(&mut self, position: [i32; 3]) -> Result<bool, Self::Error>;
}
#[derive(Debug)]
pub struct FluidStateTable(Vec<FluidState>);
impl FluidStateTable {
    pub fn new(registry: &BlockRegistry) -> io::Result<Self> {
        (0..registry.len())
            .map(|i| FluidState::from_block(registry.state(BlockStateId(i as u32)).unwrap()))
            .collect::<io::Result<Vec<_>>>()
            .map(Self)
    }
    pub fn get(&self, state: BlockStateId) -> Option<FluidState> {
        self.0.get(state.0 as usize).copied()
    }
}

/// Bound identities for converting fluid states into their legacy liquid blocks.
pub struct FluidBlockBindings {
    air: BlockStateId,
    water: [BlockStateId; 16],
    lava: [BlockStateId; 16],
    /// Each registered mod fluid's liquid block by level, in registry order:
    /// air where its block is missing (`BaseFlowingFluid.createLegacyBlock`
    /// without a block).
    mods: Vec<[BlockStateId; 16]>,
}
impl FluidBlockBindings {
    pub fn new(registry: &BlockRegistry) -> io::Result<Self> {
        let levels = |name: &str| -> io::Result<[BlockStateId; 16]> {
            let default = registry
                .default_state(name)
                .ok_or_else(|| invalid("missing liquid block"))?;
            let mut state = registry.state(default).unwrap().clone();
            let mut result = [default; 16];
            for (level, id) in result.iter_mut().enumerate() {
                state.properties.insert("level".into(), level.to_string());
                *id = registry
                    .id(&state)
                    .ok_or_else(|| invalid("missing liquid block level"))?;
            }
            Ok(result)
        };
        let air = registry
            .default_state("minecraft:air")
            .ok_or_else(|| invalid("missing air block"))?;
        let mods = crate::mod_fluids::mod_fluids()
            .iter()
            .map(|m| levels(&m.block).unwrap_or([air; 16]))
            .collect();
        Ok(Self {
            air,
            water: levels("minecraft:water")?,
            lava: levels("minecraft:lava")?,
            mods,
        })
    }
    /// `FluidState.createLegacyBlock`.
    pub fn block(&self, state: FluidState) -> BlockStateId {
        if state.kind == FluidKind::Empty {
            return self.air;
        }
        let level = if state.is_source() {
            0
        } else {
            8 - state.amount.min(8) + if state.falling { 8 } else { 0 }
        } as usize;
        if state.kind.same_family(FluidKind::Water) {
            self.water[level]
        } else if state.kind.same_family(FluidKind::Lava) {
            self.lava[level]
        } else {
            let index = usize::from(state.kind.id() - 5) / 2;
            self.mods
                .get(index)
                .map_or(self.air, |levels| levels[level])
        }
    }
}
