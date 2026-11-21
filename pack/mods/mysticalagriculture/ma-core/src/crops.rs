//! MA's crops: tiers, where a crop grows, and what it drops (`api/crop/Crop.java`, `CropTier.java`,
//! `block/MysticalCropBlock.java`, `block/InferiumCropBlock.java`, `lib/ModCorePlugin.java`).

use crate::crop_table::CROPS;
use serde_json::Value;

pub const NS: &str = "mysticalagriculture";
/// `CropBlock.MAX_AGE` (MA's crops are wheat-like, ages 0 to 7).
pub const MAX_AGE: u32 = 7;
/// `MysticalCropBlock` with `INFERIUM` never needs its farmland (`ModCrops.INFERIUM`).
pub const INFERIUM: &str = "inferium";

/// `CropTier`'s built-in tiers. `ELEMENTAL` has value 1, as `ONE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Elemental,
    One,
    Two,
    Three,
    Four,
    Five,
}

/// A tier as the game reads it: built in, or a MysticalCustomization tier
/// (`config/mysticalcustomization/tiers/<name>.json`).
#[derive(Clone, Debug, PartialEq)]
pub struct TierInfo {
    pub id: String,
    pub value: u32,
    /// `CropTier.getFarmland`: the tier's effective farmland.
    pub farmland: Option<String>,
    pub essence: Option<String>,
    pub secondary_seed_drop: bool,
    /// `CropTier.baseSecondaryChance` (0.1 unless a tier sets another).
    pub base_secondary_chance: f64,
}

impl Tier {
    pub fn value(self) -> u32 {
        match self {
            Tier::Elemental | Tier::One => 1,
            Tier::Two => 2,
            Tier::Three => 3,
            Tier::Four => 4,
            Tier::Five => 5,
        }
    }
    /// `ModCorePlugin.onRegisterCrops`: each tier's farmland and essence (ELEMENTAL shares ONE's).
    pub fn info(self) -> TierInfo {
        let (id, name) = match self {
            Tier::Elemental => ("elemental", "inferium"),
            Tier::One => ("1", "inferium"),
            Tier::Two => ("2", "prudentium"),
            Tier::Three => ("3", "tertium"),
            Tier::Four => ("4", "imperium"),
            Tier::Five => ("5", "supremium"),
        };
        TierInfo {
            id: format!("{NS}:{id}"),
            value: self.value(),
            farmland: Some(format!("{NS}:{name}_farmland")),
            essence: Some(format!("{NS}:{name}_essence")),
            secondary_seed_drop: true,
            base_secondary_chance: 0.1,
        }
    }
    pub fn from_id(id: &str) -> Option<Tier> {
        Some(match id.strip_prefix("mysticalagriculture:")? {
            "elemental" => Tier::Elemental,
            "1" => Tier::One,
            "2" => Tier::Two,
            "3" => Tier::Three,
            "4" => Tier::Four,
            "5" => Tier::Five,
            _ => return None,
        })
    }
}

/// `InfusedFarmlandBlock.FARMLANDS`: the blocks that are `IEssenceFarmland`, and their tier's value.
pub fn essence_farmland_tier(block: &str) -> Option<u32> {
    Some(match block {
        "mysticalagriculture:inferium_farmland" => 1,
        "mysticalagriculture:prudentium_farmland" => 2,
        "mysticalagriculture:tertium_farmland" => 3,
        "mysticalagriculture:imperium_farmland" => 4,
        "mysticalagriculture:supremium_farmland" => 5,
        // MysticalAgradditions' tier 6 farmland (`InsaniumFarmlandBlock`, an `InfusedFarmlandBlock`).
        "mysticalagradditions:insanium_farmland" => 6,
        _ => return None,
    })
}

/// A crop as registered: its name in MA's namespace, its tier, and its crux.
#[derive(Clone, Debug, PartialEq)]
pub struct Crop {
    pub name: String,
    pub tier: TierInfo,
    /// `Crop.getCruxBlock`: the block two below that the crop needs to grow.
    pub crux: Option<String>,
    /// `Crop.respectsEffectiveFarmland` (true for every crop MA and the pack register).
    pub respects_effective_farmland: bool,
    /// `Crop.baseSecondaryChance`, `-1` when the tier's is used.
    pub base_secondary_chance: f64,
}

