//! AE2 addons on the port: the shared definitions registry ([`defs`]) and each
//! addon's rows, one module per addon behind its `ae2-<id>` feature
//! (`pack/pack.toml`). An addon's game behaviour, when it has any, lives in
//! `game/ae2x/<id>/`.
pub mod defs;
#[cfg(feature = "ae2-megacells")]
pub mod megacells;
#[cfg(feature = "ae2-merequester")]
pub mod merequester;
#[cfg(feature = "ae2-extendedae")]
pub mod extendedae;

/// Builds the registry, which installs the addon machines' block ids and
/// the variants of AE2's machines: call before any world or block entity
/// loads.
pub fn install() {
    let _ = defs::get();
}

/// `StorageCell.canFitInsideCell` of a cell of an addon's own handler (MEGA's
/// Bulk Item Cell: empty and unfiltered); none for any other stack.
pub fn handler_cell_fits_inside(stack: &crate::items::ItemStack) -> Option<bool> {
    #[cfg(feature = "ae2-megacells")]
    if stack.item.as_ref() == megacells::BULK_ITEM_CELL {
        return Some(megacells::bulk_fits_inside(stack));
    }
    let _ = stack;
    None
}

/// `getStatus` of a cell of an addon's own handler, as `CellState`'s LED
/// number (1 empty, 2 not empty, 3 types full, 4 full); none for others.
pub fn handler_cell_state(stack: &crate::items::ItemStack) -> Option<u8> {
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
pub fn handler_cell_keys(stack: &crate::items::ItemStack, known: &dyn Fn(&str, bool) -> bool) -> Option<Vec<ae2_cell_core::ItemKey>> {
    #[cfg(feature = "ae2-megacells")]
    if stack.item.as_ref() == megacells::BULK_ITEM_CELL {
        let mut cell = megacells::bulk_cell(stack, &megacells::chains(), known).ok()?;
        return Some(cell.available_stacks().into_iter().map(|(key, _)| key).collect());
    }
    let _ = (stack, known);
    None
}
