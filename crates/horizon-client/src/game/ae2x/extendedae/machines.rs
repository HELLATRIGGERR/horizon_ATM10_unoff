//! ExtendedAE's own machines in the game: the Crystal Assembler, the Circuit
//! Cutter (the Circuit Slicer) and the Crystal Fixer, their recipe types and
//! their grid ticks. Derived from ExtendedAE 1.21-2.2.42 (`TileCrystalAssembler`,
//! `TileCircuitCutter`, `TileCrystalFixer`, `RecipeExecutor`, `FCUtil`,
//! `CrystalAssemblerRecipe`, `CircuitCutterRecipe`, `CrystalFixerRecipe`) and
//! Glodium 1.21-2.2 (`RecipeSearchContext`, `IngredientStack`),
//! https://github.com/GlodBlock/ExtendedAE commit
//! 1f86f927552f02451236465c0c96a12a3de8f66c, LGPL-3.0; see `ae2/addons/extendedae/`.
//!
//! A machine's recipe search (`RecipeSearchContext`: the current and last
//! recipe, `stuck`, `dirty`) lives in [`State`] by block entity, not in its
//! save: the mod's `load` reads the ids from the wrong tag, so a reloaded
//! machine has no recipe and starts its craft over (its progress is not saved
//! either, but the Crystal Fixer's).
use super::super::super::{Game, ae2_processing::json_ingredient, ae2_tick_tracker::Modulation};
use horizon_client::{
    ae2_world::Machine,
    ae2x::extendedae as rows,
    containers::BlockEntity,
    crafting::Recipes,
    items::{ItemStack, Items},
};
use horizon_core::{geometry::Direction, random::RandomSource};
use horizon_world::nbt::Tag;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// `Recipes::fluid_tags`: a fluid tag's members.
type FluidTags = HashMap<String, HashSet<String>>;

/// The machine's recipe type, inputs and pace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum Kind {
    Assembler,
    Cutter,
    Fixer,
}
impl Kind {
    pub(in crate::game) fn of(m: &Machine) -> Option<Self> {
        Some(match m.kind.ext()?.id {
            rows::CRYSTAL_ASSEMBLER => Self::Assembler,
            rows::CIRCUIT_CUTTER => Self::Cutter,
            rows::CRYSTAL_FIXER => Self::Fixer,
            _ => return None,
        })
    }
    pub(in crate::game) fn recipe_type(self) -> &'static str {
        match self {
            Self::Assembler => "extendedae:crystal_assembler",
            Self::Cutter => "extendedae:circuit_cutter",
            Self::Fixer => "extendedae:crystal_fixer",
        }
    }
    /// The input slots; the output slot comes after them.
    pub(in crate::game) fn inputs(self) -> usize {
        match self {
            Self::Assembler => 9,
            Self::Cutter | Self::Fixer => 1,
        }
    }
    /// `MAX_PROGRESS`.
    pub(in crate::game) fn max_progress(self) -> f64 {
        match self {
            Self::Assembler | Self::Cutter => 200.0,
            Self::Fixer => 100.0,
        }
    }
    /// `RecipeExecutor.energyMultiplier`: AE a progress step costs.
    fn energy_multiplier(self) -> f64 {
        match self {
            Self::Assembler | Self::Cutter => 10.0,
            Self::Fixer => 50.0,
        }
    }
}

/// `TileCrystalAssembler.TANK_CAP`, mB.
pub(in crate::game) const TANK_CAP: u64 = 16000;
const TANK: &str = "tank_in";
pub(in crate::game) const AUTO_EXPORT: &str = "auto_export";
pub(in crate::game) const OUTPUT_SIDES: &str = "output_side";
pub(in crate::game) const WORKING: &str = rows::WORKING;

/// `FCUtil.speedCardMap`: progress a step adds, by installed speed cards.
pub(in crate::game) fn speed(cards: usize) -> i32 {
    match cards {
        0 => 2,
        1 => 3,
        2 => 5,
        3 => 10,
        _ => 50,
    }
}

/// The tank's fluid and mB (`GenericStackInv`, one slot, fluids only).
pub(in crate::game) fn tank(m: &Machine) -> Option<(String, u64)> {
    let Some(Tag::Compound(data)) = m.config.get(&TANK.into()) else { return None };
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
pub(in crate::game) fn set_tank(m: &mut Machine, fluid: Option<(&str, u64)>) {
    match fluid.filter(|(_, n)| *n > 0) {
        Some((id, n)) => {
            m.config.insert(TANK.into(), Tag::compound([("id", Tag::String(id.into())), ("amount", Tag::Long(n as i64))]));
        }
        None => {
            m.config.remove(&TANK.into());
        }
    }
}

/// `Settings.AUTO_EXPORT` is YES.
pub(in crate::game) fn auto_export(m: &Machine) -> bool {
    matches!(m.config.get(&AUTO_EXPORT.into()), Some(Tag::String(s)) if s.to_string().ok().as_deref() == Some("YES"))
}
/// `getOutputSides`: those saved, or all six when none were (`loadTag`).
pub(in crate::game) fn output_sides(m: &Machine) -> Vec<Direction> {
    let Some(Tag::List(names)) = m.config.get(&OUTPUT_SIDES.into()) else { return Direction::ALL.to_vec() };
    // An EnumSet iterates in Direction order.
    Direction::ALL
        .into_iter()
        .filter(|d| names.iter().any(|n| matches!(n, Tag::String(s) if s.to_string_lossy() == direction_name(*d))))
        .collect()
}
pub(in crate::game) fn set_output_side(m: &mut Machine, side: Direction, on: bool) {
    let mut sides = output_sides(m);
    sides.retain(|d| *d != side);
    if on {
        sides.push(side);
    }
    let names = Direction::ALL.into_iter().filter(|d| sides.contains(d)).map(|d| Tag::String(direction_name(d).into())).collect();
    m.config.insert(OUTPUT_SIDES.into(), Tag::List(names));
}
pub(in crate::game) fn direction_name(d: Direction) -> &'static str {
    match d {
        Direction::Down => "down",
        Direction::Up => "up",
        Direction::North => "north",
        Direction::South => "south",
        Direction::West => "west",
        Direction::East => "east",
    }
}
/// `hasAutoExportWork`: an output to push and AUTO_EXPORT on.
fn auto_export_work(kind: Kind, m: &Machine) -> bool {
    kind != Kind::Fixer && auto_export(m) && m.slots.get(kind.inputs()).is_some_and(Option::is_some)
}

