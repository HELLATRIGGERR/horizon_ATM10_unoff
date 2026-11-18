//! The fork's recipe-viewer surface (JEI): mods' own recipe categories (`recipe_categories`),
//! a mod asking the viewer to show a stack (`recipe_lookup`), recipe transfers and the item
//! panel's grid. Upstream's `Stack` names a registered item only; a viewer's stack may be a
//! fluid or a mod's typed ingredient (Mekanism's chemicals), so these keep the fork's ids.

/// A stack a recipe viewer shows: an item (`item` its id, `minecraft:diamond`), a count (1 draws
/// none) and its components beyond the item's own, in vanilla's item syntax, or "". `kind` names
/// ingredient types: `"fluid"` a fluid (`item` its id, `minecraft:water`; `count` its amount in
/// mB), any other a mod's own type (`mekanism:chemical`: `item` the chemical's id, `count` its
/// amount), drawn as the recipe category's [`NativeIngredientInfo`] says. "" (the default) is an
/// item.
#[derive(Clone, Debug, Default, PartialEq)]
// exhaustive: mods build it with a literal or its constructors
#[derive(horizon_mod_api_macros::Wire)]
pub struct NativeStack {
    /// The item's, fluid's or typed ingredient's id.
    pub item: String,
    /// How many, or the amount of a fluid or typed ingredient.
    pub count: u32,
    /// The stack's data components in vanilla's item syntax, or "".
    pub components: String,
    /// "" an item, `"fluid"` a fluid, else a mod's ingredient type.
    pub kind: String,
}

impl NativeStack {
    /// The kind of a fluid.
    pub const FLUID: &'static str = "fluid";
    /// An item stack.
    pub fn item(id: &str, count: u32) -> NativeStack {
        NativeStack {
            item: id.to_owned(),
            count,
            ..NativeStack::default()
        }
    }
    /// `amount` mB of fluid `id`.
    pub fn fluid(id: &str, amount: u32) -> NativeStack {
        NativeStack::typed(Self::FLUID, id, amount)
    }
    /// `amount` of ingredient `id` of a mod's type `kind` (`mekanism:chemical`).
    pub fn typed(kind: &str, id: &str, amount: u32) -> NativeStack {
        NativeStack {
            item: id.to_owned(),
            count: amount,
            components: String::new(),
            kind: kind.to_owned(),
        }
    }
    /// Whether it is an item, not a fluid or a mod's type.
    pub fn is_item(&self) -> bool {
        self.kind.is_empty()
    }
}

/// How a mod's own ingredient type's ingredient looks in a recipe viewer (JEI's
/// `IIngredientRenderer` and `IIngredientHelper` of the type): a recipe category lists those its
/// recipes use.
#[derive(Clone, Debug, Default, PartialEq)]
// exhaustive: mods build it with a literal as their answer
#[derive(horizon_mod_api_macros::Wire)]
pub struct NativeIngredientInfo {
    /// Its type (`mekanism:chemical`), as the stacks' `kind`.
    pub kind: String,
    /// Its id (`mekanism:hydrogen`).
    pub id: String,
    /// Its name in the player's language.
    pub name: String,
    /// The GUI sprite it is drawn with (`mekanism:gui/chemical/mekanism/liquid/liquid`).
    pub sprite: String,
    /// The sprite's tint, `0xAARRGGBB`.
    pub tint: u32,
}

/// An ingredient of a mod's item panel that is not an item (`native_panel_ingredients`; JEI's
/// ingredient list holds fluids and the mods' types beside the items): how the panel draws it
/// and what its tooltip says.
#[derive(Clone, Debug, Default, PartialEq)]
// exhaustive: mods build it with a literal as their answer
#[derive(horizon_mod_api_macros::Wire)]
pub struct NativePanelIngredient {
    /// Its kind and id, as the panel's [`NativeStack`]'s; its name, the tooltip's first line and
    /// the slot widget's label; its sprite in its tint. An empty sprite draws a fluid as its
    /// fluid icon.
    pub info: NativeIngredientInfo,
    /// The tooltip's lines after the name, `§` codes read (`Text::legacy`).
    pub lines: Vec<String>,
}

