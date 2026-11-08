//! Deterministic, diff-friendly JSON layout.
//!
//! [`layout`] turns compact JSON into the layout used for every project file:
//!
//! - Two-space indentation and `\n` line endings.
//! - A container that fits into [`LINE_WIDTH`] characters stays on one line.
//! - An object inside an array (a "record", for example one track) stays on
//!   one line as long as none of its values is itself too long, so editing
//!   one object changes exactly one line in a diff.
//! - Everything else is expanded with one member or element per line.
//!
//! The output is a pure function of the input, so saving the same data
//! always produces the same bytes.

/// Target line width.
pub const LINE_WIDTH: usize = 100;

#[derive(Debug, Clone, Copy)]
struct Node {
    /// Index of the closing bracket.
    end: usize,
    /// Length when written on one line (with `": "` and `", "`).
    inline_len: usize,
    /// Number of containers nested inside (to skip them when inlining).
    descendants: usize,
    /// `true` while no direct child container is longer than the width.
    children_small: bool,
}

/// Lays out compact JSON (as produced by `serde_json::to_string`).
///
/// The input must be valid JSON; it is not re-validated.
#[must_use]
pub fn layout(compact: &str) -> String {
    let bytes = compact.as_bytes();
    let nodes = scan(bytes);
    let mut emitter = Emitter {
        src: bytes,
        nodes: &nodes,
        next: 0,
        out: Vec::with_capacity(compact.len() + compact.len() / 4),
    };
    let start = skip_ws(bytes, 0);
    if start < bytes.len() {
        emitter.value(start, 0, false, true);
    }
    emitter.out.push(b'\n');
    // Only ASCII bytes are inserted between complete UTF-8 sequences of the
    // input, so the output is valid UTF-8.
    String::from_utf8(emitter.out).unwrap_or_default()
}

fn skip_ws(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
        pos += 1;
    }
    pos
}

/// First pass: measures every container.
fn scan(bytes: &[u8]) -> Vec<Node> {
    let mut nodes: Vec<Node> = Vec::new();
    // (node index, separator count at the opening bracket)
    let mut stack: Vec<(usize, usize)> = Vec::new();
    let mut separators = 0_usize;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                index = string_end(bytes, index);
                continue;
            }
            b':' | b',' => separators += 1,
            b'{' | b'[' => {
                stack.push((nodes.len(), separators));
                nodes.push(Node {
                    end: index,
                    inline_len: 0,
                    descendants: 0,
                    children_small: true,
                });
            }
            b'}' | b']' => {
                if let Some((node, open_separators)) = stack.pop() {
                    let start = nodes[node].end;
                    let inline_len = index - start + 1 + (separators - open_separators);
                    let descendants = nodes.len() - node - 1;
                    let entry = &mut nodes[node];
                    entry.end = index;
                    entry.inline_len = inline_len;
                    entry.descendants = descendants;
                    if inline_len > LINE_WIDTH
                        && let Some(&(parent, _)) = stack.last()
                    {
                        nodes[parent].children_small = false;
                    }
                }
            }
            _ => {}
        }
        index += 1;
    }
    nodes
}

struct Emitter<'a> {
    src: &'a [u8],
    nodes: &'a [Node],
    next: usize,
    out: Vec<u8>,
}

impl Emitter<'_> {
    /// Writes the value starting at `pos` and returns the position after it.
    fn value(&mut self, pos: usize, indent: usize, in_array: bool, top: bool) -> usize {
        match self.src[pos] {
            open @ (b'{' | b'[') => {
                let node = self.nodes[self.next];
                self.next += 1;
                let is_object = open == b'{';
                let fits = node.inline_len + indent <= LINE_WIDTH;
                let record = is_object && in_array && node.children_small;
                let empty = node.end == pos + 1;
                if empty || (!top && (fits || record)) {
                    self.next += node.descendants;
                    self.inline(pos, node.end);
                    return node.end + 1;
                }
                self.expanded(pos, indent, is_object)
            }
            b'"' => {
                let end = string_end(self.src, pos);
                self.out.extend_from_slice(&self.src[pos..end]);
                end
            }
            _ => {
                let mut end = pos;
                while end < self.src.len() && !matches!(self.src[end], b',' | b']' | b'}') {
                    end += 1;
                }
                self.out.extend_from_slice(&self.src[pos..end]);
                end
            }
        }
    }

    fn expanded(&mut self, pos: usize, indent: usize, is_object: bool) -> usize {
        let (open, close) = if is_object {
            (b'{', b'}')
        } else {
            (b'[', b']')
        };
        self.out.push(open);
        let inner = indent + 2;
        let mut cursor = pos + 1;
        loop {
            self.out.push(b'\n');
            push_indent(&mut self.out, inner);
            if is_object {
                let key_end = string_end(self.src, cursor);
                self.out.extend_from_slice(&self.src[cursor..key_end]);
                self.out.extend_from_slice(b": ");
                // Skip the colon.
                cursor = key_end + 1;
            }
            cursor = self.value(cursor, inner, !is_object, false);
            if self.src[cursor] == b',' {
                self.out.push(b',');
                cursor += 1;
            } else {
                self.out.push(b'\n');
                push_indent(&mut self.out, indent);
                self.out.push(close);
                return cursor + 1;
            }
        }
    }

    /// Copies `start..=end` and adds a space after `:` and `,`.
    fn inline(&mut self, start: usize, end: usize) {
        let src = self.src;
        let mut segment = start;
        let mut index = start;
        while index <= end {
            match src[index] {
                b'"' => index = string_end(src, index),
                b':' | b',' => {
                    self.out.extend_from_slice(&src[segment..=index]);
                    self.out.push(b' ');
                    index += 1;
                    segment = index;
                }
                _ => index += 1,
            }
        }
        self.out.extend_from_slice(&src[segment..=end]);
    }
}

