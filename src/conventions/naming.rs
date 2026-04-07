use std::collections::BTreeMap;

use crate::conventions::types::{NamingCase, NamingConvention};
use crate::model::{FileType, ProjectGraph, SymbolKind};

/// Classify a symbol name into a naming case via single-pass character scan.
///
/// A name is only classified if it has at least one defining characteristic:
/// - PascalCase: starts uppercase + has lowercase + no `_` or `-`
/// - camelCase: starts lowercase + has uppercase + no `_` or `-`
/// - snake_case: all lowercase/digits + at least one `_`
/// - SCREAMING_SNAKE_CASE: all uppercase/digits + at least one `_`
/// - Ambiguous: doesn't definitively match (e.g., single lowercase word `get`)
pub fn classify_case(name: &str) -> NamingCase {
    if name.is_empty() {
        return NamingCase::Ambiguous;
    }

    let bytes = name.as_bytes();
    let first = bytes[0];

    let mut has_upper = false;
    let mut has_lower = false;
    let mut has_underscore = false;
    let mut has_hyphen = false;

    for &b in bytes {
        match b {
            b'A'..=b'Z' => has_upper = true,
            b'a'..=b'z' => has_lower = true,
            b'_' => has_underscore = true,
            b'-' => has_hyphen = true,
            b'0'..=b'9' => {}
            // Non-ASCII or special chars → ambiguous
            _ => return NamingCase::Ambiguous,
        }
    }

    let has_separator = has_underscore || has_hyphen;

    match (first.is_ascii_uppercase(), has_upper, has_lower, has_separator, has_underscore) {
        // SCREAMING_SNAKE_CASE: all upper + at least one underscore
        (true, true, false, true, true) => NamingCase::ScreamingSnakeCase,
        // PascalCase: starts upper + has lower + no separators
        (true, _, true, false, _) => NamingCase::PascalCase,
        // camelCase: starts lower + has upper + no separators
        (false, true, true, false, _) => NamingCase::CamelCase,
        // snake_case: all lower + at least one underscore
        (false, false, true, _, true) => NamingCase::SnakeCase,
        // Everything else is ambiguous
        _ => NamingCase::Ambiguous,
    }
}

/// Returns true if `name` does NOT violate `convention`.
///
/// A name violates a convention if it definitively belongs to a DIFFERENT convention.
/// Ambiguous names (single lowercase word, single uppercase word) don't violate anything.
fn conforms_to(name: &str, convention: NamingCase) -> bool {
    let actual = classify_case(name);
    // Ambiguous names conform to any convention (they don't violate)
    if actual == NamingCase::Ambiguous {
        return true;
    }
    actual == convention
}

fn symbol_kind_label(kind: &SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "function",
        SymbolKind::Method => "method",
        SymbolKind::Class => "class",
        SymbolKind::Struct => "struct",
        SymbolKind::Interface | SymbolKind::Trait => "interface",
        SymbolKind::Type => "type",
        SymbolKind::Enum => "enum",
        SymbolKind::Const => "constant",
        SymbolKind::Variable => "variable",
        SymbolKind::Module => "module",
    }
}

