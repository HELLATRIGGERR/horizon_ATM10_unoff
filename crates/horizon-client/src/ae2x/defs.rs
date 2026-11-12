//! The definitions registry: what AE2's items are to the port's storage code,
//! and what each addon built in (`ae2-<id>` features) adds to it.
//!
//! Shaped after AE2's public API (`appeng.api`, MIT): `IBasicCellItem` and
//! `StorageCells` for cells, `IStorageComponent` for storage components,
//! `Upgrades.add` and `IUpgradeableItem` for upgrade cards on items,
//! `StorageCellModels` for the model a drive draws. AE2's own rows come first,
//! then each addon's; the tables are built once, on first use, and do not
//! change after. An addon whose content is not loaded leaves rows that no item
//! reaches.
use crate::ae2_world::{AE2_CRAFTING_UNITS, AE2_ENERGY_CELLS, CraftingUnit, EnergyCell, ExtMachine, MachineVariant};
use ae2_cell_core::{CellConfig, KeyType, Tier};
use std::{
    collections::HashMap,
    sync::{
        LazyLock, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
};

/// A storage cell (`IBasicCellItem`).
#[derive(Clone, Debug, PartialEq)]
pub struct CellDef {
    pub id: String,
    /// `getKeyType`: items, fluids or an addon's type.
    pub key_type: KeyType,
    /// The tier: `getBytes` is its kilobytes × 1,024 (× 512 for a portable
    /// cell) and `getBytesPerType` its kilobytes × 8.
    pub tier: Tier,
    /// `getTotalTypes`.
    pub types: usize,
    /// `getIdleDrain`: AE/t while the cell sits in a drive or chest.
    pub idle_drain: f64,
    /// A `PortableCellItem`: half the bytes, a battery, a menu of its own.
    pub portable: bool,
    /// `StorageCellModels.registerModel`: the model drawn in a drive's slot.
    pub drive_model: String,
    /// A portable cell's `getAEMaxPower` over AE2's `PortableCellItem`'s (its
    /// `ae2:energy_capacity`, or the configured battery): 1 for AE2's cells,
    /// 8 for MEGA's (`MEGAPortableCell.getAEMaxPower`).
    pub battery_scale: f64,
    /// A portable cell's `getChargeRate` over AE2's (80 + 80 ×
    /// `getEnergyCardMultiplier`): 1 for AE2's, 2 for MEGA's.
    pub charge_scale: f64,
}
impl CellDef {
    /// The cell's storage layout, before its cards and partition.
    pub fn config(&self) -> CellConfig {
        if self.portable {
            CellConfig::portable(self.tier, self.key_type, self.types)
        } else {
            CellConfig::basic(self.tier, self.key_type, self.types)
        }
    }
}

/// A storage cell of an addon's own `ICellHandler` (`StorageCells.addCellHandler`),
/// not an `IBasicCellItem`: its inventory is the addon's (MEGA's Bulk Item Cell).
#[derive(Clone, Debug, PartialEq)]
pub struct HandlerCellDef {
    pub id: String,
    /// `StorageCell.getIdleDrain`, AE/t in a drive or chest.
    pub idle_drain: f64,
    /// `StorageCellModels.registerModel`.
    pub drive_model: String,
    /// `ICellWorkbenchItem.getConfigInventory(is).size()`: the filters it keeps.
    pub config_slots: usize,
}

/// A portable cell's battery (`AbstractPortableCell`): `getAEMaxPower` and
/// `getChargeRate` over AE2's `PortableCellItem`'s, as [`CellDef`] keeps them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortableBattery {
    pub battery_scale: f64,
    pub charge_scale: f64,
}

/// A part an addon puts on cables (`IPart` with its grid node).
#[derive(Clone, Debug, PartialEq)]
pub struct PartDef {
    pub id: String,
    /// `setIdlePowerUsage`, AE/t.
    pub idle_power: f64,
    /// `GridFlags.REQUIRE_CHANNEL`.
    pub requires_channel: bool,
    /// The storage cells it holds and mounts on the grid while online
    /// (`IChestOrDrive.getCellCount` of an `IStorageProvider` part: MEGA's
    /// Cell Dock); 0 for a part that holds none. Each cell adds its
    /// `getIdleDrain` to the part's idle power.
    pub cells: usize,
}

