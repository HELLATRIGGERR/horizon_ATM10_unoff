package dev.horizonlab.neoforge;

import com.google.gson.stream.JsonWriter;
import java.io.IOException;
import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.Property;
import net.minecraft.world.level.chunk.LevelChunk;
import net.minecraft.world.level.chunk.LevelChunkSection;
import net.minecraft.world.level.material.FlowingFluid;
import net.minecraft.world.level.material.Fluid;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.level.material.Fluids;
import net.neoforged.neoforge.fluids.FluidInteractionRegistry;
import net.neoforged.neoforge.fluids.FluidType;
import net.neoforged.neoforge.registries.NeoForgeRegistries;

/**
 * NeoForge's {@code FluidInteractionRegistry} as the pack fills it (docs/mod-fluids.md, slice 7b): for each fluid type
 * with interactions, each {@code InteractionInformation} in its list's order, run against a grid of worlds instead of
 * read from its lambdas. The current fluid (a source, and a flowing fluid of amount 4) sits at {@link #AT}, a
 * neighbour at the block {@code canInteract} asks ({@code relativePos}, north here), and a block below it; each
 * predicate is asked of every combination of the neighbours (air, a few blocks the pack's interaction code names, and
 * every fluid of the registry by its liquid block) and the blocks below. What matches is written as the blocks below
 * (none: any) and the neighbours that match, with whether the matches are exactly every pair of the two (what a rule of
 * "this below and one of these beside" can hold). Then the interaction is run once for each current fluid state
 * (source, flowing amounts 1 to 7, falling) on a matching world, and the block it leaves at {@code currentPos} is read
 * back. The worlds are written straight into the chunk section (no {@code onPlace}, which would run the interactions
 * itself) and cleared after; a run that changes a block the grid didn't place (a mod's own effect) is written as such.
 */
final class FluidInteractionFacts {
    /** Above the overworld's spawn, clear of the other facts' spot. */
    private static final BlockPos AT = new BlockPos(8, 120, 8);
    /** The blocks the interactions in ATM10 8.4's code name beside or below a fluid (vanilla's basalt, the Aether's
     *  holystone, Sauce's sourcestone, XyCraft's water block), with stone and air as the rest of the world. */
    private static final String[] CANDIDATES = {
        "minecraft:stone", "minecraft:air", "minecraft:soul_soil", "minecraft:blue_ice", "minecraft:magma_block",
        "aether:quicksoil", "xycraft_machines:water_block",
    };
    private static final String NEOFORGE_INTERACTION =
            "net.neoforged.neoforge.fluids.FluidInteractionRegistry$InteractionInformation";

    private final ServerLevel level;
    private JsonWriter json;
    private final LevelChunk chunk;

    FluidInteractionFacts(ServerLevel level, JsonWriter json) {
        this.level = level;
        this.json = json;
        this.chunk = level.getChunkAt(AT);
    }

    @SuppressWarnings("unchecked")
    private static Map<FluidType, List<FluidInteractionRegistry.InteractionInformation>> registry()
            throws ReflectiveOperationException {
        Field f = FluidInteractionRegistry.class.getDeclaredField("INTERACTIONS");
        try {
            f.setAccessible(true);
            return (Map<FluidType, List<FluidInteractionRegistry.InteractionInformation>>) f.get(null);
        } catch (RuntimeException e) {
            Field uf = sun.misc.Unsafe.class.getDeclaredField("theUnsafe");
            uf.setAccessible(true);
            sun.misc.Unsafe u = (sun.misc.Unsafe) uf.get(null);
            return (Map<FluidType, List<FluidInteractionRegistry.InteractionInformation>>)
                    u.getObject(u.staticFieldBase(f), u.staticFieldOffset(f));
        }
    }

    /** A neighbour of the grid: a block, or a fluid placed as its liquid block. */
    private record Neighbour(String name, boolean fluid, BlockState state) {}

