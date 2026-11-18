// FTB Quests' server side (support module `crate::game::ftbquests_world` of the `ftbquests` native bundle):
// - the quest book, read once from `<config>/ftbquests/quests/` (ATM10's, from the player's instance) and shared
//   with the book screen;
// - each FTB team's progress, `<world>/ftbquests/<team uuid>.snbt` in FTB Quests' layout, written on each save;
// - item tasks seen in the players' inventories every `detection_delay` ticks, checkmark tasks and reward claims
//   from the book (`ftbquests:submit_task`, `ftbquests:claim_reward`), the team's progress sent to its members;
// - the book item (`ftbquests:book`, right click) and `/ftbquests open_book` open the book;
//   `/ftbquests_probe` (FTBTEAMS_DEV=1) answers `FTBQ {json}` for the tests.
// FTB Quests is All Rights Reserved: written from its behaviour; messages are translation keys with our own
// fallback wording.

use super::{
    Game, ftbteams_world,
    native_lifecycle::WorldPhase,
    native_server::{NativeCommand, NativeServerPlayer},
};
use ftbquests_core::{
    CLAIM_REWARD, OPEN_BOOK, Request, SUBMIT_TASK, SYNC,
    file::{QuestFile, parse_id},
    snbt,
    team::{TeamData, dashed},
};
use horizon_client::text::Text;
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) const BOOK_ITEM: &str = "ftbquests:book";

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}
fn dev() -> bool {
    std::env::var("FTBTEAMS_DEV").is_ok_and(|v| v == "1")
}
fn tr(key: &str, fallback: &str) -> Text {
    Text::translate_with_fallback(key, fallback, &[])
}

/// The quest folder beside the game: `config/ftbquests/quests`.
pub(super) fn quest_dir() -> Option<PathBuf> {
    Game::native_config_dir().ok().map(|c| c.join("ftbquests").join("quests"))
}

/// The book, read once for the server and the window (the same folder); the error when it didn't read.
static FILE: Mutex<Option<Result<Arc<QuestFile>, String>>> = Mutex::new(None);

pub(super) fn quest_file() -> Option<Arc<QuestFile>> {
    let mut slot = FILE.lock().ok()?;
    if slot.is_none() {
        let loaded = match quest_dir() {
            Some(dir) => QuestFile::load(&dir, "en_us").map(|(file, warnings)| {
                for w in &warnings {
                    log::warn!("[ftbquests] {w}");
                }
                log::info!("[ftbquests] the book: {} chapters, {} quests from {}", file.chapters.len(), file.quests.len(), dir.display());
                Arc::new(file)
            }),
            None => Err("no config folder".to_owned()),
        };
        if let Err(e) = &loaded {
            log::info!("[ftbquests] no quest book: {e}");
        }
        *slot = Some(loaded);
    }
    slot.as_ref()?.as_ref().ok().cloned()
}
fn file_error() -> Option<String> {
    FILE.lock().ok()?.as_ref()?.as_ref().err().cloned()
}

struct Server {
    world: PathBuf,
    teams: BTreeMap<u128, TeamData>,
    ticks: u64,
    /// The team each player's client last got, to send a new picture when it changes.
    synced: HashMap<u128, u128>,
    gave: u64,
}

thread_local! {
    static SERVER: RefCell<Option<Server>> = const { RefCell::new(None) };
}

fn with_server<R>(f: impl FnOnce(&mut Server) -> R) -> Option<R> {
    SERVER.with(|s| s.borrow_mut().as_mut().map(f))
}

fn team_dir(world: &std::path::Path) -> PathBuf {
    world.join("ftbquests")
}

/// `native_world_lifecycle`: the teams' progress is read when the world opens, written when it closes.
pub(super) fn lifecycle(game: &mut Game<'_>, phase: WorldPhase) {
    match phase {
        WorldPhase::WorldOpenReady => {
            let Some(world) = game.native_world_dir() else { return };
            let _ = quest_file();
            let mut teams = BTreeMap::new();
            if let Ok(entries) = std::fs::read_dir(team_dir(&world)) {
                for path in entries.flatten().map(|e| e.path()) {
                    let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                    if !(name.contains('-') && name.ends_with(".snbt")) {
                        continue;
                    }
                    match snbt::read_file(&path).and_then(|c| TeamData::from_snbt(&c)) {
                        Some(data) => {
                            teams.insert(data.uuid, data);
                        }
                        None => log::warn!("[ftbquests] {} doesn't read; ignoring it", path.display()),
                    }
                }
            }
            log::info!("[ftbquests] {} team progress file(s)", teams.len());
            SERVER.with(|s| *s.borrow_mut() = Some(Server { world, teams, ticks: 0, synced: HashMap::new(), gave: 0 }));
        }
        WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed | WorldPhase::WorldOpenFailed => {
            saved();
            SERVER.with(|s| *s.borrow_mut() = None);
        }
    }
}

