//! The requester's screen on this client (`AbstractRequesterScreen`,
//! `RequesterScreen`, `RequesterTerminalScreen`, `RequestWidget`,
//! `StateBox`, `NumberField`, `SubmitButton`, `StatusDisplay`), drawn from
//! the mod's own textures at its sizes over the menu `menu.rs` opens. The
//! lines come as the `merequester:view` payload; the ghost slots are the
//! menu's container slots, so items, clicks and tooltips are the menu's.
use super::menu::{SLOTS, VIEW, locked};
use crate::game::{Game, Screen, ae2_number_entry, ae2_screens::icon};
use horizon_client::{
    ae2_screen_style::Image,
    ae2x::merequester::{Config, REQUESTER},
    hud::{HudBatch, rgba},
    ui::EditBox,
};
use horizon_protocol::packets::{configuration::CustomPayload, play::Serverbound as S};
use serde_json::{Value, json};

const GUI_WIDTH: f32 = 195.0;
const HEADER: f32 = 20.0;
const FOOTER: f32 = 99.0;
const ROW: f32 = 19.0;
const PADDING_X: f32 = 8.0;
const PADDING_Y: f32 = 6.0;
const MIN_ROWS: usize = 3;
/// `RequesterScreen.MAX_ROW_COUNT`.
const MAX_ROWS: usize = 10;
/// AE2's default `terminalMargin`.
const TERMINAL_MARGIN: f32 = 25.0;
const TEXT_MAX_WIDTH: f32 = 156.0;
/// `NumberField`: 52 x 12, 7 characters, 10 pixels kept for the unit.
const FIELD_WIDTH: f32 = 52.0;
const FIELD_PADDING: f32 = 10.0;
const FIELD_MAX: usize = 7;
const SEARCH: [f32; 4] = [104.0, 4.0, 65.0, 12.0];
/// `AETextField`: two pixels of padding, and room for the cursor (`width("_")`).
const TEXT_PADDING: f32 = 2.0;
const CURSOR_ROOM: f32 = 6.0;
/// The help button (`OpenGuideButton`), alone on the left toolbar: the
/// `verticalToolbar` of `common.json` at (3, 1).
const HELP: [i32; 4] = [3 - 2 - 16, 1 + 2, 16, 16];
/// `Scrollbar.BIG`'s handle.
const HANDLE: (i32, i32) = (12, 15);

#[derive(Clone, Debug, PartialEq)]
enum Line {
    Header { name: String, count: usize },
    Row { r: usize, i: usize, on: bool, amount: i64, batch: i64, status: String, key: bool, fluid: bool },
}

/// The open requester screen's lines and its focused number field.
pub(in crate::game) struct View {
    window: i32,
    terminal: bool,
    offset: usize,
    lines: Vec<Line>,
    field: Option<Field>,
    /// The terminal's search (`requester_terminal.json`: 65 x 12 at 104, 4).
    search: EditBox,
}
struct Field {
    line: usize,
    batch: bool,
    edit: EditBox,
}

/// `common/palette.json`.
const TEXTFIELD_TEXT: u32 = 0xFFF2F2F2;
const TEXTFIELD_PLACEHOLDER: u32 = 0xFFDEDFE3;

