//! JEI's recipe screen (`RecipesGui`, `RecipeGuiLogic`, `RecipeGuiHistory`, `IFocus`): R shows what makes an
//! ingredient, U what it is used in, one tab per category (in the order the categories were registered, the
//! categories it is a catalyst of first for U), each with its catalysts; Backspace goes back through the history.
//! Reimplemented from what the mod does (JEI is MIT, mezz); the runtime turns the game's tables and the mods'
//! categories into [`Category`]s and draws [`Viewer::page`].

use std::collections::HashMap;

/// A stack as the game names it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stack {
    pub item: String,
    pub count: u32,
    pub components: String,
}

impl Stack {
    pub fn of(item: &str) -> Stack {
        Stack { item: item.to_owned(), count: 1, components: String::new() }
    }
}

/// How a recipe is laid out, as the panel draws it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shape {
    /// `width` x `height` input cells.
    #[default]
    Grid,
    Cooking,
    Single,
    Smithing,
    Brewing,
}

/// An output, with the chance it comes (1 always) and how many when it does.
#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    pub stack: Stack,
    pub chance: f32,
    pub min: u32,
    pub max: u32,
    pub notes: Vec<String>,
}

impl Output {
    pub fn always(stack: Stack) -> Output {
        let n = stack.count.max(1);
        Output { stack, chance: 1.0, min: n, max: n, notes: Vec::new() }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recipe {
    pub shape: Shape,
    pub width: u32,
    pub height: u32,
    /// Each input slot's stacks, any of which fits, as drawn (a grid's cells row by row); empty for an empty cell.
    pub inputs: Vec<Vec<Stack>>,
    /// The first is the result; the others draw after it. Several stacks in one output cycle (`results`).
    pub outputs: Vec<Vec<Output>>,
    pub notes: Vec<String>,
}

/// Input slots a recipe draws at most (the panel's limit, outputs past the first included).
pub const SLOTS: usize = 9;

impl Recipe {
    /// A shaped crafting recipe of `width` x `height` cells, placed in JEI's 3 x 3 grid as
    /// `CraftingGridHelper.getCraftingIndex` places it.
    pub fn shaped(width: u32, height: u32, cells: Vec<Vec<Stack>>, results: Vec<Stack>) -> Recipe {
        let mut grid = vec![Vec::new(); 9];
        for (i, cell) in cells.into_iter().enumerate() {
            if let Some(slot) = grid.get_mut(crafting_index(i as u32, width, height) as usize) {
                *slot = cell;
            }
        }
        Recipe { shape: Shape::Grid, width: 3, height: 3, inputs: grid, outputs: vec![results.into_iter().map(Output::always).collect()], notes: Vec::new() }
    }
    /// A shapeless one: JEI's square of 1, 2 or 3 (`getShapelessSize`), placed the same way.
    pub fn shapeless(cells: Vec<Vec<Stack>>, results: Vec<Stack>) -> Recipe {
        let n = cells.len();
        let size = if n > 4 { 3 } else if n > 1 { 2 } else { 1 };
        Recipe::shaped(size, size, cells, results)
    }
    /// A mod's machine: its inputs in a row (three to a row past three), its outputs after the arrow.
    pub fn machine(mut inputs: Vec<Vec<Stack>>, mut outputs: Vec<Vec<Output>>, notes: Vec<String>) -> Recipe {
        inputs.truncate(SLOTS);
        outputs.truncate(1 + SLOTS - inputs.len());
        let n = inputs.len() as u32;
        let (shape, width, height) = match n {
            0 | 1 => (Shape::Single, 1, 1),
            2 | 3 => (Shape::Grid, n, 1),
            _ => (Shape::Grid, 3, n.div_ceil(3)),
        };
        // A grid's cells past the inputs stay empty, so the outputs after the result keep their places.
        let cells = if shape == Shape::Grid { (width * height) as usize } else { 1 };
        if inputs.len() < cells && outputs.len() > 1 {
            inputs.resize(cells, Vec::new());
        }
        if outputs.len() > 1 + SLOTS - inputs.len() {
            outputs.truncate(1 + SLOTS - inputs.len());
        }
        Recipe { shape, width, height, inputs, outputs, notes }
    }
    fn makes(&self, item: &str) -> bool {
        self.outputs.iter().flatten().any(|o| o.stack.item == item)
    }
    fn takes(&self, item: &str) -> bool {
        self.inputs.iter().flatten().any(|s| s.item == item)
    }
}

/// `CraftingGridHelper.getCraftingIndex`: where cell `i` of a `width` x `height` recipe goes in the 3 x 3 grid.
pub fn crafting_index(i: u32, width: u32, height: u32) -> u32 {
    if width == 1 {
        if height == 3 || height == 2 { i * 3 + 1 } else { 4 }
    } else if height == 1 {
        i + 3
    } else if width == 2 {
        let mut index = i;
        if i > 1 {
            index += 1;
            if i > 3 {
                index += 1;
            }
        }
        index
    } else if height == 2 {
        i + 3
    } else {
        i
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Category {
    pub id: String,
    pub title: String,
    pub icon: Stack,
    pub catalysts: Vec<Stack>,
    pub recipes: Vec<Recipe>,
}

/// R (`RecipeIngredientRole.OUTPUT`) or U (`INPUT`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Recipes,
    Uses,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Focus {
    pub item: String,
    pub mode: Mode,
}

/// A category the focus shows: its place in the book and its recipes' places in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub category: usize,
    pub recipes: Vec<usize>,
}

/// One state of the recipe screen, as the history keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Screen {
    pub focus: Focus,
    pub found: Vec<Found>,
    pub tab: usize,
}

/// Every category, with the items' names for the notes.
#[derive(Clone, Debug, Default)]
pub struct Book {
    pub categories: Vec<Category>,
    pub names: HashMap<String, String>,
}

impl Book {
    /// The categories `focus` shows, in the book's order; for U, the ones it is a catalyst of come first with
    /// all their recipes (JEI's catalyst lookup).
    pub fn find(&self, focus: &Focus) -> Vec<Found> {
        let item = focus.item.as_str();
        let mut catalyst = Vec::new();
        let mut rest = Vec::new();
        for (c, cat) in self.categories.iter().enumerate() {
            if focus.mode == Mode::Uses && cat.catalysts.iter().any(|s| s.item == item) {
                catalyst.push(Found { category: c, recipes: (0..cat.recipes.len()).collect() });
                continue;
            }
            let recipes: Vec<usize> = cat
                .recipes
                .iter()
                .enumerate()
                .filter(|(_, r)| match focus.mode {
                    Mode::Recipes => r.makes(item),
                    Mode::Uses => r.takes(item),
                })
                .map(|(i, _)| i)
                .collect();
            if !recipes.is_empty() {
                rest.push(Found { category: c, recipes });
            }
        }
        catalyst.retain(|f| !f.recipes.is_empty());
        catalyst.extend(rest);
        catalyst
    }
    fn name<'a>(&'a self, item: &'a str) -> &'a str {
        self.names.get(item).map_or(item, String::as_str)
    }
}

/// A recipe as the panel draws it: the input slots, the outputs past the first after them, the result, the notes.
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub shape: Shape,
    pub width: u32,
    pub height: u32,
    pub slots: Vec<Vec<Stack>>,
    pub results: Vec<Stack>,
    pub note: String,
}

/// What the panel shows: the category's title, the tabs' icons, the selected tab, its catalysts and recipes.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub title: String,
    pub tabs: Vec<Stack>,
    pub tab: usize,
    pub catalysts: Vec<Stack>,
    pub views: Vec<View>,
}

