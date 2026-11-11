// Mekanism's multiblocks in the game (support module `crate::game::mek_multiblock`
// of the `mekanism` native bundle): the framework the Dynamic Tank
// (`mek_dynamic_tank`), the Induction Matrix (`mek_matrix`), the Thermal
// Evaporation Plant (`mek_evaporation`) and the Thermoelectric Boiler
// (`mek_boiler`) stand on, and the later cuboid multiblocks will. The rules are `mekanism_energy::multiblock`
// (MIT, from Mekanism 10.7.19, Copyright (c) 2017-2025 Aidan C. Brady); this
// module is the glue: which blocks are casings, valves, ports and glass of
// which multiblock (`MultiblockManager`, `TankValidator`, `MatrixValidator`),
// their block entities as the structure's nodes (loaded and unloaded through
// the block entity lifecycle hook points), a formation run on the next level
// tick after a node comes, goes or a block inside changes
// (`FormationProtocol.doUpdate`), the formed multiblocks of each dimension,
// their caches by id (`MultiblockCache`, kept as Mekanism's saved data
// `mekanism_<manager>` in the level's attachments: what a multiblock holds
// comes back when it forms again), the `inventory_id` its blocks keep, its
// two slots mirrored into the blocks whose screen a player opened (and the
// valves, which hoppers reach), the formation's red sparkles, the
// Configurator's plain use on an unformed one (the reason it doesn't form),
// the heat that heat handlers beside them hand their capacitors (the boiler's
// casings and valves, the evaporation plant's valves), and `/mek probe`. No borrow of this state is held across a call into the
// game.

use super::{Game, mek_world};
use horizon_client::{containers::ModContainer, items::ItemStack, particles::ParticleOptions, text::Text};
use horizon_world::{
    attachments::{AttachmentCodec, AttachmentError, AttachmentType, Holder},
    fluid_stack::FluidStack,
    nbt::{Compound, JavaString, Tag},
};
use mekanism_energy::{
    boiler,
    chemical::{ChemicalRegistry, ChemicalStack},
    evaporation::{self, Heat},
    multiblock::{self, Cuboid, Failure, Inner, Node, Pos, Uuid},
    sides::Direction,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet, HashMap},
    hash::{BuildHasher, Hasher},
};

/// A multiblock manager (`Mekanism.tankManager`, `matrixManager`, `evaporationManager`, `boilerManager`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) enum Kind {
    Tank,
    Matrix,
    Evaporation,
    Boiler,
}

pub(super) const GLASS: &str = "mekanism:structural_glass";

impl Kind {
    pub(super) const ALL: [Self; 4] = [Self::Tank, Self::Matrix, Self::Evaporation, Self::Boiler];

    /// `MultiblockManager.getName`.
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Tank => "dynamicTank",
            Self::Matrix => "inductionMatrix",
            Self::Evaporation => "evaporation",
            Self::Boiler => "thermoelectricBoiler",
        }
    }

    fn casing(self) -> &'static str {
        match self {
            Self::Tank => "mekanism:dynamic_tank",
            Self::Matrix => "mekanism:induction_casing",
            Self::Evaporation => "mekanism:thermal_evaporation_block",
            Self::Boiler => super::mek_boiler::CASING,
        }
    }

    fn valve(self) -> &'static str {
        match self {
            Self::Tank => "mekanism:dynamic_valve",
            Self::Matrix => "mekanism:induction_port",
            Self::Evaporation => "mekanism:thermal_evaporation_valve",
            Self::Boiler => super::mek_boiler::VALVE,
        }
    }

    /// Its block that is neither casing nor valve (`CasingType.OTHER`): the evaporation plant's controller.
    fn other(self) -> Option<&'static str> {
        (self == Self::Evaporation).then_some(super::mek_evaporation::CONTROLLER)
    }

    /// The casing, valve or controller of a multiblock (`TileEntityMultiblock`): its manager and whether it is the
    /// valve.
    pub(super) fn of_block(block: &str) -> Option<(Self, bool)> {
        Self::ALL.into_iter().find_map(|k| {
            if block == k.casing() || k.other() == Some(block) {
                Some((k, false))
            } else if block == k.valve() {
                Some((k, true))
            } else {
                None
            }
        })
    }

    /// The blocks whose item handler reaches the multiblock's slots (Mekanism's `Attributes.INVENTORY`): the dynamic
    /// valve and the induction port, the evaporation controller; the boiler has no slots.
    fn holds_inventory(self, block: &str) -> bool {
        match self {
            Self::Evaporation => self.other() == Some(block),
            Self::Boiler => false,
            _ => block == self.valve(),
        }
    }

    /// What `block` is to a structure of this manager (`getCasingType`, `isFrameCompatible`): structural glass
    /// interfaces with every manager whose name has no "reactor".
    fn node(self, block: &str) -> Option<Node> {
        if block == self.casing() {
            Some(Node::Frame)
        } else if block == self.valve() {
            Some(Node::Valve)
        } else if self.other() == Some(block) {
            Some(Node::Other)
        } else if block == GLASS && !self.name().to_ascii_lowercase().contains("reactor") {
            Some(Node::Structural)
        } else {
            None
        }
    }

    /// `MultiblockManager` names its saved data `mekanism_<name lowered>`.
    fn data_name(self) -> &'static str {
        match self {
            Self::Tank => "dynamictank",
            Self::Matrix => "inductionmatrix",
            Self::Evaporation => "evaporation",
            Self::Boiler => "thermoelectricboiler",
        }
    }

    fn attachment(self) -> &'static AttachmentType<Compound> {
        match self {
            Self::Tank => &TANK_DATA,
            Self::Matrix => &MATRIX_DATA,
            Self::Evaporation => &EVAPORATION_DATA,
            Self::Boiler => &BOILER_DATA,
        }
    }

    /// How many containers of each type its multiblock has, as its cache is first made (`CacheSubstance.prefab`):
    /// chemical tanks, item slots, fluid tanks, energy containers, heat capacitors.
    fn shape(self) -> [usize; 5] {
        match self {
            Self::Tank => [1, 2, 1, 0, 0],
            Self::Matrix => [0, 2, 0, 1, 0],
            Self::Evaporation => [0, 4, 2, 0, 1],
            // Steam, heated coolant and cooled coolant; water; its heat.
            Self::Boiler => [3, 0, 1, 0, 1],
        }
    }

    /// The slots its blocks show.
    fn slots(self) -> usize {
        self.shape()[1]
    }
}

fn identity(c: &Compound) -> Result<Compound, AttachmentError> {
    Ok(c.clone())
}

