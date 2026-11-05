// Mekanism's Chemical Tanks in the game (support module `crate::game::mek_tanks`
// of the `mekanism` native bundle). The rules are the bundle's crate
// (`mekanism_energy::chemical_tank`, MIT, from Mekanism 10.7.19, Copyright (c)
// 2017-2025 Aidan C. Brady); this module is the glue: the Basic to Creative
// Chemical Tanks' block entities in Mekanism's saved form (`chemical_tanks`,
// `dumping`, `component_config` with its chemical and item pages), their tick
// (`TileEntityChemicalTank.onUpdateServer`: the ejector out of its output
// sides, the drain and fill slots, the dump modes), their chemical handler by
// side, the tank item's chemical handler (`Capabilities.CHEMICAL.item()`: the
// tank's contents in the item's `custom_data.mekanism.chemical_tanks`, the
// game taking none of Mekanism's own components), placing and the drop that
// keep the contents, dump mode and config, the screen (`GuiChemicalTank`),
// the item's tooltip and bar, and `/mek probe`.

use super::{
    Game, mek_chemicals,
    mek_ui::{self, Pen, Rect},
    mek_world::{self, ITEM, key, number},
    native_hooks::NativeScreen,
};
use horizon_client::{
    containers::{ModBlockEntity, ModContainer},
    hud::HudBatch,
    items::ItemStack,
    text::Text,
};
use horizon_world::{
    level::flags,
    nbt::{Compound, Tag},
};
use mekanism_energy::{
    chemical::{ChemicalHandler, ChemicalRegistry, ChemicalStack, ChemicalTank},
    chemical_tank::{self, ItemTank, OwnTank, TankBlock, TankSide},
    config::EnergyConfig,
    container::Automation,
    display,
    menu::{Button, Kind, MachineButton, TankView},
    separator::GasMode,
    sides::{self, DataType, Direction, RelativeSide},
    tier::ChemicalTankTier,
};
use serde_json::{Value, json};
use std::cell::Cell;

/// `TransmissionType.CHEMICAL`'s ordinal in `component_config`.
const CHEMICAL: u8 = 2;
const CUSTOM_DATA: &str = "minecraft:custom_data";

pub(super) fn tier_of(block: &str) -> Option<ChemicalTankTier> {
    ChemicalTankTier::of_block(block)
}

/// A tank's settings as its block entity saves them.
#[derive(Clone, Debug, PartialEq)]
struct Tank {
    facing: Direction,
    chemicals: [DataType; 6],
    eject: bool,
    items: [DataType; 6],
    dumping: GasMode,
}

impl Tank {
    /// Its item's (`ItemBlockChemicalTank.SIDE_CONFIG`, `DUMP_MODE` idle).
    fn new() -> Self {
        Self {
            facing: Direction::North,
            chemicals: chemical_tank::default_sides(),
            eject: true,
            items: chemical_tank::default_sides(),
            dumping: GasMode::Idle,
        }
    }

    /// The faces its ejector puts out of: an output or both side's.
    fn output_faces(&self) -> Vec<Direction> {
        Direction::ALL
            .into_iter()
            .filter(|&d| matches!(self.chemicals[RelativeSide::from_directions(self.facing, d).index()], DataType::Output | DataType::InputOutput))
            .collect()
    }
}

fn read(nbt: &Compound) -> Tank {
    let mut tank = Tank::new();
    tank.facing = mek_world::generator_facing(nbt);
    if let Some(Tag::Compound(config)) = nbt.get(&key("component_config")) {
        mek_world::sides_of(config, CHEMICAL, &mut tank.chemicals);
        mek_world::sides_of(config, ITEM, &mut tank.items);
        if let Some(v) = number(config.get(&key(&format!("eject{CHEMICAL}")))) {
            tank.eject = v != 0;
        }
    }
    if let Some(v) = number(nbt.get(&key("dumping"))) {
        tank.dumping = GasMode::by_id(v as i32);
    }
    tank
}

/// `facing`, `component_config` (`config2`, `eject2`, `config3`, `eject3`:
/// the item page never ejects) and `dumping` (its ordinal, `NBTUtils.writeEnum`).
fn write(nbt: &mut Compound, tank: &Tank) {
    nbt.insert(key("facing"), Tag::String(key(tank.facing.name())));
    let mut config = Compound::new();
    for (kind, sides, eject) in [(CHEMICAL, &tank.chemicals, tank.eject), (ITEM, &tank.items, false)] {
        config.insert(key(&format!("config{kind}")), Tag::IntArray(sides.iter().map(|t| t.id()).collect()));
        config.insert(key(&format!("eject{kind}")), Tag::Byte(i8::from(eject)));
    }
    nbt.insert(key("component_config"), Tag::Compound(config));
    nbt.insert(key("dumping"), Tag::Int(tank.dumping.id()));
}

