//! What a generator makes, counted: the worldgen audit's sampling
//! (horizon-client's `worldgen_audit`, the game's `worldgen-audit` command).
//! A square of chunks is generated to FEATURES as the server's pipeline does
//! (each chunk decorated in its 3×3 region of proto chunks), off the world,
//! and its blocks, quart biomes, placed-feature roots and structure starts
//! are counted by id. Wider, cheaper surveys count the noise biomes of a
//! grid of points and the structure starts of a square of chunks.
use super::generator::{OverworldGenerator, ProtoChunk};
use horizon_core::position::ChunkPos;
use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The counts of a square of generated chunks.
#[derive(Debug, Default)]
pub struct Sample {
    /// Chunks counted (decorated, in the square).
    pub chunks: u64,
    /// Chunks that failed to generate or decorate, with the error.
    pub failed: Vec<(ChunkPos, String)>,
    /// Blocks by id, over every section of the counted chunks.
    pub blocks: BTreeMap<String, u64>,
    /// Quart biomes (4×4×4 cells) by id.
    pub biomes: BTreeMap<String, u64>,
    /// Placed-feature roots: runs, and runs that placed something.
    pub roots: BTreeMap<String, [u64; 2]>,
    /// Roots skipped because they can't run, and roots that failed.
    pub skipped_roots: BTreeMap<String, u64>,
    /// Structure starts made in the counted chunks, by structure.
    pub starts: BTreeMap<String, u64>,
}

impl Sample {
    fn merge(&mut self, other: Sample) {
        self.chunks += other.chunks;
        self.failed.extend(other.failed);
        for (k, v) in other.blocks {
            *self.blocks.entry(k).or_default() += v;
        }
        for (k, v) in other.biomes {
            *self.biomes.entry(k).or_default() += v;
        }
        for (k, [a, b]) in other.roots {
            let e = self.roots.entry(k).or_default();
            e[0] += a;
            e[1] += b;
        }
        for (k, v) in other.skipped_roots {
            *self.skipped_roots.entry(k).or_default() += v;
        }
        for (k, v) in other.starts {
            *self.starts.entry(k).or_default() += v;
        }
    }
}

/// The chunks `center ± radius` (a square of `2 * radius + 1` on a side),
/// generated in tiles of `tile` chunks on a side, a tile per thread at a
/// time. A tile generates its ring of neighbours too, so border chunks are
/// generated once per tile that touches them; only the tile's own chunks
/// are counted, once every chunk of the tile is decorated.
pub fn sample(
    generator: &OverworldGenerator,
    center: ChunkPos,
    radius: i32,
    tile: i32,
    threads: usize,
) -> Sample {
    let tile = tile.max(1);
    let (lo, hi) = (-radius, radius);
    let mut tiles = Vec::new();
    let mut z = lo;
    while z <= hi {
        let mut x = lo;
        while x <= hi {
            tiles.push((
                [center.x + x, center.z + z],
                [center.x + (x + tile - 1).min(hi), center.z + (z + tile - 1).min(hi)],
            ));
            x += tile;
        }
        z += tile;
    }
    let next = AtomicUsize::new(0);
    let total = Mutex::new(Sample::default());
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| {
                let mut worker = generator.worker();
                worker.set_count_features(true);
                let mut mine = Sample::default();
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&(from, to)) = tiles.get(i) else {
                        break;
                    };
                    sample_tile(&mut worker, from, to, &mut mine);
                    worker.release();
                }
                if let Ok(mut total) = total.lock() {
                    total.merge(mine);
                }
            });
        }
    });
    total.into_inner().unwrap_or_default()
}

