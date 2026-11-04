# MEGA Cells 4.11.0

[MEGA Cells](https://github.com/62832/MEGACells) by 90, as All the Mods 10 ships it. Code LGPL-3.0
(tag `4.11.0`, `LICENSE`; `LICENSE_megacells` in the jar), so the port's derived files carry the same
terms; assets CC BY-NC-SA 3.0, generated locally from the player's jar and never committed.
Provenance in [SOURCE_MANIFEST.json](SOURCE_MANIFEST.json), the declared content in
[content.json](content.json).

Feature `ae2-megacells`; content `content/megacells` from
`ATM10_DIR=... bun run ae2/scripts/prepare-addon.ts megacells`.

## Slice 1: M-tier item and fluid cells

| MEGA | Port |
|---|---|
| `item_storage_cell_{1,4,16,64,256}m`, `fluid_storage_cell_*m`: `BasicStorageCell(0.5 × index AE/t, tier KiB, tier / 128 bytes per type, 63 or 18 types)` | registry rows (`ae2x/megacells.rs`) on `Tier::M1`..`Tier::M256`; contents and cards in AE2's own components (`ae2:storage_cell_inv`, `ae2:upgrades`, ...), as MEGA keeps them |
| `initUpgrades`: inverter, equal distribution, void on every cell, fuzzy on item cells; 4 card slots on item cells, 3 on fluid cells | the same rows |
| `initStorageCellModels`: `megacells:block/drive/cells/<size>_<kind>_cell` | the drive draws that model |
| `cell_component_*m`: `StorageComponentItem(mb × 1024 KiB)` | storage-component rows: a Condenser takes them |
| Sky Steel and Sky Bronze from `ae2:transform` in lava; `ItemEntityMixin` keeps the ingredients alive for up to 200 ticks | AE2's transform logic, and `game/ae2x/megacells/lava.rs` for the immunity |
| housings, components, cells (shaped and with housing), disassembly, inscriber press and processor chain, Sky Steel and Sky Bronze blocks | MEGA's own 47 recipes, copied as they are |
| Sky Steel and Sky Bronze ingots and blocks are fire resistant | `minecraft:damage_resistant` on the declared items |

Differences, known:

- `LavaTransformLogic.allIngredientsPresent` answers for the first lava recipe the recipe manager lists,
  in no fixed order; the port answers for any lava recipe.
- No tooltip of cell bytes and types yet (none for AE2's cells either), and no status colour on the cell item's LED.
- MEGA's guide pages, recipe advancements and its files under AE2's namespace come with later slices.

Tests: `ae2-cell-core` (M-tier byte maths, 256M at 2,130,706,432 items of one type), `horizon-client`
(registry rows, the cell codec on MEGA cells, disassembly, condenser, lava immunity) and the world test
`megacells-cells`: Sky Steel from lava, a 1M cell crafted by MEGA's recipe, a 256M item cell full at
2,130,706,432 items and a 1M cell at 63 types in an ME Drive, a 256M fluid cell full at 2,130,706,432
buckets and a 1M fluid cell at 18 types in an ME Chest, both listed in terminals, all through a restart:

```sh
AE2_TEST_STARTUP_MS=600000 bun run ae2/scripts/test-world.ts megacells-cells
```

## Slice 2: MEGA crafting CPU blocks

| MEGA | Port |
|---|---|
| `MEGACraftingUnitType(storageMb, affix)`: `getStorageBytes` 1024 × 1024 × storageMb, so 1M 1,048,576 bytes to 256M 268,435,456; `getAcceleratorThreads` 4 on the Co-Processing Unit, 0 on the rest | `CRAFTING_UNITS` rows (`ae2x/megacells.rs`) in the registry's crafting CPU table, next to AE2's `CraftingUnitType` (1,024 bytes a KiB, 1 thread); each block's row is what it adds to its cluster (`CraftingCPUCluster.addBlockEntity`), in the CPU's checks and in the ordered formation observer |
| `MEGABlocks`: AE2's `CraftingUnitBlock` and `CraftingMonitorBlock` on MEGA's type; `MEGABlockEntities`: AE2's `CraftingBlockEntity` and `CraftingMonitorBlockEntity` | each block a machine variant of AE2's block of the same kind (`base_id`), so AE2's CPU code runs it: formation, the CPU menu, jobs, escrow, the monitor's job display, colouring the monitor |
| `CraftingCPUCalculator.isValidBlockEntity`: any `CraftingBlockEntity` | MEGA's and AE2's blocks form one CPU together (a filled cuboid of at most 17 a side with some storage); their bytes and threads add up |
| `MEGACraftingUnitModelProvider`: AE2's formed models (`UnitBakedModel`, `LightBakedModel`, `MonitorBakedModel`) on MEGA's own `block/crafting/` textures, AE2's monitor lights | the CPU renderer draws a formed block from its row's texture namespace and affix; the jar's `*_formed` models are empty for AE2's built-in loader, so `content.json`'s `formed_particle` gives those variants an empty model with MEGA's ring corner as particle, as `content/ae2` has |
| `AbstractCraftingUnitBlockMixin`: a block takes `crafting_unit_transform` upgrades into blocks of its own namespace only; MEGA's unit takes a component as AE2's does; an alternate use goes back to MEGA's own unit | the same rules for every crafting block (`game/ae2_crafting_upgrade.rs`) |
| `crafting/mega_crafting_unit` (shaped), `crafting/<block>` (MEGA unit and part, shapeless), `crafting/<block>_disassembly` (`ae2:crafting_unit_transform`) | MEGA's own 15 recipes, as they are |
| loot tables: each block drops itself | MEGA's 8 loot tables |

Differences, known:

- `CraftingUnitTransformRecipe.getUpgradedBlock` answers with the first recipe of the held item in the recipe
  manager's order, a `HashMap`'s that changes with the pack's recipe count, and MEGA's mixin then refuses an
  upgrade into the other mod's block. The engineering processor and the storage monitor upgrade both mods'
  units, so in Java one of the two can't be made in place, depending on the pack; the port looks at the recipes
  of the block's own namespace, and both can.
- `CraftingBlockItem.use` (an alternate use of a crafting block item in the air gives back its part and a unit,
  MEGA's for MEGA's with `CraftingBlockItemMixin`) is not ported, for AE2's blocks either.
- No item tooltip on the Co-Processing Unit (`AcceleratorThreads`, "Provides 4 co-processing threads per block").
- Saves keep the block's id as the block entity's, as for AE2's crafting blocks; Java's are
  `megacells:mega_crafting_unit` (unit and co-processor), `megacells:mega_crafting_storage` and
  `megacells:mega_crafting_monitor`.

Tests: `horizon-client` (the rows' bytes, threads, kinds and textures; the namespace rules of the in-place
upgrade; the formation observer adding AE2's and MEGA's blocks) and the world test `megacells-crafting-cpu`: a 1M
MEGA Crafting Storage from MEGA's recipe; a MEGA unit upgraded in place with a 4M component, back to MEGA's unit
with the component returned, then a co-processor, while AE2's unit refuses MEGA's component; a CPU of MEGA's
unit, 1M storage, co-processor and monitor forms with 1,048,576 bytes and 4 co-processors, is offered for a
1,168-byte job a 1K CPU refuses, runs it and shows it on the MEGA Crafting Monitor; AE2's and MEGA's storages
and co-processors form one CPU of 1,049,600 bytes and 5 co-processors; both through a restart; a broken MEGA
block drops itself:

```sh
AE2_TEST_STARTUP_MS=600000 bun run ae2/scripts/test-world.ts megacells-crafting-cpu
```

## Slice 3: Superdense Energy Cell, Greater Energy Card

| MEGA | Port |
|---|---|
| `MEGABlocks.MEGA_ENERGY_CELL`: AE2's `EnergyCellBlock(12800000, 3200, 12800)` with `EnergyCellBlockItem`; `MEGABlockEntities`: AE2's `EnergyCellBlockEntity` | `MEGA_ENERGY_CELL` (`ae2x/megacells.rs`) in the registry's energy cell table, next to AE2's `ENERGY_CELL` (200,000 AE, 800 AE/t, priority 200) and `DENSE_ENERGY_CELL` (1,600,000, 1,600, 1,600); the block is a machine variant of AE2's dense cell, so AE2's cell code runs it with its own row: the stored maximum (`StoredEnergyAmount`, on load and placement), the grid device and the energy service's store and priority, the `fullness` state (`getStorageLevelFromFillFactor`), the Charger's capacity and rate for the item |
| `EnergyService`: injection into the lowest priority first, extraction from the highest first | unchanged: the Superdense cell fills after AE2's cells and gives its energy before them |
| `AEBaseEntityBlock.getDrops` with `EnergyCellBlockEntity.exportSettings(DISMANTLE_ITEM)`; `importSettings` on placement | the stored AE goes on the dropped cell when a player breaks it (AE2's cells too: the port had it for the wrench and the annihilation plane only), and back into the placed block; a placed item's data now reaches any addon's AE2 machine, whose saved tag the vanilla block-entity canonicaliser did not know (it passes AE2's own `ae2:` ids only) |
| `MEGACellsClient.initEnergyCellProps` / AE2's `InitItemModelsProperties`: `ae2:fill_level` = `getAECurrentPower / getAEMaxPower`; the jar's 1.21.1 item model picks `mega_energy_cell_1` to `_4` at 0.2 to 0.8 | `prepare-addon.ts` turns one-property mod `overrides` into a 26.3 `range_dispatch` on that property; item models take a mod's own range-dispatch property (`NumericProperty::Host`, NeoForge's `RegisterRangeSelectItemModelPropertyEvent`), which the game answers for every energy cell of the table |
| `MEGAItems.GREATER_ENERGY_CARD`: `EnergyCardItem(p, 8)`; `Upgrades.getEnergyCardMultiplier` sums the cards' multipliers | the registry's energy card table (AE2's card 1, MEGA's 8); every item host reads the multiplier: portable cells and the colour applicator and matter cannon `1 + 8 × multiplier` times their battery, wireless terminals `1 + multiplier`, charge rates `rate × (1 + multiplier)`; a Network Tool's pocket takes it (`Upgrades.isUpgradeCardItem`) |
| `MEGACells.initUpgrades`: the card on AE2's ten portable cells, both wireless terminals, the colour applicator and the matter cannon, 2 at most each | the same upgrade rows. The limit is per card (`getMaxInstallable`; the tooltip groups only name hosts): two-slot hosts hold two energy cards of either kind, the four-slot matter cannon two of each |
| `crafting/mega_energy_cell` (shaped), `crafting/greater_energy_card` and `greater_energy_card_upgraded` (shapeless); the cell's loot table | MEGA's own recipes and loot table, as they are |

Differences, known:

- AE2's own `energy_cell` and `dense_energy_cell` items still draw their empty model: `content/ae2`'s item definitions
  predate the `ae2:fill_level` property; the game answers it for them, regenerating `content/ae2` with
  `range_dispatch` definitions would show it.
- MEGA's `ae2:p2p_attunements/fe_p2p_tunnel` tag entry (the Superdense cell attunes an FE P2P tunnel) lives under
  AE2's namespace and waits, with the other `data/ae2` tags, for its merge into `content/ae2`.
- AE2WTLib's `addUpgradeToAllTerminals` (MEGA's card on the universal terminal) needs AE2WTLib, which the port does not carry.
- No item tooltip of stored energy (`Tooltips.energyStorageComponent`) and no comparator output
  (`EnergyCellBlock.getAnalogOutputSignal`), for AE2's cells either; no charged Superdense cell from
  `EnergyCellBlock.addToMainCreativeTab` (the port's AE2 creative catalog lists AE2's items only).
