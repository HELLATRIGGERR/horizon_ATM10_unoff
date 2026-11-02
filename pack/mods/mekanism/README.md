# Mekanism on this fork

Mekanism's energy blocks for All the Mods 10, as a native mod of this fork of Horizon. It is the native bundle in this
folder (`horizon-mixin.toml`, `hooks/`, `runtime/`), a crate of pure rules (`mekanism-energy`) and a content tool
(`mekanism-content`). `pack/scripts/mekanism.ts` builds and tests them; `NOTES.md` says where the work stands.

## Credits and licence

Mekanism is MIT licensed, Copyright (c) 2017-2025 Aidan C. Brady (<https://github.com/mekanism/Mekanism>). The rules
in `mekanism-energy` are ported from Mekanism 10.7.

No file of Mekanism is committed. Models, textures, recipes and loot tables are generated locally from the player's own
jar by `mekanism-content`, with our block declarations in `content/` over them.

## What there is

- Basic, Advanced, Elite, Ultimate and Creative Energy Cubes: Joules in the block entity, side configuration and
  ejection, the screen, placement facing, an item that keeps its energy.
- Settings from ATM10's `config/Mekanism/` (`general.toml`, `tiers.toml`, `common.toml`), read again when a file changes.
- Forge Energy at a cube's edge (hook points API 81): each side as an `IEnergyStorage`, and the ejector feeding any block
  with Forge Energy, such as AE2's Energy Acceptor, at `feConversionRate` Joules to the FE.
- Basic, Advanced, Elite and Ultimate Universal Cables (block entity lifecycle hook points, API 83): networks with one
  buffer as big as their cables together, kept in the bundle per dimension; they connect to cubes and to any block with
  Forge Energy, emit to all of them each tick, pull through a pull side, and keep their shares across saves, restarts
  and chunks unloading. Their OBJ model becomes a multipart blockstate in `mekanism-content`.

## Differences from Mekanism

- A network's acceptors are sorted by position; Mekanism goes in its `HashMap`'s order. Only who gets a leftover Joule differs.
- A new or loaded cable joins at the end of its first tick, so what it takes starts a tick later than Mekanism's own buffer does.
- A connection to a block with energy that appears with no block changing (an AE2 part, a cube's data) is noticed within a
  second. Cubes and blocks being placed or removed are noticed at once.
- A chunk's block entities load or unload with the chunk, by Horizon's own rules, and not Mekanism's "accessible" ticket level.
- Until Mekanism's Configurator and redstone sensitivity are ported, `/mek connection <pos> <side> <type>` sets a side.
- A cable's outline and collision are its core: no arms until content blocks can have several boxes by several properties.
- Cables can't be waterlogged: their six sides use all of a block's 4096 states.
