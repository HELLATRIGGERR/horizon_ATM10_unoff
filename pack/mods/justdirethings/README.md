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
`FluidCollectorT1BE`, `FluidCollectorT1Screen`, `ItemCollectorBE`, `ItemCollectorScreen`, `FilterableBE`, `FilterData`,
`DropperT1BE`, `DropperT1Screen`, `FluidPlacerT1BE`, `SensorT1BE`, `SensorT1`, `SensorT1Screen`, `GeneratorFluidT1BE`,
`GeneratorFluidT1Screen`, `RefinedFuel`, `BlockSwapperT1BE`, `BlockSwapperT1Screen`, `FerricoreWrench`, `GooTier`, `ArmorTiers`,
`ClickerT1BE`, `ClickerT2BE`, `ClickerT1Screen`, `ClickerT2Screen`, `FakePlayerUtil`, `UsefulFakePlayer`, `PoweredMachineBE`,
`BlockPlacerT2BE`, `BlockBreakerT2BE`, `DropperT2BE`, `SensorT2BE`, `FluidPlacerT2BE`, `FluidCollectorT2BE`, `BlockSwapperT2BE`, `EnergyTransmitterBE`, `TransmitterEnergyStorage`, `EnergyTransmitterScreen`),
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
- **Dropper T1**: one slot; on its clock (20 ticks by default) or on a pulse it throws `dropCount` (1 to 64) of its slot's
  item as one item entity from the centre of the block in front of it, 0.3 blocks a tick the way its direction button
  points (down by default), with its pickup delay. Screen: the slot, the redstone, direction, tick speed, count and
  delay buttons.
- **Fluid Placer T1**: one slot and an 8,000 mB tank. Each tick the slot's full bucket (or a mod's container) fills the
  tank with its fluid (the tank holds one fluid) and the bucket comes back empty; on its clock it places a bucket of the
  tank's fluid as a source block in front of it where the block can be replaced and is no liquid source. Water in the
  Nether hisses away. Fluid pipes reach the tank. Screen: the slot, the fluid bar, the redstone and tick speed buttons.
- **Sensor T1**: one filter slot; on its clock (20 ticks) it counts what is in the block in front of it (blocks, air,
  hostile mobs, passive mobs, adult or baby animals, players, any living entity or items, by the target button) that the
  filter lets through (a denylist by default; the allowlist button) and gives 15 on every side but its front while there
  is more than none. Its strong signal button makes the signal reach through solid blocks. The filter is an item for a
  block (a bucket for a liquid) and a spawn egg for an entity.
- **Fluid Generator T1**: one slot and a 4,000 mB tank of refined fuel only (tiers 2, 3 and 4: 450, 1,300 and 4,000 FE
  a mB; `fuel_tier*_fe_per_mb`), burning a millibucket a tick while its 5,000,000 FE buffer has room for all of it, and
  offering ten times its 5,000 FE a tick to each neighbour. Redstone control from its button. Screen: the slot, the
  energy and fluid bars, the redstone button.
