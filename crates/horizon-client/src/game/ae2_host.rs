//! `Game` as the host of the AE2 crates above horizon-client (`ae2_ext::Ae2Host`): the one place
//! they reach the game, and the game's calls into them.
use super::{Game, ae2_ext::Ae2Host};
use horizon_client::{streaming::Streaming, world::World};

impl Ae2Host for Game<'_> {
    fn is_remote(&self) -> bool {
        self.remote.is_some()
    }

    fn dimension(&self) -> &str {
        self.dim.id()
    }

    fn world(&self) -> &World {
        &self.world
    }

    fn streaming(&mut self) -> &mut Streaming {
        &mut self.streaming
    }
}

impl Game<'_> {
    /// The tick of the addon `id` (`Ae2Addon::tick`), when it is built in.
    pub(super) fn ae2_addon_tick(&mut self, id: &str) {
        let Some(mut ext) = self.ae2_ext.take() else {
            return;
        };
        if let Some(addon) = ext.addon(id) {
            addon.tick(self);
        }
        self.ae2_ext.put_back(ext);
    }
}
