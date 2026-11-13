// FTB Chunks' client in the game (support module `crate::game::ftbchunks_map` of the `ftbchunks` native bundle):
// the claims the server sends (`send_many_chunks`, `send_general_data`, `chunk_change_response`), and the claim
// screen, FTB Chunks' `ChunkScreen`: the map of the 31x31 chunks round the player over the terrain (the minimap
// extra's tiles through `native_map_tile`, shown with `native_mod_image`), claims in their team's colour, force-loaded
// ones crossed; a left drag claims, a right drag unclaims, with Shift they force-load and release, and the chunks the
// pointer went over go in one `request_chunk_change` when the button is let go. M opens it (FTB Chunks' map key).
// FTB Chunks is All Rights Reserved: this is written from its behaviour; the labels are its translation keys with
// our own fallback wording.

use super::{Game, ftbchunks_minimap, ftbteams_world};
use ftbchunks_core::net::{self, ChangeRequest, ChangeResponse, ChunkInfo, GeneralData, ManyChunks, Op};
use horizon_client::{hud::HudBatch, mod_api::NativeScreenView, text::Text};
use std::{
    cell::RefCell,
    collections::{BTreeSet, HashMap},
    time::{Duration, Instant},
};

const MOD: &str = "ftbchunks";
/// `FTBChunks.TILES`: the claim screen shows this many chunks a side, the player's in the middle.
pub(super) const TILES: i32 = 31;
const HALF: i32 = TILES / 2;
const IMAGE: usize = TILES as usize * 16;
/// Chunks scanned a frame while the map fills, then while it is refreshed.
const FILL_PER_FRAME: usize = 64;
const REFRESH_PER_FRAME: usize = 8;
const UNKNOWN: [u8; 4] = [16, 16, 16, 255];

struct Drag {
    left: bool,
    shift: bool,
    chunks: BTreeSet<(i32, i32)>,
    start: [f32; 2],
    last: [f32; 2],
}

/// Where the map is in the screen: its top-left corner and side in GUI pixels.
#[derive(Clone, Copy, Default)]
struct Layout {
    x: f32,
    y: f32,
    side: f32,
}

impl Layout {
    fn of(rect: [f32; 4]) -> Self {
        let [x, y, w, h] = rect;
        let side = ((w.min(h) - 40.0).max(62.0) / TILES as f32).floor() * TILES as f32;
        Layout { x: x + ((w - side) / 2.0).floor(), y: y + ((h - side) / 2.0).floor(), side }
    }
    fn cell(&self) -> f32 {
        self.side / TILES as f32
    }
    /// The chunk offset (0..31 each way) under GUI point `p`.
    fn at(&self, p: [f32; 2]) -> Option<(i32, i32)> {
        let (u, v) = ((p[0] - self.x) / self.cell(), (p[1] - self.y) / self.cell());
        (u >= 0.0 && v >= 0.0 && u < TILES as f32 && v < TILES as f32).then_some((u as i32, v as i32))
    }
}

#[derive(Default)]
struct Client {
    manager: Option<String>,
    general: GeneralData,
    /// Claimed chunks by dimension and chunk: the team and what the server said of it.
    claims: HashMap<(String, i32, i32), (u128, ChunkInfo)>,
    response: Option<(ChangeResponse, Instant)>,
    /// The screen: whether it was open last frame, the chunk at its middle and the dimension.
    open: bool,
    center: (i32, i32),
    dim: String,
    image: Vec<u8>,
    /// Next chunk (index in the 31x31) to scan, and whether one pass is done.
    next: usize,
    filled: bool,
    tiles: usize,
    dirty: bool,
    uploaded: Option<Instant>,
    layout: Layout,
    drag: Option<Drag>,
    outgoing: Vec<ChangeRequest>,
}

thread_local! {
    static CLIENT: RefCell<Client> = RefCell::new(Client::default());
}

fn dev() -> bool {
    std::env::var("FTBTEAMS_DEV").is_ok_and(|v| v == "1")
}

fn tr(key: &str, fallback: &str, args: &[String]) -> String {
    let args: Vec<Text> = args.iter().map(|a| Text::literal(a.clone())).collect();
    Text::translate_with_fallback(key, fallback, &args).plain()
}

// ---- Messages ---------------------------------------------------------------------------------------------