/// One `IngredientStack`: an `Ingredient` (or `FluidIngredient`) and its amount.
#[derive(Clone, Debug)]
struct Stack<'a> {
    ingredient: &'a Value,
    amount: u32,
}
impl<'a> Stack<'a> {
    /// `IngredientStack.ITEM_CODEC`: `ingredient`, `amount` (1 when absent, positive).
    fn parse(v: &'a Value) -> Option<Self> {
        let amount = match v.get("amount") {
            None => 1,
            Some(n) => u32::try_from(n.as_u64()?).ok().filter(|n| *n > 0)?,
        };
        Some(Self { ingredient: v.get("ingredient")?, amount })
    }
}

/// A Crystal Assembler or Circuit Cutter recipe, as its codec reads it.
#[derive(Clone, Debug)]
pub(in crate::game) struct Recipe<'a> {
    items: Vec<Stack<'a>>,
    fluid: Option<Stack<'a>>,
    pub(in crate::game) output: ItemStack,
}
impl<'a> Recipe<'a> {
    pub(in crate::game) fn parse(kind: Kind, v: &'a Value) -> Option<Self> {
        let output = output_stack(v.get("output")?)?;
        let (items, fluid) = match kind {
            // CrystalAssemblerRecipeSerializer: input_items, input_fluid (optional).
            Kind::Assembler => (
                v.get("input_items")?.as_array()?.iter().map(Stack::parse).collect::<Option<Vec<_>>>()?,
                match v.get("input_fluid") {
                    Some(f) => Some(Stack::parse(f)?),
                    None => None,
                },
            ),
            // CircuitCutterRecipeSerializer: input.
            Kind::Cutter => (vec![Stack::parse(v.get("input")?)?], None),
            Kind::Fixer => return None,
        };
        Some(Self { items, fluid, output })
    }
}
/// `ItemStack.CODEC`.
fn output_stack(v: &Value) -> Option<ItemStack> {
    v.get("id")?.as_str()?;
    let count = match v.get("count") {
        Some(n) => n.as_u64()?,
        None => 1,
    };
    if count == 0 || count > u64::from(u32::MAX) || v.get("components").is_some_and(|c| !c.is_object()) {
        return None;
    }
    Some(horizon_client::crafting::Template::parse(v)?.create())
}
/// NeoForge's `FluidIngredient`: a fluid, a fluid tag, or a list of them.
fn fluid_matches(tags: &FluidTags, v: &Value, fluid: &str) -> bool {
    match v {
        Value::Array(list) => list.iter().any(|v| fluid_matches(tags, v, fluid)),
        _ => {
            if let Some(id) = v.get("fluid").and_then(Value::as_str) {
                return id == fluid;
            }
            v.get("tag").and_then(Value::as_str).and_then(|tag| tags.get(tag)).is_some_and(|members| members.contains(fluid))
        }
    }
}

/// What a tester consumes from: a copied item stack, or the tank's fluid.
#[derive(Clone, Debug, PartialEq)]
enum Held {
    Item(ItemStack),
    Fluid(String, u64),
}

/// The output slot (`AppEngInternalInventory(.., 1, 64)`) takes all of `out`.
fn output_fits(items: &Items, slot: Option<&ItemStack>, out: &ItemStack) -> bool {
    let max = items.stack_max(out).min(64);
    match slot {
        None => out.count <= max,
        Some(s) => s.same_item_same_components(out) && s.count.checked_add(out.count).is_some_and(|n| n <= max),
    }
}

