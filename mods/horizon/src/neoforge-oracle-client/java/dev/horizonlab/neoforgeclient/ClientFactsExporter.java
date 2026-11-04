package dev.horizonlab.neoforgeclient;

import com.google.gson.stream.JsonWriter;
import java.io.IOException;
import java.io.StringWriter;
import java.io.Writer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import com.mojang.blaze3d.shaders.FogShape;
import com.mojang.blaze3d.systems.RenderSystem;
import net.minecraft.client.Camera;
import net.minecraft.client.Minecraft;
import net.minecraft.client.color.item.ItemColors;
import net.minecraft.client.renderer.FogRenderer;
import net.minecraft.client.renderer.ItemBlockRenderTypes;
import net.minecraft.client.renderer.RenderType;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.material.Fluid;
import net.minecraft.world.level.material.Fluids;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.bus.api.IEventBus;
import net.neoforged.fml.common.Mod;
import net.neoforged.fml.event.lifecycle.FMLLoadCompleteEvent;
import net.neoforged.neoforge.client.extensions.common.IClientFluidTypeExtensions;
import net.neoforged.neoforge.fluids.FluidStack;
import net.neoforged.neoforge.fluids.FluidType;
import net.neoforged.neoforge.registries.NeoForgeRegistries;
import org.joml.Vector3f;

/**
 * Dev-only client half of the NeoForge oracle (docs/mod-fluids.md, slice 2): once every mod has loaded on the client
 * (its client extensions registered, its render layers set, its item colours registered), writes what only a client
 * holds of the pack's fluids to {@code horizon.oracle.client.out}, then halts the game: each {@code FluidType}'s
 * {@code IClientFluidTypeExtensions} (still, flowing and overlay textures, tint, the camera's overlay and what its
 * {@code modifyFogColor} and {@code modifyFogRender} make of the fog), each fluid's render layer
 * ({@code ItemBlockRenderTypes.getRenderLayer}) and each bucket's item colours ({@code ItemColors.getColor}, which
 * {@code DynamicFluidContainerModel.Colors} gives its fluid layer). Its window is never shown ({@code
 * HiddenWindowMixin}). Never shipped to players.
 */
@Mod(value = "horizon_oracle_client", dist = Dist.CLIENT)
public final class ClientFactsExporter {
    public ClientFactsExporter(IEventBus modBus) {
        modBus.addListener(FMLLoadCompleteEvent.class, event -> event.enqueueWork(ClientFactsExporter::export));
    }

    private static void export() {
        Path out = Path.of(System.getProperty("horizon.oracle.client.out", "horizon-oracle-client.json"));
        try (Writer w = Files.newBufferedWriter(out, StandardCharsets.UTF_8);
                JsonWriter json = new JsonWriter(w)) {
            json.beginObject();
            json.name("schema").value(1);
            json.name("fluid_types");
            json.beginObject();
            for (FluidType type : NeoForgeRegistries.FLUID_TYPES) {
                ResourceLocation id = NeoForgeRegistries.FLUID_TYPES.getKey(type);
                if (id == null) continue;
                String one = one(() -> type(type));
                if (one != null) json.name(id.toString()).jsonValue(one);
            }
            json.endObject();
            json.name("fluids");
            json.beginObject();
            for (Fluid fluid : BuiltInRegistries.FLUID) {
                if (fluid == Fluids.EMPTY) continue;
                String one = one(() -> fluid(fluid));
                if (one != null) json.name(BuiltInRegistries.FLUID.getKey(fluid).toString()).jsonValue(one);
            }
            json.endObject();
            json.name("items");
            json.beginObject();
            ItemColors colors = Minecraft.getInstance().getItemColors();
            java.util.Set<Item> buckets = new java.util.HashSet<>();
            for (Fluid fluid : BuiltInRegistries.FLUID) {
                // A source's bucket (its flowing fluid names the same one).
                if (!fluid.isSource(fluid.defaultFluidState())) continue;
                Item bucket = fluid.getBucket();
                if (bucket == Items.AIR || !buckets.add(bucket)) continue;
                String one = one(() -> {
                    ItemStack stack = new ItemStack(bucket);
                    StringWriter b = new StringWriter();
                    JsonWriter j = new JsonWriter(b);
                    j.beginObject();
                    j.name("fluid").value(BuiltInRegistries.FLUID.getKey(fluid).toString());
                    j.name("tints");
                    j.beginArray();
                    for (int i = 0; i < 3; i++) j.value(colors.getColor(stack, i));
                    j.endArray();
                    j.endObject();
                    j.flush();
                    return b.toString();
                });
                if (one != null) json.name(BuiltInRegistries.ITEM.getKey(bucket).toString()).jsonValue(one);
            }
            json.endObject();
            json.endObject();
        } catch (Throwable e) {
            e.printStackTrace();
            Runtime.getRuntime().halt(3);
        }
        System.out.println("horizon-oracle: wrote " + out);
        // Not Minecraft.stop: nothing more of the game is wanted, and the load would go on to the title screen.
        Runtime.getRuntime().halt(0);
    }

    private interface Part {
        String write() throws IOException;
    }

