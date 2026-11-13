//! `/ftbteams` (`FTBTeamsCommands`): its usages, who may run each, and what each answers.

use crate::{
    NIL, Uuid,
    ftblib::{offline_uuid, parse_uuid, uuid_string},
    manager::{Manager, fail, not_in_party, team_not_found},
    msg::{Msg, color as c},
    team::{Prop, Rank, TeamType},
};

/// Who runs a command.
#[derive(Clone, Debug)]
pub struct Sender {
    pub uuid: Uuid,
    pub name: String,
    /// The operator level, 0 to 4.
    pub level: u8,
    /// A player, not the console.
    pub player: bool,
}

pub struct Ctx<'a> {
    pub sender: Sender,
    /// `server.isSingleplayer()`: the admin usages need no operator level then.
    pub singleplayer: bool,
    /// A player the server knows by name (online, or in its profile cache).
    pub lookup: &'a dyn Fn(&str) -> Option<(Uuid, String)>,
    /// Java's development environment: `/ftbteams_add_fake_player` is there.
    pub dev: bool,
}

/// The lines a command answers its sender with; `failed` makes the last one an error, in red.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reply {
    pub lines: Vec<Msg>,
    pub failed: bool,
}

impl Reply {
    fn ok(lines: Vec<Msg>) -> Self {
        Reply { lines, failed: false }
    }
    fn err(msg: Msg) -> Self {
        Reply { lines: vec![msg], failed: true }
    }
}

/// The usages, as the game's command tree takes them, with the operator level each needs.
pub fn usages(dev: bool) -> Vec<(&'static str, u8)> {
    let mut u = vec![
        ("ftbteams party create [name:text]", 0),
        ("ftbteams party join <team:word>", 0),
        ("ftbteams party decline <team:word>", 0),
        ("ftbteams party leave", 0),
        ("ftbteams party invite <players:player_name>", 0),
        ("ftbteams party kick <players:player_name>", 0),
        ("ftbteams party transfer_ownership <player_id:player_name>", 0),
        ("ftbteams party transfer_ownership_for <team:word> <player_id:player_name>", 2),
        ("ftbteams party settings <key:word> [value:text]", 0),
        ("ftbteams party settings_for <team:word> <key:word> [value:text]", 2),
        ("ftbteams party allies add <player:player_name>", 0),
        ("ftbteams party allies remove <player:player_name>", 0),
        ("ftbteams party allies list", 0),
        ("ftbteams server create <name:text>", 2),
        ("ftbteams server delete <team:word>", 2),
        ("ftbteams server settings <team:word> <key:word> [value:text]", 2),
        ("ftbteams msg <text:text>", 0),
        ("ftbteams info server_id", 0),
        ("ftbteams info [team:word]", 0),
        ("ftbteams list [kind:word]", 0),
        ("ftbteams force-disband <team:word>", 2),
        ("ftbteams redirect_chat", 0),
        ("ftbteams force-add <team:word> <players:player_name>", 2),
        ("ftbteams force-remove <team:word> <players:player_name>", 2),
        ("ftbteams teamstage <player:player_name> add <stage:word>", 2),
        ("ftbteams teamstage <player:player_name> remove <stage:word>", 2),
        ("ftbteams teamstage <player:player_name> list", 2),
    ];
    if dev {
        u.push(("ftbteams_add_fake_player <profile:player_name>", 2));
    }
    u
}

/// The line's words, each with the rest of the line from it (a `text` argument).
struct Words<'a> {
    words: Vec<(&'a str, &'a str)>,
}

impl<'a> Words<'a> {
    fn new(line: &'a str) -> Self {
        let mut words = Vec::new();
        let mut rest = line.trim_start();
        while !rest.is_empty() {
            let end = rest.find(' ').unwrap_or(rest.len());
            words.push((&rest[..end], rest));
            rest = rest[end..].trim_start_matches(' ');
        }
        Words { words }
    }
    fn get(&self, i: usize) -> Option<&'a str> {
        self.words.get(i).map(|w| w.0)
    }
    fn rest(&self, i: usize) -> &'a str {
        self.words.get(i).map_or("", |w| w.1)
    }
}

fn unknown() -> Reply {
    Reply::err(Msg::tr("command.unknown.command", "That command is not valid here or is unfinished", vec![]))
}

