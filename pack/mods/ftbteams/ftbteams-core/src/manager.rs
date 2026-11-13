//! `TeamManagerImpl` and the teams' operations (`PartyTeam`, `PlayerTeam`, `ServerTeam`), with their messages.
//! Failures are the command exceptions' messages; what the game must do is queued in [`Manager::out`].

use crate::{
    NIL, Uuid,
    ftblib::{
        color, parse_uuid,
        snbt::{self, Compound, Value},
        uuid_string,
    },
    msg::{Msg, color as c},
    team::{Prop, Rank, Team, TeamMessage, TeamType},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::Path,
};

/// `ftbteams-server.snbt`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Config {
    /// Lives a party has; 0 turns lives off.
    pub limited_lives: i32,
    /// Members a party may have; 0 is no limit.
    pub max_party_size: i32,
}

impl Config {
    /// The world's `serverconfig/ftbteams-server.snbt` over the instance's `config/ftbteams-server.snbt`.
    pub fn load(instance_config: Option<&Path>, world: Option<&Path>) -> Self {
        let mut config = Config::default();
        let files = [instance_config.map(|d| d.join("ftbteams-server.snbt")), world.map(|w| w.join("serverconfig/ftbteams-server.snbt"))];
        for file in files.iter().flatten() {
            let Some(c) = snbt::read_file(file) else { continue };
            let int = |k: &str| c.get(k).and_then(Value::as_i64).map(|v| v.clamp(0, i32::MAX as i64) as i32);
            config.limited_lives = int("limited_lives").unwrap_or(config.limited_lives);
            config.max_party_size = int("max_party_size").unwrap_or(config.max_party_size);
        }
        config
    }
    fn party_full(&self, members: usize) -> bool {
        self.max_party_size != 0 && self.max_party_size as usize <= members
    }
}

/// A player the manager has seen: their personal team's player.
#[derive(Clone, Debug, PartialEq)]
pub struct Known {
    pub name: String,
    pub online: bool,
    /// The team they are in: their party, else their personal team (their own id).
    pub effective: Uuid,
}

/// What FTB Teams tells other mods (`TeamEvent`), for FTB Chunks.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Created { team: Uuid, by: Uuid },
    Deleted { team: Uuid },
    JoinedParty { party: Uuid, player: Uuid },
    LeftParty { party: Uuid, player: Uuid, deleted: bool },
    Ally { party: Uuid, players: Vec<Uuid>, adding: bool },
    OwnershipTransferred { party: Uuid, from: Uuid, to: Uuid },
    LoggedIn { team: Uuid, player: Uuid },
    PropertiesChanged { team: Uuid },
}

/// What the game must do after an operation.
#[derive(Clone, Debug, PartialEq)]
pub enum Out {
    /// A chat line to an online player (`displayClientMessage`).
    Tell(Uuid, Msg),
    /// These teams to every online player, each sync personalised.
    Sync(Vec<Uuid>),
    /// Everything to this player (their login).
    SyncAll(Uuid),
    /// This player's presence to everyone.
    Presence(Uuid),
    Event(Event),
}

pub type Fail = Msg;
type Res<T = ()> = Result<T, Fail>;

pub(crate) fn fail(key: &'static str, en: &'static str, args: Vec<Msg>) -> Fail {
    Msg::tr(key, en, args)
}

pub struct Manager {
    /// The world's manager id (`ftbteams.snbt` `id`); FTB Chunks names its client map folder after it.
    pub id: Uuid,
    pub teams: BTreeMap<Uuid, Team>,
    pub known: BTreeMap<Uuid, Known>,
    pub chat_redirected: BTreeSet<Uuid>,
    pub extra: Compound,
    pub config: Config,
    pub dirty: bool,
    /// Set by a creation or a deletion: FTB Teams saves at once then.
    pub save_now: bool,
    deleted: Vec<(Uuid, TeamType)>,
    pub out: Vec<Out>,
    /// The time for new team messages, milliseconds since the epoch; the caller keeps it current.
    pub now_ms: i64,
    rng: u64,
}

