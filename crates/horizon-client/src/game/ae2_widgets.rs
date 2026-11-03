//! AE2's small widgets, drawn the way `client/gui/widgets` draws them, for the
//! screens that stand in front of the server's placeholder menus: `AE2Button`,
//! `Scrollbar`, `AETextField` in its `NumberEntryWidget`, the styles' labels and
//! half-size text.
use super::{
    Game,
    ae2_screens::{argb, inside},
};
use horizon_client::{
    ae2_screen_style::{Image, ScreenStyle},
    hud::{HudBatch, rgba},
    ui::EditBox,
};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

/// `Scrollbar.BIG`'s handle.
pub(super) const BIG_HANDLE: (i32, i32) = (12, 15);
/// `AETextField.PADDING`, and the room it keeps for the cursor (`width("_")`).
const FIELD_PADDING: i32 = 2;
const CURSOR_ROOM: i32 = 6;

/// `Scrollbar`'s state: where it is and where the press took the handle.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Scroll {
    pub(super) current: i32,
    pub(super) drag: Option<i32>,
}

impl Scroll {
    fn offset(&self, track: [i32; 4], range: i32) -> i32 {
        if range <= 0 { 0 } else { self.current * (track[3] - BIG_HANDLE.1) / range }
    }

    pub(super) fn clamp(&mut self, range: i32) {
        self.current = self.current.clamp(0, range.max(0));
    }

    /// `onMouseDown`: above or below the handle pages, on it drags.
    pub(super) fn press(&mut self, track: [i32; 4], range: i32, page: i32, y: f32) {
        self.drag = None;
        if range <= 0 {
            return;
        }
        let rel = y as i32 - track[1];
        let offset = self.offset(track, range);
        if rel < offset {
            self.current -= page;
        } else if rel < offset + BIG_HANDLE.1 {
            self.drag = Some(rel - offset);
        } else {
            self.current += page;
        }
        self.clamp(range);
    }

    /// `onMouseDrag`, `y` from the screen's top.
    pub(super) fn dragged(&mut self, track: [i32; 4], range: i32, y: f32) {
        let Some(grab) = self.drag else {
            return;
        };
        let available = (track[3] - BIG_HANDLE.1).max(1) as f32;
        let position = ((y - track[1] as f32 - grab as f32) / available).clamp(0.0, 1.0);
        self.current = (position * range as f32).round() as i32;
        self.clamp(range);
    }

    /// `onMouseWheel`.
    pub(super) fn wheel(&mut self, range: i32, page: i32, delta: f32) {
        if range > 0 {
            self.current += (-delta).clamp(-1.0, 1.0) as i32 * page;
            self.clamp(range);
        }
    }
}

/// The box of the style's widget `name`, `default` giving the size it leaves out.
pub(super) fn widget(style: &ScreenStyle, name: &str, default: (i32, i32)) -> Option<[i32; 4]> {
    let (w, h) = style.size();
    let s = style.widgets.get(name)?;
    let (x, y) = s.position.resolve(w, h);
    Some([x, y, if s.width != 0 { s.width } else { default.0 }, if s.height != 0 { s.height } else { default.1 }])
}

/// The help button, first on the left toolbar (`VerticalButtonBar`), and the
/// `index`th button after it.
pub(super) fn toolbar_button(style: &ScreenStyle, index: i32) -> [i32; 4] {
    let (w, h) = style.size();
    let (x, y) = style.widgets.get("verticalToolbar").map_or((3, 1), |s| s.position.resolve(w, h));
    [x - 2 - 16, y + 2 + index * 22, 16, 16]
}

/// `Tooltips.getAmount` for one number: at most three significant decimals.
pub(super) fn tooltip_number(value: f64) -> String {
    let places = if value < 10.0 {
        3
    } else if value < 100.0 {
        2
    } else {
        1
    };
    let text = format!("{value:.places$}");
    if text.contains('.') { text.trim_end_matches('0').trim_end_matches('.').to_owned() } else { text }
}

/// `Tooltips.getByteAmount`.
pub(super) fn byte_amount(amount: u64) -> String {
    const UNITS: [&str; 6] = ["k", "M", "G", "T", "P", "E"];
    if amount < 1024 {
        return amount.to_string();
    }
    let mut i = 0;
    let mut num = 1024u64;
    while i + 1 < UNITS.len() && amount / num >= 1000 {
        i += 1;
        num *= 1024;
    }
    format!("{}{}", tooltip_number(amount as f64 / num as f64), UNITS[i])
}

