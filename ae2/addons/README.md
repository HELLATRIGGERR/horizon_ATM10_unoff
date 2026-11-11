# AE2 addons from All the Mods 10

Each addon the port carries is a module of its own:

| Piece | Where |
|---|---|
| Switch | `[mods.<id>]` in `pack/pack.toml`: `enabled`, the Cargo `feature` (`ae2-<id>`), the `content` folder, the ATM10 `configs` |
| Rows in the definitions registry | `crates/horizon-client/src/ae2x/<id>.rs`, behind the feature |
| Machines that are AE2's with other sizes | `MachineVariant` rows (id, base, kind, sizes) the addon's `register` adds to the registry; screens as layouts under `game/ae2x/<id>/` |
| Game behaviour, when the addon has any | `crates/horizon-client/src/game/ae2x/<id>/` |
| Pure logic worth its own tests | `crates/ae2-<id>-core` |
| Provenance, slices, status | `ae2/addons/<id>/`: `SOURCE_MANIFEST.json`, `content.json`, `README.md` |
| World tests | `ae2/tests/world/<id>-*.ts`, with `new WorldFixture(label, mode, { addons: ['<id>'] })` |

`ae2/scripts/build.ts` and `test.ts` build with the features of the enabled mods; `play.ts` loads each
enabled addon's content folder next to AE2's, and the world harness loads those a fixture names (or that
`AE2_ADDONS=<id>,...` names for every fixture).

## Content

The player's own jar makes the content, as AE2's source makes `content/ae2`; nothing generated is
committed:

```sh
ATM10_DIR="/path/to/All the Mods 10" bun run ae2/scripts/prepare-addon.ts <id>
```

The script checks the jar against `SOURCE_MANIFEST.json`'s SHA-256 and writes `content/<namespace>/`:
the addon's language files, textures and models; blockstates, item definitions and `data/<ns>/horizon/`
declarations for the blocks and items `content.json` lists; and of the jar's recipes, loot tables and
tags, those of the declared items only. Recipes stay in their 1.21.1 form, which the port reads, as it
does AE2's. A recipe that names an item of a later slice, has a recipe type of the addon's own, or
carries NeoForge conditions, is left out and listed in `content-inventory.json`, with the files that
belong to later slices (the addon's files under AE2's namespace, its guide, data maps, advancements).
Screen styles and GUI textures the addon ships under AE2's namespace, when `content.json` names them
(`ae2_namespace`), go to `assets/<ns>/ae2/`, as a content mod owns its own namespace alone; the game
looks there next to AE2's, folders included (MEGA's `guis/megacells/mega_interface.png` is the GUI atlas's
`ae2:guis/megacells/mega_interface`, as in the jar). A block whose model needs a custom loader names the model to draw (`model`).
The addon's own textures that its AE2 screens draw by a namespaced id (`own_gui_textures`: AE2 Import Export Card's
`ae2importexportcard:textures/gui/upgrade_0.png`) are also kept under `assets/<ns>/ae2/textures/<ns>/`, the GUI
atlas's `ae2:<ns>/gui/upgrade_0`.

## The definitions registry

`horizon_client::ae2x::defs` holds what AE2's public API (`appeng.api`, MIT) lets an addon declare, as
tables built once on first use: AE2's own rows (`Defs::ae2`), then each built-in addon's `register`.

