// Mekanism's Logistical Transporters in the game (support module
// `crate::game::mek_transporters` of the `mekanism` native bundle). The rules are
// `mekanism_energy::transporter` (MIT, from Mekanism 10.7.19, Copyright (c)
// 2017-2025 Aidan C. Brady); this module is the glue: the transporters of each
// dimension kept here through the block entity lifecycle hook points (API
// 156), each level's tick looking at their sides (any block with a container
// on the touching side), pulling, moving the stacks and committing the new
// ones; the containers around them as the rules' inventories (hook points API
// 48's container helpers and API 425's `native_insert_slots`, a broken
// transporter's stacks dropped with `native_pop_resource`); a transporter's
// stacks and colour saved as Mekanism saves them (`items`, `color`); a
// machine's ejector putting its output into one; the stacks drawn on clients
// (`native_block_entity_update_tag`, `native_block_entity_submit`, API 45 and
// 71) where they travel; `/mek connection`, `/mek color` and `/mek probe`. No
// borrow of this state is held across a call into the game: a dimension's
// transporters are taken out while it ticks.

use super::{Game, mek_world::key, native_block_entity_draw::BlockEntityDraw};
use horizon_assets::item_model::DisplayContext;
use horizon_client::{containers::ModBlockEntity, items::ItemStack};
use horizon_world::{
    level::flags,
    nbt::{Compound, Tag},
};
use mekanism_energy::{
    network::{self, Connection, Pos},
    sides::Direction,
    transmitter::TransporterTier,
    transporter::{self, Color, Inventories, PathType, Request, Slot, Stack, TStack, Transporter, Transporters},
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeSet, HashMap},
};

/// An item stack, with the most of its item a stack holds.
#[derive(Clone, Debug)]
pub(super) struct Item {
    stack: ItemStack,
    max: u32,
}

impl Stack for Item {
    fn count(&self) -> u32 {
        self.stack.count
    }
    fn with_count(&self, count: u32) -> Self {
        Self { stack: ItemStack { count, ..self.stack.clone() }, max: self.max }
    }
    fn stacks_with(&self, other: &Self) -> bool {
        self.stack.same_item_same_components(&other.stack)
    }
    fn max_stack(&self) -> u32 {
        self.max
    }
}

/// A dimension's transporters.
#[derive(Default)]
struct DimTransporters {
    nets: Transporters<Item>,
    look: BTreeSet<Pos>,
    full_look_in: u32,
    loads: u64,
    unloads: u64,
    /// Stacks of broken transporters, dropped at the next tick.
    drops: Vec<(Pos, ItemStack)>,
    /// Transporters loaded since the last tick, read then (their stacks need the game's items).
    pending: Vec<(Pos, Compound, TransporterTier, bool)>,
}

thread_local! {
    static TRANSPORTERS: RefCell<HashMap<String, DimTransporters>> = RefCell::new(HashMap::new());
    static ANY: Cell<bool> = const { Cell::new(false) };
    static SET: RefCell<Vec<Pos>> = const { RefCell::new(Vec::new()) };
    /// Each client view written gets the next number (server side).
    static WRITTEN: Cell<i64> = const { Cell::new(0) };
    /// The game time a client first drew each view at, by position (client side).
    static SEEN: RefCell<HashMap<Pos, (i64, i64)>> = RefCell::new(HashMap::new());
}

fn with_dim<R>(dimension: &str, change: impl FnOnce(&mut DimTransporters) -> R) -> R {
    TRANSPORTERS.with(|t| {
        let mut t = t.borrow_mut();
        if !t.contains_key(dimension) {
            t.insert(dimension.to_owned(), DimTransporters::default());
        }
        change(t.get_mut(dimension).expect("just made"))
    })
}

const RESTRICTIVE: &str = "mekanism:restrictive_transporter";

