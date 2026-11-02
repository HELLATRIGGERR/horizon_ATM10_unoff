//! AE2's screens of the parts on a cable bus: `StorageBusScreen`,
//! `IOBusScreen`, `FormationPlaneScreen`, `StorageLevelEmitterScreen` and
//! `EnergyLevelEmitterScreen`, over the placeholder menus of `ae2_bus_ui`.
//!
//! The menu keeps its places (36 filters a page, the upgrade cards, one place
//! per setting), so `click-slot` still reaches them. The screen shows all the
//! filters at once from `ae2:part_view` and presses a filter by its index
//! (`CONFIG_PLACE`), which needs no page; settings are presses on their places.
//! An import bus's types open `KeyTypeSelectionScreen` (`key_type_selection.json`),
//! the client's own sub-screen over the same menu. The annihilation plane has
//! no screen in AE2 19.2.18 (the part opens no menu).
use super::{
    Game,
    ae2_screens::{argb, fill, icon, inside},
    ae2_widgets::{toolbar_button, widget},
};
use horizon_client::{
    ae2_screen_style::{Image, ScreenStyle},
    hud::{HudBatch, rgba},
    items::ItemStack,
    menu::Input,
};
use serde::{Deserialize, Serialize};

/// The menu place of filter `index`, whatever page the menu is on.
pub(super) const CONFIG_PLACE: usize = 100;
pub(super) const CONFIG_PLACES: usize = 63;
/// The storage bus's clear and partition buttons.
pub(super) const CLEAR_PLACE: usize = 200;
pub(super) const PARTITION_PLACE: usize = 201;
/// `addOpenPriorityButton`.
pub(super) const PRIORITY_PLACE: usize = 202;
/// An import bus's key types: items, then fluids (`KeyTypeSelection`).
pub(super) const KEY_TYPES_PLACE: usize = 203;
/// `KeyTypeSelectionScreen`: the generated background's width, its boxes.
const KEY_TYPES_WIDTH: i32 = 200;
const CHECKBOX_SIZE: i32 = 14;
const CHECKBOX_SPACING: i32 = CHECKBOX_SIZE + 6;
const KEY_TYPE_NAMES: [&str; 2] = ["gui.ae2.Items", "gui.ae2.Fluids"];
/// The menu place the toolbar's key type button stands for: no server click.
const KEY_TYPES_BUTTON: usize = usize::MAX;
/// `NumberEntryWidget`'s buttons from the widget's top left, in entry order:
/// x, y and width (each 20 tall).
pub(super) const NUMBER_BUTTONS: [(i32, i32, i32); 8] = [
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

mod more_icon {
    pub const CLEAR: (i32, i32) = (96, 0);
    pub const TYPE_FILTER_ALL: (i32, i32) = (160, 16);
    pub const REDSTONE_ABOVE_EQUAL: (i32, i32) = (192, 0);
    pub const REDSTONE_BELOW: (i32, i32) = (208, 0);
    pub const VIEW_MODE_ALL: (i32, i32) = (32, 16);
    pub const VIEW_MODE_CRAFTING: (i32, i32) = (48, 16);
    pub const STORAGE_FILTER_EXTRACTABLE_ONLY: (i32, i32) = (80, 48);
    pub const STORAGE_FILTER_EXTRACTABLE_NONE: (i32, i32) = (96, 48);
    pub const FILTER_ON_EXTRACT_ENABLED: (i32, i32) = (160, 48);
    pub const FILTER_ON_EXTRACT_DISABLED: (i32, i32) = (176, 48);
    pub const ACCESS_WRITE: (i32, i32) = (0, 144);
    pub const ACCESS_READ: (i32, i32) = (16, 144);
    pub const ACCESS_READ_WRITE: (i32, i32) = (32, 144);
    pub const CRAFT_HAMMER: (i32, i32) = (48, 144);
    pub const SCHEDULING_DEFAULT: (i32, i32) = (0, 240);
    pub const SCHEDULING_ROUND_ROBIN: (i32, i32) = (16, 240);
    pub const SCHEDULING_RANDOM: (i32, i32) = (32, 240);
    pub const PLACEMENT_BLOCK: (i32, i32) = (0, 224);
    pub const PLACEMENT_ITEM: (i32, i32) = (16, 224);
}

/// What the screen shows beyond the menu's places.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct PartView {
    pub(super) window: i32,
    pub(super) kind: String,
    /// Per filter, its stack's canonical text; empty when unset.
    pub(super) config: Vec<String>,
    /// How many filters the capacity cards open.
    pub(super) enabled: usize,
    pub(super) upgrades: usize,
    pub(super) redstone: String,
    pub(super) schedule: String,
    pub(super) fuzzy: String,
    pub(super) access: String,
    pub(super) filter_on_extract: bool,
    pub(super) storage_filter: String,
    pub(super) craft_only: bool,
    pub(super) item_entity: bool,
    pub(super) low: bool,
    pub(super) craft_via_redstone: bool,
    pub(super) fuzzy_card: bool,
    pub(super) redstone_card: bool,
    pub(super) crafting_card: bool,
    /// What a storage bus is attached to (`getConnectedToDescription`).
    pub(super) connected: Option<String>,
    /// A level emitter set to a fluid: its field is in buckets.
    pub(super) fluid: bool,
    /// An import bus's key types: items, fluids.
    pub(super) key_types: [bool; 2],
    #[serde(skip)]
    pub(super) stacks: Vec<Option<ItemStack>>,
}

