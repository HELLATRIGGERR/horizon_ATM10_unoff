// JEI's bookmarks in the game (support module `crate::game::jei_bookmarks` of the `jei` native bundle): the list the
// bookmark key (A) adds the hovered ingredient to, drawn left of the open container screen as JEI's bookmark list
// (its `bookmark_list_background_v2` and `bookmark_list_slot_background_v2` from the carry, a page row, slots in
// pages like the item list), and kept per world in JEI 19.70's own file: `config/jei/world/local/<world folder>/
// bookmarks.json` (`BookmarkJsonConfig`; a legacy `bookmarks.ini` there is imported once and backed up as
// `bookmarks.ini.bak`). A click on a bookmark shows its recipes (left) or uses (right) as on the item list; the
// bookmark key over a bookmark takes it off, over an ingredient anywhere else it adds it. The state is the
// bundle's crate `jei_core::bookmarks`; the screen hooks only read the game, so a click leaves an `Action` for
// `jei_recipes::frame` (through `jei_window::ask_open`).
use super::{Game, native_hooks::NativeRecipes};
use super::jei_window::{self, BAND, BOOKMARK_LIST, BOOKMARK_SLOTS};
use horizon_client::{hud::HudBatch, text::Text};
use mod_api::{NativePanelAlign, ScreenView};
use jei_core::bookmarks::{Bookmarks, NAV, SLOT, grid_within, pages};
use jei_core::recipes::{Mode, Stack};
use jei_core::window::{Rect, inside};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The border round the grid (JEI's `BORDER_MARGIN`).
const PAD: f32 = 6.0;
/// A page arrow's size (`RecipesGui.smallButtonWidth`).
const ARROW: f32 = 13.0;

/// The list as last drawn: for the clicks, keys and the probe.
#[derive(Default)]
struct Drawn {
    /// The background, with its page row.
    area: Option<Rect>,
    slots: Vec<(Rect, String)>,
    prev: Rect,
    next: Rect,
    pages: usize,
    at: Option<Instant>,
}

#[derive(Default)]
struct State {
    bookmarks: Bookmarks,
    /// The world folder the bookmarks were loaded for, and their file.
    world: Option<PathBuf>,
    file: Option<PathBuf>,
    page: usize,
    drawn: Drawn,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn state() -> std::sync::MutexGuard<'static, Option<State>> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}
fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    f(state().get_or_insert_with(State::default))
}

// ---- The file --------------------------------------------------------------------------------------------------

/// `bookmarks.json` of this world, as `BookmarkJsonConfig.getPath` puts it under `ServerConfigPathUtil.getWorldPath`:
/// `config/jei/world/local/<the world's folder name>/`.
fn world_file(world: &Path) -> Option<PathBuf> {
    let name = world.file_name()?.to_string_lossy().into_owned();
    let config = match Game::native_config_dir() {
        Ok(dir) => dir,
        Err(e) => {
            log::warn!("[jei] no config folder ({e}): the bookmarks are not kept");
            return None;
        }
    };
    Some(config.join("jei").join("world").join("local").join(name).join("bookmarks.json"))
}

/// The file's text written beside it and moved over it, as `JsonArrayFileHelper.write` does.
fn save(file: &Path, bookmarks: &Bookmarks) {
    let tmp = file.with_file_name("bookmarks.json.tmp");
    let done = file
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&tmp, bookmarks.write_json()))
        .and_then(|()| std::fs::rename(&tmp, file));
    if let Err(e) = done {
        log::warn!("[jei] bookmarks not saved to {}: {e}", file.display());
    }
}

