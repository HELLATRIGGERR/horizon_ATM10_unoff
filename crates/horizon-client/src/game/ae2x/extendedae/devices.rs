//! ExtendedAE's Extended Inscriber, Charger and Molecular Assembler in the
//! game. Derived from ExtendedAE 1.21-2.2.42 (`TileExInscriber`,
//! `InscriberThread`, `ContainerExInscriber`, `TileExCharger`,
//! `TileExMolecularAssembler`, `CraftingThread`),
//! https://github.com/GlodBlock/ExtendedAE commit
//! 1f86f927552f02451236465c0c96a12a3de8f66c, LGPL-3.0; see `ae2/addons/extendedae/`.
//!
//! An `InscriberThread` is AE2's `InscriberBlockEntity.tickingRequest` on
//! four slots of its own (the mod copied it): each thread runs as AE2's
//! Inscriber on its slots ([`thread`]), with the block's cards, buffer and
//! settings. The Extended Molecular Assembler is AE2's assembler of the
//! registry's variant row; only its speeds are the mod's ([`assembler_speed`]).
use super::super::super::{Game, ae2_buses::FormationPlan, ae2_global::GridAddress, ae2_processing as processing, ae2_tick_tracker::Modulation};
use horizon_client::{
    ae2_world::{Machine, MachineKind},
    ae2x::extendedae as rows,
    containers::BlockEntity,
    items::ItemStack,
};
use horizon_core::{geometry::Direction, random::RandomSource};
use horizon_world::nbt::{Compound, Tag};

/// The mod's devices that are AE2's machines several times over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum Device {
    Inscriber,
    Charger,
}
impl Device {
    pub(in crate::game) fn of(m: &Machine) -> Option<Self> {
        Self::of_kind(m.kind)
    }
    pub(in crate::game) fn of_kind(kind: MachineKind) -> Option<Self> {
        Some(match kind.ext()?.id {
            rows::EX_INSCRIBER => Self::Inscriber,
            rows::EX_CHARGER => Self::Charger,
            _ => return None,
        })
    }
}

/// An Extended Inscriber thread's slots: top, bottom, middle, output.
pub(in crate::game) const THREAD_SLOTS: usize = 4;
/// The settings the threads share (the tile's `ConfigManager` and
/// `stacksize`), under the keys AE2's Inscriber keeps them by.
const SHARED: [&str; 3] = ["auto_export", "inscriber_separate_sides", "inscriber_input_capacity"];
/// What a thread keeps of its own beside its progress.
const OWN: [&str; 2] = ["ae2_smash", "ae2_input_identity"];
/// The page of the Extended Inscriber's window: the thread it shows.
pub(in crate::game) const PAGE: &str = "ae2x_page";

