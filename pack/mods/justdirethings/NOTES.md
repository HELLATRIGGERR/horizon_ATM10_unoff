# Just Dire Things port: state

Worktree `port5`, branch `port/justdirethings`, on `merge/s3-bundles` (`ee513eb225`). Source and licence: see README.md.

## How to run

```
source /d/VSCODE_PROJECTS/DEV_1/atm10-work/env.sh; bash /d/VSCODE_PROJECTS/DEV_1/atm10-work/wait-slot.sh
JUSTDIRETHINGS_CARRY=<carry> bun run pack/scripts/justdirethings.ts units content stage build world
```

`units` runs the rules crate's tests. `content` makes the content mod from the carry: the Generator T1, the Block
Breaker T1, the Block Placer T1, the Fluid Collector T1, the Item Collector, the Dropper T1, the Fluid Placer T1, the
Sensor T1, the Fluid Generator T1 and the Paradox Machine go from the diamond
block template (no block entity, no menu) onto `minecraft:furnace` (block entity, menu, `facing`, `lit`), as Hostile
Neural Networks' machines do (the breaker keeps its own `active`); the goo blocks keep their template, since a declared block entity type is made for
any block. `stage` applies this bundle alone (and `HORIZON_LOCAL_BUNDLES`), `build` builds `-p horizon-game` with
`ae2-lab`, and `world` runs every test in `tests/` (`JUSTDIRETHINGS_TESTS=goo,fluid` picks some; `run.sh` takes it
as `TESTS=goo,fluid`). On this PC,
`atm10-work/jdt-work/run.sh <tag> <steps>` sets the paths: its own stage and target, upstream's pinned horizon-mixins,
the v5 resources and the empty content mod in AE2's place.

## What the bundle does

- The block entities keep Java's keys. Goo: `sideCounters` and `sideDurations` (lists of `{side, counter}` and
  `{side, duration}`, -1 where nothing runs). Generator: `burnRemaining`, `maxBurn`, `feRemaining`,
  `fuelBurnMultiplier`, `redstoneMode`, `pulsed`, `receivingRedstone`, `tickspeed`, `direction`; its energy is
  `energy` (Java keeps it in an attachment, `justdirethings:energystorage_generators`) and its fuel the container's slot
  0 (Java: the attachment `generator_item_handler`). Breaker: `redstoneMode`, `pulsed`, `receivingRedstone`,
  `tickspeed`, `sneaking`, `direction`, its tool the container's slot 0; the operation clock and the block being mined
  are not saved, as in Java (kept per thread by position).
- The recipe book is read from the world's tables (`native_recipes_of_type`, the carried types), once per thread and
  dropped when the server opens or closes its world. What each block state beside a goo converts by is cached per tier,
  as Java's `outputCache` is per goo.
- The goo's sides are read before the tick and the blocks of the sides it finishes written after its death rolls: a
  conversion changes only its own side, which no other side reads, and placing an ore draws nothing from the level's
  random.
- Breaker screen: the generator's frame, the tool slot at (80, 59) (`justslot`'s pickaxe outline while empty) and the
  redstone button; its data is the redstone mode and whether the slot holds a tool.
- Generator screen: 176x214 (Java's 176x166 menu with its top section 26 above and the title tab 20 above that: every y
  here is Java's plus 46), the fuel slot at (80, 59) on a hopper menu (its four other slots hidden and refused), the
  energy bar at (5, 25), the flame at (79, 76), the redstone button at (104, 58) (menu buttons 0 to 3 set the mode).
  The screen reads only the menu's data (`GeneratorView`, 15 bits a slot, two slots an amount).
- Fluid drop: the recipes are read from the world's tables once per thread (`justdirethings:fluiddrop`, in the
  table's order); the bundle hears every item entity's tick (`entity_tick_post`) and, for an item of a catalyst in a
  source of a recipe's input fluid, sets the output block, takes one item (the entity goes when it was the last) and
  plays the fire extinguish sound.
- Time crystal budding block: the `random_tick` hook answers its random tick (the stage, then at stage 3 the
  clusters, with the level's random in Java's order), so its template's own growth never runs.
- Paradox Machine: Java's keys (`redstoneMode`, `pulsed`, `receivingRedstone`, `tickspeed`, the area's
  `xRadiusDouble`.. `zOffset` and `renderArea`, `targetType`, `renderParadox`, `isRunning`, `timeRunning`,
  `fePerTick`, `fluidPerTick`, `paradoxEnergy`, `snapshotData` with `blocks` and `entities`,
  `restoringBlocks`, `restoringEntities`); its energy is `energy` and its fluid `timeFluid` (Java's attachments).
  Its screen is 256x214 (the top section 80 wider than the menu), with no slot of its own; menu buttons: 0 to 3 the
  redstone mode, 4 the snapshot, 5 render paradox, 6 to 8 the target, 9 render area, 10 to 21 the area's radius and
  offset of each axis up or down, 22 and 23 the tick speed. The screen's data is `ParadoxView`.
- Block Placer T1, Fluid Collector T1: Java's keys (`redstoneMode`, `pulsed`, `receivingRedstone`, `tickspeed`,
  `direction`; the collector's tank `Fluid` as `FluidTank` writes it), the slot the hopper menu's first; the
  operation clock is not saved (as in Java). The placer places through the world's placement
  (`Behaviours::place_block`, as the dispenser places a shulker box), with a fake player's look down or up for those
  directions. Menu buttons 4 to 9 set the placer's direction, 22 and 23 the tick speed.
- Item Collector: Java's keys (the clock's, the area's, `allowlist`, `compareNBT`, `blockitemfilter` -1,
  `respectPickupDelay`, `showParticles`); its nine filter slots are the 3x3 menu's, shown in a row at (8, 100), which
  no side reaches. It puts items into the inventory in front through the engine's hopper insertion
  (`native_insert_item`); menu buttons 0 to 3 redstone, 9 render area, 10 to 21 the area, 22 and 23 the tick speed, 24
  allowlist, 25 compare NBT, 26 respect pickup delay, 27 show particles.

