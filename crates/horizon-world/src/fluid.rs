//! Fluid states carried by the pinned vanilla block-state registry.
//! Flow updates, spreading and entity interaction are separate simulation work.
use crate::{
    chunk::BlockState,
    compression::invalid,
    registry::{BlockRegistry, BlockStateId},
};
use std::{
    io,
    path::Path,
    sync::{OnceLock, RwLock},
};

/// A fluid: its index in the fluid table, which is its `minecraft:fluid`
/// network id. Vanilla's five come first, in their registry order; the
/// constants keep the names the enum had, so code matches on them as before.
/// The mods' fluids follow, each its source and then its flowing form: the
/// carried ones ([`crate::mod_fluids`], NeoForge's `FluidType` and
/// `BaseFlowingFluid` with all their numbers) when the process registered
/// them, else a content root's [`MOD_FLUIDS_FILE`]. A full modpack has
/// hundreds of fluids, so the index is 16 bits.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FluidKind(u16);

/// The vanilla fluid a fluid behaves as: how it flows, what it does to
/// mobs and how it meets other fluids. A mod fluid copies water or lava
/// (#6301), as a content block copies a template block. A carried fluid
/// ([`crate::mod_fluids`]) copies neither: it is `Empty` here, and its own
/// `FluidType` ([`FluidKind::mod_fluid`]) says what it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FluidTemplate {
    Empty,
    Water,
    Lava,
}

/// Numbers a fluid gives instead of its template's (NeoForge's
/// `BaseFlowingFluid.Properties`). Vanilla's give none: water and lava keep
/// their own rules, lava's faster ones in an ultrawarm dimension included.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FluidNumbers {
    /// `getTickDelay`: ticks between spreads.
    pub tick_delay: Option<i32>,
    /// `getDropOff`: levels lost per block.
    pub drop_off: Option<i32>,
    /// `getSlopeFindDistance`: how far a spread looks for a way down.
    pub slope_find_distance: Option<i32>,
    /// `canConvertToSource`: two sources make a third.
    pub convert_to_source: Option<bool>,
}

/// One row of the fluid table.
#[derive(Clone, Copy, Debug)]
pub struct FluidInfo {
    pub name: &'static str,
    pub template: FluidTemplate,
    /// Its family's source and flowing forms (the empty fluid is both).
    pub source: FluidKind,
    pub flowing: FluidKind,
    pub numbers: FluidNumbers,
    /// A mod fluid's liquid block (a copy of its template's, `level` 0 to 15);
    /// vanilla's are `minecraft:water` and `minecraft:lava`.
    pub block: Option<&'static str>,
    /// A mod fluid's bucket item; vanilla's are `minecraft:water_bucket` and
    /// `minecraft:lava_bucket`.
    pub bucket: Option<&'static str>,
}

/// The content root's file that lists its mod fluids, in network-id order
/// (`mod_content` writes it from the mods' `data/<id>/horizon/fluid/`).
pub const MOD_FLUIDS_FILE: &str = "horizon-mod-fluids.json";

/// A mod fluid as [`MOD_FLUIDS_FILE`] lists it: its source and flowing ids,
/// template, numbers (none: the template's) and liquid block.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ModFluid {
    pub id: String,
    pub flowing: String,
    pub template: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tick_delay: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drop_off: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slope_find_distance: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub convert_to_source: Option<bool>,
}

/// The mod fluids' rows, after vanilla's: replaced whenever a content root
/// loads ([`load_mod_fluids`]), read by every [`FluidKind`] past vanilla's.
static MOD_ROWS: RwLock<&'static [FluidInfo]> = RwLock::new(&[]);

