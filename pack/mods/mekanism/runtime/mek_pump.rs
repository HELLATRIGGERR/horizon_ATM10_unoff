// Mekanism's Electric Pump and Fluidic Plenisher in the game (support module `crate::game::mek_pump` of the
// `mekanism` native bundle). Their rules are the bundle's crate (`mekanism_energy::pump`, MIT, from Mekanism
// 10.7.19, Copyright (c) 2017-2025 Aidan C. Brady); this module is the glue: their block entities in Mekanism's
// saved form (`energy_containers`, `fluid_tanks`, `progress`, the pump's `fluid` and `recurring_nodes`, the
// plenisher's `finished`, `active_nodes` and `used_nodes`, `component_upgrade`), their ticks (the upgrade
// component, the energy slot, the fluid slot, the operation over the world's blocks, the pump's emission into the
// fluid handler above it), their Joules from cables, cubes and Forge Energy through their back, their tank as a
// fluid handler on their top (the pump's drained, the plenisher's filled), their screens (`GuiElectricPump`,
// `GuiFluidicPlenisher`) and `/mek probe`.

use super::{
    Game, mek_machines,
    mek_ui::{self, Pen},
    mek_world::{self, key},
    native_hooks::{NativeFeInfo, NativeFluidTankInfo},
};
use horizon_client::{
    containers::{ModBlockEntity, ModContainer},
    items::ItemStack,
};
use horizon_core::geometry::Direction as Face;
use horizon_world::{
    chunk::{BlockProperties, BlockState},
    fluid::FluidKind,
    fluid_stack::FluidStack,
    level::flags,
    nbt::{Compound, Tag},
};
use mekanism_energy::{
    config::EnergyConfig,
    container::{Automation, EnergyContainer},
    display,
    fe::{self, Conversion, StrictEnergy},
    machine::{Upgrade, UpgradeSlot, Upgrades},
    pump::{self, FluidMachine, Plenisher, PlenisherWorld, Pump, PumpRun, PumpWorld, Spot, Tank},
    sides::Direction,
};
use std::{cell::RefCell, collections::HashMap};

/// What a machine keeps between ticks and doesn't save: its upgrade slot's progress.
#[derive(Clone, Debug, Default)]
struct Live {
    upgrade: UpgradeSlot,
    upgrade_stack: Option<(String, u32)>,
}

thread_local! {
    static LIVE: RefCell<HashMap<(String, [i32; 3]), Live>> = RefCell::new(HashMap::new());
}

pub(super) fn kind_of(block: &str) -> Option<FluidMachine> {
    FluidMachine::of_block(block)
}

pub(super) fn forget(dimension: &str, pos: [i32; 3]) {
    LIVE.with(|l| l.borrow_mut().remove(&(dimension.to_owned(), pos)));
}

pub(super) fn clear() {
    LIVE.with(|l| l.borrow_mut().clear());
}

fn beside(pos: [i32; 3], by: [i32; 3]) -> [i32; 3] {
    [pos[0] + by[0], pos[1] + by[1], pos[2] + by[2]]
}

// ---- The saved compound ---------------------------------------------------------------------

pub(super) fn read_upgrades(nbt: &Compound) -> Upgrades {
    let mut upgrades = Upgrades::default();
    if let Some(Tag::Compound(c)) = nbt.get(&key("component_upgrade"))
        && let Some(Tag::List(list)) = c.get(&key("upgrades"))
    {
        for entry in list {
            if let Tag::Compound(u) = entry {
                let kind = Upgrade::by_id(mek_world::number(u.get(&key("type"))).unwrap_or(0) as i32);
                upgrades.set(kind, mek_world::number(u.get(&key("amount"))).unwrap_or(0).max(0) as u32);
            }
        }
    }
    upgrades
}

pub(super) fn write_upgrades(nbt: &mut Compound, upgrades: &Upgrades) {
    let list: Vec<Tag> = Upgrade::ALL
        .iter()
        .filter(|u| upgrades.get(**u) > 0)
        .map(|u| {
            let mut c = Compound::new();
            c.insert(key("type"), Tag::Int(u.ordinal()));
            c.insert(key("amount"), Tag::Int(upgrades.get(*u) as i32));
            Tag::Compound(c)
        })
        .collect();
    let mut component = Compound::new();
    if !list.is_empty() {
        component.insert(key("upgrades"), Tag::List(list));
    }
    nbt.insert(key("component_upgrade"), Tag::Compound(component));
}

/// `NBTUtils.readBlockPositions`: an int array of x, y, z triples.
fn read_positions(nbt: &Compound, name: &str) -> Vec<[i32; 3]> {
    match nbt.get(&key(name)) {
        Some(Tag::IntArray(ints)) => ints.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect(),
        _ => Vec::new(),
    }
}

fn write_positions(nbt: &mut Compound, name: &str, positions: impl Iterator<Item = [i32; 3]>) {
    let ints: Vec<i32> = positions.flatten().collect();
    if ints.is_empty() {
        nbt.remove(&key(name));
    } else {
        nbt.insert(key(name), Tag::IntArray(ints));
    }
}

