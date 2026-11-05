//! ExtendedAE's Infinity and Void Cells in the game. Derived from ExtendedAE
//! 1.21-2.2.42 (`ItemInfinityCell`, `InfinityCellInventory`, `ItemVoidCell`,
//! `VoidCellInventory`, `VoidMode`), https://github.com/GlodBlock/ExtendedAE
//! commit 1f86f927552f02451236465c0c96a12a3de8f66c, LGPL-3.0; see
//! `ae2/addons/extendedae/`.
//!
//! An infinity cell is a creative cell of its one key to the storage code
//! (`horizon_client::ae2x::fixed_cell`): it gives that key without end and
//! takes it in, voided, and nothing else. A void cell is a cell of the mod's
//! own handler ([`VoidCell`]): what its partition lets in is gone, turned into
//! matter balls or singularities by its mode as a Condenser would.
use super::super::super::{Game, ae2_storage_devices::storage_fuzzy_match, ae2x::HandlerCell, lan::LanHost, shield::OFFHAND_SLOT};
use ae2_cell_core::{Action, ItemKey};
use horizon_client::{
    ae2_generic_stack::{self, GenericStack},
    ae2x::extendedae as rows,
    items::ItemStack,
    text::Text,
};
use horizon_world::nbt::Tag;

/// `EAESingletons.VOID_MODE` and `VOID_ENERGY`, the cell's components.
pub(in crate::game) const VOID_MODE: &str = "extendedae:void_mode";
pub(in crate::game) const VOID_ENERGY: &str = "extendedae:void_energy";
/// AE2's `STORAGE_CELL_INV`: what the cell holds of its output.
const STORED: &str = "ae2:storage_cell_inv";

/// `VoidMode`, by its serialized name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum Mode {
    Trash,
    MatterBalls,
    Singularity,
}
impl Mode {
    pub(in crate::game) const ALL: [Mode; 3] = [Mode::Trash, Mode::MatterBalls, Mode::Singularity];
    pub(in crate::game) fn name(self) -> &'static str {
        match self {
            Self::Trash => "TRASH",
            Self::MatterBalls => "MATTER_BALLS",
            Self::Singularity => "SINGULARITY",
        }
    }
    /// The cell's mode: `getOrDefault(VOID_MODE, TRASH)`.
    pub(in crate::game) fn of(stack: &ItemStack) -> Self {
        match stack.data().and_then(|d| d.other(VOID_MODE)) {
            Some(Tag::String(s)) => Self::ALL.into_iter().find(|m| s.to_string_lossy() == m.name()).unwrap_or(Self::Trash),
            _ => Self::Trash,
        }
    }
    /// `VoidMode.output` and `getPower`: the item and the AE2 Condenser's
    /// power for one of it (`condenser.matterBalls`, `.singularity`).
    fn output(self) -> Option<(&'static str, f64)> {
        let power = horizon_client::ae2_common_config::condenser();
        match self {
            Self::Trash => None,
            Self::MatterBalls => Some(("ae2:matter_ball", power.matter_balls)),
            Self::Singularity => Some(("ae2:singularity", power.singularity)),
        }
    }
}

