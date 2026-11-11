//! The game behaviour of AE2 addons, one module per addon behind its
//! `ae2-<id>` feature. Their rows in the definitions registry are in the
//! library's `ae2x`. The functions here are the one place the rest of the
//! game calls into the addons; each asks the addons built in.
use super::Game;
use horizon_client::ae2_world::Machine;

#[cfg(feature = "ae2-megacells")]
pub(super) mod megacells;
#[cfg(feature = "ae2-merequester")]
pub(super) mod merequester;
#[cfg(feature = "ae2-extendedae")]
pub(super) mod extendedae;
#[cfg(feature = "ae2-ae2wtlib")]
pub(super) mod ae2wtlib;
#[cfg(feature = "ae2-aeinfinitybooster")]
pub(super) mod aeinfinitybooster;
#[cfg(feature = "ae2-importexportcard")]
pub(super) mod importexportcard;
#[cfg(feature = "ae2-appflux")]
pub(super) mod appflux;
#[cfg(feature = "ae2-enderdrives")]
pub(super) mod enderdrives;
#[cfg(feature = "ae2-advancedae")]
pub(super) mod advancedae;
#[cfg(any(feature = "ae2-extendedae", feature = "ae2-advancedae"))]
pub(super) mod kits;

/// The addons' key types' world strategies (`StackWorldBehaviors.register*`),
/// registered once, before `ae2_key_strategies` is first read.
pub(super) fn register_key_strategies() {
    #[cfg(feature = "ae2-appflux")]
    appflux::register_strategies();
}

/// An addon storage bus's own partition list (`IPartitionList`) beside or in
/// place of AE2's config slots: ExtendedAE's tag, mod and precise buses.
#[derive(Clone)]
pub(super) struct MountFilter {
    /// `isEmpty`: an empty list lets every key pass.
    pub(super) empty: bool,
    /// `isListed`, also what makes the bus preferred storage.
    pub(super) listed: std::rc::Rc<dyn Fn(&ae2_machines_core::Key) -> bool>,
    /// `PreciseInventory.insert`: the most of a key the target may hold.
    pub(super) cap: Option<std::rc::Rc<dyn Fn(&ae2_machines_core::Key) -> u64>>,
    /// What the list is made of, for the storage monitor's cache.
    pub(super) describe: String,
}
impl std::fmt::Debug for MountFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.describe)
    }
}

/// An addon export bus's own `doBusWork` in place of AE2's config-slot loop
/// (`BusState::tick`), with the bus's cards, redstone and schedule.
pub(super) trait ExportLoop {
    fn run(&mut self, state: &mut ae2_machines_core::BusState, active: bool, loaded: bool, host: &mut dyn super::ae2_buses::ExportHost) -> Result<ae2_machines_core::TickResult, String>;
}

/// An addon level emitter's output for the grid's `count` of its key, its
/// `LOW_SIGNAL` setting `low` (ExtendedAE's Threshold Level Emitter: its
/// band and saved state); none for AE2's own.
pub(super) fn emitter_signal(part: &mut horizon_world::nbt::Compound, low: bool, count: i64) -> Option<bool> {
    let _ = (&part, low, count);
    #[cfg(feature = "ae2-extendedae")]
    if let Some(on) = extendedae::special::emitter_signal(part, low, count) {
        return Some(on);
    }
    None
}

/// Whether a formation plane `part` is the grid's storage (AE2's); an
/// addon's that places on its own is not (ExtendedAE's Active Formation Plane).
pub(super) fn formation_storage(part: &horizon_world::nbt::Compound) -> bool {
    let _ = part;
    #[cfg(feature = "ae2-extendedae")]
    if part.get(&"id".into()) == Some(&horizon_world::nbt::Tag::String(horizon_client::ae2x::extendedae::special::ACTIVE_FORMATION_PLANE.into())) {
        return false;
    }
    true
}

/// An interface slot's capacity for a key type, from AE2's `base`: an
/// addon's interface multiplies it (ExtendedAE's Oversize Interface).
pub(super) fn interface_capacity(machine: &Machine, key_type: ae2_cell_core::KeyType, base: u64) -> u64 {
    let _ = (machine, key_type);
    #[cfg(feature = "ae2-extendedae")]
    if let Some(n) = extendedae::interfaces::oversize_multiplier(machine, key_type) {
        return horizon_client::ae2x::extendedae::special::oversize_capacity(base, n);
    }
    base
}
/// The most of an item of `stack_max` an interface slot holds.
pub(super) fn interface_item_max(machine: &Machine, stack_max: u32) -> u32 {
    let _ = machine;
    #[cfg(feature = "ae2-extendedae")]
    if let Some(n) = extendedae::interfaces::oversize_multiplier(machine, ae2_cell_core::KeyType::ITEMS) {
        return horizon_client::ae2x::extendedae::special::oversize_item_max(stack_max, n).min(u64::from(u32::MAX)) as u32;
    }
    stack_max
}

/// A config slot of an addon bus set from the hand: the amount it keeps
/// beside the key (ExtendedAE's precise and threshold buses).
pub(super) fn bus_config_amount(part: &mut horizon_world::nbt::Compound, slot: usize, amount: Option<u64>) {
    let _ = (&part, slot, amount);
    #[cfg(feature = "ae2-extendedae")]
    extendedae::special::config_amount(part, slot, amount);
}

/// An addon provider's directional push (Advanced AE's
/// `AdvPatternProviderLogic.pushInputsDirectionally`): the face of its target
/// each of `pattern`'s sparse inputs goes into, none for the face it pushes
/// to. None where `provider` pushes `pattern` as AE2's does.
pub(super) fn provider_input_directions(
    provider: &Machine,
    pattern: &horizon_client::items::ItemStack,
) -> Option<Vec<Option<horizon_core::geometry::Direction>>> {
    #[cfg(feature = "ae2-advancedae")]
    return advancedae::provider_input_directions(provider, pattern);
    #[cfg(not(feature = "ae2-advancedae"))]
    {
        let _ = (provider, pattern);
        None
    }
}
/// Whether an addon provider's return slots take `key` (Advanced AE's
/// `FILTERED_IMPORT`: only what the provider's patterns make). True for
/// AE2's provider.
pub(super) fn provider_return_accepts(provider: &Machine, key: &ae2_cell_core::ItemKey, key_type: ae2_cell_core::KeyType) -> bool {
    #[cfg(feature = "ae2-advancedae")]
    if !advancedae::return_accepts(provider, key, key_type) {
        return false;
    }
    let _ = (provider, key, key_type);
    true
}
/// [`provider_return_accepts`] for an item stack.
pub(super) fn provider_return_accepts_item(provider: &Machine, stack: &horizon_client::items::ItemStack) -> bool {
    horizon_client::ae2_cell_codec::key(stack).map_or(true, |key| provider_return_accepts(provider, &key, ae2_cell_core::KeyType::ITEMS))
}
/// The processing pattern AE2's encoding terminal made, `encoded`, while
/// its encoded-pattern slot held `previous`: an addon's own pattern in its
/// place (Advanced AE's `MixinPatternEncodingTermMenu`), else `encoded`.
pub(super) fn reencoded_processing(previous: Option<&horizon_client::items::ItemStack>, encoded: horizon_client::items::ItemStack) -> horizon_client::items::ItemStack {
    #[cfg(feature = "ae2-advancedae")]
    if let Some(pattern) = previous.and_then(|previous| advancedae::reencoded(previous, &encoded)) {
        return pattern;
    }
    let _ = previous;
    encoded
}
/// The key mapping of addon hotkey `id`: AE2's `Hotkeys` name an addon's
/// `key.ae2.<id>`; an id `<namespace>:<name>` is its addon's own mapping,
/// `key.<namespace>.<name>` (Advanced AE's).
pub(super) fn hotkey_mapping(id: &str) -> String {
    match id.split_once(':') {
        Some((namespace, name)) => format!("key.{namespace}.{name}"),
        None => format!("key.ae2.{id}"),
    }
}

/// The encoding terminal's machine an item keeps (AE2WTLib's Wireless
/// Pattern Encoding Terminal, by itself or as the universal terminal's
/// current one); none for any other item.
pub(super) fn item_encoding_machine(stack: &horizon_client::items::ItemStack) -> Option<Result<Machine, String>> {
    #[cfg(feature = "ae2-ae2wtlib")]
    if horizon_client::ae2x::wireless::kind(stack) == Some(horizon_client::ae2x::defs::TerminalKind::PatternEncoding) {
        return Some(ae2wtlib::encoding_machine(stack));
    }
    let _ = stack;
    None
}
/// The encoding terminal's machine back into the item that keeps it.
pub(super) fn item_store_encoding(stack: &mut horizon_client::items::ItemStack, machine: &Machine) -> Option<Result<(), String>> {
    #[cfg(feature = "ae2-ae2wtlib")]
    if horizon_client::ae2x::wireless::kind(stack) == Some(horizon_client::ae2x::defs::TerminalKind::PatternEncoding) {
        return Some(ae2wtlib::store_encoding(stack, machine));
    }
    let _ = (stack, machine);
    None
}

/// A storage cell of an addon's own `ICellHandler` (`StorageCells.getCellInventory`
/// for a cell no basic handler takes): MEGA's Bulk Item Cell, Applied Flux's
/// FE cells. Keys of one type, [`HandlerCell::key_type`]; a change is written
/// back to its stack with [`HandlerCell::store`].
/// Its LED is `horizon_client::ae2x::handler_cell_state`.
pub(super) trait HandlerCell {
    /// The type of the keys it holds: items unless it says otherwise.
    fn key_type(&self) -> ae2_cell_core::KeyType {
        ae2_cell_core::KeyType::ITEMS
    }
    /// `getAvailableStacks`, in the cell's order; amounts may be 0.
    fn available_stacks(&mut self) -> Vec<(ae2_cell_core::ItemKey, u64)>;
    fn insert(&mut self, key: &ae2_cell_core::ItemKey, amount: u64, action: ae2_cell_core::Action) -> u64;
    fn extract(&mut self, key: &ae2_cell_core::ItemKey, amount: u64, action: ae2_cell_core::Action) -> u64;
    /// `isPreferredStorageFor`.
    fn is_preferred_storage_for(&self, key: &ae2_cell_core::ItemKey) -> bool;
    /// `persist`: the cell's state onto its stack.
    fn store(&self, stack: &mut horizon_client::items::ItemStack) -> Result<(), String>;
}

