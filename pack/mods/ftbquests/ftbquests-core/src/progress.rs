//! A player's progress and FTB Quests' rules over it (`TeamData`, `Quest`,
//! `Task`): tasks count up, quests complete when their tasks and dependencies
//! are done, rewards are claimed once. FTB Quests keeps this per team; this port
//! keeps it per player, in the same fields and the same file shape
//! (`<world>/ftbquests/<uuid>.snbt`).

use crate::book::{Book, Chapter, Id, Place, Progression, Quest, Task, TaskKind, id_string, parse_hex};
use crate::snbt::{self, Compound, Value};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Progress {
    /// Task id to count.
    pub tasks: BTreeMap<Id, u64>,
    /// Object id (task, quest, chapter) to the time it started, ms since 1970.
    pub started: BTreeMap<Id, u64>,
    /// Object id to the time it completed.
    pub completed: BTreeMap<Id, u64>,
    /// Reward id to the time it was claimed.
    pub claimed: BTreeMap<Id, u64>,
}

/// What `Progress::visible_in` has worked out for a progress: a whole book asks
/// about thousands of quests a frame, each with its dependencies behind it.
#[derive(Clone, Debug, Default)]
pub struct VisibleMemo(HashMap<Id, Option<bool>>);

impl VisibleMemo {
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

/// One thing that changed, to send to the player's client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Task(Id, u64),
    Started(Id, u64),
    Completed(Id, u64),
    Claimed(Id, u64),
}

impl Progress {
    pub fn task_count(&self, id: Id) -> u64 {
        self.tasks.get(&id).copied().unwrap_or(0)
    }
    pub fn is_started(&self, id: Id) -> bool {
        self.started.contains_key(&id)
    }
    pub fn is_completed(&self, id: Id) -> bool {
        self.completed.contains_key(&id)
    }
    pub fn is_claimed(&self, reward: Id) -> bool {
        self.claimed.contains_key(&reward)
    }
    pub fn apply(&mut self, change: Change) {
        match change {
            Change::Task(id, n) => {
                self.tasks.insert(id, n);
            }
            Change::Started(id, t) => {
                self.started.insert(id, t);
            }
            Change::Completed(id, t) => {
                self.completed.insert(id, t);
            }
            Change::Claimed(id, t) => {
                self.claimed.insert(id, t);
            }
        }
    }
    /// Every entry, to bring a client up to date.
    pub fn entries(&self) -> Vec<Change> {
        let mut out = Vec::with_capacity(self.tasks.len() + self.started.len() + self.completed.len() + self.claimed.len());
        out.extend(self.tasks.iter().map(|(&i, &v)| Change::Task(i, v)));
        out.extend(self.started.iter().map(|(&i, &v)| Change::Started(i, v)));
        out.extend(self.completed.iter().map(|(&i, &v)| Change::Completed(i, v)));
        out.extend(self.claimed.iter().map(|(&i, &v)| Change::Claimed(i, v)));
        out
    }

