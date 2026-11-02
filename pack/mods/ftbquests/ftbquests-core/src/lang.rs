//! The book's translations: `lang/<locale>.snbt`, whose keys name an object by
//! its kind and id (`quest.5CF320E9C4C1B1E1.title`, `..quest_desc`). As
//! `TranslationManager` does, a key missing in the player's language comes from
//! the fallback language, and then from English. ATM10 also ships the same
//! tables split up under `lang/<locale>/` (`.snbt_merged`, read by
//! `ftbquestslangsplitter`): the merged file holds them all, so those stay unread
//! unless the merged file is missing.

use crate::book::{Id, LoadError, id_string};
use crate::snbt::{self, Compound, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Default)]
pub struct Lang {
    table: HashMap<String, Value>,
    fallback: HashMap<String, Value>,
}

impl Lang {
    /// `folder` is the book's `lang` folder. English is always the last fallback.
    pub fn load(folder: &Path, locale: &str, fallback: &str) -> Result<Self, LoadError> {
        let fallback = if fallback.is_empty() { "en_us" } else { fallback };
        let mut lang = Lang { table: read_table(folder, locale)?, fallback: HashMap::new() };
        if locale != fallback {
            lang.fallback = read_table(folder, fallback)?;
        }
        if fallback != "en_us" && locale != "en_us" {
            for (key, value) in read_table(folder, "en_us")? {
                lang.fallback.entry(key).or_insert(value);
            }
        }
        Ok(lang)
    }
    /// A table from one file's text, for tests.
    pub fn from_text(text: &str) -> Result<Self, snbt::Error> {
        Ok(Lang { table: into_map(snbt::parse_compound(text)?), fallback: HashMap::new() })
    }
    pub fn len(&self) -> usize {
        self.table.len()
    }
    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }
    fn value(&self, kind: &str, id: Id, key: &str) -> Option<&Value> {
        let full = format!("{kind}.{}.{key}", id_string(id));
        self.table.get(&full).or_else(|| self.fallback.get(&full))
    }
    /// One string, or a list's lines joined.
    pub fn text(&self, kind: &str, id: Id, key: &str) -> Option<String> {
        match self.value(kind, id, key)? {
            Value::Str(s) => Some(s.clone()),
            Value::List(items) => {
                Some(items.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n"))
            }
            _ => None,
        }
    }
    pub fn title(&self, kind: &str, id: Id) -> Option<String> {
        self.text(kind, id, "title")
    }
    /// A list key's lines (a lone string is one line).
    pub fn lines(&self, kind: &str, id: Id, key: &str) -> Vec<String> {
        match self.value(kind, id, key) {
            Some(Value::List(items)) => items.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect(),
            Some(Value::Str(s)) => vec![s.clone()],
            _ => Vec::new(),
        }
    }
}

fn into_map(root: Compound) -> HashMap<String, Value> {
    root.0.into_iter().collect()
}

fn read_table(folder: &Path, locale: &str) -> Result<HashMap<String, Value>, LoadError> {
    let merged = folder.join(format!("{locale}.snbt"));
    if merged.is_file() {
        return read_file(&merged).map(into_map);
    }
    let split = folder.join(locale);
    if split.is_dir() {
        let mut table = HashMap::new();
        read_split(&split, &mut table)?;
        return Ok(table);
    }
    // A language the book isn't in reads as empty: the fallback answers.
    Ok(HashMap::new())
}

fn read_file(path: &Path) -> Result<Compound, LoadError> {
    let error = |message: String| LoadError { path: path.to_owned(), message };
    let text = fs::read_to_string(path).map_err(|e| error(e.to_string()))?;
    snbt::parse_compound(&text).map_err(|e| error(e.to_string()))
}

fn read_split(dir: &Path, table: &mut HashMap<String, Value>) -> Result<(), LoadError> {
    let error = |message: String| LoadError { path: dir.to_owned(), message };
    for entry in fs::read_dir(dir).map_err(|e| error(e.to_string()))? {
        let path = entry.map_err(|e| error(e.to_string()))?.path();
        if path.is_dir() {
            read_split(&path, table)?;
        } else if path.extension().is_some_and(|e| e == "snbt_merged" || e == "snbt") {
            table.extend(read_file(&path)?.0);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_name_an_object_by_kind_and_id() {
        let lang = Lang::from_text(
            r#"{ quest.00000000000000FF.title: "Hello", quest.00000000000000FF.quest_desc: ["a", "b"], chapter.0000000000000001.chapter_subtitle: ["c"] }"#,
        )
        .unwrap();
        assert_eq!(lang.title("quest", 255).as_deref(), Some("Hello"));
        assert_eq!(lang.lines("quest", 255, "quest_desc"), ["a", "b"]);
        assert_eq!(lang.text("quest", 255, "quest_desc").as_deref(), Some("a\nb"));
        assert_eq!(lang.lines("chapter", 1, "chapter_subtitle"), ["c"]);
        assert_eq!(lang.title("quest", 1), None);
    }

    #[test]
    fn a_language_the_book_lacks_falls_back_to_english() {
        let dir = std::env::temp_dir().join(format!("ftbq-lang-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("en_us.snbt"), r#"{ quest.0000000000000001.title: "One", quest.0000000000000002.title: "Two" }"#).unwrap();
        fs::write(dir.join("uk_ua.snbt"), r#"{ quest.0000000000000001.title: "Один" }"#).unwrap();
        let uk = Lang::load(&dir, "uk_ua", "").unwrap();
        assert_eq!(uk.title("quest", 1).as_deref(), Some("Один"));
        assert_eq!(uk.title("quest", 2).as_deref(), Some("Two"));
        let none = Lang::load(&dir, "xx_xx", "").unwrap();
        assert_eq!(none.title("quest", 1).as_deref(), Some("One"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn split_tables_are_read_when_there_is_no_merged_file() {
        let dir = std::env::temp_dir().join(format!("ftbq-split-{}", std::process::id()));
        fs::create_dir_all(dir.join("en_us/chapters")).unwrap();
        fs::write(dir.join("en_us/chapter.snbt_merged"), r#"{ chapter.0000000000000001.title: "C" }"#).unwrap();
        fs::write(dir.join("en_us/chapters/a.snbt_merged"), r#"{ quest.0000000000000002.title: "Q" }"#).unwrap();
        let lang = Lang::load(&dir, "en_us", "").unwrap();
        assert_eq!(lang.title("chapter", 1).as_deref(), Some("C"));
        assert_eq!(lang.title("quest", 2).as_deref(), Some("Q"));
        fs::remove_dir_all(&dir).unwrap();
    }
}
