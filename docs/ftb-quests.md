# FTB Quests on this fork

The plan for porting FTB Quests 2101.1.37 (`ftb-quests-neoforge-2101.1.37.jar`, with `ftbquestslangsplitter`
1.0.7) as the `pack/mods/ftbquests` bundle. ATM10's quest book is worth 6 of the 50 play-system points of the
coverage method.

## Licence

FTB Quests is All Rights Reserved (FTB), as FTB Teams and FTB Chunks are (see `pack/mods/ftbteams/README.md`). The
bundle ports behaviour only: no method body, no texture, no lang line of the mod. Every label is a translation key
with a fallback in our own words; `pack/scripts/ftbquests.ts check-text` compares the bundle's fallbacks with the
lang files of the player's jar. ATM10's quest book (`config/ftbquests/quests/`) is the modpack's content: the game
reads it from the player's instance at run time, and it is never committed, not even as a test fixture. Unit tests
write a tiny quest file of their own; the world test copies the instance's files into its own game folder.

## What FTB Quests does in ATM10

Counted from the instance's `config/ftbquests/quests/` by a script (the counts move with ATM10 updates):

- **The book:** 67 chapters in 10+ groups (`chapter_groups.snbt`), 4,892 quests, 5,327 dependency links,
  quest links (`quest_links`), chapter images (`atm:textures/questpics/...`, from ATM10's own resource pack).
  `data.snbt`: flexible progression, `default_consume_items: false`, `default_reward_team: false`,
  `default_autoclaim_rewards: "disabled"`, `detection_delay: 60`, `grid_scale: 0.5`, no loot-crate drops.
- **Tasks (6,138):** item 5,447 (833 of them with `components`, mostly FTB Filter System smart filters, and 223
  with `match_components`), checkmark 229, kill 224, structure 81, biome 54, advancement 54, dimension 29,
  observation 16, location 2, forge_energy 1, xp 1.
- **Rewards (7,353):** item 3,027, xp 1,966, random 1,046, xp_levels 949, loot 262, choice 95, command 7,
  toast 1; 1,369 are `exclude_from_claim_all`. 61 reward tables (`reward_tables/`, 794 item entries, 2 xp levels).
- **Team progress:** one `<world>/ftbquests/<team uuid>.snbt` per FTB team (`version`, `uuid`, `name`, `lock`,
  `rewards_blocked`, `task_progress`, `started`, `completed`, `repeatable`, `completion_count`,
  `claimed_rewards` keyed `<player>:<reward>`, `player_data` with pinned quests). FTB Teams is ported.
- **The lang split:** `ftbquestslangsplitter` keeps the text in `lang/<locale>/` as `file.snbt`,
  `chapter_group.snbt`, `chapter.snbt`, `reward_table.snbt` and `chapters/<chapter>.snbt` (`quest.<id>.title`,
  `quest.<id>.quest_desc`, `quest.<id>.quest_subtitle`, `task.<id>.title`, ...), 15 locales.
- **Opening the book:** the `ftbquests:book` item (right click), the quests key (unbound by default in FTB Quests),
  FTB Library's sidebar button on the inventory, `/ftbquests open_book`.
- **Claiming:** a reward is claimed from the quest view (per player unless `team_reward`), "claim all", auto-claim
  per quest; completion toasts and chat notices; item tasks auto-detect from the inventory every
  `detection_delay` ticks and on inventory changes, consuming ones are submitted from the view.

## Slices (a few days each at most)

1. **The book, item and checkmark tasks, item rewards.** Load chapters, groups, quests, tasks, rewards, reward
   tables and the split lang; the book screen (chapter list, a chapter's quest graph with icons and dependency
   lines, a quest's view); item tasks detected from the inventory and checkmark tasks clicked, for the player's
   team, saved in FTB's layout; item rewards claimed. Book item, key, `/ftbquests open_book`.
2. **The other rewards and claiming:** xp, xp levels, reward tables (`random`, `loot`, `choice` with its picker),
   command (with `permission_level`), toast; claim all (minus `exclude_from_claim_all`), auto-claim, completion
   toasts and chat notices, `rewards_blocked`.
3. **The other tasks:** kill, advancement (with `criterion`), dimension, biome, structure, observation, location,
   forge_energy, xp; item tasks with `components`/`match_components` and FTB Filter System's smart filters;
   consuming item tasks with the submit button; `optional_task`.
4. **The book's rules and looks:** `dependency_requirement`, `min_required_dependencies`, `optional`, `invisible`,
   `hide_until_deps_visible`/`_complete`, `hide_details_until_startable`, `hide_dependency_lines`, shapes and sizes,
   chapter images, quest links, lock icons, progression modes, `can_repeat`, search, pinned quests and the tracker
   HUD, the sidebar button.
5. **Teams and sync:** progress follows FTB Teams' party joins and leaves as FTB Quests does, per-player data,
   the quest file and progress sent to remote clients (no local config needed), the client's locale for the lang,
   `/ftbquests change_progress|open_book|reload` and the rest of the command tree.

## Slice 1 as built

- `pack/mods/ftbquests/ftbquests-core`: the quest file reader (FTB's SNBT, the lang split), the team data in FTB's
  layout, task progress and quest completion rules, reward claims; unit tests on a quest file they write.
- `pack/mods/ftbquests/runtime/ftbquests_world.rs`: the server side (team data per world, detection every
  `detection_delay` ticks, the client's checkmark and claim messages, the book item, `/ftbquests open_book`).
- `pack/mods/ftbquests/runtime/ftbquests_book.rs`: the book screen (`ftbquests` mod screen).
- The quest file comes from `<config>/ftbquests/quests/` beside the game (ATM10's, copied from the instance by
  the player; the world test copies it into its own game folder). The book item is in the content mod that
  `pack/scripts/ftbquests.ts content` carries from the player's jar into the work folder, never committed.
- Not yet: item tasks with components (counted by id only), consuming item tasks, the client's locale (en_us).
