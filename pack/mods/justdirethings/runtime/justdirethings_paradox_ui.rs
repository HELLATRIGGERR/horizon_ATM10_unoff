// The Paradox Machine's screen (support module `crate::game::justdirethings_paradox_ui` of the `justdirethings` native
// bundle), drawn as Just Dire Things draws it (`BaseMachineScreen`, `ParadoxMachineScreen`) from the mod's own
// `gui/sprites/background` panel, `gui/justslot`, `gui/powerbar`, `gui/fluidbar`, `gui/paradoxbar` and
// `gui/buttons/*` textures, which the carry brings from the player's jar: the title tab, the top section 80 wider than
// the menu with the energy, fluid and paradox bars, the area's radius and offset buttons, the target, render,
// snapshot, redstone and tick speed buttons, and the inventory section. Java's screen hangs its top section 26 pixels
// above the menu's 176x166 and its title tab 20 above that, and centres the wider top section on the menu: here the
// screen is 256 wide and 214 high, every x is Java's plus 40 and every y Java's plus 46. Translated from Just Dire
// Things 1.5.7 (MIT); see this bundle's README.md.

use super::{Game, justdirethings_time::PARADOX};
use horizon_client::hud::HudBatch;
use justdirethings_core::{redstone::Mode, view::ParadoxView};
use mod_api::Screen;

const BACKGROUND: &str = "justdirethings:gui/sprites/background";
const SLOT: &str = "justdirethings:gui/justslot";
const POWERBAR: &str = "justdirethings:gui/powerbar";
const FLUIDBAR: &str = "justdirethings:gui/fluidbar";
const PARADOXBAR: &str = "justdirethings:gui/paradoxbar";
const FLUID: &str = "justdirethings:block/fluid_source";
const LABEL: u32 = 0xFF40_4040;

/// The menu's left edge in this screen (`getGuiLeft()`), and its top (`getGuiTop()`).
const LEFT: f32 = 40.0;
const TOP: f32 = 46.0;
/// `topSectionTop`.
const SECTION: f32 = TOP - 26.0;
const INVENTORY: [f32; 2] = [8.0 + LEFT, 84.0 + TOP];
const ENERGY: [f32; 4] = [5.0, SECTION + 5.0, 18.0, 72.0];
const FLUID_BAR: [f32; 4] = [24.0, SECTION + 5.0, 18.0, 72.0];
const PARADOX_BAR: [f32; 4] = [256.0 - 23.0, SECTION + 5.0, 18.0, 72.0];
const RENDER_AREA: [f32; 4] = [LEFT + 152.0, SECTION + 62.0, 16.0, 16.0];
const REDSTONE: [f32; 4] = [LEFT + 134.0, SECTION + 62.0, 16.0, 16.0];
const SNAPSHOT: [f32; 4] = [LEFT + 116.0, SECTION + 62.0, 16.0, 16.0];
const RENDER_PARADOX: [f32; 4] = [LEFT + 98.0, SECTION + 62.0, 16.0, 16.0];
const TARGET: [f32; 4] = [LEFT + 56.0, SECTION + 38.0, 16.0, 16.0];
const TICK_SPEED: [f32; 4] = [LEFT + 144.0, SECTION + 40.0, 24.0, 12.0];
/// The radius (row 0) and offset (row 1) value buttons of each axis: the minus at x, the plus 30 right of it.
fn value_button(axis: usize, row: usize) -> [f32; 2] {
    [LEFT + 25.0 + 50.0 * axis as f32, SECTION + 12.0 + 15.0 * row as f32]
}

fn inside(r: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= r[0] && at[0] < r[0] + r[2] && at[1] >= r[1] && at[1] < r[1] + r[3]
}

const TARGETS: [(&str, &str); 3] = [
    ("justdirethings:gui/buttons/target-both", "Target: blocks and entities"),
    ("justdirethings:gui/buttons/filter-block", "Target: blocks"),
    ("justdirethings:gui/buttons/passivemob", "Target: entities"),
];

