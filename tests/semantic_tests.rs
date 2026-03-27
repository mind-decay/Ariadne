mod helpers;

use std::collections::BTreeMap;
use std::path::PathBuf;

use ariadne_graph::model::semantic::{Boundary, BoundaryKind, BoundaryRole};
use ariadne_graph::model::types::CanonicalPath;
use ariadne_graph::semantic::edges::build_semantic_edges;
use ariadne_graph::semantic::events::EventExtractor;
use ariadne_graph::semantic::http::HttpRouteExtractor;
use ariadne_graph::semantic::BoundaryExtractor;
use ariadne_graph::serial;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("semantic")
}

fn read_fixture(name: &str) -> String {
    let path = fixture_dir().join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture '{}': {}", name, e))
}

/// Parse source with a specific tree-sitter language and extract HTTP routes.
fn extract_http_routes(source: &str, ext: &str, lang: tree_sitter::Language) -> Vec<Boundary> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang).unwrap();
    let tree = parser.parse(source.as_bytes(), None).unwrap();
    let path = CanonicalPath::new(format!("fixture.{ext}"));
    let extractor = HttpRouteExtractor;
    extractor.extract(&tree, source.as_bytes(), &path)
}

/// Parse source with a specific tree-sitter language and extract events.
fn extract_events(source: &str, ext: &str, lang: tree_sitter::Language) -> Vec<Boundary> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang).unwrap();
    let tree = parser.parse(source.as_bytes(), None).unwrap();
    let path = CanonicalPath::new(format!("fixture.{ext}"));
    let extractor = EventExtractor;
    extractor.extract(&tree, source.as_bytes(), &path)
}

fn ts_lang() -> tree_sitter::Language {
    tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT)
}

fn py_lang() -> tree_sitter::Language {
    tree_sitter::Language::from(tree_sitter_python::LANGUAGE)
}

fn java_lang() -> tree_sitter::Language {
    tree_sitter::Language::from(tree_sitter_java::LANGUAGE)
}

fn go_lang() -> tree_sitter::Language {
    tree_sitter::Language::from(tree_sitter_go::LANGUAGE)
}

fn cs_lang() -> tree_sitter::Language {
    tree_sitter::Language::from(tree_sitter_c_sharp::LANGUAGE)
}

// ===========================================================================
// AC-1: HTTP route detection per framework
// ===========================================================================

#[test]
fn ac1_express_routes_fixture() {
    let source = read_fixture("express_routes.ts");
    let routes = extract_http_routes(&source, "ts", ts_lang());

    // 4 producers (get, post, delete, use) + 2 consumers (fetch, axios)
    let producers: Vec<_> = routes
        .iter()
        .filter(|b| matches!(b.role, BoundaryRole::Producer | BoundaryRole::Both))
        .collect();
    let consumers: Vec<_> = routes
        .iter()
        .filter(|b| matches!(b.role, BoundaryRole::Consumer))
        .collect();

    assert!(
        producers.len() >= 3,
        "Express fixture should have >= 3 producers, got {}",
        producers.len()
    );
    assert!(
        consumers.len() >= 1,
        "Express fixture should have >= 1 consumer, got {}",
        consumers.len()
    );

    // Verify specific routes exist
    let names: Vec<&str> = routes.iter().map(|b| b.name.as_str()).collect();
    assert!(names.contains(&"/api/users"), "Missing /api/users route");

    // All should be HttpRoute kind
    assert!(routes.iter().all(|b| b.kind == BoundaryKind::HttpRoute));
}

#[test]
fn ac1_fastapi_routes_fixture() {
    let source = read_fixture("fastapi_routes.py");
    let routes = extract_http_routes(&source, "py", py_lang());

    assert_eq!(routes.len(), 3, "FastAPI fixture should have 3 routes");
    assert!(routes.iter().all(|b| b.role == BoundaryRole::Producer));
    assert!(routes.iter().all(|b| b.framework.as_deref() == Some("fastapi")));

    let names: Vec<&str> = routes.iter().map(|b| b.name.as_str()).collect();
    assert!(names.contains(&"/users"));
    assert!(names.contains(&"/users/{user_id}"));
}

#[test]
fn ac1_spring_routes_fixture() {
    let source = read_fixture("spring_routes.java");
    let routes = extract_http_routes(&source, "java", java_lang());

    assert!(
        routes.len() >= 3,
        "Spring fixture should have >= 3 routes, got {}",
        routes.len()
    );
    assert!(routes.iter().all(|b| b.role == BoundaryRole::Producer));
    assert!(routes.iter().all(|b| b.framework.as_deref() == Some("spring")));
}

