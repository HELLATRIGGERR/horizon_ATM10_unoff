//! Fluid handlers of a block for native server mods (docs/native-mods.md,
//! API 160): NeoForge 1.21.1's `Capabilities.FluidHandler.BLOCK`
//! (`IFluidHandler`), asked for a block and one of its sides, with fluids as
//! [`FluidStack`]s of the registry (vanilla's and the content mods'). A mod
//! block entity answers through the hook points `native_fluid_handler_tanks`,
//! `native_fluid_handler_fill` and `native_fluid_handler_drain`; one that
//! answers none and declared tanks (`native_block_entity_fluids`, API 54) is
//! read and moved by those tanks' rules. AE2's storage, import and export
//! buses reach blocks through this dispatcher, and `FluidUtil`'s container
//! transfers (`native_fluid_interact`) through it and the item fluid helpers
//! (`native_item_fluids.rs`).
//!
//! As NeoForge's, a handler's calls are `int` millibuckets with
//! `FluidAction.SIMULATE` or `EXECUTE`: no transactions. A block entity is
//! out of the world while it answers a fill or a drain, as in its tick, and
//! answers nothing then; the same block, side and operation asked again
//! inside its own call answers nothing, and so does a chain 16 calls deep
//! (`native_fe.rs` guards Forge Energy the same way).
use super::Game;
use super::native_fe::{dim_of, put_back, take};
use super::native_fluids::{Tank, read_tanks, write_tanks};
use crate::game::{Dim, native_hooks::NativeFluidTankInfo};
use horizon_client::{
    containers::{BlockEntity, ModBlockEntity, ModFluidTanks},
    first_person::Hand,
    items::ItemStack,
};
use horizon_core::geometry::Direction;
use horizon_world::fluid::FluidKind;
use horizon_world::fluid_stack::{FluidStack, empty_sound, fill_sound};
use std::cell::RefCell;
use std::sync::Arc;

/// What a call does at a block; the re-entry guard tells them apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Tanks,
    Fill,
    Drain,
}

/// The deepest chain of fluid handler calls.
const DEPTH: usize = 16;

type Call = (Dim, [i32; 3], Option<Direction>, Op);

thread_local! {
    /// The fluid handler calls under way on this thread, outermost first.
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
}

/// A call under way; dropping it ends the call.
struct Entered;

impl Entered {
    fn enter(call: Call) -> Option<Self> {
        CALLS.with(|calls| {
            let mut calls = calls.borrow_mut();
            if calls.len() >= DEPTH || calls.contains(&call) {
                return None;
            }
            calls.push(call);
            Some(Self)
        })
    }
}

impl Drop for Entered {
    fn drop(&mut self) {
        CALLS.with(|calls| {
            calls.borrow_mut().pop();
        });
    }
}

/// The declared tanks `face` reaches, by number: a null side reaches them
/// all, as `native_fe.rs` reads a declared buffer.
fn declared_reach(tanks: &ModFluidTanks, face: Option<Direction>) -> Vec<usize> {
    let n = tanks.tanks.len();
    match face {
        Some(face) => tanks.faces[face as usize]
            .iter()
            .copied()
            .filter(|&t| t < n)
            .collect(),
        None => (0..n).collect(),
    }
}

/// A saved tank's contents as a stack; a fluid the registry lacks reads as
/// empty (`FluidStack.parse` fails on it).
fn tank_stack(tank: &Tank) -> FluidStack {
    match tank {
        (Some(fluid), amount) => FluidStack::of(fluid, *amount).unwrap_or(FluidStack::EMPTY),
        (None, _) => FluidStack::EMPTY,
    }
}

/// Declared tanks as `IFluidHandler` reads them through `face`: none on a
/// side that reaches none.
fn declared_tanks(
    tanks: &ModFluidTanks,
    face: Option<Direction>,
    entity: &ModBlockEntity,
) -> Option<Vec<NativeFluidTankInfo>> {
    let reach = declared_reach(tanks, face);
    if reach.is_empty() {
        return None;
    }
    let state = read_tanks(&entity.nbt, tanks.tanks.len());
    Some(
        reach
            .into_iter()
            .map(|t| {
                let stack = tank_stack(&state[t]);
                NativeFluidTankInfo {
                    drainable: !stack.is_empty() && tanks.outputs.contains(&t),
                    stack,
                    capacity: tanks.tanks[t],
                }
            })
            .collect(),
    )
}

