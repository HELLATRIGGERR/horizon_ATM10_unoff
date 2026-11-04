//! MEGA's bulk compression in the game: `CompressionService` (the chains a
//! server gathers from its crafting recipes and the
//! `megacells:compression_overrides` data map, at its start and on each
//! `/reload`), `SyncCompressionChainsPacket` (the chains each client gets
//! on joining and after a reload) and the Cell Workbench's compression
//! cutoff (`CompressionCutoffHost.mega$nextCompressionLimit`, the
//! `CompressionCutoffButton` of `CellWorkbenchScreenMixin` and
//! `PortableCellWorkbenchScreen`). The chain building is
//! `ae2_megacells_core::load_recipes`. Derived from MEGA Cells 4.11.0,
//! https://github.com/62832/MEGACells tag 4.11.0, LGPL-3.0; see
//! `ae2/addons/megacells/`.
use super::bulk::BulkStack;
use crate::game::{Game, lan::LanHost};
use ae2_cell_core::ItemKey;
use ae2_megacells_core::{Chains, CompressionChain, CraftingRecipe, OverrideValue, Variant};
use horizon_client::{ae2_world::Machine, ae2x::megacells as rows, crafting::Recipe, items::ItemStack};
use horizon_protocol::packets::{configuration::CustomPayload, play::Clientbound as C};
use serde_json::{Value, json};
use std::{cell::Cell, collections::HashMap, sync::Arc};

/// `SyncCompressionChainsPacket.TYPE`.
pub(in crate::game) const SYNC: &str = "megacells:get_compression_chain";
/// The window place a press of the workbench's `CompressionCutoffButton`
/// goes to (`CompressionCutoffHost.ACTION_SET_COMPRESSION_LIMIT`), beside
/// the filters' places; button 1 is backwards (`isHandlingRightClick`).
pub(in crate::game) const CUTOFF_PLACE: i16 = 99;
/// The data map's file under the resources' or a data pack's root.
const OVERRIDES: &str = "data/megacells/data_maps/item/compression_overrides.json";

/// What a server keeps of its chains: the recipe table they were gathered
/// from, their generation, and the generation each client was sent.
#[derive(Default)]
pub(in crate::game) struct Compression {
    built_for: Cell<usize>,
    built: Cell<u64>,
    sent: HashMap<u32, u64>,
}

/// `CompressionOverride.CODEC`'s `variant`: `"NONE"`, or an item id (air
/// is `NONE` too); an id of no item is an error that drops the entry.
fn override_value(value: &Value, known: &dyn Fn(&str) -> bool, loaded: &dyn Fn(&str) -> bool) -> Option<OverrideValue> {
    // NeoForge's conditions on an entry: `neoforge:mod_loaded` alone here.
    if let Some(conditions) = value.get("neoforge:conditions").and_then(Value::as_array) {
        let met = conditions.iter().all(|condition| match condition["type"].as_str() {
            Some("neoforge:mod_loaded") => condition["modid"].as_str().is_some_and(loaded),
            _ => false,
        });
        if !met {
            return None;
        }
    }
    let variant = value.get("variant")?.as_str()?;
    if variant == "NONE" {
        return Some(OverrideValue::None);
    }
    let id = if variant.contains(':') { variant.to_owned() } else { format!("minecraft:{variant}") };
    if id == "minecraft:air" {
        return Some(OverrideValue::None);
    }
    known(&id).then_some(OverrideValue::Variant(id))
}

/// The chains as `SyncCompressionChainsPacket` writes them: each variant's
/// stack and count.
fn encode_chains(chains: &Chains) -> Vec<u8> {
    let list: Vec<Value> = chains
        .chains
        .iter()
        .map(|chain| {
            Value::Array(
                chain
                    .variants()
                    .iter()
                    .filter_map(|variant| {
                        let stack = horizon_client::ae2_cell_codec::prototype(&variant.key).ok()?;
                        Some(json!({"item": horizon_client::item_data::canon_stack(Some(&stack)), "count": variant.count}))
                    })
                    .collect(),
            )
        })
        .collect();
    serde_json::to_vec(&list).unwrap_or_default()
}

