# ExtendedAE 1.21-2.2.42

[ExtendedAE](https://github.com/GlodBlock/ExtendedAE) by GlodBlock, models and textures by Sea_Kerman and
Ridanisaurus, as All the Mods 10 8.4 ships it. Code LGPL-3.0 (`LICENSE.txt`; 2.2.42 has no tag, its source is
commit `1f86f927` on branch `1.21.1-neoforge`), so the port's derived files carry the same terms; the port's code is written from the mod's behaviour, not copied.
The licence of the assets is not stated; they are generated locally from the player's jar and never
committed. Provenance in [SOURCE_MANIFEST.json](SOURCE_MANIFEST.json), the declared content in
[content.json](content.json).

Feature `ae2-extendedae`; content `content/extendedae` from
`ATM10_DIR=... bun run ae2/scripts/prepare-addon.ts extendedae`. ATM10's `config/extendedae-common.toml` keeps
the mod's defaults; ATM10's KubeJS scripts change nothing in this slice.

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

## Slice 2: Crystal Assembler, Circuit Slicer, Crystal Fixer and their recipe types; Extended IO Port

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
| `EAERegistryHandler.registerUpgrades` | `Defs::add_upgrade` rows for each device, which `ae2_machine_ui::machine_upgrade_allowed` now reads for every addon machine |
| materials: the concurrent processor, its print and press, the machine frame, entro crystal, dust and ingot, entro and silicon blocks | plain items and blocks of `content.json`, with their crafting, inscriber and transform recipes |

Checked locally against ATM10's final recipes (a NeoForge run of the whole pack): each Crystal Assembler, Circuit
Slicer and Crystal Fixer recipe this port carries is there under its id, identical in every field. The pack's
other recipes of these types need devices of later slices or come from other mods; the full pack's carries
bring those.

Differences, known:

- The output side screen (`OutputSideConfig`) is not drawn; the sides are kept and set by the window's entries 14
  to 19, and a Memory Card does not copy them (`EXTRA_SETTING`).
- A recipe search goes in the recipes' load order; Java's in its recipe manager's hash order. Only inputs that
  two recipes both accept can tell.
- The tank fills from water and lava buckets in the window only: the port has no fluid pipes, and AE2's export bus
  and pattern provider do not push fluids into it yet.
- The Extended Import and Export Buses, Inscriber, Charger and Molecular Assembler are not carried yet (their
  Crystal Assembler recipes wait with them). The buses' card rows and part models are in place.
- Entro's budding blocks, clusters and seeds (and the fixer's entro recipes) come with a later slice.

Tests: `horizon-client` (the rows, sizes, cards and IO budget in `ae2x::extendedae`; recipe matching, the search
context, the executor's steps and energy, fixer recipes and output sides in `game::ae2x::extendedae::machines`)
and the world test `extendedae-crystal`:

```sh
AE2_TEST_STARTUP_MS=600000 bun run ae2/scripts/test-world.ts extendedae-crystal
```

## Later slices (`.build/ae2-addons-plan.md` §6.1)

2. (the rest) Extended Import and Export Buses (x8 in ATM10), Inscriber, Charger, Molecular Assembler; Entro's
   budding blocks, clusters and seeds.
3. Assembler Matrix (max size 6 in ATM10).
4. Wireless Connector and Hub (range 1,000).
5. Special buses, emitters and planes.
6. Infinity and Void cells (ATM10: 256M components, 8 AE/t).
7. Oversize (x16) and Vacuum interfaces, modifiers, ME Packing Tape, Extended Pattern Access Terminal.
