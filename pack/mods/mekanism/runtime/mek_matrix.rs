// Mekanism's Induction Matrix in the game (support module `crate::game::mek_matrix`
// of the `mekanism` native bundle). The rules are `mekanism_energy::induction`
// (MIT, from Mekanism 10.7.19, Copyright (c) 2017-2025 Aidan C. Brady); the
// multiblock forms through `mek_multiblock`. This module is the glue: the
// cells' Joules in their own block entities (`energy_containers`, so they stay
// when the matrix breaks and go with a broken cell's item), its energy
// container while formed (`MatrixEnergyContainer`: the transfer of its
// providers each way a tick, queued and landing in the cells at its next
// tick), its ports (`TileEntityInductionPort`: input or output by their
// `active_state`, the Configurator's sneaking use toggling it) as energy
// handlers to cables, cubes and machines and as Forge Energy (hook points API
// 154), the output ports putting out to the block they face
// (`CableUtils.emit`), the two energy slots (`EnergyInventorySlot.drainContainer`,
// `fillContainerOrConvert`), its screen (`GuiInductionMatrix`, with
// `GuiMatrixStats` as a page) and `/mek probe`.

use super::{
    Game,
    mek_multiblock::{self as mb, Kind},
    mek_ui::{self, Pen, Rect},
    mek_world,
    native_hooks::{NativeFeInfo, NativeScreen},
};
use horizon_client::{containers::ModBlockEntity, hud::HudBatch, text::Text};
use horizon_core::geometry::Direction as Face;
use horizon_world::{level::flags, nbt::Tag};
use mekanism_energy::{
    config::{self, EnergyConfig},
    display,
    fe::{self, Conversion, StrictEnergy},
    induction::{self, Cell, InductionConfig, InductionTier, MatrixEnergy},
    multiblock::{Pos, Uuid},
    sides::Direction,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell as StdCell, RefCell},
    collections::HashMap,
    time::{Duration, Instant},
};

pub(super) const PORT: &str = "mekanism:induction_port";

thread_local! {
    static SETTINGS: RefCell<Option<(InductionConfig, config::Stamp, Instant)>> = const { RefCell::new(None) };
    /// Each formed matrix's energy container, by dimension and id.
    static MATRICES: RefCell<HashMap<(String, Uuid), MatrixEnergy>> = RefCell::new(HashMap::new());
    /// Cells' Joules to write into their block entities at the next chance.
    static WRITES: RefCell<Vec<(String, Pos, u64)>> = const { RefCell::new(Vec::new()) };
    /// 0: the matrix; 1: its statistics (`GuiMatrixStats`).
    static PAGE: StdCell<u8> = const { StdCell::new(0) };
}

/// `tiers.toml` `[induction]`, read again within a second of a change.
pub(super) fn settings() -> InductionConfig {
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
                Some(d) => (InductionConfig::load(d), config::stamp(d), Instant::now()),
                None => (InductionConfig::default(), [None; config::FILES.len()], Instant::now()),
            });
        }
        s.as_ref().map(|(c, _, _)| *c).unwrap_or_default()
    })
}

pub(super) fn clear() {
    MATRICES.with(|m| m.borrow_mut().clear());
    WRITES.with(|w| w.borrow_mut().clear());
}

/// An induction cell or provider (`IInternalMultiblock`).
pub(super) fn is_inner(block: &str) -> bool {
    InductionTier::of_cell(block).is_some() || InductionTier::of_provider(block).is_some()
}

pub(super) fn is_matrix_block(block: &str) -> bool {
    block == PORT || block == "mekanism:induction_casing"
}

fn with_matrix<R>(dimension: &str, id: Uuid, change: impl FnOnce(&mut MatrixEnergy) -> R) -> Option<R> {
    MATRICES.with(|m| m.borrow_mut().get_mut(&(dimension.to_owned(), id)).map(change))
}

