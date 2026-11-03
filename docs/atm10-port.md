# All the Mods 10 on Horizon: state and how to continue

This fork ports the All the Mods 10 modpack (ATM10 8.4, Minecraft 1.21.1, NeoForge 21.1.265) to Horizon. The goal right
now is the clean pack working as it does on Java, then faster than Java, with measurements. No mods outside the pack yet.
The mods belong to their authors: each port credits them in its `README.md` or `ae2/NOTICE.md`.

Start here on a new machine. Read "Setting up a machine", then "What to do next".

## Branches

| Branch | What it holds | State |
|---|---|---|
| `atm10-horizon-rework` (default) | Upstream merged (6e1635c1e), AE2, MEGA Cells, ME Requester, ExtendedAE's first slice, Mekanism (cubes, cables, Solar and Heat Generators), Powah, Flux Networks, `horizon-port-pack`, the full pack's start | Unit tests, the default world suites and the bundles' tests pass (3 November 2026). A world opens with all carries (below) |
| `wip/quests` | Quest book native mod, on 85020d24a | World test not passing yet. Its own screen hooks must move onto upstream's native mod screens (`Screen::Mod`, `screen_render_post`) |
| `wip/upstream-merge`, `int/atm10`, `wip/port-pack`, `wip/port-pack-2`, `wip/fullpack-rebased`, `wip/fullpack-start`, `wip/extendedae`, `wip/mekanism`, `wip/ae2-terminals`, `wip/ae2-screens`, `wip/merequester`, `wip/powah`, `wip/fluxnetworks` | Folded into the default branch | Delete when the owner agrees |
| `horizon-connect` | The owner's separate project | Leave alone |

## Setting up a machine

Windows 11 was used; 16 GB of RAM works with `CARGO_BUILD_JOBS=3` or 4, more is much better. Put the clone on an internal
NVMe drive: builds and worlds use many small files, and a USB drive is slow at those. Keep 40 GB free.

Tools: Git, Rust 1.99.1 with the MSVC build tools, Bun 1.4.4, JDK 25, Python 3.11 or later (`tomllib`). The NeoForge oracle
runs NeoForge 1.21.1, which needs Java 21: CurseForge installs a Temurin 21 under `Install\java\Jre_21`
(`HORIZON_ORACLE_JAVA`); JDK 25's `javac --release 21` builds the exporter.

Game files:

- Minecraft 26.3, installed by CurseForge or the launcher (a `.minecraft` layout), and its `server-26.3.jar`.
- The ATM10 instance, version 8.4, from CurseForge. The tools only read it. Never write to it.

```sh
git clone -b atm10-horizon-rework https://github.com/HELLATRIGGERR/horizon_connector.git horizon
cd horizon
git remote rename origin mine
git remote add origin https://github.com/horizon-rust/horizon.git
git fetch mine

bun run ae2/scripts/prepare-resources.ts --minecraft-dir "<.minecraft>" --server-jar "<server-26.3.jar>"
bun run ae2/scripts/prepare-content.ts
ATM10_DIR="<ATM10 instance>" bun run ae2/scripts/prepare-addon.ts megacells
ATM10_DIR="<ATM10 instance>" bun run ae2/scripts/prepare-addon.ts merequester
bun run ae2/scripts/build.ts
bun run ae2/scripts/play.ts --world "AE2 Test" --creative
```

Use `CARGO_INCREMENTAL=0` and `CARGO_PROFILE_FAST_DEBUG=0` to keep targets small.

## Testing

```sh
export AE2_TEST_STARTUP_MS=600000   # a fresh world can take minutes
bun run ae2/scripts/test-world.ts machine-screens provider-session
```

Optional settings: `AE2_TEST_BINARY` (a built `horizon-client.exe`), `AE2_CONTENT_DIR`, `AE2_RESOURCES`,
`AE2_ADDON_CONTENT_ROOT`.

Suites that must pass before anything lands:

```
machine-screens provider-session inscriber-menu-filter fe-partial-acceptance megacells-cells merequester-stock
portable-menu-lifecycle terminal-screen-views terminal-search terminal-craft-gestures wireless-key-types
wireless-source-components crafting-alternatives crafting-fluid-template inframe-terminal part-screens
machine-menu-identity mixed-cell-menus chest-fluid-gestures fake-slot-drag-machines emitter-fake-gestures
emitter-memory-components export-bus-crafting-schedule interface-stock-editor
```

Build for those with `--features ae2-lab,ae2-megacells,ae2-merequester`. Then the native mods, each with its own game build:

```sh
ATM10_DIR=... bun run pack/scripts/powah.ts carry content units stage build world         # see pack/mods/powah/NOTES.md
ATM10_DIR=... bun run pack/scripts/fluxnetworks.ts content units stage build world  # see pack/mods/fluxnetworks/NOTES.md
```

