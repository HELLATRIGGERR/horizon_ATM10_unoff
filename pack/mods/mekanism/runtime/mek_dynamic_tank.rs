// Mekanism's Dynamic Tank in the game (support module `crate::game::mek_dynamic_tank`
// of the `mekanism` native bundle). The rules are `mekanism_energy::dynamic_tank`
// (MIT, from Mekanism 10.7.19, Copyright (c) 2017-2025 Aidan C. Brady); the
// multiblock forms through `mek_multiblock`, and its fluid or chemical lives
// in the cache by its id there (`fluid_tanks`, `chemical_tanks`, `items`,
// `edit_mode`). This module is the glue: its size from `general.toml`
// `[dynamic_tank]`, the valves' fluid handler (hook points API 422) and
// chemical handler on every side, without a rate (`TileEntityDynamicValve`),
// the two slots that fill and empty containers as its edit mode says
// (`HybridInventorySlot.handleTank`, `drainChemicalTank`, `fillChemicalTank`),
// a held bucket used on it (`FluidUtils.handleTankInteraction`), its screen
// (`GuiDynamicTank`, with the edit mode tab) and `/mek probe`.

use super::{
    Game, mek_chemicals,
    mek_multiblock::{self as mb, Kind},
    mek_ui::{self, Pen, Rect},
    mek_world,
    native_hooks::{NativeFluidTankInfo, NativeScreen},
};
use horizon_client::{hud::HudBatch, items::ItemStack};
use horizon_core::geometry::Direction as Face;
use horizon_world::fluid_stack::FluidStack;
use mekanism_energy::{
    chemical::{ChemicalHandler, ChemicalRegistry, ChemicalStack, ChemicalTank, TankRules},
    chemical_tank::{self, OwnTank},
    config,
    display,
    dynamic_tank::{self, Current, EditMode, TankConfig},
    multiblock::{Pos, Uuid},
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::HashMap,
    time::{Duration, Instant},
};

pub(super) const VALVE: &str = "mekanism:dynamic_valve";
const CASING: &str = "mekanism:dynamic_tank";

thread_local! {
    static SETTINGS: RefCell<Option<(TankConfig, config::Stamp, Instant)>> = const { RefCell::new(None) };
    /// The slots' `isDraining` and `isFilling` (a container being worked on over several ticks), by multiblock.
    static FLAGS: RefCell<HashMap<Uuid, (bool, bool)>> = RefCell::new(HashMap::new());
}

/// `general.toml` `[dynamic_tank]`, read again within a second of a change.
pub(super) fn settings() -> TankConfig {
    let dir = Game::native_config_dir().ok();
    SETTINGS.with(|s| {
        let mut s = s.borrow_mut();
        let stale = match s.as_mut() {
            None => true,
            Some((_, stamp, looked)) if looked.elapsed() >= Duration::from_secs(1) => {
                *looked = Instant::now();
                dir.as_ref().is_some_and(|d| config::stamp(d) != *stamp)
            }
            Some(_) => false,
        };
        if stale {
            *s = Some(match &dir {
                Some(d) => (TankConfig::load(d), config::stamp(d), Instant::now()),
                None => (TankConfig::default(), [None; config::FILES.len()], Instant::now()),
            });
        }
        s.as_ref().map(|(c, _, _)| *c).unwrap_or_default()
    })
}

pub(super) fn is_tank_block(block: &str) -> bool {
    block == CASING || block == VALVE
}

/// The tank's fluid and chemical capacities (`setVolume`).
fn capacities(dimension: &str, id: Uuid) -> Option<(i32, i64)> {
    let volume = mb::with_formed(dimension, id, |f| f.volume())?;
    let c = settings();
    Some((c.fluid_capacity(volume), c.chemical_capacity(volume)))
}

/// `onCreated`: what the cache brings is capped to the tank's size (`shouldCap`).
pub(super) fn created(dimension: &str, id: Uuid) {
    let Some((fluid_cap, chemical_cap)) = capacities(dimension, id) else { return };
    mb::with_cache(dimension, Kind::Tank, id, |c| {
        if let Some(f) = c.fluids.get_mut(0)
            && f.amount() > fluid_cap
        {
            *f = f.copy_with_amount(fluid_cap);
        }
        if let Some(s) = c.chemicals.get_mut(0)
            && s.amount() > chemical_cap
        {
            s.set_amount(chemical_cap);
        }
    });
}

