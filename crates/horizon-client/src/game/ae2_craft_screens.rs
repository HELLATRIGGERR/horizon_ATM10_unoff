//! AE2's crafting screens over the placeholder menus of `ae2_craft_confirm`:
//! `CraftAmountScreen` (`craft_amount.json`), `CraftConfirmScreen`
//! (`craft_confirm.json`), `CraftingCPUScreen` (`crafting_cpu.json`, a CPU's
//! own) and `CraftingStatusScreen` (`crafting_status.json`, a terminal's, with
//! `CPUSelectionList`).
//!
//! The menu keeps its places: a press on a button is a click on the place the
//! server has for it, and `click-slot` still reaches every place as a plain
//! click. What the placeholders do not carry (the plan's or the job's table,
//! the CPUs) comes as `ae2:craft_view`; the table's scrollbars are the
//! screen's own, as in AE2.
use super::{
    Game,
    ae2_screens::{fill, icon, inside},
    ae2_terminal_screen::slot_text,
    ae2_widgets::{Scroll, byte_amount, grouped, toolbar_button, widget},
};
use horizon_client::{
    ae2_screen_style::ScreenStyle,
    hud::{HudBatch, rgba},
    items::ItemStack,
    menu::Input,
};
use serde::{Deserialize, Serialize};

/// Menu places the screens press: the CPU's selection mode, next or start,
/// cancel, the CPU cycler, a terminal's status back tab, and back to the
/// terminal from the amount and plan screens.
const MODE: usize = 300;
const NEXT: usize = 302;
const CANCEL: usize = 303;
const CPU: usize = 304;
const STATUS_BACK: usize = 320;
const BACK: usize = 321;
/// The `button` of a click on `NEXT`: open the plan and start it when it is
/// ready (shift on Next, `setAutoStart`); start it and go back to the terminal
/// (`returnToMainMenu`).
pub(super) const AUTO_START: i32 = 1;
pub(super) const START_AND_BACK: i32 = 2;
/// The first `button` of a click on `CPU` that selects a CPU by its row.
pub(super) const SELECT_CPU: i32 = 10;

/// `NumberEntryWidget`'s buttons from the widget's top left, in entry order:
/// x, y and width (each 20 tall).
const NUMBER_BUTTONS: [(i32, i32, i32); 8] = [
    (0, 0, 22),
    (28, 0, 28),
    (62, 0, 32),
    (100, 0, 38),
    (0, 42, 22),
    (28, 42, 28),
    (62, 42, 32),
    (100, 42, 38),
];
const STEPS_1000: [i64; 4] = [1, 10, 100, 1000];
const STEPS_64: [i64; 4] = [1, 16, 32, 64];
/// `NumberEntryType.of`'s unit for a fluid, and `UNIT_PADDING`.
const FLUID_UNIT: &str = "B";
const UNIT_PADDING: f32 = 3.0;

/// `AbstractTableRenderer`.
const TABLE_AT: (i32, i32) = (9, 19);
const CELL: (i32, i32) = (67, 22);
const CELL_BORDER: i32 = 1;
const COLS: i32 = 3;
const CONFIRM_ROWS: i32 = 5;
const STATUS_ROWS: i32 = 6;
const TEXT_SCALE: f32 = 0.5;
const LINE_HEIGHT: f32 = 9.0 * TEXT_SCALE;
/// `CPUSelectionList`.
const CPU_ROWS: i32 = 6;
const CPU_BUTTON_AT: (i32, i32) = (8, 19);
/// `Icon.S_STORAGE`, `S_PROCESSOR`, `S_CRAFT`, `S_TERMINAL`, `S_MACHINE`: 10 x 10.
const S_STORAGE: (i32, i32) = (208, 224);
const S_PROCESSOR: (i32, i32) = (208, 234);
const S_CRAFT: (i32, i32) = (208, 244);
const S_TERMINAL: (i32, i32) = (192, 224);
const S_MACHINE: (i32, i32) = (192, 234);
/// `Icon.CRAFT_HAMMER`.
const CRAFT_HAMMER: (i32, i32) = (48, 144);

/// A table's entry: its item, and three amounts (in storage, missing and to
/// craft on a plan; in storage, crafting and scheduled on a job).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct Row {
    pub(super) item: String,
    pub(super) fluid: bool,
    pub(super) amounts: [u64; 3],
    #[serde(skip)]
    stack: Option<ItemStack>,
}

impl Row {
    pub(super) fn new(item: String, fluid: bool, amounts: [u64; 3]) -> Self {
        Self { item, fluid, amounts, stack: None }
    }
}

/// A CPU in `CPUSelectionList` (`CraftingCpuListEntry`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct Cpu {
    pub(super) storage: u64,
    pub(super) coprocessors: u32,
    pub(super) mode: String,
    /// What it crafts: amounts[0] is the job's size.
    pub(super) job: Option<Row>,
    pub(super) progress: f32,
    pub(super) elapsed_ms: u64,
}

/// What the screens show that the menu's places do not carry.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct CraftView {
    pub(super) window: i32,
    /// The block or part the screen belongs to, for the help button.
    pub(super) host: String,
    /// The amount screen: what will be crafted.
    pub(super) what: Option<Row>,
    /// The plan's bytes once it is done, or why there is no plan.
    pub(super) bytes: Option<u64>,
    pub(super) error: Option<String>,
    pub(super) simulation: bool,
    pub(super) no_cpu: bool,
    pub(super) cpu_serial: Option<u32>,
    pub(super) cpu_bytes: u64,
    pub(super) cpu_coprocessors: u32,
    pub(super) rows: Vec<Row>,
    /// A CPU's own screen rather than a terminal's status.
    pub(super) direct: bool,
    pub(super) suspended: bool,
    pub(super) mode: String,
    pub(super) cpus: Vec<Cpu>,
    pub(super) selected: Option<usize>,
    /// The selected job's time left (`CraftingCPUScreen`'s title).
    pub(super) eta_ms: u64,
}

impl CraftView {
    /// The stacks' icons from their canonical text, when the view arrives.
    fn parsed(mut self) -> Self {
        let parse = |row: &mut Row| row.stack = horizon_client::item_data::parse_stack(&row.item).ok().flatten();
        self.rows.iter_mut().for_each(parse);
        self.what.iter_mut().for_each(parse);
        self.cpus.iter_mut().filter_map(|cpu| cpu.job.as_mut()).for_each(parse);
        self
    }
}

/// The open crafting screen's own state.
#[derive(Clone, Debug, Default)]
pub(super) struct CraftUi {
    view: Option<CraftView>,
    table: Scroll,
    cpus: Scroll,
}

