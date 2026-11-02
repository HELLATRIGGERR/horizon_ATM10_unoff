// The row in Options > Mods.
fn ftbq_mods(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::ftbq_ui::listed(mods);
}
