// FTB Chunks in the game (support module `crate::game::ftbchunks_world` of the `ftbchunks` native bundle). The
// rules are the bundle's crate `ftbchunks_core`; this module is the glue: the teams from FTB Teams' bundle
// (`super::ftbteams_world`), the world's `ftbchunks/<team>.snbt` files, `/ftbchunks`, the protection hooks and the
// force-load tickets (`native_force_chunk_in`, owned by `ftbchunks:<team>`, so `/forceload` stays vanilla's).

use super::{Game, ftbteams_world};
use mod_api::{Command, CommandArgumentValue, CommandInvocation, CommandSource, EntityRef, PlayerRef, WorldPhase};
use ftbchunks_core::{
    Actor, ChunkPos, Chunks, Claim, Config, Kind, MemberData, Protection, TeamData, TeamView, Teams, Uuid, chunk_of,
    commands::{self, Ctx, Msg},
    net, regions, waypoints,
};
use ftbteams_core::{
    Manager, PropKind, PropertyDef,
    ftblib::{
        parse_uuid,
        snbt::{self, Compound, Value},
        uuid_string,
    },
    manager::Event,
    team::{Rank, Team, TeamType},
};
use horizon_client::text::Text;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

struct Server {
    chunks: Chunks,
    dir: Option<PathBuf>,
    /// The tickets held now, by team.
    held: BTreeSet<(Uuid, ChunkPos)>,
    /// Ticks until the tickets are checked again; 0: at the next tick.
    resync_in: u32,
    /// When each player was last told an action was refused (2 s apart).
    noticed: BTreeMap<Uuid, i64>,
    /// The claims as the clients were last told them (`send_many_chunks`).
    sent: BTreeMap<ChunkPos, Claim>,
    /// Players who logged in and get the whole picture at the next tick (`login_data`).
    logins: Vec<Uuid>,
    /// Claims may have changed since `sent`: tell the clients at the next tick.
    publish: bool,
    /// FTB Chunks' team properties are on FTB Teams' manager (done at the first tick of the world).
    registered: bool,
    /// Server ticks since the world opened, and when the claims last expired (`ClaimExpirationManager`).
    ticks: u64,
    last_sweep: i64,
    /// A sweep at the next tick whatever the schedule says (the dev command).
    sweep_now: bool,
    /// When a player who hit a player where it is not allowed was last told (3 s apart).
    pvp_noticed: BTreeMap<Uuid, i64>,
}

thread_local! {
    static SERVER: RefCell<Option<Server>> = const { RefCell::new(None) };
}

const BYPASS: &str = "BypassFTBChunksProtection";

/// A server player as the rules read them (the fork's `NativeServerPlayer`), from upstream's typed reads of a
/// `PlayerRef`.
#[derive(Clone, Debug)]
struct Sp {
    uuid: Uuid,
    name: String,
    permission: u8,
    dimension: String,
    position: [f64; 3],
    yaw: f32,
    pitch: f32,
    fake: bool,
    spectator: bool,
    entity_id: i32,
    health: f32,
}

fn sp(game: &Game<'_>, p: PlayerRef) -> Option<Sp> {
    let entity = p.entity();
    let [yaw, pitch] = game.entity_rotation(entity).unwrap_or_default();
    Some(Sp {
        uuid: p.uuid(),
        name: game.player_name(p).unwrap_or_default(),
        permission: game.player_permission_level(p).unwrap_or(0),
        dimension: game.player_dimension(p)?.to_string(),
        position: game.entity_position(entity)?,
        yaw,
        pitch,
        fake: game.player_is_fake(p).unwrap_or(false),
        spectator: game.player_is_spectator(p).unwrap_or(false),
        entity_id: i32::try_from(entity.id()).unwrap_or(0),
        health: game.living_health(mod_api::LivingRef::new(entity)).unwrap_or(1.0),
    })
}

/// The players on the server.
fn server_players(game: &Game<'_>) -> Vec<Sp> {
    game.native_server_players().into_iter().filter_map(|p| sp(game, p)).collect()
}

/// The player whose action a hook is asked about.
fn acting(game: &Game<'_>) -> Option<Sp> {
    sp(game, game.native_acting_player()?)
}

fn server_player(game: &Game<'_>, uuid: Uuid) -> Option<Sp> {
    sp(game, game.fork_player(uuid)?)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

fn dev() -> bool {
    std::env::var("FTBTEAMS_DEV").is_ok_and(|v| v == "1")
}

fn text(msg: &Msg) -> Text {
    match msg {
        Msg::Lit(s) => Text::literal(s.clone()),
        Msg::Tr { key, en, args } => Text::translate_with_fallback(key, en, &args.iter().map(text).collect::<Vec<_>>()),
        Msg::Seq(items) => items.iter().fold(Text::empty(), |t, m| t.append(text(m))),
        Msg::Color(inner, rgb) => text(inner).with_color(*rgb),
    }
}

/// A property's value as `TeamProperty.toString` writes it.
fn value_text(v: &Value) -> String {
    match v {
        Value::Bool(b) => b.to_string(),
        Value::Byte(n) => n.to_string(),
        Value::Short(n) => n.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Long(n) => n.to_string(),
        Value::Float(n) => n.to_string(),
        Value::Double(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::List(items) => items.iter().map(value_text).collect::<Vec<_>>().join(","),
        Value::Compound(_) => String::new(),
    }
}

/// FTB Teams' manager as FTB Chunks reads it.
struct View<'a> {
    m: &'a Manager,
    config: &'a Config,
}

impl View<'_> {
    fn view(&self, t: &Team) -> TeamView {
        let mut props = self.config.prop_defaults.clone();
        for (id, value) in &t.props.other {
            if id.starts_with("ftbchunks:") {
                props.set(id, &value_text(value));
            }
        }
        TeamView {
            id: t.id,
            kind: match t.kind {
                TeamType::Player => Kind::Player,
                TeamType::Party => Kind::Party,
                TeamType::Server => Kind::Server,
            },
            owner: t.owner,
            name: t.props.display_name.clone(),
            color: t.props.color & 0xFF_FFFF,
            members: t.members(),
            allies: t.ranks.iter().filter(|(_, r)| **r == Rank::Ally).map(|(id, _)| *id).collect(),
            props,
        }
    }
}

