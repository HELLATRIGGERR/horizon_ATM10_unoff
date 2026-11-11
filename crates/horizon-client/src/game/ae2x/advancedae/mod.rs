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
//!
//! The large provider and the part forms are the same logic of 36 and 9
//! patterns; the upgrade items replace AE2's and ExtendedAE's providers in
//! place through the addons' kits (`kits`).
//!
//! Also here: the provider's `FILTERED_IMPORT` (`AdvPatternProviderReturnInventory`),
//! the pattern's tooltip (`MixinEncodedPatternItem`) and its re-encoding in
//! AE2's encoding terminal (`MixinPatternEncodingTermMenu`). The encoder's
//! menu is `encoder`, its screen `encoder_screen`; the Reaction Chamber is
//! `chamber`, the Quantum Computer `quantum`, the buses `buses` and the
//! Throughput Monitor `throughput`.
pub(in crate::game) mod buses;
pub(in crate::game) mod chamber;
pub(in crate::game) mod encoder;
pub(in crate::game) mod encoder_screen;
mod lab;
pub(in crate::game) mod quantum;
pub(in crate::game) mod throughput;

use crate::game::ae2_source_patterns as source;
use ae2_cell_core::{ItemKey, KeyType};
use horizon_client::{ae2_generic_stack::GenericStack, ae2_world::Machine, ae2x::advancedae as rows, items::ItemStack};
use horizon_core::geometry::Direction;
use horizon_world::nbt::Tag;

/// What the game keeps for Advanced AE while it runs; nothing of it is saved.
#[derive(Default)]
pub(in crate::game) struct State {
    /// The encoder menus open on this server, by connection.
    pub(in crate::game) encoders: std::collections::HashMap<u32, encoder::Session>,
    /// The window this client draws as the encoder, and its list's scroll.
    pub(in crate::game) encoder_window: Option<i32>,
    pub(in crate::game) encoder_scroll: crate::game::ae2_widgets::Scroll,
    /// The Reaction Chambers' searches, by block entity incarnation.
    pub(in crate::game) chambers: std::collections::HashMap<u64, chamber::Ctx>,
    /// The Throughput Monitors' readings, by part node.
    pub(in crate::game) throughput: std::collections::BTreeMap<ae2_network_core::NodeIdentity, throughput::Monitor>,
}

/// `AAESettings.FILTERED_IMPORT` in the provider's config, as
/// `IConfigManager.writeToNBT` names it: `NO` (the default) or `YES`.
pub(in crate::game) const FILTERED_IMPORT: &str = "filtered_import";
/// Its toolbar button's menu entry, after AE2's three settings: 21 for nine
/// patterns, 48 for 36.
pub(in crate::game) fn filtered_import_entry(provider: &Machine) -> usize {
    crate::game::ae2_machine_ui::MenuShape::of(provider).settings() as usize + 3
}

/// The base model of a provider part (`AdvPatternProviderPart.MODELS_*`'s
/// `MODEL_BASE`). `SmallAdvPatternProviderPart` keeps its parent's models, so
/// both parts draw the large one's.
pub(in crate::game) fn part_model(id: &str) -> Option<&'static str> {
    Some(match id {
        rows::ADV_PATTERN_PROVIDER_PART | rows::SMALL_ADV_PATTERN_PROVIDER_PART => "advanced_ae:part/adv_pattern_provider_part",
        // The buses' MODEL_BASE.
        rows::STOCK_EXPORT_BUS => "advanced_ae:part/stock_export_bus_part",
        rows::IMPORT_EXPORT_BUS => "advanced_ae:part/import_export_bus_part",
        rows::ADVANCED_IO_BUS => "advanced_ae:part/advanced_io_bus_part",
        _ => return None,
    })
}
/// The buses' screens (`AAEMenus`): `screens/stock_export_bus.json`,
/// `import_export_bus.json`, `advanced_io_bus.json`, each AE2's `io_bus.json`
/// with its own title.
pub(in crate::game) fn part_style(id: &str) -> Option<(&'static str, &'static str)> {
    Some(match id {
        rows::STOCK_EXPORT_BUS => ("advanced_ae", "stock_export_bus.json"),
        rows::IMPORT_EXPORT_BUS => ("advanced_ae", "import_export_bus.json"),
        rows::ADVANCED_IO_BUS => ("advanced_ae", "advanced_io_bus.json"),
        _ => return None,
    })
}

