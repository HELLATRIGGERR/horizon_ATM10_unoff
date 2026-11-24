// Modern Industrialization's multiblocks in the game (support module `crate::game::mi_multiblock` of the
// `modern_industrialization` native bundle): the hatches' block entities (their item slots in the engine's container,
// `inventory_fluids` and `storedEu` in the compound, as MI names them), and the electric crafting multiblocks'
// controllers. A controller matches its shape around itself (`mi_core::multiblock`), again when a block of the shape
// changes (`block_changed`, as MI's `ChunkEventListeners` call `ShapeMatcher.onBlockUpdate`), and runs the crafter over
// the matched hatches' slots and EU (`mi_core::multiblocks`). What MI does not save is kept here per position: the
// matcher, the hatches it linked and the operating state; a loaded controller matches again on its first tick.
// Modern Industrialization is Copyright (c) 2020 Azercoco & Technici4n, MIT; see this bundle's README.md.

use super::Game;
use super::mi_world::{self, STORED_EU, key, number};
use horizon_client::{containers::ModContainer, items::ItemStack};
use horizon_core::nbt::{Compound, Tag};
use mod_api::{ModBlockEntity, NativeFeInfo, PlayerRef};
use mi_core::{
    crafter::Crafter,
    energy::EnergyStore,
    hatches::{self, Hatch, HatchKind},
    inventory::{FluidSlot, ItemSlot},
    machines,
    multiblock::{BlockEntityAt, Facing, Match, Pos, ShapeMatcher, ShapeTemplate, ShapeWorld},
    multiblocks::{self, Electric, HatchInventory, MultiblockMachine, OperatingState},
    view::{FluidView, MachineView, MultiblockView, PROGRESS_SCALE},
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

/// `OrientationComponent`'s facing (`Direction.get3DDataValue`), `ActiveShapeComponent`'s index, `IsActiveComponent`.
const FACING: &str = "facingDirection";
const ACTIVE_SHAPE: &str = "activeShape";
const IS_ACTIVE: &str = "isActive";
const LOCKS: &str = "inventory_locks";

/// A controller's link to its shape (`MultiblockMachineBlockEntity.shapeMatcher` and the operating state).
struct Link {
    machine: &'static MultiblockMachine,
    facing: Facing,
    shape: usize,
    matcher: ShapeMatcher,
    members: HashSet<Pos>,
    needs_rematch: bool,
    matched: bool,
    hatches: Vec<Pos>,
    state: OperatingState,
    last: Match,
}

thread_local! {
    static LINKS: RefCell<HashMap<Pos, Link>> = RefCell::new(HashMap::new());
    /// The controller each linked hatch belongs to (`HatchBlockEntity.isMatched`).
    static OWNERS: RefCell<HashMap<Pos, Pos>> = RefCell::new(HashMap::new());
    static TEMPLATES: RefCell<HashMap<&'static str, Rc<Vec<ShapeTemplate>>>> = RefCell::new(HashMap::new());
}

/// The world closes or opens: no controller is linked.
pub(super) fn forget() {
    LINKS.with(|l| l.borrow_mut().clear());
    OWNERS.with(|o| o.borrow_mut().clear());
}

fn templates(machine: &'static MultiblockMachine) -> Rc<Vec<ShapeTemplate>> {
    TEMPLATES.with(|t| t.borrow_mut().entry(machine.id).or_insert_with(|| Rc::new(machine.templates())).clone())
}

/// The menu a hatch's item slots open in: a hopper's five, a dispenser's nine, or two rows of a chest.
pub(super) fn menu_slots(count: usize) -> (usize, &'static str) {
    if count <= 5 {
        (5, "minecraft:hopper")
    } else if count <= 9 {
        (9, "minecraft:generic_3x3")
    } else {
        (18, "minecraft:generic_9x2")
    }
}

// ---- Block entities -------------------------------------------------------

/// `block_entity_types`: each hatch and controller, one type per block and named after it.
pub(super) fn block_entity_types(types: &mut Vec<(String, Vec<String>)>) {
    for id in hatches::ALL.iter().map(|h| h.id).chain(multiblocks::all().iter().map(|m| m.id)) {
        let block = format!("modern_industrialization:{id}");
        types.push((block.clone(), vec![block]));
    }
}

/// `native_block_entity`: an empty hatch, or a controller facing north on its first shape.
pub(super) fn new_entity(block: &str, entity: &mut Option<Compound>) {
    if entity.is_some() {
        return;
    }
    if let Some(h) = hatches::of_block(block) {
        let mut nbt = Compound::new();
        match h.kind {
            HatchKind::Fluid { .. } => mi_world::write_fluids(&mut nbt, &[FluidSlot::standard(h.fluid_capacity())]),
            HatchKind::Energy { .. } => {
                nbt.insert(key(STORED_EU), Tag::Long(0));
            }
            HatchKind::Item { .. } => {}
        }
        *entity = Some(nbt);
    } else if multiblocks::of_block(block).is_some() {
        let mut nbt = Compound::new();
        mi_world::write_crafter(&mut nbt, &Crafter::new());
        nbt.insert(key(ACTIVE_SHAPE), Tag::Int(0));
        nbt.insert(key(FACING), Tag::Int(Facing::North.data_value()));
        *entity = Some(nbt);
    }
}

/// Hatches tick to show their contents; controllers run their shape and crafter.
pub(super) fn ticks(block: &str) -> bool {
    hatches::of_block(block).is_some() || multiblocks::of_block(block).is_some()
}

/// `native_block_entity_container`: an item hatch's slots (pipes insert into an input hatch and take from an output
/// one, through every face); the other hatches and the controllers open a menu with no slots.
pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if container.is_some() {
        return;
    }
    let (slots, menu, all, input) = if let Some(h) = hatches::of_block(block) {
        let (slots, menu) = menu_slots(h.item_slots());
        (slots, menu, (0..h.item_slots()).collect::<Vec<usize>>(), h.input)
    } else if multiblocks::of_block(block).is_some() {
        (5, "minecraft:hopper", Vec::new(), true)
    } else {
        return;
    };
    *container = Some(ModContainer {
        slots,
        faces: std::array::from_fn(|_| all.clone()),
        inputs: if input { all.clone() } else { Vec::new() },
        outputs: if input { Vec::new() } else { all },
        menu: Some(menu.into()),
    });
}