/// `VoidCellInventory` over its stack.
pub(in crate::game) struct VoidCell {
    /// The partition's item keys; `partitioned` also counts its fluids.
    filters: Vec<ItemKey>,
    partitioned: bool,
    /// The fuzzy mode, with a fuzzy card.
    fuzzy: Option<String>,
    /// With an inverter card the partition is a blacklist.
    inverted: bool,
    mode: Mode,
    energy: f64,
    stored: Vec<(ItemKey, u64)>,
    /// The default damage most of each filter's item, for fuzzy matching.
    max_damage: Vec<u32>,
}
impl VoidCell {
    /// `isPartitioned`.
    pub(in crate::game) fn is_partitioned(&self) -> bool {
        self.partitioned
    }
    /// `partitionList.matchesFilter(what, partitionListMode)`.
    fn matches(&self, key: &ItemKey) -> bool {
        let listed = self.filters.iter().zip(&self.max_damage).any(|(filter, max)| match &self.fuzzy {
            Some(mode) => storage_fuzzy_match(filter, key, mode, false, *max),
            None => filter == key,
        });
        listed != self.inverted
    }
    /// `fillOutput`: the void energy, at the output's power each, becomes
    /// output items; trash keeps none.
    fn fill_output(&mut self) {
        let Some((id, power)) = self.mode.output().filter(|(_, p)| *p > 0.0) else {
            self.energy = 0.0;
            return;
        };
        let made = (self.energy / power).floor();
        if made >= 1.0 {
            let Ok(key) = horizon_client::ae2_cell_codec::key(&ItemStack::new(id, 1)) else { return };
            match self.stored.iter_mut().find(|(k, _)| *k == key) {
                Some((_, n)) => *n = n.saturating_add(made as u64),
                None => self.stored.push((key, made as u64)),
            }
            self.energy -= made * power;
        }
    }
}
impl HandlerCell for VoidCell {
    fn available_stacks(&mut self) -> Vec<(ItemKey, u64)> {
        self.stored.clone()
    }
    /// Anything the partition lets in is taken whole (nothing without a
    /// partition); for real, its units become void energy.
    fn insert(&mut self, key: &ItemKey, amount: u64, action: Action) -> u64 {
        if amount == 0 || !self.partitioned || !self.matches(key) {
            return 0;
        }
        if action == Action::Modulate {
            self.energy += amount as f64;
            self.fill_output();
        }
        amount
    }
    fn extract(&mut self, key: &ItemKey, amount: u64, action: Action) -> u64 {
        let Some(at) = self.stored.iter().position(|(k, n)| k == key && *n > 0) else { return 0 };
        let taken = amount.min(self.stored[at].1);
        if action == Action::Modulate {
            self.stored[at].1 -= taken;
            if self.stored[at].1 == 0 {
                self.stored.remove(at);
            }
        }
        taken
    }
    fn is_preferred_storage_for(&self, key: &ItemKey) -> bool {
        self.partitioned && self.matches(key)
    }
    /// `persist`: the stored outputs and the energy left, each removed when none.
    fn store(&self, stack: &mut ItemStack) -> Result<(), String> {
        let entries = self
            .stored
            .iter()
            .filter(|(_, n)| *n > 0)
            .map(|(key, n)| ae2_generic_stack::encode(&GenericStack { key: key.clone(), fluid: false, amount: i64::try_from(*n).unwrap_or(i64::MAX) }))
            .collect::<Result<Vec<_>, _>>()?;
        let data = stack.data_mut();
        data.set_other(STORED, (!entries.is_empty()).then_some(Tag::List(entries)));
        data.set_other(VOID_ENERGY, (self.energy > 0.0).then(|| Tag::Double(self.energy.to_bits())));
        Ok(())
    }
}

/// One player's open Void Cell menu (`ContainerVoidCell`, `MenuLocators.forHand`).
#[derive(Clone, Copy, Debug)]
pub(in crate::game) struct VoidMenu {
    window: i32,
    /// The player's slot holding the cell.
    source: usize,
    /// The mode the screen was last told (`@GuiSync(1) mode`).
    shown: Option<i16>,
}

/// `ItemVoidCell.getUpgrades(is)`: whether a card of `id` is in.
fn card_installed(stack: &ItemStack, id: &str) -> bool {
    horizon_client::ae2_upgrade_inventory::Inventory::read(stack).is_ok_and(|cards| cards.slots.iter().flatten().any(|card| card.item.as_ref() == id))
}