    void write() throws IOException, ReflectiveOperationException {
        Map<FluidType, List<FluidInteractionRegistry.InteractionInformation>> registry = registry();
        Map<String, FluidType> types = new TreeMap<>();
        for (FluidType type : registry.keySet()) {
            ResourceLocation id = NeoForgeRegistries.FLUID_TYPES.getKey(type);
            if (id != null) types.put(id.toString(), type);
        }
        List<BlockState> belows = new ArrayList<>();
        List<String> belowNames = new ArrayList<>();
        List<Neighbour> neighbours = new ArrayList<>();
        for (String name : CANDIDATES) {
            Block block = BuiltInRegistries.BLOCK.getOptional(ResourceLocation.parse(name)).orElse(null);
            if (block == null) continue;
            belows.add(block.defaultBlockState());
            belowNames.add(name);
            neighbours.add(new Neighbour(name, false, block.defaultBlockState()));
        }
        for (Fluid fluid : BuiltInRegistries.FLUID) {
            if (fluid == Fluids.EMPTY) continue;
            FluidState state = fluid.defaultFluidState();
            if (!fluid.isSource(state) && fluid instanceof FlowingFluid f) {
                try {
                    state = f.getFlowing(4, false);
                } catch (RuntimeException e) {
                    // A flowing fluid without FlowingFluid's level (Create's potion): its default state.
                }
            }
            BlockState block;
            try {
                block = state.createLegacyBlock();
            } catch (RuntimeException e) {
                continue;
            }
            if (block.isAir() || block.getFluidState().getType() != fluid) continue;
            neighbours.add(new Neighbour(BuiltInRegistries.FLUID.getKey(fluid).toString(), true, block));
        }
        json.name("interactions");
        json.beginObject();
        for (var e : types.entrySet()) {
            List<FluidInteractionRegistry.InteractionInformation> list = registry.get(e.getValue());
            FlowingFluid current = currentFluid(e.getValue());
            json.name(e.getKey());
            json.beginArray();
            for (int i = 0; i < list.size(); i++) {
                // One interaction to a buffer, so one that throws is written as its error alone.
                JsonWriter out = json;
                java.io.StringWriter buffer = new java.io.StringWriter();
                json = new JsonWriter(buffer);
                try {
                    interaction(i, list.get(i), current, belows, belowNames, neighbours);
                    json.flush();
                    json = out;
                    json.jsonValue(buffer.toString());
                } catch (RuntimeException ex) {
                    json = out;
                    json.beginObject();
                    json.name("index").value(i);
                    json.name("error").value(ex.toString());
                    json.endObject();
                    System.out.println("horizon-oracle: interaction " + e.getKey() + " " + i + ": " + ex);
                }
                clear();
            }
            json.endArray();
        }
        json.endObject();
    }

    /** The first flowing fluid of the type whose source has a liquid block: what the grid puts at {@link #AT}. */
    private static FlowingFluid currentFluid(FluidType type) {
        for (Fluid fluid : BuiltInRegistries.FLUID) {
            if (fluid.getFluidType() == type && fluid instanceof FlowingFluid f && fluid.isSource(fluid.defaultFluidState())
                    && !f.getSource(false).createLegacyBlock().isAir()) {
                return f;
            }
        }
        return null;
    }

    private void interaction(int index, FluidInteractionRegistry.InteractionInformation info, FlowingFluid current,
            List<BlockState> belows, List<String> belowNames, List<Neighbour> neighbours) throws IOException {
        json.beginObject();
        json.name("index").value(index);
        json.name("predicate").value(declaring(info.predicate()));
        String effect = declaring(info.interaction());
        json.name("interaction").value(effect);
        // NeoForge's own: setBlockAndUpdate(currentPos, fireFluidPlaceBlockEvent(getState(currentState))) and the
        // fizz (levelEvent 1501).
        json.name("neoforge_interaction").value(effect.equals(NEOFORGE_INTERACTION));
        if (current == null) {
            json.name("current").nullValue();
            json.endObject();
            return;
        }
        json.name("current").value(BuiltInRegistries.FLUID.getKey(current.getSource()).toString());
        FluidState[] kinds = {current.getSource(false), current.getFlowing(4, false)};
        String[] kindNames = {"source", "flowing"};
        int[][] firstMatch = new int[2][];
        for (int k = 0; k < 2; k++) {
            boolean[][] m = new boolean[belows.size()][neighbours.size()];
            for (int b = 0; b < belows.size(); b++) {
                for (int n = 0; n < neighbours.size(); n++) {
                    place(belows.get(b), neighbours.get(n).state(), kinds[k].createLegacyBlock());
                    m[b][n] = info.predicate().test(level, AT, AT.north(), kinds[k]);
                    if (m[b][n] && firstMatch[k] == null) firstMatch[k] = new int[] {b, n};
                }
            }
            json.name(kindNames[k]);
            grid(m, belowNames, neighbours);
        }
        // The block the interaction leaves for each current state, on the first world that matched for its kind.
        json.name("results");
        json.beginObject();
        Map<String, FluidState> states = new LinkedHashMap<>();
        states.put("source", current.getSource(false));
        for (int amount = 1; amount < 8; amount++) states.put(String.valueOf(amount), current.getFlowing(amount, false));
        states.put("falling", current.getFlowing(8, true));
        for (var s : states.entrySet()) {
            int[] w = firstMatch[s.getValue().isSource() ? 0 : 1];
            if (w == null) continue;
            place(belows.get(w[0]), neighbours.get(w[1]).state(), s.getValue().createLegacyBlock());
            info.interaction().interact(level, AT, AT.north(), s.getValue());
            json.name(s.getKey()).value(state(level.getBlockState(AT)));
            // A mod's interaction that changes more than the current block is written so (its own effect).
            if (level.getBlockState(AT.below()) != belows.get(w[0]) || level.getBlockState(AT.north()) != neighbours.get(w[1]).state()) {
                json.name(s.getKey() + "_changes_neighbours").value(true);
            }
        }
        json.endObject();
        json.endObject();
    }

