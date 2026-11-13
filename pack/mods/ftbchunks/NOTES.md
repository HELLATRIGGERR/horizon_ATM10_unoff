# FTB Chunks: where the work stands

Slice 2 of docs/ftb-chunks.md, the server side, is done; slice 3 (the map, the claim screen, the minimap, waypoints)
is next. FTB Chunks 2101.1.22 is All Rights Reserved (FTB): the crate reimplements its behaviour; nothing of it is
copied. Build and test: `bun run pack/scripts/ftbchunks.ts units stage build world` (this PC:
`atm10-work/ftb-work/ftb.sh`; `FTB_LIST=ftbchunks/claims` for this bundle's world test).

## How it fits

- `ftbchunks-core` has no dependencies: horizon-mixins refuses a bundle crate's path dependency on another bundle's
  crate, so the teams come in as `TeamView`s (trait `Teams`) that `runtime/ftbchunks_world.rs` builds from FTB Teams'
  manager (`crate::game::ftbteams_world::with_manager`). The runtime writes the SNBT with `ftbteams_core::ftblib`.
- FTB Teams' events (`TeamEvent`: created, joined, left, deleted, ownership, login, properties) reach FTB Chunks
  through a queue in `ftbteams_world` (`take_events`), drained each server tick; the tickets are checked then (at
  once after an event or a command, else every 100 ticks): wanted minus held are held, held minus wanted let go.

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
- Idle expiry (`max_idle_days_before_unclaim|unforce`, 0 by default) and force-load expiry (set from the claim screen,
  slice 3): the fields are kept and saved; `ClaimExpirationManager` isn't there.
- FTB Ranks permissions (`ftbchunks.max_claimed`, `max_force_loaded`, `chunk_load_offline`, `no_wilderness`): ATM10
  defines none, so the config's values apply; `offline_force_loader` is read from the files.
- The `request_chunk_change` message, `send_many_chunks` sync and the action-bar "entered claim" notice: slice 3, with
  the client.
- Admin feedback to other operators; team name completions.

## Gaps in the mod API (none added: the slice works without them)

1. A cancellable mob-griefing check (NeoForge `EntityMobGriefingEvent`) for endermen.
2. `native_living_incoming_damage` with the `Game` (PvP in claims).
3. Spawning in force-loaded chunks with nobody near (`anyPlayerCloseEnoughForSpawning`).
4. Entity type tags on the server (`entity_interact_whitelist`, `nonliving_entity_attack_whitelist`).
