//! The worldgen audit (docs/atm10-port.md, "Worldgen in the full pack"): what a
//! loaded world's generators make, counted by mod. For each dimension a fresh
//! generator of the world's data and seed (so the server's caches are left
//! alone) generates a square of chunks to FEATURES (`horizon_world::worldgen::
//! audit::sample`), and wider surveys count the noise biomes of a grid and the
//! structure starts of a larger square. The report adds the placed features
//! that can't run, with their reasons, and the content root's biome modifiers:
//! present, read, applied (`mod_content::biome_modifier_census`).
use horizon_core::position::ChunkPos;
use horizon_world::worldgen::audit;
use horizon_world::worldgen::generator::{GeneratorData, OverworldGenerator};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

/// How much to generate.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// The generated square: `2 * radius + 1` chunks on a side, around 0 0.
    pub radius: i32,
    pub threads: usize,
    /// The biome survey: points every `survey_step` blocks within
    /// `survey_radius` blocks.
    pub survey_radius: i32,
    pub survey_step: i32,
    /// The structure start survey: `2 * start_radius + 1` chunks on a side.
    pub start_radius: i32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            radius: 16,
            threads: 4,
            survey_radius: 8192,
            survey_step: 64,
            start_radius: 64,
        }
    }
}

/// The dimensions audited: vanilla's three.
pub const DIMENSIONS: [&str; 3] = ["minecraft:overworld", "minecraft:the_nether", "minecraft:the_end"];

/// `ns` of `ns:path`.
fn namespace(id: &str) -> &str {
    id.split_once(':').map_or("minecraft", |(ns, _)| ns)
}

/// Whether a block id names an ore (`*_ore`, `*_ore_*`, `ore_*`): the block
/// names mods and vanilla give their ores.
pub fn is_ore(id: &str) -> bool {
    let path = id.split_once(':').map_or(id, |(_, p)| p);
    path.ends_with("_ore") || path.contains("_ore_") || path.starts_with("ore_") || path == "ore"
}

fn by_namespace(counts: &BTreeMap<String, u64>) -> Map<String, Value> {
    let mut out: BTreeMap<&str, u64> = BTreeMap::new();
    for (id, n) in counts {
        *out.entry(namespace(id)).or_default() += n;
    }
    let mut list: Vec<_> = out.into_iter().collect();
    list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    list.into_iter().map(|(k, v)| (k.to_owned(), json!(v))).collect()
}

fn sorted(counts: &BTreeMap<String, u64>) -> Map<String, Value> {
    let mut list: Vec<_> = counts.iter().collect();
    list.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    list.into_iter().map(|(k, v)| (k.clone(), json!(v))).collect()
}