/// The saved data of each manager (`MultiblockCacheDataHandler`: `{cache: [...]}`), in the level's attachments.
static TANK_DATA: AttachmentType<Compound> = AttachmentType {
    namespace: "mekanism",
    name: "dynamictank",
    default: Compound::new,
    codec: Some(AttachmentCodec { encode: identity, decode: identity }),
    copy_on_death: false,
    sync: false,
};
static MATRIX_DATA: AttachmentType<Compound> = AttachmentType {
    namespace: "mekanism",
    name: "inductionmatrix",
    default: Compound::new,
    codec: Some(AttachmentCodec { encode: identity, decode: identity }),
    copy_on_death: false,
    sync: false,
};
static EVAPORATION_DATA: AttachmentType<Compound> = AttachmentType {
    namespace: "mekanism",
    name: "evaporation",
    default: Compound::new,
    codec: Some(AttachmentCodec { encode: identity, decode: identity }),
    copy_on_death: false,
    sync: false,
};
static BOILER_DATA: AttachmentType<Compound> = AttachmentType {
    namespace: "mekanism",
    name: "thermoelectricboiler",
    default: Compound::new,
    codec: Some(AttachmentCodec { encode: identity, decode: identity }),
    copy_on_death: false,
    sync: false,
};

pub(super) fn is_air(block: &str) -> bool {
    matches!(block, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
}

/// What `MatrixValidator.validateInner` takes inside a matrix besides air: an induction cell or provider of any tier
/// (`IInternalMultiblock`).
pub(super) fn is_matrix_inner(block: &str) -> bool {
    block.strip_prefix("mekanism:").is_some_and(|b| {
        ["basic_", "advanced_", "elite_", "ultimate_"]
            .iter()
            .any(|t| b.strip_prefix(t).is_some_and(|rest| rest == "induction_cell" || rest == "induction_provider"))
    })
}

/// A block of the framework's: a casing, valve or port of some manager, or structural glass.
pub(super) fn is_part(block: &str) -> bool {
    block == GLASS || Kind::of_block(block).is_some()
}

/// A multiblock's cache (`MultiblockCache`): its containers, as Mekanism saves them, and the rest of its own
/// (`TankCache.editMode`).
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Cache {
    pub(super) chemicals: Vec<ChemicalStack>,
    pub(super) items: Vec<Option<ItemStack>>,
    pub(super) fluids: Vec<FluidStack>,
    pub(super) energy: Vec<u64>,
    /// Heat capacitors (`heat_capacitors`): none until the multiblock first saves them, as a new cache's default
    /// capacitor reads 300 K of a capacity of 1 (`CacheSubstance.HEAT.defaultPrefab`).
    pub(super) heat: Vec<Option<Heat>>,
    pub(super) edit_mode: i32,
}

impl Cache {
    fn of_shape(kind: Kind) -> Self {
        let [c, i, f, e, h] = kind.shape();
        Self {
            chemicals: vec![ChemicalStack::EMPTY; c],
            items: vec![None; i],
            fluids: vec![FluidStack::EMPTY; f],
            energy: vec![0; e],
            heat: vec![None; h],
            edit_mode: 0,
        }
    }

    /// `MultiblockCache.merge` (`StackUtils.merge`, `StorageUtils.mergeFluidTanks`, `mergeTanks`,
    /// `mergeEnergyContainers`): the other cache's contents join these; what doesn't fit or differs is lost.
    fn merge(&mut self, other: &Cache) {
        for (mine, theirs) in self.fluids.iter_mut().zip(&other.fluids) {
            let same = mine.is_empty() || FluidStack::is_same_fluid_same_components(mine, theirs);
            let (kept, _) = multiblock::merge_amount(i64::from(mine.amount().max(0)), i64::from(theirs.amount().max(0)), same, i64::from(i32::MAX));
            if mine.is_empty() && !theirs.is_empty() {
                *mine = theirs.copy_with_amount(kept as i32);
            } else if kept as i32 != mine.amount() {
                *mine = mine.copy_with_amount(kept as i32);
            }
        }
        for (mine, theirs) in self.chemicals.iter_mut().zip(&other.chemicals) {
            let same = mine.is_empty() || ChemicalStack::is_same_chemical(mine, theirs);
            let (kept, _) = multiblock::merge_amount(mine.amount().max(0), theirs.amount().max(0), same, i64::MAX);
            if mine.is_empty() && !theirs.is_empty() {
                *mine = theirs.copy_with_amount(kept);
            } else {
                mine.set_amount(kept);
            }
        }
        for (mine, theirs) in self.items.iter_mut().zip(&other.items) {
            match (mine.as_mut(), theirs) {
                (None, Some(t)) => *mine = Some(t.clone()),
                (Some(m), Some(t)) if m.item == t.item && m.data() == t.data() => m.count = m.count.saturating_add(t.count).min(64),
                _ => {}
            }
        }
        for (mine, theirs) in self.energy.iter_mut().zip(&other.energy) {
            *mine = mine.saturating_add(*theirs).min(i64::MAX as u64);
        }
        // `mergeHeatCapacitors`: heat and capacity add up (an unsaved capacitor is 300 K of a capacity of 1).
        let unsaved = Heat { stored: evaporation::AMBIENT, capacity: 1.0 };
        for (mine, theirs) in self.heat.iter_mut().zip(&other.heat) {
            *mine = Some(mine.unwrap_or(unsaved).merge(theirs.unwrap_or(unsaved)));
        }
        self.edit_mode = other.edit_mode;
    }

    /// `MultiblockCache.save` with its `inventory_id`, as `MultiblockCacheDataHandler` lists it.
    fn save(&self, id: Uuid) -> Compound {
        let mut c = Compound::new();
        c.insert(key("inventory_id"), Tag::IntArray(id.0.to_vec()));
        let mut list = |name: &str, index: &str, entries: Vec<(usize, Tag)>, count: usize| {
            if count == 0 {
                return;
            }
            c.insert(key(&format!("{name}_stored")), Tag::Int(count as i32));
            let list: Vec<Tag> = entries
                .into_iter()
                .map(|(i, stored)| {
                    let mut e = Compound::new();
                    e.insert(key(index), Tag::Byte(i as i8));
                    e.insert(key(if name == "items" { "item" } else { "stored" }), stored);
                    Tag::Compound(e)
                })
                .collect();
            if !list.is_empty() {
                c.insert(key(name), Tag::List(list));
            }
        };
        let chemicals = self.chemicals.iter().enumerate().filter_map(|(i, s)| Some((i, Tag::Compound(super::mek_chemicals::stack_nbt(s)?)))).collect();
        list("chemical_tanks", "tank", chemicals, self.chemicals.len());
        let items = self.items.iter().enumerate().filter_map(|(i, s)| Some((i, horizon_client::save::item_to_nbt(s.as_ref().filter(|s| s.count > 0)?)))).collect();
        list("items", "slot", items, self.items.len());
        let fluids = self.fluids.iter().enumerate().filter_map(|(i, s)| Some((i, Tag::Compound(s.save()?)))).collect();
        list("fluid_tanks", "tank", fluids, self.fluids.len());
        let energy = self.energy.iter().enumerate().filter(|(_, e)| **e > 0).map(|(i, e)| (i, Tag::Long((*e).min(i64::MAX as u64) as i64))).collect();
        list("energy_containers", "container", energy, self.energy.len());
        let unsaved = Heat { stored: evaporation::AMBIENT, capacity: 1.0 };
        let heat = self
            .heat
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let h = h.unwrap_or(unsaved);
                let mut e = Compound::new();
                e.insert(key("heat_capacity"), Tag::Double(h.capacity.to_bits()));
                (i, Tag::Double(h.stored.to_bits()), e)
            })
            .collect::<Vec<_>>();
        if !heat.is_empty() {
            c.insert(key("heat_capacitors_stored"), Tag::Int(heat.len() as i32));
            let list = heat
                .into_iter()
                .map(|(i, stored, mut e)| {
                    e.insert(key("container"), Tag::Byte(i as i8));
                    e.insert(key("stored"), stored);
                    Tag::Compound(e)
                })
                .collect();
            c.insert(key("heat_capacitors"), Tag::List(list));
        }
        c.insert(key("edit_mode"), Tag::Int(self.edit_mode));
        c
    }

    /// `MultiblockCache.load`.
    fn load(c: &Compound, registry: &ChemicalRegistry) -> Option<(Uuid, Self)> {
        let Some(Tag::IntArray(id)) = c.get(&key("inventory_id")) else { return None };
        let id: [i32; 4] = id.as_slice().try_into().ok()?;
        let count = |name: &str| mek_world::number(c.get(&key(&format!("{name}_stored")))).unwrap_or(0).clamp(0, 64) as usize;
        let entries = |name: &str, index: &str| -> Vec<(usize, Tag)> {
            let Some(Tag::List(list)) = c.get(&key(name)) else { return Vec::new() };
            list.iter()
                .filter_map(|e| match e {
                    Tag::Compound(e) => {
                        let i = mek_world::number(e.get(&key(index)))? as usize;
                        Some((i, e.get(&key(if name == "items" { "item" } else { "stored" }))?.clone()))
                    }
                    _ => None,
                })
                .collect()
        };
        let mut cache = Cache {
            chemicals: vec![ChemicalStack::EMPTY; count("chemical_tanks")],
            items: vec![None; count("items")],
            fluids: vec![FluidStack::EMPTY; count("fluid_tanks")],
            energy: vec![0; count("energy_containers")],
            heat: vec![None; count("heat_capacitors")],
            edit_mode: mek_world::number(c.get(&key("edit_mode"))).unwrap_or(0) as i32,
        };
        if let Some(Tag::List(list)) = c.get(&key("heat_capacitors")) {
            for e in list {
                let Tag::Compound(e) = e else { continue };
                let double = |name: &str| match e.get(&key(name)) {
                    Some(Tag::Double(v)) => Some(f64::from_bits(*v)),
                    Some(Tag::Float(v)) => Some(f64::from(f32::from_bits(*v))),
                    _ => None,
                };
                let i = mek_world::number(e.get(&key("container"))).unwrap_or(0) as usize;
                if let (Some(slot), Some(stored), Some(capacity)) = (cache.heat.get_mut(i), double("stored"), double("heat_capacity")) {
                    *slot = Some(Heat { stored, capacity: capacity.max(1.0) });
                }
            }
        }
        for (i, tag) in entries("chemical_tanks", "tank") {
            if let (Some(slot), Tag::Compound(s)) = (cache.chemicals.get_mut(i), &tag) {
                *slot = super::mek_chemicals::stack_of(s, registry);
            }
        }
        for (i, tag) in entries("items", "slot") {
            if let Some(slot) = cache.items.get_mut(i) {
                *slot = horizon_client::save::item_from_nbt(&tag);
            }
        }
        for (i, tag) in entries("fluid_tanks", "tank") {
            if let (Some(slot), Tag::Compound(s)) = (cache.fluids.get_mut(i), &tag) {
                *slot = FluidStack::parse_optional(s);
            }
        }
        for (i, tag) in entries("energy_containers", "container") {
            if let Some(slot) = cache.energy.get_mut(i) {
                *slot = mek_world::number(Some(&tag)).unwrap_or(0).max(0) as u64;
            }
        }
        Some((Uuid(id), cache))
    }
}

