# All the Mods 10 on Horizon: state and how to continue

This fork ports the All the Mods 10 modpack (ATM10 8.4, Minecraft 1.21.1, NeoForge 21.1.265) to Horizon. The goal right
now is the clean pack working as it does on Java, then faster than Java, with measurements. No mods outside the pack yet.
The mods belong to their authors: each port credits them in its `README.md` or `ae2/NOTICE.md`.

Start here on a new machine. Read "Setting up a machine", then "What to do next".

## Branches

| Branch | What it holds | State |
|---|---|---|
| `atm10-horizon-rework` (default) | Upstream merged (21a750533), AE2, MEGA Cells, ME Requester, ExtendedAE's first slice, Mekanism (cubes, cables, Solar and Heat Generators), Powah, Flux Networks, `horizon-port-pack`, the full pack's start | Unit tests, the default world suites and the bundles' tests pass (3 November 2026). A world opens with all carries (below) |
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

`build.ts` builds the fork's game package, `horizon-atm10`: horizon-client's program with the AE2 crates above
horizon-client installed first ([ae2-crates.md](ae2-crates.md)). Its executable keeps the name, so it is still
`target/fast/horizon-client.exe`. Build the game with `-p horizon-atm10`: `-p horizon-client` builds upstream's
program, without the crates above it, and a build of both at once (`--workspace`) writes two executables of that
name.

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

Local bundles that can't be published (their licences) are staged from a local directory: `HORIZON_LOCAL_BUNDLES`
lists directories (`;` between them), each a bundle or a folder of bundles, and every pack script's `stage` step adds
them beside its own (`pack/scripts/local-bundles.ts`; their `tests/`, `.git` and build output are left out). Unset,
the scripts stage what they always did.

Also run `cargo test -p horizon-client --no-default-features --features ae2-lab --bin horizon-client` and `--lib`,
`cargo test -p horizon-client-lib --no-default-features --features ae2-lab --lib` (upstream split the client's library:
the game is in `horizon-client`'s, the rest, with AE2's library modules, in `horizon-client-lib`, which has the same
`ae2-*` features), `cargo check -p horizon-client` with default features and `cargo run -p xtask -- mod-api`, which
must leave `native_hooks.rs` as it is (upstream dropped `--check`: the build generates the API and reports a hook left to fill).

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
   compost, worn slot, piston and light facts of every item and block), KubeJS's generated data packs
   (`kubejs-generated/`, see "Pack logic" below), and each batch's facts (`--batches`, `facts-<batch>.json`, 1.5 GB in
   all): one server run, about 16 minutes at `-Xmx5G` on the 16 GB PC.
   Then `tools/atm10/oracle-client.sh`: the pack on NeoForge's client, hidden, for what only a client knows of its
   fluids (textures, tints, render layers, bucket colours: `client-facts.json`, docs/mod-fluids.md), about 2.5 minutes.
3. `tools/atm10/carry-with-facts.sh`: each batch carried with its facts and the client's (about 10 minutes for all);
   the mods' fluids are declared from both.
4. `horizon-port-pack final-data <carries> --dump <dump> --work <work> --resources <26.3> --instance <ATM10> --also content/ae2`
   writes each namespace's data into the carry that owns it. The rest goes in `atm10-pack`, whose mod id is `kubejs`.
5. `tools/atm10/load-check` reads them as the game does; `tools/atm10/play-atm10.ts` passes each carry to the game as a
   `--dev-mod` (`measure-load.ps1` measures a start).

