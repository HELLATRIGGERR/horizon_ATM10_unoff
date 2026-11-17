//! The client's map cache and the large map (slice 5): FTB Chunks keeps every chunk a player has seen in one file a
//! 32x32-chunk region per dimension under `local/ftbchunks/data/<manager>/<dimension>/`, named from the region's
//! coordinates plus 60000 in hex (`0EA60-0EA60` for region 0, 0). FTB's own files are zips of block, height and biome
//! pictures; this port only has each chunk's finished 16x16 picture (`native_map_tile`), so its files keep those
//! pictures in a format of its own, under the same name with another extension, so neither reads the other's.
//! Also the large map's view (pan and zoom), the claim screen's force-load expiry steps and the force-load mark's
//! colour, all from what FTB Chunks does.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Chunks a side of a region.
pub const REGION_CHUNKS: i32 = 32;
/// A chunk's picture: 16x16 RGBA, rows north to south.
pub const TILE_BYTES: usize = 1024;
/// Our file's extension (FTB Chunks' own region files are `.zip`).
pub const EXTENSION: &str = "hzmap";
const MAGIC: &[u8; 4] = b"HZMR";
const VERSION: u8 = 1;

/// The region holding chunk `x, z`.
pub fn region_of(x: i32, z: i32) -> (i32, i32) {
    (x >> 5, z >> 5)
}

/// The region's file name: FTB Chunks' `%05X-%05X` of the coordinates plus 60000, with our extension.
pub fn region_file(rx: i32, rz: i32) -> String {
    format!("{:05X}-{:05X}.{EXTENSION}", rx + 60000, rz + 60000)
}

/// The chunks of one region the client has seen: each one's picture and when it was taken (ms since the epoch).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Region {
    pub tiles: BTreeMap<(u8, u8), (i64, Vec<u8>)>,
}

impl Region {
    /// Keeps chunk `x, z`'s picture (world chunk coordinates); true when it changed.
    pub fn put(&mut self, x: i32, z: i32, modified: i64, rgba: &[u8]) -> bool {
        if rgba.len() != TILE_BYTES {
            return false;
        }
        let key = ((x & 31) as u8, (z & 31) as u8);
        if self.tiles.get(&key).is_some_and(|(_, old)| old == rgba) {
            return false;
        }
        self.tiles.insert(key, (modified, rgba.to_vec()));
        true
    }

    pub fn get(&self, x: i32, z: i32) -> Option<&[u8]> {
        self.tiles.get(&((x & 31) as u8, (z & 31) as u8)).map(|(_, t)| t.as_slice())
    }

    /// The file: `HZMR`, a version byte, the count (u16), then each chunk's x, z (bytes), time (i64) and picture.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(7 + self.tiles.len() * (10 + TILE_BYTES));
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.extend_from_slice(&(self.tiles.len() as u16).to_le_bytes());
        for (&(x, z), (time, rgba)) in &self.tiles {
            out.extend_from_slice(&[x, z]);
            out.extend_from_slice(&time.to_le_bytes());
            out.extend_from_slice(rgba);
        }
        out
    }

    /// None for a file that is not ours or is cut short.
    pub fn decode(bytes: &[u8]) -> Option<Region> {
        if bytes.len() < 7 || &bytes[0..4] != MAGIC || bytes[4] != VERSION {
            return None;
        }
        let count = u16::from_le_bytes([bytes[5], bytes[6]]) as usize;
        let mut region = Region::default();
        let mut at = 7;
        for _ in 0..count {
            let entry = bytes.get(at..at + 10 + TILE_BYTES)?;
            let (x, z) = (entry[0], entry[1]);
            if x >= 32 || z >= 32 {
                return None;
            }
            let time = i64::from_le_bytes(entry[2..10].try_into().ok()?);
            region.tiles.insert((x, z), (time, entry[10..].to_vec()));
            at += 10 + TILE_BYTES;
        }
        Some(region)
    }
}

/// The large map's view: the block at the middle of the screen and the GUI pixels a block takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapView {
    pub center: [f64; 2],
    pub zoom: f64,
}

/// FTB Chunks' zoom range: a region's 512 blocks take 8 to 1024 pixels (here GUI pixels), doubling a wheel notch.
pub const MIN_ZOOM: f64 = 8.0 / 512.0;
pub const MAX_ZOOM: f64 = 1024.0 / 512.0;

impl Default for MapView {
    fn default() -> Self {
        MapView::new([0.0, 0.0])
    }
}

