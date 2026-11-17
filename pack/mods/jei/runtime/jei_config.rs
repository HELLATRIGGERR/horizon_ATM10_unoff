// JEI's config (support module `crate::game::jei_config` of the `jei` native bundle): `config/jei/jei-client.ini`
// read and written in JEI 19.70's format (`jei_core::config`), the runtime toggles JEI doesn't keep in it (cheat mode,
// the bookmark overlay), and JEI's config screen. The item list's wrench button opens the screen (Ctrl + click
// toggles cheat mode, as JEI's `ConfigButtonController`); it is drawn over the container screen as the recipe window
// is, with JEI's sprites from the carry and the game's buttons for the values. Every change is written to the file at
// once and applied on the next frame (`take_changed`).
use super::Game;
use super::jei_window::{self, BACKGROUND, BUTTON, BUTTON_DISABLED, BUTTON_HOVER, FACE};
use horizon_client::hud::HudBatch;
use horizon_client::mod_api::NativeScreenView;
use horizon_client::text::Text;
use jei_core::config::{CATEGORIES, Ini, OPTS};
use jei_core::window::{Rect, inside};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

static INI: Mutex<Option<Ini>> = Mutex::new(None);
/// JEI's `ClientToggleState`: cheat mode and the bookmark overlay hold for the session only.
static CHEAT: AtomicBool = AtomicBool::new(false);
static BOOKMARKS: AtomicBool = AtomicBool::new(true);
/// A value changed since the last frame took it.
static CHANGED: AtomicBool = AtomicBool::new(false);
/// The wrench was clicked; the next frame opens the screen (or, with Ctrl, toggles cheat mode).
static WRENCH_CLICKED: AtomicBool = AtomicBool::new(false);
static WRENCH: Mutex<Option<Rect>> = Mutex::new(None);
static SCREEN: Mutex<Option<Window>> = Mutex::new(None);

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

// ---- The file ----------------------------------------------------------------------------------------------------

pub(super) fn path() -> Option<PathBuf> {
    Game::native_config_dir().ok().map(|d| d.join("jei").join("jei-client.ini"))
}
fn save(ini: &Ini) {
    let Some(path) = path() else { return };
    let tmp = path.with_extension("ini.tmp");
    let written = path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(&tmp, ini.write()));
    if written.and_then(|()| std::fs::rename(&tmp, &path)).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}
/// The file as read once (a new one at JEI's defaults and ATM10's values; a file missing values gets them).
fn with<R>(f: impl FnOnce(&mut Ini) -> R) -> R {
    let mut guard = lock(&INI);
    let ini = guard.get_or_insert_with(|| match path().map(std::fs::read_to_string) {
        Some(Ok(text)) => {
            let mut ini = Ini::parse(&text);
            if ini.complete() {
                save(&ini);
            }
            ini
        }
        Some(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            let ini = Ini::fresh();
            save(&ini);
            ini
        }
        _ => Ini::fresh(),
    });
    f(ini)
}
/// Reads the file again on its next use (a world opened: changes made outside the game count, as JEI watches it).
pub(super) fn reload() {
    *lock(&INI) = None;
}
pub(super) fn value(section: &str, key: &str) -> String {
    with(|ini| ini.value(section, key))
}
pub(super) fn flag(section: &str, key: &str) -> bool {
    with(|ini| ini.flag(section, key))
}
/// A list's `maxColumns` and `maxRows`.
pub(super) fn grid_limits(section: &str) -> (usize, usize) {
    with(|ini| (ini.int(section, "maxColumns").max(1) as usize, ini.int(section, "maxRows").max(1) as usize))
}
/// Keeps a value: in the file at once, applied on the next frame.
pub(super) fn set(section: &str, key: &str, value: &str) {
    with(|ini| {
        ini.set(section, key, value);
        save(ini);
    });
    CHANGED.store(true, Ordering::Relaxed);
}
pub(super) fn take_changed() -> bool {
    CHANGED.swap(false, Ordering::Relaxed)
}
pub(super) fn cheat() -> bool {
    CHEAT.load(Ordering::Relaxed)
}
pub(super) fn set_cheat(on: bool) {
    CHEAT.store(on, Ordering::Relaxed);
}
pub(super) fn bookmarks_shown() -> bool {
    BOOKMARKS.load(Ordering::Relaxed)
}
pub(super) fn toggle_bookmarks() {
    BOOKMARKS.fetch_xor(true, Ordering::Relaxed);
}

