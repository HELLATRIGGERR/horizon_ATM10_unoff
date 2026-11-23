//! `LaserArmBlockEntity` and `LaserArmBlockBehavior`, for blocks: the target search (`basicRaycast`, `trySetNewTarget`,
//! `findNextBlockBreakTarget`), the firing (`fireAtBlock`: power into a block that holds energy, a pass-through or a
//! no-op block ends the target, any other block takes break progress and breaks), the energy of a shot and of a break.
//! Not ported: the hunter addon (shooting entities), the area and silk touch addons, the crop filter, the target
//! designator item and budding amethyst's acceleration (such a block is left alone, not broken).
use crate::{config::LaserSettings, energy::Energy};

/// What the laser makes of the block at a position when it searches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    /// Air, a fluid or a `laser_passthrough` block: the beam goes through.
    Pass,
    Solid,
    /// `minecraft:target`: the search ends there with no target.
    Terminator,
}

/// What `fireAtBlock` does to a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Behavior {
    /// `minecraft:target` and bedrock: nothing, and the block is no target.
    Noop,
    /// A block that holds energy (the atomic forge, anything with Forge Energy): the shot is energy given to it.
    Energy,
    /// `laser_accelerated` (budding blocks): not ported, left alone.
    Accelerated,
    PassThrough,
    /// Break progress.
    Break,
}

/// The level around the laser.
pub trait LaserWorld {
    fn cell(&self, pos: [i32; 3]) -> Cell;
    /// `defaultDestroyTime`: negative for an unbreakable block, 0 for air.
    fn hardness(&self, pos: [i32; 3]) -> f32;
    fn is_air(&self, pos: [i32; 3]) -> bool;
    fn behavior(&self, pos: [i32; 3]) -> Behavior;
    /// A block with energy: `(stored, capacity)`.
    fn energy_of(&self, pos: [i32; 3]) -> Option<(i64, i64)>;
    /// `insertIgnoringLimit`: how much of `amount` the block took.
    fn insert_energy(&mut self, pos: [i32; 3], amount: i64) -> i64;
    /// `finishBlockBreaking`'s world side: the drops into the laser's inventory and the block gone.
    fn break_block(&mut self, pos: [i32; 3]);
}

/// `Math.floor` of a position to a block.
fn block_at(p: [f64; 3]) -> [i32; 3] {
    [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32]
}

fn manhattan(a: [i32; 3], b: [i32; 3]) -> i32 {
    (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs()
}

/// `basicRaycast`: from `from` along `dir` in steps of 0.3 for `range`, the first solid block at the point or (with a
/// search offset) beside it; none at a terminator, or when the first point is clear and there is no offset.
pub fn raycast(world: &dyn LaserWorld, from: [f64; 3], dir: [f64; 3], range: i32, offset: f32) -> Option<[i32; 3]> {
    let off = f64::from(offset);
    let mut i = 0f32;
    while i < range as f32 {
        let to = [from[0] + dir[0] * f64::from(i), from[1] + dir[1] * f64::from(i), from[2] + dir[2] * f64::from(i)];
        let probes: &[[f64; 3]] = &[[0.0, off, 0.0], [0.0, -off, 0.0], [-off, 0.0, 0.0], [off, 0.0, 0.0], [0.0, 0.0, off], [0.0, 0.0, -off]];
        for (n, d) in probes.iter().enumerate() {
            if n > 0 && offset == 0.0 {
                return None;
            }
            let pos = block_at([to[0] + d[0], to[1] + d[1], to[2] + d[2]]);
            match world.cell(pos) {
                Cell::Terminator => return None,
                Cell::Solid => return Some(pos),
                Cell::Pass => {}
            }
        }
        i += 0.3;
    }
    None
}

/// The laser's state (Java's fields): the stored energy, the block it fires at, the direction of its search, the break
/// progress and what the current block costs.
#[derive(Clone, Debug, PartialEq)]
pub struct Laser {
    pub energy: Energy,
    pub target: Option<[i32; 3]>,
    pub direction: Option<[i32; 3]>,
    pub progress: i32,
    pub needed: i32,
    pub last_fired: i64,
    pub disabled_via_redstone: bool,
}

/// What a tick is given.
pub struct LaserCtx<'a> {
    pub settings: &'a LaserSettings,
    pub hardness_exponent: f64,
    /// The block of the laser, its head's block and the game time.
    pub pos: [i32; 3],
    pub head: [i32; 3],
    pub game_time: i64,
    /// `addonData.speed` and `.efficiency` (1 with no addons).
    pub speed: f32,
    pub efficiency: f32,
}

impl Laser {
    pub fn new(energy: Energy) -> Self {
        Self { energy, target: None, direction: None, progress: 0, needed: 1024, last_fired: i64::MIN / 2, disabled_via_redstone: false }
    }

    /// `energyRequiredToFire`.
    pub fn to_fire(ctx: &LaserCtx<'_>) -> i64 {
        (ctx.settings.energy_per_tick as f32 * (1.0 / ctx.speed)) as i32 as i64
    }

