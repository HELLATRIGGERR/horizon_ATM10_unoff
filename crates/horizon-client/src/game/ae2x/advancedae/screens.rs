//! Advanced AE's screens on the AE2 layout system: `SmallAdvPatternProviderScreen`
//! over `SmallAdvPatternProviderMenu` (AE2's `PatternProviderMenu` of nine
//! patterns), with `screens/small_adv_pattern_provider.json` (AE2's
//! `pattern_provider.json` with the title "Advanced Pattern Provider"),
//! `AdvPatternProviderScreen` over `AdvPatternProviderMenu` (36) with
//! `screens/adv_pattern_provider.json`, and
//! `ReactionChamberScreen` over `ReactionChamberMenu` with `screens/reaction_chamber.json`.
//! Derived from Advanced AE, https://github.com/pedroksl/AdvancedAE commit
//! `c322dad81e6f2ad94cb1fb861e73a7ba68e5907a`, LGPL-3.0, over AE2's
//! `PatternProviderScreen` (LGPL-3.0); see `ae2/addons/advancedae/`.
//! The window entries are `ae2_machine_ui::MenuShape`'s: 18 for nine slots, 45 for 36.
use super::{Layout, MachineKind, Progress, Stored, Toggle, icon};
use crate::game::ae2x::advancedae::chamber;

const PATTERN: &str = "ENCODED_PATTERN";
const BLANK: (i32, i32) = icon::BACKGROUND_BLANK_PATTERN;
/// `AAEIcon.FILTERED_IMPORT_ON`, `FILTERED_IMPORT_OFF` in `guis/states.png`.
const FILTERED_IMPORT_ON: (i32, i32) = icon::sheet(icon::ADVANCED_AE, (144, 0));
const FILTERED_IMPORT_OFF: (i32, i32) = icon::sheet(icon::ADVANCED_AE, (160, 0));
/// `AAEIcon.VALID_INPUT`, `INVALID_INPUT` and `CLEAR`.
const SIDE_ON: (i32, i32) = icon::sheet(icon::ADVANCED_AE, (48, 0));
const SIDE_OFF: (i32, i32) = icon::sheet(icon::ADVANCED_AE, (64, 0));
const CLEAR: (i32, i32) = icon::sheet(icon::ADVANCED_AE, (80, 0));

/// `ServerSettingToggleButton<>(Settings.AUTO_EXPORT, YesNo.NO)`.
const AUTO_EXPORT: Toggle = Toggle {
    entry: chamber::AUTO_EXPORT_ENTRY as i32,
    key: chamber::AUTO_EXPORT,
    stored: Stored::Text,
    default: 0,
    data: 4,
    title: "gui.tooltips.ae2.AutoExport",
    values: &[("NO", icon::AUTO_EXPORT_OFF, "gui.tooltips.ae2.AutoExportOff"), ("YES", icon::AUTO_EXPORT_ON, "gui.tooltips.ae2.AutoExportOn")],
    place: None,
    shown_by: None,
};
/// One side of `allowedOutputs` (`RelativeSide` order), on the toolbar while AUTO_EXPORT is on, as the
/// mod's `DIRECTIONAL_OUTPUT` button is.
const FRONT: Toggle = Toggle {
    entry: chamber::SIDE_ENTRIES.start as i32,
    key: chamber::SIDES[0].1,
    stored: Stored::Flag(false),
    default: 0,
    data: 6,
    title: "gui.tooltips.ae2.SideFront",
    values: &[("NO", SIDE_OFF, "options.off"), ("YES", SIDE_ON, "options.on")],
    place: None,
    shown_by: Some(4),
};
/// The flush buttons (`clearFluid`, `clearFluidOut`), at the style's widgets.
const FLUSH: Toggle = Toggle {
    entry: chamber::FLUSH_IN_ENTRY as i32,
    key: "aae_flush",
    stored: Stored::Flag(false),
    default: 0,
    data: 12,
    title: "",
    values: &[("", CLEAR, "gui.tooltips.advanced_ae.ClearFluidButtonHint")],
    place: Some((28, 73)),
    shown_by: None,
};

