//! AE2's ME terminal screens (`MEStorageScreen` and its subclasses
//! `CraftingTermScreen` and `PatternEncodingTermScreen`, AE2 19.2.18) over
//! the guest terminal menu (`ae2_terminal::lan_send_ae2_terminal`), laid out
//! by their screen styles (`terminals/*.json`).
//!
//! The menu keeps its places: the storage entries the server shows, the
//! crafting grid, the encoding panel's ghost slots and the patterns. The
//! screen puts each where the style puts its slot semantic and hides the
//! places that hold the placeholder controls; its own widgets stand in for
//! them. The storage rows follow `TerminalStyle` and the screen's height,
//! the scrollbar scrolls them by rows (`ae2:terminal_scroll`), the toolbar's
//! settings go out as `ae2:terminal_action`, and the server's
//! `ae2:repo_view` says what the entries are and how the settings stand. Rows
//! past the menu's places are drawn from the entries that view carries, and
//! a click on one names `REPO_EXTRA_WIRE` plus its index past the places.
use super::{
    Game,
    ae2_client_config::{ClientSettings, TERMINAL_STYLES},
    ae2_screens::{argb, icon, inside},
    ae2_terminal::{ENCODING_REPO, REPO_EXTRA_WIRE, ROLL_DOWN_BUTTON, ROLL_UP_BUTTON, TERMINAL_MAX_ROWS},
};
use horizon_client::{
    ae2_screen_style::{Image, ScreenStyle, TerminalStyle},
    hud::{HudBatch, rgba},
    items::ItemStack,
    text::Text,
};
use std::sync::OnceLock;

/// `MEStorageScreen.MIN_ROWS`.
const MIN_ROWS: i32 = 2;
/// The big scrollbar's handle (`ae2:big_scroller`), and the small one's.
const BIG_HANDLE: (i32, i32) = (12, 15);
const SMALL_HANDLE: (i32, i32) = (7, 15);
/// The processing inputs' rows (`PatternEncodingTermMenu`: 81 inputs).
const PROCESSING_ROWS: i32 = 27;
/// StonecuttingEncodingPanel's recipe grid and its buttons.
const STONE_COLS: usize = 4;
const STONE_ROWS: usize = 2;
const STONE_BUTTON: (i32, i32) = (20, 22);

/// Which terminal screen the menu is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Terminal,
    Crafting,
    Encoding,
}

/// A storage entry the menu shows: its amount, whether the network can
/// craft it, and for a fluid (shown in buckets) what draws its sprite.
#[derive(Clone, Debug, Default)]
struct Entry {
    amount: u64,
    craftable: bool,
    fluid: Option<ItemStack>,
}

/// The encoding panel as the server has it.
#[derive(Clone, Debug, Default)]
struct Panel {
    /// `EncodingMode`: crafting, processing, smithing table, stonecutting.
    mode: u8,
    page: usize,
    substitutes: bool,
    fluid_substitutes: bool,
    stonecutting: Vec<Option<ItemStack>>,
    stone_selected: Option<usize>,
    /// The encoding inputs the grid has a pattern for.
    craftable: Vec<usize>,
}

/// The server's `ae2:repo_view`.
#[derive(Clone, Debug, Default)]
pub(super) struct RepoView {
    row: usize,
    total: usize,
    ack: u32,
    pinned: bool,
    online: bool,
    /// The menu shows one of its placeholder editors instead of the view.
    overlay: bool,
    /// The placeholder editor is the key types', which `KeyTypeSelectionScreen` covers.
    key_type_edit: bool,
    entries: Vec<Option<Entry>>,
    /// The entries' stacks past the menu's places.
    extra: Vec<Option<ItemStack>>,
    sort: u8,
    view: u8,
    descending: bool,
    types: Option<[bool; 2]>,
    chest: bool,
    jobs: usize,
    panel: Option<Panel>,
}

/// The open terminal screen's own state.
#[derive(Clone, Debug, Default)]
pub(super) struct TermState {
    view: Option<RepoView>,
    /// The first row the scrollbar shows, the number of the last scroll it
    /// sent, and the rows the server last heard of.
    row: usize,
    seq: u32,
    sent_rows: i32,
    /// Where the press took the scrollbar's handle, while it drags.
    drag: Option<i32>,
    /// The client settings the screen reads (`AEConfig`), as it opened.
    style: u8,
    margin: i32,
    large_fonts: bool,
    /// The stonecutting recipes' scroll, a client-side scrollbar.
    stone_scroll: usize,
    /// `TerminalSettingsScreen` while it is open: the settings it edits.
    pub(super) sub_settings: Option<ClientSettings>,
}

impl TermState {
    pub(super) fn opened(settings: &ClientSettings) -> Self {
        Self {
            style: settings.terminal_style,
            margin: settings.terminal_margin,
            large_fonts: settings.use_large_fonts,
            ..Self::default()
        }
    }

    pub(super) fn key_types_open(&self) -> bool {
        self.view.as_ref().is_some_and(|v| v.key_type_edit)
    }
}

/// The screen's widgets that are not menu places.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Widget {
    SortBy,
    ViewMode,
    KeyTypes,
    SortDir,
    Settings,
    Style,
    CraftingStatus,
    ClearGrid,
    ClearToInventory,
    Encode,
    ClearPattern,
    Substitutions,
    FluidSubstitutions,
    Tab(u8),
    Recipe(usize),
}

/// The screen laid out for the window: its style, its storage rows and
/// size.
#[derive(Clone, Copy)]
pub(super) struct Layout {
    kind: Kind,
    style: &'static ScreenStyle,
    term: &'static TerminalStyle,
    rows: i32,
    w: i32,
    h: i32,
}

impl Layout {
    fn at(&self, position: &horizon_client::ae2_screen_style::Position) -> (i32, i32) {
        position.resolve(self.w, self.h)
    }

    fn widget(&self, name: &str) -> Option<[i32; 4]> {
        let s = self.style.widgets.get(name)?;
        let (x, y) = self.at(&s.position);
        Some([x, y, s.width, s.height])
    }

    /// Where the `index`th slot of `semantic` goes on this screen.
    fn slot(&self, semantic: &str, index: usize) -> Option<(i32, i32)> {
        let s = self.style.slots.get(semantic).filter(|s| !s.hidden)?;
        let (x, y) = self.at(&s.position);
        let (dx, dy) = s.grid.map_or((0, 0), |g| g.offset(index));
        Some((x + dx, y + dy))
    }

    /// How many storage entries the menu has places for; `ae2:repo_view`
    /// carries the rest.
    fn places(&self) -> usize {
        match self.kind {
            Kind::Terminal => 45,
            Kind::Crafting => 27,
            Kind::Encoding => ENCODING_REPO.len(),
        }
    }

    /// The storage entry under `at`, if a row shows one there (`findSlot`).
    fn entry_at(&self, at: [f32; 2]) -> Option<usize> {
        (0..(self.rows as usize) * self.term.slots_per_row.max(1) as usize).find(|&index| {
            let (x, y) = self.repo_pos(index);
            inside([x - 1, y - 1, 18, 18], at)
        })
    }

    /// The menu place of storage entry `index`.
    fn repo_wire(&self, index: usize) -> usize {
        if self.kind == Kind::Encoding { ENCODING_REPO[index] } else { index }
    }

    /// The shown storage entry at menu place `wire`.
    fn repo_index(&self, wire: usize) -> Option<usize> {
        let shown = (self.rows * 9) as usize;
        let index = if self.kind == Kind::Encoding { ENCODING_REPO.iter().position(|&w| w == wire)? } else { wire };
        (index < shown).then_some(index)
    }

    fn repo_pos(&self, index: usize) -> (i32, i32) {
        let per_row = self.term.slots_per_row.max(1) as usize;
        self.term.slot((index / per_row) as i32, (index % per_row) as i32)
    }

    /// The scrollbar's track (`scrollbar.setHeight(rows * 18 - 2)`).
    fn scrollbar(&self) -> [i32; 4] {
        let [x, y, _, _] = self.widget("scrollbar").unwrap_or([175, 18, 0, 0]);
        [x, y, BIG_HANDLE.0, self.rows * self.term.row.size().1 - 2]
    }

    /// The view cells' `UpgradesPanel`. AE2 19 moved its style place to
    /// `right: 800`, off the screen; it is drawn where AE2 had it before
    /// (`right: -2`), beside the screen, so the cells stay in reach.
    fn view_cells(&self) -> (i32, i32) {
        match self.widget("viewCells") {
            Some([x, y, _, _]) if x > -40 => (x, y),
            _ => (self.w + 2, 0),
        }
    }

    /// The storage grid's bounds (`renderLinkStatus`).
    fn grid(&self) -> [i32; 4] {
        let (x0, y0) = self.term.slot(0, 0);
        let (x1, y1) = self.term.slot(self.rows - 1, self.term.slots_per_row - 1);
        [x0 - 1, y0 - 1, x1 + 17 - (x0 - 1), y1 + 17 - (y0 - 1)]
    }

