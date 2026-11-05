//! AE2WTLib's Restock and Pick Block on the stacks a player holds
//! (`AE2wtlibEvents.restock` and `pickBlock`, `MagnetHandler.sendRestockAble`,
//! `GuiMixin.restockOverlay`): what moves between a slot and the network,
//! and the hotbar's count. AE2WTLib 19.5.1 by Mari023,
//! https://github.com/Mari023/AE2WirelessTerminalLibrary tag `19.5.1`, MIT;
//! see `ae2/addons/ae2wtlib/`. The player's side is the game's
//! (`game/ae2x/ae2wtlib/restock.rs`).
use crate::items::ItemStack;

/// `AE2wtlibTags.NO_RESTOCK`.
pub const NO_RESTOCK: &str = "ae2wtlib:no_restock";

/// `AE2wtlibEvents.restock`'s move, after its checks of the player and the
/// terminal: how many of `item` the network gives (positive) or takes
/// (negative) so that the slot now holding `now` holds `max` (the item's
/// `getMaxStackSize`). None when nothing moves: `item` is empty, the slot is
/// full, or it holds another item.
pub fn change(item: &ItemStack, now: Option<&ItemStack>, max: u32) -> Option<i64> {
    if item.count == 0 {
        return None;
    }
    let count = now.map_or(0, |n| n.count);
    let to_add = i64::from(max) - i64::from(count);
    if to_add == 0 {
        return None;
    }
    if now.is_some_and(|n| n.count > 0 && !n.same_item_same_components(item)) {
        return None;
    }
    Some(to_add)
}

/// `item.setCount(count + changed)`: what the slot holds after the network
/// moved `moved` (signed as [`change`]'s) onto `now`'s count; none when
/// that is nothing.
pub fn restocked(item: &ItemStack, now: Option<&ItemStack>, moved: i64) -> Option<ItemStack> {
    let count = i64::from(now.map_or(0, |n| n.count)) + moved;
    let count = u32::try_from(count.max(0)).unwrap_or(u32::MAX);
    (count > 0).then(|| ItemStack { count, ..item.clone() })
}

/// `GuiMixin.restockOverlay`: with restock on, a hotbar stack of more than
/// one whose item the server counted (`isRestockAble`) shows what the player
/// can reach, its own count and the network's (`getAccessibleAmount`), as
/// AE2's `ReadableNumberConverter.format(n, 3)` writes it (`format`).
pub fn hotbar_count(stack: &ItemStack, network: Option<u64>, format: impl Fn(u64) -> String) -> Option<String> {
    if stack.count == 1 {
        return None;
    }
    Some(format(u64::from(stack.count).saturating_add(network?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restock_fills_the_slot_from_the_network_or_returns_the_excess() {
        let cobble = ItemStack::new("minecraft:cobblestone", 63);
        // A block placed: the slot holds 63 of 64.
        assert_eq!(change(&cobble, Some(&cobble), 64), Some(1));
        assert_eq!(restocked(&cobble, Some(&cobble), 1).map(|s| s.count), Some(64));
        // The network had only part of it.
        let few = ItemStack::new("minecraft:cobblestone", 10);
        assert_eq!(change(&few, Some(&few), 64), Some(54));
        assert_eq!(restocked(&few, Some(&few), 20).map(|s| s.count), Some(30));
        // A full stack moves nothing; an empty item has nothing to restock.
        assert_eq!(change(&ItemStack::new("minecraft:cobblestone", 64), None, 64), Some(64), "an emptied slot is filled");
        assert_eq!(change(&ItemStack::new("minecraft:cobblestone", 64), Some(&ItemStack::new("minecraft:cobblestone", 64)), 64), None);
        assert_eq!(change(&ItemStack::new("minecraft:cobblestone", 0), None, 64), None);
        // The slot holds something else now (a bowl after a stew).
        assert_eq!(change(&ItemStack::new("minecraft:mushroom_stew", 1), Some(&ItemStack::new("minecraft:bowl", 1)), 1), None);
        // More than a stack goes back to the network.
        let over = ItemStack::new("minecraft:ender_pearl", 20);
        assert_eq!(change(&over, Some(&over), 16), Some(-4));
        assert_eq!(restocked(&over, Some(&over), -4).map(|s| s.count), Some(16));
        // Nothing came: the slot stays empty.
        assert_eq!(restocked(&cobble, None, 0), None);
    }

    #[test]
    fn the_hotbar_shows_its_own_and_the_networks_count() {
        let format = |n: u64| n.to_string();
        let stack = ItemStack::new("minecraft:cobblestone", 32);
        assert_eq!(hotbar_count(&stack, Some(1000), format).as_deref(), Some("1032"));
        assert_eq!(hotbar_count(&stack, None, format), None, "an item the server did not count");
        assert_eq!(hotbar_count(&ItemStack::new("minecraft:cobblestone", 1), Some(5), format), None, "a single item keeps its look");
    }
}
