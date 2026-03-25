use std::collections::BTreeMap;

use crate::model::temporal::{ChurnMetrics, Hotspot};
use crate::model::types::CanonicalPath;

/// Compute hotspot scores for files with churn data.
/// Uses churn x LOC x blast_radius ranking, normalized to [0.0, 1.0].
pub fn compute_hotspots(
    churn: &BTreeMap<CanonicalPath, ChurnMetrics>,
    file_lines: &BTreeMap<CanonicalPath, u32>,
    blast_radius: &BTreeMap<CanonicalPath, usize>,
) -> Vec<Hotspot> {
    let n = churn.len();
    if n == 0 {
        return Vec::new();
    }

    // Collect per-file raw values
    let entries: Vec<(CanonicalPath, u32, u32, usize)> = churn
        .iter()
        .map(|(path, metrics)| {
            let churn_val = if metrics.commits_30d > 0 {
                metrics.commits_30d
            } else {
                metrics.commits_90d
            };
            let loc = file_lines.get(path).copied().unwrap_or(0);
            let br = blast_radius.get(path).copied().unwrap_or(0);
            (path.clone(), churn_val, loc, br)
        })
        .collect();

    // Compute ranks for each dimension (1 = highest value).
    // BTreeMap iteration is deterministic, so ties are broken by insertion order (path order).
    let churn_ranks = compute_ranks(&entries, |e| e.1 as u64);
    let loc_ranks = compute_ranks(&entries, |e| e.2 as u64);
    let br_ranks = compute_ranks(&entries, |e| e.3 as u64);

    let n_f64 = n as f64;

    let mut hotspots: Vec<Hotspot> = entries
        .iter()
        .enumerate()
        .map(|(i, (path, _, _, _))| {
            let cr = churn_ranks[i];
            let lr = loc_ranks[i];
            let brr = br_ranks[i];

            let score = if n == 1 {
                1.0
            } else {
                let s = (1.0 - (cr - 1) as f64 / n_f64)
                    * (1.0 - (lr - 1) as f64 / n_f64)
                    * (1.0 - (brr - 1) as f64 / n_f64);
                // Round to 4 decimal places (D-049)
                (s * 10_000.0).round() / 10_000.0
            };

            Hotspot {
                path: path.clone(),
                score,
                churn_rank: cr,
                loc_rank: lr,
                blast_radius_rank: brr,
            }
        })
        .collect();

    // Sort by score descending, then by path for determinism
    hotspots.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.path.as_str().cmp(b.path.as_str()))
    });

    hotspots
}

