use super::*;

impl Game<'_> {
    /// Block `block`'s block entity is breaking, and drops the block's loot: put into `components`
    /// what the item it drops keeps (`copy_components` from a block entity in vanilla's block loot
    /// tables), as a component patch (`mymod:energy`). `by_player`: a player's tool breaks it
    /// (`Block.playerDestroy`), not an explosion, a command or a piston. A dropped stack of the
    /// block's own item takes them, unless the block is broken without drops. `native_place_block`
    /// hears the item placed again. Read `entity` alone: it runs from a level's own pass.
    pub(in crate::game) fn native_block_entity_drop(
        block: &str,
        entity: &ModBlockEntity,
        by_player: bool,
        components: &mut Compound,
    ) {
        Self::fire_block_entity_drop(block, entity, by_player, components);
    }
}
