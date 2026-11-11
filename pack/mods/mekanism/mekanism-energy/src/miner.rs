//! Mekanism's Digital Miner (`TileEntityDigitalMiner`, `ThreadMinerSearch`, `MinerEnergyContainer`, the
//! `MinerFilter`s and `WildcardMatcher`), from Mekanism 10.7.19 (MIT, Copyright (c) 2017-2025 Aidan C. Brady).
//!
//! The miner looks at a `(2 radius + 1)` square around it from `min` to `max` (its search, one block at a time in
//! index order: x fastest, then z, then y) and keeps the blocks its filters take (or, inverse, those none takes) in
//! a fastutil `Long2ObjectOpenHashMap` of chunk to `BitSet`. Each mine walks that map in its own order (the null
//! key's chunk 0, 0 first, then the table's slots downwards) and takes the highest index of the first chunk that has
//! one: the top layer first, `z` and `x` downwards. It mines every `ticksPerMine` ticks (80, fewer with speed
//! upgrades), the first on the tick after the search, and uses its Joules every tick it runs:
//! `ceil(usage * 16^(2 speed/8 - energy/8) [* silkMultiplier] * (1 + radius cost) * (1 + height cost))`.

use crate::machine::{Upgrade, Upgrades};
use std::collections::BTreeSet;

pub const DEFAULT_RADIUS: i32 = 10;
pub const DEFAULT_HEIGHT_RANGE: i32 = 60;
/// Its 27 main slots, then the energy slot and the upgrade component's two.
pub const MAIN_SLOTS: usize = 27;
pub const ENERGY_SLOT: usize = 27;
pub const UPGRADE_SLOT: usize = 28;
pub const UPGRADE_OUT_SLOT: usize = 29;
pub const SLOTS: usize = 30;
/// Ticks between two ejections after one (`delayTicks = 10`): one every 11.
pub const EJECT_DELAY: u32 = 10;
/// `withSupportedUpgrades`.
pub const UPGRADES: [Upgrade; 4] = [Upgrade::Speed, Upgrade::Energy, Upgrade::Anchor, Upgrade::StoneGenerator];

/// `general.toml` `[digital_miner]` and the miner's `machine-usage.toml` and `machine-storage.toml` keys.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MinerConfig {
    /// `silkMultiplier`, more than 1 (an int).
    pub silk_multiplier: i64,
    /// `maxRadius`, more than 1.
    pub max_radius: i32,
    /// `ticksPerMine`, more than 1.
    pub ticks_per_mine: u32,
    /// `digitalMiner`: Joules a tick and the store.
    pub usage: u64,
    pub storage: u64,
}

impl Default for MinerConfig {
    fn default() -> Self {
        Self { silk_multiplier: 12, max_radius: 32, ticks_per_mine: 80, usage: 1000, storage: 50_000 }
    }
}

impl MinerConfig {
    pub fn from_tables(general: Option<&toml::Table>, usage: Option<&toml::Table>, storage: Option<&toml::Table>) -> Self {
        let mut config = Self::default();
        let int = |t: Option<&toml::Table>, key: &str| t.and_then(|t| t.get(key)).and_then(toml::Value::as_integer);
        let miner = general.and_then(|g| g.get("digital_miner")).and_then(toml::Value::as_table);
        if let Some(v) = int(miner, "silkMultiplier").filter(|v| *v >= 1 && *v <= i64::from(i32::MAX)) {
            config.silk_multiplier = v;
        }
        if let Some(v) = int(miner, "maxRadius").filter(|v| (1..=i64::from(i32::MAX)).contains(v)) {
            config.max_radius = v as i32;
        }
        if let Some(v) = int(miner, "ticksPerMine").filter(|v| (1..=i64::from(i32::MAX)).contains(v)) {
            config.ticks_per_mine = v as u32;
        }
        if let Some(v) = int(usage, "digitalMiner").filter(|v| *v >= 0) {
            config.usage = v as u64;
        }
        if let Some(v) = int(storage, "digitalMiner").filter(|v| *v >= 1) {
            config.storage = v as u64;
        }
        config
    }

