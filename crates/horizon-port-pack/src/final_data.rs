//! A pack's final data (`tools/neoforge_oracle.py pack`: the recipes, tags and loot tables the running game holds
//! once every data pack, KubeJS script and Almost Unified replacement has had its say) put in place over its carries.
//!
//! The content fence decides where a file can go. A mod's files live in namespaces it owns, and no two mods own one;
//! only `minecraft` and `c` tags are shared. So:
//! - data of a namespace a carry owns replaces that carry's own (ids stay, so advancements and loot lookups still
//!   point at them). Recipes and non-worldgen tags are all replaced, which is how a recipe KubeJS removed goes away.
//! - everything else goes in one mod, the folder `atm10-pack`, with the id `kubejs` (its items' ids must stay
//!   `kubejs:...`, and a mod declares items only under its id). It owns the namespaces no mod of the pack has, and
//!   holds `c` and `minecraft` tags.
//! - a recipe of a mod that is in the pack but not carried (a native port will own that namespace, and a clash would
//!   refuse this mod) is `atm10_pack:<namespace>/<path>`.
//!
//! Vanilla's recipes and loot tables stay the game's: no mod may replace them.
//! The result is generated from the player's instance and never committed: KubeJS scripts are All Rights Reserved.
use crate::carry::{ingredient, recipe_result, renamed_ids, upgrade_loot};
use crate::oracle::Facts;
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

/// The folder of the pack's own content mod, among the carries.
pub const PACK_FOLDER: &str = "atm10-pack";
/// Its id: see the module's text.
pub const PACK_ID: &str = "kubejs";
/// The namespace of recipes whose own namespace it cannot own.
pub const MOVED_NAMESPACE: &str = "atm10_pack";

pub struct Options<'a> {
    /// `<carries>/<folder>/horizon-mod.toml`...; the pack's folder is made here.
    pub carries: &'a Path,
    /// The output of `neoforge_oracle.py pack`.
    pub dump: &'a Path,
    /// The game's prepared tables and data.
    pub resources: &'a Path,
    /// The modpack instance, for KubeJS's assets.
    pub instance: Option<&'a Path>,
    /// Mod ids of the pack: their namespaces are theirs even without a carry.
    pub mods: &'a BTreeSet<String>,
    /// Other content mods that load beside these (AE2's): folders.
    pub also: &'a [PathBuf],
}

#[derive(Debug, Default, Serialize)]
pub struct Summary {
    pub recipes_in_dump: usize,
    /// Written to carries, and to the pack mod.
    pub recipes_in_carries: usize,
    pub recipes_in_pack: usize,
    /// Under `atm10_pack:` because their mod is in the pack but not carried.
    pub recipes_moved: BTreeMap<String, usize>,
    /// Vanilla's and other mods' own files stand for these.
    pub recipes_vanilla: usize,
    pub recipes_of_also: usize,
    /// In 26.3's vanilla form.
    pub recipes_upgraded: usize,
    /// Result not an item of ours: outside `data/`, as carry does.
    pub recipes_unknown_result: BTreeMap<String, usize>,
    /// The dump has no JSON for them.
    pub recipes_failed: usize,
    /// Almost Unified's notes to its client.
    pub recipes_trackers: usize,
    pub tags_in_dump: usize,
    pub tags_in_carries: usize,
    pub tags_in_pack: usize,
    pub tags_worldgen_left: usize,
    pub tags_left_out: BTreeMap<String, usize>,
    pub loot_in_dump: usize,
    pub loot_in_carries: usize,
    pub loot_in_pack: usize,
    pub loot_vanilla: usize,
    pub loot_left_out: BTreeMap<String, usize>,
    pub loot_failed: usize,
    pub kubejs_blocks: usize,
    pub kubejs_items: usize,
    pub kubejs_assets: usize,
    /// Namespaces of mods not carried: their port will own them.
    pub kubejs_assets_left_out: usize,
    pub namespaces: Vec<String>,
}

/// Every mod id in an instance's `mods/` jars: a jar's `info` in the inventory is only there for jars fetched.
pub fn instance_mods(instance: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for jar in fs::read_dir(instance.join("mods")).into_iter().flatten().flatten() {
        let Ok(file) = fs::File::open(jar.path()) else { continue };
        let Ok(mut zip) = zip::ZipArchive::new(file) else { continue };
        let Ok(mut entry) = zip.by_name("META-INF/neoforge.mods.toml") else { continue };
        let mut text = String::new();
        if std::io::Read::read_to_string(&mut entry, &mut text).is_err() {
            continue;
        }
        let Ok(toml) = text.parse::<toml::Table>() else { continue };
        for m in toml.get("mods").and_then(|m| m.as_array()).into_iter().flatten() {
            if let Some(id) = m.get("modId").and_then(|i| i.as_str()) {
                out.insert(id.to_owned());
            }
        }
    }
    out
}

/// Writes files, making each directory once.
struct Sink {
    made: HashSet<PathBuf>,
}