/// Makes `fluids` the fluid table's rows after vanilla's: each one's source,
/// then its flowing form, in order. The rows are leaked: a process loads a
/// handful of content roots, and a `FluidKind` reads its row without a copy.
pub fn set_mod_fluids(fluids: &[ModFluid]) -> io::Result<()> {
    let base = VANILLA.len();
    if base + 2 * fluids.len() > usize::from(u16::MAX) + 1 {
        return Err(invalid("too many mod fluids"));
    }
    let leak = |s: &str| -> &'static str { Box::leak(s.to_owned().into_boxed_str()) };
    let mut rows = Vec::with_capacity(2 * fluids.len());
    for (i, f) in fluids.iter().enumerate() {
        let template = match f.template.as_str() {
            "minecraft:water" => FluidTemplate::Water,
            "minecraft:lava" => FluidTemplate::Lava,
            other => return Err(invalid(format!("fluid {}: template {other}", f.id))),
        };
        let source = FluidKind((base + 2 * i) as u16);
        let flowing = FluidKind((base + 2 * i + 1) as u16);
        let numbers = FluidNumbers {
            tick_delay: f.tick_delay,
            drop_off: f.drop_off,
            slope_find_distance: f.slope_find_distance,
            convert_to_source: f.convert_to_source,
        };
        let block = f.block.as_deref().map(leak);
        let bucket = f.bucket.as_deref().map(leak);
        for name in [&f.id, &f.flowing] {
            rows.push(FluidInfo {
                name: leak(name),
                template,
                source,
                flowing,
                numbers,
                block,
                bucket,
            });
        }
    }
    *MOD_ROWS.write().unwrap_or_else(|e| e.into_inner()) = Box::leak(rows.into_boxed_slice());
    Ok(())
}

/// Reads `root`'s [`MOD_FLUIDS_FILE`] into the fluid table; a root without
/// one (no mod declares a fluid) leaves vanilla's alone. A process that
/// registered the carried fluids ([`crate::mod_fluids`]) keeps those: they
/// come before a root's rows.
pub fn load_mod_fluids(root: &Path) -> io::Result<()> {
    let fluids: Vec<ModFluid> = match std::fs::read(root.join(MOD_FLUIDS_FILE)) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| invalid(e.to_string()))?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };
    set_mod_fluids(&fluids)
}

/// The carried fluids' rows ([`crate::mod_fluids::register_fluids`], once per
/// process), in their registry order: each source, then its flowing form.
static CARRIED_ROWS: OnceLock<&'static [FluidInfo]> = OnceLock::new();

#[cfg(test)]
thread_local! {
    /// A test of the content root's table ([`set_mod_fluids`]) reads it even
    /// where another test of the process registered the carried fluids.
    static ROOT_TABLE_ONLY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The carried fluids' rows, when the process registered any: they are the
/// table's for good, as the mods' fluids are in a NeoForge game.
fn carried_rows() -> Option<&'static [FluidInfo]> {
    #[cfg(test)]
    if ROOT_TABLE_ONLY.with(std::cell::Cell::get) {
        return None;
    }
    let fluids = crate::mod_fluids::mod_fluids();
    if fluids.is_empty() {
        return None;
    }
    Some(CARRIED_ROWS.get_or_init(|| {
        let leak = |s: &str| -> &'static str { Box::leak(s.to_owned().into_boxed_str()) };
        let mut rows = Vec::with_capacity(2 * fluids.len());
        for (i, m) in fluids.iter().enumerate() {
            let (source, flowing) = FluidKind::of_mod(i);
            let numbers = FluidNumbers {
                tick_delay: Some(m.tick_rate),
                drop_off: Some(m.level_decrease_per_block),
                slope_find_distance: Some(m.slope_find_distance),
                convert_to_source: Some(m.can_convert_to_source),
            };
            let block = (m.block != "minecraft:air").then(|| leak(&m.block));
            let bucket = m.bucket.as_deref().map(leak);
            // A fluid that is no `FlowingFluid` keeps its flowing row, unnamed.
            for name in [&m.id, &m.flowing] {
                rows.push(FluidInfo {
                    name: leak(name),
                    template: FluidTemplate::Empty,
                    source,
                    flowing,
                    numbers,
                    block,
                    bucket,
                });
            }
        }
        Box::leak(rows.into_boxed_slice())
    }))
}

fn mod_rows() -> &'static [FluidInfo] {
    if let Some(rows) = carried_rows() {
        return rows;
    }
    *MOD_ROWS.read().unwrap_or_else(|e| e.into_inner())
}

