//! Entity fluid immersion trackers and current accumulation across update ticks.
use crate::fluid::{FluidKind, FluidState};
use horizon_core::{
    geometry::{Aabb, Vec3},
    position::BlockPos,
};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FluidTagId(pub u32);
pub trait FluidMembership {
    fn fluid_in_tag(&self, fluid: FluidKind, tag: FluidTagId) -> bool;
}
#[derive(Clone, Copy, Debug)]
pub struct FluidTracker {
    pub fluid: FluidKind,
    pub height: f64,
    pub eyes_inside: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct CurrentAccumulator {
    pub height: f64,
    pub accumulated: Vec3,
    pub count: i32,
}
impl Default for CurrentAccumulator {
    fn default() -> Self {
        Self {
            height: 0.0,
            accumulated: Vec3::ZERO,
            count: 0,
        }
    }
}
pub trait CurrentTarget {
    type Error;
    fn is_player_class(&self) -> bool;
    fn velocity(&mut self) -> Result<Vec3, Self::Error>;
    /// Entity.addDeltaMovement, including finite-addition filtering and a fresh
    /// velocity read. Entity-specific overrides remain the provider's dispatch.
    fn add_delta_movement(&mut self, delta: Vec3) -> Result<(), Self::Error>;
}
impl CurrentAccumulator {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn accumulate(&mut self, current: Vec3) {
        self.accumulated = add(self.accumulated, current);
        self.count = self.count.wrapping_add(1);
    }
    pub fn apply_to<W: CurrentTarget>(&self, world: &mut W, speed: f64) -> Result<(), W::Error> {
        if self.count == 0 || self.accumulated.length_squared() < f64::from(1.0e-5f32) {
            return Ok(());
        }
        let mut current = if world.is_player_class() {
            self.accumulated.scale(1.0 / f64::from(self.count))
        } else {
            self.accumulated.normalize()
        };
        let velocity = world.velocity()?;
        current = current.scale(speed);
        if velocity.x.abs() < 0.003
            && velocity.z.abs() < 0.003
            && current.length() < 0.0045000000000000005
        {
            current = current.normalize().scale(0.0045000000000000005);
        }
        world.add_delta_movement(current)
    }
}
pub struct FluidSections<'a> {
    pub minimum_section_y: i32,
    pub has_fluid: &'a [bool],
}
pub trait FluidInteractionWorld: FluidMembership {
    type Error;
    type Chunk: Copy;
    fn interaction_box(&mut self) -> Result<Option<Aabb>, Self::Error>;
    /// FULL chunk lookup without generating/loading absent chunks.
    fn loaded_chunk(&mut self, x: i32, z: i32) -> Result<Option<Self::Chunk>, Self::Error>;
    fn chunk_sections(&mut self, chunk: Self::Chunk) -> Result<FluidSections<'_>, Self::Error>;
    fn bounding_box_min_y(&self) -> f64;
    fn block_x(&self) -> i32;
    fn eye_y(&mut self) -> Result<f64, Self::Error>;
    fn block_z(&self) -> i32;
    fn fluid_state(&mut self, position: BlockPos) -> Result<FluidState, Self::Error>;
    fn fluid_height(&mut self, state: FluidState, position: BlockPos) -> Result<f32, Self::Error>;
    fn camera_height(&mut self, state: FluidState, position: BlockPos) -> Result<f32, Self::Error>;
    fn flow(&mut self, state: FluidState, position: BlockPos) -> Result<Vec3, Self::Error>;
}
#[derive(Debug)]
pub enum FluidInteractionError<E> {
    World(E),
    NonFiniteBounds,
    BudgetExceeded,
}
impl<E: fmt::Display> fmt::Display for FluidInteractionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::World(e) => e.fmt(f),
            Self::NonFiniteBounds => f.write_str("nonfinite immersion bounds"),
            Self::BudgetExceeded => f.write_str("immersion query budget exceeded"),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for FluidInteractionError<E> {}
