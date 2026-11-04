# Mod fluids in the engine

Roadmap item 2.1. Mekanism's machines, Modern Industrialization, Mystical Agriculture and Industrial Foregoing all
hold, pump or craft with fluids of their own. On NeoForge 1.21.1 a mod's fluid is a `FluidType` (density, viscosity,
temperature, light, sounds, `canConvertToSource`...), two `BaseFlowingFluid`s (its source and its flowing fluid, with
`tickRate`, `slopeFindDistance`, `levelDecreasePerBlock`, `explosionResistance`), a `LiquidBlock` that holds it in the
world, a `BucketItem`, and on the client an `IClientFluidTypeExtensions` (still, flowing and overlay textures, a tint)
with a render layer from `ItemBlockRenderTypes`. ATM10 8.4 registers 733 fluids in 415 fluid types.

## What there was

- `horizon-world`'s fluids were vanilla's only: `FluidKind` was a closed enum (empty, water, lava and their flowing
  forms), and `FluidState::from_block`, `FluidBlockBindings` (`createLegacyBlock`), the flow port in
  `level/fluids.rs` (`FlowingFluid.tick`, `spread`, `getSpread`, `getNewLiquid`...), buckets (`level/buckets.rs`),
  the mesher (`mesher/fluid.rs`, water's and lava's sprites), the client's block facts (`blocks::Fluid`) and saved
  fluid ticks all knew water and lava by name. `registry_fluid.rs` binds those states to the block registry for
  entity and ray queries; it has no registry of fluids of its own.
- A carried mod's liquid block came over as a block on the `minecraft:lava` template (`horizon-port-pack`'s oracle
  picks a vanilla block of its class), with lava's behaviour rows: in the world it ticked as lava and flowed into
  `minecraft:lava`, while the client drew nothing (its fluid read as empty by name). Its bucket was a plain item on
  the `minecraft:stick` template. Fluid types, textures, tints and the jars' fluid tags were not carried (the full
  pack's `atm10-pack` carry has the final `c:`/`minecraft:` fluid tags from the NeoForge dump; a mod's own carry
  does not).
- AE2 keys fluids by id (`ItemKey { item_id: "minecraft:water" }`, any well-formed id is accepted), but its bucket
  transactions (`ae2_cell_codec::bucket_fluid`, `fluid_bucket`) and its fluid icons knew water and lava only.

## Design

**Declaration.** A content mod declares a fluid in `data/<id>/horizon/fluid/<path>.json` beside its blocks and items
(`horizon_mods::content::Fluid`). The file names the source fluid (`<id>:<path>`); everything else defaults as
NeoForge's builders do:

```json
{
  "bucket": "mekanism:heavy_water_bucket",
  "still_texture": "minecraft:block/water_still",
  "flowing_texture": "minecraft:block/water_flow",
  "overlay_texture": "minecraft:block/water_overlay",
  "tint": "0xFF0D1455",
  "translucent": true,
  "sounds": {"bucket_fill": "minecraft:item.bucket.fill", "bucket_empty": "minecraft:item.bucket.empty"}
}
```

