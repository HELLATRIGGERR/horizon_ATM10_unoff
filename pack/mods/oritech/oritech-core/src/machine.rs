//! The ported processing machines: each one's block, recipe type, slots (`InventorySlotAssignment`), screen slots
//! (`getGuiSlots`), machine cores (`getCorePositions`) and what it does after a craft. From `PulverizerBlockEntity`,
//! `FragmentForgeBlockEntity`, `AssemblerBlockEntity`, `FoundryBlockEntity`, `CentrifugeBlockEntity` (item recipes
//! only: no fluid addon), `PoweredFurnaceBlockEntity`, `BasicGeneratorEntity` and `BioGeneratorEntity`.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Pulverizer,
    FragmentForge,
    Assembler,
    Foundry,
    Centrifuge,
    PoweredFurnace,
    BasicGenerator,
    BioGenerator,
    AtomicForge,
    LaserArm,
    Refinery,
    RefineryModule,
    Cooler,
    FuelGenerator,
    LavaGenerator,
}

/// `InventorySlotAssignment(inputStart, inputCount, outputStart, outputCount)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slots {
    pub input_start: usize,
    pub input_count: usize,
    pub output_start: usize,
    pub output_count: usize,
}

impl Slots {
    pub fn is_input(&self, slot: usize) -> bool {
        slot >= self.input_start && slot < self.input_start + self.input_count
    }
    pub fn is_output(&self, slot: usize) -> bool {
        slot >= self.output_start && slot < self.output_start + self.output_count
    }
}

/// `ScreenProvider.GuiSlot`: a slot's place on the screen (where its item is drawn) and whether it is an output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuiSlot {
    pub index: usize,
    pub x: i32,
    pub y: i32,
    pub output: bool,
}

const fn slot(index: usize, x: i32, y: i32) -> GuiSlot {
    GuiSlot { index, x, y, output: false }
}

const fn out(index: usize, x: i32, y: i32) -> GuiSlot {
    GuiSlot { index, x, y, output: true }
}

#[derive(Debug)]
pub struct Spec {
    pub kind: Kind,
    /// The block (and the block entity type, named after it).
    pub block: &'static str,
    /// `getOwnRecipeType`.
    pub recipe_type: &'static str,
    pub slots: Slots,
    /// `getInventorySize`.
    pub inventory_size: usize,
    pub gui_slots: &'static [GuiSlot],
    /// `getCorePositions`: x back, y up, z left; empty for a machine that is no multiblock.
    pub cores: &'static [[i32; 3]],
    /// `getAddonSlots`.
    pub addon_slots: &'static [[i32; 3]],
    /// The craft is followed by `combineSmallDusts` (the pulverizer and the fragment forge).
    pub compacts_dust: bool,
    /// `inputOptionsEnabled`: the input mode button on the screen.
    pub input_options: bool,
    /// A generator (`UpgradableGeneratorBlockEntity`): it burns its input for energy instead of crafting.
    pub generator: bool,
    /// Where a generator pushes its energy (`getOutputTargets`): the block's offset and the face of that block the
    /// energy goes in by. Empty for a processing machine.
    pub outputs: &'static [([i32; 3], Face)],
}

/// A block face by name (the side of a neighbour that energy enters by).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Down,
    North,
    South,
    West,
    East,
}

impl Spec {
    pub fn is_multiblock(&self) -> bool {
        !self.cores.is_empty()
    }
}

/// The cores of the assembler and the foundry: beside, above and above beside the controller.
const THREE_CORES: &[[i32; 3]] = &[[0, 0, 1], [0, 1, 0], [0, 1, 1]];

pub static PULVERIZER: Spec = Spec {
    kind: Kind::Pulverizer,
    block: "oritech:pulverizer_block",
    recipe_type: "oritech:pulverizer",
    slots: Slots { input_start: 0, input_count: 1, output_start: 1, output_count: 2 },
    inventory_size: 3,
    gui_slots: &[slot(0, 56, 38), out(1, 117, 38), out(2, 135, 38)],
    cores: &[],
    addon_slots: &[[1, 0, 0]],
    compacts_dust: true,
    input_options: false,
    generator: false,
    outputs: &[],
};

pub static FRAGMENT_FORGE: Spec = Spec {
    kind: Kind::FragmentForge,
    block: "oritech:fragment_forge_block",
    recipe_type: "oritech:grinder",
    slots: Slots { input_start: 0, input_count: 1, output_start: 1, output_count: 3 },
    inventory_size: 4,
    gui_slots: &[slot(0, 56, 38), out(1, 117, 20), out(2, 117, 38), out(3, 117, 56)],
    cores: &[[0, 1, 0], [0, 0, 1], [0, 1, 1], [1, 0, 1], [1, 1, 1], [1, 0, 0], [1, 1, 0]],
    addon_slots: &[[0, 0, 2], [1, 0, 2], [2, 0, 0], [1, 0, -1], [-1, 0, 1]],
    compacts_dust: true,
    input_options: false,
    generator: false,
    outputs: &[],
};

