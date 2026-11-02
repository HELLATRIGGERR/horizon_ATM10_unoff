//! AE2's `NetworkStatusScreen` and `SpatialAnchorScreen` over their placeholder
//! menus, from `ae2:status_view` and `ae2:anchor_view`: real toolbar buttons,
//! the machine table with its scrollbar. The menus keep their places, so
//! `click-slot` still reaches them. Also the power unit button these screens
//! share with the Spatial IO Port and the Wireless Access Point.
use super::{
    Game,
    ae2_client_config::{next_power_unit, power_unit},
    ae2_screens::{fill, format_power, icon, inside},
    ae2_widgets::{Scroll, toolbar_button, widget},
};
use horizon_client::{
    ae2_screen_style::ScreenStyle,
    hud::HudBatch,
    items::ItemStack,
};
use serde::{Deserialize, Serialize};

/// `NetworkStatusScreen`'s table.
const ROWS: i32 = 4;
const COLUMNS: i32 = 5;
const TABLE_AT: (i32, i32) = (14, 41);
const CELL: (i32, i32) = (30, 18);
/// `Icon.POWER_UNIT_AE`, `POWER_UNIT_RF` (FE's), `OVERLAY_ON`, `OVERLAY_OFF`.
const POWER_AE: (i32, i32) = (0, 160);
const POWER_FE: (i32, i32) = (64, 160);
const OVERLAY_ON: (i32, i32) = (64, 240);
const OVERLAY_OFF: (i32, i32) = (48, 240);
/// The anchor's overlay toggle: its place in the anchor's menu.
const OVERLAY_PLACE: usize = 300;

/// The pattern access terminal's scrollbar: its six rows from the top.
const PATTERN_TRACK: [i32; 4] = [175, 18, 12, 106];

/// The icon of the power unit button.
pub(super) fn power_icon() -> (i32, i32) {
    if power_unit() == 1 { POWER_FE } else { POWER_AE }
}

/// `MachineGroup`: a machine's item and how many the grid has.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct Group {
    pub(super) item: String,
    pub(super) count: usize,
    #[serde(skip)]
    stack: Option<ItemStack>,
}

impl Group {
    pub(super) fn new(item: String, count: usize) -> Self {
        Self { item, count, stack: None }
    }
}

/// What `NetworkStatusScreen` shows.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct StatusView {
    pub(super) window: i32,
    pub(super) channels: u64,
    pub(super) stored: f64,
    pub(super) capacity: f64,
    pub(super) usage: f64,
    pub(super) groups: Vec<Group>,
}

/// What `SpatialAnchorScreen` shows.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct AnchorView {
    pub(super) window: i32,
    pub(super) overlay: bool,
    pub(super) power: f64,
    pub(super) chunks: usize,
    pub(super) all_loaded: usize,
    pub(super) loaded_worlds: usize,
    pub(super) all_chunks: usize,
    pub(super) all_worlds: usize,
}

/// The open status or anchor screen's own state.
#[derive(Clone, Debug, Default)]
pub(super) struct StatusUi {
    status: Option<StatusView>,
    anchor: Option<AnchorView>,
    scroll: Scroll,
}

/// `drawMachineCount`'s text: the count, in thousands past 9999.
fn count_text(count: usize) -> String {
    if count >= 10000 { format!("{}k", count / 1000) } else { count.to_string() }
}

