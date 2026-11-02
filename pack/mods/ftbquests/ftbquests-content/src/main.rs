//! Builds the quest book's local content mod from the player's own jars: the GUI
//! textures of FTB Quests and FTB Library, in the mod's own namespace under
//! `textures/gui`, where the game's GUI atlas reads a content mod's textures. A
//! content mod may only add files to its own namespaces, and the carried FTB
//! Quests owns `ftbquests`. Nothing it writes goes into git.
//!
//! Usage: ftbquests-content --mods <instance>/mods --out <folder>

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// The mod's textures, where the atlas reads them.
const OUT: &str = "assets/ftbquests_gui/textures/gui/";

/// Where a file of a jar goes in the mod, if it does. `.mcmeta` files stay beside
/// their textures.
fn place(name: &str) -> Option<String> {
    if !(name.ends_with(".png") || name.ends_with(".png.mcmeta")) {
        return None;
    }
    let moved = |prefix: &str, to: &str| name.strip_prefix(prefix).map(|rest| format!("{OUT}{to}{rest}"));
    moved("assets/ftbquests/textures/gui/", "")
        .or_else(|| moved("assets/ftbquests/textures/shapes/", "shapes/"))
        .or_else(|| moved("assets/ftbquests/textures/tasks/", "tasks/"))
        .or_else(|| moved("assets/ftblibrary/textures/icons/", "icons/"))
        .or_else(|| moved("assets/ftblibrary/textures/gui/background_squares", "background_squares"))
}

const MOD_TOML: &str = "# Generated from the player's FTB Quests and FTB Library jars, for local use only.\n\
id = \"ftbquests_gui\"\n\
name = \"FTB Quests GUI textures\"\n\
version = \"0.1.0\"\n\
api = \"0.2\"\n\
side = \"both\"\n\
capabilities = [\"registries\"]\n";

fn copy_from(jar: &Path, out: &Path, count: &mut BTreeMap<String, usize>) -> Result<(), String> {
    let file = fs::File::open(jar).map_err(|e| format!("{}: {e}", jar.display()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("{}: {e}", jar.display()))?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| format!("{}: {e}", jar.display()))?;
        let Some(to) = entry.name().ok().and_then(|name| place(&name)) else { continue };
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes).map_err(|e| format!("{}: {e}", jar.display()))?;
        let path = out.join(&to);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        let namespace = to.split('/').nth(1).unwrap_or_default().to_owned();
        *count.entry(namespace).or_default() += 1;
    }
    Ok(())
}

/// The jar in `mods` whose name starts with `prefix`.
fn find_jar(mods: &Path, prefix: &str) -> Result<PathBuf, String> {
    let entries = fs::read_dir(mods).map_err(|e| format!("{}: {e}", mods.display()))?;
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(prefix) && n.ends_with(".jar")))
        .collect();
    found.sort();
    found.pop().ok_or_else(|| format!("no {prefix}*.jar in {}", mods.display()))
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(PathBuf::from);
    let mods = value("--mods").ok_or("--mods <instance>/mods is needed")?;
    let out = value("--out").ok_or("--out <folder> is needed")?;
    // What an older run left, whatever it was.
    let _ = fs::remove_dir_all(out.join("assets"));
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    fs::write(out.join("horizon-mod.toml"), MOD_TOML).map_err(|e| e.to_string())?;
    let mut count = BTreeMap::new();
    for prefix in ["ftb-quests-", "ftb-library-"] {
        copy_from(&find_jar(&mods, prefix)?, &out, &mut count)?;
    }
    println!("{count:?} into {}", out.display());
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("ftbquests-content: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::place;

    #[test]
    fn textures_land_under_gui() {
        assert_eq!(place("assets/ftbquests/textures/shapes/gear/shape.png").as_deref(), Some("assets/ftbquests_gui/textures/gui/shapes/gear/shape.png"));
        assert_eq!(place("assets/ftbquests/textures/gui/close.png").as_deref(), Some("assets/ftbquests_gui/textures/gui/close.png"));
        assert_eq!(place("assets/ftblibrary/textures/icons/accept.png").as_deref(), Some("assets/ftbquests_gui/textures/gui/icons/accept.png"));
        assert_eq!(place("assets/ftblibrary/textures/gui/background_squares.png").as_deref(), Some("assets/ftbquests_gui/textures/gui/background_squares.png"));
    }

    #[test]
    fn nothing_else_comes_along() {
        assert_eq!(place("assets/ftblibrary/textures/faces/hostile.png"), None);
        assert_eq!(place("assets/ftbquests/lang/en_us.json"), None);
        assert_eq!(place("dev/ftb/mods/ftbquests/FTBQuests.class"), None);
    }
}
