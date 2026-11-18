//! `EnderDiskFrequencyScreen` (1.5.23, client): the screen a disk's use opens. Three dye selectors
//! (the frequency's three hex digits, `dyeIndices`), their arrows (+-256, +-16, +-1), the frequency
//! box, the transfer mode button and the scope button over the end portal; particles fly at the
//! window from the screen's edges. Its close sends `UpdateFrequencyPacket` for the main hand's disk.
//! The theme is the alt one (`useAltTheme` is set in its static block): its textures and colours.
use crate::defs::{self as rows, Config, Scope};
use horizon_client::{
    game::ae2_ext::{Ae2AddonScreen, Ae2Host},
    hud::{HudAtlas, HudBatch, rgba},
    items::ItemStack,
    text::Text,
};
use std::{cell::RefCell, time::Instant};

/// `WINDOW_WIDTH`, `WINDOW_HEIGHT`.
pub(crate) const WIDTH: i32 = 176;
pub(crate) const HEIGHT: i32 = 105;
const SELECTOR_X: [i32; 3] = [33, 83, 133];
const SELECTOR_Y: i32 = 37;
const ARROW_X: [i32; 3] = [30, 80, 130];
const ARROW_TOP_Y: i32 = 18;
const ARROW_BOTTOM_Y: i32 = 63;
/// The frequency box (`EditBox` at 9, 89; 70 x 12; four characters, no border).
const FIELD: [i32; 4] = [9, 89, 70, 12];
const FIELD_MAX: usize = 4;
/// The transfer mode button (`CustomImageCycleButton`, the alt theme's 102, 85) and the scope's
/// (`CustomImageButton`, 120, 85).
const MODE_BUTTON: [i32; 4] = [102, 85, 15, 14];
const SCOPE_BUTTON: [i32; 4] = [120, 85, 50, 14];
/// The end portal behind the mask: 162 x 68 at 8, 18.
const PORTAL: [i32; 4] = [8, 18, 162, 68];
/// `TEXT_COLOR` of the alt theme.
const TEXT_COLOR: u32 = 0xFFE44C;
/// `DyeColor.getTextColor` by id.
const DYE_TEXT: [u32; 16] = [
    0xFFFFFF, 0xFF681F, 0xFF00FF, 0x9AC0CD, 0xFFFF00, 0xBFFF00, 0xFF69B4, 0x808080, 0xD3D3D3, 0x00FFFF, 0xA020F0, 0x0000FF,
    0x8B4513, 0x00FF00, 0xFF0000, 0x000000,
];
/// The steps of the three selectors (`getTooltipText`).
const STEP: [i32; 3] = [256, 16, 1];
/// The textures, kept in the GUI atlas as `ae2:enderdrives/gui/<name>` (content.json's `own_gui_textures`).
const MASK: &str = "ender_disk_mask_alt";

fn argb(color: u32, alpha: u32) -> u32 {
    rgba(((color >> 16) & 0xFF) as u8, ((color >> 8) & 0xFF) as u8, (color & 0xFF) as u8, alpha.min(255) as u8)
}

fn inside([x, y]: [f32; 2], [rx, ry, rw, rh]: [i32; 4]) -> bool {
    // `isHovering`: x in [rx, rx + w), y in [ry, ry + h), in whole pixels.
    let (x, y) = (x.floor() as i32, y.floor() as i32);
    x >= rx && x < rx + rw && y >= ry && y < ry + rh
}

/// `Particle2D`.
struct Particle {
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
    lifetime: f32,
    age: f32,
    size: f32,
    flicker: f32,
}

impl Particle {
    /// `getColor`: purple, flickering, fading with age.
    fn color(&self) -> u32 {
        let flicker = 0.5 + 0.5 * (self.age * 12.0 + self.flicker).sin();
        let alpha = (255.0 * (1.0 - self.age / self.lifetime) * flicker) as u32;
        argb((((120.0 + flicker * 80.0) as u32) << 16) | ((180.0 + flicker * 75.0) as u32), alpha)
    }
}

/// The open screen.
pub(crate) struct Open {
    pub(crate) frequency: i32,
    pub(crate) dyes: [i32; 3],
    pub(crate) scope: Scope,
    pub(crate) mode: i32,
    pub(crate) field: String,
    pub(crate) focused: bool,
    particles: Vec<Particle>,
    spawn_timer: f32,
    last_frame: Option<Instant>,
    opened: Instant,
    seed: u64,
}

