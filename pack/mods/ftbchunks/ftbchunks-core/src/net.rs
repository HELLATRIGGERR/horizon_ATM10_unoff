//! The messages between FTB Chunks' server and its clients (FTB Chunks' `net` package), as the bundle carries them:
//! channels `ftbchunks:<id>` over Horizon's mod messages, JSON on the wire (as FTB Teams' sync is), each under the
//! 30 000-byte cap. The fields are FTB Chunks' payloads; their encoding is ours.

use crate::{ChunkPos, Chunks, Claim, Config, Problem, TeamView, Uuid, can_use};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const LOGIN_DATA: &str = "ftbchunks:login_data";
pub const GENERAL_DATA: &str = "ftbchunks:send_general_data";
pub const MANY_CHUNKS: &str = "ftbchunks:send_many_chunks";
pub const CHUNK_CHANGE: &str = "ftbchunks:request_chunk_change";
pub const CHANGE_RESPONSE: &str = "ftbchunks:chunk_change_response";

/// Chunks a `send_many_chunks` message carries at most (about 60 bytes each).
pub const CHUNKS_PER_MESSAGE: usize = 400;

/// `ChunkChangeOp`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Op {
    Claim,
    Unclaim,
    Load,
    Unload,
}

impl Op {
    /// `ChunkChangeOp.create`: the left button claims, the right unclaims; Shift makes them force-load and release.
    pub fn create(left: bool, shift: bool) -> Op {
        match (left, shift) {
            (true, false) => Op::Claim,
            (false, false) => Op::Unclaim,
            (true, true) => Op::Load,
            (false, true) => Op::Unload,
        }
    }
}

/// `request_chunk_change` (C2S): the chunks are in the sender's own dimension.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChangeRequest {
    pub op: Op,
    pub chunks: Vec<[i32; 2]>,
    #[serde(default)]
    pub admin: bool,
    /// The team the screen was opened as (`open_claim_gui_as`); taken only from an operator or a member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<String>,
}

/// `chunk_change_response` (S2C): how many of the requested chunks changed, and the problems by id.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChangeResponse {
    pub changed: u32,
    pub total: u32,
    pub problems: BTreeMap<String, u32>,
}

/// `send_general_data` (S2C): the claim screen's counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneralData {
    pub claimed: i32,
    pub loaded: i32,
    pub max_claims: i32,
    pub max_force_loads: i32,
}

/// `ChunkSyncInfo`: a chunk as a client sees it; times are seconds relative to now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkInfo {
    pub x: i32,
    pub z: i32,
    pub claimed: bool,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub since: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force_loaded: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<i64>,
}

fn is_zero(v: &i64) -> bool {
    *v == 0
}

impl ChunkInfo {
    pub fn unclaimed(x: i32, z: i32) -> Self {
        ChunkInfo { x, z, claimed: false, since: 0, force_loaded: None, expires: None }
    }
    pub fn of(x: i32, z: i32, claim: &Claim, now_ms: i64) -> Self {
        let secs = |t: i64| (now_ms - t).max(0) / 1000;
        ChunkInfo {
            x,
            z,
            claimed: true,
            since: secs(claim.time),
            force_loaded: (claim.force_loaded > 0).then(|| secs(claim.force_loaded)),
            expires: (claim.force_loaded > 0 && claim.expiry > 0).then(|| (claim.expiry - now_ms).max(0) / 1000),
        }
    }
}

/// `send_many_chunks` (S2C): chunks of one dimension, claimed by `team` or let go by it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ManyChunks {
    pub dim: String,
    pub team: String,
    pub chunks: Vec<ChunkInfo>,
}

