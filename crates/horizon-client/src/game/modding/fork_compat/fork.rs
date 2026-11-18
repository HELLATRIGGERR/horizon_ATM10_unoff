//! The fork's own helpers (`mapping.md` verdict `fork`) and the fork's engine glue that
//! upstream's split `native_hooks.rs` no longer holds: bodies as on the fork's main
//! (`s1-rejects/ud/..._native_hooks.rs.fork`, `native_machines.rs`, `native_client_services.rs`,
//! `native_chunk_tickets.rs`, `native_damage.rs`), on upstream's types where they moved.
//! The fork helpers that live in fork files of their own (`native_fe.rs`,
//! `native_fluid_handler.rs`, `native_chemical.rs`, `native_items.rs`, the panel files) stay
//! there.
use super::*;
use horizon_client::particles::ParticleOptions;
use std::fs;
use std::result::Result;


// From the fork's `native_hooks.rs`.
/// A biome's climate at `pos` (`native_biome_climate`), from its climate settings:
/// `Biome.getHeightAdjustedTemperature` with `sea_level` and
/// `getPrecipitationAt`.
pub(in crate::game) fn biome_climate(
    id: &str,
    base_temperature: f32,
    frozen: bool,
    downfall: f32,
    has_precipitation: bool,
    pos: BlockPos,
    sea_level: i32,
) -> NativeBiome {
    use horizon_world::worldgen::biome_temperature::{
        BiomeTemperatureSettings, TemperatureModifier,
    };
    let settings = BiomeTemperatureSettings {
        base_temperature,
        modifier: if frozen {
            TemperatureModifier::Frozen
        } else {
            TemperatureModifier::None
        },
        has_precipitation,
        downfall,
    };
    let temperature = settings.height_adjusted(pos, sea_level);
    let precipitation = if !has_precipitation {
        NativePrecipitation::None
    } else if temperature < 0.15 {
        NativePrecipitation::Snow
    } else {
        NativePrecipitation::Rain
    };
    NativeBiome {
        id: id.to_owned(),
        base_temperature,
        temperature,
        downfall,
        precipitation,
    }
}

// From the fork's `native_hooks.rs`.
/// An item's food for `native_food_table` and `native_food`: the stack's own
/// `food` and `use_remainder` components over the item's (`info`; a `!`
/// component removes it), none when it has no food.
pub(in crate::game) fn food_of(
    info: &horizon_client::items::ItemInfo,
    stack: Option<&ItemStack>,
) -> Option<NativeFood> {
    food_entry(
        &info.id,
        info.food,
        info.can_always_eat,
        info.use_remainder.clone(),
        stack,
    )
}

// From the fork's `native_hooks.rs`.
pub(in crate::game) fn food_entry(
    item: &str,
    food: Option<(u32, f32)>,
    can_always_eat: bool,
    remainder: Option<String>,
    stack: Option<&ItemStack>,
) -> Option<NativeFood> {
    let ((nutrition, saturation), remainder) = match stack {
        Some(stack) => (
            horizon_client::item_sync::food(stack, food)?,
            horizon_client::item_sync::use_remainder(stack, remainder),
        ),
        None => (food?, remainder),
    };
    Some(NativeFood {
        item: item.to_owned(),
        nutrition,
        saturation,
        can_always_eat,
        remainder,
    })
}

// From the fork's `native_hooks.rs`.
/// `native_data_files` over `root`: each namespace folder of `root/data`
/// that has `dir`, walked through links (a content root links its mods'
/// folders).
pub(in crate::game) fn data_files(root: &std::path::Path, dir: &str) -> Vec<String> {
    let plain =
        |p: &str| !p.is_empty() && p != "." && p != ".." && !p.contains('\\') && !p.contains(':');
    if !dir.split('/').all(plain) {
        return Vec::new();
    }
    let data = root.join("data");
    let mut out = Vec::new();
    let Ok(namespaces) = fs::read_dir(&data) else {
        return out;
    };
    for namespace in namespaces.flatten() {
        let Some(ns) = namespace.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let mut folders = Vec::from([(data.join(&ns).join(dir), String::new())]);
        while let Some((folder, rel)) = folders.pop() {
            let Ok(entries) = fs::read_dir(&folder) else {
                continue;
            };
            for entry in entries.flatten() {
                let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                    continue;
                };
                let child = if rel.is_empty() {
                    name.clone()
                } else {
                    [rel.as_str(), "/", name.as_str()].concat()
                };
                let path = entry.path();
                if path.is_dir() {
                    folders.push((path, child));
                } else if name.ends_with(".json") {
                    out.push(["data/", ns.as_str(), "/", dir, "/", child.as_str()].concat());
                }
            }
        }
    }
    out.sort();
    out
}

