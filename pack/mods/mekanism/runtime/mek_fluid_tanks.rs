// Mekanism's Fluid Tanks in the game (support module `crate::game::mek_fluid_tanks`
// of the `mekanism` native bundle): the Basic to Creative tanks as the pipes'
// sources and sinks. The rules are `mekanism_energy::fluid_tank` (MIT, from
// Mekanism 10.7.19, Copyright (c) 2017-2025 Aidan C. Brady); this module is the
// glue: the tank saved as Mekanism saves it (`fluid_tanks`, `active_state`), a
// fluid handler on every side (hook points API 160), an active tank giving
// what it holds down at its output a tick (`FluidUtils.emit` through the
// fluid handler helpers), the Configurator's sneaking use turning it on and
// off (`onSneakRightClick`), and `/mek probe`. Its screen, slots, edit mode
// and the item that keeps its fluid aren't ported.

use super::{Game, mek_world, native_hooks::NativeFluidTankInfo};
use horizon_client::containers::ModBlockEntity;
use horizon_core::geometry::Direction as Face;
use horizon_world::{
    fluid_stack::FluidStack,
    level::flags,
    nbt::{Compound, Tag},
};
use mekanism_energy::{
    container::Automation,
    fluid_tank::FluidTank,
    transmitter::FluidTankTier,
};
use serde_json::{Value, json};

pub(super) fn tier_of(block: &str) -> Option<FluidTankTier> {
    FluidTankTier::of_block(block)
}

fn rules(tier: FluidTankTier) -> FluidTank {
    let (capacity, output) = super::mek_pipes::settings().fluid_tank(tier);
    FluidTank { capacity, output, creative: tier == FluidTankTier::Creative }
}

/// `native_block_entity`: an empty, inactive tank.
pub(super) fn new_entity(block: &str) -> (String, Compound) {
    (block.to_owned(), Compound::new())
}

/// `active_state`: whether it gives down.
pub(super) fn active(nbt: &Compound) -> bool {
    mek_world::number(nbt.get(&mek_world::key("active_state"))).is_some_and(|v| v != 0)
}

/// `onSneakRightClick`: on or off, shown on the block at once. What it is now.
pub(super) fn toggle(game: &mut Game<'_>, pos: [i32; 3]) -> Option<bool> {
    let now = game
        .native_with_mod_block_entity(pos, |m| {
            tier_of(&m.id)?;
            let now = !active(&m.nbt);
            m.nbt.insert(mek_world::key("active_state"), Tag::Byte(i8::from(now)));
            Some(now)
        })
        .flatten()?;
    game.native_set_block_property_with(pos, "active", if now { "true" } else { "false" }, flags::UPDATE_ALL);
    Some(now)
}

/// A server tick: an active tank gives down what the block below takes, at
/// most its output (and what a fluid tank there still takes).
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, tier: FluidTankTier) {
    let shown = game.native_block_property(pos, "active");
    let on = active(&entity.nbt);
    if shown.as_deref() != Some(if on { "true" } else { "false" }) {
        game.native_set_block_property_with(pos, "active", if on { "true" } else { "false" }, flags::UPDATE_ALL);
    }
    if !on {
        return;
    }
    let tank = rules(tier);
    let held = mek_world::saved_tank(&entity.nbt);
    if held.is_empty() {
        return;
    }
    let below = [pos[0], pos[1] - 1, pos[2]];
    let below_needed = game
        .native_with_mod_block_entity(below, |m| {
            let t = rules(tier_of(&m.id)?);
            Some(t.capacity - mek_world::saved_tank(&m.nbt).amount().max(0))
        })
        .flatten();
    let limit = tank.eject_limit(below_needed);
    let offered = tank.extract(held.amount(), limit, Automation::Internal, false).amount;
    if offered <= 0 {
        return;
    }
    let dimension = game.native_dimension();
    let sent = game.native_fluid_fill(dimension, below, Some(Face::Up), &held.copy_with_amount(offered), false).clamp(0, offered);
    if sent > 0 {
        let left = tank.extract(held.amount(), sent, Automation::Internal, true).stored;
        mek_world::write_tank(&mut entity.nbt, &held.copy_with_amount(left));
    }
}

