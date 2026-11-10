// Mekanism Generators' Bio-Generator, Wind Generator and Advanced Solar Generator in the game (support module
// `crate::game::mek_gen2` of the `mekanism` native bundle). Their rules are the bundle's crate
// (`mekanism_energy::bio`, `wind`, `solar`; MIT, from Mekanism Generators 10.7.19, Copyright (c) 2017-2025 Aidan C.
// Brady); this module is the glue: their ticks (`onUpdateServer`: the ejector out of their energy faces, the
// bio-generator's fuel slot and burn, the wind generator's look at its height and sky every 20 ticks, the
// advanced solar generator's nine checks of the sky over its panel), the bio-generator's tank as a fluid handler
// on every side but the front (bioethanol from any block, none out), the wind and advanced solar generators'
// bounding blocks (`AttributeHasBounding`: refused where they don't fit, set when a player places the generator,
// taken away with it), what they keep between ticks by position (Mekanism doesn't save it), their screens' own
// parts (`GuiBioGenerator`, `GuiWindGenerator`) and `/mek probe`. The rest of a generator (its store, Forge
// Energy and cables on its energy faces, the solar screen) is `mek_world`'s and `mek_ui`'s.

use super::{
    Game,
    mek_ui::{self, Pen},
    mek_world::{self, key},
    native_hooks::NativeFluidTankInfo,
};
use horizon_client::{containers::ModBlockEntity, items::ItemStack};
use horizon_core::geometry::Direction as Face;
use horizon_world::{
    chunk::{BlockProperties, BlockState},
    fluid::{FluidKind, FluidState},
    fluid_stack::FluidStack,
    level::flags,
    nbt::{Compound, Tag},
};
use mekanism_energy::{
    bio,
    config::EnergyConfig,
    container::EnergyContainer,
    display,
    generator::GeneratorKind,
    heat::{self, Activity, Fuel, ItemFluid, SlotUse},
    menu::GeneratorView,
    sides::Direction,
    solar::{self, AdvancedSolar, SolarCheck, Sky},
    wind::{self, Heights, Wind},
};
use std::{cell::RefCell, collections::HashMap};

/// Mekanism's `bounding_block` (`MekanismBlocks.BOUNDING_BLOCK`).
pub(super) const BOUNDING: &str = "mekanism:bounding_block";

thread_local! {
    /// Each wind generator's ticker and multiplier, by dimension and position (not saved).
    static WINDS: RefCell<HashMap<(String, [i32; 3]), Wind>> = RefCell::new(HashMap::new());
    /// Each advanced solar generator's checks and its own tick count, the
    /// checks' clock (Java's game time: only the spacing of looks matters).
    static SOLARS: RefCell<HashMap<(String, [i32; 3]), (AdvancedSolar, i64)>> = RefCell::new(HashMap::new());
    /// The items of `c:fuels/bio` (200 mB) and `c:fuels/block/bio` (1800), as
    /// the content's tag files list them, for the fuel slot's hoppers.
    static BIO_FUELS: RefCell<Option<HashMap<String, i32>>> = const { RefCell::new(None) };
    /// Generators taken out of the world whose bounding blocks go at the end
    /// of the level's tick (`removeBoundingBlocks`), where the game is at hand.
    static REMOVED: RefCell<Vec<(String, [i32; 3], GeneratorKind)>> = const { RefCell::new(Vec::new()) };
}

pub(super) fn is_new(kind: GeneratorKind) -> bool {
    matches!(kind, GeneratorKind::Bio | GeneratorKind::Wind | GeneratorKind::AdvancedSolar)
}

pub(super) fn forget(dimension: &str, pos: [i32; 3]) {
    let id = (dimension.to_owned(), pos);
    WINDS.with(|w| w.borrow_mut().remove(&id));
    SOLARS.with(|s| s.borrow_mut().remove(&id));
}

pub(super) fn clear() {
    WINDS.with(|w| w.borrow_mut().clear());
    SOLARS.with(|s| s.borrow_mut().clear());
    BIO_FUELS.with(|b| *b.borrow_mut() = None);
    REMOVED.with(|r| r.borrow_mut().clear());
}

