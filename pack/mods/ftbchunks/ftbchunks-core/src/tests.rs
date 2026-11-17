use super::*;
use crate::commands::{Ctx, perform};

const A: Uuid = 1;
const B: Uuid = 2;
const C: Uuid = 3;
const P: Uuid = 100;

struct World {
    teams: Vec<TeamView>,
    online: Vec<Uuid>,
}

impl Teams for World {
    fn team(&self, id: Uuid) -> Option<TeamView> {
        self.teams.iter().find(|t| t.id == id).cloned()
    }
    fn team_of(&self, player: Uuid) -> Option<TeamView> {
        self.teams.iter().find(|t| t.kind == Kind::Party && t.is_member(player)).or_else(|| self.teams.iter().find(|t| t.id == player)).cloned()
    }
    fn find(&self, arg: &str) -> Option<TeamView> {
        self.teams.iter().find(|t| t.name == arg).cloned()
    }
    fn player(&self, name: &str) -> Option<(Uuid, String)> {
        self.teams.iter().find(|t| t.kind == Kind::Player && t.name == name).map(|t| (t.id, t.name.clone()))
    }
    fn online(&self, player: Uuid) -> bool {
        self.online.contains(&player)
    }
}

fn personal(id: Uuid, name: &str) -> TeamView {
    TeamView { id, kind: Kind::Player, owner: id, name: name.into(), color: 0xFFFFFF, members: vec![id], allies: vec![], props: Props::default() }
}

/// A, B and C with their personal teams; party P owned by A with member B and ally C.
fn world() -> World {
    let party = TeamView { id: P, kind: Kind::Party, owner: A, name: "P".into(), color: 0x00FF00, members: vec![A, B], allies: vec![C], props: Props::default() };
    World { teams: vec![personal(A, "A"), personal(B, "B"), personal(C, "C"), party], online: vec![A] }
}

fn ctx(sender: Uuid, level: u8, x: f64, z: f64) -> Ctx {
    Ctx { sender, name: format!("{sender}"), level, dimension: "minecraft:overworld".into(), pos: [x, 64.0, z], bypass: false }
}

fn run(chunks: &mut Chunks, w: &World, c: &Ctx, command: &str) -> String {
    let reply = perform(chunks, w, c, command, 1000).expect("ours");
    reply.lines.iter().map(|l| l.plain()).collect::<Vec<_>>().join(" | ")
}

fn at(x: i32, z: i32) -> ChunkPos {
    ("minecraft:overworld".into(), x, z)
}

#[test]
fn party_limits_follow_the_mode_and_caps() {
    let w = world();
    let party = w.team(P).unwrap();
    let mut chunks = Chunks::default();
    chunks.teams.entry(A).or_default().extra_claims = 100;
    chunks.teams.entry(B).or_default().extra_force_loads = 3;
    chunks.update_limits(&party);
    assert_eq!(chunks.limits(P), (600, 28), "largest of each");
    chunks.config.party_limit_mode = PartyLimitMode::Sum;
    chunks.config.hard_team_claim_limit = 1050;
    chunks.update_limits(&party);
    assert_eq!(chunks.limits(P), (1050, 53));
    chunks.config.party_limit_mode = PartyLimitMode::Average;
    chunks.update_limits(&party);
    assert_eq!(chunks.limits(P).1, 26);
    chunks.config.party_limit_mode = PartyLimitMode::Owner;
    chunks.update_limits(&party);
    assert_eq!(chunks.limits(P), (600, 25));
    assert_eq!(Chunks::default().limits(C), (500, 25), "a team with no data: the config's");
}

#[test]
fn allies_and_privacy() {
    let w = world();
    let party = w.team(P).unwrap();
    let config = Config::default();
    assert!(is_ally(&config, &party, B) && is_ally(&config, &party, C) && !is_ally(&config, &party, 4));
    assert!(can_use(&config, &party, C, Privacy::Allies) && !can_use(&config, &party, C, Privacy::Private));
    assert!(can_use(&config, &party, 4, Privacy::Public));
    let forced = Config { ally_mode: AllyMode::ForcedNone, ..Config::default() };
    assert!(!is_ally(&forced, &party, C) && is_ally(&forced, &party, B));
    assert!(can_fake_use(&config, &party, B, "[Miner]", Privacy::Allies), "a machine acting with a member's id");
    assert!(!can_fake_use(&config, &party, 9, "[Miner]", Privacy::Allies));
}

