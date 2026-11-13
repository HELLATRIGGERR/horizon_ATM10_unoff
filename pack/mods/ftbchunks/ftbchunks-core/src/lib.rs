//! FTB Chunks 2101.1.22's server side as the pack's `ftbchunks` bundle runs it (slice 2 of docs/ftb-chunks.md):
//! every team's claimed and force-loaded chunks, the limits from its members, who may use a claim (`canPlayerUse`,
//! `isAlly`), the protection policy, the chunks whose tickets must be held, and `/ftbchunks` (`commands`).
//!
//! FTB Chunks is All Rights Reserved (FTB): this is a reimplementation of its behaviour; nothing of it is copied.
//! The crate stands alone (a bundle crate may not depend on another bundle's by path): the teams come in through
//! [`Teams`], built by the runtime module from FTB Teams' manager, and files are written by the runtime.

pub mod commands;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

pub type Uuid = u128;

/// A chunk of a dimension: the dimension's id and the chunk's x and z.
pub type ChunkPos = (String, i32, i32);

/// The chunk holding block `pos`.
pub fn chunk_of(dimension: &str, x: i32, z: i32) -> ChunkPos {
    (dimension.to_owned(), x >> 4, z >> 4)
}

/// FTB Teams' team kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Player,
    Party,
    Server,
}

/// A team property's `PrivacyMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Privacy {
    Allies,
    Private,
    Public,
}

impl Privacy {
    pub fn name(self) -> &'static str {
        match self {
            Privacy::Allies => "allies",
            Privacy::Private => "private",
            Privacy::Public => "public",
        }
    }
    pub fn from_name(name: &str) -> Option<Self> {
        [Privacy::Allies, Privacy::Private, Privacy::Public].into_iter().find(|p| p.name().eq_ignore_ascii_case(name))
    }
}

/// The team properties FTB Chunks adds to FTB Teams (`ftbchunks:<name>`).
#[derive(Clone, Debug, PartialEq)]
pub struct Props {
    pub allow_fake_players: bool,
    pub allow_fake_players_by_id: bool,
    pub allow_named_fake_players: Vec<String>,
    pub entity_interact_mode: Privacy,
    pub nonliving_entity_attack_mode: Privacy,
    pub allow_explosions: bool,
    pub allow_mob_griefing: bool,
    pub claim_visibility: Privacy,
    pub location_mode: Privacy,
    pub allow_pvp: bool,
    pub block_edit_mode: Privacy,
    pub block_interact_mode: Privacy,
}

impl Default for Props {
    /// The config's `team_prop_defaults` as FTB Chunks ships them.
    fn default() -> Self {
        Self {
            allow_fake_players: false,
            allow_fake_players_by_id: true,
            allow_named_fake_players: Vec::new(),
            entity_interact_mode: Privacy::Allies,
            nonliving_entity_attack_mode: Privacy::Allies,
            allow_explosions: false,
            allow_mob_griefing: false,
            claim_visibility: Privacy::Public,
            location_mode: Privacy::Allies,
            allow_pvp: true,
            block_edit_mode: Privacy::Allies,
            block_interact_mode: Privacy::Allies,
        }
    }
}

/// The ids of [`Props`], in FTB Chunks' order.
pub const PROPERTY_IDS: [&str; 12] = [
    "ftbchunks:allow_fake_players",
    "ftbchunks:allow_named_fake_players",
    "ftbchunks:allow_fake_players_by_id",
    "ftbchunks:entity_interact_mode",
    "ftbchunks:nonliving_entity_attack_mode",
    "ftbchunks:allow_explosions",
    "ftbchunks:allow_mob_griefing",
    "ftbchunks:claim_visibility",
    "ftbchunks:location_mode",
    "ftbchunks:allow_pvp",
    "ftbchunks:block_edit_mode",
    "ftbchunks:block_interact_mode",
];

fn bool_of(text: &str) -> Option<bool> {
    match text {
        "true" | "1b" | "1" => Some(true),
        "false" | "0b" | "0" => Some(false),
        _ => None,
    }
}

