// FTB Chunks' waypoints and corner minimap in the game (support module `crate::game::ftbchunks_minimap` of the
// `ftbchunks` native bundle). Waypoints: one `waypoints.json` a dimension under `local/ftbchunks/data/<manager id>/`
// as FTB Chunks keeps them; `add_waypoint` (`/ftbchunks waypoint add`) and `player_death` (the death waypoint) from
// the server; markers and a right-click menu (add here, hide or show, remove) on the claim screen. The minimap: FTB
// Chunks' HUD map, on by default unless another minimap mod is built in, in the top right corner, a tenth of the GUI
// width, north up, 64 blocks across, round; terrain from the minimap extra's tiles (`native_map_tile`) uploaded with
// `native_mod_image`, claims in their team's colour, waypoints (on the rim when farther), the player's arrow, the
// compass letters, and the position, biome and claim lines under it.
// FTB Chunks is All Rights Reserved: this is written from its behaviour; labels are its translation keys with our
// own fallback wording.

use super::{Game, ftbchunks_largemap, ftbchunks_map, ftbteams_world};
use ftbchunks_core::waypoints::{self, AddWaypoint, PlayerDeath, Waypoints};
use horizon_client::{
    extras::Corner,
    extras_hud::Corners,
    hud::{HudBatch, MOD_IMAGE_SPRITE},
    text::Text,
};
use std::{
    cell::RefCell,
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};

/// The width of `text` in GUI pixels (upstream's `native_text_width` takes the batch it would draw into).
fn text_width(game: &Game<'_>, text: &str) -> f32 {
    horizon_client::hud::HudBatch::text_width(&game.gui.atlas, text)
}

const MOD: &str = "ftbchunks";
/// Chunks a side of the minimap's image, centred on the player's chunk (64 blocks show, plus the chunk's part).
const CHUNKS: i32 = 5;
const SIDE: usize = CHUNKS as usize * 16;
/// Blocks across the minimap at zoom 1 (FTB Chunks: a quarter of its 240-block texture).
const BLOCKS: f32 = 64.0;
/// `minimap.scale`, `minimap.font_scale`.
const SCALE: f32 = 1.0;
const FONT_SCALE: f32 = 0.5;
const UNKNOWN: [u8; 4] = [16, 16, 16, 255];
const RESCAN: Duration = Duration::from_millis(500);

#[derive(Clone, Copy)]
enum Action {
    Add,
    Toggle(usize),
    Remove(usize),
}

struct Menu {
    at: [f32; 2],
    block: [i32; 3],
    dim: String,
    items: Vec<(String, Action)>,
}

#[derive(Default)]
struct Mini {
    /// `minimap.enabled`, once toggled; None: the default.
    enabled: Option<bool>,
    manager: Option<String>,
    dims: HashMap<String, Waypoints>,
    image: Vec<u8>,
    origin: (i32, i32),
    dim: String,
    scanned: Option<Instant>,
    uploaded: bool,
    tiles: usize,
    shown: bool,
    rect: [f32; 4],
    menu: Option<Menu>,
}

thread_local! {
    static MINI: RefCell<Mini> = RefCell::new(Mini::default());
}

fn tr(key: &str, fallback: &str, args: &[String]) -> String {
    let args: Vec<Text> = args.iter().map(|a| Text::literal(a.clone())).collect();
    Text::translate_with_fallback(key, fallback, &args).plain()
}

/// FTB Chunks' `minimap.enabled` default: on unless another minimap mod's bundle is built into this game.
fn enabled_by_default() -> bool {
    waypoints::minimap_on_by_default(option_env!("HORIZON_MIXIN_MODS").unwrap_or_default())
}

fn enabled() -> bool {
    MINI.with(|m| m.borrow().enabled).unwrap_or_else(enabled_by_default)
}

// ---- Waypoints --------------------------------------------------------------------------------------------

pub(super) fn folder(game: &Game<'_>, manager: &str, dim: &str) -> PathBuf {
    game.native_instance_dir().join("local").join("ftbchunks").join("data").join(manager).join(waypoints::dimension_dir(dim))
}

