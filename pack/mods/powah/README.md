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

## On upstream's hooks (merge of nightly-source/2026-11-18)

- Settings are read at `server_started` and dropped at `world_lifecycle`'s close phases.
- `block_entity` answers the compound only (the block's declaration owns the type id); the hooks hand the API's
  `mod_api::ModBlockEntity` (`id()`, its fields through `Deref`), whose slots hold `mod_api::ItemStack`s: the fuel
  is shrunk through the API and its burn time and remainder asked of the engine's stack.
- `fe_receive` and `fe_extract` pass one `*Call`: `MODIFY_ARG` injections on the call into `fire_*`.
- `place_block` -> `on_block_placed_by` (a HEAD injection reading the call); `perform_command` -> `run_mod_command`
  (`/powah [line:text]`, operators).
- Engine needs (HANDOFF "S3"): `on_block_placed_by` fires only for items a trait mod's part listens on (N5), so a
  placed Furnator does not take its item's energy back yet. `pack/scripts/powah.ts stage` stages every bundle under
  `pack/mods`, so it passes only once Mekanism is on upstream's hooks too.