// ---- The screen --------------------------------------------------------------------------------------------------

/// A line of a category: JEI's runtime toggles (`RuntimeToggleScreenValue`) first, then the file's options.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Row {
    Cheat,
    Bookmarks,
    Opt(usize),
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Hit {
    Category(usize),
    Value(Row),
    Done,
}
#[derive(Default)]
struct Window {
    category: usize,
    scroll: usize,
    rows_shown: usize,
    frame: Rect,
    hits: Vec<(Rect, Hit)>,
}

fn rows(category: usize) -> Vec<Row> {
    let id = CATEGORIES[category].0;
    let mut rows = Vec::new();
    match id {
        "cheating" => rows.push(Row::Cheat),
        "bookmarkList" => rows.push(Row::Bookmarks),
        _ => {}
    }
    rows.extend(OPTS.iter().enumerate().filter(|(_, o)| o.section == id).map(|(i, _)| Row::Opt(i)));
    rows
}
/// A row's name, its value's label, description and default.
fn row_text(row: Row) -> (String, String, &'static str, String) {
    let on = |b: bool| if b { "ON" } else { "OFF" }.to_owned();
    match row {
        Row::Cheat => (
            "Enable Cheat Mode".into(),
            on(cheat()),
            "Turn cheat mode on or off for the current session. This value is not saved to the configuration file. The server must allow JEI cheats.",
            "false".into(),
        ),
        Row::Bookmarks => (
            "Show Bookmark Overlay".into(),
            on(bookmarks_shown()),
            "Show or hide the bookmark overlay for the current session when JEI overlays are enabled. This value is not saved to the configuration file.",
            "true".into(),
        ),
        Row::Opt(i) => {
            let o = &OPTS[i];
            (o.name.into(), o.label(&value(o.section, o.key)), o.description, o.default.into())
        }
    }
}
fn press_row(row: Row, back: bool, big: bool) {
    match row {
        Row::Cheat => set_cheat(!cheat()),
        Row::Bookmarks => toggle_bookmarks(),
        Row::Opt(i) => {
            let o = &OPTS[i];
            let next = o.cycle(&value(o.section, o.key), back, big);
            set(o.section, o.key, &next);
        }
    }
    CHANGED.store(true, Ordering::Relaxed);
}

pub(super) fn is_open() -> bool {
    lock(&SCREEN).is_some()
}
pub(super) fn open() {
    *lock(&SCREEN) = Some(Window::default());
}
pub(super) fn close() {
    *lock(&SCREEN) = None;
}
/// Each frame: the wrench's click, with Ctrl down cheat mode (`toggleCheatModeConfigButton`), else the screen.
pub(super) fn frame(game: &Game<'_>) {
    SHIFT.store(game.native_has_shift_down(), Ordering::Relaxed);
    if WRENCH_CLICKED.swap(false, Ordering::Relaxed) {
        if game.native_has_control_down() {
            set_cheat(!cheat());
            CHANGED.store(true, Ordering::Relaxed);
        } else {
            open();
        }
    }
}

const ROW: f32 = 22.0;
const CATEGORY_WIDTH: f32 = 92.0;
const VALUE_WIDTH: f32 = 116.0;
const TEXT: u32 = 0x404040;

fn vanilla_button(game: &Game<'_>, batch: &mut HudBatch, r: Rect, label: &str, cursor: [f32; 2]) {
    let hover = inside(cursor, r);
    let face = if hover { "gui/sprites/widget/button_highlighted" } else { "gui/sprites/widget/button" };
    game.native_gui_nine_slice(batch, face, r, 0xFFFFFFFF);
    let label = jei_window::fit(game, label, r[2] - 8.0);
    jei_window::centred(game, batch, &label, r, if hover { 0xFFFFA0 } else { 0xFFFFFF });
}

