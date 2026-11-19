// A furnace's screen (support module `crate::game::ironfurnaces_ui` of the `ironfurnaces` native bundle), drawn
// from Iron Furnaces' own `gui/furnace.png`, which the carry brings from the player's jar: the 176x166 panel, the
// input, fuel and output slots, the flame that burns down and the arrow that fills. Translated from
// `BlockIronFurnaceScreenBase` (Apache-2.0); see this bundle's README.md.

use super::Game;
use horizon_client::hud::HudBatch;
use mod_api::Screen;
use ironfurnaces_core::{furnace::Timers, tier::Tier};

const TEXTURE: &str = "ironfurnaces:gui/furnace";
const LABEL: u32 = 0x404040;

/// The input, fuel and output slots, as `BlockIronFurnaceContainerBase` places them.
const INPUT_SLOT: [f32; 2] = [56.0, 17.0];
const FUEL_SLOT: [f32; 2] = [56.0, 53.0];
const OUTPUT_SLOT: [f32; 2] = [116.0, 35.0];

/// `native_screen`: the panel's size and its three slots; the augment slots and the three the menu pads out to
/// nine are left out.
pub(super) fn screen(block: &str, screen: &mut Option<Screen>) {
    if screen.is_some() || Tier::of_block(block).is_none() {
        return;
    }
    let mut slots = vec![Some(INPUT_SLOT), Some(FUEL_SLOT), Some(OUTPUT_SLOT)];
    slots.resize(9, None);
    *screen = Some(Screen {
        width: 176.0,
        height: 166.0,
        slots,
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: true,
        own_background: true,
        ..Screen::default()
    });
}

/// `native_screen_draw`: `renderBg` and `renderFurnaceBg`, then the labels.
pub(super) fn draw(game: &Game<'_>, block: &str, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    if Tier::of_block(block).is_none() {
        return;
    }
    let timers = Timers {
        burn: data.first().copied().unwrap_or(0),
        used: data.get(1).copied().unwrap_or(0),
        cook: data.get(2).copied().unwrap_or(0),
        total: data.get(3).copied().unwrap_or(0),
    };
    let at = |x: f32, y: f32, w: f32, h: f32| [origin[0] + x, origin[1] + y, w, h];
    game.native_sprite(batch, TEXTURE, [0.0, 0.0, 176.0, 166.0], at(0.0, 0.0, 176.0, 166.0), None);
    if timers.burning() {
        let i = timers.flame(13).clamp(0, 13) as f32;
        game.native_sprite(batch, TEXTURE, [176.0, 12.0 - i, 14.0, i + 1.0], at(56.0, 36.0 + 12.0 - i, 14.0, i + 1.0), None);
    }
    let i = timers.arrow(24).clamp(0, 24) as f32;
    game.native_sprite(batch, TEXTURE, [176.0, 14.0, i + 1.0, 16.0], at(79.0, 34.0, i + 1.0, 16.0), None);
    let title = game.native_screen_title().unwrap_or_default();
    game.native_text(batch, &title, [origin[0] + 8.0, origin[1] + 6.0], LABEL, false);
    game.native_text(batch, "Inventory", [origin[0] + 8.0, origin[1] + 72.0], LABEL, false);
}
