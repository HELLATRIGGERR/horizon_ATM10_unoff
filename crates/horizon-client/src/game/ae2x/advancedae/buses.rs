//! Advanced AE's buses, each AE2's Export Bus (`ExportBusPart`) with work of
//! its own (`doBusWork`): the ME Stock Export Bus (`StockExportBusPart`)
//! exports each filter's key up to the amount it names in the block it faces
//! (`getCurrentStock`: what that block holds of it), the ME Import Export Bus
//! (`ImportExportBusPart`) exports as AE2's and imports what is not in its
//! filter, the ME Advanced IO Bus (`AdvancedIOBusPart`) is the stock bus at
//! eight times the operations that, with `REGULATE_STOCK` on, imports what goes
//! over each amount, then what is not in its filter. Their screens
//! (`StockExportBusScreen`, `ImportExportBusScreen`, `AdvancedIOBusScreen`) are
//! AE2's bus screen with the mod's styles, no fuzzy mode button, the amounts on
//! the filters and the Advanced IO Bus's `REGULATE_STOCK` toggle. Derived from
//! Advanced AE, https://github.com/pedroksl/AdvancedAE commit
//! `c322dad81e6f2ad94cb1fb861e73a7ba68e5907a`, LGPL-3.0, over AE2's
//! `ExportBusPart` and `IOBusPart` (LGPL-3.0); see `ae2/addons/advancedae/`.
//!
//! The import contexts are the mod's `FilteredImportStackTransferContext`,
//! whose `setInverted` keeps the opposite: with no inverter card (none of the
//! buses takes one) the import takes the keys its filter does not list, all
//! of them when the filter is empty (`isInverted` is false then).
use crate::game::{ae2_buses, ae2_part_screens::PartView};
use ae2_cell_core::AmountFormat;
use ae2_machines_core::{BusState, Direction, Key, Schedule, TickResult, TransferHost};
use horizon_client::{ae2x::advancedae as rows, items::ItemStack};
use horizon_world::nbt::{Compound, Tag};

/// Which of the mod's buses a part is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::game) enum Bus {
    StockExport,
    ImportExport,
    AdvancedIo,
}
impl Bus {
    pub(in crate::game) fn of(id: &str) -> Option<Self> {
        Some(match id {
            rows::STOCK_EXPORT_BUS => Self::StockExport,
            rows::IMPORT_EXPORT_BUS => Self::ImportExport,
            rows::ADVANCED_IO_BUS => Self::AdvancedIo,
            _ => return None,
        })
    }
    /// `StockExportBusPart.getConfig`: `ConfigInventory.configStacks(63)`, a
    /// key and an amount a filter; the Import Export Bus keeps AE2's keys.
    fn stocks(self) -> bool {
        self != Self::ImportExport
    }
}

/// The stock buses' amounts, one a filter (the mod's `extraConfig`), in the part's record.
pub(in crate::game) const STOCK: &str = "aae_stock";
/// `AAESettings.REGULATE_STOCK` as `IConfigManager` names it: `YES` (the default) or `NO`.
pub(in crate::game) const REGULATE: &str = "regulate_stock";
/// The Advanced IO Bus's `REGULATE_STOCK` button: its press on the bus menu
/// (AE2's buses have no setting at this place).
pub(in crate::game) const REGULATE_PLACE: usize = 48;
/// `AAEIcon.REGULATE_ON`, `REGULATE_OFF` in the mod's `guis/states.png`.
const REGULATE_ON: (i32, i32) = crate::game::ae2_screens::icon::sheet(crate::game::ae2_screens::icon::ADVANCED_AE, (32, 32));
const REGULATE_OFF: (i32, i32) = crate::game::ae2_screens::icon::sheet(crate::game::ae2_screens::icon::ADVANCED_AE, (48, 32));

