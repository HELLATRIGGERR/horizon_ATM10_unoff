//! Powah's seven tiers, cheapest first.

/// A tier; its index orders every table of the config.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tier {
    Starter,
    Basic,
    Hardened,
    Blazing,
    Niotic,
    Spirited,
    Nitro,
}

impl Tier {
    pub const ALL: [Tier; 7] = [
        Tier::Starter,
        Tier::Basic,
        Tier::Hardened,
        Tier::Blazing,
        Tier::Niotic,
        Tier::Spirited,
        Tier::Nitro,
    ];

    /// The name in ids and in the config (`furnator_starter`, `"starter"`).
    pub fn name(self) -> &'static str {
        match self {
            Tier::Starter => "starter",
            Tier::Basic => "basic",
            Tier::Hardened => "hardened",
            Tier::Blazing => "blazing",
            Tier::Niotic => "niotic",
            Tier::Spirited => "spirited",
            Tier::Nitro => "nitro",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_name(name: &str) -> Option<Tier> {
        Tier::ALL.into_iter().find(|t| t.name() == name)
    }

    /// The tier of a Furnator's block or block entity id (`powah:furnator_nitro`).
    pub fn of_furnator(id: &str) -> Option<Tier> {
        Tier::from_name(id.strip_prefix("powah:furnator_")?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_name_their_tier() {
        assert_eq!(Tier::of_furnator("powah:furnator_starter"), Some(Tier::Starter));
        assert_eq!(Tier::of_furnator("powah:furnator_nitro"), Some(Tier::Nitro));
        assert_eq!(Tier::of_furnator("powah:furnator_creative"), None);
        assert_eq!(Tier::of_furnator("powah:magmator_nitro"), None);
        assert_eq!(Tier::of_furnator("mekanism:furnator_basic"), None);
        for (i, t) in Tier::ALL.into_iter().enumerate() {
            assert_eq!(t.index(), i);
            assert_eq!(Tier::from_name(t.name()), Some(t));
        }
    }
}
