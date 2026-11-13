//! The addons' upgrade kits, their rows and what a kit makes of a device (the lib half; the
//! item's use is the crate horizon-ae2x-kits's): an item that, used on a device, puts another in
//! its place with all it holds. ExtendedAE's `ItemUpgrade` (`FCUtil.replaceTile`,
//! `replacePart`), derived from ExtendedAE 1.21-2.2.42,
//! https://github.com/GlodBlock/ExtendedAE commit
//! 1f86f927552f02451236465c0c96a12a3de8f66c, LGPL-3.0; see `ae2/addons/extendedae/`.
//! Advanced AE's provider upgrades (`AdvPatternProviderUpgradeItem`,
//! `AdvPatternProviderCapacityUpgradeItem`) act the same; see
//! `ae2/addons/advancedae/`.
//!
//! The item acts through `useOn`, which vanilla reaches only past the block's
//! own use: with a device that opens a menu, while the player sneaks. A kit
//! matches a device by its exact id, so a device it made takes none of it.
use crate::{
    ae2_world::{Machine, variant},
    items::ItemStack,
};
use horizon_world::nbt::Tag;

/// A kit: the item, the devices it replaces and the cable parts it replaces,
/// each with what takes its place.
pub struct Kit {
    pub item: &'static str,
    pub blocks: &'static [(&'static str, &'static str)],
    pub parts: &'static [(&'static str, &'static str)],
}

/// The machine of row `to` holding all of `old`: its slots keep their numbers,
/// its settings their values (`FCUtil.replaceTile`: the new block entity loads
/// what the old one saved).
pub fn upgraded(old: &Machine, to: &str) -> Option<Machine> {
    // A variant of AE2's device, or one of an addon's own blocks into another
    // (ExtendedAE's Wireless Connector into the Hub).
    let mut next = match variant(to) {
        Some(row) => Machine::of_variant(row),
        None => Machine::for_block(to).filter(|m| m.kind.ext().is_some() && old.kind.ext().is_some())?,
    };
    let same_kind = next.kind == old.kind || next.kind.ext().is_some();
    if !same_kind || next.slots.len() < old.slots.len() {
        return None;
    }
    next.slots[..old.slots.len()].clone_from_slice(&old.slots);
    // `AppEngInternalInventory.readFromNBT`: the new device reads the cards
    // its own upgrade inventory has slots for. Applied Flux's slot on AE2's
    // provider has none on Advanced AE's, whose logic has no upgrade inventory.
    let cards = next.upgrades.len().min(old.upgrades.len());
    next.upgrades[..cards].clone_from_slice(&old.upgrades[..cards]);
    next.energy = old.energy;
    next.config = old.config.clone();
    Some(next)
}

/// The part record for `part` as part `to`: a new part in the same place,
/// with what the old one wrote (`replacePart`, then `readFromNBT`).
pub fn upgraded_part(part: &Tag, to: &str) -> Option<Tag> {
    let mut next = part.clone();
    let stack = part.get("stack").and_then(crate::save::item_from_nbt)?;
    let stack = ItemStack { item: to.into(), ..stack };
    let Tag::Compound(record) = &mut next else { return None };
    record.insert("id".into(), Tag::String(to.into()));
    record.insert("stack".into(), crate::save::item_to_nbt(&stack));
    record.insert("ae2_part_identity".into(), Tag::IntArray(crate::save::random_uuid().to_vec()));
    Some(next)
}