/// The screen's buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Button {
    Help,
    Number(usize),
    Next,
    Back,
    Start,
    Cancel,
    SelectCpu,
    Suspend,
    CancelJob,
    Mode,
    StatusBack,
}

impl Game<'_> {
    fn ae2_craft_style(&self) -> Option<&'static ScreenStyle> {
        self.ae2_style(match self.ae2_craft_screen() {
            1 => "craft_amount.json",
            2 => "craft_confirm.json",
            3 if self.ae2_craft_ui.view.as_ref().is_some_and(|v| v.direct) => "crafting_cpu.json",
            3 => "crafting_status.json",
            _ => return None,
        })
    }

    fn ae2_craft_direct(&self) -> bool {
        self.ae2_craft_ui.view.as_ref().is_some_and(|v| v.direct)
    }

    fn ae2_craft_table_rows(&self) -> i32 {
        if self.ae2_craft_screen() == 3 { STATUS_ROWS } else { CONFIRM_ROWS }
    }

    /// `getScrollableRows`.
    fn ae2_craft_table_range(&self) -> i32 {
        let entries = self.ae2_craft_ui.view.as_ref().map_or(0, |v| v.rows.len()) as i32;
        ((entries + COLS - 1) / COLS - self.ae2_craft_table_rows()).max(0)
    }

    fn ae2_craft_cpu_range(&self) -> i32 {
        (self.ae2_craft_ui.view.as_ref().map_or(0, |v| v.cpus.len()) as i32 - CPU_ROWS).max(0)
    }

    /// The window's size and where each button is.
    fn ae2_craft_buttons(&self, style: &ScreenStyle) -> Vec<(Button, [i32; 4])> {
        let mut buttons = vec![(Button::Help, toolbar_button(style, 0))];
        let mut add = |button, name: &str, size| {
            if let Some(rect) = widget(style, name, size) {
                buttons.push((button, rect));
            }
        };
        match self.ae2_craft_screen() {
            1 => {
                add(Button::Next, "next", (38, 20));
                add(Button::Back, "back", (22, 22));
                if let Some([x, y, _, _]) = widget(style, "amountToCraft", (138, 62)) {
                    for (i, (bx, by, w)) in NUMBER_BUTTONS.into_iter().enumerate() {
                        buttons.push((Button::Number(i), [x + bx, y + by, w, 20]));
                    }
                }
            }
            2 => {
                add(Button::Start, "start", (50, 20));
                add(Button::Cancel, "cancel", (50, 20));
                add(Button::SelectCpu, "selectCpu", (180, 20));
            }
            _ => {
                add(Button::Suspend, "suspend", (50, 20));
                add(Button::CancelJob, "cancel", (50, 20));
                if self.ae2_craft_direct() {
                    buttons.push((Button::Mode, toolbar_button(style, 1)));
                } else {
                    add_back(style, &mut buttons);
                }
            }
        }
        buttons
    }

    /// Whether the button takes a press now.
    fn ae2_craft_active(&self, button: Button) -> bool {
        let view = self.ae2_craft_ui.view.as_ref();
        let startable = view.is_some_and(|v| v.bytes.is_some() && !v.simulation);
        match button {
            Button::Next => self.ae2_craft_amount().is_some(),
            Button::Start => startable && !view.is_some_and(|v| v.no_cpu),
            Button::SelectCpu => startable,
            Button::Suspend | Button::CancelJob => view.is_some_and(|v| !v.rows.is_empty()),
            _ => true,
        }
    }

    /// The amount in the field, when it is one a job can have.
    fn ae2_craft_amount(&self) -> Option<u64> {
        let text = self.ae2_craft_field()?.value;
        super::ae2_craft_confirm::craft_amount_text(&text, self.ae2_craft_fluid()).map(|(n, _)| n)
    }

    /// The amount field, on the server's menu or the local one.
    fn ae2_craft_field(&self) -> Option<horizon_client::ui::EditBox> {
        match self.remote.as_ref() {
            Some(r) => r.ae2_search.clone(),
            None => self.ae2_local_craft_field(),
        }
    }

    /// The amount field's boxes, for the search field's press.
    pub(super) fn ae2_craft_field_rects(&self) -> Option<([f32; 4], [f32; 4])> {
        if self.ae2_craft_screen() != 1 {
            return None;
        }
        let [x, y, w, h] = widget(self.ae2_craft_style()?, "amountToCraftInput", (66, 12))?;
        let (rect, mut text) = Self::ae2_field_rects([x, y, w, h]);
        if self.ae2_craft_fluid() {
            text[2] -= HudBatch::text_width(&self.gui.atlas, FLUID_UNIT) + UNIT_PADDING;
        }
        Some((rect, text))
    }

    /// Whether the amount is of a fluid, entered in buckets
    /// (`NumberEntryType.of`).
    fn ae2_craft_fluid(&self) -> bool {
        self.ae2_craft_ui.view.as_ref().and_then(|v| v.what.as_ref()).is_some_and(|w| w.fluid)
    }

    fn ae2_craft_button_at(&self, at: [f32; 2]) -> Option<Button> {
        let style = self.ae2_craft_style()?;
        self.ae2_craft_buttons(style).into_iter().find(|(_, rect)| inside(*rect, at)).map(|(b, _)| b)
    }

    /// The menu place a button stands for, where `click-slot` finds it.
    pub(super) fn ae2_craft_slot_position(&self, i: usize) -> (f32, f32) {
        let hidden = (-1000.0, -1000.0);
        let Some(style) = self.ae2_craft_style() else {
            return hidden;
        };
        let button = match (self.ae2_craft_screen(), i) {
            (1, 300) => {
                return style.slot("MACHINE_OUTPUT", 0).map_or(hidden, |(x, y)| (x as f32, y as f32));
            }
            (1, NEXT) => Button::Next,
            (1, BACK) => Button::Back,
            (2, NEXT) => Button::Start,
            (2, BACK) => Button::Cancel,
            (2, CPU) => Button::SelectCpu,
            (3, NEXT) => Button::Suspend,
            (3, CANCEL) => Button::CancelJob,
            (3, MODE) => Button::Mode,
            (3, STATUS_BACK) => Button::StatusBack,
            _ => return hidden,
        };
        self.ae2_craft_buttons(style)
            .into_iter()
            .find(|(b, _)| *b == button)
            .map_or(hidden, |(_, [x, y, _, _])| (x as f32, y as f32))
    }

    /// The menu place under `at`: a slot the plain click path can take.
    pub(super) fn ae2_craft_entry_at(&self, at: [f32; 2]) -> Option<usize> {
        if self.ae2_craft_screen() == 1
            && let Some((x, y)) = self.ae2_craft_style().and_then(|style| style.slot("MACHINE_OUTPUT", 0))
            && inside([x, y, 16, 16], at)
        {
            return Some(300);
        }
        match (self.ae2_craft_screen(), self.ae2_craft_button_at(at)?) {
            (_, Button::Next | Button::Start | Button::Suspend) => Some(NEXT),
            (_, Button::Back | Button::Cancel) => Some(BACK),
            (_, Button::SelectCpu) => Some(CPU),
            (_, Button::CancelJob) => Some(CANCEL),
            (_, Button::Mode) => Some(MODE),
            (_, Button::StatusBack) => Some(STATUS_BACK),
            _ => None,
        }
    }

    /// Whether `at` is off the screen, its toolbar, tabs and the CPU list.
    pub(super) fn ae2_craft_outside(&self, at: [f32; 2]) -> bool {
        let Some(style) = self.ae2_craft_style() else {
            return true;
        };
        let (w, h) = style.size();
        if inside([0, 0, w, h], at) || self.ae2_craft_button_at(at).is_some() {
            return false;
        }
        if widget(style, "selectCpuList", (94, 164)).is_some_and(|r| inside(r, at)) {
            return false;
        }
        let help = toolbar_button(style, 0);
        !inside([help[0] - 4, help[1] - 3, 21, 6 + 22 * if self.ae2_craft_direct() { 2 } else { 1 }], at)
    }

    /// A press the screen takes before the menu. Right presses act as left
    /// ones (`AEBaseScreen.mouseClicked`).
    pub(super) fn ae2_craft_press(&mut self, at: [f32; 2], right: bool) -> bool {
        let Some(style) = self.ae2_craft_style() else {
            return false;
        };
        // The scrollbars and the CPU list.
        if !right {
            if let Some(track) = widget(style, "scrollbar", (12, 137)).filter(|r| inside(*r, at)) {
                let range = self.ae2_craft_table_range();
                self.ae2_craft_ui.table.press(track, range, 1, at[1]);
                return true;
            }
            if let Some(track) = widget(style, "selectCpuScrollbar", (12, 137)).filter(|r| inside(*r, at)) {
                let range = self.ae2_craft_cpu_range();
                self.ae2_craft_ui.cpus.press(track, range, CPU_ROWS / 3, at[1]);
                return true;
            }
        }
        if let Some(index) = self.ae2_craft_cpu_at(style, at) {
            self.audio.play_button_click();
            self.ae2_craft_click(CPU, SELECT_CPU + index as i32);
            return true;
        }
        let Some(button) = self.ae2_craft_button_at(at) else {
            return false;
        };
        if !self.ae2_craft_active(button) {
            return true;
        }
        self.audio.play_button_click();
        match button {
            Button::Help => {
                let host = self.ae2_craft_ui.view.as_ref().map_or_else(|| "ae2:crafting_terminal".to_owned(), |v| v.host.clone());
                self.ae2_open_guide(&host);
            }
            Button::Number(entry) => self.ae2_craft_step(entry),
            Button::Next => self.ae2_craft_click(NEXT, if self.has_shift_down() { AUTO_START } else { 0 }),
            Button::Start => self.ae2_craft_click(NEXT, START_AND_BACK),
            Button::Back | Button::Cancel => self.ae2_craft_click(BACK, 0),
            Button::SelectCpu => self.ae2_craft_click(CPU, i32::from(right)),
            Button::Suspend => self.ae2_craft_click(NEXT, 0),
            Button::CancelJob => self.ae2_craft_click(CANCEL, 0),
            Button::Mode => self.ae2_craft_click(MODE, 0),
            Button::StatusBack => self.ae2_craft_click(STATUS_BACK, 0),
        }
        true
    }

    /// `CPUSelectionList.hitTestCpu`: the row of the CPU under `at`.
    fn ae2_craft_cpu_at(&self, style: &ScreenStyle, at: [f32; 2]) -> Option<usize> {
        let [x, y, _, _] = widget(style, "selectCpuList", (94, 164))?;
        let (bw, bh) = style.images.get("cpuListButton")?.size();
        let (rx, ry) = (at[0] as i32 - x - CPU_BUTTON_AT.0, at[1] as i32 - y - CPU_BUTTON_AT.1);
        if rx < 0 || rx >= bw || ry < 0 || ry % (bh + 1) == bh {
            return None;
        }
        let row = ry / (bh + 1);
        let index = self.ae2_craft_ui.cpus.current + row;
        let count = self.ae2_craft_ui.view.as_ref().map_or(0, |v| v.cpus.len()) as i32;
        (row < CPU_ROWS && index < count).then_some(index as usize)
    }

    fn ae2_craft_click(&mut self, slot: usize, button: i32) {
        if self.remote.is_some() {
            self.ae2_send_menu_button((slot - 300) as i16, button);
        } else {
            self.ae2_local_craft_click(Some(slot), button, Input::Pickup);
        }
    }

    /// A press on one of `NumberEntryWidget`'s buttons.
    fn ae2_craft_step(&mut self, entry: usize) {
        let steps = if self.has_shift_down() || self.has_control_down() { STEPS_64 } else { STEPS_1000 };
        self.ae2_craft_add(steps[entry % 4] * if entry < 4 { 1 } else { -1 });
    }

    /// `NumberEntryWidget.addQty`, in the field's units.
    fn ae2_craft_add(&mut self, delta: i64) {
        let Some(field) = self.ae2_craft_field() else {
            return;
        };
        let unit = if self.ae2_craft_fluid() { 1000 } else { 1 };
        let value = field.value.strip_prefix('=').unwrap_or(&field.value);
        let current = super::ae2_number_entry::number_value(value, unit, i64::MIN, i64::MAX, '.').unwrap_or(0) as f64 / unit as f64;
        let mut next = current + delta as f64;
        let (min, max) = (1.0, (f64::from(i32::MAX) / unit as f64).floor());
        if next < min {
            next = min;
        } else if next > max {
            next = max;
        } else if current == 1.0 && delta > 1 {
            next -= 1.0;
        }
        let text = decimal_six(next);
        self.ae2_craft_set_field(&text);
    }

    fn ae2_craft_set_field(&mut self, text: &str) {
        if let Some(r) = self.remote.as_mut() {
            if let Some(field) = r.ae2_search.as_mut() {
                field.set_value(text);
            }
            self.send(horizon_protocol::packets::play::Serverbound::RenameItem(horizon_protocol::packets::play::RenameItem { name: text.to_owned() }));
        } else {
            self.ae2_local_craft_set_field(text);
        }
    }

    /// The scrollbars' handles follow a drag.
    pub(super) fn ae2_craft_drag(&mut self) {
        let Some(style) = self.ae2_craft_style() else {
            return;
        };
        let Some(oy) = self.gui_scale().map(|b| self.inventory_origin(&b).1) else {
            return;
        };
        let y = self.cursor.1 - oy;
        if let Some(track) = widget(style, "scrollbar", (12, 137)) {
            let range = self.ae2_craft_table_range();
            self.ae2_craft_ui.table.dragged(track, range, y);
        }
        if let Some(track) = widget(style, "selectCpuScrollbar", (12, 137)) {
            let range = self.ae2_craft_cpu_range();
            self.ae2_craft_ui.cpus.dragged(track, range, y);
        }
    }

    pub(super) fn ae2_craft_release(&mut self) {
        self.ae2_craft_ui.table.drag = None;
        self.ae2_craft_ui.cpus.drag = None;
    }

    /// The mouse wheel: the table's scrollbar anywhere on the screen, the CPU
    /// list's over the list (`CPUSelectionList.onMouseWheel`).
    pub(super) fn ae2_craft_wheel(&mut self, delta: f32) -> bool {
        let Some(style) = self.ae2_craft_style() else {
            return false;
        };
        let Some((ox, oy)) = self.gui_scale().map(|b| self.inventory_origin(&b)) else {
            return false;
        };
        let at = [self.cursor.0 - ox, self.cursor.1 - oy];
        if self.ae2_craft_screen() == 1 {
            // NumberEntryWidget.onMouseWheel, over the field with a value in it.
            let over = widget(style, "amountToCraftInput", (66, 12)).is_some_and(|r| inside(r, at));
            let unit = if self.ae2_craft_fluid() { 1000 } else { 1 };
            let valued = self.ae2_craft_field().is_some_and(|f| {
                super::ae2_number_entry::number_value(f.value.strip_prefix('=').unwrap_or(&f.value), unit, i64::MIN, i64::MAX, '.').is_some()
            });
            if over && valued && delta != 0.0 {
                self.ae2_craft_add(if delta < 0.0 { -1 } else { 1 });
            }
            return over;
        }
        if widget(style, "selectCpuList", (94, 164)).is_some_and(|r| inside(r, at)) {
            let range = self.ae2_craft_cpu_range();
            self.ae2_craft_ui.cpus.wheel(range, CPU_ROWS / 3, delta);
        } else {
            let range = self.ae2_craft_table_range();
            self.ae2_craft_ui.table.wheel(range, 1, delta);
        }
        true
    }

    /// `ae2:craft_view` from the server.
    pub(super) fn ae2_accept_craft_view(&mut self, data: &[u8]) {
        let Ok(view) = serde_json::from_slice::<CraftView>(data) else {
            return;
        };
        let window = self.remote.as_ref().and_then(|r| r.menu.as_ref()).map(|m| m.0);
        if window == Some(view.window) {
            self.ae2_craft_set_view(view);
        }
    }

    /// The menu's new view; the scrollbars start over on a new window.
    pub(super) fn ae2_craft_set_view(&mut self, view: CraftView) {
        let view = view.parsed();
        let same = self.ae2_craft_ui.view.as_ref().is_some_and(|v| v.window == view.window);
        let (table, cpus) = if same { (self.ae2_craft_ui.table, self.ae2_craft_ui.cpus) } else { Default::default() };
        self.ae2_craft_ui = CraftUi { view: Some(view), table, cpus };
        let (table_range, cpu_range) = (self.ae2_craft_table_range(), self.ae2_craft_cpu_range());
        self.ae2_craft_ui.table.clamp(table_range);
        self.ae2_craft_ui.cpus.clamp(cpu_range);
    }

    /// The screens, by the style of the one the menu shows.
    pub(super) fn ae2_draw_craft(&self, batch: &mut HudBatch, ox: f32, oy: f32) {
        let Some(style) = self.ae2_craft_style() else {
            return;
        };
        let mouse = [self.cursor.0 - ox, self.cursor.1 - oy];
        let view = self.ae2_craft_ui.view.as_ref();
        self.ae2_draw_background(batch, ox, oy, style);
        if self.ae2_craft_screen() == 3 && !self.ae2_craft_direct() {
            self.ae2_draw_cpu_list(batch, ox, oy, style);
        }
        let toolbar = self.ae2_craft_buttons(style);
        let help = toolbar_button(style, 0);
        let last = if self.ae2_craft_direct() && self.ae2_craft_screen() == 3 { toolbar_button(style, 1) } else { help };
        self.ae2_draw_toolbar(batch, ox, oy, help, last);
        let tr = |key: &str, args: &[String]| fill(&self.translate(key, key), args);
        for (button, rect) in toolbar {
            let active = self.ae2_craft_active(button);
            let shift = self.has_shift_down() || self.has_control_down();
            let label = match button {
                Button::Help => {
                    self.ae2_draw_icon_button(batch, ox, oy, rect, icon::HELP, mouse);
                    continue;
                }
                Button::Mode => {
                    self.ae2_draw_cpu_mode_button(batch, ox, oy, rect, view.map_or("ANY", |v| v.mode.as_str()), mouse);
                    continue;
                }
                Button::Back | Button::StatusBack => {
                    self.ae2_draw_tab(batch, ox, oy, rect, icon::BACK);
                    continue;
                }
                Button::Number(entry) => {
                    let steps = if shift { STEPS_64 } else { STEPS_1000 };
                    format!("{}{}", if entry < 4 { '+' } else { '-' }, steps[entry % 4])
                }
                Button::Next if self.has_shift_down() => tr("gui.ae2.Start", &[]),
                Button::Next => tr("gui.ae2.Next", &[]),
                Button::Start => tr("gui.ae2.Start", &[]),
                Button::Cancel | Button::CancelJob => tr("gui.ae2.Cancel", &[]),
                Button::Suspend if view.is_some_and(|v| v.suspended) => tr("gui.ae2.Resume", &[]),
                Button::Suspend => tr("gui.ae2.Suspend", &[]),
                Button::SelectCpu => self.ae2_craft_cpu_label(),
            };
            self.ae2_draw_button(batch, ox, oy, rect, &label, active, mouse);
        }
        match self.ae2_craft_screen() {
            1 => {
                if let (Some(rect), Some(field)) = (widget(style, "amountToCraftInput", (66, 12)), self.ae2_craft_field()) {
                    let valid = self.ae2_craft_amount().is_some();
                    let text = self.ae2_craft_field_rects().map(|(_, text)| text);
                    self.ae2_draw_field(batch, ox, oy, style, rect, text, field, valid);
                    if self.ae2_craft_fluid() {
                        // NumberEntryWidget.drawBackgroundLayer.
                        let [x, y, w, h] = rect.map(|v| v as f32);
                        let width = HudBatch::text_width(&self.gui.atlas, FLUID_UNIT);
                        let top = oy + y + ((h - 9.0) / 2.0 + 1.0).trunc();
                        batch.text(&self.gui.atlas, FLUID_UNIT, ox + x + w - width, top, rgba(0x55, 0x55, 0x55, 255), false);
                    }
                }
                self.ae2_draw_texts(batch, ox, oy, style, &[]);
            }
            2 => {
                self.ae2_draw_craft_table(batch, ox, oy, style);
                let (title, status) = self.ae2_craft_plan_texts();
                self.ae2_draw_texts(batch, ox, oy, style, &[("dialog_title", title), ("cpu_status", status)]);
            }
            _ => {
                self.ae2_draw_craft_table(batch, ox, oy, style);
                self.ae2_draw_texts(batch, ox, oy, style, &[("dialog_title", self.ae2_craft_status_title())]);
            }
        }
        if let Some(track) = widget(style, "scrollbar", (12, 137)) {
            self.ae2_draw_scroller(batch, ox, oy, track, self.ae2_craft_table_range(), self.ae2_craft_ui.table);
        }
    }

    /// `CraftingCPUScreen.updateBeforeRender`: the title with the time left.
    fn ae2_craft_status_title(&self) -> String {
        let title = self.translate("gui.ae2.CraftingStatus", "Crafting Status");
        match self.ae2_craft_ui.view.as_ref().filter(|v| v.eta_ms > 0 && !v.rows.is_empty()) {
            Some(view) => format!("{title} - {}", eta_text(view.eta_ms)),
            None => title,
        }
    }

    /// `getNextCpuButtonLabel`.
    fn ae2_craft_cpu_label(&self) -> String {
        let tr = |key: &str| self.translate(key, key);
        let view = self.ae2_craft_ui.view.as_ref();
        if view.is_some_and(|v| v.no_cpu) {
            return tr("gui.ae2.NoCraftingCPUs");
        }
        let name = view.and_then(|v| v.cpu_serial).map_or_else(|| tr("gui.ae2.Automatic"), |n| format!("{} #{n}", tr("gui.ae2.CPUs")));
        fill(&tr("gui.ae2.SelectedCraftingCPU"), &[name])
    }

    /// `CraftConfirmScreen.updateBeforeRender`: the dialog's title and the
    /// line about the CPU.
    fn ae2_craft_plan_texts(&self) -> (String, String) {
        let tr = |key: &str, args: &[String]| fill(&self.translate(key, key), args);
        let view = self.ae2_craft_ui.view.as_ref();
        let details = match view {
            Some(CraftView { error: Some(error), .. }) => error.clone(),
            Some(CraftView { bytes: Some(bytes), .. }) => tr("gui.ae2.BytesUsed", &[grouped(*bytes)]),
            _ => tr("gui.ae2.CalculatingWait", &[]),
        };
        let status = match view.filter(|v| v.bytes.is_some()) {
            Some(v) if v.simulation => tr("gui.ae2.PartialPlan", &[]),
            Some(v) if v.cpu_bytes > 0 => tr("gui.ae2.ConfirmCraftCpuStatus", &[v.cpu_bytes.to_string(), v.cpu_coprocessors.to_string()]),
            Some(_) => tr("gui.ae2.ConfirmCraftNoCpu", &[]),
            None => String::new(),
        };
        (tr("gui.ae2.CraftingPlan", &[details]), status)
    }

    /// `AbstractTableRenderer.render`.
    fn ae2_draw_craft_table(&self, batch: &mut HudBatch, ox: f32, oy: f32, style: &ScreenStyle) {
        let Some(view) = self.ae2_craft_ui.view.as_ref() else {
            return;
        };
        let atlas = &self.gui.atlas;
        let status = self.ae2_craft_screen() == 3;
        let color = super::ae2_screens::argb(style.color("DEFAULT_TEXT_COLOR"));
        let scroll = self.ae2_craft_ui.table.current;
        for row in 0..self.ae2_craft_table_rows() {
            for col in 0..COLS {
                let i = ((row + scroll) * COLS + col) as usize;
                let Some(entry) = view.rows.get(i) else {
                    break;
                };
                let cell_x = TABLE_AT.0 + col * (CELL.0 + CELL_BORDER);
                let cell_y = TABLE_AT.1 + row * (CELL.1 + CELL_BORDER);
                let cell = [ox + cell_x as f32, oy + cell_y as f32, CELL.0 as f32, CELL.1 as f32];
                // CraftingStatusTableRenderer: green while crafting, yellow
                // while scheduled (`AEColor`'s black variants at 0x5A).
                if status {
                    let [_, active, pending] = entry.amounts;
                    if active > 0 {
                        batch.fill(atlas, cell[0], cell[1], cell[2], cell[3], rgba(0x2C, 0x4C, 0x2A, 0x5A));
                    } else if pending > 0 {
                        batch.fill(atlas, cell[0], cell[1], cell[2], cell[3], rgba(0x4C, 0x4A, 0x2A, 0x5A));
                    }
                }
                let lines = table_lines(self, entry, status, false);
                let height = lines.len() as f32 * LINE_HEIGHT + (lines.len().saturating_sub(1)) as f32;
                let mut y = (cell_y as f32 + (CELL.1 as f32 - height) / 2.0).round();
                let item_x = cell_x + CELL.0 - 19;
                for line in &lines {
                    let w = HudBatch::text_width(atlas, line);
                    self.ae2_small_text(batch, ox, oy, line, item_x as f32 - 2.0 - w * TEXT_SCALE, y, TEXT_SCALE, color);
                    y += LINE_HEIGHT + 1.0;
                }
                if let Some(stack) = entry.stack.as_ref() {
                    self.draw_item_model(batch, stack, ox + item_x as f32, oy + (cell_y + (CELL.1 - 16) / 2) as f32);
                }
                // CraftConfirmTableRenderer: a red wash over what is missing.
                if !status && entry.amounts[1] > 0 {
                    batch.fill(atlas, cell[0], cell[1], cell[2], cell[3], rgba(0xFF, 0, 0, 0x1A));
                }
            }
        }
    }

    /// `CPUSelectionList.drawBackgroundLayer`: the panel, which overlaps the
    /// screen's left edge, and a button for each CPU in view.
    fn ae2_draw_cpu_list(&self, batch: &mut HudBatch, ox: f32, oy: f32, style: &ScreenStyle) {
        let Some([lx, ly, lw, lh]) = widget(style, "selectCpuList", (94, 164)) else {
            return;
        };
        let (Some(background), Some(normal), Some(selected)) =
            (style.images.get("cpuList"), style.images.get("cpuListButton"), style.images.get("cpuListButtonSelected"))
        else {
            return;
        };
        self.ae2_blit_to(batch, background, background.src.unwrap_or([0, 0, lw, lh]), [ox + lx as f32, oy + ly as f32, lw as f32, lh as f32], u32::MAX);
        let view = self.ae2_craft_ui.view.as_ref();
        let color = super::ae2_screens::argb(style.color("DEFAULT_TEXT_COLOR"));
        let bar = super::ae2_screens::argb(style.color("SELECTION_COLOR"));
        let (bw, bh) = normal.size();
        let (x, mut y) = (lx + CPU_BUTTON_AT.0, ly + CPU_BUTTON_AT.1);
        let first = self.ae2_craft_ui.cpus.current.max(0) as usize;
        let tr = |key: &str| self.translate(key, key);
        let scale = 0.666;
        for (i, cpu) in view.map_or(&[][..], |v| &v.cpus[..]).iter().enumerate().skip(first).take(CPU_ROWS as usize) {
            let is_selected = view.is_some_and(|v| v.selected == Some(i));
            let image = if is_selected { selected } else { normal };
            self.ae2_blit(batch, image, image.src.unwrap_or([0, 0, bw, bh]), ox + x as f32, oy + y as f32);
            let name = format!("{} #{}", tr("gui.ae2.CPUs"), i + 1);
            self.ae2_small_text_at(batch, ox + (x + 3) as f32, oy + (y + 2) as f32, &name, scale, color);
            if let Some(job) = cpu.job.as_ref() {
                self.ae2_small_icon(batch, ox + (x + 2) as f32, oy + (y + 9) as f32, S_CRAFT);
                let amount = slot_text(job.amounts[0], job.fluid);
                self.ae2_small_text_at(batch, ox + (x + 14) as f32, oy + (y + 13) as f32, &amount, scale, color);
                if let Some(stack) = job.stack.as_ref() {
                    self.ae2_small_item(batch, stack, ox + (x + 55) as f32, oy + (y + 9) as f32, scale);
                }
                // A bar along the button's bottom: the job's progress.
                let filled = (cpu.progress * (bw - 1) as f32) as i32;
                let tint = if is_selected { rgba(0x7D, 0xA9, 0xD2, 255) } else { bar };
                batch.fill(&self.gui.atlas, ox + (x + 1) as f32, oy + (y + bh - 3) as f32, filled as f32, 1.0, tint);
            } else {
                self.ae2_small_icon(batch, ox + (x + 27) as f32, oy + (y + 9) as f32, S_STORAGE);
                self.ae2_small_text_at(batch, ox + (x + 39) as f32, oy + (y + 13) as f32, &byte_amount(cpu.storage), scale, color);
                if cpu.coprocessors > 0 {
                    self.ae2_small_icon(batch, ox + (x + 2) as f32, oy + (y + 9) as f32, S_PROCESSOR);
                    self.ae2_small_text_at(batch, ox + (x + 14) as f32, oy + (y + 13) as f32, &cpu.coprocessors.to_string(), scale, color);
                }
                match cpu.mode.as_str() {
                    "PLAYER_ONLY" => self.ae2_small_icon(batch, ox + (x + 55) as f32, oy + (y + 9) as f32, S_TERMINAL),
                    "MACHINE_ONLY" => self.ae2_small_icon(batch, ox + (x + 55) as f32, oy + (y + 9) as f32, S_MACHINE),
                    _ => {}
                }
            }
            y += bh + 1;
        }
        if let Some(track) = widget(style, "selectCpuScrollbar", (12, 137)) {
            self.ae2_draw_scroller(batch, ox, oy, track, self.ae2_craft_cpu_range(), self.ae2_craft_ui.cpus);
        }
    }

    fn ae2_small_icon(&self, batch: &mut HudBatch, x: f32, y: f32, at: (i32, i32)) {
        self.ae2_icon(batch, at, (10, 10), x, y);
    }

    /// Text scaled about `x`, `y`.
    fn ae2_small_text_at(&self, batch: &mut HudBatch, x: f32, y: f32, text: &str, scale: f32, color: u32) {
        let start = batch.vertices.len();
        batch.text(&self.gui.atlas, text, x, y, color, false);
        batch.scale_since(start, x, y, scale, scale);
    }

    /// The CPU's selection mode button (`CPU_SELECTION_MODE`).
    fn ae2_draw_cpu_mode_button(&self, batch: &mut HudBatch, ox: f32, oy: f32, rect: [i32; 4], mode: &str, mouse: [f32; 2]) {
        // A blank backdrop, then an item stands for the value.
        let item = match mode {
            "PLAYER_ONLY" => "ae2:terminal",
            "MACHINE_ONLY" => "ae2:export_bus",
            _ => return self.ae2_draw_icon_button(batch, ox, oy, rect, CRAFT_HAMMER, mouse),
        };
        let lift = if inside(rect, mouse) { 1.0 } else { 0.0 };
        let backdrop = if inside(rect, mouse) { icon::TOOLBAR_BUTTON_BACKGROUND_HOVER } else { icon::TOOLBAR_BUTTON_BACKGROUND };
        self.ae2_icon(batch, backdrop, (18, 20), ox + rect[0] as f32 - 1.0, oy + rect[1] as f32 + lift);
        self.draw_item_model(batch, &ItemStack::new(item, 1), ox + rect[0] as f32, oy + rect[1] as f32 + 1.0 + lift);
    }

    /// The tooltip under the cursor.
    pub(super) fn ae2_craft_tooltip(&self, batch: &mut HudBatch) -> bool {
        let Some(style) = self.ae2_craft_style() else {
            return false;
        };
        let (ox, oy) = self.inventory_origin(batch);
        let at = [self.cursor.0 - ox, self.cursor.1 - oy];
        let tr = |key: &str| self.translate(key, key);
        let mut lines = Vec::new();
        if self.ae2_craft_screen() == 1 && widget(style, "amountToCraftInput", (66, 12)).is_some_and(|r| inside(r, at)) {
            lines = self.ae2_craft_field_tooltip();
        } else if let Some(index) = self.ae2_craft_cpu_at(style, at) {
            lines = self.ae2_cpu_tooltip(index);
        } else if let Some(button) = self.ae2_craft_button_at(at) {
            match button {
                Button::Help => self.ae2_help_tooltip(&mut lines),
                Button::Back | Button::StatusBack => {
                    let host = self.ae2_craft_ui.view.as_ref().map_or("ae2:crafting_terminal", |v| v.host.as_str());
                    lines.push(self.pack.names.display_item(host).to_owned());
                }
                Button::Mode => {
                    lines.push(tr("gui.tooltips.ae2.CpuSelectionMode"));
                    let key = match self.ae2_craft_ui.view.as_ref().map_or("ANY", |v| v.mode.as_str()) {
                        "PLAYER_ONLY" => "gui.tooltips.ae2.CpuSelectionModePlayersOnly",
                        "MACHINE_ONLY" => "gui.tooltips.ae2.CpuSelectionModeAutomationOnly",
                        _ => "gui.tooltips.ae2.CpuSelectionModeAny",
                    };
                    lines.push(format!("\u{a7}7{}", tr(key)));
                }
                _ => {}
            }
        } else if let Some(entry) = self.ae2_craft_table_entry(at) {
            // getEntryTooltip: the item's, then the amounts in full.
            let status = self.ae2_craft_screen() == 3;
            if let Some(stack) = entry.stack.as_ref() {
                lines.push(self.pack.names.display_item(&stack.item).to_owned());
            }
            lines.extend(table_lines(self, entry, status, true));
        }
        if lines.is_empty() {
            return false;
        }
        self.ae2_tooltip_lines(batch, lines);
        true
    }

    /// `NumberEntryWidget.validate`: what is wrong with the field's text, or
    /// what an expression comes to.
    fn ae2_craft_field_tooltip(&self) -> Vec<String> {
        use super::ae2_number_entry::number_value;
        let Some(field) = self.ae2_craft_field() else {
            return Vec::new();
        };
        let tr = |key: &str, args: &[String]| fill(&self.translate(key, key), args);
        let unit: u32 = if self.ae2_craft_fluid() { 1000 } else { 1 };
        let text = field.value.strip_prefix('=').unwrap_or(&field.value);
        let line = match number_value(text, unit, i64::MIN, i64::MAX, '.') {
            None if number_value(text, 1_000_000, i64::MIN, i64::MAX, '.').is_some() => tr("gui.ae2.NumberNonInteger", &[]),
            None => tr("gui.ae2.InvalidNumber", &[]),
            Some(n) if n < 1 => tr("gui.ae2.NumberLessThanMinValue", &[decimal_six(1.0 / unit as f64)]),
            Some(n) if n > i64::from(i32::MAX) => tr("gui.ae2.NumberGreaterThanMaxValue", &[decimal_six(f64::from(i32::MAX) / unit as f64)]),
            Some(_) if field.value.trim().parse::<f64>().is_ok() => return Vec::new(),
            Some(n) => format!("= {}", decimal_six(n as f64 / unit as f64)),
        };
        vec![line]
    }

    fn ae2_craft_table_entry(&self, at: [f32; 2]) -> Option<&Row> {
        let (x, y) = (at[0] as i32 - TABLE_AT.0, at[1] as i32 - TABLE_AT.1);
        if x < 0 || y < 0 {
            return None;
        }
        let (col, row) = (x / (CELL.0 + CELL_BORDER), y / (CELL.1 + CELL_BORDER));
        if col >= COLS || row >= self.ae2_craft_table_rows() {
            return None;
        }
        let i = (row + self.ae2_craft_ui.table.current) * COLS + col;
        self.ae2_craft_ui.view.as_ref()?.rows.get(i as usize)
    }

    /// `CPUSelectionList.getTooltip`.
    fn ae2_cpu_tooltip(&self, index: usize) -> Vec<String> {
        let tr = |key: &str, args: &[String]| fill(&self.translate(key, key), args);
        let Some(cpu) = self.ae2_craft_ui.view.as_ref().and_then(|v| v.cpus.get(index)) else {
            return Vec::new();
        };
        let mut lines = vec![format!("{} #{}", tr("gui.ae2.CPUs", &[]), index + 1)];
        match cpu.coprocessors {
            0 => {}
            1 => lines.push(format!("\u{a7}7{}", tr("gui.tooltips.ae2.CpuStatusCoProcessor", &["1".into()]))),
            n => lines.push(format!("\u{a7}7{}", tr("gui.tooltips.ae2.CpuStatusCoProcessors", &[n.to_string()]))),
        }
        lines.push(format!("\u{a7}7{}", tr("gui.tooltips.ae2.CpuStatusStorage", &[byte_amount(cpu.storage)])));
        match cpu.mode.as_str() {
            "PLAYER_ONLY" => lines.push(tr("gui.tooltips.ae2.CpuSelectionModePlayersOnly", &[])),
            "MACHINE_ONLY" => lines.push(tr("gui.tooltips.ae2.CpuSelectionModeAutomationOnly", &[])),
            _ => {}
        }
        if let Some(job) = cpu.job.as_ref() {
            let name = job.stack.as_ref().map_or_else(String::new, |s| self.pack.names.display_item(&s.item).to_owned());
            lines.push(format!("{} {name}", tr("gui.tooltips.ae2.CpuStatusCrafting", &[amount_full(job.amounts[0], job.fluid)])));
            lines.push(tr("gui.tooltips.ae2.CpuStatusCraftedIn", &[percent_text(cpu.progress), duration_text(cpu.elapsed_ms)]));
        }
        lines
    }

    /// The crafting screen for the control socket's `screen`.
    pub(super) fn ae2_craft_agent_value(&self, origin: (f32, f32)) -> Option<serde_json::Value> {
        let style = self.ae2_craft_style()?;
        let rect = |[x, y, w, h]: [i32; 4]| [origin.0 + x as f32, origin.1 + y as f32, w as f32, h as f32];
        let name = |b: Button| format!("{b:?}");
        let buttons: serde_json::Map<String, serde_json::Value> = self
            .ae2_craft_buttons(style)
            .into_iter()
            .map(|(b, r)| (name(b), serde_json::json!({"rect": rect(r), "active": self.ae2_craft_active(b)})))
            .collect();
        let view = self.ae2_craft_ui.view.as_ref();
        let (w, h) = style.size();
        Some(serde_json::json!({
            "style": match self.ae2_craft_screen() {
                1 => "craft_amount.json",
                2 => "craft_confirm.json",
                3 if self.ae2_craft_direct() => "crafting_cpu.json",
                _ => "crafting_status.json",
            },
            "size": [w, h],
            "buttons": buttons,
            "text": self.ae2_craft_field().map(|f| f.value),
            "field": self.ae2_craft_field_rects().map(|(r, _)| [origin.0 + r[0], origin.1 + r[1], r[2], r[3]]),
            "button_label": self.ae2_craft_cpu_label(),
            "rows": view.map(|v| v.rows.iter().map(|r| serde_json::json!({"item": r.stack.as_ref().map(|s| s.item.to_string()), "amounts": r.amounts})).collect::<Vec<_>>()),
            "plan": view.map(|_| self.ae2_craft_plan_texts()),
            "scroll": [self.ae2_craft_ui.table.current, self.ae2_craft_table_range()],
            "cpus": view.map(|v| v.cpus.iter().enumerate().map(|(i, c)| {
                let top = CPU_BUTTON_AT.1 + (i as i32 - self.ae2_craft_ui.cpus.current) * 23;
                let [lx, ly, _, _] = widget(style, "selectCpuList", (94, 164)).unwrap_or([-88, 0, 94, 164]);
                serde_json::json!({"storage": c.storage, "coprocessors": c.coprocessors, "job": c.job.as_ref().map(|j| j.amounts[0]),
                    "rect": rect([lx + CPU_BUTTON_AT.0, ly + top, 67, 22])})
            }).collect::<Vec<_>>()),
            "selected": view.and_then(|v| v.selected),
            "suspended": view.is_some_and(|v| v.suspended),
            "mode": view.map(|v| v.mode.as_str()),
        }))
    }
}

