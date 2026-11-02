//! Builds (or finds still good) a content root as the game's start does, without the game: how
//! long and how much memory a large pack's root takes.
//!
//! content_root_build BASE CACHE MOD_OR_FOLDER_OF_MODS...
//!
//! BASE is a prepared root, CACHE the `.horizon-cache` to build in. A path holding
//! `horizon-mod.toml` is a mod; any other is a folder of mods. Run it under a tool that samples
//! the process (the peak is the one number this cannot print itself on every OS).
extern crate horizon_client_lib as horizon_client;
use horizon_client::mod_content::{Candidate, load_root, par_map};
use horizon_mods::{Capability, Package};
use std::{path::PathBuf, time::Instant};

#[global_allocator]
static ALLOCATOR: horizon_threads::Allocator = horizon_threads::Allocator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    horizon_threads::configure_allocator();
    let mut args = std::env::args().skip(1);
    let base = PathBuf::from(args.next().ok_or("expected BASE")?);
    let cache = PathBuf::from(args.next().ok_or("expected CACHE")?);
    let mut paths = Vec::new();
    for arg in args {
        let path = PathBuf::from(arg);
        if path.join("horizon-mod.toml").exists() {
            paths.push(path);
        } else {
            let mut folder: Vec<_> = std::fs::read_dir(&path)?
                .flatten()
                .map(|e| e.path())
                .collect();
            folder.sort();
            paths.extend(folder);
        }
    }
    let started = Instant::now();
    let packages = par_map(&paths, 8, |path| Package::read(path));
    let mut candidates = Vec::new();
    for (path, package) in paths.into_iter().zip(packages) {
        let manifest = package.map_err(|e| format!("{}: {e}", path.display()))?.manifest;
        if manifest.capabilities.contains(&Capability::Registries) {
            candidates.push(Candidate { path, manifest });
        }
    }
    println!(
        "manifests: {} mods, {:.2} s",
        candidates.len(),
        started.elapsed().as_secs_f64()
    );
    let started = Instant::now();
    let loaded = load_root(&base, &candidates, &[], &[], &cache)?;
    println!(
        "root: {} mods, {} effects, {:.1} s, {}",
        loaded.mods,
        loaded.effects.len(),
        started.elapsed().as_secs_f64(),
        loaded.root.display()
    );
    Ok(())
}
