//! Advanced AE 1.6.12-1.21.1 in the game (feature `ae2-advancedae`): the
//! Advanced Processing Pattern's directions (`AdvProcessingPattern`,
//! `EncodedAdvProcessingPattern`), the Small Advanced Pattern Provider's
//! directional push (`AdvPatternProviderLogic.pushPattern`,
//! `pushInputsDirectionally`, `sendStacksOut`) and the Advanced Pattern
//! Encoder's menu logic (`AdvPatternEncoderMenu`). Derived from Advanced AE,
//! https://github.com/pedroksl/AdvancedAE commit
//! `c322dad81e6f2ad94cb1fb861e73a7ba68e5907a`, LGPL-3.0; see
//! `ae2/addons/advancedae/`.
//!
//! The pattern is a processing pattern of its own item and component, so
//! AE2's decoding reads it (`Defs::add_processing_pattern`); what is Advanced
//! AE's own is the list of directions beside its sparse inputs, one per input
//! (`NULLDIR` where the input goes to the face the provider pushes to).
mod lab;

use crate::game::ae2_source_patterns as source;
use horizon_client::{ae2_generic_stack::GenericStack, ae2_world::Machine, ae2x::advancedae as rows, items::ItemStack};
use horizon_core::geometry::Direction;
use horizon_world::nbt::Tag;

/// `Direction.from3DDataValue`.
pub(in crate::game) fn direction(value: u8) -> Option<Direction> {
    Some(match value {
        0 => Direction::Down,
        1 => Direction::Up,
        2 => Direction::North,
        3 => Direction::South,
        4 => Direction::West,
        5 => Direction::East,
        _ => return None,
    })
}
/// `Direction.get3DDataValue`.
pub(in crate::game) fn direction_value(direction: Direction) -> u8 {
    match direction {
        Direction::Down => 0,
        Direction::Up => 1,
        Direction::North => 2,
        Direction::South => 3,
        Direction::West => 4,
        Direction::East => 5,
    }
}

/// `EncodedAdvProcessingPattern.directionList`: a direction per sparse input,
/// none for `NULLDIR` and for an entry that is no direction (the codec's
/// fault tolerance). None for an item that is no Advanced Processing Pattern.
pub(in crate::game) fn directions(pattern: &ItemStack) -> Option<Vec<Option<Direction>>> {
    if pattern.item.as_ref() != rows::ADV_PROCESSING_PATTERN {
        return None;
    }
    let component = pattern.data()?.other(rows::ENCODED_ADV_PROCESSING_PATTERN)?;
    let Some(Tag::List(entries)) = component.get("directionMap") else { return Some(Vec::new()) };
    Some(
        entries
            .iter()
            .map(|entry| match entry {
                Tag::String(name) => rows::nullable_direction(&name.to_string_lossy()).flatten().and_then(direction),
                _ => None,
            })
            .collect(),
    )
}

/// `AdvPatternProviderLogic.pushPattern`: the faces `pattern`'s sparse inputs
/// go to when `provider` pushes it, an Advanced AE provider pushing an
/// Advanced Processing Pattern (`directionalInputsSet`, true for any such
/// pattern with an input). None for any other provider or pattern: AE2's push.
pub(in crate::game) fn provider_input_directions(provider: &Machine, pattern: &ItemStack) -> Option<Vec<Option<Direction>>> {
    if !rows::directional_provider(provider.id()) {
        return None;
    }
    directions(pattern)
}

/// `AdvProcessingPattern.encode`: an Advanced Processing Pattern of these
/// sparse inputs and outputs, the direction of each input its key's in
/// `faces` (a key's last direction, as `dirMap` keeps it).
pub(in crate::game) fn encode(inputs: &[Option<GenericStack>], outputs: &[Option<GenericStack>], faces: &dyn Fn(&GenericStack) -> Option<Direction>) -> Result<ItemStack, String> {
    let Tag::Compound(mut data) = source::encode_processing(inputs, outputs)? else { return Err("invalid processing pattern".into()) };
    let list = inputs
        .iter()
        .map(|input| Tag::String(rows::nullable_direction_name(input.as_ref().and_then(faces).map(direction_value)).into()))
        .collect();
    data.insert("directionMap".into(), Tag::List(list));
    let component = horizon_client::ae2_components::normalize(rows::ENCODED_ADV_PROCESSING_PATTERN, Tag::Compound(data))?;
    let mut pattern = ItemStack::new(rows::ADV_PROCESSING_PATTERN, 1);
    pattern.data_mut().set_other(rows::ENCODED_ADV_PROCESSING_PATTERN, Some(component));
    Ok(pattern)
}