/// `native_block_entity_slot_accepts`: an input hatch's slots take any item; an output hatch's and the hidden slots none.
pub(super) fn slot_accepts(block: &str, slot: usize, accepts: &mut bool) {
    if let Some(h) = hatches::of_block(block) {
        if !h.input || slot >= h.item_slots() {
            *accepts = false;
        }
    } else if multiblocks::of_block(block).is_some() {
        *accepts = false;
    }
}

/// `OrientationComponent.onPlaced`: a controller placed by a player faces them (`placer.getDirection().getOpposite()`).
pub(super) fn placed(game: &mut Game<'_>, pos: Pos, placer: Option<&PlayerRef>) {
    let Some(player) = placer else { return };
    let Some([yaw, _]) = game.entity_rotation(player.entity()) else { return };
    // `Direction.fromYRot`: `floor(yaw / 90 + 0.5) & 3` is south, west, north, east.
    let facing = match ((yaw / 90.0 + 0.5).floor() as i32) & 3 {
        0 => Facing::North,
        1 => Facing::East,
        2 => Facing::South,
        _ => Facing::West,
    };
    game.native_with_mod_block_entity(pos, |m| {
        if multiblocks::of_block(m.id()).is_some() {
            m.nbt.insert(key(FACING), Tag::Int(facing.data_value()));
        }
    });
}

