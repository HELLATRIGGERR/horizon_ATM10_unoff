// The Energy Cube's screen (support module `crate::game::mek_ui` of the
// `mekanism` native bundle), drawn like Mekanism 10.7's `GuiEnergyCube` with
// Mekanism's own GUI textures (`mekanism:gui/...`, which `mekanism-content`
// copies from the carry): its panel, the wide energy gauge, the discharge (-)
// and charge (+) slots, the side configuration and energy tabs, and the side
// configuration (`GuiSideConfiguration`) with its energy and item pages. The
// Solar and Heat Generators' screens (`GuiSolarGenerator`, `GuiHeatGenerator`)
// are drawn the same way, with Mekanism Generators' textures for the sun, and
// so are the processing machines' (`GuiElectricMachine`,
// `GuiMetallurgicInfuser`): their slots, progress, power bar, the infuser's
// infusion bar and dump button, the side configuration with energy, item and
// chemical pages, and the upgrade window. A machine's slots fill its panel,
// so its windows open beside it, on the left, and the upgrade component's
// two slots sit in a holder on the right, under the upgrade tab (Mekanism
// shows them only in its upgrade window).
//
// Mekanism draws the side configuration as a floating 156x135 window; here
// the game draws a screen's items over everything the mod draws, so it is a
// page over the cube's upper half, between its two slots and above the
// inventory, with the same buttons, textures and behaviour. Which page shows
// is the screen's own (Mekanism's windows are client-side too): page buttons
// still go to the server, which ignores them, so the press is taken.
//
// It also writes a cube item's tooltip (stored energy and capacity) and its
// energy bar, from the energy its custom data keeps (`mek_world::item_data`).

use super::{Game, native_hooks::NativeScreen};
use horizon_client::{hud::HudBatch, items::ItemStack, text::Text};
use mekanism_energy::{
    config::{self, EnergyConfig},
    cube,
    display,
    generator::GeneratorKind,
    machine::{self, MachineKind, Upgrade},
    menu::{Button, GeneratorView, Kind, MachineButton, MachineView, View},
    sides::{DataType, RelativeSide},
    tier::CubeTier,
};
use std::{
    cell::{Cell, RefCell},
    time::{Duration, Instant},
};

const TITLE: u32 = 0x404040;
const SCREEN_TEXT: u32 = 0x3CFE9A;

thread_local! {
    /// 0: the cube; 1: energy side configuration; 2: item side configuration.
    static PAGE: Cell<u8> = const { Cell::new(0) };
    /// The settings this side reads (`energyType`, tier sizes), when their
    /// files last changed, and when that was last looked at.
    static SETTINGS: RefCell<Option<(EnergyConfig, Stamp, Instant)>> = const { RefCell::new(None) };
}

type Stamp = config::Stamp;

/// The settings, read again when a file changed (looked at once a second).
fn settings() -> EnergyConfig {
    SETTINGS.with(|s| {
        let mut s = s.borrow_mut();
        let stale = s.as_ref().is_none_or(|(_, _, at)| at.elapsed() >= Duration::from_secs(1));
        if stale {
            let dir = Game::native_config_dir().ok();
            let stamp = dir.as_deref().map_or([None; config::FILES.len()], config::stamp);
            if s.as_ref().is_none_or(|(_, old, _)| *old != stamp) {
                let fresh = dir.as_deref().map(EnergyConfig::load).unwrap_or_default();
                *s = Some((fresh, stamp, Instant::now()));
            } else if let Some((_, _, at)) = s.as_mut() {
                *at = Instant::now();
            }
        }
        s.as_ref().map(|(c, _, _)| c.clone()).unwrap_or_default()
    })
}

/// `native_screen`: `GuiEnergyCube`'s size, its two slots, and the labels it
/// draws itself. A screen opening starts on the cube's page.
pub(super) fn screen(block: &str, screen: &mut Option<NativeScreen>) {
    if screen.is_some() {
        return;
    }
    if let Some(kind) = GeneratorKind::of_block(block) {
        *screen = Some(generator_screen(kind));
        return;
    }
    if let Some(kind) = MachineKind::of_block(block) {
        PAGE.with(|p| p.set(0));
        *screen = Some(machine_screen(kind));
        return;
    }
    if CubeTier::of_block(block).is_none() {
        return;
    }
    PAGE.with(|p| p.set(0));
    *screen = Some(NativeScreen {
        width: 176.0,
        height: 166.0,
        slots: vec![Some([17.0, 35.0]), Some([143.0, 35.0]), None, None, None],
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: true,
        own_background: false,
    });
}

// ---- Layout -----------------------------------------------------------------

type Rect = [f32; 4];

fn inside(r: Rect, at: [f32; 2]) -> bool {
    at[0] >= r[0] && at[0] < r[0] + r[2] && at[1] >= r[1] && at[1] < r[1] + r[3]
}

const CONFIG_TAB: Rect = [-26.0, 6.0, 26.0, 26.0];
const CONFIG_BUTTON: Rect = [-21.0, 10.0, 18.0, 18.0];
const ENERGY_TAB: Rect = [-26.0, 137.0, 26.0, 26.0];
const GAUGE: Rect = [55.0, 18.0, 66.0, 50.0];
/// The side configuration page.
const WINDOW: Rect = [36.0, 2.0, 104.0, 81.0];
const CLOSE: Rect = [40.0, 6.0, 8.0, 8.0];
const EJECT_SCREEN: Rect = [40.0, 17.0, 78.0, 12.0];
const EJECT: Rect = [122.0, 16.0, 14.0, 14.0];
const CLEAR: Rect = [122.0, 65.0, 14.0, 14.0];
const ENERGY_PAGE: Rect = [40.0, 33.0, 18.0, 18.0];
const ITEMS_PAGE: Rect = [40.0, 54.0, 18.0, 18.0];

/// `GuiSideConfiguration`'s cross of side buttons (top; left, front, right;
/// back, bottom), 16 pixels each here.
fn side_button(side: RelativeSide) -> Rect {
    let (col, row) = match side {
        RelativeSide::Top => (1.0, 0.0),
        RelativeSide::Left => (0.0, 1.0),
        RelativeSide::Front => (1.0, 1.0),
        RelativeSide::Right => (2.0, 1.0),
        RelativeSide::Back => (0.0, 2.0),
        RelativeSide::Bottom => (1.0, 2.0),
    };
    [70.0 + 17.0 * col, 31.0 + 17.0 * row, 16.0, 16.0]
}