fn same_key(a: &GenericStack, b: &GenericStack) -> bool {
    a.key == b.key && a.key_type == b.key_type
}

/// `AdvProcessingPattern.getDirectionMap`: each distinct input key of
/// `pattern` in the order of its first sparse input, with its direction
/// (the last of that key's inputs).
pub(in crate::game) fn direction_map(pattern: &ItemStack) -> Option<Vec<(GenericStack, Option<Direction>)>> {
    let directions = directions(pattern)?;
    let (inputs, _) = source::processing(pattern)?.ok()?;
    let mut map: Vec<(GenericStack, Option<Direction>)> = Vec::new();
    for (index, input) in inputs.iter().enumerate() {
        let Some(input) = input else { continue };
        let face = directions.get(index).copied().flatten();
        match map.iter_mut().find(|(key, _)| same_key(key, input)) {
            Some(entry) => entry.1 = face,
            None => map.push((GenericStack { amount: 1, ..input.clone() }, face)),
        }
    }
    Some(map)
}

/// `AdvPatternEncoderMenu.decodeInputPattern`: what the output slot shows for
/// the pattern in the input slot. AE2's processing pattern becomes an
/// Advanced one with no direction set (`decodeProcessingPattern`); an
/// Advanced one is copied (`decodeAdvProcessingPattern`). None for any other.
pub(in crate::game) fn encoder_output(input: &ItemStack) -> Option<ItemStack> {
    match input.item.as_ref() {
        "ae2:processing_pattern" => {
            let (inputs, outputs) = source::processing(input)?.ok()?;
            encode(&inputs, &outputs, &|_| None).ok()
        }
        rows::ADV_PROCESSING_PATTERN => {
            direction_map(input)?;
            Some(ItemStack { count: 1, ..input.clone() })
        }
        _ => None,
    }
}

