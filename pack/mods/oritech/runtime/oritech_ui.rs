// The screen of Oritech's processing machines (support module `crate::game::oritech_ui` of the `oritech` native
// bundle), drawn from the mod's own `gui/modular` textures (`gui_base`, `itemslot`, `machine_gui_components`,
// `arrow_empty`, `arrow_full`), which the carry brings from the player's jar. Translated from `OritechMachineScreen`,
// `OritechWidgetScreen`, `EnergyDisplayWidget`, `ProgressDisplayWidget` and `ItemSlotWidget` (CC0); see this
// bundle's README.md. The numbers come in the menu's data (`MachineView`); the layout is the machine's `Spec`.

use super::Game;
use horizon_client::hud::HudBatch;
use mod_api::Screen;
use oritech_core::{machine, view::MachineView};

const BASE: &str = "oritech:gui/modular/gui_base";
const SLOT: &str = "oritech:gui/modular/itemslot";
const COMPONENTS: &str = "oritech:gui/modular/machine_gui_components";
const ARROW_EMPTY: &str = "oritech:gui/modular/arrow_empty";
const ARROW_FULL: &str = "oritech:gui/modular/arrow_full";
const WIDTH: f32 = 176.0;
const HEIGHT: f32 = 166.0;
/// `getEnergyConfiguration` (8, 24, 16, 54) with the widget's one-pixel inset.
const ENERGY: [f32; 4] = [8.0, 24.0, 16.0, 54.0];
/// `getIndicatorConfiguration`: the arrow at (80, 35), 29 by 16.
const ARROW: [f32; 4] = [80.0, 35.0, 29.0, 16.0];
/// A generator's burn indicator: under its one slot at (80, 21).
const BURN: [f32; 4] = [80.0, 43.0, 29.0, 16.0];

fn arrow(generator: bool) -> [f32; 4] {
    if generator { BURN } else { ARROW }
}
/// `LabelWidget.DARK_TEXT` and the panel's colours.
const DARK: u32 = 0x404040;
const PANEL: u32 = 0xFFC6C6C6;
const PANEL_EDGE: u32 = 0xFF555555;
const INSET: u32 = 0xFF373737;

fn inside(r: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= r[0] && at[0] < r[0] + r[2] && at[1] >= r[1] && at[1] < r[1] + r[3]
}