impl Props {
    /// Sets a property from its text (`true`, `allies`, a list's items split by commas); false when the id isn't
    /// one of these or the text doesn't parse.
    pub fn set(&mut self, id: &str, text: &str) -> bool {
        let name = id.strip_prefix("ftbchunks:").unwrap_or(id);
        let flag = |f: &mut bool| bool_of(text).map(|b| *f = b).is_some();
        let mode = |f: &mut Privacy| Privacy::from_name(text).map(|m| *f = m).is_some();
        match name {
            "allow_fake_players" => flag(&mut self.allow_fake_players),
            "allow_fake_players_by_id" => flag(&mut self.allow_fake_players_by_id),
            "allow_explosions" => flag(&mut self.allow_explosions),
            "allow_mob_griefing" => flag(&mut self.allow_mob_griefing),
            "allow_pvp" => flag(&mut self.allow_pvp),
            "entity_interact_mode" => mode(&mut self.entity_interact_mode),
            "nonliving_entity_attack_mode" => mode(&mut self.nonliving_entity_attack_mode),
            "claim_visibility" => mode(&mut self.claim_visibility),
            "location_mode" => mode(&mut self.location_mode),
            "block_edit_mode" => mode(&mut self.block_edit_mode),
            "block_interact_mode" => mode(&mut self.block_interact_mode),
            "allow_named_fake_players" => {
                self.allow_named_fake_players =
                    text.split(',').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect();
                true
            }
            _ => false,
        }
    }
}

/// `ally_mode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AllyMode {
    #[default]
    Default,
    ForcedAll,
    ForcedNone,
}

/// `party_limit_mode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PartyLimitMode {
    #[default]
    Largest,
    Owner,
    Sum,
    Average,
}

/// `force_load_mode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ForceLoadMode {
    #[default]
    Default,
    Always,
    Never,
}

/// `fake_players`, `ProtectionPolicy`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Policy {
    #[default]
    Check,
    Deny,
    Allow,
}

/// `pvp_mode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PvpMode {
    #[default]
    Always,
    Never,
    PerTeam,
}

/// `ftbchunks-world.snbt`, with FTB Chunks' defaults (ATM10 ships none).
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub disable_protection: bool,
    pub ally_mode: AllyMode,
    pub pvp_mode: PvpMode,
    pub no_wilderness: bool,
    pub no_wilderness_dimensions: Vec<String>,
    pub protect_unknown_explosions: bool,
    pub piston_protection: bool,
    pub flowing_fluid_protection: bool,
    pub fire_spread_protection: bool,
    pub fake_players: Policy,
    pub max_claimed_chunks: i32,
    pub claim_dimension_blacklist: Vec<String>,
    pub claim_dimension_whitelist: Vec<String>,
    pub max_idle_days_before_unclaim: f64,
    pub hard_team_claim_limit: i32,
    pub party_limit_mode: PartyLimitMode,
    pub force_load_mode: ForceLoadMode,
    pub max_force_loaded_chunks: i32,
    pub max_idle_days_before_unforce: f64,
    pub hard_team_force_limit: i32,
    /// `team_prop_defaults`: what a team's properties are until it sets them.
    pub prop_defaults: Props,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            disable_protection: false,
            ally_mode: AllyMode::Default,
            pvp_mode: PvpMode::Always,
            no_wilderness: false,
            no_wilderness_dimensions: Vec::new(),
            protect_unknown_explosions: true,
            piston_protection: true,
            flowing_fluid_protection: false,
            fire_spread_protection: false,
            fake_players: Policy::Check,
            max_claimed_chunks: 500,
            claim_dimension_blacklist: Vec::new(),
            claim_dimension_whitelist: Vec::new(),
            max_idle_days_before_unclaim: 0.0,
            hard_team_claim_limit: 0,
            party_limit_mode: PartyLimitMode::Largest,
            force_load_mode: ForceLoadMode::Default,
            max_force_loaded_chunks: 25,
            max_idle_days_before_unforce: 0.0,
            hard_team_force_limit: 0,
            prop_defaults: Props::default(),
        }
    }
}

/// `dimension` against a list of ids, `namespace:*` matching a namespace.
fn listed(list: &[String], dimension: &str) -> bool {
    list.iter().any(|d| d == dimension || d.strip_suffix('*').is_some_and(|ns| dimension.starts_with(ns)))
}

