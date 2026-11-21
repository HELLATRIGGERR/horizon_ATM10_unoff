fn mek_data_pack_files(files: &mut Vec<(String, Vec<u8>)>) {
    for (name, json) in crate::game::mek_radiation::DAMAGE_TYPES {
        files.push((name.into(), json.as_bytes().to_vec()));
    }
}
