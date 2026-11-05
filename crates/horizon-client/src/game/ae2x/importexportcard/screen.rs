//! AE2 Import Export Card's screens on this client: the terminal's toolbar
//! buttons (`MEStorageScreenMixin`, `UpgradeItemButton`) and the Import and
//! Export Cards' `UpgradeScreen` (`screens/import_card.json`,
//! `export_card.json`, the backgrounds `textures/gui/upgrade_<n>.png` by its
//! Capacity Cards).
//! They cover the open wireless terminal as AE2's sub screens do, its menu
//! open underneath standing for `UpgradeContainerMenu`'s; the server keeps
//! everything in the terminal's card (`ae2importexportcard:card_menu`). The
//! addon's GUI textures are carried under `assets/ae2importexportcard/ae2/textures/ae2importexportcard/`.
//! Derived from AE2 Import Export Card by Ultramega,
//! https://github.com/starforcraft/AE2-Import-Export-Card branch `1.21.1`
//! (1.9.1), MIT; see `ae2/addons/ae2importexportcard/`.
use super::CARD_MENU;
use crate::game::{
    Game,
    ae2_screens::{argb, icon, inside},
};
use horizon_client::{
    ae2_screen_style::{Image, ScreenStyle},
    ae2x::importexportcard::{self as iec, CardType},
    hud::HudBatch,
    items::ItemStack,
    text::Text,
};

const NAMESPACE: &str = "ae2importexportcard";
/// The two card screens' styles (0 import, 1 export).
const STYLES: [&str; 2] = ["import_card.json", "export_card.json"];
/// `UpgradeScreen.BACKGROUNDS`: 191 wide, taller by a row for each Capacity Card.
const WIDTH: i32 = 191;
const HEIGHTS: [i32; 4] = [163, 181, 199, 217];
/// The textures, 16 x 16 (`textures/gui/*.png`).
const CHECKMARK: &str = "ae2importexportcard/gui/checkmark";
const XMARK: &str = "ae2importexportcard/gui/xmark";
const MASS_SELECT: &str = "ae2importexportcard/gui/mass_select";
const BUTTONS: [&str; 2] = ["ae2importexportcard/gui/import_card", "ae2importexportcard/gui/export_card"];
/// The filter's slot semantics (`ModSlotSemantics`).
const CONFIG: [&str; 2] = ["AE2IMPORTEXPORTCARD_IMPORT_CONFIG", "AE2IMPORTEXPORTCARD_EXPORT_CONFIG"];
/// `Color.GREEN`.
const GREEN: u32 = 0xFF00_FF00;
/// AE2's `UpgradesPanel`: 18 a slot, 5 of padding.
const PANEL_SLOT: i32 = 18;
const PANEL_PADDING: i32 = 5;

/// A card screen as the server's view of the terminal's card shows it.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::game) struct CardView {
    filters: Vec<Option<(String, bool)>>,
    selected: [i32; iec::SELECTED_INVENTORY_SLOT_COUNT],
    upgrades: Vec<Option<String>>,
    fuzzy: String,
    capacity: usize,
    fuzzy_card: bool,
}
fn card_view(view: &serde_json::Value, ty: CardType) -> Option<CardView> {
    let v = view.get("iecard")?.get(if ty == CardType::Import { "import" } else { "export" }).filter(|v| v.is_object())?;
    let mut selected = [0; iec::SELECTED_INVENTORY_SLOT_COUNT];
    for (slot, value) in selected.iter_mut().zip(v.get("selected").and_then(|s| s.as_array()).into_iter().flatten()) {
        *slot = value.as_i64().unwrap_or(0) as i32;
    }
    let filters = v
        .get("filters")
        .and_then(|f| f.as_array())
        .map_or_else(Vec::new, |f| f.iter().map(|e| Some((e.get("id")?.as_str()?.to_owned(), e.get("fluid")?.as_bool()?))).collect());
    let upgrades = v.get("upgrades").and_then(|u| u.as_array()).map_or_else(Vec::new, |u| u.iter().map(|e| e.as_str().map(str::to_owned)).collect());
    Some(CardView {
        filters,
        selected,
        upgrades,
        fuzzy: v.get("fuzzy").and_then(|f| f.as_str()).unwrap_or("IGNORE_ALL").to_owned(),
        capacity: v.get("capacity").and_then(serde_json::Value::as_u64).unwrap_or(0).min(3) as usize,
        fuzzy_card: v.get("fuzzy_card").and_then(serde_json::Value::as_bool).unwrap_or(false),
    })
}
impl CardView {
    /// `getActiveFilterSlotCount`.
    fn filter_slots(&self) -> usize {
        iec::BASE_FILTER_SLOT_COUNT + self.capacity * iec::FILTER_SLOTS_PER_CAPACITY_CARD
    }
}

