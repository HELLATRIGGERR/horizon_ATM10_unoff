//! The ported machines as `SingleBlockCraftingMachines.registerMachineTiers` defines them: recipe type,
//! slot counts and positions, the GUI's bars, and `MachineTier`'s EU. The electric (LV) tier.
//! Machines with more than 5 item slots (mixer, chemical reactor, assembler) are deferred pending a
//! larger container type; the hopper's 5 slots cover every other machine here.

use crate::recipe::RecipeTypeRules;

/// `MachineTier`: base and maximum EU a tick, and the stored EU (`getMaxStoredEu`, also the LV casing's
/// `CableTier.LV.eu * 100`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tier {
    pub name: &'static str,
    pub base_eu: i64,
    pub max_eu: i64,
    pub eu_capacity: i64,
}

pub const LV: Tier = Tier { name: "lv", base_eu: 8, max_eu: 32, eu_capacity: 3200 };

/// `ProgressBar.Params`: where it is, its texture (`textures/gui/progress_bar/<kind>.png`, its empty half
/// above its full half) and its size (20 by 20 unless given).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bar {
    pub x: i32,
    pub y: i32,
    pub kind: &'static str,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Machine {
    /// Block id path (`electric_macerator`).
    pub id: &'static str,
    pub recipe_type: &'static str,
    pub rules: RecipeTypeRules,
    pub item_inputs: usize,
    pub item_outputs: usize,
    pub fluid_inputs: usize,
    pub fluid_outputs: usize,
    /// `Slot.x`, `Slot.y` of the item slots (inputs, then outputs) and the fluid slots.
    pub item_slots: &'static [[i32; 2]],
    pub fluid_slots: &'static [[i32; 2]],
    pub progress: Bar,
    /// `RecipeEfficiencyBar.Params`.
    pub efficiency: [i32; 2],
    /// `EnergyBar.Params`.
    pub energy: [i32; 2],
    pub background_height: i32,
    /// `ioBucketCapacity`: each fluid slot holds this many buckets.
    pub io_buckets: i64,
    pub tier: Tier,
}

impl Machine {
    pub fn item_slots(&self) -> usize {
        self.item_inputs + self.item_outputs
    }
    pub fn fluid_slots(&self) -> usize {
        self.fluid_inputs + self.fluid_outputs
    }
    pub fn fluid_capacity(&self) -> i64 {
        1000 * self.io_buckets
    }
}

const ITEMS_ONLY: RecipeTypeRules = RecipeTypeRules { item_inputs: true, fluid_inputs: false, item_outputs: true, fluid_outputs: false };
const FLUIDS_ONLY: RecipeTypeRules = RecipeTypeRules { item_inputs: false, fluid_inputs: true, item_outputs: false, fluid_outputs: true };
const ALL_IO: RecipeTypeRules = RecipeTypeRules { item_inputs: true, fluid_inputs: true, item_outputs: true, fluid_outputs: true };
const DEFAULT_ENERGY_BAR: [i32; 2] = [18, 34];

/// `"Macerator", MACERATOR, 1, 4, 0, 0`, `items.addSlot(56, 35).addSlots(102, 27, 2, 2)`.
pub const ELECTRIC_MACERATOR: Machine = Machine {
    id: "electric_macerator",
    recipe_type: "modern_industrialization:macerator",
    rules: ITEMS_ONLY,
    item_inputs: 1,
    item_outputs: 4,
    fluid_inputs: 0,
    fluid_outputs: 0,
    item_slots: &[[56, 35], [102, 27], [120, 27], [102, 45], [120, 45]],
    fluid_slots: &[],
    progress: Bar { x: 77, y: 33, kind: "macerate", width: 20, height: 20 },
    efficiency: [38, 66],
    energy: DEFAULT_ENERGY_BAR,
    background_height: 166,
    io_buckets: 16,
    tier: LV,
};

/// `"Compressor", COMPRESSOR, 1, 1, 0, 0`, `items.addSlot(56, 35).addSlot(102, 35)`.
pub const ELECTRIC_COMPRESSOR: Machine = Machine {
    id: "electric_compressor",
    recipe_type: "modern_industrialization:compressor",
    rules: ITEMS_ONLY,
    item_inputs: 1,
    item_outputs: 1,
    fluid_inputs: 0,
    fluid_outputs: 0,
    item_slots: &[[56, 35], [102, 35]],
    fluid_slots: &[],
    progress: Bar { x: 77, y: 34, kind: "compress", width: 20, height: 20 },
    efficiency: [38, 62],
    energy: [18, 30],
    background_height: 166,
    io_buckets: 16,
    tier: LV,
};