fn block(config: &EnergyConfig, tier: ChemicalTankTier) -> TankBlock {
    let (capacity, output) = config.chemical_tank(tier);
    TankBlock { tier, capacity, output }
}

fn read_tank(nbt: &Compound, block: &TankBlock, registry: &ChemicalRegistry) -> ChemicalTank {
    let stored = mek_chemicals::read_tanks(nbt, 1, registry).pop().unwrap_or(ChemicalStack::EMPTY);
    ChemicalTank::with(block.capacity, stored)
}

fn write_tank(nbt: &mut Compound, tank: &ChemicalTank) {
    mek_chemicals::write_tanks(nbt, std::slice::from_ref(&tank.stored));
}

/// Runs `use_it` with the block tank's rules: the default attribute
/// validator, the creative tank's `ALWAYS_ALLOW`.
fn with_rules<R>(block: &TankBlock, registry: &ChemicalRegistry, use_it: impl FnOnce(&mekanism_energy::chemical::TankRules<'_>) -> R) -> R {
    let creative = block.creative();
    let valid = |s: &ChemicalStack| creative || registry.passes_default(s);
    use_it(&chemical_tank::block_rules(&valid))
}

// ---- Block entities --------------------------------------------------------------

/// `native_block_entity`: its item's config.
pub(super) fn new_entity(block: &str) -> (String, Compound) {
    let mut nbt = Compound::new();
    write(&mut nbt, &Tank::new());
    (block.to_owned(), nbt)
}

/// `native_block_entity_container`: the drain slot (0) and the fill slot
/// (1) in a hopper's menu whose other slots the screen leaves out. Every
/// face reaches both and hoppers fill both with what they take; nothing
/// leaves by a hopper (an item leaves the slots by Mekanism's own rules,
/// which no hook point says: a player takes it).
pub(super) fn container() -> ModContainer {
    ModContainer {
        slots: 5,
        faces: std::array::from_fn(|_| vec![chemical_tank::DRAIN_SLOT, chemical_tank::FILL_SLOT]),
        inputs: vec![chemical_tank::DRAIN_SLOT, chemical_tank::FILL_SLOT],
        outputs: Vec::new(),
        menu: Some("minecraft:hopper".into()),
    }
}

/// `native_block_entity_slot_accepts`: the drain slot takes a chemical tank
/// item alone, the fill slot one that holds a chemical (`getDrainInsertPredicate`,
/// `fillInsertCheck`, which also look at the tank: no hook point brings it).
pub(super) fn slot_accepts(slot: usize, stack: &ItemStack, accepts: &mut bool) {
    let item = tier_of(&stack.item).is_some() && stack.count == 1;
    let ok = match slot {
        chemical_tank::DRAIN_SLOT => item,
        chemical_tank::FILL_SLOT => item && item_data_tanks(stack).is_some(),
        _ => false,
    };
    if !ok {
        *accepts = false;
    }
}

// ---- The item's chemical handler -----------------------------------------------------

/// The `mekanism` compound of a stack's custom data.
fn mekanism_data(stack: &ItemStack) -> Option<Compound> {
    mek_world::item_data(stack)
}

/// The saved `chemical_tanks` of a tank item, when it has one.
fn item_data_tanks(stack: &ItemStack) -> Option<Tag> {
    mekanism_data(stack)?.get(&key("chemical_tanks")).cloned()
}

/// Whether a tank item holds a chemical (as its custom data says).
pub(super) fn item_holds(stack: &ItemStack) -> bool {
    item_data_tanks(stack).is_some()
}

/// What a chemical tank item holds (none: not a tank item).
pub(super) fn item_stored(stack: &ItemStack, registry: &ChemicalRegistry) -> Option<(ChemicalTankTier, ChemicalStack)> {
    let tier = tier_of(&stack.item)?;
    let stored = mekanism_data(stack).map(|d| mek_chemicals::read_tanks(&d, 1, registry).pop().unwrap_or(ChemicalStack::EMPTY));
    Some((tier, stored.unwrap_or(ChemicalStack::EMPTY)))
}

/// Writes what a tank item holds into its custom data (`chemical_tanks`, as
/// the block saves it), removed when empty so the item stacks with new ones.
fn set_item_stored(stack: &mut ItemStack, stored: &ChemicalStack) {
    let mut custom = match stack.data().and_then(|d| d.other(CUSTOM_DATA)) {
        Some(Tag::Compound(c)) => c.clone(),
        _ => Compound::new(),
    };
    let mut mekanism = match custom.get(&key("mekanism")) {
        Some(Tag::Compound(c)) => c.clone(),
        _ => Compound::new(),
    };
    mek_chemicals::write_tanks(&mut mekanism, std::slice::from_ref(stored));
    if mekanism.is_empty() {
        custom.remove(&key("mekanism"));
    } else {
        custom.insert(key("mekanism"), Tag::Compound(mekanism));
    }
    let value = (!custom.is_empty()).then_some(Tag::Compound(custom));
    stack.data_mut().set_other(CUSTOM_DATA, value);
    stack.tidy();
}

/// `Capabilities.CHEMICAL.getCapability(stack)` of a chemical tank item: a
/// stack of one only. What `use_it` changed is written back.
pub(super) fn with_item<R>(stack: &mut ItemStack, registry: &ChemicalRegistry, config: &EnergyConfig, use_it: impl FnOnce(&mut dyn ChemicalHandler) -> R) -> Option<R> {
    if stack.count != 1 {
        return None;
    }
    let (tier, stored) = item_stored(stack, registry)?;
    let creative = tier == ChemicalTankTier::Creative;
    let valid = |s: &ChemicalStack| creative || registry.passes_default(s);
    let mut item = ItemTank::of_tier(tier, config.chemical_tank(tier), stored.clone(), &valid);
    let answer = use_it(&mut item);
    if item.stored != stored.normalized() {
        set_item_stored(stack, &item.stored);
    }
    Some(answer)
}

/// The machine slot at `slot` of `entity` with a chemical item in it: `use_it`
/// on its handler, the slot written back when it changed. None without a
/// handler there.
pub(super) fn with_slot_item<R>(
    entity: &mut ModBlockEntity,
    slot: usize,
    registry: &ChemicalRegistry,
    config: &EnergyConfig,
    use_it: impl FnOnce(&mut dyn ChemicalHandler) -> R,
) -> Option<R> {
    let mut stack = entity.items.get(slot).and_then(Option::as_ref).filter(|s| s.count > 0)?.clone();
    let before = stack.clone();
    let answer = with_item(&mut stack, registry, config, use_it)?;
    if stack != before
        && let Some(cell) = entity.items.get_mut(slot)
    {
        *cell = Some(stack);
    }
    Some(answer)
}

// ---- The tick -----------------------------------------------------------------------

/// `native_block_entity_tick` (`TileEntityChemicalTank.onUpdateServer`): the
/// ejector (`ChemicalUtil.emit` of the tier's output a tick, as the tank
/// itself, out of its output sides, unless dumping), the drain slot, the fill
/// slot, the dump mode; then the screen's data slots. Redstone control is
/// IGNORED, the default.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, tier: ChemicalTankTier) {
    let (config, _) = mek_world::energy_config();
    let registry = mek_chemicals::registry(game);
    let block = block(&config, tier);
    let before = read(&entity.nbt);
    let mut tank_state = before.clone();
    tank_state.facing = game.native_block_property(pos, "facing").and_then(|f| Direction::from_name(&f)).unwrap_or(Direction::North);
    let mut tank = read_tank(&entity.nbt, &block, &registry);
    let tank_before = tank.clone();

    if tank_state.eject && block.ejects(tank_state.dumping) && !tank.is_empty() {
        let offered = with_rules(&block, &registry, |rules| block.extract(&mut tank, block.output, true, Automation::Internal, rules));
        let sent = mek_chemicals::emit(game, pos, &tank_state.output_faces(), &offered);
        if sent > 0 {
            with_rules(&block, &registry, |rules| block.extract(&mut tank, sent, false, Automation::Internal, rules));
        }
    }
    for (slot, drain) in [(chemical_tank::DRAIN_SLOT, true), (chemical_tank::FILL_SLOT, false)] {
        with_rules(&block, &registry, |rules| {
            with_slot_item(entity, slot, &registry, &config, |item| {
                let mut own = OwnTank { tank: &mut tank, rules, block: Some(block) };
                if drain { chemical_tank::drain_into_item(item, &mut own) } else { chemical_tank::fill_from_item(item, &mut own) }
            })
        });
    }
    block.dump(&mut tank, tank_state.dumping, config.machines.dump_excess_keep_ratio);

    if tank_state != before || !entity.nbt.contains_key(&key("component_config")) {
        write(&mut entity.nbt, &tank_state);
    }
    if tank != tank_before {
        write_tank(&mut entity.nbt, &tank);
    }
    entity.data = view(&tank_state, &tank, &block, &registry).pack();
}

