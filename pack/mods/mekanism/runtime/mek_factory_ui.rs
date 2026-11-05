// The screens of the Combiner, the Precision Sawmill and the factories
// (support module `crate::game::mek_factory_ui` of the `mekanism` native
// bundle), drawn like Mekanism 10.7's `GuiCombiner`, `GuiPrecisionSawmill` and
// `GuiFactory` with Mekanism's own GUI textures, on `mek_ui`'s pen, tabs and
// windows: their slots, power bar and progress (a factory's down arrow for
// each process), a factory's sorting tab (`GuiSortingTab`), and the side
// configuration and upgrade windows as the other machines have them. The
// ultimate factory is 34 pixels wider, the sawing factory 21 taller, as Mekanism
// draws them; the upgrade component's slots sit beside the panel's right edge.

use super::{
    Game,
    mek_ui::{self, MachineHit, Pen, Rect, inside},
    native_hooks::NativeScreen,
};
use horizon_client::hud::HudBatch;
use mekanism_energy::{
    config::EnergyConfig,
    display,
    factory::{self, FactoryTier, FactoryView, ProcessKind},
    machine::{self, MachineKind},
    menu::{MACHINE_SLOTS, MachineButton, MachineView},
};
use std::cell::Cell;

/// `GuiSortingTab` (`GuiInsetElement` at (-26, 62), 35 high, an 18 pixel
/// button inside it 4 pixels in).
const SORT_TAB: Rect = [-26.0, 62.0, 26.0, 35.0];
const SORT_BUTTON: Rect = [-21.0, 66.0, 18.0, 18.0];

/// Where one of these screens puts things.
struct Geometry {
    width: f32,
    height: f32,
    inventory: [f32; 2],
    /// The "Inventory" label.
    label: [f32; 2],
    title_y: f32,
    power_bar: Rect,
    /// Each container slot's item position (the upgrade component's two
    /// beside the panel), none for the rest of the menu.
    slots: Vec<Option<[f32; 2]>>,
    /// Each process's progress: a factory's `DOWN` arrows, a machine's `BAR`.
    progress: Vec<Rect>,
    /// The combiner's and the sawmill's up arrow (`GuiUpArrow`).
    arrow: Option<[f32; 2]>,
}

fn geometry(kind: ProcessKind) -> Geometry {
    let layout = kind.layout();
    let menu = layout.menu().1;
    let mut slots: Vec<Option<[f32; 2]>> = vec![None; menu];
    let mut put = |slot: usize, at: [f32; 2]| {
        if let Some(cell) = slots.get_mut(slot) {
            *cell = Some(at);
        }
    };
    match kind {
        ProcessKind::Machine(MachineKind::Combiner) => {
            put(layout.inputs[0], [64.0, 17.0]);
            put(layout.extra.unwrap_or(1), [64.0, 53.0]);
            put(layout.outputs[0], [116.0, 35.0]);
            put(layout.energy, [39.0, 35.0]);
            let up = mek_ui::upgrade_slots(176.0);
            put(layout.upgrade, up[0]);
            put(layout.upgrade_out, up[1]);
            Geometry {
                width: 176.0,
                height: 166.0,
                inventory: [8.0, 84.0],
                label: [8.0, 72.0],
                title_y: 6.0,
                power_bar: [164.0, 15.0, 6.0, 54.0],
                slots,
                progress: vec![[86.0, 38.0, 25.0, 9.0]],
                arrow: Some([68.0, 38.0]),
            }
        }
        ProcessKind::Machine(_) => {
            // The Precision Sawmill.
            put(layout.inputs[0], [56.0, 17.0]);
            put(layout.outputs[0], [116.0, 35.0]);
            if let Some(&s) = layout.secondaries.first() {
                put(s, [132.0, 35.0]);
            }
            put(layout.energy, [56.0, 53.0]);
            let up = mek_ui::upgrade_slots(176.0);
            put(layout.upgrade, up[0]);
            put(layout.upgrade_out, up[1]);
            Geometry {
                width: 176.0,
                height: 166.0,
                inventory: [8.0, 84.0],
                label: [8.0, 72.0],
                title_y: 6.0,
                power_bar: [164.0, 15.0, 6.0, 54.0],
                slots,
                progress: vec![[78.0, 38.0, 25.0, 9.0]],
                arrow: Some([60.0, 38.0]),
            }
        }
        ProcessKind::Factory(f) => {
            let ultimate = f.tier == FactoryTier::Ultimate;
            let sawing = kind.saws();
            let width = if ultimate { 210.0 } else { 176.0 };
            // `TileEntityItemToItemFactory.addSlots`' columns.
            let (base, step) = match f.tier {
                FactoryTier::Basic => (55.0, 38.0),
                FactoryTier::Advanced => (35.0, 26.0),
                FactoryTier::Elite => (29.0, 19.0),
                FactoryTier::Ultimate => (27.0, 19.0),
            };
            let mut progress = Vec::new();
            for i in 0..f.tier.processes() {
                let x = base + step * i as f32;
                put(layout.inputs[i], [x, 13.0]);
                put(layout.outputs[i], [x, 57.0]);
                if let Some(&s) = layout.secondaries.get(i) {
                    put(s, [x, 77.0]);
                }
                progress.push([x + 4.0, 33.0, 8.0, 20.0]);
            }
            if let Some(extra) = layout.extra {
                put(extra, [7.0, 57.0]);
            }
            put(layout.energy, [7.0, 13.0]);
            let up = mek_ui::upgrade_slots(width);
            put(layout.upgrade, up[0]);
            put(layout.upgrade_out, up[1]);
            let inventory_y = if sawing { 105.0 } else { 85.0 };
            Geometry {
                width,
                height: if sawing { 187.0 } else { 166.0 },
                inventory: [if ultimate { 26.0 } else { 8.0 }, inventory_y],
                label: [if ultimate { 26.0 } else { 8.0 }, inventory_y - 10.0],
                title_y: 4.0,
                power_bar: [width - 12.0, 16.0, 6.0, if sawing { 75.0 } else { 54.0 }],
                slots,
                progress,
                arrow: None,
            }
        }
    }
}