/// The open part screen's own state.
#[derive(Clone, Debug, Default)]
pub(super) struct PartUi {
    view: Option<PartView>,
    /// While a press drags over the filters: those it has set.
    drag: Option<Vec<usize>>,
    /// `KeyTypeSelectionScreen` stands in front of the bus's.
    key_types: bool,
}

/// A button on the left toolbar: the place it presses, its icon or item, and
/// its tooltip's keys.
struct Tool {
    place: usize,
    icon: (i32, i32),
    title: &'static str,
    value: &'static str,
}

fn tools(v: &PartView) -> Vec<Tool> {
    use more_icon as i;
    let fuzzy = |tools: &mut Vec<Tool>| {
        let (icon, value) = match v.fuzzy.as_str() {
            "percent_99" => (icon::FUZZY_PERCENT_99, "gui.tooltips.ae2.FZPercent_99"),
            "percent_75" => (icon::FUZZY_PERCENT_75, "gui.tooltips.ae2.FZPercent_75"),
            "percent_50" => (icon::FUZZY_PERCENT_50, "gui.tooltips.ae2.FZPercent_50"),
            "percent_25" => (icon::FUZZY_PERCENT_25, "gui.tooltips.ae2.FZPercent_25"),
            _ => (icon::FUZZY_IGNORE, "gui.tooltips.ae2.FZIgnoreAll"),
        };
        tools.push(Tool { place: 44, icon, title: "gui.tooltips.ae2.FuzzyMode", value });
    };
    let mut tools = Vec::new();
    match v.kind.as_str() {
        "ae2:storage_bus" => {
            tools.push(Tool { place: CLEAR_PLACE, icon: i::CLEAR, title: "gui.tooltips.ae2.Clear", value: "gui.tooltips.ae2.ClearSettings" });
            tools.push(Tool { place: PARTITION_PLACE, icon: icon::COG, title: "gui.tooltips.ae2.PartitionStorage", value: "gui.tooltips.ae2.PartitionStorageHint" });
            tools.push(if v.storage_filter == "none" {
                Tool { place: 52, icon: i::STORAGE_FILTER_EXTRACTABLE_NONE, title: "gui.tooltips.ae2.ReportInaccessibleItems", value: "gui.tooltips.ae2.ReportInaccessibleItemsYes" }
            } else {
                Tool { place: 52, icon: i::STORAGE_FILTER_EXTRACTABLE_ONLY, title: "gui.tooltips.ae2.ReportInaccessibleItems", value: "gui.tooltips.ae2.ReportInaccessibleItemsNo" }
            });
            tools.push(if v.filter_on_extract {
                Tool { place: 50, icon: i::FILTER_ON_EXTRACT_ENABLED, title: "gui.tooltips.ae2.FilterOnExtract", value: "gui.tooltips.ae2.FilterOnExtractEnabled" }
            } else {
                Tool { place: 50, icon: i::FILTER_ON_EXTRACT_DISABLED, title: "gui.tooltips.ae2.FilterOnExtract", value: "gui.tooltips.ae2.FilterOnExtractDisabled" }
            });
            if v.fuzzy_card {
                fuzzy(&mut tools);
            }
            let (icon, value) = match v.access.as_str() {
                "read_only" => (i::ACCESS_READ, "gui.tooltips.ae2.Read"),
                "write_only" => (i::ACCESS_WRITE, "gui.tooltips.ae2.Write"),
                _ => (i::ACCESS_READ_WRITE, "gui.tooltips.ae2.ReadWrite"),
            };
            tools.push(Tool { place: 49, icon, title: "gui.tooltips.ae2.IOMode", value });
        }
        "ae2:import_bus" | "ae2:export_bus" => {
            if v.kind == "ae2:import_bus" {
                tools.push(Tool { place: KEY_TYPES_BUTTON, icon: i::TYPE_FILTER_ALL, title: "gui.ae2.ConfigureImportedTypes", value: "" });
            }
            if v.redstone_card {
                let (icon, value) = match v.redstone.as_str() {
                    "High" => (icon::REDSTONE_HIGH, "gui.tooltips.ae2.ActiveWithSignal"),
                    "Low" => (icon::REDSTONE_LOW, "gui.tooltips.ae2.ActiveWithoutSignal"),
                    "Pulse" => (icon::REDSTONE_PULSE, "gui.tooltips.ae2.ActiveOnPulse"),
                    _ => (icon::REDSTONE_IGNORE, "gui.tooltips.ae2.AlwaysActive"),
                };
                tools.push(Tool { place: 42, icon, title: "gui.tooltips.ae2.RedstoneMode", value });
            }
            if v.fuzzy_card {
                fuzzy(&mut tools);
            }
            if v.kind == "ae2:export_bus" {
                if v.crafting_card {
                    tools.push(if v.craft_only {
                        Tool { place: 51, icon: i::VIEW_MODE_CRAFTING, title: "gui.tooltips.ae2.Craft", value: "gui.tooltips.ae2.CraftOnly" }
                    } else {
                        Tool { place: 51, icon: i::VIEW_MODE_ALL, title: "gui.tooltips.ae2.Craft", value: "gui.tooltips.ae2.CraftEither" }
                    });
                }
                let (icon, value) = match v.schedule.as_str() {
                    "RoundRobin" => (i::SCHEDULING_ROUND_ROBIN, "gui.tooltips.ae2.SchedulingModeRoundRobin"),
                    "Random" => (i::SCHEDULING_RANDOM, "gui.tooltips.ae2.SchedulingModeRandom"),
                    _ => (i::SCHEDULING_DEFAULT, "gui.tooltips.ae2.SchedulingModeDefault"),
                };
                tools.push(Tool { place: 43, icon, title: "gui.tooltips.ae2.SchedulingMode", value });
            }
        }
        "ae2:formation_plane" => {
            tools.push(if v.item_entity {
                Tool { place: 53, icon: i::PLACEMENT_ITEM, title: "gui.tooltips.ae2.BlockPlacement", value: "gui.tooltips.ae2.BlockPlacementNo" }
            } else {
                Tool { place: 53, icon: i::PLACEMENT_BLOCK, title: "gui.tooltips.ae2.BlockPlacement", value: "gui.tooltips.ae2.BlockPlacementYes" }
            });
            if v.fuzzy_card {
                fuzzy(&mut tools);
            }
        }
        "ae2:level_emitter" | "ae2:energy_level_emitter" => {
            if v.crafting_card {
                tools.push(if v.craft_via_redstone {
                    Tool { place: 43, icon: i::CRAFT_HAMMER, title: "gui.tooltips.ae2.EmitterMode", value: "gui.tooltips.ae2.CraftViaRedstone" }
                } else {
                    Tool { place: 43, icon: i::ACCESS_READ, title: "gui.tooltips.ae2.EmitterMode", value: "gui.tooltips.ae2.EmitWhenCrafting" }
                });
            } else {
                tools.push(if v.low {
                    Tool { place: 42, icon: i::REDSTONE_BELOW, title: "gui.tooltips.ae2.RedstoneMode", value: "gui.tooltips.ae2.EmitLevelsBelow" }
                } else {
                    Tool { place: 42, icon: i::REDSTONE_ABOVE_EQUAL, title: "gui.tooltips.ae2.RedstoneMode", value: "gui.tooltips.ae2.EmitLevelAbove" }
                });
            }
            if v.fuzzy_card {
                fuzzy(&mut tools);
            }
        }
        _ => {}
    }
    tools
}

