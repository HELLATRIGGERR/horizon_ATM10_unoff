//! AE2's `CellWorkbenchScreen` over the workbench's paged menu: all 63 filters
//! from `ae2:workbench_view`, pressed by index (`CONFIG_PLACE`), and the
//! toolbar's separate buttons, which press the menu's entries. The machine
//! screen of `ae2_screens` draws the rest: the cell slot, upgrades, inventory.
use super::{
    Game,
    ae2_screens::{icon, inside},
    ae2_widgets::toolbar_button,
};
use horizon_client::{
    ae2_screen_style::{Image, ScreenStyle},
    hud::{HudBatch, rgba},
    items::ItemStack,
};
use serde::{Deserialize, Serialize};

/// The menu place of filter `index`, whatever page the menu is on.
pub(super) const CONFIG_PLACE: i16 = 100;
pub(super) const CONFIG_PLACES: usize = 63;
/// The menu's entries: fuzzy (left) and copy mode (right), clear (left) and
/// partition (right).
const SETTINGS: i16 = 23;
const FILTERS: i16 = 24;
/// `Icon.CLEAR`, `COPY_MODE_ON`, `COPY_MODE_OFF`.
const CLEAR: (i32, i32) = (96, 0);
const COPY_ON: (i32, i32) = (80, 176);
const COPY_OFF: (i32, i32) = (80, 192);

/// What the screen shows beyond the menu's entries.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct WorkbenchView {
    pub(super) window: i32,
    /// Per filter, its stack's canonical text; empty when unset.
    pub(super) config: Vec<String>,
    /// How many filters are open: all with a cell or in keep mode.
    pub(super) enabled: usize,
    /// How many upgrade slots the cell has.
    pub(super) upgrades: usize,
    pub(super) fuzzy: String,
    pub(super) fuzzy_card: bool,
    pub(super) keep: bool,
    #[serde(skip)]
    pub(super) stacks: Vec<Option<ItemStack>>,
}

/// A toolbar button after the help button: the entry and button it presses
/// (none for its own action), its icon, and its tooltip's keys.
struct Tool {
    press: (i16, i32),
    reverse: Option<(i16, i32)>,
    icon: (i32, i32),
    title: &'static str,
    value: &'static str,
}

fn tools(v: &WorkbenchView) -> Vec<Tool> {
    let mut tools = Vec::new();
    if v.fuzzy_card {
        let (shown, value) = match v.fuzzy.as_str() {
            "percent_99" => (icon::FUZZY_PERCENT_99, "gui.tooltips.ae2.FZPercent_99"),
            "percent_75" => (icon::FUZZY_PERCENT_75, "gui.tooltips.ae2.FZPercent_75"),
            "percent_50" => (icon::FUZZY_PERCENT_50, "gui.tooltips.ae2.FZPercent_50"),
            "percent_25" => (icon::FUZZY_PERCENT_25, "gui.tooltips.ae2.FZPercent_25"),
            _ => (icon::FUZZY_IGNORE, "gui.tooltips.ae2.FZIgnoreAll"),
        };
        tools.push(Tool { press: (SETTINGS, 0), reverse: None, icon: shown, title: "gui.tooltips.ae2.FuzzyMode", value });
    }
    tools.push(Tool { press: (FILTERS, 1), reverse: None, icon: icon::COG, title: "gui.tooltips.ae2.PartitionStorage", value: "gui.tooltips.ae2.PartitionStorageHint" });
    tools.push(Tool { press: (FILTERS, 0), reverse: None, icon: CLEAR, title: "gui.tooltips.ae2.Clear", value: "gui.tooltips.ae2.ClearSettings" });
    tools.push(Tool {
        press: (SETTINGS, 1),
        reverse: Some((SETTINGS, 1)),
        icon: if v.keep { COPY_OFF } else { COPY_ON },
        title: "gui.ae2.CopyMode",
        value: "gui.ae2.CopyModeDesc",
    });
    tools
}

