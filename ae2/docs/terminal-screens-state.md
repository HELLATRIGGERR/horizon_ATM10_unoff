# feat/ae2-terminal-screens: state

Base commit 14c934e65 (the screens branch builds on it). Commits go on top.

## Done, on top of 14c934e65
- Client side of the server changes an earlier pass left: rows no longer capped by the menu (`Layout.places`, `ae2:repo_view.extra`, `REPO_EXTRA_WIRE`), `ae2:terminal_scroll` carries the rows, shift-wheel, fluid entries draw their fluid (wrapped generic stack), craftable mark on encoding inputs.
- `game/ae2_terminal_settings.rs`: TerminalSettingsScreen (cog) and KeyTypeSelectionScreen.
- Harness: `start()` and the peer keep polling through a control timeout or reset while the game lives, and report the exit code when it died. The peer talks TCP on Windows.
- `portable_idle_step` ignored usageMultiplier: real bug, fixed (`portable_idle_drain`, unit test).

## portable-menu-lifecycle findings
- "5 AE instead of 15": deterministic bug, the idle drain did not apply `PowerMultiplier.CONFIG`.
- "read ECONNRESET": the game process died silently while opening the world (19:58:48). Windows logged event 2004 "low virtual memory" at 19:55:34 (dota2 6.2 GB, rustc 2.1 GB) and popup 26; rustc died with "memory allocation failed" at 19:55:57 and a peer died silently at 19:52:41. Not reproduced with free RAM; see the stress results.
- "Control timeout: status": the harness treated a slow world open as a failure.

## Next
Run suites: terminal-screen-views, terminal-search, terminal-craft-gestures, wireless-key-types, wireless-source-components, crafting-alternatives, crafting-fluid-template, inframe-terminal, inframe-crafting-matrix, inframe-crafting-status, inframe-pattern-encoding, machine-screens, portable-menu-lifecycle. Look at screenshots. Delete this file in the final state.
