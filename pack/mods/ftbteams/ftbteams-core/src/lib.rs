//! FTB Teams 2101.1.11's behaviour, as the pack's `ftbteams` native bundle runs it: every player's personal
//! team, parties with their owner, officers, members, invites and allies, server teams, the teams' properties,
//! team chat, `/ftbteams`, the files under `<world>/ftbteams/` and what clients are told.
//!
//! Nothing here touches the game: the runtime (`ftbteams_world`) feeds logins and commands in and carries out
//! what [`Out`](manager::Out) asks for (chat lines, syncs, presence).
//!
//! `ftblib` holds what FTB Library gives FTB Teams and FTB Chunks: SNBT, colours, UUID strings.

pub mod commands;
pub mod ftblib;
pub mod manager;
pub mod msg;
pub mod sync;
pub mod team;

pub use manager::{Config, Event, Manager, Out};
pub use msg::Msg;
pub use team::{Rank, Team, TeamType};

/// A player's or a team's id, as Java's `UUID` (most significant bits first).
pub type Uuid = u128;
/// `Util.NIL_UUID`: "nobody", the sender of the system's own team messages.
pub const NIL: Uuid = 0;

#[cfg(test)]
mod tests;