/// A transporter's tier: the Restrictive Transporter is a basic one (`RestrictiveTransporter`).
pub(super) fn tier_of(block: &str) -> Option<TransporterTier> {
    TransporterTier::of_block(block).or((block == RESTRICTIVE).then_some(TransporterTier::Basic))
}

/// Whether a transporter takes a colour: the Restrictive Transporter has none.
pub(super) fn takes_color(block: &str) -> bool {
    TransporterTier::of_block(block).is_some()
}

pub(super) fn clear() {
    TRANSPORTERS.with(|t| t.borrow_mut().clear());
    ANY.set(false);
    SET.with(|s| s.borrow_mut().clear());
}

pub(super) fn is_transporter(dimension: &str, pos: Pos) -> bool {
    ANY.get() && with_dim(dimension, |d| d.nets.get(pos).is_some())
}

// ---- The saved compound -------------------------------------------------------------

fn connections_of(nbt: &Compound) -> [Connection; 6] {
    let mut out = [Connection::Normal; 6];
    if let Some(Tag::IntArray(ids)) = nbt.get(&key("connection")) {
        for (side, id) in out.iter_mut().zip(ids) {
            *side = Connection::by_id(*id);
        }
    }
    out
}

fn write_connections(nbt: &mut Compound, connections: &[Connection; 6]) {
    nbt.insert(key("connection"), Tag::IntArray(connections.iter().map(|c| c.id()).collect()));
}

fn color_of(nbt: &Compound) -> Option<Color> {
    super::mek_world::number(nbt.get(&key("color"))).map(|v| Color::by_id(v as i32))
}

fn item(game: &Game<'_>, stack: ItemStack) -> Item {
    let max = game.native_max_stack_size(&stack.item).unwrap_or(64).max(1);
    Item { stack, max }
}

/// `TransporterStack.read`: a stack of a save (`item_oversized`, or the older `item`).
fn read_stack(game: &Game<'_>, c: &Compound) -> Option<TStack<Item>> {
    let n = |name: &str| super::mek_world::number(c.get(&key(name)));
    let stack = match c.get(&key("item_oversized")).or_else(|| c.get(&key("item"))) {
        Some(tag) => horizon_client::save::item_from_nbt(tag)?,
        None => return None,
    };
    let pos = |name: &str| n(name).map(transporter::unpack_pos);
    Some(TStack::loaded(
        item(game, stack),
        n("progress").unwrap_or(0) as i32,
        n("color").map(|v| Color::by_id(v as i32)),
        pos("original_location"),
        pos("home_location"),
        n("path_type").map(|v| PathType::by_id(v as i32)),
        n("idle_dir").map(|v| (v as usize).min(5)),
    ))
}

/// `TransporterStack.write`.
fn write_stack(stack: &TStack<Item>) -> Compound {
    let mut c = Compound::new();
    if let Some(color) = stack.color {
        c.insert(key("color"), Tag::Int(color.id()));
    }
    c.insert(key("progress"), Tag::Int(stack.progress));
    c.insert(key("original_location"), Tag::Long(stack.original.map_or(i64::MAX, transporter::pack_pos)));
    if let Some(dir) = stack.idle_dir {
        c.insert(key("idle_dir"), Tag::Int(dir as i32));
    }
    if let Some(home) = stack.home {
        c.insert(key("home_location"), Tag::Long(transporter::pack_pos(home)));
    }
    if let Some(path_type) = stack.path_type {
        c.insert(key("path_type"), Tag::Int(path_type.id()));
    }
    if stack.item.count() > 0 {
        c.insert(key("item_oversized"), horizon_client::save::item_to_nbt(&stack.item.stack));
    }
    c
}

/// `native_block_entity`: every side normal, no colour.
pub(super) fn new_entity(block: &str) -> (String, Compound) {
    let mut nbt = Compound::new();
    write_connections(&mut nbt, &[Connection::Normal; 6]);
    (block.to_owned(), nbt)
}

