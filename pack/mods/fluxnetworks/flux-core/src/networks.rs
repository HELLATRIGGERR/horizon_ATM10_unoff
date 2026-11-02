//! `FluxNetworkData` and `FluxNetwork`'s saved part: the networks of a world,
//! who may use them and what makes a new one. Writing them to the world's
//! `data/fluxnetworksdata.dat` is the game's side (`flux_world.rs`), in the
//! mod's own layout.
use std::collections::BTreeMap;

/// `FluxConstants.INVALID_NETWORK_ID`: a device on no network.
pub const NO_NETWORK: i32 = -1;
/// `FluxConstants.INVALID_NETWORK_COLOR`.
pub const NO_NETWORK_COLOR: u32 = 0xb2_b2_b2;
/// `FluxNetwork.MAX_NETWORK_NAME_LENGTH`.
pub const MAX_NAME: usize = 24;
/// `EnumNetworkColor`, in its order.
pub const PRESET_COLORS: [u32; 14] = [
    0x295E8A, 0x343477, 0x582A72, 0x882D60, 0xAA3939, 0xAA6F39, 0xC6B900, 0x609732, 0x87CEFA, 0x86608E,
    0xF08080, 0xFFC0CB, 0xFFDAB9, 0xEEDC82,
];

/// `SecurityLevel`, saved by its ordinal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Security {
    Public,
    Encrypted,
    Private,
}

impl Security {
    pub fn from_id(id: i64) -> Self {
        match id {
            0 => Self::Public,
            1 => Self::Encrypted,
            _ => Self::Private,
        }
    }

    pub fn id(self) -> i8 {
        self as i8
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Public => "Public",
            Self::Encrypted => "Encrypted",
            Self::Private => "Private",
        }
    }
}

/// `AccessLevel`, saved by its ordinal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Owner,
    Admin,
    User,
    Blocked,
    SuperAdmin,
}

impl Access {
    pub fn from_id(id: i64) -> Self {
        match id {
            0 => Self::Owner,
            1 => Self::Admin,
            2 => Self::User,
            4 => Self::SuperAdmin,
            _ => Self::Blocked,
        }
    }

    pub fn id(self) -> i8 {
        self as i8
    }

    /// `canUse`.
    pub fn can_use(self) -> bool {
        self != Self::Blocked
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Owner => "Owner",
            Self::Admin => "Admin",
            Self::User => "User",
            Self::Blocked => "Stranger",
            Self::SuperAdmin => "Super Admin",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub uuid: u128,
    pub name: String,
    pub access: Access,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Network {
    pub id: i32,
    pub name: String,
    pub color: u32,
    pub owner: u128,
    pub security: Security,
    pub password: String,
    pub members: Vec<Member>,
}

impl Network {
    /// `getPlayerAccess`, without super admins.
    pub fn access(&self, player: u128) -> Access {
        if self.owner == player {
            return Access::Owner;
        }
        if let Some(member) = self.members.iter().find(|m| m.uuid == player) {
            return member.access;
        }
        if self.security == Security::Public { Access::User } else { Access::Blocked }
    }
}

/// Why a network wasn't made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateError {
    /// `RESPONSE_NO_SPACE`: the player owns `maximumPerPlayer` already.
    NoSpace,
}

/// `FluxNetworkData`.
#[derive(Clone, Debug, PartialEq)]
pub struct Networks {
    pub by_id: BTreeMap<i32, Network>,
    /// `mUniqueID`, the last id made.
    pub unique_id: i32,
}

impl Networks {
    /// A world's first: ids count from a random `origin`, as the mod does
    /// (`SecureRandom.nextInt(50000) * 10000`).
    pub fn new(origin: i32) -> Self {
        Self { by_id: BTreeMap::new(), unique_id: origin }
    }

    pub fn get(&self, id: i32) -> Option<&Network> {
        self.by_id.get(&id)
    }

    /// `createNetwork`, without super admins: the owner is its only member.
    pub fn create(
        &mut self,
        owner: u128,
        owner_name: &str,
        name: String,
        color: u32,
        security: Security,
        password: String,
        maximum_per_player: i32,
    ) -> Result<i32, CreateError> {
        if maximum_per_player != -1 {
            let owned = self.by_id.values().filter(|n| n.owner == owner).count();
            if maximum_per_player <= 0 || owned >= maximum_per_player as usize {
                return Err(CreateError::NoSpace);
            }
        }
        loop {
            self.unique_id = self.unique_id.wrapping_add(1);
            if self.unique_id < 0 {
                self.unique_id = 1;
            }
            if !self.by_id.contains_key(&self.unique_id) {
                break;
            }
        }
        let id = self.unique_id;
        let members = vec![Member { uuid: owner, name: owner_name.to_owned(), access: Access::Owner }];
        self.by_id.insert(id, Network { id, name, color, owner, security, password, members });
        Ok(id)
    }

