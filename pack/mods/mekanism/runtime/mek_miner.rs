// Mekanism's Digital Miner in the game (support module `crate::game::mek_miner` of the `mekanism` native bundle).
// Its rules are the bundle's crate (`mekanism_energy::miner`, MIT, from Mekanism 10.7.19, Copyright (c) 2017-2025
// Aidan C. Brady); this module is the glue: its block entity in Mekanism's saved form (`radius`, `min`, `max`,
// `silk_touch`, `inverse`, `replace_target`, `inverse_replace`, `eject`, `pull`, `filters`, `running`, `delay`,
// `state`, `control_type`, `overflow`, `energy_containers`, `component_upgrade`), its tick (the upgrade component,
// the energy slot, its search when it starts and when it is loaded again, the mining with the machines' breaking
// helper, the replace block, the overflow, the ejection behind its back, its redstone control, the anchor upgrade's
// chunk tickets), its Joules from cables, cubes and Forge Energy through its bottom and the left and right of its
// bounding blocks, its screen (`GuiDigitalMiner` and, on a second page, `GuiDigitalMinerConfig`), `/mek miner` and
// `/mek probe`.

use super::{
    Game, mek_machines,
    mek_ui::{self, Pen, Rect},
    mek_world::{self, key},
    native_hooks::{NativeFeInfo, NativeScreen},
};
use horizon_client::{
    containers::{ModBlockEntity, ModContainer},
    hud::HudBatch,
    items::ItemStack,
};
use horizon_core::geometry::Direction as Face;
use horizon_world::{
    level::flags,
    nbt::{Compound, Tag},
    player_inventory::EquipmentSlot,
};
use mekanism_energy::{
    config::EnergyConfig,
    container::{Automation, EnergyContainer},
    display,
    fe::{self, Conversion, StrictEnergy},
    machine::{Upgrade, UpgradeSlot, Upgrades},
    miner::{self, ChunkMap, Filter, FilterKind, MinerWorld, Queue, SearchState, Settings},
    sides::{Direction, RelativeSide},
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

pub(super) const MINER: &str = "mekanism:digital_miner";
/// The breaker it mines as: a charged Atomic Disassembler (`ItemAtomicDisassembler.fullyChargedStack`).
const TOOL: &str = "mekanism:atomic_disassembler";
/// Blocks whose `getDestroySpeed` is below zero in vanilla 26.3: the search leaves them.
const UNBREAKABLE: [&str; 16] = [
    "minecraft:bedrock",
    "minecraft:barrier",
    "minecraft:light",
    "minecraft:command_block",
    "minecraft:chain_command_block",
    "minecraft:repeating_command_block",
    "minecraft:structure_block",
    "minecraft:jigsaw",
    "minecraft:end_portal",
    "minecraft:end_portal_frame",
    "minecraft:end_gateway",
    "minecraft:nether_portal",
    "minecraft:moving_piston",
    "minecraft:reinforced_deepslate",
    "minecraft:test_block",
    "minecraft:test_instance_block",
];

/// What a miner keeps between ticks and doesn't save: its queue (Mekanism keeps none either: a loaded miner that
/// had finished its search searches again), the search's answer for the next tick, its upgrade slot's progress, its
/// ejection delay, the replace block it misses, its redstone of the last tick and the chunks it holds.
#[derive(Clone, Debug, Default)]
struct Live {
    queue: Queue,
    init_calc: bool,
    pending: Option<(ChunkMap, i32)>,
    upgrade: UpgradeSlot,
    upgrade_stack: Option<(String, u32)>,
    eject_delay: u32,
    missing: Option<String>,
    powered_last: bool,
    held: Vec<[i32; 2]>,
    last_mined: Option<[i32; 3]>,
}

thread_local! {
    static LIVE: RefCell<HashMap<(String, [i32; 3]), Live>> = RefCell::new(HashMap::new());
    static PAGE: Cell<u8> = const { Cell::new(0) };
    static RELEASE: RefCell<Vec<(String, [i32; 3], Vec<[i32; 2]>)>> = const { RefCell::new(Vec::new()) };
}

pub(super) fn is_miner(block: &str) -> bool {
    block == MINER
}

/// Its bounding blocks (`HandleBoundingBlock`): the 3x3x2 around it but itself, as Mekanism lists them.
pub(super) fn bounding() -> Vec<[i32; 3]> {
    let mut out = Vec::with_capacity(17);
    for x in -1..=1 {
        for y in 0..=1 {
            for z in -1..=1 {
                if [x, y, z] != [0, 0, 0] {
                    out.push([x, y, z]);
                }
            }
        }
    }
    out
}

/// `native_block_entity_unload`: its live state goes, and the chunks it held are let go at the end of the tick.
pub(super) fn forget(dimension: &str, pos: [i32; 3]) {
    let live = LIVE.with(|l| l.borrow_mut().remove(&(dimension.to_owned(), pos)));
    let held = live.map(|l| l.held).unwrap_or_default();
    if !held.is_empty() {
        RELEASE.with(|r| r.borrow_mut().push((dimension.to_owned(), pos, held)));
    }
}

/// `native_level_tick_post`: lets go of the chunks of the miners that went.
pub(super) fn level_ticked(game: &mut Game<'_>, dimension: &str) {
    let due: Vec<_> = RELEASE.with(|r| {
        let mut r = r.borrow_mut();
        let (mine, rest): (Vec<_>, Vec<_>) = r.drain(..).partition(|(d, _, _)| d == dimension);
        *r = rest;
        mine
    });
    for (_, pos, held) in due {
        for chunk in held {
            game.native_force_chunk(&owner(pos), chunk, false);
        }
    }
}

pub(super) fn clear() {
    LIVE.with(|l| l.borrow_mut().clear());
    RELEASE.with(|r| r.borrow_mut().clear());
}

fn owner(pos: [i32; 3]) -> String {
    format!("mekanism:digital_miner/{},{},{}", pos[0], pos[1], pos[2])
}

fn add(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

// ---- The saved compound ---------------------------------------------------------------------

fn flag(nbt: &Compound, name: &str) -> Option<bool> {
    nbt.get(&key(name)).map(|t| mek_world::number(Some(t)).unwrap_or(0) != 0)
}

fn int(nbt: &Compound, name: &str) -> Option<i32> {
    mek_world::number(nbt.get(&key(name))).map(|v| v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32)
}

fn string(nbt: &Compound, name: &str) -> Option<String> {
    match nbt.get(&key(name)) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    }
}

/// A registry entry that isn't air.
fn item_entry(nbt: &Compound, name: &str) -> Option<String> {
    string(nbt, name).filter(|s| !s.is_empty() && !miner::is_air(s))
}

fn read_filter(c: &Compound) -> Option<Filter> {
    let kind = match string(c, "type")?.as_str() {
        "miner_itemstack_filter" => match c.get(&key("target_stack")) {
            Some(Tag::Compound(stack)) => FilterKind::Item(string(stack, "id")?),
            _ => return None,
        },
        "miner_tag_filter" => FilterKind::Tag(string(c, "tag")?),
        "miner_modid_filter" => FilterKind::ModId(string(c, "modid")?),
        _ => return None,
    };
    Some(Filter {
        kind,
        enabled: flag(c, "enabled").unwrap_or(true),
        replace_target: item_entry(c, "replace_target"),
        requires_replacement: flag(c, "requires_replacement").unwrap_or(false),
    })
}

fn write_filter(f: &Filter) -> Tag {
    let mut c = Compound::new();
    c.insert(key("type"), Tag::String(key(f.kind.type_name())));
    if !f.enabled {
        c.insert(key("enabled"), Tag::Byte(0));
    }
    if let Some(t) = &f.replace_target {
        c.insert(key("replace_target"), Tag::String(key(t)));
    }
    if f.requires_replacement {
        c.insert(key("requires_replacement"), Tag::Byte(1));
    }
    match &f.kind {
        FilterKind::Item(item) => {
            let mut stack = Compound::new();
            stack.insert(key("id"), Tag::String(key(item)));
            c.insert(key("target_stack"), Tag::Compound(stack));
        }
        FilterKind::Tag(tag) => {
            c.insert(key("tag"), Tag::String(key(tag)));
        }
        FilterKind::ModId(id) => {
            c.insert(key("modid"), Tag::String(key(id)));
        }
    }
    Tag::Compound(c)
}

/// `loadAdditional` with `readSustainedData`: the radius up to `maxRadius`, the range within the level.
fn read(nbt: &Compound, max_radius: i32) -> Settings {
    let d = Settings::default();
    let filters = match nbt.get(&key("filters")) {
        Some(Tag::List(list)) => list.iter().filter_map(|t| if let Tag::Compound(c) = t { read_filter(c) } else { None }).collect(),
        _ => Vec::new(),
    };
    let min_y = int(nbt, "min").unwrap_or(d.min_y).max(-64);
    Settings {
        radius: int(nbt, "radius").unwrap_or(d.radius).clamp(0, max_radius),
        min_y,
        max_y: int(nbt, "max").unwrap_or(d.max_y).min(319),
        silk_touch: flag(nbt, "silk_touch").unwrap_or(false),
        inverse: flag(nbt, "inverse").unwrap_or(false),
        inverse_requires_replacement: flag(nbt, "inverse_replace").unwrap_or(false),
        inverse_replace_target: item_entry(nbt, "replace_target"),
        eject: flag(nbt, "eject").unwrap_or(false),
        pull: flag(nbt, "pull").unwrap_or(false),
        filters,
        running: flag(nbt, "running").unwrap_or(false),
        delay: int(nbt, "delay").unwrap_or(0).max(0) as u32,
        state: SearchState::by_id(int(nbt, "state").unwrap_or(0)),
    }
}

/// `saveAdditional` with `writeSustainedData`.
fn write(nbt: &mut Compound, s: &Settings) {
    nbt.insert(key("radius"), Tag::Int(s.radius));
    nbt.insert(key("min"), Tag::Int(s.min_y));
    nbt.insert(key("max"), Tag::Int(s.max_y));
    for (name, v) in [("eject", s.eject), ("pull", s.pull), ("silk_touch", s.silk_touch), ("inverse", s.inverse), ("inverse_replace", s.inverse_requires_replacement), ("running", s.running)] {
        nbt.insert(key(name), Tag::Byte(i8::from(v)));
    }
    match &s.inverse_replace_target {
        Some(t) => nbt.insert(key("replace_target"), Tag::String(key(t))),
        None => nbt.remove(&key("replace_target")),
    };
    if s.filters.is_empty() {
        nbt.remove(&key("filters"));
    } else {
        nbt.insert(key("filters"), Tag::List(s.filters.iter().map(write_filter).collect()));
    }
    nbt.insert(key("delay"), Tag::Int(s.delay as i32));
    nbt.insert(key("state"), Tag::Int(s.state.ordinal()));
    if !nbt.contains_key(&key("num_powering")) {
        nbt.insert(key("num_powering"), Tag::Int(0));
    }
}

/// `IRedstoneControl.RedstoneControl`: DISABLED, HIGH, LOW, PULSE.
fn control(nbt: &Compound) -> i32 {
    int(nbt, "control_type").unwrap_or(0).rem_euclid(4)
}

/// The overflow (`OverflowAware`): what didn't fit, kept until it does.
fn read_overflow(nbt: &Compound) -> Vec<ItemStack> {
    let Some(Tag::List(list)) = nbt.get(&key("overflow")) else { return Vec::new() };
    list.iter()
        .filter_map(|t| match t {
            Tag::Compound(c) => {
                let item = string(c, "id")?;
                let count = int(c, "count").unwrap_or(0);
                (count > 0).then(|| ItemStack::new(&item, count as u32))
            }
            _ => None,
        })
        .collect()
}

fn write_overflow(nbt: &mut Compound, overflow: &[ItemStack]) {
    if overflow.is_empty() {
        nbt.remove(&key("overflow"));
        return;
    }
    let list = overflow
        .iter()
        .map(|s| {
            let mut c = Compound::new();
            c.insert(key("id"), Tag::String(key(&s.item)));
            c.insert(key("count"), Tag::Int(s.count as i32));
            Tag::Compound(c)
        })
        .collect();
    nbt.insert(key("overflow"), Tag::List(list));
}

fn store(nbt: &Compound, config: &EnergyConfig, upgrades: &Upgrades) -> EnergyContainer {
    let mut container = EnergyContainer::basic(upgrades.max_energy(config.miner.storage, config.machines.max_upgrade_multiplier));
    container.stored = mek_world::saved_energy(nbt).unwrap_or(0).min(container.capacity);
    container
}

fn facing(nbt: &Compound) -> Direction {
    mek_world::generator_facing(nbt)
}

// ---- Block entities -------------------------------------------------------------------------

/// `native_block_entity`: a fresh miner facing north, its range 0 to 60 (`minY + 60`), radius 10.
pub(super) fn new_entity() -> (String, Compound) {
    let mut nbt = Compound::new();
    nbt.insert(key("facing"), Tag::String(key(Direction::North.name())));
    write(&mut nbt, &Settings::default());
    (MINER.to_owned(), nbt)
}

/// `native_block_entity_container`: the 27 main slots, the energy slot and the upgrade slots in a 9x4 menu whose last
/// row the screen leaves out. Hoppers reach the main slots from every face of the miner itself (Mekanism: through
/// its bounding blocks' top and back-top).
pub(super) fn container() -> ModContainer {
    let main: Vec<usize> = (0..miner::MAIN_SLOTS).collect();
    ModContainer {
        slots: 36,
        faces: std::array::from_fn(|_| main.clone()),
        inputs: main.clone(),
        outputs: main,
        menu: Some("minecraft:generic_9x4".into()),
    }
}

/// `native_block_entity_slot_accepts`: the energy slot energy items, the upgrade slot the four upgrades it takes.
pub(super) fn slot_accepts(slot: usize, stack: &ItemStack, accepts: &mut bool) {
    let ok = match slot {
        s if s < miner::MAIN_SLOTS => true,
        miner::ENERGY_SLOT => mek_machines::energy_item(&stack.item) || mekanism_energy::tier::CubeTier::of_block(&stack.item).is_some(),
        miner::UPGRADE_SLOT => Upgrade::of_item(&stack.item).is_some_and(|u| miner::UPGRADES.contains(&u)),
        _ => false,
    };
    if !ok {
        *accepts = false;
    }
}

// ---- The world, as the rules read it --------------------------------------------------------

struct View<'a, 'g> {
    game: &'a Game<'g>,
}

impl MinerWorld for View<'_, '_> {
    fn block(&mut self, pos: [i32; 3]) -> Option<String> {
        Some(self.game.native_block(pos))
    }

    fn skipped(&mut self, pos: [i32; 3], block: &str) -> bool {
        block == super::mek_gen2::BOUNDING
            || self.blacklisted(block)
            || UNBREAKABLE.contains(&block)
            || matches!(block, "minecraft:water" | "minecraft:lava" | "minecraft:bubble_column")
            || horizon_world::mod_fluids::kind_by_block(block).is_some()
            || self.game.native_block_property(pos, "part").as_deref() == Some("foot") && block.ends_with("_bed")
            || self.game.native_block_property(pos, "half").as_deref() == Some("upper")
    }

    fn blacklisted(&mut self, block: &str) -> bool {
        self.game.native_block_name_in_tag(block, "mekanism:miner_blacklist").unwrap_or(false)
    }

    fn item_of(&self, block: &str) -> Option<String> {
        (!miner::is_air(block)).then(|| block.to_owned())
    }

    fn block_tag_matches(&mut self, block: &str, tag: &str) -> bool {
        !miner::is_pattern(tag) && self.game.native_block_name_in_tag(block, tag).unwrap_or(false)
    }
}

// ---- The tick -------------------------------------------------------------------------------

/// `InventoryUtils.insertItem` into the main slots: stacks of the same item first, then empty slots. What is left.
fn insert(game: &Game<'_>, entity: &mut ModBlockEntity, mut stack: ItemStack, execute: bool) -> ItemStack {
    let max = game.native_max_stack_size(&stack.item).unwrap_or(64).clamp(1, 64);
    for pass in 0..2 {
        for slot in 0..miner::MAIN_SLOTS {
            if stack.count == 0 {
                return stack;
            }
            let held = entity.items.get(slot).cloned().flatten();
            let room = match &held {
                Some(h) if pass == 0 && mek_machines::same_kind(h, &stack) => max.saturating_sub(h.count),
                None if pass == 1 => max,
                _ => 0,
            };
            let moved = room.min(stack.count);
            if moved == 0 {
                continue;
            }
            if execute && let Some(cell) = entity.items.get_mut(slot) {
                *cell = Some(match held {
                    Some(h) => ItemStack { count: h.count + moved, ..h },
                    None => ItemStack { count: moved, ..stack.clone() },
                });
            }
            stack.count -= moved;
        }
    }
    stack
}

/// `getReplace`: one of the replace target out of the main slots, else made by a stone generator upgrade (stone or
/// cobblestone), else pulled from the inventory two above it (`doPull`).
fn take_replace(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, target: &str, upgrades: &Upgrades, pull: bool) -> bool {
    if let Some(slot) = (0..miner::MAIN_SLOTS).find(|s| mek_machines::stack_in(entity, *s).is_some_and(|st| *st.item == *target)) {
        mek_machines::shrink(entity, slot, 1);
        return true;
    }
    if matches!(target, "minecraft:cobblestone" | "minecraft:stone") && upgrades.get(Upgrade::StoneGenerator) > 0 {
        return true;
    }
    if pull {
        let dimension = game.native_dimension();
        let above = add(pos, [0, 2, 0]);
        let slots = game.native_container_slots(dimension, above, Face::Down).unwrap_or_default();
        if let Some(slot) = slots.iter().find(|s| s.can_take && s.stack.as_ref().is_some_and(|st| *st.item == *target)) {
            return game.native_extract_item(dimension, above, Face::Down, slot.slot, 1).is_some_and(|got| got.count > 0);
        }
    }
    false
}

/// What one mine did.
enum Mine {
    Done,
    Missing(String),
    Nothing,
}

/// `tryMineBlock`: the next block the queue gives, broken with the miner's tool (silk touch with its mode), its replace
/// block set where it stood, the drops into the main slots and what doesn't fit into the overflow.
fn mine(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, s: &Settings, live: &mut Live, upgrades: &Upgrades, overflow: &mut Vec<ItemStack>) -> Mine {
    let found = {
        let mut view = View { game: &*game };
        live.queue.next(s, pos, &mut view)
    };
    let Some(found) = found else { return Mine::Nothing };
    let (target, required) = s.replace_for(found.filter);
    let target = target.map(str::to_owned);
    let replaced = target.as_deref().is_some_and(|t| take_replace(game, pos, entity, t, upgrades, s.pull));
    if !replaced
        && let Some(t) = &target
        && (found.filter.is_some() || s.inverse_requires_replacement)
        && required
    {
        return Mine::Missing(t.clone());
    }
    let mut tool = ItemStack::new(TOOL, 1);
    if s.silk_touch {
        tool.enchant("minecraft:silk_touch", 1);
    }
    let Some(drops) = game.native_machine_destroy_block(found.pos, Some(&tool), true) else { return Mine::Nothing };
    if replaced
        && let Some(state) = target.as_deref().and_then(|t| game.native_default_state(t))
    {
        let dimension = game.native_dimension().to_owned();
        game.native_set_block(&dimension, found.pos, state, flags::UPDATE_ALL);
    }
    for stack in drops {
        let rest = insert(game, entity, stack, true);
        if rest.count > 0 {
            overflow.push(rest);
        }
    }
    live.queue.mined(&found);
    live.last_mined = Some(found.pos);
    Mine::Done
}

/// `native_block_entity_tick`: `onUpdateServer`.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let (config, _) = mek_world::energy_config();
    mek_machines::refresh_book(game);
    let dimension = game.native_dimension().to_owned();
    let face = game.native_block_property(pos, "facing").and_then(|f| Direction::from_name(&f)).unwrap_or(Direction::North);
    if facing(&entity.nbt) != face || !entity.nbt.contains_key(&key("facing")) {
        entity.nbt.insert(key("facing"), Tag::String(key(face.name())));
    }
    let id = (dimension.clone(), pos);
    let mut live = LIVE.with(|l| l.borrow_mut().remove(&id)).unwrap_or_default();
    let mut activity = mek_world::read_activity(&entity.nbt);
    let mut shown = game.native_block_property(pos, "active").as_deref() == Some("true");
    let changed = activity.tick(shown);
    mek_world::show_active(game, pos, changed, &mut shown);

    // The upgrade component.
    let before_upgrades = super::mek_pump::read_upgrades(&entity.nbt);
    let mut upgrades = before_upgrades;
    let upgrade_stack = mek_machines::stack_in(entity, miner::UPGRADE_SLOT).map(|s| (s.item.to_string(), s.count));
    if upgrade_stack != live.upgrade_stack {
        live.upgrade.check = true;
    }
    let offered = upgrade_stack.as_ref().and_then(|(item, count)| Some((Upgrade::of_item(item)?, *count)));
    let taken = live.upgrade.tick(&mut upgrades, &miner::UPGRADES, offered);
    if taken > 0 {
        mek_machines::shrink(entity, miner::UPGRADE_SLOT, taken);
    }
    live.upgrade_stack = mek_machines::stack_in(entity, miner::UPGRADE_SLOT).map(|s| (s.item.to_string(), s.count));
    if upgrades != before_upgrades || !entity.nbt.contains_key(&key("component_upgrade")) {
        super::mek_pump::write_upgrades(&mut entity.nbt, &upgrades);
    }

    let before = read(&entity.nbt, config.miner.max_radius);
    let mut s = before.clone();
    let multiplier = config.machines.max_upgrade_multiplier;
    // The search runs in a thread in Mekanism, done by the next tick for any area this size: its answer comes in here.
    if s.state == SearchState::Searching
        && let Some((map, found)) = live.pending.take()
    {
        live.queue = Queue { map, to_mine: found, target_chunk: None };
        s.state = SearchState::Finished;
    }
    if !live.init_calc {
        // Saved while searching, it loads as finished (`loadAdditional`); finished, it searches again and runs on.
        if s.state == SearchState::Searching {
            s.state = SearchState::Finished;
        }
        if s.state == SearchState::Finished {
            let running = s.running;
            reset(&mut s, &mut live);
            start(game, pos, &mut s, &mut live);
            s.running = running;
        }
        live.init_calc = true;
    }

    let mut store = store(&entity.nbt, &config, &upgrades);
    let energy_before = store.stored;
    let mut item = mek_machines::stack_in(entity, miner::ENERGY_SLOT).cloned();
    if let Some(cube) = item.as_mut()
        && store.needed() > 0
        && let Some(got) = mek_world::drain_energy_item(cube, store.needed()).filter(|t| *t > 0)
    {
        store.insert(got, false, Automation::Internal);
        if let Some(cell) = entity.items.get_mut(miner::ENERGY_SLOT) {
            *cell = item;
        }
    } else if let Some(stack) = mek_machines::stack_in(entity, miner::ENERGY_SLOT).cloned()
        && store.needed() > 0
        && let Some((count, joules)) = mek_machines::energy_conversion(&stack)
        && stack.count >= count
        && store.needed() >= joules
    {
        store.insert(joules, false, Automation::Manual);
        mek_machines::shrink(entity, miner::ENERGY_SLOT, count);
    }

    // The overflow goes back in first.
    let mut overflow = read_overflow(&entity.nbt);
    let before_overflow = overflow.clone();
    overflow = overflow.into_iter().map(|st| insert(game, entity, st, true)).filter(|st| st.count > 0).collect();

    // Redstone: the miner's own signal or a bounding block's (`numPowering`).
    let powered = game.native_neighbor_signal(&dimension, pos) > 0
        || bounding().into_iter().any(|o| game.native_neighbor_signal(&dimension, add(pos, o)) > 0);
    let can_function = match control(&entity.nbt) {
        1 => powered,
        2 => !powered,
        3 => powered && !live.powered_last,
        _ => true,
    };
    let per_tick = config.miner.energy_per_tick(&upgrades, multiplier, &s, 384);
    let mut used = 0;
    let mut active = false;
    if overflow.is_empty() && can_function && s.running && s.state == SearchState::Finished && !live.queue.map.is_empty() {
        if store.extract(per_tick, true, Automation::Internal) == per_tick {
            active = true;
            s.delay = s.delay.saturating_sub(1);
            used = store.extract(per_tick, false, Automation::Internal);
            if s.delay == 0 {
                match mine(game, pos, entity, &s, &mut live, &upgrades, &mut overflow) {
                    Mine::Done => live.missing = None,
                    Mine::Missing(item) => live.missing = Some(item),
                    Mine::Nothing => {}
                }
                s.delay = config.miner.ticks(&upgrades, multiplier);
            }
        }
    }
    live.powered_last = powered;
    let changed = activity.set(active, shown, config.deactivation_delay);
    mek_world::show_active(game, pos, changed, &mut shown);

    // Its ejection: every 11 ticks one kind of item of its main slots into the block behind its back-top bounding block.
    if s.eject && live.eject_delay == 0 {
        eject(game, pos, entity, face);
        live.eject_delay = miner::EJECT_DELAY;
    } else if live.eject_delay > 0 {
        live.eject_delay -= 1;
    }

    // The anchor upgrade holds its own chunk and the one it works in.
    let want = if upgrades.get(Upgrade::Anchor) > 0 { live.queue.chunks(&s, pos) } else { Vec::new() };
    if want != live.held {
        let name = owner(pos);
        for c in live.held.iter().filter(|c| !want.contains(c)) {
            game.native_force_chunk(&name, *c, false);
        }
        for c in want.iter().filter(|c| !live.held.contains(c)) {
            game.native_force_chunk(&name, *c, true);
        }
        live.held = want;
    }

    if s != before {
        write(&mut entity.nbt, &s);
    }
    if overflow != before_overflow {
        write_overflow(&mut entity.nbt, &overflow);
    }
    if store.stored != energy_before || !entity.nbt.contains_key(&key("energy_containers")) {
        mek_world::write_energy(&mut entity.nbt, store.stored);
    }
    if activity != mek_world::read_activity(&entity.nbt) || !entity.nbt.contains_key(&key("active_state")) {
        mek_world::write_activity(&mut entity.nbt, activity);
    }
    entity.data = pack(&s, &live, store.stored, store.capacity, per_tick, used, control(&entity.nbt), powered, !overflow.is_empty());
    LIVE.with(|l| l.borrow_mut().insert(id, live));
}