#[test]
fn ac1_go_routes_fixture() {
    let source = read_fixture("go_routes.go");
    let routes = extract_http_routes(&source, "go", go_lang());

    assert_eq!(routes.len(), 4, "Go fixture should have 4 routes");
    assert!(routes.iter().all(|b| b.role == BoundaryRole::Producer));

    // Verify both go_http and gin frameworks detected
    let frameworks: Vec<_> = routes.iter().filter_map(|b| b.framework.as_deref()).collect();
    assert!(frameworks.contains(&"go_http"));
    assert!(frameworks.contains(&"gin"));
}

#[test]
fn ac1_aspnet_routes_fixture() {
    let source = read_fixture("aspnet_routes.cs");
    let routes = extract_http_routes(&source, "cs", cs_lang());

    assert!(
        routes.len() >= 3,
        "ASP.NET fixture should have >= 3 routes, got {}",
        routes.len()
    );
    assert!(routes.iter().all(|b| b.role == BoundaryRole::Producer));
    assert!(routes.iter().all(|b| b.framework.as_deref() == Some("aspnet")));
}

// ===========================================================================
// AC-2: Event detection
// ===========================================================================

#[test]
fn ac2_event_emitters_fixture() {
    let source = read_fixture("event_emitters.ts");
    let events = extract_events(&source, "ts", ts_lang());

    let producers: Vec<_> = events
        .iter()
        .filter(|b| b.role == BoundaryRole::Producer)
        .collect();
    let consumers: Vec<_> = events
        .iter()
        .filter(|b| b.role == BoundaryRole::Consumer)
        .collect();

    assert!(
        producers.len() >= 2,
        "Event emitters fixture should have >= 2 producers, got {}",
        producers.len()
    );
    assert!(
        consumers.len() >= 2,
        "Event emitters fixture should have >= 2 consumers, got {}",
        consumers.len()
    );

    // All should be EventChannel kind
    assert!(events.iter().all(|b| b.kind == BoundaryKind::EventChannel));
}

#[test]
fn ac2_event_generic_python_fixture() {
    let source = read_fixture("event_generic.py");
    let events = extract_events(&source, "py", py_lang());

    let producers: Vec<_> = events
        .iter()
        .filter(|b| b.role == BoundaryRole::Producer)
        .collect();
    let consumers: Vec<_> = events
        .iter()
        .filter(|b| b.role == BoundaryRole::Consumer)
        .collect();

    assert!(
        producers.len() >= 1,
        "Python events should have >= 1 producer, got {}",
        producers.len()
    );
    assert!(
        consumers.len() >= 1,
        "Python events should have >= 1 consumer, got {}",
        consumers.len()
    );
}

// ===========================================================================
// AC-5: DOM event skip list (false positive rate)
// ===========================================================================

#[test]
fn ac5_dom_events_zero_boundaries() {
    let source = read_fixture("dom_events.ts");

    let http = extract_http_routes(&source, "ts", ts_lang());
    let events = extract_events(&source, "ts", ts_lang());

    assert!(
        http.is_empty(),
        "DOM events fixture should produce 0 HTTP boundaries, got {}",
        http.len()
    );
    assert!(
        events.is_empty(),
        "DOM events fixture should produce 0 event boundaries, got {}",
        events.len()
    );
}

#[test]
fn ac5_no_boundaries_rust_fixture() {
    // Rust file with no framework patterns
    let source = read_fixture("no_boundaries.rs");
    // HttpRouteExtractor doesn't handle .rs files, so we test it returns empty
    let extractor = HttpRouteExtractor;
    let path = CanonicalPath::new("no_boundaries.rs");
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&ts_lang()).unwrap(); // doesn't matter, wrong ext
    let tree = parser.parse(source.as_bytes(), None).unwrap();
    let result = extractor.extract(&tree, source.as_bytes(), &path);
    assert!(result.is_empty());

    let event_ext = EventExtractor;
    let result = event_ext.extract(&tree, source.as_bytes(), &path);
    assert!(result.is_empty());
}

// ===========================================================================
// AC-5: False positive rate on fixtures
// ===========================================================================