fn view(state: &Tank, tank: &ChemicalTank, block: &TankBlock, registry: &ChemicalRegistry) -> TankView {
    TankView {
        stored: tank.stored().max(0) as u64,
        capacity: block.capacity.max(0) as u64,
        chemical: registry.number(tank.stored.id()),
        dumping: state.dumping.id(),
        creative: block.creative(),
        chemicals: state.chemicals,
        eject: state.eject,
        items: state.items,
    }
}

// ---- The handler -----------------------------------------------------------------------

/// The tank's chemical handler through `face` (`ConfigChemicalTankHolder`):
/// another block (`EXTERNAL`, any amount) puts in on an input side and takes
/// out of an output side; the null side reads it; a `NONE` side has none.
pub(super) fn chemical_handler<R>(
    entity: &mut ModBlockEntity,
    face: Option<Direction>,
    registry: &ChemicalRegistry,
    use_it: impl FnOnce(&mut dyn ChemicalHandler) -> R,
) -> Option<R> {
    let tier = tier_of(&entity.id)?;
    let (config, _) = mek_world::energy_config();
    let block = block(&config, tier);
    let state = read(&entity.nbt);
    let (side, can_insert, can_extract) = match face {
        None => (false, false, false),
        Some(f) => {
            let (i, o) = chemical_tank::side_io(state.chemicals[RelativeSide::from_directions(state.facing, f).index()])?;
            (true, i, o)
        }
    };
    let mut tank = read_tank(&entity.nbt, &block, registry);
    let before = tank.clone();
    let answer = with_rules(&block, registry, |rules| {
        let mut handler = TankSide { tank: &mut tank, block, rules, side, can_insert, can_extract };
        use_it(&mut handler)
    });
    if tank != before {
        write_tank(&mut entity.nbt, &tank);
    }
    Some(answer)
}