fn redstone(mode: Mode) -> (&'static str, &'static str) {
    match mode {
        Mode::Ignored => ("justdirethings:gui/buttons/redstoneignore", "Redstone: ignored"),
        Mode::Low => ("justdirethings:gui/buttons/redstonelow", "Redstone: runs while unpowered"),
        Mode::High => ("justdirethings:gui/buttons/redstonehigh", "Redstone: runs while powered"),
        Mode::Pulse => ("justdirethings:gui/buttons/redstonepulse", "Redstone: runs once a pulse"),
    }
}

/// `native_screen`: the panel and no slot of its own (the hopper menu's five are left out).
pub(super) fn screen(block: &str, screen: &mut Option<Screen>) {
    if screen.is_some() || block != PARADOX {
        return;
    }
    *screen = Some(Screen {
        width: 256.0,
        height: 166.0 + TOP + 2.0,
        slots: vec![None; 5],
        inventory_x: INVENTORY[0],
        inventory_y: INVENTORY[1],
        own_labels: true,
        own_background: true,
        ..Screen::default()
    });
}

struct Pen<'a, 'g> {
    game: &'a Game<'g>,
    batch: &'a mut HudBatch,
    origin: [f32; 2],
}

impl Pen<'_, '_> {
    fn sprite(&mut self, texture: &str, src: [f32; 4], dst: [f32; 4], tint: Option<u32>) -> bool {
        let dst = [self.origin[0] + dst[0], self.origin[1] + dst[1], dst[2], dst[3]];
        self.game.native_sprite(self.batch, texture, src, dst, tint)
    }

    /// A whole texture (its own size) drawn into `dst`, greyed when `on` is false (`GrayscaleButton`).
    fn button(&mut self, texture: &str, dst: [f32; 4], on: bool) {
        let [w, h] = self.game.native_sprite_size(texture).unwrap_or([16.0, 16.0]);
        self.sprite(texture, [0.0, 0.0, w, h], dst, (!on).then_some(0xFF70_7070));
    }

    /// The mod's `background` sprite as its nine-slice scaling draws it (236x34, borders of 8), stretched.
    fn panel(&mut self, [x, y, w, h]: [f32; 4]) {
        const W: f32 = 236.0;
        const H: f32 = 34.0;
        const B: f32 = 8.0;
        let cols = [(0.0, B, x, B), (B, W - 2.0 * B, x + B, w - 2.0 * B), (W - B, B, x + w - B, B)];
        let rows = [(0.0, B, y, B), (B, H - 2.0 * B, y + B, h - 2.0 * B), (H - B, B, y + h - B, B)];
        for (su, sw, dx, dw) in cols {
            for (sv, sh, dy, dh) in rows {
                if dw > 0.0 && dh > 0.0 {
                    self.sprite(BACKGROUND, [su, sv, sw, sh], [dx, dy, dw, dh], None);
                }
            }
        }
    }

    fn slot(&mut self, at: [f32; 2]) {
        self.sprite(SLOT, [0.0, 0.0, 18.0, 18.0], [at[0] - 1.0, at[1] - 1.0, 18.0, 18.0], None);
    }

    fn text(&mut self, text: &str, x: f32, y: f32) {
        self.game.native_text(self.batch, text, [self.origin[0] + x, self.origin[1] + y], LABEL, false);
    }

    /// A 36x72 bar texture's empty half at `r` (the texture may be narrower than the 36 Java blits it as).
    fn bar(&mut self, texture: &str, r: [f32; 4]) -> f32 {
        let u = self.game.native_sprite_size(texture).map_or(1.0, |[w, _]| w / 36.0);
        self.sprite(texture, [0.0, 0.0, 18.0 * u, 72.0], r, None);
        u
    }