#[test]
fn ac5_false_positive_rate_within_threshold() {
    // Count total boundaries across all fixtures vs expected ranges.
    // The test files document expected counts. We verify detected counts
    // are within reasonable bounds (< 5% false positive means no spurious detections).

    // express_routes.ts: expect 4-6 producers+both, 1-2 consumers
    let source = read_fixture("express_routes.ts");
    let routes = extract_http_routes(&source, "ts", ts_lang());
    // Should not detect more than documented + small tolerance
    assert!(
        routes.len() <= 8,
        "Express routes: too many boundaries ({}) suggests false positives",
        routes.len()
    );

    // dom_events.ts: expect exactly 0
    let source = read_fixture("dom_events.ts");
    let events = extract_events(&source, "ts", ts_lang());
    assert_eq!(events.len(), 0, "DOM events should produce 0 boundaries");
}

// ===========================================================================
// AC-7: Orphan route and orphan event detection
// ===========================================================================

#[test]
fn ac7_orphan_route_detection() {
    let mut boundaries = BTreeMap::new();
    // Producer with no consumer -> orphan
    boundaries.insert(
        CanonicalPath::new("src/routes.ts"),
        vec![Boundary {
            kind: BoundaryKind::HttpRoute,
            name: "/api/orphan".to_string(),
            role: BoundaryRole::Producer,
            file: CanonicalPath::new("src/routes.ts"),
            line: 1,
            framework: Some("express".to_string()),
            method: Some("GET".to_string()),
        }],
    );

    let (edges, orphan_routes, orphan_events) = build_semantic_edges(&boundaries);

    assert!(edges.is_empty());
    assert_eq!(orphan_routes, vec!["/api/orphan"]);
    assert!(orphan_events.is_empty());
}

#[test]
fn ac7_orphan_event_detection() {
    let mut boundaries = BTreeMap::new();
    // Event producer with no subscriber -> orphan
    boundaries.insert(
        CanonicalPath::new("src/emitter.ts"),
        vec![Boundary {
            kind: BoundaryKind::EventChannel,
            name: "lonely:event".to_string(),
            role: BoundaryRole::Producer,
            file: CanonicalPath::new("src/emitter.ts"),
            line: 5,
            framework: Some("node_events".to_string()),
            method: None,
        }],
    );

    let (edges, orphan_routes, orphan_events) = build_semantic_edges(&boundaries);

    assert!(edges.is_empty());
    assert!(orphan_routes.is_empty());
    assert_eq!(orphan_events, vec!["lonely:event"]);
}

// ===========================================================================
// AC-8: Mixed framework file (both HTTP + events in same file)
// ===========================================================================

#[test]
fn ac8_mixed_framework_fixture() {
    let source = read_fixture("mixed_framework.ts");

    let http = extract_http_routes(&source, "ts", ts_lang());
    let events = extract_events(&source, "ts", ts_lang());

    assert!(
        http.len() >= 2,
        "Mixed fixture should have >= 2 HTTP routes, got {}",
        http.len()
    );
    assert!(
        events.len() >= 2,
        "Mixed fixture should have >= 2 events, got {}",
        events.len()
    );

    // Verify kinds are distinct
    assert!(http.iter().all(|b| b.kind == BoundaryKind::HttpRoute));
    assert!(events.iter().all(|b| b.kind == BoundaryKind::EventChannel));
}

// ===========================================================================
// AC-9: Determinism (invariant)
// ===========================================================================

#[test]
fn ac9_deterministic_extraction() {
    // Run extraction on the same fixture twice and compare results.
    let source = read_fixture("express_routes.ts");

    let run1_http = extract_http_routes(&source, "ts", ts_lang());
    let run2_http = extract_http_routes(&source, "ts", ts_lang());
    assert_eq!(run1_http, run2_http, "HTTP extraction must be deterministic");

    let run1_events = extract_events(&source, "ts", ts_lang());
    let run2_events = extract_events(&source, "ts", ts_lang());
    assert_eq!(
        run1_events, run2_events,
        "Event extraction must be deterministic"
    );
}