- The block state follows the stored energy whenever the port refreshes AE2's visuals, not one grid tick later
  as `neighborChangePending` delays it.
- On the tick a node joins a grid, the port's fallback grid drain can take that tick's idle power from the cells in
  position order rather than by priority (seen as 2 AE off a dense cell); the energy service's extraction, by
  priority, does the rest. AE2's cells alike.

Tests: `horizon-client` (the cell's row, capacity on load and fill levels; the card's upgrade rows and its
multiplier in the matter cannon, colour applicator, portable cell and wireless terminal; the Charger filling the
Superdense cell at 32,000 AE a cycle), `ae2-network-core` (a grid device past its kind's maximum),
`horizon-assets` (a mod's range-dispatch property) and the world test `megacells-energy`: both recipes; a cell
placed with 12,000,000 AE (fullness 3) and one clamped from 20,000,000 to 12,800,000 (fullness 4); broken by the
player and placed again with its energy; in a grid behind an Energy Acceptor, AE2's dense cell fills before
MEGA's, which charges to exactly 12,800,000 AE, and a drive's idle drain comes out of MEGA's cell first; Greater
Energy Cards in a Cell Workbench give a portable cell 1,300,000 and 2,580,000 AE (1,460,000 with AE2's card) and a
Matter Cannon with two of each card 29,000,000; the energy through a restart; the item's empty, half and full
models in the inventory:

```sh
AE2_TEST_STARTUP_MS=600000 bun run ae2/scripts/test-world.ts megacells-energy
```

## Slice 4: portable M cells, Portable Cell Workbench

| MEGA | Port |
|---|---|
| `MEGAItems.itemPortable` / `fluidPortable`: `MEGAPortableCell(tier, keyType, AE2's portable cell menu, 0x80caff)`, a `PortableCellItem` of `18 + (index - 5) × 9` item types (27 at 1M to 63 at 256M) or 18 fluid types | registry rows (`ae2x/megacells.rs`, `portable_cell`) on `CellConfig::portable`: half the tier's bytes (`getBytes` = `tier.bytes() / 2`, 524,288 at 1M to 134,217,728 at 256M), a 128th of the tier's bytes a type; the drive draws the tier's cell model (`initStorageCellModels` covers the portable cells) |
| `MEGAPortableCell.getIdleDrain` 1.0, `getAEMaxPower` `super × 8`, `getChargeRate` `super × 2` | `CellDef.idle_drain` 1.0 (a drive or chest draws it), `battery_scale` 8 and `charge_scale` 2, which the portable battery reads: 160,000 AE with no card, 10,400,000 with one Greater Energy Card, 20,640,000 with two; 160 AE/t in a Charger, 1,440 and 2,720 with the cards. `onUpgradesChanged` is AE2's, so a change of cards clamps the charge to 20,000 × (1 + 8 × multiplier), before the × 8, as in Java |
| `MEGACells.initUpgrades`: inverter, equal distribution, void (and fuzzy on item cells) and two Greater Energy Cards on each portable cell; AE2's energy card is not registered on them | the same rows |
| `MEGACells.initStorageCells`: `HotkeyActions.register` for each portable cell, after AE2's, each first | the hotkeys rank the registry's portable cells by registration order, last first: MEGA's 256M cell answers before AE2's 256K |
| `MEGACells.initCapabilities`: `PoweredItemCapabilities` on the portable cells | the energy capability's probe asks the registry |
| `cells/portable/portable_{item,fluid}_cell_*m` (shapeless: ME Chest, component, Dense Energy Cell, housing) and their `ae2:storage_cell_disassembly` recipes | MEGA's own 20 recipes; `AbstractPortableCell.disassembleDrive` gives the parts back and the cell's energy to the Dense Energy Cell |
| `PortableCellWorkbenchItem` (`stacksTo(1)`): `use` opens `PortableCellWorkbenchMenu` on `PortableCellWorkbenchMenuHost`; `PortableCellWorkbenchInventory` keeps one `ICellWorkbenchItem` as the item's `minecraft:container` and a 63-slot `GenericStackInv(CONFIG_TYPES)` as `ae2:exported_config_inv`, and writes `ae2:exported_settings` | `game/ae2x/megacells/portable_workbench.rs`: the item's state as a Cell Workbench `Machine` no block holds (`ae2_workbench::workbench_machine`), on which AE2's own workbench code runs: the cell slot and `onChangeInventory` (the filters follow the cell in and, in clear-on-remove mode, out), the cards, the filters, fuzzy mode, partition, clear and copy mode (`ae2_workbench_menu_click_on`, shared with the block's menu); then `saveChanges` onto the item |
| `ItemMenuHostMixin`: the host's `getUpgrades` is the held cell's own upgrade inventory, proxied | the card slots edit the cell's `ae2:upgrades`, which sets its battery (`onUpgradesChanged`) |
| `PortableCellWorkbenchInventory`'s `IConfigManager` is built with `CLEAR_ON_REMOVE` and never reads `ae2:exported_settings` | the copy mode is the menu's: each opening starts clearing on removal; the filters themselves stay on the item |
| `PortableCellWorkbenchScreen` with `screens/megacells/portable_cell_workbench.json` (`cell_workbench.json` with the item as title) | the client draws AE2's Cell Workbench screen (`ae2_workbench_screen`) from the style the content script copies under `assets/megacells/ae2/`; the server names the window's screen (`ae2:menu_block`), which a window opened in the air now honours |
| `network/portable_cell_workbench` (shaped: Cell Workbench, Singularity, MEGA Item Cell Housing) | MEGA's recipe |

Differences, known:

- No LED or housing colour on the portable cells' items (`PortableCellItem.getColor`, tint indexes 1 and 2), for AE2's either; no
  cell tooltip, and no tooltip image of the workbench's filters and cell (`PortableCellWorkbenchTooltipComponent`).
- The workbench's compression cutoff button (`mega$nextCompressionLimit`) waits for the Bulk Item Cell (slice 7).
- A shift-click of a card from the inventory does not go into the held cell's card slots, as on the port's Cell Workbench block.
- The item's empty `minecraft:container` is left out where Java writes an empty list.
- The port's AE2 creative catalog still lists AE2's items only: no charged MEGA portable cell (`addToMainCreativeTab`).

Tests: `horizon-client-lib` (the portable rows: types, bytes, drain, factors, cards, a 1M portable item cell full at 4,128,768
items of one type, the registration order), `horizon-client` (the battery with 0, 1 and 2 Greater Energy Cards and its clamp,
the charge rate, MEGA's portable cells answering the hotkey before AE2's, the workbench item's container, filters and settings
read back) and the world test `megacells-portable`: both recipes; the cell charging at 160 AE a tick (3,200 AE in 20 ticks)
and stopping at 160,000 (AE2's 1K portable cell at 20,000); opened and storing three diamonds from its battery; 27 types and a 28th refused; 4,128,768
cobblestone; disassembled with its 100,000 AE in the Dense Energy Cell; in the Portable Cell Workbench, two Greater Energy
Cards (2,580,000 AE before the × 8) and a filter installed, AE2's energy card refused, the filters cleared with the cell out
and kept in keep-on-remove mode, a new menu clearing again; the cell, its cards and its filter in the workbench through a
restart:

```sh
AE2_TEST_STARTUP_MS=600000 bun run ae2/scripts/test-world.ts megacells-portable
```

## Slice 5: MEGA Pattern Provider and MEGA Interface, block and part

| MEGA | Port |
|---|---|
| `MEGABlocks.MEGA_PATTERN_PROVIDER` (`MEGAPatternProviderBlock`, `MEGAPatternProviderBlockEntity`) and `MEGAItems.MEGA_PATTERN_PROVIDER` (`cable_mega_pattern_provider`, `MEGAPatternProviderPart`): AE2's `PatternProviderBlockEntity` and `PatternProviderPart` with `createLogic` = `PatternProviderLogic(node, host, 18)` | machine variant rows (`ae2x/megacells.rs`, `MACHINES`) on `MachineKind::PatternProvider` with 18 slots, so AE2's provider code runs them: pushing, blocking and lock modes, AE2's nine return slots, priority, the Pattern Access Terminal, saves and drops. The crafting provider registry took a provider block of AE2's nine slots or of the largest addon's (ExtendedAE's 36) alone; it takes any provider's size now |
| `createLogic` sets `AEItemDefinitionFilter(AEItems.PROCESSING_PATTERN)` on the pattern inventory (`AppEngInternalInventory.setFilter`); `isItemValid` asks it for the menu's slots (`AppEngSlot.mayPlace`) and for the Pattern Access Terminal (`InternalInventory.insertItem`, `addItems`) | `ae2x::megacells::pattern_filter`, which `game/ae2x` answers (`pattern_inventory_accepts`) for `ae2_pattern_access::provider_pattern_valid`: every placement of a pattern into a provider goes through it, a click, shift-click or hotbar swap in the block's and the part's menu, and the Pattern Access Terminal's click and shift-click (a provider that refuses is passed over, as `addItems` passes it). A refused pattern shift-clicked into the menu goes to the return slots, which take anything (`AEBaseMenu.quickMoveStack`, a `PatternProviderReturnInventory`), and from there into the grid, as in Java |
| `MEGAPatternProviderBlock`: AE2's `PUSH_DIRECTION`, `ALL` when placed; `useItemOn` with a wrench: `setSide`, AE2's rule; `neighborChanged`: `updateRedstoneState` | the state in `content.json`; AE2's wrench, push and redstone code go by `base_id` |
| `MEGAInterfaceBlockEntity`, `MEGAInterfacePart`: `InterfaceLogic(.., 18)`: 18 config and 18 storage slots, AE2's one card slot (`UpgradeInventories.forMachine(is, 1, ..)`) | rows on `MachineKind::Interface`; the window is `MenuShape`'s for an Interface past nine slots, with one page of 18 |
| `MEGACells.initUpgrades`: `CRAFTING_CARD` and `FUZZY_CARD`, one each, on the block and the part | the same upgrade rows; the providers take none |
| `MEGAMenus`: AE2's `PatternProviderMenu` and `InterfaceMenu`; `MEGACellsClient`: AE2's `PatternProviderScreen` and `InterfaceScreen` with `screens/megacells/mega_pattern_provider.json` (176 x 215: 18 patterns in rows of nine, `BREAK_AFTER_9COLS`, then AE2's return row) and `mega_interface.json` (176 x 240: two config rows between `amtButton1..9` and `amtButton10..18`, two storage rows); their textures `guis/megacells/*.png` | layouts in `game/ae2x/megacells/screens.rs`: the provider's patterns 0..18, returns 18..27, settings 27..29, priority 53; the interface's storage 0..18, config 18..36, card 36, fuzzy mode 41 (with a fuzzy card), priority 53, a stock cog above or below each config slot. The content script copies the styles and textures under `assets/megacells/ae2/`; the GUI atlas now reads an addon's GUI textures in folders, named as in the jar (`ae2:guis/megacells/mega_interface`) |
| `MEGAPatternProviderPart.MODEL_BASE`, `MEGAInterfacePart.MODEL_BASE` (`megacells:part/mega_*`) over AE2's `part/interface_off`, `_on`, `_has_channel` | `game/ae2x/megacells::part_model` |
| `MEGAPatternProviderBlockItem.addCheckedInformation`, `MEGAPatternProviderPartItem.appendHoverText`: `MEGATranslations.ProcessingOnly` in `NOTICE` (0xffde7d, italic) | both items' tooltips, below the name (`ae2x::item_hover_lines`) |
| `network/mega_pattern_provider` and `network/mega_interface` (shapeless: AE2's device and an Accumulation Processor), their `_part` and `_block` recipes (each form from the other); the blocks' loot tables | MEGA's six recipes and two loot tables, as they are |

Differences, known:

- A Memory Card does nothing on these devices yet, as on ExtendedAE's: the port's provider and interface memory code
  still takes AE2's nine slots and ids. In Java, `PatternProviderLogic.importSettings` restores the card's patterns
  with `setItemDirect`, past MEGA's filter.
- `MEGAInterfacePart.returnToMainMenu` opens its menu anew (`MenuOpener.open`) where the block returns to it
  (`returnTo`); the port goes back from the stock and priority editors the same way for both.
- MEGA's item tags `megacells:mega_pattern_provider` and `megacells:mega_interface` (block and part) come with the
  content; nothing in this slice reads them. MEGA's guide pages and recipe advancements come later; the AppliedE
  MEGA Transmutation Interface needs AppliedE.

Tests: `horizon-client-lib` (the four rows' kinds, sizes, bases and screens, the cards, the pattern filter and the
notice), `horizon-client` (MEGA's providers refusing every encoded pattern but the processing one, the part models,
the tooltip line's text and style, the screens' entries against the server's window, an 18-slot provider block in
the crafting provider registry) and the world test `megacells-provider-interface`: the block provider opens
MEGA's screen with its settings at 27 to 29; a crafting pattern goes into no pattern slot, shift-clicked it goes
through the return slots into the grid and from the cursor it stays there; 18 processing patterns fill the pattern
slots and a 19th goes the returns' way; a wrench on its south face turns `push_direction` from `all` to `north`,
and crafts from its first and 18th patterns push into the chest to its north and none above; the block interface
opens MEGA's screen, stocks its first and 18th slots (the latter set to 2 in its stock editor), refuses a speed card
and takes a fuzzy card, which shows fuzzy mode; the parts attach to a cable, open the same screens, the provider
part refusing a crafting pattern from the cursor and taking a processing one, the interface part stocking; MEGA's six recipes; the
patterns, push direction, config, stock, card and the parts' contents through a restart; broken, the blocks and the
cable drop the devices with their patterns, stock and card:

```sh
AE2_TEST_STARTUP_MS=600000 bun run ae2/scripts/test-world.ts megacells-provider-interface
```

## Later slices (`.build/ae2-addons-plan.md` §5.1)

6. ME Cell Dock.
7. Bulk Item Cell, Compression Card, Decompression Module.
8. Cells for other addons' key types (chemical, radioactive, source, soul), with those addons.

`neoforge:mod_loaded` recipe conditions: not needed by slice 1 (its recipes have none); the 107
conditional recipes all belong to cells of other addons and are left out by the content script.
