// The screens of the Simulation Chamber and the Loot Fabricator (support module `crate::game::hostilenetworks_ui`
// of the `hostilenetworks` native bundle), drawn from the mod's own `gui/sim_chamber.png`, `gui/loot_fabricator.png`
// and `gui/default_gui.png`, which the carry brings from the player's jar. Translated from `SimChamberScreen` and
// `LootFabScreen` (MIT); see this bundle's README.md. The numbers come in the menu's data (`ChamberView`, `FabView`),
// the names and drop lists from the registry both sides read from the same data; the text is the mod's English.

use super::{
    Game,
    hostilenetworks_world::{self as world, LOOT_FABRICATOR, Registry, SIM_CHAMBER},
};
use mod_api::Screen;

/// The width of `text` in GUI pixels (upstream's `native_text_width` takes the batch it would draw into).
fn text_width(game: &Game<'_>, text: &str) -> f32 {
    horizon_client::hud::HudBatch::text_width(&game.gui.atlas, text)
}
use horizon_client::hud::HudBatch;
use hostilenetworks_core::{
    fab,
    model::Model,
    sim::{Fail, Mode, Redstone},
    view::{ChamberView, FabView, QUEUE_SHOWN},
};
use std::cell::Cell;

const CHAMBER: &str = "hostilenetworks:gui/sim_chamber";
const FABRICATOR: &str = "hostilenetworks:gui/loot_fabricator";
const PLAYER: &str = "hostilenetworks:gui/default_gui";
const WHITE: u32 = 0xFFFFFF;
const AQUA: u32 = 0x55FFFF;
const RED: u32 = 0xFF5555;
const GREEN: u32 = 0x55FF55;
const GOLD: u32 = 0xFFAA00;

type Rect = [f32; 4];

fn inside(r: Rect, at: [f32; 2]) -> bool {
    at[0] >= r[0] && at[0] < r[0] + r[2] && at[1] >= r[1] && at[1] < r[1] + r[3]
}

/// The icons of the buttons, which Java draws from vanilla's item textures.
fn redstone_icon(state: i32) -> &'static str {
    ["minecraft:redstone", "minecraft:lever", "minecraft:redstone_torch"][state.clamp(0, 2) as usize]
}

fn redstone_line(state: i32) -> &'static str {
    ["Redstone: Ignored", "Redstone: Off When Powered", "Redstone: On When Powered"][state.clamp(0, 2) as usize]
}

/// `Sim Target`, the tiers and the item names: the last part of an id in words (`self_aware`, `rotten_flesh`).
fn pretty(id: &str) -> String {
    let last = id.rsplit(['/', ':']).next().unwrap_or(id);
    let words: Vec<String> = last
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
        })
        .collect();
    words.join(" ")
}

/// The tier colours of the mod's own tier files (`color`), by name.
fn tier_color(name: &str) -> u32 {
    match name {
        "faulty" => 0xAAAAAA,
        "basic" => GREEN,
        "advanced" => AQUA,
        "superior" => 0xFF55FF,
        "self_aware" => GOLD,
        _ => WHITE,
    }
}

fn model_of<'a>(reg: &'a Registry, index: i32) -> Option<&'a Model> {
    reg.models.get(reg.ids.get(usize::try_from(index - 1).ok()?)?)
}

