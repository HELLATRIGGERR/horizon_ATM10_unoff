# Iron Furnaces on this fork

A port of **Iron Furnaces 4.3.2** (NeoForge 1.21.1), the version All the Mods 10 ships, to this fork of Horizon. It is
the native bundle in this folder (`horizon-mixin.toml`, `hooks/`, `runtime/`) and a crate of pure rules
(`ironfurnaces-core`). `pack/scripts/ironfurnaces.ts` builds and tests them.

## Source and licence

- Source: <https://github.com/Qelifern/IronFurnacesNeoForge>, branch `4.3.2`, commit
  `41fde020636b79154e1c454a893962c639d89e1b` (26 September 2025). The older repository
  `Qelifern/IronFurnaces` holds the 1.20.1 versions only, under GPL-3.0; nothing here comes from it.
- Licence: Apache License 2.0 (`LICENSE.txt` and `README-LICENSE.md` of the repository at that commit;
  `mod_license=Apache License Version 2.0`). `LICENSE-ironfurnaces` is its text and `NOTICE-ironfurnaces` its NOTICE.
- Authors: **Qelifern (pizzaatime)** and **XenoMustache**; the files carry "Copyright 2025 pizzaatime and XenoMustache".

The Rust code here was **translated from that source** (the behaviour of `BlockIronFurnaceTileBase`, `Config`,
`ItemUpgrade*` and `BlockIronFurnaceScreenBase`), not written from the mod's behaviour alone, and it is licensed under
the same Apache-2.0. The source is cloned outside this repository (`atm10-work/mod-sources/ironfurnaces`) and never
committed.

No asset or file of the mod is committed: blocks, items, models, textures, language files, recipes and loot tables
come from the player's own jar through `horizon-port-pack carry`, loaded with `--dev-mod` after `pack/scripts/ironfurnaces.ts content`
puts each furnace on the furnace template (a local copy, never committed).

## What is ported

Cooking only: the thirteen furnaces (copper, iron, silver, gold, diamond, emerald, crystal, obsidian, netherite, the
Rainbow furnace, Allthemodium, Vibranium, Unobtainium), their cooking time and fuel use, the modded tiers' and the
Rainbow furnace's many items per cooking, the speed, fuel, blasting and smoking augments, the sixteen tier upgrades,
and the furnace's screen. `NOTES.md` lists what is left.

The numbers are `config/ironfurnaces-common.toml` (ATM10's file, which `pack/scripts/sync-atm10.ts` copies), falling
back on the mod's defaults. Rules live in `ironfurnaces-core` with unit tests; nothing there depends on the engine.

## On upstream's hooks (merge of nightly-source/2026-11-18)

- Settings are read at `server_started` and dropped at `world_lifecycle`'s close phases.
- `block_entity` answers the compound only (the block's declaration owns the type id); the tick hands the API's
  `mod_api::ModBlockEntity`, whose slots hold `mod_api::ItemStack`s: the furnace works on the engine's stacks and
  writes back the slots it changed. The fuel's remainder comes from `native_crafting_remainder`.
- The fork's `use_item_on` (where `ItemStack.useOn` starts) is upstream's `use_on` v2, one `UseOnCall`: a HEAD
  injection reads it with the game and does the upgrade or the augment (on the server only), a `MODIFY_ARG` one
  answers `Success` (`use_on_answer`). A tier upgrade sets the new block through `level_set_block`.
- `screen` is `mod_api::Screen`; `screen_draw`'s batch is `draw`; sprites take a tint (`None`).
- Engine needs (HANDOFF "S3"): `use_on` is asked only while a part listens (`has_part_hook(UseOn)`, N5), so the
  upgrades and augments do nothing in a world yet.
