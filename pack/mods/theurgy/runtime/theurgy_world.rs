// Theurgy's core spagyrics apparatus in the game (support module `crate::game::theurgy_world` of the `theurgy` native
// bundle): the pyromantic brazier, the calcination oven, the liquefaction cauldron, the distiller, the incubator and
// its three vessels, the sal ammoniac accumulator and tank. The rules are the bundle's crate `theurgy_core`; this
// module is the glue: the recipes read from the world's tables, the block entities (their slots are the engine's,
// their saved keys Java's), the clicks, heat from below, the fluid tanks and their handlers. Translated from Theurgy
// 1.78.0 (MIT, Copyright 2022 klikli-dev; see this bundle's LICENSE-theurgy). Server hooks run on the thread of the
// `Game` that simulates the world, so the recipe book and the fields Java keeps in memory (the heat cache,
// `isProcessing`, `couldCraftLastTick`, the last recipe found) are kept per thread, dropped when the server opens its
// world and when it closes it.

use super::Game;
use crate::game::api_values::{TryIntoApi, TryIntoEngine};
use horizon_client::{containers::ModContainer, items::ItemStack, text::Text};
use horizon_core::geometry::Direction;
use horizon_core::nbt::{Compound, JavaString, Tag};
use horizon_world::fluid_stack::FluidStack;
use horizon_world::level::flags;
use horizon_world::registry::BlockStateId;
use mod_api::{BlockUpdateFlags, ModBlockEntity, NativeFluidTankInfo, WorldPhase};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use theurgy_core::{
    crafting::{self, Crafting, Step},
    heat::{Brazier, Consume, Fuel, HeatConsumer},
    recipe::{self, Accumulation, Calcination, Distillation, Incubation, Liquefaction, RecipeResult},
    reformation::{self, Catalysation, Catalyst, Reformation},
    storage::{self, BUCKET, Tank},
    vat::{self, VatKind, VatRecipe},
};

// ---- The apparatus ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Brazier,
    Oven,
    Cauldron,
    Distiller,
    Incubator,
    Mercury,
    Salt,
    Sulfur,
    Accumulator,
    Tank,
    Fermentation,
    Digestion,
    SourcePedestal,
    TargetPedestal,
    ResultPedestal,
    Emitter,
    Catalyst,
}

const KINDS: [(Kind, &str); 17] = [
    (Kind::Brazier, "theurgy:pyromantic_brazier"),
    (Kind::Oven, "theurgy:calcination_oven"),
    (Kind::Cauldron, "theurgy:liquefaction_cauldron"),
    (Kind::Distiller, "theurgy:distiller"),
    (Kind::Incubator, "theurgy:incubator"),
    (Kind::Mercury, "theurgy:incubator_mercury_vessel"),
    (Kind::Salt, "theurgy:incubator_salt_vessel"),
    (Kind::Sulfur, "theurgy:incubator_sulfur_vessel"),
    (Kind::Accumulator, "theurgy:sal_ammoniac_accumulator"),
    (Kind::Tank, "theurgy:sal_ammoniac_tank"),
    (Kind::Fermentation, "theurgy:fermentation_vat"),
    (Kind::Digestion, "theurgy:digestion_vat"),
    (Kind::SourcePedestal, "theurgy:reformation_source_pedestal"),
    (Kind::TargetPedestal, "theurgy:reformation_target_pedestal"),
    (Kind::ResultPedestal, "theurgy:reformation_result_pedestal"),
    (Kind::Emitter, "theurgy:sulfuric_flux_emitter"),
    (Kind::Catalyst, "theurgy:mercury_catalyst"),
];

impl Kind {
    fn of(block: &str) -> Option<Self> {
        KINDS.iter().find(|(_, b)| *b == block).map(|(k, _)| *k)
    }

    fn block(self) -> &'static str {
        KINDS.iter().find(|(k, _)| *k == self).map_or("", |(_, b)| b)
    }

    /// Two blocks high (`DoubleBlockHalf`): the lower half has the block entity.
    fn double(self) -> bool {
        matches!(self, Self::Oven | Self::Cauldron | Self::Distiller | Self::Incubator)
    }

    /// The item slots: the brazier's fuel; input and output (the combined handler's 0 and 1); the incubator's
    /// output; a vessel's or the accumulator's one.
    fn slots(self) -> usize {
        match self {
            Self::Oven | Self::Cauldron | Self::Distiller => 2,
            Self::Fermentation | Self::Digestion => vat::INPUT_SLOTS + 1,
            Self::Tank | Self::Emitter => 0,
            _ => 1,
        }
    }

    /// The vessel's item tag (`IncubatorXVesselBlockEntity.InputInventory.isItemValid`).
    fn vessel_tag(self) -> Option<&'static str> {
        match self {
            Self::Mercury => Some("theurgy:alchemical_mercuries"),
            Self::Salt => Some("theurgy:alchemical_salts"),
            Self::Sulfur => Some("theurgy:alchemical_sulfurs"),
            _ => None,
        }
    }

    /// The apparatus' fluid tank: its saved key and capacity.
    fn tank(self) -> Option<(&'static str, i32)> {
        match self {
            Self::Cauldron => Some(("solventTank", 2 * BUCKET)),
            Self::Accumulator => Some(("waterTank", 10 * BUCKET)),
            Self::Tank => Some(("tank", 2 * BUCKET)),
            Self::Fermentation | Self::Digestion => Some(("fluidTank", 10 * BUCKET)),
            _ => None,
        }
    }
}

const INPUT: usize = 0;
const OUTPUT: usize = 1;
const SOLVENT_TAG: &str = "theurgy:solvent";

// ---- The recipe book and the fields Java keeps in memory --------------------------------------------------------

#[derive(Default)]
struct Book {
    calcination: Vec<Calcination>,
    distillation: Vec<Distillation>,
    liquefaction: Vec<Liquefaction>,
    incubation: Vec<Incubation>,
    accumulation: Vec<Accumulation>,
    fermentation: Vec<VatRecipe>,
    digestion: Vec<VatRecipe>,
    reformation: Vec<Reformation>,
    catalysation: Vec<Catalysation>,
}

/// What an apparatus keeps beside its saved compound: the heat cache, `isProcessing` and `couldCraftLastTick`, the
/// `CachedCheck`'s last recipe, the input it saw last tick (to hear `onContentTypeChanged`), whether its upper half
/// was seen, the incubator's vessels.
#[derive(Clone, Debug, Default)]
struct Live {
    heat: HeatConsumer,
    processing: bool,
    could_craft: bool,
    last: Option<usize>,
    input: Option<(String, u32)>,
    seen: bool,
    upper_seen: bool,
    vessels: Option<[[i32; 3]; 3]>,
    vessel_inputs: [Option<(String, u32)>; 3],
    /// The reformation array's pedestals' items (the target's, then the sources'), as the emitter heard them last.
    array: Vec<Option<String>>,
}

