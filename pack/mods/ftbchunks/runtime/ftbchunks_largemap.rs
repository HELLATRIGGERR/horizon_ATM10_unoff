// FTB Chunks' map cache, large map and sidebar buttons (support module `crate::game::ftbchunks_largemap` of the
// `ftbchunks` native bundle).
// - The region cache: every chunk the client has seen, one file a 32x32-chunk region per dimension in the client's
//   FTB data folder (`local/ftbchunks/data/<manager>/<dim>/`, FTB Chunks' region names with our own extension, see
//   `ftbchunks_core::regions`), filled from the minimap extra's tiles (`native_map_tile`) a few chunks a frame round the
//   player, saved every few seconds and when the world closes. The claim screen and the minimap read it where a chunk
//   is not loaded.
// - The large map, FTB Chunks' `LargeMapScreen` (M): the cached terrain round a centre that a left drag moves and the
//   wheel zooms, the claims in their team's colour, waypoints with their right-click menu, the player; Space centres
//   it on the player, C (or the button) opens the claim screen, T teleports an operator to the block under the pointer.
// - FTB Library's sidebar buttons for FTB Chunks on the inventory screen: the claim screen and the map (API 432 closes
//   the inventory first).
// FTB Chunks is All Rights Reserved: written from its behaviour; the labels are translation keys with our own fallback
// wording.

use super::{Game, ftbchunks_map, ftbchunks_minimap, ftbteams_world};
use ftbchunks_core::{
    regions::{self, MapView, Region, TeleportFromMap},
    waypoints,
};
use horizon_client::{hud::HudBatch, text::Text};
use mod_api::ScreenView;
use std::{
    cell::RefCell,
    collections::{BTreeSet, HashMap},
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// The width of `text` in GUI pixels (upstream's `native_text_width` takes the batch it would draw into).
fn text_width(game: &Game<'_>, text: &str) -> f32 {
    horizon_client::hud::HudBatch::text_width(&game.gui.atlas, text)
}

const MOD: &str = "ftbchunks";
/// The large map's own screen id (the claim screen is `ftbchunks`).
pub(super) const SCREEN: &str = "ftbchunks:map";
/// The cache fills from the chunks this far round the player, a few a frame (changes show on the next pass).
const SCAN_RADIUS: i32 = 12;
const SCAN_PER_FRAME: usize = 8;
const SAVE_EVERY: Duration = Duration::from_secs(10);
const UNKNOWN: [u8; 4] = [16, 16, 16, 255];
const BACKGROUND: u32 = 0xFF10_1010;

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

fn tr(key: &str, fallback: &str, args: &[String]) -> String {
    let args: Vec<Text> = args.iter().map(|a| Text::literal(a.clone())).collect();
    Text::translate_with_fallback(key, fallback, &args).plain()
}

fn inside(r: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[1] >= r[1] && p[0] < r[0] + r[2] && p[1] < r[1] + r[3]
}

// ---- The region cache -------------------------------------------------------------------------------------

#[derive(Default)]
struct Cache {
    manager: Option<String>,
    /// `<instance>/local/ftbchunks/data/<manager>`.
    base: Option<PathBuf>,
    dims: HashMap<String, HashMap<(i32, i32), Region>>,
    dirty: BTreeSet<(String, i32, i32)>,
    saved: Option<Instant>,
    next: usize,
    /// Chunk pictures kept, region files read and written (the probe's).
    stored: usize,
    read: usize,
    written: usize,
}

impl Cache {
    fn path(&self, dim: &str, rx: i32, rz: i32) -> Option<PathBuf> {
        Some(self.base.as_ref()?.join(waypoints::dimension_dir(dim)).join(regions::region_file(rx, rz)))
    }

    /// Region `rx, rz` of `dim`, read from its file the first time.
    fn region(&mut self, dim: &str, rx: i32, rz: i32) -> &mut Region {
        let path = self.path(dim, rx, rz);
        let read = &mut self.read;
        self.dims.entry(dim.to_owned()).or_default().entry((rx, rz)).or_insert_with(|| {
            let region = path.and_then(|p| std::fs::read(p).ok()).and_then(|b| Region::decode(&b));
            *read += usize::from(region.is_some());
            region.unwrap_or_default()
        })
    }

    fn put(&mut self, dim: &str, chunk: [i32; 2], tile: &[u8]) {
        if self.base.is_none() {
            return;
        }
        let (rx, rz) = regions::region_of(chunk[0], chunk[1]);
        if self.region(dim, rx, rz).put(chunk[0], chunk[1], now_ms(), tile) {
            self.stored += 1;
            self.dirty.insert((dim.to_owned(), rx, rz));
        }
    }

    /// Writes the regions that changed (to a temporary file, then renamed over the old one).
    fn save(&mut self) {
        for (dim, rx, rz) in std::mem::take(&mut self.dirty) {
            let (Some(path), Some(region)) = (self.path(&dim, rx, rz), self.dims.get(&dim).and_then(|d| d.get(&(rx, rz)))) else {
                continue;
            };
            let tmp = path.with_extension("tmp");
            let written = path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&tmp, region.encode()))
                .and_then(|()| std::fs::rename(&tmp, &path));
            match written {
                Ok(()) => self.written += 1,
                Err(e) => log::warn!("[ftbchunks] saving {}: {e}", path.display()),
            }
        }
        self.saved = Some(Instant::now());
    }
}

