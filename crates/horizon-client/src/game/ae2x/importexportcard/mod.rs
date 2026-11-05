//! AE2 Import Export Card 1.21.1-1.9.1 in the game (feature
//! `ae2-importexportcard`): the Import and Export Cards' tick on a carried
//! wireless terminal (`WirelessTerminalItemMixin`), the Crafting Card's
//! requests, the cards' screens on the terminal (`screen`, the server's side
//! here: `UpgradeContainerMenu`, `UpgradeUpdateData`) and the Block Picker
//! (`picker`). Derived from AE2 Import Export Card by Ultramega,
//! https://github.com/starforcraft/AE2-Import-Export-Card branch `1.21.1`
//! (1.9.1), MIT; see `ae2/addons/ae2importexportcard/`. The items' data and
//! the config are `horizon_client::ae2x::importexportcard`.
use crate::game::{Game, ae2_global::GridAddress, ae2_lab, ae2_wireless, lan::LanHost};
use ae2_cell_core::FuzzyMode;
use horizon_client::{
    ae2_cell_codec::{bucket_fluid, fluid_bucket, fluid_key},
    ae2_generic_stack::GenericStack,
    ae2x::{
        defs::TerminalKind,
        importexportcard::{self as iec, CardType, Config},
        wireless,
    },
    items::ItemStack,
    lan::guest::Guest,
};
use horizon_protocol::packets::play::{self, Clientbound as C};
use std::collections::HashMap;

mod picker;
pub(in crate::game) mod screen;

/// The cards' screens' actions (`UpgradeUpdateData`, `ConfigButtonPacket`,
/// the menu's slots, `SET_BLOCK_PICKER_AMOUNT`): the first byte says which,
/// the second the card (0 import, 1 export).
pub(in crate::game) const CARD_MENU: &str = "ae2importexportcard:card_menu";

/// AE2's cards on the Import and Export Cards.
const CAPACITY_CARD: &str = "ae2:capacity_card";
const FUZZY_CARD: &str = "ae2:fuzzy_card";
const INVERTER_CARD: &str = "ae2:inverter_card";
const CRAFTING_CARD: &str = "ae2:crafting_card";
const SPEED_CARD: &str = "ae2:speed_card";
/// `FluidType.BUCKET_VOLUME`: what `FluidBucketWrapper` fills and drains.
const BUCKET: u64 = 1000;
/// `Inventory`'s `getMaxStackSize` (`Container.DEFAULT_MAX_STACK_SIZE`).
const SLOT_LIMIT: u32 = 99;

/// The calculation one terminal item last began
/// (`WirelessTerminalItemMixin.craftingJob`, a field of the `Item`: one for
/// all its stacks and players).
struct PendingCraft {
    pos: (i32, i32, i32),
    key: ae2_cell_core::ItemKey,
    registry: ae2_crafting_core::extended::Registry,
    calculation: Option<ae2_crafting_core::calculation::Calculation>,
    result: Option<Result<(ae2_crafting_core::extended::Plan, ae2_crafting_core::Inventory), ae2_crafting_core::Error>>,
}

/// What the game keeps for the cards while it runs; nothing of it is saved.
#[derive(Default)]
pub(in crate::game) struct State {
    /// By the ticking terminal's item id.
    crafts: HashMap<String, PendingCraft>,
    /// This client's open card screen's selection (`UpgradeScreen.selectedInventorySlots`,
    /// read from the card when it opens and sent whole on each change).
    pub(in crate::game) selected: Option<(u8, [i32; iec::SELECTED_INVENTORY_SLOT_COUNT])>,
}

/// `Inventory.getItem` of 0..39: the main inventory, then the armour feet
/// first (the guest's armour is head first).
fn slot(g: &Guest, slot: usize) -> Option<&Option<ItemStack>> {
    match slot {
        0..=35 => g.inventory.get(slot),
        36..=39 => g.armor.get(39 - slot),
        _ => None,
    }
}
fn slot_mut(g: &mut Guest, slot: usize) -> Option<&mut Option<ItemStack>> {
    match slot {
        0..=35 => g.inventory.get_mut(slot),
        36..=39 => g.armor.get_mut(39 - slot),
        _ => None,
    }
}
/// `PlayerArmorInvWrapper`'s slot of 36..39.
const ARMOR: [&str; 4] = ["feet", "legs", "chest", "head"];