fn beside(pos: [i32; 3], by: [i32; 3]) -> [i32; 3] {
    [pos[0] + by[0], pos[1] + by[1], pos[2] + by[2]]
}

/// The fluid state at `pos` (`Level.getFluidState`): a liquid's, a
/// waterlogged block's, kelp's and the like.
pub(super) fn fluid_at(game: &Game<'_>, pos: [i32; 3]) -> FluidState {
    let mut properties = BlockProperties::new();
    for name in ["level", "waterlogged"] {
        if let Some(v) = game.native_block_property(pos, name) {
            properties.insert(name.to_owned(), v);
        }
    }
    FluidState::from_block(&BlockState { name: game.native_block(pos), properties }).unwrap_or(FluidState::EMPTY)
}

/// `canSeeSky`: full sky light at `pos`.
fn sees_sky(game: &Game<'_>, pos: [i32; 3]) -> bool {
    game.native_light(pos).1 >= 15
}

/// The facing its block shows, copied into its compound for the hooks that see only the entity.
fn facing(game: &Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) -> Direction {
    let facing = game.native_block_property(pos, "facing").and_then(|f| Direction::from_name(&f)).unwrap_or(Direction::North);
    if mek_world::generator_facing(&entity.nbt) != facing || !entity.nbt.contains_key(&key("facing")) {
        entity.nbt.insert(key("facing"), Tag::String(key(facing.name())));
    }
    facing
}

/// `native_block_entity_tick` of a bio, wind or advanced solar generator: `tickServer`'s active state delay, then
/// `onUpdateServer` (the ejector first), then the screen's data slots. Redstone control is IGNORED; the energy
/// slot charges nothing.
pub(super) fn tick(game: &mut Game<'_>, config: &EnergyConfig, pos: [i32; 3], entity: &mut ModBlockEntity, kind: GeneratorKind) {
    let facing = facing(game, pos, entity);
    let mut activity = mek_world::read_activity(&entity.nbt);
    let mut shown = game.native_block_property(pos, "active").as_deref() == Some("true");
    let changed = activity.tick(shown);
    mek_world::show_active(game, pos, changed, &mut shown);
    let mut store = mek_world::generator_store(&entity.nbt, kind, config);
    let energy_before = store.stored;
    let data = match kind {
        GeneratorKind::Bio => tick_bio(game, config, pos, entity, facing, &mut store, &mut activity, &mut shown),
        GeneratorKind::Wind => tick_wind(game, config, pos, facing, &mut store, &mut activity, &mut shown),
        _ => tick_advanced_solar(game, config, pos, facing, &mut store, &mut activity, &mut shown),
    };
    if store.stored != energy_before {
        mek_world::write_energy(&mut entity.nbt, store.stored);
    }
    if activity != mek_world::read_activity(&entity.nbt) || !entity.nbt.contains_key(&key("active_state")) {
        mek_world::write_activity(&mut entity.nbt, activity);
    }
    entity.data = data;
}

fn set_active(game: &mut Game<'_>, pos: [i32; 3], activity: &mut Activity, active: bool, shown: &mut bool, config: &EnergyConfig) {
    let changed = activity.set(active, *shown, config.deactivation_delay);
    mek_world::show_active(game, pos, changed, shown);
}

// ---- The Bio-Generator ------------------------------------------------------------------------

fn bioethanol(stack: &FluidStack) -> bool {
    stack.is_tag(bio::FLUID_TAG)
}

