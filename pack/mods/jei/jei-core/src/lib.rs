//! JEI 19.70.0.464's ingredient list for the pack's port (bundle `jei`): the items in JEI's order and JEI's search,
//! reimplemented from what the mod does (`IngredientFilter`, `ElementPrefixParser`, `SearchTokenizer`,
//! `IngredientFilterConfig`, `IngredientSorter`). JEI is MIT (mezz); no file of it is here. The runtime module
//! (`runtime/jei_world.rs`) feeds the game's items in and shows what [`List::filter`] keeps.

#[cfg(test)]
mod tests;

/// How a search prefix takes part (`SearchMode`): `Enabled` also searches unprefixed words, `RequirePrefix` only
/// with its prefix, `Disabled` not at all (the prefix is then plain text).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Enabled,
    RequirePrefix,
    Disabled,
}

impl Mode {
    /// JEI's config names (`jei-client.ini`).
    pub fn name(self) -> &'static str {
        match self {
            Mode::Enabled => "ENABLED",
            Mode::RequirePrefix => "REQUIRE_PREFIX",
            Mode::Disabled => "DISABLED",
        }
    }
    pub fn parse(text: &str) -> Option<Mode> {
        [Mode::Enabled, Mode::RequirePrefix, Mode::Disabled].into_iter().find(|m| m.name().eq_ignore_ascii_case(text.trim()))
    }
    /// The next one, as the config button cycles it.
    pub fn next(self) -> Mode {
        match self {
            Mode::Enabled => Mode::RequirePrefix,
            Mode::RequirePrefix => Mode::Disabled,
            Mode::Disabled => Mode::Enabled,
        }
    }
}

/// The prefixed indexes, in `ElementPrefixParser`'s order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prefix {
    ModName,
    Tag,
    Tooltip,
    CreativeTab,
    Color,
    Identifier,
}

impl Prefix {
    pub const ALL: [Prefix; 6] = [Prefix::ModName, Prefix::Tag, Prefix::Tooltip, Prefix::CreativeTab, Prefix::Color, Prefix::Identifier];
    pub fn char(self) -> char {
        match self {
            Prefix::ModName => '@',
            Prefix::Tag => '#',
            Prefix::Tooltip => '$',
            Prefix::CreativeTab => '%',
            Prefix::Color => '^',
            Prefix::Identifier => '&',
        }
    }
    /// JEI's config key (`modNameSearchMode`...).
    pub fn key(self) -> &'static str {
        match self {
            Prefix::ModName => "modNameSearchMode",
            Prefix::Tag => "tagSearchMode",
            Prefix::Tooltip => "tooltipSearchMode",
            Prefix::CreativeTab => "creativeTabSearchMode",
            Prefix::Color => "colorSearchMode",
            Prefix::Identifier => "resourceLocationSearchMode",
        }
    }
    /// JEI's default (`IngredientFilterConfig`).
    pub fn default_mode(self) -> Mode {
        match self {
            Prefix::ModName | Prefix::Tag => Mode::RequirePrefix,
            Prefix::Tooltip => Mode::Enabled,
            Prefix::CreativeTab | Prefix::Color | Prefix::Identifier => Mode::Disabled,
        }
    }
    fn of(c: char) -> Option<Prefix> {
        Prefix::ALL.into_iter().find(|p| p.char() == c)
    }
}

/// The search modes, one per prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Modes(pub [Mode; 6]);

impl Default for Modes {
    fn default() -> Self {
        Modes(Prefix::ALL.map(Prefix::default_mode))
    }
}

impl Modes {
    pub fn get(&self, p: Prefix) -> Mode {
        self.0[p as usize]
    }
    pub fn set(&mut self, p: Prefix, mode: Mode) {
        self.0[p as usize] = mode;
    }
}

/// One item as the game knows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Item {
    /// `minecraft:diamond`.
    pub id: String,
    /// In the player's language.
    pub name: String,
    /// Item tags without the `#`.
    pub tags: Vec<String>,
    /// Tooltip lines besides the name and the mod name (JEI leaves those two out of `$`).
    pub tooltip: Vec<String>,
    /// The creative tabs it is in, by name.
    pub creative_tabs: Vec<String>,
    /// Its colours' names (`ColorNameConfig`), from its texture.
    pub colors: Vec<String>,
}

/// A mod's name as its jar names it, for the ATM10 mods this port knows; the namespace, capitalised, for the rest.
pub fn mod_name(namespace: &str) -> String {
    let known = match namespace {
        "minecraft" => "Minecraft",
        "ae2" => "Applied Energistics 2",
        "advanced_ae" => "Advanced AE",
        "extendedae" => "ExtendedAE",
        "megacells" => "MEGA Cells",
        "ae2wtlib" => "AE2WTLib",
        "aeinfinitybooster" => "AE Infinity Booster",
        "mekanism" => "Mekanism",
        "mekanismgenerators" => "Mekanism: Generators",
        "mekanismtools" => "Mekanism: Tools",
        "mekanismadditions" => "Mekanism: Additions",
        "alltheores" => "All The Ores",
        "allthemodium" => "Allthemodium",
        "powah" => "Powah!",
        "fluxnetworks" => "Flux Networks",
        "ftbchunks" => "FTB Chunks",
        "ftbteams" => "FTB Teams",
        "jei" => "Just Enough Items",
        _ => "",
    };
    if !known.is_empty() {
        return known.to_owned();
    }
    let mut c = namespace.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// An item ready to search: everything lowercased once.
#[derive(Clone, Debug)]
struct Entry {
    id: String,
    name: String,
    /// The mod id, the mod's name, and the name without spaces.
    mod_names: [String; 3],
    tags: Vec<String>,
    tooltip: Vec<String>,
    creative_tabs: Vec<String>,
    colors: Vec<String>,
}

/// One search term: `-` excludes, a prefix picks an index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Term {
    pub negate: bool,
    pub prefix: Option<Prefix>,
    pub text: String,
}

