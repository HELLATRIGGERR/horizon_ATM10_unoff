//! FTB's rich text, as quest titles and descriptions write it (FTB Library's
//! `TextComponentParser`, FTB Quests' `TextUtils.parseRawText`): `&a` colour and
//! style codes, `\&` for a plain ampersand, `{key}` for a translation, one
//! `{image:... width:.. height:.. align:..}` per line, `{@pagebreak}` between
//! pages, and vanilla's raw JSON text when a line is one. Lines are measured
//! and wrapped here too, with the game's text width passed in.

/// Colours of the sixteen `&` codes, as vanilla's `ChatFormatting`.
const COLORS: [u32; 16] = [
    0x000000, 0x0000AA, 0x00AA00, 0x00AAAA, 0xAA0000, 0xAA00AA, 0xFFAA00, 0xAAAAAA, 0x555555, 0x5555FF, 0x55FF55,
    0x55FFFF, 0xFF5555, 0xFF55FF, 0xFFFF55, 0xFFFFFF,
];

pub const PAGE_BREAK: &str = "{@pagebreak}";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub color: Option<u32>,
    pub bold: bool,
    pub italic: bool,
    pub underlined: bool,
    pub strikethrough: bool,
    pub obfuscated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub style: Style,
}

/// `{image:...}`: a picture with its size in GUI pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageLine {
    pub image: String,
    pub width: i32,
    pub height: i32,
    pub align: String,
}

/// One line of a description.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    Text(Vec<Run>),
    Image(ImageLine),
    PageBreak,
}

/// Runs for a raw string; a translator answers `{key}`.
pub fn parse_runs(raw: &str, translate: &dyn Fn(&str) -> Option<String>) -> Vec<Run> {
    parse_runs_at(raw, translate, 0)
}

fn parse_runs_at(raw: &str, translate: &dyn Fn(&str) -> Option<String>, depth: u8) -> Vec<Run> {
    let trimmed = raw.trim();
    if ((trimmed.starts_with('[') && trimmed.ends_with(']')) || (trimmed.starts_with('{') && trimmed.ends_with('}')))
        && depth == 0
        && let Some(runs) = json_runs(trimmed, translate)
    {
        return runs;
    }
    let mut out: Vec<Run> = Vec::new();
    let mut style = Style::default();
    let mut text = String::new();
    let mut chars = raw.char_indices().peekable();
    let flush = |text: &mut String, style: Style, out: &mut Vec<Run>| {
        if !text.is_empty() {
            out.push(Run { text: std::mem::take(text), style });
        }
    };
    while let Some((at, c)) = chars.next() {
        match c {
            '\\' => match chars.peek().map(|&(_, n)| n) {
                // `\n` as two characters: the files write newlines this way.
                Some('n') => {
                    chars.next();
                    text.push('\n');
                }
                Some(n @ ('&' | '{' | '}' | '\\')) => {
                    chars.next();
                    text.push(n);
                }
                _ => text.push('\\'),
            },
            '&' => {
                let Some(&(_, code)) = chars.peek() else {
                    text.push('&');
                    continue;
                };
                let changed = apply_code(code, &mut style);
                if let Some(next) = changed {
                    chars.next();
                    flush(&mut text, style, &mut out);
                    style = next;
                } else if code == '#' {
                    // `&#RRGGBB`.
                    let hex = raw.get(at + 2..at + 8);
                    if let Some(rgb) = hex.and_then(|h| u32::from_str_radix(h, 16).ok()) {
                        for _ in 0..7 {
                            chars.next();
                        }
                        flush(&mut text, style, &mut out);
                        style = Style { color: Some(rgb), ..style };
                    } else {
                        text.push('&');
                    }
                } else {
                    text.push('&');
                }
            }
            '{' => {
                let Some(end) = raw[at..].find('}') else {
                    text.push('{');
                    continue;
                };
                let key = &raw[at + 1..at + end];
                while chars.peek().is_some_and(|&(i, _)| i <= at + end) {
                    chars.next();
                }
                if depth < 4
                    && let Some(found) = substitute(key, translate)
                {
                    flush(&mut text, style, &mut out);
                    let inner = parse_runs_at(&found, translate, depth + 1);
                    for mut run in inner {
                        if run.style == Style::default() {
                            run.style = style;
                        }
                        out.push(run);
                    }
                } else {
                    text.push('{');
                    text.push_str(key);
                    text.push('}');
                }
            }
            _ => text.push(c),
        }
    }
    flush(&mut text, style, &mut out);
    out
}

