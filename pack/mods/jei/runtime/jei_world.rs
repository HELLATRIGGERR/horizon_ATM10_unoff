// JEI in the game (support module `crate::game::jei_world` of the `jei` native bundle): the item panel beside
// container screens as JEI's ingredient list. The list's order and search are the bundle's crate `jei_core`; this
// module is the glue: the game's items in, the kept ones out to the panel (`native_panel_items`, drawn in the viewer
// style with JEI's artwork), Ctrl+O, cheat mode (`native_give`), the Options > Mods page and the tests' probe. The
// recipe screen (R, U, Backspace) is `jei_recipes`, its window `jei_window`.
use super::Game;
use super::native_hooks::{NativeMod, NativePanelEvent, NativeStack};
use horizon_client::mod_api::{NativeIngredientInfo, NativePanelAlign, NativePanelGrid, NativePanelIngredient, NativePanelStyle};
use jei_core::blacklist::Blacklist;
use jei_core::{Item, List, Mode, Modes, Prefix, mod_id_format};
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
    /// `config/jei/blacklist.json`, read with the world.
    blacklist: Blacklist,
    /// The mods' categories' version the list was made at: the mods' registered ingredients (Mekanism's chemicals)
    /// move it.
    version: u64,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
/// Ctrl+O went down on a screen (that hook can't change the panel); the next frame flips the overlay.
static TOGGLE: AtomicBool = AtomicBool::new(false);
/// `jei-mod-id-format.ini`'s format as `§` codes, read with the world; empty: no mod name line. Its own lock: a
/// tooltip may be drawn while the state is held.
static MOD_NAME_FORMAT: Mutex<String> = Mutex::new(String::new());

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
/// `[ingredientList] hideIngredientsWhenNotSearching`.
fn hide_unsearched() -> bool {
    super::jei_config::flag("ingredientList", "hideIngredientsWhenNotSearching")
}
/// The places of the items `query` keeps, less the blacklist's unless the Cheats page shows hidden ingredients.
fn filter(list: &List, blacklist: &Blacklist, query: &str, game: &Game<'_>) -> Vec<usize> {
    let mut shown = list.filter(query, &modes(game));
    if !super::jei_config::flag("cheating", "showHiddenIngredients") {
        shown.retain(|&n| !blacklist.hides(list.kind(n), list.id(n)));
    }
    shown
}
/// JEI's ingredient list: the game's items (air left out, JEI's list never holds it), the mods' registered types
/// (Mekanism's chemicals, `native_registered_ingredients`) and the fluids (`native_fluids`, a bucket each), in JEI's
/// order; the panel is told how the ones that are not items look and what their tooltips say (the name, the mod
/// name line), the window their names and sprites (the bookmarks).
fn ingredients(game: &mut Game<'_>, t: &super::native_hooks::NativeRecipes) -> List {
    let mut items: Vec<Item> = t
        .items
        .iter()
        .filter(|i| &*i.id != "minecraft:air")
        .map(|i| Item { id: i.id.clone(), name: i.name.clone(), tags: i.tags.clone(), ..Item::default() })
        .collect();
    let fluids = game.native_fluids().into_iter().map(|id| NativeIngredientInfo {
        kind: NativeStack::FLUID.into(),
        name: super::jei_recipes::fluid_name(&id),
        id,
        ..NativeIngredientInfo::default()
    });
    let format = MOD_NAME_FORMAT.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let mut looks = Vec::new();
    let mut window = std::collections::HashMap::new();
    for info in game.native_registered_ingredients().into_iter().chain(fluids) {
        items.push(Item { id: info.id.clone(), name: info.name.clone(), kind: info.kind.clone(), ..Item::default() });
        let ns = info.id.split_once(':').map_or("minecraft", |(ns, _)| ns);
        let lines = if format.is_empty() { Vec::new() } else { vec![format!("{format}{}", jei_core::mod_name(ns))] };
        window.insert(format!("{} {}", info.kind, info.id), (info.name.clone(), info.sprite.clone(), info.tint));
        looks.push(NativePanelIngredient { info, lines });
    }
    game.native_panel_ingredients(ID, looks);
    super::jei_window::set_list_looks(std::sync::Arc::new(window));
    List::new(items)
}
/// ATM10's `config/jei/blacklist.json` (JEI's hidden ingredients), read only: none when there is no file.
fn read_blacklist() -> Blacklist {
    let Ok(dir) = Game::native_config_dir() else { return Blacklist::default() };
    let Ok(text) = std::fs::read_to_string(dir.join("jei").join("blacklist.json")) else { return Blacklist::default() };
    let Ok(serde_json::Value::Array(entries)) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Blacklist::default();
    };
    let triples: Vec<(&str, &str, &str)> = entries
        .iter()
        .filter_map(|e| {
            let ingredient = e.get("ingredient")?;
            let id = match ingredient.get("ingredient")? {
                serde_json::Value::String(id) => id.as_str(),
                other => other.get("id").or_else(|| other.get("item"))?.as_str()?,
            };
            let mode = e.get("hide_mode").and_then(|m| m.as_str()).unwrap_or("WILDCARD");
            Some((mode, ingredient.get("type")?.as_str()?, id))
        })
        .collect();
    Blacklist::from_entries(triples)
}
/// `config/jei/jei-mod-id-format.ini`'s format as `§` codes: the file's value, or JEI's `blue italic`; a new file
/// is written as JEI writes it when there is none.
fn read_mod_name_format() -> String {
    let path = Game::native_config_dir().ok().map(|d| d.join("jei").join("jei-mod-id-format.ini"));
    let text = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok());
    if text.is_none()
        && let Some(path) = &path
    {
        let _ = path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(path, mod_id_format::fresh()));
    }
    mod_id_format::codes(&text.and_then(|t| mod_id_format::read(&t)).unwrap_or_else(|| mod_id_format::DEFAULT.to_owned()))
}
/// The list's grid from `[ingredientList]` (ATM10: 18 columns, 99 rows, right, top): the engine fits what the screen
/// allows. As JEI's overlay, the search box stays with no items (a search that finds nothing, a config change that
/// leaves nothing, the list hidden until a search); Ctrl+O takes the whole panel away.
fn grid(game: &mut Game<'_>, hidden: bool) {
    let (max_columns, max_rows) = super::jei_config::grid_limits("ingredientList");
    let align = |key, default| NativePanelAlign::parse(&super::jei_config::value("ingredientList", key)).unwrap_or(default);
    let grid = NativePanelGrid {
        horizontal: align("horizontalAlignment", NativePanelAlign::End),
        vertical: align("verticalAlignment", NativePanelAlign::Start),
        keep_search: !hidden,
        ..NativePanelGrid::new(max_columns, max_rows)
    };
    game.native_panel_grid(ID, grid);
}
/// JEI's mod name line, last in an item's tooltip (`native_tooltip`): the format of `jei-mod-id-format.ini`, not
/// twice when the last line already is the name.
pub(super) fn tooltip(game: &Game<'_>, stack: &horizon_client::items::ItemStack, lines: &mut Vec<(horizon_client::text::Text, u32)>) {
    if !game.native_on(ID) {
        return;
    }
    let Ok(format) = MOD_NAME_FORMAT.try_lock().map(|f| f.clone()) else { return };
    if format.is_empty() || lines.is_empty() {
        return;
    }
    let name = jei_core::mod_name(stack.item.split(':').next().unwrap_or("minecraft"));
    if lines.last().is_some_and(|(t, _)| t.plain() == name) {
        return;
    }
    lines.push((horizon_client::text::Text::legacy(&format!("{format}{name}")), u32::MAX));
}