What ATM10 8.4 gives (2 November 2026): 287 carries, 40,557 blocks with 873,394 states (plus vanilla's and AE2's,
about 910,000 of `MAX_STATES`' 1,048,576), 14,218 items; the load check has 289 mods on and none refused. A block
whose jar has no blockstate for it is left out: the pack's run sees what other mods register in a namespace for their
own content (Bibliowoods' furniture in other mods' woods: 6,083 blocks and 270,000 states; Dyenamics and Friends',
Amendments'). Of the dump's 95,015 recipes, 78,649 are in carries and 8,746 in `atm10-pack`, 5,699 are parked (their
result is an item we don't have: Bibliocraft's furniture, Dyenamics and Friends', Powah's). A run's count is not fixed:
one mod builds its own recipes at each reload from the recipe graph, and their number changes between runs: 17,936
in the run above and 17,529 in mod fluids' (4 November: 94,608 recipes, 78,229 in carries), whose carries and dump
replaced these; ten more differ only by a random roll (Ars Nouveau's tome colour, Artifacts' eternal steak) or an
ingredient list's order (Refined Storage's upgrades).

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
| title screen, first start (`pack-start.ts`) | 8.9 s | 67 s | 89 s | Singleplayer, Create New World: first frame in the world at **213 s**; Save and Quit to Title, then the world from the list: in it 41 s later |
| title screen, second start | none | none | 19 s | the saved world from the list: first frame at 198 s (no data file: the first launch quit before writing it) |

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

Through the title screen, as a player starts it: `tools/atm10/pack-start.ts` starts every carry (and AE2's addons)
offscreen with no `--world`, with sound on (the output open, the master volume at 0), and drives the title screen
through the control socket: Singleplayer, Create New World, in the world, Save and Quit to Title, the world from the
list, in the world, quit; then a second launch that opens the saved world from the list. `HORIZON_PLAYER_START=1`
gives that game the player's start (the title shows while the world tables load behind it; a control socket
otherwise loads everything before the window). It writes `pack-start.json` (each step, the startup trace's marks,
working set and private bytes once a second). On the 64 GB PC, with other work beside it (`bash run.sh` in the
`packstart-work` folder holds the full-pack lock):

| Through the title screen | First start | Second start | Java |
|---|--:|--:|--:|
| Title shown | 89 s | 19 s | 140 s |
| Core tables; the game answers its socket (a click before goes to the loading screen's menu) | 111 s; 127 s | 63 s; 96 s | |
| World tables | 166 s | 111 s | |
| World prepared, server's world open | 197 s, 201 s | 163 s, 169 s | |
| First frame in the world | **213 s** | 198 s | 200 s |
| Save and Quit to Title, the world from the list, first frame | 41 s | | |
| Peak working set, private | 6.4, 8.7 GB | 5.6, 8.3 GB | 8.3 GB heap |
| In the world: working set, private | 5.7, 8.1 GB | 5.2, 7.2 GB | |

The pack opens through the title screen, first start and second, and a saved world reopens from the list; no
fatal error on the way. The first start is at Java's 200 s. The second is not the 73 s of run 22: `horizon-data.bin`
is written 30 s after the first world shows, and the first launch had quit before then. A start just after a power
loss (cold disk cache, the machine busy rebuilding) also reached the world, at 1,754 s, and wrote no data file in
the 20 minutes after it. 46 blockstates of mods name models 26.3 doesn't have (`minecraft:block/oak_sign`...): the
missing model, a warning.

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


### Recipes that run

A recipe that loads can still be wrong: an ingredient the matcher never accepts, a result that isn't what the file says.
The audit puts every recipe of vanilla's types through the engine's own code, in the full pack, and reads the mod types
whose machines are ported (Mekanism, AE2 and its addons) through those machines' own recipe readers:

```sh
ATM10_CARRIES=<the folder final-data wrote, with atm10-pack> bun run tools/atm10/recipe-audit.ts
bun run tools/atm10/recipe-audit.ts --summary <report.json>      # print an earlier report
```

It starts one world (AE2, every AE2 addon in `AE2_ADDON_CONTENT_ROOT`, every carry as a `--dev-mod`; about 9 GB, one
full-pack game at a time) and sends the game's control command `recipe-audit PATH [THREADS]`
(`crates/horizon-client/src/recipe_audit.rs`). `RECIPE_AUDIT_ONLY` / `RECIPE_AUDIT_WITHOUT` pick carries,
`RECIPE_AUDIT_OUT` the report's path, `RECIPE_AUDIT_THREADS` the matcher's threads (default 4), `RECIPE_AUDIT_MODS` more `--dev-mod` folders after the carries.
Mekanism is a native bundle, so its numbers need the game built with it: `pack/scripts/mekanism.ts content` and `stage`,
then a build of the staged source with the pack's features (`bun -e` over `packFeatures()`), the audit run on that
binary (`AE2_TEST_BINARY`) with the three content mods it writes (`mekanism`, `mekanismgenerators`, `alltheores`) in
`RECIPE_AUDIT_MODS` and the same-named carries in `RECIPE_AUDIT_WITHOUT`. A new mod set's first
start takes 5-10 minutes; the audit itself, under a minute on 4 threads. The report holds every failing recipe, so it
stays out of Git: only counts are kept here.

Each recipe is checked in four steps: it parsed; every ingredient and the result are registered (tags expanded); it ran
(the first registered item of each ingredient, with the components the ingredient asks for, goes in a 3x3 grid, a
furnace, the stonecutter or the smithing slots, and `Recipes::find`, `cook`, `stonecutter` or `smithing_recipe` is
asked); the result's item and count are what the file says. `recipe_audit.rs` has two `#[ignore]` unit tests (vanilla's
own recipes all run; the causes are told apart) that need the prepared 26.3 resources.

ATM10 8.4, 83,704 recipes in the world (39,603 of vanilla's audited types), matched of total:

| Type | Matched | Total |
|---|--:|--:|
| `crafting_shaped` | 22,443 | 22,950 |
| `crafting_shapeless` | 10,743 | 11,088 |
| `smelting` | 744 | 747 |
| `blasting` | 387 | 389 |
| `smoking` | 129 | 129 |
| `campfire_cooking` | 99 | 99 |
| `stonecutting` | 3,910 | 3,910 |
| `smithing_transform` | 268 | 268 |
| `smithing_trim` | 23 | 23 |
| **all** | **38,746** | **39,603** (97.8%) |

(This world has Mekanism's content mods in place of its carries, which is 10 more shaped recipes than the first count of
38,736 of 39,593; all 10 match, and the failures are the same 857.)

The 857 that don't match, by cause. None failed in the matcher itself: every recipe that parsed and resolved ran.

- Unparsed field, 388: a mod's own ingredient type. Moonlight's `block_type_swap` is 297 (Supplementaries' way signs and
  cannon boats: it swaps a wood type's child item, so it needs the mod's wood types); then Neovitae 22 (`orb_tier`),
  Iron Jetpacks 15 (`jetpack_tier`), Silent Gear 12 (`material`), Ender IO 9 (`conduit`), Immersive Engineering 7 and a
  dozen more mods with a type or two each; some are an empty list.
- Another recipe first, 341: the recipe matches, but an earlier one takes the same grid. Vanilla's bookshelf comes
  before Productive Trees' 163 and Oh The Biomes We've Gone's bookshelves (187 in all, through `#minecraft:planks`),
  then `crafting_table` 21 and Supplementaries' `item_shelf` 19. These are the pack's own conflicts, not the engine's: Java
  1.21.1 picks by a `HashMap`'s order, which is not reproduced and so not compared.
- Unknown item, 128: the result or an ingredient isn't registered. Structurize's `sceptergold` is 71 (MineColonies;
  Structurize isn't carried), Dyenamics and Friends' parked items 36, All the Compressed 7, a few others.

Two parser fixes came out of it: NeoForge's `neoforge:components` ingredient (171 of 189 recipes using it now match) and
1.21.1's list of ingredient objects as an ingredient (4). Loading the AE2 addons took 43 of 46 addon-item failures.

Mod types whose machines are ported are read through the recipe viewer categories the machines give
(`native_recipe_categories`: Mekanism's, AE2's and its addons'), which list the recipes each machine's reader accepted,
each with a `Recipe: <id>` note. A recipe is *parsed* when a category lists it, *resolved* when the entry's input slots
and outputs name items and fluids the world registers (and chemicals the category lists). The machines' matchers take a
live block entity, so no matcher runs for these: the deepest level checked is resolved, and the count below is that.
2,542 recipes in 33 types (total / parsed / resolved):

| Type | Total | Parsed | Resolved |
|---|--:|--:|--:|
| `mekanism:sawing` | 581 | 581 | 581 |
| `mekanism:crushing` | 426 | 425 | 425 |
| `mekanism:enriching` | 400 | 395 | 395 |
| `mekanism:pigment_extracting` | 178 | 178 | 178 |
| `mekanism:painting` | 176 | 176 | 176 |
| `mekanism:injecting` | 135 | 135 | 135 |
| `mekanism:purifying` | 72 | 72 | 72 |
| `mekanism:dissolution` | 58 | 41 | 41 |
| `mekanism:combining` | 54 | 54 | 54 |
| `mekanism:metallurgic_infusing` | 42 | 42 | 42 |
| `mekanism:oxidizing` | 31 | 27 | 27 |
| `mekanism:chemical_conversion` | 26 | 25 | 25 |
| `mekanism:washing` | 25 | 20 | 20 |
| `mekanism:rotary` | 22 | 20 | 20 |
| `mekanism:crystallizing` | 21 | 16 | 16 |
| `mekanism:nucleosynthesizing` | 21 | 21 | 21 |
| `mekanism:pigment_mixing` | 16 | 16 | 16 |
| `mekanism:reaction` | 14 | 14 | 14 |
| `mekanism:compressing` | 7 | 6 | 6 |
| `mekanism:chemical_infusing` | 5 | 5 | 5 |
| `mekanism:separating` | 3 | 3 | 3 |
| `mekanism:activating`, `centrifuging`, `energy_conversion`, `evaporating` | 8 | 8 | 8 |
| **Mekanism** | **2,321** | **2,280** | **2,280** |
| `ae2:inscriber` | 39 | 39 | 39 |
| `ae2:transform` | 18 | 13 | 13 |
| `ae2:entropy` | 10 | 10 | 9 |
| `ae2:charger` | 6 | 6 | 6 |
| `extendedae:crystal_assembler` | 83 | 78 | 78 |
| `extendedae:circuit_cutter` | 18 | 15 | 15 |
| `extendedae:crystal_fixer` | 12 | 12 | 12 |
| `advanced_ae:reaction` | 35 | 30 | 30 |
| **AE2 and addons** | **221** | **203** | **202** |

The 60 left out (39 unparsed, 21 with an unknown item), by cause:

- Unparsed, 39: the reader dropped the recipe. Allthemodium's slurry recipes (dissolution 17, washing and crystallizing
  5 each: a chemical the registry doesn't read; washing's input is the tag `#minecraft:water`), Productive Bees'
  `ae2:transform` bees 5, oxidizing 4, `mekmm`'s `rotary` paste (`#c:nutritional_paste`).
- Unknown item, 21: an item the world lacks. `mekmm:uu_matter` 2, Powah's `uraninite_raw` 3, MEGA's `sky_osmium_ingot`
  3, Advanced AE's `quantum_infusion_source` fluid 2 and `quantum_infused_dust`, ExtendedAE's `ingredient_buffer`,
  `fishbig` and `mddyue`, `ae2:entropy`'s `minecraft:air` output, Starbunclemania's `sauce:source_fluid`.

Not run: 41,212 recipes of mods' own types, in 599 types (Create 4,260, Modern
Industrialization 2,667, Botany Pots 1,495). They are counted, not checked: each type is its own machine, and none of
those machines is ported. That includes the types of the ported mods that no machine reads: AE2's crafting-grid specials
(`storage_cell_upgrade` 202, `storage_cell_disassembly` 165, `matter_cannon` 23, `crafting_unit_transform` 21, wireless
terminals' `combine` and `upgrade`) and Mekanism's `mek_data` 74. Another 347 recipes of vanilla's other types are not
audited either: brewing 279, `crafting_transmute` 33, banner duplication 17, and the special crafting recipes.


### Pack logic: ATM10's KubeJS

No JavaScript runs here. What ATM10's `kubejs/` does (215 scripts: 19 startup, 180 server, 16 client; a data pack, a
resource pack) reaches the game as data: through the oracle's dump, which is the game on NeoForge with KubeJS on, and
`final-data`. `tools/atm10/kubejs-audit.py` counts the scripts' calls by kind. It checks each effect against the dump,
the carries, the native content folders and vanilla's files. It copies nothing, and its report stays in the work folder:

```sh
python tools/atm10/kubejs-audit.py --instance <ATM10> --dump <dump> --carries <carries> --resources <26.3> \
  --also content/ae2 --also <addon content root> --out kubejs-audit.json
```

ATM10 8.4. Calls are counted in the scripts' text, so a call inside a loop counts once:

| Kind | Count | Applied |
|---|--:|---|
| Recipe removals | 294 calls; 190 name an id | Yes, through the dump. 188 of the 190 ids are gone everywhere, and none of the 13,036 jar recipes the dump lacks is in a carry. Not applied: 64 recipes that vanilla's or a native mod's own file still holds (AE2's matter cannon nuggets, vanilla's cake) |
| Recipe additions | 561 calls in 60 types; 144 named | Yes. All 144 named ones are in the dump. Of the dump's 277 `kubejs:` recipes, the pack mod holds 228 and 49 are parked (their result isn't ours) |
| Recipe replacements | 66 calls | Yes for carried mods. Not applied: 289 vanilla and native recipes the pack changed keep their own file |
| Tag edits | 512 calls | Yes, through the dump, except 17 literal edits Java has. 12 are tags of native or uncarried mods (MEGA's compression overrides, Curios), whose tags the dump doesn't write. The other 5 are worldgen tags, which `final-data` leaves to the carries |
| Startup registrations | 101 items, 7 blocks, 3 fluids | Yes, declared from the oracle's facts. 27 items get their texture from the script (`.texture()`, `.color()`: the enchanting bases, Theurgy's sulfurs, the ore clumps), so they draw the missing texture |
| Other registrations | 4 Mekanism chemicals, 4 MI machines with casings, hatches and 4 recipe types, 13 Modular Machinery machines | No. The machines need MI's multiblocks, and the chemicals need Mekanism's chemical registry to take data |
| Data pack (`kubejs/data`) | 337 files | Recipes (100) and loot tables (23) through the dump. Every other file is laid over the carries by `final-data` (below) |
| Data generators (`generateData`) | 29 calls in 11 scripts | Yes. The oracle exports what they made (1,100 files), and `final-data` lays it over the carries (below) |
| Resource pack (`kubejs/assets`) | 2,081 files | 1,947 are laid over the carries. 134 belong to mods that aren't carried: the native ports (AE2, MEGA, Powah...) and client-only mods |
| Event handlers | 21 | No: right-clicks, logins, NeoForge events, a command, a level unload |
| Client only | 61 tooltip lines, 34 recipe viewer removals, 5 information pages, 9 ponder scenes | No |

`final-data` with `--instance` also lays KubeJS's data pack over the carries, as the game lays a data pack over its
mods. It takes each file the dump doesn't stand for (anything but recipes, loot tables and the tags of the dump's
registries) to the carry that owns its namespace. A file replaces the one at its path. A tag file or data map adds to
the one at its path. A free namespace's file goes in the pack mod. Worldgen files and advancements get carry's 26.3
rewrite. ATM10 8.4: 190 files are laid over the carries: 49 biome modifiers, 74 worldgen biome tag files, 16 worldgen
features and sets, 9 advancements, 7 loot modifiers, 2 data maps, and 33 of mods' own data (Forbidden Arcanus rituals,
Compact Machines room templates, Silent Gear materials and traits, Productive Bees, Hostile Neural Networks). Left out:
3 of mods that aren't carried, 6 files that aren't JSON (3 structures), and KubeJS's own 11. The largest effect is on
ore generation. ATM10 turns most mods' ores off so
that All the Ores' ores stand for them: 47 biome modifiers become `neoforge:none` (Modern Industrialization 16,
Railcraft 11, Mekanism 6...). It also sets All the Ores' vein sizes and counts, and its biome tags place
Dungeons Arise's structures.

