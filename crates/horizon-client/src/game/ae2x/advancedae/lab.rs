//! `/ae2 advae ...`: the Advanced Pattern Encoder's edits
//! (`AdvPatternEncoderMenu`) on a pattern in the acting player's inventory,
//! for tests and debugging while the encoder's screen is a later slice's.
use super::{direction_map, encoder_output, with_direction};
use crate::game::{Game, ae2_lab};
use horizon_client::{ae2_generic_stack::GenericStack, items::ItemStack};
use horizon_core::geometry::Direction;
use serde_json::{Value, json};

const USAGE: &str = "usage: /ae2 advae show|encode <slot> | direction <slot> <item|fluid:<id>> <down|up|north|south|west|east|none>";

fn parse_face(word: &str) -> Result<Option<Direction>, String> {
    Ok(Some(match word {
        "down" => Direction::Down,
        "up" => Direction::Up,
        "north" => Direction::North,
        "south" => Direction::South,
        "west" => Direction::West,
        "east" => Direction::East,
        "none" => return Ok(None),
        _ => return Err(format!("not a direction: {word}")),
    }))
}
fn face_name(face: Option<Direction>) -> &'static str {
    match face {
        Some(Direction::Down) => "down",
        Some(Direction::Up) => "up",
        Some(Direction::North) => "north",
        Some(Direction::South) => "south",
        Some(Direction::West) => "west",
        Some(Direction::East) => "east",
        None => "none",
    }
}

impl Game<'_> {
    pub(in crate::game) fn ae2_advancedae_command(&mut self, words: &[&str]) -> Result<Value, String> {
        let slot = |word: &str| -> Result<usize, String> { word.parse().map_err(|_| "bad slot".into()) };
        let stack = |game: &Self, slot: usize| -> Result<ItemStack, String> {
            game.inventory.get(slot).and_then(Option::clone).ok_or_else(|| "no item in that slot".into())
        };
        match words {
            ["show", at] => {
                let pattern = stack(self, slot(at)?)?;
                let map = direction_map(&pattern).ok_or("not an Advanced Processing Pattern")?;
                let inputs: Vec<Value> = map
                    .iter()
                    .map(|(key, face)| json!({"id": key.key.item_id, "fluid": key.key_type == ae2_cell_core::KeyType::FLUIDS, "direction": face_name(*face)}))
                    .collect();
                Ok(json!({"item": pattern.item.as_ref(), "inputs": inputs}))
            }
            ["encode", at] => {
                let at = slot(at)?;
                let output = encoder_output(&stack(self, at)?).ok_or("not a processing pattern")?;
                self.inventory[at] = Some(output);
                self.ae2_advancedae_command(&["show", &at.to_string()])
            }
            ["direction", at, id, face] => {
                let at = slot(at)?;
                let key = match id.strip_prefix("fluid:") {
                    // A fluid key's components are an item key's of none (`ae2_cell_codec::key`).
                    Some(fluid) => GenericStack::fluid(ae2_cell_core::ItemKey { item_id: fluid.into(), ..ae2_lab::key(&ItemStack::new("minecraft:stone", 1))? }, 1),
                    None => GenericStack::item(ae2_lab::key(&ItemStack::new(*id, 1))?, 1),
                };
                let output = with_direction(&stack(self, at)?, &key, parse_face(face)?).ok_or("no such input of an Advanced Processing Pattern")?;
                self.inventory[at] = Some(output);
                self.ae2_advancedae_command(&["show", &at.to_string()])
            }
            _ => Err(USAGE.into()),
        }
    }
}
