// JEI in the game (support module `crate::game::jei_world` of the `jei` native bundle): the item panel beside
// container screens as JEI's ingredient list. The list's order and search are the bundle's crate `jei_core`; this
// module is the glue: the game's items in, the kept ones out to the panel (`native_panel_items`, drawn in the viewer
// style with JEI's artwork), Ctrl+O, cheat mode (`native_give`), the Options > Mods page and the tests' probe. The
// recipe screen (R, U, Backspace) is `jei_recipes`, its window `jei_window`.
use super::Game;
use super::native_hooks::{NativeMod, NativePanelEvent, NativeStack};
use horizon_client::mod_api::NativePanelStyle;
use jei_core::{Item, List, Mode, Modes, Prefix};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) const ID: &str = "jei";
const TITLE: &str = "Just Enough Items 19.70.0.464";
const DESCRIPTION: &str = "JEI's item list beside every container screen and the inventory, with JEI's search \
(@mod, #tag, $tooltip, %tab, ^colour, &id, -word, a | b, \"a phrase\"). Ctrl+O hides it; cheat mode gives items. R shows an item's recipes, U its uses, Backspace goes back, A bookmarks an item (the list on the left).";
/// JEI's keys: name, label (JEI's en_us), default. Ctrl+O is the overlay's (`toggleOverlay`); cheat mode's and the
/// bookmark overlay's are unbound, as in JEI 19.70 (`InternalKeyMappings`).
pub(super) const KEYS: [[&str; 3]; 7] = [
    ["key.jei.toggleOverlay", "Show/Hide Item List (with Ctrl)", "key.keyboard.o"],
    ["key.jei.showRecipe", "Show Recipe", "key.keyboard.r"],
    ["key.jei.showUses", "Show Uses", "key.keyboard.u"],
    ["key.jei.recipeBack", "Show Previous Recipe Page", "key.keyboard.backspace"],
    ["key.jei.bookmark", "Add/Remove Bookmark", "key.keyboard.a"],
    ["key.jei.toggleCheatMode", "Toggle Cheat Mode", "key.keyboard.unknown"],
    ["key.jei.toggleBookmarkOverlay", "Show/Hide Bookmarked Ingredients", "key.keyboard.unknown"],
];
/// The bookmark key's place in `KEYS`.
const BOOKMARK: usize = 4;
/// GLFW's Control bit in the screen hooks' modifiers.
const CONTROL: u32 = 2;

