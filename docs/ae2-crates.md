# Moving the fork's AE2 code out of `horizon-client`: plan

Worktree `atm10-wt/eae`, branch `refactor/ae2-crates` (from `int/all-1211`, 4e8619a77). Phase 1 = measure and plan;
no code moved. Scripts and reports for the numbers below: `split-work/measure.sh`, `split-work/reports/`,
`split-work/cargo-ae2.cmd` (cargo with the pack's features, used as `build_times.py --cargo`).

## 1. Inventory (what is ours)

242 files, 117.2k lines (tests included), all behind `ae2-lab` + nine addon features that are **not** default:
`ae2/scripts/build.ts` builds `--no-default-features --features ae2-lab,ae2-megacells,ae2-merequester,ae2-extendedae,
ae2-ae2wtlib,ae2-aeinfinitybooster,ae2-importexportcard,ae2-enderdrives,ae2-advancedae,ae2-appflux` (pack/pack.toml).

| Where | Files | Lines | Compiled in |
|---|---|---|---|
| `horizon-client/src/game/ae2_*.rs` | 112 | 62,074 | `horizon-client` |
| `horizon-client/src/game/ae2x/**` (addons) | 54 | 23,256 | `horizon-client` |
| `horizon-client/src/game/ae2_p2p/**`, `game/machines/ae2_capabilities.rs` | 6 | 2,280 | `horizon-client` |
| `horizon-client/src/ae2_*.rs` | 14 | 5,868 | `horizon-client-lib` (`client_lib.rs`) |
| `horizon-client/src/ae2x/**` (addon defs) | 17 | 7,201 | `horizon-client-lib` |
| `crafting/ae2.rs`, `item_render/ae2_*.rs`, `menu/ae2_network_tool.rs` | 4 | 588 | `horizon-client-lib` |
| `horizon-client-render/src/ae2_*.rs` | 4 | 2,092 | `horizon-client-render` |
| `horizon-command/src/ae2_components.rs`, `horizon-items/src/ae2_stack_identity.rs` | 2 | 1,099 | engine crates low in the tree |
| `crates/ae2-{network,crafting,cell,megacells,machines,merequester,links}-core` | 29 | 12,786 | 7 pure crates (deps: serde, each other) |

Share of the big crates: `horizon-client` 87.6k of 488.7k lines (18%), `horizon-client-lib` 13.7k of 340.4k (4%).
In `game/`: addons 23.3k (extendedae 6.7k, advancedae 6.3k, megacells 1.9k, merequester 1.6k, ae2wtlib 1.6k,
importexportcard 1.4k, appflux 0.9k, enderdrives 0.6k); AE2 itself ~13.7k of screens/UI and ~49.4k of logic.

### How it is wired into `horizon-client`

- `Game` is a **private** `struct Game<'a>` in `game.rs` with 585 private fields, 48 of them AE2's. AE2 extends it:
  174 `impl Game` blocks in 140 of the 172 AE2 files, 2,029 AE2 methods on `Game`, 1,793 `pub(super)`/`pub(in crate::game)` fns.
- AE2 -> engine: 86 non-AE2 `Game` fields read/written 3,074 times (`world` 830, `res` 315, `inventory` 294,
  `carried` 249, `dim` 164, `remote` 147, `pack` 109, `cursor` 100, `offhand` 89, `gui` 88); 123 engine `Game` methods
  called 960 times (`system_message` 92, `as_guest` 61, `spectator` 44, `held` 44, `with_level` 38, `held_mut` 35,
  `enter_level` 32, `give` 28, `level_set` 28, `has_shift_down` 27). Module paths: `save` 168 refs/52 files, `items`
  164/82, `game::lan` 130/33, `containers` 114/51, `item_data` 109/43, `text` 95/34, `menu` 81/19,
  `game::lan_crafting` 71/13, `crafting` 50/16, `game::{KeyCode,Screen,shield,Dim,Hand}` ~90.
- Engine -> AE2: 658 calls to 402 distinct AE2 `Game` methods from 40 engine files (`lan_actions.rs` 131,
  `remote.rs` 92, `game.rs` 86, `use_hand.rs` 46, `inventory_slots.rs` 42, `storage.rs` 40, `menu_clicks.rs` 37,
  `machines.rs` 31); 351 module-path refs in 43 files (lib's `menu.rs` 43, `lan/menu.rs` 14, render's
  `block_entities.rs` 33).
- 1,346 `feature = "ae2-*"` gates in 95 non-AE2 files: `horizon-client` 1,061 (`game.rs` 321: 101 `mod` lines, 48
  fields, the rest call sites; `remote.rs` 200; `lan_actions.rs` 110), lib 146, render 108, input 18, items 9,
  command 3, client-core 1.
- 29 AE2 game files (7.2k lines: `ae2_memory*`, `ae2_energy_service`, `ae2_workbench`, `ae2_channels`, the
  extendedae/advancedae/megacells `screens.rs`...) touch no `Game` at all; their only outside refs are other AE2
  modules.
- Precedent for a host trait: `game/ae2_buses.rs`'s `ExportHost` (addons get `&mut dyn ExportHost`).
- `native_hooks.rs` is **not** a runtime extension point: its hook points are empty functions that horizon-mixins
  bundles hook with HEAD at source level, and Mekanism's runtime modules are spliced into `game.rs` the same way
  (`horizon-mixin.toml` `[[module]] file = "crates/horizon-client/src/game.rs"`). So the Mekanism bundle is separable
  as *source*, but compiles inside `horizon-client` exactly like AE2 does today; it is no build-time model.
- `tools/check_layers.py` already fails on this branch: the seven `ae2-*-core` crates have no layer, and
  `horizon-command -> ae2-cell-core` breaks the portable rule.

## 2. Upstream's layout and what it allows

Upstream's rule (docs/layers.md, checked by `tools/check_layers.py`): **engine < platform < Horizon**. A crate
depends only on its own layer or lower ones; when lower code needs something from above, the lower layer offers an
extension point (a trait, callback list, event or registry naming no type from above), the higher layer plugs in,
and the program players run is assembled at the top. `horizon-client` is today the gameplay library *and* the
executable; upstream's plan (step 4, #3916) is to split it into the engine's gameplay library and a thin
executable that wires in the higher crates. We don't do that split; we only add crates of our own beside it.

What it means for AE2:

- AE2 is not vanilla, so it is not engine. It is a mod port: off by default, no built-in address, depends on the
  engine. Its crates are **platform**. So is the fork's assembled pack game (layers.md lists modpack porting
  under platform, and the pack build has no `horizon-extras`).
