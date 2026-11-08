//! A fast S-expression parser for KiCad files.
//!
//! The parser makes one pass over the text without recursion and borrows
//! every atom and every string without escapes from the input, so a 13 MB
//! board parses in well under a second.

use std::borrow::Cow;
use std::fmt;

/// A node: a list, a bare atom (symbol or number) or a quoted string.
#[derive(Debug, Clone, PartialEq)]
pub enum Sexp<'a> {
    /// `( … )`
    List(Vec<Sexp<'a>>),
    /// A bare word or number.
    Atom(&'a str),
    /// A quoted string (escapes resolved).
    Str(Cow<'a, str>),
}

/// A syntax error with its position.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("line {line}, column {column}: {message}")]
pub struct ParseError {
    /// 1-based line.
    pub line: usize,
    /// 1-based column (in bytes).
    pub column: usize,
    /// What went wrong.
    pub message: String,
}

#[allow(clippy::naive_bytecount, reason = "only used to build error messages")]
fn position(text: &str, offset: usize) -> (usize, usize) {
    let before = &text.as_bytes()[..offset.min(text.len())];
    let line = before.iter().filter(|b| **b == b'\n').count() + 1;
    let column = offset
        - before
            .iter()
            .rposition(|b| *b == b'\n')
            .map_or(0, |p| p + 1)
        + 1;
    (line, column)
}

fn error(text: &str, offset: usize, message: impl Into<String>) -> ParseError {
    let (line, column) = position(text, offset);
    ParseError {
        line,
        column,
        message: message.into(),
    }
}

/// Parses one top-level expression (trailing whitespace allowed).
///
/// # Errors
///
/// Fails on unbalanced parentheses, unterminated strings or extra content.
pub fn parse(text: &str) -> Result<Sexp<'_>, ParseError> {
    let bytes = text.as_bytes();
    let mut stack: Vec<Vec<Sexp<'_>>> = Vec::new();
    let mut root: Option<Sexp<'_>> = None;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b' ' | b'\t' | b'\r' | b'\n' => i += 1,
            b'(' => {
                if root.is_some() {
                    return Err(error(text, i, "content after the end of the file"));
                }
                stack.push(Vec::new());
                i += 1;
            }
            b')' => {
                let Some(list) = stack.pop() else {
                    return Err(error(text, i, "unexpected `)`"));
                };
                let node = Sexp::List(list);
                match stack.last_mut() {
                    Some(parent) => parent.push(node),
                    None => root = Some(node),
                }
                i += 1;
            }
            b'"' => {
                let start = i + 1;
                let mut j = start;
                let mut escaped = false;
                while j < bytes.len() && bytes[j] != b'"' {
                    if bytes[j] == b'\\' {
                        escaped = true;
                        j += 1;
                    }
                    j += 1;
                }
                if j >= bytes.len() {
                    return Err(error(text, i, "unterminated string"));
                }
                let raw = &text[start..j];
                let value = if escaped {
                    Cow::Owned(unescape(raw))
                } else {
                    Cow::Borrowed(raw)
                };
                let Some(parent) = stack.last_mut() else {
                    return Err(error(text, i, "a string outside of a list"));
                };
                parent.push(Sexp::Str(value));
                i = j + 1;
            }
            _ => {
                let start = i;
                while i < bytes.len()
                    && !matches!(bytes[i], b' ' | b'\t' | b'\r' | b'\n' | b'(' | b')' | b'"')
                {
                    i += 1;
                }
                let Some(parent) = stack.last_mut() else {
                    return Err(error(text, start, "an atom outside of a list"));
                };
                parent.push(Sexp::Atom(&text[start..i]));
            }
        }
    }
    if !stack.is_empty() {
        return Err(error(
            text,
            text.len(),
            "missing `)` at the end of the file",
        ));
    }
    root.ok_or_else(|| error(text, 0, "the file is empty"))
}

fn unescape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

