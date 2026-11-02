# Powah on this fork

A port of **Powah! Rearchitected 6.2.10**, the version All the Mods 10 ships, to this fork of Horizon. It is the
native bundle in this folder (`horizon-mixin.toml`, `hooks/`, `runtime/`), a crate of pure rules (`powah-core`) and a
content tool (`powah-content`). `pack/scripts/powah.ts` builds and tests them; `NOTES.md` says where the work stands.

## Credits and licence

Powah is licensed under the GNU LGPL 3.0 (`LICENSE_Powah` in the jar, `license="LGPLv3"` in its `neoforge.mods.toml`).

- **owmii** wrote the original Powah and its Lollipop library.
- **Technici4n** and **shartte** rearchitected it for NeoForge 1.21.1: <https://github.com/Technici4n/Powah>.
- **Cyn** and **SammySemicolon** made the textures and models.

The code here is our own, written from Powah's behaviour on a Java server, its data, its manual and its config, and
it is LGPL-3.0-or-later like the fork's AE2 code. It stays in this fork and is never proposed to Horizon.

No file of Powah is committed. Blocks, items, textures, recipes and loot tables are generated locally from the
player's own jar: `horizon-port-pack carry`, then `powah-content`.

## Numbers

The game reads `config/powah.json5` (ATM10's file, which `pack/scripts/sync-atm10.ts` copies), and falls back on the
same numbers, which are the mod's defaults. Rules live in `powah-core` with unit tests; nothing there depends on the
engine.