/// Thread `n` of an Extended Inscriber as AE2's Inscriber: its four slots,
/// the block's cards, buffer and settings, its own progress and smash.
pub(in crate::game) fn thread(m: &Machine, n: usize) -> Machine {
    let mut t = Machine::new(MachineKind::Inscriber);
    t.slots = m.slots[THREAD_SLOTS * n..THREAD_SLOTS * (n + 1)].to_vec();
    t.upgrades = m.upgrades.clone();
    t.energy = m.energy;
    for key in SHARED {
        if let Some(value) = m.config.get(&key.into()) {
            t.config.insert(key.into(), value.clone());
        }
    }
    if let Some(Tag::Compound(state)) = m.config.get(&rows::thread_key(n).as_str().into()) {
        if let Some(Tag::Double(bits)) = state.get(&"progress".into()) {
            t.progress = f64::from_bits(*bits);
        }
        for key in OWN {
            if let Some(value) = state.get(&key.into()) {
                t.config.insert(key.into(), value.clone());
            }
        }
    }
    t
}
/// Thread `n` back into its Extended Inscriber.
pub(in crate::game) fn store_thread(m: &mut Machine, n: usize, t: &Machine) {
    m.slots[THREAD_SLOTS * n..THREAD_SLOTS * (n + 1)].clone_from_slice(&t.slots);
    m.energy = t.energy;
    let mut state = Compound::new();
    if t.progress != 0.0 {
        state.insert("progress".into(), Tag::Double(t.progress.to_bits()));
    }
    for key in OWN {
        if let Some(value) = t.config.get(&key.into()) {
            state.insert(key.into(), value.clone());
        }
    }
    if state.is_empty() {
        m.config.remove(&rows::thread_key(n).as_str().into());
    } else {
        m.config.insert(rows::thread_key(n).as_str().into(), Tag::Compound(state));
    }
}
/// The thread the window shows (`ContainerExInscriber.page`).
pub(in crate::game) fn page(m: &Machine) -> usize {
    match m.config.get(&PAGE.into()) {
        Some(Tag::Int(n)) => usize::try_from(*n).unwrap_or(0).min(rows::THREADS - 1),
        _ => 0,
    }
}
/// `InscriberThread.setStackSize`: the inputs' limit, 64, 1 or 4.
pub(in crate::game) fn stack_size(m: &Machine) -> u32 {
    match m.config.get(&"inscriber_input_capacity".into()) {
        Some(Tag::String(s)) if s.to_string_lossy() == "ONE" => 1,
        Some(Tag::String(s)) if s.to_string_lossy() == "FOUR" => 4,
        _ => 64,
    }
}
/// `TileExInscriber.tickingRequest`: the most urgent of its threads'
/// (`TickRateModulation`'s order, SLEEP the least).
pub(in crate::game) fn more_urgent(a: Modulation, b: Modulation) -> Modulation {
    let rank = |m: Modulation| match m {
        Modulation::Sleep => 0,
        Modulation::Idle => 1,
        Modulation::Slower => 2,
        Modulation::Same => 3,
        Modulation::Faster => 4,
        Modulation::Urgent => 5,
    };
    if rank(b) > rank(a) { b } else { a }
}
/// What wakes a sleeping Extended Inscriber (`onChangeInventory`, a setting).
fn inscriber_token(m: &Machine) -> String {
    let settings: Vec<_> = SHARED.iter().map(|key| m.config.get(&(*key).into())).collect();
    format!("{:?}|{:?}|{settings:?}", m.slots, m.upgrades)
}

/// `TileExMolecularAssembler.MAX_THREAD`: its crafting threads, each a job
/// (`pushPattern` gives a job to the first free one).
pub(in crate::game) fn assembler_threads(m: &Machine) -> Option<usize> {
    (m.id() == rows::EX_ASSEMBLER).then_some(rows::ASSEMBLER_THREADS)
}
/// `CraftingThread.tick`: 20, 26, 34, 40, 50 or 100 progress a tick for 0 to
/// 5 speed cards, at 1, 1.3, 1.7, 2, 2.5 or 5 AE a point; AE2's is half.
pub(in crate::game) fn assembler_speed(m: &Machine) -> Option<(f64, f64)> {
    if m.id() != rows::EX_ASSEMBLER {
        return None;
    }
    Some(thread_speed(m.installed_speed_cards()))
}
/// `CraftingThread.tick` with `cards` speed cards (the Assembler Matrix's
/// speed cores), up to five.
pub(in crate::game) fn thread_speed(cards: usize) -> (f64, f64) {
    match cards.min(5) {
        0 => (20.0, 1.0),
        1 => (26.0, 1.3),
        2 => (34.0, 1.7),
        3 => (40.0, 2.0),
        4 => (50.0, 2.5),
        _ => (100.0, 5.0),
    }
}