    /// The lit part of a bar: `rows` of 70, from the texture's right half.
    fn fill(&mut self, texture: &str, r: [f32; 4], u: f32, rows: i32, tint: Option<u32>) {
        let rows = rows.clamp(0, 70) as f32;
        if rows > 0.0 {
            self.sprite(texture, [19.0 * u, 69.0 - rows, 17.0 * u, rows + 1.0], [r[0] + 1.0, r[1] + 70.0 - rows, 17.0, rows + 1.0], tint);
        }
    }
}

/// `native_screen_draw`: `renderBg` (the panels, the bars, the slots), the buttons and `renderLabels`.
pub(super) fn draw(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let view = ParadoxView::unpack(data).unwrap_or_default();
    let mut pen = Pen { game, batch, origin };
    pen.panel([20.0, 0.0, 216.0, 20.0]);
    pen.panel([0.0, SECTION, 256.0, 102.0]);
    pen.panel([LEFT, 75.0 + TOP, 176.0, 93.0]);
    for row in 0..3 {
        for col in 0..9 {
            pen.slot([INVENTORY[0] + 18.0 * col as f32, INVENTORY[1] + 18.0 * row as f32]);
        }
    }
    for col in 0..9 {
        pen.slot([INVENTORY[0] + 18.0 * col as f32, INVENTORY[1] + 58.0]);
    }
    let u = pen.bar(POWERBAR, ENERGY);
    pen.fill(POWERBAR, ENERGY, u, ParadoxView::rows(view.energy, view.energy_capacity), None);
    // The fluid bar: its back, the fluid's still texture in the fluid's tint (`renderFluid`), then its scale on top.
    let u = pen.bar(FLUIDBAR, FLUID_BAR);
    let rows = ParadoxView::rows(view.fluid, view.fluid_capacity).clamp(0, 70) as f32;
    if rows > 0.0 {
        let dst = [FLUID_BAR[0] + 1.0, FLUID_BAR[1] + 71.0 - rows, 16.0, rows];
        if !pen.sprite(FLUID, [0.0, 0.0, 16.0, rows.min(16.0)], dst, Some(0xFF00_FF00)) {
            pen.sprite(POWERBAR, [19.0 * u, 69.0 - rows, 16.0 * u, rows], dst, Some(0xFF00_FF00));
        }
    }
    pen.sprite(FLUIDBAR, [18.0 * u, 0.0, 18.0 * u, 72.0], FLUID_BAR, None);
    // The paradox bar, lit in a colour that turns (Java's rainbow follows the clock).
    let u = pen.bar(PARADOXBAR, PARADOX_BAR);
    pen.fill(PARADOXBAR, PARADOX_BAR, u, ParadoxView::rows(view.paradox_energy, view.paradox_max), Some(0xFFC0_40FF));
    // The area's value buttons and their labels.
    for axis in 0..3 {
        for row in 0..2 {
            let [x, y] = value_button(axis, row);
            pen.button("justdirethings:gui/buttons/remove", [x, y, 12.0, 12.0], true);
            pen.button("justdirethings:gui/buttons/add", [x + 30.0, y, 12.0, 12.0], true);
            let value = if row == 0 { format!("{:.1}", f64::from(view.radius[axis]) / 2.0) } else { view.offset[axis].to_string() };
            pen.text(&value, x + 11.0, y + 2.0);
        }
    }
    pen.text("Rad", LEFT + 5.0, SECTION + 14.0);
    pen.text("Off", LEFT + 5.0, SECTION + 29.0);
    for (i, axis) in ["X", "Y", "Z"].into_iter().enumerate() {
        pen.text(axis, LEFT + 44.0 + 50.0 * i as f32, SECTION + 4.0);
    }
    pen.button("justdirethings:gui/buttons/area", RENDER_AREA, view.render_area != 0);
    pen.button(redstone(Mode::from_index(view.redstone)).0, REDSTONE, true);
    pen.button("justdirethings:gui/buttons/positionswap", SNAPSHOT, true);
    pen.button("justdirethings:gui/buttons/showfakeplayer", RENDER_PARADOX, view.render_paradox != 0);
    pen.button(TARGETS[view.target.clamp(0, 2) as usize].0, TARGET, true);
    // The tick speed's number button (`NumberButton`): its value on a plain button.
    pen.button("justdirethings:gui/buttons/blank", TICK_SPEED, true);
    let ticks = view.tick_speed.to_string();
    let w = HudBatch::text_width(&pen.game.gui.atlas, &ticks);
    pen.text(&ticks, TICK_SPEED[0] + 12.0 - (w / 2.0).floor(), TICK_SPEED[1] + 2.0);
    let title = pen.game.native_screen_title().unwrap_or_else(|| "Paradox Machine".into());
    let w = HudBatch::text_width(&pen.game.gui.atlas, &title);
    pen.text(&title, 128.0 - (w / 2.0).floor(), 6.0);
}

