//! MEGA Cells 4.11.0's rows in the definitions registry (feature
//! `ae2-megacells`). Derived from MEGA Cells 4.11.0 (`MEGAItems`,
//! `MEGACells.initUpgrades`, `MEGACellsClient.initStorageCellModels`,
//! `MEGACraftingUnitType`, `MEGABlocks`, `MEGACraftingUnitModelProvider`),
//! https://github.com/62832/MEGACells tag 4.11.0, LGPL-3.0; see
//! `ae2/addons/megacells/`.
//!
//! Slice 1: the M-tier item and fluid storage cells and their components.
//! Slice 2: the MEGA crafting CPU blocks. Slice 3: the Superdense Energy Cell
//! and the Greater Energy Card. The portable M cells, the bulk cell and the
//! cells for other addons' key types come with later slices.
use super::defs::{CellDef, Defs, add_storage_cell_cards};
use crate::ae2_world::{CraftingUnit, EnergyCell, MachineKind};
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

/// `MEGABlocks.MEGA_ENERGY_CELL`: the Superdense Energy Cell,
/// `new EnergyCellBlock(12800000, 3200, 12800)` with AE2's
/// `EnergyCellBlockItem` and `EnergyCellBlockEntity`: 8 times the Dense
/// Energy Cell's capacity, twice its charge rate.
pub const MEGA_ENERGY_CELL: EnergyCell = EnergyCell {
    id: "megacells:mega_energy_cell",
    kind: MachineKind::DenseEnergyCell,
    max_power: 12_800_000.0,
    charge_rate: 3_200.0,
    priority: 12_800,
};
/// `MEGAItems.GREATER_ENERGY_CARD`: `new EnergyCardItem(p, 8)`.
pub const GREATER_ENERGY_CARD: &str = "megacells:greater_energy_card";

