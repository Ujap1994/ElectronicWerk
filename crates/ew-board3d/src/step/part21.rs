//! Just enough of ISO 10303-21 (the STEP file format) to merge component
//! files into an assembly: the entity records of the `DATA` section, their
//! arguments, and renumbering of references.

use std::collections::{BTreeMap, BTreeSet, HashMap};

/// One entity record: `#id = BODY;`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Instance number.
    pub id: u64,
    /// Everything between `=` and `;`, trimmed.
    pub body: String,
}

impl Record {
    /// The entity type (`PRODUCT_DEFINITION`); complex entities
    /// (`( A() B() )`) give an empty name.
    #[must_use]
    pub fn kind(&self) -> &str {
        let end = self
            .body
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(self.body.len());
        &self.body[..end]
    }

    /// The top-level arguments of a simple entity.
    #[must_use]
    pub fn args(&self) -> Vec<&str> {
        let Some(open) = self.body.find('(') else {
            return Vec::new();
        };
        let Some(close) = self.body.rfind(')') else {
            return Vec::new();
        };
        split_args(&self.body[open + 1..close])
    }
}

/// Splits an argument list at its top-level commas.
#[must_use]
pub fn split_args(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0_i32;
    let mut in_string = false;
    let mut start = 0;
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if in_string {
            if c == b'\'' {
                if bytes.get(i + 1) == Some(&b'\'') {
                    i += 1;
                } else {
                    in_string = false;
                }
            }
        } else {
            match c {
                b'\'' => in_string = true,
                b'(' => depth += 1,
                b')' => depth -= 1,
                b',' if depth == 0 => {
                    out.push(text[start..i].trim());
                    start = i + 1;
                }
                _ => {}
            }
        }
        i += 1;
    }
    let last = text[start..].trim();
    if !last.is_empty() || !out.is_empty() {
        out.push(last);
    }
    out
}

/// The instance number of a reference argument (`#12`).
#[must_use]
pub fn reference(arg: &str) -> Option<u64> {
    arg.trim().strip_prefix('#')?.parse().ok()
}

/// Every reference in a text (outside strings).
#[must_use]
pub fn references(text: &str) -> Vec<u64> {
    let mut out = Vec::new();
    rewrite(text, &mut |id| {
        out.push(id);
        id
    });
    out
}

/// Copies a text, replacing every reference outside strings.
pub fn rewrite(text: &str, map: &mut dyn FnMut(u64) -> u64) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    let bytes = text.as_bytes();
    let mut in_string = false;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if in_string {
            out.push(c as char);
            if c == b'\'' {
                if bytes.get(i + 1) == Some(&b'\'') {
                    out.push('\'');
                    i += 1;
                } else {
                    in_string = false;
                }
            }
            i += 1;
            continue;
        }
        if c == b'\'' {
            in_string = true;
            out.push('\'');
            i += 1;
            continue;
        }
        if c == b'#' {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end > start
                && let Ok(id) = text[start..end].parse::<u64>()
            {
                out.push('#');
                out.push_str(&map(id).to_string());
                i = end;
                continue;
            }
        }
        // Copy one (possibly multi-byte) character.
        let ch = text[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// A parsed STEP file.
#[derive(Debug, Clone, Default)]
pub struct StepFile {
    /// The schema names of `FILE_SCHEMA`.
    pub schemas: Vec<String>,
    /// The records of the `DATA` section, in file order.
    pub records: Vec<Record>,
}

/// An error reading a STEP file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

/// Removes comments (`/* … */`) outside strings.
fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if c == '\'' {
                if chars.peek() == Some(&'\'') {
                    out.push('\'');
                    chars.next();
                } else {
                    in_string = false;
                }
            }
        } else if c == '\'' {
            in_string = true;
            out.push(c);
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut last = ' ';
            for d in chars.by_ref() {
                if last == '*' && d == '/' {
                    break;
                }
                last = d;
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Splits text into statements ending with `;` (outside strings).
fn statements(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut in_string = false;
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if in_string {
            if c == b'\'' {
                if bytes.get(i + 1) == Some(&b'\'') {
                    i += 1;
                } else {
                    in_string = false;
                }
            }
        } else if c == b'\'' {
            in_string = true;
        } else if c == b';' {
            out.push(text[start..i].trim());
            start = i + 1;
        }
        i += 1;
    }
    out
}

/// Reads a STEP file.
///
/// # Errors
///
/// Fails if the text is not an ISO 10303-21 file with a `DATA` section.
pub fn parse(text: &str) -> Result<StepFile, ParseError> {
    let clean = strip_comments(text);
    if !clean.trim_start().starts_with("ISO-10303-21") {
        return Err(ParseError("not a STEP file (ISO-10303-21)".into()));
    }
    let mut file = StepFile::default();
    let mut in_data = false;
    let mut found_data = false;
    for statement in statements(&clean) {
        let compact = statement.trim();
        if compact == "DATA" || compact.starts_with("DATA(") || compact.starts_with("DATA (") {
            in_data = true;
            found_data = true;
            continue;
        }
        if compact == "ENDSEC" {
            in_data = false;
            continue;
        }
        if !in_data {
            if let Some(rest) = compact.strip_prefix("FILE_SCHEMA") {
                for part in rest.split('\'').skip(1).step_by(2) {
                    file.schemas.push(part.trim().to_owned());
                }
            }
            continue;
        }
        let Some(rest) = compact.strip_prefix('#') else {
            continue;
        };
        let Some((id, body)) = rest.split_once('=') else {
            return Err(ParseError(format!("bad record `{compact}`")));
        };
        let id: u64 = id
            .trim()
            .parse()
            .map_err(|_| ParseError(format!("bad instance number in `{compact}`")))?;
        file.records.push(Record {
            id,
            body: body.trim().to_owned(),
        });
    }
    if !found_data {
        return Err(ParseError("the STEP file has no DATA section".into()));
    }
    Ok(file)
}

/// The shape of a root product of a STEP file: what the assembly links to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Root {
    /// The `PRODUCT_DEFINITION`.
    pub definition: u64,
    /// The shape representation of the product.
    pub representation: u64,
    /// The representation's geometric context.
    pub context: Option<u64>,
}

