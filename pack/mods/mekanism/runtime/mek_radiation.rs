// Mekanism's radiation (`mekanism_energy::radiation`, `RadiationManager`): each level's sources, kept in its
// attachments as `mekanism:radiation_data` and decayed on one level tick in 20; what radiates (a broken block holding
// radioactive chemicals, a fission reactor's waste with no room and its meltdown); a player's dose (the attachment
// `mekanism:radiation`, lost on death as Mekanism's is): on one of their ticks in 20 an hour's share of the level they
// stand in, less their hazmat suit's shielding, then its decay, and from a severity drawn above
// `negativeEffectsMinSeverity` the hurt (`mekanism:radiation`) and the exhaustion it does; the Geiger Counter and the
// Dosimeter (`ItemGeigerCounter`, `ItemDosimeter`: a right click in the air), and Mekanism's `/mek radiation`
// (`add`, `get`, `heal`, `removeAll`, `addEntity`, `reduce`, for the player) with `/mek radiation probe` beside them.

use super::{Game, mek_world, native_server::NativeServerPlayer};
use horizon_client::{items::ItemStack, text::Text};
use horizon_world::{
    attachments::{AttachmentCodec, AttachmentError, AttachmentType, Holder},
    nbt::{Compound, JavaString, Tag},
    player_inventory::EquipmentSlot,
};
use mekanism_energy::{
    chemical::{ChemicalRegistry, ChemicalStack},
    radiation::{self, BASELINE, RadiationConfig, Sources},
};
use serde_json::{Value, json};
use std::{cell::RefCell, collections::HashMap};

pub(super) const GEIGER: &str = "mekanism:geiger_counter";
pub(super) const DOSIMETER: &str = "mekanism:dosimeter";
const DAMAGE: &str = "mekanism:radiation";

fn identity(c: &Compound) -> Result<Compound, AttachmentError> {
    Ok(c.clone())
}

/// `MekanismAttachmentTypes.RADIATION_LEVEL_DATA`: `{sources: [{pos: [I; x, y, z], radiation}]}`.
static LEVEL_DATA: AttachmentType<Compound> = AttachmentType {
    namespace: "mekanism",
    name: "radiation_data",
    default: Compound::new,
    codec: Some(AttachmentCodec { encode: identity, decode: identity }),
    copy_on_death: false,
    sync: false,
};

/// `MekanismAttachmentTypes.RADIATION`: a living entity's dose, `{radiation}`, kept only above the baseline and not
/// over death.
static DOSE: AttachmentType<Compound> = AttachmentType {
    namespace: "mekanism",
    name: "radiation",
    default: Compound::new,
    codec: Some(AttachmentCodec { encode: identity, decode: identity }),
    copy_on_death: false,
    sync: false,
};

#[derive(Default)]
struct Dim {
    sources: Sources,
    loaded: bool,
    dirty: bool,
    decays: u64,
}

thread_local! {
    static DIMS: RefCell<HashMap<String, Dim>> = RefCell::new(HashMap::new());
}

fn key(name: &str) -> JavaString {
    JavaString::from(name)
}

fn with_dim<R>(dimension: &str, f: impl FnOnce(&mut Dim) -> R) -> R {
    DIMS.with(|d| f(d.borrow_mut().entry(dimension.to_owned()).or_default()))
}

fn config() -> RadiationConfig {
    mek_world::energy_config().0.radiation
}

/// `isGlobalRadiationEnabled`.
pub(super) fn enabled() -> bool {
    config().enabled
}

/// `native_world_lifecycle`: a world opening or closing starts with no sources in memory.
pub(super) fn lifecycle() {
    DIMS.with(|d| d.borrow_mut().clear());
}

/// `ChemicalStack.getRadioactivity`.
pub(super) fn stack_radioactivity(registry: &ChemicalRegistry, stack: &ChemicalStack) -> f64 {
    match stack.id().and_then(|id| registry.get(id)) {
        Some(c) if !stack.is_empty() => radiation::radioactivity(c.radioactivity, stack.amount()),
        _ => 0.0,
    }
}

/// `RadiationManager.radiate(level, pos, magnitude)`, radiation on.
pub(super) fn radiate(dimension: &str, pos: [i32; 3], magnitude: f64) {
    if !enabled() {
        return;
    }
    with_dim(dimension, |d| {
        d.sources.radiate(pos, magnitude);
        d.dirty = true;
    });
}