/// The upgrade items (`AdvPatternProviderUpgradeItem`,
/// `AdvPatternProviderCapacityUpgradeItem`): by the exact class, AE2's
/// provider becomes the small one and ExtendedAE's the large one, the small
/// one the large one; block and part alike.
pub(in crate::game) const KITS: &[crate::game::ae2x::kits::Kit] = &[
    crate::game::ae2x::kits::Kit {
        item: rows::ADV_PATTERN_PROVIDER_UPGRADE,
        blocks: &[("ae2:pattern_provider", rows::SMALL_ADV_PATTERN_PROVIDER), ("extendedae:ex_pattern_provider", rows::ADV_PATTERN_PROVIDER)],
        parts: &[("ae2:cable_pattern_provider", rows::SMALL_ADV_PATTERN_PROVIDER_PART), ("extendedae:ex_pattern_provider_part", rows::ADV_PATTERN_PROVIDER_PART)],
    },
    crate::game::ae2x::kits::Kit {
        item: rows::ADV_PATTERN_PROVIDER_CAPACITY_UPGRADE,
        blocks: &[(rows::SMALL_ADV_PATTERN_PROVIDER, rows::ADV_PATTERN_PROVIDER)],
        parts: &[(rows::SMALL_ADV_PATTERN_PROVIDER_PART, rows::ADV_PATTERN_PROVIDER_PART)],
    },
];

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

/// `MixinPatternEncodingTermMenu.onEncodeProcessingPattern`: the processing
/// pattern AE2's encoding terminal made, `encoded`, while its encoded-pattern
/// slot held `previous`. When that is an Advanced Processing Pattern, each
/// input key of the new pattern that it had keeps its direction, and if any
/// does, the result is an Advanced pattern of the new inputs and outputs.
/// None leaves AE2's pattern.
pub(in crate::game) fn reencoded(previous: &ItemStack, encoded: &ItemStack) -> Option<ItemStack> {
    let old = direction_map(previous)?;
    let (inputs, outputs) = source::processing(encoded)?.ok()?;
    let kept: Vec<&(GenericStack, Option<Direction>)> = inputs.iter().flatten().filter_map(|input| old.iter().find(|(key, _)| same_key(key, input))).collect();
    if kept.is_empty() {
        return None;
    }
    let faces = |input: &GenericStack| kept.iter().find(|(key, _)| same_key(key, input)).and_then(|(_, face)| *face);
    encode(&inputs, &outputs, &faces).ok()
}

/// `condenseStacks`: each key once, in the order of its first stack, the
/// amounts added up.
fn condensed(stacks: &[Option<GenericStack>]) -> Vec<GenericStack> {
    let mut out: Vec<GenericStack> = Vec::new();
    for stack in stacks.iter().flatten() {
        match out.iter_mut().find(|s| same_key(s, stack)) {
            Some(found) => found.amount = found.amount.saturating_add(stack.amount),
            None => out.push(stack.clone()),
        }
    }
    out
}

