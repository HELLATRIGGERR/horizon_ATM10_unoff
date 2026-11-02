//! The requester's menu on the server (`RequesterMenu`,
//! `AbstractRequesterMenu`, `RequesterTerminalMenu`): a 9 x 6 window whose
//! container slots hold the ghost keys of the lines on screen, the player's
//! inventory under them, and the lines themselves as a `merequester:view`
//! payload (`RequesterSyncPacket`). The screen's edits come back as
//! `merequester:*` payloads (`RequestUpdatePacket`).
use super::{Config, Key, Requester, core, is_requester, rows};
use crate::game::{Game, ae2_fluids, ae2_global::GridAddress, ae2_lab, lan::LanHost, lan_crafting};
use horizon_client::{
    ae2_generic_stack::{self, GenericStack},
    containers::BlockEntity,
    items::ItemStack,
    menu::{Input, Kind, Menu},
};
use horizon_protocol::packets::{
    configuration::CustomPayload,
    play::{self, Clientbound as C},
};
use horizon_world::nbt::Tag;
use serde_json::{Value, json};

/// The screen's container slots: one ghost slot for each line on screen.
pub(in crate::game) const SLOTS: usize = 54;
pub(in crate::game) const VIEW: &str = "merequester:view";
/// The requester terminal's part item.
pub(in crate::game) const TERMINAL: &str = rows::TERMINAL;

#[derive(Clone, Debug, PartialEq)]
enum Line {
    /// A requester name and how many requesters share it.
    Header(String, usize),
    /// Row `index` of requester `requester` (an index into `requesters`).
    Row { requester: usize, index: usize },
}

/// One player's open requester menu.
pub(in crate::game) struct Session {
    window: i32,
    state: i32,
    pos: (i32, i32, i32),
    /// The terminal part's face; none for the requester block.
    face: Option<String>,
    dim: String,
    offset: usize,
    search: String,
    /// The requesters on screen, by the id the screen knows them by.
    requesters: Vec<GridAddress>,
    lines: Vec<Line>,
    sent: Vec<u8>,
    sent_items: Vec<Option<ItemStack>>,
    menu: Menu,
}

/// `RequestManager.convertToSuitableStack`: a wrapped generic stack is its
/// key and amount (an item at least one), any other item itself.
fn suitable(stack: &ItemStack) -> Option<(Key, i64)> {
    if stack.count == 0 {
        return None;
    }
    if let Ok(Some(generic)) = ae2_generic_stack::unwrap_item_stack(stack) {
        let amount = if generic.fluid { generic.amount } else { generic.amount.max(1) };
        return Some((Key { key: generic.key, fluid: generic.fluid }, amount));
    }
    Some((Key { key: ae2_lab::key(stack).ok()?, fluid: false }, i64::from(stack.count)))
}
/// What a ghost slot shows: the item, or the fluid wrapped.
fn shown(key: &Key) -> Option<ItemStack> {
    if key.fluid {
        ae2_generic_stack::wrap(&GenericStack { key: key.key.clone(), fluid: true, amount: 0 }).ok()
    } else {
        ae2_lab::prototype(&key.key).ok()
    }
}
/// `RequesterBlockEntity.getSortValue`.
fn sort_value(pos: (i32, i32, i32)) -> i64 {
    (i64::from(pos.2) << 24) ^ (i64::from(pos.0) << 8) ^ i64::from(pos.1)
}