/// `start`: an idle miner searches (its answer by the next tick); it runs.
fn start(game: &Game<'_>, pos: [i32; 3], s: &mut Settings, live: &mut Live) {
    if s.state == SearchState::Idle {
        let mut view = View { game };
        let result = s.search(pos, &mut view);
        if !s.inverse && !s.has_enabled_filters() {
            s.state = SearchState::Finished;
        } else {
            live.pending = Some(result);
            s.state = SearchState::Searching;
        }
    }
    s.running = true;
}

/// `stop`: a search is called off (and the miner reset), a finished one pauses.
fn stop(s: &mut Settings, live: &mut Live) {
    match s.state {
        SearchState::Searching => reset(s, live),
        SearchState::Finished => {
            s.running = false;
            live.queue.target_chunk = None;
        }
        _ => {}
    }
}

/// `reset`: back to idle with nothing to mine.
fn reset(s: &mut Settings, live: &mut Live) {
    s.state = SearchState::Idle;
    s.running = false;
    live.pending = None;
    live.queue = Queue::default();
    live.missing = None;
}

/// `InventoryUtils.getEjectItemMap` and `TransitRequest.eject`: the first kind of item (Mekanism takes the slots in a
/// shuffled order; here in slot order) that the target takes any of goes, as much as fits.
fn eject(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, face: Direction) {
    let back = face.opposite();
    let from = add(add(pos, [0, 1, 0]), back.offset());
    let at = add(from, back.offset());
    let mut kinds: Vec<ItemStack> = Vec::new();
    for slot in 0..miner::MAIN_SLOTS {
        if let Some(st) = mek_machines::stack_in(entity, slot) {
            match kinds.iter_mut().find(|k| mek_machines::same_kind(k, st)) {
                Some(k) => k.count += st.count,
                None => kinds.push(st.clone()),
            }
        }
    }
    for kind in kinds {
        let total = kind.count;
        let rest = match super::mek_transporters::insert_from(game, at, from, kind.clone()) {
            Some(rest) => Some(rest),
            None => game.native_insert_item(at, mek_world::engine_face(face), kind.clone()),
        };
        let sent = total - rest.map_or(total, |r| r.count);
        if sent == 0 {
            continue;
        }
        let mut left = sent;
        for slot in 0..miner::MAIN_SLOTS {
            if left == 0 {
                break;
            }
            if let Some(st) = mek_machines::stack_in(entity, slot)
                && mek_machines::same_kind(st, &kind)
            {
                let by = st.count.min(left);
                mek_machines::shrink(entity, slot, by);
                left -= by;
            }
        }
        return;
    }
}