/// Analyze naming conventions across all symbol kinds in the graph.
///
/// Algorithm:
/// 1. Collect all symbols from source files in scope
/// 2. Group by SymbolKind
/// 3. For each group: definitively-classified names vote for dominant case
/// 4. Conforming = names that don't violate dominant case (includes ambiguous)
/// 5. Exceptions = names that actively violate dominant case (up to 5)
/// 6. Skip groups with < 2 total symbols
pub fn naming_conventions(
    graph: &ProjectGraph,
    scope: Option<&str>,
) -> Vec<NamingConvention> {
    // Normalize scope to ensure trailing slash for prefix matching
    let scope_prefix = scope.map(|s| {
        if s.ends_with('/') { s.to_string() } else { format!("{s}/") }
    });

    // Collect symbols grouped by kind
    let mut groups: BTreeMap<&'static str, Vec<&str>> = BTreeMap::new();

    for (path, node) in &graph.nodes {
        // Filter by scope
        if let Some(ref prefix) = scope_prefix {
            if !path.as_str().starts_with(prefix.as_str()) {
                continue;
            }
        }

        // Only source files (skip tests, configs, etc.)
        if node.file_type != FileType::Source && node.file_type != FileType::TypeDef {
            continue;
        }

        for sym in &node.symbols {
            let label = symbol_kind_label(&sym.kind);
            groups.entry(label).or_default().push(&sym.name);
        }
    }

    let mut result = Vec::new();

    for (kind_label, names) in &groups {
        // Skip groups with < 2 symbols — can't detect a convention
        if names.len() < 2 {
            continue;
        }

        // Phase 1: Only definitively-classified names vote
        let mut votes: BTreeMap<NamingCase, usize> = BTreeMap::new();
        for name in names {
            let case = classify_case(name);
            if case != NamingCase::Ambiguous {
                *votes.entry(case).or_default() += 1;
            }
        }

        // If no definitive votes, all are ambiguous — skip
        if votes.is_empty() {
            continue;
        }

        // Find dominant case (most votes)
        let dominant = *votes
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(case, _)| case)
            .expect("votes is non-empty");

        // Phase 2: Count conforming (doesn't violate) and exceptions (violates)
        let mut conforming = 0usize;
        let mut exceptions = Vec::new();

        for name in names {
            if conforms_to(name, dominant) {
                conforming += 1;
            } else if exceptions.len() < 5 {
                exceptions.push(name.to_string());
            }
        }

        // Sort exceptions for deterministic output
        exceptions.sort();

        result.push(NamingConvention {
            symbol_kind: kind_label.to_string(),
            dominant_case: dominant,
            conforming,
            total: names.len(),
            exceptions,
        });
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        CanonicalPath, ContentHash, LineSpan, Node, SymbolDef, Visibility,
    };
    use std::collections::BTreeMap;

    // --- classify_case tests ---

    #[test]
    fn classify_pascal_case() {
        assert_eq!(classify_case("UserService"), NamingCase::PascalCase);
        assert_eq!(classify_case("Config"), NamingCase::PascalCase);
        assert_eq!(classify_case("XMLParser"), NamingCase::PascalCase);
        assert_eq!(classify_case("Vec3"), NamingCase::PascalCase);
    }

    #[test]
    fn classify_camel_case() {
        assert_eq!(classify_case("getUserById"), NamingCase::CamelCase);
        assert_eq!(classify_case("formatUser"), NamingCase::CamelCase);
        assert_eq!(classify_case("parseHTML"), NamingCase::CamelCase);
    }

    #[test]
    fn classify_snake_case() {
        assert_eq!(classify_case("get_user"), NamingCase::SnakeCase);
        assert_eq!(classify_case("format_user_name"), NamingCase::SnakeCase);
        assert_eq!(classify_case("is_valid"), NamingCase::SnakeCase);
    }

    #[test]
    fn classify_screaming_snake() {
        assert_eq!(classify_case("MAX_RETRIES"), NamingCase::ScreamingSnakeCase);
        assert_eq!(classify_case("API_BASE_URL"), NamingCase::ScreamingSnakeCase);
        assert_eq!(classify_case("HTTP_200"), NamingCase::ScreamingSnakeCase);
    }

    #[test]
    fn classify_ambiguous() {
        // Single lowercase word — no defining characteristic
        assert_eq!(classify_case("get"), NamingCase::Ambiguous);
        assert_eq!(classify_case("user"), NamingCase::Ambiguous);
        // Single uppercase word (no underscore)
        assert_eq!(classify_case("ID"), NamingCase::Ambiguous);
        assert_eq!(classify_case("OK"), NamingCase::Ambiguous);
        // Empty
        assert_eq!(classify_case(""), NamingCase::Ambiguous);
    }

    #[test]
    fn classify_edge_cases() {
        // Single char
        assert_eq!(classify_case("x"), NamingCase::Ambiguous);
        assert_eq!(classify_case("X"), NamingCase::Ambiguous);
        // Digits only with underscore
        assert_eq!(classify_case("a_1"), NamingCase::SnakeCase);
    }

    // --- naming_conventions tests ---

    fn make_symbol(name: &str, kind: SymbolKind) -> SymbolDef {
        SymbolDef {
            name: name.to_string(),
            kind,
            visibility: Visibility::Public,
            span: LineSpan { start: 1, end: 1 },
            signature: None,
            parent: None,
        }
    }

    fn make_node(symbols: Vec<SymbolDef>) -> Node {
        Node {
            file_type: FileType::Source,
            layer: crate::model::ArchLayer::Unknown,
            fsd_layer: None,
            arch_depth: 0,
            lines: 10,
            hash: ContentHash::new("0000000000000000".to_string()),
            exports: vec![],
            cluster: crate::model::ClusterId::new("default"),
            symbols,
        }
    }

    fn make_graph(files: Vec<(&str, Vec<SymbolDef>)>) -> ProjectGraph {
        let mut nodes = BTreeMap::new();
        for (path, symbols) in files {
            nodes.insert(CanonicalPath::new(path), make_node(symbols));
        }
        ProjectGraph {
            nodes,
            edges: vec![],
        }
    }

    #[test]
    fn conventions_groups_by_kind() {
        let graph = make_graph(vec![
            ("src/models.ts", vec![
                make_symbol("User", SymbolKind::Interface),
                make_symbol("Config", SymbolKind::Interface),
                make_symbol("AppState", SymbolKind::Interface),
            ]),
            ("src/utils.ts", vec![
                make_symbol("formatUser", SymbolKind::Function),
                make_symbol("validateEmail", SymbolKind::Function),
                make_symbol("parseConfig", SymbolKind::Function),
            ]),
        ]);

        let result = naming_conventions(&graph, None);

        let functions = result.iter().find(|c| c.symbol_kind == "function").unwrap();
        assert_eq!(functions.dominant_case, NamingCase::CamelCase);
        assert_eq!(functions.conforming, 3);
        assert_eq!(functions.total, 3);
        assert!(functions.exceptions.is_empty());

        let interfaces = result.iter().find(|c| c.symbol_kind == "interface").unwrap();
        assert_eq!(interfaces.dominant_case, NamingCase::PascalCase);
        assert_eq!(interfaces.conforming, 3);
        assert_eq!(interfaces.total, 3);
    }

    #[test]
    fn conventions_reports_exceptions() {
        let graph = make_graph(vec![
            ("src/utils.ts", vec![
                make_symbol("formatUser", SymbolKind::Function),
                make_symbol("validateEmail", SymbolKind::Function),
                make_symbol("parseConfig", SymbolKind::Function),
                // Exception: snake_case in a camelCase project
                make_symbol("get_user", SymbolKind::Function),
            ]),
        ]);

        let result = naming_conventions(&graph, None);
        let functions = result.iter().find(|c| c.symbol_kind == "function").unwrap();
        assert_eq!(functions.dominant_case, NamingCase::CamelCase);
        assert_eq!(functions.conforming, 3);
        assert_eq!(functions.total, 4);
        assert_eq!(functions.exceptions, vec!["get_user"]);
    }

    #[test]
    fn conventions_ambiguous_names_conform() {
        let graph = make_graph(vec![
            ("src/utils.ts", vec![
                make_symbol("formatUser", SymbolKind::Function),
                make_symbol("validateEmail", SymbolKind::Function),
                // Ambiguous: single lowercase word — should conform to camelCase
                make_symbol("get", SymbolKind::Function),
                make_symbol("run", SymbolKind::Function),
            ]),
        ]);

        let result = naming_conventions(&graph, None);
        let functions = result.iter().find(|c| c.symbol_kind == "function").unwrap();
        assert_eq!(functions.dominant_case, NamingCase::CamelCase);
        // 2 definitive camel + 2 ambiguous (conform) = 4 conforming
        assert_eq!(functions.conforming, 4);
        assert_eq!(functions.total, 4);
        assert!(functions.exceptions.is_empty());
    }

    #[test]
    fn conventions_scope_filter() {
        let graph = make_graph(vec![
            ("src/auth/login.ts", vec![
                make_symbol("handleLogin", SymbolKind::Function),
                make_symbol("validateToken", SymbolKind::Function),
            ]),
            ("src/utils/format.ts", vec![
                make_symbol("format_date", SymbolKind::Function),
                make_symbol("format_name", SymbolKind::Function),
            ]),
        ]);

        // Scope to src/auth only
        let result = naming_conventions(&graph, Some("src/auth"));
        let functions = result.iter().find(|c| c.symbol_kind == "function").unwrap();
        assert_eq!(functions.dominant_case, NamingCase::CamelCase);
        assert_eq!(functions.total, 2);
    }

    #[test]
    fn conventions_skips_small_groups() {
        let graph = make_graph(vec![
            ("src/app.ts", vec![
                // Only 1 function — should be skipped
                make_symbol("main", SymbolKind::Function),
                // 2 interfaces — should be included
                make_symbol("User", SymbolKind::Interface),
                make_symbol("Config", SymbolKind::Interface),
            ]),
        ]);

        let result = naming_conventions(&graph, None);
        // Function group skipped (only 1 symbol)
        assert!(result.iter().find(|c| c.symbol_kind == "function").is_none());
        // Interface group present (2 symbols)
        assert!(result.iter().find(|c| c.symbol_kind == "interface").is_some());
    }

    #[test]
    fn conventions_exceptions_capped_at_5() {
        let graph = make_graph(vec![
            ("src/mixed.ts", vec![
                // 3 camelCase (dominant)
                make_symbol("formatUser", SymbolKind::Function),
                make_symbol("validateEmail", SymbolKind::Function),
                make_symbol("parseConfig", SymbolKind::Function),
                // 7 snake_case (exceptions, but capped at 5)
                make_symbol("get_user", SymbolKind::Function),
                make_symbol("set_user", SymbolKind::Function),
                make_symbol("find_all", SymbolKind::Function),
                make_symbol("delete_one", SymbolKind::Function),
                make_symbol("update_record", SymbolKind::Function),
                make_symbol("create_entry", SymbolKind::Function),
                make_symbol("remove_item", SymbolKind::Function),
            ]),
        ]);

        let result = naming_conventions(&graph, None);
        let functions = result.iter().find(|c| c.symbol_kind == "function").unwrap();
        assert!(functions.exceptions.len() <= 5);
    }
}