#[test]
fn claim_rules_and_force_load_limits() {
    let w = world();
    let party = w.team(P).unwrap();
    let mut chunks = Chunks { config: Config { claim_dimension_blacklist: vec!["minecraft:the_*".into()], ..Config::default() }, ..Chunks::default() };
    chunks.teams.insert(P, TeamData { max_claims: 2, max_force_loads: 1, ..TeamData::default() });
    assert_eq!(chunks.claim(&party, at(0, 0), 5), Ok(()));
    assert_eq!(chunks.claim(&party, at(0, 0), 5), Err(Problem::AlreadyClaimed));
    assert_eq!(chunks.claim(&party, at(1, 0), 5), Ok(()));
    assert_eq!(chunks.claim(&party, at(2, 0), 5), Err(Problem::NotEnoughPower));
    assert_eq!(chunks.claim(&party, ("minecraft:the_end".into(), 0, 0), 5), Err(Problem::DimensionForbidden));
    assert_eq!(chunks.load(&party, &at(0, 0), false, 6), Ok(()));
    assert_eq!(chunks.load(&party, &at(0, 0), false, 6), Err(Problem::AlreadyLoaded));
    assert_eq!(chunks.load(&party, &at(1, 0), false, 6), Err(Problem::NotEnoughPower));
    assert_eq!(chunks.unclaim(A, &at(0, 0), false), Err(Problem::NotOwner));
    assert_eq!(chunks.unload(P, &at(1, 0), false), Err(Problem::NotLoaded));
    assert!(chunks.unclaim(A, &at(0, 0), true).is_ok(), "an admin's override");
    assert_eq!(chunks.claimed_by(P), 1);
    assert!(chunks.dirty.contains(&P));
}

#[test]
fn protection_follows_the_team_properties() {
    let mut w = world();
    let mut chunks = Chunks::default();
    let party = w.team(P).unwrap();
    chunks.claim(&party, at(0, 0), 1).unwrap();
    let actor = |uuid| Actor { uuid, name: String::new(), fake: false, spectator: false, bypass: false };
    let here = at(0, 0);
    assert_eq!(chunks.refuses(&w, &actor(4), &here, Protection::EditBlock, false), Some(Refusal::Prevented));
    assert_eq!(chunks.refuses(&w, &actor(B), &here, Protection::EditBlock, false), None);
    assert_eq!(chunks.refuses(&w, &actor(C), &here, Protection::InteractBlock, false), None, "an ally");
    assert_eq!(chunks.refuses(&w, &actor(4), &here, Protection::InteractBlock, true), None, "interact_whitelist");
    assert_eq!(chunks.refuses(&w, &actor(4), &at(5, 5), Protection::EditBlock, false), None, "the wilderness");
    assert_eq!(chunks.refuses(&w, &Actor { bypass: true, ..actor(4) }, &here, Protection::EditBlock, false), None);
    assert!(chunks.refuses_item(&w, &actor(4), &here, false, true), "a bucket in someone's claim");
    assert!(!chunks.refuses_item(&w, &actor(4), &here, false, false));
    assert!(!chunks.refuses_item(&w, &actor(4), &at(5, 5), false, true));
    // Explosions: blocks in the claim stay unless the team allows them.
    let mut blocks = vec![[1, 64, 1], [17, 64, 1]];
    chunks.filter_explosion(&w, "minecraft:overworld", true, &mut blocks);
    assert_eq!(blocks, vec![[17, 64, 1]]);
    w.teams.iter_mut().find(|t| t.id == P).unwrap().props.set("ftbchunks:allow_explosions", "true");
    let mut blocks = vec![[1, 64, 1]];
    chunks.filter_explosion(&w, "minecraft:overworld", true, &mut blocks);
    assert_eq!(blocks.len(), 1);
    // Pistons may not push into the claim from outside; within it they may.
    assert!(chunks.refuses_crossing(&w, "minecraft:overworld", [16, 64, 1], [15, 64, 1]));
    assert!(!chunks.refuses_crossing(&w, "minecraft:overworld", [2, 64, 1], [3, 64, 1]));
    w.teams.iter_mut().find(|t| t.id == P).unwrap().props.set("ftbchunks:block_edit_mode", "public");
    assert_eq!(chunks.refuses(&w, &actor(4), &here, Protection::EditBlock, false), None);
    let mut no_wild = Chunks { config: Config { no_wilderness: true, ..Config::default() }, ..Chunks::default() };
    assert_eq!(no_wild.refuses(&w, &actor(4), &at(9, 9), Protection::EditBlock, false), Some(Refusal::NeedClaim));
    no_wild.config.set("disable_protection", "true");
    assert_eq!(no_wild.refuses(&w, &actor(4), &at(9, 9), Protection::EditBlock, false), None);
}

