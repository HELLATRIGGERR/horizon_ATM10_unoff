//! FTB Quests 2101.1.37 for the pack's port: the quest file (`file`), a team's progress in FTB's saved layout
//! (`team`), and the messages between the server and the book screen. Reimplemented from the mod's behaviour and
//! its files' layout; FTB Quests is All Rights Reserved and nothing of it is copied here.

pub mod file;
pub mod snbt;
pub mod team;

use serde::{Deserialize, Serialize};

pub type Uuid = u128;

/// Server to client: the player's team's progress (ids as 16 hex digits), in parts.
pub const SYNC: &str = "ftbquests:sync_team_data";
/// Server to client: open the book (the book item, `/ftbquests open_book`).
pub const OPEN_BOOK: &str = "ftbquests:open_quest_book";
/// Client to server: tick a checkmark task.
pub const SUBMIT_TASK: &str = "ftbquests:submit_task";
/// Client to server: claim a reward.
pub const CLAIM_REWARD: &str = "ftbquests:claim_reward";

/// One part of the team's progress. `reset` starts a new picture; a message holds at most `PART` entries.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Sync {
    pub reset: bool,
    pub team: String,
    pub tasks: Vec<(String, i64)>,
    pub completed: Vec<String>,
    /// The rewards this player has (their own and the team's).
    pub claimed: Vec<String>,
}
pub const PART: usize = 600;

/// What the client asks of the server: the object's id, 16 hex digits.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Request {
    pub id: String,
}

/// The progress of `data` for `player`, cut into messages.
pub fn sync_parts(file: &file::QuestFile, data: &team::TeamData, player: Uuid) -> Vec<Sync> {
    let tasks: Vec<(String, i64)> = data.task_progress.iter().map(|(k, v)| (file::code(*k), *v)).collect();
    let completed: Vec<String> = data.completed.keys().map(|k| file::code(*k)).collect();
    let claimed: Vec<String> = data.claimed.keys().filter(|(p, r)| *p == player || (*p == 0 && file.rewards.contains_key(r))).map(|(_, r)| file::code(*r)).collect();
    let mut parts = Vec::new();
    let (mut t, mut c, mut r) = (tasks.chunks(PART), completed.chunks(PART), claimed.chunks(PART));
    loop {
        let (a, b, d) = (t.next(), c.next(), r.next());
        if parts.is_empty() || a.is_some() || b.is_some() || d.is_some() {
            parts.push(Sync {
                reset: parts.is_empty(),
                team: team::undashed(data.uuid),
                tasks: a.unwrap_or_default().to_vec(),
                completed: b.unwrap_or_default().to_vec(),
                claimed: d.unwrap_or_default().to_vec(),
            });
        } else {
            return parts;
        }
    }
}

#[cfg(test)]
mod tests;