// ---- Energy from outside --------------------------------------------------------------------

/// Whether the miner in `entity` takes energy through its own face `face`: its bottom.
pub(super) fn takes_energy(entity: &ModBlockEntity, face: Direction) -> Option<bool> {
    is_miner(&entity.id).then_some(face == Direction::Down)
}

/// The miner whose energy port the bounding block at `at` is, touched through its side `side`: the bounding blocks
/// left and right of it at its height, on their outer side (`notEnergyPort`).
pub(super) fn port_owner(game: &Game<'_>, at: [i32; 3], side: Direction) -> Option<[i32; 3]> {
    if game.native_block(at) != super::mek_gen2::BOUNDING {
        return None;
    }
    for d in [Direction::North, Direction::South, Direction::West, Direction::East] {
        let main = add(at, d.opposite().offset());
        if game.native_block(main) != MINER {
            continue;
        }
        let face = game.native_block_property(main, "facing").and_then(|f| Direction::from_name(&f)).unwrap_or(Direction::North);
        let left = RelativeSide::Left.direction(face);
        if (d == left || d == left.opposite()) && side == d {
            return Some(main);
        }
    }
    None
}

/// Offers `amount` Joules through `face` (any of its ports): what it didn't take.
pub(super) fn offer(entity: &mut ModBlockEntity, config: &EnergyConfig, amount: u64, simulate: bool) -> Option<u64> {
    if !is_miner(&entity.id) {
        return None;
    }
    let upgrades = super::mek_pump::read_upgrades(&entity.nbt);
    let mut container = store(&entity.nbt, config, &upgrades);
    let before = container.stored;
    let rest = container.insert(amount, simulate, Automation::External);
    if !simulate && container.stored != before {
        mek_world::write_energy(&mut entity.nbt, container.stored);
    }
    Some(rest)
}

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

