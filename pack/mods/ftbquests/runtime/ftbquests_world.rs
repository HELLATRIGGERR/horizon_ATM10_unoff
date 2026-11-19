// FTB Quests' server side (support module `crate::game::ftbquests_world` of the `ftbquests` native bundle):
// - the quest book, read once from `<config>/ftbquests/quests/` (ATM10's, from the player's instance) and shared
//   with the book screen;
// - each FTB team's progress, `<world>/ftbquests/<team uuid>.snbt` in FTB Quests' layout, written on each save;
// - item tasks seen in the players' inventories every `detection_delay` ticks, checkmark tasks and reward claims
//   from the book (`ftbquests:submit_task`, `ftbquests:claim_reward` with a choice's pick, `ftbquests:claim_all_rewards`),
//   the team's progress sent to its members;
// - the rewards given: items (what doesn't fit drops at the player's feet), experience points and levels, a reward
//   table's draws or pick, a command run as the player at the reward's permission level (on the next tick, where
//   no player's state is swapped in), a toast; claims refused while the team's `rewards_blocked` is set;
// - what the team just completed (`TeamData::newly`), handled at the next tick: completion toasts
//   (`ftbquests:notify`) to the members who are on, then the quests' auto-claimed rewards;
// - chat notices: a refused claim while rewards are blocked, a command reward's output (unless `silent`) and its
//   `feedback_message`, items dropped for want of room;
// - the book item (`ftbquests:book`, right click), `/ftbquests open_book`, `/ftbquests block_rewards`;
//   `/ftbquests_probe` (FTBTEAMS_DEV=1) answers `FTBQ {json}` for the tests.
// FTB Quests is All Rights Reserved: written from its behaviour; messages are translation keys with our own
// fallback wording.

use super::{Game, ftbteams_world};
use mod_api::{Command, CommandArgumentValue, CommandInvocation, CommandSource, WorldPhase};
use ftbquests_core::{
    CLAIM_ALL, CLAIM_REWARD, NOTIFY, Notice, OPEN_BOOK, Request, SUBMIT_TASK, SYNC,
    file::{QuestFile, code, parse_id},
    snbt,
    team::{Grant, TeamData, auto_claims, dashed},
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

/// A command reward waiting for the next tick.
struct PendingCommand {
    player: u128,
    team: String,
    reward: u64,
    command: String,
    level: u8,
    silent: bool,
    feedback: Option<String>,
}

struct Server {
    world: PathBuf,
    teams: BTreeMap<u128, TeamData>,
    ticks: u64,
    /// The team each player's client last got, to send a new picture when it changes.
    synced: HashMap<u128, u128>,
    /// For the probe: items, experience points and levels given, toasts sent, the last commands run and chat notices.
    gave: u64,
    xp_given: u64,
    levels_given: i64,
    notices: u64,
    ran: Vec<serde_json::Value>,
    told: Vec<String>,
    commands: Vec<PendingCommand>,
    rng: u64,
}

/// A uniform draw in [0, 1) (SplitMix64).
fn roll(state: &mut u64) -> f64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
}

