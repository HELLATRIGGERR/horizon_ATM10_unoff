// JEI's recipe window in the game (support module `crate::game::jei_window` of the `jei` native bundle): the page
// `jei_recipes` shows, drawn as JEI's `RecipesGui` draws it over the open screen (`native_screen_render_post`):
// centred, left of the item list (`native_panel_area`), the category tabs along its top, the catalysts on its left,
// the category title between its arrows, the page row ("1/4") under it, then as many recipes as fit by height with
// their notes wrapped to the window. Clicks, the wheel and keys on it come in through the screen hooks, which only
// read the game: they leave an `Action` for `jei_recipes::frame`. It is drawn with JEI's own sprites
// (`jei:gui/sprites/...`), which the carry of the player's JEI jar brings as a content mod's GUI textures; without
// that carry, with the game's sprites and fills in JEI's sizes and colours.
use super::Game;
use super::jei_world::ID;
use horizon_client::mod_api::{NativeRecipeTransfer, NativeScreenView, NativeStack, NativeTransferAnswer, NativeTransferHandler};
use horizon_client::{hud::HudBatch, items::ItemStack, text::Text};
use jei_core::recipes::{Mode, Page, Shape, Stack, View};
use jei_core::window::{
    ARROW, LINE, OUTPUT, OUTPUT_RIM, Rect, SLOT, Sizes, Window, inside, output_slot, pages, places, spread, window,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// What a click, the wheel or a key on the window asks for; `jei_recipes::frame` does it.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Action {
    Open(String, Mode),
    Tab(usize),
    /// The category before (-1) or after (+1), as the title's arrows.
    Category(i32),
    Page(i32),
    Close,
    /// JEI's "+" on the page's recipe `n` (`RecipeTransferButtonController.onPress`).
    Transfer(usize),
}

/// A place on the window that answers a click.
#[derive(Clone, Debug)]
enum Hit {
    Stack(Stack),
    Do(Action),
}

/// The window as last drawn: for the clicks, keys and the probe.
#[derive(Default)]
struct Drawn {
    window: Window,
    pages: usize,
    hits: Vec<(Rect, Hit)>,
    panel: Option<Rect>,
    /// The recipes on the recipe page shown, `first..end` of the page's.
    shown: std::ops::Range<usize>,
}

struct Shown {
    page: Page,
    recipe_page: usize,
    since: Instant,
    drawn_at: Instant,
    drawn: Drawn,
    /// Each recipe's category id and `book_id` (-1 past the vanilla crafting ones), for its transfer.
    sources: Vec<(String, i32)>,
    /// What the open screen would do with the shown recipes' transfers (`native_recipe_transfer`, asked each frame).
    answers: Vec<(usize, NativeTransferAnswer)>,
}

static SHOWN: Mutex<Option<Shown>> = Mutex::new(None);
static ACTIONS: Mutex<Vec<Action>> = Mutex::new(Vec::new());
static NAMES: Mutex<Option<Arc<HashMap<String, String>>>> = Mutex::new(None);

fn shown() -> std::sync::MutexGuard<'static, Option<Shown>> {
    SHOWN.lock().unwrap_or_else(|e| e.into_inner())
}
fn ask(action: Action) {
    ACTIONS.lock().unwrap_or_else(|e| e.into_inner()).push(action);
}
pub(super) fn take_actions() -> Vec<Action> {
    std::mem::take(&mut *ACTIONS.lock().unwrap_or_else(|e| e.into_inner()))
}
/// The items' names, for the labels and tooltips (set with each new book).
pub(super) fn set_names(names: Arc<HashMap<String, String>>) {
    *NAMES.lock().unwrap_or_else(|e| e.into_inner()) = Some(names);
}
pub(super) fn name(item: &str) -> String {
    let names = NAMES.lock().unwrap_or_else(|e| e.into_inner());
    names.as_ref().and_then(|n| n.get(item).cloned()).unwrap_or_else(|| item.to_owned())
}

/// The page to show, or none to close the window. The same page again keeps its recipe page.
pub(super) fn set(page: Option<Page>, sources: Vec<(String, i32)>) {
    let mut guard = shown();
    match page {
        None => *guard = None,
        Some(page) => {
            if let Some(s) = guard.as_mut().filter(|s| s.page == page) {
                s.sources = sources;
                return;
            }
            let now = Instant::now();
            let since = guard.as_ref().map_or(now, |s| s.since);
            let drawn = Drawn::default();
            *guard = Some(Shown { page, recipe_page: 0, since, drawn_at: now, drawn, sources, answers: Vec::new() });
        }
    }
}