/// `fill` on declared tanks, by API 54's rules: first the reached input
/// tanks holding the same fluid, then the empty ones. They keep no
/// components, so a stack with some fills nothing. What it took.
fn declared_fill(
    tanks: &ModFluidTanks,
    face: Option<Direction>,
    entity: &mut ModBlockEntity,
    resource: &FluidStack,
    simulate: bool,
) -> i32 {
    if resource.is_empty() || !resource.components().is_empty() {
        return 0;
    }
    let name = resource.name();
    let reach = declared_reach(tanks, face);
    let mut state = read_tanks(&entity.nbt, tanks.tanks.len());
    let mut left = resource.amount();
    for pass in 0..2 {
        for &t in &reach {
            if left == 0 || !tanks.inputs.contains(&t) {
                continue;
            }
            let entry = &mut state[t];
            let fits = match pass {
                0 => entry.0.as_deref() == Some(name),
                _ => entry.0.is_none(),
            };
            let room = tanks.tanks[t] - entry.1;
            if !fits || room <= 0 {
                continue;
            }
            let add = room.min(left);
            entry.1 += add;
            entry.0 = Some(name.to_owned());
            left -= add;
        }
    }
    let took = resource.amount() - left;
    if took > 0 && !simulate {
        write_tanks(&mut entity.nbt, &state);
    }
    took
}

/// `drain` on declared tanks: of `resource`'s fluid (at most its amount),
/// or of the first reached output tank's that holds any (at most `max`).
fn declared_drain(
    tanks: &ModFluidTanks,
    face: Option<Direction>,
    entity: &mut ModBlockEntity,
    resource: Option<&FluidStack>,
    max: i32,
    simulate: bool,
) -> FluidStack {
    let reach: Vec<usize> = declared_reach(tanks, face)
        .into_iter()
        .filter(|t| tanks.outputs.contains(t))
        .collect();
    let mut state = read_tanks(&entity.nbt, tanks.tanks.len());
    let (fluid, mut want) = match resource {
        Some(r) if r.is_empty() || !r.components().is_empty() => return FluidStack::EMPTY,
        Some(r) => (r.name().to_owned(), r.amount()),
        None => {
            let Some(first) = reach
                .iter()
                .find(|&&t| state[t].1 > 0)
                .and_then(|&t| state[t].0.clone())
            else {
                return FluidStack::EMPTY;
            };
            (first, max)
        }
    };
    let Some(kind) = FluidKind::from_name(&fluid) else {
        return FluidStack::EMPTY;
    };
    let mut took = 0;
    for &t in &reach {
        if want <= 0 {
            break;
        }
        if state[t].0.as_deref() != Some(&fluid) {
            continue;
        }
        let take = want.min(state[t].1);
        state[t].1 -= take;
        if state[t].1 == 0 {
            state[t].0 = None;
        }
        want -= take;
        took += take;
    }
    if took > 0 && !simulate {
        write_tanks(&mut entity.nbt, &state);
    }
    FluidStack::new(kind, took)
}

