//! React context boundary extraction (D-151).
//!
//! Maps `createContext()` calls to EventChannel Producer boundaries and
//! `useContext()` calls to EventChannel Consumer boundaries.

use crate::detect::js_framework;
use crate::model::semantic::{Boundary, BoundaryKind, BoundaryRole};
use crate::model::CanonicalPath;
use crate::semantic::BoundaryExtractor;

/// Extracts React context provider/consumer boundaries.
pub struct ReactBoundaryExtractor;

impl BoundaryExtractor for ReactBoundaryExtractor {
    fn extensions(&self) -> &[&str] {
        &["ts", "tsx", "js", "jsx"]
    }

    fn extract(
        &self,
        tree: &tree_sitter::Tree,
        source: &[u8],
        path: &CanonicalPath,
    ) -> Vec<Boundary> {
        let mut boundaries = Vec::new();

        // createContext() → Producer
        for name in js_framework::find_create_context_calls(tree, source) {
            boundaries.push(Boundary {
                kind: BoundaryKind::EventChannel,
                name: format!("Context:{}", name),
                role: BoundaryRole::Producer,
                file: path.clone(),
                line: 0,
                framework: Some("react".to_string()),
                method: None,
            });
        }

        // useContext() → Consumer
        for name in js_framework::find_use_context_calls(tree, source) {
            boundaries.push(Boundary {
                kind: BoundaryKind::EventChannel,
                name: format!("Context:{}", name),
                role: BoundaryRole::Consumer,
                file: path.clone(),
                line: 0,
                framework: Some("react".to_string()),
                method: None,
            });
        }

        boundaries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_tsx(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter::Language::from(
                tree_sitter_typescript::LANGUAGE_TSX,
            ))
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    fn extract(source: &str, path: &str) -> Vec<Boundary> {
        let tree = parse_tsx(source);
        let cp = CanonicalPath::new(path.to_string());
        ReactBoundaryExtractor.extract(&tree, source.as_bytes(), &cp)
    }

    // --- SC-25: createContext → Producer, useContext → Consumer ---

    #[test]
    fn create_context_produces_producer_boundary() {
        let boundaries = extract(
            "const ThemeContext = createContext('light');",
            "src/contexts/theme.ts",
        );
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].kind, BoundaryKind::EventChannel);
        assert_eq!(boundaries[0].name, "Context:ThemeContext");
        assert_eq!(boundaries[0].role, BoundaryRole::Producer);
        assert_eq!(boundaries[0].framework.as_deref(), Some("react"));
    }

    #[test]
    fn use_context_produces_consumer_boundary() {
        let boundaries = extract(
            "const theme = useContext(ThemeContext);",
            "src/components/Themed.tsx",
        );
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].kind, BoundaryKind::EventChannel);
        assert_eq!(boundaries[0].name, "Context:ThemeContext");
        assert_eq!(boundaries[0].role, BoundaryRole::Consumer);
    }

    #[test]
    fn both_provider_and_consumer_in_same_file() {
        let source = r#"
const AuthContext = createContext(null);
function useAuth() {
    return useContext(AuthContext);
}
"#;
        let boundaries = extract(source, "src/auth.tsx");
        assert_eq!(boundaries.len(), 2);

        let producer = boundaries.iter().find(|b| b.role == BoundaryRole::Producer).unwrap();
        assert_eq!(producer.name, "Context:AuthContext");

        let consumer = boundaries.iter().find(|b| b.role == BoundaryRole::Consumer).unwrap();
        assert_eq!(consumer.name, "Context:AuthContext");
    }

    #[test]
    fn no_context_no_boundaries() {
        let boundaries = extract(
            "export function add(a: number, b: number) { return a + b; }",
            "src/utils.ts",
        );
        assert!(boundaries.is_empty());
    }

    #[test]
    fn multiple_contexts() {
        let source = r#"
const ThemeContext = createContext('light');
const AuthContext = React.createContext(null);
"#;
        let boundaries = extract(source, "src/contexts.ts");
        assert_eq!(boundaries.len(), 2);
        let names: Vec<&str> = boundaries.iter().map(|b| b.name.as_str()).collect();
        assert!(names.contains(&"Context:AuthContext"));
        assert!(names.contains(&"Context:ThemeContext"));
    }
}
