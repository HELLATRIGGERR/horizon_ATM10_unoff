package dev.horizonlab.neoforge;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;
import java.util.stream.Stream;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.packs.PackResources;

/**
 * KubeJS's generated data packs as the running server holds them, written out as data pack folders: what
 * {@code ServerEvents.generateData} scripts make at load (Forbidden Arcanus rituals, Hostile Neural Networks models,
 * biome modifiers...), and KubeJS's own internal and registries packs. Each pack goes to {@code <dir>/<stage>/data/...}
 * ({@code internal}, {@code registries}, {@code before_mods}, {@code after_mods}, {@code last}), and
 * {@code <dir>/packs.json} lists them in the resource manager's order (a later pack wins) with their file counts.
 *
 * <p>KubeJS is not on the oracle's classpath: its packs are the server's packs that have its {@code stage} field, and
 * each writes itself out through its own {@code export(Path)} (what KubeJS's export command calls).
 */
final class KubeJsGenerated {
    private KubeJsGenerated() {}

    /** Writes the packs into {@code dir} (emptied first); returns what {@code packs.json} holds. */
    static JsonArray write(MinecraftServer server, Path dir) throws Exception {
        delete(dir);
        Files.createDirectories(dir);
        JsonArray packs = new JsonArray();
        List<PackResources> all = server.getResourceManager().listPacks().toList();
        for (int i = 0; i < all.size(); i++) {
            PackResources pack = all.get(i);
            String stage = stageOf(pack);
            if (stage == null) continue;
            JsonObject row = new JsonObject();
            row.addProperty("stage", stage);
            row.addProperty("pack", pack.packId());
            row.addProperty("index", i);
            row.addProperty("of", all.size());
            Path to = dir.resolve(stage);
            try {
                pack.getClass().getMethod("export", Path.class).invoke(pack, to);
                row.addProperty("files", count(to.resolve("data")));
            } catch (ReflectiveOperationException e) {
                Throwable cause = e.getCause() == null ? e : e.getCause();
                row.addProperty("failed", cause.getClass().getSimpleName() + ": " + cause.getMessage());
            }
            packs.add(row);
        }
        Files.writeString(dir.resolve("packs.json"), packs.toString(), StandardCharsets.UTF_8);
        System.out.println("horizon-oracle: KubeJS's generated packs " + packs);
        return packs;
    }

    /** A KubeJS virtual pack's stage name ({@code after_mods}...), or null for any other pack. */
    private static String stageOf(PackResources pack) {
        if (!pack.getClass().getName().startsWith("dev.latvian.mods.kubejs.")) return null;
        try {
            Object stage = pack.getClass().getField("stage").get(pack);
            return stage == null ? null : (String) stage.getClass().getField("name").get(stage);
        } catch (ReflectiveOperationException | ClassCastException e) {
            return null;
        }
    }

    private static long count(Path dir) throws IOException {
        if (!Files.isDirectory(dir)) return 0;
        try (Stream<Path> files = Files.walk(dir)) {
            return files.filter(Files::isRegularFile).count();
        }
    }

    private static void delete(Path dir) throws IOException {
        if (!Files.exists(dir)) return;
        try (Stream<Path> files = Files.walk(dir)) {
            for (Path p : files.sorted(Comparator.reverseOrder()).toList()) Files.delete(p);
        }
    }
}