fn kind_of(page: u8) -> Kind {
    if page == 2 { Kind::Items } else { Kind::Energy }
}

/// What is under `at` on `page`: the button it presses, and its tooltip.
enum Hit {
    Press(Button),
    Side(Kind, RelativeSide),
    Tip(Vec<String>),
}

fn hit(page: u8, view: &View, settings: &EnergyConfig, at: [f32; 2], right: bool) -> Option<Hit> {
    if page != 0 {
        let kind = kind_of(page);
        for side in RelativeSide::ALL {
            if inside(side_button(side), at) {
                return Some(Hit::Side(kind, side));
            }
        }
        let tip = |s: &str| Some(Hit::Tip(vec![s.to_owned()]));
        if inside(CLOSE, at) {
            return Some(Hit::Press(Button::Page(0)));
        }
        if inside(EJECT, at) {
            return if right { tip("Auto-eject") } else { Some(Hit::Press(Button::Eject(kind))) };
        }
        if inside(CLEAR, at) {
            return Some(Hit::Press(Button::Clear { kind, forward: !right }));
        }
        if inside(ENERGY_PAGE, at) {
            return Some(Hit::Press(Button::Page(1)));
        }
        if inside(ITEMS_PAGE, at) {
            return Some(Hit::Press(Button::Page(2)));
        }
        if inside(WINDOW, at) {
            return None;
        }
    }
    if inside(CONFIG_BUTTON, at) {
        return Some(Hit::Press(Button::Page(if page == 0 { 1 } else { 0 })));
    }
    if inside(ENERGY_TAB, at) {
        return Some(Hit::Tip(vec![
            format!("Input: {}/t", display::energy(view.input, settings)),
            format!("Max Output: {}/t", display::energy(view.output, settings)),
        ]));
    }
    if page == 0 && inside(GAUGE, at) {
        return Some(Hit::Tip(vec![display::fraction(view.stored, view.capacity, settings)]));
    }
    None
}

fn view_of(data: &[i32]) -> View {
    View::unpack(data).unwrap_or(View {
        stored: 0,
        capacity: 0,
        input: 0,
        output: 0,
        energy: [DataType::None; 6],
        eject: false,
        items: [DataType::None; 6],
        items_eject: false,
    })
}

// ---- Clicks and tooltips ----------------------------------------------------

/// `native_screen_click`.
pub(super) fn click(block: &str, data: &[i32], at: [f32; 2], right: bool, button: &mut Option<i32>) {
    if button.is_some() {
        return;
    }
    if let Some(kind) = MachineKind::of_block(block) {
        machine_click(kind, data, at, right, button);
        return;
    }
    if CubeTier::of_block(block).is_none() {
        return;
    }
    let page = PAGE.with(Cell::get);
    let pressed = match hit(page, &view_of(data), &settings(), at, right) {
        Some(Hit::Press(b)) => b,
        Some(Hit::Side(kind, side)) => Button::Side { kind, side: side.index(), forward: !right },
        _ => return,
    };
    if let Button::Page(p) = pressed {
        PAGE.with(|c| c.set(p));
    }
    *button = Some(pressed.id());
}

/// A side's type in its `EnumColor`, as near as legacy colour codes come
/// (Mekanism's dark red and blue are lighter than `§4` and `§1`).
fn type_name(t: DataType) -> &'static str {
    match t {
        DataType::Input => "§cInput",
        DataType::Output => "§9Output",
        DataType::InputOutput => "§5Input/Output",
        DataType::Energy => "§2Energy",
        DataType::Extra => "§eExtra",
        _ => "§7None",
    }
}

fn side_name(side: RelativeSide) -> &'static str {
    match side {
        RelativeSide::Front => "Front",
        RelativeSide::Left => "Left",
        RelativeSide::Right => "Right",
        RelativeSide::Back => "Back",
        RelativeSide::Top => "Top",
        RelativeSide::Bottom => "Bottom",
    }
}

/// `native_screen_tooltip`.
pub(super) fn tooltip(block: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    if let Some(kind) = GeneratorKind::of_block(block) {
        lines.extend(generator_tip(kind, &generator_view(data), &settings(), at));
        return;
    }
    if let Some(kind) = MachineKind::of_block(block) {
        lines.extend(machine_tip(kind, &MachineView::unpack(data).unwrap_or_default(), &settings(), at));
        return;
    }
    if CubeTier::of_block(block).is_none() {
        return;
    }
    let view = view_of(data);
    let page = PAGE.with(Cell::get);
    if inside(CONFIG_BUTTON, at) {
        lines.push("Side Configuration".into());
        return;
    }
    match hit(page, &view, &settings(), at, false) {
        Some(Hit::Tip(tip)) => lines.extend(tip),
        Some(Hit::Side(kind, side)) => {
            let sides = if kind == Kind::Items { view.items } else { view.energy };
            lines.push(side_name(side).into());
            lines.push(type_name(sides[side.index()]).into());
        }
        Some(Hit::Press(b)) => lines.push(
            match b {
                Button::Page(0) => "Close",
                Button::Page(1) if page == 0 => "Side Configuration",
                Button::Page(1) => "Energy",
                Button::Page(_) => "Items",
                Button::Eject(_) => "Auto-eject",
                Button::Clear { .. } => "Clear Sides",
                Button::Side { .. } => "",
            }
            .into(),
        ),
        None => {}
    }
}

// ---- Drawing ----------------------------------------------------------------

struct Pen<'a, 'g> {
    game: &'a Game<'g>,
    batch: &'a mut HudBatch,
    origin: [f32; 2],
    cursor: [f32; 2],
}

impl Pen<'_, '_> {
    fn sprite(&mut self, name: &str, src: Rect, dst: Rect) {
        let [x, y, w, h] = dst;
        let at = [self.origin[0] + x, self.origin[1] + y, w, h];
        self.game.native_sprite(self.batch, &format!("mekanism:gui/{name}"), src, at);
    }

    fn whole(&mut self, name: &str, size: [f32; 2], dst: Rect) {
        self.sprite(name, [0.0, 0.0, size[0], size[1]], dst);
    }

    /// A sprite named in full (`mekanismgenerators:gui/...`); false when the
    /// atlas has none.
    fn sprite_from(&mut self, name: &str, src: Rect, dst: Rect) -> bool {
        let [x, y, w, h] = dst;
        let at = [self.origin[0] + x, self.origin[1] + y, w, h];
        self.game.native_sprite(self.batch, name, src, at)
    }

