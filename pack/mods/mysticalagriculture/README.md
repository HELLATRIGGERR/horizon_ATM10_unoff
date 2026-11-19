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
