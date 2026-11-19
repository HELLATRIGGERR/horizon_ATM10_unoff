//! `horizon-materialize-modules <checkout>`: the generated `game/` module declarations written into the checkout's
//! sources (`horizon_mod_api_gen::materialize_game_modules`). Run it on a disposable copy only.
fn main() {
    let Some(root) = std::env::args_os().nth(1) else {
        eprintln!("usage: horizon-materialize-modules <checkout copy>");
        std::process::exit(2);
    };
    if let Err(e) = horizon_mod_api_gen::materialize_game_modules(std::path::Path::new(&root)) {
        eprintln!("materialize: {e}");
        std::process::exit(1);
    }
}