/// A formed multiblock (`MultiblockData` once `isFormed`).
#[derive(Clone, Debug)]
pub(super) struct Formed {
    pub(super) kind: Kind,
    /// `inventoryID`: its cache's key, which its blocks keep as `inventory_id`.
    pub(super) id: Uuid,
    pub(super) cuboid: Cuboid,
    pub(super) locations: BTreeSet<Pos>,
    pub(super) internal: BTreeSet<Pos>,
    pub(super) valves: Vec<(Pos, Direction)>,
    /// A boiler's volumes, elements and disperser layer (`BoilerValidator.postcheck`).
    pub(super) boiler: Option<boiler::Shape>,
    /// Blocks whose container shows the multiblock's slots: its valves (hoppers reach them) and the blocks whose
    /// screen a player opened.
    views: BTreeSet<Pos>,
    /// The slots as last written into them, to tell a player's or a hopper's change.
    pushed: Vec<Option<ItemStack>>,
}

impl Formed {
    pub(super) fn volume(&self) -> i32 {
        self.cuboid.volume()
    }
}

/// A dimension's multiblocks.
#[derive(Default)]
struct Dim {
    /// The framework's block entities loaded here, by position, as their block.
    nodes: HashMap<Pos, String>,
    formed: BTreeMap<Uuid, Formed>,
    /// The formed multiblock each wall or inner position belongs to.
    at: HashMap<(Kind, Pos), Uuid>,
    caches: HashMap<Kind, BTreeMap<Uuid, Cache>>,
    loaded: bool,
    dirty: BTreeSet<Kind>,
    /// Nodes to run a formation from at the next level tick.
    pending: BTreeSet<Pos>,
    /// Formed multiblocks to check again.
    recheck: BTreeSet<Uuid>,
    /// Multiblocks formed here so far, for `/mek probe`.
    formations: u64,
}

thread_local! {
    static DIMS: RefCell<HashMap<String, Dim>> = RefCell::new(HashMap::new());
    /// Blocks set since the last level tick (`native_block_changed` says no dimension).
    static SET: RefCell<Vec<Pos>> = const { RefCell::new(Vec::new()) };
    static ANY: Cell<bool> = const { Cell::new(false) };
    static IDS: Cell<u64> = const { Cell::new(0) };
    /// Heat handed to a formed multiblock's capacitor since its last tick (`heatToHandle`), by its id.
    static HEAT: RefCell<HashMap<Uuid, f64>> = RefCell::new(HashMap::new());
}

fn with_dim<R>(dimension: &str, change: impl FnOnce(&mut Dim) -> R) -> R {
    DIMS.with(|d| {
        let mut d = d.borrow_mut();
        if !d.contains_key(dimension) {
            d.insert(dimension.to_owned(), Dim::default());
        }
        change(d.get_mut(dimension).expect("just made"))
    })
}

