// FTB Teams in the game (support module `crate::game::ftbteams_world` of the `ftbteams` native bundle). The
// rules are the bundle's crate `ftbteams_core`; this module is the glue: the world's `ftbteams/` folder, logins,
// `/ftbteams`, the chat lines and syncs the rules ask for, and the client's copy of the teams.
//
// Server hooks run on the thread of the `Game` that simulates the world and client hooks on the window's, so each
// side's state is kept per thread: the server's is read when a world opens and dropped when it closes.

use super::{
    Game,
    native_lifecycle::WorldPhase,
    native_server::{NativeCommand, NativeServerPlayer},
};
use ftbteams_core::{
    Config, Manager, Out,
    manager::Event,
    commands::{self, Ctx, Sender},
    ftblib::uuid_string,
    msg::Msg,
    sync::{ClientManager, PRESENCE_CHANNEL, SYNC_CHANNEL},
};
use horizon_client::text::{ClickEvent, Style, Text};
use std::{
    cell::RefCell,
    hash::{BuildHasher, Hasher},
    path::PathBuf,
};

struct Server {
    manager: Manager,
    /// The world's folder; None for a world that isn't saved.
    dir: Option<PathBuf>,
}

thread_local! {
    static SERVER: RefCell<Option<Server>> = const { RefCell::new(None) };
    static CLIENT: RefCell<ClientManager> = RefCell::new(ClientManager::default());
    /// FTB Teams' events (`TeamEvent`) for FTB Chunks, which takes them each server tick; kept short when nobody does.
    static EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
}

/// The team events since the last call (FTB Chunks' `transferClaims`, limits and tickets follow them).
#[allow(dead_code, reason = "FTB Chunks' bundle takes them; FTB Teams alone has no reader")]
pub(super) fn take_events() -> Vec<Event> {
    EVENTS.with(|e| std::mem::take(&mut *e.borrow_mut()))
}

/// The server's teams, for the other FTB bundles (FTB Chunks): None where no world is open on this thread.
pub(super) fn with_manager<R>(f: impl FnOnce(&mut Manager) -> R) -> Option<R> {
    SERVER.with(|s| s.borrow_mut().as_mut().map(|s| {
        s.manager.now_ms = now_ms();
        f(&mut s.manager)
    }))
}