/// The style a code gives, or None when it isn't one.
fn apply_code(code: char, style: &mut Style) -> Option<Style> {
    let mut next = *style;
    match code {
        '0'..='9' | 'a'..='f' => {
            let i = code.to_digit(16)? as usize;
            next = Style { color: Some(COLORS[i]), ..Style::default() };
        }
        'k' => next.obfuscated = true,
        'l' => next.bold = true,
        'm' => next.strikethrough = true,
        'n' => next.underlined = true,
        'o' => next.italic = true,
        'r' => next = Style::default(),
        _ => return None,
    }
    Some(next)
}

/// `ClientTextComponentUtils.defaultStringToComponent`: `{open_url:.. text:..}`
/// shows its text; anything else is a translation key.
fn substitute(key: &str, translate: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    if key.is_empty() {
        return Some(String::new());
    }
    if key.contains(':') && !key.contains('.') {
        let map = properties(key);
        if let Some((_, text)) = map.iter().find(|(k, _)| k == "text") {
            return Some(text.clone());
        }
    }
    translate(key)
}

/// `StringUtils.splitProperties`: `key:value` words; a value may be in quotes.
fn properties(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        let Some(colon) = rest.find(':') else { break };
        let key = rest[..colon].trim().to_owned();
        let after = &rest[colon + 1..];
        // A value runs up to the next ` word:`, so a path's colons and a text's spaces stay in it.
        let mut end = after.len();
        for (i, _) in after.match_indices(' ') {
            let word = after[i + 1..].split(' ').next().unwrap_or("");
            if let Some(c) = word.find(':') {
                if c > 0 && word[..c].bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') && !word[c + 1..].starts_with('/') {
                    end = i;
                    break;
                }
            }
        }
        let value = after[..end].trim().trim_matches('"').to_owned();
        out.push((key, value));
        rest = after[end..].trim_start();
    }
    out
}

/// One description line: a page break, an image, or text.
pub fn parse_line(raw: &str, translate: &dyn Fn(&str) -> Option<String>) -> Line {
    let trimmed = raw.trim();
    if trimmed == PAGE_BREAK {
        return Line::PageBreak;
    }
    if let Some(inner) = trimmed.strip_prefix('{').and_then(|t| t.strip_suffix('}'))
        && inner.starts_with("image:")
    {
        let map = properties(inner);
        let get = |key: &str| map.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
        return Line::Image(ImageLine {
            image: get("image").unwrap_or_default().to_owned(),
            width: get("width").and_then(|w| w.parse().ok()).unwrap_or(100),
            height: get("height").and_then(|w| w.parse().ok()).unwrap_or(100),
            align: get("align").unwrap_or("center").to_owned(),
        });
    }
    Line::Text(parse_runs(raw, translate))
}

/// Plain text with codes and keys resolved.
pub fn plain(raw: &str, translate: &dyn Fn(&str) -> Option<String>) -> String {
    parse_runs(raw, translate).into_iter().map(|r| r.text).collect()
}

/// `Component.Serializer.fromJson` for what quests use: a string, an object
/// with `text` or `translate`, colour and style flags, `extra`, or a list whose
/// head styles its tail. None when the text isn't JSON at all: it is plain text
/// that happens to start with a bracket.
fn json_runs(text: &str, translate: &dyn Fn(&str) -> Option<String>) -> Option<Vec<Run>> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    if !matches!(value, serde_json::Value::Array(_) | serde_json::Value::Object(_)) {
        return None;
    }
    let mut out = Vec::new();
    json_into(&value, Style::default(), translate, &mut out, 0);
    Some(out)
}

fn json_into(
    value: &serde_json::Value,
    parent: Style,
    translate: &dyn Fn(&str) -> Option<String>,
    out: &mut Vec<Run>,
    depth: u8,
) {
    if depth > 16 {
        return;
    }
    match value {
        serde_json::Value::String(s) => push_text(s, parent, out),
        serde_json::Value::Number(n) => push_text(&n.to_string(), parent, out),
        serde_json::Value::Bool(b) => push_text(&b.to_string(), parent, out),
        serde_json::Value::Array(items) => {
            // The first element styles the rest (`Component.Serializer`).
            let mut style = parent;
            for (i, item) in items.iter().enumerate() {
                if i == 0 {
                    style = json_style(item, parent);
                    json_into(item, parent, translate, out, depth + 1);
                } else {
                    json_into(item, style, translate, out, depth + 1);
                }
            }
        }
        serde_json::Value::Object(map) => {
            let style = json_style(value, parent);
            if let Some(t) = map.get("text").and_then(|t| t.as_str()) {
                push_text(t, style, out);
            } else if let Some(key) = map.get("translate").and_then(|t| t.as_str()) {
                let found = translate(key).or_else(|| map.get("fallback").and_then(|f| f.as_str()).map(str::to_owned));
                push_text(found.as_deref().unwrap_or(key), style, out);
            } else if let Some(key) = map.get("keybind").and_then(|t| t.as_str()) {
                push_text(key, style, out);
            }
            if let Some(serde_json::Value::Array(extra)) = map.get("extra") {
                for item in extra {
                    json_into(item, style, translate, out, depth + 1);
                }
            }
        }
        serde_json::Value::Null => {}
    }
}

