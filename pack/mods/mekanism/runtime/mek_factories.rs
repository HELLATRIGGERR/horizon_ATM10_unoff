// Mekanism's Combiner, Precision Sawmill and factories in the game (support
// module `crate::game::mek_factories` of the `mekanism` native bundle). Their
// rules are the bundle's crate (`mekanism_energy::factory` and `machine`, MIT,
// from Mekanism 10.7.19, Copyright (c) 2017-2025 Aidan C. Brady); this module
// is the glue, beside `mek_machines` and on its helpers: their block entities
// and saved compound in Mekanism's names (a factory's `progress` an int array,
// `sorting`), their slots in Mekanism's order and the upgrade component's two,
// the recipes they read from the world's tables (`mekanism:combining`,
// `mekanism:sawing`, enriching, crushing and the world's smelting recipes for
// the factories of those), each tick in `TileEntityFactory.onUpdateServer`
// order (upgrades in, the active state's delay, the ejector, the energy slot,
// the sorting, each process's recipe on the one store), Joules from cables,
// cubes and Forge Energy on the faces their energy config opens, the tier
// installers (`native_use_item_on`, hook points API 165, and `/mek install`)
// and `/mek probe`. Each process's cached recipe, the upgrade slot's progress
// and the sorting's last look are kept here by position, not saved.

use super::{
    Game, mek_machines,
    mek_world::{self, ENERGY, ITEM},
    native_hooks::NativeFeInfo,
};
use horizon_client::{
    containers::{ModBlockEntity, ModContainer},
    items::ItemStack,
};
use horizon_core::{geometry::Direction as Face, random::RandomSource};
use horizon_world::nbt::{Compound, Tag};
use mekanism_energy::{
    config::EnergyConfig,
    container::{Automation, EnergyContainer},
    factory::{self, FactoryView, Layout, ProcessKind, SortSlot},
    fe::{self, Conversion, StrictEnergy},
    heat::Activity,
    machine::{self, Inputs, MachineKind, SideConfig, Upgrade, UpgradeSlot, Upgrades},
    menu::{self, MachineButton, MachineView},
    sides::{DataType, Direction, RelativeSide},
};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::RwLock,
};

use mek_machines::{ItemIn, item_in, same_kind, shrink, stack_in};

// ---- Recipes -------------------------------------------------------------------

/// A recipe of one of these blocks: `ItemStackToItemStackRecipe` (smelting,
/// enriching, crushing: `main` out), `CombinerRecipe` (`extra` in too) and
/// `SawmillRecipe` (`main` and `secondary` out, the secondary by `chance`).
#[derive(Clone, Debug)]
struct Recipe {
    id: String,
    input: ItemIn,
    extra: Option<ItemIn>,
    main: Option<ItemStack>,
    secondary: Option<ItemStack>,
    chance: f64,
}

impl Recipe {
    /// `getMaxSecondaryOutput`: the secondary a process must have room for.
    fn max_secondary(&self) -> Option<&ItemStack> {
        self.secondary.as_ref().filter(|_| self.chance > 0.0)
    }
}

/// The recipes by the machine whose they are, made again when a table changes.
#[derive(Default)]
struct Book {
    stamp: Vec<(usize, usize)>,
    recipes: HashMap<MachineKind, Vec<Rc<Recipe>>>,
}

const TYPES: [(MachineKind, &str); 4] = [
    (MachineKind::EnrichmentChamber, "mekanism:enriching"),
    (MachineKind::Crusher, "mekanism:crushing"),
    (MachineKind::Combiner, "mekanism:combining"),
    (MachineKind::PrecisionSawmill, "mekanism:sawing"),
];

thread_local! {
    static BOOK: RefCell<Book> = RefCell::new(Book::default());
    static EDITION: Cell<u64> = const { Cell::new(0) };
    static LIVE: RefCell<HashMap<(String, [i32; 3]), Live>> = RefCell::new(HashMap::new());
}

/// What the inputs and extra slots of each machine's recipes take, for
/// `native_block_entity_slot_accepts`, which may be asked with no world at hand.
#[derive(Default)]
struct Takes {
    inputs: HashMap<MachineKind, HashSet<String>>,
    extra: HashSet<String>,
}

static TAKES: RwLock<Option<Takes>> = RwLock::new(None);

/// A block's state that Mekanism keeps in its block entity but doesn't save.
#[derive(Clone, Debug, Default)]
struct Live {
    /// Each process's cached recipe and the book it came from.
    recipes: Vec<Option<(u64, Rc<Recipe>)>>,
    upgrade: UpgradeSlot,
    upgrade_stack: Option<(String, u32)>,
    eject_delay: u32,
    /// The process slots as the sorting last left them (`sortingNeeded` is
    /// any change since), and the book then.
    sorted: Option<(u64, Vec<Option<(String, u32)>>)>,
}

