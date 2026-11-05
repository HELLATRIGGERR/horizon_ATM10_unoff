package dev.horizonlab.neoforge;

import com.google.gson.stream.JsonWriter;
import java.io.IOException;
import java.io.StringWriter;
import java.lang.reflect.Method;
import java.util.Map;
import java.util.TreeMap;
import java.util.function.Predicate;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.sounds.SoundEvent;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.entity.vehicle.Boat;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.LevelReader;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.LevelChunk;
import net.minecraft.world.phys.Vec3;
import net.minecraft.world.level.material.FlowingFluid;
import net.minecraft.world.level.material.Fluid;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.level.material.Fluids;
import net.neoforged.neoforge.common.SoundActions;
import net.neoforged.neoforge.fluids.CauldronFluidContent;
import net.neoforged.neoforge.fluids.FluidStack;
import net.neoforged.neoforge.fluids.FluidType;
import net.neoforged.neoforge.registries.NeoForgeRegistries;

/**
 * The fluids of the export's namespaces (docs/mod-fluids.md, slice 2), as the server holds them: each fluid's
 * {@code FlowingFluid} numbers ({@code getTickDelay}, {@code getSlopeFindDistance}, {@code getDropOff}, {@code
 * getExplosionResistance}, {@code canConvertToSource}), its source, flowing fluid, {@code LiquidBlock} and bucket,
 * and each {@code FluidType} they use with its numbers, sounds and what it does to entities. The client half
 * (textures, tint, render layer) is {@code ClientFactsExporter}'s: a server has no client extensions.
 */
final class FluidFacts {
    private static final BlockPos AT = new BlockPos(0, 100, 0);
    private final ServerLevel level;
    private final Predicate<ResourceLocation> ours;
    private JsonWriter json;

    FluidFacts(ServerLevel level, Predicate<ResourceLocation> ours, JsonWriter json) {
        this.level = level;
        this.ours = ours;
        this.json = json;
    }

    void write() throws IOException {
        Map<String, FluidType> types = new TreeMap<>();
        json.name("fluids");
        json.beginObject();
        for (Fluid fluid : BuiltInRegistries.FLUID) {
            ResourceLocation id = BuiltInRegistries.FLUID.getKey(fluid);
            if (!ours.test(id) || fluid == Fluids.EMPTY) continue;
            // One fluid to a buffer, so one whose facts throw is left out whole.
            JsonWriter out = json;
            StringWriter buffer = new StringWriter();
            json = new JsonWriter(buffer);
            try {
                FluidType type = fluid.getFluidType();
                fluid(fluid, type);
                ResourceLocation typeId = NeoForgeRegistries.FLUID_TYPES.getKey(type);
                if (typeId != null) types.put(typeId.toString(), type);
                json.flush();
                json = out;
                json.name(id.toString()).jsonValue(buffer.toString());
            } catch (RuntimeException | ReflectiveOperationException e) {
                json = out;
                System.out.println("horizon-oracle: fluid left out " + id + ": " + e);
            }
        }
        json.endObject();
        // The types in the namespaces too, with no fluid of theirs.
        for (FluidType type : NeoForgeRegistries.FLUID_TYPES) {
            ResourceLocation id = NeoForgeRegistries.FLUID_TYPES.getKey(type);
            if (id != null && ours.test(id)) types.put(id.toString(), type);
        }
        json.name("fluid_types");
        json.beginObject();
        for (var e : types.entrySet()) {
            JsonWriter out = json;
            StringWriter buffer = new StringWriter();
            json = new JsonWriter(buffer);
            try {
                type(e.getValue());
                json.flush();
                json = out;
                json.name(e.getKey()).jsonValue(buffer.toString());
            } catch (RuntimeException ex) {
                json = out;
                System.out.println("horizon-oracle: fluid type left out " + e.getKey() + ": " + ex);
            }
        }
        json.endObject();
        // The dimensions a type's isVaporizedOnPlacement is asked in, and which are ultraWarm (where vanilla's water
        // evaporates): what tells a type that follows water from one that vaporizes everywhere or nowhere.
        json.name("ultrawarm");
        json.beginObject();
        for (ServerLevel l : level.getServer().getAllLevels()) {
            json.name(l.dimension().location().toString()).value(l.dimensionType().ultraWarm());
        }
        json.endObject();
        cauldrons();
    }

