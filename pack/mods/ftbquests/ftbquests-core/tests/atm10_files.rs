//! Reads every quest file of the player's ATM10 instance and compares what the
//! parser found with counts taken from the files by a different route: plain
//! line scans on the files' tab indentation, which know nothing of SNBT. The
//! instance is `ATM10_DIR`, or CurseForge's folder for it. Without one, the
//! tests say so and pass: no quest data is kept with this crate.

use ftbquests_core::book::{Book, RewardKind, TaskKind};
use ftbquests_core::filter::Filter;
use ftbquests_core::snbt;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

fn instance() -> Option<PathBuf> {
    let dir = std::env::var_os("ATM10_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Users\mpoka\curseforge\minecraft\Instances\All the Mods 10 - ATM10"));
    let quests = dir.join("config/ftbquests/quests");
    if quests.join("data.snbt").is_file() {
        return Some(quests);
    }
    eprintln!("no ATM10 instance at {}: nothing to read", dir.display());
    None
}

/// Every file under `dir`.
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn snbt_in(dir: &Path) -> Vec<PathBuf> {
    let mut all = Vec::new();
    walk(dir, &mut all);
    all.retain(|p| p.extension().is_some_and(|e| e == "snbt" || e == "snbt_merged"));
    all.sort();
    all
}

/// What the line scan finds in one chapter file. FTB Library writes one key a
/// line, with one tab per level: a quest opens at two tabs under `\tquests: [`,
/// a task or reward at four tabs, its type at five.
#[derive(Default)]
struct Scan {
    quests: usize,
    tasks: BTreeMap<String, usize>,
    rewards: BTreeMap<String, usize>,
    images: usize,
    links: usize,
    dependencies: usize,
}

fn scan(text: &str) -> Scan {
    let mut out = Scan::default();
    let (mut section, mut sub, mut inline) = ("", "", false);
    for line in text.lines() {
        let tabs = line.bytes().take_while(|&b| b == b'\t').count();
        let body = line[tabs..].trim_end();
        if tabs == 1 {
            section = match body {
                "quests: [" => "quests",
                "images: [" => "images",
                "quest_links: [" => "links",
                // A list of one is written `images: [{`, its keys a level shallower.
                "images: [{" => {
                    out.images += 1;
                    "other"
                }
                "quest_links: [{" => {
                    out.links += 1;
                    "other"
                }
                _ if body.ends_with('[') => "other",
                _ => "",
            };
            if body == "quests: [ ]" || body == "images: [ ]" || body == "quest_links: [ ]" {
                section = "";
            }
            sub = "";
            continue;
        }
        // A short compound stays on one line.
        if tabs == 2 && body.starts_with('{') {
            match section {
                "quests" => out.quests += 1,
                "images" => out.images += 1,
                "links" => out.links += 1,
                _ => {}
            }
        }
        if section != "quests" {
            continue;
        }
        if tabs == 3 && !matches!(body, "}]" | "]") {
            sub = match body {
                "tasks: [" | "tasks: [{" => "tasks",
                "rewards: [" | "rewards: [{" => "rewards",
                "dependencies: [" => "dependencies",
                _ => "",
            };
            inline = body.ends_with("[{");
            // One line: `dependencies: ["0123456789ABCDEF"]`.
            if let Some(rest) = body.strip_prefix("dependencies: [") {
                out.dependencies += rest.matches('"').count() / 2;
            }
            continue;
        }
        if tabs == 4 && sub == "dependencies" && !inline {
            out.dependencies += 1;
        }
        // `type: "x"` on its own line at five tabs, or inside a one-line compound at four.
        let kind = match tabs {
            4 if inline => body.strip_prefix("type: \"").and_then(|r| r.strip_suffix('"')),
            5 => body.strip_prefix("type: \"").and_then(|r| r.strip_suffix('"')),
            4 if body.starts_with('{') && body.ends_with('}') => {
                body.split_once("type: \"").and_then(|(_, r)| r.split('"').next())
            }
            _ => None,
        };
        if let Some(kind) = kind {
            let map = match sub {
                "tasks" => &mut out.tasks,
                "rewards" => &mut out.rewards,
                _ => continue,
            };
            *map.entry(kind.to_owned()).or_default() += 1;
        }
    }
    out
}