/// `native_block_entity_load`: the transporter with its colour and the stacks it saved, read at the level's next
/// tick, at whose end it joins (`registerOrphanTransmitter`), as Mekanism's do.
pub(super) fn loaded(dimension: &str, pos: Pos, id: &str, nbt: &Compound, tier: TransporterTier) {
    with_dim(dimension, |d| {
        d.pending.retain(|(p, _, _, _)| *p != pos);
        d.pending.push((pos, nbt.clone(), tier, !takes_color(id)));
        d.loads += 1;
    });
    ANY.set(true);
}

/// A loaded transporter, read; a restrictive one costs 1000 a step (`getCost`) and has no colour.
fn build(game: &Game<'_>, nbt: &Compound, tier: TransporterTier, restrictive: bool) -> Transporter<Item> {
    let settings = super::mek_pipes::settings();
    let (pull, speed) = settings.transporter(tier);
    let (_, ultimate) = settings.transporter(TransporterTier::Ultimate);
    let mut t = Transporter::new(speed, pull, ultimate, connections_of(nbt), if restrictive { None } else { color_of(nbt) });
    if restrictive {
        t.cost = 1000.0;
    }
    if let Some(Tag::List(items)) = nbt.get(&key("items")) {
        for entry in items {
            if let Tag::Compound(c) = entry
                && let Some(stack) = read_stack(game, c)
            {
                t.add_loaded(stack);
            }
        }
    }
    t
}

/// `native_block_entity_unload`: unloaded, its stacks stay in its save; broken, they drop.
pub(super) fn unloaded(dimension: &str, pos: Pos, removed: bool) {
    with_dim(dimension, |d| {
        d.pending.retain(|(p, _, _, _)| *p != pos);
        let mut dropped = Vec::new();
        let mut drop = |at: Pos, stack: Item| dropped.push((at, stack.stack));
        let drop: Option<&mut dyn FnMut(Pos, Item)> = if removed { Some(&mut drop) } else { None };
        d.nets.remove(pos, drop);
        d.drops.extend(dropped);
        d.unloads += 1;
        d.look.extend((0..6).map(|s| network::offset(pos, s)));
    });
}

/// A block with items beside transporters came or went.
pub(super) fn neighbour(dimension: &str, pos: Pos) {
    if ANY.get() {
        with_dim(dimension, |d| d.look.extend((0..6).map(|s| network::offset(pos, s))));
    }
}

pub(super) fn block_set(pos: Pos) {
    if ANY.get() {
        SET.with(|s| s.borrow_mut().push(pos));
    }
}

/// `native_block_entity_save`: its stacks as `items`, its colour as `color`.
pub(super) fn saved(dimension: &str, pos: Pos, nbt: &mut Compound) {
    let written = with_dim(dimension, |d| {
        let t = d.nets.get(pos)?;
        Some((t.color, t.transit.values().map(write_stack).map(Tag::Compound).collect::<Vec<_>>()))
    });
    nbt.remove(&key(CLIENT));
    let Some((color, items)) = written else { return };
    match color {
        Some(c) => nbt.insert(key("color"), Tag::Int(c.id())),
        None => nbt.remove(&key("color")),
    };
    if items.is_empty() {
        nbt.remove(&key("items"));
    } else {
        nbt.insert(key("items"), Tag::List(items));
    }
}

// ---- The inventories around them ----------------------------------------------------

struct World<'g, 'a> {
    game: &'g mut Game<'a>,
    dimension: &'static str,
}

fn face(side: usize) -> horizon_core::geometry::Direction {
    super::mek_world::engine_face(Direction::ALL[side])
}

impl Inventories<Item> for World<'_, '_> {
    fn has_inventory(&mut self, pos: Pos, side: usize) -> bool {
        self.game.native_container_slots(self.dimension, pos, face(side)).is_some()
    }

