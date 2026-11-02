//! The quest book as FTB Quests' files describe it (`config/ftbquests/quests`):
//! `data.snbt`, `chapter_groups.snbt`, `chapters/*.snbt`, `reward_tables/*.snbt`.
//! Field names and defaults follow the files FTB Quests 2101.1.36 writes; what
//! this port does not act on yet is still read, so nothing of a file is lost.

use crate::filter::Filter;
use crate::lang::Lang;
use crate::snbt::{self, Compound, Value};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// An object's id: 16 hexadecimal digits in the files, a long in the game.
pub type Id = u64;

/// Reads an id as the files write it (`"732824E03978A934"`), a bare long
/// (`table_id`), or `#ABC` (`QuestObjectBase.parseCodeString`).
pub fn parse_id(value: &Value) -> Option<Id> {
    match value {
        Value::Str(s) => parse_hex(s),
        _ => value.as_i64().map(|n| n as u64),
    }
}

pub fn parse_hex(s: &str) -> Option<Id> {
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.is_empty() || s.len() > 16 {
        return None;
    }
    u64::from_str_radix(s, 16).ok()
}

/// The form the files and the translation keys use.
pub fn id_string(id: Id) -> String {
    format!("{id:016X}")
}

/// `Tristate`: yes, no, or whatever the chapter or file says.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tristate {
    #[default]
    Default,
    True,
    False,
}

impl Tristate {
    pub fn read(nbt: &Compound, key: &str) -> Self {
        match nbt.bool(key) {
            Some(true) => Tristate::True,
            Some(false) => Tristate::False,
            None => Tristate::Default,
        }
    }
    pub fn get(self, default: bool) -> bool {
        match self {
            Tristate::Default => default,
            Tristate::True => true,
            Tristate::False => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Progression {
    #[default]
    Default,
    Linear,
    Flexible,
}

impl Progression {
    fn read(nbt: &Compound) -> Self {
        match nbt.str("progression_mode") {
            Some("linear") => Progression::Linear,
            Some("flexible") => Progression::Flexible,
            _ => Progression::Default,
        }
    }
}

/// `DependencyRequirement`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Requirement {
    #[default]
    AllCompleted,
    OneCompleted,
    AllStarted,
    OneStarted,
}

impl Requirement {
    fn read(nbt: &Compound) -> Self {
        match nbt.str("dependency_requirement") {
            Some("one_completed") => Requirement::OneCompleted,
            Some("all_started") => Requirement::AllStarted,
            Some("one_started") => Requirement::OneStarted,
            _ => Requirement::AllCompleted,
        }
    }
    pub fn needs_completion(self) -> bool {
        matches!(self, Requirement::AllCompleted | Requirement::OneCompleted)
    }
    pub fn needs_only_one(self) -> bool {
        matches!(self, Requirement::OneCompleted | Requirement::OneStarted)
    }
}

/// `ComponentMatchType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MatchComponents {
    #[default]
    None,
    Fuzzy,
    Strict,
}

/// An item as the files write one: `"minecraft:stone"`, or
/// `{ id, count, components }`. `components` are kept as read for the game to
/// turn into a real stack.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemRef {
    pub id: String,
    pub count: i64,
    pub components: Option<Compound>,
}

