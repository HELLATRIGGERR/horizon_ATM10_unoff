// Mekanism's Thermal Evaporation Plant in the game (support module `crate::game::mek_evaporation`
// of the `mekanism` native bundle). The rules are `mekanism_energy::evaporation`
// (MIT, from Mekanism 10.7.19, Copyright (c) 2017-2025 Aidan C. Brady); the
// plant forms through `mek_multiblock` (`Kind::Evaporation`: thermal
// evaporation blocks, valves, structural glass and one controller), and its
// tanks, slots and heat live in the cache by its id there (`fluid_tanks`,
// `items`, `heat_capacitors`). This module is the glue: its settings from
// `general.toml` `[thermal_evaporation]`, the air of its corners' biomes, the
// Advanced Solar Generators on its top corners (`mek_gen2`), its tick
// (`EvaporationMultiblockData.tick`: the heat, the slots, the
// `mekanism:evaporating` recipe), the valves' fluid handler (hook points API
// 422: the input tank takes a recipe's fluid, the output tank gives), the
// controller's `active`, its screen (`GuiThermalEvaporationController`) and
// `/mek probe`.

use super::{
    Game,
    mek_dynamic_tank::{self as tank, Fluid},
    mek_multiblock::{self as mb, Formed, Kind},
    mek_ui::{self, Pen, Rect},
    native_hooks::{NativeFluidTankInfo, NativeScreen},
};
use horizon_client::{hud::HudBatch, items::ItemStack};
use horizon_core::geometry::Direction as Face;
use horizon_world::{
    fluid::FluidKind,
    fluid_ingredient::SizedFluidIngredient,
    fluid_stack::FluidStack,
    level::flags,
};
use mekanism_energy::{
    config, display,
    evaporation::{self, Errors, EvaporationConfig, Heat, Supply},
    multiblock::{Pos, Uuid},
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    time::{Duration, Instant},
};

pub(super) const CONTROLLER: &str = "mekanism:thermal_evaporation_controller";
pub(super) const VALVE: &str = "mekanism:thermal_evaporation_valve";

/// A `mekanism:evaporating` recipe (`FluidToFluidRecipe`).
struct Recipe {
    input: SizedFluidIngredient,
    output: FluidStack,
}

/// What a formed plant keeps beside its cache (not saved: made again when it forms).
#[derive(Clone, Copy, Debug, Default)]
struct State {
    /// `biomeAmbientTemp`.
    ambient: f64,
    /// `tempMultiplier`.
    multiplier: f64,
    /// `lastGain`.
    gain: f64,
    /// `lastEnvironmentLoss`.
    loss: f64,
    /// The recipe's operating ticks.
    operating: i32,
    errors: Errors,
    solars: u32,
}

thread_local! {
    static SETTINGS: RefCell<Option<(EvaporationConfig, config::Stamp, Instant)>> = const { RefCell::new(None) };
    static STATES: RefCell<HashMap<Uuid, State>> = RefCell::new(HashMap::new());
    /// Controllers of plants that broke with no game at hand, darkened at the next level tick.
    static DARK: RefCell<Vec<(String, Pos)>> = const { RefCell::new(Vec::new()) };
    /// The recipes, and the world's table they were read from (its address and length).
    static BOOK: RefCell<((usize, usize), Vec<Rc<Recipe>>)> = const { RefCell::new(((0, 0), Vec::new())) };
}

/// `general.toml` `[thermal_evaporation]`, read again within a second of a change.
pub(super) fn settings() -> EvaporationConfig {
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
                Some(d) => (EvaporationConfig::load(d), config::stamp(d), Instant::now()),
                None => (EvaporationConfig::default(), [None; config::FILES.len()], Instant::now()),
            });
        }
        s.as_ref().map(|(c, _, _)| *c).unwrap_or_default()
    })
}

/// The controller, whose screen shows the plant.
pub(super) fn is_controller(block: &str) -> bool {
    block == CONTROLLER
}