    /// `MinerEnergyContainer.updateMinerEnergyPerTick`; `height` is the level's (`getHeight`, 384 in the Overworld).
    pub fn energy_per_tick(&self, upgrades: &Upgrades, multiplier: f64, settings: &Settings, height: i32) -> u64 {
        let mut base = upgrades.energy_per_tick(self.usage, multiplier) as i64;
        if settings.silk_touch {
            base = base.checked_mul(self.silk_multiplier).unwrap_or(i64::MAX);
        }
        let radius_range = f64::from(self.max_radius - 10);
        let height_range = f64::from(height - 1 - 60);
        let radius_cost = if radius_range == 0.0 { 0.0 } else { (f64::from(settings.radius - 10) / radius_range).max(0.0) };
        let height_cost =
            if height_range == 0.0 { 0.0 } else { (f64::from(settings.max_y - settings.min_y - 60) / height_range).max(0.0) };
        crate::clamp_to_long((base as f64 * (1.0 + radius_cost) * (1.0 + height_cost)).ceil()).max(0) as u64
    }

    /// `delayLength`: `MekanismUtils.getTicks(ticksPerMine)`.
    pub fn ticks(&self, upgrades: &Upgrades, multiplier: f64) -> u32 {
        upgrades.ticks(self.ticks_per_mine, multiplier)
    }
}

/// `WildcardMatcher.matches`: `*` any run, `?` any one character, `#` a digit; case-insensitive.
pub fn wildcard_matches(wildcard: &str, text: &str) -> bool {
    let w: Vec<char> = wildcard.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    matches_from(&w, &t, 0, 0, false)
}

fn matches_from(w: &[char], t: &[char], start: usize, mut ti: usize, cont: bool) -> bool {
    let mut wi = start;
    while wi < w.len() {
        let wc = w[wi];
        if wc == '*' {
            return matches_from(w, t, wi + 1, ti, true);
        }
        if ti >= t.len() {
            return false;
        }
        let fail = if wc == '#' { !t[ti].is_ascii_digit() } else { wc != '?' && wc != t[ti] };
        if fail {
            return cont && matches_from(w, t, start, ti + 1, true);
        }
        ti += 1;
        wi += 1;
    }
    ti >= t.len() || w.last() == Some(&'*')
}

/// Whether a wildcard has anything but plain characters (a tag filter of one can only be matched against a list).
pub fn is_pattern(s: &str) -> bool {
    s.contains(['*', '?', '#'])
}

/// A filter's target (`FilterType.MINER_*`): `target_stack`, `tag`, `modid`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilterKind {
    Item(String),
    Tag(String),
    ModId(String),
}

impl FilterKind {
    /// Its saved `type`.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Item(_) => "miner_itemstack_filter",
            Self::Tag(_) => "miner_tag_filter",
            Self::ModId(_) => "miner_modid_filter",
        }
    }
}

/// `MinerFilter`: its target, `enabled`, `replace_target` (`None` for air) and `requires_replacement`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    pub kind: FilterKind,
    pub enabled: bool,
    pub replace_target: Option<String>,
    pub requires_replacement: bool,
}

impl Filter {
    pub fn new(kind: FilterKind) -> Self {
        Self { kind, enabled: true, replace_target: None, requires_replacement: false }
    }

    /// `replaceTargetMatches`.
    pub fn replace_target_matches(&self, item: &str) -> bool {
        self.replace_target.as_deref() == Some(item)
    }

    /// `canFilter(state)`: the block's item is the target, a tag of it matches, its namespace matches.
    pub fn can_filter(&self, block: &str, world: &mut impl MinerWorld) -> bool {
        match &self.kind {
            FilterKind::Item(target) => world.item_of(block).is_some_and(|item| item == *target),
            FilterKind::Tag(tag) => world.block_tag_matches(block, tag),
            FilterKind::ModId(id) => wildcard_matches(id, block.split_once(':').map_or("minecraft", |(ns, _)| ns)),
        }
    }
}