/// The fuel items of the content's tag files (`data/c/tags/item/fuels/bio.json`, `.../block/bio.json`), read
/// once; Mekanism's own two before any are read.
fn bio_fuels(game: Option<&Game<'_>>) -> HashMap<String, i32> {
    if let Some(known) = BIO_FUELS.with(|b| b.borrow().clone()) {
        return known;
    }
    let mut fuels = HashMap::from([("mekanism:bio_fuel".to_owned(), bio::BIO_FUEL), ("mekanism:block_bio_fuel".to_owned(), bio::BLOCK_FUEL)]);
    let Some(game) = game else {
        return fuels;
    };
    for (file, value) in [("data/c/tags/item/fuels/block/bio.json", bio::BLOCK_FUEL), ("data/c/tags/item/fuels/bio.json", bio::BIO_FUEL)] {
        let listed = game.native_data(file).and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
        let values = listed.as_ref().and_then(|v| v.get("values")).and_then(serde_json::Value::as_array).cloned().unwrap_or_default();
        for v in values {
            let id = v.as_str().or_else(|| v.get("id").and_then(serde_json::Value::as_str));
            if let Some(id) = id.filter(|id| !id.starts_with('#')) {
                fuels.insert(id.to_owned(), value);
            }
        }
    }
    BIO_FUELS.with(|b| *b.borrow_mut() = Some(fuels.clone()));
    fuels
}

/// The slot's `fuelValue` of `stack`: the world's own tags where the server has them, the tag files else.
fn fuel_value(game: &Game<'_>, stack: &ItemStack) -> i32 {
    let tagged = |tag: &str| game.native_server_item_in_tag(stack, tag).ok();
    match (tagged(bio::BIO_TAG), tagged(bio::BLOCK_TAG)) {
        (Some(b), Some(block)) => bio::fuel_value(b, block),
        _ => bio_fuels(Some(game)).get(&*stack.item).copied().unwrap_or(0),
    }
}

/// What the fuel slot holds, as `fill_or_burn` needs it.
fn bio_fuel_in(game: &Game<'_>, stack: &ItemStack, tank: &FluidStack) -> Fuel {
    let fluid = game.native_item_fluid(stack).and_then(|held| {
        let held_stack = FluidStack::of(held.fluid.as_deref()?, held.amount)?;
        Some(ItemFluid {
            amount: held.amount.max(0) as u32,
            valid: bioethanol(&held_stack),
            same: tank.is_empty() || FluidStack::is_same_fluid_same_components(tank, &held_stack),
            whole: horizon_world::fluid_stack::bucket_fluid(&stack.item).is_some(),
        })
    });
    Fuel {
        count: stack.count,
        fluid,
        burn_time: 0,
        has_remainder: game.native_crafting_remainder(&stack.item).is_some(),
        takes_lava: tank.is_empty() || tank.name() == bio::BIOETHANOL,
    }
}

#[allow(clippy::too_many_arguments)]
fn tick_bio(
    game: &mut Game<'_>,
    config: &EnergyConfig,
    pos: [i32; 3],
    entity: &mut ModBlockEntity,
    facing: Direction,
    store: &mut EnergyContainer,
    activity: &mut Activity,
    shown: &mut bool,
) -> Vec<i32> {
    let generators = &config.generators;
    mek_world::eject(game, config, pos, &GeneratorKind::Bio.energy_faces(facing), store, bio::max_output(generators));
    let tank = mek_world::saved_tank(&entity.nbt);
    let mut amount = (tank.amount().max(0) as u32).min(generators.bio_tank_capacity);
    let mut incoming: Option<String> = None;
    // `fuelSlot.fillOrBurn()`.
    if let Some(stack) = entity.items.first().and_then(Option::as_ref).filter(|s| s.count > 0).cloned() {
        let fuel = bio_fuel_in(game, &stack, &tank);
        let value = fuel_value(game, &stack);
        let before = amount;
        match bio::fill_or_burn(&mut amount, generators.bio_tank_capacity, Some(fuel), value) {
            SlotUse::Kept => {}
            SlotUse::Drained(n) => match game.native_item_fluid_drain(&stack, n as i32) {
                Some((fluid, took, after)) if took as u32 == n => {
                    incoming = Some(fluid);
                    if let Some(slot) = entity.items.first_mut() {
                        *slot = Some(after);
                    }
                }
                _ => amount = before,
            },
            SlotUse::Remainder => {
                let rest = game.native_crafting_remainder(&stack.item);
                if let Some(slot) = entity.items.first_mut() {
                    *slot = rest;
                }
            }
            SlotUse::ShrunkOne => {
                if let Some(slot) = entity.items.first_mut() {
                    if let Some(s) = slot.as_mut() {
                        s.count -= 1;
                    }
                    if slot.as_ref().is_some_and(|s| s.count == 0) {
                        *slot = None;
                    }
                }
            }
        }
    }
    let active = bio::burn(&mut amount, store, generators.bio_generation, true);
    set_active(game, pos, activity, active, shown, config);
    let after = if amount == 0 {
        FluidStack::EMPTY
    } else if tank.is_empty() {
        let fluid = incoming.as_deref().unwrap_or(bio::BIOETHANOL);
        FluidStack::of(fluid, amount as i32).unwrap_or(FluidStack::EMPTY)
    } else {
        tank.copy_with_amount(amount as i32)
    };
    if after != tank {
        mek_world::write_tank(&mut entity.nbt, &after);
    }
    GeneratorView {
        stored: store.stored,
        capacity: store.capacity,
        produced: bio::production(active, generators),
        max_output: bio::max_output(generators),
        sees_sun: active,
        lava: amount,
        tank: generators.bio_tank_capacity,
        ..GeneratorView::default()
    }
    .pack()
}