/// `onCreated` with `MatrixValidator.postcheck`: its cells (their Joules as their block entities hold them) and
/// providers, by position.
pub(super) fn created(game: &mut Game<'_>, dimension: &str, id: Uuid) {
    let Some(internal) = mb::with_formed(dimension, id, |f| f.internal.iter().copied().collect::<Vec<_>>()) else { return };
    let cfg = settings();
    let mut m = MatrixEnergy::default();
    for p in internal {
        let block = game.native_block(p);
        if let Some(tier) = InductionTier::of_cell(&block) {
            let stored = game.native_with_mod_block_entity(p, |e| mek_world::saved_energy(&e.nbt)).flatten().unwrap_or(0);
            m.add_cell(Cell { pos: p, stored, capacity: cfg.cell(tier) });
        } else if let Some(tier) = InductionTier::of_provider(&block) {
            m.add_provider(cfg.provider(tier));
        }
    }
    MATRICES.with(|all| all.borrow_mut().insert((dimension.to_owned(), id), m));
    let ports: Vec<Pos> = mb::with_formed(dimension, id, |f| f.valves.iter().map(|(p, _)| *p).collect()).unwrap_or_default();
    show_modes(game, &ports);
}

/// `MatrixEnergyContainer.invalidate`: what is queued lands in the cells, which keep it.
pub(super) fn removed(dimension: &str, id: Uuid) {
    let Some(mut m) = MATRICES.with(|all| all.borrow_mut().remove(&(dimension.to_owned(), id))) else { return };
    if m.tick() {
        WRITES.with(|w| w.borrow_mut().extend(m.cells.iter().map(|c| (dimension.to_owned(), c.pos, c.stored))));
    }
}

/// Writes the cells' Joules that are due into their block entities.
pub(super) fn flush(game: &mut Game<'_>, dimension: &str) {
    let due: Vec<(Pos, u64)> = WRITES.with(|w| {
        let mut w = w.borrow_mut();
        let (mine, rest): (Vec<_>, Vec<_>) = w.drain(..).partition(|(d, _, _)| d == dimension);
        *w = rest;
        mine.into_iter().map(|(_, p, e)| (p, e)).collect()
    });
    for (p, stored) in due {
        game.native_with_mod_block_entity(p, |m| {
            if InductionTier::of_cell(&m.id).is_some() {
                mek_world::write_energy(&mut m.nbt, stored);
            }
        });
    }
}

/// A port's mode (`getActive`): output when its `active_state` says so.
fn output_mode(entity: &ModBlockEntity) -> bool {
    mek_world::number(entity.nbt.get(&mek_world::key("active_state"))).is_some_and(|v| v != 0)
}

fn port_matrix(dimension: &str, pos: Pos) -> Option<Uuid> {
    mb::formed_at(dimension, Kind::Matrix, pos)
}

// ---- The ports' energy -------------------------------------------------------------------------------------

/// Joules offered to the port at `pos` (`insertEnergy` through its side): taken in its input mode, at most the
/// transfer left this tick and the room left. What it didn't take.
pub(super) fn offer(dimension: &str, pos: Pos, output: bool, amount: u64, simulate: bool) -> u64 {
    if output {
        return amount;
    }
    let Some(id) = port_matrix(dimension, pos) else { return amount };
    with_matrix(dimension, id, |m| m.insert(amount, simulate)).unwrap_or(amount)
}

/// Joules drawn from the port at `pos` (`extractEnergy`): given in its output mode.
pub(super) fn draw_energy(dimension: &str, pos: Pos, output: bool, amount: u64, simulate: bool) -> u64 {
    if !output {
        return 0;
    }
    let Some(id) = port_matrix(dimension, pos) else { return 0 };
    with_matrix(dimension, id, |m| m.extract(amount, simulate)).unwrap_or(0)
}

/// Whether the block at `pos` is an induction port, and its mode, for the cables and ejectors of `mek_world`.
pub(super) fn port_at(game: &mut Game<'_>, pos: Pos) -> Option<bool> {
    game.native_with_mod_block_entity(pos, |m| (m.id == PORT).then(|| output_mode(m))).flatten()
}

/// A port's energy through one side (`ProxiedEnergyContainerHolder`: in while input, out while output; the null side
/// is read only).
struct PortSide<'a> {
    dimension: &'a str,
    pos: Pos,
    output: bool,
    open: bool,
}

