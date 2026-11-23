//! MI's multiblock framework (machines/multiblocks/): `ShapeTemplate` with its `Builder` and `LayeredBuilder`,
//! `SimpleMember`, `HatchType`/`HatchFlags`, and `ShapeMatcher`: the template turned to the controller's facing,
//! matched against the world, its hatches found, and what `buildMultiblock` would place. The world is read through
//! [`ShapeWorld`]; the bundle's runtime gives it the game's blocks and block entities.
//!
//! MI keeps a shape's members in `HashMap`s, and its matcher walks them in their iteration order, which decides the
//! order of the matched hatches (and so which hatch's slots come first among hatches at the same height). That order is
//! kept here: [`java_order`] is `HashMap`'s for `BlockPos` keys.

use std::collections::HashMap;

/// A block position, `[x, y, z]`.
pub type Pos = [i32; 3];

/// `HatchTypes`: MI's hatch types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HatchType {
    ItemInput,
    ItemOutput,
    FluidInput,
    FluidOutput,
    EnergyInput,
    EnergyOutput,
    NuclearItem,
    NuclearFluid,
    LargeTank,
}

impl HatchType {
    pub const ALL: [HatchType; 9] = [
        HatchType::ItemInput,
        HatchType::ItemOutput,
        HatchType::FluidInput,
        HatchType::FluidOutput,
        HatchType::EnergyInput,
        HatchType::EnergyOutput,
        HatchType::NuclearItem,
        HatchType::NuclearFluid,
        HatchType::LargeTank,
    ];

    /// Its id's path (`modern_industrialization:item_input`).
    pub fn id(self) -> &'static str {
        match self {
            HatchType::ItemInput => "item_input",
            HatchType::ItemOutput => "item_output",
            HatchType::FluidInput => "fluid_input",
            HatchType::FluidOutput => "fluid_output",
            HatchType::EnergyInput => "energy_input",
            HatchType::EnergyOutput => "energy_output",
            HatchType::NuclearItem => "nuclear_item",
            HatchType::NuclearFluid => "nuclear_fluid",
            HatchType::LargeTank => "large_tank",
        }
    }

    /// `HatchTypes.get(name)`: a bare path is MI's, a full id must be in MI's namespace.
    pub fn of_id(name: &str) -> Option<Self> {
        let path = match name.split_once(':') {
            Some(("modern_industrialization", path)) => path,
            Some(_) => return None,
            None => name,
        };
        Self::ALL.into_iter().find(|t| t.id() == path)
    }

    fn bit(self) -> u16 {
        1 << self as u16
    }
}

/// `HatchFlags`: the hatch types a position of a shape takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HatchFlags(u16);

impl HatchFlags {
    pub const NO_HATCH: HatchFlags = HatchFlags(0);

    pub fn of(types: &[HatchType]) -> Self {
        HatchFlags(types.iter().fold(0, |bits, t| bits | t.bit()))
    }
    pub fn allows(self, t: HatchType) -> bool {
        self.0 & t.bit() != 0
    }
    pub fn values(self) -> Vec<HatchType> {
        HatchType::ALL.into_iter().filter(|t| self.allows(*t)).collect()
    }
}

/// A horizontal direction, as a controller faces (`OrientationComponent.facingDirection`, NORTH by default).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Facing {
    #[default]
    North,
    South,
    West,
    East,
}

impl Facing {
    pub const ALL: [Facing; 4] = [Facing::North, Facing::South, Facing::West, Facing::East];

    /// `Direction.from3DDataValue` as the matcher reads it: 2 north, 3 south, 5 east; anything else falls in
    /// `toWorldPos`'s last branch, west's.
    pub fn from_data_value(v: i32) -> Self {
        match v {
            2 => Facing::North,
            3 => Facing::South,
            5 => Facing::East,
            _ => Facing::West,
        }
    }
    /// `Direction.get3DDataValue`.
    pub fn data_value(self) -> i32 {
        match self {
            Facing::North => 2,
            Facing::South => 3,
            Facing::West => 4,
            Facing::East => 5,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Facing::North => "north",
            Facing::South => "south",
            Facing::West => "west",
            Facing::East => "east",
        }
    }
    pub fn of_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.name() == name)
    }
    /// `Rotation.rotate(Direction)` for a quarter turn clockwise (seen from above): north, east, south, west.
    fn clockwise(self) -> Self {
        match self {
            Facing::North => Facing::East,
            Facing::East => Facing::South,
            Facing::South => Facing::West,
            Facing::West => Facing::North,
        }
    }
}

/// `Rotation`, in quarter turns clockwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rotation(u8);

