//! A quest folder of our own, written by the tests: two chapters in a group, an item quest, a checkmark quest that
//! waits on it, item rewards, and the split lang.

use crate::{file::*, snbt, team::*, *};
use std::{fs, path::PathBuf};

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ftbquests-core-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    for sub in ["chapters", "lang/en_us/chapters", "reward_tables"] {
        fs::create_dir_all(dir.join(sub)).unwrap();
    }
    fs::write(dir.join("data.snbt"), "{\n\tdefault_reward_team: false\n\tdetection_delay: 40\n\tgrid_scale: 0.5d\n}\n").unwrap();
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
		  rewards: [{ id: "0000000000000021", type: "item", item: { id: "minecraft:bread", count: 3 } }, { id: "0000000000000022", type: "xp", xp: 10 }] }
		{ id: "0000000000000002", x: 2.0d, y: 0.0d, dependencies: ["0000000000000001"], tasks: [{ id: "0000000000000012", type: "checkmark" }]
		  rewards: [{ id: "0000000000000023", type: "item", item: "minecraft:apple", team_reward: true }] }
	]
}"#,
    )
    .unwrap();
    fs::write(dir.join("chapters/zero.snbt"), "{ filename: \"zero\", id: \"0000000000000C01\", order_index: 0, quests: [] }").unwrap();
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
    assert_eq!((file.quests.len(), file.tasks.len(), file.rewards.len(), file.detection_delay), (2, 2, 3, 40));
    assert_eq!(file.tasks[&0x11].kind, TaskKind::Item { item: "minecraft:cobblestone".into(), count: 16, components: false });
    assert_eq!(file.rewards[&0x21].kind, RewardKind::Item { item: "minecraft:bread".into(), count: 3, components: false });
    assert_eq!(file.rewards[&0x23].kind, RewardKind::Item { item: "minecraft:apple".into(), count: 1, components: false });
    assert_eq!(file.text("chapter", 0xC02, "title").as_deref(), Some("Stone age"));
    assert_eq!(file.text("quest", 1, "quest_desc").as_deref(), Some("Line one\nline two"));
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
    assert_eq!(team.claim(&file, me, 0x21, 1), Err("quest not complete"));
    assert!(team.detect_items(&file, &[("minecraft:cobblestone".into(), 10)], 5));
    assert_eq!(team.progress(0x11), 10);
    assert!(!team.is_completed(1));
    assert!(team.detect_items(&file, &[("minecraft:cobblestone".into(), 64)], 6));
    assert_eq!(team.progress(0x11), 16, "progress stops at the count");
    assert!(team.is_completed(0x11) && team.is_completed(1));
    assert_eq!(team.claim(&file, me, 0x21, 7), Ok(Some(("minecraft:bread".into(), 3))));
    assert_eq!(team.claim(&file, me, 0x21, 7), Err("already claimed"));
    assert_eq!(team.claim(&file, mate, 0x21, 7), Ok(Some(("minecraft:bread".into(), 3))), "a player reward is each member's");
    assert_eq!(team.claim(&file, me, 0x22, 7), Err("reward type not ported yet"));
    assert!(!team.check(&file, 0x11, 8), "an item task isn't ticked");
    assert!(team.check(&file, 0x12, 8));
    assert!(team.is_completed(2) && team.is_completed(0xC02));
    assert_eq!(team.claim(&file, mate, 0x23, 9), Ok(Some(("minecraft:apple".into(), 1))));
    assert_eq!(team.claim(&file, me, 0x23, 9), Err("already claimed"), "a team reward is the team's once");

    // FTB's layout, and back.
    let text = snbt::write_string(&team.to_snbt());
    for key in ["version: 1", "uuid: \"00000000000000000000000000001234\"", "task_progress: {", "\"00000000000000000000000000000000:0000000000000023\": 9L"] {
        assert!(text.contains(key), "{key} in\n{text}");
    }
    let snbt::Value::Compound(c) = snbt::parse(&text).unwrap() else { panic!() };
    let mut back = TeamData::from_snbt(&c).unwrap();
    assert_eq!(back.kept.len(), 3, "repeatable, completion_count and player_data are written and kept");
    back.dirty = true;
    team.dirty = true;
    back.kept.clear();
    assert_eq!(back, team);
    assert_eq!(dashed(0x1234), "00000000-0000-0000-0000-000000001234");

    let parts = sync_parts(&file, &team, me);
    assert_eq!(parts.len(), 1);
    assert!(parts[0].reset && parts[0].claimed.contains(&"0000000000000021".to_owned()) && parts[0].claimed.contains(&"0000000000000023".to_owned()));
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
}