    /// `serverTick` (an assembled laser): fires when it holds a shot's energy and is not switched off.
    pub fn tick(&mut self, ctx: &LaserCtx<'_>, world: &mut dyn LaserWorld) -> bool {
        let before = self.clone();
        let shot = Self::to_fire(ctx);
        if !self.disabled_via_redstone && self.energy.amount >= shot {
            if let Some(target) = self.target {
                if self.fire(ctx, world, target, shot) {
                    self.energy.amount -= shot;
                    self.last_fired = ctx.game_time;
                } else {
                    self.find_next(ctx, world);
                }
            } else if self.direction.is_some() && (ctx.game_time + i64::from(ctx.pos[2])) % 40 == 0 {
                self.find_next(ctx, world);
            }
        }
        *self != before
    }

    /// `LaserArmBlockBehavior.fireAtBlock` for the block at `pos`.
    fn fire(&mut self, ctx: &LaserCtx<'_>, world: &mut dyn LaserWorld, pos: [i32; 3], shot: i64) -> bool {
        match world.behavior(pos) {
            Behavior::Noop | Behavior::PassThrough | Behavior::Accelerated => false,
            Behavior::Energy => {
                let Some((stored, capacity)) = world.energy_of(pos) else { return false };
                let room = capacity - stored;
                if room <= 0 || capacity <= 1 {
                    return false;
                }
                let amount = room.min(shot);
                let took = world.insert_energy(pos, amount);
                took > 0 && took <= amount
            }
            Behavior::Break => {
                self.progress += shot as i32;
                if self.progress >= self.needed {
                    self.progress -= self.needed;
                    world.break_block(pos);
                    self.find_next(ctx, world);
                }
                true
            }
        }
    }

    /// `findNextBlockBreakTarget` without the area addon: the first block along the search direction.
    pub fn find_next(&mut self, ctx: &LaserCtx<'_>, world: &dyn LaserWorld) {
        let Some(toward) = self.direction else {
            self.target = None;
            return;
        };
        let head = ctx.head;
        let d = [f64::from(toward[0] - head[0]), f64::from(toward[1] - head[1]), f64::from(toward[2] - head[2])];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if len == 0.0 {
            self.target = None;
            return;
        }
        let dir = [d[0] / len, d[1] / len, d[2] / len];
        let center = [f64::from(head[0]) + 0.5, f64::from(head[1]) + 0.5, f64::from(head[2]) + 0.5];
        let from = [center[0] + dir[0] * 1.5, center[1] + dir[1] * 1.5, center[2] + dir[2] * 1.5];
        self.target = raycast(world, from, dir, ctx.settings.range, 0.45).filter(|p| self.set_target(ctx, world, *p));
    }

    /// `trySetNewTarget` (`alsoSetDirection` false): in range, breakable and not air; the energy a break costs.
    fn set_target(&mut self, ctx: &LaserCtx<'_>, world: &dyn LaserWorld, pos: [i32; 3]) -> bool {
        let hardness = world.hardness(pos);
        if manhattan(pos, ctx.pos) > ctx.settings.range || hardness < 0.0 || world.is_air(pos) {
            return false;
        }
        self.needed = (ctx.settings.block_break_energy_base as f64 * f64::from(hardness).powf(ctx.hardness_exponent) * f64::from(ctx.efficiency)) as i32;
        true
    }

    /// `setTargetFromDesignator`: the search direction and the block itself.
    pub fn designate(&mut self, ctx: &LaserCtx<'_>, world: &dyn LaserWorld, pos: [i32; 3]) -> bool {
        let ok = world.hardness(pos) >= 0.0 && !world.is_air(pos);
        if ok {
            self.direction = Some(pos);
        }
        self.find_next(ctx, world);
        ok
    }
}

/// `BlockPos.asLong`.
pub fn pack(p: [i32; 3]) -> i64 {
    ((i64::from(p[0]) & 0x3FF_FFFF) << 38) | ((i64::from(p[2]) & 0x3FF_FFFF) << 12) | (i64::from(p[1]) & 0xFFF)
}

