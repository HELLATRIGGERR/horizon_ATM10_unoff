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
`minecraft:water` or `minecraft:lava` template, which gives it `LiquidBlock`'s class and `level` property),
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

**Fluid stacks in tanks.** Not in slice 1. The plan: a `FluidStack` (id, amount in mB, components) in
`horizon-world`, the registry's ids as its fluids, and a fluid handler capability for native mods' block entities
(NeoForge's `IFluidHandler`: tanks, fill/drain with simulate), next to Forge Energy's hooks; AE2's storage buses and
Mekanism's tanks then meet through it.

**AE2 keys.** AE2 keeps keying fluids by id. `bucket_fluid` and `fluid_bucket` read the registry, so a mod fluid goes
into and out of fluid cells, the ME Chest and terminals by its bucket, and its icon is its still texture with its tint.

## Slices

1. **Registry, flow, buckets, rendering, AE2 keys (this slice).** The declaration and its checks, `FluidKind` as a
   registry id with the mod registry and its tags, the flow port on the mod's numbers, buckets both ways, the mesher's
   sprites and tint, AE2's bucket mapping and icons. Mekanism's heavy water and brine are declared in
   `pack/mods/mekanism/content` (Mekanism's own numbers, read from its `MekanismFluids`), and `mekanism-content` brings
   the pack's fluid tags. Tests: `horizon-mods` (declaration), `horizon-world` (`mod_fluids`), `horizon-items`
   (buckets), the content-root test `a_mod_fluid_flows_by_its_own_numbers_and_fills_its_bucket`, AE2's
   `a_mod_fluid_bucket_fills_a_fluid_cell`, and the world test `pack/mods/mekanism/tests/mod-fluid-heavy-water.ts`.
2. **Every pack fluid carried.** The NeoForge oracle's `FactsExporter` writes each fluid of its namespaces (source and
   flowing ids, block, bucket, `BaseFlowingFluid`'s and `FluidType`'s numbers by reflection, sounds, the render
   layer) and, where the type exposes them, its client textures and tint (fields such as Mekanism's
   `MekanismFluidType`; the rest wants a client-side export); `horizon-port-pack carry` writes the declarations and
   keeps the jars' fluid tags. Then Mekanism's hand declarations go.
3. **Bucket and container item models.** NeoForge's `neoforge:fluid_container` model (`DynamicFluidContainerModel`:
   the bucket with the fluid's still texture masked in and tinted); today the carried bucket models have no
   `neoforge:item/bucket` parent to draw.
4. **Entities in mod fluids.** `FluidType`'s `canSwim`, `canDrown`, `canPushEntity`, `motionScale`,
   `fallDistanceModifier`, `canExtinguish`, `supportsBoating`, the camera's fog and overlay
   (`modifyFogColor`/`getRenderOverlayTexture`), and mod fluids in the `minecraft:water` or `minecraft:lava` tag
   (`the_bumblezone:sugar_water`) behaving as water or lava in physics and interactions.
5. **Fluid stacks and tanks.** As above; Mekanism's Heat Generator's lava tank first, then its machines' tanks, Modern
   Industrialization's and Industrial Foregoing's.
6. **Fluid ingredients and recipes.** NeoForge's `FluidIngredient`/`SizedFluidIngredient` and the mods' fluid recipe
   inputs and outputs, against the registry and its tags.
7. **The rest of `FluidType`.** Vaporizing in an ultrawarm dimension (`isVaporizedOnPlacement`), `FluidInteractionRegistry`
   (mods' own lava-like interactions), dripstone and cauldrons, light from fluids, LOD and minimap colours, mod fluids
   over the network to a client whose content differs, and AE2's Missing Content for a fluid the registry lacks.