    fn insert_slots(&mut self, pos: Pos, side: usize, stack: &Item) -> Option<Vec<Slot<Item>>> {
        let slots = self.game.native_insert_slots(self.dimension, pos, face(side), &stack.stack)?;
        Some(
            slots
                .into_iter()
                .map(|s| Slot { stack: s.stack.map(|held| Item { stack: held, max: stack.max }), accepts: s.accepts, limit: s.max })
                .collect(),
        )
    }

    fn insert(&mut self, pos: Pos, side: usize, stack: Item) -> Option<Item> {
        let max = stack.max;
        self.game.native_insert_item(pos, face(side), stack.stack).map(|rest| Item { stack: rest, max })
    }

    fn take_slots(&mut self, pos: Pos, side: usize) -> Vec<(usize, Item)> {
        let slots = self.game.native_container_slots(self.dimension, pos, face(side)).unwrap_or_default();
        slots.into_iter().filter(|s| s.can_take).filter_map(|s| Some((s.slot, item(self.game, s.stack?)))).collect()
    }

    fn extract(&mut self, pos: Pos, side: usize, slot: usize, count: u32) -> Option<Item> {
        let taken = self.game.native_extract_item(self.dimension, pos, face(side), slot, count)?;
        Some(item(self.game, taken))
    }

    fn drop_at(&mut self, pos: Pos, stack: Item) {
        self.game.native_pop_resource(pos, stack.stack);
    }
}

/// The dimension's transporters, taken out for `run` and put back.
fn with_taken<R>(game: &mut Game<'_>, dimension: &'static str, run: impl FnOnce(&mut DimTransporters, &mut World<'_, '_>) -> R) -> R {
    let mut d = TRANSPORTERS.with(|t| t.borrow_mut().remove(dimension)).unwrap_or_default();
    let out = run(&mut d, &mut World { game, dimension });
    TRANSPORTERS.with(|t| {
        let mut t = t.borrow_mut();
        // A transporter loaded while they were out (none should be) joins those put back.
        if let Some(mut late) = t.remove(dimension) {
            d.look.append(&mut late.look);
            d.drops.append(&mut late.drops);
            d.pending.append(&mut late.pending);
        }
        t.insert(dimension.to_owned(), d);
    });
    out
}

// ---- The level's tick --------------------------------------------------------------------

/// Which sides of the transporter at `pos` have a container, and what its block shows.
fn refresh(d: &mut DimTransporters, world: &mut World<'_, '_>, pos: Pos) {
    let Some(raw) = d.nets.get(pos).map(|t| t.connections) else { return };
    for side in 0..6 {
        let at = network::offset(pos, side);
        let inventory = raw[side] != Connection::None && d.nets.get(at).is_none() && world.has_inventory(at, network::opposite(side));
        d.nets.set_acceptor(pos, side, inventory);
    }
}

/// The six face properties, as the model shows them.
fn show(d: &DimTransporters, game: &mut Game<'_>, pos: Pos) {
    for f in Direction::ALL {
        let shown = d.nets.shown(pos, f as usize);
        if game.native_block_property(pos, f.name()).as_deref() != Some(shown.name()) {
            game.native_set_block_property_with(pos, f.name(), shown.name(), flags::UPDATE_CLIENTS);
        }
    }
}

