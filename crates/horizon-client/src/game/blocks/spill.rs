//! Block entities removed by the block behaviour (`LevelChunk.setBlockState`
//! running `BlockEntity.preRemoveSideEffects`) and the resources a destroyed
//! block drops (`Block.dropResources`), drawn from the level random at the
//! moment vanilla draws them. The item entities they make are added to the
//! world as soon as the behaviour returns, within the same tick.
use super::api_values::{TryIntoApi, TryIntoEngine};
use super::{Game, GameLevel, Screen, storage::is_shulker_box};
use horizon_client::{
    block_entity_item, block_entity_nbt,
    containers::{Banner, BlockEntity},
    furnace::FurnaceCooking,
    item_data,
    items::{BlockLootRoll, ItemStack},
    save,
};
use horizon_core::position::BlockPos;
use horizon_core::random::RandomSource;
use mod_api::EntityRef;
use std::collections::HashMap;
use {
    horizon_core::nbt::{Compound, JavaString, Tag},
    horizon_world::{
        level::{LevelEvent, removal, removal::PreRemove},
        registry::BlockStateId,
    },
};

/// Source AEItemKey.addDrops creates natural-size stacks until the shared
/// drop list already contains more than 1000 stacks. Fluids produce no item.
#[cfg(feature = "ae2-lab")]
pub(super) fn append_ae2_key_drops(out:&mut Vec<ItemStack>,prototype:&ItemStack,mut amount:u64,max:&impl Fn(&ItemStack)->u32) {
    let limit=u64::from(max(prototype).max(1));
    while amount>0 && out.len()<=1000 {
        let count=amount.min(limit);out.push(ItemStack{count:count as u32,..prototype.clone()});amount-=count;
    }
    if amount>0 {log::warn!("AE2 source drop limit omitted {amount} of {}",prototype.item);}
}
#[cfg(feature = "ae2-lab")]
fn append_provider_custody(out:&mut Vec<ItemStack>,config:&horizon_core::nbt::Compound,max:&impl Fn(&ItemStack)->u32) {
    if config.contains_key(&horizon_client::ae2_provider_queue::QUEUE.into()) {
        if let Ok(entries)=horizon_client::ae2_provider_queue::read(config) {
            for entry in entries {
                if let horizon_client::ae2_provider_queue::Entry::Item{prototype,amount}=entry {append_ae2_key_drops(out,&prototype,amount,max);}
            }
        }
    } else if let Some(Tag::List(entries))=config.get(&"ae2_provider_send_items".into()) {
        if entries.len()<=8192 {
            for entry in entries {
                if matches!(entry.get("count"),Some(Tag::Int(n)) if *n>0) {
                    if let Some(item)=horizon_client::save::item_from_nbt(entry) {append_ae2_key_drops(out,&item,u64::from(item.count),max);}
                }
            }
        }
    }
    if let Some(Tag::List(entries))=config.get(&"ae2_provider_return_items".into()) {
        for entry in entries.iter().take(9) {
            if matches!(entry.get("count"),Some(Tag::Int(n)) if *n>0) {
                if let Some(item)=horizon_client::save::item_from_nbt(entry) {append_ae2_key_drops(out,&item,u64::from(item.count),max);}
            }
        }
    }
}
/// Physical machine slots and installed parts only. Terminal listings are
/// virtual and never stored in these slots. Cable type is not recorded yet.
#[cfg(feature = "ae2-lab")]
pub(super) fn ae2_physical_drops_with_max(machine:&horizon_client::ae2_world::Machine,max:&impl Fn(&ItemStack)->u32)->Vec<ItemStack> {
    ae2_collect_physical_drops(machine,max,false)
}
/// Wrench removal adds the selected part item and its contents to one list,
/// unlike cable-block removal, whose part loot is collected separately.
#[cfg(feature = "ae2-lab")]
pub(super) fn ae2_physical_part_drops_with_max(machine:&horizon_client::ae2_world::Machine,max:&impl Fn(&ItemStack)->u32)->Vec<ItemStack> {
    ae2_collect_physical_drops(machine,max,true)
}
#[cfg(feature = "ae2-lab")]
fn ae2_collect_physical_drops(machine:&horizon_client::ae2_world::Machine,max:&impl Fn(&ItemStack)->u32,wrenched_part:bool)->Vec<ItemStack> {
    use horizon_client::ae2_world::MachineKind;
    let mut out=machine.physical_slots_and_upgrades();
    let mut additional=Vec::new();
    if matches!(machine.kind,MachineKind::CraftingUnit|MachineKind::CraftingAccelerator|MachineKind::CraftingStorage1k|MachineKind::CraftingStorage4k|MachineKind::CraftingStorage16k|MachineKind::CraftingStorage64k|MachineKind::CraftingStorage256k|MachineKind::CraftingMonitor) {
        let mut counts=std::collections::BTreeMap::<String,(ItemStack,u64)>::new();
        if let Some(Tag::List(entries))=machine.config.get(&"ae2_cpu_escrow".into()) {
            for entry in entries {
                let Some(&Tag::Int(count))=entry.get("count").filter(|n|matches!(n,Tag::Int(n) if *n>0)) else {continue;};
                let Some(mut item)=horizon_client::save::item_from_nbt(entry) else{continue;};
                // AE2's amount, past ItemStack.CODEC's 1..=99 that item_from_nbt applies.
                let amount=count as u64;item.count=1;
                let key=horizon_client::item_data::canon_stack(Some(&item));
                let value=counts.entry(key).or_insert((item,0));value.1=value.1.saturating_add(amount);
            }
        }
        // CraftingBlockEntity uses a fresh drop list for each key.
        for (_, (item,amount)) in counts {
            let mut key_drops=Vec::new();append_ae2_key_drops(&mut key_drops,&item,amount,max);out.extend(key_drops);
        }
    }
    if machine.kind==MachineKind::PatternProvider {append_provider_custody(&mut out,&machine.config,max);}
    // An addon machine's owned keys (`addAdditionalDrops`).
    if let Some(drops)=machine.kind.ext().and_then(|m|m.key_drops) {
        for (item,amount) in drops(machine) {append_ae2_key_drops(&mut out,&item,amount,max);}
    }
    if machine.kind == horizon_client::ae2_world::MachineKind::CableBus {
        let cable = machine
            .config
            .get(&"cable".into())
            .filter(|tag| tag.get("count") == Some(&Tag::Int(1)))
            .and_then(horizon_client::save::item_from_nbt)
            .filter(|stack| {
                if stack.count != 1 {
                    return false;
                }
                let Some(id) = stack.item.strip_prefix("ae2:") else {
                    return false;
                };
                let kinds = [
                    "glass_cable",
                    "covered_cable",
                    "smart_cable",
                    "covered_dense_cable",
                    "smart_dense_cable",
                ];
                kinds.iter().any(|kind| {
                    id.strip_suffix(kind)
                        .and_then(|p| p.strip_suffix('_'))
                        .is_some_and(|color| {
                            matches!(
                                color,
                                "fluix"
                                    | "white"
                                    | "orange"
                                    | "magenta"
                                    | "light_blue"
                                    | "yellow"
                                    | "lime"
                                    | "pink"
                                    | "gray"
                                    | "light_gray"
                                    | "cyan"
                                    | "purple"
                                    | "blue"
                                    | "brown"
                                    | "green"
                                    | "red"
                                    | "black"
                            )
                        })
                })
            });
        if let Some(cable) = cable {
            out.push(cable);
        }
    }
    if let Some(Tag::List(parts)) = machine.config.get(&"parts".into()) {
        // CableBusContainer.addAdditionalDrops iterates Platform.DIRECTIONS_WITH_NULL,
        // not the insertion order used by the native part storage.
        let mut ordered:Vec<_>=parts.iter().collect();
        ordered.sort_by_key(|part|match part.get("face").and_then(|tag|match tag {Tag::String(face)=>face.to_string().ok(),_=>None}).as_deref() {
            Some("down")=>0,Some("up")=>1,Some("north")=>2,Some("south")=>3,Some("west")=>4,Some("east")=>5,_=>6,
        });
        for part in ordered {
            let Some(Tag::String(id)) = part.get("id") else {
                continue;
            };
            let Ok(id) = id.to_string() else {
                continue;
            };
            if !matches!(
                horizon_client::ae2_world::base_id(id.as_str()),
                "ae2:terminal"
                    | "ae2:crafting_terminal"
                    | "ae2:pattern_encoding_terminal"
                    | "ae2:import_bus"
                    | "ae2:export_bus"
                    | "ae2:storage_bus"
                    | "ae2:level_emitter"
                    | "ae2:energy_level_emitter"
                    | "ae2:toggle_bus"
                    | "ae2:inverted_toggle_bus"
                    | "ae2:annihilation_plane"
                    | "ae2:formation_plane"
                    | "ae2:monitor" | "ae2:semi_dark_monitor" | "ae2:dark_monitor"
                    | "ae2:storage_monitor" | "ae2:conversion_monitor"
                    | "ae2:cable_interface" | "ae2:cable_pattern_provider"
                    | "ae2:pattern_access_terminal" | "ae2:cable_energy_acceptor"
                    | "ae2:quartz_fiber" | "ae2:cable_anchor"
                    | "ae2:me_p2p_tunnel" | "ae2:item_p2p_tunnel"
                    | "ae2:fluid_p2p_tunnel" | "ae2:fe_p2p_tunnel"
                    | "ae2:redstone_p2p_tunnel" | "ae2:light_p2p_tunnel"
            ) && horizon_client::ae2x::defs::get().part(id.as_str()).is_none()
            {
                continue;
            }
            let saved = part
                .get("stack")
                .filter(|tag| tag.get("count") == Some(&Tag::Int(1)))
                .and_then(horizon_client::save::item_from_nbt)
                .filter(|stack| stack.count == 1 && stack.item.as_ref() == id);
            let part_item=saved.unwrap_or_else(|| ItemStack {
                item: id.as_str().into(),
                count: 1,
                damage: 0,
                extra: None,
            });
            if wrenched_part {additional.push(part_item)} else {out.push(part_item)}
            // An addon part machine's own slots: the cell of MEGA's Cell Dock
            // (`CellDockPart.addAdditionalDrops`).
            if super::ae2_parts::is_cell_part(id.as_str()) {
                if let Some(machine)=part.get("machine") && let Ok(slots)=super::ae2_parts::strict_part_items(machine.get("slots"),super::ae2_parts::part_slot_limit(id.as_str())) {
                    additional.extend(slots.into_iter().flatten());
                }
                continue;
            }
            // Interface and provider part machines own physical, slot-tagged
            // inventories separate from virtual filters and fluid return data.
            let base=horizon_client::ae2_world::base_id(id.as_str());
            if matches!(base,"ae2:cable_interface"|"ae2:cable_pattern_provider"|"ae2:crafting_terminal"|"ae2:pattern_encoding_terminal") {
                if let Some(machine)=part.get("machine") {
                    let slot_limit=horizon_client::ae2_world::variant(id.as_str()).map_or(if base=="ae2:pattern_encoding_terminal" {2} else {9},|v|v.slots);
                    for (key,limit) in [("slots",slot_limit),("upgrades",if base=="ae2:cable_interface" {1} else {0})] {
                        if let Ok(slots)=super::ae2_parts::strict_part_items(machine.get(key),limit) {
                            additional.extend(slots.into_iter().flatten());
                        }
                    }
                    if base=="ae2:cable_pattern_provider" {
                        if let Some(Tag::Compound(config))=machine.get("config") {append_provider_custody(&mut additional,config,max);}
                    }
                }
            }
            // These cells are physical inventory owned by each terminal face.
            if matches!(id.as_str(), "ae2:terminal" | "ae2:crafting_terminal" | "ae2:pattern_encoding_terminal") {
                let tag = part.get("machine").and_then(|m| m.get("config")).and_then(|c| c.get("ae2_view_cells"));
                if let Ok(cells) = super::ae2_parts::decode_terminal_view_cells(tag) {
                    additional.extend(cells.into_iter().flatten());
                }
            }
            // Bus upgrades are real cards, distinct from state_json filters.
            if !matches!(base,"ae2:cable_interface"|"ae2:cable_pattern_provider"|"ae2:crafting_terminal"|"ae2:pattern_encoding_terminal") && let Some(Tag::List(upgrades)) = part.get("upgrades") {
                if let Ok(slots)=horizon_client::ae2_upgrade_inventory::decode(upgrades,horizon_client::ae2_world::bus_card_slots(id.as_str())) {
                    additional.extend(slots.into_iter().flatten());
                }
            }
        }
    }
    if machine.kind == horizon_client::ae2_world::MachineKind::CableBus {
        let mut faces=std::collections::HashSet::new();
        if let Some(Tag::List(facades))=machine.config.get(&"facades".into()) {
            for facade in facades {
                let Some(Tag::String(face))=facade.get("face") else {continue};
                let Ok(face)=face.to_string() else {continue};
                if !matches!(face.as_str(),"down"|"up"|"north"|"south"|"west"|"east") || faces.contains(&face) {continue;}
                let Some(stack)=facade.get("stack").filter(|n|n.get("count")==Some(&Tag::Int(1))).and_then(horizon_client::save::item_from_nbt) else {continue};
                if stack.count!=1 || stack.item.as_ref()!="ae2:facade" {continue;}
                if horizon_client::item_tooltip::ae2_facade_texture(&stack).is_none() {continue;}
                faces.insert(face);
                out.push(stack);
            }
        }
    }
    out.extend(additional);
    out
}
#[cfg(all(feature="ae2-lab",test))]
fn ae2_physical_drops(machine:&horizon_client::ae2_world::Machine)->Vec<ItemStack> {
    ae2_physical_drops_with_max(machine,&|_|64)
}