// From the fork's `native_damage.rs`.
/// A hit on a player as the game's own items see it, before the native mods hear of it (NeoForge
/// fires the addons' `EntityInvulnerabilityCheckEvent` and `LivingIncomingDamageEvent` there):
/// an AE2 addon's worn armor may change the amount or call the hit off (true).
pub(in crate::game) fn worn_items_hurt(hurting: &Hurting<'_>, amount: &mut f32) -> bool {
    let _ = (hurting, &amount);
    #[cfg(feature = "ae2-lab")]
    if let Hurting::Player(entity, source) = hurting {
        return super::ae2x::player_hurt(*entity, source.damage_type, amount);
    }
    false
}

/// From the fork's `native_hooks.rs`.
impl Game<'_> {
    /// The biome at `pos` in the world in place, as `Level.getBiome` picks
    /// it (the noise cell `BiomeManager` zooms to, so a biome's edge is
    /// ragged as in vanilla), and its climate at `pos`: the temperature there
    /// (`Biome.getTemperature`), the downfall and what falls when it rains.
    /// What a solar panel's peak or a heater's air follows. None in a chunk
    /// that isn't loaded. Like the other reads here it reads the world this
    /// game holds. API 419.
    pub(in crate::game) fn native_biome_climate(&self, pos: [i32; 3]) -> Option<NativeBiome> {
        let at = BlockPos::new(pos[0], pos[1], pos[2]);
        let [qx, qy, qz] = self.streaming.biome_zoom.quart_at_block(at);
        let (x, y, z) = (qx << 2, qy << 2, qz << 2);
        let chunk = self.world.chunks.get(&(x >> 4, z >> 4))?;
        let biomes = match &self.world_biomes {
            Some(biomes) => biomes,
            None => self.res.biomes(),
        };
        let info = biomes.get(chunk.biome(x, y, z));
        let sea_level = if self.dim == super::Dim::Nether {
            32
        } else if self.dim == super::Dim::End {
            0
        } else {
            63
        };
        Some(biome_climate(
            &info.name,
            info.temperature,
            info.frozen,
            info.downfall,
            info.precipitation,
            at,
            sea_level,
        ))
    }
    /// `BlockEntity.setChanged` for the mod block entity at `pos` in the level
    /// in place: its chunk is saved next time, without touching the entity.
    /// False when there is none there, or it is out of the world for a call.
    /// API 418.
    pub(in crate::game) fn native_block_entity_changed(&mut self, pos: [i32; 3]) -> bool {
        let key = (pos[0], pos[1], pos[2]);
        match self.world.block_entities.get(&key) {
            Some(BlockEntity::Mod(m)) if !m.id().is_empty() => {
                self.world.modified.insert((key.0 >> 4, key.2 >> 4));
                true
            }
            _ => false,
        }
    }
    /// How many ticks `stack` burns in a furnace, 0 when it is no fuel
    /// (`AbstractFurnaceBlockEntity`'s fuel values, `ItemStack.getBurnTime`).
    /// API 417.
    pub(in crate::game) fn native_burn_time(&self, stack: &ItemStack) -> i32 {
        self.res
            .providers()
            .burn_time(self.res.items(), stack, false)
    }
    /// Closes the open inventory screen (the player's own, or a container's: every window
    /// that draws on the inventory screen) as E or Escape does, items held on the cursor
    /// and in the crafting grid going back as there, so a mod can open its own screen
    /// (`native_mod_screen_open`) from a button it draws on the inventory (FTB Library's
    /// sidebar). False when no inventory screen is open. Client-side. API 432.
    pub(in crate::game) fn native_close_inventory(&mut self) -> bool {
        if self.screen != Screen::Inventory {
            return false;
        }
        self.close_container();
        self.screen = Screen::Playing;
        self.capture(true);
        true
    }
    /// Every mod's own recipe categories, as `native_recipe_categories`
    /// collects them: for a recipe viewer. Client-side. API
    /// 427.
    pub(in crate::game) fn native_collect_recipe_categories(&mut self) -> Vec<NativeRecipeCategory> {
        let mut categories = Vec::new();
        self.native_recipe_categories(&mut categories);
        // The fork's built-in AE2 and its addons (`game/ae2_recipe_viewer.rs`), after the mods':
        // the hook point itself only dispatches.
        self.ae2_recipe_categories(&mut categories);
        categories
    }
    /// Every furnace, blast furnace, smoker and campfire recipe of the world's
    /// tables, in file order: each one's `kind` (`minecraft:smelting`...), its
    /// ingredient's items, its result, cooking time and experience. A mod's
    /// electric furnace smelts with them (Mekanism's Energized Smelter takes
    /// `RecipeType.SMELTING`'s, one in and the result out). The slice stays
    /// the same until the tables change (`/reload`), so a mod can index it once
    /// per slice. Empty outside a world; like `native_recipes_of_type`, the
    /// server's table on the server thread. API 421.
    pub(in crate::game) fn native_cooking_recipes(&self) -> &[Cooking] {
        if self.outside_world() {
            return &[];
        }
        &self.recipes().cooking
    }
    /// The data files of kind `dir` (`mekanism/chemical`) in every namespace of
    /// the game's resources, content mods' included, as NeoForge's
    /// `ResourceManager.listResources(dir, path -> path.endsWith(".json"))`:
    /// `data/<ns>/<dir>/<path>.json`, sub-folders too, sorted, for
    /// `native_data` to read. Only relative paths of plain names; a world's own
    /// data packs aren't in it. API 423.
    pub(in crate::game) fn native_data_files(&self, dir: &str) -> Vec<String> {
        data_files(&self.options.resources, dir)
    }
    /// What the type of the dimension in place says of its sky and air
    /// (`Level.dimensionType()`): sky light (`hasSkyLight`), a ceiling
    /// (`hasCeiling`) and `ultraWarm`. A solar panel needs sky light, a heater
    /// runs hotter where it is ultra-warm. API 419.
    pub(in crate::game) fn native_dimension_type(&self) -> NativeDimensionType {
        let nether = self.dim == super::Dim::Nether;
        NativeDimensionType {
            has_skylight: self.world.shape.sky_light,
            has_ceiling: nether,
            ultra_warm: nether,
        }
    }
    /// Fluid `fluid`'s icon (`minecraft:water`, a content mod's fluid): its
    /// still sprite in its tint, as JEI's `FluidTankRenderer` and AE2's
    /// terminals draw a fluid in a slot, with its top left at `at`, `size`
    /// GUI pixels square. False for an unknown fluid. API 440.
    pub(in crate::game) fn native_fluid_icon(
        &self,
        batch: &mut HudBatch,
        fluid: &str,
        at: [f32; 2],
        size: f32,
    ) -> bool {
        let start = batch.vertices.len();
        if !self.draw_fluid_icon(batch, fluid, at[0], at[1]) {
            return false;
        }
        batch.scale_since(start, at[0], at[1], size / 16.0, size / 16.0);
        true
    }
    /// The fluids a recipe viewer lists, in the registry's order: those whose
    /// default state is a source (JEI's `FluidStackListFactory`), water and
    /// lava, then the content mods' fluids. API 441.
    pub(in crate::game) fn native_fluids(&self) -> Vec<String> {
        let mut fluids = Vec::from(["minecraft:water".to_owned(), "minecraft:lava".to_owned()]);
        fluids.extend(
            horizon_world::mod_fluids::mod_fluids()
                .iter()
                .map(|f| f.id.clone()),
        );
        fluids
    }
    /// The food of `stack` (`ItemStack.getFoodProperties(null)`): its own
    /// `food` and `use_remainder` components over its item's, as
    /// `native_food_table` gives an item's. None for a stack that isn't food
    /// and outside a world. API 436.
    pub(in crate::game) fn native_food(&self, stack: &ItemStack) -> Option<NativeFood> {
        if self.outside_world() {
            return None;
        }
        food_of(self.items().get(&stack.item)?, Some(stack))
    }
    /// The food table: every item with food (vanilla's `minecraft:food`
    /// component, `FoodProperties`) in the item registry's order
    /// (`BuiltInRegistries.ITEM`), each as its default stack has it
    /// (`Item.getDefaultInstance().getFoodProperties(null)`): its nutrition,
    /// saturation, `can_always_eat` and what eating it leaves. A mod builds
    /// what it makes of food from it, as Mekanism's Nutritional Liquifier and
    /// its recipe viewer do (`NutritionalLiquifierIRecipe`). Read only; built
    /// on each call, so a mod keeps what it needs. Empty outside a world.
    /// API 436.
    pub(in crate::game) fn native_food_table(&self) -> Vec<NativeFood> {
        if self.outside_world() {
            return Vec::new();
        }
        let items = self.items();
        items
            .registry_order
            .iter()
            .filter_map(|id| items.get(id))
            .filter_map(|info| food_of(info, None))
            .collect()
    }
    /// Whether the acting player builds without using items up (creative,
    /// `Abilities.instabuild`): an item used on a block stays. API 424.
    pub(in crate::game) fn native_instabuild(&self) -> bool {
        self.creative
    }
    /// Particles in the level in place from a server hook outside a block's
    /// tick (a Mekanism multiblock forming at the end of a level's tick: its
    /// red sparkles), as `native_send_particles` sends them from inside one
    /// (`ServerLevel.sendParticles`): a packet to each player whose block is
    /// within 32 blocks of `at`, and this window draws its own copy when its
    /// player is that near. A `count` of 0 is one particle at `at` moving by
    /// `spread` times `speed`; more are spread around `at` by up to `spread`.
    /// False for values that aren't finite or a negative count. API 426.
    pub(in crate::game) fn native_level_particles(
        &mut self,
        options: &ParticleOptions,
        at: [f64; 3],
        spread: [f64; 3],
        speed: f64,
        count: i32,
    ) -> bool {
        let finite = at.iter().chain(spread.iter()).all(|v| v.is_finite()) && speed.is_finite();
        if !finite || count < 0 {
            return false;
        }
        let dimension = self.native_dimension();
        self.lan_send_particles(super::lan_particles::QueuedParticle {
            dimension,
            options: options.clone(),
            state: None,
            at,
            spread,
            speed: [speed; 3],
            count,
            randomization: 0,
        });
        let Some(p) = self.native_player() else {
            return true;
        };
        let near = (at[0] - p.x).powi(2) + (at[1] - p.y).powi(2) + (at[2] - p.z).powi(2) <= 1024.0;
        if !self.native_window() || !near {
            return true;
        }
        if count == 0 {
            let velocity = [spread[0] * speed, spread[1] * speed, spread[2] * speed];
            self.particles.add(options, at, velocity);
            return true;
        }
        let mut seed =
            at[0].to_bits() ^ at[1].to_bits().rotate_left(21) ^ at[2].to_bits().rotate_left(42) | 1;
        for _ in 0..count.min(256) {
            let mut offset = [0.0; 3];
            for (o, d) in offset.iter_mut().zip(spread) {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                *o = d * ((seed >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0);
            }
            let here = [at[0] + offset[0], at[1] + offset[1], at[2] + offset[2]];
            self.particles.add(options, here, [0.0; 3]);
        }
        true
    }
    /// `Block.popResource(level, pos, stack)` in the level in place: `stack`
    /// as an item entity at a random point of the block at `pos`, as a broken
    /// block's drops come (none while `doTileDrops` is off). API 425.
    pub(in crate::game) fn native_pop_resource(&mut self, pos: [i32; 3], stack: ItemStack) {
        if stack.count > 0 {
            self.pop_resource(pos, stack);
        }
    }
    /// A number that changes each time a mod says its recipe categories
    /// changed (`native_recipe_categories_changed`). Client-side. API
    /// 427.
    pub(in crate::game) fn native_recipe_categories_version(&self) -> u64 {
        self.recipe_tables
            .categories_version
            .load(std::sync::atomic::Ordering::Relaxed)
    }
    /// A mod's recipe categories changed: recipe viewers see
    /// `native_recipe_categories_version` move and collect them again.
    /// Client-side. API 427.
    pub(in crate::game) fn native_recipe_categories_changed(&self) {
        self.recipe_tables
            .categories_version
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    /// Asks the recipe viewer to show `stack`: its recipes (`Recipes`), its
    /// uses (`Uses`), or to act on a key that went down over it in a screen
    /// the mod draws (`Key`, so its recipe, uses and bookmark keys work there
    /// as over its own list). The viewer hears it at the start of the next
    /// frame (`native_recipe_lookup`), so a screen hook may ask; with no
    /// viewer installed nothing happens. Client-side. API 429.
    pub(in crate::game) fn native_show_recipes(&self, stack: NativeStack, lookup: NativeRecipeLookup) {
        let mut lookups = self
            .recipe_tables
            .lookups
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // A burst of asks (a held key) keeps the newest few.
        if lookups.len() >= 16 {
            lookups.remove(0);
        }
        lookups.push((stack, lookup));
    }
    /// `native_sprite` in a tint (`0xAARRGGBB`): a mod's ingredient drawn
    /// as its sprite in its colour, as Mekanism's `ChemicalStackRenderer`
    /// draws a chemical. False when there is no such sprite. API 440.
    pub(in crate::game) fn native_sprite_tinted(
        &self,
        batch: &mut HudBatch,
        name: &str,
        src: [f32; 4],
        dst: [f32; 4],
        argb: u32,
    ) -> bool {
        let Some(mut r) = self.gui.atlas.sprites.get(name).copied() else {
            return false;
        };
        let (du, dv) = ((r.u1 - r.u0) / r.w as f32, (r.v1 - r.v0) / r.h as f32);
        r.u0 += src[0] * du;
        r.v0 += src[1] * dv;
        r.u1 = r.u0 + src[2] * du;
        r.v1 = r.v0 + src[3] * dv;
        let [a, red, g, b] = argb.to_be_bytes();
        batch.rect_uv(dst[0], dst[1], dst[2], dst[3], r, rgba(red, g, b, a));
        true
    }
    /// The stack in the hand the acting player uses (`getItemInHand(hand)`)
    /// where `native_use_item_on` and `native_use_item` run: shrink it to use
    /// the item up, `None` empties the hand. API 424.
    pub(in crate::game) fn native_used_stack(&mut self) -> &mut Option<ItemStack> {
        self.held_mut()
    }
    /// Whether anything hears entities joining and leaving: a trait mod, or a
    /// mixin bundle that says so (`native_entity_level_listened`).
    pub(in crate::game) fn entity_level_heard(&self) -> bool {
        if !mod_api::__private::mods().is_empty() {
            return true;
        }
        let mut listened = false;
        self.native_entity_level_listened(&mut listened);
        listened
    }
    /// `native_block_entity_unload` and `native_block_entity_load` for the mod
    /// block entities that came and went since the last block entity pass.
    pub(in crate::game) fn tell_mod_block_entity_changes(&mut self) {
        let (added, removed) = self.world.block_entities.take_mod_lifecycle();
        if added.is_empty() && removed.is_empty() {
            return;
        }
        let dimension = self.dim.id();
        for (pos, entity) in removed {
            self.native_block_entity_unload(dimension, [pos.0, pos.1, pos.2], &entity, true);
        }
        for pos in added {
            let Some(BlockEntity::Mod(m)) = self.world.block_entities.get_mut(&pos) else {
                continue;
            };
            if m.id().is_empty() {
                continue;
            }
            let mut taken = std::mem::take(&mut **m);
            let before = taken.clone();
            self.native_block_entity_load(dimension, [pos.0, pos.1, pos.2], &mut taken);
            if taken == before {
                // Nothing to save for being looked at.
                if let Some(BlockEntity::Mod(m)) = self.world.block_entities.get_mut(&pos)
                    && m.id().is_empty()
                {
                    **m = taken;
                }
            } else {
                self.put_back_mod_block_entity(pos, taken, &before.items);
            }
        }
    }
    /// `native_block_entity_unload` of the mod block entities in the chunks
    /// `gone`, just before the chunks' block entities are dropped.
    pub(in crate::game) fn tell_mod_block_entities_unloaded(
        &mut self,
        gone: &horizon_client::fast_hash::FxHashSet<(i32, i32)>,
    ) {
        let mut leaving: Vec<Pos> = Vec::new();
        for (p, e) in self.world.block_entities.iter() {
            if let BlockEntity::Mod(m) = e
                && !m.id().is_empty()
                && gone.contains(&(p.0 >> 4, p.2 >> 4))
            {
                leaving.push(*p);
            }
        }
        let dimension = self.dim.id();
        for pos in leaving {
            let Some(BlockEntity::Mod(m)) = self.world.block_entities.get_mut(&pos) else {
                continue;
            };
            let entity = std::mem::take(&mut **m);
            self.native_block_entity_unload(dimension, [pos.0, pos.1, pos.2], &entity, false);
        }
    }
    /// `native_block_entity_save` of the mod block entities in a save copy.
    pub(in crate::game) fn tell_mod_block_entities_saved(&self, chunk: &mut horizon_client::chunk::Chunk) {
        let dimension = self.dim.id();
        for (p, e) in &mut chunk.block_entities {
            if let BlockEntity::Mod(m) = e
                && !m.id().is_empty()
            {
                let id = m.id().to_owned();
                let fields: &mut horizon_client::containers::ModBlockEntityFields = &mut **m;
                self.native_block_entity_save(dimension, [p.0, p.1, p.2], &id, &mut fields.nbt);
            }
        }
    }
}

/// From the fork's `native_machines.rs`.
impl Game<'_> {
    /// `Level.clip` from `from` to `to` with `ClipContext.Block.OUTLINE` and
    /// `Fluid.NONE` in the level in place, as a machine's beam finds what it
    /// meets (Mekanism's lasers): the first block whose outline shape the
    /// segment enters, the face it enters through (a unit normal) and the
    /// point it enters at. Fluids and air let it through. None when it meets
    /// no block, for a segment of no length, and off a server.
    /// API @API:machine_beams@.
    pub(in crate::game) fn native_clip_blocks(
        &self,
        from: [f64; 3],
        to: [f64; 3],
    ) -> Option<([i32; 3], [i32; 3], [f64; 3])> {
        if !self.native_authoritative() {
            return None;
        }
        let delta = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
        let length = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
        if !length.is_finite() || length <= 0.0 {
            return None;
        }
        let dir = delta.map(|d| d / length);
        self.clip_ray_fluids(from, dir, length, false)
            .map(|hit| (hit.pos, hit.face, hit.point))
    }
    /// `BlockState.getDestroySpeed`: the hardness of the block at `pos` in
    /// the level in place (stone 1.5, obsidian 50, air 0), negative for an
    /// unbreakable one (bedrock -1). None for an unloaded block and off a
    /// server. API @API:machine_beams@.
    pub(in crate::game) fn native_destroy_speed(&self, pos: [i32; 3]) -> Option<f32> {
        if !self.native_authoritative() || !self.world.loaded(pos[0], pos[2]) {
            return None;
        }
        let state = self.world.block(pos[0], pos[1], pos[2]);
        Some(self.res.facts().get(state).hardness)
    }
    /// The drops `native_machine_destroy_block(pos, tool, true)` gives,
    /// rolled without breaking anything: `Block.getDrops(state, level, pos,
    /// blockEntity, null, tool)`, as a machine that must see where they go
    /// before it breaks the block (Mekanism's Digital Miner) rolls them. The
    /// block entity's contents go into them and the mods' say on drops
    /// (`native_loot_drops`) applies; nothing pops and no experience comes.
    /// None for air, an unloaded block and off a server.
    /// API @API:machine_breaking@.
    pub(in crate::game) fn native_machine_block_drops(
        &mut self,
        pos: [i32; 3],
        tool: Option<&ItemStack>,
    ) -> Option<Vec<ItemStack>> {
        if !self.native_authoritative() || !self.world.loaded(pos[0], pos[2]) {
            return None;
        }
        let state = self.world.block(pos[0], pos[1], pos[2]);
        if self.res.behaviours().is_air(state) {
            return None;
        }
        let at = BlockPos::new(pos[0], pos[1], pos[2]);
        let mut drops = self.with_level(|_, level, _| level.roll_machine_drops(at, state, tool));
        self.native_block_drops(pos, state, None, tool, None, &mut drops);
        Some(drops)
    }
}

