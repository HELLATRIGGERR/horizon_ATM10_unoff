// Woodworking station screen (support module `crate::game::woodworking_ui` of the
// `woodworking` native bundle). The screen mirrors a stonecutter: input slot on the left, a
// recipe grid in the centre, and an output slot on the right. The recipe grid is 4 columns ×
// 3 rows; the scroll state lives in entity.data[2] (multiples of 4). Left-click selects a
// recipe; right-click anywhere in the recipe area pages down.

use super::Game;
use horizon_client::hud::HudBatch;
use mod_api::Screen;
use woodworking_core::index::decode_input;

// Screen dimensions and slot positions.
const W: f32 = 176.0;
const H: f32 = 166.0;
const INPUT_XY: [f32; 2] = [20.0, 35.0];
const OUTPUT_XY: [f32; 2] = [152.0, 35.0];
const INV_Y: f32 = 84.0;
const INV_X: f32 = 8.0;

// Recipe grid.
const GRID_X: f32 = 52.0;
const GRID_Y: f32 = 14.0;
const CELL: f32 = 18.0;
const COLS: usize = 4;
const ROWS: usize = 3;
const PAGE: usize = COLS * ROWS;

// Colours.
const BG: u32 = 0xFFC6C6C6;
const BORDER: u32 = 0xFF373737;
const CELL_BG: u32 = 0xFF8B8B8B;
const CELL_SEL: u32 = 0xFF6060DD;
const SCROLL_BTN: u32 = 0xFF5A5A8A;
const LABEL: u32 = 0xFF404040;
const SLOT_BG: u32 = 0xFF8B8B8B;
const ARROW: u32 = 0xFFFFFFFF;

fn inside(rx: f32, ry: f32, rw: f32, rh: f32, at: [f32; 2]) -> bool {
    at[0] >= rx && at[0] < rx + rw && at[1] >= ry && at[1] < ry + rh
}

/// `native_screen`: size, two slots, inventory position.
pub(super) fn screen(block: &str, screen: &mut Option<Screen>) {
    if screen.is_some() || block != crate::game::woodworking_world::station() {
        return;
    }
    let mut slots = vec![Some(INPUT_XY), Some(OUTPUT_XY)];
    slots.resize(9, None);
    *screen = Some(Screen {
        width: W,
        height: H,
        slots,
        inventory_y: INV_Y,
        inventory_x: INV_X,
        own_labels: true,
        own_background: true,
        ..Screen::default()
    });
}

