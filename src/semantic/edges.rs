use std::collections::BTreeMap;

use crate::model::semantic::{Boundary, BoundaryKind, BoundaryRole, SemanticEdge};
use crate::model::types::CanonicalPath;

/// Build semantic edges from boundary data.
///
/// Connects files that share the same boundary names (e.g., a file producing
/// an HTTP route and a file consuming it). Returns edges, orphan routes, and
/// orphan events.
///
/// Matching rules (D-108):
/// - Exact match on (kind, name): confidence 1.0
/// - Prefix match (HttpRoute only): confidence 0.8
///
/// Self-loops are excluded (D-106): edges where producer_file == consumer_file
/// are skipped.
pub fn build_semantic_edges(
    boundaries: &BTreeMap<CanonicalPath, Vec<Boundary>>,
) -> (Vec<SemanticEdge>, Vec<String>, Vec<String>) {
    // Step 1: Build producer and consumer indexes.
    // Key: (BoundaryKind, name), Value: Vec<(file, &Boundary)>
    let mut producers: BTreeMap<(BoundaryKind, String), Vec<(CanonicalPath, &Boundary)>> =
        BTreeMap::new();
    let mut consumers: BTreeMap<(BoundaryKind, String), Vec<(CanonicalPath, &Boundary)>> =
        BTreeMap::new();

    for (path, file_boundaries) in boundaries {
        for b in file_boundaries {
            let key = (b.kind, b.name.clone());
            match b.role {
                BoundaryRole::Producer => {
                    producers
                        .entry(key)
                        .or_default()
                        .push((path.clone(), b));
                }
                BoundaryRole::Consumer => {
                    consumers
                        .entry(key)
                        .or_default()
                        .push((path.clone(), b));
                }
                BoundaryRole::Both => {
                    producers
                        .entry(key.clone())
                        .or_default()
                        .push((path.clone(), b));
                    consumers
                        .entry(key)
                        .or_default()
                        .push((path.clone(), b));
                }
            }
        }
    }

    // Step 2: Build edges from producer/consumer matches.
    // Use a BTreeMap for deduplication: (from, to, name) -> (boundary_kind, best confidence)
    let mut edge_map: BTreeMap<(CanonicalPath, CanonicalPath, String), (BoundaryKind, f64)> =
        BTreeMap::new();

    // Track which producer names have at least one consumer match (for orphan detection).
    let mut matched_producer_names: BTreeMap<(BoundaryKind, String), bool> = BTreeMap::new();

    // Initialize all producer names as unmatched.
    for key in producers.keys() {
        matched_producer_names.insert(key.clone(), false);
    }

    // Step 2a: Exact matches (confidence 1.0)
    for (key, prod_list) in &producers {
        if let Some(cons_list) = consumers.get(key) {
            matched_producer_names.insert(key.clone(), true);

            for (prod_file, _) in prod_list {
                for (cons_file, _) in cons_list {
                    // D-106: exclude self-loops
                    if prod_file == cons_file {
                        continue;
                    }
                    let edge_key =
                        (prod_file.clone(), cons_file.clone(), key.1.clone());
                    let entry = edge_map.entry(edge_key).or_insert((key.0, 0.0));
                    if 1.0 > entry.1 {
                        entry.1 = 1.0;
                    }
                }
            }
        }
    }

    // Step 2b: Prefix matches (HttpRoute only, confidence 0.8)
    let http_producers: Vec<_> = producers
        .iter()
        .filter(|(k, _)| k.0 == BoundaryKind::HttpRoute)
        .collect();
    let http_consumers: Vec<_> = consumers
        .iter()
        .filter(|(k, _)| k.0 == BoundaryKind::HttpRoute)
        .collect();

    for (prod_key, prod_list) in &http_producers {
        for (cons_key, cons_list) in &http_consumers {
            // Skip exact matches (already handled above)
            if prod_key.1 == cons_key.1 {
                continue;
            }

            // Check prefix match in either direction
            let is_prefix = prod_key.1.starts_with(&cons_key.1)
                || cons_key.1.starts_with(&prod_key.1);

            if !is_prefix {
                continue;
            }

            matched_producer_names.insert((*prod_key).clone(), true);

            for (prod_file, _) in *prod_list {
                for (cons_file, _) in *cons_list {
                    // D-106: exclude self-loops
                    if prod_file == cons_file {
                        continue;
                    }
                    // Use producer name for the edge name
                    let edge_key =
                        (prod_file.clone(), cons_file.clone(), prod_key.1.clone());
                    let entry =
                        edge_map.entry(edge_key).or_insert((BoundaryKind::HttpRoute, 0.0));
                    if 0.8 > entry.1 {
                        entry.1 = 0.8;
                    }
                }
            }
        }
    }

    // Step 3: Convert edge_map to sorted Vec<SemanticEdge> (BTreeMap is already sorted)
    let edges: Vec<SemanticEdge> = edge_map
        .into_iter()
        .map(|((from, to, name), (boundary_kind, confidence))| SemanticEdge {
            from,
            to,
            boundary_kind,
            name,
            confidence,
        })
        .collect();

    // Step 4: Collect orphans — producer names with no matching consumer
    let mut orphan_routes: Vec<String> = Vec::new();
    let mut orphan_events: Vec<String> = Vec::new();

    for ((kind, name), matched) in &matched_producer_names {
        if !matched {
            match kind {
                BoundaryKind::HttpRoute => orphan_routes.push(name.clone()),
                BoundaryKind::EventChannel => orphan_events.push(name.clone()),
            }
        }
    }

    // Deduplicate orphan names (multiple producers can share the same name)
    orphan_routes.dedup();
    orphan_events.dedup();

    (edges, orphan_routes, orphan_events)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_boundary(
        kind: BoundaryKind,
        name: &str,
        role: BoundaryRole,
        file: &str,
    ) -> Boundary {
        Boundary {
            kind,
            name: name.to_string(),
            role,
            file: CanonicalPath::new(file),
            line: 1,
            framework: None,
            method: None,
        }
    }

    #[test]
    fn exact_match_creates_edge_with_confidence_1() {
        let mut boundaries = BTreeMap::new();
        boundaries.insert(
            CanonicalPath::new("src/routes.ts"),
            vec![make_boundary(
                BoundaryKind::HttpRoute,
                "/api/users",
                BoundaryRole::Producer,
                "src/routes.ts",
            )],
        );
        boundaries.insert(
            CanonicalPath::new("src/client.ts"),
            vec![make_boundary(
                BoundaryKind::HttpRoute,
                "/api/users",
                BoundaryRole::Consumer,
                "src/client.ts",
            )],
        );

        let (edges, orphan_routes, orphan_events) = build_semantic_edges(&boundaries);

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].from, CanonicalPath::new("src/routes.ts"));
        assert_eq!(edges[0].to, CanonicalPath::new("src/client.ts"));
        assert_eq!(edges[0].confidence, 1.0);
        assert!(orphan_routes.is_empty());
        assert!(orphan_events.is_empty());
    }

    #[test]
    fn self_loop_excluded() {
        let mut boundaries = BTreeMap::new();
        boundaries.insert(
            CanonicalPath::new("src/handler.ts"),
            vec![
                make_boundary(
                    BoundaryKind::HttpRoute,
                    "/api/users",
                    BoundaryRole::Producer,
                    "src/handler.ts",
                ),
                make_boundary(
                    BoundaryKind::HttpRoute,
                    "/api/users",
                    BoundaryRole::Consumer,
                    "src/handler.ts",
                ),
            ],
        );

        let (edges, _, _) = build_semantic_edges(&boundaries);
        assert!(edges.is_empty());
    }

    #[test]
    fn prefix_match_http_route_confidence_08() {
        let mut boundaries = BTreeMap::new();
        boundaries.insert(
            CanonicalPath::new("src/routes.ts"),
            vec![make_boundary(
                BoundaryKind::HttpRoute,
                "/api/users/:id",
                BoundaryRole::Producer,
                "src/routes.ts",
            )],
        );
        boundaries.insert(
            CanonicalPath::new("src/client.ts"),
            vec![make_boundary(
                BoundaryKind::HttpRoute,
                "/api/users",
                BoundaryRole::Consumer,
                "src/client.ts",
            )],
        );

        let (edges, orphan_routes, _) = build_semantic_edges(&boundaries);

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].confidence, 0.8);
        assert!(orphan_routes.is_empty());
    }

    #[test]
    fn no_prefix_match_for_event_channel() {
        let mut boundaries = BTreeMap::new();
        boundaries.insert(
            CanonicalPath::new("src/emitter.ts"),
            vec![make_boundary(
                BoundaryKind::EventChannel,
                "user.created",
                BoundaryRole::Producer,
                "src/emitter.ts",
            )],
        );
        boundaries.insert(
            CanonicalPath::new("src/listener.ts"),
            vec![make_boundary(
                BoundaryKind::EventChannel,
                "user",
                BoundaryRole::Consumer,
                "src/listener.ts",
            )],
        );

        let (edges, _, orphan_events) = build_semantic_edges(&boundaries);

        // No edge: "user" is not an exact match for "user.created", and
        // prefix matching is HttpRoute only.
        assert!(edges.is_empty());
        assert_eq!(orphan_events, vec!["user.created"]);
    }

    #[test]
    fn orphan_routes_and_events_collected() {
        let mut boundaries = BTreeMap::new();
        boundaries.insert(
            CanonicalPath::new("src/routes.ts"),
            vec![make_boundary(
                BoundaryKind::HttpRoute,
                "/api/orphan",
                BoundaryRole::Producer,
                "src/routes.ts",
            )],
        );
        boundaries.insert(
            CanonicalPath::new("src/events.ts"),
            vec![make_boundary(
                BoundaryKind::EventChannel,
                "orphan.event",
                BoundaryRole::Producer,
                "src/events.ts",
            )],
        );

        let (edges, orphan_routes, orphan_events) = build_semantic_edges(&boundaries);

        assert!(edges.is_empty());
        assert_eq!(orphan_routes, vec!["/api/orphan"]);
        assert_eq!(orphan_events, vec!["orphan.event"]);
    }

    #[test]
    fn dedup_keeps_highest_confidence() {
        let mut boundaries = BTreeMap::new();
        // Producer with exact name
        boundaries.insert(
            CanonicalPath::new("src/routes.ts"),
            vec![
                make_boundary(
                    BoundaryKind::HttpRoute,
                    "/api/users",
                    BoundaryRole::Producer,
                    "src/routes.ts",
                ),
                // Also a producer for a longer route that prefix-matches
                make_boundary(
                    BoundaryKind::HttpRoute,
                    "/api/users/:id",
                    BoundaryRole::Producer,
                    "src/routes.ts",
                ),
            ],
        );
        boundaries.insert(
            CanonicalPath::new("src/client.ts"),
            vec![make_boundary(
                BoundaryKind::HttpRoute,
                "/api/users",
                BoundaryRole::Consumer,
                "src/client.ts",
            )],
        );

        let (edges, _, _) = build_semantic_edges(&boundaries);

        // Should have two edges:
        // 1. routes.ts -> client.ts via "/api/users" (exact, 1.0)
        // 2. routes.ts -> client.ts via "/api/users/:id" (prefix, 0.8)
        assert_eq!(edges.len(), 2);

        let exact = edges.iter().find(|e| e.name == "/api/users").unwrap();
        assert_eq!(exact.confidence, 1.0);

        let prefix = edges.iter().find(|e| e.name == "/api/users/:id").unwrap();
        assert_eq!(prefix.confidence, 0.8);
    }

    #[test]
    fn both_role_acts_as_producer_and_consumer() {
        let mut boundaries = BTreeMap::new();
        boundaries.insert(
            CanonicalPath::new("src/gateway.ts"),
            vec![make_boundary(
                BoundaryKind::HttpRoute,
                "/api/data",
                BoundaryRole::Both,
                "src/gateway.ts",
            )],
        );
        boundaries.insert(
            CanonicalPath::new("src/client.ts"),
            vec![make_boundary(
                BoundaryKind::HttpRoute,
                "/api/data",
                BoundaryRole::Consumer,
                "src/client.ts",
            )],
        );

        let (edges, _, _) = build_semantic_edges(&boundaries);

        // gateway.ts (Both=Producer) -> client.ts (Consumer): exact match
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].from, CanonicalPath::new("src/gateway.ts"));
        assert_eq!(edges[0].to, CanonicalPath::new("src/client.ts"));
    }

    #[test]
    fn empty_boundaries_returns_empty() {
        let boundaries = BTreeMap::new();
        let (edges, orphan_routes, orphan_events) = build_semantic_edges(&boundaries);
        assert!(edges.is_empty());
        assert!(orphan_routes.is_empty());
        assert!(orphan_events.is_empty());
    }

    #[test]
    fn edges_are_sorted_deterministically() {
        let mut boundaries = BTreeMap::new();
        boundaries.insert(
            CanonicalPath::new("src/b.ts"),
            vec![make_boundary(
                BoundaryKind::HttpRoute,
                "/api/z",
                BoundaryRole::Producer,
                "src/b.ts",
            )],
        );
        boundaries.insert(
            CanonicalPath::new("src/a.ts"),
            vec![
                make_boundary(
                    BoundaryKind::HttpRoute,
                    "/api/z",
                    BoundaryRole::Consumer,
                    "src/a.ts",
                ),
                make_boundary(
                    BoundaryKind::HttpRoute,
                    "/api/a",
                    BoundaryRole::Producer,
                    "src/a.ts",
                ),
            ],
        );
        boundaries.insert(
            CanonicalPath::new("src/c.ts"),
            vec![make_boundary(
                BoundaryKind::HttpRoute,
                "/api/a",
                BoundaryRole::Consumer,
                "src/c.ts",
            )],
        );

        let (edges, _, _) = build_semantic_edges(&boundaries);

        assert_eq!(edges.len(), 2);
        // Sorted by (from, to, name)
        assert_eq!(edges[0].from, CanonicalPath::new("src/a.ts"));
        assert_eq!(edges[1].from, CanonicalPath::new("src/b.ts"));
    }
}
