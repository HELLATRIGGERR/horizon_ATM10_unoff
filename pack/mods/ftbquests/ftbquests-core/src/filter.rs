//! FTB Filter System's smart filters, as quest files carry them in the
//! `ftbfiltersystem:filter` component of a `ftbfiltersystem:smart_filter` item:
//! `or(item(a:b)item_tag(c:d))`, each node a name with its argument in
//! brackets, the namespace `ftbfiltersystem:` optional.

/// What the game can say about one stack, to test a filter against.
pub trait ItemFacts {
    fn id(&self) -> &str;
    fn in_tag(&self, tag: &str) -> bool;
    fn is_block_item(&self) -> bool;
    /// Whether the stack has these components (`mode` is `fuzzy` or `strict`,
    /// `json` the filter's argument).
    fn has_components(&self, mode: &str, json: &str) -> bool;
}

#[derive(Clone, Debug, PartialEq)]
pub enum Filter {
    Item(String),
    Tag(String),
    /// The item's namespace.
    Mod(String),
    Block,
    Component { mode: String, json: String },
    Or(Vec<Filter>),
    And(Vec<Filter>),
    Not(Box<Filter>),
    /// A node this port doesn't read: it matches nothing, and keeps its text.
    Unknown(String),
}

impl Filter {
    pub fn parse(text: &str) -> Self {
        let mut p = Reader { text, at: 0 };
        let nodes = p.nodes();
        match nodes.len() {
            1 => nodes.into_iter().next().unwrap_or(Filter::Unknown(String::new())),
            _ => Filter::Or(nodes),
        }
    }
    /// `SmartFilter.test`.
    pub fn matches(&self, item: &dyn ItemFacts) -> bool {
        match self {
            Filter::Item(id) => id == item.id(),
            Filter::Tag(tag) => item.in_tag(tag),
            Filter::Mod(name) => item.id().split_once(':').is_some_and(|(ns, _)| ns == name),
            Filter::Block => item.is_block_item(),
            Filter::Component { mode, json } => item.has_components(mode, json),
            Filter::Or(all) => all.iter().any(|f| f.matches(item)),
            Filter::And(all) => all.iter().all(|f| f.matches(item)),
            Filter::Not(inner) => !inner.matches(item),
            Filter::Unknown(_) => false,
        }
    }
    /// The items it names, for icons: `ItemTask.getValidDisplayItems` without
    /// the registry walk a tag or a namespace would take.
    pub fn named_items(&self, out: &mut Vec<String>) {
        match self {
            Filter::Item(id) => out.push(id.clone()),
            Filter::Or(all) | Filter::And(all) => all.iter().for_each(|f| f.named_items(out)),
            _ => {}
        }
    }
    /// The tags it names, for icons and tooltips.
    pub fn named_tags(&self, out: &mut Vec<String>) {
        match self {
            Filter::Tag(tag) => out.push(tag.clone()),
            Filter::Or(all) | Filter::And(all) => all.iter().for_each(|f| f.named_tags(out)),
            _ => {}
        }
    }
}

struct Reader<'a> {
    text: &'a str,
    at: usize,
}

impl Reader<'_> {
    fn nodes(&mut self) -> Vec<Filter> {
        let mut out = Vec::new();
        while self.at < self.text.len() {
            let rest = &self.text[self.at..];
            let Some(open) = rest.find('(') else { break };
            let name = rest[..open].trim();
            let name = name.strip_prefix("ftbfiltersystem:").unwrap_or(name);
            self.at += open + 1;
            let body = self.body();
            out.push(node(name, body));
            // Spaces or commas between nodes, and a closing bracket of the parent's read elsewhere.
            while self.text[self.at..].starts_with([' ', ',']) {
                self.at += 1;
            }
        }
        out
    }
    /// Up to the bracket that closes this node, past quotes and braces.
    fn body(&mut self) -> &str {
        let start = self.at;
        let mut depth = 1;
        let mut quoted = false;
        let bytes = self.text.as_bytes();
        while self.at < bytes.len() {
            let b = bytes[self.at];
            if quoted {
                if b == b'\\' {
                    self.at += 1;
                } else if b == b'"' {
                    quoted = false;
                }
            } else {
                match b {
                    b'"' => quoted = true,
                    b'(' => depth += 1,
                    b')' => {
                        depth -= 1;
                        if depth == 0 {
                            let body = &self.text[start..self.at];
                            self.at += 1;
                            return body;
                        }
                    }
                    _ => {}
                }
            }
            self.at += 1;
        }
        self.at = self.text.len();
        &self.text[start..]
    }
}