/// `dumpRadiation(level, pos, tanks, false)`: each radioactive stack radiates at `pos`. Whether one did.
pub(super) fn dump(dimension: &str, pos: [i32; 3], stacks: &[ChemicalStack], registry: &ChemicalRegistry) -> bool {
    let mut any = false;
    for s in stacks {
        let r = stack_radioactivity(registry, s);
        if r > 0.0 {
            radiate(dimension, pos, r);
            any = true;
        }
    }
    any
}

/// A broken Mekanism block that holds chemicals (`TileEntityMekanism.blockRemoved` with `shouldDumpRadiation`: every
/// one with chemical tanks but a creative chemical tank) radiates what of them is radioactive.
pub(super) fn block_removed(dimension: &str, pos: [i32; 3], id: &str, nbt: &Compound) {
    // Tubes aren't Mekanism tiles: their network spreads what the last of them held (not ported).
    if id == "mekanism:creative_chemical_tank" || super::mek_tubes::tier_of(id).is_some() || !enabled() || nbt.get(&key("chemical_tanks")).is_none() {
        return;
    }
    let Some(registry) = super::mek_chemicals::loaded() else { return };
    let stacks = super::mek_chemicals::read_tanks(nbt, 16, &registry);
    dump(dimension, pos, &stacks, &registry);
}

/// What a broken block's drop keeps of its `chemical_tanks` (`collectChemicalTanks`, radiation on and the block one
/// that dumps): the radioactive stacks are left out (they radiated).
pub(super) fn drop_tanks(id: &str, tanks: &Tag) -> Option<Tag> {
    let registry = super::mek_chemicals::loaded();
    let (Some(registry), Tag::List(list)) = (registry, tanks) else { return Some(tanks.clone()) };
    if id == "mekanism:creative_chemical_tank" || !enabled() {
        return Some(tanks.clone());
    }
    let kept: Vec<Tag> = list
        .iter()
        .filter(|t| match t {
            Tag::Compound(c) => match c.get(&key("stored")) {
                Some(Tag::Compound(stored)) => stack_radioactivity(&registry, &super::mek_chemicals::stack_of(stored, &registry)) <= 0.0,
                _ => true,
            },
            _ => true,
        })
        .cloned()
        .collect();
    (!kept.is_empty()).then_some(Tag::List(kept))
}

// ---- The level's sources -------------------------------------------------------------------------------------------

/// Loads the level's saved sources once (beside any radiated before it).
fn ensure_loaded(game: &mut Game<'_>, dimension: &str) {
    if with_dim(dimension, |d| d.loaded) {
        return;
    }
    let data = game.native_attachments(Holder::Level).ok().and_then(|a| a.get_existing(&LEVEL_DATA).ok().flatten());
    let mut saved = Vec::new();
    if let Some(Tag::List(list)) = data.as_ref().and_then(|d| d.get(&key("sources"))) {
        for entry in list {
            if let Tag::Compound(c) = entry
                && let Some(Tag::IntArray(p)) = c.get(&key("pos"))
                && p.len() == 3
                && let Some(Tag::Double(m)) = c.get(&key("radiation"))
            {
                saved.push(([p[0], p[1], p[2]], f64::from_bits(*m)));
            }
        }
    }
    with_dim(dimension, |d| {
        let fresh = std::mem::take(&mut d.sources);
        for (p, m) in saved {
            d.sources.radiate(p, m);
        }
        for (p, m) in fresh.iter() {
            d.sources.radiate(p, m);
        }
        d.loaded = true;
    });
}

fn save(game: &mut Game<'_>, dimension: &str) {
    let Some(list) = with_dim(dimension, |d| {
        if !d.loaded || !std::mem::take(&mut d.dirty) {
            return None;
        }
        Some(
            d.sources
                .iter()
                .map(|(p, m)| {
                    let mut c = Compound::new();
                    c.insert(key("pos"), Tag::IntArray(p.to_vec()));
                    c.insert(key("radiation"), Tag::Double(m.to_bits()));
                    Tag::Compound(c)
                })
                .collect::<Vec<_>>(),
        )
    }) else {
        return;
    };
    let Ok(a) = game.native_attachments(Holder::Level) else { return };
    let kept = if list.is_empty() {
        a.remove(&LEVEL_DATA).map(|_| ())
    } else {
        let mut data = Compound::new();
        data.insert(key("sources"), Tag::List(list));
        a.set(&LEVEL_DATA, data).map(|_| ())
    };
    if let Err(e) = kept {
        log::warn!("[mekanism] the radiation sources weren't kept: {e:?}");
    }
}

