//! What the fork's helpers read of the world for a mod's machine: an item's food
//! (`native_food_table`, `native_food`), a biome's climate at a block (`native_biome_climate`)
//! and the dimension type in place (`native_dimension_type`).

/// An item's food (`FoodProperties`, vanilla's `minecraft:food` component), as `native_food_table`
/// and `native_food` give it: what a machine that makes something of food reads (Mekanism's
/// Nutritional Liquifier builds its recipe from the nutrition and the remainder).
#[derive(Clone, Debug, Default, PartialEq)]
// exhaustive: the game builds it with a literal; mods only read it
#[derive(horizon_mod_api_macros::Wire)]
pub struct NativeFood {
    /// The item's registry id.
    pub item: String,
    /// `nutrition`: the hunger points it gives back.
    pub nutrition: u32,
    /// `saturation`: what it adds to the saturation level.
    pub saturation: f32,
    /// `can_always_eat`: eaten on a full hunger bar.
    pub can_always_eat: bool,
    /// What eating it leaves, an item id: 1.21.1's `FoodProperties.usingConvertsTo`, the
    /// `use_remainder` component here (a stew's bowl). None when it leaves nothing.
    pub remainder: Option<String>,
}

/// What falls in a biome when it rains (`Biome.Precipitation`), as `native_biome_climate` reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[derive(horizon_mod_api_macros::Wire)]
pub enum NativePrecipitation {
    /// The biome has none (`hasPrecipitation` false: a desert, a savanna).
    #[default]
    None,
    /// Rain.
    Rain,
    /// Too cold there: the temperature below 0.15.
    Snow,
}

/// A biome and its climate at a block, as `native_biome_climate` reads them.
#[derive(Clone, Debug, Default, PartialEq)]
// exhaustive: the game builds it with a literal; mods only read it
#[derive(horizon_mod_api_macros::Wire)]
pub struct NativeBiome {
    /// Its id, `minecraft:plains`.
    pub id: String,
    /// `Biome.getBaseTemperature`: the biome file's `temperature`.
    pub base_temperature: f32,
    /// `Biome.getTemperature(pos)`: the base (0.2 in a frozen ocean's cold patches), cooler with
    /// height above the dimension's sea level + 17.
    pub temperature: f32,
    /// `ClimateSettings.downfall`.
    pub downfall: f32,
    /// `Biome.getPrecipitationAt(pos)`.
    pub precipitation: NativePrecipitation,
}

/// What the type of the dimension in place says of its sky and air (`DimensionType`), as
/// `native_dimension_type` reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
// exhaustive: the game builds it with a literal; mods only read it
#[derive(horizon_mod_api_macros::Wire)]
pub struct NativeDimensionType {
    /// `hasSkyLight`: the overworld's, not the Nether's or the End's.
    pub has_skylight: bool,
    /// `hasCeiling`: the Nether's bedrock roof.
    pub has_ceiling: bool,
    /// `ultraWarm`: the Nether's (water evaporates, lava flows far).
    pub ultra_warm: bool,
}
