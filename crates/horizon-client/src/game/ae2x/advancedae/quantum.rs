//! Advanced AE's Quantum Computer in the game. Derived from Advanced AE
//! (`AdvCraftingCPUCalculator`, `AdvCraftingCPUCluster`, `AdvCraftingCPU`,
//! `AdvCraftingBlockEntity`, `AAEAbstractCraftingUnitBlock`,
//! `AAECraftingUnitType`, `MixinCraftingService`, `QuantumComputerMenu`,
//! `QuantumComputerScreen`), https://github.com/pedroksl/AdvancedAE commit
//! `c322dad81e6f2ad94cb1fb861e73a7ba68e5907a`, LGPL-3.0; see
//! `ae2/addons/advancedae/`.
//!
//! The structure (`MBCalculator` with `AdvCraftingCPUCalculator`): a filled
//! box of the computer's blocks that nothing of them touches from outside, at
//! most `quantumComputerMaxSize` on each side, the Quantum Structure on all
//! its sides and nothing else there, inside one Quantum Core, at most
//! `quantumComputermaxDataEntanglers` Data Entanglers and
//! `quantumComputerMaxMultiThreaders` Multi-Threaders, the rest units,
//! storages and accelerators; a core alone is a computer too. A formed
//! computer is one multiblock node group (`GridFlags.MULTIBLOCK`,
//! `REQUIRE_CHANNEL`), its core connecting up and down only, the others on
//! every side. Its blocks keep the cluster in their config (`CLUSTER`),
//! rebuilt each tick as the mod rebuilds it on a neighbour's change.
//!
//! It is a crafting CPU of many jobs at once (`AdvCraftingCPUCluster.submitJob`):
//! each job an `AdvCraftingCPU` of the plan's own bytes, while the bytes left
//! (the storages' times the Data Entanglers' factor, less the jobs') hold the
//! next. The port keeps each job on one of the computer's blocks (AE2's job
//! session in that block's config, as on AE2's CPU), so a computer runs as
//! many jobs at once as it has blocks. The crafting service sees its free
//! block as a CPU of the bytes left (`getRemainingCapacityCPU`), before AE2's
//! (`MixinCraftingService.onSubmitJob`), fastest first.
use crate::game::{Game, ae2_global::GridAddress, lan::LanHost};
use horizon_client::{
    ae2_world::Machine,
    ae2x::advancedae::{QuantumConfig, QuantumPart},
    containers::BlockEntity,
};
use horizon_world::{
    chunk::BlockState,
    nbt::{Compound, Tag},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

type Pos = (i32, i32, i32);

/// The cluster a formed block belongs to (its corners, its core, its node
/// group, its storage and co-processors and how many blocks it has).
pub(in crate::game) const CLUSTER: &str = "aae_quantum";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::game) struct Bounds {
    pub(in crate::game) min: Pos,
    pub(in crate::game) max: Pos,
}
impl Bounds {
    fn all(self) -> impl Iterator<Item = Pos> {
        let (a, b) = (self.min, self.max);
        (a.0..=b.0).flat_map(move |x| (a.1..=b.1).flat_map(move |y| (a.2..=b.2).map(move |z| (x, y, z))))
    }
    /// `isBoundary`: on one of the box's sides.
    fn boundary(self, p: Pos) -> bool {
        p.0 == self.min.0 || p.1 == self.min.1 || p.2 == self.min.2 || p.0 == self.max.0 || p.1 == self.max.1 || p.2 == self.max.2
    }
}

/// A formed computer as `AdvCraftingCPUCluster.addBlockEntity` sums it up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) struct Formed {
    pub(in crate::game) bounds: Bounds,
    pub(in crate::game) core: Pos,
    pub(in crate::game) blocks: usize,
    /// The storages' bytes times the Data Entanglers' factors (`recalculateRemainingStorage`'s total).
    pub(in crate::game) storage: u64,
    /// The threads times the Multi-Threaders' factors (`getCoProcessors`).
    pub(in crate::game) coprocessors: u32,
}

fn neighbours((x, y, z): Pos) -> [Pos; 6] {
    [(x - 1, y, z), (x + 1, y, z), (x, y - 1, z), (x, y + 1, z), (x, y, z - 1), (x, y, z + 1)]
}