/// Makes the book again from the world's tables when one changed.
fn refresh_book(game: &Game<'_>) {
    let cooking = game.native_cooking_recipes();
    let mut stamp: Vec<(usize, usize)> = TYPES
        .iter()
        .map(|(_, t)| {
            let s = game.native_recipes_of_type(t);
            (s.as_ptr() as usize, s.len())
        })
        .collect();
    stamp.push((cooking.as_ptr() as usize, cooking.len()));
    if BOOK.with(|b| b.borrow().stamp == stamp) {
        return;
    }
    let mut book = Book { stamp, ..Book::default() };
    for (kind, recipe_type) in TYPES {
        let list: Vec<Rc<Recipe>> = game
            .native_recipes_of_type(recipe_type)
            .iter()
            .filter_map(|r| {
                let j = &*r.json;
                let recipe = match kind {
                    MachineKind::Combiner => Recipe {
                        id: r.id.clone(),
                        input: item_in(game, &j["main_input"])?,
                        extra: Some(item_in(game, &j["extra_input"])?),
                        main: Some(game.native_recipe_result(&j["output"])?),
                        secondary: None,
                        chance: 0.0,
                    },
                    MachineKind::PrecisionSawmill => {
                        let main = game.native_recipe_result(&j["main_output"]).filter(|s| s.count > 0);
                        let secondary = game.native_recipe_result(&j["secondary_output"]).filter(|s| s.count > 0);
                        let chance = j.get("secondary_chance").and_then(Value::as_f64).unwrap_or(0.0);
                        if main.is_none() && secondary.is_none() || !(0.0..=1.0).contains(&chance) {
                            return None;
                        }
                        Recipe { id: r.id.clone(), input: item_in(game, &j["input"])?, extra: None, main, secondary, chance }
                    }
                    _ => Recipe {
                        id: r.id.clone(),
                        input: item_in(game, &j["input"])?,
                        extra: None,
                        main: Some(game.native_recipe_result(&j["output"])?),
                        secondary: None,
                        chance: 0.0,
                    },
                };
                Some(Rc::new(recipe))
            })
            .collect();
        book.recipes.insert(kind, list);
    }
    // `MekanismRecipeType.SMELTING`: each of vanilla's smelting recipes, one in, its result out.
    let smelting = cooking
        .iter()
        .filter(|c| c.kind == "minecraft:smelting" && c.result.count > 0 && !c.ingredient.0.is_empty())
        .map(|c| {
            Rc::new(Recipe {
                id: format!("minecraft:{}", c.id),
                input: ItemIn { items: c.ingredient.0.clone(), count: 1 },
                extra: None,
                main: Some(c.result.clone()),
                secondary: None,
                chance: 0.0,
            })
        })
        .collect();
    book.recipes.insert(MachineKind::EnergizedSmelter, smelting);
    let takes = Takes {
        inputs: book.recipes.iter().map(|(kind, list)| (*kind, list.iter().flat_map(|r| r.input.items.iter().cloned()).collect())).collect(),
        extra: book.recipes[&MachineKind::Combiner].iter().flat_map(|r| r.extra.iter().flat_map(|e| e.items.iter().cloned())).collect(),
    };
    if let Ok(mut t) = TAKES.write() {
        *t = Some(takes);
    }
    BOOK.with(|b| *b.borrow_mut() = book);
    EDITION.set(EDITION.get() + 1);
}

/// `InventoryUtils.areItemsStackable`: an empty slot takes anything.
fn stacks_with(output: Option<&ItemStack>, made: Option<&ItemStack>) -> bool {
    match (output, made) {
        (Some(o), Some(m)) => same_kind(o, m),
        _ => true,
    }
}

/// What a recipe lookup of one process reads besides its input: the shared
/// extra slot and its outputs.
#[derive(Clone, Copy)]
struct ProcessSlots<'a> {
    extra: Option<&'a ItemStack>,
    output: Option<&'a ItemStack>,
    secondary: Option<&'a ItemStack>,
}

/// `findFirstRecipe` of a machine (`getRecipe`), and a factory's `findRecipe`,
/// which also asks that the recipe's output stacks with what its output slots
/// hold (`OUTPUT_CHECK`: the sawing factory's secondary slot empty, or the
/// recipe's secondary the same item). The combiner's recipes take the extra
/// item too.
fn find(kind: ProcessKind, input: &ItemStack, slots: ProcessSlots<'_>) -> Option<Rc<Recipe>> {
    BOOK.with(|b| {
        b.borrow()
            .recipes
            .get(&kind.base())?
            .iter()
            .find(|r| {
                r.input.takes(input)
                    && r.extra.as_ref().is_none_or(|e| slots.extra.is_some_and(|x| e.takes(x)) || kind.is_factory() && slots.extra.is_none())
                    && (!kind.is_factory()
                        || stacks_with(slots.output, r.main.as_ref())
                            && slots.secondary.is_none_or(|s| r.max_secondary().is_none_or(|m| same_kind(s, m))))
            })
            .cloned()
    })
}

/// `isCachedRecipeValid` (`testType`): the cached recipe still takes the input,
/// and the combiner's the extra item (a factory's also an empty extra slot).
fn still_valid(kind: ProcessKind, recipe: &Recipe, input: &ItemStack, extra: Option<&ItemStack>) -> bool {
    recipe.input.takes(input)
        && recipe.extra.as_ref().is_none_or(|e| match extra {
            Some(x) => e.takes(x),
            None => kind.is_factory(),
        })
}

// ---- The saved compound -----------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
struct Saved {
    energy: u64,
    /// Each process's progress: a machine's `progress`, a factory's int array.
    progress: Vec<u32>,
    upgrades: Upgrades,
    sides: SideConfig,
    facing: Direction,
    activity: Activity,
    sorting: bool,
}

fn key(name: &str) -> horizon_world::nbt::JavaString {
    mek_world::key(name)
}

