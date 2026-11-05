//! NeoForge's `FluidInteractionRegistry` (docs/mod-fluids.md, slice 7b): the
//! interactions content mods add for a fluid's type, which
//! `LiquidBlock.shouldSpreadLiquid` runs as `FluidInteractionRegistry.canInteract`
//! once mods' fluids are in the registry ([`active`]). NeoForge's own two for
//! lava (water beside it, soul soil under it with blue ice beside it) are the
//! level's (`level/fluids.rs`); these come after them, in the content mods'
//! order. The list is set once per process from the startup content root,
//! after the fluids ([`crate::mod_fluids::register_fluids`]).
use crate::fluid::FluidKind;
use std::{collections::HashMap, sync::OnceLock};

/// One `InteractionInformation` as the NeoForge oracle ran it: it holds with
/// one of `below` under the fluid (any when empty) and, beside it, one of
/// `fluids` or one of `blocks`; the fluid's block becomes `source` or
/// `flowing` (a block id, its default state) by whether it is a source, with
/// NeoForge's fizz when `fizz`.
#[derive(Clone, Debug, PartialEq)]
pub struct FluidInteraction {
    pub below: Vec<String>,
    pub fluids: Vec<FluidKind>,
    pub blocks: Vec<String>,
    pub source: String,
    pub flowing: String,
    pub fizz: bool,
}

/// An interaction as a content mod declares it: for `fluid` (the current
/// fluid's source id), with its neighbour fluids by id.
#[derive(Clone, Debug, PartialEq)]
pub struct DeclaredInteraction {
    pub fluid: String,
    pub below: Vec<String>,
    pub fluids: Vec<String>,
    pub blocks: Vec<String>,
    pub source: String,
    pub flowing: String,
    pub fizz: bool,
}

static REGISTRY: OnceLock<HashMap<FluidKind, Vec<FluidInteraction>>> = OnceLock::new();

/// A vanilla or registered fluid by id.
fn kind(name: &str) -> Option<FluidKind> {
    FluidKind::ALL
        .into_iter()
        .find(|k| k.name() == name)
        .or_else(|| crate::mod_fluids::kind_by_name(name))
}

/// Registers the content mods' interactions, in their order, keyed by their
/// fluid's source. A fluid the registry lacks (a mod's that isn't loaded) is
/// left out of a neighbour list, and an interaction for one is left out. Once
/// per process: false when they already were.
pub fn register_interactions(declared: impl IntoIterator<Item = DeclaredInteraction>) -> bool {
    let mut map: HashMap<FluidKind, Vec<FluidInteraction>> = HashMap::new();
    for d in declared {
        let Some(current) = kind(&d.fluid).filter(|k| *k != FluidKind::Empty) else {
            log::warn!(
                "Horizon: fluid interaction of {} left out: no such fluid",
                d.fluid
            );
            continue;
        };
        let fluids: Vec<FluidKind> = d
            .fluids
            .iter()
            .filter_map(|f| kind(f))
            .filter(|k| *k != FluidKind::Empty)
            .collect();
        if fluids.is_empty() && d.blocks.is_empty() {
            continue;
        }
        map.entry(current.source())
            .or_default()
            .push(FluidInteraction {
                below: d.below,
                fluids,
                blocks: d.blocks,
                source: d.source,
                flowing: d.flowing,
                fizz: d.fizz,
            });
    }
    REGISTRY.set(map).is_ok()
}

/// The interactions content mods added for `kind`'s type (by its source), in
/// order.
pub fn interactions_for(kind: FluidKind) -> &'static [FluidInteraction] {
    if kind == FluidKind::Empty {
        return &[];
    }
    REGISTRY
        .get()
        .and_then(|m| m.get(&kind.source()))
        .map_or(&[], Vec::as_slice)
}

/// Whether NeoForge's `FluidInteractionRegistry.canInteract` replaces
/// vanilla's `shouldSpreadLiquid`: once content mods' fluids are in the
/// registry. With vanilla's fluids alone the two agree but for a datapack's
/// water or lava tag, which vanilla's rule reads and NeoForge's (by type) does
/// not, so vanilla keeps its own.
pub fn active() -> bool {
    !crate::mod_fluids::mod_fluids().is_empty()
}

/// Interactions for [`crate::mod_fluids::register_test_fluids`]' fluids:
/// lava meeting brine (obsidian, cobblestone), brine meeting water (packed
/// ice, ice), sludge over soul soil beside blue ice (prismarine, dark
/// prismarine, no fizz), and two that name a fluid the registry lacks. Not
/// for the game.
#[doc(hidden)]
pub fn register_test_interactions() {
    crate::mod_fluids::register_test_fluids();
    let rule = |fluid: &str, fluids: &[&str], source: &str, flowing: &str| DeclaredInteraction {
        fluid: fluid.into(),
        below: Vec::new(),
        fluids: fluids.iter().map(|f| (*f).to_owned()).collect(),
        blocks: Vec::new(),
        source: source.into(),
        flowing: flowing.into(),
        fizz: true,
    };
    register_interactions([
        rule(
            "minecraft:lava",
            &["test:brine", "test:flowing_brine"],
            "minecraft:obsidian",
            "minecraft:cobblestone",
        ),
        rule(
            "test:brine",
            &["test:missing", "minecraft:water", "minecraft:flowing_water"],
            "minecraft:packed_ice",
            "minecraft:ice",
        ),
        DeclaredInteraction {
            below: vec!["minecraft:soul_soil".into()],
            blocks: vec!["minecraft:blue_ice".into()],
            fizz: false,
            ..rule(
                "test:sludge",
                &[],
                "minecraft:prismarine",
                "minecraft:dark_prismarine",
            )
        },
        rule(
            "test:missing",
            &["minecraft:water"],
            "minecraft:stone",
            "minecraft:stone",
        ),
        rule(
            "minecraft:water",
            &["test:missing"],
            "minecraft:stone",
            "minecraft:stone",
        ),
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interactions_are_keyed_by_their_fluids_source_in_order() {
        register_test_interactions();
        let brine = crate::mod_fluids::kind_by_name("test:brine").unwrap();
        let sludge = crate::mod_fluids::kind_by_name("test:sludge").unwrap();
        let lava = interactions_for(FluidKind::FlowingLava);
        assert_eq!(lava.len(), 1, "flowing lava has lava's");
        assert_eq!(lava[0].fluids, [brine, brine.flowing()]);
        let brines = interactions_for(brine.flowing());
        assert_eq!(
            brines[0].fluids,
            [FluidKind::Water, FluidKind::FlowingWater],
            "an unknown neighbour is dropped"
        );
        assert_eq!(
            (brines[0].source.as_str(), brines[0].flowing.as_str()),
            ("minecraft:packed_ice", "minecraft:ice")
        );
        let sludges = interactions_for(sludge);
        assert_eq!(
            (
                sludges[0].below.as_slice(),
                sludges[0].blocks.as_slice(),
                sludges[0].fizz
            ),
            (
                &["minecraft:soul_soil".to_owned()][..],
                &["minecraft:blue_ice".to_owned()][..],
                false
            )
        );
        // A fluid the registry lacks has none, nor one that meets nothing.
        assert!(interactions_for(FluidKind::Water).is_empty());
        assert!(interactions_for(FluidKind::Empty).is_empty());
    }
}
