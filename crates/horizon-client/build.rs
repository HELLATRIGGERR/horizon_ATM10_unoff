//! Generates the native mod API, and gives the Windows executable Horizon's icon and its stack
//! (`packaging/windows/exe.rs`, shared with the official executable's build script).
#[path = "../../packaging/windows/exe.rs"]
mod windows_exe;

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let root = std::path::Path::new(&dir).join("../..");
    let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    println!(
        "cargo:rerun-if-changed={}",
        root.join("crates/horizon-client/src/game").display()
    );
    // The fork's own root modules (AE2 and its addons, and a few more) are declared in `game.rs`
    // itself: `explicit_game_roots`, which the mod API generator reads too.
    std::fs::write(
        out_dir.join("game_modules.rs"),
        horizon_game_discovery::game_declarations(&root.join("crates/horizon-client/src/game"))
            .expect("discovering game modules"),
    )
    .expect("writing game modules");
    let tests =
        root.join("crates/horizon-client/src/crafting/ingredient/builtin_kinds_tests/families");
    println!("cargo:rerun-if-changed={}", tests.display());
    std::fs::write(
        out_dir.join("builtin_ingredient_test_modules.rs"),
        horizon_game_discovery::folder_modules(&tests).expect("discovering ingredient tests"),
    )
    .expect("writing ingredient test modules");
    horizon_mod_api_gen::generate(&root, &out_dir, horizon_mod_api_gen::Target::Client)
        .expect("generating native mod API");
    // Whether a native mod can run in a mob's turn (`game/creatures/mob_islands.rs`). The
    // launcher adds trait packages to the workspace's members.
    println!(
        "cargo:rerun-if-changed={}",
        root.join("Cargo.toml").display()
    );
    for dir in ["native-api", "native-crates"] {
        if root.join(dir).is_dir() {
            println!("cargo:rerun-if-changed={}", root.join(dir).display());
        }
    }
    // The fork's N5: the hooks of the injections horizon-mixins applied to this tree, from its record
    // of a stage (none in a checkout), for the listener gates (`game/modding/fork_compat/bundle_hooks.rs`).
    let stage = root.join(".horizon-mixins-stage.json");
    let mut injected = std::collections::BTreeSet::new();
    if stage.is_file() {
        println!("cargo:rerun-if-changed={}", stage.display());
        let text = std::fs::read_to_string(&stage).expect("reading .horizon-mixins-stage.json");
        let record: serde_json::Value =
            serde_json::from_str(&text).expect("parsing .horizon-mixins-stage.json");
        for injection in record["injections"].as_array().into_iter().flatten() {
            if let Some(hook) = injection["hook"].as_str() {
                injected.insert(hook.to_owned());
            }
        }
    }
    std::fs::write(
        out_dir.join("bundle_hooks.rs"),
        format!(
            "pub(super) const INJECTED: &[&str] = &{:?};\n",
            injected.iter().collect::<Vec<_>>()
        ),
    )
    .expect("writing bundle hooks");
    if let Some(found) = horizon_mod_api_gen::mob_turn(&root) {
        println!("cargo:rustc-env=HORIZON_MOB_TURN={found}");
    }
    // The SDK builds this library, not the platform executable. Carry the same executable
    // policy without asking Cargo to link a binary this package no longer owns.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").expect("Cargo must provide the target OS");
    let target_env =
        std::env::var("CARGO_CFG_TARGET_ENV").expect("Cargo must provide the target environment");
    println!(
        "cargo:rerun-if-changed={}",
        root.join("packaging/executable_link.rs").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        root.join("crates/horizon-guest-stack/src/guest_stack_constants.rs")
            .display()
    );
    for arg in windows_exe::executable_link::arguments(&target_os, &target_env) {
        println!("cargo:horizon-executable-link-arg={arg}");
    }
    if std::env::var("CARGO_PKG_NAME").as_deref() != Ok("horizon-client") {
        windows_exe::resources("horizon-client");
    }
}
