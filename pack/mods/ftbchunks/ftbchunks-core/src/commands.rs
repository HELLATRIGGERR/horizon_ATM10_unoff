//! `/ftbchunks` (and its alias `/chunks`) with FTB Chunks' syntax and translation keys. The fallback wording is
//! this port's own: a game with the mod's lang file shows the mod's lines.

use crate::{ChunkPos, Chunks, Kind, Privacy, Problem, TeamView, Teams, Uuid, is_ally};

/// A chat component.
#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    Lit(String),
    /// `Component.translatable(key, args)` with this port's fallback wording.
    Tr { key: &'static str, en: &'static str, args: Vec<Msg> },
    Seq(Vec<Msg>),
    Color(Box<Msg>, u32),
}

impl Msg {
    pub fn lit(s: impl Into<String>) -> Self {
        Msg::Lit(s.into())
    }
    pub fn tr(key: &'static str, en: &'static str, args: Vec<Msg>) -> Self {
        Msg::Tr { key, en, args }
    }
    pub fn color(self, rgb: u32) -> Self {
        Msg::Color(Box::new(self), rgb)
    }
    /// The fallback text, `%s` filled (tests).
    pub fn plain(&self) -> String {
        match self {
            Msg::Lit(s) => s.clone(),
            Msg::Tr { en, args, .. } => {
                let mut out = String::new();
                let mut args = args.iter();
                let mut rest = *en;
                while let Some(i) = rest.find("%s") {
                    out.push_str(&rest[..i]);
                    out.push_str(&args.next().map(Msg::plain).unwrap_or_default());
                    rest = &rest[i + 2..];
                }
                out + rest
            }
            Msg::Seq(items) => items.iter().map(Msg::plain).collect(),
            Msg::Color(inner, _) => inner.plain(),
        }
    }
}

pub const GOLD: u32 = 0xFFAA00;
pub const DARK_GREEN: u32 = 0x00AA00;
pub const RED: u32 = 0xFF5555;

/// Who runs the command, and where they stand.
#[derive(Clone, Debug)]
pub struct Ctx {
    pub sender: Uuid,
    pub name: String,
    pub level: u8,
    pub dimension: String,
    pub pos: [f64; 3],
    /// Whether `sender` has `/ftbchunks admin bypass_protection` on.
    pub bypass: bool,
}

/// The answer, and what the game must carry out.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reply {
    pub lines: Vec<Msg>,
    pub failed: bool,
    /// The sender's protection bypass, toggled.
    pub bypass: Option<bool>,
}

impl Reply {
    fn ok(line: Msg) -> Self {
        Reply { lines: vec![line], ..Reply::default() }
    }
    fn err(line: Msg) -> Self {
        Reply { lines: vec![line], failed: true, bypass: None }
    }
}

/// The usages to register, with the level each needs.
pub fn usages() -> Vec<(String, u8)> {
    let fixed: [(&str, u8); 18] = [
        ("claim [radius_in_blocks:integer]", 0),
        ("unclaim [radius_in_blocks:integer]", 0),
        ("load [radius_in_blocks:integer]", 0),
        ("unload [radius_in_blocks:integer]", 0),
        ("unclaim_all", 0),
        ("unclaim_all <team:word>", 2),
        ("unload_all", 0),
        ("unload_all <team:word>", 2),
        ("info", 0),
        ("info <x:integer> <z:integer> [dimension:word]", 0),
        ("admin bypass_protection", 2),
        ("admin unclaim_everything", 2),
        ("admin unclaim_dimension [dimension:word]", 2),
        ("admin unload_everything", 2),
        ("admin claim_as <team:word> [radius_in_blocks:integer]", 2),
        ("admin claim_as <team:word> <radius_in_blocks:integer> <anchor:block_pos> [dimension:word]", 2),
        ("admin unclaim_as <team:word> [radius_in_blocks:integer]", 2),
        ("admin unclaim_as <team:word> <radius_in_blocks:integer> <anchor:block_pos> [dimension:word]", 2),
    ];
    let mut u: Vec<(String, u8)> = fixed.iter().map(|(s, l)| (s.to_string(), *l)).collect();
    for kind in ["extra_claim_chunks", "extra_force_load_chunks"] {
        for tail in ["get", "set <number:integer>", "add <number:integer>"] {
            u.push((format!("admin {kind} <player:player_name> {tail}"), 2));
        }
    }
    ["ftbchunks", "chunks"].iter().flat_map(|root| u.iter().map(move |(rest, level)| (format!("{root} {rest}"), *level))).collect()
}

