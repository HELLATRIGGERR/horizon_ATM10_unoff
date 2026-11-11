//! Advanced AE's Reaction Chamber in the game: its recipe type
//! (`advanced_ae:reaction`), the grid tick that runs it with power from its
//! own buffer or the grid, its two tanks, auto-export by side, its menu's own
//! entries, buckets on the block and its fluid handler. Derived from Advanced
//! AE (`ReactionChamberEntity`, `ReactionChamberBlock`, `ReactionChamberMenu`,
//! `ReactionChamberRecipe`, `ReactionChamberRecipeSerializer`,
//! `ReactionChamberRecipes`), https://github.com/pedroksl/AdvancedAE commit
//! `c322dad81e6f2ad94cb1fb861e73a7ba68e5907a`, LGPL-3.0; see
//! `ae2/addons/advancedae/`.
//!
//! The machine is a [`Machine`] of ten slots (nine inputs, then the output)
//! and four cards. Its `fluidInv` is two tanks in its config: slot 1, the
//! input (`tank_in`: in only), and slot 0, the output (`tank_out`: out only).
//! `allowedOutputs` is a flag per `RelativeSide` (`output_front`, ...),
//! `AUTO_EXPORT` is AE2's `auto_export`. The search's `cachedTask` and
//! `dirty` live in [`Ctx`] by block entity, not in the save, as the mod
//! keeps them; nor are `processingTime`, `working` and `showWarning` saved.
//!
//! A recipe's `IngredientStack`s are read as the recipes write them
//! (`ingredient`, `amount`: 1 when absent) and taken as `sample()` and
//! `consume` take them: each ingredient from the slots in order, then from the
//! tank.
use crate::game::{Game, ae2_buses::FormationPlan, ae2_global::GridAddress, ae2_processing::json_ingredient, ae2_tick_tracker::Modulation};
use ae2_cell_core::ItemKey;
use horizon_client::{
    ae2_world::{Machine, MachineKind},
    ae2x::advancedae as rows,
    containers::BlockEntity,
    crafting::Recipes,
    items::{ItemStack, Items},
};
use horizon_core::geometry::Direction;
use horizon_world::{
    fluid::FluidKind,
    fluid_ingredient::FluidIngredient,
    fluid_stack::{FluidStack, positive_int},
    nbt::Tag,
};
use serde_json::Value;

/// `MAX_INPUT_SLOTS`; the output slot is the next.
pub(in crate::game) const INPUTS: usize = 9;
const OUTPUT: usize = INPUTS;
/// `MAX_PROCESSING_STEPS`.
pub(in crate::game) const MAX_STEPS: i32 = 200;
/// `MAX_TANK_CAPACITY`, mB, each tank.
pub(in crate::game) const TANK_CAP: u64 = 16_000;
/// `fluidInv` slot 1, the input, and slot 0, the output.
pub(in crate::game) const TANK_IN: &str = "tank_in";
pub(in crate::game) const TANK_OUT: &str = "tank_out";
/// `Settings.AUTO_EXPORT`, as `IConfigManager.writeToNBT` names it.
pub(in crate::game) const AUTO_EXPORT: &str = "auto_export";
/// `RelativeSide` in its order (an `EnumSet`'s), its flag in the config and
/// its side when the chamber faces north (`getUnrotatedSide`).
pub(in crate::game) const SIDES: [(&str, &str, Direction); 6] = [
    ("FRONT", "output_front", Direction::North),
    ("BACK", "output_back", Direction::South),
    ("TOP", "output_top", Direction::Up),
    ("BOTTOM", "output_bottom", Direction::Down),
    ("LEFT", "output_left", Direction::West),
    ("RIGHT", "output_right", Direction::East),
];

/// The menu's own entries after the slots (0..10) and cards (10..14): the
/// tanks, the flush buttons, the six sides, AUTO_EXPORT.
pub(in crate::game) const TANK_IN_ENTRY: usize = 14;
pub(in crate::game) const TANK_OUT_ENTRY: usize = 15;
pub(in crate::game) const FLUSH_IN_ENTRY: usize = 16;
pub(in crate::game) const FLUSH_OUT_ENTRY: usize = 17;
pub(in crate::game) const SIDE_ENTRIES: std::ops::Range<usize> = 18..24;
pub(in crate::game) const AUTO_EXPORT_ENTRY: usize = 24;
/// The data slot of `showWarning`, after the progress bar's four and AUTO_EXPORT's.
const WARNING_DATA: usize = 5;

fn chamber_kind(kind: MachineKind) -> bool {
    kind.ext().is_some_and(|row| row.id == rows::REACTION_CHAMBER)
}
pub(in crate::game) fn is_chamber(m: &Machine) -> bool {
    chamber_kind(m.kind)
}

/// `invExposed`: the inputs insert-only, the output extract-only.
pub(in crate::game) fn slot_insert(kind: MachineKind, index: usize) -> Option<bool> {
    chamber_kind(kind).then_some(index < INPUTS)
}
pub(in crate::game) fn slot_extract(kind: MachineKind, index: usize) -> Option<bool> {
    chamber_kind(kind).then_some(index == OUTPUT)
}
/// `setPowerSides(getGridConnectableSides(..))`: every side.
pub(in crate::game) fn private_power_face(kind: MachineKind) -> bool {
    chamber_kind(kind)
}
/// The node: no channel, no idle draw (`setIdlePowerUsage(0)`), every side.
pub(in crate::game) fn device(m: &Machine, device: &mut ae2_network_core::Device) -> bool {
    if !is_chamber(m) {
        return false;
    }
    device.idle_override = Some(0.0);
    device.channel_override = Some(false);
    device.faces = 63;
    true
}
/// `setWorking`: the block state's `working`.
pub(in crate::game) fn visual_props(m: &Machine, props: &mut horizon_world::block_state_properties::BlockProperties) -> bool {
    if !is_chamber(m) || !props.contains_key("working") {
        return false;
    }
    props.insert("working".into(), working(m).to_string());
    true
}
/// `IProgressProvider`: `processingTime` of `MAX_PROCESSING_STEPS`.
pub(in crate::game) fn progress(m: &Machine) -> Option<(f64, f64)> {
    is_chamber(m).then(|| (m.progress.clamp(0.0, f64::from(MAX_STEPS)), f64::from(MAX_STEPS)))
}
/// `@GuiSync(8) showWarning`, read by the screen's power alert.
pub(in crate::game) fn screen_data(m: &Machine, data: &mut Vec<i16>) {
    if !is_chamber(m) {
        return;
    }
    if data.len() <= WARNING_DATA {
        data.resize(WARNING_DATA + 1, 0);
    }
    data[WARNING_DATA] = i16::from(flag(m, rows::WARNING));
}
/// The tanks in the window: the fluid's bucket named with the fluid and
/// its amount (`FluidTankStackUpdatePacket`), none when empty.
pub(in crate::game) fn menu_items(m: &Machine, items: &mut [Option<ItemStack>]) {
    if !is_chamber(m) {
        return;
    }
    for (entry, key) in [(TANK_IN_ENTRY, TANK_IN), (TANK_OUT_ENTRY, TANK_OUT)] {
        if let Some(slot) = items.get_mut(entry) {
            *slot = tank(m, key).map(|(fluid, amount)| {
                let bucket = horizon_client::ae2_cell_codec::fluid_bucket(&fluid).unwrap_or("minecraft:bucket");
                let mut icon = ItemStack::new(bucket, 1);
                icon.data_mut().custom_name = Some(horizon_client::item_data::text_name(format!("{fluid}: {amount} / {TANK_CAP} mB")));
                icon
            });
        }
    }
}