/// What a filter's block reads in the world (`BlockState` through `MinerRegionCache` and the level).
pub trait MinerWorld {
    /// The block at `pos` as its search sees it: `None` where the chunk isn't loaded.
    fn block(&mut self, pos: [i32; 3]) -> Option<String>;
    /// Whether the search leaves it out though it isn't air: a bounding block, `#mekanism:miner_blacklist`, a bed's
    /// foot or a door's or double plant's upper half, unbreakable (`getDestroySpeed < 0`) or a liquid.
    fn skipped(&mut self, pos: [i32; 3], block: &str) -> bool;
    /// `#mekanism:miner_blacklist`, which the mining step looks at again.
    fn blacklisted(&mut self, block: &str) -> bool;
    /// `block.asItem()`'s id, `None` for one without an item.
    fn item_of(&self, block: &str) -> Option<String>;
    /// A tag filter: whether one of the block's tags matches `tag` (`WildcardMatcher`).
    fn block_tag_matches(&mut self, block: &str, tag: &str) -> bool;
}

/// `ThreadMinerSearch.State`, by ordinal (its saved `state`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchState {
    #[default]
    Idle,
    Searching,
    Paused,
    Finished,
}

impl SearchState {
    pub const ALL: [Self; 4] = [Self::Idle, Self::Searching, Self::Paused, Self::Finished];

    pub fn by_id(id: i32) -> Self {
        Self::ALL[id.rem_euclid(4) as usize]
    }

    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// Its line on the screen (`MekanismLang.MINER_*`).
    pub fn name(self) -> &'static str {
        match self {
            Self::Idle => "Not ready",
            Self::Searching => "Searching...",
            Self::Paused => "Paused",
            Self::Finished => "Ready",
        }
    }
}

/// The miner's settings (`writeSustainedData`'s, less the filters) and its run state.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub radius: i32,
    pub min_y: i32,
    pub max_y: i32,
    pub silk_touch: bool,
    pub inverse: bool,
    pub inverse_requires_replacement: bool,
    pub inverse_replace_target: Option<String>,
    pub eject: bool,
    pub pull: bool,
    pub filters: Vec<Filter>,
    pub running: bool,
    pub delay: u32,
    pub state: SearchState,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            radius: DEFAULT_RADIUS,
            min_y: 0,
            max_y: DEFAULT_HEIGHT_RANGE,
            silk_touch: false,
            inverse: false,
            inverse_requires_replacement: false,
            inverse_replace_target: None,
            eject: false,
            pull: false,
            filters: Vec::new(),
            running: false,
            delay: 0,
            state: SearchState::Idle,
        }
    }
}

impl Settings {
    pub fn diameter(&self) -> i32 {
        self.radius * 2 + 1
    }

    /// `getTotalSize`.
    pub fn total_size(&self) -> i64 {
        let d = i64::from(self.diameter());
        d * d * i64::from(self.max_y - self.min_y + 1)
    }

    /// `getStartingPos`.
    pub fn start(&self, miner: [i32; 3]) -> [i32; 3] {
        [miner[0] - self.radius, self.min_y, miner[2] - self.radius]
    }

    /// `getOffsetForIndex`.
    pub fn offset(start: [i32; 3], diameter: i32, index: u32) -> [i32; 3] {
        let i = index as i32;
        [start[0] + i % diameter, start[1] + i / diameter / diameter, start[2] + i / diameter % diameter]
    }

    /// `isReplaceTarget`: what the search never takes and external automation may put in.
    pub fn is_replace_target(&self, item: &str) -> bool {
        if self.inverse {
            self.inverse_replace_target.as_deref() == Some(item)
        } else {
            self.filters.iter().any(|f| f.enabled && f.replace_target_matches(item))
        }
    }

    pub fn has_enabled_filters(&self) -> bool {
        self.filters.iter().any(|f| f.enabled)
    }

    /// The first enabled filter that takes `block`.
    pub fn matching_filter(&self, block: &str, world: &mut impl MinerWorld) -> Option<usize> {
        self.filters.iter().position(|f| f.enabled && f.can_filter(block, world))
    }

    /// The replace target and whether it must be there for a block a filter (or none, inverse) took.
    pub fn replace_for(&self, filter: Option<usize>) -> (Option<&str>, bool) {
        match filter.and_then(|i| self.filters.get(i)) {
            Some(f) => (f.replace_target.as_deref(), f.requires_replacement),
            None => (self.inverse_replace_target.as_deref(), self.inverse_requires_replacement),
        }
    }