thread_local! {
    static BOOK: RefCell<Option<Rc<Book>>> = const { RefCell::new(None) };
    static LIVE: RefCell<HashMap<(&'static str, [i32; 3]), Live>> = RefCell::new(HashMap::new());
    /// The last click on a block (level time, position, taken): a second one in the same tick is the off hand's.
    static LAST_CLICK: RefCell<Option<(i64, [i32; 3], bool)>> = const { RefCell::new(None) };
    /// The pedestals a player has picked with the sulfuric flux emitter in hand (Java keeps them on the client).
    static SELECTION: RefCell<HashMap<u128, Vec<([i32; 3], Kind)>>> = RefCell::new(HashMap::new());
}

fn forget() {
    BOOK.with(|b| *b.borrow_mut() = None);
    LIVE.with(|l| l.borrow_mut().clear());
    LAST_CLICK.with(|c| *c.borrow_mut() = None);
    SELECTION.with(|s| s.borrow_mut().clear());
}

/// `native_server_started`: the recipe book and the machines' unsaved fields are dropped.
pub(super) fn opened() {
    forget();
}

/// `native_world_lifecycle` v2: the same as the world closes or fails to open.
pub(super) fn lifecycle(phase: WorldPhase) {
    if matches!(phase, WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed | WorldPhase::WorldOpenFailed) {
        forget();
    }
}

fn load_book(game: &Game<'_>) -> Book {
    let mut book = Book::default();
    let mut bad = 0;
    for r in game.native_recipes_of_type(recipe::CALCINATION).iter() {
        match Calcination::parse(&r.id, &r.json) {
            Ok(p) => book.calcination.push(p),
            Err(e) => { bad += 1; log::warn!("[theurgy] {e}") }
        }
    }
    for r in game.native_recipes_of_type(recipe::DISTILLATION).iter() {
        match Distillation::parse(&r.id, &r.json) {
            Ok(p) => book.distillation.push(p),
            Err(e) => { bad += 1; log::warn!("[theurgy] {e}") }
        }
    }
    for r in game.native_recipes_of_type(recipe::LIQUEFACTION).iter() {
        match Liquefaction::parse(&r.id, &r.json) {
            Ok(p) => book.liquefaction.push(p),
            Err(e) => { bad += 1; log::warn!("[theurgy] {e}") }
        }
    }
    for r in game.native_recipes_of_type(recipe::INCUBATION).iter() {
        match Incubation::parse(&r.id, &r.json) {
            Ok(p) => book.incubation.push(p),
            Err(e) => { bad += 1; log::warn!("[theurgy] {e}") }
        }
    }
    for r in game.native_recipes_of_type(recipe::ACCUMULATION).iter() {
        match Accumulation::parse(&r.id, &r.json) {
            Ok(p) => book.accumulation.push(p),
            Err(e) => { bad += 1; log::warn!("[theurgy] {e}") }
        }
    }
    for (list, kind, name) in [(&mut book.fermentation, VatKind::Fermentation, vat::FERMENTATION), (&mut book.digestion, VatKind::Digestion, vat::DIGESTION)] {
        for r in game.native_recipes_of_type(name).iter() {
            match VatRecipe::parse(kind, &r.id, &r.json) {
                Ok(p) => list.push(p),
                Err(e) => { bad += 1; log::warn!("[theurgy] {e}") }
            }
        }
    }
    for r in game.native_recipes_of_type(reformation::REFORMATION).iter() {
        match Reformation::parse(&r.id, &r.json) {
            Ok(p) => book.reformation.push(p),
            Err(e) => { bad += 1; log::warn!("[theurgy] {e}") }
        }
    }
    for r in game.native_recipes_of_type(reformation::CATALYSATION).iter() {
        match Catalysation::parse(&r.id, &r.json) {
            Ok(p) => book.catalysation.push(p),
            Err(e) => { bad += 1; log::warn!("[theurgy] {e}") }
        }
    }
    log::info!(
        "[theurgy] recipes: {} calcination, {} distillation, {} liquefaction, {} incubation, {} accumulation, {} fermentation, {} digestion, {} reformation, {} catalysation ({bad} unread)",
        book.calcination.len(),
        book.distillation.len(),
        book.liquefaction.len(),
        book.incubation.len(),
        book.accumulation.len(),
        book.fermentation.len(),
        book.digestion.len(),
        book.reformation.len(),
        book.catalysation.len()
    );
    book
}

fn book(game: &Game<'_>) -> Rc<Book> {
    if let Some(b) = BOOK.with(|b| b.borrow().clone()) {
        return b;
    }
    let fresh = Rc::new(load_book(game));
    BOOK.with(|b| *b.borrow_mut() = Some(fresh.clone()));
    fresh
}

fn live(dimension: &'static str, pos: [i32; 3]) -> Live {
    LIVE.with(|l| l.borrow().get(&(dimension, pos)).cloned().unwrap_or_default())
}

fn set_live(dimension: &'static str, pos: [i32; 3], value: Live) {
    LIVE.with(|l| l.borrow_mut().insert((dimension, pos), value));
}

// ---- Tags, stacks, the saved compound --------------------------------------------------------------------------

fn item_in_tag(game: &Game<'_>, item: &str, tag: &str) -> bool {
    game.native_server_item_in_tag(&ItemStack::new(item, 1), tag).unwrap_or(false)
}

fn fluid_in_tag(fluid: &str, tag: &str) -> bool {
    FluidStack::of(fluid, 1).is_some_and(|f| f.is_tag(tag))
}

fn key(name: &str) -> JavaString {
    JavaString::from(name)
}

fn int(nbt: &Compound, name: &str) -> i32 {
    match nbt.get(&key(name)) {
        Some(Tag::Int(v)) => *v,
        Some(Tag::Short(v)) => i32::from(*v),
        Some(Tag::Byte(v)) => i32::from(*v),
        Some(Tag::Long(v)) => *v as i32,
        _ => 0,
    }
}

fn long(nbt: &Compound, name: &str) -> i64 {
    match nbt.get(&key(name)) {
        Some(Tag::Long(v)) => *v,
        Some(Tag::Int(v)) => i64::from(*v),
        _ => 0,
    }
}

/// `CraftingBehaviour.saveAdditional`: `progress` and `totalTime` as shorts.
fn read_crafting(nbt: &Compound, live: &Live) -> Crafting {
    Crafting {
        progress: int(nbt, "progress"),
        total_time: int(nbt, "totalTime"),
        processing: live.processing,
        could_craft_last_tick: live.could_craft,
    }
}

fn write_crafting(nbt: &mut Compound, c: &Crafting, live: &mut Live) {
    nbt.insert(key("progress"), Tag::Short(c.progress as i16));
    nbt.insert(key("totalTime"), Tag::Short(c.total_time as i16));
    live.processing = c.processing;
    live.could_craft = c.could_craft_last_tick;
}

/// A `FluidTank` as `writeToNBT` saves it: `{Fluid: {id, amount}}`, an empty compound when empty.
fn read_tank(nbt: &Compound, name: &str, capacity: i32) -> Tank {
    let mut tank = Tank::new(capacity);
    if let Some(Tag::Compound(t)) = nbt.get(&key(name))
        && let Some(fluid) = t.get(&key("Fluid"))
        && let Some(stack) = FluidStack::parse(fluid)
        && !stack.is_empty()
    {
        tank.fluid = Some(stack.name().to_owned());
        tank.amount = stack.amount();
    }
    tank
}

fn write_tank(nbt: &mut Compound, name: &str, tank: &Tank) {
    let mut t = Compound::new();
    if !tank.is_empty()
        && let Some(stack) = FluidStack::of(tank.fluid_id(), tank.amount)
        && let Some(saved) = stack.save()
    {
        t.insert(key("Fluid"), Tag::Compound(saved));
    }
    nbt.insert(key(name), Tag::Compound(t));
}

/// The engine's stack in slot `i` of a ticking (API) block entity; none when empty.
fn slot(game: &Game<'_>, entity: &ModBlockEntity, i: usize) -> Option<ItemStack> {
    entity.items.get(i).and_then(|s| s.as_ref()).and_then(|s| s.try_into_engine(game.items()).ok()).filter(|s| s.count > 0)
}

fn set_slot(entity: &mut ModBlockEntity, i: usize, stack: Option<ItemStack>, slots: usize) {
    if entity.items.len() < slots.max(i + 1) {
        entity.items.resize(slots.max(i + 1), None);
    }
    entity.items[i] = stack.filter(|s| s.count > 0).and_then(|s| s.try_into_api().ok());
}

/// Slot `i` of a block entity in the level (the engine's stacks).
fn level_slot(game: &mut Game<'_>, pos: [i32; 3], i: usize) -> Option<ItemStack> {
    game.native_with_mod_block_entity(pos, |e| e.items.get(i).cloned().flatten()).flatten().filter(|s| s.count > 0)
}

fn set_level_slot(game: &mut Game<'_>, pos: [i32; 3], i: usize, stack: Option<ItemStack>, slots: usize) -> bool {
    game.native_with_mod_block_entity(pos, |e| {
        if e.items.len() < slots.max(i + 1) {
            e.items.resize(slots.max(i + 1), None);
        }
        e.items[i] = stack.filter(|s| s.count > 0);
    })
    .is_some()
}

fn max_stack(game: &Game<'_>, stack: &ItemStack) -> u32 {
    game.native_max_stack_size(&stack.item).unwrap_or(64).max(1)
}

/// What a slot holding `held` takes of `incoming` (`ItemStackHandler.insertItem` with the slot's `isItemValid`).
fn insertable(game: &Game<'_>, held: Option<&ItemStack>, incoming: &ItemStack, valid: bool) -> u32 {
    let same = match held {
        None => Some(0),
        Some(h) if h.same_item_same_components(incoming) => Some(h.count),
        Some(_) => None,
    };
    storage::insert(same, incoming.count, max_stack(game, incoming), valid)
}

/// `insertItemStacked(output, result, simulate)` into a one-slot output: whether all of it fits.
fn output_fits(game: &Game<'_>, held: Option<&ItemStack>, result: &ItemStack) -> bool {
    insertable(game, held, result, true) == result.count
}

fn merged(held: Option<ItemStack>, result: &ItemStack) -> ItemStack {
    match held {
        Some(mut h) => {
            h.count += result.count;
            h
        }
        None => result.clone(),
    }
}

fn shrink(stack: Option<ItemStack>, by: u32) -> Option<ItemStack> {
    stack.and_then(|mut s| {
        s.count = s.count.saturating_sub(by);
        (s.count > 0).then_some(s)
    })
}

/// `RecipeResult.getStack`: an item result as written, a tag result as the tag's first item (`TagUtil`).
fn result_stack(game: &Game<'_>, result: &RecipeResult) -> Option<ItemStack> {
    match result {
        RecipeResult::Item(r) => game.native_recipe_result(&r.json),
        RecipeResult::Tag { tag, count, .. } => {
            let first = game.native_tag_items()?.item_tag_order.get(tag.as_str())?.first()?.clone();
            let mut stack = ItemStack::new(&first, 1);
            stack.count = *count;
            Some(stack)
        }
    }
}

fn below(pos: [i32; 3]) -> [i32; 3] {
    [pos[0], pos[1] - 1, pos[2]]
}

fn above(pos: [i32; 3]) -> [i32; 3] {
    [pos[0], pos[1] + 1, pos[2]]
}

fn game_time(game: &Game<'_>) -> i64 {
    game.fork_level_here().and_then(|l| game.level_game_time(l)).unwrap_or(0)
}

/// The input as `onContentTypeChanged` tells changes apart: its item and count (an insert of the same item counts too,
/// as Java's `insertItem` calls it whenever it took something).
fn input_key(stack: Option<&ItemStack>) -> Option<(String, u32)> {
    stack.map(|s| (s.item.to_string(), s.count))
}

/// Whether the input changed as Java hears it: another item, empty to full or back, or more of the same.
fn input_changed(before: &Option<(String, u32)>, now: &Option<(String, u32)>) -> bool {
    match (before, now) {
        (None, None) => false,
        (Some((a, n)), Some((b, m))) => a != b || m > n,
        _ => true,
    }
}

// ---- Block entities ---------------------------------------------------------------------------------------------

/// `native_block_entity_types` (compat hook): one type per apparatus, named after its block.
pub(super) fn block_entity_types(types: &mut Vec<(String, Vec<String>)>) {
    for (_, block) in KINDS {
        types.push((block.to_owned(), vec![block.to_owned()]));
    }
}

/// `native_block_entity`: each apparatus' saved keys, empty.
pub(super) fn new_entity(block: &str, entity: &mut Option<Compound>) {
    if entity.is_some() {
        return;
    }
    let Some(kind) = Kind::of(block) else { return };
    let mut nbt = Compound::new();
    match kind {
        Kind::Brazier => {
            nbt.insert(key("remainingLitTime"), Tag::Short(0));
        }
        Kind::Oven | Kind::Cauldron | Kind::Distiller | Kind::Incubator | Kind::Accumulator | Kind::Fermentation | Kind::Digestion => {
            if !matches!(kind, Kind::Accumulator | Kind::Fermentation | Kind::Digestion) {
                nbt.insert(key("heatReceiver"), Tag::Long(0));
            }
            nbt.insert(key("progress"), Tag::Short(0));
            nbt.insert(key("totalTime"), Tag::Short(0));
        }
        Kind::Emitter => {
            nbt.insert(key("mercuryFluxStorage"), Tag::Int(0));
            nbt.insert(key("progress"), Tag::Short(0));
            nbt.insert(key("totalTime"), Tag::Short(0));
            nbt.insert(key("sourcePedestals"), Tag::List(Vec::new()));
        }
        Kind::Catalyst => {
            nbt.insert(key("mercuryFluxStorage"), Tag::Int(0));
            nbt.insert(key("mercuryFluxToConvert"), Tag::Int(0));
            nbt.insert(key("currentMercuryFluxPerTick"), Tag::Int(0));
        }
        _ => {}
    }
    if let Some((name, capacity)) = kind.tank() {
        write_tank(&mut nbt, name, &Tank::new(capacity));
    }
    *entity = Some(nbt);
}

/// `native_block_entity_ticks`: the brazier and the crafting apparatus tick (the vessels and the tank don't).
pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if let Some(kind) = Kind::of(block)
        && !matches!(kind, Kind::Mercury | Kind::Salt | Kind::Sulfur | Kind::Tank | Kind::SourcePedestal | Kind::TargetPedestal | Kind::ResultPedestal)
    {
        *ticks = true;
    }
}

