//! Writes KiCad S-expressions: a small tree builder and a printer that
//! formats the files the way KiCad does (one list per line where it helps
//! reading, short lists inline, tabs for indentation).

use std::fmt::Write as _;

use ew_model::units::Nm;

/// A node of an S-expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Sx {
    /// A bare word or number.
    Word(String),
    /// A quoted string.
    Text(String),
    /// `( … )`
    List(Vec<Self>),
}

/// Millimetres of a length in nanometres, as KiCad writes them (up to six
/// decimals, no trailing zeros).
#[must_use]
pub fn mm(value: Nm) -> String {
    number(value as f64 / 1e6)
}

/// A number with up to six decimals.
#[must_use]
pub fn number(value: f64) -> String {
    let text = format!("{value:.6}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    match text {
        "-0" | "" => "0".to_owned(),
        other => other.to_owned(),
    }
}

impl Sx {
    /// A list starting with `head`.
    #[must_use]
    pub fn list(head: &str) -> Self {
        Self::List(vec![Self::Word(head.to_owned())])
    }

    fn push(&mut self, node: Self) {
        if let Self::List(items) = self {
            items.push(node);
        }
    }

    /// Adds a bare word.
    #[must_use]
    pub fn word(mut self, word: impl Into<String>) -> Self {
        self.push(Self::Word(word.into()));
        self
    }

    /// Adds a quoted string.
    #[must_use]
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.push(Self::Text(text.into()));
        self
    }

    /// Adds a number.
    #[must_use]
    pub fn num(mut self, value: f64) -> Self {
        self.push(Self::Word(number(value)));
        self
    }

    /// Adds a length in millimetres.
    #[must_use]
    pub fn mm(mut self, value: Nm) -> Self {
        self.push(Self::Word(mm(value)));
        self
    }

    /// Adds a child list.
    #[must_use]
    pub fn child(mut self, child: Self) -> Self {
        self.push(child);
        self
    }

    /// Adds a child list if there is one.
    #[must_use]
    pub fn maybe(self, child: Option<Self>) -> Self {
        match child {
            Some(child) => self.child(child),
            None => self,
        }
    }

    /// Adds several child lists.
    #[must_use]
    pub fn children(mut self, children: impl IntoIterator<Item = Self>) -> Self {
        for child in children {
            self.push(child);
        }
        self
    }

    /// `(head value)` with a bare word.
    #[must_use]
    pub fn pair(head: &str, value: impl Into<String>) -> Self {
        Self::list(head).word(value)
    }

    /// `(head "value")`.
    #[must_use]
    pub fn named(head: &str, value: impl Into<String>) -> Self {
        Self::list(head).text(value)
    }

    /// `(head yes|no)`.
    #[must_use]
    pub fn flag(head: &str, value: bool) -> Self {
        Self::pair(head, if value { "yes" } else { "no" })
    }

    /// The text of the file (with a final line break).
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0);
        out.push('\n');
        out
    }

    fn inline_len(&self) -> Option<usize> {
        match self {
            Self::Word(w) => Some(w.len()),
            Self::Text(t) => Some(t.len() + 2),
            Self::List(items) => {
                let mut total = 2 + items.len();
                for item in items {
                    if let Self::List(inner) = item {
                        // Lists of lists are not written inline.
                        if inner.iter().any(|n| matches!(n, Self::List(_))) {
                            return None;
                        }
                    }
                    total += item.inline_len()?;
                }
                Some(total)
            }
        }
    }

    fn write(&self, out: &mut String, depth: usize) {
        match self {
            Self::Word(word) => out.push_str(word),
            Self::Text(text) => {
                out.push('"');
                for c in text.chars() {
                    match c {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        '\r' => {}
                        c => out.push(c),
                    }
                }
                out.push('"');
            }
            Self::List(items) => {
                if self.inline_len().is_some_and(|len| len + depth * 4 <= 100) {
                    out.push('(');
                    for (index, item) in items.iter().enumerate() {
                        if index > 0 {
                            out.push(' ');
                        }
                        item.write(out, depth + 1);
                    }
                    out.push(')');
                    return;
                }
                out.push('(');
                let mut first_list = true;
                for (index, item) in items.iter().enumerate() {
                    if matches!(item, Self::List(_)) {
                        out.push('\n');
                        for _ in 0..=depth {
                            out.push('\t');
                        }
                        first_list = false;
                    } else if index > 0 {
                        out.push(' ');
                    }
                    item.write(out, depth + 1);
                }
                if !first_list {
                    out.push('\n');
                    for _ in 0..depth {
                        out.push('\t');
                    }
                }
                out.push(')');
            }
        }
    }
}

/// `(at x y)` or `(at x y angle)` in millimetres. Schematics need the
/// angle wherever one is given, also when it is 0.
#[must_use]
pub fn at(x: Nm, y: Nm, angle: Option<f64>) -> Sx {
    let node = Sx::list("at").mm(x).mm(y);
    match angle {
        Some(a) => node.num(a),
        None => node,
    }
}

/// `(xy x y)`.
#[must_use]
pub fn xy(x: Nm, y: Nm) -> Sx {
    Sx::list("xy").mm(x).mm(y)
}

/// `(uuid "…")`.
#[must_use]
pub fn uuid(id: ew_model::id::Id) -> Sx {
    Sx::named("uuid", id.to_string())
}

/// `(stroke (width w) (type t))`.
#[must_use]
pub fn stroke(width: Nm, kind: &str) -> Sx {
    Sx::list("stroke")
        .child(Sx::list("width").mm(width))
        .child(Sx::pair("type", kind))
}

/// Writes `text` into `out` as one line per item (for tables).
pub fn lines(out: &mut String, items: &[String]) {
    for item in items {
        let _ = writeln!(out, "{item}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sexpr::parse;

    #[test]
    fn numbers_and_strings() {
        assert_eq!(mm(1_270_000), "1.27");
        assert_eq!(mm(-100), "-0.0001");
        assert_eq!(mm(0), "0");
        assert_eq!(number(-0.000_000_1), "0");
        let node = Sx::list("property")
            .text("Value")
            .text("say \"hi\"\\")
            .child(at(1_000_000, 2_500_000, Some(90.0)));
        let text = node.render();
        assert_eq!(
            text,
            "(property \"Value\" \"say \\\"hi\\\"\\\\\" (at 1 2.5 90))\n"
        );
        let parsed = parse(&text).unwrap();
        assert_eq!(parsed.arg(1), Some("say \"hi\"\\"));
    }

    #[test]
    fn long_lists_break_into_lines() {
        let node = Sx::list("kicad_pcb")
            .child(Sx::pair("version", "20240108"))
            .child(Sx::list("layers").child(Sx::list("0").text("F.Cu").word("signal")));
        let text = node.render();
        assert_eq!(
            text,
            "(kicad_pcb\n\t(version 20240108)\n\t(layers (0 \"F.Cu\" signal))\n)\n"
        );
        assert!(parse(&text).is_ok());
    }
}