fn part_id(part: &Compound) -> Option<String> {
    match part.get(&"id".into()) {
        Some(Tag::String(id)) => id.to_string().ok(),
        _ => None,
    }
}
/// The amount each filter keeps in stock (none set: 0).
pub(in crate::game) fn amounts(part: &Compound) -> Vec<i64> {
    let mut amounts = match part.get(&STOCK.into()) {
        Some(Tag::LongArray(values)) => values.clone(),
        _ => Vec::new(),
    };
    amounts.resize(63, 0);
    amounts
}
pub(in crate::game) fn set_amount(part: &mut Compound, slot: usize, amount: i64) {
    let mut all = amounts(part);
    if let Some(entry) = all.get_mut(slot) {
        *entry = amount.max(0);
    }
    part.insert(STOCK.into(), Tag::LongArray(all));
}
/// `REGULATE_STOCK`, on unless set to `NO`.
pub(in crate::game) fn regulates(part: &Compound) -> bool {
    part.get(&REGULATE.into()) != Some(&Tag::String("NO".into()))
}

/// A stack set in filter `slot` of a stock bus keeps its amount
/// (`ConfigMenuInventory` of stacks): an item's count, a fluid's bucket.
pub(in crate::game) fn filter_set(part: &mut Compound, slot: usize, key: Option<&Key>, held: Option<&ItemStack>) {
    if !part_id(part).and_then(|id| Bus::of(&id)).is_some_and(Bus::stocks) {
        return;
    }
    let amount = match (key, held) {
        (Some(key), _) if ae2_buses::bus_key_type(key) != ae2_cell_core::KeyType::ITEMS => 1000,
        (Some(_), Some(held)) => i64::from(held.count.max(1)),
        _ => 0,
    };
    set_amount(part, slot, amount);
}

/// A press on the bus menu's `place` that is the mod's own setting: the
/// Advanced IO Bus's `REGULATE_STOCK`.
pub(in crate::game) fn setting_press(part: &mut Compound, place: usize) -> bool {
    if place != REGULATE_PLACE || part_id(part).and_then(|id| Bus::of(&id)) != Some(Bus::AdvancedIo) {
        return false;
    }
    let next = if regulates(part) { "NO" } else { "YES" };
    part.insert(REGULATE.into(), Tag::String(next.into()));
    true
}

/// What the screen shows of the mod's bus beyond AE2's: no fuzzy mode
/// button, the amounts (`AmountFormat.SLOT` of each key's type), `REGULATE_STOCK`.
pub(in crate::game) fn part_view(part: &Compound, view: &mut PartView) {
    let Some(bus) = Bus::of(&view.id) else { return };
    view.fuzzy_card = false;
    if bus == Bus::AdvancedIo {
        view.regulate = Some(regulates(part));
    }
    if bus.stocks()
        && let Ok(state) = ae2_buses::bus_state(part)
    {
        let amounts = amounts(part);
        view.amounts = state
            .config
            .iter()
            .zip(&amounts)
            .take(crate::game::ae2_part_screens::CONFIG_PLACES)
            .map(|(key, amount)| key.as_ref().map_or_else(String::new, |key| ae2_buses::bus_key_type(key).format_amount(*amount as u64, AmountFormat::Slot)))
            .collect();
    }
}
/// The mod's toolbar buttons after AE2's: the place each presses, its icon
/// and tooltip keys (`AAESettingToggleButton`).
pub(in crate::game) fn part_tools(view: &PartView) -> Vec<(usize, (i32, i32), &'static str, &'static str)> {
    match view.regulate {
        Some(true) => vec![(REGULATE_PLACE, REGULATE_ON, "gui.tooltips.advanced_ae.RegulateCategory", "gui.tooltips.advanced_ae.RegulateOn")],
        Some(false) => vec![(REGULATE_PLACE, REGULATE_OFF, "gui.tooltips.advanced_ae.RegulateCategory", "gui.tooltips.advanced_ae.RegulateOff")],
        None => Vec::new(),
    }
}

/// `IOBusPart.tickingRequest` of the mod's bus `part`: none for any other
/// part (AE2's work, `BusState::tick`).
pub(in crate::game) fn tick<H: TransferHost>(part: &Compound, state: &mut BusState, active: bool, loaded: bool, host: &mut H) -> Option<Result<TickResult, H::Error>> {
    let bus = Bus::of(&part_id(part)?)?;
    Some(work(bus, part, state, active, loaded, host))
}

