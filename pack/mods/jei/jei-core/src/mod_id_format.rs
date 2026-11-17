//! JEI's `config/jei/jei-mod-id-format.ini` (JEI 19.70's `ModIdFormatConfig`): `[modname] modNameFormat`, the
//! formatting of the mod name line JEI puts last in item tooltips, as `ChatFormatting` names separated by spaces
//! (default `blue italic`, which is `§9§o`); an empty value turns the line off. The file is MezzConfig's layout, as
//! `jei-client.ini`.

/// The section and key.
pub const SECTION: &str = "modname";
pub const KEY: &str = "modNameFormat";
pub const DEFAULT: &str = "blue italic";

/// A new file, in JEI's layout.
pub fn fresh() -> String {
    format!(
        "[{SECTION}]\n\t# Name: Mod Name Format\n\t# Description: Formatting for mod name tooltip. Leave blank to \
         disable.\n\t# Valid Values: A chat formatting string.\n\t# Use these formatting colors:\n\t# black dark_blue \
         dark_green dark_aqua dark_red dark_purple gold gray dark_gray blue green aqua red light_purple yellow white\n\t# \
         With these formatting options:\n\t# obfuscated bold strikethrough underline italic\n\t# Default Value: \
         {DEFAULT}\n\t{KEY} = {DEFAULT}\n"
    )
}

/// The value in a file's text: `None` when the file doesn't set it (JEI's default holds then).
pub fn read(text: &str) -> Option<String> {
    let mut section = "";
    for line in text.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name;
        } else if section == SECTION
            && let Some((key, value)) = line.split_once('=')
            && key.trim() == KEY
        {
            return Some(value.trim().to_owned());
        }
    }
    None
}

/// `ChatFormatting.getByName` for each word, as `§` codes; words it doesn't know are skipped (JEI logs them).
pub fn codes(format: &str) -> String {
    const NAMES: [(&str, char); 22] = [
        ("black", '0'), ("dark_blue", '1'), ("dark_green", '2'), ("dark_aqua", '3'), ("dark_red", '4'),
        ("dark_purple", '5'), ("gold", '6'), ("gray", '7'), ("dark_gray", '8'), ("blue", '9'), ("green", 'a'),
        ("aqua", 'b'), ("red", 'c'), ("light_purple", 'd'), ("yellow", 'e'), ("white", 'f'), ("obfuscated", 'k'),
        ("bold", 'l'), ("strikethrough", 'm'), ("underline", 'n'), ("italic", 'o'), ("reset", 'r'),
    ];
    format
        .split_whitespace()
        .filter_map(|word| {
            let clean: String = word.to_ascii_lowercase().chars().filter(|c| c.is_ascii_lowercase() || *c == '_').collect();
            NAMES.iter().find(|(name, _)| *name == clean).map(|&(_, code)| format!("§{code}"))
        })
        .collect()
}