fn key(name: &str) -> JavaString {
    JavaString::from(name)
}

/// `UUID.randomUUID`.
fn random_id() -> Uuid {
    let n = IDS.get() + 1;
    IDS.set(n);
    let mut a = std::collections::hash_map::RandomState::new().build_hasher();
    a.write_u64(n);
    let mut b = std::collections::hash_map::RandomState::new().build_hasher();
    b.write_u64(n.rotate_left(17) ^ 0x9E37_79B9_7F4A_7C15);
    Uuid::random((u128::from(a.finish()) << 64) | u128::from(b.finish()))
}

fn beside(p: Pos, d: Direction) -> Pos {
    let o = d.offset();
    [p[0] + o[0], p[1] + o[1], p[2] + o[2]]
}

/// A world opens or closes.
pub(super) fn clear() {
    DIMS.with(|d| d.borrow_mut().clear());
    SET.with(|s| s.borrow_mut().clear());
    ANY.set(false);
    HEAT.with(|h| h.borrow_mut().clear());
    super::mek_matrix::clear();
    super::mek_evaporation::clear();
    super::mek_boiler::clear();
}

// ---- Block entities --------------------------------------------------------------------------

/// `native_block_entity`: the framework's blocks, induction cells and providers (`energy_containers` when they
/// hold Joules).
pub(super) fn new_entity(block: &str, entity: &mut Option<(String, Compound)>) -> bool {
    if is_part(block) || is_matrix_inner(block) {
        *entity = Some((block.to_owned(), Compound::new()));
        return true;
    }
    false
}

/// `native_block_entity_container`: the multiblock's slots in a hopper's menu (its screen shows them); the dynamic
/// valves and induction ports reach their two from every face, as Mekanism's do, the evaporation controller its
/// four (the fill slots take, the output slots give), the casings none.
pub(super) fn container(block: &str) -> Option<ModContainer> {
    let (kind, _) = Kind::of_block(block)?;
    let reach = kind.holds_inventory(block);
    // The evaporation plant's blocks and valves have no screen and no slots.
    if kind == Kind::Evaporation && !reach {
        return None;
    }
    let (all, inputs, outputs) = match kind {
        Kind::Evaporation => (vec![0, 1, 2, 3], vec![0, 2], vec![1, 3]),
        // The boiler's casings and valves open its screen, which has no slots.
        Kind::Boiler => (Vec::new(), Vec::new(), Vec::new()),
        _ => (vec![0, 1], vec![0, 1], vec![0, 1]),
    };
    Some(ModContainer {
        slots: 5,
        faces: std::array::from_fn(|_| if reach { all.clone() } else { Vec::new() }),
        inputs,
        outputs,
        menu: Some("minecraft:hopper".into()),
    })
}

/// `native_block_entity_load`: a node joins the dimension's, and a formation runs from it at the next level tick
/// (Mekanism's tile validates its structure on its tenth tick). Whether it was the framework's.
pub(super) fn loaded(dimension: &str, pos: Pos, id: &str) -> bool {
    if is_matrix_inner(id) {
        return true;
    }
    if !is_part(id) {
        return false;
    }
    with_dim(dimension, |d| {
        d.nodes.insert(pos, id.to_owned());
        d.pending.insert(pos);
    });
    ANY.set(true);
    true
}

/// `native_block_entity_unload`: a node leaving breaks the multiblock it is in (`TileEntityMultiblock.setRemoved`:
/// `structure.invalidate`), whose contents stay in its cache; its neighbours run a formation again. An inner block
/// of a matrix going makes it check again (`blockRemoved`: `recheckStructure`).
pub(super) fn unloaded(game_dimension: &str, pos: Pos, id: &str, removed: bool) -> bool {
    let inner = is_matrix_inner(id);
    if !inner && !is_part(id) {
        return false;
    }
    let mut gone = Vec::new();
    with_dim(game_dimension, |d| {
        if inner {
            if removed && let Some(owner) = d.at.get(&(Kind::Matrix, pos)).copied() {
                d.recheck.insert(owner);
            }
            return;
        }
        d.nodes.remove(&pos);
        for kind in Kind::ALL {
            if let Some(owner) = d.at.get(&(kind, pos)).copied() {
                gone.push(owner);
            }
        }
        for d6 in Direction::ALL {
            let n = beside(pos, d6);
            if d.nodes.contains_key(&n) {
                d.pending.insert(n);
            }
        }
    });
    for owner in gone {
        unform(game_dimension, owner, None);
    }
    true
}

/// `native_block_changed`.
pub(super) fn block_set(pos: Pos) {
    if ANY.get() {
        SET.with(|s| s.borrow_mut().push(pos));
    }
}

// ---- Formation -------------------------------------------------------------------------------

/// What `validateInner` finds at `p` for `kind`: air, or a matrix's cell or provider.
fn inner_at(game: &mut Game<'_>, kind: Kind, p: Pos) -> Inner {
    let block = game.native_block(p);
    if is_air(&block) {
        return Inner::Air;
    }
    if kind == Kind::Matrix && is_matrix_inner(&block) && game.native_with_mod_block_entity(p, |_| ()).is_some() {
        return Inner::Internal;
    }
    Inner::Invalid
}

/// The tile's `inventory_id` (`cachedID`).
fn tile_id(game: &mut Game<'_>, p: Pos) -> Option<Uuid> {
    game.native_with_mod_block_entity(p, |m| match m.nbt.get(&key("inventory_id")) {
        Some(Tag::IntArray(v)) => <[i32; 4]>::try_from(v.as_slice()).ok().map(Uuid),
        _ => None,
    })
    .flatten()
}

fn set_tile_id(game: &mut Game<'_>, p: Pos, id: Option<Uuid>) {
    game.native_with_mod_block_entity(p, |m| match id {
        Some(id) => {
            m.nbt.insert(key("inventory_id"), Tag::IntArray(id.0.to_vec()));
        }
        None => {
            m.nbt.remove(&key("inventory_id"));
        }
    });
}