/// Recipe `i` of the page as the open screen's transfer takes it: the input slots its shape lays out (a crafting
/// recipe's 3x3 grid; `width` x `height` the cells it uses), the result then the other outputs.
fn transfer_of(v: &View, source: Option<&(String, i32)>) -> NativeRecipeTransfer {
    let n = jei_core::window::inputs(v.shape, v.width, v.height).min(v.slots.len());
    let native = |s: &Stack| NativeStack { item: s.item.clone(), count: s.count, components: s.components.clone() };
    let inputs: Vec<Vec<NativeStack>> = v.slots[..n].iter().map(|l| l.iter().map(native).collect()).collect();
    let cols = v.width.max(1) as usize;
    let used: Vec<usize> = (0..n).filter(|&i| !v.slots[i].is_empty()).collect();
    let span = |at: &dyn Fn(usize) -> usize| {
        let (lo, hi) = used.iter().map(|&i| at(i)).fold((usize::MAX, 0), |(lo, hi), c| (lo.min(c), hi.max(c)));
        if used.is_empty() { 0 } else { (hi - lo + 1) as u32 }
    };
    let (width, height) = if v.shape == Shape::Grid { (span(&|i| i % cols), span(&|i| i / cols)) } else { (1, 1) };
    let outputs = v.results.first().into_iter().chain(v.slots[n..].iter().filter_map(|l| l.first())).map(native).collect();
    let (category, book_id) = source.cloned().unwrap_or_else(|| (String::new(), -1));
    NativeRecipeTransfer { category, book_id, width, height, inputs, outputs }
}
/// The transfers of the recipes on the recipe page shown, to ask the open screen about each frame.
pub(super) fn transfers() -> Vec<(usize, NativeRecipeTransfer)> {
    let guard = shown();
    let Some(s) = guard.as_ref() else { return Vec::new() };
    let end = s.drawn.shown.end.min(s.page.views.len());
    (s.drawn.shown.start.min(end)..end).map(|i| (i, transfer_of(&s.page.views[i], s.sources.get(i)))).collect()
}
/// Recipe `i`'s transfer, for its "+".
pub(super) fn transfer(i: usize) -> Option<NativeRecipeTransfer> {
    let guard = shown();
    let s = guard.as_ref()?;
    Some(transfer_of(s.page.views.get(i)?, s.sources.get(i)))
}
pub(super) fn set_answers(answers: Vec<(usize, NativeTransferAnswer)>) {
    if let Some(s) = shown().as_mut() {
        s.answers = answers;
    }
}

/// JEI's `RecipeTransferButtonController.updateStateForTransferError` for an answer: whether the "+" can be pressed,
/// and a COSMETIC error's highlight over it (AE2's crafting terminal: orange with items missing, blue with
/// craftable ones only).
fn button_state(a: &NativeTransferAnswer, inputs: usize) -> (bool, Option<u32>) {
    match a.handler {
        Some(NativeTransferHandler::Ae2Crafting) => {
            let none = !a.missing.is_empty() && a.missing.len() >= inputs;
            let highlight = if none || a.too_large {
                None
            } else if !a.missing.is_empty() {
                Some(0x80FFA500)
            } else if !a.craftable.is_empty() {
                Some(0x804545FF)
            } else {
                None
            };
            (!none && !a.too_large, highlight)
        }
        Some(NativeTransferHandler::Ae2Encoding) => (true, None),
        _ => (!a.too_large && a.missing.is_empty(), None),
    }
}
/// The "+"'s tooltip: JEI's "Move Items" and the error's lines (`RecipeTransferUtil.addTransferRecipeTooltip`), or
/// AE2's handlers' (`ae2.rei_jei_integration.*`).
fn button_tooltip(a: &NativeTransferAnswer, active: bool, small_grid: bool, ctrl: bool) -> Vec<Text> {
    const RED: u32 = 0xFF5555;
    const BLUE: u32 = 0x5555FF;
    let mut lines = Vec::new();
    match a.handler {
        Some(NativeTransferHandler::Ae2Encoding) => {
            lines.push(Text::literal("Encode pattern"));
            if !a.craftable.is_empty() {
                lines.push(Text::colored("Highlighted elements are already craftable", BLUE));
            }
        }
        Some(NativeTransferHandler::Ae2Crafting) => {
            // UseCraftingRecipeTransfer: a user error (too large, nothing found) gets JEI's tooltip, its COSMETIC
            // error AE2's `TransferHelper.createCraftingTooltip` (Ctrl held: "Will craft").
            if a.too_large || !active {
                lines.push(Text::literal("Move Items"));
                let text = if a.too_large { "Recipe larger than 3x3" } else { "Found no compatible items" };
                lines.push(Text::colored(text, RED));
            } else if a.missing.is_empty() && a.craftable.is_empty() {
                lines.push(Text::literal("Move Items"));
            } else {
                lines.push(Text::literal("Move items"));
                if !a.craftable.is_empty() {
                    let text = if ctrl { "Will craft unavailable items" } else { "CTRL + click to craft unavailable items" };
                    lines.push(Text::colored(text, BLUE));
                }
                if !a.missing.is_empty() {
                    lines.push(Text::colored("Missing items will be skipped", RED));
                }
            }
        }
        _ => {
            lines.push(Text::literal("Move Items"));
            if a.too_large {
                let text = if small_grid {
                    "Recipe is too large to craft in the 2x2 player crafting grid."
                } else {
                    "Recipe is too large."
                };
                lines.push(Text::colored(text, RED));
            } else if !a.missing.is_empty() {
                lines.push(Text::colored("Missing Items", RED));
            }
        }
    }
    lines
}
/// JEI's 13 px recipe buttons (`RecipeLayout.RECIPE_BUTTON_SIZE`), 2 px right of the recipe (`RECIPE_BUTTON_SPACING`),
/// the "+" level with its bottom.
const RECIPE_BUTTON: f32 = 13.0;
const RECIPE_BUTTON_SPACING: f32 = 2.0;
const BUTTON_DISABLED: Nine = Nine("button_disabled_v2", 20.0, [6.0; 4]);
pub(super) fn is_open() -> bool {
    shown().is_some()
}
/// The recipe page before or after; the last wraps to the first as JEI's arrows do.
pub(super) fn turn(by: i32) {
    if let Some(s) = shown().as_mut() {
        let n = s.drawn.pages.max(1) as i32;
        s.recipe_page = (s.recipe_page as i32 + by).rem_euclid(n) as usize;
    }
}
/// Whether the window went undrawn a while: its screen closed under it.
pub(super) fn stale() -> bool {
    shown().as_ref().is_some_and(|s| s.drawn_at.elapsed() > Duration::from_millis(500))
}
/// Whether `at` is on the window, its tabs or its catalysts (not on the item list).
pub(super) fn covers(at: [f32; 2]) -> bool {
    shown().as_ref().is_some_and(|s| on_window(&s.drawn, at))
}
fn on_window(d: &Drawn, at: [f32; 2]) -> bool {
    let w = &d.window;
    inside(at, w.area) || w.tabs.iter().any(|r| inside(at, *r)) || w.catalyst_area.is_some_and(|r| inside(at, r))
}
fn hit_at(d: &Drawn, at: [f32; 2]) -> Option<&Hit> {
    d.hits.iter().find(|(r, _)| inside(at, *r)).map(|(_, h)| h)
}

