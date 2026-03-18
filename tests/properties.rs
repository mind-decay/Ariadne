use proptest::prelude::*;

use ariadne_graph::hash::hash_content;
use ariadne_graph::model::CanonicalPath;
use ariadne_graph::diagnostic::WarningCode;

// --- CanonicalPath normalization properties ---

proptest! {
    #[test]
    fn canonical_path_no_backslashes(s in "[a-z/._]{1,50}") {
        let path = CanonicalPath::new(s);
        prop_assert!(!path.as_str().contains('\\'));
    }

    #[test]
    fn canonical_path_no_dot_slash(s in "[a-z/._]{1,50}") {
        let path = CanonicalPath::new(s);
        prop_assert!(!path.as_str().starts_with("./"));
        prop_assert!(!path.as_str().contains("/./"));
    }

    #[test]
    fn canonical_path_no_double_slashes(s in "[a-z/._]{1,50}") {
        let path = CanonicalPath::new(s);
        prop_assert!(!path.as_str().contains("//"));
    }

    #[test]
    fn canonical_path_no_trailing_slash(s in "[a-z/._]{1,50}") {
        let path = CanonicalPath::new(s);
        if !path.as_str().is_empty() {
            prop_assert!(!path.as_str().ends_with('/'));
        }
    }
}

// --- Content hash determinism ---

proptest! {
    #[test]
    fn hash_deterministic(content in "[a-zA-Z0-9 \n]{0,1000}") {
        let h1 = hash_content(content.as_bytes());
        let h2 = hash_content(content.as_bytes());
        prop_assert_eq!(h1.as_str(), h2.as_str());
    }
}

// --- CanonicalPath with backslashes ---

proptest! {
    #[test]
    fn canonical_path_normalizes_backslashes(s in "[a-z\\\\._]{1,30}") {
        let path = CanonicalPath::new(s);
        prop_assert!(!path.as_str().contains('\\'));
    }
}

// --- Warning code display format ---

#[test]
fn warning_codes_display_as_w0xx() {
    let codes = [
        WarningCode::W001ParseFailed,
        WarningCode::W002ReadFailed,
        WarningCode::W003FileTooLarge,
        WarningCode::W004BinaryFile,
        WarningCode::W006ImportUnresolved,
        WarningCode::W007PartialParse,
        WarningCode::W008ConfigParseFailed,
        WarningCode::W009EncodingError,
    ];

    for code in &codes {
        let display = format!("{}", code);
        assert!(
            display.starts_with('W'),
            "Warning code '{}' should start with 'W'",
            display
        );
        assert_eq!(
            display.len(),
            4,
            "Warning code '{}' should be exactly 4 characters (W0XX)",
            display
        );
        assert!(
            display[1..].chars().all(|c| c.is_ascii_digit()),
            "Warning code '{}' should have 3 digits after 'W'",
            display
        );
    }
}

// --- Content hash format ---

proptest! {
    #[test]
    fn hash_is_16_hex_chars(content in "[a-zA-Z0-9]{0,500}") {
        let hash = hash_content(content.as_bytes());
        let s = hash.as_str();
        prop_assert_eq!(s.len(), 16, "hash should be 16 chars, got {}", s.len());
        prop_assert!(
            s.chars().all(|c| c.is_ascii_hexdigit()),
            "hash should be hex chars only, got '{}'", s
        );
    }
}