thread_local! {
    static CACHE: RefCell<Cache> = RefCell::new(Cache::default());
    static LARGE: RefCell<Large> = RefCell::new(Large::default());
}

/// Follows the FTB Teams manager `login_data` gave (each server its own folder): another one saves and drops the old.
fn sync(game: &Game<'_>) {
    let manager = ftbchunks_map::manager();
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.manager != manager {
            c.save();
            c.dims.clear();
            c.base = manager.as_ref().map(|m| game.native_instance_dir().join("local").join("ftbchunks").join("data").join(m));
            c.manager = manager;
        }
    });
}

/// Chunk `chunk` of `dim` (the player's dimension): the live picture where it is loaded (kept in the cache), else the
/// cached one.
pub(super) fn tile(game: &mut Game<'_>, dim: &str, chunk: [i32; 2]) -> Option<[u8; 1024]> {
    if let Some(t) = game.native_map_tile(chunk) {
        CACHE.with(|c| c.borrow_mut().put(dim, chunk, &t));
        return Some(t);
    }
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        c.base.as_ref()?;
        let (rx, rz) = regions::region_of(chunk[0], chunk[1]);
        c.region(dim, rx, rz).get(chunk[0], chunk[1]).and_then(|t| t.try_into().ok())
    })
}

/// A few chunks round the player into the cache, and a save when one is due.
fn scan(game: &mut Game<'_>) {
    let Some(p) = game.native_player() else { return };
    if CACHE.with(|c| c.borrow().base.is_none()) {
        return;
    }
    let dim = p.dimension.to_string();
    let (cx, cz) = ((p.x.floor() as i32) >> 4, (p.z.floor() as i32) >> 4);
    let side = (2 * SCAN_RADIUS + 1) as usize;
    let start = CACHE.with(|c| c.borrow().next);
    for k in 0..SCAN_PER_FRAME {
        let i = (start + k) % (side * side);
        let chunk = [cx - SCAN_RADIUS + (i % side) as i32, cz - SCAN_RADIUS + (i / side) as i32];
        if let Some(t) = game.native_map_tile(chunk) {
            CACHE.with(|c| c.borrow_mut().put(&dim, chunk, &t));
        }
    }
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        c.next = (start + SCAN_PER_FRAME) % (side * side);
        if c.saved.is_none_or(|t| t.elapsed() >= SAVE_EVERY) {
            c.save();
        }
    });
}

/// `native_world_lifecycle`: the cache is written when the world closes, and forgotten.
pub(super) fn world_closed() {
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        c.save();
        *c = Cache::default();
    });
}

// ---- The large map ----------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
enum Pending {
    Claim,
    Map,
}

/// The image last uploaded: the view and area it was made for, its size, and its pixels a GUI pixel.
#[derive(Clone, Copy, PartialEq)]
struct Composed {
    view: MapView,
    rect: [f32; 4],
    size: [u32; 2],
    scale: f32,
}

struct Drag {
    left: bool,
    start: [f32; 2],
    last: [f32; 2],
}