/// Why the structure from `seed` didn't form, as the Configurator says it (`FormationResult.getResultText`);
/// none without a message.
fn failure_text(game: &mut Game<'_>, failure: Failure) -> Option<String> {
    match failure {
        Failure::Fail => None,
        Failure::InvalidFrame(p) => Some(format!("Couldn't create frame, invalid block at ({}, {}, {}).", p[0], p[1], p[2])),
        Failure::InvalidInner(p) => {
            let block = game.native_block(p);
            let name = block.split_once(':').map_or(block.as_str(), |(_, n)| n).replace('_', " ");
            Some(format!("Inner structure contains an illegal block ({name}) located at ({}, {}, {}).", p[0], p[1], p[2]))
        }
        Failure::ControllerConflict(p) => Some(format!("Controller conflict: found extra controller at ({}, {}, {}).", p[0], p[1], p[2])),
        Failure::NoController => Some("Couldn't form, no controller found.".into()),
        Failure::NoDisperser => Some("Couldn't form, no Pressure Disperser layer found.".into()),
        Failure::MissingDisperser(p) => Some(format!("Couldn't form, expected but didn't find Pressure Disperser at ({}, {}, {}).", p[0], p[1], p[2])),
        Failure::ExtraDisperser => Some("Couldn't form, found invalid Pressure Dispersers.".into()),
        Failure::InvalidSuperheating => Some("Couldn't form, invalid Superheating Element arrangement.".into()),
        Failure::AirPockets => Some("Couldn't form, found disconnected interior air pockets.".into()),
    }
}

/// `FormationProtocol.doUpdate` for the structure of `kind` holding `seed`. The multiblocks it touches that formed
/// before break; when it forms, the caches its blocks name merge into a new one under a new id, which every block
/// keeps. The positions it explored, and Ok: the id it formed under, or Err: why not. None: no structure.
fn run(game: &mut Game<'_>, dimension: &str, kind: Kind, seed: Pos) -> Option<(Vec<Pos>, Result<Uuid, Failure>)> {
    let nodes = with_dim(dimension, |d| {
        multiblock::explore(seed, |p| d.nodes.get(&p).and_then(|b| kind.node(b)))
    });
    if nodes.is_empty() || !nodes.values().any(|n| n.is_multiblock()) {
        return None;
    }
    let explored: Vec<Pos> = nodes.keys().copied().collect();
    let mut shape = None;
    let result = match kind {
        Kind::Evaporation => evaporation::form(&nodes, |p| inner_at(game, kind, p)),
        Kind::Boiler => super::mek_boiler::form(game, &nodes).map(|(f, s)| {
            shape = Some(s);
            f
        }),
        _ => multiblock::form(&nodes, multiblock::MIN_BOUNDS, multiblock::MAX_BOUNDS, |p| inner_at(game, kind, p)),
    };
    let touched: BTreeSet<Uuid> = with_dim(dimension, |d| nodes.keys().filter_map(|p| d.at.get(&(kind, *p)).copied()).collect());
    let formation = match result {
        Ok(f) => f,
        Err(failure) => {
            for id in touched {
                unform(dimension, id, Some(game));
            }
            return Some((explored, Err(failure)));
        }
    };
    // Mekanism removes the multiblock it had (`markForUpdate`) and forms it anew under a new id; the screens open on
    // its blocks stay open.
    let kept_views: BTreeSet<Pos> = with_dim(dimension, |d| touched.iter().filter_map(|id| d.formed.get(id)).flat_map(|f| f.views.iter().copied()).collect());
    for id in &touched {
        unform(dimension, *id, Some(game));
    }
    // The caches the blocks name (`idsFound`); a block naming one that is gone forgets it (`resetCache`).
    let mut found: BTreeSet<Uuid> = BTreeSet::new();
    for p in &formation.locations {
        if let Some(id) = tile_id(game, *p) {
            let known = with_dim(dimension, |d| d.caches.get(&kind).is_some_and(|c| c.contains_key(&id)));
            if known {
                found.insert(id);
            } else {
                set_tile_id(game, *p, None);
            }
        }
    }
    let id = random_id();
    let holders: Vec<Pos> =
        with_dim(dimension, |d| formation.locations.iter().copied().filter(|p| d.nodes.get(p).is_some_and(|b| kind.holds_inventory(b))).collect());
    with_dim(dimension, |d| {
        let caches = d.caches.entry(kind).or_default();
        let mut merged: Option<Cache> = None;
        for old in &found {
            if let Some(c) = caches.remove(old) {
                match merged.as_mut() {
                    None => merged = Some(c),
                    Some(m) => m.merge(&c),
                }
            }
        }
        let mut cache = merged.unwrap_or_else(|| Cache::of_shape(kind));
        let [c, i, f, e, h] = kind.shape();
        cache.chemicals.resize(c.max(cache.chemicals.len()), ChemicalStack::EMPTY);
        cache.items.resize(i.max(cache.items.len()), None);
        cache.fluids.resize(f.max(cache.fluids.len()), FluidStack::EMPTY);
        cache.energy.resize(e.max(cache.energy.len()), 0);
        cache.heat.resize(h.max(cache.heat.len()), None);
        caches.insert(id, cache.clone());
        d.dirty.insert(kind);
        let formed = Formed {
            kind,
            id,
            cuboid: formation.cuboid,
            locations: formation.locations.iter().copied().collect(),
            internal: formation.internal.iter().copied().collect(),
            valves: formation.valves.clone(),
            boiler: shape,
            views: holders.iter().copied().chain(kept_views.iter().copied().filter(|p| formation.locations.contains(p))).collect(),
            pushed: Vec::new(),
        };
        for p in formed.locations.iter().chain(&formed.internal) {
            d.at.insert((kind, *p), id);
        }
        d.formed.insert(id, formed);
        d.formations += 1;
    });
    for p in &formation.locations {
        set_tile_id(game, *p, Some(id));
    }
    // `onCreated`: the contents capped to the new size, the matrix's cells and providers, the plant's heat.
    match kind {
        Kind::Tank => super::mek_dynamic_tank::created(dimension, id),
        Kind::Matrix => super::mek_matrix::created(game, dimension, id),
        Kind::Evaporation => super::mek_evaporation::created(game, dimension, id),
        Kind::Boiler => super::mek_boiler::created(game, dimension, id),
    }
    sparkle(game, &formation.cuboid);
    Some((explored, Ok(id)))
}

/// A formed multiblock breaks (`MultiblockData.remove`): its contents stay in its cache, its blocks keep its id.
/// The slots its blocks mirrored are emptied (after the cache took what a player last put there), so no block drops
/// them; a matrix's queued Joules land in its cells first (`MatrixEnergyContainer.invalidate`).
fn unform(dimension: &str, id: Uuid, game: Option<&mut Game<'_>>) {
    if let Some(game) = game {
        pull_views(game, dimension, id);
        let Some(formed) = take_formed(dimension, id) else { return };
        if formed.kind == Kind::Matrix {
            super::mek_matrix::removed(dimension, id);
            super::mek_matrix::flush(game, dimension);
        }
        if formed.kind == Kind::Evaporation {
            super::mek_evaporation::removed(Some(game), dimension, &formed);
        }
        if formed.kind == Kind::Boiler {
            super::mek_boiler::removed(Some(game), dimension, &formed);
        }
        HEAT.with(|h| h.borrow_mut().remove(&id));
        for p in &formed.views {
            game.native_with_mod_block_entity(*p, |m| {
                for slot in m.items.iter_mut().take(formed.kind.slots()) {
                    *slot = None;
                }
                m.data.clear();
            });
        }
        return;
    }
    let Some(formed) = take_formed(dimension, id) else { return };
    if formed.kind == Kind::Matrix {
        super::mek_matrix::removed(dimension, id);
    }
    if formed.kind == Kind::Evaporation {
        super::mek_evaporation::removed(None, dimension, &formed);
    }
    if formed.kind == Kind::Boiler {
        super::mek_boiler::removed(None, dimension, &formed);
    }
    HEAT.with(|h| h.borrow_mut().remove(&id));
    PENDING_CLEAR.with(|p| p.borrow_mut().push(Clear { dimension: dimension.to_owned(), kind: formed.kind, id, views: formed.views.iter().copied().collect(), pushed: formed.pushed }));
}

