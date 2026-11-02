package dev.horizonlab.neoforge;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mojang.serialization.DynamicOps;
import com.mojang.serialization.JsonOps;
import java.io.BufferedWriter;
import java.io.IOException;
import java.io.Reader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.item.crafting.Recipe;
import net.minecraft.world.item.crafting.RecipeHolder;
import net.minecraft.world.level.storage.loot.LootTable;

/**
 * The pack's final data as the running server holds it, once every data pack, KubeJS script and Almost Unified
 * replacement has had its say: {@code recipes.jsonl}, {@code loot_tables.jsonl}, {@code tags.jsonl} (one JSON object
 * a line), {@code registries.json} (every id of every registry) and {@code summary.json}. Everything goes through
 * the game's own codecs; a recipe no codec can write falls back to its data pack file, and failing that is listed.
 */
final class PackDump {
    private final MinecraftServer server;
    private final Path dir;
    private final DynamicOps<JsonElement> ops;
    private final JsonObject summary = new JsonObject();

    PackDump(MinecraftServer server, Path dir) {
        this.server = server;
        this.dir = dir;
        this.ops = server.registryAccess().createSerializationContext(JsonOps.INSTANCE);
    }

    void run() throws IOException {
        Files.createDirectories(dir);
        recipes();
        lootTables();
        tags();
        registries();
        names();
        Files.writeString(dir.resolve("summary.json"), summary.toString(), StandardCharsets.UTF_8);
        System.out.println("horizon-oracle: dumped " + summary);
    }

    private BufferedWriter writer(String name) throws IOException {
        return Files.newBufferedWriter(dir.resolve(name), StandardCharsets.UTF_8);
    }

    private static void line(BufferedWriter out, JsonObject row) throws IOException {
        out.write(row.toString());
        out.write('\n');
    }

    private static String reason(Throwable t) {
        String m = t.getMessage();
        return t.getClass().getSimpleName() + (m == null ? "" : ": " + m.lines().findFirst().orElse(""));
    }

    private void recipes() throws IOException {
        int encoded = 0, fromFile = 0, failed = 0;
        Map<String, Integer> byType = new TreeMap<>();
        try (BufferedWriter out = writer("recipes.jsonl")) {
            for (RecipeHolder<?> holder : server.getRecipeManager().getRecipes()) {
                ResourceLocation id = holder.id();
                Recipe<?> recipe = holder.value();
                JsonObject row = new JsonObject();
                row.addProperty("id", id.toString());
                String type = BuiltInRegistries.RECIPE_TYPE.getKey(recipe.getType()) + "";
                row.addProperty("type", type);
                byType.merge(type, 1, Integer::sum);
                String problem = null;
                JsonElement json = null;
                try {
                    var result = Recipe.CODEC.encodeStart(ops, recipe);
                    json = result.result().orElse(null);
                    if (json == null) problem = result.error().map(e -> e.message()).orElse("no result");
                } catch (Throwable t) {
                    problem = reason(t);
                }
                if (json != null) {
                    encoded++;
                } else {
                    json = fileOf(id);
                    if (json == null) json = rebuilt(recipe);
                    if (json != null) {
                        fromFile++;
                        row.addProperty("from_file", true);
                    } else {
                        failed++;
                        row.addProperty("failed", problem);
                    }
                }
                if (json != null) row.add("json", json);
                line(out, row);
            }
        }
        JsonObject counts = new JsonObject();
        counts.addProperty("encoded", encoded);
        counts.addProperty("from_file", fromFile);
        counts.addProperty("failed", failed);
        JsonObject types = new JsonObject();
        byType.forEach(types::addProperty);
        counts.add("by_type", types);
        summary.add("recipes", counts);
    }

    /**
     * A shaped recipe a mod made in code, without the pattern text its codec writes (Iron Jetpacks'): the same recipe
     * from its grid.
     */
    private JsonElement rebuilt(Recipe<?> recipe) {
        if (!(recipe instanceof net.minecraft.world.item.crafting.ShapedRecipe shaped)) return null;
        try {
            JsonObject out = new JsonObject();
            out.addProperty("type", "minecraft:crafting_shaped");
            JsonObject key = new JsonObject();
            var pattern = new com.google.gson.JsonArray();
            var grid = shaped.getIngredients();
            List<net.minecraft.world.item.crafting.Ingredient> seen = new ArrayList<>();
            for (int y = 0; y < shaped.getHeight(); y++) {
                StringBuilder row = new StringBuilder();
                for (int x = 0; x < shaped.getWidth(); x++) {
                    var ingredient = grid.get(y * shaped.getWidth() + x);
                    if (ingredient.isEmpty()) {
                        row.append(' ');
                        continue;
                    }
                    int at = seen.indexOf(ingredient);
                    if (at < 0) {
                        at = seen.size();
                        seen.add(ingredient);
                        var encoded = net.minecraft.world.item.crafting.Ingredient.CODEC.encodeStart(ops, ingredient);
                        key.add(String.valueOf((char) ('A' + at)), encoded.getOrThrow());
                    }
                    row.append((char) ('A' + at));
                }
                pattern.add(row.toString());
            }
            out.add("key", key);
            out.add("pattern", pattern);
            var result = shaped.getResultItem(server.registryAccess());
            out.add("result", net.minecraft.world.item.ItemStack.CODEC.encodeStart(ops, result).getOrThrow());
            return out;
        } catch (Exception e) {
            return null;
        }
    }

