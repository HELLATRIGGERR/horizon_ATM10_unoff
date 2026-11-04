//! The game behaviour of AE2 addons, one module per addon behind its
//! `ae2-<id>` feature. Their rows in the definitions registry are in the
//! library's `ae2x`. The functions here are the one place the rest of the
//! game calls into the addons; each asks the addons built in.
use super::Game;
use horizon_client::ae2_world::Machine;
use std::collections::BTreeMap;

#[cfg(feature = "ae2-megacells")]
pub(super) mod megacells;
#[cfg(feature = "ae2-merequester")]
pub(super) mod merequester;
#[cfg(feature = "ae2-extendedae")]
pub(super) mod extendedae;

/// A storage cell of an addon's own `ICellHandler` (`StorageCells.getCellInventory`
/// for a cell no basic handler takes): MEGA's Bulk Item Cell. Item keys
/// only; a change is written back to its stack with [`HandlerCell::store`].
/// Its LED is `horizon_client::ae2x::handler_cell_state`.
pub(super) trait HandlerCell {
    /// `getAvailableStacks`, in the cell's order; amounts may be 0.
    fn available_stacks(&mut self) -> Vec<(ae2_cell_core::ItemKey, u64)>;
    fn insert(&mut self, key: &ae2_cell_core::ItemKey, amount: u64, action: ae2_cell_core::Action) -> u64;
    fn extract(&mut self, key: &ae2_cell_core::ItemKey, amount: u64, action: ae2_cell_core::Action) -> u64;
    /// `isPreferredStorageFor`.
    fn is_preferred_storage_for(&self, key: &ae2_cell_core::ItemKey) -> bool;
    /// `persist`: the cell's state onto its stack.
    fn store(&self, stack: &mut horizon_client::items::ItemStack) -> Result<(), String>;
}

/// How an addon's block turns to face the player when placed: AE2's
/// `OrientationStrategies.facing()` or `full()` (facing with spin).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Orientation {
    Facing,
    FacingWithSpin,
}
/// An addon block's orientation strategy; none for blocks that do not turn
/// (or are AE2's, or a variant of AE2's, which `ae2_placement` knows).
pub(super) fn orientation(block: &str) -> Option<Orientation> {
    #[cfg(feature = "ae2-extendedae")]
    if let Some(found) = extendedae::orientation(block) {
        return Some(found);
    }
    let _ = block;
    None
}

/// An addon machine with an AE2 machine menu (`UpgradeableMenu`): its slots
/// and cards in the chest window, its screen a layout of `ae2_screens`.
pub(super) fn machine_menu(machine: &Machine) -> bool {
    let _ = machine;
    #[cfg(feature = "ae2-extendedae")]
    if extendedae::machine_menu(machine) {
        return true;
    }
    false
}
/// Whether such a menu's slot `slot` takes items from the cursor.
pub(super) fn machine_may_place(machine: &Machine, slot: usize) -> Option<bool> {
    let _ = (machine, slot);
    #[cfg(feature = "ae2-extendedae")]
    if let Some(found) = extendedae::may_place(machine, slot) {
        return Some(found);
    }
    None
}
/// What automation (hoppers, droppers, pattern providers) may put in an
/// addon machine's slot `index`, and take out; none leaves AE2's rules.
pub(super) fn slot_insert(kind: horizon_client::ae2_world::MachineKind, index: usize) -> Option<bool> {
    let _ = (kind, index);
    #[cfg(feature = "ae2-extendedae")]
    if let Some(found) = extendedae::slot_insert(kind, index) {
        return Some(found);
    }
    None
}
pub(super) fn slot_extract(kind: horizon_client::ae2_world::MachineKind, index: usize) -> Option<bool> {
    let _ = (kind, index);
    #[cfg(feature = "ae2-extendedae")]
    if let Some(found) = extendedae::slot_extract(kind, index) {
        return Some(found);
    }
    None
}
/// Whether Forge Energy enters an addon machine's own buffer from `face`.
pub(super) fn private_power_face(kind: horizon_client::ae2_world::MachineKind, state: &horizon_world::chunk::BlockState, face: horizon_core::geometry::Direction) -> bool {
    let _ = (kind, state, face);
    #[cfg(feature = "ae2-extendedae")]
    if extendedae::private_power_face(kind, state, face) {
        return true;
    }
    false
}
/// An addon machine screen's progress bar: its value and most.
pub(super) fn machine_progress(machine: &Machine) -> Option<(f64, f64)> {
    let _ = machine;
    #[cfg(feature = "ae2-extendedae")]
    if let Some(found) = extendedae::progress(machine) {
        return Some(found);
    }
    None
}
/// The entries an addon machine's window shows past its slots and cards.
pub(super) fn machine_menu_items(machine: &Machine, items: &mut [Option<horizon_client::items::ItemStack>]) {
    let _ = (machine, &items);
    #[cfg(feature = "ae2-megacells")]
    megacells::cell_dock::menu_items(machine, items);
    #[cfg(feature = "ae2-extendedae")]
    extendedae::menu_items(machine, items);
}
/// The factor on an addon bus's operations (`getOperationsPerTick`): 1 for AE2's.
pub(super) fn bus_multiplier(id: &str) -> u64 {
    let _ = id;
    #[cfg(feature = "ae2-extendedae")]
    if let n @ 2.. = horizon_client::ae2x::extendedae::bus_operations_multiplier(id) {
        return n;
    }
    1
}
/// The screen style of an addon's part (its namespace, where its files under
/// AE2's are, and the file), when it has one of its own.
pub(super) fn part_style(id: &str) -> Option<(&'static str, &'static str)> {
    let _ = id;
    #[cfg(feature = "ae2-extendedae")]
    if let Some(found) = extendedae::part_style(id) {
        return Some(found);
    }
    None
}
/// The items an AE2 IO Port of an addon moves a tick, by its speed cards.
pub(super) fn io_port_budget(machine: &Machine) -> Option<u64> {
    let _ = machine;
    #[cfg(feature = "ae2-extendedae")]
    if machine.id() == horizon_client::ae2x::extendedae::EX_IO_PORT {
        return Some(horizon_client::ae2x::extendedae::io_port_budget(machine.installed_speed_cards()));
    }
    None
}

