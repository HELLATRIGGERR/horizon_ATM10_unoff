//! A mod block's use beyond its template, for the native bundles: a fluid container used on a
//! block whose `useItemOn` starts with NeoForge's `FluidUtil.interactWithFluidHandler`
//! (`fluid_interact_on_use`), and the menu of a block entity's container on a block whose
//! template opens none. `items/use_hand.rs` runs them on the server (and in-frame), and
//! `network/remote/interaction_prediction.rs` predicts them on a server's client.
use super::Game;
use horizon_world::registry::BlockStateId;
use mod_api::{BlockClick, InteractionResult};

impl Game<'_> {
    /// Whether a bundle says the block of `state` takes fluid containers on use
    /// (`fluid_interact_on_use`).
    pub(in crate::game) fn fluid_interact_on_use(&self, state: BlockStateId) -> bool {
        let mut takes = false;
        let block = &self.res.facts().get(state).name;
        #[cfg(test)]
        crate::game::fork_compat::test_bundle::fluid_interact_on_use(block, &mut takes);
        self.native_fluid_interact_on_use(block, &mut takes);
        takes
    }

    /// The block's `useItemOn` hook point (`native_use_item_on`), once its gate opened. A unit
    /// test's bundle answers first (a hook point's body takes no `cfg`: horizon-mixins refuses it).
    pub(in crate::game) fn block_use_item_on_hook(
        &mut self,
        state: BlockStateId,
        click: &BlockClick<'_>,
        answer: &mut Option<InteractionResult>,
    ) {
        #[cfg(test)]
        crate::game::fork_compat::test_bundle::use_item_on(&self.res.facts().get(state).name, answer);
        self.native_use_item_on(state, click, answer);
    }

    /// The block's `useWithoutItem` hook point (`native_use_without_item`), as
    /// [`Self::block_use_item_on_hook`].
    pub(in crate::game) fn block_use_without_item_hook(
        &mut self,
        state: BlockStateId,
        click: &BlockClick<'_>,
        answer: &mut Option<InteractionResult>,
    ) {
        #[cfg(test)]
        crate::game::fork_compat::test_bundle::use_without_item(&self.res.facts().get(state).name, answer);
        self.native_use_without_item(state, click, answer);
    }

    /// Whether the block of `state` has a block entity container with a menu
    /// (`native_block_entity_container`): its use opens that menu when its template's passes.
    pub(in crate::game) fn mod_menu_block(&self, state: BlockStateId) -> bool {
        self.world
            .block_entities
            .mod_container(state.0)
            .is_some_and(|c| c.menu.is_some())
    }

    /// Whether the block at `pos` opens a mod container's menu on use: a block of
    /// [`Self::mod_menu_block`] whose mod block entity is there (`open_mod_menu` needs it).
    pub(in crate::game) fn mod_menu_opens(&self, pos: [i32; 3]) -> bool {
        self.mod_menu_block(self.world.block(pos[0], pos[1], pos[2]))
            && matches!(
                self.world.block_entities.get(&(pos[0], pos[1], pos[2])),
                Some(horizon_client::containers::BlockEntity::Mod(_))
            )
    }
}