fn spend<E>(budget: &mut usize) -> Result<(), FluidInteractionError<E>> {
    if *budget == 0 {
        return Err(FluidInteractionError::BudgetExceeded);
    }
    *budget -= 1;
    Ok(())
}
fn add(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x + b.x, a.y + b.y, a.z + b.z)
}
fn java_max(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        a
    } else if b.is_nan() {
        b
    } else if a == 0.0 && b == 0.0 {
        f64::from_bits(a.to_bits() & b.to_bits())
    } else {
        a.max(b)
    }
}
/// Every required chunk must be loaded, even after a wet section is found.
/// Chunk order is Z then X; section order is ascending Y without early success.
pub fn has_fluid_and_loaded<W: FluidInteractionWorld>(
    world: &mut W,
    min: BlockPos,
    max: BlockPos,
    budget: &mut usize,
) -> Result<bool, FluidInteractionError<W::Error>> {
    let mut present = false;
    for z in (min.z >> 4)..=(max.z >> 4) {
        for x in (min.x >> 4)..=(max.x >> 4) {
            spend(budget)?;
            let Some(chunk) = world
                .loaded_chunk(x, z)
                .map_err(FluidInteractionError::World)?
            else {
                return Ok(false);
            };
            let sections = world
                .chunk_sections(chunk)
                .map_err(FluidInteractionError::World)?;
            for y in (min.y >> 4)..=(max.y >> 4) {
                spend(budget)?;
                let index = y.wrapping_sub(sections.minimum_section_y);
                if index >= 0 && (index as usize) < sections.has_fluid.len() {
                    present |= sections.has_fluid[index as usize];
                }
            }
        }
    }
    Ok(present)
}
#[derive(Clone, Default)]
pub struct EntityFluidInteraction {
    trackers: Vec<FluidTracker>,
    currents: Vec<(FluidTagId, CurrentAccumulator)>,
    /// NeoForge's pushes by `FluidType` for the content mods' fluids
    /// (`Entity.updateFluidHeightAndDoFluidPushing`'s `interimCalcs`): one a
    /// mod fluid, by its source, in the order the box met them; only those
    /// whose type `canPushEntity`.
    type_currents: Vec<(FluidKind, CurrentAccumulator)>,
}
/// Which current a fluid block adds its flow to.
#[derive(Clone, Copy)]
enum Current {
    Tag(usize),
    Type(usize),
}
impl EntityFluidInteraction {
    /// Preserve the caller's tag-set iteration order: a fluid's current belongs
    /// to the first matching tag, even when several tags contain that fluid.
    pub fn new(tags: impl IntoIterator<Item = FluidTagId>) -> Self {
        let mut state = Self::default();
        for tag in tags {
            if !state.currents.iter().any(|(t, _)| *t == tag) {
                state.currents.push((tag, CurrentAccumulator::default()));
            }
        }
        state
    }
    pub fn trackers(&self) -> &[FluidTracker] {
        &self.trackers
    }
    pub fn currents(&self) -> &[(FluidTagId, CurrentAccumulator)] {
        &self.currents
    }
    pub fn fluid_height<W: FluidMembership>(&self, tags: &W, tag: FluidTagId) -> f64 {
        let mut height = 0.0;
        for t in &self.trackers {
            if t.height > height && tags.fluid_in_tag(t.fluid, tag) {
                height = t.height;
            }
        }
        height
    }
    pub fn is_in_fluid<W: FluidMembership>(&self, tags: &W, tag: FluidTagId) -> bool {
        self.fluid_height(tags, tag) > 0.0
    }
    pub fn is_eye_in_fluid<W: FluidMembership>(&self, tags: &W, tag: FluidTagId) -> bool {
        self.trackers
            .iter()
            .any(|t| t.eyes_inside && tags.fluid_in_tag(t.fluid, tag))
    }
    pub fn apply_current<W: CurrentTarget>(
        &self,
        world: &mut W,
        tag: FluidTagId,
        speed: f64,
    ) -> Result<(), W::Error> {
        if let Some((_, current)) = self.currents.iter().find(|(t, _)| *t == tag) {
            current.apply_to(world, speed)?;
        }
        Ok(())
    }
    /// NeoForge's `forgeFluidTypeHeight` for the content mods' fluids: each
    /// mod fluid the box is in (by its source: one `FluidType` for its source
    /// and flowing fluid) with its height, in the order the box met them.
    /// Vanilla's water and lava stay the water and lava trackers'
    /// (`NeoForgeMod.WATER_TYPE`, `LAVA_TYPE`: water by identity, not by tag).
    pub fn mod_fluid_types(&self) -> Vec<(FluidKind, f64)> {
        let mut out: Vec<(FluidKind, f64)> = Vec::new();
        for t in &self.trackers {
            if t.height <= 0.0 || t.fluid.mod_fluid().is_none() {
                continue;
            }
            let source = t.fluid.source();
            match out.iter_mut().find(|(k, _)| *k == source) {
                Some((_, h)) => *h = java_max(*h, t.height),
                None => out.push((source, t.height)),
            }
        }
        out
    }
    /// `Entity.getMaxHeightFluidType` among the content mods' fluids.
    pub fn max_height_mod_fluid(&self) -> Option<(FluidKind, f64)> {
        self.mod_fluid_types()
            .into_iter()
            .fold(None, |best, (k, h)| match best {
                Some((_, b)) if b >= h => best,
                _ => Some((k, h)),
            })
    }
    /// `Entity.getEyeInFluidType` when it is a content mod's fluid: its source.
    pub fn eye_in_mod_fluid(&self) -> Option<FluidKind> {
        self.trackers
            .iter()
            .find(|t| t.eyes_inside && t.fluid.mod_fluid().is_some())
            .map(|t| t.fluid.source())
    }
    /// NeoForge's push of each content mod's fluid type the box is in, by its
    /// own `FluidType.motionScale` (`Entity.updateFluidHeightAndDoFluidPushing`).
    pub fn apply_mod_currents<W: CurrentTarget>(&self, world: &mut W) -> Result<(), W::Error> {
        for (kind, current) in &self.type_currents {
            if let Some(m) = kind.mod_fluid() {
                current.apply_to(world, m.entity.motion_scale)?;
            }
        }
        Ok(())
    }
    /// Reset previous tick state before any query. Provider failures and budget
    /// exhaustion retain all preceding tracker/current writes.
    pub fn update<W: FluidInteractionWorld>(
        &mut self,
        world: &mut W,
        ignore_current: bool,
        mut budget: usize,
    ) -> Result<bool, FluidInteractionError<W::Error>> {
        self.trackers.retain_mut(|t| {
            if t.height == 0.0 && !t.eyes_inside {
                return false;
            }
            t.height = 0.0;
            t.eyes_inside = false;
            true
        });
        for (_, c) in &mut self.currents {
            c.reset();
        }
        self.type_currents.clear();
        let Some(bounds) = world
            .interaction_box()
            .map_err(FluidInteractionError::World)?
        else {
            return Ok(false);
        };
        let a = bounds.min_position();
        let b = bounds.max_position();
        if [a.x, a.y, a.z, b.x, b.y, b.z]
            .iter()
            .any(|v| !v.is_finite())
        {
            return Err(FluidInteractionError::NonFiniteBounds);
        }
        let low = BlockPos::new(a.x.floor() as i32, a.y.floor() as i32, a.z.floor() as i32);
        let high = BlockPos::new(
            (b.x.ceil() as i32).wrapping_sub(1),
            (b.y.ceil() as i32).wrapping_sub(1),
            (b.z.ceil() as i32).wrapping_sub(1),
        );
        if !has_fluid_and_loaded(
            world,
            BlockPos::new(low.x.wrapping_sub(1), low.y, low.z.wrapping_sub(1)),
            BlockPos::new(high.x.wrapping_add(1), high.y, high.z.wrapping_add(1)),
            &mut budget,
        )? {
            return Ok(false);
        }
        let base_y = world.bounding_box_min_y();
        let block_x = world.block_x();
        let eye = world.eye_y().map_err(FluidInteractionError::World)?;
        let block_z = world.block_z();
        let mut last: Option<(FluidKind, usize, Option<Current>)> = None;
        for x in low.x..=high.x {
            for y in low.y..=high.y {
                for z in low.z..=high.z {
                    spend(&mut budget)?;
                    let p = BlockPos::new(x, y, z);
                    let state = world.fluid_state(p).map_err(FluidInteractionError::World)?;
                    if state.kind() == FluidKind::Empty {
                        continue;
                    }
                    let surface = f64::from(y)
                        + f64::from(
                            world
                                .fluid_height(state, p)
                                .map_err(FluidInteractionError::World)?,
                        );
                    if surface < a.y {
                        continue;
                    }
                    let kind = state.kind();
                    let (_, index, current) = if let Some(last) =
                        last.filter(|(k, _, _)| *k == kind)
                    {
                        last
                    } else {
                        let index =
                            if let Some(i) = self.trackers.iter().position(|t| t.fluid == kind) {
                                i
                            } else {
                                self.trackers.push(FluidTracker {
                                    fluid: kind,
                                    height: 0.0,
                                    eyes_inside: false,
                                });
                                self.trackers.len() - 1
                            };
                        let current = if ignore_current {
                            None
                        } else if let Some(m) = kind.mod_fluid() {
                            // `isPushedByFluid(FluidType)`: and its type pushes.
                            m.entity.can_push.then(|| {
                                let source = kind.source();
                                Current::Type(
                                    match self.type_currents.iter().position(|(k, _)| *k == source)
                                    {
                                        Some(i) => i,
                                        None => {
                                            self.type_currents
                                                .push((source, CurrentAccumulator::default()));
                                            self.type_currents.len() - 1
                                        }
                                    },
                                )
                            })
                        } else {
                            self.currents
                                .iter()
                                .position(|(tag, _)| world.fluid_in_tag(kind, *tag))
                                .map(Current::Tag)
                        };
                        let entry = (kind, index, current);
                        last = Some(entry);
                        entry
                    };
                    if x == block_x && z == block_z && eye >= f64::from(y) {
                        let top = f64::from(y)
                            + f64::from(
                                world
                                    .camera_height(state, p)
                                    .map_err(FluidInteractionError::World)?,
                            );
                        if eye <= top {
                            self.trackers[index].eyes_inside = true;
                        }
                    }
                    self.trackers[index].height =
                        java_max(surface - base_y, self.trackers[index].height);
                    if let Some(current) = current {
                        let mut flow =
                            world.flow(state, p).map_err(FluidInteractionError::World)?;
                        let c = match current {
                            Current::Tag(i) => &mut self.currents[i].1,
                            Current::Type(i) => &mut self.type_currents[i].1,
                        };
                        c.height = java_max(self.trackers[index].height, c.height);
                        if c.height < 0.4 {
                            flow = flow.scale(c.height);
                        }
                        c.accumulate(flow);
                    }
                }
            }
        }
        Ok(last.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A body the accumulator pushes: the player flag and the velocity it
    /// reads are the only things `apply_to` asks for.
    struct Body {
        player: bool,
        velocity: Vec3,
        added: Vec3,
    }
    impl CurrentTarget for Body {
        type Error = std::convert::Infallible;
        fn is_player_class(&self) -> bool {
            self.player
        }
        fn velocity(&mut self) -> Result<Vec3, Self::Error> {
            Ok(self.velocity)
        }
        fn add_delta_movement(&mut self, delta: Vec3) -> Result<(), Self::Error> {
            self.added = self.added.add_vector(delta);
            Ok(())
        }
    }

    /// `EntityFluidInteraction$CurrentAccumulator.applyTo`: a body that is not
    /// a player takes the direction of the accumulated current, so two samples
    /// of one block push as hard as one; a player takes their average, so the
    /// push grows with the number of blocks and is halved by two of them.
    #[test]
    fn a_current_pushes_a_player_by_its_average_and_anything_else_by_its_direction() {
        let mut acc = CurrentAccumulator::default();
        acc.accumulate(Vec3::new(1.0, 0.0, 0.0));
        let mut mob = Body {
            player: false,
            velocity: Vec3::ZERO,
            added: Vec3::ZERO,
        };
        acc.apply_to(&mut mob, 0.014).unwrap();
        assert!((mob.added.x - 0.014).abs() < 1e-12, "mob: {}", mob.added.x);
        acc.accumulate(Vec3::new(1.0, 0.0, 0.0));
        let mut mob = Body {
            player: false,
            velocity: Vec3::ZERO,
            added: Vec3::ZERO,
        };
        acc.apply_to(&mut mob, 0.014).unwrap();
        assert!(
            (mob.added.x - 0.014).abs() < 1e-12,
            "a second sample does not double a mob's push: {}",
            mob.added.x
        );
        let mut player = Body {
            player: true,
            velocity: Vec3::ZERO,
            added: Vec3::ZERO,
        };
        acc.apply_to(&mut player, 0.014).unwrap();
        assert!(
            (player.added.x - 0.014).abs() < 1e-12,
            "the average of two (1,0,0) samples is (1,0,0): {}",
            player.added.x
        );
    }

    /// The same body with a single weak sample: the average is (0.5,0,0) for a
    /// player, and a nearly still body gets the 0.0045 lift either way.
    #[test]
    fn a_weak_current_on_a_still_body_is_lifted_to_the_minimum() {
        let mut acc = CurrentAccumulator::default();
        // A tenth of a block a tick: 0.0014 once scaled, under the minimum.
        acc.accumulate(Vec3::new(0.1, 0.0, 0.0));
        let mut player = Body {
            player: true,
            velocity: Vec3::ZERO,
            added: Vec3::ZERO,
        };
        acc.apply_to(&mut player, 0.014).unwrap();
        assert!(
            (player.added.x - 0.0045).abs() < 1e-12,
            "{}",
            player.added.x
        );
        // A body that is already moving on x or z takes the small push as it is.
        let mut moving = Body {
            player: true,
            velocity: Vec3::new(0.05, 0.0, 0.0),
            added: Vec3::ZERO,
        };
        acc.apply_to(&mut moving, 0.014).unwrap();
        assert!(
            (moving.added.x - 0.0014).abs() < 1e-12,
            "{}",
            moving.added.x
        );
    }

    /// `applyTo` returns before reading the velocity with no samples, and when
    /// the accumulated current is under the 1e-5 of a squared length.
    #[test]
    fn no_current_and_a_current_under_the_threshold_push_nothing() {
        let mut body = Body {
            player: false,
            velocity: Vec3::ZERO,
            added: Vec3::ZERO,
        };
        CurrentAccumulator::default()
            .apply_to(&mut body, 0.014)
            .unwrap();
        assert_eq!(body.added, Vec3::ZERO);
        let mut acc = CurrentAccumulator::default();
        acc.accumulate(Vec3::new(0.001, 0.0, 0.0));
        acc.apply_to(&mut body, 0.014).unwrap();
        assert_eq!(body.added, Vec3::ZERO);
    }

    /// A pool of one fluid a block deep and three wide around x 0..3, z 0..3,
    /// y 0..1, its source blocks flowing east, and a body in it.
    struct Pool {
        fluid: FluidKind,
        body: Aabb,
        eye: f64,
        player: bool,
        velocity: Vec3,
        added: Vec3,
        /// What `in_mod_fluid_types` was told: the fall modifier and extinguishing.
        mod_types: Option<(f32, bool)>,
    }
    impl crate::entity_fluid_tick::EntityFluidTickWorld for Pool {
        fn is_pushed_by_fluid(&mut self) -> Result<bool, std::convert::Infallible> {
            Ok(true)
        }
        fn reset_fall_distance(&mut self) -> Result<(), std::convert::Infallible> {
            Ok(())
        }
        fn was_touching_water(&self) -> bool {
            false
        }
        fn first_tick(&self) -> bool {
            false
        }
        fn water_splash(&mut self) -> Result<(), std::convert::Infallible> {
            Ok(())
        }
        fn set_was_touching_water(&mut self, _: bool) {}
        fn fast_lava(&mut self) -> Result<bool, std::convert::Infallible> {
            Ok(false)
        }
        fn in_mod_fluid_types(
            &mut self,
            modifier: f32,
            extinguish: bool,
        ) -> Result<(), std::convert::Infallible> {
            self.mod_types = Some((modifier, extinguish));
            Ok(())
        }
    }
    impl FluidMembership for Pool {
        fn fluid_in_tag(&self, kind: FluidKind, tag: FluidTagId) -> bool {
            // WATER is tag 0: vanilla water by identity, as the game asks it.
            tag == FluidTagId(0) && kind.is_water()
        }
    }
    impl CurrentTarget for Pool {
        type Error = std::convert::Infallible;
        fn is_player_class(&self) -> bool {
            self.player
        }
        fn velocity(&mut self) -> Result<Vec3, Self::Error> {
            Ok(self.velocity)
        }
        fn add_delta_movement(&mut self, delta: Vec3) -> Result<(), Self::Error> {
            self.added = self.added.add_vector(delta);
            Ok(())
        }
    }
    impl FluidInteractionWorld for Pool {
        type Error = std::convert::Infallible;
        type Chunk = ();
        fn interaction_box(&mut self) -> Result<Option<Aabb>, Self::Error> {
            Ok(Some(self.body.deflate(Vec3::new(0.001, 0.001, 0.001))))
        }
        fn loaded_chunk(&mut self, _: i32, _: i32) -> Result<Option<()>, Self::Error> {
            Ok(Some(()))
        }
        fn chunk_sections(&mut self, _: ()) -> Result<FluidSections<'_>, Self::Error> {
            Ok(FluidSections {
                minimum_section_y: -4,
                has_fluid: &[true; 24],
            })
        }
        fn bounding_box_min_y(&self) -> f64 {
            self.body.min_position().y
        }
        fn block_x(&self) -> i32 {
            1
        }
        fn eye_y(&mut self) -> Result<f64, Self::Error> {
            Ok(self.eye)
        }
        fn block_z(&self) -> i32 {
            1
        }
        fn fluid_state(&mut self, p: BlockPos) -> Result<FluidState, Self::Error> {
            let inside = (0..3).contains(&p.x) && p.y == 0 && (0..3).contains(&p.z);
            Ok(if inside {
                FluidState::new(self.fluid, 8, false)
            } else {
                FluidState::new(FluidKind::Empty, 0, false)
            })
        }
        fn fluid_height(&mut self, _: FluidState, _: BlockPos) -> Result<f32, Self::Error> {
            Ok(8.0 / 9.0)
        }
        fn camera_height(&mut self, _: FluidState, _: BlockPos) -> Result<f32, Self::Error> {
            Ok(8.0 / 9.0)
        }
        fn flow(&mut self, _: FluidState, _: BlockPos) -> Result<Vec3, Self::Error> {
            Ok(Vec3::new(1.0, 0.0, 0.0))
        }
    }
    fn pool(fluid: FluidKind, eye: f64) -> Pool {
        Pool {
            fluid,
            body: Aabb::new(Vec3::new(1.2, 0.1, 1.2), Vec3::new(1.8, 1.9, 1.8)),
            eye,
            player: false,
            velocity: Vec3::ZERO,
            added: Vec3::ZERO,
            mod_types: None,
        }
    }

    /// NeoForge's `Entity.updateInWaterStateAndDoFluidPushing` over the tick:
    /// a mod fluid pushes, and the entity hears its type's fall distance
    /// modifier and whether it puts fires out; water tells it nothing.
    #[test]
    fn the_fluid_tick_hands_a_mod_fluids_fall_modifier_and_fire_to_the_entity() {
        crate::mod_fluids::register_test_fluids();
        let mut fluids = EntityFluidInteraction::new([FluidTagId(0), FluidTagId(1)]);
        let sludge = FluidKind::from_name("test:sludge").unwrap();
        let mut world = pool(sludge, 0.5);
        crate::entity_fluid_tick::update(
            &mut fluids,
            &mut world,
            FluidTagId(0),
            FluidTagId(1),
            1000,
        )
        .unwrap();
        assert_eq!(world.mod_types, Some((0.2, true)));
        assert!(world.added.x > 0.0, "pushed east");
        let mut world = pool(FluidKind::from_name("test:brine").unwrap(), 0.5);
        crate::entity_fluid_tick::update(
            &mut fluids,
            &mut world,
            FluidTagId(0),
            FluidTagId(1),
            1000,
        )
        .unwrap();
        assert_eq!(world.mod_types, Some((0.5, false)));
        let mut world = pool(FluidKind::Water, 0.5);
        crate::entity_fluid_tick::update(
            &mut fluids,
            &mut world,
            FluidTagId(0),
            FluidTagId(1),
            1000,
        )
        .unwrap();
        assert_eq!(world.mod_types, None);
    }

    /// NeoForge's `Entity.updateFluidHeightAndDoFluidPushing` for a content
    /// mod's fluid: its type's height (`forgeFluidTypeHeight`), its flow
    /// scaled by the type's `motionScale`, and the eyes in it; no water.
    #[test]
    fn a_mod_fluid_has_a_height_and_pushes_by_its_own_motion_scale() {
        crate::mod_fluids::register_test_fluids();
        let brine = FluidKind::from_name("test:brine").unwrap();
        let mut world = pool(brine, 0.5);
        let mut fluids = EntityFluidInteraction::new([FluidTagId(0), FluidTagId(1)]);
        assert!(fluids.update(&mut world, false, 1000).unwrap());
        let types = fluids.mod_fluid_types();
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].0, brine);
        assert!(
            (types[0].1 - (f64::from(8.0_f32 / 9.0) - 0.1)).abs() < 1e-9,
            "{}",
            types[0].1
        );
        assert_eq!(fluids.eye_in_mod_fluid(), Some(brine));
        assert!(
            !fluids.is_in_fluid(&world, FluidTagId(0)),
            "no water: NeoForge's WATER_TYPE is water's"
        );
        fluids.apply_mod_currents(&mut world).unwrap();
        assert!(
            (world.added.x - 0.014).abs() < 1e-12,
            "brine's 0.014: {}",
            world.added.x
        );
        // The eyes above the surface; the mod fluid's flowing form is the same type.
        let mut world = pool(brine.flowing(), 1.5);
        assert!(fluids.update(&mut world, false, 1000).unwrap());
        assert_eq!(
            (fluids.mod_fluid_types()[0].0, fluids.eye_in_mod_fluid()),
            (brine, None)
        );
        // A molten metal's lava-like push; an entity that isn't pushed is not.
        let sludge = FluidKind::from_name("test:sludge").unwrap();
        let mut world = pool(sludge, 0.5);
        assert!(fluids.update(&mut world, false, 1000).unwrap());
        fluids.apply_mod_currents(&mut world).unwrap();
        assert!(
            (world.added.x - 0.0045).abs() < 1e-12,
            "0.0023 lifted to the minimum: {}",
            world.added.x
        );
        let mut world = pool(sludge, 0.5);
        assert!(fluids.update(&mut world, true, 1000).unwrap());
        fluids.apply_mod_currents(&mut world).unwrap();
        assert_eq!(world.added, Vec3::ZERO);
        assert_eq!(fluids.max_height_mod_fluid().map(|(k, _)| k), Some(sludge));
    }
}