/// JEI's tokenizer: `|` splits alternatives; in each, spaces split words that all must match; `"..."` keeps spaces
/// in a word, `\` takes the next character as it is; a word starting with `-` excludes, then a prefix character whose
/// mode isn't `Disabled` picks its index (a disabled one stays part of the text). Lowercased.
pub fn parse(query: &str, modes: &Modes) -> Vec<Vec<Term>> {
    let mut groups: Vec<Vec<String>> = vec![Vec::new()];
    let mut word = String::new();
    let (mut quoted, mut escaped) = (false, false);
    let end_word = |groups: &mut Vec<Vec<String>>, word: &mut String| {
        if !word.is_empty() {
            groups.last_mut().unwrap().push(std::mem::take(word));
        }
    };
    for c in query.chars() {
        if escaped {
            word.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            quoted = !quoted;
        } else if c == '|' && !quoted {
            end_word(&mut groups, &mut word);
            groups.push(Vec::new());
        } else if c.is_whitespace() && !quoted {
            end_word(&mut groups, &mut word);
        } else {
            word.push(c);
        }
    }
    end_word(&mut groups, &mut word);
    groups
        .into_iter()
        .map(|words| words.into_iter().filter_map(|w| term(&w, modes)).collect::<Vec<_>>())
        .filter(|g| !g.is_empty())
        .collect()
}

fn term(word: &str, modes: &Modes) -> Option<Term> {
    let (negate, rest) = match word.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, word),
    };
    let mut chars = rest.chars();
    let prefix = chars.next().and_then(Prefix::of).filter(|p| modes.get(*p) != Mode::Disabled);
    let text = if prefix.is_some() { chars.as_str() } else { rest };
    let text = text.to_lowercase();
    // A bare prefix or `-` alone matches everything: no term.
    (!text.is_empty()).then_some(Term { negate, prefix, text })
}

/// Every item in JEI's order, and its search.
#[derive(Clone, Debug, Default)]
pub struct List {
    entries: Vec<Entry>,
}

impl List {
    /// The items in JEI's default order (`MOD_NAME, INGREDIENT_TYPE, CREATIVE_MENU`): Minecraft first, then the mods
    /// by name; within a mod, the order they came in (the registry's).
    pub fn new(items: Vec<Item>) -> List {
        let mut keyed: Vec<(bool, String, usize, Item)> = items
            .into_iter()
            .enumerate()
            .map(|(n, item)| {
                let namespace = item.id.split_once(':').map_or("minecraft", |(ns, _)| ns);
                (namespace != "minecraft", mod_name(namespace).to_lowercase(), n, item)
            })
            .collect();
        keyed.sort_by(|a, b| (a.0, &a.1, a.2).cmp(&(b.0, &b.1, b.2)));
        let lower = |v: &[String]| v.iter().map(|s| s.to_lowercase()).collect::<Vec<_>>();
        let entries = keyed
            .into_iter()
            .map(|(_, mod_lower, _, item)| {
                let namespace = item.id.split_once(':').map_or("minecraft", |(ns, _)| ns).to_owned();
                Entry {
                    name: item.name.to_lowercase(),
                    mod_names: [namespace, mod_lower.clone(), mod_lower.replace(' ', "")],
                    tags: lower(&item.tags),
                    tooltip: lower(&item.tooltip),
                    creative_tabs: lower(&item.creative_tabs),
                    colors: lower(&item.colors),
                    id: item.id,
                }
            })
            .collect();
        List { entries }
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    /// The item id at a place of the order.
    pub fn id(&self, index: usize) -> &str {
        &self.entries[index].id
    }
    /// The places of the items `query` keeps, in order: one alternative of it must hold, each of its terms
    /// matching (an excluding one not matching). An empty query keeps every item.
    pub fn filter(&self, query: &str, modes: &Modes) -> Vec<usize> {
        let groups = parse(query, modes);
        (0..self.entries.len())
            .filter(|&n| groups.is_empty() || groups.iter().any(|g| g.iter().all(|t| self.matches(n, t, modes) != t.negate)))
            .collect()
    }
    fn matches(&self, n: usize, term: &Term, modes: &Modes) -> bool {
        let e = &self.entries[n];
        let t = term.text.as_str();
        let any = |v: &[String]| v.iter().any(|s| s.contains(t));
        let index = |p: Prefix| match p {
            Prefix::ModName => any(&e.mod_names),
            Prefix::Tag => any(&e.tags),
            Prefix::Tooltip => any(&e.tooltip),
            Prefix::CreativeTab => any(&e.creative_tabs),
            Prefix::Color => any(&e.colors),
            Prefix::Identifier => e.id.contains(t),
        };
        match term.prefix {
            Some(p) => index(p),
            // Unprefixed: the name, and every index searched without its prefix.
            None => e.name.contains(t) || Prefix::ALL.into_iter().any(|p| modes.get(p) == Mode::Enabled && index(p)),
        }
    }
}

/// How many a cheat-mode click gives (`cheatOneItem`, `cheatItemStack`): one, or a stack with Shift or the right
/// button (the game cuts it to the item's stack size).
pub fn give_count(right: bool, shift: bool) -> u32 {
    if right || shift { 64 } else { 1 }
}