/// The block's name in words, as its language file's English names it (`pulverizer_block` is "Pulverizer").
fn title(block: &str) -> String {
    let path = block.rsplit(':').next().unwrap_or(block);
    let path = path.strip_suffix("_block").unwrap_or(path);
    path.split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `native_screen`: 176 by 166 with the player's inventory at (8, 84); the machine's slots where `getGuiSlots` puts
/// them, the hopper menu's others hidden.
pub(super) fn screen(block: &str, screen: &mut Option<Screen>) {
    let Some(spec) = machine::of_block(block) else { return };
    if screen.is_some() {
        return;
    }
    let slots = (0..5)
        .map(|i| spec.gui_slots.iter().find(|g| g.index == i).map(|g| [g.x as f32, g.y as f32]))
        .collect();
    *screen = Some(Screen {
        width: WIDTH,
        height: HEIGHT,
        slots,
        inventory_x: 8.0,
        inventory_y: 84.0,
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

    fn fill(&mut self, r: [f32; 4], argb: u32) {
        self.game.native_fill(self.batch, [self.origin[0] + r[0], self.origin[1] + r[1], r[2], r[3]], argb);
    }

    fn text(&mut self, text: &str, x: f32, y: f32, color: u32) {
        self.game.native_text(self.batch, text, [self.origin[0] + x, self.origin[1] + y], color, false);
    }

    fn item(&mut self, item: &str, x: f32, y: f32) {
        self.game.native_item(self.batch, item, [self.origin[0] + x, self.origin[1] + y], 16.0);
    }

    /// `OritechSurface.PANEL`: a light panel with a dark edge.
    fn panel(&mut self, r: [f32; 4]) {
        self.fill(r, PANEL_EDGE);
        self.fill([r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0], PANEL);
    }
}

fn text_width(game: &Game<'_>, text: &str) -> f32 {
    HudBatch::text_width(&game.gui.atlas, text)
}

/// `native_screen_draw`.
pub(super) fn draw(game: &Game<'_>, block: &str, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let Some(spec) = machine::of_block(block) else { return };
    let Some(v) = MachineView::unpack(data) else { return };
    let mut pen = Pen { game, batch, origin };
    // `addTitle`: the block's icon and name on panels above the screen, 65% of the way across.
    let name = title(block);
    let label_w = text_width(game, &name) + 10.0;
    let combined = 28.0 + label_w + 2.0;
    let x = if name.len() > 15 { WIDTH - combined } else { ((WIDTH - combined) * 65.0 / 100.0).floor() };
    pen.panel([x, -27.0, 28.0, 28.0]);
    pen.item(block, x + 6.0, -21.0);
    pen.panel([x + 28.0 + 2.0 + 6.0 - 6.0, -18.0, label_w + 6.0, 19.0]);
    pen.text(&name, x + 36.0 + 5.0, -13.0, DARK);
    // The background, then each machine slot's frame (`ItemSlotWidget`, one pixel out).
    pen.sprite(BASE, [0.0, 0.0, WIDTH, HEIGHT], [0.0, 0.0, WIDTH, HEIGHT]);
    for g in spec.gui_slots {
        pen.sprite(SLOT, [0.0, 0.0, 18.0, 18.0], [g.x as f32 - 1.0, g.y as f32 - 1.0, 18.0, 18.0]);
    }
    // `EnergyDisplayWidget` on a `PANEL_INSET` surface: the empty bar, then the full one from the bottom.
    pen.fill(ENERGY, INSET);
    let [ex, ey, ew, eh] = [ENERGY[0] + 1.0, ENERGY[1] + 1.0, ENERGY[2] - 2.0, ENERGY[3] - 2.0];
    pen.sprite(COMPONENTS, [24.0, 0.0, 24.0, 96.0], [ex, ey, ew, eh]);
    let ratio = v.energy_share();
    let filled = (eh * ratio).floor();
    if filled > 0.0 {
        let src_y = 96.0 - (96.0 * ratio).floor();
        pen.sprite(COMPONENTS, [0.0, src_y, 24.0, 96.0 - src_y], [ex, ey + eh - filled, ew, filled]);
    }
    // `ProgressDisplayWidget`: the empty arrow, then the full one from the left.
    let arrow = arrow(spec.generator);
    pen.sprite(ARROW_EMPTY, [0.0, 0.0, 29.0, 16.0], arrow);
    let share = v.progress_share();
    let w = (arrow[2] * share).floor();
    if w > 0.0 {
        pen.sprite(ARROW_FULL, [0.0, 0.0, (29.0 * share).floor(), 16.0], [arrow[0], arrow[1], w, arrow[3]]);
    }
    // A multiblock whose cores aren't all there: Java's controller says so in chat on the click that would open it.
    if spec.is_multiblock() && !v.active {
        pen.text(&format!("Missing machine cores: {}", v.missing), 30.0, 62.0, 0xAA0000);
    }
}

/// `native_screen_tooltip`: the energy bar's (`EnergyDataSource`: stored of capacity, the use a tick and the most it
/// takes a tick) and the arrow's (the progress).
pub(super) fn tooltip(block: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    let Some(spec) = machine::of_block(block) else { return };
    let Some(v) = MachineView::unpack(data) else { return };
    if inside(ENERGY, at) {
        lines.push(format!("{} / {} RF", v.energy, v.capacity));
        if spec.generator {
            lines.push(format!("§7Production: {} RF/t", v.usage));
            lines.push(format!("§7Max output: {} RF/t", v.insert));
        } else {
            lines.push(format!("§7Usage: {} RF/t", v.usage));
            lines.push(format!("§7Max input: {} RF/t", v.insert));
        }
    } else if inside(arrow(spec.generator), at) && v.total > 0 {
        lines.push(format!("{}%", (v.progress_share() * 100.0).round() as i32));
    }
}