/// `"Cutting Machine", CUTTING_MACHINE, 1, 1, 1, 0`, `items.addSlot(60, 35).addSlot(120, 35)`,
/// `fluids.addSlot(40, 35)`: an item and lubricant.
pub const ELECTRIC_CUTTING_MACHINE: Machine = Machine {
    id: "electric_cutting_machine",
    recipe_type: "modern_industrialization:cutting_machine",
    rules: RecipeTypeRules { item_inputs: true, fluid_inputs: true, item_outputs: true, fluid_outputs: false },
    item_inputs: 1,
    item_outputs: 1,
    fluid_inputs: 1,
    fluid_outputs: 0,
    item_slots: &[[60, 35], [120, 35]],
    fluid_slots: &[[40, 35]],
    progress: Bar { x: 88, y: 31, kind: "slice", width: 20, height: 20 },
    efficiency: [38, 62],
    energy: [15, 34],
    background_height: 166,
    io_buckets: 16,
    tier: LV,
};

/// `"Furnace", FURNACE, 1, 1, 0, 0`, `items.addSlot(56, 35).addSlot(102, 35)`.
pub const ELECTRIC_FURNACE: Machine = Machine {
    id: "electric_furnace",
    recipe_type: "modern_industrialization:furnace",
    rules: ITEMS_ONLY,
    item_inputs: 1, item_outputs: 1, fluid_inputs: 0, fluid_outputs: 0,
    item_slots: &[[56, 35], [102, 35]],
    fluid_slots: &[],
    progress: Bar { x: 77, y: 33, kind: "arrow", width: 20, height: 20 },
    efficiency: [38, 62], energy: [18, 30], background_height: 166, io_buckets: 16, tier: LV,
};

/// `"Distillery", DISTILLERY, 0, 0, 1, 1`, `fluids.addSlot(56, 35).addSlot(102, 35)`.
pub const ELECTRIC_DISTILLERY: Machine = Machine {
    id: "electric_distillery",
    recipe_type: "modern_industrialization:distillery",
    rules: FLUIDS_ONLY,
    item_inputs: 0, item_outputs: 0, fluid_inputs: 1, fluid_outputs: 1,
    item_slots: &[],
    fluid_slots: &[[56, 35], [102, 35]],
    progress: Bar { x: 77, y: 33, kind: "arrow", width: 20, height: 20 },
    efficiency: [38, 62], energy: [18, 30], background_height: 166, io_buckets: 16, tier: LV,
};

/// `"Centrifuge", CENTRIFUGE, 1, 4, 1, 4`,
/// `items.addSlot(42, 27).addSlots(93, 27, 2, 2)`, `fluids.addSlot(42, 45).addSlots(131, 27, 2, 2)`.
pub const ELECTRIC_CENTRIFUGE: Machine = Machine {
    id: "electric_centrifuge",
    recipe_type: "modern_industrialization:centrifuge",
    rules: ALL_IO,
    item_inputs: 1, item_outputs: 4, fluid_inputs: 1, fluid_outputs: 4,
    item_slots: &[[42, 27], [93, 27], [111, 27], [93, 45], [111, 45]],
    fluid_slots: &[[42, 45], [131, 27], [149, 27], [131, 45], [149, 45]],
    progress: Bar { x: 65, y: 33, kind: "centrifuge", width: 20, height: 20 },
    efficiency: [50, 66], energy: DEFAULT_ENERGY_BAR, background_height: 166, io_buckets: 16, tier: LV,
};

/// `"Electrolyzer", ELECTROLYZER, 1, 4, 1, 4`,
/// `items.addSlot(42, 27).addSlots(93, 27, 2, 2)`, `fluids.addSlot(42, 47).addSlots(131, 27, 2, 2)`.
pub const ELECTRIC_ELECTROLYZER: Machine = Machine {
    id: "electric_electrolyzer",
    recipe_type: "modern_industrialization:electrolyzer",
    rules: ALL_IO,
    item_inputs: 1, item_outputs: 4, fluid_inputs: 1, fluid_outputs: 4,
    item_slots: &[[42, 27], [93, 27], [111, 27], [93, 45], [111, 45]],
    fluid_slots: &[[42, 47], [131, 27], [149, 27], [131, 45], [149, 45]],
    progress: Bar { x: 66, y: 35, kind: "arrow", width: 20, height: 20 },
    efficiency: [50, 66], energy: DEFAULT_ENERGY_BAR, background_height: 166, io_buckets: 16, tier: LV,
};