fn props(f: impl FnOnce(&mut Props)) -> Props {
    let mut p = Props::default();
    f(&mut p);
    p
}

#[test]
fn pvp_follows_the_mode_and_the_claims_team() {
    let mut w = world();
    let mut chunks = Chunks::default();
    chunks.claims.insert(at(0, 0), Claim { team: P, time: 1, force_loaded: 0, expiry: 0 });
    assert!(!chunks.refuses_pvp(&w, &at(0, 0), &at(5, 5)), "always: nothing changes");
    chunks.config.pvp_mode = PvpMode::Never;
    assert!(chunks.refuses_pvp(&w, &at(5, 5), &at(0, 0)), "never: the victim in a claim");
    assert!(chunks.refuses_pvp(&w, &at(0, 0), &at(5, 5)), "never: the attacker in a claim");
    assert!(!chunks.refuses_pvp(&w, &at(5, 5), &at(6, 6)), "never: out in the wild");
    chunks.config.pvp_mode = PvpMode::PerTeam;
    assert!(!chunks.refuses_pvp(&w, &at(5, 5), &at(0, 0)), "per team: the default allows it");
    w.teams.iter_mut().find(|t| t.id == P).unwrap().props = props(|p| p.allow_pvp = false);
    assert!(chunks.refuses_pvp(&w, &at(5, 5), &at(0, 0)), "per team: the team turned it off");
    chunks.config.disable_protection = true;
    assert!(!chunks.refuses_pvp(&w, &at(5, 5), &at(0, 0)), "no protection at all");
}

#[test]
fn fire_and_fluids_stay_in_their_claim_when_asked() {
    let mut w = world();
    let mut chunks = Chunks::default();
    chunks.claims.insert(at(1, 0), Claim { team: P, time: 1, force_loaded: 0, expiry: 0 });
    let (from, to) = ([15, 64, 3], [16, 64, 3]);
    assert!(!chunks.refuses_spread(&w, "minecraft:overworld", true, from, to), "fire: off by default");
    assert!(!chunks.refuses_spread(&w, "minecraft:overworld", false, from, to), "fluid: off by default");
    chunks.config.fire_spread_protection = true;
    assert!(chunks.refuses_spread(&w, "minecraft:overworld", true, from, to), "fire into a claim");
    assert!(!chunks.refuses_spread(&w, "minecraft:overworld", false, from, to), "the fluid switch is its own");
    assert!(!chunks.refuses_spread(&w, "minecraft:overworld", true, [16, 64, 3], [16, 65, 3]), "inside one chunk");
    assert!(!chunks.refuses_spread(&w, "minecraft:overworld", true, [16, 64, 3], [15, 64, 3]), "out of a claim into the wild");
    chunks.config.flowing_fluid_protection = true;
    assert!(chunks.refuses_spread(&w, "minecraft:overworld", false, from, to));
    w.teams.iter_mut().find(|t| t.id == P).unwrap().props = props(|p| p.block_edit_mode = Privacy::Public);
    assert!(!chunks.refuses_spread(&w, "minecraft:overworld", false, from, to), "a claim anyone may edit");
}

