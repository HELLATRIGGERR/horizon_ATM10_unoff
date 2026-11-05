# ExtendedAE 1.21-2.2.42

[ExtendedAE](https://github.com/GlodBlock/ExtendedAE) by GlodBlock, models and textures by Sea_Kerman and
Ridanisaurus, as All the Mods 10 8.4 ships it. Code LGPL-3.0 (`LICENSE.txt`; 2.2.42 has no tag, its source is
commit `1f86f927` on branch `1.21.1-neoforge`), so the port's derived files carry the same terms; the port's code is written from the mod's behaviour, not copied.
The licence of the assets is not stated; they are generated locally from the player's jar and never
committed. Provenance in [SOURCE_MANIFEST.json](SOURCE_MANIFEST.json), the declared content in
[content.json](content.json).

Feature `ae2-extendedae`; content `content/extendedae` from
`ATM10_DIR=... bun run ae2/scripts/prepare-addon.ts extendedae`. ATM10's `config/extendedae-common.toml` keeps
the mod's defaults (ATM10 8.4 ships none: `extended_io_bus_multiplier` 8, `infinity_cell_energy_cost` 8). Of
ATM10's KubeJS, only its infinity cells (slice 6) touch the mod; its recipes for them are the pack's own, in
the full pack's carry.

From 2.2.41 (ATM10 8.3) to 2.2.42 the mod changed two classes and nothing else: `CraftingMatrixThread` resets the
Assembler Matrix's progress on its cached-pattern path, and `ContainerRenamer` saves the block or part it renamed.
Neither is in this slice; the jar's assets and data are the same.

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
| `TileExMolecularAssembler`: eight `CraftingThread`s of ten slots, a job each from pattern providers (`pushPattern` to the first free thread); 20, 26, 34, 40, 50 or 100 progress a tick at 1 to 5 AE a point for 0 to 5 speed cards (AE2's is half); nothing goes in by automation, the output comes out | a `MachineVariant` of AE2's Molecular Assembler (11 slots, five cards), one thread: the port's crafting CPU sends a job to an assembler by its place, one at a time; the mod's speeds (`assembler_speed`, read by `ae2_tick_assembler`); its item handler takes nothing in |
| `ContainerExMolecularAssembler`, `screens/ex_molecular_assembler.json` | its first page: the grid (EX_1), the output, five cards, the progress |
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
- The Extended Molecular Assembler runs one job at a time, not eight: the port's crafting CPU sends a job to an
  assembler by its place (`recursive_destination`, its flights by `GridAddress`), and so does the Assembler
  Matrix's. Eight threads need flights that name a thread.
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

## Later slices (`.build/ae2-addons-plan.md` §6.1)

3. Assembler Matrix (max size 6 in ATM10): needs the crafting CPU to send jobs to a crafter thread, not to an
   assembler's place (it would also give the Extended Molecular Assembler its eight threads).
4. Wireless Connector and Hub (range 1,000).
5. Special buses, emitters and planes.
7. Oversize (x16) and Vacuum interfaces, modifiers, ME Packing Tape, Extended Pattern Access Terminal.