    /// `Quest.checkDependencies` under `Quest.areDependenciesComplete`.
    pub fn deps_complete(&self, quest: &Quest) -> bool {
        let need_completion = quest.requirement.needs_completion();
        check_deps(quest, |dep| if need_completion { self.is_completed(dep) } else { self.is_started(dep) })
    }
    /// `TeamData.canStartTasks`: a flexible quest can count up before its
    /// dependencies are done; it completes when they are.
    pub fn can_start_tasks(&self, book: &Book, chapter: &Chapter, quest: &Quest) -> bool {
        book.progression_of(chapter, quest) == Progression::Flexible || self.deps_complete(quest)
    }
    /// `Quest.isVisible`.
    pub fn is_visible(&self, book: &Book, chapter: &Chapter, quest: &Quest) -> bool {
        self.visible_in(book, chapter, quest, &mut VisibleMemo::default())
    }
    /// `is_visible`, keeping what it works out. Clear the memo when the progress changes.
    pub fn visible_in(&self, book: &Book, chapter: &Chapter, quest: &Quest, memo: &mut VisibleMemo) -> bool {
        match memo.0.get(&quest.id) {
            Some(Some(known)) => return *known,
            // A cycle in the files must not loop: a quest being asked about counts as not shown.
            Some(None) => return false,
            None => {}
        }
        memo.0.insert(quest.id, None);
        let shown = self.work_out_visible(book, chapter, quest, memo);
        memo.0.insert(quest.id, Some(shown));
        shown
    }
    fn work_out_visible(&self, book: &Book, chapter: &Chapter, quest: &Quest, memo: &mut VisibleMemo) -> bool {
        if quest.invisible && !self.is_completed(quest.id) {
            let until = quest.invisible_until_tasks;
            let done = quest.tasks.iter().filter(|t| self.is_completed(t.id)).count() as i64;
            if until == 0 || done < until {
                return false;
            }
        }
        if quest.dependencies.is_empty() {
            true
        } else if quest.hide_until_deps_complete.get(chapter.hide_quest_until_deps_complete) {
            self.deps_complete(quest)
        } else if quest.hide_until_deps_visible.get(chapter.hide_quest_until_deps_visible) {
            check_deps(quest, |dep| self.object_visible(book, dep, memo))
        } else {
            quest.dependencies.iter().any(|&dep| self.object_visible(book, dep, memo))
        }
    }
    fn object_visible(&self, book: &Book, id: Id, memo: &mut VisibleMemo) -> bool {
        match book.place(id) {
            Some(Place::Quest(c, q)) => {
                let chapter = &book.chapters[c];
                self.visible_in(book, chapter, &chapter.quests[q], memo)
            }
            Some(_) => true,
            None => false,
        }
    }
    /// `Chapter.isVisible`.
    pub fn chapter_visible(&self, book: &Book, chapter: &Chapter) -> bool {
        self.chapter_visible_in(book, chapter, &mut VisibleMemo::default())
    }
    pub fn chapter_visible_in(&self, book: &Book, chapter: &Chapter, memo: &mut VisibleMemo) -> bool {
        !chapter.always_invisible && chapter.quests.iter().any(|q| self.visible_in(book, chapter, q, memo))
    }
    /// `Task.checkTaskSequence`: with sequential tasks only the first undone one counts up.
    pub fn sequence_allows(&self, quest: &Quest, chapter: &Chapter, task: Id) -> bool {
        if !quest.require_sequential_tasks.get(chapter.require_sequential_tasks) {
            return true;
        }
        for t in &quest.tasks {
            if t.id == task {
                return true;
            }
            if !self.is_completed(t.id) {
                return false;
            }
        }
        true
    }
    /// A quest's percentage (`Quest.getRelativeProgressFromChildren`).
    pub fn relative(&self, quest: &Quest) -> u32 {
        if self.is_completed(quest.id) {
            return 100;
        }
        if quest.tasks.is_empty() {
            return if self.deps_complete(quest) { 100 } else { 0 };
        }
        let total: u64 = quest.tasks.iter().map(|t| self.task_count(t.id).min(t.max_progress()) * 100 / t.max_progress()).sum();
        if total > 0 && !self.deps_complete(quest) {
            return 0;
        }
        (total / quest.tasks.len() as u64) as u32
    }
    /// Rewards done and waiting: `Quest.hasUnclaimedRewardsRaw` for the kinds that can be claimed here.
    pub fn has_unclaimed(&self, quest: &Quest) -> bool {
        self.is_completed(quest.id) && quest.rewards.iter().any(|r| r.is_supported() && !self.is_claimed(r.id))
    }
    /// A chapter's percentage, optional quests aside (`QuestObject.getRelativeProgress` over quests).
    pub fn chapter_relative(&self, chapter: &Chapter) -> u32 {
        let (mut total, mut count) = (0u32, 0u32);
        for quest in chapter.quests.iter().filter(|q| !q.optional) {
            total += self.relative(quest);
            count += 1;
        }
        if count == 0 { 0 } else { total / count }
    }