    /// The chest menu's "create": the mod's default name with a number, so
    /// they stay apart without a text field, the next preset colour, and
    /// private, as the mod's create tab starts.
    pub fn create_default(&mut self, owner: u128, owner_name: &str, maximum_per_player: i32) -> Result<i32, CreateError> {
        let owned = self.by_id.values().filter(|n| n.owner == owner).count();
        let color = PRESET_COLORS[owned % PRESET_COLORS.len()];
        let name = default_name(owner_name, owned + 1);
        self.create(owner, owner_name, name, color, Security::Private, String::new(), maximum_per_player)
    }

    /// The networks a player can connect a device to without a password.
    pub fn usable_by(&self, player: u128) -> Vec<&Network> {
        self.by_id.values().filter(|n| n.access(player).can_use()).collect()
    }
}

/// `<player>'s Network #n`, cut to the mod's limit on names.
pub fn default_name(player: &str, n: usize) -> String {
    let suffix = format!("'s Network #{n}");
    let room = MAX_NAME.saturating_sub(suffix.chars().count());
    let player: String = player.chars().take(room).collect();
    format!("{player}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALEX: u128 = 1;
    const STEVE: u128 = 2;

    #[test]
    fn ids_count_up_from_the_origin() {
        let mut n = Networks::new(70000);
        assert_eq!(n.create_default(ALEX, "Alex", 5), Ok(70001));
        assert_eq!(n.create_default(ALEX, "Alex", 5), Ok(70002));
        let wrap = &mut Networks::new(i32::MAX);
        assert_eq!(wrap.create_default(ALEX, "Alex", 5), Ok(1), "a wrapped counter starts again at 1");
    }

    #[test]
    fn a_player_owns_at_most_the_maximum() {
        let mut n = Networks::new(0);
        for _ in 0..5 {
            n.create_default(ALEX, "Alex", 5).unwrap();
        }
        assert_eq!(n.create_default(ALEX, "Alex", 5), Err(CreateError::NoSpace));
        assert!(n.create_default(STEVE, "Steve", 5).is_ok(), "each player has their own count");
        assert_eq!(n.create_default(STEVE, "Steve", 0), Err(CreateError::NoSpace), "0 only allows super admins");
        for _ in 0..10 {
            n.create_default(ALEX, "Alex", -1).unwrap();
        }
    }

    #[test]
    fn a_new_network_takes_the_next_colour_and_a_name_that_fits() {
        let mut n = Networks::new(0);
        let a = n.create_default(ALEX, "Alex", 5).unwrap();
        let b = n.create_default(ALEX, "Alex", 5).unwrap();
        assert_eq!(n.get(a).unwrap().color, PRESET_COLORS[0]);
        assert_eq!(n.get(b).unwrap().color, PRESET_COLORS[1]);
        assert_eq!(n.get(a).unwrap().name, "Alex's Network #1");
        assert_eq!(n.get(b).unwrap().name, "Alex's Network #2");
        assert_eq!(n.get(a).unwrap().security, Security::Private);
        let long = default_name("SixteenCharsName", 12);
        assert_eq!(long.chars().count(), MAX_NAME);
        assert!(long.ends_with("'s Network #12"));
    }

    #[test]
    fn access_follows_owner_members_and_security() {
        let mut n = Networks::new(0);
        let id = n.create_default(ALEX, "Alex", 5).unwrap();
        let net = n.by_id.get_mut(&id).unwrap();
        assert_eq!(net.access(ALEX), Access::Owner);
        assert_eq!(net.access(STEVE), Access::Blocked, "private");
        net.security = Security::Encrypted;
        assert_eq!(net.access(STEVE), Access::Blocked, "encrypted needs a password");
        net.security = Security::Public;
        assert_eq!(net.access(STEVE), Access::User);
        net.security = Security::Private;
        net.members.push(Member { uuid: STEVE, name: "Steve".into(), access: Access::Admin });
        assert_eq!(net.access(STEVE), Access::Admin);
        assert!(net.access(STEVE).can_use());
        assert_eq!(n.usable_by(STEVE).len(), 1);
        assert!(n.usable_by(3).is_empty());
    }

    #[test]
    fn ordinals_are_the_mods() {
        assert_eq!(Security::Private.id(), 2);
        assert_eq!(Security::from_id(0), Security::Public);
        assert_eq!(Access::SuperAdmin.id(), 4);
        assert_eq!(Access::from_id(0), Access::Owner);
        assert_eq!(Access::from_id(3), Access::Blocked);
    }
}
