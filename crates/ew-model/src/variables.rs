//! Text variables (`${NAME}`) in texts and title blocks.

/// Variables provided by the application. User variables with these names
/// are ignored.
pub const BUILTIN_VARIABLES: &[&str] = &[
    "PROJECT_NAME",
    "REVISION",
    "CURRENT_DATE",
    "SHEET_NAME",
    "FILENAME",
];

/// How deep variables may refer to other variables.
const MAX_DEPTH: usize = 8;

/// `true` for a valid variable name: a letter or `_`, then letters, digits
/// or `_`, at most 64 characters.
#[must_use]
pub fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    name.len() <= 64
        && (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Replaces `${NAME}` references using `lookup`. Unknown variables stay as
/// they are; values may contain further references (up to 8 levels deep).
#[must_use]
pub fn expand(text: &str, lookup: &dyn Fn(&str) -> Option<String>) -> String {
    expand_depth(text, lookup, 0)
}

fn expand_depth(text: &str, lookup: &dyn Fn(&str) -> Option<String>, depth: usize) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let name = &after[..end];
        match lookup(name).filter(|_| is_valid_name(name)) {
            Some(value) if depth < MAX_DEPTH => {
                out.push_str(&expand_depth(&value, lookup, depth + 1));
            }
            Some(value) => out.push_str(&value),
            None => out.push_str(&rest[start..start + 3 + end]),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Names of all `${NAME}` references in a text.
#[must_use]
pub fn references(text: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            break;
        };
        names.push(&after[..end]);
        rest = &after[end + 1..];
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn lookup_from(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: BTreeMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |name| map.get(name).cloned()
    }

    #[test]
    fn expands_known_variables() {
        let lookup = lookup_from(&[("PROJECT_NAME", "Demo"), ("REVISION", "B")]);
        assert_eq!(
            expand("${PROJECT_NAME} rev ${REVISION}", &lookup),
            "Demo rev B"
        );
        assert_eq!(expand("no vars", &lookup), "no vars");
        assert_eq!(expand("${UNKNOWN}!", &lookup), "${UNKNOWN}!");
        assert_eq!(
            expand("open ${PROJECT_NAME", &lookup),
            "open ${PROJECT_NAME"
        );
        assert_eq!(expand("${}", &lookup), "${}");
        assert_eq!(expand("äö ${REVISION} ü", &lookup), "äö B ü");
    }

    #[test]
    fn nested_and_recursive_variables() {
        let lookup = lookup_from(&[("A", "${B}"), ("B", "x"), ("LOOP", "${LOOP}")]);
        assert_eq!(expand("${A}", &lookup), "x");
        // A self-reference stops after the depth limit instead of looping.
        assert_eq!(expand("${LOOP}", &lookup), "${LOOP}");
    }

    #[test]
    fn names() {
        assert!(is_valid_name("PROJECT_NAME"));
        assert!(is_valid_name("_x1"));
        assert!(!is_valid_name(""));
        assert!(!is_valid_name("1A"));
        assert!(!is_valid_name("A-B"));
        assert!(!is_valid_name(&"A".repeat(65)));
        assert_eq!(references("${A} and ${B_2}"), vec!["A", "B_2"]);
    }
}