/// The world's bookmarks: its `bookmarks.json`, then the legacy `bookmarks.ini` next to it, imported and moved to
/// `bookmarks.ini.bak` (JEI's `loadBookmarks`), the result saved at once.
fn load(file: &Path) -> Bookmarks {
    let mut bookmarks = match std::fs::read_to_string(file) {
        Ok(text) if text.trim().is_empty() || text.trim_start().starts_with('[') => Bookmarks::parse_json(&text),
        Ok(_) => {
            // Not JEI's array: keep it aside rather than write over it.
            let _ = std::fs::rename(file, file.with_file_name("bookmarks.json.bak"));
            Bookmarks::default()
        }
        Err(_) => Bookmarks::default(),
    };
    let ini = file.with_file_name("bookmarks.ini");
    if let Ok(text) = std::fs::read_to_string(&ini) {
        bookmarks.merge(Bookmarks::import_ini(&text));
        let _ = std::fs::rename(&ini, file.with_file_name("bookmarks.ini.bak"));
        save(file, &bookmarks);
    }
    bookmarks
}

/// The folder of the world the integrated server has open. The window's own game has no `world_root` (the world
/// lives on the server thread's), so the server thread's `server_started` hook leaves it here, in the same process.
static OPEN_WORLD: Mutex<Option<PathBuf>> = Mutex::new(None);

/// `native_server_started` (server side): the open world's folder is known from when the server has it open until it
/// closes.
pub(super) fn opened(game: &mut Game<'_>) {
    *OPEN_WORLD.lock().unwrap_or_else(|e| e.into_inner()) = game.native_world_dir();
}