/// The mod name line JEI puts last in an ingredient's tooltip (`jei-mod-id-format.ini`), for namespace `ns`; none
/// when the format is empty.
pub(super) fn mod_name_line(ns: &str) -> Option<horizon_client::text::Text> {
    let format = MOD_NAME_FORMAT.try_lock().ok()?.clone();
    (!format.is_empty()).then(|| horizon_client::text::Text::legacy(&format!("{format}{}", jei_core::mod_name(ns))))
}

/// Whether the panel shows no items: Ctrl+O hid it, or it hides until a search and there is none.
fn empty(s: &State) -> bool {
    s.hidden || (hide_unsearched() && s.query.trim().is_empty())
}
/// The panel shows what the state keeps, or nothing while hidden.
fn show(game: &mut Game<'_>, s: &State) {
    let items = if empty(s) {
        Vec::new()
    } else {
        s.shown
            .iter()
            .map(|&n| match s.list.kind(n) {
                "" => NativeStack::item(s.list.id(n), 1),
                kind => NativeStack::typed(kind, s.list.id(n), 1000),
            })
            .collect()
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
            *MOD_NAME_FORMAT.lock().unwrap_or_else(|e| e.into_inner()) = read_mod_name_format();
            let version = game.native_recipe_categories_version();
            let list = ingredients(game, &t);
            let blacklist = read_blacklist();
            let shown = filter(&list, &blacklist, "", game);
            let hidden = overlay_hidden();
            game.native_panel_style(ID, NativePanelStyle::Viewer);
            grid(game, hidden);
            let s = State { list, query: String::new(), shown, hidden, blacklist, version };
            show(game, &s);
            *guard = Some(s);
        }
        // A mod registered its ingredients (Mekanism's chemicals) or changed its categories: the list again.
        (Some(t), true) if guard.as_ref().is_some_and(|s| s.version != game.native_recipe_categories_version()) => {
            let version = game.native_recipe_categories_version();
            let list = ingredients(game, &t);
            if let Some(s) = guard.as_mut() {
                s.version = version;
                s.list = list;
                s.shown = filter(&s.list, &s.blacklist, &s.query, game);
                show(game, s);
            }
        }
        (None, true) => {
            *guard = None;
            MOD_NAME_FORMAT.lock().unwrap_or_else(|e| e.into_inner()).clear();
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
        s.shown = filter(&s.list, &s.blacklist, &s.query, game);
        grid(game, s.hidden);
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
        super::jei_bookmarks::toggle(&super::jei_recipes::key(stack));
        return;
    }
    let mut guard = state();
    let Some(s) = guard.as_mut() else { return };
    match event {
        NativePanelEvent::Search(text) => {
            s.query = text.clone();
            s.shown = filter(&s.list, &s.blacklist, text, game);
            show(game, s);
        }
        // Cheat mode gives an item; a fluid or a chemical shows its recipes as without it.
        NativePanelEvent::Clicked { index, right } if cheating(game) && s.shown.get(*index as usize).is_some_and(|&n| s.list.kind(n).is_empty()) => {
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
            let id = s.list.key(n);
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
        super::jei_bookmarks::toggle(&super::jei_recipes::key(stack));
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
    match words.next() {
        Some("cheat") => super::jei_config::set_cheat(words.next() == Some("on")),
        // `set <section> <key> <value>`: a value as the config screen sets it.
        Some("set") => {
            if let (Some(section), Some(key), Some(value)) = (words.next(), words.next(), words.next()) {
                super::jei_config::set(section, key, value);
            }
        }
        _ => {}
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
            "displayed": if empty(s) { 0 } else { s.shown.len() },
            "blacklist": {
                "entries": s.blacklist.len(),
                "items": s.blacklist.items(),
                "hides": (0..s.list.len()).filter(|&n| s.blacklist.hides(s.list.kind(n), s.list.id(n))).map(|n| s.list.key(n)).collect::<Vec<_>>(),
            },
            "kinds": {
                "fluid": (0..s.list.len()).filter(|&n| s.list.kind(n) == NativeStack::FLUID).count(),
                "chemical": (0..s.list.len()).filter(|&n| s.list.kind(n) == "mekanism:chemical").count(),
            },
            "shownTyped": s.shown.iter().filter(|&&n| !s.list.kind(n).is_empty()).take(40).map(|&n| s.list.key(n)).collect::<Vec<_>>(),
            "hideUnsearched": hide_unsearched(),
            "modNameFormat": MOD_NAME_FORMAT.try_lock().map(|f| f.clone()).unwrap_or_default(),
            "area": game.native_panel_area(ID),
            "first": s.shown.iter().take(12).map(|&n| s.list.id(n)).collect::<Vec<_>>(),
            "modes": Prefix::ALL.map(|p| modes.get(p).name()),
            "recipes": super::jei_recipes::probe(),
            "bookmarks": super::jei_bookmarks::probe(),
            "config": super::jei_config::probe(),
        }),
    };
    game.native_chat(&format!("JEI {json}"));
}
