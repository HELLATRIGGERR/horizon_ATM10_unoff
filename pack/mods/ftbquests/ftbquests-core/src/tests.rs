//! A quest folder of our own, written by the tests: two chapters in a group, an item quest with a reward of each
//! kind, a checkmark quest that waits on it, an auto-claimed quest, a reward table, and the split lang.

use crate::{file::*, snbt, team::*, *};
use std::{fs, path::PathBuf};

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ftbquests-core-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    for sub in ["chapters", "lang/en_us/chapters", "reward_tables"] {
        fs::create_dir_all(dir.join(sub)).unwrap();
    }
    fs::write(dir.join("data.snbt"), "{\n\tdefault_autoclaim_rewards: \"disabled\"\n\tdefault_reward_team: false\n\tdetection_delay: 40\n\tgrid_scale: 0.5d\n}\n").unwrap();
    fs::write(
        dir.join("reward_tables/gems.snbt"),
        r#"{ id: "00000000000000F1", loot_size: 2, use_title: true, rewards: [
	{ count: 2, id: "0000000000000031", item: { count: 1, id: "minecraft:diamond" } }
	{ id: "0000000000000032", type: "xp_levels", xp_levels: 3, weight: 3.0f }
] }"#,
    )
    .unwrap();
    fs::write(dir.join("lang/en_us/reward_table.snbt"), "{ reward_table.00000000000000F1.title: \"Gems\" }").unwrap();
    fs::write(dir.join("chapter_groups.snbt"), "{ chapter_groups: [{ id: \"00000000000000A0\" }] }").unwrap();
    fs::write(
        dir.join("chapters/first.snbt"),
        r#"{
	filename: "first"
	group: "00000000000000A0"
	id: "0000000000000C02"
	order_index: 1
	quests: [
		{ id: "0000000000000001", x: 0.0d, y: 0.0d, tasks: [{ id: "0000000000000011", type: "item", item: { id: "minecraft:cobblestone", count: 1 }, count: 16L }]
		  rewards: [{ id: "0000000000000021", type: "item", item: { id: "minecraft:bread", count: 3 } }, { id: "0000000000000022", type: "xp", xp: 10 }
			{ exclude_from_claim_all: true, id: "0000000000000024", type: "xp_levels", xp_levels: 5 }
			{ id: "0000000000000025", table_id: 241L, type: "random" }
			{ id: "0000000000000026", table_id: 241L, type: "loot" }
			{ exclude_from_claim_all: true, id: "0000000000000027", table_id: 241L, type: "choice" }
			{ command: "/say {p} did it", id: "0000000000000028", permission_level: 3, type: "command" }
			{ description: "Well done", id: "0000000000000029", type: "toast" }
			{ id: "000000000000002D", type: "custom" }] }
		{ id: "0000000000000002", x: 2.0d, y: 0.0d, disable_toast: true, dependencies: ["0000000000000001"], tasks: [{ id: "0000000000000012", type: "checkmark" }]
		  rewards: [{ id: "0000000000000023", type: "item", item: "minecraft:apple", team_reward: true }] }
	]
}"#,
    )
    .unwrap();
    fs::write(
        dir.join("chapters/zero.snbt"),
        r#"{ filename: "zero", id: "0000000000000C01", order_index: 0, quests: [{ id: "0000000000000003", tasks: [{ id: "0000000000000013", type: "checkmark" }]
	rewards: [{ auto: "enabled", id: "000000000000002A", item: "minecraft:emerald", random_bonus: 2 }, { auto: "no_toast", id: "000000000000002B", type: "xp", xp: 7 }
		{ auto: "enabled", id: "000000000000002C", table_id: "00000000000000F1", type: "choice" }] }] }"#,
    )
    .unwrap();
    fs::write(dir.join("chapters/broken.snbt"), "{ id: \"0000000000000C03\" quests: [ { id: ").unwrap();
    fs::write(dir.join("lang/en_us/chapter.snbt"), "{ chapter.0000000000000C02.title: \"&aStone &rage\" }").unwrap();
    fs::write(dir.join("lang/en_us/chapters/first.snbt"), "{\n\tquest.0000000000000001.title: \"Gather rock\"\n\tquest.0000000000000001.quest_desc: [\"Line one\\\\nline two\"]\n}").unwrap();
    dir
}