/// `"Packer", PACKER, 3, 1, 0, 0`, `items.addSlots(56, 18, 1, 3).addSlot(102, 36)`.
pub const ELECTRIC_PACKER: Machine = Machine {
    id: "electric_packer",
    recipe_type: "modern_industrialization:packer",
    rules: ITEMS_ONLY,
    item_inputs: 3, item_outputs: 1, fluid_inputs: 0, fluid_outputs: 0,
    item_slots: &[[56, 18], [56, 36], [56, 54], [102, 36]],
    fluid_slots: &[],
    progress: Bar { x: 77, y: 33, kind: "arrow", width: 20, height: 20 },
    efficiency: [38, 74], energy: [18, 30], background_height: 178, io_buckets: 16, tier: LV,
};

/// `"Unpacker", UNPACKER, 1, 2, 0, 0`, `items.addSlots(56, 36, 1, 1).addSlots(102, 27, 1, 2)`.
pub const ELECTRIC_UNPACKER: Machine = Machine {
    id: "electric_unpacker",
    recipe_type: "modern_industrialization:unpacker",
    rules: ITEMS_ONLY,
    item_inputs: 1, item_outputs: 2, fluid_inputs: 0, fluid_outputs: 0,
    item_slots: &[[56, 36], [102, 27], [102, 45]],
    fluid_slots: &[],
    progress: Bar { x: 77, y: 33, kind: "arrow", width: 20, height: 20 },
    efficiency: [38, 66], energy: [18, 30], background_height: 166, io_buckets: 16, tier: LV,
};

/// `"Polarizer", POLARIZER, 2, 1, 0, 0`, `items.addSlots(56, 23, 1, 2).addSlot(102, 32)`.
pub const ELECTRIC_POLARIZER: Machine = Machine {
    id: "electric_polarizer",
    recipe_type: "modern_industrialization:polarizer",
    rules: ITEMS_ONLY,
    item_inputs: 2, item_outputs: 1, fluid_inputs: 0, fluid_outputs: 0,
    item_slots: &[[56, 23], [56, 41], [102, 32]],
    fluid_slots: &[],
    progress: Bar { x: 77, y: 30, kind: "magnet", width: 20, height: 20 },
    efficiency: [38, 62], energy: [18, 30], background_height: 166, io_buckets: 16, tier: LV,
};

/// `"Wiremill", WIREMILL, 1, 1, 0, 0`, `items.addSlot(56, 35).addSlot(102, 35)`.
pub const ELECTRIC_WIREMILL: Machine = Machine {
    id: "electric_wiremill",
    recipe_type: "modern_industrialization:wiremill",
    rules: ITEMS_ONLY,
    item_inputs: 1, item_outputs: 1, fluid_inputs: 0, fluid_outputs: 0,
    item_slots: &[[56, 35], [102, 35]],
    fluid_slots: &[],
    progress: Bar { x: 77, y: 34, kind: "wiremill", width: 20, height: 20 },
    efficiency: [38, 62], energy: [18, 30], background_height: 166, io_buckets: 16, tier: LV,
};

pub const ALL: [Machine; 11] = [
    ELECTRIC_MACERATOR, ELECTRIC_COMPRESSOR, ELECTRIC_CUTTING_MACHINE,
    ELECTRIC_FURNACE, ELECTRIC_DISTILLERY, ELECTRIC_CENTRIFUGE, ELECTRIC_ELECTROLYZER,
    ELECTRIC_PACKER, ELECTRIC_UNPACKER, ELECTRIC_POLARIZER, ELECTRIC_WIREMILL,
];

/// The machine a block is (`modern_industrialization:electric_macerator`).
pub fn of_block(block: &str) -> Option<&'static Machine> {
    let path = block.strip_prefix("modern_industrialization:")?;
    ALL.iter().find(|m| m.id == path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_layouts_match_counts() {
        for m in ALL {
            assert_eq!(m.item_slots.len(), m.item_slots(), "{}", m.id);
            assert_eq!(m.fluid_slots.len(), m.fluid_slots(), "{}", m.id);
            // Every slot inside the panel and above the player inventory (y 84).
            for [x, y] in m.item_slots.iter().chain(m.fluid_slots) {
                assert!(*x >= 7 && *x + 17 <= 169 && *y + 17 <= 80, "{} slot at {x},{y}", m.id);
            }
        }
        assert_eq!(of_block("modern_industrialization:electric_macerator").unwrap().item_outputs, 4);
        assert_eq!(of_block("modern_industrialization:electric_centrifuge").unwrap().fluid_outputs, 4);
        assert_eq!(of_block("modern_industrialization:electric_packer").unwrap().background_height, 178);
        assert!(of_block("modern_industrialization:bronze_macerator").is_none());
        assert!(of_block("minecraft:furnace").is_none());
    }
}