- A platform crate may depend on `horizon-client` and everything below; **no engine crate may depend on it**.
  AE2's crates therefore sit **above `horizon-client`'s library**, `horizon-client` calls them only through an
  extension point it defines, and the game is assembled in a package above both.
- Today the fork does the opposite: engine crates (`horizon-client`, `-lib`, `-render`, `-command`, `-items`) carry
  AE2 code and depend on the `ae2-*-core` crates, and the layer check fails (section 1).
- "First numbers" (upstream, default features, 16 jobs): in `fast`, an edit lower in the tree is *not* cheaper than
  one in `horizon-client` (`horizon-client-render` 78 s, `horizon-items` 127 s, `game.rs` 92 s), because
  `horizon-client` is rebuilt either way. Our measurement (section 3) shows the same thing more sharply. A crate
  *below* `horizon-client` (between `horizon-client-lib` and `horizon-client`) would break the layer rule **and**
  buy nothing per edit. The layer rule and the build-time goal point the same way: above.

## 3. Cheap wins, measured

Upstream's tool, unchanged: `build_times.py run --profiles fast --only incremental --jobs 6 --repeats 2`, the
default `middle` edit, cargo through `split-work/cargo-ae2.cmd` (adds the pack's `--no-default-features --features ...`
to `cargo build`; the tool has no features option), own target dirs `split-work/bt-*`. rustc 1.99.1, Windows 11,
28 threads, 64 GB. Reports: `split-work/reports/*.json|md`.

**Load:** never quiet. Another agent built Mekanism the whole time: the tool flagged other cargo/rustc/GCC processes
for every measurement; the CPU was 4-14% busy just before each build. Repeats differ by up to 34 s (12%). The
power loss at 14:18 killed the codegen-units run; it was redone after the reboot (incremental folders deleted
first) while a turbine verify and an SPS finisher were building. Rerun on a quiet machine for final numbers; the
conclusions below do not depend on a 12% error.

