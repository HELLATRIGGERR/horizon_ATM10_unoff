//! The quest file as FTB Quests keeps it in `config/ftbquests/quests/`: `data.snbt`, `chapter_groups.snbt`,
//! `chapters/*.snbt`, `reward_tables/*.snbt`, and the text split by locale under `lang/<locale>/` (the lang
//! splitter's layout: `file.snbt`, `chapter.snbt`, `chapter_group.snbt`, `reward_table.snbt`, `chapters/*.snbt`).
//! Written from the files' layout; nothing of the mod is copied.

use crate::snbt::{self, Compound, Value};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::Path,
};

/// A quest object's id as the files write it: 16 hex digits.
pub fn parse_id(text: &str) -> Option<u64> {
    u64::from_str_radix(text.trim(), 16).ok()
}
pub fn code(id: u64) -> String {
    format!("{id:016X}")
}

fn f64_of(v: Option<&Value>) -> Option<f64> {
    Some(match v? {
        Value::Float(f) => *f as f64,
        Value::Double(d) => *d,
        other => other.as_i64()? as f64,
    })
}
fn bool_of(c: &Compound, key: &str) -> bool {
    c.get(key).and_then(Value::as_i64).is_some_and(|v| v != 0)
}
fn id_of(c: &Compound, key: &str) -> Option<u64> {
    c.str(key).and_then(parse_id)
}
/// An item as the files write it: `"minecraft:stone"` or `{ id: ..., count: ..., components: {...} }`.
fn item_of(v: Option<&Value>) -> Option<(String, u64, bool)> {
    match v? {
        Value::String(s) => Some((s.clone(), 1, false)),
        Value::Compound(c) => Some((
            c.str("id")?.to_owned(),
            c.get("count").and_then(Value::as_i64).unwrap_or(1).max(1) as u64,
            c.get("components").is_some(),
        )),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TaskKind {
    /// `item`: hold `count` of `item` (by id; `components` are not matched yet).
    Item { item: String, count: u64, components: bool },
    /// `checkmark`: the player ticks it in the quest view.
    Checkmark,
    /// A type this slice doesn't run yet.
    Other(String),
}

#[derive(Clone, Debug)]
pub struct Task {
    pub id: u64,
    pub quest: u64,
    pub kind: TaskKind,
    pub optional: bool,
}

impl Task {
    /// The progress that completes the task.
    pub fn max_progress(&self) -> i64 {
        match &self.kind {
            TaskKind::Item { count, .. } => *count as i64,
            _ => 1,
        }
    }
    pub fn type_name(&self) -> &str {
        match &self.kind {
            TaskKind::Item { .. } => "item",
            TaskKind::Checkmark => "checkmark",
            TaskKind::Other(t) => t,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RewardKind {
    /// `item`: `count` of `item` (plain; `components` are not given yet).
    Item { item: String, count: u32, components: bool },
    Other(String),
}

#[derive(Clone, Debug)]
pub struct Reward {
    pub id: u64,
    pub quest: u64,
    pub kind: RewardKind,
    /// `team_reward`: one claim for the team; None takes the file's `default_reward_team`.
    pub team: Option<bool>,
    pub exclude_from_claim_all: bool,
}

impl Reward {
    pub fn type_name(&self) -> &str {
        match &self.kind {
            RewardKind::Item { .. } => "item",
            RewardKind::Other(t) => t,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Quest {
    pub id: u64,
    pub chapter: u64,
    pub x: f64,
    pub y: f64,
    pub size: f64,
    pub shape: Option<String>,
    pub icon: Option<String>,
    pub dependencies: Vec<u64>,
    pub tasks: Vec<u64>,
    pub rewards: Vec<u64>,
    pub optional: bool,
    pub invisible: bool,
    pub hide_dependency_lines: bool,
}

#[derive(Clone, Debug)]
pub struct Chapter {
    pub id: u64,
    pub filename: String,
    /// The group's id; 0 for the default group (no `group`).
    pub group: u64,
    pub icon: Option<String>,
    pub order_index: i64,
    pub default_shape: Option<String>,
    pub quests: Vec<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct QuestFile {
    pub default_reward_team: bool,
    pub default_consume_items: bool,
    pub default_shape: String,
    /// Ticks between two looks at the players' inventories (`detection_delay`).
    pub detection_delay: u32,
    pub grid_scale: f64,
    pub icon: Option<String>,
    /// The groups in `chapter_groups.snbt`'s order (the default group, 0, comes first).
    pub groups: Vec<u64>,
    /// The chapters in the book's order: by group, then `order_index`.
    pub chapters: Vec<Chapter>,
    pub quests: BTreeMap<u64, Quest>,
    pub tasks: BTreeMap<u64, Task>,
    pub rewards: BTreeMap<u64, Reward>,
    pub reward_tables: BTreeMap<u64, String>,
    /// The text, `<kind>.<ID>.<field>`: the chosen locale over en_us.
    pub lang: HashMap<String, Value>,
}

impl QuestFile {
    /// Reads the quest folder (`config/ftbquests/quests`). A chapter that doesn't parse is left out and named in
    /// the returned warnings.
    pub fn load(dir: &Path, locale: &str) -> Result<(QuestFile, Vec<String>), String> {
        let mut warnings = Vec::new();
        let read = |path: &Path| -> Result<Compound, String> {
            let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
            match snbt::parse(&text).map_err(|e| format!("{}: {e}", path.display()))? {
                Value::Compound(c) => Ok(c),
                _ => Err(format!("{}: not a compound", path.display())),
            }
        };
        let chapters_dir = dir.join("chapters");
        if !chapters_dir.is_dir() {
            return Err(format!("no quest chapters in {}", dir.display()));
        }
        let data = read(&dir.join("data.snbt")).unwrap_or_default();
        let mut file = QuestFile {
            default_reward_team: bool_of(&data, "default_reward_team"),
            default_consume_items: bool_of(&data, "default_consume_items"),
            default_shape: data.str("default_quest_shape").unwrap_or("circle").to_owned(),
            detection_delay: data.get("detection_delay").and_then(Value::as_i64).unwrap_or(20).clamp(1, 1200) as u32,
            grid_scale: f64_of(data.get("grid_scale")).unwrap_or(0.5),
            icon: item_of(data.get("icon")).map(|i| i.0),
            groups: vec![0],
            ..QuestFile::default()
        };
        if let Ok(groups) = read(&dir.join("chapter_groups.snbt")) {
            for g in groups.list("chapter_groups") {
                if let Some(id) = match g {
                    Value::Compound(c) => id_of(c, "id"),
                    _ => None,
                } {
                    file.groups.push(id);
                }
            }
        }
        let mut names: Vec<_> = fs::read_dir(&chapters_dir)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "snbt"))
            .collect();
        names.sort();
        for path in names {
            match read(&path) {
                Ok(c) => file.add_chapter(&c, &path),
                Err(e) => warnings.push(e),
            }
        }
        let order: HashMap<u64, usize> = file.groups.iter().enumerate().map(|(i, g)| (*g, i)).collect();
        file.chapters.sort_by_key(|c| (order.get(&c.group).copied().unwrap_or(usize::MAX), c.order_index));
        if let Ok(tables) = fs::read_dir(dir.join("reward_tables")) {
            for path in tables.flatten().map(|e| e.path()) {
                if let Ok(c) = read(&path) {
                    if let Some(id) = id_of(&c, "id") {
                        file.reward_tables.insert(id, path.file_stem().unwrap_or_default().to_string_lossy().into_owned());
                    }
                }
            }
        }
        for loc in ["en_us", locale] {
            let base = dir.join("lang");
            for path in lang_files(&base.join(loc)).into_iter().chain(Some(base.join(format!("{loc}.snbt")))) {
                if let Ok(c) = read(&path) {
                    file.lang.extend(c.0);
                }
            }
            if loc == locale {
                break;
            }
        }
        Ok((file, warnings))
    }

    fn add_chapter(&mut self, c: &Compound, path: &Path) {
        let Some(id) = id_of(c, "id") else { return };
        let mut chapter = Chapter {
            id,
            filename: c.str("filename").map(str::to_owned).unwrap_or_else(|| path.file_stem().unwrap_or_default().to_string_lossy().into_owned()),
            group: id_of(c, "group").unwrap_or(0),
            icon: item_of(c.get("icon")).map(|i| i.0),
            order_index: c.get("order_index").and_then(Value::as_i64).unwrap_or(0),
            default_shape: c.str("default_quest_shape").filter(|s| !s.is_empty()).map(str::to_owned),
            quests: Vec::new(),
        };
        for q in c.list("quests") {
            let Value::Compound(q) = q else { continue };
            let Some(qid) = id_of(q, "id") else { continue };
            let mut quest = Quest {
                id: qid,
                chapter: id,
                x: f64_of(q.get("x")).unwrap_or(0.0),
                y: f64_of(q.get("y")).unwrap_or(0.0),
                size: f64_of(q.get("size")).unwrap_or(1.0),
                shape: q.str("shape").filter(|s| !s.is_empty()).map(str::to_owned),
                icon: item_of(q.get("icon")).map(|i| i.0),
                dependencies: q.list("dependencies").iter().filter_map(|d| d.as_str().and_then(parse_id)).collect(),
                tasks: Vec::new(),
                rewards: Vec::new(),
                optional: bool_of(q, "optional"),
                invisible: bool_of(q, "invisible"),
                hide_dependency_lines: bool_of(q, "hide_dependency_lines"),
            };
            for t in q.list("tasks") {
                let Value::Compound(t) = t else { continue };
                let Some(tid) = id_of(t, "id") else { continue };
                let ty = t.str("type").unwrap_or("item");
                let kind = match ty {
                    "item" => match item_of(t.get("item")) {
                        Some((item, n, components)) => {
                            let count = t.get("count").and_then(Value::as_i64).map_or(n, |c| c.max(1) as u64);
                            TaskKind::Item { item, count, components }
                        }
                        None => TaskKind::Other("item".into()),
                    },
                    "checkmark" => TaskKind::Checkmark,
                    other => TaskKind::Other(other.to_owned()),
                };
                quest.tasks.push(tid);
                self.tasks.insert(tid, Task { id: tid, quest: qid, kind, optional: bool_of(t, "optional_task") });
            }
            for r in q.list("rewards") {
                let Value::Compound(r) = r else { continue };
                let Some(rid) = id_of(r, "id") else { continue };
                let kind = match (r.str("type").unwrap_or("item"), item_of(r.get("item"))) {
                    ("item", Some((item, n, components))) => {
                        let count = r.get("count").and_then(Value::as_i64).map_or(n, |c| c.max(1) as u64);
                        RewardKind::Item { item, count: count.min(u32::MAX as u64) as u32, components }
                    }
                    (other, _) => RewardKind::Other(other.to_owned()),
                };
                let team = r.get("team_reward").and_then(Value::as_i64).map(|v| v != 0);
                quest.rewards.push(rid);
                self.rewards.insert(rid, Reward { id: rid, quest: qid, kind, team, exclude_from_claim_all: bool_of(r, "exclude_from_claim_all") });
            }
            chapter.quests.push(qid);
            self.quests.insert(qid, quest);
        }
        self.chapters.push(chapter);
    }

    pub fn chapter(&self, id: u64) -> Option<&Chapter> {
        self.chapters.iter().find(|c| c.id == id)
    }

    /// A line of text (`quest.<ID>.title`), FTB's `&` formatting codes taken out.
    pub fn text(&self, kind: &str, id: u64, field: &str) -> Option<String> {
        let v = self.lang.get(&format!("{kind}.{}.{field}", code(id)))?;
        let raw = match v {
            Value::String(s) => s.clone(),
            Value::List(l) => l.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n"),
            _ => return None,
        };
        Some(plain(&raw)).filter(|s| !s.trim().is_empty())
    }

    /// A quest's icon: its own, else its first item task's item.
    pub fn quest_icon(&self, quest: &Quest) -> Option<String> {
        quest.icon.clone().or_else(|| {
            quest.tasks.iter().find_map(|t| match &self.tasks.get(t)?.kind {
                TaskKind::Item { item, .. } => Some(item.clone()),
                _ => None,
            })
        })
    }

    /// Whether a reward is claimed once for the team.
    pub fn team_reward(&self, reward: &Reward) -> bool {
        reward.team.unwrap_or(self.default_reward_team)
    }

    /// The quests that list `quest` as a dependency.
    pub fn dependents(&self, quest: u64) -> Vec<u64> {
        self.quests.values().filter(|q| q.dependencies.contains(&quest)).map(|q| q.id).collect()
    }
}

/// FTB's `&` codes (`&7`, `&l`, `&r`) out, `\n` written out as a line break.
pub fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' && chars.peek().is_some_and(|n| n.is_ascii_alphanumeric()) {
            chars.next();
        } else if c == '\\' && chars.peek() == Some(&'n') {
            chars.next();
            out.push('\n');
        } else if c != '\u{a7}' {
            out.push(c);
        } else {
            chars.next();
        }
    }
    out
}

fn lang_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for path in entries.flatten().map(|e| e.path()) {
            if path.is_dir() {
                out.extend(lang_files(&path));
            } else if path.extension().is_some_and(|x| x == "snbt") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}
