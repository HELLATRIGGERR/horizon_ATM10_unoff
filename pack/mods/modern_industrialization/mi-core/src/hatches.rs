//! MI's hatches (`machines/init/MultiblockHatches.java`, `machines/blockentities/hatches/`): the item, fluid and
//! energy hatches of each of MI's tiers, their slots and capacities, and what each gives the multiblock that matched it
//! (`appendItemInputs`, `appendFluidOutputs`, `appendEnergyInputs`...). The nuclear and large tank hatches are not
//! here: no ported multiblock takes them.

use crate::multiblock::HatchType;

/// What a hatch holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HatchKind {
    /// `rows` by `columns` standard item slots from (`x`, `y`) (`SlotPositions.addSlots`: a row at a time).
    Item { rows: usize, columns: usize, x: i32, y: i32 },
    /// One standard fluid slot of `buckets` buckets at (80, 40).
    Fluid { buckets: i64 },
    /// An `EnergyComponent` of `30 * 20 * cable_eu` EU (`CableTier.eu`); its bar at (76, 39).
    Energy { cable_eu: i64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hatch {
    /// Block id path (`steel_item_input_hatch`).
    pub id: &'static str,
    pub kind: HatchKind,
    pub input: bool,
    /// `upgradesToSteel`: every item and fluid hatch but bronze's (steam multiblocks run at the steel tier with one).
    pub upgrades_to_steel: bool,
    /// The casing it is made of (`MachineCasings`; an energy hatch's is its cable tier's).
    pub casing: &'static str,
}

/// `FLUID_HATCH_SLOT_X`, `FLUID_HATCH_SLOT_Y`.
pub const FLUID_SLOT: [i32; 2] = [80, 40];
/// `EnergyHatch`'s `EnergyBar.Params`.
pub const ENERGY_BAR: [i32; 2] = [76, 39];

const fn item(id: &'static str, input: bool, casing: &'static str, rows: usize, columns: usize, x: i32, y: i32) -> Hatch {
    let bronze = matches!(casing.as_bytes(), b"bronze");
    Hatch { id, kind: HatchKind::Item { rows, columns, x, y }, input, upgrades_to_steel: !bronze, casing }
}

const fn fluid(id: &'static str, input: bool, casing: &'static str, buckets: i64) -> Hatch {
    let bronze = matches!(casing.as_bytes(), b"bronze");
    Hatch { id, kind: HatchKind::Fluid { buckets }, input, upgrades_to_steel: !bronze, casing }
}

const fn energy(id: &'static str, input: bool, casing: &'static str, cable_eu: i64) -> Hatch {
    Hatch { id, kind: HatchKind::Energy { cable_eu }, input, upgrades_to_steel: false, casing }
}

/// `MultiblockHatches`' item, fluid and energy hatches, input then output, in its order.
pub const ALL: [Hatch; 30] = [
    // registerItemHatches(englishPrefix, prefix, casing, rows, columns, xStart, yStart)
    item("bronze_item_input_hatch", true, "bronze", 1, 1, 80, 40),
    item("bronze_item_output_hatch", false, "bronze", 1, 1, 80, 40),
    item("steel_item_input_hatch", true, "steel", 2, 1, 80, 30),
    item("steel_item_output_hatch", false, "steel", 2, 1, 80, 30),
    item("advanced_item_input_hatch", true, "mv", 2, 2, 71, 30),
    item("advanced_item_output_hatch", false, "mv", 2, 2, 71, 30),
    item("turbo_item_input_hatch", true, "hv", 3, 3, 62, 21),
    item("turbo_item_output_hatch", false, "hv", 3, 3, 62, 21),
    item("highly_advanced_item_input_hatch", true, "ev", 3, 5, 44, 18),
    item("highly_advanced_item_output_hatch", false, "ev", 3, 5, 44, 18),
    // registerFluidHatches(englishPrefix, prefix, casing, bucketCapacity)
    fluid("bronze_fluid_input_hatch", true, "bronze", 4),
    fluid("bronze_fluid_output_hatch", false, "bronze", 4),
    fluid("steel_fluid_input_hatch", true, "steel", 16),
    fluid("steel_fluid_output_hatch", false, "steel", 16),
    fluid("advanced_fluid_input_hatch", true, "mv", 64),
    fluid("advanced_fluid_output_hatch", false, "mv", 64),
    fluid("turbo_fluid_input_hatch", true, "hv", 256),
    fluid("turbo_fluid_output_hatch", false, "hv", 256),
    fluid("highly_advanced_fluid_input_hatch", true, "ev", 1024),
    fluid("highly_advanced_fluid_output_hatch", false, "ev", 1024),
    // registerEnergyHatches(CableTier): `CableTier.eu` of LV, MV, HV, EV and the superconductor.
    energy("lv_energy_input_hatch", true, "lv", 32),
    energy("lv_energy_output_hatch", false, "lv", 32),
    energy("mv_energy_input_hatch", true, "mv", 32 * 4),
    energy("mv_energy_output_hatch", false, "mv", 32 * 4),
    energy("hv_energy_input_hatch", true, "hv", 32 * 4 * 8),
    energy("hv_energy_output_hatch", false, "hv", 32 * 4 * 8),
    energy("ev_energy_input_hatch", true, "ev", 32 * 4 * 8 * 8),
    energy("ev_energy_output_hatch", false, "ev", 32 * 4 * 8 * 8),
    energy("superconductor_energy_input_hatch", true, "superconductor", 128_000_000),
    energy("superconductor_energy_output_hatch", false, "superconductor", 128_000_000),
];

/// The hatch a block is (`modern_industrialization:steel_item_input_hatch`).
pub fn of_block(block: &str) -> Option<&'static Hatch> {
    let path = block.strip_prefix("modern_industrialization:")?;
    ALL.iter().find(|h| h.id == path)
}

