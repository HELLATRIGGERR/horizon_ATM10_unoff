//! MEGA's Decompression Module (`DecompressionModulePart`) and the grid's
//! `DecompressionService`: while a grid has a module, the bulk cells mounted
//! on it offer their compression chains' patterns to autocrafting (a global
//! crafting provider, `getDecompressionPatterns` at each cell's cutoff); a
//! pushed pattern spends its input and puts its output into the grid's
//! storage at the next tick (`pushPattern`, `onServerStartTick`). The
//! service's pattern priority is the grid's, set through any module's
//! priority menu and saved on each module (`dcp`). Derived from MEGA Cells
//! 4.11.0, https://github.com/62832/MEGACells tag 4.11.0, LGPL-3.0; see
//! `ae2/addons/megacells/`.
use super::bulk::BulkStack;
use crate::game::{Game, ae2_global::GridAddress, ae2_parts::PartAddress};
use ae2_megacells_core::Pattern;
use horizon_client::{ae2x::megacells as rows, items::ItemStack};
use horizon_world::nbt::Tag;

/// `DecompressionModulePart.getBoxes`: the Cell Dock's.
pub(in crate::game) fn boxes(id: &str) -> Option<Vec<[f64; 6]>> {
    (id == rows::DECOMPRESSION_MODULE).then(|| vec![[3., 3., 12., 13., 13., 16.], [5., 5., 11., 11., 11., 12.]])
}

/// `onUseWithoutItem` opens AE2's `PriorityMenu` on the part, titled as
/// `PriorityScreen` is.
pub(in crate::game) fn menu_title(id: &str) -> Option<&'static str> {
    (id == rows::DECOMPRESSION_MODULE).then_some("Priority")
}

/// A `pushPattern` output waiting for the next tick (`patternOutputs`): the
/// grid it goes into, by a device of it, the item and its amount.
pub(in crate::game) struct PendingOutput {
    anchor: GridAddress,
    stack: ItemStack,
    amount: u64,
    /// The game tick it was pushed in: it goes in from the next one on.
    pushed: i64,
}

impl Game<'_> {
    /// The grid's Decompression Modules (`installedModules` counts every
    /// module node of the grid).
    pub(in crate::game) fn megacells_decompression_modules(&self, anchor: GridAddress) -> Vec<PartAddress> {
        self.ae2_grid_parts_of(anchor, rows::DECOMPRESSION_MODULE).unwrap_or_default()
    }

    /// `getPatternPriority`: the first saved priority of the grid's modules
    /// (`addNode` locks the first it reads), 0 before any was set.
    pub(in crate::game) fn megacells_decompression_priority(&self, anchor: GridAddress) -> i32 {
        for module in self.megacells_decompression_modules(anchor) {
            if let Ok(Some(machine)) = self.ae2_part_machine(module) {
                if let Some(Tag::Int(value)) = machine.config.get(&rows::PATTERN_PRIORITY.into()) {
                    return *value;
                }
            }
        }
        0
    }

    /// `DecompressionModulePart.setPriority` (`setPatternPriority`, then
    /// `saveNodeData` on every module of the grid): the grid's priority, kept
    /// on each module.
    pub(in crate::game) fn megacells_set_decompression_priority(&mut self, part: PartAddress, value: i32) -> bool {
        let mut stored = false;
        for module in self.megacells_decompression_modules(part.host) {
            let Ok(Some(mut machine)) = self.ae2_part_machine(module) else { continue };
            machine.config.insert(rows::PATTERN_PRIORITY.into(), Tag::Int(value));
            stored |= self.ae2_store_part_machine(module, &machine).is_ok();
        }
        stored
    }

    /// `getAvailablePatterns` after `onServerEndTick`: with a module on the
    /// grid, the decompression patterns of every bulk cell its cell hosts
    /// mount, each once (the crafting service keeps equal patterns once).
    pub(in crate::game) fn megacells_decompression_patterns(&self, anchor: GridAddress) -> Vec<(PartAddress, Pattern)> {
        let Some(module) = self.megacells_decompression_modules(anchor).into_iter().next() else { return Vec::new() };
        let Ok(cells) = self.ae2_grid_cells_encoded(anchor) else { return Vec::new() };
        let mut patterns: Vec<Pattern> = Vec::new();
        for stack in cells.iter().filter(|stack| stack.item.as_ref() == rows::BULK_ITEM_CELL) {
            let Ok(BulkStack { mut cell }) = self.megacells_bulk_cell(stack) else { continue };
            for pattern in cell.decompression_patterns() {
                if !patterns.contains(&pattern) {
                    patterns.push(pattern);
                }
            }
        }
        patterns.into_iter().map(|pattern| (module, pattern)).collect()
    }

    /// `pushPattern`: the output joins those the grid of `anchor` gets at the
    /// next tick (`patternOutputs.merge`).
    pub(in crate::game) fn megacells_push_decompression(&mut self, anchor: GridAddress, stack: &ItemStack, amount: u64) {
        let pushed = self.world.game_time;
        let pending = &mut self.ae2x.megacells.decompression_outputs;
        let mut one = stack.clone();
        one.count = 1;
        match pending.iter_mut().find(|p| p.anchor == anchor && p.stack == one) {
            Some(entry) => entry.amount = entry.amount.saturating_add(amount),
            None => pending.push(PendingOutput { anchor, stack: one, amount, pushed }),
        }
    }

    /// `onServerStartTick`: each waiting output into its grid's storage, what
    /// does not fit kept for the next tick.
    pub(in crate::game) fn tick_megacells_decompression(&mut self) {
        if self.ae2x.megacells.decompression_outputs.is_empty() {
            return;
        }
        let pending = std::mem::take(&mut self.ae2x.megacells.decompression_outputs);
        let now = self.world.game_time;
        let mut kept = Vec::new();
        for mut output in pending {
            while output.amount > 0 && output.pushed < now {
                let mut stack = output.stack.clone();
                stack.count = output.amount.min(u64::from(u32::MAX)) as u32;
                let inserted = self.ae2_grid_insert_uncharged_storage_encoded(output.anchor, &stack, u64::from(stack.count), false).unwrap_or(0);
                if inserted == 0 {
                    break;
                }
                output.amount -= inserted;
            }
            if output.amount > 0 {
                kept.push(output);
            }
        }
        self.ae2x.megacells.decompression_outputs.extend(kept);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The module's boxes, its menu's title, and its row: 10 AE/t, a channel,
    /// a part machine of its own kind with no slots.
    #[test]
    fn the_module_is_a_part_of_ten_ae_a_tick_with_a_channel() {
        horizon_client::ae2x::install();
        let part = horizon_client::ae2x::defs::get().part(rows::DECOMPRESSION_MODULE).unwrap();
        assert_eq!((part.idle_power, part.requires_channel, part.cells), (10.0, true, 0));
        let machine = crate::game::ae2_parts::part_machine(rows::DECOMPRESSION_MODULE).unwrap();
        assert!(machine.slots.is_empty());
        assert_eq!(boxes(rows::DECOMPRESSION_MODULE).unwrap().len(), 2);
        assert_eq!(menu_title(rows::DECOMPRESSION_MODULE), Some("Priority"));
        assert!(boxes("megacells:cell_dock").is_none());
    }
}
