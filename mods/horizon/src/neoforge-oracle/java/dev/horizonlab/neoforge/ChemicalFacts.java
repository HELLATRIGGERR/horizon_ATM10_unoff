package dev.horizonlab.neoforge;

import com.google.gson.stream.JsonWriter;
import java.io.IOException;
import java.io.StringWriter;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.lang.reflect.RecordComponent;
import java.util.Collection;
import java.util.Map;
import java.util.TreeMap;
import java.util.TreeSet;
import net.minecraft.core.DefaultedRegistry;
import net.minecraft.core.Holder;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.tags.TagKey;

/**
 * Mekanism's chemicals (docs/mekanism-chemicals.md): every entry of the {@code mekanism:chemical} registry as the
 * server holds it once the data packs are loaded, so their data maps and tags are final. Mekanism is not on the
 * oracle's class path, so each {@code Chemical} is read by reflection: its icon ({@code getIcon}), tint ({@code
 * getTint}) and bar colour ({@code getColorRepresentation}), translation key, radioactivity, whether a default tank refuses it ({@code hasAttributesWithValidation}),
 * {@code isGaseous} (the builder's flag or the {@code mekanism:gaseous} tag) and the builder's flag alone, the solid
 * tag of a slurry ({@code getOreTag}), its attributes ({@code getAttributes}: the builder's and the data maps', each
 * as its modern record when it has one: {@code ChemicalFuel}, {@code ChemicalRadioactivity}, {@code CooledCoolant},
 * {@code HeatedCoolant}) and its tags. Nothing is written when the pack has no Mekanism.
 */
final class ChemicalFacts {
    private static final ResourceLocation REGISTRY = ResourceLocation.fromNamespaceAndPath("mekanism", "chemical");
    private JsonWriter json;

    ChemicalFacts(JsonWriter json) {
        this.json = json;
    }

    void write() throws IOException {
        Registry<?> registry = BuiltInRegistries.REGISTRY.get(REGISTRY);
        json.name("chemicals");
        json.beginObject();
        if (registry != null) writeAll(registry);
        json.endObject();
    }

    private <T> void writeAll(Registry<T> registry) throws IOException {
        // The registry's default, the empty chemical, is no chemical of the pack.
        ResourceLocation empty = registry instanceof DefaultedRegistry<?> d
                ? d.getDefaultKey()
                : ResourceLocation.fromNamespaceAndPath("mekanism", "empty");
        Map<String, T> sorted = new TreeMap<>();
        for (Map.Entry<ResourceKey<T>, T> e : registry.entrySet()) {
            sorted.put(e.getKey().location().toString(), e.getValue());
        }
        for (Map.Entry<String, T> e : sorted.entrySet()) {
            if (empty != null && e.getKey().equals(empty.toString())) continue;
            // One chemical to a buffer, so one whose facts throw is left out whole.
            JsonWriter out = json;
            StringWriter buffer = new StringWriter();
            json = new JsonWriter(buffer);
            try {
                chemical(registry, e.getValue());
                json.flush();
                json = out;
                json.name(e.getKey()).jsonValue(buffer.toString());
            } catch (RuntimeException | ReflectiveOperationException ex) {
                json = out;
                System.out.println("horizon-oracle: chemical left out " + e.getKey() + ": " + ex);
            }
        }
    }

    private <T> void chemical(Registry<T> registry, T chemical) throws IOException, ReflectiveOperationException {
        json.beginObject();
        json.name("class").value(chemical.getClass().getName());
        json.name("texture").value(String.valueOf(call(chemical, "getIcon")));
        json.name("tint").value(((Number) call(chemical, "getTint")).intValue());
        // A bar's colour (ChemicalUtil.chemical overrides it for bio and fungi).
        json.name("color").value(((Number) call(chemical, "getColorRepresentation")).intValue());
        json.name("translation_key").value(String.valueOf(call(chemical, "getTranslationKey")));
        json.name("radioactivity").value(((Number) call(chemical, "getRadioactivity")).doubleValue());
        json.name("validated").value((Boolean) call(chemical, "hasAttributesWithValidation"));
        json.name("gaseous").value((Boolean) call(chemical, "isGaseous"));
        json.name("gaseous_builder").value((Boolean) call(chemical, "isGaseousLegacy"));
        Object ore = call(chemical, "getOreTag");
        json.name("solid_tag");
        if (ore instanceof TagKey<?> tag) json.value(tag.location().toString());
        else json.nullValue();
        json.name("attributes");
        json.beginArray();
        Object attributes = call(chemical, "getAttributes");
        if (attributes instanceof Collection<?> list) {
            // A stable order: by the attribute's kind.
            Map<String, Object> byKind = new TreeMap<>();
            for (Object legacy : list) {
                Object modern = call(legacy, "asModern");
                Object attribute = modern != null ? modern : legacy;
                byKind.put(attribute.getClass().getSimpleName(), attribute);
            }
            for (Map.Entry<String, Object> a : byKind.entrySet()) attribute(a.getKey(), a.getValue());
        }
        json.endArray();
        Holder<T> holder = registry.wrapAsHolder(chemical);
        TreeSet<String> tags = new TreeSet<>();
        holder.tags().forEach(t -> tags.add(t.location().toString()));
        json.name("tags");
        json.beginArray();
        for (String tag : tags) json.value(tag);
        json.endArray();
        json.endObject();
    }

    /** A record's components by name (a holder as its registered name), or a legacy attribute's class alone. */
    private void attribute(String kind, Object attribute) throws IOException, ReflectiveOperationException {
        json.beginObject();
        json.name("kind").value(kind);
        RecordComponent[] components = attribute.getClass().getRecordComponents();
        if (components != null) {
            for (RecordComponent c : components) {
                Object value = c.getAccessor().invoke(attribute);
                json.name(c.getName());
                if (value instanceof Holder<?> h) json.value(h.getRegisteredName());
                else if (value instanceof Number n) json.value(n);
                else if (value instanceof Boolean b) json.value(b);
                else json.value(String.valueOf(value));
            }
        }
        json.endObject();
    }

    /** {@code target.name()}, looked up on its first public class: Mekanism's bio and fungi are anonymous classes. */
    private static Object call(Object target, String name) throws ReflectiveOperationException {
        Class<?> owner = target.getClass();
        while (owner != null && !Modifier.isPublic(owner.getModifiers())) owner = owner.getSuperclass();
        Method method = (owner != null ? owner : target.getClass()).getMethod(name);
        return method.invoke(target);
    }
}
