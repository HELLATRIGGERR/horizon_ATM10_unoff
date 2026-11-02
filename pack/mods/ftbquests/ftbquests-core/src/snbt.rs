//! SNBT as FTB Library writes it: Mojang's stringified NBT with the commas
//! optional (a newline is enough), bare `true` and `false`, and keys that may
//! hold dots (`chapter.007B547630FF0478.title`).

use std::fmt;

/// One value of a file. Numbers keep their written kind; [`Value::as_f64`] and
/// [`Value::as_i64`] read any of them, as `CompoundTag.getDouble` does.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Bool(bool),
    Str(String),
    List(Vec<Value>),
    Compound(Compound),
    ByteArray(Vec<i8>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

/// A compound in file order. Quest files have a few dozen keys at most, so a
/// list stays smaller and faster than a map. The last of a repeated key wins,
/// as a `CompoundTag.put` does.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Compound(pub Vec<(String, Value)>);

impl Compound {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v)
    }
    pub fn str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }
    pub fn i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(Value::as_i64)
    }
    pub fn f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(Value::as_f64)
    }
    /// `CompoundTag.getBoolean`: a bool, or any number other than zero.
    pub fn bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(Value::as_bool)
    }
    pub fn compound(&self, key: &str) -> Option<&Compound> {
        self.get(key).and_then(Value::as_compound)
    }
    pub fn list(&self, key: &str) -> Option<&[Value]> {
        self.get(key).and_then(Value::as_list)
    }
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(k, _)| k.as_str())
    }
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        Some(match self {
            Value::Byte(n) => i64::from(*n),
            Value::Short(n) => i64::from(*n),
            Value::Int(n) => i64::from(*n),
            Value::Long(n) => *n,
            Value::Float(n) => *n as i64,
            Value::Double(n) => *n as i64,
            Value::Bool(b) => i64::from(*b),
            _ => return None,
        })
    }
    pub fn as_f64(&self) -> Option<f64> {
        Some(match self {
            Value::Float(n) => f64::from(*n),
            Value::Double(n) => *n,
            _ => self.as_i64()? as f64,
        })
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => self.as_i64().map(|n| n != 0),
        }
    }
    pub fn as_compound(&self) -> Option<&Compound> {
        match self {
            Value::Compound(c) => Some(c),
            _ => None,
        }
    }
    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(l) => Some(l),
            _ => None,
        }
    }
}

/// Where a file stopped making sense.
#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}, column {}: {}", self.line, self.column, self.message)
    }
}

impl std::error::Error for Error {}

/// Reads one value, normally the file's root compound.
pub fn parse(text: &str) -> Result<Value, Error> {
    let mut p = Parser { text: text.strip_prefix('\u{feff}').unwrap_or(text), at: 0 };
    p.skip_space();
    let value = p.value()?;
    p.skip_space();
    if p.at != p.text.len() {
        return Err(p.error("text after the value"));
    }
    Ok(value)
}

/// Reads a file's root, which must be a compound.
pub fn parse_compound(text: &str) -> Result<Compound, Error> {
    match parse(text)? {
        Value::Compound(c) => Ok(c),
        _ => Err(Error { line: 1, column: 1, message: "the root is not a compound".into() }),
    }
}

struct Parser<'a> {
    text: &'a str,
    at: usize,
}