/// `native_net_from_server`: what FTB Chunks' server sends this window's client.
pub(super) fn from_server(channel: &str, data: &[u8]) {
    CLIENT.with(|c| {
        let mut c = c.borrow_mut();
        let ok = match channel {
            net::LOGIN_DATA => serde_json::from_slice::<serde_json::Value>(data).map(|v| {
                c.manager = v["manager"].as_str().map(str::to_owned);
                c.claims.clear();
            }),
            net::GENERAL_DATA => serde_json::from_slice::<GeneralData>(data).map(|g| c.general = g),
            net::MANY_CHUNKS => serde_json::from_slice::<ManyChunks>(data).map(|m| {
                let team = net::parse_uuid_text(&m.team).unwrap_or_default();
                for info in m.chunks {
                    let key = (m.dim.clone(), info.x, info.z);
                    if info.claimed {
                        c.claims.insert(key, (team, info));
                    } else if c.claims.get(&key).is_some_and(|(t, _)| *t == team) {
                        c.claims.remove(&key);
                    }
                }
            }),
            net::CHANGE_RESPONSE => serde_json::from_slice::<ChangeResponse>(data).map(|r| c.response = Some((r, Instant::now()))),
            _ => return,
        };
        if let Err(e) = ok {
            log::warn!("[ftbchunks] a bad {channel} message: {e}");
        }
    });
}

// ---- The claim screen -------------------------------------------------------------------------------------

/// The FTB Teams manager id `login_data` gave (the client's map folder), once logged in.
pub(super) fn manager() -> Option<String> {
    CLIENT.with(|c| c.borrow().manager.clone())
}

/// The teams claiming the `n` x `n` chunks from `origin` in `dim`, row by row.
pub(super) fn owners(dim: &str, origin: (i32, i32), n: i32) -> Vec<Option<u128>> {
    CLIENT.with(|c| {
        let c = c.borrow();
        let key = |u: i32, v: i32| (dim.to_owned(), origin.0 + u, origin.1 + v);
        (0..n * n).map(|i| c.claims.get(&key(i % n, i / n)).map(|(team, _)| *team)).collect()
    })
}

pub(super) fn screen_open(game: &Game<'_>) -> bool {
    game.native_mod_screen() == Some(MOD)
}

/// `native_key`: M (FTB Chunks' `key.ftbchunks.map`) opens the claim map while playing, and closes it.
pub(super) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if !pressed || key != game.native_mod_key(MOD, "map", "key.keyboard.m") {
        return;
    }
    if screen_open(game) {
        game.native_mod_screen_close(MOD);
    } else if game.native_playing() {
        open(game);
    }
}

fn open(game: &mut Game<'_>) -> bool {
    let Some(p) = game.native_player() else { return false };
    if !game.native_mod_screen_open(MOD) {
        return false;
    }
    CLIENT.with(|c| {
        let mut c = c.borrow_mut();
        c.open = true;
        c.center = ((p.x.floor() as i32) >> 4, (p.z.floor() as i32) >> 4);
        c.dim = p.dimension.to_string();
        c.image = UNKNOWN.repeat(IMAGE * IMAGE);
        c.next = 0;
        c.filled = false;
        c.tiles = 0;
        c.dirty = true;
        c.uploaded = None;
        c.drag = None;
        c.response = None;
    });
    true
}

/// `native_frame`: the map's tiles a few a frame and their upload, and the requests the drags made.
pub(super) fn frame(game: &mut Game<'_>) {
    let outgoing = CLIENT.with(|c| std::mem::take(&mut c.borrow_mut().outgoing));
    for request in outgoing {
        let data = serde_json::to_vec(&request).unwrap_or_default();
        if let Err(e) = game.native_net_send_to_server(net::CHUNK_CHANGE, &data) {
            log::debug!("[ftbchunks] {}: {e}", net::CHUNK_CHANGE);
        }
    }
    let shown = screen_open(game);
    let (center, todo) = CLIENT.with(|c| {
        let mut c = c.borrow_mut();
        if !shown {
            if c.open {
                ftbchunks_minimap::close_menu();
            }
            c.open = false;
            c.drag = None;
            return (c.center, 0);
        }
        (c.center, if c.filled { REFRESH_PER_FRAME } else { FILL_PER_FRAME })
    });
    if todo == 0 {
        return;
    }
    let count = (TILES * TILES) as usize;
    let mut scanned = Vec::with_capacity(todo);
    let start = CLIENT.with(|c| c.borrow().next);
    for k in 0..todo {
        let i = (start + k) % count;
        let (u, v) = ((i % TILES as usize) as i32, (i / TILES as usize) as i32);
        let tile = game.native_map_tile([center.0 - HALF + u, center.1 - HALF + v]);
        scanned.push((u, v, tile));
    }
    let upload = CLIENT.with(|c| {
        let mut c = c.borrow_mut();
        c.next = (start + todo) % count;
        if start + todo >= count {
            c.filled = true;
        }
        for (u, v, tile) in scanned {
            let Some(tile) = tile else { continue };
            c.tiles += 1;
            for row in 0..16 {
                let o = ((v as usize * 16 + row) * IMAGE + u as usize * 16) * 4;
                let line = &tile[row * 64..row * 64 + 64];
                if c.image[o..o + 64] != *line {
                    c.image[o..o + 64].copy_from_slice(line);
                    c.dirty = true;
                }
            }
        }
        let due = c.uploaded.is_none_or(|t| t.elapsed() >= Duration::from_millis(if c.filled { 500 } else { 50 }));
        (c.dirty && due).then(|| {
            c.dirty = false;
            c.image.clone()
        })
    });
    if let Some(image) = upload {
        if game.native_mod_image(IMAGE as u32, IMAGE as u32, &image) {
            ftbchunks_minimap::image_taken();
            CLIENT.with(|c| c.borrow_mut().uploaded = Some(Instant::now()));
        }
    }
}