fn is_emitter(kind: &str) -> bool {
    matches!(kind, "ae2:level_emitter" | "ae2:energy_level_emitter")
}

impl Game<'_> {
    fn ae2_part_view(&self) -> Option<&PartView> {
        let r = self.remote.as_ref()?;
        if !r.ae2_bus_screen || r.ae2_pattern_access_screen || r.ae2_priority_screen {
            return None;
        }
        r.ae2_part.view.as_ref().filter(|v| r.menu.as_ref().is_some_and(|m| m.0 == v.window))
    }

    /// The part the screen is on, also while its priority screen is open.
    pub(super) fn ae2_part_kind(&self) -> Option<&str> {
        let r = self.remote.as_ref()?;
        r.ae2_part.view.as_ref().filter(|v| r.ae2_bus_screen && r.menu.as_ref().is_some_and(|m| m.0 == v.window)).map(|v| v.kind.as_str())
    }

    /// Whether a part's screen stands in front of its menu.
    pub(super) fn ae2_part_open(&self) -> bool {
        self.ae2_part_view().is_some()
    }

    fn ae2_part_style(&self) -> Option<&'static ScreenStyle> {
        self.ae2_style(match self.ae2_part_view()?.kind.as_str() {
            "ae2:storage_bus" => "storage_bus.json",
            "ae2:import_bus" => "import_bus.json",
            "ae2:export_bus" => "export_bus.json",
            "ae2:formation_plane" => "formation_plane.json",
            "ae2:level_emitter" => "level_emitter.json",
            "ae2:energy_level_emitter" => "energy_level_emitter.json",
            _ => return None,
        })
    }

    pub(super) fn ae2_part_size(&self) -> Option<(f32, f32)> {
        if self.ae2_part_key_types_open() {
            let (w, h) = key_types_size();
            return Some((w as f32, h as f32));
        }
        let (w, h) = self.ae2_part_style()?.size();
        Some((w as f32, h as f32))
    }

    /// Whether `KeyTypeSelectionScreen` stands in front of the bus's.
    fn ae2_part_key_types_open(&self) -> bool {
        self.ae2_part_view().is_some() && self.remote.as_ref().is_some_and(|r| r.ae2_part.key_types)
    }

    fn ae2_part_key_types_style(&self) -> Option<&'static ScreenStyle> {
        self.ae2_style("key_type_selection.json")
    }

    /// The boxes of `KeyTypeSelectionScreen`, in the order of the key types.
    fn ae2_part_key_type_rects(&self, style: &ScreenStyle) -> [[i32; 4]; 2] {
        let [x, y, _, _] = widget(style, "keytypes", (160, 0)).unwrap_or([10, 20, 160, 0]);
        [0, 1].map(|i| [x, y + i * CHECKBOX_SPACING, 22, CHECKBOX_SIZE])
    }

    fn ae2_part_key_types_back(&self, style: &ScreenStyle) -> [i32; 4] {
        widget(style, "back", (20, 20)).unwrap_or([176, -5, 20, 20])
    }

    /// A press on `KeyTypeSelectionScreen`: the back tab, or a box, which
    /// is inactive while it is the only one on.
    fn ae2_part_key_types_press(&mut self, at: [f32; 2], v: &PartView) -> bool {
        let Some(style) = self.ae2_part_key_types_style() else {
            return true;
        };
        if inside(self.ae2_part_key_types_back(style), at) {
            self.audio.play_button_click();
            if let Some(r) = self.remote.as_mut() {
                r.ae2_part.key_types = false;
            }
            return true;
        }
        for (i, rect) in self.ae2_part_key_type_rects(style).into_iter().enumerate() {
            let on = v.key_types.iter().filter(|on| **on).count();
            if inside(rect, at) && !(v.key_types[i] && on == 1) {
                self.audio.play_button_click();
                self.ae2_send_menu_button((KEY_TYPES_PLACE + i) as i16, 0);
                return true;
            }
        }
        true
    }

    fn ae2_draw_part_key_types(&self, batch: &mut HudBatch, ox: f32, oy: f32, v: &PartView) {
        let Some(style) = self.ae2_part_key_types_style() else {
            return;
        };
        let (w, h) = key_types_size();
        self.ae2_draw_generated_background(batch, ox, oy, w, h);
        self.ae2_draw_tab(batch, ox, oy, self.ae2_part_key_types_back(style), icon::BACK);
        let on = v.key_types.iter().filter(|on| **on).count();
        let boxes = Image { texture: "guis/checkbox".into(), texture_size: (64, 64), src: None };
        let mouse = [self.cursor.0 - ox, self.cursor.1 - oy];
        for (i, rect) in self.ae2_part_key_type_rects(style).into_iter().enumerate() {
            let active = !(v.key_types[i] && on == 1);
            let hover = active && inside(rect, mouse);
            let src = [if hover { 22 } else { 0 }, if v.key_types[i] { 40 } else { 28 }, 22, 12];
            let alpha = if active { 255 } else { 128 };
            self.ae2_blit_to(batch, &boxes, src, [ox + rect[0] as f32, oy + rect[1] as f32, 22.0, 12.0], rgba(255, 255, 255, alpha));
            let color = argb(style.color(if active { "DEFAULT_TEXT_COLOR" } else { "MUTED_TEXT_COLOR" }));
            let name = self.translate(KEY_TYPE_NAMES[i], KEY_TYPE_NAMES[i]);
            batch.text(&self.gui.atlas, &name, ox + (rect[0] + 26) as f32, oy + (rect[1] + 4) as f32, color, false);
        }
        let title = self.translate("gui.ae2.ConfigureImportedTypes", "Configure Imported Types");
        batch.text(&self.gui.atlas, &title, ox + 8.0, oy + 7.0, argb(style.color("DEFAULT_TEXT_COLOR")), false);
    }

    /// `ae2:part_view` from the server.
    pub(super) fn ae2_accept_part_view(&mut self, data: &[u8]) {
        let Ok(mut view) = serde_json::from_slice::<PartView>(data) else {
            return;
        };
        view.stacks = view.config.iter().map(|text| horizon_client::item_data::parse_stack(text).ok().flatten()).collect();
        if let Some(r) = self.remote.as_mut() {
            r.ae2_part.view = Some(view);
        }
    }

    fn ae2_part_config_slot(&self, style: &ScreenStyle, index: usize) -> Option<[i32; 4]> {
        let (x, y) = style.slot("CONFIG", index)?;
        Some([x, y, 16, 16])
    }

    /// The filter under `at` that has a slot on this screen, and whether it
    /// is open.
    fn ae2_part_config_at(&self, style: &ScreenStyle, at: [f32; 2]) -> Option<(usize, bool)> {
        let v = self.ae2_part_view()?;
        if is_emitter(&v.kind) {
            return None;
        }
        (0..CONFIG_PLACES).find_map(|i| {
            let [x, y, _, _] = self.ae2_part_config_slot(style, i)?;
            inside([x - 1, y - 1, 18, 18], at).then_some((i, i < v.enabled))
        })
    }

    /// Where menu place `i` goes: the player's slots, the emitter's filter
    /// and the upgrade cards; the others stand behind the screen's widgets.
    pub(super) fn ae2_part_slot_position(&self, i: usize) -> Option<(f32, f32)> {
        let v = self.ae2_part_view()?;
        let style = self.ae2_part_style()?;
        let hidden = (-1000.0, -1000.0);
        if self.ae2_part_key_types_open() {
            return Some(hidden);
        }
        let place = |p: Option<(i32, i32)>| p.map_or(hidden, |(x, y)| (x as f32, y as f32));
        Some(match i {
            0..=8 => place(style.slot("PLAYER_HOTBAR", i)),
            9..=35 => place(style.slot("PLAYER_INVENTORY", i - 9)),
            300 if v.kind == "ae2:level_emitter" => place(style.slot("CONFIG", 0)),
            336..=343 => {
                let k = i - 336;
                if k >= v.upgrades {
                    return Some(hidden);
                }
                let (w, h) = style.size();
                let (px, py) = style.widgets.get("upgrades").map_or((w - 2, 0), |s| s.position.resolve(w, h));
                ((px + 1) as f32, (py + 6 + k as i32 * 18) as f32)
            }
            _ => hidden,
        })
    }

    /// The field of a level emitter, for the search field's press.
    pub(super) fn ae2_part_field_rects(&self) -> Option<([f32; 4], [f32; 4])> {
        let v = self.ae2_part_view()?;
        if !is_emitter(&v.kind) || v.crafting_card {
            return None;
        }
        widget(self.ae2_part_style()?, "levelInput", (103, 12)).map(Self::ae2_field_rects)
    }

    fn ae2_part_number_buttons(&self, style: &ScreenStyle) -> Vec<(usize, [i32; 4])> {
        let Some([x, y, _, _]) = widget(style, "level", (138, 62)) else {
            return Vec::new();
        };
        NUMBER_BUTTONS.into_iter().enumerate().map(|(i, (bx, by, w))| (i, [x + bx, y + by, w, 20])).collect()
    }

    fn ae2_part_tools(&self) -> Vec<Tool> {
        self.ae2_part_view().map_or_else(Vec::new, tools)
    }

    /// A press the screen takes before the menu: the help button, the
    /// settings, the filters, the priority tab, the number entry's buttons.
    /// Right presses on buttons act as left ones.
    pub(super) fn ae2_part_press(&mut self, at: [f32; 2], right: bool) -> bool {
        let (Some(style), Some(v)) = (self.ae2_part_style(), self.ae2_part_view().cloned()) else {
            return false;
        };
        if self.ae2_part_key_types_open() {
            return self.ae2_part_key_types_press(at, &v);
        }
        if inside(toolbar_button(style, 0), at) {
            self.audio.play_button_click();
            self.ae2_open_guide(&v.kind);
            return true;
        }
        for (i, tool) in self.ae2_part_tools().into_iter().enumerate() {
            if inside(toolbar_button(style, i as i32 + 1), at) {
                self.audio.play_button_click();
                if tool.place == KEY_TYPES_BUTTON {
                    if let Some(r) = self.remote.as_mut() {
                        r.ae2_part.key_types = true;
                    }
                } else {
                    self.ae2_send_menu_button(tool.place as i16, i32::from(right));
                }
                return true;
            }
        }
        if !is_emitter(&v.kind)
            && !matches!(v.kind.as_str(), "ae2:import_bus" | "ae2:export_bus")
            && widget(style, "openPriority", (20, 20)).is_some_and(|r| inside(r, at))
        {
            self.audio.play_button_click();
            self.ae2_send_menu_button(PRIORITY_PLACE as i16, 0);
            return true;
        }
        if is_emitter(&v.kind) {
            for (i, rect) in self.ae2_part_number_buttons(style) {
                if inside(rect, at) {
                    if v.crafting_card || right {
                        return true;
                    }
                    self.audio.play_button_click();
                    let input = if self.has_shift_down() || self.has_control_down() { Input::QuickMove } else { Input::Pickup };
                    self.ae2_send_menu_input((45 + i % 4) as i16, i32::from(i >= 4), input);
                    return true;
                }
            }
            return false;
        }
        if let Some((index, open)) = self.ae2_part_config_at(style, at) {
            if open {
                self.ae2_part_config_press(index, i32::from(right));
                if !right && self.carried.is_some() {
                    if let Some(r) = self.remote.as_mut() {
                        r.ae2_part.drag = Some(vec![index]);
                    }
                }
            }
            return true;
        }
        false
    }

    fn ae2_part_config_press(&mut self, index: usize, button: i32) {
        self.ae2_send_menu_button((CONFIG_PLACE + index) as i16, button);
    }

    /// A press dragged over filters sets each once (`AEBaseScreen`'s
    /// `drag_click`).
    pub(super) fn ae2_part_drag(&mut self) {
        if !self.remote.as_ref().is_some_and(|r| r.ae2_part.drag.is_some()) {
            return;
        }
        let Some(style) = self.ae2_part_style() else {
            return;
        };
        let Some((ox, oy)) = self.gui_scale().map(|b| self.inventory_origin(&b)) else {
            return;
        };
        let at = [self.cursor.0 - ox, self.cursor.1 - oy];
        let Some((index, true)) = self.ae2_part_config_at(style, at) else {
            return;
        };
        let seen = self.remote.as_ref().and_then(|r| r.ae2_part.drag.as_ref()).is_some_and(|d| d.contains(&index));
        if !seen {
            if let Some(d) = self.remote.as_mut().and_then(|r| r.ae2_part.drag.as_mut()) {
                d.push(index);
            }
            self.ae2_part_config_press(index, 0);
        }
    }

    pub(super) fn ae2_part_release(&mut self) {
        if let Some(r) = self.remote.as_mut() {
            r.ae2_part.drag = None;
        }
    }

    /// Whether `at` is off the screen, its toolbar, tab and upgrades panel.
    pub(super) fn ae2_part_outside(&self, at: [f32; 2]) -> Option<bool> {
        let style = self.ae2_part_style()?;
        let v = self.ae2_part_view()?;
        if self.ae2_part_key_types_open() {
            let (w, h) = key_types_size();
            let back = self.ae2_part_key_types_style().map(|s| self.ae2_part_key_types_back(s));
            return Some(!inside([0, 0, w, h], at) && !back.is_some_and(|r| inside(r, at)));
        }
        let (w, h) = style.size();
        if inside([0, 0, w, h], at) {
            return Some(false);
        }
        let help = toolbar_button(style, 0);
        let count = 1 + self.ae2_part_tools().len() as i32;
        if inside([help[0] - 4, help[1] - 3, 21, 6 + 22 * count], at) {
            return Some(false);
        }
        if widget(style, "openPriority", (20, 20)).is_some_and(|r| inside(r, at)) {
            return Some(false);
        }
        let panel = [w - 2, 0, 28, 12 + 18 * v.upgrades as i32];
        Some(!(v.upgrades > 0 && inside(panel, at)))
    }

    pub(super) fn ae2_draw_part(&self, batch: &mut HudBatch, ox: f32, oy: f32) {
        let (Some(style), Some(v)) = (self.ae2_part_style(), self.ae2_part_view()) else {
            return;
        };
        if self.ae2_part_key_types_open() {
            self.ae2_draw_part_key_types(batch, ox, oy, v);
            return;
        }
        let atlas = &self.gui.atlas;
        let mouse = [self.cursor.0 - ox, self.cursor.1 - oy];
        self.ae2_draw_background(batch, ox, oy, style);
        let tools = self.ae2_part_tools();
        let help = toolbar_button(style, 0);
        self.ae2_draw_toolbar(batch, ox, oy, help, toolbar_button(style, tools.len() as i32));
        self.ae2_draw_icon_button(batch, ox, oy, help, icon::HELP, mouse);
        for (i, tool) in tools.iter().enumerate() {
            self.ae2_draw_icon_button(batch, ox, oy, toolbar_button(style, i as i32 + 1), tool.icon, mouse);
        }
        if v.upgrades > 0 {
            let (w, h) = style.size();
            let panel = style.widgets.get("upgrades").map_or((w - 2, 0), |s| s.position.resolve(w, h));
            self.ae2_draw_upgrades(batch, ox, oy, panel, v.upgrades as i32);
            // Empty upgrade slots' icon.
            for k in 0..v.upgrades {
                if self.container_kind().is_none_or(|e| e.slots().get(36 + k).is_none_or(Option::is_none)) {
                    self.ae2_icon(batch, icon::BACKGROUND_UPGRADE, (16, 16), ox + (panel.0 + 1) as f32, oy + (panel.1 + 6 + k as i32 * 18) as f32);
                }
            }
        }
        if !is_emitter(&v.kind) {
            if let Some(tab) = widget(style, "openPriority", (20, 20)) {
                self.ae2_draw_tab(batch, ox, oy, tab, icon::PRIORITY);
            }
            // `OptionalFakeSlot`s: the frame, faint while the slot is shut.
            let states = Image { texture: "guis/states".into(), texture_size: (256, 256), src: None };
            let (sx, sy) = icon::SLOT_BACKGROUND;
            for i in 0..CONFIG_PLACES {
                let Some([x, y, _, _]) = self.ae2_part_config_slot(style, i) else {
                    continue;
                };
                let open = i < v.enabled;
                let alpha = if open { 255 } else { 51 };
                self.ae2_blit_to(batch, &states, [sx, sy, 18, 18], [ox + (x - 1) as f32, oy + (y - 1) as f32, 18.0, 18.0], rgba(255, 255, 255, alpha));
                if let Some(stack) = v.stacks.get(i).and_then(Option::as_ref) {
                    self.draw_item_model(batch, stack, ox + x as f32, oy + y as f32);
                }
                if open && inside([x - 1, y - 1, 18, 18], mouse) {
                    batch.fill(atlas, ox + x as f32, oy + y as f32, 16.0, 16.0, rgba(255, 255, 255, 128));
                }
            }
        } else {
            let steps = if self.has_shift_down() || self.has_control_down() { STEPS_64 } else { STEPS_1000 };
            for (i, rect) in self.ae2_part_number_buttons(style) {
                let label = format!("{}{}", if i < 4 { '+' } else { '-' }, steps[i % 4]);
                self.ae2_draw_button(batch, ox, oy, rect, &label, !v.crafting_card, mouse);
            }
            if let (Some(rect), Some(field)) = (widget(style, "levelInput", (103, 12)), self.remote.as_ref().and_then(|r| r.ae2_search.clone())) {
                let unit = if v.fluid { 1000 } else { 1 };
                let valid = super::ae2_number_entry::number_value(&field.value, unit, 0, i64::MAX, '.').is_some();
                self.ae2_draw_field(batch, ox, oy, style, rect, None, field, valid);
                // The number entry's unit, at the field's end.
                let label = if v.kind == "ae2:energy_level_emitter" {
                    Some("AE")
                } else if v.fluid {
                    Some("B")
                } else {
                    None
                };
                if let Some(label) = label {
                    let width = HudBatch::text_width(atlas, label);
                    let (fx, fy) = (ox + (rect[0] + rect[2]) as f32 - width, oy + (rect[1] + (rect[3] - 9) / 2 + 1) as f32);
                    batch.text(atlas, label, fx, fy, rgba(0x55, 0x55, 0x55, 255), false);
                }
            }
        }
        self.ae2_draw_texts(batch, ox, oy, style, &[]);
        // StorageBusScreen.drawFG: what it is attached to, small.
        if v.kind == "ae2:storage_bus" {
            let tr = |key: &str, args: &[String]| fill(&self.translate(key, key), args);
            let text = match v.connected.as_ref() {
                Some(name) => tr("gui.ae2.AttachedTo", &[self.pack.names.display_item(name)]),
                None => tr("gui.ae2.Unattached", &[]),
            };
            let color = argb(style.color("DEFAULT_TEXT_COLOR"));
            let start = batch.vertices.len();
            batch.text(atlas, &text, ox + 10.0, oy + 17.0, color, false);
            batch.scale_since(start, ox + 10.0, oy + 17.0, 0.6, 0.6);
        }
    }

    /// The tooltip under the cursor: a button's, or a filter's stack.
    pub(super) fn ae2_part_tooltip(&mut self, batch: &mut HudBatch) -> bool {
        let (Some(style), Some(v)) = (self.ae2_part_style(), self.ae2_part_view().cloned()) else {
            return false;
        };
        let (ox, oy) = self.inventory_origin(batch);
        let at = [self.cursor.0 - ox, self.cursor.1 - oy];
        let mut lines = Vec::new();
        if self.ae2_part_key_types_open() {
            if self.ae2_part_key_types_style().is_some_and(|s| inside(self.ae2_part_key_types_back(s), at)) {
                let key = format!("item.{}", v.kind.replace(':', "."));
                lines.push(self.translate(&key, &v.kind));
            }
        } else if inside(toolbar_button(style, 0), at) {
            self.ae2_help_tooltip(&mut lines);
        } else if let Some((_, tool)) = self.ae2_part_tools().into_iter().enumerate().find(|(i, _)| inside(toolbar_button(style, *i as i32 + 1), at)) {
            lines.push(self.translate(tool.title, tool.title));
            if !tool.value.is_empty() {
                lines.extend(self.translate(tool.value, tool.value).split('\n').map(|l| format!("\u{a7}7{l}")));
            }
        } else if !is_emitter(&v.kind) && widget(style, "openPriority", (20, 20)).is_some_and(|r| inside(r, at)) {
            lines.push(self.translate("gui.ae2.Priority", "Priority"));
        } else if let Some((index, _)) = self.ae2_part_config_at(style, at) {
            if let Some(stack) = v.stacks.get(index).and_then(Option::as_ref) {
                self.draw_stack_tooltip(batch, stack, self.cursor.0, self.cursor.1);
                return true;
            }
        }
        if lines.is_empty() {
            return false;
        }
        self.ae2_tooltip_lines(batch, lines);
        true
    }

    /// The part's screen for the control socket's `screen`.
    pub(super) fn ae2_part_agent_value(&self, origin: (f32, f32)) -> Option<serde_json::Value> {
        let (style, v) = (self.ae2_part_style()?, self.ae2_part_view()?);
        let rect = |[x, y, w, h]: [i32; 4]| [origin.0 + x as f32, origin.1 + y as f32, w as f32, h as f32];
        let tools: Vec<serde_json::Value> = self
            .ae2_part_tools()
            .into_iter()
            .enumerate()
            .map(|(i, t)| serde_json::json!({"place": t.place, "value": t.value, "rect": rect(toolbar_button(style, i as i32 + 1))}))
            .collect();
        let config = (0..CONFIG_PLACES).filter_map(|i| {
            let [x, y, w, h] = self.ae2_part_config_slot(style, i)?;
            Some(serde_json::json!({"rect": rect([x, y, w, h]), "item": v.stacks.get(i).and_then(Option::as_ref).map(|s| s.item.to_string()), "open": i < v.enabled}))
        });
        let config: Vec<serde_json::Value> = if is_emitter(&v.kind) { Vec::new() } else { config.collect() };
        let numbers: Vec<serde_json::Value> = self.ae2_part_number_buttons(style).into_iter().map(|(i, r)| serde_json::json!({"entry": i, "rect": rect(r)})).collect();
        let tr = self.remote.as_ref().and_then(|r| r.ae2_search.as_ref()).map(|f| f.value.clone());
        let key_types = self.ae2_part_key_types_open().then(|| self.ae2_part_key_types_style()).flatten().map(|s| {
            serde_json::json!({
                "back": rect(self.ae2_part_key_types_back(s)),
                "boxes": self.ae2_part_key_type_rects(s).map(rect),
                "selected": v.key_types,
            })
        });
        Some(serde_json::json!({
            "kind": v.kind,
            "key_types": key_types,
            "help": rect(toolbar_button(style, 0)),
            "tools": tools,
            "config": config,
            "numbers": if is_emitter(&v.kind) { numbers } else { Vec::new() },
            "priority": widget(style, "openPriority", (20, 20)).map(rect),
            "field": self.ae2_part_field_rects().map(|(r, _)| [origin.0 + r[0], origin.1 + r[1], r[2], r[3]]),
            "text": tr,
            "connected": v.connected,
            "upgrades": v.upgrades,
            "enabled": v.enabled,
        }))
    }
}