/// The blocks a multiblock that broke with no game at hand showed its slots on (a block entity unloading), emptied at
/// the next level tick; a block whose slots changed since they were written (a block entity made anew with
/// `/data merge`, a hopper) gives them to the multiblock's cache first.
struct Clear {
    dimension: String,
    kind: Kind,
    id: Uuid,
    views: Vec<Pos>,
    pushed: Vec<Option<ItemStack>>,
}

fn take_formed(dimension: &str, id: Uuid) -> Option<Formed> {
    with_dim(dimension, |d| {
        let f = d.formed.remove(&id)?;
        for p in f.locations.iter().chain(&f.internal) {
            if d.at.get(&(f.kind, *p)) == Some(&id) {
                d.at.remove(&(f.kind, *p));
            }
        }
        Some(f)
    })
}

thread_local! {
    /// Views of multiblocks that broke with no game at hand, emptied at the next level tick.
    static PENDING_CLEAR: RefCell<Vec<Clear>> = const { RefCell::new(Vec::new()) };
}

/// `SparkleAnimation`: a hundred red dust particles over each face of the cuboid, just outside it.
fn sparkle(game: &mut Game<'_>, c: &Cuboid) {
    let dust = ParticleOptions::dust([1.0, 0.0, 0.0], 1.0);
    let lo = [f64::from(c.min[0]), f64::from(c.min[1]), f64::from(c.min[2])];
    let hi = [f64::from(c.max[0] + 1), f64::from(c.max[1] + 1), f64::from(c.max[2] + 1)];
    let mut seed = std::collections::hash_map::RandomState::new().build_hasher().finish() | 1;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    for axis in 0..3 {
        for side in [lo[axis] - 0.01, hi[axis] + 0.01] {
            for _ in 0..100 {
                let mut at = [0.0; 3];
                for i in 0..3 {
                    at[i] = if i == axis { side } else { lo[i] + (hi[i] - lo[i]) * next() };
                }
                game.native_level_particles(&dust, at, [0.0; 3], 0.0, 1);
            }
        }
    }
}

/// Loads the dimension's caches from its level's attachments, once.
fn load_caches(game: &mut Game<'_>, dimension: &str) {
    if with_dim(dimension, |d| d.loaded) {
        return;
    }
    let registry = super::mek_chemicals::registry(game);
    let mut loaded: HashMap<Kind, BTreeMap<Uuid, Cache>> = HashMap::new();
    for kind in Kind::ALL {
        let data = game.native_attachments(Holder::Level).ok().and_then(|a| a.get_existing(kind.attachment()).ok().flatten());
        let mut caches = BTreeMap::new();
        if let Some(Tag::List(list)) = data.as_ref().and_then(|d| d.get(&key("cache"))) {
            for entry in list {
                if let Tag::Compound(c) = entry
                    && let Some((id, cache)) = Cache::load(c, &registry)
                {
                    caches.insert(id, cache);
                }
            }
        }
        loaded.insert(kind, caches);
    }
    with_dim(dimension, |d| {
        for (kind, caches) in loaded {
            let mine = d.caches.entry(kind).or_default();
            for (id, cache) in caches {
                mine.entry(id).or_insert(cache);
            }
        }
        d.loaded = true;
    });
}

/// Writes the caches that changed into the level's attachments (`MultiblockCacheDataHandler.save`).
fn save_caches(game: &mut Game<'_>, dimension: &str) {
    let out: Vec<(Kind, Compound)> = with_dim(dimension, |d| {
        std::mem::take(&mut d.dirty)
            .into_iter()
            .map(|kind| {
                let list: Vec<Tag> = d.caches.get(&kind).map_or(Vec::new(), |c| c.iter().map(|(id, cache)| Tag::Compound(cache.save(*id))).collect());
                let mut data = Compound::new();
                data.insert(key("cache"), Tag::List(list));
                (kind, data)
            })
            .collect()
    });
    for (kind, data) in out {
        if let Ok(a) = game.native_attachments(Holder::Level)
            && let Err(e) = a.set(kind.attachment(), data)
        {
            log::warn!("[mekanism] the {} caches weren't kept: {e:?}", kind.data_name());
        }
    }
}

/// `native_level_tick_post`: the formations this tick's changes call for (Mekanism's tiles look on their tenth
/// tick), the slots and screens of what formed, and the caches kept.
pub(super) fn level_ticked(game: &mut Game<'_>, dimension: &str) {
    if !ANY.get() {
        return;
    }
    load_caches(game, dimension);
    super::mek_matrix::flush(game, dimension);
    let clears: Vec<Clear> = PENDING_CLEAR.with(|p| {
        let mut p = p.borrow_mut();
        let (mine, rest): (Vec<_>, Vec<_>) = p.drain(..).partition(|c| c.dimension == dimension);
        *p = rest;
        mine
    });
    for clear in clears {
        let mut adopted = false;
        for p in clear.views {
            if with_dim(dimension, |d| Kind::ALL.iter().any(|k| d.at.contains_key(&(*k, p)))) {
                continue;
            }
            let n = clear.pushed.len();
            let now = game.native_with_mod_block_entity(p, |m| {
                // A block entity made anew from saved data holds as many slots as its `Items` named.
                let mut now: Vec<Option<ItemStack>> = m.items.iter().take(n).cloned().collect();
                now.resize(n, None);
                for slot in m.items.iter_mut().take(n.max(2)) {
                    *slot = None;
                }
                m.data.clear();
                now
            });
            if let Some(now) = now
                && !adopted
                && n > 0
                && now.len() == n
                && now != clear.pushed
            {
                adopted = true;
                with_cache(dimension, clear.kind, clear.id, |c| {
                    for (slot, stack) in c.items.iter_mut().zip(now) {
                        *slot = stack.filter(|s| s.count > 0);
                    }
                });
            }
        }
    }
    // A block changed inside a formed multiblock, beside one of its blocks: it checks again.
    let set: Vec<Pos> = SET.with(|s| std::mem::take(&mut *s.borrow_mut()));
    if !set.is_empty() {
        let shapes: Vec<(Uuid, Cuboid, BTreeSet<Pos>)> = with_dim(dimension, |d| {
            d.formed.values().map(|f| (f.id, f.cuboid, f.locations.iter().chain(&f.internal).copied().collect())).collect()
        });
        for p in set {
            for (id, cuboid, known) in &shapes {
                if cuboid.relative(p) != multiblock::Relative::Inside {
                    continue;
                }
                let block = game.native_block(p);
                let internal = known.contains(&p);
                if multiblock::inside_change_matters(cuboid, p, is_air(&block), internal, |q| known.contains(&q)) {
                    with_dim(dimension, |d| d.recheck.insert(*id));
                }
            }
        }
    }
    let rechecks: Vec<(Kind, Pos)> = with_dim(dimension, |d| {
        std::mem::take(&mut d.recheck).into_iter().filter_map(|id| d.formed.get(&id).and_then(|f| Some((f.kind, *f.locations.iter().next()?)))).collect()
    });
    for (kind, seed) in rechecks {
        if run(game, dimension, kind, seed).is_none() {
            let owner = with_dim(dimension, |d| d.at.get(&(kind, seed)).copied());
            if let Some(owner) = owner {
                unform(dimension, owner, Some(game));
            }
        }
    }
    let pending: Vec<Pos> = with_dim(dimension, |d| std::mem::take(&mut d.pending).into_iter().collect());
    let mut done: BTreeSet<(Kind, Pos)> = BTreeSet::new();
    for seed in pending {
        let Some(block) = with_dim(dimension, |d| d.nodes.get(&seed).cloned()) else { continue };
        for kind in Kind::ALL {
            if kind.node(&block).is_none() || done.contains(&(kind, seed)) {
                continue;
            }
            if let Some((explored, _)) = run(game, dimension, kind, seed) {
                done.extend(explored.into_iter().map(|p| (kind, p)));
            }
        }
    }
    let ids: Vec<Uuid> = with_dim(dimension, |d| d.formed.keys().copied().collect());
    for id in ids {
        push_views(game, dimension, id);
    }
    super::mek_evaporation::flush(game, dimension);
    super::mek_boiler::flush(game, dimension);
    save_caches(game, dimension);
}

