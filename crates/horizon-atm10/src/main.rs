//! The fork's All the Mods 10 game: horizon-game's program (`crates/horizon-game/src/main.rs`)
//! with the AE2 crates above horizon-client installed before the game starts (docs/ae2-crates.md).
#![forbid(unsafe_code)]

/// Identifies this executable as the game for launcher imports (`horizon_game::GAME_IDENTITY`).
/// `main` reads it, so the linker keeps it.
static GAME_IDENTITY: [u8; horizon_game::GAME_IDENTITY.len()] = horizon_game::GAME_IDENTITY;

fn main() {
    horizon_game::prepare_main_stack();
    std::hint::black_box(&GAME_IDENTITY);
    // The addon crates, then AE2, whose extension every game makes from what they added.
    #[cfg(feature = "ae2-aeinfinitybooster")]
    horizon_ae2x_aeinfinitybooster::install();
    #[cfg(feature = "ae2-appmek")]
    horizon_ae2x_appmek::install();
    #[cfg(feature = "ae2-enderdrives")]
    horizon_ae2x_enderdrives::install();
    #[cfg(any(feature = "ae2-extendedae", feature = "ae2-advancedae"))]
    horizon_ae2x_kits::install();
    #[cfg(feature = "ae2-lab")]
    horizon_ae2::install();
    horizon_game::main();
}

#[cfg(all(test, windows))]
mod tests {
    /// The program's own header asks for the stack that opening a world needs (build.rs).
    #[test]
    fn windows_main_thread_stack_is_reserved() {
        let exe = std::fs::read(std::env::current_exe().unwrap()).unwrap();
        let u32_at = |at: usize| u32::from_le_bytes(exe[at..at + 4].try_into().unwrap()) as usize;
        // e_lfanew, then "PE\0\0" and the 20-byte file header; a PE32+ optional header holds
        // SizeOfStackReserve at 72.
        let optional = u32_at(0x3c) + 4 + 20;
        assert_eq!(&exe[optional..optional + 2], &[0x0b, 0x02], "not PE32+");
        assert!(u32_at(optional + 72) >= 8 << 20);
    }
}