/// `native_world_saved`: the changed teams' files.
pub(super) fn saved() {
    with_server(|s| {
        let dir = team_dir(&s.world);
        for data in s.teams.values_mut().filter(|d| d.dirty) {
            let path = dir.join(format!("{}.snbt", dashed(data.uuid)));
            match std::fs::create_dir_all(&dir).and_then(|()| snbt::write_file(&path, &data.to_snbt())) {
                Ok(()) => data.dirty = false,
                Err(e) => log::warn!("[ftbquests] {}: {e}", path.display()),
            }
        }
    });
}

/// The player's FTB team (their party, else their own), and its name.
fn team_of(player: u128) -> Option<(u128, String)> {
    ftbteams_world::with_manager(|m| m.team_of(player).map(|t| (t.id, t.player_name.clone()))).flatten()
}

/// The team's progress, made when the team has none yet.
fn data_of<'a>(s: &'a mut Server, team: u128, name: &str) -> &'a mut TeamData {
    s.teams.entry(team).or_insert_with(|| TeamData::new(team, name))
}

/// Sends `team`'s progress to its members who are on (all of them: `only` None), or to one player.
fn sync(game: &mut Game<'_>, file: &QuestFile, team: u128, only: Option<u128>) {
    let players: Vec<u128> = game.native_server_players().into_iter().map(|p| p.uuid).filter(|p| only.is_none_or(|o| o == *p)).collect();
    for player in players {
        if team_of(player).map(|t| t.0) != Some(team) {
            continue;
        }
        let parts = with_server(|s| s.teams.get(&team).map(|d| ftbquests_core::sync_parts(file, d, player))).flatten().unwrap_or_default();
        for part in parts {
            let data = serde_json::to_vec(&part).unwrap_or_default();
            if let Err(e) = game.native_net_send_to_player(player, SYNC, &data) {
                log::debug!("[ftbquests] {SYNC}: {e}");
            }
        }
        with_server(|s| s.synced.insert(player, team));
    }
}

/// `native_server_tick` (its end): every `detection_delay` ticks each player's inventory is looked at for the item
/// tasks; a player whose client lacks their team's progress gets it.
pub(super) fn tick(game: &mut Game<'_>) {
    let Some(file) = quest_file() else { return };
    let Some(ticks) = with_server(|s| {
        s.ticks += 1;
        s.ticks
    }) else {
        return;
    };
    let players = game.native_server_players();
    for p in &players {
        let Some((team, name)) = team_of(p.uuid) else { continue };
        if with_server(|s| s.synced.get(&p.uuid) != Some(&team)) == Some(true) {
            with_server(|s| {
                data_of(s, team, &name);
            });
            sync(game, &file, team, Some(p.uuid));
        }
        if ticks % file.detection_delay as u64 != 0 {
            continue;
        }
        let mut carried = Vec::new();
        game.native_as_player(p.uuid, |g| carried = g.native_carried_items());
        if carried.is_empty() {
            continue;
        }
        let changed = with_server(|s| data_of(s, team, &name).detect_items(&file, &carried, now_ms())).unwrap_or(false);
        if changed {
            sync(game, &file, team, None);
        }
    }
}

/// `native_player_logged_in`: the player's client gets their team's progress on the next tick.
pub(super) fn logged_in(player: u128) {
    with_server(|s| s.synced.remove(&player));
}

/// `native_net_from_client`: the book's checkmark and claim buttons.
pub(super) fn from_client(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    if channel != SUBMIT_TASK && channel != CLAIM_REWARD {
        return;
    }
    let Some(file) = quest_file() else { return };
    let Some(id) = serde_json::from_slice::<Request>(data).ok().and_then(|r| parse_id(&r.id)) else {
        log::warn!("[ftbquests] a bad {channel} message");
        return;
    };
    let Some((team, name)) = team_of(player) else { return };
    let now = now_ms();
    if channel == SUBMIT_TASK {
        if with_server(|s| data_of(s, team, &name).check(&file, id, now)) == Some(true) {
            sync(game, &file, team, None);
        }
        return;
    }
    let claimed = with_server(|s| data_of(s, team, &name).claim(&file, player, id, now));
    match claimed {
        Some(Ok(item)) => {
            if let Some((item, count)) = item {
                let mut given = 0;
                game.native_as_player(player, |g| given = g.native_give_items(&item, count));
                if given < count {
                    log::warn!("[ftbquests] {} of {count} {item} didn't fit in the inventory", count - given);
                }
                with_server(|s| s.gave += given as u64);
            }
            sync(game, &file, team, None);
        }
        Some(Err(why)) => log::debug!("[ftbquests] reward {id:016X} not claimed: {why}"),
        None => {}
    }
}

fn open_book(game: &mut Game<'_>, player: u128) {
    if let Err(e) = game.native_net_send_to_player(player, OPEN_BOOK, b"{}") {
        log::debug!("[ftbquests] {OPEN_BOOK}: {e}");
    }
}