Also run `cargo test -p horizon-client --no-default-features --features ae2-lab --bin horizon-client` and `--lib`,
`cargo test -p horizon-client-lib --no-default-features --features ae2-lab --lib` (upstream split the client's library:
the game is in `horizon-client`'s, the rest, with AE2's library modules, in `horizon-client-lib`, which has the same
`ae2-*` features), `cargo check -p horizon-client` with default features and `cargo run -p xtask -- mod-api --check`.

Machine notes:

- World tests under heavy load fail with "Control timeout: status" or `ECONNRESET`. Rerun them on an idle machine before
  reading anything into it.
- If the disk fills, failures look like test bugs (`EUNKNOWN` on the control file). `.local/world-tests` grows fast:
  delete it between runs. Junctions under `.local/resources` must go with `cmd /c rmdir`.
- If a worktree is deleted, remove its `.local/resources` junction first.

## The full pack

A carry is one mod's content converted from its jar. Carries are generated on your machine and are never committed.
`tools/atm10/README.md` lists the settings the scripts read (instance, work folder, data folder, Java 21...).

1. `horizon-port-pack import <instance> <work>`, `inventory <work>`. `all-keys.txt` holds the 287 carried keys
   (`coverage-summary.py` names the groups); `tools/atm10/batches.py` cuts them into oracle batches.
2. `tools/atm10/oracle-pack.sh` (under `fullpack.lock`): the whole pack on NeoForge with KubeJS and Almost Unified
   (`neoforge_oracle.py pack`), 448 mods. It writes the final recipes, tags and loot tables, `pack-facts.json` (fuel,
   compost, worn slot, piston and light facts of every item and block), and each batch's facts (`--batches`,
   `facts-<batch>.json`, 1.5 GB in all): one server run, about 16 minutes at `-Xmx5G` on the 16 GB PC.
3. `tools/atm10/carry-with-facts.sh`: each batch carried with its facts (about 10 minutes for all).
4. `horizon-port-pack final-data <carries> --dump <dump> --work <work> --resources <26.3> --instance <ATM10> --also content/ae2`
   writes each namespace's data into the carry that owns it. The rest goes in `atm10-pack`, whose mod id is `kubejs`.
5. `tools/atm10/load-check` reads them as the game does; `tools/atm10/play-atm10.ts` passes each carry to the game as a
   `--dev-mod` (`measure-load.ps1` measures a start).