/// Cable bus is a multipart host, not an additional physical block item.
#[cfg(feature = "ae2-lab")]
pub(super) fn ae2_synthetic_cable_host_drop(block: &str, stack: &ItemStack) -> bool {
    block == "ae2:cable_bus" && stack.item.as_ref() == "ae2:cable_bus"
}

/// What a broken block's entity gives its own dropped item (`copy_components`
/// in the block loot tables): a shulker box's contents, a banner's layers, a
/// container's custom name.
#[derive(Clone, Default)]
pub(super) struct Pending {
    pub(super) shulker: HashMap<[i32; 3], Vec<(u8, ItemStack)>>,
    pub(super) banner: HashMap<[i32; 3], Banner>,
    /// The block's item and the custom name.
    pub(super) names: HashMap<[i32; 3], (String, Tag)>,
    /// A decorated pot's sherds (back, left, right, front).
    pub(super) pot: HashMap<[i32; 3], [Option<ItemStack>; 4]>,
    /// A head's profile, note block sound and name, or a hive's bees: the
    /// dropped item and the component patch it takes.
    pub(super) copied: HashMap<[i32; 3], (String, Compound)>,
}
impl Pending {
    /// `record` for the block entity at `pos` while it is still in `world`:
    /// its custom name and lock come from there.
    pub(super) fn record_from(
        &mut self,
        world: &horizon_client::world::World,
        pos: [i32; 3],
        entity: &BlockEntity,
        block: &str,
    ) {
        let key = (pos[0], pos[1], pos[2]);
        let name = world.block_names.get(&key).cloned();
        self.record(pos, entity, block, name, world.block_lock(key));
    }
    /// Remember what the block entity at `pos` copies into its own drop.
    /// Name and lock live outside the model: snapshot the actual lock before
    /// removal forgets its source. Only the supported shulker copy policy takes it.
    pub(super) fn record(
        &mut self,
        pos: [i32; 3],
        entity: &BlockEntity,
        block: &str,
        name: Option<Tag>,
        lock: Option<&Tag>,
    ) {
        self.record_breaking(pos, entity, block, name, lock, false);
    }
    /// The same for a block a player's tool is breaking (`Block.playerDestroy`).
    pub(super) fn record_by_player(
        &mut self,
        pos: [i32; 3],
        entity: &BlockEntity,
        block: &str,
        name: Option<Tag>,
        lock: Option<&Tag>,
    ) {
        self.record_breaking(pos, entity, block, name, lock, true);
    }
    fn record_breaking(
        &mut self,
        pos: [i32; 3],
        entity: &BlockEntity,
        block: &str,
        name: Option<Tag>,
        lock: Option<&Tag>,
        by_player: bool,
    ) {
        if let BlockEntity::Banner(banner) = entity {
            self.banner.insert(pos, banner.clone());
            return;
        }
        // What a mod's block entity gives its own item (API 64).
        if let BlockEntity::Mod(m) = entity {
            let mut patch = Compound::new();
            super::Game::native_block_entity_drop(block, m, by_player, &mut patch);
            if !patch.is_empty() {
                self.copied.insert(pos, (block.to_owned(), patch));
            }
        }
        // The vanilla crafter loot table does not copy the block entity's name.
        if let Some(name) = name
            && !matches!(entity, BlockEntity::Crafter(_))
        {
            self.names.insert(pos, (block.to_owned(), name));
        }
        // `copy_components` from the block entity: the head tables copy the
        // profile (player heads), note block sound and name, the hive tables
        // the bees, and shulker tables the actual external lock. This is the
        // supported vanilla class policy, not a selected-table include filter.
        let include: &[&str] = match entity {
            BlockEntity::Chest(_) if is_shulker_box(block) => &["minecraft:lock"],
            BlockEntity::Skull(_) if block.starts_with("minecraft:player_") => &[
                "minecraft:profile",
                "minecraft:note_block_sound",
                "minecraft:custom_name",
            ],
            BlockEntity::Skull(_) => &["minecraft:custom_name"],
            BlockEntity::Beehive(_) => &["minecraft:bees"],
            // The statue loot tables copy the custom name of the golem it was.
            BlockEntity::CopperGolemStatue(_) => &["minecraft:custom_name"],
            _ => &[],
        };
        if !include.is_empty() {
            let mut saved =
                save::block_entity_encode::encode((pos[0], pos[1], pos[2]), entity, block);
            if is_shulker_box(block)
                && let (Some(lock), Tag::Compound(tag)) = (lock, &mut saved)
            {
                tag.insert("lock".into(), lock.clone());
            }
            let mut patch = block_entity_nbt::canonical(&saved)
                .map(|saved| block_entity_item::pick(&saved))
                .unwrap_or_default();
            patch.retain(|k, _| include.contains(&k.to_string_lossy().as_str()));
            if !patch.is_empty() {
                // `Blocks.wallVariant`: a wall head drops the standing item.
                let item = block.replacen("_wall_", "_", 1);
                self.copied.insert(pos, (item, patch));
            }
        }
        if is_shulker_box(block)
            && let BlockEntity::Chest(items) = entity
        {
            let kept = items
                .iter()
                .enumerate()
                .filter_map(|(i, s)| Some((i as u8, s.clone()?)))
                .collect();
            self.shulker.insert(pos, kept);
        }
    }
    /// Copy what was remembered for `pos` into a drop of the block.
    pub(super) fn attach(&mut self, pos: [i32; 3], stack: &mut ItemStack) {
        if is_shulker_box(&stack.item)
            && let Some(contents) = self.shulker.remove(&pos)
            && !contents.is_empty()
        {
            stack.data_mut().container = contents;
        }
        if self
            .names
            .get(&pos)
            .is_some_and(|(item, _)| **item == *stack.item)
            && let Some((_, name)) = self.names.remove(&pos)
        {
            stack.data_mut().custom_name = Some(name);
        }
        if self
            .copied
            .get(&pos)
            .is_some_and(|(item, _)| **item == *stack.item)
            && let Some((_, copied)) = self.copied.remove(&pos)
        {
            let mut patch = match item_data::encode_components(stack) {
                Some(Tag::Compound(c)) => c,
                _ => Default::default(),
            };
            patch.extend(copied);
            (stack.damage, stack.extra) = item_data::decode_components(Some(&Tag::Compound(patch)));
        }
        if &*stack.item == "minecraft:decorated_pot" {
            // The pot's block entity always copies its decorations, empty or not.
            let sides = self.pot.remove(&pos).unwrap_or_default();
            stack.data_mut().pot_decorations = Some(Box::new(sides));
        }
        if stack.item.ends_with("_banner")
            && let Some(Banner {
                layers,
                name,
                components,
            }) = self.banner.remove(&pos)
        {
            let mut patch = match item_data::encode_components(stack) {
                Some(Tag::Compound(c)) => c,
                _ => Default::default(),
            };
            // The remaining components in the banner loot table's copy_components.
            for key in [
                "minecraft:item_name",
                "minecraft:rarity",
                "minecraft:tooltip_display",
            ] {
                if let Some(value) = components.get(&JavaString::from(key)) {
                    patch.insert(key.into(), value.clone());
                }
            }
            patch.insert(
                "minecraft:banner_patterns".into(),
                horizon_client::banner::layers_to_nbt(&layers),
            );
            if let Some(name) = name {
                patch.insert("minecraft:custom_name".into(), name);
            }
            (stack.damage, stack.extra) = item_data::decode_components(Some(&Tag::Compound(patch)));
        }
    }
    /// Remember a decorated pot's sherds from its saved block entity.
    pub(super) fn record_saved(&mut self, pos: [i32; 3], tag: &horizon_core::nbt::Tag) {
        use horizon_core::nbt::Tag;
        if tag.get("id") != Some(&Tag::String("minecraft:decorated_pot".into())) {
            return;
        }
        let sherds = tag.get("sherds");
        let side = |name: &str| match sherds.and_then(|s| s.get(name)) {
            Some(Tag::String(id)) => Some(ItemStack {
                item: std::sync::Arc::from(id.to_string_lossy().as_str()),
                count: 1,
                damage: 0,
                extra: None,
            }),
            Some(t) => horizon_client::save::item_from_nbt(t),
            None => None,
        };
        self.pot.insert(
            pos,
            [side("back"), side("left"), side("right"), side("front")],
        );
    }
    /// The block entity's dynamic drops: a decorated pot's sherds, left,
    /// back, front then right (`DecoratedPotBlock.getDrops`).
    pub(super) fn dynamic(&self, pos: [i32; 3]) -> Vec<ItemStack> {
        self.pot
            .get(&pos)
            .map_or_else(Vec::new, |[back, left, right, front]| {
                [left, back, front, right]
                    .into_iter()
                    .flatten()
                    .cloned()
                    .collect()
            })
    }
    pub(super) fn clear(&mut self, pos: [i32; 3]) {
        self.pot.remove(&pos);
        self.shulker.remove(&pos);
        self.banner.remove(&pos);
        self.names.remove(&pos);
        self.copied.remove(&pos);
    }
}

/// Constructor random retained while drop hooks edit a not-yet-joined item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ItemRandom {
    pub own: (u64, Option<f64>),
    pub uuid: [i32; 4],
    pub bob: f32,
    pub yaw: f32,
    pub velocity: [f64; 3],
    /// The state a drop hook gave the item entity; the default is a fresh one's.
    pub state: DropState,
}

/// Item-entity state a drop hook can set (`ItemDrop`'s `thrower`, `target`, `no_gravity` and
/// `age`). The default is what `new ItemEntity` starts with.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct DropState {
    pub(crate) thrower: Option<u128>,
    pub(crate) target: Option<u128>,
    pub(crate) no_gravity: bool,
    pub(crate) age: i32,
}