impl Crop {
    fn new(name: &str, tier: TierInfo) -> Self {
        Crop { name: name.into(), tier, crux: None, respects_effective_farmland: true, base_secondary_chance: -1.0 }
    }
    pub fn block(&self) -> String {
        format!("{NS}:{}_crop", self.name)
    }
    pub fn seeds(&self) -> String {
        format!("{NS}:{}_seeds", self.name)
    }
    /// `Crop.getEssenceItem` (`<name>_essence`; INFERIUM's is `inferium_essence` too).
    pub fn essence(&self) -> String {
        format!("{NS}:{}_essence", self.name)
    }
    /// `CropTier.isEffectiveFarmland`.
    pub fn is_effective_farmland(&self, block: &str) -> bool {
        self.tier.farmland.as_deref() == Some(block)
    }
    /// `MysticalCropBlock.canGrow`: the crux two below, then (when the config asks it) the tier's farmland.
    /// No crop of MA or ATM10 asks for biomes.
    pub fn can_grow(&self, below: &str, below2: &str, requires_effective_farmland: bool) -> bool {
        if let Some(crux) = &self.crux
            && crux != below2
        {
            return false;
        }
        if requires_effective_farmland && self.name != INFERIUM && !self.is_effective_farmland(below) {
            return false;
        }
        true
    }
    /// `Crop.getSecondaryChance(below)`.
    pub fn secondary_chance(&self, below: &str) -> f64 {
        if !self.tier.secondary_seed_drop {
            return 0.0;
        }
        let mut chance = 0.0;
        if essence_farmland_tier(below).is_some() {
            chance += if self.base_secondary_chance > -1.0 { self.base_secondary_chance } else { self.tier.base_secondary_chance };
        }
        if self.respects_effective_farmland && self.is_effective_farmland(below) {
            chance += 0.1;
        }
        f64::min(chance, 1.0)
    }
}

/// The crop of a block id (`mysticalagriculture:<name>_crop`), from MA's own table.
pub fn builtin(block: &str) -> Option<Crop> {
    let name = block.strip_prefix("mysticalagriculture:")?.strip_suffix("_crop")?;
    let row = CROPS.iter().find(|row| row.name == name)?;
    Some(Crop::new(row.name, row.tier.info()))
}

/// A MysticalCustomization tier file (`tiers/<name>.json`) as `CustomTierLoader` reads it.
pub fn custom_tier(id: &str, json: &Value) -> Option<TierInfo> {
    Some(TierInfo {
        id: id.into(),
        value: json.get("value")?.as_u64()? as u32,
        farmland: json.get("farmland").and_then(Value::as_str).map(str::to_owned),
        essence: json.get("essence").and_then(Value::as_str).map(str::to_owned),
        secondary_seed_drop: json.get("secondarySeedDrop").and_then(Value::as_bool).unwrap_or(true),
        base_secondary_chance: json.get("baseSecondaryChance").and_then(Value::as_f64).unwrap_or(0.1),
    })
}

/// A MysticalCustomization crop file (`crops/<name>.json`): its tier (`mysticalagriculture:3` or a custom
/// tier's id), its crux and its secondary chance. `tiers` are the custom tiers by id.
pub fn custom_crop(name: &str, json: &Value, tiers: &[TierInfo]) -> Option<Crop> {
    let tier_id = json.get("tier")?.as_str()?;
    let tier = match Tier::from_id(tier_id) {
        Some(t) => t.info(),
        None => tiers.iter().find(|t| t.id == tier_id)?.clone(),
    };
    let mut crop = Crop::new(name, tier);
    crop.crux = json.get("crux").and_then(Value::as_str).map(str::to_owned);
    if let Some(c) = json.get("baseSecondaryChance").and_then(Value::as_f64) {
        crop.base_secondary_chance = c;
    }
    if let Some(r) = json.get("respectsEffectiveFarmland").and_then(Value::as_bool) {
        crop.respects_effective_farmland = r;
    }
    Some(crop)
}

/// What a harvest drops, in the order the block lists them.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Drops {
    pub essence: u32,
    pub seeds: u32,
    pub fertilized: bool,
}

/// MA's settings that the crops read (`mysticalagriculture-common.toml`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CropSettings {
    pub secondary_seed_drops: bool,
    pub fertilized_essence_chance: f64,
    pub requires_effective_farmland: bool,
}

impl Default for CropSettings {
    fn default() -> Self {
        CropSettings { secondary_seed_drops: true, fertilized_essence_chance: 0.1, requires_effective_farmland: false }
    }
}