/// `testRecipe`: the output fits, then each ingredient in turn takes from
/// copies of the inputs, in slot order, and the tank's fluid last; one that
/// is not filled fails it.
pub(in crate::game) fn test(kind: Kind, m: &Machine, recipe: &Recipe, items: &Items, tags: &FluidTags) -> bool {
    let n = kind.inputs();
    if !output_fits(items, m.slots.get(n).and_then(Option::as_ref), &recipe.output) {
        return false;
    }
    let mut copy: Vec<Held> = m.slots[..n].iter().flatten().filter(|s| s.count > 0).cloned().map(Held::Item).collect();
    if kind == Kind::Assembler {
        if let Some((fluid, amount)) = tank(m) {
            copy.push(Held::Fluid(fluid, amount));
        }
    }
    for (stack, is_fluid) in recipe.items.iter().map(|s| (s, false)).chain(recipe.fluid.iter().map(|s| (s, true))) {
        let mut left = stack.amount;
        for held in &mut copy {
            left = consume(items, tags, stack, is_fluid, held, left);
            if left == 0 {
                break;
            }
        }
        if left > 0 {
            return false;
        }
        copy.retain(|h| match h {
            Held::Item(s) => s.count > 0,
            Held::Fluid(_, n) => *n > 0,
        });
    }
    true
}
/// `IngredientStack.consume` when `checkType` holds: takes up to `left` from
/// `held` if the ingredient matches it; gives what is still wanted.
fn consume(items: &Items, tags: &FluidTags, stack: &Stack, is_fluid: bool, held: &mut Held, left: u32) -> u32 {
    if left == 0 {
        return 0;
    }
    match held {
        Held::Item(s) if !is_fluid && s.count > 0 && json_ingredient(items, Some(stack.ingredient), Some(s)) => {
            let taken = s.count.min(left);
            s.count -= taken;
            left - taken
        }
        Held::Fluid(fluid, n) if is_fluid && *n > 0 && fluid_matches(tags, stack.ingredient, fluid) => {
            let taken = (*n).min(u64::from(left));
            *n -= taken;
            left - taken as u32
        }
        _ => left,
    }
}
/// `runRecipe`: each ingredient takes from the input slots in order, then
/// (the Crystal Assembler's) from the tank.
pub(in crate::game) fn run(kind: Kind, m: &mut Machine, recipe: &Recipe, items: &Items, tags: &FluidTags) {
    let n = kind.inputs();
    let mut fluid = tank(m).map(|(id, amount)| Held::Fluid(id, amount));
    for (stack, is_fluid) in recipe.items.iter().map(|s| (s, false)).chain(recipe.fluid.iter().map(|s| (s, true))) {
        let mut left = stack.amount;
        for slot in &mut m.slots[..n] {
            if let Some(s) = slot.take() {
                let mut held = Held::Item(s);
                left = consume(items, tags, stack, is_fluid, &mut held, left);
                if let Held::Item(s) = held {
                    *slot = (s.count > 0).then_some(s);
                }
            }
            if left == 0 {
                break;
            }
        }
        if let Some(held) = fluid.as_mut() {
            if left > 0 {
                consume(items, tags, stack, is_fluid, held, left);
            }
        }
    }
    if kind == Kind::Assembler {
        if let Some(Held::Fluid(id, amount)) = fluid {
            set_tank(m, Some((&id, amount)));
        } else {
            set_tank(m, None);
        }
    }
}

/// A Crystal Fixer recipe (`CrystalFixerRecipeSerializer`): the block in
/// front, the one it may become, the fuel, and the chance in 10,000ths.
#[derive(Clone, Debug)]
pub(in crate::game) struct FixerRecipe<'a> {
    pub(in crate::game) input: &'a str,
    pub(in crate::game) output: &'a str,
    fuel: Stack<'a>,
    pub(in crate::game) chance: u32,
}
impl<'a> FixerRecipe<'a> {
    pub(in crate::game) fn parse(v: &'a Value) -> Option<Self> {
        let chance = u32::try_from(v.get("chance")?.as_u64()?).ok()?;
        Some(Self {
            input: v.get("input")?.get("id")?.as_str()?,
            output: v.get("output")?.get("id")?.as_str()?,
            fuel: Stack::parse(v.get("fuel")?)?,
            chance,
        })
    }
    /// `testRecipe`: the block in front is the input and the fuel slot holds
    /// enough of the fuel.
    pub(in crate::game) fn test(&self, front: &str, fuel: Option<&ItemStack>, items: &Items) -> bool {
        front == self.input && fuel.is_some_and(|s| s.count >= self.fuel.amount && json_ingredient(items, Some(self.fuel.ingredient), Some(s)))
    }
    /// `roll`: FULL_CHANCE (10,000) always; else `nextInt(10000) < chance`.
    pub(in crate::game) fn roll(&self, draw: impl FnOnce() -> u32) -> bool {
        self.chance >= 10000 || draw() < self.chance
    }
}

/// `RecipeSearchContext` of one machine.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::game) struct Ctx {
    pub(in crate::game) current: Option<String>,
    last: Option<String>,
    stuck: bool,
    dirty: bool,
    /// What the inventories held at the last tick: a change is `onInvChange`.
    seen: String,
}
impl Ctx {
    /// `onInvChange`.
    fn changed(&mut self) {
        self.stuck = false;
        self.dirty = true;
    }
    /// `shouldTick`.
    fn should_tick(&self) -> bool {
        self.current.is_some() || !self.stuck
    }
    /// `findRecipe`: the last recipe if it still passes, else a search.
    fn find(&mut self, test: impl Fn(&str) -> bool, search: impl FnOnce() -> Option<String>) {
        if let Some(last) = self.last.clone() {
            if test(&last) {
                self.current = Some(last);
                self.stuck = false;
                return;
            }
            self.last = None;
        }
        self.stuck = false;
        // onFind
        match search() {
            None if self.dirty => self.dirty = false,
            None => {
                self.stuck = true;
                self.current = None;
            }
            Some(found) => {
                self.dirty = false;
                self.last = Some(found.clone());
                self.current = Some(found);
                self.stuck = false;
            }
        }
    }
}

