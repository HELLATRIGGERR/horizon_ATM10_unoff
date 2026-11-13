//! JEI's recipe window (`RecipesGui`, `RecipeGuiTabs`, `RecipeCatalysts`, `RecipeGuiLayouts`): where its parts go on
//! a screen of a given size, in GUI pixels. The sizes are JEI's (`RecipesGui.borderPadding`, `minGuiWidth`,
//! `RecipeGuiTab.TAB_WIDTH`, `RecipeCatalysts.overlapSize`, `RecipeGuiSizing`); the runtime draws and hit-tests
//! what this places.

use crate::recipes::Shape;

/// `[x, y, width, height]`.
pub type Rect = [f32; 4];

pub const SLOT: f32 = 18.0;
/// `RecipesGui.smallButtonWidth`, `smallButtonHeight`: the category and page arrows.
pub const BUTTON: f32 = 13.0;
/// `RecipeGuiTab.TAB_WIDTH`, `TAB_HEIGHT`.
pub const TAB: f32 = 24.0;
/// A note's line under a recipe.
pub const LINE: f32 = 10.0;
/// The arrow between a recipe's inputs and its result (vanilla's furnace progress arrow).
pub const ARROW: [f32; 2] = [24.0, 16.0];
/// `borderPadding`, `navBarPadding`, `minGuiWidth`.
const BORDER: f32 = 6.0;
const NAV_PAD: f32 = 2.0;
const MIN_WIDTH: f32 = 198.0;
/// `RecipeGuiSizing.calculateInitialSize`: the height left after 58 px (the tabs and margins), between 175 and
/// `maxRecipeGuiHeight`'s default 350.
const HEIGHT_MARGIN: f32 = 58.0;
const MIN_HEIGHT: f32 = 175.0;
const MAX_HEIGHT: f32 = 350.0;
/// `TAB_GUI_OVERLAP`, `TAB_HORIZONTAL_INSET`.
const TAB_OVERLAP: f32 = 3.0;
const TAB_INSET: f32 = 2.0;
/// `RecipeCatalysts.borderSize`, `overlapSize`.
const CATALYST_BORDER: f32 = 5.0;
const CATALYST_OVERLAP: f32 = 6.0;
/// The space kept between recipes on a page, at least.
pub const GAP: f32 = 6.0;

/// What the window is made for.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sizes {
    /// The screen in GUI pixels.
    pub screen: [f32; 2],
    /// The item list's left edge, which the window keeps clear of.
    pub panel_left: Option<f32>,
    pub tabs: usize,
    pub tab: usize,
    pub catalysts: usize,
    /// The widest recipe.
    pub widest: f32,
}

/// The window's parts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Window {
    /// The background.
    pub area: Rect,
    /// The tabs shown (the page of them that holds the selected one), from tab `first_tab`.
    pub tabs: Vec<Rect>,
    pub first_tab: usize,
    /// The catalysts' tab left of the window and each catalyst's slot.
    pub catalyst_area: Option<Rect>,
    pub catalysts: Vec<Rect>,
    /// The category title's row between its arrows, and the page row ("1/4") under it.
    pub title: Rect,
    pub category_arrows: [Rect; 2],
    pub page: Rect,
    pub page_arrows: [Rect; 2],
    /// Where the recipes go.
    pub recipes: Rect,
}

/// The window on a screen of `s.screen`: centred (`RecipesGui.updateSize`), left of the item list, the tabs on top,
/// the catalysts on the left in as many columns as their count needs.
pub fn window(s: &Sizes) -> Window {
    let [w, h] = s.screen;
    let height = (h - HEIGHT_MARGIN).min(MAX_HEIGHT).max(MIN_HEIGHT).floor();
    let rows = (((height - 2.0 * CATALYST_BORDER) / SLOT).floor() as usize).max(1);
    let columns = s.catalysts.div_ceil(rows);
    let catalyst_width = if columns == 0 { 0.0 } else { columns as f32 * SLOT + 2.0 * CATALYST_BORDER };
    let extra = (catalyst_width - CATALYST_OVERLAP).max(0.0);
    let width = MIN_WIDTH.max((s.widest + 2.0 * BORDER).ceil());
    let mut left = ((w - width) / 2.0).floor();
    if let Some(panel) = s.panel_left
        && left + width > panel - 2.0
    {
        left = panel - 2.0 - width;
    }
    let left = left.max(extra + 2.0);
    let top = ((h - height + TAB - TAB_OVERLAP) / 2.0).floor().max(TAB - TAB_OVERLAP);
    let per_page = (((width - 2.0 * TAB_INSET) / TAB).floor() as usize).max(1);
    let first_tab = s.tab / per_page * per_page;
    let shown = s.tabs.saturating_sub(first_tab).min(per_page);
    let tabs = (0..shown).map(|i| [left + TAB_INSET + i as f32 * TAB, top - TAB + TAB_OVERLAP, TAB, TAB]).collect();
    let catalyst_area = (columns > 0).then(|| {
        let used = s.catalysts.min(rows) as f32;
        [left - extra, top, catalyst_width, used * SLOT + 2.0 * CATALYST_BORDER]
    });
    let catalysts = (0..s.catalysts)
        .map(|k| {
            let (column, row) = (k / rows, k % rows);
            // The first column is the one beside the window.
            let x = left - extra + CATALYST_BORDER + (columns - 1 - column) as f32 * SLOT;
            [x, top + CATALYST_BORDER + row as f32 * SLOT, SLOT, SLOT]
        })
        .collect();
    let row = |y: f32| {
        let arrows = [[left + BORDER, y, BUTTON, BUTTON], [left + width - BORDER - BUTTON, y, BUTTON, BUTTON]];
        let between = [left + BORDER + BUTTON, y, width - 2.0 * (BORDER + BUTTON), BUTTON];
        (between, arrows)
    };
    let (title, category_arrows) = row(top + BORDER);
    let (page, page_arrows) = row(top + BORDER + BUTTON + NAV_PAD);
    let recipes_top = page[1] + BUTTON + NAV_PAD + 2.0;
    Window {
        area: [left, top, width, height],
        tabs,
        first_tab,
        catalyst_area,
        catalysts,
        title,
        category_arrows,
        page,
        page_arrows,
        recipes: [left + BORDER, recipes_top, width - 2.0 * BORDER, top + height - BORDER - recipes_top],
    }
}