    /// `blitNineSlicedSized`: the corners as they are, edges and middle
    /// stretched; corners shrink to half of a smaller target.
    fn nine(&mut self, name: &str, src: Rect, side: [f32; 2], dst: Rect) {
        let cw = side[0].min(dst[2] / 2.0).floor();
        let ch = side[1].min(dst[3] / 2.0).floor();
        let cols = [
            (src[0], cw, dst[0], cw),
            (src[0] + side[0], src[2] - 2.0 * side[0], dst[0] + cw, dst[2] - 2.0 * cw),
            (src[0] + src[2] - cw, cw, dst[0] + dst[2] - cw, cw),
        ];
        let rows = [
            (src[1], ch, dst[1], ch),
            (src[1] + side[1], src[3] - 2.0 * side[1], dst[1] + ch, dst[3] - 2.0 * ch),
            (src[1] + src[3] - ch, ch, dst[1] + dst[3] - ch, ch),
        ];
        for (sy, sh, dy, dh) in rows {
            for (sx, sw, dx, dw) in cols {
                if dw > 0.0 && dh > 0.0 && sw > 0.0 && sh > 0.0 {
                    self.sprite(name, [sx, sy, sw, sh], [dx, dy, dw, dh]);
                }
            }
        }
    }

    /// `drawButton`: `button.png`'s inactive, normal or hovered row.
    fn button(&mut self, r: Rect, active: bool) {
        let row = if !active {
            0.0
        } else if inside(r, self.cursor) {
            2.0
        } else {
            1.0
        };
        self.nine("button", [0.0, row * 20.0, 200.0, 20.0], [20.0, 4.0], r);
    }

    fn image_button(&mut self, icon: &str, icon_size: f32, r: Rect, active: bool) {
        self.button(r, active);
        self.whole(icon, [icon_size, icon_size], r);
    }

    fn fill(&mut self, r: Rect, argb: u32) {
        let at = [self.origin[0] + r[0], self.origin[1] + r[1], r[2], r[3]];
        self.game.native_fill(self.batch, at, argb);
    }

    fn text(&mut self, text: &str, at: [f32; 2], rgb: u32) {
        let at = [self.origin[0] + at[0], self.origin[1] + at[1]];
        self.game.native_text(self.batch, text, at, rgb, false);
    }

    fn centered(&mut self, text: &str, left: f32, width: f32, y: f32, rgb: u32) {
        let w = self.game.native_text_width(text);
        self.text(text, [left + ((width - w) / 2.0).floor(), y], rgb);
    }

    /// `GuiSideHolder`: a tab's holder, 26 wide.
    fn holder(&mut self, r: Rect, left: bool) {
        let name = if left { "holder_left" } else { "holder_right" };
        self.nine(name, [0.0, 0.0, 26.0, 9.0], [4.0, 4.0], r);
    }

    fn slot(&mut self, kind: &str, overlay: Option<&str>, x: f32, y: f32) {
        self.whole(&format!("slot/{kind}"), [18.0, 18.0], [x, y, 18.0, 18.0]);
        if let Some(o) = overlay {
            self.whole(&format!("slot/{o}"), [18.0, 18.0], [x, y, 18.0, 18.0]);
        }
    }
}

/// `EnumColor` of a side's type (`DataType.getColor`), lightened as
/// `BasicColorButton` does, laid over the button.
fn side_color(t: DataType) -> u32 {
    match t {
        DataType::Input => 0x80D51A30,
        DataType::Output => 0x804A7FE0,
        DataType::InputOutput => 0x80B576EE,
        DataType::Energy => 0x8040B040,
        DataType::Extra => 0x80FFD24A,
        _ => 0x60E6E6E6,
    }
}

/// `native_screen_draw`.
pub(super) fn draw(game: &Game<'_>, block: &str, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    if let Some(kind) = GeneratorKind::of_block(block) {
        let cursor = game.native_cursor();
        let cursor = [cursor[0] - origin[0], cursor[1] - origin[1]];
        let mut pen = Pen { game, batch, origin, cursor };
        draw_generator(&mut pen, kind, &generator_view(data), &settings());
        return;
    }
    if let Some(kind) = MachineKind::of_block(block) {
        let cursor = game.native_cursor();
        let cursor = [cursor[0] - origin[0], cursor[1] - origin[1]];
        let mut pen = Pen { game, batch, origin, cursor };
        draw_machine(&mut pen, kind, &MachineView::unpack(data).unwrap_or_default(), &settings());
        return;
    }
    let Some(tier) = CubeTier::of_block(block) else {
        return;
    };
    let view = view_of(data);
    let page = PAGE.with(Cell::get);
    let cursor = game.native_cursor();
    let cursor = [cursor[0] - origin[0], cursor[1] - origin[1]];
    let mut pen = Pen { game, batch, origin, cursor };

    // GuiMekanism's background and the tabs beside it.
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [0.0, 0.0, 176.0, 166.0]);
    pen.holder(CONFIG_TAB, true);
    pen.image_button("configuration", 18.0, CONFIG_BUTTON, true);
    let unit = if settings().show_fe { "fe" } else { "j" };
    pen.whole(&format!("tabs/energy_info_{unit}"), [26.0, 26.0], ENERGY_TAB);

    // The wide energy gauge: its frame, the energy from the bottom, the scale.
    pen.nine("gauge/normal", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], GAUGE);
    let inner = GAUGE[3] - 2.0;
    let level = if view.stored == 0 || view.capacity == 0 {
        0.0
    } else if view.stored >= i64::MAX as u64 || tier == CubeTier::Creative {
        inner
    } else {
        (inner * (view.stored as f64 / view.capacity as f64) as f32).floor().max(1.0)
    };
    let mut y = GAUGE[1] + 1.0 + inner;
    let mut left = level;
    while left > 0.0 {
        let h = left.min(16.0);
        y -= h;
        for col in 0..4 {
            let x = GAUGE[0] + 1.0 + 16.0 * col as f32;
            pen.sprite("energy_fill", [0.0, 16.0 - h, 16.0, h], [x, y, 16.0, h]);
        }
        left -= h;
    }
    pen.whole("gauge/wide", [64.0, 48.0], [GAUGE[0] + 1.0, GAUGE[1] + 1.0, 64.0, 48.0]);

    // The slots: discharge (an OUTPUT item side) and charge (INPUT), then the inventory's.
    pen.slot("output", Some("overlay_minus"), 16.0, 34.0);
    pen.slot("input", Some("overlay_plus"), 142.0, 34.0);
    for row in 0..4 {
        let y = if row == 3 { 141.0 } else { 83.0 + 18.0 * row as f32 };
        for col in 0..9 {
            pen.slot("normal", None, 7.0 + 18.0 * col as f32, y);
        }
    }
    let title = game.native_screen_title().unwrap_or_default();
    pen.centered(&title, 0.0, 176.0, 6.0, TITLE);
    pen.text("Inventory", [8.0, 72.0], TITLE);
    if page == 0 {
        return;
    }

    // The side configuration.
    let kind = kind_of(page);
    let (sides, eject) = match kind {
        Kind::Energy => (view.energy, Some(view.eject)),
        Kind::Items | Kind::Chemicals => (view.items, None),
    };
    let [wx, wy, ww, wh] = WINDOW;
    pen.nine("shadow", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [wx - 3.0, wy - 3.0, ww + 6.0, wh + 6.0]);
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], WINDOW);
    pen.image_button("button/close", 12.0, CLOSE, true);
    let name = if kind == Kind::Energy { "Energy Config" } else { "Items Config" };
    pen.centered(name, wx, ww, wy + 4.0, TITLE);
    pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], EJECT_SCREEN);
    let state = match eject {
        Some(on) => format!("Eject: {}", if on { "On" } else { "Off" }),
        None => "Can't Eject".to_owned(),
    };
    pen.centered(&state, EJECT_SCREEN[0], EJECT_SCREEN[2], EJECT_SCREEN[1] + 2.0, SCREEN_TEXT);
    pen.image_button("button/auto_eject", 14.0, EJECT, eject.is_some());
    pen.image_button("button/clear_sides", 14.0, CLEAR, true);
    for (icon, r, on) in [("energy", ENERGY_PAGE, kind == Kind::Energy), ("items", ITEMS_PAGE, kind == Kind::Items)] {
        pen.button(r, !on);
        pen.whole(icon, [18.0, 18.0], r);
    }
    for side in RelativeSide::ALL {
        let r = side_button(side);
        pen.button(r, true);
        pen.fill([r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0], side_color(sides[side.index()]));
    }
}

