# FTB Teams: where the work stands

Slice 1 of docs/ftb-chunks.md. Build and test: `bun run pack/scripts/ftbchunks.ts units stage build world`
(this PC: `atm10-work/ftb-work/ftb.sh`).

## On upstream's hooks (merge of nightly-source/2026-11-18)

- The teams are read at `server_started` (where the fork's `WorldOpenReady` was) and written and dropped at
  `world_lifecycle`'s close phases.
- Upstream's command dispatcher parses each usage's arguments itself: its `word` takes no `:` or `#` (property ids,
  team short names) and `player_name` is gone. So `/ftbteams` is registered as `ftbteams [line:text]` (and the dev
  commands the same way), `run_mod_command` rebuilds the line and the rules parse it and check the operator levels
  as before. The client's command tree no longer completes the subcommands.
- The client's copy of the teams needs the bundle messages (`native_net_send_to_player`, `net_from_server`), which
  the merged engine does not carry yet (HANDOFF "S3", N1).

## Open properties (slice 6)

`Manager::register_property(PropertyDef)` is `TeamCollectPropertiesEvent.add`: other mods' properties (FTB Chunks' `ftbchunks:*`)
are known to `/ftbteams party settings|settings_for` and `server settings`, kept in the team's file, and shown with their
registered default. Not done: the team screen and the client sync of them (only `display_name` and `color` go to all, FTB's
own rule; the rest would go to the team's members).

## Not done yet

- Chat redirection is stored and toggled, but chat isn't diverted yet: `player_chat` is a lib hook with no `Game`,
  so it needs a queue the server tick drains, or a hook on the server side.
- The team screen and its operations (promote and demote are only there; `set_officer` is ready for it).
- `limited_lives`: the lives count is kept, deaths don't take one (`player_died`), the out-of-lives kick isn't there.
- `/ftbteams nbtedit` and `serverconfig` (FTB Library's GUIs).
- Admin feedback to other operators (`sendSuccess(.., true)`), team name completions (`native_tab_complete_v2`).
- Single player's "admin without cheats" (`isSingleplayer()`): the admin usages need level 2 here.
- `party create` with a name keeps Java's lack of a length check; the GUI path (description, colour, invites at
  creation) waits for the screen.