fn read_tank(nbt: &Compound) -> Tank {
    let stack = mek_world::saved_tank(nbt);
    if stack.is_empty() { Tank::default() } else { Tank { fluid: Some(stack.name().to_owned()), amount: stack.amount() } }
}

/// The tank back in its saved form, keeping the stack's components when the fluid stayed.
fn write_tank(nbt: &mut Compound, tank: &Tank) {
    let before = mek_world::saved_tank(nbt);
    let after = match &tank.fluid {
        Some(f) if tank.amount > 0 && before.name() == f => before.copy_with_amount(tank.amount),
        Some(f) if tank.amount > 0 => FluidStack::of(f, tank.amount).unwrap_or(FluidStack::EMPTY),
        _ => FluidStack::EMPTY,
    };
    if after != before {
        mek_world::write_tank(nbt, &after);
    }
}

fn read_pump(nbt: &Compound) -> Pump {
    let active = match nbt.get(&key("fluid")) {
        Some(Tag::Compound(c)) => Some(FluidStack::parse_optional(c)).filter(|s| !s.is_empty()).map(|s| s.name().to_owned()),
        _ => None,
    };
    Pump {
        progress: mek_world::number(nbt.get(&key("progress"))).unwrap_or(0).max(0) as u32,
        active,
        recurring: read_positions(nbt, "recurring_nodes").into_iter().collect(),
    }
}

fn write_pump(nbt: &mut Compound, state: &Pump) {
    nbt.insert(key("progress"), Tag::Int(state.progress as i32));
    match state.active.as_deref().and_then(|f| FluidStack::of(f, 1)).and_then(|s| s.save()) {
        Some(saved) => {
            nbt.insert(key("fluid"), Tag::Compound(saved));
        }
        None => {
            nbt.remove(&key("fluid"));
        }
    }
    write_positions(nbt, "recurring_nodes", state.recurring.iter().copied());
}

fn read_plenisher(nbt: &Compound) -> Plenisher {
    Plenisher {
        progress: mek_world::number(nbt.get(&key("progress"))).unwrap_or(0).max(0) as u32,
        finished: mek_world::number(nbt.get(&key("finished"))).unwrap_or(0) != 0,
        active: read_positions(nbt, "active_nodes"),
        used: read_positions(nbt, "used_nodes").into_iter().collect(),
    }
}

fn write_plenisher(nbt: &mut Compound, state: &Plenisher) {
    nbt.insert(key("progress"), Tag::Int(state.progress as i32));
    nbt.insert(key("finished"), Tag::Byte(i8::from(state.finished)));
    write_positions(nbt, "active_nodes", state.active.iter().copied());
    write_positions(nbt, "used_nodes", state.used.iter().copied());
}

/// Its store: the base Joules by its energy upgrades (`MachineEnergyContainer.input`).
fn store(nbt: &Compound, kind: FluidMachine, config: &EnergyConfig) -> EnergyContainer {
    let (_, storage) = kind.energy(&config.pump);
    let upgrades = read_upgrades(nbt);
    let mut container = EnergyContainer::basic(upgrades.max_energy(storage, config.machines.max_upgrade_multiplier));
    container.stored = mek_world::saved_energy(nbt).unwrap_or(0).min(container.capacity);
    container
}

// ---- Block entities -------------------------------------------------------------------------

/// `native_block_entity`: a fresh machine facing north.
pub(super) fn new_entity(kind: FluidMachine) -> (String, Compound) {
    let mut nbt = Compound::new();
    nbt.insert(key("facing"), Tag::String(key(Direction::North.name())));
    (kind.id().to_owned(), nbt)
}

/// `native_block_entity_container`: its slots in a 3x3 menu whose other slots the screen leaves out; hoppers put
/// containers into the fluid slot, energy items into the energy slot, and take from the output slot.
pub(super) fn container() -> ModContainer {
    let shown = vec![pump::INPUT_SLOT, pump::OUTPUT_SLOT, pump::ENERGY_SLOT];
    ModContainer {
        slots: 9,
        faces: std::array::from_fn(|_| shown.clone()),
        inputs: vec![pump::INPUT_SLOT, pump::ENERGY_SLOT],
        outputs: vec![pump::OUTPUT_SLOT],
        menu: Some("minecraft:generic_3x3".into()),
    }
}

/// `native_block_entity_slot_accepts`: the upgrade slot takes the upgrades it supports, the energy slot energy
/// items, the output slot nothing.
pub(super) fn slot_accepts(kind: FluidMachine, slot: usize, stack: &ItemStack, accepts: &mut bool) {
    let ok = match slot {
        pump::OUTPUT_SLOT | pump::UPGRADE_OUT_SLOT => false,
        pump::ENERGY_SLOT => mek_machines::energy_item(&stack.item) || mekanism_energy::tier::CubeTier::of_block(&stack.item).is_some(),
        pump::UPGRADE_SLOT => Upgrade::of_item(&stack.item).is_some_and(|u| kind.upgrades().contains(&u)),
        _ => true,
    };
    if !ok {
        *accepts = false;
    }
}