What the `generateData` scripts make at load lives only in the running game: KubeJS keeps it in virtual data packs,
one per stage. The oracle's server writes them out (`KubeJsGenerated`, through each pack's own `export`) to
`<dump>/kubejs-generated/<stage>/data`, with `packs.json` giving their order among the game's 518 packs. The pack
oracle does it on every run. A run that does only this takes 3.5 minutes: `oracle-pack.sh` with
`-Dhorizon.oracle.kubejs_generated_only=true` in `JVM` and its own `ATM10_DATA` (its `--out`), under `fullpack.lock`.
Then copy its `dump/kubejs-generated` into the dump. `final-data --instance` lays the packs as the game orders them.
`internal`, `registries` and `after_mods` go first, then `kubejs/data` over them, then `last`. `before_mods` lies under
the mods' own files. The files are handled as `kubejs/data`'s are. A loot table that a file turns off
(`neoforge:false`) is also taken out of the carry that kept the jar's. ATM10 8.4 uses `after_mods` only, plus
KubeJS's `internal` pack:

| Generated | Files | Where |
|---|--:|---|
| The Mining dimension's ores (`miningDim.js`): configured and placed features, biome modifiers on `#allthemodium:mining_features/mining_biomes` | 32 + 32 + 32 | `allthemodium`, in 26.3's form; the jar has none of them |
| Hephaestus Forge rituals (`atm:`): the star altar, auto forge, runic crucible and enchanter, MI's hatches, modularium... | 15 | the pack mod |
| Hostile Neural Networks data models | 17 | `hostilenetworks` |
| Create's zinc ore turned off (`neoforge:none`) | 1 | `create` |
| Data maps: MI's fluid fuels (creosote), KubeJS's furnace fuels (empty) | 2 | `modern_industrialization`, the pack mod |
| Loot tables turned off (Bibliocraft's furniture of absent woods, Generator Galore's, Iron's Spellbooks' arcane debris) | 935 | none in the dump; 7 jar tables taken out of carries |
| Recipes, KubeJS's block loot tables | 26, 8 | the dump has them |