/// `ShapeMatcher.onBlockUpdate`: a block of a linked shape changed, so it matches again; a controller's own block
/// changing drops its link (`setRemoved` unlinks).
pub(super) fn block_changed(pos: Pos) {
    LINKS.with(|l| {
        let Ok(mut links) = l.try_borrow_mut() else { return };
        if links.is_empty() {
            return;
        }
        if links.remove(&pos).is_some() {
            OWNERS.with(|o| o.borrow_mut().retain(|_, c| *c != pos));
        }
        for link in links.values_mut() {
            if link.members.contains(&pos) {
                link.needs_rematch = true;
            }
        }
    });
}

// ---- Energy -----------------------------------------------------------------

fn energy_hatch(entity: &ModBlockEntity) -> Option<&'static Hatch> {
    hatches::of_block(entity.id()).filter(|h| matches!(h.kind, HatchKind::Energy { .. }))
}

fn energy_of(nbt: &Compound, h: &Hatch) -> EnergyStore {
    EnergyStore::new(number(nbt.get(&key(STORED_EU))), h.eu_capacity())
}

/// `native_fe_storage`: an energy input hatch takes Forge Energy from every side (`EnergyHatch.registerEnergyApi`'s
/// insertable); an output hatch gives none here, no ported multiblock fills one.
pub(super) fn fe_storage(entity: &ModBlockEntity, info: &mut Option<NativeFeInfo>) {
    let Some(h) = energy_hatch(entity) else { return };
    if info.is_some() {
        return;
    }
    let (stored, capacity) = energy_of(&entity.nbt, h).fe(mi_world::fe_per_eu());
    *info = Some(NativeFeInfo { stored, capacity, can_receive: h.input, can_extract: false });
}

/// `native_fe_receive`: whole EU only, as the machines.
pub(super) fn fe_receive(entity: &mut ModBlockEntity, max: i64, simulate: bool, accepted: &mut Option<i64>) {
    let Some(h) = energy_hatch(entity) else { return };
    if accepted.is_some() {
        return;
    }
    if !h.input {
        *accepted = Some(0);
        return;
    }
    let mut energy = energy_of(&entity.nbt, h);
    let taken = energy.receive_fe(max, mi_world::fe_per_eu(), !simulate);
    if taken > 0 && !simulate {
        entity.nbt.insert(key(STORED_EU), Tag::Long(energy.eu));
    }
    *accepted = Some(taken);
}

/// `native_fe_extract`: an energy hatch gives none.
pub(super) fn fe_extract(entity: &ModBlockEntity) -> bool {
    energy_hatch(entity).is_some()
}

// ---- Ticks ------------------------------------------------------------------

/// `native_block_entity_tick` of a hatch or a controller.
pub(super) fn tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity) {
    if let Some(h) = hatches::of_block(entity.id()) {
        return hatch_tick(h, entity);
    }
    if let Some(machine) = multiblocks::of_block(entity.id()) {
        controller_tick(game, pos, entity, machine);
    }
}

/// A hatch's screen data: its EU, its fluid, its reserved item slots.
fn hatch_tick(h: &'static Hatch, entity: &mut ModBlockEntity) {
    let fluids = mi_world::fluid_slots(&entity.nbt, h.fluid_slots(), h.fluid_capacity());
    let energy = energy_of(&entity.nbt, h);
    let view = MachineView {
        eu: energy.eu,
        eu_capacity: energy.capacity,
        machine_locked: mi_world::read_locks(&entity.nbt).iter().fold(0, |m, (i, _)| m | 1 << i),
        fluids: fluids
            .iter()
            .map(|s| FluidView { fluid: mi_world::fluid_id(s.fluid.as_deref()), amount: s.amount, capacity: s.capacity, locked: mi_world::fluid_id(s.locked.as_deref()) })
            .collect(),
        ..MachineView::default()
    };
    let mut data = Vec::new();
    view.pack_into(&mut data);
    if entity.data != data {
        entity.data = data;
    }
}

