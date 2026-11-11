//! Applied Flux's Induction Card on an ME Interface or a Pattern Provider,
//! block or part (`ItemInductionCard`, `EnergyTicker`, `MixinInterfaceLogic`,
//! `MixinPatternProviderLogic`, `AFUtil.getSides`): while the host's node is
//! active and the card is installed, the host pushes its grid's FE into the
//! FE blocks beside it, on all six sides of a block or on the side of a part,
//! each side at the pace of its `EnergyTickRecord`, as the Flux Accessor does
//! (`EnergyHandler.send`, at most `flux_accessor.io_limit`; what the block
//! refuses goes back). The FE the host takes in through its generic slots is
//! `generic.rs`'s. Derived from Applied Flux by GlodBlock,
//! https://github.com/GlodBlock/ExtendedAE branch `appflux/1.21.1-neoforge`
//! commit `523c3e5a`, LGPL-3.0; see `ae2/addons/appflux/`.
use super::{
    accessor::{DIRECTIONS, face_of},
    generic::induction_card,
};
use super::super::super::{Game, ae2_parts::PartAddress};
use horizon_client::{
    ae2_world::{Machine, MachineKind},
    ae2x::appflux::{INDUCTION_CARD, TickRecord},
    containers::BlockEntity,
};
use horizon_core::geometry::Direction;
use horizon_world::nbt::Tag;
use std::collections::{HashMap, HashSet};

/// The pace of each side a host pushes on, by its position and the side
/// (`lastTick`); none of it is saved.
#[derive(Default)]
pub(in crate::game) struct State {
    ticks: HashMap<((i32, i32, i32), u8), TickRecord>,
}

/// A host of `EnergyTicker`: AE2's interface and pattern provider logic.
fn host(machine: &Machine) -> bool {
    matches!(machine.kind, MachineKind::Interface | MachineKind::PatternProvider)
}

/// Whether a part's saved record holds an Induction Card among its upgrades:
/// the parts worth a closer look.
fn part_has_card(part: &Tag) -> bool {
    let Some(machine) = part.get("machine") else { return false };
    matches!(machine.get("upgrades"), Some(Tag::List(cards))
        if cards.iter().any(|card| card.get("id") == Some(&Tag::String(INDUCTION_CARD.into()))))
}

/// `AFUtil.getSides`: every side of a block, the part's own side.
fn sides(part: Option<u8>) -> Vec<Direction> {
    match part {
        None => DIRECTIONS.to_vec(),
        Some(face) => DIRECTIONS.get(usize::from(face)).map_or_else(Vec::new, |side| vec![*side]),
    }
}

impl Game<'_> {
    /// `EnergyDistributeService.onLevelEndTick` for the woken `EnergyTicker`s
    /// (`updateSleep`: the card installed): each active host pushes its grid's
    /// FE on its sides when the side's record says.
    pub(in crate::game) fn tick_appflux_induction(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let mut found: Vec<((i32, i32, i32), Option<u8>)> = Vec::new();
        for (&pos, entity) in &self.world.block_entities {
            let BlockEntity::Ae2(machine) = entity else { continue };
            if host(machine) && induction_card(machine) {
                found.push((pos, None));
            } else if machine.kind == MachineKind::CableBus
                && let Some(Tag::List(parts)) = machine.config.get(&"parts".into())
            {
                for part in parts.iter().filter(|part| part_has_card(part)) {
                    let Some(Tag::String(face)) = part.get("face") else { continue };
                    if let Some(face) = super::super::super::ae2_grid::face_index(&face.to_string_lossy()) {
                        found.push((pos, Some(face)));
                    }
                }
            }
        }
        if found.is_empty() {
            self.ae2x.appflux.induction.ticks.clear();
            return;
        }
        let network = self.ae2_network();
        let Ok(analysis) = network.analyze() else { return };
        let now = self.world.game_time;
        let mut live = HashSet::new();
        for (pos, part) in found {
            let Some(address) = self.ae2_local_address(pos) else { continue };
            if let Some(face) = part {
                let Ok(Some(machine)) = self.ae2_part_machine(PartAddress { host: address, face }) else { continue };
                if !host(&machine) || !induction_card(&machine) {
                    continue;
                }
            }
            // isActive: powered, with its channel.
            let node = analysis.nodes.get(&address.network_pos());
            let active = match part {
                None => node.is_some_and(|n| n.online),
                Some(face) => node.and_then(|n| n.parts.get(&face)).is_some_and(|p| p.online),
            };
            for side in sides(part) {
                let key = (pos, face_of(side));
                live.insert(key);
                if !active {
                    continue;
                }
                let mut record = self.ae2x.appflux.induction.ticks.get(&key).copied().unwrap_or_default();
                if record.need_tick(now)
                    && let Some(sent) = self.appflux_send(address, pos, side)
                {
                    record.sent(sent);
                }
                self.ae2x.appflux.induction.ticks.insert(key, record);
            }
        }
        self.ae2x.appflux.induction.ticks.retain(|key, _| live.contains(key));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_sides_and_saved_cards_follow_the_ticker() {
        horizon_client::ae2x::install();
        assert!(host(&Machine::new(MachineKind::Interface)) && host(&Machine::new(MachineKind::PatternProvider)));
        assert!(!host(&Machine::new(MachineKind::Drive)));
        assert_eq!(sides(None).len(), 6);
        assert_eq!(sides(Some(5)), vec![Direction::North]);
        assert!(sides(Some(9)).is_empty());
        let card = |id: &str| Tag::compound([("Slot", Tag::Byte(1)), ("id", Tag::String(id.into())), ("count", Tag::Int(1))]);
        let part = |cards: Vec<Tag>| Tag::compound([("face", Tag::String("east".into())), ("machine", Tag::compound([("upgrades", Tag::List(cards))]))]);
        assert!(part_has_card(&part(vec![card(INDUCTION_CARD)])));
        assert!(!part_has_card(&part(vec![card("ae2:fuzzy_card")])));
        assert!(!part_has_card(&Tag::compound([("face", Tag::String("east".into()))])));
    }
}
