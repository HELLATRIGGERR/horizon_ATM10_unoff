//! Mekanism 10.7.19's multiblock framework (`mekanism.common.lib.multiblock`),
//! as its cuboid multiblocks (the Dynamic Tank, the Induction Matrix, and
//! later the Thermal Evaporation Plant, the Boiler, the SPS, the reactors and
//! the turbine) form: the blocks of one structure (`Structure`, the
//! multiblock tiles of one manager and the structural glass beside them,
//! `FormationProtocol.explore` over touching blocks, at most 5832), the
//! cuboid they make (`StructureHelper.fetchCuboid` over the planes of each
//! axis, `VoxelPlane`), and the check of every block in it
//! (`CuboidStructureValidator.validate`: frames on the edges, casings, valves
//! or glass on the sides, what the multiblock allows inside). Its contents
//! live in a cache by the multiblock's id (`MultiblockCache`), merged when
//! structures join (`StorageUtils.mergeFluidTanks`, `mergeTanks`,
//! `mergeEnergyContainers`). Nothing here knows the game: the caller says
//! what each position holds.
use crate::sides::Direction;
use std::collections::{BTreeMap, VecDeque};

pub type Pos = [i32; 3];

/// `FormationProtocol.MAX_SIZE`: the biggest cuboid's side.
pub const MAX_SIZE: i32 = 18;
/// `FormationProtocol.explore`'s `maxCount` (18³): a structure stops growing there.
pub const EXPLORE_LIMIT: usize = 5832;
/// `CuboidStructureValidator`'s default bounds, as (length x, height y, width z).
pub const MIN_BOUNDS: [i32; 3] = [3, 3, 3];
pub const MAX_BOUNDS: [i32; 3] = [MAX_SIZE, MAX_SIZE, MAX_SIZE];

/// `VoxelCuboid`: the box between two corners, both inside.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cuboid {
    pub min: Pos,
    pub max: Pos,
}

/// `VoxelCuboid.WallRelative`: how many of a position's coordinates sit on a bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WallRelative {
    Side,
    Edge,
    Corner,
    Invalid,
}

/// `VoxelCuboid.CuboidRelative`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relative {
    Inside,
    Walls,
    Outside,
}

/// `FormationProtocol.StructureRequirement` of the cuboid validator: an edge
/// or a corner needs a frame (`FRAME`), a side any casing (`OTHER`), the rest
/// is inside (`INNER`); a position a validator lets go (`IGNORED`: the Thermal
/// Evaporation Plant's top corners) is checked as a casing, and only a failure
/// that says so (`noIgnore`) counts there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Requirement {
    Frame,
    Other,
    Inner,
    Ignored,
}

impl Cuboid {
    pub fn new(min: Pos, max: Pos) -> Self {
        Self { min, max }
    }

    /// Along x.
    pub fn length(&self) -> i32 {
        self.max[0] - self.min[0] + 1
    }

    /// Along y.
    pub fn height(&self) -> i32 {
        self.max[1] - self.min[1] + 1
    }

    /// Along z.
    pub fn width(&self) -> i32 {
        self.max[2] - self.min[2] + 1
    }

    /// `MultiblockData.setShape`'s volume: the whole box, walls too.
    pub fn volume(&self) -> i32 {
        self.length() * self.width() * self.height()
    }

    /// (length, height, width), as `new VoxelCuboid(length, height, width)` takes them.
    pub fn size(&self) -> [i32; 3] {
        [self.length(), self.height(), self.width()]
    }

    fn matches(&self, p: Pos) -> u8 {
        (0..3).map(|i| u8::from(p[i] == self.min[i]) + u8::from(p[i] == self.max[i])).sum()
    }

    /// `getWallRelative`.
    pub fn wall_relative(&self, p: Pos) -> WallRelative {
        match self.matches(p) {
            0 => WallRelative::Invalid,
            1 => WallRelative::Side,
            2 => WallRelative::Edge,
            _ => WallRelative::Corner,
        }
    }

    /// `getRelativeLocation`.
    pub fn relative(&self, p: Pos) -> Relative {
        if (0..3).all(|i| p[i] > self.min[i] && p[i] < self.max[i]) {
            Relative::Inside
        } else if (0..3).all(|i| p[i] >= self.min[i] && p[i] <= self.max[i]) {
            Relative::Walls
        } else {
            Relative::Outside
        }
    }