impl Manager {
    pub fn new(seed: u64) -> Self {
        let mut m = Manager {
            id: NIL,
            teams: BTreeMap::new(),
            known: BTreeMap::new(),
            chat_redirected: BTreeSet::new(),
            extra: Compound::default(),
            config: Config::default(),
            dirty: true,
            save_now: false,
            deleted: Vec::new(),
            out: Vec::new(),
            now_ms: 0,
            rng: seed | 1,
        };
        m.id = m.random_uuid();
        m
    }

    fn next(&mut self) -> u64 {
        // xorshift64*
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// A version 4 UUID.
    fn random_uuid(&mut self) -> Uuid {
        let raw = ((self.next() as u128) << 64) | self.next() as u128;
        (raw & !(0xF000 << 64) & !(0xC0 << 56)) | (0x4000 << 64) | (0x80 << 56)
    }
    fn random_color(&mut self) -> u32 {
        color::random_team_color((self.next() >> 40) as f32 / (1u64 << 24) as f32)
    }

    // ---- Lookups -------------------------------------------------------------------------------------------

    /// The team a player is in (`getTeamForPlayerID`): their party, else their personal team.
    pub fn team_of(&self, player: Uuid) -> Option<&Team> {
        self.teams.get(&self.known.get(&player)?.effective)
    }
    pub fn party_of(&self, player: Uuid) -> Option<Uuid> {
        self.team_of(player).filter(|t| t.is_party()).map(|t| t.id)
    }
    pub fn is_online(&self, player: Uuid) -> bool {
        self.known.get(&player).is_some_and(|k| k.online)
    }
    pub fn name_of(&self, player: Uuid) -> Option<&str> {
        self.known.get(&player).map(|k| k.name.as_str())
    }
    /// `getTeamByName`: by short name.
    pub fn team_by_short_name(&self, name: &str) -> Option<&Team> {
        self.teams.values().find(|t| t.short_name() == name)
    }
    /// A known player by name, any case.
    pub fn known_by_name(&self, name: &str) -> Option<(Uuid, String)> {
        self.known.iter().find(|(_, k)| k.name.eq_ignore_ascii_case(name)).map(|(id, k)| (*id, k.name.clone()))
    }
    /// The name shown in team chat: `Server` for the nil id, `Someone` for nobody known.
    pub fn player_name_msg(&self, player: Uuid) -> Msg {
        match (player, self.name_of(player)) {
            (NIL, _) => Msg::lit("Server").color(c::LIGHT_PURPLE),
            (_, Some(name)) => Msg::lit(name).color(c::YELLOW),
            _ => Msg::lit("Someone"),
        }
    }
    fn display_name_of(&self, player: Uuid) -> String {
        self.name_of(player).map_or_else(|| uuid_string(player), str::to_owned)
    }
    fn online_members(&self, team: &Team) -> Vec<Uuid> {
        let members = match team.kind {
            TeamType::Player => vec![team.id],
            _ => team.members(),
        };
        members.into_iter().filter(|m| self.is_online(*m)).collect()
    }

    fn mark(&mut self, team: Uuid) {
        if let Some(t) = self.teams.get_mut(&team) {
            t.dirty = true;
        }
    }
    fn tell(&mut self, player: Uuid, msg: Msg) {
        if self.is_online(player) {
            self.out.push(Out::Tell(player, msg));
        }
    }

    /// `Team.sendMessage`: into the history, and `<Name @Team>text` to the online members.
    pub fn send_message(&mut self, team: Uuid, from: Uuid, text: Msg) {
        let Some(t) = self.teams.get(&team) else { return };
        let line = Msg::lit("<")
            .then(self.player_name_msg(from))
            .then(Msg::lit(" @"))
            .then(t.name_msg())
            .then(Msg::lit(">"))
            .then(text.clone());
        let members = self.online_members(t);
        let message = TeamMessage { from, date: self.now_ms, text: crate::sync::component_json(&text).to_string() };
        if let Some(t) = self.teams.get_mut(&team) {
            t.add_message(message);
        }
        for m in members {
            self.out.push(Out::Tell(m, line.clone()));
        }
    }

    // ---- Logins ---------------------------------------------------------------------------------------------

    /// `playerLoggedIn`: a first login makes the player's personal team; `real` is false for the dev command's
    /// fake player, which gets no sync and fires no creation event.
    pub fn player_logged_in(&mut self, player: Uuid, name: &str, real: bool) {
        let mut all = false;
        if !self.known.contains_key(&player) {
            let mut team = Team::new(player, TeamType::Player);
            team.player_name = name.to_owned();
            team.props.display_name = name.to_owned();
            team.props.color = self.random_color();
            team.ranks.insert(player, Rank::Owner);
            self.teams.insert(player, team);
            self.known.insert(player, Known { name: name.to_owned(), online: false, effective: player });
            self.dirty = true;
            if real {
                self.out.push(Out::Event(Event::Created { team: player, by: player }));
            }
            self.save_now = true;
            all = true;
        } else if self.known[&player].name != name {
            self.known.get_mut(&player).unwrap().name = name.to_owned();
            if let Some(t) = self.teams.get_mut(&player) {
                t.player_name = name.to_owned();
                t.dirty = true;
            }
            self.dirty = true;
            all = true;
        }
        let effective = self.known[&player].effective;
        if real {
            self.out.push(Out::SyncAll(player));
        }
        if all {
            self.out.push(Out::Sync(vec![effective]));
        }
        self.known.get_mut(&player).unwrap().online = real;
        self.out.push(Out::Presence(player));
        if real {
            self.out.push(Out::Event(Event::LoggedIn { team: effective, player }));
        }
    }

    pub fn player_logged_out(&mut self, player: Uuid) {
        if let Some(k) = self.known.get_mut(&player) {
            k.online = false;
            self.out.push(Out::Presence(player));
        }
    }

    // ---- Parties --------------------------------------------------------------------------------------------

    /// `createParty`. An empty name is `<player>'s Team`.
    pub fn create_party(&mut self, player: Uuid, name: &str, description: Option<&str>, color: Option<u32>) -> Res<Uuid> {
        match self.team_of(player) {
            None => return Err(team_not_found(&uuid_string(player))),
            Some(t) if t.kind != TeamType::Player => return Err(already_in_party()),
            _ => {}
        }
        let id = self.random_uuid();
        let mut party = Team::new(id, TeamType::Party);
        party.owner = player;
        party.props.display_name = if name.is_empty() { format!("{}'s Team", self.display_name_of(player)) } else { name.to_owned() };
        party.props.color = self.random_color();
        self.teams.insert(id, party);
        self.dirty = true;
        self.out.push(Out::Event(Event::Created { team: id, by: player }));
        self.save_now = true;
        let extra = self.teams[&player].extra.clone();
        let lives = self.config.limited_lives;
        let party = self.teams.get_mut(&id).unwrap();
        if let Some(d) = description {
            party.props.description = d.to_owned();
        }
        if let Some(col) = color {
            party.props.color = col;
        }
        party.extra = extra;
        party.ranks.insert(player, Rank::Owner);
        self.known.get_mut(&player).unwrap().effective = id;
        let joined = fail("ftbteams.message.joined", "%s is now in the party", vec![Msg::lit(self.display_name_of(player)).color(c::YELLOW)]);
        self.send_message(id, NIL, joined.color(c::GREEN));
        if lives > 0 {
            self.teams.get_mut(&id).unwrap().props.lives_remaining = lives;
            let n = Msg::lit(lives.to_string());
            self.send_message(id, NIL, fail("ftbteams.message.limited_lives", "The party has %s of %s lives left", vec![n.clone(), n]).color(c::GOLD));
        }
        let personal = self.teams.get_mut(&player).unwrap();
        personal.ranks.remove(&player);
        personal.dirty = true;
        self.out.push(Out::Presence(player));
        self.out.push(Out::Sync(vec![id, player]));
        self.out.push(Out::Event(Event::JoinedParty { party: id, player }));
        Ok(id)
    }

    fn lives_check(&self, party: &Team) -> Res {
        if self.config.limited_lives > 0 && party.props.lives_remaining <= 0 {
            return Err(fail("ftbteams.out_of_lives", "The party has no lives left, so nobody new may be invited", vec![]));
        }
        if self.config.party_full(party.members().len()) {
            let max = Msg::lit(self.config.max_party_size.to_string());
            return Err(fail("ftbteams.party_full", "The party is full (limit %s)", vec![max]));
        }
        Ok(())
    }

    /// `PartyTeam.join`. The invite is the command's check, not this one's.
    pub fn join(&mut self, party: Uuid, player: Uuid) -> Res {
        self.lives_check(&self.teams[&party])?;
        match self.team_of(player) {
            None => return Err(team_not_found(&uuid_string(player))),
            Some(t) if t.kind != TeamType::Player => return Err(already_in_party()),
            _ => {}
        }
        self.known.get_mut(&player).unwrap().effective = party;
        self.teams.get_mut(&party).unwrap().ranks.insert(player, Rank::Member);
        let joined = fail("ftbteams.message.joined", "%s is now in the party", vec![Msg::lit(self.display_name_of(player)).color(c::YELLOW)]);
        self.send_message(party, NIL, joined.color(c::GREEN));
        self.mark(party);
        let personal = self.teams.get_mut(&player).unwrap();
        personal.ranks.remove(&player);
        personal.dirty = true;
        self.out.push(Out::Presence(player));
        self.out.push(Out::Sync(vec![party, player]));
        self.out.push(Out::Event(Event::JoinedParty { party, player }));
        Ok(())
    }

    /// `declineInvitation`: the invite becomes an ally entry. None when the player wasn't invited.
    pub fn decline(&mut self, party: Uuid, player: Uuid) -> Option<Msg> {
        let t = self.teams.get_mut(&party)?;
        if t.rank_of(player) != Rank::Invited {
            return None;
        }
        t.ranks.insert(player, Rank::Ally);
        t.dirty = true;
        self.out.push(Out::Sync(vec![party]));
        Some(fail("ftbteams.message.declined", "Invitation turned down", vec![]))
    }

    /// `PartyTeam.leave`: the last member leaving deletes the party.
    pub fn leave(&mut self, party: Uuid, player: Uuid) -> Res {
        let t = &self.teams[&party];
        if t.owner == player && t.members().len() > 1 {
            return Err(fail("ftbteams.owner_cant_leave", "Hand the party over to someone before you leave", vec![]));
        }
        self.known.get_mut(&player).unwrap().effective = player;
        let personal = self.teams.get_mut(&player).unwrap();
        personal.ranks.insert(player, Rank::Owner);
        personal.dirty = true;
        let name = Msg::lit(self.display_name_of(player)).color(c::YELLOW);
        self.send_message(party, NIL, fail("ftbteams.message.left_party", "%s left the party", vec![name]).color(c::GOLD));
        self.teams.get_mut(&party).unwrap().ranks.remove(&player);
        self.dirty = true;
        let deleted = self.teams[&party].members().is_empty();
        if deleted {
            self.delete_team(party);
        }
        self.out.push(Out::Presence(player));
        self.out.push(Out::Event(Event::LeftParty { party, player, deleted }));
        if deleted {
            self.out.push(Out::Event(Event::Deleted { team: party }));
        }
        self.out.push(Out::Sync(vec![party, player]));
        Ok(())
    }

    fn delete_team(&mut self, team: Uuid) {
        if let Some(t) = self.teams.remove(&team) {
            self.deleted.push((t.id, t.kind));
            self.dirty = true;
            self.save_now = true;
        }
    }

    /// `PartyTeam.invite`: players already in a party, and players never seen, are passed over.
    pub fn invite(&mut self, party: Uuid, inviter: Uuid, players: &[(Uuid, String)]) -> Res {
        self.lives_check(&self.teams[&party])?;
        for (player, name) in players {
            if self.team_of(*player).is_none_or(|t| t.is_party()) {
                continue;
            }
            let t = self.teams.get_mut(&party).unwrap();
            t.ranks.insert(*player, Rank::Invited);
            t.dirty = true;
            let short = t.short_name();
            let invited = fail("ftbteams.message.invited", "%s has been invited", vec![Msg::lit(name.clone()).color(c::YELLOW)]);
            self.send_message(party, inviter, invited.color(c::GREEN));
            if *player != inviter {
                let by = Msg::lit(self.display_name_of(inviter)).color(c::YELLOW);
                self.tell(*player, fail("ftbteams.message.invite_sent", "%s asks you to join their party", vec![by]));
                let accept = Msg::tr("ftbteams.accept", "Accept", vec![])
                    .color(c::GREEN)
                    .styled(|s| s.run = Some(format!("/ftbteams party join {short}")));
                let decline = Msg::tr("ftbteams.decline", "Decline", vec![])
                    .color(c::RED)
                    .styled(|s| s.run = Some(format!("/ftbteams party decline {short}")));
                self.tell(*player, Msg::lit("[").then(accept).then(Msg::lit("] [")).then(decline).then(Msg::lit("]")));
            }
        }
        self.out.push(Out::Sync(vec![party]));
        Ok(())
    }

    /// `PartyTeam.kick`. Any officer may kick anyone but the owner, as the command allows.
    pub fn kick(&mut self, party: Uuid, sender: Uuid, players: &[(Uuid, String)]) -> Res {
        for (player, name) in players {
            if self.known.get(player).map(|k| k.effective) != Some(party) {
                return Err(not_in_party());
            }
            if self.teams[&party].owner == *player {
                return Err(fail("ftbteams.cant_kick_owner", "The owner cannot be removed", vec![]));
            }
            self.known.get_mut(player).unwrap().effective = *player;
            let personal = self.teams.get_mut(player).unwrap();
            personal.ranks.insert(*player, Rank::Owner);
            personal.dirty = true;
            let party_name = self.teams[&party].name_msg();
            let kicked = |name: Msg, party: Msg| fail("ftbteams.message.kicked", "%s was removed from %s", vec![name, party]);
            self.send_message(party, sender, kicked(Msg::lit(name.clone()).color(c::YELLOW), party_name.clone()).color(c::GOLD));
            self.teams.get_mut(&party).unwrap().ranks.remove(player);
            self.mark(party);
            self.out.push(Out::Presence(*player));
            self.out.push(Out::Sync(vec![party, *player]));
            let display = Msg::lit(self.display_name_of(*player)).color(c::YELLOW);
            self.tell(*player, kicked(display, party_name.color(c::AQUA)));
            self.out.push(Out::Event(Event::LeftParty { party, player: *player, deleted: false }));
        }
        Ok(())
    }

    /// `transferOwnership`: the old owner becomes an officer. Ok(None) when the player already owns it.
    pub fn transfer(&mut self, party: Uuid, sender: Uuid, to: (Uuid, String)) -> Res<Option<Msg>> {
        let t = &self.teams[&party];
        if !t.members().contains(&to.0) {
            let who = Msg::lit(format!("{} ({})", to.1, uuid_string(to.0)));
            return Err(fail("ftbteams.not_member", "%s does not belong to %s", vec![who, t.name_msg()]));
        }
        if t.owner == to.0 {
            return Ok(Some(Msg::lit("That player already owns the team").color(c::RED)));
        }
        let t = self.teams.get_mut(&party).unwrap();
        let from = t.owner;
        t.ranks.insert(from, Rank::Officer);
        t.owner = to.0;
        t.ranks.insert(to.0, Rank::Owner);
        t.dirty = true;
        self.out.push(Out::Event(Event::OwnershipTransferred { party, from: sender, to: to.0 }));
        let msg = fail("ftbteams.message.transfer_owner", "%s now owns the team", vec![Msg::lit(to.1).color(c::YELLOW)]).color(c::GREEN);
        self.send_message(party, sender, msg);
        self.out.push(Out::Sync(vec![party]));
        Ok(None)
    }

    /// Officers promoted or demoted (the GUI's operations).
    pub fn set_officer(&mut self, party: Uuid, sender: Uuid, player: (Uuid, String), officer: bool) -> Res {
        let t = &self.teams[&party];
        let rank = t.rank_of(player.0);
        let name = Msg::lit(player.1.clone());
        match (officer, rank) {
            (true, Rank::Member) | (false, Rank::Officer) => {}
            (true, _) => return Err(fail("ftbteams.not_member", "%s does not belong to %s", vec![name, t.name_msg()])),
            (false, _) => return Err(fail("ftbteams.not_officer", "%s holds no officer rank in %s", vec![name, t.name_msg()])),
        }
        let t = self.teams.get_mut(&party).unwrap();
        t.ranks.insert(player.0, if officer { Rank::Officer } else { Rank::Member });
        t.dirty = true;
        let name = Msg::lit(player.1).color(c::YELLOW);
        let msg = if officer {
            fail("ftbteams.message.promoted", "%s is now an officer", vec![name]).color(c::GREEN)
        } else {
            fail("ftbteams.message.demoted", "%s is an ordinary member again", vec![name]).color(c::GOLD)
        };
        self.send_message(party, sender, msg);
        self.out.push(Out::Sync(vec![party]));
        Ok(())
    }

    /// `addAlly`: players already an ally or better (invited, members) are passed over.
    pub fn add_allies(&mut self, party: Uuid, sender: Uuid, players: &[(Uuid, String)]) {
        let mut added = Vec::new();
        for (player, name) in players {
            if self.teams[&party].rank_of(*player).is_at_least(Rank::Ally) {
                continue;
            }
            self.teams.get_mut(&party).unwrap().ranks.insert(*player, Rank::Ally);
            let msg = fail("ftbteams.message.add_ally", "%s was added to your allies", vec![Msg::lit(name.clone()).color(c::YELLOW)]);
            self.send_message(party, sender, msg.color(c::GREEN));
            let party_name = Msg::lit(self.teams[&party].props.display_name.clone());
            self.tell(*player, fail("ftbteams.message.now_allied", "'%s' counts you as an ally", vec![party_name]).color(c::GREEN));
            added.push(*player);
        }
        if !added.is_empty() {
            self.mark(party);
            self.out.push(Out::Sync(vec![party]));
            self.out.push(Out::Event(Event::Ally { party, players: added, adding: true }));
        }
    }

    /// `removeAlly`: allies and invited players (a pending invite is withdrawn too).
    pub fn remove_allies(&mut self, party: Uuid, sender: Uuid, players: &[(Uuid, String)]) {
        let mut removed = Vec::new();
        for (player, name) in players {
            let t = &self.teams[&party];
            if !t.rank_of(*player).is_at_least(Rank::Ally) || t.is_member(*player) {
                continue;
            }
            self.teams.get_mut(&party).unwrap().ranks.remove(player);
            let msg = fail("ftbteams.message.remove_ally", "%s was dropped from your allies", vec![Msg::lit(name.clone()).color(c::YELLOW)]);
            self.send_message(party, sender, msg.color(c::GOLD));
            let party_name = Msg::lit(self.teams[&party].props.display_name.clone());
            self.tell(*player, fail("ftbteams.message.no_longer_allied", "'%s' no longer counts you as an ally", vec![party_name]).color(c::GOLD));
            removed.push(*player);
        }
        if !removed.is_empty() {
            self.mark(party);
            self.out.push(Out::Sync(vec![party]));
            self.out.push(Out::Event(Event::Ally { party, players: removed, adding: false }));
        }
    }

    /// `listAllies`: the ally and invited entries.
    pub fn list_allies(&self, party: Uuid) -> Vec<Msg> {
        let t = &self.teams[&party];
        let mut lines = vec![Msg::lit("Allied players:")];
        for (id, rank) in &t.ranks {
            if rank.is_at_least(Rank::Ally) && !rank.is_at_least(Rank::Member) {
                lines.push(Msg::lit(self.display_name_of(*id)).color(c::YELLOW));
            }
        }
        if lines.len() == 1 {
            lines.push(Msg::lit("nobody"));
        }
        lines
    }

    /// `AbstractTeam.settings`: shows the value, or sets it. Err for a value that may not be set or doesn't parse.
    pub fn settings(&mut self, team: Uuid, prop: Prop, value: &str) -> Res<Msg> {
        let (key, en) = prop.translation();
        let name = Msg::tr(key, en, vec![]).color(c::YELLOW);
        let t = self.teams.get_mut(&team).unwrap();
        if value.is_empty() {
            return Ok(name.then(Msg::lit("=")).then(Msg::lit(t.props.show(prop)).color(c::AQUA)));
        }
        if !prop.player_editable() {
            return Err(fail("ftbteams.message.property_not_editable", "%s is read-only", vec![name]));
        }
        if !t.props.set_from_string(prop, value) {
            return Err(fail("ftbteams.message.parse_failed", "Could not read the value %s", vec![Msg::lit(value)]));
        }
        t.dirty = true;
        self.out.push(Out::Event(Event::PropertiesChanged { team }));
        self.out.push(Out::Sync(vec![team]));
        Ok(fail("ftbteams.message.set_property", "%s is now %s", vec![name, Msg::lit(value).color(c::AQUA)]))
    }

    /// `forceDisband`: every member but the owner kicked, then the owner leaves.
    pub fn force_disband(&mut self, party: Uuid, sender: Uuid) -> Res<Msg> {
        let t = &self.teams[&party];
        let (owner, name, id) = (t.owner, t.name_msg(), uuid_string(t.id));
        let others: Vec<(Uuid, String)> = t.members().into_iter().filter(|m| *m != owner).map(|m| (m, String::new())).collect();
        self.kick(party, sender, &others)?;
        self.leave(party, owner)?;
        Ok(fail("ftbteams.message.team_disbanded", "Party '%s' (%s) was disbanded by force", vec![name, Msg::lit(id)]).color(c::GOLD))
    }

    /// `createServerTeam`.
    pub fn create_server_team(&mut self, by: Uuid, name: &str) -> Res<Msg> {
        if name.chars().count() < 3 {
            return Err(fail("ftbteams.name_too_short", "The name needs at least 3 characters", vec![]));
        }
        let id = self.random_uuid();
        let mut t = Team::new(id, TeamType::Server);
        t.props.display_name = name.to_owned();
        let msg = fail("ftbteams.message.created_server_team", "Server team '%s' created", vec![t.name_msg()]);
        self.teams.insert(id, t);
        self.dirty = true;
        if by != NIL {
            self.out.push(Out::Event(Event::Created { team: id, by }));
        }
        self.save_now = true;
        self.out.push(Out::Sync(vec![id]));
        Ok(msg)
    }

    pub fn delete_server_team(&mut self, team: Uuid) -> Msg {
        let short = self.teams[&team].short_name();
        self.delete_team(team);
        self.out.push(Out::Sync(vec![team]));
        self.out.push(Out::Event(Event::Deleted { team }));
        fail("ftbteams.message.deleted_server_team", "Server team '%s' removed", vec![Msg::lit(short)])
    }

    /// `getTeamInfo`.
    pub fn info(&self, team: Uuid) -> Vec<Msg> {
        let t = &self.teams[&team];
        let kind = Msg::lit(t.kind.name()).color(t.kind.color());
        let copy = |s: String| Msg::lit(s.clone()).color(c::YELLOW).styled(|st| st.copy = Some(s));
        let mut lines = vec![
            Msg::lit("==").then(t.name_msg()).then(Msg::lit(" [")).then(kind).then(Msg::lit("] ==")),
            fail("ftbteams.info.id", "Full id: %s", vec![copy(uuid_string(t.id))]),
            fail("ftbteams.info.short_id", "Short id: %s", vec![copy(t.short_name())]),
        ];
        if t.is_party() {
            let owner = if t.owner == NIL {
                Msg::tr("ftbteams.info.owner.none", "nobody", vec![]).color(c::GRAY)
            } else {
                let id = uuid_string(t.owner);
                Msg::lit(self.display_name_of(t.owner)).styled(|s| s.hover = Some(Box::new(Msg::lit(id))))
            };
            lines.push(fail("ftbteams.info.owner", "Owned by: %s", vec![owner]));
            lines.push(fail("ftbteams.info.members", "People in it:", vec![]));
            let members = t.members();
            if members.is_empty() {
                lines.push(Msg::lit("-").then(Msg::tr("ftbteams.info.members.none", "nobody", vec![]).color(c::GRAY)));
            }
            for m in members {
                lines.push(Msg::lit("-").then(Msg::lit(self.display_name_of(m))));
            }
        }
        lines
    }

    /// `list`: the teams by short name, joined with commas.
    pub fn list(&self, kind: Option<TeamType>) -> Msg {
        let mut teams: Vec<&Team> = self.teams.values().filter(|t| kind.is_none_or(|k| t.kind == k)).collect();
        teams.sort_by_key(|t| t.short_name());
        let mut names = Msg::Seq(Vec::new());
        for (i, t) in teams.iter().enumerate() {
            if i > 0 {
                names = names.then(Msg::lit(","));
            }
            names = names.then(t.name_msg());
        }
        if teams.is_empty() {
            names = Msg::lit("(none)");
        }
        fail("ftbteams.list", "Teams here: %s", vec![names])
    }

    /// `setChatRedirected`; true when it changed.
    pub fn set_chat_redirected(&mut self, player: Uuid, on: bool) -> bool {
        let changed = if on { self.chat_redirected.insert(player) } else { self.chat_redirected.remove(&player) };
        self.dirty |= changed;
        changed
    }

    // ---- Files ----------------------------------------------------------------------------------------------

    /// `TeamManagerImpl.load` from `<world>/ftbteams`: nothing when the folder isn't there. A player's party comes
    /// from the parties' member ranks.
    pub fn load(dir: &Path, seed: u64) -> Self {
        let mut m = Manager::new(seed);
        let root = dir.join("ftbteams");
        if !root.is_dir() {
            return m;
        }
        if let Some(c) = snbt::read_file(&root.join("ftbteams.snbt")) {
            if let Some(id) = c.str("id").and_then(parse_uuid) {
                m.id = id;
            }
            m.extra = c.compound("extra").cloned().unwrap_or_default();
            m.chat_redirected = c.list("chat_redirected").iter().filter_map(|v| v.as_str().and_then(parse_uuid)).collect();
            m.dirty = false;
        }
        for kind in TeamType::ALL {
            let Ok(entries) = fs::read_dir(root.join(kind.name())) else { continue };
            let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "snbt")).collect();
            paths.sort();
            for path in paths {
                if let Some(team) = snbt::read_file(&path).and_then(|c| Team::read(kind, &c)) {
                    if kind == TeamType::Player {
                        m.known.insert(team.id, Known { name: team.player_name.clone(), online: false, effective: team.id });
                    }
                    m.teams.insert(team.id, team);
                }
            }
        }
        let parties: Vec<(Uuid, Vec<Uuid>)> = m.teams.values().filter(|t| t.is_party()).map(|t| (t.id, t.members())).collect();
        for (party, members) in parties {
            for member in members {
                if let Some(k) = m.known.get_mut(&member) {
                    k.effective = party;
                }
            }
        }
        m
    }

    /// `saveNow`: the manager's file when it changed, each changed team's, and deleted teams' files moved to
    /// `deleted/`.
    pub fn save(&mut self, dir: &Path) -> io::Result<()> {
        let root = dir.join("ftbteams");
        if self.dirty {
            let mut c = Compound::default();
            c.put("id", Value::String(uuid_string(self.id)));
            c.put("extra", Value::Compound(self.extra.clone()));
            let redirected = self.chat_redirected.iter().map(|id| Value::String(uuid_string(*id))).collect();
            c.put("chat_redirected", Value::List(redirected));
            snbt::write_file(&root.join("ftbteams.snbt"), &c)?;
            self.dirty = false;
        }
        for t in self.teams.values_mut().filter(|t| t.dirty) {
            snbt::write_file(&root.join(t.kind.name()).join(format!("{}.snbt", uuid_string(t.id))), &t.write())?;
            t.dirty = false;
        }
        for (id, kind) in std::mem::take(&mut self.deleted) {
            let file = format!("{}.snbt", uuid_string(id));
            let from = root.join(kind.name()).join(&file);
            if !from.exists() {
                continue;
            }
            let to = root.join("deleted").join(&file);
            fs::create_dir_all(to.parent().unwrap())?;
            if to.exists() { fs::remove_file(&from)? } else { fs::rename(&from, &to)? }
        }
        self.save_now = false;
        Ok(())
    }
}

pub(crate) fn team_not_found(what: &str) -> Fail {
    fail("ftbteams.team_not_found", "No team matches '%s'", vec![Msg::lit(what)])
}
pub(crate) fn already_in_party() -> Fail {
    fail("ftbteams.already_in_party", "You already belong to a party", vec![])
}
pub(crate) fn not_in_party() -> Fail {
    fail("ftbteams.not_in_party", "You do not belong to any party", vec![])
}