#[test]
fn expiry_lets_go_of_force_loads_and_idle_teams() {
    let mut w = world();
    let mut chunks = Chunks::default();
    let day = 86_400_000;
    for (i, team) in [P, P, A].into_iter().enumerate() {
        chunks.claims.insert(at(i as i32, 0), Claim { team, time: 1, force_loaded: 5, expiry: 0 });
    }
    chunks.claims.get_mut(&at(0, 0)).unwrap().expiry = 1_000;
    chunks.claims.get_mut(&at(1, 0)).unwrap().expiry = 10_000;
    // A force-load's expiry time works whatever the config says.
    let out = chunks.expire(&w, 5_000);
    assert_eq!(out.expired_loads, [(P, at(0, 0))]);
    assert_eq!(chunks.claims[&at(0, 0)].force_loaded, 0);
    assert_eq!(chunks.claims[&at(1, 0)].force_loaded, 5, "not due yet");
    assert!(chunks.expire(&w, 5_000).is_empty(), "nothing twice");
    // Idle days are off by default: nobody loses anything however long they were away.
    chunks.teams.entry(P).or_default().last_login = 1;
    chunks.claims.get_mut(&at(1, 0)).unwrap().expiry = 0;
    assert!(chunks.expire(&w, 400 * day).is_empty() && chunks.claims.len() == 3);
    // Unforce after 2 days, unclaim after 5; the party's members are away (A is on, but only the party counts).
    chunks.config.max_idle_days_before_unforce = 2.0;
    chunks.config.max_idle_days_before_unclaim = 5.0;
    w.online = vec![C];
    let t0 = 10 * day;
    chunks.teams.entry(P).or_default().last_login = t0;
    assert!(chunks.expire(&w, t0 + day).is_empty(), "one day away");
    let out = chunks.expire(&w, t0 + 3 * day);
    assert_eq!((out.idle_unforced, out.idle_unclaimed), (vec![(P, 1)], vec![]), "three days: the force-loads go");
    assert_eq!(chunks.claims[&at(1, 0)].force_loaded, 0);
    assert_eq!(chunks.claims_of(P).count(), 2, "the claims stay");
    let out = chunks.expire(&w, t0 + 6 * day);
    assert_eq!(out.idle_unclaimed, [(P, 2)]);
    assert_eq!(chunks.claims_of(P).count(), 0);
    assert_eq!(chunks.claims_of(A).count(), 1, "A never logged in under the clock: it starts then, nothing goes");
    // Somebody on: the clock restarts and nothing goes; a team never seen start its clock now.
    w.online = vec![A];
    assert!(chunks.expire(&w, t0 + 60 * day).is_empty());
    assert_eq!(chunks.teams[&A].last_login, t0 + 60 * day);
}

#[test]
fn the_properties_ftbchunks_adds_are_all_typed_with_the_configs_defaults() {
    let mut defaults = Props::default();
    assert!(defaults.set("ftbchunks:allow_pvp", "false") && defaults.set("ftbchunks:claim_visibility", "private"));
    let specs = defaults.specs();
    assert_eq!(specs.iter().map(|s| s.id).collect::<Vec<_>>(), PROPERTY_IDS);
    let by = |id: &str| specs.iter().find(|s| s.id == id).unwrap();
    assert_eq!((by("ftbchunks:allow_pvp").kind, by("ftbchunks:allow_pvp").default.as_str()), (SpecKind::Bool, "false"));
    assert_eq!((by("ftbchunks:claim_visibility").kind, by("ftbchunks:claim_visibility").default.as_str()), (SpecKind::Privacy, "private"));
    assert_eq!(by("ftbchunks:allow_named_fake_players").kind, SpecKind::List);
    assert_eq!(by("ftbchunks:allow_explosions").default, "false");
    assert_eq!(by("ftbchunks:block_edit_mode").name_key, "ftbteamsconfig.ftbchunks.block_edit_mode");
    assert!(specs.iter().all(|s| s.id.starts_with("ftbchunks:") && !s.name_en.is_empty()));
    // What the specs show reads back through `set`.
    let mut copy = Props::default();
    for s in &specs {
        assert!(copy.set(s.id, &s.default), "{}", s.id);
    }
    assert_eq!(copy.claim_visibility, Privacy::Private);
}

#[test]
fn tickets_while_a_member_is_on() {
    let mut w = world();
    let mut chunks = Chunks::default();
    let party = w.team(P).unwrap();
    chunks.claim(&party, at(0, 0), 1).unwrap();
    chunks.load(&party, &at(0, 0), false, 2).unwrap();
    assert_eq!(chunks.wanted_tickets(&w).len(), 1, "A is on");
    w.online.clear();
    assert!(chunks.wanted_tickets(&w).is_empty(), "nobody of the party is on: no offline loader by default");
    chunks.teams.entry(P).or_default().members.entry(B).or_default().offline_force_loader = true;
    assert_eq!(chunks.wanted_tickets(&w).len(), 1);
    chunks.teams.get_mut(&P).unwrap().members.clear();
    chunks.config.set("force_load_mode", "ALWAYS");
    assert_eq!(chunks.wanted_tickets(&w).len(), 1);
}