impl StrictEnergy for PortSide<'_> {
    fn insert(&mut self, joules: i64, simulate: bool) -> i64 {
        if !self.open || joules <= 0 {
            return joules;
        }
        offer(self.dimension, self.pos, self.output, joules as u64, simulate) as i64
    }

    fn extract(&mut self, joules: i64, simulate: bool) -> i64 {
        if !self.open || joules <= 0 {
            return 0;
        }
        draw_energy(self.dimension, self.pos, self.output, joules as u64, simulate) as i64
    }
}

/// `native_fe_storage` of a port: the matrix's Joules in FE. Whether `entity` was a port.
pub(super) fn fe_storage(dimension: &str, pos: Pos, entity: &ModBlockEntity, config: &EnergyConfig, info: &mut Option<NativeFeInfo>) -> bool {
    if entity.id != PORT {
        return false;
    }
    if config.blacklist_forge {
        return true;
    }
    let rate = Conversion(config.fe_conversion_rate);
    let (stored, capacity) = port_matrix(dimension, pos).and_then(|id| with_matrix(dimension, id, |m| (m.energy(), m.storage_cap))).unwrap_or((0, 0));
    *info = Some(NativeFeInfo {
        stored: i64::from(rate.to_fe_int(stored.min(i64::MAX as u64) as i64)),
        capacity: i64::from(rate.to_fe_int(capacity.min(i64::MAX as u64) as i64)),
        can_receive: true,
        can_extract: true,
    });
    true
}

/// `native_fe_receive` and `native_fe_extract` of a port. Whether `entity` was a port.
#[allow(clippy::too_many_arguments)]
pub(super) fn fe_transfer(dimension: &str, pos: Pos, face: Option<Face>, entity: &ModBlockEntity, config: &EnergyConfig, max: i64, simulate: bool, extract: bool, moved: &mut Option<i64>) -> bool {
    if entity.id != PORT {
        return false;
    }
    if config.blacklist_forge {
        return true;
    }
    let rate = Conversion(config.fe_conversion_rate);
    let mut side = PortSide { dimension, pos, output: output_mode(entity), open: face.is_some() };
    let max = max.clamp(0, i64::from(i32::MAX)) as i32;
    let fe = if extract { fe::extract_fe(&mut side, rate, max, simulate) } else { fe::receive_fe(&mut side, rate, max, simulate) };
    *moved = Some(i64::from(fe));
    true
}

/// The Configurator's sneaking use on a port (`onSneakRightClick`): input and output swap, shown on the block.
pub(super) fn toggle(game: &mut Game<'_>, pos: Pos) -> Option<bool> {
    let now = game.native_with_mod_block_entity(pos, |m| {
        if m.id != PORT {
            return None;
        }
        let now = !output_mode(m);
        m.nbt.insert(mek_world::key("active_state"), Tag::Byte(i8::from(now)));
        Some(now)
    })??;
    game.native_set_block_property_with(pos, "active", if now { "true" } else { "false" }, flags::UPDATE_ALL);
    let player = game.native_acting_id();
    game.native_action_bar(player, Text::literal(format!("Toggled Induction Port transfer mode to {}.", induction::port_name(now))));
    Some(now)
}

/// The ports show their mode on their block (`active`), as the tile sets its state from what it saved.
fn show_modes(game: &mut Game<'_>, ports: &[Pos]) {
    for p in ports {
        let Some(output) = port_at(game, *p) else { continue };
        let shown = if output { "true" } else { "false" };
        if game.native_block_property(*p, "active").as_deref() != Some(shown) {
            game.native_set_block_property_with(*p, "active", shown, flags::UPDATE_ALL);
        }
    }
}

// ---- The tick ---------------------------------------------------------------------------------------------------

