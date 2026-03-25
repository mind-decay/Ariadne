//! MCP prompt handlers for Ariadne.
//!
//! Pure functions returning rmcp prompt types. Each prompt provides
//! structured graph context to help LLMs reason about the codebase.

use std::collections::HashMap;

use rmcp::model::{
    GetPromptResult, ListPromptsResult, Prompt, PromptArgument, PromptMessage, PromptMessageRole,
};

use crate::algo::reading_order::compute_reading_order;
use crate::analysis::smells::detect_smells;
use crate::mcp::state::GraphState;
use crate::model::CanonicalPath;

/// List all available Ariadne prompts.
pub fn list_prompts_impl() -> ListPromptsResult {
    let prompts = vec![
        Prompt::new(
            "explore-area",
            Some("Explore an area of the codebase with full graph context"),
            Some(vec![
                PromptArgument::new("path")
                    .with_description("File or directory path to explore")
                    .with_required(true),
            ]),
        ),
        Prompt::new(
            "review-impact",
            Some("Analyze the impact of changes to specific files"),
            Some(vec![
                PromptArgument::new("paths")
                    .with_description("Comma-separated file paths to analyze")
                    .with_required(true),
            ]),
        ),
        Prompt::new(
            "find-refactoring",
            Some("Find refactoring opportunities in the codebase"),
            Some(vec![
                PromptArgument::new("scope")
                    .with_description("Optional cluster or directory to focus on (default: entire project)")
                    .with_required(false),
            ]),
        ),
        Prompt::new(
            "understand-module",
            Some("Understand a module via reading order and dependency context"),
            Some(vec![
                PromptArgument::new("module")
                    .with_description("Module or cluster name to understand")
                    .with_required(true),
            ]),
        ),
    ];

    ListPromptsResult::with_all_items(prompts)
}

/// Get a specific prompt with graph-enriched content.
///
/// Returns `GetPromptResult` with structured messages containing dependency
/// graph data relevant to the requested prompt.
pub fn get_prompt_impl(
    name: &str,
    args: Option<HashMap<String, String>>,
    state: &GraphState,
) -> GetPromptResult {
    match name {
        "explore-area" => prompt_explore_area(args, state),
        "review-impact" => prompt_review_impact(args, state),
        "find-refactoring" => prompt_find_refactoring(args, state),
        "understand-module" => prompt_understand_module(args, state),
        _ => GetPromptResult::new(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            format!("Unknown prompt: {name}. Available: explore-area, review-impact, find-refactoring, understand-module"),
        )]),
    }
}

// --- Prompt implementations ---

fn prompt_explore_area(
    args: Option<HashMap<String, String>>,
    state: &GraphState,
) -> GetPromptResult {
    let path = match args.as_ref().and_then(|a| a.get("path")) {
        Some(p) => p.clone(),
        None => {
            return error_prompt("explore-area requires 'path' argument");
        }
    };

    let cp = CanonicalPath::new(&path);
    let mut sections = Vec::new();

    // File info
    if let Some(node) = state.graph.nodes.get(&cp) {
        let centrality = state.stats.centrality.get(path.as_str()).copied().unwrap_or(0.0);
        let importance = state.combined_importance.get(&cp).copied().unwrap_or(0.0);
        sections.push(format!(
            "## File: {path}\n\
             - Type: {}\n\
             - Layer: {} (depth {})\n\
             - Lines: {}\n\
             - Cluster: {}\n\
             - Centrality: {centrality:.3}\n\
             - Combined importance: {importance:.3}",
            node.file_type.as_str(),
            node.layer.as_str(),
            node.arch_depth,
            node.lines,
            node.cluster.as_str(),
        ));
    } else {
        // Try matching as a directory prefix for cluster-like exploration
        let matching: Vec<&CanonicalPath> = state
            .graph
            .nodes
            .keys()
            .filter(|p| p.as_str().starts_with(&path))
            .collect();
        if matching.is_empty() {
            return error_prompt(&format!("Path '{path}' not found in graph"));
        }
        sections.push(format!(
            "## Directory: {path}\n- Files matching prefix: {}",
            matching.len()
        ));
    }

    // Dependencies
    let incoming = state.reverse_index.get(&cp).map(|e| e.len()).unwrap_or(0);
    let outgoing = state.forward_index.get(&cp).map(|e| e.len()).unwrap_or(0);
    sections.push(format!(
        "## Dependencies\n- Incoming: {incoming}\n- Outgoing: {outgoing}"
    ));

    // Outgoing details (up to 20)
    if let Some(edges) = state.forward_index.get(&cp) {
        let detail: Vec<String> = edges
            .iter()
            .take(20)
            .map(|e| format!("  - {} ({})", e.to.as_str(), e.edge_type.as_str()))
            .collect();
        if !detail.is_empty() {
            sections.push(format!("### Imports\n{}", detail.join("\n")));
        }
    }

    // Incoming details (up to 20)
    if let Some(edges) = state.reverse_index.get(&cp) {
        let detail: Vec<String> = edges
            .iter()
            .take(20)
            .map(|e| format!("  - {} ({})", e.from.as_str(), e.edge_type.as_str()))
            .collect();
        if !detail.is_empty() {
            sections.push(format!("### Imported by\n{}", detail.join("\n")));
        }
    }

    let text = format!(
        "I want to explore this area of the codebase. Here is structural context from the dependency graph:\n\n{}",
        sections.join("\n\n")
    );

    GetPromptResult::new(vec![PromptMessage::new_text(
        PromptMessageRole::User,
        text,
    )])
    .with_description("Explore an area with dependency graph context")
}

