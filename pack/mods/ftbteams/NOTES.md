# FTB Teams: where the work stands

Slice 1 of docs/ftb-chunks.md. Build and test: `bun run pack/scripts/ftbchunks.ts units stage build world`
(this PC: `atm10-work/ftb-work/ftb.sh`).

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
