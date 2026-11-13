//! AE2 addons on the port: the shared definitions registry ([`defs`]) and each
//! addon's rows, one module per addon behind its `ae2-<id>` feature
//! (`pack/pack.toml`). An addon's game behaviour, when it has any, lives in
//! `game/ae2x/<id>/`.
pub mod defs;
pub mod rules;
pub mod kits;
pub mod wireless;
#[cfg(feature = "ae2-megacells")]
pub mod megacells;
#[cfg(feature = "ae2-merequester")]
pub mod merequester;
#[cfg(feature = "ae2-extendedae")]
pub mod extendedae;
#[cfg(feature = "ae2-ae2wtlib")]
pub mod ae2wtlib;
#[cfg(feature = "ae2-importexportcard")]
pub mod importexportcard;
#[cfg(feature = "ae2-appflux")]
pub mod appflux;
#[cfg(feature = "ae2-advancedae")]
pub mod advancedae;

/// Builds the registry, which installs the addon machines' block ids and
/// the variants of AE2's machines: call before any world or block entity
/// loads.
pub fn install() {
    let _ = defs::get();
}

/// What an addon machine does not save, cleared as a save loads (each
/// addon's `reset_unsaved` on its own machines).
pub fn reset_unsaved(m: &mut crate::ae2_world::Machine) {
    #[cfg(feature = "ae2-extendedae")]
    extendedae::reset_unsaved(m);
    #[cfg(feature = "ae2-advancedae")]
    advancedae::reset_unsaved(m);
    let _ = m;
}

/// A cell that holds one key without end and takes it in to void it
/// (ExtendedAE's `ItemInfinityCell`): to the port's storage code a creative
/// cell of that key, of its own idle drain and drive model.
#[derive(Clone, Debug, PartialEq)]
pub struct FixedCell {
    /// The key's id, of `key_type`.
    pub key: String,
    pub key_type: ae2_cell_core::KeyType,
    /// `getIdleDrain`, AE/t.
    pub idle_drain: f64,
    /// The amount `getAvailableStacks` reports, in the key's units (mB for a
    /// fluid): ExtendedAE's `Integer.MAX_VALUE * getAmountPerUnit()`.
    pub amount: u64,
}
/// The fixed cell of item `id`; none for any other item.
pub fn fixed_cell(id: &str) -> Option<FixedCell> {
    #[cfg(feature = "ae2-extendedae")]
    if let Some(cell) = extendedae::infinity_cell(id) {
        let per_unit = cell.key_type.amount_per_unit();
        return Some(FixedCell { key: cell.record, key_type: cell.key_type, idle_drain: extendedae::Config::get().infinity_cell_cost, amount: i32::MAX as u64 * per_unit });
    }
    let _ = id;
    None
}
/// The model a drive draws for a fixed cell (`StorageCellModels`).
pub fn fixed_cell_model(id: &str) -> Option<&'static str> {
    #[cfg(feature = "ae2-extendedae")]
    if let Some(model) = extendedae::infinity_cell_model(id) {
        return Some(model);
    }
    let _ = id;
    None
}

/// `StorageCell.canFitInsideCell` of a cell of an addon's own handler (MEGA's
/// Bulk Item Cell: empty and unfiltered); none for any other stack.
pub fn handler_cell_fits_inside(stack: &crate::items::ItemStack) -> Option<bool> {
    #[cfg(feature = "ae2-megacells")]
    if stack.item.as_ref() == megacells::BULK_ITEM_CELL {
        return Some(megacells::bulk_fits_inside(stack));
    }
    // ExtendedAE's infinity and void cells keep the interface's default: no.
    if fixed_cell(&stack.item).is_some() {
        return Some(false);
    }
    #[cfg(feature = "ae2-extendedae")]
    if stack.item.as_ref() == extendedae::VOID_CELL {
        return Some(false);
    }
    // EnderDiskInventory keeps StorageCell's default: no (a row of the addon's, `Defs::add_cell_kept_out_of_cells`).
    if defs::get().is_cell_kept_out_of_cells(&stack.item) {
        return Some(false);
    }
    None
}

/// `getStatus` of a cell of an addon's own handler, as `CellState`'s LED
/// number (1 empty, 2 not empty, 3 types full, 4 full); none for others.
pub fn handler_cell_state(stack: &crate::items::ItemStack) -> Option<u8> {
    // InfinityCellInventory and VoidCellInventory.getStatus: NOT_EMPTY.
    if fixed_cell(&stack.item).is_some() {
        return Some(2);
    }
    #[cfg(feature = "ae2-extendedae")]
    if stack.item.as_ref() == extendedae::VOID_CELL {
        return Some(2);
    }
    // FluxCellInventory.getStatus.
    #[cfg(feature = "ae2-appflux")]
    if let Some(cell) = appflux::fe_cell(stack) {
        return Some(match cell.status() {
            appflux::FeCellState::Empty => 1,
            appflux::FeCellState::NotEmpty => 2,
            appflux::FeCellState::Full => 4,
        });
    }
    #[cfg(feature = "ae2-megacells")]
    if stack.item.as_ref() == megacells::BULK_ITEM_CELL {
        let mut cell = megacells::bulk_cell(stack, &megacells::chains(), &|_, _| true).ok()?;
        return Some(match cell.status() {
            ae2_megacells_core::CellState::Empty => 1,
            ae2_megacells_core::CellState::NotEmpty => 2,
            ae2_megacells_core::CellState::Full => 4,
        });
    }
    let _ = stack;
    None
}

/// The keys `getAvailableStacks` of a cell of an addon's own handler lists,
/// in its order; none for any other stack.
pub fn handler_cell_keys(stack: &crate::items::ItemStack, known: &dyn Fn(&str,ae2_cell_core::KeyType)->bool) -> Option<Vec<ae2_cell_core::ItemKey>> {
    #[cfg(feature = "ae2-megacells")]
    if stack.item.as_ref() == megacells::BULK_ITEM_CELL {
        let mut cell = megacells::bulk_cell(stack, &megacells::chains(), known).ok()?;
        return Some(cell.available_stacks().into_iter().map(|(key, _)| key).collect());
    }
    let _ = (stack, known);
    None
}