impl Sink {
    fn write(&mut self, path: &Path, bytes: &[u8]) -> Result<(), String> {
        if let Some(parent) = path.parent()
            && !self.made.contains(parent)
        {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
            self.made.insert(parent.to_owned());
        }
        fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
    }
    fn json(&mut self, path: &Path, v: &Value) -> Result<(), String> {
        self.write(path, &serde_json::to_vec(v).unwrap_or_default())
    }
}

/// Reads one JSON object a line.
fn lines(path: &Path, mut each: impl FnMut(Value) -> Result<(), String>) -> Result<(), String> {
    let file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|e| format!("{}: {e}", path.display()))?;
        if line.is_empty() {
            continue;
        }
        let v = serde_json::from_str(&line).map_err(|e| format!("{}: {e}", path.display()))?;
        each(v)?;
    }
    Ok(())
}

/// The ids an ingredient of the dump stands for: items, tags opened, NeoForge's compound forms worked out.
/// `None` for an ingredient type of a mod's own.
fn members(v: &Value, tags: &HashMap<String, Vec<String>>) -> Option<Vec<String>> {
    match v {
        Value::String(s) => match s.strip_prefix('#') {
            Some(tag) => Some(tags.get(tag).cloned().unwrap_or_default()),
            None => Some(vec![s.clone()]),
        },
        Value::Array(list) => {
            let mut out = Vec::new();
            for x in list {
                out.extend(members(x, tags)?);
            }
            Some(out)
        }
        Value::Object(o) => {
            if let Some(Value::String(id)) = o.get("item") {
                return Some(vec![id.clone()]);
            }
            if let Some(Value::String(tag)) = o.get("tag") {
                return Some(tags.get(tag).cloned().unwrap_or_default());
            }
            match o.get("type").and_then(Value::as_str)? {
                "neoforge:compound" => members(o.get("children")?, tags),
                "neoforge:difference" => {
                    let minus: BTreeSet<String> = members(o.get("subtracted")?, tags)?.into_iter().collect();
                    let base = members(o.get("base")?, tags)?;
                    Some(base.into_iter().filter(|i| !minus.contains(i)).collect())
                }
                "neoforge:intersection" => {
                    let mut all = o.get("children")?.as_array()?.iter();
                    let first = members(all.next()?, tags)?;
                    let mut keep: BTreeSet<String> = first.iter().cloned().collect();
                    for c in all {
                        let m: BTreeSet<String> = members(c, tags)?.into_iter().collect();
                        keep.retain(|i| m.contains(i));
                    }
                    Some(first.into_iter().filter(|i| keep.contains(i)).collect())
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// An ingredient in 26.3's form (a single id or tag, or a list of ids), going through its members when it has no
/// such form: 26.3 has no list with a tag in it, and no NeoForge compound.
fn ingredient_26(v: &Value, tags: &HashMap<String, Vec<String>>) -> Option<Value> {
    ingredient(v).or_else(|| {
        let mut ids = members(v, tags)?;
        let mut seen = HashSet::new();
        ids.retain(|i| seen.insert(i.clone()));
        Some(match ids.as_slice() {
            [one] => json!(one),
            _ => json!(ids),
        })
    })
}

/// A recipe of the dump in 26.3's form, as carry upgrades a jar's: vanilla's types get their ingredients rewritten,
/// a mod's type stays as its mod wrote it. Whether it was upgraded.
fn upgrade(recipe: &mut Value, tags: &HashMap<String, Vec<String>>) -> bool {
    renamed_ids(recipe);
    // KubeJS's own serializers for its shaped and shapeless wrappers read as vanilla's.
    if let Some(kind) = recipe["type"].as_str().and_then(|k| match k {
        "kubejs:shaped" => Some("minecraft:crafting_shaped"),
        "kubejs:shapeless" => Some("minecraft:crafting_shapeless"),
        _ => None,
    }) {
        recipe["type"] = json!(kind);
    }
    if !recipe["type"].as_str().unwrap_or("").starts_with("minecraft:") {
        return false;
    }
    let mut out = recipe.clone();
    if let Some(key) = out.get_mut("key").and_then(Value::as_object_mut) {
        for v in key.values_mut() {
            let Some(new) = ingredient_26(v, tags) else {
                return false;
            };
            *v = new;
        }
    }
    if let Some(list) = out.get_mut("ingredients").and_then(Value::as_array_mut) {
        for v in list.iter_mut() {
            let Some(new) = ingredient_26(v, tags) else {
                return false;
            };
            *v = new;
        }
    }
    for field in ["ingredient", "base", "addition", "template"] {
        if let Some(v) = out.get_mut(field) {
            let Some(new) = ingredient_26(v, tags) else {
                return false;
            };
            *v = new;
        }
    }
    *recipe = out;
    true
}

/// Where a file of a registry's tags goes under `tags/`: `item`, `worldgen/biome`, `mekanism/chemical`.
fn tag_dir(registry: &str) -> String {
    match registry.split_once(':') {
        Some(("minecraft", path)) => path.to_owned(),
        Some((ns, path)) => format!("{ns}/{path}"),
        None => registry.to_owned(),
    }
}

fn split_id(id: &str) -> Option<(&str, &str)> {
    let (ns, path) = id.split_once(':')?;
    horizon_mods::content::check_path(&format!("data/{ns}/{path}")).ok()?;
    Some((ns, path))
}

/// Every item a declaration folder gives: its blocks with an item, and its items.
fn declared_items(carry: &Path, id: &str, out: &mut HashSet<String>) {
    let decl = carry.join(format!("data/{id}/horizon"));
    for kind in ["block", "item"] {
        let mut stack = vec![decl.join(kind)];
        while let Some(dir) = stack.pop() {
            for f in fs::read_dir(&dir).into_iter().flatten().flatten() {
                let path = f.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let Some(rel) = path
                    .strip_prefix(decl.join(kind))
                    .ok()
                    .and_then(Path::to_str)
                    .and_then(|r| r.strip_suffix(".json"))
                else {
                    continue;
                };
                let has_item = kind == "item"
                    || fs::read(&path)
                        .ok()
                        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                        .is_none_or(|v| v["item"] != json!(false));
                if has_item {
                    out.insert(format!("{id}:{}", rel.replace('\\', "/")));
                }
            }
        }
    }
}

fn manifest_of(dir: &Path) -> Option<horizon_mods::Manifest> {
    let text = fs::read_to_string(dir.join(horizon_mods::manifest::MANIFEST)).ok()?;
    horizon_mods::Manifest::parse(&text).ok()
}

/// A mod's recipes and its tags other than worldgen's: the dump's replace them.
fn clear_replaced(carry: &Path) {
    for ns in fs::read_dir(carry.join("data")).into_iter().flatten().flatten() {
        let _ = fs::remove_dir_all(ns.path().join("recipe"));
        for kind in fs::read_dir(ns.path().join("tags")).into_iter().flatten().flatten() {
            if kind.file_name() != "worldgen" {
                let path = kind.path();
                if path.is_dir() {
                    let _ = fs::remove_dir_all(path);
                } else {
                    let _ = fs::remove_file(path);
                }
            }
        }
    }
}

pub fn apply(o: &Options) -> Result<Summary, String> {
    let mut s = Summary::default();
    let pack = o.carries.join(PACK_FOLDER);
    let outside = o.carries.with_file_name(format!(
        "{}-outside",
        o.carries.file_name().and_then(|n| n.to_str()).unwrap_or("carries")
    ));
    let _ = fs::remove_dir_all(&pack);
    let _ = fs::remove_dir_all(&outside);
    // Who owns what, and every item there is.
    let mut owner: HashMap<String, PathBuf> = HashMap::new();
    let mut known: HashSet<String> = HashSet::new();
    let items: BTreeMap<String, Value> = serde_json::from_slice(
        &fs::read(o.resources.join("items.json")).map_err(|e| format!("items.json: {e}"))?,
    )
    .map_err(|e| format!("items.json: {e}"))?;
    known.extend(items.into_keys());
    for dir in fs::read_dir(o.carries).map_err(|e| format!("{}: {e}", o.carries.display()))? {
        let dir = dir.map_err(|e| e.to_string())?.path();
        if dir == pack {
            continue;
        }
        let Some(m) = manifest_of(&dir) else { continue };
        declared_items(&dir, &m.id, &mut known);
        for ns in m.owned_namespaces() {
            owner.insert(ns.to_owned(), dir.clone());
        }
        clear_replaced(&dir);
    }
    let mut also_owned: HashSet<String> = HashSet::new();
    for dir in o.also {
        let Some(m) = manifest_of(dir) else { continue };
        declared_items(dir, &m.id, &mut known);
        also_owned.extend(m.owned_namespaces().map(str::to_owned));
    }
    let mut sink = Sink { made: HashSet::new() };
    // The pack's namespaces: those of no mod, and the pack tools' own. `claimed` grows as files need it.
    let pack_tools = ["kubejs", "kubejstweaks", "almostunified"];
    let free = |ns: &str| {
        !matches!(ns, "minecraft" | "c" | "horizon")
            && !also_owned.contains(ns)
            && (pack_tools.contains(&ns) || !o.mods.contains(ns))
    };
    let mut claimed: BTreeSet<String> = BTreeSet::new();
    let dir_of = |ns: &str, claimed: &mut BTreeSet<String>| -> Option<(PathBuf, bool)> {
        if let Some(d) = owner.get(ns) {
            return Some((d.clone(), false));
        }
        free(ns).then(|| {
            claimed.insert(ns.to_owned());
            (pack.clone(), true)
        })
    };
    // Final tags first: item ingredients open them.
    let mut item_tags: HashMap<String, Vec<String>> = HashMap::new();
    let tags_file = o.dump.join("tags.jsonl");
    lines(&tags_file, |row| {
        if row["registry"] == "minecraft:item"
            && let Some(id) = row["id"].as_str()
        {
            let values = row["values"].as_array().into_iter().flatten();
            let values = values.filter_map(|v| v.as_str().map(str::to_owned)).collect();
            item_tags.insert(id.to_owned(), values);
        }
        Ok(())
    })?;
    lines(&tags_file, |row| {
        s.tags_in_dump += 1;
        let (Some(registry), Some((ns, path))) =
            (row["registry"].as_str(), row["id"].as_str().and_then(split_id))
        else {
            return Ok(());
        };
        if registry.starts_with("minecraft:worldgen/") {
            s.tags_worldgen_left += 1;
            return Ok(());
        }
        let shared = ns == "minecraft" || ns == "c";
        let target = if shared { Some((pack.clone(), true)) } else { dir_of(ns, &mut claimed) };
        let Some((root, in_pack)) = target else {
            *s.tags_left_out.entry(ns.to_owned()).or_default() += 1;
            return Ok(());
        };
        let mut values = row["values"].clone();
        renamed_ids(&mut values);
        let file = root.join(format!("data/{ns}/tags/{}/{path}.json", tag_dir(registry)));
        sink.json(&file, &json!({"values": values}))?;
        if in_pack {
            s.tags_in_pack += 1;
        } else {
            s.tags_in_carries += 1;
        }
        Ok(())
    })?;
    // KubeJS's assets: its own, and what it lays over other namespaces (textures, names) where those live.
    if let Some(instance) = o.instance {
        let assets = instance.join("kubejs/assets");
        for ns in fs::read_dir(&assets).into_iter().flatten().flatten() {
            let name = ns.file_name().to_string_lossy().into_owned();
            let root = if name == PACK_ID {
                Some(pack.clone())
            } else {
                dir_of(&name, &mut claimed).map(|(root, _)| root)
            };
            let Some(root) = root else {
                s.kubejs_assets_left_out += 1;
                continue;
            };
            let mut stack = vec![ns.path()];
            while let Some(d) = stack.pop() {
                for f in fs::read_dir(&d).into_iter().flatten().flatten() {
                    let path = f.path();
                    if path.is_dir() {
                        stack.push(path);
                        continue;
                    }
                    let Some(rel) = path.strip_prefix(&assets).ok().and_then(Path::to_str) else {
                        continue;
                    };
                    let rel = format!("assets/{}", rel.replace('\\', "/"));
                    if horizon_mods::content::check_path(&rel).is_err() {
                        continue;
                    }
                    let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                    sink.write(&root.join(&rel), &bytes)?;
                    s.kubejs_assets += 1;
                }
            }
        }
    }
    // KubeJS's items first: recipes check their results against them.
    let facts_path = o.dump.join("facts.json");
    if facts_path.exists() {
        let mut facts = Facts::read(&facts_path)?;
        facts.vanilla_items = serde_json::from_slice(&fs::read(o.resources.join("items.json")).unwrap_or_default())
            .unwrap_or_default();
        kubejs_content(o, &facts, &pack, &mut sink, &mut known, &mut s)?;
        claimed.insert(PACK_ID.to_owned());
    }
    let vanilla_recipes = o.resources.join("data/minecraft/recipe");
    lines(&o.dump.join("recipes.jsonl"), |row| {
        s.recipes_in_dump += 1;
        let Some((ns, path)) = row["id"].as_str().and_then(split_id) else {
            s.recipes_failed += 1;
            return Ok(());
        };
        let Some(json) = row.get("json") else {
            s.recipes_failed += 1;
            return Ok(());
        };
        // Almost Unified's notes to its own client, not recipes.
        if json["type"] == "almostunified:client_recipe_tracker" {
            s.recipes_trackers += 1;
            return Ok(());
        }
        if ns == "minecraft" && vanilla_recipes.join(format!("{path}.json")).exists() {
            s.recipes_vanilla += 1;
            return Ok(());
        }
        let in_also = also_owned.contains(ns)
            && o.also.iter().any(|d| d.join(format!("data/{ns}/recipe/{path}.json")).exists());
        if in_also {
            s.recipes_of_also += 1;
            return Ok(());
        }
        let mut recipe = json.clone();
        if upgrade(&mut recipe, &item_tags) {
            s.recipes_upgraded += 1;
        }
        // Vanilla refuses a recipe for an unknown item, and the game would craft air.
        let result = recipe_result(&recipe).map(str::to_owned);
        if let Some(result) = result.filter(|r| !known.contains(r)) {
            let of = result.split(':').next().unwrap_or("").to_owned();
            *s.recipes_unknown_result.entry(of).or_default() += 1;
            let file = outside.join(format!("recipe/{ns}/{path}.json"));
            return sink.json(&file, &recipe);
        }
        let (file, counted) = match dir_of(ns, &mut claimed) {
            Some((root, in_pack)) => (root.join(format!("data/{ns}/recipe/{path}.json")), in_pack),
            None => {
                *s.recipes_moved.entry(ns.to_owned()).or_default() += 1;
                claimed.insert(MOVED_NAMESPACE.to_owned());
                (pack.join(format!("data/{MOVED_NAMESPACE}/recipe/{ns}/{path}.json")), true)
            }
        };
        sink.json(&file, &recipe)?;
        if counted {
            s.recipes_in_pack += 1;
        } else {
            s.recipes_in_carries += 1;
        }
        Ok(())
    })?;
    lines(&o.dump.join("loot_tables.jsonl"), |row| {
        s.loot_in_dump += 1;
        let (Some((ns, path)), Some(json)) = (row["id"].as_str().and_then(split_id), row.get("json")) else {
            s.loot_failed += 1;
            return Ok(());
        };
        if ns == "minecraft" {
            s.loot_vanilla += 1;
            return Ok(());
        }
        let Some((root, in_pack)) = dir_of(ns, &mut claimed) else {
            *s.loot_left_out.entry(ns.to_owned()).or_default() += 1;
            return Ok(());
        };
        let mut table = json.clone();
        upgrade_loot(&mut table);
        renamed_ids(&mut table);
        sink.json(&root.join(format!("data/{ns}/loot_table/{path}.json")), &table)?;
        if in_pack {
            s.loot_in_pack += 1;
        } else {
            s.loot_in_carries += 1;
        }
        Ok(())
    })?;
    claimed.remove(PACK_ID);
    s.namespaces = claimed.iter().cloned().collect();
    let namespaces: Vec<String> = claimed.iter().map(|n| format!("{n:?}")).collect();
    let manifest = format!(
        "# Generated from the player's ATM10 instance by horizon-port-pack final-data, for local use only. Never commit.\n\
         id = {PACK_ID:?}\nname = \"atm10-pack\"\nversion = \"8.3\"\napi = \"0.2\"\nside = \"both\"\n\
         capabilities = [\"registries\"]\nnamespaces = [{}]\n",
        namespaces.join(", ")
    );
    sink.write(&pack.join(horizon_mods::manifest::MANIFEST), manifest.as_bytes())?;
    Ok(s)
}

/// The blocks and items KubeJS's startup scripts register: declarations from the oracle's facts, as carry writes
/// them; their assets from the instance's `kubejs/assets`; the models KubeJS makes up when it has none.
fn kubejs_content(
    o: &Options,
    facts: &Facts,
    pack: &Path,
    sink: &mut Sink,
    known: &mut HashSet<String>,
    s: &mut Summary,
) -> Result<(), String> {
    let vanilla = crate::pipeline::vanilla_blocks(o.resources)?;
    let allowed = |b: &str| vanilla.contains(b);
    let prefix = format!("{PACK_ID}:");
    let has = |rel: String| pack.join(rel).exists();
    let decl = pack.join(format!("data/{PACK_ID}/horizon"));
    let mut declared = BTreeSet::new();
    for (full, block) in facts.blocks.range(prefix.clone()..) {
        let Some(name) = full.strip_prefix(&prefix) else { break };
        let blockstate = pack.join(format!("assets/{PACK_ID}/blockstates/{name}.json"));
        let state = fs::read(&blockstate).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok());
        let guess = state.as_ref().and_then(|v| crate::carry::block_template(name, v));
        let modelled = state.as_ref().map(crate::carry::properties).unwrap_or_default();
        let Some(choice) =
            facts.template(full, &allowed).or_else(|| facts.extended(full, guess, Some(&modelled), &allowed))
        else {
            continue;
        };
        let mut v = json!({"template": choice.template, "properties": choice.properties});
        for (key, value) in &choice.declared {
            v[key] = value.clone();
        }
        let has_item = block.item == *full;
        if !has_item {
            v["item"] = json!(false);
        } else {
            declared.insert(full.clone());
            known.insert(full.clone());
        }
        sink.json(&decl.join(format!("block/{name}.json")), &v)?;
        s.kubejs_blocks += 1;
        if state.is_none() {
            let id = format!("{PACK_ID}:block/{name}");
            sink.json(&blockstate, &json!({"variants": {"": {"model": id}}}))?;
            if !has(format!("assets/{PACK_ID}/models/block/{name}.json")) {
                sink.json(
                    &pack.join(format!("assets/{PACK_ID}/models/block/{name}.json")),
                    &json!({"parent": "minecraft:block/cube_all", "textures": {"all": id}}),
                )?;
            }
        }
        if has_item && !has(format!("assets/{PACK_ID}/models/item/{name}.json")) {
            sink.json(
                &pack.join(format!("assets/{PACK_ID}/models/item/{name}.json")),
                &json!({"parent": format!("{PACK_ID}:block/{name}")}),
            )?;
        }
    }
    for full in facts.items.range(prefix.clone()..).map(|(k, _)| k) {
        let Some(name) = full.strip_prefix(&prefix) else { break };
        if declared.contains(full) {
            continue;
        }
        let (v, _) = facts.item(full, &[PACK_ID])?;
        sink.json(&decl.join(format!("item/{name}.json")), &v)?;
        known.insert(full.clone());
        s.kubejs_items += 1;
        if !has(format!("assets/{PACK_ID}/models/item/{name}.json")) {
            sink.json(
                &pack.join(format!("assets/{PACK_ID}/models/item/{name}.json")),
                &json!({"parent": "minecraft:item/generated", "textures": {"layer0": format!("{PACK_ID}:item/{name}")}}),
            )?;
        }
    }
    // 26.3 finds item models through a definition per item.
    for full in declared.iter().chain(facts.items.range(prefix.clone()..).map(|(k, _)| k)) {
        let Some(name) = full.strip_prefix(&prefix) else { continue };
        let def = pack.join(format!("assets/{PACK_ID}/items/{name}.json"));
        if !def.exists() {
            sink.json(
                &def,
                &json!({"model": {"type": "minecraft:model", "model": format!("{PACK_ID}:item/{name}")}}),
            )?;
        }
    }
    // The names a startup script gives, which KubeJS turns into a lang file only on a client.
    let names = o.dump.join("names.json");
    if let Ok(bytes) = fs::read(&names) {
        let lang = pack.join(format!("assets/{PACK_ID}/lang/en_us.json"));
        let mut map: Map<String, Value> =
            fs::read(&lang).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let given: BTreeMap<String, String> = serde_json::from_slice(&bytes).unwrap_or_default();
        for (id, name) in given {
            let Some((ns, path)) = id.split_once(':').filter(|(ns, _)| *ns == PACK_ID) else {
                continue;
            };
            let path = path.replace('/', ".");
            let kind = if facts.blocks.contains_key(&id) { "block" } else { "item" };
            // A server without the name answers with the key.
            let key = format!("{kind}.{ns}.{path}");
            if name != key {
                map.entry(key).or_insert(json!(name));
            }
        }
        sink.json(&lang, &Value::Object(map))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags() -> HashMap<String, Vec<String>> {
        HashMap::from([("c:gems".to_owned(), vec!["m:ruby".to_owned(), "minecraft:diamond".to_owned()])])
    }

    #[test]
    fn ingredients_with_no_26_form_open_their_tags() {
        let t = tags();
        assert_eq!(ingredient_26(&json!({"item": "m:a"}), &t), Some(json!("m:a")));
        assert_eq!(ingredient_26(&json!({"tag": "c:gems"}), &t), Some(json!("#c:gems")));
        assert_eq!(
            ingredient_26(&json!([{"tag": "c:gems"}, {"item": "m:a"}, {"item": "m:ruby"}]), &t),
            Some(json!(["m:ruby", "minecraft:diamond", "m:a"]))
        );
        assert_eq!(
            ingredient_26(&json!({"type": "neoforge:compound", "children": [{"item": "m:a"}]}), &t),
            Some(json!("m:a"))
        );
        assert_eq!(
            ingredient_26(
                &json!({"type": "neoforge:difference", "base": {"tag": "c:gems"}, "subtracted": {"item": "m:ruby"}}),
                &t
            ),
            Some(json!("minecraft:diamond"))
        );
        assert_eq!(
            ingredient_26(
                &json!({"type": "neoforge:intersection", "children": [{"tag": "c:gems"}, {"item": "m:ruby"}]}),
                &t
            ),
            Some(json!("m:ruby"))
        );
        assert_eq!(ingredient_26(&json!({"type": "create:special"}), &t), None);
    }

    #[test]
    fn recipes_upgrade_as_a_jars_do() {
        let t = tags();
        let mut shaped = json!({"type": "kubejs:shaped", "pattern": ["#"], "key": {"#": [{"tag": "c:gems"}, {"item": "m:a"}]},
            "result": {"id": "m:b", "count": 1}});
        assert!(upgrade(&mut shaped, &t));
        assert_eq!(shaped["type"], "minecraft:crafting_shaped");
        assert_eq!(shaped["key"]["#"], json!(["m:ruby", "minecraft:diamond", "m:a"]));
        // A mod's type is its mod's to read.
        let mut custom = json!({"type": "m:press", "input": {"tag": "c:gems"}});
        assert!(!upgrade(&mut custom, &t));
        assert_eq!(custom["input"], json!({"tag": "c:gems"}));
        // An ingredient type vanilla can't read leaves the recipe as it was.
        let mut odd = json!({"type": "minecraft:smelting", "ingredient": {"type": "m:x"}});
        assert!(!upgrade(&mut odd, &t));
    }

    #[test]
    fn tag_folders_follow_the_registry() {
        assert_eq!(tag_dir("minecraft:item"), "item");
        assert_eq!(tag_dir("minecraft:worldgen/biome"), "worldgen/biome");
        assert_eq!(tag_dir("mekanism:chemical"), "mekanism/chemical");
    }

    /// The whole step, over two carries and AE2's beside them: where each file ends, and the game's fence takes it.
    #[test]
    fn final_data_lands_where_the_fence_allows() {
        let dir = std::env::temp_dir().join(format!("horizon-port-pack-final-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let (carries, dump, res, also) =
            (dir.join("carries"), dir.join("dump"), dir.join("res"), dir.join("ae2"));
        for d in [&carries, &dump, &res, &also] {
            fs::create_dir_all(d).unwrap();
        }
        fs::write(res.join("items.json"), r#"{"minecraft:stick": {}, "minecraft:diamond": {}}"#).unwrap();
        fs::create_dir_all(res.join("data/minecraft/recipe")).unwrap();
        fs::write(res.join("data/minecraft/recipe/stick.json"), "{}").unwrap();
        let manifest = |id: &str, more: &str| {
            format!("id = \"{id}\"\nname = \"{id}\"\nversion = \"1\"\napi = \"0.2\"\nside = \"both\"\ncapabilities = [\"registries\"]\n{more}")
        };
        let m = carries.join("m");
        fs::create_dir_all(m.join("data/m/horizon/item")).unwrap();
        fs::write(m.join("horizon-mod.toml"), manifest("m", "")).unwrap();
        fs::write(m.join("data/m/horizon/item/gem.json"), r#"{"template": "minecraft:stick"}"#).unwrap();
        // What the dump replaces, and what it leaves.
        fs::create_dir_all(m.join("data/m/recipe")).unwrap();
        fs::write(m.join("data/m/recipe/removed_by_a_script.json"), "{}").unwrap();
        fs::create_dir_all(m.join("data/m/tags/item")).unwrap();
        fs::write(m.join("data/m/tags/item/old.json"), r#"{"values": []}"#).unwrap();
        fs::create_dir_all(m.join("data/m/tags/worldgen/biome")).unwrap();
        fs::write(m.join("data/m/tags/worldgen/biome/keep.json"), r#"{"values": []}"#).unwrap();
        fs::create_dir_all(m.join("data/m/loot_table/blocks")).unwrap();
        fs::write(m.join("data/m/loot_table/blocks/code_drop.json"), "{}").unwrap();
        fs::write(also.join("horizon-mod.toml"), manifest("ae2", "")).unwrap();
        fs::create_dir_all(also.join("data/ae2/recipe")).unwrap();
        fs::write(also.join("data/ae2/recipe/own.json"), "{}").unwrap();
        let row = |id: &str, result: &str| {
            json!({"id": id, "json": {"type": "minecraft:crafting_shapeless", "ingredients": [{"item": "minecraft:diamond"}],
                "result": {"id": result, "count": 1}}})
            .to_string()
        };
        let recipes = [
            row("m:gem", "m:gem"),
            row("minecraft:stick", "minecraft:stick"),
            row("minecraft:added_by_a_script", "m:gem"),
            row("kubejs:made_up", "minecraft:stick"),
            row("ae2:own", "minecraft:stick"),
            row("ae2:more", "minecraft:stick"),
            row("powah:cable", "minecraft:stick"),
            row("m:for_powah", "powah:cable"),
            json!({"id": "m:broken", "failed": "no codec"}).to_string(),
        ];
        fs::write(dump.join("recipes.jsonl"), recipes.join("\n")).unwrap();
        let tag = |r: &str, id: &str| json!({"registry": r, "id": id, "values": ["minecraft:chain"]}).to_string();
        let tags = [
            tag("minecraft:item", "c:gems"),
            tag("minecraft:item", "m:gems"),
            tag("minecraft:item", "powah:gems"),
            tag("minecraft:block", "minecraft:logs"),
            tag("minecraft:worldgen/biome", "m:biomes"),
        ];
        fs::write(dump.join("tags.jsonl"), tags.join("\n")).unwrap();
        let loot = |id: &str| json!({"id": id, "json": {"type": "minecraft:block", "pools": []}}).to_string();
        let tables = [loot("m:blocks/gem"), loot("minecraft:blocks/stone"), loot("powah:blocks/cable"), loot("kubejs:blocks/x")];
        fs::write(dump.join("loot_tables.jsonl"), tables.join("\n")).unwrap();
        let inst = dir.join("inst");
        for (file, text) in [
            ("kubejs/assets/kubejs/textures/item/gem.png", "png"),
            ("kubejs/assets/kubejs/lang/en_us.json", r#"{"item.kubejs.gem": "Gem"}"#),
            ("kubejs/assets/m/lang/en_us.json", "{}"),
            ("kubejs/assets/powah/lang/en_us.json", "{}"),
            ("kubejs/assets/atm/lang/en_us.json", "{}"),
        ] {
            let path = inst.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        fs::write(res.join("facts.json"), r#"{"defaults": {"minecraft:stone": 1}}"#).unwrap();
        let block = |class: &str| {
            let shape = json!([[0, 0, 0, 1, 1, 1]]);
            json!({"class": [class], "properties": {}, "default": "", "item": "x", "explosion_resistance": 0.0,
                "friction": 0.6, "speed_factor": 1.0, "jump_factor": 1.0, "states": {"": {"hardness": 0.0,
                "emission": 0, "requires_tool": false, "map_color_id": 12, "sound": {"break": "s"},
                "outline": shape, "collision": shape}}})
        };
        let plain = "net.minecraft.world.level.block.Block";
        let mut essence = block(plain);
        essence["item"] = json!("kubejs:essence");
        fs::write(
            dump.join("facts.json"),
            json!({"blocks": {"minecraft:stone": block(plain), "kubejs:essence": essence},
                "items": {"kubejs:essence": {"class": "net.minecraft.world.item.BlockItem", "max_stack_size": 64,
                    "rarity": "common", "block": "kubejs:essence"},
                    "kubejs:gem": {"class": "net.minecraft.world.item.Item", "max_stack_size": 64, "rarity": "common"}}})
            .to_string(),
        )
        .unwrap();
        fs::write(dump.join("names.json"), r#"{"kubejs:essence": "Essence", "kubejs:gem": "item.kubejs.gem"}"#).unwrap();
        let mods = BTreeSet::from(["m".to_owned(), "powah".to_owned(), "ae2".to_owned(), "kubejs".to_owned()]);
        let s = apply(&Options {
            carries: &carries,
            dump: &dump,
            resources: &res,
            instance: Some(&inst),
            mods: &mods,
            also: std::slice::from_ref(&also),
        })
        .unwrap();
        assert_eq!((s.recipes_in_dump, s.recipes_vanilla, s.recipes_of_also, s.recipes_failed), (9, 1, 1, 1));
        assert_eq!((s.recipes_in_carries, s.recipes_in_pack), (1, 4));
        assert_eq!(s.recipes_unknown_result, BTreeMap::from([("powah".to_owned(), 1)]));
        assert_eq!(
            s.recipes_moved,
            BTreeMap::from([("ae2".to_owned(), 1), ("minecraft".to_owned(), 1), ("powah".to_owned(), 1)])
        );
        assert!(m.join("data/m/recipe/gem.json").exists());
        assert!(!m.join("data/m/recipe/removed_by_a_script.json").exists());
        assert!(!m.join("data/m/tags/item/old.json").exists());
        assert!(m.join("data/m/tags/item/gems.json").exists());
        assert!(m.join("data/m/tags/worldgen/biome/keep.json").exists());
        // A table the jar's carry made for a drop in code stays; the dump's join it.
        assert!(m.join("data/m/loot_table/blocks/code_drop.json").exists());
        assert!(m.join("data/m/loot_table/blocks/gem.json").exists());
        let pack = carries.join(PACK_FOLDER);
        assert!(pack.join("data/kubejs/recipe/made_up.json").exists());
        assert!(pack.join("data/atm10_pack/recipe/powah/cable.json").exists());
        assert!(pack.join("data/atm10_pack/recipe/ae2/more.json").exists());
        assert!(pack.join("data/atm10_pack/recipe/minecraft/added_by_a_script.json").exists());
        assert!(pack.join("data/c/tags/item/gems.json").exists());
        assert!(pack.join("data/minecraft/tags/block/logs.json").exists());
        assert!(pack.join("data/kubejs/loot_table/blocks/x.json").exists());
        assert_eq!(s.tags_left_out, BTreeMap::from([("powah".to_owned(), 1)]));
        assert_eq!(s.loot_left_out, BTreeMap::from([("powah".to_owned(), 1)]));
        assert_eq!(s.loot_vanilla, 1);
        assert_eq!(s.tags_worldgen_left, 1);
        // KubeJS's content: its ids, models where it made none up, assets where each namespace lives.
        assert_eq!((s.kubejs_blocks, s.kubejs_items, s.kubejs_assets, s.kubejs_assets_left_out), (1, 1, 4, 1));
        assert!(pack.join("data/kubejs/horizon/block/essence.json").exists());
        assert!(pack.join("data/kubejs/horizon/item/gem.json").exists());
        assert!(pack.join("assets/kubejs/items/gem.json").exists());
        assert!(pack.join("assets/kubejs/models/item/gem.json").exists());
        assert!(pack.join("assets/kubejs/blockstates/essence.json").exists());
        assert!(pack.join("assets/atm/lang/en_us.json").exists());
        assert!(m.join("assets/m/lang/en_us.json").exists());
        assert!(!pack.join("assets/powah").exists());
        let lang: Value =
            serde_json::from_slice(&fs::read(pack.join("assets/kubejs/lang/en_us.json")).unwrap()).unwrap();
        assert_eq!(lang, json!({"block.kubejs.essence": "Essence", "item.kubejs.gem": "Gem"}));
        // A renamed id comes out under its new name.
        let tag: Value =
            serde_json::from_slice(&fs::read(pack.join("data/c/tags/item/gems.json")).unwrap()).unwrap();
        assert_eq!(tag["values"], json!(["minecraft:iron_chain"]));
        // The game's own reader takes both, side by side.
        let manifest_pack = manifest_of(&pack).unwrap();
        assert_eq!(manifest_pack.id, "kubejs");
        let content = horizon_mods::Content::read(&pack, &manifest_pack).unwrap();
        let carry_manifest = manifest_of(&m).unwrap();
        let own = horizon_mods::Content::read(&m, &carry_manifest).unwrap();
        assert!(
            horizon_mods::content::clash(("kubejs", &content.namespaces), ("m", &own.namespaces)).is_none()
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
