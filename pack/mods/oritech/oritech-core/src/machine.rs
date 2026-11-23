//! The ported processing machines: each one's block, recipe type, slots (`InventorySlotAssignment`), screen slots
//! (`getGuiSlots`), machine cores (`getCorePositions`) and what it does after a craft. From `PulverizerBlockEntity`,
//! `FragmentForgeBlockEntity`, `AssemblerBlockEntity` and `FoundryBlockEntity`.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Pulverizer,
    FragmentForge,
    Assembler,
    Foundry,
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
};

pub static ALL: [&Spec; 4] = [&PULVERIZER, &FRAGMENT_FORGE, &ASSEMBLER, &FOUNDRY];

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
        assert!(of_block("oritech:centrifuge_block").is_none());
        assert!(!PULVERIZER.is_multiblock() && FOUNDRY.is_multiblock() && FRAGMENT_FORGE.cores.len() == 7);
    }
}
