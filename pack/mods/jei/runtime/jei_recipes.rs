// JEI's recipe screen in the game (support module `crate::game::jei_recipes` of the `jei` native bundle): the game's
// recipe tables (`native_recipes`) and the mods' own categories (`native_collect_recipe_categories`) into
// `jei_core::recipes`, the open page out to JEI's recipe window (`jei_window`, centred over the screen beside the
// item list), and R, U, Backspace and the window's clicks in.
use super::Game;
use super::jei_world::{ID, KEYS};
use super::jei_window::{self, Action};
use super::native_hooks::{
    NativePanelEvent, NativeRecipe, NativeRecipeCategory, NativeRecipeKind, NativeRecipeLookup, NativeRecipeStack,
    NativeRecipes, NativeStack,
};
use jei_core::recipes::{Book, Category, Focus, Mode, Output, Recipe, Shape, Stack, Viewer, cooking_note};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// JEI's vanilla categories in the order `VanillaPlugin.registerCategories` registers them, with their titles
/// (JEI's en_us) and the catalysts `registerRecipeCatalysts` gives them. Fuel, composting, anvil and grindstone
/// have no recipes in the game's tables yet.
const VANILLA: [(&str, &str, &[&str]); 8] = [
    ("minecraft:crafting", "Crafting", &["minecraft:crafting_table", "minecraft:crafter"]),
    ("minecraft:stonecutting", "Stonecutting", &["minecraft:stonecutter"]),
    ("minecraft:smelting", "Smelting", &["minecraft:furnace"]),
    ("minecraft:smoking", "Smoking", &["minecraft:smoker"]),
    ("minecraft:blasting", "Blasting", &["minecraft:blast_furnace"]),
    ("minecraft:campfire_cooking", "Campfire Cooking", &["minecraft:campfire", "minecraft:soul_campfire"]),
    ("minecraft:smithing", "Smithing", &["minecraft:smithing_table"]),
    ("minecraft:brewing", "Brewing", &["minecraft:brewing_stand"]),
];

fn vanilla_index(kind: NativeRecipeKind) -> usize {
    match kind {
        NativeRecipeKind::CraftingShaped | NativeRecipeKind::CraftingShapeless => 0,
        NativeRecipeKind::Stonecutting => 1,
        NativeRecipeKind::Smelting => 2,
        NativeRecipeKind::Smoking => 3,
        NativeRecipeKind::Blasting => 4,
        NativeRecipeKind::Campfire => 5,
        NativeRecipeKind::Smithing => 6,
        NativeRecipeKind::Brewing => 7,
    }
}