pub fn register(defs: &mut Defs) {
    for unit in CRAFTING_UNITS {
        defs.add_crafting_unit(unit);
    }
    defs.add_energy_cell(MEGA_ENERGY_CELL);
    defs.add_energy_card(GREATER_ENERGY_CARD, 8);
    // MEGACells.initUpgrades: the Greater Energy Card where AE2's energy card
    // goes on items, two at most, each card counted on its own
    // (`Upgrades.getMaxInstallable` per card; the tooltip groups are names).
    // AE2's portable cells (MEGA's portable M cells come with slice 4) ...
    let portable: Vec<String> = defs.cells().filter(|cell| cell.portable).map(|cell| cell.id.clone()).collect();
    for cell in portable {
        defs.add_upgrade(GREATER_ENERGY_CARD, &cell, 2);
    }
    // ... the wireless terminals, the colour applicator and the matter cannon.
    for host in ["ae2:wireless_terminal", "ae2:wireless_crafting_terminal", "ae2:color_applicator", "ae2:matter_cannon"] {
        defs.add_upgrade(GREATER_ENERGY_CARD, host, 2);
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

    /// `MEGABlocks.MEGA_ENERGY_CELL`: `EnergyCellBlock(12800000, 3200, 12800)`,
    /// a machine of AE2's energy cell kind; AE2's cells keep
    /// `EnergyCellBlock(200000, 800, 200)` and `(1600000, 1600, 1600)`.
    #[test]
    fn superdense_energy_cell_has_megas_capacity_rate_and_priority() {
        use crate::ae2_world::{Machine, base_id, energy_cell, is_machine_block};
        crate::ae2x::install();
        let machine = Machine::for_block("megacells:mega_energy_cell").unwrap();
        assert_eq!((machine.kind, machine.id(), machine.slots.len(), machine.upgrades.len()), (MachineKind::DenseEnergyCell, "megacells:mega_energy_cell", 0, 0));
        assert_eq!(base_id("megacells:mega_energy_cell"), "ae2:dense_energy_cell");
        assert!(is_machine_block("megacells:mega_energy_cell"));
        let cell = machine.energy_cell().unwrap();
        assert_eq!((cell.max_power, cell.charge_rate, cell.priority), (12_800_000.0, 3_200.0, 12_800));
        assert_eq!(machine.source_energy_capacity(), Some(12_800_000.0));
        // StoredEnergyAmount.setStored on load: over-capacity clamps to the cell's own maximum.
        let mut loaded = machine.clone();
        loaded.set_saved_energy(20_000_000.0);
        assert_eq!(loaded.energy, 12_800_000.0);
        loaded.set_saved_energy(9_000_000.0);
        assert_eq!(loaded.energy, 9_000_000.0);
        // getStorageLevelFromFillFactor: floor(4 × clamp(fill + 0.01)), so a
        // 1.6M charge that fills AE2's dense cell is an eighth of MEGA's.
        assert_eq!([0.0, 1_600_000.0, 3_200_000.0, 6_400_000.0, 12_700_000.0, 12_800_000.0].map(|e| cell.storage_level(e)), [0, 0, 1, 2, 4, 4]);
        for (id, max, rate, priority) in [("ae2:energy_cell", 200_000.0, 800.0, 200), ("ae2:dense_energy_cell", 1_600_000.0, 1_600.0, 1_600)] {
            let cell = Machine::for_block(id).unwrap().energy_cell().copied().unwrap();
            assert_eq!((cell.max_power, cell.charge_rate, cell.priority), (max, rate, priority), "{id}");
            assert_eq!(cell.storage_level(1_600_000.0), 4);
        }
        assert!(energy_cell("megacells:mega_crafting_unit").is_none());
        assert!(Machine::for_block("megacells:mega_crafting_unit").unwrap().energy_cell().is_none());
    }

    /// `EnergyCardItem(p, 8)` where `MEGACells.initUpgrades` puts it: two at
    /// most on AE2's portable cells, wireless terminals, colour applicator and
    /// matter cannon, each card counted against its own limit; nowhere else.
    #[test]
    fn greater_energy_card_fits_where_megas_upgrades_put_it() {
        let mut defs = Defs::ae2();
        register(&mut defs);
        assert_eq!(defs.energy_card_multiplier("ae2:energy_card"), 1);
        assert_eq!(defs.energy_card_multiplier(GREATER_ENERGY_CARD), 8);
        assert_eq!(defs.energy_card_multiplier("ae2:speed_card"), 0);
        let mut hosts = vec!["ae2:wireless_terminal", "ae2:wireless_crafting_terminal", "ae2:color_applicator", "ae2:matter_cannon"];
        let portable: Vec<String> = defs.cells().filter(|c| c.portable).map(|c| c.id.clone()).collect();
        assert_eq!(portable.len(), 10);
        hosts.extend(portable.iter().map(String::as_str));
        for host in hosts {
            assert_eq!(defs.upgrade_limit(GREATER_ENERGY_CARD, host), 2, "{host}");
            assert_eq!(defs.upgrade_limit("ae2:energy_card", host), 2, "{host}");
        }
        for host in ["ae2:item_storage_cell_1k", "megacells:item_storage_cell_1m", "ae2:view_cell", "ae2:vibration_chamber"] {
            assert_eq!(defs.upgrade_limit(GREATER_ENERGY_CARD, host), 0, "{host}");
        }
    }

    /// `Upgrades.getEnergyCardMultiplier` sums the cards' multipliers, which
    /// each host turns into its capacity (`setAEMaxPowerMultiplier`) and charge
    /// rate (`getChargeRate`).
    #[test]
    fn greater_energy_card_multiplies_item_batteries_by_eight() {
        use crate::{ae2_powered_items as power, ae2_upgrade_inventory::Inventory, items::ItemStack};
        crate::ae2x::install();
        let greater = ItemStack::new(GREATER_ENERGY_CARD, 1);
        let energy = ItemStack::new("ae2:energy_card", 1);
        let capacity = |stack: &ItemStack, base: f64| power::component_power_value(stack, "ae2:energy_capacity", "ae2:energy_capacity", base).unwrap();
        // Matter cannon, four slots: two of each card, 2 + 16 = 18;
        // 200,000 × (1 + 18 × 8) AE, charged at 800 × (1 + 18) AE/t.
        let mut cannon = ItemStack::new("ae2:matter_cannon", 1);
        let mut cards = Inventory::read(&cannon).unwrap();
        for card in [&greater, &greater, &energy, &energy] {
            assert!(cards.insert_one(card));
        }
        assert!(!cards.insert_one(&greater), "a third greater card");
        assert_eq!(cards.energy_card_multiplier(), 18);
        cards.store(&mut cannon).unwrap();
        assert_eq!(power::energy(&cannon).unwrap(), (0.0, 29_000_000.0));
        assert_eq!(power::charge_rate(&cannon).unwrap(), 15_200.0);
        // Colour applicator, two slots: two greater cards, 20,000 × 129 AE at 80 × 17 AE/t.
        let mut applicator = ItemStack::new("ae2:color_applicator", 1);
        let mut cards = Inventory::read(&applicator).unwrap();
        assert!(cards.insert_one(&greater) && cards.insert_one(&greater));
        assert!(!cards.insert_one(&energy), "both slots taken");
        cards.store(&mut applicator).unwrap();
        assert_eq!(power::energy(&applicator).unwrap(), (0.0, 2_580_000.0));
        assert_eq!(power::charge_rate(&applicator).unwrap(), 1_360.0);
        // Portable cell: one greater card and one of AE2's, 20,000 × (1 + 9 × 8).
        let mut portable = ItemStack::new("ae2:portable_item_cell_1k", 1);
        let mut cards = Inventory::read(&portable).unwrap();
        assert!(cards.insert_one(&greater) && cards.insert_one(&energy));
        cards.store(&mut portable).unwrap();
        assert_eq!(capacity(&portable, 20_000.0), 1_460_000.0);
        // Wireless terminal: 1 + getEnergyCardMultiplier, no × 8: 1.6M × 9.
        let mut terminal = ItemStack::new("ae2:wireless_terminal", 1);
        let mut cards = Inventory::read(&terminal).unwrap();
        assert!(cards.insert_one(&greater));
        cards.store(&mut terminal).unwrap();
        assert_eq!(capacity(&terminal, 1_600_000.0), 14_400_000.0);
        // A cell that is no portable one takes none.
        let mut storage = Inventory::read(&ItemStack::new("ae2:item_storage_cell_1k", 1)).unwrap();
        assert!(!storage.insert_one(&greater));
    }
}