/// `native_block_entity_container`: the item handlers hoppers see. Input then output for the oven, cauldron and
/// distiller (`CombinedInvWrapper`: both give, only the input takes); the incubator's output gives only.
pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if container.is_some() {
        return;
    }
    let Some(kind) = Kind::of(block) else { return };
    let (slots, inputs, outputs): (usize, Vec<usize>, Vec<usize>) = match kind {
        Kind::Oven | Kind::Cauldron | Kind::Distiller => (2, vec![INPUT], vec![INPUT, OUTPUT]),
        Kind::Incubator => (1, Vec::new(), vec![0]),
        // The open vat's combined handler: the inputs take and give, the output gives (a closed vat's is read only).
        Kind::Fermentation | Kind::Digestion => (vat::INPUT_SLOTS + 1, (0..vat::INPUT_SLOTS).collect(), (0..=vat::INPUT_SLOTS).collect()),
        Kind::Tank | Kind::Emitter => return,
        // The result pedestal gives only (`PreventInsertWrapper`).
        Kind::ResultPedestal => (1, Vec::new(), vec![0]),
        _ => (1, vec![0], vec![0]),
    };
    let all: Vec<usize> = (0..slots).collect();
    *container = Some(ModContainer { slots, faces: std::array::from_fn(|_| all.clone()), inputs, outputs, menu: None });
}

/// `native_block_entity_tick`.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let Some(kind) = Kind::of(entity.id()) else { return };
    if kind.double() && !double_ok(game, pos, kind) {
        return;
    }
    match kind {
        Kind::Brazier => tick_brazier(game, pos, entity),
        Kind::Oven | Kind::Cauldron | Kind::Distiller => tick_single(game, pos, entity, kind),
        Kind::Incubator => tick_incubator(game, pos, entity),
        Kind::Accumulator => tick_accumulator(game, pos, entity),
        Kind::Fermentation | Kind::Digestion => tick_vat(game, pos, entity, kind),
        Kind::Emitter => tick_emitter(game, pos, entity),
        Kind::Catalyst => tick_catalyst(game, pos, entity),
        _ => {}
    }
}

/// The two halves (`updateShape`, heard here at the halves' ticks): an upper half without its lower one goes, a lower
/// half whose upper half was there and is gone goes too. True for a lower half that runs on.
fn double_ok(game: &mut Game<'_>, pos: [i32; 3], kind: Kind) -> bool {
    let dimension = game.native_dimension();
    let upper = game.native_block_property(pos, "half").as_deref() == Some("upper");
    if upper {
        if game.native_block(below(pos)) != kind.block() {
            remove(game, pos);
        }
        return false;
    }
    let mut l = live(dimension, pos);
    let has_upper = game.native_block(above(pos)) == kind.block() && game.native_block_property(above(pos), "half").as_deref() == Some("upper");
    if has_upper {
        if !l.upper_seen {
            l.upper_seen = true;
            set_live(dimension, pos, l);
        }
        return true;
    }
    if l.upper_seen {
        LIVE.with(|m| m.borrow_mut().remove(&(dimension, pos)));
        remove(game, pos);
        return false;
    }
    true
}

fn remove(game: &mut Game<'_>, pos: [i32; 3]) {
    if let (Some(level), Some(air)) = (game.fork_level_here(), game.native_default_state("minecraft:air")) {
        let _ = game.level_set_block(level, pos, air, BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS);
    }
}

/// `HeatConsumerBehaviour.isHeated`: a lit pyromantic brazier below (the only heat provider), looked at every 20
/// ticks; a change sets the apparatus' `lit`.
fn heated(game: &mut Game<'_>, pos: [i32; 3], live: &mut Live, hot_until: i64) -> bool {
    let time = game_time(game);
    let under = below(pos);
    let (heated, changed) = live.heat.is_heated(
        time,
        || game.native_block(under) == Kind::Brazier.block() && game.native_block_property(under, "lit").as_deref() == Some("true"),
        hot_until,
    );
    if changed && game.native_block_property(pos, "lit").is_some() {
        game.native_set_block_property_with(pos, "lit", if heated { "true" } else { "false" }, flags::UPDATE_ALL);
    }
    heated
}

// ---- The pyromantic brazier --------------------------------------------------------------------------------------

fn fuel(game: &Game<'_>, stack: Option<&ItemStack>) -> Option<Fuel> {
    stack.map(|s| Fuel { burn_time: game.native_burn_time(s), has_remainder: game.crafting_remainder(s).is_some() })
}

/// `PyromanticBrazierBlockEntity.tickServer`.
fn tick_brazier(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let held = slot(game, entity, 0);
    let mut b = Brazier { remaining_lit_time: int(&entity.nbt, "remainingLitTime") };
    let step = b.tick(fuel(game, held.as_ref()));
    match step.consume {
        Consume::None => {}
        Consume::Remainder => {
            let rest = held.as_ref().and_then(|s| game.crafting_remainder(s));
            set_slot(entity, 0, rest, 1);
        }
        Consume::One => set_slot(entity, 0, shrink(held, 1), 1),
    }
    if let Some(lit) = step.lit {
        game.native_set_block_property_with(pos, "lit", if lit { "true" } else { "false" }, flags::UPDATE_ALL);
    }
    if int(&entity.nbt, "remainingLitTime") != b.remaining_lit_time {
        entity.nbt.insert(key("remainingLitTime"), Tag::Int(b.remaining_lit_time));
    }
}

/// `PyromanticBrazierBlock.useItemOn`: an empty hand (or a non-fuel left in the brazier, an empty bucket) takes the
/// slot out; fuel goes in.
fn click_brazier(game: &mut Game<'_>, pos: [i32; 3]) -> bool {
    let inside = level_slot(game, pos, 0);
    let hand = game.native_used_stack().clone().filter(|s| s.count > 0);
    let non_fuel = inside.as_ref().is_some_and(|s| game.native_burn_time(s) == 0);
    if hand.is_none() || non_fuel {
        if let Some(stack) = inside {
            set_level_slot(game, pos, 0, None, 1);
            give(game, pos, stack);
            return true;
        }
        return false;
    }
    let Some(hand) = hand else { return false };
    let valid = game.native_burn_time(&hand) > 0;
    let taken = insertable(game, inside.as_ref(), &hand, valid);
    if taken == 0 {
        return false;
    }
    let mut one = hand.clone();
    one.count = taken;
    set_level_slot(game, pos, 0, Some(merged(inside, &one)), 1);
    *game.native_used_stack() = shrink(Some(hand), taken);
    true
}

// ---- The calcination oven, the distiller and the liquefaction cauldron ------------------------------------------

/// The recipe a one-input apparatus would run (the full check: item, count, solvent).
fn full_recipe(game: &Game<'_>, book: &Book, kind: Kind, input: &ItemStack, tank: &Tank, last: Option<usize>) -> Option<usize> {
    let tags = |i: &str, t: &str| item_in_tag(game, i, t);
    match kind {
        Kind::Oven => recipe::find(&book.calcination, last, |r| r.matches(&input.item, input.count, &tags)),
        Kind::Distiller => recipe::find(&book.distillation, last, |r| r.matches(&input.item, input.count, &tags)),
        Kind::Cauldron => recipe::find(&book.liquefaction, last, |r| r.matches(&input.item, tank.fluid_id(), tank.amount, &tags, &fluid_in_tag)),
        _ => None,
    }
}

/// `isIngredient` (the `CachedCheck`'s item-only test).
fn takes(game: &Game<'_>, book: &Book, kind: Kind, item: &str) -> bool {
    let tags = |i: &str, t: &str| item_in_tag(game, i, t);
    match kind {
        Kind::Oven => book.calcination.iter().any(|r| r.takes(item, &tags)),
        Kind::Distiller => book.distillation.iter().any(|r| r.takes(item, &tags)),
        Kind::Cauldron => book.liquefaction.iter().any(|r| r.takes(item, &tags)),
        Kind::Accumulator => book.accumulation.iter().any(|r| r.takes(item, &tags)),
        _ => false,
    }
}

/// (time, result, input used, solvent used) of recipe `i`.
fn single_recipe(game: &Game<'_>, book: &Book, kind: Kind, i: usize) -> Option<(i32, Option<ItemStack>, u32, i32)> {
    match kind {
        Kind::Oven => book.calcination.get(i).map(|r| (r.time, game.native_recipe_result(&r.result.json), r.ingredient.count, 0)),
        Kind::Distiller => book.distillation.get(i).map(|r| (r.time, game.native_recipe_result(&r.result.json), r.ingredient.count, 0)),
        Kind::Cauldron => book.liquefaction.get(i).map(|r| (r.time, game.native_recipe_result(&r.result.json), 1, r.solvent.amount)),
        _ => None,
    }
}

