// JEI's row in Options > Mods, with its keys (the compat hook's `NativeMod` is the compat module's type).
fn jei_mods(mods: &mut Vec<NativeMod>) {
    let (id, title, description, keys) = crate::game::jei_world::listed();
    mods.push(NativeMod { id, title, description, keys });
}
