// The Generator T1's and the Block Breaker T1's screens (support module `crate::game::justdirethings_ui` of the `justdirethings` native bundle),
// drawn as Just Dire Things draws its machines' screens (`BaseMachineScreen`, `GeneratorT1Screen`, `BlockBreakerT1Screen`)
// from the mod's own
// `gui/sprites/background` panel, `gui/justslot`, `gui/powerbar` and `gui/buttons/redstone*` textures, which the carry
// brings from the player's jar: the title tab, the top section with the energy bar, the fuel slot and its flame and the
// redstone button, and the inventory section, every slot framed. Translated from Just Dire Things 1.5.7 (MIT); see
// this bundle's README.md. Java's screen hangs its top section 26 pixels above the menu's 176x166 and its title tab 20
// above that: here the screen is 214 high and every y is Java's plus 46.

use super::{
    Game,
    justdirethings_world::{BREAKER, GENERATOR},
};
use horizon_client::hud::HudBatch;
use justdirethings_core::{redstone::Mode, view::GeneratorView};
use mod_api::Screen;

const BACKGROUND: &str = "justdirethings:gui/sprites/background";
const SLOT: &str = "justdirethings:gui/justslot";
const POWERBAR: &str = "justdirethings:gui/powerbar";
const LABEL: u32 = 0xFF40_4040;

/// Java's y of the menu's top in this screen.
const TOP: f32 = 46.0;
/// The machine's one slot: the generator's fuel, the breaker's tool.
const FUEL_SLOT: [f32; 2] = [80.0, 13.0 + TOP];
const INVENTORY: [f32; 2] = [8.0, 84.0 + TOP];
/// The energy bar (`topSectionLeft + 5`, `topSectionTop + 5`), 18x72.
const ENERGY: [f32; 4] = [5.0, 25.0, 18.0, 72.0];
/// The flame under the fuel slot.
const FLAME: [f32; 2] = [79.0, 30.0 + TOP];
/// The redstone button (`getGuiLeft() + 104`, `topSectionTop + 38`), 16x16.
const REDSTONE: [f32; 4] = [104.0, 58.0, 16.0, 16.0];

fn inside(r: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= r[0] && at[0] < r[0] + r[2] && at[1] >= r[1] && at[1] < r[1] + r[3]
}

fn redstone_texture(mode: Mode) -> &'static str {
    match mode {
        Mode::Ignored => "justdirethings:gui/buttons/redstoneignore",
        Mode::Low => "justdirethings:gui/buttons/redstonelow",
        Mode::High => "justdirethings:gui/buttons/redstonehigh",
        Mode::Pulse => "justdirethings:gui/buttons/redstonepulse",
    }
}

fn redstone_line(mode: Mode) -> &'static str {
    match mode {
        Mode::Ignored => "Redstone: ignored",
        Mode::Low => "Redstone: runs while unpowered",
        Mode::High => "Redstone: runs while powered",
        Mode::Pulse => "Redstone: runs once a pulse",
    }
}

/// `native_screen`: the panel and the machine's slot; the hopper menu's four other slots are left out.
pub(super) fn screen(block: &str, screen: &mut Option<Screen>) {
    if screen.is_some() || !(block == GENERATOR || block == BREAKER) {
        return;
    }
    let mut slots = vec![Some(FUEL_SLOT)];
    slots.resize(5, None);
    *screen = Some(Screen {
        width: 176.0,
        height: 166.0 + TOP + 2.0,
        slots,
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
    fn sprite(&mut self, texture: &str, src: [f32; 4], dst: [f32; 4]) {
        let dst = [self.origin[0] + dst[0], self.origin[1] + dst[1], dst[2], dst[3]];
        self.game.native_sprite(self.batch, texture, src, dst, None);
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
                    self.sprite(BACKGROUND, [su, sv, sw, sh], [dx, dy, dw, dh]);
                }
            }
        }
    }

    /// A slot's frame (`drawSlot`: `justslot`'s first 18x18) around the slot at `at`.
    fn slot(&mut self, at: [f32; 2]) {
        self.sprite(SLOT, [0.0, 0.0, 18.0, 18.0], [at[0] - 1.0, at[1] - 1.0, 18.0, 18.0]);
    }

    fn text(&mut self, text: &str, x: f32, y: f32, color: u32) {
        self.game.native_text(self.batch, text, [self.origin[0] + x, self.origin[1] + y], color, false);
    }
}