impl MapView {
    pub fn new(center: [f64; 2]) -> Self {
        MapView { center, zoom: 1.0 }
    }
    /// The block (fractional) under GUI point `p` of a map filling `rect` (`[x, y, w, h]`).
    pub fn block_at(&self, rect: [f32; 4], p: [f32; 2]) -> [f64; 2] {
        let mid = [f64::from(rect[0] + rect[2] / 2.0), f64::from(rect[1] + rect[3] / 2.0)];
        [self.center[0] + (f64::from(p[0]) - mid[0]) / self.zoom, self.center[1] + (f64::from(p[1]) - mid[1]) / self.zoom]
    }
    /// Where block point `b` is in the GUI.
    pub fn point_of(&self, rect: [f32; 4], b: [f64; 2]) -> [f32; 2] {
        let mid = [f64::from(rect[0] + rect[2] / 2.0), f64::from(rect[1] + rect[3] / 2.0)];
        [(mid[0] + (b[0] - self.center[0]) * self.zoom) as f32, (mid[1] + (b[1] - self.center[1]) * self.zoom) as f32]
    }
    /// A drag by `d` GUI pixels moves the map with the pointer.
    pub fn pan(&mut self, d: [f32; 2]) {
        self.center[0] -= f64::from(d[0]) / self.zoom;
        self.center[1] -= f64::from(d[1]) / self.zoom;
    }
    /// The wheel: `notches` up zooms in (doubling each), keeping the block under `anchor` where it is.
    pub fn zoom_at(&mut self, rect: [f32; 4], anchor: [f32; 2], notches: i32) {
        let before = self.block_at(rect, anchor);
        self.zoom = (self.zoom * 2f64.powi(notches)).clamp(MIN_ZOOM, MAX_ZOOM);
        let after = self.block_at(rect, anchor);
        self.center[0] += before[0] - after[0];
        self.center[1] += before[1] - after[1];
    }
}

/// The claim screen's wheel over a force-loaded chunk: the new time to expiry in seconds from `secs` (0: none).
/// Up adds an hour below a day, a day below a week, else a week; down takes the same steps off, never below 0, a day
/// or a week once past them (FTB Chunks' steps).
pub fn expiry_step(secs: i64, up: bool) -> i64 {
    const HOUR: i64 = 3600;
    const DAY: i64 = 86_400;
    const WEEK: i64 = 604_800;
    let secs = secs.max(0);
    match (up, secs) {
        (true, s) if s < DAY => s + HOUR,
        (true, s) if s < WEEK => s + DAY,
        (true, s) => s + WEEK,
        (false, s) if s <= DAY => (s - HOUR).max(0),
        (false, s) if s <= WEEK => (s - DAY).max(DAY),
        (false, s) => (s - WEEK).max(WEEK),
    }
}

/// The force-load mark's colour on a claim of colour `rgb`: the opposite hue at the same saturation and brightness,
/// so it stands out on every team colour (FTB Chunks tints its icon that way).
pub fn force_load_color(rgb: u32) -> u32 {
    let [_, r, g, b] = rgb.to_be_bytes().map(|c| f32::from(c) / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let (v, delta) = (max, max - min);
    let s = if max > 0.0 { delta / max } else { 0.0 };
    let mut h = if delta == 0.0 {
        0.0
    } else if max == r {
        ((g - b) / delta).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / delta + 2.0) / 6.0
    } else {
        ((r - g) / delta + 4.0) / 6.0
    };
    h = (h + 0.5) % 1.0;
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let (r, g, b) = match i as i32 % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    let c = |x: f32| (x * 255.0 + 0.5) as u32;
    c(r) << 16 | c(g) << 8 | c(b)
}