/// `native_screen` of one of these blocks.
pub(super) fn screen(kind: ProcessKind) -> NativeScreen {
    mek_ui::PAGE.with(|p| p.set(0));
    let g = geometry(kind);
    NativeScreen {
        width: g.width,
        height: g.height,
        slots: g.slots,
        inventory_y: g.inventory[1],
        inventory_x: g.inventory[0],
        own_labels: true,
        own_background: false,
    }
}

fn views(kind: ProcessKind, data: &[i32]) -> (MachineView, FactoryView) {
    let view = MachineView::unpack(data).unwrap_or_default();
    let extra = FactoryView::unpack(data, MACHINE_SLOTS)
        .unwrap_or_else(|| FactoryView { processes: kind.processes() as u8, progress: vec![view.progress], ..FactoryView::default() });
    (view, extra)
}

fn progress_of(view: &MachineView, extra: &FactoryView, process: usize) -> f64 {
    let done = if extra.progress.is_empty() { view.progress } else { extra.progress.get(process).copied().unwrap_or(0) };
    if view.required == 0 { 0.0 } else { (f64::from(done) / f64::from(view.required)).min(1.0) }
}

enum Hit {
    Machine(MachineHit),
    Sort,
}

fn hit(kind: ProcessKind, page: u8, view: &MachineView, extra: &FactoryView, settings: &EnergyConfig, at: [f32; 2], right: bool) -> Option<Hit> {
    let g = geometry(kind);
    if let Some(found) = mek_ui::window_hit(page, view, &machine::MACHINE_UPGRADES, settings, at, right, g.width) {
        return found.map(Hit::Machine);
    }
    if kind.is_factory() && inside(SORT_BUTTON, at) {
        return Some(Hit::Sort);
    }
    if inside(g.power_bar, at) {
        return Some(Hit::Machine(MachineHit::Tip(vec![display::fraction(view.stored, view.capacity, settings)])));
    }
    for (i, r) in g.progress.iter().enumerate() {
        if inside(*r, at) {
            let share = progress_of(view, extra, i);
            return Some(Hit::Machine(MachineHit::Tip(vec![format!("Progress: {}%", (share * 100.0).floor())])));
        }
    }
    None
}

/// `native_screen_click`.
pub(super) fn click(kind: ProcessKind, data: &[i32], at: [f32; 2], right: bool, button: &mut Option<i32>) {
    let (view, extra) = views(kind, data);
    let page = mek_ui::PAGE.with(Cell::get);
    match hit(kind, page, &view, &extra, &mek_ui::settings(), at, right) {
        Some(Hit::Sort) => *button = Some(factory::SORT_BUTTON),
        Some(Hit::Machine(MachineHit::Press(pressed))) => {
            if let MachineButton::Config(mekanism_energy::menu::Button::Page(p)) = pressed {
                mek_ui::PAGE.with(|c| c.set(p));
            }
            *button = Some(pressed.id());
        }
        _ => {}
    }
}

/// `native_screen_tooltip`.
pub(super) fn tooltip(kind: ProcessKind, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    let (view, extra) = views(kind, data);
    let page = mek_ui::PAGE.with(Cell::get);
    let g = geometry(kind);
    if inside(mek_ui::CONFIG_BUTTON, at) {
        lines.push("Side Configuration".into());
        return;
    }
    if inside(mek_ui::beside_panel([180.0, 10.0, 18.0, 18.0], g.width), at) {
        lines.push("Upgrades".into());
        return;
    }
    match hit(kind, page, &view, &extra, &mek_ui::settings(), at, false) {
        Some(Hit::Sort) => lines.push("Auto-sort".into()),
        Some(Hit::Machine(found)) => lines.extend(mek_ui::hit_lines(&view, Some(found))),
        None => {}
    }
}

