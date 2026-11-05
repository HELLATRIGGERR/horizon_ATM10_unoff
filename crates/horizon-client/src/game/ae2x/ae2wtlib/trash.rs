//! AE2WTLib's trash (`TrashMenu`, `TrashScreen`, `WCTMenuHost.INV_TRASH`):
//! from the wireless crafting terminal's Trash button, 27 slots over the
//! player's inventory, an `AppEngInternalInventory` of the menu's host that
//! nothing saves, so what is left in it is gone when the menu closes or its
//! back button returns to the terminal. The client draws
//! `screens/wtlib/trash.json` over the window (its background
//! `textures/wtlib/guis/trash.png`, carried under `assets/ae2wtlib/ae2/`).
//! AE2WTLib 19.5.1 by Mari023,
//! https://github.com/Mari023/AE2WirelessTerminalLibrary tag `19.5.1`, MIT;
//! see `ae2/addons/ae2wtlib/`.
use crate::game::{Game, Screen, ae2_screens::inside, ae2_wireless, lan::LanHost, lan_crafting};
use horizon_client::{
    ae2_screen_style::ScreenStyle,
    ae2x::{defs::TerminalKind, wireless},
    hud::HudBatch,
    items::ItemStack,
    menu::{Kind, Menu},
};
use horizon_protocol::packets::{
    configuration::CustomPayload,
    play::{self, Clientbound as C},
};

/// `WCTMenu.TRASH_MENU` and the screen's back button: one byte, 0 opens
/// the trash from the terminal, 1 goes back to it.
pub(in crate::game) const TRASH: &str = "ae2wtlib:trash";
/// The trash's window, for this client to draw it.
pub(in crate::game) const TRASH_WINDOW: &str = "ae2wtlib:trash_window";
/// The screen's style, under AE2WTLib's namespace.
const STYLE: &str = "wtlib/trash.json";
/// `new AppEngInternalInventory(27)`.
const SLOTS: usize = 27;
/// AE2's `Icon.BACK`.
const ICON_BACK: (i32, i32) = (96, 16);

/// One player's open trash.
pub(in crate::game) struct Session {
    window: i32,
    state: i32,
    /// The crafting terminal it came from (`MenuOpener.open(TrashMenu.TYPE,
    /// player, getLocator())`).
    ctx: ae2_wireless::WirelessContext,
    /// The trash's 27 slots and the player's inventory.
    menu: Menu,
}

