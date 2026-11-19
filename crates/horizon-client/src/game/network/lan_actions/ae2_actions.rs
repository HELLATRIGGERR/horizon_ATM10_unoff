//! The fork's AE2 (and addon) actions of a guest: its menus, its item and block uses, its
//! clicks, text fields and channels, on upstream's LAN host model. The host stays in
//! `self.lan`; AE2's own code takes it out as a [`HostTurn`] (`game/network/lan_host_turn.rs`)
//! through `with_lan_host`, and lends it back for upstream's helpers (`as_guest`,
//! `lan_open_container`) with `HostTurn::back`. Upstream's action files call these at the
//! fork's points, each under `cfg(feature = "ae2-lab")`.
#![cfg(feature = "ae2-lab")]
use super::*;
use crate::game::{hit::direction, lan_host_turn::HostTurn};
use horizon_client::{ae2_world::MachineKind, text::Text};

/// The face names AE2's code takes, by `Direction.get3DDataValue` (the packets' face).
fn face_name(face: i32) -> Option<&'static str> {
    Some(match face {
        0 => "down",
        1 => "up",
        2 => "north",
        3 => "south",
        4 => "west",
        5 => "east",
        _ => return None,
    })
}

/// The packets' face of the unit vector `face`.
fn face_index(face: [i32; 3]) -> i32 {
    (0..6).find(|&k| direction(k) == face).unwrap_or(-1)
}

/// The packets' hand: 1 for the off hand.
fn hand_index(hand: Hand) -> i32 {
    i32::from(hand == Hand::Off)
}

/// The chat lines a use captured, to the guest.
fn tell(host: &HostTurn, conn: u32, lines: Vec<Text>, overlay: bool) {
    for line in lines {
        host.hub.send_to(
            conn,
            &C::SystemChat(play::SystemChat {
                content: horizon_client::lan::data::component_nbt(&line.component()),
                overlay,
            }),
        );
    }
}