    /// `ThreadMinerSearch.run`: the blocks to mine and how many.
    pub fn search(&self, miner: [i32; 3], world: &mut impl MinerWorld) -> (ChunkMap, i32) {
        let mut map = ChunkMap::new();
        if !self.inverse && !self.has_enabled_filters() {
            return (map, 0);
        }
        let start = self.start(miner);
        let diameter = self.diameter();
        let size = self.total_size().clamp(0, i64::from(u32::MAX)) as u32;
        let mut accepted: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
        let mut found = 0;
        for i in 0..size {
            let pos = Self::offset(start, diameter, i);
            if pos == miner {
                continue;
            }
            let Some(block) = world.block(pos) else { continue };
            if is_air(&block) || world.skipped(pos, &block) {
                continue;
            }
            let take = match accepted.get(&block) {
                Some(v) => *v,
                None => {
                    let v = if world.item_of(&block).is_some_and(|item| self.is_replace_target(&item)) {
                        false
                    } else {
                        self.inverse != self.matching_filter(&block, world).is_some()
                    };
                    accepted.insert(block.clone(), v);
                    v
                }
            };
            if take {
                map.get_or_insert(chunk_key(pos)).insert(i);
                found += 1;
            }
        }
        (map, found)
    }
}

pub fn is_air(block: &str) -> bool {
    matches!(block, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
}

/// `ChunkPos.asLong` of a block position.
pub fn chunk_key(pos: [i32; 3]) -> i64 {
    let (x, z) = (pos[0] >> 4, pos[2] >> 4);
    (i64::from(x) & 0xFFFF_FFFF) | ((i64::from(z) & 0xFFFF_FFFF) << 32)
}

pub fn chunk_of_key(key: i64) -> [i32; 2] {
    [key as i32, (key >> 32) as i32]
}

// ---- fastutil's Long2ObjectOpenHashMap<BitSet> -------------------------------------------------

/// `HashCommon.mix(long)`.
fn mix(x: i64) -> i64 {
    let h = x.wrapping_mul(0x9E37_79B9_7F4A_7C15_u64 as i64);
    let h = h ^ ((h as u64) >> 32) as i64;
    h ^ ((h as u64) >> 16) as i64
}

const LOAD: f32 = 0.75;

/// `HashCommon.arraySize(expected, f)`.
fn array_size(expected: usize, f: f32) -> usize {
    let s = f64::from(expected as f32 / f).ceil() as u64;
    s.max(2).next_power_of_two() as usize
}

/// `HashCommon.maxFill(n, f)`.
fn max_fill(n: usize, f: f32) -> usize {
    (f64::from(n as f32 * f).ceil() as usize).min(n - 1)
}

/// The miner's chunks to mine, laid out as Java's `Long2ObjectOpenHashMap` (default size 16, load 0.75) lays
/// them, so that it is walked in the same order: the null key (chunk 0, 0) first, then the slots downwards.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkMap {
    key: Vec<i64>,
    value: Vec<BTreeSet<u32>>,
    n: usize,
    mask: usize,
    max_fill: usize,
    size: usize,
    min_n: usize,
    null: bool,
}

impl Default for ChunkMap {
    fn default() -> Self {
        Self::new()
    }
}

