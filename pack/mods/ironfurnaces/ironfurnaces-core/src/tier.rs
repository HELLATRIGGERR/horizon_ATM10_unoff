//! The furnace tiers (`BlockXFurnaceTile`): each block's id, its default cooking time and, for the modded
//! ones and the Rainbow furnace, how many items one cooking finishes.

/// A furnace block of Iron Furnaces, from the slowest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tier {
    Copper,
    Iron,
    Silver,
    Gold,
    Diamond,
    Emerald,
    Crystal,
    Obsidian,
    Netherite,
    /// The Rainbow Furnace (`million_furnace`).
    Million,
    Allthemodium,
    Vibranium,
    Unobtainium,
}

impl Tier {
    pub const ALL: [Tier; 13] = [
        Tier::Copper,
        Tier::Iron,
        Tier::Silver,
        Tier::Gold,
        Tier::Diamond,
        Tier::Emerald,
        Tier::Crystal,
        Tier::Obsidian,
        Tier::Netherite,
        Tier::Million,
        Tier::Allthemodium,
        Tier::Vibranium,
        Tier::Unobtainium,
    ];

    /// The block's path in `ironfurnaces:`.
    pub fn block_name(self) -> &'static str {
        match self {
            Tier::Copper => "copper_furnace",
            Tier::Iron => "iron_furnace",
            Tier::Silver => "silver_furnace",
            Tier::Gold => "gold_furnace",
            Tier::Diamond => "diamond_furnace",
            Tier::Emerald => "emerald_furnace",
            Tier::Crystal => "crystal_furnace",
            Tier::Obsidian => "obsidian_furnace",
            Tier::Netherite => "netherite_furnace",
            Tier::Million => "million_furnace",
            Tier::Allthemodium => "allthemodium_furnace",
            Tier::Vibranium => "vibranium_furnace",
            Tier::Unobtainium => "unobtainium_furnace",
        }
    }

    /// The tier of a block id (`ironfurnaces:iron_furnace`).
    pub fn of_block(id: &str) -> Option<Tier> {
        let name = id.strip_prefix("ironfurnaces:")?;
        Tier::ALL.into_iter().find(|t| t.block_name() == name)
    }

    /// The `config/ironfurnaces-common.toml` section: `(table, name)`. The Rainbow furnace's is
    /// `furnaces.rainbow_furnace`.
    pub fn config_section(self) -> String {
        match self {
            Tier::Million => "furnaces.rainbow_furnace".into(),
            Tier::Allthemodium | Tier::Vibranium | Tier::Unobtainium => format!("modded_furnaces.{}", self.block_name()),
            _ => format!("furnaces.{}", self.block_name()),
        }
    }

    /// The `Config.xFurnaceSpeed` default: ticks one cooking takes for a recipe of 200 ticks.
    pub fn default_speed(self) -> i32 {
        match self {
            Tier::Copper => 180,
            Tier::Iron => 160,
            Tier::Silver => 140,
            Tier::Gold => 120,
            Tier::Diamond => 80,
            Tier::Emerald | Tier::Crystal => 40,
            Tier::Obsidian | Tier::Million => 20,
            Tier::Netherite | Tier::Allthemodium => 5,
            Tier::Vibranium => 3,
            Tier::Unobtainium => 1,
        }
    }

    /// `Config.xFurnaceSmeltMult` (the Rainbow furnace's is the fixed 64 of `smelt`): how many items one
    /// cooking finishes; 1 for the others.
    pub fn default_mult(self) -> i32 {
        match self {
            Tier::Million => 64,
            Tier::Allthemodium => 16,
            Tier::Vibranium => 32,
            Tier::Unobtainium => 64,
            _ => 1,
        }
    }
}