/// `TileExCharger.POWER_THRESHOLD`.
const CHARGER_THRESHOLD: f64 = 3199.0;
/// `ChargerInvFilter.allowExtract`: a full chargeable item, or one no
/// charger recipe takes.
fn charger_done(stack: &ItemStack, items: &horizon_client::items::Items, recipes: &horizon_client::crafting::Recipes) -> bool {
    let full = processing::charger_chargeable(stack) && processing::chargeable_energy(stack).is_ok_and(|(current, max)| current >= max);
    full || processing::live_charger_recipe(stack, items, recipes).is_none()
}

impl Game<'_> {
    /// The grid ticks of the Extended Inscriber and Charger.
    pub(in crate::game) fn tick_extendedae_devices(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let mut found: Vec<_> = self
            .world
            .block_entities
            .iter()
            .filter_map(|(p, e)| match e {
                BlockEntity::Ae2(m) => Device::of(m).map(|d| (*p, d)),
                _ => None,
            })
            .collect();
        found.sort_unstable_by_key(|(p, _)| *p);
        for (pos, device) in found {
            match device {
                Device::Inscriber => self.extendedae_inscriber_tick(pos),
                Device::Charger => self.extendedae_charger_tick(pos),
            }
        }
    }

    /// `TileExInscriber.tickingRequest` at AE2's Inscriber rates: each thread
    /// ticks (`InscriberThread.tick`, AE2's Inscriber on its slots) and pushes
    /// its output out, in thread order.
    fn extendedae_inscriber_tick(&mut self, pos: (i32, i32, i32)) {
        let Some(BlockEntity::Ae2(original)) = self.world.block_entities.get(&pos) else { return };
        let original = (**original).clone();
        let res = self.res.clone();
        let sleeping = (0..rows::THREADS).all(|n| processing::inscriber_modulation(&thread(&original, n), res.items(), res.recipes(), false) == Modulation::Sleep);
        let (min, max) = horizon_client::ae2_tick_config::rates(horizon_client::ae2_tick_config::Kind::Inscriber);
        let token = inscriber_token(&original);
        if !self.ae2_variable_processor_due(pos, min, max, sleeping, token) {
            return;
        }
        let mut m = original.clone();
        let mut formation = FormationPlan::default();
        let mut rate = Modulation::Sleep;
        for n in 0..rows::THREADS {
            let mut t = thread(&m, n);
            let mut online = None;
            processing::inscriber_tick(&mut t, res.items(), res.recipes(), |amount, simulate| {
                if !*online.get_or_insert_with(|| self.ae2_online(pos)) {
                    return 0.0;
                }
                self.ae2_take_power(pos, amount, simulate)
            });
            let pushed = self.ae2_inscriber_push_output(pos, &mut t, &original, &mut formation);
            rate = more_urgent(rate, processing::inscriber_modulation(&t, res.items(), res.recipes(), pushed));
            store_thread(&mut m, n, &t);
        }
        self.ae2_variable_processor_completed(pos, rate, inscriber_token(&m));
        if m != original {
            if let Some(BlockEntity::Ae2(target)) = self.world.block_entities.get_mut(&pos) {
                **target = m;
                self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
            }
            if let Err(error) = self.ae2_commit_formation(formation) {
                log::warn!("Extended Inscriber notification: {error}");
            }
        }
    }

    /// `TileExCharger.doWork` every `TickRates.Charger`: each slot in turn
    /// charges its item from the block's buffer, then the grid, or runs its
    /// recipe (one chance in five a call, the whole buffer); after each slot
    /// a buffer under 3,199 AE takes up to 800 from the grid.
    fn extendedae_charger_tick(&mut self, pos: (i32, i32, i32)) {
        let (interval, _) = horizon_client::ae2_tick_config::rates(horizon_client::ae2_tick_config::Kind::Charger);
        let Some(ticks) = self.ae2_fixed_processor_elapsed(pos, interval) else { return };
        let Some(BlockEntity::Ae2(original)) = self.world.block_entities.get(&pos) else { return };
        let mut m = (**original).clone();
        let before = m.clone();
        let res = self.res.clone();
        let usage = horizon_client::ae2_common_config::gameplay().usage_multiplier;
        let rate = horizon_client::ae2_common_config::processing().charger_rate;
        let max_power = m.private_power_capacity().unwrap_or(3200.0);
        let online = self.ae2_online(pos);
        for slot in 0..m.slots.len() {
            if let Some(mut stack) = m.slots[slot].clone() {
                if processing::charger_chargeable(&stack) {
                    if let (Ok((current, max)), Ok(charge)) = (processing::chargeable_energy(&stack), processing::chargeable_rate(&stack))
                        && current < max
                    {
                        let wanted = charge * ticks as f64 * rate;
                        let mut taken = m.private_energy_extract(wanted * usage, true) / usage;
                        let missing = (wanted - taken).min(max - current).max(0.0);
                        if online && missing > 0.0 {
                            taken += self.ae2_take_power_raw(pos, missing, false);
                        }
                        if taken > 0.0 {
                            // injectAEPower's overflow goes back to the buffer.
                            let accepted = processing::chargeable_insert(&mut stack, taken).unwrap_or(0.0);
                            m.private_energy_insert((taken - accepted).max(0.0), true);
                            m.slots[slot] = Some(stack);
                        }
                    }
                } else if m.energy > CHARGER_THRESHOLD
                    && let Some(out) = processing::live_charger_recipe(&stack, res.items(), res.recipes())
                    && res.items().get(&out.item).is_some()
                    && self.world.random.next_float() > 0.8
                {
                    m.private_energy_extract(max_power * usage, true);
                    m.slots[slot] = Some(out);
                }
            }
            if m.energy < CHARGER_THRESHOLD && online {
                let wanted = 800f64.min(max_power - m.energy);
                let received = self.ae2_take_power_raw(pos, wanted, false);
                m.private_energy_insert(received, true);
            }
        }
        if m != before
            && let Some(BlockEntity::Ae2(target)) = self.world.block_entities.get_mut(&pos)
        {
            **target = m;
            self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
        }
    }

    /// `TileExCharger.activate`, the charger's `openGui`: the held item goes,
    /// one of it, into the first empty slot when it charges or a recipe takes
    /// it; else the first item in it drops in front of it. A wrench turns it.
    pub(in crate::game) fn extendedae_charger_use(&mut self, pos: (i32, i32, i32)) -> bool {
        let Some(BlockEntity::Ae2(m)) = self.world.block_entities.get(&pos) else { return false };
        if Device::of(m) != Some(Device::Charger) || self.held().is_some_and(|s| self.ae2_wrench_can_rotate(s)) {
            return false;
        }
        let mut m = (**m).clone();
        let held = self.held().cloned();
        let takes = held.as_ref().is_some_and(|h| processing::charger_chargeable(h) || processing::live_charger_recipe(h, self.res.items(), self.res.recipes()).is_some());
        match (takes, held, m.slots.iter().position(Option::is_none)) {
            (true, Some(mut held), Some(slot)) => {
                m.slots[slot] = Some(ItemStack { count: 1, ..held.clone() });
                held.count -= 1;
                *self.held_mut() = (held.count > 0).then_some(held);
            }
            _ => {
                let Some(slot) = m.slots.iter().position(Option::is_some) else { return true };
                let stack = m.slots[slot].take().unwrap();
                let front = self.pack.assets.registry.state(self.world.block(pos.0, pos.1, pos.2)).and_then(|s| s.properties.get("facing").cloned());
                let d = Direction::ALL.into_iter().find(|d| Some(super::machines::direction_name(*d)) == front.as_deref()).unwrap_or(Direction::North).normal();
                self.pop_resource([pos.0 + d[0], pos.1 + d[1], pos.2 + d[2]], stack);
            }
        }
        if let Some(BlockEntity::Ae2(target)) = self.world.block_entities.get_mut(&pos) {
            **target = m;
        }
        self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
        true
    }

    /// The menu's slot filter: an Extended Inscriber thread's inputs as AE2's
    /// Inscriber's (`BaseFilter`), never its output.
    pub(in crate::game) fn extendedae_may_place(&self, m: &Machine, slot: usize, item: &ItemStack) -> Option<bool> {
        if Device::of(m) == Some(Device::Inscriber) {
            let (n, i) = (slot / THREAD_SLOTS, slot % THREAD_SLOTS);
            return Some(n < rows::THREADS && i < 3 && self.ae2_inscriber_accepts(&thread(m, n), i, item));
        }
        super::may_place(m, slot)
    }

    /// Whether the Extended Inscriber, Charger or Molecular Assembler is at `address`.
    pub(in crate::game) fn extendedae_item_handler(&self, address: GridAddress) -> bool {
        matches!(self.ae2_block_entity(address), Some(BlockEntity::Ae2(m)) if Device::of(m).is_some() || m.id() == rows::EX_ASSEMBLER)
    }
    /// The slots of the device at `address` its handler on `face` reaches, in
    /// order, and whether each takes items in and gives them out.
    fn extendedae_handler_slots(&self, address: GridAddress, face: Direction, m: &Machine) -> Vec<(usize, bool, bool)> {
        match Device::of(m) {
            // AENetworkedInvBlockEntity's: the whole inventory on every side.
            Some(Device::Charger) => (0..m.slots.len()).map(|i| (i, true, true)).collect(),
            // getExposedInventoryForSide: with separate sides the tops on its
            // top, the bottoms on its bottom, middles and outputs elsewhere;
            // else all, thread by thread (CombinedInternalInventory).
            // AutomationFilter: outputs out only, inputs never out but tops
            // and bottoms with separate sides, nothing in or out of the
            // inputs of a thread that smashes.
            Some(Device::Inscriber) => {
                let mut slots = Vec::new();
                for n in 0..rows::THREADS {
                    let t = thread(m, n);
                    let Some(faces) = self.ae2_inscriber_faces(address, face, &t) else { continue };
                    let separate = faces.len() != THREAD_SLOTS;
                    let smash = matches!(t.config.get(&"ae2_smash".into()), Some(Tag::Int(s)) if *s > 0);
                    for i in faces {
                        let output = i == 3;
                        slots.push((THREAD_SLOTS * n + i, !output && !smash, output || (separate && !smash && i < 2)));
                    }
                }
                slots
            }
            // CraftingGridFilter: nothing in, the output out.
            None if m.id() == rows::EX_ASSEMBLER => vec![(9, false, true)],
            None => Vec::new(),
        }
    }
    /// What may go into slot `slot` (ChargerInvFilter, BaseFilter).
    fn extendedae_handler_accepts(&self, m: &Machine, slot: usize, stack: &ItemStack) -> bool {
        match Device::of(m) {
            Some(Device::Charger) => processing::charger_chargeable(stack) || processing::live_charger_recipe(stack, self.res.items(), self.res.recipes()).is_some(),
            Some(Device::Inscriber) => self.ae2_inscriber_accepts(&thread(m, slot / THREAD_SLOTS), slot % THREAD_SLOTS, stack),
            None => false,
        }
    }
    /// What may come out of slot `slot`.
    fn extendedae_handler_gives(&self, m: &Machine, stack: &ItemStack) -> bool {
        match Device::of(m) {
            Some(Device::Charger) => charger_done(stack, self.res.items(), self.res.recipes()),
            _ => true,
        }
    }
    /// A slot's limit: one in the charger (`AppEngInternalInventory(.., 4, 1, ..)`),
    /// the stack size for an inscriber's inputs.
    fn extendedae_handler_limit(&self, m: &Machine, slot: usize, stack: &ItemStack) -> u32 {
        let max = self.res.items().stack_max(stack).min(64);
        match Device::of(m) {
            Some(Device::Charger) => 1,
            Some(Device::Inscriber) if slot % THREAD_SLOTS < 3 => max.min(stack_size(m)),
            _ => max,
        }
    }
    pub(in crate::game) fn extendedae_item_insert(&mut self, address: GridAddress, face: Direction, stack: &ItemStack, limit: u32, simulate: bool) -> Option<u32> {
        let BlockEntity::Ae2(m) = self.ae2_block_entity(address)? else { return None };
        if Device::of(m).is_none() && m.id() != rows::EX_ASSEMBLER {
            return None;
        }
        let original = (**m).clone();
        let mut staged = original.clone();
        let mut left = limit.min(stack.count);
        for (slot, insert, _) in self.extendedae_handler_slots(address, face, &original) {
            if left == 0 {
                break;
            }
            if !insert || !self.extendedae_handler_accepts(&staged, slot, stack) {
                continue;
            }
            let max = self.extendedae_handler_limit(&staged, slot, stack);
            let moved = match staged.slots[slot].as_ref() {
                None => left.min(max),
                Some(s) if s.same_item_same_components(stack) => left.min(max.saturating_sub(s.count)),
                Some(_) => 0,
            };
            if moved == 0 {
                continue;
            }
            match staged.slots[slot].as_mut() {
                Some(s) => s.count += moved,
                None => {
                    let mut s = ItemStack { count: moved, ..stack.clone() };
                    horizon_client::ae2_stack_identity::detach_if_tracked(&mut s);
                    staged.slots[slot] = Some(s);
                }
            }
            left -= moved;
        }
        let accepted = limit.min(stack.count) - left;
        if simulate || accepted == 0 {
            return Some(accepted);
        }
        let Some(BlockEntity::Ae2(m)) = self.ae2_block_entity_mut(address) else { return Some(0) };
        if m.slots != original.slots {
            return Some(0);
        }
        m.slots = staged.slots;
        self.ae2_mark_modified(address);
        self.ae2_native_notify(&[address]);
        Some(accepted)
    }
    pub(in crate::game) fn extendedae_item_extract(&mut self, address: GridAddress, face: Direction, limit: u32, filter: Option<&ItemStack>, simulate: bool) -> Option<Option<ItemStack>> {
        let BlockEntity::Ae2(m) = self.ae2_block_entity(address)? else { return None };
        if Device::of(m).is_none() && m.id() != rows::EX_ASSEMBLER {
            return None;
        }
        let m = (**m).clone();
        let found = self.extendedae_handler_slots(address, face, &m).into_iter().filter(|(_, _, out)| *out).find_map(|(slot, _, _)| {
            let s = m.slots.get(slot)?.as_ref().filter(|s| s.count > 0 && filter.is_none_or(|f| f.same_item_same_components(s)))?;
            self.extendedae_handler_gives(&m, s).then(|| (slot, s.clone()))
        });
        let Some((slot, old)) = found else { return Some(None) };
        if limit == 0 {
            return Some(None);
        }
        let mut out = old.clone();
        out.count = out.count.min(limit);
        horizon_client::ae2_stack_identity::detach_if_tracked(&mut out);
        if simulate {
            return Some(Some(out));
        }
        let Some(BlockEntity::Ae2(target)) = self.ae2_block_entity_mut(address) else { return Some(None) };
        if target.slots.get(slot).and_then(Option::as_ref) != Some(&old) {
            return Some(None);
        }
        let mut rest = old;
        rest.count -= out.count;
        horizon_client::ae2_stack_identity::synchronize_tracked(&rest);
        target.slots[slot] = (rest.count > 0).then_some(rest);
        self.ae2_mark_modified(address);
        self.ae2_native_notify(&[address]);
        Some(Some(out))
    }
    pub(in crate::game) fn extendedae_item_candidates(&self, address: GridAddress, face: Direction) -> Option<Vec<ItemStack>> {
        let BlockEntity::Ae2(m) = self.ae2_block_entity(address)? else { return None };
        if Device::of(m).is_none() && m.id() != rows::EX_ASSEMBLER {
            return None;
        }
        Some(
            self.extendedae_handler_slots(address, face, m)
                .into_iter()
                .filter(|(_, _, out)| *out)
                .filter_map(|(slot, _, _)| m.slots.get(slot)?.as_ref().filter(|s| s.count > 0 && self.extendedae_handler_gives(m, s)).cloned())
                .collect(),
        )
    }

    /// A press on the Extended Inscriber's window: entries 0 to 3 are the
    /// shown thread's slots, 4 to 7 the cards; 22 and 23 turn the page on or
    /// back (`CUpdatePage`), 24 to 26 step its settings. True when it was one
    /// of those.
    pub(in crate::game) fn extendedae_inscriber_click(&mut self, pos: (i32, i32, i32), entry: usize, button: i32, input: horizon_client::menu::Input) -> bool {
        let Some(BlockEntity::Ae2(m)) = self.world.block_entities.get(&pos) else { return false };
        if Device::of(m) != Some(Device::Inscriber) {
            return false;
        }
        let shown = page(m);
        let slots = m.slots.len();
        match entry {
            0..THREAD_SLOTS => {
                self.ae2_machine_click_physical(pos, Some(300 + THREAD_SLOTS * shown + entry), button, input);
                true
            }
            4..8 => {
                self.ae2_machine_click_physical(pos, Some(300 + slots + entry - 4), button, input);
                true
            }
            8..22 => true,
            22..27 => {
                if input != horizon_client::menu::Input::Pickup || !(0..=1).contains(&button) {
                    return true;
                }
                let mut m = (**m).clone();
                match entry {
                    22 => {
                        m.config.insert(PAGE.into(), Tag::Int((shown + 1).min(rows::THREADS - 1) as i32));
                    }
                    23 => {
                        m.config.insert(PAGE.into(), Tag::Int(shown.saturating_sub(1) as i32));
                    }
                    _ => {
                        super::super::super::ae2_screens::step_setting(&mut m, 300 + entry, button == 1);
                    }
                }
                if let Some(BlockEntity::Ae2(target)) = self.world.block_entities.get_mut(&pos) {
                    **target = m;
                }
                self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
                true
            }
            _ => false,
        }
    }
}

