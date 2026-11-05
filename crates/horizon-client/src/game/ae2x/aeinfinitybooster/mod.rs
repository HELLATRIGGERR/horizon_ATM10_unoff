//! AEInfinityBooster 2.1.0 in the game (feature `ae2-aeinfinitybooster`):
//! an access point holding a card keeps its chunk loaded. Derived from
//! AEInfinityBooster by Hexeption (`MixinWirelessBlockEntity`,
//! `MixinWirelessBlockEntityRemove`), https://github.com/Hexeption/AEInfinityBooster
//! tag `v2.1.0`, LGPL-3.0; see `ae2/addons/aeinfinitybooster/`. The card's
//! signal and the slot are the registry's rows (`horizon_client::ae2x::aeinfinitybooster`).
use super::super::Game;
use horizon_client::{ae2_world::MachineKind, containers::BlockEntity};
use std::collections::BTreeMap;

/// What one access point did about its chunk: `ib$forcedChunk`, whether it
/// is what keeps the chunk forced.
#[derive(Clone, Copy, Debug, Default)]
struct AccessPoint {
    forced: bool,
}

#[derive(Default)]
pub(in crate::game) struct State {
    /// The access points seen, by dimension and position.
    seen: BTreeMap<(String, (i32, i32, i32)), AccessPoint>,
    next: u64,
}

/// The chunk an access point keeps loaded while it holds a card, when the
/// next check comes, and what it does on each change: `onReady` (first seen
/// with a card: the chunk is forced and its own, as it usually still is from
/// before the save), `saveChanges` (a card put in forces the chunk, its own
/// only if nothing else had forced it; a card taken out releases its own),
/// `setRemoved` (gone: its own chunk released). It never releases a chunk
/// another forced, such as by `/forceload`.
fn next(point: Option<AccessPoint>, card: bool, enabled: bool, newly_forced: impl FnOnce(bool) -> bool) -> AccessPoint {
    match point {
        None => {
            if enabled && card {
                newly_forced(true);
                AccessPoint { forced: true }
            } else {
                AccessPoint { forced: false }
            }
        }
        Some(point) => {
            let force = enabled && card;
            if force == point.forced {
                return point;
            }
            let changed = newly_forced(force);
            AccessPoint { forced: force && changed }
        }
    }
}

impl Game<'_> {
    /// Once a second: the access points' chunks, as the card's mixins keep them.
    pub(in crate::game) fn tick_aeinfinitybooster(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let now = self.world.game_time.max(0) as u64;
        if now < self.ae2x.aeinfinitybooster.next {
            return;
        }
        self.ae2x.aeinfinitybooster.next = now + 20;
        let enabled = horizon_client::ae2x::aeinfinitybooster::Config::get().chunk_loading;
        let defs = horizon_client::ae2x::defs::get();
        let dim = self.dim.id().to_owned();
        let present: BTreeMap<(i32, i32, i32), bool> = self
            .world
            .block_entities
            .iter()
            .filter_map(|(&pos, entity)| match entity {
                BlockEntity::Ae2(m) if m.kind == MachineKind::WirelessAccessPoint => Some((
                    pos,
                    m.slots.first().and_then(Option::as_ref).and_then(|s| defs.access_point_card(&s.item)).is_some_and(|c| c.chunk_loading),
                )),
                _ => None,
            })
            .collect();
        let state = std::mem::take(&mut self.ae2x.aeinfinitybooster.seen);
        let mut seen = BTreeMap::new();
        for (key, point) in state {
            if key.0 != dim || present.contains_key(&key.1) {
                seen.insert(key, point);
            } else if point.forced {
                // setRemoved: its own chunk, released.
                self.streaming.forced.remove(&(key.1.0 >> 4, key.1.2 >> 4));
            }
        }
        for (pos, card) in present {
            let chunk = (pos.0 >> 4, pos.2 >> 4);
            let key = (dim.clone(), pos);
            let streaming = &mut self.streaming;
            let point = next(seen.get(&key).copied(), card, enabled, |force| {
                if force {
                    let changed = streaming.forced.insert(chunk);
                    streaming.rank_forced(chunk.0, chunk.1, chunk.0, chunk.1);
                    changed
                } else {
                    streaming.forced.remove(&chunk)
                }
            });
            seen.insert(key, point);
        }
        self.ae2x.aeinfinitybooster.seen = seen;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_access_point_forces_and_releases_only_its_own_chunk() {
        // First seen with a card: forced and its own, whoever forced it before.
        let first = next(None, true, true, |force| { assert!(force); false });
        assert!(first.forced);
        // The card taken out: released.
        let mut released = false;
        let out = next(Some(first), false, true, |force| { assert!(!force); released = true; true });
        assert!(released && !out.forced);
        // A card put in where /forceload already holds the chunk: not its own.
        let in_again = next(Some(out), true, true, |_| false);
        assert!(!in_again.forced);
        // So taking it out leaves the chunk alone.
        let left = next(Some(in_again), false, true, |_| panic!("not its chunk"));
        assert!(!left.forced);
        // Chunk loading off in the config: nothing forced.
        assert!(!next(None, true, false, |_| panic!("disabled")).forced);
        // A card put in while nothing held the chunk: its own.
        assert!(next(Some(AccessPoint::default()), true, true, |_| true).forced);
    }
}
