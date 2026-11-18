//! The game's own item capability (`mod_api::ITEMS`, NeoForge's `Capabilities.Item.BLOCK`):
//! container slots and the composter's state-backed input/output, by the rules hoppers follow.
//! Side-local slots are numbered from zero. It acts where the world is simulated, never on
//! a client's copy, and only on the level in place.
use super::{Game, Kind, Pos, Slots, View};
use crate::game::transactions::After;
use horizon_client::items::ItemStack;
use horizon_core::geometry::Direction;
use mod_api::__private::game::CapabilityChecked;
use mod_api::{BlockSide, LevelRef, Transaction};

/// A slot of a container as a side reaches it for putting a stack in
/// (`native_insert_slots`).
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeInsertSlot {
    /// Its number.
    pub(crate) slot: usize,
    pub(crate) stack: Option<ItemStack>,
    /// Whether it takes the stack (`Container.canPlaceItem`, a furnace's
    /// `canPlaceItemThroughFace`, a mod's `inputs` and slot rule).
    pub(crate) accepts: bool,
    /// How many of the stack's item it holds at most.
    pub(crate) max: u32,
}

/// A slot of a container as a side reaches it for taking from (`native_container_slots`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeContainerSlot {
    /// Its number, for `native_extract_item`.
    pub(crate) slot: usize,
    pub(crate) stack: Option<ItemStack>,
    /// Whether its stack can be taken through this side: not a furnace's fuel
    /// through its bottom, unless it's a bucket, nor a brewing stand's reagent
    /// unless it's a glass bottle, nor a mod's slot outside its `outputs`.
    pub(crate) can_take: bool,
}

impl View {
    /// Composter capability ports keep one slot as their level changes; hopper views retain
    /// vanilla's level-dependent slot enumeration.
    fn capability_slots(&self, face: Option<Direction>) -> Slots<'_> {
        if matches!(self.kind, Kind::Composter(_)) {
            Slots::Range(0..1)
        } else {
            self.slots_for_face_iter(face)
        }
    }
}

/// The most of `stack`'s item slot `slot` of `view` holds, as `tryMoveInItem`
/// fills it.
fn insert_max(c: &super::Containers<'_>, view: &View, slot: usize, stack: &ItemStack) -> u32 {
    #[cfg(feature = "ae2-lab")]
    if let Some((_, max)) = c.ae2_slot(view, slot).and_then(|(kind, index)| super::ae2_capabilities::ae2_slot_insert(kind, index, stack)) {
        return max;
    }
    let _ = (view, slot);
    c.max_stack(stack)
}

