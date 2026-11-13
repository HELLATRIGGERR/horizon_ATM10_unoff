# FTB Teams on this fork

FTB Teams 2101.1.11 for All the Mods 10, as a native mod of this fork: the teams FTB Chunks' claims belong to.
It is this folder's bundle (`horizon-mixin.toml`, `hooks/`, `runtime/`) and a crate of the rules (`ftbteams-core`).
`pack/scripts/ftbchunks.ts` builds and tests it with the other FTB bundles; `NOTES.md` says where the work stands;
[docs/ftb-chunks.md](../../../docs/ftb-chunks.md) has the plan for the whole port.

## Licence

FTB Teams, FTB Library and FTB Chunks are All Rights Reserved (FTB). Nothing of theirs is copied here: the crate
is a reimplementation of what the mod does, written from its behaviour, and no line of its language file, no chat
or command text and no GUI label is in this folder. Every message carries the mod's own translation key
(`Text::translate_with_fallback`, as the Mekanism bundle does), so a player whose instance has the mod's jar reads the
mod's own lines from the lang file that comes through `horizon-port-pack`. Only when a key is missing does the game
show the fallback, which is plain wording written for this port. Command syntax and behaviour are the mod's. This
bundle stays in this fork.

## What there is

- Every player's personal team, made at their first login (their name, a random colour: a random hue at
  saturation 0.65, full brightness); parties with an owner, officers, members, invites and allies (one way, per
  player), server teams.
- `/ftbteams` with FTB Teams' syntax, rank checks and answers: `party create|join|decline|leave|invite|kick|
  transfer_ownership|transfer_ownership_for|settings|settings_for|allies add|remove|list`, `server create|delete|
  settings`, `msg`, `info [team]`, `info server_id`, `list [parties|server_teams|players]`, `force-disband`,
  `force-add`, `force-remove`, `redirect_chat`, `teamstage`. A usage the player's rank doesn't allow answers as an
  unknown command, as the mod's `requires` makes it.
- Properties `ftbteams:display_name|description|color|free_to_join|max_msg_history_size|team_stages|
  lives_remaining`; other mods' properties in a file are kept. `ftbteams-server.snbt` (`limited_lives`,
  `max_party_size`, both 0) from the instance's `config/`, under the world's `serverconfig/`.
- Team chat and its history (`<Name @Team>text`).
- The files as FTB Teams writes them: `<world>/ftbteams/ftbteams.snbt`, `player|party|server/<uuid>.snbt`, a
  deleted team's file moved to `deleted/`; written on each world save and at once on a creation or a deletion.
  A Java world's teams load.
- Clients get the teams (`ftbteams:sync`, `ftbteams:presence`): all at login, then each change; their own team
  with every property, the others with name and colour, everyone's ranks. FTB Chunks reads them for claim colours.

Tests: the crate's unit tests (several players), and `tests/teams.ts` in a world (`FTBTEAMS_DEV=1` gives Java's
development command `/ftbteams_add_fake_player` and the probes `/ftbteams_probe`, `/ftbteamsclient`).