// ---- The world, as the rules read it --------------------------------------------------------

/// The pump's and plenisher's view of the level in place.
struct Level<'a, 'g> {
    game: &'a mut Game<'g>,
    dimension: String,
}

fn liquid_block(name: &str) -> Option<FluidKind> {
    match name {
        "minecraft:water" => Some(FluidKind::Water),
        "minecraft:lava" => Some(FluidKind::Lava),
        _ => horizon_world::mod_fluids::kind_by_block(name).map(FluidKind::source),
    }
}

impl Level<'_, '_> {
    fn is_air(name: &str) -> bool {
        matches!(name, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
    }

    fn replaceable(&self, name: &str) -> bool {
        let state = BlockState { name: name.to_owned(), properties: BlockProperties::new() };
        self.game.native_server_block_in_tag(&state, "minecraft:replaceable").unwrap_or(false)
    }
}

impl PumpWorld for Level<'_, '_> {
    /// `BucketPickup` over a source: a liquid block at level 0, or a waterlogged block.
    fn source(&mut self, pos: [i32; 3]) -> Option<String> {
        let name = self.game.native_block(pos);
        if let Some(fluid) = liquid_block(&name) {
            return (self.game.native_block_property(pos, "level").as_deref() == Some("0")).then(|| fluid.name().to_owned());
        }
        (self.game.native_block_property(pos, "waterlogged").as_deref() == Some("true")).then(|| pump::WATER.to_owned())
    }

    /// `LiquidBlock.pickupBlock` (air with flags 11) and `SimpleWaterloggedBlock.pickupBlock` (not waterlogged).
    fn pick_up(&mut self, pos: [i32; 3]) -> Option<String> {
        let fluid = self.source(pos)?;
        if liquid_block(&self.game.native_block(pos)).is_some() {
            let air = self.game.native_default_state("minecraft:air")?;
            self.game.native_set_block(&self.dimension, pos, air, flags::UPDATE_ALL_IMMEDIATE).then_some(fluid)
        } else {
            self.game.native_set_block_property_with(pos, "waterlogged", "false", flags::UPDATE_ALL).then_some(fluid)
        }
    }

    /// `Collections.shuffle`, from the bundle's own random source.
    fn shuffle(&mut self, nodes: &mut [[i32; 3]]) {
        for i in (1..nodes.len()).rev() {
            let j = ((mek_machines::draw_random() * (i + 1) as f64) as usize).min(i);
            nodes.swap(i, j);
        }
    }
}

impl PlenisherWorld for Level<'_, '_> {
    fn spot(&mut self, pos: [i32; 3], fluid: Option<&str>) -> Option<Spot> {
        let (min, height) = self.game.native_world_height();
        if pos[1] < min || pos[1] >= min + height {
            return None;
        }
        let name = self.game.native_block(pos);
        if Self::is_air(&name) {
            return Some(Spot::Air);
        }
        let state = super::mek_gen2::fluid_at(self.game, pos);
        if state.kind() != FluidKind::Empty {
            return Some(if state.is_source() { Spot::Source } else { Spot::Flowing });
        }
        if self.replaceable(&name) {
            return Some(Spot::Replaceable);
        }
        let waterloggable = self.game.native_block_property(pos, "waterlogged").as_deref() == Some("false");
        Some(if waterloggable && fluid.is_none_or(|f| f == pump::WATER) { Spot::Container } else { Spot::Solid })
    }

