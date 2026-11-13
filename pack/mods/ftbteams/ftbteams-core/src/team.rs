//! A team: its type, ranks, properties, message history and extra data, and its file.

use crate::{
    NIL, Uuid,
    ftblib::{
        color::{self, Argb},
        parse_uuid,
        snbt::{Compound, Value},
        uuid_string,
    },
    msg::{Msg, color as c},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TeamType {
    Player,
    Party,
    Server,
}

impl TeamType {
    pub const ALL: [TeamType; 3] = [TeamType::Player, TeamType::Party, TeamType::Server];
    /// The serialized name, also the folder under `<world>/ftbteams/`.
    pub fn name(self) -> &'static str {
        match self {
            TeamType::Player => "player",
            TeamType::Party => "party",
            TeamType::Server => "server",
        }
    }
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.name() == name)
    }
    /// Its ChatFormatting colour in team names.
    pub fn color(self) -> u32 {
        match self {
            TeamType::Player => c::GRAY,
            TeamType::Party => c::AQUA,
            TeamType::Server => c::LIGHT_PURPLE,
        }
    }
}

/// `TeamRank`, in its order (the wire's ordinal).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rank {
    Enemy,
    None,
    Ally,
    Invited,
    Member,
    Officer,
    Owner,
}

impl Rank {
    pub const ALL: [Rank; 7] = [Rank::Enemy, Rank::None, Rank::Ally, Rank::Invited, Rank::Member, Rank::Officer, Rank::Owner];
    pub fn power(self) -> i32 {
        match self {
            Rank::Enemy => -100,
            Rank::None => 0,
            Rank::Ally => 50,
            Rank::Invited => 75,
            Rank::Member => 100,
            Rank::Officer => 500,
            Rank::Owner => 1000,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Rank::Enemy => "enemy",
            Rank::None => "none",
            Rank::Ally => "ally",
            Rank::Invited => "invited",
            Rank::Member => "member",
            Rank::Officer => "officer",
            Rank::Owner => "owner",
        }
    }
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.name() == name)
    }
    /// `TeamRank.isAtLeast`: a negative `other` turns the test round (only enemies are "at least an enemy").
    pub fn is_at_least(self, other: Rank) -> bool {
        if other.power() >= 0 { self.power() >= other.power() } else { self.power() <= other.power() }
    }
}

/// A team chat line kept in the history.
#[derive(Clone, Debug, PartialEq)]
pub struct TeamMessage {
    pub from: Uuid,
    /// Milliseconds since the epoch.
    pub date: i64,
    /// The component's JSON, as `Component.Serializer.toJson` wrote it.
    pub text: String,
}

/// FTB Teams' own properties (`ftbteams:*`); other mods' values are kept as they were read.
#[derive(Clone, Debug, PartialEq)]
pub struct Properties {
    pub display_name: String,
    pub description: String,
    pub color: Argb,
    pub free_to_join: bool,
    pub max_msg_history_size: i32,
    pub team_stages: BTreeSet<String>,
    pub lives_remaining: i32,
    /// Properties of other mods (FTB Chunks'), by id, as read from the file.
    pub other: Vec<(String, Value)>,
}

impl Default for Properties {
    fn default() -> Self {
        Self {
            display_name: String::new(),
            description: String::new(),
            color: color::WHITE,
            free_to_join: false,
            max_msg_history_size: 1000,
            team_stages: BTreeSet::new(),
            lives_remaining: 0,
            other: Vec::new(),
        }
    }
}

/// A property FTB Teams registers: its id and how it is shown and set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prop {
    DisplayName,
    Description,
    Color,
    FreeToJoin,
    MaxMsgHistorySize,
    TeamStages,
    LivesRemaining,
}

