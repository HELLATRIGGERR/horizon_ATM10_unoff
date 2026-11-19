// A Modern Industrialization machine's screen (support module `crate::game::mi_ui` of the
// `modern_industrialization` native bundle), drawn as MI's `MachineScreen` and its GUI components draw it
// (`EnergyBarClient`, `ProgressBarClient`, `RecipeEfficiencyBarClient`, the configurable slots), from MI's
// own textures, which the carry brings from the player's jar: `gui/container/background.png`,
// `gui/container/slot_atlas.png`, `gui/progress_bar/*.png`, `gui/efficiency_bar.png`.
// Modern Industrialization is Copyright (c) 2020 Azercoco & Technici4n, MIT; see this bundle's README.md.

use super::{Game, native_hooks::NativeScreen};
use horizon_client::hud::HudBatch;
use horizon_world::fluid::FluidKind;
use mi_core::{
    machines::{self, Machine},
    view::{MachineView, maxed_amount},
};

const BACKGROUND: &str = "modern_industrialization:gui/container/background";
const SLOT_ATLAS: &str = "modern_industrialization:gui/container/slot_atlas";
const EFFICIENCY_BAR: &str = "modern_industrialization:gui/efficiency_bar";
/// `EnergyBarClient.Renderer`'s size.
const ENERGY: [f32; 2] = [13.0, 18.0];
/// `RecipeEfficiencyBarClient`'s size.
const EFFICIENCY: [f32; 2] = [100.0, 2.0];

fn inside(x: f32, y: f32, w: f32, h: f32, at: [f32; 2]) -> bool {
    at[0] >= x && at[0] < x + w && at[1] >= y && at[1] < y + h
}

/// `native_screen`: `MachineGuiParameters`' 176 by 166 panel, the item slots where MI puts them (the
/// hopper menu's others hidden), the player inventory at 8, 84. The title and "Inventory" are vanilla's,
/// at 8, 6 and 8, 72, as `MachineScreen.renderLabels` draws them.
pub(super) fn screen(block: &str, screen: &mut Option<NativeScreen>) {
    let Some(m) = machines::of_block(block) else {
        return;
    };
    if screen.is_some() {
        return;
    }
    let mut slots: Vec<Option<[f32; 2]>> = m.item_slots.iter().map(|[x, y]| Some([*x as f32, *y as f32])).collect();
    slots.resize(5, None);
    *screen = Some(NativeScreen {
        width: 176.0,
        height: m.background_height as f32,
        slots,
        inventory_y: (m.background_height - 82) as f32,
        inventory_x: 8.0,
        own_labels: false,
        own_background: true,
    });
}

/// `native_screen_draw`: `MachineScreen.renderBg` and the components' `renderBackground`.
pub(super) fn draw(game: &Game<'_>, block: &str, data: &[i32], batch: &mut HudBatch, origin: [f32; 2]) {
    let Some(m) = machines::of_block(block) else {
        return;
    };
    let view = MachineView::unpack(data);
    let at = |x: f32, y: f32, w: f32, h: f32| [origin[0] + x, origin[1] + y, w, h];
    let (bw, bh) = (176.0, m.background_height as f32);
    // The panel: its first 4 rows from the texture's top, the rest from its bottom.
    game.native_sprite(batch, BACKGROUND, [0.0, 256.0 - bh + 4.0, bw, bh - 4.0], at(0.0, 4.0, bw, bh - 4.0));
    game.native_sprite(batch, BACKGROUND, [0.0, 0.0, bw, 4.0], at(0.0, 0.0, bw, 4.0));
    draw_progress(game, m, view.progress(), batch, &at);
    draw_energy(game, m, &view, batch, &at);
    draw_efficiency(game, m, &view, batch, &at);
    // `renderConfigurableSlotBackgrounds`: item slots (u 0, 108 when the machine reserved it), fluid
    // slots (u 18, 126), 18 by 18 from one pixel up and left of the slot.
    for (i, [x, y]) in m.item_slots.iter().enumerate() {
        let reserved = i >= m.item_inputs && view.machine_locked & (1 << (i - m.item_inputs)) != 0;
        let u = if reserved { 108.0 } else { 0.0 };
        game.native_sprite(batch, SLOT_ATLAS, [u, 0.0, 18.0, 18.0], at(*x as f32 - 1.0, *y as f32 - 1.0, 18.0, 18.0));
    }
    for (i, [x, y]) in m.fluid_slots.iter().enumerate() {
        let f = view.fluids.get(i).copied().unwrap_or_default();
        let u = if f.locked >= 0 { 126.0 } else { 18.0 };
        game.native_sprite(batch, SLOT_ATLAS, [u, 0.0, 18.0, 18.0], at(*x as f32 - 1.0, *y as f32 - 1.0, 18.0, 18.0));
        // `renderSlot`: the fluid (or the one it is locked to) fills the slot.
        let shown = if f.fluid >= 0 { f.fluid } else { f.locked };
        if let Some(kind) = u32::try_from(shown).ok().and_then(FluidKind::from_id) {
            game.native_fluid_icon(batch, kind.name(), [origin[0] + *x as f32, origin[1] + *y as f32], 16.0);
        }
    }
}

