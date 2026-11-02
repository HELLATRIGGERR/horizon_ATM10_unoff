// A Furnator's screen (support module `crate::game::powah_ui` of the `powah`
// native bundle), drawn from Powah's own `gui/container/furnator.png`, which
// the content mod carries from the player's jar: the 176x166 panel, the energy
// gauge on the left, the fuel slot with its burn bar and the bolt that lights
// while it burns. Powah is LGPL-3.0; see this bundle's README.md.

use super::{Game, native_hooks::NativeScreen};
use horizon_client::{hud::HudBatch, items::ItemStack, text::Text};
use powah_core::{
    tier::Tier,
    view::{FurnatorView, commas, short},
};

const TEXTURE: &str = "powah:gui/container/furnator";
const TITLE: u32 = 0x404040;

type Rect = [f32; 4];

/// The charge slot and the fuel slot, as the texture draws them.
const CHARGE_SLOT: [f32; 2] = [4.0, 54.0];
const FUEL_SLOT: [f32; 2] = [87.0, 18.0];
/// The inside of the energy gauge, and the red fill it takes from the bottom.
const GAUGE: Rect = [5.0, 5.0, 14.0, 39.0];
const GAUGE_FILL: [f32; 2] = [176.0, 0.0];
/// The inside of the burn bar and its dark fill.
const FUEL_BAR: Rect = [110.0, 18.0, 4.0, 16.0];
const FUEL_FILL: [f32; 2] = [190.0, 0.0];
/// The bolt over the grey one the panel has, and where it is on the texture.
const BOLT: Rect = [94.0, 43.0, 11.0, 18.0];
const BOLT_LIT: [f32; 2] = [176.0, 39.0];
/// The main panel, the title centred along its top.
const PANEL_LEFT: f32 = 24.0;
const PANEL_WIDTH: f32 = 152.0;

fn inside(r: Rect, at: [f32; 2]) -> bool {
    at[0] >= r[0] && at[0] < r[0] + r[2] && at[1] >= r[1] && at[1] < r[1] + r[3]
}

/// `native_screen`: the panel's size and its two slots; the title is drawn here.
pub(super) fn screen(block: &str, screen: &mut Option<NativeScreen>) {
    if screen.is_some() || Tier::of_furnator(block).is_none() {
        return;
    }
    *screen = Some(NativeScreen {
        width: 176.0,
        height: 166.0,
        // The charge slot and the fuel slot; the hopper menu's other three don't exist.
        slots: vec![Some(CHARGE_SLOT), Some(FUEL_SLOT), None, None, None],
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: true,
    });
}

/// `native_screen_draw`.
pub(super) fn draw(game: &Game<'_>, block: &str, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    if Tier::of_furnator(block).is_none() {
        return;
    }
    let view = FurnatorView::unpack(data);
    let at = |r: Rect| [origin[0] + r[0], origin[1] + r[1], r[2], r[3]];
    game.native_sprite(batch, TEXTURE, [0.0, 0.0, 176.0, 166.0], at([0.0, 0.0, 176.0, 166.0]));
    // Both bars are filled from the bottom.
    let rows = view.energy_rows(GAUGE[3] as i64) as f32;
    if rows > 0.0 {
        let skip = GAUGE[3] - rows;
        game.native_sprite(
            batch,
            TEXTURE,
            [GAUGE_FILL[0], GAUGE_FILL[1] + skip, GAUGE[2], rows],
            at([GAUGE[0], GAUGE[1] + skip, GAUGE[2], rows]),
        );
    }
    let rows = view.fuel_rows(FUEL_BAR[3] as i64) as f32;
    if rows > 0.0 {
        let skip = FUEL_BAR[3] - rows;
        game.native_sprite(
            batch,
            TEXTURE,
            [FUEL_FILL[0], FUEL_FILL[1] + skip, FUEL_BAR[2], rows],
            at([FUEL_BAR[0], FUEL_BAR[1] + skip, FUEL_BAR[2], rows]),
        );
    }
    if view.burning {
        game.native_sprite(batch, TEXTURE, [BOLT_LIT[0], BOLT_LIT[1], BOLT[2], BOLT[3]], at(BOLT));
    }
    let title = game.native_screen_title().unwrap_or_default();
    let x = PANEL_LEFT + ((PANEL_WIDTH - game.native_text_width(&title)) / 2.0).floor();
    game.native_text(batch, &title, [origin[0] + x, origin[1] + 5.0], TITLE, false);
}

/// `native_screen_tooltip`.
pub(super) fn tooltip(block: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    if Tier::of_furnator(block).is_none() {
        return;
    }
    let view = FurnatorView::unpack(data);
    if inside(GAUGE, at) {
        lines.push(stored_line(view.stored, view.capacity));
        lines.push(rate_line("Generates", view.generation));
        lines.push(rate_line("Max Extract", view.transfer));
    } else if inside(FUEL_BAR, at) {
        lines.push("§8Carbon".into());
    }
}

fn stored_line(stored: i64, capacity: i64) -> String {
    format!("§7Stored: §8{}/{} FE", commas(stored), short(capacity))
}

fn rate_line(name: &str, rate: i64) -> String {
    format!("§7{name}: §8{} FE/t", short(rate))
}

/// `native_tooltip`: what a Furnator's item holds.
pub(super) fn item_tooltip(stack: &ItemStack, lines: &mut Vec<(Text, u32)>) {
    let Some(tier) = Tier::of_furnator(&stack.item) else {
        return;
    };
    let stored = super::powah_world::item_energy(stack);
    let spec = super::powah_world::spec_of(tier);
    lines.push((Text::legacy(&stored_line(stored, spec.capacity)), u32::MAX));
    lines.push((Text::legacy(&rate_line("Generates", spec.generation)), u32::MAX));
    lines.push((Text::legacy(&rate_line("Max Extract", spec.transfer)), u32::MAX));
}
