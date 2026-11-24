//! The loader's own data, carried from the player's NeoForge jar (its universal jar, LGPL-2.1) into one folder mod,
//! `neoforge`, beside the carries: the common `c` tags (`c:crops` and the `c:crops/wheat`... it names), the tags of
//! `neoforge`, its additions to vanilla's tags, and its own damage type. Mods' tags and recipes name these, and no mod
//! jar defines them. Like every carry it is made on the player's machine from their own install and never committed.
//!
//! Every tag file is written as an addition (`replace` is never set), so vanilla's tags and the mods' gain members
//! instead of losing them. What the jar has besides is code-backed, or vanilla's own data in NeoForge's 1.21.1 form,
//! which the game keeps as 26.3 has it; it is counted in [`Carried::skipped`] by reason.
use crate::carry::renamed_ids;
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// The folder (and mod id) of the loader's data among the carries.
pub const FOLDER: &str = "neoforge";

#[derive(Debug, Default, Serialize, PartialEq)]
pub struct Carried {
    /// Tag files written, by namespace.
    pub tags: BTreeMap<String, usize>,
    /// Other files written (the `neoforge` damage type).
    pub data: usize,
    /// Files of `data/` left in the jar, by reason.
    pub skipped: BTreeMap<String, usize>,
}

/// Why a file of the jar's `data/` is not carried, or `None` when it is.
fn skipped(path: &str) -> Option<&'static str> {
    let mut parts = path.splitn(4, '/');
    let (_, ns, kind) = (parts.next()?, parts.next()?, parts.next()?);
    match (ns, kind) {
        ("c" | "minecraft" | "neoforge", "tags") => None,
        ("neoforge", "damage_type") => None,
        ("minecraft", "recipe" | "loot_table" | "advancement") => Some(
            "vanilla's recipes, loot tables and advancements in NeoForge's 1.21.1 form: the game keeps 26.3's own",
        ),
        ("neoforge", "data_maps") => Some(
            "data maps of vanilla's values (fuel, compost, waxing...): the game keeps 26.3's own",
        ),
        ("neoforge", "loot_modifiers") => Some("global loot modifiers: code-backed"),
        _ => Some("other"),
    }
}

/// The NeoForge jar's data into the folder mod `out` (replaced), its manifest naming the jar.
pub fn carry(jar: &Path, out: &Path) -> Result<Carried, String> {
    let file = fs::File::open(jar).map_err(|e| format!("{}: {e}", jar.display()))?;
    let mut zip = crate::zip_intake::open(file).map_err(|e| format!("{}: {e}", jar.display()))?;
    let _ = fs::remove_dir_all(out);
    let mut carried = Carried::default();
    let mut budget = crate::entry::Budget::archive();
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| format!("{}: {e}", jar.display()))?;
        let Some(name) = entry.name().ok().map(|n| n.to_string()) else {
            continue;
        };
        if !name.starts_with("data/") || name.ends_with('/') {
            continue;
        }
        if let Some(why) = skipped(&name) {
            *carried.skipped.entry(why.to_owned()).or_default() += 1;
            continue;
        }
        if !name.ends_with(".json") {
            *carried.skipped.entry("other".to_owned()).or_default() += 1;
            continue;
        }
        let declared = entry.size();
        let text = budget
            .read_text(&mut entry, declared, crate::entry::MAX_TEXT)
            .map_err(|e| format!("{name}: {e}"))?;
        let mut doc: Value = serde_json::from_str(&text).map_err(|e| format!("{name}: {e}"))?;
        let ns = name.split('/').nth(1).unwrap_or_default().to_owned();
        let doc = if name.split('/').nth(2) == Some("tags") {
            *carried.tags.entry(ns).or_default() += 1;
            addition(&doc)
        } else {
            carried.data += 1;
            renamed_ids(&mut doc);
            doc
        };
        let dest = out.join(&name);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let bytes = serde_json::to_vec_pretty(&doc).map_err(|e| e.to_string())?;
        fs::write(&dest, bytes).map_err(|e| format!("{}: {e}", dest.display()))?;
    }
    if carried.tags.is_empty() {
        return Err(format!(
            "{}: no tags under data/: not NeoForge's universal jar",
            jar.display()
        ));
    }
    let jar_name = jar
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("the loader's jar");
    let version = jar_name
        .strip_prefix("neoforge-")
        .and_then(|v| v.strip_suffix("-universal.jar"))
        .unwrap_or("0");
    let manifest = format!(
        "# Carried over from {jar_name} by horizon-port-pack, for local use only.\n# Licence: LGPL-2.1-only\nid = \"{FOLDER}\"\nname = \"NeoForge (data)\"\nversion = {}\napi = \"0.2\"\nside = \"both\"\ncapabilities = [\"registries\"]\n",
        toml::Value::String(version.to_owned())
    );
    fs::write(out.join(horizon_mod_package::manifest::MANIFEST), manifest)
        .map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(carried)
}

/// A tag file as an addition to whatever else defines the tag: its entries (ids renamed to 26.3's) and its removals,
/// never `replace`.
fn addition(doc: &Value) -> Value {
    let mut values = doc.get("values").cloned().unwrap_or_else(|| json!([]));
    renamed_ids(&mut values);
    let mut out = json!({ "values": values });
    if let Some(remove) = doc.get("remove") {
        let mut remove = remove.clone();
        renamed_ids(&mut remove);
        out["remove"] = remove;
    }
    out
}

/// The loader jar `fetch` keeps in `<work>/loader/`.
pub fn work_jar(work: &Path) -> Option<PathBuf> {
    fs::read_dir(work.join("loader"))
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.to_string_lossy().ends_with("-universal.jar"))
}

