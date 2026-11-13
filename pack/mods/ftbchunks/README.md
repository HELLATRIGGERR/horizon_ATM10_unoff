# FTB Chunks on this fork

FTB Chunks 2101.1.22 for All the Mods 10, as a native mod of this fork: claims, their protection and force-loading,
for the teams of the `ftbteams` bundle. It is this folder's bundle (`horizon-mixin.toml`, `hooks/`, `runtime/`) and a
crate of the rules (`ftbchunks-core`). `pack/scripts/ftbchunks.ts` builds and tests it with the other FTB bundles;
`NOTES.md` says where the work stands; [docs/ftb-chunks.md](../../../docs/ftb-chunks.md) has the plan.

## Licence

FTB Chunks, FTB Teams and FTB Library are All Rights Reserved (FTB). Nothing of theirs is copied here: the crate is a
reimplementation of what the mod does, written from its behaviour, and no line of its language file, no chat or
command text and no GUI label is in this folder. Every message carries the mod's own translation key
(`Text::translate_with_fallback`), so a player whose instance has the mod's jar reads the mod's own lines; only a
missing key shows the fallback, which is plain wording written for this port. Command syntax and behaviour are the
mod's. This bundle stays in this fork.

## What there is (slice 2, the server)

- Claims per team, `<world>/ftbchunks/<team uuid>.snbt` in FTB Chunks' layout (limits, extra chunks, last login,
  chunks by dimension with claim and force-load times, member data with the chunks a member brought). A party gets a
  member's claims when they join (those past its limit are let go) and gives them back when they leave.
- Limits as FTB Chunks' defaults: 500 claims and 25 force-loaded chunks a player, a party the largest of its
  members' (`party_limit_mode`), hard team caps, dimension black- and whitelists; `ftbchunks-world.snbt` from the
  instance's `config/` under the world's `serverconfig/`. Server teams have no limit.
- Protection through the hooks: breaking and starting to break (`edit_whitelist`), right clicks on blocks
  (`interact_whitelist`, with crafting tables and bells), placing where the block lands, the right-click blacklist
  (buckets, leads, name tags, splash and lingering potions), filling buckets, entities (right clicks, armor stands,
  left clicks on frames, paintings, boats and carts), explosions (`ftbchunks:allow_explosions`, unknown sources
  protected), pistons across borders (`piston_protection`). The team properties `ftbchunks:*` are read from the team
  with the config's `team_prop_defaults`; the action-bar notice comes at most every 2 seconds.
- Force-loading through Horizon's chunk tickets, owned by `ftbchunks:<team>` (`/forceload` stays vanilla's): held
  while a member of the team is on, or always with `force_load_mode ALWAYS` or a member who may load offline, as
  FTB Chunks' defaults have it. Server teams' always.
- `/ftbchunks` (alias `/chunks`): `claim|unclaim|load|unload [radius_in_blocks]`, `unclaim_all|unload_all [team]`,
  `info [x z [dimension]]`, `admin bypass_protection|unclaim_everything|unclaim_dimension|unload_everything`,
  `admin claim_as|unclaim_as <team> [radius] [anchor] [dimension]`, `admin extra_claim_chunks|extra_force_load_chunks
  <player> get|set|add`. The bypass is kept in the player's team's extra data, as FTB Chunks keeps it.

Tests: the crate's unit tests, and `tests/claims.ts` in a world (`FTBTEAMS_DEV=1` gives `/ftbchunks_probe`).