impl Game<'_> {
    fn ae2_status_view(&self) -> Option<&StatusView> {
        let r = self.remote.as_ref().filter(|r| r.ae2_network_status_screen)?;
        r.ae2_status.status.as_ref().filter(|v| r.menu.as_ref().is_some_and(|m| m.0 == v.window))
    }

    fn ae2_anchor_view(&self) -> Option<&AnchorView> {
        let r = self.remote.as_ref().filter(|r| r.ae2_anchor_screen)?;
        r.ae2_status.anchor.as_ref().filter(|v| r.menu.as_ref().is_some_and(|m| m.0 == v.window))
    }

    /// Whether one of these screens stands in front of its menu.
    pub(super) fn ae2_status_open(&self) -> bool {
        self.ae2_status_view().is_some() || self.ae2_anchor_view().is_some()
    }

    pub(super) fn ae2_accept_status_view(&mut self, data: &[u8]) {
        let Ok(mut view) = serde_json::from_slice::<StatusView>(data) else {
            return;
        };
        for group in &mut view.groups {
            group.stack = horizon_client::item_data::parse_stack(&group.item).ok().flatten();
        }
        if let Some(r) = self.remote.as_mut() {
            let range = Self::status_range(view.groups.len());
            r.ae2_status.scroll.clamp(range);
            r.ae2_status.status = Some(view);
        }
    }

    pub(super) fn ae2_accept_anchor_view(&mut self, data: &[u8]) {
        let Ok(view) = serde_json::from_slice::<AnchorView>(data) else {
            return;
        };
        if let Some(r) = self.remote.as_mut() {
            r.ae2_status.anchor = Some(view);
        }
    }

    /// `setScrollBar`.
    fn status_range(groups: usize) -> i32 {
        ((groups as i32 + COLUMNS - 1) / COLUMNS - ROWS).max(0)
    }

    fn ae2_status_style(&self) -> Option<&'static ScreenStyle> {
        if self.ae2_status_view().is_some() {
            self.ae2_style("network_status.json")
        } else if self.ae2_anchor_view().is_some() {
            self.ae2_style("spatial_anchor.json")
        } else {
            None
        }
    }

    fn ae2_status_track(&self, style: &ScreenStyle) -> Option<[i32; 4]> {
        self.ae2_status_view().and_then(|_| widget(style, "scrollbar", (12, 78)))
    }

    /// Where the anchor's overlay toggle is, for `click-slot`.
    pub(super) fn ae2_anchor_slot_position(&self, i: usize) -> Option<(f32, f32)> {
        self.ae2_anchor_view()?;
        let [x, y, _, _] = toolbar_button(self.ae2_status_style()?, 2);
        Some(if i == OVERLAY_PLACE { (x as f32, y as f32) } else { (-1000.0, -1000.0) })
    }

    /// A press these screens take before the menu.
    pub(super) fn ae2_status_press(&mut self, at: [f32; 2], right: bool) -> bool {
        let Some(style) = self.ae2_status_style() else {
            return false;
        };
        if !right && inside(toolbar_button(style, 0), at) {
            self.audio.play_button_click();
            let page = if self.ae2_status_view().is_some() { "ae2:network_tool" } else { "ae2:spatial_anchor" };
            self.ae2_open_guide(page);
            return true;
        }
        if inside(toolbar_button(style, 1), at) {
            self.audio.play_button_click();
            next_power_unit();
            return true;
        }
        if let Some(track) = self.ae2_status_track(style).filter(|t| inside(*t, at)) {
            let range = Self::status_range(self.ae2_status_view().map_or(0, |v| v.groups.len()));
            if let Some(r) = self.remote.as_mut() {
                r.ae2_status.scroll.press(track, range, 1, at[1]);
            }
            return true;
        }
        false
    }

    pub(super) fn ae2_status_drag(&mut self) {
        let Some(track) = self.ae2_status_style().and_then(|s| self.ae2_status_track(s)) else {
            return;
        };
        let range = Self::status_range(self.ae2_status_view().map_or(0, |v| v.groups.len()));
        let Some((_, oy)) = self.gui_scale().map(|b| self.inventory_origin(&b)) else {
            return;
        };
        let y = self.cursor.1 - oy;
        if let Some(r) = self.remote.as_mut() {
            r.ae2_status.scroll.dragged(track, range, y);
        }
    }

    pub(super) fn ae2_status_release(&mut self) {
        if let Some(r) = self.remote.as_mut() {
            r.ae2_status.scroll.drag = None;
        }
    }

    pub(super) fn ae2_status_wheel(&mut self, delta: f32) -> bool {
        let Some(view) = self.ae2_status_view() else {
            return false;
        };
        let range = Self::status_range(view.groups.len());
        if let Some(r) = self.remote.as_mut() {
            r.ae2_status.scroll.wheel(range, 1, delta);
        }
        true
    }

    fn ae2_status_rows(&self, view: &StatusView) -> Vec<([i32; 4], usize)> {
        let top = self.remote.as_ref().map_or(0, |r| r.ae2_status.scroll.current);
        (0..ROWS * COLUMNS)
            .filter_map(|i| {
                let index = (top * COLUMNS + i) as usize;
                view.groups.get(index)?;
                Some(([TABLE_AT.0 + i % COLUMNS * CELL.0, TABLE_AT.1 + i / COLUMNS * CELL.1, CELL.0, CELL.1], index))
            })
            .collect()
    }

    pub(super) fn ae2_draw_status(&self, batch: &mut HudBatch, ox: f32, oy: f32) {
        let Some(style) = self.ae2_status_style() else {
            return;
        };
        let mouse = [self.cursor.0 - ox, self.cursor.1 - oy];
        let tr = |key: &str, args: &[String]| fill(&self.translate(key, key), args);
        self.ae2_draw_background(batch, ox, oy, style);
        let (help, power) = (toolbar_button(style, 0), toolbar_button(style, 1));
        if let Some(v) = self.ae2_status_view() {
            self.ae2_draw_toolbar(batch, ox, oy, help, power);
            self.ae2_draw_icon_button(batch, ox, oy, help, icon::HELP, mouse);
            self.ae2_draw_icon_button(batch, ox, oy, power, power_icon(), mouse);
            let texts = [
                ("dialog_title", tr("gui.ae2.NetworkDetails", &[v.channels.to_string()])),
                ("stored_power", tr("gui.ae2.StoredPower", &[format_power(v.stored, false)])),
                ("max_power", tr("gui.ae2.MaxPower", &[format_power(v.capacity, false)])),
                ("power_usage_rate", tr("gui.ae2.PowerUsageRate", &[format_power(v.usage, true)])),
            ];
            self.ae2_draw_texts(batch, ox, oy, style, &texts);
            if let (Some(track), Some(r)) = (self.ae2_status_track(style), self.remote.as_ref()) {
                self.ae2_draw_scroller(batch, ox, oy, track, Self::status_range(v.groups.len()), r.ae2_status.scroll);
            }
            let color = super::ae2_screens::argb(style.color("DEFAULT_TEXT_COLOR"));
            for ([x, y, w, _], index) in self.ae2_status_rows(v) {
                let group = &v.groups[index];
                let item_x = x + w - 17;
                let text = count_text(group.count);
                let (tw, th) = (HudBatch::text_width(&self.gui.atlas, &text) / 2.0, 4.5);
                let (tx, ty) = (ox + item_x as f32 - 1.0 - tw, oy + y as f32 + (CELL.1 as f32 - th) / 2.0);
                let start = batch.vertices.len();
                batch.text(&self.gui.atlas, &text, tx, ty, color, false);
                batch.scale_since(start, tx, ty, 0.5, 0.5);
                if let Some(stack) = group.stack.as_ref() {
                    self.draw_item_model(batch, stack, ox + item_x as f32, oy + (y + 1) as f32);
                }
            }
        } else if let Some(v) = self.ae2_anchor_view() {
            let overlay = toolbar_button(style, 2);
            self.ae2_draw_toolbar(batch, ox, oy, help, overlay);
            self.ae2_draw_icon_button(batch, ox, oy, help, icon::HELP, mouse);
            self.ae2_draw_icon_button(batch, ox, oy, power, power_icon(), mouse);
            self.ae2_draw_icon_button(batch, ox, oy, overlay, if v.overlay { OVERLAY_ON } else { OVERLAY_OFF }, mouse);
            let texts = [
                ("used_power", tr("gui.ae2.SpatialAnchorUsedPower", &[format_power(v.power, true)])),
                ("loaded_chunks", tr("gui.ae2.SpatialAnchorLoadedChunks", &[v.chunks.to_string()])),
                ("statistics_loaded", tr("gui.ae2.SpatialAnchorAllLoaded", &[v.all_loaded.to_string(), v.loaded_worlds.to_string()])),
                ("statistics_total", tr("gui.ae2.SpatialAnchorAll", &[v.all_chunks.to_string(), v.all_worlds.to_string()])),
            ];
            self.ae2_draw_texts(batch, ox, oy, style, &texts);
        }
    }

    /// The tooltip under the cursor: the toolbar's, or a machine's.
    pub(super) fn ae2_status_tooltip(&mut self, batch: &mut HudBatch) -> bool {
        let Some(style) = self.ae2_status_style() else {
            return false;
        };
        let (ox, oy) = self.inventory_origin(batch);
        let at = [self.cursor.0 - ox, self.cursor.1 - oy];
        let mut lines = Vec::new();
        if inside(toolbar_button(style, 0), at) {
            self.ae2_help_tooltip(&mut lines);
        } else if inside(toolbar_button(style, 1), at) {
            self.ae2_power_tooltip(&mut lines);
        } else if let Some(v) = self.ae2_anchor_view() {
            if inside(toolbar_button(style, 2), at) {
                let value = if v.overlay { "gui.tooltips.ae2.OverlayModeYes" } else { "gui.tooltips.ae2.OverlayModeNo" };
                lines.push(self.translate("gui.tooltips.ae2.OverlayMode", "Overlay Mode"));
                lines.push(format!("\u{a7}7{}", self.translate(value, value)));
            }
        } else if let Some(v) = self.ae2_status_view() {
            for (rect, index) in self.ae2_status_rows(v) {
                if inside(rect, at)
                    && let Some(stack) = v.groups[index].stack.as_ref()
                {
                    lines.push(self.pack.names.display_item(&stack.item));
                    lines.push(fill(&self.translate("gui.ae2.Installed", "Installed: %s"), &[v.groups[index].count.to_string()]));
                }
            }
        }
        if lines.is_empty() {
            return false;
        }
        self.ae2_tooltip_lines(batch, lines);
        true
    }

    /// The pattern access terminal's show button and big scrollbar.
    pub(super) fn ae2_draw_pattern_access_chrome(&self, batch: &mut HudBatch, ox: f32, oy: f32) {
        let layout = self.ae2_pattern_access_layout();
        let number = |name: &str| layout.as_ref().and_then(|v| v.get(name)).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let mouse = [self.cursor.0 - ox, self.cursor.1 - oy];
        let show = [-18, 18, 16, 16];
        self.ae2_draw_toolbar(batch, ox, oy, show, show);
        // `Icon.PATTERN_TERMINAL_VISIBLE`, `NOT_FULL`, `ALL`.
        let shown = match number("filter") {
            1 => (128, 80),
            2 => (112, 80),
            _ => (96, 80),
        };
        self.ae2_draw_icon_button(batch, ox, oy, show, shown, mouse);
        let scroll = Scroll { current: number("offset"), drag: None };
        self.ae2_draw_scroller(batch, ox, oy, PATTERN_TRACK, number("maximum"), scroll);
    }

    /// `Settings.POWER_UNITS`'s tooltip.
    pub(super) fn ae2_power_tooltip(&self, lines: &mut Vec<String>) {
        let value = if power_unit() == 1 { "gui.ae2.units.fe" } else { "gui.ae2.units.appliedenergistics" };
        lines.push(self.translate("gui.tooltips.ae2.PowerUnits", "Power Units"));
        lines.push(format!("\u{a7}7{}", self.translate(value, value)));
    }

    /// These screens for the control socket's `screen`.
    pub(super) fn ae2_status_agent_value(&self, origin: (f32, f32)) -> Option<serde_json::Value> {
        let style = self.ae2_status_style()?;
        let rect = |[x, y, w, h]: [i32; 4]| [origin.0 + x as f32, origin.1 + y as f32, w as f32, h as f32];
        let cells = self.ae2_status_view().map(|v| {
            self.ae2_status_rows(v)
                .into_iter()
                .map(|(r, index)| serde_json::json!({"rect": rect(r), "item": v.groups[index].stack.as_ref().map(|s| s.item.to_string()), "count": v.groups[index].count}))
                .collect::<Vec<_>>()
        });
        Some(serde_json::json!({
            "help": rect(toolbar_button(style, 0)),
            "power": rect(toolbar_button(style, 1)),
            "unit": power_unit(),
            "overlay": self.ae2_anchor_view().map(|v| v.overlay),
            "cells": cells,
            "groups": self.ae2_status_view().map(|v| v.groups.len()),
            "scroll": self.remote.as_ref().map(|r| r.ae2_status.scroll.current),
            "track": self.ae2_status_track(style).map(rect),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_machine_table_scrolls_by_rows_of_five() {
        assert_eq!(Game::status_range(0), 0);
        assert_eq!(Game::status_range(20), 0);
        assert_eq!(Game::status_range(21), 1);
        assert_eq!(Game::status_range(25), 1);
        assert_eq!(Game::status_range(26), 2);
    }

    #[test]
    fn counts_read_in_thousands_past_9999() {
        assert_eq!(count_text(9999), "9999");
        assert_eq!(count_text(12345), "12k");
    }
}