impl Config {
    /// `DimensionFilter.isDimensionOK`.
    pub fn may_claim_in(&self, dimension: &str) -> bool {
        !listed(&self.claim_dimension_blacklist, dimension)
            && (self.claim_dimension_whitelist.is_empty() || listed(&self.claim_dimension_whitelist, dimension))
    }
    /// One leaf of the config file (`max_claimed_chunks`, `def_allow_explosions`...), its keys being unique across
    /// the groups. False for an unknown key or a value that doesn't parse; a list comes through [`Config::set_list`].
    pub fn set(&mut self, key: &str, text: &str) -> bool {
        let text = text.trim();
        let text_raw = text;
        let int = |f: &mut i32| text.parse::<i32>().map(|v| *f = v).is_ok();
        let num = |f: &mut f64| text.parse::<f64>().map(|v| *f = v).is_ok();
        let flag = |f: &mut bool| match text {
            "true" | "1" => {
                *f = true;
                true
            }
            "false" | "0" => {
                *f = false;
                true
            }
            _ => false,
        };
        let upper = text_raw.trim_matches('"').to_ascii_uppercase();
        match key {
            "disable_protection" => flag(&mut self.disable_protection),
            "no_wilderness" => flag(&mut self.no_wilderness),
            "protect_unknown_explosions" => flag(&mut self.protect_unknown_explosions),
            "piston_protection" => flag(&mut self.piston_protection),
            "flowing_fluid_protection" => flag(&mut self.flowing_fluid_protection),
            "fire_spread_protection" => flag(&mut self.fire_spread_protection),
            "max_claimed_chunks" => int(&mut self.max_claimed_chunks),
            "max_force_loaded_chunks" => int(&mut self.max_force_loaded_chunks),
            "hard_team_claim_limit" => int(&mut self.hard_team_claim_limit),
            "hard_team_force_limit" => int(&mut self.hard_team_force_limit),
            "max_idle_days_before_unclaim" => num(&mut self.max_idle_days_before_unclaim),
            "max_idle_days_before_unforce" => num(&mut self.max_idle_days_before_unforce),
            "ally_mode" => {
                let v = match upper.as_str() {
                    "DEFAULT" => AllyMode::Default,
                    "FORCED_ALL" => AllyMode::ForcedAll,
                    "FORCED_NONE" => AllyMode::ForcedNone,
                    _ => return false,
                };
                self.ally_mode = v;
                true
            }
            "pvp_mode" => {
                let v = match upper.as_str() {
                    "ALWAYS" => PvpMode::Always,
                    "NEVER" => PvpMode::Never,
                    "PER_TEAM" => PvpMode::PerTeam,
                    _ => return false,
                };
                self.pvp_mode = v;
                true
            }
            "fake_players" => {
                let v = match upper.as_str() {
                    "CHECK" => Policy::Check,
                    "DENY" => Policy::Deny,
                    "ALLOW" => Policy::Allow,
                    _ => return false,
                };
                self.fake_players = v;
                true
            }
            "party_limit_mode" => {
                let v = match upper.as_str() {
                    "LARGEST" => PartyLimitMode::Largest,
                    "OWNER" => PartyLimitMode::Owner,
                    "SUM" => PartyLimitMode::Sum,
                    "AVERAGE" => PartyLimitMode::Average,
                    _ => return false,
                };
                self.party_limit_mode = v;
                true
            }
            "force_load_mode" => {
                let v = match upper.as_str() {
                    "DEFAULT" => ForceLoadMode::Default,
                    "ALWAYS" => ForceLoadMode::Always,
                    "NEVER" => ForceLoadMode::Never,
                    _ => return false,
                };
                self.force_load_mode = v;
                true
            }
            _ => match key.strip_prefix("def_") {
                Some(name) => {
                    let id = match name {
                        "fake_players" => "allow_fake_players",
                        "fake_player_ids" => "allow_fake_players_by_id",
                        "entity_interact" => "entity_interact_mode",
                        "block_interact" => "block_interact_mode",
                        "block_edit" => "block_edit_mode",
                        "entity_attack" => "nonliving_entity_attack_mode",
                        "allow_explosions" => "allow_explosions",
                        "mob_griefing" => "allow_mob_griefing",
                        "claim_visibility" => "claim_visibility",
                        "player_visibility" => "location_mode",
                        "pvp" => "allow_pvp",
                        _ => return false,
                    };
                    self.prop_defaults.set(id, &text_raw.trim_matches('"').to_ascii_lowercase())
                }
                None => false,
            },
        }
    }
    pub fn set_list(&mut self, key: &str, items: Vec<String>) -> bool {
        match key {
            "no_wilderness_dimensions" => self.no_wilderness_dimensions = items,
            "claim_dimension_blacklist" => self.claim_dimension_blacklist = items,
            "claim_dimension_whitelist" => self.claim_dimension_whitelist = items,
            "def_named_fake_players" => self.prop_defaults.allow_named_fake_players = items,
            _ => return false,
        }
        true
    }
}