/// From the fork's `native_client_services.rs`.
impl Game<'_> {
    /// Loaded chunk `chunk`'s 16x16 map picture in the dimension the player is in, as the
    /// minimap extra draws the surface (`horizon-client-render/src/minimap.rs`: each column's
    /// `MapColor` with the map item's height shading and water depth): 1024 bytes of RGBA, rows
    /// north to south. From the minimap's tile cache, or scanned now, about a chunk section's
    /// mesh in cost, so a mod's map asks for a few chunks a frame and keeps what it got (FTB
    /// Chunks keeps regions, JourneyMap tiles). None for a chunk not loaded, with no player, and
    /// off the window's client. Upload the picture with `native_mod_image`. API 428.
    pub(in crate::game) fn native_map_tile(&mut self, chunk: [i32; 2]) -> Option<[u8; 1024]> {
        if self.player.is_none() || !self.native_window() {
            return None;
        }
        let facts = self.res.facts();
        self.extras
            .minimap
            .chunk_image(&self.world, facts, (chunk[0], chunk[1]))
    }
    /// Set this client's action bar, with the normal timer; no server command or packet.
    pub(in crate::game) fn native_client_action_bar(&mut self, text: Text, animate: bool) {
        if self.native_window() {
            self.titles.set_overlay(text, animate);
        }
    }
}