/// What a wireless terminal opens (`WTDefinition.menuType`): AE2's ME and
/// crafting terminal menus, the pattern encoding terminal's, the pattern
/// access terminal's, or another addon's menu by its id (ME Requester's
/// Requester Terminal).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TerminalKind {
    Me,
    Crafting,
    PatternEncoding,
    PatternAccess,
    Addon(&'static str),
}

/// A wireless terminal item: AE2's two, and AE2WTLib's `WTDefinition`
/// (its MIT API: `AddTerminalEvent.builder(terminalName, menuHostFactory,
/// menuType, item, icon)`, `hotkeyName`, `upgradeCount`, `componentType`).
#[derive(Clone, Debug, PartialEq)]
pub struct WirelessTerminal {
    /// `terminalName` (`crafting`, `pattern_encoding`, ...): what
    /// `ae2wtlib:current_terminal` names. AE2's own terminal, which no
    /// universal terminal takes, has its item id.
    pub name: String,
    pub item: String,
    pub kind: TerminalKind,
    /// `hotkeyName`: `wireless_<name>_terminal` unless the addon names one.
    pub hotkey: String,
    /// `upgradeCount`: the item's upgrade slots, 2 by default.
    pub upgrade_slots: usize,
    /// `componentType`, `has_<name>_terminal`: a universal terminal holds
    /// this terminal while it carries the component. None for one that
    /// can't be combined (AE2's Wireless Terminal).
    pub installed: Option<String>,
}

/// AE2WTLib's Wireless Universal Terminal (`ItemWUT`): one item that holds
/// the terminals whose components it carries and opens the current one.
#[derive(Clone, Debug, PartialEq)]
pub struct UniversalTerminal {
    pub item: String,
    /// `AE2wtlibComponents.CURRENT_TERMINAL`: the name of the terminal it opens.
    pub current: String,
}

/// An item the Wireless Access Point's booster slot takes beside AE2's
/// Wireless Booster, and the signal it gives a wireless terminal
/// (AEInfinityBooster's cards: its mixins on `WirelessAccessPointBlockEntity`,
/// `RestrictedInputSlot`, `WirelessTerminalMenuHost.getAccessPointSignal`
/// and `WirelessAccessPointMenu`).
#[derive(Clone, Debug, PartialEq)]
pub struct AccessPointCard {
    pub item: String,
    /// The squared distance `getAccessPointSignal` reports for an active
    /// access point holding the card, whatever the real distance: what the
    /// terminal's drain is reckoned from.
    pub signal: f64,
    /// Only for a player in the access point's dimension.
    pub same_dimension: bool,
    /// The drain the access point's menu shows, AE/t. The grid pays AE2's
    /// drain for one booster (`getBoosters` counts whatever is in the slot).
    pub shown_drain: f64,
    /// The access point keeps its chunk loaded while it holds the card.
    pub chunk_loading: bool,
}

