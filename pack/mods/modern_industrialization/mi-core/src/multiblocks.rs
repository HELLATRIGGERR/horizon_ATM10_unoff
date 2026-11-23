//! MI's multiblock crafting machines (`machines/blockentities/multiblocks/`): what a controller does with the hatches it
//! matched (`MultiblockInventoryComponent.rebuild`, `AbstractElectricCraftingMultiblockBlockEntity.consumeEu` and its
//! `Behavior`, `AbstractCraftingMultiblockBlockEntity.tick`'s operating states), and the Electric Blast Furnace
//! (`ElectricBlastFurnaceBlockEntity`): one shape for each coil tier, and the recipes a tier bans.

use crate::crafter::{Behavior, Crafter};
use crate::energy::EnergyStore;
use crate::hatches::{Hatch, HatchKind};
use crate::inventory::{FluidSlot, Inventory, ItemSlot};
use crate::machines::Tier;
use crate::multiblock::{Builder, HatchFlags, HatchType, Pos, ShapeTemplate, SimpleMember};
use crate::recipe::{MachineRecipe, RecipeBook, RecipeTypeRules};

/// `MachineTier.MULTIBLOCK`: base 8 and at most 128 EU a tick.
pub const MULTIBLOCK: Tier = Tier { name: "multiblock", base_eu: 8, max_eu: 128, eu_capacity: 12_800 };

/// `ElectricBlastFurnaceBlockEntity.Tier`: a coil and the highest recipe EU it runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EbfTier {
    pub coil: &'static str,
    pub max_base_eu: i64,
    pub english: &'static str,
}

/// MI's two tiers, sorted by their EU (ATM10 adds none).
pub const EBF_TIERS: [EbfTier; 2] = [
    EbfTier { coil: "modern_industrialization:cupronickel_coil", max_base_eu: 32, english: "Cupronickel" },
    EbfTier { coil: "modern_industrialization:kanthal_coil", max_base_eu: 128, english: "Kanthal" },
];

/// The EBF's shape for a tier: heatproof casings 3 by 3 below and above, two hollow rings of coils between; hatches
/// of every kind but energy output in the casings. The controller is the bottom layer's front middle.
pub fn ebf_shape(tier: &EbfTier) -> ShapeTemplate {
    let casings = SimpleMember::block("modern_industrialization:heatproof_machine_casing");
    let coils = SimpleMember::block(tier.coil);
    let hatches = HatchFlags::of(&[HatchType::ItemInput, HatchType::ItemOutput, HatchType::FluidInput, HatchType::FluidOutput, HatchType::EnergyInput]);
    Builder::new("heatproof")
        .add3by3(0, &casings, false, Some(hatches))
        .add3by3(1, &coils, true, None)
        .add3by3(2, &coils, true, None)
        .add3by3(3, &casings, false, Some(hatches))
        .build()
}

/// A crafting multiblock with an electric controller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MultiblockMachine {
    /// Block id path (`electric_blast_furnace`).
    pub id: &'static str,
    pub recipe_type: &'static str,
    pub rules: RecipeTypeRules,
    pub tier: Tier,
    /// Its shapes (`ActiveShapeComponent`), each named as `ShapeSelection` shows it, with the highest recipe EU the
    /// shape runs (`banRecipe`).
    pub shapes: &'static [(&'static str, i64)],
}

const ALL_RULES: RecipeTypeRules = RecipeTypeRules { item_inputs: true, fluid_inputs: true, item_outputs: true, fluid_outputs: true };

/// `MultiblockMachines.ELECTRIC_BLAST_FURNACE` on `MIMachineRecipeTypes.BLAST_FURNACE` (items and fluids in and out).
pub const ELECTRIC_BLAST_FURNACE: MultiblockMachine = MultiblockMachine {
    id: "electric_blast_furnace",
    recipe_type: "modern_industrialization:blast_furnace",
    rules: ALL_RULES,
    tier: MULTIBLOCK,
    shapes: &[("Cupronickel", 32), ("Kanthal", 128)],
};

pub const ALL: [MultiblockMachine; 1] = [ELECTRIC_BLAST_FURNACE];