    /// `WorldUtils.tryPlaceContainedLiquid`: none for a fluid without a block; its vapour where water evaporates;
    /// into a waterloggable block; else its source block, what was there dropping.
    fn place(&mut self, pos: [i32; 3], fluid: &str) -> bool {
        let Some(kind) = FluidKind::from_name(fluid) else {
            return false;
        };
        let block = if kind.is_water() {
            "minecraft:water".to_owned()
        } else if kind.is_lava() {
            "minecraft:lava".to_owned()
        } else {
            match kind.mod_fluid() {
                Some(m) if m.block != "minecraft:air" => m.block.clone(),
                _ => return false,
            }
        };
        let ultra_warm = self.game.native_dimension_type().ultra_warm;
        let vaporizes = match kind.mod_fluid() {
            Some(m) => m.is_vaporized_on_placement(ultra_warm),
            None => kind.is_water() && ultra_warm,
        };
        let name = self.game.native_block(pos);
        if vaporizes {
            return true;
        }
        if !Self::is_air(&name) && liquid_block(&name).is_none() && self.game.native_block_property(pos, "waterlogged").as_deref() == Some("false") {
            return kind.is_water() && self.game.native_set_block_property_with(pos, "waterlogged", "true", flags::UPDATE_ALL);
        }
        if !Self::is_air(&name) && liquid_block(&name).is_none() && let Some(state) = self.game.native_default_state(&name) {
            let mut drops = Vec::new();
            self.game.native_block_drops(pos, state, None, None, None, &mut drops);
            for stack in drops {
                self.game.native_pop_resource(pos, stack);
            }
        }
        let Some(state) = self.game.native_default_state(&block) else {
            return false;
        };
        self.game.native_set_block(&self.dimension, pos, state, flags::UPDATE_ALL_IMMEDIATE)
    }
}

// ---- The tick -------------------------------------------------------------------------------

/// The screen's data slots: Joules (4 pieces), capacity (4), Joules a tick (4), the tank's fluid (its
/// `FluidKind` number), mB (2), the pump's fluid, whether it used energy, the plenisher's `finished`.
const DATA: usize = 18;

fn push(out: &mut Vec<i32>, value: u64, pieces: usize) {
    out.extend((0..pieces).map(|i| ((value >> (16 * i)) & 0xFFFF) as i32));
}

fn read(data: &[i32], at: usize, pieces: usize) -> u64 {
    (0..pieces).fold(0, |v, i| v | (u64::from(data.get(at + i).copied().unwrap_or(0) as u16) << (16 * i)))
}

fn fluid_number(fluid: Option<&str>) -> i32 {
    fluid.and_then(FluidKind::from_name).map_or(0, |f| i32::from(f.id()))
}

/// `native_block_entity_tick`: `tickServer`'s upgrade component, then `onUpdateServer` (the energy slot, the fluid
/// slot, the operation, the pump's emission). Redstone control is IGNORED.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, kind: FluidMachine) {
    let (config, _) = mek_world::energy_config();
    mek_machines::refresh_book(game);
    let dimension = game.native_dimension().to_owned();
    let facing = game.native_block_property(pos, "facing").and_then(|f| Direction::from_name(&f)).unwrap_or(Direction::North);
    if mek_world::generator_facing(&entity.nbt) != facing || !entity.nbt.contains_key(&key("facing")) {
        entity.nbt.insert(key("facing"), Tag::String(key(facing.name())));
    }
    let id = (dimension.clone(), pos);
    let mut live = LIVE.with(|l| l.borrow_mut().remove(&id)).unwrap_or_default();

    // The upgrade component: an upgrade in its slot goes in after a second.
    let before_upgrades = read_upgrades(&entity.nbt);
    let mut upgrades = before_upgrades;
    let upgrade_stack = mek_machines::stack_in(entity, pump::UPGRADE_SLOT).map(|s| (s.item.to_string(), s.count));
    if upgrade_stack != live.upgrade_stack {
        live.upgrade.check = true;
    }
    let offered = upgrade_stack.as_ref().and_then(|(item, count)| Some((Upgrade::of_item(item)?, *count)));
    let taken = live.upgrade.tick(&mut upgrades, kind.upgrades(), offered);
    if taken > 0 {
        mek_machines::shrink(entity, pump::UPGRADE_SLOT, taken);
    }
    live.upgrade_stack = mek_machines::stack_in(entity, pump::UPGRADE_SLOT).map(|s| (s.item.to_string(), s.count));
    if upgrades != before_upgrades {
        write_upgrades(&mut entity.nbt, &upgrades);
    }

    let mut store = store(&entity.nbt, kind, &config);
    let energy_before = store.stored;
    // The energy slot (`fillContainerOrConvert`): an energy cube's item, else an item that converts to Joules.
    let mut item = mek_machines::stack_in(entity, pump::ENERGY_SLOT).cloned();
    if let Some(cube) = item.as_mut()
        && store.needed() > 0
        && let Some(taken) = mek_world::drain_energy_item(cube, store.needed()).filter(|t| *t > 0)
    {
        store.insert(taken, false, Automation::Internal);
        if let Some(cell) = entity.items.get_mut(pump::ENERGY_SLOT) {
            *cell = item;
        }
    } else if let Some(stack) = mek_machines::stack_in(entity, pump::ENERGY_SLOT).cloned()
        && store.needed() > 0
        && let Some((count, joules)) = mek_machines::energy_conversion(&stack)
        && stack.count >= count
        && store.needed() >= joules
    {
        store.insert(joules, false, Automation::Manual);
        mek_machines::shrink(entity, pump::ENERGY_SLOT, count);
    }