/// A team as FTB Chunks sees it: the runtime builds it from FTB Teams' `Team`.
#[derive(Clone, Debug, PartialEq)]
pub struct TeamView {
    pub id: Uuid,
    pub kind: Kind,
    pub owner: Uuid,
    /// `ftbteams:display_name`.
    pub name: String,
    /// `ftbteams:color`, `0xRRGGBB`.
    pub color: u32,
    /// Players ranked member or better.
    pub members: Vec<Uuid>,
    /// Players ranked exactly ally (an invite is not one).
    pub allies: Vec<Uuid>,
    pub props: Props,
}

impl TeamView {
    pub fn is_member(&self, player: Uuid) -> bool {
        self.members.contains(&player)
    }
}

/// The teams FTB Chunks asks FTB Teams about.
pub trait Teams {
    fn team(&self, id: Uuid) -> Option<TeamView>;
    /// The player's team (a party, or their personal team).
    fn team_of(&self, player: Uuid) -> Option<TeamView>;
    /// A team argument: a short name, a display name, a UUID or a known player's name.
    fn find(&self, arg: &str) -> Option<TeamView>;
    /// A known player by name: their UUID and name.
    fn player(&self, name: &str) -> Option<(Uuid, String)>;
    fn online(&self, player: Uuid) -> bool;
}

/// `isAlly`: members always; an ally entry exactly, unless `ally_mode` forces it.
pub fn is_ally(config: &Config, team: &TeamView, player: Uuid) -> bool {
    match config.ally_mode {
        AllyMode::ForcedAll => true,
        _ if team.is_member(player) => true,
        AllyMode::ForcedNone => false,
        AllyMode::Default => team.allies.contains(&player),
    }
}

/// `canPlayerUse` for a real player.
pub fn can_use(config: &Config, team: &TeamView, player: Uuid, mode: Privacy) -> bool {
    match mode {
        Privacy::Public => true,
        Privacy::Allies => is_ally(config, team, player),
        Privacy::Private => team.is_member(player),
    }
}

/// `canFakePlayerUse`: a fake player (a machine's) acting with the UUID and name it has.
pub fn can_fake_use(config: &Config, team: &TeamView, player: Uuid, name: &str, mode: Privacy) -> bool {
    if mode == Privacy::Public {
        return true;
    }
    if team.props.allow_fake_players {
        return mode == Privacy::Allies;
    }
    let by_id = team.props.allow_fake_players_by_id;
    let named = || {
        let (name, id) = (name.to_lowercase(), format!("{player:032x}"));
        team.props.allow_named_fake_players.iter().any(|n| *n == name || n.replace('-', "") == id)
    };
    match mode {
        Privacy::Allies => (by_id && is_ally(config, team, player)) || named(),
        Privacy::Private => by_id && team.is_member(player),
        Privacy::Public => true,
    }
}

/// What a member brings to their team's limits (`TeamMemberData`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemberData {
    pub max_claims: i32,
    pub max_force_loads: i32,
    /// `ftbchunks.chunk_load_offline`: the team's chunks stay loaded while all are away.
    pub offline_force_loader: bool,
    /// The chunks this member brought into the party, given back when they leave.
    pub original_claims: BTreeSet<ChunkPos>,
}