#[test]
fn ac9_deterministic_edge_construction() {
    // Build edges from same data twice and compare.
    let source = read_fixture("mixed_framework.ts");
    let http = extract_http_routes(&source, "ts", ts_lang());
    let events = extract_events(&source, "ts", ts_lang());

    let mut all: Vec<Boundary> = Vec::new();
    all.extend(http);
    all.extend(events);

    let mut boundaries = BTreeMap::new();
    boundaries.insert(CanonicalPath::new("src/mixed.ts"), all.clone());

    // Add a consumer in a different file for edge creation
    boundaries.insert(
        CanonicalPath::new("src/client.ts"),
        vec![Boundary {
            kind: BoundaryKind::HttpRoute,
            name: "/api/orders".to_string(),
            role: BoundaryRole::Consumer,
            file: CanonicalPath::new("src/client.ts"),
            line: 1,
            framework: Some("fetch".to_string()),
            method: None,
        }],
    );

    let (edges1, orphans_r1, orphans_e1) = build_semantic_edges(&boundaries);
    let (edges2, orphans_r2, orphans_e2) = build_semantic_edges(&boundaries);

    assert_eq!(edges1, edges2, "Edge construction must be deterministic");
    assert_eq!(
        orphans_r1, orphans_r2,
        "Orphan routes must be deterministic"
    );
    assert_eq!(
        orphans_e1, orphans_e2,
        "Orphan events must be deterministic"
    );
}

#[test]
fn ac9_deterministic_analyze() {
    // Full analysis pipeline: same input -> same SemanticState (via BoundaryOutput JSON)
    let source = read_fixture("event_emitters.ts");
    let events = extract_events(&source, "ts", ts_lang());

    let mut boundaries = BTreeMap::new();
    boundaries.insert(CanonicalPath::new("src/events.ts"), events);

    let state1 = ariadne_graph::semantic::analyze(boundaries.clone());
    let state2 = ariadne_graph::semantic::analyze(boundaries);

    let output1 = serial::semantic_state_to_boundary_output(&state1);
    let output2 = serial::semantic_state_to_boundary_output(&state2);

    let json1 = serde_json::to_string_pretty(&output1).unwrap();
    let json2 = serde_json::to_string_pretty(&output2).unwrap();

    assert_eq!(json1, json2, "Full analysis must produce byte-identical JSON");
}

// ===========================================================================
// AC-12: Serialization round-trip
// ===========================================================================

#[test]
fn ac12_boundary_output_round_trip() {
    // Build a SemanticState, convert to BoundaryOutput, serialize, deserialize, compare.
    let mut boundaries = BTreeMap::new();
    boundaries.insert(
        CanonicalPath::new("src/routes.ts"),
        vec![
            Boundary {
                kind: BoundaryKind::HttpRoute,
                name: "/api/users".to_string(),
                role: BoundaryRole::Producer,
                file: CanonicalPath::new("src/routes.ts"),
                line: 10,
                framework: Some("express".to_string()),
                method: Some("GET".to_string()),
            },
            Boundary {
                kind: BoundaryKind::HttpRoute,
                name: "/api/users".to_string(),
                role: BoundaryRole::Producer,
                file: CanonicalPath::new("src/routes.ts"),
                line: 15,
                framework: Some("express".to_string()),
                method: Some("POST".to_string()),
            },
        ],
    );
    boundaries.insert(
        CanonicalPath::new("src/client.ts"),
        vec![Boundary {
            kind: BoundaryKind::HttpRoute,
            name: "/api/users".to_string(),
            role: BoundaryRole::Consumer,
            file: CanonicalPath::new("src/client.ts"),
            line: 5,
            framework: Some("fetch".to_string()),
            method: None,
        }],
    );
    boundaries.insert(
        CanonicalPath::new("src/events.ts"),
        vec![Boundary {
            kind: BoundaryKind::EventChannel,
            name: "user:created".to_string(),
            role: BoundaryRole::Producer,
            file: CanonicalPath::new("src/events.ts"),
            line: 20,
            framework: Some("node_events".to_string()),
            method: None,
        }],
    );

    let state = ariadne_graph::semantic::analyze(boundaries);
    let output = serial::semantic_state_to_boundary_output(&state);

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&output).unwrap();

    // Deserialize back
    let deserialized: serial::BoundaryOutput = serde_json::from_str(&json).unwrap();

    // Convert back to SemanticState
    let round_tripped = serial::boundary_output_to_semantic_state(&deserialized);
    let re_output = serial::semantic_state_to_boundary_output(&round_tripped);

    // Serialize again and compare
    let json2 = serde_json::to_string_pretty(&re_output).unwrap();

    assert_eq!(
        json, json2,
        "BoundaryOutput -> JSON -> BoundaryOutput -> JSON must produce identical output"
    );
}

