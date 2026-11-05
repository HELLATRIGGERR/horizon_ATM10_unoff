//! ExtendedAE's Pattern Modifier and Config Modifier (`ItemPatternModifier`,
//! `HostPatternModifier`, `ContainerPatternModifier`, `GuiPatternModifier`,
//! `ItemConfigModifier`, `HostConfigModifier`, `ContainerConfigModifier`),
//! derived from ExtendedAE 1.21-2.2.42, https://github.com/GlodBlock/ExtendedAE
//! commit 1f86f927552f02451236465c0c96a12a3de8f66c, LGPL-3.0; see
//! `ae2/addons/extendedae/`.
//!
//! The Pattern Modifier holds 27 patterns, a target and four blank patterns to
//! clone with, and the two items of a replacement, in its `extendedae:generic_nbt`
//! (`STACK_TAG`). Its menu edits them on four pages; used on a pattern
//! provider (`onItemUseFirst`, not sneaking) it puts its patterns in. The
//! Config Modifier keeps one change (`ConfigSettings`: a mode and a number) in
//! `extendedae:modifier_config_settings`; used on an interface it applies the
//! change to every amount the interface stocks.
use crate::game::{Game, KeyCode, ae2_source_patterns as source, lan::LanHost, shield::OFFHAND_SLOT};
use horizon_client::{
    ae2_generic_stack::GenericStack,
    ae2_world::{Machine, MachineKind},
    ae2x::extendedae as rows,
    containers::BlockEntity,
    items::ItemStack,
    menu::{Input, Kind, Menu},
    save,
};
use horizon_protocol::packets::{configuration::CustomPayload, play::{self, Clientbound as C}};
use horizon_world::nbt::Tag;

/// `HostPatternModifier`: `patternInv` 27, `targetInv` 1, `blankPatternInv`
/// 4, `clonePatternInv` 1, `replaceInv` 2.
pub(in crate::game) const PATTERNS: usize = 27;
pub(in crate::game) const BLANKS: usize = 4;
const BLANK_PATTERN: &str = "ae2:blank_pattern";

/// The Pattern Modifier's inventories.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::game) struct Modifier {
    pub patterns: Vec<Option<ItemStack>>,
    pub target: Option<ItemStack>,
    pub blank: Vec<Option<ItemStack>>,
    pub clone: Option<ItemStack>,
    /// The item replaced and the one it is replaced with (fake slots).
    pub replace: [Option<ItemStack>; 2],
}

/// `AppEngInternalInventory.readFromNBT`: items with their `Slot`.
fn read_inv(tag: Option<&Tag>, size: usize) -> Vec<Option<ItemStack>> {
    let mut slots = vec![None; size];
    if let Some(Tag::List(entries)) = tag {
        for entry in entries {
            let slot = match entry.get("Slot") {
                Some(Tag::Int(n)) => *n as usize,
                Some(Tag::Byte(n)) => *n as usize,
                _ => continue,
            };
            if let Some(slot) = slots.get_mut(slot) {
                *slot = save::item_from_nbt(entry).filter(|s| s.count > 0);
            }
        }
    }
    slots
}
/// `writeToNBT`: none for an empty inventory.
fn write_inv(slots: &[Option<ItemStack>]) -> Option<Tag> {
    let entries: Vec<Tag> = slots
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let mut tag = save::item_to_nbt(s.as_ref()?);
            if let Tag::Compound(map) = &mut tag {
                map.insert("Slot".into(), Tag::Int(i as i32));
            }
            Some(tag)
        })
        .collect();
    (!entries.is_empty()).then_some(Tag::List(entries))
}

impl Modifier {
    pub(in crate::game) fn read(stack: &ItemStack) -> Self {
        let tag = stack.data().and_then(|d| d.other(rows::STACK_TAG));
        let get = |name: &str| tag.and_then(|t| t.get(name));
        let one = |name: &str| read_inv(get(name), 1).pop().flatten();
        let replace = read_inv(get("replaceInv"), 2);
        Self {
            patterns: read_inv(get("patternInv"), PATTERNS),
            target: one("targetInv"),
            blank: read_inv(get("blankPatternInv"), BLANKS),
            clone: one("clonePatternInv"),
            replace: [replace[0].clone(), replace[1].clone()],
        }
    }
    /// `saveChangedInventory`: the inventories that hold anything; the
    /// component goes when none does.
    pub(in crate::game) fn store(&self, stack: &mut ItemStack) {
        let entries: Vec<(&str, Tag)> = [
            ("patternInv", write_inv(&self.patterns)),
            ("targetInv", write_inv(std::slice::from_ref(&self.target))),
            ("blankPatternInv", write_inv(&self.blank)),
            ("clonePatternInv", write_inv(std::slice::from_ref(&self.clone))),
            ("replaceInv", write_inv(&self.replace)),
        ]
        .into_iter()
        .filter_map(|(name, tag)| Some((name, tag?)))
        .collect();
        let value = (!entries.is_empty()).then(|| Tag::compound(entries));
        stack.data_mut().set_other(rows::STACK_TAG, value);
    }