/// `/ftbteams ...` and `/ftbteams_add_fake_player`; None when the line is not one of them.
pub fn perform(m: &mut Manager, ctx: &Ctx<'_>, line: &str) -> Option<Reply> {
    let w = Words::new(line.strip_prefix('/').unwrap_or(line));
    match w.get(0)? {
        "ftbteams" => Some(run(m, ctx, &w).unwrap_or_else(Reply::err)),
        "ftbteams_add_fake_player" if ctx.dev => {
            let Some(name) = w.get(1) else { return Some(unknown()) };
            let (id, name) = (ctx.lookup)(name).unwrap_or_else(|| (offline_uuid(name), name.to_owned()));
            m.player_logged_in(id, &name, false);
            Some(Reply::ok(vec![]))
        }
        _ => None,
    }
}

type Res<T = Reply> = Result<T, Msg>;

/// A player argument (`GameProfileArgument`): `@s`, else a name the manager or the server knows.
fn profile(m: &Manager, ctx: &Ctx<'_>, word: Option<&str>) -> Res<(Uuid, String)> {
    let word = word.ok_or_else(|| unknown().lines.remove(0))?;
    if word == "@s" || word == "@p" {
        return Ok((ctx.sender.uuid, ctx.sender.name.clone()));
    }
    if let Some(id) = parse_uuid(word) {
        return Ok((id, m.name_of(id).unwrap_or(word).to_owned()));
    }
    m.known_by_name(word)
        .or_else(|| (ctx.lookup)(word))
        .ok_or_else(|| Msg::tr("argument.player.unknown", "No such player is known", vec![]))
}

/// A team argument (`TeamArgument`): a selector's team, a short name, else a player's name and their team.
fn team_arg(m: &Manager, ctx: &Ctx<'_>, word: Option<&str>, kind: Option<TeamType>) -> Res<Uuid> {
    let word = word.ok_or_else(|| unknown().lines.remove(0))?;
    let found = if word.starts_with('@') {
        if word != "@s" && word != "@p" {
            return Err(team_not_found(word));
        }
        m.team_of(ctx.sender.uuid).map(|t| t.id).ok_or_else(|| team_not_found(&ctx.sender.name))?
    } else if let Some(t) = m.team_by_short_name(word) {
        t.id
    } else {
        let (id, _) = m.known_by_name(word).or_else(|| (ctx.lookup)(word)).ok_or_else(|| team_not_found(word))?;
        m.team_of(id).map(|t| t.id).ok_or_else(|| team_not_found(word))?
    };
    // The argument's template: a team of another type isn't one.
    match kind {
        Some(k) if m.teams[&found].kind != k => Err(team_not_found(word)),
        _ => Ok(found),
    }
}

/// `partyTeamArg`: a party, and the sender's rank in it.
fn party_arg(m: &Manager, ctx: &Ctx<'_>, word: Option<&str>, rank: Rank) -> Res<Uuid> {
    let id = team_arg(m, ctx, word, None)?;
    let t = &m.teams[&id];
    if !t.is_party() {
        return Err(team_not_found(&t.props.display_name));
    }
    if rank != Rank::None && !t.rank_of(ctx.sender.uuid).is_at_least(rank) {
        return Err(fail("ftbteams.not_invited", "No invitation from %s is waiting for you", vec![t.name_msg()]));
    }
    Ok(id)
}

/// `getPartyTeam`: the sender's own party, and their rank in it.
fn own_party(m: &Manager, ctx: &Ctx<'_>, rank: Rank) -> Res<Uuid> {
    let t = m.team_of(ctx.sender.uuid).filter(|t| t.is_party()).ok_or_else(not_in_party)?;
    if !t.rank_of(ctx.sender.uuid).is_at_least(rank) {
        return Err(fail("ftbteams.cant_edit", "You may not change %s", vec![t.name_msg()]));
    }
    Ok(t.id)
}

fn prop_arg(word: Option<&str>) -> Res<Prop> {
    word.and_then(Prop::from_id).ok_or_else(|| Msg::tr("ftbteams.property_not_found", "No property has that id", vec![]))
}