/// The tank a valve at `pos` reaches: Some(None) for a valve of no formed tank (no tanks at all).
fn valve_of(dimension: &str, pos: Pos, block: &str) -> Option<Option<Uuid>> {
    (block == VALVE).then(|| mb::formed_at(dimension, Kind::Tank, pos))
}

fn held(dimension: &str, id: Uuid) -> (FluidStack, ChemicalStack) {
    mb::with_cache(dimension, Kind::Tank, id, |c| (c.fluids.first().cloned().unwrap_or(FluidStack::EMPTY), c.chemicals.first().cloned().unwrap_or(ChemicalStack::EMPTY)))
        .unwrap_or((FluidStack::EMPTY, ChemicalStack::EMPTY))
}

// ---- The valves' fluid handler (hook points API 422) --------------------------------------------------

/// `native_fluid_handler_tanks` of a dynamic valve: the tank's fluid tank on every side once formed, none before.
pub(super) fn fluid_tanks(dimension: &str, pos: Pos, face: Option<Face>, block: &str, tanks: &mut Option<Vec<NativeFluidTankInfo>>) -> bool {
    let Some(owner) = valve_of(dimension, pos, block) else { return false };
    *tanks = Some(match owner.and_then(|id| Some((id, capacities(dimension, id)?))) {
        None => Vec::new(),
        Some((id, (capacity, _))) => {
            let (stack, _) = held(dimension, id);
            let drainable = !stack.is_empty() && face.is_some();
            vec![NativeFluidTankInfo { stack, capacity, drainable }]
        }
    });
    true
}

/// `native_fluid_handler_fill`: what fits of a fluid, when no chemical is in; the null side reads only.
pub(super) fn fluid_fill(dimension: &str, pos: Pos, face: Option<Face>, block: &str, resource: &FluidStack, simulate: bool, filled: &mut Option<i32>) -> bool {
    let Some(owner) = valve_of(dimension, pos, block) else { return false };
    let (Some(id), Some(_)) = (owner, face) else {
        *filled = Some(0);
        return true;
    };
    let Some((capacity, _)) = capacities(dimension, id) else {
        *filled = Some(0);
        return true;
    };
    let took = mb::with_cache(dimension, Kind::Tank, id, |c| {
        let chemical = c.chemicals.first().is_some_and(|s| !s.is_empty());
        let Some(tank) = c.fluids.get_mut(0) else { return 0 };
        let same = tank.is_empty() || FluidStack::is_same_fluid_same_components(tank, resource);
        let took = dynamic_tank::fluid_room(tank.amount().max(0), capacity, same, chemical, resource.amount());
        if !simulate && took > 0 {
            *tank = if tank.is_empty() { resource.copy_with_amount(took) } else { tank.copy_with_amount(tank.amount() + took) };
        }
        took
    })
    .unwrap_or(0);
    *filled = Some(took);
    true
}

/// `native_fluid_handler_drain`: what it holds, of the fluid asked for; the null side gives nothing.
#[allow(clippy::too_many_arguments)]
pub(super) fn fluid_drain(
    dimension: &str,
    pos: Pos,
    face: Option<Face>,
    block: &str,
    resource: Option<&FluidStack>,
    max: i32,
    simulate: bool,
    drained: &mut Option<FluidStack>,
) -> bool {
    let Some(owner) = valve_of(dimension, pos, block) else { return false };
    let (Some(id), Some(_)) = (owner, face) else {
        *drained = Some(FluidStack::EMPTY);
        return true;
    };
    let got = mb::with_cache(dimension, Kind::Tank, id, |c| {
        let Some(tank) = c.fluids.get_mut(0) else { return FluidStack::EMPTY };
        if tank.is_empty() || resource.is_some_and(|r| !FluidStack::is_same_fluid_same_components(r, tank)) {
            return FluidStack::EMPTY;
        }
        let n = resource.map_or(max, FluidStack::amount).min(tank.amount()).max(0);
        let got = tank.copy_with_amount(n);
        if !simulate && n > 0 {
            let left = tank.amount() - n;
            *tank = if left > 0 { tank.copy_with_amount(left) } else { FluidStack::EMPTY };
        }
        got
    })
    .unwrap_or(FluidStack::EMPTY);
    *drained = Some(got);
    true
}