/// `screen_render_post`: JEI's look on the item list's wrench (its cheat-mode icon in red), then the screen.
pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    let cursor = screen.mouse;
    let wrench = game.native_panel_area(super::jei_world::ID).map(|a| [a[0] + a[2] - 20.0, a[3] - 26.0, 20.0, 20.0]);
    *lock(&WRENCH) = wrench;
    if let Some(r) = wrench
        && jei_window::carried(game)
    {
        jei_window::nine(game, batch, if inside(cursor, r) { &BUTTON_HOVER } else { &BUTTON }, r);
        let icon = if cheat() { "icons/config_button_cheat" } else { "icons/config_button" };
        jei_window::sprite(game, batch, icon, [r[0] + 2.0, r[1] + 2.0, 16.0, 16.0]);
    }
    let mut guard = lock(&SCREEN);
    let Some(win) = guard.as_mut() else { return };
    let (sw, sh) = (batch.width.floor(), batch.height.floor());
    let (w, h) = ((sw - 16.0).min(400.0), (sh - 16.0).min(260.0));
    let (x, y) = (((sw - w) / 2.0).floor(), ((sh - h) / 2.0).floor());
    win.frame = [x, y, w, h];
    win.hits.clear();
    game.native_fill(batch, [0.0, 0.0, sw, sh], 0x80101010);
    if !jei_window::nine(game, batch, &BACKGROUND, win.frame) {
        game.native_fill(batch, win.frame, FACE);
    }
    // The title on JEI's navigation band.
    let title = [x + 6.0, y + 6.0, w - 12.0, 16.0];
    game.native_fill(batch, title, jei_window::BAND);
    jei_window::centred(game, batch, "JEI Config", title, 0xFFFFFF);
    // The categories, a JEI button each; the open one pressed.
    for (i, (_, name)) in CATEGORIES.iter().enumerate() {
        let r = [x + 6.0, y + 26.0 + i as f32 * 21.0, CATEGORY_WIDTH, 20.0];
        if r[1] + r[3] > y + h - 30.0 {
            break;
        }
        let selected = i == win.category;
        let face = if selected { &BUTTON_DISABLED } else if inside(cursor, r) { &BUTTON_HOVER } else { &BUTTON };
        if !jei_window::nine(game, batch, face, r) {
            game.native_gui_nine_slice(batch, "gui/sprites/widget/button", r, 0xFFFFFFFF);
        }
        jei_window::centred(game, batch, name, r, if selected { 0xFFFFA0 } else { 0xFFFFFF });
        horizon_client::widgets::record(&format!("JEI Config: {name}"), r, true);
        win.hits.push((r, Hit::Category(i)));
    }
    // The category's values: a name, a button with the value.
    let rx = x + 6.0 + CATEGORY_WIDTH + 8.0;
    let rw = x + w - 8.0 - rx;
    let top = y + 26.0;
    let room = y + h - 30.0 - top;
    let all = rows(win.category);
    win.rows_shown = ((room / ROW).floor().max(1.0)) as usize;
    win.scroll = win.scroll.min(all.len().saturating_sub(win.rows_shown));
    let mut hover_row = None;
    for (n, row) in all.iter().skip(win.scroll).take(win.rows_shown).enumerate() {
        let ry = top + n as f32 * ROW;
        let (name, label, _, _) = row_text(*row);
        let name_rect = [rx, ry, rw - VALUE_WIDTH - 4.0, 20.0];
        let shown = jei_window::fit(game, &name, name_rect[2]);
        game.native_text(batch, &shown, [rx, ry + 6.0], TEXT, false);
        let b = [x + w - 8.0 - VALUE_WIDTH, ry, VALUE_WIDTH, 20.0];
        vanilla_button(game, batch, b, &label, cursor);
        horizon_client::widgets::record(&name, b, true);
        win.hits.push((b, Hit::Value(*row)));
        if inside(cursor, name_rect) || inside(cursor, b) {
            hover_row = Some(*row);
        }
    }
    if all.len() > win.rows_shown {
        let pages = format!("{}-{} / {}", win.scroll + 1, (win.scroll + win.rows_shown).min(all.len()), all.len());
        game.native_text(batch, &pages, [rx, y + h - 22.0], TEXT, false);
    }
    let done = [x + (w - 100.0) / 2.0, y + h - 26.0, 100.0, 20.0];
    vanilla_button(game, batch, done, "Done", cursor);
    horizon_client::widgets::record("Done", done, true);
    win.hits.push((done, Hit::Done));
    // The hovered value's name, description and default, as MezzConfig's screen tells them.
    if let Some(row) = hover_row {
        let (name, _, description, default) = row_text(row);
        let mut lines = vec![Text::literal(&name)];
        for l in jei_window::wrap(game, description, 200.0) {
            lines.push(Text::literal(&l));
        }
        lines.push(Text::literal(&format!("Default Value: {default}")));
        game.native_gui_tooltip(batch, &lines, cursor);
    }
}

