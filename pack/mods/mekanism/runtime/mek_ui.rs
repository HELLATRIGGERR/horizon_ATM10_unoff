// The Energy Cube's screen (support module `crate::game::mek_ui` of the
// `mekanism` native bundle), drawn like Mekanism 10.7's `GuiEnergyCube` with
// Mekanism's own GUI textures (`mekanism:gui/...`, which `mekanism-content`
// copies from the carry): its panel, the wide energy gauge, the discharge (-)
// and charge (+) slots, the side configuration and energy tabs, and the side
// configuration (`GuiSideConfiguration`) with its energy and item pages. The
// Solar and Heat Generators' screens (`GuiSolarGenerator`, `GuiHeatGenerator`)
// are drawn the same way, with Mekanism Generators' textures for the sun (the
// Gas-Burning Generator's own elements are `mek_gas`'s), and
// so are the processing machines' (`GuiElectricMachine`,
// `GuiMetallurgicInfuser`, `GuiElectrolyticSeparator`): their slots, progress,
// power bar, the infuser's infusion bar and dump button, the separator's
// fluid and chemical gauges and dump modes, the side configuration with
// energy, item and chemical pages, and the upgrade window. A machine's slots fill its panel,
// so its windows open beside it, on the left, and the upgrade component's
// two slots sit in a holder on the right, under the upgrade tab (Mekanism
// shows them only in its upgrade window). The Combiner's, the Precision
// Sawmill's and the factories' screens (`mek_factory_ui`) share the tabs and
// windows.
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

use super::{Game, mek_chemicals, native_hooks::NativeScreen};
use horizon_client::{hud::HudBatch, items::ItemStack, text::Text};
use horizon_world::fluid::FluidKind;
use mekanism_energy::{
    config::{self, EnergyConfig},
    cube,
    display,
    factory::ProcessKind,
    generator::GeneratorKind,
    machine::{self, MachineKind, Upgrade},
    menu::{Button, GeneratorView, Kind, MachineButton, MachineView, SeparatorView, View},
    separator::{self, GasMode},
    sides::{DataType, RelativeSide},
    tier::CubeTier,
};
use std::{
    cell::{Cell, RefCell},
    time::{Duration, Instant},
};

pub(super) const TITLE: u32 = 0x404040;
pub(super) const SCREEN_TEXT: u32 = 0x3CFE9A;

thread_local! {
    /// 0: the cube; 1: energy side configuration; 2: item side configuration.
    pub(super) static PAGE: Cell<u8> = const { Cell::new(0) };
    /// The settings this side reads (`energyType`, tier sizes), when their
    /// files last changed, and when that was last looked at.
    static SETTINGS: RefCell<Option<(EnergyConfig, Stamp, Instant)>> = const { RefCell::new(None) };
}

type Stamp = config::Stamp;