/// A world opens or closes.
pub(super) fn clear() {
    STATES.with(|s| s.borrow_mut().clear());
    DARK.with(|d| d.borrow_mut().clear());
}

fn refresh_book(game: &Game<'_>) {
    let table = game.native_recipes_of_type("mekanism:evaporating");
    let stamp = (table.as_ptr() as usize, table.len());
    if BOOK.with(|b| b.borrow().0 == stamp) {
        return;
    }
    let list = table
        .iter()
        .filter_map(|r| {
            let j = &*r.json;
            let input = SizedFluidIngredient::parse_flat(&j["input"]).ok()?;
            let output = FluidStack::from_json(&j["output"]).ok().filter(|o| !o.is_empty())?;
            Some(Rc::new(Recipe { input, output }))
        })
        .collect();
    BOOK.with(|b| *b.borrow_mut() = (stamp, list));
}

/// `containsRecipe`: a fluid some recipe takes, whatever the amount (`InputRecipeCache.SingleFluid`).
fn takes(fluid: &FluidStack) -> bool {
    !fluid.is_empty() && BOOK.with(|b| b.borrow().1.iter().any(|r| r.input.ingredient.test(fluid)))
}

/// `findFirstRecipe`.
fn recipe_for(fluid: &FluidStack) -> Option<Rc<Recipe>> {
    if fluid.is_empty() {
        return None;
    }
    BOOK.with(|b| b.borrow().1.iter().find(|r| r.input.ingredient.test(fluid)).cloned())
}

/// The tanks' capacities: the input's (`volume / 4 * fluidPerTank`) and the output's.
fn capacities(dimension: &str, id: Uuid) -> Option<(i32, i32)> {
    let volume = mb::with_formed(dimension, id, Formed::volume)?;
    let c = settings();
    Some((c.input_capacity(volume), c.output_tank_capacity))
}

fn set_controllers(game: &mut Game<'_>, formed: &Formed, active: bool) {
    let shown = if active { "true" } else { "false" };
    for p in &formed.locations {
        if game.native_block(*p) == CONTROLLER && game.native_block_property(*p, "active").as_deref() != Some(shown) {
            game.native_set_block_property_with(*p, "active", shown, flags::UPDATE_ALL);
        }
    }
}

/// `onCreated`: the air of its eight corners, the heat capacity of its height (the cache's heat coming in), the
/// tanks capped to its size (`shouldCap`), the solars' places, and the controller lit (`setActive(isFormed)`).
pub(super) fn created(game: &mut Game<'_>, dimension: &str, id: Uuid) {
    let Some(formed) = mb::with_formed(dimension, id, Clone::clone) else { return };
    refresh_book(game);
    let c = formed.cuboid;
    let corners: Vec<f32> = [c.min[0], c.max[0]]
        .into_iter()
        .flat_map(|x| [c.min[1], c.max[1]].into_iter().flat_map(move |y| [c.min[2], c.max[2]].into_iter().map(move |z| [x, y, z])))
        .filter_map(|p| game.native_biome_climate(p).map(|b| b.temperature))
        .collect();
    let ambient = evaporation::ambient(&corners);
    let config = settings();
    let (input_cap, output_cap) = (config.input_capacity(formed.volume()), config.output_tank_capacity);
    mb::with_cache(dimension, Kind::Evaporation, id, |cache| {
        let heat = Heat::created(cache.heat.first().copied().flatten(), c.height(), ambient, &config);
        if let Some(slot) = cache.heat.first_mut() {
            *slot = Some(heat);
        }
        for (tank, cap) in cache.fluids.iter_mut().zip([input_cap, output_cap]) {
            if tank.amount() > cap {
                *tank = tank.copy_with_amount(cap);
            }
        }
    });
    STATES.with(|s| s.borrow_mut().insert(id, State { ambient, ..State::default() }));
    set_controllers(game, &formed, true);
}