// ---- The valves' chemical handler -------------------------------------------------------------------------

/// The tank's chemical tank through a valve (`ChemicalTankWrapper` over a `VariableCapacityChemicalTank`): one
/// tank once formed, taking a chemical (not a radioactive one) only while no fluid is in; the null side reads only.
struct ValveChemical<'r> {
    stack: ChemicalStack,
    capacity: i64,
    formed: bool,
    fluid_held: bool,
    side: bool,
    registry: &'r ChemicalRegistry,
}

impl ChemicalHandler for ValveChemical<'_> {
    fn tanks(&self) -> usize {
        usize::from(self.formed)
    }

    fn in_tank(&self, _: usize) -> ChemicalStack {
        self.stack.clone()
    }

    fn capacity(&self, _: usize) -> i64 {
        self.capacity
    }

    fn is_valid(&self, _: usize, stack: &ChemicalStack) -> bool {
        self.registry.passes_default(stack)
    }

    fn insert_tank(&mut self, _: usize, stack: &ChemicalStack, simulate: bool) -> ChemicalStack {
        if !self.formed || !self.side || self.fluid_held || stack.is_empty() || !self.registry.passes_default(stack) {
            return stack.clone();
        }
        if !self.stack.is_empty() && !ChemicalStack::is_same_chemical(&self.stack, stack) {
            return stack.clone();
        }
        let took = stack.amount().min(self.capacity.saturating_sub(self.stack.amount().max(0)).max(0));
        if took <= 0 {
            return stack.clone();
        }
        if !simulate {
            self.stack = if self.stack.is_empty() { stack.copy_with_amount(took) } else { self.stack.copy_with_amount(self.stack.amount() + took) };
        }
        stack.copy_with_amount(stack.amount() - took).normalized()
    }

    fn extract_tank(&mut self, _: usize, amount: i64, simulate: bool) -> ChemicalStack {
        if !self.formed || !self.side || self.stack.is_empty() || amount <= 0 {
            return ChemicalStack::EMPTY;
        }
        let n = amount.min(self.stack.amount());
        let got = self.stack.copy_with_amount(n);
        if !simulate {
            self.stack = self.stack.copy_with_amount(self.stack.amount() - n).normalized();
        }
        got
    }
}

/// `Capabilities.CHEMICAL` of a dynamic valve at `pos` through `face`: None for any other block.
pub(super) fn chemical_handler<R>(dimension: &str, pos: Pos, block: &str, face: Option<mekanism_energy::sides::Direction>, registry: &ChemicalRegistry, use_it: impl FnOnce(&mut dyn ChemicalHandler) -> R) -> Option<R> {
    let owner = valve_of(dimension, pos, block)?;
    let caps = owner.and_then(|id| Some((id, capacities(dimension, id)?)));
    let (fluid, stack) = caps.map_or((FluidStack::EMPTY, ChemicalStack::EMPTY), |(id, _)| held(dimension, id));
    let mut handler = ValveChemical {
        stack: stack.clone(),
        capacity: caps.map_or(0, |(_, (_, c))| c),
        formed: caps.is_some(),
        fluid_held: !fluid.is_empty(),
        side: face.is_some(),
        registry,
    };
    let answer = use_it(&mut handler);
    if let Some((id, _)) = caps
        && handler.stack != stack
    {
        let now = handler.stack;
        mb::with_cache(dimension, Kind::Tank, id, |c| {
            if let Some(s) = c.chemicals.get_mut(0) {
                *s = now;
            }
        });
    }
    Some(answer)
}

// ---- The slots ------------------------------------------------------------------------------------------------

/// The fluid side of the merged tank as the slots see it (`INTERNAL`: no rate).
struct Fluid<'a> {
    tank: &'a mut FluidStack,
    capacity: i32,
    chemical: bool,
}

