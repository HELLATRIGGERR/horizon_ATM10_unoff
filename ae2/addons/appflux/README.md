# Applied Flux 1.21-2.1.7

[Applied Flux](https://github.com/GlodBlock/ExtendedAE/tree/appflux/1.21.1-neoforge) by GlodBlock, as All the Mods 10
ships it (`AppliedFlux-1.21-2.1.7-neoforge.jar`). Code LGPL-3.0 (`LICENSE.txt` of the repository, `mod_license` in
its `gradle.properties`), so the port's derived files carry the same terms; credit in [ae2/NOTICE.md](../../NOTICE.md),
the licence text in [ae2/LICENSES/AE2-LGPL-3.0.txt](../../LICENSES/AE2-LGPL-3.0.txt). The source lives on ExtendedAE's
repository, branch `appflux/1.21.1-neoforge`; that repository's tag `1.21-2.1.7-neoforge` is ExtendedAE's, and
Applied Flux 2.1.7 is the version bump `523c3e5a` at the head of the branch. The licence of the assets is not
stated; they are generated locally from the player's jar and never committed. The 2.1.7 jar bundles no library
(no `META-INF/jarjar`): GrandPower, the long-energy item capability of later versions, is not in it, and the cells of
this slice need no item energy. Provenance in [SOURCE_MANIFEST.json](SOURCE_MANIFEST.json), the declared content in
[content.json](content.json).

Feature `ae2-appflux`; content from
`ATM10_DIR=... bun run ae2/scripts/prepare-addon.ts appflux`; settings from `config/appflux-common.toml`
(`[mods.appflux]` in `pack/pack.toml`). All the Mods 10 ships no such file, so the mod's defaults hold:
`flux_cell.amount` 1,048,576 FE a byte, `flux_accessor.io_limit` 0 (none), `flux_accessor.enable` false (the accessor
does not charge the grid), `misc.enable` false (no FE through the Import Bus), `third_party_support.enable_mi` true.

## Slice 1: the FE key type, the FE cells, FE through buses

| Applied Flux 1.21-2.1.7 | Port |
|---|---|
| `FluxKeyType` (`appflux:flux`, `appflux.key.flux`): 16,384 FE an operation, `flux_cell.amount` FE a byte, 1 a unit, no unit symbol; `AEKeyTypes.register` | `ae2x/appflux.rs` `key_type()`, through the key-type registry (`ae2_key_types::register`) once, from the registry's build |
| `FluxKey` of an `EnergyType` (`FE`, `GTEU`): id `appflux:fe` / `appflux:gteu`, no components; `MAP_CODEC` `{type: "FE"}` | the key's `ItemKey` is its id; the codec reads and writes `type` beside `#t` and `#` of a `GenericStack` |
| `FluxKeyRenderHandler`: the `appflux:energy/fe` sprite in slots | `content.json` `key_icons`: an `appflux:fe` item drawn from that sprite, named as `appflux.type.fe.name` ("FE") |
| `ItemFECell(core, kilobytes, idleDrain)`, 1k to 256M (drains 0.5 to 7 AE/t), `UpgradeInventories.forItem(is, 1)`, a Void Card each; `StorageCells.addCellHandler(FECellHandler)`; `StorageCellModels` `appflux:block/drive/fe_<size>_cell` | handler-cell rows in the definitions registry; the game's `ae2x::HandlerCell` (`game/ae2x/appflux/`) in drives, the ME Chest, the IO Port and the terminals |
| `FluxCellInventory`: one amount in `appflux:fe_energy` (removed when 0), at most bytes × FE a byte (2^30 FE in a 1k cell, 2^48 in a 256M), used bytes the amount rounded up, no per-type cost; `insert` takes any `FluxKey` as its energy, a Void Card takes all; `getStatus` empty, not empty, full | `ae2x::appflux::FeCell`; the LED from `handler_cell_state`; the component is a persistent one (`horizon-command`) |
| `FEExternalStorageStrategy`: a Storage Bus mounts a block's `IEnergyStorage` on that side, lists `getEnergyStored`, inserts with `receiveEnergy`, extracts with `extractEnergy`, `int` amounts | the type's `ExternalStorageStrategy` over the engine's Forge Energy (`fe_info_at`, `fe_move_at`, narrow); the Storage Bus reads an addon type's strategy (`ae2_typed_storage_mounts_encoded`) and the grid's storage of the type takes its mounts by priority, partition, access and Void Card (`game/ae2_key_storage.rs`) |
| `FEStackExportStrategy`: `poweredExtraction` simulated, `receiveEnergy` simulated, both for real, the rest back into the grid | the type's `ExportStrategy`; an Export Bus's filter names FE as `appflux:flux` (`/ae2 bus filter_key ... appflux:flux\|appflux:fe` in tests) |
| `FEStackImportStrategy`, registered only with `misc.enable` | the type's `ImportStrategy`, registered by the same setting; so the Import Bus offers no FE in All the Mods 10 |
| cell, core, housing and material items, their recipes | declared items and the jar's recipes of them (`content.json`) |

Differences, known:

- A move through the grid's storage of the type pays AE (`StorageHelper.poweredInsert`) where Applied Flux's import
  strategy and the export strategy's give-back insert without paying.
- Mekanism's Induction Port (`MekInductionPortHandler`, long FE through Mekanism's own API) is reached as its plain
  Forge Energy, clamped to `int`.
