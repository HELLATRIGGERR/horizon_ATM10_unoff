// WoodIndex: maps each input item id to its list of (output_item_id, count) pairs.
// Also provides helpers for encoding/decoding an item id as a sequence of i32 values
// (for the entity.data wire format that syncs server state to the client's screen_draw).

use std::collections::HashMap;
use crate::generator::Recipe;

/// One entry in the recipe list for a given input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub output: String,
    pub count: u32,
}

/// The full recipe index, keyed by input item id.
pub struct WoodIndex {
    pub by_input: HashMap<String, Vec<Entry>>,
}

impl WoodIndex {
    pub fn build(recipes: Vec<Recipe>) -> Self {
        let mut by_input: HashMap<String, Vec<Entry>> = HashMap::new();
        for r in recipes {
            let entry = Entry { output: r.output, count: r.count };
            for input in r.inputs {
                by_input.entry(input).or_default().push(entry.clone());
            }
        }
        // Sort entries deterministically so server and client agree on recipe order.
        for entries in by_input.values_mut() {
            entries.sort_unstable_by(|a, b| a.output.cmp(&b.output));
            entries.dedup_by(|a, b| a.output == b.output);
        }
        Self { by_input }
    }

    pub fn recipes_for(&self, input: &str) -> &[Entry] {
        self.by_input.get(input).map(Vec::as_slice).unwrap_or(&[])
    }
}

// ---- entity.data encoding -----------------------------------------------------------------------
//
// The server stores the current input item id in entity.data so the client's screen_draw can look
// up the recipe list in its own (identical) WoodIndex without needing slot access.
//
// Layout:
//   data[0]   = number of recipes for the current input (0 = empty / no match)
//   data[1]   = selected recipe index (-1 = none)
//   data[2]   = scroll offset (0-based)
//   data[3]   = byte length L of the input item id string
//   data[4..] = L bytes of the item id, 2 bytes per value, little-endian
//
// A menu's data slots reach the client as 16-bit values, so each value carries two bytes.

/// Encode an item id string into the tail of a data vec, starting at index 3.
pub fn encode_input(id: &str, data: &mut Vec<i32>) {
    data.resize(4, 0);
    let bytes = id.as_bytes();
    data[3] = bytes.len() as i32;
    for chunk in bytes.chunks(2) {
        let low = chunk[0] as u16;
        let high = chunk.get(1).copied().unwrap_or(0) as u16;
        data.push(i32::from((low | (high << 8)) as i16));
    }
}

/// Decode the item id from `data` (see layout above). Returns `None` on malformed data.
pub fn decode_input(data: &[i32]) -> Option<String> {
    let len = *data.get(3)? as usize;
    if len == 0 {
        return Some(String::new());
    }
    let encoded = data.get(4..)?;
    let mut bytes = Vec::with_capacity(len);
    'outer: for &val in encoded {
        let bits = val as u16;
        for i in 0..2u32 {
            if bytes.len() == len {
                break 'outer;
            }
            bytes.push(((bits >> (i * 8)) & 0xFF) as u8);
        }
    }
    if bytes.len() != len {
        return None;
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::Recipe;

    #[test]
    fn roundtrip_encode_decode() {
        let id = "minecraft:oak_log";
        let mut data = vec![0i32; 4];
        encode_input(id, &mut data);
        assert_eq!(decode_input(&data).as_deref(), Some(id));
    }

    #[test]
    fn roundtrip_long_id() {
        let id = "sophisticated_storage:oak_storage_connector";
        let mut data = vec![0i32; 4];
        encode_input(id, &mut data);
        assert_eq!(decode_input(&data).as_deref(), Some(id));
    }

    #[test]
    fn survives_16_bit_data_slots() {
        // The client receives each value as an i16 widened back to i32.
        let id = "somemod:\u{e9}rable_log";
        let mut data = vec![0i32; 4];
        encode_input(id, &mut data);
        let synced: Vec<i32> = data.iter().map(|&v| i32::from(v as i16)).collect();
        assert_eq!(decode_input(&synced).as_deref(), Some(id));
        assert!(data[4..].iter().all(|&v| (i16::MIN as i32..=i16::MAX as i32).contains(&v)));
    }

    #[test]
    fn build_index_deduplicates_and_sorts() {
        let recipes = vec![
            Recipe { inputs: vec!["a".into()], output: "z".into(), count: 1 },
            Recipe { inputs: vec!["a".into()], output: "m".into(), count: 2 },
            Recipe { inputs: vec!["a".into()], output: "z".into(), count: 1 }, // dup
        ];
        let idx = WoodIndex::build(recipes);
        let entries = idx.recipes_for("a");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].output, "m");
        assert_eq!(entries[1].output, "z");
    }

    #[test]
    fn recipes_for_missing_input_is_empty() {
        let idx = WoodIndex::build(vec![]);
        assert!(idx.recipes_for("minecraft:oak_log").is_empty());
    }
}