/// `ChatFormatting` colours of `StatusDisplay`.
fn status_colour(status: &str, inactive: bool) -> u32 {
    if inactive {
        return 0x555555;
    }
    match status {
        "missing" => 0xFF5555,
        "cpu" => 0xFFAA00,
        "link" => 0xFFFF55,
        "export" => 0xAA00AA,
        _ => 0x00AA00,
    }
}
fn argb(c: u32) -> u32 {
    rgba((c >> 16) as u8, (c >> 8) as u8, c as u8, 255)
}
/// What a number field's background covers (`getVisualBounds` after
/// `adjustToType`): a fluid's leaves room for the `B` beside it.
fn field_width(fluid: bool) -> f32 {
    FIELD_WIDTH - if fluid { FIELD_PADDING } else { 0.0 }
}
/// `NumberField.setLongValue`: the amount in the key's unit, `#.######`.
fn shown_number(value: i64, fluid: bool) -> String {
    if !fluid {
        return value.max(0).to_string();
    }
    let v = value.max(0) as f64 / 1000.0;
    let text = format!("{v:.6}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}
/// `NumberField.getLongValue`: whole numbers for items, up to three
/// decimals of a bucket for fluids; none for anything else.
fn field_value(text: &str, fluid: bool) -> Option<i64> {
    if let Some((_, decimals)) = text.split_once('.') {
        let digits = decimals.chars().take_while(char::is_ascii_digit).count();
        if digits > if fluid { 3 } else { 0 } {
            return None;
        }
    }
    ae2_number_entry::number_value(text.trim(), if fluid { 1000 } else { 1 }, 0, i64::MAX, '.')
}

impl Game<'_> {
    /// The requester screen showing now, if any.
    fn merequester_view(&self) -> Option<&View> {
        self.ae2x.merequester.client.as_ref().filter(|v| {
            self.screen == Screen::Inventory && self.remote.as_ref().is_some_and(|r| r.menu.as_ref().is_some_and(|m| m.0 == v.window))
        })
    }
    fn merequester_gui_height(&self) -> f32 {
        self.renderer.as_ref().map_or(240.0, |r| self.screen_batch([r.config.width, r.config.height]).height)
    }
    /// `init`: as many rows as the window fits, at least three; the block's
    /// up to ten and its row count.
    fn merequester_rows(&self, view: &View) -> usize {
        let height = self.merequester_gui_height();
        if view.terminal {
            let possible = ((height - 2.0 * TERMINAL_MARGIN - HEADER - FOOTER) / ROW).floor().max(0.0) as usize;
            possible.clamp(MIN_ROWS, SLOTS)
        } else {
            let possible = ((height - HEADER - FOOTER) / ROW).floor().max(0.0) as usize;
            possible.clamp(MIN_ROWS, Config::get().requests.min(MAX_ROWS).max(MIN_ROWS))
        }
    }
    pub(in crate::game) fn merequester_screen_size(&self) -> Option<(f32, f32)> {
        let view = self.merequester_view()?;
        Some((GUI_WIDTH, HEADER + FOOTER + self.merequester_rows(view) as f32 * ROW + 1.0))
    }
    /// The player's inventory and the ghost slots of the request lines shown.
    pub(in crate::game) fn merequester_slots(&self) -> Option<Vec<usize>> {
        let view = self.merequester_view()?;
        let rows = self.merequester_rows(view);
        let ghosts = (0..rows).filter(|j| matches!(view.lines.get(view.offset + j), Some(Line::Row { .. }))).map(|j| 300 + j);
        Some((0..36).chain(ghosts).collect())
    }
    pub(in crate::game) fn merequester_slot_position(&self, i: usize) -> Option<(f32, f32)> {
        let (_, h) = self.merequester_screen_size()?;
        let view = self.merequester_view()?;
        Some(match i {
            0..=8 => (8.0 + i as f32 * 18.0, h - 26.0),
            9..=35 => (8.0 + ((i - 9) % 9) as f32 * 18.0, h - 84.0 + ((i - 9) / 9) as f32 * 18.0),
            // `createSlot`: ROW_HEIGHT + GUI_PADDING_X, (index + 1) * ROW_HEIGHT + 2.
            300.. if i - 300 < self.merequester_rows(view) => (ROW + PADDING_X, (i - 300 + 1) as f32 * ROW + 2.0),
            _ => (-1000.0, -1000.0),
        })
    }

    /// `merequester:view`: the lines of window `window`.
    pub(in crate::game) fn merequester_payload(&mut self, channel: &str, data: &[u8]) -> bool {
        if channel != VIEW {
            return channel.starts_with("merequester:");
        }
        let Ok(value) = serde_json::from_slice::<Value>(data) else { return true };
        let Some(window) = value.get("window").and_then(Value::as_i64).and_then(|w| i32::try_from(w).ok()) else { return true };
        let lines = value
            .get("lines")
            .and_then(Value::as_array)
            .map(|lines| {
                lines
                    .iter()
                    .map(|l| match l.get("h").and_then(Value::as_str) {
                        Some(name) => Line::Header { name: name.into(), count: l.get("n").and_then(Value::as_u64).unwrap_or(1) as usize },
                        None => Line::Row {
                            r: l.get("r").and_then(Value::as_u64).unwrap_or(0) as usize,
                            i: l.get("i").and_then(Value::as_u64).unwrap_or(0) as usize,
                            on: l.get("on").and_then(Value::as_bool).unwrap_or(true),
                            amount: l.get("amount").and_then(Value::as_i64).unwrap_or(0),
                            batch: l.get("batch").and_then(Value::as_i64).unwrap_or(1),
                            status: l.get("status").and_then(Value::as_str).unwrap_or("idle").into(),
                            key: l.get("key").and_then(Value::as_bool).unwrap_or(false),
                            fluid: l.get("fluid").and_then(Value::as_bool).unwrap_or(false),
                        },
                    })
                    .collect()
            })
            .unwrap_or_default();
        let terminal = value.get("terminal").and_then(Value::as_bool).unwrap_or(false);
        let (field, search) = match self.ae2x.merequester.client.take().filter(|v| v.window == window) {
            Some(old) => (old.field, old.search),
            None => {
                // `setInitialFocus(searchField)`.
                let mut search = EditBox::new(SEARCH[0], SEARCH[1], SEARCH[2], SEARCH[3]);
                search.max_length = 128;
                search.bordered = false;
                search.focused = terminal;
                (None, search)
            }
        };
        self.ae2x.merequester.client = Some(View {
            window,
            terminal,
            search,
            offset: value.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize,
            lines,
            field,
        });
        true
    }
    fn merequester_send(&self, channel: &str, mut value: Value) {
        let Some(view) = self.ae2x.merequester.client.as_ref() else { return };
        value["window"] = json!(view.window);
        self.send(S::CustomPayload(CustomPayload { channel: channel.into(), data: serde_json::to_vec(&value).unwrap_or_default() }));
    }

    /// `drawBG` and `drawFG`: the header, a request or a name texture for
    /// each line, the footer, the widgets and the status bars.
    pub(in crate::game) fn merequester_draw(&self, batch: &mut HudBatch, ox: f32, oy: f32) -> bool {
        let Some(view) = self.merequester_view() else { return false };
        let atlas = &self.gui.atlas;
        let texture = if view.terminal { "merequester:gui/requester_terminal" } else { "merequester:gui/requester" };
        let rows = self.merequester_rows(view);
        // HEADER_BBOX, REQUEST_BBOX (0, 38), TEXT_BBOX (0, 60), the footer (0, 114) or (0, 133).
        batch.sprite_subregion(atlas, texture, [ox, oy, GUI_WIDTH, HEADER], [0.0, 0.0], 256.0, 256.0);
        for j in 0..rows {
            let src = if matches!(view.lines.get(view.offset + j), Some(Line::Row { .. })) { 38.0 } else { 60.0 };
            batch.sprite_subregion(atlas, texture, [ox, oy + HEADER + j as f32 * ROW, GUI_WIDTH, ROW], [0.0, src], 256.0, 256.0);
        }
        let footer = if view.terminal { 133.0 } else { 114.0 };
        batch.sprite_subregion(atlas, texture, [ox, oy + HEADER + rows as f32 * ROW, GUI_WIDTH, FOOTER + 2.0], [0.0, footer], 256.0, 256.0);
        let text_colour = rgba(64, 64, 64, 255);
        batch.text(atlas, &self.merequester_title(view), ox + 8.0, oy + 6.0, text_colour, false);
        let (_, h) = self.merequester_screen_size().unwrap_or((GUI_WIDTH, 0.0));
        batch.text(atlas, &self.translate("container.inventory", "Inventory"), ox + 8.0, oy + h - 95.0, text_colour, false);
        let mouse = [self.cursor.0 - ox, self.cursor.1 - oy];
        self.ae2_draw_toolbar(batch, ox, oy, HELP, HELP);
        self.ae2_draw_icon_button(batch, ox, oy, HELP, icon::HELP, mouse);
        if view.terminal {
            self.merequester_text_field(batch, ox + SEARCH[0], oy + SEARCH[1], SEARCH[2], if view.search.focused { 24 } else { 0 });
            let mut search = view.search.clone();
            (search.x, search.y, search.width, search.height) =
                (ox + SEARCH[0] + TEXT_PADDING, oy + SEARCH[1] + TEXT_PADDING, SEARCH[2] - 2.0 * TEXT_PADDING - CURSOR_ROOM, SEARCH[3] - 2.0 * TEXT_PADDING);
            search.text_color = TEXTFIELD_TEXT;
            search.draw(batch, atlas, None);
            if search.value.is_empty() && !search.focused {
                batch.text(atlas, &self.translate("gui.ae2.SearchPlaceholder", "Search..."), search.x, search.y, TEXTFIELD_PLACEHOLDER, false);
            }
        }
        if view.lines.is_empty() {
            let text = self.translate("gui.merequester.no_requesters", "No ME Requesters found!");
            let width = HudBatch::text_width(atlas, &text);
            batch.text(atlas, &text, ox + ((GUI_WIDTH - width) / 2.0 - 10.0).floor(), oy + PADDING_Y + HEADER, text_colour, false);
        }
        for j in 0..rows {
            let y = (j + 1) as f32 * ROW + 1.0;
            match view.lines.get(view.offset + j) {
                Some(Line::Header { name, count }) => {
                    let mut text = if *count > 1 { format!("{name} ({count})") } else { name.clone() };
                    while text.chars().count() > 1 && HudBatch::text_width(atlas, &text) > TEXT_MAX_WIDTH {
                        text.pop();
                    }
                    batch.text(atlas, &text, ox + PADDING_X + 2.0, oy + PADDING_Y + HEADER + j as f32 * ROW, text_colour, false);
                }
                Some(Line::Row { on, amount, batch: size, status, key, fluid, .. }) => {
                    let line = view.offset + j;
                    // StateBox: 14 x 14 from state_box.png, the hover column under the cursor.
                    let state = [PADDING_X + 2.0, y + 2.0, 14.0, 14.0];
                    let hover = inside(state, mouse);
                    let src = [if hover { 14.0 } else { 0.0 }, if *on { 14.0 } else { 0.0 }];
                    batch.sprite_subregion(atlas, "merequester:gui/state_box", [ox + state[0], oy + state[1], 14.0, 14.0], src, 28.0, 28.0);
                    let is_locked = locked(status);
                    for (batch_field, value, dx) in [(false, *amount, 38.0), (true, *size, 92.0)] {
                        let width = FIELD_WIDTH - FIELD_PADDING - if *fluid { 10.0 } else { 0.0 };
                        let (fx, fy) = (ox + PADDING_X + dx, oy + y);
                        let focused = view.field.as_ref().filter(|f| f.line == line && f.batch == batch_field);
                        // AETextField's rows: 0, 12 when it can't be edited, 24 focused.
                        let row = if is_locked { 12 } else if focused.is_some() { 24 } else { 0 };
                        self.merequester_text_field(batch, fx, fy, field_width(*fluid), row);
                        match focused {
                            Some(field) => {
                                let mut edit = field.edit.clone();
                                (edit.x, edit.y, edit.width, edit.height) = (fx + TEXT_PADDING, fy + TEXT_PADDING, width, 8.0);
                                let valid = field_value(&edit.value, *fluid).is_some();
                                edit.text_color = if valid { 0xFFFFFFFF } else { 0xFFFF0000 };
                                edit.draw(batch, atlas, None);
                            }
                            None => {
                                // `EditBox` draws its text with a shadow; 0x707070 when it can't be edited.
                                let colour = if is_locked { argb(0x707070) } else { rgba(255, 255, 255, 255) };
                                batch.text(atlas, &shown_number(value, *fluid), fx + TEXT_PADDING, fy + TEXT_PADDING, colour, true);
                            }
                        }
                        if *fluid {
                            batch.text(atlas, "B", fx + TEXT_PADDING + FIELD_WIDTH - FIELD_PADDING, fy + TEXT_PADDING, argb(0x545454), false);
                        }
                    }
                    // SubmitButton: 12 x 12, the focused half when hovered.
                    let submit = [PADDING_X + 146.0, y, 12.0, 12.0];
                    let src = if inside(submit, mouse) { 12.0 } else { 0.0 };
                    batch.sprite_subregion(atlas, "merequester:gui/submit_button", [ox + submit[0], oy + submit[1], 12.0, 12.0], [src, 0.0], 24.0, 12.0);
                    // StatusDisplay: 118 x 2 under the fields.
                    let inactive = !(*on && *key) || *amount == 0;
                    batch.fill(atlas, ox + PADDING_X + 39.0, oy + y + 15.0, 118.0, 2.0, argb(status_colour(status, inactive)));
                }
                None => {}
            }
        }
        // Scrollbar.BIG at (175, 18): its track is the texture's, the handle
        // AE2's sprite, the disabled one when it can't move.
        let track = rows as i32 * ROW as i32 + 1;
        let max = view.lines.len().saturating_sub(rows) as i32;
        let handle_y = if max == 0 { 0 } else { (view.offset as i32).min(max) * (track - HANDLE.1) / max };
        let sprite = if max == 0 { "gui/big_scroller_disabled" } else { "gui/big_scroller" };
        let image = Image { texture: sprite.into(), texture_size: HANDLE, src: None };
        self.ae2_blit(batch, &image, [0, 0, HANDLE.0, HANDLE.1], ox + 175.0, oy + 18.0 + handle_y as f32);
        true
    }
    /// `AETextField.renderWidget`: its background in three slices.
    fn merequester_text_field(&self, batch: &mut HudBatch, x: f32, y: f32, width: f32, row: i32) {
        let image = Image { texture: "guis/text_field".into(), texture_size: (128, 128), src: None };
        let middle = (width - 2.0).min(126.0);
        self.ae2_blit(batch, &image, [0, row, 1, 12], x, y);
        self.ae2_blit(batch, &image, [1, row, middle as i32, 12], x + 1.0, y);
        self.ae2_blit(batch, &image, [127, row, 1, 12], x + width - 1.0, y);
    }

    /// A press on the screen's widgets: true when one took it. The ghost
    /// slots are left to the menu.
    pub(in crate::game) fn merequester_press(&mut self, at: [f32; 2], right: bool) -> bool {
        let Some(view) = self.merequester_view() else { return false };
        // `OpenGuideButton`.
        if !right && inside(HELP.map(|n| n as f32), at) {
            self.audio.play_button_click();
            self.ae2_open_guide(REQUESTER);
            return true;
        }
        let rows = self.merequester_rows(view);
        let (offset, lines) = (view.offset, view.lines.clone());
        if view.terminal {
            let on_search = inside(SEARCH, at);
            let clear = on_search && right && !view.search.value.is_empty();
            if let Some(view) = self.ae2x.merequester.client.as_mut() {
                view.search.focused = on_search;
                if on_search {
                    view.field = None;
                }
                if clear {
                    view.search.set_value("");
                }
            }
            if clear {
                self.merequester_send("merequester:search", json!({"text": ""}));
            }
            if on_search {
                return true;
            }
        }
        // A press anywhere else leaves the field, which takes the row's values again.
        let mut handled = false;
        let mut focus: Option<(usize, bool, String)> = None;
        let mut send: Option<(&str, Value)> = None;
        for j in 0..rows {
            let line = offset + j;
            let Some(Line::Row { r, i, on, amount, batch, status, fluid, .. }) = lines.get(line) else { continue };
            let y = (j + 1) as f32 * ROW + 1.0;
            if !right && inside([PADDING_X + 2.0, y + 2.0, 14.0, 14.0], at) {
                send = Some(("merequester:state", json!({"r": r, "i": i, "on": !on})));
                handled = true;
            } else if !right && inside([PADDING_X + 146.0, y, 12.0, 12.0], at) {
                let (a, b) = self.merequester_field_values(line, *amount, *batch, *fluid);
                send = Some(("merequester:numbers", json!({"r": r, "i": i, "amount": a.unwrap_or(0), "batch": b.unwrap_or(1)})));
                handled = true;
            } else {
                for (batch_field, value, dx) in [(false, *amount, 38.0), (true, *batch, 92.0)] {
                    if inside([PADDING_X + dx, y, field_width(*fluid), 12.0], at) {
                        handled = true;
                        // A locked row's fields can't be focused.
                        if !locked(status) {
                            focus = Some((line, batch_field, if right { String::new() } else { shown_number(value, *fluid) }));
                        }
                    }
                }
            }
        }
        if let Some(view) = self.ae2x.merequester.client.as_mut() {
            view.field = focus.map(|(line, batch, text)| {
                let mut edit = EditBox::new(0.0, 0.0, FIELD_WIDTH - FIELD_PADDING, 12.0);
                edit.max_length = FIELD_MAX;
                edit.bordered = false;
                edit.set_value(&text);
                edit.focused = true;
                Field { line, batch, edit }
            });
        }
        if let Some(view) = self.ae2x.merequester.client.as_mut() {
            if let Some((channel, value)) = &send {
                if *channel == "merequester:state" {
                    // `request.updateState` first: no flicker before the server answers.
                    if let Some(Line::Row { on, .. }) = view.lines.iter_mut().find(|l| matches!(l, Line::Row { r, i, .. } if json!(r) == value["r"] && json!(i) == value["i"])) {
                        *on = !*on;
                    }
                }
            }
        }
        if let Some((channel, value)) = send {
            self.audio.play_button_click();
            self.merequester_send(channel, value);
        }
        if !handled && inside([175.0, 18.0, 12.0, rows as f32 * ROW + 1.0], at) {
            let max = lines.len().saturating_sub(rows);
            let to = (((at[1] - 18.0) / (rows as f32 * ROW + 1.0)) * max as f32).round().clamp(0.0, max as f32) as usize;
            self.merequester_scroll_to(to);
            return true;
        }
        handled
    }
    /// The row's amount and batch as its fields hold them now.
    fn merequester_field_values(&self, line: usize, amount: i64, batch: i64, fluid: bool) -> (Option<i64>, Option<i64>) {
        let field = self.ae2x.merequester.client.as_ref().and_then(|v| v.field.as_ref()).filter(|f| f.line == line);
        let value = |is_batch: bool, current: i64| match field.filter(|f| f.batch == is_batch) {
            Some(f) => field_value(&f.edit.value, fluid),
            None => Some(current),
        };
        (value(false, amount), value(true, batch))
    }
    fn merequester_scroll_to(&mut self, offset: usize) {
        let Some(view) = self.ae2x.merequester.client.as_mut() else { return };
        if view.offset != offset {
            view.offset = offset;
            view.field = None;
            self.merequester_send("merequester:scroll", json!({"offset": offset}));
        }
    }
    /// The mouse wheel moves the list two lines (`setRange(..., 2)`).
    pub(in crate::game) fn merequester_scroll(&mut self, delta: f32) -> bool {
        let Some(view) = self.merequester_view() else { return false };
        let max = view.lines.len().saturating_sub(self.merequester_rows(view));
        let to = if delta > 0.0 { view.offset.saturating_sub(2) } else { (view.offset + 2).min(max) };
        self.merequester_scroll_to(to);
        true
    }
    pub(in crate::game) fn merequester_field_focused(&self) -> bool {
        self.merequester_view().is_some_and(|v| v.field.is_some() || (v.terminal && v.search.focused))
    }
    /// Typing in a number field; Enter submits it (`ConfirmableTextField`).
    pub(in crate::game) fn merequester_field_input(&mut self, text: Option<&str>, key: Option<winit::keyboard::KeyCode>) {
        let (ctrl, shift) = (self.has_control_down(), self.has_shift_down());
        if let Some(view) = self.ae2x.merequester.client.as_mut().filter(|v| v.terminal && v.search.focused && v.field.is_none()) {
            let before = view.search.value.clone();
            if !key.is_some_and(|key| view.search.key(key, ctrl, shift)) {
                if let Some(text) = text {
                    view.search.insert_text(text);
                }
            }
            let after = view.search.value.clone();
            if after != before {
                self.merequester_send("merequester:search", json!({"text": after}));
            }
            return;
        }
        if key == Some(winit::keyboard::KeyCode::Enter) || key == Some(winit::keyboard::KeyCode::NumpadEnter) {
            let Some(view) = self.ae2x.merequester.client.as_ref() else { return };
            let Some(field) = view.field.as_ref() else { return };
            let Some(Line::Row { r, i, amount, batch, fluid, .. }) = view.lines.get(field.line).cloned() else { return };
            let Some(value) = field_value(&field.edit.value, fluid) else { return };
            // amountFieldSubmitted / batchFieldSubmitted: an unchanged value is only shown again.
            let (amount, batch) = if field.batch { (amount, value.max(1)) } else { (value, batch) };
            let changed = if field.batch { value.max(1) != view.lines.get(field.line).map_or(0, |l| if let Line::Row { batch, .. } = l { *batch } else { 0 }) } else { value != view.lines.get(field.line).map_or(0, |l| if let Line::Row { amount, .. } = l { *amount } else { 0 }) };
            if changed {
                self.merequester_send("merequester:numbers", json!({"r": r, "i": i, "amount": amount, "batch": batch}));
            }
            if let Some(view) = self.ae2x.merequester.client.as_mut() {
                view.field = None;
            }
            return;
        }
        let Some(field) = self.ae2x.merequester.client.as_mut().and_then(|v| v.field.as_mut()) else { return };
        if !key.is_some_and(|key| field.edit.key(key, ctrl, shift)) {
            if let Some(text) = text {
                field.edit.insert_text(text);
            }
        }
    }
    /// `dialog_title` of the screen's style.
    fn merequester_title(&self, view: &View) -> String {
        if view.terminal {
            self.translate("gui.merequester.requester_terminal", "ME Requ. Terminal")
        } else {
            self.translate("block.merequester.requester", "ME Requester")
        }
    }
    /// The widgets' tooltips (`getTooltipMessage`), from the mod's language
    /// files. A locked row's ghost slot says so instead of its item's.
    pub(in crate::game) fn merequester_tooltip(&self, batch: &mut HudBatch) -> bool {
        use horizon_client::text::Text;
        let Some(view) = self.merequester_view() else { return false };
        let (ox, oy) = self.inventory_origin(batch);
        let at = [self.cursor.0 - ox, self.cursor.1 - oy];
        let rows = self.merequester_rows(view);
        let tip = |game: &Self, name: &str, fallback: &str| game.translate(&format!("tooltip.merequester.{name}"), fallback);
        let mut lines: Vec<(Text, u32)> = Vec::new();
        if inside(HELP.map(|n| n as f32), at) {
            let mut help = Vec::new();
            self.ae2_help_tooltip(&mut help);
            lines.extend(help.into_iter().map(|line| (Text::literal(line), u32::MAX)));
        }
        for j in 0..rows {
            let Some(Line::Row { status, fluid, .. }) = view.lines.get(view.offset + j) else { continue };
            let y = (j + 1) as f32 * ROW + 1.0;
            let plain = |text: String| (Text::literal(text), u32::MAX);
            if inside([PADDING_X + 2.0, y + 2.0, 14.0, 14.0], at) {
                lines.push(plain(tip(self, "toggle", "Toggle Request")));
            } else if inside([PADDING_X + 38.0, y, field_width(*fluid), 12.0], at) {
                lines.push(plain(tip(self, "amount", "Amount to Stock")));
            } else if inside([PADDING_X + 92.0, y, field_width(*fluid), 12.0], at) {
                lines.push(plain(tip(self, "batch", "Batch Size")));
            } else if inside([PADDING_X + 146.0, y, 12.0, 12.0], at) {
                lines.push(plain(tip(self, "submit", "Submit Changes")));
            } else if inside([PADDING_X + 39.0, y + 15.0, 118.0, 2.0], at) {
                lines.push(plain(tip(self, "status", "Request Status")));
                let names = [("idle", "Idling", "The Requester is idling when the requested amount is already reached or there is no crafting pattern for this item or fluid."),
                    ("missing", "Missing Ingredients", "The system is missing ingredients to craft the requested item or fluid. The process is continued when the ingredients are available."),
                    ("cpu", "No CPU", "The system can't find a suitable CPU for the craft. The process is continued when a CPU is found."),
                    ("link", "Crafting", "The system is currently crafting the requested item or fluid."),
                    ("export", "Exporting", "The Requester is trying to export the crafting results to the system. If the Requester has this status for a long time, there is no available space for the export.")];
                if self.has_shift_down() {
                    for (name, title, about) in names {
                        lines.push(plain(" ".to_string()));
                        lines.push((Text::literal(tip(self, name, title)), argb(status_colour(name, false))));
                        lines.push(plain(tip(self, &format!("{name}_desc"), about)));
                    }
                } else {
                    let title = names.iter().find(|(name, _, _)| name == status).map_or("Idling", |(_, title, _)| title);
                    lines.push((Text::literal(tip(self, status, title)), argb(status_colour(status, false))));
                    lines.push(plain(" ".to_string()));
                    lines.push(plain(self.translate("tooltip.merequester.shift_for_more", "%s for more").replace("%s", "Shift")));
                }
            } else if inside([ROW + PADDING_X, (j + 1) as f32 * ROW + 2.0, 16.0, 16.0], at) && locked(status) {
                lines.push((Text::literal(tip(self, "locked", "Locked! Request in progress.")), argb(0xFF5555)));
            }
        }
        if lines.is_empty() {
            return false;
        }
        self.draw_tooltip_lines(batch, &lines, self.cursor.0, self.cursor.1);
        true
    }
    /// The screen for the control socket's `screen` reply: where things are.
    pub(in crate::game) fn merequester_agent_value(&self) -> Option<Value> {
        let view = self.merequester_view()?;
        let rows = self.merequester_rows(view);
        let (w, h) = self.merequester_screen_size()?;
        let origin = self.renderer.as_ref().map(|r| {
            let batch = self.screen_batch([r.config.width, r.config.height]);
            batch.centered_origin(w, h)
        });
        let lines: Vec<Value> = view
            .lines
            .iter()
            .enumerate()
            .map(|(n, line)| {
                let shown = (view.offset..view.offset + rows).contains(&n).then(|| n - view.offset);
                let y = shown.map(|j| (j + 1) as f32 * ROW + 1.0);
                match line {
                    Line::Header { name, count } => json!({"header": name, "count": count, "shown": shown}),
                    Line::Row { r, i, on, amount, batch, status, key, fluid } => json!({
                        "requester": r, "row": i, "on": on, "amount": amount, "batch": batch, "status": status, "key": key, "fluid": fluid,
                        "shown": shown, "slot": shown.map(|j| 300 + j),
                        "state_box": y.map(|y| [PADDING_X + 2.0, y + 2.0, 14.0, 14.0]),
                        "amount_field": y.map(|y| [PADDING_X + 38.0, y, field_width(*fluid), 12.0]),
                        "batch_field": y.map(|y| [PADDING_X + 92.0, y, field_width(*fluid), 12.0]),
                        "submit": y.map(|y| [PADDING_X + 146.0, y, 12.0, 12.0]),
                        "status_bar": y.map(|y| [PADDING_X + 39.0, y + 15.0, 118.0, 2.0]),
                    }),
                }
            })
            .collect();
        Some(json!({
            "kind": if view.terminal { "merequester:requester_terminal" } else { "merequester:requester" },
            "title": self.merequester_title(view), "origin": origin.map(|(x, y)| [x, y]), "size": [w, h], "rows": rows, "offset": view.offset,
            "help": HELP,
            "search": view.search.value, "search_focused": view.terminal && view.search.focused,
            "focused": view.field.as_ref().map(|f| json!({"line": f.line, "batch": f.batch, "text": f.edit.value})),
            "lines": lines,
        }))
    }
}

fn inside([x, y, w, h]: [f32; 4], [px, py]: [f32; 2]) -> bool {
    px >= x && px < x + w && py >= y && py < y + h
}