impl Rotation {
    pub const NONE: Rotation = Rotation(0);
    pub const CLOCKWISE_90: Rotation = Rotation(1);
    pub const CLOCKWISE_180: Rotation = Rotation(2);
    pub const COUNTERCLOCKWISE_90: Rotation = Rotation(3);

    /// `ShapeMatcher.worldRotation`: how a template's block state turns to stand in the world.
    pub fn world(facing: Facing) -> Self {
        match facing {
            Facing::South => Rotation::NONE,
            Facing::North => Rotation::CLOCKWISE_180,
            Facing::East => Rotation::COUNTERCLOCKWISE_90,
            Facing::West => Rotation::CLOCKWISE_90,
        }
    }
    fn facing(self, f: Facing) -> Facing {
        (0..self.0).fold(f, |f, _| f.clockwise())
    }
    /// `BlockState.rotate` for the properties that turn with a block: a horizontal `facing`, an `axis` (x and z
    /// swap on a quarter turn), a sign's `rotation` (16 steps), and the side flags of walls and fences (`north`...).
    pub fn property(self, key: &str, value: &str) -> (String, String) {
        if let Some(side) = Facing::of_name(key) {
            return (self.facing(side).name().to_owned(), value.to_owned());
        }
        let value = match key {
            "facing" | "horizontal_facing" => Facing::of_name(value).map_or(value.to_owned(), |f| self.facing(f).name().to_owned()),
            "axis" if self.0 % 2 == 1 => match value {
                "x" => "z".to_owned(),
                "z" => "x".to_owned(),
                other => other.to_owned(),
            },
            "rotation" => value.parse::<u8>().map_or(value.to_owned(), |r| ((r + 4 * self.0) % 16).to_string()),
            _ => value.to_owned(),
        };
        (key.to_owned(), value)
    }
}

/// What block entity stands at a position, as the matcher asks (`world.getBlockEntity(pos)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockEntityAt {
    None,
    /// A hatch of this type, `matched` when another multiblock has it already (`isMatched`).
    Hatch { kind: HatchType, matched: bool },
    Other,
}

/// The world as a shape reads it.
pub trait ShapeWorld {
    /// The block at `pos` (`minecraft:bricks`).
    fn block(&mut self, pos: Pos) -> String;
    /// A property of the block state at `pos`.
    fn property(&mut self, pos: Pos, key: &str) -> Option<String>;
    /// The block at `pos` is in the block tag `tag` (no `#`).
    fn in_tag(&mut self, pos: Pos, tag: &str) -> bool;
    fn block_entity(&mut self, pos: Pos) -> BlockEntityAt;
}

/// `SimpleMember`: what a position of a shape takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SimpleMember {
    /// `forBlock`/`forBlockId`: any state of the block.
    Block(String),
    /// `forBlockTag`/`forBlockTagId`: any block of the tag, `block` shown in previews.
    BlockTag { block: String, tag: String },
    /// `forBlockState`/`forBlockStateId`: that state, its properties all given.
    State { block: String, properties: Vec<(String, String)> },
    /// `verticalChain`: a chain along y.
    VerticalChain,
}

impl SimpleMember {
    pub fn block(id: &str) -> Self {
        SimpleMember::Block(id.to_owned())
    }

    /// `matchesState(toTemplateState(state))` at `pos` for a controller facing `facing`: the member's state is
    /// turned into the world instead (`worldRotation` undoes `templateRotation`), which compares the same.
    pub fn matches(&self, world: &mut impl ShapeWorld, pos: Pos, facing: Facing) -> bool {
        match self {
            SimpleMember::Block(block) => world.block(pos) == *block,
            SimpleMember::BlockTag { tag, .. } => world.in_tag(pos, tag),
            SimpleMember::State { block, properties } => {
                if world.block(pos) != *block {
                    return false;
                }
                let rotation = Rotation::world(facing);
                properties.iter().all(|(k, v)| {
                    let (k, v) = rotation.property(k, v);
                    world.property(pos, &k).as_deref() == Some(v.as_str())
                })
            }
            SimpleMember::VerticalChain => world.block(pos) == "minecraft:chain" && world.property(pos, "axis").as_deref() == Some("y"),
        }
    }

    /// `getPreviewState` as it stands in the world for a controller facing `facing` (`toWorldState`): the block and
    /// the properties it is placed with.
    pub fn preview(&self, facing: Facing) -> (String, Vec<(String, String)>) {
        match self {
            SimpleMember::Block(block) | SimpleMember::BlockTag { block, .. } => (block.clone(), Vec::new()),
            SimpleMember::State { block, properties } => {
                let rotation = Rotation::world(facing);
                (block.clone(), properties.iter().map(|(k, v)| rotation.property(k, v)).collect())
            }
            SimpleMember::VerticalChain => ("minecraft:chain".to_owned(), vec![("axis".to_owned(), "y".to_owned())]),
        }
    }
}

