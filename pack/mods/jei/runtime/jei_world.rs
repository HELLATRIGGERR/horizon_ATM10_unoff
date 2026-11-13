// JEI in the game (support module `crate::game::jei_world` of the `jei` native bundle): the item panel beside
// container screens as JEI's ingredient list. The list's order and search are the bundle's crate `jei_core`; this
// module is the glue: the game's items in, the kept ones out to the panel (`native_panel_items`, drawn in the viewer
// style with JEI's artwork), Ctrl+O, cheat mode (`native_give`), the Options > Mods page and the tests' probe. The
// recipe screen (R, U, Backspace) is `jei_recipes`.
use super::Game;
use super::native_hooks::{NativeMod, NativePanelEvent, NativeStack};
use horizon_client::mod_api::NativePanelStyle;
use jei_core::{Item, List, Mode, Modes, Prefix};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) const ID: &str = "jei";
const TITLE: &str = "Just Enough Items 19.70.0.464";
const DESCRIPTION: &str = "JEI's item list beside every container screen and the inventory, with JEI's search \
(@mod, #tag, $tooltip, %tab, ^colour, &id, -word, a | b, \"a phrase\"). Ctrl+O hides it; cheat mode gives items. R shows an item's recipes, U its uses, Backspace goes back.";
/// JEI's keys: name, label (JEI's en_us), default. Ctrl+O is the overlay's (`toggleOverlay`).
pub(super) const KEYS: [[&str; 3]; 4] = [
    ["key.jei.toggleOverlay", "Show/Hide Item List (with Ctrl)", "key.keyboard.o"],
    ["key.jei.showRecipe", "Show Recipe", "key.keyboard.r"],
    ["key.jei.showUses", "Show Uses", "key.keyboard.u"],
    ["key.jei.recipeBack", "Show Previous Recipe Page", "key.keyboard.backspace"],
];
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
fn modes(game: &Game<'_>) -> Modes {
    let mut modes = Modes::default();
    for p in Prefix::ALL {
        if let Some(mode) = game.native_setting(ID, p.key()).as_deref().and_then(Mode::parse) {
            modes.set(p, mode);
        }
    }
    modes
}
/// JEI's `cheatItemsEnabled`, off by default.
fn cheating(game: &Game<'_>) -> bool {
    game.native_setting(ID, "cheatItemsEnabled").as_deref() == Some("true")
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
            let items = t.items.iter().map(|i| Item { id: i.id.clone(), name: i.name.clone(), tags: i.tags.clone(), ..Item::default() });
            let list = List::new(items.collect());
            let modes = modes(game);
            let shown = list.filter("", &modes);
            let hidden = game.native_setting(ID, "overlayEnabled").as_deref() == Some("false");
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
        && let Some(s) = guard.as_mut()
    {
        s.hidden = !s.hidden;
        game.native_set_setting(ID, "overlayEnabled", if s.hidden { "false" } else { "true" });
        show(game, s);
    }
    let in_world = guard.is_some();
    drop(guard);
    super::jei_recipes::frame(game, in_world);
}

/// The player's search or click on the panel.
pub(super) fn event(game: &mut Game<'_>, event: &NativePanelEvent) {
    let loaded = state().is_some();
    if !loaded || super::jei_recipes::event(game, event) {
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

/// `screen_key_pressed_pre`: Ctrl and JEI's overlay key on any screen; Backspace with the recipes open (not while
/// the search box takes the keys).
pub(super) fn key_pressed(game: &Game<'_>, key: &str, repeat: bool, modifiers: u32, canceled: &mut bool) {
    if *canceled || repeat || !game.native_on(ID) || state().is_none() {
        return;
    }
    let bound = |k: usize| game.native_mod_key(ID, KEYS[k][0], KEYS[k][2]);
    if modifiers & CONTROL != 0 && key == bound(0) {
        TOGGLE.store(true, Ordering::Relaxed);
        *canceled = true;
    } else if modifiers & CONTROL == 0 && key == bound(3) && !game.mod_browser_typing() && super::jei_recipes::is_open() {
        super::jei_recipes::ask_back();
        *canceled = true;
    }
}

/// The `recipe_lookup` hook: another mod's screen asks for a stack's recipes or uses.
pub(super) fn recipe_lookup(
    game: &mut Game<'_>,
    stack: &NativeStack,
    lookup: &super::native_hooks::NativeRecipeLookup,
    handled: &mut bool,
) {
    if state().is_some() {
        super::jei_recipes::lookup(game, stack, lookup, handled);
    }
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

/// The mod's settings page: cheat mode, the overlay, then a search mode per prefix (JEI's config, slice 3 brings
/// its own screen).
pub(super) fn settings(game: &mut Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id != ID {
        return;
    }
    buttons.push(format!("Cheat Mode: {}", if cheating(game) { "On" } else { "Off" }));
    let hidden = state().as_ref().map_or(game.native_setting(ID, "overlayEnabled").as_deref() == Some("false"), |s| s.hidden);
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
        0 => {
            let on = !cheating(game);
            game.native_set_setting(ID, "cheatItemsEnabled", if on { "true" } else { "false" });
        }
        1 => TOGGLE.store(true, Ordering::Relaxed),
        n if n - 2 < Prefix::ALL.len() => {
            let p = Prefix::ALL[n - 2];
            let next = modes(game).get(p).next();
            game.native_set_setting(ID, p.key(), next.name());
            let modes = modes(game);
            if let Some(s) = state().as_mut() {
                s.shown = s.list.filter(&s.query, &modes);
                show(game, s);
            }
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
        let on = words.next() == Some("on");
        game.native_set_setting(ID, "cheatItemsEnabled", if on { "true" } else { "false" });
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
        }),
    };
    game.native_chat(&format!("JEI {json}"));
}
