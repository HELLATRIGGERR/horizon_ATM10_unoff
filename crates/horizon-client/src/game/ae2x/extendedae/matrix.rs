//! ExtendedAE's Assembler Matrix in the game. Derived from ExtendedAE
//! 1.21-2.2.42 (`CalculatorAssemblerMatrix`, `ClusterAssemblerMatrix`,
//! `TileAssemblerMatrix*`, `BlockAssemblerMatrix*`, `ContainerAssemblerMatrix`,
//! `GuiAssemblerMatrix`), https://github.com/GlodBlock/ExtendedAE commit
//! 1f86f927552f02451236465c0c96a12a3de8f66c, LGPL-3.0; see `ae2/addons/extendedae/`.
//!
//! The structure (`MBCalculator` with `CalculatorAssemblerMatrix`): a filled
//! box of the mod's matrix blocks that nothing of them touches from outside,
//! 3 to `assembler_matrix_max_size` + 1 blocks on each side, frames on its
//! edges, walls or glass on its faces, function blocks inside, at least one
//! crafter and one pattern core. A formed matrix is one multiblock node group
//! (`GridFlags.MULTIBLOCK`, one channel), its blocks connect on every side
//! and draw 0.5 AE/t each, a function block 1. Its blocks keep the cluster's
//! corners in their config (`MATRIX`), rebuilt each tick as the mod rebuilds
//! its cluster, which no save keeps.
//!
//! The pattern cores are pattern providers of 36 crafting patterns to the
//! grid; a job they take goes to a free thread of a crafter core
//! (`ClusterAssemblerMatrix.pushCraftingJob`), eight a core, each AE2's
//! assembler (`game/ae2_assembler_threads.rs`) at the speed of the matrix's
//! speed cores (`CraftingThread.tick` with up to five).
use super::super::super::{Game, ae2_assembler_threads as threads, ae2_buses::FormationPlan, ae2_global::GridAddress, lan::LanHost, lan_crafting};
use horizon_client::{
    ae2_world::Machine,
    ae2x::extendedae::{self as rows, MatrixRole},
    containers::BlockEntity,
    items::ItemStack,
    menu::{Input, Kind, Menu},
};
use horizon_world::{chunk::BlockState, nbt::{Compound, Tag}};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

type Pos = (i32, i32, i32);

/// The cluster a formed matrix block belongs to: its corners, its node
/// group (the network place of its lowest corner) and its speed cores.
pub(in crate::game) const MATRIX: &str = "eae_matrix";
/// `CalculatorAssemblerMatrix.MIN_SIZE`: the corners lie 2 apart at least.
const MIN_SPAN: i32 = 2;
/// The screen's layout (`ContainerAssemblerMatrix`), the window it opens.
pub(in crate::game) const MENU: &str = "extendedae:assembler_matrix";
/// The pattern rows the screen shows at once (`GuiAssemblerMatrix.drawFG`): a core's 36.
pub(in crate::game) const MENU_SLOTS: usize = rows::MATRIX_PATTERNS;
/// Its toolbar's entries: cancel, the Pattern Access Terminal toggle, the
/// next and the previous pattern core.
pub(in crate::game) const CANCEL_ENTRY: i32 = 36;
pub(in crate::game) const ACCESS_ENTRY: i32 = 37;
pub(in crate::game) const NEXT_ENTRY: i32 = 38;
pub(in crate::game) const PREVIOUS_ENTRY: i32 = 39;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::game) struct Bounds {
    pub(in crate::game) min: Pos,
    pub(in crate::game) max: Pos,
}
impl Bounds {
    /// Its inner blocks' places.
    fn inside(self) -> impl Iterator<Item = Pos> {
        let (a, b) = (self.min, self.max);
        (a.0 + 1..b.0).flat_map(move |x| (a.1 + 1..b.1).flat_map(move |y| (a.2 + 1..b.2).map(move |z| (x, y, z))))
    }
    /// All its places.
    fn all(self) -> impl Iterator<Item = Pos> {
        let (a, b) = (self.min, self.max);
        (a.0..=b.0).flat_map(move |x| (a.1..=b.1).flat_map(move |y| (a.2..=b.2).map(move |z| (x, y, z))))
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    Inside,
    Face,
    Edge,
}
/// `isInternal`, `isEdge` (two coordinates or three on the box's sides), else a face.
fn place(p: Pos, b: Bounds) -> Place {
    let on = |v: i32, lo: i32, hi: i32| u8::from(v == lo || v == hi);
    match on(p.0, b.min.0, b.max.0) + on(p.1, b.min.1, b.max.1) + on(p.2, b.min.2, b.max.2) {
        0 => Place::Inside,
        1 => Place::Face,
        _ => Place::Edge,
    }
}
fn neighbours((x, y, z): Pos) -> [Pos; 6] {
    [(x - 1, y, z), (x + 1, y, z), (x, y - 1, z), (x, y + 1, z), (x, y, z - 1), (x, y, z + 1)]
}

/// `MBCalculator.calculateMultiblock` for every matrix block: the cluster
/// it is in, or none. From any block the mod walks to the box's corners
/// along the axes and wants that box full of matrix blocks and no matrix
/// block against its faces outside (`verifyUnownedRegion`), so a block is
/// formed exactly when the blocks joined to it are a full box that passes
/// `checkMultiblockScale` and `verifyInternalStructure`.
pub(in crate::game) fn formation(members: &BTreeMap<Pos, MatrixRole>, size: i32) -> BTreeMap<Pos, Option<Bounds>> {
    let mut out = BTreeMap::new();
    let mut seen = BTreeSet::new();
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
        let bounds = cluster(members, &component, size);
        for p in component {
            out.insert(p, bounds);
        }
    }
    out
}
fn cluster(members: &BTreeMap<Pos, MatrixRole>, component: &[Pos], size: i32) -> Option<Bounds> {
    let axis = |f: fn(&Pos) -> i32| (component.iter().map(f).min(), component.iter().map(f).max());
    let (x, y, z) = (axis(|p| p.0), axis(|p| p.1), axis(|p| p.2));
    let b = Bounds { min: (x.0?, y.0?, z.0?), max: (x.1?, y.1?, z.1?) };
    let span = [b.max.0 - b.min.0, b.max.1 - b.min.1, b.max.2 - b.min.2];
    if span.iter().any(|d| *d < MIN_SPAN || *d > size) {
        return None;
    }
    let volume: usize = span.iter().map(|d| (*d + 1) as usize).product();
    if component.len() != volume {
        return None;
    }
    let (mut crafter, mut pattern) = (false, false);
    for p in component {
        let role = members[p];
        let fits = match place(*p, b) {
            Place::Inside => role.function(),
            Place::Edge => role == MatrixRole::Frame,
            Place::Face => role == MatrixRole::Wall,
        };
        if !fits {
            return None;
        }
        crafter |= role == MatrixRole::Crafter;
        pattern |= role == MatrixRole::Pattern;
    }
    (crafter && pattern).then_some(b)
}