/// `getTotalTime`: the time of the recipe that fits the input now, else 100.
fn total_time_single(game: &Game<'_>, book: &Book, kind: Kind, input: Option<&ItemStack>, tank: &Tank, last: Option<usize>) -> i32 {
    let found = input.and_then(|s| full_recipe(game, book, kind, s, tank, last));
    crafting::total_time(found.and_then(|i| single_recipe(game, book, kind, i)).map(|r| r.0))
}

/// `CalcinationOvenBlockEntity.tickServer` (and the distiller's and the cauldron's, which differ only in their
/// recipes and the cauldron's solvent).
fn tick_single(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, kind: Kind) {
    let dimension = game.native_dimension();
    let book = book(game);
    let mut l = live(dimension, pos);
    let mut input = slot(game, entity, INPUT);
    let mut output = slot(game, entity, OUTPUT);
    let tank_spec = kind.tank();
    let mut tank = tank_spec.map_or_else(Tank::default, |(name, cap)| read_tank(&entity.nbt, name, cap));
    let mut c = read_crafting(&entity.nbt, &l);
    if c.progress > 0 && c.total_time == 0 {
        c = Crafting::loaded(c.progress, c.total_time, || total_time_single(game, &book, kind, input.as_ref(), &tank, l.last));
    }
    let now = input_key(input.as_ref());
    if l.seen && input_changed(&l.input, &now) {
        c.input_changed(total_time_single(game, &book, kind, input.as_ref(), &tank, l.last));
    }
    l.seen = true;
    let is_heated = heated(game, pos, &mut l, long(&entity.nbt, "heatReceiver"));
    let has_input = input.is_some();
    let found = input.as_ref().and_then(|s| full_recipe(game, &book, kind, s, &tank, l.last));
    if found.is_some() {
        l.last = found;
    }
    let details = found.and_then(|i| single_recipe(game, &book, kind, i));
    let can_craft = || details.as_ref().and_then(|d| d.1.as_ref()).is_some_and(|r| output_fits(game, output.as_ref(), r));
    let step = c.tick(is_heated, has_input, can_craft, || total_time_single(game, &book, kind, input.as_ref(), &tank, l.last));
    if step == Step::Craft
        && let Some((_, Some(result), used, solvent)) = details
    {
        output = Some(merged(output, &result));
        input = shrink(input, used);
        if solvent > 0 {
            tank.drain(solvent, false);
        }
        set_slot(entity, OUTPUT, output, 2);
        set_slot(entity, INPUT, input.clone(), 2);
        if input.is_none() {
            // `extractItem` emptied the input: `onContentTypeChanged`.
            c.input_changed(crafting::total_time(None));
        }
        if let Some((name, _)) = tank_spec {
            write_tank(&mut entity.nbt, name, &tank);
        }
    }
    l.input = input_key(input.as_ref());
    write_crafting(&mut entity.nbt, &c, &mut l);
    set_live(dimension, pos, l);
}

/// `TwoSlotItemHandlerBehaviour.useItemOn` (main hand): an empty hand takes the output, else the input; a held stack
/// goes into the input as far as the apparatus can process it (`canProcess`).
fn click_two_slot(game: &mut Game<'_>, pos: [i32; 3], kind: Kind) -> bool {
    let hand = game.native_used_stack().clone().filter(|s| s.count > 0);
    let input = level_slot(game, pos, INPUT);
    match hand {
        None => {
            for i in [OUTPUT, INPUT] {
                if let Some(stack) = level_slot(game, pos, i) {
                    set_level_slot(game, pos, i, None, 2);
                    give(game, pos, stack);
                    if i == INPUT {
                        reset_crafting(game, pos, kind, None);
                    }
                    return true;
                }
            }
            false
        }
        Some(hand) => {
            let book = book(game);
            let valid = input.as_ref().is_some_and(|s| s.same_item_same_components(&hand)) || takes(game, &book, kind, &hand.item);
            let taken = insertable(game, input.as_ref(), &hand, valid);
            if taken == 0 {
                return false;
            }
            let mut moved = hand.clone();
            moved.count = taken;
            let now = merged(input, &moved);
            set_level_slot(game, pos, INPUT, Some(now.clone()), 2);
            *game.native_used_stack() = shrink(Some(hand), taken);
            reset_crafting(game, pos, kind, Some(&now));
            true
        }
    }
}

/// `onInputItemChanged` for a click: the total time of what fits now, progress 0. The tick that follows sees the input
/// it already heard.
fn reset_crafting(game: &mut Game<'_>, pos: [i32; 3], kind: Kind, input: Option<&ItemStack>) {
    let dimension = game.native_dimension();
    let book = book(game);
    let mut l = live(dimension, pos);
    let tank = kind
        .tank()
        .and_then(|(name, cap)| game.native_with_mod_block_entity(pos, |e| read_tank(&e.nbt, name, cap)))
        .unwrap_or_default();
    let total = total_time_single(game, &book, kind, input, &tank, l.last);
    game.native_with_mod_block_entity(pos, |e| {
        e.nbt.insert(key("progress"), Tag::Short(0));
        e.nbt.insert(key("totalTime"), Tag::Short(total as i16));
    });
    l.input = input_key(input);
    l.seen = true;
    set_live(dimension, pos, l);
}

// ---- The incubator ---------------------------------------------------------------------------------------------

const HORIZONTAL: [(&str, [i32; 3]); 4] = [("north", [0, 0, -1]), ("east", [1, 0, 0]), ("south", [0, 0, 1]), ("west", [-1, 0, 0])];

fn offset(pos: [i32; 3], d: [i32; 3]) -> [i32; 3] {
    [pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]]
}

/// `validateMultiblock`: the mercury, salt and sulfur vessels among the four sides (the last of each found wins).
fn vessels(game: &Game<'_>, pos: [i32; 3]) -> Option<[[i32; 3]; 3]> {
    let mut found: [Option<[i32; 3]>; 3] = [None; 3];
    for (_, d) in HORIZONTAL {
        let at = offset(pos, d);
        match Kind::of(&game.native_block(at)) {
            Some(Kind::Mercury) => found[0] = Some(at),
            Some(Kind::Salt) => found[1] = Some(at),
            Some(Kind::Sulfur) => found[2] = Some(at),
            _ => {}
        }
    }
    Some([found[0]?, found[1]?, found[2]?])
}

fn incubation_found(game: &Game<'_>, book: &Book, inputs: &[Option<ItemStack>; 3], last: Option<usize>) -> Option<usize> {
    let [Some(m), Some(s), Some(u)] = inputs else { return None };
    let tags = |i: &str, t: &str| item_in_tag(game, i, t);
    recipe::find(&book.incubation, last, |r| r.matches(&m.item, &s.item, &u.item, &tags))
}

/// `IncubatorBlockEntity.tickServer`: the vessels beside it hold the inputs, heat from below, one of each taken per
/// craft.
fn tick_incubator(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let dimension = game.native_dimension();
    let book = book(game);
    let mut l = live(dimension, pos);
    // `updateShape`: the connection to each side's vessel.
    for (side, d) in HORIZONTAL {
        let vessel = Kind::of(&game.native_block(offset(pos, d))).is_some_and(|k| k.vessel_tag().is_some());
        let want = if vessel { "true" } else { "false" };
        if game.native_block_property(pos, side).is_some_and(|v| v != want) {
            game.native_set_block_property_with(pos, side, want, flags::UPDATE_ALL);
        }
    }
    let found_vessels = vessels(game, pos);
    let inputs: [Option<ItemStack>; 3] = match found_vessels {
        Some(v) => [level_slot(game, v[0], 0), level_slot(game, v[1], 0), level_slot(game, v[2], 0)],
        None => [None, None, None],
    };
    let mut c = read_crafting(&entity.nbt, &l);
    let total = |game: &Game<'_>, inputs: &[Option<ItemStack>; 3], last| {
        crafting::total_time(incubation_found(game, &book, inputs, last).and_then(|i| book.incubation.get(i)).map(|r| r.time))
    };
    if c.progress > 0 && c.total_time == 0 {
        c.total_time = total(game, &inputs, l.last);
    }
    // `onInputItemChanged`: other vessels, or a vessel's content changed.
    let keys: [Option<(String, u32)>; 3] = std::array::from_fn(|i| input_key(inputs[i].as_ref()));
    if l.seen && (l.vessels != found_vessels || keys.iter().zip(&l.vessel_inputs).any(|(now, before)| input_changed(before, now))) {
        c.input_changed(total(game, &inputs, l.last));
    }
    l.seen = true;
    l.vessels = found_vessels;
    let is_heated = heated(game, pos, &mut l, long(&entity.nbt, "heatReceiver"));
    let has_input = found_vessels.is_some() && inputs.iter().all(Option::is_some);
    let found = incubation_found(game, &book, &inputs, l.last);
    if found.is_some() {
        l.last = found;
    }
    let result = found.and_then(|i| book.incubation.get(i)).and_then(|r| result_stack(game, &r.result));
    let output = slot(game, entity, 0);
    let can_craft = || result.as_ref().is_some_and(|r| output_fits(game, output.as_ref(), r));
    let step = c.tick(is_heated, has_input, can_craft, || total(game, &inputs, l.last));
    let mut after = inputs.clone();
    if step == Step::Craft
        && let (Some(result), Some(v)) = (result, found_vessels)
    {
        set_slot(entity, 0, Some(merged(output, &result)), 1);
        for (i, at) in v.iter().enumerate() {
            after[i] = shrink(inputs[i].clone(), 1);
            set_level_slot(game, *at, 0, after[i].clone(), 1);
            game.native_block_entity_sync(*at);
        }
        if after.iter().any(Option::is_none) {
            c.input_changed(total(game, &after, l.last));
        }
    }
    l.vessel_inputs = std::array::from_fn(|i| input_key(after[i].as_ref()));
    write_crafting(&mut entity.nbt, &c, &mut l);
    set_live(dimension, pos, l);
}

