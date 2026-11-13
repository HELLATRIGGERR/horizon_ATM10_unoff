//! FTB Chunks' waypoints, client side: one `waypoints.json` per dimension in the client's map folder
//! (`local/ftbchunks/data/<manager id>/<dimension, ':' as '_'>/`), with the keys FTB Chunks writes; the death
//! waypoint made at each death; `/ftbchunks waypoint add` (the server parses it and sends `add_waypoint`); and the
//! messages `add_waypoint` and `player_death`. Written from FTB Chunks' behaviour; no part of it is copied.

use serde::{Deserialize, Serialize};

pub const ADD_WAYPOINT: &str = "ftbchunks:add_waypoint";
pub const PLAYER_DEATH: &str = "ftbchunks:player_death";
pub const FILE: &str = "waypoints.json";
/// `WaypointType` ids.
pub const DEFAULT: &str = "default";
pub const DEATH: &str = "death";
/// A new waypoint's colour when none is given, and the death waypoint's.
pub const WHITE: u32 = 0xFF_FFFF;
pub const DEATH_COLOR: u32 = 0xFF_0000;

/// One waypoint as the file holds it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Waypoint {
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    #[serde(default = "white")]
    pub color: String,
    #[serde(rename = "type", default = "default_type")]
    pub kind: String,
}

fn white() -> String {
    hex(WHITE)
}
fn default_type() -> String {
    DEFAULT.to_owned()
}

/// `#RRGGBB`.
pub fn hex(rgb: u32) -> String {
    format!("#{:06X}", rgb & 0xFF_FFFF)
}

impl Waypoint {
    pub fn new(name: &str, pos: [i32; 3], rgb: u32, kind: &str) -> Self {
        Waypoint { hidden: false, name: name.to_owned(), x: pos[0], y: pos[1], z: pos[2], color: hex(rgb), kind: kind.to_owned() }
    }
    /// The colour as 0xRRGGBB (white when the file's is not `#RRGGBB`).
    pub fn rgb(&self) -> u32 {
        u32::from_str_radix(self.color.trim_start_matches('#'), 16).map_or(WHITE, |c| c & 0xFF_FFFF)
    }
    pub fn pos(&self) -> [i32; 3] {
        [self.x, self.y, self.z]
    }
}

/// A dimension's waypoints (`WaypointManagerImpl`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Waypoints {
    #[serde(default)]
    pub waypoints: Vec<Waypoint>,
}

impl Waypoints {
    /// A file's content; an unreadable one gives none.
    pub fn parse(json: &str) -> Self {
        serde_json::from_str(json).unwrap_or_default()
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
    /// A death: the earlier death waypoints are hidden and `Death #<deaths>` is added, red, at `pos`.
    pub fn add_death(&mut self, pos: [i32; 3], deaths: i32) -> &Waypoint {
        for w in self.waypoints.iter_mut().filter(|w| w.kind == DEATH) {
            w.hidden = true;
        }
        self.waypoints.push(Waypoint::new(&format!("Death #{deaths}"), pos, DEATH_COLOR, DEATH));
        self.waypoints.last().expect("just pushed")
    }
    pub fn add(&mut self, name: &str, pos: [i32; 3], rgb: u32) {
        self.waypoints.push(Waypoint::new(name, pos, rgb, DEFAULT));
    }
}

/// The folder name of a dimension in the map folder: `minecraft:overworld` is `minecraft_overworld`.
pub fn dimension_dir(dimension: &str) -> String {
    dimension.replace(':', "_")
}

/// `add_waypoint` (`AddWaypointPacket`): from `/ftbchunks waypoint add`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddWaypoint {
    pub name: String,
    pub dim: String,
    pub pos: [i32; 3],
    pub color: u32,
    pub gui: bool,
}

/// `player_death` (`PlayerDeathPacket`): where the player died, and their death count (the `deaths` statistic).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerDeath {
    pub dim: String,
    pub pos: [i32; 3],
    pub deaths: i32,
}

/// The sixteen chat colours (`ChatFormatting`), which `ColorArgument` takes by name.
pub fn chat_color(name: &str) -> Option<u32> {
    Some(match name {
        "black" => 0x000000,
        "dark_blue" => 0x0000AA,
        "dark_green" => 0x00AA00,
        "dark_aqua" => 0x00AAAA,
        "dark_red" => 0xAA0000,
        "dark_purple" => 0xAA00AA,
        "gold" => 0xFFAA00,
        "gray" => 0xAAAAAA,
        "dark_gray" => 0x555555,
        "blue" => 0x5555FF,
        "green" => 0x55FF55,
        "aqua" => 0x55FFFF,
        "red" => 0xFF5555,
        "light_purple" => 0xFF55FF,
        "yellow" => 0xFFFF55,
        "white" => 0xFFFFFF,
        _ => return None,
    })
}

/// One coordinate of a block position: a number, `~`, `~n` (from `base`), or `^` read as `~`.
fn coordinate(word: &str, base: f64) -> Option<i32> {
    if let Some(rest) = word.strip_prefix('~').or_else(|| word.strip_prefix('^')) {
        let offset = if rest.is_empty() { 0.0 } else { rest.parse::<f64>().ok()? };
        return Some((base + offset).floor() as i32);
    }
    word.parse::<i32>().ok()
}