/// The searches of the machines loaded, by block entity incarnation.
#[derive(Default)]
pub(in crate::game) struct State {
    machines: HashMap<u64, Ctx>,
}

/// What a tick of [`execute`] needs from the world.
pub(in crate::game) trait Host {
    /// AE the machine's own buffer gives (or would give), with the
    /// configured usage multiplier, as `extractAEPower(.., CONFIG)` returns.
    fn own_power(&mut self, m: &mut Machine, amount: f64, simulate: bool) -> f64;
    /// The same from the grid's energy service.
    fn grid_power(&mut self, amount: f64, simulate: bool) -> f64;
    fn test(&self, m: &Machine, id: &str) -> bool;
    /// The first recipe that passes.
    fn search(&self, m: &Machine) -> Option<String>;
    /// `runRecipe` and the output put in; false when it is gone.
    fn finish(&mut self, m: &mut Machine, id: &str) -> bool;
}

/// `RecipeExecutor.execute(speed, true)`: a recipe in progress takes its
/// power (the machine's own buffer first, else the grid's) and steps; at
/// `MAX_PROGRESS` it runs if it still passes. Without one it searches.
pub(in crate::game) fn execute(kind: Kind, m: &mut Machine, ctx: &mut Ctx, speed: i32, host: &mut impl Host) -> Modulation {
    if let Some(current) = ctx.current.clone() {
        m.config.insert(WORKING.into(), Tag::Byte(1));
        let cost = kind.energy_multiplier() * f64::from(speed);
        let threshold = cost - 0.01;
        if host.own_power(m, cost, true) > threshold {
            host.own_power(m, cost, false);
            m.progress += f64::from(speed);
        } else if host.grid_power(cost, true) > threshold {
            host.grid_power(cost, false);
            m.progress += f64::from(speed);
        }
        if m.progress >= kind.max_progress() {
            m.progress = 0.0;
            // testRecipe holds the output's room, so the mod's `stuck` branch never runs.
            if host.test(m, &current) {
                host.finish(m, &current);
            }
            ctx.current = None;
        }
        Modulation::Urgent
    } else {
        if ctx.should_tick() {
            let snapshot = m.clone();
            ctx.find(|id| host.test(&snapshot, id), || host.search(&snapshot));
        }
        if ctx.current.is_none() {
            m.config.insert(WORKING.into(), Tag::Byte(0));
        }
        Modulation::Faster
    }
}

/// The token of what `onChangeInventory` and `onChangeTank` see.
fn inventory_token(m: &Machine, front: Option<&str>) -> String {
    format!("{:?}|{:?}|{front:?}", m.slots, m.config.get(&TANK.into()))
}

struct GameHost<'a, 'g> {
    game: &'a mut Game<'g>,
    pos: (i32, i32, i32),
    kind: Kind,
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
    fn test(&self, m: &Machine, id: &str) -> bool {
        let (items, recipes) = (self.game.res.items(), self.game.res.recipes());
        find(recipes, self.kind, id).is_some_and(|r| test(self.kind, m, &r, items, &recipes.fluid_tags))
    }
    fn search(&self, m: &Machine) -> Option<String> {
        let (items, recipes) = (self.game.res.items(), self.game.res.recipes());
        // The recipe manager's order is its map's; here the recipes' load order.
        recipes.ae2_custom.get(self.kind.recipe_type())?.iter().find_map(|(id, v)| {
            let r = Recipe::parse(self.kind, v)?;
            (items.get(&r.output.item).is_some() && test(self.kind, m, &r, items, &recipes.fluid_tags)).then(|| id.clone())
        })
    }
    fn finish(&mut self, m: &mut Machine, id: &str) -> bool {
        let (items, recipes) = (self.game.res.items(), self.game.res.recipes());
        let Some(r) = find(recipes, self.kind, id) else { return false };
        run(self.kind, m, &r, items, &recipes.fluid_tags);
        let out = &mut m.slots[self.kind.inputs()];
        match out {
            Some(s) => s.count += r.output.count,
            None => *out = Some(r.output.clone()),
        }
        true
    }
}
/// The recipe of type `kind` named `id`.
fn find<'r>(recipes: &'r Recipes, kind: Kind, id: &str) -> Option<Recipe<'r>> {
    let (_, v) = recipes.ae2_custom.get(kind.recipe_type())?.iter().find(|(name, _)| name == id)?;
    Recipe::parse(kind, v)
}