#[derive(Default)]
struct Large {
    open: bool,
    view: MapView,
    dim: String,
    rect: [f32; 4],
    mouse: [f32; 2],
    button: [f32; 4],
    drag: Option<Drag>,
    composed: Option<Composed>,
    at: Option<Instant>,
    uploads: usize,
    /// Pixels of the last image with cached terrain (the probe's).
    known: usize,
    pending: Option<Pending>,
    teleports: usize,
}

pub(super) fn screen_open(game: &Game<'_>) -> bool {
    game.native_mod_screen() == Some(SCREEN)
}

/// Opens the large map on the player, at the zoom it had.
pub(super) fn open(game: &mut Game<'_>) -> bool {
    let Some(p) = game.native_player() else { return false };
    if !game.native_mod_screen_open(SCREEN) {
        return false;
    }
    LARGE.with(|l| {
        let mut l = l.borrow_mut();
        l.open = true;
        l.view.center = [p.x, p.z];
        l.dim = p.dimension.to_string();
        l.drag = None;
        l.composed = None;
        l.at = None;
    });
    true
}

/// `native_frame`: the cache's scan and saves, the buttons' screens, and the large map's image when its view changed
/// (at most every 40 ms while it moves, else every second for what the cache gained).
pub(super) fn frame(game: &mut Game<'_>) {
    sync(game);
    scan(game);
    if let Some(p) = LARGE.with(|l| l.borrow_mut().pending.take()) {
        game.native_close_inventory();
        game.native_mod_screen_close(SCREEN);
        match p {
            Pending::Claim => ftbchunks_map::open(game),
            Pending::Map => open(game),
        };
    }
    let shown = screen_open(game);
    let mut closed = false;
    let job = LARGE.with(|l| {
        let mut l = l.borrow_mut();
        if !shown {
            closed = std::mem::take(&mut l.open);
            l.drag = None;
            return None;
        }
        if l.rect[2] <= 0.0 || l.rect[3] <= 0.0 {
            return None;
        }
        let stale = l.composed.is_none_or(|c| c.view != l.view || c.rect != l.rect);
        let every = Duration::from_millis(if stale { 40 } else { 1000 });
        l.at.is_none_or(|t| t.elapsed() >= every).then(|| (l.view, l.rect, l.dim.clone()))
    });
    if closed {
        ftbchunks_minimap::close_menu();
        CACHE.with(|c| c.borrow_mut().save());
    }
    let Some((view, rect, dim)) = job else { return };
    let (image, size, scale, known) = compose(view, rect, &dim);
    let ok = game.native_mod_image(size[0], size[1], &image);
    LARGE.with(|l| {
        let mut l = l.borrow_mut();
        l.at = Some(Instant::now());
        l.known = known;
        if ok {
            l.composed = Some(Composed { view, rect, size, scale });
            l.uploads += 1;
        }
    });
    if ok {
        ftbchunks_minimap::image_taken();
    }
}

