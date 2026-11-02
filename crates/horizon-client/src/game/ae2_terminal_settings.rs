//! The ME terminals' sub screens: `TerminalSettingsScreen` (the toolbar's
//! cog) and `KeyTypeSelectionScreen` (a wireless terminal's visible key
//! types), AE2 19.2.18, laid out by `terminals/terminal_settings.json` and
//! `key_type_selection.json`. Like AE2's they cover the terminal: the menu
//! stays open underneath and its places are hidden.
use super::{
    Game,
    ae2_client_config::{self, ClientSettings},
    ae2_screens::{argb, inside},
};
use horizon_client::{
    ae2_screen_style::{Image, ScreenStyle},
    hud::{HudBatch, rgba},
};
use std::sync::OnceLock;

/// `AECheckbox.SIZE`.
const CHECK_SIZE: i32 = 14;
/// `KeyTypeCheckboxes`: the room a checkbox takes before its label's width,
/// and the padding between them.
const KEY_TYPE_LABEL_ROOM: i32 = 24;
const KEY_TYPE_PADDING: i32 = 6;
/// `BackgroundGenerator`: a 256 x 256 sheet with a border of 4.
const BACKGROUND_SIZE: i32 = 256;
const BACKGROUND_BORDER: i32 = 4;
/// The name AE2 uses for an external item list without one installed.
const EXTERNAL_SEARCH: &str = "REI/EMI";
/// `Icon.BACK`.
const ICON_BACK: (i32, i32) = (96, 16);

/// Which sub screen covers the terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Sub {
    Settings,
    KeyTypes,
}

/// A checkbox or radio button of a sub screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Check {
    PinAutoCrafted,
    NotifyFinishedJobs,
    ClearGridOnClose,
    UseInternalSearch,
    UseExternalSearch,
    RememberSearch,
    AutoFocus,
    SyncWithExternal,
    ClearExternal,
    Items,
    Fluids,
}

/// A checkbox as the screen shows it.
#[derive(Clone, Debug)]
struct CheckBox {
    check: Check,
    rect: [i32; 4],
    label: String,
    selected: bool,
    active: bool,
    radio: bool,
}

fn styles(resources: &std::path::Path) -> &'static [Option<ScreenStyle>; 2] {
    static STYLES: OnceLock<[Option<ScreenStyle>; 2]> = OnceLock::new();
    STYLES.get_or_init(|| {
        let screens = resources.join("assets/ae2/screens");
        ["terminals/terminal_settings.json", "key_type_selection.json"].map(|path| {
            ScreenStyle::load(&screens, path).map_err(|error| log::warn!("AE2 terminal style {path}: {error}")).ok()
        })
    })
}

/// `TerminalSettingsScreen.save` for a press of `check`: the changed settings.
fn toggled(settings: ClientSettings, check: Check) -> ClientSettings {
    let mut next = settings;
    match check {
        Check::PinAutoCrafted => next.pin_auto_crafted_items ^= true,
        Check::NotifyFinishedJobs => next.notify_finished_jobs ^= true,
        Check::ClearGridOnClose => next.clear_grid_on_close ^= true,
        Check::UseInternalSearch => next.use_external_search = false,
        Check::UseExternalSearch => next.use_external_search = true,
        Check::RememberSearch => next.remember_last_search ^= true,
        Check::AutoFocus => next.auto_focus_search ^= true,
        Check::SyncWithExternal => next.sync_with_external_search ^= true,
        Check::ClearExternal => next.clear_external_search_on_open ^= true,
        Check::Items | Check::Fluids => {}
    }
    next
}

/// Writes what differs between two settings to `ae2-client.toml`.
fn save_changes(old: ClientSettings, new: ClientSettings) {
    for (section, name, before, after) in [
        ("terminals", "pinAutoCraftedItems", old.pin_auto_crafted_items, new.pin_auto_crafted_items),
        ("client", "notifyForFinishedCraftingJobs", old.notify_finished_jobs, new.notify_finished_jobs),
        ("terminals", "clearGridOnClose", old.clear_grid_on_close, new.clear_grid_on_close),
        ("search", "useExternalSearch", old.use_external_search, new.use_external_search),
        ("search", "rememberLastSearch", old.remember_last_search, new.remember_last_search),
        ("search", "autoFocusSearch", old.auto_focus_search, new.auto_focus_search),
        ("search", "syncWithExternalSearch", old.sync_with_external_search, new.sync_with_external_search),
        ("search", "clearExternalSearchOnOpen", old.clear_external_search_on_open, new.clear_external_search_on_open),
    ] {
        if before != after {
            ae2_client_config::save_setting(section, name, toml::Value::Boolean(after));
        }
    }
}

