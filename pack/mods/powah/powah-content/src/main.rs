//! `powah-content --carry DIR --out DIR`: builds the content mod `powah` that
//! the game loads with `--dev-mod`, from the carry of the player's own
//! `Powah-6.2.10.jar` (LGPL-3.0; models and textures by Cyn and SammySemicolon).
//!
//! The carry already holds Powah's 92 plain blocks, its items, textures,
//! recipes and loot tables. This tool adds what `horizon-port-pack` can't
//! guess from a jar:
//!
//! - the blocks it leaves out: the seven energy cables (six booleans, a side
//!   each) and the seven player transmitters (`top`), each with its own state
//!   properties, and the recipes it set aside because their blocks were
//!   missing (`<carry>/../outside/data/powah/recipe`);
//! - no working block on a template that works by itself: until its slice
//!   lands, a magmator is an inert block with `facing` and `lit`, not a
//!   vanilla furnace;
//! - the Furnators (`native_screen` and the machine are the native bundle's):
//!   the furnace template brings the block entity, the menu and `facing`, with
//!   the `lit` state that picks the burning face. Java draws that face with a
//!   block entity renderer; here it is a model per state.
//!
//! The output is generated and never committed: Powah's files stay local. The
//! tool may only replace a folder it made itself (`powah-content.json`).
use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};

const MARKER: &str = "powah-content.json";
const TIERS: [&str; 7] = ["starter", "basic", "hardened", "blazing", "niotic", "spirited", "nitro"];
const SIDES: [&str; 6] = ["north", "east", "south", "west", "up", "down"];
/// Y rotation of a model facing north, by the block's `facing`.
const FACINGS: [(&str, i64); 4] = [("north", 0), ("east", 90), ("south", 180), ("west", 270)];

type Result<T> = std::result::Result<T, String>;

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut carry, mut out) = (None, None);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--carry" => carry = args.next().map(PathBuf::from),
            "--out" => out = args.next().map(PathBuf::from),
            "-h" | "--help" => {
                println!("Usage: powah-content --carry DIR --out DIR");
                return;
            }
            other => fail(&format!("unknown argument {other}")),
        }
    }
    let (Some(carry), Some(out)) = (carry, out) else {
        fail("needs --carry and --out");
    };
    match build(&carry, &out) {
        Ok(report) => println!("{}", serde_json::to_string(&report).unwrap_or_default()),
        Err(e) => fail(&e),
    }
}

fn fail(message: &str) -> ! {
    eprintln!("powah-content: {message}");
    std::process::exit(1);
}

fn build(carry: &Path, out: &Path) -> Result<Value> {
    if !carry.join("horizon-mod.toml").is_file() {
        return Err(format!("{} is not a carried content mod", carry.display()));
    }
    prepare(out)?;
    let copied = copy_tree(carry, out)?;
    let blocks = out.join("data/powah/horizon/block");
    let mut declared = Vec::new();
    for tier in TIERS {
        // A cable's six sides; the machine, its block entity and shapes are slice P2.
        let cable = format!("energy_cable_{tier}");
        if !blocks.join(format!("{cable}.json")).exists() {
            let properties: Map<String, Value> = SIDES.iter().map(|s| ((*s).to_owned(), json!({ "type": "boolean" }))).collect();
            write_json(
                &blocks.join(format!("{cable}.json")),
                &json!({
                    "template": "minecraft:stone",
                    "properties": { "no_occlusion": true, "hardness": 2.0, "explosion_resistance": 6.0, "sounds": "minecraft:iron_block", "requires_tool": true },
                    "state_properties": properties,
                }),
            )?;
            declared.push(cable);
        }
        // A transmitter is two blocks high; its logic is slice P7.
        let transmitter = format!("player_transmitter_{tier}");
        if !blocks.join(format!("{transmitter}.json")).exists() {
            write_json(
                &blocks.join(format!("{transmitter}.json")),
                &json!({
                    "template": "minecraft:stone",
                    "properties": { "no_occlusion": true, "hardness": 2.0, "explosion_resistance": 6.0, "sounds": "minecraft:iron_block", "requires_tool": true },
                    "state_properties": { "top": { "type": "boolean" } },
                }),
            )?;
            declared.push(transmitter);
        }
    }
    let recipes = restore_recipes(carry, out)?;
    let mut inert = Vec::new();
    for tier in TIERS {
        let name = format!("magmator_{tier}");
        let path = blocks.join(format!("{name}.json"));
        let mut decl = read_json(&path)?;
        if decl["template"] == "minecraft:furnace" {
            decl["template"] = json!("minecraft:carved_pumpkin");
            decl["state_properties"] = json!({ "lit": { "type": "boolean" } });
            write_json(&path, &decl)?;
            inert.push(name);
        }
    }
    let mut furnators = Vec::new();
    for tier in TIERS {
        furnator(out, tier)?;
        furnators.push(format!("furnator_{tier}"));
    }
    let report = json!({
        "carry": carry, "out": out, "copied": copied, "declared": declared, "recipes_restored": recipes,
        "made_inert": inert, "furnators": furnators,
    });
    write_json(&out.join(MARKER), &report)?;
    Ok(report)
}