impl Teams for View<'_> {
    fn team(&self, id: Uuid) -> Option<TeamView> {
        self.m.teams.get(&id).map(|t| self.view(t))
    }
    fn team_of(&self, player: Uuid) -> Option<TeamView> {
        self.m.team_of(player).map(|t| self.view(t))
    }
    fn find(&self, arg: &str) -> Option<TeamView> {
        let t = self
            .m
            .team_by_short_name(arg)
            .or_else(|| parse_uuid(arg).and_then(|id| self.m.teams.get(&id)))
            .or_else(|| self.m.teams.values().find(|t| t.props.display_name == arg))
            .or_else(|| self.m.known_by_name(arg).and_then(|(id, _)| self.m.team_of(id)))?;
        Some(self.view(t))
    }
    fn player(&self, name: &str) -> Option<(Uuid, String)> {
        self.m.known_by_name(name)
    }
    fn online(&self, player: Uuid) -> bool {
        self.m.is_online(player)
    }
}

/// Runs `f` over the claims and the teams; None where no world is open on this thread.
fn with<R>(f: impl FnOnce(&mut Server, &View<'_>) -> R) -> Option<R> {
    SERVER.with(|s| {
        let mut s = s.borrow_mut();
        let server = s.as_mut()?;
        ftbteams_world::with_manager(|m| {
            let config = server.chunks.config.clone();
            f(server, &View { m, config: &config })
        })
    })
}

// ---- Files --------------------------------------------------------------------------------------------------

fn chunk_list(list: &[Value]) -> Vec<(i32, i32, &Compound)> {
    list.iter()
        .filter_map(|v| match v {
            Value::Compound(c) => Some((c.get("x")?.as_i64()? as i32, c.get("z")?.as_i64()? as i32, c)),
            _ => None,
        })
        .collect()
}

fn read_team(chunks: &mut Chunks, team: Uuid, c: &Compound) {
    let int = |c: &Compound, k: &str, or: i64| c.get(k).and_then(Value::as_i64).unwrap_or(or);
    let mut data = TeamData {
        max_claims: int(c, "max_claim_chunks", chunks.config.max_claimed_chunks as i64) as i32,
        max_force_loads: int(c, "max_force_load_chunks", chunks.config.max_force_loaded_chunks as i64) as i32,
        extra_claims: int(c, "extra_claim_chunks", 0) as i32,
        extra_force_loads: int(c, "extra_force_load_chunks", 0) as i32,
        last_login: int(c, "last_login_time", 0),
        members: BTreeMap::new(),
    };
    if let Some(dims) = c.compound("chunks") {
        for (dim, list) in &dims.0 {
            let Value::List(list) = list else { continue };
            for (x, z, e) in chunk_list(list) {
                let claim = Claim { team, time: int(e, "time", 0), force_loaded: int(e, "force_loaded", 0), expiry: int(e, "expiry_time", 0) };
                chunks.claims.insert((dim.clone(), x, z), claim);
            }
        }
    }
    if let Some(members) = c.compound("member_data") {
        for (id, m) in &members.0 {
            let (Some(id), Value::Compound(m)) = (parse_uuid(id), m) else { continue };
            let mut original = BTreeSet::new();
            if let Some(dims) = m.compound("original_claims") {
                for (dim, list) in &dims.0 {
                    if let Value::List(list) = list {
                        original.extend(chunk_list(list).into_iter().map(|(x, z, _)| (dim.clone(), x, z)));
                    }
                }
            }
            let member = MemberData {
                max_claims: int(m, "max_claimed_chunks", data.max_claims as i64) as i32,
                max_force_loads: int(m, "max_force_loaded_chunks", data.max_force_loads as i64) as i32,
                offline_force_loader: matches!(m.get("offline_force_loader"), Some(Value::Bool(true))),
                original_claims: original,
            };
            data.members.insert(id, member);
        }
    }
    chunks.teams.insert(team, data);
}

fn by_dimension<'a>(chunks: impl Iterator<Item = (&'a ChunkPos, Option<&'a Claim>)>) -> Compound {
    let mut dims: BTreeMap<&str, Vec<Value>> = BTreeMap::new();
    for (pos, claim) in chunks {
        let mut e = Compound::default();
        e.put("x", Value::Int(pos.1));
        e.put("z", Value::Int(pos.2));
        if let Some(c) = claim {
            e.put("time", Value::Long(c.time));
            if c.force_loaded > 0 {
                e.put("force_loaded", Value::Long(c.force_loaded));
            }
            if c.expiry > 0 {
                e.put("expiry_time", Value::Long(c.expiry));
            }
        }
        dims.entry(pos.0.as_str()).or_default().push(Value::Compound(e));
    }
    Compound(dims.into_iter().map(|(d, l)| (d.to_owned(), Value::List(l))).collect())
}

/// `ChunkTeamDataImpl.serializeNBT`.
fn write_team(chunks: &Chunks, team: Uuid, party: bool) -> Compound {
    let d = chunks.teams.get(&team).cloned().unwrap_or(TeamData {
        max_claims: chunks.config.max_claimed_chunks,
        max_force_loads: chunks.config.max_force_loaded_chunks,
        ..TeamData::default()
    });
    let mut c = Compound::default();
    c.put("max_claim_chunks", Value::Int(d.max_claims));
    c.put("max_force_load_chunks", Value::Int(d.max_force_loads));
    if !party && d.extra_claims > 0 {
        c.put("extra_claim_chunks", Value::Int(d.extra_claims));
    }
    if !party && d.extra_force_loads > 0 {
        c.put("extra_force_load_chunks", Value::Int(d.extra_force_loads));
    }
    c.put("last_login_time", Value::Long(d.last_login));
    c.put("chunks", Value::Compound(by_dimension(chunks.claims_of(team).map(|(p, c)| (p, Some(c))))));
    let mut members = Compound::default();
    for (id, m) in &d.members {
        let mut e = Compound::default();
        e.put("max_claimed_chunks", Value::Int(m.max_claims));
        e.put("max_force_loaded_chunks", Value::Int(m.max_force_loads));
        e.put("offline_force_loader", Value::Bool(m.offline_force_loader));
        e.put("original_claims", Value::Compound(by_dimension(m.original_claims.iter().map(|p| (p, None)))));
        members.put(&uuid_string(*id), Value::Compound(e));
    }
    c.put("member_data", Value::Compound(members));
    c
}

/// Writes the changed teams' files; a team FTB Teams no longer has, with nothing claimed, loses its file. Without
/// the teams (FTB Teams closed its world first) nothing is deleted.
fn save(server: &mut Server, m: Option<&Manager>) {
    let Some(dir) = &server.dir else { return };
    let dir = dir.join("ftbchunks");
    for team in std::mem::take(&mut server.chunks.dirty) {
        let path = dir.join(format!("{}.snbt", uuid_string(team)));
        let known = m.and_then(|m| m.teams.get(&team));
        let party = known.map_or_else(|| server.chunks.teams.get(&team).is_some_and(|d| d.members.len() > 1), |t| t.kind == TeamType::Party);
        let result = if m.is_some() && known.is_none() && server.chunks.claimed_by(team) == 0 {
            std::fs::remove_file(&path).or_else(|e| if e.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(e) })
        } else {
            std::fs::create_dir_all(&dir).and_then(|_| {
                snbt::write_file(&path, &write_team(&server.chunks, team, party))
            })
        };
        if let Err(e) = result {
            log::warn!("[ftbchunks] {}: {e}", path.display());
            server.chunks.dirty.insert(team);
        }
    }
}