fn push_text(text: &str, style: Style, out: &mut Vec<Run>) {
    if !text.is_empty() {
        out.push(Run { text: text.replace("\\n", "\n"), style });
    }
}

fn json_style(value: &serde_json::Value, parent: Style) -> Style {
    let mut style = parent;
    let Some(map) = value.as_object() else { return style };
    if let Some(color) = map.get("color").and_then(|c| c.as_str()) {
        style.color = named_color(color);
    }
    let flag = |name: &str, current: bool| map.get(name).and_then(|f| f.as_bool()).unwrap_or(current);
    style.bold = flag("bold", style.bold);
    style.italic = flag("italic", style.italic);
    style.underlined = flag("underlined", style.underlined);
    style.strikethrough = flag("strikethrough", style.strikethrough);
    style.obfuscated = flag("obfuscated", style.obfuscated);
    style
}

fn named_color(name: &str) -> Option<u32> {
    if let Some(hex) = name.strip_prefix('#') {
        return u32::from_str_radix(hex, 16).ok();
    }
    const NAMES: [&str; 16] = [
        "black", "dark_blue", "dark_green", "dark_aqua", "dark_red", "dark_purple", "gold", "gray", "dark_gray", "blue",
        "green", "aqua", "red", "light_purple", "yellow", "white",
    ];
    NAMES.iter().position(|n| *n == name).map(|i| COLORS[i])
}

/// Wraps runs to `max` GUI pixels, breaking at spaces and at newlines; a word
/// wider than the line is cut. `width` measures a piece of text in that style
/// (bold text is a pixel a character wider in the game).
pub fn wrap(runs: &[Run], max: f32, width: &dyn Fn(&str, Style) -> f32) -> Vec<Vec<Run>> {
    let mut lines: Vec<Vec<Run>> = Vec::new();
    let mut line: Vec<Run> = Vec::new();
    let mut used = 0.0f32;
    let push = |line: &mut Vec<Run>, text: &str, style: Style| {
        if let Some(last) = line.last_mut().filter(|l| l.style == style) {
            last.text.push_str(text);
        } else {
            line.push(Run { text: text.to_owned(), style });
        }
    };
    for run in runs {
        for (n, para) in run.text.split('\n').enumerate() {
            if n > 0 {
                lines.push(std::mem::take(&mut line));
                used = 0.0;
            }
            for piece in split_keep_spaces(para) {
                let w = width(piece, run.style);
                if used + w <= max {
                    push(&mut line, piece, run.style);
                    used += w;
                    continue;
                }
                if piece.trim().is_empty() {
                    // A space at the edge ends the line.
                    if !line.is_empty() {
                        lines.push(std::mem::take(&mut line));
                        used = 0.0;
                    }
                    continue;
                }
                if !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                }
                if w <= max {
                    push(&mut line, piece, run.style);
                    used = w;
                    continue;
                }
                // A word wider than a line: by characters.
                let mut part = String::new();
                for c in piece.chars() {
                    let mut next = part.clone();
                    next.push(c);
                    if width(&next, run.style) > max && !part.is_empty() {
                        push(&mut line, &part, run.style);
                        lines.push(std::mem::take(&mut line));
                        part.clear();
                    }
                    part.push(c);
                }
                used = width(&part, run.style);
                push(&mut line, &part, run.style);
            }
        }
    }
    lines.push(line);
    lines
}

