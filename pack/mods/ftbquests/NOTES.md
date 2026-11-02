# Quest book: notes

Roadmap item 1.7. Branch `feat/quests`, worktree `wt-quests`, on 85020d24a.

Run it: `ATM10_DIR=... bun run pack/scripts/ftbquests.ts content units stage build world` (targets on `C:/horizon-work`:
`target-quests-game`, `target-quests-tools`, `stage-quests`, `content-quests`; screenshots in `C:/horizon-work/shots-quests`).
`FTBQ_TESTS=book-flow` picks one world test; `FTBQ_JOBS=2` when memory is short (LLVM ran out of memory at 3 jobs).

## State

- `ftbquests-core`: SNBT, book, lang, filter, progress, names, graph, wire. `tests/atm10_files.rs` reads all 920 quest
  files of the instance and compares against line scans of the files.
- `ftbquests-content`: the GUI textures from the player's FTB Quests and FTB Library jars, into the local content mod
  `ftbquests_gui` (its own namespace: a content mod may only add files to namespaces it owns, and the carry owns
  `ftbquests`). The book item, its model and the mod's language files come from the ATM10 carry
  (`ports/atm10/mods/ftbquests/carry`).
- Engine: API 64 in this branch (screen hooks and helpers, see `docs/native-mods.md`). Other branches also took 64
  (mekanism, powah): renumber at merge. Names and signatures of `native_mod_screen_open/close`, `native_mod_screen`,
  `native_screen_render_post`, `native_screen_mouse_*_pre`, `native_screen_key_pressed_pre`, `native_quad` and
  `NativeScreenView` match upstream's (their API 78 to 111), so at the upstream merge take upstream's and drop ours;
  there the hooks also fire over other screens, and the mod checks `native_mod_screen()`. `Screen::Mod` also stops being
  behind `ae2-lab`, here as upstream.
- World tests: `tests/book-flow.ts` (the test's own small book: every rule) and `tests/book-atm10.ts` (the real files).

## Decisions

- Per player, not per team: ftbteams isn't carried. The file has FTB Quests' fields (`task_progress`, `started`,
  `completed`, `claimed_rewards`) in `<world>/ftbquests/<uuid>.snbt`, written the tick something changes.
- `detection_delay`: the inventory is fingerprinted every 10 ticks; a change runs detection after the delay (60 ticks in
  ATM10), as `FTBQuestsInventoryListener` defers. There is no inventory-changed hook.
- The quest files are read in place: the game's own `config/ftbquests/quests` first, so a test can bring its own book,
  then `ATM10_DIR`.
- The mod's key is unbound by default, as the jar ships it (`KeyMappingConfig` with key -1): Options > Mods binds it.
- FTB Quests' chapter list opens on hover unless pinned; here it starts pinned, with the pin button to unpin.
- Hooks take `&Game`: the screen keeps its state in a thread local, and leaves what must change the game for
  `native_frame`.
- `FTBQ_VIEW_FILE`: with it set, the screen writes what it shows to that file, for tests to click real places.

## Left

- Task types: kill, dimension, biome, structure, advancement, observation, location, xp, forge_energy show locked with
  their real text. Roadmap 3.5 and the hooks each needs (`native_mob_killed`, `native_player_tick` position reads).
- Rewards: xp, xp_levels, random, choice, loot, command, toast show locked with their real text. Next: xp and tables.
- questpics (176 MB, 1,207 files under `kubejs/assets/atm`) aren't in the GUI atlas: too big for it. Chapter images and
  description images whose sprite exists draw (`minecraft:block/*`, carried mods' `item/` and `block/`), others are
  skipped. Real pictures need a texture loaded at run time.
- FTB Library's sidebar button on the inventory: no hook over the inventory here; upstream's `screen_render_post` will
  give it after the merge.
- `only_from_crafting` isn't detected (no crafting hook used; ATM10 has none). `can_repeat` and timers aren't worked (one
  quest in ATM10). Text click events (`change_page`, `open_url`, 9 lines) show as text. View-dependencies buttons, search,
  pinned quests, the quest toast (a line over the hotbar instead), per-team progress, chapter images' hover text.
- Component matching (`match_components`, 223 tasks) compares what `encode_components` gives; items of mods that aren't
  carried can't be checked yet.
- `ftbfiltersystem` nodes beyond item, item_tag, mod, block, component, or, and, not read as matching nothing (none in
  ATM10).