impl Game<'_> {
    /// `AEKey.fuzzyEquals`: a fluid has no damage, so its primary key is all.
    fn iecard_fuzzy(&self, a: &GenericStack, b: &GenericStack, mode: FuzzyMode) -> bool {
        // `fuzzyEquals` of a type without fuzzy ranges: the primary key alone.
        if a.key_type!=ae2_cell_core::KeyType::ITEMS || b.key_type!=ae2_cell_core::KeyType::ITEMS {
            return a.key_type == b.key_type && a.key.item_id == b.key.item_id;
        }
        let (ka, kb) = (ae2_machines_core::Key::from(&a.key), ae2_machines_core::Key::from(&b.key));
        let max = self.res.items().get(&ka.id).map_or(0, |i| i.max_damage);
        crate::game::ae2_buses::fuzzy_key_match(&ka, &kb, iec::fuzzy_lower(mode), max)
    }

    /// `WirelessTerminalItem.inventoryTick` (`Inventory.tick`: the main
    /// inventory, the armour, the off hand) for a guest: each wireless
    /// terminal's cards. A universal terminal ticks once for each terminal it
    /// holds (`ItemWUT.inventoryTick` calls each one's).
    pub(in crate::game) fn lan_iecard_tick(&mut self, host: &mut LanHost, gi: usize) {
        if self.remote.is_some() {
            return;
        }
        let Some(g) = host.hub.guests.get(gi) else { return };
        if g.spectator() {
            return;
        }
        let sources: Vec<usize> = (0..g.inventory.len()).chain(std::iter::once(crate::game::shield::OFFHAND_SLOT)).collect();
        let mut changed = false;
        for source in sources {
            let Some(stack) = host.hub.guests.get(gi).and_then(|g| ae2_wireless::source_stack(g, source)).filter(|s| wireless::host(s)).cloned() else {
                continue;
            };
            let tickers: Vec<String> = if wireless::is_universal(&stack) {
                wireless::installed(&stack).iter().map(|t| t.item.clone()).collect()
            } else {
                vec![stack.item.to_string()]
            };
            for ticker in tickers {
                changed |= self.lan_iecard_tick_terminal(host, gi, source, &ticker);
            }
        }
        if changed {
            host.hub.send_inventory(gi);
        }
    }

    /// `tickEquippedTerminal`: a terminal in range of an active access point
    /// of its grid (`getActionableNode`), each of its Import and Export Cards.
    fn lan_iecard_tick_terminal(&mut self, host: &mut LanHost, gi: usize, source: usize, ticker: &str) -> bool {
        let Some(g) = host.hub.guests.get(gi) else { return false };
        let conn = g.conn;
        let Some(stack) = ae2_wireless::source_stack(g, source).filter(|s| wireless::host(s)).cloned() else { return false };
        let cards: Vec<(CardType, ItemStack)> = iec::cards(&stack).into_iter().filter_map(|(_, card)| CardType::of(&card.item).map(|ty| (ty, card))).collect();
        if cards.is_empty() {
            return false;
        }
        let Some(address) = self.ae2_wireless_reach(host, conn, &stack).map(|reach| reach.node) else { return false };
        let mut changed = false;
        for (ty, card) in cards {
            changed |= self.lan_iecard_tick_card(host, gi, source, ticker, address, ty, &card);
        }
        changed
    }

    /// `tickUpgradeCard`: the card's filter (18 slots and 9 a Capacity Card),
    /// its Fuzzy and Inverter Cards, over each inventory slot it has on.
    #[allow(clippy::too_many_arguments)]
    fn lan_iecard_tick_card(&mut self, host: &mut LanHost, gi: usize, source: usize, ticker: &str, address: GridAddress, ty: CardType, card: &ItemStack) -> bool {
        let selected = iec::selected_slots(card);
        let upgrades = iec::card_upgrades(card, ty);
        let filter = iec::filter(card, iec::filter_slot_count(&upgrades));
        let mode = iec::fuzzy_mode(card);
        let fuzzy = iec::installed(&upgrades, FUZZY_CARD) > 0;
        let invert = iec::installed(&upgrades, INVERTER_CARD) > 0;
        let mut changed = false;
        for (index, &chosen) in selected.iter().enumerate() {
            if chosen < 1 {
                continue;
            }
            changed |= match ty {
                // Only energy exports may target the wireless terminal itself.
                CardType::Import if index == source => false,
                CardType::Import => self.lan_iecard_import(host, gi, index, address, &filter, fuzzy, invert, mode),
                CardType::Export => self.lan_iecard_export(host, gi, index, source, ticker, address, chosen as usize, &filter, &upgrades, fuzzy, mode),
            };
        }
        changed
    }

    /// `importFromPlayerSlot`: a bucket's fluid (`importFluidFromItem`), then
    /// the stack itself (`importItem`), each when the filter passes it, as
    /// much as the network takes on its grid's power.
    #[allow(clippy::too_many_arguments)]
    fn lan_iecard_import(&mut self, host: &mut LanHost, gi: usize, index: usize, address: GridAddress, filter: &[Option<GenericStack>], fuzzy: bool, invert: bool, mode: FuzzyMode) -> bool {
        let Some(item) = host.hub.guests.get(gi).and_then(|g| slot(g, index)).cloned().flatten().filter(|s| s.count > 0) else { return false };
        // FluidBucketWrapper: a lone bucket drains whole or not at all.
        if let Some(fluid) = bucket_fluid(&item.item).filter(|_| item.count == 1 && crate::game::ae2_fluids::plain_bucket(&item)) {
            let what = GenericStack::fluid(fluid_key(fluid), 0);
            if iec::passes_filter(&what, filter, fuzzy, invert, |a, b| self.iecard_fuzzy(a, b, mode))
                && self.ae2_grid_fluid_insert_encoded(address, &what.key, BUCKET, true).unwrap_or(0) >= BUCKET
                && self.ae2_grid_fluid_insert_encoded(address, &what.key, BUCKET, false).unwrap_or(0) > 0
            {
                if let Some(cell) = host.hub.guests.get_mut(gi).and_then(|g| slot_mut(g, index)) {
                    *cell = Some(ItemStack::new("minecraft:bucket", 1));
                }
                // The item step works on the emptied bucket from the next
                // tick: AE2 Import Export Card's goes on with the filled
                // bucket's stack, which would put it in too.
                return true;
            }
        }
        let Ok(key) = ae2_lab::key(&item) else { return false };
        let what = GenericStack::item(key, 0);
        if !iec::passes_filter(&what, filter, fuzzy, invert, |a, b| self.iecard_fuzzy(a, b, mode)) {
            return false;
        }
        let inserted = self.ae2_grid_insert_encoded(address, &item, u64::from(item.count)).unwrap_or(0).min(u64::from(item.count)) as u32;
        if inserted == 0 {
            return false;
        }
        if let Some(cell) = host.hub.guests.get_mut(gi).and_then(|g| slot_mut(g, index)) {
            *cell = (item.count > inserted).then(|| ItemStack { count: item.count - inserted, ..item });
        }
        true
    }

    /// `exportToPlayerSlot`: the slot's filter (counted from 1), allowed by
    /// the config; with the Fuzzy Card the network's first key it matches
    /// (`resolveExportKey`). Items fill the slot, fluids a lone empty bucket.
    #[allow(clippy::too_many_arguments)]
    fn lan_iecard_export(
        &mut self,
        host: &mut LanHost,
        gi: usize,
        index: usize,
        source: usize,
        ticker: &str,
        address: GridAddress,
        chosen: usize,
        filter: &[Option<GenericStack>],
        upgrades: &[Option<ItemStack>],
        fuzzy: bool,
        mode: FuzzyMode,
    ) -> bool {
        let Some(Some(wanted)) = filter.get(chosen - 1) else { return false };
        let config = Config::get();
        if !config.allows_export(&wanted.key.item_id, wanted.key_type) {
            return false;
        }
        let key = if fuzzy {
            match self.iecard_resolve_fuzzy(address, wanted, mode) {
                Some(key) => key,
                None => return false,
            }
        } else {
            wanted.clone()
        };
        if !config.allows_export(&key.key.item_id, key.key_type) || index == source {
            return false;
        }
        let speed = iec::installed(upgrades, SPEED_CARD) > 0;
        if key.key_type==ae2_cell_core::KeyType::FLUIDS {
            return self.lan_iecard_export_fluid(host, gi, index, address, &key, speed);
        }
        self.lan_iecard_export_item(host, gi, index, ticker, address, &key, wanted, upgrades, speed)
    }

    /// `resolveExportKey` with the Fuzzy Card: `getCachedInventory().findFuzzy`,
    /// the first allowed key, in the port's listing order.
    fn iecard_resolve_fuzzy(&self, address: GridAddress, wanted: &GenericStack, mode: FuzzyMode) -> Option<GenericStack> {
        let stock = self.ae2_grid_keys_encoded(address, wanted.key_type).ok()?;
        let config = Config::get();
        stock
            .into_iter()
            .filter(|(_, n)| *n > 0)
            .map(|(key, _)| GenericStack { key, key_type: wanted.key_type, amount: 0 })
            .find(|candidate| {
                candidate.key.item_id == wanted.key.item_id && self.iecard_fuzzy(candidate, wanted, mode) && config.allows_export(&candidate.key.item_id, candidate.key_type)
            })
    }

    /// `exportItemToPlayerSlot`: up to a full stack, one an operation (64
    /// with the Acceleration Card); none in the network asks the Crafting
    /// Card for the filter's key.
    #[allow(clippy::too_many_arguments)]
    fn lan_iecard_export_item(
        &mut self,
        host: &mut LanHost,
        gi: usize,
        index: usize,
        ticker: &str,
        address: GridAddress,
        key: &GenericStack,
        wanted: &GenericStack,
        upgrades: &[Option<ItemStack>],
        speed: bool,
    ) -> bool {
        let Ok(prototype) = ae2_lab::prototype(&key.key) else { return false };
        let max = self.res.items().stack_max(&prototype).min(SLOT_LIMIT);
        let Some(current) = host.hub.guests.get(gi).and_then(|g| slot(g, index)).cloned() else { return false };
        let count = current.as_ref().map_or(0, |s| s.count);
        if count >= max {
            return false;
        }
        let requested = (if speed { 64 } else { 1 }).min(max - count);
        let available = self.ae2_grid_extract_uncharged_encoded(address, &key.key, u64::from(requested), true).ok().flatten().map_or(0, |s| s.count);
        if available == 0 {
            self.iecard_request_craft(ticker, address, wanted, u64::from(requested), upgrades);
            return false;
        }
        // insertItem, simulated: an empty slot (an armour one for what it
        // equips there), or more of the same.
        let accepted = match &current {
            None if index >= 36 && !self.iecard_equips(&prototype, index) => 0,
            None => available.min(requested),
            Some(s) if s.same_item_same_components(&prototype) => available.min(requested),
            Some(_) => 0,
        };
        if accepted == 0 {
            return false;
        }
        let Some(extracted) = self.ae2_grid_extract_encoded(address, &key.key, u64::from(accepted)).ok().flatten() else { return false };
        let put = extracted.count.min(accepted);
        if let Some(cell) = host.hub.guests.get_mut(gi).and_then(|g| slot_mut(g, index)) {
            let now = cell.as_ref().map_or(0, |s| s.count);
            *cell = Some(ItemStack { count: now + put, ..extracted.clone() });
        }
        if extracted.count > put {
            let back = ItemStack { count: extracted.count - put, ..extracted };
            let _ = self.ae2_grid_insert_encoded(address, &back, u64::from(back.count));
        }
        true
    }

    /// `stack.canEquip(slot)` for the armour slot 36..39.
    fn iecard_equips(&self, item: &ItemStack, index: usize) -> bool {
        let Some(slot) = index.checked_sub(36).and_then(|i| ARMOR.get(i)) else { return false };
        self.res.items().get(&item.item).and_then(|i| i.equip_slot.as_deref()) == Some(*slot)
    }

    /// `exportFluidToPlayerSlot` into a lone empty bucket
    /// (`FluidBucketWrapper.fill`): a bucket of a fluid that has one, from
    /// what the network has of the 1 (or with the Acceleration Card 64)
    /// buckets asked for. Fluids ask no Crafting Card.
    fn lan_iecard_export_fluid(&mut self, host: &mut LanHost, gi: usize, index: usize, address: GridAddress, key: &GenericStack, speed: bool) -> bool {
        let Some(bucket) = fluid_bucket(&key.key.item_id).filter(|_| key.key.components.is_empty()) else { return false };
        let empty = host.hub.guests.get(gi).and_then(|g| slot(g, index)).cloned().flatten();
        if !empty.is_some_and(|s| s.count == 1 && s.item.as_ref() == "minecraft:bucket" && crate::game::ae2_fluids::plain_bucket(&s)) {
            return false;
        }
        let asked = if speed { 64 * BUCKET } else { BUCKET };
        if self.ae2_grid_fluid_extract_encoded(address, &key.key, asked, true).unwrap_or(0) < BUCKET {
            return false;
        }
        let extracted = self.ae2_grid_fluid_extract_encoded(address, &key.key, BUCKET, false).unwrap_or(0);
        if extracted < BUCKET {
            if extracted > 0 {
                let _ = self.ae2_grid_fluid_insert_encoded(address, &key.key, extracted, false);
            }
            return false;
        }
        if let Some(cell) = host.hub.guests.get_mut(gi).and_then(|g| slot_mut(g, index)) {
            *cell = Some(ItemStack::new(bucket, 1));
        }
        true
    }

    /// `CraftingService.getRequestedAmount(what) > 0`: a CPU of the grid at
    /// `pos` crafts it.
    fn iecard_crafting_requested(&self, pos: (i32, i32, i32), key: &ae2_cell_core::ItemKey) -> bool {
        let Ok(prototype) = ae2_lab::prototype(key) else { return false };
        let target = iec::canon(&prototype);
        self.ae2_recursive_jobs(pos).into_iter().filter_map(|(cpu, ..)| self.ae2_cpu_job(cpu)).any(|job| job.key == target && job.requested > 0)
    }

    /// `requestCraftingIfPossible`: with the Crafting Card, a craftable key
    /// no CPU is making yet: the terminal item's last calculation, when it
    /// has finished, is submitted (`submitJob` with no requester, a
    /// `MachineSource`), and a new one begins (`CRAFT_LESS`) for `amount`.
    fn iecard_request_craft(&mut self, ticker: &str, address: GridAddress, key: &GenericStack, amount: u64, upgrades: &[Option<ItemStack>]) {
        if key.key_type==ae2_cell_core::KeyType::FLUIDS || !Config::get().allows_export(&key.key.item_id, key.key_type) || iec::installed(upgrades, CRAFTING_CARD) == 0 {
            return;
        }
        // An unfinished calculation leaves everything as it is.
        if self.ae2x.importexportcard.crafts.get(ticker).is_some_and(|p| p.calculation.is_some()) {
            return;
        }
        let Some((dim, pos)) = self.ae2_address_parts(address) else { return };
        if dim != self.dim || !self.ae2_craftable_outputs(pos).contains(&key.key) || self.iecard_crafting_requested(pos, &key.key) {
            return;
        }
        if let Some(done) = self.ae2x.importexportcard.crafts.remove(ticker)
            && let Some(Ok((plan, _))) = done.result
            && Config::get().allows_export(&done.key.item_id, ae2_cell_core::KeyType::ITEMS)
            && !plan.simulated
            && !(plan.steps.is_empty() && plan.emitted.is_empty())
            && let Err(error) = self.ae2_begin_unowned_craft(done.pos, &done.key, &done.registry, &plan)
        {
            log::debug!("Export Card crafting job: {error}");
        }
        let Ok((registry, inventory, target)) = self.ae2_craft_calculation_snapshot(pos, &key.key, false, amount) else { return };
        let calculation = ae2_crafting_core::calculation::Calculation::with_strategy(
            registry.clone(),
            inventory,
            target,
            amount,
            ae2_crafting_core::calculation::Strategy::CraftLess,
        );
        self.ae2x.importexportcard.crafts.insert(ticker.to_owned(), PendingCraft { pos, key: key.key.clone(), registry, calculation: Some(calculation), result: None });
    }

    /// The cards' calculations, a slice each grid tick.
    pub(in crate::game) fn tick_iecard(&mut self) {
        if self.remote.is_some() || self.ae2x.importexportcard.crafts.values().all(|p| p.calculation.is_none()) {
            return;
        }
        // The level's share, which counts the machines' and terminals'
        // calculations; with none of those, the whole budget shared among the cards'.
        let mine = self.ae2x.importexportcard.crafts.values().filter(|p| p.calculation.is_some()).count().max(1);
        let shared = self.ae2_craft_tick_allowance();
        let allowance = if shared > 0 {
            shared
        } else {
            (i64::from(horizon_client::ae2_common_config::crafting_calculation_time_per_tick()) * 1000 / mine as i64).max(1) as u64
        };
        for pending in self.ae2x.importexportcard.crafts.values_mut() {
            let Some(calculation) = pending.calculation.as_mut() else { continue };
            if !calculation.run_slice(allowance) {
                pending.result = Some(calculation.take_result().unwrap_or(Err(ae2_crafting_core::Error::WorkerPanicked)));
                pending.calculation = None;
            }
        }
    }

    // The terminal's side: what its screen shows, and the cards' screens.

    /// The open wireless terminal's item slot, for a screen of `MEStorageScreen`
    /// (the ME, crafting and pattern encoding terminals).
    fn lan_iecard_source(&self, host: &LanHost, conn: u32) -> Option<usize> {
        host.play.get(&conn).and_then(|p| p.ae2_terminal.as_ref()).and_then(|t| t.wireless_source())
    }

    /// `MEStorageScreenMixin` and `UpgradeHost` as the client reads them: the
    /// Import and Export Cards' filters, selections, cards and fuzzy mode,
    /// and the Block Picker's amount, for the cards the terminal holds.
    pub(in crate::game) fn lan_iecard_wireless_view(&self, host: &LanHost, conn: u32, ctx: &ae2_wireless::WirelessContext) -> Option<serde_json::Value> {
        if !matches!(ctx.kind, TerminalKind::Me | TerminalKind::Crafting | TerminalKind::PatternEncoding) {
            return None;
        }
        let g = host.hub.guests.iter().find(|g| g.conn == conn)?;
        let stack = ae2_wireless::source_stack(g, ctx.source)?;
        let view = |ty: CardType| {
            iec::find_card(stack, ty.card()).map(|card| {
                let upgrades = iec::card_upgrades(&card, ty);
                let filters: Vec<serde_json::Value> = iec::filter(&card, iec::MAX_FILTER_SLOT_COUNT)
                    .into_iter()
                    .map(|slot| slot.map_or(serde_json::Value::Null, |s| serde_json::json!({"id": s.key.item_id, "fluid": (s.key_type==ae2_cell_core::KeyType::FLUIDS)})))
                    .collect();
                serde_json::json!({
                    "filters": filters,
                    "selected": iec::selected_slots(&card).to_vec(),
                    "upgrades": upgrades.iter().map(|s| s.as_ref().map(|s| s.item.to_string())).collect::<Vec<_>>(),
                    "fuzzy": iec::fuzzy_name(iec::fuzzy_mode(&card)),
                    "capacity": iec::installed(&upgrades, CAPACITY_CARD).min(iec::MAX_CAPACITY_CARD_COUNT),
                    "fuzzy_card": iec::installed(&upgrades, FUZZY_CARD) > 0,
                })
            })
        };
        Some(serde_json::json!({
            "import": view(CardType::Import),
            "export": view(CardType::Export),
            "picker": iec::find_card(stack, iec::BLOCK_PICKER_CARD).map(|card| iec::block_picker_amount(&card)),
        }))
    }

    /// A card screen's action on the open terminal's card.
    pub(in crate::game) fn lan_iecard_payload(&mut self, host: &mut LanHost, conn: u32, data: &[u8]) {
        let Some(source) = self.lan_iecard_source(host, conn) else { return };
        let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn && !g.spectator()) else { return };
        // SET_BLOCK_PICKER_AMOUNT: the amount (an int).
        if let [6, amount @ ..] = data {
            let Ok(amount) = <[u8; 4]>::try_from(amount) else { return };
            if let Some(stack) = ae2_wireless::source_stack_mut(&mut host.hub.guests[gi], source)
                && iec::set_block_picker_amount(stack, i32::from_be_bytes(amount))
            {
                host.hub.send_inventory(gi);
            }
            return;
        }
        let Some(ty) = data.get(1).copied().and_then(CardType::from_id) else { return };
        let carried_before = host.hub.guests[gi].carried.clone();
        let changed = match data {
            // UpgradeUpdateData: the whole selection.
            [0, _, slots @ ..] if slots.len() == iec::SELECTED_INVENTORY_SLOT_COUNT => {
                let values: Vec<i32> = slots.iter().map(|b| i32::from(*b)).collect();
                self.iecard_edit_card(host, gi, source, ty, |card| iec::set_selected_slots(card, &values))
            }
            // A filter slot pressed (FakeSlot): 0 left, 1 right.
            [1, _, index, button] => {
                let carried = host.hub.guests[gi].carried.clone();
                let index = usize::from(*index);
                let right = *button == 1;
                self.iecard_edit_card(host, gi, source, ty, |card| {
                    let mut slots = iec::filter(card, iec::MAX_FILTER_SLOT_COUNT);
                    let Some(slot) = slots.get_mut(index) else { return };
                    *slot = fake_slot_press(slot.take(), carried.as_ref(), right);
                    let _ = iec::store_filter(card, &slots);
                })
            }
            // ConfigButtonPacket(FUZZY_MODE, backwards).
            [2, _, backwards] => self.iecard_edit_card(host, gi, source, ty, |card| {
                iec::cycle_fuzzy_mode(card, *backwards != 0);
            }),
            // The card's own slot pressed: the cursor's card in, or the slot's out.
            [4, _, index, button] => self.lan_iecard_upgrade_slot(host, gi, source, ty, usize::from(*index), *button == 1),
            // A shift-click on an inventory slot (`quickMoveStack`): a card
            // into the card's slots, anything else into an empty filter slot.
            [5, _, index] => self.lan_iecard_quick_move(host, gi, source, ty, usize::from(*index)),
            _ => false,
        };
        if changed {
            if host.hub.guests[gi].carried != carried_before {
                let carried = host.hub.guests[gi].carried.clone();
                let item = host.hub.stack(&carried);
                host.hub.send_to(conn, &C::SetCursorItem(play::SetCursorItem { item }));
            }
            host.hub.send_inventory(gi);
        }
    }

    /// The terminal's card of `ty` changed, and its selections past the
    /// active filter slots cleared (`onUpgradesChanged` on the Export Card).
    fn iecard_edit_card(&mut self, host: &mut LanHost, gi: usize, source: usize, ty: CardType, edit: impl Fn(&mut ItemStack)) -> bool {
        let Some(stack) = ae2_wireless::source_stack_mut(&mut host.hub.guests[gi], source).filter(|s| wireless::host(s)) else { return false };
        iec::edit_card(stack, ty.card(), |card| {
            edit(card);
            if ty == CardType::Export {
                let active = iec::filter_slot_count(&iec::card_upgrades(card, ty));
                let mut slots = iec::selected_slots(card);
                if iec::clamp_selected(&mut slots, active) {
                    iec::set_selected_slots(card, &slots);
                }
            }
        })
    }

    /// The card's own slot `index` pressed (`RestrictedInputSlot` of its
    /// upgrades): an empty cursor takes its card, a card that fits goes in
    /// (an occupied slot swaps).
    fn lan_iecard_upgrade_slot(&mut self, host: &mut LanHost, gi: usize, source: usize, ty: CardType, index: usize, _right: bool) -> bool {
        let Some(card) = ae2_wireless::source_stack(&host.hub.guests[gi], source).and_then(|s| iec::find_card(s, ty.card())) else { return false };
        let mut upgrades = iec::card_upgrades(&card, ty);
        if index >= upgrades.len() {
            return false;
        }
        let carried = host.hub.guests[gi].carried.clone().filter(|s| s.count > 0);
        let held = upgrades[index].take();
        let (next_slot, next_carried) = match (carried, held) {
            (None, held) => (None, held),
            (Some(card_item), held) => {
                if !iec::card_accepts(ty, &upgrades, &card_item) {
                    upgrades[index] = held;
                    return false;
                }
                let one = ItemStack { count: 1, ..card_item.clone() };
                let rest = (card_item.count > 1).then(|| ItemStack { count: card_item.count - 1, ..card_item });
                match (held, rest) {
                    (Some(_), Some(_)) => return false,
                    (Some(old), None) => (Some(one), Some(old)),
                    (None, rest) => (Some(one), rest),
                }
            }
        };
        if next_slot.is_none() && next_carried.is_none() {
            return false;
        }
        upgrades[index] = next_slot;
        let stored = self.iecard_edit_card(host, gi, source, ty, |card| {
            let _ = iec::store_card_upgrades(card, &upgrades);
        });
        if stored {
            host.hub.guests[gi].carried = next_carried;
        }
        stored
    }

    /// `AEBaseMenu.quickMoveStack` from an inventory slot: into the card's
    /// slots when it is a card they take, else (no destination) into the
    /// first empty filter slot unless the filter has it.
    fn lan_iecard_quick_move(&mut self, host: &mut LanHost, gi: usize, source: usize, ty: CardType, index: usize) -> bool {
        let Some(stack) = host.hub.guests.get(gi).and_then(|g| slot(g, index)).cloned().flatten().filter(|s| s.count > 0) else { return false };
        let Some(card) = ae2_wireless::source_stack(&host.hub.guests[gi], source).and_then(|s| iec::find_card(s, ty.card())) else { return false };
        let mut upgrades = iec::card_upgrades(&card, ty);
        if iec::card_accepts(ty, &upgrades, &stack)
            && let Some(empty) = upgrades.iter().position(Option::is_none)
        {
            upgrades[empty] = Some(ItemStack { count: 1, ..stack.clone() });
            let stored = self.iecard_edit_card(host, gi, source, ty, |card| {
                let _ = iec::store_card_upgrades(card, &upgrades);
            });
            if stored && let Some(cell) = host.hub.guests.get_mut(gi).and_then(|g| slot_mut(g, index)) {
                *cell = (stack.count > 1).then(|| ItemStack { count: stack.count - 1, ..stack.clone() });
            }
            return stored;
        }
        let Ok(key) = ae2_lab::key(&stack) else { return false };
        let what = GenericStack::item(key, 0);
        self.iecard_edit_card(host, gi, source, ty, |card| {
            let mut slots = iec::filter(card, iec::MAX_FILTER_SLOT_COUNT);
            if slots.iter().flatten().any(|s| s.key_type!=ae2_cell_core::KeyType::FLUIDS && s.key == what.key) {
                return;
            }
            if let Some(empty) = slots.iter_mut().find(|s| s.is_none()) {
                *empty = Some(what.clone());
                let _ = iec::store_filter(card, &slots);
            }
        })
    }
}