fn work<H: TransferHost>(bus: Bus, part: &Compound, state: &mut BusState, active: bool, loaded: bool, host: &mut H) -> Result<TickResult, H::Error> {
    // isSleeping (the redstone card's mode), then canDoBusWork.
    if state.sleeping() {
        return Ok(TickResult::Sleep);
    }
    if !active || !loaded {
        return Ok(TickResult::Idle);
    }
    let amounts = amounts(part);
    let mut done = match bus {
        // ExportBusPart.doBusWork.
        Bus::ImportExport => state.tick(Direction::Export, active, loaded, host)? == TickResult::Faster,
        Bus::StockExport | Bus::AdvancedIo => stock_export(state, &amounts, host)?,
    };
    state.pending_pulse = false;
    let ops = state.operations();
    let filter: Vec<Key> = state.config.iter().flatten().cloned().collect();
    let fuzzy = state.fuzzy;
    let unlisted = |host: &H, key: &Key| filter.is_empty() || !listed(host, fuzzy, &filter, key);
    match bus {
        Bus::StockExport => {}
        Bus::ImportExport => done |= import(host, ops, &unlisted)? < ops,
        Bus::AdvancedIo => {
            let mut left = ops;
            if regulates(part) {
                let slots = state.slots();
                let mut x = 0;
                while x < slots && left > 0 {
                    let index = starting_slot(state, x, slots, host);
                    x += 1;
                    let Some(key) = state.config.get(index).cloned().flatten() else { continue };
                    let amount = i64::from(amounts[index] as i32);
                    let stock = i64::from(current_stock(host, &key) as i32);
                    if stock <= amount {
                        continue;
                    }
                    let factor = host.amount_per_operation(&key).unwrap_or(1).max(1);
                    let most = (left.saturating_mul(factor) as i64).min(stock - amount);
                    let op = most as u64 / factor;
                    let one = [key];
                    let remaining = import(host, op, &|host: &H, candidate: &Key| listed(host, fuzzy, &one, candidate))?;
                    left -= op - remaining;
                    done |= remaining < op;
                }
            }
            if left > 0 {
                done |= import(host, ops, &unlisted)? < ops;
            }
        }
    }
    Ok(if state.sleeping() {
        TickResult::Sleep
    } else if done {
        TickResult::Faster
    } else {
        TickResult::Slower
    })
}