`kubejs-audit.py` reads `<dump>/kubejs-generated` too, and counts each generated file against the carries as it
counts `kubejs/data`'s. All 131 that `final-data` lays are applied.

```sh
ATM10_CARRIES=<the folder final-data wrote> bun run tools/atm10/tests/kubejs-data.ts
```

The world test loads Modern Industrialization's and All the Ores' carries. The pack replaces ten of All the Ores'
ore features (tin, lead, nickel, osmium, silver, zinc, platinum, uranium, iridium, cinnabar), and `final-data`
writes them in 26.3's form. The game runs all ten, and the nine chunks around spawn hold about 2,000 of their ore
blocks. The count varies a little between runs of the same seed. None of the 15 MI ore blocks the pack turns off
generates there. Without these files the ten don't run: the carries hold the jar's features still in 1.21.1's form
(`config` wrapped), and the game skips those as `invalid world generation field: targets`. The same holds for All
the Ores' other seven ores and all of MI's. That is a defect of these carries, not of KubeJS. The pack's files went
through carry's own 26.3 rewrite here and came out unwrapped, so carrying the two mods again should fix theirs (not
tried).

The same test proves the generated pack in effect. It also loads allthemodium's carry, and in the world it runs
`place feature allthemodium:lignite_coal_ore` in a block of stone. That puts MI's lignite coal ore there (24 blocks
on ATM10 8.4).
The feature is one of the Mining dimension's, and only the generated pack has it. A feature id the game doesn't
have fails. The Mining dimension itself can't be entered yet. Nothing in the game reads the other generated kinds
yet: rituals, data models, Create's zinc modifier (Create's own zinc feature is still in 1.21.1's form). The test checks
them in the carries.

