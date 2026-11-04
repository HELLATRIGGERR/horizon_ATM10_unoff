package dev.horizonlab.neoforgeclient.mixin;

import com.mojang.blaze3d.platform.Window;
import java.util.function.IntSupplier;
import java.util.function.LongSupplier;
import java.util.function.Supplier;
import net.neoforged.fml.loading.ImmediateWindowHandler;
import org.lwjgl.glfw.GLFW;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * The oracle's game window is made hidden (GLFW_VISIBLE false before the window is created), so a run on a shared
 * machine never shows a window: it keeps its OpenGL context, which the texture loading needs. FML's early loading
 * window is off in the run's config/fml.toml ({@code earlyWindowControl}), so this is the only window made.
 */
@Mixin(Window.class)
abstract class HiddenWindowMixin {
    @Redirect(
            method = "<init>",
            at = @At(
                    value = "INVOKE",
                    target = "Lnet/neoforged/fml/loading/ImmediateWindowHandler;setupMinecraftWindow(Ljava/util/function/IntSupplier;Ljava/util/function/IntSupplier;Ljava/util/function/Supplier;Ljava/util/function/LongSupplier;)J"))
    private long horizon$hidden(IntSupplier width, IntSupplier height, Supplier<String> title, LongSupplier monitor) {
        GLFW.glfwWindowHint(GLFW.GLFW_VISIBLE, GLFW.GLFW_FALSE);
        return ImmediateWindowHandler.setupMinecraftWindow(width, height, title, monitor);
    }
}