impl DropState {
    /// The state `drop` asks for.
    pub(crate) fn of(drop: &mod_api::ItemDrop) -> Self {
        Self {
            thrower: drop.thrower,
            target: drop.target,
            no_gravity: drop.no_gravity,
            age: drop.age,
        }
    }
    /// `drop` with this state.
    pub(crate) fn to_api(self, drop: mod_api::ItemDrop) -> mod_api::ItemDrop {
        drop.with_thrower(self.thrower)
            .with_target(self.target)
            .with_no_gravity(self.no_gravity)
            .with_age(self.age)
    }
    /// `item` as a fresh item entity with this state (`setThrower`, `setTarget`,
    /// `setNoGravity` and its age).
    pub(crate) fn apply(
        self,
        item: horizon_client::entities::ItemEntity,
    ) -> horizon_client::entities::ItemEntity {
        let uuid = horizon_client::lan::playerdata::uuid_ints;
        horizon_client::entities::ItemEntity {
            thrower: self.thrower.map(uuid),
            target: self.target.map(uuid),
            no_gravity: self.no_gravity,
            age: self.age,
            ..item
        }
    }
}

/// The one ItemEntity constructor random sequence, shared by immediate and
/// deferred spawning: UUID, bob offset, yaw, then optional default motion.
pub(super) fn prepare_item_random(velocity: Option<[f64; 3]>) -> ItemRandom {
    let mut own = horizon_client::random_source::entity();
    let uuid = horizon_client::save::insecure_uuid(&mut own);
    let bob = own.next_float() * std::f32::consts::PI * 2.0;
    let yaw = own.next_float() * 360.0;
    let velocity = velocity.unwrap_or_else(|| {
        let x = own.next_double() * 0.2 - 0.1;
        [x, 0.2, own.next_double() * 0.2 - 0.1]
    });
    ItemRandom {
        own: (&own).into(),
        uuid,
        bob,
        yaw,
        velocity,
        state: DropState::default(),
    }
}

/// An item entity to add: where, its motion (None: `new ItemEntity`'s own
/// random motion) and its pickup delay.
#[derive(Clone)]
pub(super) struct Spawn {
    pub(super) random: Option<ItemRandom>,
    pub(super) stack: ItemStack,
    pub(super) pos: [f64; 3],
    pub(super) motion: Option<[f64; 3]>,
    pub(super) delay: u32,
}

/// What a broken block spawns (`GameLevel::block_dropped`).
/// Prepared engine item entity; API snapshots exist only while a listener runs.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BlockItemDrop {
    pub(super) random: Option<ItemRandom>,
    pub(super) stack: ItemStack,
    pub(super) position: [f64; 3],
    pub(super) velocity: [f64; 3],
    pub(super) pickup_delay: u32,
    /// The item-entity state a drop hook set.
    pub(super) state: DropState,
}

impl BlockItemDrop {
    /// The drop as the mods hear and edit it.
    pub(super) fn from_api(drop: &mod_api::ItemDrop, stack: ItemStack) -> Self {
        Self {
            random: None,
            stack,
            position: drop.position,
            velocity: drop.velocity,
            pickup_delay: drop.pickup_delay,
            state: DropState::of(drop),
        }
    }
    /// The constructor random the item entity joins with, carrying the state a hook set. A drop
    /// with no state set keeps its random as it was (none: drawn when it joins).
    pub(super) fn random_with_state(&self) -> Option<ItemRandom> {
        if self.state == DropState::default() {
            return self.random;
        }
        let mut random = self
            .random
            .unwrap_or_else(|| prepare_item_random(Some(self.velocity)));
        random.state = self.state;
        Some(random)
    }
}

pub(super) struct BlockDrops {
    /// The item entities to add, as the mods left them.
    pub(super) items: Vec<BlockItemDrop>,
    /// The experience points to award at the block's centre.
    pub(super) experience: u32,
    /// A mod canceled the break's drops: `spawnAfterBreak` doesn't run.
    pub(super) canceled: bool,
}

#[derive(Clone, Default)]
pub(super) struct Spill {
    pub(super) piston_positions: Vec<super::piston_carry_state::PositionChange>,
    pub(super) spawns: Vec<Spawn>,
    /// Experience orbs (`ExperienceOrb.award`): where and how much, each
    /// and the merge id it drew from the level random.
    pub(super) orbs: Vec<([f64; 3], u32, i32)>,
    /// Block entities the behaviour removed, for the game's own follow-ups
    /// (open screens, playing songs).
    pub(super) removed: Vec<([i32; 3], BlockEntity)>,
    /// Runs of `spawns` a player's use popped (a picked berry bush), for the
    /// mods' `native_loot_drops`.
    pub(super) used: Vec<UsedDrops>,
}

/// `spawns[start..end]` came from `table` at the block `pos` in `state`.
#[derive(Clone)]
pub(super) struct UsedDrops {
    start: usize,
    end: usize,
    pos: [i32; 3],
    state: BlockStateId,
    table: &'static str,
}

impl Spill {
    pub(super) fn is_empty(&self) -> bool {
        self.piston_positions.is_empty()
            && self.spawns.is_empty()
            && self.orbs.is_empty()
            && self.removed.is_empty()
            && self.used.is_empty()
    }
}

/// Lets `say` change each use's run of `spawns`, newest first so the earlier
/// ranges hold. A run's stacks keep their spots, and a stack `say` adds takes
/// the run's last spot, else the block's top.
fn retell_runs(
    spawns: &mut Vec<Spawn>,
    used: &[UsedDrops],
    mut say: impl FnMut(&UsedDrops, [f64; 3], &mut Vec<ItemStack>),
) {
    for used in used.iter().rev() {
        let run: Vec<Spawn> = spawns.drain(used.start..used.end).collect();
        let mut drops: Vec<ItemStack> = run.iter().map(|s| s.stack.clone()).collect();
        let origin = used.pos.map(|c| f64::from(c) + 0.5);
        say(used, origin, &mut drops);
        let top = [origin[0], origin[1] + 0.5, origin[2]];
        let kept: Vec<Spawn> = drops
            .into_iter()
            .enumerate()
            .map(|(i, stack)| Spawn {
                random: run.get(i).and_then(|s| s.random),
                stack,
                pos: run.get(i).or(run.last()).map_or(top, |s| s.pos),
                motion: run.get(i).and_then(|s| s.motion),
                delay: run.get(i).map_or(10, |s| s.delay),
            })
            .collect();
        spawns.splice(used.start..used.start, kept);
    }
}

/// `Block.popResource`: a spot around the block's centre from the level random.
pub(super) fn pop_position(
    random: &mut impl horizon_core::random::RandomSource,
    pos: [i32; 3],
) -> [f64; 3] {
    let mut offset = || -0.25 + random.next_double() * 0.5;
    // EntityTypes.ITEM is 0.25 high.
    let half_height = f64::from(0.25f32) / 2.0;
    [
        f64::from(pos[0]) + 0.5 + offset(),
        f64::from(pos[1]) + 0.5 + offset() - half_height,
        f64::from(pos[2]) + 0.5 + offset(),
    ]
}

/// `Block.popResourceFromFace`'s position and motion: an item on the clicked
/// face, moved off that face and jittered across the other two axes.
pub(super) fn pop_position_from_face(
    random: &mut impl horizon_core::random::RandomSource,
    pos: [i32; 3],
    face: [i32; 3],
) -> ([f64; 3], [f64; 3]) {
    fn jitter<R: horizon_core::random::RandomSource>(random: &mut R, min: f64, max: f64) -> f64 {
        min + random.next_double() * (max - min)
    }
    // EntityTypes.ITEM is 0.25 wide and 0.25 high.
    let (half_width, half_height) = (0.125, 0.125);
    let x = f64::from(pos[0])
        + 0.5
        + if face[0] == 0 {
            jitter(random, -0.25, 0.25)
        } else {
            f64::from(face[0]) * (0.5 + half_width)
        };
    let y = f64::from(pos[1])
        + 0.5
        + if face[1] == 0 {
            jitter(random, -0.25, 0.25)
        } else {
            f64::from(face[1]) * (0.5 + half_height)
        }
        - half_height;
    let z = f64::from(pos[2])
        + 0.5
        + if face[2] == 0 {
            jitter(random, -0.25, 0.25)
        } else {
            f64::from(face[2]) * (0.5 + half_width)
        };
    let dx = if face[0] == 0 {
        jitter(random, -0.1, 0.1)
    } else {
        f64::from(face[0]) * 0.1
    };
    let dy = if face[1] == 0 {
        jitter(random, 0.0, 0.1)
    } else {
        f64::from(face[1]) * 0.1 + 0.1
    };
    let dz = if face[2] == 0 {
        jitter(random, -0.1, 0.1)
    } else {
        f64::from(face[2]) * 0.1
    };
    ([x, y, z], [dx, dy, dz])
}

/// The block entity at `pos` Horizon keeps as it was loaded, if any
/// (campfires, decorated pots, chiseled bookshelves, shelves).
pub(super) fn saved_entity(
    world: &horizon_client::world::World,
    pos: [i32; 3],
) -> Option<horizon_core::nbt::Tag> {
    use horizon_core::nbt::Tag;
    let saved = world
        .chunks
        .get(&(pos[0] >> 4, pos[2] >> 4))?
        .saved
        .as_ref()?;
    saved
        .block_entities
        .iter()
        .find(|t| {
            t.get("x") == Some(&Tag::Int(pos[0]))
                && t.get("y") == Some(&Tag::Int(pos[1]))
                && t.get("z") == Some(&Tag::Int(pos[2]))
        })
        .cloned()
}

/// Keep `tag` as the block entity at its position Horizon does not model,
/// replacing the one there.
pub(super) fn put_saved_entity(
    world: &mut horizon_client::world::World,
    pos: [i32; 3],
    tag: horizon_core::nbt::Tag,
) {
    use horizon_core::nbt::Tag;
    let Some(chunk) = world.chunks.get_mut(&(pos[0] >> 4, pos[2] >> 4)) else {
        return;
    };
    let chunk = std::sync::Arc::make_mut(chunk);
    let saved = std::sync::Arc::make_mut(chunk.saved.get_or_insert_default());
    let at = |t: &Tag| {
        t.get("x") == Some(&Tag::Int(pos[0]))
            && t.get("y") == Some(&Tag::Int(pos[1]))
            && t.get("z") == Some(&Tag::Int(pos[2]))
    };
    match saved.block_entities.iter_mut().find(|t| at(t)) {
        Some(old) => *old = tag,
        None => saved.block_entities.push(tag),
    }
}

