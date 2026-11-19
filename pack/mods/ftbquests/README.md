# FTB Quests on this fork

FTB Quests 2101.1.37 for All the Mods 10, as a native mod of this fork: ATM10's quest book, read from the player's
instance, with the progress of the teams of the `ftbteams` bundle. It is this folder's bundle (`horizon-mixin.toml`,
`hooks/`, `runtime/`) and a crate of the rules (`ftbquests-core`). `pack/scripts/ftbquests.ts` builds and tests it
with FTB Teams; [docs/ftb-quests.md](../../../docs/ftb-quests.md) has the plan and where the work stands.

## Licence

FTB Quests, FTB Teams and FTB Library are All Rights Reserved (FTB). Nothing of theirs is copied here: the crate and
the runtime are a reimplementation of what the mod does, written from its behaviour and its files' layout; no line
of its language files, no texture and no GUI label is in this folder. Every label carries a translation key with a
plain fallback written for this port; `pack/scripts/ftbquests.ts check-text` checks that no fallback is one of FTB
Quests' own lines. The book screen is drawn with fills and the game's item icons. This bundle stays in this fork.

ATM10's quest book (`config/ftbquests/quests/`: chapters, groups, lang, reward tables) is the modpack's own content.
The game reads it from `config/ftbquests/quests/` beside it at run time (copied there from the player's ATM10
instance); it is never committed, not even as a test fixture. The unit tests write a small quest file of their own;
the world test copies the instance's folder into its own game folder. The book item comes from a content mod that
`pack/scripts/ftbquests.ts content` carries from the player's jar into a folder outside the repository.

## What there is (slice 1)

- The book read whole: chapters in their groups' order, quests, tasks, rewards, reward tables, the split lang.
- Progress per FTB team, `<world>/ftbquests/<team uuid>.snbt` in FTB Quests' layout, written on each save.
- Item tasks (by item id) seen in the inventory every `detection_delay` ticks; checkmark tasks; a quest completes
  with its tasks and its dependencies (ATM10's book is `flexible`); item rewards claimed per player, or once per
  team for a team reward.
- The book screen: chapters by group, the chapter's quest grid with dependency lines and item icons, a quest's view
  with its text, tasks and rewards. It opens with the book item, `/ftbquests open_book`, or the quests key (unbound
  by default, as in FTB Quests).

Tests: the crate's unit tests, and `tests/quests.ts` in a world (`FTBTEAMS_DEV=1` gives `/ftbquests_probe` and
`/ftbquestsbook`).

## On upstream's hooks (merge of nightly-source/2026-11-18)

- The teams' progress is read at `server_started` and written and dropped at `world_lifecycle`'s close phases.
- `key` -> `input_key` (presses only), `frame` -> `render_frame_pre`, `commands` -> `register_client_commands`,
  `perform_command` -> `run_mod_command` (`/ftbquests [line:text]`, `/ftbquests_probe [line:text]`: upstream's
  dispatcher parses a usage's arguments itself).
- The book item: `use_item` passes one `UseItemCall` whose stack is the API's; a HEAD injection converts it and sends
  `open_book`, and a `MODIFY_ARG` one answers `Success`. On the merged engine `use_item` only fires while a trait
  mod has a `use_item` part (HANDOFF "S3", N5), so the item does not open the book yet; `/ftbquests open_book` does.
- The merge dropped the fork's `native_carried_items` and `native_give_items`: `ftbquests_world` reads the
  inventory and the off hand itself and gives rewards through upstream's `player_give_stack`, in whole stacks.