/// `ftbchunks-world.snbt`: the instance's `config/` under the world's `serverconfig/`.
fn load_config(instance: Option<&Path>, world: &Path) -> Config {
    let mut config = Config::default();
    fn apply(config: &mut Config, c: &Compound) {
        for (key, value) in &c.0 {
            match value {
                Value::Compound(group) => apply(config, group),
                Value::List(items) => {
                    config.set_list(key, items.iter().map(value_text).collect());
                }
                v => {
                    if !config.set(key, &value_text(v)) {
                        log::debug!("[ftbchunks] config key {key} is not read");
                    }
                }
            }
        }
    }
    let files = [instance.map(|d| d.join("ftbchunks-world.snbt")), Some(world.join("serverconfig/ftbchunks-world.snbt"))];
    for path in files.into_iter().flatten() {
        if let Some(c) = snbt::read_file(&path) {
            apply(&mut config, &c);
        }
    }
    config
}

// ---- The world ----------------------------------------------------------------------------------------------

/// `native_server_started` (NeoForge's `ServerStartedEvent`, where the fork's `WorldOpenReady` was): the claims are
/// read once the worlds are open, before any player joins.
pub(super) fn opened(game: &mut Game<'_>) {
    let Some(world) = game.native_world_dir() else { return };
    let config_dir = Game::native_config_dir().ok();
    let mut chunks = Chunks { config: load_config(config_dir.as_deref(), &world), ..Chunks::default() };
    if let Ok(entries) = std::fs::read_dir(world.join("ftbchunks")) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(id) = path.file_stem().and_then(|s| s.to_str()).and_then(parse_uuid) else { continue };
            if path.extension().is_some_and(|e| e == "snbt") {
                if let Some(c) = snbt::read_file(&path) {
                    read_team(&mut chunks, id, &c);
                }
            }
        }
    }
    log::info!("[ftbchunks] loaded {} claims of {} teams", chunks.claims.len(), chunks.teams.len());
    let server = Server {
        chunks,
        dir: Some(world),
        held: BTreeSet::new(),
        resync_in: 0,
        noticed: BTreeMap::new(),
        sent: BTreeMap::new(),
        logins: Vec::new(),
        publish: true,
        registered: false,
        ticks: 0,
        last_sweep: 0,
        sweep_now: false,
        pvp_noticed: BTreeMap::new(),
    };
    SPREAD_GUARD.with(|g| g.set((server.chunks.config.fire_spread_protection, server.chunks.config.flowing_fluid_protection)));
    SERVER.with(|s| *s.borrow_mut() = Some(server));
}

/// `native_world_lifecycle`: the claims are written and dropped as a world closes or fails to open.
pub(super) fn lifecycle(phase: WorldPhase) {
    if matches!(phase, WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed | WorldPhase::WorldOpenFailed) {
        saved_and(true);
    }
}

/// Saves, with the teams where FTB Teams still has them; `drop` forgets the world after.
fn saved_and(drop: bool) {
    SERVER.with(|s| {
        let mut s = s.borrow_mut();
        if let Some(server) = s.as_mut() {
            if ftbteams_world::with_manager(|m| save(server, Some(m))).is_none() {
                save(server, None);
            }
        }
        if drop {
            *s = None;
        }
    });
}

/// `native_world_saved`.
pub(super) fn saved() {
    saved_and(false);
}

/// `native_server_tick`: the team events (claims follow a player into a party and back), then the tickets.
pub(super) fn tick(game: &mut Game<'_>) {
    register_properties();
    remember_alive(game);
    tell_pvp(game);
    claim_notices(game);
    let events = ftbteams_world::take_events();
    let Some(changes) = with(|server, view| {
        let chunks = &mut server.chunks;
        for e in &events {
            match e {
                Event::Created { team, by } => {
                    if let Some(t) = view.team(*team).filter(|t| t.kind == Kind::Party) {
                        chunks.joined_party(&t, *by);
                    }
                }
                Event::JoinedParty { party, player } => {
                    if let Some(t) = view.team(*party) {
                        chunks.joined_party(&t, *player);
                    }
                }
                Event::LeftParty { party, player, deleted } => {
                    if let Some(me) = view.team(*player) {
                        chunks.left_party(*party, &me, *deleted);
                    }
                    if let Some(t) = view.team(*party) {
                        chunks.update_limits(&t);
                    }
                }
                Event::Deleted { team } => {
                    chunks.claims.retain(|_, c| c.team != *team);
                    chunks.teams.remove(team);
                    chunks.dirty.insert(*team);
                }
                Event::OwnershipTransferred { party, .. } => {
                    if let Some(t) = view.team(*party) {
                        chunks.update_limits(&t);
                    }
                }
                Event::PropertiesChanged { team } => {
                    if let Some(t) = view.team(*team) {
                        chunks.update_limits(&t);
                    }
                    // `claim_visibility` may have changed: the team's chunks go out again, to each as they may see them.
                    server.sent.retain(|_, c| c.team != *team);
                }
                Event::LoggedIn { team, .. } => {
                    if let Some(t) = view.team(*team) {
                        chunks.update_limits(&t);
                        if let Some(d) = chunks.teams.get_mut(team) {
                            d.last_login = now_ms();
                        }
                        chunks.dirty.insert(*team);
                    }
                }
                Event::Ally { .. } => {}
            }
        }
        server.ticks += 1;
        let now = now_ms();
        if std::mem::take(&mut server.sweep_now) || (server.ticks & 63 == 0 && now - server.last_sweep > SWEEP_EVERY_MS) {
            server.last_sweep = now;
            let out = server.chunks.expire(view, now);
            for (team, pos) in &out.expired_loads {
                log::info!("[ftbchunks] chunk {pos:?} of {} stops being kept loaded: its time ran out", uuid_string(*team));
            }
            for (team, n) in &out.idle_unclaimed {
                log::info!("[ftbchunks] {} was away too long: {n} claimed chunk(s) given up", uuid_string(*team));
            }
            for (team, n) in &out.idle_unforced {
                log::info!("[ftbchunks] {} was away too long: {n} force-loaded chunk(s) let go", uuid_string(*team));
            }
            if !out.is_empty() {
                server.resync_in = 0;
                server.publish = true;
            }
        }
        if !events.is_empty() {
            server.resync_in = 0;
            server.publish = true;
        }
        if server.resync_in > 0 {
            server.resync_in -= 1;
            return Vec::new();
        }
        server.resync_in = 100;
        let wanted = server.chunks.wanted_tickets(view);
        let mut changes: Vec<(Uuid, ChunkPos, bool)> = wanted.difference(&server.held).map(|(t, p)| (*t, p.clone(), true)).collect();
        changes.extend(server.held.difference(&wanted).map(|(t, p)| (*t, p.clone(), false)));
        server.held = wanted;
        changes
    }) else {
        return;
    };
    for (team, pos, hold) in changes {
        let owner = format!("ftbchunks:{}", uuid_string(team));
        if !game.native_force_chunk_in(&owner, &pos.0, [pos.1, pos.2], hold) {
            log::debug!("[ftbchunks] ticket {owner} {pos:?} hold={hold} changed nothing");
        }
    }
    publish(game);
}