/// The map's picture for `view` over `rect`: one pixel a GUI pixel (fewer past the mod image's 1024), each the
/// cached terrain of the block there, claims in their team's colour (alpha 100) with a full-colour line where the
/// next pixel is another's, as FTB Chunks' map shows them.
fn compose(view: MapView, rect: [f32; 4], dim: &str) -> (Vec<u8>, [u32; 2], f32, usize) {
    let s = (1024.0 / rect[2]).min(1024.0 / rect[3]).min(1.0);
    let (w, h) = (((rect[2] * s).floor() as usize).max(1), ((rect[3] * s).floor() as usize).max(1));
    let xs: Vec<i32> = (0..w).map(|i| view.block_at(rect, [rect[0] + (i as f32 + 0.5) / s, rect[1]])[0].floor() as i32).collect();
    let zs: Vec<i32> = (0..h).map(|j| view.block_at(rect, [rect[0], rect[1] + (j as f32 + 0.5) / s])[1].floor() as i32).collect();
    let claims = ftbchunks_map::claims_in(dim);
    let colors: HashMap<u128, u32> =
        ftbteams_world::with_client(|t| claims.values().map(|team| (*team, t.color(*team).unwrap_or(0xFF_FFFF))).collect());
    let mut image = UNKNOWN.repeat(w * h);
    let mut owner = vec![0u128; w * h];
    let mut known = 0;
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        for rz in zs[0] >> 9..=zs[h - 1] >> 9 {
            for rx in xs[0] >> 9..=xs[w - 1] >> 9 {
                c.region(dim, rx, rz);
            }
        }
        let Some(cached) = c.dims.get(dim) else { return };
        for (j, &z) in zs.iter().enumerate() {
            let mut last: Option<((i32, i32), Option<&[u8]>, u128)> = None;
            for (i, &x) in xs.iter().enumerate() {
                let key = (x >> 4, z >> 4);
                let (tile, team) = match last {
                    Some((k, t, o)) if k == key => (t, o),
                    _ => {
                        let t = cached.get(&(x >> 9, z >> 9)).and_then(|r| r.get(key.0, key.1));
                        let o = claims.get(&key).copied().unwrap_or(0);
                        last = Some((key, t, o));
                        (t, o)
                    }
                };
                if let Some(t) = tile {
                    known += 1;
                    let k = (((z & 15) * 16 + (x & 15)) * 4) as usize;
                    let o = (j * w + i) * 4;
                    image[o..o + 4].copy_from_slice(&t[k..k + 4]);
                }
                owner[j * w + i] = team;
            }
        }
    });
    for j in 0..h {
        for i in 0..w {
            let team = owner[j * w + i];
            if team == 0 {
                continue;
            }
            let other = |ii: usize, jj: usize| owner[jj * w + ii] != team;
            let edge = (i > 0 && other(i - 1, j)) || (i + 1 < w && other(i + 1, j)) || (j > 0 && other(i, j - 1)) || (j + 1 < h && other(i, j + 1));
            let a: u32 = if edge { 255 } else { 100 };
            let [_, r, g, b] = colors.get(&team).copied().unwrap_or(0xFF_FFFF).to_be_bytes();
            let o = (j * w + i) * 4;
            for (k, c) in [r, g, b].into_iter().enumerate() {
                image[o + k] = ((u32::from(image[o + k]) * (255 - a) + u32::from(c) * a) / 255) as u8;
            }
        }
    }
    (image, [w as u32, h as u32], s, known)
}

/// The large map as the waypoint markers see it.
fn map_frame(view: MapView, rect: [f32; 4]) -> ftbchunks_minimap::MapFrame {
    let corner = view.block_at(rect, [rect[0], rect[1]]);
    let origin = [corner[0].floor() as i32, corner[1].floor() as i32];
    let at = view.point_of(rect, [f64::from(origin[0]), f64::from(origin[1])]);
    ftbchunks_minimap::MapFrame { clip: rect, at, per_block: view.zoom as f32, origin }
}

/// `native_screen_render_post`: the large map, or the sidebar buttons on the inventory screen.
pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &ScreenView<'_>) {
    if sidebar_screen(screen) {
        sidebar(game, batch, screen);
        return;
    }
    if !screen_open(game) {
        return;
    }
    let rect = screen.rect;
    game.native_fill(batch, rect, BACKGROUND);
    let (view, dim) = LARGE.with(|l| {
        let mut l = l.borrow_mut();
        l.rect = rect;
        l.mouse = screen.mouse;
        if let Some(c) = l.composed {
            let b0 = c.view.block_at(c.rect, [c.rect[0], c.rect[1]]);
            let far = [c.rect[0] + c.size[0] as f32 / c.scale, c.rect[1] + c.size[1] as f32 / c.scale];
            let b1 = c.view.block_at(c.rect, far);
            let (p0, p1) = (l.view.point_of(rect, b0), l.view.point_of(rect, b1));
            let src = [0.0, 0.0, c.size[0] as f32, c.size[1] as f32];
            game.native_sprite(batch, horizon_client::hud::MOD_IMAGE_SPRITE, src, [p0[0], p0[1], p1[0] - p0[0], p1[1] - p0[1]], None);
        }
        (l.view, l.dim.clone())
    });
    ftbchunks_minimap::draw_on_map(game, batch, &map_frame(view, rect), &dim);
    // The player: a dot with a short line where they look.
    if let Some(p) = game.native_player() {
        let q = view.point_of(rect, [p.x, p.z]);
        let (sin, cos) = (p.yaw + 180.0).to_radians().sin_cos();
        for k in 1..=4 {
            let d = k as f32 * 1.5;
            game.native_fill(batch, [q[0] - sin * d - 0.5, q[1] + cos * d - 0.5, 1.0, 1.0], 0xFFFF_FFFF);
        }
        game.native_fill(batch, [q[0] - 2.0, q[1] - 2.0, 4.0, 4.0], 0xFF00_0000);
        game.native_fill(batch, [q[0] - 1.0, q[1] - 1.0, 2.0, 2.0], 0xFFFF_FFFF);
    }
    // The claim screen's button (C), top left.
    let label = tr("ftbchunks.gui.claimed_chunks", "Claim map", &[]);
    let button = [rect[0] + 4.0, rect[1] + 4.0, text_width(game, &label) + 10.0, 14.0];
    let hover = inside(button, screen.mouse);
    game.native_fill(batch, button, if hover { 0xE060_6060 } else { 0xC020_2020 });
    game.native_text(batch, &label, [button[0] + 5.0, button[1] + 3.0], 0xFFFFFF, true);
    LARGE.with(|l| l.borrow_mut().button = button);
    // The block under the pointer, and its claim's team.
    let b = view.block_at(rect, screen.mouse);
    let (bx, bz) = (b[0].floor() as i32, b[1].floor() as i32);
    let mut line = format!("{bx}, {bz}");
    if let Some(team) = ftbchunks_map::claims_in(&dim).get(&(bx >> 4, bz >> 4)) {
        let name = ftbteams_world::with_client(|t| t.teams.get(team).and_then(|t| t.properties.get("ftbteams:display_name").cloned()));
        if let Some(name) = name {
            line += &format!("  {name}");
        }
    }
    game.native_text(batch, &line, [rect[0] + 4.0, rect[1] + rect[3] - 12.0], 0xFFFFFF, true);
    ftbchunks_minimap::draw_menu(game, batch, screen.mouse);
}