/// `MysticalCropBlock.getDrops` and `InferiumCropBlock.getDrops`. `below` is the block under the crop when the
/// loot has an origin. `roll` gives `Math.random()`'s draws, in the Java order.
pub fn drops(crop: &Crop, age: u32, below: Option<&str>, settings: &CropSettings, mut roll: impl FnMut() -> f64) -> Drops {
    let mut d = Drops { essence: 0, seeds: 1, fertilized: false };
    if age != MAX_AGE {
        return d;
    }
    d.essence = 1;
    let Some(below) = below else { return d };
    let chance = crop.secondary_chance(below);
    if crop.name == INFERIUM {
        if let Some(tier) = essence_farmland_tier(below) {
            d.essence = (0.5 * f64::from(tier) + 0.5) as u32;
            if tier > 1 && tier % 2 == 0 && roll() < 0.5 {
                d.essence += 1;
            }
        }
        if settings.secondary_seed_drops && roll() < chance {
            d.seeds = 2;
        }
        return d;
    }
    if roll() < chance {
        d.essence = 2;
    }
    if settings.secondary_seed_drops && roll() < chance {
        d.seeds = 2;
    }
    if roll() < settings.fertilized_essence_chance {
        d.fertilized = true;
    }
    d
}

/// One block around a crop, as vanilla's `CropBlock.getGrowthSpeed` reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Soil {
    /// The block sustains the crop: farmland, MA's essence farmland among it (`FarmBlock`).
    pub farmland: bool,
    /// `isFertile`: its `moisture` above 0.
    pub moist: bool,
}

/// `CropBlock.getGrowthSpeed`: `soil` is the 3x3 under the crop, row by row from (-1, -1) to (1, 1), centre at
/// index 4; `same` says which neighbours are the same crop block, in the order west, east, north, south, then the
/// diagonals north-west, north-east, south-east, south-west.
pub fn growth_speed(soil: &[Soil; 9], same: &[bool; 8]) -> f32 {
    let mut f = 1.0f32;
    for (i, s) in soil.iter().enumerate() {
        let mut f1 = 0.0f32;
        if s.farmland {
            f1 = 1.0;
            if s.moist {
                f1 = 3.0;
            }
        }
        if i != 4 {
            f1 /= 4.0;
        }
        f += f1;
    }
    let row = same[0] || same[1];
    let column = same[2] || same[3];
    if row && column {
        f /= 2.0;
    } else if same[4..].iter().any(|b| *b) {
        f /= 2.0;
    }
    f
}