    /** The matches of one current fluid kind: the blocks below that match some neighbour (none when all do), the
     *  matching neighbours (fluids and blocks), and whether the matches are exactly every pair of the two. */
    private void grid(boolean[][] m, List<String> belowNames, List<Neighbour> neighbours) throws IOException {
        boolean[] rows = new boolean[m.length];
        boolean[] cols = new boolean[neighbours.size()];
        for (int b = 0; b < m.length; b++) {
            for (int n = 0; n < cols.length; n++) {
                if (m[b][n]) {
                    rows[b] = true;
                    cols[n] = true;
                }
            }
        }
        boolean exact = true;
        boolean allRows = true;
        for (int b = 0; b < m.length; b++) {
            allRows &= rows[b];
            for (int n = 0; n < cols.length; n++) exact &= m[b][n] == (rows[b] && cols[n]);
        }
        json.beginObject();
        json.name("exact").value(exact);
        json.name("below");
        if (allRows) {
            json.nullValue();
        } else {
            json.beginArray();
            for (int b = 0; b < m.length; b++) if (rows[b]) json.value(belowNames.get(b));
            json.endArray();
        }
        json.name("fluids");
        json.beginArray();
        for (int n = 0; n < cols.length; n++) if (cols[n] && neighbours.get(n).fluid()) json.value(neighbours.get(n).name());
        json.endArray();
        json.name("blocks");
        json.beginArray();
        for (int n = 0; n < cols.length; n++) if (cols[n] && !neighbours.get(n).fluid()) json.value(neighbours.get(n).name());
        json.endArray();
        json.endObject();
    }

    /** The world of one grid point, written into the section: the block below, the neighbour north, the current fluid
     *  at {@link #AT}, air on its other sides. */
    private void place(BlockState below, BlockState neighbour, BlockState current) {
        set(AT.below(), below);
        set(AT.north(), neighbour);
        set(AT, current);
        BlockState air = Blocks.AIR.defaultBlockState();
        set(AT.above(), air);
        set(AT.south(), air);
        set(AT.east(), air);
        set(AT.west(), air);
    }

    private void clear() {
        place(Blocks.AIR.defaultBlockState(), Blocks.AIR.defaultBlockState(), Blocks.AIR.defaultBlockState());
    }

    private void set(BlockPos pos, BlockState state) {
        LevelChunkSection section = chunk.getSection(chunk.getSectionIndex(pos.getY()));
        section.setBlockState(pos.getX() & 15, pos.getY() & 15, pos.getZ() & 15, state);
    }

    /** The class a lambda was declared in (its hidden class's name up to {@code $$Lambda}). */
    private static String declaring(Object lambda) {
        String name = lambda.getClass().getName();
        int cut = name.indexOf("$$Lambda");
        return cut < 0 ? name : name.substring(0, cut);
    }

    /** {@code id}, or {@code id[property=value,...]} for a state that isn't its block's default. */
    static String state(BlockState state) {
        String id = BuiltInRegistries.BLOCK.getKey(state.getBlock()).toString();
        if (state == state.getBlock().defaultBlockState()) return id;
        StringBuilder out = new StringBuilder(id).append('[');
        boolean first = true;
        for (Property<?> p : state.getProperties()) {
            if (!first) out.append(',');
            first = false;
            out.append(p.getName()).append('=').append(value(state, p));
        }
        return out.append(']').toString();
    }

    private static <T extends Comparable<T>> String value(BlockState state, Property<T> p) {
        return p.getName(state.getValue(p));
    }
}