/// The stack a slot shows now: several cycle a second each (JEI's `CycleTicker`).
fn now<'a>(stacks: &'a [Stack], since: Instant) -> Option<&'a Stack> {
    (!stacks.is_empty()).then(|| &stacks[since.elapsed().as_secs() as usize % stacks.len()])
}

fn item_stack(s: &Stack) -> ItemStack {
    let (damage, extra) = if s.components.is_empty() {
        (0, None)
    } else {
        horizon_client::item_data::command_components(&s.components)
            .map_or((0, None), |tag| horizon_client::item_data::decode_components(Some(&tag)))
    };
    ItemStack { item: s.item.as_str().into(), count: s.count.clamp(1, 99), damage, extra }
}

// ---- Drawing ---------------------------------------------------------------------------------------------------

/// The fallback's faces when JEI's sprites are not carried: vanilla's container panel (a light grey face, a white
/// top-left bevel, a dark bottom-right one, a black rim with cut corners), as `gui_background_v2` looks.
const FACE: u32 = 0xFFC6C6C6;
const TAB_FACE: u32 = 0xFFA8A8A8;
const NOTE: u32 = 0x808080;
const TITLE: u32 = 0xFFFFFF;
/// JEI's `pageNavigationBackground` (`assets/jei/gui/colors.json`): the band between a row's arrows.
pub(super) const BAND: u32 = 0x30000000;