/// `ClaimExpirationManager` looks at most this often (10 minutes), on a tick divisible by 64.
const SWEEP_EVERY_MS: i64 = 600_000;

/// `TeamCollectPropertiesEvent`: FTB Chunks' properties go on FTB Teams' manager, with the config's defaults.
fn register_properties() {
    let specs = SERVER.with(|s| s.borrow().as_ref().filter(|s| !s.registered).map(|s| s.chunks.config.prop_defaults.specs()));
    let Some(specs) = specs else { return };
    let done = ftbteams_world::with_manager(|m| {
        for spec in specs {
            let kind = match spec.kind {
                ftbchunks_core::SpecKind::Bool => PropKind::Bool,
                ftbchunks_core::SpecKind::List => PropKind::List,
                ftbchunks_core::SpecKind::Privacy => PropKind::Choice(vec!["allies", "private", "public"]),
            };
            m.register_property(PropertyDef {
                id: spec.id,
                kind,
                default: spec.default,
                name_key: spec.name_key,
                name_en: spec.name_en,
                player_editable: true,
            });
        }
    });
    if done.is_some() {
        SERVER.with(|s| {
            if let Some(server) = s.borrow_mut().as_mut() {
                server.registered = true;
            }
        });
    }
}

thread_local! {
    /// `fire_spread_protection`, `flowing_fluid_protection`: both off (the default) and a flow costs nothing.
    static SPREAD_GUARD: std::cell::Cell<(bool, bool)> = const { std::cell::Cell::new((false, false)) };
    /// The players' entity ids this session (the hooks that only know ids), as of the last tick.
    static BY_ENTITY: RefCell<BTreeMap<i32, Uuid>> = const { RefCell::new(BTreeMap::new()) };
    /// Players to tell that a hit was refused (the hook has no game to talk to).
    static PVP_TOLD: RefCell<Vec<Uuid>> = const { RefCell::new(Vec::new()) };
}

/// `native_block_spread`: fire or a fluid does not cross into another team's claim (`fire_spread_protection`,
/// `flowing_fluid_protection`).
pub(super) fn block_spread(
    dimension: &str,
    kind: &str,
    from: horizon_core::position::BlockPos,
    to: horizon_core::position::BlockPos,
    canceled: &mut bool,
) {
    let fire = kind == "fire";
    let (fire_on, fluid_on) = SPREAD_GUARD.with(|g| g.get());
    if *canceled || !(if fire { fire_on } else { fluid_on }) {
        return;
    }
    let refused = with(|server, view| {
        server.chunks.refuses_spread(view, dimension, fire, [from.x, from.y, from.z], [to.x, to.y, to.z])
    });
    *canceled = refused.unwrap_or(false);
}

/// `native_living_incoming_damage`: a player hurting a player (an arrow's shooter counts) where `pvp_mode` forbids it.
/// Positions are the players' blocks as of the last tick; the attacker is told at the next one.
pub(super) fn living_damage(victim: i32, kind: &str, causing: i32, canceled: &mut bool) {
    let always = SERVER.with(|s| s.borrow().as_ref().is_none_or(|s| s.chunks.config.pvp_mode == ftbchunks_core::PvpMode::Always));
    if *canceled || causing == 0 || kind != "minecraft:player" || always {
        return;
    }
    let ids = BY_ENTITY.with(|b| {
        let b = b.borrow();
        Some((*b.get(&causing)?, *b.get(&victim)?))
    });
    let Some((attacker, target)) = ids else { return };
    let spots = ALIVE.with(|a| {
        let a = a.borrow();
        Some((a.get(&attacker)?.clone(), a.get(&target)?.clone()))
    });
    let Some(((a_dim, a), (v_dim, v))) = spots else { return };
    let refused = with(|server, view| server.chunks.refuses_pvp(view, &chunk_of(&a_dim, a[0], a[2]), &chunk_of(&v_dim, v[0], v[2])));
    if refused == Some(true) {
        *canceled = true;
        PVP_TOLD.with(|t| t.borrow_mut().push(attacker));
    }
}

/// The players whose hit was refused where the hook had no game: told now.
fn tell_pvp(game: &mut Game<'_>) {
    let told: Vec<Uuid> = PVP_TOLD.with(|t| std::mem::take(&mut *t.borrow_mut()));
    for player in told {
        tell_pvp_to(game, player);
    }
}

/// The line a player gets for a refused hit, at most every 3 seconds.
fn tell_pvp_to(game: &mut Game<'_>, player: Uuid) {
    let now = now_ms();
    let due = SERVER.with(|s| {
        let mut s = s.borrow_mut();
        let Some(server) = s.as_mut() else { return false };
        let last = server.pvp_noticed.entry(player).or_insert(i64::MIN / 2);
        let due = now - *last >= 3000;
        if due {
            *last = now;
        }
        due
    });
    if due {
        game.native_action_bar(player, text(&commands::no_pvp()));
    }
}

fn resync_soon() {
    SERVER.with(|s| {
        if let Some(server) = s.borrow_mut().as_mut() {
            server.resync_in = 0;
            server.publish = true;
        }
    });
}

// ---- The clients' messages (slice 3) ------------------------------------------------------------------------

/// `native_player_logged_in`: the player gets `login_data`, the counters and every claim they may see at the next tick.
pub(super) fn logged_in(player: u128) {
    SERVER.with(|s| {
        if let Some(server) = s.borrow_mut().as_mut() {
            server.logins.push(player);
            server.publish = true;
        }
    });
}

fn json_bytes(value: &impl serde::Serialize) -> Vec<u8> {
    serde_json::to_vec(value).unwrap_or_default()
}