    let (usage, _) = kind.energy(&config.pump);
    let multiplier = config.machines.max_upgrade_multiplier;
    let per_tick = upgrades.energy_per_tick(usage, multiplier);
    let ticks = upgrades.ticks(kind.base_ticks(), multiplier);
    let mut fluid = mek_world::saved_tank(&entity.nbt);
    let (used, active, finished) = match kind {
        FluidMachine::Pump => {
            // `inputSlot.drainTank(outputSlot)`: a container takes from the tank.
            super::mek_ore_machines::drain_fluid_tank(game, entity, pump::INPUT_SLOT, pump::OUTPUT_SLOT, &mut fluid);
            mek_world::write_tank(&mut entity.nbt, &fluid);
            let mut tank = read_tank(&entity.nbt);
            let mut state = read_pump(&entity.nbt);
            let run = PumpRun { config: &config.pump, per_tick, ticks, filter: upgrades.get(Upgrade::Filter) > 0, can_function: true };
            let mut level = Level { game: &mut *game, dimension: dimension.clone() };
            let used = state.tick(pos, &mut tank, &mut store, &run, &mut level);
            write_tank(&mut entity.nbt, &tank);
            emit_up(game, pos, entity, pump::output_rate(upgrades.get(Upgrade::Speed)));
            write_pump(&mut entity.nbt, &state);
            (used, state.active, false)
        }
        FluidMachine::Plenisher => {
            // `inputSlot.fillTank(outputSlot)`: a container empties into the tank, of a fluid with a block.
            let valid = |s: &FluidStack| s.fluid().mod_fluid().is_none_or(|m| m.block != "minecraft:air");
            super::mek_ore_machines::fill_fluid_tank(game, entity, pump::INPUT_SLOT, pump::OUTPUT_SLOT, &mut fluid, pump::TANK, &valid);
            mek_world::write_tank(&mut entity.nbt, &fluid);
            let mut tank = read_tank(&entity.nbt);
            let mut state = read_plenisher(&entity.nbt);
            let mut level = Level { game: &mut *game, dimension: dimension.clone() };
            let used = state.tick(pos, &mut tank, &mut store, per_tick, ticks, config.pump.max_plenisher_nodes, &mut level);
            write_tank(&mut entity.nbt, &tank);
            write_plenisher(&mut entity.nbt, &state);
            (used, None, state.finished)
        }
    };
    if store.stored != energy_before || !entity.nbt.contains_key(&key("energy_containers")) {
        mek_world::write_energy(&mut entity.nbt, store.stored);
    }
    let tank = read_tank(&entity.nbt);
    let mut data = Vec::with_capacity(DATA);
    push(&mut data, store.stored, 4);
    push(&mut data, store.capacity, 4);
    push(&mut data, per_tick, 4);
    data.push(fluid_number(tank.fluid.as_deref()));
    push(&mut data, tank.amount.max(0) as u64, 2);
    data.push(fluid_number(active.as_deref()));
    data.push(i32::from(used > 0));
    data.push(i32::from(finished));
    entity.data = data;
    LIVE.with(|l| l.borrow_mut().insert(id, live));
}

/// `FluidUtils.emit` into the fluid handler above the pump (its down side), up to `rate` mB.
fn emit_up(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, rate: i32) {
    let fluid = mek_world::saved_tank(&entity.nbt);
    if fluid.is_empty() || rate <= 0 {
        return;
    }
    let dimension = game.native_dimension();
    let offered = fluid.copy_with_amount(fluid.amount().min(rate));
    let above = beside(pos, [0, 1, 0]);
    let side = Some(Face::Down);
    if game.native_fluid_fill(dimension, above, side, &offered, true) <= 0 {
        return;
    }
    let took = game.native_fluid_fill(dimension, above, side, &offered, false).clamp(0, offered.amount());
    if took > 0 {
        let rest = fluid.amount() - took;
        mek_world::write_tank(&mut entity.nbt, &if rest > 0 { fluid.copy_with_amount(rest) } else { FluidStack::EMPTY });
    }
}

// ---- Energy from outside --------------------------------------------------------------------

fn back(entity: &ModBlockEntity) -> Direction {
    pump::ENERGY_SIDE.direction(mek_world::generator_facing(&entity.nbt))
}

/// Whether the machine in `entity` takes energy through `face`: its back.
pub(super) fn takes_energy(entity: &ModBlockEntity, face: Direction) -> Option<bool> {
    kind_of(&entity.id)?;
    Some(back(entity) == face)
}

/// Offers `amount` Joules through `face`: what it didn't take.
pub(super) fn offer(entity: &mut ModBlockEntity, config: &EnergyConfig, face: Direction, amount: u64, simulate: bool) -> Option<u64> {
    let kind = kind_of(&entity.id)?;
    if back(entity) != face {
        return Some(amount);
    }
    let mut container = store(&entity.nbt, kind, config);
    let before = container.stored;
    let rest = container.insert(amount, simulate, Automation::External);
    if !simulate && container.stored != before {
        mek_world::write_energy(&mut entity.nbt, container.stored);
    }
    Some(rest)
}

/// Its store as its back reaches it: it takes, and gives nothing (`notExternal`).
struct Back {
    container: EnergyContainer,
    open: bool,
    taken: u64,
}

impl StrictEnergy for Back {
    fn insert(&mut self, joules: i64, simulate: bool) -> i64 {
        if !self.open || joules <= 0 {
            return joules;
        }
        let rest = self.container.insert(joules as u64, simulate, Automation::External);
        if !simulate {
            self.taken += joules as u64 - rest;
        }
        rest as i64
    }

    fn extract(&mut self, _joules: i64, _simulate: bool) -> i64 {
        0
    }
}