/// The cluster a block's config names.
pub(in crate::game) fn bounds(m: &Machine) -> Option<Bounds> {
    let Some(Tag::Compound(c)) = m.config.get(&MATRIX.into()) else { return None };
    let corner = |key: &str| match c.get(&key.into()) {
        Some(Tag::IntArray(v)) if v.len() == 3 => Some((v[0], v[1], v[2])),
        _ => None,
    };
    Some(Bounds { min: corner("min")?, max: corner("max")? })
}
/// The node group of a formed block (`IGridMultiblock`: one channel for all).
pub(in crate::game) fn group(m: &Machine) -> Option<ae2_network_core::Pos> {
    let Some(Tag::Compound(c)) = m.config.get(&MATRIX.into()) else { return None };
    match c.get(&"group".into()) {
        Some(Tag::IntArray(v)) if v.len() == 3 => Some(ae2_network_core::Pos { x: v[0], y: v[1], z: v[2] }),
        _ => None,
    }
}
/// `ClusterAssemblerMatrix.getSpeedCore` of a formed block's cluster.
pub(in crate::game) fn speed_cores(m: &Machine) -> usize {
    match m.config.get(&MATRIX.into()) {
        Some(Tag::Compound(c)) => match c.get(&"speed".into()) {
            Some(Tag::Int(n)) => usize::try_from(*n).unwrap_or(0).min(rows::MATRIX_MAX_SPEED),
            _ => 0,
        },
        _ => 0,
    }
}
/// `CraftingThread.tick` with the speed cores as its cards: the crafter
/// core's progress a tick and AE a point.
pub(in crate::game) fn crafter_speed(m: &Machine) -> Option<(f64, f64)> {
    (m.id() == rows::MATRIX_CRAFTER).then(|| super::devices::thread_speed(speed_cores(m)))
}
fn marker(b: Bounds, group: Option<ae2_network_core::Pos>, speed: usize) -> Tag {
    let mut c = Compound::new();
    c.insert("min".into(), Tag::IntArray(vec![b.min.0, b.min.1, b.min.2]));
    c.insert("max".into(), Tag::IntArray(vec![b.max.0, b.max.1, b.max.2]));
    if let Some(g) = group {
        c.insert("group".into(), Tag::IntArray(vec![g.x, g.y, g.z]));
    }
    c.insert("speed".into(), Tag::Int(speed as i32));
    Tag::Compound(c)
}
/// `TileAssemblerMatrixPattern.Filter`: a pattern the Molecular Assembler
/// crafts (`IMolecularAssemblerSupportedPattern`: crafting, smithing table
/// and stonecutting patterns).
pub(in crate::game) fn pattern_allowed(item: &ItemStack) -> bool {
    use super::super::super::ae2_processing as processing;
    processing::ae2_decode_processing_pattern(item).is_none()
        && (processing::decode_pattern(item).is_some() || processing::ae2_decode_special_pattern(item).is_some())
}
/// `getLocateID` (`BlockPos.asLong`): the order of the pattern cores on the screen.
fn locate_id(p: Pos) -> i64 {
    ((i64::from(p.0) & 0x3FF_FFFF) << 38) | ((i64::from(p.2) & 0x3FF_FFFF) << 12) | (i64::from(p.1) & 0xFFF)
}