/// The bio-generator's tank through `face` (`FluidTankHelper.forSide(facing).addTank(bioFuelTank, LEFT, RIGHT,
/// BACK, TOP, BOTTOM)`); the null side reads it. None for any other block, and on the front.
fn bio_side(entity: &ModBlockEntity, face: Option<Face>) -> Option<Option<Face>> {
    if GeneratorKind::of_block(&entity.id) != Some(GeneratorKind::Bio) {
        return None;
    }
    match face {
        Some(f) if !heat::tank_on(mek_world::generator_facing(&entity.nbt), mek_world::mek_direction(f)) => None,
        _ => Some(face),
    }
}

/// `native_fluid_handler_tanks` of a bio-generator: one tank, which no other block drains (`notExternal`).
pub(super) fn fluid_tanks(entity: &ModBlockEntity, face: Option<Face>, tanks: &mut Option<Vec<NativeFluidTankInfo>>) -> bool {
    if bio_side(entity, face).is_none() {
        return false;
    }
    let (config, _) = mek_world::energy_config();
    *tanks = Some(vec![NativeFluidTankInfo {
        stack: mek_world::saved_tank(&entity.nbt),
        capacity: config.generators.bio_tank_capacity.min(i32::MAX as u32) as i32,
        drainable: false,
    }]);
    true
}

/// `native_fluid_handler_fill` of a bio-generator: bioethanol, into an empty tank or one of the same fluid.
pub(super) fn fluid_fill(entity: &mut ModBlockEntity, face: Option<Face>, resource: &FluidStack, simulate: bool, filled: &mut Option<i32>) -> bool {
    let Some(side) = bio_side(entity, face) else {
        return false;
    };
    if side.is_none() {
        *filled = Some(0);
        return true;
    }
    let (config, _) = mek_world::energy_config();
    let tank = mek_world::saved_tank(&entity.nbt);
    let stored = tank.amount().max(0) as u32;
    let same = tank.is_empty() || FluidStack::is_same_fluid_same_components(&tank, resource);
    let took = heat::tank_insert(stored, config.generators.bio_tank_capacity, bioethanol(resource), same, resource.amount().max(0) as u32);
    if took > 0 && !simulate {
        let after = if tank.is_empty() { resource.copy_with_amount(took as i32) } else { tank.copy_with_amount((stored + took) as i32) };
        mek_world::write_tank(&mut entity.nbt, &after);
    }
    *filled = Some(took as i32);
    true
}

/// `native_fluid_handler_drain` of a bio-generator: nothing leaves (`notExternal`).
pub(super) fn fluid_drain(entity: &mut ModBlockEntity, face: Option<Face>, drained: &mut Option<FluidStack>) -> bool {
    if bio_side(entity, face).is_none() {
        return false;
    }
    *drained = Some(mek_world::saved_tank(&entity.nbt).copy_with_amount(0));
    true
}