    /// `getStructureRequirement` of `CuboidStructureValidator`.
    pub fn requirement(&self, p: Pos) -> Requirement {
        match self.wall_relative(p) {
            WallRelative::Edge | WallRelative::Corner => Requirement::Frame,
            WallRelative::Side => Requirement::Other,
            WallRelative::Invalid => Requirement::Inner,
        }
    }

    /// `getSide`: the face a valve on the walls looks out of (west and east
    /// first, then north and south, then down and up).
    pub fn side_of(&self, p: Pos) -> Option<Direction> {
        if p[0] == self.min[0] {
            Some(Direction::West)
        } else if p[0] == self.max[0] {
            Some(Direction::East)
        } else if p[2] == self.min[2] {
            Some(Direction::North)
        } else if p[2] == self.max[2] {
            Some(Direction::South)
        } else if p[1] == self.min[1] {
            Some(Direction::Down)
        } else if p[1] == self.max[1] {
            Some(Direction::Up)
        } else {
            None
        }
    }

    /// `getCenter`.
    pub fn center(&self) -> Pos {
        std::array::from_fn(|i| (self.min[i] + self.max[i]).div_euclid(2))
    }

    /// Every position, x outermost, then y, then z, as `validate` walks them.
    pub fn positions(&self) -> impl Iterator<Item = Pos> + '_ {
        (self.min[0]..=self.max[0])
            .flat_map(move |x| (self.min[1]..=self.max[1]).flat_map(move |y| (self.min[2]..=self.max[2]).map(move |z| [x, y, z])))
    }

    /// `greaterOrEqual` against a size (length, height, width).
    pub fn at_least(&self, size: [i32; 3]) -> bool {
        let s = self.size();
        s[0] >= size[0] && s[1] >= size[1] && s[2] >= size[2]
    }

    /// `maxBounds.greaterOrEqual(this)`.
    pub fn at_most(&self, size: [i32; 3]) -> bool {
        let s = self.size();
        s[0] <= size[0] && s[1] <= size[1] && s[2] <= size[2]
    }
}

/// A block of a structure as the validator sees it: a casing (`CasingType.FRAME`), a valve or port (`VALVE`), a
/// multiblock block that is neither (`OTHER`: the Thermal Evaporation Controller), or structural glass (an
/// `IStructuralMultiblock`: never a frame, and no frame of the planes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Node {
    Frame,
    Valve,
    Other,
    Structural,
}

impl Node {
    /// An `IMultiblock` (a casing, valve or port), which the planes count as frame.
    pub fn is_multiblock(self) -> bool {
        self != Node::Structural
    }
}

/// `FormationProtocol.explore` from `start`: the touching positions `node_at` says are nodes of the structure, the
/// start first, breadth first, until 5832 were reached.
pub fn explore(start: Pos, mut node_at: impl FnMut(Pos) -> Option<Node>) -> BTreeMap<Pos, Node> {
    let mut found = BTreeMap::new();
    let Some(node) = node_at(start) else {
        return found;
    };
    found.insert(start, node);
    let mut open = VecDeque::from([start]);
    while let Some(p) = open.pop_front() {
        if found.len() >= EXPLORE_LIMIT {
            break;
        }
        for d in Direction::ALL {
            let o = d.offset();
            let next = [p[0] + o[0], p[1] + o[1], p[2] + o[2]];
            if found.contains_key(&next) {
                continue;
            }
            if let Some(node) = node_at(next) {
                found.insert(next, node);
                open.push_back(next);
            }
        }
    }
    found
}

/// `VoxelPlane` of one coordinate on one axis, all its nodes merged: the
/// frame nodes' bounds (columns on the axis' horizontal, rows on its
/// vertical), and the frame nodes with the glass inside those bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Plane {
    frame: bool,
    min_col: i32,
    max_col: i32,
    min_row: i32,
    max_row: i32,
    size: i32,
}

impl Plane {
    fn length(&self) -> i32 {
        if self.frame { self.max_col - self.min_col + 1 } else { 0 }
    }