impl Game<'_> {
    /// The tanks the block declared (`native_block_entity_fluids`) at `pos`
    /// in level `dim`.
    fn declared_fluid_tanks(&self, dim: Dim, pos: [i32; 3]) -> Option<Arc<ModFluidTanks>> {
        let world = self.fe_world(dim)?;
        let state = world.block(pos[0], pos[1], pos[2]);
        world.block_entities.mod_fluids(state.0).cloned()
    }

    /// `IFluidHandler`'s tanks of the block at `pos` in level `dim` through
    /// `face`: a mod block entity's (`native_fluid_handler_tanks`, else its
    /// declared tanks). None where it has no handler on that side, and for a
    /// loop.
    pub(in crate::game) fn fluid_tanks_at(
        &self,
        dim: Dim,
        pos: [i32; 3],
        face: Option<Direction>,
    ) -> Option<Vec<NativeFluidTankInfo>> {
        let _call = Entered::enter((dim, pos, face, Op::Tanks))?;
        let key = (pos[0], pos[1], pos[2]);
        let world = self.fe_world(dim)?;
        let Some(BlockEntity::Mod(m)) = world.block_entities.get(&key) else {
            return None;
        };
        if m.id.is_empty() {
            return None;
        }
        let mut tanks = None;
        self.native_fluid_handler_tanks(dim.id(), pos, face, m, &mut tanks);
        tanks.or_else(|| declared_tanks(self.declared_fluid_tanks(dim, pos)?.as_ref(), face, m))
    }

    /// Takes the mod block entity at `pos` out of level `dim` for `call`,
    /// puts it back, and marks it changed when it changed for real.
    fn with_fluid_entity<T>(
        &mut self,
        dim: Dim,
        pos: [i32; 3],
        call: impl FnOnce(&mut Self, &mut ModBlockEntity) -> (T, bool),
    ) -> Option<T> {
        let key = (pos[0], pos[1], pos[2]);
        let mut taken = self.fe_world_mut(dim).and_then(|world| take(world, key))?;
        let items = taken.items.clone();
        let (out, moved) = call(self, &mut taken);
        let items_moved = taken.items != items;
        let back = self
            .fe_world_mut(dim)
            .is_some_and(|world| put_back(world, key, taken, moved || items_moved));
        if back && items_moved && dim == self.dim {
            self.container_changed(key);
        }
        Some(out)
    }

    /// `IFluidHandler.fill` of `resource` on the block at `pos` in level
    /// `dim` through `face`: what it took, 0 to the resource's amount.
    pub(in crate::game) fn fluid_fill_at(
        &mut self,
        dim: Dim,
        pos: [i32; 3],
        face: Option<Direction>,
        resource: &FluidStack,
        simulate: bool,
    ) -> i32 {
        if resource.is_empty() {
            return 0;
        }
        let Some(_call) = Entered::enter((dim, pos, face, Op::Fill)) else {
            return 0;
        };
        let declared = self.declared_fluid_tanks(dim, pos);
        let amount = resource.amount();
        self.with_fluid_entity(dim, pos, |game, entity| {
            let mut answer = None;
            game.native_fluid_handler_fill(
                dim.id(),
                pos,
                face,
                entity,
                resource,
                simulate,
                &mut answer,
            );
            let filled = match (answer, &declared) {
                (Some(n), _) => n,
                (None, Some(tanks)) => declared_fill(tanks, face, entity, resource, simulate),
                (None, None) => 0,
            }
            .clamp(0, amount);
            (filled, filled > 0 && !simulate)
        })
        .unwrap_or(0)
    }

    /// `IFluidHandler.drain` on the block at `pos` in level `dim` through
    /// `face`: with `resource`, `drain(FluidStack)`, else `drain(int)` of at
    /// most `max`. What left it; the empty stack for nothing.
    pub(in crate::game) fn fluid_drain_at(
        &mut self,
        dim: Dim,
        pos: [i32; 3],
        face: Option<Direction>,
        resource: Option<&FluidStack>,
        max: i32,
        simulate: bool,
    ) -> FluidStack {
        let limit = resource.map_or(max, FluidStack::amount);
        if limit <= 0 || resource.is_some_and(FluidStack::is_empty) {
            return FluidStack::EMPTY;
        }
        let Some(_call) = Entered::enter((dim, pos, face, Op::Drain)) else {
            return FluidStack::EMPTY;
        };
        let declared = self.declared_fluid_tanks(dim, pos);
        self.with_fluid_entity(dim, pos, |game, entity| {
            let mut answer = None;
            game.native_fluid_handler_drain(
                dim.id(),
                pos,
                face,
                entity,
                resource,
                max,
                simulate,
                &mut answer,
            );
            let mut drained = match (answer, &declared) {
                (Some(stack), _) => stack,
                (None, Some(tanks)) => declared_drain(tanks, face, entity, resource, max, simulate),
                (None, None) => FluidStack::EMPTY,
            };
            // A handler gives no more than was asked, and only the fluid asked for.
            if resource.is_some_and(|r| !FluidStack::is_same_fluid_same_components(r, &drained)) {
                drained = FluidStack::EMPTY;
            }
            drained.limit_size(limit);
            let moved = !drained.is_empty() && !simulate;
            (drained, moved)
        })
        .unwrap_or(FluidStack::EMPTY)
    }
}

