use crate::crops::Tier;

/// What a crop is grown from: `CropType` (`api/crop/CropType.java`). It picks the crafting seed of the crop's seed
/// recipes, `lib/ModCorePlugin.java`'s `setCraftingSeed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `CropType.RESOURCE`: its crafting seed is the Prosperity Seed Base.
    Resource,
    /// `CropType.MOB`: its crafting seed is the Soulium Seed Base.
    Mob,
}

impl Kind {
    /// `CropType.getCraftingSeed`.
    pub const fn crafting_seed(self) -> &'static str {
        match self {
            Kind::Resource => "mysticalagriculture:prosperity_seed_base",
            Kind::Mob => "mysticalagriculture:soulium_seed_base",
        }
    }
}

/// A crop's crafting material, the `LazyIngredient` its `Crop` is built with (`api/lib/LazyIngredient.java`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    /// `LazyIngredient.item`: one item.
    Item(&'static str),
    /// `LazyIngredient.tag`: an item tag.
    Tag(&'static str),
    /// `LazyIngredient.item("mysticalagriculture:soul_jar", MobSoulUtils.makeComponentMap(type))`: a Soul Jar full
    /// of the crop's own mob's souls (every mob crop's soul type is named as the crop is).
    SoulJar,
    /// `LazyIngredient.item("mysticalagriculture:experience_capsule", ...)` holding `MAX_XP_POINTS`.
    Capsule,
}

impl Material {
    /// The item or tag the material is made of. A Soul Jar's souls and a Capsule's experience are components, which
    /// an item id does not say: the runtime takes any Soul Jar or Capsule for them.
    pub fn accepts(self) -> Accepts {
        match self {
            Material::Item(id) => Accepts::Item(id.into()),
            Material::Tag(id) => Accepts::Tag(id.into()),
            Material::SoulJar => Accepts::Item("mysticalagriculture:soul_jar".into()),
            Material::Capsule => Accepts::Item("mysticalagriculture:experience_capsule".into()),
        }
    }
}

/// What an ingredient takes: one item, or the items of a tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Accepts {
    Item(String),
    Tag(String),
}

/// One crop of `ModCrops`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub name: &'static str,
    pub tier: Tier,
    pub kind: Kind,
    pub material: Material,
}

const fn res(name: &'static str, tier: Tier, material: Material) -> Row {
    Row { name, tier, kind: Kind::Resource, material }
}

/// A `CropType.MOB` crop: its material is the Soul Jar of its mob.
const fn mob(name: &'static str, tier: Tier, material: Material) -> Row {
    Row { name, tier, kind: Kind::Mob, material }
}

use Material::{Capsule, Item, SoulJar, Tag};

