// JEI's row in Options > Mods, with its key.
fn jei_mods(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::jei_world::listed(mods);
}