/// What a part's tooltip shows: lines, or a stack's own tooltip.
pub(in crate::game) enum Tooltip {
    Text(String),
    Stack(ItemStack),
}

/// A part of a card screen a press can take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Back,
    Fuzzy,
    /// A filter slot.
    Filter(u8),
    /// The card's own slot.
    Upgrade(u8),
    /// An inventory slot: 0..35, the armour 36..39.
    Player(u8),
    /// The mass select buttons: 0 the inventory, 1 the hotbar.
    MassSelect(u8),
}

fn fuzzy_icon(name: &str) -> (i32, i32) {
    match name {
        "PERCENT_99" => icon::FUZZY_PERCENT_99,
        "PERCENT_75" => icon::FUZZY_PERCENT_75,
        "PERCENT_50" => icon::FUZZY_PERCENT_50,
        "PERCENT_25" => icon::FUZZY_PERCENT_25,
        _ => icon::FUZZY_IGNORE,
    }
}
fn fuzzy_tooltip(name: &str) -> &'static str {
    match name {
        "PERCENT_99" => "gui.tooltips.ae2.FZPercent_99",
        "PERCENT_75" => "gui.tooltips.ae2.FZPercent_75",
        "PERCENT_50" => "gui.tooltips.ae2.FZPercent_50",
        "PERCENT_25" => "gui.tooltips.ae2.FZPercent_25",
        _ => "gui.tooltips.ae2.FZIgnoreAll",
    }
}
fn ty_of(which: u8) -> Option<CardType> {
    CardType::from_id(which)
}

