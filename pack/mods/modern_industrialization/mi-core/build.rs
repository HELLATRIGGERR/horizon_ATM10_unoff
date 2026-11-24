// Embeds the multiblocks the stage step read from the player's ATM10 instance (data/kubejs_multiblocks.json, written
// into the staged copy of this crate and never committed) as kubejs_multiblocks.json in OUT_DIR; without it, an empty
// list. MI_KUBEJS_MULTIBLOCKS names another file to embed (the unit tests' check against the real scripts).
use std::{env, fs, path::PathBuf};

fn main() {
    let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let staged = dir.join("data/kubejs_multiblocks.json");
    println!("cargo:rerun-if-env-changed=MI_KUBEJS_MULTIBLOCKS");
    println!("cargo:rerun-if-changed=data/kubejs_multiblocks.json");
    let from = env::var_os("MI_KUBEJS_MULTIBLOCKS").map(PathBuf::from).filter(|p| p.exists()).or_else(|| staged.exists().then_some(staged));
    if let Some(from) = &from {
        println!("cargo:rerun-if-changed={}", from.display());
    }
    let json = from.map(|p| fs::read_to_string(p).unwrap()).unwrap_or_else(|| "{\"multiblocks\":[]}".to_owned());
    fs::write(PathBuf::from(env::var("OUT_DIR").unwrap()).join("kubejs_multiblocks.json"), json).unwrap();
}