fn read(nbt: &Compound, kind: ProcessKind) -> Saved {
    let base = kind.base();
    let mut sides = SideConfig::new(base);
    if let Some(Tag::Compound(config)) = nbt.get(&key("component_config")) {
        mek_world::sides_of(config, ENERGY, &mut sides.energy);
        mek_world::sides_of(config, ITEM, &mut sides.items);
        if let Some(v) = mek_world::number(config.get(&key(&format!("eject{ITEM}")))) {
            sides.items_eject = v != 0;
        }
    }
    let mut upgrades = Upgrades::default();
    if let Some(Tag::Compound(c)) = nbt.get(&key("component_upgrade"))
        && let Some(Tag::List(list)) = c.get(&key("upgrades"))
    {
        for entry in list {
            if let Tag::Compound(u) = entry {
                let upgrade = Upgrade::by_id(mek_world::number(u.get(&key("type"))).unwrap_or(0) as i32);
                upgrades.set(upgrade, mek_world::number(u.get(&key("amount"))).unwrap_or(0).max(0) as u32);
            }
        }
    }
    // `loadAdditional`: a saved array of another length leaves the progress at 0.
    let processes = kind.processes();
    let progress = match nbt.get(&key("progress")) {
        Some(Tag::IntArray(saved)) if kind.is_factory() && saved.len() == processes => saved.iter().map(|p| (*p).max(0) as u32).collect(),
        tag if !kind.is_factory() => vec![mek_world::number(tag).unwrap_or(0).clamp(0, i64::from(i32::MAX)) as u32],
        _ => vec![0; processes],
    };
    Saved {
        energy: mek_world::saved_energy(nbt).unwrap_or(0),
        progress,
        upgrades,
        sides,
        facing: mek_world::generator_facing(nbt),
        activity: mek_world::read_activity(nbt),
        sorting: mek_world::number(nbt.get(&key("sorting"))).is_some_and(|v| v != 0),
    }
}

fn write_sides(nbt: &mut Compound, saved: &Saved) {
    nbt.insert(key("facing"), Tag::String(key(saved.facing.name())));
    let mut config = Compound::new();
    let mut put = |kind: u8, sides: &[DataType; 6], eject: bool| {
        config.insert(key(&format!("eject{kind}")), Tag::Byte(i8::from(eject)));
        config.insert(key(&format!("config{kind}")), Tag::IntArray(sides.iter().map(|t| t.id()).collect()));
    };
    put(ENERGY, &saved.sides.energy, false);
    put(ITEM, &saved.sides.items, saved.sides.items_eject);
    nbt.insert(key("component_config"), Tag::Compound(config));
}

fn write(nbt: &mut Compound, saved: &Saved, before: &Saved, kind: ProcessKind) {
    if saved.energy != before.energy || !nbt.contains_key(&key("energy_containers")) && saved.energy > 0 {
        mek_world::write_energy(nbt, saved.energy);
    }
    if saved.progress != before.progress || !nbt.contains_key(&key("progress")) {
        let tag = if kind.is_factory() {
            Tag::IntArray(saved.progress.iter().map(|p| (*p).min(i32::MAX as u32) as i32).collect())
        } else {
            Tag::Int(saved.progress.first().copied().unwrap_or(0).min(i32::MAX as u32) as i32)
        };
        nbt.insert(key("progress"), tag);
    }
    if kind.is_factory() && (saved.sorting != before.sorting || !nbt.contains_key(&key("sorting"))) {
        nbt.insert(key("sorting"), Tag::Byte(i8::from(saved.sorting)));
    }
    if saved.upgrades != before.upgrades {
        let list: Vec<Tag> = Upgrade::ALL
            .iter()
            .filter(|u| saved.upgrades.get(**u) > 0)
            .map(|u| {
                let mut c = Compound::new();
                c.insert(key("type"), Tag::Int(u.ordinal()));
                c.insert(key("amount"), Tag::Int(saved.upgrades.get(*u) as i32));
                Tag::Compound(c)
            })
            .collect();
        let mut component = Compound::new();
        if !list.is_empty() {
            component.insert(key("upgrades"), Tag::List(list));
        }
        nbt.insert(key("component_upgrade"), Tag::Compound(component));
    }
    if saved.sides != before.sides || saved.facing != before.facing || !nbt.contains_key(&key("component_config")) {
        write_sides(nbt, saved);
    }
    if saved.activity != before.activity || !nbt.contains_key(&key("active_state")) {
        mek_world::write_activity(nbt, saved.activity);
    }
}

/// The store: the block's base storage (`ProcessKind::base_storage`) by its
/// energy upgrades.
fn store(saved: &Saved, config: &EnergyConfig, kind: ProcessKind) -> EnergyContainer {
    let (usage, storage) = config.machines.of(kind.base());
    let base = kind.base_storage(usage, storage);
    let mut container = EnergyContainer::basic(saved.upgrades.max_energy(base, config.machines.max_upgrade_multiplier));
    container.stored = saved.energy.min(container.capacity);
    container
}

// ---- Block entities and slots -----------------------------------------------------

pub(super) fn kind_of(block: &str) -> Option<ProcessKind> {
    ProcessKind::of_block(block)
}

/// `native_block_entity`: the item's side config (`AttachedSideConfig`:
/// the combiner's and the combining factory's `EXTRA_MACHINE`, the others'
/// `ELECTRIC_MACHINE`), a factory not sorting.
pub(super) fn new_entity(kind: ProcessKind) -> (String, Compound) {
    let saved = Saved {
        energy: 0,
        progress: vec![0; kind.processes()],
        upgrades: Upgrades::default(),
        sides: SideConfig::new(kind.base()),
        facing: Direction::North,
        activity: Activity::default(),
        sorting: false,
    };
    let mut nbt = Compound::new();
    write_sides(&mut nbt, &saved);
    if kind.is_factory() {
        nbt.insert(key("sorting"), Tag::Byte(0));
    }
    (kind.id(), nbt)
}

