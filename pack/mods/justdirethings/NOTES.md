# Just Dire Things port: state

Worktree `port5`, branch `port/justdirethings`, on `merge/s3-bundles` (`ee513eb225`). Source and licence: see README.md.

## How to run

```
source /d/VSCODE_PROJECTS/DEV_1/atm10-work/env.sh; bash /d/VSCODE_PROJECTS/DEV_1/atm10-work/wait-slot.sh
JUSTDIRETHINGS_CARRY=<carry> bun run pack/scripts/justdirethings.ts units content stage build world
```

`units` runs the rules crate's tests. `content` makes the content mod from the carry: the Generator T1 and the Block
Breaker T1 go from the diamond block template (no block entity, no menu) onto `minecraft:furnace` (block entity, menu,
`facing`, `lit`), as Hostile Neural Networks' machines do (the breaker keeps its own `active`); the goo blocks keep their template, since a declared block entity type is made for
any block. `stage` applies this bundle alone (and `HORIZON_LOCAL_BUNDLES`), `build` builds `-p horizon-game` with
`ae2-lab`, and `world` runs `tests/breaker.ts`, `tests/generator.ts` and `tests/goo.ts` (`JUSTDIRETHINGS_TESTS=goo` picks one;
`run.sh` takes it as `TESTS=goo`). On this PC,
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

## What is left (slice 2 and on)

See `atm10-work/jdt-work/HANDOFF.md` for the next slice. In short: the goo soils, the Paradox Machine (Chapter 2's task),
the time crystals and their budding block, the fluid generator and the fuels, the other T1/T2 machines (block placer,
clicker, sensor, dropper, fluid placer and collector, item collector, block swapper; the T2 breaker), the energy transmitter, the
tools, armour and their abilities, the Fuel Canister and the Pocket Generator, the portal gun and the wands.