/// `screen_mouse_button_pressed_pre`: the screen takes every click while open; the wrench opens it.
pub(super) fn pressed(screen: &NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    if *canceled {
        return;
    }
    let at = screen.mouse;
    let mut guard = lock(&SCREEN);
    let Some(win) = guard.as_mut() else {
        drop(guard);
        if button == 0 && lock(&WRENCH).is_some_and(|r| inside(at, r)) {
            WRENCH_CLICKED.store(true, Ordering::Relaxed);
            *canceled = true;
        }
        return;
    };
    *canceled = true;
    let hit = win.hits.iter().find(|(r, _)| inside(at, *r)).map(|(_, h)| *h);
    match hit {
        Some(Hit::Category(i)) => {
            win.category = i;
            win.scroll = 0;
        }
        Some(Hit::Value(row)) if button == 0 || button == 1 => {
            drop(guard);
            press_row(row, button == 1, SHIFT.load(Ordering::Relaxed));
        }
        Some(Hit::Done) => *guard = None,
        _ => {}
    }
}
/// Shift as the last frame saw it (the mouse hooks have no modifiers): ten steps for a number.
static SHIFT: AtomicBool = AtomicBool::new(false);
pub(super) fn released(_screen: &NativeScreenView<'_>, canceled: &mut bool) {
    if is_open() {
        *canceled = true;
    }
}
pub(super) fn scrolled(_screen: &NativeScreenView<'_>, scroll: [f64; 2], canceled: &mut bool) {
    let mut guard = lock(&SCREEN);
    let Some(win) = guard.as_mut() else { return };
    *canceled = true;
    let max = rows(win.category).len().saturating_sub(win.rows_shown);
    win.scroll = if scroll[1] > 0.0 { win.scroll.saturating_sub(1) } else { (win.scroll + 1).min(max) };
}
/// A key while the screen is open: Escape closes it; nothing reaches the screen under it.
pub(super) fn key(key: &str) -> bool {
    if !is_open() {
        return false;
    }
    if key == "key.keyboard.escape" {
        close();
    }
    true
}

/// The tests' probe: the file, the toggles, the open screen's category and values.
pub(super) fn probe() -> serde_json::Value {
    let screen = lock(&SCREEN).as_ref().map(|w| {
        let rows: Vec<_> = rows(w.category)
            .into_iter()
            .map(|r| {
                let (name, label, _, _) = row_text(r);
                serde_json::json!({ "name": name, "value": label })
            })
            .collect();
        serde_json::json!({ "category": CATEGORIES[w.category].0, "frame": w.frame, "rows": rows })
    });
    serde_json::json!({
        "file": path().map(|p| p.display().to_string()),
        "cheat": cheat(),
        "bookmarks": bookmarks_shown(),
        "wrench": *lock(&WRENCH),
        "screen": screen,
    })
}
