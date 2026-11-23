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
    if !(block == GENERATOR || block == BREAKER || block == PLACER || block == COLLECTOR) {
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
    if block == COLLECTOR {
        draw_collector(game, data, batch, origin);
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

/// The Fluid Collector T1's screen (`FluidCollectorT1Screen` on `BaseMachineScreen`): its slot, the fluid bar where
/// an unpowered machine has it (`topSectionLeft + 5`), its fluid's still texture in its tint (`renderFluid`), the
/// redstone and tick speed buttons. The data is the redstone mode, whether the slot holds an item, the tick speed, the
/// fluid (0 none, 1 water, 2 lava, 3 another), the amount and the capacity.
fn draw_collector(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let mut pen = Pen { game, batch, origin };
    frame(&mut pen);
    pen.slot(FUEL_SLOT);
    let bar = ENERGY;
    let u = game.native_sprite_size(FLUIDBAR).map_or(1.0, |[w, _]| w / 36.0);
    pen.sprite(FLUIDBAR, [0.0, 0.0, 18.0 * u, 72.0], bar);
    let (code, amount, capacity) = (data.get(3).copied().unwrap_or(0), data.get(4).copied().unwrap_or(0), data.get(5).copied().unwrap_or(0));
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
                pen.tinted(POWERBAR, [19.0 * (34.0 / 36.0), 0.0, 16.0 * (34.0 / 36.0), h], [bar[0] + 1.0, y, 16.0, h], Some(0xFF3F_76E4));
            }
            left -= h;
        }
    }
    pen.sprite(FLUIDBAR, [18.0 * u, 0.0, 18.0 * u, 72.0], bar);
    tick_speed(&mut pen, data.get(2).copied().unwrap_or(20));
    labels_at(&mut pen, data.first().copied().unwrap_or(0), REDSTONE_BASE, "Fluid Collector T1");
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
    } else if block == COLLECTOR && inside(ENERGY, at) {
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
    if block == PLACER || block == COLLECTOR {
        click_machine(block, data, at, right, button);
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
    if block == PLACER || block == COLLECTOR {
        tooltip_machine(block, data, at, lines);
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