fn unknown() -> Reply {
    Reply::err(Msg::tr("command.unknown.command", "That command is not valid here or is unfinished", vec![]))
}

fn problem(p: Problem) -> Msg {
    let (key, en) = match p {
        Problem::NotOwner => ("ftbchunks.claim_result.not_owner", "another team holds it"),
        Problem::NotEnoughPower => ("ftbchunks.claim_result.not_enough_power", "the team has no claims left"),
        Problem::AlreadyClaimed => ("ftbchunks.claim_result.already_claimed", "it has an owner already"),
        Problem::DimensionForbidden => ("ftbchunks.claim_result.dimension_forbidden", "claims are not allowed in this dimension"),
        Problem::NotClaimed => ("ftbchunks.claim_result.not_claimed", "nobody holds it"),
        Problem::AlreadyLoaded => ("ftbchunks.claim_result.already_loaded", "it is kept loaded already"),
        Problem::NotLoaded => ("ftbchunks.claim_result.not_loaded", "it is not kept loaded"),
    };
    Msg::tr(key, en, vec![])
}

/// The square of chunks `radius_in_blocks >> 4` around block column `(x, z)`, nearest first.
pub fn area(dimension: &str, x: i32, z: i32, radius_in_blocks: i32) -> Vec<ChunkPos> {
    let (cx, cz, r) = (x >> 4, z >> 4, radius_in_blocks >> 4);
    let mut out: Vec<(i32, ChunkPos)> = Vec::new();
    for dx in -r..=r {
        for dz in -r..=r {
            out.push((dx * dx + dz * dz, (dimension.to_owned(), cx + dx, cz + dz)));
        }
    }
    out.sort_by_key(|(d, _)| *d);
    out.into_iter().map(|(_, p)| p).collect()
}

fn radius(word: Option<&str>) -> Result<i32, Reply> {
    let Some(word) = word else { return Ok(0) };
    let n: i32 = word.parse().map_err(|_| Reply::err(Msg::tr("parsing.int.invalid", "Not a whole number: '%s'", vec![Msg::lit(word)])))?;
    if n < 0 {
        return Err(Reply::err(Msg::tr("argument.integer.low", "The number must be %s or more, not %s", vec![Msg::lit("0"), Msg::lit(word)])));
    }
    if n > 512 {
        return Err(Reply::err(Msg::tr("argument.integer.big", "The number must be %s or less, not %s", vec![Msg::lit("512"), Msg::lit(word)])));
    }
    Ok(n)
}

fn team_not_found(word: &str) -> Reply {
    Reply::err(Msg::tr("ftbteams.team_not_found", "No team matches '%s'", vec![Msg::lit(word)]))
}

fn team_arg(teams: &dyn Teams, ctx: &Ctx, word: &str) -> Result<TeamView, Reply> {
    if word == "@s" || word == "@p" {
        return teams.team_of(ctx.sender).ok_or_else(|| team_not_found(word));
    }
    teams.find(word).ok_or_else(|| team_not_found(word))
}

/// `"<x> <z> in <dimension>"` for `ftbchunks.commands.location`'s argument.
fn location(pos: &ChunkPos) -> Msg {
    Msg::lit(format!("[{}, {}] {}", pos.1, pos.2, pos.0))
}