impl ChunkMap {
    pub fn new() -> Self {
        let n = array_size(16, LOAD);
        Self { key: vec![0; n + 1], value: vec![BTreeSet::new(); n + 1], n, mask: n - 1, max_fill: max_fill(n, LOAD), size: 0, min_n: n, null: false }
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    pub fn len(&self) -> usize {
        self.size
    }

    fn slot(&self, k: i64) -> usize {
        (mix(k) as i32 as usize) & self.mask
    }

    /// `find`: the slot holding `k`, or `Err(free slot)`.
    fn find(&self, k: i64) -> Result<usize, usize> {
        if k == 0 {
            return if self.null { Ok(self.n) } else { Err(self.n) };
        }
        let mut pos = self.slot(k);
        loop {
            let curr = self.key[pos];
            if curr == 0 {
                return Err(pos);
            }
            if curr == k {
                return Ok(pos);
            }
            pos = (pos + 1) & self.mask;
        }
    }

    /// `computeIfAbsent(k, new BitSet())`.
    pub fn get_or_insert(&mut self, k: i64) -> &mut BTreeSet<u32> {
        let pos = match self.find(k) {
            Ok(p) => p,
            Err(p) => {
                if p == self.n {
                    self.null = true;
                }
                self.key[p] = k;
                self.value[p] = BTreeSet::new();
                let full = self.size >= self.max_fill;
                self.size += 1;
                if full {
                    self.rehash(array_size(self.size + 1, LOAD));
                    self.find(k).unwrap_or(self.n)
                } else {
                    p
                }
            }
        };
        &mut self.value[pos]
    }

    fn rehash(&mut self, new_n: usize) {
        let mask = new_n - 1;
        let mut key = vec![0; new_n + 1];
        let mut value = vec![BTreeSet::new(); new_n + 1];
        let mut i = self.n;
        let real = if self.null { self.size - 1 } else { self.size };
        for _ in 0..real {
            loop {
                i -= 1;
                if self.key[i] != 0 {
                    break;
                }
            }
            let mut pos = (mix(self.key[i]) as i32 as usize) & mask;
            while key[pos] != 0 {
                pos = (pos + 1) & mask;
            }
            key[pos] = self.key[i];
            value[pos] = std::mem::take(&mut self.value[i]);
        }
        value[new_n] = std::mem::take(&mut self.value[self.n]);
        self.n = new_n;
        self.mask = mask;
        self.max_fill = max_fill(new_n, LOAD);
        self.key = key;
        self.value = value;
    }

    /// `shiftKeys(pos)`; with `wrapped`, an iterator's (the entries it moves past its position, to visit later).
    fn shift_keys(&mut self, mut pos: usize, mut wrapped: Option<&mut Vec<i64>>) {
        loop {
            let last = pos;
            pos = (last + 1) & self.mask;
            let curr = loop {
                let curr = self.key[pos];
                if curr == 0 {
                    self.key[last] = 0;
                    self.value[last] = BTreeSet::new();
                    return;
                }
                let slot = self.slot(curr);
                if if last <= pos { last >= slot || slot > pos } else { last >= slot && slot > pos } {
                    break curr;
                }
                pos = (pos + 1) & self.mask;
            };
            if pos < last
                && let Some(w) = wrapped.as_deref_mut()
            {
                w.push(curr);
            }
            self.key[last] = curr;
            self.value[last] = std::mem::take(&mut self.value[pos]);
        }
    }

    /// `remove(k)` (with its shrink).
    fn remove_key(&mut self, k: i64) {
        let Ok(pos) = self.find(k) else { return };
        if pos == self.n {
            self.null = false;
            self.value[pos] = BTreeSet::new();
        } else {
            self.value[pos] = BTreeSet::new();
            self.shift_keys(pos, None);
        }
        self.size -= 1;
        if self.n > self.min_n && self.size < self.max_fill / 4 && self.n > 16 {
            self.rehash(self.n / 2);
        }
    }
}

/// `MapIterator` over a `ChunkMap`.
struct Walk {
    pos: isize,
    last: isize,
    left: usize,
    null: bool,
    wrapped: Vec<i64>,
}

impl Walk {
    fn new(map: &ChunkMap) -> Self {
        Self { pos: map.n as isize, last: -1, left: map.size, null: map.null, wrapped: Vec::new() }
    }

    /// `nextEntry`: the slot, and whether it came from the wrapped list.
    fn next(&mut self, map: &ChunkMap) -> Option<(usize, bool)> {
        if self.left == 0 {
            return None;
        }
        self.left -= 1;
        if self.null {
            self.null = false;
            self.last = map.n as isize;
            return Some((map.n, false));
        }
        loop {
            self.pos -= 1;
            if self.pos < 0 {
                self.last = isize::MIN;
                let k = *self.wrapped.get((-self.pos - 1) as usize)?;
                let mut p = map.slot(k);
                while map.key[p] != k {
                    p = (p + 1) & map.mask;
                }
                return Some((p, true));
            }
            if map.key[self.pos as usize] != 0 {
                self.last = self.pos;
                return Some((self.pos as usize, false));
            }
        }
    }