/// `native_block_entity_container`: Mekanism's slots and the upgrade
/// component's two in the smallest grid menu that has room (the screen
/// leaves the rest out). As the other machines', every face reaches the
/// inputs, the extra and energy slots and the outputs.
pub(super) fn container(kind: ProcessKind) -> ModContainer {
    let layout = kind.layout();
    let inputs = layout.takers();
    let outputs = layout.givers();
    let mut faces = inputs.clone();
    faces.extend(&outputs);
    faces.sort_unstable();
    let (menu, slots) = layout.menu();
    ModContainer { slots, faces: std::array::from_fn(|_| faces.clone()), inputs, outputs, menu: Some(menu.into()) }
}

/// `native_block_entity_slot_accepts`: an input slot takes what a recipe of
/// the block takes (`containsRecipe`; the combiner's `containsRecipeA`), the
/// extra slot what a combining recipe takes as its extra, the energy slot
/// what converts to energy, the upgrade slot the machine upgrades; nothing
/// goes into an output. A factory's input that its output can't follow
/// (`inputProducesOutput`) isn't refused here: that needs the block's slots.
pub(super) fn slot_accepts(kind: ProcessKind, slot: usize, stack: &ItemStack, accepts: &mut bool) {
    let layout = kind.layout();
    let item = &*stack.item;
    let known = |pick: &dyn Fn(&Takes) -> bool| TAKES.read().ok().and_then(|t| t.as_ref().map(pick)).unwrap_or(true);
    let ok = if layout.inputs.contains(&slot) {
        known(&|t| t.inputs.get(&kind.base()).is_some_and(|i| i.contains(item)))
    } else if layout.extra == Some(slot) {
        known(&|t| t.extra.contains(item))
    } else if slot == layout.energy {
        mek_machines::energy_item(item) || mekanism_energy::tier::CubeTier::of_block(item).is_some()
    } else if slot == layout.upgrade {
        Upgrade::of_item(item).is_some_and(|u| machine::MACHINE_UPGRADES.contains(&u))
    } else {
        false
    };
    if !ok {
        *accepts = false;
    }
}

fn live_key(dimension: &str, pos: [i32; 3]) -> (String, [i32; 3]) {
    (dimension.to_owned(), pos)
}

pub(super) fn forget(dimension: &str, pos: [i32; 3]) {
    LIVE.with(|l| l.borrow_mut().remove(&live_key(dimension, pos)));
}

pub(super) fn clear() {
    LIVE.with(|l| l.borrow_mut().clear());
    BOOK.with(|b| *b.borrow_mut() = Book::default());
}

/// The process slots' contents, to notice a change (`sortingNeeded`).
fn snapshot(entity: &ModBlockEntity, layout: &Layout) -> Vec<Option<(String, u32)>> {
    let mut slots = layout.inputs.clone();
    slots.extend(layout.givers());
    slots.iter().map(|&s| stack_in(entity, s).map(|i| (i.item.to_string(), i.count))).collect()
}

// ---- The tick ---------------------------------------------------------------------

/// `native_block_entity_tick`: `TileEntityMekanism.tickServer` (the upgrade
/// component, the active state's delay) and `onUpdateServer` (the ejector,
/// the energy slot, a factory's sorting, then each process's
/// `RecipeCacheLookupMonitor.updateAndProcess` in turn on the one store),
/// then the menu's data slots. Redstone control is IGNORED, the default.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, kind: ProcessKind) {
    let (config, _) = mek_world::energy_config();
    let dimension = game.native_dimension();
    refresh_book(game);
    mek_machines::refresh_book(game);
    let layout = kind.layout();
    let multiplier = config.machines.max_upgrade_multiplier;
    let (usage, _) = config.machines.of(kind.base());
    let before = read(&entity.nbt, kind);
    let mut saved = before.clone();
    saved.facing = game.native_block_property(pos, "facing").and_then(|f| Direction::from_name(&f)).unwrap_or(Direction::North);
    let id = live_key(dimension, pos);
    let mut live = LIVE.with(|l| l.borrow_mut().remove(&id)).unwrap_or_default();
    live.recipes.resize(kind.processes(), None);

    // The upgrade component: an upgrade in its slot goes in after a second.
    let upgrade_stack = stack_in(entity, layout.upgrade).map(|s| (s.item.to_string(), s.count));
    if upgrade_stack != live.upgrade_stack {
        live.upgrade.check = true;
    }
    let offered = upgrade_stack.as_ref().and_then(|(item, count)| Some((Upgrade::of_item(item)?, *count)));
    let taken = live.upgrade.tick(&mut saved.upgrades, &machine::MACHINE_UPGRADES, offered);
    if taken > 0 {
        shrink(entity, layout.upgrade, taken);
    }
    live.upgrade_stack = stack_in(entity, layout.upgrade).map(|s| (s.item.to_string(), s.count));

    // The active state's delay runs down.
    let mut shown = game.native_block_property(pos, "active").as_deref() == Some("true");
    let changed = saved.activity.tick(shown);
    mek_world::show_active(game, pos, changed, &mut shown);

    // The ejector: every half second, the outputs out of the faces whose item type outputs.
    if saved.sides.items_eject {
        if live.eject_delay == 0 {
            let faces = machine::eject_faces(&saved.sides.items, saved.facing);
            for slot in layout.givers() {
                mek_machines::eject_items(game, pos, entity, slot, &faces);
            }
            live.eject_delay = machine::EJECT_DELAY;
        } else {
            live.eject_delay -= 1;
        }
    }

    let mut store = store(&saved, &config, kind);
    // The energy slot (`fillContainerOrConvert`): an energy cube's item gives
    // what it can, else an item that converts to energy, when all of it fits.
    if let Some(stack) = stack_in(entity, layout.energy).cloned()
        && store.needed() > 0
    {
        let mut item = stack.clone();
        match mek_world::drain_energy_item(&mut item, store.needed()) {
            Some(taken) if taken > 0 => {
                store.insert(taken, false, Automation::Internal);
                if let Some(cell) = entity.items.get_mut(layout.energy) {
                    *cell = Some(item);
                }
            }
            _ => {
                if let Some((count, joules)) = mek_machines::energy_conversion(&stack)
                    && stack.count >= count
                    && store.needed() >= joules
                {
                    store.insert(joules, false, Automation::Manual);
                    shrink(entity, layout.energy, count);
                }
            }
        }
    }

    // A factory's sorting, when its slots changed since it last sorted.
    let edition = EDITION.get();
    if kind.is_factory() && saved.sorting {
        let now = snapshot(entity, &layout);
        if live.sorted.as_ref().is_none_or(|(e, last)| *e != edition || *last != now) {
            sort(game, entity, kind, &layout);
            live.sorted = Some((edition, snapshot(entity, &layout)));
        }
    }

    // Each process's recipe, in turn, on the one store.
    let required = saved.upgrades.ticks(machine::BASE_TICKS, multiplier);
    let per_tick = saved.upgrades.energy_per_tick(usage, multiplier);
    let baseline = saved.upgrades.operations_per_tick(machine::BASE_TICKS, multiplier);
    let start = store.stored;
    let mut warnings = 0;
    let mut any_active = None;
    for process in 0..kind.processes() {
        let ran = run_process(game, entity, kind, &layout, process, &mut saved, &mut live, &mut store, (required, per_tick, baseline), edition, &mut warnings);
        if let Some(active) = ran {
            any_active = Some(any_active.unwrap_or(false) || active);
        }
    }
    let used = start.saturating_sub(store.stored);
    saved.energy = store.stored;
    if let Some(active) = any_active {
        let changed = saved.activity.set(active, shown, config.deactivation_delay);
        mek_world::show_active(game, pos, changed, &mut shown);
    }
    write(&mut entity.nbt, &saved, &before, kind);
    let input_joules = mek_world::take_received(pos);
    let mut data = machine_view(&saved, &store, per_tick, required, input_joules, warnings, live.upgrade.ticks).pack();
    if kind.is_factory() {
        let last_usage = if saved.activity.current { used } else { 0 };
        data.extend(FactoryView { processes: kind.processes() as u8, sorting: saved.sorting, last_usage, progress: saved.progress.clone() }.pack());
    }
    entity.data = data;
    LIVE.with(|l| l.borrow_mut().insert(id, live));
}