fn flag(m: &Machine, key: &str) -> bool {
    matches!(m.config.get(&key.into()), Some(Tag::Byte(1)))
}
fn set_flag(m: &mut Machine, key: &str, on: bool) {
    m.config.insert(key.into(), Tag::Byte(i8::from(on)));
}
pub(in crate::game) fn working(m: &Machine) -> bool {
    flag(m, rows::WORKING)
}
/// A tank's fluid and mB.
pub(in crate::game) fn tank(m: &Machine, key: &str) -> Option<(String, u64)> {
    let Some(Tag::Compound(data)) = m.config.get(&key.into()) else { return None };
    let id = match data.get(&"id".into()) {
        Some(Tag::String(id)) => id.to_string().ok()?,
        _ => return None,
    };
    let amount = match data.get(&"amount".into()) {
        Some(Tag::Long(n)) if *n > 0 => *n as u64,
        _ => return None,
    };
    Some((id, amount))
}
pub(in crate::game) fn set_tank(m: &mut Machine, key: &str, fluid: Option<(&str, u64)>) {
    match fluid.filter(|(_, n)| *n > 0) {
        Some((id, n)) => {
            m.config.insert(key.into(), Tag::compound([("id", Tag::String(id.into())), ("amount", Tag::Long(n as i64))]));
        }
        None => {
            m.config.remove(&key.into());
        }
    }
}
/// `Settings.AUTO_EXPORT` is YES.
pub(in crate::game) fn auto_export(m: &Machine) -> bool {
    matches!(m.config.get(&AUTO_EXPORT.into()), Some(Tag::String(s)) if s.to_string().ok().as_deref() == Some("YES"))
}
/// `hasAutoExportWork`: something in the output slot or tank, and AUTO_EXPORT on.
pub(in crate::game) fn auto_export_work(m: &Machine) -> bool {
    auto_export(m) && (m.slots.get(OUTPUT).is_some_and(Option::is_some) || tank(m, TANK_OUT).is_some())
}
/// `allowedOutputs` turned to the world (`BlockOrientation.getSide`) by the
/// chamber's horizontal `facing`, in `RelativeSide` order.
pub(in crate::game) fn output_directions(m: &Machine, facing: Direction) -> Vec<Direction> {
    SIDES.iter().filter(|(_, key, _)| flag(m, key)).map(|(_, _, side)| turned(*side, facing)).collect()
}
/// `side` of a chamber facing north, for one facing `facing` (a turn about the vertical).
fn turned(side: Direction, facing: Direction) -> Direction {
    const ROUND: [Direction; 4] = [Direction::North, Direction::East, Direction::South, Direction::West];
    let at = |d: Direction| ROUND.iter().position(|r| *r == d);
    match (at(side), at(facing)) {
        (Some(s), Some(f)) => ROUND[(s + f) % 4],
        _ => side,
    }
}

/// The progress a step adds by the speed cards installed (`speedFactor`).
pub(in crate::game) fn speed(cards: usize) -> i32 {
    match cards {
        0 => 2,
        1 => 3,
        2 => 5,
        3 => 10,
        _ => 50,
    }
}

/// One `IngredientStack.Item`: an ingredient and its amount.
#[derive(Clone, Debug)]
struct Stack<'a> {
    ingredient: &'a Value,
    amount: u32,
}
impl<'a> Stack<'a> {
    fn parse(v: &'a Value) -> Option<Self> {
        let amount = match v.get("amount") {
            None => 1,
            Some(n) => u32::try_from(n.as_u64()?).ok().filter(|n| *n > 0)?,
        };
        Some(Self { ingredient: v.get("ingredient")?, amount })
    }
    /// `isEmpty`: an empty ingredient (no item matches it).
    fn empty(&self) -> bool {
        self.ingredient.as_array().is_some_and(Vec::is_empty)
    }
}
/// The `IngredientStack.Fluid`: NeoForge's `FluidIngredient` and its amount.
#[derive(Clone, Debug)]
struct FluidNeed {
    ingredient: FluidIngredient,
    amount: u32,
}
impl FluidNeed {
    fn parse(v: &Value) -> Option<Self> {
        let amount = match v.get("amount") {
            None => 1,
            Some(n) => u32::try_from(positive_int(n).ok()?).ok()?,
        };
        Some(Self { ingredient: FluidIngredient::parse(v.get("ingredient")?).ok()?, amount })
    }
}