    /** One entry, or none when the mod's code throws for it (logged). */
    private static String one(Part part) {
        try {
            return part.write();
        } catch (IOException | RuntimeException e) {
            System.out.println("horizon-oracle: left out: " + e);
            return null;
        }
    }

    private static String type(FluidType type) throws IOException {
        IClientFluidTypeExtensions ext = IClientFluidTypeExtensions.of(type);
        StringWriter b = new StringWriter();
        JsonWriter j = new JsonWriter(b);
        j.beginObject();
        texture(j, "still", ext.getStillTexture());
        texture(j, "flowing", ext.getFlowingTexture());
        texture(j, "overlay", ext.getOverlayTexture());
        j.name("tint").value(ext.getTintColor());
        Fluid source = null;
        for (Fluid f : BuiltInRegistries.FLUID) {
            if (f.getFluidType() == type && f.isSource(f.defaultFluidState())) {
                source = f;
                break;
            }
        }
        if (source != null) {
            // What a stack of it shows (tanks, DynamicFluidContainerModel), which a mod may make differ.
            FluidStack stack = new FluidStack(source, FluidType.BUCKET_VOLUME);
            j.name("stack");
            j.beginObject();
            j.name("fluid").value(BuiltInRegistries.FLUID.getKey(source).toString());
            texture(j, "still", ext.getStillTexture(stack));
            texture(j, "flowing", ext.getFlowingTexture(stack));
            texture(j, "overlay", ext.getOverlayTexture(stack));
            j.name("tint").value(ext.getTintColor(stack));
            j.endObject();
        }
        try {
            // The camera's overlay under it: some mods' want a player, and there is none here.
            ResourceLocation overlay = ext.getRenderOverlayTexture(Minecraft.getInstance());
            texture(j, "render_overlay", overlay);
        } catch (RuntimeException e) {
            // Left out.
        }
        fog(j, ext);
        j.endObject();
        j.flush();
        return b.toString();
    }

    /**
     * What the type's client extension makes of the fog with the camera in it (docs/mod-fluids.md, slice 4):
     * {@code modifyFogColor} of a known colour (written when it gives another: Mekanism's gives its tint) and
     * {@code modifyFogRender} over the terrain at a 12-chunk render distance (written when it sets the shader's fog
     * start and end: Mekanism's, -8 to 24). There is no level or camera entity here: an extension that reads them
     * throws, and is left out. This runs on the render thread ({@code enqueueWork}), which the shader's fog wants.
     */
    private static void fog(JsonWriter j, IClientFluidTypeExtensions ext) throws IOException {
        try {
            Vector3f in = new Vector3f(0.25F, 0.5F, 0.75F);
            Vector3f out = ext.modifyFogColor(new Camera(), 0.0F, null, 12, 0.0F, new Vector3f(in));
            if (out != null && !out.equals(in)) {
                j.name("fog_color").beginArray().value(out.x()).value(out.y()).value(out.z()).endArray();
            }
        } catch (RuntimeException e) {
            System.out.println("horizon-oracle: fog colour left out: " + e);
        }
        float start = RenderSystem.getShaderFogStart();
        float end = RenderSystem.getShaderFogEnd();
        FogShape shape = RenderSystem.getShaderFogShape();
        try {
            float unset = -12345.0F;
            RenderSystem.setShaderFogStart(unset);
            RenderSystem.setShaderFogEnd(unset);
            ext.modifyFogRender(new Camera(), FogRenderer.FogMode.FOG_TERRAIN, 192.0F, 0.0F, 0.0F, 192.0F, FogShape.SPHERE);
            float s = RenderSystem.getShaderFogStart();
            float e = RenderSystem.getShaderFogEnd();
            if (s != unset && e != unset) {
                j.name("fog_range").beginArray().value(s).value(e).endArray();
            } else if (s != unset || e != unset) {
                System.out.println("horizon-oracle: fog range set only in part: " + s + " " + e);
            }
        } catch (RuntimeException e) {
            System.out.println("horizon-oracle: fog range left out: " + e);
        } finally {
            RenderSystem.setShaderFogStart(start);
            RenderSystem.setShaderFogEnd(end);
            RenderSystem.setShaderFogShape(shape);
        }
    }

    private static String fluid(Fluid fluid) throws IOException {
        StringWriter b = new StringWriter();
        JsonWriter j = new JsonWriter(b);
        j.beginObject();
        ResourceLocation type = NeoForgeRegistries.FLUID_TYPES.getKey(fluid.getFluidType());
        j.name("type").value(type == null ? null : type.toString());
        j.name("render_layer").value(layer(ItemBlockRenderTypes.getRenderLayer(fluid.defaultFluidState())));
        j.endObject();
        j.flush();
        return b.toString();
    }

    private static String layer(RenderType type) {
        if (type == RenderType.solid()) return "solid";
        if (type == RenderType.cutoutMipped()) return "cutout_mipped";
        if (type == RenderType.cutout()) return "cutout";
        if (type == RenderType.translucent()) return "translucent";
        if (type == RenderType.tripwire()) return "tripwire";
        return String.valueOf(type);
    }

    private static void texture(JsonWriter j, String name, ResourceLocation texture) throws IOException {
        j.name(name).value(texture == null ? null : texture.toString());
    }
}