/// `native_level_tick_pre`: on one tick in 20 (`RadiationManager.onTickPre`'s draw) the level's sources decay.
pub(super) fn level_tick_pre(game: &mut Game<'_>, dimension: &str) {
    let config = config();
    if !config.enabled {
        return;
    }
    ensure_loaded(game, dimension);
    if with_dim(dimension, |d| d.sources.is_empty()) {
        return;
    }
    if super::mek_machines::draw_random() * f64::from(radiation::ONE_IN) < 1.0 {
        with_dim(dimension, |d| {
            d.sources.decay(config.source_decay_rate);
            d.decays += 1;
            d.dirty = true;
        });
    }
}

/// `native_level_tick_post`: what changed is kept in the level's attachments.
pub(super) fn level_ticked(game: &mut Game<'_>, dimension: &str) {
    if with_dim(dimension, |d| d.dirty) {
        ensure_loaded(game, dimension);
        save(game, dimension);
    }
}

/// `getRadiationLevelAndMaxMagnitude` at a block of the level.
fn level_at(game: &mut Game<'_>, dimension: &str, at: [i32; 3]) -> (f64, f64) {
    ensure_loaded(game, dimension);
    let range = config().block_range();
    with_dim(dimension, |d| d.sources.level_and_max(at, range))
}

// ---- Doses ---------------------------------------------------------------------------------------------------------

fn dose_of(game: &mut Game<'_>, uuid: u128) -> f64 {
    let data = game.native_attachments(Holder::Player(uuid)).ok().and_then(|a| a.get_existing(&DOSE).ok().flatten());
    match data.as_ref().and_then(|d| d.get(&key("radiation"))) {
        Some(Tag::Double(v)) if f64::from_bits(*v) > BASELINE => f64::from_bits(*v),
        _ => BASELINE,
    }
}

fn set_dose(game: &mut Game<'_>, uuid: u128, dose: f64) {
    let Ok(a) = game.native_attachments(Holder::Player(uuid)) else { return };
    let kept = if dose > BASELINE {
        let mut c = Compound::new();
        c.insert(key("radiation"), Tag::Double(dose.to_bits()));
        a.set(&DOSE, c).map(|_| ())
    } else {
        a.remove(&DOSE).map(|_| ())
    };
    if let Err(e) = kept {
        log::warn!("[mekanism] a radiation dose wasn't kept: {e:?}");
    }
}

/// `RadiationUtil.getRadiationResistance`: the hazmat pieces worn (head, chest, legs, feet).
fn resistance(game: &Game<'_>, uuid: u128) -> f64 {
    let worn: Vec<ItemStack> = [EquipmentSlot::Head, EquipmentSlot::Chest, EquipmentSlot::Legs, EquipmentSlot::Feet]
        .into_iter()
        .filter_map(|slot| game.native_player_equipment(uuid, slot))
        .collect();
    radiation::resistance(worn.iter().map(|s| &*s.item))
}

/// `MekanismUtils.isPlayingMode`: survival or adventure.
fn playing(game_mode: i32) -> bool {
    matches!(game_mode, 0 | 2)
}

fn block_of(position: [f64; 3]) -> [i32; 3] {
    position.map(|v| v.floor() as i32)
}

/// `native_player_tick` (`PlayerExposure.updateEntityRadiation` for a server player): one tick in 20 the player takes
/// an hour's share of the level they stand in (playing, less their shielding) and the dose decays; then a dose
/// severe enough hurts (`mekanism:radiation`, on one roll in two) and tires them.
pub(super) fn player_tick(game: &mut Game<'_>) {
    let config = config();
    if !config.enabled {
        return;
    }
    let Some(p) = game.native_acting_player() else { return };
    let draw = super::mek_machines::draw_random;
    let before = dose_of(game, p.uuid);
    let mut dose = before;
    if draw() * f64::from(radiation::ONE_IN) < 1.0 {
        let (level, _) = level_at(game, &p.dimension, block_of(p.position));
        let resistance = resistance(game, p.uuid);
        dose = radiation::dose_event(dose, level, resistance, playing(p.game_mode), config.target_decay_rate);
    }
    if dose != before {
        set_dose(game, p.uuid, dose);
    }
    if p.health <= 0.0 || !playing(p.game_mode) {
        return;
    }
    if let Some(strength) = radiation::effect(dose, config.min_severity, draw()) {
        if draw() < 0.5 {
            let _ = game.native_hurt_entity(&p.dimension, p.entity_id as u32, strength, DAMAGE, None);
        }
        if let Some((level, saturation, exhaustion)) = game.native_player_food(p.uuid) {
            let _ = game.native_set_player_status(p.uuid, None, Some((level, saturation, exhaustion + strength)), None);
        }
    }
}

