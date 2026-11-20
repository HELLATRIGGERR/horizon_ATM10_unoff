# Mystical Agriculture on this fork

A port of **Mystical Agriculture 1.21.1-8.0.29** and the parts of its library **Cucumber 1.21.1-8.0.16** it needs, the
versions All the Mods 10 ships, to this fork of Horizon. It is the native bundle in this folder (`horizon-mixin.toml`,
`hooks/`, `runtime/`) over a crate of pure rules (`ma-core`). `pack/scripts/mysticalagriculture.ts` builds and tests
them; the state of the port is in `docs/atm10-port.md`, "Mystical Agriculture".

## Credits and licence

Both mods are by **BlakeBr0** and licensed under the MIT licence at those versions:

- Mystical Agriculture: <https://github.com/BlakeBr0/MysticalAgriculture>, branch `1.21`, commit `f38a0928`
  ("Release 8.0.29"). Its licence is `LICENSE-mysticalagriculture` here.
- Cucumber: <https://github.com/BlakeBr0/Cucumber>, branch `1.2!` (its 1.21 branch), commit `11904c08`
  ("Release 8.0.16"). Its licence is `LICENSE-cucumber` here.

`ma-core` and `runtime/ma_world.rs` are translated from their Java source to Rust (the crops, tiers and drops of
`MysticalCropBlock`, `InferiumCropBlock`, `Crop`, `CropTier` and `ModCrops`; `GrowthAcceleratorBlock`;
`InfusionAltarTileEntity`, `InfusionRecipe` and the altar's and pedestals' `useItemOn`; Cucumber's
`MultiblockPositions` and inventory rules), under the same MIT licence and with the copyright notices above.

No file of either mod is committed: blocks, items, textures, models, sounds, lang, recipes and loot tables come from
the player's own jar through `horizon-port-pack carry`. The sources are read from a clone outside the repository.

## On upstream's hooks (merge of nightly-source/2026-11-18)

- Settings and the pack's MysticalCustomization crops are read at `server_started` and dropped at `world_lifecycle`'s
  close phases (v2).
- `block_entity` answers the compound only (the block's declaration owns the type id). `block_entity_tick` hands the
  API's `mod_api::ModBlockEntity`, whose slots hold `mod_api::ItemStack`s: the altar reads them as the engine's stacks
  and writes its remainder and result back through the API. The pedestals are the levels' (engine) block entities.
- `loot_drops` hands `mod_api::LootDrops`: the broken crop's block and `age` are read from its API state through the
  block metadata. Remainders are `crafting_remainder` (the item's data, then the mods' hook).
- A Growth Accelerator draws from the level's random through `level_random` and stores it back (`level_set_random`).
- `block_entity_types`: the altar, pedestals and growth accelerators, one type per block (the compat hook E2).
- `right_click_block` on its `BlockClick`: a click on the altar or a pedestal cancels with `Success`.
- `right_click_block` is one of the hooks upstream calls only while something listens on it; the altar's and pedestals'
  clicks need the engine's N5 (HANDOFF "S3": the gates open for hooks a staged bundle injects into).
- `register_load_conditions`: MA's recipe conditions (`crop_enabled`, `crop_has_material`, `augment_enabled`;
  `ma_core::conditions`) are registered under its namespace; the registrar names the owner from the native mod
  dispatch scope, which the bundle opens itself (`enter_native_mod`). Upstream refuses a data load whose `recipe/`
  files carry unknown conditions, so without them the world does not open. The engine answers the conditions of an
  owner that is not a trait mod `false` (HANDOFF "S3", N10): those recipe files are left out, as fork main left them.
- The carry's seeds name their crop with upstream's item field `block` (the fork's `place_block` before the merge):
  a carry made before the merge needs carrying again.
- `pack/scripts/mysticalagriculture.ts stage` applies this bundle alone (plus `HORIZON_LOCAL_BUNDLES`) through
  `pack/scripts/hook-catalog.ts`; `build` builds `-p horizon-game` with `MA_FEATURES` (`ae2-lab` by default).