/// `NumberFormat.getInstance().format`: thousands with commas.
pub(super) fn grouped(amount: u64) -> String {
    let digits = amount.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

impl Game<'_> {
    /// A screen style by its path under `screens/`, loaded once.
    pub(super) fn ae2_style(&self, path: &'static str) -> Option<&'static ScreenStyle> {
        static STYLES: OnceLock<Mutex<HashMap<&'static str, Option<&'static ScreenStyle>>>> = OnceLock::new();
        let mut styles = STYLES.get_or_init(Default::default).lock().ok()?;
        *styles.entry(path).or_insert_with(|| {
            let screens = self.options.resources.join("assets/ae2/screens");
            ScreenStyle::load(&screens, path)
                .map_err(|error| log::warn!("AE2 screen style {path}: {error}"))
                .ok()
                .map(|style| &*Box::leak(Box::new(style)))
        })
    }

    /// An addon's style: its own file under its namespace (`<ns>/ae2/screens`),
    /// its includes from AE2's (`ScreenStyle::load_over`).
    pub(super) fn ae2_addon_style(&self, namespace: &'static str, path: &'static str) -> Option<&'static ScreenStyle> {
        static STYLES: OnceLock<Mutex<HashMap<(&'static str, &'static str), Option<&'static ScreenStyle>>>> = OnceLock::new();
        let mut styles = STYLES.get_or_init(Default::default).lock().ok()?;
        *styles.entry((namespace, path)).or_insert_with(|| {
            let assets = self.options.resources.join("assets");
            ScreenStyle::load_over(&assets.join(namespace).join("ae2/screens"), &assets.join("ae2/screens"), path)
                .map_err(|error| log::warn!("{namespace} screen style {path}: {error}"))
                .ok()
                .map(|style| &*Box::leak(Box::new(style)))
        })
    }

    /// The style's background at the screen's origin.
    pub(super) fn ae2_draw_background(&self, batch: &mut HudBatch, ox: f32, oy: f32, style: &ScreenStyle) {
        if let Some(background) = style.background.as_ref() {
            let (w, h) = background.size();
            let [sx, sy] = background.src.map_or([0, 0], |[x, y, _, _]| [x, y]);
            self.ae2_blit(batch, background, [sx, sy, w, h], ox, oy);
        }
    }

    /// `BackgroundGenerator.draw`: `guis/background`'s borders round its
    /// tiled middle, for a style's `generatedBackground`.
    pub(super) fn ae2_draw_generated_background(&self, batch: &mut HudBatch, ox: f32, oy: f32, w: i32, h: i32) {
        const SIZE: i32 = 256;
        const BORDER: i32 = 4;
        const TILE: i32 = SIZE - 2 * BORDER;
        if w < 2 * BORDER || h < 2 * BORDER {
            return;
        }
        let image = Image { texture: "guis/background".into(), texture_size: (SIZE, SIZE), src: None };
        let mut blit = |src: [i32; 4], x: i32, y: i32| self.ae2_blit(batch, &image, src, ox + x as f32, oy + y as f32);
        let (inner_w, inner_h) = (w - 2 * BORDER, h - 2 * BORDER);
        blit([0, 0, BORDER, BORDER], 0, 0);
        blit([SIZE - BORDER, 0, BORDER, BORDER], w - BORDER, 0);
        blit([0, SIZE - BORDER, BORDER, BORDER], 0, h - BORDER);
        blit([SIZE - BORDER, SIZE - BORDER, BORDER, BORDER], w - BORDER, h - BORDER);
        for cx in (0..inner_w).step_by(TILE as usize) {
            let tile = TILE.min(inner_w - cx);
            blit([BORDER, 0, tile, BORDER], BORDER + cx, 0);
            blit([BORDER, SIZE - BORDER, tile, BORDER], BORDER + cx, h - BORDER);
            for cy in (0..inner_h).step_by(TILE as usize) {
                blit([BORDER, BORDER, tile, TILE.min(inner_h - cy)], BORDER + cx, BORDER + cy);
            }
        }
        for cy in (0..inner_h).step_by(TILE as usize) {
            let tile = TILE.min(inner_h - cy);
            blit([0, BORDER, BORDER, tile], 0, BORDER + cy);
            blit([SIZE - BORDER, BORDER, BORDER, tile], w - BORDER, BORDER + cy);
        }
    }

    /// `AE2Button`: its sprite, a 200 x 20 nine-slice with 3 pixel borders,
    /// the label centred without a shadow, a pixel up unless hovered, and
    /// lower and dark while inactive. A label too wide scrolls; it is clipped
    /// to its start here.
    pub(super) fn ae2_draw_button(&self, batch: &mut HudBatch, ox: f32, oy: f32, rect: [i32; 4], label: &str, active: bool, mouse: [f32; 2]) {
        let [x, y, w, h] = rect;
        let atlas = &self.gui.atlas;
        let hover = active && inside(rect, mouse);
        let sprite = if !active {
            "gui/button_disabled"
        } else if hover {
            "gui/button_highlighted"
        } else {
            "gui/button"
        };
        let image = Image { texture: sprite.into(), texture_size: (200, 20), src: None };
        self.ae2_blit(batch, &image, [0, 0, w - 3, h], ox + x as f32, oy + y as f32);
        self.ae2_blit(batch, &image, [197, 0, 3, h], ox + (x + w - 3) as f32, oy + y as f32);
        let (min, max) = (x + 2, x + w - 2);
        let mut text = label.to_owned();
        while HudBatch::text_width(atlas, &text) as i32 > max - min && !text.is_empty() {
            text.pop();
        }
        let width = HudBatch::text_width(atlas, &text) as i32;
        let centre = ((min + max) / 2).clamp(min + width / 2, max - width / 2);
        let (lift, color) = if !active {
            (-1, rgba(0x41, 0x3F, 0x54, 255))
        } else if hover {
            (0, rgba(0x51, 0x74, 0x97, 255))
        } else {
            (1, rgba(0xF2, 0xF2, 0xF2, 255))
        };
        let top = y + (h - 9) / 2 + 1 - lift;
        batch.text(atlas, &text, ox + (centre - width / 2) as f32, oy + top as f32, color, false);
    }

    /// The big scrollbar's handle (`drawForegroundLayer`), the disabled one
    /// without range.
    pub(super) fn ae2_draw_scroller(&self, batch: &mut HudBatch, ox: f32, oy: f32, track: [i32; 4], range: i32, scroll: Scroll) {
        let sprite = if range <= 0 { "gui/big_scroller_disabled" } else { "gui/big_scroller" };
        let image = Image { texture: sprite.into(), texture_size: BIG_HANDLE, src: None };
        let at = oy + (track[1] + scroll.offset(track, range)) as f32;
        self.ae2_blit(batch, &image, [0, 0, BIG_HANDLE.0, BIG_HANDLE.1], ox + track[0] as f32, at);
    }

    /// Where `AETextField` takes a press and where its text goes.
    pub(super) fn ae2_field_rects(rect: [i32; 4]) -> ([f32; 4], [f32; 4]) {
        let [x, y, w, h] = rect;
        let text = [x + FIELD_PADDING, y + FIELD_PADDING, w - 2 * FIELD_PADDING - CURSOR_ROOM, h - 2 * FIELD_PADDING];
        (rect.map(|v| v as f32), text.map(|v| v as f32))
    }

    /// `AETextField`: its background in three slices, the focused row while
    /// focused, its text in the text colour or the error colour.
    pub(super) fn ae2_draw_field(&self, batch: &mut HudBatch, ox: f32, oy: f32, style: &ScreenStyle, rect: [i32; 4], text: Option<[f32; 4]>, mut field: EditBox, valid: bool) {
        let row = if field.focused { 24 } else { 0 };
        let image = Image { texture: "guis/text_field".into(), texture_size: (128, 128), src: None };
        let (r, whole) = Self::ae2_field_rects(rect);
        let text = text.unwrap_or(whole);
        let (x, y, w) = (ox + r[0], oy + r[1], r[2]);
        let middle = (w - 2.0).min(126.0);
        self.ae2_blit(batch, &image, [0, row, 1, 12], x, y);
        self.ae2_blit(batch, &image, [1, row, middle as i32, 12], x + 1.0, y);
        self.ae2_blit(batch, &image, [127, row, 1, 12], x + w - 1.0, y);
        field.bordered = false;
        field.text_color = argb(style.color(if valid { "TEXTFIELD_TEXT" } else { "TEXTFIELD_ERROR" }));
        [field.x, field.y, field.width, field.height] = [ox + text[0], oy + text[1], text[2], text[3]];
        field.draw(batch, &self.gui.atlas, None);
    }

    /// Text at `scale`, its top left at `x`, `y` from the screen's origin, as
    /// a scaled pose places it (the position in scaled pixels, truncated).
    pub(super) fn ae2_small_text(&self, batch: &mut HudBatch, ox: f32, oy: f32, text: &str, x: f32, y: f32, scale: f32, color: u32) {
        let start = batch.vertices.len();
        batch.text(&self.gui.atlas, text, ox + (x / scale).trunc(), oy + (y / scale).trunc(), color, false);
        batch.scale_since(start, ox, oy, scale, scale);
    }

    /// An item scaled about its top left (`InfoBar`'s `StackWidget`).
    pub(super) fn ae2_small_item(&self, batch: &mut HudBatch, stack: &horizon_client::items::ItemStack, x: f32, y: f32, scale: f32) {
        let start = batch.vertices.len();
        self.draw_item_model(batch, stack, x, y);
        batch.scale_since(start, x, y, scale, scale);
    }

    /// A `click-slot` on a place of the crafting screens' menus: a plain click
    /// on the place itself, as the menu has always taken it. A press on the
    /// screen's own buttons is a `click-at`. False when no such screen shows.
    pub(super) fn ae2_hidden_click(&mut self, slot: usize, right: bool, shift: bool) -> bool {
        let hidden = self.ae2_craft_screen() != 0 || self.ae2_stock_ui() || ((self.ae2_part_open() || self.ae2_status_open() || self.ae2_workbench_open()) && self.slot_position(slot).0 < -999.0);
        if !hidden || !self.slots().contains(&slot) {
            return false;
        }
        let input = if shift { horizon_client::menu::Input::QuickMove } else { horizon_client::menu::Input::Pickup };
        if self.remote.is_some() {
            self.remote_menu_ops(&[(Some(slot), i32::from(right), input)]);
        } else {
            self.menu_click(Some(slot), i32::from(right), input);
        }
        true
    }

    /// The tooltip `lines` at the cursor.
    pub(super) fn ae2_tooltip_lines(&self, batch: &mut HudBatch, lines: Vec<String>) {
        if lines.is_empty() {
            return;
        }
        let lines: Vec<(horizon_client::text::Text, u32)> =
            lines.into_iter().map(|l| (horizon_client::text::Text::literal(l).resolve_legacy(), u32::MAX)).collect();
        self.draw_tooltip_lines(batch, &lines, self.cursor.0, self.cursor.1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRACK: [i32; 4] = [218, 19, 12, 137];

    #[test]
    fn the_scrollbar_pages_drags_and_wheels_within_its_range() {
        let mut scroll = Scroll::default();
        // On the handle at the top: a drag.
        scroll.press(TRACK, 10, 1, 21.0);
        assert_eq!(scroll.drag, Some(2));
        scroll.dragged(TRACK, 10, 21.0 + 61.0);
        assert_eq!(scroll.current, 5);
        scroll.dragged(TRACK, 10, 1000.0);
        assert_eq!(scroll.current, 10);
        scroll.drag = None;
        // Above the handle, then below it: a page each.
        scroll.press(TRACK, 10, 2, 20.0);
        assert_eq!(scroll.current, 8);
        scroll.press(TRACK, 10, 2, 150.0);
        assert_eq!(scroll.current, 10);
        // The wheel steps a page, against its direction.
        scroll.wheel(10, 1, 1.0);
        assert_eq!(scroll.current, 9);
        scroll.wheel(10, 1, -1.0);
        assert_eq!(scroll.current, 10);
        scroll.wheel(10, 1, -1.0);
        assert_eq!(scroll.current, 10);
        // Without range nothing moves.
        let mut idle = Scroll::default();
        idle.press(TRACK, 0, 1, 50.0);
        idle.wheel(0, 1, -1.0);
        assert_eq!((idle.current, idle.drag), (0, None));
    }

    #[test]
    fn numbers_read_as_the_tooltips_print_them() {
        assert_eq!(byte_amount(1023), "1023");
        assert_eq!(byte_amount(4096), "4k");
        assert_eq!(byte_amount(65536), "64k");
        assert_eq!(byte_amount(1024 * 1024), "1M");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1_000), "1,000");
        assert_eq!(grouped(12_345_678), "12,345,678");
        assert_eq!(tooltip_number(12.5), "12.5");
    }
}
