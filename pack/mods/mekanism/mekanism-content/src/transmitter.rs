//! Mekanism's OBJ transmitter model (`mekanism:transmitter`, 24 objects
//! `<direction><NONE|NORMAL|PUSH|PULL>` of axis-aligned quads) as a multipart
//! blockstate over vanilla models, for the game's model format.
//!
//! Each quad becomes one zero-thickness element with one face, its corners'
//! texture coordinates kept exactly (`flip_v` as the model file says). The push
//! and pull parts' stepped rings are four mitred trapezoids each, one pixel
//! wide, which a model element can't be: each becomes a rectangle with the same
//! texture mapping, taking one of its two corners, so a ring is covered exactly
//! and only its one-pixel corners take the neighbour's side of the mitre. A part
//! is one model per direction and connection. A side with no connection shows
//! its centre face, as `TransmitterModelConfiguration.getIconStatus` says: the
//! `#side` texture when the cable runs straight past it, turned 270 degrees on
//! one of the two axes, and the centre texture otherwise.
use crate::Result;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap};

/// `Direction` order.
pub const DIRECTIONS: [&str; 6] = ["down", "up", "north", "south", "west", "east"];
const CONNECTIONS: [&str; 3] = ["normal", "push", "pull"];
/// A side with something connected.
const CONNECTED: &str = "normal|push|pull";
const NORMALS: [[f64; 3]; 6] = [[0., -1., 0.], [0., 1., 0.], [0., 0., -1.], [0., 0., 1.], [-1., 0., 0.], [1., 0., 0.]];

/// One axis-aligned quad of the model: its direction, corners (position and
/// texture coordinates, 0 to 1) and material.
#[derive(Clone, Debug, PartialEq)]
pub struct Quad {
    direction: usize,
    corners: [([f64; 3], [f64; 2]); 4],
    texture: String,
}

/// `<object name> -> quads` of an OBJ file. `materials` gives each material's
/// texture reference (`#side`), from the `.mtl`.
pub fn parse_obj(text: &str, materials: &HashMap<String, String>, flip_v: bool) -> Result<BTreeMap<String, Vec<Quad>>> {
    let (mut positions, mut uvs, mut normals) = (Vec::new(), Vec::new(), Vec::new());
    let mut objects: BTreeMap<String, Vec<Quad>> = BTreeMap::new();
    let (mut object, mut material) = (String::new(), String::new());
    let numbers = |rest: &[&str]| -> Result<Vec<f64>> {
        rest.iter().map(|s| s.parse::<f64>().map_err(|e| format!("bad number {s}: {e}"))).collect()
    };
    for (n, line) in text.lines().enumerate() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let at = |e: String| format!("line {}: {e}", n + 1);
        match fields.first().copied() {
            Some("o") => object = fields.get(1).copied().unwrap_or("").to_owned(),
            Some("usemtl") => material = fields.get(1).copied().unwrap_or("").to_owned(),
            Some("v") => positions.push(numbers(&fields[1..]).map_err(at)?),
            Some("vt") => uvs.push(numbers(&fields[1..]).map_err(at)?),
            Some("vn") => normals.push(numbers(&fields[1..]).map_err(at)?),
            Some("f") => {
                if fields.len() != 5 {
                    return Err(at(format!("{object}: a face that isn't a quad")));
                }
                let mut corners = [([0.0; 3], [0.0; 2]); 4];
                let mut normal = None;
                for (i, corner) in fields[1..].iter().enumerate() {
                    let ids: Vec<usize> = corner.split('/').map(|s| s.parse().unwrap_or(0)).collect();
                    let [p, t, m] = ids[..] else {
                        return Err(at(format!("{object}: a corner without position, texture and normal")));
                    };
                    let get = |list: &Vec<Vec<f64>>, id: usize, what: &str| -> Result<Vec<f64>> {
                        list.get(id.wrapping_sub(1)).cloned().ok_or_else(|| at(format!("no {what} {id}")))
                    };
                    let (pos, uv) = (get(&positions, p, "vertex")?, get(&uvs, t, "texture coordinate")?);
                    if pos.len() < 3 || uv.len() < 2 {
                        return Err(at("a short vertex".to_owned()));
                    }
                    normal = Some(get(&normals, m, "normal")?);
                    corners[i] = ([pos[0], pos[1], pos[2]], [uv[0], if flip_v { 1.0 - uv[1] } else { uv[1] }]);
                }
                let normal = normal.ok_or_else(|| at("a face without a normal".to_owned()))?;
                let direction = NORMALS
                    .iter()
                    .position(|d| (0..3).all(|i| (d[i] - normal[i]).abs() < 1e-6))
                    .ok_or_else(|| at(format!("{object}: a face that isn't axis-aligned")))?;
                let texture = materials
                    .get(&material)
                    .ok_or_else(|| at(format!("{object}: no texture for material {material}")))?
                    .clone();
                objects.entry(object.clone()).or_default().push(Quad { direction, corners, texture });
            }
            _ => {}
        }
    }
    Ok(objects)
}

