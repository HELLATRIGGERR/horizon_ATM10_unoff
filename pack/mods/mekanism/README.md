# Mekanism on this fork

Mekanism's energy blocks and Mekanism Generators' Solar and Heat Generators for All the Mods 10, as a native mod of
this fork of Horizon. It is the native bundle in this folder (`horizon-mixin.toml`, `hooks/`, `runtime/`), a crate of
pure rules (`mekanism-energy`) and a content tool (`mekanism-content`). `pack/scripts/mekanism.ts` builds and tests
them; `NOTES.md` says where the work stands.

## Credits and licence

Mekanism and Mekanism Generators are MIT licensed, Copyright (c) 2017-2025 Aidan C. Brady
(<https://github.com/mekanism/Mekanism>). The rules in `mekanism-energy` are ported from Mekanism 10.7.19.

No file of Mekanism is committed. Models, textures, recipes and loot tables are generated locally from the player's own
jars by `mekanism-content`, with our block declarations in `content/` over them: two content mods, `mekanism` and
`mekanismgenerators`. The heat generator's gauge draws the game's own lava texture, copied from the player's client
assets when `mekanism-content` is given them.

## What there is

- Basic, Advanced, Elite, Ultimate and Creative Energy Cubes: Joules in the block entity, side configuration and
  ejection, the screen, placement facing, an item that keeps its energy.
- Settings from ATM10's `config/Mekanism/` (`general.toml`, `tiers.toml`, `common.toml`, `generators.toml`,
  `generator-storage.toml`), read again when a file changes.
- Forge Energy at a cube's edge (hook points API 154): each side as an `IEnergyStorage`, and the ejector feeding any block
  with Forge Energy, such as AE2's Energy Acceptor, at `feConversionRate` Joules to the FE.
- Basic, Advanced, Elite and Ultimate Universal Cables (block entity lifecycle hook points, API 156): networks with one
  buffer as big as their cables together, kept in the bundle per dimension; they connect to cubes and to any block with
  Forge Energy, emit to all of them each tick, pull through a pull side, and keep their shares across saves, restarts
  and chunks unloading. Their OBJ model becomes a multipart blockstate in `mekanism-content`.
- The Solar Generator: Joules from the sun by its biome (the helpers `native_biome` and `native_dimension_type` of hook
  points API 157), the time of day and the weather, Minecraft 1.21.1's maths for the sun as Mekanism reads it on ATM10,
  out of its bottom, at ATM10's `solarGeneration = 150` (`generators.toml`) and 96 000 J (`generator-storage.toml`).
- The Heat Generator: a lava tank (24 000 mB) filled from lava buckets and furnace fuel in its fuel slot (`burnTime / 20`
  mB; a hopper puts only fuel there), heat from burning 10 mB a tick (800), from lava on each face (30) and in an
  ultra-warm dimension (100), a heat capacitor that loses heat to the biome's air and turns it into Joules at the Carnot
  efficiency, out of its front; `facing` and `active` (light 13) in its block, `active` going off after
  `blockDeactivationDelay`.
- Both generators: Forge Energy on their energy face, cables connecting there, a screen like Mekanism's
  (`GuiSolarGenerator`, `GuiHeatGenerator`), and `/mek probe` answers for tests.

## Differences from Mekanism

- A network's acceptors are sorted by position; Mekanism goes in its `HashMap`'s order. Only who gets a leftover Joule differs.
- A new or loaded cable joins at the end of its first tick, so what it takes starts a tick later than Mekanism's own buffer does.
- A connection to a block with energy that appears with no block changing (an AE2 part, a cube's data) is noticed within a
  second. Cubes and blocks being placed or removed are noticed at once.
- A chunk's block entities load or unload with the chunk, by Horizon's own rules, and not Mekanism's "accessible" ticket level.
- Until Mekanism's Configurator and redstone sensitivity are ported, `/mek connection <pos> <side> <type>` sets a side.
- A cable's outline and collision are its core: no arms until content blocks can have several boxes by several properties.
- Cables can't be waterlogged: their six sides use all of a block's 4096 states.
- A generator's peak (solar) or air (heat) is read on its first tick after it is placed or loaded, as Mekanism does; the
  biome's temperature is 26.3's (cooler above the dimension's sea level + 17), which in the overworld is 1.21.1's
  (above y 80).
- The sky's darkening that hides the sun (`skyDarken`) is computed from the same day time as its brightness; Java
  computes it a tick earlier, so at dusk and dawn the solar generator may stop or start a tick apart.
- The solar generator's outline and collision are one box, its panel's height (half a block); its block has no
  `facing` or `active`, which Mekanism's model doesn't show. The heat generator is a full block.
- Lava beside the heat generator is vanilla's lava block; mod fluids tagged lava, lava-logging, pipes into its tank and
  fluid items other than the lava bucket wait for mod fluids. No heat goes to a heat handler below (none is ported).
- Hoppers take nothing out of the heat generator's fuel slot (an empty bucket stays until a player takes it): a hook
  point for what a slot gives is missing, and coal must stay. Its energy slots charge nothing until items carry energy.
- Generators drop plain: their energy, lava and heat stay behind, as the cube did before its items kept energy.