/// Tells the clients what changed since they were last told (`send_many_chunks`, `send_general_data`), and gives
/// players who just logged in the whole picture.
fn publish(game: &mut Game<'_>) {
    let online = server_players(game);
    let Some(out) = with(|server, view| {
        if !std::mem::take(&mut server.publish) {
            return Vec::new();
        }
        let chunks = &server.chunks;
        let config = &chunks.config;
        let now = now_ms();
        let changed = chunks.changed_since(&server.sent);
        let teams: BTreeSet<Uuid> = changed
            .iter()
            .filter_map(|p| chunks.claims.get(p).or_else(|| server.sent.get(p)).map(|c| c.team))
            .collect();
        let logins = std::mem::take(&mut server.logins);
        let all: Vec<ChunkPos> = chunks.claims.keys().cloned().collect();
        let mut out: Vec<(Uuid, &'static str, Vec<u8>)> = Vec::new();
        for p in &online {
            let visible = |team: Uuid| view.team(team).is_none_or(|t| net::sees_claims(config, &t, p.uuid));
            let own = view.team_of(p.uuid).map(|t| t.id);
            let fresh = logins.contains(&p.uuid);
            if fresh {
                let login = serde_json::json!({ "manager": uuid_string(view.m.id) });
                out.push((p.uuid, net::LOGIN_DATA, json_bytes(&login)));
            }
            let (positions, before) = if fresh { (&all, &BTreeMap::new()) } else { (&changed, &server.sent) };
            for m in chunks.many_chunks(positions, before, &visible, now) {
                out.push((p.uuid, net::MANY_CHUNKS, json_bytes(&m)));
            }
            if let Some(own) = own.filter(|t| fresh || teams.contains(t)) {
                out.push((p.uuid, net::GENERAL_DATA, json_bytes(&chunks.general_data(own))));
            }
        }
        server.sent = chunks.claims.clone();
        out
    }) else {
        return;
    };
    for (player, channel, data) in out {
        if let Err(e) = game.native_net_send_to_player(player, channel, &data) {
            log::debug!("[ftbchunks] {channel} to {player:032x}: {e}");
        }
    }
}

/// `native_net_from_client`: `request_chunk_change` from the claim screen, answered by `chunk_change_response`.
pub(super) fn from_client(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    match channel {
        regions::FORCE_LOAD_EXPIRY => return force_load_expiry(game, player, data),
        regions::TELEPORT_FROM_MAP => return teleport_from_map(game, player, data),
        _ => {}
    }
    if channel != net::CHUNK_CHANGE {
        return;
    }
    let Ok(request) = serde_json::from_slice::<net::ChangeRequest>(data) else {
        log::warn!("[ftbchunks] a bad {channel} message from {player:032x}");
        return;
    };
    let Some(sender) = server_player(game, player) else { return };
    let operator = sender.permission >= 2;
    let response = with(|server, view| {
        let asked = request.team.as_deref().and_then(net::parse_uuid_text).and_then(|id| view.team(id));
        let team = match asked {
            Some(t) if operator || t.is_member(player) => Some(t),
            _ => view.team_of(player),
        }?;
        let admin = request.admin && operator;
        let (response, changed) =
            server.chunks.change(&team, request.op, &sender.dimension, &request.chunks, admin, now_ms());
        if !changed.is_empty() {
            server.resync_in = 0;
            server.publish = true;
        }
        Some(response)
    })
    .flatten();
    let Some(response) = response else { return };
    if let Err(e) = game.native_net_send_to_player(player, net::CHANGE_RESPONSE, &json_bytes(&response)) {
        log::debug!("[ftbchunks] {} to {player:032x}: {e}", net::CHANGE_RESPONSE);
    }
    tick(game);
}

/// `update_force_load_expiry` (the claim screen's wheel): from a member of the claim's team, in its dimension.
fn force_load_expiry(game: &mut Game<'_>, player: u128, data: &[u8]) {
    let Ok(m) = serde_json::from_slice::<regions::ForceLoadExpiry>(data) else {
        log::warn!("[ftbchunks] a bad {} message from {player:032x}", regions::FORCE_LOAD_EXPIRY);
        return;
    };
    let Some(sender) = server_player(game, player) else { return };
    if sender.dimension != m.dim {
        return;
    }
    let changed = with(|server, view| {
        let pos = (m.dim.clone(), m.x, m.z);
        let changed = server.chunks.set_force_load_expiry(view, player, &pos, m.relative_ms, now_ms());
        if changed {
            server.resync_in = 0;
            server.publish = true;
        }
        changed
    });
    if changed == Some(true) {
        tick(game);
    }
}

/// `teleport_from_map` (T on the large map): an operator goes to that column, on its surface when the client had no
/// height there (FTB Chunks' fallback height where the server has none either).
fn teleport_from_map(game: &mut Game<'_>, player: u128, data: &[u8]) {
    let Ok(m) = serde_json::from_slice::<regions::TeleportFromMap>(data) else { return };
    let Some(sender) = server_player(game, player) else { return };
    if sender.permission < 2 {
        return;
    }
    let [x, y, z] = m.pos;
    let y = if m.unknown_y {
        game.native_surface_heights(&m.dim, &[(x, z)], mod_api::Heightmap::MotionBlockingNoLeaves)
            .ok()
            .and_then(|h| h.first().copied().flatten())
            .unwrap_or(regions::TELEPORT_FALLBACK_Y)
    } else {
        y + 1
    };
    let to = [f64::from(x) + 0.5, f64::from(y) + 0.1, f64::from(z) + 0.5];
    if !game.native_teleport_player(player, &m.dim, to, sender.yaw, sender.pitch) {
        log::debug!("[ftbchunks] teleport_from_map: {player:032x} to {to:?} in {} refused", m.dim);
    }
}

/// FTB Chunks' notice over the hotbar when a player walks into another chunk whose claim is another team's (its
/// name in its colour) or none (the wilderness line, dark green): real players only, as FTB Chunks' section event.
fn claim_notices(game: &mut Game<'_>) {
    let players = server_players(game);
    let lines = with(|server, view| {
        NOTICE.with(|n| {
            let mut n = n.borrow_mut();
            n.retain(|id, _| players.iter().any(|p| p.uuid == *id));
            let mut out = Vec::new();
            for p in players.iter().filter(|p| !p.fake) {
                let pos = chunk_of(&p.dimension, p.position[0].floor() as i32, p.position[2].floor() as i32);
                let owner = server.chunks.claims.get(&pos).map(|c| c.team);
                if let Some((was_at, was)) = n.insert(p.uuid, (pos.clone(), owner))
                    && was_at != pos
                    && was != owner
                {
                    let line = match owner.and_then(|t| view.team(t)) {
                        Some(t) => Text::literal(t.name.clone()).with_color(t.color),
                        None => Text::translate_with_fallback("ftbchunks.wilderness", "Unclaimed land", &[]).with_color(0x00AA00),
                    };
                    out.push((p.uuid, line));
                }
            }
            out
        })
    })
    .unwrap_or_default();
    for (uuid, line) in lines {
        game.native_action_bar(uuid, line);
    }
}

// ---- Protection ---------------------------------------------------------------------------------------------

fn bypass_of(m: &Manager, player: Uuid) -> bool {
    matches!(m.teams.get(&player).and_then(|t| t.extra.get(BYPASS)), Some(Value::Bool(true)))
}

fn actor(game: &Game<'_>, m: &Manager) -> Option<(Actor, Sp)> {
    let p = acting(game)?;
    let actor = Actor { uuid: p.uuid, name: p.name.clone(), fake: p.fake, spectator: p.spectator, bypass: bypass_of(m, p.uuid) };
    Some((actor, p))
}

/// The action-bar line for a refusal, at most every 2 seconds a player.
fn notify(game: &mut Game<'_>, player: Uuid, refusal: ftbchunks_core::Refusal) {
    let now = now_ms();
    let due = SERVER.with(|s| {
        let mut s = s.borrow_mut();
        let Some(server) = s.as_mut() else { return false };
        let last = server.noticed.entry(player).or_insert(i64::MIN / 2);
        let due = now - *last >= 2000;
        if due {
            *last = now;
        }
        due
    });
    if due {
        game.native_action_bar(player, text(&commands::refusal(refusal)));
    }
}

fn tagged_block(game: &Game<'_>, pos: [i32; 3], tag: &str, builtin: &[&str]) -> bool {
    let block = game.native_block(pos);
    builtin.contains(&block.as_str()) || game.native_block_name_in_tag(&block, tag).unwrap_or(false)
}

/// Whether the acting player may do `what` at block `pos` of their level; tells them when not.
fn check(game: &mut Game<'_>, pos: [i32; 3], what: Protection, whitelisted: bool) -> bool {
    let decided = with(|server, view| {
        let (actor, player) = actor(game, view.m)?;
        let chunk = chunk_of(&player.dimension, pos[0], pos[2]);
        server.chunks.refuses(view, &actor, &chunk, what, whitelisted).map(|r| (actor.uuid, r))
    })
    .flatten();
    match decided {
        Some((player, refusal)) => {
            notify(game, player, refusal);
            false
        }
        None => true,
    }
}

const EDIT_WHITELIST: &[&str] = &["gravestone:gravestone"];
const INTERACT_WHITELIST: &[&str] = &["minecraft:crafting_table", "minecraft:bell"];
const RIGHT_CLICK_BLACKLIST: &[&str] = &[
    "minecraft:bucket",
    "minecraft:water_bucket",
    "minecraft:lava_bucket",
    "minecraft:lead",
    "minecraft:name_tag",
    "minecraft:splash_potion",
    "minecraft:lingering_potion",
];
const RIGHT_CLICK_WHITELIST: &[&str] = &["minecraft:shield", "minecraft:firework_rocket", "minecraft:map"];

fn edit_whitelisted(game: &Game<'_>, pos: [i32; 3]) -> bool {
    tagged_block(game, pos, "ftbchunks:edit_whitelist", EDIT_WHITELIST)
}

/// `native_destroy_block` (BreakEvent) and `native_left_click_block`: EDIT_BLOCK.
pub(super) fn destroy_block(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if !*canceled {
        let white = edit_whitelisted(game, pos);
        *canceled = !check(game, pos, Protection::EditBlock, white);
    }
}

/// `native_right_click_block`: INTERACT_BLOCK at the clicked block.
pub(super) fn right_click_block(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if !*canceled {
        let white = tagged_block(game, pos, "ftbchunks:interact_whitelist", INTERACT_WHITELIST);
        *canceled = !check(game, pos, Protection::InteractBlock, white);
    }
}

/// `native_entity_place` (BlockEvent.EntityPlaceEvent): a player's placement is EDIT_BLOCK where the block lands.
pub(super) fn place(game: &mut Game<'_>, placer: Option<EntityRef>, at: [i32; 3], canceled: &mut bool) {
    if *canceled {
        return;
    }
    let by_player = placer.zip(game.native_acting_player()).is_some_and(|(e, p)| e.id() == p.entity().id());
    if by_player {
        *canceled = !check(game, at, Protection::EditBlock, false);
    }
}

thread_local! {
    /// `use_item` refused the use in progress: `take_item_refusal` answers it.
    static ITEM_REFUSED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether `use_item` refused the use the hook is answering (once).
pub(super) fn take_item_refusal() -> bool {
    ITEM_REFUSED.with(|r| r.replace(false))
}

/// `native_use_item`: RIGHT_CLICK_ITEM at the player's eyes; a refusal is answered by `take_item_refusal`.
pub(super) fn use_item(game: &mut Game<'_>, stack: &horizon_client::items::ItemStack) {
    ITEM_REFUSED.with(|r| r.set(false));
    if stack.count == 0 {
        return;
    }
    let item = stack.item.to_string();
    let in_tag = |tag: &str| game.native_server_item_in_tag(stack, tag).unwrap_or(false);
    let white = RIGHT_CLICK_WHITELIST.contains(&item.as_str()) || in_tag("ftbchunks:right_click_whitelist");
    let black = RIGHT_CLICK_BLACKLIST.contains(&item.as_str()) || in_tag("ftbchunks:right_click_blacklist");
    if !black || white {
        return;
    }
    let refused = with(|server, view| {
        let (actor, p) = actor(game, view.m)?;
        let eyes = [p.position[0].floor() as i32, (p.position[1] + 1.62).floor() as i32, p.position[2].floor() as i32];
        let chunk = chunk_of(&p.dimension, eyes[0], eyes[2]);
        server.chunks.refuses_item(view, &actor, &chunk, white, black).then_some(actor.uuid)
    })
    .flatten();
    if let Some(player) = refused {
        notify(game, player, ftbchunks_core::Refusal::Prevented);
        ITEM_REFUSED.with(|r| r.set(true));
    }
}

/// `native_fill_bucket`: EDIT_FLUID at the targeted block.
pub(super) fn fill_bucket(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if !*canceled {
        *canceled = !check(game, pos, Protection::EditFluid, false);
    }
}

fn player_block(game: &Game<'_>) -> Option<[i32; 3]> {
    let p = acting(game)?;
    Some(p.position.map(|v| v.floor() as i32))
}

/// `native_interact_entity` and `native_armor_stand_manipulate`: INTERACT_ENTITY at the player's position.
pub(super) fn interact_entity(game: &mut Game<'_>, kind: &str, stop: &mut bool) {
    if *stop {
        return;
    }
    let Some(at) = player_block(game) else { return };
    *stop = !check(game, at, Protection::InteractEntity, kind == "corpse:corpse");
}

/// The entities FTB Chunks guards against left clicks (`ATTACK_NONLIVING_ENTITY`): frames, paintings, boats, carts.
fn nonliving(kind: &str) -> bool {
    let name = kind.rsplit(':').next().unwrap_or(kind);
    matches!(name, "item_frame" | "glow_item_frame" | "painting" | "leash_knot" | "end_crystal")
        || name.ends_with("boat")
        || name.ends_with("raft")
        || name.contains("minecart")
}

/// `native_attack_entity`: a blow on a player is PvP (`pvp_mode`); on a non-living target ATTACK_NONLIVING_ENTITY.
pub(super) fn attack_entity(game: &mut Game<'_>, kind: &str, pos: [f64; 3], canceled: &mut bool) {
    if *canceled {
        return;
    }
    if kind == "minecraft:player" {
        if let Some(attacker) = acting(game).filter(|p| melee_refused(p, pos)).map(|p| p.uuid) {
            *canceled = true;
            tell_pvp_to(game, attacker);
        }
    } else if nonliving(kind) {
        *canceled = !check(game, pos.map(|v| v.floor() as i32), Protection::AttackNonLiving, false);
    }
}

/// Whether `attacker`'s blow on a player standing at `victim` is refused: either stands in a claim `pvp_mode` guards.
fn melee_refused(attacker: &Sp, victim: [f64; 3]) -> bool {
    let at = |v: [f64; 3]| chunk_of(&attacker.dimension, v[0].floor() as i32, v[2].floor() as i32);
    with(|server, view| server.chunks.refuses_pvp(view, &at(attacker.position), &at(victim))).unwrap_or(false)
}

/// `native_explosion_detonate`: claimed blocks of teams that don't allow explosions stay. Upstream's hook names no
/// source, so every explosion is taken as one with a known source (`protect_unknown_explosions` no longer exempts any).
pub(super) fn explosion(dimension: &str, blocks: &mut Vec<[i32; 3]>) {
    with(|server, view| server.chunks.filter_explosion(view, dimension, true, blocks));
}

/// `native_piston_move`: no block crosses into another team's claim (`piston_protection`).
pub(super) fn piston_move(
    dimension: &str,
    piston: horizon_core::position::BlockPos,
    direction: [i32; 3],
    push: &[horizon_core::position::BlockPos],
    destroy: &[horizon_core::position::BlockPos],
    canceled: &mut bool,
) {
    if *canceled {
        return;
    }
    let refused = with(|server, view| {
        let c = &server.chunks;
        if !c.config.piston_protection || c.config.disable_protection || c.claims.is_empty() {
            return false;
        }
        let at = |p: &horizon_core::position::BlockPos| [p.x, p.y, p.z];
        push.iter().any(|p| {
            let from = at(p);
            c.refuses_crossing(view, dimension, from, [from[0] + direction[0], from[1] + direction[1], from[2] + direction[2]])
        }) || destroy.iter().any(|d| c.refuses_crossing(view, dimension, at(&piston), at(d)))
    });
    *canceled = refused.unwrap_or(false);
}

// ---- /ftbchunks ---------------------------------------------------------------------------------------------

/// `native_register_commands`. Upstream's dispatcher parses a usage's arguments itself, and its `word` takes no `:`
/// (a dimension, a colour) or `~` (a position), which FTB's own argument types read: so each root is its name and one
/// optional `text` argument, the line the rules parse as Java's tree does (`commands::usages` and the operator
/// levels stay the rules', which check them).
pub(super) fn register(commands: &mut Vec<Command>) {
    commands.push(Command::new("ftbchunks [line:text]"));
    commands.push(Command::new("chunks [line:text]"));
    if dev() {
        commands.push(Command::op("ftbchunks_probe", 2));
        commands.push(Command::op("ftbchunks_probe_pvp <x:integer> <z:integer>", 2));
    }
}

/// The command line a parsed invocation of one of `register`'s usages was typed as: its words, then the arguments.
fn line(call: &CommandInvocation) -> String {
    let mut line = call.literals.join(" ");
    for argument in &call.arguments {
        let word = match &argument.value {
            CommandArgumentValue::String(text) => text.clone(),
            CommandArgumentValue::Integer(n) => n.to_string(),
            CommandArgumentValue::Number(n) => n.to_string(),
            CommandArgumentValue::Boolean(b) => b.to_string(),
            _ => continue,
        };
        line.push(' ');
        line.push_str(&word);
    }
    line
}

/// `native_run_mod_command`: `/ftbchunks ...` (`/chunks ...`), and `/ftbchunks_probe` (tests) answering `FTBC {json}`.
pub(super) fn perform(game: &mut Game<'_>, source: &CommandSource, call: &CommandInvocation, ran: &mut bool) {
    if *ran {
        return;
    }
    let CommandSource::Player { player, .. } = source else { return };
    let Some(player) = sp(game, *player) else { return };
    let player = &player;
    let command = line(call);
    let command = command.as_str();
    let first = command.split_whitespace().next().unwrap_or_default();
    if first == "ftbchunks_probe" && dev() {
        *ran = true;
        let probe = with(|server, view| probe(server, view)).unwrap_or_else(|| serde_json::json!({ "error": "no claims on this side" }));
        game.native_feedback(Text::literal(format!("FTBC {probe}")));
        return;
    }
    if first == "ftbchunks_probe_pvp" && dev() {
        // What a blow from this player on a player at block (x, z) meets, and a hit that names the player as its own shooter.
        *ran = true;
        let xz: Vec<f64> = command.split_whitespace().skip(1).filter_map(|w| w.parse().ok()).collect();
        let victim = [xz.first().copied().unwrap_or(0.0) + 0.5, 0.0, xz.get(1).copied().unwrap_or(0.0) + 0.5];
        let attack = melee_refused(player, victim);
        let mut shot = false;
        living_damage(player.entity_id, "minecraft:player", player.entity_id, &mut shot);
        PVP_TOLD.with(|t| t.borrow_mut().clear());
        game.native_feedback(Text::literal(format!("FTBP {}", serde_json::json!({ "attack": attack, "shot": shot }))));
        return;
    }
    if first != "ftbchunks" && first != "chunks" {
        return;
    }
    if let Some(add) = waypoints::parse_add(command, player.position, &player.dimension) {
        *ran = true;
        waypoint_add(game, player.uuid, add);
        return;
    }
    let reply = with(|server, view| {
        let ctx = Ctx {
            sender: player.uuid,
            name: player.name.clone(),
            level: player.permission,
            dimension: player.dimension.clone(),
            pos: player.position,
            bypass: bypass_of(view.m, player.uuid),
        };
        commands::perform(&mut server.chunks, view, &ctx, command, now_ms())
    })
    .flatten();
    let Some(reply) = reply else { return };
    *ran = true;
    if let Some(team) = reply.open_gui {
        let open = regions::OpenClaimGui { team: net::uuid_text(team) };
        if let Err(e) = game.native_net_send_to_player(player.uuid, regions::OPEN_CLAIM_GUI, &json_bytes(&open)) {
            log::debug!("[ftbchunks] {}: {e}", regions::OPEN_CLAIM_GUI);
        }
    }
    if let Some(on) = reply.bypass {
        ftbteams_world::with_manager(|m| {
            if let Some(t) = m.teams.get_mut(&player.uuid) {
                t.extra.put(BYPASS, Value::Bool(on));
                t.dirty = true;
            }
        });
    }
    let last = reply.lines.len().saturating_sub(1);
    for (i, line) in reply.lines.iter().enumerate() {
        let line = text(line);
        game.native_feedback(if reply.failed && i == last { line.with_color(commands::RED) } else { line });
    }
    resync_soon();
    tick(game);
}

/// `/ftbchunks waypoint add`: the waypoint goes to the player's client (`add_waypoint`), which keeps it.
fn waypoint_add(game: &mut Game<'_>, player: Uuid, add: Result<waypoints::AddWaypoint, String>) {
    let add = match add {
        Ok(add) => add,
        Err(arg) => {
            let line = Text::translate_with_fallback("command.unknown.argument", "Incorrect argument for command", &[]);
            game.native_feedback(line.append(Text::literal(format!(": {arg}"))).with_color(commands::RED));
            return;
        }
    };
    if let Err(e) = game.native_net_send_to_player(player, waypoints::ADD_WAYPOINT, &json_bytes(&add)) {
        log::debug!("[ftbchunks] {}: {e}", waypoints::ADD_WAYPOINT);
    }
    if !add.gui {
        let name = Text::literal(add.name);
        game.native_feedback(Text::translate_with_fallback("ftbchunks.command.waypoint_added", "Waypoint %s saved", &[name]));
    }
}

thread_local! {
    /// Where each living player was at the last tick (the server's player list is out of reach inside the death hook).
    static ALIVE: RefCell<BTreeMap<Uuid, (String, [i32; 3])>> = const { RefCell::new(BTreeMap::new()) };
    /// Where each player last died (`player_died`), told to their client when they respawn.
    static DEATHS: RefCell<BTreeMap<Uuid, (String, [i32; 3])>> = const { RefCell::new(BTreeMap::new()) };
    /// Each player's chunk and its claim's team as of the last tick (the notice on walking into another's claim).
    static NOTICE: RefCell<BTreeMap<Uuid, (ChunkPos, Option<Uuid>)>> = const { RefCell::new(BTreeMap::new()) };
}

/// Each tick: the block, and dimension, of every living player.
fn remember_alive(game: &Game<'_>) {
    let players = server_players(game);
    ALIVE.with(|a| {
        let mut a = a.borrow_mut();
        a.retain(|id, _| players.iter().any(|p| p.uuid == *id));
        for p in players.iter().filter(|p| p.health > 0.0) {
            a.insert(p.uuid, (p.dimension.clone(), p.position.map(|v| v.floor() as i32)));
        }
    });
    BY_ENTITY.with(|b| {
        let mut b = b.borrow_mut();
        b.clear();
        b.extend(players.iter().filter(|p| p.entity_id != 0).map(|p| (p.entity_id, p.uuid)));
    });
}

/// `native_player_died`: the spot (as of the last tick) is kept for the respawn.
pub(super) fn player_died(_game: &mut Game<'_>, player: Uuid) {
    let Some(spot) = ALIVE.with(|a| a.borrow().get(&player).cloned()) else { return };
    DEATHS.with(|d| d.borrow_mut().insert(player, spot));
}

/// `native_player_respawned`: FTB Chunks' `player_death` (sent when the dead player is cloned) with the death count.
pub(super) fn player_respawned(game: &mut Game<'_>, player: Uuid) {
    let Some((dim, pos)) = DEATHS.with(|d| d.borrow_mut().remove(&player)) else { return };
    let deaths = game.native_player_stat(player, "minecraft:custom", "minecraft:deaths").unwrap_or(1).max(1);
    let death = waypoints::PlayerDeath { dim, pos, deaths };
    if let Err(e) = game.native_net_send_to_player(player, waypoints::PLAYER_DEATH, &json_bytes(&death)) {
        log::debug!("[ftbchunks] {}: {e}", waypoints::PLAYER_DEATH);
    }
}

fn probe(server: &Server, view: &View<'_>) -> serde_json::Value {
    let c = &server.chunks;
    let mut ids: BTreeSet<Uuid> = c.teams.keys().copied().collect();
    ids.extend(c.claims.values().map(|cl| cl.team));
    let teams: Vec<serde_json::Value> = ids
        .iter()
        .map(|id| {
            let (claims, loads) = c.limits(*id);
            serde_json::json!({
                "id": uuid_string(*id),
                "name": view.team(*id).map(|t| t.name),
                "claimed": c.claimed_by(*id),
                "force_loaded": c.force_loaded_by(*id),
                "max_claims": claims,
                "max_force_loads": loads,
                "extra_claims": c.teams.get(id).map_or(0, |d| d.extra_claims),
            })
        })
        .collect();
    let claims: Vec<serde_json::Value> = c
        .claims
        .iter()
        .take(64)
        .map(|(p, cl)| serde_json::json!({ "dim": p.0, "x": p.1, "z": p.2, "team": uuid_string(cl.team), "force_loaded": cl.force_loaded > 0, "expiry": cl.expiry }))
        .collect();
    let held: Vec<serde_json::Value> =
        server.held.iter().map(|(t, p)| serde_json::json!({ "team": uuid_string(*t), "dim": p.0, "x": p.1, "z": p.2 })).collect();
    let bypass: Vec<String> = view.m.teams.keys().filter(|id| bypass_of(view.m, **id)).map(|id| uuid_string(*id)).collect();
    serde_json::json!({
        "dir": server.dir.as_ref().map(|d| d.display().to_string()),
        "teams": teams,
        "claims": claims,
        "total_claims": c.claims.len(),
        "held": held,
        "bypass": bypass,
        "config": {
            "max_claimed_chunks": c.config.max_claimed_chunks,
            "max_force_loaded_chunks": c.config.max_force_loaded_chunks,
            "pvp_mode": format!("{:?}", c.config.pvp_mode),
            "fire_spread_protection": c.config.fire_spread_protection,
            "flowing_fluid_protection": c.config.flowing_fluid_protection,
            "max_idle_days_before_unclaim": c.config.max_idle_days_before_unclaim,
            "max_idle_days_before_unforce": c.config.max_idle_days_before_unforce,
        },
        "properties": view.m.registry.len(),
        "sweeps": server.last_sweep > 0,
    })
}