/// `native_screen_draw`.
pub(super) fn draw(game: &Game<'_>, kind: ProcessKind, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let cursor = game.native_cursor();
    let cursor = [cursor[0] - origin[0], cursor[1] - origin[1]];
    let mut pen = Pen { game, batch, origin, cursor };
    let pen = &mut pen;
    let settings = mek_ui::settings();
    let (view, extra) = views(kind, data);
    let page = mek_ui::PAGE.with(Cell::get);
    let g = geometry(kind);
    let layout = kind.layout();
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [0.0, 0.0, g.width, g.height]);
    mek_ui::draw_tabs(pen, &view, &settings, g.width);
    if kind.is_factory() {
        // `GuiSortingTab`: its holder, the button and On or Off under it.
        pen.holder(SORT_TAB, true);
        pen.image_button("sorting", 18.0, SORT_BUTTON, true);
        let state = if extra.sorting { "On" } else { "Off" };
        pen.centered(state, SORT_TAB[0], SORT_TAB[2], SORT_TAB[1] + 24.0, mek_ui::TITLE);
    }

    // The slots, coloured by what they are (`SlotType.get`); the sawmill's
    // outputs share one wide frame.
    let givers = layout.givers();
    for (slot, at) in g.slots.iter().enumerate().take(layout.count - 2) {
        let Some([x, y]) = *at else { continue };
        if kind == ProcessKind::Machine(MachineKind::PrecisionSawmill) && givers.contains(&slot) {
            continue;
        }
        let (frame, overlay) = if givers.contains(&slot) {
            ("output", None)
        } else if slot == layout.energy {
            ("power", Some("overlay_power"))
        } else if Some(slot) == layout.extra {
            ("extra", None)
        } else {
            ("input", None)
        };
        pen.slot(frame, overlay, x - 1.0, y - 1.0);
    }
    if kind == ProcessKind::Machine(MachineKind::PrecisionSawmill) {
        pen.whole("slot/output_wide", [42.0, 26.0], [111.0, 30.0, 42.0, 26.0]);
    }

    // The power bar (`GuiVerticalPowerBar`): the energy from the bottom.
    let bar = g.power_bar;
    pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], bar);
    let level = if view.capacity == 0 { 0.0 } else { view.stored as f64 / view.capacity as f64 };
    let inner = bar[3] - 2.0;
    let filled = (level * f64::from(inner)) as i32;
    if filled > 0 {
        let h = filled as f32;
        pen.sprite("bar/vertical_power", [0.0, 0.0, 4.0, h.min(52.0)], [bar[0] + 1.0, bar[1] + bar[3] - 1.0 - h, 4.0, h]);
    }

    // The progress: a factory's arrow down by process, a machine's bar.
    if let Some([x, y]) = g.arrow {
        pen.whole("up_arrow", [8.0, 10.0], [x, y, 8.0, 10.0]);
    }
    for (i, r) in g.progress.iter().enumerate() {
        let [px, py, pw, ph] = *r;
        let share = progress_of(&view, &extra, i);
        if kind.is_factory() {
            // `ProgressType.DOWN`: the empty arrow, then the part done from the top.
            pen.sprite("progress/down", [0.0, 0.0, pw, ph], *r);
            let done = (share * f64::from(ph)) as i32 as f32;
            if done > 0.0 {
                pen.sprite("progress/down", [pw, 0.0, pw, done], [px, py, pw, done]);
            }
        } else {
            pen.sprite("progress/bar", [0.0, 0.0, pw, ph], *r);
            let done = (share * f64::from(pw - 2.0)) as i32 as f32;
            if done > 0.0 {
                pen.sprite("progress/bar", [1.0, ph, done, ph], [px + 1.0, py, done, ph]);
            }
        }
    }

    // The player's inventory, its title and label.
    let [ix, iy] = g.inventory;
    for row in 0..4 {
        let y = if row == 3 { iy + 57.0 } else { iy - 1.0 + 18.0 * row as f32 };
        for col in 0..9 {
            pen.slot("normal", None, ix - 1.0 + 18.0 * col as f32, y);
        }
    }
    let title = pen.game.native_screen_title().unwrap_or_default();
    pen.centered(&title, 0.0, g.width, g.title_y, mek_ui::TITLE);
    pen.text("Inventory", g.label, mek_ui::TITLE);
    mek_ui::draw_windows(pen, &view, &machine::MACHINE_UPGRADES, page, &settings);
}