/// A Furnator's declaration, its lit model and its blockstate.
fn furnator(out: &Path, tier: &str) -> Result<()> {
    let name = format!("furnator_{tier}");
    let decl_path = out.join(format!("data/powah/horizon/block/{name}.json"));
    let carried = read_json(&decl_path)?;
    write_json(&decl_path, &furnator_declaration(&carried))?;
    let models = out.join("assets/powah/models/block");
    let base = read_json(&models.join("furnator.json"))?;
    write_json(&models.join("furnator_lit.json"), &lit_base_model(&base)?)?;
    let model = read_json(&models.join(format!("{name}.json")))?;
    write_json(&models.join(format!("{name}_lit.json")), &lit_model(&model)?)?;
    write_json(&out.join(format!("assets/powah/blockstates/{name}.json")), &furnator_blockstate(&name))
}

/// The furnace template: `facing`, `lit`, a block entity and a menu, with what
/// the carry knew of the block. Lit or not, it gives no light, as in Java,
/// whose lit face is only drawn. A carry with the oracle's facts declares
/// Powah's own `facing` on a template without one; the game refuses a declared
/// property its template has, so the furnace's replaces it.
fn furnator_declaration(carried: &Value) -> Value {
    let mut decl = carried.clone();
    decl["template"] = json!("minecraft:furnace");
    if let Some(fields) = decl.as_object_mut() {
        for field in ["state_properties", "state_defaults", "default_state", "placement"] {
            fields.remove(field);
        }
    }
    let mut properties = carried["properties"].as_object().cloned().unwrap_or_default();
    properties.insert("light".into(), json!(0));
    decl["properties"] = Value::Object(properties);
    decl
}

/// The block's shared model with its burning face (`#lit`) full bright.
fn lit_base_model(base: &Value) -> Result<Value> {
    let mut model = base.clone();
    let elements = model["elements"].as_array_mut().ok_or("furnator.json has no elements")?;
    let mut lit = 0;
    for element in elements {
        let faces = element["faces"].as_object().ok_or("an element without faces")?;
        if faces.values().any(|f| f["texture"] == "#2") {
            element["light_emission"] = json!(15);
            lit += 1;
        }
    }
    if lit == 0 {
        return Err("furnator.json has no element with the lit texture #2".into());
    }
    Ok(model)
}

/// A tier's model with the burning face's texture and the lit parent.
fn lit_model(model: &Value) -> Result<Value> {
    let mut lit = model.clone();
    if lit["textures"]["lit"].as_str().is_none() {
        return Err("a Furnator model without a lit texture".into());
    }
    lit["parent"] = json!("powah:block/furnator_lit");
    lit["textures"]["lit"] = json!("powah:block/furnator_lit");
    Ok(lit)
}