/// The tables. Filled by [`Defs::ae2`] and the addons' `register`.
#[derive(Debug, Default)]
pub struct Defs {
    cells: HashMap<String, CellDef>,
    handler_cells: HashMap<String, HandlerCellDef>,
    /// The batteries of portable cells of an addon's own handler.
    portable_handler_cells: HashMap<String, PortableBattery>,
    components: HashMap<String, Tier>,
    upgrade_slots: HashMap<String, usize>,
    upgrades: HashMap<(String, String), usize>,
    machines: Vec<ExtMachine>,
    variants: Vec<MachineVariant>,
    crafting_units: Vec<CraftingUnit>,
    energy_cells: Vec<EnergyCell>,
    /// Upgrade inventories an addon gives other mods' machines, by id.
    machine_upgrade_slots: Vec<(&'static str, usize)>,
    energy_cards: HashMap<String, u32>,
    parts: HashMap<String, PartDef>,
    /// The portable cells in the order they were added.
    portable_order: Vec<String>,
    /// The wireless terminals in registration order (`wirelessTerminalList`).
    wireless_terminals: Vec<WirelessTerminal>,
    universal_terminal: Option<UniversalTerminal>,
    access_point_cards: Vec<AccessPointCard>,
    /// Addons' encoded processing pattern items and the component each keeps
    /// its sparse inputs and outputs in.
    processing_patterns: Vec<(String, String)>,
}
impl Defs {
    /// An addon's encoded processing pattern item
    /// (`PatternDetailsHelper.encodedPatternItemBuilder` with a decoder of its
    /// own whose details are processing ones, Advanced AE's
    /// `AdvProcessingPattern`), whose `component` holds `sparseInputs` and
    /// `sparseOutputs` as AE2's `EncodedProcessingPattern` does.
    pub fn add_processing_pattern(&mut self, item: &str, component: &str) {
        self.processing_patterns.retain(|(id, _)| id != item);
        self.processing_patterns.push((item.to_owned(), component.to_owned()));
    }
    /// The component of processing pattern item `item`: AE2's
    /// `ae2:encoded_processing_pattern` for its own, an addon's for its.
    pub fn processing_pattern_component(&self, item: &str) -> Option<&str> {
        if item == "ae2:processing_pattern" {
            return Some("ae2:encoded_processing_pattern");
        }
        self.processing_patterns.iter().find(|(id, _)| id == item).map(|(_, component)| component.as_str())
    }
    /// The addons' processing pattern items.
    pub fn processing_pattern_items(&self) -> impl Iterator<Item = &str> {
        self.processing_patterns.iter().map(|(id, _)| id.as_str())
    }
    /// An item the access point's booster slot takes, with the signal it gives.
    pub fn add_access_point_card(&mut self, card: AccessPointCard) {
        self.access_point_cards.retain(|c| c.item != card.item);
        self.access_point_cards.push(card);
    }
    /// The card row of `item`; none for AE2's booster and any other item.
    pub fn access_point_card(&self, item: &str) -> Option<&AccessPointCard> {
        self.access_point_cards.iter().find(|c| c.item == item)
    }
    /// What the access point's booster slot takes: AE2's Wireless Booster
    /// (`AEItemDefinitionFilter`), and the cards (`AccessPointCardFilter`).
    pub fn access_point_accepts(&self, item: &str) -> bool {
        item == "ae2:wireless_booster" || self.access_point_card(item).is_some()
    }
    /// A wireless terminal item. A row for an item already listed takes its
    /// place (AE2WTLib's `crafting` over AE2's Wireless Crafting Terminal:
    /// `AEItemsMixin` makes AE2's item its `ItemWCT`). The item's upgrade
    /// slots are its `upgradeCount`; its cards are added with [`Defs::add_upgrade`].
    pub fn add_wireless_terminal(&mut self, terminal: WirelessTerminal) {
        self.upgrade_slots.insert(terminal.item.clone(), terminal.upgrade_slots);
        match self.wireless_terminals.iter_mut().find(|t| t.item == terminal.item) {
            Some(row) => *row = terminal,
            None => self.wireless_terminals.push(terminal),
        }
    }
    /// The wireless terminals in registration order.
    pub fn wireless_terminals(&self) -> impl Iterator<Item = &WirelessTerminal> {
        self.wireless_terminals.iter()
    }
    /// The row of a wireless terminal item; none for the universal terminal
    /// and for any other item.
    pub fn wireless_terminal(&self, item: &str) -> Option<&WirelessTerminal> {
        self.wireless_terminals.iter().find(|t| t.item == item)
    }
    /// `WTDefinition.of(name)`.
    pub fn wireless_terminal_named(&self, name: &str) -> Option<&WirelessTerminal> {
        self.wireless_terminals.iter().find(|t| t.name == name)
    }
    /// AE2WTLib's `UpgradeHelper.addUpgradeToAllTerminals(card, max)`: `card`
    /// on each terminal a universal terminal can hold, up to `max` and its
    /// `upgradeCount` (all of them for 0), and on the universal terminal, up
    /// to `max` (for 0 the terminals' slots together,
    /// `WUTHandler.getUpgradeCardCount`).
    pub fn add_upgrade_to_all_terminals(&mut self, card: &str, max: usize) {
        let rows: Vec<(String, usize)> = self
            .wireless_terminals
            .iter()
            .filter(|t| t.installed.is_some())
            .map(|t| (t.item.clone(), t.upgrade_slots))
            .collect();
        let all: usize = rows.iter().map(|(_, slots)| slots).sum();
        for (item, slots) in rows.into_iter().filter(|(_, slots)| *slots > 0) {
            self.add_upgrade(card, &item, if max == 0 { slots } else { max.min(slots) });
        }
        if let Some(universal) = self.universal_terminal.clone() {
            self.add_upgrade(card, &universal.item, if max == 0 { all } else { max });
        }
    }
    /// The universal terminal: its item opens the terminals it holds.
    pub fn set_universal_terminal(&mut self, universal: UniversalTerminal) {
        self.universal_terminal = Some(universal);
    }
    pub fn universal_terminal(&self) -> Option<&UniversalTerminal> {
        self.universal_terminal.as_ref()
    }
    /// A wireless terminal item, the universal one included
    /// (`WirelessTerminalItem`).
    pub fn is_wireless_terminal(&self, item: &str) -> bool {
        self.wireless_terminal(item).is_some() || self.universal_terminal.as_ref().is_some_and(|u| u.item == item)
    }
    /// A storage cell, with the upgrade inventory `BasicStorageCell` and
    /// `PortableCellItem` give it: four slots on an item cell, three on a
    /// fluid cell. Its cards are added with [`Defs::add_upgrade`].
    pub fn add_cell(&mut self, cell: CellDef) {
        let slots = if cell.key_type == KeyType::ITEMS { 4 } else { 3 };
        self.upgrade_slots.insert(cell.id.clone(), slots);
        if cell.portable && !self.portable_order.contains(&cell.id) {
            self.portable_order.push(cell.id.clone());
        }
        self.cells.insert(cell.id.clone(), cell);
    }
    /// A cell of the addon's own handler, with `upgrade_slots` card slots
    /// (`ICellWorkbenchItem.getUpgrades`). Its cards are added with [`Defs::add_upgrade`].
    pub fn add_handler_cell(&mut self, cell: HandlerCellDef, upgrade_slots: usize) {
        self.upgrade_slots.insert(cell.id.clone(), upgrade_slots);
        self.handler_cells.insert(cell.id.clone(), cell);
    }
    /// A portable cell of the addon's own handler (an `AbstractPortableCell`
    /// whose inventory is the handler's: Applied Flux's portable FE cells),
    /// with `upgrade_slots` card slots and its battery.
    pub fn add_portable_handler_cell(&mut self, cell: HandlerCellDef, upgrade_slots: usize, battery: PortableBattery) {
        self.portable_handler_cells.insert(cell.id.clone(), battery);
        self.add_handler_cell(cell, upgrade_slots);
    }
    /// The battery of any portable cell, AE2's, a basic one of an addon or
    /// one of an addon's own handler (`AbstractPortableCell`): none for any
    /// other item.
    pub fn portable_battery(&self, id: &str) -> Option<PortableBattery> {
        self.portable_cell(id)
            .map(|cell| PortableBattery { battery_scale: cell.battery_scale, charge_scale: cell.charge_scale })
            .or_else(|| self.portable_handler_cells.get(id).copied())
    }
    /// A cell of an addon's own handler; none for AE2's and the basic cells.
    pub fn handler_cell(&self, id: &str) -> Option<&HandlerCellDef> {
        self.handler_cells.get(id)
    }
    /// `StorageCellModels.model(item)`: the model a drive draws for any cell.
    pub fn drive_model(&self, id: &str) -> Option<&str> {
        self.cells
            .get(id)
            .map(|cell| cell.drive_model.as_str())
            .or_else(|| self.handler_cells.get(id).map(|cell| cell.drive_model.as_str()))
    }
    /// A storage component (`IStorageComponent`) of `tier`: what a Condenser
    /// takes as its storage, `getBytes` × 8 power.
    pub fn add_component(&mut self, id: &str, tier: Tier) {
        self.components.insert(id.to_owned(), tier);
    }
    /// The size of an item's upgrade inventory (`IUpgradeableItem.getUpgrades`).
    pub fn set_upgrade_slots(&mut self, host: &str, slots: usize) {
        self.upgrade_slots.insert(host.to_owned(), slots);
    }
    /// An upgrade inventory of `slots` an addon gives machine `id`, block or
    /// part (`Machine::id`), in place of its own (Applied Flux's mixins on
    /// AE2's interface and pattern provider logic).
    pub fn set_machine_upgrade_slots(&mut self, id: &'static str, slots: usize) {
        self.machine_upgrade_slots.retain(|(machine, _)| *machine != id);
        self.machine_upgrade_slots.push((id, slots));
    }
    /// `Upgrades.add(card, host, max)`: `host` takes up to `max` of `card`.
    pub fn add_upgrade(&mut self, card: &str, host: &str, max: usize) {
        self.upgrades.insert((card.to_owned(), host.to_owned()), max);
    }