- `/ae2 grid status` lists the keys of addon types (`keys`), for the tests.

Tests: `horizon-client-lib` (`ae2x::appflux`: the key type, its saved form, the cell maths for every tier, the
config, the registry rows), `horizon-client` (`game::ae2x::appflux`: the cell's keys, the strategies and the import
setting), and the world test `appflux-cells`.

## Slice 2: the portable FE cells, the Flux Accessor, FE cell disassembly

| Applied Flux 1.21-2.1.7 | Port |
|---|---|
| `ItemPortableFECell(quartKilobytes, idleDrain)`, 1k to 256M: quarter kilobytes × 256 bytes (2^28 FE in a 1k, 2^46 in a 256M), 0.5 to 5 AE/t, `UpgradeInventories.forItem(is, 4)`: a Void Card, two Energy Cards; the drive model of the tier's cell | `ae2x::appflux` `PORTABLE_CELLS`: handler cells (`FECellHandler` takes any `IFluxCell`) in drives, the ME Chest and the IO Port, as the FE cells; `Defs::add_portable_handler_cell` |
| `getAEMaxPower` 8 times AE2's from 1M up, `getChargeRate` (80 + 80 × energy cards) twice from 1M up; `PoweredItemCapabilities` | `PortableBattery` (`Defs::portable_battery`, which AE2's and MEGA's portable cells answer too): the battery, the Charger, the energy cards' capacity, the item's energy capability |
| `ItemFECell.use` / `onItemUseFirst` and `AbstractPortableCell.disassembleDrive`: sneak-use an empty cell, `StorageCellDisassemblyRecipe` (`disassembler/*`), the cards back, a portable cell's energy into its energy cell | AE2's cell disassembly (`ae2_cell_disassembly.rs`), now for a handler cell whose item disassembles (`Game::ae2x_handler_cell_disassembly`); MEGA's Bulk Item Cell still passes |
| `TileFluxAccessor`, `PartFluxAccessor`: 1 AE/t, a channel; `fast` and `io` (`EnergyIO`: input, output, both) saved | addon machines (`ExtMachine`, the part's a part machine), the settings in their config; the part's model and boxes |
| `NetworkFEPower` as the accessor's `IEnergyStorage` (`CapAdaptor`): the grid's FE, its room, `getInventory().insert` / `extract` as the IO mode allows | the FE dispatcher's AE2 adapters (`ae2_power.rs`) ask the addons first (`Game::ae2x_fe_storage`, `Game::ae2x_fe_move`): the block on every side, the part on its own; the grid's storage of the type without AE (`ae2_grid_typed_move_free`) |
| `EnergyDistributeService` and `EnergyHandler.send`: an active accessor that lets FE out pushes, each side at its `EnergyTickRecord`'s pace (1 to 10 ticks) or every tick in fast mode, at most `flux_accessor.io_limit`; FE the block refuses goes back | `tick_appflux_accessors`, from the addons' grid tick; `TickRecord` |
| `MixinCreativeEnergyCellBlockEntity`: `injectAEPower` gives everything back | the creative cell's power store takes no power with the feature (`ae2_energy_observer.rs`) |

Differences, known:

- `getMaxEnergyStored` counts the room in the grid's FE cells; FE blocks behind storage buses add none.
- The accessor reaches its neighbours as Forge Energy only (`EnergyHandler`'s Mekanism, Long Energy, MI and Flux
  Networks handlers are not used); Mekanism's machines take it through their FE capability.
- The accessor's menu (`ContainerFluxAccessor`: fast mode, IO mode) and its memory-card settings are not there yet;
  the saved `fast` and `io` are honoured.
- The portable FE cell's own menu (`portable_fe_cell`, AE2's `MEStorageMenu` over the cell) does not open: the
  portable cell screens read basic cells. The cell works in drives, chests, the IO Port and the Charger.
- `flux_accessor.enable` (the accessor charging its grid from stored FE, `EnergyHandler.chargeNetwork`) is not
  ported; All the Mods 10 leaves it off.
- `MixinChargerBlockEntity` is for the Wireless Chargers mod's charger (applied only with `wirelesschargers`), which
  this port does not have.