/// `material -> map_Kd` of an `.mtl` file.
pub fn parse_mtl(text: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut name = String::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        match fields[..] {
            ["newmtl", n] => name = n.to_owned(),
            ["map_Kd", t] => {
                out.insert(name.clone(), t.to_owned());
            }
            _ => {}
        }
    }
    out
}

/// How texture coordinates grow across a face in the game's models, for each
/// direction: the axis and sign of u and of v (`FaceBakery`'s corners).
fn face_axes(direction: usize) -> ((usize, f64), (usize, f64)) {
    match direction {
        0 => ((0, 1.), (2, -1.)),
        1 => ((0, 1.), (2, 1.)),
        2 => ((0, -1.), (1, -1.)),
        3 => ((0, 1.), (1, -1.)),
        4 => ((2, 1.), (1, -1.)),
        _ => ((2, -1.), (1, -1.)),
    }
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// Pixels with 6 decimals at most, as whole numbers when they are.
fn px(v: f64) -> Value {
    let v = (v * 16.0 * 1e6).round() / 1e6;
    json!(v)
}

/// The distinct values of a coordinate over a quad's corners, sorted.
fn values(quad: &Quad, axis: usize) -> Vec<f64> {
    let mut out: Vec<f64> = quad.corners.iter().map(|(p, _)| p[axis]).collect();
    out.sort_by(f64::total_cmp);
    out.dedup_by(|a, b| near(*a, *b));
    out
}

/// A rectangle as it is. A trapezoid (two parallel edges along one axis) as the
/// rectangle between them: as long as the short edge plus one end of the long
/// edge, the end a turn around the face's direction points to, so four of them
/// around a ring tile it like a pinwheel. Its texture mapping continues the
/// trapezoid's, which is linear in the two axes.
fn squared(quad: &Quad) -> Result<Quad> {
    let n = NORMALS[quad.direction];
    let flat = (0..3).find(|i| n[*i] != 0.0).unwrap_or(0);
    let (a, b) = ((flat + 1) % 3, (flat + 2) % 3);
    let (va, vb) = (values(quad, a), values(quad, b));
    if va.len() == 2 && vb.len() == 2 {
        return Ok(quad.clone());
    }
    let name = DIRECTIONS[quad.direction];
    let (w, e, across) = if va.len() == 2 {
        (a, b, va)
    } else if vb.len() == 2 {
        (b, a, vb)
    } else {
        return Err(format!("a {name} quad isn't a rectangle or a trapezoid"));
    };
    // The corners of each parallel edge, along it.
    let edge = |at: f64| {
        let mut corners: Vec<([f64; 3], [f64; 2])> = quad.corners.iter().filter(|(p, _)| near(p[w], at)).copied().collect();
        corners.sort_by(|x, y| x.0[e].total_cmp(&y.0[e]));
        corners
    };
    let (first, second) = (edge(across[0]), edge(across[1]));
    if first.len() != 2 || second.len() != 2 {
        return Err(format!("a {name} quad isn't a rectangle or a trapezoid"));
    }
    let long = |c: &[([f64; 3], [f64; 2])]| c[1].0[e] - c[0].0[e];
    let (outer, inner) = if long(&first) > long(&second) { (first, second) } else { (second, first) };
    let (outer_w, inner_w) = (outer[0].0[w], inner[0].0[w]);
    // The mapping's slopes along the edges and across them.
    let along = [
        (outer[1].1[0] - outer[0].1[0]) / long(&outer),
        (outer[1].1[1] - outer[0].1[1]) / long(&outer),
    ];
    let cross = [
        (inner[0].1[0] - outer[0].1[0] - along[0] * (inner[0].0[e] - outer[0].0[e])) / (inner_w - outer_w),
        (inner[0].1[1] - outer[0].1[1] - along[1] * (inner[0].0[e] - outer[0].0[e])) / (inner_w - outer_w),
    ];
    let map = |pe: f64, pw: f64| -> [f64; 2] {
        std::array::from_fn(|i| outer[0].1[i] + along[i] * (pe - outer[0].0[e]) + cross[i] * (pw - outer_w))
    };
    let check = map(inner[1].0[e], inner_w);
    if !near(check[0], inner[1].1[0]) || !near(check[1], inner[1].1[1]) {
        return Err(format!("a {name} trapezoid's texture isn't linear"));
    }
    // n x outward, along the edge: which end takes its corner.
    let outward = if outer_w > inner_w { 1.0 } else { -1.0 };
    let mut o = [0.0; 3];
    o[w] = outward;
    let turn = [n[1] * o[2] - n[2] * o[1], n[2] * o[0] - n[0] * o[2], n[0] * o[1] - n[1] * o[0]];
    let (lo, hi) = if turn[e] > 0.0 { (inner[0].0[e], outer[1].0[e]) } else { (outer[0].0[e], inner[1].0[e]) };
    let corner = |pe: f64, pw: f64| {
        let mut p = [0.0; 3];
        p[flat] = quad.corners[0].0[flat];
        p[e] = pe;
        p[w] = pw;
        (p, map(pe, pw))
    };
    Ok(Quad {
        direction: quad.direction,
        corners: [corner(lo, outer_w), corner(hi, outer_w), corner(hi, inner_w), corner(lo, inner_w)],
        texture: quad.texture.clone(),
    })
}

/// The element of one quad: zero thick along its direction, one face.
pub fn element(quad: &Quad) -> Result<Value> {
    let quad = &squared(quad)?;
    let pos = |i: usize| quad.corners.map(|(p, _)| p[i]);
    let (mut from, mut to) = ([0.0; 3], [0.0; 3]);
    for i in 0..3 {
        let values = pos(i);
        from[i] = values.iter().copied().fold(f64::MAX, f64::min);
        to[i] = values.iter().copied().fold(f64::MIN, f64::max);
    }
    let axis = [1, 2, 0][quad.direction / 2];
    if !near(from[axis], to[axis]) {
        return Err(format!("a {} quad isn't flat", DIRECTIONS[quad.direction]));
    }
    let ((ua, us), (va, vs)) = face_axes(quad.direction);
    // The face's corners as the game takes them: top left, bottom left, bottom
    // right, top right on the texture, before any rotation.
    let lo_hi = |axis: usize, sign: f64| if sign > 0.0 { (from[axis], to[axis]) } else { (to[axis], from[axis]) };
    let (u_lo, u_hi) = lo_hi(ua, us);
    let (v_lo, v_hi) = lo_hi(va, vs);
    let want = [(u_lo, v_lo), (u_lo, v_hi), (u_hi, v_hi), (u_hi, v_lo)];
    let mut uv = [[0.0; 2]; 4];
    for (i, (u, v)) in want.into_iter().enumerate() {
        uv[i] = quad
            .corners
            .iter()
            .find(|(p, _)| near(p[ua], u) && near(p[va], v))
            .ok_or_else(|| format!("a {} quad isn't a rectangle", DIRECTIONS[quad.direction]))?
            .1;
    }
    // `BlockElementFace`: corner i takes the (u0 v0, u0 v1, u1 v1, u1 v0) entry
    // `rotation / 90` further on.
    for turn in 0..4 {
        let q = |k: usize| uv[(k + 4 - turn) % 4];
        let (q0, q1, q2, q3) = (q(0), q(1), q(2), q(3));
        if near(q0[0], q1[0]) && near(q2[0], q3[0]) && near(q0[1], q3[1]) && near(q1[1], q2[1]) {
            let mut face = Map::new();
            face.insert("uv".into(), json!([px(q0[0]), px(q0[1]), px(q2[0]), px(q2[1])]));
            face.insert("texture".into(), json!(quad.texture));
            if turn != 0 {
                face.insert("rotation".into(), json!(turn * 90));
            }
            let mut faces = Map::new();
            faces.insert(DIRECTIONS[quad.direction].to_owned(), Value::Object(face));
            return Ok(json!({
                "from": from.map(px),
                "to": to.map(px),
                "faces": faces,
            }));
        }
    }
    Err(format!("a {} quad's texture is slanted", DIRECTIONS[quad.direction]))
}

/// `TransmitterBakedModel`'s turn of a centre face for `ROTATE_270`: its
/// corners 270 degrees around the opposite of its direction, about the block's
/// middle; each texture coordinate stays with its corner.
fn turned(quad: &Quad) -> Quad {
    let n = NORMALS[quad.direction];
    // 270 degrees around -n is 90 degrees around n: n x p + n (n . p).
    let spin = |p: [f64; 3]| {
        let c = [p[0] - 0.5, p[1] - 0.5, p[2] - 0.5];
        let dot = n[0] * c[0] + n[1] * c[1] + n[2] * c[2];
        [
            n[1] * c[2] - n[2] * c[1] + n[0] * dot + 0.5,
            n[2] * c[0] - n[0] * c[2] + n[1] * dot + 0.5,
            n[0] * c[1] - n[1] * c[0] + n[2] * dot + 0.5,
        ]
    };
    let mut out = quad.clone();
    for corner in &mut out.corners {
        corner.0 = spin(corner.0);
    }
    out
}

fn with_texture(quads: &[Quad], texture: &str) -> Vec<Quad> {
    quads.iter().map(|q| Quad { texture: texture.to_owned(), ..q.clone() }).collect()
}

/// The two pairs of sides across a side (`getIconStatus`): a pair both joined
/// and the other pair free runs the cable straight past it.
fn across(direction: usize) -> [usize; 4] {
    match direction {
        0 | 1 => [2, 3, 5, 4],
        2 | 3 => [1, 0, 5, 4],
        _ => [1, 0, 2, 3],
    }
}

/// Which centre model a free side shows for what joins the four sides across.
fn centre_variant(has: [bool; 4]) -> &'static str {
    let [a, b, c, d] = has;
    if (a || b) != (c || d) {
        if a && b {
            return "straight";
        }
        if c && d {
            return "straight_270";
        }
    }
    "center"
}