fn draw_progress(game: &Game<'_>, m: &Machine, progress: f32, batch: &mut HudBatch, at: &impl Fn(f32, f32, f32, f32) -> [f32; 4]) {
    let p = m.progress;
    let texture = format!("modern_industrialization:gui/progress_bar/{}", p.kind);
    let (x, y, w, h) = (p.x as f32, p.y as f32, p.width as f32, p.height as f32);
    game.native_sprite(batch, &texture, [0.0, 0.0, w, h], at(x, y, w, h));
    if progress > 0.0 {
        let pixels = (progress * w) as i32 as f32;
        game.native_sprite(batch, &texture, [0.0, h, pixels, h], at(x, y, pixels, h));
    }
}

fn draw_energy(game: &Game<'_>, m: &Machine, view: &MachineView, batch: &mut HudBatch, at: &impl Fn(f32, f32, f32, f32) -> [f32; 4]) {
    let [x, y] = [m.energy[0] as f32, m.energy[1] as f32];
    let [w, h] = ENERGY;
    game.native_sprite(batch, SLOT_ATLAS, [230.0, 0.0, w, h], at(x, y, w, h));
    let fill = if view.eu_capacity > 0 { view.eu as f32 / view.eu_capacity as f32 } else { 0.0 };
    let mut pixels = (fill * h * 0.9 + h * 0.1) as i32 as f32;
    if fill > 0.95 {
        pixels = h;
    }
    game.native_sprite(batch, SLOT_ATLAS, [243.0, h - pixels, w, pixels], at(x, y + h - pixels, w, pixels));
}

fn draw_efficiency(game: &Game<'_>, m: &Machine, view: &MachineView, batch: &mut HudBatch, at: &impl Fn(f32, f32, f32, f32) -> [f32; 4]) {
    let [x, y] = [m.efficiency[0] as f32, m.efficiency[1] as f32];
    let [w, h] = EFFICIENCY;
    game.native_sprite(batch, EFFICIENCY_BAR, [0.0, 2.0, w + 2.0, h + 2.0], at(x - 1.0, y - 1.0, w + 2.0, h + 2.0));
    if view.has_active_recipe && view.max_efficiency_ticks > 0 {
        let pixels = (view.efficiency_ticks as f32 / view.max_efficiency_ticks as f32 * w) as i32 as f32;
        game.native_sprite(batch, EFFICIENCY_BAR, [0.0, 0.0, pixels, h], at(x, y, pixels, h));
    }
}

/// `native_screen_tooltip`: the energy bar's and the efficiency bar's lines, in MI's English
/// (`MIText`), and a fluid slot's contents.
pub(super) fn tooltip(block: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    let Some(m) = machines::of_block(block) else {
        return;
    };
    let view = MachineView::unpack(data);
    let [ex, ey] = [m.energy[0] as f32, m.energy[1] as f32];
    let [fx, fy] = [m.efficiency[0] as f32, m.efficiency[1] as f32];
    if inside(ex, ey, ENERGY[0], ENERGY[1], at) {
        let (amount, max, unit) = maxed_amount(view.eu, view.eu_capacity);
        lines.push(format!("{amount} / {max} {unit}EU"));
    } else if inside(fx, fy, EFFICIENCY[0], EFFICIENCY[1], at) {
        if view.has_active_recipe {
            lines.push(format!("Efficiency: {} / {}", view.efficiency_ticks, view.max_efficiency_ticks));
            let factor = if view.base_recipe_eu > 0 { view.current_recipe_eu as f64 / view.base_recipe_eu as f64 } else { 0.0 };
            lines.push(format!("Overclock: x{}", trim_one_decimal(factor)));
            lines.push(format!("Current consumption: {} EU/t", view.current_recipe_eu));
        } else {
            lines.push("No active recipe to overclock".into());
        }
        lines.push(format!("Max Overclock: {} EU/t", view.max_recipe_eu));
    } else {
        for (i, [x, y]) in m.fluid_slots.iter().enumerate() {
            if inside(*x as f32, *y as f32, 16.0, 16.0, at) {
                let f = view.fluids.get(i).copied().unwrap_or_default();
                let name = u32::try_from(f.fluid).ok().and_then(FluidKind::from_id).map_or_else(|| "Empty".to_owned(), fluid_name);
                lines.push(name);
                lines.push(format!("§7{} / {} mB", f.amount, f.capacity));
                lines.push("§7Fluid Input, Left Click to Insert or Extract".into());
            }
        }
    }
    if view.matches_multiple_recipes && inside(m.progress.x as f32, m.progress.y as f32, 20.0, 20.0, at) {
        lines.push("The inputs match more than one possible recipe.".into());
    }
}

/// A fluid's display name from its id (`modern_industrialization:lubricant` -> `Lubricant`), as the game's
/// recipe viewers name one.
fn fluid_name(kind: FluidKind) -> String {
    let path = kind.name().rsplit(':').next().unwrap_or_default();
    path.split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `new DecimalFormat("#.#")`.
fn trim_one_decimal(v: f64) -> String {
    let s = format!("{:.1}", v);
    s.strip_suffix(".0").map_or(s.clone(), str::to_owned)
}