/// `native_level_tick_post`: the transporters' own ticks (`onUpdateServer`), then the new ones join; what changed
/// is saved and sent to clients.
pub(super) fn level_ticked(game: &mut Game<'_>, _dimension: &str) {
    if !ANY.get() {
        return;
    }
    // The level in place, by the name the container helpers take.
    let dimension = game.native_dimension();
    let set: Vec<Pos> = SET.with(|s| std::mem::take(&mut *s.borrow_mut()));
    let changed = with_taken(game, dimension, |d, world| {
        for (pos, stack) in std::mem::take(&mut d.drops) {
            world.game.native_pop_resource(pos, stack);
        }
        for (pos, nbt, tier, restrictive) in std::mem::take(&mut d.pending) {
            let t = build(world.game, &nbt, tier, restrictive);
            d.nets.add(pos, t);
            d.look.insert(pos);
            d.look.extend((0..6).map(|s| network::offset(pos, s)));
        }
        if d.nets.is_empty() {
            d.look.clear();
            return BTreeSet::new();
        }
        for pos in &set {
            d.look.extend((0..6).map(|s| network::offset(*pos, s)));
        }
        if d.full_look_in == 0 {
            d.full_look_in = 20;
            d.look.extend(d.nets.positions().iter().copied());
        }
        d.full_look_in -= 1;
        let looks: Vec<Pos> = std::mem::take(&mut d.look).into_iter().filter(|p| d.nets.get(*p).is_some()).collect();
        for pos in &looks {
            refresh(d, world, *pos);
        }
        d.nets.changed();
        let ticked = d.nets.tick(world);
        d.nets.commit();
        for pos in looks {
            show(d, world.game, pos);
        }
        ticked.changed
    });
    for pos in changed {
        write_client(game, dimension, pos);
    }
}

// ---- Clients ----------------------------------------------------------------------------

/// The key of what clients get of a transporter's stacks.
const CLIENT: &str = "horizon_transit";

/// What the transporter at `pos` shows: each stack's item, progress, colour and the sides it comes in by and goes
/// on by (for before and after half way), with the tier's speed and the view's number (a client counts the ticks
/// since it first drew it). Written into its block entity and sent (`native_block_entity_sync`).
fn write_client(game: &mut Game<'_>, dimension: &str, pos: Pos) {
    let number = WRITTEN.get() + 1;
    WRITTEN.set(number);
    let view = with_dim(dimension, |d| {
        let t = d.nets.get(pos)?;
        let stacks: Vec<Tag> = t
            .transit
            .values()
            .filter(|s| s.item.count() > 0)
            .map(|s| {
                let mut c = Compound::new();
                c.insert(key("item"), horizon_client::save::item_to_nbt(&s.item.stack));
                c.insert(key("progress"), Tag::Int(s.progress));
                let mut before = s.clone();
                before.progress = 0;
                let mut after = s.clone();
                after.progress = 50;
                c.insert(key("from"), Tag::Byte(before.side(pos) as i8));
                c.insert(key("to"), Tag::Byte(after.side(pos) as i8));
                if let Some(color) = s.color {
                    c.insert(key("color"), Tag::Int(color.id()));
                }
                Tag::Compound(c)
            })
            .collect();
        let mut view = Compound::new();
        view.insert(key("stacks"), Tag::List(stacks));
        view.insert(key("number"), Tag::Long(number));
        view.insert(key("speed"), Tag::Int(t.speed));
        if let Some(color) = t.color {
            view.insert(key("color"), Tag::Int(color.id()));
        }
        Some(view)
    });
    let Some(view) = view else { return };
    game.native_with_mod_block_entity(pos, |m| {
        m.nbt.insert(key(CLIENT), Tag::Compound(view));
    });
    game.native_block_entity_sync(pos);
}

/// `native_block_entity_update_tag` of a transporter: what `write_client` wrote.
pub(super) fn update_tag(entity: &ModBlockEntity, tag: &mut Option<Compound>) {
    if tag.is_some() || tier_of(&entity.id).is_none() {
        return;
    }
    let mut out = Compound::new();
    out.insert(key(CLIENT), entity.nbt.get(&key(CLIENT)).cloned().unwrap_or_else(|| Tag::Compound(Compound::new())));
    *tag = Some(out);
}

