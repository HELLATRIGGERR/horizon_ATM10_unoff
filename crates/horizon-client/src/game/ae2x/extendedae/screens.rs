//! ExtendedAE's machine screens on the AE2 layout system (`GuiExPatternProvider`,
//! `GuiExInterface`, `GuiExDrive`, `GuiExIOPort`, `GuiCrystalAssembler`,
//! `GuiCircuitCutter`, `GuiExInscriber`, `GuiExMolecularAssembler` with their
//! containers, and their `screens/*.json`).
//! Derived from ExtendedAE 1.21-2.2.42, https://github.com/GlodBlock/ExtendedAE
//! commit 1f86f927552f02451236465c0c96a12a3de8f66c, LGPL-3.0; see
//! `ae2/addons/extendedae/`.
//! The window entries are `ae2_machine_ui::MenuShape`'s.
use super::{Layout, MachineKind, PageLine, Progress, Stored, Toggle, icon};

/// The kind of a layout of the mod's own machines, which go by their id.
const OWN: MachineKind = MachineKind::Ext(u16::MAX);
/// `ServerSettingToggleButton<>(Settings.AUTO_EXPORT, YesNo.NO)` on the entry
/// the game takes as its press (`extendedae::AUTO_EXPORT_ENTRY`).
const AUTO_EXPORT: Toggle = Toggle {
    entry: 24,
    key: "auto_export",
    stored: Stored::Text,
    default: 0,
    data: 4,
    title: "gui.tooltips.ae2.AutoExport",
    values: &[
        (NO_YES[0], icon::AUTO_EXPORT_OFF, "gui.tooltips.ae2.AutoExportOff"),
        (NO_YES[1], icon::AUTO_EXPORT_ON, "gui.tooltips.ae2.AutoExportOn"),
    ],
    place: None,
    shown_by: None,
};

const NO_YES: [&str; 2] = ["NO", "YES"];