/// The computer's blocks joined to each other, each group once.
pub(in crate::game) fn components(members: &BTreeMap<Pos, QuantumPart>) -> Vec<Vec<Pos>> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for &start in members.keys() {
        if !seen.insert(start) {
            continue;
        }
        let mut component = vec![start];
        let mut queue = VecDeque::from([start]);
        while let Some(p) = queue.pop_front() {
            for n in neighbours(p) {
                if members.contains_key(&n) && seen.insert(n) {
                    component.push(n);
                    queue.push_back(n);
                }
            }
        }
        out.push(component);
    }
    out
}

/// `MBCalculator.calculateMultiblock` for a group of joined blocks: formed
/// exactly when they fill their box (the mod walks to the corners and wants
/// no block of it against the box from outside), the box passes
/// `checkMultiblockScale` and `verifyInternalStructure`; then the cluster's sums.
pub(in crate::game) fn verify(members: &BTreeMap<Pos, QuantumPart>, component: &[Pos], config: &QuantumConfig) -> Option<Formed> {
    let axis = |f: fn(&Pos) -> i32| (component.iter().map(f).min(), component.iter().map(f).max());
    let (x, y, z) = (axis(|p| p.0), axis(|p| p.1), axis(|p| p.2));
    let bounds = Bounds { min: (x.0?, y.0?, z.0?), max: (x.1?, y.1?, z.1?) };
    let span = [bounds.max.0 - bounds.min.0, bounds.max.1 - bounds.min.1, bounds.max.2 - bounds.min.2];
    // checkMultiblockScale: each side at most quantumComputerMaxSize.
    if span.iter().any(|d| *d > config.max_size - 1) {
        return None;
    }
    let volume: usize = span.iter().map(|d| (*d + 1) as usize).product();
    if component.len() != volume {
        return None;
    }
    let (mut core, mut storage) = (None, false);
    let (mut entanglers, mut threaders) = (0, 0);
    for p in bounds.all() {
        let part = *members.get(&p)?;
        let boundary = bounds.boundary(p);
        match part {
            // A core alone forms (`min.equals(max)`).
            QuantumPart::Core if bounds.min == bounds.max => {
                core = Some(p);
                storage = true;
                break;
            }
            QuantumPart::Core if !boundary && core.is_none() => core = Some(p),
            QuantumPart::Core => return None,
            QuantumPart::Structure if !boundary => return None,
            QuantumPart::DataEntangler if !boundary && entanglers < config.max_data_entanglers => entanglers += 1,
            QuantumPart::DataEntangler => return None,
            QuantumPart::MultiThreader if !boundary && threaders < config.max_multi_threaders => threaders += 1,
            QuantumPart::MultiThreader => return None,
            QuantumPart::Structure => {}
            _ if boundary => return None,
            _ => {}
        }
        storage |= part.storage_bytes() > 0;
    }
    let core = core.filter(|_| storage)?;
    // addBlockEntity's sums, then getCoProcessors and the total of recalculateRemainingStorage.
    let (mut bytes, mut factor, mut threads, mut multiplier) = (0u64, 0u64, 0u32, 0u32);
    for p in component {
        let part = members[p];
        bytes = bytes.saturating_add(part.storage_bytes());
        factor = factor.saturating_add(part.storage_multiplier(config));
        threads = threads.saturating_add(part.accelerator_threads(config));
        multiplier = multiplier.saturating_add(part.acceleration_multiplier(config));
    }
    Some(Formed {
        bounds,
        core,
        blocks: component.len(),
        storage: if factor > 0 { bytes.saturating_mul(factor) } else { bytes },
        coprocessors: if multiplier > 0 { threads.saturating_mul(multiplier) } else { threads },
    })
}

