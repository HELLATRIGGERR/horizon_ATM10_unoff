package dev.horizonlab.neoforge;

import com.google.gson.stream.JsonWriter;
import java.io.IOException;
import java.io.StringWriter;
import java.util.function.Predicate;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityDimensions;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.ai.attributes.Attribute;
import net.minecraft.world.entity.ai.attributes.AttributeSupplier;
import net.minecraft.world.entity.ai.attributes.DefaultAttributes;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.SpawnEggItem;

/**
 * The entity types of the export's namespaces, and vanilla's so templates can be compared (docs/mobs.md, slice 2):
 * each type's {@code MobCategory}, size ({@code EntityDimensions}: width, height, eye height), {@code fireImmune},
 * the class of an instance and its superclasses ({@code EntityType.create} on the overworld, never added to it), and
 * a living type's default attributes ({@code DefaultAttributes}, which NeoForge's {@code EntityAttributeCreationEvent}
 * fills). Then each spawn egg of the namespaces with the type it spawns ({@code SpawnEggItem.getType}).
 */
final class EntityFacts {
    private final ServerLevel level;
    private final Predicate<ResourceLocation> ours;
    private final JsonWriter json;

    EntityFacts(ServerLevel level, Predicate<ResourceLocation> ours, JsonWriter json) {
        this.level = level;
        this.ours = ours;
        this.json = json;
    }

    void write() throws IOException {
        json.name("entity_types");
        json.beginObject();
        for (EntityType<?> type : BuiltInRegistries.ENTITY_TYPE) {
            ResourceLocation id = BuiltInRegistries.ENTITY_TYPE.getKey(type);
            if (!ours.test(id) && !id.getNamespace().equals("minecraft")) continue;
            StringWriter buffer = new StringWriter();
            try {
                type(type, new JsonWriter(buffer));
            } catch (Throwable e) {
                System.out.println("horizon-oracle: left out entity type " + id + ": " + e);
                continue;
            }
            json.name(id.toString()).jsonValue(buffer.toString());
        }
        json.endObject();
        json.name("spawn_eggs");
        json.beginObject();
        for (Item item : BuiltInRegistries.ITEM) {
            ResourceLocation id = BuiltInRegistries.ITEM.getKey(item);
            if (!(item instanceof SpawnEggItem egg) || !ours.test(id)) continue;
            try {
                EntityType<?> type = egg.getType(new ItemStack(egg));
                json.name(id.toString()).value(BuiltInRegistries.ENTITY_TYPE.getKey(type).toString());
            } catch (Throwable e) {
                System.out.println("horizon-oracle: left out spawn egg " + id + ": " + e);
            }
        }
        json.endObject();
    }

    @SuppressWarnings("unchecked")
    private void type(EntityType<?> type, JsonWriter out) throws IOException {
        out.beginObject();
        out.name("category").value(type.getCategory().getName());
        EntityDimensions size = type.getDimensions();
        out.name("width").value(size.width());
        out.name("height").value(size.height());
        out.name("eye_height").value(size.eyeHeight());
        out.name("fire_immune").value(type.fireImmune());
        Entity entity = null;
        try {
            entity = type.create(level);
        } catch (Throwable e) {
            out.name("create_failed").value(e.toString());
        }
        if (entity != null) {
            out.name("class");
            out.beginArray();
            for (Class<?> c = entity.getClass(); c != null && c != Object.class; c = c.getSuperclass()) {
                out.value(c.getName());
            }
            out.endArray();
            out.name("living").value(entity instanceof LivingEntity);
        }
        if (DefaultAttributes.hasSupplier(type)) {
            AttributeSupplier supplier = DefaultAttributes.getSupplier((EntityType<? extends LivingEntity>) type);
            out.name("attributes");
            out.beginObject();
            for (Attribute attribute : BuiltInRegistries.ATTRIBUTE) {
                Holder<Attribute> holder = BuiltInRegistries.ATTRIBUTE.wrapAsHolder(attribute);
                if (supplier.hasAttribute(holder)) {
                    out.name(BuiltInRegistries.ATTRIBUTE.getKey(attribute).toString())
                            .value(supplier.getBaseValue(holder));
                }
            }
            out.endObject();
        }
        out.endObject();
        out.flush();
    }
}