impl Open {
    /// `new EnderDiskFrequencyScreen(frequency, scope, mode)` and `init`: a scope the config turns
    /// off is the default one.
    fn new(frequency: i32, scope: Scope, mode: i32, config: &Config) -> Self {
        let scope = if config.scope_enabled(scope) { scope } else { config.default_scope };
        let mut open = Self {
            frequency,
            dyes: [0; 3],
            scope,
            mode,
            field: frequency.to_string(),
            focused: false,
            particles: Vec::new(),
            spawn_timer: 0.0,
            last_frame: None,
            opened: Instant::now(),
            seed: 0x9E37_79B9_7F4A_7C15 ^ frequency as u64,
        };
        open.decode();
        open
    }
    /// `decodeFrequency`.
    fn decode(&mut self) {
        self.dyes = [(self.frequency >> 8) & 15, (self.frequency >> 4) & 15, self.frequency & 15];
    }
    /// `updateFrequency`: the dyes' frequency, clamped, written to the box.
    fn update(&mut self, config: &Config) {
        let raw = (self.dyes[0] << 8) | (self.dyes[1] << 4) | self.dyes[2];
        self.frequency = raw.min(config.frequency_max).max(config.frequency_min);
        self.set_field(self.frequency.to_string(), config);
    }
    /// `EditBox.setValue`, then its responder (`onFrequencyFieldChanged`).
    fn set_field(&mut self, value: String, config: &Config) {
        self.field = value.chars().take(FIELD_MAX).collect();
        if let Ok(n) = self.field.parse::<i32>()
            && n >= config.frequency_min
            && n <= config.frequency_max
        {
            self.frequency = n;
            self.decode();
        }
    }
    /// `cycleScope`: the next scope id; Team only with FTB Teams, which the pack's port has not.
    fn cycle_scope(&mut self, config: &Config) {
        let mut id = (self.scope.id() + 1) % 3;
        if id == Scope::Team.id() {
            id = 0;
        }
        self.scope = Scope::from_id(id, config);
    }
    fn random(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 40) as f32 / (1u64 << 24) as f32
    }
    fn random_between(&mut self, from: f32, to: f32) -> f32 {
        if to <= from { from } else { from + self.random() * (to - from) }
    }
    /// `spawnParticle`: from a random edge of the screen towards the window's middle.
    fn spawn(&mut self, (ox, oy): (f32, f32), (sw, sh): (f32, f32)) {
        let (cx, cy, margin) = (ox + 88.0, oy + 52.0, 20.0);
        let (w, h) = (WIDTH as f32, HEIGHT as f32);
        let side = (self.random() * 4.0) as i32;
        let (x, y) = match side {
            0 => (self.random_between(ox - margin, ox + w + margin), self.random_between(0.0, oy - margin)),
            1 => (self.random_between(ox - margin, ox + w + margin), self.random_between(oy + h + margin, sh)),
            2 => (self.random_between(0.0, ox - margin), self.random_between(oy, oy + h)),
            _ => (self.random_between(ox + w + margin, sw), self.random_between(oy, oy + h)),
        };
        let (dx, dy) = (cx - x, cy - y);
        let distance = (dx * dx + dy * dy).sqrt().max(1.0);
        let speed = 40.0 + self.random() * 20.0;
        let size = 2.0 + self.random() * 2.0;
        let flicker = self.random() * std::f32::consts::TAU;
        self.particles.push(Particle { x, y, dx: dx / distance * speed, dy: dy / distance * speed, lifetime: distance / speed, age: 0.0, size, flicker });
    }
    /// `renderUIParticles`' step: one spawn per 0.05 s of frames, the dead and those in the
    /// window dropped, the rest moved.
    fn step(&mut self, origin: (f32, f32), screen: (f32, f32)) {
        let now = Instant::now();
        let dt = self.last_frame.map_or(0.0, |last| now.duration_since(last).as_secs_f32()).min(0.25);
        self.last_frame = Some(now);
        self.spawn_timer += dt;
        if self.spawn_timer > 0.05 {
            self.spawn(origin, screen);
            self.spawn_timer -= 0.05;
        }
        let (l, t) = origin;
        let (r, b) = (l + WIDTH as f32, t + HEIGHT as f32);
        self.particles.retain(|p| {
            let within = p.x >= l && p.x <= r && p.y >= t && p.y <= b;
            p.age < p.lifetime && !within
        });
        for p in &mut self.particles {
            p.x += p.dx * dt;
            p.y += p.dy * dt;
            p.age += dt;
        }
    }
    /// The disk's state as `UpdateFrequencyPacket` carries it (`onClose`): the frequency clamped.
    fn packet(&self, config: &Config) -> (i32, i32, i32) {
        (self.frequency.min(config.frequency_max).max(config.frequency_min), self.scope.id(), self.mode)
    }
}