impl ItemRef {
    pub fn read(value: Option<&Value>) -> Option<Self> {
        match value? {
            Value::Str(id) if !id.is_empty() => Some(ItemRef { id: id.clone(), count: 1, components: None }),
            Value::Compound(c) => {
                let id = c.str("id")?;
                if id.is_empty() {
                    return None;
                }
                Some(ItemRef {
                    id: id.to_owned(),
                    count: c.i64("count").unwrap_or(1),
                    components: c.compound("components").cloned(),
                })
            }
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Settings {
    pub icon: Option<ItemRef>,
    pub default_quest_shape: String,
    pub default_consume_items: bool,
    pub default_autoclaim_rewards: String,
    pub default_reward_team: bool,
    pub detection_delay: u32,
    pub grid_scale: f64,
    pub progression: Progression,
    pub show_lock_icons: bool,
    pub hide_excluded_quests: bool,
    pub disable_gui: bool,
    pub pause_game: bool,
    pub drop_book_on_death: bool,
    pub lock_message: String,
    pub fallback_locale: String,
    pub version: i64,
    /// `presets`: a shape and size by name.
    pub presets: Vec<(String, String, f64)>,
}

#[derive(Clone, Debug)]
pub struct Group {
    pub id: Id,
    pub title: String,
}

#[derive(Clone, Debug, Default)]
pub struct Image {
    pub id: Id,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub rotation: f64,
    pub image: String,
    pub order: i64,
    pub dependency: Option<Id>,
    pub dev: bool,
    pub hover: Vec<String>,
    pub alpha: i64,
    pub corner: bool,
}

/// A link: another chapter's quest shown in this one.
#[derive(Clone, Debug, Default)]
pub struct Link {
    pub id: Id,
    pub quest: Id,
    pub x: f64,
    pub y: f64,
    pub shape: String,
    pub size: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Chapter {
    pub id: Id,
    pub filename: String,
    pub group: Id,
    pub order: i64,
    pub icon: Option<ItemRef>,
    pub title: String,
    pub subtitle: Vec<String>,
    pub default_quest_shape: String,
    pub default_hide_dependency_lines: bool,
    pub default_min_width: i64,
    pub progression: Progression,
    pub consume_items: Tristate,
    pub always_invisible: bool,
    pub hide_quest_details_until_startable: bool,
    pub hide_quest_until_deps_complete: bool,
    pub hide_quest_until_deps_visible: bool,
    pub require_sequential_tasks: bool,
    pub autofocus: Option<Id>,
    pub images: Vec<Image>,
    pub links: Vec<Link>,
    pub quests: Vec<Quest>,
}

#[derive(Clone, Debug, Default)]
pub struct Quest {
    pub id: Id,
    pub x: f64,
    pub y: f64,
    pub shape: String,
    pub size: f64,
    pub icon: Option<ItemRef>,
    pub icon_scale: f64,
    pub title: String,
    pub subtitle: String,
    pub description: Vec<String>,
    pub tasks: Vec<Task>,
    pub rewards: Vec<Reward>,
    pub dependencies: Vec<Id>,
    pub requirement: Requirement,
    pub min_required_dependencies: i64,
    pub hide_dependency_lines: Tristate,
    pub hide_dependent_lines: bool,
    pub hide_until_deps_complete: Tristate,
    pub hide_until_deps_visible: Tristate,
    pub hide_details_until_startable: Tristate,
    pub hide_text_until_complete: Tristate,
    pub hide_lock_icon: bool,
    pub optional: bool,
    pub invisible: bool,
    pub invisible_until_tasks: i64,
    pub can_repeat: Tristate,
    pub min_width: i64,
    pub progression: Progression,
    pub require_sequential_tasks: Tristate,
    pub disable_toast: bool,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum TaskKind {
    Item {
        item: ItemRef,
        count: u64,
        consume: Tristate,
        only_from_crafting: Tristate,
        match_components: MatchComponents,
        task_screen_only: bool,
        /// `ftbfiltersystem:smart_filter`'s filter, when the item is one.
        filter: Option<Filter>,
    },
    Checkmark,
    /// Another type: its name (`kill`) and its fields as read.
    Other { kind: String, fields: Compound },
}

#[derive(Clone, Debug)]
pub struct Task {
    pub id: Id,
    pub title: String,
    pub icon: Option<ItemRef>,
    pub optional: bool,
    pub kind: TaskKind,
}

impl Task {
    /// `Task.getMaxProgress`: the item count; 1 for a checkmark and for the
    /// types that aren't worked yet.
    pub fn max_progress(&self) -> u64 {
        match &self.kind {
            TaskKind::Item { count, .. } => *count,
            _ => 1,
        }
    }
    /// A type this port can complete.
    pub fn is_supported(&self) -> bool {
        !matches!(self.kind, TaskKind::Other { .. })
    }
    pub fn type_name(&self) -> &str {
        match &self.kind {
            TaskKind::Item { .. } => "item",
            TaskKind::Checkmark => "checkmark",
            TaskKind::Other { kind, .. } => kind,
        }
    }
}

#[derive(Clone, Debug)]
pub enum RewardKind {
    Item {
        item: ItemRef,
        count: i64,
        random_bonus: i64,
        only_one: bool,
    },
    /// Another type: its name (`xp`) and its fields as read.
    Other { kind: String, fields: Compound },
}

#[derive(Clone, Debug)]
pub struct Reward {
    pub id: Id,
    pub title: String,
    pub icon: Option<ItemRef>,
    pub team: Tristate,
    pub auto: String,
    pub exclude_from_claim_all: bool,
    pub kind: RewardKind,
}

impl Reward {
    pub fn is_supported(&self) -> bool {
        matches!(self.kind, RewardKind::Item { .. })
    }
    pub fn type_name(&self) -> &str {
        match &self.kind {
            RewardKind::Item { .. } => "item",
            RewardKind::Other { kind, .. } => kind,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct RewardTable {
    pub id: Id,
    pub filename: String,
    pub title: String,
    pub icon: Option<ItemRef>,
    pub rewards: usize,
}

/// Where an object sits, for finding it by id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Chapter(usize),
    Quest(usize, usize),
    Task(usize, usize, usize),
    Reward(usize, usize, usize),
}

/// A whole book, translated into one language.
#[derive(Clone, Debug, Default)]
pub struct Book {
    pub settings: Settings,
    pub groups: Vec<Group>,
    /// By group, then `order_index`, as the book lists them.
    pub chapters: Vec<Chapter>,
    pub tables: Vec<RewardTable>,
    pub title: String,
    places: HashMap<Id, Place>,
    dependants: HashMap<Id, Vec<Id>>,
}

/// A file that wouldn't load.
#[derive(Debug)]
pub struct LoadError {
    pub path: PathBuf,
    pub message: String,
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for LoadError {}

fn read_compound(path: &Path) -> Result<Compound, LoadError> {
    let error = |message: String| LoadError { path: path.to_owned(), message };
    let text = fs::read_to_string(path).map_err(|e| error(e.to_string()))?;
    snbt::parse_compound(&text).map_err(|e| error(e.to_string()))
}

/// The `.snbt` files directly in `dir`, by name.
fn snbt_files(dir: &Path) -> Result<Vec<PathBuf>, LoadError> {
    let error = |message: String| LoadError { path: dir.to_owned(), message };
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| error(e.to_string()))? {
        let path = entry.map_err(|e| error(e.to_string()))?.path();
        if path.extension().is_some_and(|e| e == "snbt") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn own_id(nbt: &Compound) -> Id {
    nbt.get("id").and_then(parse_id).unwrap_or(0)
}

fn strings(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::List(items)) => items.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect(),
        Some(Value::Str(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

impl Book {
    /// Reads a `quests` folder in the language `locale` (`en_us`), falling back
    /// on the file's fallback language and then English, as `TranslationManager` does.
    pub fn load(folder: &Path, locale: &str) -> Result<Self, LoadError> {
        let settings = read_settings(&read_compound(&folder.join("data.snbt"))?);
        let lang = Lang::load(&folder.join("lang"), locale, &settings.fallback_locale)?;
        Self::load_with(folder, settings, &lang)
    }

    fn load_with(folder: &Path, settings: Settings, lang: &Lang) -> Result<Self, LoadError> {
        let mut book = Book { settings, ..Book::default() };
        book.title = lang.title("file", 1).unwrap_or_default();

        let groups = read_compound(&folder.join("chapter_groups.snbt"))?;
        // The default group has no file entry: `Chapter.group` is empty for it.
        book.groups.push(Group { id: 0, title: String::new() });
        for group in groups.list("chapter_groups").unwrap_or_default() {
            let Some(group) = group.as_compound() else { continue };
            let id = own_id(group);
            book.groups.push(Group { id, title: lang.title("chapter_group", id).unwrap_or_default() });
        }

        let mut chapters = Vec::new();
        for path in snbt_files(&folder.join("chapters"))? {
            chapters.push(read_chapter(&read_compound(&path)?, &path, lang));
        }
        let rank: HashMap<Id, usize> = book.groups.iter().enumerate().map(|(i, g)| (g.id, i)).collect();
        // A chapter whose group the file doesn't list goes in the default group.
        for chapter in &mut chapters {
            if !rank.contains_key(&chapter.group) {
                chapter.group = 0;
            }
        }
        chapters.sort_by_key(|c| (rank[&c.group], c.order));
        book.chapters = chapters;

        let tables = folder.join("reward_tables");
        if tables.is_dir() {
            for path in snbt_files(&tables)? {
                let nbt = read_compound(&path)?;
                let id = own_id(&nbt);
                book.tables.push(RewardTable {
                    id,
                    filename: nbt.str("filename").unwrap_or_default().to_owned(),
                    title: lang.title("reward_table", id).unwrap_or_default(),
                    icon: ItemRef::read(nbt.get("icon")),
                    rewards: nbt.list("rewards").map_or(0, <[Value]>::len),
                });
            }
        }
        book.index();
        Ok(book)
    }

    /// Builds the lookups; call it after changing what the book holds.
    pub fn index(&mut self) {
        self.places.clear();
        self.dependants.clear();
        for (c, chapter) in self.chapters.iter().enumerate() {
            self.places.insert(chapter.id, Place::Chapter(c));
            for (q, quest) in chapter.quests.iter().enumerate() {
                self.places.insert(quest.id, Place::Quest(c, q));
                for (t, task) in quest.tasks.iter().enumerate() {
                    self.places.insert(task.id, Place::Task(c, q, t));
                }
                for (r, reward) in quest.rewards.iter().enumerate() {
                    self.places.insert(reward.id, Place::Reward(c, q, r));
                }
            }
        }
        // A dependency on nothing is dropped, as `Quest.readData` drops one.
        let places = &self.places;
        for chapter in &mut self.chapters {
            for quest in &mut chapter.quests {
                quest.dependencies.retain(|d| places.contains_key(d));
                for &dep in &quest.dependencies {
                    self.dependants.entry(dep).or_default().push(quest.id);
                }
            }
        }
    }

    pub fn place(&self, id: Id) -> Option<Place> {
        self.places.get(&id).copied()
    }
    pub fn quest(&self, id: Id) -> Option<&Quest> {
        match self.place(id)? {
            Place::Quest(c, q) => Some(&self.chapters[c].quests[q]),
            _ => None,
        }
    }
    pub fn chapter_of(&self, quest: Id) -> Option<&Chapter> {
        match self.place(quest)? {
            Place::Quest(c, _) => Some(&self.chapters[c]),
            _ => None,
        }
    }
    /// A task with its quest and chapter.
    pub fn task(&self, id: Id) -> Option<(&Task, &Quest, &Chapter)> {
        match self.place(id)? {
            Place::Task(c, q, t) => {
                let chapter = &self.chapters[c];
                let quest = &chapter.quests[q];
                Some((&quest.tasks[t], quest, chapter))
            }
            _ => None,
        }
    }
    /// A reward with its quest.
    pub fn reward(&self, id: Id) -> Option<(&Reward, &Quest)> {
        match self.place(id)? {
            Place::Reward(c, q, r) => {
                let quest = &self.chapters[c].quests[q];
                Some((&quest.rewards[r], quest))
            }
            _ => None,
        }
    }
    pub fn dependants(&self, id: Id) -> &[Id] {
        self.dependants.get(&id).map_or(&[], Vec::as_slice)
    }
    pub fn quests(&self) -> impl Iterator<Item = &Quest> {
        self.chapters.iter().flat_map(|c| c.quests.iter())
    }
    /// Chapters in a group, in order.
    pub fn chapters_in(&self, group: Id) -> impl Iterator<Item = &Chapter> {
        self.chapters.iter().filter(move |c| c.group == group)
    }

    /// `Quest.getShape`: its own, the chapter's, the file's.
    pub fn quest_shape<'a>(&'a self, chapter: &'a Chapter, quest: &'a Quest) -> &'a str {
        self.shape_of(chapter, &quest.shape)
    }
    pub fn shape_of<'a>(&'a self, chapter: &'a Chapter, own: &'a str) -> &'a str {
        if !own.is_empty() && own != "default" {
            own
        } else if !chapter.default_quest_shape.is_empty() && chapter.default_quest_shape != "default" {
            &chapter.default_quest_shape
        } else {
            &self.settings.default_quest_shape
        }
    }
    /// `Quest.getProgressionMode`.
    pub fn progression_of(&self, chapter: &Chapter, quest: &Quest) -> Progression {
        if quest.progression != Progression::Default {
            return quest.progression;
        }
        if chapter.progression != Progression::Default {
            return chapter.progression;
        }
        self.settings.progression
    }
    /// `ItemTask.consumesResources`: the task's, then the chapter's, then the file's.
    pub fn consumes(&self, chapter: &Chapter, consume: Tristate) -> bool {
        consume.get(chapter.consume_items.get(self.settings.default_consume_items))
    }
}

fn read_settings(nbt: &Compound) -> Settings {
    let mut presets = Vec::new();
    if let Some(found) = nbt.compound("presets") {
        for (name, value) in &found.0 {
            if let Some(p) = value.as_compound() {
                presets.push((name.clone(), p.str("shape").unwrap_or_default().to_owned(), p.f64("size").unwrap_or(1.0)));
            }
        }
    }
    Settings {
        icon: ItemRef::read(nbt.get("icon")),
        default_quest_shape: nbt.str("default_quest_shape").unwrap_or("circle").to_owned(),
        default_consume_items: nbt.bool("default_consume_items").unwrap_or(false),
        default_autoclaim_rewards: nbt.str("default_autoclaim_rewards").unwrap_or("disabled").to_owned(),
        default_reward_team: nbt.bool("default_reward_team").unwrap_or(false),
        detection_delay: nbt.i64("detection_delay").unwrap_or(20).clamp(0, i64::from(u32::MAX)) as u32,
        grid_scale: nbt.f64("grid_scale").unwrap_or(0.5),
        progression: Progression::read(nbt),
        show_lock_icons: nbt.bool("show_lock_icons").unwrap_or(true),
        hide_excluded_quests: nbt.bool("hide_excluded_quests").unwrap_or(false),
        disable_gui: nbt.bool("disable_gui").unwrap_or(false),
        pause_game: nbt.bool("pause_game").unwrap_or(false),
        drop_book_on_death: nbt.bool("drop_book_on_death").unwrap_or(false),
        lock_message: nbt.str("lock_message").unwrap_or_default().to_owned(),
        fallback_locale: nbt.str("fallback_locale").unwrap_or_default().to_owned(),
        version: nbt.i64("version").unwrap_or(0),
        presets,
    }
}

fn read_chapter(nbt: &Compound, path: &Path, lang: &Lang) -> Chapter {
    let id = own_id(nbt);
    let filename = nbt
        .str("filename")
        .map(str::to_owned)
        .or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let list = |key: &str| nbt.list(key).unwrap_or_default().iter().filter_map(Value::as_compound);
    let group = nbt.get("group").and_then(parse_id).unwrap_or(0);
    let mut images: Vec<Image> = list("images").map(read_image).collect();
    images.sort_by_key(|i| i.order);
    Chapter {
        id,
        filename,
        group,
        order: nbt.i64("order_index").unwrap_or(0),
        icon: ItemRef::read(nbt.get("icon")),
        title: lang.title("chapter", id).unwrap_or_default(),
        subtitle: lang.lines("chapter", id, "chapter_subtitle"),
        default_quest_shape: nbt.str("default_quest_shape").unwrap_or_default().to_owned(),
        default_hide_dependency_lines: nbt.bool("default_hide_dependency_lines").unwrap_or(false),
        default_min_width: nbt.i64("default_min_width").unwrap_or(0),
        progression: Progression::read(nbt),
        consume_items: Tristate::read(nbt, "consume_items"),
        always_invisible: nbt.bool("always_invisible").unwrap_or(false),
        hide_quest_details_until_startable: nbt.bool("hide_quest_details_until_startable").unwrap_or(false),
        hide_quest_until_deps_complete: nbt.bool("hide_quest_until_deps_complete").unwrap_or(false),
        hide_quest_until_deps_visible: nbt.bool("hide_quest_until_deps_visible").unwrap_or(false),
        require_sequential_tasks: nbt.bool("require_sequential_tasks").unwrap_or(false),
        autofocus: nbt.get("autofocus_id").and_then(parse_id).filter(|&a| a != 0),
        images,
        links: list("quest_links").map(read_link).collect(),
        quests: list("quests").map(|q| read_quest(q, lang)).collect(),
    }
}

fn read_image(nbt: &Compound) -> Image {
    Image {
        id: own_id(nbt),
        x: nbt.f64("x").unwrap_or(0.0),
        y: nbt.f64("y").unwrap_or(0.0),
        width: nbt.f64("width").unwrap_or(1.0),
        height: nbt.f64("height").unwrap_or(1.0),
        rotation: nbt.f64("rotation").unwrap_or(0.0),
        image: nbt.str("image").unwrap_or_default().to_owned(),
        order: nbt.i64("order").unwrap_or(0),
        dependency: nbt.get("dependency").and_then(parse_id).filter(|&d| d != 0),
        dev: nbt.bool("dev").unwrap_or(false),
        hover: strings(nbt.get("hover")),
        alpha: nbt.i64("alpha").unwrap_or(255),
        corner: nbt.bool("corner").unwrap_or(false),
    }
}

fn read_link(nbt: &Compound) -> Link {
    Link {
        id: own_id(nbt),
        quest: nbt.get("linked_quest").and_then(parse_id).unwrap_or(0),
        x: nbt.f64("x").unwrap_or(0.0),
        y: nbt.f64("y").unwrap_or(0.0),
        shape: nbt.str("shape").unwrap_or_default().to_owned(),
        size: nbt.f64("size").unwrap_or(1.0),
    }
}

fn read_quest(nbt: &Compound, lang: &Lang) -> Quest {
    let id = own_id(nbt);
    let list = |key: &str| nbt.list(key).unwrap_or_default().iter().filter_map(Value::as_compound);
    let dependencies = match nbt.get("dependencies") {
        Some(Value::List(items)) => items.iter().filter_map(parse_id).collect(),
        Some(Value::IntArray(ints)) => ints.iter().map(|&n| n as u32 as u64).collect(),
        _ => Vec::new(),
    };
    // A size of 0 is unset: `Quest.getSize` draws those at 1.
    let size = nbt.f64("size").filter(|&s| s > 0.0).unwrap_or(1.0);
    Quest {
        id,
        x: nbt.f64("x").unwrap_or(0.0),
        y: nbt.f64("y").unwrap_or(0.0),
        shape: nbt.str("shape").unwrap_or_default().to_owned(),
        size,
        icon: ItemRef::read(nbt.get("icon")),
        icon_scale: nbt.f64("icon_scale").unwrap_or(1.0),
        title: lang.title("quest", id).unwrap_or_default(),
        subtitle: lang.text("quest", id, "quest_subtitle").unwrap_or_default(),
        description: lang.lines("quest", id, "quest_desc"),
        tasks: list("tasks").map(|t| read_task(t, lang)).collect(),
        rewards: list("rewards").map(|r| read_reward(r, lang)).collect(),
        dependencies,
        requirement: Requirement::read(nbt),
        min_required_dependencies: nbt.i64("min_required_dependencies").unwrap_or(0),
        hide_dependency_lines: Tristate::read(nbt, "hide_dependency_lines"),
        hide_dependent_lines: nbt.bool("hide_dependent_lines").unwrap_or(false),
        hide_until_deps_complete: Tristate::read(nbt, "hide_until_deps_complete"),
        // The files of old versions call it `hide`.
        hide_until_deps_visible: if nbt.get("hide").is_some() {
            Tristate::read(nbt, "hide")
        } else {
            Tristate::read(nbt, "hide_until_deps_visible")
        },
        hide_details_until_startable: Tristate::read(nbt, "hide_details_until_startable"),
        hide_text_until_complete: Tristate::read(nbt, "hide_text_until_complete"),
        hide_lock_icon: nbt.bool("hide_lock_icon").unwrap_or(false),
        optional: nbt.bool("optional").unwrap_or(false),
        invisible: nbt.bool("invisible").unwrap_or(false),
        invisible_until_tasks: nbt.i64("invisible_until_tasks").unwrap_or(0),
        can_repeat: Tristate::read(nbt, "can_repeat"),
        min_width: nbt.i64("min_width").unwrap_or(0),
        progression: Progression::read(nbt),
        require_sequential_tasks: Tristate::read(nbt, "require_sequential_tasks"),
        disable_toast: nbt.bool("disable_toast").unwrap_or(false),
        tags: strings(nbt.get("tags")),
    }
}

fn read_task(nbt: &Compound, lang: &Lang) -> Task {
    let id = own_id(nbt);
    let kind = match nbt.str("type").unwrap_or_default() {
        "item" => {
            let item = ItemRef::read(nbt.get("item")).unwrap_or_default();
            let filter = item
                .components
                .as_ref()
                .and_then(|c| c.str("ftbfiltersystem:filter"))
                .map(Filter::parse);
            TaskKind::Item {
                count: nbt.i64("count").unwrap_or(1).max(1) as u64,
                consume: Tristate::read(nbt, "consume_items"),
                only_from_crafting: Tristate::read(nbt, "only_from_crafting"),
                match_components: match nbt.str("match_components") {
                    Some("fuzzy") => MatchComponents::Fuzzy,
                    Some("strict") => MatchComponents::Strict,
                    _ => MatchComponents::None,
                },
                task_screen_only: nbt.bool("task_screen_only").unwrap_or(false),
                filter,
                item,
            }
        }
        "checkmark" => TaskKind::Checkmark,
        other => TaskKind::Other { kind: other.to_owned(), fields: nbt.clone() },
    };
    Task {
        id,
        title: lang.title("task", id).unwrap_or_default(),
        icon: ItemRef::read(nbt.get("icon")),
        optional: nbt.bool("optional_task").unwrap_or(false),
        kind,
    }
}

fn read_reward(nbt: &Compound, lang: &Lang) -> Reward {
    let id = own_id(nbt);
    let kind = match nbt.str("type").unwrap_or_default() {
        "item" => {
            let item = ItemRef::read(nbt.get("item")).unwrap_or_default();
            // `ItemReward.readData`: a count of 0 means the item's own.
            let count = match nbt.i64("count").unwrap_or(0) {
                0 => item.count.max(1),
                n => n,
            };
            RewardKind::Item {
                item,
                count,
                random_bonus: nbt.i64("random_bonus").unwrap_or(0),
                only_one: nbt.bool("only_one").unwrap_or(false),
            }
        }
        other => RewardKind::Other { kind: other.to_owned(), fields: nbt.clone() },
    };
    Reward {
        id,
        title: lang.title("reward", id).unwrap_or_default(),
        icon: ItemRef::read(nbt.get("icon")),
        team: Tristate::read(nbt, "team_reward"),
        auto: nbt.str("auto").unwrap_or_default().to_owned(),
        exclude_from_claim_all: nbt.bool("exclude_from_claim_all").unwrap_or(false),
        kind,
    }
}

#[cfg(test)]
pub(crate) mod fixture {
    use super::*;

    /// A book from text: `(chapter id, chapter file body)`, no translations.
    pub fn book(settings: &str, chapters: &[&str]) -> Book {
        let lang = Lang::default();
        let mut book = Book {
            settings: read_settings(&snbt::parse_compound(settings).unwrap()),
            ..Book::default()
        };
        book.groups.push(Group { id: 0, title: String::new() });
        for text in chapters {
            let nbt = snbt::parse_compound(text).unwrap();
            book.chapters.push(read_chapter(&nbt, Path::new("x.snbt"), &lang));
        }
        book.index();
        book
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::book;
    use super::*;

    const CHAPTER: &str = r#"{
        id: "00000000000000C1"
        filename: "first"
        group: ""
        order_index: 3
        default_quest_shape: "gear"
        icon: { id: "minecraft:crafting_table" }
        quests: [
            { id: "00000000000000A1", x: 1.5d, y: -2.0d, shape: "", size: 2.0d, dependencies: ["00000000000000A2", "00000000000000FF"],
              tasks: [ { id: "00000000000000B1", type: "item", item: { id: "minecraft:stone", count: 1 }, count: 64L } ],
              rewards: [ { id: "00000000000000D1", type: "item", count: 4, item: { count: 1, id: "minecraft:torch" }, random_bonus: 2 },
                         { id: "00000000000000D2", type: "xp", xp: 100 } ] }
            { id: "00000000000000A2", x: 0.0d, y: 0.0d, shape: "hexagon", dependency_requirement: "one_started",
              tasks: [ { id: "00000000000000B2", type: "checkmark" }, { id: "00000000000000B3", type: "kill", entity: "minecraft:zombie", value: 3L } ] }
        ]
    }"#;

    #[test]
    fn reads_quests_tasks_and_rewards() {
        let book = book("{ default_quest_shape: \"circle\", progression_mode: \"flexible\" }", &[CHAPTER]);
        assert_eq!(book.chapters.len(), 1);
        let chapter = &book.chapters[0];
        assert_eq!(chapter.filename, "first");
        assert_eq!(chapter.quests.len(), 2);
        let q = &chapter.quests[0];
        assert_eq!(q.id, 0xA1);
        assert_eq!((q.x, q.y, q.size), (1.5, -2.0, 2.0));
        assert_eq!(q.tasks[0].max_progress(), 64);
        assert!(matches!(&q.rewards[0].kind, RewardKind::Item { count: 4, random_bonus: 2, .. }));
        assert_eq!(q.rewards[1].type_name(), "xp");
        assert!(!q.rewards[1].is_supported());
        let q2 = &chapter.quests[1];
        assert_eq!(q2.requirement, Requirement::OneStarted);
        assert!(q2.tasks[0].is_supported());
        assert_eq!(q2.tasks[1].type_name(), "kill");
        assert!(!q2.tasks[1].is_supported());
    }

    #[test]
    fn a_dependency_on_nothing_is_dropped_and_dependants_are_indexed() {
        let book = book("{}", &[CHAPTER]);
        assert_eq!(book.quest(0xA1).unwrap().dependencies, vec![0xA2]);
        assert_eq!(book.dependants(0xA2), &[0xA1]);
        assert!(book.dependants(0xA1).is_empty());
    }

    #[test]
    fn lookups_find_objects_by_id() {
        let book = book("{}", &[CHAPTER]);
        assert_eq!(book.task(0xB3).unwrap().1.id, 0xA2);
        assert_eq!(book.reward(0xD2).unwrap().1.id, 0xA1);
        assert_eq!(book.chapter_of(0xA2).unwrap().id, 0xC1);
        assert!(book.task(0xA1).is_none());
        assert!(book.quest(0xB1).is_none());
    }

    #[test]
    fn shape_and_progression_fall_back_in_order() {
        let book = book("{ default_quest_shape: \"circle\", progression_mode: \"flexible\" }", &[CHAPTER]);
        let chapter = &book.chapters[0];
        assert_eq!(book.quest_shape(chapter, &chapter.quests[0]), "gear");
        assert_eq!(book.quest_shape(chapter, &chapter.quests[1]), "hexagon");
        assert_eq!(book.progression_of(chapter, &chapter.quests[0]), Progression::Flexible);
    }

    #[test]
    fn ids_read_in_every_form_the_files_use() {
        assert_eq!(parse_id(&Value::Str("00000000000000FF".into())), Some(255));
        assert_eq!(parse_id(&Value::Str("#FF".into())), Some(255));
        assert_eq!(parse_id(&Value::Long(-1)), Some(u64::MAX));
        assert_eq!(parse_id(&Value::Str("not hex".into())), None);
        assert_eq!(id_string(255), "00000000000000FF");
    }

    #[test]
    fn a_reward_count_of_zero_is_the_items_own() {
        let nbt = snbt::parse_compound(r#"{ id: "1", type: "item", item: { id: "a:b", count: 5 } }"#).unwrap();
        let reward = read_reward(&nbt, &Lang::default());
        assert!(matches!(reward.kind, RewardKind::Item { count: 5, .. }));
    }

    #[test]
    fn items_read_from_a_bare_string_too() {
        let nbt = snbt::parse_compound(r#"{ a: "minecraft:stone", b: "" }"#).unwrap();
        assert_eq!(ItemRef::read(nbt.get("a")).unwrap().id, "minecraft:stone");
        assert!(ItemRef::read(nbt.get("b")).is_none());
        assert!(ItemRef::read(nbt.get("c")).is_none());
    }
}
