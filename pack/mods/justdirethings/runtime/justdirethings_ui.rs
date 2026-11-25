// The Generator T1's, Block Breaker T1's, Block Placer T1's, Fluid Collector T1's and Item Collector's screens (support module
// `crate::game::justdirethings_ui` of the `justdirethings` native bundle), drawn as Just Dire Things draws its machines'
// screens (`BaseMachineScreen`, `GeneratorT1Screen`, `BlockBreakerT1Screen`, `BlockPlacerT1Screen`,
// `FluidCollectorT1Screen`, `ItemCollectorScreen`) from the mod's own `gui/sprites/background` panel, `gui/justslot`, `gui/powerbar`,
// `gui/fluidbar` and `gui/buttons/*` textures, which the carry brings from the player's jar: the title tab, the top
// section with the energy or fluid bar, the machine's slot, the generator's flame and the redstone, direction and
// tick speed buttons, and the inventory section, every slot framed. Translated from Just Dire Things 1.5.7 (MIT); see
// this bundle's README.md. Java's screen hangs its top section 26 pixels above the menu's 176x166 and its title tab 20
// above that: here the screen is 214 high and every y is Java's plus 46.

use super::{
    Game,
    justdirethings_machines::{COLLECTOR, ITEM_COLLECTOR, PLACER},
    justdirethings_more::{DROPPER, FLUID_GEN, FLUID_PLACER, SENSOR},
    justdirethings_clicker::{CLICKER_T1, CLICKER_T2},
    justdirethings_t2::{BREAKER_T2, PLACER_T2},
    justdirethings_swapper::{SWAPPER, SWAPPER_T2},
    justdirethings_t2x::{DROPPER_T2, FLUID_COLLECTOR_T2, FLUID_PLACER_T2, SENSOR_T2},
    justdirethings_world::{BREAKER, GENERATOR, PARADOX},
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
/// Where `BaseMachineScreen` puts the redstone button when a screen keeps it (`getGuiLeft() + 134`,
/// `topSectionTop + 62`): the Fluid Collector T1's.
const REDSTONE_BASE: [f32; 4] = [134.0, 82.0, 16.0, 16.0];
/// The Block Placer T1's direction button (`getGuiLeft() + 122`, `topSectionTop + 38`).
const DIRECTION: [f32; 4] = [122.0, 58.0, 16.0, 16.0];
/// The tick speed's number button (`getGuiLeft() + 144`, `topSectionTop + 40`), 24x12.
const TICK_SPEED: [f32; 4] = [144.0, 60.0, 24.0, 12.0];
const FLUIDBAR: &str = "justdirethings:gui/fluidbar";
/// `DIRECTION_TEXTURES`, in `Direction.values()` order.
const DIRECTIONS: [(&str, &str); 6] = [
    ("justdirethings:gui/buttons/direction-down", "Direction: Down"),
    ("justdirethings:gui/buttons/direction-up", "Direction: Up"),
    ("justdirethings:gui/buttons/direction-north", "Direction: North"),
    ("justdirethings:gui/buttons/direction-south", "Direction: South"),
    ("justdirethings:gui/buttons/direction-west", "Direction: West"),
    ("justdirethings:gui/buttons/direction-east", "Direction: East"),
];

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
    super::justdirethings_paradox_ui::screen(block, screen);
    if screen.is_some() {
        return;
    }
    if block == ITEM_COLLECTOR {
        // `addFilterSlots(filterHandler, 0, 8, 54, 9, 18)`: the nine filter slots in a row inside the top section.
        *screen = Some(Screen {
            width: 176.0,
            height: 166.0 + TOP + 2.0,
            slots: (0..9).map(|i| Some([8.0 + 18.0 * i as f32, 54.0 + TOP])).collect(),
            inventory_x: INVENTORY[0],
            inventory_y: INVENTORY[1],
            own_labels: true,
            own_background: true,
            ..Screen::default()
        });
        return;
    }
    if block == CLICKER_T1 || block == CLICKER_T2 || block == PLACER_T2 || block == BREAKER_T2 {
        *screen = Some(clicker_screen(block != CLICKER_T1));
        return;
    }
    if is_t2x(block) {
        *screen = Some(t2x_screen(block));
        return;
    }
    if block == SWAPPER {
        // No slot of its own: the menu is only the player's inventory.
        *screen = Some(Screen {
            width: 176.0,
            height: 166.0 + TOP + 2.0,
            slots: vec![None; 5],
            inventory_x: INVENTORY[0],
            inventory_y: INVENTORY[1],
            own_labels: true,
            own_background: true,
            ..Screen::default()
        });
        return;
    }
    if !(block == GENERATOR || block == BREAKER || block == PLACER || block == COLLECTOR || super::justdirethings_more::owns(block)) {
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
        self.tinted(texture, src, dst, None);
    }

    fn tinted(&mut self, texture: &str, src: [f32; 4], dst: [f32; 4], tint: Option<u32>) -> bool {
        let dst = [self.origin[0] + dst[0], self.origin[1] + dst[1], dst[2], dst[3]];
        self.game.native_sprite(self.batch, texture, src, dst, tint)
    }

    /// A whole texture (its own size) drawn into `dst`.
    fn whole(&mut self, texture: &str, dst: [f32; 4]) {
        let [w, h] = self.game.native_sprite_size(texture).unwrap_or([16.0, 16.0]);
        self.sprite(texture, [0.0, 0.0, w, h], dst);
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
    if block == PARADOX {
        super::justdirethings_paradox_ui::draw(game, data, batch, origin);
        return;
    }
    if block == PLACER {
        draw_placer(game, data, batch, origin);
        return;
    }
    if block == COLLECTOR || block == FLUID_PLACER {
        draw_collector(game, block, data, batch, origin);
        return;
    }
    if block == DROPPER {
        draw_dropper(game, data, batch, origin);
        return;
    }
    if block == SENSOR {
        draw_sensor(game, data, batch, origin);
        return;
    }
    if block == FLUID_GEN {
        draw_fluid_generator(game, data, batch, origin);
        return;
    }
    if block == SWAPPER {
        draw_swapper(game, data, batch, origin);
        return;
    }
    if block == CLICKER_T1 || block == CLICKER_T2 {
        draw_clicker(game, block == CLICKER_T2, data, batch, origin);
        return;
    }
    if block == PLACER_T2 {
        draw_placer2(game, data, batch, origin);
        return;
    }
    if is_t2x(block) {
        draw_t2x(game, block, data, batch, origin);
        return;
    }
    if block == BREAKER_T2 {
        draw_breaker2(game, data, batch, origin);
        return;
    }
    if block == ITEM_COLLECTOR {
        draw_item_collector(game, data, batch, origin);
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
    labels_at(pen, redstone, REDSTONE, fallback);
}

fn labels_at(pen: &mut Pen<'_, '_>, redstone: i32, button: [f32; 4], fallback: &str) {
    let mode = Mode::from_index(redstone);
    pen.sprite(redstone_texture(mode), [0.0, 0.0, 16.0, 16.0], button);
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

/// The tick speed's number button (`NumberButton`): its value on a plain button.
fn tick_speed(pen: &mut Pen<'_, '_>, ticks: i32) {
    pen.whole("justdirethings:gui/buttons/blank", TICK_SPEED);
    let text = ticks.to_string();
    let w = HudBatch::text_width(&pen.game.gui.atlas, &text);
    pen.text(&text, TICK_SPEED[0] + 12.0 - (w / 2.0).floor(), TICK_SPEED[1] + 2.0, LABEL);
}

/// The Block Placer T1's screen (`BlockPlacerT1Screen`): its slot, the redstone, direction and tick speed buttons.
/// The data is the redstone mode, whether the slot holds an item, the direction and the tick speed.
fn draw_placer(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let mut pen = Pen { game, batch, origin };
    frame(&mut pen);
    pen.slot(FUEL_SLOT);
    let direction = data.get(2).copied().unwrap_or(0).clamp(0, 5) as usize;
    pen.whole(DIRECTIONS[direction].0, DIRECTION);
    tick_speed(&mut pen, data.get(3).copied().unwrap_or(20));
    labels(&mut pen, data.first().copied().unwrap_or(0), "Block Placer T1");
}

/// A machine's fluid bar at `bar`: the frame, the fluid's texture (`renderFluid`: water in its tint) up to its level, the
/// frame's front. `code`: 0 none, 1 water, 2 lava, 3 another.
fn fluid_bar(pen: &mut Pen<'_, '_>, bar: [f32; 4], code: i32, amount: i32, capacity: i32) {
    let u = pen.game.native_sprite_size(FLUIDBAR).map_or(1.0, |[w, _]| w / 36.0);
    pen.sprite(FLUIDBAR, [0.0, 0.0, 18.0 * u, 72.0], bar);
    let rows = (if capacity > 0 { (i64::from(amount) * 70 / i64::from(capacity)) as i32 } else { 0 }).clamp(0, 70) as f32;
    if rows > 0.0 {
        let (texture, tint) = match code {
            1 => ("minecraft:block/water_still", Some(0xFF3F_76E4)),
            2 => ("minecraft:block/lava_still", None),
            _ => ("justdirethings:block/fluid_source", None),
        };
        let mut y = bar[1] + 71.0;
        let mut left = rows;
        while left > 0.0 {
            let h = left.min(16.0);
            y -= h;
            if !pen.tinted(texture, [0.0, 0.0, 16.0, h], [bar[0] + 1.0, y, 16.0, h], tint) {
                // The GUI atlas holds the mod's own fluid texture, not the game's block textures: tinted as the fluid.
                let colour = if code == 2 { 0xFFE0_6A12 } else { 0xFF3F_76E4 };
                pen.tinted("justdirethings:block/fluid_source", [0.0, 0.0, 16.0, h], [bar[0] + 1.0, y, 16.0, h], Some(colour));
            }
            left -= h;
        }
    }
    pen.sprite(FLUIDBAR, [18.0 * u, 0.0, 18.0 * u, 72.0], bar);
}

/// The Fluid Collector T1's screen (`FluidCollectorT1Screen` on `BaseMachineScreen`): its slot, the fluid bar where
/// an unpowered machine has it (`topSectionLeft + 5`), its fluid's still texture in its tint (`renderFluid`), the
/// redstone and tick speed buttons. The data is the redstone mode, whether the slot holds an item, the tick speed, the
/// fluid (0 none, 1 water, 2 lava, 3 another), the amount and the capacity.
fn draw_collector(game: &Game<'_>, block: &str, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let mut pen = Pen { game, batch, origin };
    frame(&mut pen);
    pen.slot(FUEL_SLOT);
    fluid_bar(&mut pen, ENERGY, data.get(3).copied().unwrap_or(0), data.get(4).copied().unwrap_or(0), data.get(5).copied().unwrap_or(0));
    tick_speed(&mut pen, data.get(2).copied().unwrap_or(20));
    labels_at(&mut pen, data.first().copied().unwrap_or(0), REDSTONE_BASE, if block == FLUID_PLACER { "Fluid Placer T1" } else { "Fluid Collector T1" });
}

/// The redstone button (left: the next mode), the placer's direction button (left the next, right the one before:
/// `ToggleButton`) and the tick speed (left one more, right one less).
fn click_machine(block: &str, data: &[i32], at: [f32; 2], right: bool, button: &mut Option<i32>) {
    let redstone = if block == PLACER { REDSTONE } else { REDSTONE_BASE };
    if inside(redstone, at) {
        if !right {
            *button = Some(Mode::from_index(data.first().copied().unwrap_or(0)).next().index());
        }
    } else if block == PLACER && inside(DIRECTION, at) {
        let d = data.get(2).copied().unwrap_or(0).clamp(0, 5);
        *button = Some(4 + (d + if right { 5 } else { 1 }) % 6);
    } else if inside(TICK_SPEED, at) {
        *button = Some(if right { 23 } else { 22 });
    }
}

/// The placer's and the collector's tooltips.
fn tooltip_machine(block: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    let redstone = if block == PLACER { REDSTONE } else { REDSTONE_BASE };
    let ticks = (if block == PLACER { data.get(3) } else { data.get(2) }).copied().unwrap_or(20);
    if inside(redstone, at) {
        lines.push(redstone_line(Mode::from_index(data.first().copied().unwrap_or(0))).into());
    } else if inside(TICK_SPEED, at) {
        lines.push(format!("Ticks: {ticks}"));
    } else if block == PLACER && inside(DIRECTION, at) {
        lines.push(DIRECTIONS[data.get(2).copied().unwrap_or(0).clamp(0, 5) as usize].1.into());
    } else if (block == COLLECTOR || block == FLUID_PLACER) && inside(ENERGY, at) {
        let name = match data.get(3).copied().unwrap_or(0) {
            0 => "Empty",
            1 => "Water",
            2 => "Lava",
            _ => "Fluid",
        };
        lines.push(format!("{name}: {} / {} mB", data.get(4).copied().unwrap_or(0), data.get(5).copied().unwrap_or(0)));
    }
}

/// `native_screen_click`: the redstone button cycles the mode (the menu button of the next one's ordinal).
pub(super) fn click(_game: &Game<'_>, block: &str, data: &[i32], at: [f32; 2], right: bool, button: &mut Option<i32>) {
    if block == PARADOX {
        super::justdirethings_paradox_ui::click(data, at, right, button);
        return;
    }
    if block == PLACER || block == COLLECTOR || block == FLUID_PLACER {
        click_machine(block, data, at, right, button);
        return;
    }
    if block == SWAPPER {
        click_swapper(data, at, right, button);
        return;
    }
    if block == CLICKER_T1 || block == CLICKER_T2 {
        click_clicker(block == CLICKER_T2, data, at, right, button);
        return;
    }
    if block == PLACER_T2 {
        click_placer2(data, at, right, button);
        return;
    }
    if is_t2x(block) {
        click_t2x(block, data, at, right, button);
        return;
    }
    if block == BREAKER_T2 {
        click_breaker2(data, at, right, button);
        return;
    }
    if super::justdirethings_more::owns(block) {
        click_more(block, data, at, right, button);
        return;
    }
    if block == ITEM_COLLECTOR {
        click_item_collector(data, at, right, button);
        return;
    }
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
    if block == PARADOX {
        super::justdirethings_paradox_ui::tooltip(data, at, lines);
        return;
    }
    if block == PLACER || block == COLLECTOR || block == FLUID_PLACER {
        tooltip_machine(block, data, at, lines);
        return;
    }
    if block == SWAPPER {
        tooltip_swapper(data, at, lines);
        return;
    }
    if block == CLICKER_T1 || block == CLICKER_T2 {
        tooltip_clicker(block == CLICKER_T2, data, at, lines);
        return;
    }
    if block == PLACER_T2 {
        tooltip_placer2(data, at, lines);
        return;
    }
    if is_t2x(block) {
        tooltip_t2x(block, data, at, lines);
        return;
    }
    if block == BREAKER_T2 {
        tooltip_breaker2(data, at, lines);
        return;
    }
    if super::justdirethings_more::owns(block) {
        tooltip_more(block, data, at, lines);
        return;
    }
    if block == ITEM_COLLECTOR {
        tooltip_item_collector(data, at, lines);
        return;
    }
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

// ---- The Item Collector's screen (`ItemCollectorScreen` on `BaseMachineScreen`) -------------------------------------

/// Its buttons on the top section's last row (`topSectionTop + 62`): allowlist, compare NBT, show particles, respect
/// pickup delay and render area; the redstone button sits there too (`REDSTONE_BASE`).
const ALLOWLIST: [f32; 4] = [8.0, 82.0, 16.0, 16.0];
const COMPARE_NBT: [f32; 4] = [26.0, 82.0, 16.0, 16.0];
const SHOW_PARTICLES: [f32; 4] = [98.0, 82.0, 16.0, 16.0];
const PICKUP_DELAY: [f32; 4] = [116.0, 82.0, 16.0, 16.0];
const RENDER_AREA: [f32; 4] = [152.0, 82.0, 16.0, 16.0];
/// The filter slots' row (`addFilterSlots` at 8, 54).
const FILTER_ROW: [f32; 2] = [8.0, 54.0 + TOP];
/// A greyed `GrayscaleButton` while off.
const GREY: u32 = 0xFF70_7070;

/// The radius (row 0) and offset (row 1) value buttons of each axis (`ValueButtonsDouble`, `ValueButtons` at
/// `getGuiLeft() + 25 + 50 * axis`, `topSectionTop + 12 + 15 * row`): the minus at x, the plus 30 right of it.
fn value_button(axis: usize, row: usize) -> [f32; 2] {
    [25.0 + 50.0 * axis as f32, 32.0 + 15.0 * row as f32]
}

/// A 16x16 button texture, greyed when `on` is false.
fn toggle(pen: &mut Pen<'_, '_>, texture: &str, dst: [f32; 4], on: bool) {
    let [w, h] = pen.game.native_sprite_size(texture).unwrap_or([16.0, 16.0]);
    pen.tinted(texture, [0.0, 0.0, w, h], dst, (!on).then_some(GREY));
}

/// The screen's data (`item_collector_data`): 0 redstone, 1 tick speed, 2 allowlist, 3 compare NBT, 4 respect pickup
/// delay, 5 show particles, 6 render area, 7 to 9 the radius in half blocks, 10 to 12 the offsets.
fn d(data: &[i32], i: usize) -> i32 {
    data.get(i).copied().unwrap_or(0)
}

fn draw_item_collector(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let mut pen = Pen { game, batch, origin };
    frame(&mut pen);
    for i in 0..9 {
        pen.slot([FILTER_ROW[0] + 18.0 * i as f32, FILTER_ROW[1]]);
    }
    for axis in 0..3 {
        for row in 0..2 {
            let [x, y] = value_button(axis, row);
            pen.whole("justdirethings:gui/buttons/remove", [x, y, 12.0, 12.0]);
            pen.whole("justdirethings:gui/buttons/add", [x + 30.0, y, 12.0, 12.0]);
            let value = if row == 0 { format!("{:.1}", f64::from(d(data, 7 + axis)) / 2.0) } else { d(data, 10 + axis).to_string() };
            pen.text(&value, x + 11.0, y + 2.0, LABEL);
        }
    }
    pen.text("Rad", 5.0, 34.0, LABEL);
    pen.text("Off", 5.0, 49.0, LABEL);
    for (i, axis) in ["X", "Y", "Z"].into_iter().enumerate() {
        pen.text(axis, 44.0 + 50.0 * i as f32, 24.0, LABEL);
    }
    let allow = d(data, 2) != 0;
    pen.whole(if allow { "justdirethings:gui/buttons/allowlisttrue" } else { "justdirethings:gui/buttons/allowlistfalse" }, ALLOWLIST);
    toggle(&mut pen, "justdirethings:gui/buttons/matchnbttrue", COMPARE_NBT, d(data, 3) != 0);
    toggle(&mut pen, "justdirethings:gui/buttons/showfakeplayer", SHOW_PARTICLES, d(data, 5) != 0);
    toggle(&mut pen, "justdirethings:gui/buttons/jumpboost", PICKUP_DELAY, d(data, 4) != 0);
    toggle(&mut pen, "justdirethings:gui/buttons/area", RENDER_AREA, d(data, 6) != 0);
    tick_speed(&mut pen, data.get(1).copied().unwrap_or(20));
    labels_at(&mut pen, d(data, 0), REDSTONE_BASE, "Item Collector");
}

/// The menu button a press is on (see `justdirethings_machines::item_collector_button`): the redstone button cycles
/// (right click back), the toggles flip on a left click, the value buttons step, the tick speed goes up or down.
fn click_item_collector(data: &[i32], at: [f32; 2], right: bool, button: &mut Option<i32>) {
    let mode = Mode::from_index(d(data, 0));
    *button = if inside(REDSTONE_BASE, at) {
        Some(if right { Mode::from_index((mode.index() + 3) % 4) } else { mode.next() }.index())
    } else if inside(TICK_SPEED, at) {
        Some(if right { 23 } else { 22 })
    } else if right {
        None
    } else if inside(RENDER_AREA, at) {
        Some(9)
    } else if inside(ALLOWLIST, at) {
        Some(24)
    } else if inside(COMPARE_NBT, at) {
        Some(25)
    } else if inside(PICKUP_DELAY, at) {
        Some(26)
    } else if inside(SHOW_PARTICLES, at) {
        Some(27)
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

fn tooltip_item_collector(data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    let line = if inside(REDSTONE_BASE, at) {
        redstone_line(Mode::from_index(d(data, 0))).to_owned()
    } else if inside(TICK_SPEED, at) {
        format!("Speed (Ticks): {}", d(data, 1))
    } else if inside(ALLOWLIST, at) {
        (if d(data, 2) != 0 { "Allow List" } else { "Deny List" }).to_owned()
    } else if inside(COMPARE_NBT, at) {
        "Compare NBT".to_owned()
    } else if inside(PICKUP_DELAY, at) {
        "Respect Item Pickup Delay".to_owned()
    } else if inside(SHOW_PARTICLES, at) {
        "Show Particles".to_owned()
    } else if inside(RENDER_AREA, at) {
        "Render Area".to_owned()
    } else {
        return;
    };
    lines.push(line);
}

// ---- The Dropper T1's, Sensor T1's and Fluid Generator T1's screens ----------------------------------------------

/// The Dropper T1's drop count (`getGuiLeft() + 50`, `topSectionTop + 41`) and pickup delay (`+ 27`) number buttons, 24x12.
const COUNT: [f32; 4] = [50.0, 61.0, 24.0, 12.0];
const DELAY: [f32; 4] = [50.0, 47.0, 24.0, 12.0];
/// The Sensor T1's strong or weak signal, allowlist and target buttons (`topSectionTop + 38`).
const STRONG: [f32; 4] = [20.0, 58.0, 16.0, 16.0];
const SENSOR_ALLOW: [f32; 4] = [38.0, 58.0, 16.0, 16.0];
const TARGET: [f32; 4] = [56.0, 58.0, 16.0, 16.0];
/// `SENSOR_TARGET_TEXTURES`, in the target's ordinal order, with their tooltips.
const TARGETS: [(&str, &str); 9] = [
    ("filter-block", "Target: blocks"),
    ("filter-air", "Target: air"),
    ("mobscanner", "Target: hostile mobs"),
    ("passivemob", "Target: passive mobs"),
    ("passivemob-adult", "Target: adult animals"),
    ("passivemob-child", "Target: baby animals"),
    ("player", "Target: players"),
    ("glowing", "Target: living entities"),
    ("item", "Target: items"),
];
/// The fluid generator's fluid bar sits beside the energy bar (`getFluidBarOffset`: 24 when powered).
const GEN_FLUID: [f32; 4] = [24.0, 25.0, 18.0, 72.0];

/// A number button (`NumberButton`): its value on a plain button.
fn number(pen: &mut Pen<'_, '_>, at: [f32; 4], value: i32) {
    pen.whole("justdirethings:gui/buttons/blank", at);
    let text = value.to_string();
    let w = HudBatch::text_width(&pen.game.gui.atlas, &text);
    pen.text(&text, at[0] + 12.0 - (w / 2.0).floor(), at[1] + 2.0, LABEL);
}

/// The Dropper T1's screen (`DropperT1Screen`). The data is the redstone mode, whether the slot holds an item, the
/// direction, the tick speed, the drop count and the pickup delay.
fn draw_dropper(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let mut pen = Pen { game, batch, origin };
    frame(&mut pen);
    pen.slot(FUEL_SLOT);
    let direction = d(data, 2).clamp(0, 5) as usize;
    pen.whole(DIRECTIONS[direction].0, DIRECTION);
    tick_speed(&mut pen, data.get(3).copied().unwrap_or(20));
    pen.text("Delay", 8.0, 49.0, LABEL);
    pen.text("Count", 8.0, 63.0, LABEL);
    number(&mut pen, DELAY, d(data, 5));
    number(&mut pen, COUNT, d(data, 4).max(1));
    labels(&mut pen, d(data, 0), "Dropper T1");
}

/// The Sensor T1's screen (`SensorT1Screen`). The data is the target, strong signal, allowlist, whether the filter slot
/// holds an item and the tick speed.
fn draw_sensor(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let mut pen = Pen { game, batch, origin };
    frame(&mut pen);
    pen.slot(FUEL_SLOT);
    let strong = d(data, 1) != 0;
    pen.whole(if strong { "justdirethings:gui/buttons/redstonehigh" } else { "justdirethings:gui/buttons/redstonelow" }, STRONG);
    pen.whole(if d(data, 2) != 0 { "justdirethings:gui/buttons/allowlisttrue" } else { "justdirethings:gui/buttons/allowlistfalse" }, SENSOR_ALLOW);
    pen.whole(&format!("justdirethings:gui/buttons/{}", TARGETS[d(data, 0).clamp(0, 8) as usize].0), TARGET);
    tick_speed(&mut pen, data.get(4).copied().unwrap_or(20));
    let title = pen.game.native_screen_title().unwrap_or_else(|| "Sensor T1".into());
    let w = HudBatch::text_width(&pen.game.gui.atlas, &title);
    pen.text(&title, 88.0 - (w / 2.0).floor(), 6.0, LABEL);
}

/// The Fluid Generator T1's screen (`GeneratorFluidT1Screen`): the slot, the energy and fluid bars, the redstone button.
/// The data is the redstone mode, whether the slot holds an item, the fluid, the amount, the capacity, the energy and
/// its buffer.
fn draw_fluid_generator(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let mut pen = Pen { game, batch, origin };
    frame(&mut pen);
    pen.slot(FUEL_SLOT);
    let u = 34.0 / 36.0;
    pen.sprite(POWERBAR, [0.0, 0.0, 18.0 * u, 72.0], ENERGY);
    let (energy, max) = (d(data, 5), d(data, 6));
    let rows = (if max > 0 { i64::from(energy) * 70 / i64::from(max) } else { 0 }).clamp(0, 70) as f32;
    pen.sprite(POWERBAR, [19.0 * u, 69.0 - rows, 17.0 * u, rows + 1.0], [ENERGY[0] + 1.0, ENERGY[1] + 70.0 - rows, 17.0, rows + 1.0]);
    fluid_bar(&mut pen, GEN_FLUID, d(data, 2), d(data, 3), d(data, 4));
    labels(&mut pen, d(data, 0), "Fluid Generator T1");
}

/// The Dropper's and Sensor's and fluid generator's menu button a press is on: the redstone button cycles (right click
/// back), the dropper's direction goes on (right back), the number buttons and the tick speed step up on a left click.
fn click_more(block: &str, data: &[i32], at: [f32; 2], right: bool, button: &mut Option<i32>) {
    let mode = Mode::from_index(d(data, 0));
    let cycle = Some(if right { Mode::from_index((mode.index() + 3) % 4) } else { mode.next() }.index());
    if block == DROPPER {
        *button = if inside(REDSTONE, at) {
            cycle
        } else if inside(DIRECTION, at) {
            Some(4 + (d(data, 2).clamp(0, 5) + if right { 5 } else { 1 }) % 6)
        } else if inside(TICK_SPEED, at) {
            Some(if right { 23 } else { 22 })
        } else if inside(COUNT, at) {
            Some(if right { 29 } else { 28 })
        } else if inside(DELAY, at) {
            Some(if right { 31 } else { 30 })
        } else {
            None
        };
    } else if block == SENSOR {
        *button = if right {
            inside(TICK_SPEED, at).then_some(23)
        } else if inside(STRONG, at) {
            Some(33)
        } else if inside(SENSOR_ALLOW, at) {
            Some(24)
        } else if inside(TARGET, at) {
            Some(32)
        } else if inside(TICK_SPEED, at) {
            Some(22)
        } else {
            None
        };
    } else if inside(REDSTONE, at) {
        *button = cycle;
    }
}

fn tooltip_more(block: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    if block == DROPPER {
        if inside(REDSTONE, at) {
            lines.push(redstone_line(Mode::from_index(d(data, 0))).into());
        } else if inside(DIRECTION, at) {
            lines.push(DIRECTIONS[d(data, 2).clamp(0, 5) as usize].1.into());
        } else if inside(TICK_SPEED, at) {
            lines.push(format!("Ticks: {}", d(data, 3)));
        } else if inside(COUNT, at) {
            lines.push(format!("Drop count: {}", d(data, 4)));
        } else if inside(DELAY, at) {
            lines.push(format!("Pickup delay: {}", d(data, 5)));
        }
    } else if block == SENSOR {
        if inside(STRONG, at) {
            lines.push((if d(data, 1) != 0 { "Strong redstone signal" } else { "Weak redstone signal" }).into());
        } else if inside(SENSOR_ALLOW, at) {
            lines.push((if d(data, 2) != 0 { "Allow List" } else { "Deny List" }).into());
        } else if inside(TARGET, at) {
            lines.push(TARGETS[d(data, 0).clamp(0, 8) as usize].1.into());
        } else if inside(TICK_SPEED, at) {
            lines.push(format!("Ticks: {}", d(data, 4)));
        }
    } else if block == FLUID_GEN {
        if inside(REDSTONE, at) {
            lines.push(redstone_line(Mode::from_index(d(data, 0))).into());
        } else if inside(ENERGY, at) {
            lines.push(format!("{} / {} FE", d(data, 5), d(data, 6)));
        } else if inside(GEN_FLUID, at) {
            let name = match d(data, 2) {
                0 => "Empty",
                1 => "Water",
                2 => "Lava",
                _ => "Fluid",
            };
            lines.push(format!("{name}: {} / {} mB", d(data, 3), d(data, 4)));
        }
    }
}

// ---- The Block Swapper T1's screen (`BlockSwapperT1Screen` on `BaseMachineScreen`) -----------------------------------

/// Its buttons on the top section's button row (`topSectionTop + 38`): the partner's icon, the blocks and entity buttons and
/// the redstone button.
const PARTNER: [f32; 4] = [70.0, 58.0, 16.0, 16.0];
const SWAP_BLOCKS: [f32; 4] = [88.0, 58.0, 16.0, 16.0];
const SWAP_ENTITIES: [f32; 4] = [106.0, 58.0, 16.0, 16.0];
const SWAP_REDSTONE: [f32; 4] = [124.0, 58.0, 16.0, 16.0];
/// `SWAPPER_ENTITY_TEXTURES`, in the order of `SWAP_ENTITY_TYPE`.
const SWAP_ENTITY_BUTTONS: [(&str, &str); 9] = [
    ("justdirethings:gui/buttons/entity-none", "Entities: none"),
    ("justdirethings:gui/buttons/mobscanner", "Entities: hostile"),
    ("justdirethings:gui/buttons/passivemob", "Entities: passive"),
    ("justdirethings:gui/buttons/passivemob-adult", "Entities: adults"),
    ("justdirethings:gui/buttons/passivemob-child", "Entities: children"),
    ("justdirethings:gui/buttons/player", "Entities: players"),
    ("justdirethings:gui/buttons/glowing", "Entities: living"),
    ("justdirethings:gui/buttons/item", "Entities: items"),
    ("justdirethings:gui/buttons/entity-all", "Entities: all"),
];

/// The data is the redstone mode, whether blocks swap, the entity setting, whether the partner stands and whether it is bound.
fn draw_swapper(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let mut pen = Pen { game, batch, origin };
    frame(&mut pen);
    let blocks = d(data, 1) != 0;
    pen.whole(if blocks { "justdirethings:gui/buttons/filter-block" } else { "justdirethings:gui/buttons/filter-air" }, SWAP_BLOCKS);
    pen.whole(SWAP_ENTITY_BUTTONS[d(data, 2).clamp(0, 8) as usize].0, SWAP_ENTITIES);
    pen.whole(if d(data, 3) != 0 { "justdirethings:gui/buttons/active" } else { "justdirethings:gui/buttons/inactive" }, PARTNER);
    labels_at(&mut pen, d(data, 0), SWAP_REDSTONE, "Simple Swapper");
}

fn click_swapper(data: &[i32], at: [f32; 2], right: bool, button: &mut Option<i32>) {
    if inside(SWAP_REDSTONE, at) {
        if !right {
            *button = Some(Mode::from_index(d(data, 0)).next().index());
        }
    } else if inside(SWAP_BLOCKS, at) {
        *button = Some(4);
    } else if inside(SWAP_ENTITIES, at) {
        *button = Some(5 + (d(data, 2).clamp(0, 8) + if right { 8 } else { 1 }) % 9);
    }
}

fn tooltip_swapper(data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    if inside(SWAP_REDSTONE, at) {
        lines.push(redstone_line(Mode::from_index(d(data, 0))).into());
    } else if inside(SWAP_BLOCKS, at) {
        lines.push(if d(data, 1) != 0 { "Swaps blocks" } else { "Leaves blocks" }.into());
    } else if inside(SWAP_ENTITIES, at) {
        lines.push(SWAP_ENTITY_BUTTONS[d(data, 2).clamp(0, 8) as usize].1.into());
    } else if inside(PARTNER, at) {
        lines.push(match (d(data, 4) != 0, d(data, 3) != 0) {
            (true, true) => "Bound to a swapper",
            (true, false) => "Bound to (MISSING BLOCK)",
            _ => "Not bound: use a Ferricore Wrench on two swappers",
        }.into());
    }
}

// ---- The Clickers' screens (`ClickerT1Screen`, `ClickerT2Screen` on `BaseMachineScreen`) -----------------------------

/// The T1's buttons (`topSectionTop + 38`): show the fake player, sneak, click type, target; the hold ticks' number
/// button under them while the click type is a hold; the redstone, direction and tick speed buttons are the placer's.
const C1_SHOW: [f32; 4] = [2.0, 58.0, 16.0, 16.0];
const C1_SNEAK: [f32; 4] = [20.0, 58.0, 16.0, 16.0];
const C1_TYPE: [f32; 4] = [38.0, 58.0, 16.0, 16.0];
const C1_TARGET: [f32; 4] = [56.0, 58.0, 16.0, 16.0];
const C1_HOLD: [f32; 4] = [34.0, 75.0, 24.0, 12.0];
/// The T2's (`topSectionTop + 44` and `+ 62`), beside the area's value buttons and the filter's; its energy bar
/// stands left of the panel, in the 60 pixels Java's T2 screens widen their top section by.
const C2_SHOW: [f32; 4] = [8.0, 64.0, 16.0, 16.0];
const C2_SNEAK: [f32; 4] = [26.0, 64.0, 16.0, 16.0];
const C2_TYPE: [f32; 4] = [44.0, 64.0, 16.0, 16.0];
const C2_TARGET: [f32; 4] = [44.0, 82.0, 16.0, 16.0];
const C2_HOLD: [f32; 4] = [64.0, 84.0, 24.0, 12.0];
const C2_DIRECTION: [f32; 4] = [116.0, 82.0, 16.0, 16.0];
const C2_ALLOWLIST: [f32; 4] = [8.0, 82.0, 16.0, 16.0];
const C2_COMPARE: [f32; 4] = [26.0, 82.0, 16.0, 16.0];
const C2_ENERGY: [f32; 4] = [-25.0, 25.0, 18.0, 72.0];
const CLICK_TYPES: [(&str, &str); 3] = [
    ("justdirethings:gui/buttons/click-right", "Right Click"),
    ("justdirethings:gui/buttons/click-left", "Left Click"),
    ("justdirethings:gui/buttons/click-hold", "Hold Click"),
];

struct ClickerLayout {
    show: [f32; 4],
    sneak: [f32; 4],
    kind: [f32; 4],
    target: [f32; 4],
    hold: [f32; 4],
    redstone: [f32; 4],
    direction: [f32; 4],
}

fn clicker_layout(t2: bool) -> ClickerLayout {
    if t2 {
        ClickerLayout { show: C2_SHOW, sneak: C2_SNEAK, kind: C2_TYPE, target: C2_TARGET, hold: C2_HOLD, redstone: REDSTONE_BASE, direction: C2_DIRECTION }
    } else {
        ClickerLayout { show: C1_SHOW, sneak: C1_SNEAK, kind: C1_TYPE, target: C1_TARGET, hold: C1_HOLD, redstone: REDSTONE, direction: DIRECTION }
    }
}

/// The T1's menu: the hopper's with the machine's slot; the T2's: a 9x2 chest's, the machine's slot and the nine filter
/// slots in a row inside the top section.
fn clicker_screen(t2: bool) -> Screen {
    let mut slots = vec![Some(FUEL_SLOT)];
    if t2 {
        slots.extend((0..9).map(|i| Some([FILTER_ROW[0] + 18.0 * i as f32, FILTER_ROW[1]])));
        slots.resize(18, None);
    } else {
        slots.resize(5, None);
    }
    Screen {
        width: 176.0,
        height: 166.0 + TOP + 2.0,
        slots,
        inventory_x: INVENTORY[0],
        inventory_y: INVENTORY[1],
        own_labels: true,
        own_background: true,
        ..Screen::default()
    }
}

/// A clicker's screen; the data is `justdirethings_clicker::screen_data`'s.
fn draw_clicker(game: &Game<'_>, t2: bool, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let l = clicker_layout(t2);
    let mut pen = Pen { game, batch, origin };
    if t2 {
        pen.panel([-30.0, 20.0, 34.0, 102.0]);
    }
    frame(&mut pen);
    pen.slot(FUEL_SLOT);
    toggle(&mut pen, "justdirethings:gui/buttons/showfakeplayer", l.show, d(data, 7) != 0);
    toggle(&mut pen, "justdirethings:gui/buttons/click-sneak", l.sneak, d(data, 6) != 0);
    let kind = d(data, 4).clamp(0, 2);
    pen.whole(CLICK_TYPES[kind as usize].0, l.kind);
    let target = justdirethings_core::clicker::Target::from_index(d(data, 5));
    pen.whole(&format!("justdirethings:gui/buttons/{}", target.button().0), l.target);
    if kind == 2 {
        number(&mut pen, l.hold, d(data, 8));
    }
    pen.whole(DIRECTIONS[d(data, 2).clamp(0, 5) as usize].0, l.direction);
    tick_speed(&mut pen, data.get(3).copied().unwrap_or(20));
    if t2 {
        for i in 0..9 {
            pen.slot([FILTER_ROW[0] + 18.0 * i as f32, FILTER_ROW[1]]);
        }
        for axis in 0..3 {
            for row in 0..2 {
                let [x, y] = value_button(axis, row);
                pen.whole("justdirethings:gui/buttons/remove", [x, y, 12.0, 12.0]);
                pen.whole("justdirethings:gui/buttons/add", [x + 30.0, y, 12.0, 12.0]);
                let value = if row == 0 { format!("{:.1}", f64::from(d(data, 10 + axis)) / 2.0) } else { d(data, 13 + axis).to_string() };
                pen.text(&value, x + 11.0, y + 2.0, LABEL);
            }
        }
        let allow = d(data, 16) != 0;
        pen.whole(if allow { "justdirethings:gui/buttons/allowlisttrue" } else { "justdirethings:gui/buttons/allowlistfalse" }, C2_ALLOWLIST);
        toggle(&mut pen, "justdirethings:gui/buttons/matchnbttrue", C2_COMPARE, d(data, 17) != 0);
        toggle(&mut pen, "justdirethings:gui/buttons/area", RENDER_AREA, d(data, 18) != 0);
        let u = 34.0 / 36.0;
        pen.sprite(POWERBAR, [0.0, 0.0, 18.0 * u, 72.0], C2_ENERGY);
        let rows = justdirethings_core::t2::Powered { energy: d(data, 9) }.rows().clamp(0, 70) as f32;
        pen.sprite(POWERBAR, [19.0 * u, 69.0 - rows, 17.0 * u, rows + 1.0], [C2_ENERGY[0] + 1.0, C2_ENERGY[1] + 70.0 - rows, 17.0, rows + 1.0]);
    }
    labels_at(&mut pen, d(data, 0), l.redstone, if t2 { "Clicker T2" } else { "Clicker T1" });
}

/// The menu button a press on a clicker's screen is on (`justdirethings_clicker::button`'s ids): toggles flip on a left
/// click, the cycling buttons go on (right back), the numbers step up (right down).
fn click_clicker(t2: bool, data: &[i32], at: [f32; 2], right: bool, button: &mut Option<i32>) {
    let l = clicker_layout(t2);
    let step = |value: i32, n: i32| (value.clamp(0, n - 1) + if right { n - 1 } else { 1 }) % n;
    *button = if inside(l.redstone, at) {
        Some(step(d(data, 0), 4))
    } else if inside(l.direction, at) {
        Some(34 + step(d(data, 2), 6))
    } else if inside(TICK_SPEED, at) {
        Some(if right { 23 } else { 22 })
    } else if inside(l.kind, at) {
        Some(40 + step(d(data, 4), 3))
    } else if inside(l.target, at) {
        Some(43 + step(d(data, 5), 8))
    } else if d(data, 4) == 2 && inside(l.hold, at) {
        Some(if right { 54 } else { 53 })
    } else if right {
        None
    } else if inside(l.sneak, at) {
        Some(51)
    } else if inside(l.show, at) {
        Some(52)
    } else if !t2 {
        None
    } else if inside(RENDER_AREA, at) {
        Some(9)
    } else if inside(C2_ALLOWLIST, at) {
        Some(24)
    } else if inside(C2_COMPARE, at) {
        Some(25)
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

fn tooltip_clicker(t2: bool, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    let l = clicker_layout(t2);
    let line = if inside(l.redstone, at) {
        redstone_line(Mode::from_index(d(data, 0))).to_owned()
    } else if inside(l.direction, at) {
        DIRECTIONS[d(data, 2).clamp(0, 5) as usize].1.to_owned()
    } else if inside(TICK_SPEED, at) {
        format!("Speed (Ticks): {}", d(data, 3))
    } else if inside(l.kind, at) {
        CLICK_TYPES[d(data, 4).clamp(0, 2) as usize].1.to_owned()
    } else if inside(l.target, at) {
        justdirethings_core::clicker::Target::from_index(d(data, 5)).button().1.to_owned()
    } else if d(data, 4) == 2 && inside(l.hold, at) {
        format!("Hold Clicks (Ticks): {}", d(data, 8))
    } else if inside(l.sneak, at) {
        "Sneak Click".to_owned()
    } else if inside(l.show, at) {
        "Show Fake Player".to_owned()
    } else if t2 && inside(C2_ENERGY, at) {
        format!("{} / {} FE", d(data, 9), justdirethings_core::t2::CAPACITY)
    } else if t2 && inside(C2_ALLOWLIST, at) {
        (if d(data, 16) != 0 { "Allow List" } else { "Deny List" }).to_owned()
    } else if t2 && inside(C2_COMPARE, at) {
        "Compare NBT".to_owned()
    } else if t2 && inside(RENDER_AREA, at) {
        "Render Area".to_owned()
    } else {
        return;
    };
    lines.push(line);
}

// ---- The Block Placer T2's screen (`BlockPlacerT2Screen` on `BaseMachineScreen`) ----------------------------------------

/// The direction button beside the redstone button on the last row.
const P2_DIRECTION: [f32; 4] = [116.0, 82.0, 16.0, 16.0];

/// Which of the shared T2 parts a screen has: the radius row (a screen without it has its offset row in the radius row's
/// place), the allowlist button, and the compare button where it stands (none: no button).
#[derive(Clone, Copy)]
struct Parts {
    radius: bool,
    allow: bool,
    compare: Option<[f32; 4]>,
}

const ALL_PARTS: Parts = Parts { radius: true, allow: true, compare: Some(C2_COMPARE) };

/// The T2 parts every T2 screen shares (`BaseMachineScreen` for a `PoweredMachineBE`, `AreaAffectingBE`, `FilterableBE`):
/// the energy bar left of the panel, the filter slots, the area's value buttons, the allowlist, compare and render area
/// buttons. `energy`: where in `data` the energy, radius, offsets and flags start.
fn draw_t2_parts(pen: &mut Pen<'_, '_>, data: &[i32], energy: usize) {
    draw_t2_parts_with(pen, data, energy, ALL_PARTS);
}

fn draw_t2_parts_with(pen: &mut Pen<'_, '_>, data: &[i32], energy: usize, p: Parts) {
    for i in 0..9 {
        pen.slot([FILTER_ROW[0] + 18.0 * i as f32, FILTER_ROW[1]]);
    }
    for axis in 0..3 {
        for row in 0..2 {
            if row == 0 && !p.radius {
                continue;
            }
            let [x, y] = value_button(axis, if p.radius { row } else { 0 });
            pen.whole("justdirethings:gui/buttons/remove", [x, y, 12.0, 12.0]);
            pen.whole("justdirethings:gui/buttons/add", [x + 30.0, y, 12.0, 12.0]);
            let value = if row == 0 { format!("{:.1}", f64::from(d(data, energy + 1 + axis)) / 2.0) } else { d(data, energy + 4 + axis).to_string() };
            pen.text(&value, x + 11.0, y + 2.0, LABEL);
        }
    }
    if p.allow {
        let allow = d(data, energy + 7) != 0;
        pen.whole(if allow { "justdirethings:gui/buttons/allowlisttrue" } else { "justdirethings:gui/buttons/allowlistfalse" }, C2_ALLOWLIST);
    }
    if let Some(at) = p.compare {
        toggle(pen, "justdirethings:gui/buttons/matchnbttrue", at, d(data, energy + 8) != 0);
    }
    toggle(pen, "justdirethings:gui/buttons/area", RENDER_AREA, d(data, energy + 9) != 0);
    let u = 34.0 / 36.0;
    pen.sprite(POWERBAR, [0.0, 0.0, 18.0 * u, 72.0], C2_ENERGY);
    let rows = justdirethings_core::t2::Powered { energy: d(data, energy) }.rows().clamp(0, 70) as f32;
    pen.sprite(POWERBAR, [19.0 * u, 69.0 - rows, 17.0 * u, rows + 1.0], [C2_ENERGY[0] + 1.0, C2_ENERGY[1] + 70.0 - rows, 17.0, rows + 1.0]);
}

/// The shared T2 buttons a left press is on: render area 9, allowlist 24, compare 25, the value buttons 10 to 21.
fn click_t2_parts(at: [f32; 2]) -> Option<i32> {
    click_t2_parts_with(at, ALL_PARTS)
}

fn click_t2_parts_with(at: [f32; 2], p: Parts) -> Option<i32> {
    if inside(RENDER_AREA, at) {
        return Some(9);
    }
    if p.allow && inside(C2_ALLOWLIST, at) {
        return Some(24);
    }
    if p.compare.is_some_and(|c| inside(c, at)) {
        return Some(25);
    }
    (0..3).flat_map(|axis| (0..2).map(move |row| (axis, row))).filter(|(_, row)| p.radius || *row == 1).find_map(|(axis, row)| {
        let [x, y] = value_button(axis, if p.radius { row } else { 0 });
        let base = if row == 0 { 10 } else { 16 } + 2 * axis as i32;
        if inside([x, y, 12.0, 12.0], at) {
            Some(base + 1)
        } else if inside([x + 30.0, y, 12.0, 12.0], at) {
            Some(base)
        } else {
            None
        }
    })
}

/// The Placer T2's screen; the data is `justdirethings_t2::screen_data`'s.
fn draw_placer2(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let mut pen = Pen { game, batch, origin };
    pen.panel([-30.0, 20.0, 34.0, 102.0]);
    frame(&mut pen);
    pen.slot(FUEL_SLOT);
    pen.whole(DIRECTIONS[d(data, 2).clamp(0, 5) as usize].0, P2_DIRECTION);
    tick_speed(&mut pen, data.get(3).copied().unwrap_or(20));
    draw_t2_parts(&mut pen, data, 4);
    labels_at(&mut pen, d(data, 0), REDSTONE_BASE, "Block Placer T2");
}

fn click_placer2(data: &[i32], at: [f32; 2], right: bool, button: &mut Option<i32>) {
    let step = |value: i32, n: i32| (value.clamp(0, n - 1) + if right { n - 1 } else { 1 }) % n;
    *button = if inside(REDSTONE_BASE, at) {
        Some(step(d(data, 0), 4))
    } else if inside(P2_DIRECTION, at) {
        Some(34 + step(d(data, 2), 6))
    } else if inside(TICK_SPEED, at) {
        Some(if right { 23 } else { 22 })
    } else if right {
        None
    } else {
        click_t2_parts(at)
    };
}

fn tooltip_placer2(data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    let line = if inside(REDSTONE_BASE, at) {
        redstone_line(Mode::from_index(d(data, 0))).to_owned()
    } else if inside(P2_DIRECTION, at) {
        DIRECTIONS[d(data, 2).clamp(0, 5) as usize].1.to_owned()
    } else if inside(TICK_SPEED, at) {
        format!("Speed (Ticks): {}", d(data, 3))
    } else if inside(C2_ENERGY, at) {
        format!("{} / {} FE", d(data, 4), justdirethings_core::t2::CAPACITY)
    } else if inside(C2_ALLOWLIST, at) {
        (if d(data, 11) != 0 { "Allow List" } else { "Deny List" }).to_owned()
    } else if inside(C2_COMPARE, at) {
        "Compare NBT".to_owned()
    } else if inside(RENDER_AREA, at) {
        "Render Area".to_owned()
    } else {
        return;
    };
    lines.push(line);
}

/// The Breaker T2's screen: its tool slot (the pickaxe outline while empty), the tick speed and the T2 parts; the data is
/// 0 redstone, 1 whether the slot holds a tool, 2 tick speed, then the T2 parts from 3.
fn draw_breaker2(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let mut pen = Pen { game, batch, origin };
    pen.panel([-30.0, 20.0, 34.0, 102.0]);
    frame(&mut pen);
    let src = if d(data, 1) != 0 { [0.0, 0.0, 18.0, 18.0] } else { [18.0, 0.0, 18.0, 18.0] };
    pen.sprite(SLOT, src, [FUEL_SLOT[0] - 1.0, FUEL_SLOT[1] - 1.0, 18.0, 18.0]);
    tick_speed(&mut pen, data.get(2).copied().unwrap_or(20));
    draw_t2_parts(&mut pen, data, 3);
    labels_at(&mut pen, d(data, 0), REDSTONE_BASE, "Block Breaker T2");
}

fn click_breaker2(data: &[i32], at: [f32; 2], right: bool, button: &mut Option<i32>) {
    *button = if inside(REDSTONE_BASE, at) {
        let mode = d(data, 0).clamp(0, 3);
        Some((mode + if right { 3 } else { 1 }) % 4)
    } else if inside(TICK_SPEED, at) {
        Some(if right { 23 } else { 22 })
    } else if right {
        None
    } else {
        click_t2_parts(at)
    };
}

fn tooltip_breaker2(data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    let line = if inside(REDSTONE_BASE, at) {
        redstone_line(Mode::from_index(d(data, 0))).to_owned()
    } else if inside(TICK_SPEED, at) {
        format!("Speed (Ticks): {}", d(data, 2))
    } else if inside(C2_ENERGY, at) {
        format!("{} / {} FE", d(data, 3), justdirethings_core::t2::CAPACITY)
    } else if inside(C2_ALLOWLIST, at) {
        (if d(data, 10) != 0 { "Allow List" } else { "Deny List" }).to_owned()
    } else if inside(C2_COMPARE, at) {
        "Compare NBT".to_owned()
    } else if inside(RENDER_AREA, at) {
        "Render Area".to_owned()
    } else {
        return;
    };
    lines.push(line);
}

// ---- The other T2 machines' screens (`DropperT2Screen`, `SensorT2Screen`, `FluidPlacerT2Screen`, `FluidCollectorT2Screen`,
// `BlockSwapperT2Screen` on `BaseMachineScreen`) -------------------------------------------------------------------------

fn is_t2x(block: &str) -> bool {
    block == DROPPER_T2 || block == SENSOR_T2 || block == FLUID_PLACER_T2 || block == FLUID_COLLECTOR_T2 || block == SWAPPER_T2
}

/// The Dropper T2's nine machine slots (`addSlotBox(…, 62, -1, 3, 18, 3, 18)`), its count and pickup delay number buttons
/// (`topSectionTop + 41` and `+ 27`) and its compare button, which stands where the allowlist does elsewhere.
const D2_GRID: [f32; 2] = [62.0, 45.0];
const D2_COUNT: [f32; 4] = [20.0, 61.0, 24.0, 12.0];
const D2_DELAY: [f32; 4] = [20.0, 47.0, 24.0, 12.0];
const D2_PARTS: Parts = Parts { radius: false, allow: false, compare: Some([8.0, 82.0, 16.0, 16.0]) };
/// The Sensor T2's target, strong or weak, count test and amount buttons (`topSectionTop + 62`; the amount `+ 64`).
const S2_TARGET: [f32; 4] = [26.0, 82.0, 16.0, 16.0];
const S2_STRONG: [f32; 4] = [44.0, 82.0, 16.0, 16.0];
const S2_EQUALITY: [f32; 4] = [104.0, 82.0, 16.0, 16.0];
const S2_AMOUNT: [f32; 4] = [122.0, 84.0, 24.0, 12.0];
const S2_PARTS: Parts = Parts { radius: true, allow: true, compare: None };
/// The fluid machines' second bar, beside the energy bar on the wider side panel.
const FLUID2: [f32; 4] = [-49.0, 25.0, 18.0, 72.0];
/// The Swapper T2's blocks, entity and partner icons (`topSectionTop + 44`, `+ 38`).
const W2_BLOCKS: [f32; 4] = [8.0, 64.0, 16.0, 16.0];
const W2_ENTITIES: [f32; 4] = [26.0, 64.0, 16.0, 16.0];
const W2_PARTNER: [f32; 4] = [156.0, 58.0, 16.0, 16.0];
const EQUALS: [&str; 3] = [">", "<", "="];

/// Where the T2 parts start in each machine's data.
fn t2x_energy(block: &str) -> usize {
    match block {
        DROPPER_T2 => 6,
        SENSOR_T2 => 5,
        SWAPPER_T2 => 5,
        _ => 7,
    }
}

fn t2x_parts(block: &str) -> Parts {
    match block {
        DROPPER_T2 => D2_PARTS,
        SENSOR_T2 => S2_PARTS,
        _ => ALL_PARTS,
    }
}

/// The menu: the dropper's nine slots then its filter; the sensor's and swapper's filter slots alone; the fluid machines'
/// slot then the filter.
fn t2x_screen(block: &str) -> Screen {
    let filter = (0..9).map(|i| Some([FILTER_ROW[0] + 18.0 * i as f32, FILTER_ROW[1]]));
    let mut slots: Vec<Option<[f32; 2]>> = match block {
        DROPPER_T2 => (0..9).map(|i| Some([D2_GRID[0] + 18.0 * (i % 3) as f32, D2_GRID[1] + 18.0 * (i / 3) as f32])).collect(),
        SENSOR_T2 | SWAPPER_T2 => vec![None],
        _ => vec![Some(FUEL_SLOT)],
    };
    slots.extend(filter);
    slots.resize(18, None);
    Screen {
        width: 176.0,
        height: 166.0 + TOP + 2.0,
        slots,
        inventory_x: INVENTORY[0],
        inventory_y: INVENTORY[1],
        own_labels: true,
        own_background: true,
        ..Screen::default()
    }
}

fn draw_t2x(game: &Game<'_>, block: &str, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let mut pen = Pen { game, batch, origin };
    let fluid = block == FLUID_PLACER_T2 || block == FLUID_COLLECTOR_T2;
    pen.panel(if fluid { [-54.0, 20.0, 58.0, 102.0] } else { [-30.0, 20.0, 34.0, 102.0] });
    frame(&mut pen);
    let energy = t2x_energy(block);
    draw_t2_parts_with(&mut pen, data, energy, t2x_parts(block));
    match block {
        DROPPER_T2 => {
            for i in 0..9 {
                pen.slot([D2_GRID[0] + 18.0 * (i % 3) as f32, D2_GRID[1] + 18.0 * (i / 3) as f32]);
            }
            pen.whole(DIRECTIONS[d(data, 2).clamp(0, 5) as usize].0, DIRECTION);
            tick_speed(&mut pen, d(data, 3));
            number(&mut pen, D2_DELAY, d(data, 5));
            number(&mut pen, D2_COUNT, d(data, 4).max(1));
            labels_at(&mut pen, d(data, 0), REDSTONE_BASE, "Dropper T2");
        }
        SENSOR_T2 => {
            pen.whole(&format!("justdirethings:gui/buttons/{}", TARGETS[d(data, 0).clamp(0, 8) as usize].0), S2_TARGET);
            pen.whole(if d(data, 1) != 0 { "justdirethings:gui/buttons/redstonehigh" } else { "justdirethings:gui/buttons/redstonelow" }, S2_STRONG);
            pen.whole("justdirethings:gui/buttons/blank", S2_EQUALITY);
            pen.text(EQUALS[d(data, 2).clamp(0, 2) as usize], S2_EQUALITY[0] + 6.0, S2_EQUALITY[1] + 4.0, LABEL);
            number(&mut pen, S2_AMOUNT, d(data, 3));
            tick_speed(&mut pen, d(data, 4));
            let title = pen.game.native_screen_title().unwrap_or_else(|| "Sensor T2".into());
            let w = HudBatch::text_width(&pen.game.gui.atlas, &title);
            pen.text(&title, 88.0 - (w / 2.0).floor(), 6.0, LABEL);
        }
        SWAPPER_T2 => {
            pen.whole(if d(data, 1) != 0 { "justdirethings:gui/buttons/filter-block" } else { "justdirethings:gui/buttons/filter-air" }, W2_BLOCKS);
            pen.whole(SWAP_ENTITY_BUTTONS[d(data, 2).clamp(0, 8) as usize].0, W2_ENTITIES);
            pen.whole(if d(data, 3) != 0 { "justdirethings:gui/buttons/active" } else { "justdirethings:gui/buttons/inactive" }, W2_PARTNER);
            labels_at(&mut pen, d(data, 0), REDSTONE_BASE, "Block Swapper T2");
        }
        _ => {
            pen.slot(FUEL_SLOT);
            fluid_bar(&mut pen, FLUID2, d(data, 3), d(data, 4), d(data, 5));
            tick_speed(&mut pen, d(data, 2));
            if block == FLUID_PLACER_T2 {
                pen.whole(DIRECTIONS[d(data, 6).clamp(0, 5) as usize].0, P2_DIRECTION);
            }
            labels_at(&mut pen, d(data, 0), REDSTONE_BASE, if block == FLUID_PLACER_T2 { "Fluid Placer T2" } else { "Fluid Collector T2" });
        }
    }
}

fn click_t2x(block: &str, data: &[i32], at: [f32; 2], right: bool, button: &mut Option<i32>) {
    let step = |value: i32, n: i32| (value.clamp(0, n - 1) + if right { n - 1 } else { 1 }) % n;
    let tick = |slot: usize| (inside(TICK_SPEED, at) && d(data, slot) >= 0).then_some(if right { 23 } else { 22 });
    let redstone = || inside(REDSTONE_BASE, at).then(|| step(d(data, 0), 4));
    let parts = || if right { None } else { click_t2_parts_with(at, t2x_parts(block)) };
    *button = match block {
        DROPPER_T2 => redstone()
            .or_else(|| inside(DIRECTION, at).then(|| 34 + step(d(data, 2), 6)))
            .or_else(|| tick(3))
            .or_else(|| inside(D2_COUNT, at).then_some(if right { 29 } else { 28 }))
            .or_else(|| inside(D2_DELAY, at).then_some(if right { 31 } else { 30 }))
            .or_else(parts),
        SENSOR_T2 => tick(4)
            .or_else(|| inside(S2_TARGET, at).then_some(32))
            .or_else(|| inside(S2_STRONG, at).then_some(33))
            .or_else(|| inside(S2_EQUALITY, at).then(|| 60 + step(d(data, 2), 3)))
            .or_else(|| inside(S2_AMOUNT, at).then_some(if right { 64 } else { 63 }))
            .or_else(parts),
        SWAPPER_T2 => redstone()
            .or_else(|| (!right && inside(W2_BLOCKS, at)).then_some(4))
            .or_else(|| inside(W2_ENTITIES, at).then(|| 70 + step(d(data, 2), 9)))
            .or_else(parts),
        _ => redstone()
            .or_else(|| (block == FLUID_PLACER_T2 && inside(P2_DIRECTION, at)).then(|| 34 + step(d(data, 6), 6)))
            .or_else(|| tick(2))
            .or_else(parts),
    };
}

fn tooltip_t2x(block: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    let line = if block != SENSOR_T2 && inside(REDSTONE_BASE, at) {
        redstone_line(Mode::from_index(d(data, 0))).to_owned()
    } else if inside(C2_ENERGY, at) {
        format!("{} / {} FE", d(data, t2x_energy(block)), justdirethings_core::t2::CAPACITY)
    } else if inside(RENDER_AREA, at) {
        "Render Area".to_owned()
    } else if block == DROPPER_T2 && inside(DIRECTION, at) {
        DIRECTIONS[d(data, 2).clamp(0, 5) as usize].1.to_owned()
    } else if block == DROPPER_T2 && inside(D2_COUNT, at) {
        format!("Drop count: {}", d(data, 4))
    } else if block == DROPPER_T2 && inside(D2_DELAY, at) {
        format!("Pickup delay: {}", d(data, 5))
    } else if block == DROPPER_T2 && inside(TICK_SPEED, at) {
        format!("Speed (Ticks): {}", d(data, 3))
    } else if block == DROPPER_T2 && t2x_parts(block).compare.is_some_and(|c| inside(c, at)) {
        "Compare NBT".to_owned()
    } else if block == SENSOR_T2 && inside(S2_TARGET, at) {
        TARGETS[d(data, 0).clamp(0, 8) as usize].1.to_owned()
    } else if block == SENSOR_T2 && inside(S2_STRONG, at) {
        (if d(data, 1) != 0 { "Strong redstone signal" } else { "Weak redstone signal" }).to_owned()
    } else if block == SENSOR_T2 && inside(S2_EQUALITY, at) {
        ["Greater than", "Less than", "Equals"][d(data, 2).clamp(0, 2) as usize].to_owned()
    } else if block == SENSOR_T2 && inside(S2_AMOUNT, at) {
        format!("Amount: {}", d(data, 3))
    } else if block == SENSOR_T2 && inside(TICK_SPEED, at) {
        format!("Speed (Ticks): {}", d(data, 4))
    } else if block == SWAPPER_T2 && inside(W2_BLOCKS, at) {
        (if d(data, 1) != 0 { "Swaps blocks" } else { "Leaves blocks" }).to_owned()
    } else if block == SWAPPER_T2 && inside(W2_ENTITIES, at) {
        SWAP_ENTITY_BUTTONS[d(data, 2).clamp(0, 8) as usize].1.to_owned()
    } else if block == SWAPPER_T2 && inside(W2_PARTNER, at) {
        match (d(data, 4) != 0, d(data, 3) != 0) {
            (true, true) => "Bound to a swapper",
            (true, false) => "Bound to (MISSING BLOCK)",
            _ => "Not bound: use a Ferricore Wrench on two swappers",
        }
        .to_owned()
    } else if (block == FLUID_PLACER_T2 || block == FLUID_COLLECTOR_T2) && inside(TICK_SPEED, at) {
        format!("Speed (Ticks): {}", d(data, 2))
    } else if block == FLUID_PLACER_T2 && inside(P2_DIRECTION, at) {
        DIRECTIONS[d(data, 6).clamp(0, 5) as usize].1.to_owned()
    } else if (block == FLUID_PLACER_T2 || block == FLUID_COLLECTOR_T2) && inside(FLUID2, at) {
        let name = match d(data, 3) {
            0 => "Empty",
            1 => "Water",
            2 => "Lava",
            _ => "Fluid",
        };
        format!("{name}: {} / {} mB", d(data, 4), d(data, 5))
    } else if inside(C2_ALLOWLIST, at) && t2x_parts(block).allow {
        (if d(data, t2x_energy(block) + 7) != 0 { "Allow List" } else { "Deny List" }).to_owned()
    } else if t2x_parts(block).compare.is_some_and(|c| inside(c, at)) {
        "Compare NBT".to_owned()
    } else {
        return;
    };
    lines.push(line);
}
