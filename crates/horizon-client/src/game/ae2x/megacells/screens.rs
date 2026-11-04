//! MEGA Cells' screens on the AE2 layout system: `PortableCellWorkbenchScreen`,
//! AE2's `CellWorkbenchScreen` over `PortableCellWorkbenchMenu` with its own
//! `screens/megacells/portable_cell_workbench.json` (which includes
//! `cell_workbench.json` and names the item as its title). Derived from MEGA
//! Cells 4.11.0, https://github.com/62832/MEGACells tag 4.11.0, LGPL-3.0; see
//! `ae2/addons/megacells/`.
use super::{Layout, MachineKind, icon};

const LAYOUTS: &[(&str, Layout)] = &[
    // The cell slot and the cell's cards; the filters and the toolbar's
    // buttons are `ae2_workbench_screen`'s, as on the Cell Workbench.
    (horizon_client::ae2x::megacells::PORTABLE_CELL_WORKBENCH, Layout {
        kind: MachineKind::CellWorkbench,
        style: "megacells/portable_cell_workbench.json",
        slots: &[(0, "STORAGE_CELL")],
        icons: &[(0, icon::BACKGROUND_STORAGE_CELL)],
        upgrades: 1..5,
        toggles: &[],
        progress: None,
        priority: None,
        decorations: &[],
        grid: None,
        cogs: &[],
        lock_line: None,
        page_line: None,
        panels: &[],
    }),
];

/// The layout of a MEGA screen that goes by its menu's item or block.
pub(super) fn layout(block: &str) -> Option<&'static Layout> {
    LAYOUTS.iter().find(|(id, _)| *id == block).map(|(_, layout)| layout)
}