/// `native_fe_storage`: its store in whole FE through its bottom.
pub(super) fn fe_storage(entity: &ModBlockEntity, face: Option<Face>, config: &EnergyConfig, info: &mut Option<NativeFeInfo>) -> bool {
    if !is_miner(&entity.id) {
        return false;
    }
    if config.blacklist_forge || face.is_some_and(|f| mek_world::mek_direction(f) != Direction::Down) {
        return true;
    }
    let c = store(&entity.nbt, config, &super::mek_pump::read_upgrades(&entity.nbt));
    let rate = Conversion(config.fe_conversion_rate);
    *info = Some(NativeFeInfo {
        stored: i64::from(rate.to_fe_int(c.stored.min(i64::MAX as u64) as i64)),
        capacity: i64::from(rate.to_fe_int(c.capacity.min(i64::MAX as u64) as i64)),
        can_receive: true,
        can_extract: true,
    });
    true
}

/// `native_fe_receive` and `native_fe_extract`.
#[allow(clippy::too_many_arguments)]
pub(super) fn fe_transfer(pos: [i32; 3], entity: &mut ModBlockEntity, face: Option<Face>, config: &EnergyConfig, max: i64, simulate: bool, extract: bool, moved: &mut Option<i64>) -> bool {
    if !is_miner(&entity.id) {
        return false;
    }
    let open = face.is_some_and(|f| mek_world::mek_direction(f) == Direction::Down);
    if config.blacklist_forge || face.is_some() && !open {
        return true;
    }
    let container = store(&entity.nbt, config, &super::mek_pump::read_upgrades(&entity.nbt));
    let before = container.stored;
    let mut side = Side { container, open, taken: 0 };
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

// ---- Buttons and /mek miner -------------------------------------------------------------------

const START: i32 = 0;
const STOP: i32 = 1;
const RESET: i32 = 3;
const EJECT: i32 = 4;
const PULL: i32 = 5;
const SILK: i32 = 6;
const REDSTONE: i32 = 7;
const INVERSE: i32 = 8;
const RADIUS_DOWN: i32 = 9;
const RADIUS_UP: i32 = 10;
const MIN_DOWN: i32 = 11;
const MIN_UP: i32 = 12;
const MAX_DOWN: i32 = 13;
const MAX_UP: i32 = 14;
const INVERSE_REPLACE: i32 = 15;
const ADD_HELD: i32 = 16;
/// 20 + n toggles filter n, 40 + n deletes it.
const FILTER_TOGGLE: i32 = 20;
const FILTER_DELETE: i32 = 40;

/// Runs one of the screen's actions on the miner in `entity`; the configuration changes only while it is idle (its
/// config screen opens only then).
fn act(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, id: i32, held: Option<String>) {
    let (config, _) = mek_world::energy_config();
    let dimension = game.native_dimension().to_owned();
    let mut live = LIVE.with(|l| l.borrow_mut().remove(&(dimension.clone(), pos))).unwrap_or_default();
    let mut s = read(&entity.nbt, config.miner.max_radius);
    let idle = s.state == SearchState::Idle;
    let step = |v: i32, by: i32| v.saturating_add(by);
    match id {
        START => start(game, pos, &mut s, &mut live),
        STOP => stop(&mut s, &mut live),
        RESET => reset(&mut s, &mut live),
        EJECT => s.eject = !s.eject,
        PULL => s.pull = !s.pull,
        SILK => s.silk_touch = !s.silk_touch,
        REDSTONE => {
            let next = (control(&entity.nbt) + 1) % 4;
            entity.nbt.insert(key("control_type"), Tag::Int(next));
        }
        INVERSE if idle => s.inverse = !s.inverse,
        INVERSE_REPLACE if idle => s.inverse_requires_replacement = !s.inverse_requires_replacement,
        RADIUS_DOWN | RADIUS_UP if idle => s.radius = step(s.radius, if id == RADIUS_UP { 1 } else { -1 }).clamp(0, config.miner.max_radius),
        MIN_DOWN | MIN_UP if idle => s.min_y = step(s.min_y, if id == MIN_UP { 1 } else { -1 }).clamp(-64, s.max_y),
        MAX_DOWN | MAX_UP if idle => s.max_y = step(s.max_y, if id == MAX_UP { 1 } else { -1 }).clamp(s.min_y, 319),
        ADD_HELD if idle => {
            if let Some(item) = held {
                s.filters.push(Filter::new(FilterKind::Item(item)));
            }
        }
        n if idle && (FILTER_TOGGLE..FILTER_DELETE).contains(&n) => {
            if let Some(f) = s.filters.get_mut((n - FILTER_TOGGLE) as usize) {
                f.enabled = !f.enabled;
            }
        }
        n if idle && (FILTER_DELETE..FILTER_DELETE + 20).contains(&n) => {
            let i = (n - FILTER_DELETE) as usize;
            if i < s.filters.len() {
                s.filters.remove(i);
            }
        }
        _ => {}
    }
    write(&mut entity.nbt, &s);
    LIVE.with(|l| l.borrow_mut().insert((dimension, pos), live));
}

/// `native_menu_button`.
pub(super) fn button(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, id: i32) {
    let held = if id == ADD_HELD {
        game.native_acting_player()
            .and_then(|p| game.native_player_equipment(p.uuid, EquipmentSlot::MainHand))
            .filter(|s| s.count > 0)
            .map(|s| s.item.to_string())
    } else {
        None
    };
    act(game, pos, entity, id, held);
}

/// `/mek miner x y z <what> ...`: the screens' settings and buttons, and the filters their windows make:
/// `start`, `stop`, `reset`, `radius N`, `min N`, `max N`, `silk|eject|pull|inverse|inverse_replace on|off`,
/// `replace <item>|none`, `redstone disabled|high|low|pulse`, `filter item|tag|modid <target> [replace <item>]
/// [required] [disabled]`, `filters clear`.
pub(super) fn command(game: &mut Game<'_>, pos: [i32; 3], words: &[&str]) -> Value {
    let (config, _) = mek_world::energy_config();
    let words: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
    let done = game.native_with_mod_block_entity(pos, |m| {
        if !is_miner(&m.id) {
            return Err("no digital miner here".to_owned());
        }
        let mut s = read(&m.nbt, config.miner.max_radius);
        let on = |w: Option<&String>| w.map(|w| w == "on" || w == "true");
        let num = |w: Option<&String>| w.and_then(|w| w.parse::<i32>().ok());
        let button = match words.first().map(String::as_str) {
            Some("start") => Some(START),
            Some("stop") => Some(STOP),
            Some("reset") => Some(RESET),
            _ => None,
        };
        if let Some(b) = button {
            return Ok(Some(b));
        }
        if s.state != SearchState::Idle && !matches!(words.first().map(String::as_str), Some("silk" | "eject" | "pull" | "redstone")) {
            return Err("reset the miner first".to_owned());
        }
        match (words.first().map(String::as_str), words.get(1)) {
            (Some("radius"), w) => s.radius = num(w).ok_or("radius N")?.clamp(0, config.miner.max_radius),
            (Some("min"), w) => s.min_y = num(w).ok_or("min N")?.clamp(-64, s.max_y),
            (Some("max"), w) => s.max_y = num(w).ok_or("max N")?.clamp(s.min_y, 319),
            (Some("silk"), w) => s.silk_touch = on(w).ok_or("silk on|off")?,
            (Some("eject"), w) => s.eject = on(w).ok_or("eject on|off")?,
            (Some("pull"), w) => s.pull = on(w).ok_or("pull on|off")?,
            (Some("inverse"), w) => s.inverse = on(w).ok_or("inverse on|off")?,
            (Some("inverse_replace"), w) => s.inverse_requires_replacement = on(w).ok_or("inverse_replace on|off")?,
            (Some("replace"), w) => s.inverse_replace_target = w.filter(|w| *w != "none").cloned(),
            (Some("redstone"), w) => {
                let v = ["disabled", "high", "low", "pulse"].iter().position(|n| Some(*n) == w.map(String::as_str)).ok_or("redstone disabled|high|low|pulse")?;
                m.nbt.insert(key("control_type"), Tag::Int(v as i32));
            }
            (Some("filters"), Some(c)) if c == "clear" => s.filters.clear(),
            (Some("filter"), Some(kind)) => {
                let target = words.get(2).ok_or("filter item|tag|modid <target>")?.clone();
                let mut f = Filter::new(match kind.as_str() {
                    "item" => FilterKind::Item(target),
                    "tag" => FilterKind::Tag(target),
                    "modid" => FilterKind::ModId(target),
                    _ => return Err("filter item|tag|modid <target>".to_owned()),
                });
                let mut rest = words[3..].iter();
                while let Some(w) = rest.next() {
                    match w.as_str() {
                        "replace" => f.replace_target = rest.next().cloned().filter(|r| r != "none"),
                        "required" => f.requires_replacement = true,
                        "disabled" => f.enabled = false,
                        _ => return Err(format!("unknown filter word {w}")),
                    }
                }
                s.filters.push(f);
            }
            _ => return Err("usage: /mek miner <x> <y> <z> start|stop|reset|radius|min|max|silk|eject|pull|inverse|inverse_replace|replace|redstone|filter|filters ...".to_owned()),
        }
        write(&mut m.nbt, &s);
        Ok(None)
    });
    match done {
        None => json!({ "pos": pos, "error": "no mod block entity here" }),
        Some(Err(e)) => json!({ "pos": pos, "error": e }),
        Some(Ok(button)) => {
            if let Some(b) = button
                && let Some(mut m) = game.native_with_mod_block_entity(pos, |m| m.clone())
            {
                act(game, pos, &mut m, b, None);
                let nbt = m.nbt.clone();
                game.native_with_mod_block_entity(pos, move |e| e.nbt = nbt);
            }
            probe_at(game, pos)
        }
    }
}

fn probe_at(game: &mut Game<'_>, pos: [i32; 3]) -> Value {
    match game.native_with_mod_block_entity(pos, |m| (m.nbt.clone(), m.data.clone(), m.items.clone())) {
        Some((nbt, data, items)) => probe(game, pos, &nbt, &data, &items),
        None => json!({ "pos": pos, "error": "no mod block entity here" }),
    }
}

// ---- The screen's data ------------------------------------------------------------------------

fn push(out: &mut Vec<i32>, value: u64, pieces: usize) {
    out.extend((0..pieces).map(|i| ((value >> (16 * i)) & 0xFFFF) as i32));
}

fn read_word(data: &[i32], at: usize, pieces: usize) -> u64 {
    (0..pieces).fold(0, |v, i| v | (u64::from(data.get(at + i).copied().unwrap_or(0) as u16) << (16 * i)))
}

/// A string as its length and its UTF-8 bytes, two to a word.
fn push_str(out: &mut Vec<i32>, s: &str) {
    let bytes = &s.as_bytes()[..s.len().min(64)];
    out.push(bytes.len() as i32);
    out.extend(bytes.chunks(2).map(|c| i32::from(c[0]) | (i32::from(*c.get(1).unwrap_or(&0)) << 8)));
}

fn read_str(data: &[i32], at: &mut usize) -> String {
    let len = data.get(*at).copied().unwrap_or(0).clamp(0, 64) as usize;
    *at += 1;
    let mut bytes = Vec::with_capacity(len);
    for i in 0..len.div_ceil(2) {
        let w = data.get(*at + i).copied().unwrap_or(0);
        bytes.push((w & 0xFF) as u8);
        bytes.push(((w >> 8) & 0xFF) as u8);
    }
    bytes.truncate(len);
    *at += len.div_ceil(2);
    String::from_utf8_lossy(&bytes).into_owned()
}

#[allow(clippy::too_many_arguments)]
fn pack(s: &Settings, live: &Live, stored: u64, capacity: u64, per_tick: u64, used: u64, control: i32, powered: bool, overflow: bool) -> Vec<i32> {
    let mut d = Vec::with_capacity(64);
    push(&mut d, stored, 4);
    push(&mut d, capacity, 4);
    push(&mut d, per_tick, 4);
    push(&mut d, used, 4);
    d.extend([s.radius, s.min_y, s.max_y, live.queue.to_mine, s.state.ordinal()]);
    d.extend([s.running, s.silk_touch, s.eject, s.pull, s.inverse, s.inverse_requires_replacement, powered, overflow].map(i32::from));
    d.push(control);
    push_str(&mut d, live.missing.as_deref().unwrap_or(""));
    push_str(&mut d, s.inverse_replace_target.as_deref().unwrap_or(""));
    d.push(s.filters.len().min(8) as i32);
    for f in s.filters.iter().take(8) {
        let (kind, target) = match &f.kind {
            FilterKind::Item(t) => (0, t),
            FilterKind::Tag(t) => (1, t),
            FilterKind::ModId(t) => (2, t),
        };
        d.extend([kind, i32::from(f.enabled), i32::from(f.requires_replacement)]);
        push_str(&mut d, target);
        push_str(&mut d, f.replace_target.as_deref().unwrap_or(""));
    }
    d
}

struct ViewData {
    stored: u64,
    capacity: u64,
    per_tick: u64,
    used: u64,
    radius: i32,
    min_y: i32,
    max_y: i32,
    to_mine: i32,
    state: SearchState,
    flags: [bool; 8],
    control: i32,
    missing: String,
    replace: String,
    filters: Vec<(i32, bool, bool, String, String)>,
}

fn unpack(data: &[i32]) -> Option<ViewData> {
    if data.len() < 31 {
        return None;
    }
    let mut at = 30;
    let missing = read_str(data, &mut at);
    let replace = read_str(data, &mut at);
    let count = data.get(at).copied().unwrap_or(0).clamp(0, 8);
    at += 1;
    let mut filters = Vec::new();
    for _ in 0..count {
        let (kind, enabled, required) = (data.get(at).copied().unwrap_or(0), data.get(at + 1) == Some(&1), data.get(at + 2) == Some(&1));
        at += 3;
        let target = read_str(data, &mut at);
        let replace = read_str(data, &mut at);
        filters.push((kind, enabled, required, target, replace));
    }
    Some(ViewData {
        stored: read_word(data, 0, 4),
        capacity: read_word(data, 4, 4),
        per_tick: read_word(data, 8, 4),
        used: read_word(data, 12, 4),
        radius: data[16],
        min_y: data[17],
        max_y: data[18],
        to_mine: data[19],
        state: SearchState::by_id(data[20]),
        flags: std::array::from_fn(|i| data[21 + i] != 0),
        control: data[29],
        missing,
        replace,
        filters,
    })
}

// ---- The screen -------------------------------------------------------------------------------

const WIDTH: f32 = 176.0;
const HEIGHT: f32 = 242.0;
const INNER: Rect = [7.0, 19.0, 77.0, 69.0];
const POWER_BAR: Rect = [157.0, 39.0, 6.0, 49.0];
const BUTTONS: [Rect; 4] = [[87.0, 19.0, 61.0, 18.0], [87.0, 36.0, 61.0, 18.0], [87.0, 53.0, 61.0, 18.0], [87.0, 70.0, 61.0, 18.0]];
const SWITCHES: [Rect; 3] = [[19.0, 56.0, 15.0, 15.0], [38.0, 56.0, 15.0, 15.0], [57.0, 56.0, 15.0, 15.0]];
const REDSTONE_TAB: Rect = [176.0, 6.0, 26.0, 26.0];
const UPGRADE_HOLDER: Rect = [176.0, 34.0, 26.0, 48.0];
const ENERGY_SLOT_AT: [f32; 2] = [152.0, 20.0];
const MISSING_AT: [f32; 2] = [64.0, 21.0];
/// The config page's rows: radius, min, max, each with a minus and a plus; inverse and its replace; the filters.
const ROW: f32 = 12.0;
const CONFIG_TOP: f32 = 20.0;

pub(super) fn screen() -> NativeScreen {
    PAGE.with(|p| p.set(0));
    let mut slots: Vec<Option<[f32; 2]>> = (0..miner::MAIN_SLOTS).map(|i| Some([8.0 + (i % 9) as f32 * 18.0, 92.0 + (i / 9) as f32 * 18.0])).collect();
    slots.push(Some(ENERGY_SLOT_AT));
    slots.push(Some([181.0, 39.0]));
    slots.push(Some([181.0, 61.0]));
    slots.extend([None; 6]);
    NativeScreen { width: WIDTH, height: HEIGHT, slots, inventory_y: HEIGHT - 82.0, inventory_x: 8.0, own_labels: true, own_background: false }
}

fn config_rect(row: usize, col: f32) -> Rect {
    [col, CONFIG_TOP + ROW * row as f32, 10.0, 10.0]
}

/// The config page's hits: (button, row).
fn config_hit(v: &ViewData, at: [f32; 2], right: bool) -> Option<i32> {
    for (row, (down, up)) in [(RADIUS_DOWN, RADIUS_UP), (MIN_DOWN, MIN_UP), (MAX_DOWN, MAX_UP)].into_iter().enumerate() {
        if mek_ui::inside(config_rect(row, 120.0), at) {
            return Some(down);
        }
        if mek_ui::inside(config_rect(row, 134.0), at) {
            return Some(up);
        }
    }
    if mek_ui::inside([8.0, CONFIG_TOP + ROW * 3.0, 160.0, 10.0], at) {
        return Some(INVERSE);
    }
    if mek_ui::inside([8.0, CONFIG_TOP + ROW * 4.0, 160.0, 10.0], at) {
        return Some(INVERSE_REPLACE);
    }
    for i in 0..v.filters.len() {
        if mek_ui::inside([8.0, CONFIG_TOP + ROW * (5 + i) as f32, 160.0, 10.0], at) {
            return Some(if right { FILTER_DELETE } else { FILTER_TOGGLE } + i as i32);
        }
    }
    let add = [8.0, CONFIG_TOP + ROW * (5 + v.filters.len()) as f32, 160.0, 10.0];
    mek_ui::inside(add, at).then_some(ADD_HELD)
}

/// `native_screen_click`: the page's buttons; Config and Back switch pages on the client.
pub(super) fn click(data: &[i32], at: [f32; 2], right: bool, button: &mut Option<i32>) {
    let Some(v) = unpack(data) else { return };
    if mek_ui::inside(REDSTONE_TAB, at) {
        *button = Some(REDSTONE);
        return;
    }
    if PAGE.with(Cell::get) == 1 {
        if mek_ui::inside([148.0, 4.0, 22.0, 12.0], at) {
            PAGE.with(|p| p.set(0));
            return;
        }
        *button = config_hit(&v, at, right);
        return;
    }
    let idle = v.state == SearchState::Idle;
    let running = v.flags[0];
    for (i, r) in BUTTONS.iter().enumerate() {
        if mek_ui::inside(*r, at) {
            match i {
                0 if idle || !running => *button = Some(START),
                1 if !idle && running => *button = Some(STOP),
                2 if idle => PAGE.with(|p| p.set(1)),
                3 if !idle => *button = Some(RESET),
                _ => {}
            }
            return;
        }
    }
    for (i, r) in SWITCHES.iter().enumerate() {
        if mek_ui::inside(*r, at) {
            *button = Some([EJECT, PULL, SILK][i]);
            return;
        }
    }
}

pub(super) fn tooltip(data: &[i32], at: [f32; 2], tips: &mut Vec<String>) {
    let Some(v) = unpack(data) else { return };
    let settings = mek_ui::settings();
    if mek_ui::inside(POWER_BAR, at) {
        tips.push(display::fraction(v.stored, v.capacity, &settings));
    } else if mek_ui::inside(mek_ui::ENERGY_TAB, at) {
        tips.push(format!("Max Energy: {}", display::energy(v.capacity, &settings)));
        tips.push(format!("Needed per tick: {}", display::energy(v.per_tick, &settings)));
        tips.push(format!("Buffer free: {}", display::energy(v.capacity.saturating_sub(v.stored), &settings)));
    } else if mek_ui::inside(REDSTONE_TAB, at) {
        tips.push(format!("Redstone mode: {}", ["Ignored", "High", "Low", "Pulse"][v.control.clamp(0, 3) as usize]));
    } else if PAGE.with(Cell::get) == 0 {
        if let Some(i) = SWITCHES.iter().position(|r| mek_ui::inside(*r, at)) {
            tips.push(["Auto-eject", "Auto-pull", "Silk touch"][i].to_owned());
        } else if mek_ui::inside([MISSING_AT[0], MISSING_AT[1], 18.0, 18.0], at) {
            tips.push(if v.missing.is_empty() { "All is well!".to_owned() } else { format!("Missing block: {}", v.missing) });
        }
    } else if v.filters.iter().enumerate().any(|(i, _)| mek_ui::inside([8.0, CONFIG_TOP + ROW * (5 + i) as f32, 160.0, 10.0], at)) {
        tips.push("Click: enable or disable. Right click: delete.".to_owned());
    }
}

fn on_off(b: bool) -> &'static str {
    if b { "On" } else { "Off" }
}