/// A recipe's slots, arrow and flame from its top left corner, and the size they take.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Places {
    pub size: [f32; 2],
    /// Every slot the recipe sends, in its order: the inputs, the outputs past the first, then the result.
    pub slots: Vec<[f32; 2]>,
    pub arrow: [f32; 2],
    /// The furnace's flame under its input.
    pub flame: Option<[f32; 2]>,
}

/// Input slots a shape lays out; the slots a recipe sends past them are more outputs.
pub fn inputs(shape: Shape, width: u32, height: u32) -> usize {
    match shape {
        Shape::Grid => (width.max(1) * height.max(1)) as usize,
        Shape::Cooking | Shape::Single => 1,
        Shape::Smithing => 3,
        Shape::Brewing => 2,
    }
}

/// Extra outputs to a row after the result.
const EXTRA_ROW: usize = 4;

/// Where a recipe of `shape` with `slots` slots before its result puts them (JEI's vanilla categories: the
/// crafting grid, the furnace's input over its flame with the arrow and the result beside it, 82 px wide).
pub fn places(shape: Shape, width: u32, height: u32, slots: usize) -> Places {
    let n = inputs(shape, width, height).min(slots);
    let (w, h) = (width.max(1) as usize, height.max(1) as usize);
    let (block, mut at): ([f32; 2], Vec<[f32; 2]>) = match shape {
        Shape::Grid => ([w as f32 * SLOT, h as f32 * SLOT], (0..n).map(|i| [(i % w) as f32 * SLOT, (i / w) as f32 * SLOT]).collect()),
        Shape::Cooking => ([SLOT, 3.0 * SLOT], vec![[0.0, 0.0]; n]),
        _ => ([n.max(1) as f32 * SLOT, SLOT], (0..n).map(|i| [i as f32 * SLOT, 0.0]).collect()),
    };
    let arrow = [block[0] + 6.0, ((block[1] - ARROW[1]) / 2.0).floor()];
    let result = [arrow[0] + ARROW[0] + 6.0, ((block[1] - SLOT) / 2.0).floor()];
    let extra = slots - n;
    for k in 0..extra {
        at.push([result[0] + SLOT + 4.0 + (k % EXTRA_ROW) as f32 * SLOT, result[1] + (k / EXTRA_ROW) as f32 * SLOT]);
    }
    at.push(result);
    let right = at.iter().map(|p| p[0] + SLOT).fold(0.0, f32::max);
    let bottom = at.iter().map(|p| p[1] + SLOT).fold(block[1], f32::max);
    let flame = (shape == Shape::Cooking).then_some([1.0, 20.0]);
    let right = if shape == Shape::Cooking { right.max(82.0) } else { right };
    Places { size: [right, bottom], slots: at, arrow, flame }
}

/// Recipes `heights` tall cut into pages `room` high: each page's first recipe. A recipe taller than the room has
/// a page of its own.
pub fn pages(heights: &[f32], room: f32) -> Vec<usize> {
    let mut starts = vec![0];
    let mut used = 0.0;
    for (i, &h) in heights.iter().enumerate() {
        let need = if used == 0.0 { h } else { used + GAP + h };
        if used > 0.0 && need > room {
            starts.push(i);
            used = h;
        } else {
            used = need;
        }
    }
    starts
}

/// The tops of a page's recipes, `heights` tall, spread over `room` (`RecipeGuiLayouts`: the space left shared
/// between them).
pub fn spread(heights: &[f32], top: f32, room: f32) -> Vec<f32> {
    let used: f32 = heights.iter().sum();
    let space = ((room - used) / heights.len().max(1) as f32).max(0.0);
    let mut y = top + (space / 2.0).floor();
    heights
        .iter()
        .map(|h| {
            let at = y;
            y += h + space;
            at.floor()
        })
        .collect()
}

pub fn inside(p: [f32; 2], r: Rect) -> bool {
    p[0] >= r[0] && p[1] >= r[1] && p[0] < r[0] + r[2] && p[1] < r[1] + r[3]
}