impl Game<'_> {
    /// `as_guest` from AE2 code: the host goes back into `self.lan` for the turn.
    fn ae2_guest<R>(
        &mut self,
        host: &mut HostTurn,
        conn: u32,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Option<R> {
        host.back(self, |g| g.as_guest(conn, f))
    }

    /// `f` with `hand`'s item, in the guest's turn, with its chat lines captured.
    fn ae2_guest_captured<R>(
        &mut self,
        host: &mut HostTurn,
        conn: u32,
        hand: Hand,
        f: impl FnOnce(&mut Self) -> R,
    ) -> Option<(R, Vec<Text>)> {
        self.ae2_guest(host, conn, |g| {
            g.chat_capture = Some(Vec::new());
            let used = g.with_hand(hand, f);
            (used, g.chat_capture.take().unwrap_or_default())
        })
    }

    /// Every AE2 menu the guest has open, closed: another menu replaced it.
    pub(in crate::game) fn lan_ae2_menus_closed(&mut self, conn: u32) {
        self.with_lan_host(|g, host| {
            g.lan_ae2_terminal_closed(host, conn, None);
            g.lan_ae2_machine_closed(host, conn, None);
            g.lan_ae2_bus_closed(host, conn, None);
            g.lan_ae2_portable_closed(host, conn, None);
            g.lan_ae2_network_tool_closed(host, conn, None);
            g.lan_ae2_anchor_closed(host, conn, None);
            g.lan_craft_confirm_closed(host, conn, None);
            g.lan_pattern_access_closed(host, conn, None);
        });
    }

    /// Per tick: what the guests' AE2 menus show.
    pub(super) fn lan_ae2_menus_tick(&mut self) {
        self.with_lan_host(|g, host| {
            g.lan_refresh_ae2_anchors(host);
            g.lan_refresh_ae2_network_tools(host);
            g.lan_refresh_craft_confirms(host);
            g.lan_refresh_pattern_access(host);
            g.lan_refresh_ae2x(host);
            g.lan_refresh_ae2_buses(host);
            g.lan_refresh_ae2_terminals(host);
            g.lan_refresh_ae2_portables(host);
            g.lan_refresh_ae2_machines(host);
        });
    }

    /// `lan_open_container` for AE2's blocks: a terminal's or machine's menu, a sky stone
    /// chest's slots (whether one opened). None for a block that isn't AE2's: upstream's path.
    pub(super) fn lan_ae2_open_container(
        &mut self,
        conn: u32,
        pos: (i32, i32, i32),
    ) -> Option<bool> {
        if self.ae2_has_terminal(pos) {
            return Some(
                self.with_lan_host(|g, host| g.lan_open_ae2_terminal(host, conn, pos))
                    .unwrap_or(false),
            );
        }
        if self.ae2_has_machine_menu(pos) {
            return Some(
                self.with_lan_host(|g, host| g.lan_open_ae2_machine(host, conn, pos))
                    .unwrap_or(false),
            );
        }
        self.lan_ae2_menus_closed(conn);
        let kind = match self.world.block_entities.get(&pos) {
            Some(BlockEntity::Ae2(machine))
                if matches!(
                    machine.kind,
                    MachineKind::SkyStoneChest | MachineKind::SmoothSkyStoneChest
                ) =>
            {
                machine.kind
            }
            _ => return None,
        };
        let Some(incarnation) = self.world.block_entities.incarnation(&pos) else {
            return Some(false);
        };
        let slots = Slots::Ae2SkyChest(pos, kind, incarnation);
        let Some(items) = self
            .lan
            .as_ref()
            .and_then(|host| host.hub.guest(conn))
            .and_then(|g| slots.read(&self.world, g))
        else {
            return Some(false);
        };
        self.guest_container_closed(conn);
        self.lan_station_closed(conn, None);
        let Some(host) = self.lan.as_mut() else {
            return Some(false);
        };
        host.hub.open_container(
            conn,
            "minecraft:generic_9x4",
            literal("Sky Stone Chest"),
            pos,
            slots,
            items,
            None,
        );
        Some(true)
    }

    /// A guest's dig START on an AE2 block: a crafting monitor's craft or take, a facade's
    /// cycle. True when AE2 took it.
    pub(super) fn lan_ae2_dig_start(
        &mut self,
        conn: u32,
        gi: usize,
        pos: [i32; 3],
        face: u8,
    ) -> bool {
        self.with_lan_host(|g, host| {
            if !host.hub.guests[gi].within_block_range(pos, 0.0) {
                return false;
            }
            let at = (pos[0], pos[1], pos[2]);
            if let Some(name) = face_name(i32::from(face)) {
                if g.lan_open_monitor_craft(host, conn, at, name) {
                    return true;
                }
                if g.ae2_guest(host, conn, |g| g.ae2_try_monitor_attack(at, name))
                    .unwrap_or(false)
                {
                    host.hub.send_inventory(gi);
                    return true;
                }
            }
            let facade = g.ae2_guest_captured(host, conn, Hand::Main, |g| {
                g.ae2_try_facade_cycle(pos, direction(i32::from(face)), false)
            });
            if let Some((true, lines)) = facade {
                tell(host, conn, lines, true);
                host.hub.send_inventory(gi);
                return true;
            }
            false
        })
        .unwrap_or(false)
    }

    /// A guest's use on block `pos` (its face `face`) with `hand`, by AE2: its tools, parts,
    /// cables, memory cards and menus, in the fork's order. True when AE2 took it.
    pub(super) fn lan_ae2_use_item_on(
        &mut self,
        conn: u32,
        gi: usize,
        pos: [i32; 3],
        face: [i32; 3],
        hand: Hand,
    ) -> bool {
        let face_i = face_index(face);
        self.with_lan_host(|g, host| g.lan_ae2_use_item_on_turn(host, conn, gi, pos, face_i, hand))
            .unwrap_or(false)
    }

    fn lan_ae2_use_item_on_turn(
        &mut self,
        host: &mut HostTurn,
        conn: u32,
        gi: usize,
        pos: [i32; 3],
        face_i: i32,
        hand: Hand,
    ) -> bool {
        let hand_i = hand_index(hand);
        let at = (pos[0], pos[1], pos[2]);
        let face = direction(face_i);
        if host.hub.guests[gi].within_block_range(pos, 0.0) {
            if let Some((true, lines)) =
                self.ae2_guest_captured(host, conn, hand, |g| g.ae2_try_replicator(pos, face))
            {
                tell(host, conn, lines, false);
                host.hub.send_inventory(gi);
                return true;
            }
            if let Some((Some(placed), lines)) =
                self.ae2_guest_captured(host, conn, hand, |g| g.ae2_try_meteorite_placer(pos))
            {
                tell(host, conn, lines, false);
                if placed {
                    // Source sends the acting player nine full chunks after reporting
                    // successful live placement.
                    let (cx, cz) = (pos[0] >> 4, pos[2] >> 4);
                    let mut bodies = Vec::with_capacity(9);
                    for x in cx - 1..=cx + 1 {
                        for z in cz - 1..=cz + 1 {
                            if let Some(chunk) = self.world.chunks.get(&(x, z)) {
                                bodies.push(host.hub.encoder.body_in(chunk, &self.world));
                            }
                        }
                    }
                    host.hub.guests[gi].sender.send_many(bodies);
                }
                host.hub.send_inventory(gi);
                return true;
            }
            let used = |g: &mut Self, host: &mut HostTurn, f: &dyn Fn(&mut Self) -> bool| {
                g.ae2_guest(host, conn, |g| g.with_hand(hand, f))
                    .unwrap_or(false)
            };
            if used(self, host, &|g| g.ae2_try_eraser(pos)) {
                host.hub.send_inventory(gi);
                return true;
            }
            if self
                .ae2_guest(host, conn, |g| {
                    let hands_full = g.inventory[g.selected].is_some() || g.offhand.is_some();
                    if g.keys.raw(&KeyCode::ShiftLeft) && hands_full {
                        return false;
                    }
                    g.with_hand(hand, |g| g.ae2_try_debug_cube_use(pos))
                })
                .unwrap_or(false)
            {
                host.hub.send_inventory(gi);
                return true;
            }
            if used(self, host, &|g| g.ae2_try_debug_item_generator(pos))
                || used(self, host, &|g| g.ae2_cell_item_use(false))
            {
                host.hub.send_inventory(gi);
                return true;
            }
            if hand == Hand::Main
                && self
                    .ae2_guest(host, conn, |g| {
                        g.with_hand(Hand::Main, |g| {
                            g.ae2_try_wrench_rotate(pos, face) || g.ae2_try_wrench_disassemble(pos)
                        })
                    })
                    .unwrap_or(false)
            {
                host.hub.send_inventory(gi);
                return true;
            }
            if let Some(name) = face_name(face_i)
                && used(self, host, &|g| g.ae2_try_monitor_use(at, name))
            {
                host.hub.send_inventory(gi);
                return true;
            }
            if used(self, host, &|g| g.ae2_try_crafting_upgrade(pos, face))
                || used(self, host, &|g| g.ae2x_use_on(pos, face))
            {
                host.hub.send_inventory(gi);
                return true;
            }
        }
        if self.lan_open_ae2_network_tool_at(host, conn, hand_i, at, face_i) {
            return true;
        }
        if host.hub.guests[gi].within_block_range(pos, 0.0) {
            if self
                .ae2_guest(host, conn, |g| {
                    g.with_hand(hand, |g| {
                        let bypass = g.keys.raw(&KeyCode::ShiftLeft)
                            && (g.inventory[g.selected].is_some() || g.offhand.is_some());
                        (!bypass
                            && (g.ae2_try_crank_use(at)
                                || g.ae2_try_charger_use(at)
                                || g.ae2x_block_use(at)))
                            || g.ae2_try_tank_interaction(at)
                    })
                })
                .unwrap_or(false)
            {
                host.hub.send_inventory(gi);
                return true;
            }
            let side = face_name(face_i).unwrap_or("east");
            let facade = self.ae2_guest_captured(host, conn, hand, |g| {
                g.ae2_try_remove_facade(pos, face)
                    || g.ae2_try_facade_cycle(pos, face, true)
                    || g.ae2_try_remove_part(pos, face)
                    || g.ae2_try_facade(pos, face)
            });
            if let Some((true, lines)) = facade {
                tell(host, conn, lines, true);
                host.hub.send_inventory(gi);
                return true;
            }
            let memory = self.ae2_guest_captured(host, conn, hand, |g| {
                g.ae2_try_provider_memory(pos, side) || g.ae2_try_machine_memory_face(pos, side)
            });
            if let Some((true, lines)) = memory {
                tell(host, conn, lines, true);
                host.hub.send_inventory(gi);
                return true;
            }
            if self
                .ae2_guest(host, conn, |g| {
                    g.with_hand(hand, |g| g.ae2_try_p2p_card(at, side))
                })
                .unwrap_or(false)
            {
                host.hub.send_inventory(gi);
                return true;
            }
            if self
                .ae2_guest(host, conn, |g| {
                    g.with_hand(hand, |g| g.ae2_try_place_cable(pos, face))
                })
                .unwrap_or(false)
            {
                host.hub.send_inventory(gi);
                self.ae2_send_blocks(host, conn, pos, face);
                return true;
            }
            if let Some(name) = face_name(face_i)
                && self
                    .ae2_guest(host, conn, |g| {
                        g.with_hand(hand, |g| g.ae2_try_attach_part(at, name))
                    })
                    .unwrap_or(false)
            {
                host.hub.send_inventory(gi);
                self.ae2_send_blocks(host, conn, pos, face);
                return true;
            }
        }
        self.lan_ae2_open_block_menu(host, conn, gi, at, face)
    }

    /// The block at `pos` and the one beside it on `face`, to the guest as they are.
    fn ae2_send_blocks(&self, host: &HostTurn, conn: u32, pos: [i32; 3], face: [i32; 3]) {
        for at in [pos, [pos[0] + face[0], pos[1] + face[1], pos[2] + face[2]]] {
            let state = self.world.block(at[0], at[1], at[2]).0 as i32;
            host.hub
                .send_to(conn, &C::BlockUpdate(play::BlockUpdate { pos: at, state }));
        }
    }

    /// The menus AE2's blocks and parts open on a use: crafting CPUs, anchors, sky stone
    /// chests, pattern access, the addons', buses, the terminals and machines.
    fn lan_ae2_open_block_menu(
        &mut self,
        host: &mut HostTurn,
        conn: u32,
        gi: usize,
        target: (i32, i32, i32),
        face: [i32; 3],
    ) -> bool {
        let guest = &host.hub.guests[gi];
        let hands_full = guest.inventory[guest.selected].is_some() || guest.offhand.is_some();
        let menu_allowed = guest.within_block_range([target.0, target.1, target.2], 0.0)
            && !(guest.sneaking && hands_full);
        let hit_face = match face {
            [1, 0, 0] => Some(0),
            [-1, 0, 0] => Some(1),
            [0, 1, 0] => Some(2),
            [0, -1, 0] => Some(3),
            [0, 0, 1] => Some(4),
            [0, 0, -1] => Some(5),
            _ => None,
        };
        let side = match face {
            [0, -1, 0] => "down",
            [0, 1, 0] => "up",
            [0, 0, -1] => "north",
            [0, 0, 1] => "south",
            [-1, 0, 0] => "west",
            _ => "east",
        };
        let sky_chest = matches!(
            self.world.block_entities.get(&target),
            Some(BlockEntity::Ae2(m))
                if matches!(m.kind, MachineKind::SkyStoneChest | MachineKind::SmoothSkyStoneChest)
        );
        if menu_allowed {
            if self.lan_open_cpu_craft_status(host, conn, target)
                || self.lan_open_ae2_anchor(host, conn, target)
            {
                return true;
            }
            if sky_chest && host.back(self, |g| g.lan_open_container(conn, target)) {
                return true;
            }
            if self.lan_open_pattern_access(host, conn, target, side)
                || self.lan_open_ae2x(host, conn, target, side)
            {
                return true;
            }
            if let (Some(host_address), Some(face)) = (self.ae2_local_address(target), hit_face) {
                let part = crate::game::ae2_parts::PartAddress {
                    host: host_address,
                    face,
                };
                if self.lan_open_ae2_bus(host, conn, target, face) {
                    return true;
                }
                let kind = self.ae2_part_machine(part).ok().flatten().map(|m| m.kind);
                if kind == Some(MachineKind::Terminal)
                    && self.lan_open_ae2_view_terminal(host, conn, target, part)
                {
                    return true;
                }
                if kind == Some(MachineKind::CraftingTerminal)
                    && self.lan_open_ae2_crafting_terminal(host, conn, target, part)
                {
                    return true;
                }
                if kind == Some(MachineKind::PatternEncodingTerminal)
                    && self.lan_open_ae2_encoding_terminal(host, conn, target, part)
                {
                    return true;
                }
                if kind.is_some() && self.lan_open_ae2_part_machine(host, conn, target, part) {
                    return true;
                }
            }
        }
        if self.ae2_has_machine_menu(target) && !self.ae2_has_terminal(target) && menu_allowed {
            self.lan_open_ae2_machine(host, conn, target);
            return true;
        }
        // MEChestBlock.useWithoutItem: the top face is the terminal's, the others open the
        // chest's cell screen.
        if menu_allowed
            && face != [0, 1, 0]
            && matches!(self.world.block_entities.get(&target),
                Some(BlockEntity::Ae2(m)) if m.kind == MachineKind::Chest)
        {
            self.lan_open_ae2_machine(host, conn, target);
            return true;
        }
        let terminal_face = match self.world.block_entities.get(&target) {
            Some(BlockEntity::Ae2(m)) if m.kind == MachineKind::CableBus => {
                use horizon_core::nbt::Tag;
                matches!(m.config.get(&"parts".into()), Some(Tag::List(parts)) if parts.iter().any(|part|
                part.get("face") == Some(&Tag::String(side.into()))
                    && matches!(part.get("id"), Some(Tag::String(id))
                        if id.to_string().is_ok_and(|s| matches!(
                            s.as_str(),
                            "ae2:terminal" | "ae2:pattern_encoding_terminal"
                        )))))
            }
            _ => true,
        };
        if self.ae2_has_terminal(target) && terminal_face && menu_allowed {
            self.lan_open_ae2_terminal(host, conn, target);
            return true;
        }
        false
    }

    /// A guest's use of `hand`'s item in the air, by AE2: replicator and meteorite cycles,
    /// clearing a memory card, cells, the quartz knife, portables, the addons', the network
    /// tool and wireless terminals. True when AE2 took it. `rotation` is the packet's, set
    /// before the item looks where the player does.
    pub(super) fn lan_ae2_use_item(
        &mut self,
        conn: u32,
        gi: usize,
        hand: Hand,
        rotation: Option<(f32, f32)>,
    ) -> bool {
        let hand_i = hand_index(hand);
        self.with_lan_host(|g, host| {
            if let Some((true, lines)) =
                g.ae2_guest_captured(host, conn, hand, |g| g.ae2_try_replicator_cycle())
            {
                tell(host, conn, lines, false);
                host.hub.send_inventory(gi);
                return true;
            }
            if let Some((true, lines)) =
                g.ae2_guest_captured(host, conn, hand, |g| g.ae2_try_meteorite_cycle())
            {
                tell(host, conn, lines, false);
                host.hub.send_inventory(gi);
                return true;
            }
            if let Some((true, lines)) =
                g.ae2_guest_captured(host, conn, hand, |g| g.ae2_clear_memory_in_air())
            {
                tell(host, conn, lines, true);
                host.hub.send_inventory(gi);
                return true;
            }
            if let Some((yaw, pitch)) = rotation {
                host.hub
                    .guests
                    .update(gi, |g| (g.yaw, g.pitch) = (yaw, pitch));
            }
            if g.ae2_guest(host, conn, |g| {
                g.with_hand(hand, |g| g.ae2_cell_item_use(true))
            })
            .unwrap_or(false)
            {
                host.hub.send_inventory(gi);
                return true;
            }
            g.lan_open_quartz_knife(host, conn, hand_i)
                || g.lan_open_ae2_portable(host, conn, hand_i)
                || g.lan_ae2x_use_item(host, conn, hand_i)
                || g.lan_open_ae2_network_tool(host, conn, hand_i)
                || g.lan_open_ae2_wireless(host, conn, hand_i)
        })
        .unwrap_or(false)
    }

    /// A click in one of AE2's or its addons' menus. True when one of them took it.
    pub(super) fn lan_ae2_container_click(&mut self, conn: u32, p: &play::ContainerClick) -> bool {
        self.with_lan_host(|g, host| {
            if g.lan_ae2x_click(host, conn, p) {
                return true;
            }
            if g.lan_pattern_access_click(host, conn, p) {
                bus_ack(host, conn, p.container);
                return true;
            }
            if g.lan_craft_confirm_click(host, conn, p)
                || g.lan_ae2_anchor_click(host, conn, p)
                || g.lan_ae2_network_tool_click(host, conn, p)
            {
                return true;
            }
            if g.lan_ae2_bus_click(host, conn, p) {
                bus_ack(host, conn, p.container);
                return true;
            }
            g.lan_ae2_portable_click(host, conn, p)
                || g.lan_ae2_terminal_click(host, conn, p)
                || g.lan_ae2_machine_click(host, conn, p)
        })
        .unwrap_or(false)
    }

    /// A rename packet as AE2's text fields (priority, energy, the searches, a craft amount).
    pub(super) fn lan_ae2_rename(&mut self, conn: u32, name: &str) -> bool {
        self.with_lan_host(|g, host| {
            g.lan_ae2_priority_text(host, conn, name)
                || g.lan_ae2_energy_text(host, conn, name)
                || g.lan_pattern_access_search(host, conn, name)
                || g.lan_craft_confirm_amount(host, conn, name)
                || g.lan_ae2_terminal_search(host, conn, name)
        })
        .unwrap_or(false)
    }

    /// A close of window `window` when it is one of AE2's or its addons' menus.
    pub(super) fn lan_ae2_container_close(&mut self, conn: u32, window: i32) -> bool {
        self.with_lan_host(|g, host| {
            g.lan_pattern_access_closed(host, conn, Some(window))
                || g.lan_ae2x_closed(host, conn, window)
                || g.lan_craft_confirm_closed(host, conn, Some(window))
                || g.lan_ae2_anchor_closed(host, conn, Some(window))
                || g.lan_ae2_network_tool_closed(host, conn, Some(window))
                || g.lan_ae2_bus_closed(host, conn, Some(window))
                || g.lan_ae2_portable_closed(host, conn, Some(window))
                || g.lan_ae2_terminal_closed(host, conn, Some(window))
                || g.lan_ae2_machine_closed(host, conn, Some(window))
        })
        .unwrap_or(false)
    }

    /// A message on one of AE2's or its addons' channels. True when one of them took it.
    pub(super) fn lan_ae2_payload(&mut self, conn: u32, channel: &str, data: &[u8]) -> bool {
        self.with_lan_host(|g, host| {
            match channel {
                crate::game::ae2_hotkeys::CHANNEL => g.lan_ae2_hotkey(host, conn, data),
                _ if g.lan_ae2x_payload(host, conn, channel, data) => {}
                "ae2:mouse_wheel" => g.lan_ae2_item_wheel(host, conn, data),
                "ae2:clear_encoding" => g.lan_clear_encoding(host, conn, data),
                "ae2:jei_transfer" => g.lan_jei_transfer(host, conn, data),
                "ae2:pinned_keys" => g.lan_ae2_pinned_keys(host, conn, data),
                "ae2:terminal_scroll" => g.lan_ae2_terminal_scroll(host, conn, data),
                "ae2:terminal_action" => g.lan_ae2_terminal_action(host, conn, data),
                _ => return false,
            }
            true
        })
        .unwrap_or(false)
    }

    /// `ItemEntity.playerTouch` of the item entities `items` for guest `gi`, from AE2 code
    /// (an addon's magnet).
    pub(in crate::game) fn guest_touch_items(
        &mut self,
        host: &mut HostTurn,
        gi: usize,
        items: Vec<u32>,
    ) {
        host.back(self, |g| g.guest_touch_items_in(gi, items));
    }

    /// `ServerPlayer.doCloseContainer` for a window whose content ends with the player's
    /// inventory and told it (`Guest::inventory_told_by`): `removed` gives the cursor back,
    /// then `InventoryMenu.transferState` takes over what the window told. A client that
    /// closed the window before the answer to its last click arrived dropped that content, so
    /// what it was told is not known: the inventory goes out again, whatever the cursor held,
    /// or a slot the click changed would never be sent.
    pub(in crate::game) fn guest_told_window_closed(&mut self, host: &mut HostTurn, gi: usize) {
        if let Some(c) = host.hub.guests.update(gi, |g| g.carried.take()) {
            host.back(self, |g| g.lan_place_back(gi, vec![c]));
        }
        host.hub.send_inventory(gi);
    }
}

/// AE2's acknowledgement of a bus or pattern access click (`ae2:bus_ack`, the window id).
fn bus_ack(host: &HostTurn, conn: u32, window: i32) {
    host.hub.send_to(
        conn,
        &C::CustomPayload(horizon_protocol::packets::configuration::CustomPayload {
            channel: "ae2:bus_ack".into(),
            data: window.to_be_bytes().to_vec(),
        }),
    );
}