    /// An addon's machine block: `MachineKind::Ext` of its index.
    pub fn add_machine(&mut self, machine: ExtMachine) -> usize {
        self.machines.push(machine);
        self.machines.len() - 1
    }
    pub fn machines(&self) -> &[ExtMachine] {
        &self.machines
    }
    /// An addon's block or part that is one of AE2's machines with other
    /// sizes: AE2's kind, its own id (`ae2_world::variant`).
    pub fn add_variant(&mut self, variant: MachineVariant) {
        self.variants.push(variant);
    }
    pub fn variants(&self) -> &[MachineVariant] {
        &self.variants
    }
    /// An addon's crafting CPU block (`AbstractCraftingUnitBlock` on its own
    /// `ICraftingUnitType`): its row, and the machine variant of AE2's block
    /// of the same kind it behaves as.
    pub fn add_crafting_unit(&mut self, unit: CraftingUnit) {
        let base = AE2_CRAFTING_UNITS
            .iter()
            .find(|ae2| ae2.kind == unit.kind)
            .expect("a crafting unit has one of AE2's crafting kinds")
            .id;
        self.variants.push(MachineVariant {
            id: unit.id,
            base,
            block: unit.id,
            kind: unit.kind,
            slots: 0,
            upgrades: 0,
            part: false,
        });
        self.crafting_units.push(unit);
    }
    pub fn crafting_units(&self) -> &[CraftingUnit] {
        &self.crafting_units
    }
    /// An addon's energy cell (`EnergyCellBlock` with its own capacity, charge
    /// rate and priority): its row, and the machine variant of AE2's cell of
    /// the same kind it behaves as.
    pub fn add_energy_cell(&mut self, cell: EnergyCell) {
        let base = AE2_ENERGY_CELLS
            .iter()
            .find(|ae2| ae2.kind == cell.kind)
            .expect("an energy cell has one of AE2's energy cell kinds")
            .id;
        self.variants.push(MachineVariant {
            id: cell.id,
            base,
            block: cell.id,
            kind: cell.kind,
            slots: 0,
            upgrades: 0,
            part: false,
        });
        self.energy_cells.push(cell);
    }
    pub fn energy_cells(&self) -> &[EnergyCell] {
        &self.energy_cells
    }
    /// An `EnergyCardItem(properties, energyMultiplier)`: an upgrade card that
    /// adds `multiplier` to `Upgrades.getEnergyCardMultiplier` of the
    /// inventory it sits in. Where it fits is [`Defs::add_upgrade`]'s.
    pub fn add_energy_card(&mut self, card: &str, multiplier: u32) {
        assert!(multiplier > 0, "energyMultiplier must be > 0");
        self.energy_cards.insert(card.to_owned(), multiplier);
    }
    /// `EnergyCardItem.getEnergyMultiplier`: 0 for a card that is no energy card.
    pub fn energy_card_multiplier(&self, card: &str) -> u32 {
        self.energy_cards.get(card).copied().unwrap_or(0)
    }
    /// An addon's cable part.
    pub fn add_part(&mut self, part: PartDef) {
        self.parts.insert(part.id.clone(), part);
    }
    pub fn part(&self, id: &str) -> Option<&PartDef> {
        self.parts.get(id)
    }
    /// An addon's part that holds storage cells (`IChestOrDrive` with
    /// `IStorageProvider`): its row, and the part machine (`MachineKind::Ext`
    /// of a part row) that keeps its cells in its slots, its priority and
    /// settings in its config.
    pub fn add_cell_part(&mut self, id: &'static str, idle_power: f64, cells: usize) {
        assert!(cells > 0, "a cell part holds a cell");
        self.add_part(PartDef { id: id.to_owned(), idle_power, requires_channel: true, cells });
        self.add_machine(ExtMachine { id, physical_slots: cells, upgrade_slots: 0, key_drops: None, power_capacity: None, part: true });
    }
    /// A part that holds storage cells; none for any other part.
    pub fn cell_part(&self, id: &str) -> Option<&PartDef> {
        self.parts.get(id).filter(|part| part.cells > 0)
    }