// MA's own crops (`lib/ModCrops.java` of Mystical Agriculture 1.21.1-8.0.29), in its order: the crop's name, its tier,
// its type and its crafting material. Translated as a table: `mysticalagriculture:<name>_crop`, `<name>_seeds` and
// `<name>_essence`.
pub const CROPS: &[Row] = &[
    res("air", Tier::Elemental, Item("mysticalagriculture:air_agglomeratio")),
    res("earth", Tier::Elemental, Item("mysticalagriculture:earth_agglomeratio")),
    res("water", Tier::Elemental, Item("mysticalagriculture:water_agglomeratio")),
    res("fire", Tier::Elemental, Item("mysticalagriculture:fire_agglomeratio")),
    res("inferium", Tier::One, Item("mysticalagriculture:inferium_essence")),
    res("stone", Tier::One, Item("minecraft:stone")),
    res("dirt", Tier::One, Item("minecraft:dirt")),
    res("wood", Tier::One, Tag("minecraft:logs")),
    res("ice", Tier::One, Item("minecraft:ice")),
    res("deepslate", Tier::One, Item("minecraft:deepslate")),
    res("nature", Tier::Two, Item("mysticalagriculture:nature_agglomeratio")),
    res("dye", Tier::Two, Item("mysticalagriculture:dye_agglomeratio")),
    res("nether", Tier::Two, Item("mysticalagriculture:nether_agglomeratio")),
    res("coal", Tier::Two, Item("minecraft:coal")),
    res("coral", Tier::Two, Item("mysticalagriculture:coral_agglomeratio")),
    res("honey", Tier::Two, Item("mysticalagriculture:honey_agglomeratio")),
    res("amethyst", Tier::Two, Item("minecraft:amethyst_shard")),
    mob("pig", Tier::Two, SoulJar),
    mob("chicken", Tier::Two, SoulJar),
    mob("cow", Tier::Two, SoulJar),
    mob("sheep", Tier::Two, SoulJar),
    mob("squid", Tier::Two, SoulJar),
    mob("fish", Tier::Two, SoulJar),
    mob("slime", Tier::Two, SoulJar),
    mob("turtle", Tier::Two, SoulJar),
    mob("armadillo", Tier::Two, SoulJar),
    res("iron", Tier::Three, Tag("c:ingots/iron")),
    res("copper", Tier::Three, Item("minecraft:copper_ingot")),
    res("nether_quartz", Tier::Three, Tag("c:gems/quartz")),
    res("glowstone", Tier::Three, Tag("c:dusts/glowstone")),
    res("redstone", Tier::Three, Tag("c:dusts/redstone")),
    res("obsidian", Tier::Three, Item("minecraft:obsidian")),
    res("prismarine", Tier::Three, Item("mysticalagriculture:prismarine_agglomeratio")),
    res("sculk", Tier::Three, Item("minecraft:sculk")),
    mob("zombie", Tier::Three, SoulJar),
    mob("skeleton", Tier::Three, SoulJar),
    mob("creeper", Tier::Three, SoulJar),
    mob("spider", Tier::Three, SoulJar),
    mob("phantom", Tier::Three, SoulJar),
    mob("rabbit", Tier::Three, SoulJar),
    res("gold", Tier::Four, Tag("c:ingots/gold")),
    res("lapis_lazuli", Tier::Four, Tag("c:gems/lapis")),
    res("end", Tier::Four, Item("mysticalagriculture:end_agglomeratio")),
    res("experience", Tier::Four, Capsule),
    mob("breeze", Tier::Four, SoulJar),
    mob("blaze", Tier::Four, SoulJar),
    mob("ghast", Tier::Four, SoulJar),
    mob("enderman", Tier::Four, SoulJar),
    res("diamond", Tier::Five, Tag("c:gems/diamond")),
    res("emerald", Tier::Five, Tag("c:gems/emerald")),
    res("netherite", Tier::Five, Item("minecraft:netherite_ingot")),
    mob("wither_skeleton", Tier::Five, SoulJar),
    res("rubber", Tier::Two, Tag("c:rubber")),
    res("silicon", Tier::Two, Tag("c:silicon")),
    res("sulfur", Tier::Two, Tag("c:dusts/sulfur")),
    res("aluminum", Tier::Two, Tag("c:ingots/aluminum")),
    res("saltpeter", Tier::Two, Tag("mysticalagriculture:material/saltpeter")),
    res("tin", Tier::Three, Tag("c:ingots/tin")),
    res("bronze", Tier::Three, Tag("c:ingots/bronze")),
    res("zinc", Tier::Three, Tag("c:ingots/zinc")),
    res("brass", Tier::Three, Tag("c:ingots/brass")),
    res("silver", Tier::Three, Tag("c:ingots/silver")),
    res("lead", Tier::Three, Tag("c:ingots/lead")),
    res("graphite", Tier::Three, Tag("c:ingots/graphite")),
    res("steel", Tier::Four, Tag("c:ingots/steel")),
    res("nickel", Tier::Four, Tag("c:ingots/nickel")),
    res("constantan", Tier::Four, Tag("c:ingots/constantan")),
    res("electrum", Tier::Four, Tag("c:ingots/electrum")),
    res("invar", Tier::Four, Tag("c:ingots/invar")),
    res("uranium", Tier::Four, Tag("c:ingots/uranium")),
    res("platinum", Tier::Five, Tag("c:ingots/platinum")),
    res("iridium", Tier::Five, Tag("c:ingots/iridium")),
    res("apatite", Tier::Two, Tag("c:gems/apatite")),
    res("ruby", Tier::Four, Tag("c:gems/ruby")),
    res("sapphire", Tier::Four, Tag("c:gems/sapphire")),
    res("peridot", Tier::Four, Tag("c:gems/peridot")),
    res("soulium", Tier::Four, Item("mysticalagriculture:soulium_dust")),
    mob("blizz", Tier::Three, SoulJar),
    mob("blitz", Tier::Three, SoulJar),
    mob("basalz", Tier::Three, SoulJar),
    res("signalum", Tier::Four, Tag("c:ingots/signalum")),
    res("lumium", Tier::Four, Tag("c:ingots/lumium")),
    res("enderium", Tier::Five, Tag("c:ingots/enderium")),
    res("flux_infused_ingot", Tier::Four, Item("redstone_arsenal:flux_ingot")),
    res("flux_infused_gem", Tier::Five, Item("redstone_arsenal:flux_gem")),
    res("hop_graphite", Tier::Four, Tag("c:ingots/hop_graphite")),
    res("amethyst_bronze", Tier::Three, Tag("c:ingots/amethyst_bronze")),
    res("slimesteel", Tier::Three, Tag("c:ingots/slimesteel")),
    res("pig_iron", Tier::Three, Tag("c:ingots/pig_iron")),
    res("cobalt", Tier::Four, Tag("c:ingots/cobalt")),
    res("rose_gold", Tier::Four, Tag("c:ingots/rose_gold")),
    res("manyullyn", Tier::Five, Tag("c:ingots/manyullyn")),
    res("queens_slime", Tier::Five, Tag("c:ingots/queens_slime")),
    res("hepatizon", Tier::Five, Tag("c:ingots/hepatizon")),
    res("grains_of_infinity", Tier::Two, Item("enderio:grains_of_infinity")),
    res("copper_alloy", Tier::Three, Item("enderio:copper_alloy_ingot")),
    res("redstone_alloy", Tier::Three, Item("enderio:redstone_alloy_ingot")),
    res("conductive_alloy", Tier::Three, Item("enderio:conductive_alloy_ingot")),
    res("soularium", Tier::Four, Item("enderio:soularium_ingot")),
    res("dark_steel", Tier::Four, Item("enderio:dark_steel_ingot")),
    res("pulsating_alloy", Tier::Four, Item("enderio:pulsating_alloy_ingot")),
    res("energetic_alloy", Tier::Four, Item("enderio:energetic_alloy_ingot")),
    res("vibrant_alloy", Tier::Five, Item("enderio:vibrant_alloy_ingot")),
    res("end_steel", Tier::Five, Item("enderio:end_steel_ingot")),
    res("mystical_flower", Tier::Two, Item("mysticalagriculture:mystical_flower_agglomeratio")),
    res("manasteel", Tier::Three, Item("botania:manasteel_ingot")),
    res("elementium", Tier::Four, Item("botania:elementium_ingot")),
    res("terrasteel", Tier::Five, Item("botania:terrasteel_ingot")),
    res("osmium", Tier::Four, Tag("c:ingots/osmium")),
    res("fluorite", Tier::Four, Tag("c:gems/fluorite")),
    res("refined_glowstone", Tier::Four, Tag("c:ingots/refined_glowstone")),
    res("refined_obsidian", Tier::Four, Tag("c:ingots/refined_obsidian")),
    res("marble", Tier::Two, Tag("mysticalagriculture:material/marble")),
    res("limestone", Tier::Two, Tag("mysticalagriculture:material/limestone")),
    res("basalt", Tier::Two, Tag("chisel:basalt")),
    res("steeleaf", Tier::Three, Item("twilightforest:steeleaf_ingot")),
    res("ironwood", Tier::Three, Item("twilightforest:ironwood_ingot")),
    res("knightmetal", Tier::Four, Item("twilightforest:knightmetal_ingot")),
    res("fiery_ingot", Tier::Four, Item("twilightforest:fiery_ingot")),
    res("menril", Tier::Two, Item("integrateddynamics:menril_berries")),
    res("aquamarine", Tier::Three, Item("astralsorcery:aquamarine")),
    res("starmetal", Tier::Four, Item("astralsorcery:starmetal_ingot")),
    res("rock_crystal", Tier::Five, Item("astralsorcery:rock_crystal")),
    res("compressed_iron", Tier::Four, Tag("c:ingots/compressed_iron")),
    res("draconium", Tier::Five, Tag("c:ingots/draconium")),
    res("yellorium", Tier::Five, Tag("c:ingots/yellorium")),
    res("cyanite", Tier::Five, Tag("c:ingots/cyanite")),
    res("sky_stone", Tier::Three, Item("ae2:sky_stone_block")),
    res("certus_quartz", Tier::Three, Tag("c:gems/certus_quartz")),
    res("fluix", Tier::Four, Tag("c:gems/fluix")),
    res("quartz_enriched_iron", Tier::Three, Item("refinedstorage:quartz_enriched_iron")),
    res("energized_steel", Tier::Four, Item("powah:steel_energized")),
    res("blazing_crystal", Tier::Four, Item("powah:crystal_blazing")),
    res("niotic_crystal", Tier::Five, Item("powah:crystal_niotic")),
    res("spirited_crystal", Tier::Five, Item("powah:crystal_spirited")),
    res("uraninite", Tier::Five, Item("powah:uraninite")),
];