/// `native_screen_mouse_button_pressed_pre`: a drag starts on the map.
pub(super) fn pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32, modifiers: u32, canceled: &mut bool) {
    if !screen_open(game) || !(button == 0 || button == 1) {
        return;
    }
    *canceled = true;
    if ftbchunks_minimap::menu_click(game, screen.mouse, button) {
        return;
    }
    CLIENT.with(|c| {
        let mut c = c.borrow_mut();
        let layout = Layout::of(screen.rect);
        c.layout = layout;
        let mut chunks = BTreeSet::new();
        chunks.extend(layout.at(screen.mouse));
        c.drag = Some(Drag { left: button == 0, shift: modifiers & 1 != 0, chunks, start: screen.mouse, last: screen.mouse });
    });
}

/// `native_screen_mouse_dragged_pre`: every chunk the pointer passes over joins the selection (FTB's freehand mode).
pub(super) fn dragged(game: &Game<'_>, screen: &NativeScreenView<'_>, canceled: &mut bool) {
    if !screen_open(game) {
        return;
    }
    *canceled = true;
    CLIENT.with(|c| {
        let mut c = c.borrow_mut();
        let layout = Layout::of(screen.rect);
        let Some(drag) = c.drag.as_mut() else { return };
        let (from, to) = (drag.last, screen.mouse);
        let steps = ((to[0] - from[0]).abs().max((to[1] - from[1]).abs()) / (layout.cell() / 3.0)).ceil().max(1.0) as i32;
        for s in 1..=steps {
            let t = s as f32 / steps as f32;
            drag.chunks.extend(layout.at([from[0] + (to[0] - from[0]) * t, from[1] + (to[1] - from[1]) * t]));
        }
        drag.last = to;
    });
}

/// `native_screen_mouse_button_released_pre`: the selection goes to the server in one request.
pub(super) fn released(game: &Game<'_>, button: i32, canceled: &mut bool) {
    if !screen_open(game) || !(button == 0 || button == 1) {
        return;
    }
    *canceled = true;
    let Some(drag) = CLIENT.with(|c| c.borrow_mut().drag.take()) else { return };
    if drag.chunks.is_empty() {
        return;
    }
    let (layout, (cx, cz), dim) = CLIENT.with(|c| {
        let c = c.borrow();
        (c.layout, c.center, c.dim.clone())
    });
    // A right click that stays on one chunk: FTB Chunks' map menu where it would change nothing (no claim there), or
    // over a waypoint.
    if !drag.left && drag.chunks.len() == 1 && (drag.last[0] - drag.start[0]).hypot(drag.last[1] - drag.start[1]) < 3.0 {
        let &(u, v) = drag.chunks.first().expect("one chunk");
        let free = owners(&dim, (cx - HALF + u, cz - HALF + v), 1)[0].is_none();
        let per_block = layout.cell() / 16.0;
        let bx = (cx - HALF) * 16 + ((drag.last[0] - layout.x) / per_block).floor() as i32;
        let bz = (cz - HALF) * 16 + ((drag.last[1] - layout.y) / per_block).floor() as i32;
        let y = game.native_player().map_or(64, |p| p.y.floor() as i32);
        let origin = [(cx - HALF) * 16, (cz - HALF) * 16];
        if ftbchunks_minimap::open_menu(game, [layout.x, layout.y, layout.side], origin, &dim, drag.last, [bx, y, bz], free) {
            return;
        }
    }
    let chunks = drag.chunks.iter().map(|&(u, v)| [cx - HALF + u, cz - HALF + v]).collect();
    CLIENT.with(|c| c.borrow_mut().outgoing.push(ChangeRequest { op: Op::create(drag.left, drag.shift), chunks, admin: false, team: None }));
}

