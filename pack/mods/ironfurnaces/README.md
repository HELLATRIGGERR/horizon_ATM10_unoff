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