    /// `remove` of the entry last returned.
    fn remove(&mut self, map: &mut ChunkMap) {
        if self.last == map.n as isize {
            map.null = false;
            map.value[map.n] = BTreeSet::new();
        } else if self.pos >= 0 {
            map.value[self.last as usize] = BTreeSet::new();
            map.shift_keys(self.last as usize, Some(&mut self.wrapped));
        } else {
            let k = self.wrapped[(-self.pos - 1) as usize];
            map.remove_key(k);
            self.last = -1;
            return;
        }
        map.size -= 1;
        self.last = -1;
    }
}

/// A block `tryMineBlock` took: where, its filter, and where it sits in the map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub pos: [i32; 3],
    pub block: String,
    pub filter: Option<usize>,
    index: u32,
    slot: usize,
    wrapped: bool,
}

/// What the miner keeps between ticks and doesn't save: its blocks to mine, `cachedToMine`, the chunk it works in.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Queue {
    pub map: ChunkMap,
    pub to_mine: i32,
    pub target_chunk: Option<i64>,
}

impl Queue {
    /// `tryMineBlock` up to the block it would mine: the blocks it passes (air now, blacklisted, unloaded, no longer
    /// taken by the filters) leave the map; `None` when none is left.
    pub fn next(&mut self, settings: &Settings, miner: [i32; 3], world: &mut impl MinerWorld) -> Option<Found> {
        let start = settings.start(miner);
        let diameter = settings.diameter();
        let mut target = self.target_chunk;
        let mut walk = Walk::new(&self.map);
        while let Some((slot, wrapped)) = walk.next(&self.map) {
            let chunk = if slot == self.map.n { 0 } else { self.map.key[slot] };
            let mut current = target == Some(chunk);
            let mut previous = self.map.value[slot].last().copied();
            loop {
                let Some(index) = previous.and_then(|p| self.map.value[slot].range(..=p).next_back().copied()) else {
                    walk.remove(&mut self.map);
                    break;
                };
                if !current {
                    current = true;
                    target = Some(chunk);
                    self.target_chunk = target;
                }
                let pos = Settings::offset(start, diameter, index);
                if let Some(block) = world.block(pos)
                    && !is_air(&block)
                    && !world.blacklisted(&block)
                {
                    let filter = settings.matching_filter(&block, world);
                    if settings.inverse == filter.is_none() {
                        return Some(Found { pos, block, filter, index, slot, wrapped });
                    }
                }
                self.to_mine -= 1;
                self.map.value[slot].remove(&index);
                if self.map.value[slot].is_empty() {
                    walk.remove(&mut self.map);
                    break;
                }
                previous = index.checked_sub(1);
            }
        }
        self.target_chunk = None;
        None
    }

    /// The block `next` found was mined (or replaced): it leaves the map.
    pub fn mined(&mut self, found: &Found) {
        self.to_mine -= 1;
        let slot = found.slot;
        self.map.value[slot].remove(&found.index);
        if !self.map.value[slot].is_empty() {
            return;
        }
        if slot == self.map.n {
            self.map.null = false;
            self.map.size -= 1;
        } else if found.wrapped {
            let k = self.map.key[slot];
            self.map.remove_key(k);
        } else {
            self.map.shift_keys(slot, None);
            self.map.size -= 1;
        }
        self.target_chunk = None;
    }

