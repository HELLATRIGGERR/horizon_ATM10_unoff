# Advanced AE 1.6.12-1.21.1

[Advanced AE](https://github.com/pedroksl/AdvancedAE) by pedroksl, as All the Mods 10 ships it. Code
LGPL-3.0 (`LICENSE.md`, here [ae2/LICENSES/AdvancedAE-LGPL-3.0.txt](../../LICENSES/AdvancedAE-LGPL-3.0.txt)),
so the port's derived files carry the same terms; the port is written from the mod's behaviour and
source at commit `c322dad81e6f2ad94cb1fb861e73a7ba68e5907a` (branch `forge/1.21.1`, which sets
`mod_version=1.6.12-1.21.1`). The tag `1.6.12-1.21.1-neoforge` points at `779c9c1`, a later port to
Minecraft 26.1.2 (`mod_version=26.1.6`), so the port does not follow the tag. Models by Jm³ and
Sea_Kerman; the assets (licence not stated) are generated locally from the player's jar and never
committed. Provenance in [SOURCE_MANIFEST.json](SOURCE_MANIFEST.json), the declared content in
[content.json](content.json).

The jar bundles AE2AddonLib 1.0.3, whose `mods.toml` says LGPL-3.0 and whose repository says GPL-3.0.
Nothing is derived from it: of it the port knows only the saved form of `NullableDirection` (the
direction's `Direction.name()`, or `null`), which Advanced AE's own `EncodedAdvProcessingPattern` writes.
The bundled ae2wtlib_api (MIT) is not needed by this slice.

Feature `ae2-advancedae`; content `content/advanced_ae` from
`ATM10_DIR=... bun run ae2/scripts/prepare-addon.ts advancedae`; `[mods.advanced_ae]` in
`pack/pack.toml`. ATM10's `config/advanced_ae-common.toml` holds only the Quantum Computer's settings,
which slice 3 reads; its KubeJS changes no recipe of these slices (it changes the armor's and some cards').

## What works (slice 1)

| Advanced AE | Port |
|---|---|
| `advanced_ae:adv_processing_pattern` (`PatternDetailsHelper.encodedPatternItemBuilder(AdvProcessingPattern::new)`, which stacks to 1), its component `advanced_ae:encoded_adv_processing_pattern`: AE2's `sparseInputs` and `sparseOutputs`, and `directionMap`, a `NullableDirection` per input | a processing pattern item of its own component in the definitions registry (`Defs::add_processing_pattern`), so every processing pattern decoder, the provider and Pattern Access slots, the Memory Card's blanks and the crafting plans read it as AE2's; the component in the command codec (`ae2_components`) and on the wire |
| `AdvProcessingPattern.getDirectionMap`: one direction per input key, a key's last | `game/ae2x/advancedae`: `direction_map`; an entry that is no direction reads as none (the codec's fault tolerance) |
| `advanced_ae:small_adv_pattern_provider` (`SmallAdvPatternProviderBlock`, `AEBaseBlock.metalProps`, `push_direction`, `connection_state`), `AdvPatternProviderLogic(node, host, 9)` | a machine variant of AE2's Pattern Provider (nine patterns, nine return slots, no cards): blocking mode, the crafting lock, the Pattern Access Terminal toggle, priority, `push_direction` and the wrench as AE2's |
| `AdvPatternProviderLogic.pushPattern`: an Advanced pattern is pushed with `pushInputsDirectionally` first, each input into the face of the target its direction names (none: the face toward the provider), and when one input finds no room there, as AE2's, all to one face | the provider push (`ae2_autocrafting`): the target is chosen and the inputs sent by the faces of `directional_faces`; AE2's Pattern Provider pushes the same pattern as any processing pattern |
| `sendStacksOut` with the push's `directionMap`: what a face did not take waits and goes to its own face | the send list keeps the faces (`advanced_ae_send_faces`) beside AE2's send face until it is empty |
| `SmallAdvPatternProviderScreen` with `screens/small_adv_pattern_provider.json` | `game/ae2x/advancedae/screens.rs`: AE2's provider layout over the mod's style |
| `AdvPatternEncoderMenu`: an AE2 processing pattern in the input slot gives an Advanced one with no direction (`decodeProcessingPattern`), an Advanced one is copied, `update` sets a key's direction | `encoder_output`, `with_direction`; `/ae2 advae show|encode|direction <slot> ...` applies them to a pattern in the player's inventory, for tests |
| `advanced_ae:adv_pattern_encoder` (`AdvPatternEncoderItem`, stacks to 1) | the item, its model and recipe `advpartenc` |
| recipes `smalladvpatpro` and `advpartenc`, loot table, pickaxe tag | from the jar; `smalladvpatpro2` and `smalladvpatpropart` name the provider part, a later slice's |

## What works (slice 1b)

| Advanced AE | Port |
|---|---|
| `AdvPatternEncoderItem.use`: the encoder's menu (`AdvPatternEncoderHost`, `AdvPatternEncoderMenu`): an input slot of encoded patterns and an output slot over the player's inventory, the encoder's own slot locked; a pattern put in decodes into the output (`decodeInputPattern`, also as the menu opens), taking the output takes the input (`onChangeInventory`); the two slots stay in the item | `game/ae2x/advancedae/encoder.rs`: a 9 x 3 window of the two slots; the slots in the item as AE2's `AppEngInternalInventory` list `inOutInventory`, in its `minecraft:custom_data` (the mod keeps them in AE2AddonLib's NBT component, which the port does not read) |
| `AdvPatternEncoderScreen` with `screens/adv_pattern_encoder.json`: three rows of the inputs from the scroll, the row's slot and input, seven `DirectionInputButton`s (any, north, east, south, west, up, down; the row's face highlighted; a tooltip each), the small scrollbar (the wheel and a press past the handle move two rows); `AdvPatternEncoderChangeDirectionPacket` | `encoder_screen.rs` over the window, the mod's background and button textures; a button is a menu button (`BUTTONS + 7 * row + column`); the list is read from the output slot, as the menu's `AdvPatternEncoderPacket` sends it |
| `PATTERN_ENCODER_HOTKEY` (`key.advanced_ae.pattern_encoder_action`, unbound, category `key.advanced_ae.category`): the first encoder in the inventory opens; on the open screen the hotkey closes it (`isCloseHotkey`) | the key mapping in Controls (`horizon-client-input`, feature `ae2-advancedae`); the addons' hotkey id `advanced_ae:pattern_encoder_action`, answered on the server; the close hotkey goes no further |
| `AAESettings.FILTERED_IMPORT` and its `AAESettingToggleButton` (icons `FILTERED_IMPORT_ON`/`OFF` of the mod's `states.png`); `AdvPatternProviderReturnInventory`: while on, the return slots take only what the provider's patterns make (`getOutputCache`, `getTrackedCrafts`) | the provider's fourth toolbar button (menu entry 21, config `filtered_import`, `NO`/`YES`); the filter on the return inventory's inserts (hoppers, machines) and the menu's slots (a click, a swap, a shift-click); AE2's icon sheet takes an addon's (`icon::sheet`) |
| `MixinEncodedPatternItem`: an Advanced pattern's tooltip, AE2's lines (`Produces:`, `with:`, `and`) with each input's direction letter, `(U)` | `tooltip` in `game/ae2x/advancedae`, the item's hover lines; `/ae2 advae show` gives it too |
| `MixinPatternEncodingTermMenu`: encoding a processing pattern while the encoded slot holds an Advanced one keeps it Advanced, each input key it had keeping its direction | `reencoded`, in AE2's Pattern Encoding Terminal and AE2WTLib's wireless one |

## What works (slice 2)

| Advanced AE | Port |
|---|---|
| `advanced_ae:reaction_chamber` (`ReactionChamberBlock`: `metalProps().noOcclusion()`, `WORKING`, `horizontalFacing()`), `ReactionChamberEntity`: nine inputs and an output (`AppEngInternalInventory`, 64 a slot), `UpgradeInventories.forMachine(.., 4)` with four speed cards, `setInternalMaxPower(500000)`, a node of no channel and no idle draw on every side, `COVERED` | a machine of its own (`ExtMachine`, `ae2x/advancedae.rs`), placed facing the player; its buffer takes Forge Energy on every side; `working` from its config; the loot table and pickaxe tag from the jar |
| `ReactionChamberRecipe` (`advanced_ae:reaction`): `output` a `GenericStack` (an item or a fluid), `input_items`, `input_fluid`, `input_energy`; `ReactionChamberRecipes.findRecipe` (the inputs summed per item and components, each ingredient met by one of them, the input tank's fluid), `isValidIngredient` | `game/ae2x/advancedae/chamber.rs`: the type's recipes from the content (15 of 19: those of items the port has), the search, an input slot of the menu taking only a recipe's ingredient |
| `tickingRequest` (`TickingRequest(1, 20, ..)`): `cachedTask` until a change leaves no recipe (`dirty`); `speedFactor` 2, 3, 5, 10, 50 by cards over 200 steps; each step `floor(input_energy / ceil(200 / speed))` AE from the own buffer, else from the one of it and the grid that gives more; with too little, part steps and `showWarning`; the output in, then each input taken from the slots in order and the tank (`consume`); `processingTime`, `working`, `showWarning` and the task not saved | the same with AE2's tick tracker; the search and `dirty` in the game's state by block entity |
| `fluidInv` (`CustomGenericInv`, 16,000 mB each): slot 1 the input (in only), slot 0 the output (out only); `GENERIC_INTERNAL_INV`, so AE2's fluid handler: `fill` into the input, `drain` from the output | the tanks in the config (`tank_in`, `tank_out`); the fluid handler through the Sky Stone Tank's calls (`ae2x_tank_fill`, `ae2x_tank_drain`, `ae2x_tank_contents`): a pattern provider's push, an export bus, a P2P tunnel, an import bus |
| `ReactionChamberBlock.useItemOn` with a bucket: an empty one takes a whole bucket of the output, a full one empties into the input when all of it fits; else the menu | `advancedae_bucket_use` before AE2's item uses |
| `pushOutResult`: with `AUTO_EXPORT` on, per side of `allowedOutputs` (`RelativeSide`, turned by the facing) up to 64 of the output item and the whole output tank into what is there (`createExternalStorageStrategies`); the first side that took any ends it | the same through the port's item and fluid insertions (an export bus's) |
| `ReactionChamberScreen` with `screens/reaction_chamber.json`: the progress bar, the `AUTO_EXPORT` toggle, the `DIRECTIONAL_OUTPUT` button (shown with it on) opening AE2AddonLib's `OutputDirectionMenu`, the two flush buttons (`clearFluid`, `clearFluidOut`), the power alert (`AlertWidget`) and the two `FluidTankSlot`s; `ReactionChamberMenu` | `screens.rs`: the same style; six toolbar toggles for the six sides (shown with `AUTO_EXPORT` on, AE2's side names, the mod's `VALID_INPUT`/`INVALID_INPUT` icons) in place of AE2AddonLib's menu, the flush buttons with the mod's `CLEAR` icon, the alert while data slot 5 says so; each tank an entry at its place showing the fluid's bucket named with its amount, where a bucket on the cursor empties into the input or fills from the output |
| `saveAdditional`: the tank, `outputs`, the cards, `AUTO_EXPORT` | the machine's config: `tank_in`, `tank_out`, `output_front` .. `output_right`, `auto_export`; the port's own save form, so a Java world's chamber keeps only its slots |

## What works (slice 3)

| Advanced AE | Port |
|---|---|
| `AAECraftingUnitType`: `quantum_unit`, `quantum_core` (256 MB, the accelerator's threads), `quantum_storage_128`/`_256`, `data_entangler` (`STORAGE_MULTIPLIER`), `quantum_accelerator`, `quantum_multi_threader`, `quantum_structure` (`glassProps`); `AAEAbstractCraftingUnitBlock`'s `formed`, `powered`, `multiblocked`, `light_level` | machines of their own (`ExtMachine`, `ae2x/advancedae.rs`: `QuantumPart`); the block states the mod's; loot tables and pickaxe tag from the jar |
| `AAEConfig`'s `quantum computer` section (`quantumComputerMaxSize`, `...AcceleratorThreads`, `...MaxMultiThreaders`, `...maxDataEntanglers`, the two multiplications) | `QuantumConfig` from the game's `config/advanced_ae-common.toml`, the mod's defaults and ranges (All the Mods 10: 12 threads, factors of 6) |
| `AdvCraftingCPUCalculator`: `checkMultiblockScale`, `verifyInternalStructure` (structure on the box's sides only, one core inside, the entanglers and multi-threaders inside up to their limits, the rest inside, some storage; a core alone forms) | `game/ae2x/advancedae/quantum.rs`: `verify` on each group of joined blocks that fills its box (the matrix's reading of `MBCalculator`), each tick; the cluster in each block's config (`aae_quantum`) |
| `AdvCraftingCPUCluster.addBlockEntity`: storage, its multiplier, threads, their multiplier; `getCoProcessors`, `recalculateRemainingStorage` | the same sums; the bytes left are the total less the jobs' |
| `AdvCraftingBlockEntity`'s node: `MULTIBLOCK`, `REQUIRE_CHANNEL`, `getGridConnectableSides` (formed: the core up and down, the rest every side; unformed none); `updateSubType` | one node group, one channel; the block states each grid tick |
| `AdvCraftingCPUCluster.submitJob`: a new `AdvCraftingCPU` of `plan.bytes()` while the bytes left hold it, many at once, each AE2's crafting logic (`AdvCraftingCPULogic`); `getActiveCPUs`, `getRemainingCapacityCPU`, `cancelJob` | each job is AE2's job session on one block of the computer (the core first), of the plan's bytes and the computer's co-processors; the free block is a CPU of the bytes left |
| `MixinCraftingService`: the computers' CPUs in `getCpus`, ticked with AE2's, fed by `insertIntoCpus` after AE2's; `onSubmitJob`: a suitable computer (online, room enough, the selection mode; the preferred, then the most co-processors and least storage) before AE2's CPUs | the addons' CPU hooks (`ae2x_cpu_candidates`, `_capacity`, `_job_owners`) in AE2's CPU list, job start, job tick and receipt |
| `breakCluster` on `onRemove`: the jobs cancelled, their items out | AE2's `breakCluster` (`ae2_break_crafting_members`): the jobs cancelled, their items back to the grid, the rest dropped; also when the box changes or comes apart |
| `writeToNBT`/`readFromNBT`: the jobs (`cpuList`) and `CPU_SELECTION_MODE` on the core | each job saved in its block's config as AE2's; the mode in the core's config |
| `QuantumComputerScreen`/`QuantumComputerMenu` (`quantum_computer.json` = `crafting_status.json`), opened on a formed, active block: the jobs and the free CPU (`AdvCpuSelectionList`), the selected job, `CPU_SELECTION_MODE` in the toolbar | AE2's crafting status screen over the computer's CPUs, the mode button in place of back |
| recipes `quantumstructure`, `eaequantumunit`, `eaequantumaccel`, `quantumstorage128`/`256`, `quantumcore`, `eaequantumdataentangler`, `eaequantummultithreader` (the `not extendedae` forms stay for the game to turn off); the items they need: `shattered_singularity`, `quantum_storage_component`, `quantum_processor`, `printed_quantum_processor`, `quantum_processor_press`, `quantum_alloy` | from the jar; the Reaction Chamber's `shatteredsingularity` now (16 of its 19 recipes) |

## What works (slice 4)

| Advanced AE | Port |
|---|---|
| `advanced_ae:adv_pattern_provider` ("Advanced Extended Pattern Provider": `AdvPatternProviderBlock`, as the small one's block), `AdvPatternProviderEntity` (`AdvPatternProviderLogic(.., 36)`) | a machine variant of AE2's Pattern Provider of 36 patterns, nine return slots, no cards: the small one's logic (the directional push, Filtered Import) |
| `AdvPatternProviderPart` (36) and `SmallAdvPatternProviderPart` (9), items `adv_pattern_provider_part` and `small_adv_pattern_provider_part`: `REQUIRE_CHANNEL`, AE2's provider part pushing to the block it faces; `openMenu` by the size; `MODELS_*` over `part/adv_pattern_provider_part` (the small part keeps its parent's models) | part variants of AE2's provider part (`ae2:cable_pattern_provider`), each opening its block's screen; the directional push into the face of the block in front named by the pattern; both draw `part/adv_pattern_provider_part` |
| `AdvPatternProviderScreen` with `screens/adv_pattern_provider.json` (36 slots `BREAK_AFTER_9COLS`, `guis/adv_pattern_provider.png`) | `screens.rs`: the window of a bigger provider (`MenuShape`): settings at 45, Filtered Import 48, priority 53; the part's Filtered Import press through the addons' part menu hook |
| `AdvPatternProviderUpgradeItem.useOn` (by the exact class): AE2's `PatternProviderBlockEntity` becomes the small provider, ExtendedAE's `TileExPatternProvider` the large one (`getStateForPlacement`, then the old state's properties), the block entity replaced with its data; on a cable the part under the cursor (`PatternProviderPart` to the small part, `PartExPatternProvider` to the large part: `replacePart`, `readFromNBT`); `shrink(1)` | the addons' kits (`game/ae2x/kits.rs`, out of ExtendedAE's `ItemUpgrade`, which acts the same): used sneaking, the device is replaced in place with its slots, settings and name, the part record under the new id; the cards the new device has slots for (`readFromNBT`; Applied Flux's slot on AE2's and ExtendedAE's providers has none on Advanced AE's); a creative player keeps the item |
| `AdvPatternProviderCapacityUpgradeItem.useOn`: `SmallAdvPatternProviderEntity` and `SmallAdvPatternProviderPart` to the large ones | the same kit: the nine patterns keep their slots of 36 |
| recipes `advpatpro2`, `advpatpropart`, `smalladvpatpro2`, `smalladvpatpropart` (block and part forms), `eaeadvpatpro` (ExtendedAE's provider into the large one), `smallappupgrade`, `largeappupgrade` and `eaelargeappupgrade` (each kept or turned off by its `mod_loaded` condition); the large block's loot table and pickaxe tag | from the jar (45 recipes now) |

## What works (slice 5)

| Advanced AE | Port |
|---|---|
| `advanced_ae:stock_export_bus_part` (`StockExportBusPart`, an `ExportBusPart` of 6 cards), `import_export_bus_part` (`ImportExportBusPart`, AE2's 5), `advanced_io_bus_part` (`AdvancedIOBusPart`, 8, `getOperationsPerTick` times 8); `initUpgrades`: speed 4, capacity 5, redstone, crafting and fuzzy cards (no inverter card); `MODELS_*` over each `MODEL_BASE` | part variants of AE2's Export Bus (`ae2:export_bus`), so the cable, the cards, the tick rates, redstone, scheduling, crafting and drops are AE2's; each draws its own base model |
| `StockExportBusPart.getConfig` (`configStacks(63)`, saved as `extraConfig`): a key and an amount a filter; a stack put in keeps its count (`ConfigMenuInventory`) | the filters are the bus's keys, the amounts beside them in the part's record (`aae_stock`); set from the held stack (a fluid: a bucket), and by `/ae2 advae stock` |
| `StockExportBusPart.doBusWork`: each filter's key up to its amount less `getCurrentStock` (what the target's item and fluid handlers hold of it), within the operations; the crafting card's request when nothing went (`attemptCrafting`), craft-only, round-robin | `game/ae2x/advancedae/buses.rs`: the same over AE2's bus host (`TransferHost`), by the addons' bus hook (`ae2x::bus_tick`) |
| `ImportExportBusPart.doBusWork`: AE2's export, then an import through `FilteredImportStackTransferContext`, whose `setInverted` keeps the opposite: what the filter does not list (everything when it is empty) | AE2's `BusState::tick` for the export, then the import of the unlisted keys |
| `AdvancedIOBusPart.doBusWork`: the stock export; with `REGULATE_STOCK` (on by default) each filter's key over its amount imported (`makeFilter`, fuzzy with the card); with operations left, all of them again for the unlisted keys | the same; `regulate_stock` (`YES`/`NO`) in the part's record |
| `StockExportBusScreen`, `ImportExportBusScreen`, `AdvancedIOBusScreen` with `screens/stock_export_bus.json`, `import_export_bus.json`, `advanced_io_bus.json` (AE2's `io_bus.json`, their titles): redstone, craft-only and scheduling buttons, no fuzzy mode button; the amounts on the filters; the Advanced IO Bus's `REGULATE_STOCK` (`AAESettingToggleButton`, `REGULATE_ON`/`OFF` of `states.png`) | AE2's part screen over the mod's style (`ae2x::part_style`); the fuzzy button left out, the amounts (`AmountFormat.SLOT`) on the filters, `REGULATE_STOCK` at menu place 48 |
| `advanced_ae:throughput_monitor` (`ThroughputMonitorPart`, an `AbstractMonitorPart` with AE2's Storage Monitor models): its key and lock as AE2's | a part variant of AE2's Storage Monitor: its use, lock and key are AE2's (`ae2_monitor`, by the base id) |
| `tickingRequest` (`TickingRequest(20, 100, ..)`, `SLOWER`; `URGENT` with no reading), `ThroughputCache` (`throughputMonitorCacheSize`, 80), `averagePerTick` over `WorkRoutine`'s span, the value per tick, second, minute or ten minutes (`formatAmount(.., SLOT)` above 10, two decimals below); `configureWatchers`, `resetState` | `game/ae2x/advancedae/throughput.rs`: the same on AE2's tick tracker, the readings by part node while the game runs |
| `renderDynamic`: the item at 0.3 and its amount, then the line (`OverdriveThroughputMonitorValue` .. `SlowerThroughputMonitorValue`, `%s%s/s`), green when it grows, red when it falls | the monitor's visual carries the line (`RateVisual`); the item, the amount and the line drawn at the mod's places |
| `onUseItemOn` with `advanced_ae:throughput_monitor_configurator`: `cycleWorkRoutine`; `writeToNBT`'s `routine` | the same before AE2's monitor use; `routine` in the part's record |
| recipes `stock_export_bus`, `import_export_bus`, `advanced_io_bus`, `throughput_monitor`, `throughput_monitor_configurator` | from the jar (50 recipes now) |

## Not yet

- The encoder's Curios slot (the port has no Curios).
- With Filtered Import on, the mod's `outputCache` is filled only while the provider is on a grid; the port's filter
  reads the patterns whatever the grid. A pattern that is no processing pattern, put in the encoder's input after a
  processing one, leaves the old output in the mod (and taking it takes the new input); the port empties the output.
- A target with an ME storage capability (`AdvPatternProviderTargetCache.find`: an interface) takes every input
  whatever its face; the port asks the face.
- The Reaction Chamber's leftovers: `ReactionChamberTESR` (the fluid and items drawn inside the glass), the tooltips
  of the tanks and the power alert, the half-size flush buttons, the Memory Card's `EXPORTED_ALLOWED_SIDES`, the
  recharge from Applied Flux's FE cells (`AppliedFluxPlugin.rechargeEnergyStorage`), a storage bus on it (it sees the
  output tank only), the wrench's turn, JEI/EMI/REI, and the recipes of items the port lacks yet (`quantum_alloy`,
  `quantum_alloy_plate`, `quantum_infusion`: the fluid `quantum_infusion_source` and its block and bucket, which the
  port cannot register yet, so the Quantum Alloy has no source in survival).
- The Quantum Computer's leftovers: the formed models (`QuantumComputer*BakedModel`: the blocks keep their plain
  models, the structure the item's translucent cube), the light of a formed core and structure (`LIGHT_LEVEL` is set,
  but the port's block light does not follow a state, so they give none), the name from a renamed block, the level emitter's `isRequesting` and the
  Crafting Monitor (none in the mod) for its jobs, `quantum_infused_dust` and the alloy's block forms. The port runs as
  many jobs at once as the computer has blocks (one on a lone core), where the mod's limit is the bytes alone; a CPU
  picked in the confirmation is its free block, and a job started elsewhere meanwhile moves that block on. A job of a
  computer whose box changes is cancelled (the mod drops the cluster and its jobs with it).
- The providers' leftovers: the upgrade items' tooltips (`AAEText.PatternProviderUpgrade`,
  `PatternProviderCapacityUpgrade`), the `ae2:pattern_provider` item tag the jar adds the four providers to (the port
  merges no addon's `data/ae2` tags yet), the Memory Card on the Advanced providers (AE2's copies only AE2's).
  The upgrades act only while sneaking, as the port's ExtendedAE kits (vanilla's `useOn` comes after the provider's
  own use, which opens its menu).
- The buses' leftovers: the middle click that opens AE2's `SetAmountMenu` on a stock bus's filter (and its hint
  line, `SetAmountButtonHint`; the amount is set by the stack put in or by `/ae2 advae stock`), a filter's amount
  beyond 64 from the screen, the ME storage capability first in `getCurrentStock` (`StorageReaderImpl`: an interface
  in front is read as its item and fluid handlers), Applied Mekanistics' chemicals, the stock buses' own `extraConfig`
  in a Java world (the port saves the amounts in its own record), the Memory Card's copy of the amounts and of
  `REGULATE_STOCK`, Applied Flux's cards on the buses.
- The Throughput Monitor's leftovers: `lastHumanReadableValue` is not saved (the mod's `throughput`: after a restart
  the line is empty until the first reading, about a second), `writeVisualStateToNBT`, the line's colour for a
  painted cable when it is 0 (the port draws the amount's white), and the client side of a use with the configurator
  on a locked monitor.
- The Quantum Crafter (6), the Quantum Armor (7).

## Tests

`horizon-client-lib` (the variant, the registry rows, `NullableDirection` names, the Quantum Computer's parts and config), `horizon-command` (the component's
codec), `horizon-client` (`game/ae2x/advancedae`: the encoder's edits, the saved direction list, which provider pushes
directionally, the tooltip, re-encoding, Filtered Import; the chamber's recipes, power steps, tanks and sides; the encoder menu's slots and buttons and the item's
inventory; the screen's buttons; the providers' layouts on the windows' entries, Filtered Import's entry by size; the
upgrade items' kits; the Quantum Computer's formation, sums and saved cluster; the shared kits in ExtendedAE's tests),
and the world tests
`advancedae-provider` (the encoder's edits; the provider's screen; raw iron through a furnace's top and coal through
its side; AE2's provider pushing the same pattern to one face; a restart) and `advancedae-encoder` (the encoder's screen
used and by its hotkey, its rows and buttons; the tooltip; Filtered Import with a hopper and a shift-click; re-encoding
in AE2's terminal; a restart) and `advancedae-reaction-chamber` (power on every side; fluix for 200,000 AE of the
grid, 100 steps; the screen, its tanks, flush, a bucket from the cursor, cards and only recipe inputs; the sides with
`AUTO_EXPORT` and a chest at the front; four cards; buckets on the block; an AE2 Pattern Provider pushing items and
water and getting the fluix back; a restart; its drops) and `advancedae-quantum-computer` (no core, structure inside
or a unit on the shell do not form; a core, an entangler and an accelerator form 256 MB x 6 and 24 co-processors; a
core alone; two jobs at once from the terminal, each of its bytes; the screen's CPUs and mode; a restart; the outputs
ending both; a broken computer giving a waiting job's items back) and `advancedae-providers` (the large block's 36
slots and settings; the large part's screen and its directional push into a furnace; the upgrade on AE2's provider,
then the Capacity Upgrade, keeping the pattern and blocking mode; on ExtendedAE's provider when its content is there;
on AE2's provider part, twice, and Filtered Import on the part; a restart; the drops of the block and the part; the
recipes of the part form and the upgrade) and `advancedae-buses` (the Stock Export Bus's six cards, its amount from
the held stack and by command, the chest kept stocked; the Import Export Bus's export and import of the unlisted keys;
the Advanced IO Bus's eight cards, `REGULATE_STOCK` on its screen with no fuzzy button, the excess and the rest
imported, the amount exported back; two Throughput Monitors, one rising, one at 0, and the configurator's turn; a
restart; the drops; the monitor's recipe). The units add the buses' work (`buses.rs`) and the monitor's readings
(`throughput.rs`):

```sh
AE2_TEST_STARTUP_MS=600000 bun run ae2/scripts/test-world.ts advancedae-provider advancedae-encoder advancedae-reaction-chamber advancedae-quantum-computer advancedae-providers advancedae-buses
```