/// `native_screen_mouse_button_pressed_pre`: a left drag pans, a right click opens the waypoint menu; the button
/// opens the claim screen. On the inventory screen: the sidebar buttons.
pub(super) fn pressed(game: &Game<'_>, screen: &ScreenView<'_>, button: i32, canceled: &mut bool) {
    if sidebar_screen(screen) {
        if let Some(p) = sidebar_buttons().into_iter().find(|(_, r, _, _)| inside(*r, screen.mouse)).map(|(p, ..)| p) {
            *canceled = true;
            LARGE.with(|l| l.borrow_mut().pending = Some(p));
        }
        return;
    }
    if !screen_open(game) || !(button == 0 || button == 1) {
        return;
    }
    *canceled = true;
    if ftbchunks_minimap::menu_click(game, screen.mouse, button) {
        return;
    }
    LARGE.with(|l| {
        let mut l = l.borrow_mut();
        if button == 0 && inside(l.button, screen.mouse) {
            l.pending = Some(Pending::Claim);
            return;
        }
        l.drag = Some(Drag { left: button == 0, start: screen.mouse, last: screen.mouse });
    });
}

/// `native_screen_mouse_dragged_pre`: the map follows the pointer.
pub(super) fn dragged(game: &Game<'_>, screen: &ScreenView<'_>, canceled: &mut bool) {
    if !screen_open(game) {
        return;
    }
    *canceled = true;
    LARGE.with(|l| {
        let mut l = l.borrow_mut();
        let Some(drag) = l.drag.as_mut() else { return };
        let d = [screen.mouse[0] - drag.last[0], screen.mouse[1] - drag.last[1]];
        drag.last = screen.mouse;
        let left = drag.left;
        if left {
            l.view.pan(d);
        }
    });
}

/// `native_screen_mouse_button_released_pre`: a right click that stayed put opens FTB Chunks' map menu there.
pub(super) fn released(game: &Game<'_>, button: i32, canceled: &mut bool) {
    if !screen_open(game) || !(button == 0 || button == 1) {
        return;
    }
    *canceled = true;
    let Some((drag, view, rect, dim)) = LARGE.with(|l| {
        let mut l = l.borrow_mut();
        let drag = l.drag.take()?;
        Some((drag, l.view, l.rect, l.dim.clone()))
    }) else {
        return;
    };
    if drag.left || (drag.last[0] - drag.start[0]).hypot(drag.last[1] - drag.start[1]) >= 3.0 {
        return;
    }
    let b = view.block_at(rect, drag.last);
    let y = game.native_player().map_or(64, |p| p.y.floor() as i32);
    ftbchunks_minimap::open_menu(game, &map_frame(view, rect), &dim, drag.last, [b[0].floor() as i32, y, b[1].floor() as i32], true);
}

