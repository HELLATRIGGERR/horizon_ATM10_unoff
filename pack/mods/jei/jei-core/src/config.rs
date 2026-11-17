//! JEI's client config file, `config/jei/jei-client.ini`, as JEI 19.70 has MezzConfig (`net.mezzdev.config`) keep it:
//! `[section]`, then each value as tab-indented `# Name:`, `# Description:`, `# Valid Values:` and `# Default Value:`
//! comments over `key = value`, a blank line between values and two between sections. A file is read as it is: its
//! comments, its order and keys JEI 19.70 doesn't know (an older JEI's, such as ATM10's own file) stay as written;
//! values JEI moved (`appearance.centerSearch` to `search.centerSearch`...) are taken over once (`complete`), and
//! values missing are added with their comments. The options and their defaults are `ClientConfig`,
//! `IngredientFilterConfig`, `IngredientGridConfig` and `IngredientGridSharedConfig` of JEI 19.70; their names and
//! descriptions JEI's en_us (`jei.config.client.<section>.<key>`), which the config screen shows.

/// What a value may hold.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    B,
    /// An integer in `[min, max]`.
    I(i64, i64),
    /// One of these names.
    E(&'static [&'static str]),
}
use Kind::{B, E, I};

/// One option of the file.
#[derive(Debug)]
pub struct Opt {
    pub section: &'static str,
    pub key: &'static str,
    pub kind: Kind,
    pub default: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

/// JEI's sections in the file's and the config screen's order, with the screen's names (`jei.config.client.<id>`).
pub const CATEGORIES: [(&str, &str); 10] = [
    ("search", "Search"),
    ("lists", "Lists"),
    ("ingredientList", "Ingredient List"),
    ("bookmarkList", "Bookmark List"),
    ("input", "Controls"),
    ("recipes", "Recipes"),
    ("tooltips", "Tooltips"),
    ("lookups", "Lookups"),
    ("cheating", "Cheats"),
    ("advanced", "Advanced"),
];

/// ATM10's own `config/jei/jei-client.ini` (shipped with the pack) changes these from JEI's defaults; a new file
/// starts with them.
pub const ATM10: [(&str, &str, &str); 2] = [("ingredientList", "maxRows", "99"), ("ingredientList", "maxColumns", "18")];

/// The options, by section.
pub const OPTS: &[Opt] = &[
    Opt { section: "search", key: "modNameSearchMode", kind: E(&["ENABLED", "REQUIRE_PREFIX", "DISABLED"]), default: "REQUIRE_PREFIX", name: "Search Mod Names", description: "Search by the name of the mod that adds each ingredient. Prefix: @" },
    Opt { section: "search", key: "tagSearchMode", kind: E(&["ENABLED", "REQUIRE_PREFIX", "DISABLED"]), default: "REQUIRE_PREFIX", name: "Search Ingredient Tags", description: "Search tags that group related ingredients, such as logs or ingots. Prefix: #" },
    Opt { section: "search", key: "tooltipSearchMode", kind: E(&["ENABLED", "REQUIRE_PREFIX", "DISABLED"]), default: "ENABLED", name: "Search Tooltip Text", description: "Search words shown in ingredient tooltips. Prefix: $" },
    Opt { section: "search", key: "colorSearchMode", kind: E(&["ENABLED", "REQUIRE_PREFIX", "DISABLED"]), default: "DISABLED", name: "Search Color Names", description: "Search ingredients by their main color. Prefix: ^" },
    Opt { section: "search", key: "resourceLocationSearchMode", kind: E(&["ENABLED", "REQUIRE_PREFIX", "DISABLED"]), default: "DISABLED", name: "Search Ingredient IDs", description: "Search ingredient IDs like minecraft:stone. Prefix: &" },
    Opt { section: "search", key: "creativeTabSearchMode", kind: E(&["ENABLED", "REQUIRE_PREFIX", "DISABLED"]), default: "DISABLED", name: "Search Creative Inventory Tabs", description: "Search by the creative inventory tab that contains an ingredient. Prefix: %" },
    Opt { section: "search", key: "searchAdvancedTooltips", kind: B, default: "false", name: "Search Advanced Tooltip Text", description: "Include the extra tooltip text shown when Minecraft advanced tooltips are enabled with F3 + H." },
    Opt { section: "search", key: "searchModIds", kind: B, default: "true", name: "Search Mod IDs", description: "Let mod-name search also match mod IDs, like minecraft." },
    Opt { section: "search", key: "searchModAliases", kind: B, default: "true", name: "Search Extra Mod Names", description: "Let mod-name search match extra names added by mods and plugins." },
    Opt { section: "search", key: "searchShortModNames", kind: B, default: "false", name: "Search Mod Name Initials", description: "Let mod-name search match the first letters of each word in a mod name." },
    Opt { section: "search", key: "searchIngredientAliases", kind: B, default: "true", name: "Search Extra Ingredient Names", description: "Let ingredient search match extra names added by mods and plugins." },
    Opt { section: "search", key: "centerSearch", kind: E(&["STANDARD", "CENTERED"]), default: "STANDARD", name: "JEI Search Bar Position", description: "Choose where the JEI search bar appears." },
    Opt { section: "search", key: "searchCompletionEnabled", kind: B, default: "false", name: "Search Completion", description: "Show search completion suggestions in the JEI search bar." },
    Opt { section: "search", key: "maxSearchCompletionRows", kind: I(1, 100), default: "5", name: "Maximum Visible Suggestions", description: "Maximum number of search completion suggestions shown at once." },
    Opt { section: "lists", key: "drawBackground", kind: E(&["NONE", "BACKGROUND", "BORDER_ONLY", "GRID"]), default: "NONE", name: "Background Style", description: "Choose the background behind the ingredient, bookmark, and lookup history lists." },
    Opt { section: "lists", key: "navigationVisibility", kind: E(&["ENABLED", "AUTO_HIDE", "DISABLED"]), default: "ENABLED", name: "Navigation Visibility", description: "Choose when navigation controls are shown for the ingredient and bookmark lists. Also sets the default for recipe scrollbars." },
    Opt { section: "ingredientList", key: "overlaysEnabled", kind: B, default: "true", name: "Show JEI Overlays", description: "Show or hide the ingredient and bookmark overlays. This preference is saved across game launches." },
    Opt { section: "ingredientList", key: "maxRows", kind: I(1, 100), default: "16", name: "Maximum Ingredient List Rows", description: "Limit how many ingredient rows can be shown at once." },
    Opt { section: "ingredientList", key: "maxColumns", kind: I(2, 100), default: "9", name: "Maximum Ingredient List Columns", description: "Limit how many ingredient columns can be shown at once." },
    Opt { section: "ingredientList", key: "horizontalAlignment", kind: E(&["LEFT", "CENTER", "RIGHT"]), default: "RIGHT", name: "Horizontal Alignment", description: "Horizontal alignment of the ingredient list inside the available area." },
    Opt { section: "ingredientList", key: "verticalAlignment", kind: E(&["TOP", "CENTER", "BOTTOM"]), default: "TOP", name: "Vertical Alignment", description: "Vertical alignment of the ingredient list inside the available area." },
    Opt { section: "ingredientList", key: "layoutMode", kind: E(&["RECTANGULAR", "MAXIMIZE_AVAILABLE_SPACE"]), default: "RECTANGULAR", name: "Layout Mode", description: "Choose how the ingredient list uses the available screen space." },
    Opt { section: "ingredientList", key: "hideIngredientsWhenNotSearching", kind: B, default: "false", name: "Hide Ingredients When Not Searching", description: "Hide ingredients when the search is empty." },
    Opt { section: "ingredientList", key: "toastReflowEnabled", kind: B, default: "true", name: "Move Ingredient List Away from Toasts", description: "Move the top-right ingredient list when a toast notification appears, so the toast does not cover it." },
    Opt { section: "ingredientList", key: "compactInventoryEffects", kind: B, default: "true", name: "Compact Inventory Effects When Needed", description: "Use compact status effect icons when the full bars would overlap JEI. Disable to let JEI move around the full bars instead." },
    Opt { section: "bookmarkList", key: "addBookmarksToFrontEnabled", kind: E(&["END", "FRONT"]), default: "END", name: "New Bookmark Placement", description: "Choose where newly added bookmarks appear in the bookmark list." },
    Opt { section: "bookmarkList", key: "bookmarkOutputAsRecipe", kind: B, default: "true", name: "Bookmark Output as Recipe", description: "When true, pressing the bookmark key on a recipe's output will bookmark the recipe instead of the ingredient." },
    Opt { section: "bookmarkList", key: "dragToRearrangeBookmarksEnabled", kind: B, default: "true", name: "Drag to Rearrange Bookmarks", description: "Allow bookmarks to be reordered by dragging them in the bookmark list." },
    Opt { section: "bookmarkList", key: "bookmarkTooltipPreview", kind: B, default: "true", name: "Show Bookmark Recipe Preview", description: "Show a small recipe preview when hovering over recipe bookmarks." },
    Opt { section: "bookmarkList", key: "bookmarkTooltipIngredients", kind: B, default: "false", name: "Show Bookmark Recipe Ingredients", description: "Show the ingredients needed to craft a recipe bookmark." },
    Opt { section: "bookmarkList", key: "holdShiftToShowBookmarkTooltipFeatures", kind: B, default: "true", name: "Require Key for Bookmark Details", description: "Require the configured bookmark details key before showing extra bookmark details and transfer shortcuts." },
    Opt { section: "bookmarkList", key: "maxRows", kind: I(1, 100), default: "16", name: "Maximum Bookmark List Rows", description: "Limit how many bookmark rows can be shown at once." },
    Opt { section: "bookmarkList", key: "maxColumns", kind: I(2, 100), default: "9", name: "Maximum Bookmark List Columns", description: "Limit how many bookmark columns can be shown at once." },
    Opt { section: "bookmarkList", key: "horizontalAlignment", kind: E(&["LEFT", "CENTER", "RIGHT"]), default: "LEFT", name: "Horizontal Alignment", description: "Horizontal alignment of the bookmark list inside the available area." },
    Opt { section: "bookmarkList", key: "verticalAlignment", kind: E(&["TOP", "CENTER", "BOTTOM"]), default: "TOP", name: "Vertical Alignment", description: "Vertical alignment of the bookmark list inside the available area." },
    Opt { section: "input", key: "dragDelayInMilliseconds", kind: I(0, 1000), default: "150", name: "Mouse Drag Start Delay", description: "How long a mouse button must be held before JEI treats the action as dragging." },
    Opt { section: "input", key: "smoothScrollingEnabled", kind: B, default: "true", name: "Smooth Scrolling", description: "Scroll JEI lists smoothly instead of moving one row at a time." },
    Opt { section: "input", key: "smoothScrollRate", kind: I(1, 50), default: "9", name: "Smooth Scroll Distance", description: "How far JEI scrolls for each mouse-wheel step when smooth scrolling is enabled." },
    Opt { section: "input", key: "guiResizeEnabled", kind: B, default: "true", name: "Enable GUI Resizing", description: "Resize the recipe screen, ingredient list, bookmarks, and lookup history by dragging their edges or corners." },
    Opt { section: "input", key: "recipeSlotCyclingEnabled", kind: B, default: "true", name: "Recipe Slot Cycling", description: "Automatically cycle through ingredients in recipe slots over time." },
    Opt { section: "recipes", key: "recipeGuiHeight", kind: I(175, 7680), default: "350", name: "Maximum Recipe Screen Height", description: "Limit how tall the recipe screen can grow." },
    Opt { section: "recipes", key: "recipeGuiWidth", kind: I(198, 2147483647), default: "198", name: "Recipe Screen Width", description: "Preferred recipe screen width. The screen expands when a recipe needs more room. Drag its edges or corners to resize it." },
    Opt { section: "recipes", key: "maxRecipeGuiColumns", kind: I(1, 10), default: "2", name: "Maximum Recipe Columns", description: "Limit how many columns of recipes can appear when the recipe screen is wide enough. Drag its edges or corners to make room for more columns." },
    Opt { section: "recipes", key: "navigationMode", kind: E(&["PAGED", "SCROLLING"]), default: "PAGED", name: "Recipe Navigation Mode", description: "Choose how to move through recipes within a category. Categories always use pages." },
    Opt { section: "recipes", key: "showTagRecipesEnabled", kind: B, default: "true", name: "Show Tag Recipes", description: "Show recipe pages for tags, so recipes that accept a group of ingredients can show every matching ingredient." },
    Opt { section: "recipes", key: "recipeSortingBookmarks", kind: B, default: "true", name: "Show Bookmarked Recipes First", description: "Show recipes for bookmarked ingredients before other recipes." },
    Opt { section: "recipes", key: "recipeSortingCraftable", kind: B, default: "true", name: "Show Craftable Recipes First", description: "Show recipes with available ingredients before recipes with missing ingredients." },
    Opt { section: "recipes", key: "enableRecipesGuiIngredientsSummary", kind: B, default: "false", name: "Show Recipe Ingredient Summary in Tooltips", description: "Show a quick summary of ingredients needed for a recipe when hovering over the recipe output." },
    Opt { section: "tooltips", key: "showCreativeTabNamesEnabled", kind: B, default: "false", name: "Show Creative Tab Names", description: "Show which creative inventory tab an ingredient appears in." },
    Opt { section: "tooltips", key: "tagContentTooltipEnabled", kind: B, default: "true", name: "Show Tag Contents", description: "When a recipe accepts any ingredient from a tag, show the matching ingredients in the tooltip." },
    Opt { section: "tooltips", key: "hideSingleTagContentTooltipEnabled", kind: B, default: "true", name: "Hide One-Item Tag Contents", description: "Hide the tag contents line when a recipe tag only accepts one ingredient." },
    Opt { section: "lookups", key: "lookupFluidContentsEnabled", kind: B, default: "false", name: "Look Up Fluids Inside Items", description: "When looking up recipes for a bucket or other fluid container, also include recipes for the fluid inside." },
    Opt { section: "lookups", key: "lookupBlockTagsEnabled", kind: B, default: "true", name: "Look Up Matching Block Tags", description: "When looking up recipes for a block item, also include recipes that use matching block tags." },
    Opt { section: "lookups", key: "enabled", kind: B, default: "false", name: "Show Lookup History", description: "Show a list of ingredients recently used for recipe and use lookups." },
    Opt { section: "lookups", key: "maxRows", kind: I(1, 7), default: "2", name: "Maximum Lookup History Rows", description: "Limit how many rows of recent lookups are shown on screen." },
    Opt { section: "lookups", key: "maxColumns", kind: I(2, 100), default: "9", name: "Maximum Lookup History Columns", description: "Limit how many columns of recent lookups are shown." },
    Opt { section: "lookups", key: "maxIngredients", kind: I(10, 1000), default: "100", name: "Maximum Saved Lookup History Ingredients", description: "Limit how many recent lookup ingredients JEI remembers." },
    Opt { section: "lookups", key: "displaySide", kind: E(&["LEFT", "RIGHT"]), default: "LEFT", name: "Lookup History Side", description: "Choose the side of the screen that shows recent lookups." },
    Opt { section: "cheating", key: "giveMode", kind: E(&["INVENTORY", "MOUSE_PICKUP"]), default: "MOUSE_PICKUP", name: "Cheated Ingredient Destination", description: "Choose where cheated ingredients go after they are taken from JEI." },
    Opt { section: "cheating", key: "cheatToHotbarUsingHotkeysEnabled", kind: B, default: "false", name: "Cheat Items into Hotbar with Hotkeys", description: "Allow JEI hotkeys to place cheated ingredients directly into hotbar slots." },
    Opt { section: "cheating", key: "showHiddenIngredients", kind: B, default: "false", name: "Show Hidden Ingredients in JEI", description: "Show ingredients that normally do not appear in the creative inventory." },
    Opt { section: "advanced", key: "catchRenderErrorsEnabled", kind: B, default: "true", name: "Prevent Ingredient Render Crashes", description: "Show an error placeholder when an ingredient fails to draw, instead of crashing the game." },
    Opt { section: "advanced", key: "recipeSyncWarningEnabled", kind: B, default: "true", name: "Recipe Sync Warning", description: "Show a chat warning when the server doesn't provide synced recipes to JEI (e.g. vanilla servers, or servers without JEI installed)." },
];

/// How an older JEI's value becomes 19.70's: as it is, or a boolean as one of two names.
#[derive(Clone, Copy)]
enum Map {
    Same,
    Bool(&'static str, &'static str),
}
/// The values JEI moved (`addLegacyValue`): new section and key, old section and key.
const LEGACY: [(&str, &str, &str, &str, Map); 9] = [
    ("search", "centerSearch", "appearance", "centerSearch", Map::Bool("STANDARD", "CENTERED")),
    ("recipes", "recipeGuiHeight", "appearance", "recipeGuiHeight", Map::Same),
    ("recipes", "showTagRecipesEnabled", "cheating", "showTagRecipesEnabled", Map::Same),
    ("bookmarkList", "addBookmarksToFrontEnabled", "bookmarks", "addBookmarksToFrontEnabled", Map::Bool("END", "FRONT")),
    ("bookmarkList", "dragToRearrangeBookmarksEnabled", "bookmarks", "dragToRearrangeBookmarksEnabled", Map::Same),
    ("bookmarkList", "holdShiftToShowBookmarkTooltipFeatures", "tooltips", "holdShiftToShowBookmarkTooltipFeatures", Map::Same),
    ("lists", "drawBackground", "ingredientList", "drawBackground", Map::Bool("NONE", "BACKGROUND")),
    ("lists", "navigationVisibility", "ingredientList", "buttonNavigationVisibility", Map::Same),
    ("lookups", "enabled", "lookupHistory", "enabled", Map::Same),
];

/// The option `key` of `section`.
pub fn opt(section: &str, key: &str) -> Option<&'static Opt> {
    OPTS.iter().find(|o| o.section == section && o.key == key)
}

