//! JEI's bookmarks (slice 3): the list the bookmark key (A) adds the hovered ingredient to, or takes it off when it
//! is there already. JEI 19.70 keeps them per world in `bookmarks.json` (`BookmarkJsonConfig`, `JsonArrayFileHelper`,
//! version 2): a JSON array whose first element is `{"version":2}` and whose others are the bookmarks, each as
//! `BookmarkCodec` writes it: `{"bookmarkType":"INGREDIENT","type":"item_stack","ingredient":{"id":"minecraft:
//! iron_ingot","count":1}}` (`RECIPE` ones hold a recipe). What the port does not draw (recipes, other ingredient
//! types, an item's components) is kept as read, in its place, so a player's file loses nothing. The older
//! `bookmarks.ini` (`LegacyBookmarkConfig`: a line each, `T:` and the stack as SNBT or JSON) is read once to import
//! its item stacks.

/// One element of the file.
#[derive(Clone, Debug, PartialEq)]
enum Entry {
    /// An item stack: its id, and the element as it was read (its components kept).
    Item { id: String, json: String },
    Other(String),
}

/// The bookmarks in their order (JEI's `addBookmarksToFrontEnabled = false`: new ones go last).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bookmarks {
    entries: Vec<Entry>,
}

/// The file's version (`BookmarkJsonConfig.VERSION`).
pub const VERSION: u32 = 2;

/// The value of `key` in a flat object text: JSON's `"key":"value"` or SNBT's `key:"value"` (single quotes too).
fn string_field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let mut from = 0;
    while let Some(at) = text[from..].find(key) {
        let start = from + at;
        let end = start + key.len();
        from = end;
        let before = text[..start].chars().next_back();
        let quoted_before = matches!(before, Some('"' | '\''));
        if !quoted_before && before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let rest = &text[end..];
        let rest = if quoted_before { rest.strip_prefix(['"', '\'']) } else { Some(rest) };
        let Some(rest) = rest.and_then(|r| r.trim_start().strip_prefix(':')) else { continue };
        let rest = rest.trim_start();
        let Some(quote) = rest.chars().next().filter(|c| matches!(c, '"' | '\'')) else { continue };
        let value = &rest[1..];
        if let Some(close) = value.find(quote) {
            return Some(&value[..close]);
        }
    }
    None
}