fn run(m: &mut Manager, ctx: &Ctx<'_>, w: &Words<'_>) -> Res {
    let s = &ctx.sender;
    let me = s.uuid;
    // The nodes' `requires`: unmet, the dispatcher doesn't know the command.
    let no_party = s.player && m.team_of(me).is_some_and(|t| t.kind == TeamType::Player);
    let in_party = |m: &Manager, rank: Rank| {
        s.player && m.team_of(me).is_some_and(|t| t.is_party() && t.rank_of(me).is_at_least(rank))
    };
    let admin = ctx.singleplayer || s.level >= 2;
    let sender_or_nil = if s.player { me } else { NIL };
    match (w.get(1), w.get(2)) {
        (Some("party"), Some("create")) if no_party => {
            m.create_party(me, w.rest(3), None, None)?;
            Ok(Reply::ok(vec![]))
        }
        (Some("party"), Some("join")) if no_party => {
            let party = party_arg(m, ctx, w.get(3), Rank::Invited)?;
            m.join(party, me)?;
            Ok(Reply::ok(vec![]))
        }
        (Some("party"), Some("decline")) if no_party => {
            let party = party_arg(m, ctx, w.get(3), Rank::Invited)?;
            Ok(Reply::ok(m.decline(party, me).into_iter().collect()))
        }
        (Some("party"), Some("leave")) if in_party(m, Rank::Member) => {
            let party = own_party(m, ctx, Rank::Member)?;
            m.leave(party, me)?;
            Ok(Reply::ok(vec![]))
        }
        (Some("party"), Some("invite")) if in_party(m, Rank::Officer) => {
            let party = own_party(m, ctx, Rank::Officer)?;
            let who = profile(m, ctx, w.get(3))?;
            m.invite(party, me, &[who])?;
            Ok(Reply::ok(vec![]))
        }
        (Some("party"), Some("kick")) if in_party(m, Rank::Officer) => {
            let party = own_party(m, ctx, Rank::Officer)?;
            let who = profile(m, ctx, w.get(3))?;
            m.kick(party, me, &[who])?;
            Ok(Reply::ok(vec![]))
        }
        (Some("party"), Some("transfer_ownership")) if in_party(m, Rank::Owner) => {
            let party = own_party(m, ctx, Rank::Owner)?;
            let to = profile(m, ctx, w.get(3))?;
            Ok(Reply::ok(m.transfer(party, me, to)?.into_iter().collect()))
        }
        (Some("party"), Some("transfer_ownership_for")) if admin => {
            let party = party_arg(m, ctx, w.get(3), Rank::None)?;
            let to = profile(m, ctx, w.get(4))?;
            let mut lines: Vec<Msg> = m.transfer(party, sender_or_nil, to.clone())?.into_iter().collect();
            if !s.player && lines.is_empty() {
                lines.push(fail("ftbteams.message.transfer_owner", "%s now owns the team", vec![Msg::lit(to.1).color(c::YELLOW)]).color(c::GREEN));
            }
            Ok(Reply::ok(lines))
        }
        (Some("party"), Some("settings")) if in_party(m, Rank::Owner) => {
            let party = own_party(m, ctx, Rank::Owner)?;
            let prop = prop_arg(w.get(3))?;
            Ok(Reply::ok(vec![m.settings(party, prop, w.rest(4))?]))
        }
        (Some("party"), Some("settings_for")) if admin => {
            let party = party_arg(m, ctx, w.get(3), Rank::None)?;
            let prop = prop_arg(w.get(4))?;
            Ok(Reply::ok(vec![m.settings(party, prop, w.rest(5))?]))
        }
        (Some("party"), Some("allies")) if in_party(m, Rank::Member) => match w.get(3) {
            Some("add") if in_party(m, Rank::Officer) => {
                let party = own_party(m, ctx, Rank::Officer)?;
                let who = profile(m, ctx, w.get(4))?;
                m.add_allies(party, sender_or_nil, &[who]);
                Ok(Reply::ok(vec![]))
            }
            Some("remove") if in_party(m, Rank::Officer) => {
                let party = own_party(m, ctx, Rank::Officer)?;
                let who = profile(m, ctx, w.get(4))?;
                m.remove_allies(party, sender_or_nil, &[who]);
                Ok(Reply::ok(vec![]))
            }
            Some("list") => {
                let party = own_party(m, ctx, Rank::Member)?;
                Ok(Reply::ok(m.list_allies(party)))
            }
            _ => Ok(unknown()),
        },
        (Some("server"), Some("create")) if admin => Ok(Reply::ok(vec![m.create_server_team(sender_or_nil, w.rest(3))?])),
        (Some("server"), Some("delete")) if admin => {
            let team = team_arg(m, ctx, w.get(3), Some(TeamType::Server))?;
            Ok(Reply::ok(vec![m.delete_server_team(team)]))
        }
        (Some("server"), Some("settings")) if admin => {
            let team = team_arg(m, ctx, w.get(3), Some(TeamType::Server))?;
            let prop = prop_arg(w.get(4))?;
            Ok(Reply::ok(vec![m.settings(team, prop, w.rest(5))?]))
        }
        (Some("msg"), Some(_)) => {
            let team = m.team_of(me).map(|t| t.id).ok_or_else(|| team_not_found(&s.name))?;
            m.send_message(team, me, Msg::lit(w.rest(2)));
            Ok(Reply::ok(vec![]))
        }
        (Some("info"), Some("server_id")) => {
            let id = uuid_string(m.id);
            let copy = Msg::lit(id.clone()).styled(|st| st.copy = Some(id));
            Ok(Reply::ok(vec![Msg::lit("Id of this server: ").then(copy).color(c::YELLOW)]))
        }
        (Some("info"), word) => {
            let team = match word {
                Some(_) => team_arg(m, ctx, word, None)?,
                None => m.team_of(me).map(|t| t.id).ok_or_else(|| team_not_found(&s.name))?,
            };
            Ok(Reply::ok(m.info(team)))
        }
        (Some("list"), kind) => {
            let kind = match kind {
                None => None,
                Some("parties") => Some(TeamType::Party),
                Some("server_teams") => Some(TeamType::Server),
                Some("players") => Some(TeamType::Player),
                Some(_) => return Ok(unknown()),
            };
            Ok(Reply::ok(vec![m.list(kind)]))
        }
        (Some("force-disband"), Some(_)) if s.level >= 2 => {
            let party = party_arg(m, ctx, w.get(2), Rank::None)?;
            Ok(Reply::ok(vec![m.force_disband(party, sender_or_nil)?]))
        }
        (Some("redirect_chat"), None) if s.player => {
            let on = !m.chat_redirected.contains(&me);
            m.set_chat_redirected(me, on);
            let line = if on {
                Msg::tr("ftbteams.message.chat_redirected.on", "Your chat now goes to the team", vec![])
            } else {
                Msg::tr("ftbteams.message.chat_redirected.off", "Your chat now goes to everyone", vec![])
            };
            Ok(Reply::ok(vec![line.styled(|st| st.italic = true).color(c::GOLD)]))
        }
        (Some("force-add"), Some(_)) if admin => {
            let party = party_arg(m, ctx, w.get(2), Rank::None)?;
            let who = profile(m, ctx, w.get(3))?;
            m.join(party, who.0)?;
            Ok(Reply::ok(vec![]))
        }
        (Some("force-remove"), Some(_)) if admin => {
            let party = party_arg(m, ctx, w.get(2), Rank::None)?;
            let who = profile(m, ctx, w.get(3))?;
            m.kick(party, sender_or_nil, &[who])?;
            Ok(Reply::ok(vec![]))
        }
        (Some("teamstage"), Some(_)) if admin => teamstage(m, ctx, w),
        _ => Ok(unknown()),
    }
}