/// One side of a `FluidUtil` transfer: a block's handler or one container
/// item, reached through `C` (the game).
trait Handler<C: ?Sized> {
    fn fill(&mut self, cx: &mut C, resource: &FluidStack, simulate: bool) -> i32;
    fn drain(
        &mut self,
        cx: &mut C,
        resource: Option<&FluidStack>,
        max: i32,
        simulate: bool,
    ) -> FluidStack;
}

/// A block's fluid handler on one side (`FluidUtil.getFluidHandler(level, pos, side)`).
struct BlockSide {
    dim: Dim,
    pos: [i32; 3],
    face: Option<Direction>,
}

impl<'g> Handler<Game<'g>> for BlockSide {
    fn fill(&mut self, game: &mut Game<'g>, resource: &FluidStack, simulate: bool) -> i32 {
        game.fluid_fill_at(self.dim, self.pos, self.face, resource, simulate)
    }
    fn drain(
        &mut self,
        game: &mut Game<'g>,
        resource: Option<&FluidStack>,
        max: i32,
        simulate: bool,
    ) -> FluidStack {
        game.fluid_drain_at(self.dim, self.pos, self.face, resource, max, simulate)
    }
}

/// One container item as its `IFluidHandlerItem` (`FluidBucketWrapper`, a
/// mod's container); `container` is what it has become (`getContainer`).
struct ItemSide {
    container: ItemStack,
    capacity: Option<i32>,
}

impl ItemSide {
    /// `FluidUtil.getFluidHandler(container.copyWithCount(1))`: none for an
    /// item that holds no fluid.
    fn of(game: &Game<'_>, stack: &ItemStack) -> Option<Self> {
        let mut container = stack.clone();
        container.count = 1;
        let capacity = game.native_item_tank_capacity(&container.item);
        crate::game::native_item_fluids::contents(&container, capacity)?;
        Some(Self {
            container,
            capacity,
        })
    }
}

impl<C: ?Sized> Handler<C> for ItemSide {
    fn fill(&mut self, _: &mut C, resource: &FluidStack, simulate: bool) -> i32 {
        let Some((took, after)) = crate::game::native_item_fluids::fill_one(
            &self.container,
            self.capacity,
            resource.name(),
            resource.amount(),
        ) else {
            return 0;
        };
        if took > 0 && !simulate {
            self.container = after;
        }
        took
    }
    fn drain(
        &mut self,
        _: &mut C,
        resource: Option<&FluidStack>,
        max: i32,
        simulate: bool,
    ) -> FluidStack {
        let held = crate::game::native_item_fluids::contents(&self.container, self.capacity);
        let held_fluid = held.as_ref().and_then(|h| h.fluid.as_deref());
        let max = match resource {
            // Containers keep no components: a resource with some matches nothing.
            Some(r) if held_fluid != Some(r.name()) || !r.components().is_empty() => {
                return FluidStack::EMPTY;
            }
            Some(r) => r.amount(),
            None => max,
        };
        let Some((fluid, took, after)) =
            crate::game::native_item_fluids::drain_one(&self.container, self.capacity, max)
        else {
            return FluidStack::EMPTY;
        };
        if took > 0 && !simulate {
            self.container = after;
        }
        FluidStack::of(&fluid, took).unwrap_or(FluidStack::EMPTY)
    }
}

/// `FluidUtil.tryFluidTransfer(dest, source, maxAmount, doTransfer)`.
fn try_fluid_transfer<C: ?Sized>(
    cx: &mut C,
    dest: &mut dyn Handler<C>,
    source: &mut dyn Handler<C>,
    max: i32,
    transfer: bool,
) -> FluidStack {
    let drainable = source.drain(cx, None, max, true);
    if drainable.is_empty() {
        return FluidStack::EMPTY;
    }
    transfer_internal(cx, dest, source, drainable, transfer)
}