/// Vanilla's fluids, in `minecraft:fluid` order.
const VANILLA: [FluidInfo; 5] = {
    const fn row(
        name: &'static str,
        template: FluidTemplate,
        source: FluidKind,
        flowing: FluidKind,
    ) -> FluidInfo {
        let numbers = FluidNumbers {
            tick_delay: None,
            drop_off: None,
            slope_find_distance: None,
            convert_to_source: None,
        };
        FluidInfo {
            name,
            template,
            source,
            flowing,
            numbers,
            block: None,
            bucket: None,
        }
    }
    let (empty, flowing_water, water, flowing_lava, lava) = (
        FluidKind(0),
        FluidKind(1),
        FluidKind(2),
        FluidKind(3),
        FluidKind(4),
    );
    [
        row("minecraft:empty", FluidTemplate::Empty, empty, empty),
        row(
            "minecraft:flowing_water",
            FluidTemplate::Water,
            water,
            flowing_water,
        ),
        row(
            "minecraft:water",
            FluidTemplate::Water,
            water,
            flowing_water,
        ),
        row(
            "minecraft:flowing_lava",
            FluidTemplate::Lava,
            lava,
            flowing_lava,
        ),
        row("minecraft:lava", FluidTemplate::Lava, lava, flowing_lava),
    ]
};

#[allow(non_upper_case_globals)]
impl FluidKind {
    pub const Empty: Self = Self(0);
    pub const FlowingWater: Self = Self(1);
    pub const Water: Self = Self(2);
    pub const FlowingLava: Self = Self(3);
    pub const Lava: Self = Self(4);
    /// Vanilla's fluids.
    pub const ALL: [Self; 5] = [
        Self::Empty,
        Self::FlowingWater,
        Self::Water,
        Self::FlowingLava,
        Self::Lava,
    ];
    /// Its row in the fluid table, and its network id.
    pub const fn index(self) -> usize {
        self.0 as usize
    }
    /// The fluid at network id `index`: vanilla's or a loaded mod's.
    pub fn from_index(index: usize) -> Option<Self> {
        (index < VANILLA.len() + mod_rows().len()).then_some(Self(index as u16))
    }
    /// Every fluid of the table: vanilla's, then the loaded mods'.
    pub fn all() -> impl Iterator<Item = Self> {
        (0..VANILLA.len() + mod_rows().len()).map(|i| Self(i as u16))
    }
    /// Its row: an unknown index (no content root loaded it) reads as empty.
    pub fn info(self) -> &'static FluidInfo {
        let i = self.index();
        VANILLA
            .get(i)
            .or_else(|| mod_rows().get(i - VANILLA.len()))
            .unwrap_or(&VANILLA[0])
    }
    pub fn name(self) -> &'static str {
        self.info().name
    }
    /// The fluid named `name`: vanilla's, or a mod's source or flowing form.
    /// The carried fluids are looked up by name, not scanned: a modpack has
    /// hundreds.
    pub fn from_name(name: &str) -> Option<Self> {
        if let Some(kind) = Self::ALL.into_iter().find(|kind| kind.name() == name) {
            return Some(kind);
        }
        if carried_rows().is_some() {
            return crate::mod_fluids::kind_by_name(name);
        }
        mod_rows()
            .iter()
            .position(|row| row.name == name)
            .map(|i| Self((VANILLA.len() + i) as u16))
    }
    /// The mod fluid (its source form) whose bucket is `item`.
    pub fn of_bucket(item: &str) -> Option<Self> {
        if carried_rows().is_some() {
            return crate::mod_fluids::kind_by_bucket(item);
        }
        mod_rows()
            .iter()
            .find(|row| row.bucket == Some(item))
            .map(|row| row.source)
    }
    /// The mod fluid (its source form) whose liquid block is `block`.
    pub fn of_liquid_block(block: &str) -> Option<Self> {
        if carried_rows().is_some() {
            return crate::mod_fluids::kind_by_block(block);
        }
        mod_rows()
            .iter()
            .find(|row| row.block == Some(block))
            .map(|row| row.source)
    }
    /// One of vanilla's five: only those meet other fluids as vanilla's do
    /// (lava and water make stone); a mod fluid copies its template's flowing,
    /// not its interactions (#6301 step 8 adds those).
    pub const fn is_vanilla(self) -> bool {
        (self.0 as usize) < VANILLA.len()
    }
    pub fn template(self) -> FluidTemplate {
        self.info().template
    }
    /// The source form of its family (`minecraft:water` for flowing water).
    pub fn source(self) -> Self {
        self.info().source
    }
    /// The flowing form of its family.
    pub fn flowing(self) -> Self {
        self.info().flowing
    }
    pub fn numbers(self) -> FluidNumbers {
        self.info().numbers
    }
    /// `Fluid.isSame`: the same fluid, flowing or not.
    pub fn same_family(self, other: Self) -> bool {
        self.source().0 == other.source().0
    }
    /// Its bit in a fluid tag's mask. A carried fluid has none: its tags are
    /// its registry entry's ([`crate::mod_fluids::ModFluid`]), so a modpack's
    /// fluid in a vanilla tag (ATM10's `minecraft:water` names the Bumblezone's
    /// sugar water) doesn't turn into water.
    pub fn mask(self) -> FluidMask {
        // Past the mask's width a fluid is in no tag (a world with more mod
        // fluids than that has them flow, but tags can't name the rest).
        if (self.0 as u32) < FluidMask::BITS && self.mod_fluid().is_none() {
            1 << self.0
        } else {
            0
        }
    }
    /// The registry id ([`FluidKind::index`]): a tick type's, a saved tick's.
    pub const fn id(self) -> u16 {
        self.0
    }
    /// The fluid with registry id `id`: vanilla's, or a mod fluid's.
    pub fn from_id(id: u32) -> Option<Self> {
        Self::from_index(usize::try_from(id).ok()?)
    }
    /// Whether it is a family's source fluid (`FlowingFluid.getSource()`).
    pub fn is_source_kind(self) -> bool {
        self != Self::Empty && self.source() == self
    }
    /// Water or flowing water (by identity, not the `minecraft:water` tag).
    pub const fn is_water(self) -> bool {
        matches!(self.0, 1 | 2)
    }
    /// Lava or flowing lava (by identity, not the `minecraft:lava` tag).
    pub const fn is_lava(self) -> bool {
        matches!(self.0, 3 | 4)
    }
    /// The carried fluid this is one of (its `FluidType` and numbers), None
    /// for vanilla's and a content root's template fluids.
    pub fn mod_fluid(self) -> Option<&'static crate::mod_fluids::ModFluid> {
        let index = usize::from(self.0.checked_sub(VANILLA.len() as u16)?) / 2;
        carried_rows()?;
        crate::mod_fluids::mod_fluids().get(index)
    }
    /// The source and flowing fluid of the `index`th carried fluid.
    pub(crate) fn of_mod(index: usize) -> (Self, Self) {
        let source = Self((VANILLA.len() + 2 * index) as u16);
        (source, Self(source.0 + 1))
    }
}