// ---- Generators ---------------------------------------------------------------

/// `GuiSolarGenerator` and `GuiHeatGenerator`: the energy tab, the vertical
/// power bar on the right, and the energy slot; the solar generator's screen
/// of its energy, production and output with the sun's state beside it; the
/// heat generator's lava gauge, fuel slot and heat tab.
const HEAT_TAB: Rect = [-26.0, 109.0, 26.0, 26.0];
const POWER_BAR: Rect = [164.0, 15.0, 6.0, 54.0];
const SOLAR_SCREEN: Rect = [40.0, 23.0, 96.0, 40.0];
const SUN: Rect = [18.0, 35.0, 16.0, 16.0];
/// Lava's colour where its texture is missing (`mekanism-content` copies it
/// from the game's own when it is given the game's assets).
const LAVA: u32 = 0xFFD4_5A12;

fn generator_view(data: &[i32]) -> GeneratorView {
    GeneratorView::unpack(data).unwrap_or_default()
}

/// `native_screen` for a generator: its slots where Mekanism puts them.
fn generator_screen(kind: GeneratorKind) -> NativeScreen {
    let slots = match kind {
        GeneratorKind::Solar => vec![Some([143.0, 35.0]), None, None, None, None],
        GeneratorKind::Heat => vec![Some([17.0, 35.0]), Some([143.0, 35.0]), None, None, None],
    };
    NativeScreen { width: 176.0, height: 166.0, slots, inventory_y: 84.0, inventory_x: 8.0, own_labels: true, own_background: false }
}

/// The tooltip of what is under `at` on a generator's screen.
fn generator_tip(kind: GeneratorKind, view: &GeneratorView, settings: &EnergyConfig, at: [f32; 2]) -> Vec<String> {
    let energy = |j: u64| display::energy(j, settings);
    if inside(ENERGY_TAB, at) {
        return vec![format!("Producing: {}/t", energy(view.produced)), format!("Max Output: {}/t", energy(view.max_output))];
    }
    if inside(POWER_BAR, at) {
        return vec![display::fraction(view.stored, view.capacity, settings)];
    }
    if kind == GeneratorKind::Heat {
        if inside(HEAT_TAB, at) {
            return vec![
                format!("Temperature: {}", display::temperature(view.temperature)),
                format!("Transferred: {}/t", display::temperature(view.transfer_loss)),
                format!("Dissipated: {}/t", display::temperature(view.environment_loss)),
            ];
        }
        if inside(GAUGE, at) {
            return vec![if view.lava == 0 { "Empty".to_owned() } else { format!("Lava: {} mB", display::thousands(u64::from(view.lava))) }];
        }
    }
    Vec::new()
}

/// `text`, or when it is wider than `width`, `short` (Mekanism scales its
/// screen's text to 0.8 and scrolls what still doesn't fit).
fn fit(game: &Game<'_>, text: String, short: String, width: f32) -> String {
    if game.native_text_width(&text) <= width { text } else { short }
}