/// `native_screen_draw`: background, recipe grid, slot outlines, title.
pub(super) fn draw(game: &Game<'_>, block: &str, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    if block != crate::game::woodworking_world::station() {
        return;
    }
    let n = data.first().copied().unwrap_or(0).max(0) as usize;
    let selected = data.get(1).copied().unwrap_or(-1);
    let scroll = data.get(2).copied().unwrap_or(0).max(0) as usize;
    let input_id = decode_input(data).unwrap_or_default();

    let ox = origin[0];
    let oy = origin[1];

    // ---- Panel background ----
    game.native_fill(batch, [ox, oy, W, H], BG);
    game.native_fill(batch, [ox, oy, W, 1.0], BORDER);
    game.native_fill(batch, [ox, oy + H - 1.0, W, 1.0], BORDER);
    game.native_fill(batch, [ox, oy, 1.0, H], BORDER);
    game.native_fill(batch, [ox + W - 1.0, oy, 1.0, H], BORDER);

    // ---- Slot frames: input, output, the player's inventory and hotbar ----
    let mut frame = |x: f32, y: f32, size: f32| {
        game.native_fill(batch, [ox + x - 1.0, oy + y - 1.0, size + 2.0, size + 2.0], BORDER);
        game.native_fill(batch, [ox + x, oy + y, size, size], SLOT_BG);
    };
    frame(INPUT_XY[0], INPUT_XY[1], 16.0);
    frame(OUTPUT_XY[0] - 4.0, OUTPUT_XY[1] - 4.0, 24.0);
    for row in 0..3 {
        for col in 0..9 {
            frame(INV_X + col as f32 * 18.0, INV_Y + row as f32 * 18.0, 16.0);
        }
    }
    for col in 0..9 {
        frame(INV_X + col as f32 * 18.0, INV_Y + 58.0, 16.0);
    }

    // ---- Recipe area background ----
    let ga_w = COLS as f32 * CELL + 2.0;
    let ga_h = ROWS as f32 * CELL + 2.0;
    game.native_fill(batch, [ox + GRID_X - 1.0, oy + GRID_Y - 1.0, ga_w, ga_h], BORDER);

    // ---- Recipe cells ----
    crate::game::woodworking_world::with_index_for_draw(game, |idx_opt| {
        let entries: &[woodworking_core::index::Entry] = idx_opt
            .map(|ix| ix.recipes_for(&input_id))
            .unwrap_or(&[]);
        for row in 0..ROWS {
            for col in 0..COLS {
                let slot_idx = scroll + row * COLS + col;
                let cx = ox + GRID_X + col as f32 * CELL;
                let cy = oy + GRID_Y + row as f32 * CELL;
                let bg = if slot_idx as i32 == selected { CELL_SEL } else { CELL_BG };
                game.native_fill(batch, [cx, cy, 16.0, 16.0], bg);
                if let Some(entry) = entries.get(slot_idx) {
                    game.native_item(batch, &entry.output, [cx, cy], 16.0);
                }
            }
        }
    });

    // ---- Scroll buttons ----
    if scroll > 0 {
        // Up arrow (▲): a small triangle above the grid
        game.native_fill(batch, [ox + GRID_X + ga_w - 1.0 + 2.0, oy + GRID_Y - 1.0, 10.0, 10.0], SCROLL_BTN);
        game.native_text(batch, "^", [ox + GRID_X + ga_w + 3.0, oy + GRID_Y], ARROW, true);
    }
    let max_scroll = if n > PAGE { ((n - 1) / COLS) * COLS } else { 0 };
    if scroll < max_scroll {
        game.native_fill(batch, [ox + GRID_X + ga_w - 1.0 + 2.0, oy + GRID_Y + ga_h - 10.0 - 1.0, 10.0, 10.0], SCROLL_BTN);
        game.native_text(batch, "v", [ox + GRID_X + ga_w + 3.0, oy + GRID_Y + ga_h - 10.0], ARROW, true);
    }

    // ---- Recipe count label ----
    if !input_id.is_empty() && n > 0 {
        let count_str = format!("{}", n);
        game.native_text(batch, &count_str, [ox + INPUT_XY[0], oy + INPUT_XY[1] + 20.0], LABEL, false);
    }

    // ---- Title ----
    let title = "Woodworking";
    game.native_text(batch, title, [ox + 8.0, oy + 6.0], LABEL, false);
    game.native_text(batch, "Inventory", [ox + 8.0, oy + INV_Y - 12.0], LABEL, false);
}

/// `native_screen_click`: map a click position in the recipe grid to a button id.
/// Left click on a recipe cell → that recipe's index (0-based).
/// Right click anywhere in the grid → scroll down (button id = -2).
pub(super) fn click(
    _game: &Game<'_>,
    block: &str,
    data: &[i32],
    at: [f32; 2],
    right: bool,
    button: &mut Option<i32>,
) {
    if button.is_some() || block != crate::game::woodworking_world::station() {
        return;
    }
    let n = data.first().copied().unwrap_or(0).max(0) as usize;
    if n == 0 {
        return;
    }
    let scroll = data.get(2).copied().unwrap_or(0).max(0) as usize;

    let grid_w = COLS as f32 * CELL;
    let grid_h = ROWS as f32 * CELL;

    if !inside(GRID_X, GRID_Y, grid_w, grid_h, at) {
        return;
    }

    if right {
        let max_scroll = if n > PAGE { ((n - 1) / COLS) * COLS } else { 0 };
        if scroll < max_scroll {
            *button = Some(-2); // scroll down
        }
        return;
    }

    // Left click: recipe selection.
    let col = ((at[0] - GRID_X) / CELL).floor() as usize;
    let row = ((at[1] - GRID_Y) / CELL).floor() as usize;
    let idx = scroll + row * COLS + col;
    if idx < n {
        *button = Some(idx as i32);
    }
}
