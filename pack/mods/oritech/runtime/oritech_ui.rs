// The screen of Oritech's processing machines (support module `crate::game::oritech_ui` of the `oritech` native
// bundle), drawn from the mod's own `gui/modular` textures (`gui_base`, `itemslot`, `machine_gui_components`,
// `arrow_empty`, `arrow_full`), which the carry brings from the player's jar. Translated from `OritechMachineScreen`,
// `OritechWidgetScreen`, `EnergyDisplayWidget`, `ProgressDisplayWidget` and `ItemSlotWidget` (CC0); see this
// bundle's README.md. The numbers come in the menu's data (`MachineView`); the layout is the machine's `Spec`.

use super::Game;
use horizon_client::hud::HudBatch;
use mod_api::Screen;
use horizon_world::fluid::FluidKind;
use oritech_core::{machine::{self, Kind, Spec}, view::{self, MachineView}};

const BASE: &str = "oritech:gui/modular/gui_base";
const SLOT: &str = "oritech:gui/modular/itemslot";
const COMPONENTS: &str = "oritech:gui/modular/machine_gui_components";
const ARROW_EMPTY: &str = "oritech:gui/modular/arrow_empty";
const ARROW_FULL: &str = "oritech:gui/modular/arrow_full";
const WIDTH: f32 = 176.0;
const HEIGHT: f32 = 166.0;
/// `getEnergyConfiguration` (8, 24, 16, 54) with the widget's one-pixel inset; the refinery's is (8, 7, 18, 71).
fn energy_rect(spec: &Spec) -> [f32; 4] {
    if spec.kind == Kind::Refinery { [8.0, 7.0, 18.0, 71.0] } else { [8.0, 24.0, 16.0, 54.0] }
}

/// `getFluidConfiguration`: where a machine's tanks draw, in the order its numbers carry them. A centrifuge: input
/// and output; the refinery: input, then its output and its two modules'; the rest one tank at the default place.
fn tank_rects(spec: &Spec) -> &'static [[f32; 4]] {
    match spec.kind {
        Kind::Centrifuge => &[[28.0, 6.0, 21.0, 74.0], [147.0, 6.0, 21.0, 74.0]],
        Kind::Refinery => &[[30.0, 6.0, 21.0, 74.0], [92.0, 6.0, 21.0, 74.0], [119.0, 6.0, 21.0, 74.0], [146.0, 6.0, 21.0, 74.0]],
        _ => &[[147.0, 6.0, 21.0, 74.0]],
    }
}

/// A fluid's colour on its bar: water's and lava's, a mod's own tint.
fn fluid_color(id: i32) -> u32 {
    match FluidKind::from_tick_id(id.max(0) as u32) {
        Some(FluidKind::Water | FluidKind::FlowingWater) => 0xFF3F_76E4,
        Some(FluidKind::Lava | FluidKind::FlowingLava) => 0xFFFF_6A00,
        Some(k) => k.carried().map_or(0xFF88_8888, |m| m.tint | 0xFF00_0000),
        None => 0xFF88_8888,
    }
}
/// `getIndicatorConfiguration`: the arrow at (80, 35), 29 by 16.
const ARROW: [f32; 4] = [80.0, 35.0, 29.0, 16.0];
/// A generator's burn indicator: under its one slot at (80, 21).
const BURN: [f32; 4] = [80.0, 43.0, 29.0, 16.0];

