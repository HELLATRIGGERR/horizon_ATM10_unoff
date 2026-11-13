//! Chat components as FTB Teams builds them: translatable lines (with the English text, used when the player's
//! language has no such key), literals, colours, click and hover. The runtime turns them into the game's text.

/// ChatFormatting colours FTB Teams uses.
pub mod color {
    pub const GRAY: u32 = 0xAAAAAA;
    pub const AQUA: u32 = 0x55FFFF;
    pub const LIGHT_PURPLE: u32 = 0xFF55FF;
    pub const GREEN: u32 = 0x55FF55;
    pub const RED: u32 = 0xFF5555;
    pub const GOLD: u32 = 0xFFAA00;
    pub const YELLOW: u32 = 0xFFFF55;
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Style {
    pub color: Option<u32>,
    pub italic: bool,
    /// `ClickEvent.RunCommand`.
    pub run: Option<String>,
    /// `ClickEvent.CopyToClipboard`.
    pub copy: Option<String>,
    pub hover: Option<Box<Msg>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    Literal(String),
    /// `Component.translatable(key, args)`, `en` the en_us line.
    Translate { key: &'static str, en: &'static str, args: Vec<Msg> },
    /// Siblings, one after the other.
    Seq(Vec<Msg>),
    Styled(Box<Msg>, Style),
}

impl Msg {
    pub fn lit(s: impl Into<String>) -> Self {
        Msg::Literal(s.into())
    }
    pub fn tr(key: &'static str, en: &'static str, args: Vec<Msg>) -> Self {
        Msg::Translate { key, en, args }
    }
    pub fn styled(self, f: impl FnOnce(&mut Style)) -> Self {
        let mut style = match &self {
            Msg::Styled(_, s) => s.clone(),
            _ => Style::default(),
        };
        f(&mut style);
        match self {
            Msg::Styled(inner, _) => Msg::Styled(inner, style),
            other => Msg::Styled(Box::new(other), style),
        }
    }
    pub fn color(self, color: u32) -> Self {
        self.styled(|s| s.color = Some(color))
    }
    pub fn then(self, other: Msg) -> Self {
        match self {
            Msg::Seq(mut items) => {
                items.push(other);
                Msg::Seq(items)
            }
            first => Msg::Seq(vec![first, other]),
        }
    }

    /// The English text, as a player without the mod's language file reads it.
    pub fn plain(&self) -> String {
        match self {
            Msg::Literal(s) => s.clone(),
            Msg::Translate { en, args, .. } => format(en, &args.iter().map(Msg::plain).collect::<Vec<_>>()),
            Msg::Seq(items) => items.iter().map(Msg::plain).collect(),
            Msg::Styled(inner, _) => inner.plain(),
        }
    }
}

/// `TranslatableContents`' `%s`, `%1$s` and `%%`.
pub fn format(template: &str, args: &[String]) -> String {
    let mut out = String::new();
    let mut next = 0;
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let mut digits = String::new();
        while let Some(d) = chars.peek().filter(|d| d.is_ascii_digit()) {
            digits.push(*d);
            chars.next();
        }
        if !digits.is_empty() && chars.peek() == Some(&'$') {
            chars.next();
        }
        match chars.next() {
            Some('%') => out.push('%'),
            Some('s') => {
                let index = digits.parse::<usize>().map_or_else(
                    |_| {
                        next += 1;
                        next - 1
                    },
                    |n| n - 1,
                );
                out.push_str(args.get(index).map_or("", String::as_str));
            }
            Some(other) => {
                out.push('%');
                out.push(other);
            }
            None => out.push('%'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_translatable_contents() {
        assert_eq!(format("%s does not belong to %s", &["A".into(), "B".into()]), "A does not belong to B");
        assert_eq!(format("%2$s then %1$s, 100%%", &["a".into(), "b".into()]), "b then a, 100%");
    }
}