// ---- Buttons -------------------------------------------------------------------------------

/// A side button on `sides` (`PacketSideData`, `PacketBatchConfiguration`).
fn press(sides: &mut [DataType; 6], button: Button) {
    let supports = chemical_tank::supports;
    match button {
        Button::Side { side, forward, .. } => {
            if let Some(t) = sides.get_mut(side) {
                *t = if forward { t.next(supports) } else { t.previous(supports) };
            }
        }
        Button::Clear { forward, .. } => {
            let shared = sides.iter().all(|t| *t == sides[0]).then_some(sides[0]);
            let target = match shared {
                Some(t) if forward => t.next(supports),
                Some(t) => t.previous(supports),
                None => DataType::None,
            };
            *sides = [target; 6];
        }
        _ => {}
    }
}

/// `native_menu_button`: the side configuration (chemicals, items, the
/// chemical eject) and the dump mode (`nextMode(0)`).
pub(super) fn button(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity, id: i32) {
    let (Some(tier), Some(button)) = (tier_of(&entity.id), MachineButton::from_id(id)) else {
        return;
    };
    let before = read(&entity.nbt);
    let mut state = before.clone();
    match button {
        MachineButton::Config(b @ (Button::Side { kind, .. } | Button::Clear { kind, .. })) => match kind {
            Kind::Chemicals => press(&mut state.chemicals, b),
            Kind::Items => press(&mut state.items, b),
            Kind::Energy => {}
        },
        MachineButton::Config(Button::Eject(Kind::Chemicals)) => state.eject = !state.eject,
        MachineButton::NextMode(0) => state.dumping = state.dumping.next(),
        _ => {}
    }
    if state == before {
        return;
    }
    write(&mut entity.nbt, &state);
    let (config, _) = mek_world::energy_config();
    let registry = mek_chemicals::registry(game);
    let block = block(&config, tier);
    let tank = read_tank(&entity.nbt, &block, &registry);
    entity.data = view(&state, &tank, &block, &registry).pack();
    let _ = pos;
}

