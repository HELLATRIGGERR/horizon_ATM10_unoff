//! AE2WTLib's `MagnetScreen` on the client (`screens/wtlib/magnet.json`, its
//! background `textures/wtlib/guis/magnet.png` and the API's `icons.png`,
//! carried under `assets/ae2wtlib/ae2/`): the magnet's pickup and insert
//! filters, 27 fake slots each, the player's inventory under them, and the
//! five buttons (`togglepickupmode`, `toggleinsertmode`, `copy_up`,
//! `copy_down`, `switch`). It covers the wireless crafting terminal as AE2's
//! sub screens do, the terminal's menu open underneath standing for
//! `MagnetMenu`'s; the server keeps the filters in the item
//! (`ae2wtlib:magnet_menu`). AE2WTLib 19.5.1 by Mari023,
//! https://github.com/Mari023/AE2WirelessTerminalLibrary tag `19.5.1`, MIT;
//! see `ae2/addons/ae2wtlib/`.
use crate::game::{Game, ae2_screens::inside};
use horizon_client::{
    ae2_screen_style::{Image, ScreenStyle},
    hud::HudBatch,
    items::ItemStack,
};

/// The screen's style, under AE2WTLib's namespace.
pub(in crate::game) const STYLE: &str = "wtlib/magnet.json";
/// `Icon.TEXTURE`, 128 x 128.
const ICONS: &str = "wtlib/guis/icons";
/// `Icon`s: `MAGNET`, `NO`, `YES`, `UP`, `DOWN`, `SWITCH`, and
/// `BUTTON_BACKGROUND` (16 x 17) and its hovered form (16 x 16).
const ICON_MAGNET: (i32, i32) = (0, 0);
const ICON_NO: (i32, i32) = (32, 0);
const ICON_YES: (i32, i32) = (32, 16);
const ICON_UP: (i32, i32) = (32, 32);
const ICON_DOWN: (i32, i32) = (32, 48);
const ICON_SWITCH: (i32, i32) = (32, 64);
const ICON_TRASH: (i32, i32) = (0, 32);
const BUTTON_BACKGROUND: [i32; 4] = [63, 0, 16, 17];
const BUTTON_BACKGROUND_HOVERED: [i32; 4] = [95, 1, 16, 16];
/// The two filters' slot semantics (`AE2wtlibSlotSemantics`).
const FILTERS: [&str; 2] = ["AE2WTLIB_PICKUP_CONFIG", "AE2WTLIB_INSERT_CONFIG"];
/// The buttons: the style's widget, the action byte.
const BUTTONS: [(&str, u8); 5] = [("pickup_mode", 0), ("insert_mode", 1), ("copy_up", 2), ("copy_down", 3), ("switch", 4)];

/// What the server shows of the magnet's filters.
struct Filters {
    slots: [Vec<Option<String>>; 2],
    allow: [bool; 2],
}

/// A part of the screen a press can take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    /// A filter (0 pickup, 1 insert) and its slot.
    Filter(u8, u8),
    /// A button, by its action byte.
    Button(u8),
}

fn filters(view: &serde_json::Value) -> Option<Filters> {
    let f = view.get("filters").filter(|f| f.is_object())?;
    let list = |key: &str| -> Vec<Option<String>> {
        f.get(key).and_then(|l| l.as_array()).map_or_else(Vec::new, |l| l.iter().take(27).map(|v| v.as_str().map(str::to_owned)).collect())
    };
    let flag = |key: &str| f.get(key).and_then(serde_json::Value::as_bool).unwrap_or(false);
    Some(Filters { slots: [list("pickup"), list("insert")], allow: [flag("pickup_allow"), flag("insert_allow")] })
}