    fn height(&self) -> i32 {
        if self.frame { self.max_row - self.min_row + 1 } else { 0 }
    }

    /// `Structure.add` moves a plane to the major map once it has a frame of at least 2 by 2.
    fn major(&self) -> bool {
        self.frame && self.length() >= 2 && self.height() >= 2
    }

    /// `isFull`.
    fn full(&self) -> bool {
        self.size > 0 && self.length() * self.height() == self.size
    }

    /// `VoxelPlane.equals`: the size and the bounds.
    fn same(&self, other: &Plane) -> bool {
        (self.size, self.min_col, self.max_col, self.min_row, self.max_row) == (other.size, other.min_col, other.max_col, other.min_row, other.max_row)
    }
}

/// `Structure.Axis.horizontal` and `vertical`: the coordinates a plane of `axis` has its columns and rows on.
fn plane_axes(axis: usize) -> (usize, usize) {
    match axis {
        0 => (2, 1),
        1 => (0, 2),
        _ => (0, 1),
    }
}

fn planes(nodes: &BTreeMap<Pos, Node>, axis: usize) -> BTreeMap<i32, Plane> {
    let (col, row) = plane_axes(axis);
    let mut out: BTreeMap<i32, Plane> = BTreeMap::new();
    for (p, node) in nodes {
        if !node.is_multiblock() {
            continue;
        }
        let plane = out.entry(p[axis]).or_insert(Plane { frame: false, min_col: 0, max_col: 0, min_row: 0, max_row: 0, size: 0 });
        if plane.frame {
            plane.min_col = plane.min_col.min(p[col]);
            plane.max_col = plane.max_col.max(p[col]);
            plane.min_row = plane.min_row.min(p[row]);
            plane.max_row = plane.max_row.max(p[row]);
        } else {
            *plane = Plane { frame: true, min_col: p[col], max_col: p[col], min_row: p[row], max_row: p[row], size: 0 };
        }
        plane.size += 1;
    }
    for (p, node) in nodes {
        if node.is_multiblock() {
            continue;
        }
        let plane = out.entry(p[axis]).or_insert(Plane { frame: false, min_col: 0, max_col: 0, min_row: 0, max_row: 0, size: 0 });
        let inside = plane.frame && (plane.min_col..=plane.max_col).contains(&p[col]) && (plane.min_row..=plane.max_row).contains(&p[row]);
        if inside {
            plane.size += 1;
        }
    }
    out
}

/// `StructureHelper.fetchCuboid(structure, minBounds, maxBounds)`: on each
/// axis the first and the last major plane must be the same full plane, they
/// make the cuboid (sized within the bounds), every axis the same one, and
/// no frame may sit in a minor plane outside it. None: no cuboid (the
/// structure fails without a message).
pub fn fetch_cuboid(nodes: &BTreeMap<Pos, Node>, min: [i32; 3], max: [i32; 3]) -> Option<Cuboid> {
    let mut prev: Option<Cuboid> = None;
    for axis in 0..3 {
        let planes = planes(nodes, axis);
        let mut majors = planes.iter().filter(|(_, p)| p.major());
        let (&first_key, first) = majors.next()?;
        let (&last_key, last) = majors.last().unwrap_or((&first_key, first));
        if !first.same(last) || !first.full() {
            return None;
        }
        let (col, row) = plane_axes(axis);
        let mut lo = [0; 3];
        let mut hi = [0; 3];
        lo[axis] = first_key;
        hi[axis] = last_key;
        lo[col] = first.min_col;
        hi[col] = first.max_col;
        lo[row] = first.min_row;
        hi[row] = first.max_row;
        let cuboid = Cuboid::new(lo, hi);
        match prev {
            None if !(cuboid.at_least(min) && cuboid.at_most(max)) => return None,
            Some(p) if p != cuboid => return None,
            _ => {}
        }
        let outside = planes.iter().any(|(&k, p)| !p.major() && p.frame && (k < first_key || k > last_key));
        if outside {
            return None;
        }
        prev = Some(cuboid);
    }
    prev
}