/// From the fork's `native_chunk_tickets.rs`, on upstream's mod tickets: a fork owner string
/// (`create:train/4f0c...`, `ftbchunks:<team>`) holds a ticket under [`FORK_TICKET_SLOT`], a
/// controller slot no mod registers, so the ticket is never saved (`Save::save_mod_tickets`
/// skips a slot without an id) nor validated, as the fork's were not.
pub(in crate::game) const FORK_TICKET_SLOT: u32 = u32::MAX;

/// The ticket owner of the fork's owner string `owner`: a 128-bit hash of it, as an entity owner.
fn fork_ticket_owner(owner: &str) -> mod_api::TicketOwner {
    use std::hash::{Hash, Hasher};
    let half = |seed: u8| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        seed.hash(&mut h);
        owner.hash(&mut h);
        h.finish()
    };
    mod_api::TicketOwner::Entity((u128::from(half(0)) << 64) | u128::from(half(1)))
}

impl Game<'_> {
    /// Hold or release `owner`'s ticket on `chunk` of `dim`'s level. True when the level's
    /// tickets changed; false for an owner without a namespace, a chunk past the world's edge,
    /// off a server, and for a level that is not open (the fork opened it at the next tick;
    /// upstream's tickets live in an open level's streaming).
    pub(in crate::game) fn native_chunk_ticket(
        &mut self,
        dim: Dim,
        owner: &str,
        chunk: [i32; 2],
        hold: bool,
    ) -> bool {
        let namespaced = owner
            .split_once(':')
            .is_some_and(|(ns, path)| !ns.is_empty() && !path.is_empty());
        // `/forceload`'s world: blocks -30,000,000 to 29,999,999, so a mod's
        // corrupt saved list asks for nothing past the edge (where the ring's
        // arithmetic would overflow).
        let in_world = chunk.iter().all(|c| (-1_875_000..1_875_000).contains(c));
        if !namespaced || !self.native_authoritative() || (hold && !in_world) {
            return false;
        }
        let ticket = mod_api::world::mod_chunk_tickets::ModTicket {
            controller: FORK_TICKET_SLOT,
            owner: fork_ticket_owner(owner),
            chunk: (chunk[0], chunk[1]),
            natural_spawning: false,
        };
        let streaming = if dim == self.dim {
            Some(&mut self.streaming)
        } else {
            self.levels.parked_streaming_of_mut(dim.id())
        };
        streaming.is_some_and(|streaming| streaming.set_mod_forced(ticket, hold))
    }

    /// `native_force_chunk`: `owner`'s ticket on `chunk` of the level in place (API 412).
    pub(in crate::game) fn native_force_chunk(&mut self, owner: &str, chunk: [i32; 2], hold: bool) -> bool {
        self.native_chunk_ticket(self.dim, owner, chunk, hold)
    }

    /// `native_force_chunk_in`: `owner`'s ticket on `chunk` of the level `dimension` (API 413);
    /// false for a dimension that isn't registered.
    pub(in crate::game) fn native_force_chunk_in(
        &mut self,
        owner: &str,
        dimension: &str,
        chunk: [i32; 2],
        hold: bool,
    ) -> bool {
        if !Dim::exists(dimension) {
            return false;
        }
        self.native_chunk_ticket(Dim::from_id(dimension), owner, chunk, hold)
    }
}