impl Fluid<'_> {
    /// What of `amount` of `fluid` would go in.
    fn room(&self, fluid: &FluidStack, amount: i32) -> i32 {
        let same = self.tank.is_empty() || FluidStack::is_same_fluid(self.tank, fluid);
        dynamic_tank::fluid_room(self.tank.amount().max(0), self.capacity, same, self.chemical, amount)
    }

    fn put(&mut self, fluid: &FluidStack, amount: i32) {
        *self.tank = if self.tank.is_empty() { fluid.copy_with_amount(amount) } else { self.tank.copy_with_amount(self.tank.amount() + amount) };
    }

    fn take(&mut self, amount: i32) {
        let left = self.tank.amount() - amount;
        *self.tank = if left > 0 { self.tank.copy_with_amount(left) } else { FluidStack::EMPTY };
    }
}

fn one(stack: &ItemStack) -> ItemStack {
    let mut s = stack.clone();
    s.count = 1;
    s
}

/// `moveItem`: one of the input goes to the output as `moved` (into an empty output, or onto the same item below its
/// stack size).
fn move_item(game: &Game<'_>, input: &mut Option<ItemStack>, output: &mut Option<ItemStack>, moved: ItemStack) -> bool {
    match output.as_mut() {
        None => *output = Some(moved),
        Some(out) => {
            let max = game.native_max_stack_size(&out.item).unwrap_or(64).min(64);
            if out.item != moved.item || out.data() != moved.data() || out.count >= max {
                return false;
            }
            out.count += 1;
        }
    }
    if let Some(i) = input.as_mut() {
        i.count -= 1;
        if i.count == 0 {
            *input = None;
        }
    }
    true
}

/// `fillTank(outputSlot)` (`drainItemAndMove`): the input container empties into the tank, what fits; a container
/// left with fluid stays in the input (`filling`), an empty one goes to the output.
fn empty_container(game: &Game<'_>, input: &mut Option<ItemStack>, output: &mut Option<ItemStack>, tank: &mut Fluid<'_>, filling: &mut bool) {
    let Some(stack) = input.clone() else { return };
    let Some(held) = game.native_item_fluid(&one(&stack)) else { return };
    let Some(fluid) = held.fluid.as_deref().and_then(|f| FluidStack::of(f, held.amount)).filter(|f| !f.is_empty()) else { return };
    let room = tank.room(&fluid, fluid.amount());
    if room <= 0 {
        return;
    }
    let Some((_, took, after)) = game.native_item_fluid_drain(&one(&stack), room) else { return };
    if took <= 0 {
        return;
    }
    if stack.count == 1 && game.native_item_fluid(&after).is_some_and(|f| f.amount > 0) {
        *input = Some(after);
        tank.put(&fluid, took);
        *filling = true;
        return;
    }
    if move_item(game, input, output, after) {
        tank.put(&fluid, took);
    }
}

/// `drainTank(outputSlot)`: the input container fills from the tank; one that takes more stays in the input
/// (`draining`), a full one goes to the output.
fn fill_container(game: &Game<'_>, input: &mut Option<ItemStack>, output: &mut Option<ItemStack>, tank: &mut Fluid<'_>, draining: &mut bool) {
    let Some(stack) = input.clone() else { return };
    if game.native_item_fluid(&one(&stack)).is_none() || tank.tank.is_empty() {
        return;
    }
    let fluid = tank.tank.clone();
    let Some((took, after)) = game.native_item_fluid_fill(&one(&stack), fluid.name(), fluid.amount()) else { return };
    if took <= 0 {
        return;
    }
    if stack.count == 1 && game.native_item_fluid_fill(&after, fluid.name(), fluid.amount() - took).is_some_and(|(more, _)| more > 0) {
        *input = Some(after);
        *draining = true;
        tank.take(took);
        return;
    }
    if move_item(game, input, output, after) {
        tank.take(took);
        *draining = false;
    }
}