pub static ASSEMBLER: Spec = Spec {
    kind: Kind::Assembler,
    block: "oritech:assembler_block",
    recipe_type: "oritech:assembler",
    slots: Slots { input_start: 0, input_count: 4, output_start: 4, output_count: 1 },
    inventory_size: 5,
    gui_slots: &[slot(0, 38, 26), slot(1, 56, 26), slot(2, 38, 44), slot(3, 56, 44), out(4, 117, 36)],
    cores: THREE_CORES,
    addon_slots: &[[0, 0, -1], [0, 0, 2], [1, 0, 0]],
    compacts_dust: false,
    input_options: true,
    generator: false,
    outputs: &[],
};

pub static FOUNDRY: Spec = Spec {
    kind: Kind::Foundry,
    block: "oritech:foundry_block",
    recipe_type: "oritech:foundry",
    slots: Slots { input_start: 0, input_count: 2, output_start: 2, output_count: 1 },
    inventory_size: 3,
    gui_slots: &[slot(0, 56, 26), slot(1, 56, 44), out(2, 117, 36)],
    cores: THREE_CORES,
    addon_slots: &[[0, 0, -1], [0, 0, 2]],
    compacts_dust: false,
    input_options: true,
    generator: false,
    outputs: &[],
};

/// `CentrifugeBlockEntity`: one core above. Its fluid mode (a fluid addon) is not ported.
pub static CENTRIFUGE: Spec = Spec {
    kind: Kind::Centrifuge,
    block: "oritech:centrifuge_block",
    recipe_type: "oritech:centrifuge",
    slots: Slots { input_start: 0, input_count: 1, output_start: 1, output_count: 2 },
    inventory_size: 3,
    gui_slots: &[slot(0, 56, 38), out(1, 113, 38), out(2, 113, 56)],
    cores: &[[0, 1, 0]],
    addon_slots: &[[0, 0, -1], [0, 0, 1]],
    compacts_dust: false,
    input_options: false,
    generator: false,
    outputs: &[],
};

/// `PoweredFurnaceBlockEntity`: the world's smelting recipes (a cooking time instead of the mod's `time`), one core above.
pub static POWERED_FURNACE: Spec = Spec {
    kind: Kind::PoweredFurnace,
    block: "oritech:powered_furnace_block",
    recipe_type: "minecraft:smelting",
    slots: Slots { input_start: 0, input_count: 1, output_start: 1, output_count: 1 },
    inventory_size: 2,
    gui_slots: &[slot(0, 56, 38), out(1, 117, 38)],
    cores: &[[0, 1, 0]],
    addon_slots: &[[0, -1, 0]],
    compacts_dust: false,
    input_options: false,
    generator: false,
    outputs: &[],
};

/// `BasicGeneratorEntity.getOutputTargets`: every side (the one below is given the face `DOWN`, as Java has it).
const AROUND: &[([i32; 3], Face)] = &[
    ([0, 1, 0], Face::Down),
    ([0, -1, 0], Face::Down),
    ([1, 0, 0], Face::West),
    ([0, 0, 1], Face::North),
    ([-1, 0, 0], Face::East),
    ([0, 0, -1], Face::South),
];

/// Burns any fuel (the furnace's burn times), no recipes.
pub static BASIC_GENERATOR: Spec = Spec {
    kind: Kind::BasicGenerator,
    block: "oritech:basic_generator_block",
    recipe_type: "",
    slots: Slots { input_start: 0, input_count: 1, output_start: 1, output_count: 0 },
    inventory_size: 1,
    gui_slots: &[slot(0, 80, 21)],
    cores: &[],
    addon_slots: &[],
    compacts_dust: false,
    input_options: false,
    generator: true,
    outputs: AROUND,
};

/// `BioGeneratorEntity.getOutputTargets`.
const BIO_AROUND: &[([i32; 3], Face)] = &[
    ([0, 2, 0], Face::Down),
    ([0, -1, 0], Face::Down),
    ([1, 0, 0], Face::West),
    ([1, 1, 0], Face::West),
    ([0, 0, 1], Face::North),
    ([0, 1, 1], Face::North),
    ([-1, 0, 0], Face::East),
    ([-1, 1, 0], Face::East),
    ([0, 0, -1], Face::South),
    ([0, 1, -1], Face::South),
];

