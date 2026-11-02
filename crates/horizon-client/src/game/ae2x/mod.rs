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

/// The base model an addon's cable part draws in place of AE2's (its status
/// models are AE2's): none for AE2's own parts.
pub(super) fn part_model(id: &str) -> Option<&'static str> {
    #[cfg(feature = "ae2-extendedae")]
    if let Some(model) = extendedae::part_model(id) {
        return Some(model);
    }
    let _ = id;
    None
}

/// What the addons keep while the game runs; nothing of it is saved.
#[derive(Default)]
pub(super) struct State {
    #[cfg(feature = "ae2-merequester")]
    pub(super) merequester: merequester::State,
}

impl Game<'_> {
    /// The addons' grid ticks, after AE2's interfaces.
    pub(super) fn tick_ae2x(&mut self) {
        #[cfg(feature = "ae2-merequester")]
        self.tick_merequester();
    }

    /// An addon machine on the grid: its node's kind, idle draw, channel
    /// and the sides it connects on, from its block state's properties.
    pub(super) fn ae2x_device(&self, machine: &Machine, properties: Option<&horizon_world::block_state_properties::BlockProperties>, device: &mut ae2_network_core::Device) -> bool {
        let _ = (machine, properties, &device);
        #[cfg(feature = "ae2-merequester")]
        if merequester::device(machine, properties, device) {
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

    /// A click in an addon's menu window: true when an addon took it.
    pub(super) fn lan_ae2x_click(&mut self, host: &mut super::lan::LanHost, conn: u32, click: &horizon_protocol::packets::play::ContainerClick) -> bool {
        let _ = (&host, conn, click);
        #[cfg(feature = "ae2-merequester")]
        if self.lan_merequester_click(host, conn, click) {
            return true;
        }
        false
    }
    /// The client closed window `window`: true when it was an addon's.
    pub(super) fn lan_ae2x_closed(&mut self, host: &mut super::lan::LanHost, conn: u32, window: i32) -> bool {
        let _ = (&host, conn, window);
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
        #[cfg(feature = "ae2-extendedae")]
        if self.extendedae_kit_use(pos, face) {
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
