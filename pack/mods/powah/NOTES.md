# Powah port: state and next steps

Worktree `wt-powah`, branch `feat/powah`, on 85020d24a. Plan: `.build/energy-mods-plan.md` sections 2 and 4-8.

## How to run

```
source env.sh; bash .build/wait-slot.sh
POWAH_WORK=C:\horizon-work\powah\work POWAH_CONTENT_DIR=C:\horizon-work\powah\content \
POWAH_STAGE=C:\horizon-work\powah\stage POWAH_TARGET=C:\horizon-work\target-powah-game \
POWAH_TOOL_TARGET=C:\horizon-work\powah\target-tools POWAH_JOBS=3 \
bun run pack/scripts/powah.ts units stage build world
```

`world` also wants `ATM10_DIR`, `AE2_CONTENT_DIR`, `AE2_RESOURCES` and `AE2_TEST_STARTUP_MS=600000`.

The carry and its facts are generated in `C:\horizon-work\powah\work` and never committed: `fetch` and `carry` with the
port-pack exe in `.build/target-portpack`, facts from the oracle in `work/oracle/facts.json`, then `powah-content`.
It gives 106 blocks (92 carried, 14 declared by `powah-content`), 171 recipes (14 `powah:energizing` kept as data).

## The Java oracle

`C:\horizon-work\powah\oracle-run` is a NeoForge server with Powah; `oracle_start.py` starts it, `tr/t4.py`..`t6.py`
drive it over RCON with `tick freeze`/`tick step`. The last output is `tr/out-rerun.txt`. What it shows, all in the
crate's unit tests or `tests/furnator.ts`:

- Each tick: with no fuel burning one item goes, full or not; then the tier's generation, as much as fits and as the fuel
  has left; then each side in `Direction` order gets up to the transfer rate of what is stored.
- A starter with a stick (100 fuel ticks): 3,000 FE in 38 ticks, the last giving 40; the next item starts a tick later.
- From 19,900 of 20,000: makes 80, 20, then stops with `burning` 0; the fuel stays.
- Six sinks and 600 FE: down 240, up 240, north 120. Three sinks when full: 240 each.
- Nitro with coal: 20,000, 20,000, 8,000 FE. Lava bucket: 600,000 FE, the empty bucket stays in the slot.
- Dirt in the fuel slot stays and nothing burns. A hopper above puts in coal and not dirt; a hopper below takes from both slots.
- `setblock ... destroy`: 3 coal and a plain `furnator_starter`, no components.

From the jar's classes: `Block.playerDestroy` puts the tile's `writeStorable` (energy, `carbon_ticks`, `carbon_max_ticks`)
into `powah:stored_block_entity_state` as a compound; placing reads it back. The separate `powah:energy_stored`
(a long) is an item's own FE (batteries; slice P7), and it is what Java's item tooltip reads.

## Differences from Java

- A new block pushes on its first tick; Java's first tick after placing, or after a neighbour appears, pushes nothing.
- Block entity id per tier (`powah:furnator_starter`), Java's is `powah:furnator` with a `variant`; there is no
  migration from Java saves anyway.
- `lit` is a real block state here, picking a model, because block entity renderers don't exist yet. Java has no such
  property and draws the lit face with a renderer. It gives no light in either.
- The GUI has no side configuration or redstone buttons; every side extracts and redstone is ignored. The bolt
  appears at once; Java fades it in over 20 ticks.
- The menu is a hopper menu with three hidden slots, so vanilla clients see a hopper. Java's has its own menu type.
- The charge slot takes nothing until items hold energy (P7). Hoppers can still empty it.
- The item's tooltip shows the energy kept on it. Java's reads `powah:energy_stored` and would show 0.
- A placed block's energy is capped at the tier's capacity.

## Next: P2, energy cables

- Shapes chosen by six booleans. The content API chooses shapes by one property: extend it (plan, section 5).
- A block entity on a block whose template has none: cables are on `minecraft:stone` today (`powah-content`), so a
  block-entity template rule, or "removed on break" for any mod block entity, is needed.
- Shape updates (API 32) answering connections from `native_fe_info`.
- The network: rebuilt from the loaded cables, not saved. The Wrench's Config mode and block-entity NBT for side modes.
- Trace the oracle first: furnator, 10 starter cables, AE2 acceptor, and a branched net into two sinks.
- `native_block_entity_drop` and `native_block_entity_slot_accepts` (API 82) serve P3's cells as they are.

## State

- Passing: `powah.ts units` (powah-core 23, powah-content 6). Game builds at C:\horizon-work	arget-powah-game.
- Not yet run: the unit test added to `lan/menu.rs`; no world test has passed. `tests/furnator.ts` stopped at its first
  block because the content mod was refused (`sounds minecraft:metal is not a block`). That is fixed and the content
  regenerated, but no world has loaded it yet, so no screenshot exists.
- To find out next: `/powah probe` and `/mek probe` both answered "Unknown or incomplete command" in that refused
  world. Check whether that follows from the refusal. Then run `POWAH_TESTS` furnator and furnator-screen.
- A world start needs `wait-slot.sh 4`: at 2 GB free the harness failed with "Control timeout: status".
