# Hostile Neural Networks on this fork

A port of **Hostile Neural Networks 6.5.1** (NeoForge 1.21.1), the version All the Mods 10 ships, to this fork of
Horizon. It is the rules crate in this folder (`hostilenetworks-core`) and the native bundle that drives it in the game
(`horizon-mixin.toml`, `hooks/`, `runtime/`). `pack/scripts/hostilenetworks.ts` builds and tests them.

## Source and licence

- Source: <https://github.com/Shadows-of-Fire/Hostile-Neural-Networks>, branch `1.21`, commit
  `73979e2d4ee7094eafad5220efff5ec61bdbb7ed` ("6.5.1", `version=6.5.1` in `gradle.properties`).
- Licence: MIT, Copyright (c) 2021 Brennan Ward (`LICENSE` of the repository at that commit). `LICENSE-hostilenetworks`
  is its text. Its library Placebo (also MIT) was not needed: the rules here are the mod's own.

The Rust code here was **translated from that source** (`ModelTier`, `ModelTierRegistry`, `DataModelInstance`,
`EntityDataModel`, `SimChamberTileEntity`, `LootFabTileEntity`, `FabSelection`, `HostileConfig`), not written from the
mod's behaviour alone, and it is licensed under the same MIT. The source is cloned outside this repository
(`atm10-work/mod-sources/hostilenetworks`) and never committed.

No asset or file of the mod is committed: blocks, items, models, textures, language files, tiers and data models come
from the player's own jar through `horizon-port-pack carry`; ATM10's 17 data models, which KubeJS generates, are laid
over that carry by `final-data` (docs/atm10-port.md, "Pack logic").

## What is ported

`hostilenetworks-core`, with unit tests: the five tiers and a model's own required-data and data-gained overrides,
the tier of a model's data and the accuracy between tiers, the prediction roll, the Simulation Chamber's cycle in
inference (240 ticks, loot and predictions) and training (300 ticks, 20% more FE, one point of data on the model), its
failures and redstone modes, and the Loot Fabricator (60 ticks and 256 FE a tick a drop, fixed and queue selections,
its menu buttons), and `view.rs`, the numbers the two screens carry. The bundle (`runtime/hostilenetworks_world.rs`,
`hostilenetworks_ui.rs`, `hooks/`) is the glue: the registry of tiers and data models read from the game's data, the
machines' block entities and containers, the item components (`hostilenetworks:data_model`, `:data`, `:iterations`,
registered through `native_component_types`), Forge Energy, the menu buttons, and both screens drawn from the mod's
`gui/sim_chamber`, `gui/loot_fabricator` and `gui/default_gui` textures. `tests/chamber.ts` runs a chamber and a
fabricator in a world. `NOTES.md` lists what is left.