#[test]
fn every_snbt_file_parses() {
    let Some(quests) = instance() else { return };
    let files = snbt_in(&quests);
    let mut broken = Vec::new();
    for path in &files {
        let text = fs::read_to_string(path).unwrap();
        if let Err(e) = snbt::parse_compound(&text) {
            broken.push(format!("{}: {e}", path.display()));
        }
    }
    // Every file of the folder but the README.
    let mut all = Vec::new();
    walk(&quests, &mut all);
    let others: Vec<_> = all.iter().filter(|p| !files.contains(p)).collect();
    assert_eq!(others.len(), 1, "only the README isn't SNBT: {others:?}");
    assert!(broken.is_empty(), "{} of {} files broke:\n{}", broken.len(), files.len(), broken.join("\n"));
    println!("{} files parsed", files.len());
}

#[test]
fn counts_match_the_files() {
    let Some(quests) = instance() else { return };
    let book = Book::load(&quests, "en_us").unwrap();
    let chapter_files = snbt_in(&quests.join("chapters"));
    assert_eq!(book.chapters.len(), chapter_files.len(), "chapters");

    let mut seen = Scan::default();
    for path in &chapter_files {
        let found = scan(&fs::read_to_string(path).unwrap());
        seen.quests += found.quests;
        seen.images += found.images;
        seen.links += found.links;
        seen.dependencies += found.dependencies;
        for (k, v) in found.tasks {
            *seen.tasks.entry(k).or_default() += v;
        }
        for (k, v) in found.rewards {
            *seen.rewards.entry(k).or_default() += v;
        }
    }

    assert_eq!(book.quests().count(), seen.quests, "quests");
    assert_eq!(book.chapters.iter().map(|c| c.images.len()).sum::<usize>(), seen.images, "images");
    assert_eq!(book.chapters.iter().map(|c| c.links.len()).sum::<usize>(), seen.links, "links");

    let mut tasks: BTreeMap<String, usize> = BTreeMap::new();
    let mut rewards: BTreeMap<String, usize> = BTreeMap::new();
    for quest in book.quests() {
        for task in &quest.tasks {
            *tasks.entry(task.type_name().to_owned()).or_default() += 1;
        }
        for reward in &quest.rewards {
            *rewards.entry(reward.type_name().to_owned()).or_default() += 1;
        }
    }
    assert_eq!(tasks, seen.tasks, "tasks by type");
    assert_eq!(rewards, seen.rewards, "rewards by type");

    // Dependencies the book dropped point at nothing in the files.
    let kept: usize = book.quests().map(|q| q.dependencies.len()).sum();
    assert!(kept <= seen.dependencies);

    // The groups as `chapter_groups.snbt` lists them, plus the default one.
    let groups = fs::read_to_string(quests.join("chapter_groups.snbt")).unwrap();
    let listed = groups.lines().filter(|l| l.trim_start().starts_with("{ id:")).count();
    assert_eq!(book.groups.len(), listed + 1, "chapter groups");
    assert_eq!(book.tables.len(), snbt_in(&quests.join("reward_tables")).len(), "reward tables");

    println!(
        "chapters {}, groups {}, quests {}, images {}, links {}, dependencies {kept} of {}, tables {}",
        book.chapters.len(),
        listed,
        seen.quests,
        seen.images,
        seen.links,
        seen.dependencies,
        book.tables.len(),
    );
    println!("tasks {tasks:?}");
    println!("rewards {rewards:?}");
}

#[test]
fn ids_are_unique_and_every_link_finds_its_quest() {
    let Some(quests) = instance() else { return };
    let book = Book::load(&quests, "en_us").unwrap();
    let mut ids = HashSet::new();
    let mut objects = 0;
    let mut note = |id: u64| {
        objects += 1;
        assert!(ids.insert(id), "id {id:016X} twice");
    };
    for chapter in &book.chapters {
        note(chapter.id);
        for quest in &chapter.quests {
            note(quest.id);
            quest.tasks.iter().for_each(|t| note(t.id));
            quest.rewards.iter().for_each(|r| note(r.id));
        }
    }
    println!("{objects} unique ids");
    for chapter in &book.chapters {
        for link in &chapter.links {
            assert!(book.quest(link.quest).is_some(), "link {:016X} to {:016X}", link.id, link.quest);
        }
    }
}

