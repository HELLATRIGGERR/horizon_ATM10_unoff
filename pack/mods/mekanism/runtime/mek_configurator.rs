// Mekanism's Configurator in the game (support module `crate::game::mek_configurator`
// of the `mekanism` native bundle). The rules are `mekanism_energy::configurator`
// (MIT, from Mekanism 10.7.19, Copyright (c) 2017-2025 Aidan C. Brady); this
// module is the glue: the mode kept in the item (`custom_data.mekanism.mode`,
// and `custom_model_data` for the model it shows), its uses on blocks through
// `native_use_item_on` (where a sneaking use reaches the item, with hook points
// API 166's `native_sneaking`): a machine's, cube's or tank's side
// configuration bumped through the block's own side button, a transmitter's
// side or a transporter's colour, a fluid tank turned on or off, a machine's
// items taken out, a block turned or taken up; a plain use of the rotate mode
// turns a block clockwise before it opens its screen (`native_right_click_block`,
// as `tryWrenchRotate` does in the block's `useItemOn`). `/mek configurator
// <mode>` sets the held one's mode: Mekanism's mode key and sneak-scroll
// aren't ported.

use super::{Game, mek_fluid_tanks, mek_transporters, mek_world};
use horizon_client::{items::ItemStack, text::Text};
use horizon_world::nbt::{Compound, Tag};
use mekanism_energy::{
    configurator::{self, Mode, Transmission},
    menu::{Button, Kind},
    network::{Connection, SIDES},
    sides::{DataType, Direction, RelativeSide},
    transporter::Color,
};
use serde_json::{Value, json};

const CONFIGURATOR: &str = "mekanism:configurator";
const CUSTOM_DATA: &str = "minecraft:custom_data";
const MODEL: &str = "minecraft:custom_model_data";

/// The mode a configurator stack is in (`mekanism:configurator_mode`, here
/// `custom_data.mekanism.mode`); configurating items by default.
pub(super) fn mode_of(stack: &ItemStack) -> Mode {
    mek_world::item_data(stack).and_then(|d| mek_world::number(d.get(&mek_world::key("mode")))).map_or(Mode::ConfigurateItems, |v| Mode::by_id(v as i32))
}

/// Sets a configurator stack's mode, and the model it shows.
pub(super) fn set_mode(stack: &mut ItemStack, mode: Mode) {
    let mut data = mek_world::item_data(stack).unwrap_or_default();
    data.insert(mek_world::key("mode"), Tag::Int(mode.id()));
    let mut custom = match stack.data().and_then(|d| d.other(CUSTOM_DATA)) {
        Some(Tag::Compound(c)) => c.clone(),
        _ => Compound::new(),
    };
    custom.insert(mek_world::key("mekanism"), Tag::Compound(data));
    stack.data_mut().set_other(CUSTOM_DATA, Some(Tag::Compound(custom)));
    let mut model = Compound::new();
    model.insert(mek_world::key("floats"), Tag::List(vec![Tag::Float((mode.model() as f32).to_bits())]));
    stack.data_mut().set_other(MODEL, Some(Tag::Compound(model)));
}

fn say(game: &mut Game<'_>, line: String) {
    game.native_feedback(Text::literal(line));
}

fn side_of(face: [i32; 3]) -> Option<Direction> {
    SIDES.iter().position(|s| *s == face).map(|i| Direction::ALL[i])
}

/// The side config kind of a transmission type, as the screens' buttons know them.
fn kind_of(t: Transmission) -> Option<Kind> {
    match t {
        Transmission::Energy => Some(Kind::Energy),
        Transmission::Item => Some(Kind::Items),
        Transmission::Chemical => Some(Kind::Chemicals),
        Transmission::Fluid | Transmission::Heat => None,
    }
}

/// The data type of `side` in the side configuration of `t` saved in `nbt`, if the block has one.
fn data_type(nbt: &Compound, t: Transmission, side: RelativeSide) -> Option<DataType> {
    let Some(Tag::Compound(config)) = nbt.get(&mek_world::key("component_config")) else { return None };
    match config.get(&mek_world::key(&format!("config{}", t.id())))? {
        Tag::IntArray(ids) => ids.get(side.index()).map(|id| DataType::by_id(*id)),
        _ => None,
    }
}

/// Runs the block's own server button `id` on a copy of its entity, put back after.
fn press(game: &mut Game<'_>, pos: [i32; 3], id: i32) {
    let Some(mut entity) = game.native_with_mod_block_entity(pos, |m| m.clone()) else { return };
    mek_world::button(game, pos, &mut entity, id);
    game.native_with_mod_block_entity(pos, |m| {
        m.nbt = entity.nbt.clone();
        m.data = entity.data.clone();
        m.items = entity.items.clone();
    });
}