// ---- Placing and breaking -------------------------------------------------------------------

/// `native_place_block`: the tank faces the player who placed it
/// (`AttributeStateFacing`, horizontal), and takes its item's contents, dump
/// mode and config (`DUMP_MODE`, `SIDE_CONFIG`, `mekanism:chemicals`).
pub(super) fn placed(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3]) -> bool {
    let Some(tier) = tier_of(&stack.item) else {
        return false;
    };
    let facing = game.native_player().map_or(Direction::North, |p| sides::placement_facing(p.yaw, 0.0));
    let saved = mekanism_data(stack);
    let done = game.native_with_mod_block_entity(pos, |m| {
        (tier_of(&m.id) == Some(tier)).then(|| {
            for name in ["chemical_tanks", "component_config", "dumping"] {
                if let Some(tag) = saved.as_ref().and_then(|s| s.get(&key(name))) {
                    m.nbt.insert(key(name), tag.clone());
                }
            }
            let mut state = read(&m.nbt);
            state.facing = facing;
            write(&mut m.nbt, &state);
        })
    });
    if done.flatten().is_some() {
        game.native_set_block_property_with(pos, "facing", facing.name(), flags::UPDATE_ALL);
    }
    true
}

/// What a tank's drop keeps (`copy_components`): its contents, its dump mode
/// when not idle, its config when not its item's; none at all for an
/// untouched tank, whose drop then stacks with new ones.
pub(super) fn drop_data(nbt: &Compound) -> Option<Compound> {
    let mut data = Compound::new();
    if let Some(tanks) = nbt.get(&key("chemical_tanks")) {
        data.insert(key("chemical_tanks"), tanks.clone());
    }
    let state = read(nbt);
    if state.dumping != GasMode::Idle {
        data.insert(key("dumping"), Tag::Int(state.dumping.id()));
    }
    let fresh = Tank::new();
    if (state.chemicals, state.eject, state.items) != (fresh.chemicals, fresh.eject, fresh.items)
        && let Some(config) = nbt.get(&key("component_config"))
    {
        data.insert(key("component_config"), config.clone());
    }
    (!data.is_empty()).then_some(data)
}

// ---- /mek probe --------------------------------------------------------------------------------

pub(super) fn probe(game: &Game<'_>, pos: [i32; 3], id: &str, nbt: &Compound, items: &[Option<ItemStack>]) -> Value {
    let Some(tier) = tier_of(id) else {
        return json!({ "pos": pos, "id": id, "error": "not a chemical tank" });
    };
    let (config, from_files) = mek_world::energy_config();
    let registry = mek_chemicals::registry(game);
    let block = block(&config, tier);
    let state = read(nbt);
    let tank = read_tank(nbt, &block, &registry);
    let names = |sides: &[DataType; 6]| -> serde_json::Map<String, Value> {
        RelativeSide::ALL.iter().map(|s| (s.name().to_owned(), sides[s.index()].name().into())).collect()
    };
    let slots: Vec<Value> = items
        .iter()
        .take(2)
        .map(|s| match s {
            Some(s) => {
                let held = item_stored(s, &registry).map(|(_, c)| json!({ "id": c.id(), "amount": c.amount() }));
                json!({ "item": &*s.item, "count": s.count, "chemical": held })
            }
            None => Value::Null,
        })
        .collect();
    json!({
        "pos": pos,
        "id": id,
        "tier": tier.name(),
        "chemical": tank.stored.id(),
        "amount": tank.stored(),
        "capacity": block.capacity,
        "output": block.output,
        "facing": state.facing.name(),
        "dumping": state.dumping.name(),
        "eject": state.eject,
        "chemical_config": names(&state.chemicals),
        "item_config": names(&state.items),
        "slots": slots,
        "config": { "from_files": from_files, "dump_excess_keep_ratio": config.machines.dump_excess_keep_ratio },
    })
}

// ---- The screen (`GuiChemicalTank`) ---------------------------------------------------------------

thread_local! {
    /// 0: the tank; 2: item side configuration; 3: chemical side configuration.
    static PAGE: Cell<u8> = const { Cell::new(0) };
}