#[test]
fn joining_a_party_brings_the_claims_and_leaving_takes_them_back() {
    let w = world();
    let mut chunks = Chunks::default();
    chunks.claim(&w.team(B).unwrap(), at(3, 3), 1).unwrap();
    assert_eq!(chunks.joined_party(&w.team(P).unwrap(), B), 0);
    assert_eq!(chunks.claims[&at(3, 3)].team, P);
    chunks.left_party(P, &w.team(B).unwrap(), false);
    assert_eq!(chunks.claims[&at(3, 3)].team, B);
}

#[test]
fn commands() {
    let w = world();
    let mut chunks = Chunks::default();
    let a = ctx(A, 0, 8.0, 8.0);
    assert_eq!(run(&mut chunks, &w, &a, "ftbchunks claim"), "1 chunk(s) claimed");
    assert_eq!(chunks.claims[&at(0, 0)].team, P, "the sender's party");
    assert_eq!(run(&mut chunks, &w, &a, "chunks claim 16"), "8 chunk(s) claimed", "3x3 around, the centre was claimed");
    assert_eq!(run(&mut chunks, &w, &a, "ftbchunks claim 48"), "40 chunk(s) claimed");
    assert_eq!(run(&mut chunks, &w, &a, "ftbchunks load 16"), "9 chunk(s) now kept loaded");
    assert_eq!(run(&mut chunks, &w, &a, "ftbchunks load 48"), "16 chunk(s) now kept loaded", "25 at most");
    assert_eq!(chunks.force_loaded_by(P), 25);
    assert_eq!(run(&mut chunks, &w, &a, "ftbchunks unload"), "1 chunk(s) no longer kept loaded");
    assert_eq!(run(&mut chunks, &w, &a, "ftbchunks claim 600"), "The number must be 512 or less, not 600");
    assert_eq!(run(&mut chunks, &w, &a, "ftbchunks info"), "Chunk [0, 0] minecraft:overworld | Held by: P | Kept loaded: false");
    assert_eq!(run(&mut chunks, &w, &a, "ftbchunks info 100 100"), "Chunk [6, 6] minecraft:overworld | Nobody has claimed this chunk");
    assert_eq!(run(&mut chunks, &w, &a, "ftbchunks admin bypass_protection"), "That command is not valid here or is unfinished");
    let op = Ctx { level: 2, ..ctx(C, 2, 1000.0, 1000.0) };
    let reply = perform(&mut chunks, &w, &op, "ftbchunks admin bypass_protection", 1).unwrap();
    assert_eq!(reply.bypass, Some(true));
    assert_eq!(run(&mut chunks, &w, &op, "ftbchunks admin claim_as P 0 0 64 64"), "1 chunk(s) claimed");
    assert_eq!(run(&mut chunks, &w, &op, "ftbchunks admin claim_as Nobody"), "No team matches 'Nobody'");
    assert_eq!(run(&mut chunks, &w, &op, "ftbchunks admin extra_claim_chunks A add 7"), "Extra claims now 7 for A");
    assert_eq!(chunks.limits(P).0, 507, "the party's limit follows its member's extra claims");
    assert_eq!(run(&mut chunks, &w, &op, "ftbchunks admin extra_claim_chunks A get"), "A: 7 extra claim(s)");
    // The limit: a 23x23 square only fills the party's 507.
    let before = chunks.claimed_by(P);
    let line = run(&mut chunks, &w, &op, "ftbchunks admin claim_as P 176 2000 64 2000");
    assert_eq!(line, format!("{} chunk(s) claimed", 507 - before));
    assert_eq!(run(&mut chunks, &w, &op, "ftbchunks admin claim_as P 0 5000 64 5000"), "0 chunk(s) claimed");
    assert_eq!(run(&mut chunks, &w, &a, "ftbchunks unclaim_all"), "507 chunk(s) given up");
    assert_eq!(chunks.claimed_by(P), 0);
    assert!(perform(&mut chunks, &w, &a, "ftbteams list", 1).is_none());
}

#[test]
fn config_keys() {
    let mut c = Config::default();
    assert!(c.set("max_claimed_chunks", "12") && c.max_claimed_chunks == 12);
    assert!(c.set("ally_mode", "FORCED_ALL") && c.ally_mode == AllyMode::ForcedAll);
    assert!(c.set("def_allow_explosions", "true") && c.prop_defaults.allow_explosions);
    assert!(c.set("def_block_edit", "PUBLIC") && c.prop_defaults.block_edit_mode == Privacy::Public);
    assert!(c.set_list("claim_dimension_whitelist", vec!["minecraft:overworld".into()]) && !c.may_claim_in("minecraft:the_nether"));
    assert!(!c.set("nope", "1") && !c.set("max_claimed_chunks", "x"));
}

