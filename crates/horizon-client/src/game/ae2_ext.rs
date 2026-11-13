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
/// The addons' static rules, for questions with no game at hand: `horizon_client::ae2x::rules`.
pub use horizon_client::ae2x::rules::{AddonRules, install_rules, rules};
use horizon_client::{
    ae2_screen_style::ScreenStyle,
    ae2_world::Machine,
    containers::BlockEntity,
    hud::HudBatch,
    items::ItemStack,
    streaming::Streaming,
    text::Text,
    world::World,
};
use horizon_world::registry::{BlockRegistry, BlockStateId};
use std::{
    any::Any,
    path::Path,
    rc::Rc,
    sync::{Mutex, OnceLock, PoisonError},
};

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
    /// Calls `what` of the addon `id` ([`Ae2Addon::call`]): how one addon above reaches another.
    /// None when that addon is not built in or does not know `what`.
    fn call_addon(&mut self, id: &str, what: &str, args: &dyn Any) -> Option<Box<dyn Any>>;
    /// The level's world, to change.
    fn world_mut(&mut self) -> &mut World;
    /// Whether the acting player is a spectator.
    fn spectator(&self) -> bool;
    /// Whether the acting player sneaks (left Shift held).
    fn shift_down(&self) -> bool;
    /// The stack in the hand that acts (`Game::held`).
    fn held(&self) -> Option<&ItemStack>;
    /// Uses one of the held stack up, none in creative (`Game::consume_held`).
    fn consume_held(&mut self);
    /// The block id of a block state.
    fn block_state_name(&self, state: BlockStateId) -> &str;
    /// The block states of the pack.
    fn block_registry(&self) -> &'static BlockRegistry;
    /// Sets a block of the level as the game does (`Game::level_set`).
    fn level_set(&mut self, pos: [i32; 3], state: BlockStateId) -> Result<(), String>;
    /// A pattern provider's patterns changed (`ae2_provider_patterns_changed`).
    fn provider_patterns_changed(&self, address: GridAddress, face: Option<u8>);
    /// The same, for a provider block of the level the game holds (`ae2_provider_patterns_changed_local`).
    fn provider_patterns_changed_local(&self, pos: (i32, i32, i32), face: Option<u8>);
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
    /// Its own window ([`Ae2AddonScreen`]); none when it has none.
    fn screen(&self) -> Option<&dyn Ae2AddonScreen> {
        None
    }
    /// Its buttons and sub-screens on AE2's terminals ([`Ae2TerminalWidgets`]).
    fn terminal(&self) -> Option<&dyn Ae2TerminalWidgets> {
        None
    }
    /// The world strategies of the key types it registered with [`add_key_strategies`].
    fn key_strategies(&self) -> Option<&dyn Ae2KeyStrategies> {
        None
    }
    /// What another addon asks of it by name ([`Ae2Host::call_addon`]), with arguments and an
    /// answer whose types both sides know. None when it does not know `what`.
    fn call(&self, _host: &mut dyn Ae2Host, _what: &str, _args: &dyn Any) -> Option<Box<dyn Any>> {
        None
    }
}

/// An addon's own window, neither a machine's nor a terminal's (Advanced AE's encoder): the game
/// asks the addons in their places in `game/ae2x/mod.rs`'s `ae2x_screen_*` chain, and the first
/// that answers owns the window. Every method answers "not mine" by default.
pub trait Ae2AddonScreen {
    /// The window's size, while it is open (`ae2x_screen_size`).
    fn size(&self, _host: &dyn Ae2Host) -> Option<(f32, f32)> {
        None
    }
    /// The menu slots it shows (`ae2x_slots`).
    fn slots(&self, _host: &dyn Ae2Host) -> Option<Vec<usize>> {
        None
    }
    /// Where slot `i` is, from the window's corner (`ae2x_slot_position`).
    fn slot_position(&self, _host: &dyn Ae2Host, _i: usize) -> Option<(f32, f32)> {
        None
    }
    /// Draws the window at `ox, oy`: whether it did (`ae2x_draw`).
    fn draw(&self, _host: &dyn Ae2Host, _batch: &mut HudBatch, _ox: f32, _oy: f32) -> bool {
        false
    }
    /// A press at `at`: whether it took it (`ae2x_press`).
    fn press(&self, _host: &mut dyn Ae2Host, _at: [f32; 2], _right: bool) -> bool {
        false
    }
    /// A scroll: whether it took it (`ae2x_scroll`).
    fn scroll(&self, _host: &mut dyn Ae2Host, _delta: f32) -> bool {
        false
    }
    /// Draws the hovered element's tooltip: whether it did (`ae2x_tooltip`).
    fn tooltip(&self, _host: &dyn Ae2Host, _batch: &mut HudBatch) -> bool {
        false
    }
    /// A hotkey `input` while it is open: whether it took it (`ae2x_screen_hotkey`).
    fn hotkey(&self, _host: &mut dyn Ae2Host, _input: &str) -> bool {
        false
    }
    /// A server payload on `channel`: whether it was its own (`ae2x_client_payload`).
    fn payload(&self, _host: &mut dyn Ae2Host, _channel: &str, _data: &[u8]) -> bool {
        false
    }
    /// What `horizon-ctl state` reports of the window (`ae2x_agent_value`).
    fn agent(&self, _host: &dyn Ae2Host) -> Option<serde_json::Value> {
        None
    }
}