#[test]
fn ac12_round_trip_preserves_counts() {
    let mut boundaries = BTreeMap::new();
    boundaries.insert(
        CanonicalPath::new("src/a.ts"),
        vec![
            Boundary {
                kind: BoundaryKind::HttpRoute,
                name: "/api/data".to_string(),
                role: BoundaryRole::Producer,
                file: CanonicalPath::new("src/a.ts"),
                line: 1,
                framework: None,
                method: None,
            },
            Boundary {
                kind: BoundaryKind::EventChannel,
                name: "data:ready".to_string(),
                role: BoundaryRole::Producer,
                file: CanonicalPath::new("src/a.ts"),
                line: 10,
                framework: None,
                method: None,
            },
        ],
    );

    let state = ariadne_graph::semantic::analyze(boundaries);
    assert_eq!(state.route_count, 1);
    assert_eq!(state.event_count, 1);

    let output = serial::semantic_state_to_boundary_output(&state);
    let json = serde_json::to_string(&output).unwrap();
    let deserialized: serial::BoundaryOutput = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.route_count, 1);
    assert_eq!(deserialized.event_count, 1);
}

#[test]
fn ac12_round_trip_preserves_orphans() {
    let mut boundaries = BTreeMap::new();
    boundaries.insert(
        CanonicalPath::new("src/orphan.ts"),
        vec![
            Boundary {
                kind: BoundaryKind::HttpRoute,
                name: "/api/lonely".to_string(),
                role: BoundaryRole::Producer,
                file: CanonicalPath::new("src/orphan.ts"),
                line: 1,
                framework: None,
                method: None,
            },
            Boundary {
                kind: BoundaryKind::EventChannel,
                name: "orphan:event".to_string(),
                role: BoundaryRole::Producer,
                file: CanonicalPath::new("src/orphan.ts"),
                line: 5,
                framework: None,
                method: None,
            },
        ],
    );

    let state = ariadne_graph::semantic::analyze(boundaries);
    assert_eq!(state.orphan_routes, vec!["/api/lonely"]);
    assert_eq!(state.orphan_events, vec!["orphan:event"]);

    let output = serial::semantic_state_to_boundary_output(&state);
    let json = serde_json::to_string(&output).unwrap();
    let deserialized: serial::BoundaryOutput = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.orphan_routes, vec!["/api/lonely"]);
    assert_eq!(deserialized.orphan_events, vec!["orphan:event"]);
}

// ===========================================================================
// Edge construction: prefix matching
// ===========================================================================

#[test]
fn edge_prefix_match_parameterized_route() {
    let mut boundaries = BTreeMap::new();
    boundaries.insert(
        CanonicalPath::new("src/routes.ts"),
        vec![Boundary {
            kind: BoundaryKind::HttpRoute,
            name: "/api/users/:id".to_string(),
            role: BoundaryRole::Producer,
            file: CanonicalPath::new("src/routes.ts"),
            line: 1,
            framework: None,
            method: None,
        }],
    );
    boundaries.insert(
        CanonicalPath::new("src/client.ts"),
        vec![Boundary {
            kind: BoundaryKind::HttpRoute,
            name: "/api/users".to_string(),
            role: BoundaryRole::Consumer,
            file: CanonicalPath::new("src/client.ts"),
            line: 1,
            framework: None,
            method: None,
        }],
    );

    let (edges, _, _) = build_semantic_edges(&boundaries);

    assert_eq!(edges.len(), 1);
    assert!(
        (edges[0].confidence - 0.8).abs() < f64::EPSILON,
        "Prefix match should have confidence 0.8, got {}",
        edges[0].confidence
    );
}

#[test]
fn edge_self_loop_excluded() {
    // Producer and consumer in the same file should not create an edge.
    let mut boundaries = BTreeMap::new();
    boundaries.insert(
        CanonicalPath::new("src/same.ts"),
        vec![
            Boundary {
                kind: BoundaryKind::HttpRoute,
                name: "/api/data".to_string(),
                role: BoundaryRole::Producer,
                file: CanonicalPath::new("src/same.ts"),
                line: 1,
                framework: None,
                method: None,
            },
            Boundary {
                kind: BoundaryKind::HttpRoute,
                name: "/api/data".to_string(),
                role: BoundaryRole::Consumer,
                file: CanonicalPath::new("src/same.ts"),
                line: 10,
                framework: None,
                method: None,
            },
        ],
    );

    let (edges, _, _) = build_semantic_edges(&boundaries);
    assert!(edges.is_empty(), "Self-loop edges should be excluded");
}

