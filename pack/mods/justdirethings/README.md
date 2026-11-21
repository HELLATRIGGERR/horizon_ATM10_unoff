# Just Dire Things on this fork

A port of **Just Dire Things 1.5.7** (NeoForge 1.21.1), the version All the Mods 10 ships (`justdirethings-1.5.7.jar`),
to this fork of Horizon. It is the rules crate in this folder (`justdirethings-core`) and the native bundle that drives
it in the game (`horizon-mixin.toml`, `hooks/`, `runtime/`). `pack/scripts/justdirethings.ts` builds and tests them.

## Source and licence

- Source: <https://github.com/Direwolf20-MC/JustDireThings>, tag `v1.5.7`, commit
  `8390e0effa33cd0e0995265245baf5629b61ef22` (`mod_version=1.5.7` in `gradle.properties`).
- Licence: MIT, Copyright (c) 2023 Direwolf20-MC (`LICENSE.txt` of the repository at that tag; the jar's
  `neoforge.mods.toml` says `license="MIT"` too). `LICENSE-justdirethings` is its text.

The Rust code here was **translated from that source** (`GooBlock_Base`, `GooBlockBE_Base`, `GooBlockBE_Tier1..4`,
`GooSpreadRecipe`, `GooSpreadRecipeTag`, `GeneratorT1BE`, `BlockBreakerT1BE`, `BaseMachineBE`, `RedstoneControlledBE`,
`RedstoneControlData`, `Config`, `GeneratorT1Screen`, `BlockBreakerT1Screen`, `BaseMachineScreen`), not written from the mod's behaviour
alone, and it is licensed under the same MIT. The source is cloned outside this repository
(`atm10-work/mod-sources/justdirethings`) and never committed.

No asset or file of the mod is committed: blocks, items, models, textures, language files, recipes and tags come from
the player's own jar through `horizon-port-pack carry`. The screen's text is the bundle's own English.

## What is ported

- **Goo blocks**, the four tiers (`gooblock_tier1` to `gooblock_tier4`): a living goo converts the block on each of
  its sides that a spreading recipe names, from the world's recipe tables (`justdirethings:goospread`, a block state,
  then `justdirethings:goospread_tag`, a block tag; ATM10 adds its own through KubeJS, which the carry holds), for a
  goo of at least the recipe's tier. A side counts down the recipe's duration by the tier's step (1, 2, 5, 10 a tick)
  and its block becomes the output, facing away from the goo; a block that stops matching cancels its side. After each
  conversion the goo may die (`goo_death_chance`, 10%), with the vex's death sound; a dead goo starts nothing until
  an item of its tier's revive tag (`justdirethings:goo_revive_tier_1` to `_4`) is used on it. The block entity keeps
  Java's `sideCounters` and `sideDurations`.
- **Generator T1**: one fuel slot (fuel only), a 1,000,000 FE buffer filled at 15 FE a burn tick, burning 4 times faster
  than a furnace (the mod's coals and coal blocks 2, 4, 8 and 16 times more), going on into the next fuel within the
  tick, offering ten times its 1,000 FE a tick to each of its six neighbours, giving energy to cables that draw it and
  taking none. Redstone control (ignored, low, high, pulse) from its screen's button. Its screen is drawn from the mod's
  own panel, slot, power bar and button textures, with the energy and burn tooltips.
- **Block Breaker T1**: one tool slot; on its operation clock (every 20 ticks) and where its redstone mode lets it, it
  starts on the block in front of it (not air, not a liquid, not unbreakable) and mines it as Java's fake player: the
  tool's destroy speed (with Efficiency) over the block's hardness, over 30 for a tool correct for the block's drops
  and 100 for any other, a share a tick, with the crack the players near it see; at 1 the block breaks with the
  tool's drops and the tool takes a point of wear. A block that changes, a tool taken out or the signal lost drops the
  progress. Its `active` state follows the redstone; its screen has the tool slot (the mod's pickaxe outline while
  empty) and the redstone button.
- The settings come from `config/justdirethings-server.toml` (`goo`, `generator_t1`).

`justdirethings-core` holds the rules with unit tests; `tests/goo.ts`, `tests/generator.ts` and `tests/breaker.ts` run
them in a world.
`NOTES.md` lists the differences from Java and what is left.

## On upstream's hooks (merge of nightly-source/2026-11-18)

Hooks, all on main (no new fork hook): `world_lifecycle` v2, `server_started`, `block_entity_types` (the fork's compat
hook: one type per goo tier, the generator's and the breaker's, named after the block), `block_entity`, `block_entity_ticks`,
`block_entity_tick`, `block_entity_container`, `block_entity_slot_accepts`, `fe_storage`, `fe_extract` (a
`MODIFY_ARG` on the call into `fire_fe_extract`), `menu_button`, `use_on` v2 (a `HEAD` injection reads the call with
the game, a `MODIFY_ARG` one answers it), `screen`, `screen_draw`, `screen_click` and `screen_tooltip`.