/// One process's `CachedRecipe.process`: its cached recipe looked up again
/// when its input no longer fits it, then `machine::process` on the store.
/// The answer is whether it was active (none: no recipe to run).
#[allow(clippy::too_many_arguments)]
fn run_process(
    game: &mut Game<'_>,
    entity: &mut ModBlockEntity,
    kind: ProcessKind,
    layout: &Layout,
    process: usize,
    saved: &mut Saved,
    live: &mut Live,
    store: &mut EnergyContainer,
    (required, per_tick, baseline): (u32, u64, u32),
    edition: u64,
    warnings: &mut i32,
) -> Option<bool> {
    let input_slot = layout.inputs[process];
    let output_slot = layout.outputs[process];
    let secondary_slot = layout.secondaries.get(process).copied();
    let input = stack_in(entity, input_slot).cloned();
    let extra = layout.extra.and_then(|s| stack_in(entity, s)).cloned();
    let output = stack_in(entity, output_slot).cloned();
    let secondary = secondary_slot.and_then(|s| stack_in(entity, s)).cloned();
    let slots = ProcessSlots { extra: extra.as_ref(), output: output.as_ref(), secondary: secondary.as_ref() };
    let cached = live.recipes[process].as_ref().filter(|(e, _)| *e == edition).map(|(_, r)| r.clone());
    let progress = &mut saved.progress[process];
    let recipe = match cached {
        Some(r) if input.as_ref().is_some_and(|i| still_valid(kind, &r, i, extra.as_ref())) => Some(r),
        old => match input.as_ref().and_then(|i| find(kind, i, slots)) {
            Some(found) => {
                // A first cache takes the saved progress (`loadSavedOperatingTicks`); another starts at 0.
                *progress = if old.is_none() { machine::resumed(*progress, required) } else { 0 };
                live.recipes[process] = Some((edition, found.clone()));
                Some(found)
            }
            None => {
                if input.is_some() && process == 0 {
                    *warnings |= menu::WARN_NO_RECIPE;
                }
                if old.is_none() {
                    live.recipes[process] = None;
                }
                old
            }
        },
    };
    let recipe = recipe?;
    let count = input.as_ref().filter(|i| recipe.input.takes(i)).map_or(0, |i| i.count);
    let mut inputs = Inputs::of(count, recipe.input.count);
    if let Some(needed) = &recipe.extra {
        let held = extra.as_ref().filter(|x| needed.takes(x)).map_or(0, |x| x.count);
        inputs = inputs.both(Inputs::of(held, needed.count));
    }
    let room = |held: Option<&ItemStack>, made: Option<&ItemStack>| -> u32 {
        let Some(made) = made else {
            return u32::MAX;
        };
        let max_stack = game.native_max_stack_size(&made.item).unwrap_or(64);
        machine::output_operations(held.map(|o| (o.count, same_kind(o, made))), made.count, max_stack)
    };
    let mut space = room(output.as_ref(), recipe.main.as_ref());
    if let Some(max) = recipe.max_secondary() {
        space = space.min(room(secondary.as_ref(), Some(max)));
    }
    if space == 0 && process == 0 {
        *warnings |= menu::WARN_OUTPUT;
    }
    if per_tick > store.stored {
        *warnings |= menu::WARN_ENERGY;
    }
    let ran = machine::process(progress, required, store, per_tick, baseline, inputs, space, true);
    if ran.finished {
        shrink(entity, input_slot, recipe.input.count * ran.operations);
        if let (Some(needed), Some(at)) = (&recipe.extra, layout.extra) {
            shrink(entity, at, needed.count * ran.operations);
        }
        if let Some(main) = &recipe.main {
            add(entity, output_slot, main, main.count * ran.operations);
        }
        // `handleOutput`: the secondary rolls once an operation (Java gives each
        // successful roll `operations` times over, which a 200-tick recipe never
        // reaches: it runs one operation a tick at most).
        if let (Some(extra_out), Some(at)) = (recipe.max_secondary(), secondary_slot) {
            let dimension = game.native_dimension();
            let mut made = 0;
            for _ in 0..ran.operations {
                let roll = game.native_level_random(dimension).map_or(0.0, |r| r.next_double());
                if factory::secondary_comes(recipe.chance, roll) {
                    made += extra_out.count;
                }
            }
            if made > 0 {
                add(entity, at, extra_out, made);
            }
        }
    }
    Some(ran.active())
}