/// AE2's provider settings: blocking mode, the crafting lock and the Pattern
/// Access Terminal toggle in the left toolbar at entry `settings` on
/// (`MenuShape::settings`), then `FILTERED_IMPORT`'s button
/// (`AAESettingToggleButton`, its icons in the mod's own `states.png`). The
/// data slots 3 and 4 are the lock line's.
const fn provider_toggles(settings: i32) -> [Toggle; 4] {
    [
        Toggle {
            entry: settings,
            key: "ae2_blocking",
            stored: Stored::Flag(false),
            default: 0,
            data: 0,
            title: "gui.tooltips.ae2.InterfaceBlockingMode",
            values: BLOCKING_VALUES,
            place: None,
            shown_by: None,
        },
        Toggle {
            entry: settings + 1,
            key: "ae2_lock_mode",
            stored: Stored::Index,
            default: 0,
            data: 1,
            title: "gui.tooltips.ae2.LockCraftingMode",
            values: LOCK_VALUES,
            place: None,
            shown_by: None,
        },
        Toggle {
            entry: settings + 2,
            key: "terminal_visible",
            stored: Stored::Flag(true),
            default: 1,
            data: 2,
            title: "gui.ae2.PatternAccessTerminal",
            values: TERMINAL_VALUES,
            place: None,
            shown_by: None,
        },
        Toggle {
            entry: settings + 3,
            key: crate::game::ae2x::advancedae::FILTERED_IMPORT,
            stored: Stored::Text,
            default: 0,
            data: 5,
            title: "gui.tooltips.advanced_ae.FilteredImport",
            values: FILTERED_IMPORT_VALUES,
            place: None,
            shown_by: None,
        },
    ]
}
const BLOCKING_VALUES: &[(&str, (i32, i32), &str)] = &[
    ("NO", icon::BLOCKING_MODE_NO, "gui.tooltips.ae2.NonBlocking"),
    ("YES", icon::BLOCKING_MODE_YES, "gui.tooltips.ae2.Blocking"),
];
const LOCK_VALUES: &[(&str, (i32, i32), &str)] = &[
    ("NONE", icon::UNLOCKED, "gui.tooltips.ae2.LockCraftingModeNone"),
    ("LOCK_UNTIL_PULSE", icon::REDSTONE_PULSE, "gui.tooltips.ae2.LockCraftingUntilRedstonePulse"),
    ("LOCK_WHILE_HIGH", icon::REDSTONE_ON, "gui.tooltips.ae2.LockCraftingWhileRedstoneHigh"),
    ("LOCK_WHILE_LOW", icon::REDSTONE_OFF, "gui.tooltips.ae2.LockCraftingWhileRedstoneLow"),
    ("LOCK_UNTIL_RESULT", icon::ENTER, "gui.tooltips.ae2.LockCraftingUntilResultReturned"),
];
const TERMINAL_VALUES: &[(&str, (i32, i32), &str)] = &[
    ("NO", icon::PATTERN_ACCESS_HIDE, "gui.ae2.PatternAccessTerminalHint"),
    ("YES", icon::PATTERN_ACCESS_SHOW, "gui.ae2.PatternAccessTerminalHint"),
];
const FILTERED_IMPORT_VALUES: &[(&str, (i32, i32), &str)] = &[
    ("NO", FILTERED_IMPORT_OFF, "gui.tooltips.advanced_ae.FilteredImportOff"),
    ("YES", FILTERED_IMPORT_ON, "gui.tooltips.advanced_ae.FilteredImportOn"),
];
const SMALL_TOGGLES: [Toggle; 4] = provider_toggles(18);
const LARGE_TOGGLES: [Toggle; 4] = provider_toggles(45);