/// The multiblock a block is the controller of.
pub fn of_block(block: &str) -> Option<&'static MultiblockMachine> {
    let path = block.strip_prefix("modern_industrialization:")?;
    ALL.iter().find(|m| m.id == path)
}

impl MultiblockMachine {
    /// Its shape templates, in `ActiveShapeComponent`'s order.
    pub fn templates(&self) -> Vec<ShapeTemplate> {
        match self.id {
            "electric_blast_furnace" => EBF_TIERS.iter().map(ebf_shape).collect(),
            _ => Vec::new(),
        }
    }
    /// `banRecipe`: past the machine's `getMaxRecipeEu` (no upgrades yet), or past the active shape's tier.
    pub fn bans(&self, shape: usize, recipe_eu: i64) -> bool {
        recipe_eu > self.tier.max_eu || self.shapes.get(shape).is_some_and(|(_, max)| recipe_eu > *max)
    }
}

/// `AbstractCraftingMultiblockBlockEntity.OperatingState`, which MI does not save: a loaded controller matches again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OperatingState {
    /// The shape is not matched: nothing runs and the efficiency falls.
    #[default]
    NotMatched,
    /// Matched again: the recipe resumes once its outputs fit (`tryContinueRecipe`).
    TryingToResume,
    NormalOperation,
}

/// A matched hatch's slots, as its block entity holds them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HatchInventory {
    pub pos: Pos,
    pub hatch: &'static Hatch,
    pub items: Vec<ItemSlot>,
    pub fluids: Vec<FluidSlot>,
    pub energy: EnergyStore,
}

/// `rebuild`'s order: the matched hatches sorted by height, a stable sort (`List.sort`), so hatches at one height keep
/// the order they were matched in.
fn by_height(hatches: &[HatchInventory]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..hatches.len()).collect();
    order.sort_by_key(|&i| hatches[i].pos[1]);
    order
}

/// `MultiblockInventoryComponent`: each hatch's slots appended in `rebuild`'s order (`appendItemInputs`...).
pub fn gather(hatches: &[HatchInventory]) -> Inventory {
    let mut inv = Inventory::default();
    for i in by_height(hatches) {
        let h = &hatches[i];
        match (h.hatch.kind, h.hatch.input) {
            (HatchKind::Item { .. }, true) => inv.item_inputs.extend(h.items.iter().cloned()),
            (HatchKind::Item { .. }, false) => inv.item_outputs.extend(h.items.iter().cloned()),
            (HatchKind::Fluid { .. }, true) => inv.fluid_inputs.extend(h.fluids.iter().cloned()),
            (HatchKind::Fluid { .. }, false) => inv.fluid_outputs.extend(h.fluids.iter().cloned()),
            (HatchKind::Energy { .. }, _) => {}
        }
    }
    inv
}

/// The slots of `inv` (as [`gather`] laid them out) back into their hatches.
pub fn scatter(hatches: &mut [HatchInventory], inv: &Inventory) {
    let (mut ii, mut io, mut fi, mut fo) = (0, 0, 0, 0);
    for i in by_height(hatches) {
        let h = &mut hatches[i];
        let (slots, from, at): (&mut Vec<ItemSlot>, &[ItemSlot], &mut usize) = match (h.hatch.kind, h.hatch.input) {
            (HatchKind::Item { .. }, true) => (&mut h.items, &inv.item_inputs, &mut ii),
            (HatchKind::Item { .. }, false) => (&mut h.items, &inv.item_outputs, &mut io),
            (HatchKind::Fluid { .. }, input) => {
                let (from, at) = if input { (&inv.fluid_inputs, &mut fi) } else { (&inv.fluid_outputs, &mut fo) };
                for s in h.fluids.iter_mut() {
                    if let Some(f) = from.get(*at) {
                        *s = f.clone();
                    }
                    *at += 1;
                }
                continue;
            }
            (HatchKind::Energy { .. }, _) => continue,
        };
        for s in slots.iter_mut() {
            if let Some(f) = from.get(*at) {
                *s = f.clone();
            }
            *at += 1;
        }
    }
}

/// `consumeEu`: from each energy input hatch in turn, in the order the hatches were matched.
pub fn consume_eu(hatches: &mut [HatchInventory], max: i64, act: bool) -> i64 {
    let mut total = 0;
    for h in hatches.iter_mut().filter(|h| h.hatch.hatch_type() == HatchType::EnergyInput) {
        total += h.energy.consume(max - total, act);
    }
    total
}