/// `native_block_entity_slot_accepts` of the fuel slot (`canInsert`: a fuel item, or a container whose fluid the
/// tank takes): hoppers put in what the tag files name and bioethanol buckets.
pub(super) fn slot_accepts(slot: usize, stack: &ItemStack, accepts: &mut bool) {
    if slot != 0 {
        return;
    }
    let bucket = horizon_world::fluid_stack::bucket_fluid(&stack.item).is_some_and(|f| horizon_world::fluid_stack::fluid_in_tag(f, bio::FLUID_TAG));
    if !bucket && !bio_fuels(None).contains_key(&*stack.item) {
        *accepts = false;
    }
}

// ---- The Wind Generator -----------------------------------------------------------------------

/// The world's heights the wind reads (the logical height is the build height here: the Nether's lower one
/// has no sky to see anyway).
fn heights(game: &Game<'_>) -> Heights {
    let (min_build, height) = game.native_world_height();
    Heights { min_build, max_build: min_build + height, logical_height: height }
}

fn tick_wind(
    game: &mut Game<'_>,
    config: &EnergyConfig,
    pos: [i32; 3],
    facing: Direction,
    store: &mut EnergyContainer,
    activity: &mut Activity,
    shown: &mut bool,
) -> Vec<i32> {
    let generators = &config.generators;
    mek_world::eject(game, config, pos, &GeneratorKind::Wind.energy_faces(facing), store, wind::max_output(generators));
    let id = (game.native_dimension().to_owned(), pos);
    let mut state = WINDS.with(|w| w.borrow_mut().remove(&id)).unwrap_or_default();
    let top = beside(pos, [0, wind::TOP, 0]);
    let open = || fluid_at(game, top).kind() == FluidKind::Empty && sees_sky(game, top);
    let heights = heights(game);
    // No dimension of ATM10 is tagged `c:no_wind`.
    let blown = state.tick(store, true, false, generators, || wind::multiplier(top[1], open(), heights, generators));
    if let Some(active) = blown.active {
        set_active(game, pos, activity, active, shown, config);
    }
    let generation = wind::generation(state.multiplier, generators);
    let mut data = GeneratorView {
        stored: store.stored,
        capacity: store.capacity,
        produced: state.production(activity.current, generators),
        max_output: wind::max_output(generators),
        sees_sun: activity.current,
        ..GeneratorView::default()
    }
    .pack();
    data.extend([(generation & 0xFFFF_FFFF) as u32 as i32, (generation >> 32) as u32 as i32]);
    WINDS.with(|w| w.borrow_mut().insert(id, state));
    data
}

/// The wind generator's `getCurrentGeneration` after the generator's slots.
fn wind_generation(data: &[i32]) -> u64 {
    let at = mekanism_energy::menu::GENERATOR_SLOTS;
    match (data.get(at), data.get(at + 1)) {
        (Some(lo), Some(hi)) => u64::from(*lo as u32) | (u64::from(*hi as u32) << 32),
        _ => 0,
    }
}

// ---- The Advanced Solar Generator -------------------------------------------------------------

/// `recheckSettings`: a check over each of the nine cells, by the biome there, and each one's
/// `Mth.nextInt(random, 10, 30)` (from the bundle's own random source).
fn advanced_checks(game: &Game<'_>, pos: [i32; 3]) -> Option<AdvancedSolar> {
    let mut checks = [SolarCheck::new(0.8, 0.0, false); 9];
    for (i, offset) in solar::ADVANCED_CHECKS.iter().enumerate() {
        let biome = game.native_biome_climate(beside(pos, *offset))?;
        let rain = biome.precipitation != super::native_hooks::NativePrecipitation::None;
        checks[i] = SolarCheck::new(biome.temperature, biome.downfall, rain);
    }
    let frequencies = [0_u32; 9].map(|_| 10 + (super::mek_machines::draw_random() * 21.0) as u32);
    Some(AdvancedSolar::new(checks, frequencies))
}