impl GameLevel<'_> {
    fn pop_resource(&mut self, pos: [i32; 3], mut stack: ItemStack) {
        self.pending.attach(pos, &mut stack);
        let at = pop_position(&mut self.world.random, pos);
        // `Block.popResource` draws its three offsets, then its private
        // overload reads `GameRules.BLOCK_DROPS` and pops nothing while it is
        // off: the level random advances either way.
        if self.rules.block_drops {
            self.spill.spawns.push(Spawn {
                random: None,
                stack,
                pos: at,
                motion: None,
                delay: 10,
            });
        }
    }

    /// `Block.popResource(level, pos, stack)` with the broken block's copied components.
    /// `Block.popResource(level, pos, new ItemStack(block))`.
    pub(super) fn pop_block_item(&mut self, pos: BlockPos, state: BlockStateId) {
        let item = self.facts.get(state).item.clone();
        if &*item != "minecraft:air" {
            self.pop_resource([pos.x, pos.y, pos.z], ItemStack::new(&item, 1));
        }
    }
    /// An item `atLowerCornerWithOffset(pos, 0.5, 1.01, 0.5).offsetRandomXZ(random,
    /// 0.7)` with the default pickup delay (`ComposterBlock.extractProduce`).
    pub(super) fn pop_above(&mut self, pos: BlockPos, item: std::sync::Arc<str>) {
        let at = removal::jukebox_pop_position(&mut self.world.random, pos);
        self.spill.spawns.push(Spawn {
            random: None,
            stack: ItemStack::new(&item, 1),
            pos: at,
            motion: None,
            delay: 10,
        });
    }
    /// `Block.dropFromBlockInteractLootTable` with `Block.popResource` of
    /// every stack.
    pub(super) fn drop_interact_loot(
        &mut self,
        pos: BlockPos,
        state: BlockStateId,
        table: &'static str,
    ) {
        let drops = self.items.interact_loot(
            table,
            self.registry,
            state,
            &mut self.world.random,
            self.rand_sequences,
            self.seed,
        );
        let start = self.spill.spawns.len();
        for stack in drops {
            if &*stack.item != "minecraft:air" && stack.count > 0 {
                self.pop_resource([pos.x, pos.y, pos.z], stack);
            }
        }
        self.spill.used.push(UsedDrops {
            start,
            end: self.spill.spawns.len(),
            pos: [pos.x, pos.y, pos.z],
            state,
            table,
        });
    }
    /// `Block.dropResources(state, level, pos, blockEntity)`: the block's loot
    /// (from its random sequence, else the level random), with what its block
    /// entity copies into it, popped at once. While a mod's machine breaks
    /// the block (`native_machine_destroy_block`), the loot is rolled with its
    /// `drop_tool` (`dropResources(.., entity, tool)`) and, given `drop_into`,
    /// kept there instead of popped.
    pub(super) fn drop_resources_now(&mut self, pos: BlockPos, state: BlockStateId) {
        // CertusQuartzClusterBlock.getDrops requires an entity in the loot
        // context. This native path calls loot without an actor (e.g. support
        // loss); player mining uses its separate actor-bearing path.
        #[cfg(feature = "ae2-lab")]
        if matches!(self.facts.get(state).name.as_ref(),
            "ae2:small_quartz_bud" | "ae2:medium_quartz_bud" |
            "ae2:large_quartz_bud" | "ae2:quartz_cluster") {return;}
        let key = (pos.x, pos.y, pos.z);
        let at = [pos.x, pos.y, pos.z];
        self.unpack_loot(super::container_loot::LootTarget::Block(key), None);
        if let Some(entity) = self.world.block_entities.get(&key) {
            let entity = entity.clone();
            let block = self.facts.get(state).name.clone();
            self.pending.record_from(self.world, at, &entity, &block);
        } else if let Some(tag) = saved_entity(self.world, [pos.x, pos.y, pos.z]) {
            self.pending.record_saved(at, &tag);
        }
        let dynamic = self.pending.dynamic(at);
        let machine = self
            .drop_tool
            .as_ref()
            .is_some_and(|(target, _)| *target == key);
        let tool = self
            .drop_tool
            .as_ref()
            .filter(|_| machine)
            .and_then(|(_, tool)| tool.clone());
        let roll = horizon_client::items::BlockLootRoll {
            held: tool.as_ref(),
            dynamic: &dynamic,
            breaker: self.drop_breaker_at == Some(pos),
            ..Default::default()
        };
            #[cfg(feature = "ae2-lab")]
            if ae2_synthetic_cable_host_drop(&self.facts.get(state).name, &stack) { continue; }
        let entity = self
            .drop_entity
            .filter(|_| self.drop_breaker_at == Some(pos));
        let drops = self.roll_block_loot(pos, state, entity, roll);
        let dropped = self.block_dropped(pos, state, None, tool.as_ref(), drops, None);
        for item in dropped.items {
            if let Some(kept) = self.drop_into.as_mut().filter(|_| machine) {
                kept.push(item.stack);
            } else {
                self.spill.spawns.push(Spawn {
                    random: item.random_with_state(),
                    stack: item.stack,
                    pos: item.position,
                    motion: Some(item.velocity),
                    delay: item.pickup_delay,
                });
            }
        }
        self.pending.clear(at);
        if dropped.experience > 0 {
            let centre = [
                f64::from(pos.x) + 0.5,
                f64::from(pos.y) + 0.5,
                f64::from(pos.z) + 0.5,
            ];
            self.award_orbs(centre, dropped.experience);
        }
    }
    /// `Block.getDrops(state, level, pos, blockEntity, entity, tool)` for the block still
    /// standing: `drop_resources_now`'s roll, with what the block entity copies into the drops,
    /// drawn from the same random, but nothing popped, no `block_drops` event and no
    /// experience. A scratch record keeps a break in progress at `pos` as it was.
    pub(super) fn block_drops_of(
        &mut self,
        pos: BlockPos,
        state: BlockStateId,
        tool: Option<&ItemStack>,
        entity: Option<EntityRef>,
    ) -> Vec<ItemStack> {
        let key = (pos.x, pos.y, pos.z);
        let at = [pos.x, pos.y, pos.z];
        let mut copied = Pending::default();
        if let Some(block_entity) = self.world.block_entities.get(&key) {
            let name = self.world.block_names.get(&key).cloned();
            copied.record(
                at,
                block_entity,
                &self.facts.get(state).name,
                name,
                self.world.block_lock(key),
            );
        } else if let Some(tag) = saved_entity(self.world, at) {
            copied.record_saved(at, &tag);
        }
        let dynamic = copied.dynamic(at);
        let roll = horizon_client::items::BlockLootRoll {
            held: tool,
            dynamic: &dynamic,
            breaker: entity.is_some(),
            ..Default::default()
        };
        let mut drops = self.roll_block_loot(pos, state, entity, roll);
        drops.retain(|stack| &*stack.item != "minecraft:air" && stack.count > 0);
        for stack in &mut drops {
            copied.attach(at, stack);
        }
        drops
    }
    /// What `Block.dropResources` does with the loot it rolled for the block
    /// `state` at `pos`: `Block.popResource` builds an item entity for each
    /// stack (its position drawn from the level random, its motion from the
    /// item's own), then `spawnAfterBreak` samples the block's experience,
    /// in that order. The mods then see the list (`block_drops`, NeoForge's
    /// `BlockDropsEvent`). Nothing is spawned here: the caller adds the
    /// entities and orbs, and a canceled break leaves both empty. The random
    /// draws happen with or without a listener, in vanilla's order.
    pub(super) fn block_dropped(
        &mut self,
        pos: BlockPos,
        state: BlockStateId,
        breaker: Option<u128>,
        tool: Option<&ItemStack>,
        stacks: Vec<ItemStack>,
        exp_to_drop: Option<u32>,
    ) -> BlockDrops {
        let items = stacks
            .into_iter()
            .filter(|stack| &*stack.item != "minecraft:air" && stack.count > 0)
            .map(|stack| self.block_drop_item([pos.x, pos.y, pos.z], stack))
            .collect();
        // `tryDropExperience` samples before the silk touch effect zeroes the
        // part/vanilla starting amount before the global break_block event; a
        // spawner ignores the tool. An explicit global override is applied as given.
        let name = self.facts.get(state).name.clone();
        let silk = tool.is_some_and(|tool| tool.enchantment_level("minecraft:silk_touch") > 0);
        let experience = exp_to_drop
            .or_else(|| self.part_break_experience(pos, state, breaker, tool))
            .unwrap_or_else(|| {
                removal::break_experience(&mut self.world.random, &name)
                    .filter(|_| !silk || &*name == "minecraft:spawner")
                    .unwrap_or(0)
            });
        let experience = if exp_to_drop.is_none() && silk && &*name != "minecraft:spawner" {
            0
        } else {
            experience
        };
        self.block_drops_ready(
            pos,
            state,
            breaker,
            tool,
            BlockDrops {
                items,
                experience,
                canceled: false,
            },
        )
    }
    /// Build item entities only after vanilla has finished merging stacks.
    pub(super) fn block_drop_item(&mut self, at: [i32; 3], mut stack: ItemStack) -> BlockItemDrop {
        self.pending.attach(at, &mut stack);
        let position = pop_position(&mut self.world.random, at);
        let random = prepare_item_random(None);
        BlockItemDrop {
            random: Some(random),
            stack,
            position,
            velocity: random.velocity,
            pickup_delay: 10,
            state: DropState::default(),
        }
    }
    /// The one event/filter boundary for prepared block item entities and XP.
    /// Explosions supply their merged entities, keeping that caller's zero
    /// initial experience; the event may still award it.
    pub(super) fn block_drops_ready(
        &mut self,
        pos: BlockPos,
        state: BlockStateId,
        breaker: Option<u128>,
        tool: Option<&ItemStack>,
        dropped: BlockDrops,
    ) -> BlockDrops {
        let BlockDrops {
            mut items,
            mut experience,
            mut canceled,
        } = dropped;
        if self.native_authoritative
            && mod_api::__private::listened(mod_api::HookId::BlockDrops)
            && let Ok(mut snapshots) = items.try_into_api()
        {
            let (block_entity, breaker_entity) = self.drop_event_refs(pos, breaker);
            let mut next_experience = experience;
            let mut next_canceled = canceled;
            self.native_block_drops(crate::game::native_hooks::BlockDropsCall {
                dimension: self.dimension,
                pos,
                state,
                breaker,
                breaker_entity,
                block_entity,
                tool,
                drops: &mut snapshots,
                experience: &mut next_experience,
                cancel: &mut next_canceled,
            });
            if let Ok(mut next_items) = snapshots.try_into_engine(self.items) {
                for (next, original) in next_items.iter_mut().zip(&items) {
                    next.random = original.random;
                }
                items = next_items;
                experience = next_experience;
                canceled = next_canceled;
            }
        }
        if canceled || !self.rules.block_drops {
            items.clear();
            experience = 0;
        }
        BlockDrops {
            items,
            experience,
            canceled,
        }
    }
    /// `Block.getDrops(state, level, pos, blockEntity, null, tool)`: the loot
    /// `drop_resources_now` rolls for a machine's `tool`, with what the block
    /// entity copies into it, with nothing broken, popped or awarded
    /// (`native_machine_block_drops`).
    pub(super) fn roll_machine_drops(
        &mut self,
        pos: BlockPos,
        state: BlockStateId,
        tool: Option<&ItemStack>,
    ) -> Vec<ItemStack> {
        let key = (pos.x, pos.y, pos.z);
        let at = [pos.x, pos.y, pos.z];
        if let Some(entity) = self.world.block_entities.get(&key) {
            let entity = entity.clone();
            let name = self.world.block_names.get(&key).cloned();
            let block = self.facts.get(state).name.clone();
            self.pending.record(at, &entity, &block, name, self.world.block_lock(key));
        } else if let Some(tag) = saved_entity(self.world, at) {
            self.pending.record_saved(at, &tag);
        }
        let dynamic = self.pending.dynamic(at);
        let drops = match self.items.block_loot_sequence(self.registry, state) {
            Some(sequence) => {
                let random = self.rand_sequences.get(&sequence, self.seed);
                self.items.block_drops_exploded(
                    self.registry,
                    state,
                    tool,
                    &dynamic,
                    random,
                    None,
                    None,
                    false,
                )
            }
            None => self.items.block_drops_exploded(
                self.registry,
                state,
                tool,
                &dynamic,
                &mut self.world.random,
                None,
                None,
                false,
            ),
        };
        let mut out = Vec::new();
        for mut stack in drops {
            if &*stack.item == "minecraft:air" || stack.count == 0 {
                continue;
            }
            self.pending.attach(at, &mut stack);
            out.push(stack);
        }
        self.pending.clear(at);
        out
    }
    /// `ExperienceOrb.award`: orbs of the largest sizes, each drawing the id
    /// `tryMergeToExisting` compares from the level random.
    pub(super) fn award_orbs(&mut self, at: [f64; 3], mut amount: u32) {
        while amount > 0 {
            let value = removal::orb_value(amount);
            amount -= value;
            let merge_id = self.world.random.next_int_bound(40).unwrap_or(0);
            self.spill.orbs.push((at, value, merge_id));
        }
    }
    /// `BlockEntity.preRemoveSideEffects` and `removeBlockEntity` for the block
    /// entity of `old` at `pos`. A block entity Horizon never created reads as
    /// vanilla's fresh one: an empty container still draws a spot per slot.
    pub(crate) fn remove_entity(
        &mut self,
        pos: BlockPos,
        old: BlockStateId,
        side_effects: bool,
    ) -> u8 {
        let key = (pos.x, pos.y, pos.z);
        self.unpack_loot(super::container_loot::LootTarget::Block(key), None);
        // A replacement is a new block entity, even at the same position.
        // Headless servers cannot rely on renderer visibility to discard this state.
        if &*self.facts.get(old).name == "minecraft:end_gateway" {
            self.positions.gateways.remove(&[pos.x, pos.y, pos.z]);
        }
        // Prefer the live AE2 block entity over an older saved snapshot.
        // Removing the cached tag without side effects prevents double drops.
        #[cfg(feature = "ae2-lab")]
        let live_ae2 = matches!(
            self.world.block_entities.get(&key),
            Some(BlockEntity::Ae2(_))
        );
        #[cfg(not(feature = "ae2-lab"))]
        let live_ae2 = false;
        if live_ae2 {
            self.remove_saved_entity(pos, old, false);
        } else if self.remove_saved_entity(pos, old, side_effects) {
            self.positions.hoppers.remove(&key);
            return 0;
        }
        // Sculk blocks and creaking hearts notice their removal on their own tick.
        if matches!(
            self.world.block_entities.get(&key),
            Some(BlockEntity::Sculk(_) | BlockEntity::CreakingHeart(_))
        ) {
            return 0;
        }
        // A mod's block entity answers its parts in place (its slot, so its
        // attachments, stay until `put`), as vanilla's override runs on `this`.
        // One out for its own tick or menu button, which an empty id stands in
        // for, isn't here to answer: its part isn't asked.
        let mut part_drops = Vec::new();
        if side_effects
            && matches!(
                self.world.block_entities.get(&key),
                Some(BlockEntity::Mod(m)) if !m.id().is_empty()
            )
            && let Some((lease, BlockEntity::Mod(entity))) = self.world.block_entities.take(&key)
        {
            self.api_pre_remove_side_effects(pos, old, &entity, &mut part_drops);
            self.world
                .block_entities
                .put(lease, BlockEntity::Mod(entity));
        }
        #[cfg(feature = "ae2-lab")]
        if !self.client_side && matches!(self.world.block_entities.get(&key),Some(BlockEntity::Ae2(m)) if matches!(m.kind,horizon_client::ae2_world::MachineKind::QuantumRing|horizon_client::ae2_world::MachineKind::QuantumLink)) {
            if let (Some(address),Some(host))=(super::ae2_global::GridAddress::from_local(self.dim,key),self.world.block_entities.incarnation(&key)) {
                self.quantum.removed(address.network_pos(),host);
                self.quantum.flush_events(self.world,self.dim,key);
            }
        }
        let entity = self.world.block_entities.remove(&key);
        self.positions.hoppers.remove(&key);
        self.world.block_names.remove(&key);
        self.world.forget_block_entity_source(key);
        let name = self.facts.get(old).name.clone();
        let kind = if live_ae2 {
            PreRemove::Nothing
        } else {
            match &entity {
                // BaseContainerBlockEntity.preRemoveSideEffects: Containers.dropContents.
                Some(BlockEntity::Mod(m))
                    if !m.items.is_empty()
                        && self.world.block_entities.mod_container(old.0).is_some() =>
                {
                    PreRemove::Contents {
                        slots: m.items.len(),
                        furnace: false,
                    }
                }
                _ => removal::pre_remove(&name),
            }
        };
        let mut neighbour_updates = 0;
        if side_effects {
            #[cfg(feature = "ae2-lab")]
            if let Some(BlockEntity::Ae2(machine)) = &entity {
                let drops=ae2_physical_drops_with_max(machine,&|stack| {
                    let default=self.items.get(&stack.item).map_or(64,|info|info.max_stack).max(1);
                    super::ae2_formation::max_stack(stack,default)
                });
                for stack in drops {self.drop_item_stack(pos,Some(stack));}
            }
            match kind {
                PreRemove::Contents { slots, furnace } => {
                    let items = entity.as_ref().map_or(&[][..], BlockEntity::slots);
                    for slot in 0..slots {
                        let stack = items.get(slot).cloned().flatten();
                        self.drop_item_stack(pos, stack);
                    }
                    // AbstractFurnaceBlockEntity.preRemoveSideEffects: then the
                    // experience of what it made, at the block's centre.
                    if furnace && let Some(BlockEntity::Furnace(f)) = &entity {
                        let at = [
                            f64::from(pos.x) + 0.5,
                            f64::from(pos.y) + 0.5,
                            f64::from(pos.z) + 0.5,
                        ];
                        for (count, experience) in f.recipe_experience(self.recipes) {
                            let xp = horizon_client::containers::create_experience(
                                count,
                                experience,
                                &mut self.world.random,
                            );
                            self.award_orbs(at, xp);
                        }
                    }
                }
                PreRemove::Campfire => {
                    let items = match &entity {
                        Some(BlockEntity::Campfire(c)) => c.items.clone(),
                        _ => Default::default(),
                    };
                    for stack in items {
                        self.drop_item_stack(pos, stack);
                    }
                }
                PreRemove::Jukebox => {
                    // popOutTheItem: removeTheItem stops a playing song (its
                    // events and a neighbour update), the record pops, and
                    // onSongChanged updates the neighbours again.
                    if let Some(BlockEntity::Jukebox(j)) = &entity
                        && let Some(record) = j.record().cloned()
                    {
                        if j.playing {
                            self.jukebox_stopped(pos, old);
                            neighbour_updates += 1;
                        }
                        let at = removal::jukebox_pop_position(&mut self.world.random, pos);
                        self.spill.spawns.push(Spawn {
                            random: None,
                            stack: record,
                            pos: at,
                            motion: None,
                            delay: 10,
                        });
                        neighbour_updates += 1;
                    }
                }
                PreRemove::Lectern => {
                    let props = self.registry.state(old).map(|s| &s.properties);
                    let has_book = props
                        .and_then(|p| p.get("has_book"))
                        .is_some_and(|v| v == "true");
                    let step = match props.and_then(|p| p.get("facing")).map(String::as_str) {
                        Some("north") => [0, -1],
                        Some("south") => [0, 1],
                        Some("west") => [-1, 0],
                        _ => [1, 0],
                    };
                    if has_book
                        && let Some(BlockEntity::Lectern(l)) = &entity
                        && let Some(book) = l.book.clone()
                    {
                        self.spill.spawns.push(Spawn {
                            random: None,
                            stack: book,
                            pos: removal::lectern_book_position(pos, step),
                            motion: None,
                            delay: 10,
                        });
                    }
                }
                PreRemove::ShulkerBox
                | PreRemove::SculkShrieker
                | PreRemove::CreakingHeart
                | PreRemove::Beacon
                | PreRemove::Nothing => {}
            }
            for stack in part_drops {
                self.drop_item_stack(pos, Some(stack));
            }
        }
        // BlockEntity.setRemoved: a jukebox always reports its song stopping,
        // a beacon plays its deactivation.
        match kind {
            PreRemove::Jukebox => self.jukebox_stopped(pos, old),
            PreRemove::Beacon => self.events.push(LevelEvent::Sound {
                pos,
                sound: "minecraft:block.beacon.deactivate".into(),
                volume: 1.0,
                pitch: 1.0,
            }),
            _ => {}
        }
        if let Some(entity) = entity {
            self.spill.removed.push(([pos.x, pos.y, pos.z], entity));
        }
        neighbour_updates
    }
    /// The jukebox's `JUKEBOX_STOP_PLAY` game event and level event 1011.
    fn jukebox_stopped(&mut self, pos: BlockPos, state: BlockStateId) {
        self.events.push(LevelEvent::GameEvent {
            pos,
            event: "minecraft:jukebox_stop_play",
            state: Some(state),
        });
        self.events.push(LevelEvent::LevelEvent {
            id: 1011,
            pos,
            data: 0,
            source: None,
        });
    }
    /// A block entity Horizon keeps as it was loaded (campfires, decorated
    /// pots, chiseled bookshelves, shelves): its items spill as vanilla's do.
    fn remove_saved_entity(
        &mut self,
        pos: BlockPos,
        old: BlockStateId,
        side_effects: bool,
    ) -> bool {
        use horizon_core::nbt::Tag;
        let chunk_key = (pos.x >> 4, pos.z >> 4);
        let at = |t: &Tag| {
            let int = |k: &str| match t.get(k) {
                Some(Tag::Int(v)) => Some(*v),
                _ => None,
            };
            int("x") == Some(pos.x) && int("y") == Some(pos.y) && int("z") == Some(pos.z)
        };
        let Some(chunk) = self.world.chunks.get_mut(&chunk_key) else {
            return false;
        };
        let Some(saved) = chunk.saved.as_ref() else {
            return false;
        };
        let Some(index) = saved.block_entities.iter().position(at) else {
            return false;
        };
        let chunk = std::sync::Arc::make_mut(chunk);
        let Some(saved) = chunk.saved.as_mut() else {
            return false;
        };
        let tag = std::sync::Arc::make_mut(saved).block_entities.remove(index);
        if !side_effects {
            return true;
        }
        #[cfg(feature = "ae2-lab")]
        if let Some((_, BlockEntity::Ae2(machine))) =
            horizon_client::save::decode_block_entity(&tag)
        {
            let drops=ae2_physical_drops_with_max(&machine,&|stack| {
                let default=self.items.get(&stack.item).map_or(64,|info|info.max_stack).max(1);
                super::ae2_formation::max_stack(stack,default)
            });
            for stack in drops {self.drop_item_stack(pos,Some(stack));}
            return true;
        }
        let name = self.facts.get(old).name.clone();
        let items = match removal::pre_remove(&name) {
            PreRemove::Contents { slots, .. } if &*name == "minecraft:decorated_pot" => {
                // Containers.dropContents reads getTheItem, which unpacks the loot table.
                let mut tag = tag;
                super::decorated_pot::unpack_loot(
                    &mut tag,
                    self.loot.as_deref(),
                    &mut self.world.random,
                    self.rand_sequences,
                    self.seed,
                );
                let mut items = vec![None; slots];
                items[0] = tag
                    .get("item")
                    .and_then(horizon_client::save::item_from_nbt);
                items
            }
            PreRemove::Contents { slots, .. } => {
                horizon_client::save::decode_items(tag.get("Items"), slots)
            }
            PreRemove::Campfire => horizon_client::save::decode_items(tag.get("Items"), 4),
            _ => return true,
        };
        for stack in items {
            self.drop_item_stack(pos, stack);
        }
        true
    }
    /// `Containers.dropContents` for a mod block entity that left the world
    /// during its own tick (the block broke itself): what it held spills as
    /// vanilla's `preRemoveSideEffects` would, instead of vanishing with it.
    pub(super) fn drop_lost_contents(&mut self, pos: BlockPos, items: Vec<Option<ItemStack>>) {
        for stack in items {
            self.drop_item_stack(pos, stack);
        }
    }
    /// `Containers.dropItemStack` of one slot (an empty one still draws its spot).
    fn drop_item_stack(&mut self, pos: BlockPos, stack: Option<ItemStack>) {
        let count = stack.as_ref().map_or(0, |s| s.count);
        for piece in removal::drop_item_stack(&mut self.world.random, pos, count) {
            let Some(stack) = &stack else { break };
            self.spill.spawns.push(Spawn {
                random: None,
                stack: ItemStack {
                    count: piece.count,
                    ..stack.clone()
                },
                pos: piece.pos,
                motion: Some(piece.motion),
                delay: 0,
            });
        }
    }
}