/// `native_use_item_on` with a configurator: `ItemConfigurator.useOn` (and a
/// Mekanism block's wrench, which takes a sneaking use of the wrench mode).
pub(super) fn use_on(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3], face: [i32; 3], claimed: &mut bool) {
    if *claimed || &*stack.item != CONFIGURATOR {
        return;
    }
    let Some(side) = side_of(face) else { return };
    let sneaking = game.native_sneaking();
    let mode = mode_of(stack);
    let Some((id, nbt)) = game.native_with_mod_block_entity(pos, |m| (m.id.clone(), m.nbt.clone())) else { return };
    if !id.starts_with("mekanism") {
        return;
    }
    let dimension = game.native_dimension();
    match mode {
        _ if mode.transmission().is_some() => {
            let t = mode.transmission().expect("configurating");
            let facing = mek_world::facing_of(game, pos).unwrap_or(Direction::North);
            let relative = RelativeSide::from_directions(facing, side);
            if let Some(kind) = kind_of(t)
                && let Some(old) = data_type(&nbt, t, relative)
            {
                *claimed = true;
                if !sneaking {
                    say(game, format!("Current {} behavior: {}", t.name(), old.name()));
                    return;
                }
                press(game, pos, Button::Side { kind, side: relative.index(), forward: true }.id());
                let now = game.native_with_mod_block_entity(pos, |m| data_type(&m.nbt, t, relative)).flatten().unwrap_or(old);
                if now != old {
                    say(game, format!("{} behavior bumped to: {}", t.name(), now.name()));
                }
                return;
            }
            // `IConfigurable`: transmitters and fluid tanks.
            if mek_fluid_tanks::tier_of(&id).is_some() {
                if sneaking {
                    *claimed = true;
                    let on = mek_fluid_tanks::toggle(game, pos).unwrap_or(false);
                    say(game, format!("Configure State: {}", if on { "On" } else { "Off" }));
                }
                return;
            }
            let Some(raw) = mek_world::transmitter_connection(game, pos, side) else { return };
            *claimed = true;
            let transporter = mek_transporters::takes_color(&id);
            if !sneaking {
                // `onRightClick`: a transporter tells its colour; redstone sensitivity isn't ported.
                if transporter {
                    let color = mek_transporters::color(dimension, pos).flatten();
                    say(game, format!("Current color: {}", color.map_or("None", Color::name)));
                }
                return;
            }
            // `onSneakRightClick` on the core (a transmitter's outline here is its core only): a transporter's side
            // that connects bumps its colour (`onConfigure`), any other side its connection type.
            if transporter && raw != Connection::None {
                let old = mek_transporters::color(dimension, pos).flatten();
                let new = Color::increment(old);
                mek_transporters::set_color(game, pos, new);
                say(game, format!("Color bumped to: {}", new.map_or("None", Color::name)));
                return;
            }
            let next = configurator::next_connection(raw);
            mek_world::set_transmitter_connection(game, pos, side, next);
            say(game, format!("Connection type changed to: {}", connection_name(next)));
        }
        Mode::Empty => {
            if !sneaking && has_screen(&id) {
                return;
            }
            let items = game.native_with_mod_block_entity(pos, |m| std::mem::take(&mut m.items)).unwrap_or_default();
            if items.is_empty() {
                return;
            }
            *claimed = true;
            let len = items.len();
            let out = mek_world::beside(pos, side);
            for stack in items.into_iter().flatten().filter(|s| s.count > 0) {
                game.native_pop_resource(out, stack);
            }
            game.native_with_mod_block_entity(pos, |m| m.items = vec![None; len]);
        }
        Mode::Rotate => {
            if !sneaking {
                return;
            }
            *claimed = true;
            mek_world::set_facing(game, pos, side.opposite());
        }
        Mode::Wrench => {
            if !sneaking {
                return;
            }
            *claimed = true;
            let Some(item) = mek_world::dismantle(game, pos) else { return };
            let player = game.native_acting_id();
            if let Some(rest) = game.native_give_stack(player, item) {
                game.native_pop_resource(pos, rest);
            }
        }
        _ => {}
    }
}

/// Whether a block opens a screen on a plain use (a plain use of the empty mode reaches only those that don't).
fn has_screen(id: &str) -> bool {
    mek_transporters::tier_of(id).is_none() && super::mek_pipes::tier_of(id).is_none() && mek_fluid_tanks::tier_of(id).is_none()
}

fn connection_name(c: Connection) -> &'static str {
    match c {
        Connection::Normal => "Normal",
        Connection::Push => "Push",
        Connection::Pull => "Pull",
        Connection::None => "None",
    }
}

/// `native_right_click_block` with a configurator in the rotate mode, not
/// sneaking: the block's `useItemOn` turns it clockwise (`tryWrenchRotate`)
/// and opens no screen.
pub(super) fn right_click(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if *canceled || game.native_sneaking() {
        return;
    }
    let Some(stack) = game.native_used_stack().clone() else { return };
    if &*stack.item != CONFIGURATOR || mode_of(&stack) != Mode::Rotate {
        return;
    }
    let id = game.native_block(pos);
    if !id.starts_with("mekanism") {
        return;
    }
    let Some(facing) = mek_world::facing_of(game, pos) else { return };
    let all_ways = mek_world::faces_any_way(game, pos);
    if mek_world::set_facing(game, pos, configurator::rotate(facing, all_ways)) {
        *canceled = true;
    }
}

/// `/mek configurator <mode>`: the held configurator's mode.
pub(super) fn command(game: &mut Game<'_>, name: Option<&str>) -> Value {
    let Some(mode) = name.and_then(Mode::from_name) else {
        return json!({ "error": "usage: /mek configurator <items|fluids|chemicals|energy|heat|empty|rotate|wrench>" });
    };
    let hand = game.native_used_stack();
    match hand.as_mut() {
        Some(stack) if &*stack.item == CONFIGURATOR => {
            set_mode(stack, mode);
            json!({ "mode": mode.id(), "name": mode.name() })
        }
        _ => json!({ "error": "no configurator in hand" }),
    }
}