/// `GuiChemicalBar` (42, 16), `GuiInnerScreen` (42, 37), `GuiGasMode` (159, 72).
const BAR: Rect = [42.0, 16.0, 116.0, 10.0];
const SCREEN: Rect = [42.0, 37.0, 118.0, 28.0];
const MODE: Rect = [159.0, 72.0, 10.0, 10.0];
/// The drain (`+`) and fill (`-`) slots, where the game draws their items.
const SLOTS: [[f32; 2]; 2] = [[17.0, 17.0], [17.0, 49.0]];
/// The side configuration page over the tank's upper half, as the cube's.
const WINDOW: Rect = [36.0, 2.0, 104.0, 81.0];
const CLOSE: Rect = [40.0, 6.0, 8.0, 8.0];
const EJECT_SCREEN: Rect = [40.0, 17.0, 78.0, 12.0];
const EJECT: Rect = [122.0, 16.0, 14.0, 14.0];
const CLEAR: Rect = [122.0, 65.0, 14.0, 14.0];
const ITEMS_PAGE: Rect = [40.0, 33.0, 18.0, 18.0];
const CHEMICALS_PAGE: Rect = [40.0, 54.0, 18.0, 18.0];

/// `native_screen`: 176 x 166, its two slots; it opens on the tank's page.
pub(super) fn screen() -> NativeScreen {
    PAGE.with(|p| p.set(0));
    NativeScreen {
        width: 176.0,
        height: 166.0,
        slots: vec![Some(SLOTS[0]), Some(SLOTS[1]), None, None, None],
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: true,
        own_background: false,
    }
}

fn page_kind(page: u8) -> Kind {
    if page == 2 { Kind::Items } else { Kind::Chemicals }
}

enum Hit {
    Press(MachineButton),
    Tip(Vec<String>),
}

fn mode_name(mode: i32) -> &'static str {
    match GasMode::by_id(mode) {
        GasMode::Idle => "Idle",
        GasMode::DumpingExcess => "Dumping Excess",
        GasMode::Dumping => "Dumping",
    }
}

fn chemical_name(number: i32) -> Option<String> {
    mek_chemicals::loaded().as_deref().and_then(|r| r.by_number(number).map(mek_chemicals::name))
}

/// The inner screen's two lines.
fn screen_lines(view: &TankView) -> [String; 2] {
    let name = if view.stored == 0 { "None".to_owned() } else { chemical_name(view.chemical).unwrap_or_else(|| "?".into()) };
    let amount = if view.creative && view.stored > 0 {
        "Infinite".to_owned()
    } else if view.creative {
        "0/Infinite".to_owned()
    } else {
        format!("{}/{}", display::thousands(view.stored), display::thousands(view.capacity))
    };
    [format!("Chemical: {name}"), amount]
}

fn hit(page: u8, view: &TankView, at: [f32; 2], right: bool) -> Option<Hit> {
    let press = |b: Button| Some(Hit::Press(MachineButton::Config(b)));
    if page != 0 {
        let kind = page_kind(page);
        for side in RelativeSide::ALL {
            if mek_ui::inside(mek_ui::side_button(side), at) {
                return press(Button::Side { kind, side: side.index(), forward: !right });
            }
        }
        if mek_ui::inside(CLOSE, at) {
            return press(Button::Page(0));
        }
        if mek_ui::inside(EJECT, at) && kind == Kind::Chemicals {
            return if right { Some(Hit::Tip(vec!["Auto-eject".into()])) } else { press(Button::Eject(kind)) };
        }
        if mek_ui::inside(CLEAR, at) {
            return press(Button::Clear { kind, forward: !right });
        }
        if mek_ui::inside(ITEMS_PAGE, at) {
            return press(Button::Page(2));
        }
        if mek_ui::inside(CHEMICALS_PAGE, at) {
            return press(Button::Page(3));
        }
        if mek_ui::inside(WINDOW, at) {
            return None;
        }
    }
    if mek_ui::inside(mek_ui::CONFIG_BUTTON, at) {
        return press(Button::Page(if page == 0 { 3 } else { 0 }));
    }
    if page == 0 && mek_ui::inside(MODE, at) {
        return Some(Hit::Press(MachineButton::NextMode(0)));
    }
    if page == 0 && mek_ui::inside(BAR, at) {
        let lines = screen_lines(view);
        return Some(Hit::Tip(vec![if view.stored == 0 { "Empty".into() } else { format!("{}: {}", lines[0].trim_start_matches("Chemical: "), lines[1]) }]));
    }
    None
}

