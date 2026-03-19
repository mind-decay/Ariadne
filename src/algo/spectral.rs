use std::collections::BTreeMap;

use serde::Serialize;

use crate::algo::round4;
use crate::model::{CanonicalPath, ProjectGraph};

/// Result of spectral analysis on the dependency graph.
#[derive(Debug, Clone, Serialize)]
pub struct SpectralResult {
    /// Second-smallest eigenvalue of the graph Laplacian (algebraic connectivity).
    /// Low λ₂ → graph is close to splitting. High λ₂ → tightly connected.
    pub algebraic_connectivity: f64,
    /// Normalized connectivity: λ₂ / λ_max. Higher = more monolithic.
    pub monolith_score: f64,
    /// Natural bisection of the graph based on the Fiedler vector.
    pub natural_partitions: Vec<SpectralPartition>,
}

/// One partition from the Fiedler vector bisection.
#[derive(Debug, Clone, Serialize)]
pub struct SpectralPartition {
    pub partition_id: u32,
    pub files: Vec<CanonicalPath>,
}

/// Compute spectral analysis: algebraic connectivity, monolith score, Fiedler bisection.
///
/// Uses power iteration with deflation on the graph Laplacian (L = D - A) where
/// the graph is treated as undirected. No external sparse matrix library needed.
pub fn spectral_analysis(
    graph: &ProjectGraph,
    max_iterations: u32,
    tolerance: f64,
) -> SpectralResult {
    let nodes: Vec<&CanonicalPath> = graph.nodes.keys().collect();
    let n = nodes.len();

    if n <= 1 {
        return trivial_result(nodes);
    }

    // Build index mapping
    let idx: BTreeMap<&CanonicalPath, usize> =
        nodes.iter().enumerate().map(|(i, p)| (*p, i)).collect();

    // Build undirected adjacency
    let mut adj: Vec<Vec<usize>> = vec![vec![]; n];
    for edge in &graph.edges {
        if let (Some(&i), Some(&j)) = (idx.get(&edge.from), idx.get(&edge.to)) {
            if i != j {
                adj[i].push(j);
                adj[j].push(i);
            }
        }
    }
    for neighbors in &mut adj {
        neighbors.sort();
        neighbors.dedup();
    }

    let degree: Vec<f64> = adj.iter().map(|a| a.len() as f64).collect();

    // Laplacian matrix-vector product: L*x = D*x - A*x
    let laplacian_mv = |x: &[f64]| -> Vec<f64> {
        let mut result = vec![0.0; n];
        for i in 0..n {
            result[i] = degree[i] * x[i];
            for &j in &adj[i] {
                result[i] -= x[j];
            }
        }
        result
    };

    // Step 1: Find λ_max of L using power iteration
    let lambda_max = {
        let mut v = vec![0.0; n];
        for (i, vi) in v.iter_mut().enumerate().take(n) {
            *vi = if i % 2 == 0 { 1.0 } else { -1.0 };
        }
        normalize(&mut v);

        let mut eigenvalue = 0.0;
        for _ in 0..max_iterations {
            let lv = laplacian_mv(&v);
            let new_eigenvalue = dot(&v, &lv);
            let nrm = l2_norm(&lv);
            if nrm < 1e-15 {
                break;
            }
            v = lv;
            for x in &mut v {
                *x /= nrm;
            }
            if (new_eigenvalue - eigenvalue).abs() < tolerance {
                eigenvalue = new_eigenvalue;
                break;
            }
            eigenvalue = new_eigenvalue;
        }
        eigenvalue.max(0.0)
    };

    if lambda_max < 1e-10 {
        // Graph has no edges
        return trivial_result(nodes);
    }

    // Step 2: Find Fiedler vector using inverse power iteration on L
    // with deflation to remove the constant eigenvector (λ₁ = 0).
    //
    // We use the shifted matrix M = λ_max*I - L, which has eigenvalues (λ_max - λ_i).
    // The largest eigenvalue of M is λ_max (corresponding to the constant vector).
    // The second largest is λ_max - λ₂.
    // Power iteration on M with deflation of the constant vector gives us this.
    let inv_sqrt_n = 1.0 / (n as f64).sqrt();
    let constant_vec: Vec<f64> = vec![inv_sqrt_n; n];

    // M * x = λ_max * x - L * x
    let shifted_mv = |x: &[f64]| -> Vec<f64> {
        let lx = laplacian_mv(x);
        let mut result = vec![0.0; n];
        for i in 0..n {
            result[i] = lambda_max * x[i] - lx[i];
        }
        result
    };

    // Initialize with deterministic vector orthogonal to constant
    let mut fiedler = vec![0.0; n];
    for (i, fi) in fiedler.iter_mut().enumerate().take(n) {
        *fi = (i as f64) - (n as f64 - 1.0) / 2.0;
    }
    // Project out constant vector
    project_out(&mut fiedler, &constant_vec);
    normalize(&mut fiedler);

    let mut eigenvalue = 0.0;
    for _ in 0..max_iterations {
        let mut mv = shifted_mv(&fiedler);
        // Project out constant vector to stay in the orthogonal complement
        project_out(&mut mv, &constant_vec);

        let nrm = l2_norm(&mv);
        if nrm < 1e-15 {
            break;
        }

        let new_eigenvalue = dot(&fiedler, &mv);
        for x in &mut mv {
            *x /= nrm;
        }
        fiedler = mv;

        if (new_eigenvalue - eigenvalue).abs() < tolerance {
            eigenvalue = new_eigenvalue;
            break;
        }
        eigenvalue = new_eigenvalue;
    }

    // λ₂ = λ_max - eigenvalue_of_M
    let lambda2 = (lambda_max - eigenvalue).max(0.0);

    let monolith_score = if lambda_max > 1e-10 {
        round4(lambda2 / lambda_max)
    } else {
        0.0
    };

    // Apply sign convention (D-060): first node (lexicographically) is positive
    if fiedler[0] < 0.0 {
        for x in &mut fiedler {
            *x = -*x;
        }
    }

    // Partition by sign
    let mut part0 = Vec::new();
    let mut part1 = Vec::new();
    for (i, &val) in fiedler.iter().enumerate() {
        if val >= 0.0 {
            part0.push(nodes[i].clone());
        } else {
            part1.push(nodes[i].clone());
        }
    }

    SpectralResult {
        algebraic_connectivity: round4(lambda2),
        monolith_score,
        natural_partitions: vec![
            SpectralPartition {
                partition_id: 0,
                files: part0,
            },
            SpectralPartition {
                partition_id: 1,
                files: part1,
            },
        ],
    }
}