/// Position after the string starting at `pos` (which is a `"`).
fn string_end(bytes: &[u8], pos: usize) -> usize {
    let mut index = pos + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return index + 1,
            _ => index += 1,
        }
    }
    bytes.len()
}

fn push_indent(out: &mut Vec<u8>, indent: usize) {
    out.resize(out.len() + indent, b' ');
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json::{Value, json};

    fn lay(value: &Value) -> String {
        layout(&serde_json::to_string(value).unwrap())
    }

    #[test]
    fn small_documents() {
        assert_eq!(lay(&json!({})), "{}\n");
        assert_eq!(lay(&json!({"a": 1})), "{\n  \"a\": 1\n}\n");
        assert_eq!(lay(&json!([])), "[]\n");
    }

    #[test]
    fn short_containers_stay_inline() {
        let value = json!({"name": "Demo", "point": [1, 2], "style": {"size": 5, "bold": true}});
        assert_eq!(
            lay(&value),
            "{\n  \"name\": \"Demo\",\n  \"point\": [1, 2],\n  \"style\": {\"bold\": true, \"size\": 5}\n}\n"
        );
    }

    #[test]
    fn records_in_arrays_get_one_line_each() {
        let track = json!({
            "id": "0b0e8c1a-1111-4222-8333-444455556666",
            "layer": "cu.top",
            "net": "1b0e8c1a-1111-4222-8333-444455556666",
            "width": 250_000,
            "start": [123_456_789, 98_765_432],
            "end": [123_456_789, 99_765_432]
        });
        let value = json!({"tracks": [track.clone(), track]});
        let text = lay(&value);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 6, "{text}");
        // `json!` sorts keys, so the record starts with `end`.
        assert!(lines[2].starts_with("    {\"end\": "), "{text}");
        assert!(lines[2].len() > LINE_WIDTH);
    }

    #[test]
    fn long_members_expand() {
        let long = "x".repeat(120);
        let value = json!({"items": [{"a": long, "nested": {"b": "y".repeat(120)}}]});
        let text = lay(&value);
        assert!(text.contains("\n      \"nested\": {\n"), "{text}");
    }

    #[test]
    fn strings_with_delimiters_are_untouched() {
        let value = json!({"t": "a, b: {c} [d] \"e\" \\"});
        let text = lay(&value);
        assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), value);
        assert!(text.contains(r#""a, b: {c} [d] \"e\" \\""#));
    }

    #[test]
    fn unicode_survives() {
        let value = json!({"name": "Schaltplan Ü – 电路"});
        assert_eq!(serde_json::from_str::<Value>(&lay(&value)).unwrap(), value);
    }

    fn arb_json() -> impl Strategy<Value = Value> {
        let leaf = prop_oneof![
            Just(Value::Null),
            any::<bool>().prop_map(Value::Bool),
            any::<i64>().prop_map(|n| json!(n)),
            "[ -~äöü\\\\\"]{0,30}".prop_map(Value::String),
        ];
        leaf.prop_recursive(6, 200, 12, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..12).prop_map(Value::Array),
                prop::collection::btree_map("[a-z]{1,12}", inner, 0..12)
                    .prop_map(|map| Value::Object(map.into_iter().collect())),
            ]
        })
    }

    proptest! {
        #[test]
        fn layout_preserves_values_and_is_idempotent(value in arb_json()) {
            let text = lay(&value);
            let parsed: Value = serde_json::from_str(&text).unwrap();
            prop_assert_eq!(&parsed, &value);
            prop_assert_eq!(lay(&parsed), text.clone());
            prop_assert!(text.ends_with('\n'));
            prop_assert!(!text.contains('\r') && !text.contains('\t'));
        }
    }
}
