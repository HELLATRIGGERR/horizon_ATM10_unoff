//! The Advanced Pattern Encoder's screen on this client
//! (`AdvPatternEncoderScreen`, `DirectionInputButton`), drawn over the
//! encoder's window with `screens/adv_pattern_encoder.json` (its background
//! `textures/guis/adv_pattern_encoder.png`, carried under
//! `assets/advanced_ae/ae2/`) and the mod's button textures
//! (`textures/guis/<face>_button[_selected].png`, under
//! `assets/advanced_ae/ae2/textures/advanced_ae/`). Derived from Advanced AE,
//! https://github.com/pedroksl/AdvancedAE commit
//! `c322dad81e6f2ad94cb1fb861e73a7ba68e5907a`, LGPL-3.0; see
//! `ae2/addons/advancedae/`.
//!
//! The list of inputs is the output's direction map, as the menu's
//! `AdvPatternEncoderPacket` sends it: the client reads it from the output
//! slot it was sent. The scroll stays on the client, as the mod's does.
use super::{
    direction_map,
    encoder::{BUTTONS, COLUMNS, HOTKEY, INPUT, OUTPUT},
};
use crate::game::{Game, Screen, ae2_screens::inside};
use horizon_client::{
    ae2_generic_stack::GenericStack,
    ae2_screen_style::{Image, ScreenStyle},
    hud::HudBatch,
    items::ItemStack,
};
use horizon_core::geometry::Direction;

const NAMESPACE: &str = "advanced_ae";
const STYLE: &str = "adv_pattern_encoder.json";
/// `LIST_ANCHOR_X`, `LIST_ANCHOR_Y`; a row is `ROW_HEIGHT` 18 and
/// `ROW_SPACING` 2; `VISIBLE_ROWS`.
const ANCHOR: (i32, i32) = (18, 22);
const ROW: i32 = 20;
const VISIBLE_ROWS: usize = 3;
/// `SLOT_BBOX`: a row's slot in the background texture.
const SLOT_BOX: [i32; 4] = [146, 16, 18, 18];
const BACKGROUND: &str = "guis/adv_pattern_encoder";
/// `DIRECTION_BUTTONS_WIDTH`, `DIRECTION_BUTTONS_HEIGHT`, one pixel apart
/// (`DIRECTION_BUTTONS_OFFSET_X`) right of the row's slot.
const BUTTON: (i32, i32) = (12, 14);
/// `Scrollbar.SMALL`'s handle, and the scrollbar's height
/// (`VISIBLE_ROWS * ROW_HEIGHT + (VISIBLE_ROWS - 1) * ROW_SPACING - 2`).
const HANDLE: (i32, i32) = (7, 15);
const TRACK_HEIGHT: i32 = 56;
/// `setRange(0, rows - VISIBLE_ROWS, 2)`: the wheel and a press past the
/// handle move two rows.
const PAGE: i32 = 2;
/// `getDirButtonTextures` by column.
const TEXTURES: [&str; 7] = ["any", "north", "east", "south", "west", "up", "down"];
/// `getDirButtonText` by column.
const TOOLTIPS: [(&str, &str); 7] = [
    ("gui.tooltips.advanced_ae.AnyButton", "Target the face adjacent to the adv. pattern provider"),
    ("gui.tooltips.advanced_ae.NorthButton", "Target the NORTH face of the machine."),
    ("gui.tooltips.advanced_ae.EastButton", "Target the EAST face of the machine."),
    ("gui.tooltips.advanced_ae.SouthButton", "Target the SOUTH face of the machine."),
    ("gui.tooltips.advanced_ae.WestButton", "Target the WEST face of the machine."),
    ("gui.tooltips.advanced_ae.UpButton", "Target the UP face of the machine."),
    ("gui.tooltips.advanced_ae.DownButton", "Target the DOWN face of the machine."),
];

/// `getSelectedDirButton`: the column highlighted for a row's face.
fn column(face: Option<Direction>) -> usize {
    COLUMNS.iter().position(|c| *c == face).unwrap_or(0)
}

/// A direction button the screen shows: its box from the screen's top left,
/// its row of the list and its column, whether it is the row's face.
pub(in crate::game) struct DirectionButton {
    pub(in crate::game) rect: [i32; 4],
    pub(in crate::game) row: usize,
    pub(in crate::game) column: usize,
    pub(in crate::game) highlighted: bool,
}