#[test]
fn the_folder_loads_in_book_order() {
    let dir = folder("load");
    let (file, warnings) = QuestFile::load(&dir, "en_us").unwrap();
    assert_eq!(warnings.len(), 1, "the broken chapter is named: {warnings:?}");
    assert_eq!(file.chapters.iter().map(|c| c.filename.as_str()).collect::<Vec<_>>(), ["zero", "first"]);
    assert_eq!((file.quests.len(), file.tasks.len(), file.rewards.len(), file.detection_delay), (3, 3, 13, 40));
    assert_eq!(file.tasks[&0x11].kind, TaskKind::Item { item: "minecraft:cobblestone".into(), count: 16, components: false });
    assert_eq!(file.rewards[&0x21].kind, RewardKind::Item { item: "minecraft:bread".into(), count: 3, components: false, random_bonus: 0 });
    assert_eq!(file.rewards[&0x23].kind, RewardKind::Item { item: "minecraft:apple".into(), count: 1, components: false, random_bonus: 0 });
    assert_eq!(file.rewards[&0x22].kind, RewardKind::Xp { points: 10 });
    assert_eq!(file.rewards[&0x24].kind, RewardKind::XpLevels { levels: 5 });
    assert_eq!(file.rewards[&0x25].kind, RewardKind::Random { table: 0xF1 }, "a table id written as a decimal long");
    assert_eq!(file.rewards[&0x2C].kind, RewardKind::Choice { table: 0xF1 }, "or as 16 hex digits");
    assert_eq!(file.rewards[&0x28].kind, RewardKind::Command { command: "/say {p} did it".into(), permission_level: 3, silent: false, feedback: None });
    assert_eq!(file.rewards[&0x29].kind, RewardKind::Toast { description: "Well done".into() });
    assert_eq!(file.rewards[&0x2D].kind, RewardKind::Other("custom".into()));
    let table = &file.reward_tables[&0xF1];
    assert_eq!((table.entries.len(), table.loot_size, table.use_title, file.table_title(0xF1).as_str()), (2, 2, true, "Gems"));
    assert_eq!((table.draw(0.0), table.draw(0.24), table.draw(0.26), table.draw(0.999)), (Some(0), Some(0), Some(1), Some(1)), "weights 1 and 3");
    assert_eq!((file.auto_claim(&file.rewards[&0x21]), file.auto_claim(&file.rewards[&0x2B])), (AutoClaim::Disabled, AutoClaim::NoToast));
    assert!(file.toast_for(1) && !file.toast_for(2) && !file.toast_for(0x12), "a quest's disable_toast covers its tasks");
    assert_eq!(file.text("chapter", 0xC02, "title").as_deref(), Some("Stone age"));
    assert_eq!(file.text("quest", 1, "quest_desc").as_deref(), Some("Line one\nline two"));
    assert_eq!(plain("Logs \\& &aSaplings"), "Logs & Saplings", "an escaped & stays");
    assert_eq!(file.quest_icon(&file.quests[&1]).as_deref(), Some("minecraft:cobblestone"));
    assert_eq!(file.dependents(1), [2]);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn items_then_the_checkmark_complete_the_chapter_and_rewards_are_claimed_once() {
    let dir = folder("rules");
    let (file, _) = QuestFile::load(&dir, "en_us").unwrap();
    let (me, mate) = (0xAAu128, 0xBBu128);
    let mut team = TeamData::new(0x1234, "Rock team");
    let claim = |team: &mut TeamData, player, reward| team.claim(&file, player, reward, None, 7, &mut || 0.0);
    assert_eq!(claim(&mut team, me, 0x21), Err("quest not complete"));
    assert!(team.detect_items(&file, &[("minecraft:cobblestone".into(), 10)], 5));
    assert_eq!(team.progress(0x11), 10);
    assert!(!team.is_completed(1));
    assert!(team.detect_items(&file, &[("minecraft:cobblestone".into(), 64)], 6));
    assert_eq!(team.progress(0x11), 16, "progress stops at the count");
    assert!(team.is_completed(0x11) && team.is_completed(1));
    let bread = Ok(vec![Grant::Item { item: "minecraft:bread".into(), count: 3 }]);
    assert_eq!(claim(&mut team, me, 0x21), bread);
    assert_eq!(claim(&mut team, me, 0x21), Err("already claimed"));
    assert_eq!(claim(&mut team, mate, 0x21), bread, "a player reward is each member's");
    assert_eq!(claim(&mut team, me, 0x2D), Err("reward type not ported yet"));
    assert!(!team.check(&file, 0x11, 8), "an item task isn't ticked");
    assert!(team.check(&file, 0x12, 8));
    assert!(team.is_completed(2) && team.is_completed(0xC02));
    assert_eq!(claim(&mut team, mate, 0x23), Ok(vec![Grant::Item { item: "minecraft:apple".into(), count: 1 }]));
    assert_eq!(claim(&mut team, me, 0x23), Err("already claimed"), "a team reward is the team's once");

    // FTB's layout, and back.
    let text = snbt::write_string(&team.to_snbt());
    for key in ["version: 1", "uuid: \"00000000000000000000000000001234\"", "task_progress: {", "\"00000000000000000000000000000000:0000000000000023\": 7L"] {
        assert!(text.contains(key), "{key} in\n{text}");
    }
    let snbt::Value::Compound(c) = snbt::parse(&text).unwrap() else { panic!() };
    let mut back = TeamData::from_snbt(&c).unwrap();
    assert_eq!(back.kept.len(), 3, "repeatable, completion_count and player_data are written and kept");
    back.dirty = true;
    team.dirty = true;
    back.kept.clear();
    team.newly.clear();
    assert_eq!(back, team);
    assert_eq!(dashed(0x1234), "00000000-0000-0000-0000-000000001234");

    let parts = sync_parts(&file, &team, me);
    assert_eq!(parts.len(), 1);
    assert!(parts[0].reset && parts[0].claimed.contains(&"0000000000000021".to_owned()) && parts[0].claimed.contains(&"0000000000000023".to_owned()));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn every_reward_kind_gives_what_it_says() {
    let dir = folder("kinds");
    let (file, _) = QuestFile::load(&dir, "en_us").unwrap();
    let (me, mate) = (0xAAu128, 0xBBu128);
    let mut team = TeamData::new(1, "t");
    team.detect_items(&file, &[("minecraft:cobblestone".into(), 16)], 1);
    assert_eq!(team.newly, [0x11, 1], "the task, then its quest");
    let mut rolls = [0.0, 0.9].into_iter().cycle();
    let mut roll = move || rolls.next().unwrap();
    assert_eq!(team.claim_all_list(&file, me), [0x21, 0x22, 0x25, 0x26, 0x28, 0x29], "not the excluded, not the choice");
    assert_eq!(team.claim(&file, me, 0x22, None, 2, &mut roll), Ok(vec![Grant::Xp(10)]));
    assert_eq!(team.claim(&file, me, 0x24, None, 2, &mut roll), Ok(vec![Grant::Levels(5)]));
    let diamonds = Grant::Item { item: "minecraft:diamond".into(), count: 2 };
    assert_eq!(team.claim(&file, me, 0x25, None, 2, &mut roll), Ok(vec![diamonds.clone()]), "random: one draw");
    assert_eq!(team.claim(&file, me, 0x26, None, 2, &mut roll), Ok(vec![Grant::Levels(3), diamonds.clone()]), "loot: loot_size draws");
    assert_eq!(team.claim(&file, me, 0x27, None, 2, &mut roll), Err("pick one of the table's rewards"));
    assert_eq!(team.claim(&file, me, 0x27, Some(5), 2, &mut roll), Err("pick one of the table's rewards"));
    assert!(!team.is_claimed(&file, me, 0x27), "a refused pick claims nothing");
    assert_eq!(team.claim(&file, me, 0x27, Some(1), 2, &mut roll), Ok(vec![Grant::Levels(3)]));
    let command = Grant::Command { command: "/say {p} did it".into(), level: 3, silent: false, feedback: None };
    assert_eq!(team.claim(&file, me, 0x28, None, 2, &mut roll), Ok(vec![command]));
    assert_eq!(team.claim(&file, me, 0x29, None, 2, &mut roll), Ok(vec![Grant::Toast { reward: 0x29, description: "Well done".into() }]));
    assert_eq!(team.claim_all_list(&file, me), [0x21]);

    // Blocked rewards: nothing is claimed until the team is unblocked.
    team.rewards_blocked = true;
    assert_eq!(team.claim(&file, mate, 0x22, None, 3, &mut roll), Err("rewards blocked"));
    assert!(team.claim_all_list(&file, mate).is_empty());
    team.rewards_blocked = false;
    assert_eq!(team.claim(&file, mate, 0x22, None, 3, &mut roll), Ok(vec![Grant::Xp(10)]));

    // Auto-claim: a quest's own `auto`; a choice waits for its pick; the file's default is `disabled`.
    assert_eq!(auto_claims(&file, 3), [(0x2A, true), (0x2B, false)]);
    assert!(auto_claims(&file, 1).is_empty());
    team.newly.clear();
    assert!(team.check(&file, 0x13, 4));
    assert_eq!(team.newly, [0x13, 3, 0xC01], "the task, the quest, its chapter");
    assert_eq!(team.claim(&file, me, 0x2A, None, 4, &mut || 0.99), Ok(vec![Grant::Item { item: "minecraft:emerald".into(), count: 3 }]), "random_bonus");

    // An operator's completion skips the dependencies.
    let mut other = TeamData::new(2, "o");
    assert!(other.force_complete(&file, 2, 5));
    assert!(other.is_completed(2) && other.is_completed(0x12) && !other.is_completed(1));
    assert!(!other.force_complete(&file, 2, 5));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_dependency_done_later_completes_the_waiting_quest() {
    let dir = folder("later");
    let (file, _) = QuestFile::load(&dir, "en_us").unwrap();
    let mut team = TeamData::new(1, "t");
    assert!(team.check(&file, 0x12, 1), "flexible: the task progresses before its dependency");
    assert!(!team.is_completed(2));
    team.detect_items(&file, &[("minecraft:cobblestone".into(), 16)], 2);
    assert!(team.is_completed(1) && team.is_completed(2));
    fs::remove_dir_all(dir).unwrap();
}

/// The instance's own book, when `FTBQ_QUESTS_DIR` names it (never a fixture in the repository): every chapter reads.
#[test]
fn the_instance_book_reads_whole() {
    let Ok(dir) = std::env::var("FTBQ_QUESTS_DIR") else { return };
    let (file, warnings) = QuestFile::load(std::path::Path::new(&dir), "en_us").unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    let titled = file.chapters.iter().filter(|c| file.text("chapter", c.id, "title").is_some()).count();
    println!("BOOK chapters={} quests={} tasks={} rewards={} tables={} titled={titled}", file.chapters.len(), file.quests.len(), file.tasks.len(), file.rewards.len(), file.reward_tables.len());
    let mut kinds = std::collections::BTreeMap::<String, usize>::new();
    for r in file.rewards.values() {
        *kinds.entry(r.type_name().to_owned()).or_default() += 1;
    }
    let excluded = file.rewards.values().filter(|r| r.exclude_from_claim_all).count();
    let entries: usize = file.reward_tables.values().map(|t| t.entries.len()).sum();
    println!("REWARDS {kinds:?} excluded={excluded} table_entries={entries}");
    let missing = file.rewards.values().filter_map(|r| match r.kind {
        RewardKind::Random { table } | RewardKind::Loot { table } | RewardKind::Choice { table } => (!file.reward_tables.contains_key(&table)).then_some(table),
        _ => None,
    });
    assert_eq!(missing.count(), 0, "every table a reward names is in reward_tables/");
    assert!(!kinds.keys().any(|k| ["random", "loot", "choice", "xp", "xp_levels", "command", "toast"].contains(&k.as_str()) && file.rewards.values().any(|r| r.type_name() == k && matches!(r.kind, RewardKind::Other(_)))));
}