/// One dimension's counts.
fn dimension(
    data: &Arc<GeneratorData>,
    seed: i64,
    dim: &str,
    options: Options,
    added: &BTreeMap<String, String>,
) -> Value {
    let started = std::time::Instant::now();
    let generator = match OverworldGenerator::for_dimension(data.clone(), seed, "minecraft:normal", dim) {
        Ok(g) => g,
        Err(e) => return json!({ "error": e.to_string() }),
    };
    let unsupported = generator.unsupported_features();
    // Tiles of about 10 chunks on a side, evenly cut: a tile generates its ring of neighbours too.
    let side = 2 * options.radius + 1;
    let per_row = (side + 9) / 10;
    let tile = (side + per_row - 1) / per_row;
    let sample = audit::sample(&generator, ChunkPos::new(0, 0), options.radius, tile, options.threads);
    let sampled = started.elapsed().as_secs_f64();
    let ys: &[i32] = match dim {
        "minecraft:overworld" => &[-40, 0, 64, 120],
        _ => &[32, 64, 96],
    };
    let survey = audit::biome_survey(&generator, [0, 0], options.survey_radius, options.survey_step, ys);
    let starts = audit::start_survey(&generator, ChunkPos::new(0, 0), options.start_radius, options.threads);
    // Blocks: every mod's, and the ores of each.
    let mod_blocks: BTreeMap<String, u64> =
        sample.blocks.iter().filter(|(id, _)| namespace(id) != "minecraft").map(|(k, v)| (k.clone(), *v)).collect();
    let ores: BTreeMap<String, u64> =
        sample.blocks.iter().filter(|(id, _)| is_ore(id)).map(|(k, v)| (k.clone(), *v)).collect();
    // Placed-feature roots by namespace: features that ran, that placed, and their runs.
    let mut roots: BTreeMap<&str, [u64; 4]> = BTreeMap::new();
    for (id, [ran, placed]) in &sample.roots {
        let e = roots.entry(namespace(id)).or_default();
        e[0] += 1;
        e[1] += u64::from(*placed > 0);
        e[2] += ran;
        e[3] += placed;
    }
    let roots_by_ns: Map<String, Value> = roots
        .iter()
        .map(|(ns, [f, fp, r, p])| (ns.to_string(), json!({"features": f, "features_placing": fp, "runs": r, "runs_placing": p})))
        .collect();
    // The features biome modifiers add: did they run here, and place anything?
    let mut modifier_features = Map::new();
    for (feature, ns) in added {
        let counts = sample.roots.get(feature).copied().unwrap_or([0, 0]);
        let cant = unsupported.iter().find(|(n, _)| n == feature).map(|(_, r)| r.clone());
        if counts[0] > 0 || cant.is_some() {
            modifier_features.insert(feature.clone(), json!({"mod": ns, "runs": counts[0], "runs_placing": counts[1], "cannot_run": cant}));
        }
    }
    let mut cant_by_ns: BTreeMap<String, u64> = BTreeMap::new();
    let mut cant_reasons: BTreeMap<String, u64> = BTreeMap::new();
    for (name, reason) in &unsupported {
        *cant_by_ns.entry(namespace(name).to_owned()).or_default() += 1;
        let short: String = reason.chars().take(90).collect();
        *cant_reasons.entry(short).or_default() += 1;
    }
    let roots_list: Map<String, Value> =
        sample.roots.iter().map(|(k, [r, p])| (k.clone(), json!([r, p]))).collect();
    json!({
        "chunks": sample.chunks,
        "seconds": { "sample": sampled, "all": started.elapsed().as_secs_f64() },
        "failed_chunks": sample.failed.iter().take(20).map(|(p, e)| format!("{} {}: {e}", p.x, p.z)).collect::<Vec<_>>(),
        "failed_chunk_count": sample.failed.len(),
        "blocks_by_namespace": by_namespace(&sample.blocks),
        "mod_blocks": sorted(&mod_blocks),
        "ores": sorted(&ores),
        "ores_by_namespace": by_namespace(&ores),
        "biomes": sorted(&sample.biomes),
        "biomes_by_namespace": by_namespace(&sample.biomes),
        "survey_biomes": sorted(&survey),
        "survey_biomes_by_namespace": by_namespace(&survey),
        "features_by_namespace": roots_by_ns,
        "features": roots_list,
        "skipped_roots": sorted(&sample.skipped_roots),
        "modifier_features": modifier_features,
        "starts": sorted(&sample.starts),
        "survey_starts": sorted(&starts),
        "survey_starts_by_namespace": by_namespace(&starts),
        "cannot_run": {
            "total": unsupported.len(),
            "by_namespace": sorted(&cant_by_ns),
            "by_reason": sorted(&cant_reasons),
            "features": unsupported.iter().map(|(n, r)| (n.clone(), json!(r))).collect::<Map<_, _>>(),
        },
    })
}

/// The whole report for the world whose generator data and seed are given
/// and whose content root is `root`. `scan_dirs` are the carries (dev mod
/// directories) scanned for the raw `data/<ns>/neoforge/biome_modifier/`
/// files; pass `&[]` to scan `root/data/` instead (the unit test path).
pub fn audit(data: &Arc<GeneratorData>, seed: i64, root: &Path, scan_dirs: &[&Path], dims: &[&str], options: Options) -> Value {
    let started = std::time::Instant::now();
    let modifiers = crate::mod_content::biome_modifier_census(root, scan_dirs).unwrap_or_else(|e| json!({ "error": e.to_string() }));
    let added: BTreeMap<String, String> = modifiers["added_features"]
        .as_object()
        .map(|o| o.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_owned())).collect())
        .unwrap_or_default();
    let mut dimensions = Map::new();
    for dim in dims {
        dimensions.insert(dim.to_string(), dimension(data, seed, dim, options, &added));
    }
    let biome_ids = data.biome_names();
    json!({
        "seed": seed,
        "options": {
            "radius": options.radius, "threads": options.threads, "survey_radius": options.survey_radius,
            "survey_step": options.survey_step, "start_radius": options.start_radius,
        },
        "registered_biomes": biome_ids.len(),
        "registered_biomes_by_namespace": by_namespace(&biome_ids.iter().map(|b| (b.to_string(), 1)).collect()),
        "biome_modifiers": modifiers,
        "dimensions": dimensions,
        "seconds": started.elapsed().as_secs_f64(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ores_are_told_by_name() {
        for ore in ["alltheores:tin_ore", "alltheores:deepslate_tin_ore", "mekanism:osmium_ore", "minecraft:iron_ore", "x:ore_cluster_tin", "alltheores:other_ore_x"] {
            assert!(is_ore(ore), "{ore}");
        }
        for not in ["minecraft:stone", "alltheores:raw_tin_block", "x:core", "x:oreganos", "minecraft:redstone_wire"] {
            assert!(!is_ore(not), "{not}");
        }
    }
}