impl Parser<'_> {
    fn error(&self, message: &str) -> Error {
        let before = &self.text[..self.at];
        let line = before.bytes().filter(|&b| b == b'\n').count() + 1;
        let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
        Error { line, column, message: message.to_owned() }
    }
    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.at).copied()
    }
    /// White space and the optional commas between entries.
    fn skip_separators(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n' | b',')) {
            self.at += 1;
        }
    }
    fn skip_space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.at += 1;
        }
    }
    fn value(&mut self) -> Result<Value, Error> {
        match self.peek() {
            Some(b'{') => self.compound().map(Value::Compound),
            Some(b'[') => self.list(),
            Some(q @ (b'"' | b'\'')) => self.quoted(q).map(Value::Str),
            Some(_) => {
                let word = self.bare()?;
                Ok(scalar(word))
            }
            None => Err(self.error("the file ended where a value should be")),
        }
    }
    fn compound(&mut self) -> Result<Compound, Error> {
        self.at += 1;
        let mut entries = Vec::new();
        loop {
            self.skip_separators();
            match self.peek() {
                Some(b'}') => {
                    self.at += 1;
                    return Ok(Compound(entries));
                }
                None => return Err(self.error("the file ended inside a compound")),
                _ => {}
            }
            let key = match self.peek() {
                Some(q @ (b'"' | b'\'')) => self.quoted(q)?,
                _ => self.bare()?.to_owned(),
            };
            self.skip_space();
            if self.peek() != Some(b':') {
                return Err(self.error("a key without ':'"));
            }
            self.at += 1;
            self.skip_space();
            let value = self.value()?;
            entries.push((key, value));
        }
    }
    fn list(&mut self) -> Result<Value, Error> {
        self.at += 1;
        self.skip_space();
        // `[I; 1, 2]`: a typed array.
        let kind = self.array_kind();
        let mut items = Vec::new();
        loop {
            self.skip_separators();
            match self.peek() {
                Some(b']') => {
                    self.at += 1;
                    break;
                }
                None => return Err(self.error("the file ended inside a list")),
                _ => items.push(self.value()?),
            }
        }
        let number = |v: &Value| v.as_i64().unwrap_or(0);
        Ok(match kind {
            Some(b'B') => Value::ByteArray(items.iter().map(|v| number(v) as i8).collect()),
            Some(b'I') => Value::IntArray(items.iter().map(|v| number(v) as i32).collect()),
            Some(b'L') => Value::LongArray(items.iter().map(number).collect()),
            _ => Value::List(items),
        })
    }
    fn array_kind(&mut self) -> Option<u8> {
        let bytes = self.text.as_bytes();
        let kind = *bytes.get(self.at)?;
        if matches!(kind, b'B' | b'I' | b'L') && bytes.get(self.at + 1) == Some(&b';') {
            self.at += 2;
            return Some(kind);
        }
        None
    }
    fn quoted(&mut self, quote: u8) -> Result<String, Error> {
        self.at += 1;
        let mut out = String::new();
        let bytes = self.text.as_bytes();
        let mut run = self.at;
        while let Some(&b) = bytes.get(self.at) {
            if b == quote {
                out.push_str(&self.text[run..self.at]);
                self.at += 1;
                return Ok(out);
            }
            if b == b'\\' {
                out.push_str(&self.text[run..self.at]);
                self.at += 1;
                let Some(&e) = bytes.get(self.at) else { break };
                self.at += 1;
                match e {
                    b'n' => out.push('\n'),
                    b't' => out.push('\t'),
                    b'r' => out.push('\r'),
                    b'b' => out.push('\u{8}'),
                    b'f' => out.push('\u{c}'),
                    b'u' => {
                        let hex = self.text.get(self.at..self.at + 4).ok_or_else(|| self.error("a short \\u escape"))?;
                        let code = u32::from_str_radix(hex, 16).map_err(|_| self.error("a bad \\u escape"))?;
                        out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                        self.at += 4;
                    }
                    // `\"`, `\\`, `\'`, and any other character as itself, so
                    // a line's `\&` or `\[` keeps its character.
                    _ => {
                        self.at -= 1;
                        let c = self.text[self.at..].chars().next().unwrap_or('\\');
                        out.push(c);
                        self.at += c.len_utf8();
                    }
                }
                run = self.at;
                continue;
            }
            self.at += 1;
        }
        Err(self.error("the file ended inside a string"))
    }
    /// A key or an unquoted value: up to the next structural character or white space.
    fn bare(&mut self) -> Result<&str, Error> {
        let start = self.at;
        while let Some(b) = self.peek() {
            if matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b',' | b':' | b'{' | b'}' | b'[' | b']' | b'"' | b'\'') {
                break;
            }
            self.at += 1;
        }
        if self.at == start {
            return Err(self.error("an unexpected character"));
        }
        Ok(&self.text[start..self.at])
    }
}