fn argb(alpha: u32, rgb: u32) -> u32 {
    alpha << 24 | (rgb & 0xFF_FFFF)
}

/// `native_screen_render_post`: the claim screen.
pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if !screen_open(game) {
        return;
    }
    let layout = Layout::of(screen.rect);
    let cell = layout.cell();
    game.native_fill(batch, screen.rect, 0xC0000000);
    game.native_fill(batch, [layout.x - 1.0, layout.y - 1.0, layout.side + 2.0, layout.side + 2.0], 0xFF000000);
    let size = IMAGE as f32;
    CLIENT.with(|c| {
        let mut c = c.borrow_mut();
        c.layout = layout;
        if c.uploaded.is_some() {
            game.native_sprite(batch, horizon_client::hud::MOD_IMAGE_SPRITE, [0.0, 0.0, size, size], [layout.x, layout.y, layout.side, layout.side]);
        }
        // The chunk grid.
        for i in 1..TILES {
            let o = i as f32 * cell;
            game.native_fill(batch, [layout.x + o, layout.y, 1.0, layout.side], 0x30000000);
            game.native_fill(batch, [layout.x, layout.y + o, layout.side, 1.0], 0x30000000);
        }
        let (cx, cz) = c.center;
        let owner = |u: i32, v: i32| c.claims.get(&(c.dim.clone(), cx - HALF + u, cz - HALF + v));
        let colors = ftbteams_world::with_client(|t| {
            let mut m: HashMap<u128, u32> = HashMap::new();
            for (team, _) in c.claims.values() {
                m.entry(*team).or_insert_with(|| t.color(*team).unwrap_or(0xFF_FFFF));
            }
            m
        });
        for v in 0..TILES {
            for u in 0..TILES {
                let Some((team, info)) = owner(u, v) else { continue };
                let rgb = colors.get(team).copied().unwrap_or(0xFF_FFFF);
                let (x, y) = (layout.x + u as f32 * cell, layout.y + v as f32 * cell);
                game.native_fill(batch, [x, y, cell, cell], argb(0x64, rgb));
                // Borders where the next chunk is not the same team's.
                let other = |du: i32, dv: i32| owner(u + du, v + dv).is_none_or(|(t, _)| t != team);
                let line = argb(0xFF, rgb);
                if other(0, -1) {
                    game.native_fill(batch, [x, y, cell, 1.0], line);
                }
                if other(0, 1) {
                    game.native_fill(batch, [x, y + cell - 1.0, cell, 1.0], line);
                }
                if other(-1, 0) {
                    game.native_fill(batch, [x, y, 1.0, cell], line);
                }
                if other(1, 0) {
                    game.native_fill(batch, [x + cell - 1.0, y, 1.0, cell], line);
                }
                if info.force_loaded.is_some() {
                    // A cross for a force-loaded chunk.
                    let n = (cell / 2.0).floor().max(2.0) as i32;
                    let step = cell / n as f32;
                    for k in 0..n {
                        let d = k as f32 * step + step / 2.0 - 1.0;
                        game.native_fill(batch, [x + d, y + d, 2.0, 2.0], 0xE0FF4040);
                        game.native_fill(batch, [x + cell - d - 2.0, y + d, 2.0, 2.0], 0xE0FF4040);
                    }
                }
            }
        }
        if let Some(drag) = &c.drag {
            let tint = if drag.left { 0x70FFFFFF } else { 0x70FF0000 };
            for &(u, v) in &drag.chunks {
                game.native_fill(batch, [layout.x + u as f32 * cell, layout.y + v as f32 * cell, cell, cell], tint);
            }
        }
        // The player.
        if let Some(p) = game.native_player() {
            let px = layout.x + ((p.x / 16.0) as f32 - (cx - HALF) as f32) * cell;
            let pz = layout.y + ((p.z / 16.0) as f32 - (cz - HALF) as f32) * cell;
            game.native_fill(batch, [px - 2.0, pz - 2.0, 4.0, 4.0], 0xFF000000);
            game.native_fill(batch, [px - 1.0, pz - 1.0, 2.0, 2.0], 0xFFFFFFFF);
        }
        // The counters, FTB Chunks' claimed and force-loaded against the limits.
        let g = c.general;
        let lines = [
            format!("{}: {} / {}", tr("ftbchunks.gui.claimed", "Chunks claimed", &[]), g.claimed, g.max_claims),
            format!("{}: {} / {}", tr("ftbchunks.gui.force_loaded", "Chunks kept loaded", &[]), g.loaded, g.max_force_loads),
        ];
        let x = screen.rect[0] + 4.0;
        let mut y = screen.rect[1] + screen.rect[3] - 22.0;
        for line in &lines {
            game.native_text(batch, line, [x, y], 0xFFFFFF, true);
            y += 10.0;
        }
        let title = tr("ftbchunks.gui.claimed_chunks", "Claim map", &[]);
        let w = game.native_text_width(&title);
        game.native_text(batch, &title, [screen.rect[0] + (screen.rect[2] - w) / 2.0, layout.y - 12.0], 0xFFFFFF, true);
        if let Some((r, at)) = c.response.as_ref().filter(|(_, at)| at.elapsed() < Duration::from_secs(5)) {
            let _ = at;
            let mut line = tr("ftbchunks.claim_result", "Chunks changed: %s of %s", &[r.changed.to_string(), r.total.to_string()]);
            for (id, n) in &r.problems {
                line += &format!(", {} x{n}", tr(&format!("ftbchunks.claim_result.{id}"), &id.replace('_', " "), &[]));
            }
            game.native_text(batch, &line, [x, screen.rect[1] + 4.0], 0xFFFF55, true);
        }
        // The chunk under the pointer: its owner.
        if let Some((u, v)) = layout.at(screen.mouse)
            && let Some((team, _)) = owner(u, v)
        {
            let name = ftbteams_world::with_client(|t| {
                t.teams.get(team).and_then(|t| t.properties.get("ftbteams:display_name").cloned())
            });
            if let Some(name) = name {
                game.native_text(batch, &name, [screen.mouse[0] + 8.0, screen.mouse[1] - 10.0], 0xFFFFFF, true);
            }
        }
    });
    let (cx, cz, dim) = CLIENT.with(|c| {
        let c = c.borrow();
        (c.center.0, c.center.1, c.dim.clone())
    });
    let origin = [(cx - HALF) * 16, (cz - HALF) * 16];
    ftbchunks_minimap::draw_on_claim_map(game, batch, [layout.x, layout.y, layout.side], origin, &dim);
    ftbchunks_minimap::draw_menu(game, batch, screen.mouse);
}

