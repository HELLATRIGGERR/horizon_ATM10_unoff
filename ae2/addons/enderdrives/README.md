# Ender Drives 1.5.23

[Ender Drives](https://github.com/sts15/EnderDrives) by STS15, as All the Mods 10 8.4 ships it
(`enderdrives-neoforge-1.21.1-1.5.23.jar`; its `mods.toml` points at another of the author's repositories). Code MIT
(`LICENSE`). The repository has no tags and its public 1.21.1 history ends at 1.5.0 (commit `264ec0a`, 4 March 2026);
what 1.5.23 changed was read from the jar's classes: `DuplicateCellGuard` and the `AppEngCellInventory` and `Drive`
mixins that bind it, `EnderDBManager.insertItem` and its clamped deltas, `AbstractEnderDiskInventory`, the config's
defaults. The port's files that also carry AE2's rules stay LGPL-3.0 with them. Its assets state no licence of their
own; they are generated locally from the player's jar and never committed. Provenance in
[SOURCE_MANIFEST.json](SOURCE_MANIFEST.json), the declared content in [content.json](content.json).

Feature `ae2-enderdrives`; content `content/enderdrives` from
`ATM10_DIR=... bun run ae2/scripts/prepare-addon.ts enderdrives`; settings from `config/enderdrives-server.toml`
(`[mods.enderdrives]` in `pack/pack.toml`). It is a server config, so All the Mods 10 ships none and the mod's
defaults hold: every disk on, type limits 1, 31, 63, 127 and 255 (the creative disk `Integer.MAX_VALUE`), the
global scope by default and all three scopes on, frequencies 0 to 4095.

## Slice 1: the Ender Item Storage Cells

| Ender Drives 1.5.23 | Port |
|---|---|
| `ItemInit`: six `EnderDiskItem`s (1k to 256k, creative, stack size 1), five Ender Storage Components and the Ender Item Housing; the jar's recipes, with NeoForge's `mod_loaded` conditions choosing among the vanilla, MEGA, ExtendedAE and Advanced AE ones, and AE2's `storage_cell_disassembly` | the items (`content.json`) and the 49 recipes of them; the fluid disks', the fluid housing's and the tape disk's wait for their slices |
| `StorageCells.addCellHandler(EnderDiskInventory.HANDLER)`, `StorageCellModels.registerModel(disk, block/drive/ender_cell)`, `CellConfig.create(items, stack)` (63 filters), no cards | cells of the mod's own handler in the definitions registry (`ae2x/enderdrives.rs`); the game's `HandlerCell` (`game/ae2x/enderdrives/`) for listing, storage, preference, the ME Chest and the IO Port |
| `AbstractEnderDiskItem`: `ender_freq`, `ender_scope` (0 global, 1 private, 2 team; `FrequencyScope.fromId` falls back to the default when a scope is off), `ender_owner`, `ender_team`, `ender_team_name`, `ender_transfer_mode` (0 both ways, 1 input only, 2 output only) in `minecraft:custom_data`; `getSafeScopePrefix`: `global`, `player_<owner>` (`player_unknown`), `team_<id>` (global without a team) | the same keys and prefixes |
| `EnderDBManager`: one world-wide map of (prefix, frequency, item) to a count, `insertItem` refusing a new type at the type limit and adding up to `Long.MAX_VALUE`, `saveItem` taking out at most what is there; `data/enderdrives/enderdrives.bin` with a WAL, committed every few seconds | `Store`: the same counts, per world; saved with the world (`save_game_metadata`) to `data/enderdrives/horizon_enderdrives.dat`, each item in AE2's `GenericStack` form; read on first use in a world. A transaction of the port's that fails or only simulates puts the counts back with the blocks (`ae2_snapshot_blocks`) |
| `EnderDiskInventory`: `insert` (none when off, output only, outside the partition, or a new type at the limit; a simulation takes all), `extract` (none when off or input only), `getAvailableStacks` (everything of the frequency), `isPreferredStorageFor` (a stored key), `persist` (nothing), `getStatus` (`calculateCellState`: EMPTY, NOT_EMPTY, TYPES_FULL from three quarters of the limit, FULL), `getIdleDrain` (100 + 0.015 × max(1, items)^0.8 AE/t) | the same; the drive's idle power reads the drain each time the network is built, its LED (the drive's and a Cell Dock's) the frequency's types |
| `DuplicateCellGuard` (1.5.23), bound by the `Drive` mixins: of the disks of one storage in AE2's Drives on one grid, the one in the drive of the highest priority, then the least dimension id, x, y, z and slot, works; the others are FULL, draw nothing and give and take nothing; a disk in a drive that is offline does nothing; a disk in another host (an ME Chest, a Cell Dock) is never bound and always works; an addon's drive (`getMachines(DriveBlockEntity.class)` takes AE2's class alone) is bound but never a candidate | `ae2x_cell_mounted` on the grid's cell enumerations: a sleeping disk is not mounted; which disks sleep is kept each grid tick for the drain and the LEDs |
| `AbstractEnderDiskItem.appendHoverText`: off on the server, else the types used of the limit (coloured by use), the frequency, the mode, the scope (the owner's name when it is the player, else the UUID; the team's name), the partition's size | the same lines, the mod's language keys; on a client of a remote server the types read 0 (the mod asks the server, slice 2) |
| `UpdateFrequencyPacket` (`enderdrives:update_frequency`: frequency, scope, mode as VarInts): the main hand's disk set, a private one owned by the sender | a guest's payload does the same; `/ae2 enderdrives set <frequency> [global\|private\|team] [bidirectional\|input\|output]` does it for the acting player (in the screen's range), `info` and `store <prefix> <frequency>` show a storage |

## Slice 2: the frequency screen and disassembly

| Ender Drives 1.5.23 | Port |
|---|---|
| `AbstractEnderDiskItem.use`, not sneaking: a disk the config turns off says `tooltip.enderdrives.enderdisk.disabled` over the hotbar and fails; else the client opens `EnderDiskFrequencyScreen` (`ClientItemHooks.openFrequencyScreen`) and answers `sidedSuccess` | `horizon-ae2x-enderdrives` `Ae2Addon::item_use`, called before AE2's cell use (`ae2_cell_item_use`) and on a remote client after its `UseItem` (`remote_use_item`); the static rule `item_opens_menu` stops the click at that hand |
| `use`, sneaking: `StorageCellDisassemblyRecipe.getDisassemblyResult` for the disk, the selected slot emptied, the results placed back; no content check (the storage is the world's); the client only consumes | AE2's own cell disassembly (`ae2_cell_disassembly.rs`) with `Ae2Addon::handler_cell_disassembly` answering "empty" for every disk; the recipes are the jar's `cell_upgrade/ender_disk_*.json` |
| `EnderDiskFrequencyScreen` (176 x 105, the alt theme): the particles flying at the window from the screen's edges (`Particle2D`), the three dye selectors (`fillRoundedRect` in `DyeColor.getTextColor`), the end portal under `ender_disk_mask_alt`, the title, the arrows' highlights, the borderless box (four characters, its label while empty), the transfer mode (`CustomImageCycleButton`) and scope (`CustomImageButton`) buttons, the tooltips | `screen.rs`, an addon screen of its own (`Screen::Ae2Addon`, `Ae2Host::open_addon_screen`); its textures through content.json's `own_gui_textures` (`ae2:enderdrives/gui/*`) |
| `mouseClicked`: an arrow steps its dye by one (mod 16) and `updateFrequency` (the dyes' value clamped to the config's range, written to the box); the box's responder reads a frequency in range; `cycleScope` skips Team without FTB Teams; `TransferMode.next` | the same (`Open::update`, `set_field`, `cycle_scope`) |
| `onClose`: `UpdateFrequencyPacket` (the frequency clamped, the scope, the mode) | to the server the game plays on (`Ae2Host::send_to_server`; the integrated server's guest payload of slice 1 applies it), or straight to the main hand's disk in a world of the game's own |

Differences, known:

- The end portal behind the mask is a field of drifting specks in the portal's colours: the port draws no portal
  shader in a screen. The particles use the port's own random numbers.
- The box takes typed characters at its end and Backspace; no cursor movement or selection.
- The storage is saved with the world, not on its own every few seconds, and in the port's format: Java's
  `enderdrives.bin` (1.21.1 item NBT) is not read. A world of the mod's does not come to 26.3 anyway.
- Which disks sleep is decided each grid tick for the drain and the LEDs (the listing and storage ask at once).
- The duplicate's `enderdrives_duplicate_sleep` flag is not written to its stack, so its tooltip line is missing.
- FTB Teams is not in the port: a team disk is global unless it already carries a team.

Tests: `horizon-client-lib` (the config, the disks' data and prefixes, the storage's limits, undo and save, the
registry rows) and `horizon-client` (two disks sharing a storage, modes, partitions, limits, a failed transaction, the
frequency packet, the screen's dyes, box, scope cycle and packet), and the world test `enderdrives-disks` (slice 2:
the screen's arrows, box, buttons and close, a disabled disk's message, disassembly; two screenshots).

## Later

- Slice 2b: the item models' `enderdrives:status` and the LED tint (`ClientModEvents`); the IO Port mixins
  (`IOPortBlockEntityMixin`, `TileExIOPortMixin`) and the IO Port's fullness check for these cells; the type counts a
  client of a remote server asks for (`RequestDiskTypeCountPacket`); the duplicate-sleep flag and line.
- Slice 3: the Ender Fluid Storage Cells and the Ender Fluid Housing (`EnderFluidDBManager`), which need fluid keys in
  the addon handler cells.
- Slice 4: the Tape Disk (`TapeDBManager`, per-disk storage by UUID, byte limit).
- Slice 5: `ModCommands`, the JEI plugin, the FTB Teams scope.
