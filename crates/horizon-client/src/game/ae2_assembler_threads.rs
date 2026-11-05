//! The crafting threads of an assembler: where a crafting CPU sends a job.
//!
//! AE2's Molecular Assembler is one thread: its grid and output (slots 0..10),
//! its pattern slot (10), its `progress` and its job's keys in its config. An
//! addon's assembler holds more threads side by side (ExtendedAE's
//! `TileExMolecularAssembler` and the Assembler Matrix's crafter core, eight
//! `CraftingThread`s each): its first thread is the machine's own slots and
//! keys, as AE2's, and thread `n` past it keeps its grid and output, progress
//! and job's keys in the config compound `ae2_assembler_thread_<n>`, whose
//! items the machine drops when broken (`Machine::physical_contents`). A
//! thread seen alone ([`thread`]) is AE2's assembler of eleven slots, so the
//! assembler's tick and the CPU's dispatch and collection work on any of
//! them; a job's flight names the thread it went to.
use horizon_client::{ae2_world::{Machine, MachineKind, ASSEMBLER_THREAD_PREFIX}, save};
use horizon_world::nbt::{Compound, Tag};

/// AE2's assembler's slots: the grid, the output, the pattern.
pub(super) const SLOTS: usize = 11;
/// What a thread past the first keeps of the eleven: the grid and the output.
const THREAD_SLOTS: usize = 10;
/// A thread's job in its config: the pattern it crafts, the flight that sent
/// it, the fluids it holds for the grid, and a flight its CPU gave up.
const JOB_KEYS: [&str; 4] = ["ae2_assembler_pattern", "ae2_flight_owner", "ae2_assembler_fluid_escrow", "ae2_orphaned_flight"];

