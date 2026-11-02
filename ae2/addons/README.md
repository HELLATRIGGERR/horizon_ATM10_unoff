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
looks there next to AE2's. A block whose model needs a custom loader names the model to draw (`model`).

## The definitions registry

`horizon_client::ae2x::defs` holds what AE2's public API (`appeng.api`, MIT) lets an addon declare, as
tables built once on first use: AE2's own rows (`Defs::ae2`), then each built-in addon's `register`.

| Table | AE2 API | Read by |
|---|---|---|
| cells: id, key type, tier (kilobytes), types, idle drain, portable, drive model | `IBasicCellItem`, `StorageCells`, `StorageCellModels` | the cell codec (`is_cell`, `is_fluid_cell`, `idle_drain`), drives, chests, IO port, workbench, condenser, drive rendering |
| storage components: id, tier | `IStorageComponent` | the Condenser |
| item upgrade slots: host, size | `IUpgradeableItem.getUpgrades` | `ae2_upgrade_inventory::capacity` |
| upgrade rows: card, host, max | `Upgrades.add` | `ae2_upgrade_inventory` |
| machines: block id, slots, card slots, owned keys a break drops | `AEBaseEntityBlock` with its block entity | `MachineKind::Ext(index)` (`MachineKind::from_id`, saves, drops); the game's `ae2x` gives the grid node, block state and menu |
| machine variants: block or part id, the AE2 id it behaves as, AE2's kind, slots, card slots, the block whose screen it shows | a subclass of AE2's block entity or part with other sizes (ExtendedAE's `TileExDrive`) | `ae2_world::variant`, `base_id`, `Machine::for_block` and the block entity's id; all behaviour is the kind's |
| parts: id, idle power, channel | `IPart` with its grid node | part placement and the grid's idle draw |

`ae2-cell-core::Tier` is a size in kilobytes, so a cell of any power-of-four tier up to 256M works
with the verified byte maths. The game reaches an addon's behaviour through `game/ae2x/mod.rs` alone (its tick, grid node, block
state, menus, screen, item uses, part models and `/ae2` commands). One exception: a variant's screen is a layout of AE2's
own screen code, so `game/ae2_screens.rs` takes the addon's layouts as a child module (`addon_layout`). Next tables, as their addons need them: CPU blocks (bytes,
threads), wireless terminals (AE2WTLib's `WTDefinition`: item, menu,
hotkey, upgrade slots) and key types.
