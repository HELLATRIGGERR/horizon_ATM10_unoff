# ExtendedAE 1.21-2.2.42

[ExtendedAE](https://github.com/GlodBlock/ExtendedAE) by GlodBlock, models and textures by Sea_Kerman and
Ridanisaurus, as All the Mods 10 8.4 ships it. Code LGPL-3.0 (`LICENSE.txt`; 2.2.42 has no tag, its source is
commit `1f86f927` on branch `1.21.1-neoforge`), so the port's derived files carry the same terms; the port's code is written from the mod's behaviour, not copied.
The licence of the assets is not stated; they are generated locally from the player's jar and never
committed. Provenance in [SOURCE_MANIFEST.json](SOURCE_MANIFEST.json), the declared content in
[content.json](content.json).

Feature `ae2-extendedae`; content `content/extendedae` from
`ATM10_DIR=... bun run ae2/scripts/prepare-addon.ts extendedae`. ATM10's `config/extendedae-common.toml` keeps
the mod's defaults (ATM10 8.4 ships none: `extended_io_bus_multiplier` 8, `infinity_cell_energy_cost` 8,
`assembler_matrix_max_size` 6, `wireless_connector_max_range` 1,000, `wireless_connector_power_multiplier` 1, `pattern_modifier_multipliers` 2, 3, 5, 7, the mod's
`me_packing_tape_whitelist`). Of
ATM10's KubeJS, only its infinity cells (slice 6) touch the mod; its recipes for them are the pack's own, in
the full pack's carry.

From 2.2.41 (ATM10 8.3) to 2.2.42 the mod changed two classes and nothing else: `CraftingMatrixThread` resets the
Assembler Matrix's progress on its cached-pattern path, and `ContainerRenamer` saves the block or part it renamed.
The Assembler Matrix of slice 3 has no cached-pattern path (below); the renamer is not carried. The jar's assets
and data are the same.

## Slice 1: Extended Pattern Provider, Interface and Drive, their parts and kits

| ExtendedAE | Port |
|---|---|
| `TileExPatternProvider`, `PartExPatternProvider`: `PatternProviderLogic(.., 36)`; nine return slots | a `MachineVariant` row in the definitions registry (`horizon-client/src/ae2x/extendedae.rs`, `Defs::add_variant`) on `MachineKind::PatternProvider`: all provider behaviour is AE2's, with 36 slots |
| `TileExInterface`, `PartExInterface`: `InterfaceLogic(.., 36)`: 36 config and 36 storage slots, one card | a row on `MachineKind::Interface`; `ae2_interface.rs` goes by the machine's size |
| `TileExDrive`: `getCellCount()` 20; idle 2 AE/t plus its cells' | a row on `MachineKind::Drive` |
| `ContainerExPatternProvider`, `screens/ex_pattern_provider.json` (176 x 245): 4 rows of 9 patterns at once, no pages in this version, then the return row | `ae2x/extendedae/screens.rs`, a window of 54 entries (`ae2_machine_ui::MenuShape`): patterns 0..36, returns 36..45, settings 45..48, priority 53 |
| `ContainerExInterface`, `GuiExInterface`, `screens/ex_interface.json` (176 x 253): pages of 18 config and 18 storage slots, Next/Previous Page on the toolbar, stock cogs above the config rows, "Config Page %s" | the same window: storage 0..18, config 18..36, card 36, fuzzy 41, page 43, priority 53; the page belongs to the machine and is shared by its viewers, as `IPage` has it |
| `ContainerExDrive`, `screens/ex_drive.json` (176 x 201): `STORAGE_CELL` ten on the left, `EX_1` ten on the right, priority tab | a 27-entry window: cells 0..20, priority 26 |
| `ExDriveBakedModel`, `ExDriveTESR`: the cells in two banks of 2 x 5, the second as on a drive of the opposite facing (the back of the block), with their LEDs | `ae2_render.rs`, the base model from `extended_me_drive_base` |
| `ItemUpgrade`: `pattern_provider_upgrade`, `interface_upgrade`, `drive_upgrade` on AE2's exact device, in a sneak-use; block state and all contents kept; a new part in the same place on a cable; used up one per use, in creative too | `ae2x/extendedae/upgrade.rs`, reached by `game/ae2x/mod.rs` (`ae2x_use_on`) |
| `part/ex_pattern_provider_base`, `part/ex_interface` over AE2's status models | `ae2_render.rs`, by `game/ae2x/mod.rs` (`part_model`) |
| shapeless recipes: part from block and back, kit from device (or part) and `#c:ingots` | the mod's seven recipes, copied as they are |

AE2-namespace files of the jar (`assets/ae2/screens/ex_*.json`, `assets/ae2/textures/guis/ex_*.png`) stay in the
addon's content under `assets/extendedae/ae2/`, as a content mod owns its namespace alone: the style loader looks
there first (`ScreenStyle::load_over`), and the GUI atlas names those textures `ae2:guis/<name>` as in the jar.

Differences, known:

- The devices themselves come from Crystal Assembler recipes (`assembler/ex_*.json`), carried with that machine
  in slice 2 (below).