/// One player's open matrix screen.
pub(in crate::game) struct MatrixMenu {
    window: i32,
    /// The block used, which keeps the screen valid while it is formed.
    pos: Pos,
    /// The pattern core shown, by its order.
    page: usize,
    /// The player's inventory under it.
    menu: Menu,
    /// The data slots last sent.
    shown: Vec<i16>,
}

impl Game<'_> {
    fn matrix_machine(&self, p: Pos) -> Option<&Machine> {
        match self.world.block_entities.get(&p) {
            Some(BlockEntity::Ae2(m)) if MatrixRole::of(m.id()).is_some() => Some(m),
            _ => None,
        }
    }
    /// The blocks of the cluster `b` of a given role, in place order.
    fn matrix_members(&self, b: Bounds, role: MatrixRole) -> Vec<Pos> {
        b.inside().filter(|p| self.matrix_machine(*p).is_some_and(|m| MatrixRole::of(m.id()) == Some(role) && bounds(m) == Some(b))).collect()
    }
    /// The pattern cores of a cluster in `getLocateID` order (the screen's).
    fn matrix_pattern_cores(&self, b: Bounds) -> Vec<Pos> {
        let mut cores = self.matrix_members(b, MatrixRole::Pattern);
        cores.sort_by_key(|p| locate_id(*p));
        cores
    }

    /// Each tick: every matrix block's cluster, as the mod recalculates it on
    /// a neighbour's change; then the crafter cores of each formed one.
    pub(in crate::game) fn tick_extendedae_matrix(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let members: BTreeMap<Pos, MatrixRole> = self
            .world
            .block_entities
            .iter()
            .filter_map(|(p, e)| match e {
                BlockEntity::Ae2(m) => MatrixRole::of(m.id()).map(|r| (*p, r)),
                _ => None,
            })
            .collect();
        if members.is_empty() {
            return;
        }
        let formed = formation(&members, rows::Config::get().matrix_size);
        let mut clusters = BTreeMap::<Bounds, Tag>::new();
        for b in formed.values().flatten() {
            if clusters.contains_key(b) {
                continue;
            }
            let group = self.ae2_local_address(b.min).map(|a| a.network_pos());
            let speed = b.inside().filter(|p| members.get(p) == Some(&MatrixRole::Speed)).count().min(rows::MATRIX_MAX_SPEED);
            clusters.insert(*b, marker(*b, group, speed));
        }
        let mut changed = Vec::new();
        for (&pos, b) in &formed {
            let want = b.and_then(|b| clusters.get(&b).cloned());
            let Some(BlockEntity::Ae2(m)) = self.world.block_entities.get_mut(&pos) else { continue };
            if m.config.get(&MATRIX.into()) == want.as_ref() {
                continue;
            }
            match want {
                Some(tag) => m.config.insert(MATRIX.into(), tag),
                None => m.config.remove(&MATRIX.into()),
            };
            self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
            changed.push((pos, b.is_some()));
        }
        // The block state's `formed` with it, which the nodes connect by.
        for (pos, on) in changed {
            self.extendedae_matrix_state(pos, on, None);
        }
        for b in clusters.keys().copied().collect::<Vec<_>>() {
            for pos in self.matrix_members(b, MatrixRole::Crafter) {
                self.extendedae_crafter_tick(pos);
            }
        }
    }
    /// Sets the block's `formed` (and `powered` with it off) and the frame's `shape`.
    fn extendedae_matrix_state(&mut self, pos: Pos, formed: bool, powered: Option<bool>) {
        let old = self.world.block(pos.0, pos.1, pos.2);
        let Some(state) = self.pack.assets.registry.state(old) else { return };
        let mut props = state.properties.clone();
        props.insert("formed".into(), formed.to_string());
        props.insert("powered".into(), (formed && powered.unwrap_or(props.get("powered").is_some_and(|v| v == "true"))).to_string());
        let Ok(new) = self.pack.assets.registry.resolve(&BlockState { name: state.name.clone(), properties: props }) else { return };
        if new != old {
            let _ = self.level_set([pos.0, pos.1, pos.2], new);
        }
    }
    /// `BlockAssemblerMatrixFrame.getShapeType`: a column along the one axis
    /// with frames on both sides, else a block.
    fn extendedae_frame_shape(&self, p: Pos) -> &'static str {
        let frame = |q: Pos| self.matrix_machine(q).is_some_and(|m| m.id() == rows::MATRIX_FRAME);
        let (x, y, z) = p;
        let xx = frame((x - 1, y, z)) && frame((x + 1, y, z));
        let yy = frame((x, y - 1, z)) && frame((x, y + 1, z));
        let zz = frame((x, y, z - 1)) && frame((x, y, z + 1));
        match (xx, yy, zz) {
            (true, false, false) => "column_x",
            (false, true, false) => "column_y",
            (false, false, true) => "column_z",
            _ => "block",
        }
    }
    /// `TileAssemblerMatrixBase.updateSubType` as the grid tick projects it:
    /// `formed`, `powered` (formed and online) and the frame's `shape`.
    pub(in crate::game) fn extendedae_matrix_visual(&self, p: Pos, m: &Machine, online: bool, props: &mut horizon_world::block_state_properties::BlockProperties) -> bool {
        if MatrixRole::of(m.id()).is_none() {
            return false;
        }
        let formed = bounds(m).is_some();
        props.insert("formed".into(), formed.to_string());
        props.insert("powered".into(), (formed && online).to_string());
        if m.id() == rows::MATRIX_FRAME {
            props.insert("shape".into(), self.extendedae_frame_shape(p).into());
        }
        true
    }

    /// `TileAssemblerMatrixCrafter.tickingRequest`: each awake thread at the
    /// cluster's speed; then a thread whose job its CPU gave up, or that
    /// holds what no job owns (`ejectHeldItems` into its buffer), gives it to
    /// the grid (`OutputBuffer.flush`).
    fn extendedae_crafter_tick(&mut self, pos: Pos) {
        let Some(BlockEntity::Ae2(original)) = self.world.block_entities.get(&pos) else { return };
        let original = (**original).clone();
        if threads::busy_threads(&original) == 0 {
            return;
        }
        let mut m = original.clone();
        let mut plan = FormationPlan::default();
        if self.ae2_tick_assembler_threads(pos, &mut m, &original, &mut plan) && m != original {
            if let Some(BlockEntity::Ae2(target)) = self.world.block_entities.get_mut(&pos) {
                **target = m.clone();
                self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
            }
            if let Err(error) = self.ae2_commit_formation(plan) {
                log::warn!("Assembler Matrix crafter: {error}");
            }
        }
        let done = (0..threads::threads(&m)).filter_map(|n| threads::thread(&m, n)).any(|t| {
            threads::awake(&t) && !t.config.contains_key(&"ae2_assembler_pattern".into())
        });
        if done {
            self.ae2_recover_orphan_assembler(pos);
            self.extendedae_crafter_flush(pos, false);
        }
    }
    /// What the crafter's threads hold that no CPU flight owns goes into the
    /// grid; with `all` (`ContainerAssemblerMatrix.cancel`, `CraftingThread.stop`),
    /// every thread's job is dropped and its items go too.
    fn extendedae_crafter_flush(&mut self, pos: Pos, all: bool) {
        let Some(address) = self.ae2_local_address(pos) else { return };
        let Some(BlockEntity::Ae2(host)) = self.world.block_entities.get(&pos) else { return };
        let mut host = (**host).clone();
        let before = host.clone();
        let snapshot = self.ae2_snapshot_blocks();
        let mut plan = FormationPlan::default();
        let result = (|| -> Result<(), String> {
            for n in 0..threads::threads(&host) {
                let Some(mut t) = threads::thread(&host, n) else { continue };
                if !all && (t.config.contains_key(&"ae2_flight_owner".into()) || t.config.contains_key(&"ae2_assembler_pattern".into())) {
                    continue;
                }
                if !threads::awake(&t) && !all {
                    continue;
                }
                for slot in &mut t.slots[..10] {
                    let Some(stack) = slot.clone() else { continue };
                    let moved = self.ae2_grid_insert_uncharged_staged_encoded(address, &stack, u64::from(stack.count), &mut plan)?;
                    *slot = (moved < u64::from(stack.count)).then(|| ItemStack { count: stack.count - moved as u32, ..stack });
                }
                if all {
                    t.progress = 0.0;
                    t.config.remove(&"ae2_assembler_pattern".into());
                    t.config.remove(&"ae2_assembler_fluid_escrow".into());
                    t.config.remove(&"ae2_orphaned_flight".into());
                }
                threads::store_thread(&mut host, n, &t);
            }
            if host == before {
                return Ok(());
            }
            let Some(BlockEntity::Ae2(target)) = self.world.block_entities.get_mut(&pos) else { return Err("crafter disappeared".into()) };
            **target = host;
            self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
            self.ae2_commit_formation(plan)
        })();
        if let Err(error) = result {
            self.ae2_restore_blocks(snapshot);
            log::warn!("Assembler Matrix crafter flush: {error}");
        }
    }

    /// `TileAssemblerMatrixPattern.pushPattern` and `ClusterAssemblerMatrix.pushCraftingJob`:
    /// a job of the pattern core at `provider` goes to the first crafter core
    /// with a free thread (the mod's hash order; here place order); none
    /// unformed. None when `provider` is no pattern core.
    pub(in crate::game) fn extendedae_matrix_destination(&self, provider: GridAddress, taken: &dyn Fn(GridAddress, u8) -> bool) -> Option<Option<(GridAddress, u8)>> {
        let BlockEntity::Ae2(core) = self.ae2_block_entity(provider)? else { return None };
        if core.id() != rows::MATRIX_PATTERN {
            return None;
        }
        let Some(b) = bounds(core) else { return Some(None) };
        let (dim, _) = self.ae2_address_parts(provider)?;
        for p in b.inside() {
            let Some(address) = GridAddress::from_local(dim, p) else { continue };
            let Some(BlockEntity::Ae2(m)) = self.ae2_block_entity(address) else { continue };
            if m.id() != rows::MATRIX_CRAFTER || bounds(m) != Some(b) {
                continue;
            }
            if let Some(n) = threads::free_thread(m, |n| taken(address, n)) {
                return Some(Some((address, n)));
            }
        }
        Some(None)
    }
    /// `ClusterAssemblerMatrix.getBusyCrafterAmount`: the cluster's threads at work.
    fn matrix_running(&self, b: Bounds) -> usize {
        self.matrix_members(b, MatrixRole::Crafter).iter().filter_map(|p| self.matrix_machine(*p)).map(threads::busy_threads).sum()
    }

    // The screen (`ContainerAssemblerMatrix`), from any formed block.

    /// `BlockAssemblerMatrixBase.useWithoutItem` and `openGui`: a formed,
    /// active matrix opens its screen; an unformed block does nothing (the
    /// pattern core never opens a provider's screen). True when it was a
    /// matrix block that took the use.
    pub(in crate::game) fn lan_open_extendedae_matrix(&mut self, host: &mut LanHost, conn: u32, pos: Pos) -> bool {
        let Some(m) = self.matrix_machine(pos) else { return false };
        let pattern_core = m.id() == rows::MATRIX_PATTERN;
        let Some(b) = bounds(m) else { return pattern_core };
        if !self.ae2_online(pos) || self.matrix_pattern_cores(b).is_empty() {
            return true;
        }
        self.lan_ae2_portable_closed(host, conn, None);
        self.lan_ae2_bus_closed(host, conn, None);
        self.lan_ae2_network_tool_closed(host, conn, None);
        self.lan_ae2_anchor_closed(host, conn, None);
        self.lan_craft_confirm_closed(host, conn, None);
        self.lan_pattern_access_closed(host, conn, None);
        self.lan_ae2_terminal_closed(host, conn, None);
        self.lan_ae2_machine_closed(host, conn, None);
        self.lan_station_closed(host, conn, None);
        self.lan_crafting_closed(host, conn, None);
        self.guest_container_closed(host, conn);
        self.lan_void_cell_closed(host, conn, None);
        self.lan_matrix_closed(host, conn, None);
        use horizon_protocol::packets::{configuration::CustomPayload, play::Clientbound as C};
        host.hub.send_to(conn, &C::CustomPayload(CustomPayload { channel: "ae2:menu_block".into(), data: MENU.as_bytes().to_vec() }));
        let Some(window) = host.hub.open_window(conn, "minecraft:generic_9x6", horizon_protocol::text::translatable("gui.extendedae.assembler_matrix", &[])) else {
            return true;
        };
        Self::lan_mark_ae2_base_screen(host, conn, window);
        self.ae2x.extendedae.matrix_menus.insert(conn, MatrixMenu { window, pos, page: 0, menu: Menu::new(Kind::Chest(6)), shown: Vec::new() });
        self.lan_send_matrix(host, conn);
        true
    }
    pub(in crate::game) fn lan_matrix_closed(&mut self, host: &mut LanHost, conn: u32, window: Option<i32>) -> bool {
        if !self.ae2x.extendedae.matrix_menus.get(&conn).is_some_and(|m| window.is_none_or(|w| w == m.window)) {
            return false;
        }
        self.ae2x.extendedae.matrix_menus.remove(&conn);
        if let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) {
            self.guest_told_window_closed(host, gi);
        }
        true
    }
    /// The open screen's cluster, while its player may still use it
    /// (`AEBaseMenu.stillValid`: near the block, the block formed).
    fn lan_matrix_cluster(&self, host: &LanHost, conn: u32) -> Option<Bounds> {
        let s = self.ae2x.extendedae.matrix_menus.get(&conn)?;
        let here = host.hub.guests.iter().any(|g| g.conn == conn && host.hub.here(g) && g.next_window == s.window && g.within_block_range([s.pos.0, s.pos.1, s.pos.2], 4.0));
        if !here {
            return None;
        }
        bounds(self.matrix_machine(s.pos)?)
    }
    /// The screen's pattern core.
    fn lan_matrix_core(&self, host: &LanHost, conn: u32) -> Option<(Bounds, Pos)> {
        let b = self.lan_matrix_cluster(host, conn)?;
        let cores = self.matrix_pattern_cores(b);
        let page = self.ae2x.extendedae.matrix_menus.get(&conn)?.page.min(cores.len().checked_sub(1)?);
        Some((b, cores[page]))
    }
    /// `broadcastChanges`: the core's patterns, the player's inventory and
    /// the data slots: the Pattern Access Terminal setting (0) and the
    /// running threads (1).
    fn lan_send_matrix(&mut self, host: &mut LanHost, conn: u32) {
        use horizon_protocol::packets::play::{self, Clientbound as C};
        let Some((b, core)) = self.lan_matrix_core(host, conn) else {
            if let Some(window) = self.ae2x.extendedae.matrix_menus.get(&conn).map(|m| m.window) {
                self.lan_matrix_closed(host, conn, None);
                host.hub.send_to(conn, &C::ContainerClose(play::ContainerClose { container: window }));
            }
            return;
        };
        let Some(m) = self.matrix_machine(core) else { return };
        let visible = !matches!(m.config.get(&"terminal_visible".into()), Some(Tag::Byte(0)));
        let mut items: Vec<Option<ItemStack>> = m.slots.clone();
        items.resize(54, None);
        let data = vec![i16::from(visible), i16::try_from(self.matrix_running(b)).unwrap_or(i16::MAX)];
        let Some(g) = host.hub.guests.iter().find(|g| g.conn == conn) else { return };
        items.extend((9..36).chain(0..9).map(|i| g.inventory[i].clone()));
        let carried = g.carried.clone();
        let Some(s) = self.ae2x.extendedae.matrix_menus.get_mut(&conn) else { return };
        let packet = C::ContainerSetContent(play::ContainerSetContent {
            container: s.window,
            state: 0,
            items: items.iter().map(|i| host.hub.stack(i)).collect(),
            carried: host.hub.stack(&carried),
        });
        host.hub.send_to(conn, &packet);
        host.hub.inventory_told_by(conn, &items);
        for (id, value) in data.iter().enumerate() {
            if s.shown.get(id) != Some(value) {
                host.hub.send_to(conn, &C::ContainerSetData(play::ContainerSetData { container: s.window, id: id as i16, value: *value }));
            }
        }
        s.shown = data;
    }
    /// Each tick: a screen whose matrix broke or whose player left closes;
    /// the others are told what changed.
    pub(in crate::game) fn lan_refresh_matrix_menus(&mut self, host: &mut LanHost) {
        let conns: Vec<u32> = self.ae2x.extendedae.matrix_menus.keys().copied().collect();
        for conn in conns {
            if host.hub.guests.iter().any(|g| g.conn == conn && !host.hub.here(g)) {
                continue;
            }
            self.lan_send_matrix(host, conn);
        }
    }
    /// A toolbar press (`CEAEGenericPacket`): cancel, the Pattern Access
    /// Terminal setting for the whole cluster (`broadcastConfig`), or another
    /// pattern core. True when it was this screen's window.
    pub(in crate::game) fn lan_matrix_button(&mut self, host: &mut LanHost, conn: u32, window: i32, id: i32) -> bool {
        if !self.ae2x.extendedae.matrix_menus.get(&conn).is_some_and(|m| m.window == window) {
            return false;
        }
        let Some(b) = self.lan_matrix_cluster(host, conn) else {
            self.lan_send_matrix(host, conn);
            return true;
        };
        let cores = self.matrix_pattern_cores(b);
        match id & !super::super::super::ae2_screens::REVERSE {
            CANCEL_ENTRY => {
                for pos in self.matrix_members(b, MatrixRole::Crafter) {
                    self.extendedae_crafter_flush(pos, true);
                }
            }
            ACCESS_ENTRY => {
                // The shown core's setting, flipped, on every block of the cluster.
                let shown = self.lan_matrix_core(host, conn).and_then(|(_, core)| self.matrix_machine(core).cloned());
                let show = shown.is_some_and(|m| matches!(m.config.get(&"terminal_visible".into()), Some(Tag::Byte(0))));
                for p in b.all() {
                    if let Some(BlockEntity::Ae2(m)) = self.world.block_entities.get_mut(&p) {
                        if MatrixRole::of(m.id()).is_some() {
                            m.config.insert("terminal_visible".into(), Tag::Byte(i8::from(show)));
                            self.world.modified.insert((p.0 >> 4, p.2 >> 4));
                        }
                    }
                }
            }
            NEXT_ENTRY | PREVIOUS_ENTRY if !cores.is_empty() => {
                let forward = (id & !super::super::super::ae2_screens::REVERSE) == NEXT_ENTRY;
                if let Some(s) = self.ae2x.extendedae.matrix_menus.get_mut(&conn) {
                    let n = cores.len();
                    s.page = if forward { (s.page + 1).min(n - 1) } else { s.page.saturating_sub(1) };
                }
            }
            _ => {}
        }
        self.lan_send_matrix(host, conn);
        true
    }
    /// A click in the screen: a pattern slot (`ContainerAssemblerMatrix.doAction`,
    /// through the core's filter), a shift-click from the inventory into the
    /// first free slot of the cores (`transferStackToMenu`), or the player's
    /// inventory as any menu's.
    pub(in crate::game) fn lan_matrix_click(&mut self, host: &mut LanHost, conn: u32, p: &horizon_protocol::packets::play::ContainerClick) -> bool {
        let Some(mut s) = self.ae2x.extendedae.matrix_menus.remove(&conn) else { return false };
        if p.container != s.window {
            self.ae2x.extendedae.matrix_menus.insert(conn, s);
            return false;
        }
        let window = s.window;
        let gi = host.hub.guests.iter().position(|g| g.conn == conn && host.hub.here(g));
        let slot = i32::from(p.slot);
        let input = lan_crafting::input(p.input);
        let button = i32::from(p.button);
        if let Some(gi) = gi {
            if (0..54).contains(&slot) {
                self.ae2x.extendedae.matrix_menus.insert(conn, s);
                let core = self.lan_matrix_core(host, conn);
                if let Some((_, core)) = core.filter(|_| (slot as usize) < MENU_SLOTS) {
                    let _ = self.as_guest(host, conn, |g| g.extendedae_matrix_slot_action(core, slot as usize, button, input));
                }
                s = self.ae2x.extendedae.matrix_menus.remove(&conn).expect("the menu was just put back");
            } else if (54..90).contains(&slot) && input == Input::QuickMove {
                let index = if slot < 81 { 9 + (slot as usize - 54) } else { slot as usize - 81 };
                self.ae2x.extendedae.matrix_menus.insert(conn, s);
                if let Some(b) = self.lan_matrix_cluster(host, conn) {
                    let cores = self.matrix_pattern_cores(b);
                    let _ = self.as_guest(host, conn, |g| g.extendedae_matrix_quick_move(index, &cores));
                }
                s = self.ae2x.extendedae.matrix_menus.remove(&conn).expect("the menu was just put back");
            } else if (54..90).contains(&slot) || slot == -999 {
                self.guest_menu_op(host, gi, &mut s.menu, |m, env| {
                    let _ = m.click(env, slot, button, input);
                });
            }
        }
        let _ = window;
        self.ae2x.extendedae.matrix_menus.insert(conn, s);
        self.lan_send_matrix(host, conn);
        if let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) {
            host.hub.send_inventory(gi);
        }
        true
    }
    /// The pattern core's slot back, telling the crafting service of the change.
    fn extendedae_matrix_store(&mut self, core: Pos, slots: Vec<Option<ItemStack>>) -> bool {
        let Some(BlockEntity::Ae2(m)) = self.world.block_entities.get_mut(&core) else { return false };
        if m.id() != rows::MATRIX_PATTERN {
            return false;
        }
        if m.slots == slots {
            return true;
        }
        m.slots = slots;
        self.world.modified.insert((core.0 >> 4, core.2 >> 4));
        if let Some(address) = self.ae2_local_address(core) {
            self.ae2_provider_patterns_changed(address, None);
        }
        true
    }
    /// `doAction` on pattern slot `slot` of the core at `core`, as the player.
    fn extendedae_matrix_slot_action(&mut self, core: Pos, slot: usize, button: i32, input: Input) {
        let Some(m) = self.matrix_machine(core) else { return };
        let mut slots = m.slots.clone();
        let Some(current) = slots.get(slot).cloned() else { return };
        let held = self.carried.clone();
        match (input, button) {
            // PICKUP_OR_SET_DOWN: one pattern down, or a swap with a single
            // one, or the slot's pattern up.
            (Input::Pickup, 0) => match held.clone() {
                None => {
                    self.carried = current;
                    slots[slot] = None;
                }
                Some(h) if pattern_allowed(&h) => {
                    if current.is_none() {
                        slots[slot] = Some(ItemStack { count: 1, ..h.clone() });
                        self.carried = (h.count > 1).then(|| ItemStack { count: h.count - 1, ..h });
                    } else if h.count == 1 {
                        slots[slot] = Some(h);
                        self.carried = current;
                    }
                }
                Some(_) => {}
            },
            // SPLIT_OR_PLACE_SINGLE: one pattern down, or the slot's up.
            (Input::Pickup, 1) => match held.clone() {
                None => {
                    self.carried = current;
                    slots[slot] = None;
                }
                Some(h) if current.is_none() && pattern_allowed(&h) => {
                    slots[slot] = Some(ItemStack { count: 1, ..h.clone() });
                    self.carried = (h.count > 1).then(|| ItemStack { count: h.count - 1, ..h });
                }
                Some(_) => {}
            },
            // SHIFT_CLICK: into the player's inventory, if it fits.
            (Input::QuickMove, 0) => {
                if let Some(item) = current {
                    slots[slot] = self.give(item);
                }
            }
            // CREATIVE_DUPLICATE.
            (Input::Clone, _) if self.creative && self.carried.is_none() => self.carried = current,
            _ => {}
        }
        if !self.extendedae_matrix_store(core, slots) {
            self.carried = held;
        }
    }
    /// `transferStackToMenu`: a pattern from inventory slot `index` into the
    /// first free slot of the cores, in their order.
    fn extendedae_matrix_quick_move(&mut self, index: usize, cores: &[Pos]) {
        let Some(item) = self.inventory.get(index).cloned().flatten().filter(pattern_allowed) else { return };
        for &core in cores {
            let Some(m) = self.matrix_machine(core) else { continue };
            let mut slots = m.slots.clone();
            let Some(free) = slots.iter().position(Option::is_none) else { continue };
            slots[free] = Some(ItemStack { count: 1, ..item.clone() });
            if self.extendedae_matrix_store(core, slots) {
                self.inventory[index] = (item.count > 1).then(|| ItemStack { count: item.count - 1, ..item });
            }
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use MatrixRole::*;

    /// A box from `min` to `max` of frames, walls and, inside, `inner`.
    fn build(min: Pos, max: Pos, inner: impl Fn(Pos) -> MatrixRole) -> BTreeMap<Pos, MatrixRole> {
        let b = Bounds { min, max };
        let mut m = BTreeMap::new();
        for x in min.0..=max.0 {
            for y in min.1..=max.1 {
                for z in min.2..=max.2 {
                    let p = (x, y, z);
                    m.insert(p, match place(p, b) {
                        Place::Inside => inner(p),
                        Place::Face => Wall,
                        Place::Edge => Frame,
                    });
                }
            }
        }
        m
    }
    fn cores(p: Pos) -> MatrixRole {
        if p.0 % 2 == 0 { Crafter } else { Pattern }
    }

    #[test]
    fn a_full_box_of_the_right_blocks_forms() {
        let m = build((0, 0, 0), (3, 3, 3), cores);
        let f = formation(&m, 6);
        let b = Bounds { min: (0, 0, 0), max: (3, 3, 3) };
        assert!(f.values().all(|v| *v == Some(b)));
        // The smallest: 3 x 3 x 3, one block inside, which must then be both
        // a crafter and a pattern core: it never forms.
        assert!(formation(&build((0, 0, 0), (2, 2, 2), |_| Crafter), 6).values().all(Option::is_none));
        // Needs a crafter and a pattern core.
        assert!(formation(&build((0, 0, 0), (3, 3, 3), |_| Crafter), 6).values().all(Option::is_none));
        assert!(formation(&build((0, 0, 0), (3, 3, 3), |_| Speed), 6).values().all(Option::is_none));
    }

    #[test]
    fn its_size_is_limited_by_the_config() {
        // max_size 6: corners 6 apart at most, so 7 blocks a side.
        assert!(formation(&build((0, 0, 0), (6, 3, 3), cores), 6).values().all(Option::is_some));
        assert!(formation(&build((0, 0, 0), (7, 3, 3), cores), 6).values().all(Option::is_none));
        assert!(formation(&build((0, 0, 0), (7, 3, 3), cores), 7).values().all(Option::is_some));
        // Two apart at least on every axis.
        assert!(formation(&build((0, 0, 0), (1, 3, 3), cores), 6).values().all(Option::is_none));
    }

    #[test]
    fn a_wrong_block_a_gap_or_a_stray_neighbour_breaks_it() {
        let whole = build((0, 0, 0), (3, 3, 3), cores);
        // A wall gone.
        let mut gap = whole.clone();
        gap.remove(&(1, 0, 1));
        assert!(formation(&gap, 6).values().all(Option::is_none));
        // A frame where a wall goes, a wall on an edge, a core on a face.
        for (p, role) in [((1, 0, 1), Frame), ((0, 0, 1), Wall), ((1, 1, 0), Crafter)] {
            let mut wrong = whole.clone();
            wrong.insert(p, role);
            assert!(formation(&wrong, 6).values().all(Option::is_none), "{p:?} as {role:?}");
        }
        // A matrix block against a face outside (verifyUnownedRegion).
        let mut stray = whole.clone();
        stray.insert((4, 1, 1), Wall);
        assert!(formation(&stray, 6).values().all(Option::is_none));
        // One across a corner only touches nothing: the box still forms, it does not.
        let mut corner = whole;
        corner.insert((4, 4, 4), Frame);
        let f = formation(&corner, 6);
        assert_eq!(f[&(4, 4, 4)], None);
        assert!(f[&(0, 0, 0)].is_some());
    }

    #[test]
    fn the_marker_keeps_corners_group_and_speed() {
        let b = Bounds { min: (1, 2, 3), max: (4, 5, 6) };
        let mut m = Machine::for_block(rows::MATRIX_CRAFTER).unwrap_or_else(|| Machine::new(horizon_client::ae2_world::MachineKind::Ext(0)));
        m.config.insert(MATRIX.into(), marker(b, Some(ae2_network_core::Pos { x: 1, y: 2, z: 3 }), 7));
        assert_eq!(bounds(&m), Some(b));
        assert_eq!(group(&m), Some(ae2_network_core::Pos { x: 1, y: 2, z: 3 }));
        assert_eq!(speed_cores(&m), rows::MATRIX_MAX_SPEED);
        assert_eq!(b.inside().count(), 8);
        assert!(locate_id((0, 5, 0)) < locate_id((0, 0, 1)) && locate_id((0, 0, 1)) < locate_id((1, 0, 0)));
    }
}
