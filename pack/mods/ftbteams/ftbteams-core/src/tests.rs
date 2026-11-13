//! The rules with several players, which the world test (one real player) can't reach.

use crate::{
    Manager, Out, Rank, TeamType,
    commands::{Ctx, Reply, Sender, perform},
    ftblib::uuid_string,
    sync::ClientManager,
};

const A: u128 = 0xa;
const B: u128 = 0xb;
const C: u128 = 0xc;

fn world() -> Manager {
    let mut m = Manager::new(42);
    m.player_logged_in(A, "Alice", true);
    m.player_logged_in(B, "Bob", true);
    m.player_logged_in(C, "Carol", true);
    m.out.clear();
    m
}

fn run(m: &mut Manager, who: u128, line: &str) -> Reply {
    let name = m.name_of(who).unwrap().to_owned();
    let lookup = |_: &str| None;
    let ctx = Ctx { sender: Sender { uuid: who, name, level: 0, player: true }, singleplayer: false, lookup: &lookup, dev: false };
    perform(m, &ctx, line).expect("ours")
}

fn text(r: &Reply) -> Vec<String> {
    r.lines.iter().map(|l| l.plain()).collect()
}

fn told(m: &mut Manager, who: u128) -> Vec<String> {
    let lines = m.out.iter().filter_map(|o| match o {
        Out::Tell(p, msg) if *p == who => Some(msg.plain()),
        _ => None,
    });
    let lines = lines.collect();
    m.out.clear();
    lines
}

fn party(m: &Manager, who: u128) -> u128 {
    m.party_of(who).expect("in a party")
}

#[test]
fn invite_join_and_leave() {
    let mut m = world();
    assert!(run(&mut m, A, "ftbteams party create").lines.is_empty());
    let p = party(&m, A);
    assert_eq!(m.teams[&p].props.display_name, "Alice's Team");
    assert_eq!(told(&mut m, A), ["<Server @Alice's Team>Alice is now in the party"]);
    // Bob can't join uninvited; the command's own check.
    let short = m.teams[&p].short_name();
    assert_eq!(text(&run(&mut m, B, &format!("ftbteams party join {short}"))), ["No invitation from Alice's Team is waiting for you"]);
    run(&mut m, A, "ftbteams party invite Bob");
    assert_eq!(m.teams[&p].rank_of(B), Rank::Invited);
    assert_eq!(told(&mut m, B), ["Alice asks you to join their party", "[Accept] [Decline]"]);
    run(&mut m, B, &format!("ftbteams party join {short}"));
    assert_eq!(m.party_of(B), Some(p));
    assert_eq!(told(&mut m, A), ["<Server @Alice's Team>Bob is now in the party"]);
    // A member isn't an officer: no invite for Bob.
    assert_eq!(text(&run(&mut m, B, "ftbteams party invite Carol")), ["That command is not valid here or is unfinished"]);
    // The owner can't leave while Bob is in it.
    assert_eq!(text(&run(&mut m, A, "ftbteams party leave")), ["Hand the party over to someone before you leave"]);
    run(&mut m, B, "ftbteams party leave");
    assert_eq!(m.party_of(B), None);
    assert_eq!(m.teams[&B].ranks.get(&B), Some(&Rank::Owner));
    // The last one out deletes the party.
    run(&mut m, A, "ftbteams party leave");
    assert!(!m.teams.contains_key(&p));
    assert!(m.out.iter().any(|o| matches!(o, Out::Event(crate::Event::Deleted { team }) if *team == p)));
}

#[test]
fn decline_makes_an_ally_and_allies_are_one_way() {
    let mut m = world();
    run(&mut m, A, "ftbteams party create Fort");
    let p = party(&m, A);
    run(&mut m, A, "ftbteams party invite Bob");
    let short = m.teams[&p].short_name();
    assert_eq!(text(&run(&mut m, B, &format!("ftbteams party decline {short}"))), ["Invitation turned down"]);
    assert_eq!(m.teams[&p].rank_of(B), Rank::Ally);
    m.out.clear();
    run(&mut m, A, "ftbteams party allies add Carol");
    assert_eq!(told(&mut m, C), ["'Fort' counts you as an ally"]);
    assert_eq!(text(&run(&mut m, A, "ftbteams party allies list")), ["Allied players:", "Bob", "Carol"]);
    // Carol's own team doesn't trust Alice back.
    assert_eq!(m.team_of(C).unwrap().rank_of(A), Rank::None);
    run(&mut m, A, "ftbteams party allies remove Bob");
    assert_eq!(m.teams[&p].rank_of(B), Rank::None);
    // An invite is "ally or better": adding an invited player as an ally does nothing.
    run(&mut m, A, "ftbteams party invite Bob");
    m.out.clear();
    run(&mut m, A, "ftbteams party allies add Bob");
    assert_eq!(m.teams[&p].rank_of(B), Rank::Invited);
    assert!(told(&mut m, B).is_empty());
}

