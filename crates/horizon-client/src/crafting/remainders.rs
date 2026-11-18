//! Crafting remainders shared by menus and automated crafters.
use super::*;
impl Recipes {
    /// `CraftingRecipe.getRemainingItems` placed on the `size`x`size` grid: an item's
    /// crafting remainder (buckets, bottles), the patterned banner a duplicate
    /// keeps and the written book a clone keeps.
    pub fn remainders(
        &self,
        recipe: &str,
        grid: &[Option<ItemStack>],
        size: usize,
        remainder: &dyn Fn(&ItemStack) -> Option<ItemStack>,
    ) -> Vec<Option<ItemStack>> {
        self.remainders_with_durability(recipe, grid, size, remainder, None)
    }
    /// [`Self::remainders`] for a committed craft, which may supply server-side
    /// enchantment and random durability processing (`durability`: the loss of a
    /// use of `amount`). Planning and preview use `remainders` and do not consume
    /// randomness. AE2's recipes keep their own remainders; its quartz cutting
    /// knife stays in the grid, worn by one use.
    pub fn remainders_with_durability(
        &self,
        recipe: &str,
        grid: &[Option<ItemStack>],
        size: usize,
        remainder: &dyn Fn(&ItemStack) -> Option<ItemStack>,
        mut durability: Option<&mut dyn FnMut(&ItemStack, u32) -> u32>,
    ) -> Vec<Option<ItemStack>> {
        #[cfg(feature = "ae2-lab")]
        if let Some(result) = super::ae2::remainders(self, recipe, grid, size) {
            return result;
        }
        let mut out = vec![None; size * size];
        let kind = self
            .crafting
            .iter()
            .find(|(n, _)| n == recipe)
            .map(|(_, r)| r);
        if let Some(Recipe::Mod(recipe)) = kind
            && let Some(result) = recipe.remainders(grid, size)
        {
            return result;
        }
        let Some(input) = Input::of(grid, size) else {
            return out;
        };
        let mut damaged_knife = false;
        for (i, cell) in input.cells.iter().enumerate() {
            let Some(s) = cell else { continue };
            let at = (input.top + i / input.width) * size + input.left + i % input.width;
            if let Some(Recipe::Shapeless {
                quartz_knife: Some(knife),
                ..
            }) = kind
                && !damaged_knife
                && knife.items.contains(s.item.as_ref())
            {
                damaged_knife = true;
                let mut remaining = (*s).clone();
                remaining.count = 1;
                let max = self.facts.get(&s.item).map_or(0, |f| f.info.max_damage);
                if max > 0 {
                    let loss = durability.as_mut().map_or(1, |apply| apply(s, 1));
                    remaining.damage = remaining.damage.saturating_add(loss);
                }
                if max == 0 || remaining.damage < max {
                    out[at] = Some(remaining);
                }
                continue;
            }
            let rest = remainder(s);
            if rest.is_some() {
                out[at] = rest;
                continue;
            }
            let copy = ItemStack {
                count: 1,
                ..(*s).clone()
            };
            match kind {
                Some(Recipe::Special(Special::BannerDuplicate { .. }))
                    if !crate::banner::layers(s).is_empty() =>
                {
                    out[at] = Some(copy);
                }
                // BookCloningRecipe stops at the first copied book.
                Some(Recipe::Special(Special::BookCloning { .. }))
                    if s.data().is_some_and(|d| d.written.is_some()) =>
                {
                    out[at] = Some(copy);
                    break;
                }
                _ => {}
            }
        }
        out
    }
}
