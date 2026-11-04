//! MEGA Cells 4.11.0's rows in the definitions registry (feature
//! `ae2-megacells`). Derived from MEGA Cells 4.11.0 (`MEGAItems`,
//! `MEGACells.initUpgrades`, `MEGACellsClient.initStorageCellModels`,
//! `MEGACraftingUnitType`, `MEGABlocks`, `MEGACraftingUnitModelProvider`),
//! https://github.com/62832/MEGACells tag 4.11.0, LGPL-3.0; see
//! `ae2/addons/megacells/`.
//!
//! Slice 1: the M-tier item and fluid storage cells and their components.
//! Slice 2: the MEGA crafting CPU blocks. The portable M cells, the bulk
//! cell and the cells for other addons' key types come with later slices.
use super::defs::{CellDef, Defs, add_storage_cell_cards};
use crate::ae2_world::{CraftingUnit, MachineKind};
use ae2_cell_core::{Channel, Tier};

pub const NAMESPACE: &str = "megacells";

/// `MEGAItems.TIER_1M` to `TIER_256M`: `tier(index, component)` with
/// 4^(index - 1) KiB, so 1M is index 6.
pub const TIERS: [Tier; 5] = [Tier::M1, Tier::M4, Tier::M16, Tier::M64, Tier::M256];

/// `StorageTier.namePrefix()`: "1m" to "256m".
pub fn size(tier: Tier) -> String {
    format!("{}m", tier.kilobytes() >> 10)
}

/// `MEGACraftingUnitType(storageMb, affix)` on the block `MEGABlocks` builds
/// with it: `getStorageBytes` is 1024 × 1024 × storageMb, and
/// `getAcceleratorThreads` 4 on the co-processor, 0 on the rest.
/// `MEGACraftingUnitModelProvider` draws the formed blocks from MEGA's own
/// `block/crafting/` textures.
const fn mega_unit(id: &'static str, kind: MachineKind, storage_mb: u64, affix: &'static str) -> CraftingUnit {
    CraftingUnit {
        id,
        kind,
        storage_bytes: 1024 * 1024 * storage_mb,
        accelerator_threads: if matches!(kind, MachineKind::CraftingAccelerator) { 4 } else { 0 },
        textures: NAMESPACE,
        affix,
    }
}

/// `MEGACraftingUnitType`'s values: the MEGA Crafting Unit, the MEGA Crafting
/// Co-Processing Unit, the 1M to 256M MEGA Crafting Storage and the MEGA
/// Crafting Monitor. Each behaves as AE2's block of the same kind
/// (`CraftingUnitBlock`, `CraftingMonitorBlock`, `CraftingBlockEntity`).
pub const CRAFTING_UNITS: [CraftingUnit; 8] = [
    mega_unit("megacells:mega_crafting_unit", MachineKind::CraftingUnit, 0, "unit"),
    mega_unit("megacells:mega_crafting_accelerator", MachineKind::CraftingAccelerator, 0, "accelerator"),
    mega_unit("megacells:1m_crafting_storage", MachineKind::CraftingStorage1k, 1, "1m_storage"),
    mega_unit("megacells:4m_crafting_storage", MachineKind::CraftingStorage4k, 4, "4m_storage"),
    mega_unit("megacells:16m_crafting_storage", MachineKind::CraftingStorage16k, 16, "16m_storage"),
    mega_unit("megacells:64m_crafting_storage", MachineKind::CraftingStorage64k, 64, "64m_storage"),
    mega_unit("megacells:256m_crafting_storage", MachineKind::CraftingStorage256k, 256, "256m_storage"),
    mega_unit("megacells:mega_crafting_monitor", MachineKind::CraftingMonitor, 0, "monitor"),
];