/// The teams as this window's client knows them.
pub(super) fn with_client<R>(f: impl FnOnce(&ClientManager) -> R) -> R {
    CLIENT.with(|c| f(&c.borrow()))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

fn seed() -> u64 {
    std::collections::hash_map::RandomState::new().build_hasher().finish()
}

/// Java's development environment, where `/ftbteams_add_fake_player` is: `FTBTEAMS_DEV=1` (the world tests).
fn dev() -> bool {
    std::env::var("FTBTEAMS_DEV").is_ok_and(|v| v == "1")
}

/// The rules' component as the game's text.
fn text(msg: &Msg) -> Text {
    match msg {
        Msg::Literal(s) => Text::literal(s.clone()),
        Msg::Translate { key, en, args } => {
            Text::translate_with_fallback(key, en, &args.iter().map(text).collect::<Vec<_>>())
        }
        Msg::Seq(items) => items.iter().fold(Text::empty(), |t, m| t.append(text(m))),
        Msg::Styled(inner, s) => {
            let click = s.run.clone().map(ClickEvent::RunCommand).or_else(|| s.copy.clone().map(ClickEvent::CopyToClipboard));
            let style = Style {
                color: s.color,
                italic: s.italic.then_some(true),
                click,
                hover: s.hover.as_deref().map(|h| Box::new(text(h))),
                ..Style::default()
            };
            text(inner).with_style(&style)
        }
    }
}

fn save(server: &mut Server) {
    let Some(dir) = &server.dir else { return };
    if let Err(e) = server.manager.save(dir) {
        log::warn!("[ftbteams] {}: {e}", dir.display());
    }
}

/// Carries out what the rules asked for: chat lines, syncs, presence, a save after a creation or a deletion.
fn flush(game: &mut Game<'_>) {
    let Some((out, parts)) = SERVER.with(|s| {
        let mut s = s.borrow_mut();
        let server = s.as_mut()?;
        if server.manager.save_now {
            save(server);
        }
        let m = &mut server.manager;
        let out = std::mem::take(&mut m.out);
        let online: Vec<u128> = game.native_server_players().iter().map(|p| p.uuid).collect();
        let mut parts: Vec<(u128, &'static str, Vec<u8>)> = Vec::new();
        for o in &out {
            match o {
                Out::Sync(teams) => {
                    for p in &online {
                        parts.extend(m.sync_for(*p, Some(teams)).into_iter().map(|d| (*p, SYNC_CHANNEL, d)));
                    }
                }
                Out::SyncAll(p) => parts.extend(m.sync_for(*p, None).into_iter().map(|d| (*p, SYNC_CHANNEL, d))),
                Out::Presence(who) => {
                    if let Some(d) = m.presence_for(*who) {
                        parts.extend(online.iter().map(|p| (*p, PRESENCE_CHANNEL, d.clone())));
                    }
                }
                Out::Tell(..) | Out::Event(_) => {}
            }
        }
        Some((out, parts))
    }) else {
        return;
    };
    for o in out {
        match o {
            Out::Tell(player, msg) => {
                game.native_tell(player, text(&msg));
            }
            Out::Event(e) => EVENTS.with(|q| {
                let mut q = q.borrow_mut();
                if q.len() >= 4096 {
                    q.remove(0);
                }
                q.push(e);
            }),
            _ => {}
        }
    }
    for (player, channel, data) in parts {
        if let Err(e) = game.native_net_send_to_player(player, channel, &data) {
            log::debug!("[ftbteams] {channel} to {player:032x}: {e}");
        }
    }
}

// ---- The world ---------------------------------------------------------------------------------------------

/// `native_world_lifecycle`: the teams are read as a world opens and written and dropped as it closes.
pub(super) fn lifecycle(game: &mut Game<'_>, phase: WorldPhase) {
    match phase {
        WorldPhase::WorldOpenReady => {
            let dir = game.native_world_dir();
            let Some(world) = dir.clone() else { return };
            let mut manager = Manager::load(&world, seed());
            let config_dir = Game::native_config_dir().ok();
            manager.config = Config::load(config_dir.as_deref(), Some(&world));
            log::info!("[ftbteams] loaded team data: {} known players, {} teams total", manager.known.len(), manager.teams.len());
            SERVER.with(|s| *s.borrow_mut() = Some(Server { manager, dir }));
        }
        WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed | WorldPhase::WorldOpenFailed => {
            SERVER.with(|s| {
                if let Some(server) = s.borrow_mut().as_mut() {
                    save(server);
                }
                *s.borrow_mut() = None;
            });
            CLIENT.with(|c| *c.borrow_mut() = ClientManager::default());
        }
    }
}

/// `native_world_saved`: FTB Teams writes its changed files on each level save.
pub(super) fn saved() {
    SERVER.with(|s| {
        if let Some(server) = s.borrow_mut().as_mut() {
            save(server);
        }
    });
}

/// `native_player_logged_in`: a first login makes the player's team; every login gets the teams.
pub(super) fn logged_in(game: &mut Game<'_>, player: u128, name: &str) {
    if with_manager(|m| m.player_logged_in(player, name, true)).is_some() {
        flush(game);
    }
}

pub(super) fn logged_out(game: &mut Game<'_>, player: u128) {
    if with_manager(|m| m.player_logged_out(player)).is_some() {
        flush(game);
    }
}

// ---- /ftbteams ---------------------------------------------------------------------------------------------

/// `native_register_commands`.
pub(super) fn register(commands: &mut Vec<NativeCommand>) {
    for (usage, level) in commands::usages(dev()) {
        commands.push(if level == 0 { NativeCommand::new(usage) } else { NativeCommand::op(usage, level) });
    }
    if dev() {
        commands.push(NativeCommand::op("ftbteams_probe", 2));
    }
}

/// `native_perform_command`: `/ftbteams ...`, and `/ftbteams_probe` (tests) answering `FTBT {json}`.
pub(super) fn perform(game: &mut Game<'_>, player: &NativeServerPlayer, command: &str, ran: &mut bool) {
    if *ran {
        return;
    }
    let first = command.split_whitespace().next().unwrap_or_default();
    if first == "ftbteams_probe" && dev() {
        *ran = true;
        let probe = with_manager(|m| probe(m)).unwrap_or_else(|| serde_json::json!({ "error": "no teams on this side" }));
        let dir = SERVER.with(|s| s.borrow().as_ref().and_then(|s| s.dir.clone()));
        let mut probe = probe;
        probe["dir"] = serde_json::json!(dir.map(|d| d.display().to_string()));
        game.native_feedback(Text::literal(format!("FTBT {probe}")));
        return;
    }
    if first != "ftbteams" && first != "ftbteams_add_fake_player" {
        return;
    }
    let sender = Sender { uuid: player.uuid, name: player.name.clone(), level: player.permission, player: true };
    let reply = {
        let game: &Game<'_> = game;
        let lookup = |name: &str| game.native_profile_by_name(name);
        let ctx = Ctx { sender, singleplayer: false, lookup: &lookup, dev: dev() };
        with_manager(|m| commands::perform(m, &ctx, command)).flatten()
    };
    let Some(reply) = reply else { return };
    *ran = true;
    let last = reply.lines.len().saturating_sub(1);
    for (i, line) in reply.lines.iter().enumerate() {
        let line = text(line);
        game.native_feedback(if reply.failed && i == last { line.with_color(0xFF5555) } else { line });
    }
    flush(game);
}

fn probe(m: &Manager) -> serde_json::Value {
    let teams: Vec<serde_json::Value> = m
        .teams
        .values()
        .map(|t| {
            let ranks: serde_json::Map<String, serde_json::Value> =
                t.ranks.iter().map(|(id, r)| (uuid_string(*id), r.name().into())).collect();
            serde_json::json!({
                "id": uuid_string(t.id),
                "type": t.kind.name(),
                "short_name": t.short_name(),
                "owner": uuid_string(t.owner),
                "display_name": t.props.display_name,
                "description": t.props.description,
                "color": ftbteams_core::ftblib::color::to_string(t.props.color),
                "free_to_join": t.props.free_to_join,
                "ranks": ranks,
                "history": t.history.len(),
            })
        })
        .collect();
    let players: Vec<serde_json::Value> = m
        .known
        .iter()
        .map(|(id, k)| serde_json::json!({ "id": uuid_string(*id), "name": k.name, "online": k.online, "team": uuid_string(k.effective) }))
        .collect();
    serde_json::json!({
        "manager": uuid_string(m.id),
        "teams": teams,
        "players": players,
        "chat_redirected": m.chat_redirected.iter().map(|id| uuid_string(*id)).collect::<Vec<_>>(),
        "config": { "limited_lives": m.config.limited_lives, "max_party_size": m.config.max_party_size },
    })
}

// ---- The client ---------------------------------------------------------------------------------------------

/// `native_net_from_server`: the teams the server sends.
pub(super) fn from_server(channel: &str, data: &[u8]) {
    let result = match channel {
        SYNC_CHANNEL => CLIENT.with(|c| c.borrow_mut().apply(data)),
        PRESENCE_CHANNEL => CLIENT.with(|c| c.borrow_mut().apply_presence(data)),
        _ => return,
    };
    if let Err(e) = result {
        log::warn!("[ftbteams] a bad {channel} message: {e}");
    }
}

/// `native_commands`: the client's probe.
pub(super) fn client_commands(commands: &mut Vec<&'static str>) {
    if dev() {
        commands.push("ftbteamsclient");
    }
}

/// `native_command`: `/ftbteamsclient` puts `FTBTC {json}` in chat, what this client knows (tests).
pub(super) fn client_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    if *ran || !dev() || command.split_whitespace().next() != Some("ftbteamsclient") {
        return;
    }
    *ran = true;
    let json = with_client(|c| {
        let teams: Vec<serde_json::Value> = c
            .teams
            .iter()
            .map(|(id, t)| serde_json::json!({ "id": uuid_string(*id), "type": t.kind, "owner": t.owner, "properties": t.properties, "ranks": t.ranks }))
            .collect();
        let players: Vec<serde_json::Value> = c.players.values().map(|p| serde_json::json!(p)).collect();
        serde_json::json!({
            "manager": c.manager.map(uuid_string),
            "self": c.self_team.map(uuid_string),
            "teams": teams,
            "players": players,
        })
    });
    game.native_chat(&format!("FTBTC {json}"));
}
