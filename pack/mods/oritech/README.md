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
`Geometry`, `OritechConfig`, and for the fluids `SimpleFluidStorage`, `SimpleInOutFluidStorage`, `CentrifugeBlockEntity`'s
fluid mode, `RefineryBlockEntity`, `RefineryModuleBlockEntity`, `CoolerBlockEntity`, `FluidMultiblockGeneratorBlockEntity`,
`FuelGeneratorEntity`, `LavaGeneratorEntity`), not written from the mod's behaviour alone. The source is cloned outside this repository
(`atm10-work/mod-sources/oritech`) and never committed.

No asset or file of the mod is committed: blocks, items, models, textures, language files and recipes come from the
player's own jar through `horizon-port-pack carry`. Some of Oritech's assets are under other licences than its code
(CC BY-NC 4.0, CC BY 4.0; see the mod's README); none is copied.

## Fluids

Tanks hold a fluid name and an amount in millibuckets, saved as the `FluidStack` codec under Java's keys (`fluidin` and
`fluidout` of the centrifuge, `fluidinmain` and `fluidoutmain` of the refinery, `fluid` of the rest) and offered to other
mods through the engine's fluid handler hooks (`fluid_handler_tanks`, `_fill`, `_drain`). A fluid container used on a machine
fills from its tanks or empties into them (`MachineBlock.useItemOn`'s `tryFluidBlockItemInteraction`; the bundle names its
machines to the engine's `fluid_interact_on_use`, which moves the fluid as NeoForge's `FluidUtil.interactWithFluidHandler` does,
through the side clicked, with the bucket's sound, the container swapped in survival and kept in creative). A fluid centrifuge (a
`machine_fluid_addon` beside it), the refinery with its modules, the cooler and the fuel and lava generators run in
`oritech-core/src/fluid_machine.rs` and `generator.rs`. Where this differs from Java:

- a refinery module keeps its own tank (Java's `nodeA` and `nodeB` live in the refinery), so a module taken away takes its fluid;
- the cooler has no cold-biome bonus, and refuses to add to a slot holding another item;
- a container used on a machine that moves no fluid (a full tank, an empty bucket on an empty one) goes on to the menu, and
  the transfer plays the fluid's bucket sound (Java's interaction ends the click whatever moved, silently, and moves up to
  eight buckets of a container at once);
- the combi addon, the steam boiler addon and the steam engine are not ported, and no pipe moves fluid: buckets, hoppers
  of other mods and AE2's buses do, through the handler;
- the machine items are GeckoLib models the engine cannot draw, so the title panel of a screen shows the furnace icon of the
  block template in its place.
