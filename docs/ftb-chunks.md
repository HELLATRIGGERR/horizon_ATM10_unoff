# FTB Chunks, FTB Teams and JourneyMap on this fork

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
2. **FTB Chunks server**: claims and force-loading per team (`<world>/ftbchunks/<team>.snbt`), limits,
   `/ftbchunks claim|unclaim|load|unload|unclaim_all|unload_all|info|admin ...`, protection through the hooks above,
   the team properties FTB Chunks adds (`ftbchunks:allow_explosions`, `allow_mob_griefing`, block edit/interact and
   entity modes, claim and location visibility), force-load tickets, idle expiry, the action-bar notices, the
   `request_chunk_change` message and `send_many_chunks` sync. *World test*: claim by command and by the message,
   a fake non-ally is refused breaking, placing, using a chest and blowing TNT while a member is not; a force-loaded
   chunk keeps a hopper moving with the player 300 blocks away; limits refuse the 501st claim; all after a restart.
3. **FTB Chunks client**: the map tile helper (gap 1), region cache, the large map (M) with pan and zoom, the claim
   screen with drag selection sending one request, claim overlays in team colours, waypoints (`/ftbchunks waypoint
   add`, the map's menu), death waypoints, the minimap (off on ATM10 by default). *World test*: drag a 3x3 area on the
   claim screen with real mouse events, then the server's claims, a screenshot; a death leaves a waypoint.
4. **JourneyMap**: full map, minimap presets, waypoint manager, death waypoints, chat sharing, its modes over the
   shared tiles. *World test*: screens open with their keys, a waypoint made with B survives a restart.

## Handoff

Slice 1 is in `pack/mods/ftbteams`; `pack/mods/ftbteams/NOTES.md` says what is done and what is next.