/// Draws a generator's screen.
fn draw_generator(pen: &mut Pen<'_, '_>, kind: GeneratorKind, view: &GeneratorView, settings: &EnergyConfig) {
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [0.0, 0.0, 176.0, 166.0]);
    let unit = if settings.show_fe { "fe" } else { "j" };
    pen.whole(&format!("tabs/energy_info_{unit}"), [26.0, 26.0], ENERGY_TAB);
    // GuiVerticalPowerBar: the bar's frame, then the energy from the bottom.
    pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], POWER_BAR);
    let level = if view.capacity == 0 { 0.0 } else { view.stored as f64 / view.capacity as f64 };
    let filled = (level * 52.0) as i32;
    if filled > 0 {
        let h = filled as f32;
        pen.sprite("bar/vertical_power", [0.0, 0.0, 4.0, h], [POWER_BAR[0] + 1.0, POWER_BAR[1] + POWER_BAR[3] - 1.0 - h, 4.0, h]);
    }
    // The energy slot charges items (`EnergyInventorySlot.drain`).
    pen.slot("power", Some("overlay_power"), 142.0, 34.0);
    match kind {
        GeneratorKind::Solar => {
            let [x, y, w, h] = SOLAR_SCREEN;
            pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], SOLAR_SCREEN);
            let width = w - 6.0;
            let energy = |j: u64| display::energy(j, settings);
            let lines = [
                fit(pen.game, display::fraction(view.stored, view.capacity, settings), energy(view.stored), width),
                fit(pen.game, format!("Producing: {}/t", energy(view.produced)), format!("{}/t", energy(view.produced)), width),
                fit(pen.game, format!("Out: {}/t", energy(view.max_output)), format!("{}/t", energy(view.max_output)), width),
            ];
            // Three lines of 9 pixels 2 apart, centred in the screen's height.
            let mut line_y = y + ((h - (11.0 * 3.0 - 2.0)) / 2.0).floor();
            for line in lines {
                pen.text(&line, [x + 3.0, line_y], SCREEN_TEXT);
                line_y += 11.0;
            }
            // GuiStateTexture: the holder, then the sun or no sun.
            pen.sprite_from("mekanismgenerators:gui/state_holder", [0.0, 0.0, 16.0, 16.0], SUN);
            let icon = if view.sees_sun { "sees_sun" } else { "no_sun" };
            pen.sprite_from(&format!("mekanismgenerators:gui/{icon}"), [0.0, 0.0, 12.0, 12.0], [SUN[0] + 2.0, SUN[1] + 2.0, 12.0, 12.0]);
        }
        GeneratorKind::Heat => {
            pen.whole("tabs/heat_info_k", [26.0, 26.0], HEAT_TAB);
            pen.slot("extra", None, 16.0, 34.0);
            // GuiFluidGauge, wide: the frame, lava from the bottom, the scale.
            pen.nine("gauge/normal", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], GAUGE);
            let inner = GAUGE[3] - 2.0;
            if view.lava > 0 && view.tank > 0 {
                let scale = (view.lava as f32 / view.tank as f32 * inner).round().max(1.0);
                let mut y = GAUGE[1] + 1.0 + inner;
                let mut left = scale;
                while left > 0.0 {
                    let h = left.min(16.0);
                    y -= h;
                    for col in 0..4 {
                        let x = GAUGE[0] + 1.0 + 16.0 * col as f32;
                        if !pen.sprite_from("mekanismgenerators:gui/lava_fill", [0.0, 16.0 - h, 16.0, h], [x, y, 16.0, h]) {
                            pen.fill([x, y, 16.0, h], LAVA);
                        }
                    }
                    left -= h;
                }
            }
            pen.whole("gauge/wide", [64.0, 48.0], [GAUGE[0] + 1.0, GAUGE[1] + 1.0, 64.0, 48.0]);
        }
    }
    for row in 0..4 {
        let y = if row == 3 { 141.0 } else { 83.0 + 18.0 * row as f32 };
        for col in 0..9 {
            pen.slot("normal", None, 7.0 + 18.0 * col as f32, y);
        }
    }
    let title = pen.game.native_screen_title().unwrap_or_default();
    pen.centered(&title, 0.0, 176.0, 6.0, TITLE);
    pen.text("Inventory", [8.0, 72.0], TITLE);
}

// ---- Machines ---------------------------------------------------------------

/// `GuiUpgradeWindowTab` (right, top) and the holder of the upgrade
/// component's slots under it.
const UPGRADE_TAB: Rect = [176.0, 6.0, 26.0, 26.0];
const UPGRADE_BUTTON: Rect = [180.0, 10.0, 18.0, 18.0];
const UPGRADE_HOLDER: Rect = [176.0, 34.0, 26.0, 48.0];
/// The upgrade component's input and output slots, where the game puts their items.
const UPGRADE_SLOTS: [[f32; 2]; 2] = [[181.0, 39.0], [181.0, 61.0]];
/// The side configuration and upgrade windows, left of the panel.
const MACHINE_WINDOW: Rect = [-134.0, 2.0, 104.0, 81.0];
/// The infuser's dump button (`GuiDumpButton`).
const DUMP: Rect = [16.0, 59.0, 21.0, 10.0];
/// `GuiVerticalPowerBar` of each: the electric machines' at y 16, the infuser's at 15.
fn machine_power_bar(kind: MachineKind) -> Rect {
    if kind.infuses() { [164.0, 15.0, 6.0, 54.0] } else { [164.0, 16.0, 6.0, 54.0] }
}
/// `GuiProgress`: `BAR` at (86, 38), the infuser's `RIGHT` at (72, 47).
fn machine_progress(kind: MachineKind) -> Rect {
    if kind.infuses() { [72.0, 47.0, 32.0, 8.0] } else { [86.0, 38.0, 25.0, 9.0] }
}
/// `GuiChemicalBar` of the infuser's tank: its frame (the bar inside is 4x52).
const INFUSE_BAR: Rect = [7.0, 15.0, 6.0, 54.0];

/// Where Mekanism puts a machine's slots (the item's top left), then the
/// upgrade component's two.
fn machine_slots(kind: MachineKind) -> Vec<Option<[f32; 2]>> {
    let mut slots: Vec<Option<[f32; 2]>> = if kind.infuses() {
        vec![Some([17.0, 35.0]), Some([51.0, 43.0]), Some([109.0, 43.0]), Some([143.0, 35.0])]
    } else {
        vec![Some([64.0, 17.0]), Some([116.0, 35.0]), Some([64.0, 53.0])]
    };
    slots.extend(UPGRADE_SLOTS.map(Some));
    slots.resize(9, None);
    slots
}

fn machine_screen(kind: MachineKind) -> NativeScreen {
    NativeScreen {
        width: 176.0,
        height: 166.0,
        slots: machine_slots(kind),
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: true,
        own_background: false,
    }
}

/// The side window's kinds: energy and items, and the infuser's chemicals.
fn machine_kinds(view: &MachineView) -> Vec<Kind> {
    let mut kinds = vec![Kind::Energy, Kind::Items];
    if view.chemicals.is_some() {
        kinds.push(Kind::Chemicals);
    }
    kinds
}

fn machine_window(view: &MachineView) -> Rect {
    let [x, y, w, h] = MACHINE_WINDOW;
    [x, y, w, h + if view.chemicals.is_some() { 21.0 } else { 0.0 }]
}