/// `AdvancedSolarCheck.recheckCanSeeSun`'s look at a cell: its sky, or over a fluid there the sky of the block
/// above it, which holds no fluid.
fn cell_sees_sun(game: &Game<'_>, cell: [i32; 3]) -> bool {
    if fluid_at(game, cell).kind() == FluidKind::Empty {
        return sees_sky(game, cell);
    }
    let above = beside(cell, [0, 1, 0]);
    sees_sky(game, above) && fluid_at(game, above).kind() == FluidKind::Empty
}

fn tick_advanced_solar(
    game: &mut Game<'_>,
    config: &EnergyConfig,
    pos: [i32; 3],
    facing: Direction,
    store: &mut EnergyContainer,
    activity: &mut Activity,
    shown: &mut bool,
) -> Vec<i32> {
    let generation = config.generators.advanced_solar_generation;
    let id = (game.native_dimension().to_owned(), pos);
    let known = SOLARS.with(|s| s.borrow_mut().remove(&id));
    // The first tick ejects at twice `advancedSolarGeneration`: the checks are made after.
    let max_output = solar::advanced_max_output(generation, known.as_ref().map(|k| &k.0));
    mek_world::eject(game, config, pos, &GeneratorKind::AdvancedSolar.energy_faces(facing), store, max_output);
    let Some((mut checks, age)) = known.or_else(|| advanced_checks(game, pos).map(|c| (c, 0))) else {
        return GeneratorView { stored: store.stored, capacity: store.capacity, max_output, ..GeneratorView::default() }.pack();
    };
    let kind = game.native_dimension_type();
    let (rain, thunder) = game.native_weather();
    let sky = Sky { day_time: game.native_day_time(), rain, thunder, has_skylight: kind.has_skylight, has_ceiling: kind.has_ceiling };
    let made = solar::advanced_generate(store, generation, &mut checks, &sky, age, |i| cell_sees_sun(game, beside(pos, solar::ADVANCED_CHECKS[i])));
    set_active(game, pos, activity, made.active, shown, config);
    let data = GeneratorView {
        stored: store.stored,
        capacity: store.capacity,
        produced: made.produced,
        max_output: solar::advanced_max_output(generation, Some(&checks)),
        sees_sun: made.sees_sun,
        ..GeneratorView::default()
    }
    .pack();
    SOLARS.with(|s| s.borrow_mut().insert(id, (checks, age + 1)));
    data
}

// ---- Bounding blocks --------------------------------------------------------------------------

/// Whether a block may go at `pos` (`isValidReplaceableBlock`): inside the world, air or a block a placement
/// replaces (`#minecraft:replaceable`).
fn replaceable(game: &Game<'_>, pos: [i32; 3]) -> bool {
    let (min, height) = game.native_world_height();
    if pos[1] < min || pos[1] >= min + height {
        return false;
    }
    let name = game.native_block(pos);
    if name == "minecraft:air" || name == "minecraft:cave_air" || name == "minecraft:void_air" {
        return true;
    }
    let state = BlockState { name, properties: BlockProperties::new() };
    game.native_server_block_in_tag(&state, "minecraft:replaceable").unwrap_or(false)
}

/// `native_use_item_on` with a wind or advanced solar generator's item: `ItemBlockTooltip.placeBlock` refuses the
/// placement (the use is claimed) when a bounding block wouldn't fit where the generator would go.
pub(super) fn use_on(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3], face: [i32; 3], claimed: &mut bool) {
    if *claimed || stack.count == 0 {
        return;
    }
    let Some(kind) = GeneratorKind::of_block(&stack.item).filter(|k| !k.bounding().is_empty()) else {
        return;
    };
    let at = if replaceable(game, pos) { pos } else { beside(pos, face) };
    if !replaceable(game, at) {
        return;
    }
    if kind.bounding().into_iter().any(|offset| !replaceable(game, beside(at, offset))) {
        *claimed = true;
    }
}

/// `native_place_block`: `placeBoundingBlocks`, a bounding block at each of the generator's positions.
pub(super) fn placed(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3]) -> bool {
    let Some(kind) = GeneratorKind::of_block(&stack.item).filter(|k| !k.bounding().is_empty()) else {
        return false;
    };
    if game.native_block(pos) != kind.id() {
        return true;
    }
    let Some(state) = game.native_default_state(BOUNDING) else {
        return true;
    };
    let dimension = game.native_dimension().to_owned();
    for offset in kind.bounding() {
        game.native_set_block(&dimension, beside(pos, offset), state, flags::UPDATE_ALL);
    }
    true
}