/// `update_force_load_expiry` (C2S): chunk `x, z` of `dim` expires `relative_ms` from now (0: never).
pub const FORCE_LOAD_EXPIRY: &str = "ftbchunks:update_force_load_expiry";
/// `teleport_from_map` (C2S): an operator's teleport to a block clicked on the large map.
pub const TELEPORT_FROM_MAP: &str = "ftbchunks:teleport_from_map";
/// `open_claim_gui` (S2C): `/ftbchunks admin open_claim_gui_as <team>` opens the claim screen as that team.
pub const OPEN_CLAIM_GUI: &str = "ftbchunks:open_claim_gui";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ForceLoadExpiry {
    pub dim: String,
    pub x: i32,
    pub z: i32,
    pub relative_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TeleportFromMap {
    pub pos: [i32; 3],
    /// The client doesn't know the height there: the server takes the surface.
    pub unknown_y: bool,
    pub dim: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenClaimGui {
    pub team: String,
}

impl crate::Chunks {
    /// `UpdateForceLoadExpiryPacket.handle`: `player` (in that dimension) sets when force-loaded chunk `pos` of a
    /// team they are a member of stops being force-loaded, `relative_ms` from `now` (0: never). True when it changed.
    pub fn set_force_load_expiry(
        &mut self,
        teams: &dyn crate::Teams,
        player: crate::Uuid,
        pos: &crate::ChunkPos,
        relative_ms: i64,
        now: i64,
    ) -> bool {
        let Some(claim) = self.claims.get_mut(pos) else { return false };
        if claim.force_loaded <= 0 || !teams.team(claim.team).is_some_and(|t| t.is_member(player)) {
            return false;
        }
        let expiry = if relative_ms <= 0 { 0 } else { now + relative_ms };
        if claim.expiry == expiry {
            return false;
        }
        claim.expiry = expiry;
        let team = claim.team;
        self.dirty.insert(team);
        true
    }
}

/// FTB Chunks' fallback height for a teleport whose column has no data.
pub const TELEPORT_FALLBACK_Y: i32 = 70;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_named_as_ftb_names_regions() {
        assert_eq!(region_of(0, 0), (0, 0));
        assert_eq!(region_of(-1, 33), (-1, 1));
        assert_eq!(region_file(0, 0), "0EA60-0EA60.hzmap");
        assert_eq!(region_file(-1, 2), "0EA5F-0EA62.hzmap");
    }

    #[test]
    fn a_region_round_trips_and_rejects_others() {
        let mut r = Region::default();
        let tile: Vec<u8> = (0..TILE_BYTES).map(|i| i as u8).collect();
        assert!(r.put(-1, 40, 7, &tile));
        assert!(!r.put(-1, 40, 9, &tile), "the same picture changes nothing");
        assert!(r.put(3, 3, 8, &[1; TILE_BYTES]));
        assert!(!r.put(3, 3, 8, &[1; 5]), "a picture of the wrong size is refused");
        let back = Region::decode(&r.encode()).expect("decodes");
        assert_eq!(back, r);
        assert_eq!(back.get(-1, 40), Some(tile.as_slice()));
        assert_eq!(back.get(31, 8), Some(tile.as_slice()), "local coordinates wrap");
        assert!(Region::decode(b"PK\x03\x04 a zip").is_none());
        let mut cut = r.encode();
        cut.truncate(cut.len() - 1);
        assert!(Region::decode(&cut).is_none());
    }

    #[test]
    fn the_view_pans_and_zooms_round_the_pointer() {
        let rect = [0.0, 0.0, 400.0, 200.0];
        let mut v = MapView::new([100.0, -50.0]);
        assert_eq!(v.block_at(rect, [200.0, 100.0]), [100.0, -50.0]);
        v.pan([20.0, -10.0]);
        assert_eq!(v.center, [80.0, -40.0]);
        let anchor = [300.0, 50.0];
        let under = v.block_at(rect, anchor);
        v.zoom_at(rect, anchor, 1);
        assert_eq!(v.zoom, 2.0);
        assert_eq!(v.block_at(rect, anchor), under);
        assert_eq!(v.point_of(rect, under), anchor);
        v.zoom_at(rect, anchor, 20);
        assert_eq!(v.zoom, MAX_ZOOM);
        v.zoom_at(rect, anchor, -40);
        assert_eq!(v.zoom, MIN_ZOOM);
    }

    #[test]
    fn expiry_steps_by_hour_day_and_week() {
        assert_eq!(expiry_step(0, true), 3600);
        assert_eq!(expiry_step(86_399, true), 89_999);
        assert_eq!(expiry_step(86_400, true), 172_800);
        assert_eq!(expiry_step(604_800, true), 1_209_600);
        assert_eq!(expiry_step(1800, false), 0);
        assert_eq!(expiry_step(86_400, false), 82_800);
        assert_eq!(expiry_step(90_000, false), 86_400);
        assert_eq!(expiry_step(700_000, false), 604_800);
    }

    #[test]
    fn the_force_load_mark_takes_the_opposite_hue() {
        assert_eq!(force_load_color(0xFF0000), 0x00FFFF);
        assert_eq!(force_load_color(0x0000FF), 0xFFFF00);
        assert_eq!(force_load_color(0x808080), 0x808080, "grey has no hue to turn");
        assert_eq!(force_load_color(0x00AA00), 0xAA00AA);
    }
}