`flowing` (default `<id>:flowing_<path>`), `block` (default the fluid's id; one of the mod's blocks on the
`minecraft:water` or `minecraft:lava` template, which gives it `LiquidBlock`'s class and `level` property, or
`minecraft:air` for a fluid with none: `createLegacyBlock` gives air, and only tanks and buckets hold it), `flows`
(default true; false for a `Fluid` that is no `FlowingFluid`, as Modern Industrialization's: no flowing fluid, its
`block` air, and its bucket places nothing, as `BucketItem.emptyContents` refuses a fluid that doesn't flow),
`tick_rate` 5, `slope_find_distance` 4, `level_decrease_per_block` 1, `explosion_resistance` 1, `can_convert_to_source`
false, `light_level` 0, `density` 1000, `temperature` 300, `viscosity` 1000, `description` `block.<ns>.<path>`. `tint`
is ARGB, as Java's int or `0xAARRGGBB`. Fluid tags are ordinary data: `data/<ns>/tags/fluid/*.json`, merged into the
content root like other tags.

**Registry ids.** `FluidKind` is a registry id (`u16`): vanilla's five, then each mod fluid's flowing and source fluid
in that order (as vanilla's water and lava), mods' fluids sorted by id. Vanilla's keep their constants
(`FluidKind::Water`), so existing code reads and matches as before; `same_family`, `source()` and `flowing()` are
arithmetic on the id. The list (`horizon_world::mod_fluids`, `ModFluid`) is registered once per process from the
startup content root, before the block tables load, as the mods' mob effects are; a reused root carries its fluids in
its summary. The fluid tags of the root are resolved into each `ModFluid` (`source_tags`, `flowing_tags`). Fluid tick
types and saved fluid ticks use the id and the name.

**World and flow.** `FluidState::from_block` maps a registered mod's liquid block by its `level` as `LiquidBlock` does;
the behaviour tables give such a block's states that fluid instead of their template's; `createLegacyBlock` places the
mod's block. The flow port asks the fluid for its numbers: water's and lava's as before, a mod's from `ModFluid`
(`BaseFlowingFluid.getTickDelay`, `getDropOff`, `getSlopeFindDistance`, `FluidType.canConvertToSource`, and its
`canBeReplacedWith` and `beforeDestroyingBlock`, which are water's but for the family test). Lava's own overrides
(`spreadTo` into water, the random spread delay, `randomTick`) stay lava's.

**Buckets.** A registered bucket item is a `BucketItem` of its source (`bucket_contents`), places the source as water's
bucket does, and `LiquidBlock.pickupBlock` gives it back; the sounds are the type's `BUCKET_FILL`/`BUCKET_EMPTY`
(`BaseFlowingFluid.getPickupSound`, NeoForge's `playEmptySound`).

**Rendering.** The block atlas takes each mod fluid's still, flowing and overlay textures; the mesher draws a mod
fluid as water (same heights, flow UVs, overlay beside glass and leaves) with its own sprites, its tint in place of
the biome's water colour, and in the translucent layer when it asks for it (NeoForge's default is solid).

**Fluid stacks in tanks (slice 5).** `horizon_world::fluid_stack::FluidStack` is NeoForge's: a fluid of the registry
(`FluidKind`), an amount in mB and its component patch (kept as the patch's NBT, carried and compared, not read), with
`EMPTY`, `isEmpty`, `copyWithAmount`, `split`, `isSameFluid`, `isSameFluidSameComponents`, `is(tag)` (a mod fluid by
its registry entry's tags; vanilla's water and lava by `minecraft:water`/`minecraft:lava` and NeoForge's
`c:water`/`c:lava`) and `FluidStack.CODEC`'s NBT (`{id, amount, components}`: `save`, `parse`, `parseOptional`, a bare
path being `minecraft:`'s), and the fluid type's bucket and bucket sounds. A native mod's block entity has a fluid
handler by side as NeoForge's `Capabilities.FluidHandler.BLOCK` (`IFluidHandler`): hook points API 160,
`native_fluid_handler_tanks` (`getTanks`, `getFluidInTank`, `getTankCapacity`, and whether a simulated drain of the
tank gives anything, which AE2's storage bus filter asks), `native_fluid_handler_fill` and `native_fluid_handler_drain`
(`drain(FluidStack)` and `drain(int)`), with `FluidAction.SIMULATE`; the dispatcher (`machines/native_fluid_handler.rs`)
takes the entity out of the world for a fill or a drain and guards re-entry as Forge Energy's does, and a block that
answers none of them but declared tanks (API 54) is read and moved by those tanks' rules. The tanks of API 54 and the
item fluid helpers of API 147 take every fluid of the registry: a content mod's bucket is a `FluidBucketWrapper` (a
whole bucket of its fluid or nothing; an empty bucket takes any fluid that has a bucket). `FluidUtil`'s container
transfers are ported as NeoForge has them (`tryFluidTransfer`, `tryFillContainer`, `tryEmptyContainer`, their
`AndStow` forms with the creative player keeping the container and the result stowed by
`ItemHandlerHelper.insertItemStacked` or dropped, the fluid type's bucket sounds), and
`native_fluid_interact(hand, dimension, pos, face)` is `FluidUtil.interactWithFluidHandler`, for a mod block's use to
call (NeoForge calls it for no block of its own). AE2 meets a native handler as it meets any `IFluidHandler`
(`ExternalStorageFacade.FluidHandlerFacade`, `ae2_mod_fluids.rs`): a storage bus lists each tank (only the drainable
ones under its default extractable-only filter), inserts with `fill` and extracts with `drain(FluidStack)`; import and
export buses move through it after AE2's own hosts (P2P tunnels, interfaces, pattern providers, the sky stone tank). A
key with components moves nothing there. Its first user is Mekanism Generators' Heat Generator (`pack/mods/mekanism`):
its lava tank (`VariableCapacityFluidTank.input`, `heatTankCapacity` from `generators.toml`, 24 000 mB in ATM10) is on
every side but its front (`FluidTankHelper.forSide(...).addTank(lavaTank, LEFT, RIGHT, BACK, TOP, BOTTOM)`), takes
`FluidTags.LAVA` from any block, without a rate limit, into an empty tank or one holding the same fluid, and gives to
none (`notExternal`; the null side's proxy is read only); it is saved as Mekanism saves it
(`fluid_tanks: [{tank: 0b, stored: <FluidStack>}]`). Its fuel slot fills the tank through the item's fluid handler
(`IFluidHandlerSlot.fillTank`, `fillHandlerFromOther`: a bucket only whole, and one that doesn't fit counts as handled)
before it burns anything (`FluidFuelInventorySlot.fillOrBurn`). Lava source blocks beside it only heat it
(`TileEntityHeatGenerator.getBoost`): Java never draws them into the tank.

**AE2 keys.** AE2 keeps keying fluids by id. `bucket_fluid` and `fluid_bucket` read the registry, so a mod fluid goes
into and out of fluid cells, the ME Chest and terminals by its bucket, and its icon is its still texture with its tint.

**From the NeoForge oracle (slice 2).** Two runs of the whole pack give Java's values, none guessed. The server's
(`tools/atm10/oracle-pack.sh`, `FactsExporter` with `FluidFacts`) writes, in each batch's facts, every fluid of the
batch's namespaces: its class, type, source and flowing fluid (`FlowingFluid.getSource`/`getFlowing`, none for a
`Fluid` that doesn't flow), `getTickDelay`, `getSlopeFindDistance`, `getDropOff` and `getExplosionResistance` (the
protected ones by reflection) and `canConvertToSource` on the overworld, the `LiquidBlock` a source of it is
(`createLegacyBlock`), its bucket and pickup sound; and every `FluidType` they use with its description, light, density,
temperature, viscosity, rarity, `isLighterThanAir`, sounds (`BUCKET_FILL`, `BUCKET_EMPTY`, `FLUID_VAPORIZE`) and what it
does to a pig and a boat (`canSwim`, `canDrownIn`, `canPushEntity`, `motionScale`, `getFallDistanceModifier`,
`canExtinguish`, `canHydrate`, `supportsBoating`: slice 4's); and the batch's fluid tags. A server has no client
extensions, so the client's run (`tools/atm10/oracle-client.sh`, `neoforge_oracle.py client-pack`,
`mods/horizon/src/neoforge-oracle-client`) starts the pack on NeoForge's client with its window hidden (a mixin makes
it with `GLFW_VISIBLE` false; FML's early window is off; Crash Assistant, which opens a window of its own, is never
loaded), and once the mods have loaded (`FMLLoadCompleteEvent`) writes `client-facts.json`: each type's
`IClientFluidTypeExtensions` (still, flowing and overlay textures, `getTintColor`, the same for a bucket's stack, the
camera overlay), each fluid's `ItemBlockRenderTypes.getRenderLayer`, and each bucket's `ItemColors.getColor` by tint
index; then it halts (about 2.5 minutes, 8 GB of heap). `horizon-port-pack carry` (`fluids.rs`) declares each source
fluid of a mod's namespace from both: its liquid block is kept although the jar has no blockstate for it (Titanium's
fluids: `LiquidBlock`'s render shape is invisible), a fluid whose block another mod registers, whose block holds
another fluid or isn't on a liquid template, or whose numbers the flow can't take (a tick delay of 0) is left out with
the reason in the carry report (`fluids_unknown`), and each bucket's item definition takes its item colours as
`minecraft:constant` tints. `final-data` declares KubeJS's fluids the same way, and puts NeoForge's own item models and
masks (`assets/neoforge/models/item/`, `textures/item/mask/`, from the loader's jar) in `atm10-pack`.

**Bucket models (slice 3).** NeoForge's `"loader": "neoforge:fluid_container"` (`DynamicFluidContainerModel`) is read
with the model (`CuboidModel::fluid_container`: `fluid`, `flip_gas`, `cover_is_mask`, `apply_fluid_luminosity`, as its
`Loader.read` defaults them) and inherited down the parents, and `item_bake` bakes it as `DynamicFluidContainerModel.bake`
does: the `base` texture as an item layer (tint index 0), the `fluid` mask's shape
(`UnbakedGeometryHelper.createUnbakedItemMaskElements`, with its quirks: a run that reaches the right edge makes no box)
filled with the fluid's still texture (tint index 1, which the bucket's item colour tints; full bright for a fluid that
gives light), the `cover` mask over it, every layer translucent and lit from the front, the fluid and cover layers 1.002
and 1.004 out along z (`FLUID_TRANSFORM`, `COVER_TRANSFORM`), a gas upside down with `flip_gas`. The item renderer's
sprites (`SpriteLookup::fluid`) give vanilla's water and lava and the registry's mod fluids; the GUI atlas holds the mod
fluids' still textures wherever they lie (Mekanism's `liquid/`), first frame of an animated strip, as NeoForge's block
atlas does. NeoForge's `ClientHooks.fixItemModelSeams` is not ported (the item generator's own seams stand).

## Slices

1. **Registry, flow, buckets, rendering, AE2 keys (done).** The declaration and its checks, `FluidKind` as a
   registry id with the mod registry and its tags, the flow port on the mod's numbers, buckets both ways, the mesher's
   sprites and tint, AE2's bucket mapping and icons. Mekanism's heavy water and brine are declared in
   `pack/mods/mekanism/content` (Mekanism's own numbers, read from its `MekanismFluids`), and `mekanism-content` brings
   the pack's fluid tags. Tests: `horizon-mods` (declaration), `horizon-world` (`mod_fluids`), `horizon-items`
   (buckets), the content-root test `a_mod_fluid_flows_by_its_own_numbers_and_fills_its_bucket`, AE2's
   `a_mod_fluid_bucket_fills_a_fluid_cell`, and the world test `pack/mods/mekanism/tests/mod-fluid-heavy-water.ts`.
2. **Every pack fluid carried (done).** As "From the NeoForge oracle" above: the server's fluid facts, the client's
   run, the carry's declarations, fluids with no liquid block and fluids that don't flow. ATM10 8.4 (4 November 2026):
   the server's run (21 minutes with the dump and every batch) and the client's (2.5 minutes) give 718 fluids and 409
   types of the carried namespaces; the 288 carries and `atm10-pack` declare all 409 source fluids (406 carried, 3 of
   KubeJS): 309 that flow (281 with a liquid block, 28 without), 100 that don't (Modern Industrialization's 72,
   Extended Industrialization's 9, Iron's Spellbooks' 16 whose flowing fluid is themselves, three more), 369 with a
   bucket, 189 buckets tinted by their item colours, 124 of the 732 fluids translucent. The full-pack content root
   registers all 409 (`content_root_build`), and the load check takes the 289 mods with none refused. The pack's 8
   other fluids are in namespaces no carry owns (Advanced AE's, Sophisticated Core's, Sauce's, XyCraft Core's).
   Mekanism's hand declarations of slice 1 went: its fluids are its carry's (`mekanism-content` counts them and warns
   about a carry from before the client's run); the oracle's heavy water and brine are the files slice 1 wrote by
   hand, field for field. Tests: `horizon-port-pack` (`fluids::tests`, `carry::tests::a_mods_fluids_come_from_both_oracle_runs`),
   `horizon-mods` (fluids with no block, fluids that don't flow), and the world test
   `tools/atm10/tests/carried-fluids.ts` (four carried fluids against Java's values, placed, flowing, drawn and picked
   up; Modern Industrialization's crude oil in its declaration).
3. **Bucket and container item models (done).** As "Bucket models" above. Tests: `horizon-assets`
   (`a_fluid_container_masks_its_fluid_into_the_bucket`) and the buckets of `carried-fluids.ts` in the GUI (their fluid
   layer drawn, no missing texture, the tinted one tinted). Not ported: `ContainedFluidOverrideHandler` (a container
   whose fluid is in its stack's data, as tanks are: slice 5), the emissive layer's own render type (its quads are full
   bright, drawn as the rest).
4. **Entities in mod fluids.** `FluidType`'s `canSwim`, `canDrown`, `canPushEntity`, `motionScale`,
   `fallDistanceModifier`, `canExtinguish`, `supportsBoating`, the camera's fog and overlay
   (`modifyFogColor`/`getRenderOverlayTexture`), and mod fluids in the `minecraft:water` or `minecraft:lava` tag
   (`the_bumblezone:sugar_water`) behaving as water or lava in physics and interactions.
5. **Fluid stacks and tanks (done).** As "Fluid stacks in tanks" above. Tests: `horizon-world`
   (`fluid_stack::tests`: the codec, empty stacks and comparisons, tags, buckets and sounds), the game's
   `native_fluid_handler` (declared tanks by side with a mod fluid; a bucket moving only whole through
   `tryFluidTransfer`), `native_item_fluids` (a content mod's bucket), `ae2_mod_fluids` (keys to stacks), the
   `mekanism-energy` crate (`heat`: the fuel slot's fill and burn, the tank's sides, insert and extract), and the world
   test `pack/mods/mekanism/tests/heat-generator-tank.ts` (an export bus filling the tank to its capacity through a
   side and not through the front, a storage bus filling it with lava only and listing none of it, an import bus taking
   nothing, Mekanism's saved form across a restart). Not yet: AE2's annihilation plane picking up fluid sources
   (`FluidPickupStrategy`) and P2P fluid tunnels and pattern providers reaching native handlers; vanilla's cauldrons as
   NeoForge's `CauldronWrapper` handler (API 54's helpers read them; the dispatcher doesn't); Mekanism's machines'
   tanks, Modern Industrialization's and Industrial Foregoing's; a tank's contents in its item
   (`ContainedFluidOverrideHandler`).
6. **Fluid ingredients and recipes.** NeoForge's `FluidIngredient`/`SizedFluidIngredient` and the mods' fluid recipe
   inputs and outputs, against the registry and its tags.
7. **The rest of `FluidType`.** Vaporizing in an ultrawarm dimension (`isVaporizedOnPlacement`), `FluidInteractionRegistry`
   (mods' own lava-like interactions), dripstone and cauldrons, light from fluids, LOD and minimap colours, mod fluids
   over the network to a client whose content differs, and AE2's Missing Content for a fluid the registry lacks.