/// `native_block_entity_unload` of a generator taken out of the world: its bounding blocks go after the tick.
pub(super) fn removed(dimension: &str, pos: [i32; 3], kind: GeneratorKind) {
    if !kind.bounding().is_empty() {
        REMOVED.with(|r| r.borrow_mut().push((dimension.to_owned(), pos, kind)));
    }
}

/// `native_level_tick_post`: `removeBoundingBlocks` of the generators that went, unless one stands there again
/// (a block entity made anew, as `/data merge` does).
pub(super) fn level_ticked(game: &mut Game<'_>, dimension: &str) {
    let due: Vec<([i32; 3], GeneratorKind)> = REMOVED.with(|r| {
        let mut r = r.borrow_mut();
        let (mine, rest): (Vec<_>, Vec<_>) = r.drain(..).partition(|(d, _, _)| d == dimension);
        *r = rest;
        mine.into_iter().map(|(_, p, k)| (p, k)).collect()
    });
    let Some(air) = game.native_default_state("minecraft:air") else {
        return;
    };
    for (pos, kind) in due {
        if game.native_block(pos) == kind.id() {
            continue;
        }
        for offset in kind.bounding() {
            let at = beside(pos, offset);
            if game.native_block(at) == BOUNDING {
                game.native_set_block(dimension, at, air, flags::UPDATE_ALL);
            }
        }
    }
}

// ---- /mek probe -------------------------------------------------------------------------------

/// What `/mek probe` adds for a bio, wind or advanced solar generator.
pub(super) fn probe(game: &Game<'_>, pos: [i32; 3], kind: GeneratorKind, nbt: &Compound, data: &[i32], out: &mut serde_json::Value) {
    let (config, _) = mek_world::energy_config();
    let facing = mek_world::generator_facing(nbt);
    let view = GeneratorView::unpack(data).unwrap_or_default();
    out["facing"] = facing.name().into();
    out["energy_faces"] = kind.energy_faces(facing).iter().map(|d| d.name()).collect::<Vec<_>>().into();
    out["active"] = mek_world::read_activity(nbt).current.into();
    out["shown_active"] = (game.native_block_property(pos, "active").as_deref() == Some("true")).into();
    let bounding: Vec<serde_json::Value> = kind
        .bounding()
        .into_iter()
        .map(|o| serde_json::json!({ "pos": beside(pos, o), "block": game.native_block(beside(pos, o)) }))
        .collect();
    if !bounding.is_empty() {
        out["bounding"] = bounding.into();
    }
    let id = (game.native_dimension().to_owned(), pos);
    match kind {
        GeneratorKind::Bio => {
            let tank = mek_world::saved_tank(nbt);
            out["tank"] = serde_json::json!({ "fluid": tank.name(), "amount": tank.amount(), "capacity": config.generators.bio_tank_capacity });
            out["config"]["bio_generation"] = config.generators.bio_generation.into();
        }
        GeneratorKind::Wind => {
            let top = beside(pos, [0, wind::TOP, 0]);
            out["top"] = serde_json::json!({ "pos": top, "sky_light": game.native_light(top).1 });
            out["generation"] = wind_generation(data).into();
            if let Some(w) = WINDS.with(|w| w.borrow().get(&id).copied()) {
                out["multiplier"] = w.multiplier.into();
                out["ticker"] = w.ticker.into();
            }
            out["config"]["wind"] = serde_json::json!([config.generators.wind_generation_min, config.generators.wind_generation_max,
                config.generators.wind_min_y, config.generators.wind_max_y]);
        }
        _ => {
            out["sees_sun"] = view.sees_sun.into();
            if let Some((s, _)) = SOLARS.with(|s| s.borrow().get(&id).copied()) {
                out["checks"] = s.checks.iter().map(|c| serde_json::json!({ "sees": c.sees, "peak": c.check.peak, "every": c.frequency })).collect::<Vec<_>>().into();
            }
            out["config"]["advanced_solar_generation"] = config.generators.advanced_solar_generation.into();
        }
    }
}

