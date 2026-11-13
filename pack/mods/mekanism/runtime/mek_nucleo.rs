// The Antiprotonic Nucleosynthesizer's screen (support module `crate::game::mek_nucleo` of the
// `mekanism` native bundle), drawn like Mekanism 10.7's `GuiAntiprotonicNucleosynthesizer` with
// Mekanism's own GUI textures, on `mek_ui`'s pen, tabs and windows: a panel 20 pixels wider and 27
// taller than a machine's, the antimatter and energy gauges (`GaugeType.SMALL_MED`) over the
// chemical and energy slots, the inner screen with the process rate (the Joules the last tick took
// over a run's), the progress along the bottom (`GuiDynamicHorizontalRateBar`, from grey to purple),
// the energy tab with what the last tick took, and the side configuration and upgrade windows as
// the other machines have them. Its tick is `mek_machines`' (`mekanism_energy::nucleo`); the
// lightning between its slots (`BoltRenderer`) is not drawn.

use super::{
    Game, mek_chem_machines,
    mek_ui::{self, MachineHit, Pen, Rect, inside},
    native_hooks::NativeScreen,
};
use horizon_client::hud::HudBatch;
use mekanism_energy::{
    config::EnergyConfig,
    display,
    machine::MachineKind,
    menu::{Button, MachineButton, MachineView},
    nucleo,
};
use std::cell::Cell;

const KIND: MachineKind = MachineKind::AntiprotonicNucleosynthesizer;
/// `imageWidth + 20`, `imageHeight + 27`.
const WIDTH: f32 = 196.0;
const HEIGHT: f32 = 193.0;
/// The container's `offset(10, 27)`: the player's inventory at (18, 111).
const INVENTORY: [f32; 2] = [18.0, 111.0];
/// `inventoryLabelY = imageHeight - 93`.
const LABEL: [f32; 2] = [8.0, 100.0];
/// `GuiInnerScreen` at (45, 18), 104x68.
const SCREEN: Rect = [45.0, 18.0, 104.0, 68.0];
/// `GuiChemicalGauge` and `GuiEnergyGauge`, `SMALL_MED` (18x48), at (5, 18) and (172, 18).
const CHEMICAL_GAUGE: Rect = [5.0, 18.0, 18.0, 48.0];
const ENERGY_GAUGE: Rect = [172.0, 18.0, 18.0, 48.0];
/// `GuiDynamicHorizontalRateBar` at (5, 88), 183 wide inside its border.
const BAR: Rect = [5.0, 88.0, 185.0, 10.0];
/// Its colour from the left to the right (`Color.rgbi(60, 45, 74)` to `rgbi(100, 30, 170)`).
const BAR_FROM: [f32; 3] = [60.0, 45.0, 74.0];
const BAR_TO: [f32; 3] = [100.0, 30.0, 170.0];

/// Its slots (`getInitialInventory`): chemical, input, output, energy, then the upgrade component's.
fn slots() -> Vec<Option<[f32; 2]>> {
    let mut slots = vec![Some([6.0, 69.0]), Some([26.0, 40.0]), Some([152.0, 40.0]), Some([173.0, 69.0])];
    slots.extend(mek_ui::upgrade_slots(WIDTH).map(Some));
    slots.resize(9, None);
    slots
}

pub(super) fn screen() -> NativeScreen {
    mek_ui::PAGE.with(|p| p.set(0));
    NativeScreen {
        width: WIDTH,
        height: HEIGHT,
        slots: slots(),
        inventory_y: INVENTORY[1],
        inventory_x: INVENTORY[0],
        own_labels: true,
        own_background: false,
    }
}

fn share(view: &MachineView) -> f64 {
    if view.required == 0 { 0.0 } else { (f64::from(view.progress) / f64::from(view.required)).min(1.0) }
}

fn hit(page: u8, view: &MachineView, used: u64, settings: &EnergyConfig, at: [f32; 2], right: bool) -> Option<MachineHit> {
    if page == 0 && inside(mek_ui::ENERGY_TAB, at) {
        // `GuiEnergyTab` with `getEnergyUsed`: what the last tick took.
        return Some(MachineHit::Tip(vec![
            format!("Using: {}/t", display::energy(used, settings)),
            format!("Needed: {}", display::energy(view.capacity.saturating_sub(view.stored), settings)),
        ]));
    }
    if let Some(found) = mek_ui::window_hit(page, view, KIND.upgrades(), settings, at, right, WIDTH) {
        return found;
    }
    if inside(ENERGY_GAUGE, at) {
        return Some(MachineHit::Tip(vec![display::fraction(view.stored, view.capacity, settings)]));
    }
    if inside(CHEMICAL_GAUGE, at) {
        return Some(MachineHit::Tip(vec![mek_chem_machines::tank_line(view.infuse, view.infused)]));
    }
    if inside(BAR, at) {
        return Some(MachineHit::Tip(vec![format!("Progress: {}%", (share(view) * 100.0).floor())]));
    }
    None
}