/// `native_screen_mouse_scrolled_pre`: the wheel zooms round the pointer, doubling a notch.
pub(super) fn scrolled(game: &Game<'_>, screen: &ScreenView<'_>, scroll: [f64; 2], canceled: &mut bool) {
    if !screen_open(game) || scroll[1] == 0.0 {
        return;
    }
    *canceled = true;
    let notches = if scroll[1] > 0.0 { 1 } else { -1 };
    LARGE.with(|l| {
        let mut l = l.borrow_mut();
        let rect = l.rect;
        l.view.zoom_at(rect, screen.mouse, notches);
    });
}

/// `native_key`: M opens the large map while playing and closes it; on it Space centres it on the player, C opens the
/// claim screen and T teleports an operator to the block under the pointer (FTB Chunks' keys).
pub(super) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if !pressed {
        return;
    }
    let map_key = game.native_mod_key(MOD, "map", "key.keyboard.m");
    if !screen_open(game) {
        if key == map_key && game.native_playing() && !ftbchunks_map::screen_open(game) {
            open(game);
        }
        return;
    }
    match key {
        k if k == map_key => game.native_mod_screen_close(SCREEN),
        "key.keyboard.space" => {
            if let Some(p) = game.native_player() {
                LARGE.with(|l| l.borrow_mut().view.center = [p.x, p.z]);
            }
        }
        "key.keyboard.c" => {
            game.native_mod_screen_close(SCREEN);
            ftbchunks_map::open(game);
        }
        "key.keyboard.t" => teleport(game),
        _ => {}
    }
}

/// `teleport_from_map`: the server takes it from an operator only, and finds the surface there.
fn teleport(game: &mut Game<'_>) {
    let msg = LARGE.with(|l| {
        let l = l.borrow();
        let b = l.view.block_at(l.rect, l.mouse);
        TeleportFromMap { pos: [b[0].floor() as i32, 0, b[1].floor() as i32], unknown_y: true, dim: l.dim.clone() }
    });
    let data = serde_json::to_vec(&msg).unwrap_or_default();
    match game.native_net_send_to_server(regions::TELEPORT_FROM_MAP, &data) {
        Ok(()) => LARGE.with(|l| l.borrow_mut().teleports += 1),
        Err(e) => log::debug!("[ftbchunks] {}: {e}", regions::TELEPORT_FROM_MAP),
    }
}

// ---- FTB Library's sidebar buttons ------------------------------------------------------------------------

/// The player's own inventory (no menu from the server, no title).
fn sidebar_screen(screen: &ScreenView<'_>) -> bool {
    screen.id == "inventory" && screen.menu.is_none() && screen.title.is_none()
}

/// FTB Library's sidebar sits in the screen's top-left corner, a 16-pixel icon a button; FTB Chunks gives it two.
fn sidebar_buttons() -> [(Pending, [f32; 4], &'static str, &'static str); 2] {
    [
        (Pending::Claim, [2.0, 2.0, 16.0, 16.0], "sidebar_button.ftbchunks.chunks", "Claim map"),
        (Pending::Map, [20.0, 2.0, 16.0, 16.0], "sidebar_button.ftbchunks.map", "World map"),
    ]
}