/// `BlockPos.of`.
pub fn unpack(v: i64) -> [i32; 3] {
    [(v >> 38) as i32, ((v << 52) >> 52) as i32, ((v << 26) >> 38) as i32]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Grid {
        solid: HashMap<[i32; 3], (Cell, f32, Behavior, Option<(i64, i64)>)>,
        broken: Vec<[i32; 3]>,
        gave: Vec<([i32; 3], i64)>,
    }

    impl LaserWorld for Grid {
        fn cell(&self, p: [i32; 3]) -> Cell {
            self.solid.get(&p).map_or(Cell::Pass, |b| b.0)
        }
        fn hardness(&self, p: [i32; 3]) -> f32 {
            self.solid.get(&p).map_or(0.0, |b| b.1)
        }
        fn is_air(&self, p: [i32; 3]) -> bool {
            !self.solid.contains_key(&p)
        }
        fn behavior(&self, p: [i32; 3]) -> Behavior {
            self.solid.get(&p).map_or(Behavior::PassThrough, |b| b.2)
        }
        fn energy_of(&self, p: [i32; 3]) -> Option<(i64, i64)> {
            self.solid.get(&p).and_then(|b| b.3)
        }
        fn insert_energy(&mut self, p: [i32; 3], amount: i64) -> i64 {
            self.gave.push((p, amount));
            if let Some(b) = self.solid.get_mut(&p) {
                if let Some((s, _)) = b.3.as_mut() {
                    *s += amount;
                }
            }
            amount
        }
        fn break_block(&mut self, p: [i32; 3]) {
            self.broken.push(p);
            self.solid.remove(&p);
        }
    }

    fn block(h: f32, b: Behavior) -> (Cell, f32, Behavior, Option<(i64, i64)>) {
        (Cell::Solid, h, b, None)
    }

    fn settings() -> LaserSettings {
        LaserSettings { energy_capacity: 20_000, max_energy_insertion: 1024, energy_per_tick: 128, block_break_energy_base: 1024, range: 128 }
    }

    fn ctx(s: &LaserSettings, time: i64) -> LaserCtx<'_> {
        LaserCtx { settings: s, hardness_exponent: 0.5, pos: [0, 0, 0], head: [0, 0, -1], game_time: time, speed: 1.0, efficiency: 1.0 }
    }

    fn laser(amount: i64) -> Laser {
        let mut l = Laser::new(Energy { amount, capacity: 20_000, max_insert: 1024, max_extract: 0 });
        l.direction = Some([0, 0, -10]);
        l
    }

    #[test]
    fn the_beam_finds_the_first_solid_block_and_stops_at_a_target_block() {
        let mut g = Grid::default();
        g.solid.insert([0, 0, -6], block(1.5, Behavior::Break));
        let s = settings();
        let c = ctx(&s, 0);
        let mut l = laser(1000);
        l.find_next(&c, &g);
        assert_eq!(l.target, Some([0, 0, -6]));
        assert_eq!(l.needed, (1024.0 * 1.5f64.sqrt()) as i32, "base x hardness^0.5");
        g.solid.insert([0, 0, -4], (Cell::Terminator, 0.5, Behavior::Noop, None));
        l.find_next(&c, &g);
        assert_eq!(l.target, None, "a target block ends the search");
    }

    #[test]
    fn a_block_takes_its_energy_then_breaks_and_the_next_is_found() {
        let mut g = Grid::default();
        g.solid.insert([0, 0, -6], block(1.0, Behavior::Break));
        g.solid.insert([0, 0, -9], block(1.0, Behavior::Break));
        let s = settings();
        let mut l = laser(20_000);
        l.find_next(&ctx(&s, 0), &g);
        for t in 0..7 {
            assert!(l.tick(&ctx(&s, t), &mut g));
        }
        assert!(g.broken.is_empty(), "7 x 128 = 896 of 1024");
        l.tick(&ctx(&s, 7), &mut g);
        assert_eq!(g.broken, vec![[0, 0, -6]]);
        assert_eq!(l.energy.amount, 20_000 - 8 * 128);
        assert_eq!((l.target, l.progress), (Some([0, 0, -9]), 0));
    }

    #[test]
    fn energy_goes_to_a_block_that_holds_it_and_stops_when_it_is_full() {
        let mut g = Grid::default();
        g.solid.insert([0, 0, -5], (Cell::Solid, 5.0, Behavior::Energy, Some((0, 300))));
        let s = settings();
        let mut l = laser(1000);
        l.find_next(&ctx(&s, 0), &g);
        l.tick(&ctx(&s, 0), &mut g);
        l.tick(&ctx(&s, 1), &mut g);
        l.tick(&ctx(&s, 2), &mut g);
        assert_eq!(g.gave, vec![([0, 0, -5], 128), ([0, 0, -5], 128), ([0, 0, -5], 44)], "a shot, a shot, what room is left");
        assert_eq!(l.energy.amount, 1000 - 3 * 128);
        l.tick(&ctx(&s, 3), &mut g);
        assert_eq!(g.gave.len(), 3, "full: no more");
    }

    #[test]
    fn without_a_shots_energy_or_with_redstone_it_does_nothing() {
        let mut g = Grid::default();
        g.solid.insert([0, 0, -6], block(1.0, Behavior::Break));
        let s = settings();
        let mut l = laser(100);
        l.find_next(&ctx(&s, 0), &g);
        assert!(!l.tick(&ctx(&s, 0), &mut g));
        l.energy.amount = 5000;
        l.disabled_via_redstone = true;
        assert!(!l.tick(&ctx(&s, 0), &mut g));
    }

    #[test]
    fn a_block_pos_packs_as_java_does() {
        for p in [[0, 0, 0], [5, 64, -3], [-30_000, -64, 29_999], [3, 319, 7]] {
            assert_eq!(unpack(pack(p)), p);
        }
        assert_eq!(pack([1, 2, 3]), (1i64 << 38) | (3i64 << 12) | 2);
    }
}