/// A team's FTB Chunks data (`ChunkTeamDataImpl`) but its chunks, which are in [`Chunks::claims`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TeamData {
    pub max_claims: i32,
    pub max_force_loads: i32,
    /// `extra_claim_chunks`, `extra_force_load_chunks` (personal teams only).
    pub extra_claims: i32,
    pub extra_force_loads: i32,
    /// Epoch ms of the last member's login.
    pub last_login: i64,
    pub members: BTreeMap<Uuid, MemberData>,
}

/// A claimed chunk (`ClaimedChunkImpl`): its team, and when it was claimed and force-loaded, epoch ms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Claim {
    pub team: Uuid,
    pub time: i64,
    /// Above 0 while force-loaded.
    pub force_loaded: i64,
    /// 0: no expiry.
    pub expiry: i64,
}

/// `ClaimResult`'s problems, by their ids (`ftbchunks.claim_result.<id>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    NotOwner,
    NotEnoughPower,
    AlreadyClaimed,
    DimensionForbidden,
    NotClaimed,
    AlreadyLoaded,
    NotLoaded,
}

impl Problem {
    pub fn id(self) -> &'static str {
        match self {
            Problem::NotOwner => "not_owner",
            Problem::NotEnoughPower => "not_enough_power",
            Problem::AlreadyClaimed => "already_claimed",
            Problem::DimensionForbidden => "dimension_forbidden",
            Problem::NotClaimed => "not_claimed",
            Problem::AlreadyLoaded => "already_loaded",
            Problem::NotLoaded => "not_loaded",
        }
    }
}

/// What a player tries in a chunk (`Protection`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protection {
    EditBlock,
    InteractBlock,
    EditFluid,
    InteractEntity,
    AttackNonLiving,
}

/// Who acts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Actor {
    pub uuid: Uuid,
    pub name: String,
    pub fake: bool,
    pub spectator: bool,
    /// `/ftbchunks admin bypass_protection`.
    pub bypass: bool,
}

/// Why an action was refused: the action-bar line the player gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// `ftbchunks.need_to_claim_chunk` (no-wilderness land).
    NeedClaim,
    /// `ftbchunks.action_prevented`.
    Prevented,
}

/// Every team's claims (`ClaimedChunkManagerImpl`) and FTB Chunks data.
#[derive(Clone, Debug, Default)]
pub struct Chunks {
    pub config: Config,
    pub claims: BTreeMap<ChunkPos, Claim>,
    pub teams: BTreeMap<Uuid, TeamData>,
    /// Teams whose file must be written.
    pub dirty: BTreeSet<Uuid>,
}

impl Chunks {
    pub fn claimed_by(&self, team: Uuid) -> usize {
        self.claims.values().filter(|c| c.team == team).count()
    }
    pub fn force_loaded_by(&self, team: Uuid) -> usize {
        self.claims.values().filter(|c| c.team == team && c.force_loaded > 0).count()
    }
    pub fn claims_of(&self, team: Uuid) -> impl Iterator<Item = (&ChunkPos, &Claim)> {
        self.claims.iter().filter(move |(_, c)| c.team == team)
    }
    fn data(&mut self, team: Uuid) -> &mut TeamData {
        let config = &self.config;
        self.teams.entry(team).or_insert_with(|| TeamData {
            max_claims: config.max_claimed_chunks,
            max_force_loads: config.max_force_loaded_chunks,
            ..TeamData::default()
        })
    }
    /// A team's limits as last computed; the config's for a team with no data yet.
    pub fn limits(&self, team: Uuid) -> (i32, i32) {
        self.teams
            .get(&team)
            .map_or((self.config.max_claimed_chunks, self.config.max_force_loaded_chunks), |d| (d.max_claims, d.max_force_loads))
    }