impl Game<'_> {
    /// `/ae2 iecard filter <slot> <import|export> <index> <id|-> [fluid]`: a
    /// filter slot of the card in the wireless terminal at the acting
    /// player's inventory `slot` (26.3's SNBT has no empty key, which
    /// `filter_config` keeps its keys under); `/ae2 iecard card <slot>
    /// <import|export>`: that card's data.
    pub(in crate::game) fn ae2_iecard_command(&mut self, words: &[&str]) -> Result<serde_json::Value, String> {
        let card_type = |word: &str| match word {
            "import" => Ok(CardType::Import),
            "export" => Ok(CardType::Export),
            _ => Err(format!("not a card: {word}")),
        };
        let terminal = |game: &mut Self, slot: &str| -> Result<usize, String> {
            let slot: usize = slot.parse().map_err(|_| "bad slot")?;
            game.inventory.get(slot).and_then(Option::as_ref).filter(|s| wireless::host(s)).ok_or("no wireless terminal in that slot")?;
            Ok(slot)
        };
        match words {
            ["filter", slot, ty, index, id, rest @ ..] => {
                let (slot, ty) = (terminal(self, slot)?, card_type(ty)?);
                let index: usize = index.parse().ok().filter(|i| *i < iec::MAX_FILTER_SLOT_COUNT).ok_or("bad filter index")?;
                let key = match *id {
                    "-" => None,
                    id if rest.first() == Some(&"fluid") => Some(GenericStack::fluid(fluid_key(id), 0)),
                    id => Some(GenericStack::item(ae2_lab::key(&ItemStack::new(id, 1))?, 0)),
                };
                let Some(Some(stack)) = self.inventory.get_mut(slot) else { return Err("no terminal".into()) };
                let edited = iec::edit_card(stack, ty.card(), |card| {
                    let mut slots = iec::filter(card, iec::MAX_FILTER_SLOT_COUNT);
                    slots[index] = key.clone();
                    let _ = iec::store_filter(card, &slots);
                });
                if !edited {
                    return Err("the terminal has no such card".into());
                }
                self.ae2_iecard_command(&["card", &slot.to_string(), words[2]])
            }
            ["card", slot, ty] => {
                let (slot, ty) = (terminal(self, slot)?, card_type(ty)?);
                let stack = self.inventory[slot].as_ref().ok_or("no terminal")?;
                let card = iec::find_card(stack, ty.card()).ok_or("the terminal has no such card")?;
                let upgrades = iec::card_upgrades(&card, ty);
                Ok(serde_json::json!({
                    "filters": iec::filter(&card, iec::filter_slot_count(&upgrades)).iter().map(|s| s.as_ref().map(|s| s.key.item_id.clone())).collect::<Vec<_>>(),
                    "selected": iec::selected_slots(&card).to_vec(),
                    "upgrades": upgrades.iter().map(|s| s.as_ref().map(|s| s.item.to_string())).collect::<Vec<_>>(),
                    "fuzzy": iec::fuzzy_name(iec::fuzzy_mode(&card)),
                }))
            }
            _ => Err("usage: ae2 iecard filter <slot> <import|export> <index> <id|-> [fluid] | card <slot> <import|export>".into()),
        }
    }
}

