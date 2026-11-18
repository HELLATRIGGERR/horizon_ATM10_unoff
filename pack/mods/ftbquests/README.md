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

## What there is (slices 1 and 2)

- The book read whole: chapters in their groups' order, quests, tasks, rewards, reward tables, the split lang.
- Progress per FTB team, `<world>/ftbquests/<team uuid>.snbt` in FTB Quests' layout, written on each save.
- Item tasks (by item id) seen in the inventory every `detection_delay` ticks; checkmark tasks; a quest completes
  with its tasks and its dependencies (ATM10's book is `flexible`).
- Rewards of every kind ATM10's book has: items, experience points and levels, reward tables (`random`, `loot`,
  `choice` with a picker), commands run as the player at the reward's `permission_level`, toasts. Claimed per
  player, or once per team for a team reward; "take all" (minus `exclude_from_claim_all`), auto-claim,
  `rewards_blocked` (`/ftbquests block_rewards`). Toasts for completed tasks, quests and chapters and for what a
  claim gave; chat notices for refused claims, a command's output and items dropped for want of room.
- The book screen: chapters by group, the chapter's quest grid with dependency lines and item icons, a quest's view
  with its text, tasks and rewards, a choice reward's picker, "take all". It opens with the book item,
  `/ftbquests open_book`, or the quests key (unbound by default, as in FTB Quests).

Tests: the crate's unit tests, and `tests/quests.ts` in a world (`FTBTEAMS_DEV=1` gives `/ftbquests_probe` and
`/ftbquestsbook`).