/// `ReactionChamberRecipe.output`, a `GenericStack`: an item or a fluid.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::game) enum Output {
    Item(ItemStack),
    Fluid(String, u64),
}
/// A reaction recipe, as `ReactionChamberRecipeSerializer.CODEC` reads it.
#[derive(Clone, Debug)]
pub(in crate::game) struct Recipe<'a> {
    items: Vec<Stack<'a>>,
    fluid: FluidNeed,
    pub(in crate::game) output: Output,
    /// `input_energy`: the AE the whole craft takes.
    pub(in crate::game) energy: i32,
}
impl<'a> Recipe<'a> {
    /// `output` (`GenericStack.CODEC`), `input_items`, `input_fluid` and
    /// `input_energy`, all required.
    pub(in crate::game) fn parse(v: &'a Value) -> Option<Self> {
        Some(Self {
            output: output(v.get("output")?)?,
            items: v.get("input_items")?.as_array()?.iter().map(Stack::parse).collect::<Option<Vec<_>>>()?,
            fluid: FluidNeed::parse(v.get("input_fluid")?)?,
            energy: i32::try_from(v.get("input_energy")?.as_i64()?).ok()?,
        })
    }
    /// `getValidInputs`' items: the ingredients that are not empty.
    fn item_inputs(&self) -> impl Iterator<Item = &Stack<'a>> {
        self.items.iter().filter(|s| !s.empty())
    }
    /// The output is in the game's registries (the mod's codec refuses one that is not).
    fn known(&self, items: &Items) -> bool {
        match &self.output {
            Output::Item(stack) => items.get(&stack.item).is_some(),
            Output::Fluid(id, _) => FluidKind::from_name(id).is_some(),
        }
    }
}
/// `GenericStack.CODEC`: `#t` (`ae2:i` or `ae2:f`), the key's `id` (and an
/// item's `components`), `#` the amount.
fn output(v: &Value) -> Option<Output> {
    let amount = v.get("#")?.as_u64().filter(|n| *n > 0)?;
    let id = v.get("id")?.as_str()?;
    match v.get("#t")?.as_str()? {
        "ae2:i" => {
            let mut stack = serde_json::json!({"id": id, "count": u32::try_from(amount).ok()?});
            if let Some(components) = v.get("components") {
                stack["components"] = components.clone();
            }
            Some(Output::Item(horizon_client::crafting::Template::parse(&stack)?.create()))
        }
        "ae2:f" => Some(Output::Fluid(id.to_owned(), amount)),
        _ => None,
    }
}
/// `FluidIngredient.test` on the tank's fluid as a `FluidStack`.
fn fluid_matches(ingredient: &FluidIngredient, fluid: &str, amount: u64) -> bool {
    FluidKind::from_name(fluid).is_some_and(|kind| ingredient.test(&FluidStack::new(kind, i32::try_from(amount).unwrap_or(i32::MAX))))
}

/// `ReactionChamberRecipes.findRecipe`'s test: the inputs, one stack per
/// item and components with their counts summed, hold each item ingredient
/// in one stack of at least its amount, and the input tank its fluid.
pub(in crate::game) fn matches(m: &Machine, recipe: &Recipe, items: &Items) -> bool {
    let mut merged: Vec<ItemStack> = Vec::new();
    for stack in m.slots[..INPUTS].iter().flatten().filter(|s| s.count > 0) {
        match merged.iter_mut().find(|s| s.same_item_same_components(stack)) {
            Some(s) => s.count = s.count.saturating_add(stack.count),
            None => merged.push(stack.clone()),
        }
    }
    let items_found = recipe
        .item_inputs()
        .all(|need| merged.iter().any(|s| json_ingredient(items, Some(need.ingredient), Some(s)) && need.amount <= s.count));
    items_found && tank(m, TANK_IN).is_some_and(|(id, n)| fluid_matches(&recipe.fluid.ingredient, &id, n) && u64::from(recipe.fluid.amount) <= n)
}
/// `hasCraftWork`'s room test: the whole output fits the output slot
/// (`insertItem(.., true)`) or tank (`canAdd`).
pub(in crate::game) fn fits(m: &Machine, recipe: &Recipe, items: &Items) -> bool {
    match &recipe.output {
        Output::Item(out) => {
            let max = items.stack_max(out).min(64);
            match m.slots[OUTPUT].as_ref() {
                None => out.count <= max,
                Some(s) => s.same_item_same_components(out) && s.count.checked_add(out.count).is_some_and(|n| n <= max),
            }
        }
        Output::Fluid(id, amount) => match tank(m, TANK_OUT) {
            None => true,
            Some((held, n)) => held == *id && n + amount <= TANK_CAP,
        },
    }
}
/// The craft's end: the output in, then each input `consume`d, item
/// ingredients from the slots in order, the fluid from the input tank.
pub(in crate::game) fn finish(m: &mut Machine, recipe: &Recipe, items: &Items) {
    match &recipe.output {
        Output::Item(out) => match &mut m.slots[OUTPUT] {
            Some(s) => s.count += out.count,
            slot @ None => *slot = Some(out.clone()),
        },
        Output::Fluid(id, amount) => {
            let held = tank(m, TANK_OUT).map_or(0, |(_, n)| n);
            set_tank(m, TANK_OUT, Some((id, held + amount)));
        }
    }
    for need in recipe.item_inputs() {
        let mut left = need.amount;
        for slot in &mut m.slots[..INPUTS] {
            if let Some(s) = slot.as_mut().filter(|s| s.count > 0 && json_ingredient(items, Some(need.ingredient), Some(s))) {
                let taken = s.count.min(left);
                s.count -= taken;
                left -= taken;
                if s.count == 0 {
                    *slot = None;
                }
            }
            if left == 0 {
                break;
            }
        }
    }
    if let Some((id, n)) = tank(m, TANK_IN) {
        let taken = if fluid_matches(&recipe.fluid.ingredient, &id, n) { n.min(u64::from(recipe.fluid.amount)) } else { 0 };
        set_tank(m, TANK_IN, Some((&id, n - taken)));
    }
}
/// The recipes of the type in the game, in their load order, that its codec
/// reads.
fn recipes<'r>(recipes: &'r Recipes, items: &'r Items) -> impl Iterator<Item = (&'r str, Recipe<'r>)> {
    recipes.ae2_custom.get(rows::REACTION).into_iter().flatten().filter_map(move |(id, v)| {
        let recipe = Recipe::parse(v)?;
        recipe.known(items).then_some((id.as_str(), recipe))
    })
}
fn recipe<'r>(all: &'r Recipes, items: &'r Items, id: &str) -> Option<Recipe<'r>> {
    recipes(all, items).find(|(name, _)| *name == id).map(|(_, r)| r)
}
/// `ReactionChamberRecipes.isValidIngredient`: some recipe's item input takes `stack`.
pub(in crate::game) fn valid_ingredient(all: &Recipes, items: &Items, stack: &ItemStack) -> bool {
    recipes(all, items).any(|(_, r)| r.item_inputs().any(|need| json_ingredient(items, Some(need.ingredient), Some(stack))))
}

/// What the tick needs of the world: power, and the recipes.
pub(in crate::game) trait Host {
    /// AE the chamber's own buffer gives (or would), as `extractAEPower(.., CONFIG)` answers.
    fn own_power(&mut self, m: &mut Machine, amount: f64, simulate: bool) -> f64;
    /// The same from the grid's energy service.
    fn grid_power(&mut self, amount: f64, simulate: bool) -> f64;
    /// `IEnergyService.getIdlePowerUsage`.
    fn grid_idle(&mut self) -> f64;
    /// `findRecipe`: the first recipe the chamber meets.
    fn find(&self, m: &Machine) -> Option<String>;
    /// `input_energy` of recipe `id`, and whether its output fits.
    fn recipe(&self, m: &Machine, id: &str) -> Option<(i32, bool)>;
    /// The output put in and the inputs taken.
    fn finish(&self, m: &mut Machine, id: &str);
}