What remains, by size:

- 353 vanilla and native recipes the pack removed (64) or changed (289). The dump has their final form, but the
  content fence keeps vanilla's and the native ports' own files. Applying them needs a pack-level override that the
  engine accepts over those namespaces.
- The 27 items whose texture is in the script. `.texture()` lands in KubeJS's client virtual asset pack, which the
  client oracle could export as the server's data packs are. `.color()` is an item colour in code, in no asset: the
  client oracle would read the colours of each `kubejs:` item, and the pack mod would write 26.3 tints for them.
- Tags of native and uncarried mods (12 edits) and worldgen tags edited by scripts (5).
- Real scripting: 21 event handlers and the code behind items (`use` callbacks: the Ice and Fire eyes).
  Each is small, and each is a native hook's worth of logic in the mod it touches. About a dozen of them matter
  for play.
- Client only: tooltips and recipe viewer hiding. JEI's bundle could read them as a second blacklist.
- The Star needs these from KubeJS: its items (registered and declared today, the star shards among them), the
  star altar, auto forge, runic crucible and enchanter as MI multiblocks (not applied), their recipes (in the dump),
  and the Hephaestus Forge rituals that make the altar's item (in the pack mod; a Forbidden Arcanus port must read them).


### Iron Furnaces

`pack/mods/ironfurnaces` ports Iron Furnaces 4.3.2 from its source: Qelifern/IronFurnacesNeoForge, branch `4.3.2`
(commit 41fde020), Apache-2.0 (`LICENSE-ironfurnaces`, `NOTICE-ironfurnaces`). The older Qelifern/IronFurnaces repository
holds the 1.20.1 versions under GPL-3.0 and was not used. The blocks, items, models and recipes are the mod's carry,
whose furnaces sit on a template with no right-click: `pack/scripts/ironfurnaces.ts content` copies the carry
(`IRONFURNACES_CARRY`) to a content mod with each furnace on `minecraft:furnace`, as Powah's furnators are. The script's
steps `units content stage build world` build and test the bundle. It uses hooks on main and no new one: the world's cooking recipes (421, as Mekanism's smelter reads them), a
mod container and its menu (417), an item used on a block (424), and the screen hooks.

