//! The extension point of the fork's AE2 crates above horizon-client (docs/ae2-crates.md): what
//! the game calls on them ([`Ae2Ext`], [`Ae2Addon`]) and what they may read and do in it
//! ([`Ae2Host`], implemented for `Game` in `ae2_host.rs`). It names no type from above
//! (docs/layers.md). The program that has them (horizon-atm10) installs a factory before the game
//! starts, and every `Game` makes its own extension with it.
//!
//! Calls across are `dyn` only, both ways: no generic function and no `impl Trait` parameter on
//! either side, so none of the crates' code is compiled into horizon-client.
//!
//! The game holds its extension shared, not taken out for a call: while an addon runs, what it asks
//! of the host (the AE2 grid it builds...) may ask the addons again, this one included. So the addons'
//! methods take `&self`, and what an addon keeps is behind a `RefCell` (borrowed for a read or a
//! write, never across a call to the host).
pub use super::{ae2_global::GridAddress, ae2_parts::PartAddress, ae2_visual_sync::Visual, ae2x::HandlerCell};
use horizon_client::{ae2_world::Machine, containers::BlockEntity, items::ItemStack, streaming::Streaming, text::Text, world::World};
use std::{path::Path, rc::Rc, sync::OnceLock};

/// What the AE2 crates above may read and do in the game.
pub trait Ae2Host {
    /// Whether this game is a client of a server (`Game::remote`), whose world it only shows.
    fn is_remote(&self) -> bool;
    /// The dimension of the level the game holds (`minecraft:overworld`...).
    fn dimension(&self) -> &str;
    /// That level's world.
    fn world(&self) -> &World;
    /// The chunk loader, whose `forced` chunks stay loaded.
    fn streaming(&mut self) -> &mut Streaming;
    /// The folder of the world the game has open (`Game::world_root`): an addon's own data is saved in it.
    fn world_root(&self) -> &Path;
    /// Whether a cell may name the key `id` of `kind` (`ae2_known_cell_key`).
    fn known_cell_key(&self, id: &str, kind: ae2_cell_core::KeyType) -> bool;
    /// The block entity at a grid address of a loaded level (`ae2_block_entity`).
    fn block_entity(&self, address: GridAddress) -> Option<&BlockEntity>;
    /// The dimension's id and the position of a grid address (`ae2_address_parts`).
    fn address_parts(&self, address: GridAddress) -> Option<(&'static str, (i32, i32, i32))>;
    /// The grid address of a block of the level the game holds (`ae2_local_address`).
    fn local_address(&self, pos: (i32, i32, i32)) -> Option<GridAddress>;
    /// A Drive's or ME Chest's priority (`ae2_storage_priority`).
    fn storage_priority(&self, address: GridAddress) -> Result<i32, String>;
    /// The machine of a cable part (`ae2_part_machine`).
    fn part_machine(&self, part: PartAddress) -> Result<Option<Machine>, String>;
    /// The index of a face name, as a part keeps it (`face_index`).
    fn face_index(&self, face: &str) -> Option<u8>;
    /// The addresses of the AE2 machines of every loaded level that `wanted` takes, in the order of
    /// the levels and of their block entities.
    fn machine_addresses(&self, wanted: &dyn Fn(&Machine) -> bool) -> Vec<GridAddress>;
    /// The online cell hosts of the grid of `anchor`: its Drives and ME Chests, and its cell parts
    /// (`ae2_cell_hosts_encoded`).
    fn cell_hosts(&self, anchor: GridAddress) -> Result<(Vec<GridAddress>, Vec<PartAddress>), String>;
    /// The local player's UUID, as servers know it (`Game::own_uuid`).
    fn local_player_uuid(&self) -> u128;
    /// The UUID, as four ints, of the player acting: the guest a command runs for, else the host.
    fn acting_player_uuid(&self) -> [i32; 4];
    /// The name to join servers with (`options.player_name()`).
    fn join_name(&self) -> String;
    /// The stack in the selected hotbar slot, the main hand's.
    fn main_hand_stack(&self) -> Option<&ItemStack>;
    /// Puts `stack` in the selected hotbar slot.
    fn set_main_hand_stack(&mut self, stack: ItemStack);
}

/// What an addon's own storage was when AE2's blocks were snapshot (`ae2_snapshot_blocks`):
/// [`Ae2Snapshot::restore`] puts it back, as a transaction that fails or only simulates does;
/// dropped without it, the changes stay.
pub trait Ae2Snapshot {
    fn restore(self: Box<Self>);
}

/// One addon's game behaviour (a `horizon-ae2x-<id>` crate), with what it keeps while the game
/// runs. The game calls each method where it called the addon's code before it moved; an addon
/// leaves a method alone when it has nothing there.
pub trait Ae2Addon {
    /// The addon's id (pack/pack.toml's).
    fn id(&self) -> &'static str;
    /// A running game tick, in the addon's place in `tick_ae2x`.
    fn tick(&self, _host: &mut dyn Ae2Host) {}
    /// The same, while the server's ticks are frozen (`tick_ae2x_frozen`).
    fn tick_frozen(&self, _host: &mut dyn Ae2Host) {}
    /// Its own storage as it is now, when AE2's blocks are snapshot (`ae2x_snapshot`).
    fn snapshot(&self, _host: &dyn Ae2Host) -> Option<Box<dyn Ae2Snapshot>> {
        None
    }
    /// Its own data of the world, written with the world's save (`ae2x_save`).
    fn save(&self, _host: &mut dyn Ae2Host) {}
    /// Whether the cell in slot `slot` of the Drive or ME Chest at `at` is mounted on the grid whose
    /// online cell hosts are `hosts` (`ae2x_cell_mounted`).
    fn cell_mounted(&self, _host: &dyn Ae2Host, _at: GridAddress, _slot: usize, _stack: &ItemStack, _hosts: &[GridAddress]) -> bool {
        true
    }
    /// The idle drain of a cell of its own that changes with what it holds (`ae2x_cell_idle_drain`).
    fn cell_idle_drain(&self, _host: &dyn Ae2Host, _at: GridAddress, _slot: usize, _stack: &ItemStack) -> Option<f64> {
        None
    }
    /// What a block's visual shows of its cells that the stack alone does not tell (`ae2x_enrich_visual`).
    fn enrich_visual(&self, _host: &dyn Ae2Host, _visual: &mut Visual) {}
    /// A cell of its own handler (`ae2x_handler_cell`); none for any other stack.
    fn handler_cell(&self, _host: &dyn Ae2Host, _stack: &ItemStack) -> Option<Result<Box<dyn HandlerCell>, String>> {
        None
    }
    /// What its item adds to the tooltip below the name (`ae2x_item_hover_lines`).
    fn item_hover_lines(&self, _host: &dyn Ae2Host, _stack: &ItemStack, _translate: &dyn Fn(&str) -> Option<String>) -> Option<Vec<Text>> {
        None
    }
    /// `/ae2 <addon> ...` (`ae2x_command`): none when the words are not its.
    fn command(&self, _host: &mut dyn Ae2Host, _words: &[&str]) -> Option<Result<serde_json::Value, String>> {
        None
    }
    /// A custom payload of its channel from a LAN guest, that acts on the stack in the guest's
    /// selected slot for the player `player` (a UUID string): whether it changed the stack.
    fn lan_main_hand_payload(&self, _channel: &str, _data: &[u8], _player: &str, _stack: &mut ItemStack) -> bool {
        false
    }
}

/// What the game calls on the AE2 crates above: one per `Game`, with their state.
pub trait Ae2Ext {
    /// The addon `id`, when it is built in.
    fn addon(&self, id: &str) -> Option<&dyn Ae2Addon>;
}

/// Makes one game's extension.
pub type Factory = fn() -> Box<dyn Ae2Ext>;

static FACTORY: OnceLock<Factory> = OnceLock::new();

/// Installs the extension's factory. The program calls it once, before the game starts; the first
/// install holds.
pub fn install(factory: Factory) {
    let _ = FACTORY.set(factory);
}

/// A game's extension, made with the game; none when nothing is installed. A call clones the
/// handle, so the game stays whole for the call, and so does the extension for what it asks back.
pub(super) struct Slot(Option<Rc<dyn Ae2Ext>>);

impl Default for Slot {
    fn default() -> Self {
        Self(FACTORY.get().map(|make| Rc::from(make())))
    }
}

impl Slot {
    pub(super) fn get(&self) -> Option<Rc<dyn Ae2Ext>> {
        self.0.clone()
    }
}