/// `IFluidHandlerSlot.handleTank(outputSlot, editMode)`.
fn handle_tank(game: &Game<'_>, input: &mut Option<ItemStack>, output: &mut Option<ItemStack>, mode: EditMode, tank: &mut Fluid<'_>, flags: &mut (bool, bool)) {
    let Some(stack) = input.clone().filter(|s| s.count > 0) else { return };
    match mode {
        EditMode::Fill => fill_container(game, input, output, tank, &mut flags.0),
        EditMode::Empty => empty_container(game, input, output, tank, &mut flags.1),
        EditMode::Both => {
            let Some(held) = game.native_item_fluid(&one(&stack)) else { return };
            let fluid = held.fluid.as_deref().and_then(|f| FluidStack::of(f, held.amount)).filter(|f| !f.is_empty());
            let has_empty = fluid.is_none();
            if let Some(fluid) = &fluid
                && !flags.0
                && tank.room(fluid, fluid.amount()) > 0
            {
                empty_container(game, input, output, tank, &mut flags.1);
                return;
            }
            if flags.1 {
                if move_item(game, input, output, stack.clone()) {
                    flags.1 = false;
                }
            } else if (tank.tank.is_empty() && has_empty)
                || flags.0
                || (!tank.tank.is_empty() && game.native_item_fluid_fill(&one(&stack), tank.tank.name(), tank.tank.amount()).is_some_and(|(n, _)| n > 0))
            {
                fill_container(game, input, output, tank, &mut flags.0);
            }
        }
    }
}

/// `TankMultiblockData.tick`: the slots by what the tank holds (`MergedTank.CurrentType`): a fluid goes through
/// `handleTank`, a chemical out into a chemical item in the input slot (`drainChemicalTank`) and in from one in the
/// output slot (`fillChemicalTank`); an empty tank tries all.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, id: Uuid) {
    let Some((fluid_cap, chemical_cap)) = capacities(dimension, id) else { return };
    let Some(before) = mb::with_cache(dimension, Kind::Tank, id, |c| c.clone()) else { return };
    if before.items.iter().all(Option::is_none) {
        return;
    }
    let mut cache = before.clone();
    let mode = EditMode::by_id(cache.edit_mode);
    let mut flags = FLAGS.with(|f| f.borrow().get(&id).copied().unwrap_or_default());
    let current = Current::of(i64::from(cache.fluids[0].amount().max(0)), cache.chemicals[0].amount().max(0));
    let (mut input, mut output) = (cache.items[0].take(), cache.items[1].take());
    if matches!(current, Current::Empty | Current::Fluid) {
        let chemical = !cache.chemicals[0].is_empty();
        let mut tank = Fluid { tank: &mut cache.fluids[0], capacity: fluid_cap, chemical };
        handle_tank(game, &mut input, &mut output, mode, &mut tank, &mut flags);
    }
    if matches!(current, Current::Empty | Current::Chemical) && cache.fluids[0].is_empty() {
        let registry = mek_chemicals::registry(game);
        let (energy, _) = mek_world::energy_config();
        let valid = |s: &ChemicalStack| registry.passes_default(s);
        let rules = TankRules { can_extract: &mekanism_energy::chemical::always, can_insert: &mekanism_energy::chemical::always, valid: &valid };
        let mut tank = ChemicalTank::with(chemical_cap, cache.chemicals[0].clone());
        for (slot, drain) in [(&mut input, true), (&mut output, false)] {
            let Some(stack) = slot.as_mut() else { continue };
            super::mek_tanks::with_item(stack, &registry, &energy, |item| {
                let mut own = OwnTank { tank: &mut tank, rules: &rules, block: None };
                if drain { chemical_tank::drain_into_item(item, &mut own) } else { chemical_tank::fill_from_item(item, &mut own) }
            });
        }
        cache.chemicals[0] = tank.stored.normalized();
    }
    cache.items[0] = input;
    cache.items[1] = output;
    FLAGS.with(|f| f.borrow_mut().insert(id, flags));
    if cache != before {
        mb::with_cache(dimension, Kind::Tank, id, |c| *c = cache);
    }
}

