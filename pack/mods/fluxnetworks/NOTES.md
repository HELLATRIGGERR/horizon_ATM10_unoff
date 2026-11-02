# Flux Networks port: state and next steps

Worktree `wt-flux`, branch `feat/fluxnetworks`. Plan: `.build/energy-mods-plan.md` sections 3 and 4-8. Java source for
reference: `refs/Flux-Networks-45cf248` (8.0.0, code MIT).

Run with content and targets on C:

```
export FLUX_CONTENT_DIR=C:/horizon-work/content-flux FLUX_STAGE=C:/horizon-work/stage-flux \
       FLUX_TARGET=C:/horizon-work/target-flux-game FLUX_TOOL_TARGET=C:/horizon-work/target-flux FLUX_JOBS=3
export ATM10_DIR='C:/Users/mpoka/curseforge/minecraft/Instances/All the Mods 10 - ATM10' AE2_TEST_STARTUP_MS=600000 \
       AE2_CONTENT_DIR='E:\PROJECTS\horizon-ae2\horizon\content\ae2' AE2_RESOURCES='E:\PROJECTS\horizon-ae2\horizon\.local\resources\26.3'
bun run pack/scripts/fluxnetworks.ts content units stage build world
```

`Control timeout: status` during startup means the machine is overloaded: run it again.

## State (2026-11-02)

F0 and F1 are done. The bundle needs hooks API 82 (Forge Energy, block entity drop).

- F0: the `content` step carries the jar: 135 files, 4 recipes moved in, 8 custom recipes kept but not craftable.
- F1: one network, plug to point, on the chest menu. `units`: 42 tests pass. `world` (`tests/plug-point.ts`): passes on
  ATM10's `fluxnetworks-server.toml`, 20 marks, among them, in FE a tick into an AE2 Energy Acceptor 20 blocks from the
  plug: 8 (one debug generator), 4096 (with three beside it), plug limit 500, point limit 300, limit 0 stops; two points
  on a plug of 1000: priority 0 and 1000, surge 1000 and 0, surge with limit 300 gives 300 and 700, bypass 1000 and 0;
  network, settings and flow after a restart (1200 before and after); 12345 FE in a plug's buffer survive a restart and
  arrive exactly once; disconnect from the menu stops the flow.

## Differences from the Java mod

- The chest menu stands in for the GUI, on purpose: it has no password entry, so encrypted networks that aren't yours
  aren't listed; no editing of names, colours, members, priority or limits (the world test sets those with `/data`).
- The menu opens at the end of the tick of the click: inside the click the player's state is swapped in and the server
  has no hub to open a menu with. Several players may have one device's menu open (no "occupied" message).
- A device looks for FE neighbours every 8 ticks and at its first tick, where the mod reacts to `neighborChanged` at
  once: a new neighbour connects up to 8 ticks late. There is a `neighbor_changed` hook, not used yet.
- No super admins, no wireless, no unloaded devices listed in the saved network (`connections`), no statistics.
- A device in a chunk that doesn't tick leaves its network (Java: until the chunk unloads).
- Without a config file the block blacklist is empty; the mod's default has `actuallyadditions:block_phantom_energyface`
  (ATM10's file has it and is read).
- Blocks and items are untinted: the network colour, the storage bar and the off textures wait for F6. The item model
  is the block's centre, not NeoForge's composite with six arms.
- A broken device drops a plain item: its settings and buffer are lost until F2.
- Devices work in other dimensions on paper (the glue goes by dimension), but only one was tested.

## Next: F2

- Flux dust in the world: `left_click_block` hook, `native_item_entities` and `native_set_item_entity` exist.
- Storages (rules, FE, the cycle's storage branch; `Kind` has only plug and point now).
- Crafting hook for special recipes with a mod-computed result (wipe, storage upgrade): missing in the engine.
- Block entity data on drops (settings, energy; the content step strips the loot modifiers today) and back onto the
  block when placed: `destroy_block` and `place_block` hear it, but the engine has no way to add components to a drop.
- A way to edit priority and limit in the chest menu would make F1 usable in survival without `/data`.