/// A side of a cuboid (`VoxelCuboid.CuboidSide`): the axis (0 x, 1 y, 2 z) and whether it is the positive face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Side {
    pub axis: usize,
    pub positive: bool,
}

impl Side {
    pub const BOTTOM: Side = Side { axis: 1, positive: false };
    pub const TOP: Side = Side { axis: 1, positive: true };
    pub const NORTH: Side = Side { axis: 2, positive: false };
    pub const SOUTH: Side = Side { axis: 2, positive: true };
    pub const WEST: Side = Side { axis: 0, positive: false };
    pub const EAST: Side = Side { axis: 0, positive: true };
    /// `CuboidSide.values()` order.
    pub const ALL: [Side; 6] = [Self::BOTTOM, Self::TOP, Self::NORTH, Self::SOUTH, Self::WEST, Self::EAST];
}

/// `StructureHelper.fetchCuboid(structure, minBounds, maxBounds, sides, tolerance)`: the cuboid from the outermost
/// major plane of each of `sides` (each setting its own coordinate and, without a conflict, the bounds of the two
/// axes across it), the planes missing at most `tolerance` positions between them, no frame in a minor plane beyond
/// any of them, and the result within the bounds. Fewer than two sides: none.
pub fn fetch_cuboid_sides(nodes: &BTreeMap<Pos, Node>, min: [i32; 3], max: [i32; 3], sides: &[Side], tolerance: i32) -> Option<Cuboid> {
    if sides.len() < 2 {
        return None;
    }
    let mut missing = 0;
    // [face][axis]: what `CuboidBuilder` was set to so far.
    let mut bounds: [[Option<i32>; 3]; 2] = [[None; 3]; 2];
    for side in sides {
        let planes = planes(nodes, side.axis);
        let mut majors = planes.iter().filter(|(_, p)| p.major());
        let (&key, plane) = if side.positive { majors.last()? } else { majors.next()? };
        missing += plane.length() * plane.height() - plane.size;
        if missing > tolerance {
            return None;
        }
        bounds[usize::from(side.positive)][side.axis] = Some(key);
        let (col, row) = plane_axes(side.axis);
        for (positive, axis, value) in [(false, col, plane.min_col), (true, col, plane.max_col), (false, row, plane.min_row), (true, row, plane.max_row)] {
            let slot = &mut bounds[usize::from(positive)][axis];
            if slot.is_some_and(|v| v != value) {
                return None;
            }
            *slot = Some(value);
        }
        if planes.iter().any(|(&k, p)| !p.major() && p.frame && if side.positive { k > key } else { k < key }) {
            return None;
        }
    }
    let lo: [i32; 3] = std::array::from_fn(|i| bounds[0][i].unwrap_or(0));
    let hi: [i32; 3] = std::array::from_fn(|i| bounds[1][i].unwrap_or(0));
    let cuboid = Cuboid::new(lo, hi);
    (cuboid.at_least(min) && cuboid.at_most(max)).then_some(cuboid)
}

/// What the validator finds inside the cuboid at a position
/// (`validateInner`): air, a block the multiblock takes (an
/// `IInternalMultiblock`: an induction cell or provider), anything else, or
/// a position whose chunk isn't there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inner {
    Air,
    Internal,
    Invalid,
    Unloaded,
}

/// Why a structure didn't form: no message (`FormationResult.FAIL`), a bad
/// block in the walls (`MULTIBLOCK_INVALID_FRAME`), or inside (`MULTIBLOCK_INVALID_INNER`), a second controller
/// (`MULTIBLOCK_INVALID_CONTROLLER_CONFLICT`, never ignored) or none (`MULTIBLOCK_INVALID_NO_CONTROLLER`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Fail,
    InvalidFrame(Pos),
    InvalidInner(Pos),
    ControllerConflict(Pos),
    NoController,
}

/// A formed multiblock's shape: its cuboid, the positions of its walls (`locations`), the blocks inside that aren't
/// air (`internalLocations`), and its valves with the side each looks out of (`ValveData`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Formation {
    pub cuboid: Cuboid,
    pub locations: Vec<Pos>,
    pub internal: Vec<Pos>,
    pub valves: Vec<(Pos, Direction)>,
}