| Build (`fast`, -j 6) | As committed (MSVC `link.exe`, `codegen-units = 16`) | `codegen-units = 256` |
|---|---|---|
| Cold (the first build's timings page) | 623 s; `horizon-client` lib 359.5 s alone, from 261 s to the end; bin + link 2.8 s | 615.9 s; `horizon-client` lib 368.7 s |
| Nothing changed | 0.4 s | 0.4 s (the first one after the power loss, 198.6 s, rebuilt the wgpu chain once; not counted) |
| AE2 edit, `game/ae2x/mod.rs:717` | **277.8 s** (294.9, 260.7) | 271.3 s (248.0, 294.5; the second run was flagged at 22.9% busy) |
| `game.rs:3733` | **264.4 s** (252.6, 276.2) | 249.4 s (282.3, 216.6; the first run was flagged at 22.5% busy) |
| `ae2-crafting-core/src/lib.rs:24`: a 1.4 s crate right below `horizon-client`, used in 5 of its files | **297.8 s** (1 run): `horizon-client` 295.0 s of it | - |

What the numbers say:

- **Every edit is one rustc on `horizon-client`'s library.** It recompiles ~75% of its cold time after a one-line
  comment, whether the line is in AE2, in `game.rs` or in a crate below it. The bin and the link are 2.0-2.8 s.
- **Linker: no win.** The link is inside those 2-3 s, so `rust-lld` can save 2 s at most of ~280. The rust-lld
  runs were dropped after the first numbers; nothing committed.
- **A crate below `horizon-client` gains nothing** (the `ae2-crafting-core` row): a dependency change costs
  `horizon-client` a full-size rebuild, so the existing `ae2-*-core` crates cost ~5 min per edit too.
- **`codegen-units = 256`: no win.** It is within noise on every row (-2% to -6%, on a noisier machine). Fewer,
  smaller codegen units do not make rustc reuse more. It was set with `CARGO_PROFILE_FAST_CODEGEN_UNITS`, so the tree
  stayed clean; nothing committed. Also noted: right after the power loss, a `horizon-client` rebuild caused by a
  changed dependency (the wgpu chain) took 174.7 s with 256 units, against 295 s for `ae2-crafting-core` with 16. That
  is one sample under a different load. Worth one quiet rerun of the dependency probe at 256 before step 4, not a
  change now.
- **No profile or linker change is committed.** The time is rustc's front end and LLVM work on one 489k-line crate.
  Only making that crate smaller moves the number.

## 4. The move

### 4.1 Target shape

```
... horizon-client-render, horizon-client-lib            engine (AE2's lib half stays here until step 5)
        ^
horizon-client          engine, upstream's package (lib + bin), its shape unchanged. Two fork-only files:
                        game/ae2_ext.rs  the extension point: trait Ae2Ext (what the game calls), trait Ae2Host
                                         (what AE2 may read and do), a OnceLock-installed factory, one Game field
                        game/ae2_host.rs impl Ae2Host for a view of Game
        ^
horizon-ae2             platform  AE2's game logic (~49k lines): grids/channels, the ME runtime, machines, buses,
                                  autocrafting, storage, spatial, P2P, energy, worldgen, LAN sync; implements Ae2Ext
horizon-ae2-ui          platform  screens, widgets, terminals, the guide, HUD (~13.7k lines)
horizon-ae2x-<addon>    platform  one crate per addon, each behind its feature (21.3k lines in 9 crates)
        ^
horizon-atm10           platform  the pack's game: a bin package with horizon-client's 34-line main.rs plus
                                  horizon_ae2::install() before horizon_client::entry::main();
                                  ae2/scripts/build.ts builds -p horizon-atm10
```

Rules that keep the gain:

- **Calls across the line are `dyn`:** `&mut dyn Ae2Host` and `Box<dyn Ae2Ext>`. Neither side of the line gets a
  generic function or an `impl Trait` parameter. Otherwise the code is monomorphized back into `horizon-client` or
  the executable and recompiled there.
- **Fork-only files hold the traits.** Upstream files only get call sites, and they already have 1,346 gates.
  AE2 reaches the upstream items it needs (`game::lan::LanHost`, `lan_crafting`, `KeyCode`,
  `shield::OFFHAND_SLOT`...) through host methods. Those items are not made `pub`, so merges stay small.
- **State lives above.** The 48 `ae2_*` fields of `Game` become the extension's own state. For each call the host
  view takes the extension out of `Game` and puts it back afterwards. Any call back into AE2 made during that call
  must be queued or refused (see the risks).
- **Upstream already uses this pattern:** `beehive.rs`'s `HiveHost` is implemented by `game/beehives.rs`'s
  `GameHive<'_, '_>`, and `horizon-world` has `LevelHost`. AE2's own `ae2_buses::ExportHost` follows it too.

### 4.2 Steps (each builds and tests green before the next)

Run these checks after every step:

- `bun run ae2/scripts/build.ts`;
- `cargo test` of every moved crate, and of `horizon-client --lib/--bin` with `ae2-lab` (run-unit.sh);
- `cargo check -p horizon-client` with default features (no AE2);
- the moved subsystem's world suites (rerun.sh);
- `tools/check_layers.py`;
- a `build_times.py` probe of an edit in the moved code.

0. **Groundwork, no code moved.**
   a. Make the layer check green: give the seven `ae2-*-core` crates a layer (decision D2) and add the fork's
      exceptions.
   b. Add the `horizon-atm10` bin package (a copy of `main.rs`) and wire it into build.ts, run-unit.sh, the
      world-test scripts and packaging. The exe keeps the name `horizon-client.exe`, or the launcher learns the
      new one (decision D3).
   c. Add an empty `horizon-ae2` crate above `horizon-client`, with features forwarded:
      `horizon-atm10/ae2-* -> horizon-client/ae2-* + horizon-ae2/ae2-*`.
   d. Add `game/ae2_ext.rs` and `game/ae2_host.rs` with zero methods; `install()` sets the factory.
1. **Addons out, one per commit, smallest first.** The order: aeinfinitybooster (132 lines), enderdrives (607),
   appflux (853), importexportcard (1.4k), ae2wtlib (1.6k), merequester (1.6k), megacells (1.9k), advancedae
   (6.3k), extendedae (6.7k). The last two share `kits.rs`, which goes to a small `horizon-ae2x-kits` crate.
   - `game/ae2x/mod.rs` (1,939 lines) is already "the one place the rest of the game calls into the addons": 214
     of their 265 call-ins go through it. It stays in `horizon-client`, and its `#[cfg] addon::f(self)` calls
     become calls on registered `dyn Ae2Addon` objects.
   - From the rest of `Game` the addons need 181 methods (122 of them AE2-core `ae2_*`, 59 engine) with 659 calls,
     and 30 fields with 707 refs. The `ae2x` field (189 refs) is their own state and moves up with them. The rest:
     `world` 172, `inventory` 50, `remote` 40, `res` 40, `carried` 33.
   - That makes `Ae2Host` v1 about 150 methods, mostly one-line forwards.
2. **Pure AE2 modules move up with their first caller.** 29 game files (7.2k lines) touch no `Game`. Each moves
   in the step that moves the code calling it, because a module left in `horizon-client` cannot call up.
3. **AE2 UI out** (~25 files, 13.7k lines) into `horizon-ae2-ui`.
   - The draw, input and screen dispatch in `game.rs` (including `Screen::Ae2Guide`) becomes the
     `Ae2Ext::{draw, input, screen}` hooks.
   - The UI reaches the logic it calls through the host until step 4 moves that logic.
4. **AE2 logic out, top-down by subsystem** (~49k lines, 92 files).
   - The engine calls AE2 from 658 sites in 40 files (402 distinct AE2 methods): `lan_actions.rs` 131,
     `remote.rs` 92, `game.rs` 86, `use_hand.rs` 46, `inventory_slots.rs` 42, `storage.rs` 40,
     `menu_clicks.rs` 37, `machines.rs` 31.
   - Those calls fold into a few dozen `Ae2Ext` hooks by kind: tick, use block or item, menu click, LAN action,
     remote packet, slot rules, storage, save and load, worldgen, block entity.
   - Order: spatial and P2P; crafting and patterns; buses and interfaces; storage and terminals; the grid and
     energy last, because everything calls them.
   - The host surface grows while callers move, and shrinks as their callees follow.
   - At the end `horizon-client` keeps only `ae2_ext.rs`, `ae2_host.rs` and the hook calls.
5. **Optional, later: the lib half moves up too.** That is `horizon-client-lib` (13.7k lines), `-render` (2.1k)
   and `-command`/`-items` (1.1k), through hooks in those crates.
   - Then no engine crate depends on AE2, and `ae2-*-core` are platform with no exceptions.
   - AE2 becomes a separable mod like the Mekanism bundle's `mekanism-energy`, but compiled on its own.

### 4.3 Expected gain (estimated from section 3, -j 6, loaded machine)

The estimate assumes rustc time scales with lines:

- `horizon-client` lib takes 359.5 s cold for 488.7k lines.
- An edit costs ~75% of the cold time.
- A moved crate is rebuilt together with whatever depends on it, plus the 2 s bin.

| Edit in | Today | After step 1 | After step 4 |
|---|---|---|---|
| an addon (0.1-6.7k lines) | 278 s | **~5-10 s** | ~5-10 s |
| AE2 logic or UI | 278 s | 278 s | **~40-60 s** (`horizon-ae2` ~27 s, then -ui and the addons beside each other) |
| `game.rs` / engine | 264 s | ~254 s (-4%) | **~217 s** (-18%: 87.6k lines fewer) |
| an `ae2-*-core` crate | 298 s | 298 s | 298 s until step 5 |
| cold | 623 s | ~610 s | **~560 s**: the AE2 crates start once `horizon-client`'s metadata is ready and compile alongside its codegen |

No profile or linker change gets near these numbers: rust-lld can save at most 2 s, and 256 codegen units
saved 2-6% (within noise).

### 4.4 Risks

- **The host traits are large.** AE2 reads 86 `Game` fields (3,074 refs) and calls 123 engine methods (960
  calls). Each becomes a host method until its code moves. The work is mechanical but large: steps 1-4 take weeks,
  not days. Step 1 alone already pays back for addon work.
- **Re-entrancy.** While the extension is out of `Game`, an engine method that calls back into AE2 (item pickup,
  block change, LAN echo) finds no AE2. Every hook needs an audit. The safe default is to queue such events and
  run them after the outer call.
- **Behaviour.** cfg chains (`if ae2 ... else if vanilla ...`) become hook calls, and the hooks must keep exactly
  the same order. The world suites are the guard: run the moved subsystem's suites at every step, and the full set
  once per batch.
- **Merges at upstream releases.** The traits are fork-only, but the hook sites sit in upstream's busiest files
  (`game.rs`, `remote.rs`, `lan_actions.rs`). When upstream splits `horizon-client` into a library and an
  executable, `horizon-atm10` folds into their executable crate. The merges get harder; that was accepted.
- **The measurement premise.** The plan rests on one measurement (`ae2-crafting-core`): a changed dependency costs
  its dependents a full-size rebuild. That cuts both ways. Whatever depends on `horizon-ae2` also rebuilds fully
  when it changes, so its dependents must stay small: the UI, the addons and the bin.
- **Tests.** The 494 unit tests in AE2 game files and the 151 lib-side ones build no `Game`, so they move with
  their modules. The world suites drive the exe, so they need `horizon-atm10` (step 0b).

### 4.5 Decisions (taken 2026-11-12)

- **D1: above.** AE2 goes above `horizon-client`'s library, behind an extension point. A crate below gains
  nothing (298 s for a 1.4 s crate) and breaks the layer rule.
- **D2: platform.** `ae2-*-core` are mod logic, so they sit in the `platform` layer, with fork-only `allowed`
  entries for today's engine -> core edges.
- **D3: same exe name.** The fork's `horizon-atm10` package builds `horizon-client.exe`, so scripts, tests and
  the launcher stay as they are.
- **D4: one crate per addon.** This is the separable-mod goal, and it lets cold builds run in parallel.
- **D5: quiet baseline.** The four `fast` rows are measured again on a quiet machine before step 1. Those numbers
  are the "before".

## 5. Guardrails: gain a lot, lose nothing

The move is only worth it if builds get much faster and the game behaves exactly as before. So:

- **A pilot first.** Steps 0 and 1 begin with one small addon (AEInfinityBooster). Measure an edit in it with
  `build_times.py`. Go on to the other addons only if that edit drops from about 278 s to under 30 s. Otherwise
  stop and rethink.
- **Measure every step.** Each step's commit gets a `build_times.py` probe of an AE2 edit, a `game.rs` edit and
  the nothing-changed build. A step that makes any of them slower than the baseline by more than the noise is
  reworked or reverted, not kept.
- **No behaviour change.** A step only moves code and adds the host trait. Each step must pass `build.ts`, the
  unit tests, the default-features check, the layer check and the world suites of the code it moved. Each batch
  of steps must pass the full check before it is pushed. Hook order stays identical; any reordering is a bug.
- **Small, revertible commits.** One addon or one subsystem per commit, so a bad step reverts alone.
- **No parallel AE2 work during the move.** New AE2 slices wait until the move is done, so nothing has to be
  moved twice. The Mekanism bundle lives in `pack/mods` and goes on meanwhile.

## 6. Progress

### Step 0 (groundwork)

- **0a, layers.** The seven `ae2-*-core` crates are `platform` in `tools/layers.json`. The 16 edges from
  `horizon-client`, `-lib` and `-render` to them are fork-only `allowed` entries, in one block at the end of the
  list. `horizon-command` is portable, so an `allowed` entry cannot excuse its `ae2-cell-core` dependency. That
  dependency is gone: the key type codec of `ae2_components` asks a lookup that `horizon-client-lib`'s
  `ae2_key_types::register` installs before it registers the first type beyond the two built in. Until then
  those two are all the registry holds, so the answers are the same. `ae2_components.rs` stays in
  `horizon-command` until step 5: `horizon-items`' `item_data` and the command parser call it.
- **0b, `horizon-atm10`.** The fork's game: horizon-client's `main.rs` with `horizon_ae2::install()` (and each
  moved addon's `install()`) before `entry::main()`. Its program is named `horizon-client` (D3), and its own
  `build.rs` gives it the icon, the optional Segment Heap manifest and the 16 MB main-thread stack, as
  horizon-client's does for its program. It passes on all of horizon-client's features. `build.ts` builds
  `-p horizon-atm10`; `test.ts --client` also tests the crates above horizon-client on the game's features.
  `AE2_TEST_BINARY`, `play.ts`, `play-atm10.ts` and the world harness keep `target/fast/horizon-client.exe`.
  `pack/scripts/{mekanism,powah,fluxnetworks}.ts` still build `-p horizon-client --features ae2-lab`: the same
  game while no AE2 core code has moved; they must switch to `horizon-atm10` before step 3.
- **0c, `horizon-ae2`.** Holds the extension it installs, and the list of addon behaviours (`add_addon`).
- **0d, the extension point.** `game/ae2_ext.rs`: `Ae2Host`, `Ae2Ext`, `Ae2Addon`, the factory and the `Slot` in
  `Game` (`ae2_ext`, beside `ae2x`). `game/ae2_host.rs`: `Ae2Host for Game` and the game's calls into the addons
  (`Game::ae2_addon_*`). The extension is shared (an `Rc` the call clones), not taken out of `Game` for the call:
  see Step 1, Ender Drives, for why.

### Step 1, the pilot: AEInfinityBooster

`horizon-ae2x-aeinfinitybooster` holds both halves: the rows (`defs.rs`, from `ae2x/aeinfinitybooster.rs`) and
the game behaviour (`game.rs`, from `game/ae2x/aeinfinitybooster/`). Two extension points carry it:

- the lib's definitions registry: `ae2x::defs::install_addon_rows(id, rows)`, which `defs::get()` runs in the
  addon's old place in its order (`add_addon_rows`); installing after the registry was built panics;
- the game's: `tick_ae2x` calls `self.ae2_addon_tick("aeinfinitybooster")` where it called
  `tick_aeinfinitybooster()`. The host gives it `is_remote`, `dimension`, `world` and `streaming`.

`horizon-client`'s feature `ae2-aeinfinitybooster` gates nothing any more; it stays so that the pack's feature
list is still valid for `-p horizon-client` (test.ts, the full check's scripts).

### The pilot's measurement (section 5's guardrail)

`build_times.py run --profiles fast --only incremental --jobs 6 --repeats 2`, `middle` edit, on a quiet machine
(2026-11-12 17:44-18:53, after the full check; CPU 0.3-5.1% busy before each build; the only other "build
program" the tool saw is an idle `GCC.exe` vendor process, 5 s of CPU since boot). Base: daaaed44e in
`split-work/bt-msvc`, `-p horizon-client` (`cargo-ae2.cmd`), so these are D5's quiet baseline. Pilot: c671b4024
in `split-work/bt-pilot`, `-p horizon-atm10` (`cargo-atm10.cmd`). Reports: `split-work/reports/*-base-*`,
`*-pilot-*`.

| Edit (`fast`, -j 6) | Base daaaed44e | Pilot c671b4024 |
|---|---|---|
| Nothing changed | 0.4 s (1.6 s right after the warm-up build) | 0.4 s |
| AEInfinityBooster (base: inside `horizon-client`; pilot: `horizon-ae2x-aeinfinitybooster/src/game.rs:60`) | 166.1 s (the AE2 row) | **2.5 s** (2.52, 2.47): the crate 0.2 s, `horizon-atm10` 1.9 s |
| AE2 in `horizon-client`, `game/ae2x/mod.rs` (717 / 715) | **166.1 s** (164.9, 167.3) | 169.0 s (169.8, 168.2): `horizon-client` 165.9 s, then the crates above it |
| `game.rs` (3733 / 3742) | **169.9 s** (170.1, 169.7) | 169.9 s (170.3, 169.5) |
| `ae2-crafting-core/src/lib.rs:24` | **160.7 s** (161.1, 160.4) | - |

- On a quiet machine an edit in `horizon-client` costs ~166-170 s, not the 264-298 s of section 3 (loaded).
- **The guardrail passes**: an edit in the moved addon drops from 166 s to 2.5 s (under 30 s). Go on with step 1.
- Edits that stay in `horizon-client` cost what they did: `game.rs` the same, `ae2x/mod.rs` +2.9 s (1.7%, about
  the spread of the repeats), for `horizon-ae2`, the addon crate and the program now built after it. Watch this
  row as addons move: their crates build side by side, so it should stay a few seconds.

### Step 1, Ender Drives

`horizon-ae2x-enderdrives`: the rows and the config (`defs.rs`, from the lib's `ae2x/enderdrives.rs`) and the
game behaviour (`game.rs`, from `game/ae2x/enderdrives/`). Ender Drives reaches into AE2's core more than the
booster does, so this step made the extension point what the next addons need:

- `Ae2Addon` has default no-op methods, one per place the game called the addon: `tick`, `tick_frozen`
  (`tick_ae2x_frozen`), `snapshot` (`ae2x_snapshot`, with `Ae2Snapshot::restore`), `save`, `cell_mounted`,
  `cell_idle_drain`, `enrich_visual`, `handler_cell`, `item_hover_lines`, `command`, and
  `lan_main_hand_payload` (the guest lookup and the inventory resend stay in the game: `ae2_host.rs`). The
  game calls each one by the addon's id where `game/ae2x/mod.rs` called the addon's function, at the same place:
  `tick_enderdrives` ran in both ticks, `enderdrives_primary` in `ae2x_cell_mounted`, and so on.
- `Ae2Host` gained the 15 methods the addon needs, each a one-line forward to the AE2 core (`block_entity`,
  `address_parts`, `local_address`, `storage_priority`, `part_machine`, `cell_hosts`, `known_cell_key`,
  `machine_addresses`...) and `world_root`, the UUIDs and names, and the main hand's stack. The handles it passes
  (`GridAddress`, `PartAddress`, `Visual`, `HandlerCell`) are opaque to the addon: their types became `pub` in
  their (fork-only, private) modules and are re-exported by `ae2_ext`; no upstream item was made `pub`.
- **The extension is shared, not taken out.** The pilot's slot took the extension out of `Game` for each call.
  That was enough for the booster, which asks the host only for plain reads. Ender Drives' own tick asks the host
  for `cell_hosts`, which builds the AE2 network, and the network asks every addon for its cells' idle drain
  (`ae2x_cell_idle_drain`, including Ender Drives'): with the extension taken out that answer would have been
  missing, and the grid's power analysis in that tick changed. So `Slot` holds an `Rc<dyn Ae2Ext>` that each call
  clones, the addons' methods take `&self`, and what an addon keeps is behind a `RefCell` (or a `Mutex`),
  borrowed for a read or a write and never across a call to the host. A nested call into the same addon works;
  a nested write while a borrow is open would panic (loud, not a silent difference).
- The lib's `handler_cell_fits_inside` named `enderdrives::is_disk`; the row now marks its disks
  (`Defs::add_cell_kept_out_of_cells`) and the lib asks the registry. Same answer, no addon in the lib.

`horizon-client`'s and `-lib`'s `ae2-enderdrives` features: the lib's is gone, the client's gates nothing (as the
booster's).

Checks at this step: `build.ts` 58 warnings (as the base); units as at the pilot (client bin 1, lib 1613,
client-lib 2544 + the one known failure, command 59), the new crate 11, the booster's 3; default-features check,
layers and `xtask mod-api` clean; world: `enderdrives-disks` (9 checks: two networks share a storage, the
duplicate guard, drain, config, restart) and `aeinfinitybooster-range` (5, now through the shared extension).
An edit of `horizon-ae2x-enderdrives/src/game.rs` builds in 2.5 s (`fast`, `-j 6`, quiet machine, the program
included).


### Extension point v2 (no addon moved)

The addons after Ender Drives each need kinds of extension point the hooks above do not cover. v2 adds them,
each small and `dyn` or a registry; no addon uses them yet, so the game is the same:

- **Static rules** (`horizon_client::ae2x::rules`, re-exported by `ae2_ext`): `AddonRules`, one `&'static dyn`
  per addon (`install_rules`, `rules(id)`), for what the lib's and the game's free functions ask an addon with no
  game at hand: `reset_unsaved`, the fixed cells and handler cells, an item's name, the pack's content, and the
  tables by item id the renderer and the screens read (`part_model`, `part_boxes`, `part_style`,
  `part_menu_title`, `bus_operations_multiplier`, `display_part_screen`), the machines' slots, progress and menus.
  The place that asked `<addon>::f(...)` asks `rules("<id>")` in the same order.
- **Screens**: `Ae2Addon::screen()` gives an addon's own window (`Ae2AddonScreen`: size, slots, draw, press,
  scroll, tooltip, hotkey, payload, agent: the `ae2x_screen_*` chain), `Ae2Addon::terminal()` its buttons and
  sub-screens on AE2's terminals (`Ae2TerminalWidgets`; the terminal's `Widget`/`Sub` will carry the addon's
  id and its own number).
- **Key types at startup**: the lib's `ae2_key_types::register` is public already (an addon's `install` calls
  it); the world strategies of an addon type are `add_key_strategies(kind, owner, kinds)` (which ones it has,
  read with no game) plus `Ae2Addon::key_strategies()` (`Ae2KeyStrategies`: import, export, external storage,
  placement, through the host).
- **Keybinds** (`horizon-client-input`): `keybinds::add_mappings(rows, category)` adds a mod's mappings after
  `MAPPINGS` and its category after `CATEGORIES`; `all_mappings`, `options_mappings` (what `options.txt` holds)
  and `category_order` include them. With nothing added the lists are the same as before.
- **Addon to addon**: `Ae2Host::call_addon(id, what, args)` reaches `Ae2Addon::call` of another addon (or of
  one above, from the code still inside horizon-client), arguments and answer as `dyn Any`.
- **Host**: `world_mut`, `spectator`, `shift_down`, `held`, `consume_held`, `block_state_name`,
  `block_registry`, `level_set`, `provider_patterns_changed(_local)`, what the kits and the two big addons use
  most.

What v2 leaves to the moves: the game-typed static questions (`orientation`, `part_view`/`part_tools`,
`bus_tick` with a `dyn TransferHost`, the machine screens' `layout`/`style_slots`) need their types made public in
their fork-only modules first, as `GridAddress` was; and each hook gets its `Game::ae2_addon_<hook>` wrapper when
its first caller moves (an unused one would be a warning).

### Step 1, the kits (`horizon-ae2x-kits`)

ExtendedAE's and Advanced AE's upgrade kits are shared by the two addons, so they move first. The kit rows' type
and what a kit makes of a device (`Kit`, `upgraded`, `upgraded_part`: pure, lib types only) went to the lib's
`ae2x::kits`, where the two addons' tests still reach them; the use of a kit on a device (`kit_use`: sneaking,
the held kit, the block or part swapped with what it held, the provider's patterns announced) went to the crate,
which installs an addon `"kits"` into AE2's extension. The two addons, still inside horizon-client, call it with
v2's addon-to-addon kind, `Game::ae2_addon_kit_use` = `call_addon("kits", "use", (KITS, pos, face))`, in the
place they called `ae2x_kit_use`; once they sit above, they call `horizon_ae2x_kits::kit_use` directly. The
crate is built in with either addon's feature of horizon-atm10.