/// A chamber's search: `cachedTask`, and what the game last saw of it.
#[derive(Clone, Debug, Default)]
pub(in crate::game) struct Ctx {
    pub(in crate::game) task: Option<String>,
    /// The slots and tanks at the last tick: a change is `dirty`.
    seen: String,
    /// What wakes the tick tracker, and whether it sleeps then.
    wake: String,
    sleeping: bool,
}

/// `getTask`: the cached recipe, else one found (and kept).
fn task(m: &Machine, ctx: &mut Ctx, host: &impl Host) -> Option<String> {
    if ctx.task.is_none() {
        ctx.task = host.find(m);
    }
    ctx.task.clone()
}
/// `hasCraftWork`: a task whose output fits; without one the progress goes.
pub(in crate::game) fn craft_work(m: &mut Machine, ctx: &mut Ctx, host: &impl Host) -> bool {
    match task(m, ctx, host) {
        Some(id) => host.recipe(m, &id).is_some_and(|(_, fits)| fits),
        None => {
            m.progress = 0.0;
            working(m)
        }
    }
}

/// The craft branch's power and progress: `powerConsumption` of the
/// recipe's energy for each of the `ceil(200 / speed)` steps, from the own
/// buffer when it has it all, else from whichever of it and the grid gives
/// more; with too little, a part step and the power alert.
pub(in crate::game) fn power_step(m: &mut Machine, energy: i32, speed: i32, host: &mut impl Host) {
    let progress = m.progress as i32;
    let needed = MAX_STEPS - progress;
    let ratio = if needed < speed { needed as f32 / speed as f32 } else { 1.0 };
    let ticks = (MAX_STEPS as f32 / speed as f32).ceil() as i32;
    let consumption = ((energy as f32 / ticks as f32) * ratio).floor() as i32;
    let cost = f64::from(consumption);
    let threshold = cost - 0.01;
    let mut own = true;
    let mut offered = host.own_power(m, cost, true);
    if offered <= threshold {
        own = false;
        let from_own = offered;
        offered = host.grid_power(cost, true);
        if from_own > offered {
            own = true;
            offered = from_own;
        }
    }
    if offered > threshold {
        if own {
            host.own_power(m, cost, false);
        } else {
            host.grid_power(cost, false);
        }
        m.progress = f64::from(progress + speed);
        set_flag(m, rows::WARNING, false);
    } else if offered != 0.0 {
        let share = if own { offered / cost } else { (offered - 10.0 * host.grid_idle()) / cost };
        let factor = (share * f64::from(speed)).floor() as i32;
        if factor > 1 {
            let amount = f64::from(consumption * factor) / f64::from(speed);
            let extracted = if own { host.own_power(m, amount, false) } else { host.grid_power(amount, false) };
            let steps = (extracted / cost * f64::from(speed)).floor() as i32;
            m.progress = f64::from(progress + steps);
        }
        set_flag(m, rows::WARNING, true);
    }
}

/// `tickingRequest` up to `pushOutResult`: a change since the last tick
/// (`dirty`) drops a task the inputs no longer meet; with craft work the
/// step takes its power, and at `MAX_PROCESSING_STEPS` the output goes in
/// and the inputs out.
pub(in crate::game) fn execute(m: &mut Machine, ctx: &mut Ctx, dirty: bool, host: &mut impl Host) {
    if dirty && host.find(m).is_none() {
        m.progress = 0.0;
        set_flag(m, rows::WORKING, false);
        ctx.task = None;
    }
    if !craft_work(m, ctx, host) {
        set_flag(m, rows::WARNING, false);
        return;
    }
    set_flag(m, rows::WORKING, true);
    let step = speed(m.installed_speed_cards());
    if let Some((energy, _)) = ctx.task.clone().and_then(|id| host.recipe(m, &id)) {
        power_step(m, energy, step, host);
    }
    if m.progress as i32 >= MAX_STEPS {
        m.progress = 0.0;
        if let Some(id) = task(m, ctx, host) {
            if host.recipe(m, &id).is_some_and(|(_, fits)| fits) {
                host.finish(m, &id);
            }
        }
        ctx.task = None;
        set_flag(m, rows::WORKING, false);
    }
}

/// The token of what `onChangeInventory` and `onChangeTank` see.
fn inventory_token(m: &Machine) -> String {
    format!("{:?}|{:?}|{:?}", m.slots, m.config.get(&TANK_IN.into()), m.config.get(&TANK_OUT.into()))
}

struct GameHost<'a, 'g> {
    game: &'a mut Game<'g>,
    pos: (i32, i32, i32),
    online: Option<bool>,
}
impl Host for GameHost<'_, '_> {
    fn own_power(&mut self, m: &mut Machine, amount: f64, simulate: bool) -> f64 {
        let usage = horizon_client::ae2_common_config::gameplay().usage_multiplier;
        m.private_energy_extract(amount * usage, !simulate) / usage
    }
    fn grid_power(&mut self, amount: f64, simulate: bool) -> f64 {
        if !*self.online.get_or_insert_with(|| self.game.ae2_online(self.pos)) {
            return 0.0;
        }
        self.game.ae2_take_power(self.pos, amount, simulate)
    }
    fn grid_idle(&mut self) -> f64 {
        let Some(address) = self.game.ae2_local_address(self.pos) else { return 0.0 };
        let analysis = self.game.ae2_network().analyze().ok();
        let node = analysis.as_ref().and_then(|a| a.nodes.get(&address.network_pos()).map(|n| (a, n)));
        node.and_then(|(a, n)| a.components.get(n.component)).map_or(0.0, |c| c.idle_per_tick)
    }
    fn find(&self, m: &Machine) -> Option<String> {
        let items = self.game.res.items();
        recipes(self.game.res.recipes(), items).find(|(_, r)| matches(m, r, items)).map(|(id, _)| id.to_owned())
    }
    fn recipe(&self, m: &Machine, id: &str) -> Option<(i32, bool)> {
        let items = self.game.res.items();
        recipe(self.game.res.recipes(), items, id).map(|r| (r.energy, fits(m, &r, items)))
    }
    fn finish(&self, m: &mut Machine, id: &str) {
        let items = self.game.res.items();
        if let Some(r) = recipe(self.game.res.recipes(), items, id) {
            finish(m, &r, items);
        }
    }
}

