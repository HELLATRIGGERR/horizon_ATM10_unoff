//! What clients are told of the teams (`SyncTeamsMessage`, `UpdatePresenceMessage`) and what a client keeps
//! (`ClientTeamManagerImpl`). Sent as JSON on `ftbteams:sync` and `ftbteams:presence`.

use crate::{
    Uuid,
    ftblib::{color, parse_uuid, uuid_string},
    manager::Manager,
    msg::Msg,
    team::{Prop, TeamType},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const SYNC_CHANNEL: &str = "ftbteams:sync";
pub const PRESENCE_CHANNEL: &str = "ftbteams:presence";
/// A message's bytes at most (the game's cap is 30 000).
pub const MAX_MESSAGE: usize = 28_000;

/// The component's JSON, as `Component.Serializer.toJson` writes it.
pub fn component_json(msg: &Msg) -> Value {
    match msg {
        Msg::Literal(s) => json!({ "text": s }),
        Msg::Translate { key, en, args } => {
            let mut v = json!({ "translate": key, "fallback": en });
            if !args.is_empty() {
                v["with"] = args.iter().map(component_json).collect();
            }
            v
        }
        Msg::Seq(items) => json!({ "text": "", "extra": items.iter().map(component_json).collect::<Vec<_>>() }),
        Msg::Styled(inner, style) => {
            let mut v = component_json(inner);
            if let Some(c) = style.color {
                v["color"] = json!(format!("#{c:06X}"));
            }
            if style.italic {
                v["italic"] = json!(true);
            }
            if let Some(cmd) = &style.run {
                v["click_event"] = json!({ "action": "run_command", "command": cmd });
            }
            if let Some(text) = &style.copy {
                v["click_event"] = json!({ "action": "copy_to_clipboard", "value": text });
            }
            if let Some(h) = &style.hover {
                v["hover_event"] = json!({ "action": "show_text", "value": component_json(h) });
            }
            v
        }
    }
}

/// A team as a client sees it. Only the recipient's own team carries every property.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ClientTeam {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub removed: bool,
    /// Property id to its string form (`TeamProperty.toString`).
    pub properties: BTreeMap<String, String>,
    /// Player id to rank name: every team's ranks go to every client, invites and allies included.
    pub ranks: BTreeMap<String, String>,
}

/// `KnownClientPlayer`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct KnownPlayer {
    pub id: String,
    pub name: String,
    pub online: bool,
    /// Their team: their party, else their own id.
    pub team: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SyncMessage {
    pub manager: String,
    #[serde(rename = "self")]
    pub self_team: String,
    pub full: bool,
    pub teams: Vec<ClientTeam>,
    pub players: Vec<KnownPlayer>,
}

impl Manager {
    fn client_player(&self, id: Uuid) -> Option<KnownPlayer> {
        let k = self.known.get(&id)?;
        Some(KnownPlayer { id: uuid_string(id), name: k.name.clone(), online: k.online, team: uuid_string(k.effective) })
    }

    /// `ClientTeam.copyOf` for `recipient`, or a removed team when it is gone.
    fn client_team(&self, id: Uuid, recipient_team: Uuid, removed_kind: TeamType) -> ClientTeam {
        let Some(t) = self.teams.get(&id) else {
            return ClientTeam { id: uuid_string(id), kind: removed_kind.name().into(), removed: true, ..ClientTeam::default() };
        };
        let properties = Prop::ALL
            .into_iter()
            .filter(|p| t.id == recipient_team || p.sync_to_all())
            .map(|p| (p.id().to_owned(), t.props.show(p)))
            .collect();
        ClientTeam {
            id: uuid_string(t.id),
            owner: (t.owner != crate::NIL).then(|| uuid_string(t.owner)),
            kind: t.kind.name().into(),
            removed: false,
            properties,
            ranks: t.ranks.iter().map(|(id, r)| (uuid_string(*id), r.name().to_owned())).collect(),
        }
    }

    /// The sync `recipient` gets: every team when `teams` is None (their login), else those teams; each personal
    /// team's player rides along. Split so that no message passes [`MAX_MESSAGE`] bytes; the first is the full one.
    pub fn sync_for(&self, recipient: Uuid, teams: Option<&[Uuid]>) -> Vec<Vec<u8>> {
        let own = self.known.get(&recipient).map_or(recipient, |k| k.effective);
        let ids: Vec<Uuid> = teams.map_or_else(|| self.teams.keys().copied().collect(), <[Uuid]>::to_vec);
        let full = teams.is_none();
        let mut parts = Vec::new();
        let mut current = SyncMessage { manager: uuid_string(self.id), self_team: uuid_string(own), full, ..SyncMessage::default() };
        let mut size = 0;
        for id in ids {
            // A team that is gone was a party, unless it was a player's.
            let kind = if self.known.contains_key(&id) { TeamType::Player } else { TeamType::Party };
            let team = self.client_team(id, own, kind);
            let player = self.client_player(id);
            let bytes = serde_json::to_vec(&team).map_or(0, |b| b.len()) + player.as_ref().map_or(0, |p| p.name.len() + 120);
            if size + bytes > MAX_MESSAGE && !current.teams.is_empty() {
                let next = SyncMessage { full: false, teams: Vec::new(), players: Vec::new(), ..current.clone() };
                parts.push(std::mem::replace(&mut current, next));
                size = 0;
            }
            size += bytes;
            current.teams.push(team);
            current.players.extend(player);
        }
        parts.push(current);
        parts.into_iter().filter_map(|m| serde_json::to_vec(&m).ok()).collect()
    }

    pub fn presence_for(&self, player: Uuid) -> Option<Vec<u8>> {
        serde_json::to_vec(&self.client_player(player)?).ok()
    }
}

/// What a client knows of the teams.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientManager {
    pub manager: Option<Uuid>,
    pub self_team: Option<Uuid>,
    pub teams: BTreeMap<Uuid, ClientTeam>,
    pub players: BTreeMap<Uuid, KnownPlayer>,
}

impl ClientManager {
    /// A full sync replaces everything; a delta puts or removes its teams.
    pub fn apply(&mut self, data: &[u8]) -> Result<(), String> {
        let m: SyncMessage = serde_json::from_slice(data).map_err(|e| e.to_string())?;
        if m.full {
            *self = ClientManager::default();
        }
        self.manager = parse_uuid(&m.manager);
        self.self_team = parse_uuid(&m.self_team);
        for t in m.teams {
            let Some(id) = parse_uuid(&t.id) else { continue };
            if t.removed {
                self.teams.remove(&id);
            } else {
                self.teams.insert(id, t);
            }
        }
        for p in m.players {
            if let Some(id) = parse_uuid(&p.id) {
                self.players.insert(id, p);
            }
        }
        Ok(())
    }
    pub fn apply_presence(&mut self, data: &[u8]) -> Result<(), String> {
        let p: KnownPlayer = serde_json::from_slice(data).map_err(|e| e.to_string())?;
        if let Some(id) = parse_uuid(&p.id) {
            self.players.insert(id, p);
        }
        Ok(())
    }
    /// The team a player is in, as this client knows it.
    pub fn team_of(&self, player: Uuid) -> Option<&ClientTeam> {
        let team = parse_uuid(&self.players.get(&player)?.team)?;
        self.teams.get(&team)
    }
    /// The colour a team is drawn in (claims on the map), opaque ARGB.
    pub fn color(&self, team: Uuid) -> Option<u32> {
        color::parse(self.teams.get(&team)?.properties.get(Prop::Color.id())?)
    }
}