/// `Vec3i.hashCode`.
fn block_pos_hash(p: Pos) -> i32 {
    p[1].wrapping_add(p[2].wrapping_mul(31)).wrapping_mul(31).wrapping_add(p[0])
}

fn bucket(p: Pos, capacity: usize) -> usize {
    let h = block_pos_hash(p);
    ((h ^ ((h as u32) >> 16) as i32) as u32 as usize) & (capacity - 1)
}

/// The table size a `java.util.HashMap` of `BlockPos` keys has after `ops` (`true` puts the key, `false` removes it):
/// 16 at first, doubled when it holds more than three quarters of its size, and doubled too when a key is put in a
/// bucket that holds 8 already while the table is under 64 (`treeifyBin` resizes rather than make a tree). It never
/// shrinks.
pub fn java_capacity(ops: &[(Pos, bool)]) -> usize {
    let mut capacity = 16usize;
    let mut keys: Vec<Pos> = Vec::new();
    for &(p, put) in ops {
        if !put {
            keys.retain(|k| *k != p);
            continue;
        }
        if keys.contains(&p) {
            continue;
        }
        let b = bucket(p, capacity);
        let in_bucket = keys.iter().filter(|k| bucket(**k, capacity) == b).count();
        keys.push(p);
        if in_bucket >= 8 && capacity < 64 {
            capacity *= 2;
        }
        if keys.len() > capacity / 4 * 3 {
            capacity *= 2;
        }
    }
    capacity
}

/// The order a `java.util.HashMap` of `BlockPos` keys walks them: `keys` in the order they were first put, in a table
/// of `capacity` ([`java_capacity`]). A resize keeps the keys of one bucket in their order, so they walk bucket by
/// bucket, a bucket's keys in the order they were put. Not followed: a bucket that turns into a tree (9 keys in one
/// bucket of a table of 64 or more) walks its root first.
pub fn java_order(keys: &[Pos], capacity: usize) -> Vec<Pos> {
    let mut ordered = keys.to_vec();
    ordered.sort_by_key(|p| bucket(*p, capacity));
    ordered
}

/// `ShapeTemplate`: members at positions relative to the controller (x to its right as it is seen from the front when
/// it faces south, y up, z behind it), the hatches each position takes, and the casing its hatches show once matched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShapeTemplate {
    members: HashMap<Pos, SimpleMember>,
    hatch_flags: HashMap<Pos, HatchFlags>,
    /// `simpleMembers`' keys in the order they were first put.
    order: Vec<Pos>,
    /// What was put in and taken out of `simpleMembers`, in order, for its table's size.
    ops: Vec<(Pos, bool)>,
    pub hatch_casing: String,
}

impl ShapeTemplate {
    fn new(hatch_casing: &str) -> Self {
        ShapeTemplate { members: HashMap::new(), hatch_flags: HashMap::new(), order: Vec::new(), ops: Vec::new(), hatch_casing: hatch_casing.to_owned() }
    }
    /// The members in `simpleMembers`' iteration order.
    pub fn members(&self) -> Vec<(Pos, &SimpleMember)> {
        java_order(&self.order, java_capacity(&self.ops)).into_iter().map(|p| (p, &self.members[&p])).collect()
    }
    pub fn member(&self, pos: Pos) -> Option<&SimpleMember> {
        self.members.get(&pos)
    }
    pub fn hatch_flags(&self, pos: Pos) -> Option<HatchFlags> {
        self.hatch_flags.get(&pos).copied()
    }
    pub fn len(&self) -> usize {
        self.members.len()
    }
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
    fn put(&mut self, pos: Pos, member: SimpleMember, flags: Option<HatchFlags>) {
        if self.members.insert(pos, member).is_none() {
            self.order.push(pos);
        }
        self.ops.push((pos, true));
        if let Some(flags) = flags {
            self.hatch_flags.insert(pos, flags);
        }
    }
    fn take(&mut self, pos: Pos) {
        if self.members.remove(&pos).is_some() {
            self.order.retain(|p| *p != pos);
        }
        self.ops.push((pos, false));
        self.hatch_flags.remove(&pos);
    }
}

/// `ShapeTemplate.Builder`.
pub struct Builder {
    template: ShapeTemplate,
}

