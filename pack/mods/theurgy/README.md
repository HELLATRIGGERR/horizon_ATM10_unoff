# Theurgy on this fork

A port of **Theurgy 1.78.0** (`theurgy-1.21.1-neoforge-1.78.0.jar`, the version All the Mods 10 ships) to
this fork of Horizon. It is the native bundle in this folder (`horizon-mixin.toml`, `hooks/`, `runtime/`) and
a crate of pure rules (`theurgy-core`). `pack/scripts/theurgy.ts` builds and tests them;
`docs/atm10-port.md` says where the work stands.

## What it covers

The core spagyrics processing chain:
- the **pyromantic brazier** (the heat provider: burns fuel, lights when lit, heats the apparatus above);
- the **calcination oven** (item in, alchemical salt out);
- the **liquefaction cauldron** (item + solvent fluid in, alchemical sulfur out);
- the **distiller** (item in, mercury shard out);
- the **incubator** and its three vessels (mercury, salt and sulfur vessels beside it, result out);
- the **sal ammoniac accumulator** and **sal ammoniac tank** (water evaporated into sal ammoniac fluid).

Each apparatus has a block entity, item slots (hoppers work), fluid handlers (cauldron, accumulator, tank),
right-click interactions, and the two-block apparatus' placed-by and shape logic. Heat comes from a lit brazier
below, looked at every 20 ticks.

652 of Theurgy's 1,734 recipes have a machine that runs them: 86 calcination, 43 distillation, 297
liquefaction, 222 incubation, 4 accumulation.

## Credits and licence

Theurgy is by **klikli-dev** and its contributors: <https://github.com/klikli-dev/theurgy>. Its code is MIT
(Copyright 2022 klikli-dev, `SPDX-License-Identifier: MIT` in every source file; `LICENSES/MIT.txt` in the
repository at the tag `release/v1.21.1-1.78.0`). Its assets are CC-BY-4.0. `LICENSE-theurgy` in this folder is
the MIT licence with its copyright notice.

The code here is translated from Theurgy's own source at that tag: the crafting behaviour
(`CraftingBehaviour`, `CachedCheck`), the heat provider and consumer (`PyromanticBrazierBlockEntity`,
`HeatConsumerBehaviour`), the recipes (`CalcinationRecipe`, `DistillationRecipe`, `LiquefactionRecipe`,
`IncubationRecipe`, `AccumulationRecipe`), the fluid tank (`OneTankFluidHandlerBehaviour`), the item handlers
(`TwoSlotItemHandlerBehaviour`, `OneSlotItemHandlerBehaviour`), the incubator's multiblock check and the
apparatus blocks' `useItemOn` / `setPlacedBy` / `updateShape`. It is MIT like the original, under the notice in
`LICENSE-theurgy`.

No file of Theurgy is committed: no code, textures, models, sounds or lang files. Its blocks, items, fluids,
recipes and assets come from the carry of the player's own jar (`horizon-port-pack carry`), loaded with
`--dev-mod`. This bundle adds their behaviour.
