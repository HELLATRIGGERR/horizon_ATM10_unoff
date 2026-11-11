//! `/ae2 advae ...`: the Advanced Pattern Encoder's edits
//! (`AdvPatternEncoderMenu`) on a pattern in the acting player's inventory,
//! and the pattern's tooltip, for tests and debugging; players use the
//! encoder's screen (`encoder`, `encoder_screen`).
use super::{direction_map, encoder_output, with_direction};
use crate::game::{Game, ae2_lab};
use horizon_client::{ae2_generic_stack::GenericStack, items::ItemStack};
use horizon_core::geometry::Direction;
use serde_json::{Value, json};

const USAGE: &str = "usage: /ae2 advae show|encode <slot> | direction <slot> <item|fluid:<id>> <down|up|north|south|west|east|none> | quantum <x> <y> <z> | crafter <x> <y> <z> [keep <slot> <input> <amount> | limit <slot> <amount>] | bus|throughput <x> <y> <z> <face> | stock <x> <y> <z> <face> <filter> <amount>";

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
                // The tooltip's lines (`MixinEncodedPatternItem`), without their formatting.
                let names = &self.pack.names;
                let tooltip: Vec<String> = self
                    .advancedae_tooltip(&pattern, &|key| names.display_get(key), &|item| names.display_item(&item.item))
                    .unwrap_or_default()
                    .iter()
                    .map(horizon_client::text::Text::plain)
                    .collect();
                Ok(json!({"item": pattern.item.as_ref(), "inputs": inputs, "tooltip": tooltip}))
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
            ["crafter", x, y, z, rest @ ..] => {
                let n = |w: &str| w.parse::<i32>().map_err(|_| String::from("bad coordinate"));
                self.advancedae_crafter_command((n(x)?, n(y)?, n(z)?), rest)
            }
            ["quantum", x, y, z] => {
                let n = |w: &str| w.parse::<i32>().map_err(|_| String::from("bad coordinate"));
                self.advancedae_quantum_status((n(x)?, n(y)?, n(z)?))
            }
            // A stock bus's amounts and REGULATE_STOCK; `stock` sets a filter's amount
            // (what the mod's SetAmountMenu does).
            ["bus" | "stock" | "throughput", x, y, z, face, rest @ ..] => {
                let n = |w: &str| w.parse::<i32>().map_err(|_| String::from("bad coordinate"));
                let pos = (n(x)?, n(y)?, n(z)?);
                if words[0] == "throughput" {
                    return self.advancedae_throughput_status(pos, face);
                }
                self.advancedae_bus_command(pos, face, words[0] == "stock", rest)
            }
            _ => Err(USAGE.into()),
        }
    }
}