impl Prop {
    pub const ALL: [Prop; 7] = [
        Prop::DisplayName,
        Prop::Description,
        Prop::Color,
        Prop::FreeToJoin,
        Prop::MaxMsgHistorySize,
        Prop::TeamStages,
        Prop::LivesRemaining,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Prop::DisplayName => "ftbteams:display_name",
            Prop::Description => "ftbteams:description",
            Prop::Color => "ftbteams:color",
            Prop::FreeToJoin => "ftbteams:free_to_join",
            Prop::MaxMsgHistorySize => "ftbteams:max_msg_history_size",
            Prop::TeamStages => "ftbteams:team_stages",
            Prop::LivesRemaining => "ftbteams:lives_remaining",
        }
    }
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id() == id)
    }
    /// `ftbteamsconfig.ftbteams.<name>` and its English name.
    pub fn translation(self) -> (&'static str, &'static str) {
        match self {
            Prop::DisplayName => ("ftbteamsconfig.ftbteams.display_name", "Team name"),
            Prop::Description => ("ftbteamsconfig.ftbteams.description", "About"),
            Prop::Color => ("ftbteamsconfig.ftbteams.color", "Colour"),
            Prop::FreeToJoin => ("ftbteamsconfig.ftbteams.free_to_join", "Open to everyone"),
            Prop::MaxMsgHistorySize => ("ftbteamsconfig.ftbteams.max_msg_history_size", "Chat history length"),
            Prop::TeamStages => ("ftbteamsconfig.ftbteams.team_stages", "Stages"),
            Prop::LivesRemaining => ("ftbteamsconfig.ftbteams.lives_remaining", "Lives left"),
        }
    }
    pub fn player_editable(self) -> bool {
        !matches!(self, Prop::TeamStages | Prop::LivesRemaining)
    }
    /// Whether every client gets the value (the others go to the team's own members only).
    pub fn sync_to_all(self) -> bool {
        matches!(self, Prop::DisplayName | Prop::Color)
    }
}

impl Properties {
    /// `TeamProperty.toString` of the value.
    pub fn show(&self, prop: Prop) -> String {
        match prop {
            Prop::DisplayName => self.display_name.clone(),
            Prop::Description => self.description.clone(),
            Prop::Color => color::to_string(self.color),
            Prop::FreeToJoin => self.free_to_join.to_string(),
            Prop::MaxMsgHistorySize => self.max_msg_history_size.to_string(),
            Prop::TeamStages => format!("[{}]", self.team_stages.iter().cloned().collect::<Vec<_>>().join("\t")),
            Prop::LivesRemaining => self.lives_remaining.to_string(),
        }
    }
    /// `TeamProperty.fromString` and set: false when the text doesn't parse.
    pub fn set_from_string(&mut self, prop: Prop, text: &str) -> bool {
        let int = |min: i64| text.parse::<i64>().ok().filter(|v| i32::try_from(*v).is_ok()).map(|v| v.max(min) as i32);
        match prop {
            // The pattern `.{3,}`: three characters at least, none a line break.
            Prop::DisplayName if text.chars().count() >= 3 && !text.contains(['\n', '\r']) => {
                self.display_name = text.to_owned()
            }
            Prop::Description => self.description = text.to_owned(),
            Prop::Color => match color::parse(text) {
                Some(v) => self.color = v,
                None => return false,
            },
            Prop::FreeToJoin if text == "true" || text == "false" => self.free_to_join = text == "true",
            Prop::MaxMsgHistorySize => match int(i32::MIN as i64) {
                Some(v) => self.max_msg_history_size = v,
                None => return false,
            },
            Prop::LivesRemaining => match int(0) {
                Some(v) => self.lives_remaining = v,
                None => return false,
            },
            Prop::TeamStages if text.len() > 2 && text.starts_with('[') && text.ends_with(']') => {
                self.team_stages = text[1..text.len() - 1].split('\t').map(str::to_owned).collect()
            }
            _ => return false,
        }
        true
    }
    fn write(&self) -> Compound {
        let mut c = Compound::default();
        c.put(Prop::DisplayName.id(), Value::String(self.display_name.clone()));
        c.put(Prop::Description.id(), Value::String(self.description.clone()));
        c.put(Prop::Color.id(), Value::String(color::to_string(self.color)));
        c.put(Prop::FreeToJoin.id(), Value::Bool(self.free_to_join));
        c.put(Prop::MaxMsgHistorySize.id(), Value::Int(self.max_msg_history_size));
        let stages = self.team_stages.iter().map(|s| Value::String(s.clone())).collect();
        c.put(Prop::TeamStages.id(), Value::List(stages));
        c.put(Prop::LivesRemaining.id(), Value::Int(self.lives_remaining));
        for (k, v) in &self.other {
            c.put(k, v.clone());
        }
        c
    }
    /// A value that doesn't read keeps the default, as `fromNBT` falls back.
    fn read(c: &Compound) -> Self {
        let mut p = Properties::default();
        for (key, value) in &c.0 {
            match Prop::from_id(key) {
                Some(Prop::DisplayName) => p.display_name = value.as_str().unwrap_or_default().to_owned(),
                Some(Prop::Description) => p.description = value.as_str().unwrap_or_default().to_owned(),
                Some(Prop::Color) => p.color = value.as_str().and_then(color::parse).unwrap_or(color::WHITE),
                Some(Prop::FreeToJoin) => p.free_to_join = value.as_i64() == Some(1),
                Some(Prop::MaxMsgHistorySize) => p.max_msg_history_size = value.as_i64().map_or(1000, |v| v as i32),
                Some(Prop::LivesRemaining) => p.lives_remaining = value.as_i64().map_or(0, |v| v.max(0) as i32),
                Some(Prop::TeamStages) => {
                    let Value::List(items) = value else { continue };
                    p.team_stages = items.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect();
                }
                None => p.other.push((key.clone(), value.clone())),
            }
        }
        p
    }
}