- Dropper T1 (`justdirethings_more`): Java's keys (`dropCount`, `pickupDelay`, `direction`, the clock's); the one slot is the
  hopper menu's first. The item entity is made as `new ItemEntity(.., stack)` then given 0.3 of the direction as motion.
  Menu buttons 4 to 9 the direction, 22 and 23 the tick speed, 28 and 29 the count, 30 and 31 the delay.
- Fluid Placer T1: Java's keys (the clock's, `direction`, the tank `Fluid`); the bucket in its slot is read before the
  tick (one whole bucket at a time) and comes back empty; the block placed is the fluid's block (`..._source` becomes
  `..._block`) at level 0, set with neighbour updates and the bucket's sound. Its tank is the collector's 8,000 mB one,
  so the same fluid handler hooks serve it.
- Sensor T1: Java's keys (`senseTarget`, `strongSignal`, `senseAmount`, `equality`, `allowlist`, `compareNBT`,
  `blockStateProps`, the clock's); whether it emits is kept per thread by dimension and position (Java doesn't save it
  either) and the `block_signal` and `is_signal_source` hooks read it. A change tells the six neighbours and their
  neighbours (`neighborChanged`, `updateNeighborsAtExceptFromFacing`). Menu buttons 24 allowlist, 32 target (the next),
  33 strong or weak, 22 and 23 tick speed.
- Fluid Generator T1: Java's keys for the redstone and the tank (`Fluid`), `energy` for its buffer (an attachment in
  Java); its slot is the hopper menu's first, the tank is 4,000 mB of the refined fuels only, the buffer is
  `generator_fluid_t1_max_fe` and it pushes ten times `generator_fluid_t1_fe_per_tick` to each side, as the Generator T1.

## Differences from Java

- The goo's client side: the block entity renderer that shows the block being converted and the particles when a side
  finishes are not ported (the server's work and the sounds are).
- Redstone is read every tick; Java reads it again only after a neighbour changed (`checkedRedstone`), which gives the
  same signal except for a power change no neighbour update reports.
- The generator's buffer and slot live in the block entity's own compound, not in NeoForge attachments, so a world from
  Java doesn't carry them over. Breaking the generator keeps nothing on the item (Java's `isDefaultSettings` and item
  data are not ported).
- The breaker: its facing is the furnace template's (four sides; the carry already placed it facing away from the
  player's horizontal look, where Java takes the nearest of six), the tick speed button and the sneak button of its
  screen are not ported (the tick speed stays 20, `sneaking` saved but unused), and the place checks Java asks
  (`mayInteract`, the block-place event: spawn protection, claims) are not asked. The tool's wear goes through the engine's
  `hurtAndBreak` (`damage_living_stack`), which wears nothing while the acting player is in creative; Java's fake
  player is never creative, so the world test runs in survival.
- The Fuel Canister's multiplier (its contents') is 1 here: the canister isn't ported.
- The redstone button cycles from the mode the menu's data shows; Java's button keeps its own mode on the client, so
  two clicks faster than the data comes back (a tick or two) set the same mode twice here.
- The screen's text is the bundle's own English, not the mod's language file; the fuel slot's tooltip line with the
  burn speed multiplier is not shown.
- The first plain recipe in the table's order wins where two name the same block state; Java takes the recipe manager's
  order.
- Fluid drop: a fluid the output's fluid type vaporizes where it lands (Java's `isVaporizedOnPlacement`, water in the
  Nether) stays; Java turns it to air with its vaporize effect.