/// `MatrixMultiblockData.tick`: what was queued lands in the cells, the charge slot fills an item from the matrix
/// (`drainContainer`), the other slot an item's energy into it or a conversion (`fillContainerOrConvert`), and the
/// output ports put out to what they face, the transfer at most, split among them (`CableUtils.emit`).
pub(super) fn tick(game: &mut Game<'_>, dimension: &str, id: Uuid) {
    let Some(changed) = with_matrix(dimension, id, |m| {
        let before: Vec<u64> = m.cells.iter().map(|c| c.stored).collect();
        m.tick();
        m.cells.iter().zip(before).filter(|(c, b)| c.stored != *b).map(|(c, _)| (c.pos, c.stored)).collect::<Vec<_>>()
    }) else {
        return;
    };
    for (p, stored) in changed {
        game.native_with_mod_block_entity(p, |m| mek_world::write_energy(&mut m.nbt, stored));
    }
    // The slots.
    let items = mb::with_cache(dimension, Kind::Matrix, id, |c| c.items.clone()).unwrap_or_default();
    let mut slots = items.clone();
    if let Some(Some(item)) = slots.get_mut(0) {
        let stored = with_matrix(dimension, id, |m| m.energy()).unwrap_or(0);
        if stored > 0
            && let Some(took) = mek_world::charge_energy_item(item, stored, true).filter(|t| *t > 0)
        {
            let got = with_matrix(dimension, id, |m| m.extract(took, false)).unwrap_or(0);
            if got > 0 {
                mek_world::charge_energy_item(item, got, false);
            }
        }
    }
    if let Some(slot) = slots.get_mut(1)
        && let Some(item) = slot.as_mut()
    {
        let room = with_matrix(dimension, id, |m| u64::MAX / 2 - m.insert(u64::MAX / 2, true)).unwrap_or(0);
        if room > 0 {
            let mut probe = item.clone();
            match mek_world::drain_energy_item(&mut probe, room) {
                Some(taken) if taken > 0 => {
                    with_matrix(dimension, id, |m| m.insert(taken, false));
                    *item = probe;
                }
                Some(_) => {}
                None => {
                    if let Some((count, joules)) = super::mek_machines::energy_conversion(item)
                        && item.count >= count
                        && with_matrix(dimension, id, |m| m.insert(joules, true)) == Some(0)
                    {
                        with_matrix(dimension, id, |m| m.insert(joules, false));
                        item.count -= count;
                        if item.count == 0 {
                            *slot = None;
                        }
                    }
                }
            }
        }
    }
    if slots != items {
        mb::with_cache(dimension, Kind::Matrix, id, |c| c.items = slots);
    }
    emit(game, dimension, id);
}

/// `CableUtils.emit` of the output ports' targets: the block each faces, through its side.
fn emit(game: &mut Game<'_>, dimension: &str, id: Uuid) {
    let Some(valves) = mb::with_formed(dimension, id, |f| f.valves.clone()) else { return };
    if with_matrix(dimension, id, |m| m.energy() == 0).unwrap_or(true) {
        return;
    }
    let (config, _) = mek_world::energy_config();
    let mut targets = Vec::new();
    for (pos, side) in valves {
        if port_at(game, pos) != Some(true) {
            continue;
        }
        if let Some(target) = mek_world::energy_target(game, &config, dimension, pos, side) {
            targets.push((pos, side, target));
        }
    }
    if targets.is_empty() {
        return;
    }
    let cap = with_matrix(dimension, id, |m| m.transfer_cap).unwrap_or(0);
    let to_send = with_matrix(dimension, id, |m| m.extract(cap, true)).unwrap_or(0);
    if to_send == 0 {
        return;
    }
    let rate = Conversion(config.fe_conversion_rate);
    let sent = mekanism_energy::split::send_to_acceptors(targets.len(), to_send, |i, amount, simulate| {
        let (pos, side, target) = targets[i];
        mek_world::energy_hand_over(game, &config, rate, dimension, pos, side, target, amount, simulate)
    });
    if sent > 0 {
        with_matrix(dimension, id, |m| m.extract(sent, false));
    }
}

/// What a cell's drop keeps (`energy_containers`), as an energy cube's does.
pub(super) fn cell_drop(nbt: &horizon_world::nbt::Compound) -> Option<horizon_world::nbt::Compound> {
    let energy = nbt.get(&mek_world::key("energy_containers"))?;
    let mut data = horizon_world::nbt::Compound::new();
    data.insert(mek_world::key("energy_containers"), energy.clone());
    Some(data)
}

pub(super) fn is_cell(block: &str) -> bool {
    InductionTier::of_cell(block).is_some()
}

// ---- The screen (`GuiInductionMatrix`, `GuiMatrixStats`) ------------------------------------------------------------

const DATA: usize = 4 * 6 + 5;