fn prompt_review_impact(
    args: Option<HashMap<String, String>>,
    state: &GraphState,
) -> GetPromptResult {
    let paths_str = match args.as_ref().and_then(|a| a.get("paths")) {
        Some(p) => p.clone(),
        None => {
            return error_prompt("review-impact requires 'paths' argument");
        }
    };

    let paths: Vec<&str> = paths_str.split(',').map(|s| s.trim()).collect();
    let mut sections = Vec::new();

    sections.push(format!("## Changed files ({})", paths.len()));

    for path in &paths {
        let cp = CanonicalPath::new(*path);

        let exists = state.graph.nodes.contains_key(&cp);
        let incoming = state.reverse_index.get(&cp).map(|e| e.len()).unwrap_or(0);
        let outgoing = state.forward_index.get(&cp).map(|e| e.len()).unwrap_or(0);
        let centrality = state.stats.centrality.get(*path).copied().unwrap_or(0.0);

        if exists {
            sections.push(format!(
                "### {path}\n\
                 - In graph: yes\n\
                 - Centrality: {centrality:.3}\n\
                 - Depended on by: {incoming} files\n\
                 - Depends on: {outgoing} files"
            ));
        } else {
            sections.push(format!(
                "### {path}\n- In graph: no (new or renamed file)"
            ));
        }
    }

    // Compute combined blast radius
    let mut affected: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for path in &paths {
        let cp = CanonicalPath::new(*path);
        if let Some(edges) = state.reverse_index.get(&cp) {
            for e in edges {
                affected.insert(e.from.as_str().to_string());
            }
        }
    }
    // Remove the changed files themselves
    for path in &paths {
        affected.remove(*path);
    }

    sections.push(format!(
        "## Direct dependents (blast radius): {} files",
        affected.len()
    ));
    if !affected.is_empty() {
        let listed: Vec<String> = affected.iter().take(30).cloned().collect();
        sections.push(listed.join("\n"));
        if affected.len() > 30 {
            sections.push(format!("... and {} more", affected.len() - 30));
        }
    }

    let text = format!(
        "Review the impact of these changes. Here is structural context:\n\n{}",
        sections.join("\n\n")
    );

    GetPromptResult::new(vec![PromptMessage::new_text(
        PromptMessageRole::User,
        text,
    )])
    .with_description("Change impact analysis with dependency context")
}