// ---- Screens ----------------------------------------------------------------------------------

/// `GuiBioGenerator`'s fluid bar and `GuiInnerScreen`, `GuiWindGenerator`'s screen and state texture.
const BIO_BAR: mek_ui::Rect = [7.0, 15.0, 6.0, 54.0];
const BIO_SCREEN: mek_ui::Rect = [48.0, 23.0, 80.0, 40.0];
const WIND_SCREEN: mek_ui::Rect = [48.0, 21.0, 80.0, 44.0];
const WIND_STATE: mek_ui::Rect = [18.0, 35.0, 16.0, 16.0];
/// Bioethanol's colour (its fluid's tint) for the bar.
const BIOETHANOL_RGB: u32 = 0xFF_C2_D7_3D;

fn lines(pen: &mut Pen<'_, '_>, r: mek_ui::Rect, lines: &[(String, u32)]) {
    let [x, y, _, h] = r;
    let mut line_y = y + ((h - (11.0 * lines.len() as f32 - 2.0)) / 2.0).floor();
    for (line, rgb) in lines {
        pen.text(line, [x + 3.0, line_y], *rgb);
        line_y += 11.0;
    }
}

/// The bio and wind generators' own elements over a generator's screen.
pub(super) fn draw(pen: &mut Pen<'_, '_>, kind: GeneratorKind, view: &GeneratorView, settings: &EnergyConfig, data: &[i32]) {
    let energy = |j: u64| display::energy(j, settings);
    match kind {
        GeneratorKind::Bio => {
            pen.slot("extra", None, 16.0, 34.0);
            pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], BIO_BAR);
            let level = if view.tank == 0 { 0.0 } else { f64::from(view.lava) / f64::from(view.tank) };
            let h = (level * 52.0) as i32 as f32;
            if h > 0.0 {
                pen.fill([BIO_BAR[0] + 1.0, BIO_BAR[1] + BIO_BAR[3] - 1.0 - h, 4.0, h], BIOETHANOL_RGB);
            }
            pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], BIO_SCREEN);
            lines(pen, BIO_SCREEN, &[
                (energy(view.stored), mek_ui::SCREEN_TEXT),
                (format!("BioFuel: {}", display::thousands(u64::from(view.lava))), mek_ui::SCREEN_TEXT),
                (format!("Out: {}/t", energy(view.max_output)), mek_ui::SCREEN_TEXT),
            ]);
        }
        GeneratorKind::Wind => {
            pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], WIND_SCREEN);
            let mut shown = vec![
                (energy(view.stored), mek_ui::SCREEN_TEXT),
                (format!("Power: {}", energy(wind_generation(data))), mek_ui::SCREEN_TEXT),
                (format!("Out: {}/t", energy(view.max_output)), mek_ui::SCREEN_TEXT),
            ];
            if !view.sees_sun {
                shown.push(("Sky blocked".to_owned(), 0xAA0000));
            }
            lines(pen, WIND_SCREEN, &shown);
            pen.sprite_from("mekanismgenerators:gui/state_holder", [0.0, 0.0, 16.0, 16.0], WIND_STATE);
            let icon = if view.sees_sun { "wind_on" } else { "wind_off" };
            pen.sprite_from(&format!("mekanismgenerators:gui/{icon}"), [0.0, 0.0, 12.0, 12.0], [WIND_STATE[0] + 2.0, WIND_STATE[1] + 2.0, 12.0, 12.0]);
        }
        _ => {}
    }
}

/// The bio-generator's bar tooltip.
pub(super) fn tip(kind: GeneratorKind, view: &GeneratorView, at: [f32; 2]) -> Option<Vec<String>> {
    (kind == GeneratorKind::Bio && mek_ui::inside(BIO_BAR, at)).then(|| {
        vec![if view.lava == 0 { "Empty".to_owned() } else { format!("Bioethanol: {} mB", display::thousands(u64::from(view.lava))) }]
    })
}
