# Modern Industrialization on this fork

A port of **Modern Industrialization 2.5.11**, the version All the Mods 10 ships (`Modern-Industrialization-2.5.11.jar`),
to this fork of Horizon. It is translated from MI's own source code at that version:
<https://github.com/AztechMC/Modern-Industrialization>, tag `2.5.11` (commit `2a5cce7`).

The bundle is the native mod in this folder (`horizon-mixin.toml`, `hooks/`, `runtime/`) and a crate of pure rules
(`mi-core`). `pack/scripts/modern_industrialization.ts` tests, stages and builds them; `docs/atm10-port.md` says where
the work stands.

## Credits and licence

Modern Industrialization is by **Azercoco** and **Technici4n**, with its contributors, and its code is under the MIT
licence: `LICENSE-modern-industrialization` here is the `LICENSE` of the repository at tag 2.5.11, with its copyright
notice (Copyright (c) 2020 Azercoco & Technici4n). The Rust in this folder translates MI's Java and keeps that notice.

No file of MI is committed: no source, no jar, no texture, model, sound or lang file. MI's blocks, items, recipes,
loot tables, textures and GUI art come from the carry of the player's own jar (`horizon-port-pack carry`, or the full
pack's `final-data`), loaded as a `--dev-mod`. MI's source is read from a clone outside the repository
(`atm10-work/mod-sources/modern-industrialization`).

## What is ported

- `mi-core/recipe.rs`: `MachineRecipe`'s codec and `MachineRecipeType.validateRecipe`, and the type's per-item cache.
- `mi-core/crafter.rs`: `CrafterComponent`, the processing loop of every crafting machine: recipe start, the
  efficiency ramp (`getRecipeMaxEu`, `getRecipeMaxEfficiencyTicks`), inputs and chance inputs, outputs with machine
  locks, overlapping recipes, the 100-tick recheck of an unchanged inventory.
- `mi-core/energy.rs`: `EnergyComponent` and the Forge Energy bridge (`EnergyApi`, `forgeEnergyPerEu`).
- `mi-core/machines.rs`: the electric macerator, compressor and cutting machine of `SingleBlockCraftingMachines`
  (slots, bars, `MachineTier.LV`).
- `runtime/mi_world.rs`, `runtime/mi_ui.rs`: the block entity, its slots and saved compound, the world's recipes,
  Forge Energy, `/mi probe`, and the screen as `MachineScreen` draws it.

## On upstream's hooks (merge of nightly-source/2026-11-18)

- `forgeEnergyPerEu` is read at `server_started`; it and the recipe books are dropped at `world_lifecycle`'s close
  phases (v2).
- `block_entity_types`: the electric machines, one type per block (the compat hook E2).
- `block_entity` answers the compound only (the block's declaration owns the type id). The hooks hand the API's
  `mod_api::ModBlockEntity`, whose slots hold `mod_api::ItemStack`s: the crafter reads them as the engine's stacks and
  writes back the slots it changed (a stack that only changed count keeps its components).
- `fe_receive` and `fe_extract` pass one `*Call`: `MODIFY_ARG` injections on the call into `fire_*`.
- `perform_command` -> `run_mod_command` (`/mi [line:text]`, operators: the line is rebuilt from the invocation, as
  upstream's `word` takes no `~`); `screen` -> `mod_api::Screen`, `screen_draw`'s batch is `draw`, sprites take a
  tint (none).
- `pack/scripts/modern_industrialization.ts stage` applies through `pack/scripts/hook-catalog.ts` (`MI_ONLY=1`: this
  bundle alone, plus `HORIZON_LOCAL_BUNDLES`); `build` builds `-p horizon-game` with `MI_FEATURES` (`ae2-lab` by
  default).