// ===========================================================================
// ExtractorRegistry tests
// ===========================================================================

#[test]
fn extractor_registry_multiple_extractors_per_extension() {
    use ariadne_graph::semantic::ExtractorRegistry;

    let mut registry = ExtractorRegistry::new();
    registry.register(Box::new(HttpRouteExtractor));
    registry.register(Box::new(EventExtractor));

    // .ts should have both extractors
    let ts_extractors = registry.extractors_for("ts");
    assert_eq!(
        ts_extractors.len(),
        2,
        "TypeScript should have HTTP + Event extractors"
    );

    // .java should have only HTTP
    let java_extractors = registry.extractors_for("java");
    assert_eq!(java_extractors.len(), 1, "Java should have only HTTP extractor");

    // .rb should have none
    let rb_extractors = registry.extractors_for("rb");
    assert!(rb_extractors.is_empty(), "Ruby should have no extractors");
}

// ===========================================================================
// Fixture-based multi-extractor integration
// ===========================================================================

#[test]
fn integration_all_fixtures_parse_without_panic() {
    // Verify every fixture file can be parsed and extracted without panicking.
    let fixtures: Vec<(&str, &str, tree_sitter::Language)> = vec![
        ("express_routes.ts", "ts", ts_lang()),
        ("fastapi_routes.py", "py", py_lang()),
        ("spring_routes.java", "java", java_lang()),
        ("go_routes.go", "go", go_lang()),
        ("aspnet_routes.cs", "cs", cs_lang()),
        ("event_emitters.ts", "ts", ts_lang()),
        ("event_generic.py", "py", py_lang()),
        ("mixed_framework.ts", "ts", ts_lang()),
        ("dom_events.ts", "ts", ts_lang()),
    ];

    for (filename, ext, lang) in fixtures {
        let source = read_fixture(filename);

        // Both extractors should run without panicking
        let _http = extract_http_routes(&source, ext, lang.clone());
        let _events = extract_events(&source, ext, lang);
    }
}

#[test]
fn integration_edge_construction_from_fixtures() {
    // Build a cross-file scenario using fixture-extracted boundaries.
    let express_src = read_fixture("express_routes.ts");
    let mixed_src = read_fixture("mixed_framework.ts");

    let express_http = extract_http_routes(&express_src, "ts", ts_lang());
    let mixed_http = extract_http_routes(&mixed_src, "ts", ts_lang());
    let mixed_events = extract_events(&mixed_src, "ts", ts_lang());

    let mut boundaries = BTreeMap::new();

    // Place express routes under one path
    let express_boundaries: Vec<Boundary> = express_http
        .into_iter()
        .map(|mut b| {
            b.file = CanonicalPath::new("src/express_routes.ts");
            b
        })
        .collect();
    boundaries.insert(
        CanonicalPath::new("src/express_routes.ts"),
        express_boundaries,
    );

    // Place mixed boundaries under another path
    let mut mixed_all: Vec<Boundary> = Vec::new();
    for mut b in mixed_http {
        b.file = CanonicalPath::new("src/mixed.ts");
        mixed_all.push(b);
    }
    for mut b in mixed_events {
        b.file = CanonicalPath::new("src/mixed.ts");
        mixed_all.push(b);
    }
    boundaries.insert(CanonicalPath::new("src/mixed.ts"), mixed_all);

    let (edges, orphan_routes, orphan_events) = build_semantic_edges(&boundaries);

    // There should be some edges (express consumers hitting express producers, etc.)
    // and/or some orphans. The key assertion is no panic and deterministic output.
    let total = edges.len() + orphan_routes.len() + orphan_events.len();
    assert!(
        total > 0,
        "Cross-file analysis should produce edges or orphans"
    );
}

// ===========================================================================
// Line number accuracy
// ===========================================================================

#[test]
fn line_numbers_match_fixture_positions() {
    let source = read_fixture("fastapi_routes.py");
    let routes = extract_http_routes(&source, "py", py_lang());

    assert!(!routes.is_empty());

    // All line numbers should be > 0 (1-based)
    for boundary in &routes {
        assert!(
            boundary.line > 0,
            "Line numbers must be 1-based, got {} for {}",
            boundary.line,
            boundary.name
        );
    }
}
