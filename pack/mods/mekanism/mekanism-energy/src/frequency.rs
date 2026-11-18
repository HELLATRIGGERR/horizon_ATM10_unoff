//! Mekanism's frequencies (`mekanism.common.lib.frequency`: `Frequency`, `FrequencyManager`,
//! `FrequencyManagerWrapper`; `mekanism.common.lib.security`: `SecurityFrequency`, `SecurityUtils`), shared by every
//! frequency type (`FrequencyType`: QIO here; the Teleporter's and the Quantum Entangloporter's take the same list
//! with their own `kind`).
//!
//! A frequency is named, owned and public, private or trusted (`SecurityMode`). Public names are one list for
//! everyone (`publicManager`, keyed by the name); private and trusted names are each owner's own
//! (`privateManagers`, `trustedManagers`). A private frequency is its owner's alone; a trusted one is its owner's and
//! the players the owner trusts (`SecurityFrequency.trusted`, the Security Desk's list; Mekanism uses no teams).
//! Only the owner deletes a frequency (`FrequencyManager.remove`) or changes its colour (`PacketSetFrequencyColor`).
//! The list is saved with the world, so a frequency outlives the blocks that use it.
use serde_json::{Value, json};

/// `MAX_FREQ_LENGTH`.
pub const NAME_MAX: usize = 16;

/// `SecurityMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Security {
    Public,
    Private,
    Trusted,
}

impl Security {
    pub const ALL: [Security; 3] = [Security::Public, Security::Private, Security::Trusted];

    pub fn name(self) -> &'static str {
        match self {
            Security::Public => "public",
            Security::Private => "private",
            Security::Trusted => "trusted",
        }
    }

    /// `MekanismLang.PUBLIC`, `PRIVATE`, `TRUSTED`.
    pub fn label(self) -> &'static str {
        match self {
            Security::Public => "Public",
            Security::Private => "Private",
            Security::Trusted => "Trusted",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == s)
    }

    pub fn index(self) -> u8 {
        self as u8
    }

    pub fn from_index(i: u8) -> Option<Self> {
        Self::ALL.get(usize::from(i)).copied()
    }
}

/// `EnumColor`: its names (`color.mekanism.*`) and RGB, in its order (`getNext`, `getPrevious` step through it).
pub const COLORS: [(&str, u32); 18] = [
    ("Black", 0x404040),
    ("Blue", 0x366BD0),
    ("Green", 0x59C15F),
    ("Cyan", 0x00F3D0),
    ("Dark Red", 0xC9071F),
    ("Purple", 0xA460D9),
    ("Orange", 0xFFA160),
    ("Light Gray", 0xCFCFCF),
    ("Gray", 0x7A7A7A),
    ("Light Blue", 0x559EFF),
    ("Lime", 0x75FF89),
    ("Aqua", 0x30FFF9),
    ("Red", 0xFF383C),
    ("Magenta", 0xD55ECB),
    ("Yellow", 0xFFDD4F),
    ("White", 0xFFFFFF),
    ("Brown", 0xA17649),
    ("Pink", 0xFFBCC4),
];
/// A new QIO frequency's colour (`QIOFrequency`: `EnumColor.INDIGO`).
pub const DEFAULT_COLOR: u8 = 9;

pub fn color_step(color: u8, next: bool) -> u8 {
    let n = COLORS.len() as u8;
    if next { (color + 1) % n } else { (color + n - 1) % n }
}

/// `Frequency.FrequencyIdentity`: the name, the mode and the owner. Public names are everyone's, so two public
/// identities with the same name are the same frequency whoever made them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub name: String,
    pub security: Security,
    pub owner: u128,
}