impl Builder {
    pub fn new(hatch_casing: &str) -> Self {
        Builder { template: ShapeTemplate::new(hatch_casing) }
    }
    /// `add3by3Levels`: full 3 by 3 layers `min_y..=max_y`, hollow above the first; hatches on the first and last.
    pub fn add3by3_levels(mut self, min_y: i32, max_y: i32, member: &SimpleMember, flags: Option<HatchFlags>) -> Self {
        for y in min_y..=max_y {
            self = self.add3by3(y, member, y != min_y, if y == min_y || y == max_y { flags } else { None });
        }
        self
    }
    /// `add3by3LevelsRoofed`: as `add3by3Levels`, the last layer full too.
    pub fn add3by3_levels_roofed(mut self, min_y: i32, max_y: i32, member: &SimpleMember, flags: Option<HatchFlags>) -> Self {
        for y in min_y..=max_y {
            self = self.add3by3(y, member, y != min_y && y != max_y, if y == min_y || y == max_y { flags } else { None });
        }
        self
    }
    pub fn add(mut self, x: i32, y: i32, z: i32, member: &SimpleMember, flags: Option<HatchFlags>) -> Self {
        self.template.put([x, y, z], member.clone(), flags);
        self
    }
    /// `add3by3`: x -1 to 1, z 0 to 2 at height `y`; `hollow` leaves out the middle.
    pub fn add3by3(mut self, y: i32, member: &SimpleMember, hollow: bool, flags: Option<HatchFlags>) -> Self {
        for x in -1..=1 {
            for z in 0..=2 {
                if hollow && x == 0 && z == 1 {
                    continue;
                }
                self = self.add(x, y, z, member, flags);
            }
        }
        self
    }
    pub fn remove(mut self, x: i32, y: i32, z: i32) -> Self {
        self.template.take([x, y, z]);
        self
    }
    /// `build`: the controller's own position is no member.
    pub fn build(self) -> ShapeTemplate {
        self.remove(0, 0, 0).template
    }
}

/// `ShapeTemplate.LayeredBuilder`: layers of rows of strings, one character a block (`' '` none, `'#'` the controller):
/// layer `i`, row `j`, column `k` lands at `(k - k₀, j - j₀, -(i - i₀))` from the controller's `(i₀, j₀, k₀)`, so rows
/// go up and later layers stand further to the front.
pub struct LayeredBuilder {
    inner: Builder,
    layers: Vec<Vec<Vec<char>>>,
    missing: Vec<char>,
    keys: HashMap<char, (SimpleMember, Option<HatchFlags>)>,
    controller: [usize; 3],
}

impl LayeredBuilder {
    pub fn new(hatch_casing: &str, layers: &[Vec<String>]) -> Result<Self, String> {
        if layers.is_empty() {
            return Err("No layers provided".into());
        }
        if layers[0].is_empty() || layers[0][0].is_empty() {
            return Err("Layer 0 cannot have size 0".into());
        }
        let (dim2, dim3) = (layers[0].len(), layers[0][0].chars().count());
        let mut controller = None;
        let mut missing = Vec::new();
        let mut grid = Vec::new();
        for (i, layer) in layers.iter().enumerate() {
            if layer.len() != dim2 {
                return Err(format!("Layer {i} has invalid size, expected {dim2}"));
            }
            let mut rows = Vec::new();
            for (j, row) in layer.iter().enumerate() {
                let chars: Vec<char> = row.chars().collect();
                if chars.len() != dim3 {
                    return Err(format!("Layer {i} entry {j} has invalid size, expected {dim3}"));
                }
                for (k, c) in chars.iter().enumerate() {
                    if *c == '#' {
                        if controller.is_some() {
                            return Err("Multiple controllers found (character #)".into());
                        }
                        controller = Some([i, j, k]);
                    } else if *c != ' ' && !missing.contains(c) {
                        missing.push(*c);
                    }
                }
                rows.push(chars);
            }
            grid.push(rows);
        }
        // MI leaves the controller at 0, 0, 0 when there is none.
        Ok(LayeredBuilder { inner: Builder::new(hatch_casing), layers: grid, missing, keys: HashMap::new(), controller: controller.unwrap_or([0, 0, 0]) })
    }

    pub fn key(mut self, key: char, member: SimpleMember, flags: Option<HatchFlags>) -> Result<Self, String> {
        if self.keys.contains_key(&key) {
            return Err(format!("Key '{key}' was already defined"));
        }
        let Some(at) = self.missing.iter().position(|c| *c == key) else {
            return Err(format!("Key '{key}' it not part of the shape layers"));
        };
        self.missing.remove(at);
        self.keys.insert(key, (member, flags));
        Ok(self)
    }