impl Game<'_> {
    /// The terminal part on `face` of the cable bus at `pos`.
    pub(in crate::game) fn merequester_terminal_at(&self, pos: (i32, i32, i32), face: &str) -> bool {
        let Some(BlockEntity::Ae2(m)) = self.world.block_entities.get(&pos) else { return false };
        let Some(Tag::List(parts)) = m.config.get(&"parts".into()) else { return false };
        parts.iter().any(|p| p.get("face") == Some(&Tag::String(face.into())) && p.get("id") == Some(&Tag::String(TERMINAL.into())))
    }
    /// `getTerminalName`: the requester's custom name, or the block's.
    fn merequester_name(&self, address: GridAddress) -> String {
        self.ae2_address_parts(address)
            .and_then(|(dim, pos)| self.ae2_world_for(dim)?.block_names.get(&pos).cloned())
            .unwrap_or_else(|| self.pack.names.display_item(rows::REQUESTER))
    }
    /// The requesters a menu shows: the block's own, or the terminal grid's
    /// active ones (`getActiveMachines`).
    fn merequester_listed(&self, pos: (i32, i32, i32), face: Option<&str>) -> Vec<GridAddress> {
        let Some(address) = self.ae2_local_address(pos) else { return Vec::new() };
        if face.is_none() {
            return vec![address];
        }
        let network = self.ae2_network();
        let Ok(analysis) = network.analyze() else { return Vec::new() };
        let Some(node) = analysis.nodes.get(&address.network_pos()) else { return Vec::new() };
        let Some(component) = analysis.components.get(node.component) else { return Vec::new() };
        component
            .positions
            .iter()
            .filter(|p| analysis.nodes.get(p).is_some_and(|s| s.online))
            .filter_map(|p| GridAddress::from_network(*p))
            .filter(|a| self.merequester_machine(*a).is_some())
            .collect()
    }
    /// The menu's lines (`RequesterScreen.refreshList`,
    /// `RequesterTerminalScreen.refreshList`): the block lists its rows; the
    /// terminal groups requesters by name, sorted, each name's requesters by
    /// position, filtered by the search.
    fn merequester_lines(&self, s: &Session) -> (Vec<GridAddress>, Vec<Line>, Vec<Requester>) {
        let listed = self.merequester_listed(s.pos, s.face.as_deref());
        let loaded: Vec<(GridAddress, Requester)> = listed.into_iter().filter_map(|a| Some((a, self.merequester_load(a)?))).collect();
        if s.face.is_none() {
            let (addresses, requesters): (Vec<_>, Vec<_>) = loaded.into_iter().unzip();
            let lines = requesters.first().map_or(0, |r: &Requester| r.size());
            return (addresses, (0..lines).map(|index| Line::Row { requester: 0, index }).collect(), requesters);
        }
        let query = s.search.to_lowercase();
        let mut groups: std::collections::BTreeMap<String, Vec<(i64, GridAddress, Requester)>> = Default::default();
        for (address, r) in loaded {
            let name = self.merequester_name(address);
            let found = query.is_empty()
                || name.to_lowercase().contains(&query)
                || r.requests.iter().any(|q| q.key.as_ref().is_some_and(|k| self.merequester_key_name(k).to_lowercase().contains(&query)));
            if found {
                let sort = self.ae2_address_parts(address).map_or(0, |(_, p)| sort_value(p));
                groups.entry(name).or_default().push((sort, address, r));
            }
        }
        let (mut addresses, mut lines, mut requesters) = (Vec::new(), Vec::new(), Vec::new());
        for (name, mut members) in groups {
            members.sort_by_key(|(sort, _, _)| *sort);
            lines.push(Line::Header(name, members.len()));
            for (_, address, r) in members {
                let id = addresses.len();
                lines.extend((0..r.size()).map(|index| Line::Row { requester: id, index }));
                addresses.push(address);
                requesters.push(r);
            }
        }
        (addresses, lines, requesters)
    }
    /// A key's name as the terminal's search reads it.
    pub(in crate::game) fn merequester_key_name(&self, key: &Key) -> String {
        if key.fluid {
            return self.pack.names.display_item(&key.key.item_id);
        }
        ae2_lab::prototype(&key.key)
            .map(|item| horizon_client::stations::hover_name(&|id| self.pack.names.display_item(id), &item))
            .unwrap_or_else(|_| key.key.item_id.clone())
    }

    /// The guest uses the requester block or a requester terminal.
    pub(in crate::game) fn lan_open_merequester(&mut self, host: &mut LanHost, conn: u32, pos: (i32, i32, i32), face: &str) -> bool {
        let block = matches!(self.world.block_entities.get(&pos), Some(BlockEntity::Ae2(m)) if is_requester(m));
        let terminal = !block && self.merequester_terminal_at(pos, face) && self.ae2_part_online(pos, face);
        if !block && !terminal {
            return false;
        }
        // `RequesterBlock.useWithoutItem`: sneaking is the alternate use.
        if block && host.hub.guests.iter().any(|g| g.conn == conn && g.sneaking) {
            return false;
        }
        self.lan_pattern_access_closed(host, conn, None);
        self.lan_ae2_terminal_closed(host, conn, None);
        self.lan_ae2_machine_closed(host, conn, None);
        self.lan_ae2_portable_closed(host, conn, None);
        self.lan_ae2_network_tool_closed(host, conn, None);
        self.lan_ae2_anchor_closed(host, conn, None);
        self.lan_ae2_bus_closed(host, conn, None);
        self.lan_craft_confirm_closed(host, conn, None);
        self.lan_station_closed(host, conn, None);
        self.lan_crafting_closed(host, conn, None);
        self.guest_container_closed(host, conn);
        self.lan_merequester_closed(host, conn, None);
        let title = if block { self.pack.names.display_item(rows::REQUESTER) } else { "ME Requ. Terminal".to_string() };
        let Some(window) = host.hub.open_window(conn, "minecraft:generic_9x6", horizon_protocol::text::literal(&title)) else { return true };
        let session = Session {
            window, state: 0, pos, face: terminal.then(|| face.to_string()), dim: self.dim.id().to_string(),
            offset: 0, search: String::new(), requesters: Vec::new(), lines: Vec::new(), sent: Vec::new(),
            sent_items: Vec::new(), menu: Menu::new(Kind::Chest(6)),
        };
        self.ae2x.merequester.sessions.insert(conn, session);
        self.lan_send_merequester(host, conn, true);
        true
    }
    pub(in crate::game) fn lan_merequester_closed(&mut self, host: &mut LanHost, conn: u32, window: Option<i32>) -> bool {
        let open = self.ae2x.merequester.sessions.get(&conn).is_some_and(|s| window.is_none_or(|w| w == s.window));
        if !open {
            return false;
        }
        self.ae2x.merequester.sessions.remove(&conn);
        if let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) {
            self.guest_carried_back(host, gi);
        }
        true
    }
    /// Whether the session's player may still use it (`stillValid`).
    fn merequester_valid(&self, host: &LanHost, conn: u32, s: &Session) -> bool {
        let here = host.hub.guests.iter().any(|g| {
            g.conn == conn && host.hub.here(g) && g.next_window == s.window && g.within_block_range([s.pos.0, s.pos.1, s.pos.2], 4.0)
        });
        let target = match &s.face {
            None => matches!(self.world.block_entities.get(&s.pos), Some(BlockEntity::Ae2(m)) if is_requester(m)),
            Some(face) => self.merequester_terminal_at(s.pos, face),
        };
        here && target && s.dim == self.dim.id()
    }
    /// `broadcastChanges`: the lines and ghost slots when they changed, or
    /// all of them with `all`.
    fn lan_send_merequester(&mut self, host: &mut LanHost, conn: u32, all: bool) {
        let Some(mut s) = self.ae2x.merequester.sessions.remove(&conn) else { return };
        // Another menu took the player's window: this one is gone already.
        if host.hub.guests.iter().find(|g| g.conn == conn).is_none_or(|g| g.next_window != s.window) {
            return;
        }
        if !self.merequester_valid(host, conn, &s) {
            host.hub.send_to(conn, &C::ContainerClose(play::ContainerClose { container: s.window }));
            if let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) {
                self.guest_carried_back(host, gi);
            }
            return;
        }
        let (requesters, lines, loaded) = self.merequester_lines(&s);
        s.offset = s.offset.min(lines.len().saturating_sub(1));
        let line_json: Vec<Value> = lines
            .iter()
            .map(|line| match line {
                Line::Header(name, count) => json!({"h": name, "n": count}),
                Line::Row { requester, index } => {
                    let q = &loaded[*requester].requests[*index];
                    json!({"r": requester, "i": index, "on": q.state, "amount": q.amount, "batch": q.batch,
                        "status": q.client_status.name(), "key": q.key.is_some(), "fluid": q.key.as_ref().is_some_and(|k| k.fluid)})
                }
            })
            .collect();
        let view = serde_json::to_vec(&json!({"window": s.window, "terminal": s.face.is_some(), "offset": s.offset, "lines": line_json}))
            .unwrap_or_default();
        let mut items: Vec<Option<ItemStack>> = (0..SLOTS)
            .map(|j| match lines.get(s.offset + j) {
                Some(Line::Row { requester, index }) => loaded[*requester].requests[*index].key.as_ref().and_then(shown),
                _ => None,
            })
            .collect();
        if all || view != s.sent {
            host.hub.send_to(conn, &C::CustomPayload(CustomPayload { channel: VIEW.into(), data: view.clone() }));
        }
        let Some(g) = host.hub.guests.iter().find(|g| g.conn == conn) else { return };
        let ghosts = items.clone();
        items.extend((9..36).chain(0..9).map(|i| g.inventory[i].clone()));
        if all || ghosts != s.sent_items {
            s.state = (s.state + 1) & 0x7fff;
            let carried = host.hub.stack(&g.carried.clone());
            let wire = items.iter().map(|item| host.hub.stack(item)).collect();
            host.hub.send_to(conn, &C::ContainerSetContent(play::ContainerSetContent { container: s.window, state: s.state, items: wire, carried }));
        }
        s.sent = view;
        s.sent_items = ghosts;
        s.requesters = requesters;
        s.lines = lines;
        self.ae2x.merequester.sessions.insert(conn, s);
    }
    pub(in crate::game) fn lan_refresh_merequester(&mut self, host: &mut LanHost) {
        let conns: Vec<u32> = self.ae2x.merequester.sessions.keys().copied().collect();
        for conn in conns {
            // A player in another level is seen on that level's pass.
            if host.hub.guests.iter().any(|g| g.conn == conn && !host.hub.here(g)) {
                continue;
            }
            self.lan_send_merequester(host, conn, false);
        }
    }

    /// `AbstractRequesterMenu.doAction` on a ghost slot, and the player's
    /// inventory as any menu's.
    pub(in crate::game) fn lan_merequester_click(&mut self, host: &mut LanHost, conn: u32, p: &play::ContainerClick) -> bool {
        let Some(mut s) = self.ae2x.merequester.sessions.remove(&conn) else { return false };
        if p.container != s.window {
            self.ae2x.merequester.sessions.insert(conn, s);
            return false;
        }
        let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn && host.hub.here(g)) else {
            self.ae2x.merequester.sessions.insert(conn, s);
            return true;
        };
        let input = lan_crafting::input(p.input);
        let button = i32::from(p.button);
        let slot = i32::from(p.slot);
        if (0..SLOTS as i32).contains(&slot) {
            if let Some(Line::Row { requester, index }) = s.lines.get(s.offset + slot as usize).cloned() {
                let address = s.requesters[requester];
                let carried = host.hub.guests[gi].carried.clone();
                let creative = host.hub.guests[gi].creative();
                if let Some(cursor) = self.merequester_ghost(address, index, input, button, carried.as_ref(), creative) {
                    host.hub.guests[gi].carried = cursor;
                }
            }
        } else if (SLOTS as i32..SLOTS as i32 + 36).contains(&slot) && input == Input::QuickMove {
            // `transferStackToMenu`: the first free row takes the item's key; the item stays.
            let index = if slot < SLOTS as i32 + 27 { 9 + (slot as usize - SLOTS) } else { slot as usize - SLOTS - 27 };
            if let Some(stack) = host.hub.guests[gi].inventory[index].clone() {
                let mut order: Vec<GridAddress> = s.requesters.clone();
                order.sort_by_key(|a| self.ae2_address_parts(*a).map_or(0, |(_, p)| sort_value(p)));
                for address in order {
                    let Some(free) = self.merequester_load(address).and_then(|r| r.first_available()) else { continue };
                    if let Some((key, _)) = suitable(&stack) {
                        let unit = key.amount_per_unit();
                        self.merequester_edit(address, |r| vec![(free, r.requests[free].update_key(Some((key, 1)), unit))]);
                    }
                    break;
                }
            }
        } else {
            self.guest_menu_op(host, gi, &mut s.menu, |m, env| {
                let _ = m.click(env, slot, button, input);
            });
        }
        self.ae2x.merequester.sessions.insert(conn, s);
        self.lan_send_merequester(host, conn, true);
        true
    }
    /// A click on row `index`'s ghost slot; the new cursor stack when the
    /// click changes it.
    fn merequester_ghost(&mut self, address: GridAddress, index: usize, input: Input, button: i32, carried: Option<&ItemStack>, creative: bool) -> Option<Option<ItemStack>> {
        let r = self.merequester_load(address)?;
        let request = r.requests.get(index)?;
        // `RequestSlot.isLocked`: no edits while the row's job runs or exports.
        if request.client_status.locks_request() {
            return None;
        }
        let set = |game: &mut Self, stack: Option<(Key, i64)>| {
            let unit = stack.as_ref().map_or(1, |(k, _)| k.amount_per_unit());
            game.merequester_edit(address, |r| vec![(index, r.requests[index].update_key(stack, unit))]);
        };
        match (input, button) {
            // EMPTY_ITEM: a right click with a filled bucket asks for its fluid.
            (Input::Pickup, 1) if carried.and_then(|c| ae2_fluids::bucket_fluid(&c.item)).is_some() => {
                let fluid = ae2_fluids::bucket_fluid(&carried?.item)?;
                set(self, Some((Key { key: ae2_fluids::fluid_key(fluid), fluid: true }, 1000)));
            }
            // PICKUP_OR_SET_DOWN: the cursor's stack, or nothing.
            (Input::Pickup, 0) => set(self, carried.and_then(suitable)),
            // SPLIT_OR_PLACE_SINGLE: one of the cursor's stack, or nothing.
            (Input::Pickup, 1) => set(self, carried.map(|c| ItemStack { count: 1, ..c.clone() }).as_ref().and_then(suitable)),
            // SHIFT_CLICK clears the row.
            (Input::QuickMove, 0) => set(self, None),
            // CREATIVE_DUPLICATE: a full stack of the row's item.
            (Input::Clone, _) if creative && carried.is_none() => {
                let key = request.key.as_ref().filter(|k| !k.fluid)?;
                let mut item = ae2_lab::prototype(&key.key).ok()?;
                item.count = self.res.items().stack_max(&item);
                return Some(Some(item));
            }
            _ => {}
        }
        None
    }

    /// The screen's edits (`RequestUpdatePacket`), its scroll and its search.
    pub(in crate::game) fn lan_merequester_payload(&mut self, host: &mut LanHost, conn: u32, channel: &str, data: &[u8]) {
        let Ok(value) = serde_json::from_slice::<Value>(data) else { return };
        let Some(s) = self.ae2x.merequester.sessions.get_mut(&conn) else { return };
        if value.get("window").and_then(Value::as_i64) != Some(i64::from(s.window)) {
            return;
        }
        let row = || {
            let requester = value.get("r")?.as_u64()? as usize;
            let index = value.get("i")?.as_u64()? as usize;
            (index < Config::get().requests).then_some((requester, index))
        };
        match channel {
            "merequester:scroll" => s.offset = value.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize,
            "merequester:search" => {
                s.search = value.get("text").and_then(Value::as_str).unwrap_or("").chars().take(128).collect();
                s.offset = 0;
            }
            "merequester:state" => {
                let (Some((requester, index)), Some(on)) = (row(), value.get("on").and_then(Value::as_bool)) else { return };
                let Some(&address) = s.requesters.get(requester) else { return };
                self.merequester_edit(address, |r| vec![(index, r.requests[index].update_state(on))]);
            }
            "merequester:numbers" => {
                let Some((requester, index)) = row() else { return };
                let (Some(amount), Some(batch)) = (value.get("amount").and_then(Value::as_i64), value.get("batch").and_then(Value::as_i64)) else { return };
                let Some(&address) = s.requesters.get(requester) else { return };
                self.merequester_edit(address, |r| {
                    let request = &mut r.requests[index];
                    vec![(index, request.update_amount(amount)), (index, request.update_batch(batch))]
                });
            }
            _ => return,
        }
        self.lan_send_merequester(host, conn, false);
    }
}

/// `RequestStatus.locksRequest` of a status the screen got.
pub(in crate::game) fn locked(status: &str) -> bool {
    core::Status::from_name(status).is_some_and(core::Status::locks_request)
}
