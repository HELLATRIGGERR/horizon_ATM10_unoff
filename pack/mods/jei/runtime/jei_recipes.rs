// JEI's recipe screen in the game (support module `crate::game::jei_recipes` of the `jei` native bundle): the game's
// recipe tables (`native_recipes`) and the mods' own categories (`native_collect_recipe_categories`) into
// `jei_core::recipes`, the open page out to the panel (`native_panel_recipes`, `native_panel_catalysts`), and R, U,
// Backspace and clicks in. JEI draws its recipe screen as a window centred over the container; this port shows the
// same tabs, catalysts, recipes and pages in the engine's panel at the right edge, where the item list is.
use super::Game;
use super::jei_world::{ID, KEYS};
use super::native_hooks::{
    NativePanelEvent, NativeRecipe, NativeRecipeCategory, NativeRecipeKind, NativeRecipeLayout, NativeRecipeLookup,
    NativeRecipeStack, NativeRecipeView, NativeRecipes, NativeStack,
};
use jei_core::recipes::{Book, Category, Focus, Mode, Output, Recipe, Shape, Stack, Viewer, View, cooking_note};
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
fn native(s: Stack) -> NativeStack {
    NativeStack { item: s.item, count: s.count, components: s.components }
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
fn book(t: &NativeRecipes, mods: Vec<NativeRecipeCategory>) -> Book {
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
    for r in &t.recipes {
        categories[vanilla_index(r.kind)].recipes.push(vanilla_recipe(t, r));
    }
    categories.extend(mods.into_iter().filter(|c| !c.recipes.is_empty()).map(mod_category));
    let names = t.items.iter().map(|i| (i.id.clone(), i.name.clone())).collect();
    Book { categories, names }
}

/// The viewer, its book made again when the game's tables or the mods' categories changed.
fn ready<'a>(game: &mut Game<'_>, guard: &'a mut Option<State>) -> Option<&'a mut State> {
    let tables = game.native_recipes()?;
    let version = game.native_recipe_categories_version();
    let fresh = guard.as_ref().is_some_and(|s| Arc::ptr_eq(&s.tables, &tables) && s.version == version);
    if !fresh {
        let book = book(&tables, game.native_collect_recipe_categories());
        match guard.as_mut() {
            Some(s) => {
                s.viewer.set_book(book);
                s.tables = tables;
                s.version = version;
            }
            None => *guard = Some(State { viewer: Viewer::new(book), tables, version }),
        }
    }
    guard.as_mut()
}

fn layout(shape: Shape) -> NativeRecipeLayout {
    match shape {
        Shape::Grid => NativeRecipeLayout::Grid,
        Shape::Cooking => NativeRecipeLayout::Cooking,
        Shape::Single => NativeRecipeLayout::Single,
        Shape::Smithing => NativeRecipeLayout::Smithing,
        Shape::Brewing => NativeRecipeLayout::Brewing,
    }
}
fn recipe_view(v: View) -> NativeRecipeView {
    NativeRecipeView {
        layout: layout(v.shape),
        width: v.width,
        height: v.height,
        slots: v.slots.into_iter().map(|s| s.into_iter().map(native).collect()).collect(),
        results: v.results.into_iter().map(native).collect(),
        note: v.note,
        // "+" (recipe transfer) is slice 4.
        action: None,
    }
}

/// The open page to the panel, or the item list again when the screen closed.
fn show(game: &mut Game<'_>, viewer: &Viewer) {
    match viewer.page() {
        Some(p) => {
            let tabs = p.tabs.into_iter().map(native).collect();
            let views = p.views.into_iter().map(recipe_view).collect();
            game.native_panel_recipes(ID, &p.title, tabs, p.tab as u32, views);
            game.native_panel_catalysts(ID, p.catalysts.into_iter().map(native).collect());
        }
        None => game.native_panel_recipes(ID, "", Vec::new(), 0, Vec::new()),
    }
}

/// R or U on `item`: its recipes or uses, the open page going onto the history. Nothing happens when it has none.
pub(super) fn open(game: &mut Game<'_>, item: &str, mode: Mode) -> bool {
    let mut guard = state();
    let Some(s) = ready(game, &mut guard) else { return false };
    if !s.viewer.open(Focus { item: item.to_owned(), mode }) {
        return false;
    }
    show(game, &s.viewer);
    true
}

/// What a key does over `stack`: R its recipes, U its uses (as the player bound them).
fn key_mode(game: &Game<'_>, key: &str) -> Option<Mode> {
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
                show(game, &s.viewer);
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
        show(game, &s.viewer);
    }
}

/// Whether the recipe screen is open (for the key hook, which then asks for `back`).
pub(super) fn is_open() -> bool {
    state().as_ref().is_some_and(|s| s.viewer.current().is_some())
}
pub(super) fn ask_back() {
    BACK.store(true, Ordering::Relaxed);
}

/// Each frame: a Backspace asked for; outside a world the screen and its book go.
pub(super) fn frame(game: &mut Game<'_>, in_world: bool) {
    if !in_world {
        *state() = None;
        BACK.store(false, Ordering::Relaxed);
        return;
    }
    if BACK.swap(false, Ordering::Relaxed) {
        back(game);
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
    })
}