/// A nine-slice sprite of JEI's: its name under `jei:gui/sprites/`, its `gui.scaling` size and its border (left,
/// right, top, bottom), as its `.png.mcmeta` in JEI's jar says.
pub(super) struct Nine(&'static str, f32, [f32; 4]);
const BACKGROUND: Nine = Nine("gui_background_v2", 64.0, [16.0; 4]);
const CATALYST_TAB: Nine = Nine("catalyst_tab_v2", 28.0, [8.0, 9.0, 8.0, 8.0]);
const CATALYST_SLOTS: Nine = Nine("recipe_catalyst_slot_background_v2", 18.0, [4.0; 4]);
const BUTTON: Nine = Nine("button_enabled_v2", 20.0, [6.0; 4]);
const BUTTON_HOVER: Nine = Nine("button_highlight_v2", 20.0, [6.0; 4]);
/// The bookmark list's background and the slots' (`bookmark_list_background_v2`, `bookmark_list_slot_background_v2`).
pub(super) const BOOKMARK_LIST: Nine = Nine("bookmark_list_background_v2", 64.0, [16.0; 4]);
pub(super) const BOOKMARK_SLOTS: Nine = Nine("bookmark_list_slot_background_v2", 18.0, [4.0; 4]);

fn jei_sprite(name: &str) -> String {
    format!("jei:gui/sprites/{name}")
}
/// Whether the carry brought JEI's sprites.
fn carried(game: &Game<'_>) -> bool {
    game.native_sprite_size(&jei_sprite(BACKGROUND.0)).is_some()
}
/// JEI's sprite `name` whole over `dst`; false without it.
fn sprite(game: &Game<'_>, batch: &mut HudBatch, name: &str, dst: Rect) -> bool {
    let name = jei_sprite(name);
    let Some([w, h]) = game.native_sprite_size(&name) else { return false };
    game.native_sprite(batch, &name, [0.0, 0.0, w, h], dst)
}
/// `GuiGraphics.blitNineSlicedSprite` over `r` with a JEI sprite: the corners as they are, the edges and the centre
/// tiled, each border at most half of `r`. A pack's larger copy is read in its own pixels. False without it.
pub(super) fn nine(game: &Game<'_>, batch: &mut HudBatch, n: &Nine, [x, y, w, h]: Rect) -> bool {
    let Nine(name, size, [l, rt, t, b]) = *n;
    let name = jei_sprite(name);
    let Some(own) = game.native_sprite_size(&name) else { return false };
    let half = |v: f32| (v / 2.0).floor();
    let (l, rt, t, b) = (l.min(half(w)), rt.min(half(w)), t.min(half(h)), b.min(half(h)));
    let cols = [(0.0, l, x, l), (l, size - l - rt, x + l, w - l - rt), (size - rt, rt, x + w - rt, rt)];
    let rows = [(0.0, t, y, t), (t, size - t - b, y + t, h - t - b), (size - b, b, y + h - b, b)];
    let k = [own[0] / size, own[1] / size];
    for (sy, sh, dy, dh) in rows {
        for (sx, sw, dx, dw) in cols {
            if sw <= 0.0 || sh <= 0.0 || dw <= 0.0 || dh <= 0.0 {
                continue;
            }
            let mut oy = 0.0;
            while oy < dh {
                let th = sh.min(dh - oy);
                let mut ox = 0.0;
                while ox < dw {
                    let tw = sw.min(dw - ox);
                    let src = [sx * k[0], sy * k[1], tw * k[0], th * k[1]];
                    game.native_sprite(batch, &name, src, [dx + ox, dy + oy, tw, th]);
                    ox += sw;
                }
                oy += sh;
            }
        }
    }
    true
}

fn raised(game: &Game<'_>, batch: &mut HudBatch, [x, y, w, h]: Rect, face: u32) {
    game.native_fill(batch, [x + 1.0, y, w - 2.0, 1.0], 0xFF000000);
    game.native_fill(batch, [x + 1.0, y + h - 1.0, w - 2.0, 1.0], 0xFF000000);
    game.native_fill(batch, [x, y + 1.0, 1.0, h - 2.0], 0xFF000000);
    game.native_fill(batch, [x + w - 1.0, y + 1.0, 1.0, h - 2.0], 0xFF000000);
    game.native_fill(batch, [x + 1.0, y + 1.0, w - 2.0, h - 2.0], face);
    game.native_fill(batch, [x + 1.0, y + 1.0, w - 3.0, 2.0], 0xFFFFFFFF);
    game.native_fill(batch, [x + 1.0, y + 1.0, 2.0, h - 3.0], 0xFFFFFFFF);
    game.native_fill(batch, [x + 2.0, y + h - 3.0, w - 3.0, 2.0], 0xFF555555);
    game.native_fill(batch, [x + w - 3.0, y + 2.0, 2.0, h - 3.0], 0xFF555555);
}
/// A sunken box: a slot.
pub(super) fn inset(game: &Game<'_>, batch: &mut HudBatch, [x, y, w, h]: Rect, face: u32) {
    game.native_fill(batch, [x, y, w, h], face);
    game.native_fill(batch, [x, y, w - 1.0, 1.0], 0xFF373737);
    game.native_fill(batch, [x, y, 1.0, h - 1.0], 0xFF373737);
    game.native_fill(batch, [x + 1.0, y + h - 1.0, w - 1.0, 1.0], 0xFFFFFFFF);
    game.native_fill(batch, [x + w - 1.0, y + 1.0, 1.0, h - 1.0], 0xFFFFFFFF);
}
/// A slot's box at `(x, y)`: JEI's `slot`, or its `output_slot` (26 px, around it) for a vanilla category's result.
fn slot(game: &Game<'_>, batch: &mut HudBatch, x: f32, y: f32, output: bool) {
    if output && sprite(game, batch, "output_slot", [x - OUTPUT_RIM, y - OUTPUT_RIM, OUTPUT, OUTPUT]) {
        return;
    }
    if !sprite(game, batch, "slot", [x, y, SLOT, SLOT])
        && !game.native_sprite(batch, "gui/sprites/container/slot", [0.0, 0.0, SLOT, SLOT], [x, y, SLOT, SLOT])
    {
        inset(game, batch, [x, y, SLOT, SLOT], 0xFF8B8B8B);
    }
}
/// A stack in an 18 px slot at `(x, y)`, with its count; the white hover over it.
pub(super) fn stack(game: &Game<'_>, batch: &mut HudBatch, s: &Stack, x: f32, y: f32, cursor: [f32; 2]) {
    if game.native_item_stack(batch, &item_stack(s), [x + 1.0, y + 1.0], 16.0) && s.count > 1 {
        let count = s.count.to_string();
        let w = game.native_text_width(&count);
        game.native_text(batch, &count, [x + 18.0 - w, y + 10.0], 0xFFFFFF, true);
    }
    if inside(cursor, [x, y, SLOT, SLOT]) {
        game.native_fill(batch, [x + 1.0, y + 1.0, 16.0, 16.0], 0x80FFFFFF);
    }
}
/// JEI's small arrow button (13 px, `RecipesGui.smallButtonWidth`): its `button_*_v2` and `arrow_*_v2`, else the
/// engine's copy of them.
pub(super) fn button(game: &Game<'_>, batch: &mut HudBatch, r: Rect, next: bool, cursor: [f32; 2]) {
    let hover = inside(cursor, r);
    if !nine(game, batch, if hover { &BUTTON_HOVER } else { &BUTTON }, r) {
        let face = if hover { "horizon/jei/button_highlight_v2" } else { "horizon/jei/button_enabled_v2" };
        game.native_gui_nine_slice(batch, face, r, 0xFFFFFFFF);
    }
    let icon = if next { "arrow_next_v2" } else { "arrow_previous_v2" };
    let at = [r[0] + 2.0, r[1] + 3.0, 9.0, 7.0];
    if !sprite(game, batch, &format!("icons/{icon}"), at) {
        game.native_sprite(batch, &format!("horizon/jei/icons/{icon}"), [0.0, 0.0, 9.0, 7.0], at);
    }
}
pub(super) fn centred(game: &Game<'_>, batch: &mut HudBatch, text: &str, r: Rect, rgb: u32) {
    let w = game.native_text_width(text);
    game.native_text(batch, text, [(r[0] + (r[2] - w) / 2.0).floor(), r[1] + ((r[3] - 8.0) / 2.0).floor()], rgb, true);
}
/// `text` cut at spaces into lines `room` wide.
fn wrap(game: &Game<'_>, text: &str, room: f32) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let next = if cur.is_empty() { word.to_owned() } else { format!("{cur} {word}") };
        if !cur.is_empty() && game.native_text_width(&next) > room {
            out.push(std::mem::replace(&mut cur, word.to_owned()));
        } else {
            cur = next;
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// A recipe's note lines under it, and a furnace's experience and time, which JEI draws in its corners.
struct Notes {
    corner: Option<(String, String)>,
    lines: Vec<String>,
}
fn notes(game: &Game<'_>, v: &View, room: f32) -> Notes {
    let mut lines: Vec<&str> = v.note.lines().collect();
    let mut corner = None;
    if v.shape == Shape::Cooking && !lines.is_empty() {
        let first = lines.remove(0);
        corner = Some(match first.split_once(", ") {
            Some((xp, time)) => (xp.to_owned(), time.to_owned()),
            None => (String::new(), first.to_owned()),
        });
    }
    Notes { corner, lines: lines.iter().flat_map(|l| wrap(game, l, room)).collect() }
}

/// The window over the open screen: `native_screen_render_post`.
pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    let mut guard = shown();
    let Some(s) = guard.as_mut() else { return };
    s.drawn_at = Instant::now();
    let cursor = screen.mouse;
    let panel = game.native_panel_area(ID);
    let (sw, sh) = (batch.width.floor(), batch.height.floor());
    let page = &s.page;
    let laid: Vec<_> = page.views.iter().map(|v| places(v.shape, v.width, v.height, v.slots.len())).collect();
    let widest = laid.iter().map(|p| p.size[0] + RECIPE_BUTTON_SPACING + RECIPE_BUTTON).fold(0.0, f32::max);
    let sizes = Sizes {
        screen: [sw, sh],
        panel_left: panel.map(|a| a[0]),
        tabs: page.tabs.len(),
        tab: page.tab,
        catalysts: page.catalysts.len(),
        widest,
    };
    let w = window(&sizes);
    let room = w.recipes[2];
    let notes: Vec<Notes> = page.views.iter().map(|v| notes(game, v, room)).collect();
    let heights: Vec<f32> = laid
        .iter()
        .zip(&notes)
        .map(|(p, n)| p.size[1] + if n.lines.is_empty() { 0.0 } else { 2.0 + n.lines.len() as f32 * LINE })
        .collect();
    let starts = pages(&heights, w.recipes[3]);
    s.recipe_page = s.recipe_page.min(starts.len() - 1);
    let first = starts[s.recipe_page];
    let end = starts.get(s.recipe_page + 1).copied().unwrap_or(page.views.len());
    let mut hits: Vec<(Rect, Hit)> = Vec::new();

    // RecipesGui.renderMenuBackground: the screen darkened, the item list left as it is.
    game.native_fill(batch, [0.0, 0.0, panel.map_or(sw, |a| a[0]), sh], 0xE0101010);
    // JEI's sprites: the window's `gui_background_v2`, then the tabs over its top edge (`tab_selected` joins it),
    // the catalysts' `catalyst_tab_v2` with one `recipe_catalyst_slot_background_v2` round their slots. Without
    // them, fills: the tabs not selected behind the window, the selected one joined to it.
    let art = carried(game);
    let tab_label = |i: usize| format!("{} tab", name(&page.tabs[i].item));
    if art {
        if let Some(area) = w.catalyst_area {
            nine(game, batch, &CATALYST_TAB, area);
            nine(game, batch, &CATALYST_SLOTS, [area[0] + 5.0, area[1] + 5.0, area[2] - 10.0, area[3] - 10.0]);
        }
        nine(game, batch, &BACKGROUND, w.area);
    } else {
        for (k, r) in w.tabs.iter().enumerate() {
            if w.first_tab + k != page.tab {
                raised(game, batch, *r, TAB_FACE);
            }
        }
        if let Some(area) = w.catalyst_area {
            raised(game, batch, area, FACE);
        }
        raised(game, batch, w.area, FACE);
    }
    for (k, r) in w.tabs.iter().enumerate() {
        let i = w.first_tab + k;
        if art {
            sprite(game, batch, if i == page.tab { "tab_selected" } else { "tab_unselected" }, *r);
        } else if i == page.tab {
            raised(game, batch, *r, FACE);
            game.native_fill(batch, [r[0] + 1.0, r[1] + r[3] - 4.0, r[2] - 2.0, 5.0], FACE);
        }
        stack(game, batch, &page.tabs[i], r[0] + 3.0, r[1] + 3.0, [-1.0, -1.0]);
        hits.push((*r, Hit::Do(Action::Tab(i))));
        horizon_client::widgets::record(&tab_label(i), *r, true);
    }
    for (c, r) in page.catalysts.iter().zip(&w.catalysts) {
        if !art {
            slot(game, batch, r[0], r[1], false);
        }
        stack(game, batch, c, r[0], r[1], cursor);
        hits.push((*r, Hit::Stack(c.clone())));
        horizon_client::widgets::record(&format!("{} catalyst", name(&c.item)), *r, true);
    }

    // The title between the category arrows, the page row under it, each on JEI's band.
    let rows = [(w.title, w.category_arrows, "Category"), (w.page, w.page_arrows, "Page")];
    for (band, arrows, what) in rows {
        game.native_fill(batch, band, BAND);
        for (n, r) in arrows.iter().enumerate() {
            button(game, batch, *r, n == 1, cursor);
            let by = if n == 1 { 1 } else { -1 };
            let action = if what == "Page" { Action::Page(by) } else { Action::Category(by) };
            hits.push((*r, Hit::Do(action)));
            let label = format!("{} {what}", if n == 1 { "Next" } else { "Previous" });
            horizon_client::widgets::record(&label, *r, true);
        }
    }
    let title = fit(game, &page.title, w.title[2] - 4.0);
    centred(game, batch, &title, w.title, TITLE);
    centred(game, batch, &format!("{}/{}", s.recipe_page + 1, starts.len()), w.page, TITLE);

    // The recipes of this page, spread over the room, each centred.
    let tops = spread(&heights[first..end], w.recipes[1], w.recipes[3]);
    let mut hover: Option<Stack> = None;
    let mut button_tip: Option<Vec<Text>> = None;
    for (i, top) in (first..end).zip(tops) {
        let (v, p, n) = (&page.views[i], &laid[i], &notes[i]);
        let x = (w.recipes[0] + (room - p.size[0] - RECIPE_BUTTON_SPACING - RECIPE_BUTTON) / 2.0).floor();
        // JEI's `recipe_arrow` and `icons/flame`, else the furnace's (its arrow's tail cut to JEI's 22 px).
        let arrow = [x + p.arrow[0], top + p.arrow[1], ARROW[0], ARROW[1]];
        if !sprite(game, batch, "recipe_arrow", arrow) {
            game.native_sprite(batch, "gui/sprites/container/furnace/burn_progress", [24.0 - ARROW[0], 0.0, ARROW[0], ARROW[1]], arrow);
        }
        if let Some([fx, fy]) = p.flame {
            let at = [x + fx, top + fy, 14.0, 14.0];
            if !sprite(game, batch, "icons/flame", at) {
                game.native_sprite(batch, "gui/sprites/container/furnace/lit_progress", [0.0, 0.0, 14.0, 14.0], at);
            }
        }
        let stacks = v.slots.iter().map(Vec::as_slice).chain(std::iter::once(v.results.as_slice()));
        let result = p.slots.len().saturating_sub(1);
        for (k, (list, at)) in stacks.zip(&p.slots).enumerate() {
            let (sx, sy) = (x + at[0], top + at[1]);
            slot(game, batch, sx, sy, k == result && output_slot(v.shape));
            let Some(st) = now(list, s.since) else { continue };
            stack(game, batch, st, sx, sy, cursor);
            let r = [sx, sy, SLOT, SLOT];
            if inside(cursor, r) {
                hover = Some(st.clone());
            }
            hits.push((r, Hit::Stack(st.clone())));
            horizon_client::widgets::record(&format!("{} {i}:{k}", name(&st.item)), [sx + 1.0, sy + 1.0, 16.0, 16.0], true);
        }
        if let Some((xp, time)) = &n.corner {
            let right = x + p.size[0];
            game.native_text(batch, xp, [right - game.native_text_width(xp), top], NOTE, false);
            game.native_text(batch, time, [right - game.native_text_width(time), top + p.size[1] - 8.0], NOTE, false);
        }
        for (l, line) in n.lines.iter().enumerate() {
            let y = top + p.size[1] + 2.0 + l as f32 * LINE;
            game.native_text(batch, line, [w.recipes[0] + ((room - game.native_text_width(line)) / 2.0).floor(), y], NOTE, false);
        }
        // JEI's "+" right of the recipe (`RecipeLayoutWithButtons`), hidden when the open screen takes no such
        // recipe; over it, the slots it lacks in red (`recipeTransferMissingSlotHighlight`) and AE2's craftable
        // ones in blue.
        if let Some(a) = s.answers.iter().find(|(k, _)| *k == i).map(|(_, a)| a)
            && a.handler.is_some()
        {
            let r = [x + p.size[0] + RECIPE_BUTTON_SPACING, top + p.size[1] - RECIPE_BUTTON, RECIPE_BUTTON, RECIPE_BUTTON];
            let inputs = jei_core::window::inputs(v.shape, v.width, v.height).min(v.slots.len());
            let (active, highlight) = button_state(a, v.slots[..inputs].iter().filter(|l| !l.is_empty()).count());
            let over = inside(cursor, r);
            let face = if !active { &BUTTON_DISABLED } else if over { &BUTTON_HOVER } else { &BUTTON };
            if !nine(game, batch, face, r) {
                let name = if !active { "button_disabled_v2" } else if over { "button_highlight_v2" } else { "button_enabled_v2" };
                game.native_gui_nine_slice(batch, &format!("horizon/jei/{name}"), r, 0xFFFFFFFF);
            }
            if let Some(color) = highlight {
                game.native_fill(batch, r, color);
            }
            let icon = [r[0] + 3.0, r[1] + 3.0, 7.0, 7.0];
            if !sprite(game, batch, "icons/recipe_transfer", icon) {
                centred(game, batch, "+", r, if active { 0xFFFFFF } else { 0xA0A0A0 });
            }
            horizon_client::widgets::record(&format!("Move Items {i}"), r, active);
            if active {
                hits.push((r, Hit::Do(Action::Transfer(i))));
            }
            if over {
                for (k, at) in p.slots.iter().enumerate().take(inputs) {
                    let red = a.missing.contains(&k);
                    if red || a.craftable.contains(&k) {
                        let color = if red { 0x66FF0000 } else { 0x400000FF };
                        game.native_fill(batch, [x + at[0] + 1.0, top + at[1] + 1.0, 16.0, 16.0], color);
                    }
                }
                let small = a.handler == Some(NativeTransferHandler::CraftingGrid);
                button_tip = Some(button_tooltip(a, active, small, game.native_has_control_down()));
            }
        }
    }
    if hover.is_none()
        && let Some((_, Hit::Stack(st))) = hits.iter().find(|(r, h)| matches!(h, Hit::Stack(_)) && inside(cursor, *r))
    {
        hover = Some(st.clone());
    }
    if let Some(lines) = button_tip {
        game.native_gui_tooltip(batch, &lines, cursor);
    } else if let Some(st) = hover {
        game.native_gui_tooltip(batch, &[Text::literal(name(&st.item))], cursor);
    }
    s.drawn = Drawn { window: w, pages: starts.len(), hits, panel, shown: first..end };
}

/// `text` cut with vanilla's ellipsis to `room`.
fn fit(game: &Game<'_>, text: &str, room: f32) -> String {
    if game.native_text_width(text) <= room {
        return text.to_owned();
    }
    let mut kept: String = text.to_owned();
    while !kept.is_empty() && game.native_text_width(&format!("{kept}...")) > room {
        kept.pop();
    }
    format!("{}...", kept.trim_end())
}

// ---- Input -----------------------------------------------------------------------------------------------------

/// Whether `at` is the item list's or the bookmark list's (where the window leaves it), whose clicks go through.
fn on_list(d: &Drawn, at: [f32; 2]) -> bool {
    d.panel.is_some_and(|p| inside(at, p)) || (super::jei_bookmarks::over(at) && !on_window(d, at))
}

/// A stack of the window asks for its recipes or uses, as a click on it does (the bookmark list's clicks).
pub(super) fn ask_open(item: &str, mode: Mode) {
    ask(Action::Open(item.to_owned(), mode));
}
/// The item of the window's stack at `at`, if it is over one (the bookmark key adds it).
pub(super) fn stack_at(at: [f32; 2]) -> Option<String> {
    match hit_at(&shown().as_ref()?.drawn, at)? {
        Hit::Stack(st) => Some(st.item.clone()),
        Hit::Do(_) => None,
    }
}

/// `native_screen_mouse_button_pressed_pre`: a click on a stack shows its recipes (left) or uses (right), on a tab,
/// an arrow it acts; nothing under the window reaches the screen.
pub(super) fn pressed(screen: &NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    let guard = shown();
    let Some(s) = guard.as_ref() else { return };
    if *canceled || on_list(&s.drawn, screen.mouse) {
        return;
    }
    *canceled = true;
    match hit_at(&s.drawn, screen.mouse) {
        Some(Hit::Stack(st)) if button == 0 || button == 1 => {
            ask(Action::Open(st.item.clone(), if button == 1 { Mode::Uses } else { Mode::Recipes }))
        }
        Some(Hit::Do(action)) if button == 0 => ask(action.clone()),
        _ => {}
    }
}
/// `native_screen_mouse_button_released_pre`: off the list, the release stays the window's too.
pub(super) fn released(screen: &NativeScreenView<'_>, canceled: &mut bool) {
    if let Some(s) = shown().as_ref()
        && !on_list(&s.drawn, screen.mouse)
    {
        *canceled = true;
    }
}
/// `native_screen_mouse_scrolled_pre`: the wheel over the tabs walks the categories, elsewhere the pages.
pub(super) fn scrolled(screen: &NativeScreenView<'_>, scroll: [f64; 2], canceled: &mut bool) {
    let guard = shown();
    let Some(s) = guard.as_ref() else { return };
    if *canceled || on_list(&s.drawn, screen.mouse) {
        return;
    }
    *canceled = true;
    let by = if scroll[1] > 0.0 { -1 } else if scroll[1] < 0.0 { 1 } else { return };
    let on_tabs = s.drawn.window.tabs.iter().any(|r| inside(screen.mouse, *r));
    ask(if on_tabs { Action::Category(by) } else { Action::Page(by) });
}
/// A key on a screen while the window is open: Escape closes it (JEI's `onClose` returns to the screen under it),
/// Page Up/Down turn its pages, R/U (`mode`) over one of its stacks show that stack's. True when the key was the
/// window's.
pub(super) fn key(key: &str, mode: Option<Mode>, at: [f32; 2]) -> bool {
    let guard = shown();
    let Some(s) = guard.as_ref() else { return false };
    let action = match key {
        "key.keyboard.escape" => Action::Close,
        "key.keyboard.page.up" => Action::Page(-1),
        "key.keyboard.page.down" => Action::Page(1),
        _ => match (mode, hit_at(&s.drawn, at)) {
            (Some(mode), Some(Hit::Stack(st))) => Action::Open(st.item.clone(), mode),
            // Not over the window's stacks: the list's own keys.
            _ => return on_window(&s.drawn, at) && mode.is_some(),
        },
    };
    ask(action);
    true
}

/// The probe's view of the window (`/jeiclient`).
pub(super) fn probe() -> serde_json::Value {
    let guard = shown();
    let Some(s) = guard.as_ref() else { return serde_json::Value::Null };
    serde_json::json!({
        "page": s.recipe_page,
        "pages": s.drawn.pages,
        "area": s.drawn.window.area,
        "list": s.drawn.panel,
        "shown": [s.drawn.shown.start, s.drawn.shown.end],
        "transfers": s.answers.iter().map(|(i, a)| serde_json::json!({
            "recipe": i,
            "handler": a.handler.map(|h| format!("{h:?}")),
            "too_large": a.too_large,
            "missing": a.missing,
            "craftable": a.craftable,
        })).collect::<Vec<_>>(),
    })
}
