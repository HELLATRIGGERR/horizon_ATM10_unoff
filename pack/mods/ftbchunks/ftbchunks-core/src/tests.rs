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
