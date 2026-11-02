# All the Mods 10 on Horizon: state and how to continue

This fork ports the All the Mods 10 modpack (ATM10 8.3, Minecraft 1.21.1, NeoForge 21.1.259) to Horizon. The goal right
now is the clean pack working as it does on Java, then faster than Java, with measurements. No mods outside the pack yet.
The mods belong to their authors: each port credits them in its `README.md` or `ae2/NOTICE.md`.

Start here on a new machine. Read "Setting up a machine", then "What to do next".

## Branches

| Branch | What it holds | State |
|---|---|---|
| `atm10-horizon-rework` (default) | Upstream merged (b702e0f35), AE2, MEGA Cells, Mekanism cubes, Forge Energy hooks | Unit tests and the world suites `machine-screens`, `provider-session`, `inscriber-menu-filter`, `fe-partial-acceptance`, `megacells-cells` pass |
| `int/atm10` | The default branch plus port-pack, AE2 terminals, AE2 screens, ME Requester, Powah, Flux Networks, the Windows main thread stack | Type-checks. **World suites not finished here**: run them (below), then fast-forward the default branch |
| `wip/port-pack-2` (if present) | Fixes for the review findings below, on top of `int/atm10` | In progress |
| `wip/fullpack-rebased` (if present) | The full-pack start work moved onto `int/atm10` | In progress |
| `wip/fullpack-start` | The same work before the move, on 85020d24a | Compiled there; conflicts with upstream in `game/mods.rs` and `mod_content.rs` (upstream's per-block row logic, `accept_mod`, `register_entity_types`) |
| `wip/extendedae` | ExtendedAE first slice, on 85020d24a | Its world test has not passed yet (last run: `ECONNRESET`, on a loaded machine). Applies cleanly on upstream, conflicts with ME Requester at the shared addon points |
| `wip/mekanism` | Cube tiers on Forge Energy, Universal Cables, block entity lifecycle hooks, on 85020d24a | `mekanism-cable` and `mekanism-cube-fe` passed; conflicts in `native_hooks.rs`, `block_entities.rs`, `pack/tests/mekanism-fe` |
| `wip/quests` | Quest book native mod, on 85020d24a | World test not passing yet. Its own screen hooks must move onto upstream's native mod screens (`Screen::Mod`, `screen_render_post`) |
| `wip/port-pack`, `wip/ae2-terminals`, `wip/ae2-screens`, `wip/merequester`, `wip/powah`, `wip/fluxnetworks` | The same work as on `int/atm10`, on the old base | Delete once `int/atm10` lands |
| `horizon-connect` | The owner's separate project | Leave alone |

## Setting up a machine

Windows 11 was used; 16 GB of RAM works with `CARGO_BUILD_JOBS=3` or 4, more is much better. Put the clone on an internal
NVMe drive: builds and worlds use many small files, and a USB drive is slow at those. Keep 40 GB free.

Tools: Git, Rust 1.99.1 with the MSVC build tools, Bun 1.4.4, JDK 25, Python 3.

Game files:

- Minecraft 26.3, installed by CurseForge or the launcher (a `.minecraft` layout), and its `server-26.3.jar`.
- The ATM10 instance, version 8.3, from CurseForge. The tools only read it. Never write to it.

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

Suites to pass before `int/atm10` lands:

```
machine-screens provider-session inscriber-menu-filter fe-partial-acceptance megacells-cells merequester-stock
portable-menu-lifecycle terminal-screen-views terminal-search terminal-craft-gestures wireless-key-types
wireless-source-components crafting-alternatives crafting-fluid-template inframe-terminal part-screens
machine-menu-identity mixed-cell-menus chest-fluid-gestures fake-slot-drag-machines emitter-fake-gestures
emitter-memory-components export-bus-crafting-schedule interface-stock-editor
```

Build for those with `--features ae2-lab,ae2-megacells,ae2-merequester`. Then the native mods, each with its own game build:

```sh
ATM10_DIR=... bun run pack/scripts/powah.ts units stage build world         # see pack/mods/powah/NOTES.md
ATM10_DIR=... bun run pack/scripts/fluxnetworks.ts units stage build world  # see pack/mods/fluxnetworks/NOTES.md
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

1. `horizon-port-pack` (crate, run from `crates/horizon-port-pack`) with `tools/neoforge_oracle.py pack` starts the whole
   pack on NeoForge and dumps its final recipes, tags, loot and block facts as JSON.
2. `horizon-port-pack final-data <carries> --dump <dump> --work <work> --resources <26.3> --instance <ATM10> --also content/ae2`
   writes each namespace's data into the carry that owns it. The rest goes in `atm10-pack`, whose mod id is `kubejs`.
3. The launcher passes each carry to the game as a `--dev-mod`.

The Java baseline to beat, on the 16 GB laptop: 200 s from launch to in-world (140 s to the title, 58 s for the world),
96,202 recipes, about 8.3 GB heap.

Where the Horizon start stands: all 289 mods load with none refused. The last measured start read their content in 52 s
and built the content root in 187 s, then spent about 13 minutes before reporting incomplete block models, and had not
reached the title when it was stopped at 28 minutes. Whether it is slow or stuck is not known yet. That was with
`wip/fullpack-start`.

Known defects in the generated carries, from a review that read the files (no game has run them). Fixes for those marked
*tool* are on `wip/port-pack-2`, and some may be unfinished; check that branch first.

| Defect | Scale | Where |
|---|---|---|
| Items copy the carved pumpkin template: head slot on shift-click, compost value; pistons destroy the blocks; items burn as fuel; wrong stack size | 5,233 items; 10,238 burn; 571 stack size | *tool* `oracle.rs` |
| Lang files overwritten instead of merged | 46 files, 6,251 keys | *tool* `final_data.rs` |
| `no_occlusion` set but dampening stays 15, so blocks stop all light | 6,837 blocks | *tool* `oracle.rs` |
| Failed selectors leave the template's full cube | 1,220 blocks | *tool* `oracle.rs` |
| Empty-collision states dropped | 135 blocks | *tool* `oracle.rs` |
| The blockstate guess skips the shape check: tables that act as pressure plates or buttons | 23 plates, 9 buttons, 58 fences | *tool* `oracle.rs` |
| Flat `light` on the `redstone_lamp` template | about 240 of 279 | *tool* `oracle.rs` |
| A wrong `--dump` wipes recipes and tags before the dump is read; function tags deleted | | *tool* `final_data.rs` |
| Placement ignores declared properties: `facing` and `direction` always place at default | 909 + 711 blocks | engine, `horizon-world/src/level` |
| Mod loot entry types are skipped, so blocks drop nothing | up to 2,658 tables | engine, `horizon-world/src/loot.rs` |
| Minecarts ride vanilla rail ids only | 119 rails | engine, `minecarts.rs` |

Of the 79,957 recipes written, 43,432 are mod-typed and do nothing until a port reads them, 306 vanilla-typed ones don't
parse and 295 need items we lack, so about 35,900 can be crafted today.

`MAX_STATES` is 2^20. The pack is at about 908,000 states. Bibliocraft's furniture in other mods' woods needs about
270,000 more, so it stays out until a full start works and table memory is measured.

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
  `cargo run -p xtask -- mod-api`, which writes its `Hooks` trait method and dispatch. The fork's hooks are APIs 154-158
  since the merge of upstream 6e1635c1e (whose latest was 153): Forge Energy 154, a machine's item and slots 155, the
  block entity lifecycle 156, biome and dimension type 157, a bundle's opt-in to entity joins and leaves 158. Upstream
  gates a growing list of hooks on `mod_api::mods()`, which holds trait mods only: a bundle's mixin on such a hook never
  runs unless the gate also asks it (`native_entity_level_listened` is the fork's first). Upstream stamps its pending fragments after 153 too, so
  at each merge the fork's take the numbers after upstream's latest again. `wip/quests` still says 64.
- Compare behaviour with the Java mod, run on NeoForge where a number matters (`tools/neoforge_oracle.py`), and don't trust
  an assertion that can't fail.
- One commit per finished stage on the default branch. Merge upstream every two or three days: it moves fast.

## What to do next

1. Run the suites above on `int/atm10`, fix what fails, fast-forward the default branch.
2. Finish `wip/port-pack-2` and `wip/fullpack-rebased`, regenerate the carries, and find out whether the full start is slow
   or stuck. This is the main goal.
3. Fix the engine defects in the table above.
4. Bring `wip/mekanism`, `wip/extendedae` and `wip/quests` onto the head, in that order.
5. Then, as the roadmap in the handoff folder lays out: data maps and biome modifiers, a shared GUI kit and machine recipe
   matcher, the small required machines, mod fluids, custom dimensions and mob types as data. Measure against the Java
   baseline as you go.

Coverage today, by a count over the registries with judged weights: AE2 86.6%, Mekanism 7.5%, ATM10 as a whole about 5%.
