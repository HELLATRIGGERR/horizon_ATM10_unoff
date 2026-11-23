# Just Dire Things on this fork

A port of **Just Dire Things 1.5.7** (NeoForge 1.21.1), the version All the Mods 10 ships (`justdirethings-1.5.7.jar`),
to this fork of Horizon. It is the rules crate in this folder (`justdirethings-core`) and the native bundle that drives
it in the game (`horizon-mixin.toml`, `hooks/`, `runtime/`). `pack/scripts/justdirethings.ts` builds and tests them.

## Source and licence

- Source: <https://github.com/Direwolf20-MC/JustDireThings>, tag `v1.5.7`, commit
  `8390e0effa33cd0e0995265245baf5629b61ef22` (`mod_version=1.5.7` in `gradle.properties`).
- Licence: MIT, Copyright (c) 2023 Direwolf20-MC (`LICENSE.txt` of the repository at that tag; the jar's
  `neoforge.mods.toml` says `license="MIT"` too). `LICENSE-justdirethings` is its text.

The Rust code here was **translated from that source** (`GooBlock_Base`, `GooBlockBE_Base`, `GooBlockBE_Tier1..4`,
`GooSpreadRecipe`, `GooSpreadRecipeTag`, `GeneratorT1BE`, `BlockBreakerT1BE`, `BaseMachineBE`, `RedstoneControlledBE`,
`RedstoneControlData`, `Config`, `GeneratorT1Screen`, `BlockBreakerT1Screen`, `BaseMachineScreen`, `FluidDropRecipe`,
`EntityEvents`, `TimeCrystalBuddingBlock`, `TimeCrystalCluster`, `ParadoxMachineBE`, `ParadoxMachine`,
`ParadoxMachineScreen`, `AreaAffectingBE`, `AreaAffectingData`, `BlockPlacerT1BE`, `BlockPlacerT1Screen`,
`FluidCollectorT1BE`, `FluidCollectorT1Screen`, `ItemCollectorBE`, `ItemCollectorScreen`, `FilterableBE`, `FilterData`),
not written from the mod's behaviour alone, and it is licensed under the same MIT. The source is cloned outside this repository
(`atm10-work/mod-sources/justdirethings`) and never committed.

No asset or file of the mod is committed: blocks, items, models, textures, language files, recipes and tags come from
the player's own jar through `horizon-port-pack carry`. The screen's text is the bundle's own English.

## What is ported

- **Goo blocks**, the four tiers (`gooblock_tier1` to `gooblock_tier4`): a living goo converts the block on each of
  its sides that a spreading recipe names, from the world's recipe tables (`justdirethings:goospread`, a block state,
  then `justdirethings:goospread_tag`, a block tag; ATM10 adds its own through KubeJS, which the carry holds), for a
  goo of at least the recipe's tier. A side counts down the recipe's duration by the tier's step (1, 2, 5, 10 a tick)
  and its block becomes the output, facing away from the goo; a block that stops matching cancels its side. After each
  conversion the goo may die (`goo_death_chance`, 10%), with the vex's death sound; a dead goo starts nothing until
  an item of its tier's revive tag (`justdirethings:goo_revive_tier_1` to `_4`) is used on it. The block entity keeps
  Java's `sideCounters` and `sideDurations`.