/// `remove`: the plant broke; its controller goes dark (when the game is at hand: a block entity unloading isn't).
pub(super) fn removed(game: Option<&mut Game<'_>>, dimension: &str, formed: &Formed) {
    STATES.with(|s| s.borrow_mut().remove(&formed.id));
    match game {
        Some(game) => set_controllers(game, formed, false),
        None => DARK.with(|d| d.borrow_mut().extend(formed.locations.iter().map(|p| (dimension.to_owned(), *p)))),
    }
}

/// `native_level_tick_post`: the controllers of plants that broke while a block entity unloaded go dark, unless a
/// plant formed with them since.
pub(super) fn flush(game: &mut Game<'_>, dimension: &str) {
    let mine: Vec<Pos> = DARK.with(|d| {
        let mut d = d.borrow_mut();
        let (mine, rest): (Vec<_>, Vec<_>) = d.drain(..).partition(|(dim, _)| dim == dimension);
        *d = rest;
        mine.into_iter().map(|(_, p)| p).collect()
    });
    for p in mine {
        if mb::formed_at(dimension, Kind::Evaporation, p).is_none()
            && game.native_block(p) == CONTROLLER
            && game.native_block_property(p, "active").as_deref() == Some("true")
        {
            game.native_set_block_property_with(p, "active", "false", flags::UPDATE_ALL);
        }
    }
}

/// `EvaporationMultiblockData.tick`: the heat (the air, the solars), the multiplier, the output tank into a container
/// in its slot (`drainTank`), a container in the input slot into the input tank (`fillTank`), then the recipe.
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, id: Uuid) {
    let Some(formed) = mb::with_formed(dimension, id, Clone::clone) else { return };
    let Some(before) = mb::with_cache(dimension, Kind::Evaporation, id, |c| c.clone()) else { return };
    refresh_book(game);
    let config = settings();
    let height = formed.cuboid.height();
    let (input_cap, output_cap) = (config.input_capacity(formed.volume()), config.output_tank_capacity);
    let mut state = STATES.with(|s| s.borrow().get(&id).copied()).unwrap_or(State { ambient: evaporation::AMBIENT, ..State::default() });
    let mut cache = before.clone();
    if cache.fluids.len() < 2 || cache.items.len() < 4 || cache.heat.is_empty() {
        return;
    }
    // The heat.
    let solars = evaporation::solar_spots(&formed.cuboid).iter().filter(|p| super::mek_gen2::evaporation_solar(game, **p)).count() as u32;
    let mut heat = cache.heat[0].unwrap_or_else(|| Heat::created(None, height, state.ambient, &config));
    state.loss = evaporation::heat_tick(&mut heat, state.ambient, solars, 0.0, &config);
    state.solars = solars;
    cache.heat[0] = Some(heat);
    state.multiplier = evaporation::temp_multiplier(heat.temperature(), height, &config);
    // The slots: the output tank fills a container in slot 2 (to slot 3), a container in slot 0 empties into the
    // input tank (to slot 1).
    let (mut a, mut b, mut c, mut d) = (cache.items[0].take(), cache.items[1].take(), cache.items[2].take(), cache.items[3].take());
    let (input, output) = cache.fluids.split_at_mut(1);
    {
        let mut out = Fluid { tank: &mut output[0], capacity: output_cap, chemical: false };
        let mut draining = false;
        tank::fill_container(game, &mut c, &mut d, &mut out, &mut draining);
    }
    let held = a.as_ref().and_then(|s| game.native_item_fluid(&single(s))).and_then(|h| h.fluid.as_deref().and_then(|f| FluidStack::of(f, h.amount.max(1))));
    if held.as_ref().is_some_and(takes) {
        let mut inp = Fluid { tank: &mut input[0], capacity: input_cap, chemical: false };
        let mut filling = false;
        tank::empty_container(game, &mut a, &mut b, &mut inp, &mut filling);
    }
    cache.items[0] = a;
    cache.items[1] = b;
    cache.items[2] = c;
    cache.items[3] = d;
    // The recipe.
    let recipe = recipe_for(&cache.fluids[0]);
    let supply = recipe.as_ref().map(|r| {
        let out = &cache.fluids[1];
        let same = out.is_empty() || FluidStack::is_same_fluid_same_components(out, &r.output);
        let room = if same { output_cap - out.amount().max(0) } else { 0 };
        Supply { input: cache.fluids[0].amount().max(0), per_input: r.input.amount, room, per_output: r.output.amount(), output_blocked: !same }
    });
    let (ops, active, errors) = evaporation::process(&mut state.operating, state.multiplier, supply);
    state.errors = errors;
    state.gain = evaporation::last_gain(state.multiplier, active);
    if ops > 0
        && let Some(r) = &recipe
    {
        let used = r.input.amount.saturating_mul(ops);
        let left = cache.fluids[0].amount() - used;
        cache.fluids[0] = if left > 0 { cache.fluids[0].copy_with_amount(left) } else { FluidStack::EMPTY };
        let made = r.output.amount().saturating_mul(ops);
        let out = &mut cache.fluids[1];
        *out = if out.is_empty() { r.output.copy_with_amount(made) } else { out.copy_with_amount(out.amount().saturating_add(made).min(output_cap)) };
    }
    STATES.with(|s| s.borrow_mut().insert(id, state));
    if cache != before {
        mb::with_cache(dimension, Kind::Evaporation, id, |c| *c = cache);
    }
}