    /// `getChunkSet`: the miner's chunk and the one it works in, if that is within its radius.
    pub fn chunks(&self, settings: &Settings, miner: [i32; 3]) -> Vec<[i32; 2]> {
        let own = [miner[0] >> 4, miner[2] >> 4];
        let Some(t) = self.target_chunk.map(chunk_of_key) else { return vec![own] };
        let r = settings.radius;
        let inside = (miner[0] - r) >> 4 <= t[0] && t[0] <= (miner[0] + r) >> 4 && (miner[2] - r) >> 4 <= t[1] && t[1] <= (miner[2] + r) >> 4;
        if !inside || t == own { vec![own] } else { vec![own, t] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Flat {
        blocks: std::collections::HashMap<[i32; 3], &'static str>,
    }

    impl MinerWorld for Flat {
        fn block(&mut self, pos: [i32; 3]) -> Option<String> {
            Some(self.blocks.get(&pos).copied().unwrap_or("minecraft:air").to_owned())
        }
        fn skipped(&mut self, _pos: [i32; 3], block: &str) -> bool {
            block == "minecraft:bedrock"
        }
        fn blacklisted(&mut self, _block: &str) -> bool {
            false
        }
        fn item_of(&self, block: &str) -> Option<String> {
            Some(block.to_owned())
        }
        fn block_tag_matches(&mut self, block: &str, tag: &str) -> bool {
            tag == "c:ores" && block.ends_with("_ore")
        }
    }

    #[test]
    fn energy_per_tick_is_the_oracles() {
        let c = MinerConfig::default();
        let s = Settings { radius: 2, min_y: -58, max_y: -56, ..Settings::default() };
        let none = Upgrades::default();
        let at = |s: &Settings, u: &Upgrades| c.energy_per_tick(u, 16.0, s, 384);
        assert_eq!(at(&s, &none), 1000);
        assert_eq!(at(&Settings { silk_touch: true, ..s.clone() }, &none), 12000);
        assert_eq!(at(&Settings { radius: 32, ..s.clone() }, &none), 2000);
        assert_eq!(at(&Settings { radius: 20, min_y: -64, max_y: 319, ..s.clone() }, &none), 2910);
        assert_eq!(at(&Settings { min_y: -60, max_y: 59, ..s.clone() }, &none), 1183);
        let mut u = Upgrades::default();
        u.set(Upgrade::Speed, 2);
        assert_eq!(at(&s, &u), 4000);
        let mut u = Upgrades::default();
        u.set(Upgrade::Energy, 2);
        assert_eq!(at(&s, &u), 500);
        assert_eq!(u.max_energy(c.storage, 16.0), 100_000);
        let mut u = Upgrades::default();
        u.set(Upgrade::Speed, 8);
        u.set(Upgrade::Energy, 8);
        assert_eq!(at(&s, &u), 16000);
        assert_eq!(c.ticks(&u, 16.0), 5);
        let mut u = Upgrades::default();
        u.set(Upgrade::Speed, 4);
        u.set(Upgrade::Energy, 8);
        assert_eq!(at(&Settings { radius: 32, silk_touch: true, ..s.clone() }, &u), 24000);
    }

    #[test]
    fn wildcards() {
        assert!(wildcard_matches("mine*", "minecraft"));
        assert!(wildcard_matches("c:ores/*", "c:ores/iron"));
        assert!(!wildcard_matches("c:ores/*", "c:ores"));
        assert!(wildcard_matches("*:stone", "minecraft:stone"));
        assert!(wildcard_matches("ore?", "ores"));
        assert!(wildcard_matches("tier#", "tier3"));
        assert!(!wildcard_matches("tier#", "tierx"));
        assert!(wildcard_matches("MINECRAFT", "minecraft"));
    }

    /// The oracle's order (m1 `order`): a 5x5x3 area at x, z 3006..3010 around a miner at 3008, -40, 3008, a stone
    /// filter, air at three places and dirt at two.
    #[test]
    fn order_over_four_chunks_is_the_oracles() {
        let (x, z) = (3008, 3008);
        let mut blocks = std::collections::HashMap::new();
        for y in -58..=-56 {
            for dz in -2..=2 {
                for dx in -2..=2 {
                    blocks.insert([x + dx, y, z + dz], "minecraft:stone");
                }
            }
        }
        for p in [[x - 2, -56, z - 2], [x, -57, z], [x + 1, -58, z + 2]] {
            blocks.remove(&p);
        }
        for p in [[x + 2, -56, z + 2], [x - 1, -57, z + 1]] {
            blocks.insert(p, "minecraft:dirt");
        }
        let mut world = Flat { blocks };
        let settings = Settings {
            radius: 2,
            min_y: -58,
            max_y: -56,
            filters: vec![Filter::new(FilterKind::Item("minecraft:stone".into()))],
            ..Settings::default()
        };
        let miner = [x, -40, z];
        let (map, found) = settings.search(miner, &mut world);
        assert_eq!(found, 70);
        let mut queue = Queue { map, to_mine: found, target_chunk: None };
        let mut order = Vec::new();
        while let Some(f) = queue.next(&settings, miner, &mut world) {
            world.blocks.remove(&f.pos);
            queue.mined(&f);
            order.push([f.pos[0] - x, f.pos[1], f.pos[2] - z]);
        }
        // m1's order: a block on tick 2 and every 5 ticks after, chunk (187, 188) first, then (188, 188), (188, 187), (187, 187).
        let oracle: [[i32; 3]; 70] = [[-1, -56, 2], [-2, -56, 2], [-1, -56, 1], [-2, -56, 1], [-1, -56, 0], [-2, -56, 0], [-1, -57, 2], [-2, -57, 2], [-2, -57, 1], [-1, -57, 0], [-2, -57, 0], [-1, -58, 2], [-2, -58, 2], [-1, -58, 1], [-2, -58, 1], [-1, -58, 0], [-2, -58, 0], [1, -56, 2], [0, -56, 2], [2, -56, 1], [1, -56, 1], [0, -56, 1], [2, -56, 0], [1, -56, 0], [0, -56, 0], [2, -57, 2], [1, -57, 2], [0, -57, 2], [2, -57, 1], [1, -57, 1], [0, -57, 1], [2, -57, 0], [1, -57, 0], [2, -58, 2], [0, -58, 2], [2, -58, 1], [1, -58, 1], [0, -58, 1], [2, -58, 0], [1, -58, 0], [0, -58, 0], [2, -56, -1], [1, -56, -1], [0, -56, -1], [2, -56, -2], [1, -56, -2], [0, -56, -2], [2, -57, -1], [1, -57, -1], [0, -57, -1], [2, -57, -2], [1, -57, -2], [0, -57, -2], [2, -58, -1], [1, -58, -1], [0, -58, -1], [2, -58, -2], [1, -58, -2], [0, -58, -2], [-1, -56, -1], [-2, -56, -1], [-1, -56, -2], [-1, -57, -1], [-2, -57, -1], [-1, -57, -2], [-2, -57, -2], [-1, -58, -1], [-2, -58, -1], [-1, -58, -2], [-2, -58, -2]];
        assert_eq!(order, oracle);
        assert_eq!(queue.to_mine, 0);
        assert!(queue.map.is_empty());
    }

    #[test]
    fn many_chunks_grow_and_shrink_the_table() {
        let mut map = ChunkMap::new();
        for cx in -3..3 {
            for cz in -3..3 {
                map.get_or_insert(chunk_key([cx * 16, 0, cz * 16])).insert(1);
            }
        }
        assert_eq!(map.len(), 36);
        assert_eq!(map.n, 64);
        let mut walk = Walk::new(&map);
        let mut seen = 0;
        while walk.next(&map).is_some() {
            walk.remove(&mut map);
            seen += 1;
        }
        assert_eq!(seen, 36);
        assert!(map.is_empty());
    }

    #[test]
    fn inverse_leaves_the_filtered_and_the_replace_target() {
        let mut blocks = std::collections::HashMap::new();
        for (i, b) in ["minecraft:stone", "minecraft:dirt", "minecraft:oak_planks", "minecraft:bedrock"].into_iter().enumerate() {
            blocks.insert([i as i32, 0, 0], b);
        }
        let mut world = Flat { blocks };
        let settings = Settings {
            radius: 3,
            min_y: 0,
            max_y: 0,
            inverse: true,
            inverse_replace_target: Some("minecraft:oak_planks".into()),
            filters: vec![Filter::new(FilterKind::Item("minecraft:dirt".into()))],
            ..Settings::default()
        };
        let (_, found) = settings.search([0, 5, 0], &mut world);
        assert_eq!(found, 1);
        assert_eq!(Settings { inverse: false, ..settings.clone() }.search([0, 5, 0], &mut world).1, 1);
        let none = Settings { inverse: false, filters: vec![], ..settings };
        assert_eq!(none.search([0, 5, 0], &mut world).1, 0);
    }
}