impl Game<'_> {
    /// Vanilla containers and `WorldlyContainerHolder`s keep their direct automation path.
    /// This owner check borrows storage; it never builds a container view on the tick path.
    pub(super) fn vanilla_automation_container(&self, pos: Pos) -> bool {
        use horizon_client::containers::BlockEntity;
        match self.world.block_entities.get(&pos) {
            Some(
                BlockEntity::Mod(_)
                | BlockEntity::Beehive(_)
                | BlockEntity::Campfire(_)
                | BlockEntity::Brushable(_)
                | BlockEntity::EndPortal,
            ) => false,
            Some(entity) if !entity.slots().is_empty() => true,
            _ => matches!(
                self.res
                    .facts()
                    .get(self.world.block(pos.0, pos.1, pos.2))
                    .name
                    .as_ref(),
                "minecraft:composter" | "minecraft:decorated_pot"
            ),
        }
    }
    /// `HopperBlockEntity.inventoryFull`: every slot holds exactly a full item stack.
    pub(super) fn hopper_is_stacked(&self, view: &View) -> bool {
        let c = self.containers();
        (0..view.len()).all(|slot| {
            c.get(view, slot)
                .is_some_and(|stack| stack.count == c.max_stack(stack))
        })
    }
    /// `DropperBlock.dispenseFrom`: always commit a storage attempt, including refusal.
    /// Accepted source removal precedes root-commit callbacks that can re-enter the source.
    pub(super) fn dropper_into_part(
        &mut self,
        part: &mod_api::Capability<dyn mod_api::ResourceHandler<mod_api::ItemResource>>,
        stack: &ItemStack,
        at: [i32; 3],
        source: &View,
        slot: usize,
    ) -> bool {
        use crate::game::api_values::TryIntoApi;
        let Ok(snapshot) = stack.try_into_api() else {
            return false;
        };
        let resource = mod_api::ItemResource::of(&snapshot);
        mod_api::run_transaction(self, |game, tx| {
            let (pos, index) = source.locate(slot).ok_or(())?;
            game.journal_block_entity_slot(pos, index);
            // Reserve the offered item before writable insertion notices can re-enter.
            let reserved = game.take_one(source, slot).ok_or(())?;
            let went = part.insert_any(game, tx, &resource, 1);
            let accepted = part.checked(went, 1, &at) == Some(1);
            if !accepted {
                match game.slot_mut_at(source, slot).ok_or(())? {
                    Some(held) if held.same_item_same_components(&reserved) => {
                        held.count = held.count.checked_add(1).ok_or(())?;
                    }
                    empty @ None => *empty = Some(reserved),
                    // A writable notice replaced the slot. Only aborting can restore the
                    // reserved item without deleting that replacement; this exceptional
                    // refusal rolls back the notice's storage writes with the attempt.
                    Some(_) => return Err(()),
                }
            }
            Ok::<_, ()>(accepted)
        })
        .unwrap_or(false)
    }
    /// The level in place (`minecraft:overworld`), for a hook that isn't told
    /// which it runs in.
    pub(in crate::game) fn dimension(&self) -> &'static str {
        self.dim.id()
    }
    /// Whether this game simulates a world, as `native_set_block` asks
    /// (the fluid helpers ask it too).
    pub(in crate::game) fn native_simulates(&self) -> bool {
        self.is_logical_server()
    }
    /// The container at `pos` in the level in place, `dimension`, if this
    /// level simulates and holds items there. A composter's bone meal is for
    /// hoppers: it empties the block, and a refused move would lose it.
    fn native_item_source(&self, dimension: &str, pos: [i32; 3]) -> Option<View> {
        if !self.native_simulates() || dimension != self.dim.id() {
            return None;
        }
        let view = self.containers().view((pos[0], pos[1], pos[2]), false)?;
        (!matches!(view.kind, Kind::Composter(_)) && view.len() > 0).then_some(view)
    }
    /// The level in place, as a handle: where the game's capability lookups (a hopper's) are.
    /// None where no world is simulated.
    pub(super) fn native_level_here(&self) -> Option<LevelRef> {
        let epoch = self.native_lifecycle.epoch()?;
        self.native_simulates()
            .then(|| self.level_ref(epoch, self.dim.key()))
    }
    /// Up to `count` out of slot `slot` of `view`, without `setChanged`.
    fn take_up_to(&mut self, view: &View, slot: usize, count: u32) -> Option<ItemStack> {
        if matches!(view.kind, Kind::Composter(_)) {
            let (x, y, z) = view.parts[0].0;
            return self.composter_capability_extract([x, y, z], count);
        }
        let target = self.slot_mut_at(view, slot)?;
        let current = target.as_mut()?;
        let taken = count.min(current.count);
        let out = ItemStack {
            count: taken,
            ..current.clone()
        };
        current.count -= taken;
        if current.count == 0 {
            *target = None;
        }
        Some(out)
    }
    /// The container at `at` the item capability reaches, including the composter's
    /// state-backed ports. Inactive composter ports use its inert level-7 rules, so an input
    /// can never expose bone meal and an output can never accept compost.
    fn capability_view(&self, at: BlockSide) -> Option<View> {
        if !self.level_in_place(at.level) {
            return None;
        }
        let mut view = self
            .containers()
            .view((at.pos[0], at.pos[1], at.pos[2]), false)?;
        if let Kind::Composter(level) = view.kind {
            view.kind = Kind::Composter(match at.side {
                None | Some(Direction::Up) if level < 7 => level,
                Some(Direction::Down) if level == 8 => 8,
                None | Some(Direction::Up | Direction::Down) => 7,
                _ => return None,
            });
            return Some(view);
        }
        (view.len() > 0).then_some(view)
    }
    pub(in crate::game) fn container_set_stack_limit(
        &mut self,
        at: BlockSide,
        limit: u32,
        tx: &Transaction,
    ) -> bool {
        if !self.transaction_holds(tx) {
            return false;
        }
        if !(1..=99).contains(&limit) {
            return false;
        }
        let Some(view) = self.capability_view(at) else {
            return false;
        };
        if matches!(view.kind, Kind::Composter(_)) {
            return false;
        }
        for &(pos, _) in &view.parts {
            self.journal_block_entity(pos);
            self.world.set_container_stack_limit(pos, limit);
        }
        self.view_told(&view);
        true
    }
    /// How many slots `at`'s side reaches, including a composter's one-slot ports.
    /// None without supported storage, in a level not in place, or on a client's copy.
    pub(in crate::game) fn container_size(&self, at: BlockSide) -> Option<usize> {
        Some(self.capability_view(at)?.capability_slots(at.side).count())
    }
    /// The stack in slot `slot` of those `at`'s side reaches (`container_size`).
    pub(in crate::game) fn container_stack(&self, at: BlockSide, slot: usize) -> Option<ItemStack> {
        let view = self.capability_view(at)?;
        let slot = view.capability_slots(at.side).nth(slot)?;
        self.containers().get(&view, slot).cloned()
    }
    /// Keeps what a capability write to slot `slot` of `view` changes: that slot alone where a
    /// write there changes nothing else, else each block entity of the view whole.
    fn journal_view_slot(&mut self, view: &View, slot: usize) {
        if matches!(view.kind, Kind::Composter(_)) {
            return;
        }
        let slot_only = match view.kind {
            // Plain item slots: no state beside the slot changes. Vanilla containers defer
            // their implicit notice; mod-owned storage writes remain silent.
            Kind::Generic | Kind::Shulker | Kind::Mod => true,
            // State beside the slot: filling an empty input restarts a furnace's cooking; an
            // insert into an empty hopper sets its cooldown; a jukebox's disc starts its song;
            // a bookshelf's books are in its block state and last slot.
            Kind::Furnace | Kind::Hopper | Kind::Jukebox | Kind::Bookshelf => false,
            // Not shown to change only the slot (a crafter's disabled slots, a brewing stand's
            // brew), so kept whole as before.
            Kind::Crafter | Kind::Brewing => false,
            // A pot's item lives in saved data; composter writes use the block-state journal.
            Kind::Pot | Kind::Composter(_) => false,
        };
        if slot_only && let Some((pos, slot)) = view.locate(slot) {
            return self.journal_block_entity_slot(pos, slot);
        }
        for &(part, _) in &view.parts {
            self.journal_block_entity(part);
        }
    }
    /// Game-held vanilla containers coalesce their implicit notice at the root commit.
    fn view_told(&mut self, view: &View) {
        if matches!(view.kind, Kind::Composter(_) | Kind::Mod) {
            return;
        }
        for &(pos, _) in &view.parts {
            self.after_commit(After::Changed(pos));
        }
    }
    /// Puts as much of `stack` as slot `slot` of those `at`'s side reaches takes, as a hopper
    /// would (`canPlaceItem`, the slot's limit, the same item and components), in transaction
    /// `tx`; what didn't fit.
    pub(in crate::game) fn container_insert(
        &mut self,
        at: BlockSide,
        slot: usize,
        resource: &ItemStack,
        amount: u32,
        tx: &Transaction,
    ) -> Option<ItemStack> {
        let stack = ItemStack {
            count: amount,
            ..resource.clone()
        };
        if !self.transaction_holds(tx) {
            return Some(stack);
        }
        let Some(view) = self.capability_view(at) else {
            return Some(stack);
        };
        let Some(slot) = view.capability_slots(at.side).nth(slot) else {
            return Some(stack);
        };
        if matches!(view.kind, Kind::Composter(_)) {
            if amount == 0
                || self.containers().fits(&view, slot, &stack) == 0
                || !self.composter_capability_insert(at.pos, &stack)
            {
                return Some(stack);
            }
            return (amount > 1).then(|| ItemStack {
                count: amount - 1,
                ..stack
            });
        }
        let fits = self.containers().fits(&view, slot, &stack);
        if fits > 0 {
            let some = ItemStack {
                count: fits,
                ..stack.clone()
            };
            self.journal_view_slot(&view, slot);
            // A jukebox's disc starts at the root commit (`JukeboxBlockEntity.setTheItem`).
            if self.move_in_item(None, &view, some, slot).1 {
                self.view_told(&view);
            }
        }
        (fits < stack.count).then(|| ItemStack {
            count: stack.count - fits,
            ..stack
        })
    }
    /// How much of `resource` the slots `at`'s side reaches hold in all (`STORAGE`'s amount),
    /// in one pass over one view of the container.
    /// None where the total does not fit in 64 bits.
    pub(in crate::game) fn container_kind_amount(
        &self,
        at: BlockSide,
        resource: &ItemStack,
    ) -> Option<u64> {
        let Some(view) = self.capability_view(at) else {
            return Some(0);
        };
        let c = self.containers();
        view.capability_slots(at.side)
            .filter_map(|slot| c.get(&view, slot))
            .filter(|held| held.same_item_same_components(resource))
            .try_fold(0u64, |total, held| total.checked_add(u64::from(held.count)))
    }
    /// How much of `resource` an insert through `at`'s side takes now: each slot's room for it,
    /// by the rules `container_insert` follows, summed in one pass.
    /// None where the total does not fit in 64 bits.
    pub(in crate::game) fn container_kind_space(
        &self,
        at: BlockSide,
        resource: &ItemStack,
    ) -> Option<u64> {
        let Some(view) = self.capability_view(at) else {
            return Some(0);
        };
        let probe = ItemStack {
            count: u32::MAX,
            ..resource.clone()
        };
        let c = self.containers();
        view.capability_slots(at.side)
            .try_fold(0u64, |total, slot| {
                total.checked_add(u64::from(c.fits(&view, slot, &probe)))
            })
    }
    /// What the slots `at`'s side reaches hold, from its `from`th to before its
    /// `from + limit`th (fewer at the end), empty ones included: never more than
    /// `MAX_STORAGE_PAGE` slots, whatever `limit` asks, so a request cannot make the game
    /// allocate more than one page.
    pub(in crate::game) fn container_listing(
        &self,
        at: BlockSide,
        from: usize,
        limit: usize,
    ) -> Option<Vec<Option<ItemStack>>> {
        let view = self.capability_view(at)?;
        let c = self.containers();
        Some(
            view.capability_slots(at.side)
                .skip(from)
                .take(limit.min(mod_api::MAX_STORAGE_PAGE))
                .map(|slot| c.get(&view, slot).cloned())
                .collect(),
        )
    }
    /// The slots of the container at `pos` that its side `face` reaches, with
    /// their stacks (the fork's API 48; upstream's `container_listing` reads a
    /// side in a transaction). None where no container is: another dimension,
    /// a chunk that isn't loaded, a block without slots, a composter, or the
    /// mod block entity ticking now. A double chest's slots are both halves'.
    pub(in crate::game) fn native_container_slots(
        &self,
        dimension: &str,
        pos: [i32; 3],
        face: Direction,
    ) -> Option<Vec<NativeContainerSlot>> {
        let view = self.native_item_source(dimension, pos)?;
        let c = self.containers();
        Some(
            view.slots_for_face_iter(Some(face))
                .map(|slot| {
                    let stack = c.get(&view, slot);
                    NativeContainerSlot {
                        slot,
                        stack: stack.cloned(),
                        can_take: stack.is_some_and(|s| c.can_take(&view, slot, s, face)),
                    }
                })
                .collect(),
        )
    }
    /// Takes up to `count` items out of slot `slot` of the container at `pos`,
    /// through its side `face`; they're the mod's to place then, as a hopper's
    /// are. None when nothing can be taken: see `native_container_slots`. The
    /// fork's API 48 (upstream's `container_extract` takes in a transaction).
    pub(in crate::game) fn native_extract_item(
        &mut self,
        dimension: &str,
        pos: [i32; 3],
        face: Direction,
        slot: usize,
        count: u32,
    ) -> Option<ItemStack> {
        let view = self.native_item_source(dimension, pos)?;
        if count == 0 || !view.slots_for_face_iter(Some(face)).any(|s| s == slot) {
            return None;
        }
        let c = self.containers();
        if !c.can_take(&view, slot, c.get(&view, slot)?, face) {
            return None;
        }
        let out = self.take_up_to(&view, slot, count)?;
        self.view_changed(&view);
        Some(out)
    }
    /// Puts `stack` into the container at `pos` (a chest, a furnace, a mod
    /// container) through its side `face`, as a hopper would, one by one, and
    /// returns what did not fit: a machine's auto-output. A composter takes
    /// one a call, and a client's copy of the world none. The fork's API 20
    /// (upstream's `container_insert` puts in a transaction).
    pub(in crate::game) fn native_insert_item(
        &mut self,
        pos: [i32; 3],
        face: Direction,
        stack: ItemStack,
    ) -> Option<ItemStack> {
        if !self.native_simulates() {
            return Some(stack);
        }
        let Some(view) = self.containers().view((pos[0], pos[1], pos[2]), false) else {
            return Some(stack);
        };
        // `tryMoveInItem` puts all it's given in an empty slot, and a hopper
        // gives it one item, so a bigger stack goes in one by one or it would
        // pass the slot's limit.
        let slots: Vec<usize> = view.slots_to_add_iter(Some(face)).collect();
        let mut left = stack.count;
        while left > 0 {
            let one = ItemStack {
                count: 1,
                ..stack.clone()
            };
            if self.add_one_quietly(&view, &slots, one, face).is_some() {
                break;
            }
            left -= 1;
            // A composter's `InputContainer` holds one item.
            if matches!(view.kind, Kind::Composter(_)) {
                break;
            }
        }
        if left < stack.count {
            self.view_changed(&view);
        }
        (left > 0).then_some(ItemStack {
            count: left,
            ..stack
        })
    }
    /// One item into `view` through `face`, trying `slots`, without
    /// `setChanged`: the caller tells the container once it's done. The item,
    /// when nothing took it.
    fn add_one_quietly(
        &mut self,
        view: &View,
        slots: &[usize],
        one: ItemStack,
        face: Direction,
    ) -> Option<ItemStack> {
        if let Kind::Composter(_) = view.kind {
            // A composter takes from above only (`ComposterBlock.InputContainer`).
            let (x, y, z) = view.parts[0].0;
            let taken = face == Direction::Up && self.composter_capability_insert([x, y, z], &one);
            return (!taken).then_some(one);
        }
        let mut rest = Some(one);
        for &slot in slots {
            let Some(one) = rest.take() else { break };
            rest = self.move_in_item(None, view, one, slot).0;
        }
        rest
    }
    /// The slots of the container at `pos` that `native_insert_item` puts a
    /// stack into through its side `face` (`HopperBlockEntity.addItem`'s, in
    /// that order), with what each holds, whether it takes `stack` and up to
    /// how many of it: what an `IItemHandler` of that side tells by
    /// `isItemValid` and `getSlotLimit`, for a mod to work out what a
    /// container will take before it sends anything. None where no container
    /// is (as `native_container_slots`, and for a composter). API 425.
    pub(in crate::game) fn native_insert_slots(
        &self,
        dimension: &str,
        pos: [i32; 3],
        face: Direction,
        stack: &ItemStack,
    ) -> Option<Vec<NativeInsertSlot>> {
        let view = self.native_item_source(dimension, pos)?;
        let c = self.containers();
        Some(
            view.slots_to_add_iter(Some(face))
                .map(|slot| NativeInsertSlot {
                    slot,
                    stack: c.get(&view, slot).cloned(),
                    accepts: c.can_place(&view, slot, stack),
                    max: insert_max(&c, &view, slot, stack),
                })
                .collect(),
        )
    }
    /// Puts up to `amount` of `resource` into the slots `at`'s side reaches, those holding it
    /// first and then the empty ones, in transaction `tx`, on one view of the container: what
    /// went in. Each slot is kept and filled as `container_insert` keeps and fills it.
    pub(in crate::game) fn container_insert_kind(
        &mut self,
        at: BlockSide,
        resource: &ItemStack,
        amount: u64,
        tx: &Transaction,
    ) -> u64 {
        if amount == 0 || !self.transaction_holds(tx) {
            return 0;
        }
        let Some(view) = self.capability_view(at) else {
            return 0;
        };
        if matches!(view.kind, Kind::Composter(_)) {
            return u64::from(
                self.containers().capacity(&view, 0, resource) > 0
                    && self.composter_capability_insert(at.pos, resource),
            );
        }
        let (mut left, mut told) = (amount, false);
        for holding in [true, false] {
            for slot in view.capability_slots(at.side) {
                if left == 0 {
                    break;
                }
                let pick = match self.containers().get(&view, slot) {
                    Some(held) => holding && held.same_item_same_components(resource),
                    None => !holding,
                };
                if !pick {
                    continue;
                }
                let stack = ItemStack {
                    count: left.min(u64::from(u32::MAX)) as u32,
                    ..resource.clone()
                };
                let fits = self.containers().fits(&view, slot, &stack);
                if fits == 0 {
                    continue;
                }
                self.journal_view_slot(&view, slot);
                let some = ItemStack {
                    count: fits,
                    ..resource.clone()
                };
                if self.move_in_item(None, &view, some, slot).1 {
                    left -= u64::from(fits);
                    told = true;
                }
            }
        }
        if told {
            self.view_told(&view);
        }
        amount - left
    }
    /// Takes up to `amount` of `resource` out of the slots `at`'s side reaches and may take
    /// from, in transaction `tx`, on one view of the container: what came out.
    pub(in crate::game) fn container_extract_kind(
        &mut self,
        at: BlockSide,
        resource: &ItemStack,
        amount: u64,
        tx: &Transaction,
    ) -> u64 {
        if amount == 0 || !self.transaction_holds(tx) {
            return 0;
        }
        let Some(view) = self.capability_view(at) else {
            return 0;
        };
        let (mut left, mut told) = (amount, false);
        for slot in view.capability_slots(at.side) {
            if left == 0 {
                break;
            }
            let c = self.containers();
            let Some(held) = c.get(&view, slot) else {
                continue;
            };
            if !held.same_item_same_components(resource)
                || at
                    .side
                    .is_some_and(|face| !c.can_take(&view, slot, held, face))
            {
                continue;
            }
            self.journal_view_slot(&view, slot);
            let ask = left.min(u64::from(u32::MAX)) as u32;
            if let Some(out) = self.take_up_to(&view, slot, ask) {
                left -= u64::from(out.count);
                told = true;
            }
        }
        if told {
            self.view_told(&view);
        }
        amount - left
    }
    /// How many of `stack` slot `slot` of those `at`'s side reaches holds when empty, as
    /// NeoForge's `isValid` and `getCapacityAsLong` answer: none where it may not go in (a
    /// furnace's output, `canPlaceItem`), else the slot's limit for it.
    pub(in crate::game) fn container_capacity(
        &self,
        at: BlockSide,
        slot: usize,
        stack: &ItemStack,
    ) -> u32 {
        let Some(view) = self.capability_view(at) else {
            return 0;
        };
        view.capability_slots(at.side)
            .nth(slot)
            .map_or(0, |slot| self.containers().capacity(&view, slot, stack))
    }
    /// Takes up to `max` out of slot `slot` of those `at`'s side reaches, in transaction `tx`;
    /// through a side, only what that side may take (`canTakeItemThroughFace`). What came out.
    pub(in crate::game) fn container_extract(
        &mut self,
        at: BlockSide,
        slot: usize,
        max: u32,
        tx: &Transaction,
    ) -> Option<ItemStack> {
        if !self.transaction_holds(tx) {
            return None;
        }
        let view = self.capability_view(at)?;
        let slot = view.capability_slots(at.side).nth(slot)?;
        let c = self.containers();
        let stack = c.get(&view, slot)?;
        if max == 0
            || at
                .side
                .is_some_and(|face| !c.can_take(&view, slot, stack, face))
        {
            return None;
        }
        self.journal_view_slot(&view, slot);
        let out = self.take_up_to(&view, slot, max)?;
        // A jukebox's song stops at the root commit (`JukeboxBlockEntity.removeItem`).
        self.view_told(&view);
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use horizon_client::containers::ModContainer;
    use horizon_core::geometry::Direction;
    use std::sync::Arc;

    fn view(kind: Kind, slots: usize) -> View {
        View {
            parts: vec![((0, 0, 0), slots)],
            kind,
            container: (kind == Kind::Mod).then(|| {
                Arc::new(ModContainer {
                    slots,
                    ..Default::default()
                })
            }),
        }
    }

    #[test]
    fn a_face_only_narrows_the_slots_of_a_worldly_container() {
        assert_eq!(
            view(Kind::Furnace, 3).slots_to_add(Some(Direction::Up)),
            [0]
        );
        assert_eq!(
            view(Kind::Generic, 2).slots_to_add(Some(Direction::Up)),
            [0, 1]
        );
        assert_eq!(
            view(Kind::Shulker, 2).slots_to_add(Some(Direction::Up)),
            [0, 1]
        );
        assert_eq!(view(Kind::Furnace, 3).slots_to_add(None), [0, 1, 2]);
    }
}

#[cfg(test)]
#[path = "storage_attempt_tests.rs"]
mod storage_attempt_tests;

#[cfg(test)]
#[path = "storage_attempt_wasm_tests.rs"]
mod storage_attempt_wasm_tests;