impl Opt {
    /// The `# Valid Values:` text (MezzConfig's).
    pub fn valid(&self) -> String {
        match self.kind {
            B => "[true, false]".into(),
            I(min, max) if max == i32::MAX as i64 => format!("Any integer greater than or equal to {min}"),
            I(min, max) => format!("An integer in the range [{min}, {max}] (inclusive)"),
            E(names) => format!("[{}]", names.join(", ")),
        }
    }
    /// `value` as this option keeps it, or None when it may not hold it.
    pub fn parse(&self, value: &str) -> Option<String> {
        let v = value.trim();
        match self.kind {
            B => ["true", "false"].into_iter().find(|b| b.eq_ignore_ascii_case(v)).map(String::from),
            I(min, max) => v.parse::<i64>().ok().filter(|n| (min..=max).contains(n)).map(|n| n.to_string()),
            E(names) => names.iter().find(|n| n.eq_ignore_ascii_case(v)).map(|n| n.to_string()),
        }
    }
    /// The value after a click on its button: a boolean flips, a name goes to the next (`back`: the previous), an
    /// integer up or down by one (`big`: ten), held in its range.
    pub fn cycle(&self, value: &str, back: bool, big: bool) -> String {
        let v = self.parse(value).unwrap_or_else(|| self.default.to_owned());
        match self.kind {
            B => (v != "true").to_string(),
            I(min, max) => {
                let step = if big { 10 } else { 1 } * if back { -1 } else { 1 };
                v.parse::<i64>().unwrap_or(min).saturating_add(step).clamp(min, max).to_string()
            }
            E(names) => {
                let at = names.iter().position(|n| *n == v).unwrap_or(0);
                names[(at + if back { names.len() - 1 } else { 1 }) % names.len()].to_owned()
            }
        }
    }
    /// The value as the screen's button shows it.
    pub fn label(&self, value: &str) -> String {
        if self.key == "navigationVisibility" {
            return match value {
                "ENABLED" => "Always Show",
                "AUTO_HIDE" => "Auto Hide",
                _ => "Hide",
            }
            .into();
        }
        let named = [
            ("STANDARD", "Bottom Right"),
            ("CENTERED", "Bottom Center"),
            ("END", "End of List"),
            ("FRONT", "Front of List"),
            ("BORDER_ONLY", "Border Only"),
            ("GRID", "Background with Grid"),
            ("true", "ON"),
            ("false", "OFF"),
        ];
        if let Some((_, l)) = named.iter().find(|(n, _)| *n == value) {
            return (*l).to_owned();
        }
        if !matches!(self.kind, E(_)) {
            return value.to_owned();
        }
        let words: Vec<String> =
            value.split('_').filter(|w| !w.is_empty()).map(|w| w[..1].to_owned() + &w[1..].to_ascii_lowercase()).collect();
        words.join(" ")
    }
    /// The comments over a new value in the file.
    fn comments(&self) -> Vec<String> {
        let mut c = vec![format!("# Name: {}", self.name)];
        if !self.description.is_empty() {
            c.push(format!("# Description: {}", self.description));
        }
        c.push(format!("# Valid Values: {}", self.valid()));
        c.push(format!("# Default Value: {}", self.default));
        c
    }
}

