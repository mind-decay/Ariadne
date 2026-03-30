/// Strip JSONC comments and trailing commas so serde_json can parse the result.
///
/// Handles:
/// - `//` line comments (not inside string literals)
/// - `/* */` block comments (not inside string literals)
/// - Trailing commas before `]` and `}`
pub fn strip_jsonc_comments(input: &str) -> String {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len);
    let mut i = 0;
    let mut in_string = false;

    while i < len {
        let ch = bytes[i];

        if in_string {
            out.push(ch as char);
            if ch == b'\\' && i + 1 < len {
                // Escaped character — push it and skip
                i += 1;
                out.push(bytes[i] as char);
            } else if ch == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        // Not inside a string
        if ch == b'"' {
            in_string = true;
            out.push('"');
            i += 1;
        } else if ch == b'/' && i + 1 < len && bytes[i + 1] == b'/' {
            // Line comment — skip to end of line
            i += 2;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
        } else if ch == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            // Block comment — skip to */
            i += 2;
            while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 < len {
                i += 2; // skip */
            }
        } else {
            out.push(ch as char);
            i += 1;
        }
    }

    // Strip trailing commas before ] and }
    strip_trailing_commas(&out)
}

/// Remove trailing commas that appear before `]` or `}` (with optional whitespace between).
fn strip_trailing_commas(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == ',' {
            // Look ahead past whitespace for ] or }
            let mut j = i + 1;
            while j < len && chars[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < len && (chars[j] == ']' || chars[j] == '}') {
                // Skip the comma, keep the whitespace
                i += 1;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_json_passthrough() {
        let input = r#"{"key": "value", "num": 42}"#;
        assert_eq!(strip_jsonc_comments(input), input);
    }

    #[test]
    fn single_line_comment_removal() {
        let input = "{\n  // this is a comment\n  \"key\": \"value\"\n}";
        let expected = "{\n  \n  \"key\": \"value\"\n}";
        assert_eq!(strip_jsonc_comments(input), expected);
    }

    #[test]
    fn block_comment_removal() {
        let input = r#"{"key": /* comment */ "value"}"#;
        let expected = r#"{"key":  "value"}"#;
        assert_eq!(strip_jsonc_comments(input), expected);
    }

    #[test]
    fn comments_inside_strings_preserved() {
        let input = r#"{"key": "value // not a comment", "k2": "/* also not */"}"#;
        assert_eq!(strip_jsonc_comments(input), input);
    }

    #[test]
    fn trailing_comma_removal() {
        let input = r#"{"a": 1, "b": 2,}"#;
        let expected = r#"{"a": 1, "b": 2}"#;
        assert_eq!(strip_jsonc_comments(input), expected);
    }

    #[test]
    fn trailing_comma_in_array() {
        let input = r#"[1, 2, 3,]"#;
        let expected = r#"[1, 2, 3]"#;
        assert_eq!(strip_jsonc_comments(input), expected);
    }

    #[test]
    fn mixed_comments_and_strings() {
        let input = r#"{
  // comment
  "url": "https://example.com", // trailing
  /* block */
  "pattern": "src/**/*.ts",
}"#;
        let result = strip_jsonc_comments(input);
        // Should parse as valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["url"], "https://example.com");
        assert_eq!(parsed["pattern"], "src/**/*.ts");
    }

    #[test]
    fn empty_input() {
        assert_eq!(strip_jsonc_comments(""), "");
    }

    #[test]
    fn escaped_quotes_in_strings() {
        let input = r#"{"key": "value with \" escaped // quote"}"#;
        assert_eq!(strip_jsonc_comments(input), input);
    }
}
