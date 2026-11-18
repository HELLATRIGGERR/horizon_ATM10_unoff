use super::*;

impl Game<'_> {
    /// Slot `slot` of block `block`'s container is offered `stack`, by a hopper or a click in its
    /// menu (`Container.canPlaceItem`, `Slot.mayPlace`): clear `accepts` when the block is yours
    /// and refuses it. `fuel`: the stack burns in a furnace. It only narrows what the container's
    /// `inputs` let through, and asks neither for what a mod's own tick does nor for the stacks
    /// already in a slot. Asked often, from any thread's menu or hopper: answer from the arguments
    /// alone.
    pub(in crate::game) fn native_block_entity_slot_accepts(
        block: &str,
        slot: usize,
        stack: &ItemStack,
        fuel: bool,
        accepts: &mut bool,
    ) {
        Self::fire_block_entity_slot_accepts(block, slot, stack, fuel, accepts);
    }
}