/// How an addon's block turns to face the player when placed: AE2's
/// `OrientationStrategies.facing()` or `full()` (facing with spin).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Orientation {
    Facing,
    FacingWithSpin,
    /// `OrientationStrategies.horizontalFacing()`: `facing` toward the player, never up or down.
    Horizontal,
}
/// An addon block's orientation strategy; none for blocks that do not turn
/// (or are AE2's, or a variant of AE2's, which `ae2_placement` knows).
pub(super) fn orientation(block: &str) -> Option<Orientation> {
    #[cfg(feature = "ae2-extendedae")]
    if let Some(found) = extendedae::orientation(block) {
        return Some(found);
    }
    #[cfg(feature = "ae2-advancedae")]
    if block == horizon_client::ae2x::advancedae::REACTION_CHAMBER {
        return Some(Orientation::Horizontal);
    }
    let _ = block;
    None
}

/// An addon machine with an AE2 machine menu (`UpgradeableMenu`): its slots
/// and cards in the chest window, its screen a layout of `ae2_screens`.
pub(super) fn machine_menu(machine: &Machine) -> bool {
    let _ = machine;
    #[cfg(feature = "ae2-extendedae")]
    if extendedae::machine_menu(machine) {
        return true;
    }
    #[cfg(feature = "ae2-advancedae")]
    if advancedae::chamber::is_chamber(machine) {
        return true;
    }
    false
}
/// The most a slot of such a menu holds (`AppEngInternalInventory`'s
/// per-slot limit, the Extended Inscriber's stack size); none for 64.
pub(super) fn machine_slot_capacity(machine: &Machine, slot: usize) -> Option<u32> {
    let _ = (machine, slot);
    #[cfg(feature = "ae2-extendedae")]
    if let Some(found) = extendedae::slot_capacity(machine, slot) {
        return Some(found);
    }
    None
}
/// An addon's Molecular Assembler's progress a tick and power tax
/// (`userPower(ticks, bonusValue, acceleratorTax)`), by its cards; none for AE2's.
pub(super) fn assembler_speed(machine: &Machine) -> Option<(f64, f64)> {
    let _ = machine;
    #[cfg(feature = "ae2-extendedae")]
    if let Some(found) = extendedae::assembler_speed(machine) {
        return Some(found);
    }
    None
}
/// How many crafting threads an addon's assembler has (`CraftingThread`s:
/// ExtendedAE's eight in its Extended Molecular Assembler and each crafter
/// core of the Assembler Matrix); none for AE2's and any other machine.
pub(super) fn assembler_threads(machine: &Machine) -> Option<usize> {
    let _ = machine;
    #[cfg(feature = "ae2-extendedae")]
    if let Some(found) = extendedae::assembler_threads(machine) {
        return Some(found);
    }
    None
}
/// A block an addon's random tick runs on (`Block.randomTick`), after AE2's
/// budding quartz: ExtendedAE's budding entro.
pub(super) fn random_ticks(block: &str) -> bool {
    let _ = block;
    #[cfg(feature = "ae2-extendedae")]
    if extendedae::entro::random_ticks(block) {
        return true;
    }
    false
}
/// What automation (hoppers, droppers, pattern providers) may put in an
/// addon machine's slot `index`, and take out; none leaves AE2's rules.
pub(super) fn slot_insert(kind: horizon_client::ae2_world::MachineKind, index: usize) -> Option<bool> {
    let _ = (kind, index);
    #[cfg(feature = "ae2-extendedae")]
    if let Some(found) = extendedae::slot_insert(kind, index) {
        return Some(found);
    }
    #[cfg(feature = "ae2-advancedae")]
    if let Some(found) = advancedae::chamber::slot_insert(kind, index) {
        return Some(found);
    }
    None
}
pub(super) fn slot_extract(kind: horizon_client::ae2_world::MachineKind, index: usize) -> Option<bool> {
    let _ = (kind, index);
    #[cfg(feature = "ae2-extendedae")]
    if let Some(found) = extendedae::slot_extract(kind, index) {
        return Some(found);
    }
    #[cfg(feature = "ae2-advancedae")]
    if let Some(found) = advancedae::chamber::slot_extract(kind, index) {
        return Some(found);
    }
    None
}
/// Whether Forge Energy enters an addon machine's own buffer from `face`.
pub(super) fn private_power_face(kind: horizon_client::ae2_world::MachineKind, state: &horizon_world::chunk::BlockState, face: horizon_core::geometry::Direction) -> bool {
    let _ = (kind, state, face);
    #[cfg(feature = "ae2-extendedae")]
    if extendedae::private_power_face(kind, state, face) {
        return true;
    }
    #[cfg(feature = "ae2-advancedae")]
    if advancedae::chamber::private_power_face(kind) {
        return true;
    }
    false
}
/// An addon machine screen's progress bar: its value and most.
pub(super) fn machine_progress(machine: &Machine) -> Option<(f64, f64)> {
    let _ = machine;
    #[cfg(feature = "ae2-extendedae")]
    if let Some(found) = extendedae::progress(machine) {
        return Some(found);
    }
    #[cfg(feature = "ae2-advancedae")]
    if let Some(found) = advancedae::chamber::progress(machine) {
        return Some(found);
    }
    None
}
/// Data slots an addon machine's screen reads past its progress bar and
/// toggles (the Reaction Chamber's power alert).
pub(super) fn machine_screen_data(machine: &Machine, data: &mut Vec<i16>) {
    let _ = (machine, &data);
    #[cfg(feature = "ae2-advancedae")]
    advancedae::chamber::screen_data(machine, data);
}
/// The entries an addon machine's window shows past its slots and cards.
pub(super) fn machine_menu_items(machine: &Machine, items: &mut [Option<horizon_client::items::ItemStack>]) {
    let _ = (machine, &items);
    #[cfg(feature = "ae2-megacells")]
    megacells::cell_dock::menu_items(machine, items);
    #[cfg(feature = "ae2-extendedae")]
    extendedae::menu_items(machine, items);
    #[cfg(feature = "ae2-advancedae")]
    advancedae::chamber::menu_items(machine, items);
}
/// The factor on an addon bus's operations (`getOperationsPerTick`): 1 for AE2's.
pub(super) fn bus_multiplier(id: &str) -> u64 {
    let _ = id;
    #[cfg(feature = "ae2-extendedae")]
    if let n @ 2.. = horizon_client::ae2x::extendedae::bus_operations_multiplier(id) {
        return n;
    }
    #[cfg(feature = "ae2-advancedae")]
    if let n @ 2.. = horizon_client::ae2x::advancedae::bus_operations_multiplier(id) {
        return n;
    }
    1
}
/// The screen style of an addon's part (its namespace, where its files under
/// AE2's are, and the file), when it has one of its own.
pub(super) fn part_style(id: &str) -> Option<(&'static str, &'static str)> {
    let _ = id;
    #[cfg(feature = "ae2-extendedae")]
    if let Some(found) = extendedae::part_style(id) {
        return Some(found);
    }
    #[cfg(feature = "ae2-advancedae")]
    if let Some(found) = advancedae::part_style(id) {
        return Some(found);
    }
    None
}
/// An addon bus's work (`doBusWork` of Advanced AE's buses), for its part
/// record `part`: none where it is AE2's ([`ae2_machines_core::BusState::tick`]).
pub(super) fn bus_tick<H: ae2_machines_core::TransferHost>(
    part: &horizon_world::nbt::Compound,
    state: &mut ae2_machines_core::BusState,
    active: bool,
    loaded: bool,
    host: &mut H,
) -> Option<Result<ae2_machines_core::TickResult, H::Error>> {
    #[cfg(feature = "ae2-advancedae")]
    if let Some(outcome) = advancedae::buses::tick(part, state, active, loaded, host) {
        return Some(outcome);
    }
    let _ = (part, state, active, loaded, host);
    None
}
/// A key set in filter `slot` of an addon bus whose filters keep amounts
/// (Advanced AE's stock buses): the amount from `held`.
pub(super) fn bus_filter_set(part: &mut horizon_world::nbt::Compound, slot: usize, key: Option<&ae2_machines_core::Key>, held: Option<&horizon_client::items::ItemStack>) {
    #[cfg(feature = "ae2-advancedae")]
    advancedae::buses::filter_set(part, slot, key, held);
    let _ = (part, slot, key, held);
}
/// A press on the bus menu's `place` that is an addon bus's own setting.
pub(super) fn bus_setting_press(part: &mut horizon_world::nbt::Compound, place: usize) -> bool {
    #[cfg(feature = "ae2-advancedae")]
    if advancedae::buses::setting_press(part, place) {
        return true;
    }
    let _ = (part, place);
    false
}
/// What an addon bus's screen shows beyond AE2's, from its part record.
pub(super) fn part_view(part: &horizon_world::nbt::Compound, view: &mut super::ae2_part_screens::PartView) {
    #[cfg(feature = "ae2-advancedae")]
    advancedae::buses::part_view(part, view);
    let _ = (part, view);
}
/// An addon bus's toolbar buttons after AE2's: the place each presses, its
/// icon and tooltip keys.
pub(super) fn part_tools(view: &super::ae2_part_screens::PartView) -> Vec<(usize, (i32, i32), &'static str, &'static str)> {
    #[cfg(feature = "ae2-advancedae")]
    return advancedae::buses::part_tools(view);
    #[cfg(not(feature = "ae2-advancedae"))]
    {
        let _ = view;
        Vec::new()
    }
}
/// The items an AE2 IO Port of an addon moves a tick, by its speed cards.
pub(super) fn io_port_budget(machine: &Machine) -> Option<u64> {
    let _ = machine;
    #[cfg(feature = "ae2-extendedae")]
    if machine.id() == horizon_client::ae2x::extendedae::EX_IO_PORT {
        return Some(horizon_client::ae2x::extendedae::io_port_budget(machine.installed_speed_cards()));
    }
    None
}

/// The base model an addon's cable part draws in place of AE2's (its status
/// models are AE2's): none for AE2's own parts.
pub(super) fn part_model(id: &str) -> Option<&'static str> {
    #[cfg(feature = "ae2-megacells")]
    if let Some(model) = megacells::part_model(id) {
        return Some(model);
    }
    #[cfg(feature = "ae2-extendedae")]
    if let Some(model) = extendedae::part_model(id) {
        return Some(model);
    }
    #[cfg(feature = "ae2-appflux")]
    if id == horizon_client::ae2x::appflux::PART_FLUX_ACCESSOR {
        return Some(horizon_client::ae2x::appflux::PART_MODEL);
    }
    #[cfg(feature = "ae2-advancedae")]
    if let Some(model) = advancedae::part_model(id) {
        return Some(model);
    }
    let _ = id;
    None
}