/// `FluidUtil.tryFluidTransfer_Internal`.
fn transfer_internal<C: ?Sized>(
    cx: &mut C,
    dest: &mut dyn Handler<C>,
    source: &mut dyn Handler<C>,
    mut drainable: FluidStack,
    transfer: bool,
) -> FluidStack {
    let fillable = dest.fill(cx, &drainable, true);
    if fillable > 0 {
        drainable.set_amount(fillable);
        if !transfer {
            return drainable;
        }
        let mut drained = source.drain(cx, Some(&drainable), drainable.amount(), false);
        if !drained.is_empty() {
            let filled = dest.fill(cx, &drained, false);
            drained.set_amount(filled);
            return drained;
        }
    }
    FluidStack::EMPTY
}

impl<'g> Game<'g> {
    /// `player.level().playSound(null, x, y + 0.5, z, sound, BLOCKS, 1, 1)`
    /// at the player in place.
    fn fluid_util_sound(&mut self, sound: Option<&str>) {
        let (Some(sound), Some(p)) = (sound, self.player.as_ref()) else {
            return;
        };
        let at = [p.position.x, p.position.y + 0.5, p.position.z];
        self.audio.play(sound, Some(at), 1.0, 1.0);
    }

    /// `FluidUtil.tryFillContainer`: one item of `container` filled from
    /// `source`; the item it becomes, none on failure. `player`: the bucket
    /// sound plays at the player in place.
    fn try_fill_container(
        &mut self,
        container: &ItemStack,
        source: &mut dyn Handler<Game<'g>>,
        max: i32,
        player: bool,
        fill: bool,
    ) -> Option<ItemStack> {
        let mut item = ItemSide::of(self, container)?;
        let simulated = try_fluid_transfer(self, &mut item, source, max, false);
        if simulated.is_empty() {
            return None;
        }
        if fill {
            try_fluid_transfer(self, &mut item, source, max, true);
            if player {
                self.fluid_util_sound(fill_sound(simulated.fluid()));
            }
        } else {
            item.fill(self, &simulated, false);
        }
        Some(item.container)
    }

    /// `FluidUtil.tryEmptyContainer`: one item of `container` emptied into
    /// `dest`; the item it becomes, none on failure.
    fn try_empty_container(
        &mut self,
        container: &ItemStack,
        dest: &mut dyn Handler<Game<'g>>,
        max: i32,
        player: bool,
        drain: bool,
    ) -> Option<ItemStack> {
        let mut item = ItemSide::of(self, container)?;
        let transfer = try_fluid_transfer(self, dest, &mut item, max, drain);
        if transfer.is_empty() {
            return None;
        }
        if !drain {
            item.drain(self, Some(&transfer), transfer.amount(), false);
        }
        if drain && player {
            self.fluid_util_sound(empty_sound(transfer.fluid()));
        }
        Some(item.container)
    }

    /// `ItemHandlerHelper.insertItemStacked` into the player's inventory
    /// (`PlayerInvWrapper`: the main slots, then the off hand; armor slots
    /// take no fluid container): onto equal stacks first, then into empty
    /// slots. What did not fit.
    fn fluid_util_stow(&mut self, mut stack: ItemStack, simulate: bool) -> Option<ItemStack> {
        if stack.count == 0 {
            return None;
        }
        let max = self.res.items().stack_max(&stack);
        let mut slots: Vec<Option<ItemStack>> = self.inventory.clone();
        slots.push(self.offhand.clone());
        if max > 1 {
            for slot in slots.iter_mut().flatten() {
                if stack.count == 0 {
                    break;
                }
                if slot.same_item_same_components(&stack) && slot.count < max {
                    let add = (max - slot.count).min(stack.count);
                    slot.count += add;
                    stack.count -= add;
                }
            }
        }
        for slot in slots.iter_mut() {
            if stack.count == 0 {
                break;
            }
            if slot.is_none() {
                let mut put = stack.clone();
                put.count = stack.count.min(max);
                stack.count -= put.count;
                *slot = Some(put);
            }
        }
        if !simulate {
            self.offhand = slots.pop().flatten();
            self.inventory = slots;
        }
        (stack.count > 0).then_some(stack)
    }