/// The top-level elements of a JSON array text, each as its own text. None when `text` is not an array.
fn split_array(text: &str) -> Option<Vec<&str>> {
    let body = text.trim().strip_prefix('[')?.trim_end().strip_suffix(']')?;
    let (mut out, mut depth, mut start) = (Vec::new(), 0usize, 0usize);
    let (mut quoted, mut escaped) = (false, false);
    for (i, c) in body.char_indices() {
        if quoted {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => quoted = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            '{' | '[' => depth += 1,
            '}' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                out.push(&body[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&body[start..]);
    Some(out.into_iter().map(str::trim).filter(|e| !e.is_empty()).collect())
}

/// The element of an item stack bookmark, as `BookmarkCodec` writes it.
fn item_element(id: &str) -> String {
    format!("{{\"bookmarkType\":\"INGREDIENT\",\"type\":\"item_stack\",\"ingredient\":{{\"id\":\"{id}\",\"count\":1}}}}")
}

impl Bookmarks {
    /// `bookmarks.json`'s text. An element that is an item stack bookmark is one; a version header is dropped
    /// (written again); anything else is kept as it is. A text that is not a JSON array holds nothing.
    pub fn parse_json(text: &str) -> Bookmarks {
        let entries = split_array(text)
            .unwrap_or_default()
            .into_iter()
            .filter(|e| e.contains("bookmarkType") || !e.contains("\"version\""))
            .map(|e| {
                let item = string_field(e, "bookmarkType") == Some("INGREDIENT") && string_field(e, "type") == Some("item_stack");
                match item.then(|| string_field(e, "id")).flatten() {
                    Some(id) => Entry::Item { id: id.to_owned(), json: e.to_owned() },
                    None => Entry::Other(e.to_owned()),
                }
            })
            .collect();
        Bookmarks { entries }
    }
    /// `bookmarks.json`'s text, as `JsonArrayWriter` lays it out: `[`, the version, the bookmarks, each on its own
    /// line, `]`.
    pub fn write_json(&self) -> String {
        let mut out = format!("[\n{{\"version\":{VERSION}}}");
        for entry in &self.entries {
            out.push_str(",\n");
            out.push_str(match entry {
                Entry::Item { json, .. } | Entry::Other(json) => json,
            });
        }
        out.push_str("\n]");
        out
    }
    /// The legacy `bookmarks.ini`'s item stacks (`T:` lines): what JEI imports once. The recipe and ingredient
    /// markers need JEI's registries and are left to the backup of the file.
    pub fn import_ini(text: &str) -> Bookmarks {
        let entries = text
            .lines()
            .filter_map(|l| l.trim().strip_prefix("T:"))
            .filter_map(|stack| string_field(stack, "id"))
            .map(|id| Entry::Item { id: id.to_owned(), json: item_element(id) })
            .collect();
        Bookmarks { entries }
    }
    /// Adds `other`'s item stacks after these ones, skipping what is bookmarked already (JEI's load adds the JSON's
    /// bookmarks, then the legacy file's).
    pub fn merge(&mut self, other: Bookmarks) {
        for entry in other.entries {
            match &entry {
                Entry::Item { id, .. } if self.contains(id) => {}
                _ => self.entries.push(entry),
            }
        }
    }
    /// The bookmarked items' ids, in order: what the list shows.
    pub fn items(&self) -> Vec<&str> {
        self.entries
            .iter()
            .filter_map(|e| match e {
                Entry::Item { id, .. } => Some(id.as_str()),
                Entry::Other(_) => None,
            })
            .collect()
    }
    pub fn contains(&self, item: &str) -> bool {
        self.items().contains(&item)
    }
    pub fn is_empty(&self) -> bool {
        self.items().is_empty()
    }
    /// The bookmark key on `item`: added at the end, or removed when it is bookmarked already (JEI's
    /// `BookmarkInputHandler`). True when it was added.
    pub fn toggle(&mut self, item: &str) -> bool {
        self.toggle_at(item, false)
    }
    /// As `toggle`, a new one at the front when `front` (`bookmarkList.addBookmarksToFrontEnabled = FRONT`): after the
    /// file's version element and anything else that isn't an item, as JEI puts it first in the list.
    pub fn toggle_at(&mut self, item: &str, front: bool) -> bool {
        if front && !self.contains(item) {
            let at = self.entries.iter().position(|e| matches!(e, Entry::Item { .. })).unwrap_or(self.entries.len());
            self.entries.insert(at, Entry::Item { id: item.to_owned(), json: item_element(item) });
            return true;
        }
        if self.contains(item) {
            self.entries.retain(|e| !matches!(e, Entry::Item { id, .. } if id == item));
            false
        } else {
            self.entries.push(Entry::Item { id: item.to_owned(), json: item_element(item) });
            true
        }
    }
    /// The `index`th bookmarked item taken off.
    pub fn remove(&mut self, index: usize) -> Option<String> {
        let item = self.items().get(index)?.to_string();
        self.toggle(&item);
        Some(item)
    }
}

/// The bookmark list's slots: as many 18 px slots as `area` (`[x, y, w, h]`, under its 20 px page row) holds, at
/// most JEI's `bookmarkList` `maxColumns` 9 and `maxRows` 16, from its top left. Each slot's top left.
pub fn grid(area: [f32; 4]) -> Vec<[f32; 2]> {
    grid_within(area, 9, 16)
}
/// As `grid`, at most `max_columns` by `max_rows` (the config's `bookmarkList.maxColumns` and `maxRows`).
pub fn grid_within(area: [f32; 4], max_columns: usize, max_rows: usize) -> Vec<[f32; 2]> {
    let columns = ((area[2] / SLOT).floor() as usize).min(max_columns);
    let rows = (((area[3] - NAV) / SLOT).floor().max(0.0) as usize).min(max_rows);
    (0..rows * columns)
        .map(|k| [area[0] + (k % columns) as f32 * SLOT, area[1] + NAV + (k / columns) as f32 * SLOT])
        .collect()
}
/// The page row over the bookmarks.
pub const NAV: f32 = 20.0;
/// A slot.
pub const SLOT: f32 = 18.0;

/// How many pages `count` bookmarks make at `per_page` a page (at least one).
pub fn pages(count: usize, per_page: usize) -> usize {
    count.div_ceil(per_page.max(1)).max(1)
}
