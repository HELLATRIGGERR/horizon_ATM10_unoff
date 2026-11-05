//! AE2 Import Export Card's Block Picker Card (`BlockPickerHandler`,
//! `MinecraftMixin.pickBlock`, `BlockPickerData`) for the acting player.
//! 26.3 picks on the server (`tryPickItem`), so the pick runs there, with the
//! player's state in the game's (a guest's turn); AE2WTLib's Pick Block, when
//! built in, answers first, as its packet reaches the server first. Derived
//! from AE2 Import Export Card by Ultramega,
//! https://github.com/starforcraft/AE2-Import-Export-Card branch `1.21.1`
//! (1.9.1), MIT; see `ae2/addons/ae2importexportcard/`.
use crate::game::{Game, ae2_lab};
use horizon_client::{
    ae2x::{
        importexportcard::{self as iec, Config},
        wireless,
    },
    command::args::GameMode,
    items::ItemStack,
};

/// `PLAYER_INVENTORY_SLOT_COUNT`: the main inventory.
const MAIN: usize = 36;

impl Game<'_> {
    /// `Inventory.getItem` of 0..40 for the acting player: the main
    /// inventory, the armour feet first, the off hand.
    fn iecard_picker_slot(&mut self, slot: usize) -> Option<&mut Option<ItemStack>> {
        match slot {
            0..MAIN => self.inventory.get_mut(slot),
            36..=39 => self.armor.get_mut(39 - slot),
            40 => Some(&mut self.offhand),
            _ => None,
        }
    }

    /// `BlockPickerHandler.handle` for a block the acting player picks: in
    /// survival, a stack it holds nowhere (`contains`: all 41 slots), a free
    /// main slot (`getFreeSlot`), an item the config lets out; the first
    /// carried terminal with the card that can bring it (`tryExtractFromTerminals`)
    /// does, and `pickSlot` moves it to the suitable hotbar slot, selected.
    pub(in crate::game) fn iecard_pick_block(&mut self, stack: &ItemStack) -> bool {
        if self.remote.is_some() || self.mode != GameMode::Survival || stack.count == 0 {
            return false;
        }
        let held = self.inventory.iter().chain(self.armor.iter()).chain(std::iter::once(&self.offhand)).flatten();
        if held.into_iter().any(|s| s.same_item_same_components(stack)) {
            return false;
        }
        let Some(destination) = self.inventory.iter().take(MAIN).position(Option::is_none) else { return false };
        if ae2_lab::key(stack).is_err() || !Config::get().allows_export(&stack.item, false) {
            return false;
        }
        for slot in 0..=40 {
            if self.iecard_pick_from(slot, destination, stack) {
                let target = horizon_client::pick_item::suitable_hotbar_slot(&self.inventory[..9], self.selected);
                self.inventory.swap(target, destination);
                self.selected = target;
                return true;
            }
        }
        false
    }

    /// `tryExtract`: a wireless terminal with the Block Picker Card, in range
    /// of an access point of its grid, brings its amount (as far as the main
    /// inventory has room) on its own battery's power (`poweredExtraction`
    /// with the menu host: an AE an item, `usePower` of what it found).
    fn iecard_pick_from(&mut self, slot: usize, destination: usize, stack: &ItemStack) -> bool {
        let Some(terminal) = self.iecard_picker_slot(slot).and_then(|s| s.clone()).filter(|s| wireless::host(s)) else { return false };
        if iec::find_card(&terminal, iec::BLOCK_PICKER_CARD).is_none() {
            return false;
        }
        let position = self.player.as_ref().map_or([0.0; 3], |p| [p.position.x, p.position.y, p.position.z]);
        let Some(address) = self.ae2_wireless_reach_from(position, &terminal).map(|reach| reach.node) else { return false };
        let Ok(key) = ae2_lab::key(stack) else { return false };
        let max = self.res.items().stack_max(stack).max(1);
        // getAvailableCapacity: the main inventory's room for the item.
        let capacity: u32 = self.inventory[..MAIN]
            .iter()
            .map(|s| match s {
                None => max,
                Some(s) if s.same_item_same_components(stack) => max.saturating_sub(s.count),
                Some(_) => 0,
            })
            .sum();
        let requested = (iec::terminal_block_picker_amount(&terminal) as u32).min(capacity);
        if requested == 0 {
            return false;
        }
        // What the network has of it, past a stack too (simulated, unpowered).
        let mut plan = crate::game::ae2_buses::FormationPlan::default();
        let retrieved = self.ae2_grid_extract_bulk_staged_encoded(address, &key, u64::from(requested), false, true, &mut plan).unwrap_or(0).min(u64::from(requested)) as u32;
        let battery = wireless::energy(&terminal).map_or(0.0, |(power, _)| power);
        let powered = f64::from(retrieved).min(battery);
        let amount = ((powered + 0.9).floor() as u32).min(retrieved);
        if amount == 0 {
            return false;
        }
        if let Some(Some(terminal)) = self.iecard_picker_slot(slot) {
            let _ = wireless::transfer_energy(terminal, powered, false, false);
        }
        // A stack an extraction: the amount in turns.
        let mut extracted = 0u32;
        while extracted < amount {
            let Some(got) = self.ae2_grid_extract_uncharged_encoded(address, &key, u64::from((amount - extracted).min(max)), false).ok().flatten() else { break };
            if got.count == 0 {
                break;
            }
            extracted += got.count;
        }
        if extracted == 0 {
            return false;
        }
        // insertIntoInventory: the free slot first, then the main inventory in order.
        let mut left = extracted;
        for index in std::iter::once(destination).chain((0..MAIN).filter(|i| *i != destination)) {
            if left == 0 {
                break;
            }
            let cell = &mut self.inventory[index];
            match cell {
                None => {
                    let put = left.min(max);
                    *cell = Some(ItemStack { count: put, ..stack.clone() });
                    left -= put;
                }
                Some(s) if s.same_item_same_components(stack) && s.count < max => {
                    let put = left.min(max - s.count);
                    s.count += put;
                    left -= put;
                }
                Some(_) => {}
            }
        }
        true
    }
}
