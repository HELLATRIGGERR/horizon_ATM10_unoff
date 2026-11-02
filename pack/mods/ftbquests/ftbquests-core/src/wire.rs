//! The messages between the mod's server and client halves (API 40's channels,
//! at most 30 000 bytes each). What a client sends is a player's own input, so
//! decoding never trusts a length.

use crate::book::Id;
use crate::progress::Change;

pub const TO_SERVER: &str = "ftbquests:to_server";
pub const TO_CLIENT: &str = "ftbquests:to_client";

/// The most entries in one message: 17 bytes each, under the channel's limit.
pub const MAX_ENTRIES: usize = 1700;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToServer {
    /// The book opened: send this player's progress.
    Hello,
    /// A task's button: a checkmark, or items to hand in.
    Submit(Id),
    /// A reward's button.
    Claim(Id),
    /// The Collect button: every item reward waiting.
    ClaimAll,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToClient {
    /// The book item was used: open the screen.
    Open,
    /// Forget what you know: entries follow.
    Reset,
    Changes(Vec<Change>),
}

impl ToServer {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            ToServer::Hello => vec![1],
            ToServer::Submit(id) => with_id(2, *id),
            ToServer::Claim(id) => with_id(3, *id),
            ToServer::ClaimAll => vec![4],
        }
    }
    pub fn decode(data: &[u8]) -> Option<Self> {
        let (&kind, rest) = data.split_first()?;
        Some(match kind {
            1 if rest.is_empty() => ToServer::Hello,
            2 => ToServer::Submit(id_of(rest)?),
            3 => ToServer::Claim(id_of(rest)?),
            4 if rest.is_empty() => ToServer::ClaimAll,
            _ => return None,
        })
    }
}

impl ToClient {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            ToClient::Open => vec![1],
            ToClient::Reset => vec![2],
            ToClient::Changes(all) => {
                let mut out = Vec::with_capacity(1 + all.len() * 17);
                out.push(3);
                for change in all {
                    let (kind, id, value) = match *change {
                        Change::Task(i, v) => (0u8, i, v),
                        Change::Started(i, v) => (1, i, v),
                        Change::Completed(i, v) => (2, i, v),
                        Change::Claimed(i, v) => (3, i, v),
                    };
                    out.push(kind);
                    out.extend_from_slice(&id.to_le_bytes());
                    out.extend_from_slice(&value.to_le_bytes());
                }
                out
            }
        }
    }
    pub fn decode(data: &[u8]) -> Option<Self> {
        let (&kind, rest) = data.split_first()?;
        Some(match kind {
            1 => ToClient::Open,
            2 => ToClient::Reset,
            3 => {
                if rest.len() % 17 != 0 {
                    return None;
                }
                let mut all = Vec::with_capacity(rest.len() / 17);
                for entry in rest.chunks_exact(17) {
                    let id = u64::from_le_bytes(entry[1..9].try_into().ok()?);
                    let value = u64::from_le_bytes(entry[9..17].try_into().ok()?);
                    all.push(match entry[0] {
                        0 => Change::Task(id, value),
                        1 => Change::Started(id, value),
                        2 => Change::Completed(id, value),
                        3 => Change::Claimed(id, value),
                        _ => return None,
                    });
                }
                ToClient::Changes(all)
            }
            _ => return None,
        })
    }
}

fn with_id(kind: u8, id: Id) -> Vec<u8> {
    let mut out = vec![kind];
    out.extend_from_slice(&id.to_le_bytes());
    out
}

fn id_of(data: &[u8]) -> Option<Id> {
    Some(u64::from_le_bytes(data.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_survive_the_wire() {
        for m in [ToServer::Hello, ToServer::Submit(7), ToServer::Claim(u64::MAX), ToServer::ClaimAll] {
            assert_eq!(ToServer::decode(&m.encode()), Some(m));
        }
        let changes = vec![Change::Task(1, 2), Change::Started(3, 4), Change::Completed(5, 6), Change::Claimed(u64::MAX, 8)];
        for m in [ToClient::Open, ToClient::Reset, ToClient::Changes(changes), ToClient::Changes(vec![])] {
            assert_eq!(ToClient::decode(&m.encode()), Some(m));
        }
    }

    #[test]
    fn anything_else_is_refused() {
        assert_eq!(ToServer::decode(&[]), None);
        assert_eq!(ToServer::decode(&[2, 1, 2]), None);
        assert_eq!(ToServer::decode(&[1, 0]), None);
        assert_eq!(ToServer::decode(&[9]), None);
        assert_eq!(ToClient::decode(&[3, 1]), None);
        let mut bad_kind = vec![3, 9];
        bad_kind.extend_from_slice(&[0; 16]);
        assert_eq!(ToClient::decode(&bad_kind), None);
    }

    #[test]
    fn the_largest_message_fits_the_channel() {
        let all = vec![Change::Task(1, 1); MAX_ENTRIES];
        assert!(ToClient::Changes(all).encode().len() <= 30_000);
    }
}
