# FTB Quests on this fork

> **Handoff (slices 1 and 2 landed; next is slice 3; remove this block when it lands).**
> - **State:** the book, item and checkmark tasks, and every reward kind ATM10's book has are read and claimable in
>   a world on ATM10's own book (see As built). Units: 7 (quests) + 17 (teams); `ftbquests/quests` and `ftbteams/teams`
>   world tests; `check-text` finds 0 fallbacks equal to FTB Quests' lines.
> - **Slice 3** (see Slices) is the other task types. Start in `ftbquests-core/src/file.rs` (`TaskKind::Other` holds
>   them, and the quest view draws it as "(kind: not in the port yet)") and `team.rs` (`set_progress` takes any
>   task's progress; `detect_items` shows the shape of a detector). Detection runs in `runtime/ftbquests_world.rs`'s
>   `tick`, and completions already reach the toasts and the auto-claims through `TeamData::newly`. Add a probe
>   `pick` per kind in `tests/quests.ts`, as slice 2 did for the rewards. What each kind needs:
>   - **kill** (224): a death hook that names the killer. `native_mob_killed` hears only a player's melee blow;
>     FTB counts any kill where the player is the killer, so bows and tridents too (`native_living_death` with the
>     killing player). The entity ids are modded mobs, from the **mob stream**. Needed from it: those types
>     registered under their ids, and killable.
>   - **advancement** (54, `criterion`): `native_player_advancement_done` and
>     `native_player_advancement_criterion_grant` exist. Modded advancements need the mods' advancement data loaded.
>   - **dimension** (29), **biome** (54), **structure** (81), **location** (2): checked every `detection_delay` ticks
>     from `NativeServerPlayer.dimension` and `position`, `native_biome` (biome tags `#...` need a tag lookup).
>     No native says which structure a position is in yet (vanilla's `StructureManager.getStructureWithPieceAt`),
>     and one is needed. Custom dimensions (Twilight Forest, the mining dimension, ...) and their biomes and
>     structures come from the **dimension stream**. Needed from it: the dimension ids as FTB names them, and their
>     structure starts kept so a position can be asked.
>   - **observation** (16): what the player looks at, server side (a block or entity under the crosshair for
>     `timer` ticks). `native_raycast` is the client's and sees only blocks, so a server-side ray for blocks and
>     entities is needed.
>   - **forge_energy** (1): energy taken from an item the player carries (`native_item_energy_extract`).
>   - **xp** (1): `NativeServerPlayer.xp_level`; a consuming one takes levels with `native_give_levels` (negative).
>   - **item tasks with `components`/`match_components`** (833/223): `native_carried_items` gives ids only, so the
>     stacks' components are needed, plus an evaluator for FTB Filter System's smart filter expressions
>     (`or(and(item_tag(...)mod(...)))`), which needs item tags. **Consuming item tasks**: the view's submit button
>     taking the items (`native_take_items`). **`optional_task`** is read and left out of the completion.
> - **Driver:** `bash D:/VSCODE_PROJECTS/DEV_1/atm10-work/ftbquests-work/ftbq.sh stage build world` with
>   `AE2_PROFILE=iter FTBQ_LIST=ftbquests/quests` (also `units`, `check-text`, `content`); logs in
>   `ftbquests-work/logs/` (`world-quests.log`, screenshots in `shots-quests/`, failures in `fail-quests/`). Do not edit
>   sources while `stage` runs. A rebuild takes about 75 s. The world test waits for the shared render lease, so on a busy
>   PC it can sit at its first log line for 20 minutes or more; that is the queue, not a hang.
>   `FTBQ_QUESTS_DIR=<instance>/config/ftbquests/quests` with `units` makes the crate count ATM10's rewards by kind.
> - **Seen in the screenshots, for slice 4:** quests whose item is from a mod the fork does not have draw a grey
>   square (the icon is by item id); chapter icons likewise; quest sizes are drawn, quest shapes are not (slice 4);
>   the toasts in the corner of the shots are the world test's own (recipe book, movement hints) besides the book's.

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

## As built (slices 1 and 2)

- `pack/mods/ftbquests/ftbquests-core`: the quest file reader (FTB's SNBT, the lang split, the reward tables), the
  team data in FTB's layout, task progress and quest completion rules, reward claims and what each gives
  (`team::Grant`), claim all, auto-claim; unit tests on a quest file they write.
- `pack/mods/ftbquests/runtime/ftbquests_world.rs`: the server side (team data per world, detection every
  `detection_delay` ticks, the client's checkmark, claim, pick and claim-all messages, the rewards given, the
  completion toasts and auto-claims of what a team just completed, the book item, `/ftbquests open_book`,
  `/ftbquests block_rewards <true|false> [player]`).
- `pack/mods/ftbquests/runtime/ftbquests_book.rs`: the book screen (`ftbquests` mod screen) and the toasts.
- The quest file comes from `<config>/ftbquests/quests/` beside the game (ATM10's, copied from the instance by
  the player; the world test copies it into its own game folder). The book item is in the content mod that
  `pack/scripts/ftbquests.ts content` carries from the player's jar into the work folder, never committed.
- `pack/mods/ftbquests/tests/quests.ts`: the world test on ATM10's own book (copied from the instance into the
  test's game folder): counts, the book item, `/ftbquests open_book`, an item task, a claim, a checkmark, then the
  first reward of each other kind claimed (xp, xp_levels with "take all", random while and after
  `block_rewards`, loot, choice through the picker, command, toast), a restart, and an auto-claim on the test's copy
  of the book set to `default_autoclaim_rewards: "enabled"`. Screenshots: chapter list, quest view, rewards, blocked,
  picker, toasts, after the restart.

The rewards (ATM10's counts):

- **item** (3,027): the item and `count`, plus 0 to `random_bonus` more (135 have one); what does not fit drops at
  the player's feet with a chat notice. Plain items only: an item's `components` are not given yet.
- **xp** (1,966) and **xp_levels** (949): points as an orb's touch gives them (`native_give_experience`, which
  lets Mending take its share first; FTB gives the points straight to the player) and whole levels. Where the
  player's state can't be swapped in (the server tick, where auto-claims run), items and experience go through the
  console's `give` and `experience add` instead.
- **random** (1,046), **loot** (262), **choice** (95) and the 61 tables (796 entries: 794 items, 2 xp levels):
  `random` is one draw by weight (`weight`, 1 by default; `empty_weight` draws nothing), `loot` is
  `loot_size` draws, `choice` opens the picker in the quest view and gives the entry picked. A table's entries are
  rewards of their own (item, xp, xp_levels). `table_id` is read as a decimal long or as 16 hex digits. The
  tables' `loot_crate` (FTB's loot crate items) is not ported; ATM10 drops none (`drop_loot_crates: false`).
- **command** (7, all `/neovitae anima-network @p add N` at `permission_level: 3`): run on the next server tick as
  `execute as <player> at @s run <command>` from the console, so `@s` and `@p` are the player, after a check that
  the command's level (`native_command_info`) is within `permission_level`; a higher one is refused as an unknown
  command. Placeholders `{p}`, `{x}`, `{y}`, `{z}`, `{quest}`, `{chapter}`, `{team}`. Its output goes to the
  player's chat unless `silent`, then its `feedback_message`. Neo Vitae is not in the fork, so ATM10's own commands
  answer "unknown command".
- **toast** (1): a toast with the reward's title (else its quest's) and `description`.
- **Claiming:** per player unless `team_reward`; "take all" over the quest grid claims every reward of a completed
  quest but the `exclude_from_claim_all` ones (1,369) and the choices; `auto` (`enabled`, `no_toast`,
  `invisible`, else the file's `default_autoclaim_rewards`, `disabled` in ATM10) claims a quest's rewards for the
  members who are on when it completes; `rewards_blocked` (`/ftbquests block_rewards`) refuses claims but
  `ignore_reward_blocking` ones, the view says so and greys the buttons out.
- **Toasts:** a quest, a chapter (the challenge frame), a task of a quest with more than one, unless the object, its
  quest, chapter or the file says `disable_toast`; what a claim gave (one toast for "take all"); a toast reward.
- Not yet: item tasks with components (counted by id only), consuming item tasks, the client's locale (en_us).