/// The base model an addon's cable part draws in place of AE2's (its status
/// models are AE2's): none for AE2's own parts.
pub(super) fn part_model(id: &str) -> Option<&'static str> {
    #[cfg(feature = "ae2-megacells")]
    if let Some(model) = megacells::part_model(id) {
        return Some(model);
    }
    #[cfg(feature = "ae2-extendedae")]
    if let Some(model) = extendedae::part_model(id) {
        return Some(model);
    }
    let _ = id;
    None
}

/// An addon part's collision and selection boxes (`IPart.getBoxes`), in
/// sixteenths with the part on the south face, as `ae2_geometry` keeps AE2's.
pub(super) fn part_boxes(id: &str) -> Option<Vec<[f64; 6]>> {
    #[cfg(feature = "ae2-megacells")]
    if let Some(boxes) = megacells::cell_dock::boxes(id) {
        return Some(boxes);
    }
    let _ = id;
    None
}
/// Where an addon's cell part draws its cell (`renderDynamic`), from the
/// block's centre: the cell model's pose, which its LED shares, and the pose
/// of the part's own LED.
pub(super) fn cell_part_poses(id: &str, side: &str, spin: u8) -> Option<(glam::Mat4, glam::Mat4)> {
    #[cfg(feature = "ae2-megacells")]
    if let Some(poses) = megacells::cell_dock::poses(id, side, spin) {
        return Some(poses);
    }
    let _ = (id, side, spin);
    None
}
/// An addon part's spin when placed on cable face `side` by a player of yaw
/// `yaw` (`IPart.onPlacement`): none for a part that keeps no spin.
pub(super) fn part_placement_spin(id: &str, side: &str, yaw: f32) -> Option<u8> {
    #[cfg(feature = "ae2-megacells")]
    if let Some(spin) = megacells::cell_dock::placement_spin(id, side, yaw) {
        return Some(spin);
    }
    let _ = (id, side, yaw);
    None
}
/// The title of the menu an addon's part machine opens (AE2's machine menu
/// over the part); none for a part machine without one.
pub(super) fn part_menu_title(id: &str) -> Option<&'static str> {
    #[cfg(feature = "ae2-megacells")]
    if let Some(title) = megacells::cell_dock::menu_title(id) {
        return Some(title);
    }
    let _ = id;
    None
}

/// `AppEngInternalInventory.isItemValid` of a Pattern Provider's pattern
/// inventory past AE2's own rule (an encoded pattern): the filter an addon's
/// provider sets on it (`setFilter`). True for AE2's, which set none.
pub(super) fn pattern_inventory_accepts(machine: &Machine, item: &horizon_client::items::ItemStack) -> bool {
    #[cfg(feature = "ae2-megacells")]
    if let Some(allowed) = megacells::pattern_allowed(machine.id(), item) {
        return allowed;
    }
    let _ = (machine, item);
    true
}