fn single(stack: &ItemStack) -> ItemStack {
    let mut s = stack.clone();
    s.count = 1;
    s
}

// ---- The valves' fluid handler (hook points API 422) --------------------------------------------------

/// The plant a valve at `pos` reaches: Some(None) for a valve of no formed plant (no tanks at all).
fn valve_of(dimension: &str, pos: Pos, block: &str) -> Option<Option<Uuid>> {
    (block == VALVE).then(|| mb::formed_at(dimension, Kind::Evaporation, pos))
}

/// `native_fluid_handler_tanks` of a valve: the input and output tanks once formed (only the output drains).
pub(super) fn fluid_tanks(dimension: &str, pos: Pos, face: Option<Face>, block: &str, tanks: &mut Option<Vec<NativeFluidTankInfo>>) -> bool {
    let Some(owner) = valve_of(dimension, pos, block) else { return false };
    let held = owner.and_then(|id| Some((capacities(dimension, id)?, mb::with_cache(dimension, Kind::Evaporation, id, |c| c.fluids.clone())?)));
    *tanks = Some(match held {
        Some(((input_cap, output_cap), fluids)) if fluids.len() >= 2 => vec![
            NativeFluidTankInfo { stack: fluids[0].clone(), capacity: input_cap, drainable: false },
            NativeFluidTankInfo { stack: fluids[1].clone(), capacity: output_cap, drainable: !fluids[1].is_empty() && face.is_some() },
        ],
        _ => Vec::new(),
    });
    true
}

/// `native_fluid_handler_fill`: what fits of a fluid a recipe takes into the input tank; the null side reads only.
pub(super) fn fluid_fill(dimension: &str, pos: Pos, face: Option<Face>, block: &str, resource: &FluidStack, simulate: bool, filled: &mut Option<i32>) -> bool {
    let Some(owner) = valve_of(dimension, pos, block) else { return false };
    *filled = Some(0);
    let (Some(id), Some(_)) = (owner, face) else { return true };
    let Some((capacity, _)) = capacities(dimension, id) else { return true };
    if !takes(resource) {
        return true;
    }
    let took = mb::with_cache(dimension, Kind::Evaporation, id, |c| {
        let Some(tank) = c.fluids.get_mut(0) else { return 0 };
        let same = tank.is_empty() || FluidStack::is_same_fluid_same_components(tank, resource);
        let took = mekanism_energy::dynamic_tank::fluid_room(tank.amount().max(0), capacity, same, false, resource.amount());
        if !simulate && took > 0 {
            *tank = if tank.is_empty() { resource.copy_with_amount(took) } else { tank.copy_with_amount(tank.amount() + took) };
        }
        took
    })
    .unwrap_or(0);
    *filled = Some(took);
    true
}