#[test]
fn officers_kick_and_ownership_moves() {
    let mut m = world();
    run(&mut m, A, "ftbteams party create Fort");
    let p = party(&m, A);
    m.join(p, B).unwrap();
    m.join(p, C).unwrap();
    m.set_officer(p, A, (B, "Bob".into()), true).unwrap();
    assert_eq!(text(&run(&mut m, B, "ftbteams party kick Alice")), ["The owner cannot be removed"]);
    m.out.clear();
    run(&mut m, B, "ftbteams party kick Carol");
    assert_eq!(m.party_of(C), None);
    let to_carol = told(&mut m, C);
    assert_eq!(to_carol, ["<Bob @Fort>Carol was removed from Fort", "Carol was removed from Fort"]);
    assert_eq!(text(&run(&mut m, A, "ftbteams party transfer_ownership Carol")), ["Carol (00000000-0000-0000-0000-00000000000c) does not belong to Fort"]);
    assert_eq!(text(&run(&mut m, A, "ftbteams party transfer_ownership Alice")), ["That player already owns the team"]);
    run(&mut m, A, "ftbteams party transfer_ownership Bob");
    assert_eq!(m.teams[&p].owner, B);
    assert_eq!(m.teams[&p].rank_of(A), Rank::Officer);
    // Settings are the owner's now.
    assert_eq!(text(&run(&mut m, A, "ftbteams party settings ftbteams:description x")), ["That command is not valid here or is unfinished"]);
    assert_eq!(text(&run(&mut m, B, "ftbteams party settings ftbteams:display_name ab")), ["Could not read the value ab"]);
    assert_eq!(text(&run(&mut m, B, "ftbteams party settings ftbteams:color #ff0000")), ["Colour is now #ff0000"]);
    assert_eq!(text(&run(&mut m, B, "ftbteams party settings ftbteams:color")), ["Colour=#FF0000"]);
    assert_eq!(text(&run(&mut m, B, "ftbteams party settings ftbteams:lives_remaining 3")), ["Lives left is read-only"]);
}

#[test]
fn free_to_join_and_party_size() {
    let mut m = world();
    m.config.max_party_size = 2;
    run(&mut m, A, "ftbteams party create Open");
    let p = party(&m, A);
    run(&mut m, A, "ftbteams party settings ftbteams:free_to_join true");
    let short = m.teams[&p].short_name();
    run(&mut m, B, &format!("ftbteams party join {short}"));
    assert_eq!(m.party_of(B), Some(p));
    assert_eq!(text(&run(&mut m, C, &format!("ftbteams party join {short}"))), ["The party is full (limit 2)"]);
    // A player's name finds their team too.
    assert_eq!(text(&run(&mut m, C, "ftbteams info Bob"))[0], "==Open [party] ==");
}

#[test]
fn files_keep_everything_and_rebuild_parties() {
    let dir = std::env::temp_dir().join(format!("ftbteams-core-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut m = world();
    run(&mut m, A, "ftbteams party create Fort");
    let p = party(&m, A);
    m.join(p, B).unwrap();
    run(&mut m, A, "ftbteams party invite Carol");
    run(&mut m, A, "ftbteams redirect_chat");
    m.save(&dir).unwrap();
    let file = dir.join("ftbteams/party").join(format!("{}.snbt", uuid_string(p)));
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("\ttype: \"party\"\n") && text.contains("\"ftbteams:display_name\": \"Fort\""), "{text}");
    let back = Manager::load(&dir, 1);
    assert_eq!(back.id, m.id);
    assert_eq!(back.party_of(B), Some(p));
    assert_eq!(back.teams[&p].rank_of(C), Rank::Invited);
    assert!(back.chat_redirected.contains(&A));
    assert_eq!(back.teams[&p].history.len(), m.teams[&p].history.len());
    // Leaving last moves the file to deleted/.
    let mut back = back;
    back.leave(p, B).unwrap();
    back.leave(p, A).unwrap();
    back.save(&dir).unwrap();
    assert!(!file.exists());
    assert!(dir.join("ftbteams/deleted").join(format!("{}.snbt", uuid_string(p))).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn clients_see_their_own_team_in_full() {
    let mut m = world();
    run(&mut m, A, "ftbteams party create Fort");
    run(&mut m, A, "ftbteams party settings ftbteams:description secret");
    let p = party(&m, A);
    let mut alice = ClientManager::default();
    for part in m.sync_for(A, None) {
        alice.apply(&part).unwrap();
    }
    let mut bob = ClientManager::default();
    for part in m.sync_for(B, None) {
        bob.apply(&part).unwrap();
    }
    assert_eq!(alice.self_team, Some(p));
    assert_eq!(alice.teams[&p].properties["ftbteams:description"], "secret");
    assert!(!bob.teams[&p].properties.contains_key("ftbteams:description"));
    assert_eq!(bob.team_of(A).unwrap().properties["ftbteams:display_name"], "Fort");
    assert_eq!(bob.teams[&p].kind, TeamType::Party.name());
    // A delta removes a deleted party.
    m.leave(p, A).unwrap();
    for part in m.sync_for(B, Some(&[p, A])) {
        bob.apply(&part).unwrap();
    }
    assert!(!bob.teams.contains_key(&p));
    assert_eq!(bob.team_of(A).unwrap().properties["ftbteams:display_name"], "Alice");
}