/// The world as a controller's shape reads it: blocks and their properties in the level in place, and a hatch where a
/// hatch's block stands (another controller's when it is linked to one).
struct InWorld<'a, 'g> {
    game: &'a Game<'g>,
    owner: Pos,
}

impl ShapeWorld for InWorld<'_, '_> {
    fn block(&mut self, pos: Pos) -> String {
        self.game.native_block(pos)
    }
    fn property(&mut self, pos: Pos, key: &str) -> Option<String> {
        self.game.native_block_property(pos, key)
    }
    fn in_tag(&mut self, pos: Pos, tag: &str) -> bool {
        let block = self.game.native_block(pos);
        self.game.native_block_name_in_tag(&block, tag).unwrap_or(false)
    }
    fn block_entity(&mut self, pos: Pos) -> BlockEntityAt {
        let block = self.game.native_block(pos);
        if let Some(h) = hatches::of_block(&block) {
            let matched = OWNERS.with(|o| o.borrow().get(&pos).is_some_and(|c| *c != self.owner));
            BlockEntityAt::Hatch { kind: h.hatch_type(), matched }
        } else if machines::of_block(&block).is_some() || multiblocks::of_block(&block).is_some() {
            BlockEntityAt::Other
        } else {
            BlockEntityAt::None
        }
    }
}

fn new_link(machine: &'static MultiblockMachine, pos: Pos, facing: Facing, shape: usize, template: &ShapeTemplate) -> Link {
    let matcher = ShapeMatcher::new(pos, facing, template);
    let members = matcher.positions().collect();
    Link { machine, facing, shape, matcher, members, needs_rematch: true, matched: false, hatches: Vec::new(), state: OperatingState::NotMatched, last: Match::default() }
}

/// `HatchBlockEntity.unlink` for every hatch linked to `controller`: free again, their machine locks off.
fn unlink(game: &mut Game<'_>, controller: Pos) {
    let freed: Vec<Pos> = OWNERS.with(|o| {
        let mut o = o.borrow_mut();
        let freed: Vec<Pos> = o.iter().filter(|(_, c)| **c == controller).map(|(h, _)| *h).collect();
        for h in &freed {
            o.remove(h);
        }
        freed
    });
    for pos in freed {
        game.native_with_mod_block_entity(pos, |m| {
            let Some(h) = hatches::of_block(m.id()) else { return };
            m.nbt.remove(&key(LOCKS));
            if h.fluid_slots() > 0 {
                let mut fluids = mi_world::fluid_slots(&m.nbt, h.fluid_slots(), h.fluid_capacity());
                if fluids.iter().any(|f| f.machine_locked) {
                    fluids.iter_mut().for_each(FluidSlot::disable_machine_lock);
                    mi_world::write_fluids(&mut m.nbt, &fluids);
                }
            }
        });
    }
}

/// `link()`'s rematch and `onRematch`: the shape matched again, its hatches linked to this controller.
fn rematch(game: &mut Game<'_>, pos: Pos, link: &mut Link) {
    unlink(game, pos);
    let m = link.matcher.rematch(&mut InWorld { game: &*game, owner: pos });
    link.matched = m.ok;
    link.hatches = m.hatches.clone();
    if m.ok {
        OWNERS.with(|o| {
            let mut o = o.borrow_mut();
            for h in &m.hatches {
                o.insert(*h, pos);
            }
        });
    }
    link.last = m;
    link.needs_rematch = false;
    link.state = if link.matched { OperatingState::TryingToResume } else { OperatingState::NotMatched };
}