/// The buttons of the rows shown from row `first` of `rows` (`drawFG`'s
/// positions: the row's slot, two pixels, then each button a pixel apart).
pub(in crate::game) fn buttons(rows: &[Option<Direction>], first: usize) -> Vec<DirectionButton> {
    let mut out = Vec::new();
    for (i, face) in rows.iter().enumerate().skip(first).take(VISIBLE_ROWS) {
        let y = ANCHOR.1 + 1 + (i - first) as i32 * ROW;
        for col in 0..COLUMNS.len() {
            let x = ANCHOR.0 + 2 + 18 + (col as i32 + 1) + col as i32 * BUTTON.0;
            out.push(DirectionButton { rect: [x, y, BUTTON.0, BUTTON.1], row: i, column: col, highlighted: col == column(*face) });
        }
    }
    out
}

/// The scroll range for `rows` inputs (`getRange`, none below four rows).
fn range(rows: usize) -> i32 {
    (rows as i32 - VISIBLE_ROWS as i32).max(0)
}

impl Game<'_> {
    /// `advanced_ae:encoder_window`: the window this client draws as the encoder.
    pub(in crate::game) fn advancedae_encoder_window_payload(&mut self, data: &[u8]) {
        self.ae2x.advancedae.encoder_window = <[u8; 4]>::try_from(data).ok().map(i32::from_be_bytes);
        self.ae2x.advancedae.encoder_scroll = Default::default();
    }
    /// The encoder's style while its window is open.
    fn advancedae_encoder_style(&self) -> Option<&'static ScreenStyle> {
        let window = self.ae2x.advancedae.encoder_window?;
        let open = self.screen == Screen::Inventory && self.remote.as_ref().is_some_and(|r| r.menu.as_ref().is_some_and(|m| m.0 == window));
        if !open {
            return None;
        }
        self.ae2_addon_style(NAMESPACE, STYLE)
    }
    pub(in crate::game) fn advancedae_encoder_size(&self) -> Option<(f32, f32)> {
        let (w, h) = self.advancedae_encoder_style()?.size();
        Some((w as f32, h as f32))
    }
    /// The player's inventory, then the input and the output (300, 301).
    pub(in crate::game) fn advancedae_encoder_slots(&self) -> Option<Vec<usize>> {
        self.advancedae_encoder_style()?;
        Some((0..36).chain([300 + INPUT, 300 + OUTPUT]).collect())
    }
    pub(in crate::game) fn advancedae_encoder_slot_position(&self, i: usize) -> Option<(f32, f32)> {
        let style = self.advancedae_encoder_style()?;
        let at = match i {
            0..=8 => style.slot("PLAYER_HOTBAR", i),
            9..=35 => style.slot("PLAYER_INVENTORY", i - 9),
            _ if i == 300 + INPUT => style.slot("ENCODED_PATTERN", 0),
            _ if i == 300 + OUTPUT => style.slot("MACHINE_OUTPUT", 0),
            _ => None,
        };
        Some(at.map_or((-1000.0, -1000.0), |(x, y)| (x as f32, y as f32)))
    }
    /// The list: each input key of the output and its face.
    fn advancedae_encoder_rows(&self) -> Vec<(GenericStack, Option<Direction>)> {
        let output = self.container_kind().and_then(|e| e.slots().get(OUTPUT).cloned().flatten());
        output.as_ref().and_then(direction_map).unwrap_or_default()
    }
    /// The scrollbar's box (the style's `scrollbar` widget).
    fn advancedae_encoder_track(&self) -> [i32; 4] {
        let at = self.advancedae_encoder_style().and_then(|style| {
            let (w, h) = style.size();
            Some(style.widgets.get("scrollbar")?.position.resolve(w, h))
        });
        let (x, y) = at.unwrap_or((10, 23));
        [x, y, HANDLE.0, TRACK_HEIGHT]
    }
    /// The first row shown, within the range.
    fn advancedae_encoder_first(&self, rows: usize) -> usize {
        self.ae2x.advancedae.encoder_scroll.current.clamp(0, range(rows)) as usize
    }
    fn advancedae_encoder_buttons(&self) -> Vec<DirectionButton> {
        let rows: Vec<Option<Direction>> = self.advancedae_encoder_rows().into_iter().map(|(_, face)| face).collect();
        buttons(&rows, self.advancedae_encoder_first(rows.len()))
    }

    /// `AdvPatternEncoderScreen`: the background and labels, a slot behind
    /// each row shown (`drawBG`), the row's input and its seven buttons
    /// (`drawFG`), the small scrollbar.
    pub(in crate::game) fn advancedae_draw_encoder(&self, batch: &mut HudBatch, ox: f32, oy: f32) -> bool {
        let Some(style) = self.advancedae_encoder_style() else { return false };
        self.ae2_draw_background(batch, ox, oy, style);
        self.ae2_draw_texts(batch, ox, oy, style, &[]);
        let rows = self.advancedae_encoder_rows();
        let first = self.advancedae_encoder_first(rows.len());
        let background = Image { texture: BACKGROUND.into(), texture_size: (256, 256), src: None };
        for (i, (key, _)) in rows.iter().skip(first).take(VISIBLE_ROWS).enumerate() {
            let (x, y) = (ox + ANCHOR.0 as f32, oy + (ANCHOR.1 + i as i32 * ROW) as f32);
            self.ae2_blit(batch, &background, SLOT_BOX, x, y);
            // `wrapForDisplayOrFilter`: an item itself, a fluid as AE2's wrapped stack.
            let shown = if key.key_type == ae2_cell_core::KeyType::ITEMS {
                horizon_client::ae2_cell_codec::prototype(&key.key).ok()
            } else {
                horizon_client::ae2_generic_stack::wrap(&GenericStack { amount: 0, ..key.clone() }).ok()
            };
            if let Some(stack) = shown {
                self.draw_item(batch, &ItemStack { count: 1, ..stack }, x + 1.0, y + 1.0);
            }
        }
        for b in self.advancedae_encoder_buttons() {
            let texture = format!("advanced_ae/guis/{}_button{}", TEXTURES[b.column], if b.highlighted { "_selected" } else { "" });
            let image = Image { texture: texture.into(), texture_size: (16, 16), src: None };
            let [x, y, w, h] = b.rect;
            self.ae2_blit(batch, &image, [0, 0, w, h], ox + x as f32, oy + y as f32);
        }
        // Scrollbar.drawForegroundLayer: the disabled handle at the top without range.
        let track = self.advancedae_encoder_track();
        let range = range(rows.len());
        let offset = if range > 0 { first as i32 * (track[3] - HANDLE.1) / range } else { 0 };
        let sprite = if range > 0 { "gui/small_scroller" } else { "gui/small_scroller_disabled" };
        let handle = Image { texture: sprite.into(), texture_size: HANDLE, src: None };
        self.ae2_blit(batch, &handle, [0, 0, HANDLE.0, HANDLE.1], ox + track[0] as f32, oy + (track[1] + offset) as f32);
        true
    }

    /// A press at `at`: a direction button (`AdvPatternEncoderChangeDirectionPacket`,
    /// a menu button here) or the scrollbar (`onMouseDown`: past the handle it pages).
    pub(in crate::game) fn advancedae_encoder_press(&mut self, at: [f32; 2], right: bool) -> bool {
        if self.advancedae_encoder_style().is_none() || right {
            return false;
        }
        if let Some(b) = self.advancedae_encoder_buttons().into_iter().find(|b| inside(b.rect, at)) {
            let window = self.remote.as_ref().and_then(|r| r.menu.as_ref()).map(|(w, _)| *w);
            if let Some(container) = window {
                let button = BUTTONS + (b.row * COLUMNS.len() + b.column) as i32;
                self.send(horizon_protocol::packets::play::Serverbound::ContainerButtonClick(
                    horizon_protocol::packets::play::ContainerButtonClick { container, button },
                ));
            }
            self.audio.play_button_click();
            return true;
        }
        let track = self.advancedae_encoder_track();
        if inside(track, at) {
            let range = range(self.advancedae_encoder_rows().len());
            let scroll = &mut self.ae2x.advancedae.encoder_scroll;
            scroll.clamp(range);
            scroll.press([track[0], track[1], track[2], track[3]], range, PAGE, at[1]);
            scroll.drag = None;
            return true;
        }
        false
    }
    /// The wheel anywhere on the screen (`Scrollbar.onMouseWheel`).
    pub(in crate::game) fn advancedae_encoder_scroll(&mut self, delta: f32) -> bool {
        if self.advancedae_encoder_style().is_none() {
            return false;
        }
        let range = range(self.advancedae_encoder_rows().len());
        let scroll = &mut self.ae2x.advancedae.encoder_scroll;
        scroll.clamp(range);
        scroll.wheel(range, PAGE, delta);
        true
    }
    /// A direction button's tooltip (`Tooltip.create(getDirButtonText)`).
    pub(in crate::game) fn advancedae_encoder_tooltip(&self, batch: &mut HudBatch) -> bool {
        if self.advancedae_encoder_style().is_none() {
            return false;
        }
        let (ox, oy) = self.inventory_origin(batch);
        let at = [self.cursor.0 - ox, self.cursor.1 - oy];
        let Some(b) = self.advancedae_encoder_buttons().into_iter().find(|b| inside(b.rect, at)) else { return false };
        let (key, fallback) = TOOLTIPS[b.column];
        let text = self.translate(key, fallback);
        self.draw_tooltip_lines(batch, &[(horizon_client::text::Text::literal(text), u32::MAX)], self.cursor.0, self.cursor.1);
        true
    }
    /// `isCloseHotkey`: the encoder's hotkey closes its screen. True then.
    pub(in crate::game) fn advancedae_encoder_hotkey(&mut self, input: &str) -> bool {
        if self.advancedae_encoder_style().is_none() || self.settings.keys.get(&super::super::hotkey_mapping(HOTKEY)) != input {
            return false;
        }
        self.close_container();
        self.screen = Screen::Playing;
        self.capture(true);
        true
    }
    /// The screen for the control socket's `screen`: each row's input, its
    /// face and its buttons in GUI pixels, and the scroll.
    pub(in crate::game) fn advancedae_encoder_agent(&self) -> Option<serde_json::Value> {
        let (w, h) = self.advancedae_encoder_size()?;
        let (ox, oy) = self.renderer.as_ref().map(|r| self.screen_batch([r.config.width, r.config.height]).centered_origin(w, h))?;
        let rect = |[x, y, bw, bh]: [i32; 4]| [ox + x as f32, oy + y as f32, bw as f32, bh as f32];
        let rows = self.advancedae_encoder_rows();
        let buttons = self.advancedae_encoder_buttons();
        let first = self.advancedae_encoder_first(rows.len());
        let list: Vec<serde_json::Value> = rows
            .iter()
            .enumerate()
            .map(|(i, (key, face))| {
                let shown: Vec<_> = buttons.iter().filter(|b| b.row == i).map(|b| rect(b.rect)).collect();
                serde_json::json!({
                    "id": key.key.item_id,
                    "fluid": key.key_type == ae2_cell_core::KeyType::FLUIDS,
                    "direction": face.map(|f| format!("{f:?}").to_lowercase()),
                    "buttons": shown,
                })
            })
            .collect();
        Some(serde_json::json!({"advae_encoder": {
            "window": self.ae2x.advancedae.encoder_window,
            "size": [w, h],
            "rows": list,
            "first": first,
            "range": range(rows.len()),
            "scrollbar": rect(self.advancedae_encoder_track()),
        }}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `drawFG`: three rows from the scroll, seven buttons each right of the
    /// row's slot, the row's face highlighted.
    #[test]
    fn the_rows_buttons_sit_where_the_mod_puts_them() {
        let rows = [None, Some(Direction::Up), Some(Direction::North), Some(Direction::Down)];
        let shown = buttons(&rows, 0);
        assert_eq!(shown.len(), 21, "three rows of seven");
        assert_eq!(shown[0].rect, [39, 23, 12, 14]);
        assert_eq!(shown[6].rect, [39 + 6 * 13, 23, 12, 14]);
        assert_eq!(shown[7].rect, [39, 43, 12, 14]);
        let highlighted: Vec<(usize, usize)> = shown.iter().filter(|b| b.highlighted).map(|b| (b.row, b.column)).collect();
        assert_eq!(highlighted, [(0, 0), (1, 5), (2, 1)]);
        let scrolled = buttons(&rows, 1);
        assert_eq!((scrolled[0].row, scrolled[0].rect[1]), (1, 23));
        assert_eq!(scrolled.iter().filter(|b| b.highlighted).last().map(|b| (b.row, b.column)), Some((3, 6)));
        assert_eq!((range(2), range(3), range(5)), (0, 0, 2));
    }
}