/// `f` over dimension `dim`'s waypoints (read from the file the first time), saved after when `save`.
fn with_dim<R>(game: &Game<'_>, dim: &str, save: bool, f: impl FnOnce(&mut Waypoints) -> R) -> Option<R> {
    let manager = ftbchunks_map::manager()?;
    MINI.with(|m| {
        let mut m = m.borrow_mut();
        if m.manager.as_deref() != Some(manager.as_str()) {
            m.manager = Some(manager.clone());
            m.dims.clear();
        }
        let dir = folder(game, &manager, dim);
        let list = m.dims.entry(dim.to_owned()).or_insert_with(|| {
            std::fs::read_to_string(dir.join(waypoints::FILE)).map(|s| Waypoints::parse(&s)).unwrap_or_default()
        });
        let r = f(list);
        if save {
            let written = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(dir.join(waypoints::FILE), list.to_json()));
            if let Err(e) = written {
                log::warn!("[ftbchunks] saving {}: {e}", dir.display());
            }
        }
        Some(r)
    })
}

/// `native_net_from_server`: `add_waypoint` and `player_death`.
pub(super) fn from_server(game: &mut Game<'_>, channel: &str, data: &[u8]) {
    match channel {
        waypoints::ADD_WAYPOINT => match serde_json::from_slice::<AddWaypoint>(data) {
            // FTB Chunks opens its add screen for `gui`; without it the waypoint is added as given.
            Ok(a) => {
                with_dim(game, &a.dim, true, |w| w.add(&a.name, a.pos, a.color));
            }
            Err(e) => log::warn!("[ftbchunks] a bad {channel} message: {e}"),
        },
        waypoints::PLAYER_DEATH => match serde_json::from_slice::<PlayerDeath>(data) {
            // `waypoints.death_waypoints` is on by default.
            Ok(d) => {
                with_dim(game, &d.dim, true, |w| {
                    w.add_death(d.pos, d.deaths);
                });
            }
            Err(e) => log::warn!("[ftbchunks] a bad {channel} message: {e}"),
        },
        _ => {}
    }
}

fn diamond(game: &Game<'_>, batch: &mut HudBatch, c: [f32; 2], rgb: u32, alpha: u32) {
    let (x, y) = (c[0].floor(), c[1].floor());
    for (r, color) in [(2i32, alpha << 24), (1, alpha << 24 | (rgb & 0xFF_FFFF))] {
        for i in -r..=r {
            let w = (r - i.abs()) as f32;
            game.native_fill(batch, [x - w, y + i as f32, 2.0 * w + 1.0, 1.0], color);
        }
    }
}

fn small_text(game: &Game<'_>, batch: &mut HudBatch, text: &str, center: [f32; 2], rgb: u32, scale: f32) {
    let w = text_width(game, text);
    let start = batch.vertices.len();
    game.native_text(batch, text, [center[0] - w / 2.0, center[1]], rgb, true);
    if scale != 1.0 {
        batch.scale_since(start, center[0], center[1], scale, scale);
    }
}

/// The markers on a map (the claim screen's, the large map's): the dimension's waypoints inside `map.clip`, names
/// above them; hidden ones faint.
pub(super) fn draw_on_map(game: &Game<'_>, batch: &mut HudBatch, map: &MapFrame, dim: &str) {
    let [x, y, cw, ch] = map.clip;
    let list = with_dim(game, dim, false, |w| w.waypoints.clone()).unwrap_or_default();
    for w in &list {
        let [px, py] = map.point(w.x, w.z);
        if px < x || py < y || px >= x + cw || py >= y + ch {
            continue;
        }
        let alpha = if w.hidden { 0x70 } else { 0xFF };
        diamond(game, batch, [px, py], w.rgb(), alpha);
        if !w.hidden {
            small_text(game, batch, &w.name, [px, py - 12.0], w.rgb(), 1.0);
        }
    }
}