    pub fn cell(&self, id: &str) -> Option<&CellDef> {
        self.cells.get(id)
    }
    pub fn cells(&self) -> impl Iterator<Item = &CellDef> {
        self.cells.values()
    }
    /// A `PortableCellItem` (AE2's `AbstractPortableCell` with a cell
    /// inventory): none for any other item.
    pub fn portable_cell(&self, id: &str) -> Option<&CellDef> {
        self.cells.get(id).filter(|cell| cell.portable)
    }
    /// The portable cells in registration order: AE2's, then each addon's
    /// (`HotkeyActions.registerPortableCell` in `HotkeyActions.init`, then the
    /// addons' common setup).
    pub fn portable_cells(&self) -> impl Iterator<Item = &CellDef> {
        self.portable_order.iter().filter_map(|id| self.cells.get(id))
    }
    pub fn component(&self, id: &str) -> Option<Tier> {
        self.components.get(id).copied()
    }
    pub fn upgrade_slots(&self, host: &str) -> Option<usize> {
        self.upgrade_slots.get(host).copied()
    }
    /// `Upgrades.getMaxInstallable`: how many of `card` fit in `host`.
    pub fn upgrade_limit(&self, card: &str, host: &str) -> usize {
        self.upgrades
            .get(&(card.to_owned(), host.to_owned()))
            .copied()
            .unwrap_or(0)
    }