/// `native_screen_click`.
pub(super) fn click(data: &[i32], at: [f32; 2], right: bool, button: &mut Option<i32>) {
    let view = TankView::unpack(data).unwrap_or_default();
    let page = PAGE.with(Cell::get);
    let Some(Hit::Press(pressed)) = hit(page, &view, at, right) else {
        return;
    };
    if let MachineButton::Config(Button::Page(p)) = pressed {
        PAGE.with(|c| c.set(p));
    }
    *button = Some(pressed.id());
}

/// `native_screen_tooltip`.
pub(super) fn tooltip(data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    let view = TankView::unpack(data).unwrap_or_default();
    let page = PAGE.with(Cell::get);
    match hit(page, &view, at, false) {
        Some(Hit::Tip(tip)) => lines.extend(tip),
        Some(Hit::Press(MachineButton::NextMode(_))) => lines.push(format!("Mode: {}", mode_name(view.dumping))),
        Some(Hit::Press(MachineButton::Config(Button::Side { kind, side, .. }))) => {
            let sides = if kind == Kind::Items { view.items } else { view.chemicals };
            let side = RelativeSide::ALL[side];
            lines.push(mek_ui::side_name(side).into());
            lines.push(mek_ui::type_name(sides[side.index()]).into());
        }
        Some(Hit::Press(MachineButton::Config(b))) => lines.push(
            match b {
                Button::Page(0) => "Close",
                Button::Page(3) if page == 0 => "Side Configuration",
                Button::Page(2) => "Items",
                Button::Page(_) => "Chemicals",
                Button::Eject(_) => "Auto-eject",
                Button::Clear { .. } => "Clear Sides",
                Button::Side { .. } => "",
            }
            .into(),
        ),
        _ => {}
    }
}

/// `native_screen_draw`.
pub(super) fn draw(game: &Game<'_>, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let view = TankView::unpack(data).unwrap_or_default();
    let page = PAGE.with(Cell::get);
    let cursor = game.native_cursor();
    let cursor = [cursor[0] - origin[0], cursor[1] - origin[1]];
    let mut pen = Pen { game, batch, origin, cursor };
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [0.0, 0.0, 176.0, 166.0]);
    pen.holder(mek_ui::CONFIG_TAB, true);
    pen.image_button("configuration", 18.0, mek_ui::CONFIG_BUTTON, true);

    // The chemical bar: its frame, the chemical's colour from the left (Mekanism tints its texture).
    pen.nine("bar/base", [0.0, 0.0, 5.0, 5.0], [2.0, 2.0], BAR);
    let level = if view.creative && view.stored > 0 {
        1.0
    } else if view.capacity == 0 {
        0.0
    } else {
        (view.stored as f64 / view.capacity as f64).min(1.0)
    };
    let w = (level * f64::from(BAR[2] - 2.0)) as i32 as f32;
    if w > 0.0 {
        let tint = mek_chemicals::registry(game).by_number(view.chemical).map_or(0x808080, |c| c.color & 0xFF_FFFF);
        pen.fill([BAR[0] + 1.0, BAR[1] + 1.0, w, BAR[3] - 2.0], 0xFF00_0000 | tint);
    }
    pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], SCREEN);
    let lines = screen_lines(&view);
    pen.text(&lines[0], [SCREEN[0] + 4.0, SCREEN[1] + 4.0], mek_ui::SCREEN_TEXT);
    pen.text(&lines[1], [SCREEN[0] + 4.0, SCREEN[1] + 15.0], mek_ui::SCREEN_TEXT);
    let icon = match GasMode::by_id(view.dumping) {
        GasMode::Idle => "gas_mode_idle",
        GasMode::DumpingExcess => "gas_mode_excess",
        GasMode::Dumping => "gas_mode_dump",
    };
    pen.whole(icon, [10.0, 10.0], MODE);
    pen.slot("output", Some("overlay_plus"), SLOTS[0][0] - 1.0, SLOTS[0][1] - 1.0);
    pen.slot("input", Some("overlay_minus"), SLOTS[1][0] - 1.0, SLOTS[1][1] - 1.0);
    for row in 0..4 {
        let y = if row == 3 { 141.0 } else { 83.0 + 18.0 * row as f32 };
        for col in 0..9 {
            pen.slot("normal", None, 7.0 + 18.0 * col as f32, y);
        }
    }
    let title = game.native_screen_title().unwrap_or_default();
    pen.centered(&title, 0.0, 176.0, 6.0, mek_ui::TITLE);
    pen.text("Inventory", [8.0, 72.0], mek_ui::TITLE);
    if page == 0 {
        return;
    }

    // The side configuration.
    let kind = page_kind(page);
    let (sides, eject) = match kind {
        Kind::Items => (view.items, None),
        _ => (view.chemicals, Some(view.eject)),
    };
    let [wx, wy, ww, wh] = WINDOW;
    pen.nine("shadow", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], [wx - 3.0, wy - 3.0, ww + 6.0, wh + 6.0]);
    pen.nine("base", [0.0, 0.0, 256.0, 256.0], [4.0, 4.0], WINDOW);
    pen.image_button("button/close", 12.0, CLOSE, true);
    let name = if kind == Kind::Items { "Items Config" } else { "Chemicals Config" };
    pen.centered(name, wx, ww, wy + 4.0, mek_ui::TITLE);
    pen.nine("inner_screen", [0.0, 0.0, 256.0, 256.0], [32.0, 32.0], EJECT_SCREEN);
    let state = match eject {
        Some(on) => format!("Eject: {}", if on { "On" } else { "Off" }),
        None => "Can't Eject".to_owned(),
    };
    pen.centered(&state, EJECT_SCREEN[0], EJECT_SCREEN[2], EJECT_SCREEN[1] + 2.0, mek_ui::SCREEN_TEXT);
    pen.image_button("button/auto_eject", 14.0, EJECT, eject.is_some());
    pen.image_button("button/clear_sides", 14.0, CLEAR, true);
    for (icon, r, on) in [("items", ITEMS_PAGE, kind == Kind::Items), ("chemicals", CHEMICALS_PAGE, kind == Kind::Chemicals)] {
        pen.button(r, !on);
        pen.whole(icon, [18.0, 18.0], r);
    }
    for side in RelativeSide::ALL {
        let r = mek_ui::side_button(side);
        pen.button(r, true);
        pen.fill([r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0], mek_ui::side_color(sides[side.index()]));
    }
}