/// `CropBlock.randomTick`'s chance: the bound of `random.nextInt` whose 0 grows the crop one age.
pub fn growth_bound(speed: f32) -> i32 {
    (25.0f32 / speed) as i32 + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn always(v: f64) -> impl FnMut() -> f64 {
        move || v
    }

    #[test]
    fn table_has_mas_crops_and_tiers() {
        assert_eq!(CROPS.len(), 136);
        assert_eq!(builtin("mysticalagriculture:inferium_crop").unwrap().tier.value, 1);
        assert_eq!(builtin("mysticalagriculture:iron_crop").unwrap().tier.id, "mysticalagriculture:3");
        assert_eq!(builtin("mysticalagriculture:diamond_crop").unwrap().tier.farmland.as_deref(), Some("mysticalagriculture:supremium_farmland"));
        assert_eq!(builtin("mysticalagriculture:air_crop").unwrap().tier.farmland.as_deref(), Some("mysticalagriculture:inferium_farmland"));
        assert!(builtin("mysticalagriculture:inferium_seeds").is_none());
        assert!(builtin("minecraft:wheat").is_none());
    }

    #[test]
    fn secondary_chance_adds_the_essence_farmland_and_the_effective_one() {
        let iron = builtin("mysticalagriculture:iron_crop").unwrap();
        assert_eq!(iron.secondary_chance("minecraft:farmland"), 0.0);
        assert_eq!(iron.secondary_chance("mysticalagriculture:inferium_farmland"), 0.1);
        assert!((iron.secondary_chance("mysticalagriculture:tertium_farmland") - 0.2).abs() < 1e-12);
    }

    #[test]
    fn requires_effective_farmland_but_not_for_inferium() {
        let iron = builtin("mysticalagriculture:iron_crop").unwrap();
        assert!(iron.can_grow("minecraft:farmland", "minecraft:dirt", false));
        assert!(!iron.can_grow("minecraft:farmland", "minecraft:dirt", true));
        assert!(iron.can_grow("mysticalagriculture:tertium_farmland", "minecraft:dirt", true));
        let inferium = builtin("mysticalagriculture:inferium_crop").unwrap();
        assert!(inferium.can_grow("minecraft:farmland", "minecraft:dirt", true));
    }

    #[test]
    fn crux_two_below() {
        let tiers = [custom_tier("mysticalcustomization:magical", &serde_json::json!({"value": 7, "farmland": "mysticalagradditions:insanium_farmland", "secondarySeedDrop": false})).unwrap()];
        let json = serde_json::json!({"tier": "mysticalcustomization:magical", "crux": "kubejs:magical_soil"});
        let crop = custom_crop("allthemodium", &json, &tiers).unwrap();
        assert_eq!(crop.block(), "mysticalagriculture:allthemodium_crop");
        assert!(!crop.can_grow("minecraft:farmland", "minecraft:dirt", false));
        assert!(crop.can_grow("minecraft:farmland", "kubejs:magical_soil", false));
        assert_eq!(crop.secondary_chance("mysticalagradditions:insanium_farmland"), 0.0);
        let azure = custom_crop("azure_silver", &serde_json::json!({"tier": "mysticalagriculture:3"}), &tiers).unwrap();
        assert_eq!(azure.tier.value, 3);
    }

    #[test]
    fn drops_of_a_resource_crop() {
        let iron = builtin("mysticalagriculture:iron_crop").unwrap();
        let s = CropSettings::default();
        assert_eq!(drops(&iron, 3, Some("minecraft:farmland"), &s, always(0.0)), Drops { essence: 0, seeds: 1, fertilized: false });
        // Plain farmland: no secondary chance, so even a roll of 0 is no second essence; the fertilizer rolls.
        assert_eq!(drops(&iron, 7, Some("minecraft:farmland"), &s, always(0.0)), Drops { essence: 1, seeds: 1, fertilized: true });
        assert_eq!(drops(&iron, 7, Some("mysticalagriculture:tertium_farmland"), &s, always(0.15)), Drops { essence: 2, seeds: 2, fertilized: false });
        let atm10 = CropSettings { secondary_seed_drops: false, ..s };
        assert_eq!(drops(&iron, 7, Some("mysticalagriculture:tertium_farmland"), &atm10, always(0.05)), Drops { essence: 2, seeds: 1, fertilized: true });
        // Without an origin only the essence and the seed.
        assert_eq!(drops(&iron, 7, None, &s, always(0.0)), Drops { essence: 1, seeds: 1, fertilized: false });
    }

    #[test]
    fn inferium_drops_by_its_farmland_tier() {
        let inf = builtin("mysticalagriculture:inferium_crop").unwrap();
        let s = CropSettings { secondary_seed_drops: false, ..CropSettings::default() };
        let n = |farmland: &str, r: f64| drops(&inf, 7, Some(farmland), &s, always(r)).essence;
        assert_eq!(n("minecraft:farmland", 0.0), 1);
        assert_eq!(n("mysticalagriculture:inferium_farmland", 0.0), 1);
        assert_eq!(n("mysticalagriculture:prudentium_farmland", 0.9), 1);
        assert_eq!(n("mysticalagriculture:prudentium_farmland", 0.1), 2);
        assert_eq!(n("mysticalagriculture:tertium_farmland", 0.0), 2);
        assert_eq!(n("mysticalagriculture:imperium_farmland", 0.1), 3);
        assert_eq!(n("mysticalagriculture:supremium_farmland", 0.0), 3);
        // Inferium never drops fertilized essence.
        assert!(!drops(&inf, 7, Some("minecraft:farmland"), &s, always(0.0)).fertilized);
    }

    #[test]
    fn growth_speed_as_vanilla() {
        let moist = Soil { farmland: true, moist: true };
        let dry = Soil { farmland: true, moist: false };
        // A lone crop on moist farmland with moist farmland around: 1 + 3 + 8 * 0.75 = 10.
        assert_eq!(growth_speed(&[moist; 9], &[false; 8]), 10.0);
        assert_eq!(growth_bound(10.0), 3);
        // Dry farmland under it only: 1 + 1 = 2, bound 13.
        let mut alone = [Soil::default(); 9];
        alone[4] = dry;
        assert_eq!(growth_speed(&alone, &[false; 8]), 2.0);
        assert_eq!(growth_bound(2.0), 13);
        // A row of the same crop beside it halves nothing; a row and a column halve it.
        assert_eq!(growth_speed(&[moist; 9], &[true, false, false, false, false, false, false, false]), 10.0);
        assert_eq!(growth_speed(&[moist; 9], &[true, false, true, false, false, false, false, false]), 5.0);
        assert_eq!(growth_speed(&[moist; 9], &[false, false, false, false, false, true, false, false]), 5.0);
    }
}