/// `native_screen`: the sizes and the slots of the two menus. The chamber's hopper menu has a fifth slot, the
/// fabricator's nine-by-two menu an eighteenth: both left out.
pub(super) fn screen(block: &str, screen: &mut Option<Screen>) {
    if screen.is_some() {
        return;
    }
    *screen = match block {
        SIM_CHAMBER => Some(Screen {
            width: 232.0,
            height: 230.0,
            slots: vec![Some([-13.0, 1.0]), Some([176.0, 7.0]), Some([196.0, 7.0]), Some([186.0, 27.0]), None],
            inventory_y: 153.0,
            inventory_x: 36.0,
            own_labels: true,
            own_background: true,
            ..Screen::default()
        }),
        LOOT_FABRICATOR => {
            let mut slots = vec![Some([79.0, 62.0])];
            slots.extend((0..16).map(|i| Some([100.0 + 18.0 * (i % 4) as f32, 7.0 + 18.0 * (i / 4) as f32])));
            slots.push(None);
            Some(Screen { width: 176.0, height: 178.0, slots, inventory_y: 96.0, inventory_x: 8.0, own_labels: true, own_background: true, ..Screen::default() })
        }
        _ => None,
    };
}

/// A sprite part at `(x, y)` from the screen's top left.
struct Pen<'a, 'g> {
    game: &'a Game<'g>,
    batch: &'a mut HudBatch,
    origin: [f32; 2],
}

impl Pen<'_, '_> {
    fn sprite(&mut self, texture: &str, src: Rect, dst: Rect) {
        let dst = [self.origin[0] + dst[0], self.origin[1] + dst[1], dst[2], dst[3]];
        self.game.native_sprite(self.batch, texture, src, dst, None);
    }

    fn text(&mut self, text: &str, x: f32, y: f32, color: u32, shadow: bool) {
        self.game.native_text(self.batch, text, [self.origin[0] + x, self.origin[1] + y], color, shadow);
    }

    fn item(&mut self, item: &str, x: f32, y: f32) {
        self.game.native_item(self.batch, item, [self.origin[0] + x, self.origin[1] + y], 16.0);
    }

    /// An item with its count at the bottom right, as a slot shows one.
    fn stack(&mut self, item: &str, count: u32, x: f32, y: f32) {
        self.item(item, x, y);
        if count > 1 {
            let label = count.to_string();
            let w = text_width(self.game, &label);
            self.text(&label, x + 17.0 - w, y + 9.0, WHITE, true);
        }
    }

    fn fill(&mut self, r: Rect, argb: u32) {
        self.game.native_fill(self.batch, [self.origin[0] + r[0], self.origin[1] + r[1], r[2], r[3]], argb);
    }
}

/// `native_screen_draw`.
pub(super) fn draw(game: &Game<'_>, block: &str, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let mut pen = Pen { game, batch, origin };
    match block {
        SIM_CHAMBER => {
            if let Some(view) = ChamberView::unpack(data) {
                draw_chamber(&mut pen, &world::registry(game), &view);
            }
        }
        LOOT_FABRICATOR => {
            if let Some(view) = FabView::unpack(data) {
                draw_fabricator(&mut pen, &world::registry(game), &view);
            }
        }
        _ => {}
    }
}

// ---- Simulation Chamber ----------------------------------------------------------------------------------------

const CH_ENERGY: Rect = [211.0, 48.0, 7.0, 87.0];
const CH_DATA: Rect = [14.0, 48.0, 7.0, 87.0];
const CH_REDSTONE: Rect = [228.0, 0.0, 18.0, 18.0];
const CH_MODE: Rect = [228.0, 20.0, 18.0, 18.0];

/// Breaks `text` into lines of at most `width` GUI pixels, and at its own `\n`.
fn wrap(game: &Game<'_>, text: &str, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split(' ') {
            let longer = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
            if !line.is_empty() && text_width(game, &longer) > width {
                lines.push(std::mem::replace(&mut line, word.to_owned()));
            } else {
                line = longer;
            }
        }
        lines.push(line);
    }
    lines
}