    /// `TeamData.setProgress`: clamps to the task's maximum, starts what wasn't
    /// started and, with the dependencies done, completes the task.
    pub fn set_task(&mut self, book: &Book, task: Id, value: u64, now: u64, changes: &mut Vec<Change>) {
        let Some((found, _, _)) = book.task(task) else { return };
        let value = value.min(found.max_progress());
        let before = self.task_count(task);
        if value == before {
            return;
        }
        self.record(Change::Task(task, value), changes);
        if before == 0 {
            self.start_task(book, task, now, changes);
        }
        let (found, quest, _) = book.task(task).expect("found above");
        if value >= found.max_progress() && self.deps_complete(quest) {
            self.complete_task(book, task, now, changes);
        }
    }
    /// A checkmark's press (`CheckmarkTask` through `Task.submitTask`).
    pub fn submit_checkmark(&mut self, book: &Book, task: Id, now: u64, changes: &mut Vec<Change>) -> bool {
        let Some((found, quest, chapter)) = book.task(task) else { return false };
        if !matches!(found.kind, TaskKind::Checkmark)
            || self.is_completed(task)
            || !self.can_start_tasks(book, chapter, quest)
            || !self.sequence_allows(quest, chapter, task)
        {
            return false;
        }
        self.set_task(book, task, 1, now, changes);
        true
    }
    fn record(&mut self, change: Change, changes: &mut Vec<Change>) {
        self.apply(change);
        changes.push(change);
    }
    fn start_task(&mut self, book: &Book, task: Id, now: u64, changes: &mut Vec<Change>) {
        let Some((_, quest, chapter)) = book.task(task) else { return };
        if !self.is_started(task) {
            self.record(Change::Started(task, now), changes);
        }
        if !self.is_started(quest.id) {
            self.record(Change::Started(quest.id, now), changes);
        }
        if !self.is_started(chapter.id) {
            self.record(Change::Started(chapter.id, now), changes);
        }
    }
    /// `TeamData.markTaskCompleted`, then `Task.onCompleted`.
    fn complete_task(&mut self, book: &Book, task: Id, now: u64, changes: &mut Vec<Change>) {
        let Some((_, quest, _)) = book.task(task) else { return };
        if !self.is_completed(task) {
            self.record(Change::Completed(task, now), changes);
        }
        if self.quest_tasks_done(quest) {
            self.complete_quest(book, quest.id, now, changes);
        }
    }
    /// `QuestObject.isCompletedRaw` over a quest's tasks.
    fn quest_tasks_done(&self, quest: &Quest) -> bool {
        let (mut optional, mut done) = (0, 0);
        for task in &quest.tasks {
            if self.is_completed(task.id) {
                done += 1;
            } else if task.optional {
                optional += 1;
            } else {
                return false;
            }
        }
        // No tasks: done. All optional: at least one must be.
        quest.tasks.is_empty() || optional < quest.tasks.len() || done > 0
    }
    /// `Quest.onCompleted`: the chapter next, then the quests that wait on this one.
    fn complete_quest(&mut self, book: &Book, quest: Id, now: u64, changes: &mut Vec<Change>) {
        if self.is_completed(quest) {
            return;
        }
        self.record(Change::Completed(quest, now), changes);
        let Some(chapter) = book.chapter_of(quest) else { return };
        if !self.is_completed(chapter.id) && self.chapter_done(chapter) {
            self.record(Change::Completed(chapter.id, now), changes);
        }
        // `Quest.checkForDependantCompletion`: a flexible quest that counted up
        // earlier completes now; so does a quest that was only waiting on this one.
        for &dep in book.dependants(quest) {
            let Some(Place::Quest(c, q)) = book.place(dep) else { continue };
            let chapter = &book.chapters[c];
            let dependant = &chapter.quests[q];
            if !self.deps_complete(dependant) {
                continue;
            }
            for task in &dependant.tasks {
                if self.task_count(task.id) >= task.max_progress() && !self.is_completed(task.id) {
                    self.complete_task(book, task.id, now, changes);
                }
            }
        }
    }
    /// `QuestObject.isCompletedRaw` over a chapter's quests.
    fn chapter_done(&self, chapter: &Chapter) -> bool {
        let (mut optional, mut done) = (0, 0);
        for quest in &chapter.quests {
            if self.is_completed(quest.id) {
                done += 1;
            } else if quest.optional {
                optional += 1;
            } else {
                return false;
            }
        }
        chapter.quests.is_empty() || optional < chapter.quests.len() || done > 0
    }
    /// `TeamData.isRewardReadyToClaim` for a reward this port can give: its
    /// quest is done and it wasn't claimed.
    pub fn can_claim(&self, book: &Book, reward: Id) -> bool {
        let Some((found, quest)) = book.reward(reward) else { return false };
        found.is_supported() && self.is_completed(quest.id) && !self.is_claimed(reward)
    }
    /// `TeamData.markRewardAsClaimed`.
    pub fn mark_claimed(&mut self, reward: Id, now: u64, changes: &mut Vec<Change>) {
        if !self.is_claimed(reward) {
            self.record(Change::Claimed(reward, now), changes);
        }
    }