impl Game<'_> {
    /// [`GameLevel::roll_block_loot`] for a roll outside a level write:
    /// `entity` is `THIS_ENTITY`'s id. With no listener it builds no level.
    pub(in crate::game) fn roll_block_loot(
        &mut self,
        pos: [i32; 3],
        state: BlockStateId,
        entity: Option<u32>,
        roll: BlockLootRoll<'_>,
    ) -> Vec<ItemStack> {
        if mod_api::__private::listened(mod_api::HookId::EnchantedBlockLoot) {
            let at = BlockPos::new(pos[0], pos[1], pos[2]);
            let entity = self.loot_entity_ref(entity);
            return self.with_level(|_, level, _| level.roll_block_loot(at, state, entity, roll));
        }
        world_items!(self).block_drops(
            self.pack.assets.registry,
            state,
            roll,
            &mut self.world.random,
            Some((&mut self.rand_sequences, self.level.seed)),
        )
    }
    /// The handle of `LootContextParams.THIS_ENTITY`, by its id.
    pub(in crate::game) fn loot_entity_ref(&self, id: Option<u32>) -> Option<EntityRef> {
        let epoch = self.native_lifecycle.epoch()?;
        Some(EntityRef::new(epoch, id.filter(|id| *id != 0)?))
    }
    /// Apply the shared block-drop event's item entities and experience unchanged.
    pub(super) fn spawn_block_drops(&mut self, pos: [i32; 3], dropped: BlockDrops) {
        for item in dropped.items {
            self.add_item_drop(item);
        }
        if dropped.experience > 0 {
            self.spawn_orbs(pos.map(|v| f64::from(v) + 0.5), dropped.experience);
        }
    }
    /// Adds a drop's item entity, with the state a drop hook set.
    pub(super) fn add_item_drop(&mut self, drop: BlockItemDrop) {
        let random = drop.random_with_state();
        let velocity = Some(drop.velocity);
        self.add_item_entity_prepared(
            drop.stack,
            drop.position,
            velocity,
            drop.pickup_delay,
            random,
        );
    }
    /// Add the item entities the block behaviour spawned and follow up on the
    /// block entities it removed.
    pub(super) fn flush_spill(&mut self) {
        if self.spill.is_empty() {
            return;
        }
        let _save_lease = horizon_client::join_seq::visibility::PassLease::enter();
        let mut spill = std::mem::take(&mut self.spill);
        self.finish_piston_position_changes(std::mem::take(&mut spill.piston_positions));
        let used = std::mem::take(&mut spill.used);
        let tool = self.held().cloned();
        let player = Some(self.native_acting_id());
        retell_runs(&mut spill.spawns, &used, |used, origin, drops| {
            let block = Some((used.pos, used.state));
            self.native_use_drops(
                used.table,
                origin,
                block,
                None,
                player,
                tool.as_ref(),
                drops,
            );
        });
        for s in spill.spawns {
            self.add_item_entity_prepared(s.stack, s.pos, s.motion, s.delay, s.random);
        }
        for (at, value, merge_id) in spill.orbs {
            self.award_orb(at, value, merge_id);
        }
        for (pos, entity) in spill.removed {
            let key = (pos[0], pos[1], pos[2]);
            if matches!(entity, BlockEntity::Jukebox(_)) {
                self.jukebox_removed(pos);
            }
            if self.container == Some(key) {
                #[cfg(feature = "ae2-lab")]
                if matches!(entity, BlockEntity::Ae2(_)) {
                    // Return a real cursor item before closing the virtual view.
                    self.close_container();
                    self.screen = Screen::Playing;
                    continue;
                }
                self.container_opener_event(key, false);
                self.container = None;
                self.set_screen(Screen::Playing);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Listens to `block_drops`: takes any break of dirt, adds 2 points to the rest.
    struct DropsMod;
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for DropsMod {
        fn block_drops(
            &self,
            _: &dyn mod_api::LevelHost,
            event: &mut mod_api::BlockDropsEvent<'_>,
        ) -> mod_api::Flow {
            if event.drops.iter().any(|d| {
                mod_api::__private::item_key_id(d.stack.item()).unwrap() == "minecraft:dirt"
            }) {
                return mod_api::Flow::Cancel;
            }
            event.experience += 2;
            mod_api::Flow::Continue
        }
    }

    /// `Block.dropResources` builds its item entities (and samples the experience) before
    /// `BlockDropsEvent`, so the level random ends where it does without a listener; a mod that
    /// cancels gets no entities and no experience but the draws still happened; a mod that
    /// edits the amount changes only the amount.
    #[test]
    #[ignore = "daily"]
    fn block_drops_draws_before_the_event_and_cancel_empties_the_break() {
        use horizon_core::random::LegacyRandom;
        use std::sync::Arc;
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.options.dedicated = Some(Vec::new());
            game.world
                .chunks
                .insert((0, 0), Arc::new(horizon_client::chunk::Chunk::empty(0, 0)));
            let registry = game.pack.assets.registry;
            let dirt = registry.default_state("minecraft:dirt").unwrap();
            let ore = registry.default_state("minecraft:diamond_ore").unwrap();
            let pos = BlockPos::new(1, 64, 2);
            let break_of = |game: &mut Game<'_>, state, item, exp_to_drop| {
                game.world.random =
                    horizon_core::random::LevelRandom::for_level(LegacyRandom::new(7));
                let dropped = game.with_level(|_, level, _| {
                    level.block_dropped(
                        pos,
                        state,
                        None,
                        None,
                        vec![ItemStack::new(item, 1)],
                        exp_to_drop,
                    )
                });
                (dropped, game.world.random.next_int())
            };
            let (plain_ore, ore_after) = break_of(game, ore, "minecraft:diamond", None);
            let (plain_dirt, dirt_after) = break_of(game, dirt, "minecraft:dirt", None);
            let (overridden, _) = break_of(game, ore, "minecraft:diamond", Some(17));
            assert_eq!(overridden.experience, 17);
            assert_eq!(plain_dirt.items.len(), 1);
            let mods: mod_api::Mods =
                vec![mod_api::ModEntry::new("block-drops", Box::new(DropsMod))].into();
            mod_platform::testing::with_mods(mods, || {
                let (heard, after) = break_of(game, ore, "minecraft:diamond", None);
                assert_eq!(after, ore_after);
                assert_eq!(heard.items.len(), 1);
                assert_eq!(heard.items[0].position, plain_ore.items[0].position);
                assert_eq!(heard.experience, plain_ore.experience + 2);
                assert!(!heard.canceled);
                let (overridden, _) = break_of(game, ore, "minecraft:diamond", Some(17));
                assert_eq!(overridden.experience, 19);
                let (taken, after) = break_of(game, dirt, "minecraft:dirt", None);
                assert_eq!(after, dirt_after);
                assert!(taken.canceled && taken.items.is_empty() && taken.experience == 0);
            });
        });
    }

    /// A mod block entity's part answers its removal as vanilla's `preRemoveSideEffects`
    /// override does: found by the entity's type, not its block, reading its attachments in
    /// place, its stacks scattered after the block entity's own; skipped under flag 256, and
    /// drawing nothing when it gives nothing back.
    #[test]
    #[ignore = "daily"]
    fn a_block_entity_part_gives_back_what_it_held_when_removed() {
        use horizon_client::chunk::Chunk;
        use horizon_client::containers::{ModBlockEntity, ModBlockEntityFields};
        use mod_api::Holder;
        use mod_platform::__private::part_tests::{HOARD, HOARD_ASKED, game_thread};
        use std::sync::Arc;
        use std::sync::atomic::Ordering;
        crate::game::native_entity_refs::tests::with_game(|game| {
            let _mods = game_thread();
            game.options.dedicated = Some(Vec::new());
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            let stone = game
                .pack
                .assets
                .registry
                .default_state("minecraft:stone")
                .unwrap();
            // Each case: the entity's type, its hoard, side effects; the gold that comes out.
            let asked = HOARD_ASKED.load(Ordering::Relaxed);
            for (id, hoard, side_effects, gold) in [
                ("doubler:hoard", 5, true, 5),
                ("doubler:hoard", 5, false, 0),
                ("doubler:hoard", 0, true, 0),
                // A different declared type's removal doesn't reach `Hoard`.
                ("doubler:press_entity", 5, true, 0),
            ] {
                let pos = BlockPos::new(1, 64, 2);
                let key = (pos.x, pos.y, pos.z);
                let entity = BlockEntity::Mod(Box::new(ModBlockEntity::new(
                    id.into(),
                    ModBlockEntityFields::default(),
                )));
                game.world.block_entities.insert(key, entity);
                let holder = Holder::BlockEntity([pos.x, pos.y, pos.z]);
                let host: &mut dyn mod_api::Host = game;
                HOARD.set(host, holder, hoard).unwrap();
                let (spawns, random) = game.with_level(|_, level, _| {
                    let mut scatter = level.world.random.clone();
                    level.remove_entity(pos, stone, side_effects);
                    // The game's own draws for what came out, from the random as it was.
                    let count = level.spill.spawns.iter().map(|s| s.stack.count).sum();
                    if count > 0 {
                        removal::drop_item_stack(&mut scatter, pos, count);
                    }
                    let spawns: Vec<_> = level
                        .spill
                        .spawns
                        .iter()
                        .map(|s| (s.stack.item.to_string(), s.stack.count))
                        .collect();
                    (spawns, scatter.next_int() == level.world.random.next_int())
                });
                let total: u32 = spawns.iter().map(|(_, count)| count).sum();
                assert!(
                    spawns
                        .iter()
                        .all(|(item, _)| item == "minecraft:gold_ingot")
                );
                assert_eq!(
                    total, gold,
                    "{id} holding {hoard}, side effects {side_effects}"
                );
                assert!(random, "{id}: the random drew only the scatter");
                assert!(game.world.block_entities.get(&key).is_none());
            }
            // Only the two `doubler:hoard` removals with side effects asked it.
            assert_eq!(HOARD_ASKED.load(Ordering::Relaxed) - asked, 2);
        });
    }

    /// A mod block entity that replaces its own block during its tick is out of the level for
    /// the call, so its removal doesn't reach its part: the part isn't asked, its attachments
    /// go with the block entity, and only its container slots come back out.
    #[test]
    #[ignore = "daily"]
    fn a_block_entity_removing_itself_in_its_tick_is_not_asked() {
        use horizon_client::chunk::Chunk;
        use horizon_client::containers::{ModBlockEntity, ModBlockEntityFields, ModContainer};
        use mod_api::Holder;
        use mod_platform::__private::part_tests::{HOARD, HOARD_ASKED};
        use std::sync::Arc;
        use std::sync::atomic::Ordering;
        let Some(root) = crate::game::native_entity_refs::tests::declared_part_root(
            "game::spill::tests::a_block_entity_removing_itself_in_its_tick_is_not_asked",
        ) else {
            return;
        };
        crate::game::native_entity_refs::tests::with_game_at(root, |game| {
            game.options.dedicated = Some(Vec::new());
            game.world
                .chunks
                .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
            let registry = &game.pack.assets.registry;
            let stone = registry.default_state("minecraft:stone").unwrap();
            let granite = registry.default_state("minecraft:granite").unwrap();
            let pos = BlockPos::new(1, 64, 2);
            let key = (pos.x, pos.y, pos.z);
            game.world.set(pos.x, pos.y, pos.z, stone).unwrap();
            game.world.block_entities.set_mod_containers(
                [(
                    stone.0,
                    Arc::new(ModContainer {
                        slots: 1,
                        ..Default::default()
                    }),
                )]
                .into_iter()
                .collect(),
            );
            // Data `[1]` makes the hoard's tick set its own block to granite.
            let fields = ModBlockEntityFields {
                items: vec![Some(ItemStack::new("minecraft:diamond", 3))],
                data: vec![1],
                ..Default::default()
            };
            game.world.block_entities.insert(
                key,
                BlockEntity::Mod(Box::new(ModBlockEntity::new(
                    "doubler:hoard".into(),
                    fields,
                ))),
            );
            let holder = Holder::BlockEntity([pos.x, pos.y, pos.z]);
            let host: &mut dyn mod_api::Host = game;
            HOARD.set(host, holder, 5).unwrap();
            let asked = HOARD_ASKED.load(Ordering::Relaxed);
            let dropped = game.dropped.len();
            game.tick_mod_block_entity(key);
            assert_eq!(game.world.block(pos.x, pos.y, pos.z), granite);
            assert_eq!(HOARD_ASKED.load(Ordering::Relaxed), asked);
            assert!(game.world.block_entities.attachments(key).is_none());
            let out: Vec<_> = game.dropped[dropped..]
                .iter()
                .map(|d| (d.stack.item.to_string(), d.stack.count))
                .collect();
            assert_eq!(out, [("minecraft:diamond".to_owned(), 3)]);
        });
    }

    #[cfg(feature="ae2-lab")]
    #[test]
    fn ae2_provider_drop_limit_is_shared_across_entries_and_return_inventory() {
        use horizon_client::{ae2_provider_queue::{self,Entry},ae2_world::{Machine,MachineKind}};
        let mut provider=Machine::new(MachineKind::PatternProvider);
        provider.slots[0]=Some(ItemStack::new("ae2:processing_pattern",1));
        let first=ItemStack::new("minecraft:stone",1);let second=ItemStack::new("minecraft:diamond",1);
        ae2_provider_queue::write(&mut provider.config,&[
            Entry::Item{prototype:first.clone(),amount:999*64},Entry::Item{prototype:second.clone(),amount:129},
        ]).unwrap();
        provider.config.insert("ae2_provider_return_items".into(),Tag::List(vec![horizon_client::save::item_to_nbt(&ItemStack::new("minecraft:emerald",1))]));
        let drops=ae2_physical_drops_with_max(&provider,&|_|64);
        assert_eq!(drops.len(),1001);assert_eq!(drops[0].item.as_ref(),"ae2:processing_pattern");
        assert_eq!(drops.iter().filter(|item|item.same_item_same_components(&first)).map(|item|u64::from(item.count)).sum::<u64>(),999*64);
        assert_eq!(drops.iter().filter(|item|item.same_item_same_components(&second)).map(|item|u64::from(item.count)).sum::<u64>(),64);
        assert!(!drops.iter().any(|item|item.item.as_ref()=="minecraft:emerald"));
    }
    #[cfg(feature="ae2-lab")]
    #[test]
    fn cable_break_additional_limit_follows_source_face_order() {
        use horizon_client::{ae2_provider_queue::{self,Entry},ae2_world::{Machine,MachineKind}};
        let make_part=|face:&str,item:&str,amount:u64| {
            let mut config=Default::default();
            ae2_provider_queue::write(&mut config,&[Entry::Item{prototype:ItemStack::new(item,1),amount}]).unwrap();
            Tag::compound([("id",Tag::String("ae2:cable_pattern_provider".into())),
                ("face",Tag::String(face.into())),("machine",Tag::compound([("config",Tag::Compound(config))]))])
        };
        let mut cable=Machine::new(MachineKind::CableBus);
        // Native persistence order is deliberately opposite the Java side order.
        cable.config.insert("parts".into(),Tag::List(vec![make_part("south","minecraft:diamond",64),make_part("north","minecraft:stone",999_999)]));
        let drops=ae2_physical_drops_with_max(&cable,&|_|64);
        assert_eq!(drops.iter().filter(|stack|stack.item.as_ref()=="ae2:cable_pattern_provider").count(),2);
        assert_eq!(drops.iter().filter(|stack|stack.item.as_ref()=="minecraft:stone").count(),1001);
        assert!(!drops.iter().any(|stack|stack.item.as_ref()=="minecraft:diamond"));
    }
    #[cfg(feature="ae2-lab")]
    #[test]
    fn ae2_cpu_drop_limit_uses_a_fresh_list_per_exact_key_and_max_stack() {
        use horizon_client::ae2_world::{Machine,MachineKind};
        let mut cpu=Machine::new(MachineKind::CraftingStorage256k);
        let mut first=ItemStack::new("minecraft:stone",2_002);first.data_mut().custom_name = Some(horizon_client::item_data::text_name("one"));
        let mut second=ItemStack::new("minecraft:stone",2_002);second.data_mut().custom_name = Some(horizon_client::item_data::text_name("two"));
        cpu.config.insert("ae2_cpu_escrow".into(),Tag::List(vec![horizon_client::save::item_to_nbt(&first),horizon_client::save::item_to_nbt(&second)]));
        let drops=ae2_physical_drops_with_max(&cpu,&|_|2);
        assert_eq!(drops.len(),2_002);assert!(drops.iter().all(|item|item.count==2));
        assert_eq!(drops.iter().filter(|item|item.same_item_same_components(&first)).count(),1_001);
        assert_eq!(drops.iter().filter(|item|item.same_item_same_components(&second)).count(),1_001);
    }
    use horizon_client::{block_entity_item, item_data, mobs::raider, save};
    use horizon_core::nbt::Tag;

    #[cfg(feature = "ae2-lab")]
    #[test]
    fn terminal_faces_drop_only_their_physical_view_cells() {
        use horizon_client::ae2_world::{Machine, MachineKind};
        let mut cell = ItemStack::new("ae2:view_cell", 1);
        cell.data_mut().custom_name = Some(horizon_client::item_data::text_name("owned cell"));
        for (id, kind) in [("ae2:terminal", MachineKind::Terminal), ("ae2:crafting_terminal", MachineKind::CraftingTerminal), ("ae2:pattern_encoding_terminal", MachineKind::PatternEncodingTerminal)] {
            let mut terminal = Machine::new(kind);
            super::super::ae2_parts::ae2_set_terminal_view_cells(&mut terminal, &[None, Some(cell.clone()), None, None, None]).unwrap();
            let mut cable = Machine::new(MachineKind::CableBus);
            cable.config.insert("parts".into(), Tag::List(vec![Tag::compound([
                ("id", Tag::String(id.into())),
                ("machine", Tag::compound([("config", Tag::Compound(terminal.config))])),
            ])]));
            assert_eq!(ae2_physical_drops(&cable), vec![ItemStack::new(id, 1), cell.clone()]);
        }
    }
    #[cfg(feature = "ae2-lab")]
    #[test]
    fn ae2_drop_list_preserves_physical_cells_and_parts_after_save() {
        use horizon_client::ae2_world::{Machine, MachineKind};
        let mut drive = Machine::new(MachineKind::Drive);
        let mut cell = ItemStack::new("ae2:item_storage_cell_1k", 1);
        cell.data_mut().set_other(
            "minecraft:custom_data",
            Some(Tag::compound([("payload", Tag::Int(7))])),
        );
        drive.slots[0] = Some(cell.clone());
        let encoded =
            save::encode_block_entity((1, 64, 2), &BlockEntity::Ae2(Box::new(drive)), "ae2:drive");
        let (_, BlockEntity::Ae2(loaded)) = save::decode_block_entity(&encoded).unwrap() else {
            panic!("AE2 saved entity")
        };
        assert_eq!(ae2_physical_drops(&loaded), vec![cell]);
        let mut cable = Machine::new(MachineKind::CableBus);
        let mut physical_cable = ItemStack::new("ae2:light_blue_smart_dense_cable", 1);
        physical_cable.data_mut().custom_name = Some(horizon_client::item_data::text_name("Named cable"));
        cable
            .config
            .insert("cable".into(), save::item_to_nbt(&physical_cable));
        let mut named_terminal = ItemStack::new("ae2:terminal", 1);
        named_terminal.data_mut().custom_name = Some(horizon_client::item_data::text_name("Storage"));
        cable.config.insert(
            "parts".into(),
            Tag::List(vec![
                Tag::compound([
                    ("id", Tag::String("ae2:terminal".into())),
                    ("face", Tag::String("north".into())),
                    ("stack", save::item_to_nbt(&named_terminal)),
                ]),
                Tag::compound([
                    ("id", Tag::String("ae2:import_bus".into())),
                    ("face", Tag::String("south".into())),
                    (
                        "upgrades",
                        Tag::List(vec![save::item_to_nbt(&ItemStack::new(
                            "ae2:speed_card",
                            1,
                        ))]),
                    ),
                ]),
            ]),
        );
        let drops = ae2_physical_drops(&cable);
        assert_eq!(
            drops.iter().map(|s| s.item.as_ref()).collect::<Vec<_>>(),
            vec![
                "ae2:light_blue_smart_dense_cable",
                "ae2:terminal",
                "ae2:import_bus",
                "ae2:speed_card"
            ]
        );
        assert_eq!(drops[0], physical_cable);
        assert_eq!(drops[1], named_terminal);
        let mut facade=ItemStack::new("ae2:facade",1);
        facade.data_mut().custom_name = Some(horizon_client::item_data::text_name("Preserved facade"));
        facade.data_mut().set_other("minecraft:custom_data",Some(Tag::compound([("ae2:facade_item",Tag::String("minecraft:stone".into()))])));
        cable.config.insert("facades".into(),Tag::List(vec![
            Tag::compound([("face",Tag::String("north".into())),("stack",save::item_to_nbt(&facade))]),
            Tag::compound([("face",Tag::String("north".into())),("stack",save::item_to_nbt(&facade))]),
        ]));
        assert_eq!(ae2_physical_drops(&cable).iter().filter(|s|s.item.as_ref()=="ae2:facade").cloned().collect::<Vec<_>>(),vec![facade]);
        let mut source_facade=ItemStack::new("ae2:facade",1);
        source_facade.data_mut().set_other("ae2:facade_item",Some(Tag::String("minecraft:stone".into())));
        cable.config.insert("facades".into(),Tag::List(vec![Tag::compound([("face",Tag::String("south".into())),("stack",save::item_to_nbt(&source_facade))])]));
        assert_eq!(ae2_physical_drops(&cable).iter().filter(|s|s.item.as_ref()=="ae2:facade").cloned().collect::<Vec<_>>(),vec![source_facade]);
        let mut nested=Machine::new(MachineKind::CableBus);
        let physical=ItemStack::new("minecraft:iron_ingot",3);
        let sending=ItemStack::new("minecraft:diamond",2);
        nested.config.insert("parts".into(),Tag::List(vec![Tag::compound([
            ("id",Tag::String("ae2:cable_pattern_provider".into())),
            ("stack",save::item_to_nbt(&ItemStack::new("ae2:cable_pattern_provider",1))),
            ("machine",Tag::compound([("slots",Tag::List(vec![])),("upgrades",Tag::List(vec![])),
                ("config",Tag::compound([("ae2_provider_return_items",Tag::List(vec![save::item_to_nbt(&physical)])),("ae2_provider_send_items",Tag::List(vec![save::item_to_nbt(&sending)])),("ae2_provider_send_fluids",Tag::List(vec![Tag::compound([("fluid",Tag::String("minecraft:water".into()))])]))]))])),
        ])]));
        assert_eq!(ae2_physical_drops(&nested),vec![ItemStack::new("ae2:cable_pattern_provider",1),sending,physical]);
        let mut queued=ItemStack::new("minecraft:stone",1);
        queued.data_mut().set_other("minecraft:custom_data",Some(Tag::compound([("variant",Tag::String("exact".into()))])));
        let mut queue_config=Default::default();
        horizon_client::ae2_provider_queue::write(&mut queue_config,&[horizon_client::ae2_provider_queue::Entry::Item{prototype:queued.clone(),amount:999_999}]).unwrap();
        nested.config.insert("parts".into(),Tag::List(vec![Tag::compound([
            ("id",Tag::String("ae2:cable_pattern_provider".into())),
            ("machine",Tag::compound([("config",Tag::Compound(queue_config))]))
        ])]));
        // Block loot and additional drops use separate source lists, whereas
        // wrenching a part puts its item and additional drops in one list.
        let recovered=ae2_physical_drops(&nested);assert_eq!(recovered.len(),1002);
        assert_eq!(recovered.iter().skip(1).map(|item|u64::from(item.count)).sum::<u64>(),64_064);
        assert!(recovered.iter().skip(1).all(|item|item.same_item_same_components(&queued)));
        let wrench=ae2_physical_part_drops_with_max(&nested,&|_|64);assert_eq!(wrench.len(),1001);
        assert_eq!(wrench.iter().skip(1).map(|item|u64::from(item.count)).sum::<u64>(),64_000);
        let mut encoded=ItemStack::new("ae2:crafting_pattern",1);encoded.data_mut().custom_name = Some(horizon_client::item_data::text_name("physical encoded pattern"));
        let blank=ItemStack::new("ae2:blank_pattern",3);
        let mut enc_host=Machine::new(MachineKind::CableBus);
        let slot=|n,stack:&ItemStack|{let mut tag=save::item_to_nbt(stack);if let Tag::Compound(c)=&mut tag {c.insert("Slot".into(),Tag::Byte(n));}tag};
        enc_host.config.insert("parts".into(),Tag::List(vec![Tag::compound([
            ("id",Tag::String("ae2:pattern_encoding_terminal".into())),
            ("machine",Tag::compound([("slots",Tag::List(vec![slot(0,&blank),slot(1,&encoded)])),("upgrades",Tag::List(vec![])),("config",Tag::Compound(Default::default()))]))
        ])]));
        assert_eq!(ae2_physical_drops(&enc_host),vec![ItemStack::new("ae2:pattern_encoding_terminal",1),blank,encoded]);
        let mut sparse=Machine::new(MachineKind::CableBus);
        let card=ItemStack::new("ae2:speed_card",1);
        let valid=horizon_client::ae2_upgrade_inventory::encode(&[None,Some(card.clone())]).unwrap();
        let install=|host:&mut Machine,cards:Vec<Tag>| {host.config.insert("parts".into(),Tag::List(vec![Tag::compound([("id",Tag::String("ae2:import_bus".into())),("upgrades",Tag::List(cards))])]));};
        install(&mut sparse,valid.clone());
        assert_eq!(ae2_physical_drops(&sparse),vec![ItemStack::new("ae2:import_bus",1),card.clone()]);
        install(&mut sparse,vec![valid[0].clone(),valid[0].clone()]);
        assert_eq!(ae2_physical_drops(&sparse),vec![ItemStack::new("ae2:import_bus",1)]);
        let outside=horizon_client::ae2_upgrade_inventory::encode(&[None,None,None,None,None,Some(card)]).unwrap();
        install(&mut sparse,outside);
        assert_eq!(ae2_physical_drops(&sparse),vec![ItemStack::new("ae2:import_bus",1)]);
        let mut invalid = Machine::new(MachineKind::CableBus);
        invalid.config.insert(
            "cable".into(),
            save::item_to_nbt(&ItemStack::new("minecraft:diamond", 1)),
        );
        assert!(ae2_physical_drops(&invalid).is_empty());
        assert!(ae2_synthetic_cable_host_drop("ae2:cable_bus", &ItemStack::new("ae2:cable_bus",1)));
        assert!(!ae2_synthetic_cable_host_drop("ae2:cable_bus", &physical_cable));
        assert!(!ae2_synthetic_cable_host_drop("ae2:drive", &ItemStack::new("ae2:drive",1)));
        assert!(ae2_physical_drops(&Machine::new(MachineKind::Terminal)).is_empty());
    }

    #[test]
    fn a_used_run_is_retold_in_place() {
        let spawn = |item: &str, x: f64| Spawn {
            random: None,
            stack: ItemStack::new(item, 1),
            pos: [x, 0.0, 0.0],
            motion: None,
            delay: 10,
        };
        let mut spawns = vec![
            spawn("minecraft:stone", 0.0),
            spawn("minecraft:sweet_berries", 1.0),
            spawn("minecraft:sweet_berries", 2.0),
            spawn("minecraft:dirt", 3.0),
        ];
        let used = [UsedDrops {
            start: 1,
            end: 3,
            pos: [4, 64, 4],
            state: BlockStateId(0),
            table: "minecraft:harvest/sweet_berry_bush",
        }];
        retell_runs(&mut spawns, &used, |used, origin, drops| {
            assert_eq!(used.table, "minecraft:harvest/sweet_berry_bush");
            assert_eq!(origin, [4.5, 64.5, 4.5]);
            assert_eq!(drops.len(), 2);
            drops.remove(0);
            drops.push(ItemStack::new("minecraft:diamond", 1));
        });
        let items: Vec<(&str, f64)> = spawns.iter().map(|s| (&*s.stack.item, s.pos[0])).collect();
        assert_eq!(
            items,
            [
                ("minecraft:stone", 0.0),
                ("minecraft:sweet_berries", 1.0),
                ("minecraft:diamond", 2.0),
                ("minecraft:dirt", 3.0),
            ]
        );
    }

    #[test]
    fn ominous_banner_survives_placement_save_and_drop() {
        let original = raider::ominous_banner();
        let Tag::Compound(components) = item_data::encode_components(&original).unwrap() else {
            panic!("banner components");
        };
        for block in ["minecraft:white_banner", "minecraft:white_wall_banner"] {
            let pos = (1, 64, 2);
            let fresh = BlockEntity::for_block(block).unwrap();
            let fresh = save::block_entity_encode::encode(pos, &fresh, block);
            let placed = block_entity_item::place(&fresh, &components, false);
            let (_, placed) = save::decode_block_entity(&placed).unwrap();
            let saved = save::block_entity_encode::encode(pos, &placed, block);
            let (_, loaded) = save::decode_block_entity(&saved).unwrap();
            let BlockEntity::Banner(banner) = &loaded else {
                panic!("placed banner");
            };
            assert_eq!(banner.layers.len(), item_data::OMINOUS_LAYERS.len());
            let mut pending = Pending::default();
            pending.record([pos.0, pos.1, pos.2], &loaded, block, None, None);
            let mut dropped = ItemStack::new("minecraft:white_banner", 1);
            pending.attach([pos.0, pos.1, pos.2], &mut dropped);
            assert!(dropped.data().unwrap().ominous_banner);
            assert!(horizon_client::banner::tooltip_layers(&dropped).is_empty());
            assert_eq!(
                horizon_client::item_tooltip::name_line(&dropped, &|_| None),
                "Ominous Banner"
            );
            assert_eq!(
                horizon_client::item_tooltip::rarity(&dropped, "common"),
                "uncommon"
            );
            assert_eq!(
                item_data::encode_components(&dropped),
                Some(Tag::Compound(components.clone()))
            );
        }
    }
}