/// An addon's buttons on AE2's terminals and the sub-screens they open (AE2 Import Export Card's
/// card screens). The terminal's `Widget` and `Sub` carry the addon's id and its own number
/// `which`. Every method answers "not mine" by default.
pub trait Ae2TerminalWidgets {
    /// Its buttons on the open terminal, by number, in its order.
    fn buttons(&self, _host: &dyn Ae2Host) -> Vec<u8> {
        Vec::new()
    }
    /// Draws button `which` in `rect` (`[x, y, w, h]` from `ox, oy`).
    #[allow(clippy::too_many_arguments)]
    fn draw_button(&self, _host: &dyn Ae2Host, _batch: &mut HudBatch, _ox: f32, _oy: f32, _rect: [i32; 4], _which: u8, _mouse: [f32; 2]) {}
    /// Button `which`'s tooltip.
    fn button_tooltip(&self, _host: &dyn Ae2Host, _which: u8) -> String {
        String::new()
    }
    /// Button `which` pressed: opens its sub-screen.
    fn open(&self, _host: &mut dyn Ae2Host, _which: u8) {}
    /// Its sub-screen that is open, by number.
    fn sub(&self, _host: &dyn Ae2Host) -> Option<u8> {
        None
    }
    /// Sub-screen `which`'s style (`ae2_term_sub_style`).
    fn sub_style(&self, _host: &dyn Ae2Host, _which: u8) -> Option<&'static ScreenStyle> {
        None
    }
    /// Sub-screen `which`'s size, when it is not its style's (`ae2_term_sub_size`).
    fn sub_size(&self, _host: &dyn Ae2Host, _which: u8) -> Option<(i32, i32)> {
        None
    }
    /// Where menu slot `i` is on sub-screen `which`.
    fn sub_slot_position(&self, _host: &dyn Ae2Host, _which: u8, _i: usize) -> Option<(f32, f32)> {
        None
    }
    /// A press on sub-screen `which`, right presses too: whether it took it.
    fn sub_press(&self, _host: &mut dyn Ae2Host, _which: u8, _at: [f32; 2], _right: bool) -> bool {
        false
    }
    /// Draws sub-screen `which` at `ox, oy`.
    fn sub_draw(&self, _host: &dyn Ae2Host, _batch: &mut HudBatch, _which: u8, _ox: f32, _oy: f32) {}
    /// Draws sub-screen `which`'s foreground.
    fn sub_draw_foreground(&self, _host: &dyn Ae2Host, _batch: &mut HudBatch, _which: u8, _ox: f32, _oy: f32) {}
    /// The tooltip lines of what is under `at` on sub-screen `which`.
    fn sub_tooltip(&self, _host: &dyn Ae2Host, _which: u8, _at: [f32; 2]) -> Option<Vec<Text>> {
        None
    }
    /// Draws its marks over the inventory's items: whether it did.
    fn draw_over(&self, _host: &mut dyn Ae2Host, _batch: &mut HudBatch, _which: u8) -> bool {
        false
    }
    /// What `horizon-ctl state` reports of sub-screen `which`.
    fn sub_agent(&self, _host: &dyn Ae2Host, _which: u8, _origin: (f32, f32)) -> serde_json::Value {
        serde_json::Value::Null
    }
}