/// `MixinEncodedPatternItem.onHoverText`, which stands in for AE2's
/// `appendHoverText` on an Advanced Processing Pattern: the
/// `PatternDetailsTooltip` of `getTooltip` (the condensed outputs after
/// "Produces:", the condensed inputs after "with:", the rest after "and"),
/// each input followed by its direction's first letter. Each line in legacy
/// formatting (gray labels, `getTooltipEntryLine`'s gray " x "). None for
/// any other item, and for a pattern that does not decode.
pub(in crate::game) fn tooltip(pattern: &ItemStack, translate: &dyn Fn(&str) -> Option<String>, name: &dyn Fn(&GenericStack) -> String) -> Option<Vec<String>> {
    let map = direction_map(pattern)?;
    let (inputs, outputs) = source::processing(pattern)?.ok()?;
    if inputs.iter().all(Option::is_none) || outputs.first().is_none_or(Option::is_none) {
        return None;
    }
    let text = |key: &str, fallback: &str| translate(key).unwrap_or_else(|| fallback.to_owned());
    let (gray, reset) = ("\u{a7}7", "\u{a7}r");
    let entry = |stack: &GenericStack| match u64::try_from(stack.amount).ok().filter(|n| *n > 0) {
        Some(amount) => format!("{}{gray} x {reset}{}", stack.key_type.format_amount(amount, ae2_cell_core::AmountFormat::Full), name(stack)),
        None => name(stack),
    };
    let and = format!("{gray} {} {reset}", text("gui.ae2.And", "and"));
    let produces = format!("{gray}{}: {reset}", text("gui.ae2.Produces", "Produces"));
    let with = format!("{gray}{}: {reset}", text("gui.ae2.With", "with"));
    let mut lines = Vec::new();
    for (i, output) in condensed(&outputs).iter().enumerate() {
        lines.push(format!("{}{}", if i == 0 { &produces } else { &and }, entry(output)));
    }
    for (i, input) in condensed(&inputs).iter().enumerate() {
        // `getDirectionSideForInputKey`: `Direction.toString()`'s first letter, upper case.
        let face = map.iter().find(|(key, _)| same_key(key, input)).and_then(|(_, face)| *face);
        let letter = face.map_or(String::new(), |face| format!(" ({})", direction_letter(face)));
        lines.push(format!("{}{}{letter}", if i == 0 { &with } else { &and }, entry(input)));
    }
    Some(lines)
}

/// `Direction.toString().toUpperCase().charAt(0)`.
fn direction_letter(face: Direction) -> char {
    match face {
        Direction::Down => 'D',
        Direction::Up => 'U',
        Direction::North => 'N',
        Direction::South => 'S',
        Direction::West => 'W',
        Direction::East => 'E',
    }
}

/// `details.getOutputs()` of a provider's pattern, as keys: a processing
/// pattern's sparse outputs, a crafting or smithing or stonecutting pattern's
/// result.
fn pattern_outputs(pattern: &ItemStack) -> Vec<(ItemKey, KeyType)> {
    if let Some(Ok((_, outputs))) = source::processing(pattern) {
        return outputs.into_iter().flatten().map(|output| (output.key, output.key_type)).collect();
    }
    let output = crate::game::ae2_processing::ae2_decode_special_pattern(pattern)
        .map(|special| special.output)
        .or_else(|| crate::game::ae2_processing::decode_pattern(pattern).map(|(_, output)| output));
    output.and_then(|output| horizon_client::ae2_cell_codec::key(&output).ok()).map(|key| vec![(key, KeyType::ITEMS)]).unwrap_or_default()
}

/// `AdvPatternProviderReturnInventory`'s filter: with `FILTERED_IMPORT` on,
/// an Advanced AE provider's return slots take only what its patterns make
/// (`getOutputCache`; the keys of `getTrackedCrafts` are its patterns'
/// outputs too). True for any other provider, and with the setting off.
pub(in crate::game) fn return_accepts(provider: &Machine, key: &ItemKey, key_type: KeyType) -> bool {
    if !rows::directional_provider(provider.id()) || provider.config.get(&FILTERED_IMPORT.into()) != Some(&Tag::String("YES".into())) {
        return true;
    }
    provider.slots.iter().flatten().any(|pattern| pattern_outputs(pattern).iter().any(|(k, t)| k == key && *t == key_type))
}

/// A left or right press on `provider`'s Filtered Import button: the setting
/// steps on (back with the right).
fn filtered_import_press(provider: &mut Machine, slot: usize, button: i32, input: horizon_client::menu::Input) -> bool {
    slot == 300 + filtered_import_entry(provider)
        && input == horizon_client::menu::Input::Pickup
        && (0..=1).contains(&button)
        && rows::directional_provider(provider.id())
        && crate::game::ae2_screens::step_setting(provider, slot, button == 1)
}