// ---- The Geiger Counter and the Dosimeter --------------------------------------------------------------------------

fn colored(value: String, magnitude: f64) -> Text {
    let color = horizon_client::text::named_color(radiation::severity_color(magnitude)).unwrap_or(0xFF_FFFF);
    Text::colored(value, color)
}

fn gray(key: &str, fallback: &str, args: &[Text]) -> Text {
    let gray = horizon_client::text::named_color("gray").unwrap_or(0xAA_AAAA);
    Text::translate_with_fallback(key, fallback, args).with_color(gray)
}

fn feedback(game: &mut Game<'_>, key: &str, fallback: &str, args: &[Text]) {
    game.native_feedback(gray(key, fallback, args));
}

fn decay_line(ticks: i64, magnitude: f64) -> Text {
    let time = radiation::hours_minutes(ticks, 20.0);
    gray("gui.mekanism.radiation_decay_time", "Time to Decay: %1$s", &[colored(time, magnitude)])
}

/// The Geiger Counter's lines (`ItemGeigerCounter.use`): the level where the player stands, and with decay timers
/// on and a source near the time the greatest of them takes to decay.
fn geiger_lines(game: &mut Game<'_>, dimension: &str, at: [i32; 3]) -> Vec<Text> {
    let config = config();
    let (level, max) = level_at(game, dimension, at);
    let mut out = vec![gray("gui.mekanism.radiation_exposure", "Radiation Exposure: %1$s", &[colored(radiation::display(level, true), level)])];
    if config.decay_timers && level > BASELINE {
        out.push(decay_line(radiation::decay_ticks(max, config.source_decay_rate), level));
    }
    out
}

/// The Dosimeter's lines (`ItemDosimeter.sendDosimeterLevel`): the player's dose and, above `MIN_MAGNITUDE`, the
/// time it takes to decay.
fn dosimeter_lines(dose: f64) -> Vec<Text> {
    let config = config();
    let dose = if config.enabled { dose } else { 0.0 };
    let mut out = vec![gray("gui.mekanism.radiation_exposure", "Radiation Exposure: %1$s", &[colored(radiation::display(dose, false), dose)])];
    if config.decay_timers && dose > radiation::MIN_MAGNITUDE {
        out.push(decay_line(radiation::decay_ticks(dose, config.target_decay_rate), dose));
    }
    out
}

/// `native_use_item`: a Geiger Counter or a Dosimeter used in the air, not sneaking, tells the player its reading.
pub(super) fn use_item(game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool) {
    let item = &*stack.item;
    if *claimed || (item != GEIGER && item != DOSIMETER) || game.native_sneaking() {
        return;
    }
    let Some(p) = game.native_acting_player() else { return };
    *claimed = true;
    let lines = if item == GEIGER { geiger_lines(game, &p.dimension, block_of(p.position)) } else { dosimeter_lines(dose_of(game, p.uuid)) };
    for line in lines {
        let _ = game.native_tell(p.uuid, line);
    }
}

// ---- /mek radiation ------------------------------------------------------------------------------------------------