/// An addon part's collision and selection boxes (`IPart.getBoxes`), in
/// sixteenths with the part on the south face, as `ae2_geometry` keeps AE2's.
pub(super) fn part_boxes(id: &str) -> Option<Vec<[f64; 6]>> {
    #[cfg(feature = "ae2-megacells")]
    if let Some(boxes) = megacells::cell_dock::boxes(id).or_else(|| megacells::decompression::boxes(id)) {
        return Some(boxes);
    }
    #[cfg(feature = "ae2-appflux")]
    if id == horizon_client::ae2x::appflux::PART_FLUX_ACCESSOR {
        return Some(horizon_client::ae2x::appflux::PART_BOXES.to_vec());
    }
    let _ = id;
    None
}
/// Where an addon's cell part draws its cell (`renderDynamic`), from the
/// block's centre: the cell model's pose, which its LED shares, and the pose
/// of the part's own LED.
pub(super) fn cell_part_poses(id: &str, side: &str, spin: u8) -> Option<(glam::Mat4, glam::Mat4)> {
    #[cfg(feature = "ae2-megacells")]
    if let Some(poses) = megacells::cell_dock::poses(id, side, spin) {
        return Some(poses);
    }
    let _ = (id, side, spin);
    None
}
/// An addon part's spin when placed on cable face `side` by a player of yaw
/// `yaw` (`IPart.onPlacement`): none for a part that keeps no spin.
pub(super) fn part_placement_spin(id: &str, side: &str, yaw: f32) -> Option<u8> {
    #[cfg(feature = "ae2-megacells")]
    if let Some(spin) = megacells::cell_dock::placement_spin(id, side, yaw) {
        return Some(spin);
    }
    let _ = (id, side, yaw);
    None
}
/// The title of the menu an addon's part machine opens (AE2's machine menu
/// over the part); none for a part machine without one.
pub(super) fn part_menu_title(id: &str) -> Option<&'static str> {
    #[cfg(feature = "ae2-megacells")]
    if let Some(title) = megacells::cell_dock::menu_title(id).or_else(|| megacells::decompression::menu_title(id)) {
        return Some(title);
    }
    let _ = id;
    None
}
/// An addon part whose use opens AE2's `PriorityMenu` alone, its priority not
/// the part's own (MEGA's Decompression Module: the grid's pattern priority);
/// the menu's back button closes it (`returnToMainMenu`).
pub(super) fn part_opens_priority(id: &str) -> bool {
    #[cfg(feature = "ae2-megacells")]
    if id == horizon_client::ae2x::megacells::DECOMPRESSION_MODULE {
        return true;
    }
    let _ = id;
    false
}

/// A pattern of an addon's global crafting provider on a grid
/// (`ICraftingService.addGlobalCraftingProvider`: MEGA's
/// `DecompressionService`): the part that stands for the provider, its one
/// input and its output, each an item and an amount.
pub(super) struct GlobalPattern {
    pub(super) owner: super::ae2_parts::PartAddress,
    pub(super) input: (horizon_client::items::ItemStack, u64),
    pub(super) output: (horizon_client::items::ItemStack, u64),
}

/// `AppEngInternalInventory.isItemValid` of a Pattern Provider's pattern
/// inventory past AE2's own rule (an encoded pattern): the filter an addon's
/// provider sets on it (`setFilter`). True for AE2's, which set none.
pub(super) fn pattern_inventory_accepts(machine: &Machine, item: &horizon_client::items::ItemStack) -> bool {
    #[cfg(feature = "ae2-extendedae")]
    if let Some(allowed) = extendedae::pattern_allowed(machine, item) {
        return allowed;
    }
    #[cfg(feature = "ae2-megacells")]
    if let Some(allowed) = megacells::pattern_allowed(machine.id(), item) {
        return allowed;
    }
    let _ = (machine, item);
    true
}


/// An addon's Pattern Access Terminal (part or wireless item) with its own
/// quick move (ExtendedAE's Extended Pattern Access Terminal).
pub(super) fn pattern_terminal_extended(id: &str) -> bool {
    #[cfg(feature = "ae2-extendedae")]
    if extendedae::pattern_terminal_extended(id) {
        return true;
    }
    let _ = id;
    false
}
/// Whether such a terminal's quick move may put `pattern` into a provider of
/// the group shown by `icon`.
pub(super) fn pattern_quick_move_target(pattern: &horizon_client::items::ItemStack, icon: &ae2_cell_core::ItemKey) -> bool {
    #[cfg(feature = "ae2-extendedae")]
    return extendedae::pattern_quick_move_target(pattern, icon);
    #[cfg(not(feature = "ae2-extendedae"))]
    {
        let _ = (pattern, icon);
        true
    }
}
/// An addon's display part: its screen model, drawn `_on` or `_off` over
/// AE2's display base and status.
pub(super) fn display_part_screen(id: &str) -> Option<&'static str> {
    #[cfg(feature = "ae2-extendedae")]
    if let Some(screen) = extendedae::display_part_screen(id) {
        return Some(screen);
    }
    let _ = id;
    None
}

/// Whether an addon's item opens a menu when used (`Item.use` succeeds on
/// the client, `sidedSuccess`): the client then stops at this hand.
pub(super) fn item_opens_menu(id: &str) -> bool {
    #[cfg(feature = "ae2-extendedae")]
    if matches!(id, horizon_client::ae2x::extendedae::VOID_CELL | horizon_client::ae2x::extendedae::PATTERN_MODIFIER | horizon_client::ae2x::extendedae::CONFIG_MODIFIER) {
        return true;
    }
    #[cfg(feature = "ae2-megacells")]
    if id == horizon_client::ae2x::megacells::PORTABLE_CELL_WORKBENCH {
        return true;
    }
    let _ = id;
    false
}

/// What the addons keep while the game runs; of it only Ender Drives'
/// storage is saved, with the world ([`Game::ae2x_save`]).
#[derive(Default)]
pub(super) struct State {
    #[cfg(feature = "ae2-megacells")]
    pub(super) megacells: megacells::State,
    #[cfg(feature = "ae2-merequester")]
    pub(super) merequester: merequester::State,
    #[cfg(feature = "ae2-extendedae")]
    pub(super) extendedae: extendedae::State,
    #[cfg(feature = "ae2-aeinfinitybooster")]
    pub(super) aeinfinitybooster: aeinfinitybooster::State,
    #[cfg(feature = "ae2-ae2wtlib")]
    pub(super) ae2wtlib: ae2wtlib::State,
    #[cfg(feature = "ae2-importexportcard")]
    pub(super) importexportcard: importexportcard::State,
    #[cfg(feature = "ae2-appflux")]
    pub(super) appflux: appflux::State,
    #[cfg(feature = "ae2-enderdrives")]
    pub(super) enderdrives: enderdrives::State,
    #[cfg(feature = "ae2-advancedae")]
    pub(super) advancedae: advancedae::State,
}

/// What an addon's own storage was when AE2's blocks were snapshot
/// (`ae2_snapshot_blocks`): Ender Drives' storage, which a transaction that
/// fails or only simulates puts back with the blocks.
#[derive(Default)]
pub(super) struct Snapshot {
    #[cfg(feature = "ae2-enderdrives")]
    enderdrives: Option<enderdrives::Snapshot>,
}

/// What an addon's pick from the network came to (AE2WTLib's Pick Block,
/// AE2 Import Export Card's Block Picker Card).
#[cfg_attr(not(any(feature = "ae2-ae2wtlib", feature = "ae2-importexportcard")), allow(dead_code))]
pub(super) enum NetworkPick {
    /// The stack is in the hotbar, selected.
    Picked,
    /// None there: the craft amount menu for this key (AE2WTLib's).
    #[cfg_attr(not(feature = "ae2-ae2wtlib"), allow(dead_code))]
    Craft(ae2_cell_core::ItemKey),
}