#[cfg(test)]
mod crafter_drop_name_tests {
    use super::*;

    #[test]
    #[ignore = "daily"]
    fn actual_crafter_destruction_keeps_source_name_off_the_dropped_item() {
        crate::game::native_entity_refs::tests::with_game(|game| {
            game.build_registries();
            game.set_allow_commands(true);
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(horizon_client::chunk::Chunk::empty(0, 0)),
            );
            game.block_ticks
                .add_empty_chunk(BlockPos::new(0, 0, 0).chunk())
                .unwrap();
            assert!(game.run_command("forceload add 0 0"));
            assert!(game.run_command("fill 0 199 0 15 199 15 stone"));
            assert!(
                game.ui
                    .rules
                    .bool_at(horizon_client::gamerules::vanilla::BLOCK_DROPS)
            );
            let clock = game.world.game_time;
            let mut unexpected = Vec::new();
            // Positive name-copy and unnamed controls precede the assigned case.
            for (x, z, kind, name) in [
                (3, 11, "chest", Some("Box")),
                (11, 11, "shulker_box", Some("Box")),
                (11, 3, "crafter", None),
                (3, 3, "crafter", Some("Rig")),
            ] {
                let extra = name.map_or(String::new(), |n| format!(",CustomName:\"{n}\""));
                assert!(game.run_command(&format!(
                "setblock {x} 200 {z} minecraft:{kind}{{Items:[{{Slot:0b,id:\"minecraft:diamond\",count:3,components:{{\"minecraft:custom_data\":{{h212:1}}}}}}]{extra}}} strict"
            )));
                let key = (x, 200, z);
                let item_id = format!("minecraft:{kind}");
                assert_eq!(
                    &*game.res.facts().get(game.world.block(x, 200, z)).name,
                    item_id
                );
                assert_eq!(
                    &*game.res.facts().get(game.world.block(x, 199, z)).name,
                    "minecraft:stone"
                );
                assert_eq!(
                    game.world.block_names.get(&key),
                    name.map(|n| Tag::String(n.into())).as_ref()
                );
                let entity = game
                    .world
                    .block_entities
                    .get(&key)
                    .expect("actual source block entity");
                assert_eq!(matches!(entity, BlockEntity::Crafter(_)), kind == "crafter");
                let diamond = entity.slots()[0].as_ref().expect("actual loaded inventory");
                assert_eq!((&*diamond.item, diamond.count), ("minecraft:diamond", 3));
                assert_eq!(
                    diamond
                        .data()
                        .and_then(|d| d.other("minecraft:custom_data"))
                        .and_then(|t| t.get("h212")),
                    Some(&Tag::Int(1))
                );
                assert!(game.world.loaded(x, z), "actual destruction-level chunk");
                assert!(game.run_command(&format!(
                "execute if data block {x} 200 {z} {{id:\"minecraft:{kind}\",x:{x},y:200,z:{z}}}"
            )));
                if let Some(n) = name {
                    assert!(game.run_command(&format!(
                        "execute if data block {x} 200 {z} {{CustomName:\"{n}\"}}"
                    )));
                } else {
                    assert!(
                        game.run_command(&format!(
                            "execute unless data block {x} 200 {z} CustomName"
                        ))
                    );
                }
                let start = game.dropped.len();
                assert!(game.run_command(&format!("setblock {x} 200 {z} air destroy")));
                game.flush_spill();
                assert!(game.res.facts().is_air(game.world.block(x, 200, z)));
                assert!(!game.world.block_entities.contains_key(&key));
                assert!(!game.world.block_names.contains_key(&key));
                let emitted = &game.dropped[start..];
                let matching: Vec<_> = emitted
                    .iter()
                    .filter(|d| *d.stack.item == item_id)
                    .collect();
                assert_eq!(matching.len(), 1, "one actual block item: {kind}");
                let drop = matching[0];
                assert_eq!(drop.stack.count, 1);
                assert_eq!((drop.age, drop.pickup_delay), (0, 10));
                let actual = drop.stack.data().and_then(|d| d.custom_name.as_ref());
                if kind == "crafter" {
                    if actual.is_some() {
                        unexpected.push((name, actual.cloned()));
                    }
                } else {
                    assert_eq!(
                        actual,
                        Some(&Tag::String("Box".into())),
                        "positive ordinary name-copy control"
                    );
                }
                if kind == "shulker_box" {
                    let contents = &drop.stack.data().unwrap().container;
                    assert_eq!(contents.len(), 1);
                    let kept = &contents[0].1;
                    assert_eq!((&*kept.item, kept.count), ("minecraft:diamond", 3));
                    assert_eq!(
                        kept.data()
                            .and_then(|d| d.other("minecraft:custom_data"))
                            .and_then(|t| t.get("h212")),
                        Some(&Tag::Int(1))
                    );
                    assert_eq!(emitted.len(), 1);
                } else {
                    let contents: Vec<_> = emitted
                        .iter()
                        .filter(|d| &*d.stack.item == "minecraft:diamond")
                        .collect();
                    assert_eq!(contents.len(), 1);
                    assert_eq!(contents[0].stack.count, 3);
                    assert_eq!(
                        contents[0]
                            .stack
                            .data()
                            .and_then(|d| d.other("minecraft:custom_data"))
                            .and_then(|t| t.get("h212")),
                        Some(&Tag::Int(1))
                    );
                    assert_eq!(emitted.len(), 2);
                }
                assert_eq!(game.world.game_time, clock, "zero observation ticks");
            }
            assert!(
                unexpected.is_empty(),
                "actual vanilla crafter drop retained source name: {unexpected:?}"
            );
        });
    }
}