fn decode_chains(data: &[u8]) -> Option<Chains> {
    let list: Vec<Vec<Value>> = serde_json::from_slice(data).ok()?;
    let mut chains = Vec::new();
    for chain in list {
        let mut variants = Vec::new();
        for variant in chain {
            let stack = horizon_client::item_data::parse_stack(variant["item"].as_str()?).ok()??;
            let key = horizon_client::ae2_cell_codec::key(&stack).ok()?;
            variants.push(Variant { key, count: u32::try_from(variant["count"].as_u64()?).ok()? });
        }
        chains.push(CompressionChain::new(variants));
    }
    Some(Chains { chains, overrides: 0 })
}

impl Game<'_> {
    /// `CompressionService`'s chains. A server gathers them from its recipe
    /// table the first time it needs them and again when the table changes
    /// (`ServerStartedEvent`, a `/reload`'s `OnDatapackSyncEvent`); a client
    /// has those its server sent.
    pub(in crate::game) fn megacells_chains(&self) -> Arc<Chains> {
        if self.remote.is_none() {
            let table = self.recipes() as *const horizon_client::crafting::Recipes as usize;
            let state = &self.ae2x.megacells.compression;
            if state.built_for.get() != table {
                let chains = self.megacells_load_chains();
                log::info!("Initialised bulk compression. Gathered {} compression chains, with {} overrides.", chains.chains.len(), chains.overrides);
                state.built.set(rows::set_chains(chains));
                state.built_for.set(table);
            }
        }
        rows::chains()
    }

    /// `CompressionService.loadRecipes`: the crafting recipes of the server's
    /// table, in its order, and the data map.
    fn megacells_load_chains(&self) -> Chains {
        let mut keys: HashMap<String, Option<ItemKey>> = HashMap::new();
        let mut key = |id: &str| {
            keys.entry(id.to_owned()).or_insert_with(|| horizon_client::ae2_cell_codec::key(&ItemStack::new(id, 1)).ok()).clone()
        };
        let mut recipes = Vec::new();
        for (id, recipe) in &self.recipes().crafting {
            let (ingredients, result): (Vec<&[String]>, _) = match recipe {
                Recipe::Shaped { cells, result, .. } => (cells.iter().map(|cell| cell.as_ref().map_or(&[][..], |i| i.1.as_slice())).collect(), result),
                Recipe::Shapeless { ingredients, result, .. } => (ingredients.iter().map(|i| i.1.as_slice()).collect(), result),
                Recipe::Special(_) => continue,
            };
            let Some(item) = &result.item else { continue };
            let stack = ItemStack { item: item.clone(), count: 1, damage: result.damage, extra: result.extra.clone() };
            let Ok(result_key) = horizon_client::ae2_cell_codec::key(&stack) else { continue };
            let ingredients = ingredients.into_iter().map(|items| items.iter().filter_map(|id| key(id)).collect()).collect();
            recipes.push(CraftingRecipe { id: id.clone(), ingredients, result: result_key, count: result.count });
        }
        let overrides = self.megacells_compression_overrides();
        ae2_megacells_core::load_recipes(&recipes, &|id: &str| overrides.get(id).cloned())
    }

    /// The `megacells:compression_overrides` data map: MEGA's file from the
    /// content, then the world's data packs', tags resolved (NeoForge's
    /// `DataMapFile`: values, then removals).
    fn megacells_compression_overrides(&self) -> HashMap<String, OverrideValue> {
        let mut map = std::collections::BTreeMap::new();
        let tags = &self.items().item_tags;
        if let Ok(bytes) = std::fs::read(self.options.resources.join(OVERRIDES)) {
            if let Ok(file) = serde_json::from_slice::<Value>(&bytes) {
                crate::game::native_data_maps::apply_file(&mut map, &file, tags);
            }
        }
        let pack_key = "data_maps/megacells:item/compression_overrides";
        if let Some((_, versions)) = self.world_tables.as_ref().and_then(|t| t.files.iter().find(|(k, _)| k == pack_key)) {
            for bytes in versions {
                if let Ok(file) = serde_json::from_slice::<Value>(bytes) {
                    crate::game::native_data_maps::apply_file(&mut map, &file, tags);
                }
            }
        }
        let items = self.items();
        let known = |id: &str| items.get(id).is_some();
        let loaded = |modid: &str| modid == "minecraft" || modid == "neoforge" || self.megacells_mod_loaded(modid);
        map.into_iter().filter_map(|(id, value)| override_value(&value, &known, &loaded).map(|value| (id, value))).collect()
    }

    /// `ModList.isLoaded`: a namespace with items in the game.
    fn megacells_mod_loaded(&self, modid: &str) -> bool {
        let prefix = format!("{modid}:");
        self.items().items.keys().any(|id| id.starts_with(&prefix))
    }

    /// `SyncCompressionChainsPacket` to each client that does not have the
    /// server's current chains: on joining, and after a reload.
    pub(in crate::game) fn lan_megacells_sync_chains(&mut self, host: &mut LanHost) {
        let chains = self.megacells_chains();
        let generation = self.ae2x.megacells.compression.built.get();
        let conns: Vec<u32> = host.hub.guests.iter().map(|g| g.conn).collect();
        self.ae2x.megacells.compression.sent.retain(|conn, _| conns.contains(conn));
        let mut data = None;
        for conn in conns {
            if self.ae2x.megacells.compression.sent.get(&conn) == Some(&generation) {
                continue;
            }
            let data = data.get_or_insert_with(|| encode_chains(&chains)).clone();
            host.hub.send_to(conn, &C::CustomPayload(CustomPayload { channel: SYNC.into(), data }));
            self.ae2x.megacells.compression.sent.insert(conn, generation);
        }
    }

    /// `CompressionService.syncToClient`: the chains the server sent.
    pub(in crate::game) fn megacells_chains_payload(&mut self, channel: &str, data: &[u8]) -> bool {
        if channel != SYNC {
            return false;
        }
        if let Some(chains) = decode_chains(data) {
            rows::set_chains(chains);
        }
        true
    }

    /// `CellWorkbenchScreenMixin.updateCutoffButton`: the cutoff item a bulk
    /// cell with a chain shows on the button, while a Compression Card is in;
    /// none hides the button.
    pub(in crate::game) fn megacells_workbench_cutoff(&self, cell: Option<&ItemStack>) -> Option<ItemStack> {
        let cell = cell.filter(|cell| cell.item.as_ref() == rows::BULK_ITEM_CELL)?;
        let BulkStack { cell } = self.megacells_bulk_cell(cell).ok()?;
        if !cell.has_compression_chain() || !cell.compression_enabled() {
            return None;
        }
        cell.cutoff_item().and_then(|key| horizon_client::ae2_cell_codec::prototype(key).ok())
    }

    /// `mega$nextCompressionLimit` on the server: the workbench's bulk cell
    /// switches its cutoff (`switchCompressionCutoff`) and the host saves.
    /// False when the workbench holds no bulk cell.
    pub(in crate::game) fn megacells_next_compression_limit(&self, machine: &mut Machine, backwards: bool) -> bool {
        let Some(stack) = machine.slots.first_mut().and_then(Option::as_mut).filter(|cell| cell.item.as_ref() == rows::BULK_ITEM_CELL) else {
            return false;
        };
        let Ok(BulkStack { mut cell }) = self.megacells_bulk_cell(stack) else { return false };
        cell.switch_compression_cutoff(backwards);
        rows::store_bulk_data(stack, cell.data()).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The packet's chains come back as they went.
    #[test]
    fn chains_survive_the_sync_packet() {
        let chains = super::super::bulk::tests::iron();
        let decoded = decode_chains(&encode_chains(&chains)).unwrap();
        assert_eq!(decoded.chains, chains.chains);
        assert_eq!(decoded.chains[0].to_string(), "[1x → minecraft:iron_nugget, 9x → minecraft:iron_ingot, 9x → minecraft:iron_block]");
    }

    /// `CompressionOverride.CODEC`: NONE, air, an item, an unknown id; an
    /// entry for a mod that is not loaded is left out.
    #[test]
    fn override_values_read_as_megas_codec() {
        let known = |id: &str| id == "minecraft:snow_block";
        let loaded = |modid: &str| modid == "minecraft";
        assert_eq!(override_value(&json!({"variant": "NONE"}), &known, &loaded), Some(OverrideValue::None));
        assert_eq!(override_value(&json!({"variant": "minecraft:air"}), &known, &loaded), Some(OverrideValue::None));
        assert_eq!(override_value(&json!({"variant": "snow_block"}), &known, &loaded), Some(OverrideValue::Variant("minecraft:snow_block".into())));
        assert_eq!(override_value(&json!({"variant": "minecraft:nothing"}), &known, &loaded), None);
        let conditional = json!({"neoforge:conditions": [{"type": "neoforge:mod_loaded", "modid": "mysticalagriculture"}], "variant": "NONE"});
        assert_eq!(override_value(&conditional, &known, &loaded), None);
    }
}