pub(super) fn draw(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let settings = mek_ui::settings();
    let cursor = game.native_cursor();
    let cursor = [cursor[0] - origin[0], cursor[1] - origin[1]];
    let mut pen = Pen { game, batch, origin, cursor };
    let v = unpack(data);
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [0.0, 0.0, WIDTH, HEIGHT]);
    let unit = if settings.show_fe { "fe" } else { "j" };
    pen.whole(&format!("tabs/energy_info_{unit}"), [26.0, 26.0], mek_ui::ENERGY_TAB);
    pen.holder(REDSTONE_TAB, false);
    pen.text("R", [REDSTONE_TAB[0] + 10.0, REDSTONE_TAB[1] + 9.0], mek_ui::TITLE);
    pen.holder(UPGRADE_HOLDER, false);
    pen.slot("normal", None, 180.0, 38.0);
    pen.slot("normal", None, 180.0, 60.0);
    let title = game.native_screen_title().unwrap_or_else(|| "Digital Miner".into());
    if PAGE.with(Cell::get) == 1 {
        pen.centered(&format!("{title} Config"), 0.0, WIDTH, 5.0, mek_ui::TITLE);
        pen.button([148.0, 4.0, 22.0, 12.0], true);
        pen.text("<", [156.0, 6.0], 0xFFFFFF);
        if let Some(v) = &v {
            let rows = [format!("Radius: {}", v.radius), format!("Min: {}", v.min_y), format!("Max: {}", v.max_y)];
            for (row, line) in rows.iter().enumerate() {
                pen.text(line, [8.0, CONFIG_TOP + ROW * row as f32 + 1.0], mek_ui::TITLE);
                pen.button(config_rect(row, 120.0), true);
                pen.text("-", [123.0, CONFIG_TOP + ROW * row as f32 + 1.0], 0xFFFFFF);
                pen.button(config_rect(row, 134.0), true);
                pen.text("+", [137.0, CONFIG_TOP + ROW * row as f32 + 1.0], 0xFFFFFF);
            }
            let replace = if v.replace.is_empty() { "none".to_owned() } else { v.replace.clone() };
            pen.text(&format!("Inverse mode: {} (replace: {replace})", on_off(v.flags[4])), [8.0, CONFIG_TOP + ROW * 3.0 + 1.0], mek_ui::TITLE);
            pen.text(&format!("Require replace: {}", on_off(v.flags[5])), [8.0, CONFIG_TOP + ROW * 4.0 + 1.0], mek_ui::TITLE);
            for (i, (kind, enabled, required, target, replace)) in v.filters.iter().enumerate() {
                let kind = ["Item", "Tag", "Mod ID"][(*kind).clamp(0, 2) as usize];
                let mut line = format!("{} {kind}: {target}", if *enabled { "[x]" } else { "[ ]" });
                if !replace.is_empty() {
                    line.push_str(&format!(" > {replace}{}", if *required { "!" } else { "" }));
                }
                pen.text(&line, [8.0, CONFIG_TOP + ROW * (5 + i) as f32 + 1.0], if *enabled { mek_ui::TITLE } else { 0x808080 });
            }
            pen.text("+ Item filter: the held item", [8.0, CONFIG_TOP + ROW * (5 + v.filters.len()) as f32 + 1.0], 0x2060A0);
        }
    } else {
        pen.centered(&title, 0.0, WIDTH, 5.0, mek_ui::TITLE);
        pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], INNER);
        pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], POWER_BAR);
        if let Some(v) = &v {
            let level = if v.capacity == 0 { 0.0 } else { v.stored as f64 / v.capacity as f64 };
            let h = (level * (POWER_BAR[3] - 2.0) as f64) as i32 as f32;
            if h > 0.0 {
                pen.sprite("bar/vertical_power", [0.0, 0.0, 4.0, h], [POWER_BAR[0] + 1.0, POWER_BAR[1] + POWER_BAR[3] - 1.0 - h, 4.0, h]);
            }
            let running = if v.per_tick > v.capacity { "Not enough energy" } else if v.flags[0] { "Running" } else { "Idle" };
            for (i, line) in [running.to_owned(), v.state.name().to_owned(), format!("To mine: {}", v.to_mine)].iter().enumerate() {
                pen.text(line, [INNER[0] + 3.0, INNER[1] + 3.0 + 10.0 * i as f32], mek_ui::SCREEN_TEXT);
            }
            for (i, r) in SWITCHES.iter().enumerate() {
                let on = v.flags[[2, 3, 1][i]];
                pen.button(*r, true);
                pen.text(["E", "P", "S"][i], [r[0] + 4.0, r[1] + 4.0], if on { 0x3CFE9A } else { 0xFF5555 });
            }
            pen.slot("digital", None, MISSING_AT[0], MISSING_AT[1]);
            if !v.missing.is_empty() {
                pen.text("!", [MISSING_AT[0] + 7.0, MISSING_AT[1] + 5.0], 0xFF5555);
            }
            let idle = v.state == SearchState::Idle;
            let running = v.flags[0];
            let active = [idle || !running, !idle && running, idle, !idle];
            for (i, label) in ["Start", "Stop", "Config", "Reset"].iter().enumerate() {
                pen.button(BUTTONS[i], active[i]);
                pen.centered(label, BUTTONS[i][0], BUTTONS[i][2], BUTTONS[i][1] + 5.0, if active[i] { 0xFFFFFF } else { 0xA0A0A0 });
            }
            let _ = v.used;
        }
        pen.slot("power", Some("overlay_power"), ENERGY_SLOT_AT[0] - 1.0, ENERGY_SLOT_AT[1] - 1.0);
    }
    for i in 0..miner::MAIN_SLOTS {
        pen.slot("normal", None, 7.0 + (i % 9) as f32 * 18.0, 91.0 + (i / 9) as f32 * 18.0);
    }
    for row in 0..4 {
        let y = if row == 3 { HEIGHT - 25.0 } else { HEIGHT - 83.0 + 18.0 * row as f32 };
        for col in 0..9 {
            pen.slot("normal", None, 7.0 + 18.0 * col as f32, y);
        }
    }
    pen.text("Inventory", [8.0, HEIGHT - 94.0], mek_ui::TITLE);
}

