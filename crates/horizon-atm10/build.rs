//! Gives the fork's game program what horizon-game's build script gives upstream's: Horizon's icon
//! and the executable's stack and link settings (`packaging/windows/exe.rs`).
#[path = "../../packaging/windows/exe.rs"]
mod windows_exe;

fn main() {
    windows_exe::resources("horizon-client");
}