/// A plain use with a fluid container in hand on a formed tank (`TileEntityDynamicTank.onActivate`:
/// `FluidUtils.handleTankInteraction`): a full bucket empties into it, an empty one fills, before any screen opens.
/// Whether it did.
pub(super) fn use_container(game: &mut Game<'_>, dimension: &str, id: Uuid) -> bool {
    let Some(stack) = game.native_used_stack().clone().filter(|s| s.count > 0) else { return false };
    let Some(held) = game.native_item_fluid(&one(&stack)) else { return false };
    let Some((fluid_cap, _)) = capacities(dimension, id) else { return false };
    let Some(before) = mb::with_cache(dimension, Kind::Tank, id, |c| c.clone()) else { return false };
    let mut cache = before.clone();
    let chemical = !cache.chemicals[0].is_empty();
    let mut tank = Fluid { tank: &mut cache.fluids[0], capacity: fluid_cap, chemical };
    let result = match held.fluid.as_deref().and_then(|f| FluidStack::of(f, held.amount)).filter(|f| !f.is_empty()) {
        Some(fluid) => {
            let room = tank.room(&fluid, fluid.amount());
            game.native_item_fluid_drain(&one(&stack), room).filter(|(_, n, _)| *n > 0).map(|(_, n, after)| {
                tank.put(&fluid, n);
                after
            })
        }
        None if !tank.tank.is_empty() => {
            let fluid = tank.tank.clone();
            game.native_item_fluid_fill(&one(&stack), fluid.name(), fluid.amount()).filter(|(n, _)| *n > 0).map(|(n, after)| {
                tank.take(n);
                after
            })
        }
        None => None,
    };
    let Some(after) = result else { return false };
    if !game.native_instabuild() {
        let hand = game.native_used_stack();
        if let Some(h) = hand.as_mut() {
            h.count -= 1;
        }
        if hand.as_ref().is_some_and(|h| h.count == 0) {
            *hand = Some(after);
        } else {
            let player = game.native_acting_id();
            if let Some(rest) = game.native_give_stack(player, after) {
                let at = game.native_player().map(|p| [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32]).unwrap_or([0, 0, 0]);
                game.native_pop_resource(at, rest);
            }
        }
    }
    mb::with_cache(dimension, Kind::Tank, id, |c| *c = cache);
    true
}

/// `native_block_entity_slot_accepts` of a tank's slots: fluid containers (buckets) and chemical tanks.
pub(super) fn slot_accepts(slot: usize, stack: &ItemStack) -> bool {
    let item = &*stack.item;
    let fluid = item == "minecraft:bucket" || horizon_world::fluid_stack::bucket_fluid(item).is_some();
    let chemical = super::mek_tanks::tier_of(item).is_some();
    slot < 2 && (fluid || chemical)
}

// ---- The screen (`GuiDynamicTank`) -----------------------------------------------------------------------------

/// The screen's data slots: what it holds (0 nothing, 1 a fluid, 2 a chemical), the fluid's or chemical's number,
/// the amount and the capacity (four 16-bit pieces each), the edit mode, the volume.
const DATA: usize = 12;

fn pack(current: i32, number: i32, amount: u64, capacity: u64, mode: i32, volume: i32) -> Vec<i32> {
    let mut out = vec![current, number];
    for v in [amount, capacity] {
        out.extend((0..4).map(|i| ((v >> (16 * i)) & 0xFFFF) as i32));
    }
    out.push(mode);
    out.push(volume);
    out
}

struct View {
    current: i32,
    number: i32,
    amount: u64,
    capacity: u64,
    mode: EditMode,
    volume: i32,
}

fn unpack(data: &[i32]) -> Option<View> {
    if data.len() < DATA {
        return None;
    }
    let long = |at: usize| (0..4).fold(0u64, |v, i| v | (u64::from(data[at + i] as u16) << (16 * i)));
    Some(View { current: data[0], number: data[1], amount: long(2), capacity: long(6), mode: EditMode::by_id(data[10]), volume: data[11] })
}

/// The data slots of a formed tank, for `mek_multiblock` to show on its blocks.
pub(super) fn view(game: &Game<'_>, dimension: &str, id: Uuid) -> Vec<i32> {
    let Some((fluid_cap, chemical_cap)) = capacities(dimension, id) else { return Vec::new() };
    let volume = mb::with_formed(dimension, id, |f| f.volume()).unwrap_or(0);
    let (fluid, chemical) = held(dimension, id);
    let mode = mb::with_cache(dimension, Kind::Tank, id, |c| c.edit_mode).unwrap_or(0);
    if !fluid.is_empty() {
        return pack(1, i32::from(fluid.fluid().id()), fluid.amount().max(0) as u64, fluid_cap.max(0) as u64, mode, volume);
    }
    if !chemical.is_empty() {
        let number = mek_chemicals::registry(game).number(chemical.id());
        return pack(2, number, chemical.amount().max(0) as u64, chemical_cap.max(0) as u64, mode, volume);
    }
    pack(0, 0, 0, chemical_cap.max(0) as u64, mode, volume)
}