    /// AE2 19.2.18's rows: `AEItems`, `InitStorageCells` and the item hosts of
    /// `InitUpgrades`. Machines and parts keep their own tables for now.
    pub fn ae2() -> Self {
        let mut defs = Self::default();
        let tiers = [Tier::K1, Tier::K4, Tier::K16, Tier::K64, Tier::K256];
        for tier in tiers {
            let size = format!("{}k", tier.kilobytes());
            // StorageTier: 0.5 AE/t per index; a portable cell drains 0.5 whatever its tier.
            let drain = 0.5 * f64::from(tier.index());
            for (kind, channel, types) in [("item", KeyType::ITEMS, 63), ("fluid", KeyType::FLUIDS, 18)] {
                let model = format!("ae2:block/drive/cells/{size}_{kind}_cell");
                defs.add_cell(CellDef {
                    id: format!("ae2:{kind}_storage_cell_{size}"),
                    key_type: channel,
                    tier,
                    types,
                    idle_drain: drain,
                    portable: false,
                    drive_model: model.clone(),
                    battery_scale: 1.0,
                    charge_scale: 1.0,
                });
                let portable_types = if channel == KeyType::ITEMS {
                    63 - 9 * tier.index() as usize
                } else {
                    18
                };
                defs.add_cell(CellDef {
                    id: format!("ae2:portable_{kind}_cell_{size}"),
                    key_type: channel,
                    tier,
                    types: portable_types,
                    idle_drain: 0.5,
                    portable: true,
                    drive_model: model,
                    battery_scale: 1.0,
                    charge_scale: 1.0,
                });
            }
            defs.add_component(&format!("ae2:cell_component_{size}"), tier);
        }
        // AEItems.ENERGY_CARD: EnergyCardItem(p, 1).
        defs.add_energy_card("ae2:energy_card", 1);
        // InitUpgrades, the item hosts.
        for cell in defs.cells.values().cloned().collect::<Vec<_>>() {
            add_storage_cell_cards(&mut defs, &cell);
            if cell.portable {
                defs.add_upgrade("ae2:energy_card", &cell.id, 2);
            }
        }
        // AEItems.WIRELESS_TERMINAL and WIRELESS_CRAFTING_TERMINAL
        // (WirelessTerminalItem: two upgrade slots, both on the
        // HotkeyAction.WIRELESS_TERMINAL hotkey).
        for (item, kind) in [("ae2:wireless_terminal", TerminalKind::Me), ("ae2:wireless_crafting_terminal", TerminalKind::Crafting)] {
            defs.add_wireless_terminal(WirelessTerminal {
                name: item.to_owned(),
                item: item.to_owned(),
                kind,
                hotkey: "wireless_terminal".to_owned(),
                upgrade_slots: 2,
                installed: None,
            });
            defs.add_upgrade("ae2:energy_card", item, 2);
        }
        defs.set_upgrade_slots("ae2:view_cell", 2);
        defs.add_upgrade("ae2:fuzzy_card", "ae2:view_cell", 1);
        defs.add_upgrade("ae2:inverter_card", "ae2:view_cell", 1);
        defs.set_upgrade_slots("ae2:color_applicator", 2);
        defs.add_upgrade("ae2:energy_card", "ae2:color_applicator", 2);
        defs.add_upgrade("ae2:equal_distribution_card", "ae2:color_applicator", 1);
        defs.add_upgrade("ae2:void_card", "ae2:color_applicator", 1);
        defs.set_upgrade_slots("ae2:matter_cannon", 4);
        defs.add_upgrade("ae2:energy_card", "ae2:matter_cannon", 2);
        defs.add_upgrade("ae2:fuzzy_card", "ae2:matter_cannon", 1);
        defs.add_upgrade("ae2:inverter_card", "ae2:matter_cannon", 1);
        defs.add_upgrade("ae2:void_card", "ae2:matter_cannon", 1);
        defs.add_upgrade("ae2:speed_card", "ae2:matter_cannon", 4);
        // CreativeCellItem has a config inventory but no upgrades.
        defs.set_upgrade_slots("ae2:creative_storage_cell", 0);
        defs
    }
}

/// The cards every item and fluid storage cell takes (`InitUpgrades`): one
/// each of inverter, equal distribution and void, and fuzzy on item cells.
pub fn add_storage_cell_cards(defs: &mut Defs, cell: &CellDef) {
    for card in ["ae2:inverter_card", "ae2:equal_distribution_card", "ae2:void_card"] {
        defs.add_upgrade(card, &cell.id, 1);
    }
    if cell.key_type == KeyType::ITEMS {
        defs.add_upgrade("ae2:fuzzy_card", &cell.id, 1);
    }
}

/// The rows of the addons built above the game (`horizon-ae2x-<id>` crates), by addon id.
static ADDON_ROWS: Mutex<Vec<(&'static str, fn(&mut Defs))>> = Mutex::new(Vec::new());
/// Whether [`get`] built the registry: rows installed after that would be left out.
static BUILT: AtomicBool = AtomicBool::new(false);

/// Installs the rows of an addon built above the game, for [`get`] to add in the addon's place
/// in its order. The program calls it before the registry is first built; a second install for the
/// same addon is ignored.
pub fn install_addon_rows(addon: &'static str, rows: fn(&mut Defs)) {
    assert!(!BUILT.load(Ordering::SeqCst), "AE2 addon {addon}: rows installed after the registry was built");
    let mut installed = ADDON_ROWS.lock().unwrap_or_else(PoisonError::into_inner);
    if !installed.iter().any(|(id, _)| *id == addon) {
        installed.push((addon, rows));
    }
}

/// The installed rows of `addon`, if any.
fn add_addon_rows(addon: &str, defs: &mut Defs) {
    let rows = ADDON_ROWS.lock().unwrap_or_else(PoisonError::into_inner).iter().find(|(id, _)| *id == addon).map(|&(_, rows)| rows);
    if let Some(rows) = rows {
        rows(defs);
    }
}

/// The registry: AE2's rows, then those of every addon built in.
pub fn get() -> &'static Defs {
    static DEFS: LazyLock<Defs> = LazyLock::new(|| {
        BUILT.store(true, Ordering::SeqCst);
        #[allow(unused_mut)]
        let mut defs = Defs::ae2();
        #[cfg(feature = "ae2-megacells")]
        super::megacells::register(&mut defs);
        #[cfg(feature = "ae2-merequester")]
        super::merequester::register(&mut defs);
        #[cfg(feature = "ae2-extendedae")]
        super::extendedae::register(&mut defs);
        #[cfg(feature = "ae2-ae2wtlib")]
        super::ae2wtlib::register(&mut defs);
        // The addons' terminals (`AddTerminalEvent.register`, ME Requester's
        // loading after AE2WTLib), then the cards on all of them
        // (`UpgradeHelper.addUpgrades`).
        #[cfg(all(feature = "ae2-merequester", feature = "ae2-ae2wtlib"))]
        super::merequester::register_ae2wt(&mut defs);
        #[cfg(all(feature = "ae2-extendedae", feature = "ae2-ae2wtlib"))]
        super::extendedae::register_ae2wt(&mut defs);
        #[cfg(feature = "ae2-ae2wtlib")]
        super::ae2wtlib::add_upgrades(&mut defs);
        #[cfg(all(feature = "ae2-megacells", feature = "ae2-ae2wtlib"))]
        super::megacells::register_ae2wt(&mut defs);
        // AEInfinityBooster's cards (horizon-ae2x-aeinfinitybooster).
        add_addon_rows("aeinfinitybooster", &mut defs);
        // AE2 Import Export Card's cards, on the terminals above.
        #[cfg(feature = "ae2-importexportcard")]
        super::importexportcard::register(&mut defs);
        // Applied Flux's key type and FE cells.
        #[cfg(feature = "ae2-appflux")]
        super::appflux::register(&mut defs);
        // Ender Drives' disks, cells of its own handler.
        #[cfg(feature = "ae2-enderdrives")]
        super::enderdrives::register(&mut defs);
        #[cfg(feature = "ae2-advancedae")]
        super::advancedae::register(&mut defs);
        // The addon machines' block ids, for `MachineKind::from_id`, the
        // variants of AE2's, for `Machine::for_block`, the crafting CPU
        // blocks' bytes and threads, and the energy cells' capacities.
        crate::ae2_world::install_ext_machines(defs.machines.clone());
        crate::ae2_world::install_machine_variants(defs.variants.clone());
        crate::ae2_world::install_crafting_units(defs.crafting_units.clone());
        crate::ae2_world::install_energy_cells(defs.energy_cells.clone());
        crate::ae2_world::install_machine_upgrade_slots(defs.machine_upgrade_slots.clone());
        defs
    });
    &DEFS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rows answer as the `match`es they replaced did.
    #[test]
    fn ae2_rows_keep_the_item_hosts_answers() {
        let defs = Defs::ae2();
        for size in ["1k", "4k", "16k", "64k", "256k"] {
            for (id, slots, fuzzy, energy) in [
                (format!("ae2:item_storage_cell_{size}"), 4, 1, 0),
                (format!("ae2:fluid_storage_cell_{size}"), 3, 0, 0),
                (format!("ae2:portable_item_cell_{size}"), 4, 1, 2),
                (format!("ae2:portable_fluid_cell_{size}"), 3, 0, 2),
            ] {
                assert_eq!(defs.upgrade_slots(&id), Some(slots), "{id}");
                assert_eq!(defs.upgrade_limit("ae2:fuzzy_card", &id), fuzzy, "{id}");
                assert_eq!(defs.upgrade_limit("ae2:energy_card", &id), energy, "{id}");
                for card in ["ae2:inverter_card", "ae2:equal_distribution_card", "ae2:void_card"] {
                    assert_eq!(defs.upgrade_limit(card, &id), 1, "{card} on {id}");
                }
                assert_eq!(defs.upgrade_limit("ae2:speed_card", &id), 0);
            }
            assert!(defs.component(&format!("ae2:cell_component_{size}")).is_some());
        }
        assert_eq!(defs.upgrade_slots("ae2:matter_cannon"), Some(4));
        assert_eq!(defs.upgrade_limit("ae2:speed_card", "ae2:matter_cannon"), 4);
        assert_eq!(defs.upgrade_limit("ae2:equal_distribution_card", "ae2:matter_cannon"), 0);
        assert_eq!(defs.upgrade_slots("ae2:color_applicator"), Some(2));
        assert_eq!(defs.upgrade_limit("ae2:fuzzy_card", "ae2:color_applicator"), 0);
        assert_eq!(defs.upgrade_slots("ae2:view_cell"), Some(2));
        assert_eq!(defs.upgrade_limit("ae2:inverter_card", "ae2:view_cell"), 1);
        assert_eq!(defs.upgrade_limit("ae2:energy_card", "ae2:wireless_crafting_terminal"), 2);
        assert_eq!(defs.upgrade_slots("ae2:creative_storage_cell"), Some(0));
        assert_eq!(defs.upgrade_slots("ae2:item_storage_cell_9k"), None);
        assert!(defs.cell("ae2:item_storage_cell_9k").is_none());
    }

    #[test]
    fn ae2_cells_have_their_source_layouts() {
        let defs = Defs::ae2();
        assert_eq!(defs.cells().count(), 20);
        let cell = defs.cell("ae2:item_storage_cell_256k").unwrap();
        assert_eq!(cell.config(), CellConfig::item(Tier::K256));
        assert_eq!(cell.idle_drain, 2.5);
        assert_eq!(cell.drive_model, "ae2:block/drive/cells/256k_item_cell");
        let cell = defs.cell("ae2:fluid_storage_cell_4k").unwrap();
        assert_eq!(cell.config(), CellConfig::fluid(Tier::K4));
        let cell = defs.cell("ae2:portable_item_cell_64k").unwrap();
        assert_eq!(cell.config(), CellConfig::portable_item(Tier::K64));
        assert_eq!((cell.idle_drain, cell.drive_model.as_str()), (0.5, "ae2:block/drive/cells/64k_item_cell"));
        assert_eq!(defs.cell("ae2:portable_fluid_cell_1k").unwrap().config(), CellConfig::portable_fluid(Tier::K1));
        assert_eq!(defs.component("ae2:cell_component_16k"), Some(Tier::K16));
    }
}