fn int3(p: Pos) -> Tag {
    Tag::IntArray(vec![p.0, p.1, p.2])
}
fn marker(f: &Formed, group: Option<ae2_network_core::Pos>) -> Tag {
    let mut c = Compound::new();
    c.insert("min".into(), int3(f.bounds.min));
    c.insert("max".into(), int3(f.bounds.max));
    c.insert("core".into(), int3(f.core));
    if let Some(g) = group {
        c.insert("group".into(), Tag::IntArray(vec![g.x, g.y, g.z]));
    }
    c.insert("blocks".into(), Tag::Int(f.blocks as i32));
    c.insert("storage".into(), Tag::Long(f.storage as i64));
    c.insert("threads".into(), Tag::Int(f.coprocessors as i32));
    Tag::Compound(c)
}
/// The cluster a block's config names.
pub(in crate::game) fn cluster(m: &Machine) -> Option<Formed> {
    let Some(Tag::Compound(c)) = m.config.get(&CLUSTER.into()) else { return None };
    let pos = |key: &str| match c.get(&key.into()) {
        Some(Tag::IntArray(v)) if v.len() == 3 => Some((v[0], v[1], v[2])),
        _ => None,
    };
    let (Some(Tag::Int(blocks)), Some(Tag::Long(storage)), Some(Tag::Int(threads))) =
        (c.get(&"blocks".into()), c.get(&"storage".into()), c.get(&"threads".into()))
    else {
        return None;
    };
    Some(Formed {
        bounds: Bounds { min: pos("min")?, max: pos("max")? },
        core: pos("core")?,
        blocks: usize::try_from(*blocks).ok()?,
        storage: u64::try_from(*storage).ok()?,
        coprocessors: u32::try_from(*threads).ok()?,
    })
}
/// The node group of a formed block (`IGridMultiblock`: one channel for all).
fn group(m: &Machine) -> Option<ae2_network_core::Pos> {
    let Some(Tag::Compound(c)) = m.config.get(&CLUSTER.into()) else { return None };
    match c.get(&"group".into()) {
        Some(Tag::IntArray(v)) if v.len() == 3 => Some(ae2_network_core::Pos { x: v[0], y: v[1], z: v[2] }),
        _ => None,
    }
}
pub(in crate::game) fn part(m: &Machine) -> Option<QuantumPart> {
    QuantumPart::of(m.id())
}

/// `AdvCraftingBlockEntity`'s node: MULTIBLOCK and REQUIRE_CHANNEL, AE2's idle
/// draw; formed, the core connects up and down (`getGridConnectableSides`),
/// the rest on every side; unformed, none. The sides follow the block state
/// alone, as the grid's event replay knows the block's entity only as it was
/// added.
pub(in crate::game) fn device(m: &Machine, properties: Option<&horizon_world::block_state_properties::BlockProperties>, device: &mut ae2_network_core::Device) -> bool {
    let Some(part) = part(m) else { return false };
    let formed = properties.and_then(|p| p.get("formed")).is_some_and(|v| v == "true");
    // The node's face bits: east, west, up, down, south, north.
    device.faces = match (formed, part) {
        (false, _) => 0,
        (true, QuantumPart::Core) => (1 << 2) | (1 << 3),
        (true, _) => 63,
    };
    device.idle_override = Some(1.0);
    device.channel_override = Some(true);
    device.multiblock = if formed { group(m) } else { None };
    true
}

