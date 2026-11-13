// FTB Chunks in the game (support module `crate::game::ftbchunks_world` of the `ftbchunks` native bundle). The
// rules are the bundle's crate `ftbchunks_core`; this module is the glue: the teams from FTB Teams' bundle
// (`super::ftbteams_world`), the world's `ftbchunks/<team>.snbt` files, `/ftbchunks`, the protection hooks and the
// force-load tickets (`native_force_chunk_in`, owned by `ftbchunks:<team>`, so `/forceload` stays vanilla's).

use super::{
    Game, ftbteams_world,
    native_lifecycle::WorldPhase,
    native_server::{NativeCommand, NativeServerPlayer},
};
use ftbchunks_core::{
    Actor, ChunkPos, Chunks, Claim, Config, Kind, MemberData, Protection, TeamData, TeamView, Teams, Uuid, chunk_of,
    commands::{self, Ctx, Msg},
};
use ftbteams_core::{
    Manager,
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
}

thread_local! {
    static SERVER: RefCell<Option<Server>> = const { RefCell::new(None) };
}

const BYPASS: &str = "BypassFTBChunksProtection";

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

/// `native_world_lifecycle`: the claims are read as a world opens, written and dropped as it closes.
pub(super) fn lifecycle(game: &mut Game<'_>, phase: WorldPhase) {
    match phase {
        WorldPhase::WorldOpenReady => {
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
            let server = Server { chunks, dir: Some(world), held: BTreeSet::new(), resync_in: 0, noticed: BTreeMap::new() };
            SERVER.with(|s| *s.borrow_mut() = Some(server));
        }
        WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed | WorldPhase::WorldOpenFailed => saved_and(true),
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
                Event::OwnershipTransferred { party, .. } | Event::PropertiesChanged { team: party } => {
                    if let Some(t) = view.team(*party) {
                        chunks.update_limits(&t);
                    }
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
        if !events.is_empty() {
            server.resync_in = 0;
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
}

fn resync_soon() {
    SERVER.with(|s| {
        if let Some(server) = s.borrow_mut().as_mut() {
            server.resync_in = 0;
        }
    });
}

// ---- Protection ---------------------------------------------------------------------------------------------

fn bypass_of(m: &Manager, player: Uuid) -> bool {
    matches!(m.teams.get(&player).and_then(|t| t.extra.get(BYPASS)), Some(Value::Bool(true)))
}

fn actor(game: &Game<'_>, m: &Manager) -> Option<(Actor, NativeServerPlayer)> {
    let p = game.native_acting_player()?;
    let actor = Actor { uuid: p.uuid, name: p.name.clone(), fake: p.fake, spectator: p.game_mode == 3, bypass: bypass_of(m, p.uuid) };
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

/// `native_use_item_on`: a placement (BlockEvent.EntityPlaceEvent) is EDIT_BLOCK where the block lands.
pub(super) fn use_item_on(game: &mut Game<'_>, pos: [i32; 3], face: [i32; 3], claimed: &mut bool) {
    if !*claimed {
        let at = [pos[0] + face[0], pos[1] + face[1], pos[2] + face[2]];
        *claimed = !check(game, at, Protection::EditBlock, false);
    }
}

/// `native_use_item`: RIGHT_CLICK_ITEM at the player's eyes.
pub(super) fn use_item(game: &mut Game<'_>, stack: &horizon_client::items::ItemStack, claimed: &mut bool) {
    if *claimed || stack.count == 0 {
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
        *claimed = true;
    }
}

/// `native_fill_bucket`: EDIT_FLUID at the targeted block.
pub(super) fn fill_bucket(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if !*canceled {
        *canceled = !check(game, pos, Protection::EditFluid, false);
    }
}

fn player_block(game: &Game<'_>) -> Option<[i32; 3]> {
    let p = game.native_acting_player()?;
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

/// `native_attack_entity`: for non-living targets.
pub(super) fn attack_entity(game: &mut Game<'_>, kind: &str, pos: [f64; 3], canceled: &mut bool) {
    if !*canceled && nonliving(kind) {
        *canceled = !check(game, pos.map(|v| v.floor() as i32), Protection::AttackNonLiving, false);
    }
}

/// `native_explosion_detonate`: claimed blocks of teams that don't allow explosions stay.
pub(super) fn explosion(dimension: &str, source: Option<(u32, &str)>, blocks: &mut Vec<[i32; 3]>) {
    with(|server, view| server.chunks.filter_explosion(view, dimension, source.is_some(), blocks));
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

/// `native_register_commands`.
pub(super) fn register(commands: &mut Vec<NativeCommand>) {
    for (usage, level) in commands::usages() {
        commands.push(if level == 0 { NativeCommand::new(&usage) } else { NativeCommand::op(&usage, level) });
    }
    if dev() {
        commands.push(NativeCommand::op("ftbchunks_probe", 2));
    }
}

/// `native_perform_command`: `/ftbchunks ...` (`/chunks ...`), and `/ftbchunks_probe` (tests) answering `FTBC {json}`.
pub(super) fn perform(game: &mut Game<'_>, player: &NativeServerPlayer, command: &str, ran: &mut bool) {
    if *ran {
        return;
    }
    let first = command.split_whitespace().next().unwrap_or_default();
    if first == "ftbchunks_probe" && dev() {
        *ran = true;
        let probe = with(|server, view| probe(server, view)).unwrap_or_else(|| serde_json::json!({ "error": "no claims on this side" }));
        game.native_feedback(Text::literal(format!("FTBC {probe}")));
        return;
    }
    if first != "ftbchunks" && first != "chunks" {
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
        .map(|(p, cl)| serde_json::json!({ "dim": p.0, "x": p.1, "z": p.2, "team": uuid_string(cl.team), "force_loaded": cl.force_loaded > 0 }))
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
        "config": { "max_claimed_chunks": c.config.max_claimed_chunks, "max_force_loaded_chunks": c.config.max_force_loaded_chunks },
    })
}