/// `CuboidStructureValidator.validate`: every position of the cuboid, x outermost, then y, then z; a wall needs a
/// node of the structure (an edge a frame), the inside what `inner` says passes. The first failure ends it.
pub fn validate(cuboid: Cuboid, nodes: &BTreeMap<Pos, Node>, inner: impl FnMut(Pos) -> Inner) -> Result<Formation, Failure> {
    validate_with(cuboid, nodes, |p| cuboid.requirement(p), |_, _| None, inner)
}

/// `CuboidStructureValidator.validate` with a validator's own `getStructureRequirement` (`requirement`) and a check
/// of its own before `validateFrame`'s (`frame_check`, whose failure counts even where the position is ignored: the
/// evaporation plant's controller conflict).
pub fn validate_with(
    cuboid: Cuboid,
    nodes: &BTreeMap<Pos, Node>,
    requirement: impl Fn(Pos) -> Requirement,
    mut frame_check: impl FnMut(Pos, Option<Node>) -> Option<Failure>,
    mut inner: impl FnMut(Pos) -> Inner,
) -> Result<Formation, Failure> {
    let mut formation = Formation { cuboid, locations: Vec::new(), internal: Vec::new(), valves: Vec::new() };
    for p in cuboid.positions() {
        match requirement(p) {
            Requirement::Inner => match inner(p) {
                Inner::Air => {}
                Inner::Internal => formation.internal.push(p),
                Inner::Invalid => return Err(Failure::InvalidInner(p)),
                Inner::Unloaded => return Err(Failure::Fail),
            },
            requirement => {
                let node = nodes.get(&p).copied();
                if let Some(failure) = frame_check(p, node) {
                    return Err(failure);
                }
                match node {
                    Some(node) if requirement != Requirement::Frame || node == Node::Frame => {
                        formation.locations.push(p);
                        if node == Node::Valve
                            && let Some(side) = cuboid.side_of(p)
                        {
                            formation.valves.push((p, side));
                        }
                    }
                    _ if requirement == Requirement::Ignored => {}
                    _ => return Err(Failure::InvalidFrame(p)),
                }
            }
        }
    }
    Ok(formation)
}

/// `FormationProtocol.doUpdate`'s checks for the structure `nodes` (a structure of glass alone has no controller
/// and fails): the cuboid within `min` and `max`, then every block in it.
pub fn form(nodes: &BTreeMap<Pos, Node>, min: [i32; 3], max: [i32; 3], inner: impl FnMut(Pos) -> Inner) -> Result<Formation, Failure> {
    if !nodes.values().any(|n| n.is_multiblock()) {
        return Err(Failure::Fail);
    }
    let cuboid = fetch_cuboid(nodes, min, max).ok_or(Failure::Fail)?;
    validate(cuboid, nodes, inner)
}

/// Whether a change at `changed` matters to a formed multiblock (`TileEntityMultiblock.onNeighborChange` of its walls,
/// `TileEntityInternalMultiblock`'s of the blocks inside): a position inside the cuboid, beside one of its walls or
/// inner blocks (`known`), that is now air or isn't one of its inner blocks. A change deeper inside goes unseen, as
/// no block of the multiblock hears it.
pub fn inside_change_matters(cuboid: &Cuboid, changed: Pos, air: bool, is_internal: bool, known: impl Fn(Pos) -> bool) -> bool {
    if cuboid.relative(changed) != Relative::Inside || !(air || !is_internal) {
        return false;
    }
    Direction::ALL.iter().any(|d| {
        let o = d.offset();
        known([changed[0] + o[0], changed[1] + o[1], changed[2] + o[2]])
    })
}

/// `StorageUtils.mergeFluidTanks` and `mergeTanks` for one tank of a cache (`Integer.MAX_VALUE` or `Long.MAX_VALUE` of
/// room): `add` of the same contents joins `held` up to `capacity`, the rest is rejected; another fluid or chemical is
/// rejected whole. What the tank then holds and what was rejected.
pub fn merge_amount(held: i64, add: i64, same: bool, capacity: i64) -> (i64, i64) {
    if add <= 0 {
        return (held, 0);
    }
    if held <= 0 {
        return (add.min(capacity), (add - capacity).max(0));
    }
    if !same {
        return (held, add);
    }
    let kept = held.saturating_add(add).min(capacity);
    (kept, add - (kept - held))
}