impl Game<'_> {
    /// `VoidCellInventory.HANDLER.getCellInventory`: a void cell's inventory,
    /// its keys checked against the game's items.
    pub(in crate::game) fn extendedae_void_cell(&self, stack: &ItemStack) -> Result<VoidCell, String> {
        if stack.item.as_ref() != rows::VOID_CELL || stack.count != 1 {
            return Err("not a void cell".into());
        }
        let known = |id: &str, fluid: bool| self.ae2_known_cell_key(id, fluid);
        let settings = horizon_client::ae2_cell_codec::cell_settings_checked(stack, &known)?;
        let Some(Tag::List(entries)) = settings.get(&"filters".into()) else { return Err("invalid void cell filters".into()) };
        let mut filters = Vec::new();
        let mut partitioned = false;
        for entry in horizon_client::ae2_cell_codec::decode_view_filter_slots(entries)?.into_iter().flatten() {
            partitioned = true;
            if let Some(item) = entry.get("item") {
                let item = horizon_client::save::item_from_nbt(item).ok_or("invalid void cell filter")?;
                filters.push(horizon_client::ae2_cell_codec::key(&item)?);
            }
        }
        let max_damage = filters.iter().map(|k| self.res.items().get(&k.item_id).map_or(0, |item| item.max_damage)).collect();
        let fuzzy = card_installed(stack, "ae2:fuzzy_card").then(|| match settings.get(&"fuzzy_mode".into()) {
            Some(Tag::String(s)) => s.to_string_lossy(),
            _ => "ignore_all".into(),
        });
        let mut stored = Vec::new();
        match stack.data().and_then(|d| d.other(STORED)) {
            None => {}
            Some(Tag::List(entries)) => {
                for entry in entries {
                    if let Some(value) = ae2_generic_stack::decode_tolerant_checked(entry, known)?
                        && !value.fluid
                        && value.amount > 0
                    {
                        stored.push((value.key, value.amount as u64));
                    }
                }
            }
            Some(_) => return Err("invalid void cell contents".into()),
        }
        let energy = match stack.data().and_then(|d| d.other(VOID_ENERGY)) {
            Some(Tag::Double(bits)) => f64::from_bits(*bits).max(0.0),
            _ => 0.0,
        };
        Ok(VoidCell { filters, partitioned, fuzzy, inverted: card_installed(stack, "ae2:inverter_card"), mode: Mode::of(stack), energy, stored, max_damage })
    }

    /// `ItemVoidCell.use`: on the server, `ContainerVoidCell` over the cell in
    /// hand `hand` (0 main, 1 off). Its screen is the layout of the cell's id
    /// (`screens/void_cell.json`): the three modes' buttons, the mode's line.
    pub(in crate::game) fn lan_open_void_cell(&mut self, host: &mut LanHost, conn: u32, hand: i32) -> bool {
        use horizon_protocol::packets::{configuration::CustomPayload, play::Clientbound as C};
        let Some(guest) = host.hub.guests.iter().find(|g| g.conn == conn && !g.spectator()) else { return false };
        let source = if hand == 1 { OFFHAND_SLOT } else { guest.selected };
        let held = self.as_guest(host, conn, |g| g.stack_in(source).is_some_and(|s| s.item.as_ref() == rows::VOID_CELL && s.count == 1));
        if held != Some(true) {
            return false;
        }
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
        // The window has no block: the screen goes by the item.
        host.hub.send_to(conn, &C::CustomPayload(CustomPayload { channel: "ae2:menu_block".into(), data: rows::VOID_CELL.as_bytes().to_vec() }));
        let title = self.pack.names.display_item(rows::VOID_CELL);
        let Some(window) = host.hub.open_window(conn, "minecraft:generic_9x3", horizon_protocol::text::literal(&title)) else {
            return true;
        };
        Self::lan_mark_ae2_base_screen(host, conn, window);
        self.ae2x.extendedae.void_menus.insert(conn, VoidMenu { window, source, shown: None });
        self.lan_send_void_cell(host, conn);
        true
    }
    pub(in crate::game) fn lan_void_cell_closed(&mut self, host: &mut LanHost, conn: u32, window: Option<i32>) -> bool {
        if !self.ae2x.extendedae.void_menus.get(&conn).is_some_and(|m| window.is_none_or(|w| w == m.window)) {
            return false;
        }
        self.ae2x.extendedae.void_menus.remove(&conn);
        if let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) {
            self.guest_told_window_closed(host, gi);
        }
        true
    }
    /// The cell still in its slot, for a player who is no spectator
    /// (`ItemMenuHost.isValid`); else the menu closes.
    fn lan_void_cell_valid(&mut self, host: &mut LanHost, conn: u32) -> Option<ItemStack> {
        let menu = *self.ae2x.extendedae.void_menus.get(&conn)?;
        if !host.hub.guests.iter().any(|g| g.conn == conn && !g.spectator() && g.next_window == menu.window) {
            self.ae2x.extendedae.void_menus.remove(&conn);
            return None;
        }
        let stack = self.as_guest(host, conn, |g| g.stack_in(menu.source).filter(|s| s.item.as_ref() == rows::VOID_CELL && s.count == 1).cloned()).flatten();
        if stack.is_none() {
            use horizon_protocol::packets::play::{self, Clientbound as C};
            self.lan_void_cell_closed(host, conn, None);
            host.hub.send_to(conn, &C::ContainerClose(play::ContainerClose { container: menu.window }));
        }
        stack
    }
    /// `broadcastChanges`: the cell's mode as data slot 0, when it changed;
    /// the window's slots (none of its own) and the player's inventory.
    fn lan_send_void_cell(&mut self, host: &mut LanHost, conn: u32) {
        use horizon_protocol::packets::play::{self, Clientbound as C};
        let Some(stack) = self.lan_void_cell_valid(host, conn) else { return };
        let Some(menu) = self.ae2x.extendedae.void_menus.get(&conn).copied() else { return };
        let mode = Mode::ALL.iter().position(|m| *m == Mode::of(&stack)).unwrap_or(0) as i16;
        if menu.shown.is_none() {
            let Some(g) = host.hub.guests.iter().find(|g| g.conn == conn) else { return };
            let mut items: Vec<Option<ItemStack>> = vec![None; 27];
            items.extend((9..36).chain(0..9).map(|i| g.inventory[i].clone()));
            let carried = g.carried.clone();
            let packet = C::ContainerSetContent(play::ContainerSetContent {
                container: menu.window,
                state: 0,
                items: items.iter().map(|s| host.hub.stack(s)).collect(),
                carried: host.hub.stack(&carried),
            });
            host.hub.send_to(conn, &packet);
            host.hub.inventory_told_by(conn, &items);
        }
        if menu.shown != Some(mode) {
            host.hub.send_to(conn, &C::ContainerSetData(play::ContainerSetData { container: menu.window, id: 0, value: mode }));
            if let Some(m) = self.ae2x.extendedae.void_menus.get_mut(&conn) {
                m.shown = Some(mode);
            }
        }
    }
    /// A press on the screen's mode buttons (`CEAEGenericPacket("set", mode)`,
    /// `ContainerVoidCell.setMode`): the cell takes that mode. True when it
    /// was this menu's window.
    pub(in crate::game) fn lan_void_cell_button(&mut self, host: &mut LanHost, conn: u32, window: i32, id: i32) -> bool {
        let Some(menu) = self.ae2x.extendedae.void_menus.get(&conn).copied().filter(|m| m.window == window) else { return false };
        if self.lan_void_cell_valid(host, conn).is_none() {
            return true;
        }
        let entry = id & !super::super::super::ae2_screens::REVERSE;
        if let Some(mode) = usize::try_from(entry).ok().and_then(|i| Mode::ALL.get(i)).copied() {
            let _ = self.as_guest(host, conn, |g| {
                if let Some(stack) = g.stack_in_mut(menu.source).as_mut() {
                    stack.data_mut().set_other(VOID_MODE, Some(Tag::String(mode.name().into())));
                }
            });
        }
        self.lan_send_void_cell(host, conn);
        true
    }
    /// Each tick: a menu whose cell left its slot closes; a changed mode is sent.
    pub(in crate::game) fn lan_refresh_void_cells(&mut self, host: &mut LanHost) {
        let conns: Vec<u32> = self.ae2x.extendedae.void_menus.keys().copied().collect();
        for conn in conns {
            if host.hub.guests.iter().any(|g| g.conn == conn && !host.hub.here(g)) {
                continue;
            }
            self.lan_send_void_cell(host, conn);
        }
    }

    /// `ItemInfinityCell.appendHoverText` and `ItemVoidCell.appendHoverText`.
    pub(in crate::game) fn extendedae_hover_lines(&self, stack: &ItemStack, translate: &dyn Fn(&str) -> Option<String>) -> Option<Vec<Text>> {
        use horizon_client::text::{GREEN, RED};
        let line = |key: &str, fallback: &str, color: u32| Text::literal(translate(key).unwrap_or_else(|| fallback.to_owned())).with_color(color);
        if rows::infinity_cell(&stack.item).is_some() {
            return Some(vec![line("infinity.tooltip", "It is an infinity source.", GREEN)]);
        }
        if stack.item.as_ref() == rows::VOID_CELL {
            let mode = Mode::of(stack);
            let index = Mode::ALL.iter().position(|m| *m == mode).unwrap_or(0);
            let mut lines = vec![line(&format!("gui.extendedae.void_cell.mode.{index}"), mode.name(), GREEN)];
            if !self.extendedae_void_cell(stack).is_ok_and(|cell| cell.is_partitioned()) {
                lines.push(line("void_warn.tooltip", "Void Cell without a partition will not accept anything.", RED));
            }
            return Some(lines);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(id: &str) -> ItemKey {
        horizon_client::ae2_cell_codec::key(&ItemStack::new(id, 1)).unwrap()
    }
    fn cell(filters: &[&str], inverted: bool, mode: Mode) -> VoidCell {
        VoidCell {
            filters: filters.iter().map(|id| key(id)).collect(),
            partitioned: !filters.is_empty(),
            fuzzy: None,
            inverted,
            mode,
            energy: 0.0,
            stored: Vec::new(),
            max_damage: vec![0; filters.len()],
        }
    }

    #[test]
    fn a_void_cell_takes_its_partition_and_keeps_nothing() {
        let (cobble, dirt) = (key("minecraft:cobblestone"), key("minecraft:dirt"));
        // Unpartitioned, it takes nothing.
        let mut empty = cell(&[], false, Mode::Trash);
        assert_eq!(empty.insert(&cobble, 64, Action::Modulate), 0);
        assert!(!empty.is_preferred_storage_for(&cobble));
        let mut trash = cell(&["minecraft:cobblestone"], false, Mode::Trash);
        assert_eq!(trash.insert(&cobble, 1_000_000, Action::Modulate), 1_000_000);
        assert_eq!(trash.insert(&dirt, 5, Action::Modulate), 0);
        assert!(trash.is_preferred_storage_for(&cobble) && !trash.is_preferred_storage_for(&dirt));
        assert!(trash.available_stacks().is_empty() && trash.energy == 0.0, "trash keeps nothing");
        assert_eq!(trash.extract(&cobble, 10, Action::Modulate), 0);
        // An inverter card makes the partition a blacklist.
        let mut inverted = cell(&["minecraft:cobblestone"], true, Mode::Trash);
        assert_eq!(inverted.insert(&cobble, 3, Action::Simulate), 0);
        assert_eq!(inverted.insert(&dirt, 3, Action::Simulate), 3);
    }

    #[test]
    fn its_modes_condense_what_it_voids() {
        let cobble = key("minecraft:cobblestone");
        let ball = key("ae2:matter_ball");
        let mut balls = cell(&["minecraft:cobblestone"], false, Mode::MatterBalls);
        let power = horizon_client::ae2_common_config::condenser().matter_balls;
        assert_eq!(power, 256.0, "AE2's default");
        // A simulation keeps no energy.
        balls.insert(&cobble, 600, Action::Simulate);
        assert!(balls.available_stacks().is_empty());
        balls.insert(&cobble, 600, Action::Modulate);
        assert_eq!(balls.available_stacks(), vec![(ball.clone(), 2)]);
        assert_eq!(balls.energy, 88.0);
        balls.insert(&cobble, 168, Action::Modulate);
        assert_eq!(balls.available_stacks(), vec![(ball.clone(), 3)]);
        assert_eq!(balls.energy, 0.0);
        // What it made comes out.
        assert_eq!(balls.extract(&ball, 2, Action::Simulate), 2);
        assert_eq!(balls.extract(&ball, 5, Action::Modulate), 3);
        assert!(balls.available_stacks().is_empty());
        let mut stack = ItemStack::new(rows::VOID_CELL, 1);
        balls.insert(&cobble, 300, Action::Modulate);
        balls.store(&mut stack).unwrap();
        assert!(stack.data().unwrap().other(STORED).is_some());
        assert_eq!(stack.data().unwrap().other(VOID_ENERGY), Some(&Tag::Double(44f64.to_bits())));
        let mut singularity = cell(&["minecraft:cobblestone"], false, Mode::Singularity);
        singularity.insert(&cobble, 255_999, Action::Modulate);
        assert!(singularity.available_stacks().is_empty());
        singularity.insert(&cobble, 1, Action::Modulate);
        assert_eq!(singularity.available_stacks(), vec![(key("ae2:singularity"), 1)]);
    }

    #[test]
    fn its_mode_reads_from_the_stack() {
        let mut stack = ItemStack::new(rows::VOID_CELL, 1);
        assert_eq!(Mode::of(&stack), Mode::Trash);
        stack.data_mut().set_other(VOID_MODE, Some(Tag::String("SINGULARITY".into())));
        assert_eq!(Mode::of(&stack), Mode::Singularity);
        stack.data_mut().set_other(VOID_MODE, Some(Tag::String("nonsense".into())));
        assert_eq!(Mode::of(&stack), Mode::Trash);
    }
}