Tests: `horizon-client-lib` (`ae2x::appflux`: the portable cells' bytes, batteries and rows; `TickRecord`; `EnergyIO`;
the accessor's rows), `horizon-client` (`game::ae2x::appflux::accessor`), and the world test `appflux-accessor`: the
part powers another grid's Energy Acceptor from an FE cell, the block answers a Storage Bus of another grid, IO Ports
fill and empty a portable FE cell, a Charger charges it, sneak-use takes empty cells apart. The FE machine is AE2's
Energy Acceptor; the AE2 fixture loads no Mekanism.

## FE in crafting

The key-type paths of [ae2/addons/README.md](../README.md) (Key types, **Crafting**) carry FE as any addon type's key:
a processing pattern's FE (`{"#t":"appflux:flux",type:"FE","#":100000L}` in `ae2:encoded_processing_pattern`, and
its ghost in the Pattern Encoding Terminal), the crafting CPU's plan and escrow, and the Pattern Provider's
sends into the block on its face through `FEExternalStorageStrategy` (its `IEnergyStorage`). The world test
`keytypes-flux-crafting` loads a pattern of 100,000 FE into a diamond in the encoding terminal and encodes it again,
keeps the CPU's FE through a restart, sends it to another grid's Energy Acceptor and finishes the job with the diamond
given back. A right click with a charged energy item on an encoding slot sets its FE there (`EMPTY_ITEM` through
`FEContainerItemStrategy`).

## FE in generic slots and items

| Applied Flux 1.21-2.1.7 | Port |
|---|---|
| `GenericSlotCapacities.register(FluxKeyType.TYPE, 1000000L)` | `ae2x::appflux::SLOT_CAPACITY`, registered with the key type (`ae2_key_types::register_slot_capacity`): an ME Interface's config and stock slot (times the Oversize Interface's multiplier) and a Pattern Provider's return slot hold up to 1,000,000 FE |
| `FEGenericStackInvStorage` on every block with AE2's `GENERIC_INTERNAL_INV` (`AppFlux`'s capability listener): `receiveEnergy` / `extractEnergy` slot by slot, `getEnergyStored` the FE in them, `getMaxEnergyStored` FE's and the free slots times the capacity | `game/ae2x/appflux/generic.rs` (`appflux_generic_fe_storage`, `appflux_generic_fe_move`), asked by the FE dispatcher's AE2 adapters after the accessor: the interface's storage (`ae2_interface_storage_move`, no forwarding to the grid), the provider's return slots (which take FE and give none back); the provider returns it to the grid, a waiting CPU first |
| `AFUtil.shouldTryCast`: an `IUpgradeableObject` host (the interface; the provider through `MixinPatternProviderLogicHost`) only with an Induction Card | `generic::induction_card`: the card in the interface's upgrades, or in the provider's own upgrade slot (`appflux_upgrades` in its config, `MixinPatternProviderLogic`'s `upgrades`) |
| `FEContainerItemStrategy` over `Capabilities.EnergyStorage.ITEM`: contained FE, `receiveEnergy` and `extractEnergy` on one item, the changed item to the cursor or the inventory | `generic::container_strategy`, registered with the strategies (`ContainerItemStrategy.register`): the item's FE is AE2's `PoweredItemCapabilities` over its AE (portable cells, wireless terminals, tools, energy cells), which takes FE and gives none |

The world test `keytypes-flux-interface` marks FE in an interface's config slot with a charged energy cell, keeps the
stock through a restart, fills an energy cell from the ME Terminal (a right click drains nothing from it), and has a
Pattern Provider with an Induction Card take FE pushed by another grid's Flux Accessor and give it to the CPU waiting
for a pattern's FE output.

Differences, known:

- The Induction Card itself is not ported (its item, the interface's second upgrade slot, the provider's upgrade slot
  in its menu, `EnergyTicker`): a world gets it into an interface's or a provider's upgrades by its saved data.
- An item energy of a native mod (`native_item_energy_buffer`) needs the game, which a container strategy (a
  function of the item) does not see: such items are not FE containers yet.

## Later slices

- The Induction Card (`ItemInductionCard`; `registerPortableCellUpgrades`, `Upgrades.add` on the Interface and
  the Pattern Provider, block and part). On an Interface (`MixinInterfaceLogic`: its upgrade inventory grows to two
  slots) or a Pattern Provider (`MixinPatternProviderLogic`: an upgrade inventory of its own, with its menu and
  screen mixins) it wakes an `EnergyTicker`, a grid FE distributor like the accessor's, which pushes the grid's FE
  into the blocks on the host's sides (`AFUtil.getSides`). On a portable FE cell it charges the energy items in the
  player's inventory from the cell each tick (`inventoryTick`). The FE storage over the host's generic slots it opens
  (`FEGenericStackInvStorage`) is there already.
- The accessor's menu and memory-card settings; the portable FE cell's menu.
- MI EU, Mekanism's long FE, the accessor charging its grid.
