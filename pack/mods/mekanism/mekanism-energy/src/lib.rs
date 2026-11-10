//! Mekanism 10.7.19's energy, as All the Mods 10 runs it, with Mekanism
//! Generators 10.7.19's Solar and Heat Generators. Mekanism is MIT,
//! Copyright (c) 2017-2025 Aidan C. Brady.
//!
//! Energy is in Joules (`long`); Forge Energy is converted at the edges,
//! `feConversionRate` Joules to the FE (`fe`). Energy cubes and universal
//! cables come in tiers sized by `tiers.toml` (`tier`, `config`), and
//! machines and cable networks hand energy to their neighbours with the
//! even split of `EmitUtils.sendToAcceptors` (`split`). Energy is held in
//! containers with rates by automation type (`container`); blocks expose it
//! by side, as their side configuration says (`sides`); an energy cube keeps
//! its Joules and config and ejects out of its output faces (`cube`). Its
//! screen reads data slots and sends buttons (`menu`), and writes energy as
//! Mekanism's screens do (`display`). Mekanism Generators' Solar Generator
//! (`solar`, over Minecraft 1.21.1's sun and weather in `sky`) and Heat
//! Generator (`heat`) fill stores of their own and eject like a cube, as does
//! the Gas-Burning Generator (`gas`), which burns chemicals by their fuel,
//! the Bio-Generator (`bio`), which burns bioethanol, the Wind Generator
//! (`wind`), which follows its height, and the Advanced Solar Generator
//! (`solar`'s nine checks).
//! Mekanism's first processing machines (`machine`: the Enrichment Chamber,
//! Crusher, Energized Smelter and Metallurgic Infuser) run recipes on their
//! Joules at the pace their upgrades set. Mekanism 10.7's chemicals
//! (`chemical`: the registry, stacks, tanks and the chemical handler by side)
//! fill the infuser's tank and the Electrolytic Separator's (`separator`), the
//! Chemical Tanks and the items that keep their contents (`chemical_tank`),
//! and the Pressurized Tubes' networks (`network`, one chemical a network);
//! the Chemical Oxidizer and Chemical Infuser make chemicals (`chemical_machine`),
//! and the advanced machines take one a tick (`advanced`); the ore chain's
//! Chemical Dissolution Chamber, Chemical Washer and Chemical Crystallizer
//! (`ore`) and the Rotary Condensentrator (`rotary`) carry an ore through
//! slurries to crystals and chemicals to fluids.
//! The Combiner, the Precision Sawmill and the factories (`factory`) run
//! their recipes in processes of their own on one store, and tier
//! installers turn a machine into a factory and a factory into the next tier.
//! Mechanical Pipes move fluids in networks as the tubes do (`network`, with
//! `transmitter`'s tiers), between Fluid Tanks (`fluid_tank`) and any block with
//! a fluid handler. Logistical Transporters (`transporter`) send item stacks
//! along the cheapest paths to the inventories that take them. The
//! Configurator's modes (`configurator`) set sides, colours and facings.
//! Multiblocks form as Mekanism's do (`multiblock`: the structure, its
//! cuboid, the check of its blocks, the cache by its id): the Dynamic Tank
//! (`dynamic_tank`) and the Induction Matrix (`induction`).
//! Nothing here knows the game: the native mod's helpers bring the world.
pub mod advanced;
pub mod bio;
pub mod chemical;
pub mod chemical_machine;
pub mod chemical_tank;
pub mod config;
pub mod configurator;
pub mod container;
pub mod cube;
pub mod display;
pub mod dynamic_tank;
pub mod factory;
pub mod fe;
pub mod fluid_tank;
pub mod gas;
pub mod generator;
pub mod heat;
pub mod induction;
pub mod machine;
pub mod menu;
pub mod multiblock;
pub mod network;
pub mod ore;
pub mod rotary;
pub mod separator;
pub mod sides;
pub mod sky;
pub mod solar;
pub mod split;
pub mod tier;
pub mod transmitter;
pub mod transporter;
pub mod wind;

/// Java's `(long) d` for the energy maths: NaN is 0, the rest saturates.
pub(crate) fn clamp_to_long(d: f64) -> i64 {
    if d.is_nan() {
        0
    } else if d >= i64::MAX as f64 {
        i64::MAX
    } else if d <= i64::MIN as f64 {
        i64::MIN
    } else {
        d as i64
    }
}