/// `native_screen_click`: the menu button a press is on (see `justdirethings_time::button`).
pub(super) fn click(data: &[i32], at: [f32; 2], right: bool, button: &mut Option<i32>) {
    let Some(view) = ParadoxView::unpack(data) else { return };
    let mode = Mode::from_index(view.redstone);
    *button = if inside(REDSTONE, at) {
        Some(if right { Mode::from_index((mode.index() + 3) % 4) } else { mode.next() }.index())
    } else if inside(SNAPSHOT, at) && !right {
        Some(4)
    } else if inside(RENDER_PARADOX, at) && !right {
        Some(5)
    } else if inside(TARGET, at) {
        Some(6 + (view.target + if right { 2 } else { 1 }) % 3)
    } else if inside(RENDER_AREA, at) && !right {
        Some(9)
    } else if inside(TICK_SPEED, at) {
        Some(if right { 23 } else { 22 })
    } else {
        (0..3).flat_map(|axis| (0..2).map(move |row| (axis, row))).find_map(|(axis, row)| {
            let [x, y] = value_button(axis, row);
            let base = if row == 0 { 10 } else { 16 } + 2 * axis as i32;
            if inside([x, y, 12.0, 12.0], at) {
                Some(base + 1)
            } else if inside([x + 30.0, y, 12.0, 12.0], at) {
                Some(base)
            } else {
                None
            }
        })
    };
}

/// `native_screen_tooltip`: the bars' (`powerBarTooltip`, `fluidBarTooltip`, `paradoxBarTooltip`: what it holds and
/// what restoring the snapshot costs now) and the buttons'.
pub(super) fn tooltip(data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    let Some(v) = ParadoxView::unpack(data) else { return };
    if inside(ENERGY, at) {
        lines.push(format!("{} / {} FE", v.energy, v.energy_capacity));
        lines.push(format!("Restore cost: {} FE", v.energy_cost));
    } else if inside(FLUID_BAR, at) {
        lines.push(format!("Time Fluid: {} / {} mB", v.fluid, v.fluid_capacity));
        lines.push(format!("Restore cost: {} mB", v.fluid_cost));
    } else if inside(PARADOX_BAR, at) {
        lines.push(format!("Paradox energy: {:.2} / {:.0}", f64::from(v.paradox_energy) / 100.0, f64::from(v.paradox_max) / 100.0));
    } else if inside(REDSTONE, at) {
        lines.push(redstone(Mode::from_index(v.redstone)).1.into());
    } else if inside(SNAPSHOT, at) {
        lines.push("Snapshot area".into());
        lines.push(format!("{} block(s), {} entit(ies) to restore", v.blocks, v.entities));
    } else if inside(RENDER_PARADOX, at) {
        lines.push("Render paradox".into());
    } else if inside(TARGET, at) {
        lines.push(TARGETS[v.target.clamp(0, 2) as usize].1.into());
    } else if inside(RENDER_AREA, at) {
        lines.push("Render area".into());
    } else if inside(TICK_SPEED, at) {
        lines.push(format!("Ticks: {}", v.tick_speed));
    }
    if v.running != 0 && lines.is_empty() && at[1] < TOP + 75.0 {
        lines.push(format!("Restoring: {} / {} ticks", v.time_running, v.run_time));
    }
}