const GAUGE: Rect = [7.0, 16.0, 34.0, 56.0];
const SCREEN: Rect = [49.0, 21.0, 84.0, 46.0];
const SLOTS: [[f32; 2]; 2] = [[146.0, 21.0], [146.0, 51.0]];
const HOLDER: Rect = [141.0, 16.0, 26.0, 56.0];
/// `GuiContainerEditModeTab`: an inset tab right of the panel.
const MODE_TAB: Rect = [176.0, 138.0, 26.0, 18.0];
const MODE_BUTTON: Rect = [178.0, 138.0, 18.0, 18.0];

/// `native_screen` of a casing or valve.
pub(super) fn screen() -> NativeScreen {
    NativeScreen {
        width: 176.0,
        height: 166.0,
        slots: vec![Some(SLOTS[0]), Some(SLOTS[1]), None, None, None],
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: true,
        own_background: false,
    }
}

/// The edit mode's next (left click) or previous (right click).
pub(super) const NEXT_MODE: i32 = 7000;
pub(super) const PREVIOUS_MODE: i32 = 7001;

pub(super) fn click(at: [f32; 2], right: bool, button: &mut Option<i32>) {
    if mek_ui::inside(MODE_BUTTON, at) {
        *button = Some(if right { PREVIOUS_MODE } else { NEXT_MODE });
    }
}

/// `native_menu_button` on a casing or valve: the edit mode (`nextMode`, `previousMode`).
pub(super) fn button(game: &mut Game<'_>, pos: Pos, id: i32) {
    if id != NEXT_MODE && id != PREVIOUS_MODE {
        return;
    }
    let dimension = game.native_dimension();
    let Some(owner) = mb::formed_at(dimension, Kind::Tank, pos) else { return };
    mb::with_cache(dimension, Kind::Tank, owner, |c| {
        let mode = EditMode::by_id(c.edit_mode);
        c.edit_mode = if id == NEXT_MODE { mode.next() } else { mode.previous() }.id();
    });
}

fn stored_name(view: &View) -> Option<String> {
    match view.current {
        1 => horizon_world::fluid::FluidKind::from_id(view.number as u32).map(|f| title_case(f.name())),
        2 => mek_chemicals::loaded().as_deref().and_then(|r| r.by_number(view.number).map(mek_chemicals::name)),
        _ => None,
    }
}

fn title_case(id: &str) -> String {
    let path = id.split_once(':').map_or(id, |(_, p)| p);
    path.split('_').map(|w| {
        let mut c = w.chars();
        c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
    }).collect::<Vec<_>>().join(" ")
}

/// The inner screen's lines (`GuiInnerScreen` of `GuiDynamicTank`).
fn lines(view: &View) -> Vec<String> {
    let mut out = Vec::new();
    match stored_name(view) {
        None => out.push("Empty".into()),
        Some(name) => {
            out.push(format!("{name}:"));
            out.push(format!("{} mB", display::thousands(view.amount)));
        }
    }
    out.push("Capacity:".into());
    out.push(format!("{} mB", display::thousands(view.capacity)));
    out
}

pub(super) fn tooltip(data: &[i32], at: [f32; 2], tips: &mut Vec<String>) {
    let Some(view) = unpack(data) else { return };
    if mek_ui::inside(MODE_BUTTON, at) {
        tips.push(format!("Container Edit Mode: {}", view.mode.name()));
    } else if mek_ui::inside(GAUGE, at) {
        tips.push(match stored_name(&view) {
            None => "Empty".into(),
            Some(name) => format!("{name}: {} mB", display::thousands(view.amount)),
        });
    } else if mek_ui::inside(SCREEN, at) {
        tips.push(format!("Volume: {}", view.volume));
    }
}