- Memory Cards do nothing on these devices yet: a card's settings go with AE2's nine slots.
- A part's Interface page is kept across a restart (it lives in the part's record); a block's resets, as in the mod.
- No in-frame window for them: `--in-frame` is deprecated, and AE2's Pattern Provider has none either.
- The guide pages, the mod's advancements and its Pattern Access Terminal come with later slices.
- AE2's plain Pattern Provider and Interface parts now also open their AE2 screens, by the same `ae2:menu_block` message.

Tests: `horizon-client` (the rows and their sizes in `ae2x::extendedae`, the kits, window numbering in
`ae2_machine_ui`, screen styles) and the world test `extendedae-machines`: 20 cells listed and on the screen, 36
patterns installed and pushing from their first and last, stocking on both pages, parts, kits on blocks and a part,
the recipes, all through a restart, and broken (`setblock ... destroy`) each device drops what it holds:

```sh
AE2_TEST_STARTUP_MS=600000 bun run ae2/scripts/test-world.ts extendedae-machines
```

## Slice 2: Crystal Assembler, Circuit Slicer, Crystal Fixer and their recipe types; Extended IO Port, Import and Export Buses

| ExtendedAE | Port |
|---|---|
| `TileCrystalAssembler`: nine inputs, one output, a fluid tank of 16,000 mB (`GenericStackInv`, fluids, no extraction), four speed cards, 8,000 AE of its own; connects and takes Forge Energy on every side but its top; no channel, no idle draw | an `ExtMachine` row (`ae2x/extendedae.rs`, `OWN_MACHINES`); its tick, recipe search and push-out in `game/ae2x/extendedae/machines.rs` |
| `CrystalAssemblerRecipe` (`extendedae:crystal_assembler`): `input_items` of `{ingredient, amount}`, an optional `input_fluid`, `output`; `testRecipe` lets each ingredient take, in turn, from copies of the inputs in slot order and then the tank | `machines::Recipe`, `test`, `run`; the jar's recipes the slices can use, and those with the mod's own `extendedae:config` condition, which `prepare-addon.ts` answers from ATM10's config (`enable_crystal_assembler_inscribe_processors = true`) |
| `RecipeExecutor` with `FCUtil.speedCardMap`: 2, 3, 5, 10 or 50 progress a step for 0 to 4 cards, ten AE a progress point, from its own buffer first, then the grid; 200 to finish; the Inscriber's tick rates | `machines::execute`; a craft costs 2,000 AE (2,010 with one card: 67 steps of 3) |
| Glodium's `RecipeSearchContext`: the last recipe tried first, `stuck` after a search that found nothing twice, `onInvChange` on any slot or the tank | `machines::Ctx`, kept by block entity and not saved: `load` reads the ids from the outer tag, so a reloaded machine has no recipe, and its progress, not saved either, starts over |
| `pushOutResult` (`FCUtil.ejectInv`): with AUTO_EXPORT, up to 64 of the output to the first output side, in `Direction` order, whose block entity takes any (another of the same machine is passed over) | `extendedae_push_out`; the output sides are saved (`output_side`), all six until set |
| `ContainerCrystalAssembler`, `GuiCrystalAssembler`, `screens/crystal_assembler.json`: the inputs in threes, the tank as a STORAGE slot, a vertical progress bar, AUTO_EXPORT on the toolbar; a bucket emptied on the tank slot fills it | a `MenuShape` window of 27: inputs 0..9, output 9, cards 10..14, the tank 20, AUTO_EXPORT 24, the output sides 14..20 (`screens.rs`) |
| `TileCircuitCutter` (the Circuit Slicer, `extendedae:circuit_cutter`): the same with one input and no tank; connects on all but its front and back | the same code (`machines::Kind::Cutter`) |
| `TileCrystalFixer` (`extendedae:crystal_fixer`): one fuel slot, refuelled by a use of the block (an empty hand takes it back); 2 or 3 progress a step at 50 AE a point, 100 to finish, at 2 to 10 ticks; at the end the fuel is burnt and, with the recipe's chance in 10,000, the block in front becomes the output's default state; connects only at its back; progress saved | `machines::Kind::Fixer`, `FixerRecipe`; refuelling by `ae2x_block_use` |
| `TileExIOPort`: AE2's IO Port with five cards, 2,048 items a tick doubled by each speed card (AE2's: 256, three cards) | a `MachineVariant` on `MachineKind::IoPort`; `ae2x::io_port_budget` |
| `PartExImportBus`, `PartExExportBus`: AE2's buses with eight cards (`getUpgradeSlots`) and `getOperationsPerTick() * busSpeed`, ATM10's `extended_io_bus_multiplier` = 8: 8 items a tick with no card, 768 with four speed cards (96 x 8); the same card limits; `ContainerExIOBus` is AE2's IO bus menu with their styles (`ex_import_bus.json`, `ex_export_bus.json` over `io_bus.json`); their base models over AE2's status ones | part `MachineVariant`s of kind `CableBus` (a part, no machine): `base_id` gives the bus code AE2's id (`ae2_buses::part_kind`), `ae2_world::bus_card_slots` their eight cards, `BusState::multiplier` the factor (not saved; `ae2x::bus_multiplier` from `Config::bus_speed`). The bus window keeps AE2's five card entries 36..41 and puts the three more at 45..48 (`ae2_bus_ui::bus_card`) |
| `ItemIOBusUpgrade` (`extendedae:io_bus_upgrade`): sneak-used on AE2's Import or Export Bus, the extended one in its place with all it held | a `Kit` with parts and no block (`upgrade.rs`) |
| `EAERegistryHandler.registerUpgrades` | `Defs::add_upgrade` rows for each device, which `ae2_machine_ui::machine_upgrade_allowed` now reads for every addon machine |
| materials: the concurrent processor, its print and press, the machine frame, entro crystal, dust and ingot, entro and silicon blocks | plain items and blocks of `content.json`, with their crafting, inscriber and transform recipes |
| `TileExInscriber`: four `InscriberThread`s, each AE2's `InscriberBlockEntity.tickingRequest` on four slots of its own (top, bottom, middle, output; the mod copied it), sharing the block's four speed cards, 1,600 AE buffer, sides and auto-export settings and stack size (`setInvStackSize`: 64, 1 or 4); `tickingRequest` the most urgent of the threads'; connects and takes Forge Energy on all but its front; no channel | an `ExtMachine` of 16 slots (`OWN_MACHINES`); each thread runs as AE2's Inscriber on its slots (`game/ae2x/extendedae/devices.rs`, `thread`), through AE2's `inscriber_tick` and `ae2_inscriber_push_output`; a thread's progress and smash are not saved (`reset_unsaved`), as in the mod |
| `getExposedInventoryForSide` (`CombinedInternalInventory` of the threads, `BaseFilter`, `AutomationFilter`) | the device's item handler (`extendedae_item_insert`, `_extract`, `_candidates`), which hoppers, buses and providers meet first (`game/ae2x/mod.rs`, `ae2x_item_insert`): thread by thread, AE2's Inscriber's rules on each |
| `ContainerExInscriber`, `GuiExInscriber`, `screens/ex_inscriber.json`: a page a thread (`IPage`), AE2's Inscriber's places, separate sides, auto export, the stack size, next and previous page, "Inscription Job %s" | a 27-entry window: the shown thread's slots 0..4, cards 4..8, next page 22, previous 23, auto export 24, sides 25, stack size 26 (`extendedae_inscriber_click`); the page is the block's, shared by its viewers, as the Extended Interface's |
| `TileExCharger`: four slots of one (`AppEngInternalInventory(.., 4, 1, ChargerInvFilter)`), 3,200 AE; `doWork` every `TickRates.Charger`: each slot charges its item from the buffer, then the grid, or (buffer over 3,199) runs its charger recipe one call in five for the whole buffer, and after each slot a buffer under 3,199 takes up to 800 AE from the grid; no menu: a use puts one held item in the first free slot, else drops the first item in front (`activate`) | an `ExtMachine` of four slots; its tick (`extendedae_charger_tick`) with AE2's chargeable items (`ae2_processing::chargeable_*`), its use through `ae2x_block_use` |
| `TileExMolecularAssembler`: eight `CraftingThread`s of ten slots, a job each from pattern providers (`pushPattern` to the first free thread); 20, 26, 34, 40, 50 or 100 progress a tick at 1 to 5 AE a point for 0 to 5 speed cards (AE2's is half); nothing goes in by automation, the output comes out | a `MachineVariant` of AE2's Molecular Assembler (11 slots, five cards) of eight threads (slice 3: `game/ae2_assembler_threads.rs`, the first AE2's slots, the others in its config); the mod's speeds (`assembler_speed`, read by `ae2_tick_assembler`); its item handler takes nothing in |
| `ContainerExMolecularAssembler`, `screens/ex_molecular_assembler.json` | its first page (the first thread): the grid (EX_1), the output, five cards, the progress |
| `BlockBuddingEntro` (four, fully to hardly entroized): `randomTick` one in three grows a cluster on a random side (in air or a water source, waterlogged then; a cluster facing away one size up), then one growth in ten wears it a step down, the hardly entroized one to AE2's quartz block | `game/ae2x/extendedae/entro.rs`, reached from AE2's budding quartz tick (`ae2_random_tick_certus`, `ae2x_random_tick`); the blocks random-tick by their declarations; AE2's Growth Accelerator ticks them as `#c:budding_blocks` |
| `BlockEntroCluster` (small, medium, large bud and cluster): light 5 to 8, strength 2.5, on the block behind them | declarations on amethyst buds' and cluster's templates (shape, survival, drops by their loot tables) |
| `ItemEntroSeed.useOn`: on AE2's fluix block, one seed for a fully entroized budding block | `extendedae_seed_use`, through `ae2x_use_on` |
| the fixer's entro recipes (`fixer/entro_*`), the assembler's `budding`, `ex_inscriber`, `ex_charger`, `ex_molecular_assembler`, the shard's, seed's and dust's recipes | the jar's, carried with the items |

Checked locally against ATM10's final recipes (a NeoForge run of the whole pack): each Crystal Assembler, Circuit
Slicer and Crystal Fixer recipe this port carries is there under its id, identical in every field. The pack's
other recipes of these types need devices of later slices or come from other mods; the full pack's carries
bring those. Of the 62 recipes the content carries now, two differ in the pack, by AlmostUnified's tags:
`silicon_block` takes `#c:silicon` for its middle, `transform/entro_ingot` `#c:gems/lapis`; the full pack's
carry has those.

Differences, known:

- The output side screen (`OutputSideConfig`) is not drawn; the sides are kept and set by the window's entries 14
  to 19, and a Memory Card does not copy them (`EXTRA_SETTING`).
- A recipe search goes in the recipes' load order; Java's in its recipe manager's hash order. Only inputs that
  two recipes both accept can tell.
- The tank fills from water and lava buckets in the window only: the port has no fluid pipes, and AE2's export bus
  and pattern provider do not push fluids into it yet.
- A Memory Card does not take an Extended Import or Export Bus's settings yet, nor an Extended Inscriber's.
- The Extended Molecular Assembler's screen shows its first thread only; its other seven craft as the mod's
  (since slice 3) but have no pages.
- The Extended Inscriber's and Molecular Assembler's next and previous page buttons stay at the last and first
  page instead of hiding; a page is the block's, not each viewer's.
- No crank on the Extended Inscriber and Charger (`ICrankPowered`), no lightning on a working charger.
- The jar's `ae2:growth_acceleratable` tag stays out: a content mod adds files to its own namespaces and the
  convention tags only. AE2's own tag holds `#c:budding_blocks`, which names the budding entro.

Tests: `horizon-client` (the rows, sizes, cards and IO budget in `ae2x::extendedae`; recipe matching, the search
context, the executor's steps and energy, fixer recipes and output sides in `game::ae2x::extendedae::machines`;
threads, pages and the assembler's speeds in `devices`; growth and wear in `entro`) and the world tests
`extendedae-crystal`, `extendedae-buses` (tick-by-tick transfers with no card and four speed cards, the filter,
eight cards on the screen, the kit, the bus's recipe, a restart, the drops) and `extendedae-devices` (four
inscriptions at once, the pages, the stack size, a hopper feeding the one thread that takes its item,
auto-export, four charged crystals by hand, a provider's job in the Extended Molecular Assembler and its five
cards, a seed on fluix, clusters grown by an accelerator's random ticks and falling off, the fixer's
entro, a restart, the drops):

```sh
AE2_TEST_STARTUP_MS=600000 bun run ae2/scripts/test-world.ts extendedae-crystal extendedae-buses extendedae-devices
```

## Slice 3: the Assembler Matrix

| ExtendedAE | Port |
|---|---|
| `CalculatorAssemblerMatrix` (`MBCalculator`): from any block, the box its axes reach, full of the mod's matrix blocks with none against its faces outside (`verifyUnownedRegion`); its corners 2 to `assembler_matrix_max_size` apart on each axis (`checkMultiblockScale`; 6 by default, ATM10 ships no config: 3 to 7 blocks a side); frames on every edge and corner, walls or glass on the faces, function blocks inside (`isEdge`, `isInternal`); a crafter and a pattern core at least | `game/ae2x/extendedae/matrix.rs` (`formation`): a block is formed when the blocks joined to it fill such a box, which is where the mod's walk ends up; recalculated each tick (the mod: on load and on a neighbour's change). The cluster's corners, node group and speed cores in each block's config (`eae_matrix`, rebuilt as the mod's cluster, which no save keeps), its block state's `formed` |
| `TileAssemblerMatrixBase`: `GridFlags.MULTIBLOCK`, `REQUIRE_CHANNEL`, `IGridMultiblock`: one channel for the cluster; all sides connect while formed, none before; 0.5 AE/t, a function block 1 (`TileAssemblerMatrixFunction`) | the node (`extendedae::device`): faces by the block state's `formed`, one `multiblock` group (its lowest corner), the idle draw; the pattern core's through `ae2x_device` too, as a variant of AE2's provider |
| `updateSubType`: `FORMED`, `POWERED`; `BlockAssemblerMatrixFrame.SHAPE`, a column between frames on one axis | `extendedae_matrix_visual` (`ae2x_visual_props`), `extendedae_frame_shape` |
| `TileAssemblerMatrixPattern`: 36 patterns (`Filter`: `IMolecularAssemblerSupportedPattern`, crafting, smithing table and stonecutting), an `ICraftingProvider` and `PatternContainer` (`PATTERN_ACCESS_TERMINAL`, `getTerminalGroup`: its icon and name, "Only accepts crafting patterns") | a `MachineVariant` of AE2's Pattern Provider (36 slots): the grid's crafting service and the Pattern Access Terminal take it as AE2's provider; its filter in `pattern_inventory_accepts`, its group in `ae2x_pattern_group` |
| `pushPattern` (formed, active, its pattern) to `ClusterAssemblerMatrix.pushCraftingJob`: an available crafter core's first free thread; `isBusy` while none is | `extendedae_matrix_destination`, which the CPU's `recursive_destination` asks first (`ae2x_crafting_destination`); crafter cores in place order (the mod: its hash set's) |
| `TileAssemblerMatrixCrafter`: eight `CraftingMatrixThread`s (`MAX_THREAD`), `tick(cluster.getSpeedCore())`: 20, 26, 34, 40, 50 or 100 progress a tick at 1 to 5 AE a point for 0 to 5 speed cores (`addSpeedCore` stops at five); a thread's output and leftovers into its `OutputBuffer`, flushed into the grid; asleep while unformed | an `ExtMachine` of eleven slots and eight threads (`game/ae2_assembler_threads.rs`), each AE2's assembler, ticked while formed (`extendedae_crafter_tick`) at the cluster's speed (`devices::thread_speed`); the CPU takes a job's output from its thread as from AE2's assembler; what no job owns goes into the grid (`extendedae_crafter_flush`) |
| `ContainerAssemblerMatrix`, `GuiAssemblerMatrix`, `screens/assembler_matrix.json`: from any block of a formed, active matrix; all cores' patterns in rows of nine (a scroll bar, a search), "Running Jobs: %s" (`getBusyCrafterAmount`), Cancel (each crafter's `stop`: its threads' items into the grid), the Pattern Access Terminal toggle for the cluster (`broadcastConfig`); a pattern clicked in or out through the core's filter (`doAction`), shift-clicked into the first free slot (`transferStackToMenu`) | a 54-entry window (`MatrixMenu`, `lan_open_extendedae_matrix`): a core's 36 patterns a page (cores in `getLocateID` order), the next and previous core on the toolbar (38, 39), Cancel (36), the toggle (37), Running Jobs (data slot 1); the patterns' places (`EAE_MATRIX_PATTERN`) added to the style by the layout (`style_slots`), as the mod's screen code places them |
| the blocks; the frame, wall and glass by crafting, the crafter, pattern and speed cores in the Crystal Assembler (`assembler/assembler_matrix_*`); their loot tables | `content.json`; the glass draws its item model (`cube_all` of `glass/full`): its block model needs the mod's connected-glass loader |

AE2's crafting, for any assembler of several threads (`game/ae2_assembler_threads.rs`, `ae2_autocrafting.rs`):

| AE2 | Port |
|---|---|
| `ICraftingMachine.pushPattern` of a machine with threads (`TileExMolecularAssembler`, `ClusterAssemblerMatrix`) | a flight names its assembler's thread (`Session.flight_threads`, `AssemblerFlightOwner.thread`; thread 0, AE2's only one, written as before); a thread past the first keeps its grid, output, progress and job in the machine's config (`ae2_assembler_thread_<n>`), its items dropped with the machine (`Machine::physical_contents`); `ae2x::assembler_threads` counts an addon's |
| `CraftingCpuLogic.tickCraftingLogic`: `executeCrafting` again while its last pass pushed and operations are left; a pass offers a pattern to each provider once | the dispatch loop starts a new pass on the same terms, so one provider's machine takes as many jobs a tick as the CPU's co-processors allow (before, one a tick) |

Differences, known:

- The screen shows a pattern core's 36 patterns at a time, a page a core, with no search; the mod scrolls through
  all cores' rows.
- The matrix forms or breaks at the next tick, not at once. Its blocks expose no item handler of its patterns
  (`getPatternInv`, a capability on every block): a hopper cannot fill a pattern core.
- A crafter core has no output buffer: a job's output waits in its thread until the CPU takes it, and a core never
  refuses jobs for a full buffer (`MAX_BUFF`). No recipe cache (`CraftingMatrixThread`'s LRU): same results.
- Cancel, as in the mod, leaves the CPU waiting for the cancelled jobs' outputs until its job is cancelled.
- A cluster takes no name from its blocks (`updateName`).

Tests: `horizon-client` (formation, its size, a wrong block, a gap, a stray neighbour and the marker in
`game::ae2x::extendedae::matrix`; threads and their drops in `game::ae2_assembler_threads`; a flight's thread in
`ae2_autocrafting`; the config's size in `ae2x::extendedae`) and the world test `extendedae-matrix` (a 7-long matrix
forms on one of an ad-hoc grid's eight channels, an 8-long one does not; the screen takes a crafting pattern, not
stone, and sets the cluster's Pattern Access Terminal setting; tick by tick on a held server, a CPU with seven
co-processors gives a crafter core eight jobs at once, 64 crafts in 28 ticks; five speed cores take a craft from 7
ticks to 3; a wall taken away breaks the matrix and put back forms it; the Extended Molecular Assembler behind a
provider runs eight jobs at once):

```sh
AE2_TEST_STARTUP_MS=600000 bun run ae2/scripts/test-world.ts extendedae-matrix
```

## Slice 4: Wireless Connector, Wireless Hub and the Wireless Setup Kit

| ExtendedAE | Port |
|---|---|
| `TileWirelessConnector`: `DENSE_CAPACITY` (32 channels), all sides, no channel of its own; four cards (`UpgradeInventories.forMachine(.., 4)`): AE2's Energy Card, and MEGA's Greater Energy Card (`MEGACommonLoad`), four each | an `ExtMachine` of no slots and four cards (`ae2x/extendedae.rs`, `OWN_MACHINES`); its node (`game/ae2x/extendedae/wireless.rs`, `device`): every face, 32 channels, no channel |
| `WirelessConnect.updateStatus`: the two ends of a frequency register under `-f` and `f` in their level (`Locatables`), a third finds both taken; linked when both are active, in the same level and no further apart than `wireless_connector_max_range` (`Math.sqrt(distSqr)`, 1,000); `GridHelper.createConnection` between their nodes, which fails between neighbours already joined (the link still counts) | `ae2x::extendedae::wireless_links` (the first two ends of a frequency by when they registered, the range, not the same block); `tick_extendedae_wireless` keeps each level's links, `ae2x_connect` (from `ae2_network`) makes them `Network.links` edges, as the quantum bridge's; each link made or broken between two ready nodes is a `MeLink` mutation of the ordered event journal, so the ordered services see the same grid |
| `updatePowerUsage`: linked, `max(1, d ln d (1 - 0.1 x energy cards - 0.2 x greater cards))` with `d` at least `e`, times `wireless_connector_power_multiplier`; unlinked, the multiplier | `wireless_link_power`, `wireless_power`; set each tick into the block's config (`eae_wireless_power`, which `reset_unsaved` drops, as the mod does not save `powerUse`) for its node's idle draw |
| `TileWirelessHub`: eight links (`MAX_PORT`), their draws added; `freq0` to `freq7`, or a connector's `freq` as its first (`loadTag`); `allocatePort`: the first link not up; `killPort` | the same rows (`wireless_frequencies`, `set_wireless_frequency`; a hub made from a connector writes all eight); its screen's Disconnect buttons (window entries 10 to 17) |
| `BlockWirelessConnector.check`, `BlockWirelessHub.check` (`useItemOn`, before the menu, not sneaking): the kit's first use binds it (`WIRELESS_LOCATOR` `{a: frequency, b: {dimension, pos}}`, a new random long, never 0 nor used); the second gives the frequency to both, or fails: no place or no wireless block there (`MISSING`), the same block (`SELF_REFERENCE`), another level (`CROSS_DIMENSION`), past the range (`OUT_OF_RANGE`), no free link on a hub (`OUT_OF_PORT`, the hub's own first) | `extendedae_wireless_tool_use`, through `ae2x_use_on` |
| `ItemWirelessConnectTool.use`, sneaking: the kit forgets its block | `extendedae_wireless_tool_clear`, through `lan_ae2x_use_item` |
| `ItemWirelessConnectorUpgrade`: a connector into a hub with all it holds | a `Kit` of the mod's own blocks (`upgrade.rs`) |
| `updateBlockStateFromBlockEntity`: `connected` while a link is up | `extendedae_wireless_visual` (`ae2x_visual_props`) |
| `ContainerWirelessConnector`, `ContainerWirelessHub`: `UpgradeableMenu`s, `screens/wireless_connector.json`, `wireless_hub.json` | layouts in `screens.rs`: the four cards; the hub's Disconnect buttons |
| the blocks, the kit and the hub's kit; their Crystal Assembler and crafting recipes | `content.json` |

Differences, known:

- Each level's links are looked at every tick (the mod: when a block's frequency, node or chunk changes). The links
  are the same, but for a third end of a frequency, which the mod leaves unlinked until its own block changes: here
  it links as soon as one of the pair goes.
- The kit's far end must be loaded; the mod's `getBlockEntity` loads its chunk.
- No colours (`recolourBlock`): every block is fluix (`color` 16). No Jade line.
- The screens have the cards and the hub's Disconnect buttons, but not the power, channel and status lines, nor the
  hub's remote blocks and their highlight buttons.
- The kit's messages go to the server's chat, not the player's action bar.

Tests: `horizon-client` (rows, cards, pairing, range, the power formula, the records' names and the config in
`ae2x::extendedae`; the locator, the node and the links in `game::ae2x::extendedae::wireless`; the hub's kit in
`upgrade`) and the world test `extendedae-wireless`: two connectors 900 blocks apart paired by the kit carry a
controller's channels and power to five drives as a cable would, each end draws 900 ln 900 AE/t, the kit refuses
1,001 blocks and forgets its block when sneak-used, a hub takes eight links on its eight ports and refuses a ninth,
the hub's kit keeps a link, the links come back after a restart, a broken connector drops itself:

```sh
AE2_TEST_STARTUP_MS=600000 bun run ae2/scripts/test-world.ts extendedae-wireless
```

## Slice 6: Infinity and Void Cells

| ExtendedAE | Port |
|---|---|
| `ItemInfinityCell`, `InfinityCellInventory`: one key (`getRecord`) it lists as `Integer.MAX_VALUE * getAmountPerUnit()` (2,147,483,647 items, or that many buckets), gives without end and takes in, voided; nothing else goes in; `NOT_EMPTY`; `getIdleDrain` = `item.infinity_cell_energy_cost`, 8 AE/t | `horizon_client::ae2x::fixed_cell`: to the storage code a creative cell of that key (`creative_cell_keys_checked`), of its own amount (`creative_cell_amount`), drain and drive model; no Cell Workbench takes it (no `ICellWorkbenchItem`) |
| `infinity_water_cell`, `infinity_cobblestone_cell` and their recipes (16k component, as in ATM10) | items of `content.json` (`ae2x::extendedae::infinity_cell`) |
| ATM10's KubeJS `custom_infinity_cell`s (`InfinityCellBuilder`: `itemType`, `fluidType`, `cellModel`; 37 in 8.4: lava, dirt, moss, the stones, sands, concretes' dyes...), their recipes with MEGA's 256M component | their records and models read by `prepare-addon.ts` from the pack's `kubejs/startup_scripts/ExtendedAE/InfinityCells.js` (`pack_kubejs_items`) into `data/extendedae/ae2x/infinity_cells.json`, which the game installs with the content (`install_pack_cells`); the items and recipes are the pack's carry's |
| `getName`: "ME Infinity %s Cell" of the record's name; `infinity.tooltip` | `item_tooltip::name_line`, `ae2x_item_hover_lines` |
| `ItemVoidCell`, `VoidCellInventory`: an `ICellWorkbenchItem` of 63 filters and two cards (fuzzy, inverter); with no partition it takes nothing; what the partition lets in is taken whole and, by its `VOID_MODE`, kept as void energy that becomes matter balls (256 a ball) or singularities (256,000) as AE2's Condenser makes them, or trashed; those come back out; `isPreferredStorageFor` its partition; 1 AE/t | a cell of the mod's own handler (`HandlerCellDef`, `game/ae2x/extendedae/cells.rs`, `VoidCell`), its outputs in `ae2:storage_cell_inv`, its energy in `extendedae:void_energy` |
| `ItemVoidCell.use`, `ContainerVoidCell`, `GuiVoidCell`, `screens/void_cell.json`: the three modes' buttons, "Mode: %s" | an item menu (`lan_open_void_cell`, a 9 x 3 window with no slots of its own; the player inventory shows under it), the mode as data slot 0; a button press sets the held cell's `extendedae:void_mode` (`lan_void_cell_button`, through `lan_ae2x_button`) |

Differences, known:

- It voids items only: the port's handler cells hold item keys; a fluid in its partition counts as a partition
  but lets no fluid in.
- No tooltip image (`getTooltipImage`) on either cell; an infinity cell has no tint (`BasicStorageCell::getColor`).

Tests: `horizon-client` (the records, the pack's rows, the names, the void cell's handler row and the config's
cost in `ae2x::extendedae`; partition, inverter, modes and output in `game::ae2x::extendedae::cells`) and the
world test `extendedae-cells` (endless cobblestone out of a drive, cobblestone in and voided, nothing else in,
8 AE/t each, endless water, a KubeJS-built dirt cell of a stand-in pack mod, the void cell preferred for its
partition over a 1k cell, its inverter, an unpartitioned one, its screen setting matter ball mode and the
matter balls it makes, a restart):

```sh
AE2_TEST_STARTUP_MS=600000 bun run ae2/scripts/test-world.ts extendedae-cells
```

## Slice 7, first part: the Pattern and Config Modifiers, ME Packing Tape, the Extended Pattern Access Terminal

| ExtendedAE | Port |
|---|---|
| `HostPatternModifier`: 27 patterns, a target, four blank patterns, a clone and the two items of a replacement in `STACK_TAG` (`extendedae:generic_nbt`, AE2's `writeToNBT` lists) | `game/ae2x/extendedae/tools.rs`, `Modifier` |
| `ContainerPatternModifier.modify(scale, div)`: each processing pattern's amounts times or divided by the factor, when all divide by it or stay within 999,999 units (`getAmountPerUnit`); the factors of `item.pattern_modifier_multipliers` (`getPatternModifierNumber`, the default past the list's end) | `Modifier::modify`, `Config::modifier_multiplier` |
| `clear` (encoded patterns to blank ones), `clonePattern` (the target into the clone slot for a blank pattern, or over a pattern there), `replace` (in processing patterns the replaced item becomes the other or goes; a crafting pattern only when its recipe still makes it), `changePatternMode` (crafting patterns' substitutions and fluid substitutions) | the same `Modifier` methods; a crafting pattern's recipe checked by `assemble_named` |
| `GuiPatternModifier`: four pages (`CUpdatePage`), the patterns (`PROVIDER_PATTERN`), the target, the clone (`OutputSlot`), the blanks, two `SingleFakeSlot`s; `pattern_editor_1/2/3.png` | a window a page (27 entries for multiply and property, 54 for replace and clone), layouts `extendedae:pattern_modifier`, `_replace`, `_mode`, `_clone` (`screens.rs`) with the buttons where `init` puts them; the background from `style_slots`; clicks checked against each slot's filter (`lan_extendedae_tool_click`), buttons as window buttons (`lan_extendedae_tool_button`) |
| `ItemPatternModifier.onItemUseFirst`, not sneaking: its patterns into the pattern provider used on (block or part), each that fits whole | `extendedae_tool_use_on`, through `ae2x_use_on` |
| `ItemConfigModifier`: `MODIFIER_CONFIG_SETTINGS` `{mode, data}` (ADD, SUB, MUL, DIV, MAX, MIN, SET, RMV; times 1 by default); `onItemUseFirst` on an `IConfigInvHost`: each config slot's amount through `ConfigSettings.modify` between 1 (0 for types) and `getMaxAmount` (an item's stack size up to 99, a fluid four buckets), -1 removes it | `Settings`; on AE2's and the Extended Interface, block or part (`extendedae_config_apply`) |
| `ContainerConfigModifier`, `GuiConfigModifier`: the mode's button, the number's text field | a layout of `config_modifier.json`: the mode's button (data slot 0), the number (data slot 1) |
| `ItemMEPackingTape.onItemUseFirst` (64 uses): a block of `item.me_packing_tape_whitelist` with all it holds (`TAPE_TILE_DATA`: its type, block, state, `saveWithFullMetadata`), or the part hit (`TAPE_PART_DATA`: its item, `writeToNBT`), into a package dropped where it was; nothing else drops | `game/ae2x/extendedae/tape.rs`: the port's saved block entity (`save::encode_block_entity`, its name), the part's record on the cable |
| `ItemPackedDevice.useOn`: the block placed as its item would be and loaded, or the part placed (`PartPlacement`) and loaded | `extendedae_package_use`: the block where the use lands, the part through `ae2_try_attach_part` with the packed record over the new one (its place and node kept) |
| `PartExPatternAccessTerminal`: AE2's Pattern Access Terminal; `ContainerExPatternTerminal.quickMovePattern`: a pattern the Molecular Assembler takes only to a group shown by an assembler (AE2's, the Extended Molecular Assembler, the matrix's pattern core), any other only to the rest, across groups | a part variant of `ae2:pattern_access_terminal` (`MACHINES`); AE2's menu (`ae2_pattern_access.rs` goes by `base_id`), its quick move by `pattern_terminal_extended` and `pattern_quick_move_target`; its screen model over AE2's display base (`display_part_screen`) |
| `ItemPatternAccessTerminalUpgrade` on AE2's terminal part | a `Kit` (`upgrade.rs`) |
| `ItemWirelessExPAT`: AE2WTLib's terminal `ex_pattern_access` (`AddTerminalEvent`, hotkey `wireless_pattern_access_terminal`, two cards, `ae2wtlib:has_ex_pattern_access_terminal`), the Pattern Access menu over its access point's grid | `register_ae2wt`, a `TerminalKind::PatternAccess` row (after ME Requester's) in `ae2x/defs.rs`; the same quick move |
| the items and their recipes (the tape's, the modifiers', the terminal's, its kit's and its wireless form's, AE2WTLib's `ae2wtlib:combine` into the universal terminal) | `content.json`; the tape's durability by the new `components` of `prepare-addon.ts` |

Differences, known:

- The modifiers' buttons are AE2's icon buttons with the mod's tooltips: the factors are not written on them. A page
  change opens the page's window anew.
- The Config Modifier's number is set by four step buttons (+1, -1, +10, -10), not typed; the screen shows it up
  to 32,767. It acts on interfaces only: the other `IConfigInvHost`s (storage, import and export buses, level
  emitters, formation planes, the Cell Workbench) take nothing from it yet.
- An amount's unit is an item's 1 or a fluid's bucket; other key types come with AE2's key-type ids.
- A package holds the port's records, not the mod's NBT: one from a Java world does not unpack here. The whitelist's
  Oversize Interfaces come with that interface.
- The Extended Pattern Access Terminal's screen is AE2's: no search by inputs or outputs, no highlight in the world.

Tests: `horizon-client` (scaling, its limits, the record, replace, clear and clone, the settings' modes and the
pages in `game::ae2x::extendedae::tools`; the config lists in `ae2x::extendedae`) and the world test
`extendedae-tools`: the Pattern Modifier takes two processing patterns by shift-click and not stone, x2, ÷2 (and
not ÷2 again), x2, then fills an Extended Pattern Provider; the Config Modifier's +1 makes it x2 and an interface
stocks 6 diamonds and 64 stone; the tape packs the provider with its patterns in one use and the package puts it
back; the kit makes AE2's terminal the extended one, which, a placed one and the wireless form at an access point
list the provider's patterns:

```sh
AE2_TEST_STARTUP_MS=600000 bun run ae2/scripts/test-world.ts extendedae-tools
```

## Later slices (`.build/ae2-addons-plan.md` §6.1)

5. Special buses, emitters and planes.
7. Oversize (x16) and Vacuum interfaces: after AE2's key types become ids (NOTES).