/// `Font.split`: `text` in lines at most `max` wide.
fn split_lines(atlas: &horizon_client::hud::HudAtlas, text: &str, max: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split(' ') {
        let candidate = if current.is_empty() { word.to_owned() } else { format!("{current} {word}") };
        if !current.is_empty() && HudBatch::text_width(atlas, &candidate) > max {
            lines.push(std::mem::replace(&mut current, word.to_owned()));
        } else {
            current = candidate;
        }
    }
    lines.push(current);
    lines
}

impl Game<'_> {
    /// The sub screen covering the open terminal screen: the server's
    /// key-type editor, or the settings the cog opened.
    pub(super) fn ae2_term_sub(&self) -> Option<Sub> {
        let state = &self.remote.as_ref()?.ae2_term;
        if state.key_types_open() {
            Some(Sub::KeyTypes)
        } else {
            state.sub_settings.map(|_| Sub::Settings)
        }
    }

    fn ae2_term_sub_style(&self, sub: Sub) -> Option<&'static ScreenStyle> {
        styles(&self.options.resources)[if sub == Sub::Settings { 0 } else { 1 }].as_ref()
    }

    /// The sub screen's size: its generated background's. The key-type
    /// screen's height follows its checkboxes (`KeyTypeCheckboxes.populateScreen`).
    pub(super) fn ae2_term_sub_size(&self) -> Option<(i32, i32)> {
        let sub = self.ae2_term_sub()?;
        let (width, height) = self.ae2_term_sub_style(sub)?.size();
        if sub == Sub::Settings {
            return Some((width, height));
        }
        let top = self.ae2_term_sub_style(sub)?.widgets.get("keytypes").map_or(20, |w| w.position.resolve(width, 0).1);
        Some((width, top + 2 * (CHECK_SIZE + KEY_TYPE_PADDING) + KEY_TYPE_PADDING))
    }

    /// The back button (`TabButton` with `Icon.BACK`).
    fn ae2_term_sub_back(&self, sub: Sub) -> Option<[i32; 4]> {
        let style = self.ae2_term_sub_style(sub)?;
        let (w, h) = self.ae2_term_sub_size()?;
        let widget = style.widgets.get("back")?;
        let (x, y) = widget.position.resolve(w, h);
        Some([x, y, widget.width.max(20), widget.height.max(20)])
    }

    /// The checkboxes the screen shows now.
    fn ae2_term_sub_checks(&self, sub: Sub) -> Vec<CheckBox> {
        let Some(style) = self.ae2_term_sub_style(sub) else {
            return Vec::new();
        };
        let Some((w, h)) = self.ae2_term_sub_size() else {
            return Vec::new();
        };
        let tip = |key: &str| self.translate(key, key);
        let place = |name: &str| {
            let widget = style.widgets.get(name)?;
            let (x, y) = widget.position.resolve(w, h);
            Some([x, y, widget.width, CHECK_SIZE])
        };
        let mut boxes = Vec::new();
        if sub == Sub::KeyTypes {
            // KeyTypeSelectionScreen.updateBeforeRender: the last selected
            // type can't be turned off.
            let types = self.ae2_term_key_types().unwrap_or([false, false]);
            let selected = types.iter().filter(|on| **on).count();
            let (x, top) = style.widgets.get("keytypes").map_or((10, 20), |w| w.position.resolve(w.width, 0));
            for (i, (check, key)) in [(Check::Items, "gui.ae2.Items"), (Check::Fluids, "gui.ae2.Fluids")].into_iter().enumerate() {
                let label = tip(key);
                let width = KEY_TYPE_LABEL_ROOM + HudBatch::text_width(&self.gui.atlas, &label) as i32;
                boxes.push(CheckBox {
                    check,
                    rect: [x, top + i as i32 * (CHECK_SIZE + KEY_TYPE_PADDING), width, CHECK_SIZE],
                    label,
                    selected: types[i],
                    active: !(selected == 1 && types[i]),
                    radio: false,
                });
            }
            return boxes;
        }
        let Some(settings) = self.remote.as_ref().and_then(|r| r.ae2_term.sub_settings) else {
            return boxes;
        };
        let external = |key: &str| tip(key).replace("%s", EXTERNAL_SEARCH);
        let internal = !settings.use_external_search;
        let entries = [
            (Check::PinAutoCrafted, "pinAutoCraftedItemsCheckbox", tip("gui.ae2.TerminalSettingsPinAutoCraftedItems"), settings.pin_auto_crafted_items, true, true, false),
            (Check::NotifyFinishedJobs, "notifyForFinishedCraftingJobsCheckbox", tip("gui.ae2.TerminalSettingsNotifyForFinishedJobs"), settings.notify_finished_jobs, true, true, false),
            (Check::ClearGridOnClose, "clearGridOnCloseCheckbox", tip("gui.ae2.TerminalSettingsClearGridOnClose"), settings.clear_grid_on_close, true, true, false),
            (Check::UseInternalSearch, "useInternalSearchRadio", tip("gui.ae2.SearchSettingsUseInternalSearch"), internal, true, true, true),
            // No item list mod here: AE2 shows what would be possible, disabled.
            (Check::UseExternalSearch, "useExternalSearchRadio", external("gui.ae2.SearchSettingsUseExternalSearch"), !internal, true, false, true),
            (Check::RememberSearch, "rememberCheckbox", tip("gui.ae2.SearchSettingsRememberSearch"), settings.remember_last_search, internal, true, false),
            (Check::AutoFocus, "autoFocusCheckbox", tip("gui.ae2.SearchSettingsAutoFocus"), settings.auto_focus_search, internal, true, false),
            (Check::SyncWithExternal, "syncWithExternalCheckbox", external("gui.ae2.SearchSettingsSyncWithExternal"), settings.sync_with_external_search, internal, true, false),
            (Check::ClearExternal, "clearExternalCheckbox", external("gui.ae2.SearchSettingsClearExternal"), settings.clear_external_search_on_open, !internal, true, false),
        ];
        for (check, name, label, selected, visible, active, radio) in entries {
            if let (true, Some(rect)) = (visible, place(name)) {
                boxes.push(CheckBox { check, rect, label, selected, active, radio });
            }
        }
        boxes
    }

    /// A press on a sub screen; it takes every press inside it.
    pub(super) fn ae2_term_sub_press(&mut self, sub: Sub, at: [f32; 2], right: bool) -> bool {
        if right {
            return true;
        }
        if self.ae2_term_sub_back(sub).is_some_and(|rect| inside(rect, at)) {
            self.audio.play_button_click();
            match sub {
                Sub::KeyTypes => self.ae2_send_menu_button(2, 0),
                Sub::Settings => {
                    if let Some(r) = self.remote.as_mut() {
                        r.ae2_term.sub_settings = None;
                    }
                }
            }
            return true;
        }
        let pressed = self.ae2_term_sub_checks(sub).into_iter().find(|c| c.active && inside(c.rect, at));
        let Some(pressed) = pressed else {
            return true;
        };
        self.audio.play_button_click();
        match pressed.check {
            // KeyTypeSelectionMenu.selectKeyType: the menu's places 0 and 1.
            Check::Items => self.ae2_send_menu_button(0, 0),
            Check::Fluids => self.ae2_send_menu_button(1, 0),
            check => {
                if let Some(r) = self.remote.as_mut()
                    && let Some(old) = r.ae2_term.sub_settings
                {
                    let new = toggled(old, check);
                    r.ae2_term.sub_settings = Some(new);
                    save_changes(old, new);
                }
            }
        }
        true
    }

    /// The sub screen: `BackgroundGenerator`, its labels, back button and
    /// checkboxes (`AECheckbox.renderWidget`).
    pub(super) fn ae2_draw_term_sub(&self, batch: &mut HudBatch, sub: Sub, ox: f32, oy: f32) {
        let (Some(style), Some((w, h))) = (self.ae2_term_sub_style(sub), self.ae2_term_sub_size()) else {
            return;
        };
        self.ae2_draw_generated_background(batch, ox, oy, w, h);
        let title = (sub == Sub::KeyTypes).then(|| ("dialog_title", self.translate("gui.ae2.ConfigureVisibleTypes", "Configure Visible Types")));
        self.ae2_draw_texts(batch, ox, oy, style, title.as_slice());
        if let Some(rect) = self.ae2_term_sub_back(sub) {
            self.ae2_draw_tab(batch, ox, oy, rect, ICON_BACK);
        }
        let image = Image { texture: "guis/checkbox".into(), texture_size: (64, 64), src: None };
        let mouse = [self.cursor.0 - ox, self.cursor.1 - oy];
        for check in self.ae2_term_sub_checks(sub) {
            let hover = inside(check.rect, mouse);
            let src = match (check.radio, check.selected, hover) {
                (true, selected, hover) => [CHECK_SIZE * if hover { 3 } else { 2 }, if selected { CHECK_SIZE } else { 0 }, CHECK_SIZE, CHECK_SIZE],
                (false, selected, hover) => [if hover { 22 } else { 0 }, if selected { 40 } else { 28 }, 22, 12],
            };
            let tint = if check.active { u32::MAX } else { rgba(255, 255, 255, 128) };
            let (x, y) = (ox + check.rect[0] as f32, oy + check.rect[1] as f32);
            self.ae2_blit_to(batch, &image, src, [x, y, src[2] as f32, src[3] as f32], tint);
            let color = argb(style.color(if check.active { "DEFAULT_TEXT_COLOR" } else { "MUTED_TEXT_COLOR" }));
            let lines = split_lines(&self.gui.atlas, &check.label, (check.rect[2] - 22) as f32);
            let mut line_y = y + if lines.len() <= 1 { 4.0 } else { 1.0 };
            let text_x = x + if check.radio { 16.0 } else { 26.0 };
            for line in lines {
                batch.text(&self.gui.atlas, &line, text_x, line_y, color, false);
                line_y += 9.0;
            }
        }
    }

    /// `BackgroundGenerator.draw`: the sheet's corners, its tiled edges and
    /// middle.
    fn ae2_draw_generated_background(&self, batch: &mut HudBatch, ox: f32, oy: f32, w: i32, h: i32) {
        const B: i32 = BACKGROUND_BORDER;
        const TILE: i32 = BACKGROUND_SIZE - 2 * B;
        if w < 2 * B || h < 2 * B {
            return;
        }
        let image = Image { texture: "guis/background".into(), texture_size: (BACKGROUND_SIZE, BACKGROUND_SIZE), src: None };
        let at = |x: i32, y: i32| (ox + x as f32, oy + y as f32);
        let mut blit = |src: [i32; 4], x: i32, y: i32| {
            let (dx, dy) = at(x, y);
            self.ae2_blit(batch, &image, src, dx, dy);
        };
        let (right, bottom) = (BACKGROUND_SIZE - B, BACKGROUND_SIZE - B);
        blit([0, 0, B, B], 0, 0);
        blit([right, 0, B, B], w - B, 0);
        blit([0, bottom, B, B], 0, h - B);
        blit([right, bottom, B, B], w - B, h - B);
        let (inner_w, inner_h) = (w - 2 * B, h - 2 * B);
        for cx in (0..inner_w).step_by(TILE as usize) {
            let tile_w = TILE.min(inner_w - cx);
            blit([B, 0, tile_w, B], B + cx, 0);
            blit([B, bottom, tile_w, B], B + cx, h - B);
            for cy in (0..inner_h).step_by(TILE as usize) {
                blit([B, B, tile_w, TILE.min(inner_h - cy)], B + cx, B + cy);
            }
        }
        for cy in (0..inner_h).step_by(TILE as usize) {
            let tile_h = TILE.min(inner_h - cy);
            blit([0, B, B, tile_h], 0, B + cy);
            blit([right, B, B, tile_h], w - B, B + cy);
        }
    }

    /// The sub screen for the control socket's `screen`: its checkboxes'
    /// boxes in GUI pixels and how they stand.
    pub(super) fn ae2_term_sub_agent_value(&self, sub: Sub, origin: (f32, f32)) -> Option<serde_json::Value> {
        let rect = |[x, y, w, h]: [i32; 4]| [origin.0 + x as f32, origin.1 + y as f32, w as f32, h as f32];
        let checks: Vec<serde_json::Value> = self
            .ae2_term_sub_checks(sub)
            .into_iter()
            .map(|c| serde_json::json!({"check": format!("{:?}", c.check), "rect": rect(c.rect), "selected": c.selected, "active": c.active, "radio": c.radio, "label": c.label}))
            .collect();
        let (w, h) = self.ae2_term_sub_size()?;
        Some(serde_json::json!({
            "sub": format!("{sub:?}"),
            "size": [w, h],
            "back": self.ae2_term_sub_back(sub).map(rect),
            "checks": checks,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_press_changes_only_its_setting_and_the_radios_choose() {
        let settings = ClientSettings::default();
        let pinned = toggled(settings, Check::PinAutoCrafted);
        assert_eq!(pinned.pin_auto_crafted_items, !settings.pin_auto_crafted_items);
        assert_eq!(ClientSettings { pin_auto_crafted_items: settings.pin_auto_crafted_items, ..pinned }, settings);
        // A radio button sets its side, twice is the same.
        let external = toggled(settings, Check::UseExternalSearch);
        assert!(external.use_external_search);
        assert_eq!(toggled(external, Check::UseExternalSearch), external);
        assert!(!toggled(external, Check::UseInternalSearch).use_external_search);
        // Key types are the server's, not client settings.
        assert_eq!(toggled(settings, Check::Fluids), settings);
    }
}