- **Block Swapper T1** (`blockswappert1`): two swappers bound with a Ferricore Wrench (a click on each: a machine passes the wrench's click on to it)
  trade the block in front of each (and, by the entity button, the hostile, passive, adult, baby, player, living, item or
  all entities standing in that block) on a pulse, or its clock on ignore, where both are in the same dimension; the
  blocks that the tags `c:relocation_not_supported` and `justdirethings:swapperdeny` name and unbreakable ones stay.
  Screen: the partner's icon, the blocks, entity and redstone buttons.
- **Clicker T1** (`clickert1`): one slot; on its clock (20 ticks) or a pulse the game's fake player (`native_fake_use`,
  a player of its own, `[JustDiresFake]`) stands just inside the block in front, on the side its direction button names
  (down by default: it looks down at the block's top), holds the slot's stack and clicks: a right click (the block's
  and the item's use, then the item's own use: a bucket takes the water in front), a left click (the block hit as a
  player's first hit, which breaks what one hit breaks; an entity attacked at full strength) or a held use (held the
  hold button's ticks, then let go; the tick speed stays above them). The target button picks the block, the air
  (the block past it within reach, else the item's use in the air), or the hostile, passive, adult, baby, player or
  living creatures in the block in front, found together and clicked one a run; sneaking from its button. The stack the
  click leaves goes back in the slot; what else the fake holds (a filled bucket from a stack of empty ones) drops.
  Blocks of the tag `justdirethings:no_auto_click` are left. Screen: the slot, the show-fake-player, sneak, click type,
  target, hold ticks, redstone, direction and tick speed buttons.
- **T2 machines** share a 100,000 FE buffer (cables fill it from any side), an area (the block in front by default;
  radius and offset buttons) and nine filter slots (a denylist by default; allowlist and compare buttons), in a 9x2
  chest menu whose slot 0 is the machine's; their screens add the energy bar left of the panel.
  - **Clicker T2**: the Clicker T1 over its area (places nearest first, creatures in the area), what the filter lets
    through (a block's item, a creature's spawn egg), 250 FE a click that consumed.
  - **Block Placer T2**: the Placer T1's placement over its area nearest first, where the filter lets the block it is
    placed against through, 500 FE a placement.
  - **Block Breaker T2**: on a run it tracks every block of its area to break (not air, liquids or unbreakable ones; the
    filter on the block's item), nearest first, and mines the first at its tool's speed as the T1 does; the next waits
    for the following run. 500 FE a break, a point of the tool's wear for each.
  - **Dropper T2**: nine slots and nine filter slots in the 9x2 menu; on a run it lists the first slot with an item (with a
    filter: the first slot of each filter item, all of which must be there) and drops one listed slot's `dropCount` items
    each operation, at the area's offset, 25 FE a drop. Its screen has the offset buttons only and the compare button.
  - **Sensor T2**: the Sensor T1 over its area (every block of it counted, or the entities in it), the filter's nine slots
    for the blocks' items or the spawn eggs, the count test and amount buttons; each sense pays 2 FE a block of the area (all
    the buffer when it is short, and then it senses nothing).
  - **Fluid Placer T2** and **Fluid Collector T2**: the T1s with a 32,000 mB tank, their places found together over the
    area nearest first and one used each run, 500 FE a bucket; the placer's filter reads the block next to the place in its
    direction setting, the collector's the fluid's bucket.
  - **Block Swapper T2**: two bound T2 swappers trade the blocks of their areas (nearest first, the same offsets from each
    one's starting point, its area's offset), the entities of the area, what the filter lets through, and not what both
    areas hold; it needs 50 FE for each block of the area to run, and takes 50 FE for each block (both sides count) and
    entity moved. A radius button's change goes to the partner, and so does the radius of the swapper that was bound last.
- **Energy Transmitter**: a 1,000,000 FE buffer (the config's `energy_transmitter_t1` values), an area that starts as the
  block above it, nine filter slots, a redstone button and a show particles button (no particles are drawn). Every 50
  ticks it scans its area, nearest first, for blocks with a block entity that take Forge Energy and pass the filter and for
  other transmitters; each tick, while the network of transmitters it sees holds energy, it sends each block up to 1,000 FE
  less 1% a block of Manhattan distance (rounded down), taken from its own energy, then balances the network (every
  transmitter the average, the remainder one more each in order). What it holds, takes and gives out in a call is the
  network's, shared among its transmitters.
- **Tool and armour tiers**: ferricore, blazegold, celestigem and eclipse alloy tools, armour and the wrench are items of the
  carry (their tool rules, durability and repair, equipment assets). The carry holds no attribute modifiers for
  celestigem's and eclipse alloy's, so the content step adds the mod's own: `GooTier`'s damage bonus (celestigem 4,
  eclipse alloy 5) over each tool's base damage and speed as `createAttributes` gives them, and `ArmorTiers`' armour points,
  toughness (2 and 3) and eclipse alloy's 0.1 knockback resistance. Their abilities are not ported.
- The settings come from `config/justdirethings-server.toml` (`goo`, `generator_t1`, `time_crystal`,
  `paradox_machine`, `minimum_machine_tick_speed` is 1).

`justdirethings-core` holds the rules with unit tests; `tests/goo.ts`, `tests/generator.ts`, `tests/breaker.ts`,
`tests/fluid.ts`, `tests/crystal.ts`, `tests/paradox.ts`, `tests/placer.ts`, `tests/collector.ts`,
`tests/itemcollector.ts`, `tests/dropper.ts`, `tests/fluidplacer.ts`, `tests/sensor.ts`, `tests/fluidgen.ts`, `tests/swapper.ts`,
`tests/tools.ts`, `tests/clicker.ts` and `tests/t2.ts` run them in a world.
`NOTES.md` lists the differences from Java and what is left.

## On upstream's hooks (merge of nightly-source/2026-11-18)

Hooks, all on main (no new fork hook): `world_lifecycle` v2, `server_started`, `block_entity_types` (the fork's compat
hook: one type per goo tier and per machine, named after the block), `block_entity`, `block_entity_ticks`,
`block_entity_tick`, `block_entity_container`, `block_entity_slot_accepts`, `fe_storage`, `fe_extract` (a
`MODIFY_ARG` on the call into `fire_fe_extract`), `fe_receive`, `menu_button`, `use_on` v2 (a `HEAD` injection reads the call with
the game, a `MODIFY_ARG` one answers it), `use_item_on` (the Paradox Machine's buckets), `use_without_item` (the machines pass the
Ferricore Wrench and the Machine Settings Copier on to the item), `screen`, `screen_draw`, `screen_click`, `screen_tooltip`,
`entity_tick_listened`, `entity_tick_post` (the fluid drop), `random_tick` (the budding block) and
`fluid_handler_tanks`, `fluid_handler_fill` and `fluid_handler_drain` (the tanks of the Paradox Machine, the Fluid
Collector, the Fluid Placer and the Fluid Generator), `block_signal` and `is_signal_source` (the Sensor's redstone). The Clicker's
clicks are the engine's `native_fake_use` (`docs/atm10-port.md`, "A click on a mod block"), a helper, not a hook.