impl Hatch {
    /// `getHatchType`.
    pub fn hatch_type(&self) -> HatchType {
        match (self.kind, self.input) {
            (HatchKind::Item { .. }, true) => HatchType::ItemInput,
            (HatchKind::Item { .. }, false) => HatchType::ItemOutput,
            (HatchKind::Fluid { .. }, true) => HatchType::FluidInput,
            (HatchKind::Fluid { .. }, false) => HatchType::FluidOutput,
            (HatchKind::Energy { .. }, true) => HatchType::EnergyInput,
            (HatchKind::Energy { .. }, false) => HatchType::EnergyOutput,
        }
    }
    pub fn item_slots(&self) -> usize {
        match self.kind {
            HatchKind::Item { rows, columns, .. } => rows * columns,
            _ => 0,
        }
    }
    /// The item slots' `Slot.x`, `Slot.y`, a row at a time.
    pub fn item_positions(&self) -> Vec<[i32; 2]> {
        match self.kind {
            HatchKind::Item { rows, columns, x, y } => (0..rows).flat_map(|r| (0..columns).map(move |c| [x + 18 * c as i32, y + 18 * r as i32])).collect(),
            _ => Vec::new(),
        }
    }
    pub fn fluid_slots(&self) -> usize {
        usize::from(matches!(self.kind, HatchKind::Fluid { .. }))
    }
    /// The fluid slot's capacity in millibuckets.
    pub fn fluid_capacity(&self) -> i64 {
        match self.kind {
            HatchKind::Fluid { buckets } => buckets * 1000,
            _ => 0,
        }
    }
    /// The energy hatch's `EnergyComponent` capacity: 30 seconds of its cable tier.
    pub fn eu_capacity(&self) -> i64 {
        match self.kind {
            HatchKind::Energy { cable_eu } => 30 * 20 * cable_eu,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hatches_as_multiblock_hatches_registers_them() {
        let steel = of_block("modern_industrialization:steel_item_input_hatch").unwrap();
        assert_eq!((steel.item_slots(), steel.hatch_type(), steel.upgrades_to_steel), (2, HatchType::ItemInput, true));
        assert_eq!(steel.item_positions(), vec![[80, 30], [80, 48]], "two rows of one column");
        let turbo = of_block("modern_industrialization:turbo_item_output_hatch").unwrap();
        assert_eq!(turbo.item_positions()[..4], [[62, 21], [80, 21], [98, 21], [62, 39]], "a row at a time");
        assert_eq!(of_block("modern_industrialization:highly_advanced_item_input_hatch").unwrap().item_slots(), 15);
        let bronze = of_block("modern_industrialization:bronze_fluid_input_hatch").unwrap();
        assert_eq!((bronze.fluid_capacity(), bronze.upgrades_to_steel, bronze.hatch_type()), (4000, false, HatchType::FluidInput));
        let lv = of_block("modern_industrialization:lv_energy_input_hatch").unwrap();
        assert_eq!((lv.eu_capacity(), lv.hatch_type(), lv.item_slots()), (19_200, HatchType::EnergyInput, 0));
        assert_eq!(of_block("modern_industrialization:ev_energy_output_hatch").unwrap().eu_capacity(), 600 * 8192);
        assert!(of_block("modern_industrialization:nuclear_item_hatch").is_none());
        assert!(of_block("other:steel_item_input_hatch").is_none());
        // Every id is unique and names its own kind and direction.
        for h in ALL {
            assert_eq!(ALL.iter().filter(|o| o.id == h.id).count(), 1, "{}", h.id);
            assert!(h.id.contains(if h.input { "_input_" } else { "_output_" }), "{}", h.id);
            assert!(h.id.contains(&format!("_{}_", h.hatch_type().id().split('_').next().unwrap())), "{}", h.id);
        }
    }
}