/// The waypoint of `dim` nearest GUI point `p` on the claim screen, within 4 pixels.
fn waypoint_at(game: &Game<'_>, map: &MapFrame, dim: &str, p: [f32; 2]) -> Option<usize> {
    let list = with_dim(game, dim, false, |w| w.waypoints.clone())?;
    list.iter()
        .enumerate()
        .map(|(i, w)| {
            let [px, py] = map.point(w.x, w.z);
            (i, (px - p[0]).hypot(py - p[1]))
        })
        .filter(|(_, d)| *d <= 4.0)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// A right click that changed nothing on the claim screen at GUI point `p` over block `block`: the menu FTB Chunks'
/// map gives there (a waypoint's: hide or show, remove; elsewhere: a new waypoint). `free` is whether the chunk is
/// unclaimed (a click on a claim unclaims it instead). True when a menu opened.
pub(super) fn open_menu(game: &Game<'_>, map: &MapFrame, dim: &str, p: [f32; 2], block: [i32; 3], free: bool) -> bool {
    let items = match waypoint_at(game, map, dim, p) {
        Some(i) => {
            let hidden = with_dim(game, dim, false, |w| w.waypoints[i].hidden).unwrap_or(false);
            let toggle = if hidden { tr("ftbchunks.label.show", "Show on the maps", &[]) } else { tr("ftbchunks.label.hide", "Hide on the maps", &[]) };
            vec![(toggle, Action::Toggle(i)), (tr("ftbchunks.gui.delete", "Remove waypoint", &[]), Action::Remove(i))]
        }
        None if free => vec![(tr("ftbchunks.gui.add_waypoint", "New waypoint here", &[]), Action::Add)],
        None => return false,
    };
    MINI.with(|m| m.borrow_mut().menu = Some(Menu { at: p, block, dim: dim.to_owned(), items }));
    true
}

const ITEM_H: f32 = 12.0;

/// A map on the screen (the claim screen's or the large map's): `clip` is the area it fills (`[x, y, w, h]` GUI
/// pixels), block `origin`'s north-west corner is at GUI point `at`, and a block is `per_block` GUI pixels.
#[derive(Clone, Copy, Debug)]
pub(super) struct MapFrame {
    pub clip: [f32; 4],
    pub at: [f32; 2],
    pub per_block: f32,
    pub origin: [i32; 2],
}

impl MapFrame {
    /// The middle of block column `x, z`.
    pub fn point(&self, x: i32, z: i32) -> [f32; 2] {
        let k = self.per_block;
        [self.at[0] + (x - self.origin[0]) as f32 * k + k / 2.0, self.at[1] + (z - self.origin[1]) as f32 * k + k / 2.0]
    }
}

fn menu_width(game: &Game<'_>, menu: &Menu) -> f32 {
    menu.items.iter().map(|(l, _)| text_width(game, l)).fold(0.0, f32::max) + 8.0
}

/// A press while the menu is open: the item under it is done, and the menu closes. True when a menu was open.
pub(super) fn menu_click(game: &Game<'_>, p: [f32; 2], button: i32) -> bool {
    let Some(menu) = MINI.with(|m| m.borrow_mut().menu.take()) else { return false };
    let w = menu_width(game, &menu);
    let row = ((p[1] - menu.at[1]) / ITEM_H).floor();
    let inside = button == 0 && p[0] >= menu.at[0] && p[0] < menu.at[0] + w && row >= 0.0;
    let Some(&(_, action)) = menu.items.get(row as usize).filter(|_| inside) else { return true };
    with_dim(game, &menu.dim, true, |list| match action {
        Action::Add => {
            let [x, y, z] = menu.block;
            list.add(&format!("{x}, {z}"), [x, y, z], waypoints::WHITE);
        }
        Action::Toggle(i) => {
            if let Some(w) = list.waypoints.get_mut(i) {
                w.hidden = !w.hidden;
            }
        }
        Action::Remove(i) => {
            if i < list.waypoints.len() {
                list.waypoints.remove(i);
            }
        }
    });
    true
}

pub(super) fn draw_menu(game: &Game<'_>, batch: &mut HudBatch, mouse: [f32; 2]) {
    MINI.with(|m| {
        let m = m.borrow();
        let Some(menu) = &m.menu else { return };
        let w = menu_width(game, menu);
        let h = ITEM_H * menu.items.len() as f32;
        let [x, y] = menu.at;
        game.native_fill(batch, [x - 1.0, y - 1.0, w + 2.0, h + 2.0], 0xFF808080);
        game.native_fill(batch, [x, y, w, h], 0xF0101010);
        for (i, (label, _)) in menu.items.iter().enumerate() {
            let top = y + i as f32 * ITEM_H;
            if mouse[0] >= x && mouse[0] < x + w && mouse[1] >= top && mouse[1] < top + ITEM_H {
                game.native_fill(batch, [x, top, w, ITEM_H], 0x40FFFFFF);
            }
            game.native_text(batch, label, [x + 4.0, top + 2.0], 0xFFFFFF, true);
        }
    });
}

pub(super) fn close_menu() {
    MINI.with(|m| m.borrow_mut().menu = None);
}

// ---- The minimap ------------------------------------------------------------------------------------------

/// The claim screen took the shared mod image: upload again when the minimap shows next.
pub(super) fn image_taken() {
    MINI.with(|m| m.borrow_mut().uploaded = false);
}

/// `native_key`: FTB Chunks' `toggle_minimap` (unbound by default, as there) turns it on and off.
pub(super) fn key(game: &Game<'_>, key: &str, pressed: bool) {
    let bound = game.native_mod_key(MOD, "toggle_minimap", "key.keyboard.unknown");
    if pressed && key == bound && bound != "key.keyboard.unknown" {
        let on = !enabled();
        MINI.with(|m| m.borrow_mut().enabled = Some(on));
    }
}

/// The image of the 5x5 chunks round the player: the terrain tiles, the claims in their team's colour (alpha 100)
/// with a full-colour line where the next chunk is another's, as FTB Chunks' map shows them.
fn rescan(game: &mut Game<'_>, origin: (i32, i32), dim: &str) -> (Vec<u8>, usize) {
    let mut image = UNKNOWN.repeat(SIDE * SIDE);
    let mut tiles = 0;
    for v in 0..CHUNKS {
        for u in 0..CHUNKS {
            let Some(tile) = ftbchunks_largemap::tile(game, dim, [origin.0 + u, origin.1 + v]) else { continue };
            tiles += 1;
            for row in 0..16 {
                let o = ((v as usize * 16 + row) * SIDE + u as usize * 16) * 4;
                image[o..o + 64].copy_from_slice(&tile[row * 64..row * 64 + 64]);
            }
        }
    }
    let owners = ftbchunks_map::owners(dim, (origin.0 - 1, origin.1 - 1), CHUNKS + 2);
    let owner = |u: i32, v: i32| owners[((v + 1) * (CHUNKS + 2) + u + 1) as usize];
    let colors = ftbteams_world::with_client(|t| owners.iter().flatten().map(|team| (*team, t.color(*team).unwrap_or(0xFF_FFFF))).collect::<HashMap<_, _>>());
    for v in 0..CHUNKS {
        for u in 0..CHUNKS {
            let Some(team) = owner(u, v) else { continue };
            let [_, r, g, b] = colors.get(&team).copied().unwrap_or(0xFF_FFFF).to_be_bytes();
            let other = |du: i32, dv: i32| owner(u + du, v + dv) != Some(team);
            let (top, bottom, left, right) = (other(0, -1), other(0, 1), other(-1, 0), other(1, 0));
            for bz in 0..16 {
                for bx in 0..16 {
                    let o = ((v as usize * 16 + bz) * SIDE + u as usize * 16 + bx) * 4;
                    let edge = (top && bz == 0) || (bottom && bz == 15) || (left && bx == 0) || (right && bx == 15);
                    let a = if edge { 255 } else { 100 };
                    for (i, c) in [r, g, b].into_iter().enumerate() {
                        image[o + i] = ((u32::from(image[o + i]) * (255 - a) + u32::from(c) * a) / 255) as u8;
                    }
                }
            }
        }
    }
    (image, tiles)
}

/// The info lines under the map (`minimap.info_order` less `info_hidden`): the position, the biome, and the claim's
/// team where the player stands (the zone shows only in claims, FTB Chunks' default).
fn info_lines(game: &Game<'_>, block: [i32; 3], dim: &str) -> Vec<(String, u32)> {
    let mut lines = vec![(format!("{} {} {}", block[0], block[1], block[2]), 0xFFFFFF)];
    if let Some(biome) = game.native_biome(block) {
        let (ns, path) = biome.split_once(':').unwrap_or(("minecraft", biome.as_str()));
        let fallback = path.replace('_', " ");
        lines.push((Text::translate_with_fallback(&format!("biome.{ns}.{path}"), &fallback, &[]).plain(), 0xFFFFFF));
    }
    if let Some(team) = ftbchunks_map::owners(dim, (block[0] >> 4, block[2] >> 4), 1)[0] {
        let (name, rgb) = ftbteams_world::with_client(|t| {
            (t.teams.get(&team).and_then(|t| t.properties.get("ftbteams:display_name").cloned()), t.color(team).unwrap_or(0xFF_FFFF))
        });
        lines.extend(name.map(|n| (n, rgb)));
    }
    lines
}

/// Whether the corner minimap shows now: on, no map screen open, and a manager known.
fn shown(game: &Game<'_>) -> bool {
    enabled() && !ftbchunks_map::screen_open(game) && !ftbchunks_largemap::screen_open(game) && ftbchunks_map::manager().is_some()
}

/// `native_render_frame_pre`: the minimap's image round the player, scanned again and uploaded when due (upstream's
/// HUD hook `render_gui_post` reads the game only, so `hud` draws what this made).
pub(super) fn frame(game: &mut Game<'_>) {
    let Some(p) = game.native_player().filter(|_| shown(game)) else { return };
    let dim = p.dimension.to_string();
    let block = [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32];
    let origin = ((block[0] >> 4) - CHUNKS / 2, (block[2] >> 4) - CHUNKS / 2);
    let due = MINI.with(|m| {
        let m = m.borrow();
        m.origin != origin || m.dim != dim || m.scanned.is_none_or(|t| t.elapsed() >= RESCAN) || !m.uploaded
    });
    if due {
        let (image, tiles) = rescan(game, origin, &dim);
        let changed = MINI.with(|m| {
            let mut m = m.borrow_mut();
            let changed = m.image != image || !m.uploaded;
            (m.origin, m.dim, m.scanned, m.tiles) = (origin, dim.clone(), Some(Instant::now()), tiles);
            m.image = image;
            changed
        });
        if changed {
            let image = MINI.with(|m| m.borrow().image.clone());
            let ok = game.native_mod_image(SIDE as u32, SIDE as u32, &image);
            MINI.with(|m| m.borrow_mut().uploaded = ok);
        }
    }
}

/// `native_render_gui_post`: the corner minimap, from the image `frame` made.
pub(super) fn hud(game: &Game<'_>, batch: &mut HudBatch, corners: &mut Corners) {
    MINI.with(|m| m.borrow_mut().shown = false);
    let Some(p) = game.native_player().filter(|_| shown(game)) else { return };
    let dim = p.dimension.to_string();
    let block = [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32];
    let Some(origin) = MINI.with(|m| {
        let m = m.borrow();
        (m.uploaded && m.dim == dim).then_some(m.origin)
    }) else {
        return;
    };
    // FTB Chunks' size (`proportional`: a tenth of the GUI width, times `scale`) and place (`TOP_RIGHT`, 5 from the
    // edges); here it goes below what the corner holds (the status effects) where FTB Chunks moves them aside.
    let size = (batch.width / 10.0 * SCALE).floor().max(32.0);
    let lines = info_lines(game, block, &dim);
    let line_h = 10.0 * FONT_SCALE;
    let height = size + 3.0 + lines.len() as f32 * line_h;
    let (_, top) = corners.place(Corner::TopRight, size + 6.0, height + 3.0);
    let (x, y) = (batch.width - size - 5.0, top + 3.0);
    let r = size / 2.0;
    let (cx, cy) = (x + r, y + r);
    let k = BLOCKS / size;
    let ox = (p.x as f32 - BLOCKS / 2.0) - (origin.0 * 16) as f32;
    let oz = (p.z as f32 - BLOCKS / 2.0) - (origin.1 * 16) as f32;
    // The rim, then the map row by row inside the circle (`minimap.square` is off).
    let rows = size as i32;
    for row in -1..=rows {
        let dy = row as f32 + 0.5 - r;
        let half = ((r + 1.0).powi(2) - dy * dy).max(0.0).sqrt().round();
        game.native_fill(batch, [cx - half, y + row as f32, 2.0 * half, 1.0], 0xFF000000);
    }
    for row in 0..rows {
        let dy = row as f32 + 0.5 - r;
        let half = (r * r - dy * dy).max(0.0).sqrt().round();
        if half <= 0.0 {
            continue;
        }
        let x0 = r - half;
        game.native_sprite(batch, MOD_IMAGE_SPRITE, [ox + x0 * k, oz + row as f32 * k, 2.0 * half * k, k], [x + x0, y + row as f32, 2.0 * half, 1.0], None);
    }
    // The reticle and the compass letters (`minimap.reticle`, `minimap.compass`).
    game.native_fill(batch, [x + 2.0, cy, size - 4.0, 0.5], 0x30FFFFFF);
    game.native_fill(batch, [cx, y + 2.0, 0.5, size - 4.0], 0x30FFFFFF);
    for (letter, dx, dy) in [("N", 0.0, -1.0), ("E", 1.0, 0.0), ("S", 0.0, 1.0), ("W", -1.0, 0.0)] {
        small_text(game, batch, letter, [cx + dx * (r - 4.0), cy + dy * (r - 4.0) - 2.0], 0xFFFFFF, FONT_SCALE);
    }
    // Waypoints (`minimap.waypoints`): on the rim when they are off the map.
    let list = with_dim(game, &dim, false, |w| w.waypoints.clone()).unwrap_or_default();
    for w in list.iter().filter(|w| !w.hidden) {
        let (mut dx, mut dz) = ((w.x as f32 + 0.5 - p.x as f32) / k, (w.z as f32 + 0.5 - p.z as f32) / k);
        let d = dx.hypot(dz);
        if d > r - 3.0 {
            (dx, dz) = (dx / d * (r - 3.0), dz / d * (r - 3.0));
        }
        diamond(game, batch, [cx + dx, cy + dz], w.rgb(), 0xFF);
    }
    // The player's arrow, turned with the look (north up: 180 degrees from the yaw).
    let start = batch.vertices.len();
    for (grow, color) in [(1.0, 0xFF000000), (0.0, 0xFFFFFFFF)] {
        for (dy, half) in [(-3.0, 0.5), (-2.0, 0.5), (-1.0, 1.5), (0.0, 1.5), (1.0, 2.5), (2.0, 2.5)] {
            game.native_fill(batch, [cx - half - grow, cy + dy - grow, 2.0 * (half + grow), 1.0 + grow], color);
        }
    }
    let (sin, cos) = (p.yaw + 180.0).to_radians().sin_cos();
    for v in &mut batch.vertices[start..] {
        let (dx, dy) = (v.pos[0] - cx, v.pos[1] - cy);
        v.pos[0] = cx + dx * cos - dy * sin;
        v.pos[1] = cy + dx * sin + dy * cos;
    }
    // The info lines, centred under the map at `font_scale`.
    let mut ty = y + size + 3.0;
    for (line, rgb) in &lines {
        small_text(game, batch, line, [cx, ty], *rgb, FONT_SCALE);
        ty += line_h;
    }
    MINI.with(|m| {
        let mut m = m.borrow_mut();
        m.shown = true;
        m.rect = [x, y, size, size];
    });
}

/// The probe's view: whether the minimap is on and drawn, where, its tiles, and this dimension's waypoints.
pub(super) fn probe(game: &Game<'_>, dim: &str) -> serde_json::Value {
    let list = with_dim(game, dim, false, |w| w.waypoints.clone()).unwrap_or_default();
    let file = ftbchunks_map::manager().map(|m| folder(game, &m, dim).join(waypoints::FILE).display().to_string());
    MINI.with(|m| {
        let m = m.borrow();
        serde_json::json!({
            "enabled": m.enabled.unwrap_or_else(enabled_by_default),
            "default": enabled_by_default(),
            "shown": m.shown,
            "rect": m.rect,
            "tiles": m.tiles,
            "uploaded": m.uploaded,
            "menu": m.menu.as_ref().map(|menu| menu.items.iter().map(|(l, _)| l.clone()).collect::<Vec<_>>()),
            "waypoints": list,
            "file": file,
        })
    })
}