impl Game<'_> {
    /// The chamber's horizontal `facing`.
    fn advancedae_chamber_facing(&self, pos: (i32, i32, i32)) -> Direction {
        let state = self.pack.assets.registry.state(self.world.block(pos.0, pos.1, pos.2));
        match state.and_then(|s| s.properties.get("facing")).map(String::as_str) {
            Some("south") => Direction::South,
            Some("west") => Direction::West,
            Some("east") => Direction::East,
            _ => Direction::North,
        }
    }

    /// The Reaction Chambers' grid ticks (`TickingRequest(1, 20, ..)`), with
    /// AE2's tick tracker.
    pub(in crate::game) fn tick_advancedae_chambers(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let mut found: Vec<_> = self
            .world
            .block_entities
            .iter()
            .filter_map(|(p, e)| match e {
                BlockEntity::Ae2(m) if is_chamber(m) => Some(*p),
                _ => None,
            })
            .collect();
        found.sort_unstable();
        let live: std::collections::BTreeSet<u64> = found.iter().filter_map(|p| self.world.block_entities.incarnation(p)).collect();
        self.ae2x.advancedae.chambers.retain(|id, _| live.contains(id));
        for pos in found {
            let Some(incarnation) = self.world.block_entities.incarnation(&pos) else { continue };
            let Some(BlockEntity::Ae2(original)) = self.world.block_entities.get(&pos) else { continue };
            let mut m = (**original).clone();
            let before = m.clone();
            let mut ctx = self.ae2x.advancedae.chambers.get(&incarnation).cloned().unwrap_or_default();
            let token = inventory_token(&m);
            let sides: Vec<_> = SIDES.iter().map(|(_, key, _)| flag(&m, key)).collect();
            let wake = format!("{token}|{:?}|{}|{sides:?}", m.upgrades, auto_export(&m));
            if ctx.wake != wake {
                let host = GameHost { game: self, pos, online: None };
                let work = ctx.task.clone().or_else(|| host.find(&m)).and_then(|id| host.recipe(&m, &id)).is_some_and(|(_, fits)| fits);
                ctx.sleeping = !work && !auto_export_work(&m);
                ctx.wake = wake.clone();
            }
            if !self.ae2_variable_processor_due(pos, 1, 20, ctx.sleeping, wake.clone()) {
                self.ae2x.advancedae.chambers.insert(incarnation, ctx);
                continue;
            }
            let dirty = ctx.seen != token;
            ctx.seen = token;
            execute(&mut m, &mut ctx, dirty, &mut GameHost { game: self, pos, online: None });
            let mut plan = FormationPlan::default();
            let modulation = if self.advancedae_push_out(pos, &mut m, &before, &mut plan) {
                Modulation::Urgent
            } else if craft_work(&mut m, &mut ctx, &GameHost { game: self, pos, online: None }) {
                Modulation::Urgent
            } else if auto_export_work(&m) {
                Modulation::Slower
            } else {
                Modulation::Sleep
            };
            self.ae2_variable_processor_completed(pos, modulation, wake);
            self.ae2x.advancedae.chambers.insert(incarnation, ctx);
            if m != before {
                if let Some(BlockEntity::Ae2(target)) = self.world.block_entities.get_mut(&pos) {
                    **target = m;
                    self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
                }
                if let Err(error) = self.ae2_commit_formation(plan) {
                    log::warn!("Reaction Chamber notification: {error}");
                }
            }
        }
    }

    /// `pushOutResult`: with AUTO_EXPORT on, for each allowed side in turn,
    /// up to 64 of the output item and the whole output tank go into what is
    /// there; the first side that took any ends it.
    fn advancedae_push_out(&mut self, pos: (i32, i32, i32), m: &mut Machine, original: &Machine, plan: &mut FormationPlan) -> bool {
        if !auto_export_work(m) {
            return false;
        }
        let facing = self.advancedae_chamber_facing(pos);
        for face in output_directions(m, facing) {
            let d = face.normal();
            let neighbour = (pos.0 + d[0], pos.1 + d[1], pos.2 + d[2]);
            if self.world.block_entities.get(&neighbour).is_none() {
                continue;
            }
            let Some(target) = self.ae2_local_address(neighbour) else { continue };
            let side = face.opposite();
            let mut moved = false;
            if let Some(stack) = m.slots[OUTPUT].clone() {
                let offer = stack.count.min(64);
                let mut offered = stack.clone();
                offered.count = offer;
                horizon_client::ae2_stack_identity::detach_if_tracked(&mut offered);
                let accepted = match self.ae2_item_p2p_insert(target, side, &offered, true) {
                    Some(n) => n.min(offer),
                    None => self.ae2_native_item_insert(target, side, &offered, offer, true),
                };
                if accepted > 0 {
                    let snapshot = self.ae2_snapshot_blocks();
                    let previous = plan.len();
                    let inserted = match self.ae2_item_p2p_insert_staged(target, side, &offered, false, plan) {
                        Some(n) => n,
                        None => self.ae2_native_item_insert_staged(target, side, &offered, offer, false, plan),
                    };
                    let unchanged = matches!(self.world.block_entities.get(&pos), Some(BlockEntity::Ae2(current)) if **current == *original);
                    if inserted == 0 || inserted > offer || !unchanged {
                        self.ae2_restore_blocks(snapshot);
                        plan.truncate(previous);
                    } else {
                        let mut rest = stack;
                        rest.count -= inserted;
                        horizon_client::ae2_stack_identity::synchronize_tracked(&rest);
                        m.slots[OUTPUT] = (rest.count > 0).then_some(rest);
                        moved = true;
                    }
                }
            }
            if let Some((fluid, amount)) = tank(m, TANK_OUT) {
                let key = horizon_client::ae2_cell_codec::fluid_key(&fluid);
                let room = self.advancedae_fluid_insert(target, side, &key, amount, true, plan).min(amount);
                if room > 0 {
                    let inserted = self.advancedae_fluid_insert(target, side, &key, room, false, plan).min(amount);
                    set_tank(m, TANK_OUT, Some((&fluid, amount - inserted)));
                    moved |= inserted > 0;
                }
            }
            if moved {
                return true;
            }
        }
        false
    }
    /// A fluid into the handler at `target` on its side `side`, as an export bus offers it.
    fn advancedae_fluid_insert(&mut self, target: GridAddress, side: Direction, key: &ItemKey, amount: u64, simulate: bool, plan: &mut FormationPlan) -> u64 {
        if let Some(n) = self.ae2_fluid_p2p_fill_staged(target, side, key, amount, simulate, plan) {
            return n;
        }
        if let Some(n) = self.ae2_interface_fluid_move_staged(target, side, key, amount, true, simulate, plan) {
            return n;
        }
        if let Some(n) = self.ae2_provider_return_fluid_at(target, side, key, amount, simulate) {
            return n;
        }
        if let Some(n) = self.ae2_mod_fluid_move(target, side, key, amount, true, simulate) {
            return n;
        }
        self.ae2_tank_fill_encoded(target, key, amount, simulate).unwrap_or(0)
    }

    /// `ReactionChamberMenu.isValidForSlot`: an input takes what some
    /// recipe uses; the output takes nothing.
    pub(in crate::game) fn advancedae_may_place(&self, m: &Machine, slot: usize, item: &ItemStack) -> Option<bool> {
        is_chamber(m).then(|| slot < INPUTS && valid_ingredient(self.res.recipes(), self.res.items(), item))
    }

    /// A press on the chamber's own entries: a tank (a bucket on the cursor
    /// empties into the input tank, or an empty one fills from the output
    /// tank, a bucket at a time), a flush button (`clearFluid`,
    /// `clearFluidOut`), an output side or AUTO_EXPORT (the setting steps on).
    pub(in crate::game) fn advancedae_chamber_click(&mut self, pos: (i32, i32, i32), slot: usize, button: i32, input: horizon_client::menu::Input) -> bool {
        let Some(BlockEntity::Ae2(m)) = self.world.block_entities.get(&pos) else { return false };
        let Some(entry) = slot.checked_sub(300).filter(|e| (TANK_IN_ENTRY..=AUTO_EXPORT_ENTRY).contains(e)) else { return false };
        if !is_chamber(m) {
            return false;
        }
        if input != horizon_client::menu::Input::Pickup || !(0..=1).contains(&button) {
            return true;
        }
        let mut m = (**m).clone();
        match entry {
            TANK_IN_ENTRY => {
                let Some(carried) = self.carried.clone().filter(|c| c.count == 1) else { return true };
                let Some(fluid) = horizon_client::ae2_cell_codec::bucket_fluid(&carried.item) else { return true };
                let Some(amount) = fill_room(&m, fluid, 1000).filter(|n| *n == 1000) else { return true };
                let held = tank(&m, TANK_IN).map_or(0, |(_, n)| n);
                set_tank(&mut m, TANK_IN, Some((fluid, held + amount)));
                self.carried = Some(ItemStack::new("minecraft:bucket", 1));
            }
            TANK_OUT_ENTRY => {
                let Some(carried) = self.carried.clone().filter(|c| c.count == 1 && c.item.as_ref() == "minecraft:bucket") else { return true };
                let _ = carried;
                let Some((fluid, held)) = tank(&m, TANK_OUT).filter(|(_, n)| *n >= 1000) else { return true };
                let Some(bucket) = horizon_client::ae2_cell_codec::fluid_bucket(&fluid).filter(|b| self.res.items().get(b).is_some()) else { return true };
                set_tank(&mut m, TANK_OUT, Some((&fluid, held - 1000)));
                self.carried = Some(ItemStack::new(bucket, 1));
            }
            FLUSH_IN_ENTRY => set_tank(&mut m, TANK_IN, None),
            FLUSH_OUT_ENTRY => set_tank(&mut m, TANK_OUT, None),
            _ => {
                crate::game::ae2_screens::step_setting(&mut m, slot, button == 1);
            }
        }
        if let Some(BlockEntity::Ae2(target)) = self.world.block_entities.get_mut(&pos) {
            **target = m;
        }
        self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
        true
    }

    /// `ReactionChamberBlock.useItemOn` with a bucket: an empty one takes a
    /// bucket of the output tank (only a whole one), a full one empties into
    /// the input tank (only when all of it fits). False leaves the use to the
    /// block (its menu).
    pub(in crate::game) fn advancedae_bucket_use(&mut self, pos: [i32; 3]) -> bool {
        let pos = (pos[0], pos[1], pos[2]);
        if self.remote.is_some() {
            return false;
        }
        let Some(BlockEntity::Ae2(m)) = self.world.block_entities.get(&pos) else { return false };
        if !is_chamber(m) {
            return false;
        }
        let Some(held) = self.held().cloned() else { return false };
        let mut m = (**m).clone();
        if held.item.as_ref() == "minecraft:bucket" {
            let Some((fluid, amount)) = tank(&m, TANK_OUT).filter(|(_, n)| *n >= 1000) else { return false };
            let Some(bucket) = horizon_client::ae2_cell_codec::fluid_bucket(&fluid).filter(|b| self.res.items().get(b).is_some()) else { return false };
            set_tank(&mut m, TANK_OUT, Some((&fluid, amount - 1000)));
            *self.held_mut() = Some(ItemStack::new(bucket, 1));
            if held.count > 1 {
                if let Some(rest) = self.give(ItemStack::new("minecraft:bucket", held.count - 1)) {
                    self.pop_resource([pos.0, pos.1, pos.2], rest);
                }
            }
        } else if let Some(fluid) = horizon_client::ae2_cell_codec::bucket_fluid(&held.item) {
            if fill_room(&m, fluid, 1000) != Some(1000) {
                return false;
            }
            let amount = tank(&m, TANK_IN).map_or(0, |(_, n)| n);
            set_tank(&mut m, TANK_IN, Some((fluid, amount + 1000)));
            *self.held_mut() = Some(ItemStack::new("minecraft:bucket", 1));
        } else {
            return false;
        }
        if let Some(BlockEntity::Ae2(target)) = self.world.block_entities.get_mut(&pos) {
            **target = m;
        }
        self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
        true
    }

    /// The chamber's fluid handler (AE2's over `getTank()`, every side):
    /// `fill` goes to the input tank (`isAllowedIn` refuses the output's).
    pub(in crate::game) fn advancedae_tank_fill(&mut self, address: GridAddress, key: &ItemKey, amount: u64, simulate: bool) -> Option<u64> {
        let Some(BlockEntity::Ae2(m)) = self.ae2_block_entity(address) else { return None };
        if !is_chamber(m) {
            return None;
        }
        if !key.components.is_empty() {
            return Some(0);
        }
        let Some(moved) = fill_room(m, &key.item_id, amount) else { return Some(0) };
        if !simulate && moved > 0 {
            let held = tank(m, TANK_IN).map_or(0, |(_, n)| n);
            if let Some(BlockEntity::Ae2(m)) = self.ae2_block_entity_mut(address) {
                set_tank(m, TANK_IN, Some((&key.item_id, held + moved)));
            }
            self.ae2_mark_modified(address);
        }
        Some(moved)
    }
    /// `drain` comes from the output tank (`extract` refuses the input's).
    pub(in crate::game) fn advancedae_tank_drain(&mut self, address: GridAddress, key: &ItemKey, amount: u64, simulate: bool) -> Option<u64> {
        let Some(BlockEntity::Ae2(m)) = self.ae2_block_entity(address) else { return None };
        if !is_chamber(m) {
            return None;
        }
        let Some((fluid, held)) = tank(m, TANK_OUT).filter(|(id, _)| *id == key.item_id && key.components.is_empty()) else { return Some(0) };
        let moved = amount.min(held);
        if !simulate && moved > 0 {
            if let Some(BlockEntity::Ae2(m)) = self.ae2_block_entity_mut(address) {
                set_tank(m, TANK_OUT, Some((&fluid, held - moved)));
            }
            self.ae2_mark_modified(address);
        }
        Some(moved)
    }
    /// What can be drained: the output tank.
    pub(in crate::game) fn advancedae_tank_contents(&self, address: GridAddress) -> Option<Option<(ItemKey, u64)>> {
        let Some(BlockEntity::Ae2(m)) = self.ae2_block_entity(address) else { return None };
        is_chamber(m).then(|| tank(m, TANK_OUT).map(|(fluid, n)| (horizon_client::ae2_cell_codec::fluid_key(&fluid), n)))
    }
}