/// `StockExportBusPart.doBusWork`: each filter's key up to its amount less
/// what the target holds, at most the operations left; the crafting card's
/// request when nothing went. True when it did work (`hasDoneWork`).
fn stock_export<H: TransferHost>(state: &mut BusState, amounts: &[i64], host: &mut H) -> Result<bool, H::Error> {
    let initial = state.operations();
    let mut ops = initial;
    let slots = state.slots();
    let mut x = 0;
    while x < slots && ops > 0 {
        let index = starting_slot(state, x, slots, host);
        x += 1;
        let Some(key) = state.config.get(index).cloned().flatten() else { continue };
        // `(int) stack.amount()`.
        let amount = i64::from(amounts.get(index).copied().unwrap_or(0) as i32);
        let factor = host.amount_per_operation(&key).unwrap_or(1).max(1);
        if host.craft_only() {
            attempt_crafting(host, &mut ops, index, &key, amount, factor)?;
            continue;
        }
        let before = ops;
        let most = (ops.saturating_mul(factor) as i64).min(amount - current_stock(host, &key) as i64);
        if most > 0 {
            let moved = host.transfer_atomic(Direction::Export, &key, most as u64)?;
            if moved > 0 {
                ops = ops.saturating_sub((moved / factor).max(1));
            }
        }
        // isCraftingEnabled: the host requests nothing without the card.
        if before == ops {
            attempt_crafting(host, &mut ops, index, &key, amount, factor)?;
        }
    }
    // Round-robin advances only when something was exported.
    if ops < initial && state.schedule == Schedule::RoundRobin && slots > 0 {
        state.next_slot = (state.next_slot + x) % slots;
    }
    Ok(ops < initial)
}
/// `StockExportBusPart.attemptCrafting`: a request of what the target is short.
fn attempt_crafting<H: TransferHost>(host: &mut H, ops: &mut u64, index: usize, key: &Key, amount: i64, factor: u64) -> Result<(), H::Error> {
    let most = (ops.saturating_mul(factor) as i64).min(amount - current_stock(host, key) as i64);
    if most <= 0 {
        return Ok(());
    }
    let requested = host.request_craft(index, key, most as u64)?;
    if requested > 0 {
        *ops = ops.saturating_sub((requested / factor).max(1));
    }
    Ok(())
}
/// `ExportBusPart.getStartingSlot`.
fn starting_slot<H: TransferHost>(state: &BusState, x: usize, slots: usize, host: &mut H) -> usize {
    match state.schedule {
        Schedule::Default => x,
        Schedule::RoundRobin => (state.next_slot + x) % slots,
        Schedule::Random => host.random_slot(slots) % slots,
    }
}
/// `getCurrentStock`: what the block the bus faces holds of `key` (its item
/// and fluid handlers' available stacks).
fn current_stock<H: TransferHost>(host: &H, key: &Key) -> u64 {
    host.adjacent_slots().iter().filter(|(found, _)| found == key).map(|(_, n)| *n).sum()
}
/// `IPartitionList.isListed`, fuzzy with the fuzzy card.
fn listed<H: TransferHost>(host: &H, fuzzy: bool, filter: &[Key], key: &Key) -> bool {
    filter.iter().any(|f| if fuzzy { host.fuzzy_matches(f, key) } else { f == key })
}
/// AE2's import strategies over the target's stacks, `ops` operations in a
/// budget shared by the key types (as `BusState::tick` imports), of the keys
/// `allow` takes; the operations left.
fn import<H: TransferHost>(host: &mut H, ops: u64, allow: &dyn Fn(&H, &Key) -> bool) -> Result<u64, H::Error> {
    let mut ops = ops;
    let mut group = None;
    let mut remaining = 0;
    for (key, _) in host.adjacent_slots() {
        let factor = host.amount_per_operation(&key).unwrap_or(1).max(1);
        if group != Some(factor) {
            group = Some(factor);
            remaining = ops.saturating_mul(factor);
        }
        if remaining == 0 || !allow(host, &key) {
            continue;
        }
        let moved = host.transfer_atomic(Direction::Import, &key, remaining)?.min(remaining);
        if moved > 0 {
            ops = ops.saturating_sub((moved / factor).max(1));
            remaining -= moved;
        }
    }
    Ok(ops)
}