    pub fn build(self) -> Result<ShapeTemplate, String> {
        if !self.missing.is_empty() {
            let keys: Vec<String> = self.missing.iter().map(char::to_string).collect();
            return Err(format!("Missing keys: [{}]", keys.join(", ")));
        }
        let [i0, j0, k0] = self.controller.map(|v| v as i32);
        let mut inner = self.inner;
        for (i, layer) in self.layers.iter().enumerate() {
            for (j, row) in layer.iter().enumerate() {
                for (k, c) in row.iter().enumerate() {
                    if *c != ' ' && *c != '#' {
                        let (member, flags) = &self.keys[c];
                        inner = inner.add(k as i32 - k0, j as i32 - j0, -(i as i32 - i0), member, *flags);
                    }
                }
            }
        }
        Ok(inner.build())
    }
}

/// `ShapeMatcher.toWorldPos`: a template position for a controller at `controller` facing `facing`.
pub fn to_world_pos(controller: Pos, facing: Facing, p: Pos) -> Pos {
    let [x, y, z] = p;
    let r = match facing {
        Facing::North => [-x, y, z],
        Facing::South => [x, y, -z],
        Facing::East => [-z, y, -x],
        Facing::West => [z, y, x],
    };
    [r[0] + controller[0], r[1] + controller[1], r[2] + controller[2]]
}

/// What a match found (`ShapeMatcher.rematch`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Match {
    pub ok: bool,
    /// The matched hatches in the order they were found; none when the shape failed.
    pub hatches: Vec<Pos>,
    /// Block entities where the shape wants another block (`ShapeValidComponent.mismatchingBlockEntities`).
    pub mismatching: Vec<Pos>,
    /// The first position that failed, for tooltips and tests.
    pub first_failure: Option<Pos>,
}

/// `ShapeMatcher`: a template bound to a controller's position and facing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShapeMatcher {
    pub controller: Pos,
    pub facing: Facing,
    /// The members at their world positions, in the order MI's matcher walks them.
    pub members: Vec<(Pos, SimpleMember)>,
    hatch_flags: HashMap<Pos, HatchFlags>,
}

impl ShapeMatcher {
    pub fn new(controller: Pos, facing: Facing, template: &ShapeTemplate) -> Self {
        let listed = template.members();
        let world: Vec<Pos> = listed.iter().map(|(p, _)| to_world_pos(controller, facing, *p)).collect();
        let by_pos: HashMap<Pos, SimpleMember> = listed.iter().zip(&world).map(|((_, m), w)| (*w, (*m).clone())).collect();
        let members = java_order(&world, java_capacity(&world.iter().map(|p| (*p, true)).collect::<Vec<_>>())).into_iter().map(|p| (p, by_pos[&p].clone())).collect();
        let hatch_flags = template
            .hatch_flags
            .iter()
            .map(|(p, f)| (to_world_pos(controller, facing, *p), *f))
            .collect();
        ShapeMatcher { controller, facing, members, hatch_flags }
    }