/// The screen of one game: open or not.
#[derive(Default)]
pub(crate) struct FrequencyScreen {
    open: RefCell<Option<Open>>,
}

/// `UpdateFrequencyPacket`'s body: three VarInts.
pub(crate) fn encode_update_frequency((frequency, scope, mode): (i32, i32, i32)) -> Vec<u8> {
    let mut writer = horizon_protocol::buf::Writer::new();
    writer.varint(frequency);
    writer.varint(scope);
    writer.varint(mode);
    writer.data
}

fn sprite(batch: &mut HudBatch, atlas: &HudAtlas, name: &str, x: f32, y: f32, w: f32, h: f32) {
    if let Some(r) = atlas.sprites.get(&format!("ae2:enderdrives/gui/{name}")).copied() {
        batch.rect_uv(x, y, w, h, r, u32::MAX);
    }
}

/// The top-left `w` x `h` pixels of a sprite, drawn at that size (`blit(texture, x, y, 0, 0, w, h, 256, 256)`):
/// the mask's art is 176 x 105 in a 256 x 256 file.
fn sprite_part(batch: &mut HudBatch, atlas: &HudAtlas, name: &str, x: f32, y: f32, w: f32, h: f32) {
    if let Some(mut r) = atlas.sprites.get(&format!("ae2:enderdrives/gui/{name}")).copied() {
        r.u1 = r.u0 + (r.u1 - r.u0) * w / r.w as f32;
        r.v1 = r.v0 + (r.v1 - r.v0) * h / r.h as f32;
        batch.rect_uv(x, y, w, h, r, u32::MAX);
    }
}

/// `fillCircle`: the pixels within `r` of the centre.
fn fill_circle(batch: &mut HudBatch, atlas: &HudAtlas, cx: f32, cy: f32, r: i32, color: u32) {
    for dx in -r..=r {
        for dy in -r..=r {
            if dx * dx + dy * dy <= r * r {
                batch.fill(atlas, cx + dx as f32, cy + dy as f32, 1.0, 1.0, color);
            }
        }
    }
}

/// `fillRoundedRect`: three fills and four corner circles.
fn fill_rounded(batch: &mut HudBatch, atlas: &HudAtlas, [x, y, w, h]: [f32; 4], r: i32, color: u32) {
    let rf = r as f32;
    batch.fill(atlas, x + rf, y, w - 2.0 * rf, h, color);
    batch.fill(atlas, x, y + rf, rf, h - 2.0 * rf, color);
    batch.fill(atlas, x + w - rf, y + rf, rf, h - 2.0 * rf, color);
    fill_circle(batch, atlas, x + rf, y + rf, r, color);
    fill_circle(batch, atlas, x + w - rf - 1.0, y + rf, r, color);
    fill_circle(batch, atlas, x + rf, y + h - rf - 1.0, r, color);
    fill_circle(batch, atlas, x + w - rf - 1.0, y + h - rf - 1.0, r, color);
}

/// The end portal under the mask (`RenderType.endPortal`): the port draws no portal shader in a
/// screen, so a dark field of slowly drifting specks in the portal's colours stands for it.
fn draw_portal(batch: &mut HudBatch, atlas: &HudAtlas, ox: f32, oy: f32, seconds: f32) {
    let [px, py, pw, ph] = PORTAL.map(|v| v as f32);
    batch.fill(atlas, ox + px, oy + py, pw, ph, rgba(9, 18, 24, 255));
    const COLOURS: [u32; 5] = [0x1E5A5E, 0x2E7D74, 0x3FA3A0, 0x4F6E9E, 0x6BC2B4];
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    for i in 0..90 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let (u, v) = ((seed & 0xFFFF) as f32 / 65535.0, ((seed >> 16) & 0xFFFF) as f32 / 65535.0);
        let layer = (i % 3) as f32 + 1.0;
        let x = (u * pw + seconds * 2.0 * layer).rem_euclid(pw);
        let y = (v * ph + seconds * 0.7 * layer).rem_euclid(ph);
        let size = if i % 7 == 0 { 2.0 } else { 1.0 };
        batch.fill(atlas, ox + px + x.min(pw - size), oy + py + y.min(ph - size), size, size, argb(COLOURS[(seed >> 33) as usize % COLOURS.len()], 255));
    }
}