/// `facing` and `lit` of the furnace template, picking the model and turning it.
fn furnator_blockstate(name: &str) -> Value {
    let mut variants = Map::new();
    for (facing, y) in FACINGS {
        for (lit, suffix) in [("false", ""), ("true", "_lit")] {
            let mut apply = json!({ "model": format!("powah:block/{name}{suffix}") });
            if y != 0 {
                apply["y"] = json!(y);
            }
            variants.insert(format!("facing={facing},lit={lit}"), apply);
        }
    }
    json!({ "variants": variants })
}

/// The recipes the carry set aside for blocks it left out.
fn restore_recipes(carry: &Path, out: &Path) -> Result<usize> {
    let Some(parent) = carry.parent() else {
        return Ok(0);
    };
    let aside = parent.join("outside/data/powah/recipe");
    if !aside.is_dir() {
        return Ok(0);
    }
    fs::create_dir_all(out.join("data/powah/recipe")).map_err(|e| e.to_string())?;
    copy_tree(&aside, &out.join("data/powah/recipe"))
}

/// Empties `out` when this tool made it; refuses any other folder.
fn prepare(out: &Path) -> Result<()> {
    if out.exists() {
        let empty = fs::read_dir(out).map_err(|e| e.to_string())?.next().is_none();
        if !empty && !out.join(MARKER).is_file() {
            return Err(format!("{} exists and was not made by powah-content", out.display()));
        }
        fs::remove_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))
}

fn copy_tree(from: &Path, to: &Path) -> Result<usize> {
    let mut count = 0;
    for entry in fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            fs::create_dir_all(&target).map_err(|e| format!("{}: {e}", target.display()))?;
            count += copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), &target).map_err(|e| format!("{}: {e}", target.display()))?;
            count += 1;
        }
    }
    Ok(count)
}