/// `native_screen_click`.
pub(super) fn click(data: &[i32], at: [f32; 2], right: bool, button: &mut Option<i32>) {
    let view = MachineView::unpack(data).unwrap_or_default();
    let page = mek_ui::PAGE.with(Cell::get);
    let used = nucleo::unpack_used(data).unwrap_or(0);
    if let Some(MachineHit::Press(pressed)) = hit(page, &view, used, &mek_ui::settings(), at, right) {
        if let MachineButton::Config(Button::Page(p)) = pressed {
            mek_ui::PAGE.with(|c| c.set(p));
        }
        *button = Some(pressed.id());
    }
}

/// `native_screen_tooltip`.
pub(super) fn tooltip(data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    let view = MachineView::unpack(data).unwrap_or_default();
    let page = mek_ui::PAGE.with(Cell::get);
    if inside(mek_ui::CONFIG_BUTTON, at) {
        lines.push("Side Configuration".into());
        return;
    }
    if inside(mek_ui::beside_panel([180.0, 10.0, 18.0, 18.0], WIDTH), at) {
        lines.push("Upgrades".into());
        return;
    }
    let used = nucleo::unpack_used(data).unwrap_or(0);
    lines.extend(mek_ui::hit_lines(&view, hit(page, &view, used, &mek_ui::settings(), at, false)));
}

/// `native_screen_draw`.
pub(super) fn draw(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let cursor = game.native_cursor();
    let cursor = [cursor[0] - origin[0], cursor[1] - origin[1]];
    let mut pen = Pen { game, batch, origin, cursor };
    let pen = &mut pen;
    let settings = mek_ui::settings();
    let view = MachineView::unpack(data).unwrap_or_default();
    let used = nucleo::unpack_used(data).unwrap_or(0);
    let page = mek_ui::PAGE.with(Cell::get);
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [0.0, 0.0, WIDTH, HEIGHT]);
    mek_ui::draw_tabs(pen, &view, &settings, WIDTH);

    // The slots, coloured by what they are (`SlotType.get`); the chemical slot's `MINUS` overlay.
    let s = KIND.slots();
    for (i, at) in slots().into_iter().enumerate().take(s.count - 2) {
        let Some([x, y]) = at else { continue };
        let (frame, overlay) = if i == s.output {
            ("output", None)
        } else if i == s.energy {
            ("power", Some("overlay_power"))
        } else if Some(i) == s.infusion {
            ("extra", Some("overlay_minus"))
        } else {
            ("input", None)
        };
        pen.slot(frame, overlay, x - 1.0, y - 1.0);
    }

    // The antimatter gauge and the energy gauge.
    mek_chem_machines::gauge(pen, CHEMICAL_GAUGE, view.infuse, view.infused, nucleo::MAX_GAS);
    let r = ENERGY_GAUGE;
    pen.nine("gauge/normal", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], r);
    let inner = r[3] - 2.0;
    let level = if view.capacity == 0 { 0.0 } else { (view.stored as f64 / view.capacity as f64).min(1.0) };
    let mut left = if view.stored > 0 { ((level * f64::from(inner)) as i32 as f32).max(1.0) } else { 0.0 };
    let mut y = r[1] + 1.0 + inner;
    while left > 0.0 {
        let h = left.min(16.0);
        y -= h;
        pen.sprite("energy_fill", [0.0, 16.0 - h, 16.0, h], [r[0] + 1.0, y, 16.0, h]);
        left -= h;
    }
    pen.whole("gauge/small_med", [16.0, 46.0], [r[0] + 1.0, r[1] + 1.0, 16.0, 46.0]);

    // The inner screen and its process rate, centred at its bottom.
    pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], SCREEN);
    let rate = format!("Process Rate: {}", nucleo::process_rate(used, view.per_tick));
    pen.centered(&rate, SCREEN[0], SCREEN[2], SCREEN[1] + SCREEN[3] - 11.0, mek_ui::SCREEN_TEXT);

    // The progress: its frame, then the part done, its colour scaled along the bar.
    pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], BAR);
    let inside_w = BAR[2] - 2.0;
    let done = (share(&view) * f64::from(inside_w)) as i32;
    let step = 3;
    let mut x = 0;
    while x < done {
        let w = step.min(done - x);
        let t = (x as f32 + w as f32 / 2.0) / inside_w;
        let c = |i: usize| (BAR_FROM[i] + (BAR_TO[i] - BAR_FROM[i]) * t).round() as u32;
        let argb = 0xFF00_0000 | (c(0) << 16) | (c(1) << 8) | c(2);
        pen.fill([BAR[0] + 1.0 + x as f32, BAR[1] + 1.0, w as f32, BAR[3] - 2.0], argb);
        x += w;
    }

    // The player's inventory, the title and the label.
    let [ix, iy] = INVENTORY;
    for row in 0..4 {
        let y = if row == 3 { iy + 57.0 } else { iy - 1.0 + 18.0 * row as f32 };
        for col in 0..9 {
            pen.slot("normal", None, ix - 1.0 + 18.0 * col as f32, y);
        }
    }
    let title = game.native_screen_title().unwrap_or_else(|| "Antiprotonic Nucleosynthesizer".into());
    pen.centered(&title, 0.0, WIDTH, 6.0, mek_ui::TITLE);
    pen.text("Inventory", LABEL, mek_ui::TITLE);
    mek_ui::draw_windows(pen, &view, KIND.upgrades(), page, &settings);
}
