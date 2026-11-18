//! A team's progress, `<world>/ftbquests/<team uuid>.snbt` in FTB Quests' layout: `version`, `uuid` (undashed),
//! `name`, `lock`, `rewards_blocked`, `task_progress`, `started` and `completed` (quest object id to value or time),
//! `claimed_rewards` (`<player>:<reward>` to time, the nil player for a team reward), and the keys this slice
//! keeps as it read them (`repeatable`, `completion_count`, `player_data`).

use crate::{
    Uuid,
    file::{AutoClaim, QuestFile, RewardKind, TaskKind, code, parse_id},
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
    /// The tasks, quests and chapters completed since the server last looked (for the toasts and the auto-claims;
    /// not saved).
    pub newly: Vec<u64>,
}

/// What a claim hands the player, for the server to give.
#[derive(Clone, Debug, PartialEq)]
pub enum Grant {
    Item { item: String, count: u32 },
    Xp(u32),
    Levels(i32),
    /// `command` as written (placeholders not yet filled), run at `level`.
    Command { command: String, level: u8, silent: bool, feedback: Option<String> },
    Toast { reward: u64, description: String },
}

/// The rewards of `quest` that are claimed when it completes, and whether each shows its toast. A choice waits for
/// the player's pick.
pub fn auto_claims(file: &QuestFile, quest: u64) -> Vec<(u64, bool)> {
    let Some(q) = file.quests.get(&quest) else { return Vec::new() };
    q.rewards
        .iter()
        .filter_map(|r| file.rewards.get(r))
        .filter(|r| !matches!(r.kind, RewardKind::Choice { .. }))
        .filter_map(|r| match file.auto_claim(r) {
            AutoClaim::Enabled => Some((r.id, true)),
            AutoClaim::NoToast | AutoClaim::Invisible => Some((r.id, false)),
            AutoClaim::Default | AutoClaim::Disabled => None,
        })
        .collect()
}

/// The grants of a reward kind; a table's entries are rewards of their own (one level deep).
fn grants(file: &QuestFile, kind: &RewardKind, reward: u64, choice: Option<usize>, roll: &mut dyn FnMut() -> f64, depth: u8, out: &mut Vec<Grant>) -> Result<(), &'static str> {
    let table = |t: &u64| file.reward_tables.get(t).ok_or("no such reward table");
    match kind {
        RewardKind::Item { item, count, random_bonus, .. } => {
            let bonus = if *random_bonus > 0 { ((roll() * (*random_bonus as f64 + 1.0)) as u32).min(*random_bonus) } else { 0 };
            out.push(Grant::Item { item: item.clone(), count: count.saturating_add(bonus) });
        }
        RewardKind::Xp { points } => out.push(Grant::Xp(*points)),
        RewardKind::XpLevels { levels } => out.push(Grant::Levels(*levels)),
        RewardKind::Command { command, permission_level, silent, feedback } => {
            out.push(Grant::Command { command: command.clone(), level: *permission_level, silent: *silent, feedback: feedback.clone() })
        }
        RewardKind::Toast { description } => out.push(Grant::Toast { reward, description: description.clone() }),
        RewardKind::Random { table: t } | RewardKind::Loot { table: t } if depth == 0 => {
            let t = table(t)?;
            let draws = if matches!(kind, RewardKind::Loot { .. }) { t.loot_size } else { 1 };
            for _ in 0..draws {
                if let Some(i) = t.draw(roll()) {
                    grants(file, &t.entries[i].kind, t.entries[i].id, None, roll, depth + 1, out)?;
                }
            }
        }
        RewardKind::Choice { table: t } if depth == 0 => {
            let t = table(t)?;
            let e = choice.and_then(|i| t.entries.get(i)).ok_or("pick one of the table's rewards")?;
            grants(file, &e.kind, e.id, None, roll, depth + 1, out)?;
        }
        _ => return Err("reward type not ported yet"),
    }
    Ok(())
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
            newly: Vec::new(),
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
            self.newly.push(task);
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
            self.complete_quest(file, id, now);
            queue.extend(file.dependents(id));
        }
    }

    fn complete_quest(&mut self, file: &QuestFile, id: u64, now: i64) {
        self.completed.insert(id, now);
        self.newly.push(id);
        self.dirty = true;
        let Some(ch) = file.quests.get(&id).and_then(|q| file.chapter(q.chapter)) else { return };
        let done = ch.quests.iter().all(|x| file.quests.get(x).is_some_and(|x| x.optional || x.tasks.is_empty()) || self.is_completed(*x));
        if done && !self.is_completed(ch.id) {
            self.completed.insert(ch.id, now);
            self.newly.push(ch.id);
        }
    }

    /// Completes the quest and its tasks whatever its dependencies (what an operator's `change_progress complete`
    /// does), then the quests that wait on it.
    pub fn force_complete(&mut self, file: &QuestFile, quest: u64, now: i64) -> bool {
        let Some(q) = file.quests.get(&quest) else { return false };
        if self.is_completed(quest) {
            return false;
        }
        for t in q.tasks.iter().filter_map(|t| file.tasks.get(t)) {
            if !self.is_completed(t.id) {
                self.task_progress.insert(t.id, t.max_progress());
                self.completed.insert(t.id, now);
                self.newly.push(t.id);
            }
        }
        self.started.entry(quest).or_insert(now);
        self.complete_quest(file, quest, now);
        for d in file.dependents(quest) {
            self.check_quest(file, d, now);
        }
        true
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

    /// Whether `player` may claim the reward now, or why not.
    pub fn claimable(&self, file: &QuestFile, player: Uuid, reward: u64) -> Result<(), &'static str> {
        let r = file.rewards.get(&reward).ok_or("unknown reward")?;
        if self.rewards_blocked && !r.ignore_blocking {
            return Err("rewards blocked");
        }
        if !self.is_completed(r.quest) {
            return Err("quest not complete");
        }
        if self.is_claimed(file, player, reward) {
            return Err("already claimed");
        }
        if matches!(r.kind, RewardKind::Other(_)) {
            return Err("reward type not ported yet");
        }
        Ok(())
    }

    /// Claims a reward for `player` (`choice`: the table entry a choice reward takes; `roll`: uniform draws in
    /// [0, 1) for the tables and random bonuses). What to give, or why not; nothing changes on an error.
    pub fn claim(&mut self, file: &QuestFile, player: Uuid, reward: u64, choice: Option<usize>, now: i64, roll: &mut dyn FnMut() -> f64) -> Result<Vec<Grant>, &'static str> {
        self.claimable(file, player, reward)?;
        let r = &file.rewards[&reward];
        let mut out = Vec::new();
        grants(file, &r.kind, reward, choice, roll, 0, &mut out)?;
        self.claimed.insert((if file.team_reward(r) { 0 } else { player }, reward), now);
        self.dirty = true;
        Ok(out)
    }

    /// The rewards "claim all" takes for `player`: every claimable one of a complete quest, but those marked
    /// `exclude_from_claim_all` and the choices (they wait for a pick).
    pub fn claim_all_list(&self, file: &QuestFile, player: Uuid) -> Vec<u64> {
        file.rewards
            .values()
            .filter(|r| !r.exclude_from_claim_all && !matches!(r.kind, RewardKind::Choice { .. }) && self.is_completed(r.quest))
            .filter(|r| self.claimable(file, player, r.id).is_ok())
            .map(|r| r.id)
            .collect()
    }
}