    /** NeoForge's cauldron fluid contents of the namespaces' blocks ({@code CauldronFluidContent.getForBlock}: vanilla's
     *  three and what mods registered by {@code RegisterCauldronFluidContentEvent}), each with its fluid, total amount
     *  and levels. */
    private void cauldrons() throws IOException {
        json.name("cauldrons");
        json.beginObject();
        for (Block block : BuiltInRegistries.BLOCK) {
            ResourceLocation id = BuiltInRegistries.BLOCK.getKey(block);
            CauldronFluidContent content = ours.test(id) ? CauldronFluidContent.getForBlock(block) : null;
            if (content == null) continue;
            json.name(id.toString());
            json.beginObject();
            json.name("fluid").value(key(content.fluid));
            json.name("total_amount").value(content.totalAmount);
            json.name("max_level").value(content.maxLevel);
            json.name("level_property").value(content.levelProperty == null ? null : content.levelProperty.getName());
            json.endObject();
        }
        json.endObject();
    }

    private void fluid(Fluid fluid, FluidType type) throws IOException, ReflectiveOperationException {
        json.beginObject();
        json.name("class");
        json.beginArray();
        for (Class<?> c = fluid.getClass(); c != null && c != Object.class; c = c.getSuperclass()) json.value(c.getName());
        json.endArray();
        ResourceLocation typeId = NeoForgeRegistries.FLUID_TYPES.getKey(type);
        json.name("type").value(typeId == null ? null : typeId.toString());
        FluidState state = fluid.defaultFluidState();
        json.name("is_source").value(fluid.isSource(state));
        if (fluid instanceof FlowingFluid flowing) {
            json.name("source").value(key(flowing.getSource()));
            json.name("flowing").value(key(flowing.getFlowing()));
            json.name("slope_find_distance").value((int) call(fluid, "getSlopeFindDistance", LevelReader.class));
            json.name("drop_off").value((int) call(fluid, "getDropOff", LevelReader.class));
            json.name("can_convert_to_source").value(flowing.canConvertToSource(state, level, AT));
        }
        // The block a source of it is (createLegacyBlock): air when it has none.
        FluidState source = fluid instanceof FlowingFluid f ? f.getSource(false) : state;
        json.name("block").value(BuiltInRegistries.BLOCK.getKey(source.createLegacyBlock().getBlock()).toString());
        // What the light engine reads of that block in place (BlockLightEngine.getEmission: NeoForge's
        // getLightEmission(level, pos)); FluidType.getLightLevel reaches no light engine in NeoForge 1.21.1.
        if (!source.createLegacyBlock().isAir()) json.name("block_light").value(emission(source.createLegacyBlock()));
        json.name("bucket").value(BuiltInRegistries.ITEM.getKey(fluid.getBucket()).toString());
        json.name("tick_delay").value(fluid.getTickDelay(level));
        json.name("explosion_resistance").value((float) call(fluid, "getExplosionResistance"));
        json.name("pickup_sound").value(fluid.getPickupSound().map(FluidFacts::sound).orElse(null));
        json.endObject();
    }