impl Game<'_> {
    /// The export loop of an addon's export bus `part`; none for AE2's.
    pub(super) fn ae2x_export_loop(&self, part: &horizon_world::nbt::Compound) -> Option<Box<dyn ExportLoop>> {
        let _ = part;
        #[cfg(feature = "ae2-extendedae")]
        if let Some(found) = self.extendedae_export_loop(part) {
            return Some(found);
        }
        None
    }
    /// What an addon's annihilation plane `part` takes into the grid
    /// (ExtendedAE's Smart Annihilation Plane); none for AE2's, which takes all.
    pub(super) fn ae2x_plane_filter(&self, part: &horizon_world::nbt::Compound) -> Option<Box<dyn Fn(&horizon_client::items::ItemStack) -> bool>> {
        let _ = part;
        #[cfg(feature = "ae2-extendedae")]
        if let Some(found) = self.extendedae_plane_filter(part) {
            return Some(found);
        }
        None
    }
    /// A formation plane that works on its own, once a bus tick: an addon's
    /// (ExtendedAE's Active Formation Plane); AE2's is storage and does nothing here.
    pub(super) fn ae2x_plane_tick(&mut self, pos: (i32, i32, i32), part: &mut horizon_world::nbt::Compound, identity: ae2_network_core::NodeIdentity, service: Option<u64>, now: u64) {
        let _ = (pos, &part, identity, service, now);
        #[cfg(feature = "ae2-extendedae")]
        self.extendedae_active_plane_tick(pos, part, identity, service, now);
    }
    /// The partition list of an addon's storage bus `part`, on its item or
    /// fluid channel; none for AE2's own.
    pub(super) fn ae2x_mount_filter(&self, part: &horizon_world::nbt::Compound, fluid: bool) -> Option<MountFilter> {
        let _ = (part, fluid);
        #[cfg(feature = "ae2-extendedae")]
        if let Some(filter) = self.extendedae_mount_filter(part, fluid) {
            return Some(filter);
        }
        None
    }
    pub(super) fn ae2x_snapshot(&self) -> Snapshot {
        Snapshot {
            #[cfg(feature = "ae2-enderdrives")]
            enderdrives: Some(self.enderdrives_snapshot()),
        }
    }
    pub(super) fn ae2x_restore(&mut self, snapshot: Snapshot) {
        #[cfg(feature = "ae2-enderdrives")]
        if let Some(ender) = snapshot.enderdrives {
            ender.restore();
        }
        let _ = snapshot;
    }
    /// The addons' own data of the world, written with its save
    /// (`save_game_metadata`): Ender Drives' storage.
    pub(super) fn ae2x_save(&mut self) {
        #[cfg(feature = "ae2-enderdrives")]
        self.enderdrives_save();
    }
    /// Whether the cell in slot `slot` of the Drive or ME Chest at `host`
    /// is mounted on the grid whose online cell hosts are `hosts`: Ender
    /// Drives' `DuplicateCellGuard` leaves a second disk of one storage out.
    pub(super) fn ae2x_cell_mounted(&self, host: super::ae2_global::GridAddress, slot: usize, stack: &horizon_client::items::ItemStack, hosts: &[super::ae2_global::GridAddress]) -> bool {
        let _ = (host, slot, stack, hosts);
        #[cfg(feature = "ae2-enderdrives")]
        if !self.enderdrives_primary(host, slot, stack, hosts) {
            return false;
        }
        true
    }
    /// `StorageCell.getIdleDrain` of an addon's cell that changes with what it
    /// holds (an Ender Drives disk), in slot `slot` of the Drive or ME Chest
    /// at `host`; none for a cell whose row has it.
    pub(super) fn ae2x_cell_idle_drain(&self, host: super::ae2_global::GridAddress, slot: usize, stack: &horizon_client::items::ItemStack) -> Option<f64> {
        let _ = (host, slot, stack);
        #[cfg(feature = "ae2-enderdrives")]
        if let Some(drain) = self.enderdrives_idle_drain(host, slot, stack) {
            return Some(drain);
        }
        None
    }
    /// What a block's visual shows of an addon's cells that the stack alone
    /// does not tell: Ender Drives' LEDs, by their storage.
    pub(super) fn ae2x_enrich_visual(&self, v: &mut super::ae2_visual_sync::Visual) {
        let _ = &v;
        #[cfg(feature = "ae2-enderdrives")]
        self.enderdrives_visual(v);
    }
    /// Whether a slot of an addon machine's menu takes `item` from the cursor.
    pub(super) fn ae2x_machine_may_place(&self, machine: &Machine, slot: usize, item: &horizon_client::items::ItemStack) -> Option<bool> {
        let _ = (machine, slot, item);
        #[cfg(feature = "ae2-extendedae")]
        if let Some(found) = self.extendedae_may_place(machine, slot, item) {
            return Some(found);
        }
        #[cfg(feature = "ae2-advancedae")]
        if let Some(found) = self.advancedae_may_place(machine, slot, item) {
            return Some(found);
        }
        None
    }
    /// An addon machine's own item handler (`getExposedInventoryForSide`),
    /// which hoppers, buses and providers meet in place of its slots: true
    /// for one the addon handles in [`Game::ae2x_item_insert`].
    pub(super) fn ae2x_item_handler(&self, address: super::ae2_global::GridAddress) -> bool {
        let _ = address;
        #[cfg(feature = "ae2-extendedae")]
        if self.extendedae_item_handler(address) {
            return true;
        }
        false
    }
    /// `insertItem` into such a handler on `face`: what it takes of `stack`
    /// (at most `limit`); none for a block no addon handles.
    pub(super) fn ae2x_item_insert(&mut self, address: super::ae2_global::GridAddress, face: horizon_core::geometry::Direction, stack: &horizon_client::items::ItemStack, limit: u32, simulate: bool) -> Option<u32> {
        let _ = (address, face, stack, limit, simulate);
        #[cfg(feature = "ae2-extendedae")]
        if let Some(found) = self.extendedae_item_insert(address, face, stack, limit, simulate) {
            return Some(found);
        }
        None
    }
    /// `extractItem` from such a handler: the stack it gives.
    pub(super) fn ae2x_item_extract(&mut self, address: super::ae2_global::GridAddress, face: horizon_core::geometry::Direction, limit: u32, filter: Option<&horizon_client::items::ItemStack>, simulate: bool) -> Option<Option<horizon_client::items::ItemStack>> {
        let _ = (address, face, limit, filter, simulate);
        #[cfg(feature = "ae2-extendedae")]
        if let Some(found) = self.extendedae_item_extract(address, face, limit, filter, simulate) {
            return Some(found);
        }
        None
    }
    /// An addon machine's fluid handler (AE2's adapter over its
    /// `GENERIC_INTERNAL_INV`), as the Sky Stone Tank's is met: `fill` of
    /// `amount` mB of `key` (what it takes), `drain` (what it gives), and what
    /// comes out (`contents`). None for a block no addon handles.
    pub(super) fn ae2x_tank_fill(&mut self, address: super::ae2_global::GridAddress, key: &ae2_cell_core::ItemKey, amount: u64, simulate: bool) -> Option<u64> {
        let _ = (address, key, amount, simulate);
        #[cfg(feature = "ae2-advancedae")]
        if let Some(found) = self.advancedae_tank_fill(address, key, amount, simulate) {
            return Some(found);
        }
        None
    }
    pub(super) fn ae2x_tank_drain(&mut self, address: super::ae2_global::GridAddress, key: &ae2_cell_core::ItemKey, amount: u64, simulate: bool) -> Option<u64> {
        let _ = (address, key, amount, simulate);
        #[cfg(feature = "ae2-advancedae")]
        if let Some(found) = self.advancedae_tank_drain(address, key, amount, simulate) {
            return Some(found);
        }
        None
    }
    pub(super) fn ae2x_tank_contents(&self, address: super::ae2_global::GridAddress) -> Option<Option<(ae2_cell_core::ItemKey, u64)>> {
        let _ = address;
        #[cfg(feature = "ae2-advancedae")]
        if let Some(found) = self.advancedae_tank_contents(address) {
            return Some(found);
        }
        None
    }
    /// What could come out of such a handler.
    pub(super) fn ae2x_item_candidates(&self, address: super::ae2_global::GridAddress, face: horizon_core::geometry::Direction) -> Option<Vec<horizon_client::items::ItemStack>> {
        let _ = (address, face);
        #[cfg(feature = "ae2-extendedae")]
        if let Some(found) = self.extendedae_item_candidates(address, face) {
            return Some(found);
        }
        None
    }
    /// An addon block's random tick (`randomTick`), `block` being its id.
    pub(super) fn ae2x_random_tick(&mut self, pos: [i32; 3], block: &str) {
        let _ = (pos, block);
        #[cfg(feature = "ae2-extendedae")]
        if extendedae::entro::random_ticks(block) {
            self.extendedae_entro_random_tick(pos, block);
        }
    }
    /// `StorageCells.getCellInventory` of a cell of an addon's own handler:
    /// none for any other stack (AE2's basic cells, creative cells, items).
    pub(super) fn ae2x_handler_cell(&self, stack: &horizon_client::items::ItemStack) -> Option<Result<Box<dyn HandlerCell>, String>> {
        #[cfg(feature = "ae2-megacells")]
        if stack.item.as_ref() == horizon_client::ae2x::megacells::BULK_ITEM_CELL {
            return Some(self.megacells_bulk_cell(stack).map(|cell| Box::new(cell) as Box<dyn HandlerCell>));
        }
        #[cfg(feature = "ae2-extendedae")]
        if stack.item.as_ref() == horizon_client::ae2x::extendedae::VOID_CELL {
            return Some(self.extendedae_void_cell(stack).map(|cell| Box::new(cell) as Box<dyn HandlerCell>));
        }
        #[cfg(feature = "ae2-appflux")]
        if horizon_client::ae2x::appflux::cell_bytes(&stack.item).is_some() {
            return Some(self.appflux_fe_cell(stack).map(|cell| Box::new(cell) as Box<dyn HandlerCell>));
        }
        #[cfg(feature = "ae2-enderdrives")]
        if horizon_client::ae2x::enderdrives::is_disk(&stack.item) {
            return Some(self.enderdrives_cell(stack).map(|cell| Box::new(cell) as Box<dyn HandlerCell>));
        }
        let _ = stack;
        None
    }
    /// A cell of an addon's own handler whose sneak-use disassembles it when
    /// empty (Applied Flux's `ItemFECell.disassembleDrive`, and
    /// `AbstractPortableCell`'s for its portable cells): whether it is empty.
    /// None for a handler cell with no `use` (MEGA's Bulk Item Cell).
    pub(super) fn ae2x_handler_cell_disassembly(&self, stack: &horizon_client::items::ItemStack) -> Option<bool> {
        #[cfg(feature = "ae2-appflux")]
        if horizon_client::ae2x::appflux::cell_bytes(&stack.item).is_some() {
            return Some(self.appflux_fe_cell(stack).is_ok_and(|mut cell| cell.available_stacks().is_empty()));
        }
        let _ = stack;
        None
    }
    /// The tooltip image of a cell of an addon's own handler
    /// (`getTooltipImage`): its upgrades and its content.
    pub(super) fn ae2x_cell_preview(&self, stack: &horizon_client::items::ItemStack, show_upgrades: bool, show_content: bool) -> Option<(Vec<horizon_client::items::ItemStack>, Vec<(ae2_cell_core::ItemKey, u64)>)> {
        #[cfg(feature = "ae2-megacells")]
        if stack.item.as_ref() == horizon_client::ae2x::megacells::BULK_ITEM_CELL {
            return self.megacells_bulk_preview(stack, show_upgrades, show_content);
        }
        let _ = (stack, show_upgrades, show_content);
        None
    }
    /// What an addon's item adds to its tooltip below its name
    /// (`Item.appendHoverText`), each line in its own style; `name` gives an
    /// item's hover name.
    pub(super) fn ae2x_item_hover_lines(&self, stack: &horizon_client::items::ItemStack, translate: &dyn Fn(&str) -> Option<String>, name: &dyn Fn(&horizon_client::items::ItemStack) -> horizon_client::text::Text) -> Vec<horizon_client::text::Text> {
        #[cfg(feature = "ae2-extendedae")]
        if let Some(lines) = self.extendedae_hover_lines(stack, translate) {
            return lines;
        }
        #[cfg(feature = "ae2-enderdrives")]
        if let Some(lines) = self.enderdrives_hover_lines(stack, translate) {
            return lines;
        }
        // MixinEncodedPatternItem: an Advanced Processing Pattern's own lines.
        #[cfg(feature = "ae2-advancedae")]
        if let Some(lines) = self.advancedae_tooltip(stack, translate, &|item| name(item).plain()) {
            return lines;
        }
        #[cfg(feature = "ae2-megacells")]
        {
            if stack.item.as_ref() == horizon_client::ae2x::megacells::BULK_ITEM_CELL {
                return self.megacells_bulk_hover_lines(stack, name);
            }
            let lines = megacells::hover_lines(&stack.item, translate);
            if !lines.is_empty() {
                return lines;
            }
        }
        // ItemInductionCard.appendHoverText.
        #[cfg(feature = "ae2-appflux")]
        if stack.item.as_ref() == horizon_client::ae2x::appflux::INDUCTION_CARD {
            let line = translate("item.appflux.induction_card.tooltip").unwrap_or_else(|| "Allow AE devices to accept or send energy".to_owned());
            return vec![horizon_client::text::Text::literal(line).with_color(horizon_client::text::GRAY)];
        }
        let _ = (stack, translate, name);
        Vec::new()
    }
    /// What a Cell Workbench's cell adds to its screen: MEGA's compression
    /// cutoff button over a bulk cell (`CellWorkbenchScreenMixin`), shown
    /// with its item; none hides it.
    pub(super) fn ae2x_workbench_cutoff(&self, cell: Option<&horizon_client::items::ItemStack>) -> Option<horizon_client::items::ItemStack> {
        #[cfg(feature = "ae2-megacells")]
        return self.megacells_workbench_cutoff(cell);
        #[cfg(not(feature = "ae2-megacells"))]
        {
            let _ = cell;
            None
        }
    }
    /// A press an addon put on a Cell Workbench's menu at window place
    /// `place` (MEGA's cutoff button, `mega$nextCompressionLimit`), on the
    /// workbench `machine`: true when an addon took it.
    pub(super) fn ae2x_workbench_press(&self, machine: &mut Machine, place: i16, button: i32) -> bool {
        #[cfg(feature = "ae2-megacells")]
        if place == megacells::compression::CUTOFF_PLACE {
            self.megacells_next_compression_limit(machine, button == 1);
            return true;
        }
        let _ = (machine, place, button);
        false
    }
    /// The patterns the addons' global crafting providers offer on the grid of
    /// `anchor`: MEGA's decompression patterns.
    pub(super) fn ae2x_global_patterns(&self, anchor: super::ae2_global::GridAddress) -> Vec<GlobalPattern> {
        #[cfg(feature = "ae2-megacells")]
        {
            let stack = |key: &ae2_cell_core::ItemKey| horizon_client::ae2_cell_codec::prototype(key).ok();
            self.megacells_decompression_patterns(anchor)
                .into_iter()
                .filter_map(|(owner, pattern)| {
                    Some(GlobalPattern {
                        owner,
                        input: (stack(&pattern.from.key)?, u64::from(pattern.from.count)),
                        output: (stack(&pattern.to.key)?, u64::from(pattern.to.count)),
                    })
                })
                .collect()
        }
        #[cfg(not(feature = "ae2-megacells"))]
        {
            let _ = anchor;
            Vec::new()
        }
    }
    /// Whether a global provider still offers its patterns on the grid of
    /// `anchor` (MEGA's: some module on the grid).
    pub(super) fn ae2x_global_provider_present(&self, anchor: super::ae2_global::GridAddress) -> bool {
        #[cfg(feature = "ae2-megacells")]
        {
            !self.megacells_decompression_modules(anchor).is_empty()
        }
        #[cfg(not(feature = "ae2-megacells"))]
        {
            let _ = anchor;
            false
        }
    }
    /// `ICraftingProvider.pushPattern` of a global provider: the inputs are
    /// spent, the outputs go into the grid of `anchor` (MEGA's at the next tick).
    pub(super) fn ae2x_push_global_pattern(&mut self, anchor: super::ae2_global::GridAddress, output: &horizon_client::items::ItemStack, amount: u64) {
        #[cfg(feature = "ae2-megacells")]
        self.megacells_push_decompression(anchor, output, amount);
        let _ = (anchor, output, amount);
    }
    /// The Pattern Access Terminal's group of an addon's provider block
    /// (`PatternContainer.getTerminalGroup`) when it is not AE2's by its
    /// neighbours: its name and tooltip. None for AE2's way.
    pub(super) fn ae2x_pattern_group(&self, machine: &Machine) -> Option<(String, Vec<String>)> {
        let _ = machine;
        #[cfg(feature = "ae2-extendedae")]
        if machine.id() == horizon_client::ae2x::extendedae::MATRIX_PATTERN {
            let name = self.pack.names.display_item(machine.id());
            let line = self.translate("gui.extendedae.assembler_matrix.pattern", "Only accepts crafting patterns");
            return Some((name, vec![line]));
        }
        None
    }
    /// Where a crafting job of the provider at `provider` goes when an addon's
    /// provider sends it elsewhere than to its neighbours (ExtendedAE's
    /// Assembler Matrix pattern core: a free crafter core thread of its
    /// matrix, none while it has none): the assembler and thread, or none.
    /// None when the provider is no such addon's.
    pub(super) fn ae2x_crafting_destination(&self, provider: super::ae2_global::GridAddress, taken: &dyn Fn(super::ae2_global::GridAddress, u8) -> bool) -> Option<Option<(super::ae2_global::GridAddress, u8)>> {
        let _ = (provider, taken);
        #[cfg(feature = "ae2-extendedae")]
        if let Some(found) = self.extendedae_matrix_destination(provider, taken) {
            return Some(found);
        }
        None
    }
    /// The priority a part that opens AE2's `PriorityMenu` alone shows
    /// (`IPriorityHost.getPriority`); none for any other part.
    pub(super) fn ae2x_part_priority(&self, part: super::ae2_parts::PartAddress, id: &str) -> Option<i32> {
        #[cfg(feature = "ae2-megacells")]
        if id == horizon_client::ae2x::megacells::DECOMPRESSION_MODULE {
            return Some(self.megacells_decompression_priority(part.host));
        }
        let _ = (part, id);
        None
    }
    /// `IPriorityHost.setPriority` of such a part: none for any other part.
    pub(super) fn ae2x_set_part_priority(&mut self, part: super::ae2_parts::PartAddress, id: &str, value: i32) -> Option<bool> {
        #[cfg(feature = "ae2-megacells")]
        if id == horizon_client::ae2x::megacells::DECOMPRESSION_MODULE {
            return Some(self.megacells_set_decompression_priority(part, value));
        }
        let _ = (part, id, value);
        None
    }
    /// The addons' grid services while the server's ticks are frozen (AE2's
    /// grid services keep running).
    pub(super) fn tick_ae2x_frozen(&mut self) {
        // Which ender disks sleep, as DuplicateCellGuard decides on each mount.
        #[cfg(feature = "ae2-enderdrives")]
        self.tick_enderdrives();
        #[cfg(feature = "ae2-megacells")]
        self.tick_megacells_decompression();
        // The Assembler Matrix's crafter cores are grid tickables.
        #[cfg(feature = "ae2-extendedae")]
        self.tick_extendedae_matrix();
    }
    /// The addons' grid ticks, after AE2's interfaces.
    pub(super) fn tick_ae2x(&mut self) {
        // DecompressionService.onServerStartTick: what was pushed in an earlier tick.
        #[cfg(feature = "ae2-megacells")]
        self.tick_megacells_decompression();
        #[cfg(feature = "ae2-merequester")]
        self.tick_merequester();
        #[cfg(feature = "ae2-extendedae")]
        self.tick_extendedae_machines();
        #[cfg(feature = "ae2-extendedae")]
        self.tick_extendedae_devices();
        #[cfg(feature = "ae2-extendedae")]
        self.tick_extendedae_matrix();
        #[cfg(feature = "ae2-extendedae")]
        self.tick_extendedae_wireless();
        #[cfg(feature = "ae2-extendedae")]
        self.tick_extendedae_vacuum();
        #[cfg(feature = "ae2-advancedae")]
        self.tick_advancedae_chambers();
        #[cfg(feature = "ae2-advancedae")]
        self.tick_advancedae_quantum();
        #[cfg(feature = "ae2-advancedae")]
        self.tick_advancedae_throughput();
        #[cfg(feature = "ae2-aeinfinitybooster")]
        self.tick_aeinfinitybooster();
        #[cfg(feature = "ae2-importexportcard")]
        self.tick_iecard();
        // EnergyDistributeService.onLevelEndTick: the Flux Accessors push FE,
        // then the interfaces and providers with an Induction Card.
        #[cfg(feature = "ae2-appflux")]
        self.tick_appflux_accessors();
        #[cfg(feature = "ae2-enderdrives")]
        self.tick_enderdrives();
        #[cfg(feature = "ae2-appflux")]
        self.tick_appflux_induction();
    }

    /// A use on an addon's monitor part that is its own (Advanced AE's
    /// Throughput Monitor Configurator): true when it took the use.
    pub(super) fn ae2x_monitor_use(&mut self, pos: (i32, i32, i32), face: &str) -> bool {
        #[cfg(feature = "ae2-advancedae")]
        if self.advancedae_configurator_use(pos, face) {
            return true;
        }
        let _ = (pos, face);
        false
    }
    /// The line an addon's monitor part draws under its amount (Advanced
    /// AE's Throughput Monitor); none for AE2's monitors.
    pub(super) fn ae2x_monitor_line(&self, network: &ae2_network_core::Network, pos: (i32, i32, i32), face: &str) -> Option<super::ae2_visual_sync::RateVisual> {
        #[cfg(feature = "ae2-advancedae")]
        return self.advancedae_monitor_line(network, pos, face);
        #[cfg(not(feature = "ae2-advancedae"))]
        {
            let _ = (network, pos, face);
            None
        }
    }

    /// An addon machine on the grid: its node's kind, idle draw, channel
    /// and the sides it connects on, from its block state's properties.
    pub(super) fn ae2x_device(&self, machine: &Machine, properties: Option<&horizon_world::block_state_properties::BlockProperties>, device: &mut ae2_network_core::Device) -> bool {
        let _ = (machine, properties, &device);
        #[cfg(feature = "ae2-merequester")]
        if merequester::device(machine, properties, device) {
            return true;
        }
        #[cfg(feature = "ae2-extendedae")]
        if extendedae::device(machine, properties, device) {
            return true;
        }
        #[cfg(feature = "ae2-appflux")]
        if appflux::accessor::device(machine, device) {
            return true;
        }
        #[cfg(feature = "ae2-advancedae")]
        if advancedae::chamber::device(machine, device) || advancedae::quantum::device(machine, properties, device) {
            return true;
        }
        false
    }

    /// The Forge Energy storage an addon's machine, or its part on side
    /// `face` of a cable bus, gives that side (its `IEnergyStorage`
    /// capability), before AE2's own: none where no addon answers.
    pub(super) fn ae2x_fe_storage(&self, address: super::ae2_global::GridAddress, face: horizon_core::geometry::Direction) -> Option<super::ae2_power::FeStorage> {
        #[cfg(feature = "ae2-appflux")]
        if let Some(storage) = self.appflux_accessor_fe_storage(address, face) {
            return Some(storage);
        }
        // FEGenericStackInvStorage on an interface's or a provider's generic slots.
        #[cfg(feature = "ae2-appflux")]
        if let Some(storage) = self.appflux_generic_fe_storage(address, face) {
            return Some(storage);
        }
        let _ = (address, face);
        None
    }
    /// `receiveEnergy` (with `extract`, `extractEnergy`) of `amount` FE on
    /// that storage: what moved; none where no addon answers.
    pub(super) fn ae2x_fe_move(&mut self, address: super::ae2_global::GridAddress, face: horizon_core::geometry::Direction, amount: u32, simulate: bool, extract: bool) -> Option<u32> {
        #[cfg(feature = "ae2-appflux")]
        if let Some(moved) = self.appflux_accessor_fe_move(address, face, amount, simulate, extract) {
            return Some(moved);
        }
        #[cfg(feature = "ae2-appflux")]
        if let Some(moved) = self.appflux_generic_fe_move(address, face, amount, simulate, extract) {
            return Some(moved);
        }
        let _ = (address, face, amount, simulate, extract);
        None
    }

    /// The addons' own grid connections, after AE2's (ExtendedAE's wireless
    /// links, `GridHelper.createConnection`).
    pub(super) fn ae2x_connect(&self, network: &mut ae2_network_core::Network) {
        let _ = &network;
        #[cfg(feature = "ae2-extendedae")]
        self.extendedae_wireless_connect(network);
    }

    /// An addon machine's block state properties, set each grid tick
    /// (`updateBlockStateFromBlockEntity`): false when it has none.
    pub(super) fn ae2x_visual_props(&self, pos: (i32, i32, i32), machine: &Machine, online: bool, props: &mut horizon_world::block_state_properties::BlockProperties) -> bool {
        let _ = (pos, machine, online, &props);
        #[cfg(feature = "ae2-extendedae")]
        if self.extendedae_matrix_visual(pos, machine, online, props) || self.extendedae_wireless_visual(pos, machine, props) {
            return true;
        }
        #[cfg(feature = "ae2-merequester")]
        if merequester::visual_props(machine, props) {
            return true;
        }
        #[cfg(feature = "ae2-extendedae")]
        if extendedae::visual_props(machine, props) {
            return true;
        }
        #[cfg(feature = "ae2-advancedae")]
        if advancedae::chamber::visual_props(machine, props) || self.advancedae_quantum_visual(machine, online, props) {
            return true;
        }
        false
    }

    /// A guest uses an addon's block or part: true when an addon opened its menu.
    pub(super) fn lan_open_ae2x(&mut self, host: &mut super::lan::LanHost, conn: u32, pos: (i32, i32, i32), face: &str) -> bool {
        let _ = (&host, conn, pos, face);
        #[cfg(feature = "ae2-extendedae")]
        if self.lan_open_extendedae_matrix(host, conn, pos) {
            return true;
        }
        #[cfg(feature = "ae2-merequester")]
        if self.lan_open_merequester(host, conn, pos, face) {
            return true;
        }
        #[cfg(feature = "ae2-advancedae")]
        if self.lan_open_advancedae_quantum(host, conn, pos) {
            return true;
        }
        false
    }

    /// The addons' crafting CPUs on the grid of `anchor` for a job of
    /// `required` bytes, before AE2's (Advanced AE's Quantum Computer:
    /// `MixinCraftingService.onSubmitJob`): each CPU's place, storage and
    /// co-processors.
    pub(super) fn ae2x_cpu_candidates(&self, anchor: (i32, i32, i32), required: u64) -> Vec<(super::ae2_global::GridAddress, u64, u32)> {
        let _ = (anchor, required);
        #[cfg(feature = "ae2-advancedae")]
        return self.advancedae_cpu_candidates(anchor, required);
        #[cfg(not(feature = "ae2-advancedae"))]
        Vec::new()
    }
    /// An addon CPU's storage and co-processors (a job's own bytes on the
    /// Quantum Computer); none for AE2's.
    pub(super) fn ae2x_cpu_capacity(&self, cpu: super::ae2_global::GridAddress) -> Option<(u64, u32)> {
        let _ = cpu;
        #[cfg(feature = "ae2-advancedae")]
        return self.advancedae_cpu_capacity(cpu);
        #[cfg(not(feature = "ae2-advancedae"))]
        None
    }
    /// The addon CPUs holding a job on the grid of `anchor`, which take what
    /// enters it after AE2's (`insertIntoCpus`).
    pub(super) fn ae2x_cpu_job_owners(&self, anchor: super::ae2_global::GridAddress) -> Vec<super::ae2_global::GridAddress> {
        let _ = anchor;
        #[cfg(feature = "ae2-advancedae")]
        return self.advancedae_cpu_job_owners(anchor);
        #[cfg(not(feature = "ae2-advancedae"))]
        Vec::new()
    }
    /// The blocks whose jobs the removal of an addon CPU block at `pos`
    /// cancels; none for any other block.
    pub(super) fn ae2x_cpu_break_members(&self, pos: (i32, i32, i32)) -> Option<Vec<(i32, i32, i32)>> {
        let _ = pos;
        #[cfg(feature = "ae2-advancedae")]
        return self.advancedae_cpu_break_members(pos);
        #[cfg(not(feature = "ae2-advancedae"))]
        None
    }
    /// Where an addon CPU keeps its selection mode, when not on itself.
    pub(super) fn ae2x_cpu_settings(&self, cpu: super::ae2_global::GridAddress) -> Option<super::ae2_global::GridAddress> {
        let _ = cpu;
        #[cfg(feature = "ae2-advancedae")]
        return self.advancedae_cpu_settings(cpu);
        #[cfg(not(feature = "ae2-advancedae"))]
        None
    }
    /// The CPUs an addon's own crafting status screen at `pos` lists.
    pub(super) fn ae2x_cluster_cpus(&self, pos: (i32, i32, i32)) -> Option<Vec<super::ae2_global::GridAddress>> {
        let _ = pos;
        #[cfg(feature = "ae2-advancedae")]
        return self.advancedae_cluster_cpus(pos);
        #[cfg(not(feature = "ae2-advancedae"))]
        None
    }

    /// The menu of an addon's wireless terminal (`TerminalKind::Addon`, by
    /// its menu id), over the grid of the access point at `anchor`: ME
    /// Requester's Requester Terminal. True when one opened.
    pub(super) fn lan_open_ae2x_wireless(&mut self, host: &mut super::lan::LanHost, conn: u32, anchor: (i32, i32, i32), ctx: super::ae2_wireless::WirelessContext, menu: &str) -> bool {
        let _ = (&host, conn, anchor, &ctx, menu);
        #[cfg(feature = "ae2-merequester")]
        if menu == horizon_client::ae2x::merequester::WIRELESS_TERMINAL {
            return self.lan_open_merequester_wireless(host, conn, anchor, ctx);
        }
        false
    }
    /// An addon's link from a wireless item to its grid that takes over from
    /// the access points (AE2WTLib's Quantum Bridge Card), in any level.
    pub(super) fn ae2x_wireless_link(&self, stack: &horizon_client::items::ItemStack) -> Option<super::ae2_wireless::Reach> {
        #[cfg(feature = "ae2-ae2wtlib")]
        if let Some(reach) = self.ae2wtlib_quantum_reach(stack) {
            return Some(reach);
        }
        let _ = stack;
        None
    }
    /// A link that recharges its item ([`super::ae2_wireless::Reach::recharge`]):
    /// the item at guest `conn`'s `source` slot, from the grid of `node`.
    pub(super) fn ae2x_wireless_recharge(&mut self, host: &mut super::lan::LanHost, conn: u32, source: usize, node: super::ae2_global::GridAddress) {
        let _ = (&host, conn, source, node);
        #[cfg(feature = "ae2-ae2wtlib")]
        self.ae2wtlib_recharge(host, conn, source, node);
    }
    /// The item slot of the addon's wireless menu `conn` has open.
    pub(super) fn lan_ae2x_wireless_source(&self, conn: u32) -> Option<usize> {
        let _ = conn;
        #[cfg(feature = "ae2-merequester")]
        if let Some(source) = self.merequester_wireless_source(conn) {
            return Some(source);
        }
        None
    }

    /// A guest uses the item in hand `hand` (0 main, 1 off) in the air
    /// (`Item.use`): true when an addon's item opened its menu.
    pub(super) fn lan_ae2x_use_item(&mut self, host: &mut super::lan::LanHost, conn: u32, hand: i32) -> bool {
        let _ = (&host, conn, hand);
        #[cfg(feature = "ae2-megacells")]
        if self.lan_open_portable_workbench(host, conn, hand) {
            return true;
        }
        #[cfg(feature = "ae2-extendedae")]
        if self.lan_open_void_cell(host, conn, hand) || self.lan_open_extendedae_tool(host, conn, hand) {
            return true;
        }
        #[cfg(feature = "ae2-advancedae")]
        if self.lan_open_advancedae_encoder_hand(host, conn, hand) {
            return true;
        }
        // ItemWirelessConnectTool.use, sneaking.
        #[cfg(feature = "ae2-extendedae")]
        if self.as_guest(host, conn, |g| g.with_hand(if hand == 1 { super::Hand::Off } else { super::Hand::Main }, |g| g.extendedae_wireless_tool_clear())).unwrap_or(false) {
            return true;
        }
        false
    }
    /// A press on an addon's menu screen's button (`ContainerButtonClick`):
    /// true when it was an addon's window.
    pub(super) fn lan_ae2x_button(&mut self, host: &mut super::lan::LanHost, conn: u32, window: i32, id: i32) -> bool {
        let _ = (&host, conn, window, id);
        #[cfg(feature = "ae2-extendedae")]
        if self.lan_void_cell_button(host, conn, window, id) || self.lan_matrix_button(host, conn, window, id) || self.lan_extendedae_tool_button(host, conn, window, id) {
            return true;
        }
        #[cfg(feature = "ae2-advancedae")]
        if self.lan_advancedae_encoder_button(host, conn, window, id) {
            return true;
        }
        false
    }

    /// A click in an addon's menu window: true when an addon took it.
    pub(super) fn lan_ae2x_click(&mut self, host: &mut super::lan::LanHost, conn: u32, click: &horizon_protocol::packets::play::ContainerClick) -> bool {
        let _ = (&host, conn, click);
        #[cfg(feature = "ae2-megacells")]
        if self.lan_portable_workbench_click(host, conn, click) {
            return true;
        }
        #[cfg(feature = "ae2-merequester")]
        if self.lan_merequester_click(host, conn, click) {
            return true;
        }
        #[cfg(feature = "ae2-ae2wtlib")]
        if self.lan_ae2wtlib_trash_click(host, conn, click) {
            return true;
        }
        #[cfg(feature = "ae2-extendedae")]
        if self.lan_matrix_click(host, conn, click) || self.lan_extendedae_tool_click(host, conn, click) {
            return true;
        }
        #[cfg(feature = "ae2-advancedae")]
        if self.lan_advancedae_encoder_click(host, conn, click) {
            return true;
        }
        false
    }
    /// The client closed window `window`: true when it was an addon's.
    pub(super) fn lan_ae2x_closed(&mut self, host: &mut super::lan::LanHost, conn: u32, window: i32) -> bool {
        let _ = (&host, conn, window);
        #[cfg(feature = "ae2-megacells")]
        if self.lan_portable_workbench_closed(host, conn, Some(window)) {
            return true;
        }
        #[cfg(feature = "ae2-merequester")]
        if self.lan_merequester_closed(host, conn, Some(window)) {
            return true;
        }
        #[cfg(feature = "ae2-ae2wtlib")]
        if self.lan_ae2wtlib_trash_closed(host, conn, Some(window)) {
            return true;
        }
        #[cfg(feature = "ae2-extendedae")]
        if self.lan_void_cell_closed(host, conn, Some(window)) || self.lan_matrix_closed(host, conn, Some(window)) || self.lan_extendedae_tool_closed(host, conn, Some(window)) {
            return true;
        }
        #[cfg(feature = "ae2-advancedae")]
        if self.lan_advancedae_encoder_closed(host, conn, Some(window)) {
            return true;
        }
        false
    }
    /// Shift and the wheel with no screen open, on an addon's held item:
    /// true when one took it.
    pub(super) fn ae2x_item_wheel(&mut self, delta: f32) -> bool {
        #[cfg(feature = "ae2-ae2wtlib")]
        if self.ae2wtlib_item_wheel(delta) {
            return true;
        }
        let _ = delta;
        false
    }
    /// A guest's tick, after its carried items' and before what it touches
    /// (`Player.aiStep`: `inventory.tick`, then `touch`): AE2 Import Export
    /// Card's cards on its wireless terminals, AE2WTLib's hotbar counts and
    /// magnet.
    pub(super) fn lan_ae2x_guest_tick(&mut self, host: &mut super::lan::LanHost, gi: usize) {
        let _ = (&host, gi);
        #[cfg(feature = "ae2-importexportcard")]
        self.lan_iecard_tick(host, gi);
        #[cfg(feature = "ae2-ae2wtlib")]
        self.lan_ae2wtlib_tick(host, gi);
    }

    /// After a guest's packet: what an addon tells the guest at once (the
    /// slots AE2WTLib's restock changed).
    pub(super) fn lan_ae2x_after_packet(&mut self, host: &mut super::lan::LanHost, conn: u32) {
        let _ = (&host, conn);
        #[cfg(feature = "ae2-ae2wtlib")]
        if let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) {
            self.lan_ae2wtlib_resync(host, gi);
        }
    }

    // The acting player's events (a guest's turn): AE2WTLib's Restock and
    // Pick Block, which its mixins and NeoForge's events start.

    /// A restock of the acting player's slot as it stands
    /// (`PlayerInteractEvent.RightClickBlock` as `useItemOn` starts,
    /// `EntityInteractSpecific`, `ArrowNockEvent`, `ArrowLooseEvent`).
    pub(super) fn ae2x_restock_slot(&mut self, slot: usize) {
        let _ = slot;
        #[cfg(feature = "ae2-ae2wtlib")]
        self.ae2wtlib_restock_slot(slot);
    }
    /// `ServerPlayerGameMode.useItemOn` returned: the hand's slot and the
    /// stack it held as the use began.
    pub(super) fn ae2x_use_on_returned(&mut self, slot: usize, before: Option<&horizon_client::items::ItemStack>) {
        let _ = (slot, before);
        #[cfg(feature = "ae2-ae2wtlib")]
        self.ae2wtlib_use_on_returned(slot, before);
    }
    /// `LivingEntityUseItemEvent.Finish` of `item` (as it was before its
    /// use finished), after the mods'.
    pub(super) fn ae2x_use_finished(&mut self, item: &horizon_client::items::ItemStack) {
        let _ = item;
        #[cfg(feature = "ae2-ae2wtlib")]
        self.ae2wtlib_use_finished(item);
    }
    /// `ServerPlayer.drop(boolean)` returned: `before` is how many the
    /// selected slot held.
    pub(super) fn ae2x_dropped(&mut self, before: u32) {
        let _ = before;
        #[cfg(feature = "ae2-ae2wtlib")]
        self.ae2wtlib_dropped(before);
    }
    /// `tryPickItem` found `stack` nowhere and the player has no infinite
    /// materials: an addon may bring it from the network. `from_entity` for
    /// an entity's pick.
    pub(super) fn ae2x_pick_from_network(&mut self, stack: &horizon_client::items::ItemStack, from_entity: bool) -> Option<NetworkPick> {
        let _ = (stack, from_entity);
        #[cfg(feature = "ae2-ae2wtlib")]
        match self.ae2wtlib_pick_block(stack) {
            ae2wtlib::restock::Pick::Picked => return Some(NetworkPick::Picked),
            ae2wtlib::restock::Pick::Craft(key) => return Some(NetworkPick::Craft(key)),
            ae2wtlib::restock::Pick::Nothing => {}
        }
        // The Block Picker Card's packet follows AE2WTLib's (`pickBlock`'s
        // TAIL), for blocks.
        #[cfg(feature = "ae2-importexportcard")]
        if !from_entity && self.iecard_pick_block(stack) {
            return Some(NetworkPick::Picked);
        }
        None
    }
    /// The craft amount menu a pick asked for.
    pub(super) fn lan_ae2x_pick_craft(&mut self, host: &mut super::lan::LanHost, conn: u32, key: ae2_cell_core::ItemKey) {
        let _ = (&host, conn, &key);
        #[cfg(feature = "ae2-ae2wtlib")]
        self.lan_ae2wtlib_pick_craft(host, conn, key);
    }
    /// The count this client's hotbar shows for `stack` in place of its own
    /// (AE2WTLib's `GuiMixin`).
    pub(super) fn ae2x_hotbar_count(&self, stack: &horizon_client::items::ItemStack) -> Option<String> {
        let _ = stack;
        #[cfg(feature = "ae2-ae2wtlib")]
        if let Some(text) = self.ae2wtlib_hotbar_count(stack) {
            return Some(text);
        }
        None
    }
    /// `ItemEntityPickupEvent.Pre` for item entity `index` touching a guest:
    /// true when an addon took some of its stack (AE2WTLib's pickup into
    /// the network).
    pub(super) fn lan_ae2x_item_pickup(&mut self, host: &mut super::lan::LanHost, conn: u32, index: usize) -> bool {
        let _ = (&host, conn, index);
        #[cfg(feature = "ae2-ae2wtlib")]
        if self.lan_ae2wtlib_item_pickup(host, conn, index) {
            return true;
        }
        false
    }
    /// The addons' hotkey actions (`HotkeyActions.register` after AE2's), by
    /// id: the key mapping is `key.ae2.<id>`.
    pub(super) fn ae2x_hotkeys() -> &'static [&'static str] {
        static HOTKEYS: std::sync::LazyLock<Vec<&'static str>> = std::sync::LazyLock::new(|| {
            #[allow(unused_mut)]
            let mut hotkeys = Vec::new();
            #[cfg(feature = "ae2-ae2wtlib")]
            hotkeys.extend(ae2wtlib::HOTKEYS);
            // The other addons' terminals in AE2WTLib's registry: their
            // `WTDefinitionBuilder.hotkeyName`s (ME Requester's).
            #[cfg(feature = "ae2-ae2wtlib")]
            hotkeys.extend(
                horizon_client::ae2x::defs::get()
                    .wireless_terminals()
                    .filter(|t| t.installed.is_some() && matches!(t.kind, horizon_client::ae2x::defs::TerminalKind::Addon(_)))
                    .map(|t| t.hotkey.as_str()),
            );
            #[cfg(feature = "ae2-advancedae")]
            hotkeys.push(advancedae::encoder::HOTKEY);
            hotkeys
        });
        &HOTKEYS
    }
    /// An addon's hotkey action for a guest: true when it ran
    /// (`HotkeyAction.run`).
    pub(super) fn lan_ae2x_hotkey(&mut self, host: &mut super::lan::LanHost, conn: u32, id: &str) -> bool {
        let _ = (&host, conn, id);
        #[cfg(feature = "ae2-ae2wtlib")]
        if self.lan_ae2wtlib_hotkey(host, conn, id) {
            return true;
        }
        #[cfg(feature = "ae2-advancedae")]
        if id == advancedae::encoder::HOTKEY && self.lan_advancedae_hotkey(host, conn) {
            return true;
        }
        false
    }
    /// An addon hotkey `input` while its screen is open (Advanced AE's
    /// `isCloseHotkey`): true when it closed the screen.
    pub(super) fn ae2x_screen_hotkey(&mut self, input: &str) -> bool {
        let _ = input;
        #[cfg(feature = "ae2-advancedae")]
        if self.advancedae_encoder_hotkey(input) {
            return true;
        }
        false
    }
    /// What an addon adds to a wireless terminal's view (AE2WTLib's terminal
    /// settings and magnet mode on the crafting terminal).
    pub(super) fn lan_ae2x_wireless_view(&self, host: &super::lan::LanHost, conn: u32, ctx: &super::ae2_wireless::WirelessContext) -> Option<serde_json::Value> {
        let _ = (host, conn, ctx);
        #[cfg(feature = "ae2-ae2wtlib")]
        let view = self.lan_ae2wtlib_wireless_view(host, conn, ctx);
        #[cfg(not(feature = "ae2-ae2wtlib"))]
        let view: Option<serde_json::Value> = None;
        // AE2 Import Export Card's cards, under their own key.
        #[cfg(feature = "ae2-importexportcard")]
        let view = match self.lan_iecard_wireless_view(host, conn, ctx) {
            Some(cards) => {
                let mut object = view.unwrap_or_else(|| serde_json::json!({}));
                object["iecard"] = cards;
                Some(object)
            }
            None => view,
        };
        view
    }
    /// A custom payload from a guest: true when an addon's channel.
    pub(super) fn lan_ae2x_payload(&mut self, host: &mut super::lan::LanHost, conn: u32, channel: &str, data: &[u8]) -> bool {
        let _ = (&host, conn, channel, data);
        #[cfg(feature = "ae2-ae2wtlib")]
        if channel.starts_with("ae2wtlib:") {
            return self.lan_ae2wtlib_payload(host, conn, channel, data);
        }
        #[cfg(feature = "ae2-merequester")]
        if channel.starts_with("merequester:") {
            self.lan_merequester_payload(host, conn, channel, data);
            return true;
        }
        #[cfg(feature = "ae2-importexportcard")]
        if channel == importexportcard::CARD_MENU {
            self.lan_iecard_payload(host, conn, data);
            return true;
        }
        // The frequency screen's UpdateFrequencyPacket.
        #[cfg(feature = "ae2-enderdrives")]
        if channel == horizon_client::ae2x::enderdrives::UPDATE_FREQUENCY {
            self.lan_enderdrives_payload(host, conn, data);
            return true;
        }
        false
    }
    /// The addons' open menus, once a tick (`broadcastChanges`).
    pub(super) fn lan_refresh_ae2x(&mut self, host: &mut super::lan::LanHost) {
        let _ = &host;
        #[cfg(feature = "ae2-megacells")]
        self.lan_refresh_portable_workbenches(host);
        #[cfg(feature = "ae2-megacells")]
        self.lan_megacells_sync_chains(host);
        #[cfg(feature = "ae2-merequester")]
        self.lan_refresh_merequester(host);
        #[cfg(feature = "ae2-ae2wtlib")]
        self.lan_refresh_ae2wtlib_trash(host);
        #[cfg(feature = "ae2-extendedae")]
        self.lan_refresh_void_cells(host);
        #[cfg(feature = "ae2-extendedae")]
        self.lan_refresh_matrix_menus(host);
        #[cfg(feature = "ae2-extendedae")]
        self.lan_refresh_extendedae_tools(host);
        #[cfg(feature = "ae2-advancedae")]
        self.lan_refresh_advancedae_encoders(host);
    }

    /// `/ae2 <addon> ...`: none when no addon has the command.
    pub(super) fn ae2x_command(&mut self, words: &[&str]) -> Option<Result<serde_json::Value, String>> {
        let _ = words;
        #[cfg(feature = "ae2-merequester")]
        if words.first() == Some(&"requester") {
            return Some(self.ae2_requester_command(&words[1..]));
        }
        #[cfg(feature = "ae2-extendedae")]
        if words.first() == Some(&"eae") {
            return Some(self.extendedae_command(&words[1..]));
        }
        #[cfg(feature = "ae2-importexportcard")]
        if words.first() == Some(&"iecard") {
            return Some(self.ae2_iecard_command(&words[1..]));
        }
        #[cfg(feature = "ae2-enderdrives")]
        if words.first() == Some(&"enderdrives") {
            return Some(self.ae2_enderdrives_command(&words[1..]));
        }
        #[cfg(feature = "ae2-advancedae")]
        if words.first() == Some(&"advae") {
            return Some(self.ae2_advancedae_command(&words[1..]));
        }
        None
    }

    /// The held item used on block `pos`, side `face`, before AE2's own
    /// item uses (`Item.useOn`): true when an addon's item acted.
    pub(super) fn ae2x_use_on(&mut self, pos: [i32; 3], face: [i32; 3]) -> bool {
        let _ = (pos, face);
        #[cfg(feature = "ae2-megacells")]
        if self.megacells_dock_wrench(pos, face) {
            return true;
        }
        #[cfg(feature = "ae2-extendedae")]
        if self.extendedae_wireless_tool_use(pos)
            || self.extendedae_tool_use_on(pos, face)
            || self.extendedae_tape_use(pos, face)
            || self.extendedae_kit_use(pos, face)
            || self.extendedae_seed_use(pos)
            || self.extendedae_package_on(pos, face)
        {
            return true;
        }
        #[cfg(feature = "ae2-advancedae")]
        if self.advancedae_bucket_use(pos) || self.advancedae_kit_use(pos, face) {
            return true;
        }
        false
    }

    /// A use of an addon's block by the hand (its `openGui` when the block has
    /// no menu): true when an addon acted.
    pub(super) fn ae2x_block_use(&mut self, pos: (i32, i32, i32)) -> bool {
        let _ = pos;
        #[cfg(feature = "ae2-extendedae")]
        if self.extendedae_fixer_use(pos) || self.extendedae_charger_use(pos) {
            return true;
        }
        false
    }
    /// A press on window slot `slot` of an addon machine's AE2 menu that is
    /// none of its slots or cards (a tank, a toggle): true when an addon took it.
    pub(super) fn ae2x_machine_click(&mut self, pos: (i32, i32, i32), slot: usize, button: i32, input: horizon_client::menu::Input) -> bool {
        let _ = (pos, slot, button, input);
        #[cfg(feature = "ae2-extendedae")]
        if (300..354).contains(&slot) && self.extendedae_machine_click(pos, slot - 300, button, input) {
            return true;
        }
        #[cfg(feature = "ae2-advancedae")]
        if self.advancedae_machine_click(pos, slot, button, input) {
            return true;
        }
        false
    }
    /// [`Self::ae2x_machine_click`] on the menu of an addon's part.
    pub(super) fn ae2x_part_machine_click(&mut self, part: super::ae2_parts::PartAddress, slot: usize, button: i32, input: horizon_client::menu::Input) -> bool {
        let _ = (part, slot, button, input);
        #[cfg(feature = "ae2-advancedae")]
        if self.advancedae_part_machine_click(part, slot, button, input) {
            return true;
        }
        false
    }

    /// A Memory Card used on an addon's block: true when an addon took it.
    pub(super) fn ae2x_memory(&mut self, pos: [i32; 3]) -> bool {
        let _ = pos;
        #[cfg(feature = "ae2-merequester")]
        if self.ae2_try_requester_memory(pos) {
            return true;
        }
        false
    }

    // An addon's screen on this client, over the menu its server opened.

    /// A server payload of an addon's channel: true when an addon took it.
    pub(super) fn ae2x_client_payload(&mut self, channel: &str, data: &[u8]) -> bool {
        let _ = (channel, data);
        #[cfg(feature = "ae2-megacells")]
        if self.megacells_chains_payload(channel, data) {
            return true;
        }
        #[cfg(feature = "ae2-merequester")]
        if self.merequester_payload(channel, data) {
            return true;
        }
        #[cfg(feature = "ae2-advancedae")]
        if channel == advancedae::encoder::ENCODER_WINDOW {
            self.advancedae_encoder_window_payload(data);
            return true;
        }
        #[cfg(feature = "ae2-ae2wtlib")]
        match channel {
            ae2wtlib::restock::RESTOCK_AMOUNTS => {
                self.ae2wtlib_restock_amounts_payload(data);
                return true;
            }
            ae2wtlib::trash::TRASH_WINDOW => {
                self.ae2wtlib_trash_window_payload(data);
                return true;
            }
            _ => {}
        }
        false
    }
    /// The open addon screen's size.
    pub(super) fn ae2x_screen_size(&self) -> Option<(f32, f32)> {
        #[cfg(feature = "ae2-merequester")]
        if let Some(size) = self.merequester_screen_size() {
            return Some(size);
        }
        #[cfg(feature = "ae2-ae2wtlib")]
        if let Some(size) = self.ae2wtlib_trash_size() {
            return Some(size);
        }
        #[cfg(feature = "ae2-advancedae")]
        if let Some(size) = self.advancedae_encoder_size() {
            return Some(size);
        }
        None
    }
    /// The slots the open addon screen shows.
    pub(super) fn ae2x_slots(&self) -> Option<Vec<usize>> {
        #[cfg(feature = "ae2-merequester")]
        if let Some(slots) = self.merequester_slots() {
            return Some(slots);
        }
        #[cfg(feature = "ae2-ae2wtlib")]
        if let Some(slots) = self.ae2wtlib_trash_slots() {
            return Some(slots);
        }
        #[cfg(feature = "ae2-advancedae")]
        if let Some(slots) = self.advancedae_encoder_slots() {
            return Some(slots);
        }
        None
    }
    /// Where the open addon screen puts slot `i`.
    pub(super) fn ae2x_slot_position(&self, i: usize) -> Option<(f32, f32)> {
        let _ = i;
        #[cfg(feature = "ae2-merequester")]
        if let Some(at) = self.merequester_slot_position(i) {
            return Some(at);
        }
        #[cfg(feature = "ae2-ae2wtlib")]
        if let Some(at) = self.ae2wtlib_trash_slot_position(i) {
            return Some(at);
        }
        #[cfg(feature = "ae2-advancedae")]
        if let Some(at) = self.advancedae_encoder_slot_position(i) {
            return Some(at);
        }
        None
    }
    /// Draws the open addon screen's background and widgets: true then.
    pub(super) fn ae2x_draw(&self, batch: &mut horizon_client::hud::HudBatch, ox: f32, oy: f32) -> bool {
        let _ = (&batch, ox, oy);
        #[cfg(feature = "ae2-merequester")]
        if self.merequester_draw(batch, ox, oy) {
            return true;
        }
        #[cfg(feature = "ae2-ae2wtlib")]
        if self.ae2wtlib_draw_trash(batch, ox, oy) {
            return true;
        }
        #[cfg(feature = "ae2-advancedae")]
        if self.advancedae_draw_encoder(batch, ox, oy) {
            return true;
        }
        false
    }
    /// A press at `at` (from the screen's top left) on an addon screen's
    /// widgets: true when one took it.
    pub(super) fn ae2x_press(&mut self, at: [f32; 2], right: bool) -> bool {
        let _ = (at, right);
        #[cfg(feature = "ae2-merequester")]
        if self.merequester_press(at, right) {
            return true;
        }
        #[cfg(feature = "ae2-ae2wtlib")]
        if !right && self.ae2wtlib_trash_press(at) {
            return true;
        }
        #[cfg(feature = "ae2-advancedae")]
        if self.advancedae_encoder_press(at, right) {
            return true;
        }
        false
    }
    pub(super) fn ae2x_scroll(&mut self, delta: f32) -> bool {
        let _ = delta;
        #[cfg(feature = "ae2-merequester")]
        if self.merequester_scroll(delta) {
            return true;
        }
        #[cfg(feature = "ae2-advancedae")]
        if self.advancedae_encoder_scroll(delta) {
            return true;
        }
        false
    }
    /// Whether an addon screen's text field takes the keyboard.
    pub(super) fn ae2x_field_focused(&self) -> bool {
        #[cfg(feature = "ae2-merequester")]
        if self.merequester_field_focused() {
            return true;
        }
        false
    }
    pub(super) fn ae2x_field_input(&mut self, text: Option<&str>, key: Option<winit::keyboard::KeyCode>) {
        let _ = (text, key);
        #[cfg(feature = "ae2-merequester")]
        self.merequester_field_input(text, key);
    }
    /// The tooltip of the addon screen's widget under the cursor: true when drawn.
    pub(super) fn ae2x_tooltip(&self, batch: &mut horizon_client::hud::HudBatch) -> bool {
        let _ = &batch;
        #[cfg(feature = "ae2-merequester")]
        if self.merequester_tooltip(batch) {
            return true;
        }
        #[cfg(feature = "ae2-ae2wtlib")]
        if self.ae2wtlib_trash_tooltip(batch) {
            return true;
        }
        #[cfg(feature = "ae2-advancedae")]
        if self.advancedae_encoder_tooltip(batch) {
            return true;
        }
        false
    }
    /// The open addon screen for the control socket's `screen` reply.
    pub(super) fn ae2x_agent_value(&self) -> Option<serde_json::Value> {
        #[cfg(feature = "ae2-merequester")]
        if let Some(value) = self.merequester_agent_value() {
            return Some(value);
        }
        #[cfg(feature = "ae2-ae2wtlib")]
        if let Some(value) = self.ae2wtlib_trash_agent() {
            return Some(value);
        }
        #[cfg(feature = "ae2-advancedae")]
        if let Some(value) = self.advancedae_encoder_agent() {
            return Some(value);
        }
        None
    }
}