What runs: the thirteen furnaces with their cooking time and fuel use from `config/ironfurnaces-common.toml` (an
iron furnace takes 160 ticks for a 200-tick recipe and a coal lights 1,280 ticks), the 16-, 32- and 64-item cookings of
the Allthemodium, Vibranium, Unobtainium and Rainbow furnaces, the speed, fuel, blasting and smoking augments (put in
by a sneaking click), the Iron Furnaces' tier upgrades in place (a sneaking click keeps the input, fuel, output, fire
and facing), and the furnace's screen from the jar's `gui/furnace.png`. Not ported: the factory and generator
augments, the side configuration, redstone modes and copy tool, the Wireless Energy Heater, the Rainbow furnace's
network, stored experience, and the vanilla furnace to Iron or Copper upgrade. The input slot takes any item. Unit
tests of the tiers, the config, the cooking and fuel arithmetic and the upgrades are in the crate; the world test
`tests/furnace.ts` checks the speeds in game time, the fuel use and an upgrade; `NOTES.md` lists the differences.

## Porting rules

- Mod assets come from the player's own jars at run time. Never commit jars, textures, models or generated content.
- ATM10's KubeJS scripts are All Rights Reserved: read them for their effects only, copy nothing, and keep anything
  derived from them out of git. Planning notes that quote pack data stay in the local handoff folder.
- LGPL mods (AE2, Powah and others) stay in this fork and are never proposed upstream. Upstream's contributing rules forbid
  even reading LGPL code for its pull requests. Flux Networks' GUI art is CC BY-NC-SA: local only.
- A mod's own source code. This fork's rule differs from rule 4 of `docs/modpacks.md` (upstream's, kept as it is
  there).
  - A mod whose code licence allows derivatives may be ported from its source: read its code at the version ATM10
    ships, translate it to Rust and verify it against the oracle. Take the licence from the source repo at that
    version, since a jar's `mods.toml` can be stale.
  - **Permissive** (MIT, Apache, BSD, ISC, zlib, CC0, Unlicense). For example Mekanism, Create's code, Modern
    Industrialization, Occultism, Apotheosis, Eternal Starlight, Industrial Foregoing, Mystical Agriculture, Refined
    Storage, Silent Gear, Hostile Neural Networks, Flux Networks, Just Dire Things, Ender IO.
    - The bundle keeps the mod's copyright notice and licence text in its folder (`LICENSE-<mod>`).
    - Its README names the mod, the version and the source repo it was ported from.
  - **Copyleft** (LGPL, GPL, AGPL, MPL). For example AE2's code, Powah, Advanced AE, ExtendedAE, Twilight Forest, the
    Aether's code, Ars Nouveau, PneumaticCraft, Deeper and Darker.
    - The bundle is published under the mod's licence with its source, in a repository of its own rather than this
      fork's MIT tree.
    - LGPL ports already in this tree (AE2, Powah) keep their licence notes.
  - **All Rights Reserved, custom, no-derivatives or non-commercial licences, and mods with no stated licence** (FTB's
    mods, Allthemodium, AllTheOres, Forbidden Arcanus, Iron's Spells, Productive Bees, Sophisticated Backpacks,
    Cataclysm): behaviour only, as `docs/modpacks.md` rule 4 says.
  - **Assets.** Whatever the code licence, assets (textures, models, sounds, lang text) are never copied. Create's,
    Apotheosis' and the Aether's are All Rights Reserved even where their code is open. Assets still come from the
    player's own jars at run time.
  - Mods' sources are cloned outside the repo (`atm10-work/mod-sources/`) and never committed.