/// Compute ranks for entries based on a value extractor.
/// Rank 1 = highest value. Ties get the same rank (dense ranking).
fn compute_ranks<F>(entries: &[(CanonicalPath, u32, u32, usize)], value_fn: F) -> Vec<u32>
where
    F: Fn(&(CanonicalPath, u32, u32, usize)) -> u64,
{
    // Create index-value pairs and sort by value descending
    let mut indexed: Vec<(usize, u64)> = entries.iter().enumerate().map(|(i, e)| (i, value_fn(e))).collect();
    indexed.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let mut ranks = vec![0u32; entries.len()];
    let mut current_rank = 1u32;

    for i in 0..indexed.len() {
        if i > 0 && indexed[i].1 < indexed[i - 1].1 {
            current_rank = (i + 1) as u32;
        }
        ranks[indexed[i].0] = current_rank;
    }

    ranks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_churn(commits_30d: u32, commits_90d: u32) -> ChurnMetrics {
        ChurnMetrics {
            commits_30d,
            commits_90d,
            commits_1y: 0,
            lines_changed_30d: 0,
            lines_changed_90d: 0,
            authors_30d: 0,
            last_changed: None,
            top_authors: Vec::new(),
        }
    }

    #[test]
    fn test_empty_input() {
        let result = compute_hotspots(&BTreeMap::new(), &BTreeMap::new(), &BTreeMap::new());
        assert!(result.is_empty());
    }

    #[test]
    fn test_single_file_score_is_one() {
        let mut churn = BTreeMap::new();
        churn.insert(CanonicalPath::new("src/main.rs"), make_churn(10, 20));

        let mut lines = BTreeMap::new();
        lines.insert(CanonicalPath::new("src/main.rs"), 100u32);

        let mut br = BTreeMap::new();
        br.insert(CanonicalPath::new("src/main.rs"), 5usize);

        let result = compute_hotspots(&churn, &lines, &br);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].score, 1.0);
        assert_eq!(result[0].churn_rank, 1);
        assert_eq!(result[0].loc_rank, 1);
        assert_eq!(result[0].blast_radius_rank, 1);
    }

    #[test]
    fn test_known_rankings() {
        let mut churn = BTreeMap::new();
        churn.insert(CanonicalPath::new("a.rs"), make_churn(10, 0)); // highest churn
        churn.insert(CanonicalPath::new("b.rs"), make_churn(5, 0));
        churn.insert(CanonicalPath::new("c.rs"), make_churn(1, 0)); // lowest churn

        let mut lines = BTreeMap::new();
        lines.insert(CanonicalPath::new("a.rs"), 50u32); // lowest LOC
        lines.insert(CanonicalPath::new("b.rs"), 200u32); // highest LOC
        lines.insert(CanonicalPath::new("c.rs"), 100u32);

        let mut br = BTreeMap::new();
        br.insert(CanonicalPath::new("a.rs"), 3usize);
        br.insert(CanonicalPath::new("b.rs"), 10usize); // highest BR
        br.insert(CanonicalPath::new("c.rs"), 1usize); // lowest BR

        let result = compute_hotspots(&churn, &lines, &br);
        assert_eq!(result.len(), 3);

        // Verify a.rs: churn_rank=1, loc_rank=3, br_rank=2
        let a = result.iter().find(|h| h.path.as_str() == "a.rs").unwrap();
        assert_eq!(a.churn_rank, 1);
        assert_eq!(a.loc_rank, 3);
        assert_eq!(a.blast_radius_rank, 2);

        // Verify b.rs: churn_rank=2, loc_rank=1, br_rank=1
        let b = result.iter().find(|h| h.path.as_str() == "b.rs").unwrap();
        assert_eq!(b.churn_rank, 2);
        assert_eq!(b.loc_rank, 1);
        assert_eq!(b.blast_radius_rank, 1);

        // Verify c.rs: churn_rank=3, loc_rank=2, br_rank=3
        let c = result.iter().find(|h| h.path.as_str() == "c.rs").unwrap();
        assert_eq!(c.churn_rank, 3);
        assert_eq!(c.loc_rank, 2);
        assert_eq!(c.blast_radius_rank, 3);

        // b.rs should be top hotspot (rank 2,1,1 — best combined)
        assert_eq!(result[0].path.as_str(), "b.rs");
    }

    #[test]
    fn test_scores_in_valid_range() {
        let mut churn = BTreeMap::new();
        let mut lines = BTreeMap::new();
        let mut br = BTreeMap::new();

        for i in 0..10 {
            let path = CanonicalPath::new(format!("file{i}.rs"));
            churn.insert(path.clone(), make_churn(i + 1, 0));
            lines.insert(path.clone(), (i + 1) * 10);
            br.insert(path, (i + 1) as usize);
        }

        let result = compute_hotspots(&churn, &lines, &br);
        for h in &result {
            assert!(
                h.score >= 0.0 && h.score <= 1.0,
                "Score {} out of range for {}",
                h.score,
                h.path.as_str()
            );
        }
    }

    #[test]
    fn test_rounding_to_4_decimal_places() {
        let mut churn = BTreeMap::new();
        let mut lines = BTreeMap::new();
        let mut br = BTreeMap::new();

        for i in 0..5 {
            let path = CanonicalPath::new(format!("f{i}.rs"));
            churn.insert(path.clone(), make_churn(i * 3 + 1, 0));
            lines.insert(path.clone(), i * 17 + 5);
            br.insert(path, (i * 2 + 1) as usize);
        }

        let result = compute_hotspots(&churn, &lines, &br);
        for h in &result {
            let rounded = (h.score * 10_000.0).round() / 10_000.0;
            assert_eq!(
                h.score, rounded,
                "Score {} not rounded to 4 decimal places for {}",
                h.score,
                h.path.as_str()
            );
        }
    }

    #[test]
    fn test_fallback_to_commits_90d() {
        let mut churn = BTreeMap::new();
        churn.insert(CanonicalPath::new("a.rs"), make_churn(0, 15)); // 0 in 30d, uses 90d
        churn.insert(CanonicalPath::new("b.rs"), make_churn(5, 20)); // uses 30d

        let lines = BTreeMap::new();
        let br = BTreeMap::new();

        let result = compute_hotspots(&churn, &lines, &br);
        assert_eq!(result.len(), 2);

        // a.rs should have higher churn rank (15 > 5)
        let a = result.iter().find(|h| h.path.as_str() == "a.rs").unwrap();
        assert_eq!(a.churn_rank, 1);
    }

    #[test]
    fn test_missing_loc_and_blast_radius_defaults() {
        let mut churn = BTreeMap::new();
        churn.insert(CanonicalPath::new("a.rs"), make_churn(10, 0));
        churn.insert(CanonicalPath::new("b.rs"), make_churn(5, 0));

        // No lines or blast radius data provided
        let result = compute_hotspots(&churn, &BTreeMap::new(), &BTreeMap::new());
        assert_eq!(result.len(), 2);

        // Both have LOC=0, BR=0, so those ranks are tied at 1
        // Only churn differs
        let a = result.iter().find(|h| h.path.as_str() == "a.rs").unwrap();
        assert_eq!(a.churn_rank, 1);
        assert_eq!(a.loc_rank, 1); // tied
        assert_eq!(a.blast_radius_rank, 1); // tied
    }

    #[test]
    fn test_tie_breaking_deterministic() {
        let mut churn = BTreeMap::new();
        churn.insert(CanonicalPath::new("a.rs"), make_churn(5, 0));
        churn.insert(CanonicalPath::new("b.rs"), make_churn(5, 0));

        let mut lines = BTreeMap::new();
        lines.insert(CanonicalPath::new("a.rs"), 100u32);
        lines.insert(CanonicalPath::new("b.rs"), 100u32);

        let mut br = BTreeMap::new();
        br.insert(CanonicalPath::new("a.rs"), 5usize);
        br.insert(CanonicalPath::new("b.rs"), 5usize);

        let result1 = compute_hotspots(&churn, &lines, &br);
        let result2 = compute_hotspots(&churn, &lines, &br);

        assert_eq!(result1.len(), result2.len());
        for (a, b) in result1.iter().zip(result2.iter()) {
            assert_eq!(a.path.as_str(), b.path.as_str());
            assert_eq!(a.score, b.score);
        }

        // With all values tied, both should have score 1.0 and same rank
        assert_eq!(result1[0].score, result1[1].score);
    }

    #[test]
    fn test_sorted_by_score_descending() {
        let mut churn = BTreeMap::new();
        let mut lines = BTreeMap::new();
        let mut br = BTreeMap::new();

        for i in 0..7 {
            let path = CanonicalPath::new(format!("file{i}.rs"));
            churn.insert(path.clone(), make_churn(i + 1, 0));
            lines.insert(path.clone(), (i + 1) * 20);
            br.insert(path, (i + 1) as usize);
        }

        let result = compute_hotspots(&churn, &lines, &br);
        for w in result.windows(2) {
            assert!(
                w[0].score >= w[1].score,
                "Not sorted: {} >= {} failed",
                w[0].score,
                w[1].score
            );
        }
    }
}