fn sample_tile(
    worker: &mut super::generator::GeneratorWorker,
    from: [i32; 2],
    to: [i32; 2],
    out: &mut Sample,
) {
    let mut protos: HashMap<(i32, i32), ProtoChunk> = HashMap::new();
    for z in from[1] - 1..=to[1] + 1 {
        for x in from[0] - 1..=to[0] + 1 {
            match worker.generate_proto(ChunkPos::new(x, z)) {
                Ok(chunk) => {
                    protos.insert((x, z), chunk);
                }
                Err(e) => {
                    if (from[0]..=to[0]).contains(&x) && (from[1]..=to[1]).contains(&z) {
                        out.failed.push((ChunkPos::new(x, z), e.to_string()));
                    }
                }
            }
        }
    }
    let mut decorated = Vec::new();
    for z in from[1]..=to[1] {
        for x in from[0]..=to[0] {
            let keys: Vec<(i32, i32)> = (0..9).map(|i| (x + i % 3 - 1, z + i / 3 - 1)).collect();
            if keys.iter().any(|k| !protos.contains_key(k)) {
                continue;
            }
            let chunks: Vec<ProtoChunk> = keys.iter().filter_map(|k| protos.remove(k)).collect();
            let Ok(mut region) = <[ProtoChunk; 9]>::try_from(chunks) else {
                continue;
            };
            match worker.decorate(&mut region) {
                Ok(stats) => {
                    decorated.push((x, z));
                    for (name, [ran, placed]) in stats.roots {
                        let e = out.roots.entry(name).or_default();
                        e[0] += u64::from(ran);
                        e[1] += u64::from(placed);
                    }
                    for name in stats.skipped {
                        *out.skipped_roots.entry(name).or_default() += 1;
                    }
                }
                Err(e) => out.failed.push((ChunkPos::new(x, z), e.to_string())),
            }
            for (key, chunk) in keys.into_iter().zip(region) {
                protos.insert(key, chunk);
            }
            for start in worker.structure_starts(ChunkPos::new(x, z)) {
                if start.chunk == ChunkPos::new(x, z) {
                    *out.starts.entry(start.name.clone()).or_default() += 1;
                }
            }
        }
    }
    let mut states: HashMap<u32, u64> = HashMap::new();
    let mut biomes: HashMap<u32, u64> = HashMap::new();
    let mut buf = Box::new([0u32; 4096]);
    for key in decorated {
        let Some(chunk) = protos.get(&key) else {
            continue;
        };
        out.chunks += 1;
        let (min_y, height) = (chunk.terrain.min_y(), chunk.terrain.height());
        let mut y = min_y;
        while y < min_y + height {
            if chunk.terrain.section_states(y, &mut buf) {
                let mut run = (buf[0], 0u64);
                for &state in buf.iter() {
                    if state == run.0 {
                        run.1 += 1;
                    } else {
                        *states.entry(run.0).or_default() += run.1;
                        run = (state, 1);
                    }
                }
                *states.entry(run.0).or_default() += run.1;
            }
            for qz in 0..4 {
                for qx in 0..4 {
                    for qy in 0..4 {
                        let id = chunk.biome_id_at(qx * 4, y + qy * 4, qz * 4);
                        *biomes.entry(id.0).or_default() += 1;
                    }
                }
            }
            y += 16;
        }
    }
    let Some(any) = protos.values().next() else {
        return;
    };
    let data = any.data().clone();
    let registry = data.registry();
    for (state, n) in states {
        let name = registry
            .state(super::super::registry::BlockStateId(state))
            .map(|s| s.name.clone())
            .unwrap_or_else(|| format!("?state {state}"));
        *out.blocks.entry(name).or_default() += n;
    }
    for (id, n) in biomes {
        let name = data.biome_name(super::biome::BiomeId(id)).to_owned();
        *out.biomes.entry(name).or_default() += n;
    }
}

/// The noise biomes of a grid of points: every `step` blocks within
/// `radius` blocks of `center` (x and z), at each of `ys`.
pub fn biome_survey(
    generator: &OverworldGenerator,
    center: [i32; 2],
    radius: i32,
    step: i32,
    ys: &[i32],
) -> BTreeMap<String, u64> {
    let mut worker = generator.worker();
    let step = step.max(4);
    let mut out = BTreeMap::new();
    let mut z = -radius;
    while z <= radius {
        let mut quarts = Vec::new();
        let mut x = -radius;
        while x <= radius {
            for &y in ys {
                quarts.push([(center[0] + x) >> 2, y >> 2, (center[1] + z) >> 2]);
            }
            x += step;
        }
        for biome in worker.noise_biomes(&quarts) {
            *out.entry(biome.unwrap_or_else(|| "?".into())).or_default() += 1;
        }
        z += step;
    }
    out
}

/// The structure starts made in the chunks `center ± radius`, by structure,
/// on `threads` threads (no terrain is generated: placement checks only).
pub fn start_survey(
    generator: &OverworldGenerator,
    center: ChunkPos,
    radius: i32,
    threads: usize,
) -> BTreeMap<String, u64> {
    let side = (2 * radius + 1).max(1) as usize;
    let next = AtomicUsize::new(0);
    let total = Mutex::new(BTreeMap::<String, u64>::new());
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| {
                let mut worker = generator.worker();
                let mut mine = BTreeMap::<String, u64>::new();
                loop {
                    let row = next.fetch_add(1, Ordering::Relaxed);
                    if row >= side {
                        break;
                    }
                    let z = center.z - radius + row as i32;
                    for x in center.x - radius..=center.x + radius {
                        let at = ChunkPos::new(x, z);
                        for start in worker.structure_starts(at) {
                            if start.chunk == at {
                                *mine.entry(start.name.clone()).or_default() += 1;
                            }
                        }
                    }
                    worker.forget_structure_starts();
                    worker.release();
                }
                if let Ok(mut total) = total.lock() {
                    for (k, v) in mine {
                        *total.entry(k).or_default() += v;
                    }
                }
            });
        }
    });
    total.into_inner().unwrap_or_default()
}