impl Game<'_> {
    /// `WCTMenu.openTrashMenu`: the open wireless crafting terminal's trash
    /// in its place.
    pub(in crate::game) fn lan_ae2wtlib_open_trash(&mut self, host: &mut LanHost, conn: u32) {
        let Some(source) = self.lan_wireless_menu_source(host, conn) else { return };
        let Some(g) = host.hub.guests.iter_mut().find(|g| g.conn == conn && !g.spectator()) else { return };
        let Some(stack) = ae2_wireless::source_stack_mut(g, source).filter(|s| wireless::kind(s) == Some(TerminalKind::Crafting)) else { return };
        let Some((target_dim, target)) = ae2_wireless::target(stack) else { return };
        let source_owner = horizon_client::ae2_stack_identity::StackRef::capture(stack);
        let ctx = ae2_wireless::WirelessContext { source, target_dim, target, kind: TerminalKind::Crafting, idle: 0, source_owner };
        self.lan_ae2_terminal_closed(host, conn, None);
        self.lan_pattern_access_closed(host, conn, None);
        self.lan_ae2wtlib_trash_closed(host, conn, None);
        let Some(window) = host.hub.open_window(conn, "minecraft:generic_9x3", horizon_protocol::text::translatable("gui.ae2wtlib.trash", &[])) else { return };
        host.hub.send_to(conn, &C::CustomPayload(CustomPayload { channel: TRASH_WINDOW.into(), data: window.to_be_bytes().to_vec() }));
        self.ae2x.ae2wtlib.trash.insert(conn, Session { window, state: 0, ctx, menu: Menu::new(Kind::Chest(3)) });
        self.lan_send_ae2wtlib_trash(host, conn);
    }
    /// The trash's back button (`AESubScreen.addBackButton`,
    /// `returnToMainMenu`): the terminal again, the trash emptied.
    fn lan_ae2wtlib_trash_back(&mut self, host: &mut LanHost, conn: u32) {
        let Some(source) = self.ae2x.ae2wtlib.trash.get(&conn).map(|s| s.ctx.source) else { return };
        self.lan_ae2wtlib_trash_closed(host, conn, None);
        self.lan_open_ae2_wireless_source(host, conn, source);
    }
    /// The trash's payload: open it, or go back.
    pub(in crate::game) fn lan_ae2wtlib_trash_payload(&mut self, host: &mut LanHost, conn: u32, data: &[u8]) {
        match data {
            [0] => self.lan_ae2wtlib_open_trash(host, conn),
            [1] => self.lan_ae2wtlib_trash_back(host, conn),
            _ => {}
        }
    }
    /// The window closed: what the trash holds is gone, the cursor's stack
    /// goes back to the player.
    pub(in crate::game) fn lan_ae2wtlib_trash_closed(&mut self, host: &mut LanHost, conn: u32, window: Option<i32>) -> bool {
        if !self.ae2x.ae2wtlib.trash.get(&conn).is_some_and(|s| window.is_none_or(|w| w == s.window)) {
            return false;
        }
        self.ae2x.ae2wtlib.trash.remove(&conn);
        if let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) {
            self.guest_carried_back(host, gi);
        }
        true
    }
    /// The window's content: the trash, then the player's inventory.
    fn lan_send_ae2wtlib_trash(&mut self, host: &mut LanHost, conn: u32) {
        let Some(s) = self.ae2x.ae2wtlib.trash.get_mut(&conn) else { return };
        let Some(g) = host.hub.guests.iter().find(|g| g.conn == conn) else { return };
        let mut items: Vec<Option<ItemStack>> = (0..SLOTS).map(|i| s.menu.slot(i).cloned()).collect();
        items.extend((9..36).chain(0..9).map(|i| g.inventory[i].clone()));
        s.state = (s.state + 1) & 0x7fff;
        let carried = host.hub.stack(&g.carried.clone());
        let wire = items.iter().map(|item| host.hub.stack(item)).collect();
        let (window, state) = (s.window, s.state);
        host.hub.send_to(conn, &C::ContainerSetContent(play::ContainerSetContent { container: window, state, items: wire, carried }));
    }
    /// A click in the trash's window: AE2's slots, a chest's rules.
    pub(in crate::game) fn lan_ae2wtlib_trash_click(&mut self, host: &mut LanHost, conn: u32, p: &play::ContainerClick) -> bool {
        let Some(mut s) = self.ae2x.ae2wtlib.trash.remove(&conn) else { return false };
        if p.container != s.window {
            self.ae2x.ae2wtlib.trash.insert(conn, s);
            return false;
        }
        if let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn && host.hub.here(g)) {
            let input = lan_crafting::input(p.input);
            self.guest_menu_op(host, gi, &mut s.menu, |m, env| {
                let _ = m.click(env, i32::from(p.slot), i32::from(p.button), input);
            });
        }
        self.ae2x.ae2wtlib.trash.insert(conn, s);
        self.lan_send_ae2wtlib_trash(host, conn);
        true
    }
    /// `AEBaseMenu.broadcastChanges`: a trash whose terminal left its slot,
    /// or whose window another menu took, closes.
    pub(in crate::game) fn lan_refresh_ae2wtlib_trash(&mut self, host: &mut LanHost) {
        let conns: Vec<u32> = self.ae2x.ae2wtlib.trash.keys().copied().collect();
        for conn in conns {
            if host.hub.guests.iter().any(|g| g.conn == conn && !host.hub.here(g)) {
                continue;
            }
            let Some(s) = self.ae2x.ae2wtlib.trash.get(&conn) else { continue };
            let window = s.window;
            let taken = host.hub.guests.iter().find(|g| g.conn == conn).is_none_or(|g| g.next_window != window);
            if taken {
                self.ae2x.ae2wtlib.trash.remove(&conn);
                continue;
            }
            if !self.ae2_wireless_source_valid(host, conn, &s.ctx) {
                self.lan_ae2wtlib_trash_closed(host, conn, None);
                host.hub.send_to(conn, &C::ContainerClose(play::ContainerClose { container: window }));
            }
        }
    }

    // The client's screen.

    /// `ae2wtlib:trash_window`: the window this client draws as the trash.
    pub(in crate::game) fn ae2wtlib_trash_window_payload(&mut self, data: &[u8]) {
        self.ae2x.ae2wtlib.trash_window = <[u8; 4]>::try_from(data).ok().map(i32::from_be_bytes);
    }
    /// The trash's style while its window is open.
    fn ae2wtlib_trash_style(&self) -> Option<&'static ScreenStyle> {
        let window = self.ae2x.ae2wtlib.trash_window?;
        let open = self.screen == Screen::Inventory && self.remote.as_ref().is_some_and(|r| r.menu.as_ref().is_some_and(|m| m.0 == window));
        if !open {
            return None;
        }
        self.ae2_addon_style("ae2wtlib", STYLE)
    }
    pub(in crate::game) fn ae2wtlib_trash_size(&self) -> Option<(f32, f32)> {
        let (w, h) = self.ae2wtlib_trash_style()?.size();
        Some((w as f32, h as f32))
    }
    /// The player's inventory and the trash's slots (300 and on).
    pub(in crate::game) fn ae2wtlib_trash_slots(&self) -> Option<Vec<usize>> {
        self.ae2wtlib_trash_style()?;
        Some((0..36).chain(300..300 + SLOTS).collect())
    }
    pub(in crate::game) fn ae2wtlib_trash_slot_position(&self, i: usize) -> Option<(f32, f32)> {
        let style = self.ae2wtlib_trash_style()?;
        let at = match i {
            0..=8 => style.slot("PLAYER_HOTBAR", i),
            9..=35 => style.slot("PLAYER_INVENTORY", i - 9),
            300.. if i - 300 < SLOTS => style.slot("AE2WTLIB_TRASH", i - 300),
            _ => None,
        };
        Some(at.map_or((-1000.0, -1000.0), |(x, y)| (x as f32, y as f32)))
    }
    /// The back button (`TabButton` with `Icon.BACK`).
    fn ae2wtlib_trash_back(&self) -> Option<[i32; 4]> {
        let style = self.ae2wtlib_trash_style()?;
        let (w, h) = style.size();
        let widget = style.widgets.get("back")?;
        let (x, y) = widget.position.resolve(w, h);
        Some([x, y, widget.width.max(20), widget.height.max(20)])
    }
    /// `TrashScreen`: the background, its labels and the back button.
    pub(in crate::game) fn ae2wtlib_draw_trash(&self, batch: &mut HudBatch, ox: f32, oy: f32) -> bool {
        let Some(style) = self.ae2wtlib_trash_style() else { return false };
        self.ae2_draw_background(batch, ox, oy, style);
        self.ae2_draw_texts(batch, ox, oy, style, &[]);
        if let Some(rect) = self.ae2wtlib_trash_back() {
            self.ae2_draw_tab(batch, ox, oy, rect, ICON_BACK);
        }
        true
    }
    /// A press on the back button.
    pub(in crate::game) fn ae2wtlib_trash_press(&mut self, at: [f32; 2]) -> bool {
        if !self.ae2wtlib_trash_back().is_some_and(|rect| inside(rect, at)) {
            return false;
        }
        self.audio.play_button_click();
        self.ae2_send_payload(TRASH, vec![1]);
        true
    }
    pub(in crate::game) fn ae2wtlib_trash_tooltip(&self, batch: &mut HudBatch) -> bool {
        let Some(rect) = self.ae2wtlib_trash_back() else { return false };
        let (ox, oy) = self.inventory_origin(batch);
        if !inside(rect, [self.cursor.0 - ox, self.cursor.1 - oy]) {
            return false;
        }
        let text = self.translate("gui.ae2.Back", "Back");
        self.draw_tooltip_lines(batch, &[(horizon_client::text::Text::literal(text), u32::MAX)], self.cursor.0, self.cursor.1);
        true
    }
    /// The screen for the control socket's `screen`.
    pub(in crate::game) fn ae2wtlib_trash_agent(&self) -> Option<serde_json::Value> {
        let (w, h) = self.ae2wtlib_trash_size()?;
        let origin = self.renderer.as_ref().map(|r| self.screen_batch([r.config.width, r.config.height]).centered_origin(w, h));
        let back = self.ae2wtlib_trash_back().zip(origin).map(|([x, y, bw, bh], (ox, oy))| [ox + x as f32, oy + y as f32, bw as f32, bh as f32]);
        Some(serde_json::json!({"trash": {"window": self.ae2x.ae2wtlib.trash_window, "size": [w, h], "back": back}}))
    }
}
