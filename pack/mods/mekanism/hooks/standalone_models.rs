// Models no blockstate names: the QIO drives in a Drive Array's slots (`DriveArrayBakedModel`).
fn mek_standalone_models(models: &mut Vec<String>) {
    crate::game::mek_qio::standalone_models(models);
}