/// The list while a world is open.
struct State {
    list: List,
    query: String,
    /// The places of the items the query keeps: the panel's items, in order.
    shown: Vec<usize>,
    hidden: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
/// Ctrl+O went down on a screen (that hook can't change the panel); the next frame flips the overlay.
static TOGGLE: AtomicBool = AtomicBool::new(false);

fn state() -> std::sync::MutexGuard<'static, Option<State>> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}
fn dev() -> bool {
    std::env::var("JEI_DEV").is_ok_and(|v| v == "1")
}
/// The search modes, `[search]` of `jei-client.ini`.
fn modes(_game: &Game<'_>) -> Modes {
    let mut modes = Modes::default();
    for p in Prefix::ALL {
        if let Some(mode) = Mode::parse(&super::jei_config::value("search", p.key())) {
            modes.set(p, mode);
        }
    }
    modes
}
/// JEI's cheat mode, off at each start (JEI doesn't keep it).
fn cheating(_game: &Game<'_>) -> bool {
    super::jei_config::cheat()
}
/// Ctrl+O's state, `ingredientList.overlaysEnabled`.
fn overlay_hidden() -> bool {
    !super::jei_config::flag("ingredientList", "overlaysEnabled")
}

/// The panel shows what the state keeps, or nothing while hidden.
fn show(game: &mut Game<'_>, s: &State) {
    let items = if s.hidden {
        Vec::new()
    } else {
        s.shown.iter().map(|&n| NativeStack { item: s.list.id(n).to_owned(), count: 1, components: String::new() }).collect()
    };
    game.native_panel_items(ID, items);
}

/// Each frame: the list follows the world and the mod's on/off; Ctrl+O flips the overlay.
pub(super) fn frame(game: &mut Game<'_>) {
    let tables = if game.native_on(ID) { game.native_recipes() } else { None };
    let mut guard = state();
    match (tables, guard.is_some()) {
        (Some(t), false) => {
            super::jei_config::reload();
            let items = t.items.iter().map(|i| Item { id: i.id.clone(), name: i.name.clone(), tags: i.tags.clone(), ..Item::default() });
            let list = List::new(items.collect());
            let modes = modes(game);
            let shown = list.filter("", &modes);
            let hidden = overlay_hidden();
            game.native_panel_style(ID, NativePanelStyle::Viewer);
            let s = State { list, query: String::new(), shown, hidden };
            show(game, &s);
            *guard = Some(s);
        }
        (None, true) => {
            *guard = None;
            game.native_panel_items(ID, Vec::new());
        }
        _ => {}
    }
    if TOGGLE.swap(false, Ordering::Relaxed)
        && let Some(s) = guard.as_ref()
    {
        super::jei_config::set("ingredientList", "overlaysEnabled", if s.hidden { "true" } else { "false" });
    }
    super::jei_config::frame(game);
    // A value changed (the config screen, Ctrl+O): the search and the overlay follow it at once.
    if super::jei_config::take_changed()
        && let Some(s) = guard.as_mut()
    {
        s.hidden = overlay_hidden();
        s.shown = s.list.filter(&s.query, &modes(game));
        show(game, s);
    }
    let in_world = guard.is_some();
    drop(guard);
    super::jei_recipes::frame(game, in_world);
    super::jei_bookmarks::frame(game, in_world);
}

/// The player's search or click on the panel.
pub(super) fn event(game: &mut Game<'_>, event: &NativePanelEvent) {
    let loaded = state().is_some();
    if !loaded || super::jei_recipes::event(game, event) {
        return;
    }
    // JEI's bookmark key over an ingredient (the list's or a slot of the screen's): bookmarked, or taken off again.
    if let NativePanelEvent::Key { key, stack } = event
        && *key == game.native_mod_key(ID, KEYS[BOOKMARK][0], KEYS[BOOKMARK][2])
    {
        super::jei_bookmarks::toggle(&stack.item);
        return;
    }
    let mut guard = state();
    let Some(s) = guard.as_mut() else { return };
    match event {
        NativePanelEvent::Search(text) => {
            s.query = text.clone();
            s.shown = s.list.filter(text, &modes(game));
            show(game, s);
        }
        NativePanelEvent::Clicked { index, right } if cheating(game) => {
            let Some(&n) = s.shown.get(*index as usize) else { return };
            let id = s.list.id(n).to_owned();
            drop(guard);
            if let Err(e) = game.native_give(&id, jei_core::give_count(*right, false)) {
                game.native_chat(&e);
            }
        }
        // Without cheat mode a click shows the item's recipes, a right click its uses.
        NativePanelEvent::Clicked { index, right } => {
            let Some(&n) = s.shown.get(*index as usize) else { return };
            let id = s.list.id(n).to_owned();
            drop(guard);
            let mode = if *right { jei_core::recipes::Mode::Uses } else { jei_core::recipes::Mode::Recipes };
            super::jei_recipes::open(game, &id, mode);
        }
        _ => {}
    }
}

/// `screen_key_pressed_pre`: Ctrl and JEI's overlay key on any screen; Backspace with the recipes open, and the
/// recipe window's keys (Escape, Page Up/Down, R/U over its stacks); not while the search box takes the keys.
pub(super) fn key_pressed(game: &Game<'_>, key: &str, repeat: bool, modifiers: u32, canceled: &mut bool) {
    if *canceled || repeat || !game.native_on(ID) || state().is_none() {
        return;
    }
    if super::jei_config::key(key) {
        *canceled = true;
        return;
    }
    let bound = |k: usize| game.native_mod_key(ID, KEYS[k][0], KEYS[k][2]);
    if !game.mod_browser_typing() && key == bound(5) {
        super::jei_config::set_cheat(!super::jei_config::cheat());
        *canceled = true;
    } else if !game.mod_browser_typing() && key == bound(6) {
        super::jei_config::toggle_bookmarks();
        *canceled = true;
    } else if modifiers & CONTROL != 0 && key == bound(0) {
        TOGGLE.store(true, Ordering::Relaxed);
        *canceled = true;
    } else if modifiers & CONTROL == 0 && key == bound(3) && !game.mod_browser_typing() && super::jei_recipes::is_open() {
        super::jei_recipes::ask_back();
        *canceled = true;
    } else if modifiers & CONTROL == 0 && key == bound(BOOKMARK) && !game.mod_browser_typing() {
        // The bookmark key over a bookmark takes it off, over a stack of the recipe window it adds it (the list's
        // and the screen's slots reach `event` as the panel's `Key`).
        let at = game.native_cursor();
        if let Some(item) = super::jei_bookmarks::slot_at(at).or_else(|| super::jei_window::stack_at(at)) {
            super::jei_bookmarks::toggle(&item);
            *canceled = true;
        }
    } else if !game.mod_browser_typing() && super::jei_window::is_open() {
        let mode = super::jei_recipes::key_mode(game, key);
        if super::jei_window::key(key, mode, game.native_cursor()) {
            *canceled = true;
        }
    }
}

/// The `recipe_lookup` hook: another mod's screen asks for a stack's recipes or uses.
pub(super) fn recipe_lookup(
    game: &mut Game<'_>,
    stack: &NativeStack,
    lookup: &super::native_hooks::NativeRecipeLookup,
    handled: &mut bool,
) {
    if state().is_none() {
        return;
    }
    // Another mod's screen asks what the bookmark key over a stack does: it bookmarks it.
    if let super::native_hooks::NativeRecipeLookup::Key(key) = lookup
        && !*handled
        && *key == game.native_mod_key(ID, KEYS[BOOKMARK][0], KEYS[BOOKMARK][2])
    {
        super::jei_bookmarks::toggle(&stack.item);
        *handled = true;
        return;
    }
    super::jei_recipes::lookup(game, stack, lookup, handled);
}

/// The mod's row in Options > Mods, with its key.
pub(super) fn listed(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: ID.into(),
        title: TITLE.into(),
        description: DESCRIPTION.into(),
        keys: KEYS.iter().map(|k| k.map(String::from)).collect(),
    });
}