pub static BIO_GENERATOR: Spec = Spec {
    kind: Kind::BioGenerator,
    block: "oritech:bio_generator_block",
    recipe_type: "oritech:bio_generator",
    slots: Slots { input_start: 0, input_count: 1, output_start: 1, output_count: 0 },
    inventory_size: 1,
    gui_slots: &[slot(0, 80, 21)],
    cores: &[[0, 1, 0]],
    addon_slots: &[[1, 0, 0], [1, 1, 0]],
    compacts_dust: false,
    input_options: false,
    generator: true,
    outputs: BIO_AROUND,
};

/// `AtomicForgeBlockEntity`: a ring of eight cores. The recipe's energy comes from the laser arm only (no insertion).
pub static ATOMIC_FORGE: Spec = Spec {
    kind: Kind::AtomicForge,
    block: "oritech:atomic_forge_block",
    recipe_type: "oritech:atomic_forge",
    slots: Slots { input_start: 0, input_count: 3, output_start: 3, output_count: 1 },
    inventory_size: 4,
    gui_slots: &[slot(0, 50, 36), slot(1, 74, 17), slot(2, 74, 55), out(3, 117, 36)],
    cores: &[[1, 0, 1], [1, 0, 0], [1, 0, -1], [0, 0, 1], [0, 0, -1], [-1, 0, 1], [-1, 0, 0], [-1, 0, -1]],
    addon_slots: &[],
    compacts_dust: false,
    input_options: false,
    generator: false,
    outputs: &[],
};

/// `LaserArmBlockEntity`: one core behind it (the controller's opposite facing), the laser head in front. Its three
/// slots are outputs only: what the laser mines. The recipe type is the laser's block-to-item recipes.
pub static LASER_ARM: Spec = Spec {
    kind: Kind::LaserArm,
    block: "oritech:laser_arm_block",
    recipe_type: "oritech:laser",
    slots: Slots { input_start: 0, input_count: 0, output_start: 0, output_count: 3 },
    inventory_size: 3,
    gui_slots: &[out(0, 117, 20), out(1, 117, 38), out(2, 117, 56)],
    cores: &[[1, 0, 0]],
    addon_slots: &[[-1, 0, 0]],
    compacts_dust: false,
    input_options: false,
    generator: false,
    outputs: &[],
};

/// `RefineryBlockEntity`: nine cores (a block high, two deep, behind and to the right), one fluid input slot and one
/// output slot for its catalyst items; the fluid tanks are in the entity.
pub static REFINERY: Spec = Spec {
    kind: Kind::Refinery,
    block: "oritech:refinery_block",
    recipe_type: "oritech:refinery",
    slots: Slots { input_start: 0, input_count: 1, output_start: 1, output_count: 1 },
    inventory_size: 2,
    gui_slots: &[slot(0, 62, 8), out(1, 62, 61)],
    cores: &[[0, 1, 0], [0, 0, -1], [0, 1, -1], [1, 0, -1], [1, 1, -1], [1, 0, 0], [1, 1, 0], [2, 0, -1], [2, 1, -1]],
    addon_slots: &[],
    compacts_dust: false,
    input_options: false,
    generator: false,
    outputs: &[],
};

/// `RefineryModuleBlockEntity`: a multiblock of five cores with no inventory and no energy; its tank is one of the
/// refinery's (the delegating storage).
pub static REFINERY_MODULE: Spec = Spec {
    kind: Kind::RefineryModule,
    block: "oritech:refinery_module_block",
    recipe_type: "",
    slots: Slots { input_start: 0, input_count: 0, output_start: 0, output_count: 0 },
    inventory_size: 0,
    gui_slots: &[],
    cores: &[[0, 0, -1], [1, 0, -1], [1, 0, 0], [2, 0, -1], [2, 0, 0]],
    addon_slots: &[],
    compacts_dust: false,
    input_options: false,
    generator: false,
    outputs: &[],
};

/// `CoolerBlockEntity`: one core beside it, no input slot and one output slot; water or lava from its tank.
pub static COOLER: Spec = Spec {
    kind: Kind::Cooler,
    block: "oritech:cooler_block",
    recipe_type: "oritech:cooler",
    slots: Slots { input_start: 0, input_count: 0, output_start: 0, output_count: 1 },
    inventory_size: 1,
    gui_slots: &[out(0, 117, 36)],
    cores: &[[0, 0, -1]],
    addon_slots: &[[0, 0, -2]],
    compacts_dust: false,
    input_options: false,
    generator: false,
    outputs: &[],
};