    fn panel_origin(&self) -> (i32, i32) {
        (8, self.h - 165)
    }
}

fn styles(resources: &std::path::Path) -> &'static [Option<ScreenStyle>; 4] {
    static STYLES: OnceLock<[Option<ScreenStyle>; 4]> = OnceLock::new();
    STYLES.get_or_init(|| {
        let screens = resources.join("assets/ae2/screens");
        ["terminals/terminal.json", "terminals/wireless_terminal.json", "terminals/crafting_terminal.json", "terminals/pattern_encoding_terminal.json"]
            .map(|path| {
                ScreenStyle::load(&screens, path)
                    .map_err(|error| log::warn!("AE2 terminal style {path}: {error}"))
                    .ok()
                    .filter(|s| s.terminal.is_some())
            })
    })
}

/// `ReadableNumberConverter.format(long, width)`.
fn readable(number: u64, width: usize) -> String {
    let text = number.to_string();
    if text.len() <= width {
        return text;
    }
    let (mut base, mut last, mut exponent, mut size) = (number, number as f64 * 1000.0, 0usize, text.len());
    let mut postfix = 'K';
    while size > width {
        last = base as f64;
        base /= 1000;
        postfix = ['K', 'M', 'G', 'T', 'P', 'E'][exponent.min(5)];
        exponent += 1;
        size = base.to_string().len() + 1;
    }
    let precise = format!("{}{postfix}", decimal_down(last / 1000.0, 1));
    if precise.len() <= width { precise } else { format!("{base}{postfix}") }
}

/// `ReadableNumberConverter.format(double, width)`.
fn readable_units(number: f64, width: usize) -> String {
    let integer_digits = (number.log10() + 1.0).max(0.0) as i32;
    let fraction_digits = width as i32 - integer_digits - 1;
    let min_fraction = 10f64.powi(-fraction_digits);
    let fraction = number - number.floor();
    if fraction < 1e-9 || integer_digits > width as i32 - 1 {
        return readable(number as u64, width);
    }
    if fraction + 1e-9 < min_fraction && integer_digits - 1 <= width as i32 {
        return format!("~{}", readable(number as u64, width.saturating_sub(1)));
    }
    decimal_down(number, fraction_digits.max(0) as usize)
}

/// `DecimalFormat(".#")` rounding down with `digits` fraction digits at
/// most: no leading zero, no trailing zeros.
fn decimal_down(value: f64, digits: usize) -> String {
    let scale = 10u64.pow(digits as u32);
    let scaled = (value * scale as f64).floor() as u64;
    let (whole, fraction) = (scaled / scale.max(1), scaled % scale.max(1));
    let mut out = if whole == 0 && fraction != 0 { String::new() } else { whole.to_string() };
    if fraction != 0 {
        let digits = format!("{fraction:0width$}", width = digits);
        out.push('.');
        out.push_str(digits.trim_end_matches('0'));
    }
    out
}

/// `AEKeyType.formatAmount(amount, SLOT)`: buckets for fluids.
fn slot_amount(entry: &Entry, large: bool) -> String {
    let width = if large { 3 } else { 4 };
    if entry.fluid.is_some() { readable_units(entry.amount as f64 / 1000.0, width) } else { readable(entry.amount, width) }
}

fn parse_view(data: &[u8]) -> Option<(i64, RepoView)> {
    let v: serde_json::Value = serde_json::from_slice(data).ok()?;
    let window = v.get("window")?.as_i64()?;
    let uint = |key: &str| v.get(key).and_then(serde_json::Value::as_u64).unwrap_or(0);
    let flag = |key: &str| v.get(key).and_then(serde_json::Value::as_bool).unwrap_or(false);
    let entries = v.get("entries").and_then(|e| e.as_array()).map_or_else(Vec::new, |list| {
        list.iter()
            .take(TERMINAL_MAX_ROWS * 9)
            .map(|e| {
                let e = e.as_array()?;
                let fluid = e.get(2)?.as_str().and_then(|id| {
                    let key = ae2_cell_core::ItemKey { item_id: id.to_owned(), components: Vec::new() };
                    horizon_client::ae2_generic_stack::wrap(&horizon_client::ae2_generic_stack::GenericStack { key, fluid: true, amount: 1 }).ok()
                });
                Some(Entry { amount: e.first()?.as_u64()?, craftable: e.get(1)?.as_bool()?, fluid })
            })
            .collect()
    });
    let extra = v.get("extra").and_then(|e| e.as_array()).map_or_else(Vec::new, |list| {
        list.iter().take(TERMINAL_MAX_ROWS * 9).map(|s| s.as_str().and_then(|s| horizon_client::item_data::parse_stack(s).ok().flatten())).collect()
    });
    let types = v.get("types").and_then(|t| t.as_array()).and_then(|t| Some([t.first()?.as_bool()?, t.get(1)?.as_bool()?]));
    let panel = v.get("encoding").filter(|p| p.is_object()).map(|p| Panel {
        mode: p.get("mode").and_then(|m| m.as_u64()).unwrap_or(0).min(3) as u8,
        page: p.get("page").and_then(|m| m.as_u64()).unwrap_or(0).min(8) as usize,
        substitutes: p.get("substitutes").and_then(|m| m.as_bool()).unwrap_or(false),
        fluid_substitutes: p.get("fluid_substitutes").and_then(|m| m.as_bool()).unwrap_or(false),
        stonecutting: p.get("stonecutting").and_then(|m| m.as_array()).map_or_else(Vec::new, |list| {
            list.iter().take(256).map(|s| s.as_str().and_then(|s| horizon_client::item_data::parse_stack(s).ok().flatten())).collect()
        }),
        stone_selected: p.get("stone_selected").and_then(|m| m.as_u64()).map(|n| n as usize),
        craftable: p.get("craftable").and_then(|m| m.as_array()).map_or_else(Vec::new, |list| list.iter().filter_map(|n| n.as_u64()).map(|n| n as usize).collect()),
    });
    Some((
        window,
        RepoView {
            row: uint("row") as usize,
            total: uint("total") as usize,
            ack: uint("ack") as u32,
            pinned: flag("pinned"),
            online: v.get("online").and_then(serde_json::Value::as_bool).unwrap_or(true),
            overlay: flag("overlay"),
            key_type_edit: flag("key_type_edit"),
            entries,
            extra,
            sort: uint("sort").min(2) as u8,
            view: uint("view").min(2) as u8,
            descending: flag("descending"),
            types,
            chest: flag("priority"),
            jobs: uint("jobs") as usize,
            panel,
        },
    ))
}

