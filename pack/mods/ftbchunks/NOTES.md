# FTB Chunks: where the work stands

Slices 2 (the server), 3 (the claim screen, its messages, the map tile API 428), 4 (waypoints, the death waypoint, the
corner minimap) and 5 (the large map with its region cache, the sidebar buttons, the action-bar notice, the last server
messages) of docs/ftb-chunks.md are done; what is left is the handoff at the top of that file. FTB Chunks 2101.1.22 is All Rights Reserved (FTB): the crate
reimplements its behaviour; nothing of it is copied. Build and test: `bun run pack/scripts/ftbchunks.ts units stage build world` (this PC:
`atm10-work/ftb-work/ftb.sh`; `FTB_LIST=ftbchunks/claims`, `ftbchunks/map`, `ftbchunks/waypoints` or `ftbchunks/region-map` for this bundle's world tests).

## How it fits

- `ftbchunks-core` has no dependencies: horizon-mixins refuses a bundle crate's path dependency on another bundle's
  crate, so the teams come in as `TeamView`s (trait `Teams`) that `runtime/ftbchunks_world.rs` builds from FTB Teams'
  manager (`crate::game::ftbteams_world::with_manager`). The runtime writes the SNBT with `ftbteams_core::ftblib`.
- FTB Teams' events (`TeamEvent`: created, joined, left, deleted, ownership, login, properties) reach FTB Chunks
  through a queue in `ftbteams_world` (`take_events`), drained each server tick; the tickets are checked then (at
  once after an event or a command, else every 100 ticks): wanted minus held are held, held minus wanted let go.

## Slice 4: waypoints and the minimap

- `ftbchunks-core/src/waypoints.rs`: FTB's `waypoints.json` keys, `add_death`, `parse_add` for `/ftbchunks waypoint add`,
  `add_waypoint` / `player_death` messages, `minimap_on_by_default`. The client half is `runtime/ftbchunks_minimap.rs`
  (waypoints under `<instance>/local/ftbchunks/data/<manager>/<dim>/`, markers and the right-click menu on the claim
  screen, the corner minimap on the `hud` hook); the server half is in `ftbchunks_world.rs` (`perform`, `player_died`,
  `player_respawned`). World test `tests/waypoints.ts`.
- The death spot is the player's block as of the last tick (`ALIVE`, filled in `tick`): inside `player_died` the server's
  player list is empty (the host's players are taken out), so `native_server_players` cannot be asked there.
  `native_player_stat` works in `player_respawned` (the player is swapped in) and gives the `minecraft:deaths` count.
- The minimap is on by default because no other minimap mod is built in (FTB's own rule over `journeymap`, `voxelmap`,
  `antiqueatlas`, `xaerominimap`; JourneyMap is not ported). No new engine API.
- Not done: FTB's waypoint add/edit screens and manager, in-world beacons, `death_waypoint_autoremove_distance`,
  sharing, reading `ftbchunks-client.snbt` (the options are the defaults; the unbound `toggle_minimap` key switches it).
- The labels are translation keys with our own fallbacks (`ftbchunks.gui.add_waypoint`, `ftbchunks.label.hide|show`,
  `ftbchunks.gui.delete`, `ftbchunks.command.waypoint_added`); none equals a line of the FTB jars' lang files.

## Slice 5: the large map and its region cache

- `ftbchunks-core/src/regions.rs`: `Region` (a 32x32-chunk file, each seen chunk's 16x16 RGBA and the time it was seen; our own
  format `HZMR`, FTB's region file names `%05X-%05X` of the region + 60000 with the extension `.hzmap`, because FTB's zips
  need block ids we don't have), `MapView` (pan, zoom round the pointer, FTB's zoom range), `expiry_step` (the hour/day/week
  wheel steps of a force-load), `force_load_color` (the team colour's hue + 0.5, the rule read off `ChunkScreenPanel$ChunkButton`
  with `javap`), and the messages `update_force_load_expiry`, `teleport_from_map`, `open_claim_gui`.
- `runtime/ftbchunks_largemap.rs` (`[[module]] largemap`): the region cache is filled 8 chunks a frame within 12 chunks of
  the player from `native_map_tile`, saved every 10 s, when the large map closes and on `WorldCloseFinal`, in
  `local/ftbchunks/data/<manager>/<dim>/`. `tile()` (live tile, else cached) is what the claim screen and the minimap read,
  so a chunk the client has seen shows after it unloads and after a restart, and the dark disc round the player is gone.
  The large map (screen `ftbchunks:map`; M opens and closes it, left drag pans, wheel zooms, right click = the waypoint menu,
  Space centres, C or the top-left button opens the claim screen, T sends `teleport_from_map`) is one image a GUI pixel
  composed from the cache with the claims baked in, redrawn every 40 ms while it moves and each second otherwise.
  `ftbchunks_minimap.rs` keeps the marker and menu geometry both maps share (`MapFrame`).
- Sidebar: two 16 px buttons (claim screen, map) top left of the player's own inventory (screen id `inventory`; the creative
  inventory has none), icons drawn in fills, names `sidebar_button.ftbchunks.chunks|map`. A click needs the inventory closed
  first: the new engine API 432 `native_close_inventory` (`native_hooks.rs`, `docs/atm10-port.md`); the new injection
  `screen_mouse_scrolled_pre` (an existing hook, v1) carries the wheel.
- Server (`ftbchunks_world.rs`): `update_force_load_expiry` (a member, same dimension), `teleport_from_map` (operators; the
  surface from `native_surface_heights`, else 70), `admin open_claim_gui_as`, and the action-bar notice when the player
  enters a chunk whose owner differs (the team's name in its colour, or `ftbchunks.wilderness` in dark green; real players only).
- Probe: `/ftbchunksclient` has `large` {open, center, zoom, rect, button, uploads, known, image, teleports, sidebar, cache};
  `/ftbchunksbigmap [save | has <cx> <cz> | center <x> <z>]`. World test `tests/region-map.ts` (cache fills, the notice out of
  and into a claim, a teleport 1500 blocks away leaves the spawn chunk cached and its file written, a restart reads it back,
  M, a drag, the wheel, the cached spawn on the map, the claim screen nine chunks from the claim, the sidebar button).
- Test traps: a client command's chat line can reach the reply a frame late (the reply then still holds the command before:
  `probeKey` asks again until its own key is there); at zoom 1/8 a chunk is 2x2 pixels, so `known` counts 4 a chunk.
- Labels are translation keys with our own fallbacks (`ftbchunks.wilderness`, `sidebar_button.ftbchunks.*`, `ftbchunks.gui.claimed_chunks`); a script over the slice's diff found no string equal to a line of the FTB jars' lang files (only identifiers and
  probe keys such as `open`, `zoom`, `save`, `regions` match single words).
- Not done: FTB's waypoint manager and settings screens on the large map, a dimension switch, the chunk grid toggle, other
  players on the map, releasing a region after `region_release_time`, the sidebar on the creative inventory.

## Not done yet

- The team properties `ftbchunks:*` are read from the team's file (FTB Teams keeps unknown properties) with the
  config's defaults, but `/ftbteams party settings ftbchunks:...` doesn't know them: FTB Teams' `Prop` is a closed
  enum. Needs an open property registry in `ftbteams-core` (`TeamCollectPropertiesEvent`).
- PvP modes (`pvp_mode`, `allow_pvp`): the default `ALWAYS` changes nothing; `native_living_incoming_damage` has no
  `Game`, so the other modes need a hook with one.
- Mob griefing (endermen): `native_entity_change_block` only listens. Needs a cancellable form (gap, below).
- Fire spread and fluid flow across borders (`fire_spread_protection`, `flowing_fluid_protection`, both off by
  default): `native_block_spread` / `native_fire_odds` are there; not wired yet.
- Farmland trampling (EDIT_BLOCK with the trampler), `EntityInteractSpecific`, the block-item rule of
  `blockRightClick` (INTERACT allowed but EDIT refused for a held block item: here the placement is checked where
  the block lands through `use_item_on` instead), the food and harmless-potion exemptions of `RIGHT_CLICK_ITEM`
  (only the blacklist refuses, so only splash and lingering potions of healing differ), entity type tags (the
  `corpse:corpse` whitelist is built in).
- Fake players beyond `allow_fake_players` / `by_id` / named; the prevented-access log.
- Force-loading counts as a player near for spawning (`ChunkMapMixin`): needs a hook (gap).
- Idle expiry (`max_idle_days_before_unclaim|unforce`, 0 by default) and force-load expiry (set from the claim screen and the
  wheel over a force-loaded chunk, slice 5): the fields are kept and saved; `ClaimExpirationManager` isn't there.
- FTB Ranks permissions (`ftbchunks.max_claimed`, `max_force_loaded`, `chunk_load_offline`, `no_wilderness`): ATM10
  defines none, so the config's values apply; `offline_force_loader` is read from the files.
- Admin feedback to other operators; team name completions.

## Gaps in the mod API (none added: the slice works without them)

1. A cancellable mob-griefing check (NeoForge `EntityMobGriefingEvent`) for endermen.
2. `native_living_incoming_damage` with the `Game` (PvP in claims).
3. Spawning in force-loaded chunks with nobody near (`anyPlayerCloseEnoughForSpawning`).
4. Entity type tags on the server (`entity_interact_whitelist`, `nonliving_entity_attack_whitelist`).
