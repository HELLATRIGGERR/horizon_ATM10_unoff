//! MEGA Cells' screens on the AE2 layout system: `PortableCellWorkbenchScreen`,
//! AE2's `CellWorkbenchScreen` over `PortableCellWorkbenchMenu` with its own
//! `screens/megacells/portable_cell_workbench.json` (which includes
//! `cell_workbench.json` and names the item as its title); and
//! `MEGACellsClient`'s `PatternProviderScreen` and `InterfaceScreen` over
//! AE2's `PatternProviderMenu` and `InterfaceMenu` (`MEGAMenus`), with
//! `screens/megacells/mega_pattern_provider.json` and `mega_interface.json`.
//! Derived from MEGA Cells 4.11.0, https://github.com/62832/MEGACells tag
//! 4.11.0, LGPL-3.0; see `ae2/addons/megacells/`.
//! The window entries are `ae2_machine_ui::MenuShape`'s: 54 for 18 slots.
use super::{Layout, MachineKind, Stored, Toggle, icon};

const NO_YES: [&str; 2] = ["NO", "YES"];
const PATTERN: &str = "ENCODED_PATTERN";
const BLANK: (i32, i32) = icon::BACKGROUND_BLANK_PATTERN;

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
    // PatternProviderMenu over MEGA's 18 patterns (ENCODED_PATTERN, two rows,
    // BREAK_AFTER_9COLS), then AE2's nine return slots (STORAGE); its three
    // settings and the priority tab as on AE2's.
    (horizon_client::ae2x::megacells::MEGA_PATTERN_PROVIDER, Layout {
        kind: MachineKind::PatternProvider,
        style: "megacells/mega_pattern_provider.json",
        slots: &[
            (0, PATTERN), (1, PATTERN), (2, PATTERN), (3, PATTERN), (4, PATTERN), (5, PATTERN), (6, PATTERN), (7, PATTERN), (8, PATTERN),
            (9, PATTERN), (10, PATTERN), (11, PATTERN), (12, PATTERN), (13, PATTERN), (14, PATTERN), (15, PATTERN), (16, PATTERN), (17, PATTERN),
            (18, "STORAGE"), (19, "STORAGE"), (20, "STORAGE"), (21, "STORAGE"), (22, "STORAGE"), (23, "STORAGE"),
            (24, "STORAGE"), (25, "STORAGE"), (26, "STORAGE"),
        ],
        icons: &[
            (0, BLANK), (1, BLANK), (2, BLANK), (3, BLANK), (4, BLANK), (5, BLANK), (6, BLANK), (7, BLANK), (8, BLANK),
            (9, BLANK), (10, BLANK), (11, BLANK), (12, BLANK), (13, BLANK), (14, BLANK), (15, BLANK), (16, BLANK), (17, BLANK),
        ],
        upgrades: 0..0,
        toggles: &[
            Toggle {
                entry: 27,
                key: "ae2_blocking",
                stored: Stored::Flag(false),
                default: 0,
                data: 0,
                title: "gui.tooltips.ae2.InterfaceBlockingMode",
                values: &[
                    (NO_YES[0], icon::BLOCKING_MODE_NO, "gui.tooltips.ae2.NonBlocking"),
                    (NO_YES[1], icon::BLOCKING_MODE_YES, "gui.tooltips.ae2.Blocking"),
                ],
                place: None,
                shown_by: None,
            },
            Toggle {
                entry: 28,
                key: "ae2_lock_mode",
                stored: Stored::Index,
                default: 0,
                data: 1,
                title: "gui.tooltips.ae2.LockCraftingMode",
                values: &[
                    ("NONE", icon::UNLOCKED, "gui.tooltips.ae2.LockCraftingModeNone"),
                    ("LOCK_UNTIL_PULSE", icon::REDSTONE_PULSE, "gui.tooltips.ae2.LockCraftingUntilRedstonePulse"),
                    ("LOCK_WHILE_HIGH", icon::REDSTONE_ON, "gui.tooltips.ae2.LockCraftingWhileRedstoneHigh"),
                    ("LOCK_WHILE_LOW", icon::REDSTONE_OFF, "gui.tooltips.ae2.LockCraftingWhileRedstoneLow"),
                    ("LOCK_UNTIL_RESULT", icon::ENTER, "gui.tooltips.ae2.LockCraftingUntilResultReturned"),
                ],
                place: None,
                shown_by: None,
            },
            Toggle {
                entry: 29,
                key: "terminal_visible",
                stored: Stored::Flag(true),
                default: 1,
                data: 2,
                title: "gui.ae2.PatternAccessTerminal",
                values: &[
                    (NO_YES[0], icon::PATTERN_ACCESS_HIDE, "gui.ae2.PatternAccessTerminalHint"),
                    (NO_YES[1], icon::PATTERN_ACCESS_SHOW, "gui.ae2.PatternAccessTerminalHint"),
                ],
                place: None,
                shown_by: None,
            },
        ],
        progress: None,
        priority: Some(53),
        decorations: &[],
        grid: None,
        cogs: &[],
        lock_line: Some([1, 3, 4]),
        page_line: None,
        panels: &[],
    }),
    // InterfaceMenu over MEGA's 18 slots: the config rows (CONFIG, two rows)
    // under amtButton1..9 and over amtButton10..18, the storage rows
    // (STORAGE); the card, fuzzy mode with a fuzzy card, the priority tab.
    // One page: `MenuShape`'s storage 0..18, config 18..36, card 36.
    (horizon_client::ae2x::megacells::MEGA_INTERFACE, Layout {
        kind: MachineKind::Interface,
        style: "megacells/mega_interface.json",
        slots: &[
            (18, "CONFIG"), (19, "CONFIG"), (20, "CONFIG"), (21, "CONFIG"), (22, "CONFIG"), (23, "CONFIG"),
            (24, "CONFIG"), (25, "CONFIG"), (26, "CONFIG"),
            (27, "CONFIG"), (28, "CONFIG"), (29, "CONFIG"), (30, "CONFIG"), (31, "CONFIG"), (32, "CONFIG"),
            (33, "CONFIG"), (34, "CONFIG"), (35, "CONFIG"),
            (0, "STORAGE"), (1, "STORAGE"), (2, "STORAGE"), (3, "STORAGE"), (4, "STORAGE"), (5, "STORAGE"),
            (6, "STORAGE"), (7, "STORAGE"), (8, "STORAGE"),
            (9, "STORAGE"), (10, "STORAGE"), (11, "STORAGE"), (12, "STORAGE"), (13, "STORAGE"), (14, "STORAGE"),
            (15, "STORAGE"), (16, "STORAGE"), (17, "STORAGE"),
        ],
        icons: &[],
        upgrades: 36..37,
        toggles: &[Toggle {
            entry: 41,
            key: "ae2_fuzzy_mode",
            stored: Stored::Text,
            default: 0,
            data: 0,
            title: "gui.tooltips.ae2.FuzzyMode",
            values: &[
                ("ignore_all", icon::FUZZY_IGNORE, "gui.tooltips.ae2.FZIgnoreAll"),
                ("percent_99", icon::FUZZY_PERCENT_99, "gui.tooltips.ae2.FZPercent_99"),
                ("percent_75", icon::FUZZY_PERCENT_75, "gui.tooltips.ae2.FZPercent_75"),
                ("percent_50", icon::FUZZY_PERCENT_50, "gui.tooltips.ae2.FZPercent_50"),
                ("percent_25", icon::FUZZY_PERCENT_25, "gui.tooltips.ae2.FZPercent_25"),
            ],
            place: None,
            shown_by: Some(1),
        }],
        progress: None,
        priority: Some(53),
        decorations: &[],
        grid: None,
        // InterfaceScreen: amtButton1..9 from interface.json, 10..18 from MEGA's style.
        cogs: &[(18..27, (8, 35)), (27..36, (8, 89))],
        lock_line: None,
        page_line: None,
        panels: &[],
    }),
];