fn read_json(path: &Path) -> Result<Value> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let mut text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    text.push('\n');
    fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_furnator_keeps_what_the_carry_knew_on_the_furnace_template() {
        let decl = furnator_declaration(&json!({
            "template": "minecraft:carved_pumpkin",
            "properties": { "hardness": 2.0, "sounds": "minecraft:stone" },
        }));
        assert_eq!(decl["template"], "minecraft:furnace");
        assert_eq!(decl["properties"]["hardness"], 2.0);
        assert_eq!(decl["properties"]["sounds"], "minecraft:stone");
        assert_eq!(decl["properties"]["light"], 0);
    }

    #[test]
    fn a_furnator_declares_nothing_the_furnace_template_has() {
        let decl = furnator_declaration(&json!({
            "template": "minecraft:iron_block",
            "components": { "minecraft:max_stack_size": 1 },
            "placement": { "facing": "horizontal_look_opposite" },
            "properties": { "hardness": 2.0 },
            "state_defaults": { "facing": "north" },
            "state_properties": { "facing": ["north", "east", "south", "west", "up", "down"] },
        }));
        assert_eq!(decl["template"], "minecraft:furnace");
        assert_eq!(decl["components"]["minecraft:max_stack_size"], 1);
        for field in ["state_properties", "state_defaults", "placement"] {
            assert!(decl.get(field).is_none(), "{field}");
        }
    }

    #[test]
    fn only_the_burning_face_is_full_bright() {
        let lit = lit_base_model(&json!({ "elements": [
            { "faces": { "north": { "texture": "#0" } } },
            { "faces": { "north": { "texture": "#2" } } },
        ] }))
        .unwrap();
        assert!(lit["elements"][0].get("light_emission").is_none());
        assert_eq!(lit["elements"][1]["light_emission"], 15);
        assert!(lit_base_model(&json!({ "elements": [{ "faces": { "north": { "texture": "#0" } } }] })).is_err());
    }

    #[test]
    fn the_lit_model_swaps_the_texture_and_parent() {
        let lit = lit_model(&json!({
            "parent": "powah:block/furnator",
            "textures": { "lit": "powah:block/furnator_unlit", "ov": "powah:block/starter_ov" },
        }))
        .unwrap();
        assert_eq!(lit["parent"], "powah:block/furnator_lit");
        assert_eq!(lit["textures"]["lit"], "powah:block/furnator_lit");
        assert_eq!(lit["textures"]["ov"], "powah:block/starter_ov");
    }

    #[test]
    fn the_blockstate_has_a_turned_model_per_facing_and_lit() {
        let state = furnator_blockstate("furnator_nitro");
        let variants = state["variants"].as_object().unwrap();
        assert_eq!(variants.len(), 8);
        assert_eq!(variants["facing=north,lit=false"], json!({ "model": "powah:block/furnator_nitro" }));
        assert_eq!(variants["facing=east,lit=true"], json!({ "model": "powah:block/furnator_nitro_lit", "y": 90 }));
        assert_eq!(variants["facing=west,lit=false"]["y"], 270);
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("powah-content-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn it_replaces_only_its_own_folder() {
        let dir = scratch("own");
        let out = dir.join("out");
        fs::create_dir_all(&out).unwrap();
        fs::write(out.join("keep.txt"), "x").unwrap();
        assert!(prepare(&out).is_err(), "a folder of something else");
        assert!(out.join("keep.txt").exists());
        fs::write(out.join(MARKER), "{}").unwrap();
        prepare(&out).unwrap();
        assert!(!out.join("keep.txt").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_small_carry_comes_out_whole() {
        let dir = scratch("carry");
        let carry = dir.join("mod/carry");
        let blocks = carry.join("data/powah/horizon/block");
        let models = carry.join("assets/powah/models/block");
        fs::create_dir_all(&blocks).unwrap();
        fs::create_dir_all(&models).unwrap();
        fs::write(carry.join("horizon-mod.toml"), "id = \"powah\"\n").unwrap();
        for tier in TIERS {
            write_json(&blocks.join(format!("furnator_{tier}.json")), &json!({ "template": "minecraft:carved_pumpkin" })).unwrap();
            write_json(&blocks.join(format!("magmator_{tier}.json")), &json!({ "template": "minecraft:furnace" })).unwrap();
            write_json(&models.join(format!("furnator_{tier}.json")), &json!({ "parent": "powah:block/furnator", "textures": { "lit": "powah:block/furnator_unlit" } })).unwrap();
        }
        write_json(&models.join("furnator.json"), &json!({ "elements": [{ "faces": { "north": { "texture": "#2" } } }] })).unwrap();
        let aside = dir.join("mod/outside/data/powah/recipe/crafting");
        write_json(&aside.join("cable_starter.json"), &json!({ "type": "minecraft:crafting_shaped" })).unwrap();
        let out = dir.join("out");
        let report = build(&carry, &out).unwrap();
        assert_eq!(report["declared"].as_array().unwrap().len(), 14);
        assert_eq!(report["recipes_restored"], 1);
        assert_eq!(report["made_inert"].as_array().unwrap().len(), 7);
        assert!(out.join("data/powah/recipe/crafting/cable_starter.json").is_file());
        let cable = read_json(&out.join("data/powah/horizon/block/energy_cable_nitro.json")).unwrap();
        assert_eq!(cable["state_properties"].as_object().unwrap().len(), 6);
        // `sounds` names a block whose sounds it takes.
        assert_eq!(cable["properties"]["sounds"], "minecraft:iron_block");
        let magmator = read_json(&out.join("data/powah/horizon/block/magmator_basic.json")).unwrap();
        assert_eq!(magmator["template"], "minecraft:carved_pumpkin");
        let furnator = read_json(&out.join("data/powah/horizon/block/furnator_basic.json")).unwrap();
        assert_eq!(furnator["template"], "minecraft:furnace");
        assert!(out.join("assets/powah/blockstates/furnator_basic.json").is_file());
        assert!(out.join("assets/powah/models/block/furnator_basic_lit.json").is_file());
        assert!(out.join(MARKER).is_file());
        let _ = fs::remove_dir_all(&dir);
    }
}