/// What a mod asks the recipe viewer to do with a stack (`native_show_recipes`, heard as
/// `recipe_lookup`).
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(horizon_mod_api_macros::Wire)]
pub enum NativeRecipeLookup {
    /// The recipes that make it (JEI's R, a left click).
    Recipes,
    /// The recipes it is used in (JEI's U, a right click).
    Uses,
    /// A key (vanilla's name, `key.keyboard.a`) went down while the pointer was on the stack in a
    /// screen the mod draws: the viewer acts on it as over its own list (its recipe, uses and
    /// bookmark keys, as the player bound them).
    Key(String),
}

/// A recipe a recipe viewer moves into the open screen (`native_recipe_transfer`, JEI's "+"
/// beside a recipe).
#[derive(Clone, Debug, Default, PartialEq)]
// exhaustive: mods build it with a literal as their ask
#[derive(horizon_mod_api_macros::Wire)]
pub struct NativeRecipeTransfer {
    /// The viewer's category (`minecraft:crafting`, `minecraft:smelting`, a mod's `ae2:inscriber`).
    pub category: String,
    /// A crafting recipe's `book_id`, -1 for others.
    pub book_id: i32,
    /// The cells a crafting recipe takes (a shapeless one its square), checked against the open
    /// grid: its width.
    pub width: u32,
    /// Its height.
    pub height: u32,
    /// The input slots in the viewer's order (a crafting recipe's 3x3 grid, row by row), each
    /// slot's alternatives with the count it takes; an empty slot is empty.
    pub inputs: Vec<Vec<NativeStack>>,
    /// The outputs, the result first.
    pub outputs: Vec<NativeStack>,
}

/// The open screen's handler for a recipe viewer's transfer (JEI's `IRecipeTransferHandler`s).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(horizon_mod_api_macros::Wire)]
pub enum NativeTransferHandler {
    /// A crafting grid: the inventory's 2x2, a crafting table's, a mod menu type's (JEI's basic
    /// handler).
    CraftingGrid,
    /// A furnace's, blast furnace's or smoker's input, for its own recipe type.
    Furnace,
    /// AE2's Pattern Encoding Terminal: the recipe is encoded, no items needed.
    Ae2Encoding,
    /// AE2's crafting terminal: the ingredients come from the ME network.
    Ae2Crafting,
}

/// What a recipe viewer's transfer would do or did (`native_recipe_transfer`).
#[derive(Clone, Debug, Default, PartialEq)]
// exhaustive: the game builds it with a literal; mods only read it
#[derive(horizon_mod_api_macros::Wire)]
pub struct NativeTransferAnswer {
    /// None: the open screen takes no such recipe (JEI hides its button).
    pub handler: Option<NativeTransferHandler>,
    /// The recipe is larger than the open grid (a 3x3 one in the inventory's 2x2).
    pub too_large: bool,
    /// Input slots (indices into `inputs`) nothing on hand fills; for AE2's crafting terminal,
    /// nor its network can craft.
    pub missing: Vec<usize>,
    /// AE2: input slots its network can craft (the crafting terminal: those it lacks; the
    /// encoding terminal: any).
    pub craftable: Vec<usize>,
    /// Whether the transfer was made.
    pub done: bool,
}

/// A category of a mod's own recipes for recipe viewers (`recipe_categories`): what a fishing
/// spot, a sieve or a machine turns its inputs into, each output with its odds.
#[derive(Clone, Debug, Default, PartialEq)]
// exhaustive: mods build it with a literal as their answer
#[derive(horizon_mod_api_macros::Wire)]
pub struct NativeRecipeCategory {
    /// Its id (`mymod:sifting`).
    pub id: String,
    /// Its name, as the mod words it in the player's language.
    pub title: String,
    /// One short line under the name, or "".
    pub subtitle: String,
    /// The tab's icon.
    pub icon: NativeStack,
    /// The items and blocks that do it (a rod, a sieve), as JEI's catalysts.
    pub catalysts: Vec<NativeStack>,
    /// Its recipes.
    pub recipes: Vec<NativeCategoryRecipe>,
    /// How the mod's own ingredient types its recipes use look: Mekanism's chemicals. Items and
    /// fluids need none.
    pub ingredients: Vec<NativeIngredientInfo>,
}