/// `FrequencyScope.translationKey`.
fn scope_key(scope: Scope) -> &'static str {
    match scope {
        Scope::Global => "screen.enderdrives.scope.global",
        Scope::Personal => "screen.enderdrives.scope.private",
        Scope::Team => "screen.enderdrives.scope.team",
    }
}

impl FrequencyScreen {
    /// `ClientItemHooks.openFrequencyScreen` for the disk `stack`.
    pub(crate) fn open(&self, stack: &ItemStack) {
        let config = Config::get();
        let open = Open::new(rows::frequency(stack), rows::scope(stack, config), rows::transfer_mode(stack), config);
        *self.open.borrow_mut() = Some(open);
    }
    fn translated(host: &dyn Ae2Host, key: &str) -> String {
        host.translate(key).unwrap_or_else(|| key.to_owned())
    }
    /// The window's corner, as the game centres it.
    fn origin(batch: &HudBatch) -> (f32, f32) {
        batch.centered_origin(WIDTH as f32, HEIGHT as f32)
    }
}

impl Ae2AddonScreen for FrequencyScreen {
    fn size(&self, _host: &dyn Ae2Host) -> Option<(f32, f32)> {
        self.open.borrow().as_ref().map(|_| (WIDTH as f32, HEIGHT as f32))
    }

    /// `render`: the particles, the selectors, the portal and the mask, the title, the arrows'
    /// highlights, the widgets, the box's label while it is empty and not focused.
    fn draw(&self, host: &dyn Ae2Host, batch: &mut HudBatch, ox: f32, oy: f32) -> bool {
        let mut guard = self.open.borrow_mut();
        let Some(open) = guard.as_mut() else { return false };
        let atlas = host.hud_atlas();
        let mouse = host.gui_cursor();
        let local = [mouse[0] - ox, mouse[1] - oy];
        open.step((ox, oy), (batch.width, batch.height));
        for p in &open.particles {
            batch.fill(atlas, p.x.floor(), p.y.floor(), p.size.floor().max(1.0), p.size.floor().max(1.0), p.color());
        }
        // The portal is drawn after the selectors but 200 back (`translate(0, 0, -200)`), so it is
        // behind them; the mask after both, at their depth, over them.
        draw_portal(batch, atlas, ox, oy, open.opened.elapsed().as_secs_f32());
        for (i, dye) in open.dyes.iter().enumerate() {
            let rect = [ox + (SELECTOR_X[i] - 3) as f32, oy + (SELECTOR_Y - 2) as f32, 16.0, 30.0];
            fill_rounded(batch, atlas, rect, 3, argb(DYE_TEXT[*dye as usize & 15], 255));
        }
        sprite_part(batch, atlas, MASK, ox, oy, WIDTH as f32, HEIGHT as f32);
        let title = Self::translated(host, "screen.enderdrives.frequency_selector");
        batch.text(atlas, &title, ox + 12.0, oy + 6.0, argb(TEXT_COLOR, 255), false);
        for i in 0..3 {
            let (x, top, bottom) = (ARROW_X[i] as f32, ARROW_TOP_Y as f32, (ARROW_BOTTOM_Y + 1) as f32);
            if inside(local, [ARROW_X[i] + 4, ARROW_TOP_Y + 2, 8, 11]) {
                sprite(batch, atlas, "up_arrow_highlight_alt", ox + x, oy + top, 16.0, 16.0);
            }
            if inside(local, [ARROW_X[i] + 4, ARROW_BOTTOM_Y + 1 + 2, 8, 11]) {
                sprite(batch, atlas, "down_arrow_highlight_alt", ox + x, oy + bottom, 16.0, 16.0);
            }
        }
        // The box: its text with a shadow, the cursor while focused (`_` at the end, blinking).
        let [fx, fy, _, _] = FIELD.map(|v| v as f32);
        let advance = batch.text(atlas, &open.field, ox + fx, oy + fy, argb(TEXT_COLOR, 255), true);
        if open.focused && (open.opened.elapsed().as_millis() / 300) % 2 == 0 {
            batch.text(atlas, "_", ox + fx + advance, oy + fy, argb(TEXT_COLOR, 255), true);
        }
        // The transfer mode button: its texture by mode, hovered or not.
        let mode = match open.mode {
            1 => "transport_input",
            2 => "transport_output",
            _ => "transport_bidirectional",
        };
        let [mx, my, mw, mh] = MODE_BUTTON.map(|v| v as f32);
        let hovered = inside(local, MODE_BUTTON);
        let name = if hovered { format!("{mode}_hover_alt") } else { format!("{mode}_alt") };
        sprite(batch, atlas, &name, ox + mx, oy + my, mw, mh);
        // The scope button: its texture, the scope's name centred on it.
        let [sx, sy, sw, sh] = SCOPE_BUTTON.map(|v| v as f32);
        let scope_hovered = inside(local, SCOPE_BUTTON);
        sprite(batch, atlas, if scope_hovered { "scope_button_hover_alt" } else { "scope_button_alt" }, ox + sx, oy + sy, sw, sh);
        let scope = Self::translated(host, scope_key(open.scope));
        let width = HudBatch::text_width(atlas, &scope);
        batch.text(atlas, &scope, ox + sx + ((sw - width) / 2.0).floor(), oy + sy + ((sh - 8.0) / 2.0).floor(), argb(TEXT_COLOR, 255), false);
        if open.field.is_empty() && !open.focused {
            let label = Self::translated(host, "screen.enderdrives.frequency_label");
            batch.text(atlas, &label, ox + fx, oy + fy, argb(TEXT_COLOR, 255), true);
        }
        true
    }