struct FixerHost<'a, 'g> {
    game: &'a mut Game<'g>,
    pos: (i32, i32, i32),
    front: Option<String>,
    online: Option<bool>,
}
impl FixerHost<'_, '_> {
    fn recipe<'r>(recipes: &'r Recipes, id: &str) -> Option<FixerRecipe<'r>> {
        let (_, v) = recipes.ae2_custom.get(Kind::Fixer.recipe_type())?.iter().find(|(name, _)| name == id)?;
        FixerRecipe::parse(v)
    }
}
impl Host for FixerHost<'_, '_> {
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
    fn test(&self, m: &Machine, id: &str) -> bool {
        let Some(front) = self.front.as_deref() else { return false };
        Self::recipe(self.game.res.recipes(), id).is_some_and(|r| r.test(front, m.slots[0].as_ref(), self.game.res.items()))
    }
    fn search(&self, m: &Machine) -> Option<String> {
        let front = self.front.as_deref()?;
        let items = self.game.res.items();
        self.game.res.recipes().ae2_custom.get(Kind::Fixer.recipe_type())?.iter().find_map(|(id, v)| {
            FixerRecipe::parse(v)?.test(front, m.slots[0].as_ref(), items).then(|| id.clone())
        })
    }
    fn finish(&mut self, m: &mut Machine, id: &str) -> bool {
        let Some(r) = Self::recipe(self.game.res.recipes(), id) else { return false };
        let (output, amount) = (r.output.to_owned(), r.fuel.amount);
        if let Some(fuel) = m.slots[0].as_mut() {
            fuel.count = fuel.count.saturating_sub(amount);
        }
        if m.slots[0].as_ref().is_some_and(|s| s.count == 0) {
            m.slots[0] = None;
        }
        let roll = r.roll(|| self.game.world.random.next_int_bound(10000).map_or(0, |n| n as u32));
        if roll {
            self.game.extendedae_fixer_set_front(self.pos, &output);
        }
        true
    }
}