// ---- The client messages (slice 3) ----

use crate::net::{self, ChangeRequest, ChunkInfo, Op};

#[test]
fn the_buttons_and_shift_pick_the_operation() {
    assert_eq!(Op::create(true, false), Op::Claim);
    assert_eq!(Op::create(false, false), Op::Unclaim);
    assert_eq!(Op::create(true, true), Op::Load);
    assert_eq!(Op::create(false, true), Op::Unload);
}

#[test]
fn a_change_request_runs_over_the_set_and_counts_problems() {
    let w = world();
    let mut chunks = Chunks::default();
    let a = w.team(A).unwrap();
    let b = w.team(B).unwrap();
    let dim = "minecraft:overworld";
    chunks.claim(&b, (dim.into(), 1, 0), 0).unwrap();
    let set = [[0, 0], [1, 0], [2, 0], [0, 0]];
    let (r, changed) = chunks.change(&a, Op::Claim, dim, &set, false, 5000);
    assert_eq!((r.changed, r.total), (2, 3), "the duplicate counts once");
    assert_eq!(r.problems.get("already_claimed"), Some(&1));
    assert_eq!(changed.len(), 2);
    let (r, _) = chunks.change(&a, Op::Load, dim, &[[0, 0], [1, 0], [5, 5]], false, 6000);
    assert_eq!(r.changed, 1);
    assert_eq!(r.problems.get("not_owner"), Some(&1));
    assert_eq!(r.problems.get("not_claimed"), Some(&1));
    let (r, _) = chunks.change(&a, Op::Unclaim, dim, &[[1, 0]], true, 7000);
    assert_eq!(r.changed, 1, "an operator's admin request takes another team's chunk");
    let g = chunks.general_data(A);
    assert_eq!((g.claimed, g.loaded, g.max_claims, g.max_force_loads), (2, 1, 500, 25));
}

#[test]
fn chunk_messages_group_by_team_hide_private_claims_and_split() {
    let w = world();
    let mut chunks = Chunks::default();
    let a = w.team(A).unwrap();
    let dim = "minecraft:overworld";
    let before = chunks.claims.clone();
    for x in 0..450 {
        chunks.claim(&a, (dim.into(), x, 0), 1000).unwrap();
    }
    chunks.load(&a, &(dim.into(), 0, 0), false, 2000).unwrap();
    let changed = chunks.changed_since(&before);
    assert_eq!(changed.len(), 450);
    let msgs = chunks.many_chunks(&changed, &before, &|_| true, 11_000);
    assert_eq!(msgs.len(), 2, "400 chunks a message");
    assert_eq!(msgs[0].team, net::uuid_text(A));
    let first = msgs[0].chunks[0];
    assert_eq!(first, ChunkInfo { x: 0, z: 0, claimed: true, since: 10, force_loaded: Some(9), expires: None });
    let hidden = chunks.many_chunks(&changed[..1], &before, &|_| false, 11_000);
    assert_eq!(hidden[0].chunks[0], ChunkInfo::unclaimed(0, 0));
    let snapshot = chunks.claims.clone();
    chunks.unclaim(A, &(dim.into(), 3, 0), false).unwrap();
    let gone = chunks.changed_since(&snapshot);
    let msgs = chunks.many_chunks(&gone, &snapshot, &|_| true, 11_000);
    assert_eq!((msgs[0].team.clone(), msgs[0].chunks[0]), (net::uuid_text(A), ChunkInfo::unclaimed(3, 0)));
    let mut private = a.clone();
    private.props.claim_visibility = Privacy::Private;
    assert!(net::sees_claims(&chunks.config, &private, A) && !net::sees_claims(&chunks.config, &private, C));
}

#[test]
fn the_wire_is_json_and_takes_a_team_uuid() {
    let req: ChangeRequest =
        serde_json::from_str(r#"{"op":"LOAD","chunks":[[1,2],[-3,4]],"team":"00000000-0000-0000-0000-000000000064"}"#).unwrap();
    assert_eq!(req.op, Op::Load);
    assert!(!req.admin);
    assert_eq!(req.team.as_deref().and_then(net::parse_uuid_text), Some(P));
    let text = serde_json::to_string(&ChunkInfo::unclaimed(1, 2)).unwrap();
    assert_eq!(text, r#"{"x":1,"z":2,"claimed":false}"#);
}