/// A team of any type. A party's owner is also in its ranks, as `owner`.
#[derive(Clone, Debug, PartialEq)]
pub struct Team {
    pub id: Uuid,
    pub kind: TeamType,
    /// A party's owner; NIL for the others.
    pub owner: Uuid,
    /// A personal team's player name.
    pub player_name: String,
    pub ranks: BTreeMap<Uuid, Rank>,
    pub props: Properties,
    pub history: Vec<TeamMessage>,
    pub extra: Compound,
    pub dirty: bool,
}

impl Team {
    pub fn new(id: Uuid, kind: TeamType) -> Self {
        Self {
            id,
            kind,
            owner: NIL,
            player_name: String::new(),
            ranks: BTreeMap::new(),
            props: Properties::default(),
            history: Vec::new(),
            extra: Compound::default(),
            dirty: true,
        }
    }
    pub fn is_party(&self) -> bool {
        self.kind == TeamType::Party
    }
    /// `getRankForPlayer`.
    pub fn rank_of(&self, player: Uuid) -> Rank {
        match self.kind {
            TeamType::Player if player == self.id => Rank::Owner,
            TeamType::Party if player == self.owner => Rank::Owner,
            TeamType::Party => self.ranks.get(&player).copied().unwrap_or(if self.props.free_to_join { Rank::Invited } else { Rank::None }),
            _ => self.ranks.get(&player).copied().unwrap_or(Rank::None),
        }
    }
    /// The players of rank member and above.
    pub fn members(&self) -> Vec<Uuid> {
        self.ranks.iter().filter(|(_, r)| r.is_at_least(Rank::Member)).map(|(id, _)| *id).collect()
    }
    pub fn is_member(&self, player: Uuid) -> bool {
        self.rank_of(player).is_at_least(Rank::Member)
    }
    /// `getShortName`: the name's word characters (others become `_`), 50 at most, `#` and the id's first 8 digits.
    pub fn short_name(&self) -> String {
        let name: String = self
            .props
            .display_name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
            .take(50)
            .collect();
        format!("{name}#{}", &uuid_string(self.id)[..8])
    }
    /// `getName`: the display name in the type's colour, clicking it shows the team's info.
    pub fn name_msg(&self) -> Msg {
        self.styled_name(self.kind.color())
    }
    /// `getColoredName`: the same in the team's colour.
    pub fn colored_name_msg(&self) -> Msg {
        self.styled_name(self.props.color & 0xFF_FFFF)
    }
    fn styled_name(&self, color: u32) -> Msg {
        let command = format!("/ftbteams info {}", self.short_name());
        Msg::lit(self.props.display_name.clone()).styled(|s| {
            s.color = Some(color);
            s.run = Some(command);
            s.hover = Some(Box::new(Msg::tr("ftbteams.click_show_info", "Click to see the team's details", vec![])));
        })
    }
    /// Adds a chat line, the oldest going past `max_msg_history_size`.
    pub fn add_message(&mut self, message: TeamMessage) {
        self.history.push(message);
        let max = self.props.max_msg_history_size.max(0) as usize;
        if self.history.len() > max {
            let cut = self.history.len() - max;
            self.history.drain(..cut);
        }
        self.dirty = true;
    }

