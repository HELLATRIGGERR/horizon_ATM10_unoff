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
import net.minecraft.world.entity.vehicle.Boat;
import net.minecraft.world.level.LevelReader;
import net.minecraft.world.level.material.FlowingFluid;
import net.minecraft.world.level.material.Fluid;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.level.material.Fluids;
import net.neoforged.neoforge.common.SoundActions;
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
        }
        json.name("sounds");
        json.beginObject();
        sound("bucket_fill", type.getSound(SoundActions.BUCKET_FILL));
        sound("bucket_empty", type.getSound(SoundActions.BUCKET_EMPTY));
        sound("fluid_vaporize", type.getSound(SoundActions.FLUID_VAPORIZE));
        json.endObject();
        entity(type);
        json.endObject();
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