/// A linked hatch's slots and EU, and its item stacks as the engine holds them.
fn read_hatch(game: &Game<'_>, pos: Pos) -> Option<(HatchInventory, Vec<Option<ItemStack>>)> {
    let m = game.native_mod_block_entity(pos)?;
    let hatch = hatches::of_block(m.id())?;
    let mut items: Vec<ItemSlot> = (0..hatch.item_slots()).map(|i| mi_world::item_slot(game, m.items.get(i).and_then(Option::as_ref))).collect();
    for (i, item) in mi_world::read_locks(&m.nbt) {
        if let Some(s) = items.get_mut(i) {
            s.enable_machine_lock(&item);
        }
    }
    let fluids = mi_world::fluid_slots(&m.nbt, hatch.fluid_slots(), hatch.fluid_capacity());
    let energy = energy_of(&m.nbt, hatch);
    Some((HatchInventory { pos, hatch, items, fluids, energy }, m.items.clone()))
}

fn write_hatch(game: &mut Game<'_>, h: &HatchInventory, stacks: &[Option<ItemStack>]) {
    let changed = mi_world::changed_stacks(stacks, &h.items);
    game.native_with_mod_block_entity(h.pos, |m| {
        for (i, new) in changed {
            if m.items.len() <= i {
                m.items.resize(i + 1, None);
            }
            m.items[i] = new;
        }
        mi_world::write_item_locks(&mut m.nbt, &h.items, 0);
        if h.hatch.fluid_slots() > 0 {
            mi_world::write_fluids(&mut m.nbt, &h.fluids);
        }
        if h.hatch.eu_capacity() > 0 {
            m.nbt.insert(key(STORED_EU), Tag::Long(h.energy.eu));
        }
    });
}

/// The facing and shape a controller's compound holds.
fn orientation(nbt: &Compound, shapes: usize) -> (Facing, usize) {
    let facing = nbt.get(&key(FACING)).map_or(Facing::North, |t| Facing::from_data_value(number(Some(t)) as i32));
    let shape = (number(nbt.get(&key(ACTIVE_SHAPE))).max(0) as usize).min(shapes.saturating_sub(1));
    (facing, shape)
}

/// `AbstractCraftingMultiblockBlockEntity.tick`: `link()`, then the crafter over the linked hatches.
fn controller_tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity, machine: &'static MultiblockMachine) {
    let templates = templates(machine);
    if templates.is_empty() {
        return;
    }
    let (facing, shape) = orientation(&entity.nbt, templates.len());
    let old = LINKS.with(|l| l.borrow_mut().remove(&pos));
    // A new facing (the wrench) or shape (`ActiveShapeComponent.setShape`) unlinks and matches again.
    let mut link = match old {
        Some(l) if l.facing == facing && l.shape == shape && l.machine.id == machine.id => l,
        _ => {
            unlink(game, pos);
            new_link(machine, pos, facing, shape, &templates[shape])
        }
    };
    if link.needs_rematch {
        rematch(game, pos, &mut link);
    }
    let (mut hs, stacks): (Vec<HatchInventory>, Vec<Vec<Option<ItemStack>>>) =
        if link.matched { link.hatches.iter().filter_map(|p| read_hatch(game, *p)).unzip() } else { (Vec::new(), Vec::new()) };
    let before = hs.clone();
    let book = mi_world::book_of(game, machine.recipe_type, machine.rules);
    let mut inv = multiblocks::gather(&hs);
    let mut crafter = mi_world::read_crafter(&entity.nbt);
    let crafter_before = crafter.clone();
    let active = {
        let g: &Game<'_> = game;
        let max_stack = |item: &str| g.native_max_stack_size(item).unwrap_or(64);
        let mut random = mi_world::next_float;
        let mut b = Electric { hatches: &mut hs, machine, shape, max_stack: &max_stack, random: &mut random };
        multiblocks::tick(&mut link.state, &mut crafter, &book, &mut inv, &mut b)
    };
    multiblocks::scatter(&mut hs, &inv);
    for ((h, old), stacks) in hs.iter().zip(&before).zip(&stacks) {
        if h != old {
            write_hatch(game, h, stacks);
        }
    }
    if crafter != crafter_before {
        mi_world::write_crafter(&mut entity.nbt, &crafter);
    }
    if (number(entity.nbt.get(&key(IS_ACTIVE))) != 0) != active {
        entity.nbt.insert(key(IS_ACTIVE), Tag::Byte(i8::from(active)));
    }
    let shows = link.matched && crafter.has_active_recipe();
    let base = crafter.active_recipe.as_deref().and_then(|id| book.get(id)).map_or(0, |r| i64::from(r.eu));
    let view = MultiblockView {
        shape_valid: link.matched,
        matches_multiple_recipes: crafter.matches_multiple_recipes,
        has_active_recipe: shows,
        progress: if shows { (f64::from(crafter.progress()) * PROGRESS_SCALE as f64) as i64 } else { 0 },
        efficiency_ticks: i64::from(crafter.efficiency_ticks),
        max_efficiency_ticks: i64::from(crafter.max_efficiency_ticks),
        current_recipe_eu: crafter.recipe_max_eu,
        base_recipe_eu: base,
        active_shape: shape as i64,
    };
    let mut data = Vec::new();
    view.pack_into(&mut data);
    if entity.data != data {
        entity.data = data;
    }
    LINKS.with(|l| l.borrow_mut().insert(pos, link));
}