fn children(body: &str) -> Vec<Filter> {
    Reader { text: body, at: 0 }.nodes()
}

fn node(name: &str, body: &str) -> Filter {
    match name {
        "item" => Filter::Item(body.trim().to_owned()),
        "item_tag" => Filter::Tag(body.trim().trim_start_matches('#').to_owned()),
        "mod" => Filter::Mod(body.trim().to_owned()),
        "block" => Filter::Block,
        "component" => {
            let (mode, json) = body.split_once(':').unwrap_or(("fuzzy", body));
            Filter::Component { mode: mode.trim().to_owned(), json: json.trim().to_owned() }
        }
        "or" => Filter::Or(children(body)),
        "and" => Filter::And(children(body)),
        "not" => {
            let mut inner = children(body);
            match inner.len() {
                1 => Filter::Not(Box::new(inner.remove(0))),
                _ => Filter::Not(Box::new(Filter::Or(inner))),
            }
        }
        other => Filter::Unknown(format!("{other}({body})")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fact(&'static str, &'static [&'static str], bool);

    impl ItemFacts for Fact {
        fn id(&self) -> &str {
            self.0
        }
        fn in_tag(&self, tag: &str) -> bool {
            self.1.contains(&tag)
        }
        fn is_block_item(&self) -> bool {
            self.2
        }
        fn has_components(&self, _mode: &str, json: &str) -> bool {
            json.contains("yes")
        }
    }

    #[test]
    fn or_of_items() {
        let f = Filter::parse("or(item(aether:a)item(aether:b))");
        assert_eq!(f, Filter::Or(vec![Filter::Item("aether:a".into()), Filter::Item("aether:b".into())]));
        assert!(f.matches(&Fact("aether:b", &[], false)));
        assert!(!f.matches(&Fact("aether:c", &[], false)));
        let mut items = Vec::new();
        f.named_items(&mut items);
        assert_eq!(items, ["aether:a", "aether:b"]);
    }

    #[test]
    fn the_namespace_is_optional_and_tags_read() {
        let f = Filter::parse("ftbfiltersystem:item_tag(c:chests)");
        assert_eq!(f, Filter::Tag("c:chests".into()));
        assert!(f.matches(&Fact("minecraft:chest", &["c:chests"], true)));
        assert!(!f.matches(&Fact("minecraft:stone", &[], true)));
    }

    #[test]
    fn and_of_a_tag_and_a_mod() {
        let f = Filter::parse("or(and(item_tag(curios:charm)mod(apotheosis)))");
        assert!(f.matches(&Fact("apotheosis:x", &["curios:charm"], false)));
        assert!(!f.matches(&Fact("other:x", &["curios:charm"], false)));
        assert!(!f.matches(&Fact("apotheosis:x", &[], false)));
    }

    #[test]
    fn components_keep_their_brackets_and_quotes() {
        let f = Filter::parse(r#"or(component(fuzzy:{"gateways:gateway":"apotheosis:tiered/frontier (yes)"})item(a:b))"#);
        let Filter::Or(all) = &f else { panic!("{f:?}") };
        assert_eq!(all.len(), 2);
        let Filter::Component { mode, json } = &all[0] else { panic!("{f:?}") };
        assert_eq!(mode, "fuzzy");
        assert!(json.ends_with("(yes)\"}"));
        assert!(f.matches(&Fact("x:y", &[], false)));
    }

    #[test]
    fn empty_block_and_not_and_unknown() {
        assert!(Filter::parse("or(block())").matches(&Fact("a:b", &[], true)));
        assert!(!Filter::parse("or(block())").matches(&Fact("a:b", &[], false)));
        assert!(Filter::parse("not(item(a:b))").matches(&Fact("a:c", &[], false)));
        assert!(!Filter::parse("not(item(a:b))").matches(&Fact("a:b", &[], false)));
        let f = Filter::parse("max_stack(16)");
        assert_eq!(f, Filter::Unknown("max_stack(16)".into()));
        assert!(!f.matches(&Fact("a:b", &[], false)));
    }

    #[test]
    fn nothing_parses_to_an_empty_or() {
        assert_eq!(Filter::parse(""), Filter::Or(vec![]));
        assert!(!Filter::parse("").matches(&Fact("a:b", &[], false)));
    }
}