    /// The file: FTB Quests' team data fields, for one player.
    pub fn to_snbt(&self, uuid: &str, name: &str) -> String {
        fn map(out: &mut String, key: &str, entries: &BTreeMap<Id, u64>) {
            if entries.is_empty() {
                out.push_str(&format!("\t{key}: {{ }}\n"));
                return;
            }
            out.push_str(&format!("\t{key}: {{\n"));
            for (id, value) in entries {
                out.push_str(&format!("\t\t\"{}\": {value}L\n", id_string(*id)));
            }
            out.push_str("\t}\n");
        }
        let mut out = String::from("{\n\tversion: 1\n");
        out.push_str(&format!("\tuuid: {}\n", quote(uuid)));
        out.push_str(&format!("\tname: {}\n", quote(name)));
        map(&mut out, "task_progress", &self.tasks);
        map(&mut out, "started", &self.started);
        map(&mut out, "completed", &self.completed);
        map(&mut out, "claimed_rewards", &self.claimed);
        out.push_str("}\n");
        out
    }
    pub fn from_snbt(text: &str) -> Result<Self, snbt::Error> {
        let root = snbt::parse_compound(text)?;
        let map = |key: &str| read_map(root.compound(key));
        Ok(Progress { tasks: map("task_progress"), started: map("started"), completed: map("completed"), claimed: map("claimed_rewards") })
    }
}

fn read_map(found: Option<&Compound>) -> BTreeMap<Id, u64> {
    let Some(found) = found else { return BTreeMap::new() };
    found
        .0
        .iter()
        .filter_map(|(k, v)| Some((parse_hex(k)?, Value::as_i64(v)? as u64)))
        .collect()
}

fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `Quest.checkDependencies`.
fn check_deps(quest: &Quest, mut check: impl FnMut(Id) -> bool) -> bool {
    if quest.min_required_dependencies > 0 {
        let needed = quest.min_required_dependencies as usize;
        return quest.dependencies.iter().filter(|&&d| check(d)).take(needed).count() == needed;
    }
    if quest.requirement.needs_only_one() {
        quest.dependencies.iter().any(|&d| check(d))
    } else {
        quest.dependencies.iter().all(|&d| check(d))
    }
}