/// The player's own NeoForge `version` jar beside an instance: CurseForge keeps it in
/// `<instances>/../Install/libraries`, the other launchers in the game folder's `libraries`.
pub fn installed_jar(instance: &Path, version: &str) -> Option<PathBuf> {
    let rel = format!("net/neoforged/neoforge/{version}/neoforge-{version}-universal.jar");
    let curseforge = instance
        .parent()?
        .parent()
        .map(|root| root.join("Install/libraries").join(&rel));
    [
        curseforge,
        Some(instance.join("libraries").join(&rel)),
        instance.parent().map(|p| p.join("libraries").join(&rel)),
    ]
    .into_iter()
    .flatten()
    .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn jar(dir: &Path, files: &[(&str, &str)]) -> PathBuf {
        let path = dir.join("neoforge-1.2.3-universal.jar");
        let mut z = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        for (name, text) in files {
            z.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(text.as_bytes()).unwrap();
        }
        z.finish().unwrap();
        path
    }

    fn read(path: PathBuf) -> Value {
        serde_json::from_slice(
            &fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
        )
        .unwrap()
    }

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hpp-loader-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn tags_are_carried_as_additions_and_code_stays_behind() {
        let dir = tmp("carry");
        let jar = jar(
            &dir,
            &[
                (
                    "data/c/tags/item/grains.json",
                    r##"{"replace": true, "values": ["#c:grains/oat", {"id": "#forge:grains", "required": false}]}"##,
                ),
                (
                    "data/c/tags/item/grains/oat.json",
                    r#"{"values": ["minecraft:chain"]}"#,
                ),
                (
                    "data/minecraft/tags/block/sand_like.json",
                    r##"{"values": ["#c:sands"], "remove": ["minecraft:chain"]}"##,
                ),
                (
                    "data/c/tags/worldgen/biome/is_wet.json",
                    r#"{"values": ["minecraft:river"]}"#,
                ),
                ("data/neoforge/tags/item/fuels.json", r#"{"values": []}"#),
                (
                    "data/neoforge/damage_type/sting.json",
                    r#"{"message_id": "sting", "exhaustion": 0.1, "scaling": "never"}"#,
                ),
                (
                    "data/minecraft/recipe/stick.json",
                    r#"{"type": "minecraft:crafting_shaped"}"#,
                ),
                ("data/minecraft/loot_table/blocks/fern.json", r#"{}"#),
                (
                    "data/neoforge/data_maps/item/fuel.json",
                    r#"{"values": {}}"#,
                ),
                (
                    "data/neoforge/loot_modifiers/global_loot_modifiers.json",
                    r#"{"entries": []}"#,
                ),
                ("assets/neoforge/lang/en_us.json", r#"{}"#),
                ("META-INF/MANIFEST.MF", "x"),
            ],
        );
        let out = dir.join("carries").join(FOLDER);
        fs::create_dir_all(out.join("data/c/tags/item")).unwrap();
        fs::write(out.join("data/c/tags/item/stale.json"), r#"{"values": []}"#).unwrap();
        let c = carry(&jar, &out).unwrap();
        assert_eq!(
            c.tags,
            BTreeMap::from([
                ("c".into(), 3),
                ("minecraft".into(), 1),
                ("neoforge".into(), 1)
            ])
        );
        assert_eq!((c.data, c.skipped.values().sum::<usize>()), (1, 4));
        assert!(c.skipped.keys().any(|k| k.contains("code-backed")));
        // Additions only: replace dropped, entries kept as they are (optional ones too), 1.21.1's ids renamed.
        assert_eq!(
            read(out.join("data/c/tags/item/grains.json")),
            json!({"values": ["#c:grains/oat", {"id": "#forge:grains", "required": false}]})
        );
        assert_eq!(
            read(out.join("data/c/tags/item/grains/oat.json")),
            json!({"values": ["minecraft:iron_chain"]})
        );
        assert_eq!(
            read(out.join("data/minecraft/tags/block/sand_like.json")),
            json!({"values": ["#c:sands"], "remove": ["minecraft:iron_chain"]})
        );
        assert!(out.join("data/c/tags/worldgen/biome/is_wet.json").is_file());
        assert!(out.join("data/neoforge/damage_type/sting.json").is_file());
        for gone in [
            "data/minecraft/recipe",
            "data/minecraft/loot_table",
            "data/neoforge/data_maps",
            "data/neoforge/loot_modifiers",
            "assets",
            "data/c/tags/item/stale.json",
        ] {
            assert!(!out.join(gone).exists(), "{gone}");
        }
        let manifest =
            fs::read_to_string(out.join(horizon_mod_package::manifest::MANIFEST)).unwrap();
        let m = horizon_mod_package::Manifest::parse(&manifest).unwrap();
        assert_eq!((m.id.as_str(), m.version.as_str()), (FOLDER, "1.2.3"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_jar_with_no_tags_is_not_the_loader() {
        let dir = tmp("none");
        let jar = jar(&dir, &[("data/x/recipe/a.json", "{}")]);
        assert!(
            carry(&jar, &dir.join(FOLDER))
                .unwrap_err()
                .contains("not NeoForge")
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_installed_jar_is_found_beside_a_curseforge_instance() {
        let dir = tmp("installed");
        let instance = dir.join("minecraft/Instances/Pack");
        fs::create_dir_all(&instance).unwrap();
        assert_eq!(installed_jar(&instance, "21.1.9"), None);
        let lib = dir.join("minecraft/Install/libraries/net/neoforged/neoforge/21.1.9/neoforge-21.1.9-universal.jar");
        fs::create_dir_all(lib.parent().unwrap()).unwrap();
        fs::write(&lib, b"jar").unwrap();
        assert_eq!(installed_jar(&instance, "21.1.9"), Some(lib));
        let _ = fs::remove_dir_all(&dir);
    }
}