/// `native_fluid_handler_drain`: what the output tank holds, of the fluid asked for; the null side gives nothing.
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
    *drained = Some(FluidStack::EMPTY);
    let (Some(id), Some(_)) = (owner, face) else { return true };
    let got = mb::with_cache(dimension, Kind::Evaporation, id, |c| {
        let Some(tank) = c.fluids.get_mut(1) else { return FluidStack::EMPTY };
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

/// The controller's slots: a container of a fluid a recipe takes in 0, an empty bucket in 2; 1 and 3 only give.
pub(super) fn slot_accepts(slot: usize, stack: &ItemStack) -> bool {
    let item = &*stack.item;
    match slot {
        0 => horizon_world::fluid_stack::bucket_fluid(item).is_some_and(|f| takes(&FluidStack::new(f, 1000))),
        2 => item == "minecraft:bucket",
        _ => false,
    }
}

// ---- The screen (`GuiThermalEvaporationController`) ----------------------------------------------------------

/// The screen's data slots, 16-bit pieces: each tank's fluid number, amount and capacity, the height, the
/// temperature, the production and the loss (as `f32` bits), and the warnings.
const DATA: usize = 18;

fn pack(fluids: [(&FluidStack, i32); 2], height: i32, temperature: f64, gain: f64, loss: f64, errors: Errors) -> Vec<i32> {
    let mut out = Vec::with_capacity(DATA);
    let split = |out: &mut Vec<i32>, v: u32| {
        out.push((v & 0xFFFF) as i32);
        out.push((v >> 16) as i32);
    };
    for (fluid, capacity) in fluids {
        out.push(if fluid.is_empty() { -1 } else { fluid.fluid().id() as i32 });
        split(&mut out, fluid.amount().max(0) as u32);
        split(&mut out, capacity.max(0) as u32);
    }
    out.push(height);
    for v in [temperature, gain, loss] {
        split(&mut out, (v as f32).to_bits());
    }
    out.push(i32::from(errors.not_enough_input) | i32::from(errors.not_enough_output_space) << 1 | i32::from(errors.input_doesnt_produce_output) << 2);
    out
}

struct View {
    fluids: [(i32, u32, u32); 2],
    height: i32,
    temperature: f64,
    gain: f64,
    loss: f64,
    errors: i32,
}

fn unpack(data: &[i32]) -> Option<View> {
    if data.len() < DATA {
        return None;
    }
    let word = |at: usize| (data[at] as u32 & 0xFFFF) | ((data[at + 1] as u32 & 0xFFFF) << 16);
    let tank = |at: usize| (data[at], word(at + 1), word(at + 3));
    let float = |at: usize| f64::from(f32::from_bits(word(at)));
    Some(View { fluids: [tank(0), tank(5)], height: data[10], temperature: float(11), gain: float(13), loss: float(15), errors: data[17] })
}

/// The data slots of a formed plant, for `mek_multiblock` to show on its controller.
pub(super) fn view(_game: &Game<'_>, dimension: &str, id: Uuid) -> Vec<i32> {
    let Some((input_cap, output_cap)) = capacities(dimension, id) else { return Vec::new() };
    let height = mb::with_formed(dimension, id, |f| f.cuboid.height()).unwrap_or(0);
    let Some((fluids, heat)) = mb::with_cache(dimension, Kind::Evaporation, id, |c| (c.fluids.clone(), c.heat.first().copied().flatten())) else {
        return Vec::new();
    };
    if fluids.len() < 2 {
        return Vec::new();
    }
    let state = STATES.with(|s| s.borrow().get(&id).copied()).unwrap_or_default();
    let temperature = heat.map_or(state.ambient, |h| h.temperature());
    pack([(&fluids[0], input_cap), (&fluids[1], output_cap)], height, temperature, state.gain, state.loss, state.errors)
}

const WIDTH: f32 = 196.0;
const INPUT_GAUGE: Rect = [6.0, 13.0, 18.0, 60.0];
const OUTPUT_GAUGE: Rect = [172.0, 13.0, 18.0, 60.0];
const SCREEN: Rect = [48.0, 19.0, 100.0, 40.0];
const RATE_BAR: Rect = [58.0, 62.0, 80.0, 10.0];
const HEAT_TAB: Rect = [-26.0, 109.0, 26.0, 26.0];
const SLOTS: [[f32; 2]; 4] = [[28.0, 20.0], [28.0, 51.0], [152.0, 20.0], [152.0, 51.0]];

/// `native_screen` of the controller.
pub(super) fn screen() -> NativeScreen {
    NativeScreen {
        width: WIDTH,
        height: 166.0,
        slots: vec![Some(SLOTS[0]), Some(SLOTS[1]), Some(SLOTS[2]), Some(SLOTS[3]), None],
        inventory_y: 84.0,
        inventory_x: 18.0,
        own_labels: true,
        own_background: false,
    }
}

fn fluid_name(number: i32) -> Option<String> {
    (number >= 0).then(|| FluidKind::from_id(number as u32)).flatten().map(|f| title_case(f.name()))
}

fn title_case(id: &str) -> String {
    let path = id.split_once(':').map_or(id, |(_, p)| p);
    path.split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The inner screen's lines.
fn lines(view: &View) -> Vec<String> {
    vec![
        "Formed".into(),
        format!("Height: {}", view.height),
        format!("Temp: {}", display::temperature(view.temperature)),
        format!("Production: {} mB/t", (view.gain * 100.0).round() / 100.0),
    ]
}

pub(super) fn tooltip(data: &[i32], at: [f32; 2], tips: &mut Vec<String>) {
    let Some(view) = unpack(data) else { return };
    for (gauge, (number, amount, _)) in [(INPUT_GAUGE, view.fluids[0]), (OUTPUT_GAUGE, view.fluids[1])] {
        if mek_ui::inside(gauge, at) {
            tips.push(match fluid_name(number) {
                None => "Empty".into(),
                Some(name) => format!("{name}: {} mB", display::thousands(u64::from(amount))),
            });
            return;
        }
    }
    if mek_ui::inside(RATE_BAR, at) {
        tips.push(display::temperature(view.temperature));
    } else if mek_ui::inside(HEAT_TAB, at) {
        tips.push(format!("Dissipated: {}/t", display::temperature(view.loss)));
    }
}

fn gauge(pen: &mut Pen<'_, '_>, r: Rect, (number, amount, capacity): (i32, u32, u32)) {
    pen.nine("gauge/normal", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], r);
    let inner = r[3] - 2.0;
    let level = if capacity == 0 { 0.0 } else { (f64::from(amount) / f64::from(capacity)).min(1.0) };
    let h = (level * f64::from(inner)) as i32 as f32;
    if h > 0.0 && number >= 0 {
        pen.fill([r[0] + 1.0, r[1] + 1.0 + inner - h, r[2] - 2.0, h], 0xFF00_0000 | mek_ui::fluid_color(number as u16));
    }
    pen.whole("gauge/standard", [16.0, 58.0], [r[0] + 1.0, r[1] + 1.0, 16.0, 58.0]);
}

pub(super) fn draw(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let view = unpack(data).unwrap_or(View { fluids: [(-1, 0, 0); 2], height: 0, temperature: 0.0, gain: 0.0, loss: 0.0, errors: 0 });
    let cursor = game.native_cursor();
    let cursor = [cursor[0] - origin[0], cursor[1] - origin[1]];
    let mut pen = Pen { game, batch, origin, cursor };
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [0.0, 0.0, WIDTH, 166.0]);
    pen.whole("tabs/heat_info_k", [26.0, 26.0], HEAT_TAB);
    gauge(&mut pen, INPUT_GAUGE, view.fluids[0]);
    gauge(&mut pen, OUTPUT_GAUGE, view.fluids[1]);
    pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], SCREEN);
    for (i, line) in lines(&view).iter().enumerate() {
        pen.text(line, [SCREEN[0] + 3.0, SCREEN[1] + 3.0 + 9.0 * i as f32], mek_ui::SCREEN_TEXT);
    }
    // `GuiHorizontalRateBar`: the temperature against 3000 K.
    pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], RATE_BAR);
    let w = ((view.temperature / evaporation::MAX_MULTIPLIER_TEMP).clamp(0.0, 1.0) * 78.0) as i32 as f32;
    if w > 0.0 {
        pen.sprite("bar/horizontal_rate", [0.0, 0.0, w, 8.0], [RATE_BAR[0] + 1.0, RATE_BAR[1] + 1.0, w, 8.0]);
    }
    for (i, [x, y]) in SLOTS.iter().enumerate() {
        pen.slot(if i % 2 == 0 { "input" } else { "output" }, None, x - 1.0, y - 1.0);
    }
    pen.whole("down_arrow", [8.0, 9.0], [32.0, 39.0, 8.0, 9.0]);
    pen.whole("down_arrow", [8.0, 9.0], [156.0, 39.0, 8.0, 9.0]);
    if view.errors != 0 {
        // The gauges' and the bar's warnings (`WarningTracker`): a red frame where it applies.
        for (bit, r) in [(1, INPUT_GAUGE), (2, OUTPUT_GAUGE), (4, RATE_BAR)] {
            if view.errors & bit != 0 {
                pen.fill([r[0], r[1], r[2], 1.0], 0xFFFF_0000);
            }
        }
    }
    for row in 0..4 {
        let y = if row == 3 { 141.0 } else { 83.0 + 18.0 * row as f32 };
        for col in 0..9 {
            pen.slot("normal", None, 17.0 + 18.0 * col as f32, y);
        }
    }
    // `withGui(..., EVAPORATION_PLANT)`: the plant's name, not the controller's.
    let title = "Thermal Evaporation Plant";
    let (left, right) = (INPUT_GAUGE[0] + INPUT_GAUGE[2], OUTPUT_GAUGE[0]);
    pen.centered(title, left, right - left, 5.0, mek_ui::TITLE);
    pen.text("Inventory", [18.0, 74.0], mek_ui::TITLE);
}