/// Runs `/ftbchunks ...`; None when the command isn't one of FTB Chunks'.
pub fn perform(chunks: &mut Chunks, teams: &dyn Teams, ctx: &Ctx, command: &str, now: i64) -> Option<Reply> {
    let words: Vec<&str> = command.split_whitespace().collect();
    if !matches!(words.first(), Some(&"ftbchunks") | Some(&"chunks")) {
        return None;
    }
    Some(run(chunks, teams, ctx, &words[1..], now).unwrap_or_else(|r| r))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Op {
    Claim,
    Unclaim,
    Load,
    Unload,
}

/// One operation over an area for `team`: how many chunks it changed.
fn apply(chunks: &mut Chunks, team: &TeamView, op: Op, area: &[ChunkPos], admin: bool, now: i64) -> (usize, Vec<Problem>) {
    let mut done = 0;
    let mut problems = Vec::new();
    for pos in area {
        let r = match op {
            Op::Claim => chunks.claim(team, pos.clone(), now),
            Op::Unclaim => chunks.unclaim(team.id, pos, admin).map(|_| ()),
            Op::Load => chunks.load(team, pos, admin, now),
            Op::Unload => chunks.unload(team.id, pos, admin),
        };
        match r {
            Ok(()) => done += 1,
            Err(p) => problems.push(p),
        }
    }
    (done, problems)
}

fn counted(op: Op, n: usize) -> Msg {
    let n = vec![Msg::lit(n.to_string())];
    match op {
        Op::Claim => Msg::tr("ftbchunks.commands.claimed", "%s chunk(s) claimed", n),
        Op::Unclaim => Msg::tr("ftbchunks.commands.unclaimed", "%s chunk(s) given up", n),
        Op::Load => Msg::tr("ftbchunks.commands.force_loaded", "%s chunk(s) now kept loaded", n),
        // FTB Chunks 2101.1.22 names a key its lang file doesn't have here; it is kept.
        Op::Unload => Msg::tr("ftbchunks.command.unloaded", "%s chunk(s) no longer kept loaded", n),
    }
}

fn run(chunks: &mut Chunks, teams: &dyn Teams, ctx: &Ctx, w: &[&str], now: i64) -> Result<Reply, Reply> {
    let admin = ctx.level >= 2;
    let (bx, bz) = (ctx.pos[0].floor() as i32, ctx.pos[2].floor() as i32);
    let own = || teams.team_of(ctx.sender).ok_or_else(unknown);
    let op = match w.first().copied() {
        Some("claim") => Some(Op::Claim),
        Some("unclaim") => Some(Op::Unclaim),
        Some("load") => Some(Op::Load),
        Some("unload") => Some(Op::Unload),
        _ => None,
    };
    if let Some(op) = op {
        if w.len() > 2 {
            return Err(unknown());
        }
        let r = radius(w.get(1).copied())?;
        let team = own()?;
        chunks.update_limits(&team);
        let (n, _) = apply(chunks, &team, op, &area(&ctx.dimension, bx, bz, r), false, now);
        return Ok(Reply::ok(counted(op, n)));
    }
    match w {
        ["unclaim_all"] | ["unclaim_all", _] | ["unload_all"] | ["unload_all", _] => {
            let team = match w.get(1) {
                Some(word) if admin => team_arg(teams, ctx, word)?,
                Some(_) => return Err(unknown()),
                None => own()?,
            };
            let mine: Vec<ChunkPos> = chunks.claims_of(team.id).map(|(p, _)| p.clone()).collect();
            let op = if w[0] == "unclaim_all" { Op::Unclaim } else { Op::Unload };
            let (n, _) = apply(chunks, &team, op, &mine, false, now);
            Ok(Reply::ok(counted(op, n)))
        }
        ["info"] | ["info", _, _] | ["info", _, _, _] => {
            let (x, z) = match (w.get(1), w.get(2)) {
                (Some(x), Some(z)) => (x.parse::<i32>().map_err(|_| unknown())?, z.parse::<i32>().map_err(|_| unknown())?),
                _ => (bx, bz),
            };
            let dimension = w.get(3).map_or(ctx.dimension.clone(), |d| d.to_string());
            let pos: ChunkPos = (dimension, x >> 4, z >> 4);
            let mut lines = vec![Msg::tr("ftbchunks.commands.location", "Chunk %s", vec![location(&pos)])];
            match chunks.claims.get(&pos).copied() {
                None => lines.push(Msg::tr("ftbchunks.commands.not_claimed", "Nobody has claimed this chunk", vec![])),
                Some(claim) => {
                    let team = teams.team(claim.team);
                    let hidden = team.as_ref().is_some_and(|t| {
                        t.props.claim_visibility != Privacy::Public
                            && !admin
                            && !match t.props.claim_visibility {
                                Privacy::Private => t.is_member(ctx.sender),
                                _ => is_ally(&chunks.config, t, ctx.sender),
                            }
                    });
                    if hidden {
                        lines.push(Msg::tr("ftbchunks.commands.cant_determined", "This chunk's owner is not shown to you", vec![]));
                    } else {
                        let name = team.map_or(Msg::lit(format!("{:032x}", claim.team)), |t| Msg::lit(t.name).color(t.color));
                        lines.push(Msg::Seq(vec![Msg::tr("ftbchunks.commands.owner", "Held by: ", vec![]), name]));
                        let loaded = if claim.force_loaded > 0 { "true" } else { "false" };
                        lines.push(Msg::tr("ftbchunks.commands.is_force_loaded", "Kept loaded: %s", vec![Msg::lit(loaded)]));
                    }
                }
            }
            Ok(Reply { lines, ..Reply::default() })
        }
        ["admin", ..] if !admin => Err(unknown()),
        ["admin", "bypass_protection"] => {
            let on = !ctx.bypass;
            let line = if on {
                Msg::tr("ftbchunks.command.bypass_protection_enabled", "Claims no longer stop you", vec![])
            } else {
                Msg::tr("ftbchunks.command.bypass_protection_disabled", "Claims stop you again", vec![])
            };
            Ok(Reply { lines: vec![line], failed: false, bypass: Some(on) })
        }
        ["admin", "unclaim_everything"] | ["admin", "unload_everything"] => {
            let all: Vec<(ChunkPos, Uuid)> = chunks.claims.iter().map(|(p, c)| (p.clone(), c.team)).collect();
            let total = all.len();
            let mut n = 0;
            for (pos, team) in all {
                let r = if w[1] == "unclaim_everything" { chunks.unclaim(team, &pos, true).map(|_| ()) } else { chunks.unload(team, &pos, true) };
                n += usize::from(r.is_ok());
            }
            Ok(Reply::ok(if w[1] == "unclaim_everything" {
                counted(Op::Unclaim, n)
            } else {
                Msg::tr("ftbchunks.commands.unloaded", "%s of %s claimed chunk(s) no longer kept loaded", vec![Msg::lit(n.to_string()), Msg::lit(total.to_string())])
            }))
        }
        ["admin", "unclaim_dimension"] | ["admin", "unclaim_dimension", _] => {
            let dimension = w.get(2).map_or(ctx.dimension.as_str(), |d| *d);
            let all: Vec<(ChunkPos, Uuid)> = chunks.claims.iter().filter(|(p, _)| p.0 == dimension).map(|(p, c)| (p.clone(), c.team)).collect();
            let n = all.into_iter().filter(|(pos, team)| chunks.unclaim(*team, pos, true).is_ok()).count();
            Ok(Reply::ok(counted(Op::Unclaim, n)))
        }
        ["admin", verb @ ("claim_as" | "unclaim_as"), team, rest @ ..] => {
            let team = team_arg(teams, ctx, team)?;
            let r = radius(rest.first().copied())?;
            let coord = |i: usize, base: f64| -> Result<i32, Reply> {
                let word = rest[i];
                if let Some(rel) = word.strip_prefix('~') {
                    let d: f64 = if rel.is_empty() { 0.0 } else { rel.parse().map_err(|_| unknown())? };
                    Ok((base + d).floor() as i32)
                } else {
                    word.parse().map_err(|_| unknown())
                }
            };
            let (x, z) = match rest.len() {
                0 | 1 => (bx, bz),
                4 | 5 => (coord(1, ctx.pos[0])?, coord(3, ctx.pos[2])?),
                _ => return Err(unknown()),
            };
            let dimension = rest.get(4).map_or(ctx.dimension.as_str(), |d| *d);
            if team.kind != Kind::Server {
                chunks.update_limits(&team);
            }
            let op = if *verb == "claim_as" { Op::Claim } else { Op::Unclaim };
            let (n, _) = apply(chunks, &team, op, &area(dimension, x, z, r), false, now);
            Ok(Reply::ok(counted(op, n)))
        }
        ["admin", kind @ ("extra_claim_chunks" | "extra_force_load_chunks"), player, verb, value @ ..] => {
            let (id, name) = teams
                .player(player)
                .ok_or_else(|| Reply::err(Msg::tr("ftbchunks.command.no_personal_info", "No personal team found for %s", vec![Msg::lit(*player)])))?;
            let claims = *kind == "extra_claim_chunks";
            let current = chunks.teams.get(&id).map_or(0, |d| if claims { d.extra_claims } else { d.extra_force_loads });
            let shown = |n: i32| {
                let args = vec![Msg::lit(name.clone()), Msg::lit(n.to_string())];
                if claims {
                    Msg::tr("ftbchunks.command.extra_chunks_claimed", "%s: %s extra claim(s)", args)
                } else {
                    Msg::tr("ftbchunks.command.extra_forceload_chunks", "%s: %s extra kept-loaded chunk(s)", args)
                }
            };
            let n = match (*verb, value) {
                ("get", []) => return Ok(Reply::ok(shown(current))),
                ("set", [v]) => v.parse::<i32>().map_err(|_| unknown())?,
                ("add", [v]) => current + v.parse::<i32>().map_err(|_| unknown())?,
                _ => return Err(unknown()),
            }
            .max(0);
            let personal = teams.team(id).ok_or_else(|| Reply::err(Msg::tr("ftbchunks.command.no_personal_info", "No personal team found for %s", vec![Msg::lit(name.clone())])))?;
            let data = chunks.teams.entry(id).or_default();
            if claims {
                data.extra_claims = n;
            } else {
                data.extra_force_loads = n;
            }
            chunks.dirty.insert(id);
            chunks.update_limits(&personal);
            if let Some(team) = teams.team_of(id).filter(|t| t.id != id) {
                chunks.update_limits(&team);
            }
            let args = vec![Msg::lit(n.to_string()), Msg::lit(name)];
            Ok(Reply::ok(if claims {
                Msg::tr("ftbchunks.command.set_extra_chunks_claimed", "Extra claims now %s for %s", args)
            } else {
                Msg::tr("ftbchunks.command.set_extra_forceload_chunks", "Extra kept-loaded chunks now %s for %s", args)
            }))
        }
        _ => Err(unknown()),
    }
}

/// The action-bar line for a refusal, gold.
pub fn refusal(r: crate::Refusal) -> Msg {
    match r {
        crate::Refusal::NeedClaim => Msg::tr("ftbchunks.need_to_claim_chunk", "Claim this chunk before you do that here", vec![]),
        crate::Refusal::Prevented => Msg::tr("ftbchunks.action_prevented", "This land is claimed: you may not do that here", vec![]),
    }
    .color(GOLD)
}

/// A problem's line, for callers that report one (the claim screen's summary, slice 3).
pub fn problem_line(p: Problem) -> Msg {
    problem(p).color(RED)
}