    /// `FluidUtil.tryFillContainerAndStow` (with `fill` false,
    /// `tryEmptyContainerAndStow`) for the player in place: a creative player
    /// keeps the container; one alone is replaced; one of several is shrunk
    /// and the filled one goes into the inventory, or is dropped.
    fn fluid_container_and_stow(
        &mut self,
        container: &ItemStack,
        handler: &mut dyn Handler<Game<'g>>,
        fill: bool,
    ) -> Option<ItemStack> {
        if container.count == 0 {
            return None;
        }
        let mut attempt = |game: &mut Self, real: bool| {
            if fill {
                game.try_fill_container(container, handler, i32::MAX, true, real)
            } else {
                game.try_empty_container(container, handler, i32::MAX, true, real)
            }
        };
        if self.creative {
            return attempt(self, true).map(|_| container.clone());
        }
        if container.count == 1 {
            return attempt(self, true);
        }
        let simulated = attempt(self, false)?;
        let _ = self.fluid_util_stow(simulated, true);
        // A player is there, so the real transfer goes on whether it fits.
        if let Some(result) = attempt(self, true)
            && let Some(rest) = self.fluid_util_stow(result, false)
        {
            self.give_or_drop(rest);
        }
        let mut shrunk = container.clone();
        shrunk.count -= 1;
        Some(shrunk)
    }
}