    /// `ChunkTeamDataImpl.updateLimits`: each member's own limits (the config's, plus their personal team's extra
    /// chunks), then the team's: a personal team has its player's, a party's follow `party_limit_mode`, then the hard
    /// caps. True when they changed.
    pub fn update_limits(&mut self, team: &TeamView) -> bool {
        let config = self.config.clone();
        let people: Vec<Uuid> = if team.kind == Kind::Party { team.members.clone() } else { vec![team.id] };
        let extras: Vec<(i32, i32)> =
            people.iter().map(|p| self.teams.get(p).map_or((0, 0), |d| (d.extra_claims, d.extra_force_loads))).collect();
        let data = self.data(team.id);
        let mut all = Vec::new();
        for (p, (extra_c, extra_f)) in people.iter().zip(extras) {
            let m = data.members.entry(*p).or_default();
            m.max_claims = config.max_claimed_chunks + extra_c;
            m.max_force_loads = config.max_force_loaded_chunks + extra_f;
            all.push((m.max_claims, m.max_force_loads));
        }
        let owner = data.members.get(&team.owner).map(|m| (m.max_claims, m.max_force_loads));
        let n = all.len().max(1) as i32;
        let sum = (all.iter().map(|a| a.0).sum::<i32>(), all.iter().map(|a| a.1).sum::<i32>());
        let max = (all.iter().map(|a| a.0).max().unwrap_or(0), all.iter().map(|a| a.1).max().unwrap_or(0));
        let mut limits = match (team.kind, config.party_limit_mode) {
            (Kind::Party, PartyLimitMode::Owner) => owner.unwrap_or(max),
            (Kind::Party, PartyLimitMode::Sum) => sum,
            (Kind::Party, PartyLimitMode::Average) => (sum.0 / n, sum.1 / n),
            _ => max,
        };
        if config.hard_team_claim_limit > 0 {
            limits.0 = limits.0.min(config.hard_team_claim_limit);
        }
        if config.hard_team_force_limit > 0 {
            limits.1 = limits.1.min(config.hard_team_force_limit);
        }
        let changed = (data.max_claims, data.max_force_loads) != limits;
        (data.max_claims, data.max_force_loads) = limits;
        if changed {
            self.dirty.insert(team.id);
        }
        changed
    }

    /// `ChunkTeamDataImpl.claim`: a server team has no limit.
    pub fn claim(&mut self, team: &TeamView, pos: ChunkPos, now: i64) -> Result<(), Problem> {
        if self.claims.contains_key(&pos) {
            return Err(Problem::AlreadyClaimed);
        }
        if !self.config.may_claim_in(&pos.0) {
            return Err(Problem::DimensionForbidden);
        }
        if team.kind != Kind::Server && self.claimed_by(team.id) >= self.limits(team.id).0.max(0) as usize {
            return Err(Problem::NotEnoughPower);
        }
        self.claims.insert(pos, Claim { team: team.id, time: now, force_loaded: 0, expiry: 0 });
        self.dirty.insert(team.id);
        Ok(())
    }
    /// `unclaim`: another team's chunk only with `admin`. Un-force-loads it first.
    pub fn unclaim(&mut self, team: Uuid, pos: &ChunkPos, admin: bool) -> Result<Claim, Problem> {
        match self.claims.get(pos) {
            None => Err(Problem::NotClaimed),
            Some(c) if c.team != team && !admin => Err(Problem::NotOwner),
            Some(_) => {
                let claim = self.claims.remove(pos).unwrap();
                self.dirty.insert(claim.team);
                Ok(claim)
            }
        }
    }
    /// `forceLoad`: the owning team's limit applies, a server team's none.
    pub fn load(&mut self, team: &TeamView, pos: &ChunkPos, admin: bool, now: i64) -> Result<(), Problem> {
        let Some(claim) = self.claims.get(pos).copied() else { return Err(Problem::NotClaimed) };
        if claim.team != team.id && !admin {
            return Err(Problem::NotOwner);
        }
        if claim.force_loaded > 0 {
            return Err(Problem::AlreadyLoaded);
        }
        let owner_is_server = claim.team == team.id && team.kind == Kind::Server;
        if !owner_is_server && self.force_loaded_by(claim.team) >= self.limits(claim.team).1.max(0) as usize {
            return Err(Problem::NotEnoughPower);
        }
        let c = self.claims.get_mut(pos).unwrap();
        c.force_loaded = now.max(1);
        self.dirty.insert(claim.team);
        Ok(())
    }
    pub fn unload(&mut self, team: Uuid, pos: &ChunkPos, admin: bool) -> Result<(), Problem> {
        match self.claims.get_mut(pos) {
            None => Err(Problem::NotClaimed),
            Some(c) if c.team != team && !admin => Err(Problem::NotOwner),
            Some(c) if c.force_loaded == 0 => Err(Problem::NotLoaded),
            Some(c) => {
                c.force_loaded = 0;
                c.expiry = 0;
                let t = c.team;
                self.dirty.insert(t);
                Ok(())
            }
        }
    }

