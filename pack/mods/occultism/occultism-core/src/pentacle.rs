//! Pentacles: the chalk patterns a ritual needs around its golden bowl. Occultism keeps them as Modonomicon's dense
//! multiblocks (`data/<ns>/modonomicon/multiblocks/<id>.json`) and checks one with `Multiblock.validate(level, pos)`:
//! the pattern's `0` sits on the bowl, and any of the four rotations may fit. The format, as Occultism's files use it:
//! `pattern` is a list of layers from the top down, each a list of rows (z) of characters (x); `mapping` names a
//! matcher per character (`modonomicon:block`, `modonomicon:tag`, `modonomicon:display`, which accepts anything);
//! `_` is any block and a space is air.

use serde_json::Value;

/// `net.minecraft.world.level.block.Rotation`, in `Rotation.values()` order, the order `validate` tries them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
    None,
    Clockwise90,
    Clockwise180,
    CounterClockwise90,
}

impl Rotation {
    pub const ALL: [Rotation; 4] = [Rotation::None, Rotation::Clockwise90, Rotation::Clockwise180, Rotation::CounterClockwise90];

    /// `BlockPos.rotate(rotation)`.
    pub fn apply(self, [x, y, z]: [i32; 3]) -> [i32; 3] {
        match self {
            Rotation::None => [x, y, z],
            Rotation::Clockwise90 => [-z, y, x],
            Rotation::Clockwise180 => [-x, y, -z],
            Rotation::CounterClockwise90 => [z, y, -x],
        }
    }
}