/// The viewer and what its book was made from.
struct State {
    viewer: Viewer,
    tables: Arc<NativeRecipes>,
    version: u64,
    /// Each vanilla category's recipes as indices into `tables.recipes`, for their `book_id`s.
    native: Vec<Vec<usize>>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
/// Backspace went down on a screen with the recipes open (that hook can't change the panel); the next frame goes back.
static BACK: AtomicBool = AtomicBool::new(false);

fn state() -> std::sync::MutexGuard<'static, Option<State>> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

fn stack(s: NativeStack) -> Stack {
    Stack { item: s.item, count: s.count.max(1), components: s.components }
}
fn slot(t: &NativeRecipes, list: &[NativeRecipeStack]) -> Vec<Stack> {
    list.iter()
        .filter_map(|s| {
            let item = t.items.get(s.item as usize)?;
            Some(Stack { item: item.id.clone(), count: s.count.max(1), components: s.components.clone() })
        })
        .collect()
}

fn vanilla_recipe(t: &NativeRecipes, r: &NativeRecipe) -> Recipe {
    let cells: Vec<Vec<Stack>> = r.slots.iter().map(|s| slot(t, s)).collect();
    let results = slot(t, &r.results);
    let shape = match r.kind {
        NativeRecipeKind::CraftingShaped => return Recipe::shaped(r.width, r.height, cells, results),
        NativeRecipeKind::CraftingShapeless => return Recipe::shapeless(cells, results),
        NativeRecipeKind::Smelting | NativeRecipeKind::Blasting | NativeRecipeKind::Smoking | NativeRecipeKind::Campfire => {
            Shape::Cooking
        }
        NativeRecipeKind::Stonecutting => Shape::Single,
        NativeRecipeKind::Smithing => Shape::Smithing,
        NativeRecipeKind::Brewing => Shape::Brewing,
    };
    let notes = if shape == Shape::Cooking { vec![cooking_note(r.experience, r.cooking_time)] } else { Vec::new() };
    Recipe { shape, width: 1, height: 1, inputs: cells, outputs: vec![results.into_iter().map(Output::always).collect()], notes }
}

fn mod_category(c: NativeRecipeCategory) -> Category {
    let catalysts: Vec<Stack> = c.catalysts.into_iter().map(stack).collect();
    let icon = if c.icon.item.is_empty() { catalysts.first().cloned().unwrap_or_default() } else { stack(c.icon) };
    let recipes = c
        .recipes
        .into_iter()
        .map(|r| {
            let inputs = r.inputs.into_iter().map(|s| s.into_iter().map(stack).collect()).collect();
            let outputs = r
                .outputs
                .into_iter()
                .map(|o| vec![Output { stack: stack(o.stack), chance: o.chance, min: o.min, max: o.max, notes: o.notes }])
                .collect();
            Recipe::machine(inputs, outputs, r.notes)
        })
        .collect();
    Category { id: c.id, title: c.title, icon, catalysts, recipes }
}

/// Vanilla's categories from the tables, then the mods' in the order they came.
fn book(t: &NativeRecipes, mods: Vec<NativeRecipeCategory>) -> (Book, Vec<Vec<usize>>) {
    let mut categories: Vec<Category> = VANILLA
        .iter()
        .map(|(id, title, catalysts)| Category {
            id: (*id).into(),
            title: (*title).into(),
            icon: Stack::of(catalysts[0]),
            catalysts: catalysts.iter().map(|c| Stack::of(c)).collect(),
            recipes: Vec::new(),
        })
        .collect();
    let mut native = vec![Vec::new(); VANILLA.len()];
    for (n, r) in t.recipes.iter().enumerate() {
        categories[vanilla_index(r.kind)].recipes.push(vanilla_recipe(t, r));
        native[vanilla_index(r.kind)].push(n);
    }
    categories.extend(mods.into_iter().filter(|c| !c.recipes.is_empty()).map(mod_category));
    let names = t.items.iter().map(|i| (i.id.clone(), i.name.clone())).collect();
    (Book { categories, names }, native)
}

/// The viewer, its book made again when the game's tables or the mods' categories changed.
fn ready<'a>(game: &mut Game<'_>, guard: &'a mut Option<State>) -> Option<&'a mut State> {
    let tables = game.native_recipes()?;
    let version = game.native_recipe_categories_version();
    let fresh = guard.as_ref().is_some_and(|s| Arc::ptr_eq(&s.tables, &tables) && s.version == version);
    if !fresh {
        let (book, native) = book(&tables, game.native_collect_recipe_categories());
        jei_window::set_names(Arc::new(book.names.clone()));
        match guard.as_mut() {
            Some(s) => {
                s.viewer.set_book(book);
                s.tables = tables;
                s.version = version;
                s.native = native;
            }
            None => *guard = Some(State { viewer: Viewer::new(book), tables, version, native }),
        }
    }
    guard.as_mut()
}

/// The open page to the window, or the window closed.
fn show(s: &State) {
    jei_window::set(s.viewer.page(), sources(s));
}
/// Each recipe of the page: its category's id and, for a vanilla one, its `book_id` (for its transfer).
fn sources(s: &State) -> Vec<(String, i32)> {
    let Some(cur) = s.viewer.current() else { return Vec::new() };
    let Some(found) = cur.found.get(cur.tab) else { return Vec::new() };
    let id = &s.viewer.book.categories[found.category].id;
    let native = s.native.get(found.category);
    let book_id = |r: usize| native.and_then(|n| n.get(r)).and_then(|&n| s.tables.recipes.get(n)).map_or(-1, |n| n.book_id);
    found.recipes.iter().take(jei_core::recipes::RECIPES).map(|&r| (id.clone(), book_id(r))).collect()
}

/// R or U on `item`: its recipes or uses, the open page going onto the history. Nothing happens when it has none.
pub(super) fn open(game: &mut Game<'_>, item: &str, mode: Mode) -> bool {
    let mut guard = state();
    let Some(s) = ready(game, &mut guard) else { return false };
    if !s.viewer.open(Focus { item: item.to_owned(), mode }) {
        return false;
    }
    show(s);
    true
}

/// What a key does over `stack`: R its recipes, U its uses (as the player bound them).
pub(super) fn key_mode(game: &Game<'_>, key: &str) -> Option<Mode> {
    let bound = |k: usize| game.native_mod_key(ID, KEYS[k][0], KEYS[k][2]);
    if key == bound(1) {
        Some(Mode::Recipes)
    } else if key == bound(2) {
        Some(Mode::Uses)
    } else {
        None
    }
}

/// The panel's events that belong to the recipe screen; true when it took the event.
pub(super) fn event(game: &mut Game<'_>, event: &NativePanelEvent) -> bool {
    let (item, mode) = match event {
        // A key over the window is the window's (`jei_window::key`), not the slot of the screen under it.
        NativePanelEvent::Key { .. } if jei_window::covers(game.native_cursor()) => return true,
        NativePanelEvent::Key { key, stack } => match key_mode(game, key) {
            Some(mode) => (stack.item.clone(), mode),
            None => return false,
        },
        // JEI: a left click on an ingredient in a recipe shows its recipes, a right click its uses.
        NativePanelEvent::SlotClicked { stack, right, .. } | NativePanelEvent::Catalyst { stack, right, .. } => {
            (stack.item.clone(), if *right { Mode::Uses } else { Mode::Recipes })
        }
        NativePanelEvent::Tab(tab) => {
            if let Some(s) = state().as_mut() {
                s.viewer.tab(*tab as usize);
                show(s);
            }
            return true;
        }
        NativePanelEvent::Back => {
            back(game);
            return true;
        }
        _ => return false,
    };
    open(game, &item, mode);
    true
}

/// The `recipe_lookup` hook: another mod's screen asks for a stack's recipes, uses, or what a key over it does.
pub(super) fn lookup(game: &mut Game<'_>, stack: &NativeStack, lookup: &NativeRecipeLookup, handled: &mut bool) {
    if *handled || !game.native_on(ID) {
        return;
    }
    let mode = match lookup {
        NativeRecipeLookup::Recipes => Mode::Recipes,
        NativeRecipeLookup::Uses => Mode::Uses,
        NativeRecipeLookup::Key(key) => match key_mode(game, key) {
            Some(mode) => mode,
            None => return,
        },
        _ => return,
    };
    open(game, &stack.item, mode);
    *handled = true;
}

/// Backspace (JEI's `recipeBack`): the page before, or the list when there was none.
fn back(game: &mut Game<'_>) {
    if let Some(s) = state().as_mut() {
        s.viewer.back();
        show(s);
    }
}

/// Whether the recipe screen is open (for the key hook, which then asks for `back`).
pub(super) fn is_open() -> bool {
    state().as_ref().is_some_and(|s| s.viewer.current().is_some())
}
pub(super) fn ask_back() {
    BACK.store(true, Ordering::Relaxed);
}

/// Closes the window and drops the history (JEI's `onClose`).
fn close() {
    if let Some(s) = state().as_mut() {
        s.viewer.close();
        show(s);
    }
}

/// Each frame: a Backspace and what the window's clicks, wheel and keys asked for; the window goes with the screen
/// under it, and outside a world the window and its book go.
pub(super) fn frame(game: &mut Game<'_>, in_world: bool) {
    if !in_world {
        *state() = None;
        jei_window::set(None, Vec::new());
        jei_window::take_actions();
        BACK.store(false, Ordering::Relaxed);
        return;
    }
    if BACK.swap(false, Ordering::Relaxed) {
        back(game);
    }
    for action in jei_window::take_actions() {
        match action {
            Action::Open(item, mode) => {
                open(game, &item, mode);
            }
            Action::Tab(tab) => select(|_, _| tab),
            Action::Category(by) => select(|tab, n| (tab as i32 + by).rem_euclid(n as i32) as usize),
            Action::Page(by) => jei_window::turn(by),
            Action::Close => close(),
            Action::Transfer(i) => transfer(game, i),
        }
    }
    if jei_window::stale() {
        close();
    }
    // JEI asks the screen's transfer handler about each recipe shown (`updateRecipeTransferError`) for its "+".
    let answers = jei_window::transfers()
        .into_iter()
        .map(|(i, t)| (i, game.native_recipe_transfer(&t, false, false, false)))
        .collect();
    jei_window::set_answers(answers);
}

/// Another tab: `to(tab, tabs)` picks it.
fn select(to: impl Fn(usize, usize) -> usize) {
    if let Some(s) = state().as_mut()
        && let Some(cur) = s.viewer.current()
    {
        let n = cur.found.len().min(jei_core::recipes::TABS);
        s.viewer.tab(to(cur.tab, n.max(1)));
        show(s);
    }
}

/// The probe's view of the screen (`/jeiclient`).
pub(super) fn probe() -> serde_json::Value {
    let guard = state();
    let Some(s) = guard.as_ref() else { return serde_json::Value::Null };
    let Some(cur) = s.viewer.current() else { return serde_json::Value::Null };
    let book = &s.viewer.book;
    let found = &cur.found[cur.tab];
    let cat = &book.categories[found.category];
    serde_json::json!({
        "focus": cur.focus.item,
        "mode": if cur.focus.mode == Mode::Recipes { "recipes" } else { "uses" },
        "category": cat.id,
        "title": cat.title,
        "tabs": cur.found.iter().map(|f| book.categories[f.category].id.as_str()).collect::<Vec<_>>(),
        "catalysts": cat.catalysts.iter().map(|c| c.item.as_str()).collect::<Vec<_>>(),
        "recipes": found.recipes.len(),
        "history": s.viewer.history(),
        "categories": book.categories.len(),
        "window": jei_window::probe(),
    })
}

/// JEI's "+" on recipe `i` (`RecipeTransferButtonController.transferRecipe`): Shift moves as many as the items allow,
/// Ctrl asks AE2's crafting terminal to craft what is missing; a transfer made closes the window, back to the screen
/// under it (`onSuccessfulTransfer`).
fn transfer(game: &mut Game<'_>, i: usize) {
    let Some(t) = jei_window::transfer(i) else { return };
    let (max, ctrl) = (game.native_has_shift_down(), game.native_has_control_down());
    if game.native_recipe_transfer(&t, true, max, ctrl).done {
        close();
    }
}