/// The buttons, with icons of our own drawn in fills (FTB's textures are not ours to ship), and the hovered one's name.
fn sidebar(game: &Game<'_>, batch: &mut HudBatch, screen: &ScreenView<'_>) {
    let team_rgb = game
        .native_player()
        .and_then(|_| ftbteams_world::with_client(|t| t.self_team.and_then(|id| t.color(id))))
        .unwrap_or(0x55_AAFF);
    for (kind, r, key, fallback) in sidebar_buttons() {
        let hover = inside(r, screen.mouse);
        game.native_fill(batch, r, if hover { 0xFFA0_A0A0 } else { 0xFF6B_6B6B });
        game.native_fill(batch, [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0], 0xFF2A_2A2A);
        let (x, y) = (r[0] + 3.0, r[1] + 3.0);
        match kind {
            // A 2x2 grid of chunks, one claimed.
            Pending::Claim => {
                for (k, rgb) in [0x5C_8A3A, 0x5C_8A3A, 0x5C_8A3A, team_rgb].into_iter().enumerate() {
                    let (u, v) = ((k % 2) as f32 * 5.0, (k / 2) as f32 * 5.0);
                    game.native_fill(batch, [x + u, y + v, 4.0, 4.0], 0xFF00_0000 | rgb);
                }
            }
            // Land and water.
            Pending::Map => {
                game.native_fill(batch, [x, y, 10.0, 10.0], 0xFF5C_8A3A);
                game.native_fill(batch, [x + 5.0, y, 5.0, 4.0], 0xFF3F_76E4);
                game.native_fill(batch, [x, y + 7.0, 4.0, 3.0], 0xFFC2_B280);
            }
        }
        if hover {
            let name = tr(key, fallback, &[]);
            let w = text_width(game, &name);
            let at = [screen.mouse[0] + 10.0, screen.mouse[1] + 2.0];
            game.native_fill(batch, [at[0] - 2.0, at[1] - 2.0, w + 4.0, 12.0], 0xF010_0010);
            game.native_text(batch, &name, at, 0xFFFFFF, true);
        }
    }
}

// ---- The tests' probe -------------------------------------------------------------------------------------

/// The cache and the large map, for `/ftbchunksclient`.
pub(super) fn probe() -> serde_json::Value {
    let cache = CACHE.with(|c| {
        let c = c.borrow();
        let tiles: usize = c.dims.values().flat_map(|d| d.values()).map(|r| r.tiles.len()).sum();
        serde_json::json!({
            "folder": c.base.as_ref().map(|b| b.display().to_string()),
            "regions": c.dims.values().map(|d| d.values().filter(|r| !r.tiles.is_empty()).count()).sum::<usize>(),
            "tiles": tiles,
            "stored": c.stored,
            "read": c.read,
            "written": c.written,
            "dirty": c.dirty.len(),
        })
    });
    LARGE.with(|l| {
        let l = l.borrow();
        serde_json::json!({
            "open": l.open,
            "center": l.view.center,
            "zoom": l.view.zoom,
            "rect": l.rect,
            "button": l.button,
            "uploads": l.uploads,
            "known": l.known,
            "image": l.composed.map(|c| c.size),
            "teleports": l.teleports,
            "sidebar": sidebar_buttons().map(|(_, r, _, _)| r),
            "cache": cache,
        })
    })
}

/// `/ftbchunksbigmap`: opens the large map as M does; `save` writes the cache now; `has <cx> <cz>`: whether that chunk
/// of the player's dimension is cached and loaded, and its region file; `center <x> <z>` moves the open map there.
pub(super) fn command(game: &mut Game<'_>, args: &str) -> serde_json::Value {
    let words: Vec<&str> = args.split_whitespace().collect();
    let num = |i: usize| words.get(i).and_then(|w| w.parse::<i32>().ok()).unwrap_or(0);
    match words.first().copied() {
        Some("save") => {
            CACHE.with(|c| c.borrow_mut().save());
            return serde_json::json!({ "saved": true });
        }
        Some("has") => {
            let chunk = [num(1), num(2)];
            let dim = game.native_player().map(|p| p.dimension.to_string()).unwrap_or_default();
            let live = game.native_map_tile(chunk).is_some();
            return CACHE.with(|c| {
                let mut c = c.borrow_mut();
                let (rx, rz) = regions::region_of(chunk[0], chunk[1]);
                let folder = c.base.as_ref().map(|b| b.join(waypoints::dimension_dir(&dim)).display().to_string());
                let cached = c.base.is_some() && c.region(&dim, rx, rz).get(chunk[0], chunk[1]).is_some();
                serde_json::json!({ "cached": cached, "live": live, "folder": folder, "file": regions::region_file(rx, rz) })
            });
        }
        Some("center") => {
            LARGE.with(|l| l.borrow_mut().view.center = [f64::from(num(1)) + 0.5, f64::from(num(2)) + 0.5]);
            return serde_json::json!({ "center": [num(1), num(2)] });
        }
        _ => {}
    }
    game.native_mod_screen_close(MOD);
    serde_json::json!({ "opened": open(game) })
}