/// `native_block_entity_submit` of a transporter (`RenderLogisticalTransporter`): each stack where it is now,
/// three quarters of its size, as a dropped item.
pub(super) fn draw(game: &Game<'_>, pos: Pos, block: &str, entity: &ModBlockEntity, draw: &mut BlockEntityDraw) {
    if tier_of(block).is_none() {
        return;
    }
    let Some(Tag::Compound(view)) = entity.nbt.get(&key(CLIENT)) else { return };
    let n = |c: &Compound, name: &str| super::mek_world::number(c.get(&key(name)));
    let number = n(view, "number").unwrap_or(0);
    let first = SEEN.with(|s| {
        let mut s = s.borrow_mut();
        let entry = s.entry(pos).or_insert((number, draw.game_time));
        if entry.0 != number {
            *entry = (number, draw.game_time);
        }
        entry.1
    });
    let elapsed = (draw.game_time - first).max(0) as f32;
    let speed = n(view, "speed").unwrap_or(5) as f32;
    let Some(Tag::List(stacks)) = view.get(&key("stacks")) else { return };
    for (i, entry) in stacks.iter().enumerate() {
        let Tag::Compound(c) = entry else { continue };
        let Some(stack) = c.get(&key("item")).and_then(horizon_client::save::item_from_nbt) else { continue };
        // `onUpdateClient`: the client moves it on by the speed a tick, up to 100.
        let progress = (n(c, "progress").unwrap_or(0) as f32 + speed * (elapsed + draw.partial)).min(100.0);
        let side = if progress < 50.0 { n(c, "from") } else { n(c, "to") }.unwrap_or(0).clamp(0, 5) as usize;
        let s = network::SIDES[side];
        let p = progress / 100.0 - 0.5;
        draw.push();
        draw.translate([0.5 + s[0] as f32 * p, 0.25 + s[1] as f32 * p, 0.5 + s[2] as f32 * p]);
        draw.scale([0.75; 3]);
        game.native_draw_item(draw, &stack, DisplayContext::Ground, None, i as i32);
        draw.pop();
    }
}

// ---- A machine's ejector ----------------------------------------------------------------

/// A machine at `from` puts `stack` into the transporter at `pos` (`TransitRequest.eject` into a transporter's
/// `CursedTransporterItemHandler`: `insert` with no colour): what it didn't take, or none when there's no joined
/// transporter there (and the stack goes the usual way).
pub(super) fn insert_from(game: &mut Game<'_>, pos: Pos, from: Pos, stack: ItemStack) -> Option<ItemStack> {
    let dimension = game.native_dimension();
    if !is_transporter(dimension, pos) {
        return None;
    }
    let offered = item(game, stack.clone());
    let response = with_taken(game, dimension, |d, world| d.nets.insert(world, pos, from, &Request::simple(&offered), None, 0, true));
    let taken = response.map_or(0, |r| r.count);
    write_client(game, dimension, pos);
    game.native_block_entity_changed(pos);
    Some(ItemStack { count: stack.count - taken.min(stack.count), ..stack })
}

// ---- /mek connection, /mek color, /mek probe ---------------------------------------------

/// A transporter's side set and saved; none when `pos` holds no transporter.
pub(super) fn set_connection(game: &mut Game<'_>, pos: Pos, side: Direction, connection: Connection) -> Option<Value> {
    let dimension = game.native_dimension();
    game.native_with_mod_block_entity(pos, |m| {
        tier_of(&m.id)?;
        let mut connections = connections_of(&m.nbt);
        connections[side as usize] = connection;
        write_connections(&mut m.nbt, &connections);
        Some(())
    })
    .flatten()?;
    with_dim(dimension, |d| {
        if let Some(t) = d.nets.get_mut(pos) {
            t.connections[side as usize] = connection;
        }
        for (_, nbt, _, _) in d.pending.iter_mut().filter(|(p, _, _, _)| *p == pos) {
            let mut connections = connections_of(nbt);
            connections[side as usize] = connection;
            write_connections(nbt, &connections);
        }
        d.look.insert(pos);
        d.look.insert(network::offset(pos, side as usize));
    });
    Some(probe(game, pos))
}

