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

## Later slices

- The portable FE cells (`ItemPortableFECell`, a quarter of the bytes, their menu and battery).
- The Flux Accessor, block and part (`TileFluxAccessor`, `PartFluxAccessor`, `NetworkFEPower`, `EnergyTicker`):
  needs the game's own block to answer the FE dispatcher (`extra_fe_info` reaches AE2's machine kinds only).
- The Induction Card on Interfaces and Pattern Providers (`MixinInterfaceLogic`, `MixinPatternProviderLogic`): FE in
  their generic slots, which hold items and fluids only.
- Filling and draining energy items in the terminal (`FEContainerItemStrategy`): the terminal's gestures use the
  bucket strategy of fluids only; marking FE in a config slot with an energy item.
- `GenericSlotCapacities` (1,000,000 FE), the Charger and Creative Energy Cell mixins, MI EU, Mekanism's long FE.