/// A set of fluids, one bit per [`FluidKind`] (fluid tags): vanilla's five and
/// 61 mod fluids with their flowing forms.
pub type FluidMask = u128;

impl std::fmt::Debug for FluidKind {
    /// The enum's variant names, which logs and probes print.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const NAMES: [&str; 5] = ["Empty", "FlowingWater", "Water", "FlowingLava", "Lava"];
        match NAMES.get(self.index()) {
            Some(name) => f.write_str(name),
            None => write!(f, "Mod({})", self.name()),
        }
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
        let flowing = kind == kind.flowing();
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
            if flowing
                && let Some(value) = properties
                    .get("level")
                    .and_then(serde_json::Value::as_str)
                    .and_then(crate::registry::java_integer)
                    .filter(|v| (1..=8).contains(v))
            {
                state.amount = value as u8;
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
    pub fn is_source(self) -> bool {
        self.kind.0 != FluidKind::Empty.0 && self.kind.0 == self.kind.source().0
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
    pub fn from_block(state: &BlockState) -> io::Result<Self> {
        let level = state.properties.get("level");
        // A mod's liquid block (a copy of water's or lava's, with `level`) holds
        // its own fluid. Its lookup (a lock and a scan) comes last: this runs
        // for every block an entity's fluid checks touch.
        let source = match state.name.as_str() {
            "minecraft:water" => Some(FluidKind::Water),
            "minecraft:lava" => Some(FluidKind::Lava),
            name if level.is_some() => FluidKind::of_liquid_block(name),
            _ => None,
        };
        if let Some(source) = source {
            let level = level
                .and_then(|v| v.parse::<u8>().ok())
                .filter(|v| *v <= 15)
                .ok_or_else(|| invalid("invalid liquid block level"))?;
            let kind = if level == 0 { source } else { source.flowing() };
            return Ok(Self {
                kind,
                amount: if level == 0 || level >= 8 {
                    8
                } else {
                    8 - level
                },
                falling: level >= 8,
            });
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
    /// Each mod fluid's liquid block by level, in the table's order (one per
    /// source and flowing pair): air where it has none or the registry lacks
    /// it (`BaseFlowingFluid.createLegacyBlock` without a block, a block a
    /// modpack's jar gave no blockstate).
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
        Ok(Self {
            air,
            water: levels("minecraft:water")?,
            lava: levels("minecraft:lava")?,
            mods: FluidKind::all()
                .filter(|kind| !kind.is_vanilla() && kind.source() == *kind)
                .map(|kind| {
                    kind.info()
                        .block
                        .and_then(|block| levels(block).ok())
                        .unwrap_or([air; 16])
                })
                .collect(),
        })
    }
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
            // A mod fluid with no liquid block of its own can't stand in the world.
            let pair = state.kind.source().index().checked_sub(VANILLA.len());
            pair.and_then(|pair| self.mods.get(pair / 2))
                .map_or(self.air, |levels| levels[level])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FluidKind, FluidNumbers, FluidState, FluidTemplate};

    /// The table gives vanilla's fluids what the enum gave them: their
    /// `minecraft:fluid` ids and names, families, sources and no numbers of
    /// their own (water's and lava's rules stay in the flowing code).
    #[test]
    fn vanillas_fluids_keep_their_ids_families_and_rules() {
        let names = [
            "minecraft:empty",
            "minecraft:flowing_water",
            "minecraft:water",
            "minecraft:flowing_lava",
            "minecraft:lava",
        ];
        for (id, (kind, name)) in FluidKind::ALL.into_iter().zip(names).enumerate() {
            assert_eq!((kind.index(), kind.name()), (id, name));
            assert_eq!(FluidKind::from_name(name), Some(kind));
            assert_eq!(kind.numbers(), FluidNumbers::default());
            assert_eq!(kind.mask(), 1 << id);
        }
        use FluidKind as K;
        let rows = [
            (K::Empty, FluidTemplate::Empty, K::Empty, K::Empty),
            (
                K::FlowingWater,
                FluidTemplate::Water,
                K::Water,
                K::FlowingWater,
            ),
            (K::Water, FluidTemplate::Water, K::Water, K::FlowingWater),
            (K::FlowingLava, FluidTemplate::Lava, K::Lava, K::FlowingLava),
            (K::Lava, FluidTemplate::Lava, K::Lava, K::FlowingLava),
        ];
        for (kind, template, source, flowing) in rows {
            assert_eq!(
                (kind.template(), kind.source(), kind.flowing()),
                (template, source, flowing)
            );
        }
        assert!(K::Water.same_family(K::FlowingWater) && K::Lava.same_family(K::FlowingLava));
        assert!(!K::Water.same_family(K::Lava) && !K::Empty.same_family(K::Water));
        assert!(K::Empty.same_family(K::Empty));
        let sources: Vec<_> = K::ALL
            .into_iter()
            .filter(|k| FluidState::new(*k, 8, false).is_source())
            .collect();
        assert_eq!(sources, [K::Water, K::Lava]);
        // Logs and probes print the enum's names.
        assert_eq!(
            format!("{:?} {:?}", K::FlowingLava, K::Empty),
            "FlowingLava Empty"
        );
    }

    /// A content root's mod fluids take the ids after vanilla's, each source
    /// then its flowing form, with their template and numbers; vanilla's rows
    /// don't move. The table is the process's: this test puts it back.
    /// The fluid table is the process's: tests that set it take turns.
    static TABLE: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Reads the content root's table on this test's thread, whatever
    /// carried fluids another test registered; put back when dropped.
    struct RootTableOnly;
    impl RootTableOnly {
        fn on() -> Self {
            super::ROOT_TABLE_ONLY.with(|only| only.set(true));
            Self
        }
    }
    impl Drop for RootTableOnly {
        fn drop(&mut self) {
            super::ROOT_TABLE_ONLY.with(|only| only.set(false));
        }
    }

    /// The carried fluids, once registered, are the table's after vanilla's:
    /// each source then its flowing form, with all their numbers, no
    /// template (their `FluidType` says what they do) and no bit in vanilla's
    /// tag masks; they are found by name, block and bucket.
    #[test]
    fn the_carried_fluids_are_the_tables_with_their_numbers() {
        crate::mod_fluids::register_test_fluids();
        let brine = FluidKind::from_name("test:brine").unwrap();
        let flowing = FluidKind::from_name("test:flowing_brine").unwrap();
        let sludge = FluidKind::from_name("test:sludge").unwrap();
        assert_eq!((brine.index(), flowing.index(), sludge.index()), (5, 6, 7));
        assert_eq!((flowing.source(), brine.flowing()), (brine, flowing));
        assert!(brine.is_source_kind() && !flowing.is_source_kind());
        assert_eq!(brine.template(), FluidTemplate::Empty);
        assert_eq!(
            sludge.numbers(),
            FluidNumbers {
                tick_delay: Some(10),
                drop_off: Some(2),
                slope_find_distance: Some(2),
                convert_to_source: Some(true),
            }
        );
        assert_eq!(brine.mask(), 0);
        assert_eq!(brine.mod_fluid().map(|m| m.id.as_str()), Some("test:brine"));
        assert_eq!(FluidKind::of_bucket("test:brine_bucket"), Some(brine));
        assert_eq!(FluidKind::of_liquid_block("test:sludge"), Some(sludge));
        assert_eq!(format!("{brine:?}"), "Mod(test:brine)");
    }

    #[test]
    fn a_roots_mod_fluids_join_the_table_after_vanillas() {
        let _table = TABLE.lock().unwrap_or_else(|e| e.into_inner());
        let _root = RootTableOnly::on();
        use super::{FluidMask, MOD_FLUIDS_FILE, ModFluid, load_mod_fluids, set_mod_fluids};
        let honey = ModFluid {
            id: "ruby:honey".into(),
            flowing: "ruby:flowing_honey".into(),
            template: "minecraft:water".into(),
            block: Some("ruby:honey".into()),
            tick_delay: Some(20),
            drop_off: Some(2),
            ..ModFluid::default()
        };
        let tar = ModFluid {
            id: "ruby:tar".into(),
            flowing: "ruby:flowing_tar".into(),
            template: "minecraft:lava".into(),
            ..ModFluid::default()
        };
        let root = std::env::temp_dir().join(format!("horizon-mod-fluids-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join(MOD_FLUIDS_FILE),
            serde_json::to_vec(&[honey, tar]).unwrap(),
        )
        .unwrap();
        load_mod_fluids(&root).unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        let names: Vec<_> = FluidKind::all().map(|k| k.name()).collect();
        assert_eq!(
            names[5..],
            [
                "ruby:honey",
                "ruby:flowing_honey",
                "ruby:tar",
                "ruby:flowing_tar"
            ]
        );
        let honey = FluidKind::from_name("ruby:honey").unwrap();
        let flowing = FluidKind::from_name("ruby:flowing_honey").unwrap();
        assert_eq!((honey.index(), flowing.index()), (5, 6));
        assert_eq!((flowing.source(), honey.flowing()), (honey, flowing));
        assert!(honey.same_family(flowing) && !honey.same_family(FluidKind::Water));
        assert!(!honey.is_vanilla() && FluidKind::Lava.is_vanilla());
        assert_eq!(honey.template(), FluidTemplate::Water);
        assert_eq!(
            (honey.numbers().tick_delay, honey.numbers().drop_off),
            (Some(20), Some(2))
        );
        assert_eq!(honey.info().block, Some("ruby:honey"));
        let tar = FluidKind::from_name("ruby:tar").unwrap();
        assert_eq!(
            (tar.template(), tar.numbers()),
            (FluidTemplate::Lava, FluidNumbers::default())
        );
        assert!(
            FluidState::new(honey, 8, false).is_source()
                && !FluidState::new(flowing, 3, false).is_source()
        );
        assert_eq!(honey.mask(), 1 << 5);
        assert_eq!(FluidKind::from_index(9), None);
        // Past the mask's width a fluid is in no tag rather than another's bit.
        assert_eq!(FluidKind(FluidMask::BITS as u16).mask(), 0);
        // A root without the file has vanilla's fluids only.
        set_mod_fluids(&[]).unwrap();
        assert_eq!(FluidKind::all().count(), 5);
        assert_eq!(FluidKind::from_name("ruby:honey"), None);
    }

    /// A mod's liquid block holds its own fluid at each `level`, as water's
    /// and lava's do, and a mod fluid state stands in the world as that block.
    #[test]
    fn a_mod_liquid_block_holds_its_fluid_and_the_fluid_places_it() {
        use super::{FluidBlockBindings, ModFluid, set_mod_fluids};
        use crate::registry::BlockRegistry;
        let _table = TABLE.lock().unwrap_or_else(|e| e.into_inner());
        let _root = RootTableOnly::on();
        let mut rows = vec![r#"["minecraft:air", true, false, "", "minecraft:air"]"#.to_owned()];
        let mut defaults = vec![r#""minecraft:air": 0"#.to_owned()];
        for block in ["minecraft:water", "minecraft:lava", "ruby:honey"] {
            defaults.push(format!(r#""{block}": {}"#, rows.len()));
            for level in 0..16 {
                rows.push(format!(r#"["{block}", false, true, "", {{"id": "{block}", "properties": {{"level": "{level}"}}}}]"#));
            }
        }
        let json = format!(
            r#"{{"defaults": {{{}}}, "states": [{}]}}"#,
            defaults.join(","),
            rows.join(",")
        );
        let (registry, _) = BlockRegistry::from_facts_json_bytes(json.as_bytes()).unwrap();
        set_mod_fluids(&[ModFluid {
            id: "ruby:honey".into(),
            flowing: "ruby:flowing_honey".into(),
            template: "minecraft:water".into(),
            block: Some("ruby:honey".into()),
            bucket: Some("ruby:honey_bucket".into()),
            ..ModFluid::default()
        }])
        .unwrap();
        let honey = FluidKind::from_name("ruby:honey").unwrap();
        // Its bucket pours the source; the flowing form has none of its own.
        assert_eq!(FluidKind::of_bucket("ruby:honey_bucket"), Some(honey));
        assert_eq!(honey.info().bucket, Some("ruby:honey_bucket"));
        assert_eq!(FluidKind::of_bucket("minecraft:water_bucket"), None);
        assert_eq!(
            crate::level::mod_bucket_contents("ruby:honey_bucket"),
            Some((
                crate::level::BucketContent::Fluid(honey),
                "minecraft:item.bucket.empty",
                None
            ))
        );
        assert!(!crate::level::is_lava_bucket("ruby:honey_bucket"));
        let state = |level: u32| {
            registry
                .state(crate::registry::BlockStateId(33 + level))
                .unwrap()
        };
        let fluid = |level: u32| FluidState::from_block(state(level)).unwrap();
        assert_eq!(
            (fluid(0).kind(), fluid(0).amount(), fluid(0).falling()),
            (honey, 8, false)
        );
        assert_eq!(
            (fluid(3).kind(), fluid(3).amount(), fluid(3).falling()),
            (honey.flowing(), 5, false)
        );
        assert_eq!(
            (fluid(8).kind(), fluid(8).falling()),
            (honey.flowing(), true)
        );
        // Water's own block still holds water.
        let water =
            FluidState::from_block(registry.state(crate::registry::BlockStateId(1)).unwrap())
                .unwrap();
        assert_eq!(water.kind(), FluidKind::Water);
        // Each honey state, placed back, is the same block and level.
        let bindings = FluidBlockBindings::new(&registry).unwrap();
        for level in [0, 3, 8] {
            assert_eq!(
                bindings.block(fluid(level)),
                crate::registry::BlockStateId(33 + level)
            );
        }
        set_mod_fluids(&[]).unwrap();
    }

    #[test]
    fn a_fluid_state_reads_its_flowing_form_from_the_table() {
        let flowing = FluidState::from_json(
            r#"{"id":"minecraft:flowing_lava","properties":{"level":"3","falling":"false"}}"#,
        )
        .unwrap();
        assert_eq!(
            (flowing.kind(), flowing.amount(), flowing.is_source()),
            (FluidKind::FlowingLava, 3, false)
        );
        let source =
            FluidState::from_json(r#"{"id":"minecraft:water","properties":{"level":"3"}}"#)
                .unwrap();
        assert_eq!((source.amount(), source.is_source()), (8, true));
    }
}