    pub fn positions(&self) -> impl Iterator<Item = Pos> + '_ {
        self.members.iter().map(|(p, _)| *p)
    }

    pub fn hatch_flags(&self, pos: Pos) -> Option<HatchFlags> {
        self.hatch_flags.get(&pos).copied()
    }

    /// `rematch` (without the unlinking before and the linking after, which the caller does): every member is
    /// looked at, a hatch first (its type allowed here and no other multiblock's), then the block.
    pub fn rematch(&self, world: &mut impl ShapeWorld) -> Match {
        let mut m = Match { ok: true, ..Match::default() };
        for (pos, member) in &self.members {
            let entity = world.block_entity(*pos);
            if let BlockEntityAt::Hatch { kind, matched: false } = entity
                && self.hatch_flags(*pos).is_some_and(|f| f.allows(kind))
            {
                m.hatches.push(*pos);
                continue;
            }
            let fits = member.matches(world, *pos, self.facing);
            if entity != BlockEntityAt::None && !fits {
                m.mismatching.push(*pos);
            }
            if !fits {
                m.ok = false;
                m.first_failure.get_or_insert(*pos);
            }
        }
        if !m.ok {
            m.hatches.clear();
        }
        m
    }

    /// `buildMultiblock`'s list: each member not matched yet, with the block and properties it would be placed as.
    pub fn missing(&self, world: &mut impl ShapeWorld) -> Vec<(Pos, String, Vec<(String, String)>)> {
        self.members
            .iter()
            .filter(|(pos, member)| !member.matches(world, *pos, self.facing))
            .map(|(pos, member)| {
                let (block, properties) = member.preview(self.facing);
                (*pos, block, properties)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A world of blocks with properties and block entities.
    #[derive(Default)]
    struct Blocks {
        blocks: HashMap<Pos, (String, Vec<(String, String)>)>,
        entities: HashMap<Pos, BlockEntityAt>,
        tags: HashMap<String, Vec<String>>,
    }
    impl Blocks {
        fn set(&mut self, p: Pos, block: &str) {
            self.blocks.insert(p, (block.to_owned(), Vec::new()));
        }
    }
    impl ShapeWorld for Blocks {
        fn block(&mut self, pos: Pos) -> String {
            self.blocks.get(&pos).map_or("minecraft:air".to_owned(), |b| b.0.clone())
        }
        fn property(&mut self, pos: Pos, key: &str) -> Option<String> {
            self.blocks.get(&pos)?.1.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
        }
        fn in_tag(&mut self, pos: Pos, tag: &str) -> bool {
            let block = self.block(pos);
            self.tags.get(tag).is_some_and(|t| t.contains(&block))
        }
        fn block_entity(&mut self, pos: Pos) -> BlockEntityAt {
            self.entities.get(&pos).copied().unwrap_or(BlockEntityAt::None)
        }
    }

    fn coke_oven() -> ShapeTemplate {
        let flags = HatchFlags::of(&[HatchType::ItemInput, HatchType::ItemOutput, HatchType::FluidInput, HatchType::FluidOutput]);
        Builder::new("bricks").add3by3_levels(-1, 1, &SimpleMember::block("minecraft:bricks"), Some(flags)).build()
    }

    #[test]
    fn builders_place_members_and_hatches_as_mi() {
        // The coke oven: 3 by 3 by 3 of bricks, hollow above the bottom, the controller in the middle layer's front.
        let t = coke_oven();
        assert_eq!(t.len(), 9 + 7 + 8, "the middle layer's front middle is the controller");
        assert!(t.member([0, 0, 0]).is_none(), "the controller's own position");
        assert!(t.member([0, 0, 1]).is_none() && t.member([0, 1, 1]).is_none(), "hollow");
        assert!(t.member([0, -1, 1]).is_some());
        assert!(t.hatch_flags([1, -1, 2]).unwrap().allows(HatchType::FluidOutput));
        assert_eq!(t.hatch_flags([1, 0, 2]), None, "no hatch in the middle layer");
        assert!(t.hatch_flags([-1, 1, 0]).is_some());
        // add3by3LevelsRoofed closes the top.
        let roofed = Builder::new("c").add3by3_levels_roofed(0, 2, &SimpleMember::block("a:b"), None).build();
        assert!(roofed.member([0, 2, 1]).is_some() && roofed.member([0, 1, 1]).is_none());
        // add without flags keeps the flags already there, as HashMap.put is only called with some.
        let kept = Builder::new("c").add(1, 0, 0, &SimpleMember::block("a:b"), Some(HatchFlags::of(&[HatchType::EnergyInput]))).add(1, 0, 0, &SimpleMember::block("a:c"), None).build();
        assert_eq!(kept.member([1, 0, 0]), Some(&SimpleMember::block("a:c")));
        assert!(kept.hatch_flags([1, 0, 0]).unwrap().allows(HatchType::EnergyInput));
        assert_eq!(HatchType::of_id("modern_industrialization:energy_input"), Some(HatchType::EnergyInput));
        assert_eq!(HatchType::of_id("other:energy_input"), None);
        assert_eq!(HatchFlags::NO_HATCH.values(), vec![]);
    }

    #[test]
    fn layered_builder_reads_layers_as_mi() {
        // Two layers along z: the front one holds the controller.
        let layers = vec![vec!["aaa".to_owned(), "a#a".to_owned()], vec!["bbb".to_owned(), "bhb".to_owned()]];
        let t = LayeredBuilder::new("c", &layers)
            .unwrap()
            .key('a', SimpleMember::block("x:a"), None)
            .unwrap()
            .key('b', SimpleMember::block("x:b"), None)
            .unwrap()
            .key('h', SimpleMember::block("x:b"), Some(HatchFlags::of(&[HatchType::ItemInput])))
            .unwrap()
            .build()
            .unwrap();
        // Layer 0, row 0 is y -1 (rows go up from the controller's row 1), column k is x k - 1; layer 1 is z -1.
        assert_eq!(t.member([-1, -1, 0]), Some(&SimpleMember::block("x:a")));
        assert_eq!(t.member([1, 0, 0]), Some(&SimpleMember::block("x:a")));
        assert_eq!(t.member([0, 0, -1]), Some(&SimpleMember::block("x:b")));
        assert!(t.hatch_flags([0, 0, -1]).unwrap().allows(HatchType::ItemInput));
        assert_eq!(t.len(), 11);
        let bad = |layers: Vec<Vec<&str>>| {
            let layers: Vec<Vec<String>> = layers.into_iter().map(|l| l.into_iter().map(str::to_owned).collect()).collect();
            LayeredBuilder::new("c", &layers).err()
        };
        assert_eq!(bad(vec![vec!["a#", "aa"], vec!["a"]]).as_deref(), Some("Layer 1 has invalid size, expected 2"));
        assert_eq!(bad(vec![vec!["##"]]).as_deref(), Some("Multiple controllers found (character #)"));
        let partial = LayeredBuilder::new("c", &[vec!["a#b".to_owned()]]).unwrap().key('a', SimpleMember::block("x:a"), None).unwrap();
        assert_eq!(partial.build().err().as_deref(), Some("Missing keys: [b]"));
        let unknown = LayeredBuilder::new("c", &[vec!["a#".to_owned()]]).unwrap().key('z', SimpleMember::block("x:a"), None).err();
        assert_eq!(unknown.as_deref(), Some("Key 'z' it not part of the shape layers"));
    }

    #[test]
    fn world_positions_turn_with_the_controller() {
        let c = [100, 64, 200];
        // One block behind the controller and one to its template right.
        assert_eq!(to_world_pos(c, Facing::North, [0, 0, 1]), [100, 64, 201]);
        assert_eq!(to_world_pos(c, Facing::South, [0, 0, 1]), [100, 64, 199]);
        assert_eq!(to_world_pos(c, Facing::East, [0, 0, 1]), [99, 64, 200]);
        assert_eq!(to_world_pos(c, Facing::West, [0, 0, 1]), [101, 64, 200]);
        assert_eq!(to_world_pos(c, Facing::North, [1, 2, 0]), [99, 66, 200]);
        assert_eq!(to_world_pos(c, Facing::East, [1, 0, 0]), [100, 64, 199]);
        assert_eq!(Facing::from_data_value(5), Facing::East);
        assert_eq!(Facing::from_data_value(1), Facing::West, "up falls in toWorldPos' last branch");
        // A stair facing north in the template faces south for a controller facing north (a half turn).
        assert_eq!(Rotation::world(Facing::North).property("facing", "north"), ("facing".into(), "south".into()));
        assert_eq!(Rotation::world(Facing::East).property("facing", "north"), ("facing".into(), "west".into()));
        assert_eq!(Rotation::world(Facing::West).property("axis", "x"), ("axis".into(), "z".into()));
        assert_eq!(Rotation::world(Facing::West).property("north", "true"), ("east".into(), "true".into()));
        assert_eq!(Rotation::world(Facing::East).property("rotation", "2"), ("rotation".into(), "14".into()));
    }

    #[test]
    fn matches_in_every_facing_and_finds_hatches() {
        let t = coke_oven();
        for facing in Facing::ALL {
            let c = [10, 70, -5];
            let matcher = ShapeMatcher::new(c, facing, &t);
            let mut w = Blocks::default();
            for (p, _) in &matcher.members {
                w.set(*p, "minecraft:bricks");
            }
            assert_eq!(matcher.rematch(&mut w), Match { ok: true, ..Match::default() }, "{facing:?}");
            // A hatch where hatches go is matched; where none go it is a mismatching block entity.
            let bottom = to_world_pos(c, facing, [1, -1, 2]);
            let middle = to_world_pos(c, facing, [1, 0, 2]);
            w.set(bottom, "modern_industrialization:steel_item_input_hatch");
            w.entities.insert(bottom, BlockEntityAt::Hatch { kind: HatchType::ItemInput, matched: false });
            let m = matcher.rematch(&mut w);
            assert!(m.ok && m.hatches == vec![bottom], "{facing:?}: {m:?}");
            w.set(middle, "modern_industrialization:steel_item_input_hatch");
            w.entities.insert(middle, BlockEntityAt::Hatch { kind: HatchType::ItemInput, matched: false });
            let m = matcher.rematch(&mut w);
            assert_eq!((m.ok, m.hatches.len(), m.mismatching.clone()), (false, 0, vec![middle]), "{facing:?}");
            // A hatch another multiblock holds, or of a type not allowed, doesn't fit.
            w.set(middle, "minecraft:bricks");
            w.entities.remove(&middle);
            w.entities.insert(bottom, BlockEntityAt::Hatch { kind: HatchType::ItemInput, matched: true });
            assert!(!matcher.rematch(&mut w).ok);
            w.entities.insert(bottom, BlockEntityAt::Hatch { kind: HatchType::EnergyInput, matched: false });
            assert_eq!(matcher.rematch(&mut w).first_failure, Some(bottom));
            // Turned the wrong way, the same blocks don't fit.
            let other = ShapeMatcher::new(c, if facing == Facing::North { Facing::East } else { Facing::North }, &t);
            w.entities.clear();
            w.set(bottom, "minecraft:bricks");
            assert!(!other.rematch(&mut w).ok);
            assert_eq!(matcher.missing(&mut w), vec![]);
        }
    }

    #[test]
    fn states_tags_and_chains() {
        let stair = SimpleMember::State { block: "a:stairs".into(), properties: vec![("facing".into(), "north".into()), ("half".into(), "bottom".into())] };
        let t = Builder::new("c")
            .add(0, 0, 1, &stair, None)
            .add(1, 0, 1, &SimpleMember::BlockTag { block: "a:log".into(), tag: "minecraft:logs".into() }, None)
            .add(-1, 0, 1, &SimpleMember::VerticalChain, None)
            .build();
        let c = [0, 0, 0];
        let m = ShapeMatcher::new(c, Facing::East, &t);
        let mut w = Blocks::default();
        w.tags.insert("minecraft:logs".into(), vec!["b:log".into()]);
        let at = |p| to_world_pos(c, Facing::East, p);
        w.blocks.insert(at([0, 0, 1]), ("a:stairs".into(), vec![("facing".into(), "west".into()), ("half".into(), "bottom".into())]));
        w.set(at([1, 0, 1]), "b:log");
        w.blocks.insert(at([-1, 0, 1]), ("minecraft:chain".into(), vec![("axis".into(), "x".into())]));
        let r = m.rematch(&mut w);
        assert_eq!((r.ok, r.first_failure), (false, Some(at([-1, 0, 1]))));
        let missing = m.missing(&mut w);
        assert_eq!(missing, vec![(at([-1, 0, 1]), "minecraft:chain".to_owned(), vec![("axis".to_owned(), "y".to_owned())])]);
        w.blocks.insert(at([-1, 0, 1]), ("minecraft:chain".into(), vec![("axis".into(), "y".into())]));
        assert!(m.rematch(&mut w).ok);
        // The stair as the template has it (north) is wrong for a controller facing east.
        w.blocks.insert(at([0, 0, 1]), ("a:stairs".into(), vec![("facing".into(), "north".into()), ("half".into(), "bottom".into())]));
        assert!(!m.rematch(&mut w).ok);
        assert_eq!(stair.preview(Facing::East).1[0], ("facing".to_owned(), "west".to_owned()));
    }

    #[test]
    fn walk_order_is_java_hashmaps() {
        // BlockPos(0, 0, 1).hashCode() = 31 * 31 = 961: bucket 961 & 15 = 1; (1, 0, 0) is 1, bucket 1 too, so the
        // first put walks first; (0, 1, 0) is 31: bucket 15.
        let keys = [[0, 1, 0], [0, 0, 1], [1, 0, 0]];
        let puts = |keys: &[Pos]| keys.iter().map(|p| (*p, true)).collect::<Vec<_>>();
        assert_eq!(java_capacity(&puts(&keys)), 16);
        assert_eq!(java_order(&keys, 16), vec![[0, 0, 1], [1, 0, 0], [0, 1, 0]]);
        // 13 keys grow the table to 32 (12 is 16's threshold): (0, 0, 1) goes to bucket 961 & 31 = 1, (16, 0, 0) to 16.
        let many: Vec<Pos> = (0..12).map(|i| [i + 4, 0, 0]).chain([[0, 0, 1]]).collect();
        assert_eq!(java_capacity(&puts(&many)), 32);
        let order = java_order(&many, 32);
        assert_eq!(order[0], [0, 0, 1]);
        assert_eq!(order.last(), Some(&[15, 0, 0]));
        // Taking keys out never shrinks the table; 12 keys stay at 16.
        let mut ops = puts(&many);
        ops.push(([0, 0, 1], false));
        assert_eq!(java_capacity(&ops), 32);
        assert_eq!(java_capacity(&puts(&many[..12])), 16);
        // Nine keys in one bucket of 16 (x a multiple of 16, the rest 0) double it before the threshold does.
        let bin: Vec<Pos> = (0..9).map(|i| [16 * i, 0, 0]).collect();
        assert_eq!(java_capacity(&puts(&bin[..8])), 16);
        assert_eq!(java_capacity(&puts(&bin)), 32);
        // A negative hash: (-2, 0, 0) is -2, spread to 0xFFFF0001, bucket 1.
        assert_eq!(block_pos_hash([-2, 0, 0]), -2);
        assert_eq!(java_order(&[[2, 0, 0], [-2, 0, 0]], 16), vec![[-2, 0, 0], [2, 0, 0]]);
    }
}
