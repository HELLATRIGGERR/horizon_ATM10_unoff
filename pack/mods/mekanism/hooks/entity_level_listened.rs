// Mekanism hears entities joining a level (`native_entity_join_level`): a mixin bundle says so.
fn mek_entity_level_listened(listened: &mut bool) {
    *listened = true;
}