/// `OneSlotItemHandlerBehaviour.useItemOn` on a vessel (its tag), the accumulator (`canProcess`) or the incubator
/// (its output gives only): an empty hand takes the slot out, a held stack goes in.
fn click_one_slot(game: &mut Game<'_>, pos: [i32; 3], kind: Kind) -> bool {
    let hand = game.native_used_stack().clone().filter(|s| s.count > 0);
    let inside = level_slot(game, pos, 0);
    match hand {
        None => {
            let Some(stack) = inside else { return false };
            set_level_slot(game, pos, 0, None, 1);
            give(game, pos, stack);
            true
        }
        Some(hand) => {
            let valid = match kind {
                Kind::Incubator => false,
                Kind::Accumulator => {
                    let book = book(game);
                    inside.as_ref().is_some_and(|s| s.same_item_same_components(&hand)) || takes(game, &book, kind, &hand.item)
                }
                // The pedestals' `InputInventory.isItemValid`; the result pedestal's wrapper refuses inserts.
                Kind::SourcePedestal | Kind::TargetPedestal => item_in_tag(game, &hand.item, "theurgy:alchemical_sulfurs_and_niters"),
                Kind::ResultPedestal => false,
                // `canProcess`: an item a catalysation recipe takes.
                Kind::Catalyst => {
                    let book = book(game);
                    let tags = |i: &str, t: &str| item_in_tag(game, i, t);
                    book.catalysation.iter().any(|r| r.takes(&hand.item, &tags))
                }
                _ => kind.vessel_tag().is_some_and(|tag| item_in_tag(game, &hand.item, tag)),
            };
            let taken = insertable(game, inside.as_ref(), &hand, valid);
            if taken == 0 {
                return false;
            }
            let mut moved = hand.clone();
            moved.count = taken;
            set_level_slot(game, pos, 0, Some(merged(inside, &moved)), 1);
            *game.native_used_stack() = shrink(Some(hand), taken);
            true
        }
    }
}

// ---- The sal ammoniac accumulator ------------------------------------------------------------------------------

/// The water tank's validator (`craftingBehaviour::canProcess` for fluids): the fluid it holds, or one an
/// accumulation recipe evaporates.
fn accumulator_takes_fluid(book: &Book, tank: &Tank, fluid: &str) -> bool {
    tank.fluid_id() == fluid || book.accumulation.iter().any(|r| r.takes_fluid(fluid, &fluid_in_tag))
}

fn accumulation_found(game: &Game<'_>, book: &Book, solute: Option<&ItemStack>, tank: &Tank, last: Option<usize>) -> Option<usize> {
    let tags = |i: &str, t: &str| item_in_tag(game, i, t);
    let item = solute.map_or("", |s| &*s.item);
    recipe::find(&book.accumulation, last, |r| r.matches(item, tank.fluid_id(), tank.amount, &tags, &fluid_in_tag))
}

/// `SalAmmoniacAccumulatorBlockEntity.tickServer`: with a fluid handler below (the sal ammoniac tank, or another
/// mod's), water and an optional solute make sal ammoniac into it, no heat needed.
fn tick_accumulator(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let dimension = game.native_dimension();
    let under = below(pos);
    if game.native_fluid_handler(dimension, under, Some(Direction::Up)).is_none() {
        return;
    }
    let book = book(game);
    let mut l = live(dimension, pos);
    let mut water = read_tank(&entity.nbt, "waterTank", 10 * BUCKET);
    let mut solute = slot(game, entity, 0);
    let mut c = read_crafting(&entity.nbt, &l);
    let total = |game: &Game<'_>, solute: Option<&ItemStack>, water: &Tank, last| {
        crafting::total_time(accumulation_found(game, &book, solute, water, last).and_then(|i| book.accumulation.get(i)).map(|r| r.time))
    };
    if c.progress > 0 && c.total_time == 0 {
        c.total_time = total(game, solute.as_ref(), &water, l.last);
    }
    let now = input_key(solute.as_ref());
    if l.seen && input_changed(&l.input, &now) {
        c.input_changed(total(game, solute.as_ref(), &water, l.last));
    }
    l.seen = true;
    let has_input = !water.is_empty() || solute.is_some();
    let found = if has_input { accumulation_found(game, &book, solute.as_ref(), &water, l.last) } else { None };
    if found.is_some() {
        l.last = found;
    }
    let recipe = found.and_then(|i| book.accumulation.get(i)).cloned();
    let result = recipe.as_ref().and_then(|r| FluidStack::of(&r.result.id, r.result.amount));
    // `canCraft`: the fluid handler below takes the whole result (a simulated fill; asked only with input).
    let could = result.as_ref().is_some_and(|f| game.native_fluid_fill(dimension, under, Some(Direction::Up), f, true) == f.amount());
    let step = c.tick(true, has_input, || could, || total(game, solute.as_ref(), &water, l.last));
    if step == Step::Craft
        && let (Some(r), Some(f)) = (recipe, result)
    {
        game.native_fluid_fill(dimension, under, Some(Direction::Up), &f, false);
        if r.solute.is_some() {
            solute = shrink(solute, 1);
            set_slot(entity, 0, solute.clone(), 1);
            if solute.is_none() {
                c.input_changed(total(game, None, &water, l.last));
            }
        }
        if let Some(e) = &r.evaporant {
            water.drain(e.amount, false);
            write_tank(&mut entity.nbt, "waterTank", &water);
        }
    }
    l.input = input_key(solute.as_ref());
    write_crafting(&mut entity.nbt, &c, &mut l);
    set_live(dimension, pos, l);
}

// ---- The fermentation and digestion vats -----------------------------------------------------------------------

const OUT_SLOT: usize = vat::INPUT_SLOTS;

fn vat_list(book: &Book, kind: Kind) -> &[VatRecipe] {
    if kind == Kind::Fermentation { &book.fermentation } else { &book.digestion }
}

/// The tank's validator (`canProcess(FluidStack)`): the fluid it holds, or one a recipe of the vat takes.
fn vat_takes_fluid(book: &Book, kind: Kind, tank: &Tank, fluid: &str) -> bool {
    vat::can_process_fluid(vat_list(book, kind), tank.fluid_id(), fluid, &fluid_in_tag)
}

type Owned = Vec<Option<(String, u32)>>;

fn vat_owned(inputs: &[Option<ItemStack>]) -> Owned {
    inputs.iter().map(|s| s.as_ref().map(|s| (s.item.to_string(), s.count))).collect()
}

fn vat_view(owned: &Owned) -> Vec<vat::Stack<'_>> {
    owned.iter().map(|o| o.as_ref().map(|(i, c)| (i.as_str(), *c))).collect()
}

/// `getRecipe`: the first recipe (the last found if it still fits) the slots and the tank satisfy.
fn vat_found(game: &Game<'_>, book: &Book, kind: Kind, owned: &Owned, tank: &Tank, last: Option<usize>) -> Option<usize> {
    let tags = |i: &str, t: &str| item_in_tag(game, i, t);
    let view = vat_view(owned);
    recipe::find(vat_list(book, kind), last, |r| r.matches(&view, tank.fluid_id(), tank.amount, &tags, &fluid_in_tag))
}

/// `getTotalTime`: the fitting recipe's time, else 200.
fn vat_total(game: &Game<'_>, book: &Book, kind: Kind, owned: &Owned, tank: &Tank, last: Option<usize>) -> i32 {
    vat_found(game, book, kind, owned, tank, last).and_then(|i| vat_list(book, kind).get(i)).map_or(vat::DEFAULT_TIME, |r| r.time)
}

/// `canCraft`: the result fits the output slot whole.
fn vat_can_craft(game: &Game<'_>, recipe: &VatRecipe, output: Option<&ItemStack>) -> bool {
    game.native_recipe_result(&recipe.result.json).is_some_and(|r| output_fits(game, output, &r))
}

/// `FermentationVatBlockEntity.tickServer` (and the digestion vat's): closed, it crafts the recipe the slots and the
/// tank make; when nothing is processing any more the lid opens. (No redstone: neither the auto close nor the
/// signals.)
fn tick_vat(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, kind: Kind) {
    let dimension = game.native_dimension();
    let book = book(game);
    let mut l = live(dimension, pos);
    let open = game.native_block_property(pos, "open").as_deref() != Some("false");
    let mut inputs: Vec<Option<ItemStack>> = (0..vat::INPUT_SLOTS).map(|i| slot(game, entity, i)).collect();
    let mut output = slot(game, entity, OUT_SLOT);
    let mut tank = read_tank(&entity.nbt, "fluidTank", 10 * BUCKET);
    let mut c = read_crafting(&entity.nbt, &l);
    let mut owned = vat_owned(&inputs);
    if c.progress > 0 && c.total_time == 0 {
        c.total_time = vat_total(game, &book, kind, &owned, &tank, l.last);
    }
    // `onInputItemChanged`: a slot's item (not its count) changed.
    let keys: [Option<(String, u32)>; 3] = std::array::from_fn(|i| owned[i].as_ref().map(|(item, _)| (item.clone(), 0)));
    if l.seen && keys != l.vessel_inputs {
        c.input_changed(vat_total(game, &book, kind, &owned, &tank, l.last));
    }
    l.seen = true;
    let has_input = inputs.iter().any(Option::is_some);
    let found = if has_input { vat_found(game, &book, kind, &owned, &tank, l.last) } else { None };
    if found.is_some() {
        l.last = found;
    }
    let recipe = found.and_then(|i| vat_list(&book, kind).get(i)).cloned();
    let can_craft = || recipe.as_ref().is_some_and(|r| vat_can_craft(game, r, output.as_ref()));
    let step = c.tick(!open, has_input, can_craft, || vat_total(game, &book, kind, &owned, &tank, l.last));
    if step == Step::Craft
        && let Some(r) = &recipe
        && let Some(result) = game.native_recipe_result(&r.result.json)
    {
        let tags = |i: &str, t: &str| item_in_tag(game, i, t);
        let taken = r.consumption(&vat_view(&owned), &tags);
        output = Some(merged(output, &result));
        let mut emptied = false;
        for (i, n) in taken {
            inputs[i] = shrink(inputs[i].take(), n);
            emptied |= inputs[i].is_none();
        }
        tank.drain(r.fluid.amount, false);
        for (i, s) in inputs.iter().enumerate() {
            set_slot(entity, i, s.clone(), vat::INPUT_SLOTS + 1);
        }
        set_slot(entity, OUT_SLOT, output.clone(), vat::INPUT_SLOTS + 1);
        write_tank(&mut entity.nbt, "fluidTank", &tank);
        owned = vat_owned(&inputs);
        if emptied {
            c.input_changed(vat_total(game, &book, kind, &owned, &tank, l.last));
        }
    }
    l.vessel_inputs = std::array::from_fn(|i| owned[i].as_ref().map(|(item, _)| (item.clone(), 0)));
    if !c.processing && !open {
        game.native_set_block_property_with(pos, "open", "true", flags::UPDATE_CLIENTS);
    }
    // The fermentation vat's visual `has_output`.
    if kind == Kind::Fermentation {
        let want = if output.is_some() { "true" } else { "false" };
        if game.native_block_property(pos, "has_output").is_some_and(|v| v != want) {
            game.native_set_block_property_with(pos, "has_output", want, flags::UPDATE_ALL);
        }
    }
    write_crafting(&mut entity.nbt, &c, &mut l);
    set_live(dimension, pos, l);
}