/// `native_fe_storage`: its store in whole FE through its back.
pub(super) fn fe_storage(entity: &ModBlockEntity, face: Option<Face>, config: &EnergyConfig, info: &mut Option<NativeFeInfo>) -> bool {
    let Some(kind) = kind_of(&entity.id) else {
        return false;
    };
    if config.blacklist_forge || face.is_some_and(|f| mek_world::mek_direction(f) != back(entity)) {
        return true;
    }
    let container = store(&entity.nbt, kind, config);
    let rate = Conversion(config.fe_conversion_rate);
    *info = Some(NativeFeInfo {
        stored: i64::from(rate.to_fe_int(container.stored.min(i64::MAX as u64) as i64)),
        capacity: i64::from(rate.to_fe_int(container.capacity.min(i64::MAX as u64) as i64)),
        can_receive: true,
        can_extract: true,
    });
    true
}

/// `native_fe_receive` and `native_fe_extract`.
#[allow(clippy::too_many_arguments)]
pub(super) fn fe_transfer(pos: [i32; 3], entity: &mut ModBlockEntity, face: Option<Face>, config: &EnergyConfig, max: i64, simulate: bool, extract: bool, moved: &mut Option<i64>) -> bool {
    let Some(kind) = kind_of(&entity.id) else {
        return false;
    };
    let open = face.is_some_and(|f| mek_world::mek_direction(f) == back(entity));
    if config.blacklist_forge || face.is_some() && !open {
        return true;
    }
    let container = store(&entity.nbt, kind, config);
    let before = container.stored;
    let mut side = Back { container, open, taken: 0 };
    let rate = Conversion(config.fe_conversion_rate);
    let max = max.clamp(0, i64::from(i32::MAX)) as i32;
    let fe = if extract { fe::extract_fe(&mut side, rate, max, simulate) } else { fe::receive_fe(&mut side, rate, max, simulate) };
    if !simulate && side.container.stored != before {
        mek_world::write_energy(&mut entity.nbt, side.container.stored);
    }
    if side.taken > 0 {
        mek_world::add_received(pos, side.taken);
    }
    *moved = Some(i64::from(fe));
    true
}

// ---- The tank from outside ------------------------------------------------------------------

/// The tank through `face`: on its top only (the null side reads it).
fn tank_side(entity: &ModBlockEntity, face: Option<Face>) -> Option<(FluidMachine, bool)> {
    let kind = kind_of(&entity.id)?;
    match face {
        Some(f) if pump::TANK_SIDE.direction(mek_world::generator_facing(&entity.nbt)) != mek_world::mek_direction(f) => None,
        _ => Some((kind, face.is_some())),
    }
}

/// `native_fluid_handler_tanks`: the pump's tank gives (`BasicFluidTank.output`), the plenisher's takes (`input`).
pub(super) fn fluid_tanks(entity: &ModBlockEntity, face: Option<Face>, tanks: &mut Option<Vec<NativeFluidTankInfo>>) -> bool {
    let Some((kind, _)) = tank_side(entity, face) else {
        return kind_of(&entity.id).is_some();
    };
    *tanks = Some(vec![NativeFluidTankInfo { stack: mek_world::saved_tank(&entity.nbt), capacity: pump::TANK, drainable: kind == FluidMachine::Pump }]);
    true
}

/// `native_fluid_handler_fill`: only the plenisher's, with a fluid that has a block, as the tank's validator says
/// (`canBePlacedInLevel`).
pub(super) fn fluid_fill(entity: &mut ModBlockEntity, face: Option<Face>, resource: &FluidStack, simulate: bool, filled: &mut Option<i32>) -> bool {
    let Some((kind, sided)) = tank_side(entity, face) else {
        if kind_of(&entity.id).is_some() {
            *filled = Some(0);
            return true;
        }
        return false;
    };
    let placeable = resource.fluid().mod_fluid().is_none_or(|m| m.block != "minecraft:air");
    if kind != FluidMachine::Plenisher || !sided || !placeable || resource.is_empty() {
        *filled = Some(0);
        return true;
    }
    let tank = mek_world::saved_tank(&entity.nbt);
    if !(tank.is_empty() || FluidStack::is_same_fluid_same_components(&tank, resource)) {
        *filled = Some(0);
        return true;
    }
    let took = resource.amount().min(pump::TANK - tank.amount().max(0)).max(0);
    if took > 0 && !simulate {
        let after = if tank.is_empty() { resource.copy_with_amount(took) } else { tank.copy_with_amount(tank.amount() + took) };
        mek_world::write_tank(&mut entity.nbt, &after);
    }
    *filled = Some(took);
    true
}