/// `native_world_lifecycle`: the world closed (or failed to open); the bookmarks are no longer its.
pub(super) fn closed() {
    *OPEN_WORLD.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// The tables the item names were last taken from.
static NAMED_FROM: Mutex<Option<Arc<NativeRecipes>>> = Mutex::new(None);

/// The names of the items for the labels and tooltips, from the game's tables: the recipe window takes them only
/// when it first opens, and the bookmarks are drawn before that.
fn names(game: &mut Game<'_>) {
    let Some(tables) = game.native_recipes() else { return };
    let mut seen = NAMED_FROM.lock().unwrap_or_else(|e| e.into_inner());
    if seen.as_ref().is_some_and(|s| Arc::ptr_eq(s, &tables)) {
        return;
    }
    jei_window::set_names(Arc::new(tables.items.iter().map(|i| (i.id.clone(), i.name.clone())).collect()));
    *seen = Some(tables);
}

/// Each frame (from `jei_world::frame`): the bookmarks follow the world; outside one they go.
pub(super) fn frame(game: &mut Game<'_>, in_world: bool) {
    if in_world {
        names(game);
    }
    let world = if in_world { OPEN_WORLD.lock().unwrap_or_else(|e| e.into_inner()).clone() } else { None };
    let mut guard = state();
    if guard.as_ref().is_some_and(|s| s.world == world) {
        return;
    }
    let file = world.as_deref().and_then(world_file);
    let bookmarks = file.as_deref().map(load).unwrap_or_default();
    *guard = Some(State { bookmarks, world, file, ..State::default() });
}

// ---- Adding and removing ---------------------------------------------------------------------------------------

/// The bookmark key on `item`: it is added, or taken off when it is bookmarked already; the file is written.
pub(super) fn toggle(item: &str) -> bool {
    with(|s| {
        let front = super::jei_config::value("bookmarkList", "addBookmarksToFrontEnabled") == "FRONT";
        let added = s.bookmarks.toggle_at(item, front);
        if let Some(file) = &s.file {
            save(file, &s.bookmarks);
        }
        added
    })
}

/// The bookmark under `at`, if the list is drawn there.
pub(super) fn slot_at(at: [f32; 2]) -> Option<String> {
    if jei_window::covers(at) {
        return None;
    }
    let guard = state();
    let d = &guard.as_ref()?.drawn;
    fresh(d).then(|| d.slots.iter().find(|(r, _)| inside(at, *r)).map(|(_, item)| item.clone())).flatten()
}

/// Whether the list was drawn a moment ago (not while another screen is up).
fn fresh(d: &Drawn) -> bool {
    d.at.is_some_and(|t| t.elapsed() < Duration::from_millis(500))
}
/// Whether `at` is on the list: its background and page row.
pub(super) fn over(at: [f32; 2]) -> bool {
    state().as_ref().is_some_and(|s| fresh(&s.drawn) && s.drawn.area.is_some_and(|a| inside(at, a)))
}

// ---- Drawing ---------------------------------------------------------------------------------------------------

/// The list left of the open container screen, under the item list's top: `native_screen_render_post`.
pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &ScreenView<'_>) {
    let covered = jei_window::covers(screen.mouse);
    let mut guard = state();
    let s = guard.get_or_insert_with(State::default);
    s.drawn = Drawn::default();
    let Some(list) = game.native_panel_area(super::jei_world::ID) else { return };
    // JEI's bookmark overlay toggle (the config screen's "Show Bookmark Overlay", `key.jei.toggleBookmarkOverlay`).
    if !super::jei_config::bookmarks_shown() {
        return;
    }
    let (max_columns, max_rows) = super::jei_config::grid_limits("bookmarkList");
    let items: Vec<String> = s.bookmarks.items().into_iter().map(str::to_owned).collect();
    if items.is_empty() {
        return;
    }
    // Left of the screen's panel, as high as the item list: `[8, list.y, screen.x - 16, list.h]`.
    let outer = [8.0, list[1], screen.rect[0] - 16.0, list[3]];
    let room = outer[2] - 2.0 * PAD;
    let columns = ((room / SLOT).floor().max(0.0) as usize).min(max_columns);
    if columns < 2 {
        return;
    }
    // Aligned as `[bookmarkList]` says (JEI's default and ATM10's: left, top), as the engine aligns the item list.
    let align = |key, default| NativePanelAlign::parse(&super::jei_config::value("bookmarkList", key)).unwrap_or(default);
    let x = (outer[0] + PAD + align("horizontalAlignment", NativePanelAlign::Start).offset(room, columns as f32 * SLOT)).floor();
    let rows = (((outer[3] - 2.0 * PAD - NAV) / SLOT).floor().max(0.0) as usize).min(max_rows);
    let dy = align("verticalAlignment", NativePanelAlign::Start).offset(outer[3], 2.0 * PAD + NAV + rows as f32 * SLOT).floor();
    let outer = [outer[0], outer[1] + dy, outer[2], outer[3] - dy];
    let slots = grid_within([x, outer[1] + PAD, columns as f32 * SLOT, outer[3] - 2.0 * PAD], max_columns, max_rows);
    let per_page = slots.len();
    if per_page == 0 {
        return;
    }
    let n_pages = pages(items.len(), per_page);
    s.page = s.page.min(n_pages - 1);
    let rows = per_page / columns;
    let width = columns as f32 * SLOT;
    let area = [x - PAD, outer[1], width + 2.0 * PAD, 2.0 * PAD + NAV + rows as f32 * SLOT];
    let cursor = screen.mouse;

    if !jei_window::nine(game, batch, &BOOKMARK_LIST, area) {
        game.native_fill(batch, area, 0xC0303030);
    }
    // The page row: JEI's arrows round the "n/m" on its band.
    let nav_y = outer[1] + PAD + 3.0;
    let prev = [x, nav_y, ARROW, ARROW];
    let next = [x + width - ARROW, nav_y, ARROW, ARROW];
    game.native_fill(batch, [x, nav_y, width, ARROW], BAND);
    jei_window::button(game, batch, prev, false, cursor);
    jei_window::button(game, batch, next, true, cursor);
    jei_window::centred(game, batch, &format!("{}/{}", s.page + 1, n_pages), [x, nav_y, width, ARROW], 0xFFFFFF);
    horizon_client::widgets::record("Previous Bookmark Page", prev, true);
    horizon_client::widgets::record("Next Bookmark Page", next, true);

    let first = s.page * per_page;
    let shown = items.len().saturating_sub(first).min(per_page);
    let grid_rect = [x, outer[1] + PAD + NAV, width, rows as f32 * SLOT];
    if !jei_window::nine(game, batch, &BOOKMARK_SLOTS, grid_rect) {
        for at in &slots {
            jei_window::inset(game, batch, [at[0], at[1], SLOT, SLOT], 0xFF8B8B8B);
        }
    }
    let mut hits = Vec::with_capacity(shown);
    let mut hover = None;
    for (item, at) in items[first..first + shown].iter().zip(&slots) {
        let r = [at[0], at[1], SLOT, SLOT];
        jei_window::stack(game, batch, &Stack::of_key(item), at[0], at[1], cursor);
        if inside(cursor, r) {
            hover = Some(item.clone());
        }
        horizon_client::widgets::record(&format!("{} bookmark", jei_window::name(item)), [r[0] + 1.0, r[1] + 1.0, 16.0, 16.0], true);
        hits.push((r, item.clone()));
    }
    if let Some(item) = hover
        && !covered
    {
        game.native_gui_tooltip(batch, &[Text::literal(jei_window::name(&item))], cursor);
    }
    s.drawn = Drawn { area: Some(area), slots: hits, prev, next, pages: n_pages, at: Some(Instant::now()) };
}