fn trivial_result(nodes: Vec<&CanonicalPath>) -> SpectralResult {
    SpectralResult {
        algebraic_connectivity: 0.0,
        monolith_score: 0.0,
        natural_partitions: vec![SpectralPartition {
            partition_id: 0,
            files: nodes.into_iter().cloned().collect(),
        }],
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn l2_norm(v: &[f64]) -> f64 {
    dot(v, v).sqrt()
}

fn normalize(v: &mut [f64]) {
    let nrm = l2_norm(v);
    if nrm > 1e-15 {
        for x in v.iter_mut() {
            *x /= nrm;
        }
    }
}

/// Project vector v onto the orthogonal complement of u (assumed normalized).
fn project_out(v: &mut [f64], u: &[f64]) {
    let proj = dot(v, u);
    for (vi, &ui) in v.iter_mut().zip(u) {
        *vi -= proj * ui;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn make_node(cluster: &str) -> Node {
        Node {
            file_type: FileType::Source,
            layer: ArchLayer::Unknown,
            arch_depth: 0,
            lines: 10,
            hash: ContentHash::new("0000000000000000".to_string()),
            exports: vec![],
            cluster: ClusterId::new(cluster),
        }
    }

    fn make_edge(from: &str, to: &str) -> Edge {
        Edge {
            from: CanonicalPath::new(from),
            to: CanonicalPath::new(to),
            edge_type: EdgeType::Imports,
            symbols: vec![],
        }
    }

    #[test]
    fn complete_graph_high_connectivity() {
        // K5: λ₂ = 5 (all eigenvalues of L for Kn are n, except λ₁ = 0)
        let names = ["a.ts", "b.ts", "c.ts", "d.ts", "e.ts"];
        let mut nodes = BTreeMap::new();
        for name in &names {
            nodes.insert(CanonicalPath::new(*name), make_node("root"));
        }
        let mut edges = Vec::new();
        for i in 0..names.len() {
            for j in (i + 1)..names.len() {
                edges.push(make_edge(names[i], names[j]));
            }
        }
        let graph = ProjectGraph { nodes, edges };
        let result = spectral_analysis(&graph, 200, 1e-6);

        assert!(
            result.algebraic_connectivity > 4.0,
            "K5 should have λ₂ = 5, got {}",
            result.algebraic_connectivity
        );
    }

    #[test]
    fn path_graph_low_connectivity() {
        let mut nodes = BTreeMap::new();
        for name in ["a.ts", "b.ts", "c.ts", "d.ts", "e.ts"] {
            nodes.insert(CanonicalPath::new(name), make_node("root"));
        }
        let edges = vec![
            make_edge("a.ts", "b.ts"),
            make_edge("b.ts", "c.ts"),
            make_edge("c.ts", "d.ts"),
            make_edge("d.ts", "e.ts"),
        ];
        let graph = ProjectGraph { nodes, edges };
        let result = spectral_analysis(&graph, 200, 1e-6);

        // P5: λ₂ ≈ 0.382 (2 - 2cos(π/5))
        assert!(
            result.algebraic_connectivity > 0.1 && result.algebraic_connectivity < 1.0,
            "P5 should have λ₂ ≈ 0.382, got {}",
            result.algebraic_connectivity
        );
    }

    #[test]
    fn complete_more_connected_than_path() {
        let names = ["a.ts", "b.ts", "c.ts", "d.ts", "e.ts"];

        let mut nodes_k = BTreeMap::new();
        for name in &names {
            nodes_k.insert(CanonicalPath::new(*name), make_node("root"));
        }
        let mut edges_k = Vec::new();
        for i in 0..names.len() {
            for j in (i + 1)..names.len() {
                edges_k.push(make_edge(names[i], names[j]));
            }
        }
        let graph_k = ProjectGraph {
            nodes: nodes_k,
            edges: edges_k,
        };
        let result_k = spectral_analysis(&graph_k, 200, 1e-6);

        let mut nodes_p = BTreeMap::new();
        for name in &names {
            nodes_p.insert(CanonicalPath::new(*name), make_node("root"));
        }
        let edges_p = vec![
            make_edge("a.ts", "b.ts"),
            make_edge("b.ts", "c.ts"),
            make_edge("c.ts", "d.ts"),
            make_edge("d.ts", "e.ts"),
        ];
        let graph_p = ProjectGraph {
            nodes: nodes_p,
            edges: edges_p,
        };
        let result_p = spectral_analysis(&graph_p, 200, 1e-6);

        assert!(
            result_k.algebraic_connectivity > result_p.algebraic_connectivity,
            "K5 λ₂ ({}) should be > P5 λ₂ ({})",
            result_k.algebraic_connectivity,
            result_p.algebraic_connectivity
        );
    }

    #[test]
    fn disconnected_components_zero_lambda2() {
        let mut nodes = BTreeMap::new();
        nodes.insert(CanonicalPath::new("a.ts"), make_node("c1"));
        nodes.insert(CanonicalPath::new("b.ts"), make_node("c1"));
        nodes.insert(CanonicalPath::new("x.ts"), make_node("c2"));
        nodes.insert(CanonicalPath::new("y.ts"), make_node("c2"));

        let edges = vec![make_edge("a.ts", "b.ts"), make_edge("x.ts", "y.ts")];
        let graph = ProjectGraph { nodes, edges };
        let result = spectral_analysis(&graph, 200, 1e-6);

        assert!(
            result.algebraic_connectivity < 0.01,
            "Disconnected graph should have λ₂ ≈ 0, got {}",
            result.algebraic_connectivity
        );
    }

    #[test]
    fn sign_convention_first_node_positive() {
        let mut nodes = BTreeMap::new();
        for name in ["a.ts", "b.ts", "c.ts", "d.ts"] {
            nodes.insert(CanonicalPath::new(name), make_node("root"));
        }
        let edges = vec![
            make_edge("a.ts", "b.ts"),
            make_edge("b.ts", "c.ts"),
            make_edge("c.ts", "d.ts"),
        ];
        let graph = ProjectGraph { nodes, edges };
        let result = spectral_analysis(&graph, 200, 1e-6);

        assert_eq!(result.natural_partitions.len(), 2);
        let part0_files: Vec<&str> = result.natural_partitions[0]
            .files
            .iter()
            .map(|f| f.as_str())
            .collect();
        assert!(
            part0_files.contains(&"a.ts"),
            "First node should be in partition 0"
        );
    }

    #[test]
    fn determinism() {
        let mut nodes = BTreeMap::new();
        for name in ["a.ts", "b.ts", "c.ts", "d.ts", "e.ts"] {
            nodes.insert(CanonicalPath::new(name), make_node("root"));
        }
        let edges = vec![
            make_edge("a.ts", "b.ts"),
            make_edge("b.ts", "c.ts"),
            make_edge("c.ts", "d.ts"),
            make_edge("d.ts", "e.ts"),
            make_edge("a.ts", "e.ts"),
        ];
        let graph = ProjectGraph { nodes, edges };

        let first = spectral_analysis(&graph, 200, 1e-6);
        for _ in 0..5 {
            let result = spectral_analysis(&graph, 200, 1e-6);
            assert_eq!(first.algebraic_connectivity, result.algebraic_connectivity);
            assert_eq!(first.monolith_score, result.monolith_score);
        }
    }

    #[test]
    fn single_node() {
        let mut nodes = BTreeMap::new();
        nodes.insert(CanonicalPath::new("a.ts"), make_node("root"));
        let graph = ProjectGraph {
            nodes,
            edges: vec![],
        };
        let result = spectral_analysis(&graph, 200, 1e-6);
        assert_eq!(result.algebraic_connectivity, 0.0);
    }

    #[test]
    fn two_clusters_connected_by_bridge() {
        // Two triangles connected by a single bridge edge → Fiedler separates them
        // {a,b,c} triangle + {x,y,z} triangle + bridge c→x
        let mut nodes = BTreeMap::new();
        for name in ["a.ts", "b.ts", "c.ts", "x.ts", "y.ts", "z.ts"] {
            nodes.insert(CanonicalPath::new(name), make_node("root"));
        }
        let edges = vec![
            // Triangle 1
            make_edge("a.ts", "b.ts"),
            make_edge("b.ts", "c.ts"),
            make_edge("a.ts", "c.ts"),
            // Triangle 2
            make_edge("x.ts", "y.ts"),
            make_edge("y.ts", "z.ts"),
            make_edge("x.ts", "z.ts"),
            // Bridge
            make_edge("c.ts", "x.ts"),
        ];
        let graph = ProjectGraph { nodes, edges };
        let result = spectral_analysis(&graph, 200, 1e-6);

        assert_eq!(result.natural_partitions.len(), 2);
        let p0: Vec<&str> = result.natural_partitions[0]
            .files
            .iter()
            .map(|f| f.as_str())
            .collect();
        let p1: Vec<&str> = result.natural_partitions[1]
            .files
            .iter()
            .map(|f| f.as_str())
            .collect();

        // {a,b,c} should be in one partition and {x,y,z} in the other
        let abc_in_p0 = p0.contains(&"a.ts") && p0.contains(&"b.ts") && p0.contains(&"c.ts");
        let abc_in_p1 = p1.contains(&"a.ts") && p1.contains(&"b.ts") && p1.contains(&"c.ts");
        assert!(
            abc_in_p0 || abc_in_p1,
            "Bridged triangles should be separated. P0: {:?}, P1: {:?}",
            p0,
            p1
        );
    }
}