- **Generator T1**: one fuel slot (fuel only), a 1,000,000 FE buffer filled at 15 FE a burn tick, burning 4 times faster
  than a furnace (the mod's coals and coal blocks 2, 4, 8 and 16 times more), going on into the next fuel within the
  tick, offering ten times its 1,000 FE a tick to each of its six neighbours, giving energy to cables that draw it and
  taking none. Redstone control (ignored, low, high, pulse) from its screen's button. Its screen is drawn from the mod's
  own panel, slot, power bar and button textures, with the energy and burn tooltips.
- **Block Breaker T1**: one tool slot; on its operation clock (every 20 ticks) and where its redstone mode lets it, it
  starts on the block in front of it (not air, not a liquid, not unbreakable) and mines it as Java's fake player: the
  tool's destroy speed (with Efficiency) over the block's hardness, over 30 for a tool correct for the block's drops
  and 100 for any other, a share a tick, with the crack the players near it see; at 1 the block breaks with the
  tool's drops and the tool takes a point of wear. A block that changes, a tool taken out or the signal lost drops the
  progress. Its `active` state follows the redstone; its screen has the tool slot (the mod's pickaxe outline while
  empty) and the redstone button.
- **Fluid drop** (`justdirethings:fluiddrop`, the world's recipes, ATM10's KubeJS ones included): an item entity of a
  recipe's catalyst in a source block of the recipe's input fluid turns it into the output and is one item less.
  Polymorphic catalyst in water makes polymorphic fluid; a time crystal in polymorphic fluid makes time fluid. The
  fluids, their buckets and their flow are the carry's.
- **Time crystal**: a living tier 4 goo turns budding amethyst (or a time crystal block) into the time crystal budding
  block (the goo recipes `budding_time_*`). A random tick moves its `stage` on by its dimension (to 1 outside the
  Nether and the End, 2 in the Nether, 3 in the End, or the config's lists); at stage 3 it grows time crystal clusters
  facing away from it, small to full size, one random tick in five, and each growth has a 5% chance to send it back to
  stage 0. Budding amethyst's own growth does not run on it.
- **Paradox Machine**: a 10,000,000 FE buffer and a 16,000 mB tank of time fluid (fluid pipes and energy cables fill it; a
  bucket held on it does not yet, see NOTES); its area (the block in front by default, radius and offset buttons) is snapshotted on
  its screen's button: the blocks of the tag `justdirethings:paradox_allow` and the living entities (no player, none
  of `justdirethings:paradox_deny` or `c:teleporting_not_supported`), with their data. On its redstone (pulse by
  default) it restores what is missing since, over ten ticks a block or entity, for 250,000 FE and 50 mB each, and
  gains 0.25 paradox energy each. Its screen has the energy, fluid and paradox bars, the area buttons, the target
  (blocks, entities or both), render, snapshot, redstone and tick speed buttons.
- **Block Placer T1**: one slot; on its clock (20 ticks by default) it places the block item of its slot in front of
  it where the block there can be replaced, as its fake player would, clicking the face its direction button names
  (down by default: a log stands). Screen: the slot, the redstone, direction and tick speed buttons.
- **Fluid Collector T1**: on its clock it takes the source block of a fluid in front of it into its 8,000 mB tank a
  bucket at a time (the tank holds one fluid), and fills the fluid container in its slot from the tank. Fluid pipes
  reach the tank. Screen: the slot, the fluid bar, the redstone and tick speed buttons.
- **Item Collector**: on its clock it takes the items lying in its area (the block behind it by default; radius and
  offset buttons) into the inventory it faces, as far as they fit, through its nine filter slots (a denylist by
  default; allowlist and compare-NBT buttons) and its respect-pickup-delay setting. Screen: the area, filter, redstone,
  particles, pickup delay, render area and tick speed buttons.
- The settings come from `config/justdirethings-server.toml` (`goo`, `generator_t1`, `time_crystal`,
  `paradox_machine`, `minimum_machine_tick_speed` is 1).

`justdirethings-core` holds the rules with unit tests; `tests/goo.ts`, `tests/generator.ts`, `tests/breaker.ts`,
`tests/fluid.ts`, `tests/crystal.ts`, `tests/paradox.ts`, `tests/placer.ts`, `tests/collector.ts` and
`tests/itemcollector.ts` run them in a world.
`NOTES.md` lists the differences from Java and what is left.

## On upstream's hooks (merge of nightly-source/2026-11-18)

Hooks, all on main (no new fork hook): `world_lifecycle` v2, `server_started`, `block_entity_types` (the fork's compat
hook: one type per goo tier and per machine, named after the block), `block_entity`, `block_entity_ticks`,
`block_entity_tick`, `block_entity_container`, `block_entity_slot_accepts`, `fe_storage`, `fe_extract` (a
`MODIFY_ARG` on the call into `fire_fe_extract`), `fe_receive`, `menu_button`, `use_on` v2 (a `HEAD` injection reads the call with
the game, a `MODIFY_ARG` one answers it), `use_item_on`, `screen`, `screen_draw`, `screen_click`, `screen_tooltip`,
`entity_tick_listened`, `entity_tick_post` (the fluid drop), `random_tick` (the budding block) and
`fluid_handler_tanks`, `fluid_handler_fill` and `fluid_handler_drain` (the Paradox Machine's and the Fluid
Collector's tanks).