impl StepFile {
    /// The records by instance number.
    #[must_use]
    pub fn by_id(&self) -> HashMap<u64, &Record> {
        self.records.iter().map(|r| (r.id, r)).collect()
    }

    /// The products that are no other product's part, with their shapes.
    #[must_use]
    pub fn roots(&self) -> Vec<Root> {
        let by_id = self.by_id();
        let mut definitions: BTreeSet<u64> = BTreeSet::new();
        let mut children: BTreeSet<u64> = BTreeSet::new();
        // PRODUCT_DEFINITION_SHAPE → its definition.
        let mut shapes: BTreeMap<u64, u64> = BTreeMap::new();
        // definition shape → representation.
        let mut representations: BTreeMap<u64, u64> = BTreeMap::new();
        for record in &self.records {
            let kind = record.kind();
            let args = record.args();
            match kind {
                "PRODUCT_DEFINITION" | "PRODUCT_DEFINITION_WITH_ASSOCIATED_DOCUMENTS" => {
                    definitions.insert(record.id);
                }
                "NEXT_ASSEMBLY_USAGE_OCCURRENCE" | "ASSEMBLY_COMPONENT_USAGE" => {
                    if let Some(child) = args.get(4).and_then(|a| reference(a)) {
                        children.insert(child);
                    }
                }
                "PRODUCT_DEFINITION_SHAPE" => {
                    if let Some(def) = args.get(2).and_then(|a| reference(a)) {
                        shapes.insert(record.id, def);
                    }
                }
                "SHAPE_DEFINITION_REPRESENTATION" => {
                    if let (Some(shape), Some(rep)) = (
                        args.first().and_then(|a| reference(a)),
                        args.get(1).and_then(|a| reference(a)),
                    ) {
                        representations.insert(shape, rep);
                    }
                }
                _ => {}
            }
        }
        let mut roots = Vec::new();
        for definition in definitions.difference(&children) {
            let Some(representation) = shapes
                .iter()
                .filter(|(_, def)| *def == definition)
                .find_map(|(shape, _)| representations.get(shape).copied())
            else {
                continue;
            };
            let context = by_id
                .get(&representation)
                .and_then(|r| r.args().get(2).and_then(|a| reference(a)));
            roots.push(Root {
                definition: *definition,
                representation,
                context,
            });
        }
        roots
    }

    /// The largest instance number.
    #[must_use]
    pub fn max_id(&self) -> u64 {
        self.records.iter().map(|r| r.id).max().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "ISO-10303-21;
HEADER;
/* a comment with #99 and ; */
FILE_DESCRIPTION(('x'),'2;1');
FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));
ENDSEC;
DATA;
#1 = APPLICATION_CONTEXT('core data');
#3 = SHAPE_DEFINITION_REPRESENTATION(#4,#10);
#4 = PRODUCT_DEFINITION_SHAPE('','',#5);
#5 = PRODUCT_DEFINITION('design','',#6,#9);
#10 = ADVANCED_BREP_SHAPE_REPRESENTATION('it''s #7',(#11,#15),
  #20);
#20 = ( GEOMETRIC_REPRESENTATION_CONTEXT(3) REPRESENTATION_CONTEXT('a;b','c') );
ENDSEC;
END-ISO-10303-21;
";

    #[test]
    fn reads_records_and_finds_the_root() {
        let file = parse(SAMPLE).unwrap();
        assert_eq!(
            file.schemas,
            vec!["AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }"]
        );
        assert_eq!(file.records.len(), 6);
        assert_eq!(file.records[4].kind(), "ADVANCED_BREP_SHAPE_REPRESENTATION");
        assert_eq!(file.records[5].kind(), "");
        assert_eq!(
            file.records[4].args(),
            vec!["'it''s #7'", "(#11,#15)", "#20"]
        );
        let roots = file.roots();
        assert_eq!(
            roots,
            vec![Root {
                definition: 5,
                representation: 10,
                context: Some(20)
            }]
        );
        assert_eq!(file.max_id(), 20);
    }

    #[test]
    fn renumbers_references_but_not_strings() {
        let text = "FOO('a #1 b',#2,(#3,#40),'it''s #5')";
        let out = rewrite(text, &mut |id| id + 100);
        assert_eq!(out, "FOO('a #1 b',#102,(#103,#140),'it''s #5')");
        assert_eq!(references(text), vec![2, 3, 40]);
        assert!(parse("hello").is_err());
    }
}
