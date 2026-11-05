//! AE2WTLib's Quantum Bridge Card for a wireless terminal
//! (`WTMenuHost.isQuantumLinked`, `findQuantumBridge`, `getActionableNode`,
//! `getPowerDrainPerTick`, `consumeIdlePower`'s `recharge`), AE2WTLib 19.5.1
//! by Mari023, https://github.com/Mari023/AE2WirelessTerminalLibrary tag
//! `19.5.1`, MIT; see `ae2/addons/ae2wtlib/`. The item's rules are
//! `horizon_client::ae2x::ae2wtlib::quantum`; the bridge is found in AE2's
//! quantum registry, which is the server's, so the terminal reaches it from
//! any level.
use crate::game::{Dim, Game, ae2_global::GridAddress, ae2_quantum, ae2_wireless::{self, Reach}, lan::LanHost};
use horizon_client::{
    ae2_world::MachineKind,
    ae2x::ae2wtlib::quantum::{self, Status},
    containers::BlockEntity,
    items::ItemStack,
};

impl Game<'_> {
    /// `isQuantumLinked`: the card, the singularity's frequency, the bridge
    /// registered under it or its negation (`findQuantumBridge`), its node on
    /// a grid, that grid the one of the access point the item is linked to
    /// when that one is there (`getLinkedGrid`), and powered. The bridge's
    /// center, the node the terminal then acts through, or why not.
    pub(in crate::game) fn ae2wtlib_quantum_link(&self, stack: &ItemStack) -> Result<GridAddress, Status> {
        let frequency = quantum::frequency(stack, ae2_quantum::frequency)?;
        let bridge = self
            .ae2_quantum_bridge(frequency)
            .or_else(|| self.ae2_quantum_bridge(frequency.wrapping_neg()))
            .ok_or(Status::BridgeNotFound)?;
        let network = self.ae2_network();
        let analysis = network.analyze().map_err(|_| Status::BridgeNotFound)?;
        let node = analysis.nodes.get(&bridge.network_pos()).ok_or(Status::BridgeNotFound)?;
        let linked = ae2_wireless::target(stack).and_then(|(dim, pos)| {
            let level = Dim::from_id(&dim);
            let address = self.ae2_dimension_address(level, pos).filter(|_| level.id() == dim)?;
            matches!(self.ae2_block_entity(address), Some(BlockEntity::Ae2(m)) if m.kind == MachineKind::WirelessAccessPoint)
                .then(|| analysis.nodes.get(&address.network_pos()))
                .flatten()
        });
        if linked.is_some_and(|access| access.component != node.component) {
            return Err(Status::DifferentNetworks);
        }
        if !node.powered {
            return Err(Status::NotPowered);
        }
        Ok(bridge)
    }
    /// The bridge's link when it is connected: it takes over from the access
    /// points (`getActionableNode`), drains 22.5 AE a tick and recharges the
    /// item.
    pub(in crate::game) fn ae2wtlib_quantum_reach(&self, stack: &ItemStack) -> Option<Reach> {
        let node = self.ae2wtlib_quantum_link(stack).ok()?;
        Some(Reach { node, drain: quantum::DRAIN, recharge: true })
    }
    /// `WTMenuHost.recharge`: what the battery of the item at guest `conn`'s
    /// `source` slot misses, from the grid of `node` (`PowerMultiplier.ONE`),
    /// never taking the grid below half of what it can store.
    pub(in crate::game) fn ae2wtlib_recharge(&mut self, host: &mut LanHost, conn: u32, source: usize, node: GridAddress) {
        let Some(stack) = host.hub.guests.iter().find(|g| g.conn == conn).and_then(|g| ae2_wireless::source_stack(g, source)) else {
            return;
        };
        let Ok((power, capacity)) = ae2_wireless::energy(stack) else { return };
        let Some((stored, max)) = self.ae2_energy_levels(node) else { return };
        let Some(amount) = quantum::recharge_amount(capacity - power, stored, max) else { return };
        let extracted = self.ae2_take_power_encoded_raw(node, amount, false);
        if extracted <= 0.0 {
            return;
        }
        if let Some(stack) = host.hub.guests.iter_mut().find(|g| g.conn == conn).and_then(|g| ae2_wireless::source_stack_mut(g, source)) {
            let _ = ae2_wireless::transfer_energy(stack, extracted, true, false);
        }
    }
}
