# Oritech on this fork

A port of **Oritech 1.2.13** (NeoForge 1.21.1), the version All the Mods 10 ships, to this fork of Horizon: the rules
crate in this folder (`oritech-core`) and the native bundle that drives it in the game (`horizon-mixin.toml`,
`hooks/`, `runtime/`). `pack/scripts/oritech.ts` builds and tests them.

## Source and licence

- Source: <https://github.com/Rearth/Oritech>, tag `v1.2.13`, commit `4069b6dd14b05dfe0c80126618739b64054630ea`
  ("Add 1.2.13 changelog", `version = 1.2.13` in `gradle.properties`).
- Licence: CC0-1.0 (`LICENSE.md` of the repository at that commit, and `license = "CC0-1.0"` in the jar's
  `neoforge.mods.toml`). `LICENSE-oritech` keeps that notice and text.

The Rust code here was **translated from that source** (`MachineBlockEntity`, `UpgradableMachineBlockEntity`,
`MultiblockMachineEntity`, `MultiblockMachineController`, `PulverizerBlockEntity`, `FragmentForgeBlockEntity`,
`AssemblerBlockEntity`, `FoundryBlockEntity`, `OritechRecipe`, `OritechRecipeType`, `DynamicEnergyStorage`,
`Geometry`, `OritechConfig`), not written from the mod's behaviour alone. The source is cloned outside this repository
(`atm10-work/mod-sources/oritech`) and never committed.

No asset or file of the mod is committed: blocks, items, models, textures, language files and recipes come from the
player's own jar through `horizon-port-pack carry`. Some of Oritech's assets are under other licences than its code
(CC BY-NC 4.0, CC BY 4.0; see the mod's README); none is copied.
