//! PDF objects and the tokenizer shared by files and content streams.

use std::collections::BTreeMap;
use std::rc::Rc;

/// A dictionary: names (without the slash) to objects.
pub type Dict = BTreeMap<Vec<u8>, Object>;

/// A stream: its dictionary and its data as stored in the file (still
/// filtered, already decrypted).
#[derive(Debug, Clone, PartialEq)]
pub struct Stream {
    /// Stream dictionary.
    pub dict: Dict,
    /// Raw (filtered) data.
    pub data: Rc<Vec<u8>>,
}

/// A PDF object.
#[derive(Debug, Clone, PartialEq)]
pub enum Object {
    /// `null`, and everything that cannot be read.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// An integer.
    Int(i64),
    /// A real number.
    Real(f64),
    /// A name, without the slash, `#xx` escapes resolved.
    Name(Vec<u8>),
    /// A literal or hexadecimal string.
    String(Vec<u8>),
    /// An array.
    Array(Vec<Object>),
    /// A dictionary.
    Dict(Dict),
    /// A stream.
    Stream(Stream),
    /// A reference to an indirect object.
    Ref(u32, u16),
}

impl Object {
    /// The number, if this is one.
    #[must_use]
    pub fn number(&self) -> Option<f64> {
        match self {
            Self::Int(v) => Some(*v as f64),
            Self::Real(v) => Some(*v),
            _ => None,
        }
    }

    /// The integer, if this is one (reals are truncated).
    #[must_use]
    pub fn int(&self) -> Option<i64> {
        match self {
            Self::Int(v) => Some(*v),
            Self::Real(v) if v.is_finite() => Some(*v as i64),
            _ => None,
        }
    }

    /// The name, if this is one.
    #[must_use]
    pub fn name(&self) -> Option<&[u8]> {
        match self {
            Self::Name(v) => Some(v),
            _ => None,
        }
    }

    /// The string bytes, if this is a string.
    #[must_use]
    pub fn string(&self) -> Option<&[u8]> {
        match self {
            Self::String(v) => Some(v),
            _ => None,
        }
    }

    /// The array, if this is one.
    #[must_use]
    pub fn array(&self) -> Option<&[Object]> {
        match self {
            Self::Array(v) => Some(v),
            _ => None,
        }
    }

    /// The dictionary of a dictionary or a stream.
    #[must_use]
    pub fn dict(&self) -> Option<&Dict> {
        match self {
            Self::Dict(d) => Some(d),
            Self::Stream(s) => Some(&s.dict),
            _ => None,
        }
    }

    /// The stream, if this is one.
    #[must_use]
    pub fn stream(&self) -> Option<&Stream> {
        match self {
            Self::Stream(s) => Some(s),
            _ => None,
        }
    }

    /// Numbers of an array of numbers (other entries count as zero).
    #[must_use]
    pub fn numbers(&self) -> Vec<f64> {
        self.array()
            .map(|a| a.iter().map(|o| o.number().unwrap_or(0.0)).collect())
            .unwrap_or_default()
    }
}

/// A token of the PDF syntax.
#[derive(Debug, Clone, PartialEq)]
pub enum Token<'a> {
    /// An integer.
    Int(i64),
    /// A real number.
    Real(f64),
    /// A name.
    Name(Vec<u8>),
    /// A string.
    String(Vec<u8>),
    /// `[`
    ArrayStart,
    /// `]`
    ArrayEnd,
    /// `<<`
    DictStart,
    /// `>>`
    DictEnd,
    /// `{` or `}` (PostScript calculator functions; skipped).
    Brace,
    /// Any other word: `obj`, `R`, `true`, an operator …
    Keyword(&'a [u8]),
}

/// Whitespace as the PDF syntax defines it.
#[must_use]
pub const fn is_white(b: u8) -> bool {
    matches!(b, b'\0' | b'\t' | b'\n' | b'\x0c' | b'\r' | b' ')
}

