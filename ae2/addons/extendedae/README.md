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

- The devices themselves come from Crystal Assembler recipes (`assembler/ex_*.json`), which wait for that machine
  in slice 2 (with the Concurrent Processor). Until then they come from commands or a creative player.
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

## Later slices (`.build/ae2-addons-plan.md` §6.1)

2. Extended IO Port and buses (x8 in ATM10), Inscriber, Charger, Molecular Assembler; Crystal Assembler, Circuit
   Slicer, Crystal Fixer and their recipe types; Entro.
3. Assembler Matrix (max size 6 in ATM10).
4. Wireless Connector and Hub (range 1,000).
5. Special buses, emitters and planes.
6. Infinity and Void cells (ATM10: 256M components, 8 AE/t).
7. Oversize (x16) and Vacuum interfaces, modifiers, ME Packing Tape, Extended Pattern Access Terminal.
