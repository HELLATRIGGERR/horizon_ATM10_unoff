//! AE2WTLib's `WirelessTerminalSettingsScreen` on the client: the wireless
//! crafting terminal's sub screen (`screens/wtlib/wireless_terminal_settings.json`,
//! carried under `assets/ae2wtlib/ae2/screens/`) with its five checkboxes,
//! each change sent whole (`TerminalSettingsPacket`). AE2WTLib 19.5.1 by
//! Mari023, https://github.com/Mari023/AE2WirelessTerminalLibrary tag
//! `19.5.1`, MIT; see `ae2/addons/ae2wtlib/`. The terminal screen draws it
//! as it draws AE2's own sub screens (`ae2_terminal_settings`).
use crate::game::Game;
use horizon_client::ae2x::ae2wtlib::magnet::MagnetMode;

/// The screen's style, under AE2WTLib's namespace.
pub(in crate::game) const STYLE: &str = "wtlib/wireless_terminal_settings.json";
/// The checkboxes: the style's widget and the label's key, in the
/// screen's order (`pickBlock`, `craftIfMissing`, `restock`, `magnet`,
/// `pickupToME`).
const CHECKS: [(&str, &str); 5] = [
    ("pickBlock", "gui.ae2wtlib.pick_block.text"),
    ("craftIfMissing", "gui.ae2wtlib.craft_if_missing.text"),
    ("restock", "gui.ae2wtlib.restock.text"),
    ("magnet", "gui.ae2wtlib.magnet.text"),
    ("pickupToME", "gui.ae2wtlib.pickup_to_me.text"),
];

/// What the screen shows: the item's settings when it opened, as the
/// checkboxes then stand.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::game) struct Settings {
    checks: [bool; 5],
    /// The item has a Magnet Card: the magnet's two boxes are active.
    card: bool,
}
impl Settings {
    /// The server's view of the item (`lan_ae2wtlib_wireless_view`): the
    /// magnet's boxes show the stored mode, card or not.
    pub(in crate::game) fn from_view(view: &serde_json::Value) -> Option<Self> {
        let flag = |key: &str| view.get(key).and_then(serde_json::Value::as_bool).unwrap_or(false);
        let stored = MagnetMode::from_byte(view.get("stored")?.as_i64()? as i8);
        let mode = view.get("magnet")?.as_i64()?;
        Some(Self {
            checks: [flag("pick_block"), flag("craft_if_missing"), flag("restock"), stored.magnet(), stored.pickup_to_me()],
            card: mode != i64::from(MagnetMode::NoCard.id()) && mode != i64::from(MagnetMode::Invalid.id()),
        })
    }
    /// Whether box `index` takes presses: craft if missing follows pick
    /// block, the magnet's follow the card.
    fn active(self, index: usize) -> bool {
        match index {
            1 => self.checks[0],
            3 | 4 => self.card,
            _ => true,
        }
    }
    /// `TerminalSettingsPacket`'s fields in its order: pick block, restock,
    /// magnet, pickup to ME, craft if missing.
    pub(in crate::game) fn payload(self) -> Vec<u8> {
        let [pick_block, craft_if_missing, restock, magnet, pickup] = self.checks;
        [pick_block, restock, magnet, pickup, craft_if_missing].map(u8::from).to_vec()
    }
}

impl Game<'_> {
    /// The universal terminal's settings view of the open terminal, when it
    /// is a wireless crafting terminal's.
    fn ae2wtlib_view_settings(&self) -> Option<Settings> {
        Settings::from_view(self.ae2_term_addon_view()?)
    }
    /// `WCTScreen`'s settings button is shown.
    pub(in crate::game) fn ae2wtlib_settings_button(&self) -> bool {
        self.ae2wtlib_view_settings().is_some()
    }
    /// The settings button: the sub screen opens on the item's settings.
    pub(in crate::game) fn ae2wtlib_open_settings(&mut self) {
        let settings = self.ae2wtlib_view_settings();
        self.ae2_term_set_wtlib_settings(settings);
    }
    /// The checkboxes: (style widget, label, selected, active).
    pub(in crate::game) fn ae2wtlib_settings_checks(&self, settings: Settings) -> Vec<(&'static str, String, bool, bool)> {
        CHECKS
            .iter()
            .enumerate()
            .map(|(i, (widget, key))| (*widget, self.translate(key, key), settings.checks[i], settings.active(i)))
            .collect()
    }
    /// A press on box `index`: it turns, and the whole is sent (`save`).
    pub(in crate::game) fn ae2wtlib_settings_press(&mut self, settings: Settings, index: usize) {
        if index >= CHECKS.len() || !settings.active(index) {
            return;
        }
        let mut next = settings;
        next.checks[index] ^= true;
        self.ae2_term_set_wtlib_settings(Some(next));
        self.ae2_send_payload(super::magnet::TERMINAL_SETTINGS, next.payload());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_payload_and_active_boxes() {
        let view = serde_json::json!({"pick_block": true, "craft_if_missing": false, "restock": true, "magnet": -1, "stored": 3});
        let settings = Settings::from_view(&view).unwrap();
        assert_eq!(settings.checks, [true, false, true, true, true], "the stored mode shows without a card");
        assert!(!settings.card && !settings.active(3) && !settings.active(4) && settings.active(1));
        assert_eq!(settings.payload(), [1, 1, 1, 1, 0], "pick block, restock, magnet, pickup to ME, craft if missing");
        let bare = Settings::from_view(&serde_json::json!({"magnet": 0, "stored": 0})).unwrap();
        assert!(bare.card && !bare.active(1), "craft if missing waits for pick block");
    }
}
