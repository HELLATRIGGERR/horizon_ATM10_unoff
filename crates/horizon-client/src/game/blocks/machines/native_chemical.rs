//! The engine's own blocks as Mekanism's chemical handler, for native mods
//! (API 437): AE2's generic inventories (`GENERIC_INTERNAL_INV`: an ME
//! Interface's storage, a Pattern Provider's return slots, blocks and parts),
//! as Applied Mekanistics 1.6.3's `GenericStackChemicalStorage` (by ramidzkh,
//! LGPL-3.0, see `ae2/addons/appmek/`) answers `Capabilities.CHEMICAL` there:
//! one tank a slot, the slot's key when it is a chemical, the type's slot
//! capacity. The engine names no chemical: an addon registers the key type
//! whose keys are chemicals by their registry id
//! (`ae2_key_types::register_chemical_type`); without one, or without AE2,
//! nothing here answers. API 435 is the other way: AE2 asking a mod's block.
use super::Game;
use super::native_fe::dim_of;
use mod_api::NativeChemicalTankInfo;
use horizon_core::geometry::Direction;

#[cfg(feature = "ae2-lab")]
mod ae2 {
    use super::{Direction, Game, NativeChemicalTankInfo};
    use crate::game::{ae2_global::GridAddress, ae2_interface};
    use ae2_cell_core::{ItemKey, KeyType};
    use horizon_client::{ae2_key_types as key_types, ae2_world::MachineKind, containers::BlockEntity};
    use horizon_core::nbt::Tag;

    /// Which generic inventory a side reaches.
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(super) enum Inventory {
        /// `InterfaceLogic.getStorage()`: inserts and extracts.
        Interface,
        /// `PatternProviderReturnInventory`: inserts only (`canExtract` false); its tick returns
        /// what it holds to the grid.
        Provider,
    }

    /// The chemical `key` names: its id when it is a key of the chemical type `kind`.
    fn chemical(kind: KeyType, key: &ItemKey) -> Option<&str> {
        let (type_id, id) = key_types::marker_parts(key)?;
        (type_id == kind.id() && key.components.is_empty()).then_some(id)
    }
    /// The chemical as the fluid lane names it.
    pub(super) fn marker(kind: KeyType, chemical: &str) -> ItemKey {
        key_types::addon_marker(kind, &ItemKey { item_id: chemical.to_owned(), components: Vec::new() })
    }
    /// One tank of a slot: its chemical, or an empty tank.
    fn tank(kind: KeyType, slot: Option<&(ItemKey, u64)>, capacity: u64, extractable: bool) -> NativeChemicalTankInfo {
        match slot.and_then(|(key, n)| Some((chemical(kind, key)?, *n))) {
            Some((id, n)) if n > 0 => NativeChemicalTankInfo {
                chemical: id.to_owned(),
                amount: n.min(i64::MAX as u64) as i64,
                capacity: capacity.min(i64::MAX as u64) as i64,
                extractable,
            },
            _ => NativeChemicalTankInfo { capacity: capacity.min(i64::MAX as u64) as i64, ..Default::default() },
        }
    }
    /// A provider's return slots of the fluid lane.
    fn provider_lane(machine: &horizon_client::ae2_world::Machine) -> Vec<Option<(ItemKey, u64)>> {
        let mut lane = vec![None; 9];
        if let Some(Tag::List(entries)) = machine.config.get(&"ae2_provider_return_fluids".into()) {
            for entry in entries {
                if let (Some(Tag::Byte(slot)), Some(key), Some(Tag::Long(n))) =
                    (entry.get("Slot"), key_types::lane_entry_key(entry, "fluid"), entry.get("amount"))
                    && let Some(place) = lane.get_mut(*slot as usize)
                {
                    *place = u64::try_from(*n).ok().map(|n| (key, n));
                }
            }
        }
        lane
    }

