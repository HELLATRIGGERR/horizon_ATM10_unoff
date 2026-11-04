//! MEGA Cells 4.11.0's rows in the definitions registry (feature
//! `ae2-megacells`). Derived from MEGA Cells 4.11.0 (`MEGAItems`,
//! `MEGACells.initUpgrades`, `MEGACellsClient.initStorageCellModels`,
//! `MEGACraftingUnitType`, `MEGABlocks`, `MEGACraftingUnitModelProvider`,
//! `MEGAPortableCell`, `MEGAPatternProviderBlock`, `MEGAPatternProviderPart`,
//! `MEGAInterfaceBlockEntity`, `MEGAInterfacePart`),
//! https://github.com/62832/MEGACells tag 4.11.0, LGPL-3.0; see
//! `ae2/addons/megacells/`.
//!
//! Slice 1: the M-tier item and fluid storage cells and their components.
//! Slice 2: the MEGA crafting CPU blocks. Slice 3: the Superdense Energy Cell
//! and the Greater Energy Card. Slice 4: the portable M cells and the Portable
//! Cell Workbench. Slice 5: the MEGA Pattern Provider and the MEGA Interface,
//! blocks and parts. The bulk cell and the cells for other addons' key types
//! come with later slices.
use super::defs::{CellDef, Defs, add_storage_cell_cards};
use crate::ae2_world::{CraftingUnit, EnergyCell, MachineKind, MachineVariant};
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
/// `MEGAItems.PORTABLE_CELL_WORKBENCH`: `PortableCellWorkbenchItem`, a Cell
/// Workbench the player carries, with the cell it holds inside it.
pub const PORTABLE_CELL_WORKBENCH: &str = "megacells:portable_cell_workbench";

/// `MEGABlocks.MEGA_PATTERN_PROVIDER`: `MEGAPatternProviderBlock` with
/// `MEGAPatternProviderBlockEntity`, AE2's `PatternProviderBlockEntity` on
/// MEGA's logic; and `MEGAItems.MEGA_PATTERN_PROVIDER`, the part.
pub const MEGA_PATTERN_PROVIDER: &str = "megacells:mega_pattern_provider";
pub const MEGA_PATTERN_PROVIDER_PART: &str = "megacells:cable_mega_pattern_provider";
/// `MEGABlocks.MEGA_INTERFACE` (`MEGAInterfaceBlock`, `MEGAInterfaceBlockEntity`)
/// and `MEGAItems.MEGA_INTERFACE`, the part (`MEGAInterfacePart`).
pub const MEGA_INTERFACE: &str = "megacells:mega_interface";
pub const MEGA_INTERFACE_PART: &str = "megacells:cable_mega_interface";
/// `MEGAPatternProviderBlock.createLogic`: the pattern inventory's
/// `AEItemDefinitionFilter(AEItems.PROCESSING_PATTERN)`.
pub const PROCESSING_PATTERN: &str = "ae2:processing_pattern";

/// The MEGA devices that are AE2's with other sizes.
pub const MACHINES: [MachineVariant; 4] = [
    // MEGAPatternProviderBlock.createLogic: PatternProviderLogic(node, host, 18);
    // the return inventory stays AE2's nine slots.
    MachineVariant {
        id: MEGA_PATTERN_PROVIDER,
        base: "ae2:pattern_provider",
        block: MEGA_PATTERN_PROVIDER,
        kind: MachineKind::PatternProvider,
        slots: 18,
        upgrades: 0,
        part: false,
    },
    // MEGAPatternProviderPart.createLogic: the same logic on a cable.
    MachineVariant {
        id: MEGA_PATTERN_PROVIDER_PART,
        base: "ae2:cable_pattern_provider",
        block: MEGA_PATTERN_PROVIDER,
        kind: MachineKind::PatternProvider,
        slots: 18,
        upgrades: 0,
        part: true,
    },
    // MEGAInterfaceBlockEntity.createLogic: InterfaceLogic(node, this, item, 18),
    // 18 config and 18 storage slots and AE2's one card
    // (`UpgradeInventories.forMachine(is, 1, ..)`).
    MachineVariant {
        id: MEGA_INTERFACE,
        base: "ae2:interface",
        block: MEGA_INTERFACE,
        kind: MachineKind::Interface,
        slots: 18,
        upgrades: 1,
        part: false,
    },
    // MEGAInterfacePart.createLogic: the same on a cable.
    MachineVariant {
        id: MEGA_INTERFACE_PART,
        base: "ae2:cable_interface",
        block: MEGA_INTERFACE,
        kind: MachineKind::Interface,
        slots: 18,
        upgrades: 1,
        part: true,
    },
];

