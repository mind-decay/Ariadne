/// Compute the 2D Pareto frontier.
///
/// For each point (effort_score, impact_score):
/// - Lower effort is better
/// - Higher impact is better
/// - A point is dominated if another point has <= effort AND >= impact (strict in at least one)
///
/// Returns one (is_on_frontier, dominated_by_index) per input point.
/// `dominated_by_index` is the index of the first dominating point (iteration order: 0..n).
///
/// Precondition: all f64 values must be finite.
pub fn pareto_frontier(points: &[(f64, f64)]) -> Vec<(bool, Option<usize>)> {
    if points.is_empty() {
        return Vec::new();
    }

    debug_assert!(
        points.iter().all(|(e, i)| e.is_finite() && i.is_finite()),
        "all pareto_frontier input values must be finite"
    );

    let n = points.len();
    let mut results = Vec::with_capacity(n);

    for i in 0..n {
        let mut dominated_by = None;
        for j in 0..n {
            if j == i {
                continue;
            }
            // j dominates i if j has <= effort AND >= impact, with at least one strict
            if points[j].0 <= points[i].0
                && points[j].1 >= points[i].1
                && (points[j].0 < points[i].0 || points[j].1 > points[i].1)
            {
                dominated_by = Some(j);
                break;
            }
        }
        match dominated_by {
            Some(j) => results.push((false, Some(j))),
            None => results.push((true, None)),
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input() {
        assert_eq!(pareto_frontier(&[]), vec![]);
    }

    #[test]
    fn single_point() {
        let result = pareto_frontier(&[(0.5, 0.5)]);
        assert_eq!(result, vec![(true, None)]);
    }

    #[test]
    fn all_on_frontier() {
        // (low effort, low impact) and (high effort, high impact) — neither dominates
        let result = pareto_frontier(&[(0.2, 0.3), (0.8, 0.9)]);
        assert_eq!(result, vec![(true, None), (true, None)]);
    }

    #[test]
    fn one_dominates_all() {
        // Point 0 has lowest effort and highest impact
        let points = [(0.1, 0.9), (0.5, 0.5), (0.8, 0.3)];
        let result = pareto_frontier(&points);
        assert_eq!(result[0], (true, None));
        assert!(!result[1].0);
        assert_eq!(result[1].1, Some(0));
        assert!(!result[2].0);
        assert_eq!(result[2].1, Some(0));
    }

    #[test]
    fn duplicate_points() {
        let result = pareto_frontier(&[(0.5, 0.5), (0.5, 0.5)]);
        // Neither strictly dominates the other
        assert_eq!(result, vec![(true, None), (true, None)]);
    }

    #[test]
    fn same_effort_different_impact() {
        // Same effort, different impact — only highest impact on frontier
        let result = pareto_frontier(&[(0.5, 0.3), (0.5, 0.7), (0.5, 0.9)]);
        // Point 2 (impact 0.9) dominates point 0 (impact 0.3) and point 1 (impact 0.7)
        assert!(!result[0].0); // dominated
        assert!(!result[1].0); // dominated
        assert_eq!(result[2], (true, None)); // on frontier
    }

    #[test]
    fn same_impact_different_effort() {
        // Same impact, different effort — only lowest effort on frontier
        let result = pareto_frontier(&[(0.3, 0.5), (0.7, 0.5), (0.9, 0.5)]);
        assert_eq!(result[0], (true, None)); // lowest effort, on frontier
        assert!(!result[1].0); // dominated by 0
        assert!(!result[2].0); // dominated by 0
    }

    #[test]
    fn boundary_dominance() {
        // Equal effort, strictly higher impact → first dominates second
        let result = pareto_frontier(&[(0.3, 0.7), (0.3, 0.5)]);
        assert_eq!(result[0], (true, None));
        assert_eq!(result[1], (false, Some(0)));
    }

    #[test]
    fn dominated_by_index_valid() {
        let points = [(0.1, 0.9), (0.5, 0.5), (0.3, 0.7), (0.9, 0.1)];
        let result = pareto_frontier(&points);
        for (i, (on_frontier, dominated_by)) in result.iter().enumerate() {
            if let Some(j) = dominated_by {
                assert!(!on_frontier);
                assert_ne!(i, *j);
                // Verify j actually dominates i
                assert!(points[*j].0 <= points[i].0);
                assert!(points[*j].1 >= points[i].1);
                assert!(points[*j].0 < points[i].0 || points[*j].1 > points[i].1);
            } else {
                assert!(on_frontier);
            }
        }
    }
}