/// The data slots of a formed matrix: its Joules, capacity, last input, last output, transfer cap (four 16-bit
/// pieces each), its cells and providers, and its size.
pub(super) fn view(dimension: &str, id: Uuid) -> Vec<i32> {
    let Some(m) = with_matrix(dimension, id, |m| m.clone()) else { return Vec::new() };
    let size = mb::with_formed(dimension, id, |f| f.cuboid.size()).unwrap_or([0; 3]);
    let mut out = Vec::with_capacity(DATA);
    for v in [m.energy(), m.storage_cap, m.last_input, m.last_output, m.transfer_cap, 0] {
        out.extend((0..4).map(|i| ((v >> (16 * i)) & 0xFFFF) as i32));
    }
    out.extend([m.cells.len() as i32, m.providers as i32, size[0], size[1], size[2]]);
    out
}

struct View {
    energy: u64,
    capacity: u64,
    input: u64,
    output: u64,
    transfer: u64,
    cells: i32,
    providers: i32,
    size: [i32; 3],
}

fn unpack(data: &[i32]) -> View {
    if data.len() < DATA {
        return View { energy: 0, capacity: 0, input: 0, output: 0, transfer: 0, cells: 0, providers: 0, size: [0; 3] };
    }
    let long = |at: usize| (0..4).fold(0u64, |v, i| v | (u64::from(data[at + i] as u16) << (16 * i)));
    View {
        energy: long(0),
        capacity: long(4),
        input: long(8),
        output: long(12),
        transfer: long(16),
        cells: data[24],
        providers: data[25],
        size: [data[26], data[27], data[28]],
    }
}

const GAUGE: Rect = [7.0, 16.0, 34.0, 56.0];
const SCREEN: Rect = [49.0, 21.0, 84.0, 46.0];
const SLOTS: [[f32; 2]; 2] = [[146.0, 21.0], [146.0, 51.0]];
const HOLDER: Rect = [141.0, 16.0, 26.0, 56.0];
/// `GuiMatrixTab`: the statistics tab, left of the panel.
const STATS_TAB: Rect = [-26.0, 6.0, 26.0, 26.0];
const STATS_BUTTON: Rect = [-21.0, 10.0, 18.0, 18.0];
const ENERGY_TAB: Rect = [-26.0, 137.0, 26.0, 26.0];

pub(super) fn screen() -> NativeScreen {
    PAGE.with(|p| p.set(0));
    NativeScreen {
        width: 176.0,
        height: 166.0,
        slots: vec![Some(SLOTS[0]), Some(SLOTS[1]), None, None, None],
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: true,
        own_background: false,
    }
}

/// The statistics tab switches the page on the client (Mekanism opens `GuiMatrixStats`); the press is taken.
pub(super) const STATS: i32 = 7100;

pub(super) fn click(at: [f32; 2], button: &mut Option<i32>) {
    if mek_ui::inside(STATS_BUTTON, at) {
        PAGE.with(|p| p.set(1 - p.get().min(1)));
        *button = Some(STATS);
    }
}

fn lines(view: &View, settings: &EnergyConfig, page: u8) -> Vec<String> {
    if page == 1 {
        return vec![
            format!("Input: {}", display::fraction(view.input, view.transfer, settings)),
            format!("Output: {}", display::fraction(view.output, view.transfer, settings)),
            format!("Dimensions: {} x {} x {}", view.size[0], view.size[1], view.size[2]),
            format!("Constituents: {} cells, {} providers", view.cells, view.providers),
        ];
    }
    vec![
        format!("Energy: {}", display::energy(view.energy, settings)),
        format!("Capacity: {}", display::energy(view.capacity, settings)),
        format!("Input: {}/t", display::energy(view.input, settings)),
        format!("Output: {}/t", display::energy(view.output, settings)),
    ]
}

pub(super) fn tooltip(data: &[i32], at: [f32; 2], tips: &mut Vec<String>) {
    let view = unpack(data);
    let settings = mek_ui::settings();
    if mek_ui::inside(STATS_BUTTON, at) {
        tips.push(if PAGE.with(StdCell::get) == 0 { "Matrix Statistics".into() } else { "Induction Matrix".into() });
    } else if mek_ui::inside(GAUGE, at) || mek_ui::inside(ENERGY_TAB, at) {
        tips.push(format!("Storing: {}", display::fraction(view.energy, view.capacity, &settings)));
        tips.push(format!("Input: {}/t", display::energy(view.input, &settings)));
        tips.push(format!("Output: {}/t", display::energy(view.output, &settings)));
    }
}