/// `AEItemDefinitionFilter.allowInsert` on the pattern inventory of provider
/// block or part `id` (`MEGAPatternProviderBlock.createLogic`, which both
/// MEGA's block entity and part call): the one item it takes. None for any
/// other provider, whose `AppEngInternalInventory` has no filter.
pub fn pattern_filter(id: &str) -> Option<&'static str> {
    matches!(id, MEGA_PATTERN_PROVIDER | MEGA_PATTERN_PROVIDER_PART).then_some(PROCESSING_PATTERN)
}

/// `MEGAPatternProviderBlockItem.addCheckedInformation` and
/// `MEGAPatternProviderPartItem.appendHoverText`: the line both items add
/// (`MEGATranslations.ProcessingOnly`), in `NOTICE`, 0xffde7d and italic.
pub fn hover_line(id: &str) -> Option<(&'static str, u32)> {
    matches!(id, MEGA_PATTERN_PROVIDER | MEGA_PATTERN_PROVIDER_PART).then_some(("gui.tooltips.megacells.ProcessingOnly", 0xffde7d))
}

/// `MEGAPortableCell(props, tier, keyType, menu, 0x80caff)`:
/// `PortableCellItem(keyType, 18 + (index - 5) × 9 types for items, 18 for
/// fluids, tier)`, so 27 to 63 item types from 1M to 256M; `getIdleDrain`
/// 1.0, `getAEMaxPower` 8 times and `getChargeRate` twice AE2's portable
/// cell's; the drive model is the tier's cell's (`initStorageCellModels`).
pub fn portable_cell(tier: Tier, channel: Channel) -> CellDef {
    let size = size(tier);
    let (kind, types) = match channel {
        Channel::Item => ("item", 18 + (tier.index() as usize - 5) * 9),
        Channel::Fluid => ("fluid", 18),
    };
    CellDef {
        id: format!("{NAMESPACE}:portable_{kind}_cell_{size}"),
        channel,
        tier,
        types,
        idle_drain: 1.0,
        portable: true,
        drive_model: format!("{NAMESPACE}:block/drive/cells/{size}_{kind}_cell"),
        battery_scale: 8.0,
        charge_scale: 2.0,
    }
}

