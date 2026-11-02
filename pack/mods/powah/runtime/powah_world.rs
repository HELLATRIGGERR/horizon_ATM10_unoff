// Powah's Furnators in the game (support module `crate::game::powah_world` of
// the `powah` native bundle). The rules are the bundle's crate `powah_core`;
// this module is the glue: the block entity and its saved compound, the fuel
// slot, the neighbours' Forge Energy, the settings from `config/powah.json5`,
// the menu's data slots (its screen is `powah_ui`) and `/powah probe` for tests.
// Server hooks run on the thread of the `Game` that simulates the world, so the
// settings are kept per thread, read when a world opens and dropped when it
// closes. Powah is LGPL-3.0; see this bundle's README.md.

use super::{
    Game,
    native_hooks::NativeFeInfo,
    native_lifecycle::WorldPhase,
    native_server::{NativeCommand, NativeServerPlayer},
};
use horizon_client::{
    containers::{ModBlockEntity, ModContainer},
    item_data,
    items::ItemStack,
    text::Text,
};
use horizon_core::geometry::Direction;
use horizon_world::{
    level::flags,
    nbt::{Compound, JavaString, Tag},
};
use powah_core::{
    config::{GeneratorSpec, PowahConfig},
    furnator::{self, FUEL_SLOT, Furnator, STORED_STATE},
    tier::Tier,
};
use std::cell::RefCell;

/// The settings of the open world, and whether `powah.json5` was there.
struct Settings {
    config: PowahConfig,
    from_file: bool,
}

thread_local! {
    static SETTINGS: RefCell<Option<Settings>> = const { RefCell::new(None) };
}

fn read_settings() -> Settings {
    match Game::native_config_dir() {
        Ok(dir) => Settings {
            config: PowahConfig::load(&dir),
            from_file: dir.join("powah.json5").is_file(),
        },
        Err(e) => {
            log::warn!("[powah] no config folder ({e}): Powah's defaults");
            Settings { config: PowahConfig::default(), from_file: false }
        }
    }
}

/// Runs `read` on the settings, read on first use if the world's opening didn't.
fn with_settings<R>(read: impl FnOnce(&Settings) -> R) -> R {
    SETTINGS.with(|s| read(s.borrow_mut().get_or_insert_with(read_settings)))
}

/// What a tier of Furnator runs on.
pub(super) fn spec_of(tier: Tier) -> GeneratorSpec {
    with_settings(|s| s.config.furnators.spec(tier))
}

/// `native_world_lifecycle`: settings are read as a world opens (ATM10's file,
/// copied by `pack/scripts/sync-atm10.ts`) and dropped as it closes.
pub(super) fn lifecycle(phase: WorldPhase) {
    let fresh = matches!(phase, WorldPhase::WorldOpenReady).then(read_settings);
    SETTINGS.with(|s| *s.borrow_mut() = fresh);
}

// ---- The saved compound, in Powah's names ---------------------------------

const ENERGY: &str = "energy_stored_main_energy";
const CARBON: &str = "carbon_ticks";
const CARBON_MAX: &str = "carbon_max_ticks";
const BURNING: &str = "burning";
/// Six sides, by `Direction`: 1 is Powah's "extract only", all a Furnator has.
const SIDE_TYPES: &str = "side_transfer_type";
const REDSTONE: &str = "redstone_mode";

fn key(name: &str) -> JavaString {
    JavaString::from(name)
}

fn number(tag: Option<&Tag>) -> Option<i64> {
    match tag? {
        Tag::Byte(v) => Some(i64::from(*v)),
        Tag::Short(v) => Some(i64::from(*v)),
        Tag::Int(v) => Some(i64::from(*v)),
        Tag::Long(v) => Some(*v),
        Tag::Float(bits) => Some(f32::from_bits(*bits) as i64),
        Tag::Double(bits) => Some(f64::from_bits(*bits) as i64),
        _ => None,
    }
}

fn read(nbt: &Compound) -> Furnator {
    let get = |name| number(nbt.get(&key(name))).unwrap_or(0).max(0);
    Furnator {
        stored: get(ENERGY),
        carbon: get(CARBON),
        carbon_max: get(CARBON_MAX),
        burning: get(BURNING) != 0,
    }
}

fn write(nbt: &mut Compound, f: &Furnator) {
    nbt.insert(key(ENERGY), Tag::Long(f.stored));
    nbt.insert(key(CARBON), Tag::Double((f.carbon as f64).to_bits()));
    nbt.insert(key(CARBON_MAX), Tag::Double((f.carbon_max as f64).to_bits()));
    nbt.insert(key(BURNING), Tag::Byte(i8::from(f.burning)));
}