    private void type(FluidType type) throws IOException {
        json.beginObject();
        json.name("class");
        json.beginArray();
        for (Class<?> c = type.getClass(); c != null && c != Object.class; c = c.getSuperclass()) json.value(c.getName());
        json.endArray();
        json.name("description").value(type.getDescriptionId());
        json.name("light_level").value(type.getLightLevel());
        json.name("density").value(type.getDensity());
        json.name("temperature").value(type.getTemperature());
        json.name("viscosity").value(type.getViscosity());
        json.name("rarity").value(type.getRarity().getSerializedName());
        json.name("lighter_than_air").value(type.isLighterThanAir());
        Fluid any = null;
        for (Fluid f : BuiltInRegistries.FLUID) {
            if (f.getFluidType() == type && f.isSource(f.defaultFluidState())) {
                any = f;
                break;
            }
        }
        if (any != null) {
            FluidStack stack = new FluidStack(any, FluidType.BUCKET_VOLUME);
            json.name("can_convert_to_source").value(type.canConvertToSource(stack));
            json.name("can_hydrate").value(type.canHydrate(stack));
            // isVaporizedOnPlacement(level, pos, a bucket of it) in each dimension, as BucketItem.emptyContents asks
            // it: by default in an ultraWarm one for water's type or a fluid in minecraft:water, but a mod's type
            // may say otherwise (Mekanism's never, Create Dragons Plus' dyes wherever water evaporates).
            json.name("vaporized_on_placement");
            json.beginObject();
            for (ServerLevel l : level.getServer().getAllLevels()) {
                boolean vaporized;
                try {
                    vaporized = type.isVaporizedOnPlacement(l, AT, stack);
                } catch (RuntimeException e) {
                    continue;
                }
                json.name(l.dimension().location().toString()).value(vaporized);
            }
            json.endObject();
        }
        json.name("sounds");
        json.beginObject();
        sound("bucket_fill", type.getSound(SoundActions.BUCKET_FILL));
        sound("bucket_empty", type.getSound(SoundActions.BUCKET_EMPTY));
        sound("fluid_vaporize", type.getSound(SoundActions.FLUID_VAPORIZE));
        sound("cauldron_drip", type.getSound(SoundActions.CAULDRON_DRIP));
        json.endObject();
        entity(type);
        // Slice 7b: NeoForge's dripstone hook (FluidType.getDripInfo, which PointedDripstoneBlock asks of the fluid
        // above a stalactite), and what a source of it does to blocks, boats and dropped items.
        FluidType.DripstoneDripInfo drip = type.getDripInfo();
        if (drip != null) {
            json.name("drip");
            json.beginObject();
            json.name("chance").value(drip.chance());
            json.name("particle").value(drip.dripParticle() == null
                    ? null
                    : BuiltInRegistries.PARTICLE_TYPE.getKey(drip.dripParticle().getType()).toString());
            json.name("cauldron").value(BuiltInRegistries.BLOCK.getKey(drip.filledCauldron()).toString());
            json.endObject();
        }
        if (any != null) world(type, any.defaultFluidState());
        json.endObject();
    }

    /** {@code canHydrate(FluidState, getter, pos, source, sourcePos)} for farmland, a sponge and a coral block (what
     *  FarmBlock.isNearWater, SpongeBlock and CoralBlock ask through canBeHydrated); {@code supportsBoating(FluidState,
     *  boat)} (Boat's canBoatInFluid); {@code setItemMovement} on a dropped item, measured. */
    private void world(FluidType type, FluidState state) throws IOException {
        json.name("hydrates");
        json.beginObject();
        Map<String, Block> sources = new java.util.LinkedHashMap<>();
        sources.put("farmland", Blocks.FARMLAND);
        sources.put("sponge", Blocks.SPONGE);
        sources.put("coral", Blocks.TUBE_CORAL_BLOCK);
        for (var e : sources.entrySet()) {
            boolean hydrates;
            try {
                hydrates = type.canHydrate(state, level, AT, e.getValue().defaultBlockState(), AT.below());
            } catch (RuntimeException ex) {
                continue;
            }
            json.name(e.getKey()).value(hydrates);
        }
        json.endObject();
        Boolean boating = null;
        try {
            Boat boat = EntityType.BOAT.create(level);
            if (boat != null) boating = type.supportsBoating(state, boat);
        } catch (RuntimeException e) {
            // Left out.
        }
        if (boating != null) json.name("boating").value(boating);
        itemMovement(type);
    }

    /** {@code FluidType.setItemMovement} measured on a dropped item: NeoForge's default (water's {@code x * 0.99F,
     *  y + (y < 0.06F ? 5.0E-4F : 0), z * 0.99F}) or a mod's own. Written as that form's numbers when every probe fits
     *  it (the horizontal scale, what is added below 0.06F and from there up), else as the probes. */
    private void itemMovement(FluidType type) throws IOException {
        ItemEntity item;
        try {
            item = new ItemEntity(level, AT.getX() + 0.5, AT.getY(), AT.getZ() + 0.5, new ItemStack(Items.STONE));
        } catch (RuntimeException e) {
            return;
        }
        double[][] probes = {{1, 0, 1}, {0.25, 0.0625, -1}, {0.5, -0.5, 0.5}, {1, 0.5, 1}};
        Vec3[] out = new Vec3[probes.length];
        try {
            for (int i = 0; i < probes.length; i++) {
                item.setDeltaMovement(probes[i][0], probes[i][1], probes[i][2]);
                type.setItemMovement(item);
                out[i] = item.getDeltaMovement();
            }
        } catch (RuntimeException e) {
            return;
        }
        double h = out[0].x, below = out[0].y, above = snap(out[1].y - 0.0625, out[1].y, out[3].y);
        boolean fits = out[0].z == h
                && out[1].x == 0.25 * h && out[1].z == -h && out[1].y == 0.0625 + above
                && out[2].x == 0.5 * h && out[2].z == 0.5 * h && out[2].y == -0.5 + below
                && out[3].x == h && out[3].y == 0.5 + above;
        json.name("item_movement");
        json.beginObject();
        if (fits) {
            json.name("horizontal").value(h);
            json.name("below").value(below);
            json.name("above").value(above);
        } else {
            json.name("probes");
            json.beginArray();
            for (int i = 0; i < probes.length; i++) {
                json.beginArray();
                for (double v : probes[i]) json.value(v);
                json.value(out[i].x).value(out[i].y).value(out[i].z);
                json.endArray();
            }
            json.endArray();
        }
        json.endObject();
    }

