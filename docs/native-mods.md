# Native mods

Native mod docs live in the modding book: [native mods](modding/src/how-to/native-mods.md), [trait mods, Parts and attachments](modding/src/concepts/parts.md), [injections](modding/src/concepts/injections.md) and the [hook notes](https://github.com/horizon-rust/horizon/tree/main/crates/horizon-mod-vocab/src/hooks).

## Living death version 2

`Hooks::living_death` receives a mutable `LivingDeathEvent` on the server only.
Its fields are `entity`, `kind`, `source: &HurtSource`,
`weapon: Option<&ItemStack>` and `cancel`. This is a clean contract break:
the old top-level `damage_type` and `attacker` fields and callback signature
are removed, with no compatibility callback.

The source is the accepted killing blow seen by `living_incoming_damage`:
damage type, real causing entity ID (including players), direct entity ID
(including arrows), and original optional source position. The optional weapon
is that blow's complete stack, including damage and component overrides, not
a later read of the attacker's hand.
`/damage` keeps the selected registered trident type rather than relabeling it
as thrown damage. Its explicit `by` (direct) and `from` (causing) entities remain
independent, including a locally bound causing player. A damage type alone does
not manufacture an arrow-family direct entity.

Cancellation suppresses death loot, experience and death messages without
writing health. A listener that sets health to 5 leaves the entity alive at 5;
an unhealed body at health <= 0 is removed after 20 ticks without redispatch
or loot. Cancellation alone does not heal or force health to 1.
The cancelled continuation belongs to the living body: full-body teleports preserve
its original removal deadline, rather than restarting it or retrying death.
A later positive `set_health` revives a cancelled dying body, clears that deadline
and permits a subsequent genuine death; transaction abort restores the prior
health and continuation together. Saved mobs retain the optional Horizon-owned
`horizon:death_canceled` bit alongside their real `DeathTime`; ordinary loading
resumes a saved dying body without running its death callback or loot again.

### Portable contract regression

`crates/horizon-mod-wasm/tests/fixture/src/lending.rs` supplies the same
`Lender::living_death` listener and `check_death` assertions to the native
`collect_positions_copy_in_and_out_natively_as_in_the_real_guest` test and
the compiled-guest `real_member` test. They dispatch a player/arrow source
with distinct causing/direct IDs and a nonintegral position, and check the
bow's identity, damage and complete custom-component patch, both optional
weapon branches, cancellation, and an unchanged borrowed weapon. Their scope is
the portable record boundary, not Game death timing or loot behavior.

Build the matching real guest and standard module, then run both selectors:

```sh
cargo build -p cargo-horizon-mod
# From crates/horizon-mod-wasm/tests/fixture, with ROOT set to the repository:
"$ROOT/target/debug/cargo-horizon-mod" horizon-mod build --wasm --catalog points.json
"$ROOT/target/debug/cargo-horizon-mod" horizon-mod wasm-std "$ROOT/target/horizon-wasm-std.wasm" --catalog points.json
# From the repository root:
cargo test -p horizon-mod-wasm --features listener-fixture --test native_listener collect_positions_copy_in_and_out_natively_as_in_the_real_guest -- --exact
HORIZON_WASM_FIXTURE="$PWD/crates/horizon-mod-wasm/tests/fixture/target/horizon-mod/wasm_fixture.hmod" HORIZON_WASM_STD="$PWD/target/horizon-wasm-std.wasm" cargo test -p horizon-mod-wasm --features listener-fixture --test real_member real_member -- --ignored --exact
```

### Victim ownership and hot path

One immutable accepted-hit record follows the mob or player body; there is no
separate death-weapon table. Mob reconstruction moves it through the existing
non-Clone vehicle transfer token, along with the processed death attempt; the
real `DeathTime` NBT codec preserves the original removal deadline. A full player
teleport restore moves the record through the existing body-replacement boundary,
and respawn discards it.
Each body stores an optional shared pointer; snapshots share the immutable
record rather than copying its source name and stack. Records are captured only
for listened fatal deaths/loot or accepted projectiles needed by `loot_drops`.
Without those listeners, no accepted record or saved weapon is allocated.
Armor's existing deferred event carries the actual cause, direct entity and
position as scalars; synchronous equipment wear borrows the paused hit's name.
The attack scope retains a projectile's weapon only when incoming damage,
death or loot listeners need it. No idle-tick sweep or second source map is added.

Prepared-resource Game regressions live in
`game::native_hooks::death_source_tests`; the player cases additionally require
loopback sockets. `game::lan_actions::guest_body::tests::accepted_hit_moves_through_teleport_restore_but_not_respawn`
covers the ownership transfer. These are source regressions, not a record of a
completed run.

The clean-room contract references are the public
[NeoForge 26.3 LivingDeathEvent Javadoc](https://aldak0.ru/javadoc/26.3.x/net/neoforged/neoforge/event/entity/living/LivingDeathEvent.html)
and [ArmorHurtEvent Javadoc](https://aldak0.ru/javadoc/26.3.x/net/neoforged/neoforge/event/entity/living/ArmorHurtEvent.html),
plus the pinned game's `LivingEntity.die`, `LivingEntity.tickDeath` and
`DamageSources` factories. NeoForge's LGPL source and patches were not read.

## Prebuilt SDK layout

The v4 prebuilt archive separates target artifacts (`deps/`) from host proc-macro
libraries (`host/`). Cargo builds with an explicit `--target <rustc host triple>`,
so host build dependencies cannot introduce a second metadata variant into the
target SDK. Only host proc-macro dynamic libraries are shipped, not their host
dependency metadata. Consumers pass both folders as `-L dependency` paths; named
extern and facade lookups use only `deps/`. `native/` still holds native linker
inputs. The identity manifest hashes files in all three folders.

Target compilation retains `-C prefer-dynamic` (and `-C rpath` on macOS), while
Cargo's explicit target keeps those flags off host proc-macros. Multiple versions
of a target dependency are legitimate: uniqueness is required only when selecting
a named extern or publishing an internals facade, never across the whole SDK.

To rebuild and prove the archive with the launcher's pinned Rust toolchain active:

```sh
python3 tools/prebuilt_game.py build prebuilt
python3 tools/prebuilt_game.py smoke prebuilt
python3 tools/prebuilt_game.py exports prebuilt
python3 tools/native_dylib_probe.py --sdk prebuilt --output target/native-dylib-probe
```

For a server archive, add `--server` to the build command. Use a fresh
`CARGO_TARGET_DIR` when collecting a clean-build proof; all commands must use the
same compiler as `launcher/src/rust_toolchain.rs`.

## Upstream service contracts carried by this sync

The upstream additions below describe behavior to preserve in the typed `Host` and
handle API, not additional `native_*` entry points or a bundle API counter. Use the
book and generated API reference for the available signatures; pending hook fragments
must be converted to the hook catalog before being advertised as available.

- Tamed-mob death messages are offered after construction and only when an online
  owner would receive them with `show_death_messages` enabled. Listeners may replace
  or suppress the message in dispatch order. The server-only named-mob log is not
  a second player-message hook.
- Dedicated-server settings describe the live listener: account authentication,
  bound port and whether Velocity forwarding is active. They never expose the
  forwarding secret, and are unavailable before listening or on integrated/LAN
  servers. Secure-profile enforcement and hardcore are not part of this read.
- Vehicle and passenger reads use entity handles (`entity_vehicle` and
  `entity_passengers` on `Host`), not parallel numeric-ID helpers. Passenger order
  follows vanilla, with the controlling passenger first. These are read-only.
- UUID lookup from a read-only context must not swap levels. A mutable lookup can
  reach other loaded levels; entity NBT reads there must refuse unloaded levels,
  missing entities and client-side use rather than load a level implicitly.
- Map reads return the 128 × 128 row-major color bytes and scale, dimension and
  locked metadata. Reading saved data must not install a loaded map as a side
  effect; unknown maps and client-side world reads return no value.
- Living-entity sight uses eye-to-eye COLLIDER shapes, no fluids, the source's
  collision context and vanilla's 128-block limit. Unknown entities, different
  levels and unavailable server levels must not report visibility. The typed
  `living_has_line_of_sight` and `level_line_of_sight` reads replace raw-ID helper
  proposals; the point-to-point form has no entity collision context.
- Camera submersion (main's `native_camera_submersion(camera, fog_type)`, API 646): a client
  listener runs when the camera's fluid submersion type is evaluated
  (`Camera.getFluidInCamera`), before fog and underwater screen overlays are prepared.
  Setting `fog_type` to `FogType::Atmospheric` clears underwater, lava and powder-snow fog
  and overlays (Freecam uses it). Hook entry: `camera_submersion` v1, client, `listen`.
- Client entity update (main's `native_client_entity_update(viewer, id, nbt) -> bool`,
  `host.client_entity_update`): like `native_client_entity_set` for a few fields. It reads
  `nbt` as `set` does but sends only the data of the keys `nbt` has (`transformation` sends
  its four parts); a key at its default value is sent, so a field can go back to it. A
  display's `start_interpolation` goes with it, so a changed transformation eases over
  `interpolation_duration`. `set` still sends every field. False when `id` isn't this
  viewer's client-side entity, when the viewer isn't on, and outside a player's hook.

These are contract notes, not runtime verification of the converted services.

## Living absorption

Living handles expose `absorption(host)` on both sides and
`set_absorption(host, amount)` on the server. Resolution follows health: armor
stands, dragons and stale handles return `None` on reads. A non-finite write
returns `false` and changes nothing; accepted amounts clamp to
`0..=max_absorption` for that entity. Player and mob writes roll back with the
transaction. This is the absorption amount, not the absorption status effect.

## The fork's native APIs (ATM10)

The fork's additions to the native mod API (levels 416 to 441, `docs/atm10-port.md`), kept here as the fork wrote
them until S2 gives each hook its vocabulary file and S3 its bundle rows. The names are the fork's `native_*` ones.

The pending `panel_catalysts` helper, `native_panel_catalysts(mod_id, catalysts)`, shows the items that make the recipes
on the panel's page as a column of slots left of them, as JEI draws a category's catalysts; a click on one comes back as
`NativePanelEvent::Catalyst { index, stack, right }`, a key over one as `Key`. At most 16; a new recipe page has none
until the mod sets them again. A recipe's slots beyond its layout's are drawn as more outputs after the result, and a
`note` of several lines (`\n`) takes a row each. Bundles need `"hooks": @API:panel_catalysts@`.

Since API 422 every fluid of the registry is (a content mod's too), and AE2's buses and `native_fluid_interact` reach these tanks through the fluid handler dispatcher.

API 416 adds [Forge Energy](#forge-energy) on a mod block's block entity, by side, as NeoForge 1.21.1's `IEnergyStorage` capability: the hook points `native_fe_storage`, `native_fe_receive` and `native_fe_extract` answer for your block entities, and the helpers `native_fe_info`, `native_fe_offer` and `native_fe_draw` reach any block with energy in any open level: other mods' (their hooks, or the buffer a block declares through `native_block_entity_energy`) and the game's own machines. Upstream's `native_energy`, `native_energy_insert`, `native_energy_extract` and `native_move_energy` reach the same blocks in the level in place. Bundles that use them need "hooks": 416.

API 417 adds what a machine's item and slots need: `native_block_entity_drop` puts components on the item a broken block drops (`copy_components` from a block entity in vanilla's block loot tables, or only when `by_player`, as a mod's `Block.playerDestroy` would; `native_place_block` hears the item placed again), `native_block_entity_slot_accepts` narrows what a hopper or a menu click puts into a mod's slot (`Container.canPlaceItem`, `Slot.mayPlace`), and the helpers `native_burn_time(stack)` and `native_crafting_remainder(item)` read a stack's furnace fuel ticks (`ItemStack.getBurnTime`) and what a used-up item leaves (`Item.getCraftingRemainder`). Bundles that use them need "hooks": 417.

API 418 adds a block entity's lifecycle, as NeoForge's `BlockEntity.onLoad`, `setRemoved`, `onChunkUnloaded` and `saveAdditional`: `native_block_entity_load` (placed, or loaded with its chunk; told at the start of the level's next running block entity pass), `native_block_entity_unload` (`removed`: broken or replaced, told at that same point; otherwise its chunk is unloading, told at once, after the chunk's last save snapshot), `native_block_entity_save` (change the copy of `nbt` that goes to disk) and the helper `native_block_entity_changed(pos)` (`setChanged` without touching the entity). A cable network kept in a mod's own state needs no ticking block entity with them. Closing a world tells nothing: use `native_world_lifecycle`. Bundles that use them need "hooks": 418.

API 419 adds what a generator reads of the world around it, beside `native_light`, `native_weather` and `native_day_time`: the helper `native_biome_climate(pos)` gives the biome `Level.getBiome` picks at a block (the zoomed noise cell, as vanilla) with its climate there, `NativeBiome { id, base_temperature, temperature, downfall, precipitation }`, where `temperature` is `Biome.getTemperature(pos)` (cooler above the sea level + 17) and `precipitation` none, rain or snow (`getPrecipitationAt`); and `native_dimension_type()` gives the type of the dimension in place, `NativeDimensionType { has_skylight, has_ceiling, ultra_warm }`. (Upstream's `native_biome(pos)` gives the id alone.) Both read the world this game holds, as `native_light` does. Bundles that use them need "hooks": 419.

API 420 adds a mixin bundle saying it listens to entities joining and leaving a level: the game walks its entity lists for `native_entity_join_level` and `native_entity_leave_level` only while something hears them, and a mixin isn't in the trait mods' list, so a bundle that hooks them hooks `native_entity_level_listened(listened)` too and sets `listened`. Trait mods need not. Bundles need "hooks": 420.

API 421 adds the helper `native_cooking_recipes()`: every furnace, blast furnace, smoker and campfire recipe of the world's tables in file order, each with its `kind` (`minecraft:smelting`...), its ingredient's items, its result, cooking time and experience, so a mod's electric furnace smelts what a furnace does (Mekanism's Energized Smelter). The slice stays the same until the tables change (`/reload`), so a mod can index it once per slice. It is empty outside a world and, like `native_recipes_of_type`, the server's table on the server thread. Bundles that use it need "hooks": 421.

API 422 adds fluid handlers on a mod block's block entity, by side, as NeoForge 1.21.1's `Capabilities.FluidHandler.BLOCK` (`IFluidHandler`), with fluids as `horizon_world::fluid_stack::FluidStack`s (NeoForge's `FluidStack`: a fluid of the registry, vanilla's or a content mod's, an amount in millibuckets and its component patch, saved by `FluidStack.CODEC`'s NBT `{id, amount, components}`). The hook points, asked as the Forge Energy ones of API 416 are: `native_fluid_handler_tanks(dimension, pos, face, entity, tanks)` sets `tanks` (each tank's `stack`, `capacity` and `drainable`, whether a simulated drain of it gives anything) when `entity.id` is yours, none on a side without a handler; `native_fluid_handler_fill(dimension, pos, face, entity, resource, simulate, filled)` (`fill`) and `native_fluid_handler_drain(dimension, pos, face, entity, resource, max, simulate, drained)` (`drain(FluidStack)` with a `resource`, else `drain(int)` of at most `max`), the entity out of the world during the call. A block that answers none and declared tanks (API 54) is read and moved by those tanks. The helpers: `native_fluid_handler(dimension, pos, face)`, `native_fluid_fill(dimension, pos, face, resource, simulate)` and `native_fluid_drain(dimension, pos, face, resource, max, simulate)`, which reach any open level, and `native_fluid_interact(hand, dimension, pos, face)`, NeoForge's `FluidUtil.interactWithFluidHandler` for the player in place, for a block's use: the container in `hand` is filled from the block, or else emptied into it, with the bucket's sound, one alone replaced, one of several shrunk and the result stowed or dropped, a creative player keeping it. AE2's storage, import and export buses reach these handlers. A cauldron of NeoForge's `CauldronFluidContent` (vanilla's three, and a content mod's that its fluid declares, docs/mod-fluids.md slice 7a) answers on every side, in the level in place, as NeoForge's `CauldronWrapper`: one tank, filled and drained only in whole increments (water a whole bucket for its three levels). The tanks of API 54 and the item fluid helpers of API 147 now take every fluid of the registry, a content mod's bucket holding its fluid as vanilla's do. Bundles need "hooks": 422.

API 423 adds `native_data_files(dir)`, NeoForge's `ResourceManager.listResources(dir, path -> path.endsWith(".json"))` over the game's resources, content mods' included: every `data/<ns>/<dir>/**/*.json` in every namespace, sorted, for `native_data` to read. A mod reads the entries of a registry of its own that content mods declare as data with it: Mekanism's bundle (`pack/mods/mekanism`) reads its chemicals from `data/<ns>/mekanism/chemical/` (docs/mekanism-chemicals.md). A world's own data packs aren't in it. Bundles need "hooks": 423.

API 424 adds what a mod's item used on a block needs to act as the item's own `useOn`: `native_used_stack()`, the stack in the hand the acting player uses where `native_use_item_on` and `native_use_item` run (`getItemInHand(hand)`; shrink it, or set `None`, to use the item up), and `native_instabuild()`, whether that player builds without using items up (creative, `Abilities.instabuild`). With upstream's `native_default_state(block)` for `native_set_block` and `native_state_with_property`, Mekanism's bundle (`pack/mods/mekanism`) turns a machine into a factory and a factory into the next tier with its tier installers on them. Bundles need "hooks": 424.

API 425 adds `native_insert_slots(dimension, pos, face, stack)`, the slots of a container that `native_insert_item` puts a stack into through a side, in that order, with what each holds, whether it takes the stack (`isItemValid`) and up to how many of it (`getSlotLimit`), so a mod works out what an inventory will take before it sends anything; and `native_pop_resource(pos, stack)`, `Block.popResource` in the level in place. Mekanism's bundle (`pack/mods/mekanism`) routes its Logistical Transporters' stacks with them, drops a broken transporter's, and, with upstream's `native_sneaking()`, gives its Configurator a sneaking use. Bundles need "hooks": 425.

API 426 adds `native_level_particles(options, at, spread, speed, count)`, `ServerLevel.sendParticles` from a server hook outside a block's tick, in the level in place: a packet to each player whose block is within 32 blocks of `at`, and this window's own copy when its player is that near, as `native_send_particles` sends them from inside a block's tick. Mekanism's bundle (`pack/mods/mekanism`) sends a multiblock's red sparkles with it when the multiblock forms at the end of a level's tick. Bundles need "hooks": 426.

API 427 adds `native_recipe_categories(categories)` (client) collects the mods' own recipe categories for a recipe viewer: push a `NativeRecipeCategory` (id, title, subtitle, icon, catalysts, and recipes of inputs to outputs, each output with its chance, count range and notes) for a fishing spot, a sieve or a machine. A viewer gets them all with `native_collect_recipe_categories()` and asks again when `native_recipe_categories_version()` moves, which a mod makes happen with `native_recipe_categories_changed()` (its server's data came). Bundles need "hooks": 427.

API 428 adds `native_map_tile(chunk)`: a loaded chunk's 16x16 map picture in the player's dimension, RGBA rows north to south, as the minimap extra draws the surface (each column's `MapColor` with the map item's height shading and water depth), from the minimap's tile cache or scanned on the spot; none for a chunk not loaded and off the window's client. A mod's map (FTB Chunks' claim screen and map, JourneyMap) asks for a few chunks a frame, keeps them, and shows them with `native_mod_image`. Bundles need "hooks": 428.

API 429 adds `native_show_recipes(stack, lookup)` (client, `host.show_recipes`) asks whatever recipe viewer is installed to show `stack`'s recipes (`NativeRecipeLookup::Recipes`), its uses (`Uses`), or to act on a key that went down over it in a screen the mod draws (`Key("key.keyboard.a")`), so a quest book's items open the viewer as its own list's do. The viewer hears it at the start of the next frame as `native_recipe_lookup(stack, lookup, handled)` and sets `handled`; with no viewer, nothing happens. Bundles need "hooks": 429.

API 431 adds `native_panel_area(mod_id)` (client) gives the area mod `mod_id`'s item panel takes on the open screen, `[x, y, width, height]` in GUI pixels, or None while it isn't showing (no container screen, another mod's panel, nothing to show, no room). A window the mod draws over the screen from `native_screen_render_post` (JEI's recipe window) keeps clear of it and lets the clicks on it through from `native_screen_mouse_button_pressed_pre`. Bundles need "hooks": 431.

API 434 adds `native_panel_grid(mod_id, grid)` (client): how big mod `mod_id`'s item panel grid may grow and where it sits, kept for the session (`NativePanelGrid`: `max_columns`, `max_rows`, `horizontal` and `vertical` as `NativePanelAlign::{Start, Center, End}` in the room beside the screen, and `keep_search`, which keeps the page buttons and search box while the mod shows no items). The grid takes as many columns and rows as fit, up to the limits, as JEI's ingredient list does with `[ingredientList] maxColumns/maxRows` (ATM10 asks for 18 x 99); Horizon's Recipes panel keeps 9 x 16 at the right until it asks. `native_panel_area` follows the grid. Bundles need "hooks": 434.

API 435 adds `native_chemical_handler_tanks(dimension, pos, face, tanks)` (`&self`, read only), `native_chemical_handler_insert(dimension, pos, face, chemical, amount, simulate, inserted)` and `native_chemical_handler_extract(dimension, pos, face, chemical, amount, simulate, extracted)` (server) are a block's chemical handler through `face`, as Mekanism's `Capabilities.CHEMICAL.block()` answers it: the tanks (`NativeChemicalTankInfo`: `chemical`, its registry id, empty for none; `amount` and `capacity` in millibuckets; `extractable`, whether a simulated extraction through that side gives any), and inserting or extracting one chemical, simulated or for real. The engine names no chemical; the mod that owns chemicals (Mekanism's bundle) answers for its blocks and leaves the outs none elsewhere. AE2's Applied Mekanistics asks them for its storage, import and export buses (`Ae2Host::chemical_tanks`, `chemical_insert`, `chemical_extract`). Bundles need "hooks": 435.

API 436 adds the helpers `native_food_table()` and `native_food(stack)`: the food table, every item with food (vanilla's `minecraft:food` component, `FoodProperties`) in the item registry's order, each as its default stack has it, and one stack's food, its own `food` and `use_remainder` components over its item's (`NativeFood`: `item`, `nutrition`, `saturation`, `can_always_eat` and `remainder`, what eating it leaves: 1.21.1's `usingConvertsTo`, a stew's bowl). Read only; the table is built on each call, so a mod keeps what it needs. Mekanism's Nutritional Liquifier builds its recipe from the food in its slot (`NutritionalLiquifierIRecipe`: `nutrition * 50` mB of nutritional paste and the remainder) and its recipe viewer category from the table. Empty and none outside a world. Bundles that use them need "hooks": 436.

API 437 adds the helpers `native_chemical_tanks(dimension, pos, face)`, `native_chemical_insert(dimension, pos, face, chemical, amount, simulate)` and `native_chemical_extract(dimension, pos, face, chemical, amount, simulate)` (server): the engine's own blocks as Mekanism's chemical handler, the other way from API 435, so a mod's machines and pipes push chemicals into them and pull them out. AE2's generic inventories answer, as Applied Mekanistics 1.6.3's `GenericStackChemicalStorage` does: an ME Interface's storage slots and a Pattern Provider's return slots (blocks, and parts on that side of a cable bus), one tank a slot (`NativeChemicalTankInfo`: the slot's chemical or empty, the type's slot capacity, whether it gives it back; a provider's slots take and give nothing back, and return what they take to the grid; an interface's tick hands what no config asks for to its grid). The key type whose keys are chemicals is the one an addon registers with `ae2_key_types::register_chemical_type` (Applied Mekanistics' `appmek:chemical`); without it, AE2 or such a block, the tanks are none and nothing moves. Call them where your own handler lookup finds nothing (Mekanism's `with_handler` falls through to them). Bundles need "hooks": 437.

API 441 puts those ingredients in the item panel too, as JEI's ingredient list holds fluids and the mods' types beside the items: `native_panel_items` may send a `NativeStack` of kind `"fluid"` or a mod's type; the panel draws a fluid as its icon (`native_fluid_icon`) and a type as `native_panel_ingredients(mod_id, Vec<NativePanelIngredient>)` says (`NativePanelIngredient { info: NativeIngredientInfo, lines }`: the sprite in its tint, the name as the tooltip's first line and the slot widget's label, then `lines` with `§` codes; an empty sprite draws a fluid as its icon; kept per mod for the session, each call replacing the last). A click on one comes back as `Clicked` with its index and a key over one as `Key` with the stack as sent, as for an item. `native_register_ingredients(kind, Vec<NativeIngredientInfo>)` gives every ingredient of a mod's type for the viewers' lists (JEI's `IModIngredientRegistration.register`; Mekanism registers its chemicals on the client in a world, and none out of it; a change moves `native_recipe_categories_version`), `native_registered_ingredients()` reads them back, type by type. `native_fluids()` lists the fluids a viewer lists, the registry's source fluids in its order (JEI's `FluidStackListFactory`: water, lava, then the content mods'). JEI appends them in its order (by mod, then items, Mekanism's chemicals, fluids) with the blacklist applied to them. The helpers' bundles need "hooks": 441.

API 440 adds ingredients that are not items to recipe viewers: `NativeStack::kind` ("" an item, `"fluid"` a fluid, any other a mod's own ingredient type such as Mekanism's `mekanism:chemical`; `item` is the ingredient's id and `count` its amount in millibuckets for the two kinds), with the constructors `NativeStack::item(id, count)`, `fluid(id, amount)`, `typed(kind, id, amount)` and `is_item()`; `NativeIngredientInfo` (`kind`, `id`, `name`, `sprite`, `tint` `0xAARRGGBB`) and `NativeRecipeCategory::ingredients`, how a category's typed ingredients look (name in the player's language and the GUI sprite with its tint, as JEI's `IIngredientRenderer` of the type draws it; fluids need none); and the helpers `native_fluid_icon(batch, fluid, at, size)` (a fluid's still sprite in its tint, as JEI's `FluidTankRenderer` and AE2's terminals draw one; false for an unknown fluid) and `native_sprite_tinted(batch, name, src, dst, argb)` (`native_sprite` in a tint). A category's slots hold any of the kinds (`NativeCategoryRecipe::inputs`, `NativeChanceOutput::stack`), so a viewer matches by `(kind, id)`; a viewer that only draws items shows what it does not know as nothing. AE2's item transformation shows its water as `NativeStack::fluid("minecraft:water", 1000)`; Mekanism's categories give their chemicals and fluids as slots (JEI draws them with the amount in mB in the tooltip, R and U on them open the ingredient's recipes and uses). The helpers' bundles need "hooks": 440.

### Forge Energy

`crates/horizon-client/src/game/blocks/machines/native_fe.rs`: Forge Energy (FE) between blocks, as NeoForge 1.21.1's `IEnergyStorage` capability, which Mekanism, Powah, Flux Networks and AE2 speak at their edges. A block exposes it per side; `face` is always the side of the block being reached (a cable east of a battery reaches its `East` side), and none means the block itself, as a null side. Amounts are FE as `i64`. A call is narrow, Java's `int`, unless it says `wide` (Flux Networks' long FE): a narrow amount is clamped to `i32::MAX` before a hook sees it and a narrow readback saturates there. There are no transactions: `simulate` asks and changes nothing.

- **Your block entities** answer on the hook points, after checking `entity.id` and that the out parameter is still none (several mods hook the same point): `native_fe_storage` sets `NativeFeInfo { stored, capacity, can_receive, can_extract }` for a side that has energy and leaves it none for one that hasn't; `native_fe_receive` and `native_fe_extract` set what they took or gave, 0 to `max`, and change nothing with `simulate`. During a receive or extract the entity is out of the world, as in its tick, and the engine marks it changed when energy moved. Answer the readback from `entity` and your own state: it is asked often, from inside other blocks' transfers, maybe while another level is in place.
- **The helpers** find a block's FE: `native_fe_info(dimension, pos, face, wide)` reads it (none where that side has no energy), `native_fe_offer(dimension, pos, face, amount, simulate, wide)` offers energy and answers what was taken, `native_fe_draw(...)` draws it and answers what was given. They reach mod block entities and the game's own machines (in this fork AE2's energy acceptors, chargers, FE P2P tunnels and debug generator, which in turn reach mod blocks), in the level in place and the parked ones, on the server only.
- **Loops.** The block entity ticking now, or answering a receive or extract now, is out of the world and answers nothing. A forwarder may call the helpers from inside its hooks: the same block, side and operation asked again inside its own call answers nothing (a readback, an empty one), and so does a chain 16 calls deep.
- **One energy with upstream's buffers.** A mod block that declares a buffer through `native_block_entity_energy` (API 142, `Capabilities.Energy.BLOCK`) and answers none of the hooks above reads and moves as that buffer (`SimpleEnergyHandler`: its `energy` int, its limits per call and its faces; a null side reaches all of them). The other way round, `native_energy`, `native_energy_insert`, `native_energy_extract` and `native_move_energy` reach a block without a declared buffer through these hooks and the game's machines, in the level in place, as narrow calls; an `IEnergyStorage` has no per-call limits, so `native_energy` reports `i32::MAX` for a side that receives or gives. Mekanism's cubes and cables (per-entity side configuration, cables that hold nothing), Flux Networks (64-bit amounts, any level) and Powah's Furnator (its Java save keys) stay on the hooks.

- `native_machine_block_drops(pos, tool)` (`Host::block_drops`) is `Block.getDrops(state, level, pos, blockEntity, null,
  tool)`: the drops `native_machine_destroy_block` gives for the same tool, rolled with nothing broken, popped or
  awarded, for a machine that checks they fit before it breaks the block (Mekanism's Digital Miner).

A slot is not only items (API 440): a `NativeStack` has a `kind`, `"fluid"` for a fluid (`item` its id, `count` its
amount in mB) or a mod's own ingredient type (`mekanism:chemical`, `item` its id, `count` its amount), as JEI's
ingredient types. A category lists how its typed ingredients look in `NativeRecipeCategory::ingredients`
(`NativeIngredientInfo`: kind, id, name, GUI sprite and tint), and a viewer draws a fluid with
`native_fluid_icon(batch, fluid, at, size)` and a typed one with `native_sprite_tinted` of that sprite. JEI's window
shows each as a 16x16 slot with JEI's tooltip (the name, the amount in mB in grey, the mod's name), and R, U and a click
on it open that ingredient's recipes and uses; the focus is the key `<kind> <id>` (`fluid minecraft:water`), an item's
its id. Its list shows items only for now.
