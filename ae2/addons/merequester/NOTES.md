# ME Requester port: state and next steps

Branch `feat/merequester`, worktree `wt-req`, target `C:/horizon-work/target-req`. Build and test with
`C:/horizon-work/req-build.sh`, `req-test.sh merequester-stock` (uses
`AE2_ADDON_CONTENT_ROOT=C:\horizon-work\addon-content`, made by `prepare-addon.ts merequester`) and
`req-world.sh` (the other AE2 suites with the main checkout's content).

## Why no job started (fixed)

`merequester-stock` stopped at "the row shows Crafting" with known 50, target 100, batch 64. The row's
note said `no calculation: node service unavailable`: the ordered event replay had no node for the
requester, while the plain network analysis did, so the requester looked online. Three places only knew
AE2's own blocks: `ae2_event_replay::networked`, `Game::ae2_event_device` and the `is_ae2` state filter in
`ae2_sync_events` (an `ae2:` prefix, which dropped the block state and paused readiness with "ready node
state unavailable"). All three now take addon machines, so any later `MachineKind::Ext` gets this too.

## Open

- Terminal style button: waits for `feat/ae2-terminal-screens` (`ClientSettings.terminal_style`,
  `save_terminal_style`). Rows are ATM10's FULL until then; switch `merequester_rows` to the setting after
  that merges.
- Addon guide pages: `merequester.md` isn't loaded, the help button opens the guide's home. MEGA has the
  same gap; `prepare-addon.ts` lists `ae2guide` under later.
- The scrollbar handle can't be dragged; no fluid-container tooltip; no JEI drag and drop; no wireless
  terminal (AE2WTLib); no number field validation messages.
- One run of the world test ended with `read ECONNRESET` 34 commands in, with about 2 GB of RAM free and no
  panic in the log. Cause not found. The harness fix for a reset control socket is on the terminals branch
  (`e2529c42c`). The next runs passed.