    /// `ContainerPatternModifier.modify`: each processing pattern's inputs and
    /// outputs times (or divided by) `scale`, when every amount divides by it
    /// or stays within 999,999 units.
    pub(in crate::game) fn modify(&mut self, scale: i64, div: bool) {
        if scale <= 0 {
            return;
        }
        for slot in self.patterns.iter_mut() {
            let Some(Ok((inputs, outputs))) = slot.as_ref().and_then(source::processing) else { continue };
            if !(scalable(&inputs, scale, div) && scalable(&outputs, scale, div)) {
                continue;
            }
            let scaled = |list: &[Option<GenericStack>]| list.iter().map(|s| s.as_ref().map(|s| GenericStack { amount: if div { s.amount / scale } else { s.amount * scale }, ..s.clone() })).collect::<Vec<_>>();
            if let Some(next) = processing_pattern(&scaled(&inputs), &scaled(&outputs)) {
                *slot = Some(next);
            }
        }
    }
    /// `clear`: every encoded pattern back to a blank one.
    pub(in crate::game) fn clear(&mut self) {
        for slot in self.patterns.iter_mut() {
            if slot.as_ref().is_some_and(crate::game::ae2_pattern_access::encoded_pattern_item) {
                *slot = Some(ItemStack::new(BLANK_PATTERN, 1));
            }
        }
    }
    /// `clonePattern`: a copy of the target into the clone slot, which takes a
    /// blank pattern when empty and is written over when it holds a pattern.
    pub(in crate::game) fn clone_pattern(&mut self) {
        let Some(target) = self.target.clone().filter(decodable) else { return };
        match &self.clone {
            None => {
                if let Some(blank) = self.blank.iter_mut().find(|s| s.as_ref().is_some_and(|s| s.item.as_ref() == BLANK_PATTERN)) {
                    let stack = blank.as_mut().expect("a blank pattern");
                    stack.count -= 1;
                    if stack.count == 0 {
                        *blank = None;
                    }
                    self.clone = Some(target);
                }
            }
            Some(clone) if crate::game::ae2_pattern_access::encoded_pattern_item(clone) => self.clone = Some(target),
            Some(_) => {}
        }
    }
    /// `replace`: in each processing pattern, every stack of the replaced item
    /// becomes the other (or goes, with none); a crafting pattern takes the
    /// change only when its recipe still makes the same (`valid`, which
    /// `new AECraftingPattern` checks).
    pub(in crate::game) fn replace(&mut self, valid: &dyn Fn(&str, &[Option<ItemStack>], &ItemStack) -> bool) {
        let Some(from) = self.replace[0].as_ref() else { return };
        let Ok(from) = horizon_client::ae2_cell_codec::key(from) else { return };
        let with = self.replace[1].as_ref().and_then(|s| horizon_client::ae2_cell_codec::key(s).ok());
        for slot in self.patterns.iter_mut() {
            let Some(pattern) = slot.as_ref() else { continue };
            if let Some(Ok((inputs, outputs))) = source::processing(pattern) {
                let swap = |list: &[Option<GenericStack>]| {
                    list.iter()
                        .map(|s| match s {
                            Some(s) if !s.fluid && s.key == from => with.clone().map(|key| GenericStack { key, fluid: false, amount: s.amount }),
                            other => other.clone(),
                        })
                        .collect::<Vec<_>>()
                };
                if let Some(next) = processing_pattern(&swap(&inputs), &swap(&outputs)) {
                    *slot = Some(next);
                }
            } else if let Some(Ok(crafting)) = source::crafting(pattern) {
                let grid: Vec<Option<ItemStack>> = crafting
                    .grid
                    .iter()
                    .map(|s| match s {
                        Some(s) if horizon_client::ae2_cell_codec::key(s).ok().as_ref() == Some(&from) => with.as_ref().and_then(|key| horizon_client::ae2_cell_codec::prototype(key).ok()),
                        other => other.clone(),
                    })
                    .collect();
                if !valid(&crafting.recipe, &grid, &crafting.output) {
                    // It is an invalid change: the rest stays as it is.
                    return;
                }
                if let Ok(next) = crate::game::ae2_processing::ae2_encode_pattern_source(&grid, &crafting.output, &crafting.recipe, crafting.substitutes, crafting.fluid_substitutes) {
                    *slot = Some(next);
                }
            }
        }
    }
    /// `changePatternMode`: each crafting pattern's substitutions (mode 0) or
    /// fluid substitutions (mode 1) set to `value`.
    pub(in crate::game) fn pattern_mode(&mut self, mode: u8, value: bool) {
        for slot in self.patterns.iter_mut() {
            let Some(Ok(c)) = slot.as_ref().and_then(source::crafting) else { continue };
            let (substitutes, fluids) = (if mode == 0 { value } else { c.substitutes }, if mode == 1 { value } else { c.fluid_substitutes });
            if let Ok(next) = crate::game::ae2_processing::ae2_encode_pattern_source(&c.grid, &c.output, &c.recipe, substitutes, fluids) {
                *slot = Some(next);
            }
        }
    }
}
/// `AEKeyType.getAmountPerUnit`: an item 1, a fluid a bucket.
fn per_unit(stack: &GenericStack) -> i64 {
    if stack.fluid { 1000 } else { 1 }
}
/// `checkModify`.
fn scalable(stacks: &[Option<GenericStack>], scale: i64, div: bool) -> bool {
    stacks.iter().flatten().all(|s| if div { s.amount % scale == 0 } else { s.amount.saturating_mul(scale) <= 999_999 * per_unit(s) })
}
/// `PatternDetailsHelper.encodeProcessingPattern`: a new pattern of these.
fn processing_pattern(inputs: &[Option<GenericStack>], outputs: &[Option<GenericStack>]) -> Option<ItemStack> {
    let tag = source::encode_processing(inputs, outputs).ok()?;
    let mut pattern = ItemStack::new("ae2:processing_pattern", 1);
    pattern.data_mut().set_other("ae2:encoded_processing_pattern", Some(tag));
    Some(pattern)
}
/// `PatternDetailsHelper.decodePattern(..) != null`.
fn decodable(stack: &ItemStack) -> bool {
    use crate::game::ae2_processing as p;
    p::decode_pattern(stack).is_some() || p::ae2_decode_processing_pattern(stack).is_some() || p::ae2_decode_special_pattern(stack).is_some()
}