/// `native_fluid_handler_tanks` of a fluid tank: its one tank on every side.
/// Whether `entity` is a fluid tank.
pub(super) fn fluid_tanks(face: Option<Face>, entity: &ModBlockEntity, tanks: &mut Option<Vec<NativeFluidTankInfo>>) -> bool {
    let Some(tier) = tier_of(&entity.id) else {
        return false;
    };
    let stack = mek_world::saved_tank(&entity.nbt);
    let drainable = !stack.is_empty() && face.is_some();
    *tanks = Some(vec![NativeFluidTankInfo { stack, capacity: rules(tier).capacity, drainable }]);
    true
}

/// `native_fluid_handler_fill` of a fluid tank (`EXTERNAL`): what fits, none
/// of another fluid; the null side's proxy is read only. Whether `entity` is
/// a fluid tank.
pub(super) fn fluid_fill(face: Option<Face>, entity: &mut ModBlockEntity, resource: &FluidStack, simulate: bool, filled: &mut Option<i32>) -> bool {
    let Some(tier) = tier_of(&entity.id) else {
        return false;
    };
    if face.is_none() || resource.is_empty() {
        *filled = Some(0);
        return true;
    }
    let held = mek_world::saved_tank(&entity.nbt);
    let same = held.is_empty() || FluidStack::is_same_fluid_same_components(&held, resource);
    let moved = rules(tier).insert(held.amount().max(0), same, resource.amount(), Automation::External, !simulate);
    if !simulate && moved.stored != held.amount().max(0) {
        let after = if held.is_empty() { resource.copy_with_amount(moved.stored) } else { held.copy_with_amount(moved.stored) };
        mek_world::write_tank(&mut entity.nbt, &after);
    }
    *filled = Some(moved.amount);
    true
}

/// `native_fluid_handler_drain` of a fluid tank (`EXTERNAL`): `drain(FluidStack)`
/// only of what it holds; the null side gives nothing. Whether `entity` is a
/// fluid tank.
pub(super) fn fluid_drain(face: Option<Face>, entity: &mut ModBlockEntity, resource: Option<&FluidStack>, max: i32, simulate: bool, drained: &mut Option<FluidStack>) -> bool {
    let Some(tier) = tier_of(&entity.id) else {
        return false;
    };
    let held = mek_world::saved_tank(&entity.nbt);
    if face.is_none() || held.is_empty() || resource.is_some_and(|r| !FluidStack::is_same_fluid_same_components(r, &held)) {
        *drained = Some(FluidStack::EMPTY);
        return true;
    }
    let wanted = resource.map_or(max, FluidStack::amount);
    let moved = rules(tier).extract(held.amount(), wanted, Automation::External, !simulate);
    if !simulate && moved.stored != held.amount() {
        mek_world::write_tank(&mut entity.nbt, &held.copy_with_amount(moved.stored));
    }
    *drained = Some(held.copy_with_amount(moved.amount));
    true
}

/// What `/mek probe` answers for a fluid tank.
pub(super) fn probe(game: &mut Game<'_>, pos: [i32; 3]) -> Option<Value> {
    game.native_with_mod_block_entity(pos, |m| {
        let tier = tier_of(&m.id)?;
        let held = mek_world::saved_tank(&m.nbt);
        let (capacity, output) = super::mek_pipes::settings().fluid_tank(tier);
        Some(json!({
            "pos": pos, "id": m.id, "tier": tier.name(), "fluid": (!held.is_empty()).then(|| held.name()), "amount": held.amount().max(0),
            "capacity": capacity, "output": output, "active": active(&m.nbt),
        }))
    })
    .flatten()
}