/// One recipe of a [`NativeRecipeCategory`].
#[derive(Clone, Debug, Default, PartialEq)]
// exhaustive: mods build it with a literal as their answer
#[derive(horizon_mod_api_macros::Wire)]
pub struct NativeCategoryRecipe {
    /// Its input slots, each with the stacks it takes; none for a recipe with no input (a
    /// catch). A slot may hold fluids or a mod's typed ingredients (`NativeStack::kind`) as well
    /// as items.
    pub inputs: Vec<Vec<NativeStack>>,
    /// Its outputs, each with its chance.
    pub outputs: Vec<NativeChanceOutput>,
    /// Lines about the whole recipe.
    pub notes: Vec<String>,
}

/// An output of a [`NativeCategoryRecipe`] that comes with a chance.
#[derive(Clone, Debug, Default, PartialEq)]
// exhaustive: mods build it with a literal as their answer
#[derive(horizon_mod_api_macros::Wire)]
pub struct NativeChanceOutput {
    /// What comes.
    pub stack: NativeStack,
    /// The chance it comes, 0 to 1, each time the recipe runs (a catch, a block broken).
    pub chance: f32,
    /// How many come when it does, at least.
    pub min: u32,
    /// At most.
    pub max: u32,
    /// Lines about this output (`needs a brush`).
    pub notes: Vec<String>,
}

/// How big a native mod's item panel grid may grow and where it sits in the room the screen
/// leaves (`native_panel_grid`; JEI's `[ingredientList]` in `jei-client.ini`). The grid takes as
/// many columns and rows as fit, up to these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// exhaustive: mods build it with `new` or a literal as their answer
#[derive(horizon_mod_api_macros::Wire)]
pub struct NativePanelGrid {
    /// The most columns.
    pub max_columns: usize,
    /// The most rows.
    pub max_rows: usize,
    /// In the free width beside the screen (JEI's `horizontalAlignment`).
    pub horizontal: NativePanelAlign,
    /// In the height over the search box, page buttons and all (JEI's `verticalAlignment`).
    pub vertical: NativePanelAlign,
    /// Keep the page buttons and search box while the mod shows no items, so the player can
    /// type a query (JEI's `hideIngredientsWhenNotSearching`).
    pub keep_search: bool,
}

impl NativePanelGrid {
    /// Up to `max_columns` x `max_rows`, at the right of the free width, top aligned, gone with
    /// no items: the viewer style's grid until a mod asks.
    pub const fn new(max_columns: usize, max_rows: usize) -> Self {
        Self {
            max_columns,
            max_rows,
            horizontal: NativePanelAlign::End,
            vertical: NativePanelAlign::Start,
            keep_search: false,
        }
    }
}

impl Default for NativePanelGrid {
    /// Horizon's Recipes panel's: 9 columns, 16 rows.
    fn default() -> Self {
        Self::new(9, 16)
    }
}

/// Where a panel's grid sits in its room (`NativePanelGrid`): left or top (`Start`), the
/// middle, right or bottom (`End`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[derive(horizon_mod_api_macros::Wire)]
pub enum NativePanelAlign {
    /// Left or top.
    #[default]
    Start,
    /// The middle.
    Center,
    /// Right or bottom.
    End,
}

impl NativePanelAlign {
    /// JEI's `LEFT`/`TOP`, `CENTER`, `RIGHT`/`BOTTOM`.
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_uppercase().as_str() {
            "LEFT" | "TOP" => Some(Self::Start),
            "CENTER" => Some(Self::Center),
            "RIGHT" | "BOTTOM" => Some(Self::End),
            _ => None,
        }
    }
    /// The offset of a `size` long piece in `room`: 0, half the rest (rounded down) or all of
    /// it.
    pub fn offset(self, room: f32, size: f32) -> f32 {
        let rest = (room - size).max(0.0);
        match self {
            Self::Start => 0.0,
            Self::Center => (rest / 2.0).floor(),
            Self::End => rest,
        }
    }
}