// ---- The tests' probe -------------------------------------------------------------------------------------

/// `native_commands`: the client's probe.
pub(super) fn client_commands(commands: &mut Vec<&'static str>) {
    if dev() {
        commands.push("ftbchunksclient");
        commands.push("ftbchunksmap");
    }
}

/// `native_command`: `/ftbchunksclient` puts `FTBCC {json}` in chat (what this client knows, and where the claim
/// screen's map is); `/ftbchunksmap` opens the claim screen as M does.
pub(super) fn client_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    if *ran || !dev() {
        return;
    }
    match command.split_whitespace().next() {
        Some("ftbchunksmap") => {
            *ran = true;
            let opened = open(game);
            game.native_chat(&format!("FTBCC {}", serde_json::json!({ "opened": opened })));
        }
        Some("ftbchunksclient") => {
            *ran = true;
            let open = screen_open(game);
            let dim = game.native_player().map(|p| p.dimension.to_string()).unwrap_or_default();
            let minimap = ftbchunks_minimap::probe(game, &dim);
            let json = CLIENT.with(|c| {
                let c = c.borrow();
                let mut by_team: HashMap<String, (u32, u32)> = HashMap::new();
                for ((dim, _, _), (team, info)) in &c.claims {
                    if *dim == c.dim || c.dim.is_empty() {
                        let e = by_team.entry(net::uuid_text(*team)).or_default();
                        e.0 += 1;
                        e.1 += u32::from(info.force_loaded.is_some());
                    }
                }
                let l = c.layout;
                serde_json::json!({
                    "manager": c.manager,
                    "general": c.general,
                    "claims": by_team,
                    "response": c.response.as_ref().map(|r| &r.0),
                    "screen": open,
                    "center": [c.center.0, c.center.1],
                    "map": [l.x, l.y, l.side],
                    "cell": l.cell(),
                    "tiles": c.tiles,
                    "filled": c.filled,
                    "uploaded": c.uploaded.is_some(),
                    "minimap": minimap,
                })
            });
            game.native_chat(&format!("FTBCC {json}"));
        }
        _ => {}
    }
}