// ---- /mek probe -------------------------------------------------------------------------------

pub(super) fn probe(game: &Game<'_>, pos: [i32; 3], nbt: &Compound, data: &[i32], items: &[Option<ItemStack>]) -> Value {
    let (config, _) = mek_world::energy_config();
    let dimension = game.native_dimension().to_owned();
    let s = read(nbt, config.miner.max_radius);
    let v = unpack(data);
    let (to_mine, missing, held, chunks, last) = LIVE.with(|l| {
        l.borrow()
            .get(&(dimension, pos))
            .map(|live| (live.queue.to_mine, live.missing.clone(), live.held.clone(), live.queue.map.len(), live.last_mined))
            .unwrap_or((0, None, Vec::new(), 0, None))
    });
    let main: Vec<Value> = items
        .iter()
        .take(miner::MAIN_SLOTS)
        .enumerate()
        .filter_map(|(i, s)| s.as_ref().map(|s| json!({ "slot": i, "item": &*s.item, "count": s.count })))
        .collect();
    let filters: Vec<Value> = s
        .filters
        .iter()
        .map(|f| json!({ "type": f.kind.type_name(), "enabled": f.enabled, "replace": f.replace_target, "required": f.requires_replacement }))
        .collect();
    json!({
        "pos": pos,
        "id": MINER,
        "energy": mek_world::saved_energy(nbt).unwrap_or(0),
        "capacity": v.as_ref().map_or(0, |v| v.capacity),
        "per_tick": v.as_ref().map_or(0, |v| v.per_tick),
        "used": v.as_ref().map_or(0, |v| v.used),
        "radius": s.radius, "min": s.min_y, "max": s.max_y,
        "silk_touch": s.silk_touch, "inverse": s.inverse, "eject": s.eject, "pull": s.pull,
        "running": s.running, "delay": s.delay, "state": s.state.name(), "state_id": s.state.ordinal(),
        "to_mine": to_mine, "chunks": chunks, "last_mined": last, "missing": missing, "held_chunks": held,
        "control_type": control(nbt),
        "filters": filters,
        "upgrades": Upgrade::ALL.iter().filter_map(|u| { let n = super::mek_pump::read_upgrades(nbt).get(*u); (n > 0).then(|| json!({ "type": u.name(), "amount": n })) }).collect::<Vec<_>>(),
        "items": main,
        "overflow": read_overflow(nbt).iter().map(|s| json!({ "item": &*s.item, "count": s.count })).collect::<Vec<_>>(),
        "active": game.native_block_property(pos, "active").as_deref() == Some("true"),
    })
}