    /// `transferClaims`, when `player` joins `party`: their personal team's claims become the party's, those past the
    /// party's limit are let go; the party remembers which were theirs.
    pub fn joined_party(&mut self, party: &TeamView, player: Uuid) -> usize {
        self.update_limits(party);
        let mine: Vec<ChunkPos> = self.claims_of(player).map(|(p, _)| p.clone()).collect();
        let room = (self.limits(party.id).0.max(0) as usize).saturating_sub(self.claimed_by(party.id));
        let mut dropped = 0;
        for (i, pos) in mine.iter().enumerate() {
            if i < room || party.kind == Kind::Server {
                self.claims.get_mut(pos).unwrap().team = party.id;
                self.data(party.id).members.entry(player).or_default().original_claims.insert(pos.clone());
            } else {
                self.claims.remove(pos);
                dropped += 1;
            }
        }
        self.dirty.insert(player);
        self.dirty.insert(party.id);
        dropped
    }
    /// A member left `party` (or it was disbanded): the chunks they brought go back to their personal team as far as
    /// its limit allows; a disbanded party's others are let go.
    pub fn left_party(&mut self, party: Uuid, player: &TeamView, deleted: bool) {
        self.update_limits(player);
        let originals = self.teams.get_mut(&party).and_then(|d| d.members.remove(&player.id)).map(|m| m.original_claims);
        for pos in originals.unwrap_or_default() {
            if self.claims.get(&pos).is_some_and(|c| c.team == party) {
                let room = self.claimed_by(player.id) < self.limits(player.id).0.max(0) as usize;
                if room {
                    self.claims.get_mut(&pos).unwrap().team = player.id;
                } else {
                    self.claims.remove(&pos);
                }
            }
        }
        if deleted {
            self.claims.retain(|_, c| c.team != party);
            self.teams.remove(&party);
        }
        self.dirty.insert(player.id);
        self.dirty.insert(party);
    }

    /// `canDoOfflineForceLoading`.
    pub fn offline_force_loading(&self, team: Uuid) -> bool {
        match self.config.force_load_mode {
            ForceLoadMode::Always => true,
            ForceLoadMode::Never => false,
            ForceLoadMode::Default => self.teams.get(&team).is_some_and(|d| d.members.values().any(|m| m.offline_force_loader)),
        }
    }
    /// The force-loaded chunks whose tickets must be held now: a team's while one of its members is on, or always
    /// when it may load offline (FTB Chunks adds and drops the tickets as the first member logs in and the last out).
    /// A server team's always, as it has no members to be on.
    pub fn wanted_tickets(&self, teams: &dyn Teams) -> BTreeSet<(Uuid, ChunkPos)> {
        let mut on: BTreeMap<Uuid, bool> = BTreeMap::new();
        self.claims
            .iter()
            .filter(|(_, c)| c.force_loaded > 0)
            .filter(|(_, c)| {
                *on.entry(c.team).or_insert_with(|| match teams.team(c.team) {
                    None => false,
                    Some(t) if t.kind == Kind::Server => true,
                    Some(t) => self.offline_force_loading(t.id) || t.members.iter().chain([&t.id]).any(|m| teams.online(*m)),
                })
            })
            .map(|(p, c)| (c.team, p.clone()))
            .collect()
    }

    /// The team whose claim holds block `(x, z)` of `dimension`, with its view.
    pub fn owner_at(&self, teams: &dyn Teams, pos: &ChunkPos) -> Option<TeamView> {
        self.claims.get(pos).and_then(|c| teams.team(c.team))
    }

    /// `noWilderness` for this dimension: the list decides where it names the dimension, the flag elsewhere.
    fn no_wilderness(&self, dimension: &str) -> bool {
        listed(&self.config.no_wilderness_dimensions, dimension) || self.config.no_wilderness
    }