/// A rectangle of the side window, relative to its top left (the cube's
/// `WINDOW` layout, which starts at (36, 2)).
fn at_window(window: Rect, r: Rect) -> Rect {
    [window[0] + r[0], window[1] + r[1], r[2], r[3]]
}

fn side_button_in(window: Rect, side: RelativeSide) -> Rect {
    let r = side_button(side);
    [r[0] - 36.0 + window[0], r[1] - 2.0 + window[1], r[2], r[3]]
}

fn page_kind(page: u8) -> Option<Kind> {
    match page {
        1 => Some(Kind::Energy),
        2 => Some(Kind::Items),
        3 => Some(Kind::Chemicals),
        _ => None,
    }
}

fn kind_page(kind: Kind) -> u8 {
    match kind {
        Kind::Energy => 1,
        Kind::Items => 2,
        Kind::Chemicals => 3,
    }
}

fn sides_of_kind(view: &MachineView, kind: Kind) -> [DataType; 6] {
    match kind {
        Kind::Energy => view.energy,
        Kind::Items => view.items,
        Kind::Chemicals => view.chemicals.unwrap_or([DataType::None; 6]),
    }
}

/// The upgrade window's rows: each upgrade the machine takes.
fn upgrade_row(window: Rect, i: usize) -> Rect {
    [window[0] + 4.0, window[1] + 16.0 + 21.0 * i as f32, window[2] - 8.0, 20.0]
}

fn uninstall_button(window: Rect, i: usize) -> Rect {
    let row = upgrade_row(window, i);
    [row[0] + row[2] - 15.0, row[1] + 4.0, 14.0, 12.0]
}

enum MachineHit {
    Press(MachineButton),
    Tip(Vec<String>),
}

fn machine_hit(kind: MachineKind, page: u8, view: &MachineView, settings: &EnergyConfig, at: [f32; 2], right: bool) -> Option<MachineHit> {
    let press = |b: Button| Some(MachineHit::Press(MachineButton::Config(b)));
    let window = machine_window(view);
    if let Some(side_kind) = page_kind(page) {
        for side in RelativeSide::ALL {
            if inside(side_button_in(window, side), at) {
                return press(Button::Side { kind: side_kind, side: side.index(), forward: !right });
            }
        }
        if inside(at_window(window, [4.0, 4.0, 8.0, 8.0]), at) {
            return press(Button::Page(0));
        }
        if inside(at_window(window, [86.0, 14.0, 14.0, 14.0]), at) && side_kind == Kind::Items {
            return if right { Some(MachineHit::Tip(vec!["Auto-eject".into()])) } else { press(Button::Eject(side_kind)) };
        }
        if inside(at_window(window, [86.0, 63.0, 14.0, 14.0]), at) {
            return press(Button::Clear { kind: side_kind, forward: !right });
        }
        for (i, k) in machine_kinds(view).into_iter().enumerate() {
            if inside(at_window(window, [4.0, 31.0 + 21.0 * i as f32, 18.0, 18.0]), at) {
                return press(Button::Page(kind_page(k)));
            }
        }
        if inside(window, at) {
            return None;
        }
    }
    if page == 4 {
        for (i, upgrade) in machine::MACHINE_UPGRADES.into_iter().enumerate() {
            if inside(uninstall_button(window, i), at) {
                return Some(MachineHit::Press(MachineButton::Remove { upgrade, all: right }));
            }
        }
        if inside(at_window(window, [4.0, 4.0, 8.0, 8.0]), at) {
            return press(Button::Page(0));
        }
        if inside(window, at) {
            return None;
        }
    }
    if inside(CONFIG_BUTTON, at) {
        return press(Button::Page(if page_kind(page).is_some() { 0 } else { 1 }));
    }
    if inside(UPGRADE_BUTTON, at) {
        return press(Button::Page(if page == 4 { 0 } else { 4 }));
    }
    if kind.infuses() && inside(DUMP, at) {
        return Some(MachineHit::Press(MachineButton::Dump));
    }
    let energy = |j: u64| display::energy(j, settings);
    if inside(ENERGY_TAB, at) {
        let using = if view.active { view.per_tick } else { 0 };
        return Some(MachineHit::Tip(vec![
            format!("Using: {}/t", energy(using)),
            format!("Needed: {}", energy(view.capacity.saturating_sub(view.stored))),
        ]));
    }
    if inside(machine_power_bar(kind), at) {
        return Some(MachineHit::Tip(vec![display::fraction(view.stored, view.capacity, settings)]));
    }
    if inside(machine_progress(kind), at) {
        let share = if view.required == 0 { 0.0 } else { f64::from(view.progress) / f64::from(view.required) };
        return Some(MachineHit::Tip(vec![format!("Progress: {}%", (share * 100.0).floor())]));
    }
    if kind.infuses() && inside(INFUSE_BAR, at) {
        let line = match machine::infuse_of_number(view.infuse) {
            _ if view.infused == 0 => "Empty".to_owned(),
            Some((id, _)) => format!("{}: {} mB", infuse_name(id), display::thousands(view.infused)),
            None => format!("{} mB", display::thousands(view.infused)),
        };
        return Some(MachineHit::Tip(vec![line]));
    }
    None
}

/// An infuse type's name (`chemical.mekanism.<name>`).
fn infuse_name(id: &str) -> &'static str {
    match id {
        "mekanism:carbon" => "Carbon",
        "mekanism:redstone" => "Redstone",
        "mekanism:diamond" => "Diamond",
        "mekanism:refined_obsidian" => "Refined Obsidian",
        "mekanism:bio" => "Biomass",
        "mekanism:fungi" => "Fungi",
        "mekanism:gold" => "Gold",
        "mekanism:tin" => "Tin",
        _ => "Chemical",
    }
}

fn upgrade_name(upgrade: Upgrade) -> &'static str {
    match upgrade {
        Upgrade::Speed => "Speed",
        Upgrade::Energy => "Energy",
        Upgrade::Filter => "Filter",
        Upgrade::Chemical => "Chemical",
        Upgrade::Muffling => "Muffling",
        Upgrade::Anchor => "Anchor",
        Upgrade::StoneGenerator => "Stone Generator",
    }
}