/// A value of the file with the comments over it (`# ...`, as read).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    pub comments: Vec<String>,
    pub key: String,
    pub value: String,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Section {
    pub name: String,
    pub entries: Vec<Entry>,
}
/// The file: lines before the first section, the sections, comments after the last value.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ini {
    pub head: Vec<String>,
    pub sections: Vec<Section>,
    pub tail: Vec<String>,
}

impl Ini {
    pub fn parse(text: &str) -> Ini {
        let mut ini = Ini::default();
        let mut pending: Vec<String> = Vec::new();
        for line in text.lines() {
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            if t.starts_with('[') && t.ends_with(']') {
                if ini.sections.is_empty() {
                    ini.head.append(&mut pending);
                }
                ini.sections.push(Section { name: t[1..t.len() - 1].trim().to_owned(), entries: Vec::new() });
                continue;
            }
            if !t.starts_with('#')
                && let (Some(s), Some((k, v))) = (ini.sections.last_mut(), t.split_once('='))
            {
                let comments = std::mem::take(&mut pending);
                s.entries.push(Entry { comments, key: k.trim().to_owned(), value: v.trim().to_owned() });
            } else {
                pending.push(t.to_owned());
            }
        }
        if ini.sections.is_empty() {
            ini.head.append(&mut pending);
        }
        ini.tail = pending;
        ini
    }
    /// The file's text, in MezzConfig's layout.
    pub fn write(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if !self.head.is_empty() {
            parts.push(self.head.join("\n") + "\n");
        }
        for s in &self.sections {
            let entries: Vec<String> = s
                .entries
                .iter()
                .map(|e| {
                    let comments: String = e.comments.iter().map(|c| format!("\t{c}\n")).collect();
                    format!("{comments}\t{} = {}\n", e.key, e.value)
                })
                .collect();
            parts.push(format!("[{}]\n{}", s.name, entries.join("\n")));
        }
        if !self.tail.is_empty() {
            parts.push(self.tail.iter().map(|c| format!("\t{c}\n")).collect());
        }
        parts.join("\n\n")
    }
    /// The value as written, if any.
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        let mut entries = self.sections.iter().filter(|s| s.name == section).flat_map(|s| &s.entries);
        entries.find(|e| e.key == key).map(|e| e.value.as_str())
    }
    /// The option's value: as written when it may hold it, else its default.
    pub fn value(&self, section: &str, key: &str) -> String {
        let Some(o) = opt(section, key) else { return self.get(section, key).unwrap_or_default().to_owned() };
        self.get(section, key).and_then(|v| o.parse(v)).unwrap_or_else(|| o.default.to_owned())
    }
    pub fn flag(&self, section: &str, key: &str) -> bool {
        self.value(section, key) == "true"
    }
    pub fn int(&self, section: &str, key: &str) -> i64 {
        self.value(section, key).parse().unwrap_or(0)
    }
    /// Sets a value: written over the old one (its comments kept), else added with JEI's comments at the end of its
    /// section (made after the others when missing).
    pub fn set(&mut self, section: &str, key: &str, value: &str) {
        let mut entries = self.sections.iter_mut().filter(|s| s.name == section).flat_map(|s| &mut s.entries);
        if let Some(e) = entries.find(|e| e.key == key) {
            e.value = value.to_owned();
            return;
        }
        let comments = opt(section, key).map(Opt::comments).unwrap_or_default();
        let entry = Entry { comments, key: key.to_owned(), value: value.to_owned() };
        match self.sections.iter_mut().find(|s| s.name == section) {
            Some(s) => s.entries.push(entry),
            None => self.sections.push(Section { name: section.to_owned(), entries: vec![entry] }),
        }
    }
    /// Adds every option the file lacks: an older JEI's value where JEI moved it, else the default. True when
    /// anything was added.
    pub fn complete(&mut self) -> bool {
        let mut changed = false;
        for o in OPTS {
            if self.get(o.section, o.key).is_some() {
                continue;
            }
            let legacy = LEGACY.iter().find(|l| l.0 == o.section && l.1 == o.key).and_then(|&(_, _, s, k, map)| {
                let old = self.get(s, k)?;
                match map {
                    Map::Same => o.parse(old),
                    Map::Bool(f, t) => Some(if old.trim().eq_ignore_ascii_case("true") { t } else { f }.to_owned()),
                }
            });
            self.set(o.section, o.key, &legacy.unwrap_or_else(|| o.default.to_owned()));
            changed = true;
        }
        changed
    }
    /// A new file: every option at JEI's default, but for ATM10's own values.
    pub fn fresh() -> Ini {
        let mut ini = Ini::default();
        ini.complete();
        for (s, k, v) in ATM10 {
            ini.set(s, k, v);
        }
        ini
    }
}