/// How many threads `m` crafts on: AE2's assembler one, an addon's its own
/// count; 0 for a machine that is no assembler.
pub(super) fn threads(m: &Machine) -> usize {
    if m.slots.len() != SLOTS {
        return 0;
    }
    if let Some(n) = super::ae2x::assembler_threads(m) {
        return n;
    }
    usize::from(m.kind == MachineKind::MolecularAssembler)
}
fn thread_key(n: usize) -> String {
    format!("{ASSEMBLER_THREAD_PREFIX}{n}")
}
/// Thread `n` of `m` as AE2's assembler: its slots (no pattern slot past the
/// first), the host's cards, its progress and its job's keys; none past the
/// assembler's threads.
pub(super) fn thread(m: &Machine, n: usize) -> Option<Machine> {
    if n >= threads(m) {
        return None;
    }
    let mut t = Machine::new(MachineKind::MolecularAssembler);
    t.variant = m.variant.filter(|v| v.kind == MachineKind::MolecularAssembler);
    t.upgrades = m.upgrades.clone();
    t.energy = m.energy;
    if n == 0 {
        t.slots = m.slots.clone();
        t.progress = m.progress;
        t.config = m.config.clone();
        return Some(t);
    }
    if let Some(Tag::Compound(state)) = m.config.get(&thread_key(n).as_str().into()) {
        let slots = save::decode_items(state.get(&"slots".into()), THREAD_SLOTS);
        t.slots[..THREAD_SLOTS].clone_from_slice(&slots);
        if let Some(Tag::Double(bits)) = state.get(&"progress".into()) {
            t.progress = f64::from_bits(*bits);
        }
        for key in JOB_KEYS {
            if let Some(value) = state.get(&key.into()) {
                t.config.insert(key.into(), value.clone());
            }
        }
    }
    Some(t)
}
/// Thread `n` back into `m`: its slots, progress and job's keys.
pub(super) fn store_thread(m: &mut Machine, n: usize, t: &Machine) {
    if n == 0 {
        let own = SLOTS.min(m.slots.len()).min(t.slots.len());
        m.slots[..own].clone_from_slice(&t.slots[..own]);
        m.progress = t.progress;
        for key in JOB_KEYS {
            match t.config.get(&key.into()) {
                Some(value) => {
                    m.config.insert(key.into(), value.clone());
                }
                None => {
                    m.config.remove(&key.into());
                }
            }
        }
        return;
    }
    let mut state = Compound::new();
    if t.slots.iter().take(THREAD_SLOTS).any(Option::is_some) {
        state.insert("slots".into(), save::encode_items(&t.slots[..THREAD_SLOTS.min(t.slots.len())]));
    }
    if t.progress != 0.0 {
        state.insert("progress".into(), Tag::Double(t.progress.to_bits()));
    }
    for key in JOB_KEYS {
        if let Some(value) = t.config.get(&key.into()) {
            state.insert(key.into(), value.clone());
        }
    }
    let key = thread_key(n);
    if state.is_empty() {
        m.config.remove(&key.as_str().into());
    } else {
        m.config.insert(key.as_str().into(), Tag::Compound(state));
    }
}
/// A thread that takes a job (`CraftingThread.acceptJob`, AE2's
/// `MolecularAssemblerBlockEntity.isBusy`): nothing in it, no pattern, no
/// fluid held for it.
pub(super) fn free(t: &Machine) -> bool {
    t.slots.iter().all(Option::is_none)
        && !t.config.contains_key(&"ae2_assembler_pattern".into())
        && !t.config.contains_key(&"ae2_assembler_fluid_escrow".into())
}
/// The first thread of `m` that takes a job and that `taken` does not name
/// (`TileExMolecularAssembler.pushPattern`: the first thread that accepts).
pub(super) fn free_thread(m: &Machine, taken: impl Fn(u8) -> bool) -> Option<u8> {
    (0..threads(m))
        .filter_map(|n| Some((u8::try_from(n).ok()?, thread(m, n)?)))
        .find(|(n, t)| !taken(*n) && free(t))
        .map(|(n, _)| n)
}
/// Whether thread `t` has anything to do (`CraftingThread.isAwake`): a job or
/// something in its grid or output.
pub(super) fn awake(t: &Machine) -> bool {
    t.slots.iter().take(THREAD_SLOTS).any(Option::is_some) || t.config.contains_key(&"ae2_assembler_pattern".into())
}
/// The threads of `m` with something to do.
pub(super) fn busy_threads(m: &Machine) -> usize {
    (0..threads(m)).filter_map(|n| thread(m, n)).filter(awake).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use horizon_client::items::ItemStack;

    #[test]
    fn a_thread_past_the_first_keeps_its_own_slots_and_job() {
        let mut m = Machine::new(MachineKind::MolecularAssembler);
        let mut t = Machine::new(MachineKind::MolecularAssembler);
        t.slots[0] = Some(ItemStack::new("minecraft:oak_planks", 1));
        t.slots[9] = Some(ItemStack::new("minecraft:stick", 4));
        t.progress = 40.0;
        t.config.insert("ae2_assembler_pattern".into(), Tag::String("p".into()));
        t.config.insert("ae2_flight_owner".into(), Tag::String("o".into()));
        store_thread(&mut m, 3, &t);
        assert!(m.slots.iter().all(Option::is_none), "thread 0's slots untouched");
        assert!(!m.config.contains_key(&"ae2_assembler_pattern".into()), "thread 0's keys untouched");
        let Some(Tag::Compound(state)) = m.config.get(&"ae2_assembler_thread_3".into()) else { panic!("thread state") };
        assert_eq!(state.get(&"ae2_flight_owner".into()), Some(&Tag::String("o".into())));
        // What a broken assembler drops includes the thread's items.
        let drops = m.physical_contents();
        assert_eq!(drops.len(), 2);
        assert!(drops.iter().any(|s| s.item.as_ref() == "minecraft:stick" && s.count == 4));
        // AE2's own assembler has one thread: thread 3 is not read back.
        assert!(thread(&m, 3).is_none());
        // Emptied, the thread leaves no state behind.
        store_thread(&mut m, 3, &Machine::new(MachineKind::MolecularAssembler));
        assert!(!m.config.contains_key(&"ae2_assembler_thread_3".into()));
    }

    #[test]
    fn ae2s_assembler_is_its_own_single_thread() {
        let mut m = Machine::new(MachineKind::MolecularAssembler);
        assert_eq!(threads(&m), 1);
        assert_eq!(threads(&Machine::new(MachineKind::Drive)), 0);
        m.slots[10] = Some(ItemStack::new("ae2:crafting_pattern", 1));
        m.config.insert("unrelated".into(), Tag::Byte(1));
        let mut t = thread(&m, 0).unwrap();
        assert!(!free(&t), "a pattern in its slot keeps it from jobs");
        assert!(thread(&m, 1).is_none());
        t.slots[0] = Some(ItemStack::new("minecraft:stone", 1));
        t.progress = 12.0;
        t.config.insert("ae2_assembler_pattern".into(), Tag::String("p".into()));
        store_thread(&mut m, 0, &t);
        assert_eq!(m.progress, 12.0);
        assert!(m.config.contains_key(&"ae2_assembler_pattern".into()));
        assert!(m.config.contains_key(&"unrelated".into()));
        assert_eq!(busy_threads(&m), 1);
        t.config.remove(&"ae2_assembler_pattern".into());
        t.slots[0] = None;
        store_thread(&mut m, 0, &t);
        assert!(!m.config.contains_key(&"ae2_assembler_pattern".into()));
        assert_eq!(busy_threads(&m), 0);
    }
}