/// What the input tank takes of `amount` mB of `fluid` (`GenericStackInv.insert`):
/// none for a fluid the game does not know.
fn fill_room(m: &Machine, fluid: &str, amount: u64) -> Option<u64> {
    if !horizon_client::ae2_cell_codec::valid_fluid_id(fluid) {
        return None;
    }
    Some(match tank(m, TANK_IN) {
        None => amount.min(TANK_CAP),
        Some((held, n)) if held == fluid => amount.min(TANK_CAP.saturating_sub(n)),
        Some(_) => 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn chamber() -> Machine {
        horizon_client::ae2x::install();
        Machine::for_block(rows::REACTION_CHAMBER).unwrap()
    }
    fn stack(id: &str, n: u32) -> Option<ItemStack> {
        Some(ItemStack::new(id, n))
    }
    /// Items of 64 a stack.
    fn items() -> Items {
        Items::for_tests(std::collections::HashMap::new())
    }
    /// fluixcrystals.json, as the jar has it.
    fn fluix() -> Value {
        json!({"type": "advanced_ae:reaction", "input_energy": 200000,
            "input_fluid": {"amount": 500, "ingredient": {"fluid": "minecraft:water"}},
            "input_items": [{"amount": 16, "ingredient": {"item": "ae2:charged_certus_quartz_crystal"}},
                {"amount": 16, "ingredient": {"item": "minecraft:redstone"}}, {"amount": 16, "ingredient": {"item": "minecraft:quartz"}}],
            "output": {"#": 64, "#t": "ae2:i", "id": "ae2:fluix_crystal"}})
    }
    /// A recipe of a fluid output (quantum_infusion.json's shape).
    fn infusion() -> Value {
        json!({"type": "advanced_ae:reaction", "input_energy": 20000,
            "input_fluid": {"amount": 4000, "ingredient": {"fluid": "minecraft:water"}},
            "input_items": [{"ingredient": {"item": "minecraft:stone"}}],
            "output": {"#": 1000, "#t": "ae2:f", "id": "minecraft:lava"}})
    }
    fn filled(m: &mut Machine) {
        m.slots[0] = stack("ae2:charged_certus_quartz_crystal", 10);
        m.slots[1] = stack("ae2:charged_certus_quartz_crystal", 6);
        m.slots[2] = stack("minecraft:redstone", 20);
        m.slots[5] = stack("minecraft:quartz", 16);
        set_tank(m, TANK_IN, Some(("minecraft:water", 1200)));
    }

    #[test]
    fn the_codec_reads_the_mods_recipes() {
        let v = fluix();
        let r = Recipe::parse(&v).unwrap();
        assert_eq!(r.energy, 200_000);
        assert_eq!(r.output, Output::Item(ItemStack::new("ae2:fluix_crystal", 64)));
        assert_eq!(r.items.iter().map(|s| s.amount).collect::<Vec<_>>(), [16, 16, 16]);
        assert_eq!(r.fluid.amount, 500);
        let v = infusion();
        let r = Recipe::parse(&v).unwrap();
        assert_eq!(r.output, Output::Fluid("minecraft:lava".into(), 1000));
        assert_eq!(r.items[0].amount, 1, "an absent amount is one");
        let mut v = fluix();
        v.as_object_mut().unwrap().remove("input_fluid");
        assert!(Recipe::parse(&v).is_none(), "input_fluid is required");
    }

    #[test]
    fn a_recipe_matches_the_inputs_summed_by_item_and_the_input_tank() {
        let (items, v) = (items(), fluix());
        let r = Recipe::parse(&v).unwrap();
        let mut m = chamber();
        filled(&mut m);
        assert!(matches(&m, &r, &items), "10 + 6 charged certus count as 16");
        set_tank(&mut m, TANK_IN, Some(("minecraft:water", 499)));
        assert!(!matches(&m, &r, &items), "too little water");
        set_tank(&mut m, TANK_IN, Some(("minecraft:lava", 1200)));
        assert!(!matches(&m, &r, &items), "not the recipe's fluid");
        set_tank(&mut m, TANK_IN, Some(("minecraft:water", 500)));
        m.slots[5] = stack("minecraft:quartz", 15);
        assert!(!matches(&m, &r, &items));
    }

    #[test]
    fn a_craft_puts_the_output_in_and_takes_the_inputs_in_slot_order() {
        let (items, v) = (items(), fluix());
        let r = Recipe::parse(&v).unwrap();
        let mut m = chamber();
        filled(&mut m);
        assert!(fits(&m, &r, &items));
        finish(&mut m, &r, &items);
        assert_eq!(m.slots[OUTPUT], stack("ae2:fluix_crystal", 64));
        assert_eq!(m.slots[0], None);
        assert_eq!(m.slots[1], None);
        assert_eq!(m.slots[2], stack("minecraft:redstone", 4));
        assert_eq!(m.slots[5], None);
        assert_eq!(tank(&m, TANK_IN), Some(("minecraft:water".into(), 700)));
        filled(&mut m);
        assert!(!fits(&m, &r, &items), "64 fluix fill the output slot");
    }

    #[test]
    fn a_fluid_output_goes_to_the_output_tank_up_to_its_capacity() {
        let (items, v) = (items(), infusion());
        let r = Recipe::parse(&v).unwrap();
        let mut m = chamber();
        m.slots[3] = stack("minecraft:stone", 2);
        set_tank(&mut m, TANK_IN, Some(("minecraft:water", 8000)));
        assert!(matches(&m, &r, &items) && fits(&m, &r, &items));
        finish(&mut m, &r, &items);
        assert_eq!(tank(&m, TANK_OUT), Some(("minecraft:lava".into(), 1000)));
        assert_eq!(tank(&m, TANK_IN), Some(("minecraft:water".into(), 4000)));
        assert_eq!(m.slots[3], stack("minecraft:stone", 1));
        set_tank(&mut m, TANK_OUT, Some(("minecraft:lava", 15_001)));
        assert!(!fits(&m, &r, &items));
        set_tank(&mut m, TANK_OUT, Some(("minecraft:water", 10)));
        assert!(!fits(&m, &r, &items), "another fluid in the output tank");
    }

    #[test]
    fn the_sides_turn_with_the_chambers_facing() {
        let mut m = chamber();
        for key in ["output_front", "output_left", "output_top"] {
            set_flag(&mut m, key, true);
        }
        assert_eq!(output_directions(&m, Direction::North), [Direction::North, Direction::Up, Direction::West]);
        assert_eq!(output_directions(&m, Direction::East), [Direction::East, Direction::Up, Direction::North]);
        assert_eq!(output_directions(&m, Direction::South), [Direction::South, Direction::Up, Direction::East]);
        assert_eq!(output_directions(&m, Direction::West), [Direction::West, Direction::Up, Direction::South]);
        assert!(!auto_export_work(&m), "AUTO_EXPORT is off");
        m.config.insert(AUTO_EXPORT.into(), Tag::String("YES".into()));
        assert!(!auto_export_work(&m), "nothing to push");
        set_tank(&mut m, TANK_OUT, Some(("minecraft:lava", 10)));
        assert!(auto_export_work(&m));
    }

    /// A host of a fixed recipe and power.
    struct Fake {
        own: f64,
        grid: f64,
        idle: f64,
        energy: i32,
        found: bool,
        taken: Vec<(bool, f64)>,
    }
    impl Host for Fake {
        fn own_power(&mut self, _: &mut Machine, amount: f64, simulate: bool) -> f64 {
            let got = amount.min(self.own);
            if !simulate {
                self.own -= got;
                self.taken.push((true, got));
            }
            got
        }
        fn grid_power(&mut self, amount: f64, simulate: bool) -> f64 {
            let got = amount.min(self.grid);
            if !simulate {
                self.grid -= got;
                self.taken.push((false, got));
            }
            got
        }
        fn grid_idle(&mut self) -> f64 {
            self.idle
        }
        fn find(&self, _: &Machine) -> Option<String> {
            self.found.then(|| "r".to_owned())
        }
        fn recipe(&self, m: &Machine, _: &str) -> Option<(i32, bool)> {
            Some((self.energy, m.slots[OUTPUT].is_none()))
        }
        fn finish(&self, m: &mut Machine, _: &str) {
            m.slots[OUTPUT] = stack("ae2:fluix_crystal", 64);
        }
    }

    #[test]
    fn a_craft_takes_its_energy_over_its_steps_by_the_speed_cards() {
        // 200,000 AE: no card, 100 steps of 2 at 2,000; four cards, 4 steps of 50 at 50,000.
        for (cards, steps, each) in [(0, 100, 2000.0), (1, 67, 2985.0), (4, 4, 50_000.0)] {
            let mut m = chamber();
            for slot in m.upgrades.iter_mut().take(cards) {
                *slot = stack("ae2:speed_card", 1);
            }
            let mut host = Fake { own: 0.0, grid: 1e9, idle: 0.0, energy: 200_000, found: true, taken: Vec::new() };
            let mut ctx = Ctx::default();
            let mut n = 0;
            while m.slots[OUTPUT].is_none() && n < 1000 {
                execute(&mut m, &mut ctx, n == 0, &mut host);
                n += 1;
                assert!(n == steps || working(&m), "working while it crafts");
            }
            assert_eq!(n, steps, "{cards} cards");
            assert_eq!(host.taken.first(), Some(&(false, each)), "{cards} cards: the grid pays each step");
            assert!(!working(&m) && m.progress == 0.0);
        }
    }

    #[test]
    fn the_own_buffer_pays_first_and_too_little_power_raises_the_alert() {
        let mut m = chamber();
        let mut host = Fake { own: 4000.0, grid: 0.0, idle: 0.0, energy: 200_000, found: true, taken: Vec::new() };
        let mut ctx = Ctx::default();
        execute(&mut m, &mut ctx, true, &mut host);
        execute(&mut m, &mut ctx, false, &mut host);
        assert_eq!(host.taken, [(true, 2000.0), (true, 2000.0)]);
        assert_eq!(m.progress, 4.0);
        // A step with neither: no progress, and the alert only once some power was offered.
        execute(&mut m, &mut ctx, false, &mut host);
        assert_eq!(m.progress, 4.0);
        assert!(!flag(&m, rows::WARNING));
        // Four cards: 50,000 a step; 30,000 in the grid is 3/5 of it: floor(0.6 * 50) = 30 steps' worth.
        for slot in m.upgrades.iter_mut() {
            *slot = stack("ae2:speed_card", 1);
        }
        host.grid = 30_000.0;
        execute(&mut m, &mut ctx, false, &mut host);
        assert_eq!(host.taken.last(), Some(&(false, 30_000.0)));
        assert_eq!(m.progress, 34.0);
        assert!(flag(&m, rows::WARNING), "too little power for full speed");
        host.grid = 1e9;
        execute(&mut m, &mut ctx, false, &mut host);
        assert!(!flag(&m, rows::WARNING));
    }

    #[test]
    fn a_change_that_leaves_no_recipe_drops_the_progress() {
        let mut m = chamber();
        let mut host = Fake { own: 1e9, grid: 0.0, idle: 0.0, energy: 200_000, found: true, taken: Vec::new() };
        let mut ctx = Ctx::default();
        execute(&mut m, &mut ctx, true, &mut host);
        assert_eq!(m.progress, 2.0);
        host.found = false;
        execute(&mut m, &mut ctx, false, &mut host);
        assert_eq!(m.progress, 4.0, "no change seen: the task it has goes on (cachedTask)");
        execute(&mut m, &mut ctx, true, &mut host);
        assert_eq!(m.progress, 0.0);
        assert!(!working(&m) && ctx.task.is_none());
    }

    #[test]
    fn the_input_tank_takes_one_fluid_up_to_its_capacity() {
        let mut m = chamber();
        assert_eq!(fill_room(&m, "minecraft:water", 20_000), Some(TANK_CAP));
        set_tank(&mut m, TANK_IN, Some(("minecraft:water", 15_500)));
        assert_eq!(fill_room(&m, "minecraft:water", 1000), Some(500));
        assert_eq!(fill_room(&m, "minecraft:lava", 1000), Some(0));
        assert!(slot_insert(m.kind, 8) == Some(true) && slot_insert(m.kind, OUTPUT) == Some(false));
        assert!(slot_extract(m.kind, 0) == Some(false) && slot_extract(m.kind, OUTPUT) == Some(true));
    }
}