/// The settings, read again when a file changed (looked at once a second).
pub(super) fn settings() -> EnergyConfig {
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
    if super::mek_tanks::tier_of(block).is_some() {
        *screen = Some(super::mek_tanks::screen());
        return;
    }
    if let Some(kind) = ProcessKind::of_block(block) {
        *screen = Some(super::mek_factory_ui::screen(kind));
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

pub(super) type Rect = [f32; 4];

pub(super) fn inside(r: Rect, at: [f32; 2]) -> bool {
    at[0] >= r[0] && at[0] < r[0] + r[2] && at[1] >= r[1] && at[1] < r[1] + r[3]
}

pub(super) const CONFIG_TAB: Rect = [-26.0, 6.0, 26.0, 26.0];
pub(super) const CONFIG_BUTTON: Rect = [-21.0, 10.0, 18.0, 18.0];
pub(super) const ENERGY_TAB: Rect = [-26.0, 137.0, 26.0, 26.0];
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
pub(super) fn side_button(side: RelativeSide) -> Rect {
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
    if super::mek_tanks::tier_of(block).is_some() {
        super::mek_tanks::click(data, at, right, button);
        return;
    }
    if let Some(kind) = ProcessKind::of_block(block) {
        super::mek_factory_ui::click(kind, data, at, right, button);
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
pub(super) fn type_name(t: DataType) -> &'static str {
    match t {
        DataType::Input => "§cInput",
        DataType::Output => "§9Output",
        DataType::InputOutput => "§5Input/Output",
        DataType::Energy => "§2Energy",
        DataType::Extra => "§eExtra",
        _ => "§7None",
    }
}

pub(super) fn side_name(side: RelativeSide) -> &'static str {
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
    if super::mek_tanks::tier_of(block).is_some() {
        super::mek_tanks::tooltip(data, at, lines);
        return;
    }
    if let Some(kind) = ProcessKind::of_block(block) {
        super::mek_factory_ui::tooltip(kind, data, at, lines);
        return;
    }
    if let Some(kind) = GeneratorKind::of_block(block) {
        if kind == GeneratorKind::GasBurning
            && let Some(tip) = super::mek_gas::tip(data, at)
        {
            lines.extend(tip);
            return;
        }
        lines.extend(generator_tip(kind, &generator_view(data), &settings(), at));
        return;
    }
    if let Some(kind) = MachineKind::of_block(block) {
        lines.extend(machine_tip(kind, data, &settings(), at));
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

pub(super) struct Pen<'a, 'g> {
    pub(super) game: &'a Game<'g>,
    pub(super) batch: &'a mut HudBatch,
    pub(super) origin: [f32; 2],
    pub(super) cursor: [f32; 2],
}

impl Pen<'_, '_> {
    pub(super) fn sprite(&mut self, name: &str, src: Rect, dst: Rect) {
        let [x, y, w, h] = dst;
        let at = [self.origin[0] + x, self.origin[1] + y, w, h];
        self.game.native_sprite(self.batch, &format!("mekanism:gui/{name}"), src, at);
    }

    pub(super) fn whole(&mut self, name: &str, size: [f32; 2], dst: Rect) {
        self.sprite(name, [0.0, 0.0, size[0], size[1]], dst);
    }

    /// A sprite named in full (`mekanismgenerators:gui/...`); false when the
    /// atlas has none.
    pub(super) fn sprite_from(&mut self, name: &str, src: Rect, dst: Rect) -> bool {
        let [x, y, w, h] = dst;
        let at = [self.origin[0] + x, self.origin[1] + y, w, h];
        self.game.native_sprite(self.batch, name, src, at)
    }

    /// `blitNineSlicedSized`: the corners as they are, edges and middle
    /// stretched; corners shrink to half of a smaller target.
    pub(super) fn nine(&mut self, name: &str, src: Rect, side: [f32; 2], dst: Rect) {
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
    pub(super) fn button(&mut self, r: Rect, active: bool) {
        let row = if !active {
            0.0
        } else if inside(r, self.cursor) {
            2.0
        } else {
            1.0
        };
        self.nine("button", [0.0, row * 20.0, 200.0, 20.0], [20.0, 4.0], r);
    }

    pub(super) fn image_button(&mut self, icon: &str, icon_size: f32, r: Rect, active: bool) {
        self.button(r, active);
        self.whole(icon, [icon_size, icon_size], r);
    }

    pub(super) fn fill(&mut self, r: Rect, argb: u32) {
        let at = [self.origin[0] + r[0], self.origin[1] + r[1], r[2], r[3]];
        self.game.native_fill(self.batch, at, argb);
    }

    pub(super) fn text(&mut self, text: &str, at: [f32; 2], rgb: u32) {
        let at = [self.origin[0] + at[0], self.origin[1] + at[1]];
        self.game.native_text(self.batch, text, at, rgb, false);
    }

    pub(super) fn centered(&mut self, text: &str, left: f32, width: f32, y: f32, rgb: u32) {
        let w = self.game.native_text_width(text);
        self.text(text, [left + ((width - w) / 2.0).floor(), y], rgb);
    }

    /// `GuiSideHolder`: a tab's holder, 26 wide.
    pub(super) fn holder(&mut self, r: Rect, left: bool) {
        let name = if left { "holder_left" } else { "holder_right" };
        self.nine(name, [0.0, 0.0, 26.0, 9.0], [4.0, 4.0], r);
    }

    pub(super) fn slot(&mut self, kind: &str, overlay: Option<&str>, x: f32, y: f32) {
        self.whole(&format!("slot/{kind}"), [18.0, 18.0], [x, y, 18.0, 18.0]);
        if let Some(o) = overlay {
            self.whole(&format!("slot/{o}"), [18.0, 18.0], [x, y, 18.0, 18.0]);
        }
    }
}

/// `EnumColor` of a side's type (`DataType.getColor`), lightened as
/// `BasicColorButton` does, laid over the button.
pub(super) fn side_color(t: DataType) -> u32 {
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
    if super::mek_tanks::tier_of(block).is_some() {
        super::mek_tanks::draw(game, data, batch, origin);
        return;
    }
    if let Some(kind) = ProcessKind::of_block(block) {
        super::mek_factory_ui::draw(game, kind, data, batch, origin);
        return;
    }
    if let Some(kind) = GeneratorKind::of_block(block) {
        let cursor = game.native_cursor();
        let cursor = [cursor[0] - origin[0], cursor[1] - origin[1]];
        let mut pen = Pen { game, batch, origin, cursor };
        draw_generator(&mut pen, kind, &generator_view(data), &settings(), data);
        return;
    }
    if let Some(kind) = MachineKind::of_block(block) {
        let cursor = game.native_cursor();
        let cursor = [cursor[0] - origin[0], cursor[1] - origin[1]];
        let mut pen = Pen { game, batch, origin, cursor };
        draw_machine(&mut pen, kind, data, &settings());
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
        GeneratorKind::Heat | GeneratorKind::GasBurning => vec![Some([17.0, 35.0]), Some([143.0, 35.0]), None, None, None],
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
fn draw_generator(pen: &mut Pen<'_, '_>, kind: GeneratorKind, view: &GeneratorView, settings: &EnergyConfig, data: &[i32]) {
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
        GeneratorKind::GasBurning => super::mek_gas::draw(pen, data),
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
/// `GuiVerticalPowerBar` of each: the electric machines' at y 16, the infuser's
/// and the separator's at 15.
fn machine_power_bar(kind: MachineKind) -> Rect {
    if kind.oxidizes() || kind.infuses_chemicals() {
        super::mek_chem_machines::POWER_BAR
    } else if kind.infuses() || kind.separates() {
        [164.0, 15.0, 6.0, 54.0]
    } else {
        [164.0, 16.0, 6.0, 54.0]
    }
}
/// `GuiProgress`: `BAR` at (86, 38), the infuser's `RIGHT` at (72, 47); the
/// separator has none that fills (`BI`, drawn as its arrows).
fn machine_progress(kind: MachineKind) -> Rect {
    if kind.separates() || kind.infuses_chemicals() {
        [0.0, 0.0, 0.0, 0.0]
    } else if kind.oxidizes() {
        [64.0, 40.0, 48.0, 8.0]
    } else if kind.infuses() {
        [72.0, 47.0, 32.0, 8.0]
    } else {
        [86.0, 38.0, 25.0, 9.0]
    }
}
/// The separator's gauges (`GuiFluidGauge` STANDARD at (5, 10), the left and
/// right `GuiChemicalGauge` SMALL at (58, 18) and (100, 18)) and its dump mode
/// buttons (`GuiGasMode` at (7, 72) and (159, 72)).
const FLUID_GAUGE: Rect = [5.0, 10.0, 18.0, 60.0];
const LEFT_GAUGE: Rect = [58.0, 18.0, 18.0, 30.0];
const RIGHT_GAUGE: Rect = [100.0, 18.0, 18.0, 30.0];
const LEFT_MODE: Rect = [7.0, 72.0, 10.0, 10.0];
const RIGHT_MODE: Rect = [159.0, 72.0, 10.0, 10.0];
/// The infuser's tank, in mB (`MAX_INFUSE`).
const INFUSE_TANK: f64 = 1000.0;
/// `GuiChemicalBar` of the infuser's tank: its frame (the bar inside is 4x52).
const INFUSE_BAR: Rect = [7.0, 15.0, 6.0, 54.0];

/// Where Mekanism puts a machine's slots (the item's top left), then the
/// upgrade component's two.
fn machine_slots(kind: MachineKind) -> Vec<Option<[f32; 2]>> {
    let mut slots: Vec<Option<[f32; 2]>> = if kind.oxidizes() || kind.infuses_chemicals() {
        super::mek_chem_machines::slots(kind)
    } else if kind.infuses() {
        vec![Some([17.0, 35.0]), Some([51.0, 43.0]), Some([109.0, 43.0]), Some([143.0, 35.0])]
    } else if kind.separates() {
        vec![Some([26.0, 35.0]), Some([59.0, 52.0]), Some([101.0, 52.0]), Some([143.0, 35.0])]
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
pub(super) fn machine_kinds(view: &MachineView) -> Vec<Kind> {
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

pub(super) fn sides_of_kind(view: &MachineView, kind: Kind) -> [DataType; 6] {
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

pub(super) enum MachineHit {
    Press(MachineButton),
    Tip(Vec<String>),
}

fn machine_hit(kind: MachineKind, page: u8, view: &MachineView, settings: &EnergyConfig, at: [f32; 2], right: bool) -> Option<MachineHit> {
    if let Some(hit) = window_hit(page, view, kind.upgrades(), settings, at, right, 176.0) {
        return hit;
    }
    if kind.infuses() && inside(DUMP, at) {
        return Some(MachineHit::Press(MachineButton::Dump));
    }
    if inside(machine_power_bar(kind), at) {
        return Some(MachineHit::Tip(vec![display::fraction(view.stored, view.capacity, settings)]));
    }
    if kind.progresses() && inside(machine_progress(kind), at) {
        let share = if view.required == 0 { 0.0 } else { f64::from(view.progress) / f64::from(view.required) };
        return Some(MachineHit::Tip(vec![format!("Progress: {}%", (share * 100.0).floor())]));
    }
    if kind.infuses() && inside(INFUSE_BAR, at) {
        return Some(MachineHit::Tip(vec![chemical_line(view.infuse, view.infused)]));
    }
    None
}

/// What `at` hits of a machine screen's windows and tabs (`width` its
/// panel's width, the upgrade tab beside it): the side configuration and
/// upgrade windows, their tab buttons and the energy tab. `Some(None)` is a
/// window's empty part; none, nothing of these.
pub(super) fn window_hit(page: u8, view: &MachineView, upgrades: &[Upgrade], settings: &EnergyConfig, at: [f32; 2], right: bool, width: f32) -> Option<Option<MachineHit>> {
    let press = |b: Button| Some(MachineHit::Press(MachineButton::Config(b)));
    let window = machine_window(view);
    if let Some(side_kind) = page_kind(page) {
        for side in RelativeSide::ALL {
            if inside(side_button_in(window, side), at) {
                return Some(press(Button::Side { kind: side_kind, side: side.index(), forward: !right }));
            }
        }
        if inside(at_window(window, [4.0, 4.0, 8.0, 8.0]), at) {
            return Some(press(Button::Page(0)));
        }
        if inside(at_window(window, [86.0, 14.0, 14.0, 14.0]), at) && side_kind == Kind::Items {
            return Some(if right { Some(MachineHit::Tip(vec!["Auto-eject".into()])) } else { press(Button::Eject(side_kind)) });
        }
        if inside(at_window(window, [86.0, 63.0, 14.0, 14.0]), at) {
            return Some(press(Button::Clear { kind: side_kind, forward: !right }));
        }
        for (i, k) in machine_kinds(view).into_iter().enumerate() {
            if inside(at_window(window, [4.0, 31.0 + 21.0 * i as f32, 18.0, 18.0]), at) {
                return Some(press(Button::Page(kind_page(k))));
            }
        }
        if inside(window, at) {
            return Some(None);
        }
    }
    if page == 4 {
        for (i, upgrade) in upgrades.iter().copied().enumerate() {
            if inside(uninstall_button(window, i), at) {
                return Some(Some(MachineHit::Press(MachineButton::Remove { upgrade, all: right })));
            }
        }
        if inside(at_window(window, [4.0, 4.0, 8.0, 8.0]), at) {
            return Some(press(Button::Page(0)));
        }
        if inside(window, at) {
            return Some(None);
        }
    }
    if inside(CONFIG_BUTTON, at) {
        return Some(press(Button::Page(if page_kind(page).is_some() { 0 } else { 1 })));
    }
    if inside(beside_panel(UPGRADE_BUTTON, width), at) {
        return Some(press(Button::Page(if page == 4 { 0 } else { 4 })));
    }
    let energy = |j: u64| display::energy(j, settings);
    if inside(ENERGY_TAB, at) {
        let using = if view.active { view.per_tick } else { 0 };
        return Some(Some(MachineHit::Tip(vec![
            format!("Using: {}/t", energy(using)),
            format!("Needed: {}", energy(view.capacity.saturating_sub(view.stored))),
        ])));
    }
    None
}

/// A rectangle of the upgrade tab, which Mekanism puts right of the panel,
/// for a panel `width` wide (176 but for the ultimate factory's 210).
pub(super) fn beside_panel(r: Rect, width: f32) -> Rect {
    [r[0] + width - 176.0, r[1], r[2], r[3]]
}

/// A chemical tank's tooltip line: its chemical's name and mB, or "Empty".
fn chemical_line(number: i32, amount: u64) -> String {
    if amount == 0 {
        return "Empty".to_owned();
    }
    match mek_chemicals::loaded().as_deref().and_then(|r| r.by_number(number).map(mek_chemicals::name)) {
        Some(name) => format!("{name}: {} mB", display::thousands(amount)),
        None => format!("{} mB", display::thousands(amount)),
    }
}

/// A chemical's bar colour (`getColorRepresentation`), grey when unknown.
fn chemical_color(game: &Game<'_>, number: i32) -> u32 {
    mek_chemicals::registry(game).by_number(number).map_or(0x808080, |c| c.color & 0xFF_FFFF)
}

/// The separator's own hits: its gauges' tooltips and its dump mode buttons.
fn separator_hit(extra: &SeparatorView, at: [f32; 2]) -> Option<MachineHit> {
    let mode = |m: i32| match GasMode::by_id(m) {
        GasMode::Idle => "Idle",
        GasMode::DumpingExcess => "Dumping Excess",
        GasMode::Dumping => "Dumping",
    };
    if inside(LEFT_MODE, at) {
        return Some(MachineHit::Press(MachineButton::NextMode(0)));
    }
    if inside(RIGHT_MODE, at) {
        return Some(MachineHit::Press(MachineButton::NextMode(1)));
    }
    if inside(FLUID_GAUGE, at) {
        let line = if extra.fluid_amount == 0 {
            "Empty".to_owned()
        } else {
            let name = FluidKind::from_id(u32::from(extra.fluid)).map_or("Fluid", |f| if f == FluidKind::Water { "Water" } else { f.name() });
            format!("{name}: {} mB", display::thousands(u64::from(extra.fluid_amount)))
        };
        return Some(MachineHit::Tip(vec![line]));
    }
    if inside(LEFT_GAUGE, at) {
        return Some(MachineHit::Tip(vec![chemical_line(extra.left, extra.left_amount), format!("Mode: {}", mode(extra.dump_left))]));
    }
    if inside(RIGHT_GAUGE, at) {
        return Some(MachineHit::Tip(vec![chemical_line(extra.right, extra.right_amount), format!("Mode: {}", mode(extra.dump_right))]));
    }
    None
}

pub(super) fn upgrade_name(upgrade: Upgrade) -> &'static str {
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
    let own = if page == 0 && kind.separates() { SeparatorView::unpack(data).and_then(|extra| separator_hit(&extra, at)) } else { None };
    let Some(MachineHit::Press(pressed)) = own.or_else(|| machine_hit(kind, page, &view, &settings(), at, right)) else {
        return;
    };
    if let MachineButton::Config(Button::Page(p)) = pressed {
        PAGE.with(|c| c.set(p));
    }
    *button = Some(pressed.id());
}

fn machine_tip(kind: MachineKind, data: &[i32], settings: &EnergyConfig, at: [f32; 2]) -> Vec<String> {
    let page = PAGE.with(Cell::get);
    let view = &MachineView::unpack(data).unwrap_or_default();
    if page == 0
        && (kind.oxidizes() || kind.infuses_chemicals())
        && let Some(tip) = super::mek_chem_machines::tip(kind, view, data, at)
    {
        return tip;
    }
    if page == 0
        && kind.separates()
        && let Some(extra) = SeparatorView::unpack(data)
    {
        match separator_hit(&extra, at) {
            Some(MachineHit::Tip(tip)) => return tip,
            Some(MachineHit::Press(_)) => return vec!["Dump mode (click: next)".into()],
            None => {}
        }
    }
    if inside(CONFIG_BUTTON, at) {
        return vec!["Side Configuration".into()];
    }
    if inside(UPGRADE_BUTTON, at) {
        return vec!["Upgrades".into()];
    }
    hit_lines(view, machine_hit(kind, page, view, settings, at, false))
}

/// The tooltip of what a machine screen's hit test found.
pub(super) fn hit_lines(view: &MachineView, hit: Option<MachineHit>) -> Vec<String> {
    match hit {
        Some(MachineHit::Tip(tip)) => tip,
        Some(MachineHit::Press(MachineButton::Config(Button::Side { kind: k, side, .. }))) => {
            let side = RelativeSide::ALL[side];
            vec![side_name(side).into(), type_name(sides_of_kind(view, k)[side.index()]).into()]
        }
        Some(MachineHit::Press(b)) => vec![
            match b {
                MachineButton::Dump => "Dump",
                MachineButton::NextMode(_) => "Dump mode",
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

/// A gauge (`GuiGauge`): its frame, what it holds from the bottom in `rgb`, and
/// its scale over it.
fn gauge(pen: &mut Pen<'_, '_>, r: Rect, level: f64, rgb: u32, scale: &str) {
    pen.nine("gauge/normal", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], r);
    let inner = r[3] - 2.0;
    let h = (level.clamp(0.0, 1.0) * f64::from(inner)) as i32 as f32;
    if h > 0.0 {
        pen.fill([r[0] + 1.0, r[1] + 1.0 + inner - h, r[2] - 2.0, h], 0xFF00_0000 | rgb);
    }
    pen.whole(scale, [16.0, r[3] - 2.0], [r[0] + 1.0, r[1] + 1.0, 16.0, r[3] - 2.0]);
}

/// The fluid's colour in a gauge: water's, else its type's tint.
fn fluid_color(fluid: u16) -> u32 {
    match FluidKind::from_id(u32::from(fluid)) {
        Some(FluidKind::Water) | Some(FluidKind::FlowingWater) | None => 0x3F76E4,
        Some(f) => horizon_world::mod_fluids::mod_fluids().iter().find(|m| m.id == f.name()).map_or(0x808080, |m| m.tint & 0xFF_FFFF),
    }
}

/// `GuiElectrolyticSeparator`'s own elements: the fluid gauge, the two
/// chemical gauges and the dump mode buttons.
fn draw_separator(pen: &mut Pen<'_, '_>, extra: &SeparatorView) {
    gauge(pen, FLUID_GAUGE, f64::from(extra.fluid_amount) / f64::from(separator::MAX_FLUID), fluid_color(extra.fluid), "gauge/standard");
    let max = separator::MAX_GAS as f64;
    let left = chemical_color(pen.game, extra.left);
    let right = chemical_color(pen.game, extra.right);
    gauge(pen, LEFT_GAUGE, extra.left_amount as f64 / max, left, "gauge/small");
    gauge(pen, RIGHT_GAUGE, extra.right_amount as f64 / max, right, "gauge/small");
    for (r, mode) in [(LEFT_MODE, extra.dump_left), (RIGHT_MODE, extra.dump_right)] {
        let name = match GasMode::by_id(mode) {
            GasMode::Idle => "gas_mode_idle",
            GasMode::DumpingExcess => "gas_mode_excess",
            GasMode::Dumping => "gas_mode_dump",
        };
        pen.whole(name, [10.0, 10.0], r);
    }
}

/// `GuiElectricMachine` / `GuiMetallurgicInfuser` / `GuiElectrolyticSeparator`
/// and their windows.
fn draw_machine(pen: &mut Pen<'_, '_>, kind: MachineKind, data: &[i32], settings: &EnergyConfig) {
    let view = &MachineView::unpack(data).unwrap_or_default();
    let page = PAGE.with(Cell::get);
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [0.0, 0.0, 176.0, 166.0]);
    draw_tabs(pen, view, settings, 176.0);

    // The slots: Mekanism's, coloured by what their side type is (`SlotType.get`).
    let s = kind.slots();
    for (i, at) in machine_slots(kind).into_iter().take(s.count - 2).enumerate() {
        let Some([x, y]) = at else { continue };
        let (frame, overlay) = if i == s.output || Some(i) == s.output2 {
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

    if kind.oxidizes() || kind.infuses_chemicals() {
        super::mek_chem_machines::draw(pen, kind, view, data);
    }
    // The power bar (`GuiVerticalPowerBar`): the energy from the bottom.
    let bar = machine_power_bar(kind);
    let vertical = !kind.oxidizes() && !kind.infuses_chemicals();
    if vertical {
        pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], bar);
    }
    let level = if view.capacity == 0 { 0.0 } else { view.stored as f64 / view.capacity as f64 };
    let filled = (level * 52.0) as i32;
    if filled > 0 && vertical {
        let h = filled as f32;
        pen.sprite("bar/vertical_power", [0.0, 0.0, 4.0, h], [bar[0] + 1.0, bar[1] + bar[3] - 1.0 - h, 4.0, h]);
    }

    // The progress (`GuiProgress`): the background, then the part done.
    let progress = machine_progress(kind);
    let share = if view.required == 0 { 0.0 } else { (f64::from(view.progress) / f64::from(view.required)).min(1.0) };
    let [px, py, pw, ph] = progress;
    if !vertical {
        // Drawn with their gauges (`mek_chem_machines::draw`).
    } else if kind.separates() {
        if let Some(extra) = SeparatorView::unpack(data) {
            draw_separator(pen, &extra);
        }
    } else if kind.infuses() {
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
        let level = (view.infused as f64 / INFUSE_TANK).min(1.0);
        let h = (level * 52.0) as i32 as f32;
        if h > 0.0 {
            let tint = chemical_color(pen.game, view.infuse);
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

    draw_windows(pen, view, kind.upgrades(), page, settings);
}

/// A machine screen's tabs, its panel `width` wide: side configuration
/// (left), energy (left, bottom), upgrades (right) and the holder of the
/// upgrade component's slots under it.
pub(super) fn draw_tabs(pen: &mut Pen<'_, '_>, view: &MachineView, settings: &EnergyConfig, width: f32) {
    pen.holder(CONFIG_TAB, true);
    pen.image_button("configuration", 18.0, CONFIG_BUTTON, true);
    let unit = if settings.show_fe { "fe" } else { "j" };
    pen.whole(&format!("tabs/energy_info_{unit}"), [26.0, 26.0], ENERGY_TAB);
    let dx = width - 176.0;
    pen.holder(beside_panel(UPGRADE_TAB, width), false);
    pen.image_button("upgrade", 18.0, beside_panel(UPGRADE_BUTTON, width), true);
    pen.holder(beside_panel(UPGRADE_HOLDER, width), false);
    pen.slot("normal", Some("overlay_upgrade"), UPGRADE_SLOTS[0][0] + dx - 1.0, UPGRADE_SLOTS[0][1] - 1.0);
    pen.slot("normal", None, UPGRADE_SLOTS[1][0] + dx - 1.0, UPGRADE_SLOTS[1][1] - 1.0);
    // The upgrade going in: `ProgressType.INSTALLING`'s arrow down between the two.
    let installing = view.upgrade_ticks as f32 / machine::UPGRADE_TICKS as f32;
    if installing > 0.0 {
        pen.fill([180.0 + dx, 57.5, 18.0 * installing.min(1.0), 2.0], 0xFF3CFE9A);
    }
}

/// Where the upgrade component's two slots sit beside a panel `width` wide.
pub(super) fn upgrade_slots(width: f32) -> [[f32; 2]; 2] {
    UPGRADE_SLOTS.map(|[x, y]| [x + width - 176.0, y])
}

/// A machine screen's side configuration and upgrade windows, left of its panel.
pub(super) fn draw_windows(pen: &mut Pen<'_, '_>, view: &MachineView, upgrades: &[Upgrade], page: u8, settings: &EnergyConfig) {
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
        for (i, upgrade) in upgrades.iter().copied().enumerate() {
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
    if super::mek_tanks::item_tooltip(stack, lines) {
        return;
    }
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
    if super::mek_tanks::item_bar(stack, bar) {
        return;
    }
    let Some((stored, capacity)) = item_energy(stack, &settings()) else {
        return;
    };
    *bar = (stack.count == 1).then(|| (display::bar_width(stored, capacity), display::ENERGY_COLOR));
}