/// The colour of the transporter at `pos`, set (the Configurator's `onConfigure`, `/mek color`) and saved; what it
/// was. None when `pos` holds no transporter.
pub(super) fn set_color(game: &mut Game<'_>, pos: Pos, color: Option<Color>) -> Option<Option<Color>> {
    let dimension = game.native_dimension();
    // The block entity first: one placed this tick is read from it at the next.
    let saved = game
        .native_with_mod_block_entity(pos, |m| {
            takes_color(&m.id).then_some(())?;
            let old = color_of(&m.nbt);
            match color {
                Some(c) => m.nbt.insert(key("color"), Tag::Int(c.id())),
                None => m.nbt.remove(&key("color")),
            };
            Some(old)
        })
        .flatten()?;
    let old = with_dim(dimension, |d| {
        let old = d.nets.get_mut(pos).map(|t| std::mem::replace(&mut t.color, color));
        for (_, nbt, _, _) in d.pending.iter_mut().filter(|(p, _, _, _)| *p == pos) {
            match color {
                Some(c) => nbt.insert(key("color"), Tag::Int(c.id())),
                None => nbt.remove(&key("color")),
            };
        }
        d.look.insert(pos);
        d.look.extend((0..6).map(|s| network::offset(pos, s)));
        old
    });
    write_client(game, dimension, pos);
    Some(old.unwrap_or(saved))
}

pub(super) fn color(dimension: &str, pos: Pos) -> Option<Option<Color>> {
    with_dim(dimension, |d| d.nets.get(pos).map(|t| t.color).or_else(|| d.pending.iter().find(|(p, _, _, _)| *p == pos).map(|(_, nbt, _, _)| color_of(nbt))))
}

/// A colour by name (`red`, `dark_blue`, `none`) or ordinal.
pub(super) fn color_named(name: &str) -> Option<Option<Color>> {
    if name == "none" {
        return Some(None);
    }
    if let Ok(id) = name.parse::<i32>() {
        return Some(Some(Color::by_id(id)));
    }
    let wanted = name.replace('_', "").to_ascii_lowercase();
    Color::ALL.iter().find(|c| format!("{c:?}").to_ascii_lowercase() == wanted).map(|c| Some(*c))
}

/// What `/mek probe` answers for a transporter.
pub(super) fn probe(game: &mut Game<'_>, pos: Pos) -> Value {
    let dimension = game.native_dimension();
    let id = game.native_with_mod_block_entity(pos, |m| m.id.clone()).unwrap_or_default();
    let Some(tier) = tier_of(&id) else {
        return json!({ "pos": pos, "id": id, "error": "not a transporter" });
    };
    let faces: serde_json::Map<String, Value> =
        Direction::ALL.iter().map(|&d| (d.name().to_owned(), game.native_block_property(pos, d.name()).unwrap_or_default().into())).collect();
    with_dim(dimension, |d| {
        let network = d.nets.network_of(pos);
        let Some(t) = d.nets.get(pos) else {
            return json!({ "pos": pos, "id": id, "error": "not loaded" });
        };
        let transit: Vec<Value> = t
            .transit
            .values()
            .map(|s| {
                json!({
                    "item": &*s.item.stack.item, "count": s.item.count(), "progress": s.progress, "path_type": format!("{:?}", s.path_type()),
                    "dest": s.dest(), "home": s.home, "color": s.color.map(|c| format!("{c:?}")), "idle_dir": s.idle_dir,
                })
            })
            .collect();
        json!({
            "pos": pos, "id": id, "tier": tier.name(), "speed": t.speed, "pull": t.pull, "color": t.color.map(|c| format!("{c:?}")),
            "connections": t.connections.iter().map(|c| c.name()).collect::<Vec<_>>(), "faces": faces, "network": network,
            "delay": t.delay, "transit": transit, "loads": d.loads, "unloads": d.unloads,
        })
    })
}