/// `native_screen_draw`: `renderBg` (the panels, the slots, the energy bar, the flame), the redstone button and the
/// title (`renderLabels`).
pub(super) fn draw(game: &Game<'_>, block: &str, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    if block == BREAKER {
        draw_breaker(game, data, batch, origin);
        return;
    }
    if block != GENERATOR {
        return;
    }
    let view = GeneratorView::unpack(data).unwrap_or_default();
    let mut pen = Pen { game, batch, origin };
    frame(&mut pen);
    pen.slot(FUEL_SLOT);
    // `powerbar.png` is 34 wide but Java blits it as 36: its u's are scaled the same way here.
    let u = 34.0 / 36.0;
    pen.sprite(POWERBAR, [0.0, 0.0, 18.0 * u, 72.0], ENERGY);
    let rows = view.energy_rows().clamp(0, 70) as f32;
    pen.sprite(POWERBAR, [19.0 * u, 69.0 - rows, 17.0 * u, rows + 1.0], [ENERGY[0] + 1.0, ENERGY[1] + 70.0 - rows, 17.0, rows + 1.0]);
    pen.sprite(SLOT, [0.0, 18.0, 18.0, 18.0], [FLAME[0], FLAME[1], 18.0, 18.0]);
    if let Some(lit) = view.flame_rows() {
        let lit = lit.clamp(0, 18) as f32;
        pen.sprite(SLOT, [18.0, 36.0 - lit, 18.0, lit + 3.0], [FLAME[0], FLAME[1] + 18.0 - lit, 18.0, lit + 3.0]);
    }
    labels(&mut pen, view.redstone, "Generator T1");
}

/// What every machine's screen has: the title tab, the top section and the inventory section with its slots framed.
fn frame(pen: &mut Pen<'_, '_>) {
    pen.panel([20.0, 0.0, 136.0, 20.0]);
    pen.panel([0.0, 20.0, 176.0, 102.0]);
    pen.panel([0.0, 75.0 + TOP, 176.0, 93.0]);
    for row in 0..3 {
        for col in 0..9 {
            pen.slot([INVENTORY[0] + 18.0 * col as f32, INVENTORY[1] + 18.0 * row as f32]);
        }
    }
    for col in 0..9 {
        pen.slot([INVENTORY[0] + 18.0 * col as f32, INVENTORY[1] + 58.0]);
    }
}

/// The redstone button and the title (`renderLabels`: the block's name, centred over the tab).
fn labels(pen: &mut Pen<'_, '_>, redstone: i32, fallback: &str) {
    let mode = Mode::from_index(redstone);
    pen.sprite(redstone_texture(mode), [0.0, 0.0, 16.0, 16.0], REDSTONE);
    let title = pen.game.native_screen_title().unwrap_or_else(|| fallback.into());
    let w = HudBatch::text_width(&pen.game.gui.atlas, &title);
    pen.text(&title, 88.0 - (w / 2.0).floor(), 6.0, LABEL);
}

/// The breaker's screen (`BlockBreakerT1Screen`): its tool slot, drawn with `justslot`'s pickaxe outline while empty
/// (`drawMachineSlot`), and the redstone button. The data is the redstone mode and whether the slot holds a tool.
fn draw_breaker(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let mut pen = Pen { game, batch, origin };
    frame(&mut pen);
    let has_tool = data.get(1).is_some_and(|t| *t != 0);
    let src = if has_tool { [0.0, 0.0, 18.0, 18.0] } else { [18.0, 0.0, 18.0, 18.0] };
    pen.sprite(SLOT, src, [FUEL_SLOT[0] - 1.0, FUEL_SLOT[1] - 1.0, 18.0, 18.0]);
    labels(&mut pen, data.first().copied().unwrap_or(0), "Block Breaker T1");
}

/// `native_screen_click`: the redstone button cycles the mode (the menu button of the next one's ordinal).
pub(super) fn click(_game: &Game<'_>, block: &str, data: &[i32], at: [f32; 2], right: bool, button: &mut Option<i32>) {
    if right || !inside(REDSTONE, at) {
        return;
    }
    let mode = match block {
        GENERATOR => GeneratorView::unpack(data).map(|v| v.redstone),
        BREAKER => data.first().copied(),
        _ => None,
    };
    if let Some(mode) = mode {
        *button = Some(Mode::from_index(mode).next().index());
    }
}

/// `native_screen_tooltip`: the energy bar's (`powerBarTooltip`: the buffer, the FE a tick, the burn time left) and
/// the redstone button's.
pub(super) fn tooltip(block: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    if block == BREAKER {
        if inside(REDSTONE, at) {
            lines.push(redstone_line(Mode::from_index(data.first().copied().unwrap_or(0))).into());
        }
        return;
    }
    if block != GENERATOR {
        return;
    }
    let Some(view) = GeneratorView::unpack(data) else { return };
    if inside(ENERGY, at) {
        lines.push(format!("{} / {} FE", view.energy, view.capacity));
        lines.push(format!("{} FE/t", view.fe_per_tick));
        if view.burn_remaining > 0 {
            lines.push(format!("Burn time: {:.1}s", f64::from(view.burn_remaining) / 20.0));
        } else {
            lines.push("No fuel".into());
        }
    } else if inside(REDSTONE, at) {
        lines.push(redstone_line(Mode::from_index(view.redstone)).into());
    }
}