impl Identity {
    pub fn same(&self, other: &Identity) -> bool {
        self.name == other.name && self.security == other.security && (self.security == Security::Public || self.owner == other.owner)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frequency {
    /// `FrequencyType`'s name: `qio`, `teleporter`, `inventory`.
    pub kind: String,
    pub name: String,
    pub owner: u128,
    pub owner_name: String,
    pub security: Security,
    /// An index into `COLORS` (`IColorableFrequency`; kept for every kind, used by QIO's).
    pub color: u8,
}

impl Frequency {
    pub fn identity(&self) -> Identity {
        Identity { name: self.name.clone(), security: self.security, owner: self.owner }
    }
}

/// A player's trusted list (`SecurityFrequency`): their id and name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Trust {
    pub owner: u128,
    pub players: Vec<(u128, String)>,
}

/// Every frequency of every kind and every owner's trusted players, as the world saves them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frequencies {
    pub list: Vec<Frequency>,
    pub trusted: Vec<Trust>,
    /// Bumped on every change: the screens list them again, the world writes them again.
    pub revision: u64,
}

/// A typed name as Mekanism takes it: trimmed, at most `NAME_MAX` characters; None when empty.
pub fn clean_name(name: &str) -> Option<String> {
    let name: String = name.trim().chars().take(NAME_MAX).collect();
    (!name.trim().is_empty()).then(|| name.trim_end().to_owned())
}

impl Frequencies {
    /// `SecurityUtils.isTrusted`: the owner, or a player on the owner's list.
    pub fn is_trusted(&self, owner: u128, player: u128) -> bool {
        owner == player || self.trusted.iter().any(|t| t.owner == owner && t.players.iter().any(|(p, _)| *p == player))
    }

    /// Whether `player` may see and use `f` (`SecurityUtils.canAccessObject` for a frequency).
    pub fn can_access(&self, f: &Frequency, player: u128) -> bool {
        match f.security {
            Security::Public => true,
            Security::Private => f.owner == player,
            Security::Trusted => self.is_trusted(f.owner, player),
        }
    }

    pub fn find(&self, kind: &str, id: &Identity) -> Option<&Frequency> {
        self.list.iter().find(|f| f.kind == kind && f.identity().same(id))
    }

    fn position(&self, kind: &str, id: &Identity) -> Option<usize> {
        self.list.iter().position(|f| f.kind == kind && f.identity().same(id))
    }

    /// The list `GuiFrequencySelector` shows in a mode: every public one; the player's own private ones; the
    /// trusted ones of the owners who trust the player, theirs included. In the order they were made.
    pub fn visible(&self, kind: &str, security: Security, player: u128) -> Vec<&Frequency> {
        self.list.iter().filter(|f| f.kind == kind && f.security == security && self.can_access(f, player)).collect()
    }

    /// `FrequencyManager.getOrCreateFrequency` for a name the player typed in a mode: the public one of that name
    /// whoever made it, else the player's own; a new one (owned by the player) when there is none.
    pub fn get_or_create(&mut self, kind: &str, name: &str, security: Security, player: u128, player_name: &str) -> Option<Identity> {
        let name = clean_name(name)?;
        let id = Identity { name, security, owner: player };
        if let Some(f) = self.find(kind, &id) {
            return Some(f.identity());
        }
        self.list.push(Frequency {
            kind: kind.to_owned(),
            name: id.name.clone(),
            owner: player,
            owner_name: player_name.to_owned(),
            security,
            color: DEFAULT_COLOR,
        });
        self.revision += 1;
        Some(id)
    }

    /// `FrequencyManager.validateAndUpdate` for a holder whose frequency the list lacks: it goes back on it.
    pub fn restore(&mut self, kind: &str, id: &Identity, owner_name: &str) {
        if self.find(kind, id).is_none() {
            self.list.push(Frequency {
                kind: kind.to_owned(),
                name: id.name.clone(),
                owner: id.owner,
                owner_name: owner_name.to_owned(),
                security: id.security,
                color: DEFAULT_COLOR,
            });
            self.revision += 1;
        }
    }

