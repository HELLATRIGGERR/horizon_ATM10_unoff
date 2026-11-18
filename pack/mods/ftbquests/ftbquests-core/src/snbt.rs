//! FTB Library's SNBT files: one entry a line, tabs, no commas, as `SNBT.write` lays them out, and a reader that
//! also takes vanilla's commas.

use std::{fs, io, path::Path};

/// An SNBT value. A compound keeps its keys in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(String),
    List(Vec<Value>),
    Compound(Compound),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Compound(pub Vec<(String, Value)>);

impl Compound {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
    pub fn put(&mut self, key: &str, value: Value) {
        match self.0.iter_mut().find(|(k, _)| k == key) {
            Some((_, v)) => *v = value,
            None => self.0.push((key.to_owned(), value)),
        }
    }
    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
    pub fn compound(&self, key: &str) -> Option<&Compound> {
        match self.get(key)? {
            Value::Compound(c) => Some(c),
            _ => None,
        }
    }
    pub fn list(&self, key: &str) -> &[Value] {
        match self.get(key) {
            Some(Value::List(l)) => l,
            _ => &[],
        }
    }
}

impl Value {
    /// The number, whatever its width (a boolean is 0 or 1).
    pub fn as_i64(&self) -> Option<i64> {
        Some(match self {
            Value::Bool(b) => *b as i64,
            Value::Byte(v) => *v as i64,
            Value::Short(v) => *v as i64,
            Value::Int(v) => *v as i64,
            Value::Long(v) => *v,
            Value::Float(v) => *v as i64,
            Value::Double(v) => *v as i64,
            _ => return None,
        })
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
}

fn simple_key(key: &str) -> bool {
    !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '+'))
}

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

fn write_value(out: &mut String, value: &Value, depth: usize) {
    let indent = |out: &mut String, n: usize| out.extend(std::iter::repeat_n('\t', n));
    match value {
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Byte(v) => out.push_str(&format!("{v}b")),
        Value::Short(v) => out.push_str(&format!("{v}s")),
        Value::Int(v) => out.push_str(&v.to_string()),
        Value::Long(v) => out.push_str(&format!("{v}L")),
        Value::Float(v) => out.push_str(&format!("{v:?}f")),
        Value::Double(v) => out.push_str(&format!("{v:?}d")),
        Value::String(s) => out.push_str(&quote(s)),
        Value::List(items) if items.is_empty() => out.push_str("[ ]"),
        Value::List(items) => {
            out.push_str("[\n");
            for item in items {
                indent(out, depth + 1);
                write_value(out, item, depth + 1);
                out.push('\n');
            }
            indent(out, depth);
            out.push(']');
        }
        Value::Compound(c) if c.0.is_empty() => out.push_str("{ }"),
        Value::Compound(c) => {
            out.push_str("{\n");
            for (k, v) in &c.0 {
                indent(out, depth + 1);
                out.push_str(&if simple_key(k) { k.clone() } else { quote(k) });
                out.push_str(": ");
                write_value(out, v, depth + 1);
                out.push('\n');
            }
            indent(out, depth);
            out.push('}');
        }
    }
}

/// The text `SNBT.write` puts in a file.
pub fn write_string(root: &Compound) -> String {
    let mut out = String::new();
    write_value(&mut out, &Value::Compound(root.clone()), 0);
    out.push('\n');
    out
}

/// Writes `root` to `path` as FTB Library does: into `<name>.tmp`, then moved over the file.
pub fn write_file(path: &Path, root: &Compound) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("snbt.tmp");
    fs::write(&tmp, write_string(root))?;
    fs::rename(&tmp, path)
}

pub fn read_file(path: &Path) -> Option<Compound> {
    let text = fs::read_to_string(path).ok()?;
    match parse(&text) {
        Ok(Value::Compound(c)) => Some(c),
        _ => None,
    }
}