/// `native_level_tick_pre`: each formed multiblock's tick (its master's `MultiblockData.tick`), before the blocks
/// of the level, with its slots as the blocks that show them hold them.
pub(super) fn level_tick_pre(game: &mut Game<'_>, dimension: &str) {
    if !ANY.get() {
        return;
    }
    load_caches(game, dimension);
    super::mek_matrix::flush(game, dimension);
    let ids: Vec<(Kind, Uuid)> = with_dim(dimension, |d| d.formed.values().map(|f| (f.kind, f.id)).collect());
    for (kind, id) in ids {
        pull_views(game, dimension, id);
        match kind {
            Kind::Tank => super::mek_dynamic_tank::tick(game, dimension, id),
            Kind::Matrix => super::mek_matrix::tick(game, dimension, id),
            Kind::Evaporation => super::mek_evaporation::tick(game, dimension, id),
            Kind::Boiler => super::mek_boiler::tick(game, dimension, id),
        }
        push_views(game, dimension, id);
    }
    save_caches(game, dimension);
}

// ---- The slots, mirrored into the blocks that show them -----------------------------------------

/// A player or a hopper changed the slots of a block that shows them: the cache takes what it now holds.
fn pull_views(game: &mut Game<'_>, dimension: &str, id: Uuid) {
    let Some((kind, views, pushed)) = with_formed(dimension, id, |f| (f.kind, f.views.clone(), f.pushed.clone())) else { return };
    if pushed.is_empty() {
        return;
    }
    for p in views {
        let Some(now) = game.native_with_mod_block_entity(p, |m| {
            let mut now: Vec<Option<ItemStack>> = m.items.iter().take(pushed.len()).cloned().collect();
            now.resize(pushed.len(), None);
            now
        }) else {
            with_dim(dimension, |d| d.formed.get_mut(&id).map(|f| f.views.remove(&p)));
            continue;
        };
        if now.len() == pushed.len() && now != pushed {
            with_cache(dimension, kind, id, |c| {
                for (slot, stack) in c.items.iter_mut().zip(now) {
                    *slot = stack.filter(|s| s.count > 0);
                }
            });
            break;
        }
    }
}

/// Writes the cache's slots and the screen's data into every block that shows them.
fn push_views(game: &mut Game<'_>, dimension: &str, id: Uuid) {
    let Some((kind, views)) = with_formed(dimension, id, |f| (f.kind, f.views.clone())) else { return };
    let items = with_cache(dimension, kind, id, |c| c.items.clone()).unwrap_or_default();
    let data = match kind {
        Kind::Tank => super::mek_dynamic_tank::view(game, dimension, id),
        Kind::Matrix => super::mek_matrix::view(dimension, id),
        Kind::Evaporation => super::mek_evaporation::view(game, dimension, id),
        Kind::Boiler => super::mek_boiler::view(game, dimension, id),
    };
    for p in views {
        game.native_with_mod_block_entity(p, |m| {
            if m.items.len() < items.len() {
                m.items.resize(5.max(items.len()), None);
            }
            for (slot, stack) in m.items.iter_mut().zip(&items) {
                if slot != stack {
                    *slot = stack.clone();
                }
            }
            if m.data != data {
                m.data = data.clone();
            }
        });
    }
    with_dim(dimension, |d| {
        if let Some(f) = d.formed.get_mut(&id) {
            f.pushed = items;
        }
    });
}

// ---- What the kinds read ----------------------------------------------------------------------------

/// The formed multiblock whose walls or inside hold `pos`.
pub(super) fn formed_at(dimension: &str, kind: Kind, pos: Pos) -> Option<Uuid> {
    with_dim(dimension, |d| d.at.get(&(kind, pos)).copied())
}

pub(super) fn with_formed<R>(dimension: &str, id: Uuid, use_it: impl FnOnce(&Formed) -> R) -> Option<R> {
    with_dim(dimension, |d| d.formed.get(&id).map(use_it))
}

/// `change` on a formed multiblock's cache, kept at the end of the tick.
pub(super) fn with_cache<R>(dimension: &str, kind: Kind, id: Uuid, change: impl FnOnce(&mut Cache) -> R) -> Option<R> {
    with_dim(dimension, |d| {
        let cache = d.caches.get_mut(&kind)?.get_mut(&id)?;
        let before = cache.clone();
        let out = change(cache);
        if *cache != before {
            d.dirty.insert(kind);
        }
        Some(out)
    })
}

// ---- Heat from the blocks beside them ---------------------------------------------------------------------