pub fn register(defs: &mut Defs) {
    for unit in CRAFTING_UNITS {
        defs.add_crafting_unit(unit);
    }
    for tier in TIERS {
        let size = size(tier);
        // MEGAItems.component: StorageComponentItem(mb * 1024 KiB).
        defs.add_component(&format!("{NAMESPACE}:cell_component_{size}"), tier);
        // MEGAItems.itemCell / fluidCell: BasicStorageCell(idleDrain 0.5 * index,
        // tier.bytes() / 1024 KiB, tier.bytes() / 128 per type, 63 or 18 types).
        for (kind, channel, types) in [("item", Channel::Item, 63), ("fluid", Channel::Fluid, 18)] {
            let cell = CellDef {
                id: format!("{NAMESPACE}:{kind}_storage_cell_{size}"),
                channel,
                tier,
                types,
                idle_drain: 0.5 * f64::from(tier.index()),
                portable: false,
                // MEGACellsClient.initStorageCellModels.
                drive_model: format!("{NAMESPACE}:block/drive/cells/{size}_{kind}_cell"),
            };
            // MEGACells.initUpgrades: inverter, equal distribution and void on
            // every item and fluid cell, fuzzy on item cells.
            add_storage_cell_cards(defs, &cell);
            defs.add_cell(cell);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ae2_cell_core::CellConfig;

    #[test]
    fn m_tier_cells_have_megas_layouts_cards_and_models() {
        let mut defs = Defs::ae2();
        register(&mut defs);
        assert_eq!(defs.cells().count(), 30);
        for (tier, size, drain) in [
            (Tier::M1, "1m", 3.0),
            (Tier::M4, "4m", 3.5),
            (Tier::M16, "16m", 4.0),
            (Tier::M64, "64m", 4.5),
            (Tier::M256, "256m", 5.0),
        ] {
            let item = defs.cell(&format!("megacells:item_storage_cell_{size}")).unwrap();
            assert_eq!(item.config(), CellConfig::item(tier));
            assert_eq!(item.idle_drain, drain);
            assert_eq!(item.drive_model, format!("megacells:block/drive/cells/{size}_item_cell"));
            let fluid = defs.cell(&format!("megacells:fluid_storage_cell_{size}")).unwrap();
            assert_eq!(fluid.config(), CellConfig::fluid(tier));
            assert_eq!(defs.upgrade_slots(&item.id), Some(4));
            assert_eq!(defs.upgrade_slots(&fluid.id), Some(3));
            assert_eq!(defs.upgrade_limit("ae2:fuzzy_card", &item.id), 1);
            assert_eq!(defs.upgrade_limit("ae2:fuzzy_card", &fluid.id), 0);
            assert_eq!(defs.upgrade_limit("ae2:void_card", &fluid.id), 1);
            assert_eq!(defs.upgrade_limit("ae2:energy_card", &item.id), 0);
            assert_eq!(defs.component(&format!("megacells:cell_component_{size}")), Some(tier));
        }
        assert_eq!(defs.cell("megacells:item_storage_cell_256m").unwrap().config().total_bytes(), 1 << 28);
    }

    /// `MEGACraftingUnitType`: 1,048,576 bytes a MiB of storage, 4 threads on
    /// the co-processor; AE2's `CraftingUnitType` keeps 1,024 a KiB and 1.
    #[test]
    fn mega_crafting_blocks_have_megas_bytes_and_threads() {
        use crate::ae2_world::{Machine, base_id, crafting_unit, is_machine_block};
        crate::ae2x::install();
        for (id, kind, bytes, threads, base) in [
            ("megacells:mega_crafting_unit", MachineKind::CraftingUnit, 0, 0, "ae2:crafting_unit"),
            ("megacells:mega_crafting_accelerator", MachineKind::CraftingAccelerator, 0, 4, "ae2:crafting_accelerator"),
            ("megacells:1m_crafting_storage", MachineKind::CraftingStorage1k, 1_048_576, 0, "ae2:1k_crafting_storage"),
            ("megacells:4m_crafting_storage", MachineKind::CraftingStorage4k, 4_194_304, 0, "ae2:4k_crafting_storage"),
            ("megacells:16m_crafting_storage", MachineKind::CraftingStorage16k, 16_777_216, 0, "ae2:16k_crafting_storage"),
            ("megacells:64m_crafting_storage", MachineKind::CraftingStorage64k, 67_108_864, 0, "ae2:64k_crafting_storage"),
            ("megacells:256m_crafting_storage", MachineKind::CraftingStorage256k, 268_435_456, 0, "ae2:256k_crafting_storage"),
            ("megacells:mega_crafting_monitor", MachineKind::CraftingMonitor, 0, 0, "ae2:crafting_monitor"),
        ] {
            let machine = Machine::for_block(id).unwrap();
            assert_eq!((machine.kind, machine.id(), machine.slots.len(), machine.upgrades.len()), (kind, id, 0, 0));
            let unit = machine.crafting_unit().unwrap();
            assert_eq!((unit.storage_bytes, unit.accelerator_threads), (bytes, threads), "{id}");
            assert_eq!(crafting_unit(id), Some(unit));
            assert_eq!(base_id(id), base);
            assert!(is_machine_block(id));
            assert!(MachineKind::from_id(id).is_none());
        }
        // The formed models draw MEGA's textures.
        let storage = crafting_unit("megacells:16m_crafting_storage").unwrap();
        assert_eq!(storage.texture(&format!("{}_light", storage.affix)), "megacells:block/crafting/16m_storage_light");
        // AE2's own blocks keep AE2's numbers.
        for (id, bytes, threads) in [("ae2:256k_crafting_storage", 262_144, 0), ("ae2:crafting_accelerator", 0, 1), ("ae2:crafting_unit", 0, 0)] {
            let unit = Machine::for_block(id).unwrap().crafting_unit().unwrap();
            assert_eq!((unit.storage_bytes, unit.accelerator_threads, unit.textures), (bytes, threads, "ae2"), "{id}");
        }
        assert!(Machine::for_block("ae2:drive").unwrap().crafting_unit().is_none());
    }
}