- Native mods hook named hook points (`game/native_hooks.rs`, `native-hooks.json`). Hooks are only appended. That file holds
  no `cfg` or macros outside tests. A new hook is added in upstream's style: its `native_*` function with the placeholder
  body, a fragment in `tools/native-hook-fragments/` with its `mod_api` contract,
  `python3 tools/stamp_native_hooks.py --only <fragment>` (upstream's pending fragments stay as they are), and
  `cargo run -p xtask -- mod-api`, which fills the placeholder with the dispatch; the `Hooks` trait, the dispatch and
  the `Host` helpers are generated into `OUT_DIR` at build time (upstream's 9116ac391), none of it is committed.
  The fork's hooks are APIs 416-441 on top of upstream's 415 (still its latest at the release merged on 17 November, nightly-source/2026-11-13-9f42064): Forge Energy 416, a
  machine's item and slots 417, the block entity lifecycle 418, biome climate and dimension type 419, a bundle's opt-in
  to entity joins and leaves 420, the world's cooking recipes for a mod's machines 421, fluid handlers 422, the
  content's data files 423, an item's use on a block 424, a container's insert slots and `native_pop_resource` 425,
  particles from outside a block's tick (`native_level_particles`, Mekanism's multiblocks) 426, recipe categories for
  viewers (upstream's pending fragment `recipe_categories`, stamped for the Mekanism bundle's viewer categories) 427,
  a loaded chunk's map picture for mods' maps (`native_map_tile` over the minimap extra's scanner, FTB Chunks' claim
  screen) 428,
  a mod asking the recipe viewer for a stack's recipes or uses (`native_show_recipes`, `recipe_lookup`; JEI's bundle) 429,
  the area a mod's item panel takes, for a window drawn over the screen (`native_panel_area`; JEI's recipe window) 431,
  closing the open inventory screen from a mod (`native_close_inventory`; FTB Library's sidebar buttons, FTB Chunks'
  bundle) 432,
  a recipe viewer's transfer of a recipe into the open screen (`native_recipe_transfer`: the screen's handler, the
  missing slots and the move; JEI's "+"; AE2's terminals answer through the `ae2:jei_transfer` payload, no hook of
  their own) 433, the size and alignment of a mod's item panel grid (`native_panel_grid`: the most columns and rows,
  the alignment, keeping the search box with no items; JEI's `[ingredientList]`) 434, a block's chemical handler
  (`native_chemical_handler_tanks`, `_insert`, `_extract`: Mekanism's `IChemicalHandler` with chemicals as registry ids,
  for Applied Mekanistics' buses) 435, the food table (`native_food_table`, `native_food`: each item's nutrition,
  saturation and what eating it leaves, for Mekanism's Nutritional Liquifier) 436,
  the engine's blocks as a chemical handler (`native_chemical_tanks`, `_insert`, `_extract`: AE2's ME Interface and
  Pattern Provider for Applied Mekanistics, the other way from 435) 437,
  fluids and a mod's own ingredient types as recipe viewer ingredients (`NativeStack::kind`, `NativeIngredientInfo`,
  `NativeRecipeCategory::ingredients`, `native_fluid_icon`, `native_sprite_tinted`: JEI's fluid and Mekanism chemical
  slots) 440, the same in the item panel (`native_panel_ingredients`, `NativePanelIngredient`, `native_fluids`, `native_register_ingredients`:
  JEI's ingredient list with fluids and Mekanism's chemicals) 441 (430, 438 and 439 are unused).
  At each merge the fork's take the numbers after upstream's latest again, in the same order: the merge notes'
  `fork_hooks.py unstamp` turns the stamped numbers back into fragments, then each is stamped with `--only`. The
  fork's biome helper is `native_biome_climate` (upstream's `native_biome` gives the id alone); `native_default_state`
  and `native_sneaking` are upstream's (pending fragments `default_state` and `host_bridge`). Upstream
  gates a growing list of hooks on `mod_api::mods()`, which holds trait mods only: a bundle's mixin on such a hook never
  runs unless the gate also asks it (`native_entity_level_listened` is the fork's first; upstream's `native_joins_flush`
  and `native_joins_skip` ask `entity_level_heard`). `wip/quests` still says 64.
- Compare behaviour with the Java mod, run on NeoForge where a number matters (`tools/neoforge_oracle.py`), and don't trust
  an assertion that can't fail.
- One commit per finished stage on the default branch. Merge upstream every two or three days: it moves fast.

## What to do next