fn draw_chamber(pen: &mut Pen<'_, '_>, reg: &Registry, v: &ChamberView) {
    pen.sprite(CHAMBER, [0.0, 0.0, 216.0, 141.0], [8.0, 0.0, 216.0, 141.0]);
    // The model slot's frame and the two buttons' backs.
    for at in [[-14.0, 0.0], [CH_REDSTONE[0], CH_REDSTONE[1]], [CH_MODE[0], CH_MODE[1]]] {
        pen.sprite(CHAMBER, [0.0, 141.0, 18.0, 18.0], [at[0], at[1], 18.0, 18.0]);
    }
    pen.item(redstone_icon(v.redstone), CH_REDSTONE[0] + 1.0, CH_REDSTONE[1] + 1.0);
    let training = v.mode == Mode::Training.index();
    pen.item(if training { "minecraft:experience_bottle" } else { "minecraft:ender_eye" }, CH_MODE[0] + 1.0, CH_MODE[1] + 1.0);
    if !v.can_train {
        pen.fill([CH_MODE[0] + 1.0, CH_MODE[1] + 1.0, 16.0, 16.0], 0xA0000000);
    }
    // The dark overlays drop from the top of each bar: energy left, and the model's data towards its next tier.
    let rows = ChamberView::dark_rows(v.energy, v.capacity).clamp(0, 87) as f32;
    pen.sprite(CHAMBER, [18.0, 141.0, 7.0, rows], [CH_ENERGY[0], CH_ENERGY[1], 7.0, rows]);
    let model = model_of(reg, v.model);
    let data_rows = match model {
        Some(_) if v.is_max => 0.0,
        Some(_) => ChamberView::dark_rows(v.data - v.tier_data, v.next_tier_data - v.tier_data).clamp(0, 87) as f32,
        None => 87.0,
    };
    pen.sprite(CHAMBER, [18.0, 141.0, 7.0, data_rows], [CH_DATA[0], CH_DATA[1], 7.0, data_rows]);
    // Java draws the data model with a renderer of its own (its item model is `builtin/entity`); the blank chip stands in.
    if model.is_some() {
        pen.item("hostilenetworks:blank_data_model", -13.0, 1.0);
    }
    pen.sprite(PLAYER, [0.0, 0.0, 176.0, 90.0], [28.0, 145.0, 176.0, 90.0]);

    // `renderLabels`: the percentage of the run, the model's lines and the body's.
    let max = Mode::from_index(v.mode).runtime();
    if v.runtime > 0 {
        let percent = (100.0 * (max - v.runtime) as f32 / max as f32).ceil().min(99.0) as i32;
        pen.text(&format!("{percent}%"), 184.0, 123.0, AQUA, true);
    }
    let tier_name = reg.tiers.all().get(v.tier as usize).map(|t| t.name.as_str()).unwrap_or_default();
    if let Some(m) = model {
        pen.text(&format!("Sim Target: {}", pretty(&m.target)), 18.0, 9.0, WHITE, false);
        pen.text(&format!("Model Tier: {}", pretty(tier_name)), 18.0, 21.0, tier_color(tier_name), false);
        let accuracy = v.accuracy as f32 / 100.0;
        pen.text(&format!("Model Accuracy: {}%", format_percent(accuracy)), 18.0, 33.0, tier_color(tier_name), false);
    }
    let lines: Vec<(String, u32)> = if v.fail != 0 {
        let fail = Fail::from_index(v.fail);
        let text = match fail {
            Fail::None => String::new(),
            Fail::Output => "Cannot begin simulation\nOutput buffers are occupied".into(),
            Fail::Energy => "Cannot begin simulation\nSystem energy levels critical".into(),
            Fail::Input => format!("Cannot begin simulation\nMissing input: {}", model.and_then(|m| m.input.first()).map_or("ERROR".into(), |i| pretty(i))),
            Fail::Model => "Please insert a Data Model to begin the simulation".into(),
            Fail::Faulty => "Insufficient data in model. Please insert a basic model or better".into(),
            Fail::EnergyMidCycle => "Simulation progress paused\nSystem energy levels critical".into(),
            Fail::Redstone => "Simulation progress paused\nIncorrect redstone signal applied".into(),
            Fail::CannotTrain => "Cannot begin simulation\nModel can no longer be trained".into(),
        };
        wrap(pen.game, &text, 174.0).into_iter().map(|l| (l, 0xFF9090)).collect()
    } else if v.runtime > 0 {
        // The run's lines come out one by one as the run goes on (Java's `TickableTextList` types them).
        let done = ((max - v.runtime) as f32 / max as f32 * 7.0).ceil() as usize;
        let status = if v.success > 0 { ("success", GOLD) } else { ("failed", RED) };
        let texts: [String; 7] = if training {
            [
                "> Launching runtime v6.5.1".into(),
                format!("> Iteration #{} started", v.iters),
                "> Loading model from chip memory".into(),
                "> Training on live data".into(),
                "> Adjusting model weights".into(),
                "> Sequence adjusted".into(),
                "> Processing results...".into(),
            ]
        } else {
            [
                "> Launching runtime v6.5.1".into(),
                format!("> Iteration #{} started", v.iters),
                "> Loading model from chip memory".into(),
                "> Assessing threat level".into(),
                "> Engaging enemy".into(),
                format!("> Sequence prediction {}", status.0),
                "> Processing results...".into(),
            ]
        };
        texts
            .into_iter()
            .take(done.min(7))
            .enumerate()
            .map(|(i, t)| (t, if i == 5 && !training { status.1 } else { 0xB0B0B0 }))
            .collect()
    } else {
        Vec::new()
    };
    for (i, (line, color)) in lines.iter().take(9).enumerate() {
        pen.text(line, 29.0, 51.0 + 9.0 * i as f32, *color, false);
    }
}