/// `native_fluid_handler_drain`: only the pump's, through its top.
pub(super) fn fluid_drain(entity: &mut ModBlockEntity, face: Option<Face>, resource: Option<&FluidStack>, max: i32, simulate: bool, drained: &mut Option<FluidStack>) -> bool {
    let Some((kind, sided)) = tank_side(entity, face) else {
        if kind_of(&entity.id).is_some() {
            *drained = Some(FluidStack::EMPTY);
            return true;
        }
        return false;
    };
    let tank = mek_world::saved_tank(&entity.nbt);
    let matches = resource.is_none_or(|r| FluidStack::is_same_fluid_same_components(&tank, r));
    if kind != FluidMachine::Pump || !sided || tank.is_empty() || !matches {
        *drained = Some(FluidStack::EMPTY);
        return true;
    }
    let wanted = resource.map_or(max, FluidStack::amount).max(0);
    let given = wanted.min(tank.amount());
    if given > 0 && !simulate {
        let rest = tank.amount() - given;
        mek_world::write_tank(&mut entity.nbt, &if rest > 0 { tank.copy_with_amount(rest) } else { FluidStack::EMPTY });
    }
    *drained = Some(tank.copy_with_amount(given));
    true
}

// ---- /mek probe -----------------------------------------------------------------------------

pub(super) fn probe(game: &Game<'_>, pos: [i32; 3], id: &str, nbt: &Compound, data: &[i32], items: &[Option<ItemStack>]) -> serde_json::Value {
    let Some(kind) = kind_of(id) else {
        return serde_json::json!({ "pos": pos, "id": id, "error": "not a pump" });
    };
    let (config, from_files) = mek_world::energy_config();
    let tank = read_tank(nbt);
    let upgrades = read_upgrades(nbt);
    let store = store(nbt, kind, &config);
    let slots: Vec<serde_json::Value> = items
        .iter()
        .take(pump::SLOTS)
        .map(|s| s.as_ref().map_or(serde_json::Value::Null, |s| serde_json::json!({ "item": &*s.item, "count": s.count })))
        .collect();
    let mut out = serde_json::json!({
        "pos": pos,
        "id": id,
        "energy": store.stored,
        "capacity": store.capacity,
        "per_tick": read(data, 8, 4),
        "used_energy": data.get(16).copied().unwrap_or(0) != 0,
        "tank": { "fluid": tank.fluid, "amount": tank.amount, "capacity": pump::TANK },
        "facing": mek_world::generator_facing(nbt).name(),
        "upgrades": Upgrade::ALL.iter().filter(|u| upgrades.get(**u) > 0).map(|u| serde_json::json!([u.name(), upgrades.get(*u)])).collect::<Vec<_>>(),
        "slots": slots,
        "config": { "from_files": from_files, "range": config.pump.range, "heavy_water_amount": config.pump.heavy_water_amount,
                    "max_plenisher_nodes": config.pump.max_plenisher_nodes, "usage": kind.energy(&config.pump).0 },
    });
    match kind {
        FluidMachine::Pump => {
            let state = read_pump(nbt);
            out["progress"] = state.progress.into();
            out["active_type"] = state.active.into();
            out["recurring_nodes"] = state.recurring.iter().copied().collect::<Vec<_>>().into();
        }
        FluidMachine::Plenisher => {
            let state = read_plenisher(nbt);
            out["progress"] = state.progress.into();
            out["finished"] = state.finished.into();
            out["active_nodes"] = state.active.clone().into();
            out["used_nodes"] = state.used.len().into();
        }
    }
    let _ = game;
    out
}

// ---- The screen -----------------------------------------------------------------------------

/// `GuiElectricPump` and `GuiFluidicPlenisher`: the inner screen, the down arrow, the power bar, the standard
/// fluid gauge and the energy tab.
const INNER: mek_ui::Rect = [54.0, 23.0, 80.0, 42.0];
const POWER_BAR: mek_ui::Rect = [164.0, 15.0, 6.0, 54.0];
const GAUGE: mek_ui::Rect = [6.0, 13.0, 18.0, 60.0];
const ARROW: mek_ui::Rect = [32.0, 39.0, 8.0, 10.0];
/// The upgrade holder beside the panel (Mekanism shows these slots only in its upgrade window).
const UPGRADE_HOLDER: mek_ui::Rect = [176.0, 34.0, 26.0, 48.0];

pub(super) fn screen() -> super::native_hooks::NativeScreen {
    super::native_hooks::NativeScreen {
        width: 176.0,
        height: 166.0,
        slots: vec![Some([28.0, 20.0]), Some([28.0, 51.0]), Some([143.0, 35.0]), Some([181.0, 39.0]), Some([181.0, 61.0])],
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: true,
        own_background: false,
    }
}

struct View {
    stored: u64,
    capacity: u64,
    per_tick: u64,
    fluid: u16,
    amount: u64,
    active: u16,
    used: bool,
    finished: bool,
}

fn view(data: &[i32]) -> Option<View> {
    (data.len() >= DATA).then(|| View {
        stored: read(data, 0, 4),
        capacity: read(data, 4, 4),
        per_tick: read(data, 8, 4),
        fluid: data[12] as u16,
        amount: read(data, 13, 2),
        active: data[15] as u16,
        used: data[16] != 0,
        finished: data[17] != 0,
    })
}