/// `handleFakeSlotAction` on a key-only filter slot: a left press sets the
/// cursor's item (an empty cursor clears); a right press clears a set slot or
/// sets an empty one; a right press with a filled bucket sets its fluid
/// (`EMPTY_ITEM`).
fn fake_slot_press(current: Option<GenericStack>, carried: Option<&ItemStack>, right: bool) -> Option<GenericStack> {
    let carried = carried.filter(|c| c.count > 0);
    if right
        && let Some(fluid) = carried.and_then(|c| bucket_fluid(&c.item))
    {
        return Some(GenericStack::fluid(fluid_key(fluid), 0));
    }
    let item = || carried.and_then(|c| ae2_lab::key(c).ok()).map(|key| GenericStack::item(key, 0));
    match (right, current) {
        (false, _) => item(),
        (true, Some(_)) => None,
        (true, None) => item(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_filter_slot_takes_the_cursor_and_a_bucket_its_fluid() {
        let cobble = ItemStack::new("minecraft:cobblestone", 5);
        let set = fake_slot_press(None, Some(&cobble), false).unwrap();
        assert_eq!((set.key.item_id.as_str(), set.key_type, set.amount), ("minecraft:cobblestone", ae2_cell_core::KeyType::ITEMS, 0));
        assert_eq!(fake_slot_press(Some(set.clone()), None, false), None, "an empty cursor clears");
        assert_eq!(fake_slot_press(Some(set.clone()), Some(&cobble), true), None, "right: a set slot clears");
        assert_eq!(fake_slot_press(None, Some(&cobble), true).unwrap().key.item_id, "minecraft:cobblestone");
        let water = fake_slot_press(None, Some(&ItemStack::new("minecraft:water_bucket", 1)), true).unwrap();
        assert_eq!((water.key.item_id.as_str(), water.key_type), ("minecraft:water", ae2_cell_core::KeyType::FLUIDS));
        let bucket = fake_slot_press(None, Some(&ItemStack::new("minecraft:water_bucket", 1)), false).unwrap();
        assert_eq!(bucket.key_type, ae2_cell_core::KeyType::ITEMS, "a left press sets the bucket item");
    }
}