/// Whether an addon's item opens a menu when used (`Item.use` succeeds on
/// the client, `sidedSuccess`): the client then stops at this hand.
pub(super) fn item_opens_menu(id: &str) -> bool {
    #[cfg(feature = "ae2-megacells")]
    if id == horizon_client::ae2x::megacells::PORTABLE_CELL_WORKBENCH {
        return true;
    }
    let _ = id;
    false
}

/// What the addons keep while the game runs; nothing of it is saved.
#[derive(Default)]
pub(super) struct State {
    #[cfg(feature = "ae2-megacells")]
    pub(super) megacells: megacells::State,
    #[cfg(feature = "ae2-merequester")]
    pub(super) merequester: merequester::State,
    #[cfg(feature = "ae2-extendedae")]
    pub(super) extendedae: extendedae::State,
}

impl Game<'_> {
    /// `StorageCells.getCellInventory` of a cell of an addon's own handler:
    /// none for any other stack (AE2's basic cells, creative cells, items).
    pub(super) fn ae2x_handler_cell(&self, stack: &horizon_client::items::ItemStack) -> Option<Result<Box<dyn HandlerCell>, String>> {
        #[cfg(feature = "ae2-megacells")]
        if stack.item.as_ref() == horizon_client::ae2x::megacells::BULK_ITEM_CELL {
            return Some(self.megacells_bulk_cell(stack).map(|cell| Box::new(cell) as Box<dyn HandlerCell>));
        }
        let _ = stack;
        None
    }
    /// The tooltip image of a cell of an addon's own handler
    /// (`getTooltipImage`): its upgrades and its content.
    pub(super) fn ae2x_cell_preview(&self, stack: &horizon_client::items::ItemStack, show_upgrades: bool, show_content: bool) -> Option<(Vec<horizon_client::items::ItemStack>, Vec<(ae2_cell_core::ItemKey, u64)>)> {
        #[cfg(feature = "ae2-megacells")]
        if stack.item.as_ref() == horizon_client::ae2x::megacells::BULK_ITEM_CELL {
            return self.megacells_bulk_preview(stack, show_upgrades, show_content);
        }
        let _ = (stack, show_upgrades, show_content);
        None
    }
    /// What an addon's item adds to its tooltip below its name
    /// (`Item.appendHoverText`), each line in its own style; `name` gives an
    /// item's hover name.
    pub(super) fn ae2x_item_hover_lines(&self, stack: &horizon_client::items::ItemStack, translate: &dyn Fn(&str) -> Option<String>, name: &dyn Fn(&horizon_client::items::ItemStack) -> horizon_client::text::Text) -> Vec<horizon_client::text::Text> {
        #[cfg(feature = "ae2-megacells")]
        {
            if stack.item.as_ref() == horizon_client::ae2x::megacells::BULK_ITEM_CELL {
                return self.megacells_bulk_hover_lines(stack, name);
            }
            let lines = megacells::hover_lines(&stack.item, translate);
            if !lines.is_empty() {
                return lines;
            }
        }
        let _ = (stack, translate, name);
        Vec::new()
    }
    /// The addons' grid ticks, after AE2's interfaces.
    pub(super) fn tick_ae2x(&mut self) {
        #[cfg(feature = "ae2-merequester")]
        self.tick_merequester();
        #[cfg(feature = "ae2-extendedae")]
        self.tick_extendedae_machines();
    }

    /// An addon machine on the grid: its node's kind, idle draw, channel
    /// and the sides it connects on, from its block state's properties.
    pub(super) fn ae2x_device(&self, machine: &Machine, properties: Option<&horizon_world::block_state_properties::BlockProperties>, device: &mut ae2_network_core::Device) -> bool {
        let _ = (machine, properties, &device);
        #[cfg(feature = "ae2-merequester")]
        if merequester::device(machine, properties, device) {
            return true;
        }
        #[cfg(feature = "ae2-extendedae")]
        if extendedae::device(machine, properties, device) {
            return true;
        }
        false
    }

    /// An addon machine's block state properties, set each grid tick
    /// (`updateBlockStateFromBlockEntity`): false when it has none.
    pub(super) fn ae2x_visual_props(&self, machine: &Machine, props: &mut horizon_world::block_state_properties::BlockProperties) -> bool {
        let _ = (machine, &props);
        #[cfg(feature = "ae2-merequester")]
        if merequester::visual_props(machine, props) {
            return true;
        }
        #[cfg(feature = "ae2-extendedae")]
        if extendedae::visual_props(machine, props) {
            return true;
        }
        false
    }

    /// A guest uses an addon's block or part: true when an addon opened its menu.
    pub(super) fn lan_open_ae2x(&mut self, host: &mut super::lan::LanHost, conn: u32, pos: (i32, i32, i32), face: &str) -> bool {
        let _ = (&host, conn, pos, face);
        #[cfg(feature = "ae2-merequester")]
        if self.lan_open_merequester(host, conn, pos, face) {
            return true;
        }
        false
    }

    /// A guest uses the item in hand `hand` (0 main, 1 off) in the air
    /// (`Item.use`): true when an addon's item opened its menu.
    pub(super) fn lan_ae2x_use_item(&mut self, host: &mut super::lan::LanHost, conn: u32, hand: i32) -> bool {
        let _ = (&host, conn, hand);
        #[cfg(feature = "ae2-megacells")]
        if self.lan_open_portable_workbench(host, conn, hand) {
            return true;
        }
        false
    }

    /// A click in an addon's menu window: true when an addon took it.
    pub(super) fn lan_ae2x_click(&mut self, host: &mut super::lan::LanHost, conn: u32, click: &horizon_protocol::packets::play::ContainerClick) -> bool {
        let _ = (&host, conn, click);
        #[cfg(feature = "ae2-megacells")]
        if self.lan_portable_workbench_click(host, conn, click) {
            return true;
        }
        #[cfg(feature = "ae2-merequester")]
        if self.lan_merequester_click(host, conn, click) {
            return true;
        }
        false
    }
    /// The client closed window `window`: true when it was an addon's.
    pub(super) fn lan_ae2x_closed(&mut self, host: &mut super::lan::LanHost, conn: u32, window: i32) -> bool {
        let _ = (&host, conn, window);
        #[cfg(feature = "ae2-megacells")]
        if self.lan_portable_workbench_closed(host, conn, Some(window)) {
            return true;
        }
        #[cfg(feature = "ae2-merequester")]
        if self.lan_merequester_closed(host, conn, Some(window)) {
            return true;
        }
        false
    }
    /// A custom payload from a guest: true when an addon's channel.
    pub(super) fn lan_ae2x_payload(&mut self, host: &mut super::lan::LanHost, conn: u32, channel: &str, data: &[u8]) -> bool {
        let _ = (&host, conn, channel, data);
        #[cfg(feature = "ae2-merequester")]
        if channel.starts_with("merequester:") {
            self.lan_merequester_payload(host, conn, channel, data);
            return true;
        }
        false
    }
    /// The addons' open menus, once a tick (`broadcastChanges`).
    pub(super) fn lan_refresh_ae2x(&mut self, host: &mut super::lan::LanHost) {
        let _ = &host;
        #[cfg(feature = "ae2-megacells")]
        self.lan_refresh_portable_workbenches(host);
        #[cfg(feature = "ae2-merequester")]
        self.lan_refresh_merequester(host);
    }

    /// `/ae2 <addon> ...`: none when no addon has the command.
    pub(super) fn ae2x_command(&mut self, words: &[&str]) -> Option<Result<serde_json::Value, String>> {
        let _ = words;
        #[cfg(feature = "ae2-merequester")]
        if words.first() == Some(&"requester") {
            return Some(self.ae2_requester_command(&words[1..]));
        }
        None
    }

    /// The held item used on block `pos`, side `face`, before AE2's own
    /// item uses (`Item.useOn`): true when an addon's item acted.
    pub(super) fn ae2x_use_on(&mut self, pos: [i32; 3], face: [i32; 3]) -> bool {
        let _ = (pos, face);
        #[cfg(feature = "ae2-megacells")]
        if self.megacells_dock_wrench(pos, face) {
            return true;
        }
        #[cfg(feature = "ae2-extendedae")]
        if self.extendedae_kit_use(pos, face) {
            return true;
        }
        false
    }

    /// A use of an addon's block by the hand (its `openGui` when the block has
    /// no menu): true when an addon acted.
    pub(super) fn ae2x_block_use(&mut self, pos: (i32, i32, i32)) -> bool {
        let _ = pos;
        #[cfg(feature = "ae2-extendedae")]
        if self.extendedae_fixer_use(pos) {
            return true;
        }
        false
    }
    /// A press on window slot `slot` of an addon machine's AE2 menu that is
    /// none of its slots or cards (a tank, a toggle): true when an addon took it.
    pub(super) fn ae2x_machine_click(&mut self, pos: (i32, i32, i32), slot: usize, button: i32, input: horizon_client::menu::Input) -> bool {
        let _ = (pos, slot, button, input);
        #[cfg(feature = "ae2-extendedae")]
        if (300..354).contains(&slot) && self.extendedae_machine_click(pos, slot - 300, button, input) {
            return true;
        }
        false
    }

    /// A Memory Card used on an addon's block: true when an addon took it.
    pub(super) fn ae2x_memory(&mut self, pos: [i32; 3]) -> bool {
        let _ = pos;
        #[cfg(feature = "ae2-merequester")]
        if self.ae2_try_requester_memory(pos) {
            return true;
        }
        false
    }

    // An addon's screen on this client, over the menu its server opened.

    /// A server payload of an addon's channel: true when an addon took it.
    pub(super) fn ae2x_client_payload(&mut self, channel: &str, data: &[u8]) -> bool {
        let _ = (channel, data);
        #[cfg(feature = "ae2-merequester")]
        if self.merequester_payload(channel, data) {
            return true;
        }
        false
    }
    /// The open addon screen's size.
    pub(super) fn ae2x_screen_size(&self) -> Option<(f32, f32)> {
        #[cfg(feature = "ae2-merequester")]
        if let Some(size) = self.merequester_screen_size() {
            return Some(size);
        }
        None
    }
    /// The slots the open addon screen shows.
    pub(super) fn ae2x_slots(&self) -> Option<Vec<usize>> {
        #[cfg(feature = "ae2-merequester")]
        if let Some(slots) = self.merequester_slots() {
            return Some(slots);
        }
        None
    }
    /// Where the open addon screen puts slot `i`.
    pub(super) fn ae2x_slot_position(&self, i: usize) -> Option<(f32, f32)> {
        let _ = i;
        #[cfg(feature = "ae2-merequester")]
        if let Some(at) = self.merequester_slot_position(i) {
            return Some(at);
        }
        None
    }
    /// Draws the open addon screen's background and widgets: true then.
    pub(super) fn ae2x_draw(&self, batch: &mut horizon_client::hud::HudBatch, ox: f32, oy: f32) -> bool {
        let _ = (&batch, ox, oy);
        #[cfg(feature = "ae2-merequester")]
        if self.merequester_draw(batch, ox, oy) {
            return true;
        }
        false
    }
    /// A press at `at` (from the screen's top left) on an addon screen's
    /// widgets: true when one took it.
    pub(super) fn ae2x_press(&mut self, at: [f32; 2], right: bool) -> bool {
        let _ = (at, right);
        #[cfg(feature = "ae2-merequester")]
        if self.merequester_press(at, right) {
            return true;
        }
        false
    }
    pub(super) fn ae2x_scroll(&mut self, delta: f32) -> bool {
        let _ = delta;
        #[cfg(feature = "ae2-merequester")]
        if self.merequester_scroll(delta) {
            return true;
        }
        false
    }
    /// Whether an addon screen's text field takes the keyboard.
    pub(super) fn ae2x_field_focused(&self) -> bool {
        #[cfg(feature = "ae2-merequester")]
        if self.merequester_field_focused() {
            return true;
        }
        false
    }
    pub(super) fn ae2x_field_input(&mut self, text: Option<&str>, key: Option<winit::keyboard::KeyCode>) {
        let _ = (text, key);
        #[cfg(feature = "ae2-merequester")]
        self.merequester_field_input(text, key);
    }
    /// The tooltip of the addon screen's widget under the cursor: true when drawn.
    pub(super) fn ae2x_tooltip(&self, batch: &mut horizon_client::hud::HudBatch) -> bool {
        let _ = &batch;
        #[cfg(feature = "ae2-merequester")]
        if self.merequester_tooltip(batch) {
            return true;
        }
        false
    }
    /// The open addon screen for the control socket's `screen` reply.
    pub(super) fn ae2x_agent_value(&self) -> Option<serde_json::Value> {
        #[cfg(feature = "ae2-merequester")]
        if let Some(value) = self.merequester_agent_value() {
            return Some(value);
        }
        None
    }
}