impl Game<'_> {
    fn iecard_card(&self, which: u8) -> Option<CardView> {
        card_view(self.ae2_term_addon_view()?, ty_of(which)?)
    }

    /// `MEStorageScreenMixin`: the buttons the terminal's cards show
    /// (`setVisibility(getInstalledUpgrades(card) > 0)`).
    pub(in crate::game) fn iecard_buttons(&self) -> Vec<u8> {
        let mut buttons = Vec::new();
        for which in 0..2 {
            if self.iecard_card(which).is_some() {
                buttons.push(which);
            }
        }
        buttons
    }
    /// A toolbar button pressed: the card's screen over the terminal.
    pub(in crate::game) fn iecard_open(&mut self, which: u8) {
        let selected = self.iecard_card(which).map(|v| v.selected);
        self.ae2x.importexportcard.selected = selected.map(|s| (which, s));
        if let Some(r) = self.remote.as_mut() {
            r.ae2_term.iecard = Some(which);
            if let Some(field) = r.ae2_search.as_mut() {
                field.focused = false;
            }
        }
    }
    /// The back button (`AESubScreen.goBack`, `switchToScreen(parent)`).
    fn iecard_close(&mut self) {
        self.ae2x.importexportcard.selected = None;
        if let Some(r) = self.remote.as_mut() {
            r.ae2_term.iecard = None;
        }
    }
    /// The open card screen, while the terminal still holds its card.
    pub(in crate::game) fn iecard_sub(&self) -> Option<u8> {
        let which = self.remote.as_ref()?.ae2_term.iecard?;
        self.iecard_card(which).is_some().then_some(which)
    }
    pub(in crate::game) fn iecard_style(&self, which: u8) -> Option<&'static ScreenStyle> {
        self.ae2_addon_style(NAMESPACE, STYLES.get(usize::from(which)).copied()?)
    }
    /// `updateImageHeight`: the card screen's background follows its
    /// Capacity Cards.
    pub(in crate::game) fn iecard_size(&self, which: u8) -> Option<(i32, i32)> {
        Some((WIDTH, HEIGHTS[self.iecard_card(which)?.capacity]))
    }
    fn iecard_place(&self, which: u8, semantic: &str, index: usize) -> Option<(i32, i32)> {
        let style = self.iecard_style(which)?;
        let (w, h) = self.iecard_size(which)?;
        let slot = style.slots.get(semantic).filter(|s| !s.hidden)?;
        let (x, y) = slot.position.resolve(w, h);
        let (dx, dy) = slot.grid.map_or((0, 0), |g| g.offset(index));
        Some((x + dx, y + dy))
    }
    /// Where the terminal's menu place `i` goes: the player's inventory at
    /// the style's slots; the terminal's own places are hidden.
    pub(in crate::game) fn iecard_slot_position(&self, which: u8, i: usize) -> Option<(f32, f32)> {
        let at = match i {
            0..=8 => self.iecard_place(which, "PLAYER_HOTBAR", i),
            9..=35 => self.iecard_place(which, "PLAYER_INVENTORY", i - 9),
            _ => None,
        }?;
        Some((at.0 as f32, at.1 as f32))
    }
    /// The armour slots (`ArmorSlot` at x 2, below the filters), head first:
    /// `Inventory` slot 39 down to 36.
    fn iecard_armor_place(capacity: usize, k: usize) -> (i32, i32) {
        (2, 70 + capacity as i32 * 18 + k as i32 * 18)
    }
    /// `getMassSelectY`.
    fn iecard_mass_select_y(capacity: usize) -> i32 {
        76 + capacity as i32 * 18
    }
    /// The style's `back` widget (`TabButton`, 20 x 20).
    fn iecard_back(&self, which: u8) -> Option<[i32; 4]> {
        let style = self.iecard_style(which)?;
        let (w, h) = self.iecard_size(which)?;
        let widget = style.widgets.get("back")?;
        let (x, y) = widget.position.resolve(w, h);
        Some([x, y, widget.width.max(20), widget.height.max(20)])
    }
    /// The `upgrades` widget (`common.json`: two from the right edge).
    fn iecard_panel(&self, which: u8) -> Option<(i32, i32)> {
        let style = self.iecard_style(which)?;
        let (w, h) = self.iecard_size(which)?;
        Some(style.widgets.get("upgrades").map_or((w - 2, 0), |s| s.position.resolve(w, h)))
    }

    /// The parts of the open screen, from its top left.
    fn iecard_parts(&self, which: u8) -> Vec<(Part, [i32; 4])> {
        let mut parts = Vec::new();
        if let Some(back) = self.iecard_back(which) {
            parts.push((Part::Back, back));
        }
        let Some(card) = self.iecard_card(which) else { return parts };
        // The left toolbar: the fuzzy mode, with the Fuzzy Card.
        if card.fuzzy_card {
            let (x, y) = self.iecard_style(which).and_then(|s| s.widgets.get("verticalToolbar")).map_or((18, 1), |w| w.position.resolve(WIDTH, 0));
            parts.push((Part::Fuzzy, [x - 2 - 16, y + 2, 16, 16]));
        }
        let semantic = CONFIG[usize::from(which)];
        for index in 0..card.filter_slots().min(iec::MAX_FILTER_SLOT_COUNT) {
            if let Some((x, y)) = self.iecard_place(which, semantic, index) {
                parts.push((Part::Filter(index as u8), [x, y, 16, 16]));
            }
        }
        if let Some((px, py)) = self.iecard_panel(which) {
            for index in 0..card.upgrades.len() {
                let (col, row) = (index as i32 / 8, index as i32 % 8);
                parts.push((Part::Upgrade(index as u8), [px + 1 + col * PANEL_SLOT, py + PANEL_PADDING + 1 + row * PANEL_SLOT, 16, 16]));
            }
        }
        for i in 0..36 {
            if let Some((x, y)) = self.iecard_slot_position(which, i) {
                parts.push((Part::Player(i as u8), [x as i32, y as i32, 16, 16]));
            }
        }
        for k in 0..4 {
            let (x, y) = Self::iecard_armor_place(card.capacity, k);
            parts.push((Part::Player((39 - k) as u8), [x, y, 16, 16]));
        }
        let mass = Self::iecard_mass_select_y(card.capacity);
        parts.push((Part::MassSelect(0), [24 + 160, mass + 1, 4, 5]));
        parts.push((Part::MassSelect(1), [24 + 160, mass + 1 + 16 * 3 + 10, 4, 5]));
        parts
    }

    /// `UpgradeScreen.sendUpdate`: the whole selection.
    fn iecard_send_selection(&mut self, which: u8) {
        let Some((open, selected)) = self.ae2x.importexportcard.selected else { return };
        if open != which {
            return;
        }
        let mut data = vec![0, which];
        data.extend(selected.iter().map(|v| (*v).clamp(0, 255) as u8));
        self.ae2_send_payload(CARD_MENU, data);
    }
    /// The open selection, as the screen keeps it.
    fn iecard_selected(&mut self, which: u8) -> Option<&mut [i32; iec::SELECTED_INVENTORY_SLOT_COUNT]> {
        let fresh = self.iecard_card(which)?.selected;
        let state = &mut self.ae2x.importexportcard.selected;
        if state.is_none_or(|(open, _)| open != which) {
            *state = Some((which, fresh));
        }
        state.as_mut().map(|(_, s)| s)
    }
    /// The stack in `Inventory` slot `index` on this client.
    fn iecard_inventory_stack(&self, index: usize) -> Option<&ItemStack> {
        match index {
            0..=35 => self.inventory.get(index)?.as_ref(),
            36..=39 => self.armor.get(39 - index)?.as_ref(),
            _ => None,
        }
    }

    /// A press on the open card screen; it takes every press inside it but
    /// those on the inventory with an item on the cursor (`super.mouseClicked`:
    /// the menu's own click).
    pub(in crate::game) fn iecard_sub_press(&mut self, which: u8, at: [f32; 2], right: bool) -> bool {
        let Some((part, _)) = self.iecard_parts(which).into_iter().find(|(_, r)| inside(*r, at)) else { return true };
        let button = u8::from(right);
        match part {
            Part::Back => {
                self.audio.play_button_click();
                self.iecard_close();
            }
            Part::Fuzzy => {
                self.audio.play_button_click();
                self.ae2_send_payload(CARD_MENU, vec![2, which, button]);
            }
            Part::Filter(index) => self.ae2_send_payload(CARD_MENU, vec![1, which, index, button]),
            Part::Upgrade(index) => self.ae2_send_payload(CARD_MENU, vec![4, which, index, button]),
            Part::Player(index) => {
                if self.carried.is_some() {
                    return false;
                }
                let index = usize::from(index);
                if self.has_shift_down() {
                    if self.iecard_inventory_stack(index).is_some() {
                        self.ae2_send_payload(CARD_MENU, vec![5, which, index as u8]);
                    }
                    return true;
                }
                let (Some(ty), Some(card)) = (ty_of(which), self.iecard_card(which)) else { return true };
                let filters = card.filter_slots();
                let Some(selected) = self.iecard_selected(which) else { return true };
                selected[index] = if right { 0 } else { iec::increase_selected(ty, selected[index], filters) };
                self.iecard_send_selection(which);
                self.audio.play_button_click();
            }
            Part::MassSelect(region) => {
                let (Some(ty), Some(card)) = (ty_of(which), self.iecard_card(which)) else { return true };
                let filters = card.filter_slots();
                let Some(selected) = self.iecard_selected(which) else { return true };
                let range = if region == 1 { 0..9 } else { 9..36 };
                for i in range {
                    selected[i] = if right { 0 } else { iec::increase_selected(ty, selected[i], filters) };
                }
                self.iecard_send_selection(which);
                self.audio.play_button_click();
            }
        }
        true
    }

    /// The screen: `drawBG`, its labels, back button, fuzzy button, filter
    /// slots' keys and the card's slots (`UpgradesPanel`).
    pub(in crate::game) fn iecard_draw_sub(&self, batch: &mut HudBatch, which: u8, ox: f32, oy: f32) {
        let (Some(style), Some((w, h))) = (self.iecard_style(which), self.iecard_size(which)) else { return };
        let atlas = &self.gui.atlas;
        let mouse = [self.cursor.0 - ox, self.cursor.1 - oy];
        let text_color = argb(style.color("DEFAULT_TEXT_COLOR"));
        let label = |id: &str| -> Option<(String, (i32, i32))> {
            let text = style.text.get(id)?;
            let key = text.text.get("translate")?.as_str()?;
            Some((self.translate(key, key), text.position.resolve(w, h)))
        };
        let Some(card) = self.iecard_card(which) else { return };
        let image = Image { texture: format!("ae2importexportcard/gui/upgrade_{}", card.capacity), texture_size: (256, 256), src: None };
        self.ae2_blit(batch, &image, [0, 0, w, h], ox, oy);
        for id in ["dialog_title", "player_inventory_title"] {
            if let Some((text, (x, y))) = label(id) {
                batch.text(atlas, &text, ox + x as f32, oy + y as f32, text_color, false);
            }
        }
        if let Some((px, py)) = self.iecard_panel(which) {
            self.ae2_draw_upgrades(batch, ox, oy, (px, py), card.upgrades.len() as i32);
        }
        for (part, rect) in self.iecard_parts(which) {
            let (x, y) = (ox + rect[0] as f32, oy + rect[1] as f32);
            match part {
                Part::Back => self.ae2_draw_tab(batch, ox, oy, rect, icon::BACK),
                Part::Fuzzy => self.ae2_draw_icon_button(batch, ox, oy, rect, fuzzy_icon(&card.fuzzy), mouse),
                Part::Filter(index) => {
                    if let Some(Some((id, fluid))) = card.filters.get(usize::from(index)) {
                        self.iecard_draw_key(batch, id, *fluid, x, y);
                    }
                }
                Part::Upgrade(index) => {
                    if let Some(Some(id)) = card.upgrades.get(usize::from(index)) {
                        self.draw_item(batch, &ItemStack::new(id, 1), x, y);
                    }
                }
                Part::Player(index) if index >= 36 => {
                    if let Some(stack) = self.iecard_inventory_stack(usize::from(index)) {
                        self.draw_item(batch, stack, x, y);
                    }
                }
                _ => {}
            }
        }
    }
    /// A filter's key: the item, or the fluid as AE2's wrapped stack draws it.
    fn iecard_draw_key(&self, batch: &mut HudBatch, id: &str, fluid: bool, x: f32, y: f32) {
        let stack = if fluid {
            let key = horizon_client::ae2_cell_codec::fluid_key(id);
            horizon_client::ae2_generic_stack::wrap(&horizon_client::ae2_generic_stack::GenericStack { key, fluid: true, amount: 0 }).ok()
        } else {
            Some(ItemStack::new(id, 1))
        };
        if let Some(stack) = stack {
            self.draw_item(batch, &stack, x, y);
        }
    }

    /// `drawFG` over the slots' items: the Export Card's filter numbers, each
    /// inventory slot's mark (a check or its filter's number, an X when off),
    /// the mass select buttons.
    pub(in crate::game) fn iecard_draw_foreground(&self, batch: &mut HudBatch, which: u8, ox: f32, oy: f32) {
        let (Some(ty), Some(card)) = (ty_of(which), self.iecard_card(which)) else { return };
        let selected = self.ae2x.importexportcard.selected.filter(|(open, _)| *open == which).map_or(card.selected, |(_, s)| s);
        let atlas = &self.gui.atlas;
        let mark = |batch: &mut HudBatch, texture: &str, x: f32, y: f32| {
            self.ae2_blit(batch, &Image { texture: texture.into(), texture_size: (16, 16), src: None }, [0, 0, 16, 16], x, y);
        };
        let number = |batch: &mut HudBatch, n: i32, x: f32, y: f32| {
            let text = n.to_string();
            let width = HudBatch::text_width(atlas, &text);
            let start = batch.vertices.len();
            batch.text(atlas, &text, x + 32.0 - width, y, GREEN, true);
            batch.scale_since(start, x, y, 0.5, 0.5);
        };
        for (part, rect) in self.iecard_parts(which) {
            let (x, y) = (ox + rect[0] as f32, oy + rect[1] as f32);
            match part {
                Part::Filter(index) if ty == CardType::Export => number(batch, i32::from(index) + 1, x, y),
                Part::Player(index) => match (ty, selected[usize::from(index)]) {
                    (_, 0) | (_, i32::MIN..0) => mark(batch, XMARK, x, y),
                    (CardType::Import, _) => mark(batch, CHECKMARK, x, y),
                    (CardType::Export, n) => number(batch, n, x, y),
                },
                Part::MassSelect(region) => {
                    let top = Self::iecard_mass_select_y(card.capacity) + if region == 1 { 16 * 3 + 10 } else { 0 };
                    mark(batch, MASS_SELECT, ox + 183.0, oy + top as f32);
                }
                _ => {}
            }
        }
    }

    /// The hovered part's tooltip: the back and fuzzy buttons, a filter's key,
    /// the card's cards and its compatible ones.
    pub(in crate::game) fn iecard_tooltip(&self, which: u8, at: [f32; 2]) -> Option<Tooltip> {
        let (part, _) = self.iecard_parts(which).into_iter().find(|(_, r)| inside(*r, at))?;
        Some(Tooltip::Text(match part {
            Part::Back => self.translate("gui.ae2.Back", "Back"),
            Part::Fuzzy => {
                let card = self.iecard_card(which)?;
                format!("{}\n{}", self.translate("gui.tooltips.ae2.FuzzyMode", "Fuzzy Comparison"), self.translate(fuzzy_tooltip(&card.fuzzy), ""))
            }
            // A filter's key: the item's own tooltip, a fluid's name.
            Part::Filter(index) => {
                let (id, fluid) = self.iecard_card(which)?.filters.get(usize::from(index))?.clone()?;
                if !fluid {
                    return Some(Tooltip::Stack(ItemStack::new(&id, 1)));
                }
                self.translate(&format!("block.{}", id.replace(':', ".")), &id)
            }
            Part::Upgrade(index) => match self.iecard_card(which)?.upgrades.get(usize::from(index))?.clone() {
                Some(id) => return Some(Tooltip::Stack(ItemStack::new(&id, 1))),
                None => self.translate("gui.ae2.CompatibleUpgrades", "Compatible Upgrades"),
            },
            _ => return None,
        }))
    }

    /// `MEStorageScreenMixin`'s button (`UpgradeItemButton`): the toolbar's
    /// backdrop, the card's texture over it.
    pub(in crate::game) fn iecard_draw_button(&self, batch: &mut HudBatch, ox: f32, oy: f32, rect: [i32; 4], which: u8, mouse: [f32; 2]) {
        let hover = inside(rect, mouse);
        let lift = if hover { 1.0 } else { 0.0 };
        let (x, y) = (ox + rect[0] as f32, oy + rect[1] as f32);
        self.ae2_icon(batch, if hover { icon::TOOLBAR_BUTTON_BACKGROUND_HOVER } else { icon::TOOLBAR_BUTTON_BACKGROUND }, (18, 20), x - 1.0, y + lift);
        let image = Image { texture: BUTTONS[usize::from(which.min(1))].into(), texture_size: (16, 16), src: None };
        self.ae2_blit(batch, &image, [0, 0, 16, 16], x, y + 1.0 + lift);
    }
    /// Its message: the card's name.
    pub(in crate::game) fn iecard_button_tooltip(&self, which: u8) -> String {
        match which {
            0 => self.translate("item.ae2importexportcard.import_card", "Import Card"),
            _ => self.translate("item.ae2importexportcard.export_card", "Export Card"),
        }
    }

    /// The open card screen for the control socket's `screen`: its parts in
    /// GUI pixels and what the card holds.
    pub(in crate::game) fn iecard_agent(&self, which: u8, origin: (f32, f32)) -> serde_json::Value {
        let rect = |[x, y, w, h]: [i32; 4]| [origin.0 + x as f32, origin.1 + y as f32, w as f32, h as f32];
        let parts: Vec<serde_json::Value> = self
            .iecard_parts(which)
            .into_iter()
            .map(|(part, r)| serde_json::json!({"part": format!("{part:?}"), "rect": rect(r)}))
            .collect();
        let card = self.iecard_card(which);
        serde_json::json!({
            "which": which,
            "parts": parts,
            "selected": card.as_ref().map(|c| c.selected.to_vec()),
            "filters": card.as_ref().map(|c| c.filters.iter().map(|f| f.as_ref().map(|(id, _)| id.clone())).collect::<Vec<_>>()),
            "upgrades": card.as_ref().map(|c| c.upgrades.clone()),
            "fuzzy": card.as_ref().map(|c| c.fuzzy.clone()),
        })
    }
    /// The hovered part's tooltip drawn (true then), after the marks.
    pub(in crate::game) fn iecard_draw_over(&mut self, batch: &mut HudBatch, which: u8) -> bool {
        let (ox, oy) = self.inventory_origin(batch);
        self.iecard_draw_foreground(batch, which, ox, oy);
        let (x, y) = self.cursor;
        match self.iecard_tooltip(which, [x - ox, y - oy]) {
            None => false,
            Some(Tooltip::Stack(stack)) => {
                self.draw_stack_tooltip(batch, &stack, x, y);
                true
            }
            Some(Tooltip::Text(text)) => {
                let lines: Vec<(Text, u32)> = text.lines().map(|l| (Text::legacy(l), u32::MAX)).collect();
                self.draw_tooltip_lines(batch, &lines, x, y);
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_view_carries_the_cards_screens() {
        let view = serde_json::json!({"iecard": {
            "import": {"filters": [{"id": "minecraft:dirt", "fluid": false}, null], "selected": [1, 0, 1], "upgrades": ["ae2:capacity_card", null, null, null], "fuzzy": "PERCENT_75", "capacity": 1, "fuzzy_card": false},
            "export": null,
        }});
        let import = card_view(&view, CardType::Import).unwrap();
        assert_eq!(import.filters[0], Some(("minecraft:dirt".to_owned(), false)));
        assert_eq!(import.filters[1], None);
        assert_eq!(&import.selected[..3], &[1, 0, 1]);
        assert_eq!(import.selected[39], 0);
        assert_eq!(import.filter_slots(), 27);
        assert_eq!(import.fuzzy, "PERCENT_75");
        assert!(card_view(&view, CardType::Export).is_none(), "no Export Card in the terminal");
        assert_eq!(fuzzy_icon(&import.fuzzy), icon::FUZZY_PERCENT_75);
    }
}