/// The layout of a MEGA screen that goes by its menu's item or block.
pub(super) fn layout(block: &str) -> Option<&'static Layout> {
    LAYOUTS.iter().find(|(id, _)| *id == block).map(|(_, layout)| layout)
}

#[cfg(test)]
mod tests {
    use crate::game::ae2_machine_ui::MenuShape;
    use horizon_client::{ae2_world::Machine, ae2x::megacells as rows};

    /// MEGA's screens put the settings, card, priority and slots on the
    /// entries the server's window gives them for 18 slots.
    #[test]
    fn mega_layouts_sit_on_their_windows_entries() {
        horizon_client::ae2x::install();
        let provider = MenuShape::of(&Machine::for_block(rows::MEGA_PATTERN_PROVIDER).unwrap());
        let layout = super::layout(rows::MEGA_PATTERN_PROVIDER).unwrap();
        assert_eq!(provider.window(), "minecraft:generic_9x6");
        let settings = i32::from(provider.settings());
        assert_eq!(layout.toggles.iter().map(|t| t.entry).collect::<Vec<_>>(), [settings, settings + 1, settings + 2]);
        assert_eq!(layout.priority, Some(provider.priority() as i32));
        let returns = provider.returns();
        assert_eq!(layout.slots.len(), 27);
        for &(entry, semantic) in layout.slots {
            let entry = entry as i16;
            assert_eq!(semantic, if returns.contains(&entry) { "STORAGE" } else { "ENCODED_PATTERN" }, "entry {entry}");
            assert!(entry < returns.end);
        }

        let interface = MenuShape::of(&Machine::for_block(rows::MEGA_INTERFACE).unwrap());
        let layout = super::layout(rows::MEGA_INTERFACE).unwrap();
        // One page of 18: every slot shown at once.
        assert_eq!(interface.page, Some(0));
        let config = interface.config_entries();
        assert_eq!(layout.slots.len(), 36);
        for &(entry, semantic) in layout.slots {
            assert_eq!(semantic == "CONFIG", config.contains(&(entry as u8)), "entry {entry}");
            if semantic == "STORAGE" {
                assert_eq!(interface.internal(entry as i16, true), Some(300 + entry));
            }
        }
        // The card is the slot after the 18 physical ones.
        assert_eq!(interface.internal(layout.upgrades.start as i16, true), Some(300 + 18));
        assert_eq!(layout.priority, Some(interface.priority() as i32));
        // A cog over each config slot, nine to a row.
        assert_eq!(layout.cogs.iter().map(|(range, _)| range.len()).sum::<usize>(), 18);
        assert!(layout.cogs.iter().all(|(range, _)| config.contains(&(range.start as u8)) && config.contains(&(range.end as u8 - 1))));
    }
}