// ---- /mi --------------------------------------------------------------------

fn slots_json(items: &[Option<ItemStack>], count: usize) -> Vec<Value> {
    (0..count)
        .map(|i| match items.get(i).and_then(Option::as_ref) {
            Some(s) => json!({ "item": &*s.item, "count": s.count }),
            None => Value::Null,
        })
        .collect()
}

fn fluids_json(fluids: &[FluidSlot]) -> Vec<Value> {
    fluids.iter().map(|f| json!({ "fluid": f.fluid, "amount": f.amount, "capacity": f.capacity, "locked": f.locked })).collect()
}

/// `/mi probe` of a hatch or a controller: its slots, or its shape, linked hatches, crafter and the recipes it could
/// start (`candidates`, with `banned` for those the active shape refuses).
pub(super) fn probe(game: &mut Game<'_>, pos: Pos, id: &str, nbt: &Compound, items: &[Option<ItemStack>]) -> Option<Value> {
    if let Some(h) = hatches::of_block(id) {
        let energy = energy_of(nbt, h);
        return Some(json!({
            "pos": pos,
            "id": id,
            "hatch": h.hatch_type().id(),
            "slots": slots_json(items, h.item_slots()),
            "locks": mi_world::read_locks(nbt),
            "fluids": fluids_json(&mi_world::fluid_slots(nbt, h.fluid_slots(), h.fluid_capacity())),
            "eu": energy.eu,
            "eu_capacity": energy.capacity,
            "linked_to": OWNERS.with(|o| o.borrow().get(&pos).copied()),
        }));
    }
    let machine = multiblocks::of_block(id)?;
    let templates = templates(machine);
    let (facing, shape) = orientation(nbt, templates.len());
    let link = LINKS.with(|l| l.borrow().get(&pos).map(|l| (l.matched, l.hatches.clone(), l.state, l.last.clone(), l.members.len())));
    let (matched, linked, state, last, members) = link.unwrap_or((false, Vec::new(), OperatingState::NotMatched, Match::default(), 0));
    let c = mi_world::read_crafter(nbt);
    let book = mi_world::book_of(game, machine.recipe_type, machine.rules);
    let (mut hs, _): (Vec<HatchInventory>, Vec<_>) = linked.iter().filter_map(|p| read_hatch(game, *p)).unzip();
    let hatch_list: Vec<Value> = hs.iter().map(|h| json!({ "pos": h.pos, "id": h.hatch.id })).collect();
    let mut inv = multiblocks::gather(&hs);
    let held: Vec<String> = inv.item_inputs.iter().filter_map(|s| s.item.clone()).collect();
    let g: &Game<'_> = game;
    let max_stack = |item: &str| g.native_max_stack_size(item).unwrap_or(64);
    let mut random = mi_world::next_float;
    let mut b = Electric { hatches: &mut hs, machine, shape, max_stack: &max_stack, random: &mut random };
    use mi_core::crafter::{Behavior, put_fluid_outputs, put_item_outputs, take_fluid_inputs, take_item_inputs};
    let candidates: Vec<Value> = book
        .candidates(held.iter().map(String::as_str))
        .iter()
        .map(|r| {
            let banned = b.ban_recipe(r);
            let (i, f) = (take_item_inputs(r, &mut inv, true, &mut b), take_fluid_inputs(r, &mut inv, true, &mut b));
            let (o, fo) = (put_item_outputs(r, &mut inv, true, false, &mut b), put_fluid_outputs(r, &mut inv, true, false, &mut b));
            json!({ "id": r.id, "eu": r.eu, "banned": banned, "items": i, "fluids": f, "outputs": o, "fluid_outputs": fo })
        })
        .collect();
    Some(json!({
        "pos": pos,
        "id": id,
        "recipe_type": machine.recipe_type,
        "recipes": book.recipes.len(),
        "facing": facing.name(),
        "active_shape": shape,
        "shape_name": machine.shapes.get(shape).map(|s| s.0),
        "members": members,
        "shape_valid": matched,
        "operating_state": format!("{state:?}"),
        "first_failure": last.first_failure,
        "mismatching": last.mismatching,
        "hatches": hatch_list,
        "item_inputs": inv.item_inputs.len(),
        "item_outputs": inv.item_outputs.len(),
        "fluid_inputs": inv.fluid_inputs.len(),
        "fluid_outputs": inv.fluid_outputs.len(),
        "eu": hs_eu(&linked, game),
        "active_recipe": c.active_recipe,
        "used_energy": c.used_energy,
        "recipe_energy": c.recipe_energy,
        "recipe_max_eu": c.recipe_max_eu,
        "efficiency_ticks": c.efficiency_ticks,
        "max_efficiency_ticks": c.max_efficiency_ticks,
        "is_active": number(nbt.get(&key(IS_ACTIVE))) != 0,
        "candidates": candidates,
    }))
}