fn add(entity: &mut ModBlockEntity, slot: usize, made: &ItemStack, count: u32) {
    match entity.items.get_mut(slot) {
        Some(Some(out)) => out.count += count,
        Some(cell) => *cell = Some(ItemStack { count, ..made.clone() }),
        None => {}
    }
}

/// `sortInventory` over the factory's input slots: `factory::sort_inputs`
/// with each kind's recipe (`getNeededInput` of the recipe for a stack of
/// the total) and the empty processes whose outputs take that recipe's
/// (`inputProducesOutput`).
fn sort(game: &Game<'_>, entity: &mut ModBlockEntity, kind: ProcessKind, layout: &Layout) {
    let stacks: Vec<Option<ItemStack>> = layout.inputs.iter().map(|&s| stack_in(entity, s).cloned()).collect();
    let slots: Vec<Option<SortSlot<usize>>> = stacks
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let s = s.as_ref()?;
            // The kind: the first process holding the same item and components.
            let first = stacks.iter().position(|o| o.as_ref().is_some_and(|o| same_kind(o, s))).unwrap_or(i);
            Some(SortSlot { key: first, count: s.count, max_stack: game.native_max_stack_size(&s.item).unwrap_or(64).min(64) })
        })
        .collect();
    let extra = layout.extra.and_then(|s| stack_in(entity, s)).cloned();
    let process_slots = |p: usize| ProcessSlots {
        extra: extra.as_ref(),
        output: stack_in(entity, layout.outputs[p]),
        secondary: layout.secondaries.get(p).and_then(|&s| stack_in(entity, s)),
    };
    let sorted = factory::sort_inputs(
        &slots,
        |&first, total, process| {
            let stack = ItemStack { count: total, ..stacks[first].clone().unwrap_or_else(|| ItemStack::new("minecraft:air", 0)) };
            find(kind, &stack, process_slots(process)).map_or(1, |r| r.input.count)
        },
        |process, &first| {
            let slots = process_slots(process);
            slots.output.is_none() || stacks[first].as_ref().is_some_and(|s| find(kind, s, slots).is_some())
        },
    );
    for (process, new) in sorted.into_iter().enumerate() {
        let slot = layout.inputs[process];
        let value = new.and_then(|(first, count)| Some(ItemStack { count, ..stacks[first].clone()? }));
        if let Some(cell) = entity.items.get_mut(slot) {
            *cell = value;
        }
    }
}

fn machine_view(saved: &Saved, store: &EnergyContainer, per_tick: u64, required: u32, input: u64, warnings: i32, upgrade_ticks: u32) -> MachineView {
    MachineView {
        stored: saved.energy,
        capacity: store.capacity,
        per_tick,
        progress: saved.progress.first().copied().unwrap_or(0),
        required,
        active: saved.activity.current,
        energy: saved.sides.energy,
        items: saved.sides.items,
        chemicals: None,
        items_eject: saved.sides.items_eject,
        upgrades: saved.upgrades,
        upgrade_ticks,
        infuse: 0,
        infused: 0,
        input,
        warnings,
    }
}

// ---- Energy from outside ------------------------------------------------------------

pub(super) fn takes_energy(entity: &ModBlockEntity, face: Direction) -> Option<bool> {
    let kind = kind_of(&entity.id)?;
    let saved = read(&entity.nbt, kind);
    Some(saved.sides.takes_energy(saved.facing, face))
}

pub(super) fn offer(entity: &mut ModBlockEntity, config: &EnergyConfig, face: Direction, amount: u64, simulate: bool) -> Option<u64> {
    let kind = kind_of(&entity.id)?;
    let saved = read(&entity.nbt, kind);
    if !saved.sides.takes_energy(saved.facing, face) {
        return Some(amount);
    }
    let mut container = store(&saved, config, kind);
    let rest = container.insert(amount, simulate, Automation::External);
    if !simulate && container.stored != saved.energy {
        mek_world::write_energy(&mut entity.nbt, container.stored);
    }
    Some(rest)
}

/// The store as one side reaches it: an input side takes, nothing gives.
struct Side {
    container: EnergyContainer,
    open: bool,
    taken: u64,
}

