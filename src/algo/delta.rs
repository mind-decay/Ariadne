use std::collections::BTreeMap;

use crate::model::{CanonicalPath, ContentHash, Node};

/// Result of comparing old graph against current filesystem state.
#[derive(Debug)]
pub struct DeltaResult {
    pub changed: Vec<CanonicalPath>,
    pub added: Vec<CanonicalPath>,
    pub removed: Vec<CanonicalPath>,
    pub requires_full_recompute: bool,
}

/// Compare old graph nodes against current file hashes.
/// Pure function — no I/O. Depends on model/ only (D-033).
///
/// `requires_full_recompute` is true when >5% of files changed (architecture.md §Algorithms §6).
pub fn compute_delta(
    old_nodes: &BTreeMap<CanonicalPath, Node>,
    current_files: &[(CanonicalPath, ContentHash)],
) -> DeltaResult {
    let current_map: BTreeMap<&CanonicalPath, &ContentHash> = current_files
        .iter()
        .map(|(path, hash)| (path, hash))
        .collect();

    let mut changed = Vec::new();
    let mut added = Vec::new();
    let mut removed = Vec::new();

    // Find changed and removed files
    for (path, node) in old_nodes {
        match current_map.get(path) {
            Some(current_hash) => {
                if current_hash.as_str() != node.hash.as_str() {
                    changed.push(path.clone());
                }
            }
            None => {
                removed.push(path.clone());
            }
        }
    }

    // Find added files
    for (path, _) in current_files {
        if !old_nodes.contains_key(path) {
            added.push(path.clone());
        }
    }

    // Vectors are already sorted since we iterate BTreeMap keys (old_nodes)
    // and current_files may not be sorted, so sort added explicitly
    added.sort();

    let total_changes = changed.len() + added.len() + removed.len();
    let threshold = if old_nodes.is_empty() {
        0 // any changes on empty graph → full recompute
    } else {
        (old_nodes.len() as f64 * 0.05) as usize
    };
    let requires_full_recompute = total_changes > threshold;

    DeltaResult {
        changed,
        added,
        removed,
        requires_full_recompute,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ArchLayer, ClusterId, FileType, Node};

    fn make_node(hash: &str) -> Node {
        Node {
            file_type: FileType::Source,
            layer: ArchLayer::Unknown,
            arch_depth: 0,
            lines: 10,
            hash: ContentHash::new(hash.to_string()),
            exports: vec![],
            cluster: ClusterId::new("default"),
        }
    }

    fn make_old_graph(files: &[(&str, &str)]) -> BTreeMap<CanonicalPath, Node> {
        files
            .iter()
            .map(|(path, hash)| (CanonicalPath::new(*path), make_node(hash)))
            .collect()
    }

    fn make_current(files: &[(&str, &str)]) -> Vec<(CanonicalPath, ContentHash)> {
        files
            .iter()
            .map(|(path, hash)| {
                (
                    CanonicalPath::new(*path),
                    ContentHash::new(hash.to_string()),
                )
            })
            .collect()
    }

    #[test]
    fn no_changes() {
        let old = make_old_graph(&[("a.ts", "aaa"), ("b.ts", "bbb")]);
        let current = make_current(&[("a.ts", "aaa"), ("b.ts", "bbb")]);
        let result = compute_delta(&old, &current);
        assert!(result.changed.is_empty());
        assert!(result.added.is_empty());
        assert!(result.removed.is_empty());
        assert!(!result.requires_full_recompute);
    }

    #[test]
    fn one_file_changed() {
        // 21 files total, 1 changed = ~4.8% < 5% → no full recompute
        let mut files: Vec<(&str, &str)> = Vec::new();
        let names: Vec<String> = (0..21).map(|i| format!("f{}.ts", i)).collect();
        for name in &names {
            files.push((name.as_str(), "same_hash"));
        }
        let old = make_old_graph(&files);
        let mut current = make_current(&files);
        current[0].1 = ContentHash::new("different".to_string());

        let result = compute_delta(&old, &current);
        assert_eq!(result.changed.len(), 1);
        assert!(result.added.is_empty());
        assert!(result.removed.is_empty());
        assert!(!result.requires_full_recompute);
    }

    #[test]
    fn files_added() {
        let old = make_old_graph(&[("a.ts", "aaa")]);
        let current = make_current(&[("a.ts", "aaa"), ("b.ts", "bbb")]);
        let result = compute_delta(&old, &current);
        assert!(result.changed.is_empty());
        assert_eq!(result.added.len(), 1);
        assert_eq!(result.added[0].as_str(), "b.ts");
        assert!(result.removed.is_empty());
        // 1 change out of 1 old node = 100% → full recompute
        assert!(result.requires_full_recompute);
    }

    #[test]
    fn files_removed() {
        let old = make_old_graph(&[("a.ts", "aaa"), ("b.ts", "bbb")]);
        let current = make_current(&[("a.ts", "aaa")]);
        let result = compute_delta(&old, &current);
        assert!(result.changed.is_empty());
        assert!(result.added.is_empty());
        assert_eq!(result.removed.len(), 1);
        assert_eq!(result.removed[0].as_str(), "b.ts");
    }

    #[test]
    fn threshold_triggers_full_recompute() {
        // 20 files, 2 changed = 10% > 5% → full recompute
        let mut old_files: Vec<(&str, &str)> = Vec::new();
        let names: Vec<String> = (0..20).map(|i| format!("f{:02}.ts", i)).collect();
        for name in &names {
            old_files.push((name.as_str(), "hash"));
        }
        let old = make_old_graph(&old_files);
        let mut current = make_current(&old_files);
        current[0].1 = ContentHash::new("changed1".to_string());
        current[1].1 = ContentHash::new("changed2".to_string());

        let result = compute_delta(&old, &current);
        assert_eq!(result.changed.len(), 2);
        assert!(result.requires_full_recompute);
    }

    #[test]
    fn empty_old_graph_all_added() {
        let old = BTreeMap::new();
        let current = make_current(&[("a.ts", "aaa"), ("b.ts", "bbb")]);
        let result = compute_delta(&old, &current);
        assert!(result.changed.is_empty());
        assert_eq!(result.added.len(), 2);
        assert!(result.removed.is_empty());
        assert!(result.requires_full_recompute);
    }

    #[test]
    fn empty_current_all_removed() {
        let old = make_old_graph(&[("a.ts", "aaa"), ("b.ts", "bbb")]);
        let current = Vec::new();
        let result = compute_delta(&old, &current);
        assert!(result.changed.is_empty());
        assert!(result.added.is_empty());
        assert_eq!(result.removed.len(), 2);
        assert!(result.requires_full_recompute);
    }

    #[test]
    fn results_sorted() {
        let old = make_old_graph(&[("z.ts", "aaa"), ("a.ts", "bbb")]);
        let current = make_current(&[("m.ts", "mmm"), ("c.ts", "ccc")]);
        let result = compute_delta(&old, &current);

        // removed should be sorted
        let removed_strs: Vec<&str> = result.removed.iter().map(|p| p.as_str()).collect();
        assert_eq!(removed_strs, vec!["a.ts", "z.ts"]);

        // added should be sorted
        let added_strs: Vec<&str> = result.added.iter().map(|p| p.as_str()).collect();
        assert_eq!(added_strs, vec!["c.ts", "m.ts"]);
    }
}