/// The slots and tank of the vat at `pos` in the level, and the recipe they make.
fn vat_state(game: &mut Game<'_>, pos: [i32; 3], kind: Kind) -> (Vec<Option<ItemStack>>, Option<ItemStack>, Tank, Option<usize>) {
    let inputs: Vec<Option<ItemStack>> = (0..vat::INPUT_SLOTS).map(|i| level_slot(game, pos, i)).collect();
    let output = level_slot(game, pos, OUT_SLOT);
    let tank = game.native_with_mod_block_entity(pos, |e| read_tank(&e.nbt, "fluidTank", 10 * BUCKET)).unwrap_or_default();
    let found = vat_found(game, &book(game), kind, &vat_owned(&inputs), &tank, live(game.native_dimension(), pos).last);
    (inputs, output, tank, found)
}

/// `GenericVatInteractionBehaviour.useItemOn` (main hand), then the tank's and the slots' behaviours. Sneaking with an
/// empty hand opens or closes the lid (closing needs a recipe that can craft); a closed vat refuses everything else.
fn click_vat(game: &mut Game<'_>, pos: [i32; 3], kind: Kind) -> bool {
    let hand = game.native_used_stack().clone().filter(|s| s.count > 0);
    let open = game.native_block_property(pos, "open").as_deref() != Some("false");
    if !(game.native_sneaking() && hand.is_none()) {
        if !open {
            return true;
        }
        return click_tank(game, pos, kind) || click_vat_slots(game, pos, kind, hand);
    }
    if open {
        let book = book(game);
        let (_, output, _, found) = vat_state(game, pos, kind);
        let can = found.and_then(|i| vat_list(&book, kind).get(i)).is_some_and(|r| vat_can_craft(game, r, output.as_ref()));
        if can {
            game.native_set_block_property_with(pos, "open", "false", flags::UPDATE_CLIENTS);
        }
    } else {
        game.native_set_block_property_with(pos, "open", "true", flags::UPDATE_CLIENTS);
        // `stopProcessing`.
        let dimension = game.native_dimension();
        let mut l = live(dimension, pos);
        l.processing = false;
        l.could_craft = false;
        set_live(dimension, pos, l);
        game.native_with_mod_block_entity(pos, |e| e.nbt.insert(key("progress"), Tag::Short(0)));
    }
    true
}

/// `DynamicOneOutputSlotItemHandlerBehaviour.useItemOn`: an empty hand takes the output, else the first input; a held
/// stack goes into the first input slot that takes any (`InputInventory.isItemValid`).
fn click_vat_slots(game: &mut Game<'_>, pos: [i32; 3], kind: Kind, hand: Option<ItemStack>) -> bool {
    let n = vat::INPUT_SLOTS + 1;
    let Some(hand) = hand else {
        for i in [OUT_SLOT, 0, 1, 2] {
            if let Some(stack) = level_slot(game, pos, i) {
                set_level_slot(game, pos, i, None, n);
                give(game, pos, stack);
                return true;
            }
        }
        return false;
    };
    let book = book(game);
    let (inputs, _, tank, found) = vat_state(game, pos, kind);
    let owned = vat_owned(&inputs);
    let tags = |i: &str, t: &str| item_in_tag(game, i, t);
    let view = vat_view(&owned);
    let can = vat::can_process_item(vat_list(&book, kind), found, &view, &hand.item, &tags);
    let _ = tank;
    for i in 0..vat::INPUT_SLOTS {
        let elsewhere = inputs.iter().enumerate().any(|(j, s)| j != i && s.as_ref().is_some_and(|s| s.same_item_same_components(&hand)));
        let taken = insertable(game, inputs[i].as_ref(), &hand, can && !elsewhere);
        if taken == 0 {
            continue;
        }
        let mut moved = hand.clone();
        moved.count = taken;
        set_level_slot(game, pos, i, Some(merged(inputs[i].clone(), &moved)), n);
        *game.native_used_stack() = shrink(Some(hand), taken);
        return true;
    }
    false
}

// ---- Fluids: buckets and fluid handlers ------------------------------------------------------------------------

/// Whether tank `name` of an apparatus takes `fluid` (its `FluidTank` validator).
fn tank_valid(game: &Game<'_>, kind: Kind, tank: &Tank, fluid: &str) -> bool {
    match kind {
        Kind::Cauldron => fluid_in_tag(fluid, SOLVENT_TAG),
        Kind::Accumulator => accumulator_takes_fluid(&book(game), tank, fluid),
        Kind::Fermentation | Kind::Digestion => vat_takes_fluid(&book(game), kind, tank, fluid),
        _ => true,
    }
}

/// `OneTankFluidHandlerBehaviour.useItemOn` (main hand): sneaking with an empty hand voids the tank; a full bucket
/// empties into it if it takes the whole bucket; an empty bucket fills from a bucket or more.
fn click_tank(game: &mut Game<'_>, pos: [i32; 3], kind: Kind) -> bool {
    let Some((name, capacity)) = kind.tank() else { return false };
    let hand = game.native_used_stack().clone().filter(|s| s.count > 0);
    let Some(mut tank) = game.native_with_mod_block_entity(pos, |e| read_tank(&e.nbt, name, capacity)) else { return false };
    let Some(hand) = hand else {
        if game.native_sneaking() {
            tank.drain(i32::MAX, false);
            game.native_with_mod_block_entity(pos, |e| write_tank(&mut e.nbt, name, &tank));
            return true;
        }
        return false;
    };
    if game.native_item_fluid(&hand).is_none() {
        return false;
    }
    // Item into the tank.
    if let Some((fluid, amount, emptied)) = game.native_item_fluid_drain(&hand, i32::MAX) {
        let valid = tank_valid(game, kind, &tank, &fluid);
        if tank.fill(&fluid, amount, valid, true) == amount {
            tank.fill(&fluid, amount, valid, false);
            game.native_with_mod_block_entity(pos, |e| write_tank(&mut e.nbt, name, &tank));
            hand_becomes(game, pos, hand, emptied);
            return true;
        }
    }
    // The tank into the item.
    if !tank.is_empty() {
        let fluid = tank.fluid_id().to_owned();
        if let Some((taken, filled)) = game.native_item_fluid_fill(&hand, &fluid, tank.amount)
            && taken > 0
        {
            tank.drain(taken, false);
            game.native_with_mod_block_entity(pos, |e| write_tank(&mut e.nbt, name, &tank));
            hand_becomes(game, pos, hand, filled);
            return true;
        }
    }
    false
}

/// `updateFluidContainerInHand`: one container of the held stack is used; a lone one becomes the new container, else
/// the new one goes to the inventory.
fn hand_becomes(game: &mut Game<'_>, pos: [i32; 3], hand: ItemStack, container: ItemStack) {
    if hand.count <= 1 {
        *game.native_used_stack() = Some(container);
    } else {
        *game.native_used_stack() = shrink(Some(hand), 1);
        give(game, pos, container);
    }
}

fn api_fluid(fluid: &str, amount: i32) -> Option<mod_api::FluidStack> {
    let key = mod_api::__private::fluid_key(fluid)?;
    Some(mod_api::FluidStack::new(key, amount.max(0).unsigned_abs()))
}

fn engine_fluid(stack: &mod_api::FluidStack) -> FluidStack {
    super::machines::native_fluid_handler::engine_fluid_stack(stack)
}

/// `native_fluid_handler_tanks`: the cauldron's solvent tank, the accumulator's water tank, the tank's own, on every
/// side.
pub(super) fn fluid_tanks(entity: &ModBlockEntity, tanks: &mut Option<Vec<NativeFluidTankInfo>>) {
    if tanks.is_some() {
        return;
    }
    let Some((name, capacity)) = Kind::of(entity.id()).and_then(Kind::tank) else { return };
    let tank = read_tank(&entity.nbt, name, capacity);
    let stack = if tank.is_empty() { None } else { api_fluid(tank.fluid_id(), tank.amount) };
    *tanks = Some(vec![NativeFluidTankInfo { drainable: stack.is_some(), stack, capacity }]);
}

/// `native_fluid_handler_fill` (`FluidTank.fill` through the validator).
pub(super) fn fluid_fill(entity: &mut ModBlockEntity, resource: &mod_api::FluidStack, simulate: bool, filled: &mut Option<i32>) {
    if filled.is_some() {
        return;
    }
    let Some(kind) = Kind::of(entity.id()) else { return };
    let Some((name, capacity)) = kind.tank() else { return };
    let offered = engine_fluid(resource);
    if offered.is_empty() {
        *filled = Some(0);
        return;
    }
    let mut tank = read_tank(&entity.nbt, name, capacity);
    let fluid = offered.name();
    let valid = match kind {
        Kind::Cauldron => fluid_in_tag(fluid, SOLVENT_TAG),
        // The recipe book of this thread, once an accumulator has ticked; before that any fluid.
        Kind::Accumulator => BOOK.with(|b| b.borrow().as_ref().is_none_or(|b| accumulator_takes_fluid(b, &tank, fluid))),
        Kind::Fermentation | Kind::Digestion => BOOK.with(|b| b.borrow().as_ref().is_none_or(|b| vat_takes_fluid(b, kind, &tank, fluid))),
        _ => true,
    };
    let taken = tank.fill(fluid, offered.amount(), valid, simulate);
    if !simulate && taken > 0 {
        write_tank(&mut entity.nbt, name, &tank);
    }
    *filled = Some(taken);
}