impl StrictEnergy for Side {
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

pub(super) fn fe_storage(entity: &ModBlockEntity, face: Option<Face>, config: &EnergyConfig, info: &mut Option<NativeFeInfo>) {
    let Some(kind) = kind_of(&entity.id) else {
        return;
    };
    let saved = read(&entity.nbt, kind);
    if config.blacklist_forge || face.is_some_and(|f| !saved.sides.takes_energy(saved.facing, mek_world::mek_direction(f))) {
        return;
    }
    let container = store(&saved, config, kind);
    let rate = Conversion(config.fe_conversion_rate);
    *info = Some(NativeFeInfo {
        stored: i64::from(rate.to_fe_int(container.stored.min(i64::MAX as u64) as i64)),
        capacity: i64::from(rate.to_fe_int(container.capacity.min(i64::MAX as u64) as i64)),
        can_receive: true,
        can_extract: true,
    });
}

#[allow(clippy::too_many_arguments)]
pub(super) fn fe_transfer(
    pos: [i32; 3],
    entity: &mut ModBlockEntity,
    face: Option<Face>,
    config: &EnergyConfig,
    max: i64,
    simulate: bool,
    extract: bool,
    moved: &mut Option<i64>,
) {
    let Some(kind) = kind_of(&entity.id) else {
        return;
    };
    let saved = read(&entity.nbt, kind);
    let open = face.is_some_and(|f| saved.sides.takes_energy(saved.facing, mek_world::mek_direction(f)));
    if config.blacklist_forge || face.is_some() && !open {
        return;
    }
    let mut side = Side { container: store(&saved, config, kind), open, taken: 0 };
    let rate = Conversion(config.fe_conversion_rate);
    let max = max.clamp(0, i64::from(i32::MAX)) as i32;
    let fe = if extract { fe::extract_fe(&mut side, rate, max, simulate) } else { fe::receive_fe(&mut side, rate, max, simulate) };
    if !simulate && side.container.stored != saved.energy {
        mek_world::write_energy(&mut entity.nbt, side.container.stored);
    }
    if side.taken > 0 {
        mek_world::add_received(pos, side.taken);
    }
    *moved = Some(i64::from(fe));
}

// ---- Tier installers -----------------------------------------------------------------

/// `ItemTierInstaller.useOn` (a player sneaking, so the machine's screen
/// doesn't open first): the block becomes the next tier with what it held
/// (`getUpgradeData`, `parseUpgradeData`): its energy (up to the new block's
/// store before its upgrades are read, as `setEnergy` clamps), each process's
/// progress, input and outputs, the energy and extra slots, its upgrades and
/// the upgrade slots, its side configuration and ejecting, a factory's
/// sorting, and its facing. Whether it changed; the installer is used up
/// unless the player builds in creative.
pub(super) fn use_installer(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3], claimed: &mut bool) {
    if *claimed || factory::installer(&stack.item).is_none() {
        return;
    }
    if install(game, pos, &stack.item).is_ok() {
        *claimed = true;
        if !game.native_instabuild() {
            let hand = game.native_used_stack();
            if let Some(held) = hand.as_mut() {
                held.count = held.count.saturating_sub(1);
            }
            if hand.as_ref().is_some_and(|s| s.count == 0) {
                *hand = None;
            }
        }
    }
}

/// The installer's conversion of the block at `pos`: what it became.
pub(super) fn install(game: &mut Game<'_>, pos: [i32; 3], installer: &str) -> Result<String, String> {
    let old = game.native_block(pos);
    let new = factory::upgraded(&old, installer).ok_or_else(|| format!("{installer} doesn't upgrade {old}"))?;
    let (from, to) = (factory::layout_of(&old).ok_or("no layout")?, factory::layout_of(&new).ok_or("no layout")?);
    let kind = kind_of(&new).ok_or("not a factory")?;
    // Take the old block's contents out first, so replacing it drops nothing.
    let taken = game.native_with_mod_block_entity(pos, |m| (m.nbt.clone(), std::mem::take(&mut m.items))).ok_or("no block entity")?;
    let (nbt, items) = taken;
    let facing = game.native_block_property(pos, "facing").unwrap_or_else(|| "north".into());
    let state = game.native_default_state(&new).ok_or("no such block")?;
    let dimension = game.native_dimension();
    if !game.native_set_block(dimension, pos, state, 3) {
        game.native_with_mod_block_entity(pos, |m| m.items = items);
        return Err("the block didn't change".into());
    }
    game.native_set_block_property(pos, "facing", &facing);
    mek_machines::forget(dimension, pos);
    forget(dimension, pos);
    let (config, _) = mek_world::energy_config();
    let old_progress: Vec<u32> = match nbt.get(&key("progress")) {
        Some(Tag::IntArray(p)) => p.iter().map(|v| (*v).max(0) as u32).collect(),
        tag => vec![mek_world::number(tag).unwrap_or(0).max(0) as u32],
    };
    let sorting = mek_world::number(nbt.get(&key("sorting"))).is_some_and(|v| v != 0);
    let moved = game.native_with_mod_block_entity(pos, |m| {
        let mut saved = read(&m.nbt, kind);
        let before = saved.clone();
        // `setEnergy` before the upgrades are read: the new block's own store.
        let (usage, storage) = config.machines.of(kind.base());
        saved.energy = mek_world::saved_energy(&nbt).unwrap_or(0).min(kind.base_storage(usage, storage));
        for (i, p) in old_progress.iter().enumerate().take(saved.progress.len()) {
            saved.progress[i] = *p;
        }
        saved.sorting = sorting;
        saved.facing = Direction::from_name(&facing).unwrap_or(Direction::North);
        write(&mut m.nbt, &saved, &before, kind);
        for name in ["component_config", "component_upgrade"] {
            if let Some(tag) = nbt.get(&key(name)) {
                m.nbt.insert(key(name), tag.clone());
            }
        }
        let len = m.items.len().max(to.menu().1);
        m.items.resize(len, None);
        for (a, b) in from.transfer(&to) {
            if let (Some(Some(stack)), Some(cell)) = (items.get(a), m.items.get_mut(b)) {
                *cell = Some(stack.clone());
            }
        }
    });
    if moved.is_none() {
        return Err("the new block has no block entity".into());
    }
    game.native_block_entity_changed(pos);
    Ok(new)
}