/// `ItemConfigModifier.ConfigSettings.Mode`, in its order.
pub(in crate::game) const MODES: [&str; 8] = ["add", "sub", "mul", "div", "max", "min", "set", "rmv"];
/// `ConfigSettings`: the mode's index in [`MODES`] and the number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) struct Settings {
    pub mode: usize,
    pub data: i64,
}
impl Settings {
    /// `ConfigSettings.DEFAULT`: times 1.
    pub(in crate::game) const DEFAULT: Self = Self { mode: 2, data: 1 };
    pub(in crate::game) fn read(stack: &ItemStack) -> Self {
        let Some(tag) = stack.data().and_then(|d| d.other(rows::MODIFIER_SETTINGS)) else { return Self::DEFAULT };
        let mode = match tag.get("mode") {
            Some(Tag::String(m)) => MODES.iter().position(|n| *n == m.to_string_lossy()),
            _ => None,
        };
        let data = match tag.get("data") {
            Some(Tag::Long(v)) => Some(*v),
            Some(Tag::Int(v)) => Some(i64::from(*v)),
            _ => None,
        };
        match (mode, data) {
            (Some(mode), Some(data)) => Self { mode, data },
            _ => Self::DEFAULT,
        }
    }
    pub(in crate::game) fn store(&self, stack: &mut ItemStack) {
        let tag = Tag::compound([("mode", Tag::String(MODES[self.mode].into())), ("data", Tag::Long(self.data))]);
        stack.data_mut().set_other(rows::MODIFIER_SETTINGS, Some(tag));
    }
    /// `ConfigSettings.modify`: the amount a config slot of `value` gets,
    /// between `min` and `max`; -1 removes it.
    pub(in crate::game) fn modify(&self, value: i64, min: i64, max: i64) -> i64 {
        match MODES[self.mode] {
            "add" => value.saturating_add(self.data).min(max),
            "sub" => value.saturating_sub(self.data).max(min),
            "mul" => value.saturating_mul(self.data).min(max),
            "div" => (value / self.data.max(1)).max(min),
            "max" => max,
            "min" => min,
            "set" => self.data.clamp(min, max.max(min)),
            _ => -1,
        }
    }
}

/// An open Pattern Modifier or Config Modifier menu (`MenuLocators.forHand`).
pub(in crate::game) struct ToolMenu {
    window: i32,
    /// The player's slot holding the item.
    source: usize,
    item: &'static str,
    /// `ContainerPatternModifier.page`: 0 multiply, 1 replace, 2 pattern mode, 3 clone.
    page: usize,
    state: i32,
    menu: Menu,
}

/// The Pattern Modifier's window per page: its layout's id, its size, and
/// which window entry holds which slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    Pattern(usize),
    Target,
    Clone,
    Blank(usize),
    Replace(usize),
}
fn page_layout(page: usize) -> &'static str {
    match page {
        1 => "extendedae:pattern_modifier_replace",
        2 => "extendedae:pattern_modifier_mode",
        3 => "extendedae:pattern_modifier_clone",
        _ => rows::PATTERN_MODIFIER,
    }
}
/// Pages 0 and 2 show the 27 patterns in a window of 27; page 1 adds the
/// replacement's two fake slots (entries 27 and 28), page 3 shows the target,
/// the clone and the blanks (27, 28, 29..33) in a window of 54.
fn page_size(page: usize) -> usize {
    if matches!(page, 1 | 3) { 54 } else { 27 }
}
fn place(page: usize, entry: usize) -> Option<Place> {
    match (page, entry) {
        (0..=2, 0..PATTERNS) => Some(Place::Pattern(entry)),
        (1, 27) | (1, 28) => Some(Place::Replace(entry - 27)),
        (3, 27) => Some(Place::Target),
        (3, 28) => Some(Place::Clone),
        (3, 29..33) => Some(Place::Blank(entry - 29)),
        _ => None,
    }
}
fn get(m: &Modifier, p: Place) -> Option<ItemStack> {
    match p {
        Place::Pattern(i) => m.patterns[i].clone(),
        Place::Target => m.target.clone(),
        Place::Clone => m.clone.clone(),
        Place::Blank(i) => m.blank[i].clone(),
        Place::Replace(i) => m.replace[i].clone(),
    }
}
fn set(m: &mut Modifier, p: Place, stack: Option<ItemStack>) {
    match p {
        Place::Pattern(i) => m.patterns[i] = stack,
        Place::Target => m.target = stack,
        Place::Clone => m.clone = stack,
        Place::Blank(i) => m.blank[i] = stack,
        Place::Replace(i) => m.replace[i] = stack,
    }
}
/// What each slot takes (`RestrictedInputSlot`: PROVIDER_PATTERN for the
/// patterns and the target, BLANK_PATTERN for the blanks; `OutputSlot` takes nothing).
fn takes(p: Place, stack: &ItemStack) -> bool {
    match p {
        Place::Pattern(_) | Place::Target => crate::game::ae2_pattern_access::encoded_pattern_item(stack),
        Place::Blank(_) => stack.item.as_ref() == BLANK_PATTERN,
        Place::Clone | Place::Replace(_) => false,
    }
}

/// The Pattern Modifier's buttons, as its window's toggle entries.
pub(in crate::game) const PAGE_BUTTON: i32 = 36;
/// `x n` for the config's four factors, then `÷ n`.
pub(in crate::game) const MULTIPLY_BUTTONS: std::ops::Range<i32> = 37..41;
pub(in crate::game) const DIVIDE_BUTTONS: std::ops::Range<i32> = 41..45;
pub(in crate::game) const CLEAR_BUTTON: i32 = 45;
pub(in crate::game) const REPLACE_BUTTON: i32 = 46;
/// Substitutions on, off; fluid substitutions on, off.
pub(in crate::game) const MODE_BUTTONS: std::ops::Range<i32> = 47..51;
pub(in crate::game) const CLONE_BUTTON: i32 = 51;
/// The Config Modifier's: its mode (cycled), and the number's steps.
pub(in crate::game) const CONFIG_MODE_BUTTON: i32 = 0;
pub(in crate::game) const CONFIG_STEPS: [(i32, i64); 4] = [(1, 1), (2, -1), (3, 10), (4, -10)];

