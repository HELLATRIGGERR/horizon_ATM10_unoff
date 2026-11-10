//! Advanced AE's screens on the AE2 layout system: `SmallAdvPatternProviderScreen`
//! over `SmallAdvPatternProviderMenu` (AE2's `PatternProviderMenu` of nine
//! patterns), with `screens/small_adv_pattern_provider.json` (AE2's
//! `pattern_provider.json` with the title "Advanced Pattern Provider").
//! Derived from Advanced AE, https://github.com/pedroksl/AdvancedAE commit
//! `c322dad81e6f2ad94cb1fb861e73a7ba68e5907a`, LGPL-3.0, over AE2's
//! `PatternProviderScreen` (LGPL-3.0); see `ae2/addons/advancedae/`.
//! The window entries are `ae2_machine_ui::MenuShape`'s: 18 for nine slots.
use super::{Layout, MachineKind, Stored, Toggle, icon};

const PATTERN: &str = "ENCODED_PATTERN";
const BLANK: (i32, i32) = icon::BACKGROUND_BLANK_PATTERN;

const LAYOUTS: &[(&str, Layout)] = &[
    // The nine patterns and nine return slots; blocking mode, the crafting
    // lock and the Pattern Access Terminal toggle in the left toolbar, the
    // priority tab, as AE2's. `FILTERED_IMPORT`'s button is a later slice's.
    (horizon_client::ae2x::advancedae::SMALL_ADV_PATTERN_PROVIDER, Layout {
        kind: MachineKind::PatternProvider,
        style: "small_adv_pattern_provider.json",
        slots: &[
            (0, PATTERN), (1, PATTERN), (2, PATTERN), (3, PATTERN), (4, PATTERN), (5, PATTERN), (6, PATTERN), (7, PATTERN), (8, PATTERN),
            (9, "STORAGE"), (10, "STORAGE"), (11, "STORAGE"), (12, "STORAGE"), (13, "STORAGE"), (14, "STORAGE"),
            (15, "STORAGE"), (16, "STORAGE"), (17, "STORAGE"),
        ],
        icons: &[(0, BLANK), (1, BLANK), (2, BLANK), (3, BLANK), (4, BLANK), (5, BLANK), (6, BLANK), (7, BLANK), (8, BLANK)],
        upgrades: 0..0,
        toggles: &[
            Toggle {
                entry: 18,
                key: "ae2_blocking",
                stored: Stored::Flag(false),
                default: 0,
                data: 0,
                title: "gui.tooltips.ae2.InterfaceBlockingMode",
                values: &[
                    ("NO", icon::BLOCKING_MODE_NO, "gui.tooltips.ae2.NonBlocking"),
                    ("YES", icon::BLOCKING_MODE_YES, "gui.tooltips.ae2.Blocking"),
                ],
                place: None,
                shown_by: None,
            },
            Toggle {
                entry: 19,
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
                entry: 20,
                key: "terminal_visible",
                stored: Stored::Flag(true),
                default: 1,
                data: 2,
                title: "gui.ae2.PatternAccessTerminal",
                values: &[
                    ("NO", icon::PATTERN_ACCESS_HIDE, "gui.ae2.PatternAccessTerminalHint"),
                    ("YES", icon::PATTERN_ACCESS_SHOW, "gui.ae2.PatternAccessTerminalHint"),
                ],
                place: None,
                shown_by: None,
            },
        ],
        progress: None,
        priority: Some(26),
        decorations: &[],
        grid: None,
        cogs: &[],
        lock_line: Some([1, 3, 4]),
        page_line: None,
        panels: &[],
    }),
];

/// The layout of an Advanced AE screen that goes by its menu's block.
pub(super) fn layout(block: &str) -> Option<&'static Layout> {
    LAYOUTS.iter().find(|(id, _)| *id == block).map(|(_, layout)| layout)
}

#[cfg(test)]
mod tests {
    use crate::game::ae2_machine_ui::MenuShape;
    use horizon_client::{ae2_world::Machine, ae2x::advancedae as rows};

    /// The provider's screen puts its settings and priority on the entries
    /// the server's window gives a provider of nine patterns, as AE2's.
    #[test]
    fn the_small_provider_layout_sits_on_its_windows_entries() {
        horizon_client::ae2x::install();
        let provider = MenuShape::of(&Machine::for_block(rows::SMALL_ADV_PATTERN_PROVIDER).unwrap());
        let ae2 = MenuShape::of(&Machine::for_block("ae2:pattern_provider").unwrap());
        assert_eq!(provider, ae2);
        let layout = super::layout(rows::SMALL_ADV_PATTERN_PROVIDER).unwrap();
        let settings = i32::from(provider.settings());
        assert_eq!(layout.toggles.iter().map(|t| t.entry).collect::<Vec<_>>(), [settings, settings + 1, settings + 2]);
        assert_eq!(layout.priority, Some(provider.priority() as i32));
        let returns = provider.returns();
        assert_eq!(layout.slots.len(), 18);
        for &(entry, semantic) in layout.slots {
            let entry = entry as i16;
            assert_eq!(semantic, if returns.contains(&entry) { "STORAGE" } else { "ENCODED_PATTERN" }, "entry {entry}");
        }
    }
}