/// Fluid handler helpers for native mods. API 160.
#[allow(dead_code)]
impl Game<'_> {
    /// The fluid handler of the block at `pos` in `dimension` through `face`
    /// (the side of that block you touch; none is the block itself), as
    /// `IFluidHandler` reads it: each tank's fluid and capacity. None where
    /// it has no handler on that side, in a level that isn't open, on a
    /// client's copy of the world, and for the block entity ticking now.
    /// API 160.
    pub(in crate::game) fn native_fluid_handler(
        &self,
        dimension: &str,
        pos: [i32; 3],
        face: Option<Direction>,
    ) -> Option<Vec<NativeFluidTankInfo>> {
        if !self.native_simulates() {
            return None;
        }
        self.fluid_tanks_at(dim_of(dimension)?, pos, face)
    }

    /// `IFluidHandler.fill` of `resource` on the block at `pos` in
    /// `dimension` through `face`, as `native_fluid_handler` finds it: what it
    /// took (with `simulate`, would take), 0 to the resource's amount. API 160.
    pub(in crate::game) fn native_fluid_fill(
        &mut self,
        dimension: &str,
        pos: [i32; 3],
        face: Option<Direction>,
        resource: &FluidStack,
        simulate: bool,
    ) -> i32 {
        if !self.native_simulates() {
            return 0;
        }
        let Some(dim) = dim_of(dimension) else {
            return 0;
        };
        self.fluid_fill_at(dim, pos, face, resource, simulate)
    }

    /// `IFluidHandler.drain` on the block at `pos` in `dimension` through
    /// `face`: with `resource`, that fluid (at most its amount); without, at
    /// most `max` of whatever comes out first. What left it (with
    /// `simulate`, would leave); the empty stack for nothing. API 160.
    pub(in crate::game) fn native_fluid_drain(
        &mut self,
        dimension: &str,
        pos: [i32; 3],
        face: Option<Direction>,
        resource: Option<&FluidStack>,
        max: i32,
        simulate: bool,
    ) -> FluidStack {
        if !self.native_simulates() {
            return FluidStack::EMPTY;
        }
        let Some(dim) = dim_of(dimension) else {
            return FluidStack::EMPTY;
        };
        self.fluid_drain_at(dim, pos, face, resource, max, simulate)
    }

    /// `FluidUtil.interactWithFluidHandler(player, hand, level, pos, side)`
    /// for the player in place, as a block's use calls it: the container in
    /// `hand` is filled from the block's handler on `face`, or else emptied
    /// into it, with the bucket's sound; one alone becomes the result, one of
    /// several is shrunk and the result stowed (or dropped), and a creative
    /// player keeps the container. Whether it moved fluid. False where the
    /// block has no handler on that side, the hand is empty or holds no
    /// container, and in a level that isn't the one in place. API 160.
    pub(in crate::game) fn native_fluid_interact(
        &mut self,
        hand: Hand,
        dimension: &str,
        pos: [i32; 3],
        face: Option<Direction>,
    ) -> bool {
        if !self.native_simulates() || dimension != self.dim.id() {
            return false;
        }
        let held = match hand {
            Hand::Main => self.inventory.get(self.selected).cloned().flatten(),
            Hand::Off => self.offhand.clone(),
        };
        let Some(held) = held.filter(|s| s.count > 0) else {
            return false;
        };
        if self.fluid_tanks_at(self.dim, pos, face).is_none() {
            return false;
        }
        let mut block = BlockSide {
            dim: self.dim,
            pos,
            face,
        };
        let result = match self.fluid_container_and_stow(&held, &mut block, true) {
            Some(result) => Some(result),
            None => self.fluid_container_and_stow(&held, &mut block, false),
        };
        let Some(result) = result else {
            return false;
        };
        let result = (result.count > 0).then_some(result);
        match hand {
            Hand::Main => {
                if let Some(slot) = self.inventory.get_mut(self.selected) {
                    *slot = result;
                }
            }
            Hand::Off => self.offhand = result,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use horizon_world::nbt::{Compound, JavaString, Tag};

    fn declared() -> ModFluidTanks {
        // Two tanks: 0 fillable from the top, 1 drainable from the bottom;
        // the null side reaches both.
        let mut faces: [Vec<usize>; 6] = Default::default();
        faces[Direction::Up as usize] = vec![0];
        faces[Direction::Down as usize] = vec![1, 0];
        ModFluidTanks {
            tanks: vec![4000, 2000],
            faces,
            inputs: vec![0],
            outputs: vec![1],
        }
    }

    fn entity_with(tanks: &[Tank]) -> ModBlockEntity {
        let mut nbt = Compound::new();
        write_tanks(&mut nbt, tanks);
        ModBlockEntity::new(horizon_client::containers::ModBlockEntityFields {
            id: "test:tank".into(),
            nbt,
            ..Default::default()
        })
    }

    #[test]
    fn declared_tanks_fill_and_drain_by_their_sides_with_any_registry_fluid() {
        horizon_world::mod_fluids::register_test_fluids();
        let tanks = declared();
        let mut entity = entity_with(&[(None, 0), (Some("test:brine".into()), 1500)]);
        let brine = FluidStack::of("test:brine", 3000).unwrap();
        // The top reaches the input tank; the bottom only drains.
        assert_eq!(
            declared_fill(&tanks, Some(Direction::Up), &mut entity, &brine, true),
            3000
        );
        assert!(
            read_tanks(&entity.nbt, 2)[0].0.is_none(),
            "a simulated fill changes nothing"
        );
        assert_eq!(
            declared_fill(&tanks, Some(Direction::Up), &mut entity, &brine, false),
            3000
        );
        assert_eq!(
            declared_fill(&tanks, Some(Direction::Up), &mut entity, &brine, false),
            1000
        );
        assert_eq!(
            declared_fill(&tanks, Some(Direction::North), &mut entity, &brine, false),
            0
        );
        // A stack with components fills nothing: the tanks keep none.
        let mut patch = Compound::new();
        patch.insert(JavaString::from("x"), Tag::Int(1));
        let patched = FluidStack::with_components(brine.fluid(), 10, patch);
        assert_eq!(declared_fill(&tanks, None, &mut entity, &patched, false), 0);
        let info = declared_tanks(&tanks, Some(Direction::Down), &entity).unwrap();
        assert_eq!(
            info.iter()
                .map(|t| (t.stack.name(), t.stack.amount(), t.capacity, t.drainable))
                .collect::<Vec<_>>(),
            [
                ("test:brine", 1500, 2000, true),
                ("test:brine", 4000, 4000, false)
            ]
        );
        assert_eq!(declared_tanks(&tanks, Some(Direction::East), &entity), None);
        // Draining takes the output tank only, at most what was asked.
        let out = declared_drain(
            &tanks,
            Some(Direction::Down),
            &mut entity,
            None,
            1000,
            false,
        );
        assert_eq!((out.name(), out.amount()), ("test:brine", 1000));
        let water = FluidStack::new(FluidKind::Water, 1000);
        assert!(
            declared_drain(
                &tanks,
                Some(Direction::Down),
                &mut entity,
                Some(&water),
                1000,
                false
            )
            .is_empty()
        );
        let rest = declared_drain(&tanks, None, &mut entity, Some(&brine), 0, false);
        assert_eq!(rest.amount(), 500);
        assert_eq!(
            read_tanks(&entity.nbt, 2),
            vec![(Some("test:brine".into()), 4000), (None, 0)]
        );
    }

    struct Tank1(FluidStack, i32);
    impl Handler<()> for Tank1 {
        fn fill(&mut self, _: &mut (), r: &FluidStack, simulate: bool) -> i32 {
            if !self.0.is_empty() && !FluidStack::is_same_fluid_same_components(&self.0, r) {
                return 0;
            }
            let took = r.amount().min(self.1 - self.0.amount());
            if !simulate && took > 0 {
                self.0 = r.copy_with_amount(self.0.amount() + took);
            }
            took
        }
        fn drain(
            &mut self,
            _: &mut (),
            r: Option<&FluidStack>,
            max: i32,
            simulate: bool,
        ) -> FluidStack {
            if r.is_some_and(|r| !FluidStack::is_same_fluid_same_components(r, &self.0)) {
                return FluidStack::EMPTY;
            }
            let out = self
                .0
                .copy_with_amount(r.map_or(max, FluidStack::amount).min(self.0.amount()));
            if !simulate {
                self.0.shrink(out.amount());
            }
            out
        }
    }

    #[test]
    fn a_bucket_moves_only_whole_into_and_out_of_a_tank() {
        // FluidUtil.tryFluidTransfer with FluidBucketWrapper's rules, no world.
        horizon_world::mod_fluids::register_test_fluids();
        let game_less = |dest: &mut dyn Handler<()>, source: &mut dyn Handler<()>, max: i32| {
            try_fluid_transfer(&mut (), dest, source, max, true)
        };
        let mut bucket = ItemSide {
            container: ItemStack::new("test:brine_bucket", 1),
            capacity: None,
        };
        let mut tank = Tank1(FluidStack::EMPTY, 1500);
        let moved = game_less(&mut tank, &mut bucket, i32::MAX);
        assert_eq!((moved.name(), moved.amount()), ("test:brine", 1000));
        assert_eq!(&*bucket.container.item, "minecraft:bucket");
        // 500 in the tank is less than a bucket: an empty bucket takes nothing.
        tank.0.shrink(500);
        let moved = game_less(&mut bucket, &mut tank, i32::MAX);
        assert!(moved.is_empty());
        assert_eq!(tank.0.amount(), 500);
        let mut sludge_tank = Tank1(FluidStack::of("test:sludge", 1000).unwrap(), 1000);
        let moved = game_less(&mut bucket, &mut sludge_tank, i32::MAX);
        assert_eq!(
            (moved.name(), &*bucket.container.item),
            ("test:sludge", "test:sludge_bucket")
        );
        // Another fluid's tank takes no bucket, nor one with less than a bucket of room;
        // the bucket stays full.
        assert!(game_less(&mut tank, &mut bucket, i32::MAX).is_empty());
        let mut nearly_full = Tank1(FluidStack::of("test:sludge", 600).unwrap(), 1500);
        assert!(game_less(&mut nearly_full, &mut bucket, i32::MAX).is_empty());
        assert_eq!(&*bucket.container.item, "test:sludge_bucket");
        // With a bucket of room, it empties whole.
        nearly_full.0.shrink(100);
        assert_eq!(
            game_less(&mut nearly_full, &mut bucket, i32::MAX).amount(),
            1000
        );
        assert_eq!(
            (nearly_full.0.amount(), &*bucket.container.item),
            (1500, "minecraft:bucket")
        );
    }
}