/// Tabs and recipes a page holds at most (the panel's limits).
pub const TABS: usize = 16;
pub const RECIPES: usize = 256;
/// A note's line at most, in characters, so it stays inside the panel.
const LINE: usize = 26;

/// The recipe screen: the open state and the history behind it.
#[derive(Default)]
pub struct Viewer {
    pub book: Book,
    current: Option<Screen>,
    history: Vec<Screen>,
}

impl Viewer {
    pub fn new(book: Book) -> Viewer {
        Viewer { book, current: None, history: Vec::new() }
    }
    pub fn current(&self) -> Option<&Screen> {
        self.current.as_ref()
    }
    pub fn history(&self) -> usize {
        self.history.len()
    }
    /// Shows `focus`; the open state goes onto the history. False, and nothing changes, when it has no recipes
    /// (JEI opens nothing then). The same focus again keeps the tab.
    pub fn open(&mut self, focus: Focus) -> bool {
        if self.current.as_ref().is_some_and(|s| s.focus == focus) {
            return true;
        }
        let found = self.book.find(&focus);
        if found.is_empty() {
            return false;
        }
        if let Some(old) = self.current.take() {
            self.history.push(old);
        }
        self.current = Some(Screen { focus, found, tab: 0 });
        true
    }
    /// Backspace: the state before; false when there was none and the screen closed.
    pub fn back(&mut self) -> bool {
        self.current = self.history.pop();
        self.current.is_some()
    }
    pub fn close(&mut self) {
        self.current = None;
        self.history.clear();
    }
    pub fn tab(&mut self, tab: usize) {
        if let Some(s) = self.current.as_mut()
            && tab < s.found.len().min(TABS)
        {
            s.tab = tab;
        }
    }
    /// The categories again (the mods' changed): the history is dropped, the open focus looked up again.
    pub fn set_book(&mut self, book: Book) {
        self.book = book;
        self.history.clear();
        if let Some(old) = self.current.take() {
            let found = self.book.find(&old.focus);
            if !found.is_empty() {
                let tab = old.tab.min(found.len() - 1);
                self.current = Some(Screen { focus: old.focus, found, tab });
            }
        }
    }
    /// The open state as the panel draws it.
    pub fn page(&self) -> Option<Page> {
        let s = self.current.as_ref()?;
        let found = s.found.get(s.tab)?;
        let cat = &self.book.categories[found.category];
        let tabs = s.found.iter().take(TABS).map(|f| self.book.categories[f.category].icon.clone()).collect();
        let views = found.recipes.iter().take(RECIPES).map(|&r| self.view(&cat.recipes[r], &s.focus)).collect();
        Some(Page { title: cat.title.clone(), tabs, tab: s.tab, catalysts: cat.catalysts.clone(), views })
    }
    /// The focused ingredient alone in the slots that take it (JEI locks a focused slot's cycle), the chances and
    /// counts of the outputs as lines under the recipe.
    fn view(&self, r: &Recipe, focus: &Focus) -> View {
        let lock = |stacks: Vec<Stack>| {
            let hit: Vec<Stack> = stacks.iter().filter(|s| s.item == focus.item).cloned().collect();
            if hit.is_empty() { stacks } else { hit }
        };
        let inputs = r.inputs.iter().cloned().map(|s| if focus.mode == Mode::Uses { lock(s) } else { s });
        let outputs: Vec<Vec<Stack>> = r
            .outputs
            .iter()
            .map(|o| {
                let stacks = o.iter().map(|o| o.stack.clone()).collect();
                if focus.mode == Mode::Recipes { lock(stacks) } else { stacks }
            })
            .collect();
        let mut slots: Vec<Vec<Stack>> = inputs.collect();
        slots.extend(outputs.iter().skip(1).cloned());
        let mut lines: Vec<String> = Vec::new();
        for o in r.outputs.iter().filter_map(|o| o.first()) {
            let name = self.book.name(&o.stack.item);
            if o.chance < 1.0 {
                lines.push(format!("{name}: {}%", percent(o.chance)));
            }
            if o.min != o.max {
                lines.push(format!("{name}: {}-{}", o.min, o.max));
            }
            lines.extend(o.notes.iter().cloned());
        }
        lines.extend(r.notes.iter().cloned());
        let note = lines.iter().flat_map(|l| wrap(l, LINE)).collect::<Vec<_>>().join("\n");
        View {
            shape: r.shape,
            width: r.width,
            height: r.height,
            slots,
            results: outputs.into_iter().next().unwrap_or_default(),
            note,
        }
    }
}

/// A chance as JEI words it: whole percent, or one decimal under 1%.
pub fn percent(chance: f32) -> String {
    let p = chance * 100.0;
    if p >= 1.0 || p == 0.0 { format!("{}", p.round() as u32) } else { format!("{:.1}", p) }
}

/// A line cut at spaces into lines of at most `max` characters.
pub fn wrap(line: &str, max: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for word in line.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > max {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// A furnace recipe's note as JEI draws it (`gui.jei.category.smelting.experience`, `.time.seconds`).
pub fn cooking_note(experience: f32, ticks: u32) -> String {
    let xp = format!("{:.2}", experience);
    let xp = xp.trim_end_matches('0').trim_end_matches('.');
    let secs = ticks as f32 / 20.0;
    let secs = if secs.fract() == 0.0 { format!("{}", secs as u32) } else { format!("{secs:.1}") };
    if experience > 0.0 { format!("{xp} XP, {secs}s") } else { format!("{secs}s") }
}
