//! `/ae2 requester ...`: a requester's rows, buffers and statuses, and the
//! edits its menu makes, for tests and debugging.
use super::{Config, Key, core, is_requester, rows};
use crate::game::{Game, ae2_global::GridAddress, ae2_lab};
use horizon_client::{containers::BlockEntity, items::ItemStack};
use serde_json::{Value, json};

const USAGE: &str = "usage: /ae2 requester status|set|toggle|clear x y z [row] [item|fluid:<id>] [amount] [batch]";

/// `minecraft:stick` is an item, `fluid:minecraft:water` a fluid.
fn parse_key(word: &str) -> Result<Key, String> {
    if let Some(id) = word.strip_prefix("fluid:") {
        return Ok(Key { key: ae2_cell_core::ItemKey { item_id: id.into(), components: Vec::new() }, fluid: true });
    }
    Ok(Key { key: ae2_lab::key(&ItemStack::new(word, 1))?, fluid: false })
}
fn key_json(key: Option<&Key>) -> Value {
    key.map_or(Value::Null, |k| json!({"id": k.key.item_id, "fluid": k.fluid}))
}

impl Game<'_> {
    pub(in crate::game) fn ae2_requester_command(&mut self, w: &[&str]) -> Result<Value, String> {
        if w.len() < 4 {
            return Err(USAGE.into());
        }
        let pos = (
            w[1].parse().map_err(|_| "x coordinate")?,
            w[2].parse().map_err(|_| "y coordinate")?,
            w[3].parse().map_err(|_| "z coordinate")?,
        );
        if !matches!(self.world.block_entities.get(&pos), Some(BlockEntity::Ae2(m)) if is_requester(m)) && !self.ae2_ensure_machine(pos) {
            return Err("there is no ME Requester at that position".into());
        }
        let address = self.ae2_local_address(pos).ok_or("invalid AE2 anchor")?;
        let row = || -> Result<usize, String> {
            let row: usize = w.get(4).ok_or("missing row")?.parse().map_err(|_| "row")?;
            (row < Config::get().requests).then_some(row).ok_or_else(|| "no such row".into())
        };
        match w[0] {
            "status" => self.merequester_status(address),
            "set" => {
                let (row, key) = (row()?, parse_key(w.get(5).ok_or("missing key")?)?);
                let amount: i64 = w.get(6).ok_or("missing amount")?.parse().map_err(|_| "amount")?;
                let batch: Option<i64> = w.get(7).map(|b| b.parse().map_err(|_| "batch")).transpose()?;
                let unit = key.amount_per_unit();
                self.merequester_edit(address, |r| {
                    let request = &mut r.requests[row];
                    let mut changes = vec![(row, request.update_key(Some((key, amount)), unit))];
                    if let Some(batch) = batch {
                        changes.push((row, request.update_batch(batch)));
                    }
                    changes
                });
                self.merequester_status(address)
            }
            "toggle" => {
                let row = row()?;
                let on = match w.get(5).copied() {
                    Some("on") => true,
                    Some("off") => false,
                    _ => return Err("toggle needs on or off".into()),
                };
                self.merequester_edit(address, |r| vec![(row, r.requests[row].update_state(on))]);
                self.merequester_status(address)
            }
            "clear" => {
                let row = row()?;
                self.merequester_edit(address, |r| vec![(row, r.requests[row].update_key(None, 1))]);
                self.merequester_status(address)
            }
            _ => Err(USAGE.into()),
        }
    }

    fn merequester_status(&self, address: GridAddress) -> Result<Value, String> {
        let r = self.merequester_load(address).ok_or("no ME Requester there")?;
        let machine_config = match self.ae2_block_entity(address) {
            Some(BlockEntity::Ae2(m)) => m.config.clone(),
            _ => return Err("no ME Requester there".into()),
        };
        let network = self.ae2_network();
        let node = network.analyze().ok().and_then(|a| a.nodes.get(&address.network_pos()).map(|s| (s.online, s.powered)));
        let rows_json: Vec<Value> = r
            .requests
            .iter()
            .zip(&r.storages)
            .zip(&r.states)
            .enumerate()
            .map(|(i, ((q, s), state))| {
                let link = match state {
                    core::State::Link(l) => json!(l),
                    _ => Value::Null,
                };
                json!({
                    "row": i, "state": q.state, "key": key_json(q.key.as_ref()), "amount": q.amount, "batch": q.batch,
                    "status": q.client_status.name(), "server_status": state.status().name(), "locked": q.client_status.locks_request(),
                    "buffer": s.buffer_amount(), "buffer_key": key_json(s.key.as_ref()), "pending": s.pending, "known": s.known,
                    "link": link, "link_done": rows::link(&machine_config, i).is_some_and(|(_, d)| d),
                })
            })
            .collect();
        let config = Config::get();
        Ok(json!({
            "online": node.is_some_and(|n| n.0), "powered": node.is_some_and(|n| n.1),
            "active": rows::active(&machine_config), "jobs": self.merequester_jobs(address),
            "note": self.ae2x.merequester.notes.get(&address),
            "config": {"requests": config.requests, "idle_energy": config.idle_energy, "require_channel": config.require_channel},
            "rows": rows_json,
        }))
    }
}