pub(super) fn draw(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let view = unpack(data);
    let settings = mek_ui::settings();
    let page = PAGE.with(StdCell::get);
    let cursor = game.native_cursor();
    let cursor = [cursor[0] - origin[0], cursor[1] - origin[1]];
    let mut pen = Pen { game, batch, origin, cursor };
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [0.0, 0.0, 176.0, 166.0]);
    pen.holder(STATS_TAB, true);
    pen.image_button("stats", 18.0, STATS_BUTTON, true);
    let unit = if settings.show_fe { "fe" } else { "j" };
    pen.whole(&format!("tabs/energy_info_{unit}"), [26.0, 26.0], ENERGY_TAB);
    // The medium energy gauge.
    pen.nine("gauge/normal", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], GAUGE);
    let inner = GAUGE[3] - 2.0;
    let level = if view.capacity == 0 { 0.0 } else { (view.energy as f64 / view.capacity as f64).min(1.0) };
    let mut left = if view.energy > 0 { ((level * f64::from(inner)) as i32 as f32).max(1.0) } else { 0.0 };
    let mut y = GAUGE[1] + 1.0 + inner;
    while left > 0.0 {
        let h = left.min(16.0);
        y -= h;
        for col in 0..2 {
            pen.sprite("energy_fill", [0.0, 16.0 - h, 16.0, h], [GAUGE[0] + 1.0 + 16.0 * col as f32, y, 16.0, h]);
        }
        left -= h;
    }
    pen.whole("gauge/medium", [32.0, 54.0], [GAUGE[0] + 1.0, GAUGE[1] + 1.0, 32.0, 54.0]);
    pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], SCREEN);
    for (i, line) in lines(&view, &settings, page).iter().enumerate() {
        let w = game.native_text_width(line);
        let line = if w > SCREEN[2] - 6.0 { line.replace(": ", ":") } else { line.clone() };
        pen.text(&line, [SCREEN[0] + 3.0, SCREEN[1] + 3.0 + 10.0 * i as f32], mek_ui::SCREEN_TEXT);
    }
    pen.nine("element_holder", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], HOLDER);
    pen.slot("normal", Some("overlay_plus"), SLOTS[0][0] - 1.0, SLOTS[0][1] - 1.0);
    pen.slot("normal", Some("overlay_minus"), SLOTS[1][0] - 1.0, SLOTS[1][1] - 1.0);
    for row in 0..4 {
        let y = if row == 3 { 141.0 } else { 83.0 + 18.0 * row as f32 };
        for col in 0..9 {
            pen.slot("normal", None, 7.0 + 18.0 * col as f32, y);
        }
    }
    let title = game.native_screen_title().unwrap_or_else(|| "Induction Matrix".into());
    pen.centered(&title, 0.0, 176.0, 6.0, mek_ui::TITLE);
    pen.text("Inventory", [8.0, 74.0], mek_ui::TITLE);
}

// ---- /mek probe ---------------------------------------------------------------------------------------------------

pub(super) fn probe(dimension: &str, id: Uuid) -> Value {
    let Some(m) = with_matrix(dimension, id, |m| m.clone()) else { return json!({}) };
    json!({
        "energy": m.energy(), "storage_cap": m.storage_cap, "transfer_cap": m.transfer_cap,
        "last_input": m.last_input, "last_output": m.last_output, "queued_input": m.queued_input, "queued_output": m.queued_output,
        "cells": m.cells.iter().map(|c| json!({ "pos": c.pos, "stored": c.stored, "capacity": c.capacity })).collect::<Vec<_>>(),
        "providers": m.providers,
    })
}

/// The side a port faces out of, for `/mek probe` of a port.
pub(super) fn port_probe(game: &mut Game<'_>, pos: Pos) -> Option<Value> {
    let output = port_at(game, pos)?;
    let dimension = game.native_dimension();
    let side = port_matrix(dimension, pos).and_then(|id| mb::with_formed(dimension, id, |f| f.valves.iter().find(|(p, _)| *p == pos).map(|(_, s)| *s)).flatten());
    Some(json!({ "mode": induction::port_name(output), "side": side.map(Direction::name) }))
}