What ATM10 8.4 gives (2 November 2026): 287 carries, 40,557 blocks with 873,394 states (plus vanilla's and AE2's,
about 910,000 of `MAX_STATES`' 1,048,576), 14,218 items; the load check has 289 mods on and none refused. A block
whose jar has no blockstate for it is left out: the pack's run sees what other mods register in a namespace for their
own content (Bibliowoods' furniture in other mods' woods: 6,083 blocks and 270,000 states; Dyenamics and Friends',
Amendments'). Of the dump's 95,015 recipes, 78,649 are in carries and 8,746 in `atm10-pack`, 5,699 are parked (their
result is an item we don't have: Bibliocraft's furniture, Dyenamics and Friends', Powah's).

The Java baseline to beat, on the 16 GB laptop: 200 s from launch to in-world (140 s to the title, 58 s for the world),
96,202 recipes, about 8.3 GB heap.

Where the Horizon start stands: the start was neither slow nor stuck in one place: it failed, one fatal error after
another, each hidden behind the one before (first starts on this PC):

| Start | Content read | Root build | Title shown | Ended |
|---|--:|--:|--:|---|
| `wip/fullpack-start` (old laptop) | 52 s | 187 s | not reached | stopped at 28 min |
| run 1 | 3.5 s | 65 s | 145 s | the GUI atlas overflowed (79,198 textures, 286 M texels): the load fell back to vanilla's resources, and joined a world with no mod content |
| run 2 | 8.0 s | 81 s | 103 s | crash: a mod's own death message type (`productivemetalworks:foundry`) |
| run 3 | 9.0 s | 100 s | 131 s | crash: damage type tags naming types of mods not here |
| run 4 | 7.8 s | 70 s | 98 s | crash at 180 s: 33,380 block sprites, terrain vertices index 32,767 |
| run 5 | 10.5 s | 74 s | 107 s | the same with 33,242 (missing textures share a layer) |
| run 6 | 6.2 s | 44 s | 65 s | core tables ready at 120 s; the world's loaders took the game to 10 GB (a 1.3 GB JSON tree) and the memory floor stopped it at 128 s |
| run 7 | 8.7 s | 64 s | 92 s | core tables at 178 s (a minute reading item definitions one by one); the machine (7 GB free) ran out as the world loaded |
| run 8 | 10.1 s | 50 s | 75 s | core tables at 126 s (item definitions read on every thread); started with 4 GB free, stopped by the memory floor at 128 s as the world loaded |
| run 9 | 6.9 s | 43 s | 66 s | core tables at 107 s; the memory floor at 109 s: 1.3 GB read whole to hash it |
| run 12 | 5.8 s | 42 s | 59 s | tables one at a time: memory holds (peak 6.8 GB); crash at 226 s: 36,446 sprites, more than 8 arrays of 2,048 |
| run 19 | 7.3 s | 45 s | 69 s | **first world with all carries**: first frame at 275 s |
| run 21 | 5.6 s | 49 s | 68 s | a first start on the final code: first frame at 249 s, its data file written behind it in 273 s |
| run 22 | none | none | 6.5 s | a second start: first frame at **73 s** |

A world opens with all carries. All of these with the session locked, so no frame rate here compares with Java's (the
compositor holds the window to 30 fps); memory, tick and load times do. Against Java's 200 s, on the same PC:

| | First start (run 21) | Second start (run 22) |
|---|--:|--:|
| Content root built | 49 s | reused |
| Core tables | 108 s | 21 s |
| World tables | 178 s | 35 s |
| Renderer | 190 s | 46 s |
| Server's data | 216 s | 56 s |
| Generator, server's game | 237 s | 71 s |
| First frame in the world | 249 s | **73 s** |
| Peak working set | 6.7 GB | 6.9 GB |
| At spawn: working set, private (with 1.5 GB of GPU memory) | 5.0, 9.4 GB | 6.6, 9.1 GB |
| Lowest free memory, from 7-8 GB at the start | 0.2 GB | 1.0 GB |

The server thread's tick, over runs 18-22: 5.0-8.8 ms standing at spawn (peak 7.9-15.6 ms); 7.2-16.6 ms on a 400-block
flight at 10 blocks a second (peak 15-47 ms, and one 1.08 s spike in run 22 not looked into). 637 chunks, 322-1,911
block entities, 168-228 mobs. Java's ATM10 was not measured beside it: its 8.3 GB heap doesn't fit in what this PC leaves.

The fixes, in the engine: content mods' GUI and entity textures (Java binds those one by one when drawn) are optional in
the GUI atlas and fill only what room and the texture budget leave; a mod's own damage effects and death message types
read as vanilla's defaults; damage tags lose only the references that are missing; the content root's tag merge is no
longer quadratic (40 s of the build); every texture that doesn't load shares one layer; terrain vertices index sprite
layers with 32 bits. What opened the world, in the engine: terrain.wgsl grows past 8 sprite arrays, to 24 on a GPU that binds
them; a mod's growing plant takes its way of growing from its template, and a structure that doesn't decode is left out,
not the generator; a large registry (past 200,000 states) loads the world's tables and a world's opening one at a time;
the behaviour snapshot's fingerprint hashes files in pieces (it read 1.3 GB whole, side by side with everything else);
`model-states.json` stays text until a block is reached, and the behaviour document is read only on a snapshot miss;
6 million baked faces let go of their sprite names; a content root gets its own `horizon-data.bin`, written behind the
first start, so the next ones open one file for 600,000: that took the world from 172 s to 73 s. A piston that finished
moving also removed its own base (upstream's 5b2970e45, still on its main): fixed here. `HORIZON_STARTUP_TRACE` now
records memory at each step's end. On the way: 319 of the pack's placed features can't run yet and are skipped, and
`underground_villages:underground_village` is left out (its own height provider).

What holds the memory now, in MB, by `resource_memory`: the registry 260; baked block models about 1,250 (276,297
variants, 6.26 million faces at 128 bytes each); parsed models 360; recipes 656; items 221; block facts and behaviours
297. The GUI atlas is 640 MB of GPU memory. Baking models when a chunk first needs them would take the most off.

Known defects in the generated carries, from a review that read the files, and what the regenerated carries say
against Java's facts (`verify-defects.py` in the work folder; numbers for ATM10 8.4):

| Defect | Before | Now | Where |
|---|---|---|---|
| Items copy the carved pumpkin template: head slot, compost value; pistons destroy the blocks; items burn as fuel; wrong stack size | 5,233 items; 10,238 burn; 571 stack size | worn slot, fuel, piston reaction: 0 differ from Java; stack size compared with the template's own | *tool* `oracle.rs` |
| Lang files overwritten instead of merged | 46 files, 6,251 keys | merged key by key | *tool* `final_data.rs` |
| `no_occlusion` set but dampening stays 15 | 6,837 blocks | light dampening: 0 differ from Java | *tool* `oracle.rs` |
| Failed selectors leave the template's full cube | 1,220 blocks | 458 | *tool* `oracle.rs` |
| Empty-collision states dropped | 135 blocks | 117 blocks keep a collision Java hasn't | *tool* `oracle.rs` |
| The blockstate guess skips the shape check | 23 plates, 9 buttons, 58 fences | 0, 0, 9 | *tool* `oracle.rs` |
| Flat `light` on the `redstone_lamp` template | about 240 of 279 | 40 on the template, 2 flat | *tool* `oracle.rs` |
| A wrong `--dump` wipes recipes and tags; function tags deleted | | the dump is read through first; function tags kept | *tool* `final_data.rs` |
| Placement ignores declared properties | 909 + 711 blocks | rules by name, and `placement` where Java's own placement shows it | engine (a2dd15acd), *tool* |
| Mod loot entry types are skipped | up to 2,658 tables | vanilla's eight types; 1.21.1 tag entries upgraded | engine (a2dd15acd), *tool* |
| Minecarts ride vanilla rail ids only | 119 rails | `#minecraft:rails` and `BaseRailBlock`; booster rails on the powered rail | engine (a2dd15acd), *tool* |


## Porting rules

- Mod assets come from the player's own jars at run time. Never commit jars, textures, models or generated content.
- ATM10's KubeJS scripts are All Rights Reserved: read them for their effects only, copy nothing, and keep anything
  derived from them out of git. Planning notes that quote pack data stay in the local handoff folder.
- LGPL mods (AE2, Powah and others) stay in this fork and are never proposed upstream. Upstream's contributing rules forbid
  even reading LGPL code for its pull requests. Flux Networks' GUI art is CC BY-NC-SA: local only.
- Native mods hook named hook points (`game/native_hooks.rs`, `native-hooks.json`). Hooks are only appended. That file holds
  no `cfg` or macros outside tests. A new hook is added in upstream's style: its `native_*` function with the placeholder
  body, a fragment in `tools/native-hook-fragments/` with its `mod_api` contract, `python3 tools/stamp_native_hooks.py`
  run with only the fork's fragment there (upstream's pending ones moved aside, then put back), and
  `cargo run -p xtask -- mod-api`, which writes its `Hooks` trait method and dispatch. The fork's hooks are APIs 154-159
  since the merge of upstream 6e1635c1e (whose latest was 153): Forge Energy 154, a machine's item and slots 155, the
  block entity lifecycle 156, biome and dimension type 157, a bundle's opt-in to entity joins and leaves 158, the
  world's cooking recipes for a mod's machines 159. Upstream
  gates a growing list of hooks on `mod_api::mods()`, which holds trait mods only: a bundle's mixin on such a hook never
  runs unless the gate also asks it (`native_entity_level_listened` is the fork's first). Upstream stamps its pending fragments after 153 too, so
  at each merge the fork's take the numbers after upstream's latest again. `wip/quests` still says 64.
- Compare behaviour with the Java mod, run on NeoForge where a number matters (`tools/neoforge_oracle.py`), and don't trust
  an assertion that can't fail.
- One commit per finished stage on the default branch. Merge upstream every two or three days: it moves fast.

## What to do next

1. A flake to find first: an `interact` with a block's menu right after a `tp` sometimes never opens it (about 1 run in 6
   of `crafting-large-job-persistence`, `provider-condenser-fluid`, `terminal-craft-gestures`; the screen stays
   `playing`). In the failed transcripts the player drifts after the `tp`, faster each tick. Check that a teleport
   resets the motion as Java's does, and what the server's `may_reach_block` sees.
2. With the session unlocked: frame rate at spawn against Java's. Then what is still loaded for nothing: baked models on
   demand (about 1 GB, and `assets.state_models`' 25 s on a first start), lazy GUI/entity textures (640 MB of GPU memory),
   the 319 placed features that can't run. A first start still dips to 0.2 GB free on a 16 GB PC.
3. Open a world through the title screen (every run so far used `--world`), with sound on.
4. Fix the engine defects in the table above.
5. ExtendedAE's next slices (`ae2/addons/extendedae/README.md`); Mekanism's next machines (its first four are on
   `feat/mekanism-machines`, see `pack/mods/mekanism/NOTES.md`); bring `wip/quests` onto the head.
6. Then, as the roadmap in the handoff folder lays out: data maps and biome modifiers, a shared GUI kit and machine recipe
   matcher, the small required machines, mod fluids, custom dimensions and mob types as data. Measure against the Java
   baseline as you go.

Coverage today, by a count over the registries with judged weights: AE2 86.6%, Mekanism 7.5%, ATM10 as a whole about 5%.