fn prefix_label(p: Prefix) -> &'static str {
    match p {
        Prefix::ModName => "Mod name",
        Prefix::Tag => "Tag",
        Prefix::Tooltip => "Tooltip",
        Prefix::CreativeTab => "Creative tab",
        Prefix::Color => "Colour",
        Prefix::Identifier => "Resource id",
    }
}
fn mode_label(m: Mode) -> &'static str {
    match m {
        Mode::Enabled => "Enabled",
        Mode::RequirePrefix => "Require prefix",
        Mode::Disabled => "Disabled",
    }
}

/// The mod's page in Options > Mods: cheat mode, the overlay, then a search mode per prefix (the same values as JEI's
/// config screen, which the item list's wrench opens).
pub(super) fn settings(game: &mut Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id != ID {
        return;
    }
    buttons.push(format!("Cheat Mode: {}", if cheating(game) { "On" } else { "Off" }));
    let hidden = overlay_hidden();
    buttons.push(format!("Item List: {}", if hidden { "Hidden" } else { "Shown" }));
    let modes = modes(game);
    for p in Prefix::ALL {
        buttons.push(format!("{} search ({}): {}", prefix_label(p), p.char(), mode_label(modes.get(p))));
    }
}

pub(super) fn settings_click(game: &mut Game<'_>, mod_id: &str, button: usize) {
    if mod_id != ID {
        return;
    }
    match button {
        0 => super::jei_config::set_cheat(!cheating(game)),
        1 => TOGGLE.store(true, Ordering::Relaxed),
        n if n - 2 < Prefix::ALL.len() => {
            let p = Prefix::ALL[n - 2];
            let next = modes(game).get(p).next();
            super::jei_config::set("search", p.key(), next.name());
        }
        _ => {}
    }
}

/// The client's own commands (the tests' probe, `JEI_DEV=1`).
pub(super) fn client_commands(commands: &mut Vec<&'static str>) {
    if dev() {
        commands.push("jeiclient");
    }
}

/// `/jeiclient` puts `JEI {json}` in chat: the list as this client holds it. `/jeiclient cheat on|off` sets cheat
/// mode.
pub(super) fn client_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    let mut words = command.split_whitespace();
    if *ran || !dev() || words.next() != Some("jeiclient") {
        return;
    }
    *ran = true;
    if words.next() == Some("cheat") {
        super::jei_config::set_cheat(words.next() == Some("on"));
    }
    let modes = modes(game);
    let json = match state().as_ref() {
        None => serde_json::json!({ "loaded": false }),
        Some(s) => serde_json::json!({
            "loaded": true,
            "hidden": s.hidden,
            "cheat": cheating(game),
            "query": s.query,
            "total": s.list.len(),
            "shown": s.shown.len(),
            "first": s.shown.iter().take(12).map(|&n| s.list.id(n)).collect::<Vec<_>>(),
            "modes": Prefix::ALL.map(|p| modes.get(p).name()),
            "recipes": super::jei_recipes::probe(),
            "bookmarks": super::jei_bookmarks::probe(),
            "config": super::jei_config::probe(),
        }),
    };
    game.native_chat(&format!("JEI {json}"));
}