const LAYOUTS: &[(&str, Layout)] = &[
    // PatternProviderMenu's slots, all 36 patterns at once (ENCODED_PATTERN,
    // BREAK_AFTER_9COLS: the mod has no pages for them).
    ("extendedae:ex_pattern_provider", Layout {
        kind: MachineKind::PatternProvider,
        style: "ex_pattern_provider.json",
        slots: &[
            (0, "ENCODED_PATTERN"), (1, "ENCODED_PATTERN"), (2, "ENCODED_PATTERN"), (3, "ENCODED_PATTERN"), (4, "ENCODED_PATTERN"), (5, "ENCODED_PATTERN"),
            (6, "ENCODED_PATTERN"), (7, "ENCODED_PATTERN"), (8, "ENCODED_PATTERN"), (9, "ENCODED_PATTERN"), (10, "ENCODED_PATTERN"), (11, "ENCODED_PATTERN"),
            (12, "ENCODED_PATTERN"), (13, "ENCODED_PATTERN"), (14, "ENCODED_PATTERN"), (15, "ENCODED_PATTERN"), (16, "ENCODED_PATTERN"), (17, "ENCODED_PATTERN"),
            (18, "ENCODED_PATTERN"), (19, "ENCODED_PATTERN"), (20, "ENCODED_PATTERN"), (21, "ENCODED_PATTERN"), (22, "ENCODED_PATTERN"), (23, "ENCODED_PATTERN"),
            (24, "ENCODED_PATTERN"), (25, "ENCODED_PATTERN"), (26, "ENCODED_PATTERN"), (27, "ENCODED_PATTERN"), (28, "ENCODED_PATTERN"), (29, "ENCODED_PATTERN"),
            (30, "ENCODED_PATTERN"), (31, "ENCODED_PATTERN"), (32, "ENCODED_PATTERN"), (33, "ENCODED_PATTERN"), (34, "ENCODED_PATTERN"), (35, "ENCODED_PATTERN"),
            (36, "STORAGE"), (37, "STORAGE"), (38, "STORAGE"), (39, "STORAGE"), (40, "STORAGE"), (41, "STORAGE"),
            (42, "STORAGE"), (43, "STORAGE"), (44, "STORAGE"),
        ],
        icons: &[
            (0, icon::BACKGROUND_BLANK_PATTERN), (1, icon::BACKGROUND_BLANK_PATTERN), (2, icon::BACKGROUND_BLANK_PATTERN), (3, icon::BACKGROUND_BLANK_PATTERN),
            (4, icon::BACKGROUND_BLANK_PATTERN), (5, icon::BACKGROUND_BLANK_PATTERN), (6, icon::BACKGROUND_BLANK_PATTERN), (7, icon::BACKGROUND_BLANK_PATTERN),
            (8, icon::BACKGROUND_BLANK_PATTERN), (9, icon::BACKGROUND_BLANK_PATTERN), (10, icon::BACKGROUND_BLANK_PATTERN), (11, icon::BACKGROUND_BLANK_PATTERN),
            (12, icon::BACKGROUND_BLANK_PATTERN), (13, icon::BACKGROUND_BLANK_PATTERN), (14, icon::BACKGROUND_BLANK_PATTERN), (15, icon::BACKGROUND_BLANK_PATTERN),
            (16, icon::BACKGROUND_BLANK_PATTERN), (17, icon::BACKGROUND_BLANK_PATTERN), (18, icon::BACKGROUND_BLANK_PATTERN), (19, icon::BACKGROUND_BLANK_PATTERN),
            (20, icon::BACKGROUND_BLANK_PATTERN), (21, icon::BACKGROUND_BLANK_PATTERN), (22, icon::BACKGROUND_BLANK_PATTERN), (23, icon::BACKGROUND_BLANK_PATTERN),
            (24, icon::BACKGROUND_BLANK_PATTERN), (25, icon::BACKGROUND_BLANK_PATTERN), (26, icon::BACKGROUND_BLANK_PATTERN), (27, icon::BACKGROUND_BLANK_PATTERN),
            (28, icon::BACKGROUND_BLANK_PATTERN), (29, icon::BACKGROUND_BLANK_PATTERN), (30, icon::BACKGROUND_BLANK_PATTERN), (31, icon::BACKGROUND_BLANK_PATTERN),
            (32, icon::BACKGROUND_BLANK_PATTERN), (33, icon::BACKGROUND_BLANK_PATTERN), (34, icon::BACKGROUND_BLANK_PATTERN), (35, icon::BACKGROUND_BLANK_PATTERN),
        ],
        upgrades: 0..0,
        toggles: &[
            Toggle {
                entry: 45,
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
                entry: 46,
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
                entry: 47,
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
    // A page of 18: config rows EX_1 and EX_3, storage rows EX_2 and EX_4 (the
    // second page's EX_5 to EX_8 have the same places). Left toolbar: fuzzy with
    // a fuzzy card, then Next or Previous Page, one at a time.
    ("extendedae:ex_interface", Layout {
        kind: MachineKind::Interface,
        style: "ex_interface.json",
        slots: &[
            (18, "EX_1"), (19, "EX_1"), (20, "EX_1"), (21, "EX_1"), (22, "EX_1"), (23, "EX_1"),
            (24, "EX_1"), (25, "EX_1"), (26, "EX_1"),
            (0, "EX_2"), (1, "EX_2"), (2, "EX_2"), (3, "EX_2"), (4, "EX_2"), (5, "EX_2"),
            (6, "EX_2"), (7, "EX_2"), (8, "EX_2"),
            (27, "EX_3"), (28, "EX_3"), (29, "EX_3"), (30, "EX_3"), (31, "EX_3"), (32, "EX_3"),
            (33, "EX_3"), (34, "EX_3"), (35, "EX_3"),
            (9, "EX_4"), (10, "EX_4"), (11, "EX_4"), (12, "EX_4"), (13, "EX_4"), (14, "EX_4"),
            (15, "EX_4"), (16, "EX_4"), (17, "EX_4"),
        ],
        icons: &[],
        upgrades: 36..37,
        toggles: &[
            Toggle {
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
            },
            Toggle {
                entry: 43,
                key: "ae2x_page",
                stored: Stored::Index,
                default: 0,
                data: 2,
                title: "",
                values: &[
                    ("0", icon::ARROW_RIGHT, "gui.extendedae.ex_interface.next"),
                    ("1", icon::ARROW_LEFT, "gui.extendedae.ex_interface.pre"),
                ],
                place: None,
                shown_by: None,
            },
        ],
        progress: None,
        priority: Some(53),
        decorations: &[],
        grid: None,
        // amtButton1..18: above the config rows.
        cogs: &[(18..27, (8, 35)), (27..36, (8, 95))],
        lock_line: None,
        page_line: Some(PageLine { data: 2, key: "gui.extendedae.ex_interface.config", fallback: "Config Page %s", at: (8.0, 24.0), per_value: false, count: false }),
        panels: &[],
    }),
    // The drive's ten cells on the left, ten more on the right (EX_1).
    ("extendedae:ex_drive", Layout {
        kind: MachineKind::Drive,
        style: "ex_drive.json",
        slots: &[
            (0, "STORAGE_CELL"), (1, "STORAGE_CELL"), (2, "STORAGE_CELL"), (3, "STORAGE_CELL"), (4, "STORAGE_CELL"), (5, "STORAGE_CELL"),
            (6, "STORAGE_CELL"), (7, "STORAGE_CELL"), (8, "STORAGE_CELL"), (9, "STORAGE_CELL"),
            (10, "EX_1"), (11, "EX_1"), (12, "EX_1"), (13, "EX_1"), (14, "EX_1"), (15, "EX_1"),
            (16, "EX_1"), (17, "EX_1"), (18, "EX_1"), (19, "EX_1"),
        ],
        icons: &[
            (0, icon::BACKGROUND_STORAGE_CELL), (1, icon::BACKGROUND_STORAGE_CELL), (2, icon::BACKGROUND_STORAGE_CELL), (3, icon::BACKGROUND_STORAGE_CELL),
            (4, icon::BACKGROUND_STORAGE_CELL), (5, icon::BACKGROUND_STORAGE_CELL), (6, icon::BACKGROUND_STORAGE_CELL), (7, icon::BACKGROUND_STORAGE_CELL),
            (8, icon::BACKGROUND_STORAGE_CELL), (9, icon::BACKGROUND_STORAGE_CELL), (10, icon::BACKGROUND_STORAGE_CELL), (11, icon::BACKGROUND_STORAGE_CELL),
            (12, icon::BACKGROUND_STORAGE_CELL), (13, icon::BACKGROUND_STORAGE_CELL), (14, icon::BACKGROUND_STORAGE_CELL), (15, icon::BACKGROUND_STORAGE_CELL),
            (16, icon::BACKGROUND_STORAGE_CELL), (17, icon::BACKGROUND_STORAGE_CELL), (18, icon::BACKGROUND_STORAGE_CELL), (19, icon::BACKGROUND_STORAGE_CELL),
        ],
        upgrades: 0..0,
        toggles: &[],
        progress: None,
        priority: Some(26),
        decorations: &[],
        grid: None,
        cogs: &[],
        lock_line: None,
        page_line: None,
        panels: &[],
    }),
    // ContainerExIOPort: AE2's IO Port menu with five cards; GuiExIOPort's
    // toolbar as GuiIOPort's (fullness, redstone with a card), the
    // operation mode in its place on the screen.
    ("extendedae:ex_io_port", Layout {
        kind: MachineKind::IoPort,
        style: "ex_io_port.json",
        slots: &[
            (0, "MACHINE_INPUT"), (1, "MACHINE_INPUT"), (2, "MACHINE_INPUT"), (3, "MACHINE_INPUT"), (4, "MACHINE_INPUT"), (5, "MACHINE_INPUT"),
            (6, "MACHINE_OUTPUT"), (7, "MACHINE_OUTPUT"), (8, "MACHINE_OUTPUT"), (9, "MACHINE_OUTPUT"), (10, "MACHINE_OUTPUT"), (11, "MACHINE_OUTPUT"),
        ],
        icons: &[
            (0, icon::BACKGROUND_STORAGE_CELL), (1, icon::BACKGROUND_STORAGE_CELL), (2, icon::BACKGROUND_STORAGE_CELL), (3, icon::BACKGROUND_STORAGE_CELL),
            (4, icon::BACKGROUND_STORAGE_CELL), (5, icon::BACKGROUND_STORAGE_CELL), (6, icon::BACKGROUND_STORAGE_CELL), (7, icon::BACKGROUND_STORAGE_CELL),
            (8, icon::BACKGROUND_STORAGE_CELL), (9, icon::BACKGROUND_STORAGE_CELL), (10, icon::BACKGROUND_STORAGE_CELL), (11, icon::BACKGROUND_STORAGE_CELL),
        ],
        upgrades: 12..17,
        toggles: &[
            Toggle {
                entry: 24,
                key: "io_fullness",
                stored: Stored::Text,
                default: 0,
                data: 0,
                title: "gui.tooltips.ae2.OperationMode",
                values: &[
                    ("empty", icon::FULLNESS_EMPTY, "gui.tooltips.ae2.MoveWhenEmpty"),
                    ("half", icon::FULLNESS_HALF, "gui.tooltips.ae2.MoveWhenWorkIsDone"),
                    ("full", icon::FULLNESS_FULL, "gui.tooltips.ae2.MoveWhenFull"),
                ],
                place: None,
                shown_by: None,
            },
            Toggle {
                entry: 25,
                key: "io_redstone",
                stored: Stored::Text,
                default: 0,
                data: 1,
                title: "gui.tooltips.ae2.RedstoneMode",
                values: &[
                    ("ignore", icon::REDSTONE_IGNORE, "gui.tooltips.ae2.AlwaysActive"),
                    ("low", icon::REDSTONE_LOW, "gui.tooltips.ae2.ActiveWithoutSignal"),
                    ("high", icon::REDSTONE_HIGH, "gui.tooltips.ae2.ActiveWithSignal"),
                ],
                place: None,
                shown_by: Some(3),
            },
            Toggle {
                entry: 26,
                key: "io_operation",
                stored: Stored::Text,
                default: 1,
                data: 2,
                title: "gui.tooltips.ae2.TransferDirection",
                values: &[
                    ("fill", icon::ARROW_LEFT, "gui.tooltips.ae2.TransferToStorageCell"),
                    ("empty", icon::ARROW_RIGHT, "gui.tooltips.ae2.TransferToNetwork"),
                ],
                place: Some((80, 17)),
                shown_by: None,
            },
        ],
        progress: None,
        priority: None,
        // drawBG: a 1k cell and a drive, where GuiIOPort draws them.
        decorations: &[("ae2:item_storage_cell_1k", (58, 17)), ("ae2:drive", (102, 17))],
        grid: None,
        cogs: &[],
        lock_line: None,
        page_line: None,
        panels: &[],
    }),
    // ContainerCrystalAssembler: nine inputs (BREAK_AFTER_3COLS), the tank as
    // a STORAGE slot, the output; four cards; GuiCrystalAssembler's vertical
    // progress bar and AUTO_EXPORT toggle. Its output side screen
    // (OutputSideConfig) is not drawn: the sides are the entries 14 to 19.
    ("extendedae:crystal_assembler", Layout {
        kind: OWN,
        style: "crystal_assembler.json",
        slots: &[
            (0, "MACHINE_INPUT"), (1, "MACHINE_INPUT"), (2, "MACHINE_INPUT"), (3, "MACHINE_INPUT"), (4, "MACHINE_INPUT"), (5, "MACHINE_INPUT"),
            (6, "MACHINE_INPUT"), (7, "MACHINE_INPUT"), (8, "MACHINE_INPUT"), (9, "MACHINE_OUTPUT"), (20, "STORAGE"),
        ],
        icons: &[],
        upgrades: 10..14,
        toggles: &[AUTO_EXPORT],
        progress: Some(Progress { name: "progressBar", current: 0, max: 2 }),
        priority: None,
        decorations: &[],
        grid: None,
        cogs: &[],
        lock_line: None,
        page_line: None,
        panels: &[],
    }),
    // ContainerExInscriber: the thread shown (its page) in AE2's Inscriber's
    // places, four cards; GuiExInscriber's toolbar: separate sides, auto
    // export, the stack size, next and previous page (each hidden on the last
    // and first page in the mod; here they stop there); "Inscription Job %s".
    ("extendedae:ex_inscriber", Layout {
        kind: OWN,
        style: "ex_inscriber.json",
        slots: &[(0, "INSCRIBER_PLATE_TOP"), (1, "INSCRIBER_PLATE_BOTTOM"), (2, "MACHINE_INPUT"), (3, "MACHINE_OUTPUT")],
        icons: &[(0, icon::BACKGROUND_PLATE), (1, icon::BACKGROUND_PLATE), (2, icon::BACKGROUND_INGOT)],
        upgrades: 4..8,
        toggles: &[
            Toggle {
                entry: 25,
                key: "inscriber_separate_sides",
                stored: Stored::Text,
                default: 0,
                data: 4,
                title: "gui.tooltips.ae2.InscriberSideness",
                values: &[
                    (NO_YES[0], icon::INSCRIBER_COMBINED_SIDES, "gui.tooltips.ae2.InscriberSidenessCombined"),
                    (NO_YES[1], icon::INSCRIBER_SEPARATE_SIDES, "gui.tooltips.ae2.InscriberSidenessSeparate"),
                ],
                place: None,
                shown_by: None,
            },
            Toggle { data: 5, ..AUTO_EXPORT },
            // CycleEPPButton: 64, 1, 4.
            Toggle {
                entry: 26,
                key: "inscriber_input_capacity",
                stored: Stored::Text,
                default: 0,
                data: 6,
                title: "",
                values: &[
                    ("SIXTY_FOUR", icon::INSCRIBER_BUFFER_64, "gui.extendedae.ex_inscriber.stackable"),
                    ("ONE", icon::INSCRIBER_BUFFER_1, "gui.extendedae.ex_inscriber.unstackable"),
                    ("FOUR", icon::INSCRIBER_BUFFER_4, "gui.extendedae.ex_inscriber.fourstackable"),
                ],
                place: None,
                shown_by: None,
            },
            Toggle {
                entry: 22,
                key: "ae2x_page",
                stored: Stored::Index,
                default: 0,
                data: 7,
                title: "",
                values: &[
                    ("0", icon::ARROW_RIGHT, "gui.extendedae.ex_inscriber.next"),
                    ("1", icon::ARROW_RIGHT, "gui.extendedae.ex_inscriber.next"),
                    ("2", icon::ARROW_RIGHT, "gui.extendedae.ex_inscriber.next"),
                    ("3", icon::ARROW_RIGHT, "gui.extendedae.ex_inscriber.next"),
                ],
                place: None,
                shown_by: None,
            },
            Toggle {
                entry: 23,
                key: "ae2x_page",
                stored: Stored::Index,
                default: 0,
                data: 7,
                title: "",
                values: &[
                    ("0", icon::ARROW_LEFT, "gui.extendedae.ex_inscriber.pre"),
                    ("1", icon::ARROW_LEFT, "gui.extendedae.ex_inscriber.pre"),
                    ("2", icon::ARROW_LEFT, "gui.extendedae.ex_inscriber.pre"),
                    ("3", icon::ARROW_LEFT, "gui.extendedae.ex_inscriber.pre"),
                ],
                place: None,
                shown_by: None,
            },
        ],
        progress: Some(Progress { name: "progressBar", current: 0, max: 2 }),
        priority: None,
        decorations: &[],
        grid: None,
        cogs: &[],
        lock_line: None,
        page_line: Some(PageLine { data: 7, key: "gui.extendedae.ex_inscriber.number", fallback: "Inscription Job %s", at: (8.0, 16.0), per_value: false, count: false }),
        panels: &[],
    }),
    // ContainerExMolecularAssembler: the crafting grid of the job (EX_1, as a
    // grid of three) and its output, five cards, the progress bar. The port's
    // assembler runs one job (see `devices.rs`), so it has the first page only;
    // no pattern slot (the thread has none).
    ("extendedae:ex_molecular_assembler", Layout {
        kind: MachineKind::MolecularAssembler,
        style: "ex_molecular_assembler.json",
        slots: &[
            (0, "EX_1"), (1, "EX_1"), (2, "EX_1"), (3, "EX_1"), (4, "EX_1"), (5, "EX_1"), (6, "EX_1"), (7, "EX_1"), (8, "EX_1"),
            (9, "MACHINE_OUTPUT"),
        ],
        icons: &[],
        upgrades: 11..16,
        toggles: &[],
        progress: Some(Progress { name: "progressBar", current: 0, max: 2 }),
        priority: None,
        decorations: &[],
        grid: Some((0..9, 4)),
        cogs: &[],
        lock_line: None,
        page_line: Some(PageLine { data: 5, key: "gui.extendedae.ex_molecular_assembler.number", fallback: "Crafting Job %s", at: (8.0, 16.0), per_value: false, count: false }),
        panels: &[],
    }),
    // ContainerAssemblerMatrix, GuiAssemblerMatrix: four rows of nine
    // patterns (`AssemblerMatrixSlot`s, placed by the screen's code: the
    // style's `EAE_MATRIX_PATTERN`, `style_slots`), a pattern core's 36 at a
    // time (the mod scrolls all its cores' rows); the left toolbar's Cancel
    // and Pattern Access Terminal buttons, then the next and previous core;
    // "Running Jobs: %s" (data slot 1).
    ("extendedae:assembler_matrix", Layout {
        kind: OWN,
        style: "assembler_matrix.json",
        slots: &[(0, "EAE_MATRIX_PATTERN"), (1, "EAE_MATRIX_PATTERN"), (2, "EAE_MATRIX_PATTERN"), (3, "EAE_MATRIX_PATTERN"), (4, "EAE_MATRIX_PATTERN"), (5, "EAE_MATRIX_PATTERN"), (6, "EAE_MATRIX_PATTERN"), (7, "EAE_MATRIX_PATTERN"), (8, "EAE_MATRIX_PATTERN"), (9, "EAE_MATRIX_PATTERN"), (10, "EAE_MATRIX_PATTERN"), (11, "EAE_MATRIX_PATTERN"), (12, "EAE_MATRIX_PATTERN"), (13, "EAE_MATRIX_PATTERN"), (14, "EAE_MATRIX_PATTERN"), (15, "EAE_MATRIX_PATTERN"), (16, "EAE_MATRIX_PATTERN"), (17, "EAE_MATRIX_PATTERN"), (18, "EAE_MATRIX_PATTERN"), (19, "EAE_MATRIX_PATTERN"), (20, "EAE_MATRIX_PATTERN"), (21, "EAE_MATRIX_PATTERN"), (22, "EAE_MATRIX_PATTERN"), (23, "EAE_MATRIX_PATTERN"), (24, "EAE_MATRIX_PATTERN"), (25, "EAE_MATRIX_PATTERN"), (26, "EAE_MATRIX_PATTERN"), (27, "EAE_MATRIX_PATTERN"), (28, "EAE_MATRIX_PATTERN"), (29, "EAE_MATRIX_PATTERN"), (30, "EAE_MATRIX_PATTERN"), (31, "EAE_MATRIX_PATTERN"), (32, "EAE_MATRIX_PATTERN"), (33, "EAE_MATRIX_PATTERN"), (34, "EAE_MATRIX_PATTERN"), (35, "EAE_MATRIX_PATTERN")],
        icons: &[],
        upgrades: 0..0,
        toggles: &[
            Toggle {
                entry: 36,
                key: "eae_matrix_cancel",
                stored: Stored::Index,
                default: 0,
                data: 2,
                title: "gui.extendedae.assembler_matrix.cancel",
                // Icon.CLEAR.
                values: &[("CANCEL", (96, 0), "gui.extendedae.assembler_matrix.cancel")],
                place: None,
                shown_by: None,
            },
            Toggle {
                entry: 37,
                key: "terminal_visible",
                stored: Stored::Flag(true),
                default: 1,
                data: 0,
                title: "gui.ae2.PatternAccessTerminal",
                values: &[
                    (NO_YES[0], icon::PATTERN_ACCESS_HIDE, "gui.ae2.PatternAccessTerminalHint"),
                    (NO_YES[1], icon::PATTERN_ACCESS_SHOW, "gui.ae2.PatternAccessTerminalHint"),
                ],
                place: None,
                shown_by: None,
            },
            Toggle {
                entry: 38,
                key: "ae2x_page",
                stored: Stored::Index,
                default: 0,
                data: 2,
                title: "",
                values: &[("NEXT", icon::ARROW_RIGHT, "gui.extendedae.ex_interface.next")],
                place: None,
                shown_by: None,
            },
            Toggle {
                entry: 39,
                key: "ae2x_page",
                stored: Stored::Index,
                default: 0,
                data: 2,
                title: "",
                values: &[("PREVIOUS", icon::ARROW_LEFT, "gui.extendedae.ex_interface.pre")],
                place: None,
                shown_by: None,
            },
        ],
        progress: None,
        priority: None,
        decorations: &[],
        grid: None,
        cogs: &[],
        lock_line: None,
        page_line: Some(PageLine { data: 1, key: "gui.extendedae.assembler_matrix.threads", fallback: "Running Jobs: %s", at: (80.0, 19.0), per_value: false, count: true }),
        panels: &[],
    }),
    // ContainerVoidCell, GuiVoidCell: the three modes' buttons in their places,
    // "Mode: %s" of the mode below them (data slot 0, the mode's index).
    (horizon_client::ae2x::extendedae::VOID_CELL, Layout {
        kind: OWN,
        style: "void_cell.json",
        slots: &[],
        icons: &[],
        upgrades: 0..0,
        toggles: &[
            Toggle {
                entry: 0,
                key: "void_mode",
                stored: Stored::Index,
                default: 0,
                data: 0,
                title: "gui.extendedae.void_cell.mode.0",
                values: &[("TRASH", icon::CONDENSER_OUTPUT_TRASH, "gui.extendedae.void_cell.mode.0")],
                place: Some((22, 20)),
                shown_by: None,
            },
            Toggle {
                entry: 1,
                key: "void_mode",
                stored: Stored::Index,
                default: 0,
                data: 0,
                title: "gui.extendedae.void_cell.mode.1",
                values: &[("MATTER_BALLS", icon::CONDENSER_OUTPUT_MATTER_BALL, "gui.extendedae.void_cell.mode.1")],
                place: Some((54, 20)),
                shown_by: None,
            },
            Toggle {
                entry: 2,
                key: "void_mode",
                stored: Stored::Index,
                default: 0,
                data: 0,
                title: "gui.extendedae.void_cell.mode.2",
                values: &[("SINGULARITY", icon::CONDENSER_OUTPUT_SINGULARITY, "gui.extendedae.void_cell.mode.2")],
                place: Some((84, 20)),
                shown_by: None,
            },
        ],
        progress: None,
        priority: None,
        decorations: &[],
        grid: None,
        cogs: &[],
        lock_line: None,
        page_line: Some(PageLine { data: 0, key: "gui.extendedae.void_cell.mode.", fallback: "Mode", at: (5.0, 42.0), per_value: true, count: false }),
        panels: &[],
    }),
    // ContainerCircuitCutter: one input, the output, four cards; as the assembler's.
    ("extendedae:circuit_cutter", Layout {
        kind: OWN,
        style: "circuit_cutter.json",
        slots: &[(0, "MACHINE_INPUT"), (1, "MACHINE_OUTPUT")],
        icons: &[],
        upgrades: 2..6,
        toggles: &[AUTO_EXPORT],
        progress: Some(Progress { name: "progressBar", current: 0, max: 2 }),
        priority: None,
        decorations: &[],
        grid: None,
        cogs: &[],
        lock_line: None,
        page_line: None,
        panels: &[],
    }),
    // ContainerWirelessConnector, ContainerWirelessHub (UpgradeableMenu): the
    // four energy cards; the hub's Disconnect buttons where GuiWirelessHub puts
    // them (window entries 10 to 17, `killPort`).
    (horizon_client::ae2x::extendedae::WIRELESS_CONNECTOR, Layout {
        kind: OWN,
        style: "wireless_connector.json",
        slots: &[],
        icons: &[],
        upgrades: 0..4,
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
    (horizon_client::ae2x::extendedae::WIRELESS_HUB, Layout {
        kind: OWN,
        style: "wireless_hub.json",
        slots: &[],
        icons: &[],
        upgrades: 0..4,
        toggles: &[
            Toggle { entry: 10, key: "", stored: Stored::Index, default: 0, data: 0, title: "gui.wireless_hub.disconnect.tooltip", values: &[("press", icon::INVALID, "gui.wireless_hub.disconnect.tooltip")], place: Some((55, 42)), shown_by: None },
            Toggle { entry: 11, key: "", stored: Stored::Index, default: 0, data: 0, title: "gui.wireless_hub.disconnect.tooltip", values: &[("press", icon::INVALID, "gui.wireless_hub.disconnect.tooltip")], place: Some((55, 70)), shown_by: None },
            Toggle { entry: 12, key: "", stored: Stored::Index, default: 0, data: 0, title: "gui.wireless_hub.disconnect.tooltip", values: &[("press", icon::INVALID, "gui.wireless_hub.disconnect.tooltip")], place: Some((55, 98)), shown_by: None },
            Toggle { entry: 13, key: "", stored: Stored::Index, default: 0, data: 0, title: "gui.wireless_hub.disconnect.tooltip", values: &[("press", icon::INVALID, "gui.wireless_hub.disconnect.tooltip")], place: Some((55, 126)), shown_by: None },
            Toggle { entry: 14, key: "", stored: Stored::Index, default: 0, data: 0, title: "gui.wireless_hub.disconnect.tooltip", values: &[("press", icon::INVALID, "gui.wireless_hub.disconnect.tooltip")], place: Some((138, 42)), shown_by: None },
            Toggle { entry: 15, key: "", stored: Stored::Index, default: 0, data: 0, title: "gui.wireless_hub.disconnect.tooltip", values: &[("press", icon::INVALID, "gui.wireless_hub.disconnect.tooltip")], place: Some((138, 70)), shown_by: None },
            Toggle { entry: 16, key: "", stored: Stored::Index, default: 0, data: 0, title: "gui.wireless_hub.disconnect.tooltip", values: &[("press", icon::INVALID, "gui.wireless_hub.disconnect.tooltip")], place: Some((138, 98)), shown_by: None },
            Toggle { entry: 17, key: "", stored: Stored::Index, default: 0, data: 0, title: "gui.wireless_hub.disconnect.tooltip", values: &[("press", icon::INVALID, "gui.wireless_hub.disconnect.tooltip")], place: Some((138, 126)), shown_by: None },
        ],
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

/// `GuiAssemblerMatrix.drawFG`: its pattern slots at `col * 18 + 8`,
/// `(row + 1) * 18 + 13`, which its style does not hold.
pub(super) fn style_slots(block: &str, style: &mut horizon_client::ae2_screen_style::ScreenStyle) {
    use horizon_client::ae2_screen_style::{Grid, Position, SlotStyle};
    if block == "extendedae:assembler_matrix" {
        let position = Position { left: Some(8), top: Some(31), right: None, bottom: None };
        style.slots.insert("EAE_MATRIX_PATTERN".into(), SlotStyle { position, grid: Some(Grid::BreakAfter(9)), hidden: false });
    }
}

/// The layout of block `block`'s screen.
pub(super) fn layout(block: &str) -> Option<&'static Layout> {
    LAYOUTS.iter().find(|(id, _)| *id == block).map(|(_, layout)| layout)
}