/// `UpgradeUtils.getMultScaledInfo`: `Effect: <multiplier ^ (installed/max), to the hundredth>x`.
fn upgrade_effect(upgrade: Upgrade, installed: u32, settings: &EnergyConfig) -> Option<String> {
    if upgrade.max() <= 1 {
        return None;
    }
    let effect = settings.machines.max_upgrade_multiplier.powf(f64::from(installed) / f64::from(upgrade.max()));
    let shown = ((effect * 100.0).round() / 100.0) as f32;
    Some(if shown.fract() == 0.0 { format!("Effect: {shown:.1}x") } else { format!("Effect: {shown}x") })
}

fn machine_click(kind: MachineKind, data: &[i32], at: [f32; 2], right: bool, button: &mut Option<i32>) {
    let view = MachineView::unpack(data).unwrap_or_default();
    let page = PAGE.with(Cell::get);
    let Some(MachineHit::Press(pressed)) = machine_hit(kind, page, &view, &settings(), at, right) else {
        return;
    };
    if let MachineButton::Config(Button::Page(p)) = pressed {
        PAGE.with(|c| c.set(p));
    }
    *button = Some(pressed.id());
}

fn machine_tip(kind: MachineKind, view: &MachineView, settings: &EnergyConfig, at: [f32; 2]) -> Vec<String> {
    let page = PAGE.with(Cell::get);
    if inside(CONFIG_BUTTON, at) {
        return vec!["Side Configuration".into()];
    }
    if inside(UPGRADE_BUTTON, at) {
        return vec!["Upgrades".into()];
    }
    match machine_hit(kind, page, view, settings, at, false) {
        Some(MachineHit::Tip(tip)) => tip,
        Some(MachineHit::Press(MachineButton::Config(Button::Side { kind: k, side, .. }))) => {
            let side = RelativeSide::ALL[side];
            vec![side_name(side).into(), type_name(sides_of_kind(view, k)[side.index()]).into()]
        }
        Some(MachineHit::Press(b)) => vec![
            match b {
                MachineButton::Dump => "Dump",
                MachineButton::Remove { .. } => "Uninstall (right click: all)",
                MachineButton::Config(Button::Page(0)) => "Close",
                MachineButton::Config(Button::Page(1)) => "Energy",
                MachineButton::Config(Button::Page(2)) => "Items",
                MachineButton::Config(Button::Page(3)) => "Chemicals",
                MachineButton::Config(Button::Page(_)) => "Upgrades",
                MachineButton::Config(Button::Eject(_)) => "Auto-eject",
                MachineButton::Config(Button::Clear { .. }) => "Clear Sides",
                MachineButton::Config(Button::Side { .. }) => "",
            }
            .into(),
        ],
        None => Vec::new(),
    }
}