/// `FuelGeneratorEntity`: seventeen cores in a block three long, two high and three wide. Its two outputs depend on
/// its facing (`outputs` is empty: the glue lists them).
pub static FUEL_GENERATOR: Spec = Spec {
    kind: Kind::FuelGenerator,
    block: "oritech:fuel_generator_block",
    recipe_type: "oritech:fuel_generator",
    slots: Slots { input_start: 0, input_count: 1, output_start: 1, output_count: 1 },
    inventory_size: 2,
    gui_slots: &[slot(0, 55, 35), out(1, 112, 35)],
    cores: &[
        [0, 0, 1], [0, 0, -1], [-1, 0, 1], [-1, 0, 0], [-1, 0, -1], [-2, 0, 1], [-2, 0, 0], [-2, 0, -1],
        [0, 1, 1], [0, 1, 0], [0, 1, -1], [-1, 1, 1], [-1, 1, 0], [-1, 1, -1], [-2, 1, 1], [-2, 1, 0], [-2, 1, -1],
    ],
    addon_slots: &[[1, 0, 0], [-3, 0, 0]],
    compacts_dust: false,
    input_options: false,
    generator: true,
    outputs: &[],
};

/// `LavaGeneratorEntity.getOutputTargets`.
const LAVA_AROUND: &[([i32; 3], Face)] = &[
    ([0, 2, 0], Face::Down),
    ([0, -1, 0], Face::Down),
    ([1, 0, 0], Face::West),
    ([1, 1, 0], Face::West),
    ([0, 0, 1], Face::North),
    ([0, 1, 1], Face::North),
    ([-1, 0, 0], Face::East),
    ([-1, 1, 0], Face::East),
    ([0, 0, -1], Face::South),
    ([0, 1, -1], Face::South),
];

pub static LAVA_GENERATOR: Spec = Spec {
    kind: Kind::LavaGenerator,
    block: "oritech:lava_generator_block",
    recipe_type: "oritech:lava_generator",
    slots: Slots { input_start: 0, input_count: 1, output_start: 1, output_count: 1 },
    inventory_size: 2,
    gui_slots: &[slot(0, 55, 35), out(1, 112, 35)],
    cores: &[[0, 1, 0]],
    addon_slots: &[[1, 0, 0], [1, 1, 0]],
    compacts_dust: false,
    input_options: false,
    generator: true,
    outputs: LAVA_AROUND,
};

pub static ALL: [&Spec; 15] = [&PULVERIZER, &FRAGMENT_FORGE, &ASSEMBLER, &FOUNDRY, &CENTRIFUGE, &POWERED_FURNACE, &BASIC_GENERATOR, &BIO_GENERATOR, &ATOMIC_FORGE, &LASER_ARM, &REFINERY, &REFINERY_MODULE, &COOLER, &FUEL_GENERATOR, &LAVA_GENERATOR];

/// The machine of a block id.
pub fn of_block(block: &str) -> Option<&'static Spec> {
    ALL.iter().copied().find(|s| s.block == block)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_slots_fit_the_inventory() {
        for spec in ALL {
            let s = spec.slots;
            assert_eq!(s.input_start + s.input_count, s.output_start, "{:?}: outputs follow inputs", spec.kind);
            assert_eq!(s.output_start + s.output_count, spec.inventory_size, "{:?}", spec.kind);
            assert_eq!(spec.gui_slots.len(), spec.inventory_size, "{:?}: one screen slot each", spec.kind);
            for g in spec.gui_slots {
                assert_eq!(g.output, s.is_output(g.index), "{:?} slot {}", spec.kind, g.index);
            }
            assert!(spec.inventory_size <= 5, "every machine fits the hopper menu");
        }
        assert_eq!(of_block("oritech:fragment_forge_block").map(|s| s.recipe_type), Some("oritech:grinder"));
        assert!(of_block("oritech:atomic_forge_block").is_some_and(|s| s.cores.len() == 8 && s.slots.input_count == 3));
        assert!(of_block("oritech:laser_arm_block").is_some_and(|s| s.slots.output_count == 3 && s.cores == [[1, 0, 0]]));
        assert!(of_block("oritech:refinery_block").is_some_and(|s| s.cores.len() == 9));
        assert!(of_block("oritech:fuel_generator_block").is_some_and(|s| s.cores.len() == 17 && s.generator));
        assert_eq!(of_block("oritech:powered_furnace_block").map(|s| s.recipe_type), Some("minecraft:smelting"));
        assert_eq!(of_block("oritech:centrifuge_block").map(|s| s.cores.len()), Some(1));
        assert!(of_block("oritech:basic_generator_block").is_some_and(|s| s.generator && s.recipe_type.is_empty()));
        assert!(!PULVERIZER.is_multiblock() && FOUNDRY.is_multiblock() && FRAGMENT_FORGE.cores.len() == 7);
    }
}