impl crate::game::Game<'_> {
    /// `/ae2 advae bus|stock`: the mod's bus on `face` of the cable at
    /// `pos`, its amounts and `REGULATE_STOCK`; `stock` sets a filter's amount
    /// (what the mod's `SetAmountMenu` does).
    pub(in crate::game) fn advancedae_bus_command(&mut self, pos: (i32, i32, i32), face: &str, set: bool, rest: &[&str]) -> Result<serde_json::Value, String> {
        let Some(horizon_client::containers::BlockEntity::Ae2(machine)) = self.world.block_entities.get_mut(&pos) else { return Err("cable missing".into()) };
        let Some(Tag::List(parts)) = machine.config.get_mut(&"parts".into()) else { return Err("no cable parts".into()) };
        let Some(Tag::Compound(part)) = parts.iter_mut().find(|p| p.get("face") == Some(&Tag::String(face.into()))) else { return Err("no part on that face".into()) };
        let bus = part_id(part).and_then(|id| Bus::of(&id)).ok_or("not an Advanced AE bus")?;
        match (set, rest) {
            (false, []) => {}
            (true, [slot, amount]) if bus.stocks() => {
                let slot: usize = slot.parse().map_err(|_| "bad filter slot")?;
                let amount: i64 = amount.parse().map_err(|_| "bad amount")?;
                if slot >= 63 {
                    return Err("bad filter slot".into());
                }
                set_amount(part, slot, amount);
                part.insert("bus_next_tick".into(), Tag::Long(0));
            }
            (true, [_, _]) => return Err("its filters keep no amounts".into()),
            _ => return Err("usage: /ae2 advae bus <x> <y> <z> <face> | stock <x> <y> <z> <face> <filter> <amount>".into()),
        }
        let value = serde_json::json!({"amounts": amounts(part), "regulate": regulates(part), "stocks": bus.stocks()});
        if set {
            self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// A chest (the target) and a grid of item keys, one item an operation.
    #[derive(Default)]
    struct Fake {
        target: BTreeMap<Key, u64>,
        grid: BTreeMap<Key, u64>,
    }
    impl TransferHost for Fake {
        type Error = String;
        fn adjacent_slots(&self) -> Vec<(Key, u64)> {
            self.target.iter().map(|(k, n)| (k.clone(), *n)).collect()
        }
        fn amount_per_operation(&self, _key: &Key) -> Option<u64> {
            Some(1)
        }
        fn fuzzy_matches(&self, filter: &Key, candidate: &Key) -> bool {
            filter.id == candidate.id
        }
        fn fuzzy_candidates(&self, filter: &Key) -> Vec<Key> {
            vec![filter.clone()]
        }
        fn transfer_atomic(&mut self, direction: Direction, key: &Key, limit: u64) -> Result<u64, String> {
            let (from, to) = match direction {
                Direction::Export => (&mut self.grid, &mut self.target),
                Direction::Import => (&mut self.target, &mut self.grid),
            };
            let n = from.get(key).copied().unwrap_or(0).min(limit);
            if n > 0 {
                *from.get_mut(key).unwrap() -= n;
                from.retain(|_, v| *v > 0);
                *to.entry(key.clone()).or_default() += n;
            }
            Ok(n)
        }
    }
    fn key(id: &str) -> Key {
        Key { id: id.into(), components: Vec::new() }
    }
    fn bus_part(id: &str, filters: &[(&str, i64)], speed: u8) -> (Compound, BusState) {
        let mut part = Compound::new();
        part.insert("id".into(), Tag::String(id.into()));
        let mut state = BusState::new(ae2_cell_core::KeyType::ITEMS);
        state.speed_cards = speed;
        state.multiplier = rows::bus_operations_multiplier(id);
        for (i, (item, amount)) in filters.iter().enumerate() {
            state.config[i] = Some(key(item));
            set_amount(&mut part, i, *amount);
        }
        (part, state)
    }

    #[test]
    fn the_stock_bus_exports_up_to_each_amount_less_what_the_target_holds() {
        let (part, mut state) = bus_part(rows::STOCK_EXPORT_BUS, &[("minecraft:cobblestone", 100), ("minecraft:sand", 5)], 4);
        let mut host = Fake::default();
        host.grid.insert(key("minecraft:cobblestone"), 1000);
        host.grid.insert(key("minecraft:sand"), 1000);
        host.target.insert(key("minecraft:cobblestone"), 30);
        assert_eq!(tick(&part, &mut state, true, true, &mut host).unwrap(), Ok(TickResult::Faster));
        assert_eq!(host.target[&key("minecraft:cobblestone")], 100, "70 more of 96 operations");
        assert_eq!(host.target[&key("minecraft:sand")], 5, "and 5 of the 26 left");
        assert_eq!(tick(&part, &mut state, true, true, &mut host).unwrap(), Ok(TickResult::Slower), "stocked: no work");
        assert_eq!(host.grid[&key("minecraft:cobblestone")], 930);
    }

    #[test]
    fn the_import_export_bus_exports_its_filter_and_imports_the_rest() {
        let (part, mut state) = bus_part(rows::IMPORT_EXPORT_BUS, &[("minecraft:cobblestone", 0)], 0);
        let mut host = Fake::default();
        host.grid.insert(key("minecraft:cobblestone"), 10);
        host.target.insert(key("minecraft:dirt"), 3);
        tick(&part, &mut state, true, true, &mut host).unwrap().unwrap();
        assert_eq!(host.target.get(&key("minecraft:cobblestone")), Some(&1), "AE2's export: one operation");
        assert_eq!(host.target.get(&key("minecraft:dirt")), Some(&2), "one operation of what the filter does not list");
        tick(&part, &mut state, true, true, &mut host).unwrap().unwrap();
        assert_eq!(host.target.get(&key("minecraft:cobblestone")), Some(&2), "its own key stays");
        // An empty filter imports everything.
        let (part, mut state) = bus_part(rows::IMPORT_EXPORT_BUS, &[], 0);
        tick(&part, &mut state, true, true, &mut host).unwrap().unwrap();
        assert_eq!(host.target.get(&key("minecraft:cobblestone")), Some(&1));
    }

    #[test]
    fn the_advanced_io_bus_regulates_the_excess_then_imports_the_rest() {
        let (mut part, mut state) = bus_part(rows::ADVANCED_IO_BUS, &[("minecraft:cobblestone", 10)], 0);
        assert_eq!(state.operations(), 8, "AE2's one operation times 8");
        let mut host = Fake::default();
        host.target.insert(key("minecraft:cobblestone"), 25);
        host.target.insert(key("minecraft:dirt"), 20);
        tick(&part, &mut state, true, true, &mut host).unwrap().unwrap();
        assert_eq!(host.target[&key("minecraft:cobblestone")], 17, "8 of the 15 over: all its operations");
        assert_eq!(host.target[&key("minecraft:dirt")], 20, "none left for the rest");
        tick(&part, &mut state, true, true, &mut host).unwrap().unwrap();
        assert_eq!(host.target[&key("minecraft:cobblestone")], 10, "the rest of the excess");
        assert_eq!(host.target[&key("minecraft:dirt")], 12, "an operation left: the unlisted import, of all eight");
        assert!(setting_press(&mut part, REGULATE_PLACE) && !regulates(&part));
        host.target.insert(key("minecraft:cobblestone"), 30);
        tick(&part, &mut state, true, true, &mut host).unwrap().unwrap();
        assert_eq!(host.target[&key("minecraft:cobblestone")], 30, "no regulation: its own key stays");
        assert_eq!(host.target[&key("minecraft:dirt")], 4);
        // Below the amount it exports.
        host.grid.insert(key("minecraft:cobblestone"), 100);
        host.target.insert(key("minecraft:cobblestone"), 4);
        tick(&part, &mut state, true, true, &mut host).unwrap().unwrap();
        assert_eq!(host.target[&key("minecraft:cobblestone")], 10);
    }

    #[test]
    fn a_filter_set_keeps_the_held_count_and_the_regulate_button_is_the_io_bus_alone() {
        let (mut part, _) = bus_part(rows::STOCK_EXPORT_BUS, &[], 0);
        let cobblestone = Key::from(&crate::game::ae2_lab::key(&ItemStack::new("minecraft:cobblestone", 1)).unwrap());
        filter_set(&mut part, 3, Some(&cobblestone), Some(&ItemStack::new("minecraft:cobblestone", 48)));
        assert_eq!(amounts(&part)[3], 48);
        filter_set(&mut part, 3, None, None);
        assert_eq!(amounts(&part)[3], 0);
        let (mut other, _) = bus_part(rows::IMPORT_EXPORT_BUS, &[], 0);
        filter_set(&mut other, 0, Some(&cobblestone), Some(&ItemStack::new("minecraft:cobblestone", 48)));
        assert!(other.get(&STOCK.into()).is_none(), "AE2's keys only");
        assert!(!setting_press(&mut part, REGULATE_PLACE) && !setting_press(&mut other, REGULATE_PLACE));
        assert!(regulates(&part), "on by default");
        assert!(tick(&Compound::new(), &mut BusState::new(ae2_cell_core::KeyType::ITEMS), true, true, &mut Fake::default()).is_none(), "AE2's bus");
    }
}