/// `native_use_item`: the book item opens the book (FTB Quests' `QuestBookItem`).
pub(super) fn use_item(game: &mut Game<'_>, stack: &horizon_client::items::ItemStack, claimed: &mut bool) {
    if *claimed || &*stack.item != BOOK_ITEM {
        return;
    }
    *claimed = true;
    let player = game.native_acting_id();
    open_book(game, player);
}

pub(super) fn register(commands: &mut Vec<NativeCommand>) {
    commands.push(NativeCommand::new("ftbquests open_book"));
    if dev() {
        commands.push(NativeCommand::op("ftbquests_probe", 2));
        commands.push(NativeCommand::op("ftbquests_probe pick", 2));
    }
}

/// Quests of the book for the tests to play, in book order, none waiting on another: one whose only task is a
/// plain vanilla item and which gives a plain vanilla item, and one with a checkmark.
fn pick(f: &QuestFile) -> serde_json::Value {
    use ftbquests_core::file::{RewardKind, TaskKind, code};
    let vanilla = |i: &str| i.starts_with("minecraft:");
    let free = |q: &ftbquests_core::file::Quest| q.dependencies.iter().all(|d| !f.quests.contains_key(d));
    let quests = || f.chapters.iter().flat_map(|c| c.quests.iter().filter_map(|q| f.quests.get(q)).map(move |q| (c, q)));
    let item = quests().find_map(|(c, q)| {
        let [t] = q.tasks.as_slice() else { return None };
        let TaskKind::Item { item, count, components: false } = &f.tasks.get(t)?.kind else { return None };
        if !free(q) || !vanilla(item) || *count > 64 {
            return None;
        }
        let (r, ritem, rcount) = q.rewards.iter().find_map(|r| match &f.rewards.get(r)?.kind {
            RewardKind::Item { item, count, components: false } if vanilla(item) && *count <= 64 => Some((*r, item.clone(), *count)),
            _ => None,
        })?;
        Some(serde_json::json!({
            "quest": code(q.id), "chapter": c.filename, "task": code(*t), "task_item": item, "task_count": count,
            "reward": code(r), "reward_item": ritem, "reward_count": rcount,
        }))
    });
    let check = quests().find_map(|(c, q)| {
        let t = q.tasks.iter().find(|t| matches!(f.tasks.get(t).map(|t| &t.kind), Some(TaskKind::Checkmark)))?;
        free(q).then(|| serde_json::json!({ "quest": code(q.id), "chapter": c.filename, "task": code(*t) }))
    });
    serde_json::json!({ "item": item, "check": check })
}

/// `native_perform_command`: `/ftbquests open_book`; `/ftbquests_probe` for the tests.
pub(super) fn perform(game: &mut Game<'_>, player: &NativeServerPlayer, command: &str, ran: &mut bool) {
    if *ran {
        return;
    }
    let words: Vec<&str> = command.split_whitespace().collect();
    match words.as_slice() {
        ["ftbquests", "open_book"] => {
            *ran = true;
            open_book(game, player.uuid);
        }
        ["ftbquests", ..] => {
            *ran = true;
            game.native_feedback(tr("ftbquests.port.not_yet", "This part of the quest commands isn't in the port yet."));
        }
        ["ftbquests_probe", ..] if dev() => {
            *ran = true;
            let file = quest_file();
            let me = team_of(player.uuid);
            let json = with_server(|s| {
                let teams: Vec<serde_json::Value> = s
                    .teams
                    .values()
                    .map(|d| {
                        serde_json::json!({
                            "id": dashed(d.uuid), "tasks": d.task_progress.len(), "completed": d.completed.len(),
                            "claimed": d.claimed.len(), "dirty": d.dirty,
                            "progress": d.task_progress.iter().map(|(k, v)| (format!("{k:016X}"), *v)).collect::<BTreeMap<_, _>>(),
                            "done": d.completed.keys().map(|k| format!("{k:016X}")).collect::<Vec<_>>(),
                        })
                    })
                    .collect();
                serde_json::json!({
                    "world": s.world.display().to_string(), "ticks": s.ticks, "gave": s.gave, "teams": teams,
                    "team": me.as_ref().map(|t| dashed(t.0)),
                })
            })
            .unwrap_or_else(|| serde_json::json!({ "error": "no world on this side" }));
            let mut json = json;
            json["quest_dir"] = serde_json::json!(quest_dir().map(|d| d.display().to_string()));
            json["file_error"] = serde_json::json!(file_error());
            if let (Some(f), Some(&"pick")) = (&file, words.get(1)) {
                json["pick"] = pick(f);
            }
            if let Some(f) = file {
                json["chapters"] = serde_json::json!(f.chapters.len());
                json["quests"] = serde_json::json!(f.quests.len());
                json["tasks"] = serde_json::json!(f.tasks.len());
                json["rewards"] = serde_json::json!(f.rewards.len());
                json["detection_delay"] = serde_json::json!(f.detection_delay);
            }
            game.native_feedback(Text::literal(format!("FTBQ {json}")));
        }
        _ => {}
    }
}