/// What an item of a Furnator keeps for the block, when a player broke one.
fn item_state(stack: &ItemStack) -> Option<Furnator> {
    let Tag::Compound(components) = item_data::encode_components(stack)? else {
        return None;
    };
    let Tag::Compound(state) = components.get(&key(STORED_STATE))? else {
        return None;
    };
    Some(read(state))
}

/// The energy an item of a Furnator holds.
pub(super) fn item_energy(stack: &ItemStack) -> i64 {
    item_state(stack).map_or(0, |f| f.stored)
}

// ---- Block entities -------------------------------------------------------

/// `native_block_entity`: a new, empty Furnator.
pub(super) fn new_entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if entity.is_some() || Tier::of_furnator(block).is_none() {
        return;
    }
    let mut nbt = Compound::new();
    write(&mut nbt, &Furnator::default());
    nbt.insert(key(SIDE_TYPES), Tag::IntArray(vec![1; 6]));
    nbt.insert(key(REDSTONE), Tag::Int(0));
    *entity = Some((block.to_owned(), nbt));
}

/// `native_block_entity_ticks`: Furnators tick.
pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if Tier::of_furnator(block).is_some() {
        *ticks = true;
    }
}

/// `native_block_entity_container`: the charge slot (0) and the fuel slot (1), in
/// a hopper's five-slot menu whose other three slots the screen leaves out. Every
/// face reaches both; hoppers put into the fuel slot and take from both.
pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if container.is_some() || Tier::of_furnator(block).is_none() {
        return;
    }
    *container = Some(ModContainer {
        slots: 5,
        faces: std::array::from_fn(|_| vec![0, 1]),
        inputs: vec![FUEL_SLOT],
        outputs: vec![0, 1],
        menu: Some("minecraft:hopper".into()),
    });
}

/// `native_block_entity_slot_accepts`: fuel only, in the fuel slot.
pub(super) fn slot_accepts(block: &str, slot: usize, fuel: bool, accepts: &mut bool) {
    if Tier::of_furnator(block).is_some() && !furnator::slot_accepts(slot, fuel) {
        *accepts = false;
    }
}

/// `native_block_entity_drop`: a player's break keeps the energy and the fuel
/// burning on the item, full or empty, as `Block.playerDestroy` does in Powah.
/// An explosion or a command drops the loot table's plain item.
pub(super) fn drop_components(block: &str, entity: &ModBlockEntity, by_player: bool, components: &mut Compound) {
    if !by_player || Tier::of_furnator(block).is_none() {
        return;
    }
    let f = read(&entity.nbt);
    let mut state = Compound::new();
    state.insert(key(ENERGY), Tag::Long(f.stored));
    state.insert(key(CARBON), Tag::Double((f.carbon as f64).to_bits()));
    state.insert(key(CARBON_MAX), Tag::Double((f.carbon_max as f64).to_bits()));
    components.insert(key(STORED_STATE), Tag::Compound(state));
}

/// `native_place_block`: what the item keeps goes back into the block.
pub(super) fn placed(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3]) {
    let Some(tier) = Tier::of_furnator(&stack.item) else {
        return;
    };
    let Some(kept) = item_state(stack) else {
        return;
    };
    let capacity = spec_of(tier).capacity;
    game.native_with_mod_block_entity(pos, |m| {
        if Tier::of_furnator(&m.id).is_some() {
            let mut f = read(&m.nbt);
            f.stored = kept.stored.min(capacity);
            f.carbon = kept.carbon;
            f.carbon_max = kept.carbon_max;
            write(&mut m.nbt, &f);
        }
    });
}

/// The fuel slot's next item, gone: what it is worth. An item that leaves
/// something behind (a lava bucket) leaves it in the slot.
fn take_fuel(game: &Game<'_>, entity: &mut ModBlockEntity, energy_per_fuel_tick: i64) -> Option<i64> {
    let slot = entity.items.get_mut(FUEL_SLOT)?;
    let stack = slot.as_mut()?;
    let burn = game.native_burn_time(stack);
    if burn <= 0 {
        return None;
    }
    stack.count -= 1;
    if stack.count == 0 {
        let rest = game.native_crafting_remainder(&stack.item);
        *slot = rest;
    }
    Some(furnator::fuel_value(burn, energy_per_fuel_tick))
}

