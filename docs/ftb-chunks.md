# FTB Chunks, FTB Teams and JourneyMap on this fork

> **Handoff (2026-11-13, branch `feat/ftb-chunks`, worktree `atm10-wt/megacells`): slices 1 and 2 are done, slice 3 is next.**
>
> **Done.**
> - *Slice 1, FTB Teams* (`pack/mods/ftbteams`, commit `FTB Teams, slice 1: teams, invites and allies`): teams, parties,
>   invites, allies, server teams, team chat, the files, the client's copy over `ftbteams:sync`. World test
>   `ftbteams/teams`.
> - *Slice 2, FTB Chunks server* (`pack/mods/ftbchunks`, commit `FTB Chunks, slice 2: claims, protection and
>   force-loading`): crate `ftbchunks-core` (no dependencies: a bundle crate may not path-depend on another bundle's, so
>   the teams come in as `TeamView`s through trait `Teams`; config, team properties, limits, claims, force-load
>   bookkeeping, protection policy, wanted tickets, `/ftbchunks` in `commands.rs`; 8 unit tests), runtime
>   `runtime/ftbchunks_world.rs` (FTB Teams' manager as `Teams`, `<world>/ftbchunks/<team>.snbt` and
>   `ftbchunks-world.snbt`, the protection hooks, tickets through `native_force_chunk_in` owned by
>   `ftbchunks:<team>`, `/ftbchunks_probe` behind `FTBTEAMS_DEV=1`), 16 hooks in `horizon-mixin.toml`. FTB Teams'
>   runtime queues its events (`ftbteams_world::take_events`), which FTB Chunks drains each server tick (tickets are
>   re-checked at once after an event or a command, else every 100 ticks).
>   World test `ftbchunks/claims` (`pack/mods/ftbchunks/tests/claims.ts`, about 4 minutes) passes: a non-ally's break
>   and place are refused in a claim and a member's go through; 500 claims and 25 force-loads, the 501st and 26th
>   refused; a furnace keeps smelting in a force-loaded chunk with the member online 400 blocks away; the claims,
>   force-loads and tickets come back after a restart; the tickets are let go when nobody of the team is on (FTB
>   Chunks' defaults: no member has `ftbchunks.chunk_load_offline`, `force_load_mode DEFAULT`; the port keeps that and
>   the test checks it); the admin bypass; `unclaim_all`.
>
> **Next: slice 3, the FTB Chunks client** (map screen with drag claim selection, the minimap, waypoints), in this order:
> 1. *The server half the client needs* (none of it is written; channels are `ftbchunks:<id>`, as FTB Teams' are, over
>    `native_net_send_to_player` (30 000 bytes a message, 32 a tick) and the `net_from_client` / `net_from_server`
>    hooks; `ftbteams_world::from_server` and `hooks/net_from_server.rs` are the pattern, the wire is JSON as there).
>    The payload list is in `ftb-work/notes/ftbchunks-spec.md` section 8 (local, from the jar). Needed first:
>    `login_data` (the world's team-manager id: the client's map folder), `send_general_data` (claimed, loaded and
>    the two maxima, on login and after each change), `send_many_chunks` / `send_chunk` (a chunk: x, z, claimed,
>    seconds since the claim, force-load flag and time, expiry; broadcast, or to allies only when the team hides its
>    claims), the C2S `request_chunk_change` (op CLAIM, UNCLAIM, LOAD, UNLOAD; the chunk set in the player's own
>    dimension; admin flag; optional team id taken only for an operator or a member) answered by
>    `chunk_change_response` (changed, total, problems by id), then `update_force_load_expiry`,
>    `teleport_from_map` (operators), `player_death`, `add_waypoint`, `open_claim_gui` and the 2-second action-bar
>    notice on entering a claim. `Chunks::{claim, unclaim, load, unload}` in the crate are the operations; the message
>    handler loops them over the set and counts `Problem`s (`Problem::id`) for the response. Add a unit test per
>    message to the crate and a client probe, as slice 1's `ftbteams_probe` does.
> 2. *The map tile API* (gap 1 below), then the client's region cache (512x512 regions kept under
>    `local/ftbchunks/data/<manager id>/<dimension>/`).
> 3. *The screens*: the large map (M: pan, zoom, centre on the player, the context menu), the claim screen over it
>    (left drag claims, right drag unclaims, Shift forces and releases a force-load; one `request_chunk_change` on
>    mouse release; freehand, rectangle and circle selection; the 3x3 test in the plan's slice 3), claim overlays in
>    team colours, then the minimap (off on ATM10 by default: JourneyMap is present), then waypoints (`/ftbchunks
>    waypoint add`, the map's menu, death waypoints, `waypoints.json` per dimension). Hooks exist: `mod_screen_open`,
>    `screen_draw`, `screen_click`, `screen_mouse_dragged_pre`, `screen_mouse_scrolled_pre`, `mod_key` (M, J, N, B),
>    `hud` / `gui_layer_post`, `map_decorations`.
> 4. The world test for the slice: drag a 3x3 area on the claim screen with real mouse events, then read the server's
>    claims with `/ftbchunks_probe`, a screenshot; a death leaves a waypoint.
>
> **Still open from slice 2** (detail in `pack/mods/ftbchunks/NOTES.md`): PvP modes, mob griefing, fire and fluid across
> borders, `/ftbteams party settings ftbchunks:*` (FTB Teams' `Prop` is a closed enum: it needs an open property
> registry in `ftbteams-core`), idle and force-load expiry, farmland trampling, the finer item rules.
>
> **API gaps** (add the smallest way through `tools/native-hook-fragments/`, docs/atm10-port.md "Porting rules", only
> when a slice needs it):
> 1. Map tiles for mods: a client `native_map_tile(dimension, chunk)` over the minimap extra's scan cache, and a dynamic
>    RGBA texture upload that a screen can draw (`mod_image` is for files). Slice 3 needs both.
> 2. A cancellable mob-griefing check (endermen) and `native_living_incoming_damage` with the `Game` (PvP in claims).
> 3. Spawning near force-loaded chunks (`anyPlayerCloseEnoughForSpawning`).
> 4. Entity type tags on the server (the interact and attack whitelists; `corpse:corpse` is built in).
> 5. Fake players (`native_is_player_fake` exists; machines that break blocks without a player never reach
>    `destroy_block`).
>
> **How to run.** `atm10-work/ftb-work/ftb.sh [units] [stage] [build] [world]` (this PC's paths for `bun run
> pack/scripts/ftbchunks.ts <step>`; `wait-slot.sh 3`, `-j 4`; export `AE2_PROFILE=iter`). `stage` applies only the
> FTB bundles and `build` compiles the client; do not edit sources while either runs (`E_STALE_SOURCE`). World tests:
> `FTB_LIST=ftbchunks/claims ftb.sh world` (or `ftbteams/teams`; the default is every bundle's); logs in
> `ftb-work/logs/world-<name>.log`, a failed run's folder in `ftb-work/logs/fail-<name>/`. Unit tests alone: `cargo test
> --offline --manifest-path pack/mods/<bundle>/<crate>/Cargo.toml --target-dir ftb-work/target-tools`, then delete the
> crate's `Cargo.lock`. The second player in a test is FTB Teams' dev fake player (`/ftbteams_add_fake_player Bob`,
> `FTBTEAMS_DEV=1`). Extracted jars: `ftb-work/jars/`; notes from them: `ftb-work/notes/`.
>
> **Licence rule for every slice.** The FTB mods are All Rights Reserved and this repository is public: no line of
> their language files, no chat or command text, no GUI label, no method body is committed. A message is
> `Text::translate_with_fallback(key, fallback, args)` with the mod's own key, so the game shows the mod's line from
> the lang file the player's jar brings, and the fallback is our own plain wording. Keys, command syntax and behaviour
> stay the mod's. Tests assert the fallback, or a regex for a vanilla key.

The plan for ATM10's map and chunk-claiming mods: FTB Teams 2101.1.11, FTB Chunks 2101.1.22, FTB Library 2101.1.39
and JourneyMap 1.21.1-6.0.9. Behaviour is ported from the jars (read with `javap`); no file of theirs is committed.
The detailed notes taken from the jars (command trees, messages, file keys, packets) are local, in the handoff
folder's `ftb-work/notes/` (`ftbteams-spec.md`, `ftbchunks-spec.md`, `journeymap-spec.md`).

ATM10 ships no config for any of them, so their defaults apply. The pack's only change is a KubeJS tag script that
adds every Waystones block to `ftbchunks:interact_whitelist`; `kubejs/assets/{ftbchunks,journeymap}` hold only a
`pt_br` translation. FTB Ranks defines no FTB Chunks node, so the limits are the config's: 500 claims and 25
force-loaded chunks a player.

## What players use

- **FTB Teams.** Every player has a personal team from their first login. `/ftbteams party create` makes a party;
  officers invite (the invitee gets accept and decline links) and kick; the owner transfers ownership and edits the
  properties (name, description, colour, free to join). Allies are players a party trusts, one way. Team chat
  (`/ftbteams msg`, `redirect_chat`). Admins: server teams, `force-add`, `force-remove`, `force-disband`. The
  team screen is the mod's GUI over the same operations.
- **FTB Chunks.** A team claims chunks by dragging on the claim screen (left drag claims, right drag unclaims, with
  Shift they force-load and un-force-load; one request on mouse release). Claims protect against non-allies:
  breaking, placing, using blocks, using items (buckets, leads, name tags, harmful potions), entities, explosions,
  enderman griefing, pistons across borders; PvP stays on (`pvp_mode ALWAYS`). Force-loaded chunks keep machines
  running while the team is away, which ATM10's automation depends on. Limits: `max_claimed_chunks` 500,
  `max_force_loaded_chunks` 25, a party's from its members' (`party_limit_mode LARGEST`). The full-screen map (M),
  the minimap (off by default when JourneyMap is installed, as on ATM10), waypoints, death waypoints, and the
  action-bar notice on entering a claim.
- **JourneyMap.** Full-screen map (J), the minimap (Ctrl+J, two presets: round, 30%, top right, north up), day,
  night, underground, topo and biome modes, the waypoint manager (N, B creates one), death waypoints, waypoint
  sharing in chat. It works client-only; a server with JourneyMap unlocks its server options.
- **FTB Library.** What the others use of it: SNBT files and its server config layering
  (`<instance>/config/<name>.snbt` under `<world>/serverconfig/<name>.snbt`), `Color4I` colours, split packets.
  Its UI toolkit is not ported: the screens are drawn with Horizon's own GUI hooks.

## Where the code lives

Three native bundles under `pack/mods/`, as Mekanism, Powah and Flux Networks are, each separable:

| Bundle | Holds | Needs |
|---|---|---|
| `pack/mods/ftbteams` | crate `ftbteams-core` (teams, ranks, properties, commands, SNBT files, the sync messages; FTB Library's SNBT and colour parsing as `ftbteams_core::lib`), runtime `ftbteams_world` (server and client halves) | hooks only |
| `pack/mods/ftbchunks` | crate `ftbchunks-core` (claims, limits, protection policy, force-load bookkeeping, config), runtime `ftbchunks_world`, then the map screen and minimap | `ftbteams` (path dependency on `ftbteams-core`, calls `crate::game::ftbteams_world`) |
| `pack/mods/journeymap` | crate `journeymap-core` (waypoints, presets, options), runtime with its screens | the map tile API below |

`pack/scripts/ftbchunks.ts` builds and tests all three (`units`, `stage`, `build`, `world`), as `mekanism.ts` does.
**Text and licence.** FTB Teams, FTB Chunks and FTB Library are All Rights Reserved and this repository is public, so
no line of their language files, no chat or command text and no GUI label is committed. Every message is
`Text::translate_with_fallback(key, fallback, args)` with the mod's own translation key; when the player's instance
carries the mod's jar (lang files and GUI sprites come through `horizon-port-pack`, like every content mod's) the game
shows the mod's own wording from that file, and only a missing key shows the fallback, which is plain wording of ours.
Keys, command syntax and behaviour stay the mod's: they are behaviour, not text.

## Hooks

Everything slice 1 and 2 need exists in `native-hooks.json`:

- Teams: `world_lifecycle` (load/drop), `player_logged_in`/`player_logged_out`, `register_commands`/`perform_command`,
  `world_saved` (FTB Teams saves on each level save), helpers `native_tell`, `native_feedback`, `native_server_players`,
  `native_profile_by_name`, `native_world_dir`, `native_net_send_to_player` (30 000 bytes a message, 32 a tick), and
  on the client `net_from_server`, `command`/`commands` (a client probe for tests).
- Protection: `destroy_block` (BreakEvent), `left_click_block`, `right_click_block` (interact, and placing: FTB checks
  EDIT_BLOCK on a block item there), `use_item_on` (claims a placement), `use_item` (RIGHT_CLICK_ITEM), `fill_bucket`
  (EDIT_FLUID), `interact_entity`, `armor_stand_manipulate`, `attack_entity` (non-living targets: frames, boats,
  minecarts), `explosion_detonate` (take claimed positions out of `blocks`), `piston_move`, `block_spread` and
  `fire_odds` (fire across borders, off by default), `living_incoming_damage` (PvP modes, off by default),
  `entity_change_block` (enderman griefing), `action_bar` for the 2-second notice when an action is refused.
- Force-loading: `native_force_chunk(owner, chunk, hold)` / `native_force_chunk_in` / `native_forced_chunks`
  (vanilla's tickets, owned by `ftbchunks:<team>`), so `/forceload` stays upstream's and both coexist.
  FTB Chunks also counts force-loaded chunks as a player being close, for spawning (`ChunkMapMixin`): a gap, listed below.
- Screens: `mod_screen_open`, `screen_draw`, `screen_click`, `screen_mouse_dragged_pre`, `screen_mouse_scrolled_pre`,
  `mod_key` (M, J, N, B), `hud` / `gui_layer_post` for a minimap, `map_decorations`.

Gaps, to add the smallest way through `tools/native-hook-fragments/` (docs/atm10-port.md, "Porting rules") when the
slice needs them:
1. **Map tiles for mods.** The minimap extra (`horizon-client-render/src/minimap.rs`) already scans one 16x16 tile per
   loaded chunk (map colour, height, water depth) and composes a fixed 128x128 image around the player. The big maps
   need the tiles themselves: a client helper `native_map_tile(dimension, chunk) -> Option<MapTile>` reading the same
   cache (and asking a scan for a chunk not yet scanned), so FTB Chunks and JourneyMap keep their own region cache of
   explored chunks (FTB: 512x512 regions; JourneyMap: 512x512 PNG tiles), and a helper to upload an RGBA image to a
   mod texture and draw it in a screen at a scale (`mod_image` exists for files; a dynamic one is needed).
2. **Spawning near force-loaded chunks** (`anyPlayerCloseEnoughForSpawning`): a hook or a ticket flag.
3. **Fake players** (`PlayerHooks.isFake`): `native_is_player_fake` exists; machines that break blocks without a
   player (AE2's annihilation plane, Mekanism's digital miner) don't reach `destroy_block` at all, so `fake_players
   CHECK` only matters for mod bots. Listed, not needed first.

## Tiles shared with the minimap extra

One scanner, three views. The minimap extra's tile (`MapColor`, height, water) is the unit. FTB Chunks' region cache
keeps per-column colour, height and water flags in 512x512 regions saved under the game dir
(`local/ftbchunks/data/<manager id>/<dimension>/`), filled from `native_map_tile` as chunks load and kept for chunks
that unload, which the minimap extra forgets. JourneyMap keeps its own folder and modes (day/night/caves/topo/biome)
over the same tiles; caves need the per-height slice the extra already computes underground. FTB Chunks' own minimap
is off by default on ATM10 (JourneyMap present), so JourneyMap's minimap is the one players see; both draw through
`gui_layer_post` with the composed image, not the extra's fixed corner map.

## Teams: storage and sync

As FTB Teams 2101.1.11 stores them, so a world from Java keeps its teams: `<world>/ftbteams/ftbteams.snbt` (manager id,
`chat_redirected`), `player/<uuid>.snbt`, `party/<uuid>.snbt`, `server/<uuid>.snbt`, a deleted team's file moved to
`deleted/`. The team type comes from the folder; a player's party is not stored but rebuilt from the parties' member
ranks. Written on each world save and at once when a team is created or deleted. Ranks: enemy -100, none 0, ally
50, invited 75, member 100, officer 500, owner 1000; invites and allies are entries in a party's ranks.

Clients get the teams on the channel `ftbteams:sync` (JSON, split under 30 000 bytes): a full sync at login, deltas
after a change, each personalised (the player's own team with all its properties, the others with name and colour,
everyone's ranks), and `ftbteams:presence` when a player comes or goes. The client keeps a `ClientTeamManager` the map
and the claim screen read (claim colours, ally checks).

## Slices

1. **FTB Teams core** (server + sync): teams, invites, allies, transfer, kick, leave, settings, server teams, team
   chat and redirect, `/ftbteams` with Java's syntax and message keys, the files, the client's copy. *World test*
   `pack/mods/ftbteams/tests/teams.ts`: one player and a dev-only fake player (`/ftbteams_add_fake_player`, as in
   Java's dev environment, behind `FTBTEAMS_DEV=1`), every command's answer, the files after a restart, the client's
   copy through a client probe. The crate's unit tests cover the multi-player rules.
2. **FTB Chunks server** (done): claims and force-loading per team (`<world>/ftbchunks/<team>.snbt`), limits,
   `/ftbchunks claim|unclaim|load|unload|unclaim_all|unload_all|info|admin ...`, protection through the hooks above,
   force-load tickets. *World test* `pack/mods/ftbchunks/tests/claims.ts`: claim by command, a fake non-ally is refused
   breaking and placing while a member is not; limits refuse the 501st claim and the 26th force-load; a force-loaded
   furnace keeps smelting with the member 400 blocks away; the claims and tickets after a restart; the tickets go
   when nobody of the team is on; the admin bypass. Left for slice 3 with the client: `request_chunk_change`,
   `send_many_chunks` and the action-bar notice; left in `pack/mods/ftbchunks/NOTES.md`: PvP, mob griefing, fire and
   fluid borders, `ftbchunks:*` team properties, expiry.
3. **FTB Chunks client**: the map tile helper (gap 1), region cache, the large map (M) with pan and zoom, the claim
   screen with drag selection sending one request, claim overlays in team colours, waypoints (`/ftbchunks waypoint
   add`, the map's menu), death waypoints, the minimap (off on ATM10 by default). *World test*: drag a 3x3 area on the
   claim screen with real mouse events, then the server's claims, a screenshot; a death leaves a waypoint.
4. **JourneyMap**: full map, minimap presets, waypoint manager, death waypoints, chat sharing, its modes over the
   shared tiles. *World test*: screens open with their keys, a waypoint made with B survives a restart.

## Handoff

The handoff for the next slice is the block at the top of this file. Slice 1 is in `pack/mods/ftbteams`
(`NOTES.md` there), slice 2 in `pack/mods/ftbchunks` (`NOTES.md` there).