pub fn register(defs: &mut Defs) {
    for unit in CRAFTING_UNITS {
        defs.add_crafting_unit(unit);
    }
    defs.add_energy_cell(MEGA_ENERGY_CELL);
    defs.add_energy_card(GREATER_ENERGY_CARD, 8);
    for machine in MACHINES {
        defs.add_variant(machine);
    }
    // MEGACells.initUpgrades: a crafting card and a fuzzy card on the MEGA
    // Interface, block and part, one of each, as on AE2's Interface.
    for card in ["ae2:crafting_card", "ae2:fuzzy_card"] {
        for host in [MEGA_INTERFACE, MEGA_INTERFACE_PART] {
            defs.add_upgrade(card, host, 1);
        }
    }
    // MEGACells.initUpgrades: the Greater Energy Card where AE2's energy card
    // goes on items, two at most, each card counted on its own
    // (`Upgrades.getMaxInstallable` per card; the tooltip groups are names).
    // AE2's ten portable cells (MEGA's own get theirs below) ...
    let portable: Vec<String> = defs.portable_cells().map(|cell| cell.id.clone()).collect();
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
                battery_scale: 1.0,
                charge_scale: 1.0,
            };
            // MEGACells.initUpgrades: inverter, equal distribution and void on
            // every item and fluid cell, fuzzy on item cells.
            add_storage_cell_cards(defs, &cell);
            defs.add_cell(cell);
        }
    }
    // MEGAItems.itemPortable / fluidPortable, after the cells (`CELLS` order).
    for channel in [Channel::Item, Channel::Fluid] {
        for tier in TIERS {
            let cell = portable_cell(tier, channel);
            // MEGACells.initUpgrades: the storage cell cards, and the Greater
            // Energy Card, two at most, on a portable one. AE2's energy card
            // is not registered on MEGA's portable cells.
            add_storage_cell_cards(defs, &cell);
            defs.add_upgrade(GREATER_ENERGY_CARD, &cell.id, 2);
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
        // AE2's 20, MEGA's 10 cells and 10 portable cells.
        assert_eq!(defs.cells().count(), 40);
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
        let portable: Vec<String> = defs.portable_cells().filter(|c| c.id.starts_with("ae2:")).map(|c| c.id.clone()).collect();
        assert_eq!(portable.len(), 10);
        hosts.extend(portable.iter().map(String::as_str));
        for host in hosts {
            assert_eq!(defs.upgrade_limit(GREATER_ENERGY_CARD, host), 2, "{host}");
            assert_eq!(defs.upgrade_limit("ae2:energy_card", host), 2, "{host}");
        }
        // MEGA's portable cells take its card alone.
        for host in ["megacells:portable_item_cell_1m", "megacells:portable_fluid_cell_256m"] {
            assert_eq!(defs.upgrade_limit(GREATER_ENERGY_CARD, host), 2, "{host}");
            assert_eq!(defs.upgrade_limit("ae2:energy_card", host), 0, "{host}");
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
    /// `MEGAPortableCell`: `PortableCellItem` with 18 + (index - 5) × 9 item
    /// types (27 at 1M to 63 at 256M) and 18 fluid types, half the tier's
    /// bytes (`getBytes` = `tier.bytes() / 2`, 524,288 at 1M), a 128th per
    /// type, 1.0 AE/t in a drive, 8 times the battery and twice the charge
    /// rate; the storage cell cards and two Greater Energy Cards
    /// (`initUpgrades`), AE2's energy card none.
    #[test]
    fn portable_m_cells_have_megas_types_bytes_drain_and_cards() {
        let mut defs = Defs::ae2();
        register(&mut defs);
        for (tier, size, item_types, bytes) in [
            (Tier::M1, "1m", 27, 524_288),
            (Tier::M4, "4m", 36, 2_097_152),
            (Tier::M16, "16m", 45, 8_388_608),
            (Tier::M64, "64m", 54, 33_554_432),
            (Tier::M256, "256m", 63, 134_217_728),
        ] {
            for (kind, channel, types, slots) in [("item", Channel::Item, item_types, 4), ("fluid", Channel::Fluid, 18, 3)] {
                let id = format!("megacells:portable_{kind}_cell_{size}");
                let cell = defs.portable_cell(&id).unwrap_or_else(|| panic!("{id}"));
                let config = cell.config();
                assert_eq!((config.channel, config.max_types(), config.total_bytes(), config.bytes_per_type()), (channel, types, bytes, bytes / 64), "{id}");
                assert!(config.portable);
                assert_eq!((cell.idle_drain, cell.battery_scale, cell.charge_scale), (1.0, 8.0, 2.0), "{id}");
                assert_eq!(cell.drive_model, format!("megacells:block/drive/cells/{size}_{kind}_cell"));
                assert_eq!(defs.upgrade_slots(&id), Some(slots));
                for card in ["ae2:inverter_card", "ae2:equal_distribution_card", "ae2:void_card"] {
                    assert_eq!(defs.upgrade_limit(card, &id), 1, "{card} on {id}");
                }
                assert_eq!(defs.upgrade_limit("ae2:fuzzy_card", &id), usize::from(channel == Channel::Item));
                assert_eq!((defs.upgrade_limit(GREATER_ENERGY_CARD, &id), defs.upgrade_limit("ae2:energy_card", &id)), (2, 0));
            }
        }
        // AE2's own keep theirs: 63 - 9 × index item types, 0.5 AE/t, AE2's battery.
        let ae2 = defs.portable_cell("ae2:portable_item_cell_256k").unwrap();
        assert_eq!((ae2.config().max_types(), ae2.idle_drain, ae2.battery_scale, ae2.charge_scale), (18, 0.5, 1.0, 1.0));
        assert!(defs.portable_cell("megacells:item_storage_cell_1m").is_none());
        // One type fills a 1M portable item cell at (524,288 - 8,192) × 8 items.
        let mut one = ae2_cell_core::Cell::new(defs.portable_cell("megacells:portable_item_cell_1m").unwrap().config());
        let key = ae2_cell_core::ItemKey { item_id: "minecraft:cobblestone".into(), components: Vec::new() };
        assert_eq!(one.insert(&key, u64::MAX, ae2_cell_core::Action::Modulate), 4_128_768);
        // The hotkeys' order: AE2's portable item cells, then MEGA's, by tier.
        let order: Vec<&str> = defs.portable_cells().filter(|c| c.channel == Channel::Item).map(|c| c.id.as_str()).collect();
        assert_eq!(order.first().copied(), Some("ae2:portable_item_cell_1k"));
        assert_eq!(&order[4..], ["ae2:portable_item_cell_256k", "megacells:portable_item_cell_1m", "megacells:portable_item_cell_4m",
            "megacells:portable_item_cell_16m", "megacells:portable_item_cell_64m", "megacells:portable_item_cell_256m"]);
    }

    /// `MEGAPatternProviderBlock.createLogic` (18 patterns, processing only),
    /// `MEGAInterfaceBlockEntity` and `MEGAInterfacePart.createLogic` (18 config
    /// and storage slots, AE2's one card) and `MEGACells.initUpgrades`.
    #[test]
    fn mega_provider_and_interface_are_ae2s_with_eighteen_slots() {
        use crate::ae2_world::{Machine, base_id, is_machine_block, variant};
        crate::ae2x::install();
        for (id, kind, slots, upgrades) in [(MEGA_PATTERN_PROVIDER, MachineKind::PatternProvider, 18, 0), (MEGA_INTERFACE, MachineKind::Interface, 18, 1)] {
            let machine = Machine::for_block(id).unwrap();
            assert_eq!((machine.kind, machine.slots.len(), machine.upgrades.len(), machine.id()), (kind, slots, upgrades, id));
            assert!(is_machine_block(id));
        }
        for (id, base, block) in [
            (MEGA_PATTERN_PROVIDER, "ae2:pattern_provider", MEGA_PATTERN_PROVIDER),
            (MEGA_PATTERN_PROVIDER_PART, "ae2:cable_pattern_provider", MEGA_PATTERN_PROVIDER),
            (MEGA_INTERFACE, "ae2:interface", MEGA_INTERFACE),
            (MEGA_INTERFACE_PART, "ae2:cable_interface", MEGA_INTERFACE),
        ] {
            assert_eq!((base_id(id), variant(id).unwrap().block, variant(id).unwrap().slots), (base, block, 18), "{id}");
        }
        // The parts are no blocks.
        assert!(Machine::for_block(MEGA_PATTERN_PROVIDER_PART).is_none() && !is_machine_block(MEGA_INTERFACE_PART));
        // AE2's own keep nine.
        assert_eq!(Machine::for_block("ae2:pattern_provider").unwrap().slots.len(), 9);
        assert_eq!(Machine::for_block("ae2:interface").unwrap().slots.len(), 9);

        let defs = super::super::defs::get();
        for host in [MEGA_INTERFACE, MEGA_INTERFACE_PART] {
            assert_eq!((defs.upgrade_limit("ae2:crafting_card", host), defs.upgrade_limit("ae2:fuzzy_card", host)), (1, 1), "{host}");
            for card in ["ae2:speed_card", "ae2:capacity_card", "ae2:inverter_card", "ae2:redstone_card", GREATER_ENERGY_CARD] {
                assert_eq!(defs.upgrade_limit(card, host), 0, "{card} on {host}");
            }
        }
        // The providers take cards of no kind.
        assert_eq!(defs.upgrade_limit("ae2:crafting_card", MEGA_PATTERN_PROVIDER), 0);

        // Only processing patterns, on MEGA's providers; AE2's take any encoded pattern.
        assert_eq!(pattern_filter(MEGA_PATTERN_PROVIDER), Some("ae2:processing_pattern"));
        assert_eq!(pattern_filter(MEGA_PATTERN_PROVIDER_PART), Some("ae2:processing_pattern"));
        for other in ["ae2:pattern_provider", "ae2:cable_pattern_provider", MEGA_INTERFACE, MEGA_INTERFACE_PART] {
            assert_eq!(pattern_filter(other), None, "{other}");
        }
        // The notice on both provider items, none on the interfaces.
        assert_eq!(hover_line(MEGA_PATTERN_PROVIDER_PART), Some(("gui.tooltips.megacells.ProcessingOnly", 0xffde7d)));
        assert_eq!(hover_line(MEGA_PATTERN_PROVIDER), hover_line(MEGA_PATTERN_PROVIDER_PART));
        assert_eq!(hover_line(MEGA_INTERFACE), None);
    }
}