/// The Extended Inscriber's window: the shown thread's slots, then the cards.
pub(in crate::game) fn inscriber_menu_items(m: &Machine, items: &mut [Option<ItemStack>]) {
    let shown = page(m);
    for (entry, slot) in items.iter_mut().enumerate().take(22) {
        *slot = match entry {
            0..THREAD_SLOTS => m.slots.get(THREAD_SLOTS * shown + entry).cloned().flatten(),
            4..8 => m.upgrades.get(entry - 4).cloned().flatten(),
            _ => None,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inscriber() -> Machine {
        horizon_client::ae2x::install();
        Machine::for_block(rows::EX_INSCRIBER).unwrap()
    }

    #[test]
    fn a_thread_is_ae2s_inscriber_on_its_slots() {
        let mut m = inscriber();
        m.slots[6] = Some(ItemStack::new("minecraft:redstone", 3));
        m.slots[7] = Some(ItemStack::new("ae2:logic_processor", 1));
        m.upgrades[0] = Some(ItemStack::new("ae2:speed_card", 1));
        m.energy = 400.0;
        m.config.insert("auto_export".into(), Tag::String("YES".into()));
        let mut t = thread(&m, 1);
        assert_eq!((t.kind, t.slots.len(), t.installed_speed_cards(), t.energy), (MachineKind::Inscriber, 4, 1, 400.0));
        assert_eq!(t.slots[2].as_ref().unwrap().count, 3);
        assert_eq!(t.slots[3].as_ref().unwrap().item.as_ref(), "ae2:logic_processor");
        assert_eq!(t.config.get(&"auto_export".into()), Some(&Tag::String("YES".into())));
        assert_eq!(t.progress, 0.0);
        // Its progress and smash are its own; the buffer is the block's.
        t.progress = 37.0;
        t.config.insert("ae2_smash".into(), Tag::Int(3));
        t.energy = 370.0;
        t.slots[2] = None;
        store_thread(&mut m, 1, &t);
        assert_eq!((m.energy, m.slots[6].clone()), (370.0, None));
        let again = thread(&m, 1);
        assert_eq!((again.progress, again.config.get(&"ae2_smash".into())), (37.0, Some(&Tag::Int(3))));
        assert_eq!(thread(&m, 0).progress, 0.0);
        // A reload starts every thread over (InscriberThread saves neither).
        let mut reloaded = m.clone();
        rows::reset_unsaved(&mut reloaded);
        assert_eq!(thread(&reloaded, 1).progress, 0.0);
        assert!(thread(&reloaded, 1).config.get(&"ae2_smash".into()).is_none());
        // A thread with nothing of its own keeps nothing.
        let empty = thread(&reloaded, 2);
        store_thread(&mut reloaded, 2, &empty);
        assert!(!reloaded.config.contains_key(&rows::thread_key(2).as_str().into()));
    }

    #[test]
    fn the_window_shows_the_page_and_its_settings() {
        let mut m = inscriber();
        assert_eq!((page(&m), stack_size(&m)), (0, 64));
        m.slots[13] = Some(ItemStack::new("minecraft:gold_ingot", 2));
        m.upgrades[3] = Some(ItemStack::new("ae2:speed_card", 1));
        m.config.insert(PAGE.into(), Tag::Int(3));
        let mut items = vec![Some(ItemStack::new("minecraft:dirt", 1)); 27];
        inscriber_menu_items(&m, &mut items);
        assert_eq!(items[1].as_ref().unwrap().item.as_ref(), "minecraft:gold_ingot");
        assert_eq!(items[7].as_ref().unwrap().item.as_ref(), "ae2:speed_card");
        assert!(items[8..22].iter().all(Option::is_none));
        assert!(items[22].is_some(), "the settings' entries are the screen's");
        m.config.insert(PAGE.into(), Tag::Int(9));
        assert_eq!(page(&m), 3);
        for (value, size) in [("ONE", 1), ("FOUR", 4), ("SIXTY_FOUR", 64)] {
            m.config.insert("inscriber_input_capacity".into(), Tag::String(value.into()));
            assert_eq!(stack_size(&m), size);
        }
    }

    #[test]
    fn threads_report_their_most_urgent_rate() {
        use Modulation::*;
        assert_eq!(more_urgent(Sleep, Slower), Slower);
        assert_eq!(more_urgent(Urgent, Sleep), Urgent);
        assert_eq!(more_urgent(Faster, Idle), Faster);
        assert_eq!([Sleep, Sleep, Sleep, Sleep].into_iter().fold(Sleep, more_urgent), Sleep);
    }

    #[test]
    fn the_assembler_runs_twice_ae2s_speed() {
        horizon_client::ae2x::install();
        let mut m = Machine::for_block(rows::EX_ASSEMBLER).unwrap();
        assert_eq!(assembler_speed(&m), Some((20.0, 1.0)));
        for (i, (speed, tax)) in [(26.0, 1.3), (34.0, 1.7), (40.0, 2.0), (50.0, 2.5), (100.0, 5.0)].into_iter().enumerate() {
            m.upgrades[i] = Some(ItemStack::new("ae2:speed_card", 1));
            assert_eq!(assembler_speed(&m), Some((speed, tax)));
        }
        assert_eq!(assembler_speed(&Machine::new(MachineKind::MolecularAssembler)), None);
    }
}
