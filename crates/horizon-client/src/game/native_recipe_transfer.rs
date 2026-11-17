//! A recipe viewer's transfer of a recipe into the open screen (`native_recipe_transfer`, JEI's "+" beside a recipe):
//! which handler of the screen takes it, which of its slots nothing on hand fills, and the move itself. The handlers
//! are JEI's basic ones (a crafting grid, a furnace's input) and, with AE2, `ae2jeiintegration`'s (the Pattern
//! Encoding Terminal's and the crafting terminal's, `ae2_recipe_transfer`). API 433.
use super::*;
use horizon_client::mod_api::{NativeRecipeTransfer, NativeStack, NativeTransferAnswer, NativeTransferHandler};

/// The furnace menu's input slot (300 input, 301 fuel, 302 result).
const FURNACE_INPUT: usize = 300;

/// JEI's `RecipeTransferUtil.getRecipeTransferOperations` in short: each input slot in turn takes one of its
/// alternatives from `pool` (item id, count), the first with one left; the slots none fills. Empty slots need
/// nothing.
pub(super) fn transfer_missing(inputs: &[Vec<NativeStack>], pool: &mut Vec<(String, u64)>) -> Vec<usize> {
    let mut missing = Vec::new();
    for (i, alts) in inputs.iter().enumerate() {
        if alts.is_empty() {
            continue;
        }
        let found = alts.iter().find_map(|a| pool.iter().position(|(item, n)| *item == a.item && *n > 0));
        match found {
            Some(k) => pool[k].1 -= 1,
            None => missing.push(i),
        }
    }
    missing
}

/// Stacks counted by item id.
pub(super) fn transfer_pool<'a>(stacks: impl Iterator<Item = &'a ItemStack>) -> Vec<(String, u64)> {
    let mut pool: Vec<(String, u64)> = Vec::new();
    for s in stacks {
        match pool.iter_mut().find(|(item, _)| *item == *s.item) {
            Some((_, n)) => *n += u64::from(s.count),
            None => pool.push((s.item.to_string(), u64::from(s.count))),
        }
    }
    pool
}

impl Game<'_> {
    /// What the open screen does with `recipe` from a recipe viewer, as JEI's `IRecipeTransferHandler` of the screen
    /// answers: no handler (the viewer hides its button), a recipe too large for the grid, the input slots nothing
    /// on hand fills (`missing`), and with AE2 the ones its network can craft. With `run` it moves the recipe as
    /// JEI's "+" does when nothing is missing: into a crafting grid as `native_transfer_recipe_all` (`max`: as many
    /// sets as the items allow, Shift), one of the input into a furnace's (`max`: the whole stack), the recipe
    /// encoded into AE2's Pattern Encoding Terminal, the ingredients pulled from the ME network into its crafting
    /// terminal (`craft_missing`: Ctrl, autocrafting asked for what is missing and craftable). Client-side. API 433.
    pub(super) fn native_recipe_transfer(
        &mut self,
        recipe: &NativeRecipeTransfer,
        run: bool,
        max: bool,
        craft_missing: bool,
    ) -> NativeTransferAnswer {
        let _ = craft_missing;
        let none = NativeTransferAnswer::default();
        if self.player.is_none() || self.spectator() || self.carried.is_some() && run {
            return none;
        }
        if recipe.category == "minecraft:crafting" && self.mod_recipe_screen() {
            let modded = self.mod_crafting_open();
            if self.creative && self.craft_size != 3 && !modded {
                return none;
            }
            let size = if modded { 3 } else { self.craft_size as u32 };
            let mut answer = NativeTransferAnswer { handler: Some(NativeTransferHandler::CraftingGrid), ..none };
            if recipe.width > size || recipe.height > size {
                answer.too_large = true;
                return answer;
            }
            // JEI counts the grid's own items with the inventory's.
            let mut pool = transfer_pool(self.inventory.iter().take(36).chain(&self.craft_grid).flatten());
            answer.missing = transfer_missing(&recipe.inputs, &mut pool);
            if run && answer.missing.is_empty() && recipe.book_id >= 0 {
                self.transfer_viewer_recipe(recipe.book_id, max);
                answer.done = true;
            }
            return answer;
        }
        let furnace = match self.remote_menu_type() {
            Some(menu) => menu.to_owned(),
            None => match self.container_kind() {
                Some(BlockEntity::Furnace(f)) => match &*f.kind.recipe_type() {
                    "minecraft:blasting" => "minecraft:blast_furnace".to_owned(),
                    "minecraft:smoking" => "minecraft:smoker".to_owned(),
                    _ => "minecraft:furnace".to_owned(),
                },
                _ => return none,
            },
        };
        let takes = match furnace.as_str() {
            "minecraft:furnace" => "minecraft:smelting",
            "minecraft:blast_furnace" => "minecraft:blasting",
            "minecraft:smoker" => "minecraft:smoking",
            _ => return none,
        };
        if recipe.category != takes || self.screen != Screen::Inventory {
            return none;
        }
        let mut answer = NativeTransferAnswer { handler: Some(NativeTransferHandler::Furnace), ..none };
        let need: &[NativeStack] = recipe.inputs.first().map_or(&[], Vec::as_slice);
        let input = self.slot_view(FURNACE_INPUT);
        let mut pool = transfer_pool(self.inventory.iter().take(36).flatten().chain(input.as_ref()));
        answer.missing = transfer_missing(&recipe.inputs[..recipe.inputs.len().min(1)], &mut pool);
        if !run || !answer.missing.is_empty() {
            return answer;
        }
        use horizon_client::menu::Input;
        let holds = |s: &ItemStack| need.iter().any(|n| *s.item == *n.item);
        // JEI clears the recipe's slots into the inventory first, then fills them.
        if input.as_ref().is_some_and(|s| !holds(s) || !max) {
            self.menu_click(Some(FURNACE_INPUT), 0, Input::QuickMove);
        }
        let Some(from) = (0..36).find(|&i| self.inventory.get(i).and_then(Option::as_ref).is_some_and(|s| holds(s))) else {
            return answer;
        };
        self.menu_click(Some(from), 0, Input::Pickup);
        self.menu_click(Some(FURNACE_INPUT), if max { 0 } else { 1 }, Input::Pickup);
        if self.carried.is_some() {
            self.menu_click(Some(from), 0, Input::Pickup);
        }
        answer.done = true;
        answer
    }
}