impl Game<'_> {
    fn ae2_workbench_view(&self) -> Option<&WorkbenchView> {
        let r = self.remote.as_ref()?;
        if r.ae2_priority_screen {
            return None;
        }
        r.ae2_workbench.as_ref().filter(|v| r.menu.as_ref().is_some_and(|m| m.0 == v.window))
    }

    /// Whether this screen stands in front of the workbench's menu.
    pub(super) fn ae2_workbench_open(&self) -> bool {
        self.ae2_workbench_view().is_some()
    }

    /// How many upgrade slots to show: the cell's, for the workbench.
    pub(super) fn ae2_workbench_upgrades(&self) -> Option<usize> {
        self.ae2_workbench_view().map(|v| v.upgrades.min(4))
    }

    fn ae2_workbench_style(&self) -> Option<&'static ScreenStyle> {
        self.ae2_style("cell_workbench.json")
    }

    /// `ae2:workbench_view` from the server.
    pub(super) fn ae2_accept_workbench_view(&mut self, data: &[u8]) {
        let Ok(mut view) = serde_json::from_slice::<WorkbenchView>(data) else {
            return;
        };
        view.stacks = view.config.iter().map(|text| horizon_client::item_data::parse_stack(text).ok().flatten()).collect();
        if let Some(r) = self.remote.as_mut() {
            r.ae2_workbench = Some(view);
        }
    }

    fn ae2_workbench_filter_at(&self, style: &ScreenStyle, at: [f32; 2]) -> Option<usize> {
        (0..CONFIG_PLACES).find(|i| style.slot("CONFIG", *i).is_some_and(|(x, y)| inside([x - 1, y - 1, 18, 18], at)))
    }

    /// A press the screen takes before the menu: a toolbar button or a filter.
    pub(super) fn ae2_workbench_press(&mut self, at: [f32; 2], right: bool) -> bool {
        let (Some(style), Some(v)) = (self.ae2_workbench_style(), self.ae2_workbench_view().cloned()) else {
            return false;
        };
        for (i, tool) in tools(&v).into_iter().enumerate() {
            if inside(toolbar_button(style, i as i32 + 1), at) {
                if !right || tool.reverse.is_some() {
                    self.audio.play_button_click();
                    self.ae2_send_menu_button(tool.press.0, tool.press.1);
                }
                return true;
            }
        }
        if let Some(index) = self.ae2_workbench_filter_at(style, at) {
            if index < v.enabled {
                self.ae2_send_menu_button(CONFIG_PLACE + index as i16, i32::from(right));
                let held = self.carried.is_some();
                if let Some(r) = self.remote.as_mut() {
                    r.ae2_workbench_drag = held.then(|| (right, vec![index]));
                }
            }
            return true;
        }
        false
    }

    /// `AEBaseScreen.mouseDragged` over the filters with a stack held: each filter the drag reaches joins it,
    /// and from the second on every one of them is set again, as AE2 sends them.
    pub(super) fn ae2_workbench_drag(&mut self) {
        let Some((right, mut filters)) = self.remote.as_ref().and_then(|r| r.ae2_workbench_drag.clone()) else {
            return;
        };
        let (Some(style), Some(enabled), Some((ox, oy))) = (
            self.ae2_workbench_style(),
            self.ae2_workbench_view().map(|v| v.enabled),
            self.gui_scale().map(|b| self.inventory_origin(&b)),
        ) else {
            return;
        };
        let at = [self.cursor.0 - ox, self.cursor.1 - oy];
        let Some(index) = self.ae2_workbench_filter_at(style, at).filter(|&i| i < enabled && !filters.contains(&i)) else {
            return;
        };
        if self.carried.is_none() {
            return;
        }
        filters.push(index);
        for &i in &filters {
            self.ae2_send_menu_button(CONFIG_PLACE + i as i16, i32::from(right));
        }
        if let Some(r) = self.remote.as_mut() {
            r.ae2_workbench_drag = Some((right, filters));
        }
    }

    pub(super) fn ae2_workbench_release(&mut self) {
        if let Some(r) = self.remote.as_mut() {
            r.ae2_workbench_drag = None;
        }
    }

    /// The filters and toolbar over the machine screen.
    pub(super) fn ae2_draw_workbench(&self, batch: &mut HudBatch, ox: f32, oy: f32) {
        let (Some(style), Some(v)) = (self.ae2_workbench_style(), self.ae2_workbench_view()) else {
            return;
        };
        let mouse = [self.cursor.0 - ox, self.cursor.1 - oy];
        let tools = tools(v);
        let help = toolbar_button(style, 0);
        self.ae2_draw_toolbar(batch, ox, oy, help, toolbar_button(style, tools.len() as i32));
        self.ae2_draw_icon_button(batch, ox, oy, help, icon::HELP, mouse);
        for (i, tool) in tools.iter().enumerate() {
            self.ae2_draw_icon_button(batch, ox, oy, toolbar_button(style, i as i32 + 1), tool.icon, mouse);
        }
        let states = Image { texture: "guis/states".into(), texture_size: (256, 256), src: None };
        let (sx, sy) = icon::SLOT_BACKGROUND;
        for i in 0..CONFIG_PLACES {
            let Some((x, y)) = style.slot("CONFIG", i) else {
                continue;
            };
            let open = i < v.enabled;
            let alpha = if open { 255 } else { 51 };
            self.ae2_blit_to(batch, &states, [sx, sy, 18, 18], [ox + (x - 1) as f32, oy + (y - 1) as f32, 18.0, 18.0], rgba(255, 255, 255, alpha));
            if let Some(stack) = v.stacks.get(i).and_then(Option::as_ref) {
                self.draw_item_model(batch, stack, ox + x as f32, oy + y as f32);
            }
            if open && inside([x - 1, y - 1, 18, 18], mouse) {
                batch.fill(&self.gui.atlas, ox + x as f32, oy + y as f32, 16.0, 16.0, rgba(255, 255, 255, 128));
            }
        }
    }

    /// The tooltip under the cursor: a toolbar button's, or a filter's stack.
    pub(super) fn ae2_workbench_tooltip(&mut self, batch: &mut HudBatch) -> bool {
        let (Some(style), Some(v)) = (self.ae2_workbench_style(), self.ae2_workbench_view().cloned()) else {
            return false;
        };
        let (ox, oy) = self.inventory_origin(batch);
        let at = [self.cursor.0 - ox, self.cursor.1 - oy];
        if let Some((_, tool)) = tools(&v).into_iter().enumerate().find(|(i, _)| inside(toolbar_button(style, *i as i32 + 1), at)) {
            let mut lines = vec![self.translate(tool.title, tool.title)];
            lines.extend(self.translate(tool.value, tool.value).split('\n').map(|l| format!("\u{a7}7{l}")));
            self.ae2_tooltip_lines(batch, lines);
            return true;
        }
        if let Some(stack) = self.ae2_workbench_filter_at(style, at).and_then(|i| v.stacks.get(i)).and_then(Option::as_ref) {
            self.draw_stack_tooltip(batch, stack, self.cursor.0, self.cursor.1);
            return true;
        }
        false
    }

    /// The workbench's screen for the control socket's `screen`.
    pub(super) fn ae2_workbench_agent_value(&self, origin: (f32, f32)) -> Option<serde_json::Value> {
        let (style, v) = (self.ae2_workbench_style()?, self.ae2_workbench_view()?);
        let rect = |[x, y, w, h]: [i32; 4]| [origin.0 + x as f32, origin.1 + y as f32, w as f32, h as f32];
        let config: Vec<serde_json::Value> = (0..CONFIG_PLACES)
            .filter_map(|i| {
                let (x, y) = style.slot("CONFIG", i)?;
                let stack = v.stacks.get(i).and_then(Option::as_ref);
                Some(serde_json::json!({"rect": rect([x, y, 16, 16]), "item": stack.map(|s| s.item.to_string()), "components": stack.and_then(|s| self.stack_value(s).get("components").cloned()), "open": i < v.enabled}))
            })
            .collect();
        let tools: Vec<serde_json::Value> = tools(v)
            .iter()
            .enumerate()
            .map(|(i, t)| serde_json::json!({"title": t.title, "value": t.value, "rect": rect(toolbar_button(style, i as i32 + 1))}))
            .collect();
        Some(serde_json::json!({"config": config, "tools": tools, "upgrades": v.upgrades, "keep": v.keep, "fuzzy": v.fuzzy}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fuzzy_button_needs_its_card_and_the_toolbar_keeps_java_s_order() {
        let mut view = WorkbenchView::default();
        let titles = |v: &WorkbenchView| tools(v).iter().map(|t| t.title).collect::<Vec<_>>();
        assert_eq!(titles(&view), ["gui.tooltips.ae2.PartitionStorage", "gui.tooltips.ae2.Clear", "gui.ae2.CopyMode"]);
        view.fuzzy_card = true;
        assert_eq!(titles(&view)[0], "gui.tooltips.ae2.FuzzyMode");
        assert_eq!(tools(&view)[1].press, (FILTERS, 1));
        assert_eq!(tools(&view)[2].press, (FILTERS, 0));
        assert_eq!(tools(&view)[3].press, (SETTINGS, 1));
    }
}