    /** A recipe's data pack file, for what its codec cannot write. */
    private JsonElement fileOf(ResourceLocation id) {
        ResourceLocation file = ResourceLocation.fromNamespaceAndPath(id.getNamespace(), "recipe/" + id.getPath() + ".json");
        var resource = server.getResourceManager().getResource(file);
        if (resource.isEmpty()) return null;
        try (Reader r = resource.get().openAsReader()) {
            return JsonParser.parseReader(r);
        } catch (Exception e) {
            return null;
        }
    }

    private void lootTables() throws IOException {
        int encoded = 0, failed = 0;
        try (BufferedWriter out = writer("loot_tables.jsonl")) {
            var tables = server.reloadableRegistries();
            for (ResourceLocation id : tables.getKeys(Registries.LOOT_TABLE)) {
                JsonObject row = new JsonObject();
                row.addProperty("id", id.toString());
                try {
                    LootTable table = tables.getLootTable(ResourceKey.create(Registries.LOOT_TABLE, id));
                    var result = LootTable.DIRECT_CODEC.encodeStart(ops, table);
                    if (result.result().isPresent()) {
                        row.add("json", result.result().get());
                        encoded++;
                    } else {
                        row.addProperty("failed", result.error().map(e -> e.message()).orElse("no result"));
                        failed++;
                    }
                } catch (Throwable t) {
                    row.addProperty("failed", reason(t));
                    failed++;
                }
                line(out, row);
            }
        }
        JsonObject counts = new JsonObject();
        counts.addProperty("encoded", encoded);
        counts.addProperty("failed", failed);
        summary.add("loot_tables", counts);
    }

    private void tags() throws IOException {
        JsonObject counts = new JsonObject();
        try (BufferedWriter out = writer("tags.jsonl")) {
            for (var entry : server.registryAccess().registries().toList()) {
                int n = tagsOf(out, entry.key().location().toString(), entry.value());
                if (n > 0) counts.addProperty(entry.key().location().toString(), n);
            }
        }
        summary.add("tags", counts);
    }

    private static <T> int tagsOf(BufferedWriter out, String registry, Registry<T> values) throws IOException {
        List<JsonObject> rows = new ArrayList<>();
        values.getTags().forEach(pair -> {
            JsonObject row = new JsonObject();
            row.addProperty("registry", registry);
            row.addProperty("id", pair.getFirst().location().toString());
            var members = new com.google.gson.JsonArray();
            for (var holder : pair.getSecond()) {
                holder.unwrapKey().ifPresent(key -> members.add(key.location().toString()));
            }
            row.add("values", members);
            rows.add(row);
        });
        for (JsonObject row : rows) line(out, row);
        return rows.size();
    }

    /** The display names of KubeJS's items and blocks: a startup script gives them, and only a client gets a lang file. */
    private void names() throws IOException {
        JsonObject names = new JsonObject();
        for (var item : BuiltInRegistries.ITEM) {
            ResourceLocation id = BuiltInRegistries.ITEM.getKey(item);
            if (!id.getNamespace().equals("kubejs")) continue;
            try {
                names.addProperty(id.toString(), new net.minecraft.world.item.ItemStack(item).getHoverName().getString());
            } catch (Throwable t) {
                // A name that needs a client: the lang file or the id stands in.
            }
        }
        for (var block : BuiltInRegistries.BLOCK) {
            ResourceLocation id = BuiltInRegistries.BLOCK.getKey(block);
            if (!id.getNamespace().equals("kubejs") || names.has(id.toString())) continue;
            try {
                names.addProperty(id.toString(), block.getName().getString());
            } catch (Throwable t) {
                // As above.
            }
        }
        Files.writeString(dir.resolve("names.json"), names.toString(), StandardCharsets.UTF_8);
    }

    private void registries() throws IOException {
        JsonObject all = new JsonObject();
        JsonObject counts = new JsonObject();
        for (var entry : server.registryAccess().registries().toList()) {
            var ids = new com.google.gson.JsonArray();
            entry.value().keySet().stream().map(ResourceLocation::toString).sorted().forEach(ids::add);
            all.add(entry.key().location().toString(), ids);
            counts.addProperty(entry.key().location().toString(), ids.size());
        }
        Files.writeString(dir.resolve("registries.json"), all.toString(), StandardCharsets.UTF_8);
        summary.add("registries", counts);
    }
}