struct Reader<'a> {
    s: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn skip(&mut self) {
        while self.at < self.s.len() {
            match self.s[self.at] {
                b' ' | b'\t' | b'\r' | b'\n' | b',' => self.at += 1,
                // `//` comments, which FTB Library's config files carry.
                b'/' if self.s.get(self.at + 1) == Some(&b'/') => {
                    while self.at < self.s.len() && self.s[self.at] != b'\n' {
                        self.at += 1;
                    }
                }
                b'#' => {
                    while self.at < self.s.len() && self.s[self.at] != b'\n' {
                        self.at += 1;
                    }
                }
                _ => return,
            }
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.skip();
        self.s.get(self.at).copied()
    }
    fn quoted(&mut self) -> Result<String, String> {
        let q = self.s[self.at];
        self.at += 1;
        let mut out = Vec::new();
        while self.at < self.s.len() {
            let c = self.s[self.at];
            self.at += 1;
            match c {
                b'\\' => {
                    let e = *self.s.get(self.at).ok_or("an escape at the end")?;
                    self.at += 1;
                    out.push(match e {
                        b'n' => b'\n',
                        b't' => b'\t',
                        other => other,
                    });
                }
                c if c == q => return String::from_utf8(out).map_err(|e| e.to_string()),
                c => out.push(c),
            }
        }
        Err("a string without its end".into())
    }
    fn bare(&mut self) -> String {
        let start = self.at;
        while self.at < self.s.len()
            && (self.s[self.at].is_ascii_alphanumeric() || matches!(self.s[self.at], b'_' | b'-' | b'.' | b'+' | b':'))
        {
            self.at += 1;
        }
        String::from_utf8_lossy(&self.s[start..self.at]).into_owned()
    }
    fn key(&mut self) -> Result<String, String> {
        let key = match self.peek() {
            Some(b'"' | b'\'') => self.quoted()?,
            _ => {
                let start = self.at;
                while self.at < self.s.len() && !matches!(self.s[self.at], b':' | b' ' | b'\t' | b'\n' | b'\r') {
                    self.at += 1;
                }
                String::from_utf8_lossy(&self.s[start..self.at]).into_owned()
            }
        };
        if self.peek() != Some(b':') {
            return Err(format!("no ':' after the key {key:?}"));
        }
        self.at += 1;
        Ok(key)
    }
    fn value(&mut self) -> Result<Value, String> {
        match self.peek().ok_or("a value missing")? {
            b'{' => {
                self.at += 1;
                let mut c = Compound::default();
                loop {
                    if self.peek() == Some(b'}') {
                        self.at += 1;
                        return Ok(Value::Compound(c));
                    }
                    let k = self.key()?;
                    let v = self.value()?;
                    c.0.push((k, v));
                }
            }
            b'[' => {
                self.at += 1;
                // Typed arrays: `[I; 1, 2]`, `[B; ...]`, `[L; ...]`.
                if matches!(self.s.get(self.at..self.at + 2), Some([b'I' | b'B' | b'L', b';'])) {
                    self.at += 2;
                }
                let mut items = Vec::new();
                loop {
                    if self.peek() == Some(b']') {
                        self.at += 1;
                        return Ok(Value::List(items));
                    }
                    items.push(self.value()?);
                }
            }
            b'"' | b'\'' => Ok(Value::String(self.quoted()?)),
            _ => Ok(number(&self.bare())),
        }
    }
}

fn number(word: &str) -> Value {
    let (body, suffix) = match word.chars().last() {
        Some(c) if c.is_ascii_alphabetic() && word.len() > 1 => (&word[..word.len() - 1], c.to_ascii_lowercase()),
        _ => (word, ' '),
    };
    let parsed = match suffix {
        'b' => body.parse().ok().map(Value::Byte),
        's' => body.parse().ok().map(Value::Short),
        'l' => body.parse().ok().map(Value::Long),
        'f' => body.parse().ok().map(Value::Float),
        'd' => body.parse().ok().map(Value::Double),
        _ if word.contains('.') => word.parse().ok().map(Value::Double),
        _ => word.parse().ok().map(Value::Int),
    };
    match (word, parsed) {
        ("true", _) => Value::Bool(true),
        ("false", _) => Value::Bool(false),
        (_, Some(v)) => v,
        _ => Value::String(word.to_owned()),
    }
}

pub fn parse(text: &str) -> Result<Value, String> {
    let mut r = Reader { s: text.as_bytes(), at: 0 };
    r.value()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_ftb_style_and_reads_it_back() {
        let mut inner = Compound::default();
        inner.put("ftbteams:display_name", Value::String("A \"b\"".into()));
        inner.put("ftbteams:free_to_join", Value::Bool(false));
        let mut root = Compound::default();
        root.put("id", Value::String("x".into()));
        root.put("properties", Value::Compound(inner));
        root.put("date", Value::Long(5));
        root.put("list", Value::List(vec![]));
        let text = write_string(&root);
        assert_eq!(
            text,
            "{\n\tid: \"x\"\n\tproperties: {\n\t\t\"ftbteams:display_name\": \"A \\\"b\\\"\"\n\t\t\"ftbteams:free_to_join\": false\n\t}\n\tdate: 5L\n\tlist: [ ]\n}\n"
        );
        assert_eq!(parse(&text), Ok(Value::Compound(root)));
    }

    #[test]
    fn reads_vanilla_commas_and_comments() {
        let v = parse("# a comment\n{a: 1b, b: [I; 1, 2], c: 'q', d: 2.5}").unwrap();
        let Value::Compound(c) = v else { panic!() };
        assert_eq!(c.get("a"), Some(&Value::Byte(1)));
        assert_eq!(c.list("b"), &[Value::Int(1), Value::Int(2)]);
        assert_eq!(c.str("c"), Some("q"));
        assert_eq!(c.get("d"), Some(&Value::Double(2.5)));
    }
}