impl Game<'_> {
    fn quantum_machine(&self, p: Pos) -> Option<&Machine> {
        match self.world.block_entities.get(&p) {
            Some(BlockEntity::Ae2(m)) if part(m).is_some() => Some(m),
            _ => None,
        }
    }
    /// The blocks of a formed computer, its core first, then in place order.
    fn quantum_members(&self, f: &Formed) -> Vec<Pos> {
        let mut out = vec![f.core];
        out.extend(f.bounds.all().filter(|p| *p != f.core && self.quantum_machine(*p).and_then(cluster).is_some_and(|c| c.bounds == f.bounds)));
        out
    }
    /// Every formed computer of this level, by its box.
    fn quantum_clusters(&self) -> BTreeMap<Bounds, Formed> {
        let mut out = BTreeMap::new();
        for (p, e) in self.world.block_entities.iter() {
            let BlockEntity::Ae2(m) = e else { continue };
            if part(m).is_none() {
                continue;
            }
            if let Some(f) = cluster(m).filter(|f| f.core == *p) {
                out.insert(f.bounds, f);
            }
        }
        out
    }
    /// The bytes a block's job holds (`AdvCraftingCPU.bytes`), none without a job.
    fn quantum_job_bytes(&self, p: Pos) -> Option<u64> {
        let address = self.ae2_local_address(p)?;
        self.ae2_cpu_job_capacity(address)
    }
    /// `getAvailableStorage`: the total less the jobs' bytes.
    fn quantum_remaining(&self, f: &Formed, members: &[Pos]) -> u64 {
        let used: u64 = members.iter().filter_map(|p| self.quantum_job_bytes(*p)).fold(0, u64::saturating_add);
        f.storage.saturating_sub(used)
    }
    /// The block the next job goes to: the core while it holds none, else the
    /// first block without one.
    fn quantum_free_member(&self, members: &[Pos]) -> Option<Pos> {
        members.iter().copied().find(|p| self.quantum_machine(*p).is_some_and(|m| !Self::ae2_cpu_has_job(m)))
    }

    /// Each tick: every block's cluster, as the mod recalculates it on a
    /// neighbour's change. A cluster that changes or comes apart gives its
    /// jobs up first (`breakCluster`: cancelled, their items back to the grid).
    pub(in crate::game) fn tick_advancedae_quantum(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let members: BTreeMap<Pos, QuantumPart> = self
            .world
            .block_entities
            .iter()
            .filter_map(|(p, e)| match e {
                BlockEntity::Ae2(m) => part(m).map(|r| (*p, r)),
                _ => None,
            })
            .collect();
        if members.is_empty() {
            return;
        }
        let config = QuantumConfig::get();
        let mut want = BTreeMap::<Pos, Option<Tag>>::new();
        for component in components(&members) {
            // A computer across an unloaded chunk keeps what it was.
            let loaded = component.iter().all(|p| neighbours(*p).iter().all(|n| self.world.loaded(n.0, n.2)));
            if !loaded {
                continue;
            }
            let formed = verify(&members, &component, config);
            let tag = formed.as_ref().map(|f| marker(f, self.ae2_local_address(f.bounds.min).map(|a| a.network_pos())));
            for p in component {
                want.insert(p, tag.clone());
            }
        }
        let mut changed = Vec::new();
        let mut broken = BTreeMap::<Bounds, Vec<Pos>>::new();
        for (&pos, tag) in &want {
            let Some(m) = self.quantum_machine(pos) else { continue };
            if m.config.get(&CLUSTER.into()) == tag.as_ref() {
                continue;
            }
            if let Some(old) = cluster(m) {
                broken.entry(old.bounds).or_default();
            }
            changed.push((pos, tag.clone()));
        }
        // The old clusters' blocks, while they still form them.
        for (bounds, list) in &mut broken {
            *list = bounds.all().filter(|p| self.quantum_machine(*p).and_then(cluster).is_some_and(|c| c.bounds == *bounds)).collect();
        }
        for (_, list) in broken {
            if let Some(&anchor) = list.first()
                && list.iter().any(|p| self.quantum_machine(*p).is_some_and(Self::ae2_cpu_has_job))
                && let Err(error) = self.ae2_break_crafting_members(anchor, list)
            {
                log::warn!("Quantum Computer at {anchor:?}: {error}");
            }
        }
        for (pos, tag) in changed {
            let formed = tag.is_some();
            let Some(BlockEntity::Ae2(m)) = self.world.block_entities.get_mut(&pos) else { continue };
            match tag {
                Some(tag) => m.config.insert(CLUSTER.into(), tag),
                None => m.config.remove(&CLUSTER.into()),
            };
            self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
            self.advancedae_quantum_state(pos, formed);
        }
    }
    /// The block state's `formed` (and `powered` off with it), which the node connects by.
    fn advancedae_quantum_state(&mut self, pos: Pos, formed: bool) {
        let old = self.world.block(pos.0, pos.1, pos.2);
        let Some(state) = self.pack.assets.registry.state(old) else { return };
        let mut props = state.properties.clone();
        if !props.contains_key("formed") {
            return;
        }
        props.insert("formed".into(), formed.to_string());
        if !formed {
            props.insert("powered".into(), "false".into());
            props.insert("multiblocked".into(), "false".into());
            props.insert("light_level".into(), "0".into());
        }
        let Ok(new) = self.pack.assets.registry.resolve(&BlockState { name: state.name.clone(), properties: props }) else { return };
        if new != old {
            let _ = self.level_set([pos.0, pos.1, pos.2], new);
        }
    }
    /// `AdvCraftingBlockEntity.updateSubType` as the grid tick projects it:
    /// `formed`, `powered` (online), `multiblocked` (more than one block) and
    /// `light_level` (12 on a formed, powered core).
    pub(in crate::game) fn advancedae_quantum_visual(&self, m: &Machine, online: bool, props: &mut horizon_world::block_state_properties::BlockProperties) -> bool {
        let Some(part) = part(m) else { return false };
        if !props.contains_key("formed") {
            return false;
        }
        let formed = cluster(m);
        props.insert("formed".into(), formed.is_some().to_string());
        props.insert("powered".into(), online.to_string());
        props.insert("multiblocked".into(), formed.is_some_and(|f| f.blocks > 1).to_string());
        let light = if part == QuantumPart::Core && formed.is_some() && online { 12 } else { 0 };
        props.insert("light_level".into(), light.to_string());
        true
    }

    /// The formed computers on the grid of `anchor` as CPUs for a job of
    /// `required` bytes: each its free block, the bytes left and its
    /// co-processors (`findSuitableAdvCraftingCPU`: online, room enough;
    /// fastest first, then the smallest).
    pub(in crate::game) fn advancedae_cpu_candidates(&self, anchor: Pos, required: u64) -> Vec<(GridAddress, u64, u32)> {
        let clusters = self.quantum_clusters();
        if clusters.is_empty() {
            return Vec::new();
        }
        let Some(anchor) = self.ae2_local_address(anchor) else { return Vec::new() };
        let Ok(analysis) = self.ae2_network().analyze() else { return Vec::new() };
        let Some(component) = analysis.nodes.get(&anchor.network_pos()).filter(|s| s.online).map(|s| s.component) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for f in clusters.values() {
            let Some(core) = self.ae2_local_address(f.core) else { continue };
            if !analysis.nodes.get(&core.network_pos()).is_some_and(|s| s.online && s.component == component) {
                continue;
            }
            let members = self.quantum_members(f);
            let remaining = self.quantum_remaining(f, &members);
            if remaining < required {
                continue;
            }
            let Some(free) = self.quantum_free_member(&members).and_then(|p| self.ae2_local_address(p)) else { continue };
            out.push((free, remaining, f.coprocessors));
        }
        // FAST_FIRST_COMPARATOR: the most co-processors, then the least storage.
        out.sort_by_key(|(address, remaining, coprocessors)| (std::cmp::Reverse(*coprocessors), *remaining, *address));
        out
    }
    /// A computer's block as a CPU: a job's own bytes and the computer's
    /// co-processors, or the bytes left on its free block; nothing while it is
    /// unformed. None for any other block.
    pub(in crate::game) fn advancedae_cpu_capacity(&self, cpu: GridAddress) -> Option<(u64, u32)> {
        let (dim, local) = self.ae2_address_parts(cpu)?;
        if dim != self.dim {
            return None;
        }
        let m = self.quantum_machine(local)?;
        let Some(f) = cluster(m) else { return Some((0, 0)) };
        if let Some(bytes) = self.ae2_cpu_job_capacity(cpu) {
            return Some((bytes, f.coprocessors));
        }
        let members = self.quantum_members(&f);
        Some((self.quantum_remaining(&f, &members), f.coprocessors))
    }
    /// The computers' blocks holding a job on the grid of `anchor`
    /// (`MixinCraftingService.onInsertIntoCpus`, after AE2's CPUs).
    pub(in crate::game) fn advancedae_cpu_job_owners(&self, anchor: GridAddress) -> Vec<GridAddress> {
        let clusters = self.quantum_clusters();
        if clusters.is_empty() {
            return Vec::new();
        }
        let Ok(analysis) = self.ae2_network().analyze() else { return Vec::new() };
        let Some(component) = analysis.nodes.get(&anchor.network_pos()).map(|s| s.component) else { return Vec::new() };
        let mut out = Vec::new();
        for f in clusters.values() {
            for p in self.quantum_members(f) {
                let Some(address) = self.ae2_local_address(p) else { continue };
                if self.quantum_machine(p).is_some_and(Self::ae2_cpu_has_job)
                    && analysis.nodes.get(&address.network_pos()).is_some_and(|s| s.component == component)
                {
                    out.push(address);
                }
            }
        }
        out
    }
    /// `AAEAbstractCraftingUnitBlock.onRemove` → `breakCluster`: the blocks
    /// whose jobs a removal of `pos` cancels. None for any other block.
    pub(in crate::game) fn advancedae_cpu_break_members(&self, pos: Pos) -> Option<Vec<Pos>> {
        let m = self.quantum_machine(pos)?;
        let members = match cluster(m) {
            Some(f) => self.quantum_members(&f),
            None => vec![pos],
        };
        Some(members.into_iter().filter(|p| self.quantum_machine(*p).is_some_and(Self::ae2_cpu_has_job)).collect())
    }
    /// Where a computer's block keeps the settings of its CPUs: the core
    /// (`AdvCraftingCPUCluster.configManager`). None for any other block.
    pub(in crate::game) fn advancedae_cpu_settings(&self, cpu: GridAddress) -> Option<GridAddress> {
        let (dim, local) = self.ae2_address_parts(cpu)?;
        if dim != self.dim {
            return None;
        }
        let f = cluster(self.quantum_machine(local)?)?;
        self.ae2_local_address(f.core)
    }
    /// `QuantumComputerMenu`'s CPUs: the jobs' blocks, then the free block
    /// (`getActiveCPUs`, `getRemainingCapacityCPU`). None for any other block.
    pub(in crate::game) fn advancedae_cluster_cpus(&self, pos: Pos) -> Option<Vec<GridAddress>> {
        let f = cluster(self.quantum_machine(pos)?)?;
        let members = self.quantum_members(&f);
        let mut out: Vec<GridAddress> =
            members.iter().filter(|p| self.quantum_machine(**p).is_some_and(Self::ae2_cpu_has_job)).filter_map(|p| self.ae2_local_address(*p)).collect();
        if let Some(free) = self.quantum_free_member(&members).and_then(|p| self.ae2_local_address(p)) {
            out.push(free);
        }
        Some(out)
    }
    /// `useWithoutItem`: a formed, active computer opens its screen
    /// (`QuantumComputerMenu`); any other block of it does nothing.
    pub(in crate::game) fn lan_open_advancedae_quantum(&mut self, host: &mut LanHost, conn: u32, pos: Pos) -> bool {
        let Some(m) = self.quantum_machine(pos) else { return false };
        if cluster(m).is_none() || !self.ae2_online(pos) {
            return false;
        }
        self.lan_open_quantum_craft_status(host, conn, pos)
    }
    /// `/ae2 advae quantum <x> <y> <z>`: the computer a block is in.
    pub(in crate::game) fn advancedae_quantum_status(&self, pos: Pos) -> Result<Value, String> {
        let m = self.quantum_machine(pos).ok_or("no Quantum Computer block there")?;
        let Some(f) = cluster(m) else { return Ok(json!({"formed": false, "part": format!("{:?}", part(m))})) };
        let members = self.quantum_members(&f);
        let jobs: Vec<Value> = members
            .iter()
            .filter_map(|p| {
                let address = self.ae2_local_address(*p)?;
                let bytes = self.ae2_cpu_job_capacity(address)?;
                let job = self.ae2_cpu_job(address);
                Some(json!({"at": [p.0, p.1, p.2], "bytes": bytes, "item": job.as_ref().map(|j| j.key.clone()), "requested": job.as_ref().map(|j| j.requested)}))
            })
            .collect();
        let address = self.ae2_local_address(f.core);
        Ok(json!({
            "formed": true,
            "min": [f.bounds.min.0, f.bounds.min.1, f.bounds.min.2],
            "max": [f.bounds.max.0, f.bounds.max.1, f.bounds.max.2],
            "core": [f.core.0, f.core.1, f.core.2],
            "blocks": f.blocks,
            "storage": f.storage,
            "coprocessors": f.coprocessors,
            "remaining": self.quantum_remaining(&f, &members),
            "online": self.ae2_online(f.core),
            "mode": address.and_then(|a| self.ae2_cpu_selection_mode(a)),
            "jobs": jobs,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box_of(min: Pos, max: Pos, fill: QuantumPart) -> BTreeMap<Pos, QuantumPart> {
        let b = Bounds { min, max };
        b.all().map(|p| (p, if b.boundary(p) { QuantumPart::Structure } else { fill })).collect()
    }
    fn formed(members: &BTreeMap<Pos, QuantumPart>, config: &QuantumConfig) -> Option<Formed> {
        let list = components(members);
        assert_eq!(list.len(), 1);
        verify(members, &list[0], config)
    }

    #[test]
    fn a_structure_shell_around_one_core_forms() {
        let config = QuantumConfig::default();
        let members = box_of((0, 0, 0), (2, 2, 2), QuantumPart::Core);
        let f = formed(&members, &config).unwrap();
        assert_eq!((f.core, f.blocks, f.storage, f.coprocessors), ((1, 1, 1), 27, 256 << 20, 8));
        // A core alone is a computer too.
        let alone = BTreeMap::from([((5, 5, 5), QuantumPart::Core)]);
        assert_eq!(formed(&alone, &config).map(|f| (f.blocks, f.storage)), Some((1, 256 << 20)));
    }

    #[test]
    fn what_does_not_form() {
        let config = QuantumConfig::default();
        // No core inside.
        assert!(formed(&box_of((0, 0, 0), (2, 2, 2), QuantumPart::Storage128), &config).is_none());
        // A unit on the shell.
        let mut shell = box_of((0, 0, 0), (2, 2, 2), QuantumPart::Core);
        shell.insert((0, 1, 1), QuantumPart::Unit);
        assert!(formed(&shell, &config).is_none());
        // Structure inside.
        let mut inner = box_of((0, 0, 0), (3, 2, 2), QuantumPart::Core);
        inner.insert((2, 1, 1), QuantumPart::Structure);
        assert!(formed(&inner, &config).is_none());
        // Two cores.
        assert!(formed(&box_of((0, 0, 0), (3, 2, 2), QuantumPart::Core), &config).is_none());
        // A hole.
        let mut hole = box_of((0, 0, 0), (2, 2, 2), QuantumPart::Core);
        hole.remove(&(0, 0, 0));
        assert!(formed(&hole, &config).is_none());
        // Larger than quantumComputerMaxSize.
        let mut big = box_of((0, 0, 0), (7, 2, 2), QuantumPart::Unit);
        big.insert((1, 1, 1), QuantumPart::Core);
        assert!(formed(&big, &config).is_none());
        big.retain(|p, _| p.0 < 7);
        for z in 0..3 {
            for y in 0..3 {
                big.insert((6, y, z), QuantumPart::Structure);
            }
        }
        assert!(formed(&big, &config).is_some(), "seven blocks on a side is the default limit");
        // A lone block that is no core.
        assert!(formed(&BTreeMap::from([((0, 0, 0), QuantumPart::Unit)]), &config).is_none());
    }

    #[test]
    fn the_parts_add_storage_and_threads_and_their_factors() {
        let config = QuantumConfig { data_entangler_multiplication: 6, multi_threader_multiplication: 6, accelerator_threads: 12, ..QuantumConfig::default() };
        // 6 x 3 x 3: inside, from x = 1: core, 128 MB storage, accelerator, entangler.
        let mut members = box_of((0, 0, 0), (5, 2, 2), QuantumPart::Unit);
        members.insert((1, 1, 1), QuantumPart::Core);
        members.insert((2, 1, 1), QuantumPart::Storage128);
        members.insert((3, 1, 1), QuantumPart::Accelerator);
        members.insert((4, 1, 1), QuantumPart::DataEntangler);
        let f = formed(&members, &config).unwrap();
        assert_eq!(f.storage, ((256 + 128) << 20) * 6, "the bytes times the entangler's factor");
        assert_eq!(f.coprocessors, 24, "the core's and the accelerator's threads");
        members.insert((3, 1, 1), QuantumPart::MultiThreader);
        assert_eq!(formed(&members, &config).unwrap().coprocessors, 12 * 6, "times the multi-threader's factor");
        // One entangler at most (quantumComputermaxDataEntanglers).
        members.insert((3, 1, 1), QuantumPart::DataEntangler);
        assert!(formed(&members, &config).is_none());
    }

    #[test]
    fn the_marker_round_trips() {
        let f = Formed { bounds: Bounds { min: (1, 2, 3), max: (3, 4, 5) }, core: (2, 3, 4), blocks: 27, storage: 1 << 40, coprocessors: 96 };
        let mut m = Machine::for_block("ae2:crafting_unit").unwrap();
        m.config.insert(CLUSTER.into(), marker(&f, Some(ae2_network_core::Pos { x: 1, y: 2, z: 3 })));
        assert_eq!(cluster(&m), Some(f));
        assert_eq!(group(&m), Some(ae2_network_core::Pos { x: 1, y: 2, z: 3 }));
    }
}