impl crate::game::Game<'_> {
    /// A press on the Filtered Import button of a provider's screen (menu
    /// entry [`filtered_import_entry`], window slot `slot`): the setting steps
    /// on; or on a Reaction Chamber's own entries. False for any other press.
    pub(in crate::game) fn advancedae_machine_click(&mut self, pos: (i32, i32, i32), slot: usize, button: i32, input: horizon_client::menu::Input) -> bool {
        if self.advancedae_chamber_click(pos, slot, button, input) {
            return true;
        }
        let Some(horizon_client::containers::BlockEntity::Ae2(machine)) = self.world.block_entities.get_mut(&pos) else { return false };
        if !filtered_import_press(machine, slot, button, input) {
            return false;
        }
        self.world.modified.insert((pos.0 >> 4, pos.2 >> 4));
        true
    }

    /// [`Self::advancedae_machine_click`] on a provider part's screen.
    pub(in crate::game) fn advancedae_part_machine_click(&mut self, part: crate::game::ae2_parts::PartAddress, slot: usize, button: i32, input: horizon_client::menu::Input) -> bool {
        let Ok(Some(mut machine)) = self.ae2_part_machine(part) else { return false };
        filtered_import_press(&mut machine, slot, button, input) && self.ae2_store_part_machine(part, &machine).is_ok()
    }

    /// An upgrade item used on block `pos`, side `face` (sneaking; `useOn`).
    pub(in crate::game) fn advancedae_kit_use(&mut self, pos: [i32; 3], face: [i32; 3]) -> bool {
        self.ae2x_kit_use(KITS, pos, face)
    }

    /// The tooltip of an Advanced Processing Pattern, `item_name` naming an
    /// item (fluids by their names).
    pub(in crate::game) fn advancedae_tooltip(&self, stack: &ItemStack, translate: &dyn Fn(&str) -> Option<String>, item_name: &dyn Fn(&ItemStack) -> String) -> Option<Vec<horizon_client::text::Text>> {
        let name = |stack: &GenericStack| {
            if stack.key_type == KeyType::ITEMS {
                horizon_client::ae2_cell_codec::prototype(&stack.key).map_or_else(|_| stack.key.item_id.clone(), |item| item_name(&item))
            } else {
                self.pack.names.display_item(&stack.key.item_id)
            }
        };
        Some(tooltip(stack, translate, &name)?.iter().map(|line| horizon_client::text::Text::legacy(line)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, amount: i64) -> Option<GenericStack> {
        Some(GenericStack::item(horizon_client::ae2_cell_codec::key(&ItemStack::new(id, 1)).unwrap(), amount))
    }
    fn fluid(id: &str, amount: i64) -> Option<GenericStack> {
        Some(GenericStack::fluid(horizon_client::ae2_cell_codec::fluid_key(id), amount))
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

    /// `MixinEncodedPatternItem`: AE2's tooltip lines of the condensed outputs
    /// and inputs, each input with its direction's letter.
    #[test]
    fn the_tooltip_names_each_inputs_direction() {
        horizon_client::ae2x::install();
        let inputs = [item("minecraft:raw_iron", 2), item("minecraft:coal", 1), item("minecraft:raw_iron", 1), fluid("minecraft:water", 250)];
        let faces = |input: &GenericStack| match input.key.item_id.as_str() {
            "minecraft:raw_iron" => Some(Direction::Up),
            "minecraft:water" => Some(Direction::West),
            _ => None,
        };
        let pattern = encode(&inputs, &[item("minecraft:iron_ingot", 3), item("minecraft:stone", 1)], &faces).unwrap();
        let name = |stack: &GenericStack| stack.key.item_id.trim_start_matches("minecraft:").to_owned();
        let lines = tooltip(&pattern, &|_| None, &name).unwrap();
        let plain: Vec<String> = lines.iter().map(|line| horizon_client::text::Text::legacy(line).plain()).collect();
        let water = format!(" and {} x water (W)", KeyType::FLUIDS.format_amount(250, ae2_cell_core::AmountFormat::Full));
        assert_eq!(plain, ["Produces: 3 x iron_ingot", " and 1 x stone", "with: 3 x raw_iron (U)", " and 1 x coal", water.as_str()]);
        assert_eq!(lines[0], "\u{a7}7Produces: \u{a7}r3\u{a7}7 x \u{a7}riron_ingot");
        let translated = tooltip(&pattern, &|key| (key == "gui.ae2.With").then(|| "mit".to_owned()), &name).unwrap();
        assert!(translated[2].contains("mit: "));
        assert_eq!(tooltip(&ae2_pattern(&inputs, &[item("minecraft:iron_ingot", 1)]), &|_| None, &name), None, "AE2's pattern keeps AE2's tooltip");
    }

    /// `MixinPatternEncodingTermMenu`: re-encoding over an Advanced pattern
    /// keeps the directions of the inputs still there; with none of them
    /// left, AE2's pattern stays.
    #[test]
    fn reencoding_keeps_the_directions_of_the_inputs_left() {
        horizon_client::ae2x::install();
        let old = encode(&[item("minecraft:raw_iron", 1), item("minecraft:coal", 1)], &[item("minecraft:iron_ingot", 1)], &|input| {
            (input.key.item_id == "minecraft:raw_iron").then_some(Direction::Up)
        })
        .unwrap();
        let new = ae2_pattern(&[item("minecraft:coal", 2), None, item("minecraft:raw_iron", 4), item("minecraft:stone", 1)], &[item("minecraft:iron_ingot", 2)]);
        let advanced = reencoded(&old, &new).unwrap();
        assert_eq!(advanced.item.as_ref(), rows::ADV_PROCESSING_PATTERN);
        assert_eq!(directions(&advanced).unwrap(), vec![None, None, Some(Direction::Up), None]);
        let (inputs, outputs) = source::processing(&advanced).unwrap().unwrap();
        assert_eq!((inputs[0].as_ref().unwrap().amount, outputs[0].as_ref().unwrap().amount), (2, 2));
        let unrelated = ae2_pattern(&[item("minecraft:stone", 1)], &[item("minecraft:dirt", 1)]);
        assert_eq!(reencoded(&old, &unrelated), None, "no input left: AE2's pattern");
        assert_eq!(reencoded(&new, &unrelated), None, "over AE2's pattern: AE2's");
    }

    /// `AdvPatternProviderReturnInventory`: with Filtered Import on, the
    /// Advanced provider's return slots take only its patterns' outputs.
    #[test]
    fn filtered_import_takes_only_the_patterns_outputs() {
        horizon_client::ae2x::install();
        let key = |id: &str| horizon_client::ae2_cell_codec::key(&ItemStack::new(id, 1)).unwrap();
        let mut small = Machine::for_block(rows::SMALL_ADV_PATTERN_PROVIDER).unwrap();
        small.slots[0] = Some(ae2_pattern(&[item("minecraft:raw_iron", 1)], &[item("minecraft:iron_ingot", 1), fluid("minecraft:lava", 100)]));
        let lava = fluid("minecraft:lava", 1).unwrap();
        assert!(return_accepts(&small, &key("minecraft:cobblestone"), KeyType::ITEMS), "off by default");
        small.config.insert(FILTERED_IMPORT.into(), Tag::String("YES".into()));
        assert!(return_accepts(&small, &key("minecraft:iron_ingot"), KeyType::ITEMS));
        assert!(return_accepts(&small, &lava.key, KeyType::FLUIDS));
        assert!(!return_accepts(&small, &lava.key, KeyType::ITEMS), "the key type counts");
        assert!(!return_accepts(&small, &key("minecraft:cobblestone"), KeyType::ITEMS));
        assert!(!return_accepts(&small, &key("minecraft:raw_iron"), KeyType::ITEMS), "an input is no output");
        let mut ae2 = Machine::for_block("ae2:pattern_provider").unwrap();
        ae2.config.insert(FILTERED_IMPORT.into(), Tag::String("YES".into()));
        assert!(return_accepts(&ae2, &key("minecraft:cobblestone"), KeyType::ITEMS), "AE2's provider has no filter");
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
        let large = crate::game::ae2_parts::part_machine(rows::ADV_PATTERN_PROVIDER_PART).unwrap();
        assert_eq!((large.slots.len(), provider_input_directions(&large, &advanced)), (36, Some(vec![Some(Direction::Up)])), "the part pushes as the block");
    }

    #[test]
    fn filtered_import_sits_after_the_settings_of_each_size() {
        horizon_client::ae2x::install();
        let mut large = Machine::for_block(rows::ADV_PATTERN_PROVIDER).unwrap();
        let small = Machine::for_block(rows::SMALL_ADV_PATTERN_PROVIDER).unwrap();
        assert_eq!((filtered_import_entry(&small), filtered_import_entry(&large)), (21, 48));
        assert!(!filtered_import_press(&mut large, 321, 0, horizon_client::menu::Input::Pickup), "the small one's entry is a return slot here");
        assert!(filtered_import_press(&mut large, 348, 0, horizon_client::menu::Input::Pickup));
        assert_eq!(large.config.get(&FILTERED_IMPORT.into()), Some(&Tag::String("YES".into())));
        let mut ae2 = Machine::for_block("ae2:pattern_provider").unwrap();
        assert!(!filtered_import_press(&mut ae2, 321, 0, horizon_client::menu::Input::Pickup), "AE2's provider has no such button");
    }

    #[test]
    fn the_upgrade_items_turn_each_provider_into_the_next_with_its_patterns() {
        use crate::game::ae2x::kits::{upgraded, upgraded_part};
        horizon_client::ae2x::install();
        let kit = |id: &str| KITS.iter().find(|kit| kit.item == id).unwrap();
        let upgrade = kit(rows::ADV_PATTERN_PROVIDER_UPGRADE);
        let capacity = kit(rows::ADV_PATTERN_PROVIDER_CAPACITY_UPGRADE);
        for (from, to) in upgrade.blocks.iter().chain(capacity.blocks) {
            assert_eq!(horizon_client::ae2_world::variant(to).map(|row| row.part), Some(false));
            assert!(Machine::for_block(from).is_some(), "{from}");
        }
        for (_, to) in upgrade.parts.iter().chain(capacity.parts) {
            assert_eq!(horizon_client::ae2_world::variant(to).map(|row| row.part), Some(true));
        }
        // AE2's provider: the small one, the pattern in its slot, the settings kept.
        let mut provider = Machine::for_block("ae2:pattern_provider").unwrap();
        provider.slots[8] = Some(encode(&[item("minecraft:stone", 1)], &[item("minecraft:dirt", 1)], &|_| Some(Direction::Up)).unwrap());
        provider.config.insert("priority".into(), Tag::Int(7));
        provider.config.insert("ae2_blocking".into(), Tag::Byte(1));
        let small = upgraded(&provider, upgrade.blocks[0].1).unwrap();
        assert_eq!((small.id(), small.slots.len()), (rows::SMALL_ADV_PATTERN_PROVIDER, 9));
        assert_eq!((small.slots[8].clone(), small.config.clone()), (provider.slots[8].clone(), provider.config.clone()));
        // The capacity upgrade: 36 slots, the first nine the small one's.
        let large = upgraded(&small, capacity.blocks[0].1).unwrap();
        assert_eq!((large.id(), large.slots.len(), large.slots[8].clone()), (rows::ADV_PATTERN_PROVIDER, 36, provider.slots[8].clone()));
        assert!(upgraded(&large, rows::SMALL_ADV_PATTERN_PROVIDER).is_none(), "never fewer slots");
        // A part keeps its record under the new id.
        let part = Tag::compound([
            ("id", Tag::String("ae2:cable_pattern_provider".into())),
            ("face", Tag::String("north".into())),
            ("stack", horizon_client::save::item_to_nbt(&ItemStack::new("ae2:cable_pattern_provider", 1))),
            ("machine", Tag::compound([("config", Tag::compound([("priority", Tag::Int(5))]))])),
        ]);
        let next = upgraded_part(&part, upgrade.parts[0].1).unwrap();
        assert_eq!(next.get("id"), Some(&Tag::String(rows::SMALL_ADV_PATTERN_PROVIDER_PART.into())));
        assert_eq!((next.get("face"), next.get("machine")), (part.get("face"), part.get("machine")));
        assert_eq!(part_model(rows::SMALL_ADV_PATTERN_PROVIDER_PART), part_model(rows::ADV_PATTERN_PROVIDER_PART));
    }
}