- Paradox Machine: a bucket held on it does not fill or empty the tank: the engine calls `use_item_on` only when a trait
  mod's Part listens to it, so a native bundle's mixin on it never runs (the world test sets `timeFluid` in the saved
  compound). Fluid pipes use the tank handler hooks and work. The way out is a `listened` gate for it, as
  `entity_tick_listened`, or the item-side `use_on` v2 as Iron Furnaces uses it.
- Paradox Machine: the paradox itself (the entity a full paradox energy spawns) is not ported, so paradox energy stays
  at its most once full; the area and paradox renders (`renderArea`, `renderParadox`) are saved but not drawn.
  The fluid in its screen's bar is drawn in a plain green tint.
- Machines' tick speed: the minimum is the config's default (1); `minimum_machine_tick_speed` is not read. Shift and
  control on the number buttons (10 and 64 at a time) are not ported.
- Block Placer T1: the place checks of Java's fake player (`mayInteract`, the block-place event) are not asked, and
  its item components are not applied to the placed block entity (a named or filled block item places a plain block).
- Fluid Collector T1: the screen's fluid is the water's or lava's still texture where the GUI atlas holds it, else the mod's
  fluid texture in the fluid's tint (blue for water, orange for lava).
- Item Collector: the filter slots hold real items (Java's are ghost slots that copy one of the stack the cursor
  holds and keep nothing), so a filter item is put in and taken out as in any container and drops when the block is
  broken; the flow particles (`showParticles`) and the area render are not drawn; items go in as a hopper puts them
  (into the first slot that takes them, one at a time), where Java's `insertItemStacked` fills partial stacks first.

- Dropper T1: the pickup delay and count buttons step by 1 (Java's shift and control steps are not ported).
- Fluid Placer T1: a container that holds less than a bucket, or one that gives less than the tank would take, is not drained in
  parts (a bucket is whole or nothing as in Java; a mod's canister would give what it holds); the place checks of the
  fake player (`canBreakAndPlaceAt`: spawn protection, claims) are not asked; the vaporizing of water in the Nether is
  the bucket's hiss with no smoke; the Fluid Placer's screen text is the bundle's English.
- Sensor T1: the block state filter (`blockStateProps`, the T1's scrolling list of properties) is not ported (the key is saved
  empty): the filter slot matches a block by its item's name, a liquid by its bucket's; an entity by its spawn egg (the
  creature catcher is not ported); the count test (`senseAmount`, `equality`) is the T2's, kept at 0 and more-than; the area is
  the one block in front. Compare NBT has no button.
- Fluid Generator T1: a bucket in its slot is drained whole (4,000 mB holds four buckets); the fluid bars of the Fluid
  Placer and the Fluid Generator are drawn as the Fluid Collector's.
- Block Swapper T1: both swappers must be in the dimension the game simulates (the engine writes blocks only in the level in
  place; a swapper bound across dimensions reads as missing); a block with a block entity (a chest, a machine) is not
  swapped (no engine helper moves a block entity's contents between positions; Java moves them with their NBT); the
  swap writes both states with neighbour and client updates (flags 3), where Java writes with 51 and then validates the
  blocks (`canSurvive`, `updateFromNeighbourShapes`); no particles; the bind is by a sneaking click of the wrench (the
  swapper's menu answers a plain click before the item does, and `use_item_on` is gated, see the Paradox note), the
  first swapper clicked is remembered by the game (not on the wrench: no foil, no tooltip) and the messages Java shows
  ("Bound to", "Binding Removed") are not shown; the entity filter is by kind only (the T2's filter slots are not ported).
- Tools and armour: celestigem's and eclipse alloy's attribute modifiers come from the content step (the carry has none);
  every ability (the toggles, FE, the paxel's, ore scanning, the hammer, flight...), the Pocket Generator, Fuel and
  Fluid Canisters, the wands and the bows are items the carry declares as plain items: they exist and do nothing special.

## What is left

See `atm10-work/jdt-work/HANDOFF.md` for the next slice. In short: the Clicker T1 (it needs an engine helper for a
fake player's clicks: the use of the held item on a block or entity in front of it, and left or right click with a hold,
through the same path as a player's `use_item_on`, which the engine gates on a trait mod's Part), the Block Swapper's
cross-dimension swap and block entities, the T2 machines (area, filter slots and a 100,000 FE buffer on each; the Block
Placer T2 and Block Breaker T2 are the nearest), the goo soils, the paradox entity and the time wand (an item with fluid and energy, and an entity that
accelerates block entity ticks), the energy transmitter, the tools, armour and their abilities, the Fuel Canister and the
Pocket Generator, the portal gun and the wands.