/// A provider menu's slots: the patterns' entries, then the nine return
/// slots' (`AdvPatternProviderMenu`: `ENCODED_PATTERN`, `STORAGE`).
const fn provider_slots<const N: usize>() -> [(usize, &'static str); N] {
    let mut slots = [(0, PATTERN); N];
    let mut entry = 0;
    while entry < N {
        slots[entry] = (entry, if entry + 9 < N { PATTERN } else { "STORAGE" });
        entry += 1;
    }
    slots
}
/// The blank pattern each empty pattern slot shows.
const fn blank_patterns<const N: usize>() -> [(usize, (i32, i32)); N] {
    let mut icons = [(0, BLANK); N];
    let mut entry = 0;
    while entry < N {
        icons[entry] = (entry, BLANK);
        entry += 1;
    }
    icons
}
const SMALL_SLOTS: [(usize, &str); 18] = provider_slots();
const LARGE_SLOTS: [(usize, &str); 45] = provider_slots();
const SMALL_BLANKS: [(usize, (i32, i32)); 9] = blank_patterns();
const LARGE_BLANKS: [(usize, (i32, i32)); 36] = blank_patterns();

const LAYOUTS: &[(&str, Layout)] = &[
    // The nine patterns and nine return slots, the settings, the priority
    // tab, as AE2's (`SmallAdvPatternProviderScreen`).
    (horizon_client::ae2x::advancedae::SMALL_ADV_PATTERN_PROVIDER, Layout {
        kind: MachineKind::PatternProvider,
        style: "small_adv_pattern_provider.json",
        slots: &SMALL_SLOTS,
        icons: &SMALL_BLANKS,
        upgrades: 0..0,
        toggles: &SMALL_TOGGLES,
        progress: None,
        priority: Some(26),
        decorations: &[],
        grid: None,
        cogs: &[],
        lock_line: Some([1, 3, 4]),
        page_line: None,
        panels: &[],
    }),
    // `AdvPatternProviderScreen` with `adv_pattern_provider.json`: all 36
    // patterns at once (`BREAK_AFTER_9COLS`), the nine return slots; the
    // window of a bigger provider (`MenuShape`): settings at 45, priority 53.
    (horizon_client::ae2x::advancedae::ADV_PATTERN_PROVIDER, Layout {
        kind: MachineKind::PatternProvider,
        style: "adv_pattern_provider.json",
        slots: &LARGE_SLOTS,
        icons: &LARGE_BLANKS,
        upgrades: 0..0,
        toggles: &LARGE_TOGGLES,
        progress: None,
        priority: Some(53),
        decorations: &[],
        grid: None,
        cogs: &[],
        lock_line: Some([1, 3, 4]),
        page_line: None,
        panels: &[],
    }),
    // ReactionChamberMenu: nine inputs (BREAK_AFTER_3COLS), the output, four cards; the screen's
    // vertical progress bar, AUTO_EXPORT and the six output sides on the toolbar, the flush buttons, the
    // power alert (data slot 5). Its two FluidTankSlots are entries at the tanks' places, a bucket named
    // with the fluid and its amount.
    (horizon_client::ae2x::advancedae::REACTION_CHAMBER, Layout {
        kind: MachineKind::Ext(u16::MAX),
        style: "reaction_chamber.json",
        slots: &[
            (0, "MACHINE_INPUT"), (1, "MACHINE_INPUT"), (2, "MACHINE_INPUT"), (3, "MACHINE_INPUT"), (4, "MACHINE_INPUT"), (5, "MACHINE_INPUT"),
            (6, "MACHINE_INPUT"), (7, "MACHINE_INPUT"), (8, "MACHINE_INPUT"), (9, "MACHINE_OUTPUT"),
            (chamber::TANK_IN_ENTRY, TANK_IN), (chamber::TANK_OUT_ENTRY, TANK_OUT),
        ],
        icons: &[],
        upgrades: 10..14,
        toggles: &[
            AUTO_EXPORT,
            FRONT,
            Toggle { entry: FRONT.entry + 1, key: chamber::SIDES[1].1, data: 7, title: "gui.tooltips.ae2.SideBack", ..FRONT },
            Toggle { entry: FRONT.entry + 2, key: chamber::SIDES[2].1, data: 8, title: "gui.tooltips.ae2.SideTop", ..FRONT },
            Toggle { entry: FRONT.entry + 3, key: chamber::SIDES[3].1, data: 9, title: "gui.tooltips.ae2.SideBottom", ..FRONT },
            Toggle { entry: FRONT.entry + 4, key: chamber::SIDES[4].1, data: 10, title: "gui.tooltips.ae2.SideLeft", ..FRONT },
            Toggle { entry: FRONT.entry + 5, key: chamber::SIDES[5].1, data: 11, title: "gui.tooltips.ae2.SideRight", ..FRONT },
            FLUSH,
            Toggle { entry: chamber::FLUSH_OUT_ENTRY as i32, data: 13, place: Some((140, 73)), ..FLUSH },
        ],
        progress: Some(Progress { name: "progressBar", current: 0, max: 2 }),
        priority: None,
        decorations: &[],
        grid: None,
        cogs: &[],
        lock_line: None,
        page_line: None,
        panels: &["powerAlert?5"],
    }),
];

/// The tanks' entries, at the middle of `FluidTankSlot`s 16 x 58 at (9, 21) and (151, 21).
const TANK_IN: &str = "AAE_TANK_IN";
const TANK_OUT: &str = "AAE_TANK_OUT";

/// The places of the slots the Reaction Chamber's screen draws by its code (its tanks).
pub(super) fn style_slots(block: &str, style: &mut horizon_client::ae2_screen_style::ScreenStyle) {
    use horizon_client::ae2_screen_style::{Position, SlotStyle};
    if block != horizon_client::ae2x::advancedae::REACTION_CHAMBER {
        return;
    }
    for (semantic, left) in [(TANK_IN, 9), (TANK_OUT, 151)] {
        let position = Position { left: Some(left), top: Some(21 + (58 - 16) / 2), right: None, bottom: None };
        style.slots.insert(semantic.into(), SlotStyle { position, grid: None, hidden: false });
    }
}

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
        assert_eq!(layout.toggles.iter().map(|t| t.entry).collect::<Vec<_>>(), [settings, settings + 1, settings + 2, settings + 3]);
        assert!(layout.toggles.iter().all(|t| t.entry < layout.priority.unwrap()), "Filtered Import's entry is free");
        assert_eq!(layout.toggles[3].values[1].1, (256 + 144, 0), "Advanced AE's own icon");
        assert_eq!(layout.priority, Some(provider.priority() as i32));
        let returns = provider.returns();
        assert_eq!(layout.slots.len(), 18);
        for &(entry, semantic) in layout.slots {
            let entry = entry as i16;
            assert_eq!(semantic, if returns.contains(&entry) { "STORAGE" } else { "ENCODED_PATTERN" }, "entry {entry}");
        }
    }

    /// The 36-pattern provider's screen sits on the window of a bigger
    /// provider: the patterns, the returns, the settings and Filtered Import
    /// after them, the priority last.
    #[test]
    fn the_large_provider_layout_sits_on_its_windows_entries() {
        horizon_client::ae2x::install();
        let machine = Machine::for_block(rows::ADV_PATTERN_PROVIDER).unwrap();
        let shape = MenuShape::of(&machine);
        let layout = super::layout(rows::ADV_PATTERN_PROVIDER).unwrap();
        let settings = i32::from(shape.settings());
        assert_eq!(layout.toggles.iter().map(|t| t.entry).collect::<Vec<_>>(), [settings, settings + 1, settings + 2, settings + 3]);
        assert_eq!(layout.toggles[3].entry as usize, crate::game::ae2x::advancedae::filtered_import_entry(&machine));
        assert_eq!(layout.priority, Some(shape.priority() as i32));
        assert!(layout.toggles.iter().all(|t| t.entry < layout.priority.unwrap()));
        assert_eq!((layout.slots.len(), layout.icons.len()), (45, 36));
        let returns = shape.returns();
        for &(entry, semantic) in layout.slots {
            assert_eq!(semantic, if returns.contains(&(entry as i16)) { "STORAGE" } else { "ENCODED_PATTERN" }, "entry {entry}");
        }
        // A part's screen is its block's.
        assert_eq!(horizon_client::ae2_world::variant(rows::ADV_PATTERN_PROVIDER_PART).unwrap().block, rows::ADV_PATTERN_PROVIDER);
    }
}
