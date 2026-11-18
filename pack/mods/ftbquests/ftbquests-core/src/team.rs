//! A team's progress, `<world>/ftbquests/<team uuid>.snbt` in FTB Quests' layout: `version`, `uuid` (undashed),
//! `name`, `lock`, `rewards_blocked`, `task_progress`, `started` and `completed` (quest object id to value or time),
//! `claimed_rewards` (`<player>:<reward>` to time, the nil player for a team reward), and the keys this slice
//! keeps as it read them (`repeatable`, `completion_count`, `player_data`).

use crate::{
    Uuid,
    file::{QuestFile, RewardKind, TaskKind, code, parse_id},
    snbt::{Compound, Value},
};
use std::collections::BTreeMap;

pub fn undashed(id: Uuid) -> String {
    format!("{id:032x}")
}
pub fn dashed(id: Uuid) -> String {
    let s = undashed(id);
    format!("{}-{}-{}-{}-{}", &s[..8], &s[8..12], &s[12..16], &s[16..20], &s[20..])
}
pub fn parse_uuid(text: &str) -> Option<Uuid> {
    let hex: String = text.chars().filter(|c| *c != '-').collect();
    (hex.len() == 32).then(|| u128::from_str_radix(&hex, 16).ok()).flatten()
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TeamData {
    pub uuid: Uuid,
    pub name: String,
    pub lock: bool,
    pub rewards_blocked: bool,
    pub task_progress: BTreeMap<u64, i64>,
    pub started: BTreeMap<u64, i64>,
    pub completed: BTreeMap<u64, i64>,
    pub claimed: BTreeMap<(Uuid, u64), i64>,
    /// `repeatable`, `completion_count`, `player_data`, as read.
    pub kept: Vec<(String, Value)>,
    pub dirty: bool,
}

fn long_map(c: Option<&Compound>) -> BTreeMap<u64, i64> {
    c.map(|c| c.0.iter().filter_map(|(k, v)| Some((parse_id(k)?, v.as_i64()?))).collect()).unwrap_or_default()
}
fn put_map(root: &mut Compound, key: &str, map: &BTreeMap<u64, i64>) {
    root.put(key, Value::Compound(Compound(map.iter().map(|(k, v)| (code(*k), Value::Long(*v))).collect())));
}

impl TeamData {
    pub fn new(uuid: Uuid, name: &str) -> Self {
        TeamData { uuid, name: name.to_owned(), dirty: true, ..TeamData::default() }
    }

    pub fn from_snbt(c: &Compound) -> Option<Self> {
        let uuid = parse_uuid(c.str("uuid")?)?;
        let flag = |k: &str| c.get(k).and_then(Value::as_i64).is_some_and(|v| v != 0);
        let claimed = c
            .compound("claimed_rewards")
            .map(|m| {
                m.0.iter()
                    .filter_map(|(k, v)| {
                        let (player, reward) = k.split_once(':')?;
                        Some(((parse_uuid(player)?, parse_id(reward)?), v.as_i64()?))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let kept = ["repeatable", "completion_count", "player_data"]
            .iter()
            .filter_map(|k| Some((k.to_string(), c.get(k)?.clone())))
            .collect();
        Some(TeamData {
            uuid,
            name: c.str("name").unwrap_or_default().to_owned(),
            lock: flag("lock"),
            rewards_blocked: flag("rewards_blocked"),
            task_progress: long_map(c.compound("task_progress")),
            started: long_map(c.compound("started")),
            completed: long_map(c.compound("completed")),
            claimed,
            kept,
            dirty: false,
        })
    }

    pub fn to_snbt(&self) -> Compound {
        let mut root = Compound::default();
        root.put("version", Value::Int(1));
        root.put("uuid", Value::String(undashed(self.uuid)));
        root.put("name", Value::String(self.name.clone()));
        root.put("lock", Value::Bool(self.lock));
        root.put("rewards_blocked", Value::Bool(self.rewards_blocked));
        put_map(&mut root, "task_progress", &self.task_progress);
        put_map(&mut root, "started", &self.started);
        put_map(&mut root, "completed", &self.completed);
        for key in ["repeatable", "completion_count"] {
            root.put(key, self.kept.iter().find(|(k, _)| k == key).map_or(Value::Compound(Compound::default()), |(_, v)| v.clone()));
        }
        let claimed = self.claimed.iter().map(|((p, r), t)| (format!("{}:{}", undashed(*p), code(*r)), Value::Long(*t))).collect();
        root.put("claimed_rewards", Value::Compound(Compound(claimed)));
        root.put("player_data", self.kept.iter().find(|(k, _)| k == "player_data").map_or(Value::Compound(Compound::default()), |(_, v)| v.clone()));
        root
    }

    pub fn progress(&self, task: u64) -> i64 {
        self.task_progress.get(&task).copied().unwrap_or(0)
    }
    pub fn is_completed(&self, id: u64) -> bool {
        self.completed.contains_key(&id)
    }

    /// Whether all of the quest's dependencies are complete (the ones that aren't quests of the file count as done).
    pub fn dependencies_complete(&self, file: &QuestFile, quest: u64) -> bool {
        file.quests.get(&quest).is_some_and(|q| q.dependencies.iter().all(|d| !file.quests.contains_key(d) || self.is_completed(*d)))
    }

    /// Whether the quest's tasks may progress. ATM10's book is `flexible`: they always may, and the quest completes
    /// once its dependencies are complete too.
    pub fn can_start_tasks(&self, file: &QuestFile, quest: u64) -> bool {
        file.quests.contains_key(&quest) && !self.lock
    }

    /// Sets a task's progress (clamped to its maximum); true when anything changed. A task that completes marks
    /// its quest, chapter and file done where they now are.
    pub fn set_progress(&mut self, file: &QuestFile, task: u64, progress: i64, now: i64) -> bool {
        let Some(t) = file.tasks.get(&task) else { return false };
        if self.is_completed(task) || !self.can_start_tasks(file, t.quest) {
            return false;
        }
        let progress = progress.clamp(0, t.max_progress());
        if progress <= self.progress(task) {
            return false;
        }
        self.task_progress.insert(task, progress);
        self.started.entry(t.quest).or_insert(now);
        if progress >= t.max_progress() {
            self.completed.insert(task, now);
            self.check_quest(file, t.quest, now);
        }
        self.dirty = true;
        true
    }

    /// Completes the quest when its required tasks and dependencies are, then the quests that wait on it.
    pub fn check_quest(&mut self, file: &QuestFile, quest: u64, now: i64) {
        let mut queue = vec![quest];
        while let Some(id) = queue.pop() {
            let Some(q) = file.quests.get(&id) else { continue };
            if self.is_completed(id) || !self.dependencies_complete(file, id) {
                continue;
            }
            let required: Vec<u64> = q.tasks.iter().copied().filter(|t| file.tasks.get(t).is_some_and(|t| !t.optional)).collect();
            if required.is_empty() || !required.iter().all(|t| self.is_completed(*t)) {
                continue;
            }
            self.completed.insert(id, now);
            self.dirty = true;
            if let Some(ch) = file.chapter(q.chapter) {
                let done = ch.quests.iter().all(|x| file.quests.get(x).is_some_and(|x| x.optional || x.tasks.is_empty()) || self.is_completed(*x));
                if done && !self.is_completed(ch.id) {
                    self.completed.insert(ch.id, now);
                }
            }
            queue.extend(file.dependents(id));
        }
    }

    /// The item task progress a look at the inventory gives: `carried` is (item id, count).
    pub fn detect_items(&mut self, file: &QuestFile, carried: &[(String, u32)], now: i64) -> bool {
        let mut changed = false;
        for t in file.tasks.values() {
            if let TaskKind::Item { item, .. } = &t.kind {
                if self.is_completed(t.id) {
                    continue;
                }
                let n: i64 = carried.iter().filter(|(i, _)| i == item).map(|(_, n)| *n as i64).sum();
                if n > 0 {
                    changed |= self.set_progress(file, t.id, n, now);
                }
            }
        }
        changed
    }

    /// Ticks a checkmark task (the quest view's button).
    pub fn check(&mut self, file: &QuestFile, task: u64, now: i64) -> bool {
        matches!(file.tasks.get(&task).map(|t| &t.kind), Some(TaskKind::Checkmark)) && self.set_progress(file, task, 1, now)
    }

    /// Whether `player` has the reward (the nil player's claim for a team reward).
    pub fn is_claimed(&self, file: &QuestFile, player: Uuid, reward: u64) -> bool {
        let team = file.rewards.get(&reward).is_some_and(|r| file.team_reward(r));
        self.claimed.contains_key(&(if team { 0 } else { player }, reward))
    }

    /// Claims a reward for `player`: its quest must be complete, the reward not yet theirs. The item to give, or
    /// why not.
    pub fn claim(&mut self, file: &QuestFile, player: Uuid, reward: u64, now: i64) -> Result<Option<(String, u32)>, &'static str> {
        let r = file.rewards.get(&reward).ok_or("unknown reward")?;
        if self.rewards_blocked {
            return Err("rewards blocked");
        }
        if !self.is_completed(r.quest) {
            return Err("quest not complete");
        }
        if self.is_claimed(file, player, reward) {
            return Err("already claimed");
        }
        let item = match &r.kind {
            RewardKind::Item { item, count, .. } => Some((item.clone(), *count)),
            RewardKind::Other(_) => return Err("reward type not ported yet"),
        };
        self.claimed.insert((if file.team_reward(r) { 0 } else { player }, reward), now);
        self.dirty = true;
        Ok(item)
    }
}