/// Delimiters as the PDF syntax defines them.
#[must_use]
pub const fn is_delimiter(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

const fn hex_value(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// A tokenizer over PDF bytes.
#[derive(Debug, Clone)]
pub struct Lexer<'a> {
    data: &'a [u8],
    /// Current position.
    pub pos: usize,
}

/// Nesting limit for arrays and dictionaries (broken files).
const MAX_DEPTH: usize = 64;

impl<'a> Lexer<'a> {
    /// A lexer at `pos`.
    #[must_use]
    pub const fn new(data: &'a [u8], pos: usize) -> Self {
        Self { data, pos }
    }

    /// The bytes being read.
    #[must_use]
    pub const fn data(&self) -> &'a [u8] {
        self.data
    }

    /// Skips whitespace and comments.
    pub fn skip_white(&mut self) {
        while let Some(&b) = self.data.get(self.pos) {
            if is_white(b) {
                self.pos += 1;
            } else if b == b'%' {
                while let Some(&c) = self.data.get(self.pos) {
                    if c == b'\n' || c == b'\r' {
                        break;
                    }
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    /// The next token, or `None` at the end.
    pub fn token(&mut self) -> Option<Token<'a>> {
        self.skip_white();
        let b = *self.data.get(self.pos)?;
        match b {
            b'[' => {
                self.pos += 1;
                Some(Token::ArrayStart)
            }
            b']' => {
                self.pos += 1;
                Some(Token::ArrayEnd)
            }
            b'{' | b'}' => {
                self.pos += 1;
                Some(Token::Brace)
            }
            b'<' => {
                if self.data.get(self.pos + 1) == Some(&b'<') {
                    self.pos += 2;
                    Some(Token::DictStart)
                } else {
                    self.pos += 1;
                    Some(Token::String(self.hex_string()))
                }
            }
            b'>' => {
                // `>>`, or a stray `>`.
                self.pos += 1;
                if self.data.get(self.pos) == Some(&b'>') {
                    self.pos += 1;
                }
                Some(Token::DictEnd)
            }
            b'(' => {
                self.pos += 1;
                Some(Token::String(self.literal_string()))
            }
            b'/' => {
                self.pos += 1;
                Some(Token::Name(self.name()))
            }
            b')' => {
                self.pos += 1;
                Some(Token::Keyword(b")"))
            }
            _ => {
                let start = self.pos;
                while let Some(&c) = self.data.get(self.pos) {
                    if is_white(c) || is_delimiter(c) {
                        break;
                    }
                    self.pos += 1;
                }
                let word = &self.data[start..self.pos];
                Some(number(word).unwrap_or(Token::Keyword(word)))
            }
        }
    }

    fn name(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        while let Some(&c) = self.data.get(self.pos) {
            if is_white(c) || is_delimiter(c) {
                break;
            }
            self.pos += 1;
            if c == b'#' {
                let hi = self.data.get(self.pos).copied().and_then(hex_value);
                let lo = self.data.get(self.pos + 1).copied().and_then(hex_value);
                if let (Some(hi), Some(lo)) = (hi, lo) {
                    out.push(hi << 4 | lo);
                    self.pos += 2;
                    continue;
                }
            }
            out.push(c);
        }
        out
    }

    fn hex_string(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut high: Option<u8> = None;
        while let Some(&c) = self.data.get(self.pos) {
            self.pos += 1;
            if c == b'>' {
                break;
            }
            if let Some(v) = hex_value(c) {
                if let Some(h) = high.take() {
                    out.push(h << 4 | v);
                } else {
                    high = Some(v);
                }
            }
        }
        if let Some(h) = high {
            out.push(h << 4);
        }
        out
    }

    fn literal_string(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut depth = 1usize;
        while let Some(&c) = self.data.get(self.pos) {
            self.pos += 1;
            match c {
                b'(' => {
                    depth += 1;
                    out.push(c);
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    out.push(c);
                }
                b'\\' => {
                    let Some(&e) = self.data.get(self.pos) else {
                        break;
                    };
                    self.pos += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'\r' => {
                            if self.data.get(self.pos) == Some(&b'\n') {
                                self.pos += 1;
                            }
                        }
                        b'\n' => {}
                        b'0'..=b'7' => {
                            let mut value = u32::from(e - b'0');
                            for _ in 0..2 {
                                match self.data.get(self.pos) {
                                    Some(&d @ b'0'..=b'7') => {
                                        value = value * 8 + u32::from(d - b'0');
                                        self.pos += 1;
                                    }
                                    _ => break,
                                }
                            }
                            out.push((value & 0xff) as u8);
                        }
                        other => out.push(other),
                    }
                }
                b'\r' => {
                    // End-of-line markers in strings read as `\n`.
                    if self.data.get(self.pos) == Some(&b'\n') {
                        self.pos += 1;
                    }
                    out.push(b'\n');
                }
                _ => out.push(c),
            }
        }
        out
    }

    /// Reads one object; `refs` reads `n g R` as a reference (files, not
    /// content streams). `None` at the end or at a keyword that is not an
    /// object (the lexer is then after that keyword).
    pub fn object(&mut self, refs: bool) -> Option<Object> {
        self.object_at(refs, 0)
    }

    fn object_at(&mut self, refs: bool, depth: usize) -> Option<Object> {
        let token = self.token()?;
        self.from_token(token, refs, depth)
    }

    /// Turns a token into an object, reading the rest of arrays and
    /// dictionaries.
    pub fn from_token(&mut self, token: Token<'a>, refs: bool, depth: usize) -> Option<Object> {
        match token {
            Token::Int(v) => {
                if refs && (0..=i64::from(u32::MAX)).contains(&v) {
                    let save = self.pos;
                    if let Some(Token::Int(g)) = self.token()
                        && (0..=65_535).contains(&g)
                        && let Some(Token::Keyword(b"R")) = self.token()
                    {
                        return Some(Object::Ref(v as u32, g as u16));
                    }
                    self.pos = save;
                }
                Some(Object::Int(v))
            }
            Token::Real(v) => Some(Object::Real(v)),
            Token::Name(n) => Some(Object::Name(n)),
            Token::String(s) => Some(Object::String(s)),
            Token::ArrayStart => {
                let mut items = Vec::new();
                if depth > MAX_DEPTH {
                    return Some(Object::Array(items));
                }
                loop {
                    let save = self.pos;
                    match self.token() {
                        None | Some(Token::ArrayEnd) => break,
                        Some(Token::DictEnd) => {
                            self.pos = save;
                            break;
                        }
                        Some(Token::Keyword(k)) if is_structure_keyword(k) => {
                            self.pos = save;
                            break;
                        }
                        Some(t) => {
                            if let Some(o) = self.from_token(t, refs, depth + 1) {
                                items.push(o);
                            }
                        }
                    }
                }
                Some(Object::Array(items))
            }
            Token::DictStart => {
                let mut dict = Dict::new();
                if depth > MAX_DEPTH {
                    return Some(Object::Dict(dict));
                }
                loop {
                    let save = self.pos;
                    match self.token() {
                        None | Some(Token::DictEnd) => break,
                        Some(Token::Name(key)) => {
                            let value_at = self.pos;
                            match self.token() {
                                Some(Token::DictEnd) | None => {
                                    dict.insert(key, Object::Null);
                                    break;
                                }
                                Some(Token::Keyword(k)) if is_structure_keyword(k) => {
                                    self.pos = value_at;
                                    break;
                                }
                                Some(t) => {
                                    let value =
                                        self.from_token(t, refs, depth + 1).unwrap_or(Object::Null);
                                    dict.insert(key, value);
                                }
                            }
                        }
                        Some(Token::Keyword(k)) if is_structure_keyword(k) => {
                            self.pos = save;
                            break;
                        }
                        Some(_) => {}
                    }
                }
                Some(Object::Dict(dict))
            }
            Token::Keyword(b"true") => Some(Object::Bool(true)),
            Token::Keyword(b"false") => Some(Object::Bool(false)),
            Token::Keyword(b"null") => Some(Object::Null),
            Token::ArrayEnd | Token::DictEnd | Token::Brace | Token::Keyword(_) => None,
        }
    }
}

/// Keywords that end an object in a broken file.
fn is_structure_keyword(word: &[u8]) -> bool {
    matches!(
        word,
        b"endobj" | b"stream" | b"endstream" | b"obj" | b"xref" | b"trailer" | b"startxref"
    )
}

/// Reads a number token; lenient about doubled signs and stray characters
/// the way viewers are.
fn number(word: &[u8]) -> Option<Token<'_>> {
    let first = *word.first()?;
    if !(first.is_ascii_digit() || matches!(first, b'+' | b'-' | b'.')) {
        return None;
    }
    let mut negative = false;
    let mut i = 0;
    while i < word.len() && matches!(word[i], b'+' | b'-') {
        negative ^= word[i] == b'-';
        i += 1;
    }
    let rest = &word[i..];
    if rest.is_empty() {
        // A lone sign reads as zero.
        return Some(Token::Int(0));
    }
    let dots = rest.split(|c| *c == b'.').count() - 1;
    if dots > 1 || !rest.iter().all(|c| c.is_ascii_digit() || *c == b'.') {
        return None;
    }
    let text = std::str::from_utf8(rest).ok()?;
    if text.contains('.') {
        let (int, frac) = text.split_once('.').unwrap_or((text, ""));
        let value: f64 = format!(
            "{}.{}",
            if int.is_empty() { "0" } else { int },
            if frac.is_empty() { "0" } else { frac }
        )
        .parse()
        .ok()?;
        Some(Token::Real(if negative { -value } else { value }))
    } else {
        let value: i64 = text.parse().unwrap_or(i64::MAX);
        Some(Token::Int(if negative { -value } else { value }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Option<Object> {
        Lexer::new(text.as_bytes(), 0).object(true)
    }

    #[test]
    fn reads_simple_objects() {
        assert_eq!(read("42"), Some(Object::Int(42)));
        assert_eq!(read("-.5"), Some(Object::Real(-0.5)));
        assert_eq!(read("4."), Some(Object::Real(4.0)));
        assert_eq!(read("--3"), Some(Object::Int(3)));
        assert_eq!(read("/A#20B"), Some(Object::Name(b"A B".to_vec())));
        assert_eq!(read("true"), Some(Object::Bool(true)));
        assert_eq!(read("12 0 R"), Some(Object::Ref(12, 0)));
        assert_eq!(
            read("<48656C6C6F>"),
            Some(Object::String(b"Hello".to_vec()))
        );
        assert_eq!(read("<414>"), Some(Object::String(b"A@".to_vec())));
    }

    #[test]
    fn reads_literal_strings_with_escapes() {
        assert_eq!(
            read(r"(a\(b\)c (nested) \101\n\\)"),
            Some(Object::String(b"a(b)c (nested) A\n\\".to_vec()))
        );
        assert_eq!(
            read("(line\\\r\ncontinued)"),
            Some(Object::String(b"linecontinued".to_vec()))
        );
    }

    #[test]
    fn reads_nested_structures() {
        let object =
            read("<< /Type /Page /Kids [1 0 R 2 0 R] /Box [0 0 612.5 792] /D << /X null >> >>");
        let Some(Object::Dict(d)) = object else {
            panic!("not a dict");
        };
        assert_eq!(
            d.get(b"Type".as_slice()),
            Some(&Object::Name(b"Page".to_vec()))
        );
        assert_eq!(
            d.get(b"Kids".as_slice()),
            Some(&Object::Array(vec![Object::Ref(1, 0), Object::Ref(2, 0)]))
        );
        assert_eq!(
            d.get(b"Box".as_slice()).map(Object::numbers),
            Some(vec![0.0, 0.0, 612.5, 792.0])
        );
    }

    #[test]
    fn stops_broken_dictionaries_at_structure_keywords() {
        let mut lexer = Lexer::new(b"<< /Length 5 stream\nhello", 0);
        let Some(Object::Dict(d)) = lexer.object(true) else {
            panic!("not a dict");
        };
        assert_eq!(d.get(b"Length".as_slice()), Some(&Object::Int(5)));
        assert_eq!(lexer.token(), Some(Token::Keyword(b"stream")));
    }

    #[test]
    fn content_streams_keep_numbers_apart() {
        let mut lexer = Lexer::new(b"1 0 0 1 72 720 cm", 0);
        let mut values = Vec::new();
        while let Some(o) = lexer.object(false) {
            values.push(o);
        }
        assert_eq!(values.len(), 6);
    }
}