fn teamstage(m: &mut Manager, ctx: &Ctx<'_>, w: &Words<'_>) -> Res {
    let (player, name) = profile(m, ctx, w.get(2))?;
    let team = m.team_of(player).map(|t| t.id).ok_or_else(|| team_not_found(&name))?;
    let stage = w.get(4).unwrap_or_default().to_owned();
    let t = m.teams.get_mut(&team).unwrap();
    let line = match w.get(3) {
        Some("add") if !stage.is_empty() => {
            if t.props.team_stages.insert(stage.clone()) {
                t.dirty = true;
            }
            fail("ftbteams.message.added_stage", "Stage %s joined the team", vec![Msg::lit(stage)])
        }
        Some("remove") if !stage.is_empty() => {
            if t.props.team_stages.remove(&stage) {
                t.dirty = true;
            }
            fail("ftbteams.message.removed_stage", "Stage %s left the team", vec![Msg::lit(stage)])
        }
        Some("list") if t.props.team_stages.is_empty() => {
            return Ok(Reply::ok(vec![fail("ftbteams.message.no_team_stages", "The team has no stages", vec![]).color(c::GOLD)]));
        }
        Some("list") => {
            let count = Msg::lit(t.props.team_stages.len().to_string());
            let mut lines = vec![fail("ftbteams.message.team_stages_header", "Team stages (%s):", vec![count]).color(c::YELLOW)];
            lines.extend(t.props.team_stages.iter().map(|s| Msg::lit(format!("\u{2022}{s}"))));
            return Ok(Reply::ok(lines));
        }
        _ => return Ok(unknown()),
    };
    m.out.push(crate::Out::Sync(vec![team]));
    Ok(Reply::ok(vec![line]))
}