/// The models of one cable tier and its multipart blockstate. `textures` has
/// `side` and `center`; `display` is the item model's. Paths are relative to
/// the content root, under `assets/mekanism/`.
pub fn cable_models(
    tier: &str,
    objects: &BTreeMap<String, Vec<Quad>>,
    side: &str,
    center: &str,
    display: Option<&Value>,
) -> Result<Vec<(String, Value)>> {
    let block = format!("{tier}_universal_cable");
    let dir = format!("block/transmitter/small/universal_cable/{tier}");
    let textures = json!({ "side": side, "center": center, "particle": center });
    let quads = |name: String| -> Result<Vec<Quad>> {
        let found = objects.get(&name).ok_or_else(|| format!("the model has no object {name}"))?;
        // Every centre_<direction> material is the one centre texture here.
        Ok(found
            .iter()
            .map(|q| {
                let texture = if q.texture.starts_with("#center") { "#center" } else { "#side" };
                Quad { texture: texture.to_owned(), ..q.clone() }
            })
            .collect())
    };
    let elements = |list: &[Quad]| -> Result<Vec<Value>> { list.iter().map(element).collect() };
    let model = |list: &[Quad]| -> Result<Value> { Ok(json!({ "textures": textures, "elements": elements(list)? })) };
    let mut files = Vec::new();
    let mut multipart = Vec::new();
    let mut item_elements = Vec::new();
    for (d, name) in DIRECTIONS.iter().enumerate() {
        let upper = |c: &str| format!("{name}{}", c.to_uppercase());
        for connection in CONNECTIONS {
            let part = quads(upper(connection))?;
            // The item: up and down normal, the rest free (`TransmitterBakedModel`).
            if connection == "normal" && d < 2 {
                item_elements.extend(elements(&part)?);
            }
            files.push((format!("assets/mekanism/models/{dir}/{name}_{connection}.json"), model(&part)?));
            multipart.push(json!({
                "when": { *name: connection },
                "apply": { "model": format!("mekanism:{dir}/{name}_{connection}") },
            }));
        }
        let free = quads(upper("none"))?;
        if d >= 2 {
            item_elements.extend(elements(&free)?);
        }
        let straight = with_texture(&free, "#side");
        let spun: Vec<Quad> = straight.iter().map(turned).collect();
        for (variant, list) in [("center", &free), ("straight", &straight), ("straight_270", &spun)] {
            files.push((format!("assets/mekanism/models/{dir}/{name}_{variant}.json"), model(list)?));
        }
        let others = across(d);
        for bits in 0..16u8 {
            let has = [bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0];
            let mut when = Map::new();
            when.insert((*name).to_owned(), json!("none"));
            for (i, other) in others.iter().enumerate() {
                let value = if has[i] { CONNECTED } else { "none" };
                when.insert(DIRECTIONS[*other].to_owned(), json!(value));
            }
            multipart.push(json!({
                "when": when,
                "apply": { "model": format!("mekanism:{dir}/{name}_{}", centre_variant(has)) },
            }));
        }
    }
    files.push((format!("assets/mekanism/blockstates/{block}.json"), json!({ "multipart": multipart })));
    let mut item = json!({ "parent": "minecraft:block/block", "textures": textures, "elements": item_elements });
    if let Some(display) = display {
        item["display"] = display.clone();
    }
    files.push((format!("assets/mekanism/models/item/{block}.json"), item));
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corners(list: [([f64; 3], [f64; 2]); 4]) -> Quad {
        Quad { direction: 1, corners: list, texture: "#side".to_owned() }
    }

    /// A small model: a free side and the three connections of `up`.
    const OBJ: &str = "o upNONE
v 0.6875 0.6875 0.6875
v 0.3125 0.6875 0.6875
v 0.6875 0.6875 0.3125
v 0.3125 0.6875 0.3125
vt 1 0
vt 1 1
vt 0 1
vt 0 0
vn 0 1 0
usemtl Up
f 1/1/1 3/2/1 4/3/1 2/4/1
";

    fn materials() -> HashMap<String, String> {
        HashMap::from([("Up".to_owned(), "#center_up".to_owned())])
    }

    #[test]
    fn an_up_quad_becomes_one_flat_face_with_its_texture_corners() {
        let objects = parse_obj(OBJ, &materials(), true).unwrap();
        let quads = &objects["upNONE"];
        assert_eq!(quads.len(), 1);
        let e = element(&quads[0]).unwrap();
        assert_eq!(e["from"], json!([5.0, 11.0, 5.0]));
        assert_eq!(e["to"], json!([11.0, 11.0, 11.0]));
        let face = &e["faces"]["up"];
        assert_eq!(face["texture"], "#center_up");
        // flip_v: the corner (11, 11, 11) has (1, 1 - 0) = (16, 16), and (5, 11, 5) has (0, 0).
        assert_eq!(face["uv"], json!([0.0, 0.0, 16.0, 16.0]));
        assert!(face.get("rotation").is_none());
    }

    #[test]
    fn texture_axes_across_the_position_axes_take_a_rotation() {
        // u follows z and v follows x on an up face: a quarter turn.
        let q = corners([
            ([0.0, 1.0, 0.0], [0.0, 0.0]),
            ([1.0, 1.0, 0.0], [0.0, 1.0]),
            ([1.0, 1.0, 1.0], [1.0, 1.0]),
            ([0.0, 1.0, 1.0], [1.0, 0.0]),
        ]);
        let e = element(&q).unwrap();
        let face = &e["faces"]["up"];
        assert!(face["rotation"].is_i64(), "{face}");
        // Rebuilding each corner from uv and rotation gives the OBJ's coordinates.
        let uv: Vec<f64> = face["uv"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap() / 16.0).collect();
        let list = [[uv[0], uv[1]], [uv[0], uv[3]], [uv[2], uv[3]], [uv[2], uv[1]]];
        let turn = (face["rotation"].as_i64().unwrap() / 90) as usize;
        // Corners top left, bottom left, bottom right, top right of an up face.
        let positions = [[0.0, 1.0, 0.0], [0.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, 0.0]];
        for (i, p) in positions.iter().enumerate() {
            let expected = q.corners.iter().find(|c| c.0 == *p).unwrap().1;
            assert_eq!(list[(i + turn) % 4], expected, "corner {i}");
        }
    }

    #[test]
    fn a_ring_of_trapezoids_becomes_a_pinwheel_of_rectangles() {
        // Facing east at x = 0.9375: outer square 0.375..0.625, inner 0.4375..0.5625 (y, z),
        // with the texture mappings of Mekanism's eastPUSH.
        let east = |list: [([f64; 2], [f64; 2]); 4]| Quad {
            direction: 5,
            corners: list.map(|([y, z], uv)| ([0.9375, y, z], uv)),
            texture: "#side".to_owned(),
        };
        let ring = [
            east([([0.375, 0.625], [0.0, 0.5]), ([0.375, 0.375], [1.0, 0.5]), ([0.4375, 0.4375], [0.75, 0.75]), ([0.4375, 0.5625], [0.25, 0.75])]),
            east([([0.625, 0.625], [1.0, 0.75]), ([0.375, 0.625], [0.0, 0.75]), ([0.4375, 0.5625], [0.25, 0.5]), ([0.5625, 0.5625], [0.75, 0.5])]),
            east([([0.5625, 0.4375], [0.75, 0.25]), ([0.625, 0.375], [1.0, 0.5]), ([0.625, 0.625], [0.0, 0.5]), ([0.5625, 0.5625], [0.25, 0.25])]),
            east([([0.375, 0.375], [0.0, 0.25]), ([0.625, 0.375], [1.0, 0.25]), ([0.5625, 0.4375], [0.75, 0.5]), ([0.4375, 0.4375], [0.25, 0.5])]),
        ];
        // On a 64-cell grid over the outer square, each cell of the ring is covered exactly once.
        let cell = 0.25 / 64.0;
        let mut seen = vec![vec![0u8; 64]; 64];
        for trapezoid in &ring {
            let r = squared(trapezoid).unwrap();
            assert_eq!(values(&r, 1).len(), 2);
            assert_eq!(values(&r, 2).len(), 2);
            let (ys, zs) = (values(&r, 1), values(&r, 2));
            for (i, row) in seen.iter_mut().enumerate() {
                for (j, count) in row.iter_mut().enumerate() {
                    let (y, z) = (0.375 + (i as f64 + 0.5) * cell, 0.375 + (j as f64 + 0.5) * cell);
                    if y > ys[0] && y < ys[1] && z > zs[0] && z < zs[1] {
                        *count += 1;
                    }
                }
            }
            // What a trapezoid's own corners had, the rectangle's mapping keeps.
            let e = element(trapezoid).unwrap();
            assert!(e["faces"]["east"]["uv"].is_array());
        }
        for (i, row) in seen.iter().enumerate() {
            for (j, count) in row.iter().enumerate() {
                let (y, z) = (0.375 + (i as f64 + 0.5) * cell, 0.375 + (j as f64 + 0.5) * cell);
                let inside = y > 0.4375 && y < 0.5625 && z > 0.4375 && z < 0.5625;
                assert_eq!(*count, u8::from(!inside), "cell {i} {j}");
            }
        }
        // The mapping continues: on the first trapezoid u = (0.625 - z) * 4, v = 0.5 + (y - 0.375) * 4.
        let r = squared(&ring[0]).unwrap();
        for (p, uv) in r.corners {
            assert!(near(uv[0], (0.625 - p[2]) * 4.0) && near(uv[1], 0.5 + (p[1] - 0.375) * 4.0), "{p:?} {uv:?}");
        }
    }

    #[test]
    fn a_slanted_texture_or_face_stops_the_build() {
        let mut q = corners([
            ([0.0, 1.0, 0.0], [0.0, 0.0]),
            ([1.0, 1.0, 0.0], [1.0, 0.25]),
            ([1.0, 1.0, 1.0], [1.0, 1.0]),
            ([0.0, 1.0, 1.0], [0.0, 1.0]),
        ]);
        assert!(element(&q).unwrap_err().contains("slanted"));
        q.corners[0].0[1] = 0.5;
        assert!(element(&q).is_err());
        // Neither rectangle nor trapezoid.
        let mut kite = corners([
            ([0.0, 1.0, 0.5], [0.0, 0.0]),
            ([0.5, 1.0, 0.0], [1.0, 0.0]),
            ([1.0, 1.0, 0.5], [1.0, 1.0]),
            ([0.5, 1.0, 1.0], [0.0, 1.0]),
        ]);
        assert!(element(&kite).unwrap_err().contains("trapezoid"));
        // A trapezoid with a texture that isn't linear in its axes.
        kite.corners = [
            ([0.0, 1.0, 0.0], [0.0, 0.0]),
            ([1.0, 1.0, 0.0], [1.0, 0.0]),
            ([0.75, 1.0, 0.5], [0.9, 0.5]),
            ([0.25, 1.0, 0.5], [0.25, 0.5]),
        ];
        assert!(element(&kite).unwrap_err().contains("linear"));
        let slanted = OBJ.replace("vn 0 1 0", "vn 0 0.7 0.7");
        assert!(parse_obj(&slanted, &materials(), true).unwrap_err().contains("axis-aligned"));
    }

    #[test]
    fn the_turned_centre_keeps_its_place_and_moves_its_texture() {
        let objects = parse_obj(OBJ, &materials(), true).unwrap();
        let quad = &objects["upNONE"][0];
        let spun = turned(quad);
        let (a, b) = (element(quad).unwrap(), element(&spun).unwrap());
        assert_eq!(a["from"], b["from"]);
        assert_eq!(a["to"], b["to"]);
        assert_ne!(a["faces"], b["faces"]);
        // Four turns are where it started.
        let back = turned(&turned(&spun));
        let round = turned(&back);
        assert_eq!(element(&round).unwrap()["faces"], a["faces"]);
    }

    #[test]
    fn a_free_side_shows_as_get_icon_status_says() {
        for (has, want) in [
            ([true, true, false, false], "straight"),
            ([false, false, true, true], "straight_270"),
            ([true, false, false, false], "center"),
            ([true, true, true, false], "center"),
            ([true, true, true, true], "center"),
            ([false, false, false, false], "center"),
            ([false, false, true, false], "center"),
        ] {
            assert_eq!(centre_variant(has), want, "{has:?}");
        }
    }

    /// All 24 objects, one quad each.
    fn whole_model() -> BTreeMap<String, Vec<Quad>> {
        let mut out = BTreeMap::new();
        for (d, name) in DIRECTIONS.iter().enumerate() {
            let n = NORMALS[d];
            let at = |a: f64, b: f64| -> [f64; 3] {
                let plane = |i: usize| match i {
                    _ if n[i] != 0.0 => 0.5 + n[i] * 0.1875,
                    _ => 0.0,
                };
                let free: Vec<usize> = (0..3).filter(|i| n[*i] == 0.0).collect();
                let mut p = [plane(0), plane(1), plane(2)];
                p[free[0]] = a;
                p[free[1]] = b;
                p
            };
            let quad = |texture: &str| Quad {
                direction: d,
                corners: [
                    (at(0.25, 0.25), [0.0, 0.0]),
                    (at(0.75, 0.25), [1.0, 0.0]),
                    (at(0.75, 0.75), [1.0, 1.0]),
                    (at(0.25, 0.75), [0.0, 1.0]),
                ],
                texture: texture.to_owned(),
            };
            out.insert(format!("{name}NONE"), vec![quad(&format!("#center_{name}"))]);
            for c in ["NORMAL", "PUSH", "PULL"] {
                out.insert(format!("{name}{c}"), vec![quad("#side")]);
            }
        }
        out
    }

    #[test]
    fn a_tier_has_36_parts_and_one_rule_per_case() {
        let display = json!({ "gui": { "scale": [1, 1, 1] } });
        let files = cable_models("basic", &whole_model(), "mekanism:s", "mekanism:c", Some(&display)).unwrap();
        let models = files.iter().filter(|(p, _)| p.contains("/universal_cable/basic/")).count();
        assert_eq!(models, 36);
        let (_, state) = files.iter().find(|(p, _)| p.ends_with("blockstates/basic_universal_cable.json")).unwrap();
        let rules = state["multipart"].as_array().unwrap();
        assert_eq!(rules.len(), 6 * (3 + 16));
        // Up free, north and south joined, east and west free: the straight side texture.
        let matching: Vec<&Value> = rules
            .iter()
            .filter(|r| {
                let w = &r["when"];
                w["up"] == "none" && w["north"] == CONNECTED && w["south"] == CONNECTED && w["east"] == "none" && w["west"] == "none"
            })
            .collect();
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0]["apply"]["model"], "mekanism:block/transmitter/small/universal_cable/basic/up_straight");
        let (_, item) = files.iter().find(|(p, _)| p.ends_with("models/item/basic_universal_cable.json")).unwrap();
        assert_eq!(item["display"], display);
        assert_eq!(item["elements"].as_array().unwrap().len(), 6, "down and up normal, four free");
        assert!(item["elements"][0]["faces"]["down"]["texture"] == "#side");
    }

    #[test]
    fn a_missing_object_stops_the_build() {
        let mut objects = whole_model();
        objects.remove("northPUSH");
        assert!(cable_models("basic", &objects, "s", "c", None).unwrap_err().contains("northPUSH"));
    }

    #[test]
    fn materials_come_from_the_mtl() {
        let m = parse_mtl("newmtl None\nKd 1 1 1\nmap_Kd #side\n\nnewmtl CentreMaterialUp\nmap_Kd #center_up\n");
        assert_eq!(m["None"], "#side");
        assert_eq!(m["CentreMaterialUp"], "#center_up");
    }
}