fn arrow(spec: &Spec) -> [f32; 4] {
    match spec.kind {
        Kind::Refinery => [54.0, 35.0, 29.0, 16.0],
        // The fluid generators' slots sit left and right of the arrow, not above it.
        _ if spec.generator && spec.inventory_size == 1 => BURN,
        _ => ARROW,
    }
}
/// `LabelWidget.DARK_TEXT` and the panel's colours.
const DARK: u32 = 0xFF40_4040;
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
    // The machines' items are GeckoLib entity models (`builtin/entity`), which draw nothing: the template block's icon stands in.
    let drawn = pen.batch.vertices.len();
    pen.item(block, x + 6.0, -21.0);
    if pen.batch.vertices.len() == drawn {
        pen.item("minecraft:furnace", x + 6.0, -21.0);
    }
    pen.panel([x + 28.0 + 2.0 + 6.0 - 6.0, -18.0, label_w + 6.0, 19.0]);
    pen.text(&name, x + 36.0 + 5.0, -13.0, DARK);
    // The background, then each machine slot's frame (`ItemSlotWidget`, one pixel out).
    pen.sprite(BASE, [0.0, 0.0, WIDTH, HEIGHT], [0.0, 0.0, WIDTH, HEIGHT]);
    for g in spec.gui_slots {
        pen.sprite(SLOT, [0.0, 0.0, 18.0, 18.0], [g.x as f32 - 1.0, g.y as f32 - 1.0, 18.0, 18.0]);
    }
    // `EnergyDisplayWidget` on a `PANEL_INSET` surface: the empty bar, then the full one from the bottom.
    let energy = energy_rect(spec);
    pen.fill(energy, INSET);
    let [ex, ey, ew, eh] = [energy[0] + 1.0, energy[1] + 1.0, energy[2] - 2.0, energy[3] - 2.0];
    pen.sprite(COMPONENTS, [24.0, 0.0, 24.0, 96.0], [ex, ey, ew, eh]);
    let ratio = v.energy_share();
    let filled = (eh * ratio).floor();
    if filled > 0.0 {
        let src_y = 96.0 - (96.0 * ratio).floor();
        pen.sprite(COMPONENTS, [0.0, src_y, 24.0, 96.0 - src_y], [ex, ey + eh - filled, ew, filled]);
    }
    // `ProgressDisplayWidget`: the empty arrow, then the full one from the left.
    let arrow = arrow(spec);
    pen.sprite(ARROW_EMPTY, [0.0, 0.0, 29.0, 16.0], arrow);
    let share = v.progress_share();
    let w = (arrow[2] * share).floor();
    if w > 0.0 {
        pen.sprite(ARROW_FULL, [0.0, 0.0, (29.0 * share).floor(), 16.0], [arrow[0], arrow[1], w, arrow[3]]);
    }
    // The fluid tanks: an inset bar, filled from the bottom with the fluid's colour.
    for (rect, tank) in tank_rects(spec).iter().zip(view::unpack_tanks(data)) {
        if tank.capacity <= 0 {
            continue;
        }
        pen.fill(*rect, INSET);
        let [bx, by, bw, bh] = [rect[0] + 1.0, rect[1] + 1.0, rect[2] - 2.0, rect[3] - 2.0];
        let share = (tank.amount as f32 / tank.capacity as f32).clamp(0.0, 1.0);
        let height = (bh * share).floor();
        if height > 0.0 && tank.fluid > 0 {
            pen.fill([bx, by + bh - height, bw, height], fluid_color(tank.fluid));
        }
    }
    // A multiblock whose cores aren't all there: Java's controller says so in chat on the click that would open it.
    if spec.is_multiblock() && !v.active {
        pen.text(&format!("Missing machine cores: {}", v.missing), 30.0, 62.0, 0xFFAA_0000);
    }
}

/// `native_screen_tooltip`: the energy bar's (`EnergyDataSource`: stored of capacity, the use a tick and the most it
/// takes a tick) and the arrow's (the progress).
pub(super) fn tooltip(block: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    let Some(spec) = machine::of_block(block) else { return };
    let Some(v) = MachineView::unpack(data) else { return };
    if let Some((_, tank)) = tank_rects(spec).iter().zip(view::unpack_tanks(data)).find(|(r, t)| t.capacity > 0 && inside(**r, at)) {
        if let Some(kind) = FluidKind::from_tick_id(tank.fluid.max(0) as u32).filter(|_| tank.fluid > 0) {
            lines.push(title(kind.name()));
        }
        lines.push(format!("{} / {} mB", tank.amount, tank.capacity));
    } else if inside(energy_rect(spec), at) {
        lines.push(format!("{} / {} RF", v.energy, v.capacity));
        if spec.generator {
            lines.push(format!("§7Production: {} RF/t", v.usage));
            lines.push(format!("§7Max output: {} RF/t", v.insert));
        } else {
            lines.push(format!("§7Usage: {} RF/t", v.usage));
            lines.push(format!("§7Max input: {} RF/t", v.insert));
        }
    } else if inside(arrow(spec), at) && v.total > 0 {
        lines.push(format!("{}%", (v.progress_share() * 100.0).round() as i32));
    }
}