// ---- /mek probe -------------------------------------------------------------------------------------------------

pub(super) fn probe(_game: &Game<'_>, dimension: &str, id: Uuid) -> Value {
    let (input_cap, output_cap) = capacities(dimension, id).unwrap_or((0, 0));
    let state = STATES.with(|s| s.borrow().get(&id).copied()).unwrap_or_default();
    let (fluids, heat, items) = mb::with_cache(dimension, Kind::Evaporation, id, |c| {
        (
            c.fluids.iter().map(|f| json!({ "fluid": (!f.is_empty()).then(|| f.name()), "amount": f.amount().max(0) })).collect::<Vec<_>>(),
            c.heat.first().copied().flatten(),
            c.items.iter().map(|s| s.as_ref().map(|s| json!({ "item": &*s.item, "count": s.count }))).collect::<Vec<_>>(),
        )
    })
    .unwrap_or_default();
    let config = settings();
    json!({
        "input": fluids.first(), "output": fluids.get(1), "input_capacity": input_cap, "output_capacity": output_cap,
        "heat": heat.map(|h| h.stored), "heat_capacity": heat.map(|h| h.capacity), "temperature": heat.map(|h| h.temperature()),
        "ambient": state.ambient, "multiplier": state.multiplier, "production": state.gain, "environment_loss": state.loss,
        "solars": state.solars, "operating_ticks": state.operating,
        "errors": { "not_enough_input": state.errors.not_enough_input, "not_enough_output_space": state.errors.not_enough_output_space,
            "input_doesnt_produce_output": state.errors.input_doesnt_produce_output },
        "slots": items,
        "config": { "heat_capacity": config.heat_capacity, "temp_multiplier": config.temp_multiplier, "fluid_per_tank": config.fluid_per_tank,
            "output_tank_capacity": config.output_tank_capacity, "solar_multiplier": config.solar_multiplier, "heat_dissipation": config.heat_dissipation },
    })
}