    impl Game<'_> {
        /// The address and side of the generic inventory at `pos` in `dimension` through
        /// `face`; none for a block that is no AE2 block, and for a cable bus's null side.
        pub(super) fn chemical_inventory_at(&self, dimension: &str, pos: [i32; 3], face: Option<Direction>) -> Option<(GridAddress, Direction, Inventory)> {
            let address = self.ae2_dimension_address(super::dim_of(dimension)?, (pos[0], pos[1], pos[2]))?;
            let Some(BlockEntity::Ae2(machine)) = self.ae2_block_entity(address) else { return None };
            if face.is_none() && machine.kind == MachineKind::CableBus {
                return None;
            }
            // A block's own inventory is the same on every side.
            let face = face.unwrap_or(Direction::Up);
            if self.ae2_interface_machine_at(address, face).is_some() {
                return Some((address, face, Inventory::Interface));
            }
            self.ae2_provider_return_machine_at(address, face)
                .filter(|m| m.kind == MachineKind::PatternProvider)
                .map(|_| (address, face, Inventory::Provider))
        }
        /// `getChemicalTanks` / `getChemicalInTank` / `getChemicalTankCapacity` there.
        pub(super) fn chemical_inventory_tanks(&self, address: GridAddress, face: Direction, inventory: Inventory) -> Option<Vec<NativeChemicalTankInfo>> {
            let kind = key_types::chemical_type()?;
            let any = marker(kind, "");
            Some(match inventory {
                Inventory::Interface => {
                    let machine = self.ae2_interface_machine_at(address, face)?;
                    let stock = ae2_interface::interface_fluid_stock(&machine).ok()?;
                    let capacity = ae2_interface::interface_key_capacity(&machine, &any);
                    (0..machine.slots.len())
                        .map(|i| tank(kind, stock.get(i).and_then(Option::as_ref).filter(|_| machine.slots[i].is_none()), capacity, true))
                        .collect()
                }
                Inventory::Provider => {
                    let machine = self.ae2_provider_return_machine_at(address, face)?;
                    let capacity = key_types::lane_capacity(&any);
                    provider_lane(&machine).iter().map(|slot| tank(kind, slot.as_ref(), capacity, false)).collect()
                }
            })
        }
        /// `insertChemical` (with `insert`) or `extractChemical(stack)` of `amount` of
        /// `chemical` there, slot by slot as AE2's inventory moves a key: what moved.
        #[allow(clippy::too_many_arguments)]
        pub(super) fn chemical_inventory_move(&mut self, address: GridAddress, face: Direction, inventory: Inventory, chemical: &str, amount: u64, insert: bool, simulate: bool) -> u64 {
            let Some(kind) = key_types::chemical_type() else { return 0 };
            let key = marker(kind, chemical);
            match (inventory, insert) {
                (Inventory::Interface, _) => self.ae2_interface_storage_move(address, face, &key, amount, insert, simulate),
                (Inventory::Provider, true) => self.ae2_provider_return_fluid_at(address, face, &key, amount, simulate),
                (Inventory::Provider, false) => Some(0),
            }
            .unwrap_or(0)
            .min(amount)
        }
    }

}

/// Chemical handler helpers for native mods. API 437.
#[allow(dead_code)]
impl Game<'_> {
    /// The chemical handler of the engine's block at `pos` in `dimension`
    /// through `face` (the side of that block you touch; none is the block
    /// itself), as Mekanism's `IChemicalHandler` reads it: AE2's ME Interface
    /// (its storage slots) and Pattern Provider (its return slots), blocks
    /// and parts on that side of a cable bus, one tank a slot
    /// (`NativeChemicalTankInfo`: the slot's chemical or empty, the type's
    /// slot capacity, whether it gives it back; a provider's slots give
    /// nothing back). None for any other block, where no addon registered a
    /// chemical key type, in a level that isn't open and on a client's copy
    /// of the world: a mod's own blocks answer API 435's hook points
    /// instead. API 437.
    pub(in crate::game) fn native_chemical_tanks(
        &self,
        dimension: &str,
        pos: [i32; 3],
        face: Option<Direction>,
    ) -> Option<Vec<NativeChemicalTankInfo>> {
        if !self.native_simulates() {
            return None;
        }
        #[cfg(feature = "ae2-lab")]
        {
            let (address, face, inventory) = self.chemical_inventory_at(dimension, pos, face)?;
            self.chemical_inventory_tanks(address, face, inventory)
        }
        #[cfg(not(feature = "ae2-lab"))]
        {
            let _ = (dim_of(dimension), pos, face);
            None
        }
    }

    /// `IChemicalHandler.insertChemical` of `amount` of `chemical` (its
    /// registry id) on the block at `pos` in `dimension` through `face`, as
    /// `native_chemical_tanks` finds it: what it took (with `simulate`, would
    /// take), 0 to `amount`. An interface keeps it in its slots (its tick
    /// hands what no config asks for to the grid); a provider returns it to
    /// its grid. API 437.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::game) fn native_chemical_insert(
        &mut self,
        dimension: &str,
        pos: [i32; 3],
        face: Option<Direction>,
        chemical: &str,
        amount: i64,
        simulate: bool,
    ) -> i64 {
        self.native_chemical_move(dimension, pos, face, chemical, amount, true, simulate)
    }

    /// `IChemicalHandler.extractChemical(stack, action)` there, as
    /// `native_chemical_insert`: at most `amount` of `chemical` and only that
    /// one; what left. A provider's return slots give nothing. API 437.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::game) fn native_chemical_extract(
        &mut self,
        dimension: &str,
        pos: [i32; 3],
        face: Option<Direction>,
        chemical: &str,
        amount: i64,
        simulate: bool,
    ) -> i64 {
        self.native_chemical_move(dimension, pos, face, chemical, amount, false, simulate)
    }

    #[allow(clippy::too_many_arguments)]
    fn native_chemical_move(
        &mut self,
        dimension: &str,
        pos: [i32; 3],
        face: Option<Direction>,
        chemical: &str,
        amount: i64,
        insert: bool,
        simulate: bool,
    ) -> i64 {
        if !self.native_simulates() || amount <= 0 || chemical.is_empty() {
            return 0;
        }
        #[cfg(feature = "ae2-lab")]
        {
            let Some((address, face, inventory)) = self.chemical_inventory_at(dimension, pos, face) else {
                return 0;
            };
            self.chemical_inventory_move(address, face, inventory, chemical, amount as u64, insert, simulate) as i64
        }
        #[cfg(not(feature = "ae2-lab"))]
        {
            let _ = (dim_of(dimension), pos, face, insert, simulate);
            0
        }
    }
}