| Table | AE2 API | Read by |
|---|---|---|
| cells: id, key type, tier (kilobytes), types, idle drain, portable, drive model; a portable cell's battery and charge-rate factors over AE2's | `IBasicCellItem`, `StorageCells`, `StorageCellModels`; `PortableCellItem` (`getAEMaxPower`, `getChargeRate`, `HotkeyActions.registerPortableCell`) | the cell codec (`is_cell`, `is_fluid_cell`, `idle_drain`), drives, chests, IO port, workbench, condenser, drive rendering; for the portable cells (`portable_cell`, `portable_cells` in registration order): their menu, battery and charge in a Charger, energy capability, upgrade store, disassembly and hotkeys |
| cells of an addon's own handler: id, idle drain, drive model, filters; their card slots | `StorageCells.addCellHandler` with an `ICellHandler` of its own (MEGA's Bulk Item Cell, ExtendedAE's Void Cell, Ender Drives' disks; Applied Flux's FE cells, of its key type) | the codec (`is_cell`, `idle_drain`, `config_slots`), `is_valid_storage_cell`; the game's `ae2x::HandlerCell` for the listing, storage, preference, the ME Chest and the IO Port; `handler_cell_state` for LEDs |
| storage components: id, tier | `IStorageComponent` | the Condenser |
| item upgrade slots: host, size | `IUpgradeableItem.getUpgrades` | `ae2_upgrade_inventory::capacity` |
| upgrade rows: card, host, max | `Upgrades.add` | `ae2_upgrade_inventory` |
| machines: block id, slots, card slots, owned keys a break drops | `AEBaseEntityBlock` with its block entity | `MachineKind::Ext(index)` (`MachineKind::from_id`, saves, drops); the game's `ae2x` gives the grid node, block state and menu |
| machine variants: block or part id, the AE2 id it behaves as, AE2's kind, slots, card slots, the block whose screen it shows | a subclass of AE2's block entity or part with other sizes (ExtendedAE's `TileExDrive`) | `ae2_world::variant`, `base_id`, `Machine::for_block` and the block entity's id; all behaviour is the kind's |
| parts: id, idle power, channel; the cells a part holds (`IChestOrDrive` with `IStorageProvider`: MEGA's Cell Dock), its state a part machine of its own kind (an `ExtMachine` row with `part`) | `IPart` with its grid node | part placement and the grid's idle draw; a cell part's cells: the grid's cell mounts at the part's priority (`ae2_cell_hosts_encoded`), their idle drain, the menu's slot filter, the part face's item handler (hoppers), drops, the client's cell and LED |
| crafting CPU blocks: block id, AE2's kind, storage bytes, co-processor threads, texture namespace and affix | `ICraftingUnitType` on AE2's `CraftingUnitBlock` (MEGA Cells' `MEGACraftingUnitType`) | `ae2_world::crafting_unit`: the CPU's bytes and threads (`CraftingCPUCluster.addBlockEntity`), its formation, the formed model's textures, the in-place upgrade; each is also a machine variant of AE2's block of its kind |
| energy cells: block id, AE2's kind, max power, charge rate, priority | `EnergyCellBlock(maxPower, chargeRate, priority)` with `EnergyCellBlockItem` (MEGA Cells' Superdense Energy Cell) | `ae2_world::energy_cell`: the block entity's stored-energy maximum (saves, placement, the grid device, the energy service's store and priority), its `fullness`, the item's capacity and charge rate in a Charger, its `ae2:fill_level` model property, the energy its drop keeps; each is also a machine variant of AE2's cell of its kind |
| energy cards: card id, multiplier | `EnergyCardItem(properties, energyMultiplier)` (MEGA Cells' Greater Energy Card, 8) | `Upgrades.getEnergyCardMultiplier` of an item's cards: the batteries and charge rates of portable cells, wireless terminals, the colour applicator and the matter cannon; where a card fits is its upgrade rows' |
| wireless terminals: name, item, menu kind, hotkey, upgrade slots, the universal terminal's component; the universal terminal: item, current-terminal component | AE2WTLib's `WTDefinition` (`AddTerminalEvent.builder`, MIT API) over AE2's `WirelessTerminalItem`s; `ItemWUT` | `ae2x::wireless`: which items are wireless terminals and what each opens (the universal one its current terminal), linking, battery, charge rate and card slots; the game's terminal, encoding terminal and Pattern Access menus on an item, item use and hotkeys |
| cards on a card: an upgrade card that takes cards of its own and is no `IUpgradeableItem` | `Upgrades.add(card, host)` with an addon's card as the host (AE2 Import Export Card's Import and Export Cards) | the upgrade rows alone, so `upgrade_limit` answers while the card has no upgrade slots (`capacity` none: AE2's `add_item_upgrade` leaves it); its own cards are read and checked by the addon (`ae2x::importexportcard`) |
| access point cards: item, signal, same dimension only, shown drain, chunk loading | AEInfinityBooster's mixins on `WirelessAccessPointBlockEntity`, `RestrictedInputSlot`, `WirelessTerminalMenuHost.getAccessPointSignal` and `WirelessAccessPointMenu` | the booster slot's filter, the terminal's nearest access point, the access point's screen; the game's `ae2x` keeps the chunk loaded |
| processing pattern items: item, component | `PatternDetailsHelper.encodedPatternItemBuilder` with an addon's decoder of processing details (Advanced AE's `AdvProcessingPattern`, its `EncodedAdvProcessingPattern` beside AE2's sparse lists) | `processing_pattern_component`: the processing pattern decoders (`ae2_decode_processing_pattern` and the source view of the component), `encoded_pattern_item` (provider and Pattern Access slots, Memory Card blanks); what the addon keeps beside the sparse lists is read by its own code |

`ae2-cell-core::Tier` is a size in kilobytes, so a cell of any power-of-four tier up to 256M works
with the verified byte maths. The game reaches an addon's behaviour through `game/ae2x/mod.rs` alone (its tick, grid node, block
state, menus, screen, item uses, part models and `/ae2` commands). One exception: a variant's screen is a layout of AE2's
own screen code, so `game/ae2_screens.rs` takes the addon's layouts as a child module (`addon_layout`).

## Key types

AE2's `AEKeyType` registry (`ae2:keytypes`): what a stack's key is, beside its identity. AE2 registers
items (`ae2:i`, raw id 0) and fluids (`ae2:f`, raw id 1) first, as `AppEngBase.registerKeyTypes` does; an
addon registers its own after them, once, before its content is used (Applied Flux's `appflux:flux`,
Applied Mekanistics' chemicals). A key stays an `ItemKey` (id and canonical component bytes); its
`KeyType` travels beside it wherever AE2's `AEKey.getType()` would be asked: `GenericStack.key_type`,
`CellConfig.key_type`, the cell definitions' `key_type`, a cell's stored keys of other types, the crafting
CPU's byte scale, a bus state's channel, the selections of an import bus and a wireless terminal.

| Piece | Where | AE2 |
|---|---|---|
| Id, description, amounts, unit, fuzzy ranges | `ae2_cell_core::KeyTypeDef`, `keys::register` | `AEKeyType` (`getId`, `getDescription`, `getAmountPerOperation`, `getAmountPerByte`, `getAmountPerUnit`, `getUnitSymbol`, `supportsFuzzyRangeSearch`), `AEKeyTypes.register` |
| Amount text | `KeyType::format_amount` (`AmountFormat::Full`, `Slot`, `SlotLargeFont`) | `AEKeyType.formatAmount`, `ReadableNumberConverter` |
| Sets of types | `KeyTypeSet` | `KeyTypeSelection`, `ae2:enabled_key_types` |
| A key with its type | `TypedKey` | `AEKey` |
| Saved form of a key | `horizon_client::ae2_key_types::KeyCodec` (`id_components_*` for a key of an id and components) | `AEKeyType.codec()` |
| Icon in item slots | `KeyBehaviour.icon` | `AEKeyRendering.register` |
| Ids the game knows | `KeyBehaviour.known` (an unknown one loads as Missing Content) | the registry lookup of `AEKey.MAP_CODEC` |
| Items that hold the type | `KeyBehaviour.container`: `ContainerStrategy` (contained, fill, drain) | `ContainerItemStrategies.register` |
| Import, export, storage bus, formation | the game's `ae2_key_strategies::register` (`ImportStrategy`, `ExportStrategy`, `ExternalStorageStrategy`, `PlacementStrategy`), from `game/ae2x/mod.rs` `register_key_strategies` before the first read | `StackWorldBehaviors.register*Strategy` |
| An addon's key type, all of it | Applied Flux's `appflux:flux` (`ae2x/appflux.rs`, `game/ae2x/appflux/`): FE keys, FE cells of its own handler, FE through storage, import and export buses | `FluxKeyType`, `FECellHandler`, `FE*Strategy` |

`horizon_client::ae2_key_types::register(def, behaviour)` registers both halves; registering the same
definition again returns the same type, another definition under a taken id is refused. The raw id is
the order of registration and never saved: saves name a type by its id (`#t` in a `GenericStack`, AE2's
`enabledKeyTypes` of an import bus, `ae2:enabled_key_types` of a wireless terminal), so a world saved with
an addon's type and loaded without it keeps the stack as Missing Content that saves back unchanged. A bus
state's JSON keeps `"Item"` and `"Fluid"` for AE2's two.

What reaches a key of an addon's type, and what doesn't yet:

- **Storage**: a cell of an addon's own handler holds keys of its `HandlerCell::key_type` (Applied Flux's FE
  cells) in drives, the ME Chest and the IO Port. A basic cell of the type (a `CellDef` with its `key_type`)
  holds it with the type's bytes, in AE2's form only (`ae2:storage_cell_inv`); drives, the ME Chest, cell docks, the IO Port and portable
  cells read and write it (`ae2_cell_codec::read_any_cell_checked`, `write_any_cell`). The grid's storage of
  the type is its cells' (`game/ae2_key_storage.rs`: `ae2_grid_keys_encoded`, `ae2_grid_key_move`), paid in
  AE per `getAmountPerOperation`; creative and fixed cells may name its keys.
- **Terminals**: the ME and wireless terminals list every type (`horizon:key/<type id>|<id>` stands for an
  addon type's key in the listing), sort it in its units, show it with its icon and amount, and filter it
  with the key-type buttons, which list every registered type.
- **Buses**: an import bus selects among the types with an import strategy; import and export buses move
  an addon type's keys through its `ImportStrategy` / `ExportStrategy`.
- **Storage buses**: a bus mounts a block the type's `ExternalStorageStrategy` finds storage in, and the grid's
  storage of the type lists and moves through it by priority, partition, access and Void Card.
- **Crafting**: a processing pattern's inputs and outputs may be keys of an addon's type, saved in AE2's form
  (`ae2:encoded_processing_pattern`, the type's id in `#t`); the Pattern Encoding Terminal shows such a key as a
  ghost of its type (a wrapped stack's own type, a loaded pattern's) and encodes it again. Crafting carries such
  a key beside the fluids as its marker (`ae2_key_types::addon_marker`, `horizon:key/<type id>|<id>`; a job names
  it `fluid|<marker>|<components>`):
  the plan counts the grid's keys of the type (`ae2_grid_crafting_fluids`) and their bytes by the type's amount
  per byte, the CPU reserves them into its escrow (`ae2_cpu_fluid_escrow`, the type's id in `#t` beside the key's)
  and pays one AE per `getAmountPerOperation` of a push, and the Pattern Provider sends them through the type's
  `ExternalStorageStrategy` on its face (`PatternProviderTargetCache`), keeping what the block refuses in its
  send queue the same way; blocking mode asks the same strategy whether the block holds the key. The grid's
  moves of the type (`ae2_grid_key_move_encoded`, and the fluid moves crafting makes with a marker) give an
  insert to the CPUs waiting for the key first (`CraftingService.insertIntoCpus`), so an Import Bus can bring a
  pattern's output of the type back; the Flux Accessor's free moves do not.
- **Not yet**: formation planes place items and fluids only (the placement strategy is registered but not
  read); a Pattern Provider takes back items and fluids only (Applied Flux's `FEGenericStackInvStorage`, a
  provider's or an interface's internal inventory as FE storage, is not ported), and the ME Interface's config
  and stock slots hold items and fluids only; the terminal's fill and drain gestures use the bucket strategy of
  fluids, not an addon type's `ContainerStrategy` (Applied Flux's `FEContainerItemStrategy` over an item's FE is
  not ported); the native workbench keeps an addon type's filter in AE2's `GenericStack` form (`stack`).