/// The dashed form of a UUID, as FTB Teams writes it.
pub fn uuid_text(id: Uuid) -> String {
    let h = format!("{id:032x}");
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

pub fn parse_uuid_text(text: &str) -> Option<Uuid> {
    let hex: String = text.chars().filter(|c| *c != '-').collect();
    (hex.len() == 32).then(|| u128::from_str_radix(&hex, 16).ok()).flatten()
}

/// Whether `player` sees `team`'s claims (`claim_visibility`): others see those chunks as unclaimed.
pub fn sees_claims(config: &Config, team: &TeamView, player: Uuid) -> bool {
    can_use(config, team, player, team.props.claim_visibility)
}

impl Chunks {
    /// `RequestChunkChangePacket.handle` once the team is known: `op` over each chunk of `chunks` in `dimension`,
    /// as `team` (the sender's, or the one the screen was opened as). `admin` lets an operator change other teams'
    /// chunks. Returns the answer and the chunks that changed.
    pub fn change(
        &mut self,
        team: &TeamView,
        op: Op,
        dimension: &str,
        chunks: &[[i32; 2]],
        admin: bool,
        now: i64,
    ) -> (ChangeResponse, Vec<ChunkPos>) {
        let mut response = ChangeResponse::default();
        let mut changed = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for &[x, z] in chunks {
            if !seen.insert((x, z)) {
                continue;
            }
            response.total += 1;
            let pos: ChunkPos = (dimension.to_owned(), x, z);
            let result: Result<(), Problem> = match op {
                Op::Claim => self.claim(team, pos.clone(), now),
                Op::Unclaim => self.unclaim(team.id, &pos, admin).map(|_| ()),
                Op::Load => self.load(team, &pos, admin, now),
                Op::Unload => self.unload(team.id, &pos, admin),
            };
            match result {
                Ok(()) => {
                    response.changed += 1;
                    changed.push(pos);
                }
                Err(p) => *response.problems.entry(p.id().to_owned()).or_default() += 1,
            }
        }
        (response, changed)
    }

    /// `send_general_data` for a player of `team`.
    pub fn general_data(&self, team: Uuid) -> GeneralData {
        let (max_claims, max_force_loads) = self.limits(team);
        GeneralData {
            claimed: self.claimed_by(team) as i32,
            loaded: self.force_loaded_by(team) as i32,
            max_claims,
            max_force_loads,
        }
    }

    /// The `send_many_chunks` messages for `positions` (claimed or let go), grouped by dimension and team. `before`
    /// gives the team of a chunk no longer claimed. `visible(team)` is false where the recipient must not see the
    /// team's claims: those chunks go as unclaimed.
    pub fn many_chunks(
        &self,
        positions: &[ChunkPos],
        before: &BTreeMap<ChunkPos, Claim>,
        visible: &dyn Fn(Uuid) -> bool,
        now: i64,
    ) -> Vec<ManyChunks> {
        let mut groups: BTreeMap<(String, Uuid), Vec<ChunkInfo>> = BTreeMap::new();
        for pos in positions {
            let (dim, x, z) = pos;
            let (team, info) = match self.claims.get(pos) {
                Some(c) if visible(c.team) => (c.team, ChunkInfo::of(*x, *z, c, now)),
                Some(c) => (c.team, ChunkInfo::unclaimed(*x, *z)),
                None => match before.get(pos) {
                    Some(old) => (old.team, ChunkInfo::unclaimed(*x, *z)),
                    None => continue,
                },
            };
            groups.entry((dim.clone(), team)).or_default().push(info);
        }
        let mut out = Vec::new();
        for ((dim, team), infos) in groups {
            for part in infos.chunks(CHUNKS_PER_MESSAGE) {
                out.push(ManyChunks { dim: dim.clone(), team: uuid_text(team), chunks: part.to_vec() });
            }
        }
        out
    }

    /// The chunks whose claim changed between `before` and now.
    pub fn changed_since(&self, before: &BTreeMap<ChunkPos, Claim>) -> Vec<ChunkPos> {
        let mut out: Vec<ChunkPos> =
            self.claims.iter().filter(|(p, c)| before.get(*p) != Some(*c)).map(|(p, _)| p.clone()).collect();
        out.extend(before.keys().filter(|p| !self.claims.contains_key(*p)).cloned());
        out
    }
}