    /// `shouldPreventInteraction`: None when `actor` may do `what` at chunk `pos`; `whitelisted` is the target's tag
    /// (`ftbchunks:edit_whitelist`, `interact_whitelist`, `entity_interact_whitelist`...).
    pub fn refuses(&self, teams: &dyn Teams, actor: &Actor, pos: &ChunkPos, what: Protection, whitelisted: bool) -> Option<Refusal> {
        if self.config.disable_protection || actor.bypass {
            return None;
        }
        if actor.fake && self.config.fake_players != Policy::Check {
            return (self.config.fake_players == Policy::Deny).then_some(Refusal::Prevented);
        }
        let owner = self.owner_at(teams, pos);
        let Some(team) = owner else {
            return self.no_wilderness(&pos.0).then_some(Refusal::NeedClaim).filter(|_| !whitelisted && !actor.spectator);
        };
        if whitelisted {
            return None;
        }
        let mode = match what {
            Protection::EditBlock | Protection::EditFluid => team.props.block_edit_mode,
            Protection::InteractBlock => team.props.block_interact_mode,
            Protection::InteractEntity => team.props.entity_interact_mode,
            Protection::AttackNonLiving => team.props.nonliving_entity_attack_mode,
        };
        let allowed = if actor.fake {
            can_fake_use(&self.config, &team, actor.uuid, &actor.name, mode)
        } else {
            can_use(&self.config, &team, actor.uuid, mode)
        };
        (!allowed && (actor.fake || !actor.spectator)).then_some(Refusal::Prevented)
    }

    /// `RIGHT_CLICK_ITEM` at the actor's eyes: food, a potion without harm and `right_click_whitelist` always; in a
    /// claim the actor may not use, `right_click_blacklist` items are refused; nothing else is.
    pub fn refuses_item(&self, teams: &dyn Teams, actor: &Actor, pos: &ChunkPos, whitelisted: bool, blacklisted: bool) -> bool {
        if self.config.disable_protection || actor.bypass || whitelisted || !blacklisted {
            return false;
        }
        if actor.fake && self.config.fake_players != Policy::Check {
            return self.config.fake_players == Policy::Deny;
        }
        let Some(team) = self.owner_at(teams, pos) else { return false };
        !can_use(&self.config, &team, actor.uuid, team.props.block_interact_mode)
    }

    /// `ExplosionEvent.Detonate`: the blocks in chunks of teams that don't allow explosions are taken out.
    /// `known_source` false (a ghast's fireball) is protected as `protect_unknown_explosions` says.
    pub fn filter_explosion(&self, teams: &dyn Teams, dimension: &str, known_source: bool, blocks: &mut Vec<[i32; 3]>) {
        if self.config.disable_protection || blocks.is_empty() || (!known_source && !self.config.protect_unknown_explosions) {
            return;
        }
        let mut protected: BTreeMap<(i32, i32), bool> = BTreeMap::new();
        blocks.retain(|b| {
            let (cx, cz) = (b[0] >> 4, b[2] >> 4);
            !*protected.entry((cx, cz)).or_insert_with(|| {
                self.owner_at(teams, &(dimension.to_owned(), cx, cz)).is_some_and(|t| !t.props.allow_explosions)
            })
        });
    }

    /// `PistonHelper.shouldPreventPistonMovement`: a block may not cross from one team's land (or the wilderness)
    /// into another's that doesn't let everyone edit.
    pub fn refuses_crossing(&self, teams: &dyn Teams, dimension: &str, from: [i32; 3], to: [i32; 3]) -> bool {
        let (a, b) = (chunk_of(dimension, from[0], from[2]), chunk_of(dimension, to[0], to[2]));
        if a == b {
            return false;
        }
        let (ta, tb) = (self.claims.get(&a).map(|c| c.team), self.claims.get(&b).map(|c| c.team));
        ta != tb && self.owner_at(teams, &b).is_some_and(|t| t.props.block_edit_mode != Privacy::Public)
    }

    /// `EntityMobGriefingEvent` for the griefing blacklist (endermen): not in a claim that forbids it.
    pub fn refuses_griefing(&self, teams: &dyn Teams, pos: &ChunkPos) -> bool {
        !self.config.disable_protection && self.owner_at(teams, pos).is_some_and(|t| !t.props.allow_mob_griefing)
    }
}