// ---- The item's tooltip and bar ---------------------------------------------------------------------

/// `ItemBlockChemicalTank.appendHoverText` under the name: what it holds
/// (`StorageUtils.addStoredSubstance`) and the tier's capacity.
pub(super) fn item_tooltip(stack: &ItemStack, lines: &mut Vec<(Text, u32)>) -> bool {
    let Some(tier) = tier_of(&stack.item) else {
        return false;
    };
    let Some(registry) = mek_chemicals::loaded() else {
        return true;
    };
    let config = mek_ui::settings();
    let (_, stored) = item_stored(stack, &registry).unwrap_or((tier, ChemicalStack::EMPTY));
    let at = lines.len().min(1);
    let creative = tier == ChemicalTankTier::Creative;
    let held = match stored.id().and_then(|id| registry.get(id)) {
        None => "§7Empty".to_owned(),
        Some(c) if creative => format!("§eChemical: §6{}: §7Infinite", mek_chemicals::name(c)),
        Some(c) => format!("§eChemical: §6{}: §7{} mB", mek_chemicals::name(c), display::thousands(stored.amount().max(0) as u64)),
    };
    let capacity = if creative { "Infinite".to_owned() } else { format!("{} mB", display::thousands(config.chemical_tank(tier).0.max(0) as u64)) };
    lines.insert(at, (Text::legacy(&held), u32::MAX));
    lines.insert(at + 1, (Text::legacy(&format!("§9Capacity: §7{capacity}")), u32::MAX));
    true
}

/// `isBarVisible`, `getBarWidth`, `getBarColor`: a single tank holding a
/// chemical shows how full it is in its chemical's colour.
pub(super) fn item_bar(stack: &ItemStack, bar: &mut Option<(i32, u32)>) -> bool {
    let Some(tier) = tier_of(&stack.item) else {
        return false;
    };
    let Some(registry) = mek_chemicals::loaded() else {
        return true;
    };
    let Some((_, stored)) = item_stored(stack, &registry) else {
        return true;
    };
    let Some(chemical) = stored.id().and_then(|id| registry.get(id)) else {
        return true;
    };
    if stack.count == 1 {
        let capacity = mek_ui::settings().chemical_tank(tier).0;
        *bar = Some((display::bar_width(stored.amount().max(0) as u64, capacity.max(1) as u64), chemical.color & 0xFF_FFFF));
    }
    true
}