/// An addon key type in the world (AE2's `StackWorldBehaviors`: import, export, external storage,
/// placement), for the types its owner registered with [`add_key_strategies`]. The buses and the
/// storage ask the owner where they ask an addon type's strategies (`ae2_key_strategies::addon`).
/// `face` is a direction's index (`Direction::index`).
#[allow(clippy::too_many_arguments)]
pub trait Ae2KeyStrategies {
    /// What an import bus at `at` may take from its side `face` (`ImportStrategy::offered`).
    fn import_offered(&self, _host: &dyn Ae2Host, _at: GridAddress, _face: u8) -> Vec<(ae2_cell_core::ItemKey, u64)> {
        Vec::new()
    }
    /// Moves up to `amount` of `key` from beside `at` into `target`'s grid (`ImportStrategy::transfer`).
    fn import(&self, _host: &mut dyn Ae2Host, _at: GridAddress, _target: GridAddress, _face: u8, _key: &ae2_cell_core::ItemKey, _amount: u64) -> Result<u64, String> {
        Ok(0)
    }
    /// Moves up to `amount` of `key` from `source`'s grid beside `at` (`ExportStrategy::transfer`).
    fn export(&self, _host: &mut dyn Ae2Host, _at: GridAddress, _source: GridAddress, _face: u8, _key: &ae2_cell_core::ItemKey, _amount: u64) -> Result<u64, String> {
        Ok(0)
    }
    /// What the block beside `at` on `face` stores of the type; none without such storage
    /// (`ExternalStorageStrategy::entries`).
    fn storage_entries(&self, _host: &dyn Ae2Host, _at: GridAddress, _face: u8) -> Option<Vec<(ae2_cell_core::ItemKey, u64)>> {
        None
    }
    /// Inserts or extracts up to `amount` of `key` there, or simulates it (`ExternalStorageStrategy::change`).
    fn storage_change(&self, _host: &mut dyn Ae2Host, _at: GridAddress, _face: u8, _key: &ae2_cell_core::ItemKey, _amount: u64, _insert: bool, _simulate: bool) -> Result<u64, String> {
        Ok(0)
    }
    /// Places up to `amount` of `key` in front of `at` (`PlacementStrategy::place`).
    fn place(&self, _host: &mut dyn Ae2Host, _at: GridAddress, _face: u8, _key: &ae2_cell_core::ItemKey, _amount: u64, _simulate: bool) -> Result<u64, String> {
        Ok(0)
    }
}

/// Which strategies an addon key type has: `StackWorldBehaviors.with*Strategy` lists the types by
/// these, with no game at hand.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyStrategyKinds {
    pub import: bool,
    pub export: bool,
    pub external_storage: bool,
    pub placement: bool,
}

/// The addon key types with strategies: the type, its owner's id and which it has.
type KeyStrategyRow = (ae2_cell_core::KeyType, &'static str, KeyStrategyKinds);
static KEY_STRATEGIES: Mutex<Vec<KeyStrategyRow>> = Mutex::new(Vec::new());

/// Registers the world strategies of the addon `owner`'s key type `kind`, which the lib's
/// `ae2_key_types::register` registered first. The addon's `install` calls it before the game
/// starts; a type registered again keeps the first, as AE2's `putIfAbsent` does.
pub fn add_key_strategies(kind: ae2_cell_core::KeyType, owner: &'static str, kinds: KeyStrategyKinds) {
    let mut added = KEY_STRATEGIES.lock().unwrap_or_else(PoisonError::into_inner);
    if !added.iter().any(|(known, _, _)| *known == kind) {
        added.push((kind, owner, kinds));
    }
}

/// The owner and the strategies of the addon key type `kind`, when one was registered.
pub fn key_strategies(kind: ae2_cell_core::KeyType) -> Option<(&'static str, KeyStrategyKinds)> {
    KEY_STRATEGIES.lock().unwrap_or_else(PoisonError::into_inner).iter().find(|(known, _, _)| *known == kind).map(|&(_, owner, kinds)| (owner, kinds))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_type_keeps_the_first_strategies_registered() {
        let kind = ae2_cell_core::KeyType::FLUIDS;
        assert_eq!(key_strategies(kind), None);
        let first = KeyStrategyKinds { external_storage: true, ..KeyStrategyKinds::default() };
        add_key_strategies(kind, "probe", first);
        add_key_strategies(kind, "other", KeyStrategyKinds { import: true, ..first });
        assert_eq!(key_strategies(kind), Some(("probe", first)));
    }
}