/// An unquoted word: a number with its suffix, a boolean, or a plain string.
fn scalar(word: &str) -> Value {
    match word {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }
    let (body, suffix) = match word.as_bytes().last() {
        Some(b'b' | b'B' | b's' | b'S' | b'l' | b'L' | b'f' | b'F' | b'd' | b'D') if word.len() > 1 => {
            word.split_at(word.len() - 1)
        }
        _ => (word, ""),
    };
    let numeric = body.bytes().next().is_some_and(|b| b.is_ascii_digit() || matches!(b, b'-' | b'+' | b'.'))
        && body.bytes().all(|b| b.is_ascii_digit() || matches!(b, b'-' | b'+' | b'.' | b'e' | b'E'));
    if numeric {
        let integer = body.parse::<i64>();
        let float = body.parse::<f64>();
        match suffix {
            "b" | "B" => {
                if let Ok(n) = integer {
                    return Value::Byte(n as i8);
                }
            }
            "s" | "S" => {
                if let Ok(n) = integer {
                    return Value::Short(n as i16);
                }
            }
            "l" | "L" => {
                if let Ok(n) = integer {
                    return Value::Long(n);
                }
            }
            "f" | "F" => {
                if let Ok(n) = float {
                    return Value::Float(n as f32);
                }
            }
            "d" | "D" => {
                if let Ok(n) = float {
                    return Value::Double(n);
                }
            }
            _ => {
                if let Ok(n) = integer {
                    return i32::try_from(n).map_or(Value::Long(n), Value::Int);
                }
                if let Ok(n) = float {
                    return Value::Double(n);
                }
            }
        }
    }
    Value::Str(word.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newlines_stand_in_for_commas() {
        let root = parse_compound("{\n\ta: 1\n\tb: 2.5d\n\tc: true,\n\td: \"x\"\n}\n").unwrap();
        assert_eq!(root.i64("a"), Some(1));
        assert_eq!(root.f64("b"), Some(2.5));
        assert_eq!(root.bool("c"), Some(true));
        assert_eq!(root.str("d"), Some("x"));
    }

    #[test]
    fn keys_may_hold_dots_and_numbers_keep_their_suffix() {
        let root = parse_compound("{ chapter.00FF.title: \"T\", n: 7L, s: 3s, b: 1b, f: 0.5f }").unwrap();
        assert_eq!(root.str("chapter.00FF.title"), Some("T"));
        assert_eq!(root.get("n"), Some(&Value::Long(7)));
        assert_eq!(root.get("s"), Some(&Value::Short(3)));
        assert_eq!(root.get("b"), Some(&Value::Byte(1)));
        assert_eq!(root.get("f"), Some(&Value::Float(0.5)));
        assert_eq!(root.bool("b"), Some(true));
    }

    #[test]
    fn lists_and_typed_arrays() {
        let root = parse_compound("{ a: [ ], b: [\n1\n2\n], c: [I; 1, 2], d: [L; 3L], e: [B; 1b, 0b], f: [{ x: 1 } { x: 2 }] }").unwrap();
        assert_eq!(root.list("a").unwrap().len(), 0);
        assert_eq!(root.list("b").unwrap().len(), 2);
        assert_eq!(root.get("c"), Some(&Value::IntArray(vec![1, 2])));
        assert_eq!(root.get("d"), Some(&Value::LongArray(vec![3])));
        assert_eq!(root.get("e"), Some(&Value::ByteArray(vec![1, 0])));
        assert_eq!(root.list("f").unwrap().len(), 2);
    }

    #[test]
    fn strings_keep_their_escapes_and_their_unicode() {
        let root = parse_compound(r#"{ a: "q\"uote\n\\", b: 'it\'s', c: "éé", d: "\&a" }"#).unwrap();
        assert_eq!(root.str("a"), Some("q\"uote\n\\"));
        assert_eq!(root.str("b"), Some("it's"));
        assert_eq!(root.str("c"), Some("éé"));
        assert_eq!(root.str("d"), Some("&a"));
    }

    #[test]
    fn bare_words_that_are_not_numbers_stay_strings() {
        let root = parse_compound("{ a: hello, b: 12ab, c: -5, d: 1e3 }").unwrap();
        assert_eq!(root.str("a"), Some("hello"));
        assert_eq!(root.str("b"), Some("12ab"));
        assert_eq!(root.i64("c"), Some(-5));
        assert_eq!(root.f64("d"), Some(1000.0));
    }

    #[test]
    fn errors_say_where() {
        let e = parse_compound("{\n a: 1\n b: \n").unwrap_err();
        assert_eq!(e.line, 4);
        assert!(parse("{ a: 1 } trailing").is_err());
        assert!(parse("{ a: \"open").is_err());
    }

    #[test]
    fn a_large_number_stays_a_long() {
        let root = parse_compound("{ a: 5000000000 }").unwrap();
        assert_eq!(root.get("a"), Some(&Value::Long(5_000_000_000)));
    }

    #[test]
    fn a_byte_order_mark_is_skipped() {
        assert!(parse_compound("\u{feff}{ a: 1 }").is_ok());
    }
}