/// A multiblock's id (`inventory_id`, a `UUID` saved as four ints).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Uuid(pub [i32; 4]);

impl Uuid {
    /// A version 4 id from 128 random bits (`UUID.randomUUID`).
    pub fn random(bits: u128) -> Self {
        let bits = (bits & !(0xF << 76) & !(0x3 << 62)) | (0x4 << 76) | (0x2 << 62);
        Self::from_u128(bits)
    }

    pub fn from_u128(bits: u128) -> Self {
        Self([(bits >> 96) as u32 as i32, (bits >> 64) as u32 as i32, (bits >> 32) as u32 as i32, bits as u32 as i32])
    }

    pub fn as_u128(self) -> u128 {
        self.0.iter().fold(0u128, |a, &w| (a << 32) | u128::from(w as u32))
    }
}

impl std::fmt::Display for Uuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let b = self.as_u128();
        write!(f, "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}", b >> 96, (b >> 80) & 0xFFFF, (b >> 64) & 0xFFFF, (b >> 48) & 0xFFFF, b & 0xFFFF_FFFF_FFFF)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// A hollow box of frames from `min` to `max` (edges and sides alike), as `/fill ... hollow` builds one.
    fn hollow(min: Pos, max: Pos) -> BTreeMap<Pos, Node> {
        let c = Cuboid::new(min, max);
        c.positions().filter(|p| c.relative(*p) == Relative::Walls).map(|p| (p, Node::Frame)).collect()
    }

    fn air(_: Pos) -> Inner {
        Inner::Air
    }

    #[test]
    fn a_hollow_box_forms_from_3_to_18() {
        for n in [3, 4, 7, 18] {
            let nodes = hollow([0, 0, 0], [n - 1, n - 1, n - 1]);
            let f = form(&nodes, MIN_BOUNDS, MAX_BOUNDS, air).unwrap_or_else(|e| panic!("{n}: {e:?}"));
            assert_eq!(f.cuboid.size(), [n, n, n]);
            assert_eq!(f.cuboid.volume(), n * n * n, "the volume is the whole box");
            assert_eq!(f.locations.len(), nodes.len());
            assert!(f.internal.is_empty() && f.valves.is_empty());
        }
        let long = hollow([5, 60, -3], [9, 62, 14]);
        assert_eq!(form(&long, MIN_BOUNDS, MAX_BOUNDS, air).map(|f| f.cuboid.size()), Ok([5, 3, 18]));
    }

    #[test]
    fn too_small_or_too_big_has_no_cuboid() {
        assert_eq!(form(&hollow([0, 0, 0], [2, 1, 2]), MIN_BOUNDS, MAX_BOUNDS, air), Err(Failure::Fail), "3x2x3");
        assert_eq!(form(&hollow([0, 0, 0], [18, 2, 2]), MIN_BOUNDS, MAX_BOUNDS, air), Err(Failure::Fail), "19 long");
        assert_eq!(form(&hollow([0, 0, 0], [1, 1, 1]), MIN_BOUNDS, MAX_BOUNDS, air), Err(Failure::Fail), "2x2x2");
    }

    #[test]
    fn a_hole_in_a_side_leaves_no_cuboid_and_no_message() {
        let mut nodes = hollow([0, 0, 0], [2, 2, 2]);
        nodes.remove(&[1, 2, 1]);
        assert_eq!(form(&nodes, MIN_BOUNDS, MAX_BOUNDS, air), Err(Failure::Fail));
        let mut corner = hollow([0, 0, 0], [3, 3, 3]);
        corner.remove(&[3, 3, 3]);
        assert_eq!(form(&corner, MIN_BOUNDS, MAX_BOUNDS, air), Err(Failure::Fail), "a corner missing");
    }

    #[test]
    fn glass_and_valves_go_on_sides_not_edges() {
        let mut nodes = hollow([0, 0, 0], [3, 3, 3]);
        nodes.insert([1, 3, 1], Node::Structural);
        nodes.insert([2, 3, 2], Node::Structural);
        nodes.insert([0, 1, 2], Node::Valve);
        nodes.insert([2, 0, 1], Node::Valve);
        let f = form(&nodes, MIN_BOUNDS, MAX_BOUNDS, air).expect("glass on the top, valves on the west and the bottom");
        assert_eq!(f.valves, vec![([0, 1, 2], Direction::West), ([2, 0, 1], Direction::Down)]);
        nodes.insert([0, 3, 1], Node::Structural);
        assert_eq!(form(&nodes, MIN_BOUNDS, MAX_BOUNDS, air), Err(Failure::InvalidFrame([0, 3, 1])), "glass on an edge");
        nodes.insert([0, 3, 1], Node::Valve);
        assert_eq!(form(&nodes, MIN_BOUNDS, MAX_BOUNDS, air), Err(Failure::InvalidFrame([0, 3, 1])), "a valve on an edge");
    }

    #[test]
    fn a_side_of_glass_alone_still_counts_inside_its_frame() {
        // A 3x3x3 whose whole top middle is glass: the top plane's frame is its ring, the glass inside it fills it.
        let mut nodes = hollow([0, 0, 0], [2, 2, 2]);
        nodes.insert([1, 2, 1], Node::Structural);
        assert!(form(&nodes, MIN_BOUNDS, MAX_BOUNDS, air).is_ok());
        // Glass sticking out beside the box is part of the structure but no frame: still formed.
        nodes.insert([1, 3, 1], Node::Structural);
        assert!(form(&nodes, MIN_BOUNDS, MAX_BOUNDS, air).is_ok());
        // A casing sticking out is a frame outside the cuboid: no cuboid.
        nodes.insert([1, 3, 1], Node::Frame);
        assert_eq!(form(&nodes, MIN_BOUNDS, MAX_BOUNDS, air), Err(Failure::Fail));
    }

    #[test]
    fn the_inside_must_be_what_the_multiblock_takes() {
        let nodes = hollow([0, 0, 0], [2, 3, 2]);
        let stone = |p: Pos| if p == [1, 2, 1] { Inner::Invalid } else { Inner::Air };
        assert_eq!(form(&nodes, MIN_BOUNDS, MAX_BOUNDS, stone), Err(Failure::InvalidInner([1, 2, 1])));
        let cells = |p: Pos| if p[1] == 1 { Inner::Internal } else { Inner::Air };
        assert_eq!(form(&nodes, MIN_BOUNDS, MAX_BOUNDS, cells).map(|f| f.internal), Ok(vec![[1, 1, 1]]));
        let unloaded = |_: Pos| Inner::Unloaded;
        assert_eq!(form(&nodes, MIN_BOUNDS, MAX_BOUNDS, unloaded), Err(Failure::Fail));
    }

    #[test]
    fn a_wrong_block_in_a_side_leaves_its_plane_short_and_an_edge_names_it() {
        // Stone (no node) in the middle of the west side: that plane isn't full, so no cuboid and no message.
        let mut nodes = hollow([0, 0, 0], [2, 2, 2]);
        nodes.remove(&[0, 1, 1]);
        assert_eq!(form(&nodes, MIN_BOUNDS, MAX_BOUNDS, air), Err(Failure::Fail));
        // A node that fills a plane but may not stand there (glass on an edge) is named.
        let mut edge = hollow([0, 0, 0], [2, 2, 2]);
        edge.insert([1, 0, 0], Node::Structural);
        assert_eq!(form(&edge, MIN_BOUNDS, MAX_BOUNDS, air), Err(Failure::InvalidFrame([1, 0, 0])));
    }

    #[test]
    fn explore_follows_touching_nodes_only() {
        let mut world = hollow([0, 0, 0], [2, 2, 2]);
        world.extend(hollow([10, 0, 0], [12, 2, 2]));
        let found = explore([0, 0, 0], |p| world.get(&p).copied());
        assert_eq!(found.len(), 26);
        assert!(found.keys().all(|p| p[0] <= 2));
        assert!(explore([5, 5, 5], |p| world.get(&p).copied()).is_empty());
        // Two boxes that touch are one structure: its cuboid is both, and the walls between them are inside it.
        let mut touching = hollow([0, 0, 0], [2, 2, 2]);
        touching.extend(hollow([3, 0, 0], [5, 2, 2]));
        let both = explore([0, 0, 0], |p| touching.get(&p).copied());
        assert_eq!(both.len(), 52);
        let blocks = |p: Pos| if touching.contains_key(&p) { Inner::Invalid } else { Inner::Air };
        assert_eq!(form(&both, MIN_BOUNDS, MAX_BOUNDS, blocks), Err(Failure::InvalidInner([2, 1, 1])));
    }

    #[test]
    fn explore_stops_at_5832() {
        let all: BTreeSet<Pos> = (0..20).flat_map(|x| (0..20).flat_map(move |y| (0..20).map(move |z| [x, y, z]))).collect();
        let n = explore([0, 0, 0], |p| all.contains(&p).then_some(Node::Frame)).len();
        assert!((EXPLORE_LIMIT..EXPLORE_LIMIT + 6).contains(&n), "{n}: stops once 5832 were reached");
    }

    #[test]
    fn the_cuboid_knows_its_walls() {
        let c = Cuboid::new([0, 0, 0], [3, 4, 5]);
        assert_eq!(c.requirement([0, 0, 0]), Requirement::Frame, "a corner");
        assert_eq!(c.requirement([1, 0, 0]), Requirement::Frame, "an edge");
        assert_eq!(c.requirement([1, 0, 1]), Requirement::Other, "a side");
        assert_eq!(c.requirement([1, 1, 1]), Requirement::Inner);
        assert_eq!(c.side_of([0, 1, 1]), Some(Direction::West));
        assert_eq!(c.side_of([3, 1, 1]), Some(Direction::East));
        assert_eq!(c.side_of([1, 1, 5]), Some(Direction::South));
        assert_eq!(c.side_of([1, 4, 1]), Some(Direction::Up));
        assert_eq!(c.relative([4, 1, 1]), Relative::Outside);
        assert_eq!(c.relative([3, 1, 1]), Relative::Walls);
        assert_eq!(c.relative([2, 2, 2]), Relative::Inside);
        assert_eq!(c.volume(), 4 * 5 * 6);
    }

    #[test]
    fn a_change_inside_matters_beside_a_wall_or_an_inner_block() {
        let c = Cuboid::new([0, 0, 0], [6, 6, 6]);
        let walls = |p: Pos| c.relative(p) == Relative::Walls;
        assert!(inside_change_matters(&c, [1, 3, 3], false, false, walls), "dirt beside the west wall");
        assert!(!inside_change_matters(&c, [3, 3, 3], false, false, walls), "dirt in the middle: no wall hears it");
        assert!(!inside_change_matters(&c, [7, 3, 3], false, false, walls), "outside");
        assert!(!inside_change_matters(&c, [1, 3, 3], false, true, walls), "a known inner block that stays");
        assert!(inside_change_matters(&c, [1, 3, 3], true, true, walls), "an inner block gone");
    }

    #[test]
    fn caches_merge_like_contents_and_reject_the_rest() {
        assert_eq!(merge_amount(0, 500, false, i64::from(i32::MAX)), (500, 0));
        assert_eq!(merge_amount(300, 500, true, i64::from(i32::MAX)), (800, 0));
        assert_eq!(merge_amount(300, 500, false, i64::from(i32::MAX)), (300, 500), "another fluid is rejected");
        assert_eq!(merge_amount(i64::from(i32::MAX) - 10, 500, true, i64::from(i32::MAX)), (i64::from(i32::MAX), 490));
    }

    #[test]
    fn ids_print_as_java_does() {
        let id = Uuid::from_u128(0x0123_4567_89ab_cdef_0011_2233_4455_6677);
        assert_eq!(id.to_string(), "01234567-89ab-cdef-0011-223344556677");
        assert_eq!(id.0, [0x0123_4567, 0x89ab_cdef_u32 as i32, 0x0011_2233, 0x4455_6677]);
        assert_eq!(Uuid::from_u128(id.as_u128()), id);
        let v4 = Uuid::random(u128::MAX).to_string();
        assert_eq!(&v4[14..15], "4");
        assert!(matches!(&v4[19..20], "8" | "9" | "a" | "b"));
    }
}