/// The EU of a controller's linked energy input hatches.
fn hs_eu(linked: &[Pos], game: &Game<'_>) -> i64 {
    linked.iter().filter_map(|p| read_hatch(game, *p)).filter(|(h, _)| h.hatch.input && h.hatch.eu_capacity() > 0).map(|(h, _)| h.energy.eu).sum()
}

/// `/mi shape x y z`: what the controller's active shape still lacks, each position with the block (and its properties,
/// turned to the controller's facing) `buildMultiblock` would place there: the shape's preview as data.
pub(super) fn shape(game: &mut Game<'_>, pos: Pos) -> Value {
    let Some(m) = game.native_mod_block_entity(pos) else {
        return json!({ "pos": pos, "error": "no mod block entity here" });
    };
    let Some(machine) = multiblocks::of_block(m.id()) else {
        return json!({ "pos": pos, "id": m.id(), "error": "not a multiblock controller" });
    };
    let templates = templates(machine);
    let (facing, shape) = orientation(&m.nbt, templates.len());
    let matcher = ShapeMatcher::new(pos, facing, &templates[shape]);
    let missing = matcher.missing(&mut InWorld { game: &*game, owner: pos });
    let hatch_positions = matcher.positions().filter(|p| matcher.hatch_flags(*p).is_some_and(|f| f != mi_core::multiblock::HatchFlags::NO_HATCH)).count();
    json!({
        "pos": pos,
        "id": m.id(),
        "facing": facing.name(),
        "active_shape": shape,
        "members": matcher.members.len(),
        "hatch_positions": hatch_positions,
        "missing": missing.iter().map(|(p, block, props)| json!({ "pos": p, "block": block, "properties": props })).collect::<Vec<_>>(),
    })
}