impl Game<'_> {
    /// The terminal screen the open menu shows, if any. A placeholder
    /// editor the menu shows instead (search keys, craft amount) keeps the
    /// plain chest screen.
    pub(super) fn ae2_term_kind(&self) -> Option<Kind> {
        let r = self.remote.as_ref()?;
        if !r.ae2_terminal_screen || r.ae2_priority_screen || r.ae2_term.view.as_ref().is_some_and(|v| v.overlay && !v.key_type_edit) {
            return None;
        }
        Some(if r.ae2_crafting_screen {
            Kind::Crafting
        } else if r.ae2_encoding_screen {
            Kind::Encoding
        } else {
            Kind::Terminal
        })
    }

    fn ae2_term(&self) -> Option<&TermState> {
        self.remote.as_ref().map(|r| &r.ae2_term)
    }

    fn ae2_term_view(&self) -> Option<&RepoView> {
        self.ae2_term()?.view.as_ref()
    }

    fn ae2_term_panel(&self) -> Panel {
        self.ae2_term_view().and_then(|v| v.panel.clone()).unwrap_or_default()
    }

    fn ae2_term_mode(&self) -> u8 {
        self.ae2_term_view().and_then(|v| v.panel.as_ref()).map_or(0, |p| p.mode)
    }

    /// `MEStorageScreen.init`: rows from the screen's height, the margin and
    /// the terminal style, at least two.
    pub(super) fn ae2_term_layout(&self) -> Option<Layout> {
        let kind = self.ae2_term_kind()?;
        let wireless = self.ae2_term_view().is_some_and(|v| v.types.is_some());
        let styles = styles(&self.options.resources);
        let style = match kind {
            Kind::Terminal if wireless => styles[1].as_ref(),
            Kind::Terminal => styles[0].as_ref(),
            Kind::Crafting => styles[2].as_ref(),
            Kind::Encoding => styles[3].as_ref(),
        }?;
        let term = style.terminal.as_ref()?;
        let state = self.ae2_term()?;
        let height = self.gui_scale().map_or(240.0, |b| b.height) as i32;
        let possible = term.possible_rows(height - 2 * state.margin);
        let multiplier = i32::from(state.style.min(3)) + 1;
        let mut layout = Layout { kind, style, term, rows: MIN_ROWS, w: term.width(), h: 0 };
        layout.rows = (possible * multiplier / 4).max(MIN_ROWS).min(TERMINAL_MAX_ROWS as i32);
        layout.h = term.height(layout.rows);
        Some(layout)
    }

    pub(super) fn ae2_term_size(&self) -> Option<(f32, f32)> {
        if let Some((w, h)) = self.ae2_term_sub_size() {
            return Some((w as f32, h as f32));
        }
        self.ae2_term_layout().map(|l| (l.w as f32, l.h as f32))
    }

    /// The key types a wireless terminal shows.
    pub(super) fn ae2_term_key_types(&self) -> Option<[bool; 2]> {
        self.ae2_term_view()?.types
    }

    /// The stack of shown storage entry `index`: its menu place's, or what
    /// `ae2:repo_view` carries past the places.
    fn ae2_term_entry_stack(&self, l: &Layout, index: usize) -> Option<&ItemStack> {
        if index < l.places() {
            return self.ae2_term_stack(l.repo_wire(index));
        }
        self.ae2_term_view()?.extra.get(index - l.places())?.as_ref()
    }

    /// What a click on shown storage entry `index` names.
    fn ae2_term_entry_wire(&self, l: &Layout, index: usize) -> i16 {
        if index < l.places() { l.repo_wire(index) as i16 } else { REPO_EXTRA_WIRE + (index - l.places()) as i16 }
    }

    fn ae2_term_stack(&self, wire: usize) -> Option<&ItemStack> {
        self.container_kind().and_then(|e| e.slots().get(wire)).and_then(Option::as_ref)
    }

    /// Where menu place `i` goes on the screen (off it when the screen
    /// hides it); none when no terminal screen is open.
    pub(super) fn ae2_term_slot_position(&self, i: usize) -> Option<(f32, f32)> {
        let l = self.ae2_term_layout()?;
        let hidden = (-1000.0, -1000.0);
        if self.ae2_term_sub().is_some() {
            return Some(hidden);
        }
        let place = |p: Option<(i32, i32)>| p.map_or(hidden, |(x, y)| (x as f32, y as f32));
        let mode = self.ae2_term_mode();
        Some(match i {
            0..=8 => place(l.slot("PLAYER_HOTBAR", i)),
            9..=35 => place(l.slot("PLAYER_INVENTORY", i - 9)),
            360..=364 => {
                let (x, y) = l.view_cells();
                ((x + 1) as f32, (y + 6 + (i as i32 - 360) * 18) as f32)
            }
            365..=373 => place(l.slot("TOOLBOX", i - 365)),
            300..=353 => {
                let wire = i - 300;
                if let Some(index) = l.repo_index(wire) {
                    place(Some(l.repo_pos(index)))
                } else {
                    match (l.kind, wire) {
                        (Kind::Crafting, 27..=35) => place(l.slot("CRAFTING_GRID", wire - 27)),
                        (Kind::Crafting, 36) => place(l.slot("CRAFTING_RESULT", 0)),
                        (Kind::Encoding, 49) => place(l.slot("BLANK_PATTERN", 0)),
                        (Kind::Encoding, 50) => place(l.slot("ENCODED_PATTERN", 0)),
                        // The encode button presses this place.
                        (Kind::Encoding, 13) => place(l.widget("encodePattern").map(|[x, y, _, _]| (x, y))),
                        (Kind::Encoding, 0..=8) => place(match mode {
                            0 => l.slot("CRAFTING_GRID", wire),
                            1 => l.slot("PROCESSING_INPUTS", wire),
                            2 => l.slot(["SMITHING_TABLE_TEMPLATE", "SMITHING_TABLE_BASE", "SMITHING_TABLE_ADDITION"].get(wire).copied().unwrap_or(""), 0),
                            _ if wire == 0 => l.slot("STONECUTTING_INPUT", 0),
                            _ => None,
                        }),
                        (Kind::Encoding, 18..=20) if mode == 1 => place(l.slot("PROCESSING_OUTPUTS", wire - 18)),
                        // A menu place a widget stands for sits under it, so a
                        // click on the place is a press of the widget.
                        _ => match self.ae2_term_widget_slot(&l, wire) {
                            Some([x, y, w, h]) => ((x + w / 2 - 8) as f32, (y + h / 2 - 8) as f32),
                            None => hidden,
                        },
                    }
                }
            }
            _ => hidden,
        })
    }

    /// `click-slot` on a menu place the terminal screen does not show (a
    /// storage entry below its rows, a placeholder control with no widget):
    /// the click goes to the place itself. False for a place it shows.
    pub(super) fn ae2_term_hidden_click(&mut self, slot: usize, (x, _): (f32, f32), right: bool, shift: bool) -> bool {
        if self.ae2_term_kind().is_none() || x > -500.0 || !self.slots().contains(&slot) {
            return false;
        }
        let input = if shift { horizon_client::menu::Input::QuickMove } else { horizon_client::menu::Input::Pickup };
        self.remote_menu_ops(&[(Some(slot), i32::from(right), input)]);
        true
    }

    /// Whether `slot` is a menu place under one of the screen's widgets,
    /// which shows the widget rather than a slot.
    pub(super) fn ae2_term_button_slot(&self, slot: usize) -> bool {
        self.ae2_term_layout().is_some_and(|l| slot.checked_sub(300).is_some_and(|wire| self.ae2_term_widget_slot(&l, wire).is_some()))
    }

    /// The box of the widget that stands for menu place `wire`, the
    /// placeholder control the server keeps there: the toolbar's settings,
    /// the crafting status tab, the grid's clear buttons, the encode button,
    /// the mode (its next tab) and the substitution toggles.
    fn ae2_term_widget_slot(&self, l: &Layout, wire: usize) -> Option<[i32; 4]> {
        let widget = match (l.kind, wire) {
            (Kind::Encoding, 13) => Widget::Encode,
            (Kind::Encoding, 46) => Widget::Tab((self.ae2_term_mode() + 1) % 4),
            (Kind::Encoding, 47) => Widget::Substitutions,
            (Kind::Encoding, 44) => Widget::FluidSubstitutions,
            (Kind::Encoding, _) => return None,
            (_, 46) => Widget::SortBy,
            (_, 47) => Widget::ViewMode,
            (_, 51) => Widget::SortDir,
            (_, 52) => Widget::KeyTypes,
            (_, 49) => Widget::CraftingStatus,
            (Kind::Crafting, 37) => Widget::ClearGrid,
            (Kind::Crafting, 38) => Widget::ClearToInventory,
            _ => return None,
        };
        self.ae2_term_widgets(l).into_iter().find(|(w, _)| *w == widget).map(|(_, rect)| rect)
    }

    /// `MEStorageScreen.renderSlot` for menu place `i` at (x, y): a storage
    /// entry's item with its amount (`StackSizeRenderer`). True when the
    /// screen drew the place (or hides its item).
    pub(super) fn ae2_term_draw_slot(&self, batch: &mut HudBatch, i: usize, x: f32, y: f32) -> bool {
        let Some(l) = self.ae2_term_layout() else {
            return false;
        };
        if self.ae2_term_button_slot(i) {
            return true;
        }
        // An encoding input keeps its item and adds PatternEncodingTermScreen's mark.
        if let Some(wire) = i.checked_sub(300).filter(|&wire| self.ae2_term_input_craftable(&l, wire))
            && let Some(stack) = self.ae2_term_stack(wire)
        {
            self.draw_item(batch, stack, x, y);
            self.ae2_size_label(batch, x - 11.0, y - 11.0, "+", false);
            return true;
        }
        let Some(index) = i.checked_sub(300).and_then(|wire| l.repo_index(wire)) else {
            return false;
        };
        self.ae2_term_draw_entry(batch, &l, index, x, y);
        true
    }

    /// Whether the encoding panel's input at menu place `wire` holds what
    /// the grid has a pattern for (`shouldShowCraftableIndicatorForSlot`).
    fn ae2_term_input_craftable(&self, l: &Layout, wire: usize) -> bool {
        l.kind == Kind::Encoding && self.ae2_term_view().and_then(|v| v.panel.as_ref()).is_some_and(|p| p.craftable.contains(&wire))
    }

    /// `MEStorageScreen.renderSlot`: the entry's item or fluid and its size label.
    fn ae2_term_draw_entry(&self, batch: &mut HudBatch, l: &Layout, index: usize, x: f32, y: f32) {
        let Some(stack) = self.ae2_term_entry_stack(l, index) else {
            return;
        };
        let view = self.ae2_term_view();
        let entry = view.and_then(|v| v.entries.get(index)).and_then(Option::as_ref);
        self.draw_item_model(batch, entry.and_then(|e| e.fluid.as_ref()).unwrap_or(stack), x, y);
        let (Some(view), Some(entry)) = (view, entry) else {
            return;
        };
        let large = self.ae2_term().is_some_and(|s| s.large_fonts);
        if entry.craftable && (view.view == 2 || entry.amount == 0) {
            self.ae2_size_label(batch, x, y, "+", large);
        } else {
            self.ae2_size_label(batch, x, y, &slot_amount(entry, large), large);
            if entry.craftable {
                self.ae2_size_label(batch, x - 11.0, y - 11.0, "+", false);
            }
        }
    }

    /// The rows past the menu's places, which are no menu slots: their
    /// entries and the highlight of the one under the cursor.
    fn ae2_draw_extra_entries(&self, batch: &mut HudBatch, l: &Layout, ox: f32, oy: f32, mouse: [f32; 2]) {
        let shown = (l.rows as usize) * l.term.slots_per_row.max(1) as usize;
        let hovered = l.entry_at(mouse).filter(|&index| index >= l.places());
        for index in l.places()..shown {
            let (x, y) = l.repo_pos(index);
            let (x, y) = (ox + x as f32, oy + y as f32);
            if hovered == Some(index) {
                self.draw_slot_highlight(batch, false, x, y);
            }
            self.ae2_term_draw_entry(batch, l, index, x, y);
            if hovered == Some(index) {
                self.draw_slot_highlight(batch, true, x, y);
            }
        }
    }

    /// `StackSizeRenderer.renderSizeLabel`: `text` at the slot's bottom
    /// right, scaled down, white over a dark shadow.
    fn ae2_size_label(&self, batch: &mut HudBatch, x: f32, y: f32, text: &str, large: bool) {
        let atlas = &self.gui.atlas;
        let scale = if large { 0.85 } else { 0.666 };
        let offset = if large { 0.0 } else { -1.0 };
        let width = HudBatch::text_width(atlas, text);
        // The slot's place from the screen's origin, as the label's pose has it.
        let (ox, oy) = (x.floor(), y.floor());
        let lx = ((offset + 16.0 + 2.0 - width * scale) / scale).trunc();
        let ly = ((offset + 16.0 - 5.0 * scale) / scale).trunc();
        let start = batch.vertices.len();
        batch.text(atlas, text, ox + lx + 1.0, oy + ly + 1.0, rgba(0x41, 0x3f, 0x54, 255), false);
        batch.text(atlas, text, ox + lx, oy + ly, rgba(255, 255, 255, 255), false);
        batch.scale_since(start, ox, oy, scale, scale);
    }

    /// The left toolbar's buttons as `MEStorageScreen.init` adds them
    /// (`VerticalButtonBar`: 16 x 16, 22 apart).
    fn ae2_term_toolbar(&self, l: &Layout) -> Vec<(Widget, [i32; 4])> {
        let (x, y) = self.ae2_style_toolbar(l);
        let mut buttons = Vec::new();
        if l.term.sortable {
            buttons.push(Widget::SortBy);
        }
        if l.term.supports_auto_crafting {
            buttons.push(Widget::ViewMode);
        }
        if self.ae2_term_view().is_some_and(|v| v.types.is_some()) {
            buttons.push(Widget::KeyTypes);
        }
        buttons.push(Widget::SortDir);
        buttons.push(Widget::Settings);
        buttons.push(Widget::Style);
        buttons.into_iter().enumerate().map(|(i, b)| (b, [x - 2 - 16, y + 2 + i as i32 * 22, 16, 16])).collect()
    }

    fn ae2_style_toolbar(&self, l: &Layout) -> (i32, i32) {
        l.widget("verticalToolbar").map_or((3, 1), |[x, y, _, _]| (x, y))
    }

    /// Every widget of the screen with its box.
    fn ae2_term_widgets(&self, l: &Layout) -> Vec<(Widget, [i32; 4])> {
        let mut widgets = self.ae2_term_toolbar(l);
        if l.term.supports_auto_crafting
            && let Some([x, y, w, h]) = l.widget("craftingStatus")
        {
            widgets.push((Widget::CraftingStatus, [x, y, w.max(20), h.max(20)]));
        }
        let small = |name: &str| l.widget(name).map(|[x, y, _, _]| [x, y, 8, 8]);
        match l.kind {
            Kind::Terminal => {}
            Kind::Crafting => {
                widgets.extend(small("clearCraftingGrid").map(|r| (Widget::ClearGrid, r)));
                widgets.extend(small("clearToPlayerInv").map(|r| (Widget::ClearToInventory, r)));
            }
            Kind::Encoding => {
                let panel = self.ae2_term_panel();
                for mode in 0..4u8 {
                    if let Some([x, y, w, h]) = l.widget(&format!("modeTabButton{mode}")) {
                        widgets.push((Widget::Tab(mode), [x, y, w.max(22), h.max(22)]));
                    }
                }
                widgets.extend(l.widget("encodePattern").map(|[x, y, _, _]| (Widget::Encode, [x, y, 16, 16])));
                let (clear, subs) = match panel.mode {
                    0 => ("craftingClearPattern", Some("craftingSubstitutions")),
                    1 => ("processingClearPattern", None),
                    2 => ("smithingTableClearPattern", Some("smithingTableSubstitutions")),
                    _ => ("", None),
                };
                widgets.extend(small(clear).map(|r| (Widget::ClearPattern, r)));
                widgets.extend(subs.and_then(small).map(|r| (Widget::Substitutions, r)));
                if panel.mode == 0 {
                    widgets.extend(small("craftingFluidSubstitutions").map(|r| (Widget::FluidSubstitutions, r)));
                }
                if panel.mode == 3 {
                    let (px, py) = self.ae2_mode_panel(l);
                    let first = self.ae2_term().map_or(0, |s| s.stone_scroll) * STONE_COLS;
                    for i in 0..STONE_COLS * STONE_ROWS {
                        if first + i < panel.stonecutting.len() {
                            let (col, row) = ((i % STONE_COLS) as i32, (i / STONE_COLS) as i32);
                            let rect = [px + 26 + col * STONE_BUTTON.0, py + 12 + row * STONE_BUTTON.1, STONE_BUTTON.0, STONE_BUTTON.1];
                            widgets.push((Widget::Recipe(first + i), rect));
                        }
                    }
                }
            }
        }
        widgets
    }

    /// The encoding panel's top left (`modePanelN`).
    fn ae2_mode_panel(&self, l: &Layout) -> (i32, i32) {
        let mode = self.ae2_term_mode();
        l.widget(&format!("modePanel{mode}")).map_or((9, l.h - 166), |[x, y, _, _]| (x, y))
    }

    /// The processing inputs' scrollbar (`processingPatternModeScrollbar`),
    /// or the stonecutting recipes' one, with its range and position.
    fn ae2_panel_scrollbar(&self, l: &Layout) -> Option<([i32; 4], usize, usize)> {
        if l.kind != Kind::Encoding {
            return None;
        }
        let panel = self.ae2_term_panel();
        match panel.mode {
            1 => {
                let [x, y, _, h] = l.widget("processingPatternModeScrollbar")?;
                Some(([x, y, SMALL_HANDLE.0, h], (PROCESSING_ROWS - 3) as usize, panel.page * 3))
            }
            3 => {
                let [x, y, _, h] = l.widget("stonecuttingPatternModeScrollbar")?;
                let rows = panel.stonecutting.len().div_ceil(STONE_COLS);
                let scroll = self.ae2_term().map_or(0, |s| s.stone_scroll);
                Some(([x, y, SMALL_HANDLE.0, h], rows.saturating_sub(STONE_ROWS), scroll))
            }
            _ => None,
        }
    }

    /// The search field (`AETextField` at the style's `search`) and where
    /// its text goes, from the screen's top left.
    pub(super) fn ae2_term_field(&self) -> Option<([f32; 4], [f32; 4])> {
        if self.ae2_term_sub().is_some() {
            return None;
        }
        let l = self.ae2_term_layout()?;
        let [x, y, w, h] = l.widget("search").unwrap_or([80, 4, 89, 12]);
        // AETextField.PADDING and the room it keeps for the cursor.
        let text = [x + 2, y + 2, w - 4 - 6, h - 4];
        Some(([x, y, w, h].map(|v| v as f32), text.map(|v| v as f32)))
    }

    /// The scroll range (`updateScrollbar`): the last first row, and the
    /// rows a page and a wheel step move.
    fn ae2_term_range(&self, l: &Layout) -> (usize, usize) {
        let total = self.ae2_term_view().map_or(0, |v| v.total);
        (total.saturating_sub(l.rows as usize), (l.rows as usize / 6).max(1))
    }

    /// Scrolls the storage rows to `row` (clamped) and tells the server.
    fn ae2_term_scroll_to(&mut self, row: i64) {
        let Some(l) = self.ae2_term_layout() else {
            return;
        };
        let (max, _) = self.ae2_term_range(&l);
        let row = row.clamp(0, max as i64) as usize;
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        if r.ae2_term.row == row {
            return;
        }
        r.ae2_term.row = row;
        self.ae2_term_send_scroll(l.rows);
    }

    /// `ae2:terminal_scroll`: the window, the first row, the request's
    /// number and the rows the screen shows.
    fn ae2_term_send_scroll(&mut self, rows: i32) {
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        r.ae2_term.seq = r.ae2_term.seq.wrapping_add(1);
        r.ae2_term.sent_rows = rows;
        let window = r.menu.as_ref().map_or(0, |m| m.0);
        let mut data = window.to_be_bytes().to_vec();
        data.extend((r.ae2_term.row as u32).to_be_bytes());
        data.extend(r.ae2_term.seq.to_be_bytes());
        data.extend((rows as u32).to_be_bytes());
        self.ae2_send_payload("ae2:terminal_scroll", data);
    }

    /// Tells the server when the screen shows other rows than it last heard
    /// (`init` after a resize or a new terminal style), once it has opened.
    fn ae2_term_sync_rows(&mut self) {
        if self.ae2_term_view().is_none() || self.ae2_term_sub().is_some() {
            return;
        }
        let Some(l) = self.ae2_term_layout() else {
            return;
        };
        let known = self.ae2_term().map_or(0, |s| s.sent_rows);
        // The menu's places need no word until rows go past them.
        if known != l.rows && (known != 0 || l.rows as usize * 9 > l.places()) {
            self.ae2_term_send_scroll(l.rows);
        }
    }

    /// `ae2:repo_view` from the server: the view, and its scroll when it
    /// answers this screen's last request.
    pub(super) fn ae2_accept_repo_view(&mut self, data: &[u8]) {
        let Some((window, view)) = parse_view(data) else {
            return;
        };
        let Some(r) = self.remote.as_mut().filter(|r| r.menu.as_ref().is_some_and(|m| i64::from(m.0) == window)) else {
            return;
        };
        if view.ack == r.ae2_term.seq {
            r.ae2_term.row = view.row;
        }
        let stones = view.panel.as_ref().map_or(0, |p| p.stonecutting.len().div_ceil(STONE_COLS));
        r.ae2_term.stone_scroll = r.ae2_term.stone_scroll.min(stones.saturating_sub(STONE_ROWS));
        r.ae2_term.view = Some(view);
        self.ae2_term_sync_rows();
    }

    /// Whether `at` (from the screen's top left) is off the screen and its
    /// toolbar, tabs and panels (`hasClickedOutside`).
    pub(super) fn ae2_term_outside(&self, at: [f32; 2]) -> Option<bool> {
        let l = self.ae2_term_layout()?;
        if self.ae2_term_sub().is_some() {
            let (w, h) = self.ae2_term_sub_size()?;
            return Some(!inside([0, -8, w, h + 8], at));
        }
        if inside([0, 0, l.w, l.h], at) || self.ae2_term_widgets(&l).iter().any(|(_, r)| inside(*r, at)) {
            return Some(false);
        }
        let toolbar = self.ae2_term_toolbar(&l);
        if let (Some((_, first)), Some((_, last))) = (toolbar.first(), toolbar.last())
            && inside([first[0] - 4, first[1] - 3, 21, last[1] + 16 + 6 - first[1] + 6], at)
        {
            return Some(false);
        }
        let remote = self.remote.as_ref()?;
        if remote.ae2_view_cells.is_some() {
            let (x, y) = l.view_cells();
            if inside([x, y, 28, 10 + 5 * 18], at) {
                return Some(false);
            }
        }
        if remote.ae2_terminal_toolbox.is_some()
            && let Some([x, y, w, h]) = l.widget("toolbox")
            && inside([x, y, w, h], at)
        {
            return Some(false);
        }
        Some(true)
    }

    /// A press the screen's widgets take (true then): the toolbar, the
    /// crafting status tab, the scrollbars, the crafting and encoding
    /// buttons and tabs.
    pub(super) fn ae2_term_press(&mut self, at: [f32; 2], right: bool) -> bool {
        let Some(l) = self.ae2_term_layout() else {
            return false;
        };
        if let Some(sub) = self.ae2_term_sub() {
            return self.ae2_term_sub_press(sub, at, right);
        }
        let track = l.scrollbar();
        if !right && inside(track, at) {
            let (max, page) = self.ae2_term_range(&l);
            if max > 0 {
                let row = self.ae2_term().map_or(0, |s| s.row);
                let offset = (row * (track[3] - BIG_HANDLE.1).max(0) as usize / max) as i32;
                let rel = at[1] as i32 - track[1];
                if rel < offset {
                    self.ae2_term_scroll_to(row as i64 - page as i64);
                } else if rel < offset + BIG_HANDLE.1 {
                    if let Some(r) = self.remote.as_mut() {
                        r.ae2_term.drag = Some(rel - offset);
                    }
                } else {
                    self.ae2_term_scroll_to((row + page) as i64);
                }
            }
            return true;
        }
        if let Some((rect, range, current)) = self.ae2_panel_scrollbar(&l)
            && inside(rect, at)
        {
            if !right && range > 0 {
                let offset = (current * (rect[3] - SMALL_HANDLE.1).max(0) as usize / range) as i32;
                let rel = at[1] as i32 - rect[1];
                self.ae2_panel_step(&l, if rel < offset { -1 } else { 1 });
            }
            return true;
        }
        if let Some(index) = l.entry_at(at).filter(|&index| index >= l.places()) {
            // A row past the menu's places has no slot to take the click.
            let input = if self.has_shift_down() { horizon_client::menu::Input::QuickMove } else { horizon_client::menu::Input::Pickup };
            self.ae2_send_menu_input(self.ae2_term_entry_wire(&l, index), i32::from(right), input);
            return true;
        }
        let Some((widget, _)) = self.ae2_term_widgets(&l).into_iter().find(|(_, r)| inside(*r, at)) else {
            return false;
        };
        let settings = matches!(widget, Widget::SortBy | Widget::ViewMode | Widget::SortDir | Widget::Style);
        if right && !settings {
            return true;
        }
        self.audio.play_button_click();
        let window = self.remote.as_ref().and_then(|r| r.menu.as_ref()).map_or(0, |m| m.0);
        let action = |action: u8| {
            let mut data = window.to_be_bytes().to_vec();
            data.extend([action, u8::from(right)]);
            data
        };
        match widget {
            Widget::SortBy => self.ae2_send_payload("ae2:terminal_action", action(0)),
            Widget::ViewMode => self.ae2_send_payload("ae2:terminal_action", action(1)),
            Widget::SortDir => self.ae2_send_payload("ae2:terminal_action", action(2)),
            Widget::CraftingStatus => self.ae2_send_payload("ae2:terminal_action", action(3)),
            // TerminalSettingsScreen, on the client alone.
            Widget::Settings => {
                let settings = super::ae2_client_config::settings();
                if let Some(r) = self.remote.as_mut() {
                    r.ae2_term.sub_settings = Some(settings);
                    if let Some(field) = r.ae2_search.as_mut() {
                        field.focused = false;
                    }
                }
            }
            Widget::Style => {
                if let Some(r) = self.remote.as_mut() {
                    let style = &mut r.ae2_term.style;
                    *style = if right { (*style + 3) % 4 } else { (*style + 1) % 4 };
                    super::ae2_client_config::save_terminal_style(*style);
                }
                // The rows change: keep the scroll in the new range.
                let row = self.ae2_term().map_or(0, |s| s.row);
                self.ae2_term_scroll_to(row as i64);
            }
            // KeyTypeSelectionButton; with shift the menu's own shortcut
            // steps the visible types.
            Widget::KeyTypes if self.has_shift_down() => self.ae2_send_menu_input(52, 0, horizon_client::menu::Input::QuickMove),
            Widget::KeyTypes => self.ae2_send_menu_button(52, 0),
            Widget::ClearGrid => self.ae2_send_menu_button(37, 0),
            Widget::ClearToInventory => self.ae2_send_menu_button(38, 0),
            Widget::Encode => self.ae2_send_menu_button(13, 0),
            Widget::Substitutions => self.ae2_send_menu_button(47, 0),
            Widget::FluidSubstitutions => self.ae2_send_menu_button(44, 0),
            Widget::Tab(mode) => {
                if self.ae2_term_panel().mode != mode {
                    self.ae2_send_menu_button(46, 10 + i32::from(mode));
                }
            }
            Widget::Recipe(index) => self.ae2_send_menu_button(43, 10 + index as i32),
            Widget::ClearPattern => self.ae2_send_payload("ae2:clear_encoding", window.to_be_bytes().to_vec()),
        }
        true
    }

    /// One page of the encoding panel's own scrollbar: a processing page
    /// (three rows of inputs) or a row of stonecutting recipes.
    fn ae2_panel_step(&mut self, l: &Layout, step: i32) {
        let Some((_, range, current)) = self.ae2_panel_scrollbar(l) else {
            return;
        };
        match self.ae2_term_panel().mode {
            1 => {
                let page = current / 3;
                if step < 0 && page > 0 {
                    self.ae2_send_menu_button(42, 0);
                } else if step > 0 && (page + 1) * 3 <= range {
                    self.ae2_send_menu_button(43, 0);
                }
            }
            _ => {
                if let Some(r) = self.remote.as_mut() {
                    let scroll = &mut r.ae2_term.stone_scroll;
                    *scroll = (*scroll as i64 + i64::from(step) * STONE_ROWS as i64).clamp(0, range as i64) as usize;
                }
            }
        }
    }

    /// The scrollbar's handle follows a drag.
    pub(super) fn ae2_term_drag(&mut self) {
        let Some(grab) = self.ae2_term().and_then(|s| s.drag) else {
            return;
        };
        let Some(l) = self.ae2_term_layout() else {
            return;
        };
        let Some(oy) = self.gui_scale().map(|b| self.inventory_origin(&b).1) else {
            return;
        };
        let track = l.scrollbar();
        let (max, _) = self.ae2_term_range(&l);
        let available = (track[3] - BIG_HANDLE.1).max(1) as f32;
        let position = ((self.cursor.1 - oy - track[1] as f32 - grab as f32) / available).clamp(0.0, 1.0);
        self.ae2_term_scroll_to((position * max as f32).round() as i64);
    }

    pub(super) fn ae2_term_release(&mut self) {
        if let Some(r) = self.remote.as_mut() {
            r.ae2_term.drag = None;
        }
    }

    /// The mouse wheel: the storage scrollbar takes it anywhere on the
    /// screen (`captureMouseWheel`), the encoding panel's when the storage
    /// has nothing to scroll. Shift over a storage entry rolls one item
    /// (`mouseScrolled`: up into the network, down out of it).
    pub(super) fn ae2_term_wheel(&mut self, delta: f32) -> bool {
        let Some(l) = self.ae2_term_layout() else {
            return false;
        };
        if self.ae2_term_sub().is_some() {
            return true;
        }
        if self.has_shift_down() {
            let Some((ox, oy)) = self.gui_scale().map(|b| self.inventory_origin(&b)) else {
                return false;
            };
            let Some(index) = l.entry_at([self.cursor.0 - ox, self.cursor.1 - oy]) else {
                return false;
            };
            let button = if delta > 0.0 { ROLL_DOWN_BUTTON } else { ROLL_UP_BUTTON };
            for _ in 0..(delta.abs().round() as i32).max(1) {
                self.ae2_send_menu_input(self.ae2_term_entry_wire(&l, index), button, horizon_client::menu::Input::Pickup);
            }
            return true;
        }
        let step = (-delta).clamp(-1.0, 1.0);
        let (max, page) = self.ae2_term_range(&l);
        if max > 0 {
            let row = self.ae2_term().map_or(0, |s| s.row) as i64;
            self.ae2_term_scroll_to(row + (step * page as f32) as i64);
        } else if self.ae2_panel_scrollbar(&l).is_some_and(|(_, range, _)| range > 0) && step != 0.0 {
            self.ae2_panel_step(&l, step.signum() as i32);
        }
        true
    }

    /// The screen: its background built from the terminal style, the
    /// toolbar, panels, labels, search field, scrollbar and, for the
    /// subclasses, the crafting grid's buttons and the encoding panel.
    pub(super) fn ae2_draw_terminal(&self, batch: &mut HudBatch, ox: f32, oy: f32) {
        if let Some(sub) = self.ae2_term_sub() {
            self.ae2_draw_term_sub(batch, sub, ox, oy);
            return;
        }
        let Some(l) = self.ae2_term_layout() else {
            return;
        };
        let mouse = [self.cursor.0 - ox, self.cursor.1 - oy];
        let term = l.term;
        let blit = |game: &Self, batch: &mut HudBatch, image: &Image, y: i32| {
            let (w, h) = image.size();
            let [sx, sy] = image.src.map_or([0, 0], |[x, y, _, _]| [x, y]);
            game.ae2_blit(batch, image, [sx, sy, w, h], ox, oy + y as f32);
        };
        // drawBG: the header, the rows (two at least), the bottom.
        let mut y = 0;
        blit(self, batch, &term.header, y);
        y += term.header.size().1;
        let shown = l.rows.max(2);
        for row in 0..shown {
            let image = if row == 0 { &term.first_row } else if row + 1 == shown { &term.last_row } else { &term.row };
            blit(self, batch, image, y);
            y += term.row.size().1;
        }
        blit(self, batch, &term.bottom, y);
        let view = self.ae2_term_view();
        if view.is_some_and(|v| v.pinned) {
            let image = Image { texture: "guis/terminal".into(), texture_size: (256, 256), src: None };
            self.ae2_blit(batch, &image, [0, 204, 162, 18], ox + 7.0, oy + term.header.size().1 as f32);
        }
        if l.kind == Kind::Encoding {
            self.ae2_draw_encoding_panel(batch, ox, oy, &l, mouse);
        }
        self.ae2_draw_extra_entries(batch, &l, ox, oy, mouse);
        // The panels beside the screen.
        let remote = self.remote.as_ref();
        if remote.is_some_and(|r| r.ae2_view_cells.is_some()) {
            self.ae2_draw_upgrades(batch, ox, oy, l.view_cells(), 5);
        }
        if remote.is_some_and(|r| r.ae2_terminal_toolbox.is_some())
            && let (Some(image), Some([x, y, w, h])) = (l.style.images.get("toolbox"), l.widget("toolbox"))
        {
            let src = image.src.unwrap_or([0, 0, w, h]);
            self.ae2_blit_to(batch, image, src, [ox + x as f32, oy + y as f32, w as f32, h as f32], u32::MAX);
        }
        // The toolbar and the other widgets.
        let toolbar = self.ae2_term_toolbar(&l);
        if let (Some((_, first)), Some((_, last))) = (toolbar.first(), toolbar.last()) {
            self.ae2_draw_toolbar(batch, ox, oy, *first, *last);
        }
        for (widget, rect) in self.ae2_term_widgets(&l) {
            self.ae2_draw_widget(batch, ox, oy, &l, widget, rect, mouse);
        }
        self.ae2_draw_term_texts(batch, ox, oy, &l);
        self.ae2_draw_term_field(batch, ox, oy, &l);
        // The scrollbar's handle (drawForegroundLayer), disabled without range.
        let track = l.scrollbar();
        let (max, _) = self.ae2_term_range(&l);
        let row = self.ae2_term().map_or(0, |s| s.row).min(max);
        let offset = if max == 0 { 0 } else { row as i32 * (track[3] - BIG_HANDLE.1).max(0) / max as i32 };
        let sprite = if max == 0 { "gui/big_scroller_disabled" } else { "gui/big_scroller" };
        let image = Image { texture: sprite.into(), texture_size: BIG_HANDLE, src: None };
        self.ae2_blit(batch, &image, [0, 0, BIG_HANDLE.0, BIG_HANDLE.1], ox + track[0] as f32, oy + (track[1] + offset) as f32);
        // renderLinkStatus: a shade over the grid while it is offline.
        if view.is_some_and(|v| !v.online) {
            let [x, y, w, h] = l.grid();
            batch.fill(&self.gui.atlas, ox + x as f32, oy + y as f32, w as f32, h as f32, rgba(0, 0, 0, 0x3f));
        }
    }

    fn ae2_draw_widget(&self, batch: &mut HudBatch, ox: f32, oy: f32, l: &Layout, widget: Widget, rect: [i32; 4], mouse: [f32; 2]) {
        let view = self.ae2_term_view();
        let panel = self.ae2_term_panel();
        let half = |game: &Self, batch: &mut HudBatch, shown: (i32, i32)| {
            game.ae2_icon(batch, shown, (8, 8), ox + rect[0] as f32, oy + rect[1] as f32);
        };
        match widget {
            Widget::SortBy => {
                let shown = match view.map_or(0, |v| v.sort) { 1 => (16, 64), 2 => (96, 64), _ => (0, 64) };
                self.ae2_draw_icon_button(batch, ox, oy, rect, shown, mouse);
            }
            Widget::ViewMode => {
                let shown = match view.map_or(0, |v| v.view) { 1 => (0, 16), 2 => (48, 16), _ => (32, 16) };
                self.ae2_draw_icon_button(batch, ox, oy, rect, shown, mouse);
            }
            Widget::KeyTypes => self.ae2_draw_icon_button(batch, ox, oy, rect, (160, 16), mouse),
            Widget::SortDir => {
                let shown = if view.is_some_and(|v| v.descending) { (16, 48) } else { (0, 48) };
                self.ae2_draw_icon_button(batch, ox, oy, rect, shown, mouse);
            }
            // Icon.COG.
            Widget::Settings => self.ae2_draw_icon_button(batch, ox, oy, rect, (32, 64), mouse),
            Widget::Style => {
                let style = self.ae2_term().map_or(0, |s| s.style.min(3));
                self.ae2_draw_icon_button(batch, ox, oy, rect, (i32::from(style) * 16, 208), mouse);
            }
            Widget::CraftingStatus => {
                // TabButton (BOX) with the hammer; the active jobs on it.
                self.ae2_draw_tab(batch, ox, oy, rect, (48, 144));
                let jobs = view.map_or(0, |v| v.jobs);
                if jobs > 0 {
                    let x = ox + (rect[0] + (rect[2] - 18) / 2) as f32;
                    let y = oy + (rect[1] + (rect[3] - 18) / 2) as f32;
                    self.ae2_size_label(batch, x, y, &jobs.to_string(), self.ae2_term().is_some_and(|s| s.large_fonts));
                }
            }
            Widget::ClearGrid => half(self, batch, (224, 192)),
            Widget::ClearToInventory => half(self, batch, (232, 192)),
            Widget::ClearPattern => half(self, batch, (224, 200)),
            Widget::Substitutions => half(self, batch, if panel.substitutes { (224, 208) } else { (232, 208) }),
            Widget::FluidSubstitutions => half(self, batch, if panel.fluid_substitutes { (224, 216) } else { (232, 216) }),
            Widget::Encode => self.ae2_draw_icon_button(batch, ox, oy, rect, (128, 0), mouse),
            Widget::Tab(mode) => {
                // TabButton (HORIZONTAL): its backdrop, the mode's icon three in.
                let backdrop = if panel.mode == mode { (128, 150) } else { (128, 128) };
                let (x, y) = (ox + rect[0] as f32, oy + rect[1] as f32);
                self.ae2_icon(batch, backdrop, (22, 22), x, y);
                self.ae2_icon(batch, (i32::from(mode) * 16, 32), (16, 16), x + 3.0, y + 2.0);
            }
            Widget::Recipe(index) => {
                let selected = panel.stone_selected == Some(index);
                let hover = inside(rect, mouse);
                let src = if selected { 162 } else if hover { 184 } else { 140 };
                let image = Image { texture: "guis/pattern_modes".into(), texture_size: (256, 256), src: None };
                let (x, y) = (ox + rect[0] as f32, oy + rect[1] as f32);
                self.ae2_blit(batch, &image, [124, src, STONE_BUTTON.0, STONE_BUTTON.1], x, y);
                if let Some(Some(result)) = panel.stonecutting.get(index) {
                    let lift = if selected || hover { 3.0 } else { 2.0 };
                    self.draw_item(batch, result, x + 2.0, y + lift);
                }
            }
        }
        let _ = l;
    }

    /// The encoding panel (`EncodingModePanel.drawBackgroundLayer`), the
    /// result it would encode, the empty pattern slots' icons and its own
    /// scrollbar.
    fn ae2_draw_encoding_panel(&self, batch: &mut HudBatch, ox: f32, oy: f32, l: &Layout, mouse: [f32; 2]) {
        let panel = self.ae2_term_panel();
        let (px, py) = l.panel_origin();
        let src = match panel.mode {
            1 => [0, 70],
            2 => [128, 70],
            3 => [0, 140],
            _ => [0, 0],
        };
        let image = Image { texture: "guis/pattern_modes".into(), texture_size: (256, 256), src: None };
        self.ae2_blit(batch, &image, [src[0], src[1], 124, 66], ox + px as f32, oy + py as f32);
        let result = match panel.mode {
            0 => l.slot("CRAFTING_RESULT", 0),
            2 => l.slot("SMITHING_TABLE_RESULT", 0),
            _ => None,
        };
        if let (Some((x, y)), Some(stack)) = (result, self.ae2_term_stack(13)) {
            self.draw_item(batch, stack, ox + x as f32, oy + y as f32);
        }
        for (semantic, wire, shown) in [("BLANK_PATTERN", 49, icon::BACKGROUND_BLANK_PATTERN), ("ENCODED_PATTERN", 50, (240, 112))] {
            if self.ae2_term_stack(wire).is_none()
                && let Some((x, y)) = l.slot(semantic, 0)
            {
                self.ae2_icon(batch, shown, (16, 16), ox + x as f32, oy + y as f32);
            }
        }
        if let Some((rect, range, current)) = self.ae2_panel_scrollbar(l) {
            let offset = if range == 0 { 0 } else { (current.min(range) * (rect[3] - SMALL_HANDLE.1).max(0) as usize / range) as i32 };
            let sprite = if range == 0 { "gui/small_scroller_disabled" } else { "gui/small_scroller" };
            let image = Image { texture: sprite.into(), texture_size: SMALL_HANDLE, src: None };
            self.ae2_blit(batch, &image, [0, 0, SMALL_HANDLE.0, SMALL_HANDLE.1], ox + rect[0] as f32, oy + (rect[1] + offset) as f32);
        }
        let _ = mouse;
    }

    /// The style's labels on this screen's size: the title (the ME Chest's
    /// own name), the crafting grid's and the inventory's.
    fn ae2_draw_term_texts(&self, batch: &mut HudBatch, ox: f32, oy: f32, l: &Layout) {
        let chest = self.ae2_term_view().is_some_and(|v| v.chest);
        for (id, text) in &l.style.text {
            let label = if id == "dialog_title" && chest {
                self.translate("gui.ae2.MEChest", "ME Chest")
            } else if let Some(key) = text.text.get("translate").and_then(|v| v.as_str()) {
                self.translate(key, key)
            } else if let Some(plain) = text.text.get("text").and_then(|v| v.as_str()) {
                plain.to_owned()
            } else {
                continue;
            };
            let (x, y) = l.at(&text.position);
            let width = HudBatch::text_width(&self.gui.atlas, &label);
            let x = match text.align {
                horizon_client::ae2_screen_style::Align::Left => x as f32,
                horizon_client::ae2_screen_style::Align::Center => x as f32 - width / 2.0,
                horizon_client::ae2_screen_style::Align::Right => x as f32 - width,
            };
            batch.text(&self.gui.atlas, &label, ox + x, oy + y as f32, argb(l.style.color(&text.color)), false);
        }
    }

    /// `AETextField.renderWidget`: three slices of `text_field.png`, the
    /// focused row while focused, the placeholder while empty.
    fn ae2_draw_term_field(&self, batch: &mut HudBatch, ox: f32, oy: f32, l: &Layout) {
        let (Some((rect, text)), Some(mut field)) = (self.ae2_term_field(), self.remote.as_ref().and_then(|r| r.ae2_search.clone())) else {
            return;
        };
        let row = if field.focused { 24 } else { 0 };
        let image = Image { texture: "guis/text_field".into(), texture_size: (128, 128), src: None };
        let (x, y, w) = (ox + rect[0], oy + rect[1], rect[2]);
        let middle = (w - 2.0).min(126.0);
        self.ae2_blit(batch, &image, [0, row, 1, 12], x, y);
        self.ae2_blit(batch, &image, [1, row, middle as i32, 12], x + 1.0, y);
        self.ae2_blit(batch, &image, [127, row, 1, 12], x + w - 1.0, y);
        field.bordered = false;
        field.text_color = argb(l.style.color("TEXTFIELD_TEXT"));
        [field.x, field.y, field.width, field.height] = [ox + text[0], oy + text[1], text[2], text[3]];
        let empty = field.value.is_empty() && !field.focused;
        field.draw(batch, &self.gui.atlas, None);
        if empty {
            let placeholder = self.translate("gui.ae2.SearchPlaceholder", "Search...");
            batch.text(&self.gui.atlas, &placeholder, ox + text[0], oy + text[1], argb(l.style.color("TEXTFIELD_PLACEHOLDER")), false);
        }
    }

    /// The tooltip of the widget under the cursor; true when it drew one.
    pub(super) fn ae2_term_tooltip(&mut self, batch: &mut HudBatch) -> bool {
        // Every frame the screen shows: the rows may have changed.
        self.ae2_term_sync_rows();
        let Some(l) = self.ae2_term_layout() else {
            return false;
        };
        if self.ae2_term_sub().is_some() {
            return false;
        }
        let (ox, oy) = self.inventory_origin(batch);
        let at = [self.cursor.0 - ox, self.cursor.1 - oy];
        if self.carried.is_none()
            && let Some(stack) = l.entry_at(at).filter(|&index| index >= l.places()).and_then(|index| self.ae2_term_entry_stack(&l, index).cloned())
        {
            self.draw_stack_tooltip(batch, &stack, self.cursor.0, self.cursor.1);
            return true;
        }
        let view = self.ae2_term_view();
        let panel = self.ae2_term_panel();
        let tip = |key: &str| self.translate(key, key);
        let gray = |key: &str| tip(key).split('\n').map(|line| format!("\u{a7}7{line}")).collect::<Vec<_>>();
        let mut lines: Vec<String> = Vec::new();
        if let Some((widget, _)) = self.ae2_term_widgets(&l).into_iter().find(|(_, r)| inside(*r, at)) {
            let (title, value): (String, Vec<String>) = match widget {
                Widget::SortBy => (
                    tip("gui.tooltips.ae2.SortBy"),
                    gray(["gui.tooltips.ae2.ItemName", "gui.tooltips.ae2.NumberOfItems", "gui.tooltips.ae2.Mod"][usize::from(view.map_or(0, |v| v.sort).min(2))]),
                ),
                Widget::ViewMode => (
                    tip("gui.tooltips.ae2.View"),
                    gray(["gui.tooltips.ae2.StoredCraftable", "gui.tooltips.ae2.StoredItems", "gui.tooltips.ae2.Craftable"][usize::from(view.map_or(0, |v| v.view).min(2))]),
                ),
                Widget::KeyTypes => (tip("gui.ae2.ConfigureVisibleTypes"), Vec::new()),
                Widget::SortDir => (
                    tip("gui.tooltips.ae2.SortOrder"),
                    gray(if view.is_some_and(|v| v.descending) { "gui.tooltips.ae2.Descending" } else { "gui.tooltips.ae2.Ascending" }),
                ),
                Widget::Settings => (tip("gui.tooltips.ae2.TerminalSettings"), Vec::new()),
                Widget::Style => {
                    let style = self.ae2_term().map_or(0, |s| s.style.min(3));
                    let name = ["Small", "Medium", "Tall", "Full"][usize::from(style)];
                    (tip("gui.tooltips.ae2.TerminalStyle"), gray(&format!("gui.tooltips.ae2.TerminalStyle_{name}")))
                }
                Widget::CraftingStatus => (tip("gui.ae2.CraftingStatus"), Vec::new()),
                Widget::ClearGrid => (tip("gui.tooltips.ae2.Stash"), gray("gui.tooltips.ae2.StashDesc")),
                Widget::ClearToInventory => (tip("gui.tooltips.ae2.StashToPlayer"), gray("gui.tooltips.ae2.StashToPlayerDesc")),
                Widget::Encode => (tip("gui.tooltips.ae2.Encode"), gray("gui.tooltips.ae2.EncodeDescription")),
                Widget::ClearPattern => (tip("gui.tooltips.ae2.Clear"), gray("gui.tooltips.ae2.ClearSettings")),
                Widget::Substitutions if panel.substitutes => {
                    (tip("gui.tooltips.ae2.SubstitutionsOn"), gray("gui.tooltips.ae2.SubstitutionsDescEnabled"))
                }
                Widget::Substitutions => (tip("gui.tooltips.ae2.SubstitutionsOff"), gray("gui.tooltips.ae2.SubstitutionsDescDisabled")),
                Widget::FluidSubstitutions => (
                    tip("gui.tooltips.ae2.FluidSubstitutions"),
                    gray(if panel.fluid_substitutes {
                        "gui.tooltips.ae2.FluidSubstitutionsDescEnabled"
                    } else {
                        "gui.tooltips.ae2.FluidSubstitutionsDescDisabled"
                    }),
                ),
                Widget::Tab(mode) => (
                    tip(["gui.ae2.CraftingPattern", "gui.ae2.ProcessingPattern", "gui.ae2.SmithingTablePattern", "gui.ae2.StonecuttingPattern"][usize::from(mode.min(3))]),
                    Vec::new(),
                ),
                Widget::Recipe(index) => {
                    if let Some(Some(stack)) = panel.stonecutting.get(index) {
                        self.draw_stack_tooltip(batch, stack, self.cursor.0, self.cursor.1);
                        return true;
                    }
                    return false;
                }
            };
            lines.push(title);
            lines.extend(value);
        } else if self.ae2_term_field().is_some_and(|(r, _)| inside(r.map(|v| v as i32), at)) && self.carried.is_none() {
            lines.push(tip("gui.ae2.SearchTooltip"));
            lines.extend(["gui.ae2.SearchTooltipModId", "gui.ae2.SearchTooltipTag", "gui.ae2.SearchTooltipItemId"].map(|k| format!("\u{a7}7{}", tip(k))));
        } else {
            return false;
        }
        // AEBaseScreen.drawTooltip: wrapped to half the screen less 40.
        let max = batch.width / 2.0 - 40.0;
        let mut wrapped = Vec::new();
        for line in lines {
            let (prefix, body) = match line.strip_prefix("\u{a7}7") {
                Some(body) => ("\u{a7}7", body.to_owned()),
                None => ("", line),
            };
            let mut current = String::new();
            for word in body.split(' ') {
                let candidate = if current.is_empty() { word.to_owned() } else { format!("{current} {word}") };
                if !current.is_empty() && HudBatch::text_width(&self.gui.atlas, &candidate) > max {
                    wrapped.push(format!("{prefix}{current}"));
                    current = word.to_owned();
                } else {
                    current = candidate;
                }
            }
            wrapped.push(format!("{prefix}{current}"));
        }
        let lines: Vec<(Text, u32)> = wrapped.into_iter().map(|l| (Text::legacy(&l), u32::MAX)).collect();
        self.draw_tooltip_lines(batch, &lines, self.cursor.0, self.cursor.1);
        true
    }

    /// The terminal screen for the control socket's `screen`: its rows,
    /// size, scroll, and the widgets' boxes in GUI pixels.
    pub(super) fn ae2_term_agent_value(&self, origin: (f32, f32)) -> Option<serde_json::Value> {
        let l = self.ae2_term_layout()?;
        if let Some(sub) = self.ae2_term_sub() {
            return self.ae2_term_sub_agent_value(sub, origin);
        }
        let rect = |[x, y, w, h]: [i32; 4]| [origin.0 + x as f32, origin.1 + y as f32, w as f32, h as f32];
        let widgets: Vec<serde_json::Value> = self
            .ae2_term_widgets(&l)
            .into_iter()
            .map(|(w, r)| serde_json::json!({"widget": format!("{w:?}"), "rect": rect(r)}))
            .collect();
        let state = self.ae2_term()?;
        Some(serde_json::json!({
            "kind": format!("{:?}", l.kind),
            "rows": l.rows,
            "size": [l.w, l.h],
            "row": state.row,
            "view_row": self.ae2_term_view().map_or(0, |v| v.row),
            "total": self.ae2_term_view().map_or(0, |v| v.total),
            "style": TERMINAL_STYLES[usize::from(state.style.min(3))],
            "scrollbar": rect(l.scrollbar()),
            "widgets": widgets,
            "craftable_inputs": self.ae2_term_view().and_then(|v| v.panel.as_ref()).map_or_else(Vec::new, |p| p.craftable.clone()),
            "entries": (0..(l.rows as usize) * 9).filter_map(|index| {
                let stack = self.ae2_term_entry_stack(&l, index)?;
                let (x, y) = l.repo_pos(index);
                let fluid = self.ae2_term_view()?.entries.get(index)?.as_ref().is_some_and(|e| e.fluid.is_some());
                Some(serde_json::json!({"index": index, "item": stack.item.as_ref(), "extra": index >= l.places(), "fluid": fluid, "rect": rect([x, y, 16, 16])}))
            }).collect::<Vec<_>>(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_view_carries_fluids_rows_past_the_places_and_the_key_type_editor() {
        let extra = horizon_client::item_data::canon_stack(Some(&ItemStack::new("minecraft:stone", 3)));
        let empty = horizon_client::item_data::canon_stack(None);
        let data = serde_json::json!({
            "window": 3, "row": 2, "total": 9, "overlay": true, "key_type_edit": true,
            "entries": [[5, false, null], [2000, true, "minecraft:water"], null],
            "extra": [extra, empty],
            "encoding": {"mode": 1, "craftable": [0, 4]},
        });
        let (window, view) = parse_view(data.to_string().as_bytes()).unwrap();
        assert_eq!((window, view.row, view.total), (3, 2, 9));
        assert!(view.overlay && view.key_type_edit);
        let entry = |i: usize| view.entries[i].as_ref();
        assert!(entry(0).unwrap().fluid.is_none());
        let water = entry(1).unwrap().fluid.as_ref().unwrap();
        assert_eq!(water.item.as_ref(), "ae2:wrapped_generic_stack");
        assert!(entry(2).is_none());
        assert_eq!(view.extra.len(), 2);
        assert_eq!(view.extra[0].as_ref().map(|s| (s.item.as_ref(), s.count)), Some(("minecraft:stone", 3)));
        assert!(view.extra[1].is_none());
        assert_eq!(view.panel.unwrap().craftable, vec![0, 4]);
        // Buckets for a fluid, plain numbers otherwise.
        assert_eq!(slot_amount(entry(1).unwrap(), false), "2");
        assert_eq!(slot_amount(entry(0).unwrap(), false), "5");
    }

    #[test]
    fn amounts_follow_readable_number_converter() {
        assert_eq!(readable(32, 4), "32");
        assert_eq!(readable(1234, 4), "1234");
        assert_eq!(readable(12345, 4), "12K");
        assert_eq!(readable(99999, 4), "99K");
        assert_eq!(readable(123456, 4), "123K");
        assert_eq!(readable(1_500_000, 4), "1.5M");
        assert_eq!(readable(1_500_000, 3), "1M");
        assert_eq!(readable(20_000_000_000, 4), "20G");
        assert_eq!(readable_units(1.0, 4), "1");
        assert_eq!(readable_units(1.5, 4), "1.5");
        assert_eq!(readable_units(0.25, 4), ".25");
        assert_eq!(readable_units(0.001, 4), ".001");
        assert_eq!(readable_units(0.0001, 4), "~0");
        assert_eq!(readable_units(12345.5, 4), "12K");
    }
}