1. The flake after a `tp` (13 November): the world harness, not the engine. A teleport with a look
   (`tp @s X Y Z YAW PITCH`) didn't match the harness's pattern, so the `look-at` or `interact` after it ran while the
   client still stood where it was (`appflux-induction-card`: 13.1 blocks away; the Digital Miner: 27.5; radiation's
   spot). The others were waited for by where the player stands: one teleported into the air before its floor is built
   falls more than the 2 blocks allowed before the next poll on a loaded machine (`assembler-menu-filter`), and a
   teleport to where the player stands passed at once, and the engine sent none for it (Java's
   `ServerPlayer.teleportTo` always does; `teleport_to` now marks it). The control `status` now names the last teleport the client
   applied (`player.teleport`: id, position) and the harness waits for a newer one at the target. The motion was
   already Java's: absolute `/tp` coordinates carry no `DELTA` relatives, and both sides zero it. Reproduced without
   load by making the client read the server's packets every 250 ms (a test-build hook, never committed). Six suites
   (`appflux-induction-card`, `crafting-large-job-persistence`, `fake-slot-drag-machines`, `terminal-craft-gestures`,
   `provider-condenser-fluid`, `assembler-menu-filter`) twice: the old harness failed 4 of 12, the harness reading the
   teleport id alone 7 of 12 (6 of them waiting for the id a `tp` to the same spot never sent), and with the `/tp`
   change 1 of 12 (`fake-slot-drag-machines`' workbench drag filled one filter of two; no `tp` near it). Without the
   hook the same six passed, as did `megacells-energy` and 19 of 22 more AE2 suites at `--jobs 4` (the 3 below);
   Mekanism `radiation` (4 runs) and `digital-miner` (3) pass. Left: `portable-fluid-gestures`, `terminal-fluid-gestures` and
   `wireless-menu-lifecycle` fail every run (9 of 9) here, with the main binary and the old harness too, so not this
   change; the "menu never opens with the client at its spot" of `crafting-large-job-persistence` and
   `fake-slot-drag-machines` (the server got every use within reach, no teleport pending) did not come back in the 12
   runs after the change, so its cause is not found; and radiation's server position moving 0.987 blocks after its
   last `tp` did not come back in 4 runs.

   The flake that came after it (17 November: `no face of block … is visible from here: minecraft:stone … is in the
   way` in `assembler-forced-pattern-menu`, Mekanism `appmek-handlers` and JEI `transfer-ae2`) was not a stale position:
   the client and the server agreed, and the player was falling. Two tests teleported half a block past the north edge
   of their floor (`z - 3.5` beside a floor that starts at `z - 3`) and the harness aimed while the player fell, so a
   slow client fell further before the aim and the floor's edge came between the eye and the block. JEI's test
   teleported before it built its floor: the client fell as soon as its chunk was there, the server took those moves
   while there was no floor yet, and the player stood under the floor (its screenshot). The harness now lets the player
   land before it aims (`settle`: two statuses a tick apart at the same place, 2 s at most). If a floor was built under
   the player's `tp` spot after the teleport and the player fell through it, the harness repeats the same `tp` once
   (`stand`). A sight or reach error from a player well below its `tp` spot says the player fell. The two tests now
   stand on their floors, and the sight error names the eye. With the 250 ms hook: `assembler-forced-pattern-menu`
   failed 6 of 10 before the change and 0 of 10 after; `appmek-handlers` failed 5 of 10 before and 0 of 10 after; six
   suites that build after their `tp` failed 0 of 12 both times; a probe that lands before its floor is built failed
   before, and passed 3 of 3 after. Without the hook, 22 of 22 AE2 suites passed at `--jobs 4`, `megacells-energy` and
   `part-screens` among them.
2. Memory of the full pack: done so far are lazy baked models (e76d5862c) and the data file written after the world
   opens (2b056e630); a first start now opens with about 1 GB free on 16 GB. Left: recipes held as JSON trees (656 MB),
   lazy GUI/entity textures (640 MB of GPU memory), the 319 placed features that can't run. Frame rate against Java
   was measured on 3 November (891 against about 178 fps at spawn); measure again when mods draw more.
3. ~~Open a world through the title screen, with sound on~~: done, `tools/atm10/pack-start.ts` (above). Left: the
   second start through the title screen with the data file (why a title-screen start wrote none in 20 minutes), and a
   click on the loading screen's menu before the game answers its socket (the test clicks only once it does).
4. Fix the engine defects in the table above.
5. ExtendedAE's next slices (`ae2/addons/extendedae/README.md`: Ex Inscriber, Charger, Molecular Assembler); Mekanism's
   chemicals and gases, then factories (`pack/mods/mekanism/NOTES.md`); bring `wip/quests` onto the head.
6. Then, as the roadmap in the handoff folder lays out: data maps and biome modifiers, a shared GUI kit and machine recipe
   matcher, the small required machines, mod fluids (slice 1, the registry, flow, buckets, drawing and AE2 keys, is done:
   [mod-fluids.md](mod-fluids.md) has the design and the slices left), custom dimensions and mob types as data. Measure
   against the Java baseline as you go.

Coverage today, by a count over the registries with judged weights: AE2 86.6%, Mekanism 7.5%, ATM10 as a whole about 5%.