fn keep_last<T>(list: &mut Vec<T>, item: T) {
    list.push(item);
    if list.len() > 8 {
        list.remove(0);
    }
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

/// `native_server_started` (where the fork's `WorldOpenReady` was): the teams' progress is read once the worlds are
/// open, before any player joins.
pub(super) fn opened(game: &mut Game<'_>) {
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
    let server = Server {
        world,
        teams,
        ticks: 0,
        synced: HashMap::new(),
        gave: 0,
        xp_given: 0,
        levels_given: 0,
        notices: 0,
        ran: Vec::new(),
        told: Vec::new(),
        commands: Vec::new(),
        rng: now_ms() as u64,
    };
    SERVER.with(|s| *s.borrow_mut() = Some(server));
}

/// `native_world_lifecycle`: the teams' progress is written and dropped when the world closes or fails to open.
pub(super) fn lifecycle(phase: WorldPhase) {
    if matches!(phase, WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed | WorldPhase::WorldOpenFailed) {
        saved();
        SERVER.with(|s| *s.borrow_mut() = None);
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
    let players: Vec<u128> = game.native_server_players().into_iter().map(|p| p.uuid()).filter(|p| only.is_none_or(|o| o == *p)).collect();
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

fn notify(game: &mut Game<'_>, player: u128, notice: &Notice) {
    let data = serde_json::to_vec(notice).unwrap_or_default();
    match game.native_net_send_to_player(player, NOTIFY, &data) {
        Ok(()) => {
            with_server(|s| s.notices += 1);
        }
        Err(e) => log::debug!("[ftbquests] {NOTIFY}: {e}"),
    }
}

/// A chat notice for the player.
fn tell(game: &mut Game<'_>, player: u128, line: Text) {
    let plain = line.plain();
    if game.native_tell(player, line) {
        with_server(|s| keep_last(&mut s.told, plain));
    }
}

/// A console command for the named player's grant; whether it ran.
fn console(game: &mut Game<'_>, name: &str, line: String) -> bool {
    !name.is_empty() && game.native_run_command(&line)
}

/// The online player `uuid`: their name and block position (the fork's `NativeServerPlayer` fields).
fn whereabouts(game: &Game<'_>, uuid: u128) -> Option<(String, [f64; 3])> {
    let p = game.fork_player(uuid)?;
    Some((game.player_name(p)?, game.entity_position(p.entity())?))
}

/// How many plain `item` fit in the main inventory of the player in place (`Inventory.add`'s room): the rest of a
/// given stack is thrown at their feet.
fn room_for(game: &Game<'_>, item: &str) -> u32 {
    let Some(max) = game.res.items().get(item).map(|i| (i.max_stack as u32).max(1)) else { return 0 };
    game.inventory
        .iter()
        .take(36)
        .map(|s| match s {
            None => max,
            Some(s) if *s.item == *item && s.damage == 0 && s.extra.is_none() => max.saturating_sub(s.count),
            Some(_) => 0,
        })
        .sum()
}

/// Gives what a claim of `reward` handed out; what it gave, for the toast. Commands wait for the next tick. Where
/// the player's state can't be swapped in (the server tick's auto-claims), items and experience go through the
/// console's `give` and `experience add` instead.
fn give(game: &mut Game<'_>, player: u128, team: &str, reward: u64, grants: Vec<Grant>) -> Notice {
    let mut notice = Notice { kind: "reward".into(), id: code(reward), ..Notice::default() };
    let name = whereabouts(game, player).map(|(name, _)| name).unwrap_or_default();
    for grant in grants {
        match grant {
            Grant::Item { item, count } => {
                // `player_give_stack` throws what doesn't fit at the player's feet, as `/give` does.
                let mut room = 0;
                let swapped = game.native_as_player(player, |g| room = room_for(g, &item));
                let mut given = if swapped { give_items(game, player, &item, count) } else { 0 };
                if given == 0 && console(game, &name, format!("give {name} {item} {count}")) {
                    given = count;
                }
                if swapped && given > 0 && room < count {
                    let name = Text::translate_with_fallback(&format!("item.{}", item.replace(':', ".")), &item, &[]);
                    let args = [Text::literal((count - room).to_string()), name];
                    tell(game, player, Text::translate_with_fallback("ftbquests.port.dropped", "No room for %s %s: dropped at your feet.", &args));
                }
                with_server(|s| s.gave += given as u64);
                notice.items.push((item, count));
            }
            Grant::Xp(points) => {
                let handle = game.fork_player(player);
                if handle.is_some_and(|p| game.player_give_experience_points(p, points)) || console(game, &name, format!("experience add {name} {points} points")) {
                    with_server(|s| s.xp_given += points as u64);
                } else {
                    log::warn!("[ftbquests] {points} experience for {player:032x} not given (reward {reward:016X})");
                }
                notice.xp += points;
            }
            Grant::Levels(levels) => {
                let handle = game.fork_player(player);
                if handle.is_some_and(|p| game.player_give_levels(p, levels)) || console(game, &name, format!("experience add {name} {levels} levels")) {
                    with_server(|s| s.levels_given += levels as i64);
                } else {
                    log::warn!("[ftbquests] {levels} levels for {player:032x} not given (reward {reward:016X})");
                }
                notice.levels += levels;
            }
            Grant::Command { command, level, silent, feedback } => {
                with_server(|s| s.commands.push(PendingCommand { player, team: team.to_owned(), reward, command, level, silent, feedback }));
            }
            Grant::Toast { reward, description } => notify(game, player, &Notice { kind: "toast".into(), id: code(reward), text: description, ..Notice::default() }),
        }
    }
    notice
}

/// Claims `reward` for `player` and gives it; what it gave, or None (refused: blocked rewards are told in chat).
fn claim_one(game: &mut Game<'_>, file: &QuestFile, team: u128, name: &str, player: u128, reward: u64, choice: Option<usize>) -> Option<Notice> {
    let claimed = with_server(|s| {
        let mut rng = s.rng;
        let out = data_of(s, team, name).claim(file, player, reward, choice, now_ms(), &mut || roll(&mut rng));
        s.rng = rng;
        out
    })?;
    match claimed {
        Ok(grants) => Some(give(game, player, name, reward, grants)),
        Err(why) => {
            if why == "rewards blocked" {
                tell(game, player, tr("ftbquests.port.blocked", "Your team's rewards are held back for now; this one stays unclaimed."));
            }
            log::debug!("[ftbquests] reward {reward:016X} not claimed: {why}");
            None
        }
    }
}

fn worth_a_toast(n: &Notice) -> bool {
    !n.items.is_empty() || n.xp > 0 || n.levels != 0
}

/// `{p}` the player's name, `{x}` `{y}` `{z}` their block, `{quest}` and `{chapter}` the ids, `{team}` the
/// team's name; no leading slash.
fn fill(command: &str, (player, position): &(String, [f64; 3]), file: &QuestFile, reward: u64, team: &str) -> String {
    let quest = file.rewards.get(&reward).map_or(0, |r| r.quest);
    let chapter = file.quests.get(&quest).map_or(0, |q| q.chapter);
    let [x, y, z] = position.map(|v| v.floor() as i64);
    command
        .trim()
        .trim_start_matches('/')
        .replace("{p}", player)
        .replace("{x}", &x.to_string())
        .replace("{y}", &y.to_string())
        .replace("{z}", &z.to_string())
        .replace("{quest}", &code(quest))
        .replace("{chapter}", &code(chapter))
        .replace("{team}", team)
}

/// The level vanilla's command `label` needs (an alias resolved, `tp` gives `teleport`'s): the fork's
/// `native_command_info`'s `level`, which the merge dropped. None for one vanilla lacks.
fn command_level(label: &str) -> Option<u8> {
    let d = horizon_command::vanilla::game_dispatcher();
    let label = label.strip_prefix('/').unwrap_or(label);
    let mut node = d.find(label)?;
    if let Some(target) = d.nodes[node].redirect.filter(|t| d.nodes[0].children.contains(t)) {
        node = target;
    }
    Some(d.nodes[node].permission)
}

/// The command rewards claimed since the last tick, each run as its player (`execute as` them `at` them, so `@s`
/// and `@p` are they) when the command's level is within the reward's `permission_level`.
fn run_commands(game: &mut Game<'_>, file: &QuestFile) {
    let pending = with_server(|s| std::mem::take(&mut s.commands)).unwrap_or_default();
    for c in pending {
        let Some(p) = whereabouts(game, c.player) else { continue };
        let line = fill(&c.command, &p, file, c.reward, &c.team);
        let needs = command_level(line.split_whitespace().next().unwrap_or_default());
        let (ran, output) = if needs.is_some_and(|n| n > c.level) {
            (false, vec![Text::translate("command.unknown.command", &[]).with_color(horizon_client::text::RED)])
        } else {
            game.run_console_command(&format!("execute as {} at @s run {line}", p.0), true)
        };
        if !c.silent {
            for l in &output {
                tell(game, c.player, l.clone());
            }
        }
        if let (true, Some(f)) = (ran, &c.feedback) {
            tell(game, c.player, Text::translate_with_fallback(f, f, &[]));
        }
        log::info!("[ftbquests] command reward {:016X} for {}: {line} (level {}) ran={ran}", c.reward, p.0, c.level);
        let entry = serde_json::json!({
            "reward": code(c.reward), "line": line, "level": c.level, "needs": needs, "ran": ran,
            "output": output.iter().map(Text::plain).collect::<Vec<_>>(),
        });
        with_server(|s| keep_last(&mut s.ran, entry));
    }
}

/// What the teams completed since the last tick: toasts to the members who are on, then the quests' auto-claimed
/// rewards for each of them, then the new progress.
fn completions(game: &mut Game<'_>, file: &QuestFile, players: &[u128]) {
    let done: Vec<(u128, String, Vec<u64>)> =
        with_server(|s| s.teams.iter_mut().filter(|(_, d)| !d.newly.is_empty()).map(|(id, d)| (*id, d.name.clone(), std::mem::take(&mut d.newly))).collect()).unwrap_or_default();
    for (team, name, ids) in done {
        let members: Vec<u128> = players.iter().copied().filter(|p| team_of(*p).map(|t| t.0) == Some(team)).collect();
        for id in ids {
            // A task's own toast only where its quest has more than one; the quest's follows anyway.
            let kind = if let Some(t) = file.tasks.get(&id) {
                file.quests.get(&t.quest).is_some_and(|q| q.tasks.len() > 1).then_some("task")
            } else if file.quests.contains_key(&id) {
                Some("quest")
            } else {
                file.chapter(id).map(|_| "chapter")
            };
            if let (Some(kind), true) = (kind, file.toast_for(id)) {
                for m in &members {
                    notify(game, *m, &Notice { kind: kind.into(), id: code(id), ..Notice::default() });
                }
            }
            if kind == Some("quest") {
                for (reward, toast) in auto_claims(file, id) {
                    for m in &members {
                        if let Some(n) = claim_one(game, file, team, &name, *m, reward, None).filter(|n| toast && worth_a_toast(n)) {
                            notify(game, *m, &n);
                        }
                    }
                }
            }
        }
        sync(game, file, team, None);
    }
}

/// `native_server_tick` (its end): the commands and completions since the last tick; every `detection_delay` ticks
/// each player's inventory is looked at for the item tasks; a player whose client lacks their team's progress gets it.
pub(super) fn tick(game: &mut Game<'_>) {
    let Some(file) = quest_file() else { return };
    let Some(ticks) = with_server(|s| {
        s.ticks += 1;
        s.ticks
    }) else {
        return;
    };
    let players: Vec<u128> = game.native_server_players().into_iter().map(|p| p.uuid()).collect();
    run_commands(game, &file);
    completions(game, &file, &players);
    for p in &players {
        let p = Online { uuid: *p };
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
        game.native_as_player(p.uuid, |g| carried = carried_items(g));
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

/// `native_net_from_client`: the book's checkmark, claim, pick and claim-all buttons.
pub(super) fn from_client(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    if channel != SUBMIT_TASK && channel != CLAIM_REWARD && channel != CLAIM_ALL {
        return;
    }
    let Some(file) = quest_file() else { return };
    let Some(request) = serde_json::from_slice::<Request>(data).ok() else {
        log::warn!("[ftbquests] a bad {channel} message");
        return;
    };
    let Some((team, name)) = team_of(player) else { return };
    let now = now_ms();
    if channel == CLAIM_ALL {
        // One toast for all of it, as the book's "claim all" gathers what it took.
        let list = with_server(|s| data_of(s, team, &name).claim_all_list(&file, player)).unwrap_or_default();
        let mut all = Notice { kind: "reward".into(), ..Notice::default() };
        for reward in list {
            if let Some(n) = claim_one(game, &file, team, &name, player, reward, None) {
                all.items.extend(n.items);
                all.xp += n.xp;
                all.levels += n.levels;
            }
        }
        if worth_a_toast(&all) {
            notify(game, player, &all);
        }
        sync(game, &file, team, None);
        return;
    }
    let Some(id) = parse_id(&request.id) else { return };
    if channel == SUBMIT_TASK {
        if with_server(|s| data_of(s, team, &name).check(&file, id, now)) == Some(true) {
            sync(game, &file, team, None);
        }
        return;
    }
    if let Some(n) = claim_one(game, &file, team, &name, player, id, request.index) {
        if worth_a_toast(&n) {
            notify(game, player, &n);
        }
        sync(game, &file, team, None);
    }
}

/// A player on the server (the tick's loop reads the UUID only).
struct Online {
    uuid: u128,
}

/// The plain stacks (no damage, no components) the player in place carries in the main inventory and the off hand,
/// summed by item: what the item tasks count (the fork's `native_carried_items`, which the merge dropped).
fn carried_items(game: &Game<'_>) -> Vec<(String, u32)> {
    let mut out: Vec<(String, u32)> = Vec::new();
    let held = game.inventory.iter().take(36).chain(std::iter::once(&game.offhand));
    for s in held.flatten().filter(|s| s.damage == 0 && s.extra.is_none()) {
        match out.iter_mut().find(|(i, _)| *i == *s.item) {
            Some((_, n)) => *n += s.count,
            None => out.push((s.item.to_string(), s.count)),
        }
    }
    out
}

/// Gives `player` `count` plain `item` in whole stacks as `/give` does (what does not fit is thrown for them to pick
/// up); how many it gave (the fork's `native_give_items`, which the merge dropped).
fn give_items(game: &mut Game<'_>, player: u128, item: &str, count: u32) -> u32 {
    let Some(max) = game.res.items().get(item).map(|i| i.max_stack) else { return 0 };
    let Some(handle) = game.fork_player(player) else { return 0 };
    let max = (max as u32).max(1);
    let mut given = 0;
    while given < count {
        let n = (count - given).min(max);
        if game.player_give_stack(handle, horizon_client::items::ItemStack::new(item, n)).is_err() {
            break;
        }
        given += n;
    }
    given
}

fn open_book(game: &mut Game<'_>, player: u128) -> bool {
    match game.native_net_send_to_player(player, OPEN_BOOK, b"{}") {
        Ok(()) => true,
        Err(e) => {
            log::debug!("[ftbquests] {OPEN_BOOK}: {e}");
            false
        }
    }
}

thread_local! {
    /// `use_item` opened the book for the use in progress: `take_book_use` answers it.
    static BOOK_USED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether `use_item` took the use the hook is answering (once).
pub(super) fn take_book_use() -> bool {
    BOOK_USED.with(|b| b.replace(false))
}

/// `native_use_item`: the book item opens the book (FTB Quests' `QuestBookItem`), on the server (the window's
/// client has no player to send to); the use is then answered `Success` (`take_book_use`).
pub(super) fn use_item(game: &mut Game<'_>, stack: &horizon_client::items::ItemStack) {
    BOOK_USED.with(|b| b.set(false));
    if &*stack.item != BOOK_ITEM {
        return;
    }
    let player = game.native_acting_id();
    if open_book(game, player) {
        BOOK_USED.with(|b| b.set(true));
    }
}

/// `native_register_commands`. `/ftbquests` takes one optional `text` line its rules read (upstream's dispatcher
/// parses a usage's arguments itself, and its `word` takes no `:`): `open_book`, `block_rewards <true|false>
/// [player]` (level 2, checked in `perform`); the probe's `pick`, `complete <quest>`, `command <level> <line>`.
pub(super) fn register(commands: &mut Vec<Command>) {
    commands.push(Command::new("ftbquests [line:text]"));
    if dev() {
        commands.push(Command::op("ftbquests_probe [line:text]", 2));
    }
}

/// The command line a parsed invocation of one of `register`'s usages was typed as: its words, then the `text`.
fn line(call: &CommandInvocation) -> String {
    let mut line = call.literals.join(" ");
    for argument in &call.arguments {
        if let CommandArgumentValue::String(text) = &argument.value {
            line.push(' ');
            line.push_str(text);
        }
    }
    line
}

/// Quests of the book for the tests to play, in book order, none waiting on another: one whose only task is a
/// plain vanilla item and which gives a plain vanilla item, and one with a checkmark. Then, for each reward kind of
/// this slice, the first reward of that kind on a quest no other pick has (the test completes its quest with
/// `/ftbquests_probe complete`), and `auto`: an xp reward of yet another quest, for the auto-claim.
fn pick(f: &QuestFile) -> serde_json::Value {
    use ftbquests_core::file::{RewardKind, TaskKind};
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
            RewardKind::Item { item, count, components: false, random_bonus: 0 } if vanilla(item) && *count <= 64 => Some((*r, item.clone(), *count)),
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
    let mut used: Vec<String> = [&item, &check].iter().filter_map(|p| p.as_ref()?["quest"].as_str().map(str::to_owned)).collect();
    let mut picks = serde_json::json!({ "item": item, "check": check });
    for (name, kind) in [("xp", "xp"), ("xp_levels", "xp_levels"), ("random", "random"), ("loot", "loot"), ("choice", "choice"), ("command", "command"), ("toast", "toast"), ("auto", "xp")] {
        // The auto-claim's quest from the book's end, where nothing the test did before completes it.
        let order: Vec<_> = if name == "auto" { quests().collect::<Vec<_>>().into_iter().rev().collect() } else { quests().collect() };
        let found = order.into_iter().find_map(|(c, q)| {
            if used.contains(&code(q.id)) {
                return None;
            }
            let r = q.rewards.iter().filter_map(|r| f.rewards.get(r)).find(|r| r.type_name() == kind && !matches!(r.kind, RewardKind::Other(_)))?;
            if name == "auto" && r.exclude_from_claim_all {
                return None;
            }
            let takes = |r: &&ftbquests_core::file::Reward| !r.exclude_from_claim_all && !matches!(r.kind, RewardKind::Choice { .. } | RewardKind::Other(_));
            let mut p = serde_json::json!({
                "quest": code(q.id), "chapter": c.filename, "reward": code(r.id), "excluded": r.exclude_from_claim_all,
                "quest_rewards": q.rewards.iter().map(|r| code(*r)).collect::<Vec<_>>(),
                "claim_all": q.rewards.iter().filter_map(|r| f.rewards.get(r)).filter(takes).map(|r| code(r.id)).collect::<Vec<_>>(),
            });
            match &r.kind {
                RewardKind::Xp { points } => p["xp"] = (*points).into(),
                RewardKind::XpLevels { levels } => p["levels"] = (*levels).into(),
                RewardKind::Random { table } | RewardKind::Loot { table } | RewardKind::Choice { table } => {
                    let t = f.reward_tables.get(table)?;
                    p["table"] = code(*table).into();
                    p["entries"] = t.entries.len().into();
                    p["loot_size"] = t.loot_size.into();
                }
                RewardKind::Command { command, permission_level, .. } => {
                    p["command"] = command.clone().into();
                    p["permission_level"] = (*permission_level).into();
                }
                _ => {}
            }
            Some(p)
        });
        if let Some(q) = found.as_ref().and_then(|p| p["quest"].as_str()) {
            used.push(q.to_owned());
        }
        picks[name] = found.unwrap_or_default();
    }
    picks
}

/// `/ftbquests block_rewards <true|false> [player]`: sets the team's `rewards_blocked` (the player's own team by default).
fn block_rewards(game: &mut Game<'_>, player: &Online, words: &[&str]) {
    let Some(blocked) = words.get(2).and_then(|w| w.parse::<bool>().ok()) else {
        game.native_feedback(tr("ftbquests.port.block_usage", "Say true or false: /ftbquests block_rewards <true|false> [player]"));
        return;
    };
    let who = match words.get(3) {
        Some(name) => game.native_player_by_name(name).map(|p| p.uuid()),
        None => Some(player.uuid),
    };
    let Some((team, name)) = who.and_then(team_of) else {
        game.native_feedback(tr("ftbquests.port.no_team", "No such player on the server."));
        return;
    };
    let Some(file) = quest_file() else { return };
    with_server(|s| {
        let d = data_of(s, team, &name);
        d.rewards_blocked = blocked;
        d.dirty = true;
    });
    sync(game, &file, team, None);
    let args = [Text::literal(name), Text::literal(blocked.to_string())];
    game.native_feedback(Text::translate_with_fallback("ftbquests.port.blocked_set", "Rewards of %s held back: %s", &args));
}

/// `native_run_mod_command`: `/ftbquests open_book`; `/ftbquests_probe` for the tests.
pub(super) fn perform(game: &mut Game<'_>, source: &CommandSource, call: &CommandInvocation, ran: &mut bool) {
    if *ran {
        return;
    }
    let CommandSource::Player { player, permission } = source else { return };
    let level = permission.level();
    let player = Online { uuid: player.uuid() };
    let command = line(call);
    let words: Vec<&str> = command.split_whitespace().collect();
    match words.as_slice() {
        ["ftbquests", "open_book"] => {
            *ran = true;
            open_book(game, player.uuid);
        }
        ["ftbquests", "block_rewards", ..] => {
            *ran = true;
            if level >= 2 {
                block_rewards(game, &player, &words);
            } else {
                game.native_feedback(Text::translate("command.unknown.command", &[]).with_color(horizon_client::text::RED));
            }
        }
        ["ftbquests", ..] => {
            *ran = true;
            game.native_feedback(tr("ftbquests.port.not_yet", "This part of the quest commands isn't in the port yet."));
        }
        ["ftbquests_probe", ..] if dev() => {
            *ran = true;
            let file = quest_file();
            let me = team_of(player.uuid);
            if let (Some(f), Some((team, name)), ["ftbquests_probe", "complete", quest]) = (&file, &me, words.as_slice()) {
                let id = parse_id(quest).unwrap_or(0);
                let done = with_server(|s| data_of(s, *team, name).force_complete(f, id, now_ms())).unwrap_or(false);
                game.native_feedback(Text::literal(format!("FTBQC {{\"quest\":\"{}\",\"done\":{done}}}", code(id))));
                return;
            }
            // A command reward's run, for a line of the test's own: `command <level> <line>`.
            if let (Some((_, name)), ["ftbquests_probe", "command", level, line @ ..]) = (&me, words.as_slice()) {
                let level = level.parse::<u8>().unwrap_or(2).min(4);
                let command = line.join(" ");
                with_server(|s| s.commands.push(PendingCommand { player: player.uuid, team: name.clone(), reward: 0, command, level, silent: false, feedback: None }));
                game.native_feedback(Text::literal("FTBQC {}"));
                return;
            }
            let xp = game
                .fork_player(player.uuid)
                .and_then(|p| game.player_experience(p))
                .map(|e| serde_json::json!({ "level": e.level, "progress": e.progress }));
            let json = with_server(|s| {
                let teams: Vec<serde_json::Value> = s
                    .teams
                    .values()
                    .map(|d| {
                        serde_json::json!({
                            "id": dashed(d.uuid), "tasks": d.task_progress.len(), "completed": d.completed.len(),
                            "claimed": d.claimed.len(), "dirty": d.dirty, "blocked": d.rewards_blocked,
                            "claims": d.claimed.keys().map(|(_, r)| code(*r)).collect::<Vec<_>>(),
                            "progress": d.task_progress.iter().map(|(k, v)| (format!("{k:016X}"), *v)).collect::<BTreeMap<_, _>>(),
                            "done": d.completed.keys().map(|k| format!("{k:016X}")).collect::<Vec<_>>(),
                        })
                    })
                    .collect();
                serde_json::json!({
                    "world": s.world.display().to_string(), "ticks": s.ticks, "gave": s.gave, "teams": teams,
                    "team": me.as_ref().map(|t| dashed(t.0)), "xp_given": s.xp_given, "levels_given": s.levels_given,
                    "notices": s.notices, "ran": s.ran, "told": s.told, "xp": xp,
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