/// `native_fluid_handler_drain` (`FluidTank.drain`, of a fluid or of whatever it holds).
pub(super) fn fluid_drain(entity: &mut ModBlockEntity, resource: Option<&mod_api::FluidStack>, max: i32, simulate: bool, drained: &mut Option<mod_api::FluidStack>) {
    if drained.is_some() {
        return;
    }
    let Some((name, capacity)) = Kind::of(entity.id()).and_then(Kind::tank) else { return };
    let mut tank = read_tank(&entity.nbt, name, capacity);
    let out = match resource.map(engine_fluid) {
        Some(r) if r.is_empty() => None,
        Some(r) => tank.drain_fluid(r.name(), r.amount(), simulate),
        None => tank.drain(max, simulate),
    };
    if !simulate && out.is_some() {
        write_tank(&mut entity.nbt, name, &tank);
    }
    *drained = out.and_then(|(fluid, amount)| api_fluid(&fluid, amount));
}

// ---- The mercurial reformation array and the mercury catalyst ---------------------------------------------------

const EMITTER_ITEM: &str = "theurgy:sulfuric_flux_emitter";

/// `SulfuricFluxEmitterSelectedPoint.CODEC`: `{blockPos: [x, y, z], type}`.
fn point_tag(pos: [i32; 3], kind: &str) -> Tag {
    let mut c = Compound::new();
    c.insert(key("blockPos"), Tag::IntArray(pos.to_vec()));
    c.insert(key("type"), Tag::String(JavaString::from(kind)));
    Tag::Compound(c)
}

fn point_pos(c: &Compound) -> Option<[i32; 3]> {
    match c.get(&key("blockPos")) {
        Some(Tag::IntArray(v)) if v.len() == 3 => Some([v[0], v[1], v[2]]),
        _ => None,
    }
}

fn read_pos(nbt: &Compound, name: &str) -> Option<[i32; 3]> {
    match nbt.get(&key(name)) {
        Some(Tag::Compound(c)) => point_pos(c),
        _ => None,
    }
}

fn read_sources(nbt: &Compound) -> Vec<[i32; 3]> {
    match nbt.get(&key("sourcePedestals")) {
        Some(Tag::List(list)) => list.iter().filter_map(|t| if let Tag::Compound(c) = t { point_pos(c) } else { None }).collect(),
        _ => Vec::new(),
    }
}

/// The emitter's selected pedestals that are still the right pedestals (`validateMultiblock`).
#[derive(Default)]
struct Array {
    sources: Vec<[i32; 3]>,
    target: Option<[i32; 3]>,
    result: Option<[i32; 3]>,
}

impl Array {
    fn valid(&self) -> bool {
        self.target.is_some() && self.result.is_some() && !self.sources.is_empty()
    }
}

fn array_of(game: &Game<'_>, nbt: &Compound) -> Array {
    let is = |p: [i32; 3], k: Kind| game.native_block(p) == k.block();
    Array {
        sources: read_sources(nbt).into_iter().filter(|p| is(*p, Kind::SourcePedestal)).collect(),
        target: read_pos(nbt, "targetPedestal").filter(|p| is(*p, Kind::TargetPedestal)),
        result: read_pos(nbt, "resultPedestal").filter(|p| is(*p, Kind::ResultPedestal)),
    }
}

/// The target pedestal's stack and each source pedestal's.
fn array_stacks(game: &mut Game<'_>, a: &Array) -> (Option<ItemStack>, Vec<Option<ItemStack>>) {
    let target = a.target.and_then(|p| level_slot(game, p, 0));
    let sources = a.sources.iter().map(|p| level_slot(game, *p, 0)).collect();
    (target, sources)
}

fn array_found(game: &Game<'_>, book: &Book, target: Option<&ItemStack>, sources: &[Option<ItemStack>], flux: i32, last: Option<usize>) -> Option<usize> {
    let tags = |i: &str, t: &str| item_in_tag(game, i, t);
    let view: Vec<reformation::Stack<'_>> = sources.iter().map(|s| s.as_ref().map(|s| (&*s.item, s.count))).collect();
    let target = target.map(|s| &*s.item);
    recipe::find(&book.reformation, last, |r| r.matches(target, &view, flux, &tags))
}

/// `getTotalTime`: the fitting recipe's time, else 100.
fn array_total(game: &Game<'_>, book: &Book, target: Option<&ItemStack>, sources: &[Option<ItemStack>], flux: i32, last: Option<usize>) -> i32 {
    crafting::total_time(array_found(game, book, target, sources, flux, last).and_then(|i| book.reformation.get(i)).map(|r| r.time))
}

fn array_keys(target: Option<&ItemStack>, sources: &[Option<ItemStack>]) -> Vec<Option<String>> {
    std::iter::once(target).chain(sources.iter().map(Option::as_ref)).map(|s| s.map(|s| s.item.to_string())).collect()
}

/// `SulfuricFluxEmitterBlockEntity.tickServer`: with a valid array, items on a source pedestal and the target, and
/// enough flux for a recipe, it crafts every `time` ticks: the sources are used up, the flux spent, the result goes to
/// the result pedestal (the target stays). (The array is checked every tick, where Java checks it when a pedestal
/// changes or the emitter is clicked.)
fn tick_emitter(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let dimension = game.native_dimension();
    let book = book(game);
    let mut l = live(dimension, pos);
    let a = array_of(game, &entity.nbt);
    let mut flux = int(&entity.nbt, "mercuryFluxStorage");
    let mut c = read_crafting(&entity.nbt, &l);
    let (target, mut sources) = array_stacks(game, &a);
    let has_input = a.valid() && target.is_some() && sources.iter().any(Option::is_some);
    if c.progress > 0 && c.total_time == 0 {
        c.total_time = array_total(game, &book, target.as_ref(), &sources, flux, l.last);
    }
    let mut keys = array_keys(target.as_ref(), &sources);
    if l.seen && keys != l.array {
        c.input_changed(array_total(game, &book, target.as_ref(), &sources, flux, l.last));
    }
    l.seen = true;
    let found = if has_input { array_found(game, &book, target.as_ref(), &sources, flux, l.last) } else { None };
    if found.is_some() {
        l.last = found;
    }
    let recipe = found.and_then(|i| book.reformation.get(i)).cloned();
    let result = recipe.as_ref().and_then(|r| game.native_recipe_result(&r.result.json));
    let output = a.result.and_then(|p| level_slot(game, p, 0));
    let can_craft = || result.as_ref().is_some_and(|r| output_fits(game, output.as_ref(), r));
    let step = c.tick(true, has_input, can_craft, || array_total(game, &book, target.as_ref(), &sources, flux, l.last));
    if step == Step::Craft
        && let (Some(r), Some(result), Some(out)) = (recipe, result, a.result)
    {
        let taken = {
            let tags = |i: &str, t: &str| item_in_tag(game, i, t);
            let view: Vec<reformation::Stack<'_>> = sources.iter().map(|s| s.as_ref().map(|s| (&*s.item, s.count))).collect();
            r.consumption(&view, &tags)
        };
        flux -= reformation::extract(flux, reformation::EMITTER_CAPACITY, r.mercury_flux);
        for (i, n) in taken {
            sources[i] = shrink(sources[i].take(), n);
            set_level_slot(game, a.sources[i], 0, sources[i].clone(), 1);
            game.native_block_entity_sync(a.sources[i]);
        }
        set_level_slot(game, out, 0, Some(merged(output, &result)), 1);
        game.native_block_entity_sync(out);
        entity.nbt.insert(key("mercuryFluxStorage"), Tag::Int(flux));
        let now = array_keys(target.as_ref(), &sources);
        if now != keys {
            // `onContentTypeChanged`: a source pedestal emptied.
            c.input_changed(array_total(game, &book, target.as_ref(), &sources, flux, l.last));
            keys = now;
        }
    }
    l.array = keys;
    write_crafting(&mut entity.nbt, &c, &mut l);
    set_live(dimension, pos, l);
}

const SIDES: [[i32; 3]; 6] = [[0, -1, 0], [0, 1, 0], [0, 0, -1], [1, 0, 0], [0, 0, 1], [-1, 0, 0]];

/// `MercuryCatalystBlockEntity.tickServer`: an item a catalysation recipe takes is consumed and its flux converted
/// into the storage a few per tick; every 20 ticks it offers 40 to each neighbour that takes mercury flux (here the
/// sulfuric flux emitter; no pipes). Java's `enabled` state (redstone) is not kept: it is always on.
fn tick_catalyst(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let book = book(game);
    let held = slot(game, entity, 0);
    let mut k = Catalyst {
        stored: int(&entity.nbt, "mercuryFluxStorage"),
        to_convert: int(&entity.nbt, "mercuryFluxToConvert"),
        per_tick: int(&entity.nbt, "currentMercuryFluxPerTick"),
    };
    let before = k;
    let consumed = {
        let tags = |i: &str, t: &str| item_in_tag(game, i, t);
        let item = held.as_ref().map(|s| s.item.to_string());
        k.tick(held.is_some(), || {
            let item = item?;
            book.catalysation.iter().find(|r| r.takes(&item, &tags)).map(|r| (r.total_flux, r.flux_per_tick))
        })
    };
    if consumed {
        set_slot(entity, 0, shrink(held, 1), 1);
    }
    if game_time(game) % reformation::PUSH_INTERVAL == 0 {
        for d in SIDES {
            if k.stored <= 0 {
                break;
            }
            let at = offset(pos, d);
            if Kind::of(&game.native_block(at)) != Some(Kind::Emitter) {
                continue;
            }
            let offered = reformation::extract(k.stored, reformation::CATALYST_CAPACITY, reformation::PUSH_AMOUNT);
            let received = game
                .native_with_mod_block_entity(at, |e| {
                    let have = int(&e.nbt, "mercuryFluxStorage");
                    let got = reformation::receive(have, reformation::EMITTER_CAPACITY, offered);
                    if got > 0 {
                        e.nbt.insert(key("mercuryFluxStorage"), Tag::Int(have + got));
                    }
                    got
                })
                .unwrap_or(0);
            k.stored -= reformation::extract(k.stored, reformation::CATALYST_CAPACITY, received);
        }
    }
    if k != before {
        entity.nbt.insert(key("mercuryFluxStorage"), Tag::Int(k.stored));
        entity.nbt.insert(key("mercuryFluxToConvert"), Tag::Int(k.to_convert));
        entity.nbt.insert(key("currentMercuryFluxPerTick"), Tag::Int(k.per_tick));
    }
}