impl Game<'_> {
    /// `ItemPatternModifier.use` and `ItemConfigModifier.use`: their menus
    /// over the item in hand `hand` (0 main, 1 off).
    pub(in crate::game) fn lan_open_extendedae_tool(&mut self, host: &mut LanHost, conn: u32, hand: i32) -> bool {
        let Some(guest) = host.hub.guests.iter().find(|g| g.conn == conn && !g.spectator()) else { return false };
        let source = if hand == 1 { OFFHAND_SLOT } else { guest.selected };
        let item = self.as_guest(host, conn, |g| g.stack_in(source).filter(|s| s.count == 1).map(|s| s.item.clone())).flatten();
        let item = match item.as_deref() {
            Some(rows::PATTERN_MODIFIER) => rows::PATTERN_MODIFIER,
            Some(rows::CONFIG_MODIFIER) => rows::CONFIG_MODIFIER,
            _ => return false,
        };
        self.lan_open_extendedae_tool_page(host, conn, source, item, 0);
        true
    }
    fn lan_open_extendedae_tool_page(&mut self, host: &mut LanHost, conn: u32, source: usize, item: &'static str, page: usize) {
        self.lan_ae2_portable_closed(host, conn, None);
        self.lan_ae2_bus_closed(host, conn, None);
        self.lan_ae2_network_tool_closed(host, conn, None);
        self.lan_ae2_anchor_closed(host, conn, None);
        self.lan_craft_confirm_closed(host, conn, None);
        self.lan_pattern_access_closed(host, conn, None);
        self.lan_ae2_terminal_closed(host, conn, None);
        self.lan_ae2_machine_closed(host, conn, None);
        self.lan_station_closed(host, conn, None);
        self.lan_crafting_closed(host, conn, None);
        self.guest_container_closed(host, conn);
        self.lan_void_cell_closed(host, conn, None);
        self.ae2x.extendedae.tool_menus.remove(&conn);
        let (layout, size) = if item == rows::CONFIG_MODIFIER { (rows::CONFIG_MODIFIER, 27) } else { (page_layout(page), page_size(page)) };
        // The window has no block: the screen goes by its layout's id.
        host.hub.send_to(conn, &C::CustomPayload(CustomPayload { channel: "ae2:menu_block".into(), data: layout.as_bytes().to_vec() }));
        let title = self.pack.names.display_item(item);
        let kind = if size == 54 { "minecraft:generic_9x6" } else { "minecraft:generic_9x3" };
        let Some(window) = host.hub.open_window(conn, kind, horizon_protocol::text::literal(&title)) else { return };
        Self::lan_mark_ae2_base_screen(host, conn, window);
        let menu = Menu::new(Kind::Chest(size / 9));
        self.ae2x.extendedae.tool_menus.insert(conn, ToolMenu { window, source, item, page, state: 0, menu });
        self.lan_send_extendedae_tool(host, conn);
    }
    pub(in crate::game) fn lan_extendedae_tool_closed(&mut self, host: &mut LanHost, conn: u32, window: Option<i32>) -> bool {
        if !self.ae2x.extendedae.tool_menus.get(&conn).is_some_and(|m| window.is_none_or(|w| w == m.window)) {
            return false;
        }
        self.ae2x.extendedae.tool_menus.remove(&conn);
        if let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) {
            self.guest_told_window_closed(host, gi);
        }
        true
    }
    /// The item still in its slot, for a player who is no spectator
    /// (`ItemMenuHost.isValid`); else the menu closes.
    fn lan_extendedae_tool_valid(&mut self, host: &mut LanHost, conn: u32) -> Option<ItemStack> {
        let (window, source, item) = self.ae2x.extendedae.tool_menus.get(&conn).map(|m| (m.window, m.source, m.item))?;
        if !host.hub.guests.iter().any(|g| g.conn == conn && !g.spectator() && g.next_window == window) {
            self.ae2x.extendedae.tool_menus.remove(&conn);
            return None;
        }
        let stack = self.as_guest(host, conn, |g| g.stack_in(source).filter(|s| s.item.as_ref() == item && s.count == 1).cloned()).flatten();
        if stack.is_none() {
            self.lan_extendedae_tool_closed(host, conn, None);
            host.hub.send_to(conn, &C::ContainerClose(play::ContainerClose { container: window }));
        }
        stack
    }
    /// `broadcastChanges`: the window's slots and the player's, the page and,
    /// for the Config Modifier, its mode and number (data slots 0 to 4).
    fn lan_send_extendedae_tool(&mut self, host: &mut LanHost, conn: u32) {
        let Some(stack) = self.lan_extendedae_tool_valid(host, conn) else { return };
        let Some(m) = self.ae2x.extendedae.tool_menus.get_mut(&conn) else { return };
        m.state = (m.state + 1) & 0x7FFF;
        let size = m.menu.slot_count() - 36;
        let mut items: Vec<Option<ItemStack>> = vec![None; size];
        let mut data = vec![0i16; 5];
        if m.item == rows::PATTERN_MODIFIER {
            let inv = Modifier::read(&stack);
            for (entry, slot) in items.iter_mut().enumerate() {
                *slot = place(m.page, entry).and_then(|p| get(&inv, p));
            }
            data[0] = m.page as i16;
        } else {
            let settings = Settings::read(&stack);
            data[0] = settings.mode as i16;
            let n = settings.data.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
            data[1] = n as i16;
            data[2] = (n >> 16) as i16;
        }
        let Some(g) = host.hub.guests.iter().find(|g| g.conn == conn) else { return };
        items.extend((9..36).chain(0..9).map(|i| g.inventory[i].clone()));
        let carried = g.carried.clone();
        let packet = C::ContainerSetContent(play::ContainerSetContent {
            container: m.window,
            state: m.state,
            items: items.iter().map(|s| host.hub.stack(s)).collect(),
            carried: host.hub.stack(&carried),
        });
        let window = m.window;
        host.hub.send_to(conn, &packet);
        host.hub.inventory_told_by(conn, &items);
        for (id, value) in data.into_iter().enumerate() {
            host.hub.send_to(conn, &C::ContainerSetData(play::ContainerSetData { container: window, id: id as i16, value }));
        }
    }
    /// Each tick: a menu whose item left its slot closes.
    pub(in crate::game) fn lan_refresh_extendedae_tools(&mut self, host: &mut LanHost) {
        let conns: Vec<u32> = self.ae2x.extendedae.tool_menus.keys().copied().collect();
        for conn in conns {
            if host.hub.guests.iter().any(|g| g.conn == conn && !host.hub.here(g)) {
                continue;
            }
            let _ = self.lan_extendedae_tool_valid(host, conn);
        }
    }

    /// A click in the Pattern Modifier's window: its slots as their
    /// `RestrictedInputSlot`s, `OutputSlot` and `SingleFakeSlot`s, the player's
    /// inventory around them; the item's own slot takes no click.
    pub(in crate::game) fn lan_extendedae_tool_click(&mut self, host: &mut LanHost, conn: u32, p: &play::ContainerClick) -> bool {
        let Some((window, source, item, page)) = self.ae2x.extendedae.tool_menus.get(&conn).map(|m| (m.window, m.source, m.item, m.page)) else { return false };
        if p.container != window {
            return false;
        }
        let Some(stack) = self.lan_extendedae_tool_valid(host, conn) else { return true };
        let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) else { return true };
        let input = crate::game::lan_crafting::input(p.input);
        let button = i32::from(p.button);
        let size = page_size(page) as i16;
        let source_entry = if source < 9 { size + 27 + source as i16 } else if source < 36 { size + source as i16 - 9 } else { -1 };
        let locked = item != rows::PATTERN_MODIFIER || p.slot == source_entry || (input == Input::Swap && (button as usize == source || (button == 40 && source == OFFHAND_SLOT)));
        if locked {
            self.lan_send_extendedae_tool(host, conn);
            return true;
        }
        let mut inv = Modifier::read(&stack);
        let target = usize::try_from(p.slot).ok().filter(|s| *s < size as usize);
        // A fake slot: the cursor's item, one of it, or nothing (`SingleFakeSlot`).
        if let Some(Place::Replace(i)) = target.and_then(|e| place(page, e)) {
            let carried = host.hub.guests[gi].carried.clone().map(|mut s| {
                s.count = 1;
                s
            });
            inv.replace[i] = carried;
            let _ = self.as_guest(host, conn, |g| g.stack_in_mut(source).as_mut().map(|s| inv.store(s)));
            self.lan_send_extendedae_tool(host, conn);
            return true;
        }
        if target.is_some_and(|e| place(page, e).is_none()) {
            self.lan_send_extendedae_tool(host, conn);
            return true;
        }
        // Shift from the player's side: into the first slot of this page that
        // takes it (AEBaseMenu.quickMoveStack).
        if input == Input::QuickMove && target.is_none() {
            let index = match usize::try_from(p.slot).ok().map(|s| s - size as usize) {
                Some(s @ 0..27) => 9 + s,
                Some(s @ 27..36) => s - 27,
                _ => {
                    self.lan_send_extendedae_tool(host, conn);
                    return true;
                }
            };
            let moving = host.hub.guests[gi].inventory[index].clone();
            if let Some(mut moving) = moving {
                for entry in 0..size as usize {
                    let Some(at) = place(page, entry).filter(|at| takes(*at, &moving)) else { continue };
                    match get(&inv, at) {
                        None => {
                            let room = if matches!(at, Place::Blank(_)) { 64 } else { 1 };
                            let n = moving.count.min(room);
                            set(&mut inv, at, Some(ItemStack { count: n, ..moving.clone() }));
                            moving.count -= n;
                        }
                        Some(mut there) if there.same_item_same_components(&moving) && matches!(at, Place::Blank(_)) => {
                            let n = moving.count.min(64u32.saturating_sub(there.count));
                            there.count += n;
                            moving.count -= n;
                            set(&mut inv, at, Some(there));
                        }
                        Some(_) => {}
                    }
                    if moving.count == 0 {
                        break;
                    }
                }
                host.hub.guests[gi].inventory[index] = (moving.count > 0).then_some(moving);
                let _ = self.as_guest(host, conn, |g| g.stack_in_mut(source).as_mut().map(|s| inv.store(s)));
            }
            self.lan_send_extendedae_tool(host, conn);
            return true;
        }
        // Any other click as a chest's, then each slot checked: a change no
        // slot would take puts everything back.
        let Some(mut menu) = self.ae2x.extendedae.tool_menus.get_mut(&conn).map(|m| std::mem::replace(&mut m.menu, Menu::new(Kind::Chest(1)))) else { return true };
        {
            let env = self.menu_env();
            for entry in 0..size as usize {
                menu.set_slot(&env, entry, place(page, entry).and_then(|at| get(&inv, at)));
            }
        }
        let before_inventory = host.hub.guests[gi].inventory.clone();
        let before_carried = host.hub.guests[gi].carried.clone();
        self.guest_menu_op(host, gi, &mut menu, |m, env| {
            let _ = m.click(env, i32::from(p.slot), button, input);
        });
        let mut next = inv.clone();
        let mut ok = true;
        for entry in 0..size as usize {
            let after = menu.slot(entry).cloned();
            match place(page, entry) {
                Some(at) => {
                    let was = get(&inv, at);
                    let fine = after == was
                        || after.is_none()
                        || (takes(at, after.as_ref().expect("a stack")) && after.as_ref().is_some_and(|s| s.count <= if matches!(at, Place::Blank(_)) { 64 } else { 1 }));
                    ok &= fine;
                    set(&mut next, at, after);
                }
                None => ok &= after.is_none(),
            }
        }
        if ok {
            let _ = self.as_guest(host, conn, |g| g.stack_in_mut(source).as_mut().map(|s| next.store(s)));
        } else {
            host.hub.guests[gi].inventory = before_inventory;
            host.hub.guests[gi].carried = before_carried;
        }
        if let Some(m) = self.ae2x.extendedae.tool_menus.get_mut(&conn) {
            m.menu = menu;
        }
        self.lan_send_extendedae_tool(host, conn);
        true
    }

    /// A press on the modifiers' buttons (`CEAEGenericPacket`, `CUpdatePage`).
    pub(in crate::game) fn lan_extendedae_tool_button(&mut self, host: &mut LanHost, conn: u32, window: i32, id: i32) -> bool {
        let Some((source, item, page)) = self.ae2x.extendedae.tool_menus.get(&conn).filter(|m| m.window == window).map(|m| (m.source, m.item, m.page)) else { return false };
        let Some(stack) = self.lan_extendedae_tool_valid(host, conn) else { return true };
        let reverse = id & crate::game::ae2_screens::REVERSE != 0;
        let entry = id & !crate::game::ae2_screens::REVERSE;
        if item == rows::CONFIG_MODIFIER {
            let mut settings = Settings::read(&stack);
            if entry == CONFIG_MODE_BUTTON {
                // ConfigSettings.Mode.getNext; backwards on the other button.
                settings.mode = if reverse { (settings.mode + MODES.len() - 1) % MODES.len() } else { (settings.mode + 1) % MODES.len() };
            } else if let Some((_, step)) = CONFIG_STEPS.iter().find(|(e, _)| *e == entry) {
                settings.data = settings.data.saturating_add(*step).max(0);
            }
            let _ = self.as_guest(host, conn, |g| g.stack_in_mut(source).as_mut().map(|s| settings.store(s)));
            self.lan_send_extendedae_tool(host, conn);
            return true;
        }
        if entry == PAGE_BUTTON {
            // CUpdatePage: (page + 1) % 4; the window changes with it.
            let next = if reverse { (page + 3) % 4 } else { (page + 1) % 4 };
            self.lan_open_extendedae_tool_page(host, conn, source, item, next);
            return true;
        }
        let mut inv = Modifier::read(&stack);
        let config = rows::Config::get();
        match entry {
            e if MULTIPLY_BUTTONS.contains(&e) && page == 0 => inv.modify(config.modifier_multiplier((e - MULTIPLY_BUTTONS.start) as usize), false),
            e if DIVIDE_BUTTONS.contains(&e) && page == 0 => inv.modify(config.modifier_multiplier((e - DIVIDE_BUTTONS.start) as usize), true),
            CLEAR_BUTTON if page == 0 => inv.clear(),
            REPLACE_BUTTON if page == 1 => {
                let recipes = self.res.recipes();
                inv.replace(&|recipe, grid, output| recipes.assemble_named(recipe, grid, 3).as_ref() == Some(output));
            }
            e if MODE_BUTTONS.contains(&e) && page == 2 => {
                let i = e - MODE_BUTTONS.start;
                inv.pattern_mode((i / 2) as u8, i % 2 == 0);
            }
            CLONE_BUTTON if page == 3 => inv.clone_pattern(),
            _ => {}
        }
        let _ = self.as_guest(host, conn, |g| g.stack_in_mut(source).as_mut().map(|s| inv.store(s)));
        self.lan_send_extendedae_tool(host, conn);
        true
    }

    /// `onItemUseFirst` of the held Pattern Modifier or Config Modifier on
    /// block `pos`, side `face`: the Pattern Modifier (not sneaking) puts its
    /// patterns into the pattern provider there, each one that fits whole
    /// (`getPatternInv().addItems`); the Config Modifier changes every amount
    /// the interface there stocks. True when the item acted.
    pub(in crate::game) fn extendedae_tool_use_on(&mut self, pos: [i32; 3], face: [i32; 3]) -> bool {
        if self.remote.is_some() || self.spectator() {
            return false;
        }
        let Some(held) = self.held().cloned() else { return false };
        match held.item.as_ref() {
            rows::PATTERN_MODIFIER if !self.keys.contains(&KeyCode::ShiftLeft) => self.extendedae_modifier_fill(pos, face, held),
            rows::CONFIG_MODIFIER => self.extendedae_config_apply(pos, face, &held),
            _ => false,
        }
    }
    /// `FCUtil.findDevice`: the machine at `pos` of `kind`, a block or the
    /// part on the face hit, with where to write it back.
    fn extendedae_device_at(&self, pos: [i32; 3], face: [i32; 3], kind: MachineKind) -> Option<(Machine, Option<crate::game::ae2_parts::PartAddress>)> {
        let key = (pos[0], pos[1], pos[2]);
        let BlockEntity::Ae2(machine) = self.world.block_entities.get(&key)? else { return None };
        if machine.kind == kind {
            return Some(((**machine).clone(), None));
        }
        if machine.kind != MachineKind::CableBus {
            return None;
        }
        // Face bits: east, west, up, down, south, north.
        let index = match face {
            [1, 0, 0] => 0,
            [-1, 0, 0] => 1,
            [0, 1, 0] => 2,
            [0, -1, 0] => 3,
            [0, 0, 1] => 4,
            [0, 0, -1] => 5,
            _ => return None,
        };
        let address = crate::game::ae2_parts::PartAddress { host: self.ae2_local_address(key)?, face: index };
        let part = self.ae2_part_machine(address).ok().flatten().filter(|m| m.kind == kind)?;
        Some((part, Some(address)))
    }
    fn extendedae_store_device(&mut self, pos: [i32; 3], machine: Machine, part: Option<crate::game::ae2_parts::PartAddress>) -> bool {
        match part {
            Some(address) => self.ae2_store_part_machine(address, &machine).is_ok(),
            None => {
                let key = (pos[0], pos[1], pos[2]);
                let Some(BlockEntity::Ae2(old)) = self.world.block_entities.get_mut(&key) else { return false };
                **old = machine;
                self.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
                true
            }
        }
    }
    fn extendedae_modifier_fill(&mut self, pos: [i32; 3], face: [i32; 3], held: ItemStack) -> bool {
        let Some((mut provider, part)) = self.extendedae_device_at(pos, face, MachineKind::PatternProvider) else { return false };
        let mut inv = Modifier::read(&held);
        let mut moved = false;
        for slot in inv.patterns.iter_mut() {
            let Some(pattern) = slot.clone() else { continue };
            if !crate::game::ae2_pattern_access::provider_pattern_valid(&provider, &pattern) {
                continue;
            }
            let Some(free) = provider.slots.iter().position(Option::is_none) else { break };
            provider.slots[free] = Some(pattern);
            *slot = None;
            moved = true;
        }
        if moved && self.extendedae_store_device(pos, provider, part) {
            if let Some(stack) = self.held_mut().as_mut() {
                inv.store(stack);
            }
            match part {
                Some(address) => self.ae2_provider_patterns_changed(address.host, Some(address.face)),
                None => self.ae2_provider_patterns_changed_local((pos[0], pos[1], pos[2]), None),
            }
        }
        // SUCCESS or FAIL: the block's own use does not follow either way.
        true
    }
    fn extendedae_config_apply(&mut self, pos: [i32; 3], face: [i32; 3], held: &ItemStack) -> bool {
        let Some((mut interface, part)) = self.extendedae_device_at(pos, face, MachineKind::Interface) else { return false };
        let settings = Settings::read(held);
        let Ok(mut items) = crate::game::ae2_interface::interface_config(&interface) else { return true };
        let Ok(mut fluids) = crate::game::ae2_interface::interface_fluid_config(&interface) else { return true };
        // CONFIG_STACKS: at least 1; an item up to its stack size (at most 99),
        // a fluid four buckets (useRegisteredCapacities).
        for slot in items.iter_mut() {
            let Some(stack) = slot.as_mut() else { continue };
            let max = i64::from(self.res.items().get(&stack.item).map_or(64, |i| i.max_stack).clamp(1, 99));
            let amount = settings.modify(i64::from(stack.count), 1, max);
            if amount < 0 {
                *slot = None;
            } else {
                stack.count = amount as u32;
            }
        }
        for slot in fluids.iter_mut() {
            let Some((_, amount)) = slot.as_mut() else { continue };
            let next = settings.modify(*amount as i64, 1, crate::game::ae2_interface::INTERFACE_FLUID_CAPACITY as i64);
            if next < 0 {
                *slot = None;
            } else {
                *amount = next as u64;
            }
        }
        for (i, stack) in items.iter().enumerate() {
            if let Some((key, amount)) = fluids[i].clone() {
                let _ = crate::game::ae2_interface::set_interface_fluid_config_amount(&mut interface, i, Some((&key, amount)));
            } else {
                let _ = crate::game::ae2_interface::set_interface_config(&mut interface, i, stack.as_ref());
            }
        }
        // findName: the block's (or part's) name.
        let name = self.pack.names.display_item(interface.id());
        self.extendedae_store_device(pos, interface, part);
        self.system_message(horizon_client::text::Text::translate("chat.config_modifier.success", &[horizon_client::text::Text::literal(name)]));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ae2_cell_core::ItemKey;

    fn item(id: &str, amount: i64) -> Option<GenericStack> {
        Some(GenericStack { key: horizon_client::ae2_cell_codec::key(&ItemStack::new(id, 1)).unwrap(), fluid: false, amount })
    }
    fn fluid(id: &str, amount: i64) -> Option<GenericStack> {
        let key = horizon_client::ae2_cell_codec::key(&ItemStack::new("minecraft:stone", 1)).unwrap();
        Some(GenericStack { key: ItemKey { item_id: id.into(), ..key }, fluid: true, amount })
    }

    #[test]
    fn the_modifier_scales_processing_patterns_within_their_limits() {
        let mut inv = Modifier { patterns: vec![None; PATTERNS], blank: vec![None; BLANKS], ..Default::default() };
        inv.patterns[0] = processing_pattern(&[item("minecraft:iron_ore", 2), None, fluid("minecraft:water", 1000)], &[item("minecraft:iron_ingot", 3)]);
        inv.patterns[1] = processing_pattern(&[item("minecraft:stone", 999_999)], &[item("minecraft:dirt", 1)]);
        inv.modify(2, false);
        let (inputs, outputs) = source::processing(inv.patterns[0].as_ref().unwrap()).unwrap().unwrap();
        assert_eq!((inputs[0].as_ref().unwrap().amount, inputs[1].clone(), inputs[2].as_ref().unwrap().amount), (4, None, 2000));
        assert_eq!(outputs[0].as_ref().unwrap().amount, 6);
        // 999,999 x 2 is past the limit: unchanged.
        assert_eq!(source::processing(inv.patterns[1].as_ref().unwrap()).unwrap().unwrap().0[0].as_ref().unwrap().amount, 999_999);
        // ÷ 3 does not divide 4: unchanged; ÷ 2 does.
        inv.modify(3, true);
        assert_eq!(source::processing(inv.patterns[0].as_ref().unwrap()).unwrap().unwrap().0[0].as_ref().unwrap().amount, 4);
        inv.modify(2, true);
        assert_eq!(source::processing(inv.patterns[0].as_ref().unwrap()).unwrap().unwrap().1[0].as_ref().unwrap().amount, 3);

        // The item keeps it all in `extendedae:generic_nbt`.
        let mut stack = ItemStack::new(rows::PATTERN_MODIFIER, 1);
        inv.store(&mut stack);
        assert_eq!(Modifier::read(&stack), inv);
        Modifier { patterns: vec![None; PATTERNS], blank: vec![None; BLANKS], ..Default::default() }.store(&mut stack);
        assert!(stack.data().and_then(|d| d.other(rows::STACK_TAG)).is_none());
    }

    #[test]
    fn replace_clear_and_clone() {
        let mut inv = Modifier { patterns: vec![None; PATTERNS], blank: vec![None; BLANKS], ..Default::default() };
        inv.patterns[0] = processing_pattern(&[item("minecraft:iron_ore", 2), item("minecraft:coal", 1)], &[item("minecraft:iron_ingot", 2)]);
        inv.replace = [Some(ItemStack::new("minecraft:coal", 1)), Some(ItemStack::new("minecraft:charcoal", 1))];
        inv.replace(&|_, _, _| true);
        let (inputs, _) = source::processing(inv.patterns[0].as_ref().unwrap()).unwrap().unwrap();
        assert_eq!(inputs[1].as_ref().unwrap().key.item_id, "minecraft:charcoal");
        // Replaced with nothing: the stack goes.
        inv.replace = [Some(ItemStack::new("minecraft:charcoal", 1)), None];
        inv.replace(&|_, _, _| true);
        assert_eq!(source::processing(inv.patterns[0].as_ref().unwrap()).unwrap().unwrap().0[1], None);

        // Clone: a blank pattern used, then the clone written over.
        inv.target = inv.patterns[0].clone();
        inv.clone_pattern();
        assert_eq!(inv.clone, None, "no blank pattern, no clone");
        inv.blank[2] = Some(ItemStack::new(BLANK_PATTERN, 2));
        inv.clone_pattern();
        assert_eq!((inv.clone.clone(), inv.blank[2].as_ref().map(|s| s.count)), (inv.target.clone(), Some(1)));
        inv.clone_pattern();
        assert_eq!(inv.blank[2].as_ref().map(|s| s.count), Some(1), "a clone over a clone takes none");

        inv.patterns[5] = Some(ItemStack::new("minecraft:stone", 3));
        inv.clear();
        assert_eq!(inv.patterns[0].as_ref().map(|s| s.item.to_string()), Some(BLANK_PATTERN.to_owned()));
        assert_eq!(inv.patterns[5].as_ref().map(|s| s.item.to_string()), Some("minecraft:stone".to_owned()));
    }

    #[test]
    fn config_settings_modify_as_the_mod_does() {
        let s = |mode: &str, data| Settings { mode: MODES.iter().position(|m| *m == mode).unwrap(), data };
        assert_eq!(Settings::DEFAULT, s("mul", 1));
        assert_eq!(s("mul", 2).modify(40, 1, 64), 64);
        assert_eq!(s("mul", 2).modify(10, 1, 64), 20);
        assert_eq!(s("add", 5).modify(62, 1, 64), 64);
        assert_eq!(s("sub", 5).modify(3, 1, 64), 1);
        assert_eq!(s("div", 0).modify(9, 1, 64), 9, "divided by at least 1");
        assert_eq!(s("div", 4).modify(9, 1, 64), 2);
        assert_eq!((s("max", 0).modify(9, 1, 64), s("min", 0).modify(9, 1, 64)), (64, 1));
        assert_eq!(s("set", 100).modify(9, 1, 64), 64);
        assert_eq!(s("rmv", 0).modify(9, 1, 64), -1);
        let mut stack = ItemStack::new(rows::CONFIG_MODIFIER, 1);
        assert_eq!(Settings::read(&stack), Settings::DEFAULT);
        s("set", 12).store(&mut stack);
        assert_eq!(Settings::read(&stack), s("set", 12));
    }

    #[test]
    fn the_extended_terminal_sends_crafting_patterns_to_assemblers_only() {
        horizon_client::ae2x::install();
        let mut grid = vec![None; 9];
        grid[0] = Some(ItemStack::new("minecraft:oak_log", 1));
        let crafting = crate::game::ae2_processing::ae2_encode_pattern_source(&grid, &ItemStack::new("minecraft:oak_planks", 4), "minecraft:oak_planks", false, false).unwrap();
        let processing = processing_pattern(&[item("minecraft:stone", 1)], &[item("minecraft:dirt", 1)]).unwrap();
        let icon = |id: &str| horizon_client::ae2_cell_codec::key(&ItemStack::new(id, 1)).unwrap();
        use super::super::pattern_quick_move_target as target;
        for assembler in ["ae2:molecular_assembler", rows::EX_ASSEMBLER, rows::MATRIX_PATTERN] {
            assert!(target(&crafting, &icon(assembler)) && !target(&processing, &icon(assembler)), "{assembler}");
        }
        assert!(!target(&crafting, &icon("ae2:drive")) && target(&processing, &icon("ae2:drive")));
        assert!(super::super::pattern_terminal_extended(rows::EX_PATTERN_TERMINAL) && super::super::pattern_terminal_extended(rows::WIRELESS_EX_PAT));
        assert!(!super::super::pattern_terminal_extended("ae2:pattern_access_terminal"));
    }

    #[test]
    fn each_page_shows_its_slots() {
        assert_eq!((page_size(0), page_size(1), page_size(2), page_size(3)), (27, 54, 27, 54));
        assert_eq!(place(0, 26), Some(Place::Pattern(26)));
        assert_eq!(place(1, 28), Some(Place::Replace(1)));
        assert_eq!((place(3, 0), place(3, 27), place(3, 28), place(3, 32)), (None, Some(Place::Target), Some(Place::Clone), Some(Place::Blank(3))));
        assert!(takes(Place::Pattern(0), &ItemStack::new("ae2:processing_pattern", 1)));
        assert!(!takes(Place::Pattern(0), &ItemStack::new("minecraft:stone", 1)));
        assert!(takes(Place::Blank(0), &ItemStack::new(BLANK_PATTERN, 1)) && !takes(Place::Clone, &ItemStack::new(BLANK_PATTERN, 1)));
    }
}