/// `AdvPatternEncoderMenu.update`: `pattern` with input key `key` sent to
/// `face` (none: the provider's face).
pub(in crate::game) fn with_direction(pattern: &ItemStack, key: &GenericStack, face: Option<Direction>) -> Option<ItemStack> {
    let mut map = direction_map(pattern)?;
    let entry = map.iter_mut().find(|(k, _)| same_key(k, key))?;
    entry.1 = face;
    let (inputs, outputs) = source::processing(pattern)?.ok()?;
    let faces = |input: &GenericStack| map.iter().find(|(k, _)| same_key(k, input)).and_then(|(_, face)| *face);
    encode(&inputs, &outputs, &faces).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ae2_cell_core::ItemKey;

    fn item(id: &str, amount: i64) -> Option<GenericStack> {
        Some(GenericStack::item(horizon_client::ae2_cell_codec::key(&ItemStack::new(id, 1)).unwrap(), amount))
    }
    fn fluid(id: &str, amount: i64) -> Option<GenericStack> {
        let key = horizon_client::ae2_cell_codec::key(&ItemStack::new("minecraft:stone", 1)).unwrap();
        Some(GenericStack::fluid(ItemKey { item_id: id.into(), ..key }, amount))
    }
    fn ae2_pattern(inputs: &[Option<GenericStack>], outputs: &[Option<GenericStack>]) -> ItemStack {
        let mut pattern = ItemStack::new("ae2:processing_pattern", 1);
        pattern.data_mut().set_other("ae2:encoded_processing_pattern", Some(source::encode_processing(inputs, outputs).unwrap()));
        pattern
    }

    #[test]
    fn the_encoder_turns_a_processing_pattern_into_an_advanced_one_with_no_directions() {
        horizon_client::ae2x::install();
        let inputs = [item("minecraft:raw_iron", 2), None, item("minecraft:coal", 1), fluid("minecraft:water", 1000)];
        let outputs = [item("minecraft:iron_ingot", 2)];
        let advanced = encoder_output(&ae2_pattern(&inputs, &outputs)).unwrap();
        assert_eq!(advanced.item.as_ref(), rows::ADV_PROCESSING_PATTERN);
        assert_eq!(directions(&advanced).unwrap(), vec![None; 4]);
        // It decodes as a processing pattern: AE2's sparse inputs and outputs.
        let ids = |stacks: &[Option<GenericStack>]| stacks.iter().map(|s| s.as_ref().map(|s| (s.key.item_id.clone(), s.key_type, s.amount))).collect::<Vec<_>>();
        let (decoded_in, decoded_out) = source::processing(&advanced).unwrap().unwrap();
        assert_eq!((ids(&decoded_in), ids(&decoded_out)), (ids(&inputs), ids(&outputs)));
        assert_eq!(encoder_output(&advanced), Some(advanced.clone()), "an advanced pattern is copied");
        assert_eq!(encoder_output(&ItemStack::new("ae2:crafting_pattern", 1)), None);
    }

    #[test]
    fn a_direction_is_set_per_key_and_kept_per_sparse_input() {
        horizon_client::ae2x::install();
        let inputs = [item("minecraft:raw_iron", 1), item("minecraft:coal", 1), item("minecraft:raw_iron", 1)];
        let outputs = [item("minecraft:iron_ingot", 2)];
        let advanced = encoder_output(&ae2_pattern(&inputs, &outputs)).unwrap();
        let ore = item("minecraft:raw_iron", 5).unwrap();
        let up = with_direction(&advanced, &ore, Some(Direction::Up)).unwrap();
        assert_eq!(directions(&up).unwrap(), vec![Some(Direction::Up), None, Some(Direction::Up)]);
        let coal = item("minecraft:coal", 1).unwrap();
        let both = with_direction(&up, &coal, Some(Direction::East)).unwrap();
        assert_eq!(directions(&both).unwrap(), vec![Some(Direction::Up), Some(Direction::East), Some(Direction::Up)]);
        let map = direction_map(&both).unwrap();
        assert_eq!(map.len(), 2, "one entry per key, in first-input order");
        assert_eq!(map[0].0.key.item_id, "minecraft:raw_iron");
        assert_eq!(map[1].1, Some(Direction::East));
        let cleared = with_direction(&both, &ore, None).unwrap();
        assert_eq!(directions(&cleared).unwrap(), vec![None, Some(Direction::East), None]);
        assert_eq!(with_direction(&both, &item("minecraft:stone", 1).unwrap(), Some(Direction::Up)), None, "no such input");
    }

    #[test]
    fn the_saved_direction_list_is_advanced_aes() {
        horizon_client::ae2x::install();
        let advanced = encode(&[item("minecraft:stone", 1), None], &[item("minecraft:dirt", 1)], &|_| Some(Direction::North)).unwrap();
        let component = advanced.data().unwrap().other(rows::ENCODED_ADV_PROCESSING_PATTERN).unwrap();
        let Some(Tag::List(list)) = component.get("directionMap") else { panic!("no direction list") };
        let names: Vec<String> = list.iter().map(|t| match t { Tag::String(s) => s.to_string_lossy(), _ => String::new() }).collect();
        assert_eq!(names, ["NORTH", "null"]);
        // Read from the mod's own save: an entry that is no direction is none.
        let mut odd = advanced.clone();
        let mut tag = component.clone();
        if let Tag::Compound(m) = &mut tag {
            m.insert("directionMap".into(), Tag::List(vec![Tag::String("WEST".into()), Tag::String("sideways".into())]));
        }
        odd.data_mut().set_other(rows::ENCODED_ADV_PROCESSING_PATTERN, Some(tag));
        assert_eq!(directions(&odd).unwrap(), vec![Some(Direction::West), None]);
    }

    #[test]
    fn only_the_advanced_provider_pushes_directionally() {
        horizon_client::ae2x::install();
        let advanced = encode(&[item("minecraft:stone", 1)], &[item("minecraft:dirt", 1)], &|_| Some(Direction::Up)).unwrap();
        let small = Machine::for_block(rows::SMALL_ADV_PATTERN_PROVIDER).unwrap();
        let ae2 = Machine::for_block("ae2:pattern_provider").unwrap();
        assert_eq!(provider_input_directions(&small, &advanced), Some(vec![Some(Direction::Up)]));
        assert_eq!(provider_input_directions(&ae2, &advanced), None, "AE2's provider pushes it as any processing pattern");
        assert_eq!(provider_input_directions(&small, &ae2_pattern(&[item("minecraft:stone", 1)], &[item("minecraft:dirt", 1)])), None);
    }
}