/// `/mek radiation ...` as Mekanism's `RadiationCommand` (for the player who runs it, at a block position), answering
/// its line and then the bundle's `MEK {json}`; `probe [x y z]` gives the sources, the level there and the dose.
pub(super) fn command(game: &mut Game<'_>, player: &NativeServerPlayer, words: &[&str]) -> Value {
    let dimension = player.dimension.clone();
    let here = block_of(player.position);
    let pos = |at: &[&str]| -> Option<[i32; 3]> {
        if at.len() < 3 {
            return Some(here);
        }
        let c = |w: &str, from: f64| match w.strip_prefix('~') {
            Some("") => Some(from.floor() as i32),
            Some(rest) => Some((from + rest.parse::<f64>().ok()?).floor() as i32),
            None => w.parse::<f64>().ok().map(|v| v.floor() as i32),
        };
        Some([c(at[0], player.position[0])?, c(at[1], player.position[1])?, c(at[2], player.position[2])?])
    };
    ensure_loaded(game, &dimension);
    let dim_name = dimension.strip_prefix("minecraft:").map_or(dimension.clone(), |d| match d {
        "overworld" => "Overworld".to_owned(),
        "the_nether" => "The Nether".to_owned(),
        "the_end" => "The End".to_owned(),
        other => other.to_owned(),
    });
    match words.first().copied() {
        Some("add") => {
            let Some(m) = words.get(1).and_then(|w| w.parse::<f64>().ok()).filter(|m| *m > 0.0 && *m <= 10_000.0) else {
                return json!({ "error": "usage: /mek radiation add <magnitude> [x y z]" });
            };
            let Some(at) = pos(&words[2..]) else { return json!({ "error": "bad position" }) };
            radiate(&dimension, at, m);
            let place = Text::literal(format!("{}, {}, {}", at[0], at[1], at[2]));
            feedback(
                game,
                "command.mekanism.radiation.add",
                "Added %1$s radiation at (%2$s) in %3$s.",
                &[colored(radiation::display(m, true), m), place, Text::literal(dim_name)],
            );
            json!({ "added": m, "pos": at })
        }
        Some("get") => {
            let Some(at) = pos(&words[1..]) else { return json!({ "error": "bad position" }) };
            let (level, max) = level_at(game, &dimension, at);
            let place = Text::literal(format!("{}, {}, {}", at[0], at[1], at[2]));
            let shown = radiation::display(level, true);
            feedback(
                game,
                "command.mekanism.radiation.get",
                "Current radiation at (%1$s) in %2$s: %3$s",
                &[place, Text::literal(dim_name), colored(shown.clone(), level)],
            );
            json!({ "pos": at, "level": level, "max": max, "shown": shown })
        }
        Some("removeAll") => {
            with_dim(&dimension, |d| {
                d.sources.clear();
                d.dirty = true;
            });
            feedback(game, "command.mekanism.radiation.remove_all", "Removed all radiation sources.", &[]);
            json!({ "removed": true })
        }
        Some("heal") => {
            set_dose(game, player.uuid, BASELINE);
            feedback(game, "command.mekanism.radiation.clear", "Cleared player radiation.", &[]);
            json!({ "dose": BASELINE })
        }
        Some(sub @ ("addEntity" | "reduce")) => {
            let Some(m) = words.get(1).and_then(|w| w.parse::<f64>().ok()).filter(|m| *m > 0.0 && *m <= 10_000.0) else {
                return json!({ "error": format!("usage: /mek radiation {sub} <magnitude>") });
            };
            let dose = dose_of(game, player.uuid);
            let after = if sub == "addEntity" { radiation::radiated(dose, m) } else { (dose - m).max(BASELINE) };
            set_dose(game, player.uuid, after);
            let shown = colored(radiation::display(m, false), m);
            if sub == "addEntity" {
                feedback(game, "command.mekanism.radiation.add_entity", "Added %1$s radiation to player.", &[shown]);
            } else {
                feedback(game, "command.mekanism.radiation.reduce", "Reduced player radiation by %1$s.", &[shown]);
            }
            json!({ "dose": after })
        }
        Some("probe") => {
            let Some(at) = pos(&words[1..]) else { return json!({ "error": "bad position" }) };
            let (level, max) = level_at(game, &dimension, at);
            let dose = dose_of(game, player.uuid);
            let (sources, decays) = with_dim(&dimension, |d| {
                (d.sources.iter().map(|(p, m)| json!({ "pos": p, "radiation": m })).collect::<Vec<_>>(), d.decays)
            });
            let resistance = resistance(game, player.uuid);
            json!({ "pos": at, "level": level, "max": max, "sources": sources, "decays": decays, "dose": dose, "resistance": resistance })
        }
        _ => json!({ "error": "usage: /mek radiation add|get|heal|removeAll|addEntity|reduce|probe ..." }),
    }
}