/// What the world holds, as a pentacle reads it.
pub trait Blocks {
    /// The block at `pos` (`occultism:chalk_glyph_white`; `minecraft:air` where nothing is).
    fn block(&self, pos: [i32; 3]) -> String;
    /// Property `key` of the block at `pos`.
    fn property(&self, pos: [i32; 3], key: &str) -> Option<String>;
    /// Whether block `block` is in block tag `tag` (no `#`).
    fn in_tag(&self, block: &str, tag: &str) -> bool;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Matcher {
    Any,
    Air,
    /// `modonomicon:display`: shown in the book, never checked.
    Display,
    Block { block: String, properties: Vec<(String, String)> },
    Tag { tag: String, properties: Vec<(String, String)> },
}

fn is_air(block: &str) -> bool {
    matches!(block, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
}

/// `minecraft:candle[lit=true]` as a name and its properties.
fn split_state(state: &str) -> (String, Vec<(String, String)>) {
    let Some((name, rest)) = state.split_once('[') else {
        return (crate::ingredient::qualified(state), Vec::new());
    };
    let properties = rest
        .trim_end_matches(']')
        .split(',')
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect();
    (crate::ingredient::qualified(name), properties)
}

impl Matcher {
    fn parse(json: &Value) -> Result<Matcher, String> {
        let field = |name: &str| json.get(name).and_then(Value::as_str).ok_or_else(|| format!("matcher without {name}: {json}"));
        match json.get("type").and_then(Value::as_str) {
            Some("modonomicon:display") => Ok(Matcher::Display),
            Some("modonomicon:any") => Ok(Matcher::Any),
            Some("modonomicon:block") | Some("modonomicon:blockstate") => {
                let (block, properties) = split_state(field("block")?);
                Ok(Matcher::Block { block, properties })
            }
            Some("modonomicon:tag") => {
                let (tag, properties) = split_state(field("tag")?.trim_start_matches('#'));
                Ok(Matcher::Tag { tag, properties })
            }
            other => Err(format!("unknown matcher type {other:?}")),
        }
    }

    pub fn test(&self, world: &dyn Blocks, pos: [i32; 3]) -> bool {
        let has = |properties: &[(String, String)]| properties.iter().all(|(k, v)| world.property(pos, k).as_deref() == Some(v));
        match self {
            Matcher::Any | Matcher::Display => true,
            Matcher::Air => is_air(&world.block(pos)),
            Matcher::Block { block, properties } => world.block(pos) == *block && has(properties),
            Matcher::Tag { tag, properties } => world.in_tag(&world.block(pos), tag) && has(properties),
        }
    }

    /// Whether this position matters for the ritual (the book shows the rest).
    pub fn checked(&self) -> bool {
        !matches!(self, Matcher::Any | Matcher::Display)
    }
}

/// A dense multiblock: its size (x, layers, z), the position of its `0` and the matcher of every position.
#[derive(Clone, Debug)]
pub struct Pentacle {
    pub id: String,
    pub size: [i32; 3],
    pub offset: [i32; 3],
    pub symmetrical: bool,
    pub targets: Vec<([i32; 3], Matcher)>,
}

impl Pentacle {
    pub fn parse(id: &str, json: &Value) -> Result<Pentacle, String> {
        if json.get("type").and_then(Value::as_str) != Some("modonomicon:dense") {
            return Err(format!("{id}: not a dense multiblock"));
        }
        let mut mapping = std::collections::HashMap::new();
        for (key, matcher) in json.get("mapping").and_then(Value::as_object).ok_or(format!("{id}: no mapping"))? {
            let mut chars = key.chars();
            let (Some(c), None) = (chars.next(), chars.next()) else {
                return Err(format!("{id}: mapping key {key:?}"));
            };
            mapping.insert(c, Matcher::parse(matcher).map_err(|e| format!("{id}: {e}"))?);
        }
        let layers: Vec<Vec<&str>> = json
            .get("pattern")
            .and_then(Value::as_array)
            .ok_or(format!("{id}: no pattern"))?
            .iter()
            .map(|layer| layer.as_array().map(|rows| rows.iter().filter_map(Value::as_str).collect()).unwrap_or_default())
            .collect();
        let height = layers.len() as i32;
        let depth = layers.first().map_or(0, Vec::len) as i32;
        let width = layers.first().and_then(|l| l.first()).map_or(0, |r| r.chars().count()) as i32;
        let mut targets = Vec::new();
        let mut offset = None;
        for (i, layer) in layers.iter().enumerate() {
            if layer.len() as i32 != depth {
                return Err(format!("{id}: layer {i} has {} rows, not {depth}", layer.len()));
            }
            for (j, row) in layer.iter().enumerate() {
                if row.chars().count() as i32 != width {
                    return Err(format!("{id}: row {j} of layer {i} is not {width} wide"));
                }
                for (k, c) in row.chars().enumerate() {
                    // Patchouli's and Modonomicon's layout: x = column, y = layers from the bottom, z = row.
                    let pos = [k as i32, height - i as i32 - 1, j as i32];
                    if c == '0' {
                        offset = Some(pos);
                    }
                    let matcher = match (mapping.get(&c), c) {
                        (Some(m), _) => m.clone(),
                        (None, '_') => Matcher::Any,
                        (None, ' ') => Matcher::Air,
                        (None, c) => return Err(format!("{id}: character {c:?} has no mapping")),
                    };
                    targets.push((pos, matcher));
                }
            }
        }
        Ok(Pentacle {
            id: id.to_owned(),
            size: [width, height, depth],
            offset: offset.ok_or(format!("{id}: no center (0)"))?,
            symmetrical: json.get("symmetrical").and_then(Value::as_bool).unwrap_or(false),
            targets,
        })
    }

    /// The world position of pattern position `pos` with the center on `center`: `center + rotate(pos - offset)`.
    pub fn world_pos(&self, center: [i32; 3], rotation: Rotation, pos: [i32; 3]) -> [i32; 3] {
        let [dx, dy, dz] = rotation.apply([pos[0] - self.offset[0], pos[1] - self.offset[1], pos[2] - self.offset[2]]);
        [center[0] + dx, center[1] + dy, center[2] + dz]
    }

    /// Every checked position missing in one rotation.
    pub fn missing(&self, world: &dyn Blocks, center: [i32; 3], rotation: Rotation) -> Vec<[i32; 3]> {
        self.targets
            .iter()
            .filter(|(_, m)| m.checked())
            .map(|(pos, m)| (self.world_pos(center, rotation, *pos), m))
            .filter(|(at, m)| !m.test(world, *at))
            .map(|(at, _)| at)
            .collect()
    }

    /// `Multiblock.validate(level, center)`: the first rotation that fits, none if none does. A symmetrical
    /// pentacle is tried as it is only.
    pub fn validate(&self, world: &dyn Blocks, center: [i32; 3]) -> Option<Rotation> {
        let rotations: &[Rotation] = if self.symmetrical { &Rotation::ALL[..1] } else { &Rotation::ALL };
        rotations.iter().copied().find(|r| self.targets.iter().all(|(pos, m)| m.test(world, self.world_pos(center, *r, *pos))))
    }

    /// The box `Ritual.getSacrificialBowls` searches: 8 blocks around the bowl, and from one layer below the
    /// pentacle's lowest to one above its highest. Corners, inclusive.
    pub fn bowl_box(&self, center: [i32; 3]) -> ([i32; 3], [i32; 3]) {
        let top = self.size[1] - self.offset[1] - 1 + 1;
        let bottom = self.offset[1] + 1;
        let r = crate::ritual::SACRIFICIAL_BOWL_RANGE;
        ([center[0] - r, center[1] - bottom, center[2] - r], [center[0] + r, center[1] + top, center[2] + r])
    }

    /// The box's positions in `BlockPos.betweenClosed` order (x fastest, then y, then z): the order bowls are
    /// found, and so the order ingredients are taken from them.
    pub fn bowl_positions(&self, center: [i32; 3]) -> Vec<[i32; 3]> {
        let (min, max) = self.bowl_box(center);
        let mut out = Vec::new();
        for z in min[2]..=max[2] {
            for y in min[1]..=max[1] {
                for x in min[0]..=max[0] {
                    out.push([x, y, z]);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    /// A small pentacle of the same shape as Occultism's: glyphs and candles above a display-only floor.
    fn sample() -> Pentacle {
        let json = json!({
            "type": "modonomicon:dense",
            "mapping": {
                "0": {"type": "modonomicon:tag", "display": "occultism:golden_sacrificial_bowl", "tag": "#occultism:center_sacrificial_bowl"},
                "W": {"type": "modonomicon:tag", "tag": "#occultism:foundation_glyphs_any"},
                "1": {"type": "modonomicon:tag", "tag": "#minecraft:candles"},
                "2": {"type": "modonomicon:block", "block": "minecraft:skeleton_skull"},
                "*": {"type": "modonomicon:display", "display": "occultism:otherstone"}
            },
            "pattern": [
                ["_W2", "W0_", "1__"],
                ["***", "***", "***"]
            ]
        });
        Pentacle::parse("test:sample", &json).unwrap()
    }

    #[derive(Default)]
    struct World(HashMap<[i32; 3], String>);
    impl Blocks for World {
        fn block(&self, pos: [i32; 3]) -> String {
            self.0.get(&pos).cloned().unwrap_or_else(|| "minecraft:air".into())
        }
        fn property(&self, _: [i32; 3], _: &str) -> Option<String> {
            None
        }
        fn in_tag(&self, block: &str, tag: &str) -> bool {
            match tag {
                "occultism:center_sacrificial_bowl" => block == "occultism:golden_sacrificial_bowl",
                "occultism:foundation_glyphs_any" => block == "occultism:chalk_glyph_white",
                "minecraft:candles" => block == "minecraft:candle",
                _ => false,
            }
        }
    }

    /// The sample drawn around `center` in `rotation`.
    fn drawn(p: &Pentacle, center: [i32; 3], rotation: Rotation) -> World {
        let mut w = World::default();
        for (pos, m) in &p.targets {
            let block = match m {
                Matcher::Tag { tag, .. } if tag == "occultism:center_sacrificial_bowl" => "occultism:golden_sacrificial_bowl",
                Matcher::Tag { tag, .. } if tag == "minecraft:candles" => "minecraft:candle",
                Matcher::Tag { .. } => "occultism:chalk_glyph_white",
                Matcher::Block { block, .. } => block,
                _ => continue,
            };
            w.0.insert(p.world_pos(center, rotation, *pos), block.into());
        }
        w
    }

    #[test]
    fn the_layout_is_patchoulis_top_layer_first_and_the_center_is_the_offset() {
        let p = sample();
        assert_eq!(p.size, [3, 2, 3]);
        assert_eq!(p.offset, [1, 1, 1], "0 is in the top layer, the middle row and column");
        assert_eq!(p.targets.iter().filter(|(_, m)| m.checked()).count(), 5);
        assert!(p.targets.contains(&([2, 1, 0], Matcher::Block { block: "minecraft:skeleton_skull".into(), properties: vec![] })));
        assert_eq!(p.world_pos([10, 64, 10], Rotation::None, [2, 1, 0]), [11, 64, 9]);
        // CLOCKWISE_90 takes (x, z) to (-z, x).
        assert_eq!(p.world_pos([10, 64, 10], Rotation::Clockwise90, [2, 1, 0]), [11, 64, 11]);
    }

    #[test]
    fn a_pentacle_fits_in_each_rotation_and_reports_the_first_that_fits() {
        let p = sample();
        let center = [100, 64, -20];
        for rotation in Rotation::ALL {
            assert_eq!(p.validate(&drawn(&p, center, rotation), center), Some(rotation), "{rotation:?}");
        }
        // The floor is display only: drawn on air it still fits.
        assert!(drawn(&p, center, Rotation::None).0.keys().all(|pos| pos[1] == 64));
    }

    #[test]
    fn a_missing_or_wrong_glyph_breaks_it() {
        let p = sample();
        let center = [0, 70, 0];
        let mut w = drawn(&p, center, Rotation::Clockwise180);
        let skull = p.world_pos(center, Rotation::Clockwise180, [2, 1, 0]);
        w.0.insert(skull, "minecraft:wither_skeleton_skull".into());
        assert_eq!(p.validate(&w, center), None);
        assert_eq!(p.missing(&w, center, Rotation::Clockwise180), vec![skull]);
        w.0.insert(skull, "minecraft:skeleton_skull".into());
        assert_eq!(p.validate(&w, center), Some(Rotation::Clockwise180));
        w.0.remove(&center);
        assert_eq!(p.validate(&w, center), None, "the bowl is part of it");
    }

    #[test]
    fn bowls_are_searched_eight_around_and_one_past_each_layer() {
        let p = sample();
        // Two layers, the center on the top one (y 1): one below the floor (-2) to one above the top (+1).
        assert_eq!(p.bowl_box([0, 64, 0]), ([-8, 62, -8], [8, 65, 8]));
        let order = p.bowl_positions([0, 64, 0]);
        assert_eq!(order.len(), 17 * 4 * 17);
        assert_eq!(&order[..2], &[[-8, 62, -8], [-7, 62, -8]], "x first, then y, then z");
        assert_eq!(order[17], [-8, 63, -8]);
    }

    #[test]
    fn unmapped_characters_and_a_missing_center_are_refused() {
        let bad = json!({"type": "modonomicon:dense", "mapping": {}, "pattern": [["_X_"]]});
        assert!(Pentacle::parse("t:bad", &bad).unwrap_err().contains("'X'"));
        let no_center = json!({"type": "modonomicon:dense", "mapping": {}, "pattern": [["___"]]});
        assert!(Pentacle::parse("t:none", &no_center).unwrap_err().contains("no center"));
    }

    /// Every pentacle of a carry parses, with its sizes; `OCCULTISM_CARRY` names the carry (skipped without it).
    #[test]
    fn the_carrys_pentacles_parse() {
        let Some(carry) = std::env::var_os("OCCULTISM_CARRY") else { return };
        let dir = std::path::Path::new(&carry).join("data/occultism/modonomicon/multiblocks");
        let mut seen = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let json: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            let id = format!("occultism:{}", path.file_stem().unwrap().to_string_lossy());
            let p = Pentacle::parse(&id, &json).unwrap();
            if id == "occultism:craft_foliot" {
                assert_eq!((p.size, p.offset), ([9, 2, 9], [4, 1, 4]));
                assert_eq!(p.targets.iter().filter(|(_, m)| m.checked()).count(), 37, "28 glyphs, 8 candles and the bowl");
            }
            seen += 1;
        }
        assert!(seen >= 20, "{seen} pentacles");
    }
}