impl Game<'_> {
    fn ae2wtlib_magnet_filters(&self) -> Option<Filters> {
        filters(self.ae2_term_addon_view()?)
    }
    /// `WCTScreen.setMagnetModeText`: the Magnet Filter button shows while
    /// the terminal has the card.
    pub(in crate::game) fn ae2wtlib_magnet_button(&self) -> bool {
        let Some(view) = self.ae2_term_addon_view() else { return false };
        let mode = view.get("magnet").and_then(serde_json::Value::as_i64).unwrap_or(-1);
        mode >= 0 && filters(view).is_some()
    }
    pub(in crate::game) fn ae2wtlib_magnet_style(&self) -> Option<&'static ScreenStyle> {
        self.ae2_addon_style("ae2wtlib", STYLE)
    }
    /// The parts, from the screen's top left.
    fn ae2wtlib_magnet_parts(&self) -> Vec<(Part, [i32; 4])> {
        let Some(style) = self.ae2wtlib_magnet_style() else { return Vec::new() };
        let (w, h) = style.size();
        let mut parts = Vec::new();
        for (which, semantic) in FILTERS.iter().enumerate() {
            for index in 0..27u8 {
                if let Some((x, y)) = style.slot(semantic, usize::from(index)) {
                    parts.push((Part::Filter(which as u8, index), [x, y, 16, 16]));
                }
            }
        }
        for (name, action) in BUTTONS {
            if let Some(widget) = style.widgets.get(name) {
                let (x, y) = widget.position.resolve(w, h);
                parts.push((Part::Button(action), [x, y, 16, 16]));
            }
        }
        parts
    }
    /// Where the player's inventory place `i` goes (0..9 the hotbar).
    pub(in crate::game) fn ae2wtlib_magnet_slot_position(&self, i: usize) -> Option<(f32, f32)> {
        let style = self.ae2wtlib_magnet_style()?;
        let (x, y) = match i {
            0..=8 => style.slot("PLAYER_HOTBAR", i)?,
            9..=35 => style.slot("PLAYER_INVENTORY", i - 9)?,
            _ => return None,
        };
        Some((x as f32, y as f32))
    }
    /// A press on the screen: a filter slot takes the cursor's item (or is
    /// cleared by an empty one), a button sends its action. False on the
    /// player's places, which the menu takes.
    pub(in crate::game) fn ae2wtlib_magnet_press(&mut self, at: [f32; 2]) -> bool {
        if (0..36).any(|i| self.ae2wtlib_magnet_slot_position(i).is_some_and(|(x, y)| inside([x as i32, y as i32, 16, 16], at))) {
            return false;
        }
        let Some((part, _)) = self.ae2wtlib_magnet_parts().into_iter().find(|(_, r)| inside(*r, at)) else { return true };
        let data = match part {
            Part::Filter(which, index) => vec![5, which, index],
            Part::Button(action) => {
                self.audio.play_button_click();
                vec![action]
            }
        };
        self.ae2_send_payload(super::magnet::MAGNET_MENU, data);
        true
    }
    /// `MagnetScreen`: the background, the filters' items and the buttons.
    pub(in crate::game) fn ae2wtlib_draw_magnet(&self, batch: &mut HudBatch, ox: f32, oy: f32) {
        let Some(style) = self.ae2wtlib_magnet_style() else { return };
        self.ae2_draw_background(batch, ox, oy, style);
        self.ae2_draw_texts(batch, ox, oy, style, &[]);
        let filters = self.ae2wtlib_magnet_filters();
        let icons = Image { texture: ICONS.into(), texture_size: (128, 128), src: None };
        let mouse = [self.cursor.0 - ox, self.cursor.1 - oy];
        for (part, rect) in self.ae2wtlib_magnet_parts() {
            let (x, y) = (ox + rect[0] as f32, oy + rect[1] as f32);
            match part {
                Part::Filter(which, index) => {
                    let item = filters.as_ref().and_then(|f| f.slots[usize::from(which)].get(usize::from(index)).cloned().flatten());
                    if let Some(item) = item {
                        self.draw_item(batch, &ItemStack::new(&item, 1), x, y);
                    }
                }
                Part::Button(action) => {
                    // IconButton.renderWidget: the backdrop a pixel left, a
                    // pixel lower while hovered, the icon over it.
                    let hover = inside(rect, mouse);
                    let lift = if hover { 1.0 } else { 0.0 };
                    let bg = if hover { BUTTON_BACKGROUND_HOVERED } else { BUTTON_BACKGROUND };
                    self.ae2_blit(batch, &icons, bg, x - 1.0, y + lift);
                    let allow = |which: usize| filters.as_ref().is_some_and(|f| f.allow[which]);
                    let (sx, sy) = match action {
                        0 | 1 if allow(usize::from(action)) => ICON_YES,
                        0 | 1 => ICON_NO,
                        2 => ICON_UP,
                        3 => ICON_DOWN,
                        _ => ICON_SWITCH,
                    };
                    self.ae2_blit(batch, &icons, [sx, sy, 16, 16], x - 1.0, y + lift);
                }
            }
        }
    }
    /// `WCTScreen`'s Magnet Filter button (`Icon.MAGNET`), on the toolbar's
    /// backdrop as the terminal's other toolbar buttons.
    pub(in crate::game) fn ae2wtlib_draw_magnet_button(&self, batch: &mut HudBatch, ox: f32, oy: f32, rect: [i32; 4], mouse: [f32; 2]) {
        use crate::game::ae2_screens::icon;
        let hover = inside(rect, mouse);
        let lift = if hover { 1.0 } else { 0.0 };
        let (x, y) = (ox + rect[0] as f32, oy + rect[1] as f32);
        self.ae2_icon(batch, if hover { icon::TOOLBAR_BUTTON_BACKGROUND_HOVER } else { icon::TOOLBAR_BUTTON_BACKGROUND }, (18, 20), x - 1.0, y + lift);
        let icons = Image { texture: ICONS.into(), texture_size: (128, 128), src: None };
        self.ae2_blit(batch, &icons, [ICON_MAGNET.0, ICON_MAGNET.1, 16, 16], x, y + 1.0 + lift);
    }
    /// `WCTScreen`'s Trash button (`Icon.TRASH`), as the Magnet Filter's.
    pub(in crate::game) fn ae2wtlib_draw_trash_button(&self, batch: &mut HudBatch, ox: f32, oy: f32, rect: [i32; 4], mouse: [f32; 2]) {
        use crate::game::ae2_screens::icon;
        let hover = inside(rect, mouse);
        let lift = if hover { 1.0 } else { 0.0 };
        let (x, y) = (ox + rect[0] as f32, oy + rect[1] as f32);
        self.ae2_icon(batch, if hover { icon::TOOLBAR_BUTTON_BACKGROUND_HOVER } else { icon::TOOLBAR_BUTTON_BACKGROUND }, (18, 20), x - 1.0, y + lift);
        let icons = Image { texture: ICONS.into(), texture_size: (128, 128), src: None };
        self.ae2_blit(batch, &icons, [ICON_TRASH.0, ICON_TRASH.1, 16, 16], x, y + 1.0 + lift);
    }
    /// The hovered part's tooltip: a button's message, a filter's item.
    pub(in crate::game) fn ae2wtlib_magnet_tooltip(&self, at: [f32; 2]) -> Option<String> {
        let (part, _) = self.ae2wtlib_magnet_parts().into_iter().find(|(_, r)| inside(*r, at))?;
        let filters = self.ae2wtlib_magnet_filters();
        let tr = |key: &str| self.translate(key, key);
        Some(match part {
            Part::Filter(which, index) => {
                let item = filters?.slots[usize::from(which)].get(usize::from(index)).cloned().flatten()?;
                let (namespace, path) = item.split_once(':').unwrap_or(("minecraft", &item));
                self.translate(&format!("item.{namespace}.{path}"), &item)
            }
            Part::Button(action @ (0 | 1)) => {
                let allow = filters.is_some_and(|f| f.allow[usize::from(action)]);
                let key = if action == 0 { "gui.ae2wtlib.pickup_filter" } else { "gui.ae2wtlib.insert_filter" };
                tr(key).replace("%s", &tr(if allow { "gui.ae2wtlib.allow" } else { "gui.ae2wtlib.deny" }))
            }
            Part::Button(2) => tr("gui.ae2wtlib.copy_pickup"),
            Part::Button(3) => tr("gui.ae2wtlib.copy_insert"),
            Part::Button(_) => tr("gui.ae2wtlib.switch"),
        })
    }
    /// The screen for the control socket's `screen`: its filter slots and
    /// buttons in GUI pixels, what each filter holds and allows.
    pub(in crate::game) fn ae2wtlib_magnet_agent(&self, origin: (f32, f32)) -> serde_json::Value {
        let rect = |[x, y, w, h]: [i32; 4]| [origin.0 + x as f32, origin.1 + y as f32, w as f32, h as f32];
        let filters = self.ae2wtlib_magnet_filters();
        let parts: Vec<serde_json::Value> = self
            .ae2wtlib_magnet_parts()
            .into_iter()
            .map(|(part, r)| match part {
                Part::Filter(which, index) => serde_json::json!({"filter": which, "index": index, "rect": rect(r),
                    "item": filters.as_ref().and_then(|f| f.slots[usize::from(which)].get(usize::from(index)).cloned().flatten())}),
                Part::Button(action) => serde_json::json!({"button": BUTTONS[usize::from(action)].0, "rect": rect(r)}),
            })
            .collect();
        serde_json::json!({"parts": parts, "allow": filters.map(|f| f.allow)})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_view_carries_both_filters_and_their_modes() {
        let view = serde_json::json!({"filters": {"pickup": ["minecraft:dirt", null], "insert": [], "pickup_allow": true, "insert_allow": false}});
        let f = filters(&view).unwrap();
        assert_eq!(f.slots[0], [Some("minecraft:dirt".to_owned()), None]);
        assert!(f.slots[1].is_empty());
        assert_eq!(f.allow, [true, false]);
        assert!(filters(&serde_json::json!({"filters": null})).is_none());
    }
}