/// `addBackButton`'s tab on the status screen, unless the view's a CPU's own.
fn add_back(style: &ScreenStyle, buttons: &mut Vec<(Button, [i32; 4])>) {
    if let Some(rect) = widget(style, "back", (22, 22)) {
        buttons.push((Button::StatusBack, rect));
    }
}

/// `DurationFormatUtils.formatDuration(ms, "HH:mm:ss")`.
fn eta_text(ms: u64) -> String {
    let seconds = ms / 1000;
    format!("{:02}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60)
}

/// `Tooltips.ofDuration`, from milliseconds.
fn duration_text(ms: u64) -> String {
    let mut seconds = ms / 1000;
    if seconds == 0 {
        return if ms > 0 { "~0s".to_owned() } else { "0s".to_owned() };
    }
    let mut text = String::new();
    if seconds >= 3600 {
        text += &format!("{}h", seconds / 3600);
        seconds %= 3600;
    }
    if seconds >= 60 {
        text += &format!("{}m", seconds / 60);
        seconds %= 60;
    }
    if seconds > 0 {
        text += &format!("{seconds}s");
    }
    text
}

/// `Tooltips.ofPercent`'s `#.##%`.
fn percent_text(progress: f32) -> String {
    format!("{}%", format!("{:.2}", f64::from(progress) * 100.0).trim_end_matches('0').trim_end_matches('.'))
}

/// `DecimalFormat("#.######")`.
fn decimal_six(value: f64) -> String {
    let text = format!("{value:.6}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// `AEKeyType.formatAmount(amount, FULL)`: thousands grouped, fluids in buckets.
fn amount_full(amount: u64, fluid: bool) -> String {
    if !fluid {
        return grouped(amount);
    }
    let whole = grouped(amount / 1000);
    let rest = amount % 1000;
    let text = if rest == 0 { whole } else { format!("{whole}.{}", format!("{rest:03}").trim_end_matches('0')) };
    format!("{text} B")
}

/// `getEntryDescription`, or the tooltip's lines with `full` amounts.
fn table_lines(game: &Game<'_>, row: &Row, status: bool, full: bool) -> Vec<String> {
    let keys = if status {
        ["gui.ae2.FromStorage", "gui.ae2.Crafting", "gui.ae2.Scheduled"]
    } else {
        ["gui.ae2.FromStorage", "gui.ae2.Missing", "gui.ae2.ToCraft"]
    };
    let mut lines = Vec::new();
    for (key, amount) in keys.into_iter().zip(row.amounts) {
        if amount > 0 {
            let text = if full { amount_full(amount, row.fluid) } else { slot_text(amount, row.fluid) };
            lines.push(fill(&game.translate(key, key), &[text]));
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_read_as_the_screens_print_them() {
        assert_eq!(eta_text(3_723_000), "01:02:03");
        assert_eq!(eta_text(100 * 3_600_000), "100:00:00");
        assert_eq!(duration_text(0), "0s");
        assert_eq!(duration_text(500), "~0s");
        assert_eq!(duration_text(61_000), "1m1s");
        assert_eq!(duration_text(3_723_000), "1h2m3s");
        assert_eq!(duration_text(7_200_000), "2h");
    }

    #[test]
    fn amounts_read_as_the_tables_print_them() {
        assert_eq!(percent_text(0.5), "50%");
        assert_eq!(percent_text(0.0), "0%");
        assert_eq!(percent_text(0.12345), "12.35%");
        assert_eq!(amount_full(1_234_567, false), "1,234,567");
        assert_eq!(amount_full(1500, true), "1.5 B");
        assert_eq!(amount_full(2000, true), "2 B");
        assert_eq!(decimal_six(2147483.647), "2147483.647");
        assert_eq!(decimal_six(100.0), "100");
    }

    #[test]
    fn a_view_survives_its_wire_form_with_its_icons() {
        // The server sends each row's stack as canonical SNBT.
        let wire_stack = |id: &str| horizon_client::item_data::canon_stack(Some(&ItemStack::new(id, 1)));
        let mut view = CraftView { window: 7, host: "ae2:terminal".into(), ..CraftView::default() };
        view.rows.push(Row::new(wire_stack("minecraft:oak_log"), false, [1, 2, 3]));
        view.cpus.push(Cpu { storage: 4096, job: Some(Row::new(wire_stack("minecraft:stone"), false, [5, 0, 0])), ..Cpu::default() });
        let wire = serde_json::to_vec(&view).unwrap();
        let back = serde_json::from_slice::<CraftView>(&wire).unwrap().parsed();
        assert_eq!(back.rows[0].amounts, [1, 2, 3]);
        assert_eq!(&*back.rows[0].stack.as_ref().unwrap().item, "minecraft:oak_log");
        assert_eq!(&*back.cpus[0].job.as_ref().unwrap().stack.as_ref().unwrap().item, "minecraft:stone");
    }
}