/// Splits a command line into words, a `"quoted name"` being one word.
fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = line.trim().chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        let mut word = String::new();
        if c == '"' {
            chars.next();
            while let Some(c) = chars.next() {
                match c {
                    '"' => break,
                    '\\' => word.extend(chars.next()),
                    c => word.push(c),
                }
            }
        } else {
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() {
                    break;
                }
                word.push(c);
                chars.next();
            }
        }
        out.push(word);
    }
    out
}

/// `/ftbchunks waypoint add <name> <position> [<dimension> [<color> [<gui>]]]` (or `/chunks ...`) from a player at
/// `pos` in `dimension`: the waypoint to send, an error for a malformed one, None for another command.
pub fn parse_add(command: &str, pos: [f64; 3], dimension: &str) -> Option<Result<AddWaypoint, String>> {
    let w = words(command);
    let is_root = matches!(w.first().map(String::as_str), Some("ftbchunks" | "chunks"));
    if !is_root || w.get(1).map(String::as_str) != Some("waypoint") || w.get(2).map(String::as_str) != Some("add") {
        return None;
    }
    let parsed = || -> Result<AddWaypoint, &'static str> {
        let name = w.get(3).ok_or("name")?.clone();
        let mut p = [0; 3];
        for (i, slot) in p.iter_mut().enumerate() {
            *slot = coordinate(w.get(4 + i).ok_or("position")?, pos[i]).ok_or("position")?;
        }
        let dim = match w.get(7) {
            Some(d) if d.contains(':') => d.clone(),
            Some(d) => format!("minecraft:{d}"),
            None => dimension.to_owned(),
        };
        let color = match w.get(8) {
            Some(c) => chat_color(c).ok_or("color")?,
            None => WHITE,
        };
        let gui = match w.get(9).map(String::as_str) {
            Some("true") => true,
            Some("false") | None => false,
            Some(_) => return Err("gui"),
        };
        Ok(AddWaypoint { name, dim, pos: p, color, gui })
    };
    Some(parsed().map_err(str::to_owned))
}

/// The minimap mods whose presence turns FTB Chunks' own minimap off by default
/// (`FTBChunksClientConfig.hasOtherMinimapMod`).
pub const OTHER_MINIMAP_MODS: [&str; 4] = ["journeymap", "voxelmap", "antiqueatlas", "xaerominimap"];

/// The `minimap.enabled` default: on unless one of `OTHER_MINIMAP_MODS` is among `mods` (comma-separated ids, the
/// bundles staged into this build).
pub fn minimap_on_by_default(mods: &str) -> bool {
    !mods.split(',').map(|m| m.split_whitespace().next().unwrap_or_default()).any(|m| OTHER_MINIMAP_MODS.contains(&m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_keeps_ftb_chunks_keys() {
        let mut w = Waypoints::default();
        w.add("Base", [1, 64, -3], 0x55FF55);
        let json = w.to_json();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let first = &v["waypoints"][0];
        assert_eq!(first["hidden"], false);
        assert_eq!(first["name"], "Base");
        assert_eq!(first["x"], 1);
        assert_eq!(first["z"], -3);
        assert_eq!(first["color"], "#55FF55");
        assert_eq!(first["type"], "default");
        assert_eq!(Waypoints::parse(&json), w);
        assert_eq!(w.waypoints[0].rgb(), 0x55FF55);
        assert_eq!(Waypoints::parse("not json"), Waypoints::default());
    }

    #[test]
    fn a_death_hides_the_earlier_death_waypoints() {
        let mut w = Waypoints::default();
        w.add("Home", [0, 70, 0], WHITE);
        w.add_death([10, 60, 10], 1);
        let last = w.add_death([20, 50, 20], 2).clone();
        assert_eq!(last.name, "Death #2");
        assert_eq!(last.color, "#FF0000");
        assert_eq!(last.kind, DEATH);
        assert!(!last.hidden);
        assert!(w.waypoints[1].hidden, "the first death waypoint is hidden");
        assert!(!w.waypoints[0].hidden, "a default waypoint is not");
    }

    #[test]
    fn the_add_command_parses_ftb_chunks_arguments() {
        let at = [10.7, 64.0, -5.2];
        let a = parse_add("ftbchunks waypoint add Base ~ ~1 ~-2", at, "minecraft:overworld").unwrap().unwrap();
        assert_eq!(a, AddWaypoint { name: "Base".into(), dim: "minecraft:overworld".into(), pos: [10, 65, -8], color: WHITE, gui: false });
        let b = parse_add("chunks waypoint add \"My base\" 1 2 3 the_nether red true", at, "minecraft:overworld").unwrap().unwrap();
        assert_eq!((b.name.as_str(), b.dim.as_str(), b.pos, b.color, b.gui), ("My base", "minecraft:the_nether", [1, 2, 3], 0xFF5555, true));
        assert!(parse_add("ftbchunks waypoint add Base 1 2", at, "x").unwrap().is_err());
        assert!(parse_add("ftbchunks waypoint add Base 1 2 3 minecraft:overworld pink", at, "x").unwrap().is_err());
        assert!(parse_add("ftbchunks claim", at, "x").is_none());
    }

    #[test]
    fn the_minimap_is_on_unless_another_minimap_mod_is_built_in() {
        assert!(minimap_on_by_default(""));
        assert!(minimap_on_by_default("ftbteams,ftbchunks,mekanism"));
        assert!(!minimap_on_by_default("ftbteams,journeymap,ftbchunks"));
        assert!(!minimap_on_by_default("xaerominimap 1.0"));
        assert_eq!(dimension_dir("minecraft:the_nether"), "minecraft_the_nether");
    }
}