/// How an item task stands against what the player carries: the new count, if
/// it rose (`ItemTask.submitTask` for a task that doesn't consume).
pub fn detected(task: &Task, progress: &Progress, carried: u64) -> Option<u64> {
    let TaskKind::Item { count, .. } = &task.kind else { return None };
    let found = carried.min(*count);
    (found > progress.task_count(task.id)).then_some(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::fixture::book;

    const CHAPTER: &str = r#"{
        id: "00000000000000C1", group: "", filename: "c"
        quests: [
            { id: "0000000000000001", tasks: [ { id: "0000000000000011", type: "item", item: { id: "minecraft:stone", count: 1 }, count: 4L } ],
              rewards: [ { id: "0000000000000021", type: "item", item: { id: "minecraft:torch", count: 2 } }, { id: "0000000000000022", type: "xp", xp: 10 } ] }
            { id: "0000000000000002", dependencies: ["0000000000000001"],
              tasks: [ { id: "0000000000000012", type: "checkmark" } ] }
            { id: "0000000000000003", dependencies: ["0000000000000002"], hide_until_deps_complete: true,
              tasks: [ { id: "0000000000000013", type: "checkmark" }, { id: "0000000000000014", type: "kill", entity: "minecraft:zombie", value: 1L } ] }
            { id: "0000000000000004", optional: true, invisible: true,
              tasks: [ { id: "0000000000000015", type: "checkmark" } ] }
        ]
    }"#;

    fn linear() -> Book {
        book("{ progression_mode: \"linear\" }", &[CHAPTER])
    }

    #[test]
    fn an_item_task_counts_up_and_completes_its_quest_and_unlocks_the_next() {
        let book = linear();
        let mut p = Progress::default();
        let mut c = Vec::new();
        let (second, chapter) = (book.quest(2).unwrap(), &book.chapters[0]);
        assert!(!p.can_start_tasks(&book, chapter, second));

        p.set_task(&book, 0x11, 3, 100, &mut c);
        assert_eq!(p.task_count(0x11), 3);
        assert!(p.is_started(0x11) && p.is_started(1) && p.is_started(0xC1));
        assert!(!p.is_completed(0x11));

        p.set_task(&book, 0x11, 9, 200, &mut c);
        assert_eq!(p.task_count(0x11), 4, "clamped to the task's count");
        assert!(p.is_completed(0x11) && p.is_completed(1));
        assert!(p.can_start_tasks(&book, chapter, second));
        assert!(c.contains(&Change::Completed(1, 200)));
    }

    #[test]
    fn a_checkmark_waits_for_its_dependencies_in_linear_mode() {
        let book = linear();
        let mut p = Progress::default();
        let mut c = Vec::new();
        assert!(!p.submit_checkmark(&book, 0x12, 1, &mut c));
        assert!(!p.is_completed(2));
        p.set_task(&book, 0x11, 4, 2, &mut c);
        assert!(p.submit_checkmark(&book, 0x12, 3, &mut c));
        assert!(p.is_completed(0x12) && p.is_completed(2));
        assert!(!p.submit_checkmark(&book, 0x12, 4, &mut c), "only once");
        assert!(!p.submit_checkmark(&book, 0x11, 4, &mut c), "not an item task");
    }

    #[test]
    fn flexible_quests_count_early_and_complete_when_the_dependency_does() {
        let book = book("{ progression_mode: \"flexible\" }", &[CHAPTER]);
        let mut p = Progress::default();
        let mut c = Vec::new();
        assert!(p.submit_checkmark(&book, 0x12, 1, &mut c), "flexible: tasks count before the dependency");
        assert_eq!(p.task_count(0x12), 1);
        assert!(!p.is_completed(0x12));
        assert!(!p.is_completed(2));
        p.set_task(&book, 0x11, 4, 2, &mut c);
        assert!(p.is_completed(1));
        assert!(p.is_completed(0x12), "completes once the dependency does");
        assert!(p.is_completed(2));
    }

    #[test]
    fn a_quest_with_an_unsupported_task_stays_open_and_the_chapter_too() {
        let book = linear();
        let mut p = Progress::default();
        let mut c = Vec::new();
        p.set_task(&book, 0x11, 4, 1, &mut c);
        p.submit_checkmark(&book, 0x12, 2, &mut c);
        assert!(p.submit_checkmark(&book, 0x13, 3, &mut c));
        assert!(p.is_completed(0x13));
        assert!(!p.is_completed(3), "the kill task is still open");
        assert_eq!(p.relative(book.quest(3).unwrap()), 50);
        assert!(!p.is_completed(0xC1));
        p.set_task(&book, 0x14, 1, 4, &mut c);
        assert!(p.is_completed(3));
        assert!(p.is_completed(0xC1), "the optional quest doesn't hold the chapter");
    }

    #[test]
    fn rewards_are_claimed_once_and_only_when_the_quest_is_done() {
        let book = linear();
        let mut p = Progress::default();
        let mut c = Vec::new();
        assert!(!p.can_claim(&book, 0x21));
        p.set_task(&book, 0x11, 4, 1, &mut c);
        assert!(p.can_claim(&book, 0x21));
        assert!(p.has_unclaimed(book.quest(1).unwrap()));
        assert!(!p.can_claim(&book, 0x22), "an xp reward can't be claimed here yet");
        p.mark_claimed(0x21, 5, &mut c);
        assert!(!p.can_claim(&book, 0x21));
        assert!(!p.has_unclaimed(book.quest(1).unwrap()));
    }

    #[test]
    fn visibility_follows_invisible_and_hide_until() {
        let book = linear();
        let mut p = Progress::default();
        let chapter = &book.chapters[0];
        let mut c = Vec::new();
        assert!(p.is_visible(&book, chapter, book.quest(1).unwrap()));
        assert!(p.is_visible(&book, chapter, book.quest(2).unwrap()), "a shown dependency shows it");
        assert!(!p.is_visible(&book, chapter, book.quest(3).unwrap()), "hide_until_deps_complete");
        assert!(!p.is_visible(&book, chapter, book.quest(4).unwrap()), "invisible until done");
        p.set_task(&book, 0x11, 4, 1, &mut c);
        p.submit_checkmark(&book, 0x12, 2, &mut c);
        assert!(p.is_visible(&book, chapter, book.quest(3).unwrap()));
    }

    #[test]
    fn requirements_and_a_minimum_of_dependencies() {
        let text = r#"{ id: "00000000000000C2", group: "", quests: [
            { id: "0000000000000031", tasks: [ { id: "0000000000000041", type: "checkmark" } ] }
            { id: "0000000000000032", tasks: [ { id: "0000000000000042", type: "checkmark" } ] }
            { id: "0000000000000033", dependency_requirement: "one_completed", dependencies: ["0000000000000031", "0000000000000032"], tasks: [ { id: "0000000000000043", type: "checkmark" } ] }
            { id: "0000000000000034", min_required_dependencies: 2, dependency_requirement: "one_completed", dependencies: ["0000000000000031", "0000000000000032"], tasks: [ { id: "0000000000000044", type: "checkmark" } ] }
            { id: "0000000000000035", dependency_requirement: "all_started", dependencies: ["0000000000000031"], tasks: [ { id: "0000000000000045", type: "checkmark" } ] }
        ] }"#;
        let book = book("{ progression_mode: \"linear\" }", &[text]);
        let mut p = Progress::default();
        let mut c = Vec::new();
        assert!(!p.deps_complete(book.quest(0x33).unwrap()));
        p.submit_checkmark(&book, 0x41, 1, &mut c);
        assert!(p.deps_complete(book.quest(0x33).unwrap()), "one is enough");
        assert!(!p.deps_complete(book.quest(0x34).unwrap()), "needs two");
        assert!(p.deps_complete(book.quest(0x35).unwrap()), "started counts");
        p.submit_checkmark(&book, 0x42, 1, &mut c);
        assert!(p.deps_complete(book.quest(0x34).unwrap()));
    }

    #[test]
    fn a_dependency_cycle_does_not_hang() {
        let text = r#"{ id: "00000000000000C3", group: "", quests: [
            { id: "0000000000000051", dependencies: ["0000000000000052"], tasks: [ { id: "0000000000000061", type: "checkmark" } ] }
            { id: "0000000000000052", dependencies: ["0000000000000051"], tasks: [ { id: "0000000000000062", type: "checkmark" } ] }
        ] }"#;
        let book = book("{}", &[text]);
        let p = Progress::default();
        assert!(!p.is_visible(&book, &book.chapters[0], book.quest(0x51).unwrap()));
    }

    #[test]
    fn sequential_tasks_go_in_order() {
        let text = r#"{ id: "00000000000000C4", group: "", quests: [
            { id: "0000000000000071", require_sequential_tasks: true, tasks: [ { id: "0000000000000081", type: "checkmark" }, { id: "0000000000000082", type: "checkmark" } ] }
        ] }"#;
        let book = book("{}", &[text]);
        let mut p = Progress::default();
        let mut c = Vec::new();
        assert!(!p.submit_checkmark(&book, 0x82, 1, &mut c));
        assert!(p.submit_checkmark(&book, 0x81, 1, &mut c));
        assert!(p.submit_checkmark(&book, 0x82, 2, &mut c));
        assert!(p.is_completed(0x71));
    }

    #[test]
    fn detection_only_ever_raises_a_task() {
        let book = linear();
        let (task, _, _) = book.task(0x11).unwrap();
        let mut p = Progress::default();
        assert_eq!(detected(task, &p, 2), Some(2));
        p.tasks.insert(0x11, 2);
        assert_eq!(detected(task, &p, 1), None, "putting items down keeps the count");
        assert_eq!(detected(task, &p, 2), None);
        assert_eq!(detected(task, &p, 99), Some(4));
    }

    #[test]
    fn progress_survives_its_file() {
        let book = linear();
        let mut p = Progress::default();
        let mut c = Vec::new();
        p.set_task(&book, 0x11, 4, 1_700_000_000_000, &mut c);
        p.mark_claimed(0x21, 1_700_000_000_001, &mut c);
        let text = p.to_snbt("0123456789abcdef0123456789abcdef", "A \"quoted\" name");
        let root = snbt::parse_compound(&text).unwrap();
        assert_eq!(root.str("name"), Some("A \"quoted\" name"));
        assert_eq!(root.compound("task_progress").unwrap().get("0000000000000011"), Some(&Value::Long(4)));
        assert_eq!(Progress::from_snbt(&text).unwrap(), p);
    }

    #[test]
    fn an_empty_progress_survives_its_file_too() {
        let text = Progress::default().to_snbt("u", "n");
        assert_eq!(Progress::from_snbt(&text).unwrap(), Progress::default());
    }

    #[test]
    fn changes_rebuild_the_same_progress_on_a_client() {
        let book = linear();
        let mut p = Progress::default();
        let mut c = Vec::new();
        p.set_task(&book, 0x11, 4, 1, &mut c);
        p.mark_claimed(0x21, 2, &mut c);
        let mut client = Progress::default();
        c.iter().for_each(|&x| client.apply(x));
        assert_eq!(client, p);
        let mut fresh = Progress::default();
        p.entries().into_iter().for_each(|x| fresh.apply(x));
        assert_eq!(fresh, p);
    }
}