/// `GuiElectricMachine` / `GuiMetallurgicInfuser` and their windows.
fn draw_machine(pen: &mut Pen<'_, '_>, kind: MachineKind, view: &MachineView, settings: &EnergyConfig) {
    let page = PAGE.with(Cell::get);
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [0.0, 0.0, 176.0, 166.0]);
    // The tabs: side configuration (left), energy (left, bottom), upgrades (right).
    pen.holder(CONFIG_TAB, true);
    pen.image_button("configuration", 18.0, CONFIG_BUTTON, true);
    let unit = if settings.show_fe { "fe" } else { "j" };
    pen.whole(&format!("tabs/energy_info_{unit}"), [26.0, 26.0], ENERGY_TAB);
    pen.holder(UPGRADE_TAB, false);
    pen.image_button("upgrade", 18.0, UPGRADE_BUTTON, true);
    pen.holder(UPGRADE_HOLDER, false);
    pen.slot("normal", Some("overlay_upgrade"), UPGRADE_SLOTS[0][0] - 1.0, UPGRADE_SLOTS[0][1] - 1.0);
    pen.slot("normal", None, UPGRADE_SLOTS[1][0] - 1.0, UPGRADE_SLOTS[1][1] - 1.0);
    // The upgrade going in: `ProgressType.INSTALLING`'s arrow down between the two.
    let installing = view.upgrade_ticks as f32 / machine::UPGRADE_TICKS as f32;
    if installing > 0.0 {
        pen.fill([180.0, 57.5, 18.0 * installing.min(1.0), 2.0], 0xFF3CFE9A);
    }

    // The slots: Mekanism's, coloured by what their side type is (`SlotType.get`).
    let s = kind.slots();
    for (i, at) in machine_slots(kind).into_iter().take(s.count - 2).enumerate() {
        let Some([x, y]) = at else { continue };
        let (frame, overlay) = if i == s.output {
            ("output", None)
        } else if i == s.energy {
            ("power", Some("overlay_power"))
        } else if Some(i) == s.infusion {
            ("extra", None)
        } else {
            ("input", None)
        };
        pen.slot(frame, overlay, x - 1.0, y - 1.0);
    }

    // The power bar (`GuiVerticalPowerBar`): the energy from the bottom.
    let bar = machine_power_bar(kind);
    pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], bar);
    let level = if view.capacity == 0 { 0.0 } else { view.stored as f64 / view.capacity as f64 };
    let filled = (level * 52.0) as i32;
    if filled > 0 {
        let h = filled as f32;
        pen.sprite("bar/vertical_power", [0.0, 0.0, 4.0, h], [bar[0] + 1.0, bar[1] + bar[3] - 1.0 - h, 4.0, h]);
    }

    // The progress (`GuiProgress`): the background, then the part done.
    let progress = machine_progress(kind);
    let share = if view.required == 0 { 0.0 } else { (f64::from(view.progress) / f64::from(view.required)).min(1.0) };
    let [px, py, pw, ph] = progress;
    if kind.infuses() {
        pen.sprite("progress/right", [0.0, 0.0, pw, ph], progress);
        let done = (share * f64::from(pw)) as i32 as f32;
        if done > 0.0 {
            pen.sprite("progress/right", [0.0, ph, done, ph], [px, py, done, ph]);
        }
    } else {
        pen.whole("up_arrow", [8.0, 10.0], [68.0, 38.0, 8.0, 10.0]);
        pen.sprite("progress/bar", [0.0, 0.0, pw, ph], progress);
        let done = (share * f64::from(pw - 2.0)) as i32 as f32;
        if done > 0.0 {
            pen.sprite("progress/bar", [1.0, ph, done, ph], [px + 1.0, py, done, ph]);
        }
    }

    if kind.infuses() {
        // The infusion bar (`GuiChemicalBar`): the infuse type's colour from
        // the bottom (Mekanism tints its texture with it), and the dump button.
        pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], INFUSE_BAR);
        let level = (view.infused as f64 / machine::INFUSE_TANK as f64).min(1.0);
        let h = (level * 52.0) as i32 as f32;
        if h > 0.0 {
            let tint = machine::infuse_of_number(view.infuse).map_or(0x808080, |(_, tint)| tint);
            pen.fill([INFUSE_BAR[0] + 1.0, INFUSE_BAR[1] + 53.0 - h, 4.0, h], 0xFF00_0000 | tint);
        }
        pen.sprite("dump", [0.0, 0.0, 21.0, 10.0], DUMP);
    }

    for row in 0..4 {
        let y = if row == 3 { 141.0 } else { 83.0 + 18.0 * row as f32 };
        for col in 0..9 {
            pen.slot("normal", None, 7.0 + 18.0 * col as f32, y);
        }
    }
    let title = pen.game.native_screen_title().unwrap_or_default();
    pen.centered(&title, 0.0, 176.0, 6.0, TITLE);
    pen.text("Inventory", [8.0, 72.0], TITLE);

    let window = machine_window(view);
    let [wx, wy, ww, wh] = window;
    if let Some(side_kind) = page_kind(page) {
        // `GuiSideConfiguration`, as the cube's.
        pen.nine("shadow", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [wx - 3.0, wy - 3.0, ww + 6.0, wh + 6.0]);
        pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], window);
        pen.image_button("button/close", 12.0, at_window(window, [4.0, 4.0, 8.0, 8.0]), true);
        let name = match side_kind {
            Kind::Energy => "Energy Config",
            Kind::Items => "Items Config",
            Kind::Chemicals => "Chemicals Config",
        };
        pen.centered(name, wx, ww, wy + 4.0, TITLE);
        let screen = at_window(window, [4.0, 15.0, 78.0, 12.0]);
        pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], screen);
        let eject = (side_kind == Kind::Items).then_some(view.items_eject);
        let state = match eject {
            Some(on) => format!("Eject: {}", if on { "On" } else { "Off" }),
            None => "Can't Eject".to_owned(),
        };
        pen.centered(&state, screen[0], screen[2], screen[1] + 2.0, SCREEN_TEXT);
        pen.image_button("button/auto_eject", 14.0, at_window(window, [86.0, 14.0, 14.0, 14.0]), eject.is_some());
        pen.image_button("button/clear_sides", 14.0, at_window(window, [86.0, 63.0, 14.0, 14.0]), true);
        for (i, k) in machine_kinds(view).into_iter().enumerate() {
            let r = at_window(window, [4.0, 31.0 + 21.0 * i as f32, 18.0, 18.0]);
            pen.button(r, k != side_kind);
            let icon = match k {
                Kind::Energy => "energy",
                Kind::Items => "items",
                Kind::Chemicals => "chemicals",
            };
            pen.whole(icon, [18.0, 18.0], r);
        }
        let sides = sides_of_kind(view, side_kind);
        for side in RelativeSide::ALL {
            let r = side_button_in(window, side);
            pen.button(r, true);
            pen.fill([r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0], side_color(sides[side.index()]));
        }
    } else if page == 4 {
        // `GuiUpgradeWindow`: each upgrade the machine takes, how many are
        // in, what they do, and its uninstall button.
        pen.nine("shadow", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [wx - 3.0, wy - 3.0, ww + 6.0, wh + 6.0]);
        pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], window);
        pen.image_button("button/close", 12.0, at_window(window, [4.0, 4.0, 8.0, 8.0]), true);
        pen.centered("Upgrades", wx, ww, wy + 4.0, TITLE);
        for (i, upgrade) in machine::MACHINE_UPGRADES.into_iter().enumerate() {
            let row = upgrade_row(window, i);
            pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], row);
            let at = [pen.origin[0] + row[0] + 2.0, pen.origin[1] + row[1] + 2.0];
            pen.game.native_item(pen.batch, &upgrade.item(), at, 16.0);
            let installed = view.upgrades.get(upgrade);
            pen.text(&format!("{} {}/{}", upgrade_name(upgrade), installed, upgrade.max()), [row[0] + 20.0, row[1] + 2.0], SCREEN_TEXT);
            if let Some(effect) = upgrade_effect(upgrade, installed, settings) {
                pen.text(&effect, [row[0] + 20.0, row[1] + 11.0], SCREEN_TEXT);
            }
            let r = uninstall_button(window, i);
            pen.button(r, installed > 0);
            pen.centered("-", r[0], r[2], r[1] + 2.0, 0xE0E0E0);
        }
    }
}

// ---- Items ------------------------------------------------------------------

/// The Joules a cube item holds and its tier's capacity: its custom data's
/// energy; none saved is an empty cube, or a full creative one.
fn item_energy(stack: &ItemStack, settings: &EnergyConfig) -> Option<(u64, u64)> {
    let tier = CubeTier::of_block(&stack.item)?;
    let capacity = settings.cube(tier).0;
    let saved = super::mek_world::item_data(stack).and_then(|d| super::mek_world::saved_energy(&d));
    Some((cube::stored_from(tier, saved, capacity), capacity))
}

/// `native_tooltip`: `ItemBlockEnergyCube.appendHoverText`, under the name:
/// the stored energy (`StorageUtils.addStoredEnergy`) and the tier's capacity.
pub(super) fn item_tooltip(stack: &ItemStack, lines: &mut Vec<(Text, u32)>) {
    let settings = settings();
    let Some((stored, capacity)) = item_energy(stack, &settings) else {
        return;
    };
    let at = lines.len().min(1);
    let stored = display::fraction(stored, capacity, &settings);
    let capacity = display::energy(capacity, &settings);
    lines.insert(at, (Text::legacy(&format!("§aStored energy: §7{stored}")), u32::MAX));
    lines.insert(at + 1, (Text::legacy(&format!("§9Capacity: §7{capacity}")), u32::MAX));
}

/// `native_item_bar`: a single cube's energy bar (`getEnergyBarWidth`,
/// `energyColor`); a stack of several shows none.
pub(super) fn item_bar(stack: &ItemStack, bar: &mut Option<(i32, u32)>) {
    let Some((stored, capacity)) = item_energy(stack, &settings()) else {
        return;
    };
    *bar = (stack.count == 1).then(|| (display::bar_width(stored, capacity), display::ENERGY_COLOR));
}