    /// `serializeNBT`: the team's file.
    pub fn write(&self) -> Compound {
        let mut c = Compound::default();
        c.put("id", Value::String(uuid_string(self.id)));
        c.put("type", Value::String(self.kind.name().into()));
        match self.kind {
            TeamType::Party => c.put("owner", Value::String(uuid_string(self.owner))),
            TeamType::Player => c.put("player_name", Value::String(self.player_name.clone())),
            TeamType::Server => {}
        }
        let mut ranks = Compound::default();
        for (id, rank) in &self.ranks {
            ranks.put(&uuid_string(*id), Value::String(rank.name().into()));
        }
        c.put("ranks", Value::Compound(ranks));
        c.put("properties", Value::Compound(self.props.write()));
        let history = self
            .history
            .iter()
            .map(|m| {
                let mut e = Compound::default();
                e.put("from", Value::String(uuid_string(m.from)));
                e.put("date", Value::Long(m.date));
                e.put("text", Value::String(m.text.clone()));
                Value::Compound(e)
            })
            .collect();
        c.put("message_history", Value::List(history));
        c.put("extra", Value::Compound(self.extra.clone()));
        c
    }

    /// `deserializeNBT`, the type given by the folder the file was in.
    pub fn read(kind: TeamType, c: &Compound) -> Option<Self> {
        let id = parse_uuid(c.str("id")?)?;
        let mut t = Team::new(id, kind);
        t.dirty = false;
        t.owner = c.str("owner").and_then(parse_uuid).unwrap_or(NIL);
        t.player_name = c.str("player_name").unwrap_or_default().to_owned();
        if let Some(ranks) = c.compound("ranks") {
            for (k, v) in &ranks.0 {
                if let (Some(id), Some(rank)) = (parse_uuid(k), v.as_str().and_then(Rank::from_name)) {
                    t.ranks.insert(id, rank);
                }
            }
        }
        t.props = c.compound("properties").map(Properties::read).unwrap_or_default();
        for entry in c.list("message_history") {
            let Value::Compound(e) = entry else { continue };
            t.history.push(TeamMessage {
                from: e.str("from").and_then(parse_uuid).unwrap_or(NIL),
                date: e.get("date").and_then(Value::as_i64).unwrap_or(0),
                text: e.str("text").unwrap_or_default().to_owned(),
            });
        }
        t.extra = c.compound("extra").cloned().unwrap_or_default();
        Some(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_compare_as_java() {
        assert!(Rank::Invited.is_at_least(Rank::Ally));
        assert!(!Rank::Ally.is_at_least(Rank::Invited));
        assert!(Rank::Enemy.is_at_least(Rank::Enemy));
        assert!(!Rank::None.is_at_least(Rank::Enemy));
    }

    #[test]
    fn short_names() {
        let mut t = Team::new(0x1a2b3c4d_0000_4000_8000_000000000000, TeamType::Party);
        t.props.display_name = "Dev's Team ü".into();
        assert_eq!(t.short_name(), "Dev_s_Team__#1a2b3c4d");
    }

    #[test]
    fn files_round_trip() {
        let mut t = Team::new(7, TeamType::Party);
        t.owner = 1;
        t.ranks.insert(1, Rank::Owner);
        t.ranks.insert(2, Rank::Invited);
        t.props.display_name = "P".into();
        t.props.other.push(("ftbchunks:allow_pvp".into(), Value::Bool(true)));
        t.add_message(TeamMessage { from: NIL, date: 3, text: "{\"text\":\"x\"}".into() });
        let back = Team::read(TeamType::Party, &t.write()).unwrap();
        assert_eq!(Team { dirty: true, ..back }, t);
    }
}