impl Game<'_> {
    /// The block in front of the Crystal Fixer at `pos` (`getFacingBlock`) and where it is.
    fn extendedae_fixer_front(&self, pos: (i32, i32, i32)) -> Option<(String, [i32; 3])> {
        let state = self.pack.assets.registry.state(self.world.block(pos.0, pos.1, pos.2))?;
        let facing = state.properties.get("facing").map_or("north", String::as_str);
        let d = Direction::ALL.into_iter().find(|d| direction_name(*d) == facing)?.normal();
        let at = [pos.0 + d[0], pos.1 + d[1], pos.2 + d[2]];
        let front = self.pack.assets.registry.state(self.world.block(at[0], at[1], at[2]))?;
        Some((front.name.to_string(), at))
    }
    /// `setNewBlock`: the output's default state (`setBlockAndUpdate`).
    fn extendedae_fixer_set_front(&mut self, pos: (i32, i32, i32), block: &str) {
        let Some((_, at)) = self.extendedae_fixer_front(pos) else { return };
        let Some(id) = self.pack.assets.registry.default_state(block) else { return };
        if let Err(error) = self.level_set(at, id) {
            log::warn!("Crystal Fixer at {pos:?}: {error}");
        }
    }

    /// The grid ticks of ExtendedAE's own machines (`tickingRequest`), with
    /// AE2's tick tracker: the Crystal Assembler and Circuit Cutter at the
    /// Inscriber's rates, the Crystal Fixer at 2 to 10 ticks.
    pub(in crate::game) fn tick_extendedae_machines(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let mut found: Vec<_> = self
            .world
            .block_entities
            .iter()
            .filter_map(|(p, e)| match e {
                BlockEntity::Ae2(m) => Kind::of(m).map(|kind| (*p, kind)),
                _ => None,
            })
            .collect();
        found.sort_unstable_by_key(|(p, _)| *p);
        let live: std::collections::BTreeSet<u64> = found.iter().filter_map(|(p, _)| self.world.block_entities.incarnation(p)).collect();
        self.ae2x.extendedae.machines.retain(|id, _| live.contains(id));
        for (pos, kind) in found {
            let Some(incarnation) = self.world.block_entities.incarnation(&pos) else { continue };
            let Some(BlockEntity::Ae2(original)) = self.world.block_entities.get(&pos) else { continue };
            let mut m = (**original).clone();
            let before = m.clone();
            let front = (kind == Kind::Fixer).then(|| self.extendedae_fixer_front(pos).map(|(name, _)| name)).flatten();
            let mut ctx = self.ae2x.extendedae.machines.get(&incarnation).cloned().unwrap_or_default();
            let token = inventory_token(&m, front.as_deref());
            if ctx.seen != token {
                ctx.changed();
                ctx.seen = token;
            }
            let sleeping = !ctx.should_tick() && !auto_export_work(kind, &m);
            let wake = format!("{}|{:?}|{:?}|{:?}", ctx.seen, m.upgrades, m.config.get(&AUTO_EXPORT.into()), m.config.get(&OUTPUT_SIDES.into()));
            let (min, max) = match kind {
                Kind::Fixer => (2, 10),
                _ => horizon_client::ae2_tick_config::rates(horizon_client::ae2_tick_config::Kind::Inscriber),
            };
            if !self.ae2_variable_processor_due(pos, min, max, sleeping, wake.clone()) {
                self.ae2x.extendedae.machines.insert(incarnation, ctx);
                continue;
            }
            let mut formation = super::super::super::ae2_buses::FormationPlan::default();
            let modulation = if kind != Kind::Fixer && self.extendedae_push_out(pos, kind, &mut m, &before, &mut formation) {
                Modulation::Urgent
            } else if kind == Kind::Fixer {
                // genProgress: nextInt(2, 4).
                let step = 2 + self.world.random.next_int_bound(2).unwrap_or(0);
                let mut host = FixerHost { game: self, pos, front, online: None };
                execute(kind, &mut m, &mut ctx, step, &mut host)
            } else {
                let step = speed(m.installed_speed_cards());
                let mut host = GameHost { game: self, pos, kind, online: None };
                execute(kind, &mut m, &mut ctx, step, &mut host)
            };
            self.ae2_variable_processor_completed(pos, modulation, wake);
            self.ae2x.extendedae.machines.insert(incarnation, ctx);
            if m != before {
                if let Some(BlockEntity::Ae2(target)) = self.world.block_entities.get_mut(&pos) {
                    **target = m;
                    self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
                }
                if let Err(error) = self.ae2_commit_formation(formation) {
                    log::warn!("ExtendedAE machine notification: {error}");
                }
            }
        }
    }

    /// `pushOutResult` (`FCUtil.ejectInv`): with AUTO_EXPORT on, up to 64 of
    /// the output go to the first output side, in Direction order, whose
    /// neighbour takes any; a machine of the same kind is passed over.
    fn extendedae_push_out(&mut self, pos: (i32, i32, i32), kind: Kind, m: &mut Machine, original: &Machine, plan: &mut super::super::super::ae2_buses::FormationPlan) -> bool {
        if !auto_export_work(kind, m) {
            return false;
        }
        let out = kind.inputs();
        for face in output_sides(m) {
            let d = face.normal();
            let neighbour = (pos.0 + d[0], pos.1 + d[1], pos.2 + d[2]);
            match self.world.block_entities.get(&neighbour) {
                None => continue,
                Some(BlockEntity::Ae2(other)) if Kind::of(other) == Some(kind) => continue,
                Some(_) => {}
            }
            let Some(target) = self.ae2_local_address(neighbour) else { continue };
            let Some(stack) = m.slots[out].clone() else { return false };
            let offer = stack.count.min(64);
            let mut offered = stack.clone();
            offered.count = offer;
            horizon_client::ae2_stack_identity::detach_if_tracked(&mut offered);
            let accepted = match self.ae2_item_p2p_insert(target, face.opposite(), &offered, true) {
                Some(n) => n.min(offer),
                None => self.ae2_native_item_insert(target, face.opposite(), &offered, offer, true),
            };
            if accepted == 0 {
                continue;
            }
            let before = self.ae2_snapshot_blocks();
            let previous = plan.len();
            let moved = match self.ae2_item_p2p_insert_staged(target, face.opposite(), &offered, false, plan) {
                Some(n) => n,
                None => self.ae2_native_item_insert_staged(target, face.opposite(), &offered, offer, false, plan),
            };
            let unchanged = matches!(self.world.block_entities.get(&pos), Some(BlockEntity::Ae2(current)) if **current == *original);
            if moved == 0 || moved > offer || !unchanged {
                self.ae2_restore_blocks(before);
                plan.truncate(previous);
                continue;
            }
            let mut rest = stack;
            rest.count -= moved;
            horizon_client::ae2_stack_identity::synchronize_tracked(&rest);
            m.slots[out] = (rest.count > 0).then_some(rest);
            return true;
        }
        false
    }

    /// A press on a window entry of the Crystal Assembler or Circuit Cutter
    /// that is no slot of theirs: the tank (a bucket empties into it, as
    /// AE2's `EMPTY_ITEM` on a storage slot; nothing comes back out:
    /// `canExtract` is false), the AUTO_EXPORT toggle, and an output side
    /// (`CEAEGenericPacket("set_side")`). True when it was one of those.
    pub(in crate::game) fn extendedae_machine_click(&mut self, pos: (i32, i32, i32), entry: usize, button: i32, input: horizon_client::menu::Input) -> bool {
        let Some(BlockEntity::Ae2(m)) = self.world.block_entities.get(&pos) else { return false };
        let Some(kind) = Kind::of(m).filter(|k| *k != Kind::Fixer) else { return false };
        let own = (entry == super::TANK_ENTRY && kind == Kind::Assembler) || entry == super::AUTO_EXPORT_ENTRY || super::side_entry(entry).is_some();
        if !own {
            return false;
        }
        if input != horizon_client::menu::Input::Pickup || !(0..=1).contains(&button) {
            return true;
        }
        let mut m = (**m).clone();
        if entry == super::TANK_ENTRY {
            let Some(carried) = self.carried.clone() else { return true };
            let Some(fluid) = horizon_client::ae2_cell_codec::bucket_fluid(&carried.item) else { return true };
            let amount = match tank(&m) {
                None => 1000,
                Some((id, n)) if id == fluid && n + 1000 <= TANK_CAP => n + 1000,
                Some(_) => return true,
            };
            if carried.count != 1 || self.res.items().get("minecraft:bucket").is_none() {
                return true;
            }
            set_tank(&mut m, Some((fluid, amount)));
            self.carried = Some(ItemStack::new("minecraft:bucket", 1));
        } else if entry == super::AUTO_EXPORT_ENTRY {
            let on = !auto_export(&m);
            m.config.insert(AUTO_EXPORT.into(), Tag::String(if on { "YES" } else { "NO" }.into()));
        } else if let Some(side) = super::side_entry(entry) {
            let on = !output_sides(&m).contains(&side);
            set_output_side(&mut m, side, on);
        }
        if let Some(BlockEntity::Ae2(target)) = self.world.block_entities.get_mut(&pos) {
            **target = m;
        }
        self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
        true
    }

    /// `TileCrystalFixer.refuel`, the fixer's `openGui`: an empty hand takes
    /// its fuel back; a held stack goes in as far as it fits. A wrench turns
    /// it first (`AEBaseEntityBlock.useItemOn`).
    pub(in crate::game) fn extendedae_fixer_use(&mut self, pos: (i32, i32, i32)) -> bool {
        let Some(BlockEntity::Ae2(m)) = self.world.block_entities.get(&pos) else { return false };
        if Kind::of(m) != Some(Kind::Fixer) || self.held().is_some_and(|s| self.ae2_wrench_can_rotate(s)) {
            return false;
        }
        let mut m = (**m).clone();
        match self.held().cloned() {
            None => {
                let Some(fuel) = m.slots[0].take() else { return true };
                if let Some(rest) = self.give(fuel) {
                    self.pop_resource([pos.0, pos.1, pos.2], rest);
                }
            }
            Some(mut held) => {
                let max = self.res.items().stack_max(&held).min(64);
                let room = match &m.slots[0] {
                    None => max,
                    Some(s) if s.same_item_same_components(&held) => max.saturating_sub(s.count),
                    Some(_) => 0,
                };
                let moved = room.min(held.count);
                if moved > 0 {
                    match &mut m.slots[0] {
                        Some(s) => s.count += moved,
                        slot @ None => *slot = Some(ItemStack { count: moved, ..held.clone() }),
                    }
                    held.count -= moved;
                    *self.held_mut() = (held.count > 0).then_some(held);
                }
            }
        }
        if let Some(BlockEntity::Ae2(target)) = self.world.block_entities.get_mut(&pos) {
            **target = m;
        }
        self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn machine(id: &str) -> Machine {
        horizon_client::ae2x::install();
        Machine::for_block(id).unwrap()
    }
    fn stack(id: &str, n: u32) -> Option<ItemStack> {
        Some(ItemStack::new(id, n))
    }
    /// Items of 64 a stack, and no fluid tags.
    fn data() -> (Items, FluidTags) {
        (Items::for_tests(HashMap::new()), FluidTags::new())
    }
    fn fluix() -> Value {
        json!({"type":"extendedae:crystal_assembler","input_fluid":{"amount":100,"ingredient":{"fluid":"minecraft:water"}},
            "input_items":[{"amount":4,"ingredient":{"item":"ae2:charged_certus_quartz_crystal"}},{"amount":4,"ingredient":{"item":"minecraft:redstone"}},{"amount":4,"ingredient":{"item":"minecraft:quartz"}}],
            "output":{"count":8,"id":"ae2:fluix_crystal"}})
    }

    #[test]
    fn speed_cards_map_as_fcutil_does() {
        assert_eq!((0..=5).map(speed).collect::<Vec<_>>(), [2, 3, 5, 10, 50, 50]);
    }

    #[test]
    fn assembler_takes_ingredients_across_slots_and_water_from_its_tank() {
        let (items, tags) = data();
        let v = fluix();
        let r = Recipe::parse(Kind::Assembler, &v).unwrap();
        let mut m = machine(rows::CRYSTAL_ASSEMBLER);
        m.slots[0] = stack("minecraft:redstone", 2);
        m.slots[3] = stack("ae2:charged_certus_quartz_crystal", 5);
        m.slots[4] = stack("minecraft:redstone", 3);
        m.slots[8] = stack("minecraft:quartz", 4);
        // No water yet.
        assert!(!test(Kind::Assembler, &m, &r, &items, &tags));
        set_tank(&mut m, Some(("minecraft:water", 99)));
        assert!(!test(Kind::Assembler, &m, &r, &items, &tags));
        set_tank(&mut m, Some(("minecraft:water", 1000)));
        assert!(test(Kind::Assembler, &m, &r, &items, &tags));
        run(Kind::Assembler, &mut m, &r, &items, &tags);
        assert_eq!(m.slots[0], None);
        assert_eq!(m.slots[3], stack("ae2:charged_certus_quartz_crystal", 1));
        assert_eq!(m.slots[4], stack("minecraft:redstone", 1));
        assert_eq!(m.slots[8], None);
        assert_eq!(tank(&m), Some(("minecraft:water".to_owned(), 900)));
        // A full output fails the test.
        m.slots[9] = stack("ae2:fluix_crystal", 60);
        m.slots[0] = stack("minecraft:redstone", 4);
        m.slots[1] = stack("ae2:charged_certus_quartz_crystal", 4);
        m.slots[2] = stack("minecraft:quartz", 4);
        assert!(!test(Kind::Assembler, &m, &r, &items, &tags));
        m.slots[9] = stack("ae2:fluix_crystal", 56);
        assert!(test(Kind::Assembler, &m, &r, &items, &tags));
        // The last of the tank empties it.
        set_tank(&mut m, Some(("minecraft:water", 100)));
        run(Kind::Assembler, &mut m, &r, &items, &tags);
        assert_eq!(tank(&m), None);
    }

    #[test]
    fn cutter_reads_its_single_input() {
        let (items, tags) = data();
        let v = json!({"type":"extendedae:circuit_cutter","input":{"ingredient":{"item":"minecraft:glass"},"amount":2},"output":{"id":"ae2:printed_silicon","count":9}});
        let r = Recipe::parse(Kind::Cutter, &v).unwrap();
        assert_eq!(r.output.count, 9);
        let mut m = machine(rows::CIRCUIT_CUTTER);
        m.slots[0] = stack("minecraft:glass", 1);
        assert!(!test(Kind::Cutter, &m, &r, &items, &tags));
        m.slots[0] = stack("minecraft:glass", 3);
        assert!(test(Kind::Cutter, &m, &r, &items, &tags));
        run(Kind::Cutter, &mut m, &r, &items, &tags);
        assert_eq!(m.slots[0], stack("minecraft:glass", 1));
    }

    /// The search state as Glodium's RecipeSearchContext steps it.
    #[test]
    fn search_context_sleeps_on_the_second_empty_search_after_a_change() {
        let mut ctx = Ctx::default();
        ctx.changed();
        ctx.find(|_| false, || None);
        assert!(ctx.should_tick() && !ctx.dirty);
        ctx.find(|_| false, || None);
        assert!(!ctx.should_tick() && ctx.stuck);
        ctx.changed();
        ctx.find(|_| false, || Some("r".into()));
        assert_eq!((ctx.current.as_deref(), ctx.last.as_deref()), (Some("r"), Some("r")));
        ctx.current = None;
        // The last recipe is tried before any search.
        ctx.find(|id| id == "r", || panic!("searched"));
        assert_eq!(ctx.current.as_deref(), Some("r"));
    }

    struct Fake {
        own: f64,
        grid: f64,
        passes: bool,
        finished: u32,
    }
    impl Host for Fake {
        fn own_power(&mut self, _: &mut Machine, amount: f64, simulate: bool) -> f64 {
            let n = amount.min(self.own);
            if !simulate {
                self.own -= n;
            }
            n
        }
        fn grid_power(&mut self, amount: f64, simulate: bool) -> f64 {
            let n = amount.min(self.grid);
            if !simulate {
                self.grid -= n;
            }
            n
        }
        fn test(&self, _: &Machine, _: &str) -> bool {
            self.passes
        }
        fn search(&self, _: &Machine) -> Option<String> {
            self.passes.then(|| "r".into())
        }
        fn finish(&mut self, _: &mut Machine, _: &str) -> bool {
            self.finished += 1;
            true
        }
    }

    /// No cards: 2 progress and 20 AE a step, 100 steps a craft after the
    /// step that finds the recipe; four cards: 50 and 500 AE, four steps.
    #[test]
    fn executor_steps_and_pays_as_recipe_executor_does() {
        for (cards, steps, ae) in [(0, 100, 2000.0), (1, 67, 2010.0), (2, 40, 2000.0), (3, 20, 2000.0), (4, 4, 2000.0)] {
            let mut m = machine(rows::CRYSTAL_ASSEMBLER);
            let mut ctx = Ctx::default();
            let mut host = Fake { own: 0.0, grid: 1e9, passes: true, finished: 0 };
            assert_eq!(execute(Kind::Assembler, &mut m, &mut ctx, speed(cards), &mut host), Modulation::Faster);
            let mut n = 0;
            while host.finished == 0 {
                assert_eq!(execute(Kind::Assembler, &mut m, &mut ctx, speed(cards), &mut host), Modulation::Urgent);
                n += 1;
            }
            assert_eq!((n, 1e9 - host.grid), (steps, ae), "{cards} cards");
            assert_eq!(m.progress, 0.0);
            assert!(ctx.current.is_none());
        }
        // Its own buffer pays first; without power nothing moves.
        let mut m = machine(rows::CRYSTAL_ASSEMBLER);
        let mut ctx = Ctx { current: Some("r".into()), ..Ctx::default() };
        let mut host = Fake { own: 25.0, grid: 0.0, passes: true, finished: 0 };
        execute(Kind::Assembler, &mut m, &mut ctx, 2, &mut host);
        assert_eq!((host.own, m.progress), (5.0, 2.0));
        execute(Kind::Assembler, &mut m, &mut ctx, 2, &mut host);
        assert_eq!((host.own, m.progress), (5.0, 2.0));
    }

    #[test]
    fn fixer_recipes_read_their_blocks_fuel_and_chance() {
        let (items, _) = data();
        let v = json!({"input":{"id":"ae2:flawed_budding_quartz","count":1},"output":{"id":"ae2:flawless_budding_quartz","count":1},"fuel":{"ingredient":{"item":"ae2:charged_certus_quartz_crystal"}},"chance":500});
        let r = FixerRecipe::parse(&v).unwrap();
        assert!(r.test("ae2:flawed_budding_quartz", stack("ae2:charged_certus_quartz_crystal", 1).as_ref(), &items));
        assert!(!r.test("ae2:chipped_budding_quartz", stack("ae2:charged_certus_quartz_crystal", 1).as_ref(), &items));
        assert!(!r.test("ae2:flawed_budding_quartz", None, &items));
        assert!(r.roll(|| 499) && !r.roll(|| 500));
        let full = FixerRecipe { chance: 10000, ..r.clone() };
        assert!(full.roll(|| 9999));
    }

    #[test]
    fn output_sides_default_to_all_and_keep_direction_order() {
        let mut m = machine(rows::CRYSTAL_ASSEMBLER);
        assert_eq!(output_sides(&m), Direction::ALL.to_vec());
        set_output_side(&mut m, Direction::Up, false);
        set_output_side(&mut m, Direction::Down, false);
        assert_eq!(output_sides(&m), [Direction::North, Direction::South, Direction::West, Direction::East]);
        set_output_side(&mut m, Direction::Down, true);
        assert_eq!(output_sides(&m)[0], Direction::Down);
    }
}