    /** What is added from 0.06F up, from {@code y + it} at y = 0.0625 and 0.5: of the doubles a few ulps around the
     *  difference that give both sums, the one with the shortest decimal form (Java's own constant, as -0.04 for an
     *  item's gravity, rather than the difference's rounding). */
    private static double snap(double guess, double at0625, double at05) {
        double best = Double.NaN;
        double c = guess;
        for (int i = 0; i < 4; i++) c = Math.nextDown(c);
        for (int i = 0; i < 9; i++, c = Math.nextUp(c)) {
            if (0.0625 + c == at0625 && 0.5 + c == at05
                    && (Double.isNaN(best) || Double.toString(c).length() < Double.toString(best).length())) {
                best = c;
            }
        }
        return Double.isNaN(best) ? guess : best;
    }

    /** {@code state.getLightEmission(level, pos)} with it written at {@link #AT} (no onPlace), then the old block. */
    private int emission(BlockState state) {
        LevelChunk chunk = level.getChunkAt(AT);
        var section = chunk.getSection(chunk.getSectionIndex(AT.getY()));
        BlockState old = section.setBlockState(AT.getX() & 15, AT.getY() & 15, AT.getZ() & 15, state);
        try {
            return state.getLightEmission(level, AT);
        } finally {
            section.setBlockState(AT.getX() & 15, AT.getY() & 15, AT.getZ() & 15, old);
        }
    }

    /** What the type does to a pig in it and a boat on it (FluidType's entity methods; slice 4 reads them). */
    private void entity(FluidType type) throws IOException {
        LivingEntity pig;
        Boat boat;
        try {
            pig = EntityType.PIG.create(level);
            boat = EntityType.BOAT.create(level);
        } catch (RuntimeException e) {
            return;
        }
        if (pig == null || boat == null) return;
        json.name("entity");
        json.beginObject();
        try {
            json.name("can_swim").value(type.canSwim(pig));
            json.name("can_drown").value(type.canDrownIn(pig));
            json.name("can_push").value(type.canPushEntity(pig));
            json.name("motion_scale").value(type.motionScale(pig));
            json.name("fall_distance_modifier").value(type.getFallDistanceModifier(pig));
            json.name("can_extinguish").value(type.canExtinguish(pig));
            json.name("can_hydrate").value(type.canHydrate(pig));
            json.name("supports_boating").value(type.supportsBoating(boat));
        } catch (RuntimeException e) {
            // A mod's method that wants more than a pig: what it said before is kept.
        }
        json.endObject();
    }

    private void sound(String name, SoundEvent event) throws IOException {
        if (event != null) json.name(name).value(sound(event));
    }

    private static String sound(SoundEvent event) {
        return event.getLocation().toString();
    }

    private static String key(Fluid fluid) {
        return BuiltInRegistries.FLUID.getKey(fluid).toString();
    }

    /** A protected {@code FlowingFluid}/{@code Fluid} method, the most derived override, on this level. */
    private Object call(Fluid fluid, String name, Class<?>... params) throws ReflectiveOperationException {
        for (Class<?> c = fluid.getClass(); c != null; c = c.getSuperclass()) {
            try {
                Method m = c.getDeclaredMethod(name, params);
                m.setAccessible(true);
                return params.length == 0 ? m.invoke(fluid) : m.invoke(fluid, level);
            } catch (NoSuchMethodException e) {
                // Declared further up.
            }
        }
        throw new NoSuchMethodException(name);
    }
}