    /// The widgets' tooltips (the buttons', in their `renderWidget`), then the arrows' (`render`).
    fn tooltip(&self, host: &dyn Ae2Host, batch: &mut HudBatch) -> bool {
        let guard = self.open.borrow();
        let Some(open) = guard.as_ref() else { return false };
        let (ox, oy) = Self::origin(batch);
        let mouse = host.gui_cursor();
        let local = [mouse[0] - ox, mouse[1] - oy];
        let key = if inside(local, MODE_BUTTON) {
            Some(match open.mode {
                1 => "enderdrives.transfer_mode.input".to_owned(),
                2 => "enderdrives.transfer_mode.output".to_owned(),
                _ => "enderdrives.transfer_mode.bidirectional".to_owned(),
            })
        } else if inside(local, SCOPE_BUTTON) {
            Some("screen.enderdrives.toggle_scope".to_owned())
        } else {
            (0..3).find_map(|i| {
                if inside(local, [ARROW_X[i], ARROW_TOP_Y, 16, 16]) {
                    Some(format!("screen.enderdrives.tooltip.plus_{}", STEP[i]))
                } else if inside(local, [ARROW_X[i], ARROW_BOTTOM_Y, 16, 16]) {
                    Some(format!("screen.enderdrives.tooltip.minus_{}", STEP[i]))
                } else {
                    None
                }
            })
        };
        let Some(key) = key else { return false };
        drop(guard);
        host.draw_tooltip(batch, &[Text::literal(Self::translated(host, &key))], mouse);
        true
    }

    /// `mouseClicked`: the left button only; an arrow steps its dye, else the widgets: the box
    /// takes the focus (and loses it to a press elsewhere), the buttons cycle.
    fn press(&self, _host: &mut dyn Ae2Host, at: [f32; 2], right: bool) -> bool {
        let mut guard = self.open.borrow_mut();
        let Some(open) = guard.as_mut() else { return false };
        if right {
            return false;
        }
        let config = Config::get();
        for i in 0..3 {
            if inside(at, [ARROW_X[i], ARROW_TOP_Y, 16, 16]) {
                open.dyes[i] = (open.dyes[i] + 1) % 16;
                open.update(config);
                return true;
            }
            if inside(at, [ARROW_X[i], ARROW_BOTTOM_Y, 16, 16]) {
                open.dyes[i] = (open.dyes[i] + 15) % 16;
                open.update(config);
                return true;
            }
        }
        open.focused = inside(at, FIELD);
        if inside(at, MODE_BUTTON) {
            open.mode = (open.mode + 1) % 3;
        } else if inside(at, SCOPE_BUTTON) {
            open.cycle_scope(config);
        }
        true
    }