/// `IAttributeExtension.FORMAT`: up to one decimal.
fn format_percent(p: f32) -> String {
    let s = format!("{:.1}", (p * 10.0).round() / 10.0);
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// `native_screen_click` on the chamber: the redstone and mode buttons.
fn click_chamber(v: &ChamberView, at: [f32; 2], right: bool, button: &mut Option<i32>) {
    if right {
        return;
    }
    if inside(CH_REDSTONE, at) {
        *button = Some(Redstone::from_index(v.redstone).next().index());
    } else if inside(CH_MODE, at) && v.can_train {
        *button = Some(3 + Mode::from_index(v.mode).next().index());
    }
}

fn tooltip_chamber(reg: Option<&Registry>, v: &ChamberView, at: [f32; 2], lines: &mut Vec<String>) {
    let model = reg.and_then(|r| model_of(r, v.model));
    if inside(CH_ENERGY, at) {
        lines.push(format!("{}/{} FE", v.energy, v.capacity));
        if model.is_some() {
            lines.push(format!("Model energy cost: {} FE/t", v.cost));
        }
    } else if inside(CH_DATA, at) && model.is_some() {
        lines.push(if v.is_max { "§cI am at maximum potential".into() } else { format!("{}/{} Data collected", v.data - v.tier_data, v.next_tier_data - v.tier_data) });
    } else if inside([229.0, 1.0, 16.0, 16.0], at) {
        lines.push(redstone_line(v.redstone).into());
    } else if inside([229.0, 19.0, 16.0, 16.0], at) {
        if !v.can_train {
            lines.push("Training Mode is disabled by the server config".into());
        } else if v.mode == Mode::Training.index() {
            lines.extend(["Mode: Training".into(), "§7Simulations increase model data,".into(), "§7but produce no outputs".into(), String::new(), "§bEnergy Cost: 120%".into(), "§bSimulation Speed: 100%".into()]);
        } else {
            lines.extend(["Mode: Inference".into(), "§7Simulations produce output,".into(), "§7but models do not gain data".into(), String::new(), "§bEnergy Cost: 100%".into(), "§bSimulation Speed: 125%".into()]);
        }
    }
}

// ---- Loot Fabricator -------------------------------------------------------------------------------------------

const FAB_ENERGY: Rect = [6.0, 10.0, 7.0, 53.0];
const FAB_REDSTONE: Rect = [-22.0, 0.0, 18.0, 18.0];
const FAB_MODE: Rect = [180.0, 0.0, 18.0, 18.0];
const FAB_CLEAR: Rect = [200.0, 0.0, 18.0, 18.0];
const FAB_PREVIEW: Rect = [79.0, 5.0, 16.0, 16.0];
const FAB_LEFT: Rect = [13.0, 68.0, 29.0, 12.0];
const FAB_RIGHT: Rect = [46.0, 68.0, 29.0, 12.0];
const QUEUE_AT: [f32; 2] = [180.0, 30.0];
const QUEUE_CELLS: usize = 9;

thread_local! {
    /// The palette's page: the screen's own, kept by the client as Java's screen keeps it.
    static PAGE: Cell<usize> = const { Cell::new(0) };
}

fn pages(model: &Model) -> usize {
    model.fab_drops.len().div_ceil(9).max(1)
}

fn page(model: &Model) -> usize {
    PAGE.with(|p| {
        let page = p.get().min(pages(model) - 1);
        p.set(page);
        page
    })
}

/// The palette cell a point is on: `(x, y)` of 3 by 3 on the page, as `(index, rect)`.
fn palette_cell(model: &Model, at: [f32; 2]) -> Option<usize> {
    let base = page(model) * 9;
    (0..9).find(|i| {
        let r = [17.0 + 19.0 * (i % 3) as f32, 9.0 + 19.0 * (i / 3) as f32, 16.0, 16.0];
        base + i < model.fab_drops.len() && inside(r, at)
    })
    .map(|i| base + i)
}

/// The queue cells: how many entries show and whether the last cell is the "+N" overflow.
fn queue_shape(len: i32) -> (usize, bool) {
    let overflow = len as usize > QUEUE_CELLS;
    (if overflow { QUEUE_CELLS - 1 } else { len as usize }, overflow)
}

fn queue_rect(i: usize) -> Rect {
    [QUEUE_AT[0] + 18.0 * (i % 3) as f32, QUEUE_AT[1] + 18.0 * (i / 3) as f32, 18.0, 18.0]
}

fn draw_fabricator(pen: &mut Pen<'_, '_>, reg: &Registry, v: &FabView) {
    pen.sprite(FABRICATOR, [0.0, 0.0, 176.0, 83.0], [0.0, 0.0, 176.0, 83.0]);
    let energy = (53.0 * v.energy as f32 / v.capacity.max(1) as f32).floor().clamp(0.0, 53.0);
    pen.sprite(FABRICATOR, [0.0, 83.0, 7.0, energy], [6.0, 10.0 + 53.0 - energy, 7.0, energy]);
    let progress = (35.0 * v.runtime as f32 / fab::RUNTIME as f32).floor().clamp(0.0, 35.0);
    pen.sprite(FABRICATOR, [7.0, 83.0, 6.0, progress], [84.0, 23.0 + 35.0 - progress, 6.0, progress]);
    pen.sprite(PLAYER, [0.0, 0.0, 176.0, 90.0], [0.0, 88.0, 176.0, 90.0]);
    pen.sprite(FABRICATOR, [31.0, 83.0, 18.0, 18.0], [FAB_REDSTONE[0], FAB_REDSTONE[1], 18.0, 18.0]);
    pen.item(redstone_icon(v.redstone), FAB_REDSTONE[0] + 1.0, FAB_REDSTONE[1] + 1.0);
    let Some(model) = model_of(reg, v.model) else { return };
    let (queue, current) = (v.mode == 1, page(model));

    // The palette: the selected drop's frame (fixed mode), then the drops of the page.
    let selected = v.selected - 1;
    if !queue && selected >= 0 && selected as usize / 9 == current {
        let i = selected as usize % 9;
        pen.sprite(FABRICATOR, [31.0, 83.0, 18.0, 18.0], [16.0 + 19.0 * (i % 3) as f32, 8.0 + 19.0 * (i / 3) as f32, 18.0, 18.0]);
    }
    if let Some(drop) = usize::try_from(selected).ok().and_then(|i| model.fab_drops.get(i)) {
        pen.stack(&drop.item, drop.count, FAB_PREVIEW[0], FAB_PREVIEW[1]);
    }
    for i in 0..9 {
        let Some(drop) = model.fab_drops.get(current * 9 + i) else { break };
        pen.stack(&drop.item, drop.count, 17.0 + 19.0 * (i % 3) as f32, 9.0 + 19.0 * (i / 3) as f32);
    }
    for (rect, text, shown) in [(FAB_LEFT, "<", current > 0), (FAB_RIGHT, ">", current + 1 < pages(model))] {
        if shown {
            pen.fill(rect, 0xFF404040);
            pen.text(text, rect[0] + 12.0, rect[1] + 2.0, WHITE, true);
        }
    }

    pen.sprite(FABRICATOR, [31.0, 83.0, 18.0, 18.0], [FAB_MODE[0], FAB_MODE[1], 18.0, 18.0]);
    pen.item(if queue { "minecraft:knowledge_book" } else { "minecraft:item_frame" }, FAB_MODE[0] + 1.0, FAB_MODE[1] + 1.0);
    if queue {
        pen.sprite(FABRICATOR, [31.0, 83.0, 18.0, 18.0], [FAB_CLEAR[0], FAB_CLEAR[1], 18.0, 18.0]);
        pen.item("minecraft:barrier", FAB_CLEAR[0] + 1.0, FAB_CLEAR[1] + 1.0);
        pen.text("Current Queue", QUEUE_AT[0], 20.0, AQUA, false);
        let (shown, overflow) = queue_shape(v.queue_len);
        let cursor = if v.queue_len > 0 { v.cursor as usize } else { usize::MAX };
        for i in 0..QUEUE_CELLS {
            let r = queue_rect(i);
            pen.sprite(FABRICATOR, [if i < shown && i == cursor { 31.0 } else { 13.0 }, 83.0, 18.0, 18.0], r);
            if i < shown.min(QUEUE_SHOWN) {
                let entry = v.queue[i] - 1;
                if let Some(drop) = usize::try_from(entry).ok().and_then(|e| model.fab_drops.get(e)) {
                    pen.stack(&drop.item, drop.count, r[0] + 1.0, r[1] + 1.0);
                }
            } else if i == QUEUE_CELLS - 1 && overflow {
                let label = format!("+{}", v.queue_len as usize - shown);
                let w = text_width(pen.game, &label);
                pen.text(&label, r[0] + 1.0 + (16.0 - w) / 2.0, r[1] + 2.0 + 3.5, WHITE, true);
            }
        }
    }
}

fn click_fabricator(reg: &Registry, v: &FabView, at: [f32; 2], right: bool, button: &mut Option<i32>) {
    if right {
        return;
    }
    if inside(FAB_REDSTONE, at) {
        *button = Some(fab::REDSTONE_BASE + Redstone::from_index(v.redstone).next().index());
        return;
    }
    let Some(model) = model_of(reg, v.model) else { return };
    let (queue, current) = (v.mode == 1, page(model));
    if inside(FAB_LEFT, at) && current > 0 {
        PAGE.with(|p| p.set(current - 1));
    } else if inside(FAB_RIGHT, at) && current + 1 < pages(model) {
        PAGE.with(|p| p.set(current + 1));
    } else if inside(FAB_MODE, at) {
        *button = Some(fab::BTN_CYCLE_MODE);
    } else if let Some(index) = palette_cell(model, at) {
        if queue || v.selected - 1 != index as i32 {
            *button = Some(fab::DROP_BASE + index as i32);
        }
    } else if v.selected > 0 && inside(FAB_PREVIEW, at) {
        if queue && v.queue_len > 0 {
            *button = Some(fab::QUEUE_REMOVE_BASE + v.cursor);
        } else if !queue {
            *button = Some(fab::BTN_CLEAR_FIXED);
        }
    } else if queue {
        if inside(FAB_CLEAR, at) {
            *button = Some(fab::BTN_CLEAR_QUEUE);
            return;
        }
        let (shown, _) = queue_shape(v.queue_len);
        if let Some(i) = (0..shown.min(QUEUE_SHOWN)).find(|i| inside(queue_rect(*i), at)) {
            *button = Some(fab::QUEUE_REMOVE_BASE + i as i32);
        }
    }
}

/// An item as a tooltip names it: its count and its name in words.
fn drop_line(model: &Model, index: usize) -> Option<String> {
    let drop = model.fab_drops.get(index)?;
    Some(format!("{}x {}", drop.count, pretty(&drop.item)))
}

fn tooltip_fabricator(reg: Option<&Registry>, v: &FabView, at: [f32; 2], lines: &mut Vec<String>) {
    if inside(FAB_ENERGY, at) {
        lines.push(format!("{}/{} FE", v.energy, v.capacity));
        lines.push(format!("Energy cost: {} FE/t", v.cost));
        return;
    }
    if inside(FAB_REDSTONE, at) {
        lines.push(redstone_line(v.redstone).into());
        return;
    }
    let Some(model) = reg.and_then(|r| model_of(r, v.model)) else { return };
    let queue = v.mode == 1;
    if inside(FAB_MODE, at) {
        lines.push(if queue { "Mode: Queue" } else { "Mode: Fixed" }.into());
    } else if let Some(index) = palette_cell(model, at) {
        lines.extend(drop_line(model, index));
        if queue {
            lines.push("§7Click to add to queue".into());
        }
    } else if v.selected > 0 && inside(FAB_PREVIEW, at) {
        lines.extend(drop_line(model, v.selected as usize - 1));
        lines.push(if queue { "§7Click to clear current item" } else { "§7Click to clear selection" }.into());
    } else if queue && inside(FAB_CLEAR, at) {
        lines.push("Click to clear queue".into());
    } else if queue {
        let (shown, overflow) = queue_shape(v.queue_len);
        if let Some(i) = (0..shown.min(QUEUE_SHOWN)).find(|i| inside(queue_rect(*i), at)) {
            lines.extend(drop_line(model, v.queue[i] as usize - 1));
            lines.push("§7Click to remove from queue".into());
        } else if overflow && inside(queue_rect(QUEUE_CELLS - 1), at) {
            lines.push(format!("+{} more in the queue", v.queue_len as usize - shown));
        }
    }
}

// ---- The hooks -------------------------------------------------------------------------------------------------

/// `native_screen_click`.
pub(super) fn click(game: &Game<'_>, block: &str, data: &[i32], at: [f32; 2], right: bool, button: &mut Option<i32>) {
    if button.is_some() {
        return;
    }
    match block {
        SIM_CHAMBER => {
            if let Some(v) = ChamberView::unpack(data) {
                click_chamber(&v, at, right, button);
            }
        }
        LOOT_FABRICATOR => {
            if let Some(v) = FabView::unpack(data) {
                click_fabricator(&world::registry(game), &v, at, right, button);
            }
        }
        _ => {}
    }
}

/// `native_screen_tooltip`: the registry is the one the screen's last frame made.
pub(super) fn tooltip(block: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    let reg = world::cached_registry();
    match block {
        SIM_CHAMBER => {
            if let Some(v) = ChamberView::unpack(data) {
                tooltip_chamber(reg.as_deref(), &v, at, lines);
            }
        }
        LOOT_FABRICATOR => {
            if let Some(v) = FabView::unpack(data) {
                tooltip_fabricator(reg.as_deref(), &v, at, lines);
            }
        }
        _ => {}
    }
}