/// An electric crafting multiblock as its crafter sees it (`AbstractElectricCraftingMultiblockBlockEntity` and the EBF's
/// overrides): EU from the energy hatches, the `MULTIBLOCK` tier, the active shape's ban.
pub struct Electric<'a> {
    pub hatches: &'a mut [HatchInventory],
    pub machine: &'static MultiblockMachine,
    pub shape: usize,
    pub max_stack: &'a dyn Fn(&str) -> u32,
    pub random: &'a mut dyn FnMut() -> f32,
}

impl Behavior for Electric<'_> {
    fn consume_eu(&mut self, max: i64, act: bool) -> i64 {
        consume_eu(self.hatches, max, act)
    }
    fn base_recipe_eu(&self) -> i64 {
        self.machine.tier.base_eu
    }
    fn max_recipe_eu(&self) -> i64 {
        self.machine.tier.max_eu
    }
    fn ban_recipe(&self, recipe: &MachineRecipe) -> bool {
        self.machine.bans(self.shape, i64::from(recipe.eu))
    }
    fn max_stack(&self, item: &str) -> u32 {
        (self.max_stack)(item)
    }
    fn next_float(&mut self) -> f32 {
        (self.random)()
    }
}

/// `AbstractCraftingMultiblockBlockEntity.tick` after `link()`: a matched machine resumes once its outputs fit, then
/// runs its crafter; otherwise its efficiency falls. True when it worked (`isActive`).
pub fn tick(state: &mut OperatingState, crafter: &mut Crafter, book: &RecipeBook, inv: &mut Inventory, behavior: &mut impl Behavior) -> bool {
    if *state == OperatingState::TryingToResume && crafter.try_continue(book, inv, behavior) {
        *state = OperatingState::NormalOperation;
    }
    if *state == OperatingState::NormalOperation {
        crafter.tick(book, inv, behavior)
    } else {
        crafter.decrease_efficiency_ticks();
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hatches;
    use crate::multiblock::{BlockEntityAt, Facing, ShapeMatcher, ShapeWorld, to_world_pos};
    use crate::recipe::{FluidInput, FluidOutput, ItemInput, ItemOutput};
    use std::collections::HashMap;

    fn hatch(id: &str, pos: Pos) -> HatchInventory {
        let hatch = hatches::of_block(&format!("modern_industrialization:{id}")).unwrap();
        HatchInventory {
            pos,
            hatch,
            items: (0..hatch.item_slots()).map(|_| ItemSlot::new(None, 0, 64)).collect(),
            fluids: (0..hatch.fluid_slots()).map(|_| FluidSlot::standard(hatch.fluid_capacity())).collect(),
            energy: EnergyStore::new(0, hatch.eu_capacity()),
        }
    }

    #[test]
    fn ebf_shapes_by_coil() {
        let shapes = ELECTRIC_BLAST_FURNACE.templates();
        assert_eq!(shapes.len(), 2);
        let t = &shapes[0];
        assert_eq!(t.len(), 9 + 8 + 8 + 9 - 1, "the controller is the bottom layer's front middle");
        assert!(t.member([0, 0, 0]).is_none());
        assert_eq!(t.member([0, 1, 0]), Some(&SimpleMember::block("modern_industrialization:cupronickel_coil")));
        assert!(t.member([0, 1, 1]).is_none() && t.member([0, 2, 1]).is_none(), "hollow rings");
        assert_eq!(t.member([0, 3, 1]), Some(&SimpleMember::block("modern_industrialization:heatproof_machine_casing")));
        assert!(t.hatch_flags([1, 0, 2]).unwrap().allows(HatchType::EnergyInput));
        assert!(!t.hatch_flags([1, 3, 2]).unwrap().allows(HatchType::EnergyOutput));
        assert_eq!(t.hatch_flags([1, 1, 2]), None, "no hatch among the coils");
        assert_eq!(shapes[1].member([1, 2, 2]), Some(&SimpleMember::block("modern_industrialization:kanthal_coil")));
        assert_eq!(t.hatch_casing, "heatproof");
        // banRecipe: a cupronickel EBF runs up to 32 EU/t, a kanthal one up to 128, nothing past MULTIBLOCK's 128.
        let ebf = &ELECTRIC_BLAST_FURNACE;
        assert!(!ebf.bans(0, 32) && ebf.bans(0, 64) && !ebf.bans(1, 64) && !ebf.bans(1, 128) && ebf.bans(1, 129));
        assert_eq!(of_block("modern_industrialization:electric_blast_furnace"), Some(&ELECTRIC_BLAST_FURNACE));
        assert_eq!(of_block("modern_industrialization:steam_blast_furnace"), None);
    }

    /// An EBF built in a world of names, its hatches' block entities beside it.
    struct World {
        blocks: HashMap<Pos, String>,
        hatches: HashMap<Pos, BlockEntityAt>,
    }
    impl ShapeWorld for World {
        fn block(&mut self, pos: Pos) -> String {
            self.blocks.get(&pos).cloned().unwrap_or_else(|| "minecraft:air".into())
        }
        fn property(&mut self, _: Pos, _: &str) -> Option<String> {
            None
        }
        fn in_tag(&mut self, _: Pos, _: &str) -> bool {
            false
        }
        fn block_entity(&mut self, pos: Pos) -> BlockEntityAt {
            self.hatches.get(&pos).copied().unwrap_or(BlockEntityAt::None)
        }
    }

    #[test]
    fn a_built_ebf_matches_with_its_hatches_in_each_facing() {
        let t = &ELECTRIC_BLAST_FURNACE.templates()[0];
        for facing in Facing::ALL {
            let c = [0, 64, 0];
            let m = ShapeMatcher::new(c, facing, t);
            let mut w = World { blocks: HashMap::new(), hatches: HashMap::new() };
            for (p, member) in &m.members {
                w.blocks.insert(*p, member.preview(facing).0);
            }
            assert!(m.rematch(&mut w).ok, "{facing:?}");
            // Hatches in two casings of the roof and one of the floor; one where the coils are is a mismatch.
            let at = |p| to_world_pos(c, facing, p);
            for (p, kind) in [([-1, 3, 0], HatchType::ItemInput), ([1, 3, 2], HatchType::FluidOutput), ([1, 0, 1], HatchType::EnergyInput)] {
                w.blocks.insert(at(p), "modern_industrialization:some_hatch".into());
                w.hatches.insert(at(p), BlockEntityAt::Hatch { kind, matched: false });
            }
            let r = m.rematch(&mut w);
            assert!(r.ok, "{facing:?}: {r:?}");
            assert_eq!(r.hatches.len(), 3);
            w.blocks.insert(at([1, 1, 0]), "modern_industrialization:some_hatch".into());
            w.hatches.insert(at([1, 1, 0]), BlockEntityAt::Hatch { kind: HatchType::ItemInput, matched: false });
            let r = m.rematch(&mut w);
            assert_eq!((r.ok, r.mismatching.clone()), (false, vec![at([1, 1, 0])]), "{facing:?}");
            // The kanthal shape doesn't take cupronickel coils.
            w.blocks.insert(at([1, 1, 0]), "modern_industrialization:cupronickel_coil".into());
            w.hatches.remove(&at([1, 1, 0]));
            let kanthal = ShapeMatcher::new(c, facing, &ELECTRIC_BLAST_FURNACE.templates()[1]);
            assert!(!kanthal.rematch(&mut w).ok && m.rematch(&mut w).ok);
        }
    }

    #[test]
    fn hatch_slots_by_height_and_back() {
        // Matched in this order: a roof item input, a floor item input, an output, a fluid input, two energy hatches.
        let mut hs = vec![
            hatch("steel_item_input_hatch", [0, 3, 0]),
            hatch("bronze_item_input_hatch", [1, 0, 0]),
            hatch("steel_item_output_hatch", [2, 0, 0]),
            hatch("steel_fluid_input_hatch", [0, 3, 2]),
            hatch("lv_energy_input_hatch", [0, 3, 1]),
            hatch("mv_energy_input_hatch", [0, 0, 1]),
        ];
        hs[0].items[0] = ItemSlot::new(Some("a:roof"), 3, 64);
        hs[1].items[0] = ItemSlot::new(Some("a:floor"), 5, 64);
        hs[3].fluids[0].fluid = Some("a:oil".into());
        hs[3].fluids[0].amount = 500;
        let mut inv = gather(&hs);
        // The floor's hatch first, then the roof's two slots.
        let items: Vec<_> = inv.item_inputs.iter().map(|s| (s.item.clone(), s.count)).collect();
        assert_eq!(items, vec![(Some("a:floor".into()), 5), (Some("a:roof".into()), 3), (None, 0)]);
        assert_eq!((inv.item_outputs.len(), inv.fluid_inputs.len(), inv.fluid_outputs.len()), (2, 1, 0));
        inv.item_inputs[0].decrement(5);
        inv.item_outputs[1] = ItemSlot::new(Some("a:out"), 2, 64);
        inv.fluid_inputs[0].decrement(100);
        scatter(&mut hs, &inv);
        assert!(hs[1].items[0].is_blank());
        assert_eq!(hs[0].items[0].count, 3);
        assert_eq!(hs[2].items[1].item.as_deref(), Some("a:out"));
        assert_eq!(hs[3].fluids[0].amount, 400);
        assert_eq!(gather(&hs), inv);
        // EU from the energy hatches in their matched order: the LV one first.
        hs[4].energy.eu = 10;
        hs[5].energy.eu = 100;
        assert_eq!(consume_eu(&mut hs, 30, false), 30);
        assert_eq!((hs[4].energy.eu, hs[5].energy.eu), (10, 100));
        assert_eq!(consume_eu(&mut hs, 30, true), 30);
        assert_eq!((hs[4].energy.eu, hs[5].energy.eu), (0, 80));
        assert_eq!(consume_eu(&mut hs, 500, true), 80);
    }

    fn oil() -> MachineRecipe {
        // ATM10's `materials/blast_furnace/raw_oil_to_oil`: 2 EU/t for 200 ticks.
        MachineRecipe {
            id: "mi:raw_oil_to_oil".into(),
            eu: 2,
            duration: 200,
            item_inputs: vec![],
            fluid_inputs: vec![FluidInput { fluids: vec!["mi:raw_synthetic_oil".into()], amount: 100, probability: 1.0 }],
            item_outputs: vec![],
            fluid_outputs: vec![FluidOutput { fluid: "mi:synthetic_oil".into(), amount: 100, probability: 1.0 }],
        }
    }
    fn coke() -> MachineRecipe {
        // ATM10's coke from 16 coals: 64 EU/t for 200 ticks, 16 coke and 8,000 mB of creosote.
        MachineRecipe {
            id: "mi:coke".into(),
            eu: 64,
            duration: 200,
            item_inputs: vec![ItemInput { items: vec!["minecraft:coal".into()], amount: 16, probability: 1.0 }],
            fluid_inputs: vec![],
            item_outputs: vec![ItemOutput { item: "mi:coke".into(), amount: 16, probability: 1.0 }],
            fluid_outputs: vec![FluidOutput { fluid: "mi:creosote".into(), amount: 8000, probability: 1.0 }],
        }
    }

    #[test]
    fn an_ebf_runs_its_tier_and_waits_while_unmatched() {
        let book = RecipeBook::new(vec![oil(), coke()]);
        let mut hs = vec![
            hatch("steel_item_input_hatch", [0, 0, 0]),
            hatch("steel_item_output_hatch", [1, 0, 0]),
            hatch("steel_fluid_input_hatch", [0, 3, 0]),
            hatch("steel_fluid_output_hatch", [1, 3, 0]),
            hatch("steel_fluid_output_hatch", [2, 3, 0]),
            hatch("lv_energy_input_hatch", [2, 0, 0]),
        ];
        hs[5].energy.eu = 19_200;
        hs[2].fluids[0].fluid = Some("mi:raw_synthetic_oil".into());
        hs[2].fluids[0].amount = 200;
        hs[0].items[0] = ItemSlot::new(Some("minecraft:coal"), 16, 64);
        let max_stack = |_: &str| 64;
        let mut random = || 0.0;
        let mut crafter = Crafter::new();
        let mut state = OperatingState::TryingToResume;
        let mut run = |hs: &mut Vec<HatchInventory>, shape: usize, crafter: &mut Crafter, state: &mut OperatingState| {
            let mut inv = gather(hs);
            let mut b = Electric { hatches: hs, machine: &ELECTRIC_BLAST_FURNACE, shape, max_stack: &max_stack, random: &mut random };
            let active = tick(state, crafter, &book, &mut inv, &mut b);
            scatter(hs, &inv);
            active
        };
        // Cupronickel: the coke's 64 EU/t is banned, the oil runs at MULTIBLOCK's base 8 EU/t: 400 EU in 50 ticks.
        assert!(run(&mut hs, 0, &mut crafter, &mut state));
        assert_eq!(state, OperatingState::NormalOperation, "nothing was running: it resumes at once");
        assert_eq!((crafter.active_recipe.as_deref(), crafter.used_energy, hs[2].fluids[0].amount), (Some("mi:raw_oil_to_oil"), 8, 100));
        for _ in 1..50 {
            run(&mut hs, 0, &mut crafter, &mut state);
        }
        assert_eq!((hs[3].fluids[0].fluid.as_deref(), hs[3].fluids[0].amount, hs[5].energy.eu), (Some("mi:synthetic_oil"), 100, 19_200 - 400));
        assert_eq!(hs[0].items[0].count, 16, "the coal waits");
        // Unmatched: nothing runs, the efficiency falls; matched again it resumes.
        run(&mut hs, 0, &mut crafter, &mut state);
        assert_eq!(crafter.used_energy, 8);
        state = OperatingState::NotMatched;
        assert!(!run(&mut hs, 0, &mut crafter, &mut state));
        assert_eq!((crafter.used_energy, crafter.efficiency_ticks), (8, 0));
        state = OperatingState::TryingToResume;
        for _ in 0..49 {
            run(&mut hs, 0, &mut crafter, &mut state);
        }
        assert_eq!(hs[3].fluids[0].amount, 200);
        // Kanthal: the coke runs at its own 64 EU/t; its creosote goes to the empty fluid output.
        crafter = Crafter::new();
        state = OperatingState::TryingToResume;
        assert!(run(&mut hs, 1, &mut crafter, &mut state));
        assert_eq!((crafter.active_recipe.as_deref(), crafter.recipe_max_eu, hs[0].items[0].count), (Some("mi:coke"), 64, 0));
        for _ in 1..200 {
            run(&mut hs, 1, &mut crafter, &mut state);
        }
        assert_eq!(hs[1].items[0].item.as_deref(), Some("mi:coke"));
        assert_eq!((hs[4].fluids[0].fluid.as_deref(), hs[4].fluids[0].amount), (Some("mi:creosote"), 8000));
    }

    #[test]
    fn resuming_waits_for_room_and_reserves_the_outputs_again() {
        let book = RecipeBook::new(vec![coke()]);
        let mut inv = Inventory {
            item_inputs: vec![ItemSlot::new(None, 0, 64)],
            item_outputs: vec![ItemSlot::new(Some("a:full"), 64, 64)],
            fluid_inputs: vec![],
            fluid_outputs: vec![FluidSlot::standard(16_000)],
        };
        let mut c = Crafter { active_recipe: Some("mi:coke".into()), used_energy: 640, recipe_energy: 12_800, ..Crafter::new() };
        let mut hs: Vec<HatchInventory> = vec![];
        let max_stack = |_: &str| 64;
        let mut random = || 0.0;
        let mut b = Electric { hatches: &mut hs, machine: &ELECTRIC_BLAST_FURNACE, shape: 1, max_stack: &max_stack, random: &mut random };
        let mut state = OperatingState::TryingToResume;
        assert!(!tick(&mut state, &mut c, &book, &mut inv, &mut b));
        assert_eq!(state, OperatingState::TryingToResume, "the coke has no room");
        inv.item_outputs[0] = ItemSlot::new(None, 0, 64);
        assert!(c.try_continue(&book, &mut inv, &mut b));
        assert_eq!(inv.item_outputs[0].locked.as_deref(), Some("mi:coke"));
        assert_eq!(inv.fluid_outputs[0].locked.as_deref(), Some("mi:creosote"));
        assert!(Crafter::new().try_continue(&book, &mut inv, &mut b), "nothing to resume");
    }
}