/// The formed multiblock whose capacitor the block at `pos` is a heat handler of (`TileEntityBoilerCasing`'s and
/// `TileEntityThermalEvaporationValve`'s `getInitialHeatCapacitors`: the multiblock's, none until it forms), with
/// its total inverse conduction (both `VariableHeatCapacitor`s say 1).
pub(super) fn heat_sink(dimension: &str, pos: Pos, block: &str) -> Option<(Uuid, f64)> {
    let kind = match block {
        super::mek_boiler::CASING | super::mek_boiler::VALVE => Kind::Boiler,
        super::mek_evaporation::VALVE => Kind::Evaporation,
        _ => return None,
    };
    let id = formed_at(dimension, kind, pos)?;
    Some((id, boiler::INVERSE_CONDUCTION))
}

/// `IHeatHandler.handleHeat` of a multiblock's capacitor: it lands at the multiblock's next update.
pub(super) fn hand_heat(id: Uuid, heat: f64) {
    HEAT.with(|h| *h.borrow_mut().entry(id).or_insert(0.0) += heat);
}

/// What was handed to a multiblock's capacitor since its last tick.
pub(super) fn take_heat(id: Uuid) -> f64 {
    HEAT.with(|h| h.borrow_mut().remove(&id)).unwrap_or(0.0)
}

// ---- Players ----------------------------------------------------------------------------------------------

/// `native_right_click_block` on a casing, valve or port. Formed: its screen opens with the multiblock's slots
/// (a tank first lets a held bucket fill or empty it, `manageInventory`). Not formed: no screen for an empty hand;
/// a Configurator in a configurating mode runs the formation now (`onRightClick`: `runUpdate`) and says why it
/// failed. A sneaking use with an item in hand goes on to the item (the Configurator's `onSneakRightClick`).
pub(super) fn right_click(game: &mut Game<'_>, pos: Pos, canceled: &mut bool) {
    if *canceled {
        return;
    }
    let block = game.native_block(pos);
    let Some((kind, _)) = Kind::of_block(&block) else { return };
    let dimension = game.native_dimension();
    let stack = game.native_used_stack().clone();
    if game.native_sneaking() {
        if stack.as_ref().is_none_or(|s| s.count == 0) {
            *canceled = true;
        }
        return;
    }
    let formed = formed_at(dimension, kind, pos);
    if let Some(id) = formed {
        // Only the evaporation controller has a screen (`withGui`).
        if kind == Kind::Evaporation && !kind.holds_inventory(&block) {
            return;
        }
        if kind == Kind::Tank && super::mek_dynamic_tank::use_container(game, dimension, id) {
            *canceled = true;
            return;
        }
        with_dim(dimension, |d| d.formed.get_mut(&id).map(|f| f.views.insert(pos)));
        push_views(game, dimension, id);
        return;
    }
    let configurating = stack.as_ref().is_some_and(|s| &*s.item == "mekanism:configurator" && super::mek_configurator::mode_of(s).transmission().is_some());
    if stack.as_ref().is_some_and(|s| s.count > 0) && !configurating {
        // An item in hand: the block's use goes on as a container block's (its screen, empty), so a sneaking use
        // places against it; Mekanism opens nothing and the item acts.
        return;
    }
    *canceled = true;
    if !configurating {
        return;
    }
    load_caches(game, dimension);
    if let Some((_, Err(failure))) = run(game, dimension, kind, pos)
        && let Some(text) = failure_text(game, failure)
    {
        let player = game.native_acting_id();
        game.native_tell(player, Text::literal(text));
    }
}

/// `native_destroy_block`: the slots a block mirrors aren't its own, so nothing drops them.
pub(super) fn breaking(game: &mut Game<'_>, pos: Pos) {
    let id = game.native_block(pos);
    if Kind::of_block(&id).is_some() {
        game.native_with_mod_block_entity(pos, |m| {
            for slot in m.items.iter_mut() {
                *slot = None;
            }
        });
    }
}

// ---- /mek probe ----------------------------------------------------------------------------------------------

/// `/mek probe` of a casing, valve, port or glass: the multiblocks it is in, and what each holds.
pub(super) fn probe(game: &mut Game<'_>, pos: Pos, id: &str) -> Option<Value> {
    if !is_part(id) {
        return None;
    }
    let dimension = game.native_dimension();
    let tile = tile_id(game, pos).map(|u| u.to_string());
    let mut formed = Vec::new();
    for kind in Kind::ALL {
        let Some(owner) = formed_at(dimension, kind, pos) else { continue };
        let Some(mut v) = with_formed(dimension, owner, |f| {
            json!({
                "kind": kind.name(), "id": f.id.to_string(), "min": f.cuboid.min, "max": f.cuboid.max,
                "size": f.cuboid.size(), "volume": f.volume(), "locations": f.locations.len(), "internal": f.internal.len(),
                "valves": f.valves.iter().map(|(p, s)| json!({ "pos": p, "side": s.name() })).collect::<Vec<_>>(),
                "views": f.views.len(),
            })
        }) else {
            continue;
        };
        let extra = match kind {
            Kind::Tank => super::mek_dynamic_tank::probe(game, dimension, owner),
            Kind::Matrix => super::mek_matrix::probe(dimension, owner),
            Kind::Evaporation => super::mek_evaporation::probe(game, dimension, owner),
            Kind::Boiler => super::mek_boiler::probe(game, dimension, owner),
        };
        if let (Some(v), Value::Object(extra)) = (v.as_object_mut(), extra) {
            v.extend(extra);
        }
        formed.push(v);
    }
    let (caches, formations) = with_dim(dimension, |d| (d.caches.values().map(BTreeMap::len).sum::<usize>(), d.formations));
    let active = game.native_block_property(pos, "active");
    Some(json!({
        "pos": pos, "id": id, "formed": !formed.is_empty(), "multiblocks": formed, "inventory_id": tile,
        "caches": caches, "formations": formations, "active": active,
    }))
}

/// Every cache of a dimension (`/mek multiblock caches`), for tests.
pub(super) fn caches_json(game: &mut Game<'_>) -> Value {
    let dimension = game.native_dimension();
    load_caches(game, dimension);
    with_dim(dimension, |d| {
        let mut out = serde_json::Map::new();
        for (kind, caches) in &d.caches {
            let list: Vec<Value> = caches
                .iter()
                .map(|(id, c)| {
                    json!({
                        "id": id.to_string(),
                        "fluids": c.fluids.iter().map(|f| json!({ "fluid": (!f.is_empty()).then(|| f.name()), "amount": f.amount().max(0) })).collect::<Vec<_>>(),
                        "chemicals": c.chemicals.iter().map(|s| json!({ "chemical": s.id(), "amount": s.amount().max(0) })).collect::<Vec<_>>(),
                        "items": c.items.iter().map(|s| s.as_ref().map(|s| json!({ "item": &*s.item, "count": s.count }))).collect::<Vec<_>>(),
                        "heat": c.heat.iter().map(|h| h.map(|h| json!({ "stored": h.stored, "capacity": h.capacity }))).collect::<Vec<_>>(),
                        "edit_mode": c.edit_mode,
                    })
                })
                .collect();
            out.insert(kind.name().into(), Value::Array(list));
        }
        Value::Object(out)
    })
}