fn color(game: &Game<'_>, view: &View) -> u32 {
    match view.current {
        1 => match horizon_world::fluid::FluidKind::from_id(view.number as u32) {
            Some(horizon_world::fluid::FluidKind::Lava) => 0xD45A12,
            Some(f) if f.name() != "minecraft:water" => {
                horizon_world::mod_fluids::mod_fluids().iter().find(|m| m.id == f.name()).map_or(0x3F76E4, |m| m.tint & 0xFF_FFFF)
            }
            _ => 0x3F76E4,
        },
        2 => mek_chemicals::registry(game).by_number(view.number).map_or(0x808080, |c| c.color & 0xFF_FFFF),
        _ => 0,
    }
}

pub(super) fn draw(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let view = unpack(data).unwrap_or(View { current: 0, number: 0, amount: 0, capacity: 0, mode: EditMode::Both, volume: 0 });
    let cursor = game.native_cursor();
    let cursor = [cursor[0] - origin[0], cursor[1] - origin[1]];
    let mut pen = Pen { game, batch, origin, cursor };
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [0.0, 0.0, 176.0, 166.0]);
    // The merged tank's medium gauge, filled from the bottom in the fluid's or chemical's colour.
    pen.nine("gauge/normal", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], GAUGE);
    let inner = GAUGE[3] - 2.0;
    let level = if view.capacity == 0 { 0.0 } else { (view.amount as f64 / view.capacity as f64).min(1.0) };
    let h = (level * f64::from(inner)) as i32 as f32;
    if h > 0.0 {
        pen.fill([GAUGE[0] + 1.0, GAUGE[1] + 1.0 + inner - h, GAUGE[2] - 2.0, h], 0xFF00_0000 | color(game, &view));
    }
    pen.whole("gauge/medium", [32.0, 54.0], [GAUGE[0] + 1.0, GAUGE[1] + 1.0, 32.0, 54.0]);
    pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], SCREEN);
    for (i, line) in lines(&view).iter().enumerate() {
        pen.text(line, [SCREEN[0] + 4.0, SCREEN[1] + 3.0 + 10.0 * i as f32], mek_ui::SCREEN_TEXT);
    }
    pen.nine("element_holder", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], HOLDER);
    pen.slot("input", None, SLOTS[0][0] - 1.0, SLOTS[0][1] - 1.0);
    pen.slot("output", None, SLOTS[1][0] - 1.0, SLOTS[1][1] - 1.0);
    pen.whole("down_arrow", [8.0, 9.0], [150.0, 39.0, 8.0, 9.0]);
    pen.holder(MODE_TAB, false);
    let icon = match view.mode {
        EditMode::Both => "container_edit_mode_both",
        EditMode::Fill => "container_edit_mode_fill",
        EditMode::Empty => "container_edit_mode_empty",
    };
    pen.whole(icon, [18.0, 18.0], MODE_BUTTON);
    for row in 0..4 {
        let y = if row == 3 { 141.0 } else { 83.0 + 18.0 * row as f32 };
        for col in 0..9 {
            pen.slot("normal", None, 7.0 + 18.0 * col as f32, y);
        }
    }
    let title = game.native_screen_title().unwrap_or_else(|| "Dynamic Tank".into());
    pen.centered(&title, 0.0, 176.0, 6.0, mek_ui::TITLE);
    pen.text("Inventory", [8.0, 74.0], mek_ui::TITLE);
}

// ---- /mek probe -------------------------------------------------------------------------------------------------

pub(super) fn probe(game: &Game<'_>, dimension: &str, id: Uuid) -> Value {
    let (fluid_cap, chemical_cap) = capacities(dimension, id).unwrap_or((0, 0));
    let (fluid, chemical) = held(dimension, id);
    let (mode, items) = mb::with_cache(dimension, Kind::Tank, id, |c| {
        (c.edit_mode, c.items.iter().map(|s| s.as_ref().map(|s| json!({ "item": &*s.item, "count": s.count }))).collect::<Vec<_>>())
    })
    .unwrap_or_default();
    let _ = game;
    json!({
        "fluid": (!fluid.is_empty()).then(|| fluid.name()), "fluid_amount": fluid.amount().max(0), "fluid_capacity": fluid_cap,
        "chemical": chemical.id(), "chemical_amount": chemical.amount().max(0), "chemical_capacity": chemical_cap,
        "edit_mode": EditMode::by_id(mode).name(), "slots": items,
        "config": { "fluid_per_tank": settings().fluid_per_tank, "chemical_per_tank": settings().chemical_per_tank },
    })
}
