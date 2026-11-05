// The Mechanical Pipes and Fluid Tanks of the `mekanism` native bundle (support module
// `crate::game::mek_transmit`): where `mek_world`'s hooks hand them over, so
// each of `mek_world`'s hooks asks this module once. Mekanism is MIT,
// Copyright (c) 2017-2025 Aidan C. Brady.

use super::{Game, mek_fluid_tanks, mek_pipes, native_hooks::NativeFluidTankInfo};
use horizon_client::containers::ModBlockEntity;
use horizon_core::geometry::Direction as Face;
use horizon_world::{fluid_stack::FluidStack, nbt::Compound};
use mekanism_energy::{network::Connection, sides::Direction};
use serde_json::Value;

/// A world opens or closes.
pub(super) fn clear() {
    mek_pipes::clear();
}

/// `native_block_entity`: whether the block is one of these and got its entity.
pub(super) fn new_entity(block: &str, entity: &mut Option<(String, Compound)>) -> bool {
    if mek_pipes::tier_of(block).is_some() {
        *entity = Some(mek_pipes::new_entity(block));
        return true;
    }
    if mek_fluid_tanks::tier_of(block).is_some() {
        *entity = Some(mek_fluid_tanks::new_entity(block));
        return true;
    }
    false
}

/// `native_block_entity_ticks`.
pub(super) fn ticks(block: &str) -> bool {
    mek_fluid_tanks::tier_of(block).is_some()
}

/// A server tick of one of these block entities: whether it was one.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) -> bool {
    if let Some(tier) = mek_fluid_tanks::tier_of(&entity.id) {
        mek_fluid_tanks::tick(game, pos, entity, tier);
        return true;
    }
    false
}

/// `native_block_entity_load` of any Mekanism block entity: a transmitter
/// joins its network (and the hook is done), any other block is looked at by
/// the pipes beside it.
pub(super) fn loaded(dimension: &str, pos: [i32; 3], entity: &ModBlockEntity) -> bool {
    if let Some(tier) = mek_pipes::tier_of(&entity.id) {
        mek_pipes::loaded(dimension, pos, &entity.nbt, tier);
        return true;
    }
    mek_pipes::neighbour(dimension, pos);
    false
}

/// `native_block_entity_unload`.
pub(super) fn unloaded(dimension: &str, pos: [i32; 3], entity: &ModBlockEntity, removed: bool) -> bool {
    if mek_pipes::tier_of(&entity.id).is_some() {
        mek_pipes::unloaded(dimension, pos, removed);
        return true;
    }
    mek_pipes::neighbour(dimension, pos);
    false
}

/// `native_block_entity_save`.
pub(super) fn saved(dimension: &str, pos: [i32; 3], id: &str, nbt: &mut Compound) -> bool {
    if mek_pipes::tier_of(id).is_some() {
        mek_pipes::saved(dimension, pos, nbt);
        return true;
    }
    false
}

/// `native_block_changed`.
pub(super) fn block_set(pos: [i32; 3]) {
    mek_pipes::block_set(pos);
}

/// `native_level_tick_post`.
pub(super) fn level_ticked(game: &mut Game<'_>, dimension: &str) {
    mek_pipes::level_ticked(game, dimension);
}

/// `native_fluid_handler_tanks`: whether one of these answered.
pub(super) fn fluid_tanks(dimension: &str, pos: [i32; 3], face: Option<Face>, entity: &ModBlockEntity, tanks: &mut Option<Vec<NativeFluidTankInfo>>) -> bool {
    mek_pipes::fluid_tanks(dimension, pos, face, entity, tanks) || mek_fluid_tanks::fluid_tanks(face, entity, tanks)
}

/// `native_fluid_handler_fill`: whether one of these answered.
#[allow(clippy::too_many_arguments)]
pub(super) fn fluid_fill(dimension: &str, pos: [i32; 3], face: Option<Face>, entity: &mut ModBlockEntity, resource: &FluidStack, simulate: bool, filled: &mut Option<i32>) -> bool {
    mek_pipes::fluid_fill(dimension, pos, face, entity, resource, simulate, filled) || mek_fluid_tanks::fluid_fill(face, entity, resource, simulate, filled)
}

/// `native_fluid_handler_drain`: whether one of these answered.
#[allow(clippy::too_many_arguments)]
pub(super) fn fluid_drain(
    dimension: &str,
    pos: [i32; 3],
    face: Option<Face>,
    entity: &mut ModBlockEntity,
    resource: Option<&FluidStack>,
    max: i32,
    simulate: bool,
    drained: &mut Option<FluidStack>,
) -> bool {
    mek_pipes::fluid_drain(dimension, pos, face, entity, resource, max, simulate, drained)
        || mek_fluid_tanks::fluid_drain(face, entity, resource, max, simulate, drained)
}

/// `/mek probe` of one of these.
pub(super) fn probe(game: &mut Game<'_>, pos: [i32; 3], id: &str) -> Option<Value> {
    if mek_pipes::tier_of(id).is_some() {
        return Some(mek_pipes::probe(game, pos));
    }
    if mek_fluid_tanks::tier_of(id).is_some() {
        return mek_fluid_tanks::probe(game, pos);
    }
    None
}

/// `/mek connection` on one of these transmitters.
pub(super) fn set_connection(game: &mut Game<'_>, pos: [i32; 3], face: Direction, connection: Connection) -> Option<Value> {
    mek_pipes::set_connection(game, pos, face, connection)
}