fn acting_uuid(game: &Game<'_>) -> Option<u128> {
    game.native_acting_player().map(|p| p.uuid())
}

fn tell(game: &mut Game<'_>, uuid: Option<u128>, line: &str) {
    if let Some(uuid) = uuid {
        game.native_action_bar(uuid, Text::literal(line));
    }
}

fn summary(sources: usize, target: bool, result: bool) -> String {
    if sources == 0 && !target && !result {
        return "Sulfuric flux emitter: nothing selected".to_owned();
    }
    let mut missing = Vec::new();
    if !target {
        missing.push("no target pedestal");
    }
    if sources == 0 {
        missing.push("no source pedestals");
    }
    if !result {
        missing.push("no result pedestal");
    }
    if missing.is_empty() {
        format!("Sulfuric flux emitter: {sources} source, 1 target and 1 result pedestal")
    } else {
        format!("Sulfuric flux emitter: {sources} source pedestal(s); {}", missing.join(", "))
    }
}

/// The emitter in hand, right click on a pedestal (`SelectionBehaviour.onRightClickBlock`): it is picked (one target
/// and one result at most, any number of sources); picking it again takes it back (`cycleMode` has no mode).
fn select(game: &mut Game<'_>, pos: [i32; 3], kind: Kind) -> bool {
    let Some(uuid) = acting_uuid(game) else { return false };
    let now = SELECTION.with(|s| {
        let mut s = s.borrow_mut();
        let list = s.entry(uuid).or_default();
        if let Some(i) = list.iter().position(|(p, _)| *p == pos) {
            list.remove(i);
        } else {
            if kind != Kind::SourcePedestal {
                list.retain(|(_, k)| *k != kind);
            }
            list.push((pos, kind));
        }
        list.clone()
    });
    let line = summary(
        now.iter().filter(|(_, k)| *k == Kind::SourcePedestal).count(),
        now.iter().any(|(_, k)| *k == Kind::TargetPedestal),
        now.iter().any(|(_, k)| *k == Kind::ResultPedestal),
    );
    tell(game, Some(uuid), &line);
    true
}

/// `SulfuricFluxEmitterBlockItem.updateCustomBlockEntityTag` and `onPlace`: the placed emitter takes the player's picks
/// that are within 16 blocks; the picks are used up.
fn place_emitter(game: &mut Game<'_>, pos: [i32; 3], placer: Option<u128>) {
    let Some(uuid) = placer else { return };
    let picked = SELECTION.with(|s| s.borrow_mut().remove(&uuid)).unwrap_or_default();
    let range = reformation::SELECTION_RANGE;
    let near = |p: [i32; 3]| (0..3).map(|i| (p[i] - pos[i]).pow(2)).sum::<i32>() < range * range;
    let kept: Vec<([i32; 3], Kind)> = picked.iter().copied().filter(|(p, _)| near(*p)).collect();
    let sources: Vec<[i32; 3]> = kept.iter().filter(|(_, k)| *k == Kind::SourcePedestal).map(|(p, _)| *p).collect();
    let target = kept.iter().rev().find(|(_, k)| *k == Kind::TargetPedestal).map(|(p, _)| *p);
    let result = kept.iter().rev().find(|(_, k)| *k == Kind::ResultPedestal).map(|(p, _)| *p);
    game.native_with_mod_block_entity(pos, |e| {
        e.nbt.insert(key("sourcePedestals"), Tag::List(sources.iter().map(|p| point_tag(*p, "SOURCE")).collect()));
        match target {
            Some(p) => e.nbt.insert(key("targetPedestal"), point_tag(p, "TARGET")),
            None => e.nbt.remove(&key("targetPedestal")),
        };
        match result {
            Some(p) => e.nbt.insert(key("resultPedestal"), point_tag(p, "RESULT")),
            None => e.nbt.remove(&key("resultPedestal")),
        };
    });
    let line = if kept.len() < picked.len() {
        format!("{} selected pedestal(s) are out of range and were dropped", picked.len() - kept.len())
    } else {
        summary(sources.len(), target.is_some(), result.is_some())
    };
    tell(game, Some(uuid), &line);
}

/// `SulfuricFluxEmitterInteractionBehaviour.useItemOn` (main hand): says which pedestals it has and whether the array
/// is whole. (No outlines.)
fn click_emitter(game: &mut Game<'_>, pos: [i32; 3]) -> bool {
    let Some(nbt) = game.native_with_mod_block_entity(pos, |e| e.nbt.clone()) else { return false };
    let a = array_of(game, &nbt);
    let uuid = acting_uuid(game);
    tell(game, uuid, &summary(a.sources.len(), a.target.is_some(), a.result.is_some()));
    true
}

// ---- Clicks and placing ----------------------------------------------------------------------------------------

/// `placeItemBackInInventory`: into the acting player's inventory, what doesn't fit dropped at the block.
fn give(game: &mut Game<'_>, pos: [i32; 3], stack: ItemStack) {
    if let Some(player) = game.native_acting_player()
        && let Some(left) = game.native_give_stack(player.uuid(), stack)
    {
        game.native_pop_resource(pos, left);
    }
}

/// `native_right_click_block`: each apparatus' `useItemOn` (the upper half of a two-block apparatus acts as its lower
/// half). A click taken ends there; otherwise the default interaction goes on (placing a block, say).
pub(super) fn right_click(game: &mut Game<'_>, pos: [i32; 3], main_hand: bool, canceled: &mut bool) {
    if *canceled || game.remote.is_some() {
        return;
    }
    let block = game.native_block(pos);
    let Some(kind) = Kind::of(&block) else { return };
    let now = game_time(game);
    if let Some((time, at, taken)) = LAST_CLICK.with(|l| *l.borrow())
        && time == now
        && at == pos
        && !main_hand
    {
        *canceled |= taken;
        return;
    }
    // The sulfuric flux emitter in hand picks pedestals (`SulfuricFluxEmitterBlockItem.useOn` and the selection
    // behaviour), sneaking or not.
    let selecting = main_hand
        && matches!(kind, Kind::SourcePedestal | Kind::TargetPedestal | Kind::ResultPedestal)
        && game.native_used_stack().as_ref().is_some_and(|s| s.count > 0 && &*s.item == EMITTER_ITEM);
    // A sneaking player holding something skips the block's own use (`Player.isSecondaryUseActive`).
    if !selecting && (!main_hand || (game.native_sneaking() && game.native_used_stack().as_ref().is_some_and(|s| s.count > 0))) {
        return;
    }
    let pos = if kind.double() && game.native_block_property(pos, "half").as_deref() == Some("upper") { below(pos) } else { pos };
    let taken = if selecting { select(game, pos, kind) } else { match kind {
        Kind::Brazier => click_brazier(game, pos),
        Kind::Oven | Kind::Distiller => click_two_slot(game, pos, kind),
        Kind::Cauldron => click_tank(game, pos, kind) || click_two_slot(game, pos, kind),
        Kind::Incubator | Kind::Mercury | Kind::Salt | Kind::Sulfur => click_one_slot(game, pos, kind),
        Kind::Accumulator => click_tank(game, pos, kind) || click_one_slot(game, pos, kind),
        Kind::Tank => click_tank(game, pos, kind),
        Kind::Fermentation | Kind::Digestion => click_vat(game, pos, kind),
        Kind::SourcePedestal | Kind::TargetPedestal | Kind::ResultPedestal | Kind::Catalyst => click_one_slot(game, pos, kind),
        Kind::Emitter => click_emitter(game, pos),
    } };
    if taken {
        game.native_block_entity_sync(pos);
        *canceled = true;
    }
    LAST_CLICK.with(|l| *l.borrow_mut() = Some((now, pos, taken)));
}

/// The block state `state` with property `key` set to `value`; none where it has no such property.
fn with_property(game: &Game<'_>, state: BlockStateId, key: &str, value: &str) -> Option<BlockStateId> {
    let registry = game.pack.assets.registry;
    let mut s = registry.state(state)?.clone();
    if !s.properties.contains_key(key) {
        return None;
    }
    s.properties.insert(key.to_owned(), value.to_owned());
    registry.resolve(&s).ok()
}

fn replaceable(block: &str) -> bool {
    matches!(
        block,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air" | "minecraft:short_grass" | "minecraft:fern" | "minecraft:snow" | "minecraft:water" | "minecraft:lava"
    )
}

/// `native_on_block_placed_by`: a two-block apparatus' `setPlacedBy` sets its upper half, the same state with
/// `half=upper`, where the block above can be replaced.
pub(super) fn placed(game: &mut Game<'_>, pos: [i32; 3], placer: Option<u128>) {
    let Some(kind) = Kind::of(&game.native_block(pos)) else { return };
    if kind == Kind::Emitter {
        place_emitter(game, pos, placer);
        return;
    }
    if !kind.double() || game.native_block_property(pos, "half").as_deref() != Some("lower") {
        return;
    }
    let up = above(pos);
    if !replaceable(&game.native_block(up)) {
        return;
    }
    let Some(level) = game.fork_level_here() else { return };
    let Some(state) = game.level_block_state(level, pos) else { return };
    if let Some(upper) = with_property(game, state, "half", "upper") {
        let _ = game.level_set_block(level, up, upper, BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS);
    }
}