/// Words and the spaces between them, in order.
fn split_keep_spaces(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut in_space = None;
    for (i, c) in text.char_indices() {
        let space = c == ' ';
        match in_space {
            Some(was) if was != space => {
                out.push(&text[start..i]);
                start = i;
            }
            _ => {}
        }
        in_space = Some(space);
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn colour_codes_split_runs_and_reset() {
        let runs = parse_runs("plain &aGreen &lbold&r end", &none);
        assert_eq!(runs[0].text, "plain ");
        assert_eq!(runs[0].style, Style::default());
        assert_eq!(runs[1].text, "Green ");
        assert_eq!(runs[1].style.color, Some(0x55FF55));
        assert_eq!(runs[2].text, "bold");
        assert!(runs[2].style.bold);
        assert_eq!(runs[2].style.color, Some(0x55FF55));
        assert_eq!(runs[3].text, " end");
        assert_eq!(runs[3].style, Style::default());
    }

    #[test]
    fn a_colour_clears_the_style_as_vanilla_does() {
        let runs = parse_runs("&lA&cB", &none);
        assert!(runs[0].style.bold);
        assert!(!runs[1].style.bold);
        assert_eq!(runs[1].style.color, Some(0xFF5555));
    }

    #[test]
    fn ampersands_that_are_not_codes_stay() {
        assert_eq!(plain("Fish & Chips", &none), "Fish & Chips");
        assert_eq!(plain("A\\&aB", &none), "A&aB");
        assert_eq!(plain("end&", &none), "end&");
    }

    #[test]
    fn hex_colours() {
        let runs = parse_runs("&#112233x", &none);
        assert_eq!(runs[0].text, "x");
        assert_eq!(runs[0].style.color, Some(0x112233));
    }

    #[test]
    fn backslash_n_is_a_newline() {
        assert_eq!(plain("a\\nb", &none), "a\nb");
        assert_eq!(plain("a\\qb", &none), "a\\qb");
    }

    #[test]
    fn braces_translate_or_stay() {
        let tr = |k: &str| (k == "item.minecraft.stone").then(|| "Stone".to_owned());
        assert_eq!(plain("Mine {item.minecraft.stone}!", &tr), "Mine Stone!");
        assert_eq!(plain("{no.such.key}", &tr), "{no.such.key}");
    }

    #[test]
    fn a_translation_takes_the_style_around_it() {
        let tr = |_: &str| Some("x".to_owned());
        let runs = parse_runs("&a{k}", &tr);
        assert_eq!(runs[0].style.color, Some(0x55FF55));
    }

    #[test]
    fn image_lines_read_their_properties() {
        let line = parse_line("{image:atm:textures/questpics/a/b.png width:150 height:75 align:left}", &none);
        assert_eq!(
            line,
            Line::Image(ImageLine {
                image: "atm:textures/questpics/a/b.png".into(),
                width: 150,
                height: 75,
                align: "left".into()
            })
        );
        assert_eq!(parse_line("{@pagebreak}", &none), Line::PageBreak);
        assert!(matches!(parse_line("text", &none), Line::Text(_)));
    }

    #[test]
    fn raw_json_reads() {
        let runs = parse_runs(r#"{"text":"Hi ","color":"gold","extra":[{"text":"there","bold":true}]}"#, &none);
        assert_eq!(runs[0].text, "Hi ");
        assert_eq!(runs[0].style.color, Some(0xFFAA00));
        assert_eq!(runs[1].text, "there");
        assert!(runs[1].style.bold);
        assert_eq!(runs[1].style.color, Some(0xFFAA00));
        let list = parse_runs(r#"["",{"text":"a","color":"red"},"b"]"#, &none);
        assert_eq!(list[0].text, "a");
        assert_eq!(list[1].text, "b");
        assert_eq!(list[1].style.color, None);
    }

    #[test]
    fn brackets_that_are_not_json_are_text() {
        assert_eq!(plain("[Linux user joke goes here]", &none), "[Linux user joke goes here]");
    }

    #[test]
    fn wrap_breaks_at_spaces_and_newlines_and_cuts_long_words() {
        let width = |t: &str, _: Style| t.chars().count() as f32;
        let run = |t: &str| Run { text: t.to_owned(), style: Style::default() };
        let text = |lines: Vec<Vec<Run>>| -> Vec<String> {
            lines.iter().map(|l| l.iter().map(|r| r.text.as_str()).collect::<String>()).collect()
        };
        assert_eq!(text(wrap(&[run("aaa bbb ccc")], 7.0, &width)), ["aaa bbb", "ccc"]);
        assert_eq!(text(wrap(&[run("a\nb")], 7.0, &width)), ["a", "b"]);
        assert_eq!(text(wrap(&[run("abcdefgh")], 3.0, &width)), ["abc", "def", "gh"]);
        assert_eq!(text(wrap(&[run("")], 3.0, &width)), [""]);
    }

    #[test]
    fn wrap_keeps_styles_across_a_line() {
        let width = |t: &str, _: Style| t.chars().count() as f32;
        let runs = parse_runs("aa &cbb cc", &none);
        let lines = wrap(&runs, 5.0, &width);
        assert_eq!(lines[0][1].style.color, Some(0xFF5555));
        assert_eq!(lines[1][0].style.color, Some(0xFF5555));
    }
}