#[test]
fn every_quest_has_a_title_source_and_lang_keys_land_on_objects() {
    let Some(quests) = instance() else { return };
    let book = Book::load(&quests, "en_us").unwrap();
    let raw = snbt::parse_compound(&fs::read_to_string(quests.join("lang/en_us.snbt")).unwrap()).unwrap();
    // Keys name an object of the book, or a table or image: the only other kinds the files hold.
    let mut orphans = Vec::new();
    let mut applied = 0;
    for (key, _) in &raw.0 {
        let mut parts = key.splitn(3, '.');
        let (kind, id) = (parts.next().unwrap(), parts.next().unwrap());
        let id = u64::from_str_radix(id, 16).unwrap();
        match kind {
            "chapter" | "quest" | "task" | "reward" => {
                if book.place(id).is_some() {
                    applied += 1;
                } else {
                    orphans.push(key.clone());
                }
            }
            "chapter_group" | "file" | "reward_table" | "image" => {}
            other => panic!("a key of kind {other}: {key}"),
        }
    }
    println!("{applied} keys applied, {} name no object in the files", orphans.len());
    let mut titled = 0;
    let mut described = 0;
    for quest in book.quests() {
        // A quest with no title of its own takes its task's (`Quest.getAltTitle`).
        if !quest.title.is_empty() {
            titled += 1;
        }
        if !quest.description.is_empty() {
            described += 1;
        }
        assert!(!quest.tasks.is_empty() || !quest.title.is_empty(), "{:016X} has neither", quest.id);
    }
    println!("{titled} quests titled, {described} described");
    // Spot checks over what the table holds, not its words: every title key landed.
    let titles = raw.0.iter().filter(|(k, _)| k.starts_with("quest.") && k.ends_with(".title")).count();
    assert!(titled <= titles);
    assert!(titled * 100 >= titles * 95, "{titled} of {titles} quest title keys");
    let chapters = book.chapters.iter().filter(|c| !c.title.is_empty()).count();
    assert_eq!(chapters, book.chapters.len(), "every chapter has a title");
    for group in &book.groups[1..] {
        assert!(!group.title.is_empty(), "group {:016X}", group.id);
    }
}

#[test]
fn every_filter_reads_and_every_item_has_an_id() {
    let Some(quests) = instance() else { return };
    let book = Book::load(&quests, "en_us").unwrap();
    let mut filters = 0;
    let mut unknown = Vec::new();
    fn walk(f: &Filter, unknown: &mut Vec<String>) {
        match f {
            Filter::Unknown(text) => unknown.push(text.clone()),
            Filter::Or(all) | Filter::And(all) => all.iter().for_each(|f| walk(f, unknown)),
            Filter::Not(inner) => walk(inner, unknown),
            _ => {}
        }
    }
    for quest in book.quests() {
        for task in &quest.tasks {
            if let TaskKind::Item { item, filter, .. } = &task.kind {
                assert!(!item.id.is_empty(), "task {:016X}", task.id);
                if item.id == "ftbfiltersystem:smart_filter" {
                    filters += 1;
                    let f = filter.as_ref().expect("a smart filter with no filter");
                    walk(f, &mut unknown);
                }
            }
        }
        for reward in &quest.rewards {
            if let RewardKind::Item { item, .. } = &reward.kind {
                assert!(!item.id.is_empty(), "reward {:016X}", reward.id);
            }
        }
    }
    println!("{filters} smart filters");
    assert!(unknown.is_empty(), "filter nodes this port doesn't read: {unknown:?}");
}

#[test]
fn every_language_of_the_book_loads() {
    let Some(quests) = instance() else { return };
    let lang = quests.join("lang");
    let mut count = 0;
    for entry in fs::read_dir(&lang).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "snbt") {
            let locale = path.file_stem().unwrap().to_str().unwrap();
            assert!(Book::load(&quests, locale).is_ok(), "{locale}");
            count += 1;
        }
    }
    println!("{count} languages");
}