/// `native_block_entity_tick`: burn, push, and what the menu shows. The block's
/// `lit` picks the model with the burning face; Java draws that face with a
/// renderer and has no such state, so neighbours aren't told.
pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    let Some(tier) = Tier::of_furnator(&entity.id) else {
        return;
    };
    let (spec, per_fuel_tick) = with_settings(|s| (s.config.furnators.spec(tier), s.config.energy_per_fuel_tick));
    let before = read(&entity.nbt);
    let mut f = before;
    if f.wants_fuel()
        && let Some(value) = take_fuel(game, entity, per_fuel_tick)
    {
        f.start_fuel(value);
    }
    f.generate(&spec);
    if f.stored > 0 {
        let dimension = game.native_dimension();
        f.push(&spec, |side, amount| {
            let face = Direction::ALL[side];
            let [dx, dy, dz] = face.normal();
            let at = [pos[0] + dx, pos[1] + dy, pos[2] + dz];
            game.native_fe_offer(dimension, at, Some(face.opposite()), amount, false, false)
        });
    }
    if f != before {
        write(&mut entity.nbt, &f);
    }
    if f.burning != before.burning {
        let lit = if f.burning { "true" } else { "false" };
        game.native_set_block_property_with(pos, "lit", lit, flags::UPDATE_CLIENTS);
    }
    entity.data.clear();
    f.view(&spec).pack_into(&mut entity.data);
}

// ---- Forge Energy ---------------------------------------------------------

/// `native_fe_storage`: every side, and the block itself, gives and never takes.
pub(super) fn fe_storage(entity: &ModBlockEntity, info: &mut Option<NativeFeInfo>) {
    if info.is_some() {
        return;
    }
    let Some(tier) = Tier::of_furnator(&entity.id) else {
        return;
    };
    *info = Some(NativeFeInfo {
        stored: read(&entity.nbt).stored,
        capacity: spec_of(tier).capacity,
        can_receive: false,
        can_extract: true,
    });
}

/// `native_fe_receive`: a generator takes no energy.
pub(super) fn fe_receive(entity: &ModBlockEntity, accepted: &mut Option<i64>) {
    if accepted.is_none() && Tier::of_furnator(&entity.id).is_some() {
        *accepted = Some(0);
    }
}

/// `native_fe_extract`: up to the tier's rate a call.
pub(super) fn fe_extract(entity: &mut ModBlockEntity, max: i64, simulate: bool, given: &mut Option<i64>) {
    if given.is_some() {
        return;
    }
    let Some(tier) = Tier::of_furnator(&entity.id) else {
        return;
    };
    let mut f = read(&entity.nbt);
    let out = f.extract(&spec_of(tier), max, simulate);
    if out > 0 && !simulate {
        write(&mut entity.nbt, &f);
    }
    *given = Some(out);
}

// ---- /powah probe ---------------------------------------------------------

/// `native_register_commands`.
pub(super) fn commands(commands: &mut Vec<NativeCommand>) {
    commands.push(NativeCommand::op("powah probe <pos:block_pos>", 2));
}

/// A coordinate of the command line: a number, or `~`/`~n` from the player.
fn coordinate(word: Option<&str>, from: f64) -> Option<i32> {
    let word = word?;
    match word.strip_prefix('~') {
        Some("") => Some(from.floor() as i32),
        Some(rest) => Some((from + rest.parse::<f64>().ok()?).floor() as i32),
        None => word.parse().ok(),
    }
}

/// `native_perform_command`: `/powah probe x y z` answers `POWAH {json}` with
/// what the Furnator there holds and runs on.
pub(super) fn perform(game: &mut Game<'_>, player: &NativeServerPlayer, command: &str, ran: &mut bool) {
    let mut words = command.split_whitespace();
    if *ran || words.next() != Some("powah") {
        return;
    }
    *ran = true;
    let usage = || serde_json::json!({ "error": "usage: /powah probe <x> <y> <z>" });
    let answer = match words.next() {
        Some("probe") => {
            let p = player.position;
            match (coordinate(words.next(), p[0]), coordinate(words.next(), p[1]), coordinate(words.next(), p[2])) {
                (Some(x), Some(y), Some(z)) => probe(game, [x, y, z]),
                _ => usage(),
            }
        }
        _ => usage(),
    };
    game.native_feedback(Text::literal(format!("POWAH {answer}")));
}

fn probe(game: &mut Game<'_>, pos: [i32; 3]) -> serde_json::Value {
    let found = game.native_with_mod_block_entity(pos, |m| (m.id.clone(), read(&m.nbt)));
    let Some((id, f)) = found else {
        return serde_json::json!({ "pos": pos, "error": "no mod block entity here" });
    };
    let Some(tier) = Tier::of_furnator(&id) else {
        return serde_json::json!({ "pos": pos, "id": id, "error": "not a Powah block" });
    };
    let spec = spec_of(tier);
    serde_json::json!({
        "pos": pos,
        "id": id,
        "tier": tier.name(),
        "energy": f.stored,
        "capacity": spec.capacity,
        "generation": spec.generation,
        "transfer": spec.transfer,
        "carbon": f.carbon,
        "carbon_max": f.carbon_max,
        "burning": f.burning,
        "facing": game.native_block_property(pos, "facing"),
        "lit": game.native_block_property(pos, "lit"),
        "from_file": with_settings(|s| s.from_file),
        "energy_per_fuel_tick": with_settings(|s| s.config.energy_per_fuel_tick),
    })
}