// ---- Buttons and probes ---------------------------------------------------------------

/// `native_menu_button`: the side configuration, the upgrade window's
/// uninstall (into the upgrade output slot, as far as it fits) and a
/// factory's sorting tab (`toggleSorting`).
pub(super) fn button(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, id: i32) {
    let Some(kind) = kind_of(&entity.id) else {
        return;
    };
    let before = read(&entity.nbt, kind);
    let mut saved = before.clone();
    if id == factory::SORT_BUTTON && kind.is_factory() {
        saved.sorting = !saved.sorting;
    } else {
        match MachineButton::from_id(id) {
            Some(MachineButton::Config(b)) => {
                saved.sides.press(kind.base(), b);
            }
            Some(MachineButton::Remove { upgrade, all }) => {
                let installed = saved.upgrades.get(upgrade);
                let out = kind.layout().upgrade_out;
                let item = upgrade.item();
                let max = game.native_max_stack_size(&item).unwrap_or(64).min(64);
                let held = stack_in(entity, out);
                let room = match held {
                    None => max,
                    Some(s) if &*s.item == item => max.saturating_sub(s.count),
                    Some(_) => 0,
                };
                let remove = (if all { installed } else { 1 }).min(room).min(installed);
                if remove > 0 {
                    saved.upgrades.set(upgrade, installed - remove);
                    let count = held.map_or(0, |s| s.count) + remove;
                    if let Some(cell) = entity.items.get_mut(out) {
                        *cell = Some(ItemStack::new(&item, count));
                    }
                    LIVE.with(|l| {
                        if let Some(live) = l.borrow_mut().get_mut(&live_key(game.native_dimension(), pos)) {
                            live.upgrade.check = true;
                        }
                    });
                }
            }
            _ => {}
        }
    }
    if saved != before {
        write(&mut entity.nbt, &saved, &before, kind);
        let mut view = MachineView::unpack(&entity.data).unwrap_or_default();
        view.energy = saved.sides.energy;
        view.items = saved.sides.items;
        view.items_eject = saved.sides.items_eject;
        view.upgrades = saved.upgrades;
        let mut data = view.pack();
        if kind.is_factory() {
            let mut extra = FactoryView::unpack(&entity.data, menu::MACHINE_SLOTS).unwrap_or_default();
            extra.processes = kind.processes() as u8;
            extra.sorting = saved.sorting;
            data.extend(extra.pack());
        }
        entity.data = data;
    }
}

/// What `/mek probe` answers for one of these blocks.
pub(super) fn probe(game: &Game<'_>, pos: [i32; 3], id: &str, nbt: &Compound, data: &[i32], items: &[Option<ItemStack>]) -> Value {
    let Some(kind) = kind_of(id) else {
        return serde_json::json!({ "pos": pos, "id": id, "error": "not a machine" });
    };
    let (config, from_files) = mek_world::energy_config();
    let saved = read(nbt, kind);
    let view = MachineView::unpack(data).unwrap_or_default();
    let extra = FactoryView::unpack(data, menu::MACHINE_SLOTS);
    let live = LIVE.with(|l| l.borrow().get(&live_key(game.native_dimension(), pos)).cloned()).unwrap_or_default();
    let names = |sides: &[DataType; 6]| -> serde_json::Map<String, Value> {
        RelativeSide::ALL.iter().map(|s| (s.name().to_owned(), sides[s.index()].name().into())).collect()
    };
    let layout = kind.layout();
    let slots: Vec<Value> = items
        .iter()
        .take(layout.count)
        .map(|s| s.as_ref().map_or(Value::Null, |s| serde_json::json!({ "item": &*s.item, "count": s.count })))
        .collect();
    let (usage, storage) = config.machines.of(kind.base());
    serde_json::json!({
        "pos": pos,
        "id": id,
        "processes": kind.processes(),
        "energy": saved.energy,
        "capacity": store(&saved, &config, kind).capacity,
        "per_tick": view.per_tick,
        "progress": saved.progress,
        "required": view.required,
        "active": saved.activity.current,
        "shown_active": game.native_block_property(pos, "active").as_deref() == Some("true"),
        "facing": saved.facing.name(),
        "sorting": saved.sorting,
        "last_usage": extra.map(|e| e.last_usage),
        "recipes": live.recipes.iter().map(|r| r.as_ref().map(|(_, r)| r.id.clone())).collect::<Vec<_>>(),
        "upgrades": Upgrade::ALL.iter().filter(|u| saved.upgrades.get(**u) > 0).map(|u| (u.name().to_owned(), saved.upgrades.get(*u).into())).collect::<serde_json::Map<String, Value>>(),
        "upgrade_ticks": live.upgrade.ticks,
        "energy_config": names(&saved.sides.energy),
        "item_config": names(&saved.sides.items),
        "item_eject": saved.sides.items_eject,
        "slots": slots,
        "layout": { "inputs": layout.inputs, "outputs": layout.outputs, "secondaries": layout.secondaries, "extra": layout.extra, "energy": layout.energy, "upgrade": layout.upgrade },
        "warnings": view.warnings,
        "config": { "from_files": from_files, "usage": usage, "storage": storage, "max_upgrade_multiplier": config.machines.max_upgrade_multiplier },
        "recipe_count": BOOK.with(|b| b.borrow().recipes.get(&kind.base()).map_or(0, Vec::len)),
    })
}
