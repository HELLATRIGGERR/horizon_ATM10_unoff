# Iron Furnaces port: state

Worktree `fluids`, branch `port/ironfurnaces`, on 94858a4ead. Source: see README.md.

## How to run

```
source /d/VSCODE_PROJECTS/DEV_1/atm10-work/env.sh; bash /d/VSCODE_PROJECTS/DEV_1/atm10-work/wait-slot.sh
bun run pack/scripts/ironfurnaces.ts units content stage build world
```

`content` makes the content mod from the carry (`IRONFURNACES_CARRY`, default `atm10-work/fullpack/carries/ironfurnaces`):
the carry's furnaces are on the carved pumpkin template, which has no right-click, so each goes on `minecraft:furnace`
(its block entity, menu and `facing`; `lit` is the template's). `world` wants that content mod (`IRONFURNACES_CONTENT_DIR`), `ATM10_DIR`,
`AE2_CONTENT_DIR`, `AE2_RESOURCES` and `AE2_TEST_STARTUP_MS=600000`.

## Differences from Java

- The container is nine slots in a nine-slot menu (input, fuel, output, the three augments, three unused); the
  screen shows the first three. Java has its own menu type and an augment page.
- The input slot takes any item (the hook says only whether an item is fuel); Java asks for a recipe.
- A hopper reaches the default side configuration only (top input and fuel, bottom output, sides fuel): the side
  buttons, redstone modes and the copy tool are not ported. A bucket can't be taken out of the fuel slot by a hopper.
- Augments go in by a sneaking right click with the item (a plain click opens the screen); Java's augment screen
  and a plain click are not ported. The blue augments (factory, generator) are not ported.
- Upgrades work on Iron Furnaces' own furnaces; the vanilla furnace to Iron or Copper is not, since the engine's
  furnace is not a mod block entity. The upgraded furnace keeps its facing (Java faces it towards the player).
- The experience a recipe stored in the furnace, the Rainbow furnace's network and owner, the Wireless Energy Heater
  and the heater item are not ported.