    /// `EditBox.charTyped` while the box has the focus.
    fn text(&self, _host: &mut dyn Ae2Host, text: &str) -> bool {
        let mut guard = self.open.borrow_mut();
        let Some(open) = guard.as_mut().filter(|open| open.focused) else { return false };
        let typed: String = text.chars().filter(|c| !c.is_control() && *c != '\u{a7}').collect();
        let value = format!("{}{typed}", open.field);
        open.set_field(value, Config::get());
        true
    }

    /// `EditBox.keyPressed`: Backspace deletes the last character.
    fn key(&self, _host: &mut dyn Ae2Host, key: &str) -> bool {
        let mut guard = self.open.borrow_mut();
        let Some(open) = guard.as_mut().filter(|open| open.focused) else { return false };
        if key != "Backspace" {
            return false;
        }
        let mut value = open.field.clone();
        value.pop();
        open.set_field(value, Config::get());
        true
    }

    /// `onClose`: `UpdateFrequencyPacket` for the main hand's disk: to the server this game plays
    /// on, or straight to the disk in a world of its own.
    fn closed(&self, host: &mut dyn Ae2Host) {
        let Some(open) = self.open.borrow_mut().take() else { return };
        let config = Config::get();
        let packet = open.packet(config);
        if host.send_to_server(rows::UPDATE_FREQUENCY, &encode_update_frequency(packet)) {
            return;
        }
        let player = horizon_protocol::crypto::uuid_string(host.local_player_uuid());
        if let Some(mut stack) = host.main_hand_stack().cloned()
            && crate::game::apply_update_frequency(&mut stack, packet.0, packet.1, packet.2, &player, config)
        {
            host.set_main_hand_stack(stack);
        }
    }

    fn agent(&self, _host: &dyn Ae2Host) -> Option<serde_json::Value> {
        let guard = self.open.borrow();
        let open = guard.as_ref()?;
        let rect = |[x, y, w, h]: [i32; 4]| serde_json::json!([x, y, w, h]);
        Some(serde_json::json!({
            "frequency": open.frequency,
            "dyes": open.dyes,
            "scope": open.scope.name(),
            "mode": open.mode,
            "field": open.field,
            "focused": open.focused,
            "size": [WIDTH, HEIGHT],
            "arrows_up": (0..3).map(|i| rect([ARROW_X[i], ARROW_TOP_Y, 16, 16])).collect::<Vec<_>>(),
            "arrows_down": (0..3).map(|i| rect([ARROW_X[i], ARROW_BOTTOM_Y, 16, 16])).collect::<Vec<_>>(),
            "field_rect": rect(FIELD),
            "mode_button": rect(MODE_BUTTON),
            "scope_button": rect(SCOPE_BUTTON),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dyes_are_the_frequencys_hex_digits() {
        let config = Config::default();
        let mut open = Open::new(0x2A7, Scope::Global, 0, &config);
        assert_eq!(open.dyes, [2, 10, 7]);
        open.dyes[0] = (open.dyes[0] + 15) % 16;
        open.update(&config);
        assert_eq!((open.frequency, open.field.as_str()), (0x1A7, "423"));
        open.set_field("40".into(), &config);
        assert_eq!((open.frequency, open.dyes), (40, [0, 2, 8]));
        open.set_field("x".into(), &config);
        assert_eq!(open.frequency, 40, "a box that does not read keeps the frequency");
        open.set_field("123456".into(), &config);
        assert_eq!((open.field.as_str(), open.frequency), ("1234", 1234));
    }

    #[test]
    fn the_scope_cycles_past_team_and_the_packet_is_three_varints() {
        let config = Config::default();
        let mut open = Open::new(5, Scope::Global, 2, &config);
        open.cycle_scope(&config);
        assert_eq!(open.scope, Scope::Personal);
        open.cycle_scope(&config);
        assert_eq!(open.scope, Scope::Global, "no FTB Teams: Team is skipped");
        assert_eq!(encode_update_frequency((300, 1, 2)), vec![0xAC, 0x02, 1, 2]);
    }
}
