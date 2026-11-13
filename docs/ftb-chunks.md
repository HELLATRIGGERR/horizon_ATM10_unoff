# FTB Chunks, FTB Teams and JourneyMap on this fork

> **Handoff (2026-11-13, branch `feat/ftb-chunks`, worktree `atm10-wt/megacells`): slices 1 to 3 are done and pass; the
> next is slice 4: waypoints, the death waypoint and the corner minimap (FTB Chunks' client, second half).**
>
> **Done.** *Slice 1, FTB Teams* (`pack/mods/ftbteams`, world test `ftbteams/teams`, 29 checks). *Slice 2, FTB Chunks
> server* (`pack/mods/ftbchunks`: crate `ftbchunks-core`, runtime `runtime/ftbchunks_world.rs`, world test
> `ftbchunks/claims`, 22 checks, about 2 minutes). Details of both in each bundle's `NOTES.md`. *Slice 3, the claim
> map* (one commit after `4282a27368`), below.
>
> **Slice 3, the claim map.**
> - *Crate* `ftbchunks-core/src/net.rs` (depends on serde and serde_json): channels `ftbchunks:login_data`,
>   `send_general_data`, `send_many_chunks`, `request_chunk_change`, `chunk_change_response`; `Op::create(left,
>   shift)`; `Chunks::change` (the op over the chunk set, problems counted by `Problem::id`), `general_data`,
>   `many_chunks` (grouped by dimension and team, 400 a message, a team's claims hidden as unclaimed from players its
>   `claim_visibility` excludes), `changed_since`. The crate's 12 unit tests pass (4 are this slice's).
> - *Server* (`ftbchunks_world.rs`): `logged_in` queues a player for `login_data` + counters + every claim;
>   `publish` (end of each `tick`, only when flagged by a command, a request, a team event or a login) diffs the claims
>   against `Server.sent` and sends `send_many_chunks` and `send_general_data`; `from_client` takes
>   `request_chunk_change` (team from the request only for an operator or a member, admin only for an operator) and
>   answers `chunk_change_response`.
> - *Client* (module `runtime/ftbchunks_map.rs`): the client's copy of the claims and counters; the claim screen (FTB's
>   `ChunkScreen`): M (`native_mod_key("ftbchunks", "map", "key.keyboard.m")`) opens a mod screen with the 31x31 chunks
>   round the player; `frame` scans 64 chunks a frame with `native_map_tile` into a 496x496 image and uploads it with
>   `native_mod_image` (API 355); `render` (on `screen_render_post`) draws it with `native_sprite`
>   (`MOD_IMAGE_SPRITE`), the chunk grid, claims in team colours (ftbteams' client colour) with borders, force-loaded
>   chunks crossed, the selection, the player, the counters (`ftbchunks.gui.claimed` / `force_loaded`, fallback wording
>   ours), the last `ftbchunks.claim_result`; press/drag/release hooks build a freehand selection (interpolated between
>   drag events) and queue one `request_chunk_change`, sent from `frame`. Dev probe (`FTBTEAMS_DEV=1`):
>   `/ftbchunksclient` answers `FTBCC {json}` (manager, counters, claims by team, last response, screen open, map rect
>   and cell, tiles), `/ftbchunksmap` opens the screen.
> - *Hooks* in `horizon-mixin.toml`: `player_logged_in`, `net_from_client`, `net_from_server`, `key`, `frame`,
>   `screen_render_post`, `screen_mouse_button_pressed_pre`, `screen_mouse_dragged_pre`,
>   `screen_mouse_button_released_pre`, `commands`, `command` (helpers in `hooks/`).
> - *Engine hook* **API 428** `native_map_tile(chunk) -> Option<[u8; 1024]>` (`game/native_client_services.rs`): a
>   loaded chunk's 16x16 RGBA map picture from the minimap extra's scanner (`Minimap::chunk_image` in
>   `horizon-client-render/src/minimap.rs`: the cached surface tile or a fresh scan that is not kept; unit test
>   `a_chunk_image_is_the_surface_picture_of_a_loaded_chunk`, the crate's 8 `minimap` tests pass). `ExtrasState.minimap`
>   is `pub(super)` for it. Fragment `map_tiles` stamped (`native-hooks.json`, `docs/native-mods.md`), listed in
>   docs/atm10-port.md "The fork's hooks". No new image hook: `native_mod_image` already uploads a dynamic RGBA image.
> - *World test* `pack/mods/ftbchunks/tests/map.ts` (`FTB_LIST=ftbchunks/map`, passes, 73 s): login data (manager id,
>   limits 500 / 25), M opens the screen through the agent's `key m tap` (the `/ftbchunksmap` fallback was not needed),
>   a serpentine `drag-at` over the 3x3 chunks from the player's claims 9 chunks on the server and client, `key shift
>   down` + the same drag force-loads them (9 tickets), the screenshot, a right drag unclaims a row, re-claiming counts
>   `already_claimed`, Escape closes. Shift reaches the press hook's `modifiers` and the drag events fire on the mod
>   screen, so no `game.physical_keys` fallback was needed. `ftb.sh` copies a passing test's PNGs to
>   `ftb-work/logs/shots-<test>/` (`shots-map/ftbchunks-claim-map.png`).
> - *What the screenshot shows* (two runs looked at). The 31x31 map (7 GUI px a chunk) with the title "Claim map" above
>   it; terrain (water, grass, forest, in the minimap extra's shading) only over the 117 chunks the client has loaded,
>   a dark disc round the player inside a dark square (no region cache yet: slice 5) with a faint chunk grid over both;
>   the 3x3 claim in the team's colour with a lighter border (the personal team's colour is random per world: green
>   in one run, red in the other), each force-loaded chunk crossed; the player's marker and name; "Chunks claimed: 9 /
>   500" and "Chunks kept loaded: 9 / 25" bottom left; "Chunks changed: 9 of 9" in yellow top left. Two things to
>   polish: the cross is always red, so on a red team it nearly disappears (use a contrasting colour); the probe's
>   chat lines show through behind the screen (test noise only). The fallback wording is ours on purpose: the first
>   draft's three labels matched the lang file's lines word for word, so they were reworded (check new labels the same
>   way against `ftb-work/jars/*/assets/*/lang/en_us.json`).
>
> **The minimap default, checked in the jar** (`FTBChunksClientConfig`, `javap -c`): `hasOtherMinimapMod()` is true when
> any of `journeymap`, `voxelmap`, `antiqueatlas` or `xaerominimap` is loaded (`Platform.isModLoaded`), and the
> `minimap.enabled` option's default is `!hasOtherMinimapMod()`. ATM10 ships JourneyMap and no `ftbchunks-client.snbt`,
> so **FTB Chunks' corner minimap is OFF by default on ATM10** (the coordinator's "on" does not match the jar). Slice 4
> builds it, ships it default-off with the same rule, and the option turns it on. There is no "is a mod loaded" helper
> in `native-hooks.json`: decide between a bundle constant (the ids above checked against the instance's mod list when
> a host helper exists) and a plain default-off until `journeymap` lands as a bundle.
>
> **Slice 4, in order.**
> 1. *Waypoints and the death waypoint* (client). On the transition to `Screen::Dead`, hide the earlier `death` ones and
>    add `Death #<n>`, `#FF0000`, at the player's block. File `waypoints.json` per dimension under
>    `<instance>/local/ftbchunks/data/<manager id>/<dimension with ':' as '_'>/` in FTB's layout
>    (`{"waypoints":[{"hidden","name","x","y","z","color":"#RRGGBB","type":"default"|"death"}]}`; read the exact keys
>    from the jar with `javap` before writing). Draw them on the claim screen (the screen already has the world to
>    screen mapping in `ftbchunks_map.rs`) and add `/ftbchunks waypoint add` (the server sends `add_waypoint`; see
>    `ftbchunks-spec.md` section 8). World test: `/kill`, `respawn`, the file on disk, the probe's list, the mark on
>    the screenshot.
> 2. *The corner minimap.* The `minimap` hook (`NativeMinimap`: corner, size, round) shows the extra's own map; claims
>    and waypoints over it need `gui_layer_post`. Options from `ftbchunks-spec.md` (position `TOP_RIGHT`, scale, zoom,
>    `locked_north`, `waypoints`, `square`), default off (see above), plus a test with the option on and a screenshot.
>
> **After slice 4 (slice 5).** The large map (M's other half: pan, zoom, a region cache under the same folder, so
> unloaded chunks show too) and the inventory sidebar button (FTB Library's `ftbchunks:chunks`/`ftbchunks:map`), the
> action-bar notice on entering a claim, `update_force_load_expiry`, `teleport_from_map`, `player_death` (server side),
> `open_claim_gui`. Then JourneyMap (slice 6) over the same tiles.
>
> **How to run.** `atm10-work/ftb-work/ftb.sh [units] [stage] [build] [world]` (this PC's paths for `bun run
> pack/scripts/ftbchunks.ts <step>`; `wait-slot.sh 3`, `-j 4`; export `AE2_PROFILE=iter`). `stage` applies only the
> FTB bundles and `build` compiles the client; do not edit sources while either runs (`E_STALE_SOURCE`). World tests:
> `FTB_LIST=ftbchunks/map ftb.sh world` (or `ftbchunks/claims`, `ftbteams/teams`); logs in
> `ftb-work/logs/world-<name>.log`, a failed run's folder in `ftb-work/logs/fail-<name>/`, a pass's PNGs in
> `ftb-work/logs/shots-<name>/`. The engine's minimap tests: `CARGO_TARGET_DIR=ftb-work/target-tools cargo test
> --offline -j 4 -p horizon-client-render --lib minimap` (through `wait-slot.sh 3`). Bundle unit tests alone: `cargo
> test --offline --manifest-path pack/mods/<bundle>/<crate>/Cargo.toml --target-dir ftb-work/target-tools`, then
> delete the crate's `Cargo.lock`. The second player in a test is FTB Teams' dev fake player
> (`/ftbteams_add_fake_player Bob`, `FTBTEAMS_DEV=1`). Extracted jars: `ftb-work/jars/`; notes: `ftb-work/notes/`
> (`ftbchunks-spec.md` sections 8 and 9 for the packets and the client map).
>
> **Still open from slice 2** (detail in `pack/mods/ftbchunks/NOTES.md`): PvP modes, mob griefing, fire and fluid across
> borders, `/ftbteams party settings ftbchunks:*` (FTB Teams' `Prop` is a closed enum: it needs an open property
> registry in `ftbteams-core`), idle and force-load expiry, farmland trampling, the finer item rules.
>
> **API gaps** (add the smallest way through `tools/native-hook-fragments/`, docs/atm10-port.md "Porting rules", only
> when a slice needs it):
> 1. Map tiles for mods: done in slice 3 (API 428 `native_map_tile`; `native_mod_image` uploads the dynamic image).
> 2. A cancellable mob-griefing check (endermen) and `native_living_incoming_damage` with the `Game` (PvP in claims).
> 3. Spawning near force-loaded chunks (`anyPlayerCloseEnoughForSpawning`).
> 4. Entity type tags on the server (the interact and attack whitelists; `corpse:corpse` is built in).
> 5. Fake players (`native_is_player_fake` exists; machines that break blocks without a player never reach
>    `destroy_block`).
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
1. **Map tiles for mods** (done in slice 3): API 428 `native_map_tile(chunk) -> Option<[u8; 1024]>` reads the minimap
   extra's cache (a scan for a chunk not yet scanned) and `native_mod_image` uploads a dynamic RGBA image a screen draws.
   Still to build in the bundles: the region cache of explored chunks (FTB: 512x512 regions; JourneyMap: 512x512 PNG
   tiles) that outlives the extra's tiles.
2. **Spawning near force-loaded chunks** (`anyPlayerCloseEnoughForSpawning`): a hook or a ticket flag.
3. **Fake players** (`PlayerHooks.isFake`): `native_is_player_fake` exists; machines that break blocks without a
   player (AE2's annihilation plane, Mekanism's digital miner) don't reach `destroy_block` at all, so `fake_players
   CHECK` only matters for mod bots. Listed, not needed first.

## Tiles shared with the minimap extra

One scanner, three views. The minimap extra's tile (`MapColor`, height, water) is the unit. FTB Chunks' region cache
keeps per-column colour, height and water flags in 512x512 regions saved under the game dir
(`local/ftbchunks/data/<manager id>/<dimension>/`), filled from `native_map_tile(chunk)` as chunks load and kept for chunks
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
   when nobody of the team is on; the admin bypass. Done in slice 3 with the client: `request_chunk_change`,
   `send_many_chunks` (the action-bar notice is slice 5); left in `pack/mods/ftbchunks/NOTES.md`: PvP, mob griefing, fire and
   fluid borders, `ftbchunks:*` team properties, expiry.
3. **FTB Chunks client: the claim map** (done): the map tile helper (gap 1, API 428), the claim screen (M) with drag
   selection sending one request on release, claim and force-load overlays in team colours, the counters, the server's
   `send_many_chunks` / `request_chunk_change` half. *World test* `pack/mods/ftbchunks/tests/map.ts`: a 3x3 drag with
   real mouse events claims 9 chunks, Shift force-loads them, a right drag unclaims, a screenshot.
4. **FTB Chunks client: waypoints, death waypoint, corner minimap** (next; the minimap defaults off on ATM10, see
   the handoff). *World test*: a death leaves a waypoint file and a mark on the map; the minimap with its option on.
5. **FTB Chunks client: the large map** (pan, zoom, region cache), the sidebar button, the action-bar notice and the
   server's remaining messages (see the handoff).
6. **JourneyMap**: full map, minimap presets, waypoint manager, death waypoints, chat sharing, its modes over the
   shared tiles. *World test*: screens open with their keys, a waypoint made with B survives a restart.

## Handoff

The handoff for the next slice is the block at the top of this file. Slice 1 is in `pack/mods/ftbteams`
(`NOTES.md` there), slice 2 in `pack/mods/ftbchunks` (`NOTES.md` there).
