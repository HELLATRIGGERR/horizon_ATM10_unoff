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

## Later slices (`.build/ae2-addons-plan.md` §5.1)

3. Superdense Energy Cell, Greater Energy Card.
4. Portable M cells (`CellConfig::portable` is ready; MEGA's type count is `18 + (index - 5) × 9`), Portable Cell Workbench.
5. MEGA Pattern Provider and Interface, block and part.
6. ME Cell Dock.
7. Bulk Item Cell, Compression Card, Decompression Module.
8. Cells for other addons' key types (chemical, radioactive, source, soul), with those addons.

`neoforge:mod_loaded` recipe conditions: not needed by slice 1 (its recipes have none); the 107
conditional recipes all belong to cells of other addons and are left out by the content script.