fn fluid_name(number: u16) -> Option<String> {
    let kind = FluidKind::from_id(u32::from(number)).filter(|k| *k != FluidKind::Empty)?;
    let id = kind.name();
    let path = id.split(':').nth(1).unwrap_or(id);
    Some(path.split('_').map(|w| {
        let mut c = w.chars();
        c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
    }).collect::<Vec<_>>().join(" "))
}

pub(super) fn draw(game: &Game<'_>, kind: FluidMachine, data: &[i32], batch: &mut horizon_client::hud::HudBatch, origin: [f32; 2]) {
    let settings = mek_ui::settings();
    let cursor = game.native_cursor();
    let cursor = [cursor[0] - origin[0], cursor[1] - origin[1]];
    let mut pen = Pen { game, batch, origin, cursor };
    let v = view(data);
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [0.0, 0.0, 176.0, 166.0]);
    let unit = if settings.show_fe { "fe" } else { "j" };
    pen.whole(&format!("tabs/energy_info_{unit}"), [26.0, 26.0], mek_ui::ENERGY_TAB);
    pen.holder(UPGRADE_HOLDER, false);
    pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], POWER_BAR);
    if let Some(v) = &v {
        let level = if v.capacity == 0 { 0.0 } else { v.stored as f64 / v.capacity as f64 };
        let h = (level * 52.0) as i32 as f32;
        if h > 0.0 {
            pen.sprite("bar/vertical_power", [0.0, 0.0, 4.0, h], [POWER_BAR[0] + 1.0, POWER_BAR[1] + POWER_BAR[3] - 1.0 - h, 4.0, h]);
        }
    }
    pen.nine("gauge/normal", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], GAUGE);
    if let Some(v) = &v
        && v.amount > 0
    {
        let inner = GAUGE[3] - 2.0;
        let h = ((v.amount as f32 / pump::TANK as f32) * inner).round().max(1.0);
        pen.fill([GAUGE[0] + 1.0, GAUGE[1] + 1.0 + inner - h, GAUGE[2] - 2.0, h], 0xFF00_0000 | mek_ui::fluid_color(v.fluid));
    }
    pen.whole("gauge/standard", [16.0, 58.0], [GAUGE[0] + 1.0, GAUGE[1] + 1.0, 16.0, 58.0]);
    pen.slot("normal", None, 27.0, 19.0);
    pen.slot("output", None, 27.0, 50.0);
    pen.slot("power", Some("overlay_power"), 142.0, 34.0);
    pen.slot("normal", None, 180.0, 38.0);
    pen.slot("output", None, 180.0, 60.0);
    pen.whole("down_arrow", [8.0, 10.0], ARROW);
    pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], INNER);
    if let Some(v) = &v {
        let mut lines = vec![display::fraction(v.stored, v.capacity, &settings)];
        if v.amount > 0 {
            lines.push(format!("{}: {} mB", fluid_name(v.fluid).unwrap_or_default(), display::thousands(v.amount)));
        } else if kind == FluidMachine::Pump && v.active != 0 {
            lines.push(fluid_name(v.active).unwrap_or_default());
        } else {
            lines.push("No fluid".to_owned());
        }
        if kind == FluidMachine::Plenisher {
            lines.push(if v.finished { "Finished".to_owned() } else { "Working".to_owned() });
        }
        let mut y = INNER[1] + 4.0;
        for line in lines {
            let short = if game.native_text_width(&line) > INNER[2] - 6.0 { line.split(':').next().unwrap_or(&line).to_owned() } else { line };
            pen.text(&short, [INNER[0] + 3.0, y], mek_ui::SCREEN_TEXT);
            y += 11.0;
        }
    }
    for row in 0..4 {
        let y = if row == 3 { 141.0 } else { 83.0 + 18.0 * row as f32 };
        for col in 0..9 {
            pen.slot("normal", None, 7.0 + 18.0 * col as f32, y);
        }
    }
    let title = game.native_screen_title().unwrap_or_default();
    pen.centered(&title, 0.0, 176.0, 5.0, mek_ui::TITLE);
    pen.text("Inventory", [8.0, 74.0], mek_ui::TITLE);
}

pub(super) fn tooltip(data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    let Some(v) = view(data) else {
        return;
    };
    let settings = mek_ui::settings();
    if mek_ui::inside(GAUGE, at) {
        lines.push(if v.amount == 0 { "Empty".into() } else { format!("{}: {} mB", fluid_name(v.fluid).unwrap_or_default(), display::thousands(v.amount)) });
    } else if mek_ui::inside(POWER_BAR, at) {
        lines.push(display::fraction(v.stored, v.capacity, &settings));
    } else if mek_ui::inside(mek_ui::ENERGY_TAB, at) {
        let used = if v.used { v.per_tick } else { 0 };
        lines.push(format!("Using: {}/t", display::energy(used, &settings)));
        lines.push(format!("Needed: {}", display::energy(v.capacity.saturating_sub(v.stored), &settings)));
    }
}