    /// A frequency picked from the list: the stored one when the player may use it.
    pub fn select(&self, kind: &str, id: &Identity, player: u128) -> Option<Identity> {
        self.find(kind, id).filter(|f| self.can_access(f, player)).map(Frequency::identity)
    }

    /// `FrequencyManager.remove`: the owner's alone. True when it went.
    pub fn remove(&mut self, kind: &str, id: &Identity, by: u128) -> bool {
        match self.position(kind, id) {
            Some(i) if self.list[i].owner == by => {
                self.list.remove(i);
                self.revision += 1;
                true
            }
            _ => false,
        }
    }

    /// `PacketSetFrequencyColor`: the owner steps the colour on or back. The new colour, None for another player.
    pub fn step_color(&mut self, kind: &str, id: &Identity, by: u128, next: bool) -> Option<u8> {
        let i = self.position(kind, id).filter(|&i| self.list[i].owner == by)?;
        let f = &mut self.list[i];
        f.color = color_step(f.color, next);
        self.revision += 1;
        Some(f.color)
    }

    /// `SecurityFrequency.addTrusted`: false when already there or the owner themself.
    pub fn trust(&mut self, owner: u128, player: u128, name: &str) -> bool {
        if owner == player || self.is_trusted(owner, player) {
            return false;
        }
        match self.trusted.iter_mut().find(|t| t.owner == owner) {
            Some(t) => t.players.push((player, name.to_owned())),
            None => self.trusted.push(Trust { owner, players: vec![(player, name.to_owned())] }),
        }
        self.revision += 1;
        true
    }

    /// `SecurityFrequency.removeTrusted`.
    pub fn untrust(&mut self, owner: u128, player: u128) -> bool {
        let Some(t) = self.trusted.iter_mut().find(|t| t.owner == owner) else { return false };
        let before = t.players.len();
        t.players.retain(|(p, _)| *p != player);
        let gone = t.players.len() != before;
        if gone {
            self.revision += 1;
        }
        gone
    }

    pub fn to_json(&self) -> Value {
        let list: Vec<Value> = self
            .list
            .iter()
            .map(|f| {
                json!({ "kind": f.kind, "name": f.name, "owner": format!("{:032x}", f.owner), "owner_name": f.owner_name,
                    "security": f.security.name(), "color": f.color })
            })
            .collect();
        let trusted: Vec<Value> = self
            .trusted
            .iter()
            .map(|t| {
                let players: Vec<Value> = t.players.iter().map(|(p, n)| json!({ "id": format!("{p:032x}"), "name": n })).collect();
                json!({ "owner": format!("{:032x}", t.owner), "players": players })
            })
            .collect();
        json!({ "frequencies": list, "trusted": trusted })
    }

