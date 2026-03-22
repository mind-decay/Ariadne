use tree_sitter::Node;

/// Find first child node of the given kind.
pub(crate) fn find_child_by_kind<'a>(node: &Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    let result = node
        .children(&mut cursor)
        .find(|child| child.kind() == kind);
    result
}

/// Strip surrounding quotes (single, double, or backtick) from a string slice.
#[allow(dead_code)]
pub(crate) fn strip_quotes(s: &str) -> &str {
    if s.len() >= 2 {
        let bytes = s.as_bytes();
        let first = bytes[0];
        let last = bytes[s.len() - 1];
        if (first == b'"' && last == b'"')
            || (first == b'\'' && last == b'\'')
            || (first == b'`' && last == b'`')
        {
            return &s[1..s.len() - 1];
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_double_quotes() {
        assert_eq!(strip_quotes("\"hello\""), "hello");
    }

    #[test]
    fn strip_single_quotes() {
        assert_eq!(strip_quotes("'hello'"), "hello");
    }

    #[test]
    fn strip_backtick_quotes() {
        assert_eq!(strip_quotes("`hello`"), "hello");
    }

    #[test]
    fn no_quotes_unchanged() {
        assert_eq!(strip_quotes("hello"), "hello");
    }

    #[test]
    fn mismatched_quotes_unchanged() {
        assert_eq!(strip_quotes("\"hello'"), "\"hello'");
    }

    #[test]
    fn empty_string_unchanged() {
        assert_eq!(strip_quotes(""), "");
    }

    #[test]
    fn single_char_unchanged() {
        assert_eq!(strip_quotes("\""), "\"");
    }
}