/// `KeyTypeSelectionScreen.setHeight`: the boxes' top, a box for each of the
/// two key types, and the padding.
fn key_types_size() -> (i32, i32) {
    (KEY_TYPES_WIDTH, 20 + KEY_TYPE_NAMES.len() as i32 * CHECKBOX_SPACING + 6)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(kind: &str) -> PartView {
        PartView { kind: kind.into(), fuzzy_card: true, redstone_card: true, crafting_card: true, ..Default::default() }
    }

    #[test]
    fn toolbars_follow_the_java_screens_order() {
        let places = |v: &PartView| tools(v).iter().map(|t| t.place).collect::<Vec<_>>();
        assert_eq!(places(&view("ae2:storage_bus")), [CLEAR_PLACE, PARTITION_PLACE, 52, 50, 44, 49]);
        assert_eq!(places(&view("ae2:import_bus")), [KEY_TYPES_BUTTON, 42, 44]);
        assert_eq!(places(&view("ae2:export_bus")), [42, 44, 51, 43]);
        assert_eq!(places(&view("ae2:formation_plane")), [53, 44]);
        assert_eq!(places(&view("ae2:level_emitter")), [43, 44]);
        let mut bare = view("ae2:level_emitter");
        (bare.fuzzy_card, bare.crafting_card) = (false, false);
        assert_eq!(places(&bare), [42]);
        let mut export = view("ae2:export_bus");
        (export.redstone_card, export.fuzzy_card, export.crafting_card) = (false, false, false);
        assert_eq!(places(&export), [43]);
    }

    #[test]
    fn the_key_type_screen_is_as_tall_as_its_two_boxes() {
        assert_eq!(key_types_size(), (200, 66));
    }
}