impl<'a> Sexp<'a> {
    /// The text of an atom or string.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Atom(text) => Some(text),
            Self::Str(text) => Some(text),
            Self::List(_) => None,
        }
    }

    /// The elements of a list (empty for atoms).
    #[must_use]
    pub fn items(&self) -> &[Sexp<'a>] {
        match self {
            Self::List(items) => items,
            _ => &[],
        }
    }

    /// The first element of a list if it is an atom (`segment` in
    /// `(segment …)`).
    #[must_use]
    pub fn head(&self) -> Option<&str> {
        match self.items().first() {
            Some(Self::Atom(head)) => Some(head),
            _ => None,
        }
    }

    /// `true` if this is a list starting with `name`.
    #[must_use]
    pub fn is(&self, name: &str) -> bool {
        self.head() == Some(name)
    }

    /// The elements after the head.
    #[must_use]
    pub fn args(&self) -> &[Sexp<'a>] {
        self.items().get(1..).unwrap_or(&[])
    }

    /// The text of argument `index` (0 = first after the head).
    #[must_use]
    pub fn arg(&self, index: usize) -> Option<&str> {
        self.args().get(index).and_then(Self::text)
    }

    /// Argument `index` as a number.
    #[must_use]
    pub fn num(&self, index: usize) -> Option<f64> {
        self.arg(index).and_then(|text| text.parse().ok())
    }

    /// Child lists with the given head.
    pub fn all<'s>(&'s self, name: &'s str) -> impl Iterator<Item = &'s Sexp<'a>> + 's {
        self.args().iter().filter(move |item| item.is(name))
    }

    /// The first child list with the given head.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Sexp<'a>> {
        self.args().iter().find(|item| item.is(name))
    }

    /// The first argument of the child `name`: `(width 0.25)` → `"0.25"`.
    #[must_use]
    pub fn value(&self, name: &str) -> Option<&str> {
        self.get(name).and_then(|child| child.arg(0))
    }

    /// The first argument of the child `name` as a number.
    #[must_use]
    pub fn number(&self, name: &str) -> Option<f64> {
        self.get(name).and_then(|child| child.num(0))
    }

    /// A boolean flag in either form: a bare word (`hide`, KiCad 6/7) or a
    /// child list (`(hide yes)`, KiCad 8 and later).
    #[must_use]
    pub fn flag(&self, name: &str) -> bool {
        self.args().iter().any(|item| match item {
            Self::Atom(word) => *word == name,
            Self::List(_) => item.is(name) && !matches!(item.arg(0), Some("no" | "false")),
            Self::Str(_) => false,
        })
    }

    /// `true` if any argument is the bare word `word`.
    #[must_use]
    pub fn has_word(&self, word: &str) -> bool {
        self.args()
            .iter()
            .any(|item| matches!(item, Self::Atom(text) if *text == word))
    }
}

impl fmt::Display for Sexp<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Atom(text) => f.write_str(text),
            Self::Str(text) => write!(f, "{text:?}"),
            Self::List(items) => {
                f.write_str("(")?;
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        f.write_str(" ")?;
                    }
                    fmt::Display::fmt(item, f)?;
                }
                f.write_str(")")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_lists_atoms_and_strings() {
        let tree = parse("(kicad_pcb (version 20260206) (net \"GND\") (at 1.5 -2 90))\n").unwrap();
        assert!(tree.is("kicad_pcb"));
        assert_eq!(tree.value("version"), Some("20260206"));
        assert_eq!(tree.value("net"), Some("GND"));
        let at = tree.get("at").unwrap();
        assert_eq!(
            (at.num(0), at.num(1), at.num(2)),
            (Some(1.5), Some(-2.0), Some(90.0))
        );
    }

    #[test]
    fn resolves_escapes_and_keeps_utf8() {
        let tree = parse(r#"(text "a \"b\"\nÜbersicht – 1\\2")"#).unwrap();
        assert_eq!(tree.arg(0), Some("a \"b\"\nÜbersicht – 1\\2"));
        assert!(matches!(&tree.args()[0], Sexp::Str(Cow::Owned(_))));
        let plain = parse(r#"(text "plain")"#).unwrap();
        assert!(matches!(&plain.args()[0], Sexp::Str(Cow::Borrowed(_))));
    }

    #[test]
    fn reads_flags_in_both_styles() {
        let old = parse("(property (effects hide))").unwrap();
        assert!(old.get("effects").unwrap().flag("hide"));
        let new = parse("(property (hide yes) (locked no))").unwrap();
        assert!(new.flag("hide"));
        assert!(!new.flag("locked"));
        assert!(!new.flag("missing"));
        assert!(parse("(pad smd rect)").unwrap().has_word("smd"));
    }

    #[test]
    fn reports_errors_with_positions() {
        let error = parse("(a\n  (b \"x)").unwrap_err();
        assert_eq!(error.line, 2);
        assert!(error.message.contains("unterminated"));
        assert!(parse("(a))").unwrap_err().message.contains("unexpected"));
        assert!(parse("(a").unwrap_err().message.contains("missing"));
        assert!(parse("  ").unwrap_err().message.contains("empty"));
        assert!(
            parse("(a) (b)")
                .unwrap_err()
                .message
                .contains("after the end")
        );
    }

    #[test]
    fn displays_round_trip() {
        let text = "(a b \"c d\" (e 1))";
        assert_eq!(parse(text).unwrap().to_string(), text);
    }
}