// ---- Input -----------------------------------------------------------------------------------------------------

/// `native_screen_mouse_button_pressed_pre`: a click on a bookmark shows its recipes (left) or uses (right), on an
/// arrow it turns the page; the list takes every click on it.
pub(super) fn pressed(screen: &ScreenView<'_>, button: i32, canceled: &mut bool) {
    let at = screen.mouse;
    if *canceled || !over(at) || jei_window::covers(at) {
        return;
    }
    *canceled = true;
    let mut guard = state();
    let Some(s) = guard.as_mut() else { return };
    if let Some((_, item)) = s.drawn.slots.iter().find(|(r, _)| inside(at, *r)) {
        if button == 0 || button == 1 {
            jei_window::ask_open(item, if button == 1 { Mode::Uses } else { Mode::Recipes });
        }
    } else if button == 0 && (inside(at, s.drawn.prev) || inside(at, s.drawn.next)) {
        turn(s, if inside(at, s.drawn.prev) { -1 } else { 1 });
    }
}
/// `native_screen_mouse_button_released_pre`: the release stays the list's too.
pub(super) fn released(screen: &ScreenView<'_>, canceled: &mut bool) {
    if !*canceled && over(screen.mouse) && !jei_window::covers(screen.mouse) {
        *canceled = true;
    }
}
/// `native_screen_mouse_scrolled_pre`: the wheel over the list turns its pages.
pub(super) fn scrolled(screen: &ScreenView<'_>, scroll: [f64; 2], canceled: &mut bool) {
    if *canceled || !over(screen.mouse) || jei_window::covers(screen.mouse) {
        return;
    }
    *canceled = true;
    let by = if scroll[1] > 0.0 { -1 } else if scroll[1] < 0.0 { 1 } else { return };
    with(|s| turn(s, by));
}
/// The page before or after; the last wraps to the first as JEI's arrows do.
fn turn(s: &mut State, by: i32) {
    let n = s.drawn.pages.max(1) as i32;
    s.page = (s.page as i32 + by).rem_euclid(n) as usize;
}

/// The probe's view of the list (`/jeiclient`).
pub(super) fn probe() -> serde_json::Value {
    with(|s| {
        serde_json::json!({
            "items": s.bookmarks.items(),
            "page": s.page,
            "pages": s.drawn.pages,
            "area": s.drawn.area,
            "slots": s.drawn.slots.len(),
            "file": s.file.as_ref().map(|f| f.to_string_lossy().replace('\\', "/")),
        })
    })
}