    pub fn from_json(v: &Value) -> Self {
        let id = |v: &Value| v.as_str().and_then(|s| u128::from_str_radix(s, 16).ok());
        let mut out = Self::default();
        for f in v["frequencies"].as_array().into_iter().flatten() {
            let (Some(kind), Some(name), Some(owner), Some(security)) =
                (f["kind"].as_str(), f["name"].as_str(), id(&f["owner"]), f["security"].as_str().and_then(Security::parse))
            else {
                continue;
            };
            let color = f["color"].as_u64().filter(|&c| (c as usize) < COLORS.len()).unwrap_or(u64::from(DEFAULT_COLOR)) as u8;
            let owner_name = f["owner_name"].as_str().unwrap_or("").to_owned();
            out.list.push(Frequency { kind: kind.to_owned(), name: name.to_owned(), owner, owner_name, security, color });
        }
        for t in v["trusted"].as_array().into_iter().flatten() {
            let Some(owner) = id(&t["owner"]) else { continue };
            let players = t["players"].as_array().into_iter().flatten().filter_map(|p| Some((id(&p["id"])?, p["name"].as_str().unwrap_or("").to_owned()))).collect();
            out.trusted.push(Trust { owner, players });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE: u128 = 1;
    const BOB: u128 = 2;
    const CAROL: u128 = 3;

    fn names(v: Vec<&Frequency>) -> Vec<&str> {
        v.into_iter().map(|f| f.name.as_str()).collect()
    }

    #[test]
    fn private_frequencies_are_the_owners_alone() {
        let mut f = Frequencies::default();
        f.get_or_create("qio", "Shared", Security::Public, ALICE, "Alice").unwrap();
        let secret = f.get_or_create("qio", "Secret", Security::Private, ALICE, "Alice").unwrap();
        assert_eq!(names(f.visible("qio", Security::Private, ALICE)), ["Secret"]);
        assert!(f.visible("qio", Security::Private, BOB).is_empty());
        assert_eq!(names(f.visible("qio", Security::Public, BOB)), ["Shared"]);
        assert_eq!(f.select("qio", &secret, BOB), None);
        // Bob's own "Secret" is another frequency; a public name is everyone's.
        let bobs = f.get_or_create("qio", "Secret", Security::Private, BOB, "Bob").unwrap();
        assert_ne!(bobs, secret);
        let shared = f.get_or_create("qio", " Shared ", Security::Public, BOB, "Bob").unwrap();
        assert_eq!(shared.owner, ALICE);
        assert_eq!(f.list.len(), 3);
        assert!(f.visible("teleporter", Security::Public, ALICE).is_empty());
    }

    #[test]
    fn trusted_frequencies_follow_the_owners_list() {
        let mut f = Frequencies::default();
        let team = f.get_or_create("qio", "Team", Security::Trusted, ALICE, "Alice").unwrap();
        assert!(f.visible("qio", Security::Trusted, BOB).is_empty());
        assert!(f.trust(ALICE, BOB, "Bob"));
        assert!(!f.trust(ALICE, BOB, "Bob"));
        assert_eq!(names(f.visible("qio", Security::Trusted, BOB)), ["Team"]);
        assert!(f.visible("qio", Security::Trusted, CAROL).is_empty());
        assert_eq!(f.select("qio", &team, BOB), Some(team.clone()));
        // Trust doesn't let Bob delete it or change its colour.
        assert!(!f.remove("qio", &team, BOB));
        assert_eq!(f.step_color("qio", &team, BOB, true), None);
        assert!(f.untrust(ALICE, BOB));
        assert!(f.visible("qio", Security::Trusted, BOB).is_empty());
    }

    #[test]
    fn colours_step_through_enum_color_and_the_owner_deletes() {
        let mut f = Frequencies::default();
        let id = f.get_or_create("qio", "A", Security::Public, ALICE, "Alice").unwrap();
        assert_eq!(f.find("qio", &id).unwrap().color, DEFAULT_COLOR);
        assert_eq!(f.step_color("qio", &id, ALICE, true), Some(10));
        assert_eq!(f.step_color("qio", &id, ALICE, false), Some(9));
        assert_eq!(color_step(0, false), 17);
        assert_eq!(color_step(17, true), 0);
        assert!(!f.remove("qio", &id, BOB));
        assert!(f.remove("qio", &id, ALICE));
        assert!(f.list.is_empty());
        assert_eq!(clean_name("   "), None);
        assert_eq!(clean_name("abcdefghijklmnopqrstuvwxyz").unwrap().len(), NAME_MAX);
    }

    #[test]
    fn the_saved_list_reads_back() {
        let mut f = Frequencies::default();
        let id = f.get_or_create("qio", "Secret", Security::Private, ALICE, "Alice").unwrap();
        f.step_color("qio", &id, ALICE, true);
        f.get_or_create("qio", "Team", Security::Trusted, ALICE, "Alice");
        f.trust(ALICE, BOB, "Bob");
        let back = Frequencies::from_json(&f.to_json());
        assert_eq!(back.list, f.list);
        assert_eq!(back.trusted, f.trusted);
    }
}