fn prompt_find_refactoring(
    args: Option<HashMap<String, String>>,
    state: &GraphState,
) -> GetPromptResult {
    let scope = args.as_ref().and_then(|a| a.get("scope")).cloned();
    let mut sections = Vec::new();

    // Detect smells
    let all_smells = detect_smells(
        &state.graph,
        &state.stats,
        &state.clusters,
        &state.cluster_metrics,
        state.temporal.as_ref(),
    );

    // Filter by scope if provided
    let smells: Vec<_> = if let Some(ref scope) = scope {
        all_smells
            .iter()
            .filter(|s| s.files.iter().any(|f| f.as_str().starts_with(scope.as_str())))
            .collect()
    } else {
        all_smells.iter().collect()
    };

    let scope_desc = scope.as_deref().unwrap_or("entire project");
    sections.push(format!(
        "## Refactoring analysis for: {scope_desc}\n- Smells detected: {}",
        smells.len()
    ));

    // Group by severity
    for severity in &["High", "Medium", "Low"] {
        let matching: Vec<_> = smells
            .iter()
            .filter(|s| format!("{:?}", s.severity) == *severity)
            .collect();
        if !matching.is_empty() {
            sections.push(format!("### {severity} severity ({} issues)", matching.len()));
            for s in matching.iter().take(10) {
                let file_list: Vec<&str> = s.files.iter().map(|f| f.as_str()).collect();
                sections.push(format!(
                    "- **{:?}**: {} (files: {})",
                    s.smell_type,
                    s.explanation,
                    file_list.join(", ")
                ));
            }
        }
    }

    // Top unstable clusters
    let mut unstable: Vec<_> = state
        .cluster_metrics
        .iter()
        .filter(|(id, _)| {
            scope
                .as_ref()
                .map(|s| id.as_str().starts_with(s.as_str()))
                .unwrap_or(true)
        })
        .filter(|(_, m)| m.distance > 0.5)
        .collect();
    unstable.sort_by(|a, b| {
        b.1.distance
            .partial_cmp(&a.1.distance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if !unstable.is_empty() {
        sections.push("### Clusters far from main sequence".to_string());
        for (id, m) in unstable.iter().take(5) {
            sections.push(format!(
                "- **{}**: D={:.2} (I={:.2}, A={:.2})",
                id.as_str(),
                m.distance,
                m.instability,
                m.abstractness,
            ));
        }
    }

    let text = format!(
        "Find refactoring opportunities based on structural analysis:\n\n{}",
        sections.join("\n\n")
    );

    GetPromptResult::new(vec![PromptMessage::new_text(
        PromptMessageRole::User,
        text,
    )])
    .with_description("Refactoring opportunities from structural analysis")
}

fn prompt_understand_module(
    args: Option<HashMap<String, String>>,
    state: &GraphState,
) -> GetPromptResult {
    let module = match args.as_ref().and_then(|a| a.get("module")) {
        Some(m) => m.clone(),
        None => {
            return error_prompt("understand-module requires 'module' argument");
        }
    };

    let cluster_id = crate::model::ClusterId::new(&module);
    let mut sections = Vec::new();

    // Find files in this module/cluster
    let files: Vec<CanonicalPath> = if let Some(cluster) = state.clusters.clusters.get(&cluster_id)
    {
        sections.push(format!(
            "## Module: {module}\n- Cluster with {} files",
            cluster.files.len()
        ));
        cluster.files.to_vec()
    } else {
        // Fallback: match as directory prefix
        let matching: Vec<CanonicalPath> = state
            .graph
            .nodes
            .keys()
            .filter(|p| p.as_str().starts_with(&module))
            .cloned()
            .collect();
        if matching.is_empty() {
            return error_prompt(&format!(
                "Module '{module}' not found as cluster or directory prefix"
            ));
        }
        sections.push(format!(
            "## Module: {module}\n- {} files matching prefix",
            matching.len()
        ));
        matching
    };

    // Cluster metrics
    if let Some(metrics) = state.cluster_metrics.get(&cluster_id) {
        sections.push(format!(
            "## Metrics\n\
             - Afferent coupling (Ca): {}\n\
             - Efferent coupling (Ce): {}\n\
             - Instability (I): {:.2}\n\
             - Abstractness (A): {:.2}\n\
             - Distance from main sequence (D): {:.2}",
            metrics.afferent_coupling,
            metrics.efferent_coupling,
            metrics.instability,
            metrics.abstractness,
            metrics.distance,
        ));
    }

    // Compute reading order
    let reading_result = compute_reading_order(&files, &state.graph, 2);
    if !reading_result.entries.is_empty() {
        sections.push(format!(
            "## Suggested reading order ({} files)",
            reading_result.entries.len()
        ));
        for (i, entry) in reading_result.entries.iter().enumerate() {
            sections.push(format!(
                "{}. {} — {} (layer {}, depth {})",
                i + 1,
                entry.path.as_str(),
                entry.reason,
                entry.layer,
                entry.depth,
            ));
        }
    }

    // External dependencies
    let mut external_deps: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();
    let file_set: std::collections::BTreeSet<&CanonicalPath> = files.iter().collect();
    for file in &files {
        if let Some(edges) = state.forward_index.get(file) {
            for e in edges {
                if !file_set.contains(&e.to) {
                    external_deps.insert(e.to.as_str().to_string());
                }
            }
        }
    }
    if !external_deps.is_empty() {
        sections.push(format!(
            "## External dependencies ({} files)",
            external_deps.len()
        ));
        for dep in external_deps.iter().take(20) {
            sections.push(format!("- {dep}"));
        }
    }

    let text = format!(
        "Help me understand this module. Here is structural context:\n\n{}",
        sections.join("\n\n")
    );

    GetPromptResult::new(vec![PromptMessage::new_text(
        PromptMessageRole::User,
        text,
    )])
    .with_description("Module understanding with reading order and dependency context")
}

/// Produce an error prompt result with a single user message.
fn error_prompt(message: &str) -> GetPromptResult {
    GetPromptResult::new(vec![PromptMessage::new_text(
        PromptMessageRole::User,
        message.to_string(),
    )])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn list_prompts_returns_four() {
        let result = list_prompts_impl();
        assert_eq!(result.prompts.len(), 4);

        let names: Vec<&str> = result.prompts.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"explore-area"));
        assert!(names.contains(&"review-impact"));
        assert!(names.contains(&"find-refactoring"));
        assert!(names.contains(&"understand-module"));
    }

    #[test]
    fn explore_area_requires_path() {
        let result = get_prompt_impl("explore-area", None, &make_empty_state());
        assert_eq!(result.messages.len(), 1);
        if let rmcp::model::PromptMessageContent::Text { ref text } = result.messages[0].content {
            assert!(text.contains("requires"));
        }
    }

    #[test]
    fn explore_area_with_valid_path_returns_context() {
        let state = make_populated_state();
        let args = Some(HashMap::from([("path".to_string(), "src/a.ts".to_string())]));
        let result = get_prompt_impl("explore-area", args, &state);
        assert_eq!(result.messages.len(), 1);
        if let rmcp::model::PromptMessageContent::Text { ref text } = result.messages[0].content {
            assert!(text.contains("src/a.ts"), "Should mention the file path");
            assert!(text.contains("Dependencies"), "Should include dependency section");
        } else {
            panic!("Expected Text content");
        }
    }

    #[test]
    fn review_impact_requires_paths() {
        let result = get_prompt_impl("review-impact", None, &make_empty_state());
        assert_eq!(result.messages.len(), 1);
        if let rmcp::model::PromptMessageContent::Text { ref text } = result.messages[0].content {
            assert!(text.contains("requires"));
        } else {
            panic!("Expected Text content");
        }
    }

    #[test]
    fn review_impact_with_valid_paths_returns_context() {
        let state = make_populated_state();
        let args = Some(HashMap::from([(
            "paths".to_string(),
            "src/a.ts, src/b.ts".to_string(),
        )]));
        let result = get_prompt_impl("review-impact", args, &state);
        assert_eq!(result.messages.len(), 1);
        if let rmcp::model::PromptMessageContent::Text { ref text } = result.messages[0].content {
            assert!(text.contains("Changed files"), "Should have changed files section");
            assert!(text.contains("blast radius"), "Should have blast radius section");
        } else {
            panic!("Expected Text content");
        }
    }

    #[test]
    fn find_refactoring_works_without_scope() {
        let state = make_populated_state();
        let result = get_prompt_impl("find-refactoring", None, &state);
        assert_eq!(result.messages.len(), 1);
        if let rmcp::model::PromptMessageContent::Text { ref text } = result.messages[0].content {
            assert!(
                text.contains("entire project"),
                "Without scope, should default to entire project"
            );
            assert!(
                text.contains("Refactoring analysis"),
                "Should contain analysis section"
            );
        } else {
            panic!("Expected Text content");
        }
    }

    #[test]
    fn find_refactoring_with_scope() {
        let state = make_populated_state();
        let args = Some(HashMap::from([("scope".to_string(), "src".to_string())]));
        let result = get_prompt_impl("find-refactoring", args, &state);
        assert_eq!(result.messages.len(), 1);
        if let rmcp::model::PromptMessageContent::Text { ref text } = result.messages[0].content {
            assert!(
                text.contains("src"),
                "Should mention the scope"
            );
        } else {
            panic!("Expected Text content");
        }
    }

    #[test]
    fn understand_module_requires_module_arg() {
        let result = get_prompt_impl("understand-module", None, &make_empty_state());
        assert_eq!(result.messages.len(), 1);
        if let rmcp::model::PromptMessageContent::Text { ref text } = result.messages[0].content {
            assert!(text.contains("requires"));
        } else {
            panic!("Expected Text content");
        }
    }

    #[test]
    fn understand_module_with_valid_cluster() {
        let state = make_populated_state();
        let args = Some(HashMap::from([("module".to_string(), "src".to_string())]));
        let result = get_prompt_impl("understand-module", args, &state);
        assert_eq!(result.messages.len(), 1);
        if let rmcp::model::PromptMessageContent::Text { ref text } = result.messages[0].content {
            assert!(text.contains("Module: src"), "Should identify the module");
            assert!(
                text.contains("reading order") || text.contains("Suggested reading order"),
                "Should include reading order"
            );
        } else {
            panic!("Expected Text content");
        }
    }

    #[test]
    fn understand_module_not_found() {
        let state = make_empty_state();
        let args = Some(HashMap::from([(
            "module".to_string(),
            "nonexistent".to_string(),
        )]));
        let result = get_prompt_impl("understand-module", args, &state);
        assert_eq!(result.messages.len(), 1);
        if let rmcp::model::PromptMessageContent::Text { ref text } = result.messages[0].content {
            assert!(text.contains("not found"), "Should report module not found");
        } else {
            panic!("Expected Text content");
        }
    }

    #[test]
    fn unknown_prompt_returns_message() {
        let result = get_prompt_impl("nonexistent", None, &make_empty_state());
        assert_eq!(result.messages.len(), 1);
        if let rmcp::model::PromptMessageContent::Text { ref text } = result.messages[0].content {
            assert!(text.contains("Unknown prompt"));
        }
    }

    /// Minimal empty GraphState for unit tests.
    fn make_empty_state() -> GraphState {
        let graph = crate::model::ProjectGraph {
            nodes: BTreeMap::new(),
            edges: Vec::new(),
        };
        let stats = crate::model::StatsOutput {
            version: 1,
            centrality: BTreeMap::new(),
            sccs: Vec::new(),
            layers: BTreeMap::new(),
            summary: crate::model::StatsSummary {
                max_depth: 0,
                avg_in_degree: 0.0,
                avg_out_degree: 0.0,
                bottleneck_files: Vec::new(),
                orphan_files: Vec::new(),
            },
        };
        let clusters = crate::model::ClusterMap {
            clusters: BTreeMap::new(),
        };

        GraphState::from_loaded_data(graph, stats, clusters, BTreeMap::new(), None)
    }

    /// GraphState with two files and one cluster for prompt content tests.
    fn make_populated_state() -> GraphState {
        use crate::model::*;

        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/a.ts"),
            Node {
                file_type: FileType::Source,
                layer: ArchLayer::Service,
                fsd_layer: None,
                arch_depth: 1,
                lines: 100,
                hash: ContentHash::new("abc123".to_string()),
                exports: vec![Symbol::new("foo")],
                cluster: ClusterId::new("src"),
                symbols: Vec::new(),
            },
        );
        nodes.insert(
            CanonicalPath::new("src/b.ts"),
            Node {
                file_type: FileType::Source,
                layer: ArchLayer::Service,
                fsd_layer: None,
                arch_depth: 0,
                lines: 50,
                hash: ContentHash::new("def456".to_string()),
                exports: vec![],
                cluster: ClusterId::new("src"),
                symbols: Vec::new(),
            },
        );

        let edges = vec![Edge {
            from: CanonicalPath::new("src/a.ts"),
            to: CanonicalPath::new("src/b.ts"),
            edge_type: EdgeType::Imports,
            symbols: vec![Symbol::new("bar")],
        }];

        let graph = ProjectGraph { nodes, edges };

        let stats = StatsOutput {
            version: 1,
            centrality: BTreeMap::new(),
            sccs: vec![],
            layers: BTreeMap::new(),
            summary: StatsSummary {
                max_depth: 1,
                avg_in_degree: 0.5,
                avg_out_degree: 0.5,
                bottleneck_files: vec![],
                orphan_files: vec![],
            },
        };

        let mut clusters_map = BTreeMap::new();
        clusters_map.insert(
            ClusterId::new("src"),
            Cluster {
                files: vec![
                    CanonicalPath::new("src/a.ts"),
                    CanonicalPath::new("src/b.ts"),
                ],
                file_count: 2,
                internal_edges: 1,
                external_edges: 0,
                cohesion: 1.0,
            },
        );
        let clusters = ClusterMap {
            clusters: clusters_map,
        };

        GraphState::from_loaded_data(graph, stats, clusters, BTreeMap::new(), None)
    }
}
